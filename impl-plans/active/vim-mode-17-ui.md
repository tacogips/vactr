# Vim Mode: Setting, Clipboard Adapter, Status Row and Toggle Implementation Plan

**Status**: Completed
**Plan ID**: VIM-17 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 3.7, 4.8 (adapter), 5, 6; design-docs/specs/design-ui-style.md
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00 (`VimSnapshot`, `ClipboardAdapter` in `vim/types.ts`)
- **Parallel (wave 1)**: VIM-10..16, VIM-18
- **Consumer**: VIM-30, which wires these into `mount.ts`

## Intent and Context

This plan builds the DOM pieces of Vim mode. They stay outside the pure
core, and none of them is wired into `mount.ts` here (VIM-30 does that).

1. `editor/src/code/vim-settings.ts`: the per-viewer setting
   `vactr.vimMode`. Imitate the guarded storage in `editor/src/ui/shell.tsx`
   (`read`, `write`, `pageStorage`, `StorageLike`).
2. `editor/src/code/vim-clipboard.ts`: a `ClipboardAdapter` over
   `navigator.clipboard` that rejects when the API is unavailable.
3. `editor/src/code/vim-status-view.tsx`: the Solid status row, with the
   mode-label button and the ex/search input row.
4. The transport toggle button: `editor/src/ui/transport-view.tsx` and
   `editor/src/code/transport.ts` gain an **optional** `vim` option. When
   the option is absent (every existing test), no button renders, so
   existing transport tests are unaffected.
5. CSS: `editor/src/code/code.css` for the row,
   `editor/src/app/app.css` for the toggle's pressed state.

The UI style is solid, flat and square: `border-radius: 0`, no native
look, at least 8px between separate controls, and token-only values
(`--vt-*` from `editor/src/app/theme.css`). The common execution rules in
`vim-mode-00-contract.md` apply.

## Non-Goals

- Do not edit `mount.ts`, `input.ts` or any `vim/` core file.
- Do not edit `test/ui/controls.test.ts` or `test/style/ui-style.mjs`
  (VIM-30 extends the style probe).
- No new token in `theme.css`; use the existing tokens.

## File-Level Changes

### `editor/src/code/vim-settings.ts` (new, about 25 lines)

- `export const VIM_SETTING_KEY = 'vactr.vimMode'`
- `export function readVimEnabled(storage: StorageLike | null): boolean`
  returns true only for the exact string `'true'`. A throw or null gives
  false.
- `export function writeVimEnabled(storage: StorageLike | null, enabled: boolean): void`
  writes `'true'`/`'false'` and swallows throws.
- Import `type StorageLike` from `../ui/shell`.

### `editor/src/code/vim-clipboard.ts` (new, about 25 lines)

- `export function createClipboardAdapter(clipboard: Pick<Clipboard, 'readText' | 'writeText'> | null | undefined): ClipboardAdapter`.
  When `clipboard?.readText` or `writeText` is missing, the matching method
  returns `Promise.reject(new Error('clipboard unavailable'))`. A
  synchronous throw from the API is converted to a rejected promise.
- `export function browserClipboard(): ClipboardAdapter` calls
  `createClipboardAdapter(globalThis.navigator?.clipboard)` inside
  try/catch.
- No Tauri plugin (VQ8).

### `editor/src/code/vim-status-view.tsx` (new, at most 160 lines)

```ts
export interface VimStatusHandlers { submit(text: string): void; cancel(): void; tapMode(): void }
export function mountVimStatus(parent: HTMLElement, snapshot: Accessor<VimSnapshot>, handlers: VimStatusHandlers): () => void
```

It uses `render()` from `solid-js/web`, as `code/mount.ts` mounts
`CodeSurface`. The DOM is:

- The root is `div.vact-vim-status` with `data-mode={snapshot().mode}`.
- `button.vact-vim-mode` (`type="button"`) shows `snapshot().label`. Its
  `aria-label` is `vim mode <label>; activate to enter insert mode`, or
  `...; activate to leave insert mode` in insert or replace. Click calls
  `tapMode()`.
