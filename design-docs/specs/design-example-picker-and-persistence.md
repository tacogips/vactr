# Example Picker, Flashy Default Document and Document Persistence

Status: Proposed (2026-10-16, workflow session 379, branch `wf/examples`).

This design adds four things to the editor (`editor/`, web and Tauri shells):

1. a build-time **example registry** over the repository `examples/` tree;
2. a toolbar **document picker** (examples, saved documents, file open,
   default template actions) whose open action is one undoable replacement;
3. a **flashy default document** (a new `examples/showcase.vact`) shown on
   first open, loaded but never auto-played;
4. **persistence**: web draft autosave and a named document library in
   `localStorage`, and a **default template** choice (web: `localStorage`;
   Tauri app: a narrow Rust command writing one fixed file in the app config
   directory).

User decisions that remain open are in
[`design-docs/user-qa/pending-examples-questions.md`](../user-qa/pending-examples-questions.md)
(XQ1-XQ3). The recommendations there are followed by default.

## 1. Scope

In scope: the registry and its manifest, the picker UI, the web storage
module, the default-template service (web and app), startup precedence, the
showcase example, the Tauri commands, tests (vitest unit and integration, the
wasm evaluation test over every registered example, Tauri `cargo check` and
`cargo test`, a Playwright e2e scenario), README and e2e README updates.

Out of scope:

- `editor/src/code/input.ts`, `keyboard.ts` (owned by `wf/vim-mode`),
  `atlas.ts`, `renderer.ts` (owned by `wf/code-render`), and the editor
  settings UI. This design does not edit them.
- Changing the evaluator, the session protocol, or the wasm core.
- Wiring the bind-panel Save button or `FileAccess` to a new file model. The
  existing Save (bind control, `FileAccess.save`/`saveSidecar`) is unchanged.
- Native-tier-only and external-sample examples in the picker (section 3.3).
- Draft autosave and the named library in the Tauri app (XQ2).
- Evaluating, hushing or stopping on document replacement. Replacing the
  buffer is an edit like any other; it never evaluates (no autoplay).

## 2. Current behavior (baseline, checked 2026-10-16)

- `editor/src/app/main.ts` `boot` picks the tier (`?session=` native, Tauri
  native via `TauriTransport` with a wasm fallback, else the browser wasm
  tier), then `createEditor(root, deps)` mounts the shell, `MOUNT_ORDER`
  areas and the song controls. The document starts empty:
  `editor/src/code/mount.ts` creates `DocumentSync(client.document('main.vact'), Text.of(['']))`.
- `CodeSurface` (`editor/src/code/surface.ts`) is the single
  `@codemirror/state` authority with `history()`; `dispatch` applies each
  transaction to `DocumentSync` exactly once. Paste and IME already use
  `isolateHistory.of('full')`. The undo byte budget is 32 MiB
  (`HISTORY_UNDO_BYTES`).
- `FileAccess` (`editor/src/platform/files.ts`): `open()` exists in
  `BrowserFiles`, `TauriFiles` and `MemoryFiles` but no UI calls it. Save is
  the bind control's Save button (`bind/mount.ts` `save()` -> `bind/save.ts`)
  writing `main.vact` through a dialog.
- No unsaved-change tracking exists.
- `localStorage` holds only small guarded preferences (`vactr.sideFolded`,
  `vactr.section.*.folded`, `vactr.sampleMap`, `vactr.pkg.proxy`,
  `vactr.midi.active-inputs`); `pageStorage(win)` in `ui/shell.tsx` returns
  `null` when access throws.
- Tauri: commands `session_connect`, `session_send`, `session_close`,
  `self_check_enabled`, `self_check` in `editor/src-tauri/src/lib.rs`.
  `build.rs` is plain `tauri_build::build()` (no app-command manifest), so
  app commands are not ACL-gated; `capabilities/default.json` lists only
  `core:default`, `dialog:allow-open/save`, `fs:allow-read/write-text-file`
  with no static fs scope. Dialog-picked paths enter the fs scope for the
  session only, so a path stored for a later launch cannot be read again.
- No repository example uses a visual (`out o0`) chain; visual chains
  evaluate in the wasm tier (`editor/test/wasm/abi.test.ts`,
  `criteria.test.ts`).
