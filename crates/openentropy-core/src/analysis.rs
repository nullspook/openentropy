//! Comprehensive entropy source analysis beyond NIST min-entropy.
//!
//! This module provides research-oriented metrics for characterizing entropy
//! sources: autocorrelation profiles, spectral analysis, bit bias, distribution
//! statistics, stationarity, runs analysis, and entropy scaling.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::f64::consts::PI;

use crate::math;

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Autocorrelation at a single lag.
#[derive(Debug, Clone, Serialize)]
pub struct LagCorrelation {
    pub lag: usize,
    pub correlation: f64,
}

/// Autocorrelation profile across multiple lags.
#[derive(Debug, Clone, Serialize)]
pub struct AutocorrResult {
    pub lags: Vec<LagCorrelation>,
    pub max_abs_correlation: f64,
    pub max_abs_lag: usize,
    /// 95% significance threshold (2/sqrt(n)).
    pub threshold: f64,
    /// Number of lags exceeding the threshold.
    pub violations: usize,
}

/// Single spectral bin.
#[derive(Debug, Clone, Serialize)]
pub struct SpectralBin {
    /// Normalized frequency (0.0 to 0.5).
    pub frequency: f64,
    pub power: f64,
}

/// FFT-based spectral analysis result.
#[derive(Debug, Clone, Serialize)]
pub struct SpectralResult {
    /// Top 10 spectral peaks by power.
    pub peaks: Vec<SpectralBin>,
    /// Spectral flatness (Wiener entropy): 1.0 = white noise, 0.0 = tonal.
    pub flatness: f64,
    /// Dominant frequency (normalized, 0.0–0.5).
    pub dominant_frequency: f64,
    /// Total spectral power.
    pub total_power: f64,
}

/// Per-bit-position bias analysis.
#[derive(Debug, Clone, Serialize)]
pub struct BitBiasResult {
    /// Probability of 1 for each bit position (0=LSB, 7=MSB).
    pub bit_probabilities: [f64; 8],
    /// Overall bias (deviation from 0.5).
    pub overall_bias: f64,
    /// Chi-squared statistic for uniformity.
    pub chi_squared: f64,
    /// Approximate p-value.
    pub p_value: f64,
    /// Any bit position deviating > 0.01 from 0.5.
    pub has_significant_bias: bool,
}

/// Distribution statistics.
#[derive(Debug, Clone, Serialize)]
pub struct DistributionResult {
    pub mean: f64,
    pub variance: f64,
    pub std_dev: f64,
    pub skewness: f64,
    pub kurtosis: f64,
    /// Byte value histogram (256 bins).
    pub histogram: Vec<u64>,
    /// KS-style statistic vs uniform.
    pub ks_statistic: f64,
    /// Approximate p-value for KS-style statistic (heuristic for discrete bytes).
    pub ks_p_value: f64,
}