- `span.vact-vim-pending` holds `pending` (empty when none).
- `span.vact-vim-recording` holds `recording @<reg>`; render it only when
  `recording` is set.
- `span.vact-vim-search` holds `/<pattern>`; render it only when `search`
  is set.
- When `prompt` is null: `span.vact-vim-message` with
  `role={messageKind === 'error' ? 'alert' : 'status'}`, so errors are
  announced assertively (design 5), plus `data-kind={messageKind}` and
  `message`.
- When `prompt` is set: `label.vact-vim-prompt` holding
  `span.vact-vim-prompt-kind` (`:`, `/` or `?`) and
  `input.vact-vim-ex` (`type="text"`,
  `aria-label="vim command"` for `:`, otherwise `"vim search"`,
  `autocomplete="off"`, `spellcheck={false}`). The input starts with
  `prompt.initial`; it is focused on mount with the caret at the end.

Input behavior:

- `keydown` `Enter` with `isComposing === false` and `keyCode !== 229`
  calls `submit(value)`.
- `Escape`, or `Ctrl-[`, when not composing, calls `cancel()`.
- `blur` calls `cancel()` unless submit or cancel already ran for this
  prompt.
- `preventDefault` and `stopPropagation` for these handled keys only.

### CSS

**`editor/src/code/code.css`**: append a "Vim mode" block.

- `.vact-code[data-vim='on'] { display: grid; grid-template-rows: minmax(0, 1fr) auto; }`
- `.vact-code[data-vim='on'] > .vact-code-canvas { grid-row: 1; height: 100%; min-height: 0; }`
- `.vact-vim-status`:
  - `grid-row: 2; display: flex; align-items: center;`
  - `gap: var(--vt-space-2); padding: var(--vt-space-1) var(--vt-space-2);`
  - `background: var(--vt-surface); border-top: var(--vt-border-w) solid var(--vt-border);`
  - `font-family: var(--vt-font-mono); font-size: var(--vt-text-sm); color: var(--vt-text); border-radius: 0;`
- `.vact-vim-mode`: `min-width: var(--vt-control-h)`,
  `height: var(--vt-control-h-compact)` (36px under `pointer: coarse`,
  which the style test requires), `border-radius: 0`, and a
  `font-weight: 600` label. Its `data-mode` variants change only the
  fill or border token: insert uses the accent border, visual uses the
  selection fill.
- `.vact-vim-message[data-kind='error']`: color `var(--vt-danger)` if that
  token exists in `theme.css`, otherwise `var(--vt-warn)`. Check with grep
  first.
- `.vact-vim-prompt`: `display: flex; flex: 1 1 auto; gap: var(--vt-space-1); align-items: center;`
- `.vact-vim-ex`: `flex: 1 1 auto; min-width: 0; height: var(--vt-control-h-compact); border-radius: 0; font: inherit;`
  (the base `appearance: none` already applies).
- The style test (`test/style/ui-style.mjs`, extended by VIM-30) checks
  every element for these: radius 0; `appearance: none` on buttons and
  inputs; a button fill luminance of at most 0.5; at least 8px between
  controls on one row; and coarse targets of at least 36px. The row must
  satisfy all of them.

Every rule is token-only and `border-radius: 0`.

**`editor/src/app/app.css`**: append the
`.vact-vim-toggle[aria-pressed='true']` fill and border with the accent
tokens, in the same pattern as the existing pressed or primary rules
(grep `vact-primary`). Add `.vact-vim-toggle { font-family: var(--vt-font-mono); font-size: var(--vt-text-xs); }`.

### Transport toggle

- **`editor/src/ui/transport-view.tsx`.** `TransportViewProps` gains
  `vim?: { enabled: Accessor<boolean>; toggle: () => void }`. When it is
  present, render after the stop-all `IconButton` (line 202) a
  `button.vact-icon-button.vact-vim-toggle`. The button has
  `type="button"`, `aria-pressed={enabled() ? 'true' : 'false'}`,
  `aria-label="Vim mode: on|off"`, `title` equal to the label, the text
  content `vim`, and `onClick={toggle}`. It follows the existing
  `IconButton` structure (lines 61-78). Pass a `class` and children, and
  add an `ariaPressed` prop if needed, rather than a raw new markup style.