- `npm run build` defaults to the RELEASE wasm
  (`target/wasm32-unknown-unknown/release/vactr.wasm`), which is absent in
  this worktree; only the debug cdylib is prebuilt. `npm run test:style` and
  every e2e script serve `editor/dist`.

## 3. Example registry

### 3.1 Source and loading

- New module `editor/src/examples/registry.ts` uses
  `import.meta.glob('../../../examples/**/*.vact', { query: '?raw', import: 'default' })`
  (LAZY: each example becomes its own chunk and is fetched on open, so the
  main bundle does not grow with the example count) and an EAGER raw glob of
  `examples/tracks/catalog.json` (parsed with `JSON.parse`).
- `tsconfig.json` has `types: []`, so a local ambient declaration
  `editor/src/examples/glob.d.ts` declares the minimal `ImportMeta.glob`
  signature used here. `tsconfig.json` is not changed.
- `editor/vite.config.ts` gains `server.fs.allow: ['.', '../examples']` so
  `npm run dev` can serve the example chunks (build mode needs nothing).
  This is the only Vite config change.
- Vitest resolves `import.meta.glob` itself; registry tests run under the
  default jsdom environment.

### 3.2 Manifest

Hand-written metadata lives in `editor/src/examples/manifest.ts`:

| Field | Type | Rule |
|---|---|---|
| `id` | string | path under `examples/` without `.vact`, e.g. `acid-techno`, `synth-labs/lorenz`, `tracks/house/warm-current`; matches `^[a-z0-9][a-z0-9/-]{0,95}$` |
| `title` | string | 1-60 chars |
| `description` | string | 1-140 chars, taken from the file's header comment (tracks: `catalog.json` `desc`) |
| `category` | string | `Showcase`, `Examples`, `Synth labs`, `Effect labs`, `Song mode`, or `Tracks: <genre>` |
| `requires` | `('audio-input' \| 'midi')[]` | empty for almost every entry |

- Track entries are generated from `catalog.json` (`g`, `slug`, `title`,
  `desc`, `bpm`): `id = tracks/<g>/<slug>`, `category = Tracks: <g>`,
  description `"<desc> <bpm> BPM."`. No track metadata is duplicated by hand.
- `EXCLUDED: { path, reason }[]` lists every `.vact` under the scanned
  directories that is not registered, with a recorded reason.
- `DEFAULT_EXAMPLE_ID = 'showcase'`.
- Registry API (names fixed for the plans): `listExamples(): ExampleEntry[]`
  (manifest order, grouped by category), `findExample(id)`,
  `loadExample(id): Promise<string>` (rejects for an unknown id or a failed
  chunk load).
- A unit test asserts: every manifest entry has a glob loader; every `.vact`
  under `examples/`, `synth-labs/`, `effect-labs/`, `song-mode/` and
  `tracks/*/` is in exactly one of the registered set or `EXCLUDED`; ids are
  unique and valid; `DEFAULT_EXAMPLE_ID` is registered with empty `requires`.

### 3.3 What is registered

Scanned directories: top-level `examples/*.vact`, `synth-labs/`,
`effect-labs/`, `song-mode/`, `tracks/<genre>/`. The rule:

- Registered = loads and evaluates without error in the wasm browser tier
  (section 8.2 proves it). Registered entries are shown in every shell.
- Excluded (hidden everywhere, reason recorded in `EXCLUDED`):
  - needs an external sample file that is not bundled:
    `sample-timestamps.vact` (`./samples/amen-recreation/source.wav`),
    `tracks/breakcore/glass-teeth.vact` (needs the
    `examples/samples/amen-recreation` WAV, which is not bundled);
  - needs a native-tier-only host: `quad-stems.vact` (four-output host);
  - any further example that fails the wasm evaluation test, unless the
    example itself is fixed (the evaluator is never changed).
- Marked (shown with a badge, still openable): entries whose `requires`
  is non-empty. `live-external-voices.vact` is `['audio-input']` (it reads
  `host-in-l`/`host-in-r`; disconnected lanes are zero). No repository
  example needs MIDI input today, so no entry carries `midi`; the value
  exists so the issue's MIDI flag has a defined badge.
- Native-tier-only examples are not offered in the Tauri app either in this
  release (XQ3).

