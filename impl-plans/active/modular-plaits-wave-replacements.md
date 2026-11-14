# Plaits Wave and String Modes Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

This plan covers pinned Plaits positions 5, 6, 13 and 14. The user requires
original wave content rather than reusing an upstream wavetable. Positions
5, 13 and 14 read `wav_integrated_waves`, which is generated from an input
whose individual wave origins remain unaudited. Their renderers may be
implemented with new procedural wave functions, but must then be labeled
resource substitutions/adaptations, with audible differences disclosed.
Position 6 is separately audited: its broad `resources.h` includes do not
access `wav_integrated_waves` or `waves.bin`, but `ensemble.h` indirectly reads `lut_sine`
through `SineRaw`. The analytic replacement clears that table dependency.

## Related Plans

- **Previous**: `impl-plans/active/modular-plaits-oscillators.md`
- **Depends On**: `modular-audio-foundation.md`, `modular-synth-engines.md`

## Modules

### `src/dsp/ugen/terrain_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState,
              mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>);
```

Position 5 has five analytic source terrains and three modes derived from
integrated wave data. Keep the trajectory, timbre/radius, harmonics/terrain
selection and morph/offset, with new procedural terrains replacing those
three resource modes. The user-terrain input requires a separate clear data
contract; never silently load upstream data.

### `src/dsp/ugen/string_machine_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState,
              mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>);
```

Position 6 uses divide-down chord oscillators, filtering and ensemble;
complete a symbol-level dependency audit before removing the provisional
wave-asset flag in the public coverage manifest. Preserve left/right
ensemble outputs and note, harmonics/chord, timbre/filter and morph/
registration controls.

### `src/dsp/ugen/table_terrain_pair.rs` and `src/dsp/ugen/chord_pair.rs`

```rust
pub const STATE_FLOATS: usize;
pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState,
              mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>);
