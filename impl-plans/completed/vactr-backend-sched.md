# Vactr Back End: Scheduler, Slot Table, Runtime, Dry Run (BE-SCHED) Implementation Plan

**planId**: BE-SCHED (vactr-core.md TASK-007 except MIDI input/clock/note lifetime, which BE-MIDI completes)
**Status**: Completed (accepted by integration review; reconciled by BE-FINAL session 186; archive after the workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 11.2, 11.3 (all of it), 11.4, 11.5, 11.6, 10.3, 10.4, 18, 12.8.1, 12.8.3, 12.8.4, 12.8.5, 12.8.7
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactr/issues/3
**dependsOn**: BE-CONTRACTS
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json

---

## Intent and Context

TASK-007 turns bound patterns into timed host events. This plan builds `sched::Runtime` (12.8.3), which owns the slot
table, the two-horizon scheduler (query horizon with staging, commit horizon with POD conversion), the occurrence merge
and committed ledger, the per-slot two-class `SlotControl` channel with monotone merge and re-send, the authoritative
`ControlCells` with both tier transports, the dry run, telemetry, `once`/`at`, tempo change and captured-output
forwarding. The semantics are fixed by design 11.3; this plan says where they live and how they are proven. Inputs:
`Evaluator` (`src/ns/evaluator.rs`, `vm_and_ns`), `StagedEffect`/`EffectSink` (`src/ns/stage.rs`), `QueryVm`/`VmQuery`
(`src/pattern/eval.rs`, `src/vm/query_vm.rs`), `query` (`src/pattern/query.rs`), `Clock` (`src/clock/clock.rs`), and the
BE-CONTRACTS types (`host/caps.rs`, `host/wire.rs`, `host/testing.rs`, `dsp/cells.rs`, `dsp/controls.rs`).
BE-DSP and BE-INST run at the same time; this plan uses a stub `InstResolver` in tests.

## Non-Goals

- No MIDI input draining, `midi-notes` realization, note lifetime, clock slave/master or transport (BE-MIDI). `use-clock`
  and `midi-clock-out` effects are STORED in `Runtime` state and not acted on; `MidiInHost` is never polled here.
- No DSP rendering, no instrument realization, no real hosts. Visual per-frame uniforms are out of scope (12.8.1).
- No edits outside the writePaths below; no `mod.rs` edits (the skeleton exists).

## writePaths (exclusive)

- `src/sched/slots.rs`, `runtime.rs`, `staging.rs`, `ledger.rs`, `commit.rs`, `control.rs`, `cells.rs`, `dryrun.rs`,
  `telemetry.rs`, `oneshot.rs`
- `src/sched/tests/sched.rs` and new files under `src/sched/tests/sched/` (declared by `sched.rs`)
- `src/pattern/step.rs`, `src/pattern/combinators/control.rs` (fill `Event::late` / `Event::cells`, item 9)
- `impl-plans/active/vactr-backend-sched.md`

## sharedPaths

None.

## File-Level Changes (signatures only)

1. `slots.rs` (11.2, 12.8.3): `SlotKind { Pattern, Texture }`; `Binding { Pattern(Rc<Pat>), Texture(Rc<TexNode>) }`;
   `Slot { key: SlotKey, kind, bound: Option<Binding>, pending: Option<(Binding, Ratio64 /*boundary*/)>, gen: u32, orbit: u8, muted: bool }`;
   `SlotTable` (d1..d9 == slot 1..9, named slots, `out o0..o3` as texture slots; `get`, `get_or_insert`, `iter`, `hush_all`).
2. `runtime.rs`: `RuntimeConfig { lookahead: f64 = 0.120, commit_lead: f64 = 0.030, resend_ticks: 3, transport_diag_ticks: 20, late_threshold: 4, widen_step: 0.010, widen_cap: 0.200, cell_pool: 1024, tier: Tier }`,
   `Tier { Native(AtomicCells), Browser }`; `Runtime::new(hosts: Hosts, resolver: Rc<dyn InstResolver>, caps: CapabilitySet, cfg) -> (Runtime, RuntimeSink)`;
   `RuntimeSink: EffectSink` pushing into `Rc<RefCell<CommandQueue>>`; `drain(&mut self, ev: &mut Evaluator) -> Vec<Diagnostic>`
   (SlotBind -> dry run -> pending or diagnostics; Revoke -> stop/hush; CellUpdate/TweakRefresh/Bindings -> cell write +
   staging invalidation; Tempo(Bpm|Cycle) -> tempo change; Tempo(Clock|MidiClockOut) -> stored; OneShot -> oneshot queue;
   Console -> forwarded; Install -> `AudioHost::swap_graph` + `CellInit` for cell-backed defaults);
   `tick(&mut self, ev: &mut Evaluator, host_now: f64) -> TickReport { diags, console: Vec<Rc<str>>, faults: Vec<Failure> }`
   running 11.3 steps (1)-(6), sampling `InstResolver::signal_inputs()` once per tick into their cells, draining
   `HostMsg` acks, re-sending unacked controls and cell messages, and auto-widening `commit_lead` on late counters
   (`latency-widened`); `telemetry(&mut self) -> Vec<PlayingEvent>`; `input_cells(&mut self) -> &mut InputCells`;
   `slot_gen(key) -> Option<u32>`; `pub(crate) fn invalidate(&mut self, slot: SlotKey, span: TimeSpan)`.
3. `staging.rs` (11.3 layer 2): `OccRecord { key: OccKey, whole: TimeSpan, payload: Event, covered: SmallCoverage, onset_committed: bool, output: Vec<Rc<str>> }`
   per (slot, gen); `merge_fragment` = coverage UNION only, never delete, never replace; `invalidate(span)` removes
   coverage inside the span, drops a record only when its `whole.begin` is in the span and its key is absent from the
   re-query, and replaces a surviving in-span record's payload wholesale from the new snapshot (semantic payload
   replacement); records whose onset is outside the span are only re-extended. Queries are split at every cycle
   boundary; a slot with `pending` queries the old binding before the boundary and the new one at or after it
   (prospective activation). Captured query output is attached to the query fragment (12.8.3).