- **`editor/src/code/transport.ts`.** `TransportOptions` gains
  `vim?: { enabled: Accessor<boolean>; toggle: () => void }`, passed
  through to `TransportView` in the `createComponent` props (around
  line 160). Nothing else changes.

### Tests

- **`editor/test/code/vim-settings.test.ts`** (at least 6 cases): the
  default false; `'true'` -> true; `'1'` -> false; `getItem` throws ->
  false; `setItem` throws -> no throw; a write/read round trip.
- **`editor/test/code/vim-clipboard.test.ts`** (at least 5 cases):
  resolves through a stub; a missing API rejects; a synchronous throw
  becomes a rejection; `writeText` reject propagates; `browserClipboard()`
  under jsdom without `navigator.clipboard` rejects but does not throw.
- **`editor/test/ui/vim-status.test.tsx`** (at least 10 cases), using a
  Solid signal for the snapshot:
  - `data-mode` and the label update;
  - pending, recording and search show and hide;
  - the message has `role=status` for info and `role=alert` for errors,
    with `data-kind`;
  - prompt mount focuses the input with the initial `'<,'>` text;
  - Enter submits once;
  - Enter while `isComposing` does not submit;
  - Escape cancels;
  - blur after submit does not cancel twice;
  - the mode button click calls `tapMode`;
  - dispose removes the DOM.
- **`editor/test/ui/vim-toggle.test.ts`** (at least 4 cases), building
  `TransportBar` as `test/ui/controls.test.ts` does (copy its setup
  pattern, do not edit it):
  - no `vim` option -> no `.vact-vim-toggle`;
  - with the option -> a button with `aria-pressed='false'`;
  - a click calls `toggle`;
  - an `enabled` signal flip -> `aria-pressed='true'` and the label
    `Vim mode: on`.

## Invariants

- Existing transport tests (`test/ui/controls.test.ts`) and the
  `test/ui/style-tokens.test.ts` static token check pass unchanged.
- No `border-radius` other than 0 and no hard-coded color in the new CSS
  (the static token test enforces this).

## Acceptance Criteria

- [x] The three new modules exist with the exact exports above, and the
      transport option is optional.
- [x] At least 25 new tests pass, and `controls.test.ts` and
      `style-tokens.test.ts` pass unchanged.
- [x] Each new file is under 200 lines.

## Verification

