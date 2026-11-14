# Plaits Oscillator Modes Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-27
**Last Updated**: 2026-09-30

## Design Document Reference

This plan covers pinned Plaits positions 7–9 and 11, extending
`modular-plaits-engines.md` without exceeding its ten-task limit. Each mode
requires a neutral `.vact` instrument, all sound controls, independently
addressable main and auxiliary outputs, fixed callback state, provenance and
an honest coverage classification. No aggregate `resources.cc`, DX7-derived
patch bank, unaudited wavetable or other audio asset is imported.

## Related Plans

- **Previous**: `impl-plans/active/modular-plaits-engines.md`
- **Depends On**: `modular-audio-foundation.md`, `modular-synth-engines.md`

## Modules

### `src/dsp/ugen/chip_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState,
              mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>);
```

Translate position 7's chord/square main and stepped-triangle bass auxiliary
with clocked arpeggiation versus unclocked chord behavior. Audit the MIT
chord-bank, quantizer, oscillator and arpeggiator dependencies. Some headers
include `resources.h`; verify actual symbol use rather than importing the
aggregate resource file. Preserve authored note, harmonics/chord, timbre/
inversion-or-pattern and morph/shape controls, plus clock/trigger mode.

### `src/dsp/ugen/analog_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState,
              mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>);
```

Translate position 8's pinned variable-shape and variable-saw source stages.
Keep the outputs and oscillator control roles separate from the already
implemented position 0 `filter-voice`.

### `src/dsp/ugen/shape_pair.rs` and `src/dsp/ugen/grain_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState,
              mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>);
