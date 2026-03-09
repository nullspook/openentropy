//! `openentropy compare` — differential analysis of two recorded sessions.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use openentropy_core::analysis;
use openentropy_core::comparison::{self, ComparisonResult};
use openentropy_core::conditioning::{MinEntropyReport, min_entropy_estimate};
use openentropy_core::session::{self, SessionMeta};
use openentropy_core::trials::{TrialAnalysis, TrialConfig, stouffer_combine, trial_analysis};
use serde::Serialize;

/// Arguments for the compare command.
pub struct CompareArgs {
    pub session_a: String,
    pub session_b: String,
    pub output: Option<String>,
    pub entropy: bool,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize)]
struct SourcePairReport {
    source: String,
    size_a: usize,
    size_b: usize,
    analysis_a: analysis::SourceAnalysis,
    analysis_b: analysis::SourceAnalysis,
    comparison: ComparisonResult,
    trials_a: TrialAnalysis,
    trials_b: TrialAnalysis,
    #[serde(skip_serializing_if = "Option::is_none")]
    entropy_a: Option<MinEntropyReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    entropy_b: Option<MinEntropyReport>,
}

/// Run the compare command.
pub fn run(args: CompareArgs) {
    let profile = openentropy_core::AnalysisProfile::parse(&args.profile);
    let config = profile.to_config();
    let do_entropy = args.entropy || config.entropy;

    let dir_a = PathBuf::from(&args.session_a);
    let dir_b = PathBuf::from(&args.session_b);

    // Validate session directories.
    for (label, dir) in [("A", &dir_a), ("B", &dir_b)] {
        if !dir.join("session.json").exists() {
            eprintln!("Not a session directory ({label}): {}", dir.display());
            eprintln!("Expected session.json in that directory.");
            std::process::exit(1);
        }
    }

    // Load metadata.
    let meta_a = super::read_session_meta(&dir_a);
    let meta_b = super::read_session_meta(&dir_b);
    let mut data_a = load_session_source_data(&dir_a);
    let mut data_b = load_session_source_data(&dir_b);
    let (common_sources, only_a, only_b) = comparable_source_names(&data_a, &data_b);

    if common_sources.is_empty() {
        eprintln!("No common sources with recorded raw data were found between these sessions.");
        eprintln!("  Session A sources: {}", join_source_names(data_a.keys()));
        eprintln!("  Session B sources: {}", join_source_names(data_b.keys()));
        std::process::exit(1);
    }

    // --- Session header ---
    println!("Differential Analysis \u{2014} Session A vs Session B\n");
    println!(
        "Profile: {} | Entropy: {}",
        args.profile,
        if do_entropy { "on" } else { "off" }
    );
    print_session_header("A", &dir_a, &meta_a, total_source_bytes(&data_a));
    print_session_header("B", &dir_b, &meta_b, total_source_bytes(&data_b));
    println!(
        "Compared common sources (raw indexed data): {}",
        common_sources.join(", ")
    );
    print_skipped_sources(&only_a, &only_b);
    if meta_a.conditioning != meta_b.conditioning {
        println!(
            "Note: session conditioning differs (A: {}, B: {}), but `compare` uses raw indexed samples, so conditioned output is not part of this analysis.",
            meta_a.conditioning, meta_b.conditioning
        );
    }
    println!();

    let reports = build_source_pair_reports(&mut data_a, &mut data_b, &common_sources, do_entropy);

    for report in &reports {
        print_source_pair_report(report, reports.len() > 1);
    }

    // --- Optional JSON output ---
    if let Some(path) = &args.output {
        let session_a_sources: serde_json::Map<String, serde_json::Value> = reports
            .iter()
            .map(|report| {
                (
                    report.source.clone(),
                    serde_json::json!({
                        "bytes": report.size_a,
                        "analysis": report.analysis_a,
                        "trials": report.trials_a,
                        "entropy": report.entropy_a,
                    }),
                )
            })
            .collect();
        let session_b_sources: serde_json::Map<String, serde_json::Value> = reports
            .iter()
            .map(|report| {
                (
                    report.source.clone(),
                    serde_json::json!({
                        "bytes": report.size_b,
                        "analysis": report.analysis_b,
                        "trials": report.trials_b,
                        "entropy": report.entropy_b,
                    }),
                )
            })
            .collect();

        let mut json = serde_json::json!({
            "session_a": {
                "path": args.session_a,
                "meta": meta_a,
                "sources": session_a_sources,
            },
            "session_b": {
                "path": args.session_b,
                "meta": meta_b,
                "sources": session_b_sources,
            },
            "compare_mode": "per_source_raw_indexed",
            "compared_sources": &common_sources,
            "skipped_sources": {
                "only_in_a": &only_a,
                "only_in_b": &only_b,
            },
            "comparisons": &reports,
        });
        if let [report] = reports.as_slice() {
            json["comparison"] = serde_json::json!(report.comparison);
            json["stouffer"] =
                serde_json::json!(stouffer_combine(&[&report.trials_a, &report.trials_b,]));
        }
        super::write_json(&json, path, "Comparison results");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_session_source_data(session_dir: &Path) -> BTreeMap<String, Vec<u8>> {
    match session::load_session_raw_data(session_dir) {
        Ok(data) => data.into_iter().collect(),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                eprintln!(
                    "Missing raw.bin or raw_index.csv in {}",
                    session_dir.display()
                );
            } else {
                eprintln!(
                    "Failed to load indexed raw session data from {}: {e}",
                    session_dir.display()
                );
            }
            std::process::exit(1);
        }
    }
}

