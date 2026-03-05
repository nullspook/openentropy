---
title: 'Statistics Analysis'
description: 'Goodness-of-fit, serial-correlation, and group inference methods'
---

Statistics analysis provides classical tests for distributional fit and dependence.

Implemented in `openentropy_core::statistics`.

## Single-stream methods

- `cramer_von_mises`: uniformity goodness-of-fit test
- `ljung_box` / `_default`: multi-lag autocorrelation significance test
- `gap_test` / `_default`: interval gap structure against expected random gaps
- `statistics_analysis`: one-call orchestrator for the single-stream statistics set

## Group-level methods

- `anova`: parametric group mean test
- `kruskal_wallis`: non-parametric group rank test
- `levene_test`: equal-variance test across groups
- `power_analysis` / `_default`: approximate power and required sample sizing
- `bonferroni_correction`, `holm_bonferroni_correction`: family-wise correction

## CLI

```bash
openentropy analyze --statistics
openentropy analyze --profile deep
```

## Python SDK

```python
from openentropy import statistics_analysis, ljung_box, anova, holm_bonferroni_correction

stats = statistics_analysis(data)
lb = ljung_box(data)
a = anova([group_a, group_b, group_c])
holm = holm_bonferroni_correction([0.001, 0.02, 0.2], 0.05)
```

## Related

- [Analysis System](/openentropy/concepts/analysis/)
- [Verdict System](/openentropy/concepts/analysis-verdicts/)
