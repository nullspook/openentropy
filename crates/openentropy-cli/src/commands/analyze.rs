use std::collections::HashMap;
use std::time::Instant;

use openentropy_core::analysis;
use openentropy_core::conditioning::{ConditioningMode, condition, min_entropy_estimate};
use openentropy_core::verdict::{
    metric_or_na, verdict_anderson_darling, verdict_apen, verdict_autocorr, verdict_bias,
    verdict_bientropy, verdict_compression, verdict_corrdim, verdict_cramer_von_mises, verdict_dfa,
    verdict_distribution, verdict_hurst, verdict_ljung_box, verdict_lyapunov, verdict_permen,
    verdict_rqa_det, verdict_runs, verdict_sampen, verdict_spectral, verdict_stationarity,
};

pub struct AnalyzeArgs {
    pub positional: Vec<String>,
    pub all: bool,
    pub profile: String,
    pub samples: Option<usize>,
    pub output: Option<String>,
    pub cross_correlation: bool,
    pub entropy: bool,
    pub conditioning: Option<String>,
    pub include_telemetry: bool,
    pub report: bool,
    pub chaos: bool,
    pub temporal: bool,
    pub statistics: bool,
    pub synchrony: bool,
    pub chaos_extended: bool,
}

/// Resolved values after merging profile defaults with explicit flags.
struct Resolved {
    samples: usize,
    conditioning: String,
    entropy: bool,
    report: bool,
    cross_correlation: bool,
    chaos: bool,
    temporal: bool,
    statistics: bool,
    synchrony: bool,
    chaos_extended: bool,
}

pub fn run(args: AnalyzeArgs) {
    let profile = openentropy_core::AnalysisProfile::parse(&args.profile);
    let config = profile.to_config();

    let resolved = Resolved {
        samples: args.samples.unwrap_or(match profile {
            openentropy_core::AnalysisProfile::Quick => 10_000,
            openentropy_core::AnalysisProfile::Deep => 100_000,
            _ => 50_000,
        }),
        conditioning: args
            .conditioning
            .as_deref()
            .unwrap_or(match profile {
                openentropy_core::AnalysisProfile::Security => "sha256",
                _ => "raw",
            })
            .to_string(),
        entropy: args.entropy || config.entropy,
        report: args.report || matches!(profile, openentropy_core::AnalysisProfile::Security),
        cross_correlation: args.cross_correlation || config.cross_correlation,
        chaos: args.chaos || config.chaos,
        temporal: args.temporal || matches!(profile, openentropy_core::AnalysisProfile::Deep),
        statistics: args.statistics || matches!(profile, openentropy_core::AnalysisProfile::Deep),
        synchrony: args.synchrony,
        chaos_extended: args.chaos_extended
            || matches!(profile, openentropy_core::AnalysisProfile::Deep),
    };

    println!(
        "Profile: {} | Samples: {} | Conditioning: {}",
        args.profile, resolved.samples, resolved.conditioning
    );

    if resolved.report {
        run_report(&args, &resolved);
    } else {
        run_analysis(&args, &resolved);
    }
}

// ---------------------------------------------------------------------------
// Forensic analysis path (default)
// ---------------------------------------------------------------------------