fn comparable_source_names(
    data_a: &BTreeMap<String, Vec<u8>>,
    data_b: &BTreeMap<String, Vec<u8>>,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let sources_a: BTreeSet<String> = data_a.keys().cloned().collect();
    let sources_b: BTreeSet<String> = data_b.keys().cloned().collect();

    let common = sources_a.intersection(&sources_b).cloned().collect();
    let only_a = sources_a.difference(&sources_b).cloned().collect();
    let only_b = sources_b.difference(&sources_a).cloned().collect();

    (common, only_a, only_b)
}

fn total_source_bytes(data: &BTreeMap<String, Vec<u8>>) -> usize {
    data.values().map(Vec::len).sum()
}

fn join_source_names<'a>(names: impl Iterator<Item = &'a String>) -> String {
    let collected: Vec<&str> = names.map(String::as_str).collect();
    if collected.is_empty() {
        "none".to_string()
    } else {
        collected.join(", ")
    }
}

fn build_source_pair_reports(
    data_a: &mut BTreeMap<String, Vec<u8>>,
    data_b: &mut BTreeMap<String, Vec<u8>>,
    common_sources: &[String],
    do_entropy: bool,
) -> Vec<SourcePairReport> {
    let trial_config = TrialConfig::default();

    common_sources
        .iter()
        .map(|source| {
            let raw_a = data_a.remove(source).unwrap_or_default();
            let raw_b = data_b.remove(source).unwrap_or_default();
            let analysis_a = analysis::full_analysis(source, &raw_a);
            let analysis_b = analysis::full_analysis(source, &raw_b);
            let comparison = comparison::compare_with_analysis(
                &format!("{source} (A)"),
                &raw_a,
                &analysis_a,
                &format!("{source} (B)"),
                &raw_b,
                &analysis_b,
            );
            let trials_a = trial_analysis(&raw_a, &trial_config);
            let trials_b = trial_analysis(&raw_b, &trial_config);

            SourcePairReport {
                source: source.clone(),
                size_a: raw_a.len(),
                size_b: raw_b.len(),
                analysis_a,
                analysis_b,
                comparison,
                trials_a,
                trials_b,
                entropy_a: do_entropy.then(|| min_entropy_estimate(&raw_a)),
                entropy_b: do_entropy.then(|| min_entropy_estimate(&raw_b)),
            }
        })
        .collect()
}

fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn print_session_header(label: &str, dir: &Path, meta: &SessionMeta, raw_size: usize) {
    let tags_str = if meta.tags.is_empty() {
        String::new()
    } else {
        let tags: Vec<String> = meta.tags.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        format!(", {}", tags.join(", "))
    };
    let note_str = meta
        .note
        .as_deref()
        .map(|n| format!(", {n}"))
        .unwrap_or_default();

    println!(
        "  {label}: {} ({}{}{note_str}, {}, {})",
        dir_name(dir),
        meta.sources.join(", "),
        tags_str,
        format_size(raw_size),
        super::format_duration_ms(meta.duration_ms),
    );
}

fn print_skipped_sources(only_a: &[String], only_b: &[String]) {
    if only_a.is_empty() && only_b.is_empty() {
        return;
    }

    println!("Skipped unmatched sources:");
    if !only_a.is_empty() {
        println!("  Only in A: {}", only_a.join(", "));
    }
    if !only_b.is_empty() {
        println!("  Only in B: {}", only_b.join(", "));
    }
}

fn print_entropy_breakdown(label: &str, report: &MinEntropyReport) {
    println!("{label}:");
    for line in format!("{report}").lines() {
        println!("  {line}");
    }
}

fn print_source_pair_report(report: &SourcePairReport, many_sources: bool) {
    if many_sources {
        println!("{}", "=".repeat(72));
        println!(
            "Source: {} (A: {}, B: {})",
            report.source,
            format_size(report.size_a),
            format_size(report.size_b)
        );
        println!("{}", "=".repeat(72));
    } else {
        println!(
            "Comparing source `{}` (A: {}, B: {})\n",
            report.source,
            format_size(report.size_a),
            format_size(report.size_b)
        );
    }

    print_forensic_comparison(&report.analysis_a, &report.analysis_b);

    println!("\nRunning deep differential comparison...\n");
    print_two_sample(&report.comparison);
    print_temporal(&report.comparison);
    print_multi_lag(&report.comparison);
    print_markov(&report.comparison);
    print_digram(&report.comparison);
    print_run_lengths(&report.comparison);
    print_effect_size(&report.comparison);

    println!("\nTrial-Level Analysis (PEAR-style, 200-bit trials):\n");
    print_trial_comparison(&report.trials_a, &report.trials_b, "Session A", "Session B");

    if let (Some(entropy_a), Some(entropy_b)) = (&report.entropy_a, &report.entropy_b) {
        println!("Min-Entropy Breakdown:\n");
        print_entropy_breakdown("  Session A", entropy_a);
        println!();
        print_entropy_breakdown("  Session B", entropy_b);
        println!();
    }
}

fn significance_stars(delta: f64, threshold_2: f64, threshold_3: f64) -> &'static str {
    let abs = delta.abs();
    if abs >= threshold_3 {
        "***"
    } else if abs >= threshold_2 {
        "**"
    } else {
        ""
    }
}

