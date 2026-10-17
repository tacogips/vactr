# Vim Keymap: Hint Panel, Keymap Settings Panel and Status-Row Views Implementation Plan

**Status**: Completed
**Plan ID**: VK-16 (wave 1)
**Design Reference**: design-docs/specs/design-vim-keymap-hints-hop.md sections 4.5, 6.4, 8.1; design-docs/specs/design-ui-style.md
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Related Plans

- **Depends on**: VK-00 (`HintView`, `VimKeyInput`), VK-01 (`KeymapIssue` type)
- **Parallel (wave 1)**: VK-10, VK-11, VK-12, VK-13, VK-14, VK-15
- **Consumer**: VK-30 mounts these views; VK-40 adds them to the style probe
- Common Execution Rules: `impl-plans/completed/vim-keymap-00-contract.md` (apply in full)

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

- [x] Pinned view APIs; budgets met (35/90 and 67/170 lines; status view 121 lines)
- [x] At least 14 new passing cases; existing vim view tests pass (18 new; 58/58 focused suite)
- [x] CSS square and token-based; every `border-radius` match is `0`
- [x] Progress Log entry added

## Progress Log

### Session: 2026-10-17 VK-16 implementation

**Tasks Completed**: Added the hint panel and keymap settings views, optional status-row keymap button and command-line resolver, square token-based CSS, and 18 view/status test cases. Existing Vim mount/input tests passed unchanged.

**Source Hashes**:
- `editor/src/code/vim-hints-view.tsx`: `f240faecd5fb12bd9a36f9fdcc25cf2aef020935a05147526c6d5bcb3d2dad28` (35 lines)
- `editor/src/code/vim-keymap-view.tsx`: `0587174cf6c656cb032ba01b2811b5961136a42f31ccc74da2d71770fe2b1bb6` (67 lines)
- `editor/src/code/vim-status-view.tsx`: `a7423643cbc5c29fa21323851bb28152450d74b4276966fcb7fc23dc04fc1366` (121 lines)
- `editor/test/canvas/vim-hints-view.test.ts`: `bfb381deefd8958867306a229613a2c48751151492e8a92513fb54f679295b48`
- `editor/test/canvas/vim-keymap-view.test.ts`: `e972594cee016f6550af7f7839cb57cf0cbe552b396b5e45373b7c5ab3b3d1fa`
- `editor/test/canvas/vim-status-keymap.test.ts`: `b3dee54d41e89446750e59dab9117bde1b85fc09470cae90c31ebc3dc6f16472`
- `editor/src/code/code.css`: `cb22a1eeb26854e00adbc95e59037ca18743da1178a1f71c097d3502d2c075ae`

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-16/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-16/attempt-3/typecheck.log", notes: "Scoped VK-16 TSX and tests typecheck."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/vim-hints-view.test.ts test/canvas/vim-keymap-view.test.ts test/canvas/vim-status-keymap.test.ts test/canvas/vim-mount.test.ts test/canvas/vim-input.test.ts", exitStatus: 0, testsRun: 58, testsPassed: 58, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-16/attempt-3/vitest.log", notes: "5 files passed; the three new files contribute 18 cases; existing Vim mount/input tests pass."}`
- `{command: "cd editor && rg -n 'border-radius' src/code/code.css", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-16/attempt-3/css-radius.log", notes: "Every reported value is 0."}`
- Scope note: `cd editor && npm run test:style` exited 1 in attempt-3 before analysis because `editor/dist/index.html` was absent. This is outside VK-16 gates: the committed manifest assigns `npm run test:style` and the `editor/dist` artifact root to VK-40, whose style probe covers both panels. This attempt is not a VK-16 verification gate or blocker.

**Evidence**: Per-edit intent and full logs are under `tmp/vim-keymap/VK-16/attempt-1/` through `attempt-3/`. Initial focused failures were corrected; the final-source focused run passes 58/58. The prior DR-K1 mapper argument-routing finding remains owned by VK-10/VK-13/VK-20 and is outside VK-16 paths.

**Downstream**: VK-30 owns mounting these views. Independent review and serial integration are later workflow steps. VK-40 owns the shared style probe and includes both panels.


### Session: 2026-10-17 VK-16 review repair

**Tasks Completed**: Resolved `IR-VK16-false-style-gate-blocker`. VK-16's three manifest-owned gates pass on the current sources. The `npm run test:style` check belongs to VK-40, as recorded above; it is not a VK-16 blocker. The initial attempt-4 wrapper log is preserved at `tmp/vim-keymap/VK-16/attempt-4/` and was incomplete because zsh rejected assignment to its read-only `status` variable; all gates were rerun with complete logs under `attempt-4/retry-1/`.

**Verification**:
- `{command: "cd editor && ./node_modules/.bin/tsc --noEmit -p ../tmp/vim-keymap/VK-16/tsconfig.json", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-16/attempt-4/retry-1/typecheck.log", notes: "Scoped VK-16 typecheck on unchanged plan source identities."}`
- `{command: "cd editor && ./node_modules/.bin/vitest run test/canvas/vim-hints-view.test.ts test/canvas/vim-keymap-view.test.ts test/canvas/vim-status-keymap.test.ts test/canvas/vim-mount.test.ts test/canvas/vim-input.test.ts", exitStatus: 0, testsRun: 58, testsPassed: 58, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-16/attempt-4/retry-1/vitest.log", notes: "Five files passed; 18 new view/status cases and existing Vim mount/input tests passed."}`
- `{command: "cd editor && rg -n 'border-radius' src/code/code.css", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/vim-keymap/VK-16/attempt-4/retry-1/css-radius.log", notes: "Every reported border-radius value is 0."}`

**Source Identities**: The seven VK-16 source/test Git blobs remain `53e1c8a8`, `71e911f2`, `53b8e755`, `ccfe3183`, `79ca810b`, `2b8a7d51`, and `dd95b81b`; no source files were changed by this repair. The dispatch manifest is unchanged.

**Downstream**: Independent test-integrity and adversarial review, VK-30 mounting, and serial integration remain downstream. VK-16 implementation has no remaining owned blocker.
