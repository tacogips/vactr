# Vactrol Middle End: Pattern Engine, Signals, Clock, Visual Chains (ME-PATTERN) Implementation Plan

**planId**: ME-PATTERN (implements vactrol-core.md TASK-006 except the criteria that need the VM, which ME-INTEGRATE completes)
**Status**: Completed (implemented, gate-verified, adversarial review 0 blocking, integration review accepted in session 177; removed from the dispatch manifest by the session-178 amendment; source rides in the single workflow commit; archiving to impl-plans/completed/ after the workflow commit, on user confirmation)
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

- [x] Step-list and mini-notation equivalence goldens pass (`src/pattern/tests/steps.rs`, 14 tests)
- [x] `euclid 3 8`, `maybe`, `choose`, `degrade-by` reproducible and pure (`src/pattern/tests/random.rs`, 5 tests)
- [x] Sample-region goldens and the full cover-equivalence matrix pass (`region.rs` 6 tests, `cover.rs` 4 tests)
- [x] Mixed valid/failing queries keep siblings and record origin-carrying faults (`faults.rs`, 7 tests)
- [x] SOUND FIRST / first-structure goldens and `kit:` resolution tests pass (`sound.rs`, 9 tests)
- [x] `MidiNotes` / input-lane walk tests, `Sig::Cc`/`Sig::Analyzer`, and the `:midi` clock anchor test pass
  (`input.rs` 6, `signals.rs` 4, `src/clock/tests/clock.rs` 4)
- [x] Visual goldens (shader source, `TextAsset`, uniforms, render-safe `ShaderDesc`) pass (`src/tex/tests/`, 12 tests)
- [x] V1-V8 pass with logs cited; `final-hashes.txt` written (session 177 progress log)

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

### Session: 2026-09-25 (session 177, ME-PATTERN implementer)

**Tasks Completed**: every ME-PATTERN deliverable.
- `src/pattern/`: `pat.rs` (`Pat`, `PatNode`, `PParam`, `SliceCuts`, structural node ids), `step.rs` (`Step`, step lists,
  squeeze, `Pure`), `query.rs` (`TimeSpan`, `Event`, `Controls`, `QueryResult`, `query`, dispatch), `occ.rs` (`OccKey`),
  `rng.rs` (splitmix hash RNG), `signal.rs` (`Sig`), `eval.rs` (`QueryVm`, `QueryCtx`, `InputCells`, `AnalyzerId`,
  `HostSig`, parameter evaluation, bounds), `build.rs` (`pure`, `signal`, `pattern_of`, `param_of`, `alt`,
  `midi_channel`), `combinators/{mod,time,structure,random,region,music,control,input,sound}.rs` (builders and queries
  for the whole TASK-006 vocabulary), `tests/{mod,stub_vm,steps,combinators,random,region,cover,sound,input,signals,faults}.rs`.
- `src/clock/`: `tempo.rs` (`Tempo`, exact cps), `clock.rs` (`Clock` with piecewise-linear anchors, `ClockSource`,
  `MidiClockSync` for `:midi`), `tests/clock.rs`.
- `src/tex/`: `texnode.rs` (`TexNode`, `TexKind`, `BlendOp`, `ModKind`, `VParam`, `pipe`, the M1 `scale_texture` /
  `shape_source` builders; `OutId` kept), `shader.rs` (`ShaderDesc`, `TextAsset`, `compile_tex`: GLSL ES 3.0 from a
  fixed 39-operator snippet library), `uniforms.rs` (`UniformPlan`, `UniformSpec`, `Uniforms`, `resolve_uniforms`),
  `tests/{mod,goldens}.rs`. Written by a rust-coding subagent under this plan's write scope and reviewed here.
- sharedPaths: `src/lib.rs` gains exactly `pub mod clock;` (crate doc untouched); `src/pattern/mod.rs` and
  `src/tex/mod.rs` declare the new modules.

**Design and plan differences** (design 10.1 wins; ME-INTEGRATE should read these):
- `Pat` has a fourth field `id: NodeId`, a structural hash of the node, so a rebuilt identical pattern keeps its random
  draws and occurrence keys. It is the "node id" of 10.2/10.3.
- `PatNode` concrete shapes: `Control(name, value, subject)`, `Grid(subject, bools)`, `Euclid(subject, k, n, rot)`,
  `ScaleNotes(root, name, subject)`, `Chord(chord values, subject)`, `Voicing(subject)`, `Arp(subject, mode)`. Added:
  `Pure(Step)` (one atomic value per cycle, so a chord list `[:c :m7]` is not subdivided), `Hurry`, `Maybe`, `Choose`,
  `Hold`, `Repeat` (the step constructors the design names in prose). `alt` builds `Cat`. `sometimes`/`rarely`/`often`
  are `SometimesBy` with 0.5/0.25/0.75.
