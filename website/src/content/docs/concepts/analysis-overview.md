---
title: 'Choose an Analysis Path'
description: 'How to pick the right analysis profile and module for your goal'
slug: concepts/analysis-path
---

OpenEntropy analysis is exposed through two execution surfaces:

| Surface | Entry point | What it controls |
|---------|-------------|------------------|
| **Dispatcher API** | `openentropy_core::dispatcher::analyze` | `forensic`, `entropy`, `chaos`, `trials`, `cross_correlation` |
| **CLI analyze command** | `openentropy analyze` | Dispatcher modules plus CLI-only toggles: `--temporal`, `--statistics`, `--synchrony`, `--chaos-extended` |

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

## Profiles at a glance

CLI profiles (`openentropy analyze`):

| Profile | Forensic | Entropy | Chaos Core | Chaos Extended | Temporal | Statistics | Synchrony | Trials | Cross-Corr |
|---------|:--------:|:-------:|:----------:|:--------------:|:--------:|:----------:|:---------:|:------:|:----------:|
| `quick` | ✓ | — | — | — | — | — | —* | — | — |
| `standard` | ✓ | — | — | — | — | — | —* | — | — |
| `deep` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | —* | ✓ | ✓ |
| `security` | ✓ | ✓ | — | — | — | — | —* | — | — |

Dispatcher profiles (`openentropy_core::dispatcher::AnalysisProfile` and Python `analyze(profile=...)`):

| Profile | Forensic | Entropy | Chaos | Trials | Cross-Corr |
|---------|:--------:|:-------:|:-----:|:------:|:----------:|
| `quick` | ✓ | — | — | — | — |
| `standard` | ✓ | — | — | — | — |
| `deep` | ✓ | ✓ | ✓ | ✓ | ✓ |
| `security` | ✓ | ✓ | — | — | — |

`*` Synchrony always requires 2+ streams and is enabled explicitly with `--synchrony` (CLI) or direct synchrony APIs.

## Which analysis should I use?

Start with the question you are trying to answer:

| If your question is... | Use | Why |
|------------------------|-----|-----|
| "Is this source healthy at a basic level?" | [Forensic Analysis](/openentropy/concepts/analysis-forensic/) | Baseline checks: autocorrelation, spectral flatness, bias, distribution, stationarity, runs |
| "How much entropy density does this source have?" | [Entropy Breakdown](/openentropy/concepts/analysis-entropy/) | Multi-estimator min-entropy view and grade |
| "Are two sources independent enough to combine?" | [Cross-Correlation](/openentropy/concepts/analysis-cross-correlation/) | Pairwise correlation matrix and flagged pairs |
| "Do I trust PASS/WARN/FAIL quickly?" | [Verdict System](/openentropy/concepts/analysis-verdicts/) | Threshold model used by CLI and reports |
| "Is behavior drifting over time?" | [Temporal Analysis](/openentropy/concepts/analysis-temporal/) | Change points, anomalies, bursts, shifts, drift |
| "Are there formal dependence/goodness-of-fit issues?" | [Statistics Analysis](/openentropy/concepts/analysis-statistics/) | Cramer-von Mises, Ljung-Box, gap, group tests |
| "Are two streams coupled or event-synchronized?" | [Synchrony Analysis](/openentropy/concepts/analysis-synchrony/) | Mutual information, coherence proxy, cross-sync, global events |
| "Is this random-looking output actually structured/chaotic?" | [Chaos Theory Analysis](/openentropy/concepts/analysis-chaos/) | Core + extended entropy/complexity methods |
| "How stable is deviation across repeated 200-bit trials?" | [Trial Analysis Methodology](/openentropy/concepts/trials/) | PEAR-style trial framing and Stouffer combination |

## Recommended Workflow

For most users:

1. Start with `openentropy analyze --profile security` for security validation or `--profile deep` for research.
2. Review [Forensic Analysis](/openentropy/concepts/analysis-forensic/) and [Entropy Breakdown](/openentropy/concepts/analysis-entropy/) first.
3. Use [Verdict System](/openentropy/concepts/analysis-verdicts/) for triage, then inspect raw metrics on any WARN/FAIL.
4. Add advanced modules only when needed: temporal/statistics/synchrony/chaos/trials.

## Analysis pages

- [Forensic Analysis](/openentropy/concepts/analysis-forensic/)
- [Entropy Breakdown](/openentropy/concepts/analysis-entropy/)
- [Verdict System](/openentropy/concepts/analysis-verdicts/)
- [Cross-Correlation](/openentropy/concepts/analysis-cross-correlation/)
- [Trial Analysis Methodology](/openentropy/concepts/trials/)
- [Chaos Theory Analysis](/openentropy/concepts/analysis-chaos/)
- [Temporal Analysis](/openentropy/concepts/analysis-temporal/)
- [Statistics Analysis](/openentropy/concepts/analysis-statistics/)
- [Synchrony Analysis](/openentropy/concepts/analysis-synchrony/)

## Attribution

The expanded analysis inventory in OpenEntropy was informed by the open-source
work in [vikingdude81/qrng-analysis-toolkit](https://github.com/vikingdude81/qrng-analysis-toolkit).
