# Vactrol Middle End: Pattern Engine, Signals, Clock, Visual Chains (ME-PATTERN) Implementation Plan

**planId**: ME-PATTERN (implements vactrol-core.md TASK-006 except the criteria that need the VM, which ME-INTEGRATE completes)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md sections 9 (visual chains), 10.1 (representation, sample-region operators, "Sound first and the first-structure rule"), 10.2-10.5, 11.1, 11.7 (`midi-notes` after `s`, input lane, `cc`, `:midi` clock), 7.1.2, 7.1.3 (Query VM handle, Input cells), 7.1.4 (chords, `grid`, overloads, sound kit), 7.1.5; design-music.md sections 1-3; design-visual.md
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/2
**dependsOn**: ME-MASKS (7.1.6 codes), ME-FRONTEND (`Value::Sound`, `PathVal`)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

A pattern is a lazy, infinite function of time to events, represented as an inspectable node tree (10.1). This plan
builds `Pat`/`PatNode`/`Event`/`TimeSpan`/`OccKey`, the pure `query` with exact `Ratio64` positions and event-local
fault recording (10.3), every combinator of the design-music vocabulary in scope, the sample-region operators with
layer-1 cover equivalence, the pure hash RNG, signals, `Tempo`/`Clock`/`ClockSource` with the `:midi` anchor math, the
SOUND FIRST `PatNode::Sound { src, kit }` with the first-structure rule, `MidiNotes { subject, channel }` with the
input-lane walk, and visual chains (`TexNode`, `compile_tex`, `ShaderDesc`, `UniformPlan`). Closures, late refs and the
sound kit are reached only through the `QueryVm` trait defined here (7.1.3); tests use a stub. ME-INTEGRATE implements
`QueryVm` for `Vm` and the native wrappers.

## Non-Goals

- No edits to `value/`, `ns/`, `vm/`, `compile/` or `types/`. `VarSlotRef` and `Closure` are opaque here.
- No natives (`s`, `note`, `d1`, ... are ME-INTEGRATE wrappers around the builders here).
- No scheduler, slot table, layer-2 dedup, staging invalidation, live MIDI realization loop, render host or GL context
  (TASK-007/008). `compile_tex` only produces strings and plain metadata.
- No `PParam::Late`/`PParam::Fn` behavior through a real VM and no tweaked-probability re-query (ME-INTEGRATE).

## writePaths (exclusive)

- `src/pattern/pat.rs` (replace the `Pat` shell), `step.rs`, `query.rs`, `occ.rs`, `rng.rs`, `signal.rs` (replace the
  `Sig` shell), `eval.rs` (`QueryVm`, `QueryCtx`, `InputCells`), `build.rs` (constructors from `Value`s used by ME-INTEGRATE)
- `src/pattern/combinators/mod.rs`, `time.rs`, `structure.rs`, `random.rs`, `region.rs`, `music.rs`, `control.rs`,
  `input.rs`, `sound.rs`
- `src/pattern/tests/mod.rs`, `steps.rs`, `combinators.rs`, `random.rs`, `region.rs`, `cover.rs`, `sound.rs`, `input.rs`,
  `signals.rs`, `faults.rs`, `stub_vm.rs`
- `src/clock/mod.rs`, `src/clock/tempo.rs`, `src/clock/clock.rs`, `src/clock/tests/mod.rs`, `src/clock/tests/clock.rs`
- `src/tex/texnode.rs` (replace the shell; keep `OutId`), `src/tex/shader.rs`, `src/tex/uniforms.rs`,
  `src/tex/tests/mod.rs`, `src/tex/tests/goldens.rs`
- `impl-plans/active/vactrol-middle-pattern.md`

## sharedPaths

- `src/lib.rs`: add exactly `pub mod clock;` (7.1.7: only this wave adds a module). Do not touch the crate doc.
- `src/pattern/mod.rs`, `src/tex/mod.rs`: declare the new modules.

## File-Level Changes (signatures only; vactrol-core.md Module 5 sketch is the shape, with the 10.1 amendments)

- `pat.rs`: `Pat { node: PatNode, span: Option<Span>, structured: bool }`; `PatNode` exactly as design 10.1, including
  `Sound { src: PParam, kit: Option<PParam> }` and `MidiNotes { subject: Rc<Pat>, channel: Option<u8> }`;
  `PParam { Const(Value), Late(VarSlotRef), Fn(Value), Pat(Rc<Pat>) }`; `SliceCuts { Equal(PParam), Manual(Box<[PParam]>) }`.
- `query.rs`: `TimeSpan`, `Event { whole, part, value, controls: BTreeMap<KwId, Value>, src: Option<SrcRef>, occ: OccKey }`,
  `QueryResult { events, faults, output }`, `query(p: &Pat, span: TimeSpan, cx: &mut QueryCtx) -> QueryResult` (never
  `Err`; event-local and subtree-local faults with origin; all `Ratio64` math checked, overflow is a fault).
