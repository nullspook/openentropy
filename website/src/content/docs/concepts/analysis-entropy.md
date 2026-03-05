---
title: 'Entropy Breakdown'
description: 'NIST-style min-entropy estimators and entropy grade interpretation'
---

The entropy module provides a detailed min-entropy assessment inspired by
NIST SP 800-90B. It runs multiple estimators and reports conservative values.

Implemented in `openentropy_core::conditioning`.

> **What it is:** A conservative, multi-estimator min-entropy assessment.
>
> **Use it for:** Security decisions where entropy density and worst-case estimates matter.
>
> **Input shape:** One byte stream (`bytes` / `&[u8]`).

## Use this when

- You need entropy-density estimates for security decisions.
- You want more than a single Shannon value (MCV/collision/Markov/compression/t-tuple).
- You are comparing sources by conservative min-entropy grade.

## Estimators

| Estimator | Method | Notes |
|-----------|--------|-------|
| Shannon | Information entropy | Classical `H = -sum p log2 p`, max 8 bits/byte |
| MCV | Most common value | Conservative min-entropy estimate |
| Collision | Collision spacing | Repetition-distance based estimate |
| Markov | Transition model | Captures sequential dependence |
| Compression | Universal compression | Pattern recurrence estimate |
| t-Tuple | Tuple frequency | Repeated tuple dominance estimate |

`min_entropy` uses the conservative estimate used by the core report; a
diagnostic floor is also available for additional caution.

## Entropy Grade

| Grade | Min-Entropy | Interpretation |
|:-----:|:-----------:|----------------|
| A | `>= 6.0` | Excellent density for cryptographic seeding |
| B | `>= 4.0` | Good entropy density |
| C | `>= 2.0` | Moderate redundancy present |
| D | `>= 1.0` | High predictability risk |
| F | `< 1.0` | Insufficient entropy |

## How to run it

- `--profile security` for security-focused validation
- `--profile deep` for broad research characterization
- `--entropy` to enable entropy breakdown in custom runs

## Related

- [Choose an Analysis Path](/openentropy/concepts/analysis-path/)
- [Forensic Analysis](/openentropy/concepts/analysis-forensic/)
- [Verdict System](/openentropy/concepts/analysis-verdicts/)
