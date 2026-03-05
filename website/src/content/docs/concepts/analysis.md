---
title: 'Analysis System'
description: 'Overview of profiles and analysis categories with links to detailed pages'
---

openentropy analysis is organized into tiers controlled by the dispatcher in
`openentropy_core::dispatcher`: forensic baseline plus optional entropy,
chaos core/extended, temporal, statistics, synchrony, trials, and
cross-correlation.

Use this page as the hub, then jump to focused deep-dive pages.

## Quick Start

```bash
openentropy analyze --profile quick
openentropy analyze --profile deep
openentropy analyze --profile security
```

```python
from openentropy import analyze
report = analyze([("my_source", data)], profile="deep")
```

```rust
use openentropy_core::dispatcher::{analyze, AnalysisProfile};
let report = analyze(&[("my_source", &data)], &AnalysisProfile::Deep.to_config());
```

## Analysis Profiles

| Profile | Forensic | Entropy | Chaos Core | Chaos Extended | Temporal | Statistics | Synchrony | Trials | Cross-Corr | Use Case |
|---------|:--------:|:-------:|:----------:|:--------------:|:--------:|:----------:|:---------:|:------:|:----------:|----------|
| `quick` | ✓ | — | — | — | — | — | — | — | — | Fast sanity check (10K samples) |
| `standard` | ✓ | — | — | — | — | — | — | — | — | Default analysis (50K samples) |
| `deep` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | —* | ✓ | ✓ | Full characterization (100K samples) |
| `security` | ✓ | ✓ | — | — | — | — | — | — | — | Cryptographic validation |

`*` Synchrony is explicit (`--synchrony`) because it requires 2+ streams.

## Detailed Pages

- [Forensic Analysis](/openentropy/concepts/analysis-forensic/)
- [Entropy Breakdown](/openentropy/concepts/analysis-entropy/)
- [Chaos Theory Analysis](/openentropy/concepts/analysis-chaos/)
- [Temporal Analysis](/openentropy/concepts/analysis-temporal/)
- [Statistics Analysis](/openentropy/concepts/analysis-statistics/)
- [Synchrony Analysis](/openentropy/concepts/analysis-synchrony/)
- [Cross-Correlation](/openentropy/concepts/analysis-cross-correlation/)
- [Verdict System](/openentropy/concepts/analysis-verdicts/)
- [Trial Analysis Methodology](/openentropy/concepts/trials/)

## Forensic Analysis

For metrics and interpretation of autocorrelation, spectral flatness, bit bias,
distribution, stationarity, and runs, see
[Forensic Analysis](/openentropy/concepts/analysis-forensic/).

## Entropy Breakdown

For Shannon/MCV/collision/Markov/compression/t-tuple estimators and grade
interpretation, see
[Entropy Breakdown](/openentropy/concepts/analysis-entropy/).

## Chaos Theory Analysis

For core and extended chaos interpretation (Hurst/Lyapunov/correlation-dimension/BiEntropy/
epiplexity plus SampEn/ApEn/DFA/RQA/Hurst variants), see
[Chaos Theory Analysis](/openentropy/concepts/analysis-chaos/).

## Temporal Analysis

For change-point/anomaly/burst/shift/drift and inter-session stability,
see [Temporal Analysis](/openentropy/concepts/analysis-temporal/).

## Statistics Analysis

For Cramer-von Mises, Ljung-Box, gap tests, and group-level inference tools,
see [Statistics Analysis](/openentropy/concepts/analysis-statistics/).

## Synchrony Analysis

For mutual information, sign coherence proxy, cross-sync, and global events,
see [Synchrony Analysis](/openentropy/concepts/analysis-synchrony/).

## Cross-Correlation

For source-independence checks and pair interpretation, see
[Cross-Correlation](/openentropy/concepts/analysis-cross-correlation/).

## Verdict System

For PASS/WARN/FAIL threshold tables and reading guidance, see
[Verdict System](/openentropy/concepts/analysis-verdicts/).

## Attribution

The expanded analysis inventory in OpenEntropy was informed by the open-source
work in [vikingdude81/qrng-analysis-toolkit](https://github.com/vikingdude81/qrng-analysis-toolkit).
