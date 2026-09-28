# Streams Control Algorithms and Audio Adaptations

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Split FX-003 in `modular-audio-effects.md`. The pinned MIT `streams`
firmware computes gain and frequency CV for external analog VCA/VCF hardware.
Its six processor functions are control algorithms, not firmware audio DSP.
Vactrol should translate the control behavior and pair it with a separately
specified digital gain/filter stage where an audible bus effect is useful.
Do not describe such a stage as a source audio port or claim analog hardware
parity. Source `resources.cc` and generated tables require individual audit;
prefer analytic math over importing generated data.

## Modules

### `src/dsp/effects/dynamic_control.rs`

```rust
pub fn params(kind: EffectKind) -> &'static [ParamDef];
pub fn mem_len(kind: EffectKind, sample_rate: f32, caps: &CapabilitySet) -> usize;
pub fn init(kind: EffectKind, state: &mut FxState, mem: &mut [f32], sample_rate: f32);
pub fn process(kind: EffectKind, state: &mut FxState, mem: &mut [f32],
               left: &mut [f32], right: &mut [f32], values: &[f32], ctx: &mut FxCtx<'_>);
```

Separate small kernel modules may implement each function. The bus path
retains stereo input/output. If source `audio` and `excite` roles cannot both
be supplied by the bus interface, expose a documented sidechain/control
input before claiming that role is covered. All alternate modes, channel
linking, local/global controls, and meaningful attack/release/filter controls
must be addressable in `.vact` and editor metadata. Keep every touched Rust
source under 1000 lines.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| STR-001 | Audit six source modes, parameter mappings, control-rate and analog boundary | FX-003 | Completed |
| STR-002 | Envelope and plucked/damped vactrol control with digital gain/filter adaptation | STR-001 | Completed (adaptation) |
| STR-003 | Follower and compressor controls with stereo digital gain/filter adaptation | STR-001 | Completed (adaptation) |
| STR-004 | Filter controller and Lorenz control generator with explicit audio placement | STR-001 | Completed (adaptation) |
| STR-005 | Independent source-control comparisons and provenance review | STR-002..004 | Not started |
| STR-006 | Native/browser capacity, sidechain, controls and callback-allocation verification | STR-002..005 | Not started |

## Source Map

| Source at pinned revision | Control output | Vactrol scope |
|---|---|---|
| `streams/envelope.{h,cc}` | Gain and frequency envelopes | Control translation plus digital audio adaptation |
| `streams/vactrol.{h,cc}` | Gain and frequency pluck/ring curves | Control translation plus digital audio adaptation |
| `streams/follower.{h,cc}` | Signal level to gain and frequency | Control translation plus digital audio adaptation |
| `streams/compressor.{h,cc}` | Level-dependent gain and reduction | Control translation plus digital audio adaptation |
| `streams/filter_controller.h` | Excitation to frequency CV | Control translation plus digital audio adaptation |
| `streams/lorenz_generator.{h,cc}` | Chaotic gain and frequency control | Control translation plus digital audio adaptation |

## Completion Criteria

- [x] Six published control functions and alternate behavior are mapped with source file and revision; all six have runnable digital adaptations.
- [x] Every implemented control of the six runnable effects is exposed and validated in `.vact` and editor metadata; source-ignored alternate/linked settings on functions 4–5 are documented rather than exposed as placebos.
- [x] Digital audio adaptations are explicitly distinguished from source firmware control code.
- [x] First-slice stereo, sidechain, sample-rate, memory and callback-allocation contracts pass native/browser tests.
- [ ] Generated resources are audited or independently replaced; notices and comparisons are recorded.
- [x] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass for the first slice.

## Progress Log

### Session: 2026-09-28 — filter controller and Lorenz audio placement

`stream-filter` is an original stereo low-pass with neutral gain, driven by
source-role offset and signed nonlinear amount applied to a selected self or
right-sidechain excitation. Vactrol adds scalar excite, cutoff bounds and
mix. Source `FilterController` instead emits gain CV=0 and frequency CV for
analog hardware; its `Configure` ignores alternate/globals.

