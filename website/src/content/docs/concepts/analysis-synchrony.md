---
title: 'Synchrony Analysis'
description: 'Cross-stream dependence and shared-event detection'
---

Synchrony analysis evaluates dependence between two or more entropy streams.

Implemented in `openentropy_core::synchrony`.

> **What it is:** A cross-stream coupling analysis for shared information and event synchronization.
>
> **Use it for:** Detecting whether two or more streams are influenced by common causes.
>
> **Input shape:** Two streams for pair methods; three or more for global event detection.

## Use this when

- You have 2+ streams and want coupling/synchronization evidence.
- You suspect shared external events affecting multiple sources at once.
- Cross-correlation alone is insufficient and you need MI/coherence/event overlap.

## Methods

- `mutual_information`: shared-information estimate with normalized MI
- `phase_coherence`: binary sign-coherence proxy (not Hilbert phase extraction)
- `cross_sync`: lagged cross-correlation and best lag
- `global_event_detection`: simultaneous outlier events across multiple streams
- `synchrony_analysis`: one-call pairwise orchestrator

## CLI

Requires 2+ sources:

```bash
openentropy analyze clock_jitter sleep_jitter --synchrony
```

## Python SDK

```python
from openentropy import synchrony_analysis, mutual_information, global_event_detection

pair = synchrony_analysis(data_a, data_b)
mi = mutual_information(data_a, data_b)
global_events = global_event_detection([data_a, data_b, data_c])
```

## Rust SDK

```rust
use openentropy_core::{synchrony_analysis, mutual_information, global_event_detection};

let pair = synchrony_analysis(&data_a, &data_b);
let mi = mutual_information(&data_a, &data_b);
let events = global_event_detection(&[&data_a, &data_b, &data_c]);
```

## Related

- [Choose an Analysis Path](/openentropy/concepts/analysis-path/)
- [Cross-Correlation](/openentropy/concepts/analysis-cross-correlation/)
