# Modular Audio DSP Foundation Implementation Plan

**Status**: In Progress (MOD-001 and MOD-002 complete)
**Design Reference**: `design-docs/specs/design-mutable-audio.md`
**Created**: 2026-09-27
**Last Updated**: 2026-09-29

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
    pub vactr_template: Option<&'static str>,
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

### MOD-004 graph shape contract

MOD-004 adds typed bounded audio output ports and per-edge output selection.
Implementation signatures are specified in subtasks MOD-004A..MOD-004F
below; mono graph behavior remains the default.

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
| MOD-001 | Pin source revision and audit every included file/resource and `stmlib` dependency | none | Completed; `verification/upstream_inventory.toml` and `mise run audit-upstream` |
| MOD-002 | Establish neutral user-facing names and upstream-to-Vactr mode/parameter matrix | MOD-001 | Completed; neutral renames, Clouds/Warps inventories, cross-family naming and resolution tests |
| MOD-003 | Deliver typed arbitrary instrument controls end to end, removing silent skips | MOD-002 | Completed for scalar audio parameters |
| MOD-004 | Add typed mono/stereo UGen outputs, edge output selection, voice routing, and same-revision codec compatibility | MOD-003 | Design in `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004`; arbitrary UGen edges pending |
| MOD-005 | Define preallocated state, memory budgets and rate/block adaptation | MOD-004 | Bounded host contract complete; broader per-engine parity pending |
| MOD-006 | Add deterministic comparison harness, coverage report and editor metadata checks | MOD-005 | 24-position inventory and editor/graph checks complete; FM position-10 three-scenario raw-kernel baseline and source-stage follow-up measured, other comparisons pending |
| MOD-006A | Local pinned Plaits FM raw-kernel reference probe and metric-only comparison against Vactr position 10 | MOD-006 inventory | Completed and independently verified; source parity not claimed |

### MOD-004 subtasks

