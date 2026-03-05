//! # Chaos Theory Analysis
//!
//! Methods for applying chaos theory analysis to QRNG output.
//! These functions help distinguish genuine quantum randomness from
//! deterministic or structured behavior in sampled byte streams.

use flate2::Compression;
use flate2::write::ZlibEncoder;
use serde::Serialize;
use std::f64::consts::LN_2;

use crate::math;

#[derive(Debug, Clone, Serialize)]
pub struct HurstResult {
    pub hurst_exponent: f64,
    pub is_valid: bool,
    pub r_squared: f64,
    pub rs_values: Vec<(usize, f64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BootstrapHurstResult {
    pub observed_hurst: f64,
    pub mean_surrogate_hurst: f64,
    pub std_surrogate_hurst: f64,
    pub p_value: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    pub n_surrogates: usize,
    pub is_significant: bool,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RollingHurstWindow {
    pub offset: usize,
    pub hurst: f64,
    pub r_squared: f64,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RollingHurstResult {
    pub windows: Vec<RollingHurstWindow>,
    pub mean_hurst: f64,
    pub std_hurst: f64,
    pub min_hurst: f64,
    pub max_hurst: f64,
    pub window_size: usize,
    pub step: usize,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DfaResult {
    pub alpha: f64,
    pub r_squared: f64,
    pub fluctuations: Vec<(usize, f64)>,
    pub order: usize,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BiEntropyResult {
    pub bien: f64,
    pub tbien: f64,
    pub derivative_entropies: Vec<f64>,
    pub num_derivatives: usize,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EpiplexityResult {
    pub compression_ratio: f64,       // compressed_size / raw_size
    pub structural_info: f64,         // 1.0 - compression_ratio (epiplexity)
    pub remaining_entropy: f64,       // compression_ratio (unpredictability)
    pub delta_compression_ratio: f64, // compress(diff(data)) / diff_size
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SampleEntropyResult {
    pub sample_entropy: f64,
    pub m: usize,
    pub r: f64,
    pub count_a: u64,
    pub count_b: u64,
    pub actual_samples: usize,
    pub is_valid: bool,
}

fn sample_std_dev(data: &[f64]) -> f64 {
    let n = data.len();
    if n < 2 {
        return 0.0;
    }

    let mean = data.iter().sum::<f64>() / n as f64;
    let variance = data
        .iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / (n as f64 - 1.0);

    variance.sqrt()
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }

    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn log_spaced_windows(min: usize, max: usize, count: usize) -> Vec<usize> {
    if max <= min || count < 2 {
        return vec![min, max];
    }

    let log_min = (min as f64).ln();
    let log_max = (max as f64).ln();
    (0..count)
        .map(|i| {
            let t = i as f64 / (count - 1) as f64;
            (log_min + t * (log_max - log_min)).exp().round() as usize
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn subsample_to_limit(data: &[u8], max_samples: usize) -> Vec<f64> {
    let data_f64: Vec<f64> = data.iter().map(|&x| x as f64).collect();
    if data_f64.len() > max_samples {
        let step = (data_f64.len() / max_samples).max(1);
        data_f64
            .iter()
            .step_by(step)
            .take(max_samples)
            .copied()
            .collect()
    } else {
        data_f64
    }
}

pub fn sample_entropy(data: &[u8], m: usize, r: f64) -> SampleEntropyResult {
    const MAX_SAMPLES: usize = 5000;

    if m == 0 || data.len() <= m + 1 {
        return SampleEntropyResult {
            sample_entropy: f64::NAN,
            m,
            r,
            count_a: 0,
            count_b: 0,
            actual_samples: data.len().min(MAX_SAMPLES),
            is_valid: false,
        };
    }

    let sampled = subsample_to_limit(data, MAX_SAMPLES);
    if sampled.len() <= m + 1 {
        return SampleEntropyResult {
            sample_entropy: f64::NAN,
            m,
            r,
            count_a: 0,
            count_b: 0,
            actual_samples: sampled.len(),
            is_valid: false,
        };
    }

    let std_dev = sample_std_dev(&sampled);
    if std_dev == 0.0 || !r.is_finite() || r <= 0.0 {
        return SampleEntropyResult {
            sample_entropy: f64::NAN,
            m,
            r,
            count_a: 0,
            count_b: 0,
            actual_samples: sampled.len(),
            is_valid: false,
        };
    }

    let n = sampled.len();
    let n_m = n - m;
    let n_m1 = n - m - 1;
    let mut count_b: u64 = 0;
    let mut count_a: u64 = 0;

    for i in 0..n_m {
        for j in 0..n_m {
            if i == j {
                continue;
            }

            let mut matches_m = true;
            for k in 0..m {
                if (sampled[i + k] - sampled[j + k]).abs() >= r {
                    matches_m = false;
                    break;
                }
            }

            if !matches_m {
                continue;
            }

            count_b += 1;

            if i < n_m1 && j < n_m1 && (sampled[i + m] - sampled[j + m]).abs() < r {
                count_a += 1;
            }
        }
    }

    if count_b == 0 || count_a == 0 {
        return SampleEntropyResult {
            sample_entropy: f64::NAN,
            m,
            r,
            count_a,
            count_b,
            actual_samples: sampled.len(),
            is_valid: false,
        };
    }

    let sampen = -((count_a as f64) / (count_b as f64)).ln();
    SampleEntropyResult {
        sample_entropy: sampen,
        m,
        r,
        count_a,
        count_b,
        actual_samples: sampled.len(),
        is_valid: sampen.is_finite(),
    }
}

pub fn sample_entropy_default(data: &[u8]) -> SampleEntropyResult {
    const DEFAULT_M: usize = 2;
    const MAX_SAMPLES: usize = 5000;

    if data.len() <= DEFAULT_M + 1 {
        return SampleEntropyResult {
            sample_entropy: f64::NAN,
            m: DEFAULT_M,
            r: f64::NAN,
            count_a: 0,
            count_b: 0,
            actual_samples: data.len().min(MAX_SAMPLES),
            is_valid: false,
        };
    }

    let sampled = subsample_to_limit(data, MAX_SAMPLES);
    let std_dev = sample_std_dev(&sampled);
    if std_dev == 0.0 {
        return SampleEntropyResult {
            sample_entropy: f64::NAN,
            m: DEFAULT_M,
            r: 0.0,
            count_a: 0,
            count_b: 0,
            actual_samples: sampled.len(),
            is_valid: false,
        };
    }

    sample_entropy(data, DEFAULT_M, 0.2 * std_dev)
}

#[allow(dead_code)]
fn shannon_entropy_binary(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut ones = 0usize;
    let total_bits = data.len() * 8;
    for &byte in data {
        ones += byte.count_ones() as usize;
    }

    let p1 = ones as f64 / total_bits as f64;
    let p0 = 1.0 - p1;

    let h0 = if p0 <= 0.0 {
        0.0
    } else {
        -p0 * (p0.ln() / LN_2)
    };
    let h1 = if p1 <= 0.0 {
        0.0
    } else {
        -p1 * (p1.ln() / LN_2)
    };
    h0 + h1
}

pub fn hurst_exponent(data: &[u8]) -> HurstResult {
    if data.len() < 100 {
        return HurstResult {
            hurst_exponent: f64::NAN,
            is_valid: false,
            r_squared: 0.0,
            rs_values: vec![],
        };
    }

    let n = data.len();
    let min_window = 10usize;
    let max_window = n / 4;
    if max_window <= min_window {
        return HurstResult {
            hurst_exponent: f64::NAN,
            is_valid: false,
            r_squared: 0.0,
            rs_values: vec![],
        };
    }

    let step = ((max_window - min_window) / 20).max(1);
    let mut window_sizes = Vec::new();
    let mut rs_means = Vec::new();
    let mut rs_values = Vec::new();

    for window_size in (min_window..max_window).step_by(step) {
        let mut rs_for_size = Vec::new();

        for start in (0..=(n - window_size)).step_by(window_size) {
            let segment = &data[start..start + window_size];
            let segment_f64: Vec<f64> = segment.iter().map(|&x| x as f64).collect();

            let mean = segment_f64.iter().sum::<f64>() / window_size as f64;

            let mut cumulative = 0.0;
            let mut min_cum = 0.0;
            let mut max_cum = 0.0;
            for &value in &segment_f64 {
                cumulative += value - mean;
                if cumulative < min_cum {
                    min_cum = cumulative;
                }
                if cumulative > max_cum {
                    max_cum = cumulative;
                }
            }

            let range = max_cum - min_cum;
            let std_dev = sample_std_dev(&segment_f64);

            if std_dev > 1e-10 {
                rs_for_size.push(range / std_dev);
            }
        }

        if !rs_for_size.is_empty() {
            let mean_rs = rs_for_size.iter().sum::<f64>() / rs_for_size.len() as f64;
            window_sizes.push(window_size as f64);
            rs_means.push(mean_rs);
            rs_values.push((window_size, mean_rs));
        }
    }

    if rs_values.len() < 5 {
        return HurstResult {
            hurst_exponent: f64::NAN,
            is_valid: false,
            r_squared: 0.0,
            rs_values,
        };
    }

    let log_n: Vec<f64> = window_sizes.iter().map(|&v| v.ln()).collect();
    let log_rs: Vec<f64> = rs_means.iter().map(|&v| v.ln()).collect();
    let (slope, _intercept, r_squared) = math::linear_regression(&log_n, &log_rs);

    HurstResult {
        hurst_exponent: slope,
        is_valid: slope.is_finite(),
        r_squared,
        rs_values,
    }
}

pub fn rolling_hurst(data: &[u8], window_size: usize, step: usize) -> RollingHurstResult {
    let invalid = || RollingHurstResult {
        windows: vec![],
        mean_hurst: f64::NAN,
        std_hurst: f64::NAN,
        min_hurst: f64::NAN,
        max_hurst: f64::NAN,
        window_size,
        step,
        is_valid: false,
    };

    if window_size == 0 || step == 0 || data.len() < window_size {
        return invalid();
    }

    let mut windows = Vec::new();
    let mut offset = 0usize;
    while offset + window_size <= data.len() {
        let result = hurst_exponent(&data[offset..offset + window_size]);
        windows.push(RollingHurstWindow {
            offset,
            hurst: result.hurst_exponent,
            r_squared: result.r_squared,
            is_valid: result.is_valid,
        });
        offset += step;
    }

    let valid_hursts: Vec<f64> = windows
        .iter()
        .filter_map(|w| {
            if w.is_valid && w.hurst.is_finite() {
                Some(w.hurst)
            } else {
                None
            }
        })
        .collect();

    if valid_hursts.is_empty() {
        return RollingHurstResult {
            windows,
            mean_hurst: f64::NAN,
            std_hurst: f64::NAN,
            min_hurst: f64::NAN,
            max_hurst: f64::NAN,
            window_size,
            step,
            is_valid: false,
        };
    }

    let mean_hurst = valid_hursts.iter().sum::<f64>() / valid_hursts.len() as f64;
    let std_hurst = sample_std_dev(&valid_hursts);
    let min_hurst = valid_hursts.iter().copied().fold(f64::INFINITY, f64::min);
    let max_hurst = valid_hursts
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    RollingHurstResult {
        windows,
        mean_hurst,
        std_hurst,
        min_hurst,
        max_hurst,
        window_size,
        step,
        is_valid: true,
    }
}

pub fn rolling_hurst_default(data: &[u8]) -> RollingHurstResult {
    rolling_hurst(data, 512, 64)
}

pub fn bootstrap_hurst(data: &[u8], n_bootstrap: usize) -> BootstrapHurstResult {
    let invalid = || BootstrapHurstResult {
        observed_hurst: f64::NAN,
        mean_surrogate_hurst: f64::NAN,
        std_surrogate_hurst: f64::NAN,
        p_value: f64::NAN,
        ci_lower: f64::NAN,
        ci_upper: f64::NAN,
        n_surrogates: 0,
        is_significant: false,
        is_valid: false,
    };

    let observed = hurst_exponent(data);
    if !observed.is_valid || n_bootstrap == 0 {
        return invalid();
    }

    let mut state = data
        .iter()
        .fold(0u64, |acc, &b| acc.wrapping_mul(31).wrapping_add(b as u64));

    let mut surrogate_hursts = Vec::with_capacity(n_bootstrap);
    for _ in 0..n_bootstrap {
        let mut surrogate = data.to_vec();
        for i in (1..surrogate.len()).rev() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = (state as usize) % (i + 1);
            surrogate.swap(i, j);
        }

        let result = hurst_exponent(&surrogate);
        if result.is_valid {
            surrogate_hursts.push(result.hurst_exponent);
        }
    }

    if surrogate_hursts.is_empty() {
        return invalid();
    }

    let mean_surrogate_hurst = surrogate_hursts.iter().sum::<f64>() / surrogate_hursts.len() as f64;
    let std_surrogate_hurst = sample_std_dev(&surrogate_hursts);

    let mut sorted = surrogate_hursts.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let ci_lower = percentile(&sorted, 2.5);
    let ci_upper = percentile(&sorted, 97.5);

    let ge_count = surrogate_hursts
        .iter()
        .filter(|&&h| h >= observed.hurst_exponent)
        .count();
    let p_value = ge_count as f64 / surrogate_hursts.len() as f64;

    BootstrapHurstResult {
        observed_hurst: observed.hurst_exponent,
        mean_surrogate_hurst,
        std_surrogate_hurst,
        p_value,
        ci_lower,
        ci_upper,
        n_surrogates: surrogate_hursts.len(),
        is_significant: observed.hurst_exponent < ci_lower || observed.hurst_exponent > ci_upper,
        is_valid: true,
    }
}

pub fn bootstrap_hurst_default(data: &[u8]) -> BootstrapHurstResult {
    bootstrap_hurst(data, 200)
}

pub fn dfa(data: &[u8], order: usize) -> DfaResult {
    if data.len() < 40 || order != 1 {
        return DfaResult {
            alpha: f64::NAN,
            r_squared: 0.0,
            fluctuations: vec![],
            order,
            is_valid: false,
        };
    }

    let data_f64: Vec<f64> = data.iter().map(|&x| x as f64).collect();
    let mean = data_f64.iter().sum::<f64>() / data_f64.len() as f64;

    let mut integrated = Vec::with_capacity(data_f64.len());
    let mut cumulative = 0.0;
    for &v in &data_f64 {
        cumulative += v - mean;
        integrated.push(cumulative);
    }

    let min_window = 10usize;
    let max_window = integrated.len() / 4;
    if max_window <= min_window {
        return DfaResult {
            alpha: f64::NAN,
            r_squared: 0.0,
            fluctuations: vec![],
            order,
            is_valid: false,
        };
    }

    let window_sizes = log_spaced_windows(min_window, max_window, 20);
    let mut fluctuations = Vec::new();
    let mut log_n = Vec::new();
    let mut log_f = Vec::new();

    for &window_size in &window_sizes {
        let n_windows = integrated.len() / window_size;
        if n_windows < 2 {
            continue;
        }

        let t: Vec<f64> = (0..window_size).map(|i| i as f64).collect();
        let mut rms_values = Vec::with_capacity(n_windows);

        for w in 0..n_windows {
            let start = w * window_size;
            let end = start + window_size;
            let segment = &integrated[start..end];

            let (slope, intercept, _r2) = math::linear_regression(&t, segment);
            if !slope.is_finite() || !intercept.is_finite() {
                continue;
            }

            let mse = segment
                .iter()
                .enumerate()
                .map(|(i, &y)| {
                    let trend = slope * i as f64 + intercept;
                    let d = y - trend;
                    d * d
                })
                .sum::<f64>()
                / window_size as f64;

            let rms = mse.sqrt();
            if rms.is_finite() {
                rms_values.push(rms);
            }
        }

        if !rms_values.is_empty() {
            let f_n = rms_values.iter().sum::<f64>() / rms_values.len() as f64;
            if f_n > 1e-12 {
                fluctuations.push((window_size, f_n));
                log_n.push((window_size as f64).ln());
                log_f.push(f_n.ln());
            }
        }
    }

    if fluctuations.len() < 4 {
        return DfaResult {
            alpha: f64::NAN,
            r_squared: 0.0,
            fluctuations,
            order,
            is_valid: false,
        };
    }

    let (alpha, _intercept, r_squared) = math::linear_regression(&log_n, &log_f);
    let is_valid = alpha.is_finite();

    DfaResult {
        alpha,
        r_squared,
        fluctuations,
        order,
        is_valid,
    }
}

pub fn dfa_default(data: &[u8]) -> DfaResult {
    dfa(data, 1)
}

fn bytes_to_bits(data: &[u8]) -> Vec<bool> {
    let mut bits = Vec::with_capacity(data.len() * 8);
    for &byte in data {
        for bit_index in 0..8 {
            bits.push(((byte >> bit_index) & 1) == 1);
        }
    }
    bits
}

fn binary_derivative(bits: &[bool]) -> Vec<bool> {
    if bits.len() < 2 {
        return vec![];
    }

    let mut derivative = Vec::with_capacity(bits.len() - 1);
    for i in 0..(bits.len() - 1) {
        derivative.push(bits[i] ^ bits[i + 1]);
    }
    derivative
}

fn shannon_entropy_bits(bits: &[bool]) -> f64 {
    if bits.is_empty() {
        return 0.0;
    }

    let ones = bits.iter().filter(|&&bit| bit).count();
    let p = ones as f64 / bits.len() as f64;
    if p <= 0.0 || p >= 1.0 {
        return 0.0;
    }

    let q = 1.0 - p;
    -p * (p.ln() / LN_2) - q * (q.ln() / LN_2)
}

pub fn bientropy(data: &[u8]) -> BiEntropyResult {
    if data.len() < 2 {
        return BiEntropyResult {
            bien: 0.0,
            tbien: 0.0,
            derivative_entropies: vec![],
            num_derivatives: 0,
            is_valid: false,
        };
    }

    let mut bien_sum = 0.0;
    let mut tbien_sum = 0.0;
    let mut chunk_count = 0usize;
    let mut derivative_sums: Vec<f64> = vec![];
    let mut derivative_counts: Vec<usize> = vec![];

    for chunk in data.chunks(32) {
        let bits = bytes_to_bits(chunk);
        if bits.len() < 2 {
            continue;
        }

        let max_k = 20usize.min(bits.len() - 1);
        let mut current = bits;
        let mut entropies = Vec::with_capacity(max_k + 1);

        for k in 0..=max_k {
            let h = shannon_entropy_bits(&current);
            entropies.push(h);

            if derivative_sums.len() <= k {
                derivative_sums.push(0.0);
                derivative_counts.push(0);
            }
            derivative_sums[k] += h;
            derivative_counts[k] += 1;

            if k < max_k {
                current = binary_derivative(&current);
            }
        }

        let mut bien_weighted = 0.0;
        let mut tbien_weighted = 0.0;
        let mut tbien_weight_total = 0.0;
        for (k, &h) in entropies.iter().enumerate() {
            let bien_weight = 2f64.powi(k as i32);
            let tbien_weight = (k as f64 + 2.0).log2();
            bien_weighted += bien_weight * h;
            tbien_weighted += tbien_weight * h;
            tbien_weight_total += tbien_weight;
        }

        let bien_normalizer = 2f64.powi((max_k + 1) as i32) - 1.0;
        let chunk_bien = if bien_normalizer > 0.0 {
            bien_weighted / bien_normalizer
        } else {
            0.0
        };
        let chunk_tbien = if tbien_weight_total > 0.0 {
            tbien_weighted / tbien_weight_total
        } else {
            0.0
        };

        bien_sum += chunk_bien;
        tbien_sum += chunk_tbien;
        chunk_count += 1;
    }

    if chunk_count == 0 {
        return BiEntropyResult {
            bien: 0.0,
            tbien: 0.0,
            derivative_entropies: vec![],
            num_derivatives: 0,
            is_valid: false,
        };
    }

    let derivative_entropies: Vec<f64> = derivative_sums
        .iter()
        .zip(derivative_counts.iter())
        .map(|(&sum, &count)| if count > 0 { sum / count as f64 } else { 0.0 })
        .collect();

    BiEntropyResult {
        bien: bien_sum / chunk_count as f64,
        tbien: tbien_sum / chunk_count as f64,
        num_derivatives: derivative_entropies.len(),
        derivative_entropies,
        is_valid: true,
    }
}

fn compress_bytes(data: &[u8]) -> usize {
    use std::io::Write;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    let _ = encoder.write_all(data);
    encoder.finish().map(|v| v.len()).unwrap_or(data.len())
}

pub fn epiplexity(data: &[u8]) -> EpiplexityResult {
    // Guard: insufficient data
    if data.len() < 20 {
        return EpiplexityResult {
            compression_ratio: 0.0,
            structural_info: 0.0,
            remaining_entropy: 0.0,
            delta_compression_ratio: 0.0,
            is_valid: false,
        };
    }

    // Compression ratio of raw data
    let compressed_size = compress_bytes(data) as f64;
    let raw_size = data.len() as f64;
    let compression_ratio = compressed_size / raw_size;

    // Structural information (epiplexity) = 1 - compression_ratio
    let structural_info = 1.0 - compression_ratio;

    // Remaining entropy = compression_ratio (unpredictability)
    let remaining_entropy = compression_ratio;

    // Delta compression: differences between adjacent bytes
    let delta: Vec<u8> = data.windows(2).map(|w| w[1].wrapping_sub(w[0])).collect();

    let delta_compressed_size = compress_bytes(&delta) as f64;
    let delta_size = delta.len() as f64;
    let delta_compression_ratio = if delta_size > 0.0 {
        delta_compressed_size / delta_size
    } else {
        0.0
    };

    EpiplexityResult {
        compression_ratio,
        structural_info,
        remaining_entropy,
        delta_compression_ratio,
        is_valid: true,
    }
}

fn time_delay_embedding(data: &[f64], m: usize, tau: usize) -> Vec<Vec<f64>> {
    let n = data.len();
    if n < m * tau {
        return vec![];
    }
    let num_vectors = n - (m - 1) * tau;
    (0..num_vectors)
        .map(|i| (0..m).map(|j| data[i + j * tau]).collect())
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct LyapunovResult {
    pub lyapunov_exponent: f64,
    pub divergence_curve: Vec<f64>,
    pub embedding_dim: usize,
    pub delay: usize,
    pub r_squared: f64,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorrelationDimResult {
    pub dimension: f64,
    pub r_squared: f64,
    pub embedding_dim_used: usize,
    pub num_points_used: usize,
    pub is_valid: bool,
}

pub fn lyapunov_exponent(data: &[u8]) -> LyapunovResult {
    const EMBEDDING_DIM: usize = 3;
    const TAU: usize = 1;
    const MAX_ITER: usize = 50;
    const THEILER_WINDOW: usize = 3;

    let invalid = || LyapunovResult {
        lyapunov_exponent: f64::NAN,
        divergence_curve: vec![],
        embedding_dim: EMBEDDING_DIM,
        delay: TAU,
        r_squared: 0.0,
        is_valid: false,
    };

    if data.len() < 500 {
        return invalid();
    }

    let data_f64: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    let sampled: Vec<f64> = if data_f64.len() > 5000 {
        let step = (data_f64.len() / 5000).max(1);
        data_f64.iter().step_by(step).take(5000).copied().collect()
    } else {
        data_f64
    };

    let embedded = time_delay_embedding(&sampled, EMBEDDING_DIM, TAU);
    if embedded.len() < 100 {
        return invalid();
    }

    let reference_indices: Vec<usize> = if embedded.len() > 200 {
        let step = (embedded.len() / 200).max(1);
        (0..embedded.len()).step_by(step).take(200).collect()
    } else {
        (0..embedded.len()).collect()
    };

    let mut divergence_sums = vec![0.0; MAX_ITER];
    let mut divergence_counts = vec![0usize; MAX_ITER];

    for &i in &reference_indices {
        let mut nearest_neighbor = None;
        let mut min_distance = f64::INFINITY;

        for j in 0..embedded.len() {
            if i.abs_diff(j) <= THEILER_WINDOW {
                continue;
            }
            let d = math::euclidean_distance(&embedded[i], &embedded[j]);
            if d <= 1e-10 {
                continue;
            }
            if d < min_distance {
                min_distance = d;
                nearest_neighbor = Some(j);
            }
        }

        let Some(j) = nearest_neighbor else {
            continue;
        };

        for step in 0..MAX_ITER {
            if i + step >= embedded.len() || j + step >= embedded.len() {
                break;
            }
            let d = math::euclidean_distance(&embedded[i + step], &embedded[j + step]);
            if d > 1e-10 {
                divergence_sums[step] += d.ln();
                divergence_counts[step] += 1;
            }
        }
    }

    let mut steps = Vec::new();
    let mut mean_log_divergence = Vec::new();
    for step in 0..MAX_ITER {
        if divergence_counts[step] >= 5 {
            steps.push(step as f64);
            mean_log_divergence.push(divergence_sums[step] / divergence_counts[step] as f64);
        }
    }

    if steps.len() < 5 {
        return invalid();
    }

    let fit_len = steps.len().min(15);
    let (slope, _intercept, r_squared) =
        math::linear_regression(&steps[..fit_len], &mean_log_divergence[..fit_len]);
    if !slope.is_finite() {
        return invalid();
    }

    LyapunovResult {
        lyapunov_exponent: slope,
        divergence_curve: mean_log_divergence,
        embedding_dim: EMBEDDING_DIM,
        delay: TAU,
        r_squared,
        is_valid: true,
    }
}

pub fn correlation_dimension(data: &[u8]) -> CorrelationDimResult {
    const MAX_EMBEDDING: usize = 8;
    const SUBSAMPLE_MAX: usize = 500;
    const NUM_RADII: usize = 15;

    if data.len() < 1000 {
        return CorrelationDimResult {
            dimension: f64::NAN,
            r_squared: 0.0,
            embedding_dim_used: 0,
            num_points_used: 0,
            is_valid: false,
        };
    }

    let data_f64: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    let subsampled: Vec<f64> = if data_f64.len() > SUBSAMPLE_MAX {
        let step = (data_f64.len() / SUBSAMPLE_MAX).max(1);
        data_f64
            .iter()
            .step_by(step)
            .take(SUBSAMPLE_MAX)
            .copied()
            .collect()
    } else {
        data_f64
    };

    let mut accepted = Vec::new();

    for m in 2..=MAX_EMBEDDING {
        let embedded = time_delay_embedding(&subsampled, m, 1);
        if embedded.len() < 10 {
            continue;
        }

        let n = embedded.len();
        let total_pairs = n * (n - 1) / 2;
        if total_pairs == 0 {
            continue;
        }

        let mut distances = Vec::with_capacity(total_pairs);
        for i in 0..n {
            for j in (i + 1)..n {
                distances.push(math::euclidean_distance(&embedded[i], &embedded[j]));
            }
        }

        distances.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if distances.len() < 100 {
            continue;
        }

        let r_min = distances[distances.len() / 100];
        let r_max = distances[distances.len() / 2];
        if r_min <= 0.0 || r_max <= r_min {
            continue;
        }

        let mut log_r = Vec::with_capacity(NUM_RADII);
        let mut log_c = Vec::with_capacity(NUM_RADII);
        let ratio = r_max / r_min;

        for k in 0..NUM_RADII {
            let r = r_min * ratio.powf(k as f64 / (NUM_RADII - 1) as f64);
            let count = distances.partition_point(|&d| d < r);
            let c = count as f64 / total_pairs as f64;
            if c > 0.0 {
                log_r.push(r.ln());
                log_c.push(c.ln());
            }
        }

        if log_r.len() < 2 {
            continue;
        }

        let (slope, _intercept, r_squared) = math::linear_regression(&log_r, &log_c);
        if r_squared > 0.9 && slope.is_finite() && slope > 0.0 {
            accepted.push((m, slope, r_squared));
        }
    }

    if accepted.is_empty() {
        return CorrelationDimResult {
            dimension: f64::NAN,
            r_squared: 0.0,
            embedding_dim_used: 0,
            num_points_used: subsampled.len(),
            is_valid: false,
        };
    }

    let take_n = accepted.len().min(3);
    let avg_d2 = accepted[accepted.len() - take_n..]
        .iter()
        .map(|(_, slope, _)| *slope)
        .sum::<f64>()
        / take_n as f64;

    let (last_m, _last_slope, last_r2) = accepted[accepted.len() - 1];

    CorrelationDimResult {
        dimension: avg_d2,
        r_squared: last_r2,
        embedding_dim_used: last_m,
        num_points_used: subsampled.len(),
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RqaResult {
    pub recurrence_rate: f64,
    pub determinism: f64,
    pub laminarity: f64,
    pub avg_diagonal_length: f64,
    pub max_diagonal_length: usize,
    pub embedding_dim: usize,
    pub delay: usize,
    pub threshold: f64,
    pub num_points_used: usize,
    pub is_valid: bool,
}

pub fn rqa(data: &[u8], embedding_dim: usize, delay: usize, threshold: f64) -> RqaResult {
    const SUBSAMPLE_MAX: usize = 1000;

    let invalid = || RqaResult {
        recurrence_rate: 0.0,
        determinism: 0.0,
        laminarity: 0.0,
        avg_diagonal_length: 0.0,
        max_diagonal_length: 0,
        embedding_dim,
        delay,
        threshold,
        num_points_used: 0,
        is_valid: false,
    };

    if data.is_empty()
        || embedding_dim == 0
        || delay == 0
        || !threshold.is_finite()
        || threshold <= 0.0
    {
        return invalid();
    }

    let data_f64: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    let embedded = time_delay_embedding(&data_f64, embedding_dim, delay);
    if embedded.len() < 2 {
        return invalid();
    }

    let vectors: Vec<Vec<f64>> = if embedded.len() > SUBSAMPLE_MAX {
        let step = (embedded.len() / SUBSAMPLE_MAX).max(1);
        embedded
            .iter()
            .step_by(step)
            .take(SUBSAMPLE_MAX)
            .cloned()
            .collect()
    } else {
        embedded
    };

    let n = vectors.len();
    if n < 2 {
        return invalid();
    }

    let mut total_recurrence_points = 0usize;
    let mut diag_points_in_lines = 0usize;
    let mut diag_line_count = 0usize;
    let mut max_diagonal_length = 0usize;
    let mut vertical_points_in_lines = 0usize;

    let mut prev_diag_lengths = vec![0usize; n + 1];
    let mut curr_diag_lengths = vec![0usize; n + 1];
    let mut vertical_lengths = vec![0usize; n];

    for i in 0..n {
        curr_diag_lengths[0] = 0;

        for j in 0..n {
            let recurrence =
                i != j && math::euclidean_distance(&vectors[i], &vectors[j]) < threshold;

            if recurrence {
                total_recurrence_points += 1;

                let diag_len = prev_diag_lengths[j] + 1;
                curr_diag_lengths[j + 1] = diag_len;
                if diag_len == 2 {
                    diag_points_in_lines += 2;
                    diag_line_count += 1;
                    max_diagonal_length = max_diagonal_length.max(2);
                } else if diag_len > 2 {
                    diag_points_in_lines += 1;
                    max_diagonal_length = max_diagonal_length.max(diag_len);
                }

                let vert_len = vertical_lengths[j] + 1;
                vertical_lengths[j] = vert_len;
                if vert_len == 2 {
                    vertical_points_in_lines += 2;
                } else if vert_len > 2 {
                    vertical_points_in_lines += 1;
                }
            } else {
                curr_diag_lengths[j + 1] = 0;
                vertical_lengths[j] = 0;
            }
        }

        std::mem::swap(&mut prev_diag_lengths, &mut curr_diag_lengths);
    }

    let total_pairs = n * (n - 1);
    let recurrence_rate = if total_pairs > 0 {
        total_recurrence_points as f64 / total_pairs as f64
    } else {
        0.0
    };

    let determinism = if total_recurrence_points > 0 {
        diag_points_in_lines as f64 / total_recurrence_points as f64
    } else {
        0.0
    };

    let laminarity = if total_recurrence_points > 0 {
        vertical_points_in_lines as f64 / total_recurrence_points as f64
    } else {
        0.0
    };

    let avg_diagonal_length = if diag_line_count > 0 {
        diag_points_in_lines as f64 / diag_line_count as f64
    } else {
        0.0
    };

    RqaResult {
        recurrence_rate,
        determinism,
        laminarity,
        avg_diagonal_length,
        max_diagonal_length,
        embedding_dim,
        delay,
        threshold,
        num_points_used: n,
        is_valid: true,
    }
}

pub fn rqa_default(data: &[u8]) -> RqaResult {
    rqa(data, 3, 1, 10.0)
}

/// Aggregated chaos theory analysis results for a data stream.
#[derive(Debug, Clone, Serialize)]
pub struct ChaosAnalysis {
    pub hurst: HurstResult,
    pub lyapunov: LyapunovResult,
    pub correlation_dimension: CorrelationDimResult,
    pub bientropy: BiEntropyResult,
    pub epiplexity: EpiplexityResult,
    pub sample_entropy: SampleEntropyResult,
    pub dfa: DfaResult,
    pub rqa: RqaResult,
    pub bootstrap_hurst: BootstrapHurstResult,
    pub rolling_hurst: RollingHurstResult,
}

/// Run all 5 chaos theory analysis methods on the given data.
///
/// This is a separate entry point from `full_analysis()` - chaos methods are
/// computationally expensive (O(n^2)) and should only be run when explicitly requested.
pub fn chaos_analysis(data: &[u8]) -> ChaosAnalysis {
    ChaosAnalysis {
        hurst: hurst_exponent(data),
        lyapunov: lyapunov_exponent(data),
        correlation_dimension: correlation_dimension(data),
        bientropy: bientropy(data),
        epiplexity: epiplexity(data),
        sample_entropy: sample_entropy_default(data),
        dfa: dfa_default(data),
        rqa: rqa_default(data),
        bootstrap_hurst: bootstrap_hurst_default(data),
        rolling_hurst: rolling_hurst_default(data),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_data_seeded(n: usize, seed: u64) -> Vec<u8> {
        let mut data = Vec::with_capacity(n);
        let mut state: u64 = seed;
        for _ in 0..n {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            data.push((state >> 33) as u8);
        }
        data
    }

    #[test]
    fn test_hurst_random() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = hurst_exponent(&data);

        assert!(result.is_valid);
        assert!(result.hurst_exponent >= 0.35 && result.hurst_exponent <= 0.65);
    }

    #[test]
    fn test_hurst_constant() {
        let data = vec![42u8; 1000];
        let result = hurst_exponent(&data);

        assert!(!result.is_valid);
    }

    #[test]
    fn test_rolling_hurst_random_seeded() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = rolling_hurst(&data, 1000, 100);

        assert!(result.is_valid);
        assert_eq!(result.windows.len(), 91);
        for window in &result.windows {
            assert!(window.is_valid);
            assert!(window.hurst >= 0.3 && window.hurst <= 0.7);
        }
    }

    #[test]
    fn test_rolling_hurst_data_too_short() {
        let data = random_data_seeded(500, 0x12345678);
        let result = rolling_hurst(&data, 1000, 100);

        assert!(!result.is_valid);
        assert!(result.windows.is_empty());
    }

    #[test]
    fn test_rolling_hurst_empty_invalid() {
        let data: &[u8] = &[];
        let result = rolling_hurst_default(data);

        assert!(!result.is_valid);
        assert!(result.windows.is_empty());
    }

    #[test]
    fn test_rolling_hurst_mean_consistency() {
        let data = random_data_seeded(10000, 0xcafebabe);
        let result = rolling_hurst_default(&data);
        assert!(result.is_valid);

        let valid_hursts: Vec<f64> = result
            .windows
            .iter()
            .filter(|w| w.is_valid)
            .map(|w| w.hurst)
            .collect();

        let expected_mean = valid_hursts.iter().sum::<f64>() / valid_hursts.len() as f64;
        assert!((result.mean_hurst - expected_mean).abs() < 1e-12);
    }

    #[test]
    fn test_bootstrap_hurst_random_deterministic() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result_a = bootstrap_hurst(&data, 200);
        let result_b = bootstrap_hurst(&data, 200);

        assert!(result_a.is_valid);
        assert_eq!(result_a.n_surrogates, 200);
        assert_eq!(result_a.p_value, result_b.p_value);
        assert_eq!(result_a.mean_surrogate_hurst, result_b.mean_surrogate_hurst);
        assert_eq!(result_a.ci_lower, result_b.ci_lower);
        assert_eq!(result_a.ci_upper, result_b.ci_upper);
    }

    #[test]
    fn test_bootstrap_hurst_correlated_significant() {
        let data: Vec<u8> = (0..5000).map(|i| (i / 20) as u8).collect();
        let result = bootstrap_hurst(&data, 200);

        assert!(result.is_valid);
        assert!(result.is_significant);
    }

    #[test]
    fn test_bootstrap_hurst_empty_invalid() {
        let result = bootstrap_hurst_default(&[]);

        assert!(!result.is_valid);
        assert_eq!(result.n_surrogates, 0);
    }

    #[test]
    fn test_bootstrap_hurst_too_short_invalid() {
        let data = random_data_seeded(50, 0x12345678);
        let result = bootstrap_hurst_default(&data);

        assert!(!result.is_valid);
        assert_eq!(result.n_surrogates, 0);
    }

    #[test]
    fn test_dfa_random() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = dfa(&data, 1);

        assert!(result.is_valid);
        assert!(result.alpha >= 0.3 && result.alpha <= 0.7);
    }

    #[test]
    fn test_dfa_constant() {
        let data = vec![42u8; 10000];
        let result = dfa_default(&data);

        assert!(!result.is_valid);
    }

    #[test]
    fn test_dfa_empty() {
        let data: &[u8] = &[];
        let result = dfa_default(data);

        assert!(!result.is_valid);
        assert!(result.fluctuations.is_empty());
    }

    #[test]
    fn test_dfa_too_short() {
        let data = random_data_seeded(20, 0x12345678);
        let result = dfa_default(&data);

        assert!(!result.is_valid);
    }

    #[test]
    fn test_bientropy_random() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = bientropy(&data);

        assert!(result.is_valid);
        assert!(result.bien > 0.90);
        assert!(result.tbien > 0.90);
    }

    #[test]
    fn test_bientropy_constant() {
        let data = vec![0u8; 1000];
        let result = bientropy(&data);

        assert!(result.is_valid);
        assert!(result.bien < 0.05);
    }

    #[test]
    fn test_bientropy_alternating() {
        let data = vec![0xAAu8; 1000];
        let result = bientropy(&data);

        assert!(result.is_valid);
        assert!(result.bien < 0.50);
    }

    #[test]
    fn test_epiplexity_random() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = epiplexity(&data);

        assert!(result.is_valid);
        assert!(
            result.compression_ratio > 0.95,
            "Random data should have high compression ratio (low compressibility)"
        );
    }

    #[test]
    fn test_epiplexity_patterned() {
        // Repeating [0, 1, 2, 3] pattern — highly compressible
        let data: Vec<u8> = (0..10000u32).map(|i| (i % 4) as u8).collect();
        let result = epiplexity(&data);

        assert!(result.is_valid);
        assert!(
            result.compression_ratio < 0.10,
            "Patterned data should have low compression ratio (high compressibility)"
        );
    }

    #[test]
    fn test_lyapunov_random() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = lyapunov_exponent(&data);

        assert!(result.is_valid);
        assert!(result.lyapunov_exponent.abs() < 0.3);
    }

    #[test]
    fn test_lyapunov_logistic() {
        let mut x = 0.1f64;
        let mut data = Vec::with_capacity(5000);
        for _ in 0..5000 {
            x = 4.0 * x * (1.0 - x);
            data.push((x * 255.0) as u8);
        }

        let result = lyapunov_exponent(&data);
        assert!(result.is_valid);
        assert!(
            result.lyapunov_exponent > 0.3,
            "expected logistic Lyapunov > 0.3, got {}",
            result.lyapunov_exponent
        );
    }

    #[test]
    fn test_lyapunov_short() {
        let data = random_data_seeded(100, 0x12345678);
        let result = lyapunov_exponent(&data);

        assert!(!result.is_valid);
    }

    #[test]
    fn test_corrdim_random() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = correlation_dimension(&data);
        assert!(
            result.is_valid,
            "correlation_dimension should be valid for 5000 random bytes"
        );
        assert!(
            result.dimension > 2.0,
            "random data should have D2 > 2.0, got {}",
            result.dimension
        );
    }

    #[test]
    fn test_corrdim_short() {
        let data = random_data_seeded(500, 0x12345678);
        let result = correlation_dimension(&data);
        assert!(
            !result.is_valid,
            "correlation_dimension should be invalid for 500 bytes (below 1000 minimum)"
        );
    }

    #[test]
    fn test_rqa_random_low_recurrence() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = rqa_default(&data);

        assert!(result.is_valid);
        assert!(
            result.recurrence_rate < 0.1,
            "random data should have low RR, got {}",
            result.recurrence_rate
        );
    }

    #[test]
    fn test_rqa_periodic_high_recurrence_and_determinism() {
        let data: Vec<u8> = (0..5000).map(|i| (i % 10) as u8).collect();
        let result = rqa_default(&data);

        assert!(result.is_valid);
        assert!(
            result.recurrence_rate > 0.2,
            "periodic data should have high RR, got {}",
            result.recurrence_rate
        );
        assert!(
            result.determinism > 0.8,
            "periodic data should have high DET, got {}",
            result.determinism
        );
    }

    #[test]
    fn test_rqa_constant_near_full_recurrence() {
        let data = vec![42u8; 5000];
        let result = rqa_default(&data);

        assert!(result.is_valid);
        assert!(
            result.recurrence_rate > 0.99,
            "constant data should have RR near 1.0, got {}",
            result.recurrence_rate
        );
    }

    #[test]
    fn test_rqa_empty_invalid() {
        let data: &[u8] = &[];
        let result = rqa_default(data);

        assert!(!result.is_valid);
    }

    #[test]
    fn test_sample_entropy_random() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = sample_entropy_default(&data);
        assert!(result.is_valid);
        assert!(result.sample_entropy > 0.0);
    }

    #[test]
    fn test_sample_entropy_periodic_lower_than_random() {
        let random = random_data_seeded(5000, 0xdeadbeef);
        let periodic: Vec<u8> = (0..5000).map(|i| (i % 4) as u8).collect();

        let random_result = sample_entropy_default(&random);
        let periodic_result = sample_entropy_default(&periodic);

        assert!(random_result.is_valid);
        assert!(periodic_result.is_valid);
        assert!(periodic_result.sample_entropy < random_result.sample_entropy);
    }

    #[test]
    fn test_sample_entropy_constant_invalid() {
        let data = vec![42u8; 5000];
        let result = sample_entropy_default(&data);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_sample_entropy_empty_invalid() {
        let data: &[u8] = &[];
        let result = sample_entropy_default(data);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_sample_entropy_single_byte_invalid() {
        let data: &[u8] = &[42];
        let result = sample_entropy_default(data);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_empty_input() {
        let data: &[u8] = &[];
        let h = hurst_exponent(data);
        let l = lyapunov_exponent(data);
        let c = correlation_dimension(data);
        let b = bientropy(data);
        let e = epiplexity(data);
        assert!(!h.is_valid);
        assert!(!l.is_valid);
        assert!(!c.is_valid);
        assert!(!b.is_valid);
        assert!(!e.is_valid);
    }

    #[test]
    fn test_single_byte() {
        let data: &[u8] = &[42];
        let h = hurst_exponent(data);
        let l = lyapunov_exponent(data);
        let c = correlation_dimension(data);
        let e = epiplexity(data);
        assert!(!h.is_valid);
        assert!(!l.is_valid);
        assert!(!c.is_valid);
        assert!(!e.is_valid);
    }

    #[test]
    fn test_chaos_analysis_orchestrator() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = chaos_analysis(&data);
        assert!(result.hurst.is_valid);
        assert!(result.lyapunov.is_valid);
        assert!(result.correlation_dimension.is_valid);
        assert!(result.bientropy.is_valid);
        assert!(result.epiplexity.is_valid);
        assert!(result.sample_entropy.is_valid);
        assert!(result.dfa.is_valid);
        assert!(result.rqa.is_valid);
        assert!(result.bootstrap_hurst.is_valid);
        assert!(result.rolling_hurst.is_valid);
    }

    #[test]
    fn test_chaos_analysis_serializes() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = chaos_analysis(&data);
        let json = serde_json::to_string(&result).expect("serialization failed");
        assert!(json.contains("sample_entropy"));
        assert!(json.contains("dfa"));
    }

    #[test]
    fn test_performance_100k() {
        let data = random_data_seeded(100_000, 0xCAFEBABE);
        let _result = chaos_analysis(&data);
    }
}
