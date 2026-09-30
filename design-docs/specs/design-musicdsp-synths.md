# MusicDSP synthesis expansion

## Overview

The user extended the active goal on 2026-09-30 to include useful synthesis
from bdejong/musicdsp. Use the same pinned checkout and per-entry notices in
[the source audit](../references/musicdsp-audit.md). Existing Vactr already
provides bandlimited basic oscillators, FM/PM, simple additive synthesis,
wavetable/granular synthesis, colored noise and many Mutable voice families.
Add distinct controls and structures rather than renaming these engines.
No unknown-license snippet, attachment, table or preset is copied. These
are independently implemented mathematical algorithms with research credit.

## Selected kernels

| UGen | Controls | Algorithm and distinction |
|------|----------|---------------------------|
| `dsf-osc` | freq Hz, spacing ratio, rolloff, count | Finite geometrically weighted partial sum, original closed-form expression checked against explicit sum. Independent partial spacing and exponential rolloff exceed existing harmonic additive tilt controls. Exclude partials at/above Nyquist; handle rolloff near one without singularity. |
| `chebyshev-osc` | freq Hz, harmonic-1 through harmonic-8 signed coefficients | Chebyshev recurrence on cosine phase with independently weighted harmonic coefficients, normalized by absolute weight sum. Prune harmonics beyond Nyquist. This is a harmonic oscillator rather than an arbitrary-input distortion claim. |
| `gaussian-noise` | sigma, optional bounded mean | Deterministic seeded Box-Muller normal noise distinct from existing uniform noise. Finite logarithm argument, cached second variate and measurable normal moments. No platform RNG dependency or allocation. |
| `lorenz-osc` | rate, chaos, output-axis | Continuous Lorenz attractor with bounded-step numerical integration and original analytic parameters; chaotic modulation/audio texture, no equal-temperament pitch promise. |
| `rossler-osc` | rate, chaos, output-axis | Continuous Rössler attractor, separately initialized and integrated; distinct from random LFOs and Lorenz. |
| `am-formant-osc` | freq Hz, formant-1/2 Hz, bandwidth-1/2 Hz, balance | Cosine-phase formantic kernel amplitude-modulated by two adjacent harmonic carriers with fractional crossfade. Maintains harmonic pitch while moving spectral envelope. No vowel tables or source data; distinguish from existing formant filtering. |

DSF entries 68/140 and AM entry 224 contain normalization/indexing caveats;
verify equations independently rather than transcribing the posted formulas.
Chaos entry 184 is research only; implement differential equations with a
bounded integration step and deterministic state. Gaussian entries 109/113/168
are references for a normal distribution, not license grants. Chebyshev entry
187 motivates polynomial recurrence.

PADsynth (213) has an explicit public-domain reference but requires a separate
spectral wavetable generation/setup design; it is recorded as deferred and
is not represented by simple detuned additive oscillators in this batch.

## Integration and verification

Append new `UGenSpec` codec tags; preserve all old graph tags and node controls.
Provide typed names, port metadata, implicit instrument controls, lowering,
preallocated state sizing and native/browser codec round trips. No callback
allocation, locks or variable-length work. All touched Rust files stay below
1000 lines. Explicitly bound DSF partial counts, AM kernel harmonics and
chaos integration work per output sample.

Tests establish DSF versus explicit partial sum and Nyquist limiting; signed
Chebyshev harmonic amplitudes; Gaussian mean/variance/kurtosis and repeatability;
chaos nonconstant bounded output and functional controls at 44.1/48/96 kHz;
AM formant pitch/harmonicity and envelope movement. Test extreme controls,
automation, deterministic callback partitions and zero callback allocations.
Add a sample-free `examples/musicdsp-synth-lab.vact` with explicit instruments
and concise controls, plus direct audition commands. Required specialized
Rust test agent runs formatting, full tests, clippy and native/browser builds
after both effects and synths have integrated.

## References

See [MusicDSP source audit](../references/musicdsp-audit.md).

## Audition

```sh
target/debug/vactr run examples/musicdsp-synth-lab.vact --host native --cycles 16
target/debug/vactr run examples/synth-labs/dsf.vact --host native --cycles 16
target/debug/vactr run examples/synth-labs/am-formant.vact --host native --cycles 16
target/debug/vactr run examples/synth-labs/lorenz.vact --host native --cycles 16
```

Each kernel has a standalone file in `examples/synth-labs/`. Lorenz and
Rössler rate controls set simulation speed; they are chaotic textures
and do not track the equal-tempered note pattern. Gaussian noise has an
unclipped normal distribution; the lab uses a conservative sigma and gain.
The existing `stream-lorenz` effect remains separate from these standalone
continuous source UGens.
