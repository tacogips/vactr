# MusicDSP effects, filters, and resonators

## Overview

The 2026-09-30 user goal extends the verified kick-keyed rumble work with
MusicDSP effects that add useful capabilities, more filters, and more
reverb/delay types. Explicit requests include a parametric resonator, comb
filters, spring reverb, a Blackhole-pedal-inspired spacious reverb, and
shimmer with pitch shifting in its feedback path.

Audit reference: bdejong/musicdsp at
`d7c8525c9b3aa007b34caa682624f4e7a28c7264`. Its Effects and Filters entries
are research references, not a blanket source license. Most snippets have
no explicit license; one Univibe entry states GPL-2.0. Do not copy these
snippets or their attachments into the MIT crate. Implement the mathematical
ideas independently using existing Rust primitives, and credit the research
in the pinned audit and references. No new dependency is required.

Vactr already has compressor/limiter, EQ, simple filters, feedback comb,
delay/ping-pong/multitap, four-line FDN variants, phaser, saturation, pitch
shifting, and modal/Rings adaptations. New kinds must expose distinct
structures or controls instead of additional names for those existing kinds.

## Additions

### Filters and resonators

| Kind | Purpose and controls | Difference from existing kinds |
|------|----------------------|--------------------------------|
| `svf-filter` | TPT state-variable bus filter, cutoff/Q, low/high/band/notch/peak modes | Existing SVF is only a voice kernel |
| `butterworth-filter` | Low/high pass, cutoff and selectable 2/4/6/8-pole order | Existing bus filters are fixed biquads |
| `chebyshev-filter` | Type-I low/high pass, order, passband ripple | Adjustable equiripple response, absent today |
| `ladder-filter` | Bus four-pole resonant ladder with drive | Existing ladder is a synth kernel |
| `allpass-filter` | Second-order unity-magnitude phase section, cutoff/Q | Direct phase-section control, not EQ gain |
| `parametric-resonator` | Four independent frequency, RT60 decay, and gain controls, wet mix | Modal currently derives harmonics from one frequency/spread |
| `feedback-resonator` | Tuned body, nonlinear feedback, damping, distance delay and mix | Interactive distorted body/feedback loop instead of linear horn |
| existing `comb` | Preserve defaults; append feedback/feedforward/allpass modes and damping | True comb variants, no renamed duplicate |

Frequency and decay are physical units. Resonator pole radius must be below
one at every valid setting. Prefer a coupled rotation form for tunable
long resonances to retain stability under automation. An impulse's RT60
requires radius exp(-ln(1000)/(sample_rate*decay)); normalize excitation
so tuning/long decay do not cause unbounded resonant gain. Keep left/right
state separate. Butterworth/Chebyshev filters must implement the declared
response mathematically, with measured frequency-response tests.

### MusicDSP effects

| Kind | Structure and controls | Research |
|------|------------------------|----------|
| `foldback` | Periodic reflection at a threshold, drive/output/mix | Entry 203; existing folds are oscillator-specific |
| `variable-clip` | Continuous soft-to-hard clip, hardness/drive/output/mix | Entry 104; current clip has no hardness |
| `alien-wah` | Complex rotating feedback delay, rate/depth/feedback/delay/stereo phase/mix | Entry 70; distinct from real allpass phaser |
| `dynamic-convolution` | Short damped-sinusoid IR bank chosen per input-sample amplitude, frequency/sweep/decay/drive/mix | Entry 207; existing convolution has one static IR |

For dynamic convolution, preserve the per-input-sample amplitude region
through its entire convolution history. A time-varying biquad or output
waveshaper is not a substitute. Keep a fixed bounded IR bank and history,
preallocated in effect memory; no expensive IR allocation/rebuild in a
callback. Parameter automation may update fixed coefficient tables at
bounded cost, with explicit CPU/memory limits.

### Reverbs and delays

| Kind | Required structure and controls |
|------|---------------------------------|
| `early-reflections` | First-order rectangular-room image-source echoes, room dimensions, source/listener position, wall loss and mix |
| `schroeder-reverb` | Parallel decaying combs and series allpass diffusion, size/RT60/damping/mix |
| `spring-reverb` | Dispersive allpass/waveguide feedback with loss, length/tension/dispersion/decay/drive/mix |
| `space-reverb` | Long, dense modulated FDN with size/decay/damping/modulation, normal versus swelling envelope, freeze and mix |
| `shimmer-reverb` | Diffuse reverb with a real pitch shifter recirculating inside feedback; semitone shift/feedback/damping/size/decay/mix |
| `tape-delay` | Multiple playback heads, filtered nonlinear feedback, time/feedback/tone/drive/wow/mix |
| `diffusion-delay` | Rhythmic feedback echo with allpass dispersion, time/feedback/diffusion/tone/mix |

