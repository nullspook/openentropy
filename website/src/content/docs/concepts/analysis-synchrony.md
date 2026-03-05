---
title: 'Synchrony Analysis'
description: 'Cross-stream dependence and shared-event detection'
---

Synchrony analysis evaluates dependence between two or more entropy streams.

Implemented in `openentropy_core::synchrony`.

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

- [Analysis System](/openentropy/concepts/analysis/)
- [Cross-Correlation](/openentropy/concepts/analysis-cross-correlation/)