- `occ.rs`: `OccKey { path: SmallVec-free Vec<(NodeId, u32)>, anchor: Ratio64, cycle: i64 }` (no new dependency: use `Vec`).
- `rng.rs`: pure hash RNG keyed by (seed, node id, cycle[, sequence]).
- `eval.rs`: `trait QueryVm { fn call(&mut self, f: &Value, args: &[Value]) -> Result<Value, Failure>; fn deref(&mut self, r: &VarSlotRef) -> Result<Value, Failure>; fn take_output(&mut self) -> Vec<(Origin, Rc<str>)>; fn sound_kit(&mut self) -> Result<Value, Failure>; }`;
  `InputCells` keyed by (channel, controller), `AnalyzerId`, `HostSig`; `QueryCtx { vm: &mut dyn QueryVm, cells: &InputCells, seed: u64 }`.
- `combinators/*`: the TASK-006 vocabulary (fast, slow, rev, every, whenmod, sometimes/rarely/often, sometimes-by,
  degrade-by, alt, maybe, euclid, hold, repeat, choose, stack, cat, fastcat, superimpose, off, jux, iter, chop, ply,
  chunk, hurry, segment, range (subject first), scale (pattern subject = scale notes), chord (`:maj7 :m7 :dom7 :sus4`
  intervals from 7.1.4), voicing, arp, grid, striate, slice, splice, loop-at, fit, cut, controls). Every numeric
  parameter is a `PParam`. Region operators follow 10.1 exactly (whole-span anchored, occurrence-keyed).
- `combinators/sound.rs` (10.1 "Sound first"): `sound(src: PParam, kit: Option<PParam>) -> Pat` (unstructured for one
  sound, structured for `PParam::Pat`); keyword resolution per query through `kit` or `cx.vm.sound_kit()`; a missing key
  is an event-local `unknown-sound` fault; a `Value::Sound` is used as is; a sound bank (list of sounds) plus an `n`
  control picks the i-th sound (index outside -> event-local `slice-index`); structure-giving steps (list-valued or
  structured controls, `euclid`, `grid`, `midi-notes`) on an unstructured subject replace the structure; scalar/signal
  controls keep it unstructured; any other operator or a sink realizes it as one event per cycle `[c, c+1)` (M4); a
  control on a structured subject samples the value at `whole.begin` (or `part.begin` if `whole` is `None`).
- `combinators/input.rs` (11.7): `midi_notes(subject: Rc<Pat>, channel) -> Pat`; `input_lane_walk(p: &Pat) -> Result<LanePlan, (DiagCode::InputLaneOperator, Span)>`
  classifying the operators after `midi-notes` (supported per-note: controls, scale/chord kin, `maybe`/`degrade-by`/
  `sometimes-by`; unsupported: re-timing/structural operators, and a structured subject); `realize_note(plan, note, seq, cx) -> Option<Event>`
  applying the per-note operators with the RNG keyed by node id and arrival sequence. `MidiNotes` yields no events under
  `query`.
- `signal.rs`: `Sig` per the Module 5 sketch, `Sig::Cc` and `Sig::Analyzer` and host signals read `InputCells`.
- `clock/`: `Tempo { bpm, beats_per_cycle }`, `Clock` (piecewise-linear host seconds <-> `Ratio64` cycles, anchors on
  tempo change, derived cps), `ClockSource { Internal, MidiClock, Link }` (`Link` selection is the checker's
  `clock-source-unavailable`), `:midi` anchor math taking pulse timestamps as arguments (smoothing keeps logical
  positions exact).
- `tex/`: `TexNode`, `TexKind` (design 9.1 Hydra sources, geometry, color, blend, modulate, chain, `Text`), `VParam`,
  `compile_tex(&TexNode) -> Result<(ShaderDesc, UniformPlan), Failure>` (GLSL ES 3.0 from per-operator snippets),
  `ShaderDesc { source: String, uniform_names: Box<[String]>, assets: Box<[TextAsset]> }`, `TextAsset`,
  `UniformPlan`/`UniformSpec`, `resolve_uniforms(&UniformPlan, frame_time, cx) -> Vec<(String, f32)>`. `texture` subject
  overloads of `scale`/`shape` (M1) are builders here; dispatch is ME-INTEGRATE's.

## Required Tests (golden values are exact `Ratio64`s)

- `steps.rs`: `[:bd :sd [:hh :hh]]` gives onsets 0, 1/3, 2/3 and 5/6 (wholes 1/3, 1/3, 1/6, 1/6); every row of the
  design-music section 3 mini-notation equivalence table; nil is a rest; nested subdivision.
