# UI Style: Tokens, Base Rules and Shell Implementation Plan

**Status**: Ready
**Plan ID**: UI-STYLE-SHELL (wave 1)
**Design Reference**: design-docs/specs/design-ui-style.md, sections 3, 4.1-4.3, 4.5 (transport, side bar, section headers, status bar) and 5
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

## Related Plans

- **Parallel (wave 1)**:
  - `ui-style-bind-params.md`
  - `ui-style-code.md`
  - `ui-style-panels.md`
- **Next (wave 2)**: `ui-style-verify.md`. It depends on this plan and the
  other wave-1 plans.

## Intent and Context

The user wants the whole editor chrome to be solid, flat and square. No
rounded corners, no native white widgets, and intentional spacing. This plan
lays the foundation:

- The token file.
- Its load order.
- The global base element rules that remove the native look from every
  control.
- The variant classes.
- The shell surfaces:
  - transport toolbar
  - side bar
  - section headers
  - status bar

The component sheets (other plans) use the same token names. Those names are
fixed in design sections 3.2 and 3.3 and repeated below. Do not rename them.

Current code:

- `editor/index.html` links only `./src/app/app.css`.
- `editor/src/app/app.css` holds:
  - shell layout
  - `.vact-icon-button` (radius 6px at line 85, `opacity: 0.6` when disabled)
  - `.vact-light` (radius 50% at line 204)
  - the transport, section toggle and status rules
- `Apply song` is created in `editor/src/app/song.ts:mount`
  (`doc.createElement('button')`, no class).
- `Enable MIDI` is created in `editor/src/midi/mount.ts:mount`
  (`className = 'midi-enable'`).

## Non-Goals

- Do not edit any component sheet: `bind.css`, `code.css`, `midi.css`,
  `params.css`, `pkg.css` or `visual.css`. Other plans own them.
- Do not edit any TS/TSX file other than the two one-line class edits below.
- Do not add tests. `ui-style-verify.md` owns them.
- Do not touch the protected files:
  - `editor/src/code/{pointer,perf-hook,renderer,mount,layout,input,keyboard,accessibility,sync,syntax,syntax-core,history,frame,highlight,transport,eval}.ts`
  - `editor/src/app/clock.ts`
  - `editor/src/bind/{mount,write}.ts`
  - `editor/src/visual/{frame,scopes,video,render-host}.ts`
  - `editor/test/e2e/*`
- No theme switching, no JS reading of tokens, no new dependencies.

## File-Level Changes

### 1. `editor/src/app/theme.css` (new)

- Contents allowed: only `:root { ... }` custom-property declarations, plus
  one `@media (pointer: coarse) { :root { ... } }` block. Nothing else: no
  element, class or `*` rules, and no `@import`.
- Required token names. The values come from design section 3.2 and 3.3
  tables; copy them exactly.
  - Color:
    - Backgrounds and surfaces: `--vt-bg`, `--vt-bg-sunken`, `--vt-surface`,
      `--vt-raised`, `--vt-raised-hover`, `--vt-raised-active`
    - Borders: `--vt-border`, `--vt-border-strong`
    - Text: `--vt-text`, `--vt-text-muted`, `--vt-text-disabled`
    - Accent: `--vt-accent`, `--vt-accent-hover`, `--vt-accent-active`,
      `--vt-on-accent`, `--vt-selection`
    - Status: `--vt-success`, `--vt-warn`, `--vt-danger`, `--vt-danger-bg`,
      `--vt-danger-text`
    - Data and highlights: `--vt-data-1`, `--vt-data-2`, `--vt-flash`,
      `--vt-flash-error`, `--vt-playing-bg`, `--vt-playing-outline`
    - Syntax: `--vt-syn-comment`, `--vt-syn-directive`, `--vt-syn-keyword`,
      `--vt-syn-number`, `--vt-syn-string`, `--vt-syn-head`,
      `--vt-syn-bracket`
    - Icons: `--vt-icon-chevron`, `--vt-icon-check`
  - Type: `--vt-font-sans`, `--vt-font-mono`, `--vt-text-xs`, `--vt-text-sm`,
    `--vt-text-md`, `--vt-text-lg`.
  - Spacing: `--vt-space-1` .. `--vt-space-5`.
  - Size: `--vt-control-h` (28px), `--vt-control-h-compact` (24px),
    `--vt-header-h` (28px), `--vt-border-w`, `--vt-focus-w`,
    `--vt-focus-offset`, `--vt-radius` (`0`).
- Coarse block values: `--vt-control-h: 40px`,
  `--vt-control-h-compact: 36px`, `--vt-header-h: 36px`.
