//! # Synchrony Analysis
//!
//! Multi-source coordination methods for byte-stream synchrony and event detection.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MutualInfoResult {
    pub mutual_information: f64,
    pub normalized_mi: f64,
    pub entropy_a: f64,
    pub entropy_b: f64,
    pub joint_entropy: f64,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhaseCoherenceResult {
    pub coherence: f64,
    pub mean_phase_diff: f64,
    pub phase_std: f64,
    pub is_coherent: bool,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CrossSyncResult {
    pub correlation: f64,
    pub lag_at_max: i64,
    pub max_cross_correlation: f64,
    pub is_synchronized: bool,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalEvent {
    pub offset: usize,
    pub n_streams_affected: usize,
    pub mean_deviation: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GlobalEventResult {
    pub events: Vec<GlobalEvent>,
    pub n_events: usize,
    pub event_rate: f64,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SynchronyAnalysis {
    pub mutual_info: MutualInfoResult,
    pub phase_coherence: PhaseCoherenceResult,
    pub cross_sync: CrossSyncResult,
}

pub fn synchrony_analysis(data_a: &[u8], data_b: &[u8]) -> SynchronyAnalysis {
    SynchronyAnalysis {
        mutual_info: mutual_information(data_a, data_b),
        phase_coherence: phase_coherence(data_a, data_b),
        cross_sync: cross_sync(data_a, data_b),
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn std_dev(values: &[f64], avg: f64) -> f64 {
    let var = values
        .iter()
        .map(|v| {
            let d = *v - avg;
            d * d
        })
        .sum::<f64>()
        / values.len() as f64;
    var.sqrt()
}

fn pearson_corr(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let a = &a[..n];
    let b = &b[..n];
    let mean_a = mean(a);
    let mean_b = mean(b);

    let mut num = 0.0;
    let mut den_a = 0.0;
    let mut den_b = 0.0;
    for i in 0..n {
        let da = a[i] - mean_a;
        let db = b[i] - mean_b;
        num += da * db;
        den_a += da * da;
        den_b += db * db;
    }

    let den = (den_a * den_b).sqrt();
    if den <= f64::EPSILON {
        0.0
    } else {
        (num / den).clamp(-1.0, 1.0)
    }
}

pub fn mutual_information(data_a: &[u8], data_b: &[u8]) -> MutualInfoResult {
    let n = data_a.len().min(data_b.len());
    if n < 2 {
        return MutualInfoResult {
            mutual_information: 0.0,
            normalized_mi: 0.0,
            entropy_a: 0.0,
            entropy_b: 0.0,
            joint_entropy: 0.0,
            is_valid: false,
        };
    }

    let mut hist_a = [0_u64; 256];
    let mut hist_b = [0_u64; 256];
    let mut joint = vec![0_u64; 256 * 256];
    for i in 0..n {
        let a = data_a[i] as usize;
        let b = data_b[i] as usize;
        hist_a[a] += 1;
        hist_b[b] += 1;
        joint[a * 256 + b] += 1;
    }

    let n_f = n as f64;
    let entropy_a = hist_a
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / n_f;
            -p * p.ln()
        })
        .sum::<f64>();

    let entropy_b = hist_b
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / n_f;
            -p * p.ln()
        })
        .sum::<f64>();

    let joint_entropy = joint
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / n_f;
            -p * p.ln()
        })
        .sum::<f64>();

    let mutual_information = (entropy_a + entropy_b - joint_entropy).max(0.0);
    let denom = (entropy_a * entropy_b).sqrt();
    let normalized_mi = if denom <= f64::EPSILON {
        0.0
    } else {
        (mutual_information / denom).clamp(0.0, 1.0)
    };

    MutualInfoResult {
        mutual_information,
        normalized_mi,
        entropy_a,
        entropy_b,
        joint_entropy,
        is_valid: true,
    }
}

pub fn phase_coherence(data_a: &[u8], data_b: &[u8]) -> PhaseCoherenceResult {
    let n = data_a.len().min(data_b.len());
    if n < 2 {
        return PhaseCoherenceResult {
            coherence: 0.0,
            mean_phase_diff: 0.0,
            phase_std: 0.0,
            is_coherent: false,
            is_valid: false,
        };
    }

    let mean_a = data_a[..n].iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    let mean_b = data_b[..n].iter().map(|&x| x as f64).sum::<f64>() / n as f64;

    let mut diffs = Vec::with_capacity(n);
    for i in 0..n {
        let sign_a = if data_a[i] as f64 > mean_a { 1.0 } else { -1.0 };
        let sign_b = if data_b[i] as f64 > mean_b { 1.0 } else { -1.0 };
        diffs.push(sign_a * sign_b);
    }

    let mean_phase_diff = mean(&diffs);
    let phase_std = std_dev(&diffs, mean_phase_diff);
    let coherence = mean_phase_diff.abs();

    PhaseCoherenceResult {
        coherence,
        mean_phase_diff,
        phase_std,
        is_coherent: coherence > 0.5,
        is_valid: true,
    }
}