```

Position 9 references generated fold curves; audit the MIT generator and
derive any needed transfer analytically instead of importing the aggregate
resource file. Position 11 renders two grainlets and a Z oscillator; VOSIM is commented out;
audit their source and all dependencies before implementation.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| SYN-005A | Position 7 clocked/unclocked chiptune voice, source and asset audit | MOD-003–005 | Bounded adaptation complete; external-clock and BLEP parity pending |
| SYN-005B | Position 8 virtual-analog pair, distinct from position 0 | MOD-003–005 | Bounded adaptation complete; BLEP/interpolation parity pending |
| SYN-005C | Position 9 waveshaping pair with cleared fold math | MOD-001, MOD-003–005 | Bounded adaptation complete; spectral/source parity pending |
| SYN-005D | Position 11 grain-oscillator pair with audited sources | MOD-001, MOD-003–005 | Bounded adaptation complete; BLEP/filter parity pending |
| SYN-005D2 | Translate position 11 grainlet/Z BLEP resets and output high-pass stages | SYN-005D | Completed and independently verified; voice layer PLV-001 is SourceStage (probe A–E pass, `tmp/plv/s209/PLV-40/7-compare.json`); block interpolation remains separate |
| SYN-005E | Source comparison and fidelity classification | SYN-005A–D, MOD-006 | Completed: positions 7/8/9/11 numerically measured; 7 not comparable for clock semantics, 8/9/11 measured gaps |

### SYN-005D2 grain source-stage translation

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, position
11 fidelity paragraph; pinned MIT `grain_engine`, `grainlet_oscillator`,
`z_oscillator`, `stmlib/dsp/polyblep.h` and `filter.h`.

**Deliverables**: Update `src/dsp/ugen/grain_pair.rs` with two-frame
quadratic BLEP corrections for both main grainlets and the auxiliary Z
oscillator, and the source-shaped output one-pole high-pass. Retain
analytic sine, the existing five-port graph and independent main/aux
outputs. Resize fixed state and update install-budget tests if needed;
never import `lut_sine`, `resources.cc`, waveform or other audio asset.

**Completion Criteria**:
- [x] Source reset crossings, pending samples and high-pass stages are present; all sound controls respond independently on main and auxiliary.
- [x] Per-voice state is fixed at install and partition-invariant; native/browser 44.1/48/96 kHz × 64/256, late starts, event reset and zero callback allocations pass.
- [x] `.vact`, editor, codec and channel routing remain valid; coverage and notice disclose analytic sine and source interpolation differences without a full-port claim.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Chiptune | `src/dsp/ugen/chip_pair.rs` | Adaptation complete | Chord/clock, controls, native/browser, state budget |
| Virtual analog | `src/dsp/ugen/analog_pair.rs` | Adaptation complete | Controls, main/aux, native/browser rate/block, budget |
| Waveshaping | `src/dsp/ugen/shape_pair.rs` | Adaptation complete | Controls, main/aux, native/browser, state budget |
| Grain oscillators | `src/dsp/ugen/grain_pair.rs` | Source-stage translation | BLEP resets, controls, main/aux, partition, native/browser, state budget |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Two-output voice, editor, memory bounds | `modular-audio-foundation.md` | Bounded path available |
| Source and generated-resource audit | `MOD-001`, `THIRD_PARTY_NOTICES.md` | Per-engine review required |
| Source reference comparison | `MOD-006` | Pending |

## Completion Criteria

- [ ] All four positions have cleared renderers and accurate coverage states.
- [ ] Every sound control is accessible through `.vact` and editor metadata.
- [ ] Main/aux, clock/trigger, native/browser and memory-limit behavior are tested.
- [ ] Provenance and numerical differences are recorded for every position.
- [ ] Quiet cargo check, strict Clippy, full tests, rustfmt and diff checks pass.

## Progress Log

### Session: 2026-09-29, positions 7/8/9/11 raw-kernel comparison

The shared Plaits oscillator probe now drives positions 7, 8, 9 and 11
directly alongside the positions recorded in `modular-plaits-engines.md`.
It evaluates note 69, 48 kHz, 24-frame blocks, 24 warm-up frames and four
control settings: `(harmonics,timbre,morph,accent,trigger)` of
`(0.1,0.25,0.2,0.8,1)`, `(0.5,0.5,0.5,0.8,1)`,
`(0.9,0.75,0.8,0.8,1)` and `(0.5,0.5,0.5,0.3,0)`. The fixed
Vactr pitch is 441.173337 Hz after conversion from the source note. The
middle setting is tabulated as reference/Vactr RMS, zero-lag to aligned
normalized RMS error, aligned correlation, spectral centroid and estimated
fundamental in Hz. All channel metrics for all four scenarios are emitted
by the JSON probe.

| Position | Main: RMS; NRMSE; r; centroid; f0 | Aux: RMS; NRMSE; r; centroid; f0 | Classification |
|---|---|---|---|
| 7 | 1.000/1.000; 1.415→1.415; unavailable; 23/1040; unavailable/440.9 | .574/.615; 1.465→1.465; -.000; 1817/264; 1764.9/220.7 | Not comparable: source consumes external trigger edges and retains clock state; Vactr substitutes an internal clock/rate. |
| 8 | .466/.526; .662→.621; .849; 805/953; 441.1/441.1 | .737/.572; 1.475→.473; .888; 1483/1774; 882.8/440.9 | Measured gap: variable oscillators and sync-difference spectra differ. |
| 9 | .495/.553; 1.569→1.228; .331; 2166/2098; 441.2/441.2 | .435/.488; 1.832→.915; .634; 3111/2764; 441.1/441.1 | Measured gap: analytic fold/overtone functions and antialiasing differ. |
| 11 | .369/.368; .725→.031; 1.000; 487/486; 441.5/441.5 | .461/.461; 1.326→.194; .981; 3796/3796; 441.1/441.1 | Measured gap: grainlet main aligns closely, while auxiliary Z/high-pass stages retain error. |

The close threshold is aligned normalized RMS error below 0.10 and
correlation at least 0.99 on every scenario and channel. Position 7 still
has numerical measurements but is excluded from like-for-like classification
because its clock controls have no one-to-one mapping. The kernel files and
manifest fidelity labels were unchanged; raw signal comparison does not
validate Plaits voice/LPG or Vactr event behavior; that layer has its own PLV-001 comparison (`tmp/plv/s209/PLV-40/7-compare.json`).

### Session: 2026-09-28, SYN-005D2 grain BLEP and filter stages

Both main grainlets and the auxiliary Z oscillator now carry the source
quadratic two-frame BLEP across reset boundaries; each output uses the
dirty-tangent one-pole high-pass. Analytic sine and per-sample controls
replace the source table and block/crossing-time interpolation. The paired
voice still uses exactly 16 preallocated floats, rejecting a 15-float
install budget. Independent review matched phase crossings, BLEP signs,
pending state and filter recurrence to the pinned MIT source. Dynamic
four-control partition tests, native/browser 44.1/48/96 kHz × 64/256
late-start and zero-allocation tests, `.vact` controls, editor and codec
pass. Quiet native/no-default/wasm checks, strict Clippy, rustfmt, Taplo,
19 mise task validations and full Cargo tests pass (1,440 passed, one
ignored). The manifest records `SourceStage`; source numeric comparison
and voice-layer SourceStage evidence is recorded under PLV-001 (`tmp/plv/s209/PLV-40/7-compare.json`); oscillator parity remains open.

### Session: 2026-09-28, SYN-005C position 9 waveshaping adaptation

Audited the pinned MIT waveshaping engine, slope oscillator, sine helper,
`stmlib` dependencies and generated-table source. The waveshaper generator
labels its curves as borrowed from Tides, so `shape-voice` uses independently
written analytic transfers and imports no generated curve, fold, sine or
aggregate resource data. `.vact` exposes pitch/frequency,
`shape-harmonics` (source harmonics role), timbre/fold drive and morph/slope.
Main is folded variable slope; auxiliary blends sine-like triangle and
folded overtones. The manifest records `Adaptation` with cleared replacement
resources. Two four-float node states are reserved at install and a
7-float budget fails. Focused tests cover all controls, deterministic reset,
independent outputs and native/browser 44.1/48/96 kHz × 64/256-frame
allocation-free rendering.

The transfer curves, spectrum, integrated-BLEP slope correction and exact
block interpolation differ from source. Numerical comparison remains
SYN-005E work.

### Session: 2026-09-28, SYN-005D position 11 grain oscillator adaptation

Split graph-builder names/ports into `src/dsp/build/names.rs`, preserving the
`build` public facade and keeping touched Rust files below 1000 lines.
Audited the pinned MIT grain engine, two grainlet oscillators, Z oscillator,
sine helper and used `stmlib` headers. VOSIM is commented out in source
`Render` and is not claimed. `grain-pair-voice` exposes pitch/frequency,
`grain-harmonics` (source harmonics role), timbre/formant and morph/shape.
Two grainlets form the filtered main path; a Z oscillator forms filtered
auxiliary output. Two eight-float node states are reserved at install;
a 15-float budget fails. Tests cover control response, deterministic reset,
independent outputs and native/browser 44.1/48/96 kHz × 64/256-frame
allocation-free rendering. The manifest records position 11 as `Adaptation`.

Analytic sine replaces the generated `lut_sine` and no external waveform
asset is copied. The source's polyBLEP discontinuity correction, exact
block parameter interpolation and stmlib high-pass coefficients are not
reproduced. These numerical and source-comparison gaps remain SYN-005E work.

### Session: 2026-09-28, SYN-005B position 8 virtual-analog adaptation

The pinned engine header selects `VA_VARIANT 2`; its MIT engine,
variable-shape/saw oscillator headers and used `stmlib` dependencies were
inspected. `analog-pair-voice` exposes pitch/frequency, `analog-detune`
(source harmonics role), timbre/square-and-sync and morph/saw-and-shape via
`.vact` and editor metadata. Main is the normalized variable square/saw mix;
auxiliary is the synchronized variable-shape oscillator difference.
Two eight-float node states are preallocated; a 15-float budget fails graph
installation. Tests cover all control responses, deterministic reset,
distinct finite main/aux and native/browser 44.1/48/96 kHz × 64/256-frame
allocation-free rendering. Position 8 is `Adaptation` in the public manifest.

The five MIT detune interval constants are retained with notice. Vactr
uses analytic oscillator transitions and omits source polyBLEP correction
and exact block parameter interpolation; no generated resource or waveform
table is imported. Upstream numerical comparison remains SYN-005E work.

### Session: 2026-09-27, SYN-005A position 7 chiptune adaptation

Pinned MIT chiptune engine, chord bank, arpeggiator, square/triangle
oscillators and used `stmlib` dependencies were inspected. The NES triangle
header includes `resources.h` but reads no generated resource symbol.
`chip-voice` exposes note/frequency, `chip-chord` (source harmonics role),
timbre/inversion or pattern, morph/shape, `chip-clocked` and the Vactr
extension `chip-rate` in `.vact` and editor metadata. The MIT 11×4 chord
intervals are retained with notice; no waveform/lookup resource is imported.
Five square main voices form an unclocked chord; clocked mode uses one
internally stepped square arpeggio. A 32-step triangle bass is independently
sent to auxiliary output. Two 16-float node states are reserved at install;
31 floats fails explicitly. Tests cover chord/clock/rate response,
deterministic reset, scheduled notes, distinct main/aux and native/browser
44.1/48/96 kHz × 64/256-frame allocation-free rendering.

Position 7 is `Adaptation` in the public manifest. Source external-clock
state persists across triggers, whereas Vactr note events create separate
voices with an internal clock. Oscillator polyBLEP, parameter interpolation,
exact inversion and arpeggiator selection also differ; SYN-005E comparison
remains open.

### Session: 2026-09-27

The pinned chiptune, virtual-analog, waveshaping and grain engine entry points
were inspected. Chiptune's triangle header broadly includes `resources.h`
but its renderer reads no generated wave table. Waveshaping reads generated
fold curves and needs a specific generator audit. No renderer is claimed yet.
