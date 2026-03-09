//! `openentropy record` — record a session of entropy collection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use openentropy_core::conditioning::condition;
use openentropy_core::session::{SessionConfig, SessionMeta, SessionWriter};

const DEFAULT_SWEEP_TIMEOUT_SECS: f64 = 10.0;

pub struct RecordArgs {
    pub positional: Vec<String>,
    pub all: bool,
    pub duration: Option<String>,
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub output: Option<String>,
    pub interval: Option<String>,
    pub analyze: bool,
    pub conditioning: String,
    pub include_telemetry: bool,
    pub calibrate: bool,
}

/// Run the record command.
#[allow(clippy::too_many_lines)]
pub fn run(args: RecordArgs) {
    // Parse conditioning mode
    let mode = super::parse_conditioning(&args.conditioning);

    // Resolve sources: positional names, --all, or default fast sources
    let sources = super::resolve_sources(&args.positional, args.all);

    // Build pool from the resolved sources for per-source raw byte collection
    let mut pool = openentropy_core::EntropyPool::new(None);
    let available: Vec<String> = sources.iter().map(|s| s.name().to_string()).collect();
    for source in sources {
        pool.add_source(source);
    }

    // Calibration check
    if args.calibrate {
        use openentropy_core::trials::{TrialConfig, calibration_check};

        println!("Running calibration check...\n");
        let config = TrialConfig::default();
        let mut any_failed = false;

        for source_name in &available {
            let cal_bytes = match pool.get_source_raw_bytes(source_name, 5000) {
                Some(bytes) => bytes,
                None => {
                    eprintln!("  {source_name}: FAIL (calibration collection error)");
                    any_failed = true;
                    continue;
                }
            };
            if cal_bytes.is_empty() {
                eprintln!("  {source_name}: FAIL (no data returned during calibration)");
                any_failed = true;
                continue;
            }
            let result = calibration_check(&cal_bytes, &config);
            let status = if result.is_suitable { "PASS" } else { "FAIL" };
            println!(
                "  {source_name}: {status} ({} trials, Z={:+.4}, bias={:.6}, H={:.4})",
                result.analysis.num_trials,
                result.analysis.terminal_z,
                result.bit_bias,
                result.shannon_entropy,
            );
            for warning in &result.warnings {
                println!("    ! {warning}");
            }
            if !result.is_suitable {
                any_failed = true;
            }
        }

        if any_failed {
            eprintln!("\nCalibration failed. Source(s) not suitable for PEAR-style experiments.");
            eprintln!("Fix source issues or omit --calibrate to skip.");
            std::process::exit(1);
        }
        println!("\nCalibration passed. Proceeding with recording.\n");
    }

    // Parse duration
    let max_duration = args.duration.as_deref().map(parse_duration);

    // Parse interval
    let interval_dur = args.interval.as_deref().map(parse_duration);

    // Parse tags
    let mut tag_map = HashMap::new();
    for tag in &args.tags {
        if let Some((k, v)) = tag.split_once(':') {
            tag_map.insert(k.to_string(), v.to_string());
        } else {
            eprintln!("Warning: ignoring malformed tag '{tag}' (expected key:value)");
        }
    }

    // Build session config
    let output_dir = args
        .output
        .as_ref()
        .map_or_else(|| PathBuf::from("sessions"), PathBuf::from);

    let include_telemetry = args.include_telemetry;

    let config = SessionConfig {
        sources: available.clone(),
        conditioning: mode,
        interval: interval_dur,
        output_dir,
        tags: tag_map,
        note: args.note.clone(),
        duration: max_duration,
        sample_size: 1000,
        include_analysis: args.analyze,
        include_telemetry,
    };

    // Create session writer
    let mut writer = match SessionWriter::new(config) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error creating session: {e}");
            std::process::exit(1);
        }
    };

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    if let Err(e) = ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }) {
        eprintln!("Warning: could not set Ctrl+C handler: {e}");
    }

    // Print session start info
    let session_dir = writer.session_dir().to_path_buf();
    println!("Recording session");
    println!("  Sources:   {}", available.join(", "));
    println!("  Conditioning: {mode}");
    if let Some(d) = max_duration {
        println!("  Duration:  {}s", d.as_secs());
    } else {
        println!("  Duration:  until Ctrl+C");
    }
    if let Some(iv) = interval_dur {
        println!("  Interval:  {}ms", iv.as_millis());
    } else {
        println!("  Interval:  continuous");
    }
    println!(
        "  Analysis:  {}",
        if args.analyze { "enabled" } else { "disabled" }
    );
    println!(
        "  Telemetry: {}",
        if include_telemetry {
            "enabled (session start/end snapshot)"
        } else {
            "disabled"
        }
    );
    println!("  Output:    {}", session_dir.display());
    println!();

    // Recording loop
    let start = Instant::now();
    let stop_at = build_stop_at(start, max_duration);
    let mut had_write_error = false;

    'outer: while running.load(Ordering::SeqCst) {
        // Check duration limit
        if deadline_reached(Instant::now(), stop_at) {
            break;
        }

        // Collect from all enabled sources under one shared timeout budget.
        // This keeps `record --all --duration ...` bounded even when a subset
        // of sources are slow or temporarily hung.
        let sweep_timeout_secs = sweep_timeout_secs(Instant::now(), stop_at);
        if sweep_timeout_secs <= 0.0 {
            break;
        }
        let raw_by_source = pool.collect_enabled_raw_n(&available, sweep_timeout_secs, 1000);

        for source_name in &available {
            let Some(raw) = raw_by_source.get(source_name) else {
                continue;
            };
            let conditioned = condition(raw, raw.len(), mode);

            if let Err(e) = writer.write_sample(source_name, raw, &conditioned) {
                eprintln!("\nError writing sample: {e}");
                had_write_error = true;
                break 'outer;
            }
        }

        // Print status
        let elapsed = start.elapsed();
        let total = writer.total_samples();
        print!(
            "\r  Samples: {total:<8} Elapsed: {:.1}s",
            elapsed.as_secs_f64()
        );
        let _ = std::io::Write::flush(&mut std::io::stdout());

        // Wait for interval if configured
        if let Some(iv) = interval_dur {
            let deadline = sleep_deadline(Instant::now(), iv, stop_at);
            while Instant::now() < deadline && running.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    println!();
    println!();

    if had_write_error {
        eprintln!("Recording stopped due to write error.");
    }

    // Finalize session
    match writer.finish() {
        Ok(dir) => {
            println!("Session saved to {}", dir.display());
            println!("  session.json          — metadata");
            println!("  samples.csv           — per-sample raw/conditioned metrics");
            println!("  raw.bin               — raw entropy bytes");
            println!("  raw_index.csv         — byte offset index for raw.bin");
            println!("  conditioned.bin       — conditioned entropy bytes");
            println!("  conditioned_index.csv — byte offset index for conditioned.bin");
            if include_telemetry {
                let meta_path = dir.join("session.json");
                if let Ok(raw) = std::fs::read_to_string(&meta_path)
                    && let Ok(meta) = serde_json::from_str::<SessionMeta>(&raw)
                    && let Some(t) = meta.telemetry_v1
                {
                    println!(
                        "  telemetry_v1:         {} ({:.1}s, {} metrics)",
                        t.model_id,
                        t.elapsed_ms as f64 / 1000.0,
                        t.end.metrics.len()
                    );
                }
            }
        }
        Err(e) => {
            eprintln!("Error finalizing session: {e}");
            std::process::exit(1);
        }
    }
}