- `combinators.rs`: fast/slow/rev/every/whenmod/stack/cat/fastcat/off/iter/ply/chunk/segment/range/scale/chord/arp/grid
  goldens from design-music section 3 examples.
- `random.rs`: `euclid 3 8`, `maybe`, `choose`, `degrade-by` reproducible under a fixed seed and pure (same span, same events).
- `region.rs` + `cover.rs`: every sample-region golden and the COVER EQUIVALENCE matrix of vactrol-core.md TASK-006
  (full vs adjacent vs overlapping vs repeated windows for `chop 2`, `chop 8`, `striate 8`, `splice`, `fit`; shared
  occurrences returned by both windows with the same `OccKey`; continuation `part.begin > whole.begin` carries its onset's
  region; identical stack twins have distinct `OccKey`s; Astra's chop-2 counterexample; striate ordinals stable across
  windows; `whole = None` under a region operator -> event-local fault; out-of-range slice index event-local;
  unsorted dynamic manual points -> query fault `bad-slice-points`); `splice`/`loop-at 2`/`fit` commit-time speed marker
  asserted with a mock bank duration and tempo.
- `faults.rs`: a mixed valid/failing query keeps sibling events and records one origin-carrying fault per failure.
- `sound.rs` (10.1 goldens through the stub `QueryVm`): `s :pluck > note [:e2 :g2 :b2]` three events per cycle;
  `s [:bd :sn] > n [0 1 2 3]` two events with `n` 0 and 2; `s :pluck > gain [0.5 1] > note [:c :e :g]` two events with
  `:c` and `:e`; `s :bd > euclid 3 8` three onsets; `s :bd > gain 0.5` one event per cycle; `kit: Some` resolves against
  the given dict and never calls `sound_kit()` (stub counts calls); `kit: None` calls `sound_kit()` per query and a stub
  kit swap between two queries changes the resolved sound; a missing key -> event-local `unknown-sound`; a `Value::Sound`
  is used as is; a bank with `n 1` picks the second sound.
- `input.rs`: `MidiNotes` yields no events under `query`; `s :pluck > midi-notes > degrade-by 1` drops every realized
  note; control/scale nodes decorate per note in tree order; a `stack` of an input lane and a queried branch realize
  independently; `fast`/`rev`/`every`/`chop` after `midi-notes`, and a structured subject (`s [:a :b] > midi-notes`), give
  `input-lane-operator`.
- `signals.rs`: signal values at exact phases; `Sig::Cc`/`Sig::Analyzer` read their cells at query time.
- `clock/tests/clock.rs`: bpm/beats-per-cycle -> cps; tempo-change anchors keep continuity; `:midi` anchor keeps logical
  positions exact under jittered synthetic pulse timestamps.
- `tex/tests/goldens.rs`: `osc 20 > rotate 0.5 > out o0` compiles to a stable shader source string (golden);
  `text "hello" > out o1` produces its `TextAsset`; uniforms resolve per frame from the `UniformPlan`; `ShaderDesc` holds
  only strings and plain data (a test asserting it is `Send + 'static`).

## Invariants

- `query` is pure relative to the read snapshot; no global state; RNG is a pure hash.
- No panic; all ratio math checked; no `std::{thread,fs,time,net,process}`; no dependency (no `smallvec`).
- Files under 800 lines (combinators are split by group for this reason).
- `pattern/` and `tex/` never edit `value/`, `ns/`, `vm/`.

## Edit Protocol

Identical to ME-MASKS "Edit Protocol (common to every ME plan)", evidence under
`tmp/me-middle-20260925-s175/ME-PATTERN/attempt-<n>/`. Parallel with ME-CHECK and ME-VM: pre/post hash `src/lib.rs`
(one-line edit) and never edit files outside this plan.

## Verification

The ME-MASKS table and log rule with `<plan>` = `pattern`: V1, V2, V3, V3t, V3f, V6a, V6b, V4 (under 800), V5, V7, V8.

## Completion Criteria (map to vactrol-core.md TASK-006)

- [ ] Step-list and mini-notation equivalence goldens pass
- [ ] `euclid 3 8`, `maybe`, `choose`, `degrade-by` reproducible and pure
- [ ] Sample-region goldens and the full cover-equivalence matrix pass
- [ ] Mixed valid/failing queries keep siblings and record origin-carrying faults
- [ ] SOUND FIRST / first-structure goldens and `kit:` resolution tests pass
- [ ] `MidiNotes` / input-lane walk tests, `Sig::Cc`/`Sig::Analyzer`, and the `:midi` clock anchor test pass
- [ ] Visual goldens (shader source, `TextAsset`, uniforms, render-safe `ShaderDesc`) pass
- [ ] V1-V8 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-006)
- **Previous**: vactrol-middle-masks.md, vactrol-middle-frontend.md
- **Next**: vactrol-middle-integrate.md