- Icon tokens:
  - `--vt-icon-chevron` is `url("data:image/svg+xml,...")`: a 10x6 down
    chevron path with `stroke="%239aa3ad"` and `stroke-linejoin="miter"`,
    `stroke-linecap="square"`, no fill.
  - `--vt-icon-check` is the same idea with `stroke="%230b0d10"`.
  - The `#` inside a data URI MUST be percent-encoded as `%23`. A raw `#`
    breaks the URI in WebKit.
- Add a short header comment that cites design-ui-style.md section 3.

### 2. `editor/index.html`

Add `<link rel="stylesheet" href="./src/app/theme.css" />` on the line
immediately before the existing `./src/app/app.css` link. Change nothing else.

### 3. `editor/src/app/app.css`

**Specificity rule (main pitfall).** Write every base element rule inside
`:where(...)`, so its specificity is zero. Example:
`:where(button):where(:hover:not(:disabled))`. Then any class rule in any
sheet wins: `.params-tab`, `.vact-section-toggle`, `.vact-primary`, and so
on. Without `:where`, `button:hover:not(:disabled)` (0,2,1) beats
`.vact-section-toggle:hover` (0,2,0) and breaks component styling. The only
exception is `[hidden] { display: none !important; }`.

**Base rules.** Follow design section 4.1 exactly.

- Element baseline: `html, body` use `--vt-bg`, `--vt-text`, and the font
  `var(--vt-text-md)/1.4 var(--vt-font-sans)`.
- `[hidden]` gets `display: none !important`.
- `button, input, select, textarea` get `border-radius: var(--vt-radius)`,
  `appearance: none` and `-webkit-appearance: none`.
- `button` and the file-selector button:
  - Selectors: `::file-selector-button` and `::-webkit-file-upload-button`.
    Put each pseudo in its own rule; an unknown pseudo invalidates the whole
    selector list in some engines.
  - Box: 1px `--vt-border-strong` border, `--vt-raised` fill, `--vt-text`,
    `font-family: var(--vt-font-sans)`, `font-size: var(--vt-text-sm)`.
  - Layout: `min-height: var(--vt-control-h)`, `padding: 0 var(--vt-space-3)`,
    `display: inline-flex`, `align-items: center`, `justify-content: center`,
    `gap: var(--vt-space-1)`, `cursor: pointer`.
  - States:
    - Hover: `--vt-raised-hover`.
    - Active: `--vt-raised-active`.
    - `[aria-pressed="true"]`: `--vt-raised-active` fill and `--vt-accent`
      border.
    - `:disabled`: `--vt-surface` fill, `--vt-border` border,
      `--vt-text-disabled` text, `cursor: default`.
- `:focus-visible` (button, select, input, `[tabindex]`): outline
  `var(--vt-focus-w) solid var(--vt-accent)`, offset `var(--vt-focus-offset)`.
- `select`:
  - Same box as a button, with `padding-right: 28px` expressed with tokens.
  - Chevron: `background: var(--vt-icon-chevron) no-repeat right
    var(--vt-space-2) center / 10px`, with the fill color set via
    `background-color`.
  - Because the shorthand resets the color, set `background-color` after it,
    or use the longhands.
- `input[type=text|url|search|number]`:
  - Box: `--vt-bg-sunken`, 1px `--vt-border-strong`,
    `min-height: var(--vt-control-h)`, `padding: 0 var(--vt-space-2)`,
    `--vt-text`, `font: inherit`.
  - The placeholder uses `--vt-text-muted`.
  - Disabled uses the disabled tokens.
- `input[type=file]`: no border, transparent background, `--vt-text-muted`,
  `--vt-text-sm`, `min-height: var(--vt-control-h)`. Its selector button
  gets `margin-right: var(--vt-space-2)`.
- `input[type=checkbox]`:
  - Box: 16px square, 1px `--vt-border-strong` border, `--vt-bg-sunken` fill,
    `margin: 0`.
  - `:checked`: `--vt-accent` fill, `--vt-accent` border,
    `--vt-icon-check` centered and contained.
- `input[type=range]`:
  - Box: transparent, `height: var(--vt-control-h-compact)`, `margin: 0`.
  - Track (`::-webkit-slider-runnable-track` and `::-moz-range-track`):
    4px tall, `--vt-border-strong`.
  - Thumb (`::-webkit-slider-thumb`, with `appearance: none` and a negative
    `margin-top` to centre it on the 4px track, plus `::-moz-range-thumb`):
    10px wide, 16px tall, `--vt-text`, `border: 0`, radius 0.
  - Under `@media (pointer: coarse)`, the thumb is 24px tall.
  - When disabled, the thumb uses `--vt-text-disabled`.
  - Write each vendor pseudo in its own rule.

**Variants (design section 4.2).**