fn run_analysis(args: &AnalyzeArgs, resolved: &Resolved) {
    let telemetry = super::telemetry::TelemetryCapture::start(args.include_telemetry);
    let mode = super::parse_conditioning(&resolved.conditioning);

    let sources = super::resolve_sources(&args.positional, args.all);

    println!("Forensic analysis — spectral, bias, stationarity, runs, distribution");
    println!("(For throughput/stability ranking, use `bench` instead.)\n");
    println!(
        "Analyzing {} source(s), {} samples each...\n",
        sources.len(),
        resolved.samples,
    );

    let mut all_results = Vec::new();
    let mut all_data: Vec<(String, Vec<u8>)> = Vec::new();

    for source in &sources {
        let name = source.name().to_string();
        print!("  {name}...");
        let t0 = Instant::now();
        let mut data = source.collect(resolved.samples);
        if data.is_empty() {
            // Retry once — USB/hardware sources may need reconnection time.
            std::thread::sleep(std::time::Duration::from_secs(1));
            data = source.collect(resolved.samples);
        }
        let collect_time = t0.elapsed();

        if data.is_empty() {
            println!(" (no data, skipped)");
            continue;
        }

        let result = analysis::full_analysis(&name, &data);
        println!(" {:.2}s, {} bytes", collect_time.as_secs_f64(), data.len());

        print_source_forensics(&result);

        if resolved.entropy {
            let entropy_input = if mode == ConditioningMode::Raw {
                data.clone()
            } else {
                condition(&data, data.len(), mode)
            };
            let report = min_entropy_estimate(&entropy_input);
            let report_str = format!("{report}");
            println!(
                "  ├─ Min-Entropy Breakdown (conditioning: {}, {} bytes)",
                resolved.conditioning,
                entropy_input.len()
            );
            for line in report_str.lines() {
                println!("  │  {line}");
            }
            println!("  └─");
        }

        all_results.push(result);

        if resolved.cross_correlation
            || resolved.chaos
            || resolved.temporal
            || resolved.statistics
            || resolved.synchrony
            || resolved.chaos_extended
        {
            all_data.push((name, data));
        }
    }

    // Cross-correlation matrix.
    let cross_matrix = if resolved.cross_correlation && all_data.len() >= 2 {
        Some(analysis::cross_correlation_matrix(&all_data))
    } else {
        None
    };

    if let Some(ref matrix) = cross_matrix {
        super::print_cross_correlation(matrix, all_data.len());
    }

    let chaos_results: Vec<(String, serde_json::Value)> = if resolved.chaos {
        println!();
        println!("  ┌─ Chaos Analysis (Core)");
        let results: Vec<_> = all_data
            .iter()
            .map(|(name, data)| {
                let hurst = openentropy_core::chaos::hurst_exponent(data);
                let lyapunov = openentropy_core::chaos::lyapunov_exponent(data);
                let correlation_dimension = openentropy_core::chaos::correlation_dimension(data);
                let bientropy = openentropy_core::chaos::bientropy(data);
                let epiplexity = openentropy_core::chaos::epiplexity(data);

                let hurst_value = metric_or_na(hurst.hurst_exponent, hurst.is_valid);
                let hurst_r2 = metric_or_na(hurst.r_squared, hurst.is_valid);
                let lyapunov_value = metric_or_na(lyapunov.lyapunov_exponent, lyapunov.is_valid);
                let corrdim_value = metric_or_na(
                    correlation_dimension.dimension,
                    correlation_dimension.is_valid,
                );
                let bientropy_value = metric_or_na(bientropy.bien, bientropy.is_valid);
                let tbientropy_value = metric_or_na(bientropy.tbien, bientropy.is_valid);
                let epiplexity_ratio =
                    metric_or_na(epiplexity.compression_ratio, epiplexity.is_valid);
                println!("  │");
                println!("  │  {}", name);
                println!(
                    "  │  {:<22} {:>4}  H={} (R²={})",
                    "Hurst exponent",
                    verdict_hurst(hurst.hurst_exponent).as_str(),
                    hurst_value,
                    hurst_r2,
                );
                println!(
                    "  │  {:<22} {:>4}  λ={}",
                    "Lyapunov exponent",
                    verdict_lyapunov(lyapunov.lyapunov_exponent).as_str(),
                    lyapunov_value,
                );
                println!(
                    "  │  {:<22} {:>4}  D₂={}",
                    "Correlation dim",
                    verdict_corrdim(correlation_dimension.dimension).as_str(),
                    corrdim_value,
                );
                println!(
                    "  │  {:<22} {:>4}  BiEn={} TBiEn={}",
                    "BiEntropy",
                    verdict_bientropy(bientropy.bien).as_str(),
                    bientropy_value,
                    tbientropy_value,
                );
                println!(
                    "  │  {:<22} {:>4}  ratio={}",
                    "Epiplexity",
                    verdict_compression(epiplexity.compression_ratio).as_str(),
                    epiplexity_ratio,
                );
                (
                    name.clone(),
                    serde_json::json!({
                        "hurst": hurst,
                        "lyapunov": lyapunov,
                        "correlation_dimension": correlation_dimension,
                        "bientropy": bientropy,
                        "epiplexity": epiplexity,
                    }),
                )
            })
            .collect();
        println!("  └─");
        results
    } else {
        Vec::new()
    };

    let mut statistics_results: HashMap<String, serde_json::Value> = HashMap::new();
    let mut temporal_results: HashMap<String, serde_json::Value> = HashMap::new();
    let mut chaos_extended_results: HashMap<String, serde_json::Value> = HashMap::new();
    let mut synchrony_results: HashMap<String, serde_json::Value> = HashMap::new();

    // Statistics analysis.
    if resolved.statistics {
        println!();
        println!("  ┌─ Statistics Analysis");
        for (name, data) in &all_data {
            let stats = openentropy_core::statistics::statistics_analysis(data);
            println!("  │");
            println!("  │  {}", name);
            println!(
                "  │  {:<22} {:>4}  W={:.4} p={:.4}",
                "Cramér-von Mises",
                verdict_cramer_von_mises(stats.cramer_von_mises.p_value).as_str(),
                stats.cramer_von_mises.statistic,
                stats.cramer_von_mises.p_value,
            );
            println!(
                "  │  {:<22} {:>4}  Q={:.4} p={:.4}",
                "Ljung-Box",
                verdict_ljung_box(stats.ljung_box.p_value).as_str(),
                stats.ljung_box.q_statistic,
                stats.ljung_box.p_value,
            );
            println!(
                "  │  {:<22}  mean_gap={:.4} expected={:.4}",
                "Gap test", stats.gap_test.mean_gap, stats.gap_test.expected_gap,
            );
            statistics_results.insert(name.clone(), serde_json::json!(stats));
        }
        println!("  └─");
    }

    // Temporal analysis.
    if resolved.temporal {
        println!();
        println!("  ┌─ Temporal Analysis");
        for (name, data) in &all_data {
            let temporal = openentropy_core::temporal::temporal_analysis_suite(data);
            println!("  │");
            println!("  │  {}", name);
            println!(
                "  │  {:<22}  change_points={}",
                "Change-point detection",
                temporal.change_points.change_points.len(),
            );
            println!(
                "  │  {:<22}  anomalies={}",
                "Anomaly detection",
                temporal.anomalies.anomalies.len(),
            );
            println!(
                "  │  {:<22}  bursts={}",
                "Burst detection",
                temporal.bursts.bursts.len(),
            );
            println!(
                "  │  {:<22}  shifts={}",
                "Shift detection",
                temporal.shifts.shifts.len(),
            );
            println!(
                "  │  {:<22}  segments={}",
                "Temporal drift",
                temporal.drift.segments.len(),
            );
            println!(
                "  │  {:<22}  drift_score={:.4}",
                "Drift score", temporal.drift.drift_slope,
            );
            temporal_results.insert(name.clone(), serde_json::json!(temporal));
        }
        println!("  └─");
    }

    // Chaos extended analysis.
    if resolved.chaos_extended {
        println!();
        println!("  ┌─ Chaos Extended Analysis");
        for (name, data) in &all_data {
            let sampen = openentropy_core::chaos::sample_entropy_default(data);
            let apen = openentropy_core::analysis::approximate_entropy_default(data);
            let dfa = openentropy_core::chaos::dfa_default(data);
            let rqa = openentropy_core::chaos::rqa_default(data);
            let rolling_hurst = openentropy_core::chaos::rolling_hurst_default(data);
            let bootstrap_hurst = openentropy_core::chaos::bootstrap_hurst_default(data);
            let permen = openentropy_core::analysis::permutation_entropy_default(data);
            let anderson_darling = openentropy_core::analysis::anderson_darling(data);
            println!("  │");
            println!("  │  {}", name);
            println!(
                "  │  {:<22} {:>4}  SampEn={:.4}",
                "Sample entropy",
                verdict_sampen(sampen.sample_entropy).as_str(),
                sampen.sample_entropy,
            );
            println!(
                "  │  {:<22} {:>4}  ApEn={:.4}",
                "Approx entropy",
                verdict_apen(apen.apen).as_str(),
                apen.apen,
            );
            println!(
                "  │  {:<22} {:>4}  α={:.4}",
                "DFA",
                verdict_dfa(dfa.alpha).as_str(),
                dfa.alpha,
            );
            println!(
                "  │  {:<22} {:>4}  DET={:.4}",
                "RQA",
                verdict_rqa_det(rqa.determinism).as_str(),
                rqa.determinism,
            );
            println!(
                "  │  {:<22}  mean_H={} p={}",
                "Bootstrap Hurst",
                metric_or_na(
                    bootstrap_hurst.mean_surrogate_hurst,
                    bootstrap_hurst.is_valid
                ),
                metric_or_na(bootstrap_hurst.p_value, bootstrap_hurst.is_valid),
            );
            println!(
                "  │  {:<22}  mean_H={}",
                "Rolling Hurst",
                metric_or_na(rolling_hurst.mean_hurst, rolling_hurst.is_valid),
            );
            println!(
                "  │  {:<22} {:>4}  H={:.4}",
                "Permutation entropy",
                verdict_permen(permen.normalized_entropy).as_str(),
                permen.normalized_entropy,
            );
            println!(
                "  │  {:<22} {:>4}  p={:.4}",
                "Anderson-Darling",
                verdict_anderson_darling(anderson_darling.p_value).as_str(),
                anderson_darling.p_value,
            );
            chaos_extended_results.insert(
                name.clone(),
                serde_json::json!({
                    "sample_entropy": sampen,
                    "approximate_entropy": apen,
                    "dfa": dfa,
                    "rqa": rqa,
                    "bootstrap_hurst": bootstrap_hurst,
                    "rolling_hurst": rolling_hurst,
                    "permutation_entropy": permen,
                    "anderson_darling": anderson_darling,
                }),
            );
        }
        println!("  └─");
    }

    if resolved.synchrony {
        println!();
        if all_data.len() < 2 {
            println!("  ┌─ Synchrony Analysis");
            println!("  │  N/A — requires at least 2 analyzed sources");
            println!("  └─");
        } else {
            println!("  ┌─ Synchrony Analysis");
            for i in 0..all_data.len() {
                for j in (i + 1)..all_data.len() {
                    let (name_a, data_a) = &all_data[i];
                    let (name_b, data_b) = &all_data[j];
                    let sync = openentropy_core::synchrony::synchrony_analysis(data_a, data_b);

                    println!("  │");
                    println!("  │  {} ↔ {}", name_a, name_b);
                    println!(
                        "  │  {:<22}  NMI={}",
                        "Mutual information",
                        metric_or_na(sync.mutual_info.normalized_mi, sync.mutual_info.is_valid),
                    );
                    println!(
                        "  │  {:<22}  coh={}",
                        "Sign coherence",
                        metric_or_na(
                            sync.phase_coherence.coherence,
                            sync.phase_coherence.is_valid
                        ),
                    );
                    println!(
                        "  │  {:<22}  r={} lag={}",
                        "Cross sync",
                        metric_or_na(
                            sync.cross_sync.max_cross_correlation,
                            sync.cross_sync.is_valid
                        ),
                        sync.cross_sync.lag_at_max,
                    );
                    let key = format!("{}__vs__{}", name_a, name_b);
                    synchrony_results.insert(key, serde_json::json!(sync));
                }
            }

            let stream_refs: Vec<&[u8]> =
                all_data.iter().map(|(_, data)| data.as_slice()).collect();
            let global = openentropy_core::synchrony::global_event_detection(&stream_refs);
            println!("  │");
            println!(
                "  │  {:<22}  events={} rate={}",
                "Global events",
                global.n_events,
                metric_or_na(global.event_rate, global.is_valid),
            );
            synchrony_results.insert(
                "global_event_detection".to_string(),
                serde_json::json!(global),
            );
            println!("  └─");
        }
    }

    let telemetry_report = telemetry.finish();
    if let Some(ref window) = telemetry_report {
        super::telemetry::print_window_summary("analyze", window);
    }

    // JSON output.
    if let Some(ref path) = args.output {
        let mut json = if let Some(matrix) = cross_matrix {
            serde_json::json!({
                "sources": all_results,
                "cross_correlation": matrix,
            })
        } else {
            serde_json::json!({ "sources": all_results })
        };
        if let Some(window) = telemetry_report {
            json["telemetry_v1"] = serde_json::json!(window);
        }
        if !chaos_results.is_empty() {
            let chaos_map: std::collections::HashMap<String, _> =
                chaos_results.into_iter().collect();
            json["chaos"] = serde_json::json!(chaos_map);
        }
        if !statistics_results.is_empty() {
            json["statistics"] = serde_json::json!(statistics_results);
        }
        if !temporal_results.is_empty() {
            json["temporal"] = serde_json::json!(temporal_results);
        }
        if !chaos_extended_results.is_empty() {
            json["chaos_extended"] = serde_json::json!(chaos_extended_results);
        }
        if !synchrony_results.is_empty() {
            json["synchrony"] = serde_json::json!(synchrony_results);
        }
        super::write_json(&json, path, "Results");
    }
}