4. `ledger.rs`: committed-occ set per (slot, gen) bounded to the query horizon; commit consults it so an occurrence emits
   at most once.
5. `commit.rs` (11.4, 12.8.7): `commit(record, slot, gen, clock, cells, resolver) -> Result<Committed, Failure>`:
   seconds conversion once via `Clock`; `Route` from `resolver.route(sound)`; control mapping through
   `dsp::controls` (`note`/`n` through the scale to `freq`, `gain` to `amp`, keyword/bool encoding, `orbit`/`bus`/`cut`/`legato`
   routing); every control listed in `Event::cells` becomes `Ctl::Cell(id)` (cell allocated on first reference), EXCEPT
   when the incarnation is not yet acked on the browser tier, where it becomes `Ctl::Const(current value)` (the
   documented downgrade); more than `MAX_CTLS` controls is `Failure(too-many-controls)`; `MidiEvent`/`OscEvent` bake values
   at transmission (the documented sink deviation). Granular admission (12.6, 12.8.8): a `density`, `size` or capture
   depth above the tier cap is checked with `CapabilitySet::require` at commit and reported as a diagnostic with the
   event's origin (the event still commits; the audio side clamps spawning). A per-event failure drops only that event
   and its held output.
   Sample resources (16.1 "the scheduler references a resource only after that ack"): `commit.rs` also holds
   `SampleTable` (`SampleSrc -> resource id`, state `Loading | Installed | Retiring`). After a successful bind dry run,
   `drain` requests every sample and bank referenced by the dry-run events (`Route::Audio { sample: Some(..) }`) through
   `Hosts::samples` and `AudioHost::install_sample`; `Installed` moves the entry to `Installed`. An event whose resource
   is not yet `Installed` (first use of a sample not seen by the dry run) is dropped as an event-local
   `Failure(host-unavailable)` "sample `x` is still loading", reported once per resource, and the load is requested. The
   resource id is written into the event's `bank` control at commit. For `SampleSrc::Bank`, commit sets `index` from the
   event's `n` control (default 0) before the lookup (7.1.4).