- `.vact-primary`:
  - Fill and border `--vt-accent`, text `--vt-on-accent`, weight 600.
  - Hover `--vt-accent-hover`, active `--vt-accent-active`.
  - Add an explicit `.vact-primary:disabled` rule that restores the disabled
    tokens; the class otherwise beats the zero-specificity base disabled rule.
- Danger treatment, on the existing classes (no markup change):
  - `.vact-stop-all, .vact-mute`: icon `color: var(--vt-danger)`.
  - `.vact-hush`: icon `color: var(--vt-warn)`.
  - `.vact-hush`, `.vact-stop-all` and `.vact-mute` on
    `:hover:not(:disabled)`: `--vt-danger-bg` fill and `--vt-danger` border.

**Icon buttons (section 4.3).**

- `.vact-icon-button`: `width` and `height` `var(--vt-control-h)`,
  `padding: 0`, radius via base.
- Remove `border-radius: 6px`.
- Delete the existing `.vact-icon-button:hover:not(:disabled)` rule
  (`app.css:91-93`). The base hover rule covers it. Keeping it would make the
  danger hover depend on source order at equal specificity.
- Replace `opacity: 0.6` on disabled with the disabled tokens.
- `.vact-slot .vact-icon-button`: width and height
  `var(--vt-control-h-compact)`.
- `.vact-light`: an 8px square, with no radius declaration (or
  `var(--vt-radius)`). Fill `--vt-border`; lit fill `--vt-success`.

**Transport and toolbar groups (section 4.5).**

- `.pane-transport`:
  - `--vt-surface` fill, `flex-wrap: wrap`, `gap: var(--vt-space-2)`.
  - Padding `var(--vt-space-1) var(--vt-space-2)`; bottom border
    `var(--vt-border-w) solid var(--vt-border)`.
- `.vact-transport`: `gap: var(--vt-space-2)` (was 10px).
- Group separators:
  - `.vact-eval-status`, `.vact-level` and `[data-song-controls]` get a
    `border-left` of `var(--vt-border-w) solid var(--vt-border-strong)` and
    `padding-left: var(--vt-space-2)`.
  - `.vact-clock` gets `border-right` and `padding-right` in the same way.
  - Never put separators on buttons.
- `[data-song-controls]`:
  - `display: inline-flex`, `align-items: center`,
    `gap: var(--vt-space-2)`, `flex-wrap: wrap`.
  - `[role=status]` uses `--vt-text-muted` and `--vt-text-sm`.
  - `[role=alert]` uses `--vt-danger`.
  - `> div` is `inline-flex` with gap `var(--vt-space-2)`.
- `.vact-slots` gap is `var(--vt-space-2)`; `.vact-slot` gap is
  `var(--vt-space-1)` (tight group). `.vact-eval-status` inner gap is
  `var(--vt-space-1)`.
- The state colors keep their meaning but use tokens:
  - `--vt-warn`: off, suspended, diagnostics
  - `--vt-success`: running, ok, midi-locked, run
  - `--vt-danger`: failed, not-delivered, midi-lost
  - `--vt-text-muted`: starting, pending
  - Beat dot: `--vt-border-strong` off, `--vt-accent` on

**Side bar, section headers and status bar.**

- `.pane-side`: border-left uses tokens. Keep `min-width: 280px`, which is
  fixed geometry.
- `.pane-side-bar`: `min-height: var(--vt-header-h)`, padding
  `var(--vt-space-1) var(--vt-space-2)`.
- `.vact-section-toggle`:
  - Size: `min-height: var(--vt-header-h)`, `padding: 0 var(--vt-space-2)`.
  - Box: `border: 0`, `--vt-surface` fill, `justify-content: flex-start`.
  - Text: `--vt-text-muted`, `--vt-text-xs`, uppercase,
    `letter-spacing: 0.06em`, weight 600.
  - Hover: text `--vt-text`, fill stays `--vt-surface`.
- `.vact-section-chevron` keeps `transition: transform 120ms ease`. Add
  `@media (prefers-reduced-motion: reduce) { .vact-section-chevron { transition: none; } }`.
- `.pane-status`:
  - `--vt-surface` fill, top border, padding
    `var(--vt-space-1) var(--vt-space-2)`.
  - `--vt-text-muted` and `--vt-text-sm`.
  - `[data-state='failed']` uses `--vt-danger`.
- `.pane-code` gets `background: var(--vt-bg)`. The canvas clears to
  transparent and shows this color.

**Global constraints for `app.css` after the edit.**

- Zero hex, `rgb`, `rgba` or `hsl` literals.
- No generic font keywords (`system-ui`, `sans-serif`, `monospace`,
  `ui-monospace`, `ui-sans-serif`).
- Every `border-radius` value is `0` or `var(--vt-radius)`.
- No `order` property and no `*-reverse` directions.
- Fixed-geometry px stays: `280px`, the `38px` rail, the `8px` light, the
  `16px` checkbox, and the `16px`/`24px` thumb.