1. A scoped typecheck per common rule 5 (the 3 new modules,
   `src/ui/transport-view.tsx`, `src/code/transport.ts` and the 4 new
   tests):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-17/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts test/ui/controls.test.ts test/ui/style-tokens.test.ts`
   must exit 0 with 0 failed. Record the counts.

## Progress Log

### Session: 2026-10-16 VIM-17 implementation

**Tasks Completed**: Added guarded Vim setting persistence, browser
clipboard adapter, reactive Solid status and prompt row, optional
transport toggle, token-only square UI styles, and focused tests.

**Write-path hashes before edits** (existing files were clean at the
checkpoint; baseline hashes are from `09a3406bcd19afd4b9039eda787c4d23cf97ffdb`):
- `vim-settings.ts`: missing; final `3e3a28a42bba471c818612049b09976e275bacaf`
- `vim-clipboard.ts`: missing; final `ede05cc95ec3d07bf10fb8e0a0755f1e15eaa775`
- `vim-status-view.tsx`: missing; final `903820c7716ab1aa83af65dc1e34df9467227a62`
- `transport-view.tsx`: before `b22e08bcf90e79a22cd3b950b51af3db6954496f`; final `de3d5493065773bbeca1099feb2583e6b951a335`
- `transport.ts`: before `930396aa34a16ff0cd8a83d2811e98c3c2a82c38`; final `523295c934d5cabf7f1b420b06bdca466cf06d01`
- `code.css`: before `072ed338019fd53691c6d5ebc3c7e875a9f74ba0`; final `9279aebdc3b0da6fc3048a28a162ad521cde620a`
- `app.css`: before `b9f175a88968e7ba0982f04840d8a420cb1f5793`; final `569c56d508728d573be8942126b69e21ce1997d8`
- `vim-settings.test.ts`: missing; final `af1215f79b4264256db78744a58598db59952a6f`
- `vim-clipboard.test.ts`: missing; final `41aa0da1b532510d4b4758722075091f894c1aef`
- `vim-status.test.tsx`: missing; final `1a76f9c4538264997104c591dc884beee9a39637`
- `vim-toggle.test.ts`: missing; final `e592d18009a6f0401eec6e7606913e3b61a266c3`
- This plan before update: `9956c9420536c7cc1efa99cc4a9dbf80e333dbab`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-17/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/typecheck-final.log", notes: "Scoped typecheck passed with no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts test/ui/controls.test.ts test/ui/style-tokens.test.ts", exitStatus: 0, testsRun: 43, testsPassed: 43, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/focused-vitest-attempt-02.log", notes: "6 files passed, including unchanged controls and style token tests; all 25 VIM-17 tests passed."}`
- `{command: "cd editor && wc -l src/code/vim-settings.ts src/code/vim-clipboard.ts src/code/vim-status-view.tsx test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts | awk 'BEGIN { ok=1 } NF == 2 && $2 != \"total\" { if ($1 >= 200) ok=0; print } END { if (!ok) exit 1 }'", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/line-count-final-3.log", notes: "All seven new source/test files are below 200 lines."}`

**Earlier Attempts**: `typecheck-attempt-01.log` caught an overly broad
ARIA pressed type, corrected to the literal `true`/`false` union.
`focused-vitest-attempt-01.log` caught prompt focus occurring before DOM
insertion; focus now runs reactively after insertion and all 43 tests pass.
The initial line-count shell checks counted `wc`'s total row; the final
filtered check in `line-count-final-3.log` passed.

**Drift**: Preserved concurrent edits in VIM-10..16 files, including
`input.ts`, `renderer.ts`, `surface.ts`, cursor-shape code and their tests.
No files outside this plan's write paths were edited here.

**Blockers**: None. Mount wiring is owned by VIM-30; formal review and
workflow finalization remain downstream.

### Session: 2026-10-17 Step 6 rerun