fn print_forensic_comparison(a: &analysis::SourceAnalysis, b: &analysis::SourceAnalysis) {
    println!("Forensic Comparison:");
    println!(
        "  {:<25} {:>12} {:>12} {:>12}",
        "Metric", "Session A", "Session B", "Delta"
    );
    println!(
        "  {} {} {} {}",
        "\u{2500}".repeat(25),
        "\u{2500}".repeat(12),
        "\u{2500}".repeat(12),
        "\u{2500}".repeat(12)
    );

    type Row = (&'static str, f64, f64, &'static str, (f64, f64));
    let rows: &[Row] = &[
        (
            "Shannon H",
            a.shannon_entropy,
            b.shannon_entropy,
            ".4",
            (0.02, 0.05),
        ),
        (
            "Min-Entropy",
            a.min_entropy,
            b.min_entropy,
            ".4",
            (0.1, 0.2),
        ),
        (
            "Byte Mean",
            a.distribution.mean,
            b.distribution.mean,
            ".2",
            (2.0, 5.0),
        ),
        (
            "Variance",
            a.distribution.variance,
            b.distribution.variance,
            ".2",
            (20.0, 50.0),
        ),
        (
            "KS Statistic (vs unif)",
            a.distribution.ks_statistic,
            b.distribution.ks_statistic,
            ".6",
            (0.005, 0.01),
        ),
        (
            "Bit Bias",
            a.bit_bias.overall_bias,
            b.bit_bias.overall_bias,
            ".6",
            (0.002, 0.005),
        ),
    ];

    for &(name, va, vb, fmt, (t2, t3)) in rows {
        let d = vb - va;
        match fmt {
            ".4" => println!(
                "  {:<25} {:>12.4} {:>12.4} {:>+12.4} {}",
                name,
                va,
                vb,
                d,
                significance_stars(d, t2, t3)
            ),
            ".2" => println!(
                "  {:<25} {:>12.2} {:>12.2} {:>+12.2} {}",
                name,
                va,
                vb,
                d,
                significance_stars(d, t2, t3)
            ),
            _ => println!(
                "  {:<25} {:>12.6} {:>12.6} {:>+12.6} {}",
                name,
                va,
                vb,
                d,
                significance_stars(d, t2, t3)
            ),
        }
    }

    // Rows without significance stars.
    let d = b.bit_bias.chi_squared - a.bit_bias.chi_squared;
    println!(
        "  {:<25} {:>12.2} {:>12.2} {:>+12.2}",
        "Bit Chi\u{00b2}", a.bit_bias.chi_squared, b.bit_bias.chi_squared, d,
    );

    let d = b.autocorrelation.max_abs_correlation - a.autocorrelation.max_abs_correlation;
    println!(
        "  {:<25} {:>12.6} {:>12.6} {:>+12.6}",
        "Autocorr max|r|",
        a.autocorrelation.max_abs_correlation,
        b.autocorrelation.max_abs_correlation,
        d,
    );

    let d = b.spectral.flatness - a.spectral.flatness;
    println!(
        "  {:<25} {:>12.4} {:>12.4} {:>+12.4}",
        "Spectral Flatness", a.spectral.flatness, b.spectral.flatness, d,
    );

    let d = b.stationarity.f_statistic - a.stationarity.f_statistic;
    println!(
        "  {:<25} {:>12.2} {:>12.2} {:>+12.2}",
        "Stationarity F", a.stationarity.f_statistic, b.stationarity.f_statistic, d,
    );
}

fn print_two_sample(c: &ComparisonResult) {
    let ts = &c.two_sample;
    println!("Two-Sample Tests (A vs B):");

    // KS
    let ks_verdict = if ts.ks_p_value < 0.001 {
        "***"
    } else if ts.ks_p_value < 0.01 {
        "**"
    } else if ts.ks_p_value < 0.05 {
        "*"
    } else {
        "ns"
    };
    println!(
        "  KS statistic:           {:.6}  (p = {:.4e})  {}",
        ts.ks_statistic, ts.ks_p_value, ks_verdict
    );

    // Chi-squared homogeneity
    let chi2_verdict = if ts.chi2_p_value < 0.001 {
        "***"
    } else if ts.chi2_p_value < 0.01 {
        "**"
    } else if ts.chi2_p_value < 0.05 {
        "*"
    } else {
        "ns"
    };
    let chi2_note = if !ts.chi2_reliable {
        "  (low expected counts)"
    } else {
        ""
    };
    println!(
        "  Chi\u{00b2} homogeneity:      {:.2}  (df={}, p = {:.4e})  {}{}",
        ts.chi2_homogeneity, ts.chi2_df, ts.chi2_p_value, chi2_verdict, chi2_note
    );

    // Mann-Whitney
    let mw_verdict = if ts.mann_whitney_p_value < 0.001 {
        "***"
    } else if ts.mann_whitney_p_value < 0.01 {
        "**"
    } else if ts.mann_whitney_p_value < 0.05 {
        "*"
    } else {
        "ns"
    };
    println!(
        "  Mann-Whitney U (norm):  {:.6}  (p = {:.4e})  {}",
        ts.mann_whitney_u, ts.mann_whitney_p_value, mw_verdict
    );

    println!();
}