A static scan found no other sample or MIDI dependency (`grep -liE`
with `sample|\.wav|\.flac|load\(`, `midi|\bcc\b` and
`external|adc|live-in|input` over top-level, `song-mode/`, `synth-labs/`
and `effect-labs/`: no MIDI hits, and the remaining sample hits are
comments such as "sample-free"; the tracks README names only breakcore as
sampled). The evaluation test is
the authority; the implementation plan records each exclusion it adds.

### 3.4 Showcase (built-in flashy default)

New `examples/showcase.vact` (MIT, project-owned, hand-written). Chosen over
the existing techno examples because none of them has a visual chain, and
the issue asks for a visually and sonically busy first impression.

Content contract (the plan writes the text; this fixes the bounds):

- header comment: what it is, "press Run (Mod-Shift-Enter) to play";
- `use-bpm` between 126 and 136;
- 5 to 8 sound slots using built-in, sample-free voices already proven in
  the browser tier (for example `:phase-drum`, `:dual-hat-voice`,
  `:acid-bass` with accent/slide, `:wobble-bass`, `:fm` with several
  algorithms); every slot `gain` at most 0.32 and the sum of slot gains at
  most 1.2 (the acid-techno levels);
- one visual chain to `out o0` built from documented Hydra-model sources
  and transforms (`osc`, `kaleid`, `rotate`, `modulate`, `colorama`, ...)
  driven by time signals;
- at most 40 lines; no samples, no MIDI, no host input.

Verified by the wasm test (no error diagnostics, no failed forms, `tempo`
published, `playing` events on at least 4 slots and at least one render
`program` record within 4 s of ticks) and by the e2e loudness check
(section 8.4).

## 4. Document session and replacement

New `editor/src/documents/session.ts`, `DocumentSession`, owns the
"which document is this" state on top of `CodeSurface` (it never creates
another editor state or view):

- `source: DocSource` where
  `DocSource = {kind:'example', id} | {kind:'library', id, name} | {kind:'template'} | {kind:'file', name} | {kind:'new'}`.
- `baseline: Text | null`, set on every load and save; `dirty()` is
  `baseline === null ? !doc.eq(Text.empty) : !doc.eq(baseline)`. A restored
  draft keeps its recorded `source` with `baseline = null` (treated as
  unsaved).
- `replace(text, source)`: ONE transaction
  `{ changes: {from: 0, to: doc.length, insert: text}, selection: {anchor: 0}, annotations: isolateHistory.of('full'), userEvent: 'input.replace.document', scrollIntoView: true }`
  so one undo restores the previous text exactly and one redo reapplies it.
  It then sets `source` and `baseline`.
- `loadInitial(text, source)` (startup only): applies the same change with
  `Transaction.addToHistory.of(false)` (no undo step back to an empty
  buffer), and only if the buffer is still empty and unchanged since mount;
  otherwise it is skipped so a user edit made during startup is never
  overwritten. For startup row 1 with a web draft, `loadInitial` of the
  draft is followed by one `replace` of the example (section 7).
- Replacement never evaluates, hushes or stops. Song controls observe the
  change as an ordinary document edit.