**Tasks Completed**: Rechecked the VIM-17 source on the shared `wf/vim-mode`
tree and reran the scoped typecheck, six-file focused suite, and new-file
line-count gate. No VIM-17 source correction was needed; only this progress
record and plan-local verification evidence were added in this rerun.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-17/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/typecheck-rerun.log", notes: "Scoped typecheck passed with no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts test/ui/controls.test.ts test/ui/style-tokens.test.ts", exitStatus: 0, testsRun: 43, testsPassed: 43, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/focused-vitest-rerun.log", notes: "Six files passed, including unchanged controls and style-token tests."}`
- `{command: "cd editor && wc -l src/code/vim-settings.ts src/code/vim-clipboard.ts src/code/vim-status-view.tsx test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts | awk 'BEGIN { ok=1 } NF == 2 && $2 != \"total\" { if ($1 >= 200) ok=0; print } END { if (!ok) exit 1 }'", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/line-count-rerun.log", notes: "All seven new source/test files remain below 200 lines."}`

**Shared Tree**: Other plan logs changed concurrently in the shared worktree;
they were preserved. No VIM-17 source or test file is modified in this rerun.

### Session: 2026-10-17 implementation-node rerun

**Tasks Completed**: Confirmed the existing VIM-17 implementation against the
assigned plan and reran its scoped typecheck, required six-file Vitest suite,
and new-file line-count gate on the current shared tree. No source or test
correction was required.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-17/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-02/typecheck.log", notes: "Scoped typecheck passed with no diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts test/ui/controls.test.ts test/ui/style-tokens.test.ts", exitStatus: 0, testsRun: 43, testsPassed: 43, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-02/focused-vitest.log", notes: "Six files passed; all 25 assigned Vim tests and unchanged controls/style-token suites passed."}`
- `{command: "cd editor && wc -l src/code/vim-settings.ts src/code/vim-clipboard.ts src/code/vim-status-view.tsx test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts | awk 'BEGIN { ok=1 } NF == 2 && $2 != \"total\" { if ($1 >= 200) ok=0; print } END { if (!ok) exit 1 }'", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-02/line-count.log", notes: "All seven new source/test files are below 200 lines."}`

**Source Review**: Confirmed the transport `vim` option remains optional and
absent options do not render its control; the status row and toggle styling
use existing `--vt-*` tokens and radius 0. VIM-15's concurrently modified
`editor/src/code/input.ts` and `editor/test/canvas/input-vim-hook.test.ts`
were preserved and remain outside VIM-17 write paths.

### Session: 2026-10-17 Step 6 implementation rerun

**Tasks Completed**: Revalidated the assigned VIM-17 contract on the shared
`wf/vim-mode` tree. The VIM-17 implementation and tests required no source
correction. Recorded current-source hashes and reran scoped typecheck, the
required six-file Vitest suite, and the new-file line-count gate.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-17/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-03/typecheck.log", notes: "Scoped typecheck completed without diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts test/ui/controls.test.ts test/ui/style-tokens.test.ts", exitStatus: 0, testsRun: 43, testsPassed: 43, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-03/focused-vitest.log", notes: "Six files passed, including all 25 VIM-17 cases and unchanged controls/style-token suites."}`
- `{command: "wc -l src/code/vim-settings.ts src/code/vim-clipboard.ts src/code/vim-status-view.tsx test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts | awk 'BEGIN { ok=1 } NF == 2 && $2 != \"total\" { if ($1 >= 200) ok=0; print } END { if (!ok) exit 1 }'", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-03/line-count.log", notes: "Line counts are 19, 32, 80, 30, 29, 96 and 52; all are below 200."}`
- `shasum -a 256` over all VIM-17 tracked paths recorded in the implementation-node output; source and test hashes match the pre-node VIM-17 snapshot at `tmp/riela-fanout/AEF7BFF2-312E-4395-BA2C-4F6FF3C45B90/5C69B9C2-8F5D-4DC3-83A6-CFB6AA3DB5AD.json`.

**Prior Setup Errors**: Two initial command wrappers used an incorrect
working-directory-relative log path and failed before `tsc` launched. They
produced no verification logs; the corrected scoped typecheck above passed.

**Scope Handoff**: The Chromium normal-mode IME finding at
`editor/src/code/input.ts:44` remains assigned to VIM-15 and VIM-40. It is
outside VIM-17 write paths and does not change VIM-17 completion. VIM-15's
concurrent edits and all sibling plan changes were preserved. Formal review
and workflow finalization remain downstream.

### Session: 2026-10-17 Step 6 implementation-node verification

**Tasks Completed**: Rechecked VIM-17 on the current `wf/vim-mode` tree. No
source or test changes were needed. The scoped typecheck, required six-file
Vitest suite, and new-file line-count gate passed; controls and style-token
tests remained unchanged.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-17/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-05/typecheck.log", notes: "Scoped typecheck completed without diagnostics."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts test/ui/controls.test.ts test/ui/style-tokens.test.ts", exitStatus: 0, testsRun: 43, testsPassed: 43, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-05/focused-vitest.log", notes: "Six files passed; all 25 assigned Vim tests and unchanged controls/style-token suites passed."}`
- `{command: "cd editor && wc -l src/code/vim-settings.ts src/code/vim-clipboard.ts src/code/vim-status-view.tsx test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts | awk 'BEGIN { ok=1 } NF == 2 && $2 != \"total\" { if ($1 >= 200) ok=0; print } END { if (!ok) exit 1 }'", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-mode/VIM-17/attempt-05/line-count.log", notes: "All seven new source/test files remain below 200 lines."}`

**Evidence Intent**: `tmp/vim-mode/VIM-17/attempt-05/plan-update-intent.json`
records the intended progress-log hunk before this update.