pub fn cross_sync(data_a: &[u8], data_b: &[u8]) -> CrossSyncResult {
    let n = data_a.len().min(data_b.len());
    if n < 2 {
        return CrossSyncResult {
            correlation: 0.0,
            lag_at_max: 0,
            max_cross_correlation: 0.0,
            is_synchronized: false,
            is_valid: false,
        };
    }

    let step = n.div_ceil(2000);
    let mut a = Vec::new();
    let mut b = Vec::new();
    let mut i = 0;
    while i < n {
        a.push(data_a[i] as f64);
        b.push(data_b[i] as f64);
        i += step;
    }
    let m = a.len().min(b.len());
    if m < 2 {
        return CrossSyncResult {
            correlation: 0.0,
            lag_at_max: 0,
            max_cross_correlation: 0.0,
            is_synchronized: false,
            is_valid: false,
        };
    }

    let a = &a[..m];
    let b = &b[..m];
    let correlation = pearson_corr(a, b);

    let mut lag_at_max = 0_i64;
    let mut max_cross_correlation = correlation;
    let mut best_abs = correlation.abs();

    for lag in -10_i64..=10_i64 {
        let (aa, bb): (&[f64], &[f64]) = if lag < 0 {
            let l = (-lag) as usize;
            if l >= m {
                continue;
            }
            (&a[..m - l], &b[l..m])
        } else {
            let l = lag as usize;
            if l >= m {
                continue;
            }
            (&a[l..m], &b[..m - l])
        };
        if aa.len() < 2 || bb.len() < 2 {
            continue;
        }
        let c = pearson_corr(aa, bb);
        let abs_c = c.abs();
        if abs_c > best_abs {
            best_abs = abs_c;
            max_cross_correlation = c;
            lag_at_max = lag;
        }
    }

    CrossSyncResult {
        correlation,
        lag_at_max,
        max_cross_correlation,
        is_synchronized: max_cross_correlation.abs() > 0.3,
        is_valid: true,
    }
}