`stream-lorenz` uses a bounded host-rate Lorenz integration, local rate plus
excitation modulation, and a VCA/VCF depth balance to drive authored stereo
gain and low-pass stages. Left and right use the source's channel-index x/z
role swap; source `Configure` also ignores alternate/globals. Neither effect
exposes these ignored settings as a placebo. Both are manifest
Adaptation/Replacement rows 4–5, completing six runnable role adaptations
without claiming firmware audio DSP or hardware parity. No source rate,
waveform, coefficient or other generated lookup table is imported. Tests
cover signed amount, offset, Lorenz determinism/variation, channel roles,
sidechain, `.vact`/editor, codec, memory rejection preserving the live bus,
native/browser rates/blocks and callback allocations. Exact CV curves,
source processor timing and physical two-pair I/O remain for STR-005/006.

### Session: 2026-09-28 — follower and compressor digital effects

`stream-follower` now uses three independently tracked frequency bands for
level and spectral centroid control; its alternate mode holds digital gain
neutral and maps centroid/frequency control to the digital filter cutoff.
This is a Vactrol filter-only adaptation of the source CV role, not the
source's analog CV remapping. `stream-compressor` uses attack/release RMS
detection, threshold, ratio/makeup amount, hard/soft knee alternates and
source-style linked global substitution. Both run as authored stereo digital
audio effects with optional right-input sidechain detection; source firmware
outputs CV to analog hardware. Their input/output, filters, table-free
coefficients, ratio and knee curves are numerical adaptations. The source
compressor has a five-second sidechain-present detector and falls back to
`audio` when `excite` falls quiet; Vactrol uses immediate quiet-input
fallback. Both effect kinds are appended to the wire ordinal list and rows
2–3 of the six-function manifest are Adaptation/Replacement. Rows 4–5 remain
pending. Focused tests cover every parameter's role, local/global
substitution, modes, sidechain, stereo output, native/browser rate and block
matrix, codec, install preflight and callback allocation.

An independent review caught an inverted alternate mapping in the initial
follower slice (frequency CV to gain with the filter open). The corrected
path fixes gain at unity and retains CV-controlled cutoff; a focused
regression holds both cutoff bounds equal to prove CV cannot alter gain,
then separates them to prove the cutoff responds.

### Session: 2026-09-28 — envelope and vactrol digital effects

The first two source-ordered roles now run as `stream-envelope` and
`stream-vactrol` stereo bus/master effects. A public six-row manifest keeps
firmware gain/frequency CV distinct from Vactrol's original digital gain and
low-pass processing; rows 2–5 remain pending. Each effect exposes local
shape and response, linked global attack/decay, alternate AD/AR or
damped/plucked behavior, independent or shared excitation, optional right
sidechain, scalar excite, trigger/gate, detector threshold, cutoff bounds and
mix. Tests cover audio response, mode distinctions, linked-global
substitution, stereo lanes, `.vact`/editor controls, native/browser codec,
supported rates/blocks, capacity rejection and callback allocation.

The bus has two audio channels rather than the source's two independent
audio/excite pairs. The optional right-channel detector is shared sidechain
control while the right audio output remains independent; separate external
excite for each lane is not available. The digital low-pass/gain stage is
authored for Vactrol and is not the upstream analog hardware. Exact source
CV curves, source lookup numerics and full two-pair I/O remain for STR-005.

### Session: 2026-09-28 — source boundary audit

The pinned `processor.cc` registers six functions. `processor.h` forwards
`audio` and `excite` ADC values to each function and returns `gain` and
`frequency`; `streams.cc` writes those results to DAC/PWM outputs. The
analog VCA/VCF signal path is outside the published firmware. All inspected
source headers carry Emilie Gillet's MIT grant. This plan therefore keeps
source-control fidelity separate from any newly implemented digital audio
effect; neither alone is a full emulation of the physical module.