6. `control.rs` (11.3): per slot one immediate entry (monotone merge via `SlotControl::merge`) and one future entry
   (replaces only itself), delivered in `effective_time` order to all three sinks of a pattern slot; re-send after
   `resend_ticks` until `SlotControlAck`; `host-transport` diagnostic after `transport_diag_ticks`; event batches carry
   the current gen (piggyback). Rebind is deadline-aware (three cases of 11.3: sufficient, insufficient with flagged
   reduced lead, delivery failure density-bounded).
7. `cells.rs` (11.3): `ControlCells` (authoritative values, per-id epochs, pool of `cell_pool`), browser protocol:
   `CellInit` on first reference or inst install, ack tracking, per-tick `CellBatch` coalesced latest-wins, at most one
   in-flight + one pending batch, seq monotone, full-snapshot resync (`resync()`: `CellInit` only for never-initialized
   incarnations, then one snapshot batch stream), retire + epoch-gated reuse; native: release-store into `AtomicCells`.
   Pool exhaustion commits `Ctl::Const` and reports one `beyond-capability` "control cell pool exhausted".
8. `dryrun.rs` (11.5): `dry_run(b: &Binding, vm: &mut dyn QueryVm, cells: &InputCells, seed: u64) -> QueryResult` (one cycle,
   Query effect mode; textures run `compile_tex`); `telemetry.rs`: `PlayingEvent` (11.6 fields incl. `kind`), bounded
   queue, per-slot rolling window writing `InputCells` `hits`/`ctrl`; `oneshot.rs`: `once` binds an ephemeral slot for
   one iteration from now or `at:`; `at` thunks run in Normal mode at their beat via the evaluator; tempo change
   re-anchors the clock, bumps every gen with `effective_time: now`, clears staging, re-queries.
9. `pattern/step.rs`, `pattern/combinators/control.rs`: when a step value is a direct `Value::VarRef` resolved to a scalar,
   set `Event::late`; `pair_up`/`query_mapped` copy a value event's `late` into the result's `cells` under the control
   name. Query results are otherwise unchanged (all existing pattern tests keep passing).
10. Visual slots: activation calls `RenderHost::set_program`; `stop`/`hush` call it with an empty `ShaderDesc`.
11. Faults: query faults are forwarded with slot + beat immediately; a slot's diagnostics clear after a clean cycle.

## Required Tests (`src/sched/tests/sched/*.rs`; mock clock, recording hosts, explicit cycle duration and transport delay)

Each maps to a vactr-core.md TASK-007 criterion; assert EMITTED MULTIPLICITY at the recording host, never set equality.
- `merge.rs` (criterion 1): exact ratio positions; queries split at every boundary; sufficient-lead rebind swaps only at
  the boundary; region partition invariance through staging (`chop`, `striate` staged across ticks, boundary splits and a
  control-write re-stage equal the full-cycle dry run); overlapping windows [0,3/4)+[1/4,1) commit onset 1/2 once
  (including the partial-commit variant); onset preservation under clipping (whole [1,2) under `chop 2`, both windows
  staged pre-commit, both orders: onsets 1 and 3/2 once each); boundary-crossing fragments in both orders; partial
  invalidation of [5/4,2); repeated window; identical two-branch stack commits both; semantic payload replacement
  (`slice` index 0 -> 1: same OccKey and gen, one emission, region [1/2,1), no old-region output); continuation-only
  invalidation leaves the onset payload unchanged.
- `rebind.rs` (criterion 2): the three deadline cases with `B - now` chosen against `commit_lead + L_ctl`; three
  simultaneous old-gen boundary events each cut on control receipt; missed deadline reported.
- `control.rs` (criteria 3, 4, and 10 for pattern and texture slots): stop/hush/tempo/set-tweak (value; `maybe` 0->1 restores,
  `degrade-by` 0->1 removes) across boundaries, `once`/`at`; audio/MIDI/OSC slot+gen identity; hush-then-stop Panic,
  hush-then-tempo Panic + gen, hush-then-rebind both entries in order; duplicate delivery idempotent; MIDI stale-started
  note gets an immediate note-off (recording MIDI host); OSC revocation only on the untransmitted queue; lost future
  control with a gen-carrying batch first triggers only the default piggyback policy; repeated loss exercises re-send
  and the transport threshold; hush/stop new events cease within control latency; texture slot program cleared.
