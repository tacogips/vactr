# Vactrol Editor: Slider Panel, Binding Keys, Write-Back, Directives, Persistence (ED-BIND) Implementation Plan

**planId**: ED-BIND (issue #5, TASK-010, wave 3; site enumeration and keys, overlay and source-edit writes, commit,
mouse drag on literals, the slider panel, the directive control panel, CC routing and learn, ExternalFile persistence,
mode-scoped saving, reactive display wiring)
**Status**: Completed (implemented, gate-verified, adversarial review and integration review accepted in session 187; removed from the dispatch manifest by the session-188 amendment; source rides in the single workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 15.1.6, 15.1.4 (stale handling, rate), 15.1.12
(criteria 2, 3, 5, 6, 7, 8, 10); 13, 13.5 (BindingKey, write-back, learn); 14.5.5, 14.5.6, 14.5.8 (the
`<doc>.bindings.json` format); design-docs/specs/command.md; design-docs/user-qa/pending-editor-questions.md E4
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-CODE (`CodeApi`: the view, `mapWireSpan`, revision history, selected site), ED-MIDI (`MidiApi`: CC
stream and learn)
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

This plan is the controller-binding UI of the Decided Editor Requirements. Every slider, drag, learned CC and (later,
ED-PARAMS) parameter-editor handle goes through ONE site-write function exposed as `BindApi.writeSite`:
- OVERLAY mode sends `set-tweak`, and the text is untouched;
- SOURCE-EDIT mode and COMMIT perform a validated text edit, then an `eval` of the owning form.

Sites come only from the session (`eval-result.sites`, `bindings.sites`, with `key` and `call`). The plan also covers
directive-backed panels and learn write-back through `directive-edit`, the editor-owned ExternalFile set (E4), and
mode-scoped saving.

## Non-Goals

- No parameter-editor graphics or grid/roll displays (ED-PARAMS). No MIDI device handling (ED-MIDI).
- No add-to-panel UI (panel membership is edited in the text only). No session-side persistence switching (E4).
- No language parsing. Numeric literal text is read only at a site's mapped span, for write-back verification.

## writePaths

- `editor/src/bind/mount.ts` (fills the ED-SCAFFOLD stub), `editor/src/bind/sites.ts`, `editor/src/bind/write.ts`,
  `editor/src/bind/drag.ts`, `editor/src/bind/panel.ts`, `editor/src/bind/directives.ts`,
  `editor/src/bind/routing.ts`, `editor/src/bind/persistence.ts`, `editor/src/bind/save.ts`,
  `editor/src/bind/bind.css`
- `editor/test/bind/sites.test.ts`, `editor/test/bind/write.test.ts`, `editor/test/bind/drag.test.ts`,
  `editor/test/bind/panel.test.ts`, `editor/test/bind/directives.test.ts`, `editor/test/bind/routing.test.ts`,
  `editor/test/bind/persistence.test.ts`, `editor/test/bind/save.test.ts`, `editor/test/bind/reactive.test.ts`,
  `editor/test/bind/multisite.test.ts`, `editor/test/bind/reconcile.test.ts`,
  `editor/test/bind/fixtures.ts` (scripted server messages, including every 14.5.5 batch shape)
- `impl-plans/active/vactrol-editor-bind.md`

## sharedPaths

None.

## File-Level Changes (behavior and signatures; no code)

1. **`sites.ts`.** `SiteTable`.
   - Built from `eval-result.sites`, and updated by `bindings.sites` (replace by form).
   - Entries: `{id, form_gen, span(rev), tier, origin, value, key?, call?, literalText, bindingId}`.
   - `bindingId` is `key` when present. Otherwise it is a tracked span created at the first sighting and mapped
     through `CodeApi.mapWireSpan` on every edit.
   - When a fresh table arrives, each tracked binding RE-KEYS to the new `TweakId` whose span equals its mapped span.
     No match means `unbound`; a touched mapping (null) means `stale`.
   - Keyed bindings re-key by `key`. Duplicate literals are distinct ids.
   - `literalText` is read from the document at the mapped span when the table arrives.
2. **`write.ts`.** `writeSite(id, value)` dispatches on the binding's mode, which is per slider and defaults to
   OVERLAY.
   - OVERLAY: `client.setTweak({file, id, form_gen, value})`. The client already rate-limits and stamps the epoch. The
     overlay value is shown beside the literal as a CodeMirror widget. The text is never changed.
   - SOURCE-EDIT and `commit(id)`:
     - VERIFY that the current text at the mapped span equals `literalText`; on a mismatch, decline and show a notice,
       with no edit and no message;
     - format the new literal: an integer when the literal has no `.`, else trimmed to at most 6 decimals, or to the
       ParamMeta step when stepped;
     - dispatch the CodeMirror change (the ED-CODE sync sends `doc-changed` and bumps the epoch);
     - then `client.eval({span: owning form span})`, where the owning form span comes from `eval-result.forms` mapped
       to now;
     - at most one eval is in flight per form, and the latest value is kept and re-applied when the eval returns.
   - `stale-binding` replies:
     - `stale-form-gen`: keep the value and re-send it latest-wins to the re-keyed site once fresh sites arrive;
     - `edit-invalidated` and `unreconciled-edit`: mark the slider STALE until the next `eval-result`/`bindings`
       refreshes the site;
     - `superseded-definition`: drop the pending write.
   - `manual` tier sites show a "re-evaluate to hear" badge; `reeval` shows "next cycle".
   - Implements `BindApi` (`writeSite`, `mode`, `learn`, `siteById`) and sets `deps.bind`.
3. **`drag.ts`.** A CodeMirror extension: pointer-down on a literal whose span is a site's current span, then vertical
   drag.
   - The scale is ParamMeta `range` and `curve` via `site.call` and `manifest.editors[call.name].params[param]` when
     present; otherwise the step is 1% of max(|value|, 1).
   - Writes go through `writeSite`. There is no other write path.
4. **`panel.ts`.** The right-pane slider panel.
   - Groups by `origin`: pattern literals, bindings (top-level let/var), inst defaults.
   - Each slider shows its label (key, else the literal line excerpt), the value, the tier badge, a mode toggle
     (overlay / source-edit), commit (overlay only), learn, and the STALE/unbound state.
   - It subscribes to the store per `site:<id>` / `name:<n>` key and repaints only affected rows.
   - Failed/blocked `states` show the previous value with the diagnostic or the `blocked-on` badge (14.5.5).
5. **`directives.ts`.** The control panel from `eval-result.directives`:
   - entries grouped by label or line, with each binding's param, `cc` and `ch`;
   - directive lint diagnostics (`unknown-label`, `duplicate-label`, `ambiguous-selector`, ...) come from the static
     diagnostics and are rendered by ED-CODE's lint source; this panel shows a marker next to the affected entry;
   - in ExternalFile mode the panel is built from the editor set instead.
6. **`routing.ts`.** CC routing: `MidiApi.onCc` event -> binding -> site -> `writeSite`, with the value scaled
   0..127 to the ParamMeta range, else 0..1.
   - Directive mode: match the directive table `bindings` by `key == site.key`, else by span containment plus
     `site.call.param == binding.param`, honoring `ch` (the file default from `file_level.midi_ch`).
   - ExternalFile mode: the editor set.
   - Keys are full `BindingKey` spellings, so `hats.lpf`/`hats.hpf` and `hats.lpf.1`/`hats.lpf.2` never cross-talk.
   - `learn(id)`:
     - Directive mode: `MidiApi.learnNext()`, then `client.learn({binding: key ?? id, cc, ch})`, then on
       `directive-edit`, verify `expected` equals the current text at the mapped span and apply `text` as a
       CodeMirror change (the sync sends `doc-changed`); a mismatch is declined with a notice;
     - ExternalFile mode: record `{cc, ch}` in the editor set and send NO `learn`.
7. **`persistence.ts`.** `PersistenceMode = 'directive' | 'external-file'`, a per-document toggle, default
   `directive`.
   - `EditorBindingSet` entries `{key, panel, midi?: {cc, ch}, overlay?}`.
   - `toJson()` gives `{"v": 1, "bindings": [...]}` with keys spelled `label.site.n.param`, overlays kept (14.5.8).
     `fromJson()` ignores unknown `v` with a notice.
   - Switching to ExternalFile copies the current panel (directive bindings plus learned mappings plus current
     overlays) into the set, with NO text change.
8. **`save.ts`.** `save()` through `deps.files`:
   - Directive mode: `save(name, bufferText)`.
   - ExternalFile mode: `save(name, bufferText)` plus `saveSidecar(name + '.bindings.json', set.toJson())`.

   The buffer text never receives overlay values; only `commit` writes a value into the text. The editor never inserts
   or strips `#@` text except through a verified learn `directive-edit` in Directive mode.
9. **`mount.ts`.** Builds the panel and control panel in the right pane, installs the drag extension on
   `CodeApi.view`, and wires the store, client, `stale-binding`, `directive-edit` and `MidiApi` (read at use time).

## Required Tests

All tests use `RecordingTransport` (the "recording host" at the protocol boundary) and scripted fixtures from
`fixtures.ts`.
- `write.test.ts` (criterion 2):
  - on a `direct` site, overlay sends exactly one `set-tweak` with `form_gen` and the current epoch, and the document
    text is unchanged;
  - `commit` makes a verified edit and then one form `eval`;
  - on a `reeval` site the tier badge is "next cycle", a scripted `bindings` re-keys the site, and a subsequent move
    targets the new id;
  - a `manual` site shows the badge;
  - source-edit mode edits the text and evals the form, with at most one eval in flight, and the latest value is
    applied after the reply;
  - a text drift at the span declines with no message.
- `drag.test.ts`: a drag on a site literal produces the same `set-tweak` as the slider; a drag on a non-site number
  does nothing; ParamMeta range scaling.
- `panel.test.ts` (criterion 3): an `eval-result` containing a pattern literal, a top-level `let` number and an `inst`
  parameter default lists all three under their origin groups. The inst-default slider in overlay sends `set-tweak`
  with no `eval` (heard at the next voice under the cell contract, which is the session's job; asserted here as
  no-re-eval). Learn via a fake `MidiApi` maps a CC to that slider, and later CC events move it.
- `directives.test.ts` (criterion 5):
  - opening a document with the spec's directive examples (copy them from design 13.5 and
    `tests/fixtures/directives/vocabulary.toml`, scripted as `eval-result.directives`) shows the declared panel;
  - learning a mapped parameter sends `learn`, and the scripted `directive-edit` updates the `#@` comment text after
    verification;
  - a mismatching `expected` is declined;
  - a scripted `unknown-label` diagnostic shows the entry marker;
  - switching to ExternalFile leaves the document text byte-identical and the panel preserved.
