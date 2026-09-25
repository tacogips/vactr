# Vactrol Editor: CodeMirror Surface, Highlighting, Transport, Sample Browser (ED-CODE) Implementation Plan

**planId**: ED-CODE (issue #5, TASK-010, wave 2; the `.vact` mode, diagnostics, eval keys and flash, document sync
from CodeMirror, revision history and span mapping, playing-step highlighting, transport bar, sample bank browser)
**Status**: Completed (implemented, gate-verified, adversarial review and integration review accepted in session 186; removed from the dispatch manifest by the session-187 amendment; source rides in the single workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 15.1.4 (revisions, epochs, offsets, tier features),
15.1.5, 15.1.12 (criteria 1 and 10); 14.4 (highlighting, `doc_revision` mapping); 11.7 (clock status);
design-docs/user-qa/pending-editor-questions.md E3
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-SCAFFOLD
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

This plan builds the code pane on CodeMirror 6 over the ED-SCAFFOLD client and store. It owns:
- the conversion of CodeMirror transactions into protocol document sync (`protocol/document.ts` + `utf8.ts`);
- the revision history that maps any span from its `doc_revision` to the current document. ED-BIND reuses this history
  for tracked binding spans, and ED-PARAMS reuses it for display pitch lookup.

It also covers TASK-010 criterion 1 (highlight within one lookahead window, mock clock) and the highlight half of
criterion 10 (edit reconciliation).

## Non-Goals

- No sliders, write-back, drag or learn (ED-BIND); no visual panes or analyzer displays (ED-VISUAL); no MIDI (ED-MIDI).
- No Lezer grammar. The tokenizer is for highlighting only and never decides bindings.
- No per-slot level meters (E3). There is no separate panic message.

## writePaths

- `editor/src/code/mount.ts` (fills the ED-SCAFFOLD stub), `editor/src/code/language.ts`,
  `editor/src/code/diagnostics.ts`, `editor/src/code/eval.ts`, `editor/src/code/sync.ts`,
  `editor/src/code/history.ts`, `editor/src/code/highlight.ts`, `editor/src/code/transport.ts`,
  `editor/src/code/samples.ts`, `editor/src/code/code.css`
- `editor/test/code/language.test.ts`, `editor/test/code/diagnostics.test.ts`, `editor/test/code/eval.test.ts`,
  `editor/test/code/sync.test.ts`, `editor/test/code/history.test.ts`, `editor/test/code/highlight.test.ts`,
  `editor/test/code/reconcile.test.ts`, `editor/test/code/transport.test.ts`, `editor/test/code/samples.test.ts`
- `impl-plans/active/vactrol-editor-code.md`

## sharedPaths

None.

## File-Level Changes (behavior and signatures; no code)

1. **`language.ts`.** A `StreamLanguage` tokenizer and highlight style covering:
   - line comments `#`, and `#@` directive lines as a distinct token;
   - keywords `:name`, numbers (int, float, ratio), strings, and path/url literals;
   - the heads `let var upd fn inst bus look master import slot if`.

   Export `vactLanguage()`.
2. **`history.ts`.** `RevisionHistory`:
   - `record(rev, changes: ChangeSet)` keeps the last 256 revisions;
   - `mapSpan(span16, fromRev, toRev) -> span16 | null` composes the change sets. It returns null when any change
     touches the span or `fromRev` is older than the history.

   The protocol spans are converted with the per-revision `Utf8Index`: store the text per revision only while it is
   within the history, or store the byte-length bookkeeping needed to convert.
3. **`sync.ts`.** A CodeMirror `ViewPlugin` or transaction listener that, per document-changing transaction:
   - converts the change set to byte changes against the base text and computes the dirty new-revision byte spans
     (`insert` ranges plus zero-length spans at pure deletions);
   - calls `DocSync.edit` (the epoch increments synchronously);
   - records the change in `RevisionHistory`.

   `mapWireSpan(wireSpan, rev) -> {from, to} | null` goes from bytes at a revision to the current UTF-16 positions.
4. **`diagnostics.ts`.** A `@codemirror/lint` source merging three kinds:
   - static diagnostics from `eval-result`;
   - browser-tier `session_check` diagnostics, requested 300 ms after the last edit via `WasmCore.check`, only when
     `deps.tier === 'browser'`;
   - runtime `diag` entries, shown with `slot` and `beat` in the message and removed on `clear` for their slot.

   Each span is mapped from its revision. An unmappable one is dropped.
5. **`eval.ts`.** A keymap:
   - `Mod-Enter`: the form at the cursor. The span runs from the nearest line at or above the cursor that starts at
     column 0 with a character other than space, `#` or `>`, through the line before the next such line, excluding
     trailing blank lines. Send it as a byte span.
   - `Mod-Shift-Enter`: the whole document (no span).
   - `Mod-.`: `hush`.

   `eval` always carries the full text, the revision and the epoch (via `DocSync`). A flash decoration covers the span
   for 200 ms. Forms whose `eval-result.forms[i].failure` is set flash the error class.
6. **`highlight.ts`.** `HighlightScheduler(clock: Clock)`:
   - `onPlaying(events)` stores entries `{start: time, end: time + dur_seconds(dur, tempo), span, rev}`, where
     `dur_seconds` uses the latest `tempo.bpm` and `beats_per_cycle`;
   - `tick()` (called per animation frame; in tests, by the test) applies decorations for the entries with
     `start <= now < end`, mapped through `mapWireSpan`, and removes expired ones;
   - an event without `src`, or with an unmappable span, is dropped;
   - on the native tier, times are anchored at batch receipt: offset = `clock.now()` minus the smallest `time` in the
     first batch. This is best effort and documented in code.
7. **`transport.ts`.** The transport bar:
   - tempo and cycle/beat from `tempo`, extrapolated with the clock between messages;
   - MIDI clock status from `tempo.clock`: `internal`, `midi locked` or `midi lost`;
   - a hush/panic button that sends `hush`;
   - a per-slot list built from `playing` and `eval-result` slots, with an activity light lit for 150 ms after an
     event of that slot and a mute button that sends `stop {slot}`;
   - a master level readout from `levels[0].rms`.
8. **`samples.ts`.**
   - The sample bank browser lists `manifest.sounds`.
   - `SampleLibrary` (browser tier):
     - `loadMap(url)` fetches `{"<bank>": ["<url>", ...]}`;
     - `loadBank(bank)` fetches and decodes each entry with `AudioContext.decodeAudioData` and hands it over as
       `bank:index` via `WasmCore.samplePut`;
     - `frames(bank, index)` returns the decoded interleaved frames for previews and the ED-PARAMS sampler editor.
   - Each entry shows a small waveform preview (canvas 2D; skipped when the context is unavailable).
   - The sample-map URL is user-entered and kept in `localStorage`.
   - On the native tier the browser lists names only.
9. **`mount.ts`.**
   - Creates the `EditorView` in the code pane with the language, the sync, the lint, the eval keymap and the
     highlight extensions, and the transport bar in its pane.
   - Subscribes to the store (`diag`, `tempo`, `levels`, `sites`, and `playing` through the client).
   - Sets `deps.code`, which implements the ED-SCAFFOLD `CodeApi` contract of `app/apis.ts`:
     - `selectedSiteId` is the site whose current span contains the main cursor;
     - `samples` implements `SampleLibraryApi`.

     `apis.ts` and `deps.ts` are not edited.

## Required Tests

- `language.test.ts`: token classes for each construct, and `#@` distinct from `#`.
- `sync.test.ts`:
  - a typed insertion yields `doc-changed` with correct byte changes and dirty spans for non-ASCII text;
  - the epoch increments before the debounce;
  - a `set-tweak` issued inside the debounce window is preceded by the flushed `doc-changed` (RecordingTransport
    order).
- `history.test.ts`: mapping across an insertion above, a deletion above, an edit inside (null), and a history
  overflow (null).
- `highlight.test.ts` (MockClock) (criterion 1):
  - an event at t=1.0 with dur 1/4 cycle at 120 bpm and 4 beats per cycle is inactive at 0.99, active at 1.0 and
    1.49, and inactive at 1.5;
  - the decoration is applied at most one lookahead window (use the runtime default lookahead constant, documented in
    the test) after the event time, in the worst case of a `tick` cadence of 1/60 s.
- `reconcile.test.ts` (criterion 10, highlights):
  - insertion above a playing form keeps its highlight at the mapped position;
  - deletion of the highlighted literal drops it;
  - reordering two forms maps each highlight with its text;
  - duplicate literals highlight independently by span;
  - a `playing` event tagged with an earlier `doc_revision` (a stored list evaluated before later edits) highlights
    against that revision through its own `src`.
- `diagnostics.test.ts`: static, check and runtime sources merge; a runtime `clear` removes the slot's markers; an
  unmappable diagnostic is dropped.
- `eval.test.ts`: the form-at-cursor span rule on multi-line chains (`>` lines and indented lines); the whole-document
  eval; the flash lifetime of 200 ms on fake timers; the failure flash class.
- `transport.test.ts`: tempo and clock-status rendering, including `midi lost`; the hush and mute messages; the
  activity light timing.
- `samples.test.ts`: the manifest list; `loadBank` calls `samplePut` with `bank:index` keys (mocked `decodeAudioData`
  and `fetch`); the native tier shows no previews.

## Invariants

- The code module never interprets language semantics for bindings. Spans come from the session.
- Every protocol span is converted between UTF-8 and UTF-16 only through `Utf8Index`.
- All tests use `RecordingTransport` and `MockClock` from `editor/test/support/`, with no real timers except vitest
  fake timers.
- Every TS file stays under 800 lines.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-CODE`.

## Verification (`<wave>` = `code`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5, plus:

| # | Command | Evidence |
|---|---------|----------|
| C1 | LOG(`own`): `cd editor && npx vitest run test/code` | `exit=0`, 9 files passed |

## Completion Criteria

- [x] Items 1-9 implemented
- [x] Required tests pass (criterion 1 automated proxy; criterion 10 highlight half)
- [x] Common rows and C1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-CODE implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 186, ED-CODE implementer)

**Files**: all 20 writePaths. `editor/src/code/mount.ts` (the ED-SCAFFOLD stub) is the only pre-existing file
edited; the other 19 are new. No file outside writePaths was touched; `app/apis.ts` and `app/deps.ts` are unchanged.
Evidence: `tmp/ed-editor-20260926-s186/ED-CODE/attempt-1/` (`intent.md`, `notes.md`, `pre-edit-hashes.txt`,
`post-edit-hashes.txt`, `final-hashes.txt`, `v4-e0.txt`). The pre-edit hash of mount.ts matched the scaffold's
final hash, so there was no drift.

**Items**:
1. `language.ts`: `vactLanguage()` (StreamLanguage + HighlightStyle) and the tokenizer `vactParser`. `#@` maps to a
   `directiveTag` (a sub-tag of `meta`), distinct from `#` (lineComment). It also covers `:kw`, int/float/ratio
   numbers, strings, path/url literals at 6.5.8 token boundaries, and heads at the start of a line only.
   `tokenize(lines)` is used by the tests.
2. `history.ts`: `RevisionHistory` keeps the last 256 revisions as `Text` ropes plus their ChangeSets. `mapSpan`
   returns null when a change overlaps the span interior or inserts strictly inside it (abutting edits keep it), or
   when a revision is out of range. `mapWireSpan` converts through a lazily built, LRU-cached per-revision
   `Utf8Index`.
3. `sync.ts`: `DocumentSync.apply(tr)` converts a transaction to byte changes and dirty spans with `Utf8Index`, calls
   `DocSync.edit` synchronously (epoch first), and records the history. The `extension()` updateListener is
   installed FIRST in the editor, so later listeners see the edit already recorded. It also provides
   `mapWireSpan` and `toWireSpan`.
4. `diagnostics.ts`: `DiagnosticsController` merges three sources into `setDiagnostics`, adding a `lintGutter` in
   mount:
   - static: from `eval-result` at its `doc_revision`;
   - check: `WasmCore.check`, 300 ms after the last edit, browser tier only;
   - runtime: `diag` per slot, with message "(slot s, beat n/d)", at the latest eval revision; a `clear` drops the
     slot.

   An unmappable diagnostic is dropped.
5. `eval.ts`: `formSpanAt` implements the span rule, treating tab as whitespace. When no form start is above the
   cursor, the span starts at line 1. `EvalController` binds `Mod-Enter`, `Mod-Shift-Enter` and `Mod-.` at
   `Prec.high`. The eval sends the full text plus a byte span through `client.eval`, which flushes and stamps.
   Flashes (`vact-flash`, and `vact-flash-error` for forms with `failure`) last 200 ms.
6. `highlight.ts`: `HighlightScheduler` has `onPlaying`, `tick`, `active` and `clear`. `durSeconds` uses the latest
   tempo, defaulting to 120 bpm and 4 beats per cycle before any `tempo`. `TimeAnchor` is `audio` (identity) on the
   browser tier and `receipt` on the native tier (offset = now minus the minimum time of the first batch; best
   effort, documented). `playingField` holds the decorations. Events without `src`, from another file, or with an
   unmappable span are dropped.
7. `transport.ts`: `TransportBar` shows:
   - tempo and cycle/beat, extrapolated and held while `midi lost`;
   - the clock status;
   - hush (`hush`);
   - slots from `playing` plus diagnostic `slot` names (v1 `eval-result` has no slot table), with a 150 ms
     activity light from each event time and mute (`stop`);
   - the master level as dBFS from `levels[0].rms`.
8. `samples.ts`: `SampleLibrary` provides `loadMap`, `loadBank`, which decodes each entry and calls
   `samplePut('bank:index')`, and `frames`. Failed entries are skipped. `SampleBrowser` lists `manifest.sounds`. On
   the browser tier it adds the map url input, persisted as `localStorage['vactrol.sampleMap']`, and canvas
   previews, which are skipped without a 2D context. The native tier shows names only.
9. `mount.ts`: sets up the EditorView (sync first, language, line numbers, history, lint gutter, highlight, eval
   keymap, default keymap), the TransportBar in the transport pane, and the SampleBrowser under the editor.
   - Store subscriptions: `tempo`, `levels`, `manifest`. Client listeners: `playing`, `eval-result`, `diag`.
   - One rAF loop ticks the highlights and the transport bar.
   - `deps.code` implements CodeApi. `selectedSiteId` picks the smallest site span, mapped from the latest eval
     revision, that contains the main cursor.
   - The document file is `DOC_FILE = 'main.vact'`.

**Decisions and deviations**:
- `code.css` is loaded by a `<link>` built from `new URL('./code.css', import.meta.url)`, because a side-effect
  `import './code.css'` fails `tsc` (TS2882: no CSS module types are pinned, and no `.d.ts` is in writePaths). Vite
  inlines the small stylesheet as a `data:text/css` URL (checked in `target/ed-dist/ED-CODE/assets/index-*.js`).
  ED-FINAL may move it into `app.css` or `index.html` if a `data:` stylesheet conflicts with a future CSP.
- Reorder semantics (reconcile.test.ts): CodeMirror's `moveLineDown` keeps the moved form's text and re-types the
  line it jumps over. The moved form's highlight follows it, and the re-typed line's highlight drops until the next
  event of the new revision, as the "a span touched by a change is dropped" rule requires. When a reorder moves the
  text around both forms, both highlights follow.
- The criterion 1 proxy uses the runtime default lookahead `0.120` s (`src/sched/runtime.rs` `SchedConfig`),
  documented in highlight.test.ts. With a 1/60 s tick the decoration is applied within one frame
  (<= 16.7 ms < 120 ms) at four event phases.

**Tests** (C1 `target/fe-logs/ed-code-own-s186-1.log` exit=0: 9 files, 57 tests): language 5, history 5, sync 6,
highlight 6, reconcile 7, eval 8, diagnostics 5, transport 7 (including 2 mount/CodeApi tests), samples 8.

**Verification** (session 186; all logs under `target/fe-logs/`, each run after the final code change):
- V1 `ed-code-build-s186-1.log` exit=0; V2 `ed-code-clippy-s186-1.log` exit=0.
- V3 `ed-code-nextest-s186-1.log` exit=0 (985 run, 985 passed, 1 skipped).
- V3t `ed-code-cargotest-s186-1.log` exit=0 (lib 964, cli 9, directive_fixtures 2, spec_fixtures 10 + 1 ignored;
  0 failed).
- V7 `ed-code-fmt-s186-1.log` exit=0.
- V6a `ed-code-wasm32-s186-1.log` exit=0; V6b `ed-code-wasm32-hostwasm-s186-1.log` exit=0; V6c
  `target/ed-wasm/ED-CODE.wasm` copied (1437858 bytes).
- V4: largest `.rs` is 799 lines (src/dsp/build.rs, untouched). E0: node v26.9.0, npm 11.19.1.
- E1 (npm ls) `ed-code-npm-ls-s186-1.log` exit=0; E2 `ed-code-npm-check-s186-1.log` exit=0.
- E3 `ed-code-npm-test-s186-1.log` exit=0 (32 files, 182 tests, including sibling suites present in the shared tree).
- E4 `ed-code-npm-build-s186-1.log` exit=0 (to `target/ed-dist/ED-CODE`); E4c exit 0.
- E5: largest `.ts` is 446 lines (src/protocol/types.ts); the largest ED-CODE file is samples.ts at 327 lines.

These ran on the shared tree while sibling plans were in flight; the join re-verifies.

**Pending (not ED-CODE work)**: formal test-integrity/adversarial review, the integration review, the vactrol-core
TASK-010 criteria bookkeeping (ED-FINAL) and the commit belong to later workflow steps. A real-browser visual check
of the highlight and flash styling is part of the ED-FINAL manual gate.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-wire.md, vactrol-editor-midi.md
- **Next**: vactrol-editor-bind.md
