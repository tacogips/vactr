# Vactr Editor: WebMIDI Access, Device Picker, Learn, Forwarding (ED-MIDI) Implementation Plan

**planId**: ED-MIDI (issue #5, TASK-010, wave 2; WebMIDI access on a user action, the input device picker, CC event
stream and learn capture, forwarding of raw MIDI to the browser session)
**Status**: Completed (implemented, gate-verified, adversarial review and integration review accepted in session 186; removed from the dispatch manifest by the session-187 amendment; source rides in the single workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 15.1.9, 15.1.1 (Tauri has no WebMIDI), 11.7;
design-docs/specs/command.md `session_midi_in`
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactr/issues/5
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
- `impl-plans/active/vactr-editor-midi.md`

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

The common protocol in `vactr-editor-scaffold.md`, with `<planId>` = `ED-MIDI`.

## Verification (`<wave>` = `midi`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5, plus:

| # | Command | Evidence |
|---|---------|----------|
| M1 | LOG(`own`): `cd editor && npx vitest run test/midi` | `exit=0`, 4 files passed |
| M2 | `grep -rn "getUserMedia" editor/src \|\| echo none` | prints `none` |

## Completion Criteria

- [x] Items 1-6 implemented
- [x] Required tests pass
- [x] Common rows and M1-M2 pass with logs cited; `final-hashes.txt` written (M2: see the session-186 note)

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-MIDI implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 186, ED-MIDI implementer)

**Tasks Completed**: items 1-6. Dependency ED-SCAFFOLD is in the runtime `acceptedPlanIds`.

- `editor/src/midi/access.ts`: `requestMidi(nav?)` calls `requestMIDIAccess({sysex: false})` and returns null when the
  API is absent or the request rejects (`NOT_AVAILABLE` = "MIDI not available on this host"). The `*Like` interfaces
  are the structural Web MIDI subset.
- `editor/src/midi/devices.ts`: `DevicePicker` lists connected inputs, refreshes on `statechange` (re-attaching and
  detaching `midimessage` listeners), persists the active set by input name (id when unnamed) under
  `localStorage['vactr.midi.active-inputs']`, and dispatches only active inputs' messages. Inputs start inactive.
- `editor/src/midi/learn.ts`: `parseCc` (0xBn, channel 1..16), `CcStream implements MidiApi` (`onCc`, `learnNext`,
  `cancelLearn`, plus `onLearnState` for the indicator). A second `learnNext` rejects the first with
  `LearnCancelled`. Notes and clock are not CC events.
- `editor/src/midi/forward.ts`: `audioTime` (`contextTime + (timeStamp - performanceTime) / 1000`, falling back to
  `currentTime`), `forwardable` (the native `parse_midi` set: note on/off, CC, F8/FA/FB/FC), `Forwarder` over
  `core.midiIn`.
- `editor/src/midi/mount.ts`: MIDI section in the right pane (Enable MIDI button, status, picker, learn indicator
  with Cancel). Access is requested only from the click handler. `deps.midi` is set only on grant and cleared on
  dispose. Forwarding only on `tier === 'browser'` with `deps.core`. CC event `time` is the audio-clock time on the
  browser tier and page-clock seconds on the native tier, matching `deps.clock`. `midi.css` is loaded through
  `new URL('./midi.css', import.meta.url)` and a `<link>`, because TypeScript 7 rejects a side-effect CSS import
  (TS2882) and no plan owns a CSS module declaration (recorded for ED-FINAL in `notes.md`; Vite inlines it).
- `editor/test/support/midi.ts`: `FakeMIDIAccess`, `FakeMIDIInput.emit(bytes, timeStamp)`, `fakeNavigator`,
  `MemoryStorage`, `midiDeps`, `mountMidi`.
- Tests: `access.test.ts` (7), `devices.test.ts` (5), `learn.test.ts` (8), `forward.test.ts` (7). Forwarding tests
  use the real `WasmCore` over `FakeCore` and assert the `session_midi_in` bytes and times.

**Verification** (logs under `target/fe-logs/`; each ends with `exit=`):
- V1 `ed-midi-build-s186-1.log` exit=0; V2 `ed-midi-clippy-s186-1.log` exit=0; V7 `ed-midi-fmt-s186-1.log` exit=0.
- V3 `ed-midi-nextest-s186-1.log` exit=0, 985 run, 985 passed, 1 skipped.
- V3t `ed-midi-cargotest-s186-1.log` exit=0 (lib 964, cli 9, directive_fixtures 2, spec_fixtures 10 passed).
- V6a `ed-midi-wasm32-s186-1.log` exit=0; V6b `ed-midi-wasm32-hostwasm-s186-1.log` exit=0; V6c copied to
  `target/ed-wasm/ED-MIDI.wasm`. V4 largest `.rs` 799 lines (`src/dsp/build.rs`).
- E0 node v26.9.0, npm 11.19.1. E1 `ed-midi-npm-ls-s186-1.log` exit=0.
- E2 `ed-midi-npm-check-s186-1.log` exit=1, sibling-caused: `src/pkg/driver.ts` imported ED-WIRE's not-yet-written
  `./opfs`. Counting run `ed-midi-npm-check-s186-2.log` exit=0 after the sibling file landed.
- E3 `ed-midi-npm-test-s186-2.log` exit=0, 14 files, 82 tests passed (run 1 also exit=0, 14/82).
- E4 `ed-midi-npm-build-s186-2.log` exit=0; E4c exit 0. E5 largest `.ts` 446 lines (`src/protocol/types.ts`).
- M1 `ed-midi-own-s186-2.log` exit=0, 4 files, 27 tests passed.
- M2 printed `none` right after the ED-MIDI edits. On the later moving tree it prints one line,
  `editor/src/visual/meters.ts:6`, a COMMENT in ED-VISUAL's in-flight file stating that no `getUserMedia` is used.
  No call exists anywhere (`grep ... | grep -vE ':[0-9]+:\s*(//|\*)'` prints nothing). Left to the join.
- `final-hashes.txt` written under `tmp/ed-editor-20260926-s186/ED-MIDI/attempt-1/`.

**Notes**: no Rust change. No file outside writePaths touched. Formal review, integration and commit are later
workflow steps.

### CLOSING NOTE (ED-FINAL, session 188)

Status confirmed Completed (accepted). Final-tree evidence (ED-FINAL attempt-2, `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/`): join integrity re-checked (this plan's hashes OK or explained); every row exit=0 in `target/fe-logs/ed-final-*-s188-1.log`: build, build-lsp, clippy, clippy-lsp, fmt, nextest (1007 passed, 1 skipped), cargo test, both wasm32 builds, clippy wasm32 host-wasm, npm ci/check/test (54 files, 336 tests)/build (`VACTR_REQUIRE_SESSION_ABI=1`), real-wasm vitest (3 files, 20 tests), Tauri fetch/check/fmt, session subset, lsp_smoke, spec fixtures. Own evidence: `test/midi/*` (4 files) in target/fe-logs/ed-final-npm-test-verbose-s188-1.log; `abi.test.ts` "MIDI input: CC 74 = 127 ..." against the real artifact.

## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-010)
- **Previous**: vactr-editor-scaffold.md. **Parallel**: vactr-editor-wire.md, vactr-editor-code.md
- **Next**: vactr-editor-bind.md
