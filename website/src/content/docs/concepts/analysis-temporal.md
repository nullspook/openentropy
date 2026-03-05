---
title: 'Temporal Analysis'
description: 'Detecting non-stationary behavior over time in entropy streams'
---

Temporal analysis tracks how a source changes across the stream.

Implemented in `openentropy_core::temporal`.

> **What it is:** A time-structure analysis for drift, shifts, bursts, and anomalies.
>
> **Use it for:** Diagnosing instability over time that aggregate metrics can hide.
>
> **Input shape:** Usually one byte stream; stability checks use multiple session streams.

## Use this when

- You suspect drift, bursts, or regime shifts over time.
- Forensic metrics look unstable across repeated runs.
- You need change-point or anomaly windows, not just aggregate scores.

## Methods

- `change_point_detection` / `_default`: significant mean shifts between adjacent segments
- `anomaly_detection` / `_default`: outlier windows and anomaly rate
- `burst_detection` / `_default`: high-intensity burst intervals
- `shift_detection` / `_default`: windowed mean shifts with z-score threshold
- `temporal_drift` / `_default`: trend slope and drift confidence over segments
- `inter_session_stability`: cross-session consistency score
- `temporal_analysis_suite`: one-call orchestrator for single-stream temporal checks

## CLI

```bash
openentropy analyze --temporal
openentropy analyze --profile deep
```

## Python SDK

```python
from openentropy import temporal_analysis_suite, temporal_drift, inter_session_stability

suite = temporal_analysis_suite(data)
drift = temporal_drift(data)
stability = inter_session_stability([data_a, data_b, data_c])
```

## Rust SDK

```rust
use openentropy_core::{temporal_analysis_suite, temporal_drift, inter_session_stability};

let suite = temporal_analysis_suite(&data);
let drift = temporal_drift(&data, 10);
let stability = inter_session_stability(&[&data_a, &data_b, &data_c]);
```

## Related

- [Choose an Analysis Path](/openentropy/concepts/analysis-path/)
- [Statistics Analysis](/openentropy/concepts/analysis-statistics/)
