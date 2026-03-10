---
title: 'Quantum Sources'
description: 'Hardware quantum random source integration'
---

Quantum sources provide entropy from explicitly quantum physical processes.

## Sources

- `qcicada` — Crypta Labs QCicada USB QRNG (photonic shot-noise source)

## Operational Notes

- Requires compatible USB hardware.
- Mode controls are available for raw/conditioned/sample output paths.
- OpenEntropy uses QCicada fresh-start continuous mode for collection, discarding already-buffered device input once after mode entry and avoiding reuse of the one-shot `ready_bytes` buffer between reads.
