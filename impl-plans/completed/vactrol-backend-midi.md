# Vactrol Back End: MIDI Input, Note Lifetime, Clock and Transport (BE-MIDI) Implementation Plan

**planId**: BE-MIDI (vactrol-core.md TASK-007 MIDI deliverables: `MidiInHost` draining, `cc` cells, `midi-notes` live realization, note lifetime, MIDI clock slave/master, Start/Stop/Continue, `use-clock`, `midi-clock-out`)
**Status**: Completed (accepted by integration review; reconciled by BE-FINAL session 186; archive after the workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 11.7 (all), 11.1, 11.3 (priority channel, `SlotControl`), 12.8.12 "MIDI wiring (chosen: MIDI owns the runtime.rs edit)", 12.8.4 (clock thresholds)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
**dependsOn**: BE-SCHED (Runtime, tick/drain points, commit path), BE-DSP (`Engine`, voices, `TagMap`, release handling for the voice-termination assertions)
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json

---

## Intent and Context

MIDI input arrives on the IO thread and is drained by the evaluator each tick; user code never runs off it (11.7).
BE-SCHED left `Runtime` with `use-clock`/`midi-clock-out` stored but inert and never polls `MidiInHost`. This plan adds
the MIDI modules and wires them into `sched/runtime.rs` at the tick and drain points: `cc` into `InputCells`, live
`midi-notes` realization via the existing `pattern::combinators::input::{input_lane_walk, realize_note}`, the
`NoteInstance` records and `VoiceRelease` tags, the clock slave over the existing `clock::MidiClockSync`, transport
freeze/resume, clock-loss freewheel, and the clock master. BE-NATIVE and BE-WASM run at the same time and never write
`sched/` or `clock/`.

## Non-Goals

- No midir code (BE-NATIVE) and no WebMIDI (TASK-010). No Ableton Link (`:link` stays the checker diagnostic).
- No quantization of live input (11.7: a later option).
- No changes to BE-SCHED's staging/merge/control semantics; only additive calls at the tick/drain points.

## writePaths (exclusive)

- `src/sched/midi_in.rs`, `src/sched/midi_clock.rs`, `src/sched/runtime.rs` (wiring only)
- `src/sched/tests/midi.rs` and new files under `src/sched/tests/midi/`
- `src/clock/clock.rs`, `src/clock/tests/clock.rs` (ONLY if the slave anchor or freewheel needs a change; recorded)
- `impl-plans/active/vactrol-backend-midi.md`

## sharedPaths

None.

## File-Level Changes (signatures only)

1. `midi_in.rs`: `MidiIn` state in `Runtime`; `drain(&mut self, host: &mut dyn MidiInHost, cells: &mut InputCells, lanes: &LaneTable, now: f64) -> Vec<LiveNote>`;
   `cc` -> `InputCells::set_cc(channel, controller, value / 127)`; `NoteInstance { slot, gen, seq, filtered, channel,
   pitch, open }` recorded for EVERY NoteOn (filtered too); `LaneTable`: the `input_lane_walk` result per slot, computed
   when a binding containing `MidiNotes` activates (a lane error is already rejected at dry run); a surviving note is
   realized with `realize_note`, committed immediately (bypassing staging) and posted as `CtlMsg::LiveNoteOn { tag, ev }`
   with `VoiceTag = (slot, channel, pitch, seq)`; NoteOff matches the EARLIEST unmatched instance of (channel, pitch):
   filtered -> consumed silently, surviving -> `CtlMsg::VoiceRelease { tag }` on the same priority channel;
   `stop` (Natural) closes a slot's open instances (later NoteOffs consumed silently); hush's Panic reaches open voices
   through the normal `SlotControl` path.
2. `midi_clock.rs`: slave: pulses (24 ppq) timestamped, period smoothed with factor 0.1, clock re-anchored continuously
   with exact logical positions (via `MidiClockSync`); `Start` resets to the next cycle 0, `Stop` freezes the scheduler
   (staging cleared, `SlotControl` Natural to all sinks), `Continue` resumes from the frozen position; no pulse within
   500 ms freewheels at the last smoothed tempo with `clock-lost`; `use-bpm` under `:midi` gives `clock-external`.
   Master (`midi-clock-out true`): `MidiEvent::Clock`/`Start`/`Stop` emitted from the internal clock with the same
   lookahead/commit discipline as note events.
3. `runtime.rs` wiring: at tick start `midi_in.drain(..)` and slave anchoring; after commit, master clock emission; in
   `drain` the stored `Tempo(Clock)`/`Tempo(MidiClockOut)` effects take effect; stop/hush call the instance closing.

## Required Tests (`src/sched/tests/midi/*.rs`; mock clock, recording hosts, a scripted `MidiInHost`)

- `clock.rs` (TASK-007 criterion 6): jittered synthetic 24-ppq pulses drive a smoothed anchor with exact logical
  positions; Start/Stop/Continue freeze and resume; clock loss freewheels with `clock-lost`; clock master emits
  Clock/Start/Stop with commit discipline; `use-bpm` under `:midi` is `clock-external`.
- `input.rs` (criterion 6 remainder): `midi-notes` input sounds a voice within one drain tick + commit path; `cc` writes
  its cell; `midi-notes` never produces events in dry runs; `degrade-by 1` after `midi-notes` drops every note but still
  records filtered instances.
- `lifetime.rs` (criterion 7; recorded `CtlMsg`s are fed into a `dsp::Engine` to assert voice state): held note releases on
  its NoteOff via tag; repeated same-pitch NoteOns release earliest-first; an old-gen held voice surviving a rebind
  (`None`) is terminated by its later NoteOff while the new binding's voices stay untouched (both halves); `stop`'s
  Natural puts every open input voice into release and closes its records; a filtered NoteOn consumes its own NoteOff
  without releasing another same-pitch voice; a forced release-before-start (reordered in the mock transport) hits the
  tombstone and the late NoteOn is dropped; tag-map exhaustion steals the oldest open voice with a diagnostic and no
  allocation; hush Panic-gates open input voices.
- The criterion-10 open-input half: stop's Natural lets open input voices enter release (asserted in `lifetime.rs`).

## Invariants

- Live input bypasses staging and never uses the time-ordered ring; starts and releases share one FIFO channel.
- Logical positions stay exact `Ratio64` under jitter; only the host-seconds anchor moves.
- No `std::{thread,fs,time,net,process}`. No `.rs` file reaches 800 lines (split `runtime.rs` wiring into `midi_*.rs`
  helpers if `runtime.rs` approaches the limit).

## Edit Protocol

Common protocol of `vactrol-backend-contracts.md`; evidence under `tmp/be-backend-20260925-s181/BE-MIDI/attempt-<n>/`.
Before the first `sched/runtime.rs` edit, write the intent snapshot (the exact call sites added) to `intent.md`.

## Verification

Common table with `<wave>` = `midi`: V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8. Plus M1:
`NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/sched::tests::midi/)'`
as LOG(`be-midi-own`), and M2: BE-SCHED's own-test filter re-run (`test(/sched::tests::sched/)`) as LOG(`be-midi-schedregress`)
proving the wiring changed no scheduler behavior.

## Completion Criteria (map to vactrol-core.md TASK-007)

- [x] `MidiInHost` draining, `cc` cells, `midi-notes` live realization, note lifetime, clock slave/master, transport,
      `use-clock`/`midi-clock-out` implemented and wired (src/sched/midi_in.rs, src/sched/midi_clock.rs; runtime.rs
      call sites listed in tmp/be-backend-20260925-s181/BE-MIDI/attempt-1/intent.md and runtime.rs.diff)
- [x] TASK-007 criteria 6 and 7 proven; the open-input half of criterion 10 proven (src/sched/tests/midi/{input,clock,
      transport,lifetime}.rs, 17 tests; be-midi-own-s182-1.log 17/17)
- [x] V1-V8, M1, M2 pass with logs cited; `final-hashes.txt` written (see the session-182 log)

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-MIDI implementer)` entry. Edit only this log.)

### Session: 2026-09-25 (session 182, BE-MIDI implementer)

**Dependency admission**: BE-SCHED and BE-DSP are in the dispatch `acceptedPlanIds`; the pre-edit runtime.rs hash
(bd9c7331...) equals BE-SCHED's final-hashes.txt.

**Work**:
- `src/sched/midi_in.rs` (539 lines): `MidiIn` (lane table cached by bound-pattern identity, ordered pending notes,
  arrivals, seq counters), `NoteInstance { key, slot, gen, seq, filtered, channel, pitch, open, sink }`, `Arrival`,
  `LiveSink`. `take_midi_in` (cc -> `InputCells::set_cc(ch, controller, value/127)`, clock/transport dispatch, NoteOn
  velocity 0 = NoteOff), `play_live_notes` (after activation: `input_lane_walk` per bound pattern, `realize_note`,
  `commit()` directly, `CtlMsg::LiveNoteOn { tag, ev }` per voice on the audio priority channel; MIDI-out notes with
  open duration; OSC sent), `note_off` (earliest unmatched arrival of (channel, pitch); filtered/closed consumed
  silently; one `VoiceRelease` per started voice, MIDI-out a note-off), `close_live_notes` (stop/hush/transport stop),
  `note_stolen` (`voice-steal` warning from the engine's cumulative `Counters.stolen`).
- `src/sched/midi_clock.rs` (466 lines): `MidiClockState`; source switch by rebuilding the `Clock` exactly at the
  current position (no clock/ edit needed); pulses through `MidiClockSync`; `Start` restarts every pattern slot at
  cycle 0 (gen bump, `None`), `Stop` freezes (gen bump, staging/ledger cleared, `Natural` to all sinks, instances
  closed, clock pinned), `Continue` resumes from the frozen position; clock-lost after `midi_clock_timeout` with
  freewheel and re-sync on the pulse-grid point nearest the freewheeled position; master Start/Clock/Stop/Continue
  sent only when a pulse's host time enters the commit horizon.
- `src/sched/runtime.rs` (749 lines; call sites only, diff in attempt-1/runtime.rs.diff): two `RuntimeConfig` fields
  (`midi_clock_timeout` 0.5, `clock_smoothing` 0.1, design 12.8.4), two `Runtime` fields, `Revoke` -> `close_live_notes`,
  `Tempo` -> `apply_clock_settings`, tick -> `take_midi_in` + frozen early return + `play_live_notes` after activation +
  `emit_midi_clock` after commit, `Counters` arm -> `note_stolen`.
- Tests: `src/sched/tests/midi.rs` (harness: scripted `MidiInHost`, `MidiRig`, `EngineRig` feeding recorded calls into
  a real `dsp::Engine` at their arrival times, every block under `alloc_probe::armed` with zero allocations asserted),
  `midi/input.rs` (4), `midi/clock.rs` (3), `midi/transport.rs` (2), `midi/lifetime.rs` (8).
- `clock/` not edited (hashes equal pre-edit).

**Design differences (recorded)**:
1. `NoteInstance` records are grouped per arrival: a NoteOff matches the earliest unmatched arrival of (channel,
   pitch) and settles every listening lane's instance of it (two slots, or a stack of two lanes, both release).
2. A chord realized from one note shares one tag; the release posts one `VoiceRelease` per started voice.
3. MIDI channel numbering of `MidiInEvent.ch` is 1..16, as `cc channel:` and `midi-notes channel:` (BE-NATIVE's
   adapter maps status nibbles 0..15 to 1..16).
4. MIDI-routed live notes are sent with `dur = inf` and ended by `MidiEvent::NoteOff`; stop/hush send that note-off
   directly (a MIDI host need not track open notes). They bypass `recent_midi`.
5. `Start` carries no timestamp: cycle 0 is anchored at the draining tick's host time; `once` slots and `at` thunks
   keep their positions across a `Start` rewind (documented limitation).
6. `midi-clock-out true` sends `Start` together with the first pulse that enters the commit horizon.

**Verification** (logs under target/fe-logs/, each ends with exit=):
| Gate | Command | Log | Result |
|------|---------|-----|--------|
| V1 | `CARGO_TERM_QUIET=true cargo build` | be-midi-build-s182-1.log | exit 0 |
| V2 | `cargo clippy --all-targets -- -D warnings` | be-midi-clippy-s182-1.log | exit 0 |
| V8 | `cargo fmt --check` | be-midi-fmt-s182-2.log | exit 0 (the earlier be-midi-fmt-s182-1.log exit 1 had diffs only in BE-WASM's then in-progress src/host/wasm/{main_half,worklet_half}.rs) |
| V8 own | `rustfmt --edition 2021 --check` on the 8 owned .rs files | be-midi-fmt-owned-s182-1.log | exit 0 |
| V3 | nextest (full) | be-midi-nextest-s182-1.log | exit 0, 747/747 |
| V3t | `CARGO_TERM_QUIET=true cargo test` | be-midi-cargotest-s182-1.log | exit 0, lib 737 passed, fixtures 10 passed |
| V3f | nextest `binary(spec_fixtures)` | be-midi-fixtures-s182-1.log | exit 0, 10/10 |
| M1 | nextest `test(/sched::tests::midi/)` | be-midi-own-s182-1.log | exit 0, 17/17 |
| M2 | nextest `test(/sched::tests::sched/)` | be-midi-schedregress-s182-1.log | exit 0, 47/47 |
| V6a | `cargo build --target wasm32-unknown-unknown` | be-midi-wasm32-s182-1.log | exit 0 |
| V6b | same, `--no-default-features --features host-wasm` | be-midi-wasm32-hostwasm-s182-1.log | exit 0 |
| V4 | largest .rs files | be-midi-wc-s182-1.log | largest 786 (dsp/engine.rs); owned max runtime.rs 749 |
| V5 | std I/O grep | be-midi-stdgrep-s182-1.log | none |
Mutation checks (isolated copy /tmp/vactrol-midi-mut, own target dir): latest-first NoteOff matching fails 2 lifetime
tests; a no-op `close_live_notes` fails the stop and hush tests.

**Evidence**: tmp/be-backend-20260925-s181/BE-MIDI/attempt-1/{intent.md, pre-edit-hashes.txt, runtime.rs.pre,
runtime.rs.diff, owned-rs.txt, final-hashes.txt}.

**Downstream (not this plan)**: formal test-integrity/adversarial/integration review, core-plan checkboxes and README
(BE-FINAL), commit.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-007)
- **Previous**: vactrol-backend-sched.md, vactrol-backend-dsp.md
- **Next**: vactrol-backend-finalize.md

### FINDING KEY NOTE (operator, 2026-09-25, after the BE-NATIVE and BE-WASM attempt-1 failures)

- The step6-test-integrity-check and step7-adversarial-review output contracts
  reject unknown keys INSIDE each `findings[]` item. BE-NATIVE failed with
  `$.findings[0].intentRef additional property is not allowed` and BE-WASM with
  `$.findings[0].intentIncerence ...`: both were misspellings of the accepted key
  `intentReference`. Use ONLY the keys the riela contract defines for a finding item:
  `findingId`, `severity`, `category`, `file`, `line`, `message`, `evidence`
  (confirmed against the riela binary); put anything else, such as an intent
  reference or a fix-cost note, inside the `message` or `evidence` text. `findings` must
  be present (empty array when none) and the outputs must not carry `planId`.

### Closing note (BE-FINAL, session 186)

Accepted by the integration review (acceptedPlanIds) and reconciled by BE-FINAL on the joined tree: every final-tree gate exits 0 (`target/fe-logs/be-final-<check>-s186-1.log`), and the TASK-007/008 checkboxes in vactrol-core.md cite this plan's tests. Archive to impl-plans/completed/ in the separate docs commit after the workflow commit.
