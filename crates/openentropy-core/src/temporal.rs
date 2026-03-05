//! # Temporal Analysis
//!
//! Temporal methods for identifying shifts and localized anomalies in byte
//! streams collected from entropy sources.

use serde::Serialize;

use crate::math;

#[derive(Debug, Clone, Serialize)]
pub struct ChangePoint {
    pub offset: usize,
    pub mean_before: f64,
    pub mean_after: f64,
    pub magnitude: f64,
    pub significance: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChangePointResult {
    pub change_points: Vec<ChangePoint>,
    pub n_segments: usize,
    pub is_valid: bool,
}

pub fn change_point_detection(data: &[u8], min_segment: usize) -> ChangePointResult {
    if min_segment == 0 || data.len() < 2 * min_segment {
        return ChangePointResult {
            change_points: Vec::new(),
            n_segments: 0,
            is_valid: false,
        };
    }

    let values: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    let n = values.len();
    let global_mean = values.iter().sum::<f64>() / n as f64;
    let global_std = stddev(&values, global_mean);

    if global_std < 1e-12 {
        return ChangePointResult {
            change_points: Vec::new(),
            n_segments: 1,
            is_valid: true,
        };
    }

    let x: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let (slope, intercept, _) = math::linear_regression(&x, &values);
    let detrended: Vec<f64> = if slope.is_nan() || intercept.is_nan() {
        values.clone()
    } else {
        values
            .iter()
            .enumerate()
            .map(|(i, &v)| v - (slope * i as f64 + intercept))
            .collect()
    };
    let detrended_mean = detrended.iter().sum::<f64>() / n as f64;

    let mut cusum = vec![0.0_f64; n + 1];
    for i in 0..n {
        cusum[i + 1] = cusum[i] + (detrended[i] - detrended_mean);
    }

    let mut best_offset = min_segment;
    let mut best_score = f64::NEG_INFINITY;
    #[allow(clippy::needless_range_loop)]
    for k in min_segment..=(n - min_segment) {
        let score = cusum[k].abs();
        if score > best_score {
            best_score = score;
            best_offset = k;
        }
    }

    let mean_before = values[..best_offset].iter().sum::<f64>() / best_offset as f64;
    let right_len = n - best_offset;
    let mean_after = values[best_offset..].iter().sum::<f64>() / right_len as f64;
    let magnitude = (mean_after - mean_before).abs();
    let significance = magnitude / global_std;

    let change_points = if significance >= 1.5 {
        vec![ChangePoint {
            offset: best_offset,
            mean_before,
            mean_after,
            magnitude,
            significance,
        }]
    } else {
        Vec::new()
    };

    ChangePointResult {
        n_segments: change_points.len() + 1,
        change_points,
        is_valid: true,
    }
}

pub fn change_point_detection_default(data: &[u8]) -> ChangePointResult {
    change_point_detection(data, 100)
}

#[derive(Debug, Clone, Serialize)]
pub struct Anomaly {
    pub offset: usize,
    pub window_mean: f64,
    pub window_entropy: f64,
    pub z_score: f64,
    pub anomaly_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnomalyDetectionResult {
    pub anomalies: Vec<Anomaly>,
    pub total_windows: usize,
    pub anomaly_rate: f64,
    pub is_valid: bool,
}

pub fn anomaly_detection(
    data: &[u8],
    window_size: usize,
    z_threshold: f64,
) -> AnomalyDetectionResult {
    if window_size == 0 || data.len() < window_size {
        return AnomalyDetectionResult {
            anomalies: Vec::new(),
            total_windows: 0,
            anomaly_rate: 0.0,
            is_valid: false,
        };
    }

    let total_windows = data.len() / window_size;
    if total_windows == 0 {
        return AnomalyDetectionResult {
            anomalies: Vec::new(),
            total_windows: 0,
            anomaly_rate: 0.0,
            is_valid: false,
        };
    }

    let mut means = Vec::with_capacity(total_windows);
    let mut entropies = Vec::with_capacity(total_windows);
    for w in 0..total_windows {
        let start = w * window_size;
        let end = start + window_size;
        let window = &data[start..end];
        means.push(window.iter().map(|&b| b as f64).sum::<f64>() / window_size as f64);
        entropies.push(shannon_entropy(window));
    }

    let global_mean = means.iter().sum::<f64>() / total_windows as f64;
    let global_std = stddev(&means, global_mean);
    if global_std < 1e-12 {
        return AnomalyDetectionResult {
            anomalies: Vec::new(),
            total_windows,
            anomaly_rate: 0.0,
            is_valid: true,
        };
    }

    let mut anomalies = Vec::new();
    for i in 0..total_windows {
        let z_score = (means[i] - global_mean).abs() / global_std;
        if z_score > z_threshold {
            anomalies.push(Anomaly {
                offset: i * window_size,
                window_mean: means[i],
                window_entropy: entropies[i],
                z_score,
                anomaly_type: if means[i] > global_mean {
                    "high_mean".to_string()
                } else {
                    "low_mean".to_string()
                },
            });
        }
    }

    let anomaly_rate = anomalies.len() as f64 / total_windows as f64;
    AnomalyDetectionResult {
        anomalies,
        total_windows,
        anomaly_rate,
        is_valid: true,
    }
}

pub fn anomaly_detection_default(data: &[u8]) -> AnomalyDetectionResult {
    anomaly_detection(data, 256, 3.0)
}

#[derive(Debug, Clone, Serialize)]
pub struct Burst {
    pub start: usize,
    pub end: usize,
    pub length: usize,
    pub mean_value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BurstResult {
    pub bursts: Vec<Burst>,
    pub n_bursts: usize,
    pub max_burst_length: usize,
    pub mean_burst_length: f64,
    pub total_burst_fraction: f64,
    pub is_valid: bool,
}

pub fn burst_detection(data: &[u8], threshold_percentile: f64) -> BurstResult {
    if data.is_empty() {
        return BurstResult {
            bursts: Vec::new(),
            n_bursts: 0,
            max_burst_length: 0,
            mean_burst_length: 0.0,
            total_burst_fraction: 0.0,
            is_valid: false,
        };
    }

    let mut sorted = data.to_vec();
    sorted.sort_unstable();
    let p = threshold_percentile.clamp(0.0, 100.0);
    let idx = ((p / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    let threshold = sorted[idx];

    let mut bursts = Vec::new();
    let mut i = 0;
    while i < data.len() {
        if data[i] < threshold {
            i += 1;
            continue;
        }
        let start = i;
        let mut sum = 0.0;
        while i < data.len() && data[i] >= threshold {
            sum += data[i] as f64;
            i += 1;
        }
        let end = i - 1;
        let length = end - start + 1;
        bursts.push(Burst {
            start,
            end,
            length,
            mean_value: sum / length as f64,
        });
    }

    let n_bursts = bursts.len();
    let total_burst_len: usize = bursts.iter().map(|b| b.length).sum();
    let max_burst_length = bursts.iter().map(|b| b.length).max().unwrap_or(0);
    let mean_burst_length = if n_bursts == 0 {
        0.0
    } else {
        total_burst_len as f64 / n_bursts as f64
    };

    BurstResult {
        bursts,
        n_bursts,
        max_burst_length,
        mean_burst_length,
        total_burst_fraction: total_burst_len as f64 / data.len() as f64,
        is_valid: true,
    }
}

pub fn burst_detection_default(data: &[u8]) -> BurstResult {
    burst_detection(data, 95.0)
}

#[derive(Debug, Clone, Serialize)]
pub struct Shift {
    pub offset: usize,
    pub mean_before: f64,
    pub mean_after: f64,
    pub delta: f64,
    pub z_score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShiftResult {
    pub shifts: Vec<Shift>,
    pub n_shifts: usize,
    pub is_valid: bool,
}

pub fn shift_detection(data: &[u8], window_size: usize, threshold_sigma: f64) -> ShiftResult {
    if window_size == 0 || data.len() < 2 * window_size {
        return ShiftResult {
            shifts: Vec::new(),
            n_shifts: 0,
            is_valid: false,
        };
    }

    let n_windows = data.len() / window_size;
    if n_windows < 2 {
        return ShiftResult {
            shifts: Vec::new(),
            n_shifts: 0,
            is_valid: false,
        };
    }

    let mut means = Vec::with_capacity(n_windows);
    for i in 0..n_windows {
        let start = i * window_size;
        let end = start + window_size;
        let mean = data[start..end].iter().map(|&b| b as f64).sum::<f64>() / window_size as f64;
        means.push(mean);
    }

    let global_mean = means.iter().sum::<f64>() / means.len() as f64;
    let global_std = stddev(&means, global_mean);
    if global_std < 1e-12 {
        return ShiftResult {
            shifts: Vec::new(),
            n_shifts: 0,
            is_valid: true,
        };
    }

    let mut shifts = Vec::new();

    // Compute differences between consecutive window means
    let mut diffs = Vec::new();
    for i in 0..(means.len() - 1) {
        diffs.push((means[i + 1] - means[i]).abs());
    }

    // Compute std of differences to normalize z-scores
    let diff_mean = diffs.iter().sum::<f64>() / diffs.len() as f64;
    let diff_std = stddev(&diffs, diff_mean);

    for i in 0..(means.len() - 1) {
        let delta = (means[i + 1] - means[i]).abs();
        let z_score = if diff_std > 1e-12 {
            delta / diff_std
        } else {
            0.0
        };
        if z_score > threshold_sigma {
            shifts.push(Shift {
                offset: (i + 1) * window_size,
                mean_before: means[i],
                mean_after: means[i + 1],
                delta,
                z_score,
            });
        }
    }

    ShiftResult {
        n_shifts: shifts.len(),
        shifts,
        is_valid: true,
    }
}

pub fn shift_detection_default(data: &[u8]) -> ShiftResult {
    shift_detection(data, 500, 3.0)
}

#[derive(Debug, Clone, Serialize)]
pub struct DriftSegment {
    pub index: usize,
    pub mean: f64,
    pub variance: f64,
    pub entropy: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DriftResult {
    pub segments: Vec<DriftSegment>,
    pub drift_slope: f64,
    pub drift_r_squared: f64,
    pub is_drifting: bool,
    pub is_valid: bool,
}

pub fn temporal_drift(data: &[u8], n_segments: usize) -> DriftResult {
    if n_segments < 2 || data.len() < n_segments {
        return DriftResult {
            segments: Vec::new(),
            drift_slope: 0.0,
            drift_r_squared: 0.0,
            is_drifting: false,
            is_valid: false,
        };
    }

    let mut segments = Vec::with_capacity(n_segments);
    let mut means = Vec::with_capacity(n_segments);
    let mut x = Vec::with_capacity(n_segments);

    for i in 0..n_segments {
        let start = i * data.len() / n_segments;
        let end = (i + 1) * data.len() / n_segments;
        let segment = &data[start..end];
        let (mean, variance) = mean_variance(segment);
        let entropy = shannon_entropy(segment);

        segments.push(DriftSegment {
            index: i,
            mean,
            variance,
            entropy,
        });
        means.push(mean);
        x.push(i as f64);
    }

    let (drift_slope, intercept, drift_r_squared) = math::linear_regression(&x, &means);
    let residual_sum_sq = means
        .iter()
        .zip(x.iter())
        .map(|(y, xi)| {
            let y_hat = drift_slope * *xi + intercept;
            (y - y_hat).powi(2)
        })
        .sum::<f64>();
    let residual_variance = residual_sum_sq / n_segments as f64;
    let stderr = (residual_variance / n_segments as f64).sqrt();
    let is_drifting = drift_slope.is_finite() && drift_slope.abs() > 2.0 * stderr;

    DriftResult {
        segments,
        drift_slope,
        drift_r_squared,
        is_drifting,
        is_valid: true,
    }
}

pub fn temporal_drift_default(data: &[u8]) -> DriftResult {
    temporal_drift(data, 10)
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionStats {
    pub index: usize,
    pub mean: f64,
    pub variance: f64,
    pub entropy: f64,
    pub sample_size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct StabilityResult {
    pub session_stats: Vec<SessionStats>,
    pub cv_mean: f64,
    pub cv_variance: f64,
    pub cv_entropy: f64,
    pub is_stable: bool,
    pub is_valid: bool,
}

pub fn inter_session_stability(sessions: &[&[u8]]) -> StabilityResult {
    if sessions.len() < 2 {
        return StabilityResult {
            session_stats: Vec::new(),
            cv_mean: 0.0,
            cv_variance: 0.0,
            cv_entropy: 0.0,
            is_stable: false,
            is_valid: false,
        };
    }

    let mut session_stats = Vec::new();
    for (index, session) in sessions.iter().enumerate() {
        if session.is_empty() {
            continue;
        }
        let (mean, variance) = mean_variance(session);
        session_stats.push(SessionStats {
            index,
            mean,
            variance,
            entropy: shannon_entropy(session),
            sample_size: session.len(),
        });
    }

    if session_stats.len() < 2 {
        return StabilityResult {
            session_stats,
            cv_mean: 0.0,
            cv_variance: 0.0,
            cv_entropy: 0.0,
            is_stable: false,
            is_valid: false,
        };
    }

    let means: Vec<f64> = session_stats.iter().map(|s| s.mean).collect();
    let variances: Vec<f64> = session_stats.iter().map(|s| s.variance).collect();
    let entropies: Vec<f64> = session_stats.iter().map(|s| s.entropy).collect();

    let cv_mean = coefficient_of_variation(&means);
    let cv_variance = coefficient_of_variation(&variances);
    let cv_entropy = coefficient_of_variation(&entropies);

    StabilityResult {
        session_stats,
        cv_mean,
        cv_variance,
        cv_entropy,
        is_stable: cv_mean < 0.1 && cv_variance < 0.1 && cv_entropy < 0.1,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TemporalAnalysisSuite {
    pub change_points: ChangePointResult,
    pub anomalies: AnomalyDetectionResult,
    pub bursts: BurstResult,
    pub shifts: ShiftResult,
    pub drift: DriftResult,
}

pub fn temporal_analysis_suite(data: &[u8]) -> TemporalAnalysisSuite {
    TemporalAnalysisSuite {
        change_points: change_point_detection_default(data),
        anomalies: anomaly_detection_default(data),
        bursts: burst_detection_default(data),
        shifts: shift_detection_default(data),
        drift: temporal_drift_default(data),
    }
}

fn mean_variance(values: &[u8]) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let n = values.len() as f64;
    let mean = values.iter().map(|&b| b as f64).sum::<f64>() / n;
    let variance = values
        .iter()
        .map(|&b| {
            let d = b as f64 - mean;
            d * d
        })
        .sum::<f64>()
        / n;
    (mean, variance)
}

fn coefficient_of_variation(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let sd = stddev(values, mean);
    if mean.abs() < 1e-12 {
        if sd < 1e-12 { 0.0 } else { f64::INFINITY }
    } else {
        sd / mean.abs()
    }
}

fn stddev(values: &[f64], mean: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    let variance = values
        .iter()
        .map(|v| {
            let d = v - mean;
            d * d
        })
        .sum::<f64>()
        / values.len() as f64;
    variance.sqrt()
}

fn shannon_entropy(window: &[u8]) -> f64 {
    if window.is_empty() {
        return 0.0;
    }

    let mut counts = [0_usize; 256];
    for &b in window {
        counts[b as usize] += 1;
    }

    let n = window.len() as f64;
    let mut entropy = 0.0;
    for count in counts {
        if count == 0 {
            continue;
        }
        let p = count as f64 / n;
        entropy -= p * p.log2();
    }
    entropy
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_data_seeded(len: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        let mut data = Vec::with_capacity(len);
        for _ in 0..len {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            data.push((state >> 33) as u8);
        }
        data
    }

    #[test]
    fn change_point_detects_clear_shift() {
        let mut data = vec![0_u8; 5000];
        data.extend(vec![255_u8; 5000]);

        let result = change_point_detection(&data, 100);
        assert!(result.is_valid);
        let cp = result
            .change_points
            .iter()
            .min_by_key(|cp| cp.offset.abs_diff(5000));
        assert!(cp.is_some());
        let cp = cp.expect("expected a change point near midpoint");
        assert!(cp.offset.abs_diff(5000) <= 200);
    }

    #[test]
    fn change_point_random_data_has_few_significant_changes() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = change_point_detection(&data, 100);
        assert!(result.is_valid);
        assert!(result.change_points.len() <= 2);
    }

    #[test]
    fn change_point_constant_data_has_no_change_points() {
        let data = vec![42_u8; 10000];
        let result = change_point_detection(&data, 100);
        assert!(result.is_valid);
        assert_eq!(result.change_points.len(), 0);
    }

    #[test]
    fn change_point_too_short_is_invalid() {
        let data = random_data_seeded(50, 0x1234);
        let result = change_point_detection(&data, 100);
        assert!(!result.is_valid);
    }

    #[test]
    fn anomaly_detection_random_data_has_low_rate() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = anomaly_detection(&data, 256, 3.0);
        assert!(result.is_valid);
        assert!(result.anomaly_rate < 0.05);
    }

    #[test]
    fn anomaly_detection_detects_injected_spike_window() {
        let mut data = vec![128_u8; 5000];
        let spike_start = 256 * 5;
        for b in data.iter_mut().skip(spike_start).take(256) {
            *b = 255;
        }

        let result = anomaly_detection(&data, 256, 3.0);
        assert!(result.is_valid);
        assert!(result.anomalies.iter().any(|a| a.offset == spike_start));
    }

    #[test]
    fn anomaly_detection_too_short_is_invalid() {
        let data = random_data_seeded(100, 0x9abc);
        let result = anomaly_detection(&data, 256, 3.0);
        assert!(!result.is_valid);
    }

    #[test]
    fn anomaly_detection_empty_input_is_invalid_no_panic() {
        let result = anomaly_detection(&[], 256, 3.0);
        assert!(!result.is_valid);
        assert_eq!(result.anomalies.len(), 0);
    }

    #[test]
    fn burst_detection_random_data_has_few_short_bursts() {
        let data = random_data_seeded(10_000, 0x1234_5678);
        let result = burst_detection(&data, 95.0);
        assert!(result.is_valid);
        assert!(result.n_bursts < 700);
        assert!(result.mean_burst_length < 3.0);
    }

    #[test]
    fn burst_detection_detects_injected_high_run() {
        let mut data = vec![128_u8; 10_000];
        let start = 4_200;
        let end = 4_300;
        for b in data.iter_mut().take(end).skip(start) {
            *b = 255;
        }

        let result = burst_detection(&data, 99.0);
        assert!(result.is_valid);
        assert!(
            result
                .bursts
                .iter()
                .any(|b| b.start <= start && b.end >= end - 1)
        );
    }

    #[test]
    fn burst_detection_empty_is_invalid() {
        let result = burst_detection(&[], 95.0);
        assert!(!result.is_valid);
    }

    #[test]
    fn burst_detection_all_255_is_single_burst() {
        let data = vec![255_u8; 5000];
        let result = burst_detection(&data, 95.0);
        assert!(result.is_valid);
        assert_eq!(result.n_bursts, 1);
        assert_eq!(result.max_burst_length, data.len());
    }

    #[test]
    fn shift_detection_detects_clear_mean_shift() {
        let mut data = vec![0_u8; 5000];
        data.extend(vec![128_u8; 5000]);
        let result = shift_detection(&data, 500, 3.0);

        assert!(result.is_valid);
        assert!(result.shifts.iter().any(|s| s.offset.abs_diff(5000) <= 500));
    }

    #[test]
    fn shift_detection_random_data_has_few_shifts() {
        let data = random_data_seeded(10_000, 0xa5a5_a5a5);
        let result = shift_detection(&data, 500, 3.0);
        assert!(result.is_valid);
        assert!(result.n_shifts <= 2);
    }

    #[test]
    fn shift_detection_short_data_is_invalid() {
        let data = random_data_seeded(900, 0x9999);
        let result = shift_detection(&data, 500, 3.0);
        assert!(!result.is_valid);
    }

    #[test]
    fn shift_detection_empty_is_invalid() {
        let result = shift_detection(&[], 500, 3.0);
        assert!(!result.is_valid);
    }

    #[test]
    fn temporal_drift_increasing_mean_detected() {
        let mut data = Vec::with_capacity(10_000);
        for segment in 0..10 {
            let base = 20 + segment * 20;
            data.extend(std::iter::repeat_n(base as u8, 1000));
        }

        let result = temporal_drift(&data, 10);
        assert!(result.is_valid);
        assert!(result.is_drifting);
        assert!(result.drift_slope > 0.0);
    }

    #[test]
    fn temporal_drift_stationary_random_not_drifting() {
        let data = random_data_seeded(10_000, 0x1111_2222);
        let result = temporal_drift(&data, 10);
        assert!(result.is_valid);
        assert!(!result.is_drifting);
    }

    #[test]
    fn temporal_drift_short_data_is_invalid() {
        let data = random_data_seeded(8, 0x7777);
        let result = temporal_drift(&data, 10);
        assert!(!result.is_valid);
    }

    #[test]
    fn temporal_drift_empty_is_invalid() {
        let result = temporal_drift(&[], 10);
        assert!(!result.is_valid);
    }

    #[test]
    fn inter_session_stability_similar_sessions_are_stable() {
        let s1 = random_data_seeded(5000, 0x1001);
        let s2 = random_data_seeded(5000, 0x1002);
        let s3 = random_data_seeded(5000, 0x1003);
        let s4 = random_data_seeded(5000, 0x1004);
        let s5 = random_data_seeded(5000, 0x1005);
        let sessions: Vec<&[u8]> = vec![&s1, &s2, &s3, &s4, &s5];

        let result = inter_session_stability(&sessions);
        assert!(result.is_valid);
        assert!(result.is_stable);
    }

    #[test]
    fn inter_session_stability_biased_session_is_unstable() {
        let s1 = random_data_seeded(5000, 0x2001);
        let s2 = random_data_seeded(5000, 0x2002);
        let s3 = random_data_seeded(5000, 0x2003);
        let s4 = random_data_seeded(5000, 0x2004);
        let s5 = vec![255_u8; 5000];
        let sessions: Vec<&[u8]> = vec![&s1, &s2, &s3, &s4, &s5];

        let result = inter_session_stability(&sessions);
        assert!(result.is_valid);
        assert!(!result.is_stable);
    }

    #[test]
    fn inter_session_stability_too_few_sessions_is_invalid() {
        let s1 = random_data_seeded(5000, 0x3001);
        let sessions: Vec<&[u8]> = vec![&s1];
        let result = inter_session_stability(&sessions);
        assert!(!result.is_valid);
    }

    #[test]
    fn inter_session_stability_empty_sessions_is_invalid() {
        let s1 = Vec::<u8>::new();
        let s2 = Vec::<u8>::new();
        let sessions: Vec<&[u8]> = vec![&s1, &s2];
        let result = inter_session_stability(&sessions);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_temporal_analysis_suite_serializes() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = temporal_analysis_suite(&data);
        let json = serde_json::to_string(&result).expect("serialization failed");
        assert!(json.contains("change_points"));
        assert!(json.contains("anomalies"));
    }
}