Unsaved-change protection when opening anything from the picker (example,
saved document, file, New, template): if `dirty()` the picker shows an
inline confirmation row ("Replace the current document? Undo (Mod-Z)
restores it." with Replace and Cancel); otherwise it replaces directly. In
both cases the replacement stays undoable. No `window.confirm`.

## 5. Web persistence (`localStorage`)

New `editor/src/documents/storage.ts`, `DocumentStorage`, constructed with
the page storage (`pageStorage(win)`; extended locally to the `Storage`
methods it needs: `getItem`, `setItem`, `removeItem`, `key`, `length`).

### 5.1 Schema v1

| Key | Value |
|---|---|
| `vactr.doc.v1.draft` | `{v:1, text, source: DocSource, savedAt}` |
| `vactr.doc.v1.item.<id>` | `{v:1, id, name, text, createdAt, updatedAt}` (`id` from `crypto.randomUUID()`, else time+random) |
| `vactr.doc.v1.default-template` | `{v:1, template: DefaultTemplate}` (web only, section 6) |

- Versioning: a value whose `v` is not `1` or whose shape does not validate
  is ignored (treated as absent) and NEVER deleted or overwritten by
  reads, so data from another build is not destroyed. A future schema uses
  a new key prefix (`vactr.doc.v2.`); v1 has no migration.
- Listing scans keys with the `vactr.doc.v1.item.` prefix (no separate
  index to keep consistent). Corrupt items are skipped and counted; the
  picker shows "N unreadable saved documents ignored" when N > 0.
- Names: trimmed, 1-80 chars, no control characters, unique among valid
  items (exact match). A taken name in Save as or Rename shows an inline
  error; nothing is overwritten implicitly.
- Size cap: `DOC_MAX_CHARS = 262144` UTF-16 code units for the draft,
  library items and web template snapshots. Larger documents are not
  written; the status says to use the file Save instead. (This also keeps
  autosave off the 1 MiB canvas evidence workload.)

### 5.2 Failure handling

Every storage access is inside try/catch; no storage method throws. At
construction a probe (`setItem`/`removeItem` of `vactr.doc.v1.probe`)
decides `available`. Write results are
`{ok: true} | {ok: false, reason: 'unavailable' | 'quota' | 'too-large' | 'invalid-name' | 'name-taken' | 'missing' | 'error'}`
(`quota` = `QuotaExceededError` or legacy code 22/1014). Unavailable storage
(private mode, `SecurityError`, `null` storage) disables draft autosave and
the library section with one notice; the editor still opens with the
startup document.

### 5.3 Draft autosave (web only)

- The draft holds UNSAVED edits only. A document change schedules a
  trailing 1000 ms debounce (no max-wait); `pagehide` and
  `visibilitychange` to hidden flush it immediately. When the debounce
  fires, the session's dirty state is read THEN (not at change time): a
  dirty buffer writes the draft, a clean buffer clears it.
- Every load or save that leaves the buffer clean (startup `loadInitial`
  of a clean source, a picker replacement, Save, Save as) cancels the
  pending write and clears the draft. So first open writes no draft, and a
  reload after opening an example unchanged falls through to the default
  template or the showcase.
- Not written when the text exceeds `DOC_MAX_CHARS` or storage is
  unavailable; a quota failure shows one status notice until the next
  successful write.
- The draft records `source` so that after a reload Save still targets the
  same library entry.
- Opening anything replaces the buffer and clears the draft (above). No
  path discards unsaved text that is not in the undo history: a picker
  open is an undoable `replace` of the current buffer, and at startup an
  existing draft is either restored (row 2) or, when `?example=` wins
  (row 1), loaded first and then replaced undoably (section 7). One Mod-Z
  then brings the draft text back. The buffer is dirty again, so the next
  autosave writes it back to the draft key.

### 5.4 Named library (web only)

Picker actions: Save (overwrites the current `library` source entry if it
still exists, else behaves as Save as), Save as (inline name field), Open
(row click), Rename (inline name field), Delete (inline two-step confirm).
Entries are listed newest `updatedAt` first, above the examples.

## 6. Default template

`DefaultTemplate = {kind: 'example', id} | {kind: 'snapshot', name, text}`.

- "Use current document as default template": stores `{kind:'example', id}`
  when the current source is an example and the buffer is clean, else a
  snapshot of the buffer (name: the library name, file name, or example
  title). A snapshot is a frozen copy: later edits to a library entry or a
  file do not change it. Snapshots are used because the app's fs scope is
  dialog-only per session, so a stored file path could not be read at the
  next launch.
- "Reset to built-in default": clears the stored choice.
- "New": replaces the buffer (with the section 4 protection) by the chosen
  template, or by the showcase when none is set.
- The picker marks the example that is the current default.
- An `example` template whose id is no longer registered falls back to the
  built-in default with a status notice.

`TemplateStore` (interface in `editor/src/documents/types.ts`):
`get(): Promise<DefaultTemplate | null>`, `set(t): Promise<StorageResult>`,
`reset(): Promise<StorageResult>`; no method rejects (`get` failures
resolve `null`, write failures resolve `{ok: false, ...}`).

- Web: `LocalTemplateStore` (in `editor/src/documents/storage.ts`) over
  `DocumentStorage` (key above). Offered on the web per XQ1 (recommended).
- App: `TauriTemplateStore` (`editor/src/documents/tauri-templates.ts`)
  calls the Rust commands through the injected
  `invoke` from `@tauri-apps/api/core` (used whenever `isTauri(win)`, also
  after a native-connection fallback to the wasm tier).

### 6.1 Why a Rust command in the app

WKWebView `localStorage` for the `tauri://localhost` origin is persistent
in normal use, but it is tied to the webview data store and origin: dev
builds (`http://localhost:<port>`) and release builds use different
origins, and the store is cleared with website data. More decisively, the
acceptance "persists across app restarts" can be proven by `cargo test`
against a real file, while webview storage persistence cannot be tested in
this repository. The command is small and needs no capability change.

### 6.2 Tauri commands

New module `editor/src-tauri/src/template.rs`, registered in
`generate_handler!` in `lib.rs`:

| Command | Arguments | Result |
|---|---|---|
| `default_template_get` | none | `Result<Option<DefaultTemplate>, String>` |
| `default_template_set` | `template: DefaultTemplate` | `Result<(), String>` |
| `default_template_reset` | none | `Result<(), String>` |

- File: `<app_config_dir>/default-template.json`, content
  `{"version": 1, "template": {...}}` (serde, `kind` tag, lowercase). The
  path is fixed in Rust; no command takes a path or file name.
- Validation in `set`: `id` matches the manifest id pattern; `name` 1-120
  chars without control characters; `text` at most 1 MiB UTF-8. Invalid
  input returns `Err` and leaves the file unchanged.
- Write: `create_dir_all`, write `default-template.json.tmp` in the same
  directory, then `rename` (atomic replace).
- `get`: missing file -> `Ok(None)`; unreadable, unparsable, wrong
  `version` or invalid content -> `Ok(None)` (startup continues; the file
  is left in place).
- `reset`: removes the file; missing file is `Ok(())`.
- The file logic takes the directory as a parameter so tests use a unique
  directory under `std::env::temp_dir()` (no new dependency).
- `capabilities/default.json` is unchanged: app commands are not ACL-gated
  (plain `tauri_build::build()`, no `AppManifest`; the existing `session_*`
  and `self_check*` commands already run with no capability entry; the plan
  re-confirms this against the pinned `tauri =2.11.6`), the fs plugin scope stays
  empty, and the command writes only through `std::fs` to the fixed path.

## 7. Startup precedence

After `createEditor` mounts the documents area, `boot` awaits its
`ready` promise (which never rejects) before returning. The first match
wins:

| # | Source | Web | App |
|---|---|---|---|
| 1 | explicit URL `?example=<id>` (registered id) | yes | yes (when a URL is supplied) |
| 2 | draft `vactr.doc.v1.draft` | yes | no (XQ2) |
| 3 | chosen default template | `localStorage` | Rust command |
| 4 | built-in default `showcase` | yes | yes |

- Row 1 with an existing valid draft (web only): the draft must stay
  recoverable. Startup first applies the draft with
  `loadInitial(draft.text, draft.source, {restored: true})` (no undo
  step), then the example with `session.replace(exampleText, {kind:'example', id})`,
  which is ONE undoable step. The status shows
  "Opened <id>; Undo (Mod-Z) restores your unsaved draft". The replace
  is clean, so the DR-L1 rule clears the draft key, but the draft text is
  the undo target. Undoing makes the buffer dirty and the autosave
  rewrites the draft. Precedence is unchanged: the example is what the
  user sees. The app has no draft, so this never applies there.
- An unknown `?example=` id adds a status notice and falls through.
- "Explicit file" has no startup form today (no shell passes a file at
  launch; `vactr serve <file>` evaluates the file but never filled the
  buffer). Open file is an in-session picker action.
- If loading the selected source fails (chunk load error, unregistered
  template id), the next source is tried; if all fail the buffer stays
  empty with a status notice. Startup never throws.
- The document is loaded, not evaluated: no `eval`, no audio resume. The
  Run button (or Mod-Shift-Enter) plays it.
- `createEditor` gains an optional third parameter
  `{ documents?: DocumentsOptions }`; only `boot` passes it, so existing
  `createEditor` tests keep an empty document and no picker.
  `DocumentsOptions = { shell: 'web' | 'app', storage, templates, search }`.

## 8. Picker UI

`editor/src/documents/mount.ts` mounts it; the view is
`editor/src/ui/picker-view.tsx` with styles in `editor/src/ui/picker.css`
(design tokens from `theme.css`; no edits to `app.css`, `transport-view.tsx`
or code-pane files).

- Trigger: a text button "Documents" (class `vact-doc-picker-toggle`,
  `aria-haspopup="dialog"`, `aria-expanded`) appended to the transport pane
  in its own `section[data-doc-picker]`, the same additive pattern as the
  song controls.
- Panel: `div role="dialog" aria-label="Documents"` overlaying the top of
  the code pane; not modal; `Escape` closes and returns focus to the
  trigger; opening focuses the filter field. After any open, New or
  replace action the panel closes and focus moves to the code editor, so
  Mod-Z undoes the replacement immediately.
- Contents, in order: a filter field (`input type="text"`, filters titles,
  descriptions and categories); an action row (New, Open file..., Save,
  Save as..., Use as default template, Reset default template; Save and
  Save as only when the library is available on the web; Reset only when a
  default is set); "Saved in this browser" (web only) with per-row Open,
  Rename, Delete; "Examples" grouped under category headings, each row
  showing title, description, `needs audio input`/`needs MIDI` badges and a
  `default` badge.
- Keyboard: every control is a `button` or text input in tab order; Up/Down
  move focus between list rows (roving `tabindex`); Enter/Space activate;
  inline forms submit on Enter and cancel on Escape.
- Open file... calls the existing `deps.files.open()` (dialog in Tauri, File
  System Access or `<input type=file>` on the web); `FileCancelled` is
  silent; the file Save path is unchanged.
- Style (design-ui-style.md): square, flat, opaque fills, 1px borders,
  `appearance: none`, token spacing, 2px solid focus ring, touch sizing.
  `editor/test/style/ui-style.mjs` is extended to open the picker before
  its scan (a mechanical update listed in the plan) so the picker is gated
  by `npm run test:style`.
- Status messages go to a `role="status"` line in the panel footer.

## 9. Verification

### 9.1 Build prerequisite

The release wasm is absent; every command that needs `editor/dist`
(`npm run build`, `npm run test:style`, e2e) runs after
`VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build`
(the e2e README already documents this non-gating form).

### 9.2 Wasm evaluation test

`editor/test/wasm/examples.test.ts` (`@vitest-environment node`) uses
`test/support/wasm.ts` (`loadVactrWasm`, `ackSampleInstalls`, `jsonOf`) like
`first-track.test.ts`: for every registered entry it starts a session,
subscribes, sends one whole-document `eval` of the raw file text read from
disk, acknowledges worklet-bound records, and fails on any `error`
diagnostic or failed form, naming the example. The showcase additionally
asserts the section 3.4 activity bounds. A fresh session per example is the
default; if wall time is a problem, one session with `hush` between
examples is acceptable and recorded.

### 9.3 Unit and integration tests (vitest, jsdom)

- `test/examples/registry.test.ts`: section 3.2 assertions.
- `test/documents/storage.test.ts`: round trips; corrupt JSON, wrong `v`,
  invalid shape ignored and not deleted; unreadable count; quota error on
  `setItem`; throwing `localStorage` getter; `null` storage; name rules;
  size cap.
- `test/documents/session.test.ts`: `replace` is one undo step and one redo
  step even right after typing; `loadInitial` adds no undo step and skips a
  non-empty buffer; `dirty()` rules.
- `test/documents/startup.test.ts`: the section 7 table for both shells,
  including fall-through on an unknown `?example=`, an unregistered template
  id and a rejected chunk load, and row 1 over a web draft (the result
  carries the draft as `underlying`; after startup one undo restores the
  draft text).
- `test/documents/templates.test.ts`: web store; Tauri store with a mock
  `invoke` (argument shapes, `get` resolving `null` on rejection).
- `test/documents/picker.test.ts`: keyboard navigation, dirty confirmation,
  library actions, Tauri shell hides library and draft.
- `test/app/main.test.ts`: `boot` tests get the documents area; any
  assertion that assumed an empty startup document is updated mechanically
  and listed.

### 9.4 E2E scenario

New `editor/test/e2e/examples.mjs` and `npm run e2e:examples`, reusing
`serve.mjs` (`127.0.0.1`, port 0, so never 5173/5174) and
`installSilentSink` (Chromium, headless, `--mute-audio`; post-sink peak must
be exactly 0, no direct connections). It loads `/?perf=1` and reads the
buffer through `window.__vactrPerf.doc()`:

1. fresh context: the buffer equals `examples/showcase.vact`; after 1.5 s
   the pre-sink onset count is 0 (not auto-played);
2. click the audio control and Run: pre-sink onsets >= 1 within 10 s,
   pre-sink peak <= -1 dBFS; then Hush;
3. open the picker from the toolbar with the keyboard, choose
   `acid-techno` with arrows and Enter: buffer equals that file; Mod-Z
   restores the showcase; Mod-Shift-Z (redo) reapplies acid-techno;
4. type a marker, wait 1.5 s, reload: the buffer equals the edited text;
5. Save as `e2e-one` (buffer now clean), open `acid-techno` (no
   confirmation), type a character, open `e2e-one`: the confirmation row
   appears; Replace: text matches the saved text; rename to `e2e-two`;
   delete: no longer listed;
6. open `fm6-algorithms` (clean), Use as default template, open
   `acid-techno`, New: buffer equals `fm6-algorithms`; reload (the buffer
   is clean, so no draft): buffer equals `fm6-algorithms` (startup row 3);
   Reset default template, New: buffer equals the showcase;
7. type a marker (a draft now exists), wait 1.5 s, load
   `/?perf=1&example=wobble-techno`: buffer equals wobble-techno and the
   status contains "restores your unsaved draft"; focus the code input and
   press ControlOrMeta+z: buffer equals the marker text (draft recovered);
   wait 1.5 s and reload: buffer equals the marker text (draft rewritten);
8. a context whose `localStorage` throws on every access (init script):
   the editor opens with the showcase and the picker shows the
   storage-unavailable notice.

Output: one JSON line `{testsRun, testsPassed, failureCount, checks}`; exit
0 pass, 1 failure, 2 blocked (missing dist or browser). Logs under
`tmp/examples-e2e/`. The existing `npm run e2e` canvas evidence harness is
not changed; its behavior profile uses `ta.fill`, which replaces the new
startup document.

### 9.5 Commands

- `cd editor && npm run check`
- `cd editor && npx vitest run test/examples test/documents test/wasm/examples.test.ts test/app/main.test.ts`
- `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml`
- `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml`
- under `/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`:
  `cd editor && npm test && VACTR_WASM=../target/wasm32-unknown-unknown/debug/vactr.wasm npm run build && npm run test:style && npm run e2e:examples`

Artifact roots for every plan: `target/`, `tree-sitter-vact/tree-sitter-vact.wasm`,
`editor/node_modules`, `editor/dist`, `tmp/`, Playwright outputs
(`tmp/examples-e2e/`, `tmp/ui-style/`).

## 10. Documentation

- `README.md` "Editor": the picker, the startup precedence table, web
  storage keys and limits, the app default-template file location, the
  `?example=<id>` URL form, `npm run e2e:examples`.
- `editor/test/e2e/README.md`: the examples scenario and its command.

## 11. Rollout constraints

- New files are additive; edits to shared files are limited to
  `app/main.ts` (boot wiring, `createEditor` optional parameter),
  `vite.config.ts` (`server.fs.allow`), `package.json` (`e2e:examples`
  script), `src-tauri/src/lib.rs` (module and three handler names),
  `test/style/ui-style.mjs` (open the picker), README files.
- Every touched source file stays under 1000 lines.
- No autoplay; tests stay silent; system volume is never touched.
- Only repository examples (MIT, project-owned) are bundled.

## 12. Risks

| Risk | Mitigation |
|---|---|
| Example fails browser-tier evaluation | Section 9.2 test; fix the example or exclude with a recorded reason |
| Showcase too loud | Gain bounds (3.4) and the e2e pre-sink peak gate |
| Storage errors block startup | No-throw storage API; startup fall-through; e2e step 8 |
| Draft shadows the default template | By design on the web (precedence 2 before 3); New applies the template; app has no draft (XQ2) |
| Autosave cost on huge documents | 262144-char cap and trailing 1000 ms debounce |
| Merge conflicts with `wf/vim-mode`, `wf/code-render` | Section 11 file list; no edits to their files |
| Bundle growth | Lazy per-example chunks; only `catalog.json` is eager |
| Tauri surface | Fixed path, validated input, no capability or fs scope change |
