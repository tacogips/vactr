# EVM-21: Solid views, clipboard and decorations typed against emaki/vim

**Status**: Ready
**planId**: EVM-21. **Wave**: 2, in parallel with EVM-20. **dependsOn**:
EVM-10.
**Design reference**: `design-docs/specs/design-emaki-vim-migration.md`
VM-D8 and VB9.
**Rules**: `impl-plans/active/emaki-vim-00-rules.md`.

## Intent and context

The view and glue files import types from the local engine
(`./vim/types`, `./vim/keymap-schema`). After this plan they import the
equivalent public types from `'emaki/vim'`.

The one data-shape change: the public `VimSnapshot.prompt` is
`{ kind, text }`. The local engine had `{ kind, initial }`. All other
snapshot fields are the same, but `hints` and `hop` are required
(`HintView | null` and `HopView | null`).

## Non-goals

- No behavior change in any view, and no CSS change.
- `resolveCmdline` and `applyCmdlineKey` stay in `vim-status-view.tsx` with
  the same behavior (VM-D9). Only their types change.
- Do not touch `vim-mount.ts`. EVM-30 switches the snapshot source.

## File-level changes (type and import only, except the prompt field)

| File | Change |
|---|---|
| `editor/src/code/vim-status-view.tsx` | `VimSnapshot` from `'emaki/vim'`. `VimKey` becomes `string`. `VimKeyInput` becomes `ModalKeyInput` from `'emaki/vim'`. The `resolveCmdline` signature becomes `(input: ModalKeyInput) => string \| null`. `activePrompt().initial` becomes `activePrompt().text`. |
| `editor/src/code/vim-hints-view.tsx` | `HintView` from `'emaki/vim'` |
| `editor/src/code/vim-keymap-view.tsx` | `KeymapIssue` and `KeymapPreset` (and any other schema types) from `'emaki/vim'` |
| `editor/src/code/vim-clipboard.ts` | `ClipboardAdapter` becomes `NonNullable<VimEngineOptions['clipboard']>`, with `VimEngineOptions` from `'emaki/vim'`. Keep the exported function names `browserClipboard` and `createClipboardAdapter`. If a local alias name is needed by callers, keep `export type ClipboardAdapter = ...`. |
| `editor/src/code/decorations.ts` | `CursorShape` becomes `VimSnapshot['cursorShape']`, imported as a type from `'emaki/vim'` |
| `editor/test/ui/vim-status.test.tsx` | the `VimSnapshot` import comes from `'emaki/vim'`; fixtures use `prompt: { kind, text }` and add `hints: null, hop: null` |
| `editor/test/canvas/vim-status-keymap.test.ts` | the same fixture changes; the stub resolver tests stay as they are |
| `editor/test/canvas/vim-hints-view.test.ts` | `HintView` from `'emaki/vim'` |
| `editor/test/canvas/vim-keymap-view.test.ts` | if it imports `KEYMAP_PRESETS` or schema types from `src/code/vim/*`, switch them to `'emaki/vim'`; the assertions stay |
| `editor/test/code/vim-clipboard.test.ts` | imports only, if needed |

Pitfalls:

- emaki clipboard `write(text): void`. A promise-returning `write` is
  assignable to it, so do not change runtime behavior.
- Do not rename CSS classes or `data-*` attributes. `test:style` and the
  parity suite depend on them.
- Fixture snapshots must satisfy the emaki type exactly. Missing
  `hints`/`hop` is a tsc error.
- If a keymap-view test asserts `vactr.*` IDs from the `mise-darwin` preset
  text, the preset now comes from emaki and shows `emaki.*` (design section
  8). Update only that literal, and note it in the progress log.

## Expected transient breakage

Until EVM-30, `vim-mount.ts` passes the old engine's snapshot (`initial`)
into the status view. So the canvas mount tests and the parity suite may fail
at runtime, and tsc may report errors in `vim-mount.ts`. Those belong to
EVM-30. No owned file may show tsc errors.

## Acceptance criteria

- [ ] `grep -nE "code/vim/|'\./vim/" <each owned file>` returns nothing.
- [ ] `npx vitest run test/ui/vim-status.test.tsx test/canvas/vim-status-keymap.test.ts test/canvas/vim-hints-view.test.ts test/canvas/vim-keymap-view.test.ts test/code/vim-clipboard.test.ts`
      exits 0.
- [ ] Filtered tsc shows no error lines for any owned file.
- [ ] No assertion is removed or relaxed, as shown by the diff; record the
      `git diff --stat` in the log.
- [ ] The progress log is updated.

## Verification commands (run in `editor/`)

| Command | Evidence |
|---|---|
| the vitest command above | exit 0 |
| `npx tsc --noEmit -p . > tmp/emaki-vim/EVM-21/tsc.log 2>&1; grep -E "vim-status-view\|vim-hints-view\|vim-keymap-view\|vim-clipboard\|decorations\|vim-status\.test\|vim-status-keymap\|vim-hints-view\.test\|vim-keymap-view\.test" tmp/emaki-vim/EVM-21/tsc.log` | no matches |
| `git diff -U0 -- test/ui/vim-status.test.tsx test/canvas/vim-status-keymap.test.ts test/canvas/vim-hints-view.test.ts test/canvas/vim-keymap-view.test.ts test/code/vim-clipboard.test.ts \| grep -E "^-.*expect\("` | each removed `expect(` line has a re-added counterpart that differs only in import, type or fixture text |

## Progress Log