- `cells.rs` (criterion 5): every case of the criterion, native via `NativeTransport` and browser via
  `BrowserTransport` (delayed batch, first-use-before-init `Const`, init replay after update, reuse Vacant -> Live,
  pre-ack Const exception, stalled-consumer burst bounded to 1+1 and converging, stale epoch after reuse inert,
  reconnect snapshot before new voice starts, MIDI/OSC baking).
- `output.rs` (criterion 8): captured `print` reaches the console only at commit; restage replaces held output; a
  write after commit leaves emitted output untouched.
- `dryrun.rs` (criterion 9): success and failure leave namespace, slots, staging, pending queues and host outputs
  unchanged (snapshot compare); failure keeps the old binding; dry-run `print` only in the report.
- `granular.rs` (TASK-008 criteria 5 and 6, diagnostic half; 12.6 "Admission"): with `CapabilitySet::browser()`, a
  committed event whose `density` exceeds `max_grain_density`, one whose `size` exceeds `max_grain_size`, and one whose
  capture depth exceeds `max_capture_seconds` each give ONE `beyond-capability` diagnostic whose origin carries the
  event's span, slot and beat, and each event still reaches the recording audio host (no dropout; the audio side clamps).
  Together with BE-DSP's clamp-and-count tests (`src/dsp/tests/dsp/granular.rs`, `analyzer.rs`), this proves criteria 5
  and 6.
- `faults.rs` (criterion 11): mixed valid/failing events: valid play, each fault has slot + beat, diagnostics clear after
  a clean cycle.

## Invariants

- Logical time is `Ratio64` everywhere in `sched/`; `f64` seconds only in `commit.rs` and host-facing calls.
- No `std::{thread,fs,time,net,process}` in `sched/`. No panic on any input; ratio overflow is a fault.
- Every existing test keeps passing (pattern changes are additive). No `.rs` file reaches 800 lines.

## Edit Protocol

The common protocol of `vactr-backend-contracts.md` "Edit Protocol (common to every BE plan)", evidence under
`tmp/be-backend-20260925-s181/BE-SCHED/attempt-<n>/`. BE-DSP and BE-INST edit the same tree concurrently; their files are
disjoint from this plan's.

## Verification

The common table of `vactr-backend-contracts.md` with `<wave>` = `sched`: V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8.
Plus S1: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/sched::tests/)'`
as LOG(`be-sched-own`), `exit=0`, run count cited per test file.

## Completion Criteria (map to vactr-core.md TASK-007)

- [x] `Runtime`, `RuntimeSink`, `SlotTable`, staging/merge/ledger, commit, control channel, cells, dry run, telemetry,
      `once`/`at`, tempo change, captured output implemented as listed
- [x] TASK-007 criteria 1, 2, 3, 4, 5, 8 (captured print), 9 (dry run), 11 (mixed faults) proven by the tests above
      (criteria numbered in order of the TASK-007 checklist; 6 and 7 are BE-MIDI's, 12 is BE-FINAL's)
- [x] Criterion 10 (hush/stop per release class) proven for pattern and texture slots; the open-input-voice half is BE-MIDI's
- [x] TASK-008 criteria 5 and 6, diagnostic half (granular admission with origin, event still commits) proven by
      `src/sched/tests/sched/granular.rs`