### 4. `editor/src/app/song.ts`

In `mount`, add `apply.className = 'song-apply vact-primary';` after
`apply.type = 'button'`. Change nothing else. `test/app/song.test.ts:165`
selects `[data-song-controls] > button`, and DOM order is unchanged.

### 5. `editor/src/midi/mount.ts`

Change `enable.className = 'midi-enable'` to
`enable.className = 'midi-enable vact-primary'`.
`test/support/midi.ts:142` queries `.midi-enable`, which still matches.

## Invariants

- No class is removed or renamed, and no DOM order changes. Accessibility
  labels are unchanged.
- `theme.css` contains only `:root` blocks (one optionally in
  `@media (pointer: coarse)`).
- Base rules have zero specificity, except `[hidden]`.
- The token names exactly match the list above. Other plans depend on them.
- Other wave-1 plans (`UI-STYLE-BIND-PARAMS`, `UI-STYLE-CODE`,
  `UI-STYLE-PANELS`) edit sibling files in the same worktree at the same
  time. Never restore, checkout or reformat a file outside this plan's
  writePaths, even if `git status` shows it modified.

## Setup (not gating)

If `target/wasm32-unknown-unknown/debug/vactr.wasm` is missing, run this from
the repository root first:

`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`

`test/support/wasm.ts` requires it. `target/` is an artifact root. Cargo's
build lock serializes concurrent runs from parallel plans.

## Verification (gating, run from `editor/`)

| Command | Must show |
|---|---|
| `npm run check` | Exit 0 |
| `./node_modules/.bin/vitest run` | Exit 0. The test count is not lower than the baseline recorded before editing, and no existing test file changes. |
| `npm run build` | Exit 0. `editor/dist/index.html` contains two stylesheet links, and the theme sheet comes first. Check with `grep -n 'stylesheet' dist/index.html`. |

Mechanical checks:

| Command | Must show |
|---|---|
| `grep -nE '#[0-9a-fA-F]{3,8}\b\|rgba?\(\|hsl' src/app/app.css` | No output |
| `grep -nE 'border-radius:\s*[^0v;]' src/app/app.css` | No output |
| `grep -nE 'system-ui\|sans-serif\|monospace' src/app/app.css` | No output |
| Ownership check (see below) | This plan wrote only its owned files |

**Ownership check (replaces any whole-worktree diff expectation).**

Owned files are the five source write paths plus this plan file:

- `editor/src/app/theme.css`
- `editor/index.html`
- `editor/src/app/app.css`
- `editor/src/app/song.ts`
- `editor/src/midi/mount.ts`
- `impl-plans/active/ui-style-shell.md`

The watched non-owned set is `editor/src/{bind/bind,code/code,midi/midi,params/params,pkg/pkg,visual/visual}.css`.

1. Before the first edit, record `git hash-object <path>` for every owned
   and watched file in the progress log. Use `absent` for `theme.css`.
2. After the final edit, record them again.
3. Pass when both of these hold:
   - All five owned source hashes changed. `theme.css` goes from `absent` to
     a hash.
   - Every watched non-owned hash that differs is owned by a sibling wave-1
     plan in `impl-plans/active/ui-style-dispatch.json`. Cite that planId.
4. Fail if a non-owned path changed that no sibling owns.

For human-readable evidence, use
`git diff --stat -- editor/index.html editor/src/app/app.css editor/src/app/song.ts editor/src/midi/mount.ts`
(`theme.css` is untracked and appears in `git status`). Do not expect a
whole-worktree `git diff --stat` to list only this plan's files.

## Test Cases

These are asserted by `ui-style-verify.md`; listed here so the implementation
targets them.

- Built app at 1440x900 -> every element computes `border-radius` 0px, and
  `Apply song` has the accent fill.
- `Apply song` disabled (no code) -> the disabled tokens, not accent.
- 1180x820 with a coarse pointer -> every visible button is at least 36px
  tall.
- The song status span is at least 8px to the right of `Apply song`.

## Completion Criteria

- [ ] `theme.css` is created with every listed token and the coarse block.
- [ ] `index.html` links `theme.css` before `app.css`.
- [ ] `app.css` holds the base rules, variants, icon buttons, transport
  groups, headers and status bar, and is token-only.
- [ ] `song.ts` and `midi/mount.ts` class edits are made.
- [ ] Gating commands exit 0; their logs are recorded in the progress log.
- [ ] The ownership check passes and its hash table is in the progress log.

## Progress Log

Edit only this plan's log.

### Session: (implementer fills in)

- Baseline vitest count.
- Pre-edit and post-edit `git hash-object` table for the owned and watched
  non-owned files, with the sibling planId cited for each differing
  non-owned hash.
- Command exit codes and log paths.
