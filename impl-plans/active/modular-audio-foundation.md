# Modular Audio DSP Foundation Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md`
**Created**: 2026-09-27
**Last Updated**: 2026-09-27

## Design Document Reference

Build the shared porting contract before individual instruments and effects.
The source tree is pinned at `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`.
This plan covers source/asset provenance, parameter delivery, stereo graph
ports, preallocated state, and the verification harness.

## Modules

### `src/dsp/ported/manifest.rs`

```rust
pub enum CoverageState { Pending, Adaptation, SourceStage, SourcePort, Unavailable }
pub enum ResourceState { None, Replacement, NeedsAudit, RightsBlocked }
pub struct PortedAlgorithm {
    pub position: u8,
    pub source_engine: &'static str,
    pub source_path: &'static str,
    pub source_revision: &'static str,
    pub vactrol_template: Option<&'static str>,
    pub controls: CommonControls,
    pub main_outputs: u8,
    pub aux_outputs: u8,
    pub coverage: CoverageState,
    pub resources: ResourceState,
    pub resource_flags: ResourceFlags,
}
pub fn plaits_algorithms() -> &'static [PortedAlgorithm];
pub fn plaits_coverage_summary() -> String;
```

MOD-006 begins with all 24 registered Plaits positions in a checked static
manifest. Status distinguishes a runnable architectural adaptation from a
source-stage translation and a complete source port. Shared note, harmonics,
timbre and morph roles are mapped to neutral `.vact` names for implemented
positions; source-only trigger/LPG semantics remain explicit gaps. Resource
flags distinguish engine code from uncleared DX7 patches, TI ROM words and
other generated or recorded data. A checked inventory can later expand to
Braids, Clouds, Rings, Elements, Warps and utility audio modules. A report
must never label a partial adaptation as complete.

### `src/dsp/graph.rs`, `src/dsp/ugen/`, `src/dsp/effects/`

```rust
pub struct StereoPorts { pub inputs: u8, pub outputs: u8 }
pub fn port_shape(kind: UGenKind) -> StereoPorts;
pub fn effect_port_shape(kind: EffectKind) -> StereoPorts;
```

### `src/ns/insts.rs`, `src/sched/commit.rs`, `src/types/manifest.rs`

```rust
pub fn resolve_declared_param(inst: &InstDef, name: KwId) -> Option<CtlId>;
pub fn validate_declared_param(inst: &InstDef, name: KwId, value: &Value) -> Result<Ctl, Failure>;
```

Use established project types where possible; these are the intended public
contracts, not a demand for parallel registries.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| MOD-001 | Pin source revision and audit every included file/resource and `stmlib` dependency | none | Not started |
| MOD-002 | Establish neutral user-facing names and upstream-to-Vactrol mode/parameter matrix | MOD-001 | Not started |
| MOD-003 | Deliver typed arbitrary instrument controls end to end, removing silent skips | MOD-002 | Completed for scalar audio parameters |
| MOD-004 | Add dual-input/dual-output graph and host wire contract with compatibility tests | MOD-003 | Stereo bus/effect and bounded voice main/aux path complete; arbitrary stereo edges pending |
| MOD-005 | Define preallocated state, memory budgets and rate/block adaptation | MOD-004 | Bounded host contract complete; broader per-engine parity pending |
| MOD-006 | Add deterministic comparison harness, coverage report and editor metadata checks | MOD-005 | 24-position inventory and editor/graph checks complete; FM position-10 three-scenario raw-kernel baseline and source-stage follow-up measured, other comparisons pending |
| MOD-006A | Local pinned Plaits FM raw-kernel reference probe and metric-only comparison against Vactrol position 10 | MOD-006 inventory | Completed and independently verified; source parity not claimed |

### MOD-006A comparison contract

**Deliverables**: `verification/plaits_fm_reference.cc` (original external
probe source), `examples/fm_pair_reference.rs` (Vactrol raw-kernel probe),
`verification/compare_plaits_fm.py` (revision and clean-source checks, isolated build/run,
metrics), a `mise` task with pinned Clang/Python, and a recorded comparison
result in this progress log. The probe
entry points accept fixed 48 kHz, 24-frame-block scenarios with note 69 and
three harmonics/timbre/morph settings, including 0.5/0.5/0.5, for carrier/sub
output. Vactrol frequency must
account for the pinned source's 47,872.34 Hz corrected-note constant and its
four phase advances per output sample after a two-octave note shift: note 69
maps to `440 × 48,000 / 47,872.34` Hz at the output.

