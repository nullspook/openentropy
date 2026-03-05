---
title: 'Python Analysis Workflows'
description: 'Task-oriented analysis flows using Python dispatcher and focused calls'
---

This page groups the Python analysis surface by workflow.

## One-call Analysis (recommended)

```python
from openentropy import analyze
report = analyze([("source", data)], profile="deep")
```

Use `profile="security"` for cryptographic validation and
`profile="deep"` for full characterization.

## Tiered Analysis Calls

```python
from openentropy import (
    temporal_analysis_suite,
    statistics_analysis,
    synchrony_analysis,
    sample_entropy,
    dfa_analysis,
    rqa_analysis,
)

temporal = temporal_analysis_suite(data)
statistics = statistics_analysis(data)
sync = synchrony_analysis(data_a, data_b)
extended = {
    "sampen": sample_entropy(data),
    "dfa": dfa_analysis(data),
    "rqa": rqa_analysis(data),
}
```

## Forensic + Chaos + Trials

```python
from openentropy import full_analysis, chaos_analysis, trial_analysis

forensic = full_analysis("source", data)
chaos = chaos_analysis(data)
trials = trial_analysis(data, bits_per_trial=200)
```

## Source-to-source Comparison

```python
from openentropy import compare
delta = compare("a", data_a, "b", data_b)
```

## Related

- [Choose an Analysis Path](/openentropy/concepts/analysis-path/)
- [Python API Reference](/openentropy/python-sdk/reference/)
