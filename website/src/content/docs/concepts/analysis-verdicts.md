---
title: 'Verdict System'
description: 'PASS/WARN/FAIL thresholds for forensic, chaos, and advanced statistical metrics'
---

Each source report includes automated verdicts for forensic, chaos, and selected
extended statistical metrics.
Verdicts are computed in `openentropy_core::verdict`.

> **What it is:** The threshold layer that maps raw metrics to `PASS`/`WARN`/`FAIL`/`N/A`.
>
> **Use it for:** Fast triage, then follow up by reading the underlying metric values.
>
> **Input shape:** Metric outputs from analysis modules (not raw bytes directly).

## Verdict Values

- `PASS`: in expected range
- `WARN`: borderline or potentially concerning
- `FAIL`: outside acceptable range
- `N/A`: metric unavailable or invalid

## Forensic Verdict Thresholds

| Metric | PASS | WARN | FAIL |
|--------|------|------|------|
| Autocorrelation | `max |r| <= 0.05` | `<= 0.15` | `> 0.15` |
| Spectral flatness | `>= 0.75` | `>= 0.50` | `< 0.50` |
| Bit bias | low overall + no significant bit | any significant bit | high overall bias |
| Distribution (KS p) | `>= 0.01` | `>= 0.001` | `< 0.001` |
| Stationarity | stationary and low F | not stationary | high F |
| Runs | near expected | moderate drift | severe drift |

## Chaos Verdict Thresholds

| Metric | PASS | WARN | FAIL |
|--------|------|------|------|
| Hurst | `0.4 <= H <= 0.6` | `0.3 <= H <= 0.7` | outside warn band |
| Lyapunov | `|lambda| < 0.1` | `|lambda| < 0.2` | `|lambda| >= 0.2` |
| Correlation dimension | `D2 > 3.0` | `D2 > 2.0` | `D2 <= 2.0` |
| BiEntropy | `> 0.95` | `> 0.90` | `<= 0.90` |
| Compression ratio | `> 0.99` | `> 0.95` | `<= 0.95` |

## Extended Verdict Thresholds

| Metric | PASS | WARN | FAIL |
|--------|------|------|------|
| Sample entropy | `> 1.0` | `0.5..=1.0` | `< 0.5` |
| Approximate entropy | `> 1.0` | `0.5..=1.0` | `< 0.5` |
| DFA alpha | `0.4 < α < 0.6` | `0.3..=0.7` | outside warn band |
| RQA determinism | `< 0.1` | `<= 0.3` | `> 0.3` |
| Permutation entropy | `> 0.95` | `0.8..=0.95` | `< 0.8` |
| Anderson-Darling p-value | `> 0.05` | `0.01..=0.05` | `< 0.01` |
| Cramer-von Mises p-value | `> 0.05` | `0.01..=0.05` | `< 0.01` |
| Ljung-Box p-value | `> 0.05` | `0.01..=0.05` | `< 0.01` |

## Extended Verdict Thresholds

| Metric | PASS | WARN | FAIL |
|--------|------|------|------|
| Sample entropy | `> 1.0` | `0.5..=1.0` | `< 0.5` |
| Approximate entropy | `> 1.0` | `0.5..=1.0` | `< 0.5` |
| DFA alpha | `0.4 < α < 0.6` | `0.3..=0.7` | outside warn band |
| RQA determinism | `< 0.1` | `<= 0.3` | `> 0.3` |
| Permutation entropy | `> 0.95` | `0.8..=0.95` | `< 0.8` |
| Anderson-Darling p-value | `> 0.05` | `0.01..=0.05` | `< 0.01` |
| Cramer-von Mises p-value | `> 0.05` | `0.01..=0.05` | `< 0.01` |
| Ljung-Box p-value | `> 0.05` | `0.01..=0.05` | `< 0.01` |

## Reading Results

Treat verdicts as triage, not absolute proof. A single fail can reflect sample
size, transient conditions, or one sensitive metric. Confirm with larger samples
and `deep` profile runs before making hard decisions.

`AnalysisReport.verdicts` from the dispatcher includes forensic + core chaos
verdicts. Extended verdict helpers (`verdict_sampen`, `verdict_apen`,
`verdict_dfa`, `verdict_rqa_det`, `verdict_permen`, `verdict_anderson_darling`,
`verdict_ljung_box`, `verdict_cramer_von_mises`) are used by CLI extended
analysis output and are available for custom integrations.

## Related

- [Choose an Analysis Path](/openentropy/concepts/analysis-path/)
- [Forensic Analysis](/openentropy/concepts/analysis-forensic/)
- [Chaos Theory Analysis](/openentropy/concepts/analysis-chaos/)