- `save.test.ts` (criterion 6; two tests):
  - Directive mode: the saved `.vact` equals the buffer, including the `#@` comments with learned CCs, and there is no
    sidecar;
  - ExternalFile mode: the saved `.vact` equals the buffer, contains no editor-written binding text, and the sidecar
    JSON holds the bindings;
  - in both modes an active overlay value does not appear in the saved text until `commit`.
- `multisite.test.ts` (criterion 7):
  - learned CCs on `hats.lpf` and `hats.hpf`, and on `hats.lpf.1` and `hats.lpf.2`, operate their own sites
    simultaneously: interleaved CC events produce `set-tweak`s only to their own ids;
  - persist and read back per key in both modes;
  - moving the labeled line (a scripted re-eval with the same keys, new spans) keeps the mappings;
  - reordering the same-named sites (scripted new ordinals) migrates the mappings;
  - a broken mapping shows STALE.
- `reactive.test.ts` (criterion 8):
  - For each 14.5.5 batch shape, scripted from the session tests' expectations: changing edge, failed diamond and its
    recovery, provisional rollback (X shows the restored 7 and never 1 or 2), abort/retry, conditional unblocking,
    switch-toward, status recovery (badges cleared, value unchanged), late failure (`blocked-on` with the restored 7),
    newly discovered selector, and ordinary failure then `upd b 1` recovery with the diagnostic cleared:
    - the panel and value displays repaint once per batch;
    - unrelated rows do not repaint (render counters);
    - no provisional value is ever rendered (a DOM text history spy).