**Completion Criteria**:
- [x] Reject the wrong upstream Eurorack or `stmlib` revisions.
- [x] Compile and execute the external probe from the user's separate checkout without copying upstream code, resources, audio, or generated artifacts into Vactrol.
- [x] Render both Vactrol raw-kernel outputs under the documented control mapping and report per-channel RMS, correlation and normalized error after a fixed warm-up.
- [x] Keep the manifest at `Adaptation`; state clearly that the result is a kernel comparison, not full voice/LPG/trigger or `.vact` parity.
- [x] Quiet Cargo checks, strict Clippy, tests, format and diff checks pass after Rust changes; independent Rust review is complete.

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Provenance manifest | `src/dsp/ported/manifest.rs` | Plaits inventory complete; other family inventories pending | Ordered positions, resource flags, template/editor and graph checks |
| Parameter pipeline | `src/ns/insts.rs`, `src/sched/commit.rs` | Scalar controls complete | Custom audio response, type and capacity faults |
| Stereo audio graph | `src/dsp/graph.rs`, `src/dsp/bus.rs`, `src/dsp/voice.rs` | Bus/effect boundary and explicit voice aux output | Native/browser serialized Matrix and voice routing |
| Metadata and tests | `src/types/manifest.rs`, `src/host/tests/e2e/` | Scalar control metadata complete | End-to-end custom and global-name tests |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Voice and effect ports | This foundation plan | Pending |
| Full coverage sign-off | Every voice/effect plan | Pending |

## Completion Criteria

- [ ] Source and resource rights are recorded per included component.
- [x] Finite scalar declared instrument controls reach audio through the shared native/browser event layout.
- [ ] Stereo input/output survives every graph and install/wire path (bus effects and voice main/aux output verified; arbitrary multi-output UGen edges remain pending).
- [x] Per-event control capacity failures produce faults without truncation.
- [x] Callback allocation and rate/block behavior are verified for the FM drum on native and browser tiers at 44.1/48/96 kHz and 64/256-frame quanta.
- [ ] Cargo check, clippy and targeted tests pass with `CARGO_TERM_QUIET=true`.

## Progress Log

### Session: 2026-09-28, MOD-006A Plaits FM raw-kernel comparison

