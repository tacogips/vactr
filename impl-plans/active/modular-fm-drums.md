# FM Percussion Ports Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-27
**Last Updated**: 2026-09-28

## Design Document Reference

Deliver triggered FM percussion from the MIT-licensed Peaks DSP, then a
separate FM/noise/feedback percussion voice after auditing the Faust source
and its library dependencies. Use Vactr product names and controls.

## Modules

### `src/dsp/ugen/fm_drum.rs`

```rust
pub struct FmDrumState {
    pub phase: f32,
    pub pitch_env: f32,
    pub fm_env: f32,
    pub amp_env: f32,
    pub feedback: f32,
}
pub fn render_fm_drum(
    state: &mut FmDrumState,
    controls: &[f32],
    output: &mut [f32],
    sample_rate: f32,
);
```

The specific state may be folded into the existing `NodeState`. The kernel
must reset on event trigger, render finite output without callback allocation,
and respond to frequency, FM amount, pitch sweep, decay, noise and drive.

### `src/dsp/ugen/catalog.rs`, `src/dsp/meta.rs`, `src/prelude/templates.vact`

```rust
pub fn fm_drum_ports() -> &'static [Port];
pub fn fm_drum_editor() -> EditorDecl;
```

Wire the kernel into graph lowering, native/browser encoding, a neutral
template name, and editor/type metadata. The second voice gets its own
algorithm and parameter list only after source and dependency rights pass.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| FMD-001 | Verify Peaks source/dependencies and record notices | none | Completed for first voice |
| FMD-002 | Implement analytic FM drum kernel and all sound parameters | FMD-001, MOD-003 | Completed for first voice |
| FMD-003 | Expose first FM drum through `.vact`, metadata, and native/browser rendering | FMD-002 | Completed for first voice |
| FMD-004 | Audit separate Faust percussion voice and standard-library rights | none | Completed; no Faust library code used |
| FMD-005 | Independently implement second FM/noise/feedback voice | FMD-004, MOD-003 | Completed as `:fusion-drum` |
| FMD-006 | Integration, parameter-response, allocation, and parity tests | FMD-003, FMD-005 | Integration, parameter response and allocation pass; a numeric `compare-peaks-drums` upstream-parity probe now exists and ran (measured gaps, no full parity claimed) |
| FMD-007 | Design original EFM-inspired voice family from thesis signal-flow research without copying protected expression | FMD-001, MOD-003 | First bounded member implemented as `feedback-metal-drum`; broader family pending |
| FMD-008 | Translate Peaks FM drum signal stages with analytic wave/curve replacements and separate controls | FMD-002, FMD-006 | Completed and independently verified; a source numerical comparison now runs under FMD-006 (`compare-peaks-drums`) |

### FMD-008 source-stage fidelity pass

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Peaks FM
drum fidelity paragraph; pinned `peaks/drums/fm_drum.{h,cc}` and individually
MIT-noticed `peaks/resources/{lookup_tables,waveforms}.py`.

**Deliverables**: Update `src/dsp/ugen/fm_drum.rs` to use a single sine phase,
FM and auxiliary pitch envelopes, amplitude decay, delayed sample feedback,
noise mix and soft overdrive. Preserve the six existing `.vact` ports, event
reset, graph/wire compatibility, deterministic seed and preallocated state.
Represent source-generated curves with analytic functions, not copied tables.
Update Peaks coverage and notices to the verified fidelity level.

**Completion Criteria**:
- [x] Every named control changes its documented sound role; independent noise and drive can each reach the full authored range.
- [x] The source signal stages are present with deterministic event reset and stable state at 44.1/48/96 kHz, 64/256 frames and late starts, without callback allocation.
- [x] Native/browser codec, `.vact` and editor tests pass; no waveform/preset/lookup array is imported or full source parity claimed.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Analytic Peaks FM drum signal stages | `src/dsp/ugen/fm_drum.rs` | Source-stage translation | End-to-end response, partition and host matrix |
| Graph and metadata | `src/dsp/graph.rs`, `src/dsp/ugen/catalog.rs`, `src/dsp/meta.rs` | Implemented for first voice | Catalog and end-to-end |
| Prelude and tests | `src/prelude/templates.vact`, `src/host/tests/e2e/` | Implemented for `:phase-drum` | End-to-end |
| Three-component percussion | `src/dsp/ugen/fusion_drum.rs` | Implemented as `:fusion-drum` | End-to-end and browser codec |
| Original coupled-FM percussion | `src/dsp/ugen/feedback_metal.rs` | Implemented as `:feedback-metal-drum`; research reference only | Control response, deterministic extremes, native/browser |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Declared FM drum control routing | `modular-audio-foundation.md` MOD-003 | Satisfied by typed instrument parameters |
| Second voice | Faust provenance audit | Satisfied through independently written primitives |

## Completion Criteria

- [x] Two distinct FM percussion voices render finite, audible sound.
- [x] Every declared parameter is addressable in `.vact` and editor metadata.
- [x] MIT notices are retained for any ported source.
- [x] No uncleared preset, wavetable or sample is included.
- [x] Native/browser and live-parameter tests pass.
- [x] `CARGO_TERM_QUIET=true` cargo check, clippy and tests pass.

## Progress Log

### Session: 2026-09-29 (FMD-006 upstream-parity comparison probe)