- [ ] V1-V8 and S1 pass with logs cited; `final-hashes.txt` written (session 182: V1, V3f, V4, V5, V6a, V6b, V8 and S1
      pass and `final-hashes.txt` is written; V2, V3 and V3t fail only in BE-DSP/BE-INST files that are still being
      written, 0 findings and 0 failures in this plan's files; re-run at the wave join / BE-FINAL)

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-SCHED implementer)` entry: work done, design differences, hash
and intent paths, evidence per row, blockers. Edit only this log.)

### Session: 2026-09-25 (session 182, BE-SCHED implementer)

**Work done** (every file inside this plan's writePaths; no `mod.rs` edited):
- `slots.rs` (241 lines): `SlotId`/`CtlId` kept; `SlotKind { Pattern, Texture }`, `Binding::{Pattern, Texture}` with
  `from_value`, `Slot` (key, id, kind, bound, pending `(Binding, boundary)`, gen, orbit, muted, ephemeral, per-generation
  lanes, fault bookkeeping), `SlotTable` (d1..d9 = ids 1..9, named slots and `o0..o3` from 10, `get`, `get_or_insert`,
  `iter`, `remove`, `hush_all`).
- `staging.rs` (634): `Coverage` (coalesced interval list), `OccRecord`, `Fragment`, `Dirty`, `Staging`
  (`merge_fragment` = coverage union only; `invalidate` + `replace` = scoped removal and semantic payload replacement;
  `truncate_from` for rebind; `emittable`; `mark_committed`; `expire`), `Lane` (one per (slot, gen): `[from, until)`,
  offset for `once`, `queried_to`, ledger, dirty spans, `once` overrides), `query_lane`/`extend`/`query_span` (queries
  split at every cycle boundary of pattern time).
- `ledger.rs` (45): per-lane emitted-occurrence set, expiring when the position passes the whole.
- `commit.rs` (631): `commit` (seconds once through `Clock`; `Route` per event; `note`/`n` through the scale to `freq`,
  `gain` -> `amp` id, keyword/bool encoding, `orbit | cut << 8` in `voice_hint`; late controls -> `Ctl::Cell` via
  `ControlCells` or the pre-ack `Ctl::Const`; `too-many-controls`; MIDI/OSC baked at transmission; chords give one event
  per tone), `SampleTable` (`Loading | Installed | Retiring | Failed`, requested after a successful bind dry run, first
  unseen use dropped with ONE `host-unavailable` "still loading", `bank` control = resource id, bank `index` from `n`),
  granular admission (`density`, `size`, live capture depth -> `CapabilitySet::require`, diagnostic with span + slot +
  beat, event still commits), and the tick's commit pass (`commit_all`, `send`).
- `control.rs` (350): `ControlChannel` (immediate entry = monotone `SlotControl::merge`, future entry replaces only
  itself, delivered to audio + MIDI + OSC in effective-time order, re-send every `resend_ticks`, `host-transport` after
  `transport_diag_ticks`, missed boundary deadline reported once), `Runtime::rebind` (next cycle boundary, prospective
  lane, old staged work at/after the boundary discarded, boundary control for a slot that already played) and
  `Runtime::revoke` (stop Natural / hush Panic, texture outputs set to the empty program), MIDI stale-start note-off.
- `cells.rs` (543): `ControlCells` (authoritative values, per-id epochs, pool, ack-gated `CellInit`, per-tick coalesced
  `CellBatch` with at most one in flight + one pending, re-send capped at the transport threshold, `resync` snapshot,
  retire + epoch-gated reuse, pool exhaustion -> `Const` + one `beyond-capability`), `CellPort`, `Tier`, `CellKey`,
  `CellMap`.
- `dryrun.rs` (34), `telemetry.rs` (101: `PlayingEvent` incl. `kind` and `reduced_lead`, bounded queue, per-slot window
  writing `hits`/`ctrl`), `oneshot.rs` (221: `AtQueue`, `run_thunk` in Normal mode, `Runtime::tempo` re-anchor + gen bump
  + restage, `Runtime::one_shot` ephemeral one-cycle lane shifted to its start).
- `runtime.rs` (718): `RuntimeConfig` (12.8.4 defaults + `tier` + `seed`), `CommandQueue`, `RuntimeSink`, `DrainReport`,
  `TickReport`, `Runtime::{new, drain, tick, telemetry, input_cells, slot_gen, invalidate, resync_cells, ...}`; `drain`
  applies SlotBind (dry run -> pending or faults), Revoke, CellUpdate/TweakRefresh/Bindings (cell write + invalidation of
  every uncommitted span), Tempo (Bpm/Cycle applied; Clock/MidiClockOut stored for BE-MIDI), OneShot, Console, Install
  (cell-backed defaults initialized, then `swap_graph`); `tick` runs 11.3 steps (1)-(6), drains `HostMsg` acks
  (controls, cells, samples, late counters, analysis cells), samples `InstResolver::signal_inputs()`, and auto-widens
  `commit_lead` (`latency-widened`). `#[cfg(test)]` hooks: `stage_span`, `requery_dirty`, `commit_only`, `queued`.
