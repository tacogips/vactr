# Vactrol Back End: MIDI Input, Note Lifetime, Clock and Transport (BE-MIDI) Implementation Plan

**planId**: BE-MIDI (vactrol-core.md TASK-007 MIDI deliverables: `MidiInHost` draining, `cc` cells, `midi-notes` live realization, note lifetime, MIDI clock slave/master, Start/Stop/Continue, `use-clock`, `midi-clock-out`)
**Status**: Ready
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

- [ ] `MidiInHost` draining, `cc` cells, `midi-notes` live realization, note lifetime, clock slave/master, transport,
      `use-clock`/`midi-clock-out` implemented and wired
- [ ] TASK-007 criteria 6 and 7 proven; the open-input half of criterion 10 proven
- [ ] V1-V8, M1, M2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-MIDI implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-007)
- **Previous**: vactrol-backend-sched.md, vactrol-backend-dsp.md
- **Next**: vactrol-backend-finalize.md
