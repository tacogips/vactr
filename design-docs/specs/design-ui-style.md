# Editor UI Style: Solid, Flat, Square

Status: Proposed (2026-10-05, workflow session 275, branch `wf/ui-style`).
Amended in session 276 (2026-10-05) with no redesign: the built-CSS order
rule (3.1), named-color word boundaries (8.1), the Proxy input and
native-tier `hidden` checks (8.2), and portable shell gates (8.3).

This document defines the visual system for the editor chrome in the web and
iPad (Tauri) editor: the toolbar and transport, the side pane (controls,
values, params, packages, MIDI, visuals, analyzers), the status bar, the
sample view, the completion popup, and the diagnostics overlays. It covers
design tokens, component rules, spacing, touch sizing, and how the canvas
renderer palette maps to the tokens. It does not change behavior, DOM
structure, class names used by JS or tests, accessibility labels, or focus
order.

## 1. Problem

The before screenshot is `tmp/ui-style/before/ui-full-1440x900@2x.png`. It can
be reproduced with `node tmp/ui-style/shot.mjs <outdir>` from `editor/` after
`npm run build`. The app background is dark, but:

- Many controls are unstyled native widgets: light, rounded, and packed with
  no margin. Examples are `Apply song`, `Enable MIDI`, the `editors`/`grid`/`roll`
  tabs, the persistence `<select>`, the Visuals `<select>`, the `Choose File`
  input, and the Packages Proxy input.
- Rounded corners remain: `app.css` `.vact-icon-button` (6px) and `.vact-light`
  (50%), `code.css` `.vact-playing` (2px) and `.vact-code-diag-tooltip` (3px).
- Colors, fonts, and spacing are hard-coded per file with drifting values
  (`#111`, `#333`, `#888`, `0.25rem`, `6px`, `monospace`).
- Section headers, panel padding, and toolbar grouping are inconsistent. For
  example, the "No song applied" status text touches the `Apply song` button.

## 2. Principles

1. **Square.** Every element computes `border-radius: 0px`, with no exceptions.
2. **Flat and solid.** Controls use an opaque token fill with a 1px border.
   There are no gradients, shadows, or translucent glass effects. State changes
   only fill, border, and text color.
3. **Never native.** Every form control sets `appearance: none`, and
   `-webkit-appearance: none` where needed, and is drawn from tokens. The only
   native surface left is the open `<select>` option list, which the OS draws
   and CSS cannot reach in WebKit.
4. **One token layer.** All colors, font families, font sizes, spacing,
   control heights, and focus-ring values come from CSS custom properties in a
   single file.
5. **Spaced.** Separate controls are at least 8px apart. Only declared tight
   groups use 4px.

## 3. Token Layer

### 3.1 File and Load Order

- New file: `editor/src/app/theme.css`. It contains only custom-property
  declarations on `:root`, plus `:root` overrides inside
  `@media (pointer: coarse)`. It has no component or element rules.
- `editor/index.html` links `./src/app/theme.css` immediately before
  `./src/app/app.css`. The component sheets (`bind`, `code`, `midi`, `params`,
  `pkg`, `visual`) are injected later by their `mount` modules as `<link>`
  tags. Custom properties resolve at computed-value time, so those sheets can
  use the tokens no matter when they load. No `mount.ts` file changes. This
  matters because `code/mount.ts` and `bind/mount.ts` are protected.
- Build output: Vite merges the two `index.html` sheets into one hashed asset
  under `editor/dist/assets/`, in source order, so `dist/index.html` has a
  single stylesheet link. The built-order rule is that the first `--vt-bg`
  in that asset comes before the first `.vact-icon-button`. Nothing may
  force two links (no `index.html`, `vite.config.ts` or CSS change for this).
- Prefix: `--vt-`.

### 3.2 Color Tokens