- `pattern/step.rs`, `pattern/combinators/control.rs` (item 9): a step that is a direct `VarRef` to a slot holding a
  plain value sets `Event::late`; `pair_up` returns the value event's `late` and `query_mapped` copies it into
  `Event::cells` under the control name when the mapped value is unchanged (a chord's mapped tones are not cells).
  Query results are otherwise unchanged; all pre-existing pattern tests pass.

**Tests** (`src/sched/tests/sched.rs` rig + 8 files, 47 tests, emitted multiplicity asserted at the recording hosts;
`played()` models the 11.3 sink contract with an explicit control delay): merge.rs 12 (criterion 1), rebind.rs 3
(criterion 2), control.rs 13 (criteria 3, 4, 10 incl. texture), cells.rs 9 (criterion 5, both tiers), output.rs 2
(criterion 8), dryrun.rs 2 (criterion 9), granular.rs 2 (TASK-008 criteria 5/6 diagnostic half), faults.rs 4
(criterion 11, sample table, telemetry). A mutation check (coverage replace instead of union) fails 8 merge tests.

**Design differences** (the design wins where it is explicit; these fill gaps):
- `Tier::Browser(Box<dyn CellPort>)` instead of a bare `Browser`: `AudioHost::post` carries one fixed-size record and a
  `CellBatch`'s entries follow its header in the byte stream, so cell records need their own port (12.8.5 "only the
  transport differs"). BE-WASM implements `CellPort` over its main-half channel.
- `drain` returns `DrainReport { diags, faults, dry_output, console }`: dry-run faults are `Failure`s (no `DiagCode`
  maps them); dry-run `print` stays in `dry_output`.
- Captured output is held per occurrence ONSET (10.4 "keyed by its event's identity"): a window with occurrences may
  re-evaluate continuing events, so its own output is dropped and a point query at each new uncommitted onset captures
  that event's lines. Windows without occurrences keep their output (forwarded when they pass the commit horizon).
  Overlapping polyphony at the same point shares the point query's lines (documented limit).
- A rebind always lands at `ceil(pos)` (11.2); a fresh slot at a boundary starts at once. `once` starts at the commit
  horizon or `pos + at/beats_per_cycle`; an `at` body runs when its position is reached and a `once` it stages starts at
  that tick's commit horizon.
- `orbit`/`cut` travel in `voice_hint` (`orbit | cut << 8`); `bus` and audio `legato` have no `AudioEvent` field (legato
  scales MIDI `dur`); a later wave adds them if the engine needs them.
- MIDI stale-start recovery is issued by the scheduler (`MidiEvent::NoteOff` for committed older-gen notes at/after a
  control's effective time), since `MidiHost` has no ack path; OSC irrevocability is the sink's queue rule.
- Live granular capture depth = `max(size, position)` seconds when a `source` control is present.
- Runtime site cells take ids `0..min(cell_pool, INST_CELL_BASE)`, below BE-INST's instrument range (896..1024).
- `Runtime::new` builds the clock with `expect` on the default tempo (cps 1/2, cannot fail on any input).
- Rust was written by this plan's single implementer (one owner for the cross-file contracts, as BE-CONTRACTS did);
  checks were run directly with logged gates instead of the check-and-test-after-modify agent.

**Cross-wave notes for BE-FINAL** (not edited here): `InstResolver` has no `default_cells`, so a cell-backed inst default
is initialized with its control row default at `Install` and is not rewritten on `TweakRefresh`; BE-MIDI owns the
`use-clock`/`midi-clock-out` behavior stored in `Runtime::{clock_request, midi_clock_out}`.

**Evidence** (`tmp/be-backend-20260925-s181/BE-SCHED/attempt-1/`: `pre-edit-hashes.txt`, `post-edit-hashes.txt`,
`intent.md`, `notes.md`, `owned-rs.txt`, `v4.txt`, `v5.txt`, `v8.txt`, `scratch-source-hashes.txt`,
`runtime.pre-split.rs`, `final-hashes.txt`; no drift event: no other plan wrote these files). Logs in `target/fe-logs/`:
- V1 `be-sched-build-s182-2.log` exit=0 (run 1 exit=0)
- V2 `be-sched-clippy-s182-2.log` exit=101: every finding is in BE-DSP files (`dsp/effects/*`, `dsp/engine.rs`,
  `dsp/ring.rs`, `dsp/ugen/mod.rs`, `dsp/tests/dsp.rs`); 0 in this plan's files. Isolated copy
  (`be-sched-clippy-scratch-s182-1.log`, BE-DSP files stubbed): findings only in BE-INST's `dsp/build.rs`/`ns/insts.rs`.
- V3 `be-sched-nextest-s182-1.log` exit=100: 623 tests, the only failure is BE-INST's
  `types::tests::natives::required_names_present_and_out_of_scope_absent` ("bus must be absent");
  `be-sched-nextest-nofailfast-s182-1.log`: 623 run, 622 passed, 1 failed (the same). Run 2 (`-s182-2`) could not
  compile tests: BE-DSP's `src/dsp/tests/dsp.rs` declares test files not yet written.
- V3t `be-sched-cargotest-s182-1.log` exit=101: lib 612 passed, 1 failed (the same BE-INST test).
- V3f `be-sched-fixtures-s182-1.log` exit=0: 10 run, 10 passed, 1 skipped.
- S1 `be-sched-own-s182-1.log` exit=0: 47 run, 47 passed (merge 12, rebind 3, control 13, cells 9, output 2, dryrun 2,
  granular 2, faults 4). Current source (after the last change, the cell-pool cap): `be-sched-own-scratch-s182-1.log`
  exit=0, 47/47, on an isolated copy whose owned files hash-match the shared tree (`scratch-source-hashes.txt`);
  `be-sched-scratch-full-s182-1.log`: 569 run, 568 passed, the 1 failure is the BE-INST test above.
- V6a `be-sched-wasm32-s182-2.log` exit=0; V6b `be-sched-wasm32-hostwasm-s182-2.log` exit=0,
  `target/wasm32-unknown-unknown/debug/vactr.wasm` exists.
- V4 largest `.rs` in the tree 785 (`dsp/engine.rs`); this plan's largest `sched/runtime.rs` 718 (all under 800).
- V5 prints `none`. V8 `rustfmt --edition 2021 --check` on the 21 owned `.rs` files exit 0.

**Blockers**: none for this plan's deliverables. V2/V3/V3t on the shared tree wait on BE-DSP/BE-INST files outside this
plan's writePaths (recorded, not repaired, per the manifest's parallel-safety rule); they are re-run at the wave join.

## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-007)
- **Previous**: vactr-backend-contracts.md
- **Next**: vactr-backend-midi.md, vactr-backend-native.md, vactr-backend-wasm.md

### Closing note (BE-FINAL, session 186)

Accepted by the integration review (acceptedPlanIds) and reconciled by BE-FINAL on the joined tree: every final-tree gate exits 0 (`target/fe-logs/be-final-<check>-s186-1.log`), and the TASK-007/008 checkboxes in vactr-core.md cite this plan's tests. BE-FINAL serial repairs touched this plan's area: R3 (commit writes the `bus` control), R4 (commit resolves `speed-fit`), R2c (a `bank`/`table`/`source` keyword control overrides the routed bank); targeted tests for criteria 3, 4, 8, 9, 10 sub-clauses were added in `src/host/tests/e2e/sched_gaps.rs`. Archive to impl-plans/completed/ in the separate docs commit after the workflow commit.
