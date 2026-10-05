# UI Style: Bind and Params Sheets Implementation Plan

**Status**: Ready
**Plan ID**: UI-STYLE-BIND-PARAMS (wave 1)
**Design Reference**: design-docs/specs/design-ui-style.md, sections 4.3 (bind-row icon buttons), 4.4 (tabs), 4.5 (side-pane sections, subsection titles, label/value rows, params canvases, bind status colors), 4.6 and 5
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

## Related Plans

- **Parallel (wave 1)**:
  - `ui-style-shell.md` (owns `theme.css`, the base element rules and
    `.vact-icon-button`)
  - `ui-style-code.md`
  - `ui-style-panels.md`
- **Next (wave 2)**: `ui-style-verify.md`

## Intent and Context

The controls and values pane and the params pane look worst today:

- Native rounded white buttons with 0 margin.
- The editors/grid/roll tabs are native buttons with only `font-weight: bold`
  marking the active one.
- Literal colors such as `#111`, `#fc6`, `#8fd`, `#e06c75` and `#d0a770`.
- `rem` spacing.

This plan converts `editor/src/bind/bind.css` and
`editor/src/params/params.css` to the tokens declared by `ui-style-shell.md`
in `editor/src/app/theme.css`. The base element rules in `app.css` already
make every `button`, `select` and range input flat and square; this plan
only adds component layout and color.

Token names to use (exact):

- Surfaces and borders: `--vt-bg-sunken`, `--vt-surface`, `--vt-border`,
  `--vt-border-strong`
- Text: `--vt-text`, `--vt-text-muted`
- Accent and status: `--vt-accent`, `--vt-warn`, `--vt-danger`
- Data and syntax: `--vt-data-1`, `--vt-data-2`, `--vt-syn-directive`
- Fonts and sizes: `--vt-font-sans`, `--vt-font-mono`, `--vt-text-xs`,
  `--vt-text-sm`
- Spacing and sizes: `--vt-space-1..5`, `--vt-control-h`,
  `--vt-control-h-compact`, `--vt-border-w`, `--vt-radius`

The sheets load later than `theme.css` (injected by `bind/mount.ts` and
`params/mount.ts`). Custom properties still resolve, so no load-order work is
needed.

## Non-Goals

- Do not edit any TSX/TS file. Every needed class already exists:
  - `.params-tab[data-tab]`, `.params-open`, `.params-open-wave`
  - `.bind-row`, `.bind-mode`, `.bind-select`, `.bind-slider`
  - `.bind-persistence`, `.bind-save`
  - `.params-handle-input`, `.params-learn`, `.params-xy-x`, `.params-xy-y`
- Do not edit `bind/mount.ts` or `bind/write.ts` (protected), or any other
  sheet.
- Do not restyle base controls here (fills, borders, focus). `app.css` owns
  them. Do not redeclare `appearance` or the button palette.

## File-Level Changes

### `editor/src/bind/bind.css`

**Sections.**

- `.bind-panel, .bind-controls`:
  - Font: `font: var(--vt-text-sm) var(--vt-font-sans)`.
  - Padding `var(--vt-space-2) var(--vt-space-3)`; top border
    `var(--vt-border-w) solid var(--vt-border)`.
- `.bind-panel` gets `overflow-x: auto` (coarse overflow, design section 5).

**Subsection titles** (`h3` in both sections):

- `--vt-text-xs`, uppercase, `letter-spacing: 0.06em`, weight 600,
  `--vt-text-muted`.
- Margin `var(--vt-space-2) 0 var(--vt-space-1)`.
- `.bind-ctl-group h4`: `--vt-text-xs`, normal case, `--vt-text-muted`,
  margin `var(--vt-space-1) 0 0`.

**Rows.**

- `.bind-row`:
  - Keep `grid-template-columns` (fixed geometry; `rem` there is allowed).
  - Set `column-gap: var(--vt-space-2)`, `row-gap: var(--vt-space-1)`,
    padding `var(--vt-space-1) 0`.
