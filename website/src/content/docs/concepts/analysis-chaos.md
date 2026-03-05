---
title: 'Chaos Theory Analysis'
description: 'Distinguishing deterministic structure from random behavior'
---

Chaos analysis helps distinguish genuinely random behavior from deterministic
systems that only look random.

Implemented in `openentropy_core::chaos` and `openentropy_core::analysis`.

> **What it is:** A structure/complexity analysis that checks for deterministic dynamics in random-looking data.
>
> **Use it for:** Research characterization when forensic/entropy checks are not enough to explain behavior.
>
> **Input shape:** One byte stream (`bytes` / `&[u8]`).

## Use this when

- You are doing research characterization, not just pass/fail validation.
- You need to separate random-looking output from structured/chaotic dynamics.
- You want complexity metrics beyond baseline forensic tests.

## Tiers

- Core tier (`--chaos`): Hurst, Lyapunov, correlation dimension, BiEntropy, epiplexity
- Extended tier (`--chaos-extended`): Sample entropy, Approximate entropy, DFA, RQA,
  rolling/bootstrap Hurst, permutation entropy, Anderson-Darling

Implementation note by module:

- `openentropy_core::chaos`: Hurst, Lyapunov, correlation dimension, BiEntropy, epiplexity,
  Sample entropy, DFA, RQA, rolling Hurst, bootstrap Hurst
- `openentropy_core::analysis`: Approximate entropy, permutation entropy, Anderson-Darling

## Hurst Exponent

Measures long-range dependence (R/S analysis).

- `H ~= 0.5`: random-walk-like
- `H > 0.5`: persistent trend behavior
- `H < 0.5`: anti-persistent behavior

## Lyapunov Exponent

Measures sensitivity to initial conditions.

- `lambda ~= 0`: no clear deterministic chaos signature
- `lambda > 0`: chaotic divergence
- `lambda < 0`: convergent behavior

## Correlation Dimension

Measures attractor dimensionality.

- High `D2` suggests high-dimensional/random-like behavior
- Low `D2` can indicate deterministic low-dimensional structure

## BiEntropy

Measures entropy persistence through derivative levels of the bitstream.

- Higher values indicate stronger disorder and less structure

## Epiplexity

Compression-ratio complexity metric.

- Ratio near `1.0` indicates incompressible/random-like data
- Lower ratios imply compressible structure

## Extended Methods

- **Sample entropy (`sample_entropy`)**: irregularity/complexity estimator (SampEn)
- **Approximate entropy (`approximate_entropy`)**: ApEn regularity metric
- **DFA (`dfa`)**: long-range correlation estimate via detrended fluctuations
- **RQA (`rqa`)**: recurrence structure and determinism metrics
- **Rolling Hurst (`rolling_hurst`)**: local H estimate across windows
- **Bootstrap Hurst (`bootstrap_hurst`)**: uncertainty intervals and surrogate p-value
- **Permutation entropy (`permutation_entropy`)**: ordinal-pattern complexity
- **Anderson-Darling (`anderson_darling`)**: distribution conformity test used in extended tier

## Related

- [Choose an Analysis Path](/openentropy/concepts/analysis-path/)
- [Verdict System](/openentropy/concepts/analysis-verdicts/)
