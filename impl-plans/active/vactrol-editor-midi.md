# Vactrol Editor: WebMIDI Access, Device Picker, Learn, Forwarding (ED-MIDI) Implementation Plan

**planId**: ED-MIDI (issue #5, TASK-010, wave 2; WebMIDI access on a user action, the input device picker, CC event
stream and learn capture, forwarding of raw MIDI to the browser session)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 15.1.9, 15.1.1 (Tauri has no WebMIDI), 11.7;
design-docs/specs/command.md `session_midi_in`
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-SCAFFOLD
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

Controller binding needs a CC stream and a learn capture (ED-BIND consumes them through `MidiApi`). The browser tier
also needs raw MIDI forwarded to the session so the language's `cc`, note input and MIDI clock sync work (11.7). The
MIDI clock STATUS display is ED-CODE's (`tempo.clock` in the transport bar). This plan provides the MIDI plumbing only.

## Non-Goals

- No slider or binding logic (ED-BIND). No MIDI output or clock-out (E2). No native midir changes.
- No sysex: `requestMIDIAccess({sysex: false})`.
- No automatic permission prompt. Access is requested only on a user click.

## writePaths

- `editor/src/midi/mount.ts` (fills the ED-SCAFFOLD stub), `editor/src/midi/access.ts`, `editor/src/midi/devices.ts`,
  `editor/src/midi/learn.ts`, `editor/src/midi/forward.ts`, `editor/src/midi/midi.css`
- `editor/test/support/midi.ts`
- `editor/test/midi/access.test.ts`, `editor/test/midi/devices.test.ts`, `editor/test/midi/learn.test.ts`,
  `editor/test/midi/forward.test.ts`
- `impl-plans/active/vactrol-editor-midi.md`

## sharedPaths

None.

## File-Level Changes (behavior and signatures; no code)

1. **`access.ts`.** `requestMidi(): Promise<MIDIAccess | null>`:
   - it is called only from the "Enable MIDI" button handler;
   - when `navigator.requestMIDIAccess` is absent (Tauri WKWebView) or rejects, it returns null and the pane shows
     "MIDI not available on this host".
2. **`devices.ts`.** The device picker:
   - it lists `access.inputs` by name and id, refreshed on `statechange`;
   - checkboxes select the ACTIVE inputs, persisted in `localStorage` by input name;
   - only messages from active inputs are dispatched.
3. **`learn.ts`.** Parses messages into CC events `{cc, ch (1..16), value (0..127), time}`.
   - `onCc(cb)` subscribes.
   - `learnNext()` resolves with the next CC `{cc, ch}` from an active input.
   - `cancelLearn()` rejects the pending learn.
   - Note and clock messages are not CC events.
4. **`forward.ts`** (browser tier only, when `deps.core` exists). Every message from an active input (CC, note on/off,
   clock 0xF8, start 0xFA, continue 0xFB, stop 0xFC) goes to `core.midiIn(bytes, audioTime)`:
   - `audioTime` is converted from the event's `timeStamp` with `AudioContext.getOutputTimestamp()`
     (`contextTime + (timeStamp - performanceTime) / 1000`);
   - when that is unavailable, `currentTime` at receipt is used;
   - on the native tier nothing is forwarded, because the session's own midir input serves the language.
5. **`mount.ts`.** Renders the MIDI pane (enable button, picker, learn indicator) and sets `deps.midi`, which
   implements `MidiApi` from `app/apis.ts`. `deps.midi` is left undefined until access is granted, and ED-BIND treats
   an absent API as MIDI unavailable.
6. **`test/support/midi.ts`.** `FakeMIDIAccess` with inputs that can `emit(bytes, timeStamp)` and `statechange`.

## Required Tests

- `access.test.ts`: there is no request before a click; the absent API yields the not-available text; a rejection
  yields the same.
- `devices.test.ts`: the list updates on `statechange`; an inactive input's messages are ignored; the selection
  persists across a remount.
- `learn.test.ts`: CC parsing covers the channel nibble to 1..16; `learnNext` resolves with the next CC only; cancel
  works; notes are ignored for learn.
- `forward.test.ts`: with a fake core, CC, note and clock bytes are forwarded with converted times (a fake
  `getOutputTimestamp`); the native tier forwards nothing; there is no forwarding before access is granted.

## Invariants

- No `getUserMedia` anywhere, and no audio input.
- MIDI access is never requested without a user action.
- Every TS file stays under 800 lines.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-MIDI`.

## Verification (`<wave>` = `midi`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5, plus:

| # | Command | Evidence |
|---|---------|----------|
| M1 | LOG(`own`): `cd editor && npx vitest run test/midi` | `exit=0`, 4 files passed |
| M2 | `grep -rn "getUserMedia" editor/src \|\| echo none` | prints `none` |

## Completion Criteria

- [ ] Items 1-6 implemented
- [ ] Required tests pass
- [ ] Common rows and M1-M2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-MIDI implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-wire.md, vactrol-editor-code.md
- **Next**: vactrol-editor-bind.md
