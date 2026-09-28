# Programmable Digital Drums Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-music.md#41-programmable-digital-drums-author-2026-09-27`
**Created**: 2026-09-27
**Last Updated**: 2026-09-28

## Design Document Reference

Implement Vactr's original digital drum kit and make all of its declared
sound parameters addressable in `.vact`. The external repository is a research
reference for an MIT-compatible drum DSP design. DaisySP code may be ported
only with its applicable MIT copyright and license notices. No third-party
wave tables, samples, presets, lookup data, or asset-derived traces may be
reused. The product and templates use Vactr names.

## Modules and interfaces

### Instrument parameter contract

`src/dsp/controls.rs`, `src/ns/insts.rs`, `src/sched/commit.rs`,
`src/types/check.rs`, and `src/types/manifest.rs`:

```rust
pub struct InstParamDef {
    pub name: &'static str,
    pub ctl: CtlId,
    pub default: f32,
    pub range: (f32, f32),
    pub domain: CtlDomain,
}
pub fn resolve_inst_param(inst: &InstDef, name: KwId) -> Option<&InstParamDef>;
pub fn encode_inst_param(param: &InstParamDef, value: &Value) -> Result<f32, Failure>;
```

The precise ownership of `InstParamDef` may follow the existing `InstDef`
and `ParamMeta` layout. A dynamic header name must reach the selected voice,
including through a late cell, and must be checked against its declared type.
Reject capacity overflow and unknown controls explicitly.

### Voice DSP and templates

`src/dsp/ugen/`, `src/dsp/graph.rs`, `src/dsp/ugen/catalog.rs`,
`src/prelude/templates.vact`, and `src/dsp/meta.rs`:

```rust
pub enum DigitalDrumFamily { Tonal, Snare, Metal, Hat }
pub struct DigitalDrumParams { pub family: DigitalDrumFamily }
pub fn digital_drum_ports(family: DigitalDrumFamily) -> &'static [Port];
```

Use existing ugens where their behavior is sufficient; add original bounded
ugens for missing modulation, pitch envelopes, transient generation, and
voice-specific noise/metallic spectra. Four named templates expose every
applicable parameter from the design inventory. Add kit keyword aliases and
metadata without weakening existing generic synthesis templates.

## Subtasks

| Task | Deliverable | Depends on | Status |
|------|-------------|------------|--------|
| DDRUM-001 | Inventory original sound controls and map sequencer/host controls to Vactr constructs | none | Partial; per-voice controls exist, kit-wide inventory open |
| DDRUM-002 | Route, validate, and publish arbitrary declared instrument controls; reject overflows | DDRUM-001 | Implemented by MOD-003 typed parameter routing |
| DDRUM-003 | Build and test tonal drum and snare graphs with all applicable controls | DDRUM-002 | Several neutral tonal/snare voices runnable; unified kit pending |
| DDRUM-004 | Build and test metallic and hat graphs with all applicable controls | DDRUM-002 | Several neutral metallic/hat voices runnable; unified kit pending |
| DDRUM-005 | Add LFO/velocity targets, transient controls, and audio-rate behavior | DDRUM-003, DDRUM-004 | Partial; individual voice velocity/transients runnable, kit mapping open |
| DDRUM-006 | Expose kit/metadata, `.vact` examples, and native/browser end-to-end tests | DDRUM-005 | Individual templates/editor/host tests runnable; kit API pending |

## Module status

| Module | File path | Status | Tests |
|--------|-----------|--------|-------|
| Parameter routing | `src/ns/insts.rs`, `src/sched/commit.rs` | Typed custom controls implemented; kit-level mapping pending | MOD-003 tests |
| Type and editor metadata | `src/types/`, `src/dsp/meta.rs` | Per-voice metadata implemented; kit editor pending | Template metadata tests |
| DSP and templates | `src/dsp/`, `src/prelude/templates.vact` | Multiple original voices, including `feedback-metal-drum`; unified kit pending | Per-voice audio tests |
| End-to-end | `src/host/tests/e2e/` | Per-voice paths implemented; kit path pending | Native/browser/control response |

## Dependencies

| Feature | Depends on | Status |
|---------|------------|--------|
| Voice DSP | Typed parameter routing | Satisfied |
| Kit and editor | Voice DSP, metadata | Pending |

## Completion Criteria

- [ ] Every original sound-control category has a documented Vactr mapping.
- [ ] Any ported DaisySP code retains applicable MIT notices; no third-party wave table, sample, preset, or lookup data enters the implementation.
- [ ] Every exposed voice knob accepts `.vact` pattern and live-cell values.
- [ ] Tonal drum, snare, cymbal, and hat render finite, audible audio.
- [ ] Parameter changes produce measurable sound differences by family.
- [ ] Unknown controls and resource limit breaches produce diagnostics.
- [ ] Editor metadata covers every declared parameter.
- [ ] `CARGO_TERM_QUIET=true cargo check`, clippy, and relevant tests pass.

## Progress Log

### Session: 2026-09-27

**Tasks Completed**: Design gap identified and implementation plan created.
**Tasks In Progress**: Original parameter inventory and independent DSP specification.
**Blockers**: None.
**Notes**: Existing seven templates provide generic digital synthesis, but event commit currently skips custom controls without a built-in row. Preserve unrelated working-tree changes.

### Session: 2026-09-28

Typed custom instrument controls now route through the scheduler, and several
separate original drum templates have `.vact`, editor and native/browser tests.
`feedback-metal-drum` adds a coupled-FM metallic voice based only on general
percussion synthesis research. This plan's unified kit inventory, common
velocity/LFO assignment, family aliases and kit-level editor remain open.
The 2026-09-27 statement about custom controls being silently skipped is
historical and was resolved by MOD-003; no imported LXR, Elektron or other
uncleared waveform or preset assets are part of this progress.