/// Anderson-Darling goodness-of-fit test against byte-uniform distribution.
#[derive(Debug, Clone, Serialize)]
pub struct AndersonDarlingResult {
    pub statistic: f64,
    pub p_value: f64,
    pub is_uniform: bool,
    pub critical_values: Vec<(f64, f64)>,
    pub is_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApproxEntropyResult {
    pub apen: f64,
    pub m: usize,
    pub r: f64,
    pub phi_m: f64,
    pub phi_m_plus_1: f64,
    pub actual_samples: usize,
    pub is_valid: bool,
}

/// Stationarity test result.
#[derive(Debug, Clone, Serialize)]
pub struct StationarityResult {
    /// Heuristic stationarity flag based on windowed F-statistic threshold.
    pub is_stationary: bool,
    /// ANOVA-like F-statistic comparing window means (heuristic).
    pub f_statistic: f64,
    /// Per-window means.
    pub window_means: Vec<f64>,
    /// Per-window standard deviations.
    pub window_std_devs: Vec<f64>,
    /// Number of windows used.
    pub n_windows: usize,
}

/// Runs analysis result.
#[derive(Debug, Clone, Serialize)]
pub struct RunsResult {
    /// Longest consecutive run of the same byte value.
    pub longest_run: usize,
    /// Expected longest run for random data of this size.
    pub expected_longest_run: f64,
    /// Total number of runs.
    pub total_runs: usize,
    /// Expected total runs for random data.
    pub expected_runs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermutationEntropyResult {
    pub permutation_entropy: f64,
    pub normalized_entropy: f64,
    pub order: usize,
    pub delay: usize,
    pub n_patterns: usize,
    pub pattern_counts: Vec<(Vec<usize>, usize)>,
    pub is_valid: bool,
}

/// Entropy at a specific sample size.
#[derive(Debug, Clone, Serialize)]
pub struct EntropyPoint {
    pub sample_size: usize,
    pub shannon_h: f64,
    pub min_entropy: f64,
    pub collection_time_ms: u64,
}

/// Entropy scaling across sample sizes.
#[derive(Debug, Clone, Serialize)]
pub struct ScalingResult {
    pub points: Vec<EntropyPoint>,
}

/// Throughput measurement.
#[derive(Debug, Clone, Serialize)]
pub struct ThroughputResult {
    /// Bytes per second at each tested sample size.
    pub measurements: Vec<ThroughputPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThroughputPoint {
    pub sample_size: usize,
    pub bytes_per_second: f64,
    pub collection_time_ms: u64,
}

/// Cross-correlation between two sources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossCorrPair {
    pub source_a: String,
    pub source_b: String,
    pub correlation: f64,
    pub flagged: bool,
}

/// Cross-correlation matrix result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossCorrMatrix {
    pub pairs: Vec<CrossCorrPair>,
    /// Pairs with |r| > 0.3.
    pub flagged_count: usize,
}

/// Full per-source analysis.
#[derive(Debug, Clone, Serialize)]
pub struct SourceAnalysis {
    pub source_name: String,
    pub sample_size: usize,
    /// Shannon entropy (bits/byte, max 8.0).
    pub shannon_entropy: f64,
    /// Min-entropy via MCV estimator (bits/byte, max 8.0).
    pub min_entropy: f64,
    pub autocorrelation: AutocorrResult,
    pub spectral: SpectralResult,
    pub bit_bias: BitBiasResult,
    pub distribution: DistributionResult,
    pub stationarity: StationarityResult,
    pub runs: RunsResult,
}

// ---------------------------------------------------------------------------
// Analysis functions
// ---------------------------------------------------------------------------

/// Compute autocorrelation profile for lags 1..max_lag.
pub fn autocorrelation_profile(data: &[u8], max_lag: usize) -> AutocorrResult {
    let n = data.len();
    if n == 0 || max_lag == 0 {
        return AutocorrResult {
            lags: Vec::new(),
            max_abs_correlation: 0.0,
            max_abs_lag: 0,
            threshold: 0.0,
            violations: 0,
        };
    }

    let max_lag = max_lag.min(n / 2);
    if max_lag == 0 {
        return AutocorrResult {
            lags: Vec::new(),
            max_abs_correlation: 0.0,
            max_abs_lag: 0,
            threshold: 2.0 / (n as f64).sqrt(),
            violations: 0,
        };
    }
    let arr: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    let mean: f64 = arr.iter().sum::<f64>() / n as f64;
    let var: f64 = arr.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;

    let threshold = 2.0 / (n as f64).sqrt();
    let mut lags = Vec::with_capacity(max_lag);
    let mut max_abs = 0.0f64;
    let mut max_abs_lag = 1;
    let mut violations = 0;

    for lag in 1..=max_lag {
        let corr = if var < 1e-10 {
            0.0
        } else {
            let mut sum = 0.0;
            let count = n - lag;
            for i in 0..count {
                sum += (arr[i] - mean) * (arr[i + lag] - mean);
            }
            sum / (count as f64 * var)
        };

        if corr.abs() > max_abs {
            max_abs = corr.abs();
            max_abs_lag = lag;
        }
        if corr.abs() > threshold {
            violations += 1;
        }

        lags.push(LagCorrelation {
            lag,
            correlation: corr,
        });
    }

    AutocorrResult {
        lags,
        max_abs_correlation: max_abs,
        max_abs_lag,
        threshold,
        violations,
    }
}

/// Compute spectral analysis via DFT (no external FFT crate).
pub fn spectral_analysis(data: &[u8]) -> SpectralResult {
    let n = data.len().min(4096); // Cap for performance
    if n < 2 {
        return SpectralResult {
            peaks: Vec::new(),
            flatness: 0.0,
            dominant_frequency: 0.0,
            total_power: 0.0,
        };
    }

    let arr: Vec<f64> = data[..n].iter().map(|&b| b as f64 - 127.5).collect();

    // Compute power spectrum via DFT (only positive frequencies).
    let n_freq = n / 2;
    let mut power_spectrum: Vec<f64> = Vec::with_capacity(n_freq);

    for k in 1..=n_freq {
        let mut re = 0.0;
        let mut im = 0.0;
        let freq = 2.0 * PI * k as f64 / n as f64;
        for (j, &x) in arr.iter().enumerate() {
            re += x * (freq * j as f64).cos();
            im -= x * (freq * j as f64).sin();
        }
        power_spectrum.push((re * re + im * im) / n as f64);
    }

    let total_power: f64 = power_spectrum.iter().sum();

    // Spectral flatness = geometric_mean / arithmetic_mean
    let arith_mean = total_power / n_freq as f64;
    let log_sum: f64 = power_spectrum
        .iter()
        .map(|&p| if p > 1e-20 { p.ln() } else { -46.0 }) // ln(1e-20) ≈ -46
        .sum();
    let geo_mean = (log_sum / n_freq as f64).exp();
    let flatness = if arith_mean > 1e-20 {
        (geo_mean / arith_mean).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Find peaks.
    let mut indexed: Vec<(usize, f64)> = power_spectrum.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let peaks: Vec<SpectralBin> = indexed
        .iter()
        .take(10)
        .map(|&(i, p)| SpectralBin {
            frequency: (i + 1) as f64 / n as f64,
            power: p,
        })
        .collect();

    let dominant_frequency = peaks.first().map(|p| p.frequency).unwrap_or(0.0);

    SpectralResult {
        peaks,
        flatness,
        dominant_frequency,
        total_power,
    }
}

/// Analyze per-bit-position bias.
pub fn bit_bias(data: &[u8]) -> BitBiasResult {
    if data.is_empty() {
        return BitBiasResult {
            bit_probabilities: [0.0; 8],
            overall_bias: 0.0,
            chi_squared: 0.0,
            p_value: 1.0,
            has_significant_bias: false,
        };
    }

    let n = data.len() as f64;
    let mut counts = [0u64; 8];

    for &byte in data {
        for (bit, count) in counts.iter_mut().enumerate() {
            if byte & (1 << bit) != 0 {
                *count += 1;
            }
        }
    }

    let bit_probs: [f64; 8] = {
        let mut arr = [0.0; 8];
        for (i, &c) in counts.iter().enumerate() {
            arr[i] = c as f64 / n;
        }
        arr
    };

    let overall_bias: f64 = bit_probs.iter().map(|&p| (p - 0.5).abs()).sum::<f64>() / 8.0;

    // Chi-squared: for each bit position, expected = n/2
    let expected = n / 2.0;
    let chi_squared: f64 = counts
        .iter()
        .map(|&c| {
            let diff = c as f64 - expected;
            diff * diff / expected
        })
        .sum();

    // Approximate p-value from chi-squared with 8 degrees of freedom.
    // Use the incomplete gamma function approximation.
    let p_value = math::chi_squared_p_value(chi_squared, 8);

    let has_significant_bias = bit_probs.iter().any(|&p| (p - 0.5).abs() > 0.01);

    BitBiasResult {
        bit_probabilities: bit_probs,
        overall_bias,
        chi_squared,
        p_value,
        has_significant_bias,
    }
}

/// Compute distribution statistics.
pub fn distribution_stats(data: &[u8]) -> DistributionResult {
    if data.is_empty() {
        return DistributionResult {
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            skewness: 0.0,
            kurtosis: 0.0,
            histogram: vec![0u64; 256],
            ks_statistic: 0.0,
            ks_p_value: 1.0,
        };
    }

    let n = data.len() as f64;
    let arr: Vec<f64> = data.iter().map(|&b| b as f64).collect();

    let mean = arr.iter().sum::<f64>() / n;
    let variance = arr.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    let skewness = if std_dev > 1e-10 {
        arr.iter()
            .map(|&x| ((x - mean) / std_dev).powi(3))
            .sum::<f64>()
            / n
    } else {
        0.0
    };

    let kurtosis = if std_dev > 1e-10 {
        arr.iter()
            .map(|&x| ((x - mean) / std_dev).powi(4))
            .sum::<f64>()
            / n
            - 3.0 // excess kurtosis
    } else {
        0.0
    };

    // Histogram
    let mut histogram = vec![0u64; 256];
    for &b in data {
        histogram[b as usize] += 1;
    }

    // KS test vs uniform [0, 255]
    let mut sorted: Vec<f64> = arr.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut ks_stat = 0.0f64;
    for (i, &x) in sorted.iter().enumerate() {
        let empirical = (i + 1) as f64 / n;
        let theoretical = (x + 0.5) / 256.0; // uniform over [0, 255]
        let diff = (empirical - theoretical).abs();
        if diff > ks_stat {
            ks_stat = diff;
        }
    }
    // Approximate p-value (Kolmogorov-Smirnov)
    let sqrt_n = n.sqrt();
    let lambda = (sqrt_n + 0.12 + 0.11 / sqrt_n) * ks_stat;
    let ks_p = (-2.0 * lambda * lambda).exp().clamp(0.0, 1.0) * 2.0;

    DistributionResult {
        mean,
        variance,
        std_dev,
        skewness,
        kurtosis,
        histogram,
        ks_statistic: ks_stat,
        ks_p_value: ks_p.min(1.0),
    }
}

pub fn approximate_entropy(data: &[u8], m: usize, r: f64) -> ApproxEntropyResult {
    const MAX_SAMPLES: usize = 5000;

    let mut sampled: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    if sampled.len() > MAX_SAMPLES {
        let step = (sampled.len() / MAX_SAMPLES).max(1);
        sampled = sampled
            .iter()
            .step_by(step)
            .take(MAX_SAMPLES)
            .copied()
            .collect();
    }

    let actual_samples = sampled.len();
    if m == 0 || actual_samples <= m + 1 || !r.is_finite() || r <= 0.0 {
        return ApproxEntropyResult {
            apen: f64::NAN,
            m,
            r,
            phi_m: f64::NAN,
            phi_m_plus_1: f64::NAN,
            actual_samples,
            is_valid: false,
        };
    }

    let mean = sampled.iter().sum::<f64>() / actual_samples as f64;
    let variance = sampled
        .iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / (actual_samples as f64 - 1.0);
    let std_dev = variance.sqrt();
    if std_dev == 0.0 {
        return ApproxEntropyResult {
            apen: f64::NAN,
            m,
            r,
            phi_m: f64::NAN,
            phi_m_plus_1: f64::NAN,
            actual_samples,
            is_valid: false,
        };
    }

    fn phi(sampled: &[f64], m: usize, r: f64) -> Option<f64> {
        if sampled.len() < m {
            return None;
        }

        let n_templates = sampled.len() - m + 1;
        if n_templates == 0 {
            return None;
        }

        let mut sum_ln = 0.0;
        for i in 0..n_templates {
            let mut count = 0usize;
            for j in 0..n_templates {
                let mut max_dist = 0.0;
                for k in 0..m {
                    let dist = (sampled[i + k] - sampled[j + k]).abs();
                    if dist > max_dist {
                        max_dist = dist;
                    }
                    if max_dist >= r {
                        break;
                    }
                }
                if max_dist < r {
                    count += 1;
                }
            }

            if count == 0 {
                return None;
            }

            let c_i = count as f64 / n_templates as f64;
            if c_i <= 0.0 {
                return None;
            }
            sum_ln += c_i.ln();
        }

        Some(sum_ln / n_templates as f64)
    }

    let Some(phi_m) = phi(&sampled, m, r) else {
        return ApproxEntropyResult {
            apen: f64::NAN,
            m,
            r,
            phi_m: f64::NAN,
            phi_m_plus_1: f64::NAN,
            actual_samples,
            is_valid: false,
        };
    };

    let Some(phi_m_plus_1) = phi(&sampled, m + 1, r) else {
        return ApproxEntropyResult {
            apen: f64::NAN,
            m,
            r,
            phi_m,
            phi_m_plus_1: f64::NAN,
            actual_samples,
            is_valid: false,
        };
    };

    let apen = phi_m - phi_m_plus_1;
    ApproxEntropyResult {
        apen,
        m,
        r,
        phi_m,
        phi_m_plus_1,
        actual_samples,
        is_valid: apen.is_finite(),
    }
}

pub fn approximate_entropy_default(data: &[u8]) -> ApproxEntropyResult {
    const DEFAULT_M: usize = 2;
    const MAX_SAMPLES: usize = 5000;

    let mut sampled: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    if sampled.len() > MAX_SAMPLES {
        let step = (sampled.len() / MAX_SAMPLES).max(1);
        sampled = sampled
            .iter()
            .step_by(step)
            .take(MAX_SAMPLES)
            .copied()
            .collect();
    }

    if sampled.len() <= DEFAULT_M + 1 {
        return ApproxEntropyResult {
            apen: f64::NAN,
            m: DEFAULT_M,
            r: f64::NAN,
            phi_m: f64::NAN,
            phi_m_plus_1: f64::NAN,
            actual_samples: sampled.len(),
            is_valid: false,
        };
    }

    let mean = sampled.iter().sum::<f64>() / sampled.len() as f64;
    let variance = sampled
        .iter()
        .map(|x| {
            let d = x - mean;
            d * d
        })
        .sum::<f64>()
        / (sampled.len() as f64 - 1.0);
    let std_dev = variance.sqrt();
    if std_dev == 0.0 {
        return ApproxEntropyResult {
            apen: f64::NAN,
            m: DEFAULT_M,
            r: 0.0,
            phi_m: f64::NAN,
            phi_m_plus_1: f64::NAN,
            actual_samples: sampled.len(),
            is_valid: false,
        };
    }

    approximate_entropy(data, DEFAULT_M, 0.2 * std_dev)
}

pub fn anderson_darling(data: &[u8]) -> AndersonDarlingResult {
    const MAX_SAMPLES: usize = 2048;

    let critical_values = vec![
        (0.25, 0.787),
        (0.10, 1.248),
        (0.05, 1.610),
        (0.025, 2.092),
        (0.01, 2.880),
    ];

    if data.len() < 2 {
        return AndersonDarlingResult {
            statistic: 0.0,
            p_value: 0.0,
            is_uniform: false,
            critical_values,
            is_valid: false,
        };
    }

    let sample = if data.len() > MAX_SAMPLES {
        &data[..MAX_SAMPLES]
    } else {
        data
    };

    let n = sample.len();
    let n_f = n as f64;

    let mut sorted = sample.to_vec();
    sorted.sort_unstable();

    let u: Vec<f64> = sorted.iter().map(|&x| (x as f64 + 0.5) / 256.0).collect();

    let mut sum = 0.0;
    for i in 0..n {
        let coeff = (2 * i + 1) as f64;
        let left = u[i].ln();
        let right = (1.0 - u[n - 1 - i]).ln();
        sum += coeff * (left + right);
    }

    let a2 = -n_f - (sum / n_f);
    let statistic = a2 * (1.0 + 4.0 / n_f - 25.0 / (n_f * n_f));

    let p_value = if statistic > 10.0 {
        0.0
    } else if statistic >= 0.6 {
        (1.2937 - 5.709 * statistic + 0.0186 * statistic * statistic).exp()
    } else if statistic >= 0.34 {
        (0.9177 - 4.279 * statistic - 1.38 * statistic * statistic).exp()
    } else if statistic >= 0.2 {
        1.0 - (-8.318 + 42.796 * statistic - 59.938 * statistic * statistic).exp()
    } else {
        1.0 - (-13.436 + 101.14 * statistic - 223.73 * statistic * statistic).exp()
    }
    .clamp(0.0, 1.0);

    AndersonDarlingResult {
        statistic,
        p_value,
        is_uniform: p_value > 0.05,
        critical_values,
        is_valid: true,
    }
}

/// Test stationarity by comparing window means (ANOVA-like).
pub fn stationarity_test(data: &[u8]) -> StationarityResult {
    let n_windows = 10usize;
    let window_size = data.len() / n_windows;
    if window_size < 10 {
        return StationarityResult {
            is_stationary: true,
            f_statistic: 0.0,
            window_means: vec![],
            window_std_devs: vec![],
            n_windows: 0,
        };
    }

    let mut window_means = Vec::with_capacity(n_windows);
    let mut window_std_devs = Vec::with_capacity(n_windows);

    for w in 0..n_windows {
        let start = w * window_size;
        let end = start + window_size;
        let window = &data[start..end];
        let arr: Vec<f64> = window.iter().map(|&b| b as f64).collect();
        let mean = arr.iter().sum::<f64>() / arr.len() as f64;
        let var = arr.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / arr.len() as f64;
        window_means.push(mean);
        window_std_devs.push(var.sqrt());
    }

    // One-way ANOVA F-statistic
    let grand_mean: f64 = window_means.iter().sum::<f64>() / n_windows as f64;
    let between_var: f64 = window_means
        .iter()
        .map(|&m| (m - grand_mean).powi(2))
        .sum::<f64>()
        / (n_windows - 1) as f64
        * window_size as f64;

    let within_var: f64 = window_std_devs.iter().map(|&s| s * s).sum::<f64>() / n_windows as f64;

    let f_stat = if within_var > 1e-10 {
        between_var / within_var
    } else {
        0.0
    };

    // F critical value at α=0.05, df1=9, df2→∞ ≈ 1.88.
    // This is the asymptotic value; for finite df2 the true critical value
    // is slightly higher (more permissive), so this is conservative.
    let is_stationary = f_stat < 1.88;

    StationarityResult {
        is_stationary,
        f_statistic: f_stat,
        window_means,
        window_std_devs,
        n_windows,
    }
}

/// Analyze runs of consecutive identical byte values.
pub fn runs_analysis(data: &[u8]) -> RunsResult {
    if data.is_empty() {
        return RunsResult {
            longest_run: 0,
            expected_longest_run: 0.0,
            total_runs: 0,
            expected_runs: 0.0,
        };
    }

    let mut longest = 1usize;
    let mut current = 1usize;
    let mut total_runs = 1usize;

    for i in 1..data.len() {
        if data[i] == data[i - 1] {
            current += 1;
            if current > longest {
                longest = current;
            }
        } else {
            total_runs += 1;
            current = 1;
        }
    }

    let n = data.len() as f64;
    // Expected longest run of same byte ≈ log_256(n) for byte-level
    let expected_longest = (n.ln() / 256.0_f64.ln()).max(1.0);
    // Expected total runs ≈ n * (1 - 1/256) + 1
    let expected_runs = n * (1.0 - 1.0 / 256.0) + 1.0;

    RunsResult {
        longest_run: longest,
        expected_longest_run: expected_longest,
        total_runs,
        expected_runs,
    }
}

pub fn permutation_entropy(data: &[u8], order: usize, delay: usize) -> PermutationEntropyResult {
    if order < 2 || delay == 0 {
        return PermutationEntropyResult {
            permutation_entropy: 0.0,
            normalized_entropy: 0.0,
            order,
            delay,
            n_patterns: 0,
            pattern_counts: Vec::new(),
            is_valid: false,
        };
    }

    let required_len = (order - 1).saturating_mul(delay).saturating_add(2);
    if data.len() < required_len {
        return PermutationEntropyResult {
            permutation_entropy: 0.0,
            normalized_entropy: 0.0,
            order,
            delay,
            n_patterns: 0,
            pattern_counts: Vec::new(),
            is_valid: false,
        };
    }

    let arr: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    let n_windows = data.len() - (order - 1) * delay;

    let mut counts: HashMap<Vec<usize>, usize> = HashMap::new();
    for i in 0..n_windows {
        let mut window = Vec::with_capacity(order);
        for j in 0..order {
            window.push(arr[i + j * delay]);
        }

        let mut pattern: Vec<usize> = (0..order).collect();
        pattern.sort_by(|&a, &b| {
            window[a]
                .partial_cmp(&window[b])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        *counts.entry(pattern).or_insert(0) += 1;
    }

    let total_patterns = n_windows as f64;
    let mut entropy = 0.0;
    for &count in counts.values() {
        let p = count as f64 / total_patterns;
        if p > 0.0 {
            entropy -= p * p.ln();
        }
    }

    let max_entropy = (2..=order).map(|v| (v as f64).ln()).sum::<f64>();
    let normalized_entropy = if max_entropy > 0.0 {
        entropy / max_entropy
    } else {
        0.0
    };

    let n_patterns = counts.len();
    let mut pattern_counts: Vec<(Vec<usize>, usize)> = counts.into_iter().collect();
    pattern_counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pattern_counts.truncate(20);

    PermutationEntropyResult {
        permutation_entropy: entropy,
        normalized_entropy,
        order,
        delay,
        n_patterns,
        pattern_counts,
        is_valid: true,
    }
}

pub fn permutation_entropy_default(data: &[u8]) -> PermutationEntropyResult {
    permutation_entropy(data, 3, 1)
}

/// Compute cross-correlation matrix between multiple sources.
pub fn cross_correlation_matrix(sources_data: &[(String, Vec<u8>)]) -> CrossCorrMatrix {
    let mut pairs = Vec::new();
    let mut flagged_count = 0;

    for i in 0..sources_data.len() {
        for j in (i + 1)..sources_data.len() {
            let (ref name_a, ref data_a) = sources_data[i];
            let (ref name_b, ref data_b) = sources_data[j];
            let min_len = data_a.len().min(data_b.len());
            if min_len < 100 {
                continue;
            }
            let corr = pearson_correlation(&data_a[..min_len], &data_b[..min_len]);
            let flagged = corr.abs() > 0.3;
            if flagged {
                flagged_count += 1;
            }
            pairs.push(CrossCorrPair {
                source_a: name_a.clone(),
                source_b: name_b.clone(),
                correlation: corr,
                flagged,
            });
        }
    }

    CrossCorrMatrix {
        pairs,
        flagged_count,
    }
}

/// Run all per-source analysis on raw byte data.
pub fn full_analysis(source_name: &str, data: &[u8]) -> SourceAnalysis {
    use crate::conditioning::{quick_min_entropy, quick_shannon};
    SourceAnalysis {
        source_name: source_name.to_string(),
        sample_size: data.len(),
        shannon_entropy: quick_shannon(data),
        min_entropy: quick_min_entropy(data),
        autocorrelation: autocorrelation_profile(data, 100),
        spectral: spectral_analysis(data),
        bit_bias: bit_bias(data),
        distribution: distribution_stats(data),
        stationarity: stationarity_test(data),
        runs: runs_analysis(data),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Pearson correlation coefficient between two byte slices.
pub fn pearson_correlation(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len() as f64;
    let a_f: Vec<f64> = a.iter().map(|&x| x as f64).collect();
    let b_f: Vec<f64> = b.iter().map(|&x| x as f64).collect();
    let mean_a = a_f.iter().sum::<f64>() / n;
    let mean_b = b_f.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;
    for i in 0..a.len() {
        let da = a_f[i] - mean_a;
        let db = b_f[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    let denom = (var_a * var_b).sqrt();
    if denom < 1e-10 { 0.0 } else { cov / denom }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    fn random_data(n: usize) -> Vec<u8> {
        random_data_seeded(n, 0xdeadbeef)
    }

    #[test]
    fn test_autocorrelation_random() {
        let data = random_data(10000);
        let result = autocorrelation_profile(&data, 50);
        assert_eq!(result.lags.len(), 50);
        // Random data should have low autocorrelation.
        assert!(result.max_abs_correlation < 0.1);
    }

    #[test]
    fn test_autocorrelation_correlated() {
        // Data with strong lag-1 correlation.
        let mut data = vec![0u8; 1000];
        for (i, byte) in data.iter_mut().enumerate().take(1000) {
            *byte = if i % 2 == 0 { 200 } else { 50 };
        }
        let result = autocorrelation_profile(&data, 10);
        assert!(result.max_abs_correlation > 0.5);
    }

    #[test]
    fn test_spectral_analysis() {
        let data = random_data(1024);
        let result = spectral_analysis(&data);
        assert!(result.flatness > 0.0);
        assert!(!result.peaks.is_empty());
    }

    #[test]
    fn test_bit_bias_random() {
        let data = random_data(10000);
        let result = bit_bias(&data);
        for &p in &result.bit_probabilities {
            assert!((p - 0.5).abs() < 0.05);
        }
    }

    #[test]
    fn test_bit_bias_all_ones() {
        let data = vec![0xFF; 1000];
        let result = bit_bias(&data);
        for &p in &result.bit_probabilities {
            assert!((p - 1.0).abs() < 0.001);
        }
        assert!(result.has_significant_bias);
    }

    #[test]
    fn test_distribution_stats() {
        let data = random_data(10000);
        let result = distribution_stats(&data);
        // Mean should be near 127.5 for random bytes.
        assert!((result.mean - 127.5).abs() < 10.0);
        assert!(result.variance > 0.0);
    }

    #[test]
    fn test_anderson_darling_random_uniform() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = anderson_darling(&data);
        assert!(result.is_valid);
        assert!(result.is_uniform);
        assert!(result.p_value > 0.05);
    }

    #[test]
    fn test_anderson_darling_constant_rejects_uniform() {
        let data = vec![42u8; 1000];
        let result = anderson_darling(&data);
        assert!(result.is_valid);
        assert!(!result.is_uniform);
        assert!(result.statistic > 100.0);
    }

    #[test]
    fn test_anderson_darling_biased_zero_data() {
        let data: Vec<u8> = (0..1000).map(|_| 0u8).collect();
        let result = anderson_darling(&data);
        assert!(result.is_valid);
        assert!(!result.is_uniform);
    }

    #[test]
    fn test_anderson_darling_empty_invalid() {
        let result = anderson_darling(&[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_stationarity_stationary() {
        let data = random_data(10000);
        let result = stationarity_test(&data);
        assert!(result.is_stationary);
        assert_eq!(result.n_windows, 10);
    }

    #[test]
    fn test_runs_analysis() {
        let data = random_data(10000);
        let result = runs_analysis(&data);
        assert!(result.total_runs > 0);
        assert!(result.longest_run >= 1);
    }

    #[test]
    fn test_cross_correlation() {
        let a = random_data_seeded(1000, 0xdeadbeef);
        let b = random_data_seeded(1000, 0xcafebabe12345678);
        let result = cross_correlation_matrix(&[("a".to_string(), a), ("b".to_string(), b)]);
        assert_eq!(result.pairs.len(), 1);
        assert!(result.pairs[0].correlation.abs() < 0.3);
    }

    #[test]
    fn test_approximate_entropy_random_high() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = approximate_entropy_default(&data);
        assert!(result.is_valid);
        assert!(result.apen > 0.5);
    }

    #[test]
    fn test_approximate_entropy_periodic_lower_than_random() {
        let random = random_data_seeded(5000, 0xdeadbeef);
        let periodic: Vec<u8> = (0..5000).map(|i| (i % 4) as u8).collect();

        let random_result = approximate_entropy_default(&random);
        let periodic_result = approximate_entropy_default(&periodic);

        assert!(periodic_result.is_valid);
        assert!(random_result.is_valid);
        assert!(periodic_result.apen < random_result.apen);
    }

    #[test]
    fn test_approximate_entropy_constant_invalid() {
        let data = vec![42u8; 5000];
        let result = approximate_entropy_default(&data);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_approximate_entropy_empty_invalid() {
        let result = approximate_entropy_default(&[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn test_permutation_entropy_random_high() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = permutation_entropy(&data, 3, 1);
        assert!(result.is_valid);
        assert!(result.normalized_entropy > 0.95);
    }

    #[test]
    fn test_permutation_entropy_monotone_low() {
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let result = permutation_entropy(&data, 3, 1);
        assert!(result.is_valid);
        assert!(result.normalized_entropy < 0.3);
    }

    #[test]
    fn test_permutation_entropy_constant_zero() {
        let data = vec![42u8; 1000];
        let result = permutation_entropy_default(&data);
        assert!(result.is_valid);
        assert!(result.normalized_entropy.abs() < 1e-12);
        assert_eq!(result.n_patterns, 1);
    }

    #[test]
    fn test_permutation_entropy_empty_invalid() {
        let result = permutation_entropy_default(&[]);
        assert!(!result.is_valid);
        assert_eq!(result.n_patterns, 0);
    }

    #[test]
    fn test_full_analysis() {
        let data = random_data(1000);
        let result = full_analysis("test_source", &data);
        assert_eq!(result.source_name, "test_source");
        assert_eq!(result.sample_size, 1000);
    }
}