- `OccKey.path` is a `Vec` (no `smallvec`). `query` fills `anchor`/`cycle` at the end.
- `QueryCtx` also carries `tempo: Tempo` (for `time`, `beat` and fault beats). Fault origins carry the node or step span
  and the beat (`anchor * beats_per_cycle`); `slot` stays `None` until the scheduler fills it.
- Parameter timing: structural parameters (`fast`, `iter`, `every`, `whenmod`, `chunk`, `euclid`, `segment`,
  `repeat`, `off`) are read once for a constant and at each cycle start otherwise; per-event parameters (probabilities,
  chop/ply/striate counts, slice cuts and index, `range` bounds, `arp` mode) at the event's `whole.begin`.
- First-structure rule: the builders set `structured`; a `Control`/`Chord` query reads its CHILDREN's flags (fixed at
  construction) to pick the mode. A control whose value has a rest at the subject onset drops the event (Tidal `#`).
- `slice`/`splice` with a structured index list on an unstructured subject take the structure from the index (the
  first-structure rule applied to the list, needed for design-music `s :break > slice 8 [0 2 4 7]`); otherwise timing
  is unchanged and the index is sampled at `whole.begin` (10.1).
- Region controls are exact `Value::Ratio` `begin`/`end` (chop composes with an existing region). The commit-time rate
  fit is the `speed-fit` marker control (`[:splice fraction cycles]`, `[:fit cycles]`, `[:loop-at n]`) resolved by
  `SpeedFit::resolve(sample_seconds, cycle_seconds)`; `loop-at` also sets `loop 1` and the full region.
- A bank (a list of sounds from the kit) is picked by `n` (default 0) when `query` finalizes, so the `n` control may come
  after `s`. An index outside the bank is event-local `slice-index`.