- `.bind-name`: gap `var(--vt-space-2)`.
- `.bind-controls-header`:
  - `display: flex`, `flex-wrap: wrap`, `align-items: center`.
  - `gap: var(--vt-space-2)`, `min-height: var(--vt-control-h)`.
  - Its `h3` has `margin: 0`.

**Fonts.** `.bind-label`, `.bind-name-label`, `.bind-ctl-text` and
`.bind-ctl-binding` use `var(--vt-font-mono)`. Remove the literal
`ui-monospace, SFMono-Regular, Menlo, monospace`.

**Status colors.**

| Classes | Token |
|---|---|
| `.bind-tier`, `.bind-notice` | `--vt-warn` |
| Stale/unbound `.bind-state`, `.bind-form`, `.bind-name-badge`, `.bind-ctl-marker`, the `[data-diagnostic]` border | `--vt-danger` |
| `.bind-midi`, `.bind-overlay` | `--vt-data-1` |
| `.bind-ctl-text` | `--vt-syn-directive` |
| `.bind-ctl-entry` left border | `var(--vt-border-strong)`, keeping the 2px width |

Badge sizes of `10px` become `var(--vt-text-xs)`.

**Icon buttons.**

- `.bind-row .vact-icon-button, .bind-controls .vact-icon-button`:
  - Replace `padding: 0.2rem` with `padding: 0`.
  - Width and height `var(--vt-control-h-compact)`.
- `.bind-select`: keep `width: 100%`. The base `select` rule handles
  appearance.

### `editor/src/params/params.css`

**Section.**

- `.params`:
  - `font: var(--vt-text-sm)/1.3 var(--vt-font-sans)`, gap
    `var(--vt-space-2)`.
  - Padding `var(--vt-space-2) var(--vt-space-3)`; top border with tokens.

**Tabs (design section 4.4).**

- `.params-tabs`: `display: flex`, `gap: var(--vt-space-1)` (tight group),
  bottom border `var(--vt-border-w) solid var(--vt-border)`.
- `.params-tab`:
  - Box: transparent background, `border: 0`,
    `border-bottom: 2px solid transparent`,
    `min-height: var(--vt-control-h)`, `padding: 0 var(--vt-space-3)`.
  - Text: `--vt-text-muted`, `--vt-text-xs`, uppercase,
    `letter-spacing: 0.06em`.
  - Hover: `--vt-text`. Keep the background transparent on hover and active,
    and override the base fills with class-level rules.
- Active tab: keep the three existing selectors
  `.params[data-tab="X"] .params-tab[data-tab="X"]` exactly. They set
  `--vt-text`, `border-bottom-color: var(--vt-accent)` and weight 600.

**Lists, groups and handles.**

- `.params-list` and `.params-handles`: gap `var(--vt-space-1)`. Rows stack
  vertically, so the 8px rule applies only within a row.
- `.params-group`, `.params-handle` and `.params-xy-pick`:
  - `.params-group` and `.params-handle` gap `var(--vt-space-2)`.
  - `.params-xy-pick` gap `var(--vt-space-1)` (declared tight group).
- `.params-group-label`, `.params-handle-label` and `.params-grid-slot` use
  `var(--vt-font-mono)`, replacing the bare `monospace`.
- `.params-title`: `--vt-text-xs`, weight 600, `--vt-text-muted`, margin-top
  `var(--vt-space-1)`.

**Canvases and marks.**

| Selector | Token |
|---|---|
| `.params-canvas`, `.params-grid`, `.params-roll` | `--vt-bg-sunken` |
| `.params-grid-row` border | `--vt-border` |
| `.params-grid-slot` | `--vt-text-muted` |
| `.params-grid-step` | `--vt-data-1` |
| `.params-roll-note` | `--vt-data-2` |
| `.params-roll-note[data-lane="unpitched"]` | `--vt-text-muted` |
| `.params-hint` | `--vt-warn` |
| `.params-no-preview` | `--vt-text-muted` |

`.params-handle.params-disabled` keeps `opacity: 0.45`. That is a state, not
a color literal.