fn print_temporal(c: &ComparisonResult) {
    let t = &c.temporal;
    println!(
        "Temporal Anomaly Detection ({}B windows, z vs theoretical):",
        t.window_size
    );

    let ratio = if t.anomaly_count_a > 0 {
        format!(
            " ({:.1}x)",
            t.anomaly_count_b as f64 / t.anomaly_count_a as f64
        )
    } else if t.anomaly_count_b > 0 {
        " (inf x)".to_string()
    } else {
        String::new()
    };

    println!(
        "  A: {} anomalous windows (max |z| = {:.2})",
        t.anomaly_count_a, t.max_z_a
    );
    println!(
        "  B: {} anomalous windows (max |z| = {:.2}){}",
        t.anomaly_count_b, t.max_z_b, ratio
    );

    // Windowed entropy stats.
    if !t.windowed_entropy_a.is_empty() && !t.windowed_entropy_b.is_empty() {
        let mean_ent_a: f64 =
            t.windowed_entropy_a.iter().sum::<f64>() / t.windowed_entropy_a.len() as f64;
        let mean_ent_b: f64 =
            t.windowed_entropy_b.iter().sum::<f64>() / t.windowed_entropy_b.len() as f64;
        let min_ent_a = t
            .windowed_entropy_a
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let min_ent_b = t
            .windowed_entropy_b
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        println!(
            "  Windowed entropy \u{2014} A: mean={:.4} min={:.4}  B: mean={:.4} min={:.4}",
            mean_ent_a, min_ent_a, mean_ent_b, min_ent_b
        );
    }
}

fn print_multi_lag(c: &ComparisonResult) {
    let ml = &c.multi_lag;
    println!("\nMulti-Lag Autocorrelation:");
    println!("  {:<8} {:>12} {:>12} {:>12}", "Lag", "A", "B", "Delta");
    for (i, &lag) in ml.lags.iter().enumerate() {
        let a = ml.autocorr_a[i];
        let b = ml.autocorr_b[i];
        println!("  {:<8} {:>+12.6} {:>+12.6} {:>+12.6}", lag, a, b, b - a);
    }
}

fn print_markov(c: &ComparisonResult) {
    let m = &c.markov;
    println!("\nBit Markov Transitions (P(1|from)):");
    println!(
        "  {:<6} {:>10} {:>10} {:>10} {:>10}",
        "Bit", "A: P(1|0)", "A: P(1|1)", "B: P(1|0)", "B: P(1|1)"
    );
    for bit in 0..8 {
        println!(
            "  bit {:<2} {:>10.4} {:>10.4} {:>10.4} {:>10.4}",
            bit,
            m.transitions_a[bit][0][1],
            m.transitions_a[bit][1][1],
            m.transitions_b[bit][0][1],
            m.transitions_b[bit][1][1],
        );
    }
}

fn print_digram(c: &ComparisonResult) {
    let dg = &c.digram;
    println!("\nDigram Chi\u{00b2} Uniformity:");
    if dg.sufficient_data {
        println!(
            "  A: {:.1}   B: {:.1}   (\u{0394} {:+.1})",
            dg.chi2_a,
            dg.chi2_b,
            dg.chi2_b - dg.chi2_a
        );
    } else {
        println!(
            "  Insufficient data (need >= {} bytes for valid chi\u{00b2} approximation)",
            dg.min_sample_bytes
        );
    }
}