Design reference: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004`.
Each subtask is scoped to one implementation session. “Deliverables” lists
paths and API signatures only; implementation code belongs in source files.

**Fanout execution (2026-09-29).** MOD-004 runs as eight plans with
disjoint write paths. Those plans are authoritative for file-level scope;
the subtasks below keep the acceptance view.

| Wave | Plan | Covers |
|---|---|---|
| 0 | `mod004-00-baseline.md` (MOD004-00) | Golden render/graph digests at the pre-change code; test module scaffolding |
| 1 | `mod004-10-shape-contract.md` (MOD004-10) | MOD-004A contract: shapes, `Edge.output`, `UGenKind::Output`, shared derivation |
| 1 | `mod004-11-engine-split.md` (MOD004-11) | `engine.rs` split before MOD-004B/C |
| 1 | `mod004-12-kernel-pairs.md` (MOD004-12) | MOD-004E kernels: nine pair entry points, `sample::play_stereo` |
| 2 | `mod004-20-select-lowering.md` (MOD004-20) | MOD-004A lowering, MOD-004E selector (VM, checker, lowering) |
| 2 | `mod004-21-codec.md` (MOD004-21) | MOD-004D |
| 2 | `mod004-22-voice-runtime.md` (MOD004-22) | MOD-004B, MOD-004C |
| 3 | `mod004-30-template-migration.md` (MOD004-30) | MOD-004E template migration |
| 4 | `mod004-40-regression-closeout.md` (MOD004-40) | MOD-004F and closeout |

Dependencies: 00 -> {10, 11, 12}; 10 -> {20, 21}; {10, 11, 12} -> 22;
{20, 21, 22} -> 30 -> 40.

#### MOD-004A: Evaluator graph output shapes and edge selection

**Status**: NOT_STARTED
**Depends on**: MOD-003
**Parallelizable**: No
**Deliverables**:
- `src/dsp/graph.rs` + `src/dsp/graph/shape.rs` — `pub enum AudioOutputShape`; `pub struct NodeAudioShape`; `pub struct Edge { from: u16, to: u16, port: u8, output: u8 }`; `UGenKind::Output(u8)`; `pub enum OutputSelector`; `pub fn select_output(kind: &UGenKind, sel: OutputSelector) -> Result<u8, SelectError>` (the single owner, MOD004-10); `derive_shapes`, `voice_layout`, `assign_slices`
- `src/dsp/build.rs` — `lower_inst` lowers the `UGenKind::Output` wrapper to edge output indexes and checks shapes, layout and capacity (MOD004-20)
- `src/vm/call.rs`, `src/types/infer_call.rs` — calling a UGen value selects an output (MOD004-20). `src/ns/insts.rs` is unchanged: selection is validated in the VM, so no `validate_ugen_output` is needed.

**Completion Criteria**:
- [ ] Every node kind declares bounded output count and mono/stereo shape.
- [ ] Lowering retains selected source-output indexes and validates source index and channel-shape compatibility.
- [ ] Invalid output selection and shape mismatch produce typed definition-time diagnostics.
- [ ] Legacy single-output nodes lower as output zero without changing existing graph semantics.

#### MOD-004B: Compiled output buffers and voice routing

**Status**: NOT_STARTED
**Depends on**: MOD-004A
**Parallelizable**: No
**Deliverables**:
- `src/dsp/ugen/mod.rs` — `pub struct Src`; `pub struct NodeSpec`; `pub const MAX_OUTPUTS_PER_NODE: usize`
- `src/dsp/ugen/template.rs` — `pub fn build(&mut self, raw: &RawGraph, env: &BuildEnv) -> Result<(), BuildError>`
- `src/dsp/voice.rs` — `pub fn render<C: CellRead + ?Sized>(voice: &mut Voice, template: &Template, frames: usize, ctx: &mut RenderCtx<'_, C>) -> (usize, bool)`
- `src/dsp/engine.rs` — `pub struct Engine`
- `src/dsp/ugen/build_helpers.rs` — `pub(super) fn mem_need(node: &Node, env: &BuildEnv) -> (usize, usize)`

**Completion Criteria**:
- [ ] Compiled templates assign dense bounded channel-buffer ranges and validate total buffer capacity.
- [ ] Edges read the chosen output and all declared channels; no callback shape inference or allocation is introduced.
- [ ] Mono sink pan and stereo/main-aux channel mapping match the design; additional outputs require explicit routing.
- [ ] Output-buffer and shared-kernel state requirements are included in memory accounting and fail with explicit capacity diagnostics.

#### MOD-004C: Stereo voice-local effects and pan behavior

**Status**: NOT_STARTED
**Depends on**: MOD-004B
**Parallelizable**: No
**Deliverables**:
- `src/dsp/effects/prim.rs` — `pub fn balance_gains(pan: f32) -> (f32, f32)`. `src/dsp/effects/mod.rs` is unchanged: the voice effect unit already takes left/right slices, and only the voice-side downmix goes away.
- `src/dsp/voice.rs` — `pub struct PostFx`; `pub fn render<C: CellRead + ?Sized>(voice: &mut Voice, template: &Template, frames: usize, ctx: &mut RenderCtx<'_, C>) -> (usize, bool)`
- `src/dsp/engine.rs` — `pub fn render(&mut self, out: &mut [f32], frames: usize)`

**Completion Criteria**:
- [ ] Stereo voice effect nodes process independent left/right channels with the existing wet/dry semantics and no downmix.
- [ ] Voice gain, gate, orbit send, and post effects handle all routed channels consistently.
- [ ] Mono pan remains unchanged; stereo and main/aux pan use the selected documented balance law.
- [ ] Existing bus/master stereo effect processing remains unchanged.
- [ ] Stereo and main/aux voices use unity-center balance pan (owner decision, 2026-09-29); mono voices keep equal-power pan.

#### MOD-004D: Graph codec shape and edge encoding

**Status**: NOT_STARTED
**Depends on**: MOD-004A
**Parallelizable**: Yes, with MOD-004B and MOD-004C
**Deliverables**:
- `src/dsp/arena.rs` — `pub fn encode_inst(def: &InstDef, out: &mut Vec<u8>) -> Result<(), BuildError>`; `pub fn decode_graph(bytes: &[u8], raw: &mut RawGraph, bus: &mut BusTemplate) -> Result<GraphKind, FaultCode>`
- `src/dsp/ugen/catalog/codec.rs` — `pub(crate) fn put_spec(out: &mut Out<'_>, spec: &UGenSpec) -> Result<(), BuildError>`; `pub(crate) fn get_node(input: &mut In<'_>, raw: &mut RawGraph) -> Result<(), FaultCode>`
- `src/dsp/ugen/catalog.rs` — `pub const MAX_PORTS: usize`

**Completion Criteria**:
- [ ] Output shapes and edge source-output indexes have a deterministic encoded representation shared by native and browser.
- [ ] Same-revision native/browser peers install the same mono, multi-mono, and stereo graphs.
- [ ] No legacy decoder (owner decision, 2026-09-29): the instrument record tag changes from `I` to `V`, and an old-tag or malformed shape payload is rejected with `BadRecord`.
- [ ] Exact byte fixtures and encode/decode/re-encode round trips cover each graph shape.

#### MOD-004E: `.vact` selector and dual-instance template migration

**Status**: NOT_STARTED
**Depends on**: MOD-004B, MOD-004C, MOD-004D
**Parallelizable**: No
**Deliverables**:
- `src/prelude/templates.vact` — the nine eligible templates listed in the design: `filter-voice`, `fm-pair-voice`, `analog-pair-voice`, `chord-layer-voice`, `wave-grid-voice`, `terrain-voice`, `string-machine-voice`, `shape-voice`, `stage-chain-voice`. `phase-pair-voice` is not eligible because its mode changes state.
- `src/dsp/ugen/{va_filter,fm_pair,analog_pair,chord_pair,table_terrain_pair,terrain_pair,string_machine_pair,shape_pair,stage_chain}.rs` — `*_pair(…, main, aux, …)` entry points; `src/dsp/ugen/sample.rs` — `play_stereo`
- The manifest `main_outputs`/`aux_outputs` fields in `src/dsp/ported/*.rs` are unchanged: they describe the upstream module.
- `select_output`/`OutputSelector` are owned by MOD-004A (MOD004-10). MOD-004E only uses them from `.vact`.

**Completion Criteria**:
- [ ] Calling a multi-output UGen value with an output keyword (`(p :aux)`) or a numeric index (`(p 1)`) selects that output. Selection from a single-output node, an unknown name or an out-of-range index is a definition-time diagnostic (owner decision, 2026-09-29).
- [ ] True simultaneous dual outputs share one node state; distinct mode configurations remain distinct nodes.
- [ ] Existing duplicate-node templates remain valid and unchanged unless their migration passes bit-identical output comparison.
- [ ] Legacy `aux-out`, `out3`, and `out4` templates retain their routing behavior.

#### MOD-004F: Compatibility, callback, and render regression coverage

**Status**: NOT_STARTED
**Depends on**: MOD-004A, MOD-004B, MOD-004C, MOD-004D, MOD-004E
**Parallelizable**: No
**Deliverables**:
- `src/dsp/tests/dsp/` — unit, codec, allocation-probe, rate/block, and template-render regressions
- `src/host/tests/e2e/templates/` — golden digests, `.vact` selection, and migrated-template equivalence (this directory exists)
- `src/dsp/alloc_probe.rs` is unchanged: the existing `Rig::step` and `E2e::run_stereo_for` probes already assert zero callback allocations.

**Completion Criteria**:
- [ ] Every existing prelude template has fixed-event before/after render coverage; unchanged templates are bit-identical.
- [ ] Migrated simultaneous-output templates compare main and aux independently against the prior two-node output.
- [ ] Native and browser e2e coverage verifies channel independence through intermediate effects, voice pan/balance, and orbit routing.
- [ ] Allocation probes pass with multi-output nodes and stereo voice-local effects active.
- [ ] Stateful multi-output renders satisfy supported sample-rate and callback-block partition invariance.

### MOD-006A comparison contract

**Deliverables**: `verification/plaits_fm_reference.cc` (original external
probe source), `examples/fm_pair_reference.rs` (Vactr raw-kernel probe),
`verification/compare_plaits_fm.py` (revision and clean-source checks, isolated build/run,
metrics), a `mise` task with pinned Clang/Python, and a recorded comparison
result in this progress log. The probe
entry points accept fixed 48 kHz, 24-frame-block scenarios with note 69 and
three harmonics/timbre/morph settings, including 0.5/0.5/0.5, for carrier/sub
output. Vactr frequency must
account for the pinned source's 47,872.34 Hz corrected-note constant and its
four phase advances per output sample after a two-octave note shift: note 69
maps to `440 × 48,000 / 47,872.34` Hz at the output.

**Completion Criteria**:
- [x] Reject the wrong upstream Eurorack or `stmlib` revisions.
- [x] Compile and execute the external probe from the user's separate checkout without copying upstream code, resources, audio, or generated artifacts into Vactr.
- [x] Render both Vactr raw-kernel outputs under the documented control mapping and report per-channel RMS, correlation and normalized error after a fixed warm-up.
- [x] Keep the manifest at `Adaptation`; state clearly that the result is a kernel comparison, not full voice/LPG/trigger or `.vact` parity.
- [x] Quiet Cargo checks, strict Clippy, tests, format and diff checks pass after Rust changes; independent Rust review is complete.

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Upstream file inventory | `verification/upstream_inventory.toml`, `verification/audit_upstream.py` | 217 files audited at the pinned revisions | Negative check: a nonexistent upstream path fails the audit |
| Provenance manifest | `src/dsp/ported/*.rs` | Inventories for Plaits, Braids, Elements, Rings, Tides, Peaks, Streams, Stages, Frames, Clouds and Warps | Ordered positions, resource flags, template/editor and graph checks |
| Parameter pipeline | `src/ns/insts.rs`, `src/sched/commit.rs` | Scalar controls complete | Custom audio response, type and capacity faults |
| Stereo audio graph | `src/dsp/graph.rs`, `src/dsp/bus.rs`, `src/dsp/voice.rs` | Bus/effect boundary and explicit voice aux output | Native/browser serialized Matrix and voice routing |
| Metadata and tests | `src/types/manifest.rs`, `src/host/tests/e2e/` | Scalar control metadata complete | End-to-end custom and global-name tests |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Voice and effect ports | This foundation plan | Pending |
| Full coverage sign-off | Every voice/effect plan | Pending |

## Completion Criteria

- [x] Source and resource rights are recorded per included component (per file; unaudited wave, map and digit assets stay excluded).
- [x] Finite scalar declared instrument controls reach audio through the shared native/browser event layout.
- [ ] Stereo input/output survives every graph and install/wire path (bus effects and voice main/aux output verified; arbitrary multi-output UGen edges remain pending).
- [x] Per-event control capacity failures produce faults without truncation.
- [x] Callback allocation and rate/block behavior are verified for the FM drum on native and browser tiers at 44.1/48/96 kHz and 64/256-frame quanta.
- [ ] Cargo check, clippy and targeted tests pass with `CARGO_TERM_QUIET=true`.

## Progress Log

### Session: 2026-09-29, MOD-002 neutral names and mode matrix

**Tasks Completed**:
- User-facing names no longer reuse upstream module names. The user
  approved each mapping below. Upstream paths and internal Rust
  identifiers keep their engineering names, and no aliases were kept.

  | Old | New |
  |---|---|
  | `elements-voice` | `exciter-voice` |
  | `elements-external-voice` | `exciter-external-voice` |
  | `elements-bank` | `exciter-bank` |
  | `elements-internal-core` | `exciter-core` |
  | `rings-voice` | `resonator-voice` |
  | `rings-external-voice` | `resonator-external-voice` |
  | `rings-part-core` | `resonator-part-core` |
  | `tides2-voice` | `tidal-poly-voice` |
  | `tides2-quad-voice` | `tidal-poly-quad-voice` |
  | `el-*` controls | `ex-*` |
  | `rings-*` controls | `reso-*` |
  | `braids-{color,shape,strike,sync,timbre}` | `macro-*` |

- New checked inventories: `src/dsp/ported/clouds.rs` and
  `src/dsp/ported/warps.rs`.
  - Clouds: the four `PlaybackMode` positions map to
    `texture-{grain,stretch,loop,spectral}`, all Adaptation/Replacement.
  - Warps: positions 0–6 of the `modulation_algorithm` range map to
    `dual-mod`. Crossfade, both ring modulations, XOR, comparator and
    vocoder are SourceStage; fold is Adaptation. The hidden frequency
    shifter maps to `shift-pair` as Adaptation.
  - No row claims SourcePort.
- Every published audio family now has an ordered upstream-mode to
  Vactr-name inventory.
- New tests in `src/dsp/ported/tests.rs`:
  - No template, effect, or editor-declared instrument name or parameter
    has a hyphen token equal to an upstream module name. The only
    exemption is the phaser's `stages` control, a DSP term unrelated to
    the module.
  - Every template or effect named by any family inventory resolves to a
    registered template, an effect, or an opt-in example template.
  - Clouds and Warps rows are ordered, pinned, and claim no SourcePort.

**Verification**: independent check-and-test review confirmed:
- nextest: 1,487 passed, 1 skipped;
- quiet check, strict Clippy, rustfmt, the wasm32 library check: pass;
- upstream audit: zero errors;
- the grep for old user-facing names: no hits.

**Remaining**: The per-parameter matrix covers each engine's common
controls and mode selectors. Exact per-knob source equivalence is still
tracked in each family plan.

### Session: 2026-09-29, MOD-001 upstream file inventory and audit

**Tasks Completed**: Added `verification/upstream_inventory.toml`. It covers
all 217 Eurorack and `stmlib` files that Vactr's sources, notices, active
plans, specs and verification scripts name. Each entry records the license
read from the pinned header, a kind (code, generator, aggregate resource or
binary asset), and a use: 199 `source`, 1 `partial` and 17 `excluded` with
reasons. `verification/audit_upstream.py` (`mise run audit-upstream`,
pinned Python) runs against a separate clean checkout at Eurorack
`08460a69` and `stmlib` `e3bd7c9c`. It fails when a declared license
differs from its header. It also fails in these cases:

- a non-MIT file or asset is used;
- Vactr names an upstream path that is absent upstream or from the
  inventory (notice names are resolved per section, with full brace
  expansion);
- a used file lacks a notice;
- a non-MIT transitive `#include` is not explicitly excluded;
- a tracked file is a byte-identical upstream copy;
- six-float or twelve-integer windows from excluded or partial resource
  tables and binary assets appear in Vactr literals without a declared
  import.

**Findings**:
- The product rename had corrupted three upstream facts. Streams' source
  file `streams/vactrol.{h,cc}` and class `Vactrol` had become `vactr` in
  the notice, the Streams plan and the `src/dsp/ported/streams.rs`
  manifest row. These are restored. The user-facing `stream-vactr` name is
  unchanged.
- The Stages notice claimed a nonexistent `variable_shape_oscillator.cc`.
  The Elements notice claimed a nonexistent `patch.cc`. Both are corrected.
- `elements/resources/samples.py` is GPL-3.0 and remains excluded.
- The only non-MIT file in the transitive closure is the GPL-3.0
  `stmlib/ui/event_queue.h`, reached through `frames/ui.h` from the cited
  Frames firmware main. Both are excluded.
- The used `stmlib` closure otherwise contains only MIT files.
- The only matched table import is the twenty declared Warps `fb_*` rows.
  No other generated table, DX7 or TI data, wave, map or digit asset
  appears in Vactr.

**Verification**:
- Negative check: restoring the damaged Streams source path in
  `streams.rs` makes the audit fail.
- Independent check-and-test review of the Rust change passed: quiet
  check, strict Clippy, rustfmt, the wasm32 library check, diff check,
  and nextest (1,483 passed, 1 skipped).

**Remaining**: MOD-002. The origins of `waves.bin`, `map.bin` and
`digits.bin` are still unknown, so they stay excluded. The audit proves
non-import and notice coverage, not source fidelity. Re-run it after any
new upstream reference, and after any revision change.

### Session: 2026-09-28, MOD-006A Plaits FM raw-kernel comparison

An opt-in `mise run compare-plaits-fm` task uses pinned Clang 21.1.8 and
Python 3.12.14. It verifies Eurorack revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` and `stmlib` revision
`e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`, compiles the separate
upstream FM engine into a temporary executable, and compares 2,376 frames
after one 24-frame warm-up block at 48 kHz. Run it as
`VACTR_MI_REFERENCE=/path/to/eurorack mise run compare-plaits-fm`.
Both probes use note 69, common
controls 0.5 and independent carrier/sub output; the Vactr probe accounts
for the source's four phase advances and corrected note constant. Before
SYN-002C2, main reference/Vactr RMS was 0.6667/0.7072, correlation
-0.0945 and normalized RMS error 1.5250. Aux RMS is 0.6695/0.7342,
correlation 0.8473 and normalized RMS error 0.5868. The same pitch mapping
was checked against the pinned source before recording these values. The
baseline waveform gap is substantial; it does not support a `SourcePort` claim.
The comparison is raw-kernel only, with no `.vact` event, trigger, LPG,
host-output or browser pathway. The separate checkout's generated resources
are compiled only for the local reference process; no upstream object,
audio, table or sample is stored in Vactr. More parameter points, isolated
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
