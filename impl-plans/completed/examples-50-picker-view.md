# EX-50: Document Picker View (Solid component and CSS)

**Status**: Completed
**Plan ID**: EX-50 (wave 1, parallel)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` section 8; `design-docs/specs/design-ui-style.md`
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

## Intent and Context

Users open examples and saved documents from a toolbar picker. This plan
writes the presentational Solid component and its stylesheet. All
behavior (what opening does, storage, confirmation logic) comes in
through `PickerProps.actions` and accessors (EX-00 contract); EX-60 wires
it. The view owns only view-local state: the filter text, which inline
name form or delete confirmation is open, and roving focus.

UI style rules (design-ui-style.md): every element `border-radius: 0`,
opaque token fills, 1px borders, `appearance: none`, token spacing
(`--vt-space-*`), 2px solid focus ring (`--vt-focus-w`), control heights
`--vt-control-h`, never a native widget look. `editor/test/ui/style-tokens.test.ts`
scans every CSS file under `editor/src`: colors must be `var(--vt-*)`
tokens, no named colors, no raw hex.

## Non-goals

- No storage, registry, session or Tauri logic; no `window.confirm` or
  `window.prompt`.
- Do not edit `app.css`, `theme.css`, `transport-view.tsx`, `shell.tsx`,
  `icons.tsx` or any `editor/src/code/*` file.
- No global keyboard shortcut.

## Dependencies

- **dependsOn**: EX-00 (`PickerProps`, `PickerActions`, `ExampleEntry`,
  `LibraryEntry`, `LibraryView`, `Shell`)
- **Blocks**: EX-60, EX-70 (style gate)

## writePaths

- `editor/src/ui/picker-view.tsx`
- `editor/src/ui/picker.css`
- `editor/test/ui/picker-view.test.tsx`
- `impl-plans/active/examples-50-picker-view.md`
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-50` (artifact root; evidence logs)

## sharedPaths

None.

## Patterns to Imitate

- Solid view with signals and `aria-*`: `editor/src/ui/shell.tsx:FoldButton`,
  `SectionHeader`; `editor/src/ui/transport-view.tsx`.
- Stylesheet link loading: `editor/src/midi/mount.ts` (`STYLESHEET = new URL('./midi.css', import.meta.url).href`
  plus a `link[data-...]` guard as in `editor/src/code/mount.ts:addStylesheet`).
- Token usage: `editor/src/app/app.css` rules for `.vact-icon-button`,
  `.vact-primary`, text inputs.
- Solid component tests: `editor/test/ui/controls.test.ts`.

## File-level Changes

### `editor/src/ui/picker-view.tsx`

- `export function PickerView(props: PickerProps): JSX.Element`.
- `export function ensurePickerStylesheet(doc: Document): void` (adds
  `link[data-vact-picker-css]` once).
- Structure (classes and attributes are a test and style contract):
  - Trigger: `button.vact-doc-picker-toggle` text "Documents",
    `aria-haspopup="dialog"`, `aria-expanded` from `open()`, click ->
    `actions.toggle()`.
  - Panel (rendered only when `open()`): `div.vact-doc-picker[role="dialog"][aria-label="Documents"]`.
    On open, focus the filter input. `Escape` anywhere in the panel ->
    `actions.close()` and focus the trigger (unless an inline form is open:
    then Escape cancels that form only).
  - Status: `span.vact-doc-picker-status[role="status"]` next to the
    trigger, rendered always (open or closed), text `status()`.
  - Filter: `input.vact-doc-picker-filter[type="text"][aria-label="Filter documents"]`;
    case-insensitive substring over title, description, category, and
    library names. ArrowDown in the filter focuses the first visible row.
  - Pending replace (when `pendingReplace()` is non-null):
    `div.vact-doc-picker-confirm[role="alertdialog"]` with the text and
    buttons "Replace" (`actions.confirmReplace`, focused when shown) and
    "Cancel" (`actions.cancelReplace`).
  - Action row `div.vact-doc-picker-actions`: New, Open file..., Save and
    Save as... (only when `canSave()`), Use as default template, Reset
    default template (only when `hasTemplate()`). Save: when
    `saveTarget()` is a name, call `actions.save()` (title "Save to
    <name>"); when null, open the Save as form instead.
  - Save as / Rename open an inline form: text input + Save/Cancel; Enter
    submits, Escape cancels; the action returns an error string -> keep
    the form open and show it in `.vact-doc-picker-error[role="alert"]`;
    `null` -> close the form.
  - Library section (only when `shell === 'web'`): heading "Saved in this
    browser"; when `library().available` is false show
    `.vact-doc-picker-notice` "Browser storage is unavailable; saving to
    this browser is disabled"; else rows newest first: open button (name +
    relative date text), Rename, Delete (two-step: first click shows
    "Confirm delete" + "Cancel"; confirm -> `actions.remove(id)`); when
    `unreadable > 0` show "<n> unreadable saved documents ignored".
  - Examples section: heading "Examples"; one `h3`-level group per category
    in `examples` order; each row `button.vact-doc-picker-item[data-example-id]`
    with title, description, badges `span.vact-doc-picker-badge`
    ("needs audio input", "needs MIDI", "default" when id equals
    `templateExampleId()`); click -> `actions.openExample(id)`.
- Keyboard: all row buttons are in one roving-`tabindex` list
  (`tabindex=0` on the active row, `-1` on the others); ArrowDown/ArrowUp
  move focus to next/previous visible row (library and example rows in
  display order), Home/End to first/last; Enter/Space use native button
  activation. Tab moves between filter, action row and list.

### `editor/src/ui/picker.css`

- Panel positioned absolutely below the transport bar over the code pane,
  max-height with internal scroll, solid `--vt-surface` fill, 1px
  `--vt-border-strong` border, `--vt-space-*` padding/gaps, z-index above
  the code canvas.
- Buttons/inputs: `appearance: none`, `border-radius: 0`, token fills and
  borders, `:focus-visible` outline `var(--vt-focus-w) solid var(--vt-accent)`.
- Badges: square, `--vt-raised` fill, `--vt-text-xs`.
- Only `var(--vt-*)` colors.

## Invariants

- The component calls only `props.actions`; it performs no storage, I/O
  or document edits.
- Every interactive element is a `button` or `input type="text"`.
- No `border-radius` other than 0; no shadows, gradients or transparency.

## Pitfalls

- Re-rendering lists must not lose roving focus: key rows by id
  (`<For>`), keep the active id in a signal.
- Filtering can remove the active row: fall back to the first visible row.
- Escape inside an inline form must not close the panel.
- Do not use `<select>`, `<dialog>`, `type="search"` or native confirm.
- CSS link guard: do not add the stylesheet twice.

## Tests (input -> expected)

`editor/test/ui/picker-view.test.tsx` (jsdom; render with `solid-js/web`
`render`; spy actions):
- `open()` false -> no `[role="dialog"]`; trigger `aria-expanded="false"`;
  click trigger -> `toggle` called.
- `open()` true -> dialog present; filter focused.
- three examples in two categories -> two group headings, three
  `[data-example-id]` rows in order; click second -> `openExample(id2)`.
- entry with `requires:['audio-input']` -> badge "needs audio input";
  `templateExampleId()` equal to an id -> "default" badge on that row.
- focus first row, ArrowDown -> second row focused; End -> last; Home -> first.
- filter "acid" -> only matching rows; ArrowDown stays within them.
- Escape on panel -> `close` called and trigger focused.
- shell 'app' -> no "Saved in this browser" section, no Save buttons
  even if `canSave()` true.
- web, `library().available` false -> unavailable notice text present.
- Save as: type "song" + Enter -> `saveAs('song')`; returning
  "Name already used" keeps the form and shows the `role=alert` text;
  returning null closes it.
- Delete: first click shows confirm; confirm -> `remove(id)`; Cancel ->
  not called.
- `pendingReplace()` set -> alertdialog with Replace focused; click ->
  `confirmReplace`.
- `hasTemplate()` false -> no Reset button.
- `saveTarget()` "song" -> Save click calls `save()`; `saveTarget()` null
  -> Save click opens the name form and does not call `save()`.
- `status()` "Saved song" with `open()` false -> status text visible next
  to the trigger.
- filter focused, ArrowDown -> first visible row focused.

## Verification

Evidence in `tmp/examples/EX-50/`, each log ending with its exit status.

1. `cd editor && npx vitest run test/ui/picker-view.test.tsx test/ui/style-tokens.test.ts > ../tmp/examples/EX-50/vitest.log 2>&1` -> all pass (style-tokens proves the CSS uses tokens only).
2. `cd editor && npm run check > ../tmp/examples/EX-50/check.log 2>&1` -> exit 0 for this plan's files.

Records use `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`. The built-page style
gate (`npm run test:style`) runs in EX-70.

## Completion Criteria

- [x] `PickerView` and `ensurePickerStylesheet` exported; classes and
      roles as specified.
- [x] All listed tests pass; `style-tokens.test.ts` passes.
- [x] Files under 1000 lines.
- [x] Progress Log with records and pre/post sha256.

## Edit Protocol

Fresh read before each edit; sha256 before/after; writePaths only; stop and
record on drift; no git index or branch operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored.
**Blockers**: None.

### Session: 2026-10-17
**Tasks Completed**: Implemented the presentational picker, token-only stylesheet, keyboard navigation, filter, inline save/rename/delete flows, replacement confirmation, and focused tests. The view calls behavior only through `PickerActions`. Final acceptance self-check passed.
**Blockers**: None for EX-50. The combined-tree integration and review gates remain downstream workflow work.

**Verification**:

```json
{"command":"bash -c 'cd editor; npx vitest run test/ui/picker-view.test.tsx test/ui/style-tokens.test.ts > ../tmp/examples/EX-50/vitest-attempt-6.log 2>&1; result=$?; printf \"\\nexitStatus=%s\\n\" \"$result\" >> ../tmp/examples/EX-50/vitest-attempt-6.log; exit \"$result\"'","exitStatus":0,"testsRun":21,"testsPassed":21,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-50/vitest-attempt-6.log","notes":"PickerView and style-tokens suites; complete log records 2 files and 21 passing tests."}
{"command":"bash -c 'cd editor; npm run check > ../tmp/examples/EX-50/check-attempt-6.log 2>&1; result=$?; printf \"\\nexitStatus=%s\\n\" \"$result\" >> ../tmp/examples/EX-50/check-attempt-6.log; exit \"$result\"'","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/examples/EX-50/check-attempt-6.log","notes":"Full TypeScript check passed on the shared tree."}
```

Earlier attempts and complete logs are retained in `tmp/examples/EX-50/`: `vitest.log` and `check.log` record attempt 1; `vitest-attempt-2.log` and `check-attempt-2.log` record attempt 2; `vitest-attempt-5.log` and `check-attempt-5.log` record attempt 5. Attempt 1 exposed three picker failures and strict type errors; attempt 2 passed its focused tests while the shared EX-30 Tauri template tests still had type errors; attempt 5 exposed the inline-form Escape propagation bug. The final run above follows those corrections and passes on the current source.

**SHA-256 (pre -> post)**:

- `editor/src/ui/picker-view.tsx`: absent (empty preimage `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`) -> `1f041d98f8c5cb4016aae821b896021c66f03920d2e6935c77c41ee68c83965e`
- `editor/src/ui/picker.css`: absent (empty preimage `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`) -> `c4e72b1ad47d93f5cacb83849a5662303d4550cd1bc0d78119088dea87677ea9`
- `editor/test/ui/picker-view.test.tsx`: absent (empty preimage `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`) -> `3fca85bdea1f8d62ac5eab2d31e8b3f5ff0c1acf604df0c752e05c460390f56d`
- `impl-plans/active/examples-50-picker-view.md`: `336cbf67afb4c89ec099b97b5b05b8826bd00f3d764ad6af18bf35e30d2f190d` -> recorded in `tmp/examples/EX-50/plan-sha256-attempt-2.txt` after this update.