/// Parse a duration string like "5m", "30s", "1h", "100ms".
fn parse_duration(s: &str) -> Duration {
    let s = s.trim();

    let (numeric, multiplier) = if let Some(rest) = s.strip_suffix("ms") {
        (rest, 1u64)
    } else if let Some(rest) = s.strip_suffix('s') {
        (rest, 1000)
    } else if let Some(rest) = s.strip_suffix('m') {
        (rest, 60_000)
    } else if let Some(rest) = s.strip_suffix('h') {
        (rest, 3_600_000)
    } else {
        // Assume seconds
        (s, 1000)
    };

    let value: u64 = numeric.parse().unwrap_or_else(|_| {
        eprintln!("Invalid duration: {s}");
        std::process::exit(1);
    });

    let millis = value.checked_mul(multiplier).unwrap_or_else(|| {
        eprintln!("Duration too large: {s}");
        std::process::exit(1);
    });
    Duration::from_millis(millis)
}

fn build_stop_at(start: Instant, max_duration: Option<Duration>) -> Option<Instant> {
    max_duration.and_then(|duration| start.checked_add(duration))
}

fn sweep_timeout_secs(now: Instant, stop_at: Option<Instant>) -> f64 {
    match stop_at {
        Some(deadline) => deadline
            .saturating_duration_since(now)
            .as_secs_f64()
            .min(DEFAULT_SWEEP_TIMEOUT_SECS),
        None => DEFAULT_SWEEP_TIMEOUT_SECS,
    }
}

fn deadline_reached(now: Instant, stop_at: Option<Instant>) -> bool {
    stop_at.is_some_and(|deadline| now >= deadline)
}

fn sleep_deadline(now: Instant, interval: Duration, stop_at: Option<Instant>) -> Instant {
    let interval_deadline = now.checked_add(interval).unwrap_or(now);
    match stop_at {
        Some(deadline) if deadline < interval_deadline => deadline,
        _ => interval_deadline,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_SWEEP_TIMEOUT_SECS, build_stop_at, deadline_reached, sleep_deadline,
        sweep_timeout_secs,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn build_stop_at_adds_duration_when_present() {
        let start = Instant::now();
        let stop_at = build_stop_at(start, Some(Duration::from_secs(2))).unwrap();
        assert!(stop_at > start);
    }

    #[test]
    fn deadline_reached_returns_false_without_deadline() {
        assert!(!deadline_reached(Instant::now(), None));
    }

    #[test]
    fn deadline_reached_returns_true_at_or_after_deadline() {
        let now = Instant::now();
        let deadline = now.checked_sub(Duration::from_millis(1)).unwrap();
        assert!(deadline_reached(now, Some(deadline)));
    }

    #[test]
    fn sweep_timeout_secs_uses_default_without_deadline() {
        let timeout = sweep_timeout_secs(Instant::now(), None);
        assert_eq!(timeout, DEFAULT_SWEEP_TIMEOUT_SECS);
    }

    #[test]
    fn sweep_timeout_secs_caps_remaining_time() {
        let now = Instant::now();
        let deadline = now.checked_add(Duration::from_millis(250)).unwrap();
        let timeout = sweep_timeout_secs(now, Some(deadline));
        assert!(timeout > 0.0);
        assert!(timeout <= 0.25);
    }

    #[test]
    fn sleep_deadline_clamps_interval_to_stop_at() {
        let now = Instant::now();
        let stop_at = now.checked_add(Duration::from_millis(50)).unwrap();
        let deadline = sleep_deadline(now, Duration::from_secs(1), Some(stop_at));
        assert_eq!(deadline, stop_at);
    }
}
