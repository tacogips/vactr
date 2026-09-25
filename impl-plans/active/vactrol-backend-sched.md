# Vactrol Back End: Scheduler, Slot Table, Runtime, Dry Run (BE-SCHED) Implementation Plan

**planId**: BE-SCHED (vactrol-core.md TASK-007 except MIDI input/clock/note lifetime, which BE-MIDI completes)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 11.2, 11.3 (all of it), 11.4, 11.5, 11.6, 10.3, 10.4, 18, 12.8.1, 12.8.3, 12.8.4, 12.8.5, 12.8.7
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
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
- `impl-plans/active/vactrol-backend-sched.md`

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

Each maps to a vactrol-core.md TASK-007 criterion; assert EMITTED MULTIPLICITY at the recording host, never set equality.
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

The common protocol of `vactrol-backend-contracts.md` "Edit Protocol (common to every BE plan)", evidence under
`tmp/be-backend-20260925-s181/BE-SCHED/attempt-<n>/`. BE-DSP and BE-INST edit the same tree concurrently; their files are
disjoint from this plan's.

## Verification

The common table of `vactrol-backend-contracts.md` with `<wave>` = `sched`: V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8.
Plus S1: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/sched::tests/)'`
as LOG(`be-sched-own`), `exit=0`, run count cited per test file.

## Completion Criteria (map to vactrol-core.md TASK-007)

- [ ] `Runtime`, `RuntimeSink`, `SlotTable`, staging/merge/ledger, commit, control channel, cells, dry run, telemetry,
      `once`/`at`, tempo change, captured output implemented as listed
- [ ] TASK-007 criteria 1, 2, 3, 4, 5, 8 (captured print), 9 (dry run), 11 (mixed faults) proven by the tests above
      (criteria numbered in order of the TASK-007 checklist; 6 and 7 are BE-MIDI's, 12 is BE-FINAL's)
- [ ] Criterion 10 (hush/stop per release class) proven for pattern and texture slots; the open-input-voice half is BE-MIDI's
- [ ] TASK-008 criteria 5 and 6, diagnostic half (granular admission with origin, event still commits) proven by
      `src/sched/tests/sched/granular.rs`
- [ ] V1-V8 and S1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-SCHED implementer)` entry: work done, design differences, hash
and intent paths, evidence per row, blockers. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-007)
- **Previous**: vactrol-backend-contracts.md
- **Next**: vactrol-backend-midi.md, vactrol-backend-native.md, vactrol-backend-wasm.md