```

Position 13 scans banks of integrated waves and position 14 blends a chord
string oscillator with 15 selected waves. Use documented original procedural
wave families, with parameter roles and main/aux routing preserved; do not
copy the generated array or input `waves.bin`.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| SYN-006A | Position 6 divide-down string machine, full source-symbol audit | MOD-003–005 | Bounded adaptation complete; source fidelity comparison pending |
| SYN-006B | Position 5 terrain voice with original replacements for 3 wave-data modes | MOD-001, MOD-003–005 | Bounded adaptation complete; user-terrain and source parity pending |
| SYN-006C | Position 13 original-wave bank scanner | MOD-001, MOD-003–005 | Bounded adaptation complete; custom map and source parity pending |
| SYN-006D | Position 14 divide-down chord plus original-wave layer | MOD-001, MOD-003–005 | Bounded adaptation complete; source parity pending |
| SYN-006E | Source comparison and resource-substitution coverage review | SYN-006A–D, MOD-006 | Not started |

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| String machine | `src/dsp/ugen/string_machine_pair.rs` | Adaptation complete | Controls, stereo, native/browser, state budget |
| Wave terrain | `src/dsp/ugen/terrain_pair.rs` | Adaptation complete | Eight defaults, controls, main/aux, native/browser, budget |
| Wave bank | `src/dsp/ugen/table_terrain_pair.rs` | Adaptation complete | 8×8×3 cells, controls, main/aux, native/browser, budget |
| Chord wave layer | `src/dsp/ugen/chord_pair.rs` | Adaptation complete | Chord/inversion, 15 timbres, main/aux, native/browser, budget |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Two-output voice and fixed install memory | `modular-audio-foundation.md` | Bounded path available |
| Individual original-wave provenance | `MOD-001`, `THIRD_PARTY_NOTICES.md` | Original generation required |
| Full source comparison | `MOD-006` | Pending |

## Completion Criteria

- [ ] All four positions have renderers and accurate per-position resource/fidelity states.
- [ ] Original wave functions contain no copied upstream or LXR wavetable data.
- [ ] Every sound control, stereo/aux role, rate and memory limit is tested.
- [ ] License notices and numerical differences are documented.
- [ ] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass.

## Progress Log

### Session: 2026-09-28, SYN-006D position 14 chord-layer adaptation

Audited the pinned MIT chord engine, chord bank, divide-down and integrated-
wave oscillator dependencies. The source selects fifteen waves from
`wav_integrated_waves` derived from unaudited `waves.bin`.
`chord-layer-voice` replaces that layer with fifteen original analytic
timbres; no upstream wave or generated table is imported. The manifest
records `Adaptation`/`ResourceState::Replacement`, retains
`ResourceFlags::WAVES` solely for source provenance, and now has no
`Pending` Plaits positions. Four source chord notes rotate across five
voice slots for inversion crossfade. The main sums the full chord and
auxiliary emphasizes inversion-selected notes; source-like auxiliary root
selection can make chord-type changes leave aux unchanged.

`.vact` exposes note/frequency, `layer-chord` (harmonics role),
timbre/inversion and morph/registration-to-wave blend. Two 16-float node
states are reserved at install and a 31-float budget fails. Tests cover
fifteen timbres, chord/inversion, control response, deterministic reset,
independent outputs and native/browser 44.1/48/96 kHz × 64/256-frame
allocation-free rendering. Source BLEP, precise wave spectra, registration
and block interpolation remain SYN-006E fidelity work.

### Session: 2026-09-28, SYN-006C position 13 procedural wave-grid adaptation

Audited the pinned MIT wavetable engine, interpolator interface and used
`stmlib` dependencies. The source reads `wav_integrated_waves` from
unaudited `waves.bin`, so `wave-grid-voice` substitutes three original
analytic wave families evaluated over 8×8 grid positions. No source wave
sample, aggregate resource or generated table is imported. The manifest
records `Adaptation`/`ResourceState::Replacement` and retains
`ResourceFlags::WAVES` solely for upstream provenance. Custom/user wave
maps remain unavailable without a cleared user-data contract.

`.vact` exposes note/frequency, `wave-bank` (source harmonics/z role),
timbre/x and morph/y. The 0–7 z scan mirrors into three analytic families,
with separate continuous main and source-style 1/32-truncated auxiliary.
Two eight-float node states are reserved at install; a 15-float budget
fails. Tests cover all 192 grid cells, mirrored bank mapping, axis
responses, quantization, control routing, reset and distinct outputs,
native/browser 44.1/48/96 kHz × 64/256-frame allocation-free rendering.
Original spectra, source integrated-wave differentiation and exact control
smoothing remain SYN-006E fidelity work.

### Session: 2026-09-28, SYN-006B position 5 wave-terrain adaptation

Audited the pinned MIT terrain engine, sine oscillator, wavetable interface
and used dependencies. Source modes 0-4 evaluate analytic terrain formulas,
while modes 5-7 actually read `wav_integrated_waves` generated from
unaudited `waves.bin`; mode 8 expects a user-supplied 64×64 terrain.
`terrain-voice` implements the first five analytic roles and three new
procedural surfaces. It imports no upstream wave data or lookup table.
Its `ResourceState::Replacement` permits the bounded voice to run, while
`ResourceFlags::WAVES` preserves upstream provenance; the coverage summary
now says “upstream wave assets unaudited.” Mode 8 remains unavailable until
a cleared user-terrain data contract exists.

`.vact` exposes pitch/frequency, `terrain-select` (harmonics role),
timbre/path radius and morph/x offset. Main is the terrain trace; aux is
its transformed path/terrain signal. Two four-float node states are
reserved at installation and a seven-float budget fails. Tests exercise
all eight default selections and both outputs, control response,
deterministic reset, native/browser 44.1/48/96 kHz × 64/256-frame
allocation-free rendering. Position 5 is `Adaptation`: analytic sine,
procedural replacement spectra, path calculation and mode interpolation
differ from source. Fidelity comparison remains SYN-006E work.

### Session: 2026-09-28, SYN-006A position 6 string-machine adaptation

A symbol-level audit of the pinned MIT engine, chord bank, divide-down
oscillator, ensemble/FX engine and used `stmlib` headers found no read of
`wav_integrated_waves` or `waves.bin`. Although the engine and ensemble broadly include
`resources.h`, ensemble uses `SineRaw`, which reads generated `lut_sine`.
`string-machine-voice` replaces that LFO lookup with analytic sine and no
generated table is imported. The public manifest clears the provisional
WAVES flag and records a cleared replacement resource and `Adaptation`.

The instrument exposes pitch/frequency, `machine-chord` (source harmonics
role), timbre/filter-plus-ensemble and morph/registration in `.vact` and
editor metadata. Four independently phased chord voices feed separate L/R
filters and fixed-size ensemble lines. Two 2064-float node states are
reserved at install and a 4127-float budget fails. Tests cover control
response, deterministic reset, distinct L/R, native/browser
44.1/48/96 kHz × 64/256-frame allocation-free rendering. Oscillator
BLEP, exact registration, source SVF and three-tap ensemble topology remain
SYN-006E fidelity work.

### Session: 2026-09-28

Source inspection distinguishes the position 6 broad resource include from
positions 5, 13 and 14, which actually read `wav_integrated_waves`. Position
6 needs final dependency verification; the others remain pending original
wave replacement and source-behavior comparison.
