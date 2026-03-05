---
title: 'Cross-Correlation'
description: 'Inter-source independence checks using pairwise correlation'
---

Cross-correlation evaluates dependence between multiple entropy sources.

Implemented in `openentropy_core::analysis` via
`cross_correlation_matrix()`.

> **What it is:** A pairwise dependence matrix across multiple sources.
>
> **Use it for:** Independence screening before combining sources in a pool or extractor.
>
> **Input shape:** Two or more named byte streams.

## Use this when

- You are combining multiple sources and want an independence sanity check.
- You suspect shared hardware paths or environmental coupling.
- You need to identify source pairs that should not be treated as independent.

## What It Computes

- Pairwise Pearson correlation for all source pairs
- Flag count for pairs whose absolute correlation exceeds threshold

| Metric | Description |
|--------|-------------|
| `pairs` | Correlation coefficient for each source pair |
| `flagged_count` | Number of pairs with `|r| > 0.3` |

## Interpretation

Independent sources should show low pairwise correlation. Elevated
cross-correlation can indicate shared hardware pathways, scheduling
coupling, or environmental coupling that reduces effective independence.

## Usage Notes

- Requires two or more sources
- Enabled by `--profile deep` or `--cross-correlation`

## Related

- [Choose an Analysis Path](/openentropy/concepts/analysis-path/)
- [Forensic Analysis](/openentropy/concepts/analysis-forensic/)