The upstream-parity part of FMD-006 is now covered by an opt-in
`mise run compare-peaks-drums` task, shared with `modular-peaks-functions.md`
PKS-002 since it also compares the bass/snare/hat drums.
`verification/peaks_drums_reference.cc` triggers the pinned `peaks/drums/
fm_drum.{h,cc}` once at the source's own 48 kHz rate (compiled locally
against the aggregate `peaks/resources.cc`, never linked into Vactr) and
`examples/peaks_drums_reference.rs` drives `fm_drum::render` directly with
the matching Vactr controls; `verification/compare_peaks_drums.py` reports
RMS, peak, correlation and normalized RMS error after onset alignment,
spectral centroid, and -20/-40 dB decay times for three scenarios (`clean`,
`noisy`, `driven`) that span frequency, `fm-amount`, decay, and the
source's single dual-purpose `noise` parameter (mapped to Vactr's separate
`drum-noise`/`drive` at each end and their shared midpoint boundary).
`pitch-sweep` has no source counterpart and stays at its 0.5 default in
every scenario.

Measured results: RMS tracks within about 20% on `clean`/`noisy` and
crosses over on `driven` (source 0.315 vs. Vactr 0.255); correlation after
onset alignment is low (0.307/-0.037/0.048); spectral centroid is close on
`clean` (202 vs. 197 Hz) and within about 25-5% on `noisy`/`driven`; -20 dB
decay matches exactly on `noisy` (0.215 s both) and is within 1.4x
elsewhere; -40 dB decay is within 1.5x on every scenario. The `clean`
scenario also shows the source's 3-sample excitation-delay onset that
Vactr's kernel starts immediately, a known, already-documented difference
(`THIRD_PARTY_NOTICES.md` notes trigger-phase and block-tail pitch
alignment differ). Full metrics and the mapping/unit-conversion notes are
recorded in `modular-peaks-functions.md`'s 2026-09-29 session and in
`verification/compare_peaks_drums.py`'s module docstring. These are
measured gaps, not full source parity; `fm-drum`'s manifest row stays
`SourceStage`, not `SourcePort`. No kernel change was made: the gaps are
consistent with the already-documented analytic/floating-point and
host-rate-timing differences rather than an implementation bug.
`CARGO_TERM_QUIET=true cargo fmt --check`, `cargo check -q`, strict
Clippy, `cargo check -q --target wasm32-unknown-unknown --lib`,
`cargo nextest run`, `mise run compare-peaks-drums`, `mise tasks validate`
and `mise run audit-upstream` (219 files, zero errors) pass.

### Session: 2026-09-28 (FMD-008 Peaks FM drum stages)

`phase-drum` now uses one analytic sine phase, separate FM and auxiliary
pitch envelopes, event-local four-sample pitch updates with previous-sample
feedback, amplitude decay, noise mix and tanh drive. All six existing ports
remain separately addressable in `.vact`; no generated Peaks waveform,
lookup, pitch table or preset map was imported. The public Peaks row is
`SourceStage`/`Replacement`, not `SourcePort`. Independent review verified
source stage order, state isolation, variable-buffer control changes and
exact callback partition output. Native/browser 44.1/48/96 kHz × 64/256
late-start and allocation tests, two-hit, editor and end-to-end control tests
pass. Quiet native/no-default/wasm checks, strict Clippy, rustfmt, Taplo,
19 mise task validations and full Cargo tests pass (1,434 passed, one
ignored). Analytic/float curves, trigger phase, source block-tail pitch
alignment and Vactr's separate controls still differ numerically; a
pinned source comparison is pending.

### Session: 2026-09-27

**Tasks Completed**: Located Peaks FM drum source, separate Faust percussion candidate, and Erik Larsson's EFM thesis; excluded unlicensed `md-drum-synth` code. Implemented `:phase-drum` with analytic sine FM, pitch and FM envelopes, noise and drive. Recorded Peaks attribution in `THIRD_PARTY_NOTICES.md`. End-to-end tests verify audible, finite rendering, zero callback allocations, and response to every sound control.
**Tasks In Progress**: Broader Mutable Instruments coverage and upstream parity comparison.
**Blockers**: Full source-engine parity needs a separate comparison harness; the second voice intentionally uses new Rust primitives because Faust's `fi.resonbp` carries a separate STK-4.3 notice and manual translation of the Faust library has no demonstrated MIT-compatible grant.

### Session: 2026-09-27 (second percussion voice)

**Tasks Completed**: Audited the MIT-declared `fbnfm_drumvoice.dsp` and exact Faust library calls. Implemented `:fusion-drum` with feedback noise, filtered noise, and sine FM as separate graph nodes. Exposed 25 component controls plus event velocity, note frequency, and overall amplitude. Expanded graph ports to twelve and event controls to 32 to allow all controls on one hit. End-to-end tests cover every component control, velocity, note, gain, and combined routing; browser-tier rendering and codec round-trip tests cover the new nodes. `CARGO_TERM_QUIET=true` cargo check, strict clippy, and full cargo test pass (994 library tests). Recorded source attribution and the Faust library license boundary in `THIRD_PARTY_NOTICES.md`.
**Tasks In Progress**: Deterministic comparison against an upstream build, if available under compatible terms.
**Blockers**: None for this independently implemented second voice.

### Session: 2026-09-28 (original coupled-FM percussion)

**Tasks Completed**: Audited an accessible scan of Erik Larsson's EFM thesis
and the official Elektron manual as research references. Added a separate
coupled two-operator FM hit with decaying self-feedback, pitch sweep,
deterministic filtered noise injected into the FM network, independent
body/modulator/noise envelopes, resonant filter and bounded drive. Its neutral
`.vact` template exposes note frequency, velocity, thirteen authored sound
parameters and overall gain; the editor, graph codec and native/browser host
recognize the same controls. No thesis assembly, diagram, text, numeric array,
source waveform, sample, preset or firmware is used.
**Tasks In Progress**: Additional original family members and any independent
aural comparison, if desired; no source or hardware parity claim.
**Blockers**: None for this first member. The thesis and manual are not
software or asset licenses, so any future members must retain the same
independent-implementation boundary.
