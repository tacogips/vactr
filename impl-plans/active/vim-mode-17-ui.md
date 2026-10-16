# Vim Mode: Setting, Clipboard Adapter, Status Row and Toggle Implementation Plan

**Status**: Ready
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

- [ ] The three new modules exist with the exact exports above, and the
      transport option is optional.
- [ ] At least 25 new tests pass, and `controls.test.ts` and
      `style-tokens.test.ts` pass unchanged.
- [ ] Each new file is under 200 lines.

## Verification

1. A scoped typecheck per common rule 5 (the 3 new modules,
   `src/ui/transport-view.tsx`, `src/code/transport.ts` and the 4 new
   tests):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-17/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim-settings.test.ts test/code/vim-clipboard.test.ts test/ui/vim-status.test.tsx test/ui/vim-toggle.test.ts test/ui/controls.test.ts test/ui/style-tokens.test.ts`
   must exit 0 with 0 failed. Record the counts.

## Progress Log

### Session: (not started)
