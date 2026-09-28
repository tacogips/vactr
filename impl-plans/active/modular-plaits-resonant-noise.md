# Plaits Resonant and Noise Engines Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-27
**Last Updated**: 2026-09-27

## Design Document Reference

This continues the Plaits engine coverage after `modular-plaits-engines.md`,
which has reached its ten-task limit. Implement positions 18–20 from the pinned
MIT source with neutral Vactrol names, all exposed controls, and separately
addressable main and auxiliary audio. A source-stage translation remains
distinct from a verified source port. No generated waveform, ROM or patch
asset is eligible without an individual provenance audit.

## Related Plans

- **Previous**: `impl-plans/active/modular-plaits-engines.md`
- **Depends On**: `modular-audio-foundation.md`, `modular-synth-engines.md`

## Modules

### `src/dsp/ugen/particle_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
);
```

Use six independently seeded particle filters, an impulse auxiliary path,
and a filtered/diffused main path. The pinned source uses an 8192-word delay
buffer; preserve a bounded install-time memory contract and document any
diffuser or host-rate difference. A two-output voice must fail installation
cleanly if its requested state exceeds the host budget.

### `src/dsp/ugen/string_pair.rs` and `src/dsp/ugen/modal_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(
    ins: &[Inp<'_>; MAX_PORTS],
    st: &mut NodeState,
    mem: &mut [f32],
    out: &mut [f32],
    kx: &Kx<'_>,
);
```

Before coding either engine, audit every source dependency and any generated
resource separately. Model trigger and resonator state without callback
allocation. Preserve main and auxiliary signal roles and all note,
harmonics, timbre and morph controls.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| SYN-004A | Audit and implement position 18 six-particle voice, with distinct impulse and filtered outputs | MOD-003–005 | Bounded adaptation complete; source diffuser/filter parity pending |
| SYN-004B | Audit and implement position 19 string voice with both outputs | SYN-004A | Bounded adaptation complete; persistent rotation and DSP parity pending |
| SYN-004C | Audit and implement position 20 modal voice with both outputs | SYN-004A | Source-stage translation complete; numerical parity pending |
| SYN-004D | Compare implementations against pinned source behavior and update fidelity metadata | SYN-004A–C, MOD-006 | Not started |

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Particle kernel and graph | `src/dsp/ugen/particle_pair.rs` | Adaptation complete | Native/browser rate/block, controls, reset, budget |
| String kernel and graph | `src/dsp/ugen/string_pair.rs` | Adaptation complete | Controls, scheduled events, native/browser rate/block and budget |
| Modal kernel and graph | `src/dsp/ugen/modal_pair.rs` | Source-stage translation complete | Controls, retrigger, native/browser rate/block and budget |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Two-output voice and editor metadata | `modular-audio-foundation.md` | Bounded path available |
| Provenance and asset audit | `THIRD_PARTY_NOTICES.md`, `MOD-001` | Per-engine review required |
| Full source fidelity | `MOD-006` reference comparisons | Pending |

## Completion Criteria

- [x] All three positions have cleared renderers and accurate coverage states (18 and 19 adaptations; 20 source-stage translation).
- [x] All sound controls are exposed through `.vact` and editor metadata for these bounded implementations.
- [x] Main and auxiliary outputs, triggers, memory limits and native/browser rendering are tested.
- [x] Source and numerical differences and notices are recorded; comparison remains under SYN-004D.
- [x] Quiet cargo check, strict Clippy, tests, rustfmt and diff checks pass for the bounded 18–20 implementations.

## Progress Log

### Session: 2026-09-27, SYN-004B position 19 three-string adaptation

Pinned MIT string engine, voice, string and delay-line sources and used
`stmlib` dependencies were inspected. `string-voice` exposes note/frequency,
`string-structure` (the source harmonics role), timbre/brightness,
morph/damping, event velocity/accent and explicit `string-sustain` through
`.vact` and editor metadata. It preallocates three 2048-sample delay lines,
three 512-sample stretch lines and scalar state per output node: 7,716
floats/node or 15,432 for the dual-output graph. The normal 0.5-second host
voice budget supports this graph at 44.1/48/96 kHz; a one-float-short budget
fails installation. Native/browser graph codecs, zero-allocation renders,
distinct main/aux and 64/256-frame tests pass. Two scheduled hits create
two playable voices. Position 19 moves to `Adaptation` in the manifest.

This is not source-stage-complete: Vactrol's per-event voice lifecycle cannot
reproduce the source's persistent rotation and pitch-history transfer among
three shared strings. It uses linear rather than Hermite interpolation,
one-pole rather than source SVF excitation/damping, simplified bridge and
dispersion stretch processing, and seeded per-voice rather than global RNG.
The fixed 2048-sample main delay also clamps low-note tuning, especially at
96 kHz (roughly below 47 Hz before correction); the rate matrix checks
audibility and finiteness, not low-note pitch accuracy.
The MIT SVF-shift expression is evaluated analytically, with no generated
resource table. Exact source comparison remains open under SYN-004D.

### Session: 2026-09-27, SYN-004C position 20 modal voice

The pinned MIT modal engine, voice, resonator, dust generator and used
`stmlib` dependencies were inspected. `modal-voice` exposes note/frequency,
`modal-structure` (the source harmonics role), timbre/brightness,
morph/damping, event velocity/accent and explicit `modal-sustain` through
`.vact` and editor metadata. It renders 24 bounded resonator modes and a
filtered strike or dust exciter, with resonator main and filtered-excitation
auxiliary outputs. Two fixed 48-float node states are budgeted at install;
tests cover a 95-float rejection, scheduled retrigger, native/browser graph
codec and finite distinct output at 44.1/48/96 kHz × 64/256-frame blocks.
Position 20 is `SourceStage` in the public manifest, not a verified full
port. The MIT stiffness generator is evaluated analytically without loading
aggregate `resources.cc`; this differs from interpolation over its 65-entry
table. Exact tangent, analytic cosine, per-voice RNG, host-rate coefficients
and absent block harmonics smoothing are additional numerical differences.
Upstream audio comparison remains open under SYN-004D.

### Session: 2026-09-27, SYN-004A position 18 particle adaptation

The pinned particle engine, six-particle filter, diffuser and used MIT
`stmlib` dependencies were inspected. `particle-voice` exposes frequency,
`particle-spread` (the source harmonics role), density/timbre and
diffusion-or-Q/morph through `.vact` and editor metadata. Independent main
resonance and auxiliary impulse nodes use fixed, installation-budgeted state;
each event starts a deterministic six-impulse burst. Native and browser graph
codecs and 44.1/48/96 kHz × 64/256-frame rendering are tested without
callback allocation. Position 18 moves from Pending to Adaptation in the
public manifest. The source's seven-stage 8192-word granular diffuser,
dirty-frequency SVF, global RNG and exact per-block impulse scheduling are
not reproduced; Vactrol uses a two-line analytic diffuser, TPT filters and
per-voice RNG. Source comparison and exact numerical parity remain open.

### Session: 2026-09-27

The pinned MIT particle source and dependencies were inspected. The engine
contains six impulse-driven resonant filters, a post-filter and an 8192-word
granular diffuser. Position 18 has no identified generated audio asset, but
its dependency audit and implementation remain open.
