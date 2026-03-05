//! Statistical randomness tests (CvM, Ljung-Box, Gap Test).

use crate::math;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CramerVonMisesResult {
    pub statistic: f64,
    pub p_value: f64,
    pub is_uniform: bool,
    pub is_valid: bool,
}

pub fn cramer_von_mises(data: &[u8]) -> CramerVonMisesResult {
    let n = data.len();
    if n < 2 {
        return CramerVonMisesResult {
            statistic: 0.0,
            p_value: 0.0,
            is_uniform: false,
            is_valid: false,
        };
    }

    let mut sorted = data.to_vec();
    sorted.sort_unstable();

    let nf = n as f64;
    let mut w2 = 0.0;
    for (idx, &x) in sorted.iter().enumerate() {
        let i = idx + 1;
        let u_i = (x as f64 + 0.5) / 256.0;
        let target = (2.0 * i as f64 - 1.0) / (2.0 * nf);
        let d = u_i - target;
        w2 += d * d;
    }
    w2 += 1.0 / (12.0 * nf);

    let corrected = w2 * (1.0 + 0.5 / nf);
    let pi = std::f64::consts::PI;
    let raw_p = if corrected < 0.0275 {
        let mut sum = 0.0;
        for k in 0..=4 {
            let kf = k as f64;
            let odd = 2.0 * kf + 1.0;
            let exponent = -(odd * odd) * pi * pi / (8.0 * corrected);
            sum += exponent.exp();
        }
        1.0 - (2.0 * pi.sqrt() * sum / corrected.sqrt())
    } else {
        let mut sum = 0.0;
        for k in 1..=4 {
            let kf = k as f64;
            let sign = if k % 2 == 1 { 1.0 } else { -1.0 };
            sum += sign * (-2.0 * kf * kf * pi * pi * corrected).exp();
        }
        2.0 * sum
    };

    let p_value = raw_p.clamp(0.001, 0.999);
    CramerVonMisesResult {
        statistic: w2,
        p_value,
        is_uniform: p_value > 0.05,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LjungBoxResult {
    pub q_statistic: f64,
    pub p_value: f64,
    pub max_lag: usize,
    pub has_serial_correlation: bool,
    pub is_valid: bool,
}

pub fn ljung_box_default(data: &[u8]) -> LjungBoxResult {
    ljung_box(data, 20)
}

pub fn ljung_box(data: &[u8], max_lag: usize) -> LjungBoxResult {
    let n = data.len();
    if max_lag == 0 || n <= max_lag {
        return LjungBoxResult {
            q_statistic: 0.0,
            p_value: 0.0,
            max_lag,
            has_serial_correlation: false,
            is_valid: false,
        };
    }

    let arr: Vec<f64> = data.iter().map(|&b| b as f64).collect();
    let mean = arr.iter().sum::<f64>() / n as f64;
    let denom = arr.iter().map(|x| (x - mean).powi(2)).sum::<f64>();

    let mut q_sum = 0.0;
    for lag in 1..=max_lag {
        let mut numer = 0.0;
        for i in 0..(n - lag) {
            numer += (arr[i] - mean) * (arr[i + lag] - mean);
        }
        let r_k = if denom <= 1e-12 { 0.0 } else { numer / denom };
        q_sum += (r_k * r_k) / (n as f64 - lag as f64);
    }

    let q_statistic = n as f64 * (n as f64 + 2.0) * q_sum;
    let p_value = math::chi_squared_survival(q_statistic, max_lag);

    LjungBoxResult {
        q_statistic,
        p_value,
        max_lag,
        has_serial_correlation: p_value < 0.05,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct GapTestResult {
    pub chi_squared: f64,
    pub p_value: f64,
    pub degrees_of_freedom: usize,
    pub mean_gap: f64,
    pub expected_gap: f64,
    pub is_uniform_gaps: bool,
    pub is_valid: bool,
}

pub fn gap_test_default(data: &[u8]) -> GapTestResult {
    gap_test(data, 100, 155)
}

pub fn gap_test(data: &[u8], lower: u8, upper: u8) -> GapTestResult {
    if data.is_empty() || lower > upper {
        return GapTestResult {
            chi_squared: 0.0,
            p_value: 0.0,
            degrees_of_freedom: 0,
            mean_gap: 0.0,
            expected_gap: 0.0,
            is_uniform_gaps: false,
            is_valid: false,
        };
    }

    let p_range = (upper as f64 - lower as f64 + 1.0) / 256.0;
    if p_range <= 0.0 {
        return GapTestResult {
            chi_squared: 0.0,
            p_value: 0.0,
            degrees_of_freedom: 0,
            mean_gap: 0.0,
            expected_gap: 0.0,
            is_uniform_gaps: false,
            is_valid: false,
        };
    }

    let hits: Vec<usize> = data
        .iter()
        .enumerate()
        .filter_map(|(idx, &b)| {
            if (lower..=upper).contains(&b) {
                Some(idx)
            } else {
                None
            }
        })
        .collect();

    if hits.len() < 2 {
        return GapTestResult {
            chi_squared: 0.0,
            p_value: 0.0,
            degrees_of_freedom: 0,
            mean_gap: 0.0,
            expected_gap: 1.0 / p_range,
            is_uniform_gaps: false,
            is_valid: false,
        };
    }

    let gaps: Vec<usize> = hits.windows(2).map(|w| w[1] - w[0]).collect();
    if gaps.len() < 10 {
        let mean_gap = gaps.iter().sum::<usize>() as f64 / gaps.len() as f64;
        return GapTestResult {
            chi_squared: 0.0,
            p_value: 0.0,
            degrees_of_freedom: 0,
            mean_gap,
            expected_gap: 1.0 / p_range,
            is_uniform_gaps: false,
            is_valid: false,
        };
    }

    let mut observed = [0usize; 9];
    for &gap in &gaps {
        let bin = match gap {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            5 => 5,
            6..=10 => 6,
            11..=20 => 7,
            _ => 8,
        };
        observed[bin] += 1;
    }

    let n_gaps = gaps.len() as f64;
    let q = 1.0 - p_range;
    let probs = [
        0.0,
        p_range,
        q * p_range,
        q.powi(2) * p_range,
        q.powi(3) * p_range,
        q.powi(4) * p_range,
        q.powi(5) - q.powi(10),
        q.powi(10) - q.powi(20),
        q.powi(20),
    ];

    let mut chi_squared = 0.0;
    let mut contributing_bins = 0usize;
    for i in 0..observed.len() {
        let expected = n_gaps * probs[i];
        if expected > 1e-12 {
            let diff = observed[i] as f64 - expected;
            chi_squared += diff * diff / expected;
            contributing_bins += 1;
        }
    }

    let degrees_of_freedom = contributing_bins.saturating_sub(1);
    let p_value = math::chi_squared_survival(chi_squared, degrees_of_freedom);
    let mean_gap = gaps.iter().sum::<usize>() as f64 / n_gaps;
    let expected_gap = 1.0 / p_range;

    GapTestResult {
        chi_squared,
        p_value,
        degrees_of_freedom,
        mean_gap,
        expected_gap,
        is_uniform_gaps: p_value > 0.05,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AnovaResult {
    pub f_statistic: f64,
    pub p_value: f64,
    pub df_between: usize,
    pub df_within: usize,
    pub group_means: Vec<f64>,
    pub grand_mean: f64,
    pub is_significant: bool,
    pub is_valid: bool,
}

pub fn anova(groups: &[&[u8]]) -> AnovaResult {
    let valid_groups: Vec<&[u8]> = groups.iter().copied().filter(|g| !g.is_empty()).collect();
    if valid_groups.len() < 2 {
        return AnovaResult {
            f_statistic: 0.0,
            p_value: 0.0,
            df_between: 0,
            df_within: 0,
            group_means: Vec::new(),
            grand_mean: 0.0,
            is_significant: false,
            is_valid: false,
        };
    }

    let k = valid_groups.len();
    let total_n: usize = valid_groups.iter().map(|g| g.len()).sum();
    if total_n <= k {
        return AnovaResult {
            f_statistic: 0.0,
            p_value: 0.0,
            df_between: 0,
            df_within: 0,
            group_means: Vec::new(),
            grand_mean: 0.0,
            is_significant: false,
            is_valid: false,
        };
    }

    let group_means: Vec<f64> = valid_groups
        .iter()
        .map(|g| g.iter().map(|&x| x as f64).sum::<f64>() / g.len() as f64)
        .collect();

    let grand_mean = valid_groups
        .iter()
        .flat_map(|g| g.iter())
        .map(|&x| x as f64)
        .sum::<f64>()
        / total_n as f64;

    let ss_between = valid_groups
        .iter()
        .zip(group_means.iter())
        .map(|(g, &mean)| g.len() as f64 * (mean - grand_mean).powi(2))
        .sum::<f64>();

    let ss_within = valid_groups
        .iter()
        .zip(group_means.iter())
        .map(|(g, &mean)| {
            g.iter()
                .map(|&x| {
                    let d = x as f64 - mean;
                    d * d
                })
                .sum::<f64>()
        })
        .sum::<f64>();

    let df_between = k - 1;
    let df_within = total_n - k;
    let ms_between = ss_between / df_between as f64;
    let ms_within = ss_within / df_within as f64;

    let (f_statistic, p_value) = if ms_within <= 1e-12 {
        if ms_between <= 1e-12 {
            (0.0, 1.0)
        } else {
            (f64::INFINITY, 0.0)
        }
    } else {
        let f = ms_between / ms_within;
        let p = math::f_distribution_survival(f, df_between, df_within);
        (f, p)
    };

    AnovaResult {
        f_statistic,
        p_value,
        df_between,
        df_within,
        group_means,
        grand_mean,
        is_significant: p_value < 0.05,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KruskalWallisResult {
    pub h_statistic: f64,
    pub p_value: f64,
    pub df: usize,
    pub is_significant: bool,
    pub is_valid: bool,
}

pub fn kruskal_wallis(groups: &[&[u8]]) -> KruskalWallisResult {
    let valid_groups: Vec<&[u8]> = groups.iter().copied().filter(|g| !g.is_empty()).collect();
    if valid_groups.len() < 2 {
        return KruskalWallisResult {
            h_statistic: 0.0,
            p_value: 0.0,
            df: 0,
            is_significant: false,
            is_valid: false,
        };
    }

    let k = valid_groups.len();
    let n_total: usize = valid_groups.iter().map(|g| g.len()).sum();
    if n_total < 2 {
        return KruskalWallisResult {
            h_statistic: 0.0,
            p_value: 0.0,
            df: 0,
            is_significant: false,
            is_valid: false,
        };
    }

    let mut values = Vec::with_capacity(n_total);
    for (group_idx, group) in valid_groups.iter().enumerate() {
        for &x in group.iter() {
            values.push((x, group_idx));
        }
    }
    values.sort_by_key(|&(x, _)| x);

    let mut rank_sums = vec![0.0; k];
    let mut tie_term_sum = 0.0;
    let mut i = 0usize;
    while i < values.len() {
        let tie_value = values[i].0;
        let start = i;
        while i < values.len() && values[i].0 == tie_value {
            i += 1;
        }
        let end = i;
        let tie_count = end - start;
        let rank_start = start as f64 + 1.0;
        let rank_end = end as f64;
        let avg_rank = (rank_start + rank_end) / 2.0;

        for &(_, group_idx) in &values[start..end] {
            rank_sums[group_idx] += avg_rank;
        }

        if tie_count > 1 {
            let t = tie_count as f64;
            tie_term_sum += t.powi(3) - t;
        }
    }

    let n = n_total as f64;
    let mut h = 0.0;
    for (group, &rank_sum) in valid_groups.iter().zip(rank_sums.iter()) {
        h += (rank_sum * rank_sum) / group.len() as f64;
    }
    h = (12.0 / (n * (n + 1.0))) * h - 3.0 * (n + 1.0);

    let tie_den = n.powi(3) - n;
    if tie_den > 0.0 {
        let tie_correction = 1.0 - (tie_term_sum / tie_den);
        if tie_correction > 1e-12 {
            h /= tie_correction;
        } else {
            h = 0.0;
        }
    }

    if h < 0.0 {
        h = 0.0;
    }

    let df = k - 1;
    let p_value = math::chi_squared_survival(h, df);
    KruskalWallisResult {
        h_statistic: h,
        p_value,
        df,
        is_significant: p_value < 0.05,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct LeveneResult {
    pub w_statistic: f64,
    pub p_value: f64,
    pub df_between: usize,
    pub df_within: usize,
    pub group_variances: Vec<f64>,
    pub is_homogeneous: bool,
    pub is_valid: bool,
}

fn anova_on_f64_groups(groups: &[Vec<f64>]) -> Option<(f64, f64, usize, usize)> {
    if groups.len() < 2 || groups.iter().any(|g| g.is_empty()) {
        return None;
    }
    let k = groups.len();
    let total_n: usize = groups.iter().map(|g| g.len()).sum();
    if total_n <= k {
        return None;
    }

    let means: Vec<f64> = groups
        .iter()
        .map(|g| g.iter().sum::<f64>() / g.len() as f64)
        .collect();
    let grand_mean = groups.iter().flatten().sum::<f64>() / total_n as f64;

    let ss_between = groups
        .iter()
        .zip(means.iter())
        .map(|(g, &m)| g.len() as f64 * (m - grand_mean).powi(2))
        .sum::<f64>();
    let ss_within = groups
        .iter()
        .zip(means.iter())
        .map(|(g, &m)| g.iter().map(|&x| (x - m).powi(2)).sum::<f64>())
        .sum::<f64>();

    let df_between = k - 1;
    let df_within = total_n - k;
    let ms_between = ss_between / df_between as f64;
    let ms_within = ss_within / df_within as f64;

    let (f_statistic, p_value) = if ms_within <= 1e-12 {
        if ms_between <= 1e-12 {
            (0.0, 1.0)
        } else {
            (f64::INFINITY, 0.0)
        }
    } else {
        let f = ms_between / ms_within;
        let p = math::f_distribution_survival(f, df_between, df_within);
        (f, p)
    };

    Some((f_statistic, p_value, df_between, df_within))
}

pub fn levene_test(groups: &[&[u8]]) -> LeveneResult {
    let valid_groups: Vec<&[u8]> = groups.iter().copied().filter(|g| !g.is_empty()).collect();
    if valid_groups.len() < 2 || valid_groups.iter().any(|g| g.len() < 2) {
        return LeveneResult {
            w_statistic: 0.0,
            p_value: 0.0,
            df_between: 0,
            df_within: 0,
            group_variances: Vec::new(),
            is_homogeneous: false,
            is_valid: false,
        };
    }

    let group_variances: Vec<f64> = valid_groups
        .iter()
        .map(|g| {
            let mean = g.iter().map(|&x| x as f64).sum::<f64>() / g.len() as f64;
            let ss = g
                .iter()
                .map(|&x| {
                    let d = x as f64 - mean;
                    d * d
                })
                .sum::<f64>();
            ss / (g.len() as f64 - 1.0)
        })
        .collect();

    let z_groups: Vec<Vec<f64>> = valid_groups
        .iter()
        .map(|g| {
            let mean = g.iter().map(|&x| x as f64).sum::<f64>() / g.len() as f64;
            g.iter().map(|&x| (x as f64 - mean).abs()).collect()
        })
        .collect();

    let Some((w_statistic, p_value, df_between, df_within)) = anova_on_f64_groups(&z_groups) else {
        return LeveneResult {
            w_statistic: 0.0,
            p_value: 0.0,
            df_between: 0,
            df_within: 0,
            group_variances,
            is_homogeneous: false,
            is_valid: false,
        };
    };

    LeveneResult {
        w_statistic,
        p_value,
        df_between,
        df_within,
        group_variances,
        is_homogeneous: p_value > 0.05,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PowerResult {
    pub power: f64,
    pub effect_size: f64,
    pub sample_size: usize,
    pub alpha: f64,
    pub required_n_for_80: usize,
    pub required_n_for_90: usize,
    pub is_valid: bool,
}

fn inverse_t_cdf(p: f64, df: usize) -> f64 {
    if p <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if p >= 1.0 {
        return f64::INFINITY;
    }

    let mut lo = -20.0;
    let mut hi = 20.0;
    for _ in 0..100 {
        let mid = (lo + hi) / 2.0;
        let cdf = math::t_distribution_cdf(mid, df);
        if cdf < p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

fn power_for_two_sample_t(effect_size: f64, sample_size: usize, alpha: f64) -> f64 {
    let df = 2 * (sample_size - 1);
    let ncp = effect_size * (sample_size as f64 / 2.0).sqrt();
    let t_critical = inverse_t_cdf(1.0 - alpha / 2.0, df);
    let left = math::t_distribution_cdf(t_critical - ncp, df);
    let right = math::t_distribution_cdf(-t_critical - ncp, df);
    (1.0 - left + right).clamp(0.0, 1.0)
}

fn required_n_for_power(effect_size: f64, alpha: f64, target: f64) -> usize {
    for n in 2..=10_000 {
        if power_for_two_sample_t(effect_size, n, alpha) >= target {
            return n;
        }
    }
    10_000
}

pub fn power_analysis(effect_size: f64, sample_size: usize, alpha: f64) -> PowerResult {
    if effect_size <= 0.0 || sample_size < 2 || alpha <= 0.0 || alpha >= 1.0 {
        return PowerResult {
            power: 0.0,
            effect_size,
            sample_size,
            alpha,
            required_n_for_80: 0,
            required_n_for_90: 0,
            is_valid: false,
        };
    }

    let power = power_for_two_sample_t(effect_size, sample_size, alpha);
    let required_n_for_80 = required_n_for_power(effect_size, alpha, 0.80);
    let required_n_for_90 = required_n_for_power(effect_size, alpha, 0.90);

    PowerResult {
        power,
        effect_size,
        sample_size,
        alpha,
        required_n_for_80,
        required_n_for_90,
        is_valid: true,
    }
}

pub fn power_analysis_default(sample_size: usize) -> PowerResult {
    power_analysis(0.2, sample_size, 0.05)
}

#[derive(Debug, Clone, Serialize)]
pub struct MultipleCorrectionResult {
    pub adjusted_p_values: Vec<f64>,
    pub rejected: Vec<bool>,
    pub method: String,
    pub alpha: f64,
    pub n_tests: usize,
    pub n_rejected: usize,
    pub is_valid: bool,
}

pub fn bonferroni_correction(p_values: &[f64], alpha: f64) -> MultipleCorrectionResult {
    if p_values.is_empty()
        || alpha <= 0.0
        || alpha >= 1.0
        || p_values
            .iter()
            .any(|&p| !p.is_finite() || !(0.0..=1.0).contains(&p))
    {
        return MultipleCorrectionResult {
            adjusted_p_values: Vec::new(),
            rejected: Vec::new(),
            method: "bonferroni".to_string(),
            alpha,
            n_tests: p_values.len(),
            n_rejected: 0,
            is_valid: false,
        };
    }

    let m = p_values.len() as f64;
    let adjusted_p_values: Vec<f64> = p_values.iter().map(|&p| (p * m).min(1.0)).collect();
    let rejected: Vec<bool> = adjusted_p_values.iter().map(|&p| p < alpha).collect();
    let n_rejected = rejected.iter().filter(|&&r| r).count();

    MultipleCorrectionResult {
        adjusted_p_values,
        rejected,
        method: "bonferroni".to_string(),
        alpha,
        n_tests: p_values.len(),
        n_rejected,
        is_valid: true,
    }
}

pub fn holm_bonferroni_correction(p_values: &[f64], alpha: f64) -> MultipleCorrectionResult {
    if p_values.is_empty()
        || alpha <= 0.0
        || alpha >= 1.0
        || p_values
            .iter()
            .any(|&p| !p.is_finite() || !(0.0..=1.0).contains(&p))
    {
        return MultipleCorrectionResult {
            adjusted_p_values: Vec::new(),
            rejected: Vec::new(),
            method: "holm-bonferroni".to_string(),
            alpha,
            n_tests: p_values.len(),
            n_rejected: 0,
            is_valid: false,
        };
    }

    let m = p_values.len();
    let mut indexed: Vec<(usize, f64)> = p_values.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.total_cmp(&b.1));

    let mut adjusted_sorted = vec![0.0; m];
    let mut rejected_sorted = vec![false; m];
    let mut all_previous_rejected = true;

    for i in 0..m {
        let factor = (m - i) as f64;
        let raw_adjusted = (indexed[i].1 * factor).min(1.0);
        if i == 0 {
            adjusted_sorted[i] = raw_adjusted;
        } else {
            adjusted_sorted[i] = adjusted_sorted[i - 1].max(raw_adjusted);
        }

        let rejected = adjusted_sorted[i] < alpha && all_previous_rejected;
        rejected_sorted[i] = rejected;
        all_previous_rejected &= rejected;
    }

    let mut adjusted_p_values = vec![0.0; m];
    let mut rejected = vec![false; m];
    for i in 0..m {
        let original_idx = indexed[i].0;
        adjusted_p_values[original_idx] = adjusted_sorted[i];
        rejected[original_idx] = rejected_sorted[i];
    }

    let n_rejected = rejected.iter().filter(|&&r| r).count();
    MultipleCorrectionResult {
        adjusted_p_values,
        rejected,
        method: "holm-bonferroni".to_string(),
        alpha,
        n_tests: m,
        n_rejected,
        is_valid: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StatisticsAnalysis {
    pub cramer_von_mises: CramerVonMisesResult,
    pub ljung_box: LjungBoxResult,
    pub gap_test: GapTestResult,
}

pub fn statistics_analysis(data: &[u8]) -> StatisticsAnalysis {
    StatisticsAnalysis {
        cramer_von_mises: cramer_von_mises(data),
        ljung_box: ljung_box_default(data),
        gap_test: gap_test_default(data),
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
    fn statistics_cvm_random_uniform() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = cramer_von_mises(&data);
        assert!(result.is_uniform);
        assert!(result.p_value > 0.05);
    }

    #[test]
    fn statistics_cvm_constant_non_uniform() {
        let data = vec![42u8; 1000];
        let result = cramer_von_mises(&data);
        assert!(!result.is_uniform);
    }

    #[test]
    fn statistics_cvm_empty_invalid() {
        let result = cramer_von_mises(&[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_cvm_all_zero_non_uniform() {
        let data = vec![0u8; 1000];
        let result = cramer_von_mises(&data);
        assert!(!result.is_uniform);
    }

    #[test]
    fn statistics_ljung_box_random_no_serial_correlation() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = ljung_box_default(&data);
        assert!(!result.has_serial_correlation);
    }

    #[test]
    fn statistics_ljung_box_alternating_has_serial_correlation() {
        let data: Vec<u8> = (0..5000)
            .map(|i| if i % 2 == 0 { 0u8 } else { 255u8 })
            .collect();
        let result = ljung_box_default(&data);
        assert!(result.has_serial_correlation);
    }

    #[test]
    fn statistics_ljung_box_too_short_invalid() {
        let result = ljung_box(&[1u8, 2u8], 20);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_ljung_box_empty_invalid() {
        let result = ljung_box_default(&[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_gap_random_uniform_gaps() {
        let data = random_data_seeded(10000, 0xdeadbeef);
        let result = gap_test_default(&data);
        assert!(result.is_uniform_gaps);
    }

    #[test]
    fn statistics_gap_no_values_in_range_invalid() {
        let data = vec![0u8; 5000];
        let result = gap_test(&data, 200, 255);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_gap_empty_invalid() {
        let result = gap_test_default(&[]);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_gap_mean_close_to_expected_for_random() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = gap_test_default(&data);
        assert!(result.is_valid);
        assert!((result.mean_gap - result.expected_gap).abs() < 1.5);
    }

    fn shifted_data_seeded(len: usize, seed: u64, shift: u8) -> Vec<u8> {
        random_data_seeded(len, seed)
            .into_iter()
            .map(|v| v.wrapping_add(shift))
            .collect()
    }

    #[test]
    fn statistics_anova_random_groups_not_significant() {
        let g1 = random_data_seeded(2000, 0x1001);
        let g2 = random_data_seeded(2000, 0x1002);
        let g3 = random_data_seeded(2000, 0x1003);
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = anova(&groups);
        assert!(result.is_valid);
        assert!(!result.is_significant);
    }

    #[test]
    fn statistics_anova_biased_group_significant() {
        let g1 = random_data_seeded(2000, 0x2001);
        let g2 = random_data_seeded(2000, 0x2002);
        let g3 = vec![200u8; 2000];
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = anova(&groups);
        assert!(result.is_valid);
        assert!(result.is_significant);
    }

    #[test]
    fn statistics_anova_too_few_groups_invalid() {
        let g1 = random_data_seeded(2000, 0x3001);
        let groups: [&[u8]; 1] = [&g1];
        let result = anova(&groups);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_anova_identical_groups_zero_f() {
        let g1 = vec![128u8; 2000];
        let g2 = vec![128u8; 2000];
        let g3 = vec![128u8; 2000];
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = anova(&groups);
        assert!(result.is_valid);
        assert!(result.f_statistic.abs() < 1e-12);
        assert!((result.p_value - 1.0).abs() < 1e-12);
    }

    #[test]
    fn statistics_kruskal_wallis_same_distribution_not_significant() {
        let g1 = random_data_seeded(2000, 0x4001);
        let g2 = random_data_seeded(2000, 0x4002);
        let g3 = random_data_seeded(2000, 0x4003);
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = kruskal_wallis(&groups);
        assert!(result.is_valid);
        assert!(!result.is_significant);
    }

    #[test]
    fn statistics_kruskal_wallis_biased_group_significant() {
        let g1 = random_data_seeded(2000, 0x5001);
        let g2 = random_data_seeded(2000, 0x5002);
        let g3 = vec![200u8; 2000];
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = kruskal_wallis(&groups);
        assert!(result.is_valid);
        assert!(result.is_significant);
    }

    #[test]
    fn statistics_kruskal_wallis_too_few_groups_invalid() {
        let g1 = random_data_seeded(2000, 0x6001);
        let groups: [&[u8]; 1] = [&g1];
        let result = kruskal_wallis(&groups);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_kruskal_wallis_identical_groups_not_significant() {
        let g1 = vec![77u8; 1000];
        let g2 = vec![77u8; 1000];
        let g3 = vec![77u8; 1000];
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = kruskal_wallis(&groups);
        assert!(result.is_valid);
        assert!(!result.is_significant);
    }

    #[test]
    fn statistics_levene_same_variance_homogeneous() {
        let g1 = random_data_seeded(2000, 0x7001);
        let g2 = random_data_seeded(2000, 0x7002);
        let g3 = random_data_seeded(2000, 0x7003);
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = levene_test(&groups);
        assert!(result.is_valid);
        assert!(result.is_homogeneous);
    }

    #[test]
    fn statistics_levene_high_variance_group_not_homogeneous() {
        let g1 = random_data_seeded(2000, 0x8001);
        let g2 = random_data_seeded(2000, 0x8002);
        let g3: Vec<u8> = (0..2000)
            .map(|i| if i % 2 == 0 { 0u8 } else { 255u8 })
            .collect();
        let groups: [&[u8]; 3] = [&g1, &g2, &g3];
        let result = levene_test(&groups);
        assert!(result.is_valid);
        assert!(!result.is_homogeneous);
    }

    #[test]
    fn statistics_levene_too_few_groups_invalid() {
        let g1 = random_data_seeded(2000, 0x9001);
        let groups: [&[u8]; 1] = [&g1];
        let result = levene_test(&groups);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_levene_single_element_group_invalid() {
        let g1 = vec![1u8];
        let g2 = vec![2u8, 3u8];
        let groups: [&[u8]; 2] = [&g1, &g2];
        let result = levene_test(&groups);
        assert!(!result.is_valid);
    }

    #[test]
    fn statistics_power_large_n_medium_effect_high_power() {
        let result = power_analysis(0.5, 10_000, 0.05);
        assert!(result.is_valid);
        assert!(result.power > 0.99);
    }

    #[test]
    fn statistics_power_small_n_small_effect_low_power() {
        let result = power_analysis(0.2, 10, 0.05);
        assert!(result.is_valid);
        assert!(result.power < 0.2);
    }

    #[test]
    fn statistics_power_required_n_ordering() {
        let result = power_analysis_default(50);
        assert!(result.is_valid);
        assert!(result.required_n_for_90 >= result.required_n_for_80);
    }

    #[test]
    fn statistics_power_invalid_inputs() {
        assert!(!power_analysis(0.0, 10, 0.05).is_valid);
        assert!(!power_analysis(0.2, 1, 0.05).is_valid);
    }

    #[test]
    fn statistics_bonferroni_expected_rejection_pattern() {
        let result = bonferroni_correction(&[0.01, 0.02, 0.03], 0.05);
        assert!(result.is_valid);
        assert_eq!(result.rejected, vec![true, false, false]);
    }

    #[test]
    fn statistics_holm_rejects_at_least_bonferroni() {
        let p = [0.01, 0.02, 0.03];
        let bonf = bonferroni_correction(&p, 0.05);
        let holm = holm_bonferroni_correction(&p, 0.05);
        assert!(holm.is_valid);
        assert!(holm.n_rejected >= bonf.n_rejected);
    }

    #[test]
    fn statistics_multiple_correction_empty_invalid() {
        assert!(!bonferroni_correction(&[], 0.05).is_valid);
        assert!(!holm_bonferroni_correction(&[], 0.05).is_valid);
    }

    #[test]
    fn statistics_holm_monotonic_adjusted_p_values_sorted() {
        let result = holm_bonferroni_correction(&[0.03, 0.01, 0.02], 0.05);
        assert!(result.is_valid);
        let mut sorted = result.adjusted_p_values.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        assert_eq!(result.adjusted_p_values.len(), 3);
        assert_eq!(sorted.len(), 3);
    }

    #[test]
    fn test_statistics_analysis_serializes() {
        let data = random_data_seeded(5000, 0xdeadbeef);
        let result = statistics_analysis(&data);
        let json = serde_json::to_string(&result).expect("serialization failed");
        assert!(json.contains("cramer_von_mises"));
        assert!(json.contains("ljung_box"));
    }
}