Spring is an original dispersive model, not a hardware emulation claim.
Space takes the user-visible spacious/swelling/freeze idea from the
Blackhole pedal; no Eventide DSP, presets or trademark name is shipped.
Gravity-style negative settings must audibly/measurably swell; do not
claim realtime reversal of future audio. Shimmer must shift the tail in
the feedback loop, not only add a post-reverb pitch-shift unit. Use bounded
feedback and an energy-preserving FDN matrix. Freeze disables new injection
and damping/decay losses while retaining a bounded recirculating tail.

## Integration contracts

- Every new kind runs in bus and instrument effect position with metadata
  and graph-codec round trips on native and browser tiers.
- Keep every existing effect index stable: append new kinds after the
  existing complete ALL list, including analyzers and texture entries.
- Existing comb defaults and parameter indices remain unchanged.
- Missing required delay/state memory rejects installation before
  retiring a live chain; no zero-memory substitute or reduced algorithm.
- Required memory is allocated at graph setup; callbacks allocate and lock
  nothing. Typed metadata exposes units and bounded controls.
- Feedback/delay automation must not read uninitialized history or create
  nonfinite output. Test sample rates 44.1/48/96 kHz and callback partitions.
- Avoid global FxState expansion when state can live in a kind's own
  preallocated memory. Keep each touched Rust source below 1000 lines.

## Verification and examples

DSP evidence must establish advertised frequency response, impulse/decay
behavior, stereo isolation, pitch ratios, diffuse/swelling tails and
functional control changes. Test extreme valid controls and automation,
short-region rejection preserving live graphs, zero callback allocations,
metadata/codec/instrument paths, and meaningful defaults.

Provide `examples/filter-lab.vact`, `examples/resonator-lab.vact`, and
`examples/reverb-delay-lab.vact`: evaluable, sample-free comparisons using
a bounded number of simultaneous buses. Provide direct commands for
playing each effect variant. Keep `techno-pattern.vact` unchanged. Add
a useful resonator or spring treatment to industrial metal, and a quiet
space/shimmer texture only after balance is verified; preserve ducking.

## References

See `../references/musicdsp-audit.md` and `../references/README.md`.

## Audition commands

Each effect has its own sample-free file in `examples/effect-labs/`; only one
persistent effect bus is active, retaining the full tail between triggers.
The dry reference is panned left and the treatment right. For a centered
comparison set both `pan` values to 0.5. Allpass uses parallel dry/wet mixing
to make its phase cancellation audible; unity magnitude alone is not louder.

```sh
target/debug/vactr run examples/reverb-delay-lab.vact --host native --cycles 16
target/debug/vactr run examples/effect-labs/space-reverb.vact --host native --cycles 16
target/debug/vactr run examples/effect-labs/shimmer-reverb.vact --host native --cycles 16
target/debug/vactr run examples/filter-lab.vact --host native --cycles 16
target/debug/vactr run examples/resonator-lab.vact --host native --cycles 16
```

Space gravity values below zero produce a causal swelling tail. Re-evaluate
the bus with `freeze: 1` after a tail has accumulated to hold it; re-evaluate
with `freeze: 0` to release. Shimmer `shift: 12` raises the recirculating tail
one octave; negative shifts descend. Additional files cover every new kind
and all three extended comb modes.

## State budget

Full-state admission rejects undersized graphs before replacing a live chain.
Production defaults now reserve four sample-rate seconds of floats per voice
and ten per bus, covering one full new reverb per voice and useful bus chains.
At 48 kHz and the native 256-voice capacity, voice buffers reserve approximately
197 MB; the browser 64-voice capacity reserves approximately 49 MB. Six bus
buffers add approximately 11.5 MB plus existing capture allowance. At 192 kHz
these float-buffer costs quadruple. Embedders can reduce capacities or budgets
explicitly; an insufficient graph is rejected rather than replaced by a reduced
algorithm. The existing two-million-float state ceiling is retained.

## Filter mode controls

| Effect | Numeric mode |
|--------|--------------|
| `svf-filter` | 0 lowpass, 1 highpass, 2 constant-peak bandpass, 3 notch, 4 low-minus-high peak |
| `butterworth-filter`, `chebyshev-filter` | 0 lowpass, 1 highpass; order rounds to 2/4/6/8 |
| `comb` | 0 feedback, 1 feedforward, 2 allpass; original time/feedback/mix parameter order retained |

Parametric-resonator frequencies use Hz, decay controls are RT60 seconds,
and mode gains are dB. These four modes are independently tunable. The
existing derived-harmonic modal and Rings banks remain available separately.