## Pitfalls

- Do not use a bare named color anywhere: no `white`, `black` or `red`.
  `transparent` and `currentColor` are allowed.
- Do not change `grid-template-columns` widths. Bind tests do not measure
  them, but the layout depends on them.
- Tabs must override base hover fills at class level. Because the base rules
  use `:where()`, a plain `.params-tab:hover` wins. Do not use `!important`.
- Do not add `display` to `.params-pane`. It relies on `hidden`.
- Other wave-1 plans (`UI-STYLE-SHELL`, `UI-STYLE-CODE`, `UI-STYLE-PANELS`)
  edit sibling files in the same worktree at the same time. Never restore,
  checkout or reformat a file outside this plan's writePaths, even if
  `git status` shows it modified.

## Invariants

- No selector used by tests or JS is removed. That includes `.params-tab`,
  `[data-tab]`, `.bind-row`, `.bind-group:not(:has(.bind-row))` and the
  `:empty` hides.
- No border-radius other than 0, no color literal and no generic font
  keyword remain in either file.

## Setup (not gating)

If `target/wasm32-unknown-unknown/debug/vactr.wasm` is missing, run from the
repository root:

`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`

## Verification (gating, run from `editor/`)

| Command | Must show |
|---|---|
| `npm run check` | Exit 0 |
| `./node_modules/.bin/vitest run` | Exit 0, test count not below the recorded baseline |
| `npm run build` | Exit 0 |
| `grep -nE '#[0-9a-fA-F]{3,8}\b\|rgba?\(\|hsl\|\bwhite\b\|\bblack\b' src/bind/bind.css src/params/params.css` | No output |
| `grep -nE 'monospace\|system-ui\|sans-serif' src/bind/bind.css src/params/params.css` | No output |
| Ownership check (see below) | This plan wrote only its owned files |

**Ownership check (replaces any whole-worktree diff expectation).**

Owned files are `editor/src/bind/bind.css`, `editor/src/params/params.css`
and this plan file, `impl-plans/active/ui-style-bind-params.md`.

The watched non-owned set is:

- `editor/src/app/app.css`
- `editor/src/app/theme.css`, if present
- `editor/index.html`
- `editor/src/app/song.ts`
- `editor/src/midi/mount.ts`
- `editor/src/code/code.css`
- `editor/src/midi/midi.css`
- `editor/src/pkg/pkg.css`
- `editor/src/visual/visual.css`

1. Before the first edit, record `git hash-object <path>` for every owned
   and watched file in the progress log. Use `absent` for a missing file.
2. After the final edit, record them again.
3. Pass when both of these hold:
   - Both owned CSS hashes changed.
   - Every watched non-owned hash that differs is owned by a sibling wave-1
     plan in `impl-plans/active/ui-style-dispatch.json`. Cite that planId.
4. Fail if a non-owned path changed that no sibling owns.

For human-readable evidence, use
`git diff --stat -- editor/src/bind/bind.css editor/src/params/params.css impl-plans/active/ui-style-bind-params.md`.
Do not expect a whole-worktree `git diff --stat` to list only this plan's
files.

## Test Cases

These are asserted later by `ui-style-verify.md`.

- Params tabs render with 4px between them, the active tab has an accent
  underline, and the computed radius is 0.
- The persistence select and the Save icon button in
  `.bind-controls-header` are at least 8px apart.
- Coarse pointer -> the tabs, select and Save button are at least 36px tall.

## Completion Criteria

- [ ] Both sheets are token-only, with section padding and subsection titles
  per the design.
- [ ] Tabs are restyled with the active selectors unchanged.
- [ ] Gating commands exit 0, with logs recorded.
- [ ] The ownership check passes and its hash table is in the progress log.

## Progress Log

Edit only this plan's log.

### Session: (implementer fills in)

- Baseline vitest count.
- Pre-edit and post-edit `git hash-object` table for the owned and watched
  non-owned files, with the sibling planId cited for each differing
  non-owned hash.
- Command exit codes and log paths.