/// Print forensic test results for a single source as a compact table.
fn print_source_forensics(r: &analysis::SourceAnalysis) {
    let grade = openentropy_core::grade_min_entropy(r.min_entropy.max(0.0));

    println!();
    println!(
        "  {} — H={:.3} H∞={:.3} (grade {}) — {} bytes",
        r.source_name, r.shannon_entropy, r.min_entropy, grade, r.sample_size
    );

    // 7 forensic tests, each with a verdict and key metric.
    let ac = &r.autocorrelation;
    let sp = &r.spectral;
    let bb = &r.bit_bias;
    let d = &r.distribution;
    let st = &r.stationarity;
    let ru = &r.runs;

    let tests: Vec<(&str, &str, String)> = vec![
        (
            "Autocorrelation",
            verdict_autocorr(ac.max_abs_correlation).as_str(),
            format!(
                "max|r|={:.4} at lag {}, {}/{} violations",
                ac.max_abs_correlation,
                ac.max_abs_lag,
                ac.violations,
                ac.lags.len()
            ),
        ),
        (
            "Spectral flatness",
            verdict_spectral(sp.flatness).as_str(),
            format!(
                "flatness={:.4} (1.0=white noise), dominant freq={:.4}",
                sp.flatness, sp.dominant_frequency
            ),
        ),
        (
            "Bit bias",
            verdict_bias(bb.overall_bias, bb.has_significant_bias).as_str(),
            format!(
                "overall={:.4}, bits=[{}]",
                bb.overall_bias,
                bb.bit_probabilities
                    .iter()
                    .map(|p| format!("{:.3}", p))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        ),
        (
            "Distribution",
            verdict_distribution(d.ks_p_value).as_str(),
            format!(
                "KS p={:.4}, mean={:.1}, skew={:.3}, kurt={:.3}",
                d.ks_p_value, d.mean, d.skewness, d.kurtosis
            ),
        ),
        (
            "Stationarity",
            verdict_stationarity(st.f_statistic, st.is_stationary).as_str(),
            format!("F={:.2} ({} windows)", st.f_statistic, st.n_windows),
        ),
        (
            "Runs",
            verdict_runs(ru, r.sample_size).as_str(),
            format!(
                "longest={} (expect {:.0}), total={} (expect {:.0})",
                ru.longest_run, ru.expected_longest_run, ru.total_runs, ru.expected_runs
            ),
        ),
    ];

    for (name, verdict, detail) in &tests {
        println!("    {:<20} {:>4}  {}", name, verdict, detail);
    }
    println!();
}

// ---------------------------------------------------------------------------
// NIST test battery path (--report)
// ---------------------------------------------------------------------------

fn run_report(args: &AnalyzeArgs, resolved: &Resolved) {
    let telemetry = super::telemetry::TelemetryCapture::start(args.include_telemetry);
    let mode = super::parse_conditioning(&resolved.conditioning);

    let sources = super::resolve_sources(&args.positional, args.all);

    if sources.is_empty() {
        eprintln!("No sources matched filter.");
        std::process::exit(1);
    }

    println!("NIST randomness test battery — formal pass/fail with p-values");
    println!("(For throughput/stability ranking, use `bench` instead.)\n");
    println!(
        "Testing {} source(s), {} samples each...\n",
        sources.len(),
        resolved.samples
    );

    let mut all_results = Vec::new();

    for src in &sources {
        let info = src.info();
        print!("  {}...", info.name);

        let t0 = Instant::now();
        let mut raw_data = src.collect(resolved.samples);
        if raw_data.is_empty() {
            // Retry once — USB/hardware sources may need reconnection time.
            std::thread::sleep(std::time::Duration::from_secs(1));
            raw_data = src.collect(resolved.samples);
        }
        let data = condition(&raw_data, raw_data.len(), mode);
        print!(" {} bytes", data.len());

        if data.is_empty() {
            println!(" (no data)");
            continue;
        }

        let results = openentropy_tests::run_all_tests(&data);
        let elapsed = t0.elapsed().as_secs_f64();
        let score = openentropy_tests::calculate_quality_score(&results);
        let passed = results.iter().filter(|r| r.passed).count();

        println!(
            " -> {:.0}/100 ({}/{} passed) [{:.1}s]",
            score,
            passed,
            results.len(),
            elapsed
        );

        all_results.push((info.name.to_string(), data, results));
    }

    if all_results.is_empty() {
        eprintln!("No sources produced data.");
        std::process::exit(1);
    }

    // Summary table
    println!("\n{}", "=".repeat(60));
    println!(
        "{:<25} {:>6} {:>6} {:>8}",
        "Source", "Score", "Grade", "Pass"
    );
    println!("{}", "-".repeat(60));

    let mut sorted_indices: Vec<usize> = (0..all_results.len()).collect();
    sorted_indices.sort_by(|&a, &b| {
        let sa = openentropy_tests::calculate_quality_score(&all_results[a].2);
        let sb = openentropy_tests::calculate_quality_score(&all_results[b].2);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    for &idx in &sorted_indices {
        let (ref name, _, ref results) = all_results[idx];
        let score = openentropy_tests::calculate_quality_score(results);
        let grade = if score >= 80.0 {
            'A'
        } else if score >= 60.0 {
            'B'
        } else if score >= 40.0 {
            'C'
        } else if score >= 20.0 {
            'D'
        } else {
            'F'
        };
        let passed = results.iter().filter(|r| r.passed).count();
        println!(
            "  {:<23} {:>5.1} {:>6} {:>4}/{}",
            name,
            score,
            grade,
            passed,
            results.len()
        );
    }

    let telemetry_report = telemetry.finish_and_print("analyze --report");

    // Markdown output.
    if let Some(ref path) = args.output {
        let report = generate_markdown_report(&all_results, telemetry_report.as_ref());
        if let Err(e) = std::fs::write(path, &report) {
            eprintln!("Failed to write report to {path}: {e}");
        } else {
            println!("\nReport saved to: {path}");
        }
    }
}

fn generate_markdown_report(
    results: &[(String, Vec<u8>, Vec<openentropy_tests::TestResult>)],
    telemetry: Option<&openentropy_core::TelemetryWindowReport>,
) -> String {
    let mut report = String::new();
    report.push_str("# OpenEntropy — NIST Randomness Test Report\n\n");
    report.push_str(&format!(
        "Generated: Unix timestamp: {}\n\n",
        super::unix_timestamp_now()
    ));
    if let Some(t) = telemetry {
        report.push_str("## Telemetry Context (`telemetry_v1`)\n\n");
        report.push_str(&format!(
            "- Elapsed: {:.2}s\n- Host: {}/{}\n- CPU count: {}\n- Metrics observed: {}\n\n",
            t.elapsed_ms as f64 / 1000.0,
            t.end.os,
            t.end.arch,
            t.end.cpu_count,
            t.end.metrics.len()
        ));
    }

    for (name, data, tests) in results {
        let score = openentropy_tests::calculate_quality_score(tests);
        let passed = tests.iter().filter(|r| r.passed).count();
        report.push_str(&format!("## {name}\n\n"));
        report.push_str(&format!(
            "- Samples: {} bytes\n- Score: {:.1}/100\n- Passed: {}/{}\n\n",
            data.len(),
            score,
            passed,
            tests.len()
        ));

        report.push_str("| Test | P | Grade | p-value | Statistic | Details |\n");
        report.push_str("|------|---|-------|---------|-----------|--------|\n");
        for t in tests {
            let ok = if t.passed { "Y" } else { "N" };
            let pval = t
                .p_value
                .map(|p| format!("{p:.6}"))
                .unwrap_or_else(|| "—".to_string());
            report.push_str(&format!(
                "| {} | {} | {} | {} | {:.4} | {} |\n",
                t.name, ok, t.grade, pval, t.statistic, t.details
            ));
        }
        report.push_str("\n---\n\n");
    }

    report
}