Contrast ratios below are against `--vt-raised` (#1d2128) unless stated
otherwise. All text pairs meet WCAG AA, which requires at least 4.5:1.

| Token | Value | Use |
|---|---|---|
| `--vt-bg` | `#111316` | App and code pane background. It is unchanged from today because the canvas clears to transparent and shows this color. |
| `--vt-bg-sunken` | `#0b0d10` | Text inputs, canvases, grid/roll backgrounds, visual panes |
| `--vt-surface` | `#16191e` | Toolbar, status bar, section headers, popups, tooltips |
| `--vt-raised` | `#1d2128` | Button and select fill |
| `--vt-raised-hover` | `#262b34` | Hover fill |
| `--vt-raised-active` | `#2f3540` | Active (`:active`) and pressed (`aria-pressed="true"`) fill |
| `--vt-border` | `#2a2f36` | Pane dividers and the disabled border |
| `--vt-border-strong` | `#3a414b` | Control borders and separators |
| `--vt-text` | `#d8dee9` | Primary text. This is the canvas default text color (`atlas.ts`). Contrast is about 13.8:1 on `--vt-bg` and about 11.9:1 on `--vt-raised`. |
| `--vt-text-muted` | `#9aa3ad` | Labels, section headers, status, placeholders. Contrast is about 6.3:1 on `--vt-raised`, 5.6:1 on `--vt-raised-hover`, 6.9:1 on `--vt-surface`, 7.3:1 on `--vt-bg`, 7.6:1 on `--vt-bg-sunken` and 4.7:1 on `--vt-selection`. |
| `--vt-text-disabled` | `#6b727c` | Disabled control text (WCAG exempts it) |
| `--vt-accent` | `#5aa9ff` | Primary fill, focus ring, active tab indicator, beat dot |
| `--vt-accent-hover` | `#7bbaff` | Primary hover |
| `--vt-accent-active` | `#4a94e6` | Primary active |
| `--vt-on-accent` | `#0b0d10` | Text and icons on accent (about 7.9:1, and about 6.2:1 on `--vt-accent-active`) |
| `--vt-selection` | `#24384f` | Selected completion row. `--vt-text` on it is about 8.9:1. |
| `--vt-success` | `#5fd38a` | Running, ok, MIDI locked, lit slot, run icon |
| `--vt-warn` | `#f0b44c` | Suspended, diagnostics, hush icon, hints, tier badge |
| `--vt-danger` | `#ff6b6b` | Failed, stale/unbound, errors, stop icons (about 5.8:1 on raised, 6.3:1 on surface) |
| `--vt-danger-bg` | `#3a1d1f` | Danger hover fill and the visual error banner background |
| `--vt-danger-text` | `#ffb4a8` | Text on `--vt-danger-bg` (about 9.0:1) |
| `--vt-data-1` | `#88c0d0` | Info and data marks: MIDI tag, overlay, grid steps, composition |
| `--vt-data-2` | `#ebcb8b` | Roll notes, playing highlight |
| `--vt-flash` | `rgba(136, 192, 208, 0.35)` | Eval flash |
| `--vt-flash-error` | `rgba(191, 97, 106, 0.45)` | Eval error flash |
| `--vt-playing-bg` | `rgba(235, 203, 139, 0.45)` | Playing highlight fill |
| `--vt-playing-outline` | `rgba(235, 203, 139, 0.8)` | Playing highlight outline |
| `--vt-syn-comment` | `#7a7f87` | Syntax: comment; also used for the canvas gutter |
| `--vt-syn-directive` | `#b07bd8` | Syntax: directive; also used for the `bind-ctl-text` row |
| `--vt-syn-keyword` | `#d08770` | Syntax: keyword; also used for the diagnostic tooltip border |
| `--vt-syn-number` | `#88c0d0` | Syntax: number |
| `--vt-syn-string` | `#a3be8c` | Syntax: string and path |
| `--vt-syn-head` | `#ebcb8b` | Syntax: call head |
| `--vt-syn-bracket` | `#8a8f98` | Syntax: bracket |

The `--vt-syn-*` values are identical to `renderer.ts` `GPU_TOKEN_COLORS`, so
the DOM and canvas token colors stay equal (section 7).

Two more tokens hold the image for the select chevron and the checkbox mark:
`--vt-icon-chevron` and `--vt-icon-check`. Each is an SVG data URI with a
miter-joined stroke in `#9aa3ad` (chevron) or `#0b0d10` (check). Custom
properties cannot be interpolated into a data URI, so these hex values are
literal inside `theme.css`. They must stay equal to `--vt-text-muted` and
`--vt-on-accent`.

### 3.3 Type, Spacing, Size, Focus

| Token | Value |
|---|---|
| `--vt-font-sans` | `system-ui, -apple-system, "Segoe UI", Roboto, sans-serif` |
| `--vt-font-mono` | `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace` |
| `--vt-text-xs` / `-sm` / `-md` / `-lg` | `11px` / `12px` / `13px` / `14px` |
| `--vt-space-1` .. `--vt-space-5` | `4px` / `8px` / `12px` / `16px` / `24px` |
| `--vt-control-h` | `28px` (coarse: `40px`) |
| `--vt-control-h-compact` | `24px` (coarse: `36px`). Used by slot stop buttons, bind-row icon buttons, checkbox labels and range inputs. |
| `--vt-header-h` | `28px` (coarse: `36px`). Used by section toggles and the side bar. |
| `--vt-border-w` | `1px` |
| `--vt-focus-w` / `--vt-focus-offset` | `2px` / `1px`. The color is `--vt-accent`. |
| `--vt-radius` | `0` |

Type roles:

| Role | Size | Font |
|---|---|---|
| Body | `-md` (13px) | Sans |
| Controls, side-pane text, status bar | `-sm` (12px) | Sans |
| Section headers, subsection titles, badges, pane labels | `-xs` (11px) | Sans |
| Code-like text | Same size as the surrounding text | Mono. Covers labels such as `.bind-label`, `.params-group-label`, completion rows and readouts. |

## 4. Component Rules

### 4.1 Base Element Rules (`app.css`)

`app.css` holds global type-selector rules. They have the lowest specificity,
so a component class can refine them, but no component sheet may reintroduce
a radius, native appearance, or a literal color.

- **`[hidden]`**: `display: none !important`. Base rules set `display` on
  controls, and they must not defeat the `hidden` attribute that views use
  (for example `.bind-commit`, `.pkg-proxy`, and `.params-pane`).
- **`button, input, select, textarea`**: `border-radius: var(--vt-radius)`
  and `appearance: none`. This also squares the protected canvas
  accessibility/input-bridge `textarea`. That `textarea` is invisible
  (`opacity: 0`), and its other styling stays in `code.css`.
- **`button`** and **`::file-selector-button`**:
  - `appearance: none`, radius 0, 1px `--vt-border-strong` border,
    `--vt-raised` fill, `--vt-text` color.
  - Font: `font-family: var(--vt-font-sans)` and `font-size: var(--vt-text-sm)`.
  - Sizing: `min-height: var(--vt-control-h)`, horizontal padding
    `--vt-space-3`, `inline-flex` centered, `gap: --vt-space-1`,
    `cursor: pointer`.
  - States:

    | State | Fill | Border | Text |
    |---|---|---|---|
    | `:hover:not(:disabled)` | `--vt-raised-hover` | | |
    | `:active:not(:disabled)` | `--vt-raised-active` | | |
    | `[aria-pressed="true"]` | `--vt-raised-active` | `--vt-accent` | |
    | `:disabled` | `--vt-surface` | `--vt-border` | `--vt-text-disabled` |

    Disabled controls also get `cursor: default`. The current `opacity: 0.6`
    rule is removed.
- **`:focus-visible`** on every focusable control: `outline` of `--vt-focus-w`
  solid `--vt-accent`, offset `--vt-focus-offset`. No rule may remove the
  outline without this replacement.
- **`select`**:
  - Same box as a button.
  - Right padding is `--vt-space-5` + `--vt-space-1` (28px).
  - The chevron is `background: var(--vt-icon-chevron) no-repeat right 8px center / 10px`.
- **`input`** of type `text`, `url`, `search` or `number`:
  - Box: `appearance: none`, radius 0, `--vt-bg-sunken` fill, 1px
    `--vt-border-strong` border, `min-height: var(--vt-control-h)`.
  - Text: horizontal padding `--vt-space-2`; the placeholder uses
    `--vt-text-muted`.
  - States: hover and focus-visible as for buttons; disabled as for buttons.
- **`input[type=file]`**: the element itself has no border and no fill, uses
  `--vt-text-muted` text, has `--vt-text-sm` size, and has
  `min-height: var(--vt-control-h)`, so its own box also meets the coarse
  target rule. Its
  `::file-selector-button` (and `::-webkit-file-upload-button`) is styled as a
  button with `margin-right: --vt-space-2`.
- **`input[type=checkbox]`**:
  - Box: `appearance: none`, a 16px square with radius 0, a 1px
    `--vt-border-strong` border and `--vt-bg-sunken` fill.
  - When `:checked`, it gets an `--vt-accent` fill, an `--vt-accent` border,
    and `--vt-icon-check` centered.
  - Focus, disabled, and hover follow the button rules.
- **`input[type=range]`**:
  - Box: `appearance: none`, transparent background,
    `height: var(--vt-control-h-compact)`.
  - Track (`::-webkit-slider-runnable-track` and `::-moz-range-track`): 4px tall,
    `--vt-border-strong` fill, radius 0.
  - Thumb (`::-webkit-slider-thumb` and `::-moz-range-thumb`): 10px wide and
    16px tall (coarse: 24px tall), `--vt-text` fill, no border, radius 0.
  - Disabled: the thumb uses `--vt-text-disabled`.
- **Transitions**: controls have none, so state changes are instant. The only
  transition is `.vact-section-chevron` (120ms). It gets a
  `@media (prefers-reduced-motion: reduce) { transition: none }` guard. Any
  later transition must have the same guard in the same file.
- **Focus order**: CSS must not use `order`, `*-reverse` flex/grid directions,
  or positive `tabindex`, so DOM order stays the focus order.

### 4.2 Variants

The variant classes live in `app.css`.

| Variant | Fill | Border | Text | Hover | Active |
|---|---|---|---|---|---|
| `.vact-primary` | `--vt-accent` | `--vt-accent` | `--vt-on-accent`, weight 600 | `--vt-accent-hover` | `--vt-accent-active` |

`.vact-primary` uses the normal disabled state.

The danger treatment is applied through the existing classes
`.vact-hush`, `.vact-stop-all` and `.vact-mute`; there is no markup change:

- Icon color: `--vt-danger`. `.vact-hush` is the exception and keeps the
  `--vt-warn` icon so "silence" stays distinct from "stop".
- Hover: `--vt-danger-bg` fill with an `--vt-danger` border.

Assignment:

| Control | Where | Treatment |
|---|---|---|
| `Apply song` | `app/song.ts` | Add `class="song-apply vact-primary"`. |
| `Enable MIDI` | `midi/mount.ts` | Change `className` from `midi-enable` to `midi-enable vact-primary`. The file is not protected. |
| Hush, stop every slot, per-slot stop | `.vact-hush`, `.vact-stop-all`, `.vact-mute` | Danger treatment (above) |
| All other buttons | | Default treatment. This includes song instrument `Mute`/`Unmute` buttons (pressed state through `aria-pressed`), MIDI learn `Cancel`, params `.params-open`/`.params-open-wave`, `.bind-mode`, and the run and audio buttons (state-colored icons on the default fill). |

No other markup changes are needed.

### 4.3 Icon Buttons

- `.vact-icon-button`: square, with width and height `var(--vt-control-h)`,
  padding 0 and radius 0. It otherwise inherits the button rules.
- `.vact-slot .vact-icon-button`, `.bind-row .vact-icon-button` and
  `.bind-controls .vact-icon-button`: square at `var(--vt-control-h-compact)`.
  The `0.2rem` padding override is removed.
- `.vact-light`: an 8px square (not a circle) in `--vt-border`, or
  `--vt-success` when lit.
- The SVG beat dots in `.vact-beat-ring` are icon geometry, not boxes, and stay
  circles.

### 4.4 Tabs (`params.css`)

- `.params-tabs` is a flat bar:
  - `display: flex`, `gap: var(--vt-space-1)`.
  - 1px `--vt-border` bottom border.
- `.params-tab`:
  - Box: transparent fill, no side or top border, a 2px transparent bottom
    border, `min-height: var(--vt-control-h)`, padding `0 --vt-space-3`.
  - Text: `--vt-text-muted`, `--vt-text-xs`, uppercase,
    `letter-spacing: 0.06em`.
- Hover: `--vt-text`.
- Active: the existing `.params[data-tab=X] .params-tab[data-tab=X]`
  selectors set `--vt-text`, an `--vt-accent` bottom border, and weight 600.
  These selectors and the `data-tab` attributes are unchanged.

### 4.5 Layout Surfaces

**Transport toolbar** (`.pane-transport`):

- `--vt-surface` fill, `flex-wrap: wrap`, `gap: var(--vt-space-2)`, padding
  `--vt-space-1 --vt-space-2`, 1px `--vt-border` bottom border.
- `.vact-transport` gap is `--vt-space-2`.
- Groups are separated with CSS only (DOM unchanged). A separator is a 1px
  `--vt-border-strong` `border-left` plus `padding-left: --vt-space-2` on the
  first element of each group, and none of these elements is a button:
  - `.vact-eval-status` starts the readout group: eval, tempo, position, clock.
  - `.vact-clock` gets `border-right` plus `padding-right` to close the
    readout group before the hush/stop group.
  - `.vact-level` starts the level and slots group.
  - `[data-song-controls]` starts the song group.
- `[data-song-controls]`: `display: inline-flex`, align center,
  `gap: var(--vt-space-2)`.
  - The status span uses `--vt-text-muted` and `--vt-text-sm`.
  - The alert span uses `--vt-danger`.
  - The instruments `div` is `inline-flex` with gap `--vt-space-2`.
  - This puts the "No song applied" text at least 8px away from `Apply song`.

**Side bar and section headers**:

- `.pane-side-bar`: `min-height: var(--vt-header-h)` and padding
  `--vt-space-1 --vt-space-2`.
- `.vact-section-toggle`: `min-height: var(--vt-header-h)`, padding
  `0 --vt-space-2`, `--vt-surface` fill, `--vt-text-muted` text,
  `--vt-text-xs`, uppercase, `letter-spacing: 0.06em`, weight 600.
  - Hover text is `--vt-text`.
  - It keeps the standard focus ring.

**Side-pane sections** (`.midi-pane`, `.bind-panel`, `.bind-controls`,
`.params`, `.pkg-pane`, `.visual-panes`, `.visual-analyzers`):

- Uniform padding `--vt-space-2 --vt-space-3` and 1px `--vt-border` dividers.
- Inner vertical rhythm is `--vt-space-2`.

**Subsection titles** (`.bind-panel h3`, `.bind-controls h3`, `.midi-title`,
`.pkg-title`, `.visual-title`, `.params-title`, `.bind-ctl-group h4`):

- `--vt-text-xs`, uppercase, `letter-spacing: 0.06em`, weight 600,
  `--vt-text-muted`.
- `.bind-ctl-group h4` and `.params-title` keep normal case.

**Header rows** (`.midi-header`, `.bind-controls-header`, `.visual-header`):

- `display: flex`, `flex-wrap: wrap`, align center, `gap: var(--vt-space-2)`,
  `min-height: var(--vt-control-h)`.

**Label/value rows**:

- `.bind-row` keeps its grid, with `column-gap: var(--vt-space-2)` and
  `row-gap: var(--vt-space-1)`.
- `.params-handle`, `.params-group` and `.pkg-imports li` use gap
  `--vt-space-2`.
- `.midi-devices label` is `inline-flex` with gap `--vt-space-2`, aligned
  center, and `min-height: var(--vt-control-h-compact)`.
- `.pkg-proxy`: label above input with gap `--vt-space-1`.

**Visual area**:

- `.visual-pane` and `.visual-display canvas` use the `--vt-bg-sunken` fill.
- `.visual-banner` uses `--vt-danger-bg` fill, `--vt-danger-text` text and
  mono `--vt-text-sm`.
- `.visual-pane-label` uses `--vt-text-muted` on a `--vt-surface` chip with
  padding `0 --vt-space-1`.

**Status bar** (`.pane-status`):

- `--vt-surface` fill, 1px `--vt-border` top border, padding
  `--vt-space-1 --vt-space-2`, `--vt-text-muted` and `--vt-text-sm`.
- `[data-state='failed']` uses `--vt-danger`.

**Code area** (`code.css`, token-only edits):

- **Token colors**: the `.vact-tok-*` rules move to `var(--vt-syn-*)`.
- **Highlights**:
  - Flash and playing backgrounds use the matching tokens.
  - `.vact-playing` uses radius 0.
- **GPU status** (`.vact-code-gpu-status`): `--vt-surface` fill,
  `--vt-text-muted` text, `--vt-text-xs`, padding `--vt-space-1 --vt-space-2`.
- **Diagnostic tooltip** (`.vact-code-diag-tooltip`): radius 0, `--vt-surface`
  fill, 1px `--vt-syn-keyword` border, `--vt-text` text.
- **Completion popup**: CSS only, because `completion-popup.ts` sets position,
  `maxHeight` and row height inline, and those stay.
  - `.vact-completion`: `--vt-surface` fill, 1px `--vt-border-strong` border,
    radius 0, `z-index: 10`, mono `--vt-text-sm`, minimum width 16rem.
  - `.vact-completion-item`: grid of label, kind and detail with
    `column-gap: var(--vt-space-2)`, padding `0 --vt-space-2`, align center,
    `cursor: pointer`.
  - `[aria-selected='true']` uses `--vt-selection` fill and `--vt-text`.
  - `.vact-completion-kind` uses `--vt-syn-keyword` and `--vt-text-xs`.
  - `.vact-completion-detail` uses `--vt-text-muted` with ellipsis.
- **Sample view** (`.vact-samples`): the URL input and icon buttons follow the
  base rules. `.vact-entry` keeps its existing gap of `--vt-space-2`.

**Params canvases**:

| Element | Fill or color |
|---|---|
| `.params-canvas`, `.params-grid`, `.params-roll` | `--vt-bg-sunken` |
| `.params-grid-row` divider | `--vt-border` |
| `.params-grid-step` | `--vt-data-1` |
| `.params-roll-note` | `--vt-data-2` |
| `.params-roll-note[data-lane="unpitched"]` | `--vt-text-muted` |
| `.params-hint` | `--vt-warn` |
| `.params-no-preview` | `--vt-text-muted` |

**Bind status colors**:

| Old literal | Token | Classes |
|---|---|---|
| `#d0a770` | `--vt-warn` | `.bind-tier`, `.bind-notice` |
| `#e06c75` | `--vt-danger` | State, form and marker classes, and the diagnostic entry border |
| `#88c0d0` | `--vt-data-1` | `.bind-midi`, `.bind-overlay` |
| `#b07bd8` | `--vt-syn-directive` | `.bind-ctl-text` |
| `#3b4252` | `--vt-border-strong` | Entry border |

### 4.6 Spacing Rules

- Separate adjacent visible controls on the same row are at least
  `--vt-space-2` (8px) apart.
- Only these tight groups use `--vt-space-1` (4px):
  - `.vact-slot` (light, name, stop button)
  - `.params-tabs`
  - `.params-xy-pick`
  - `.vact-eval-status` inner content
- Text that sits next to a control in the same row (status, hint, "No file
  chosen") is at least 8px from it.
- Padding, margin and gap use `--vt-space-*` tokens or `0`. Borders use
  `--vt-border-w`. Raw `rem` and `px` spacing literals are replaced. Fixed
  geometry is exempt: `.vact-preview` size, the input-bridge 1px box and the
  side rail `38px`.

## 5. Touch Sizing (`pointer: coarse`)

- `theme.css` raises `--vt-control-h` to 40px, and `--vt-control-h-compact`
  and `--vt-header-h` to 36px, inside `@media (pointer: coarse)`. Every
  interactive target is sized from these tokens, so it is at least 36px tall.
  This covers buttons, icon buttons, slot buttons, tabs, selects, inputs, file
  buttons, section toggles, range inputs, and checkbox labels.
- Overflow at 1180x820:
  - `.pane-transport` and `.vact-transport` wrap.
  - Header rows wrap.
  - `.pane-right` already scrolls.
  - `.bind-row` gets `overflow-x: auto` on `.bind-panel` rather than
    shrinking targets.
- Component sheets may add their own `@media (pointer: coarse)` blocks only to
  apply tokens to elements that do not already use them. The style test
  handles such blocks (section 8).

## 6. Change Boundary

The only files to change are:

- `editor/index.html`
- `editor/src/app/theme.css` (new)
- `editor/src/app/app.css`
- `editor/src/bind/bind.css`
- `editor/src/code/code.css` (token and radius edits only)
- `editor/src/midi/midi.css`
- `editor/src/params/params.css`
- `editor/src/pkg/pkg.css`
- `editor/src/visual/visual.css`
- `editor/src/app/song.ts` (one class attribute)
- `editor/src/midi/mount.ts` (one `className`)
- `editor/package.json` (one script)
- New tests: `editor/test/ui/style-tokens.test.ts` and
  `editor/test/style/ui-style.mjs`

No TSX view needs a change; the existing classes are enough.

The concurrency-protected files stay untouched:

- `editor/src/code/{pointer,perf-hook,renderer,mount,layout,input,keyboard,accessibility,sync,syntax,syntax-core,history,frame,highlight,transport,eval}.ts`
- `editor/src/app/clock.ts`
- `editor/src/bind/{mount,write}.ts`
- `editor/src/visual/{frame,scopes,video,render-host}.ts`
- `editor/test/e2e/*`

Their surfaces are styled only through CSS selectors and tokens.
`completion-popup.ts` is not edited either.

The canvas-drawn colors in `editor/src/visual/{spectrum,meters,text-asset}.ts`
are data-visualization drawing, not chrome. They stay as they are; see
follow-up F2.

## 7. Canvas Palette Mapping (Follow-up F1)

`renderer.ts` and `atlas.ts` hard-code their palette. They are owned by the
concurrent `wf/canvas` run and are not edited here. The canvas clears to
transparent (`renderer.ts:176`), so its background is `--vt-bg` through
`.pane-code`. That color is unchanged, so the canvas does not visually clash
with the new chrome.

Follow-up for the canvas owner: read these tokens with
`getComputedStyle(document.documentElement).getPropertyValue(...)` at mount.

| Renderer constant (location) | Current | Token | Note |
|---|---|---|---|
| `GPU_TOKEN_COLORS` (`renderer.ts:41-45`) | `#7a7f87 #b07bd8 #d08770 #88c0d0 #a3be8c #a3be8c #ebcb8b #8a8f98` | `--vt-syn-comment/directive/keyword/number/string/string/head/bracket` | Identical values |
| Default glyph color (`atlas.ts:72`) | `#d8dee9` | `--vt-text` | Identical |
| Gutter numbers (`renderer.ts:214`) | `#7a7f87` | `--vt-syn-comment` | Identical. Contrast is about 4.6:1 on `--vt-bg`. |
| Diagnostic underline (`renderer.ts:217`) | `[0.75,0.2,0.25,1]` (about `#bf3340`) | `--vt-danger` | Differs; align in the follow-up |
| Composition underline and selection handles (`:217`, `:233`) | `[0.53,0.75,0.82,1]` | `--vt-data-1` | Same color |
| Call-head underline (`:218`) | `[0.55,0.6,0.66,0.6]` | `--vt-text-muted` at 0.6 alpha | Close |
| Cursor (`:220`) | `[0.9,0.92,0.94,1]` | `--vt-text` | Close |
| Binding label fill and text (`:226-227`) | `[0.15,0.19,0.25,0.95]` / `#ebcb8b` | `--vt-raised` / `--vt-syn-head` | Close / identical |

F2: map the analyzer and spectrum canvas colors in `src/visual/{spectrum,meters,text-asset}.ts`
to the `--vt-data-*` and `--vt-text-*` tokens. This is out of scope for this
run.

## 8. Verification

### 8.1 Static Token Check (vitest, gating)

New file: `editor/test/ui/style-tokens.test.ts`. It runs in the default
`vitest run` include. It reads files with `node:fs` and asserts the following:

1. **Radius.** In every `editor/src/**/*.css`, each `border-radius` declaration
   (and each `border-*-radius` longhand) has the value `0` or
   `var(--vt-radius)`.
2. **Colors.** Every CSS file except `src/app/theme.css` contains no color
   literal: no `#hex`, `rgb(`, `rgba(`, `hsl(`, or named colors other than
   `transparent`, `currentColor` and `inherit`. Named colors are matched
   only in values of color-bearing properties, and only as whole words. A
   word joined to a letter, digit or `-` is not a color, so `white-space`,
   `--vt-*` names and identifiers such as `blackout` never match.
3. **Fonts.** Every CSS file except `theme.css` contains no generic font-family
   keyword (`monospace`, `sans-serif`, `system-ui`, `ui-monospace`,
   `ui-sans-serif`).
4. **Token file shape.** `theme.css` contains only `:root` rules, optionally
   inside `@media (pointer: coarse)`, and declares every token named in
   sections 3.2 and 3.3.
5. **Load order.** `editor/index.html` links `./src/app/theme.css` before
   `./src/app/app.css`.
6. **Reduced motion.** Every CSS file with a `transition` or `animation`
   declaration also contains a `prefers-reduced-motion` block.

### 8.2 Playwright Style Test (outside the sandbox)

The test is `editor/test/style/ui-style.mjs`, run by
`npm run test:style` (`node test/style/ui-style.mjs`).

**Location.** It is not under `test/e2e/` and does not match the vitest
include pattern, so the gating suite never runs it. It imports `startServer`
from `../e2e/serve.mjs` read-only and serves the built `editor/dist`, with no
backend. It requires `npm run build` first. It runs outside the Codex sandbox
because it binds `127.0.0.1`.

**Silent.**

- Both engines launch headless. Chromium also gets `--mute-audio`; WebKit has
  no such flag, and the next rule keeps it silent.
- The script never clicks, presses keys in, or focuses audio, run, transport,
  or song controls. Its only interactions are `hover` on non-transport fixture
  controls and `focus` via `Tab` on fixture controls.
- It never changes system volume.

**Engines and viewports.** Chromium and WebKit, each at:

- A. 1440x900, `deviceScaleFactor: 2`, fine pointer.
- B. 1180x820, `deviceScaleFactor: 2`, `hasTouch: true`, `isMobile: true`
  where the engine supports it.

**Coarse mode.**

- If `matchMedia('(pointer: coarse)').matches` is false in viewport B, the
  script forces coarse mode. It collects the inner `cssText` of every
  `CSSMediaRule` whose `conditionText` contains `pointer: coarse`, from all
  loaded stylesheets, and appends that text unwrapped through `addStyleTag`.
  This applies the shipped rules, not a copy.
- It logs `coarse-mode: emulated|forced` per engine.

**Fixture probe.**

- After the app settles, the script appends a `<div data-style-probe>` to
  `.pane-right`. It contains:
  - A `button`, a disabled `button`, and a `button.vact-primary`.
  - A `.vact-icon-button`.
  - A `select`.
  - `input`s of type `text` and `url`.
  - A `label` that wraps an `input[type=checkbox]`.
  - An `input[type=range]` and an `input[type=file]`.
  - A `label.pkg-proxy` with the `hidden` attribute that wraps an
    `input[type=url]`. This is the native-tier markup from `pkg-view.tsx`.
    The test serves the browser tier (no `?session=`), so this is the only
    deterministic way to check the native tier without a backend.
- These cover base rules for controls that only appear after user actions,
  such as MIDI device checkboxes and bind sliders.
- The probe is removed before screenshots.

**Assertions.** Each engine and viewport fails with a list of offending
selectors and values.

A control counts as visible when all of these hold:

- Its bounding box is non-zero.
- Its computed `display` is not `none` and its `visibility` is not `hidden`.
- Its own and ancestor `opacity` is above 0. This excludes the canvas input
  bridge.

1. **Radius.** Every element in the document, including the probe, computes
   all four `border-*-radius` as `0px`.
2. **Appearance.**
   - Every visible `button`, `select` and `input` that is not hidden computes
     `appearance` (or `-webkit-appearance`) as `none`.
   - In the probe, the chevron is present: `select` has a
     `background-image` other than `none`.
   - For `::file-selector-button`, the script reads
     `getComputedStyle(input, '::file-selector-button')`. If the engine returns
     an empty `borderTopLeftRadius` for the pseudo element, it logs
     `file-pseudo: unsupported` for that engine and relies on the screenshot.
     Otherwise it asserts radius `0px` and a background color equal to the
     probe `button`'s.
3. **Native colors.** No visible control computes a background luminance
   above 0.5. This rejects the white/light-grey native look; the primary
   accent has luminance of about 0.38.
4. **Gaps.**
   - For visible controls in the same row, the script looks at each control
     and the nearest control to its right, among pairs whose vertical ranges
     overlap by at least half the smaller height. The horizontal gap must be
     at least 8px. When both controls share the nearest tight-group ancestor
     from section 4.6, it must be at least 4px. No two controls overlap.
   - Named pair: the left edge of the song status span is at least 8px past
     the right edge of `Apply song`.
5. **Coarse targets.** In viewport B, every visible `button`, `select`,
   `input` that is not a checkbox, and the probe checkbox `label` have a
   bounding height of at least 36px.
6. **Focus ring.** After `Tab` into the probe `button`, it computes
   `outline-style: solid` and `outline-width: 2px`.
7. **Packages Proxy.**
   - Browser tier: the real `.pkg-proxy input` (outside the probe) is
     visible. All four corner radii are `0px`. `background-color` equals the
     probe text input's (the `--vt-bg-sunken` fill). `border-top-width` is
     `1px`. In viewport B its height is at least 36px.
   - Native tier: the probe `label.pkg-proxy[hidden]` computes
     `display: none`, so the global `[hidden]` rule beats the
     `.pkg-proxy { display: flex }` rule. It and its input are left out of
     every visibility-based check.
   - No source change is needed for this. `pkg.css` sets the fill and
     border, and the zero-specificity base rules supply radius 0 and the
     minimum height.

**Screenshots.** Written to `tmp/ui-style/after/`:

- `<engine>-1440x900@2x.png`
- `<engine>-1180x820-coarse@2x.png`

The intake harness command `node tmp/ui-style/shot.mjs tmp/ui-style/after` also
writes `tmp/ui-style/after/ui-full.png`. For comparison, see
`tmp/ui-style/before/ui-full-1440x900@2x.png`.

**Exit status.** The script exits 0 only if every assertion passes in both
engines.

### 8.3 Gating Commands (run from `editor/`)

- `npm run check`
- `./node_modules/.bin/vitest run`, which includes the static token check and
  keeps existing assertions unchanged.
- `npm run build`
- After a build, outside the sandbox:
  - `npm run test:style`
  - `node ../tmp/ui-style/shot.mjs ../tmp/ui-style/after`

Shell gates in plans and the manifest must stay portable to BSD grep on
macOS, which has no `\s`:

- Radius: `! grep -nE 'border-radius:[[:space:]]*[^[:space:]0v;]' <css>`.
  It passes on `0` and `var(--vt-radius)` and fails on any other literal.
- Named colors: match whole words only, for example
  `(^|[^-[:alnum:]])(white|black)([^-[:alnum:]]|$)`, so `white-space` does
  not match (8.1 check 2).
- Built order: a read-only Node check over `editor/dist/assets/*.css`. It
  exits nonzero unless `indexOf('--vt-bg')` is at least 0 and less than
  `indexOf('.vact-icon-button')` (3.1). No check expects two stylesheet
  links in `dist/index.html`.

Negative controls are reported separately and are not gating. For example,
temporarily set `border-radius: 4px` in `pkg.css`; both the vitest token check
and the style test must fail.

## 9. Implementation Split

Token names are fixed above, so the three units can run in parallel. Their
write paths do not overlap.

| Unit | Write paths |
|---|---|
| U1: tokens, base rules, shell | `editor/src/app/theme.css`, `editor/index.html`, `editor/src/app/app.css`, `editor/src/app/song.ts`, `editor/src/midi/mount.ts` |
| U2: component sheets | `editor/src/bind/bind.css`, `editor/src/code/code.css`, `editor/src/midi/midi.css`, `editor/src/params/params.css`, `editor/src/pkg/pkg.css`, `editor/src/visual/visual.css` |
| U3: verification | `editor/test/ui/style-tokens.test.ts`, `editor/test/style/ui-style.mjs`, `editor/package.json` |

Artifact roots: `target/`, `editor/dist/`, `tmp/ui-style/`,
`tree-sitter-vact/tree-sitter-vact.wasm`.

## 10. Decisions Recorded with Defaults

Open user choices, each with a default that is followed unless overridden, are
in `design-docs/user-qa/pending-ui-style-questions.md`:

- UQ1: accent hue
- UQ2: danger assignment for hush
- UQ3: the global `[hidden]` rule