- `reconcile.test.ts` (criterion 10, bindings half):
  - insertion, deletion and reordering above and inside a bound form keep bindings attached or drop them cleanly;
  - duplicate literals bind independently;
  - an edit without re-eval followed by a delayed CC: `doc-changed` goes first, and a scripted
    `stale-binding edit-invalidated` shows STALE;
  - write-back declines on text mismatch.
- `routing.test.ts`: channel filtering, the file-default `ch`, value scaling, and ExternalFile routing.
- `persistence.test.ts`: the JSON round trip in the 14.5.8 format and the key spellings; an unknown `v` is ignored
  with a notice.
- `sites.test.ts`: re-keying by span and by key; stale and unbound states.

## Invariants

- Exactly one write function (`writeSite`). Slider, drag, CC and (ED-PARAMS) handles all call it.
- No write without a flushed `doc-changed` first (the client guarantees it; a test asserts the order).
- Text edits happen only after verification against expected text.
- No binding data is written into the text except verified `directive-edit`s in Directive mode.
- Every TS file stays under 800 lines.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-BIND`.

## Verification (`<wave>` = `bind`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5, plus:

| # | Command | Evidence |
|---|---------|----------|
| B1 | LOG(`own`): `cd editor && npx vitest run test/bind` | `exit=0`, 11 test files passed |

## Completion Criteria

- [x] Items 1-9 implemented
- [x] Required tests pass (criteria 2, 3 slider/learn, 5, 6, 7, 8, 10 bindings half)
- [x] Common rows and B1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-BIND implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 187, ED-BIND implementer)

**Tasks Completed**: items 1-9 and every required test file. Evidence: `tmp/ed-editor-20260926-s186/ED-BIND/attempt-1/`
(`intent.md`, `notes.md`, `pre-edit-hashes.txt`, `post-edit-hashes.txt`, `final-hashes.txt`).

**Implementation**:
- `sites.ts`: `SiteTable` of bindings with stable `bindingId`s. A fresh `eval-result` re-keys in two passes: a clean
  span mapping first (a keyed binding landing on a same-family key with a new ordinal MIGRATES), then keyed bindings by
  key; otherwise `unbound`, or `stale` when the mapping was touched; dropped at the second miss unless it carries an
  overlay or an ExternalFile entry. `bindings.sites` update by id, then span, then key, in the session's current
  revision (`DocSync.baseRevision`, since the session remaps spans on `doc-changed`); unmatched bindings of a
  recomputed form become `unbound`.
- `write.ts`: `SiteWriter` implements `BindApi`; `writeSite` is the one write path (slider, drag, CC and later
  ED-PARAMS handles). OVERLAY sends `set-tweak`; SOURCE-EDIT and `commit` verify the text at the mapped span against the
  last-known literal, format it (integer / at most 6 decimals keeping a `.` / stepped), dispatch one CodeMirror change,
  then `eval` the owning form's span; one eval in flight per form, latest values re-applied after the reply. In-flight
  slots keep the evaluated form span because the writer's own edit touches the `eval-result` form mapping.
  `stale-binding` reasons handled as the plan states. Tier badges "next cycle" / "re-evaluate to hear".
- `drag.ts`: `DragController` (pointer-down on a bound literal, vertical drag, ParamMeta range/curve else 1% of
  max(|v|, 1) per px, 3 px click slop) writing through `BindApi.writeSite`; the overlay widget `StateField`.
- `panel.ts`: slider rows grouped by origin plus value displays per name; each row subscribes to its own
  `site:<id>`/`name:<n>` keys; the table's store subscription is registered first so a batch re-keys before rows
  repaint; a whole `eval-result` is one rebuild from the client listener.
- `directives.ts`: control panel grouped by label or line, entry markers for directive lint codes, the persistence mode
  select and Save; ExternalFile mode renders the set.
- `routing.ts`: CC routing (directive `bindings` by key, else span containment plus `call.param`; `ch`, file default,
  omni otherwise; mappings learned since the last `eval-result`; ExternalFile through the set), 0..127 scaling, learn in
  both modes, verified `directive-edit` application (declined outside Directive mode, per E4).
- `persistence.ts`: `EditorBindingSet` in the 14.5.8 format (keys or `{span, param}`, overlays kept, atomic
  `renameAll` for ordinal migrations), unknown `v` ignored with a notice; `Persistence` mode with the panel snapshot on
  switching into ExternalFile. `save.ts`: mode-scoped save through `deps.files`.
- `mount.ts`: `BindArea` wires the store, client (`eval-result`, `bindings`, `stale-binding`, unsolicited
  `directive-edit`), the drag/overlay extensions via `StateEffect.appendConfig`, `deps.midi` polled every 500 ms (it
  appears only after the user enables MIDI) and sets `deps.bind`. `bind.css` is loaded as a Vite asset URL.

**Tests** (`editor/test/bind/`, all on `RecordingTransport` with scripted fixtures): write (criterion 2), drag, panel
(criterion 3), directives (criterion 5), save (criterion 6, two mode tests), multisite (criterion 7), reactive
(criterion 8, all twelve 14.5.5 batch shapes plus a re-keying batch, render counters and a render-history spy),
reconcile (criterion 10 bindings half), routing, persistence, sites: 11 files, 74 tests.

**Verification** (session 187, logs under `target/fe-logs/`, all `exit=0`, run after the final code change):
- V1 `ed-bind-build-s187-1.log`; V2 `ed-bind-clippy-s187-1.log`; V3 `ed-bind-nextest-s187-1.log` (989 run, 989
  passed, 2 skipped); V3t `ed-bind-cargotest-s187-1.log` (lib 968 passed + cli 9 + directive_fixtures 2 +
  spec_fixtures 10 = 989 passed, 0 failed); V7 `ed-bind-fmt-s187-1.log`; V6a `ed-bind-wasm32-s187-1.log`; V6b
  `ed-bind-wasm32-hostwasm-s187-1.log`; V6c `target/ed-wasm/ED-BIND.wasm` copied; V4 `ed-bind-rs-lines-s187-1.log`
  (max 799 lines, `src/dsp/build.rs`).
- E0 `ed-bind-node-s187-1.log` (node v26.9.0, npm 11.19.1); E1 `ed-bind-npm-ls-s187-1.log`; E2
  `ed-bind-npm-check-s187-1.log`; E3 `ed-bind-npm-test-s187-1.log` (43 files, 256 tests passed); E4
  `ed-bind-npm-build-s187-1.log`; E4c `ed-bind-dist-check-s187-1.log`; E5 `ed-bind-ts-lines-s187-1.log` (max 447 lines,
  `editor/test/bind/fixtures.ts`).
- B1 `ed-bind-own-s187-1.log`: 11 test files, 74 tests passed.

**Notes**:
- No Rust file was touched; no file outside this plan's writePaths was edited.
- The reorder-migration test uses an insertion before the same-named sites (every literal maps cleanly, ordinals
  shift); a reorder that retypes a literal leaves that binding STALE for re-confirmation, per 14.5.8.
- Directive-mode ordinal rewriting in `#@` text on reorder stays with the session (its scripted table supplies the
  migrated keys); the editor migrates its own ExternalFile set keys.

### CLOSING NOTE (ED-FINAL, session 188)

Status confirmed Completed (accepted). Final-tree evidence (ED-FINAL attempt-2, `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/`): join integrity re-checked (this plan's hashes OK or explained); every row exit=0 in `target/fe-logs/ed-final-*-s188-1.log`: build, build-lsp, clippy, clippy-lsp, fmt, nextest (1007 passed, 1 skipped), cargo test, both wasm32 builds, clippy wasm32 host-wasm, npm ci/check/test (54 files, 336 tests)/build (`VACTROL_REQUIRE_SESSION_ABI=1`), real-wasm vitest (3 files, 20 tests), Tauri fetch/check/fmt, session subset, lsp_smoke, spec fixtures. Own evidence: `test/bind/*` (11 files) in target/fe-logs/ed-final-npm-test-verbose-s188-1.log; the tier observables are cross-checked against the real artifact by `criteria.test.ts` criteria 2 and 3.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-code.md, vactrol-editor-midi.md. **Parallel**: vactrol-editor-wasm.md,
  vactrol-editor-visual.md
- **Next**: vactrol-editor-params.md