- `sometimes-by` draws independently on the original and the transformed events (not Tidal's complementary halves);
  `off` applies `f` and then shifts. Both are equal for the documented examples.
- Note numbers are `12 * octave + pitch class`, default octave 5 (`:c` = 60). `voicing` is close position with the first
  tone in `[60, 72)`. `chord_pattern_of` reads a `[root quality]` list (also under `alt`/`choose`/`stack`) as ONE chord.
- `input_lane_walk` returns `Result<LanePlan, LaneError { code, span: Option<Span>, message }>` (patterns built in tests
  have no span). `realize_note(&Lane, &LiveNote, &mut QueryCtx) -> Result<Option<Event>, Failure>`; the realized event
  has `whole = None`, `part = [at, at]`, `note` and `velocity` controls. `sometimes-by` on a lane runs the transform on
  a one-note pattern.
- `Sig::Lag` reads the inner signal (smoothing state belongs to the frame evaluator, TASK-007/010);
  `Sig::MapRange(Rc<Sig>, f64, f64)`; `Sig::Ctrl`/`Hits` read telemetry cells in `InputCells`.
- Bounds (7.1.5): query tree depth 256, 64 nested re-entries (VM calls and point samples), 1,000,000 work units per
  query; each is a fault (`depth-exceeded`, `fuel-exhausted`), never a panic or a hang.
- `Clock::set_tempo` fails with "clock is external" when slaved; 64 anchors are kept. `ClockSource::Link` is
  represented, `is_available()` is false (the diagnostic is the checker's).
- `tex`: `resolve_uniforms(plan, frame_time: Ratio64 /* cycles */, cx) -> Result<Vec<(String, f32)>, Failure>` (the first
  failing uniform fails the frame; the renderer keeps the previous values). Each spec is forced exactly once per call.
  Const params are inlined; every other `VParam` is `u0, u1, ...`. Text sources use `u_text<id>` samplers and
  `src oN` uses `u_outN`. Host builtins `time` and `resolution` are not in `uniform_names`. Composition order is
  geometry and modulate ops on the coordinate, then color and blend ops on the color (Hydra's shape, simplified).
  "A failing chain leaves the previous program in place" is the slot table's activation (TASK-007); here a failing
  chain is an `Err` from `compile_tex` (tested).
- Deferred to ME-INTEGRATE, as planned: `PParam::Late` through a real VM (the stub cannot build a `VarSlotRef`), the
  tweaked-probability re-query, a session `sound-kit` heard through `Vm`, and the native wrappers.

**Evidence** (`tmp/me-middle-20260925-s175/ME-PATTERN/attempt-1/`): `pre-edit-hashes.txt`, `intent.md`,
`post-edit-hashes.txt`, `inline-checks.txt`, `owned-rs.txt`, `final-hashes.txt`. Drift event: while this plan ran, a
concurrent worker outside ME-PATTERN rustfmt-reformatted several `src/pattern`/`src/clock` files (whitespace only; the
tex subagent confirmed it ran no formatter). The content was intact and every test still passed; no reapply was needed.
While other plans' files were mid-edit, compile checks ran in `tmp/.../ME-PATTERN/scratch` (their files reverted to
HEAD there); the rows below ran on the real shared tree.

**Verification** (session 177, real tree, default target dir):
- V1 `target/fe-logs/pattern-build-s177-2.log`: `exit=0`, no warnings.
- V2 `target/fe-logs/pattern-clippy-s177-4.log`: `exit=0`. Runs 1-3 (`pattern-clippy-s177-{1,2,3}.log`, `exit=101`)
  failed only on `needless_return` in ME-VM's in-progress `src/vm/vm.rs:304-320`. Per the edit protocol that file was
  not touched; the scratch copy showed ME-PATTERN's files clippy-clean, and run 4 passed once ME-VM fixed its file.
- V3 `target/fe-logs/pattern-nextest-s177-2.log`: 400 run, 400 passed, 0 failed, `exit=0`.
- V3t `target/fe-logs/pattern-cargotest-s177-2.log`: lib 392 passed, spec_fixtures 8 passed, 0 failed, `exit=0`.
- V3f `target/fe-logs/pattern-fixtures-s177-2.log`: 8 run, 8 passed, `exit=0`.
- V6a `target/fe-logs/pattern-wasm32-s177-2.log`: `exit=0`. V6b `target/fe-logs/pattern-wasm32-hostwasm-s177-2.log`: `exit=0`.
- V4: largest in the tree `src/compile/compiler.rs` 770 (ME-VM); largest owned `src/tex/shader.rs` 571; all under 800.
- V5: `none`. V7: empty (no dependency change). V8: `rustfmt --edition 2021 --check` on the 40 owned `.rs` files
  (`owned-rs.txt`), exit 0.
- Earlier n=1 rows (`pattern-{build,nextest,cargotest,fixtures,wasm32,wasm32-hostwasm}-s177-1.log`, all `exit=0`) ran
  before a final doc-comment-only edit in `src/clock/mod.rs`; the n=2 rows above are the counting logs.
- Owned test counts: pattern 64 (`steps` 14, `combinators` 9, `random` 5, `region` 6, `cover` 4, `sound` 9, `input` 6,
  `signals` 4, `faults` 7), clock 4, tex 12.

**Blockers**: none owned by this plan. Formal review, ME-INTEGRATE and the workflow commit are downstream.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-006)
- **Previous**: vactrol-middle-masks.md, vactrol-middle-frontend.md
- **Next**: vactrol-middle-integrate.md

### OUTPUT CONTRACT NOTE (operator, 2026-09-25, after the ME-PATTERN attempt-1 failure)

- `planId` belongs ONLY in the step6-implement output payload. The
  step6-test-integrity-check and step7-adversarial-review outputs MUST NOT
  contain `planId` (their contracts reject additional properties; ME-PATTERN
  attempt 1 failed with "output contract $.planId additional property is not
  allowed" after a green gate).
- The adversarial-review output MUST contain the `findings` array (empty when
  none) and the integration-review output MUST contain `needs_revision`.

### Session: 2026-09-25 (session 178, attempt-2 re-verification)

**Context**: step6 redispatch after attempt 1 was rejected by a later step's output contract (`$.planId` additional
property). No implementation defect was reported, and the review feedback is empty.
**Source changes**: none. `shasum -a 256 -c attempt-1/final-hashes.txt` matches every owned source file; only this plan
document differs (operator note plus this entry). Evidence: `tmp/me-middle-20260925-s175/ME-PATTERN/attempt-2/`
(`intent.md`, `pre-verify-hashes.txt`, `final-hashes.txt`). No drift occurred during verification.
**Verification** (combined tree with ME-CHECK and ME-VM present, all `exit=0`):
- V1 `target/fe-logs/pattern-build-s178-1.log`; V2 `pattern-clippy-s178-1.log`.
- V3 `pattern-nextest-s178-1.log`: 409 run, 409 passed. V3t `pattern-cargotest-s178-1.log`: lib 401 passed, spec_fixtures
  8 passed, 0 failed. V3f `pattern-fixtures-s178-1.log`: 8 run, 8 passed.
- V6a `pattern-wasm32-s178-1.log`; V6b `pattern-wasm32-hostwasm-s178-1.log`.
- V4: the largest file in the tree is `src/compile/compiler.rs` at 792 lines, and every file is under 800. V5: `none`.
  V7: empty. V8: `rustfmt --edition 2021 --check` on the 40 owned files, exit 0. Owned tests (`cargo test --lib --
  pattern:: clock:: tex::`): 80 passed.
**Blockers**: none. Formal review, ME-INTEGRATE and the workflow commit are downstream.