fn print_run_lengths(c: &ComparisonResult) {
    let rl = &c.run_lengths;
    println!("\nRun-Length Distribution (top 5 by length):");

    let top_a: Vec<_> = rl.distribution_a.iter().rev().take(5).collect();
    let top_b: Vec<_> = rl.distribution_b.iter().rev().take(5).collect();

    println!("  A:");
    for (len, count) in &top_a {
        println!("    length {len}: {count} runs");
    }
    println!("  B:");
    for (len, count) in &top_b {
        println!("    length {len}: {count} runs");
    }
}

fn print_trial_comparison(a: &TrialAnalysis, b: &TrialAnalysis, label_a: &str, label_b: &str) {
    println!("  {:<25} {:>12} {:>12}", "Metric", label_a, label_b);
    println!(
        "  {} {} {}",
        "\u{2500}".repeat(25),
        "\u{2500}".repeat(12),
        "\u{2500}".repeat(12)
    );
    println!(
        "  {:<25} {:>12} {:>12}",
        "Num trials", a.num_trials, b.num_trials
    );
    println!(
        "  {:<25} {:>+12.4} {:>+12.4}",
        "Terminal Z", a.terminal_z, b.terminal_z
    );
    println!(
        "  {:<25} {:>+12.6} {:>+12.6}",
        "Effect size", a.effect_size, b.effect_size
    );
    println!(
        "  {:<25} {:>+12.1} {:>+12.1}",
        "Cum. deviation", a.terminal_cumulative_deviation, b.terminal_cumulative_deviation
    );
    println!("  {:<25} {:>12.4} {:>12.4}", "Mean Z", a.mean_z, b.mean_z);
    println!("  {:<25} {:>12.4} {:>12.4}", "Std Z", a.std_z, b.std_z);

    // Stouffer combination
    let stouffer = stouffer_combine(&[a, b]);
    println!();
    println!(
        "  Stouffer combined Z:  {:+.4} (p = {:.2e})",
        stouffer.stouffer_z, stouffer.p_value
    );
    println!(
        "  Combined effect size: {:+.6} ({} total trials)",
        stouffer.combined_effect_size, stouffer.total_trials
    );
    println!();
}

fn print_effect_size(c: &ComparisonResult) {
    let cd = c.aggregate.cohens_d;
    let cliff = c.two_sample.cliffs_delta;

    let cliff_mag = if cliff.abs() < 0.147 {
        "negligible"
    } else if cliff.abs() < 0.33 {
        "small"
    } else if cliff.abs() < 0.474 {
        "medium"
    } else {
        "large"
    };

    let cohen_mag = if cd.abs() < 0.2 {
        "negligible"
    } else if cd.abs() < 0.5 {
        "small"
    } else if cd.abs() < 0.8 {
        "medium"
    } else {
        "large"
    };

    println!("\nEffect Size:");
    println!("  Cliff's delta: {:+.4} ({cliff_mag})", cliff);
    println!("  Cohen's d:     {:+.4} ({cohen_mag})", cd);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparable_source_names_returns_common_and_skipped_sets() {
        let data_a = BTreeMap::from([
            ("clock_jitter".to_string(), vec![1, 2, 3]),
            ("sleep_jitter".to_string(), vec![4, 5]),
        ]);
        let data_b = BTreeMap::from([
            ("clock_jitter".to_string(), vec![9, 8, 7]),
            ("mach_timing".to_string(), vec![6]),
        ]);

        let (common, only_a, only_b) = comparable_source_names(&data_a, &data_b);

        assert_eq!(common, vec!["clock_jitter".to_string()]);
        assert_eq!(only_a, vec!["sleep_jitter".to_string()]);
        assert_eq!(only_b, vec!["mach_timing".to_string()]);
    }

    #[test]
    fn total_source_bytes_sums_grouped_lengths() {
        let data = BTreeMap::from([
            ("a".to_string(), vec![1, 2, 3]),
            ("b".to_string(), vec![4, 5]),
        ]);

        assert_eq!(total_source_bytes(&data), 5);
    }
}