An opt-in `mise run compare-plaits-fm` task uses pinned Clang 21.1.8 and
Python 3.12.14. It verifies Eurorack revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` and `stmlib` revision
`e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`, compiles the separate
upstream FM engine into a temporary executable, and compares 2,376 frames
after one 24-frame warm-up block at 48 kHz. Run it as
`VACTROL_MI_REFERENCE=/path/to/eurorack mise run compare-plaits-fm`.
Both probes use note 69, common
controls 0.5 and independent carrier/sub output; the Vactrol probe accounts
for the source's four phase advances and corrected note constant. Before
SYN-002C2, main reference/Vactrol RMS was 0.6667/0.7072, correlation
-0.0945 and normalized RMS error 1.5250. Aux RMS is 0.6695/0.7342,
correlation 0.8473 and normalized RMS error 0.5868. The same pitch mapping
was checked against the pinned source before recording these values. The
baseline waveform gap is substantial; it does not support a `SourcePort` claim.
The comparison is raw-kernel only, with no `.vact` event, trigger, LPG,
host-output or browser pathway. The separate checkout's generated resources
are compiled only for the local reference process; no upstream object,
audio, table or sample is stored in Vactrol. More parameter points, isolated
spectral comparisons and the other engines remain MOD-006 work.
Independent review reproduced the metrics, verified the fixed note mapping
against the pinned source and passed 1,420 Rust tests (one ignored), native,
no-default-feature and wasm checks, strict Clippy, rustfmt, the mise task,
task validation, Taplo and diff checks. Physical audio audition was not part
of this deterministic kernel comparison.

### Session: 2026-09-27, MOD-006 Plaits coverage inventory

**Tasks Completed**: Added a public 24-row manifest in the exact pinned `plaits/dsp/voice.cc` registry order, with per-position source path/revision, neutral template name, common control-role mapping, main/aux count, fidelity status and resource flags. `plaits_coverage_summary()` reports eleven pending, five adaptations, four source-stage translations, zero complete source ports and four unavailable published ROM-backed modes. Four wave-asset positions remain pending provenance audit, including position 13: MIT `wavetables.py` reads `waves.bin`, whose individual origin has not been established. Tests check contiguous positions, editor controls, registered templates, realized dual-output graphs and DX7/TI resource exclusions.
**Remaining**: Deterministic upstream-output comparisons, more detailed provenance for wave-backed and other pending positions, and inventories for other Mutable families. This inventory does not sign off any full Plaits source port.

### Session: 2026-09-27, MOD-005 bounded host contract

**Tasks Completed**: Added explicit `EngineConfig::validate` and `Engine::try_with_config` checks before buffer allocation. Supported rates are finite 8–192 kHz and `max_block` is 1–8192 frames; nonfinite or over-limit delay-state seconds fail with `ConfigError`. The existing infallible constructor now rejects invalid configurations explicitly rather than silently substituting 48 kHz or a one-frame block. The callback retains its preallocated chunking behavior. Tests verify native and browser serialized FM drum installs and finite, audible, allocation-free renders at 44.1/48/96 kHz and two block sizes, plus installation rejection when the feedback drum's fixed 1001-sample state exceeds the per-voice budget.
**Remaining**: The 1001-sample feedback delay is sample-count based by design; duration changes with host rate. Broader per-engine rate equivalence, arbitrary block-boundary comparison, and aggregate memory ceilings across voices/buses remain for later foundation work. No claim of full Mutable engine parity.

### Session: 2026-09-27

**Tasks Completed**: Official source and license inventory started; design and foundation plan created.
**Tasks In Progress**: MOD-001; MOD-003 scalar parameter foundation implemented and verified.
**Blockers**: Some resources refer to external preset/ROM data and require provenance review.

### Session: 2026-09-27, scalar parameter routing

**Tasks Completed**: MOD-003 scalar header parameters now retain their declared type and wire ID from realization through pattern compilation, checker, scheduler and editor manifest. Unknown audio instrument controls and over-capacity events fail explicitly. End-to-end tests cover a custom name absent from the global control table, wrong type, an undeclared name, a global-name type override and 33-control overflow. The audio callback remains allocation-free in the render test.
**Wire note**: `MAX_CTLS=32` fixes the browser `AudioEvent::ENCODED_LEN`; native and browser peers built from the same revision agree. This is not a versioned cross-release or persisted wire format, so mixed revisions must not be paired. A versioned negotiation is still needed if mixed-release peers become a requirement.
**Remaining at that session**: Keyword/resource and aggregate headers remained realization-time data rather than generic scalar audio controls. Stereo and full Mutable engine coverage were not claimed.

### Session: 2026-09-27, MOD-004 stereo bus boundary

**Tasks Completed**: Defined `AudioPortShape` and an explicit 2-input/2-output contract for bus effect chains. The existing bus/master processor already holds independent left and right preallocated buffers; no channel-count field was added to the graph codec, because the decoded bus effect kind determines the same shape on both native and browser tiers. New native and browser tests install two hard-panned sources, serialize/install a stereo Matrix bus, route both voices through it and verify that its swapped outputs stay distinct through the master. Engine callback allocation checks run in both tests.
**Remaining at that session**: Voice-internal UGen edges and effect nodes were mono; an effect used as a voice node downmixed the kernel's stereo result. Dual-output instruments and per-edge channel identity needed a separate graph/voice-buffer revision before Clouds, Warps or Rings could be represented faithfully. That session delivered the narrower working stereo bus/effect layer, not any Mutable effect port.

### Session: 2026-09-27, MOD-004 bounded voice main/aux path

**Tasks Completed**: Added `aux-out` as an explicit `.vact` UGen output tap. A voice graph can now construct independent mono main and auxiliary branches; `aux-out` sends its input to the auxiliary bus while contributing silence to the main branch. A preallocated second voice output buffer keeps main/aux separate through voice gain, post effects, bus/orbit routing and interleaved host output. The graph codec serializes the marker as node tag 36; native/browser tests verify decoded templates and distinct audio. Existing mono instruments retain their old pan behavior, while a voice with `aux-out` routes main→L and aux→R without collapsing them. No per-callback memory is allocated.
**Remaining**: This is an explicit two-output voice boundary, not a general multi-output UGen/edge type. Intermediate UGen edges are still mono, voice-local stereo effects still downmix their own outputs, and pan on a two-output voice does not alter its channel mapping. Stereo sampler internals and arbitrary per-edge output selection remain for the broader MOD-004 design and future Mutable parity work.
**Verification**: `.vact` end-to-end, native and browser graph-codec tests demonstrate independent main/aux output and unchanged mono pan behavior. The audio callback allocation probes pass. `CARGO_TERM_QUIET=true cargo check -q`, strict Clippy and the full `cargo test -q` suite (1010 library tests and integration suites), rustfmt and diff checks pass; all touched Rust sources are under 1000 lines.