pub fn global_event_detection(streams: &[&[u8]]) -> GlobalEventResult {
    if streams.len() < 2 {
        return GlobalEventResult {
            events: Vec::new(),
            n_events: 0,
            event_rate: 0.0,
            is_valid: false,
        };
    }

    let min_len = streams.iter().map(|s| s.len()).min().unwrap_or(0);
    if min_len == 0 {
        return GlobalEventResult {
            events: Vec::new(),
            n_events: 0,
            event_rate: 0.0,
            is_valid: false,
        };
    }

    let window_size = 100;
    let total_windows = min_len / window_size;
    if total_windows == 0 {
        return GlobalEventResult {
            events: Vec::new(),
            n_events: 0,
            event_rate: 0.0,
            is_valid: false,
        };
    }

    let n_streams = streams.len();
    let mut stream_window_means = vec![vec![0.0_f64; total_windows]; n_streams];
    for (si, stream) in streams.iter().enumerate() {
        #[allow(clippy::needless_range_loop)]
        for w in 0..total_windows {
            let start = w * window_size;
            let end = start + window_size;
            let avg =
                stream[start..end].iter().map(|&x| x as f64).sum::<f64>() / window_size as f64;
            stream_window_means[si][w] = avg;
        }
    }

    let mut stream_global_means = vec![0.0_f64; n_streams];
    let mut stream_global_stds = vec![0.0_f64; n_streams];
    for si in 0..n_streams {
        let m = mean(&stream_window_means[si]);
        let s = std_dev(&stream_window_means[si], m);
        stream_global_means[si] = m;
        stream_global_stds[si] = s;
    }

    let mut events = Vec::new();
    #[allow(clippy::needless_range_loop)]
    for w in 0..total_windows {
        let mut affected = 0_usize;
        let mut dev_sum = 0.0_f64;
        for si in 0..n_streams {
            let s = stream_global_stds[si];
            if s <= f64::EPSILON {
                continue;
            }
            let d = (stream_window_means[si][w] - stream_global_means[si]).abs();
            if d > 2.0 * s {
                affected += 1;
                dev_sum += d;
            }
        }
        if affected * 2 >= n_streams {
            events.push(GlobalEvent {
                offset: w * window_size,
                n_streams_affected: affected,
                mean_deviation: if affected > 0 {
                    dev_sum / affected as f64
                } else {
                    0.0
                },
            });
        }
    }

    let n_events = events.len();
    GlobalEventResult {
        events,
        n_events,
        event_rate: n_events as f64 / total_windows as f64,
        is_valid: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_data_seeded(len: usize, seed: u64) -> Vec<u8> {
        let mut state = seed;
        let mut data = Vec::with_capacity(len);
        for _ in 0..len {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            data.push((state >> 33) as u8);
        }
        data
    }

    #[test]
    fn mutual_information_identical_high_nmi() {
        let data = random_data_seeded(5000, 1);
        let result = mutual_information(&data, &data);
        assert!(result.is_valid);
        assert!((result.normalized_mi - 1.0).abs() < 0.02);
    }

    #[test]
    fn mutual_information_independent_low_nmi() {
        let data_a = random_data_seeded(5000, 1);
        let data_b = random_data_seeded(5000, 2);
        let result = mutual_information(&data_a, &data_b);
        assert!(result.is_valid);
        assert!(result.normalized_mi < 0.5);
    }

    #[test]
    fn mutual_information_empty_invalid() {
        let result = mutual_information(&[], &[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn mutual_information_single_byte_invalid() {
        let result = mutual_information(&[1], &[1]);
        assert!(!result.is_valid);
    }

    #[test]
    fn phase_coherence_identical_high() {
        let data = random_data_seeded(3000, 11);
        let result = phase_coherence(&data, &data);
        assert!(result.is_valid);
        assert!((result.coherence - 1.0).abs() < 1e-9);
        assert!(result.mean_phase_diff > 0.95);
    }

    #[test]
    fn phase_coherence_independent_low() {
        let data_a = random_data_seeded(3000, 11);
        let data_b = random_data_seeded(3000, 91);
        let result = phase_coherence(&data_a, &data_b);
        assert!(result.is_valid);
        assert!(result.coherence < 0.3);
    }

    #[test]
    fn phase_coherence_empty_invalid() {
        let result = phase_coherence(&[], &[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn phase_coherence_anti_correlated_high() {
        let mut data_a = Vec::new();
        let mut data_b = Vec::new();
        for i in 0..3000 {
            if i % 2 == 0 {
                data_a.push(240);
                data_b.push(20);
            } else {
                data_a.push(20);
                data_b.push(240);
            }
        }
        let result = phase_coherence(&data_a, &data_b);
        assert!(result.is_valid);
        assert!((result.coherence - 1.0).abs() < 1e-9);
        assert!(result.mean_phase_diff < -0.95);
    }

    #[test]
    fn cross_sync_identical_high_corr() {
        let data = random_data_seeded(4000, 7);
        let result = cross_sync(&data, &data);
        assert!(result.is_valid);
        assert!((result.correlation - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cross_sync_independent_low_corr() {
        let data_a = random_data_seeded(4000, 7);
        let data_b = random_data_seeded(4000, 13);
        let result = cross_sync(&data_a, &data_b);
        assert!(result.is_valid);
        assert!(result.correlation.abs() < 0.2);
    }

    #[test]
    fn cross_sync_empty_invalid() {
        let result = cross_sync(&[], &[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn cross_sync_lagged_copy_nonzero_lag() {
        let base = random_data_seeded(5000, 88);
        let mut lagged = vec![0_u8; 5];
        lagged.extend(base.iter().copied().take(4995));
        let result = cross_sync(&base, &lagged);
        assert!(result.is_valid);
        assert_ne!(result.lag_at_max, 0);
    }

    #[test]
    fn global_event_independent_low_rate() {
        let a = random_data_seeded(3000, 31);
        let b = random_data_seeded(3000, 37);
        let c = random_data_seeded(3000, 41);
        let streams: [&[u8]; 3] = [&a, &b, &c];
        let result = global_event_detection(&streams);
        assert!(result.is_valid);
        assert!(result.event_rate < 0.2);
    }

    #[test]
    fn global_event_injected_spike_detected() {
        let mut a = random_data_seeded(3000, 31);
        let mut b = random_data_seeded(3000, 37);
        let mut c = random_data_seeded(3000, 41);
        for i in 1000..1100 {
            a[i] = 255;
            b[i] = 255;
            c[i] = 255;
        }
        let streams: [&[u8]; 3] = [&a, &b, &c];
        let result = global_event_detection(&streams);
        assert!(result.is_valid);
        assert!(result.n_events >= 1);
    }

    #[test]
    fn global_event_less_than_two_streams_invalid() {
        let a = random_data_seeded(1000, 1);
        let streams: [&[u8]; 1] = [&a];
        let result = global_event_detection(&streams);
        assert!(!result.is_valid);
    }

    #[test]
    fn global_event_empty_invalid() {
        let a: [u8; 0] = [];
        let b: [u8; 0] = [];
        let streams: [&[u8]; 2] = [&a, &b];
        let result = global_event_detection(&streams);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_synchrony_analysis_serializes() {
        let data_a = random_data_seeded(5000, 0xdeadbeef);
        let data_b = random_data_seeded(5000, 0xcafebabe);
        let result = synchrony_analysis(&data_a, &data_b);
        let json = serde_json::to_string(&result).expect("serialization failed");
        assert!(json.contains("mutual_info"));
        assert!(json.contains("cross_sync"));
    }
}
