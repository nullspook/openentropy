---
title: 'Rust Analysis Workflows'
description: 'Task-oriented analysis flows with dispatcher and focused modules'
---

This page maps common Rust analysis tasks to the right API entry points.

## One-call Analysis (recommended)

```rust
use openentropy_core::dispatcher::{analyze, AnalysisProfile};
let report = analyze(&[("source", &data)], &AnalysisProfile::Deep.to_config());
```

Use `AnalysisProfile::Security` for security-focused checks.

## Forensic + Chaos + Trials

```rust
use openentropy_core::{full_analysis, trial_analysis};
use openentropy_core::chaos::chaos_analysis;

let forensic = full_analysis("source", &data);
let chaos = chaos_analysis(&data);
let trials = trial_analysis(&data, &Default::default());
```

## Temporal + Statistics + Synchrony Tiers

```rust
use openentropy_core::{statistics_analysis, temporal_analysis_suite, synchrony_analysis};

let stats = statistics_analysis(&data);
let temporal = temporal_analysis_suite(&data);
let sync = synchrony_analysis(&data_a, &data_b);

println!("Ljung-Box p={:.4}", stats.ljung_box.p_value);
println!("Drift slope={:.4}", temporal.drift.drift_slope);
println!("NMI={:.4}", sync.mutual_info.normalized_mi);
```

## Source-to-source Comparison

```rust
use openentropy_core::compare;
let delta = compare("a", &data_a, "b", &data_b);
```

## Related

- [Analysis System](/openentropy/concepts/analysis/)
- [Rust API Reference](/openentropy/rust-sdk/api/)
