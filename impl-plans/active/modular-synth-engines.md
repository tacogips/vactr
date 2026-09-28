# Modular Synth Engine Ports Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-27
**Last Updated**: 2026-09-27

## Design Document Reference

Port every eligible published oscillator, physical-model, percussion, and
audio-rate function engine. Use neutral Vactr names, the shared parameter
manifest, and only cleared code/resources. `modular-audio-foundation.md` is
the prerequisite; `modular-fm-drums.md` owns the Peaks FM drum slice.
Plaits-specific engine slices and fidelity work are tracked in
`impl-plans/active/modular-plaits-engines.md`.

## Modules

### `src/dsp/ported/voice.rs`

```rust
pub enum VoiceFamily { Macro, Modal, String, Drum, Function }
pub struct VoiceSpec {
    pub family: VoiceFamily,
    pub model: u16,
    pub parameter_count: u16,
    pub output_count: u8,
}
pub fn voice_specs() -> &'static [VoiceSpec];
```

Each concrete engine keeps preallocated state and renders at the host sample
rate. A stable mode identifier and typed parameter schema reach `.vact`,
the editor, graph lowering, and the scheduler. External resources are
separate from code and installed through the resource host.

The pinned `plaits/dsp/voice.cc` registers 24 engine positions. Its source
order is the stable inventory for the initial mapping:

| Positions | Source engine(s) | Resource review |
|---|---|---|
| 0–1 | Virtual-analog VCF, phase distortion | DSP dependencies pending |
| 2–4 | Six-operator FM (three registrations) | Bundled DX7-derived patch-bank provenance unresolved; use user-provided/original patches until cleared |
| 5–7 | Wave terrain, string machine, chiptune | Generated wave resources require per-file review |
| 8–11 | Virtual analog, waveshaping, FM, grain | DSP dependencies pending |
| 12–14 | Additive, wavetable, chord | Generated waveform resources require per-file review; no LXR data |
| 15 | Speech | LPC words state they were extracted from TI ROMs; do not bundle pending rights clearance |
| 16–20 | Swarm, noise, particle, string, modal | DSP dependencies pending |
| 21–23 | Bass drum, snare drum, hi-hat | DSP dependencies pending; separate from Peaks percussion |

This is an engine-ID inventory, not a claim that their controls, secondary
outputs, dependencies or assets have been fully audited or ported. A mode
backed by uncleared data must use user-supplied or original replacement
resources and disclose the resulting sonic difference.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| SYN-001 | Enumerate all Plaits engine IDs, controls, two outputs and resource dependencies | MOD-002 | In progress: IDs and high-risk resources mapped |
| SYN-002 | Port Plaits engines 0–15 with tests and metadata | SYN-001, MOD-005 | Not started |
| SYN-003 | Port Plaits engines 16–23 with tests and metadata | SYN-001, MOD-005 | Not started |
| SYN-004 | Enumerate and port every Braids macro shape | MOD-002, MOD-005 | 47 accessible positions enumerated; see `modular-braids-shapes.md` |
| SYN-005 | Port Elements exciters, resonators, voices and effects | MOD-005 | Patch/source boundary audited; see `modular-elements-model.md` |
| SYN-006 | Port Rings modes, polyphony, strum and stereo effects | MOD-005 | Six-model source inventory; see `modular-rings-resonator.md` |
| SYN-007 | Adapt Peaks bass, snare and hi-hat architectures; coordinate FM drum plan | MOD-003 (full MOD-005 deferred) | Four drum adaptations; remaining function audit in `modular-peaks-functions.md` |
| SYN-008 | Port Tides generations and audio-rate Stages/Frames functions | MOD-005 | Tides source inventory; see `modular-tides-functions.md`; Stages/Frames pending |

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Engine registry | `src/dsp/ported/voice.rs` | Not started | - |
| Voice kernels | `src/dsp/ugen/analog_percussion.rs` | Three analytic modes | Native/browser codec render |
| Instrument metadata | `src/session/editors.rs`, `src/prelude/templates.vact` | Dynamic header schema | Editor-knob check |
| End-to-end | `src/host/tests/e2e/templates.rs` | Three playable instruments | Per-parameter audio response |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| All ported voices | `modular-audio-foundation.md` | Pending |
| Peaks FM drum | `modular-fm-drums.md` | In progress |

## Completion Criteria

- [ ] Every eligible voice/model has a manifest entry and working renderer.
- [ ] Every mode and control is settable in `.vact` and appears in metadata.
- [ ] Stereo/secondary outputs and triggers are verified.
- [ ] Resource rights and notices are recorded for each engine.
- [ ] Browser/native output is finite, audible, and callback-allocation-free.
- [ ] Quiet cargo check, clippy and tests pass.

## Progress Log

### Session: 2026-09-27

**Tasks Completed**: Scope documented; all 24 Plaits engine positions mapped from the pinned source, with DX7 patch banks and TI speech ROM extracts flagged for exclusion pending clearance.
**Tasks In Progress**: SYN-001 control, secondary-output and dependency inventory.
**Blockers**: Foundation interfaces and resource provenance.

### Session: 2026-09-27, SYN-007 analytic percussion slice

**Source audit**: At revision `08460a69`, the MIT-covered bass header exposes frequency, punch, tone, decay; snare exposes frequency, tone, snappy, decay. High-hat `Configure` is empty; its four Vactr controls are extensions. The three implementations use original floating-point oscillators, envelopes, filters and seeded noise. They import no Peaks, `stmlib`, resource or wave-table code/data. The copyright/MIT notice is in `THIRD_PARTY_NOTICES.md`.
**Implementation**: One fixed-state percussion UGen has three selected architectures; distinct `low-drum`, `wire-drum`, and `metal-hat` `.vact` templates expose all named controls through MOD-003's typed scalar parameter path. The graph codec carries the mode and parameters unchanged on native and browser tiers.
**Dependency rationale**: Each node uses existing preallocated `NodeState` and no variable-size delay/resource memory, so this narrow slice does not require the broader MOD-005 memory-budget contract. It does not establish parity for all Peaks modes or any other Mutable engine.
**Verification**: End-to-end control-response, editor schema, native/browser codec render and zero-allocation callback checks pass. `CARGO_TERM_QUIET=true cargo check -q`, strict Clippy, full `cargo test -q` (1001 library tests and integration suites), rustfmt and diff checks pass.
