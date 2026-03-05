//! # openentropy-core
//!
//! **Your computer is a hardware noise observatory.**
//!
//! `openentropy-core` is the core entropy harvesting library that extracts randomness
//! from 60+ unconventional hardware sources — clock jitter, DRAM row buffer timing,
//! CPU speculative execution, Bluetooth RSSI, NVMe latency, and more.
//!
//! ## Quick Start
//!
//! ```no_run
//! use openentropy_core::EntropyPool;
//!
//! // Auto-detect all available sources and create a pool
//! let pool = EntropyPool::auto();
//!
//! // Get conditioned random bytes
//! let random_bytes = pool.get_random_bytes(256);
//! assert_eq!(random_bytes.len(), 256);
//!
//! // Check pool health
//! let health = pool.health_report();
//! println!("{}/{} sources healthy", health.healthy, health.total);
//! ```
//!
//! ## Architecture
//!
//! Sources → Pool (concatenate) → Conditioning → Output
//!
//! Three output modes:
//! - **Sha256** (default): SHA-256 conditioning mixes all source bytes with state,
//!   counter, timestamp, and OS entropy. Cryptographically strong output.
//! - **VonNeumann**: debiases raw bytes without destroying noise structure.
//! - **Raw** (`get_raw_bytes`): source bytes pass through unchanged — no hashing,
//!   no whitening, no mixing between sources.
//!
//! Raw mode preserves the actual hardware noise signal for researchers studying
//! device entropy characteristics. Most QRNG APIs (ANU, Outshift) run DRBG
//! post-processing that destroys the raw hardware signal. We don't.
//!
//! Every source implements the [`EntropySource`] trait. The [`EntropyPool`]
//! collects from all registered sources and concatenates their byte streams.

pub mod analysis;
pub mod benchmark;
pub mod chaos;
pub mod comparison;
pub mod conditioning;
pub mod dispatcher;
pub(crate) mod math;
pub mod platform;
pub mod pool;
pub mod session;
pub mod source;
pub mod sources;
pub mod statistics;
pub mod telemetry;
pub mod temporal;
pub mod trials;
pub mod verdict;

pub use analysis::{
    AndersonDarlingResult, ApproxEntropyResult, AutocorrResult, BitBiasResult, CrossCorrMatrix,
    CrossCorrPair, DistributionResult, PermutationEntropyResult, RunsResult, SourceAnalysis,
    SpectralResult, StationarityResult, anderson_darling, approximate_entropy,
    approximate_entropy_default, autocorrelation_profile, bit_bias, cross_correlation_matrix,
    distribution_stats, full_analysis, pearson_correlation, permutation_entropy,
    permutation_entropy_default, runs_analysis, spectral_analysis, stationarity_test,
};
pub use benchmark::{
    BenchConfig, BenchReport, BenchSourceReport, PoolQualityReport, RankBy, benchmark_sources,
};
pub use chaos::{
    BiEntropyResult, BootstrapHurstResult, ChaosAnalysis, CorrelationDimResult, DfaResult,
    EpiplexityResult, HurstResult, LyapunovResult, RollingHurstResult, RollingHurstWindow,
    RqaResult, SampleEntropyResult, bientropy, bootstrap_hurst, bootstrap_hurst_default,
    chaos_analysis, correlation_dimension, dfa, dfa_default, epiplexity, hurst_exponent,
    lyapunov_exponent, rolling_hurst, rolling_hurst_default, rqa, rqa_default, sample_entropy,
    sample_entropy_default,
};
pub use comparison::{
    AggregateDelta, ComparisonResult, DigramAnalysis, MarkovAnalysis, MultiLagAnalysis,
    RunLengthComparison, TemporalAnalysis, TwoSampleTests, WindowAnomaly, aggregate_delta,
    cliffs_delta, compare, compare_with_analysis, digram_analysis, markov_analysis,
    multi_lag_analysis, run_length_comparison, temporal_analysis, two_sample_tests,
};
pub use conditioning::{
    ConditioningMode, MinEntropyReport, QualityReport, condition, grade_min_entropy,
    min_entropy_estimate, quick_autocorrelation_lag1, quick_min_entropy, quick_quality,
    quick_shannon,
};
pub use dispatcher::{
    AnalysisConfig, AnalysisProfile, AnalysisReport, SourceReport, VerdictSummary, analyze,
};
pub use platform::{detect_available_sources, platform_info};
pub use pool::{EntropyPool, HealthReport, SourceHealth, SourceInfoSnapshot};
pub use session::{
    MachineInfo, SessionConfig, SessionMeta, SessionSourceAnalysis, SessionWriter,
    detect_machine_info, list_sessions, load_session_raw_data,
};
pub use source::{EntropySource, Platform, Requirement, SourceCategory, SourceInfo};
pub use statistics::{
    AnovaResult, CramerVonMisesResult, GapTestResult, KruskalWallisResult, LeveneResult,
    LjungBoxResult, MultipleCorrectionResult, PowerResult, StatisticsAnalysis, anova,
    bonferroni_correction, cramer_von_mises, gap_test, gap_test_default,
    holm_bonferroni_correction, kruskal_wallis, levene_test, ljung_box, ljung_box_default,
    power_analysis, power_analysis_default, statistics_analysis,
};
pub use telemetry::{
    MODEL_ID as TELEMETRY_MODEL_ID, MODEL_VERSION as TELEMETRY_MODEL_VERSION, TelemetryMetric,
    TelemetryMetricDelta, TelemetrySnapshot, TelemetryWindowReport, build_telemetry_window,
    collect_telemetry_snapshot, collect_telemetry_window,
};
pub use temporal::{
    Anomaly, AnomalyDetectionResult, Burst, BurstResult, ChangePoint, ChangePointResult,
    DriftResult, DriftSegment, SessionStats, Shift, ShiftResult, StabilityResult,
    TemporalAnalysisSuite, anomaly_detection, anomaly_detection_default, burst_detection,
    burst_detection_default, change_point_detection, change_point_detection_default,
    inter_session_stability, shift_detection, shift_detection_default, temporal_analysis_suite,
    temporal_drift, temporal_drift_default,
};
pub use trials::{
    CalibrationResult, StoufferResult, TrialAnalysis, TrialConfig, calibration_check,
    stouffer_combine, trial_analysis,
};
pub use verdict::{
    Verdict, metric_or_na, verdict_anderson_darling, verdict_apen, verdict_autocorr, verdict_bias,
    verdict_bientropy, verdict_compression, verdict_corrdim, verdict_cramer_von_mises, verdict_dfa,
    verdict_distribution, verdict_hurst, verdict_ljung_box, verdict_lyapunov, verdict_permen,
    verdict_rqa_det, verdict_runs, verdict_sampen, verdict_spectral, verdict_stationarity,
};
pub mod synchrony;
pub use synchrony::{
    CrossSyncResult, GlobalEvent, GlobalEventResult, MutualInfoResult, PhaseCoherenceResult,
    SynchronyAnalysis, cross_sync, global_event_detection, mutual_information, phase_coherence,
    synchrony_analysis,
};
/// Library version (from Cargo.toml).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
