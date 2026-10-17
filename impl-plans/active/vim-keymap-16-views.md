# Vim Keymap: Hint Panel, Keymap Settings Panel and Status-Row Views Implementation Plan

**Status**: Ready
**Plan ID**: VK-16 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 4.5, 6.4, 8.1; design-docs/specs/design-ui-style.md
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`HintView`, `VimKeyInput`), VK-01 (`KeymapIssue` type)
- **Parallel (wave 1)**: VK-10, VK-11, VK-12, VK-13, VK-14, VK-15
- **Consumer**: VK-30 mounts these views; VK-40 adds them to the style probe
- Common Execution Rules: `impl-plans/active/vim-keymap-00-contract.md` (apply in full)

## Intent and Context

Three Solid views, styled solid, flat, square and spaced (never a native
widget look, `design-ui-style.md`; the user's standing preference):

- the bottom-left next-key hint panel (no focus, pointer-events none,
  aria-live summary);
- the keymap settings panel (preset segmented buttons, JSON textarea,
  Apply/Import/Export/Reset/Close, issue list);
- the status row gains a `keymap` button and `c`-mode key resolution in
  the ex input.

Imitate `editor/src/code/vim-status-view.tsx:mountVimStatus` (Solid
`render` into a parent, returns a dispose function) and the existing
`.vact-vim-*` rules in `editor/src/code/code.css`, using the `--vt-*`
tokens from `editor/src/app/theme.css`.

## Non-Goals

- No wiring into `vim-mount.ts`/`mount.ts` (VK-30); no controller logic;
  no storage calls. Views take accessors and callbacks only.
- No `<select>`; no border radius anywhere.

## File-Level Changes

### editor/src/code/vim-hints-view.tsx (new, budget 90 lines)

```ts
export const HINT_MAX_ROWS = 24;
export function mountVimHints(parent: HTMLElement, hints: Accessor<HintView | null>): () => void;
```
Renders `.vact-vim-hints` only when `hints()` is non-null: header
`.vact-vim-hints-prefix` with the prefix; grid `.vact-vim-hints-grid`
(`aria-hidden="true"`) of rows `.vact-vim-hints-key` + `.vact-vim-hints-desc`
(groups get `data-group="true"`); more than `HINT_MAX_ROWS` entries ->
the first `HINT_MAX_ROWS - 1` plus a row `+N more`; a visually hidden
`.vact-vim-hints-summary` with `aria-live="polite"` holding
`hints().summary`. No focusable elements.

### editor/src/code/vim-keymap-view.tsx (new, budget 170 lines)

```ts
export interface KeymapPanelProps {
  open: Accessor<boolean>; text: Accessor<string>; issues: Accessor<readonly KeymapIssue[]>;
  presets: readonly { id: string; label: string }[]; activePreset: Accessor<string | null>;
  status: Accessor<string | null>;
  onPreset(id: string): void; onInput(text: string): void; onApply(): void;
  onImport(): void; onExport(): void; onReset(): void; onClose(): void;
}
export function mountVimKeymapPanel(parent: HTMLElement, props: KeymapPanelProps): () => void;
```
When open: `<section class="vact-vim-keymap" aria-label="vim keymap">`
with a `role="group"` preset row of `<button class="vact-vim-keymap-preset" aria-pressed>`
per preset, a `<textarea class="vact-vim-keymap-text" spellcheck=false aria-label="vim keymap JSON">`,
buttons `.vact-vim-keymap-apply/-import/-export/-reset/-close` (text
labels `Apply`, `Import`, `Export`, `Reset`, `Close`), a status line
(`role="status"`), and `<ul class="vact-vim-keymap-issues" role="list">`
with one `<li data-severity>` per issue showing `path: message`.
Keydown inside the panel: `Escape` -> `onClose()`; every keydown calls
`stopPropagation()` so keys never reach the code bridge. On open, focus
the textarea.

### editor/src/code/vim-status-view.tsx (shared)

- `VimStatusHandlers` gains optional `openKeymap?(): void`; when present,
  render `<button type="button" class="vact-vim-keymap-button" aria-label="open vim keymap settings">keymap</button>`
  after the pending area.
- `mountVimStatus` gains an optional 4th parameter
  `resolveCmdline?: (input: VimKeyInput) => VimKey | null`. In the ex
  input `onKeyDown`, before the existing Enter/Escape handling and only
  when not composing: call it with the event; for a returned key apply
  `<Esc>` -> cancel, `<CR>` -> submit, `<Home>`/`<End>`/`<Left>`/`<Right>`
  -> move the caret, `<BS>`/`<Del>` -> delete one char before/after the
  caret, a printable -> insert at the caret; then `preventDefault` and
  `stopPropagation`. `null` -> existing behavior.
- Existing callers (no 4th argument, no `openKeymap`) behave exactly as
  before.

### editor/src/code/code.css (shared)

Add rules for `.vact-vim-hints` (absolute, `left: 8px`, `bottom` = status
row height + 8px, `max-height: 40%`, `overflow: hidden`, solid token
background, 1px token border, `border-radius: 0`, monospace token font,
8px padding and gaps, `pointer-events: none`, grid columns about 24ch),
`.vact-vim-hints-summary` (visually hidden clip pattern),
`.vact-vim-keymap` (solid panel above the status row, 8px padding and
gaps), its buttons using the existing icon/text button token styles
with `aria-pressed='true'` highlight, the textarea (monospace, flat,
square), the issue list, and `.vact-vim-keymap-button`. All scoped under
`.vact-code[data-vim='on']`.

## Invariants

- Existing `vim-mount.test.ts` and `vim-input.test.ts` pass unchanged.
- No `border-radius` other than `0`; no `appearance` overrides that bring
  back native looks.

## Test Cases

`editor/test/canvas/vim-hints-view.test.ts` (jsdom):
- `null` -> no `.vact-vim-hints`; a view with 3 entries -> 3 rows, summary text, `aria-live="polite"`, grid `aria-hidden="true"`
- 30 entries -> 24 rows with the last `+7 more`; group entries carry `data-group`
- no focusable descendants (`querySelectorAll('button,input,textarea,[tabindex]')` empty)

`editor/test/canvas/vim-keymap-view.test.ts`:
- closed -> nothing rendered; open -> textarea focused with `text()`
- preset button click -> `onPreset(id)`; active preset has `aria-pressed="true"`
- input event -> `onInput(value)`; each action button calls its handler
- issues render `path: message`; `Escape` keydown -> `onClose`; keydown does not bubble to a parent listener

`editor/test/canvas/vim-status-keymap.test.ts`:
- with `openKeymap` -> button present and calls it; without -> absent
- with `resolveCmdline` mapping Ctrl-g -> `<Esc>`: Ctrl-g in the ex input calls `cancel`; Ctrl-a -> `<Home>` moves the caret to 0; null result -> default behavior

## Verification (from editor/)

1. Scoped typecheck including the three views and the three tests -> exit 0
2. `./node_modules/.bin/vitest run test/canvas/vim-hints-view.test.ts test/canvas/vim-keymap-view.test.ts test/canvas/vim-status-keymap.test.ts test/canvas/vim-mount.test.ts test/canvas/vim-input.test.ts` -> exit 0, failureCount 0, at least 14 new cases
3. `rg -n 'border-radius' src/code/code.css` -> every match is `0` (record the output)

## Completion Criteria

- [ ] Pinned view APIs; budgets met
- [ ] At least 14 new passing cases; existing vim view tests pass
- [ ] CSS square and token-based
- [ ] Progress Log entry added

## Progress Log

(none yet)
