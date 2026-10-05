# UI Style: MIDI, Packages and Visual Sheets Implementation Plan

**Status**: Ready
**Plan ID**: UI-STYLE-PANELS (wave 1)
**Design Reference**: design-docs/specs/design-ui-style.md, section 4.5 (side-pane sections, subsection titles, header rows, label/value rows, visual area), plus sections 4.6 and 5
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

## Related Plans

- **Parallel (wave 1)**:
  - `ui-style-shell.md` (owns `theme.css`, the base rules, `.vact-primary`
    on Enable MIDI, and the file-input base styling)
  - `ui-style-bind-params.md`
  - `ui-style-code.md`
- **Next (wave 2)**: `ui-style-verify.md`

## Intent and Context

Three side-pane sections still use literal colors and uneven spacing:

| Sheet | Literal colors |
|---|---|
| `editor/src/midi/midi.css` | `#8b95a1`, `#e0b050`, `#2a2f36` |
| `editor/src/pkg/pkg.css` | `#333`, `#b90`, `#d55`, `#888`, with `6px` padding |
| `editor/src/visual/visual.css` | `#000`, `#3a1d1a`, `#ffb4a8`, `#9aa4ae`, `#0b0d10`, `#d8dde3` |

Their markup:

- `.midi-pane > .midi-header` contains `.midi-title`, `button.midi-enable`
  and `.midi-status`. It is followed by `.midi-learn` (text plus a `Cancel`
  button) and `ul.midi-devices` with `li > label > input[type=checkbox]`.
- `section.pkg-pane` contains:
  - `.pkg-title`
  - `label.pkg-proxy` holding the text `Proxy` and an `input[type=url]`
  - `.pkg-hint`
  - `ul.pkg-imports` with `li > .pkg-path` plus a `button.vact-icon-button`
    or `code`
  - `.pkg-status`
  - `ul.pkg-diagnostics`
- `section.visual-panes` contains `.visual-header` (`.visual-title`,
  `select.visual-select`, an `input[type=file][data-role=video-input]` and
  `.visual-video-status`), `.visual-banner`, and `.visual-grid >
  .visual-pane` (canvas plus `.visual-pane-label`). `.visual-analyzers`
  contains `.visual-master` and `.visual-analyzer-list` with
  `.visual-display` (canvas) and `.visual-readout`.

This plan converts the three sheets to the tokens declared by
`ui-style-shell.md`. Use the same token names as `ui-style-bind-params.md`;
`--vt-danger-bg` and `--vt-danger-text` are also needed for the banner.

## Non-Goals

- No TS/TSX edits. In particular, do not touch `midi/mount.ts` (owned by
  `ui-style-shell.md` for this run), `visual/panes.ts`, `pkg-view.tsx`, or
  the protected `visual/{frame,scopes,video,render-host}.ts`.
- Do not change canvas drawing colors in `visual/{spectrum,meters,text-asset}.ts`
  (design F2).
- Do not restyle the base button, select, input, checkbox or file-input boxes.
  `app.css` owns them.

## File-Level Changes

### `editor/src/midi/midi.css`

- `.midi-pane`: padding `var(--vt-space-2) var(--vt-space-3)`, border-bottom
  `var(--vt-border-w) solid var(--vt-border)`.
- `.midi-header`: `display: flex`, `flex-wrap: wrap`, `align-items: center`,
  `gap: var(--vt-space-2)`, `min-height: var(--vt-control-h)`.
- `.midi-title`: `--vt-text-xs`, uppercase, `letter-spacing: 0.06em`, weight
  600, `--vt-text-muted`.
- `.midi-status` and `.midi-devices-empty`: `var(--vt-text-muted)`.
- `.midi-devices`: margin `var(--vt-space-2) 0 0`.
- `.midi-devices label`:
  - `display: inline-flex`, `align-items: center`, `gap: var(--vt-space-2)`.
  - `min-height: var(--vt-control-h-compact)`. Under coarse pointers this
    makes the label the at-least-36px target.
- `.midi-learn`: gap `var(--vt-space-2)`, margin-top `var(--vt-space-2)`,
  color `var(--vt-warn)`.
- Keep `.midi-learn[hidden] { display: none; }` as is (redundant with the
  global rule, but harmless).

### `editor/src/pkg/pkg.css`

- `.pkg-pane`:
  - Gap `var(--vt-space-2)`, padding `var(--vt-space-2) var(--vt-space-3)`.
  - Border-top `var(--vt-border-w) solid var(--vt-border)`,
    `font-size: var(--vt-text-sm)`.
- `.pkg-title`: subsection title style (`--vt-text-xs`, uppercase,
  `letter-spacing: 0.06em`, weight 600, `--vt-text-muted`). It replaces
  `font-weight: bold`.
- `.pkg-proxy`:
  - `display: flex`, `flex-direction: column`, `gap: var(--vt-space-1)`,
    `color: var(--vt-text-muted)`.
  - The native tier hides this label with the `hidden` attribute. The global
    `[hidden] { display: none !important; }` rule in `app.css`
    (`ui-style-shell.md`) keeps it hidden despite the `display: flex`. Do
    not add a local `[hidden]` rule.
- `.pkg-proxy input`: keep `width: 100%`, `box-sizing` and `font: inherit`.
- `.pkg-hint`: `var(--vt-warn)`.
- `.pkg-imports li`: gap `var(--vt-space-2)`, `min-height:
  var(--vt-control-h)`.
- `.pkg-imports code`: `font-family: var(--vt-font-mono)`.
- `.pkg-diagnostics li`: `var(--vt-danger)`.
- `.pkg-status`: `var(--vt-text-muted)`.

### `editor/src/visual/visual.css`

- `.visual-panes, .visual-analyzers`: padding
  `var(--vt-space-2) var(--vt-space-3)`, border-top with tokens.
- `.visual-header`: add `flex-wrap: wrap`, `gap: var(--vt-space-2)`,
  `min-height: var(--vt-control-h)`, `margin-bottom: var(--vt-space-2)`.
- `.visual-title`: subsection title style.
- `.visual-video-status`: `var(--vt-text-muted)`, `--vt-text-sm`.
- `.visual-banner`:
  - Margin-bottom `var(--vt-space-2)`, padding
    `var(--vt-space-1) var(--vt-space-2)`.
  - Color `var(--vt-danger-text)`, background `var(--vt-danger-bg)`.
  - Font `var(--vt-text-sm)/1.3 var(--vt-font-mono)`.
- `.visual-grid`: gap `var(--vt-space-1)`.
- `.visual-pane`: `var(--vt-bg-sunken)`.
- `.visual-pane-label`:
  - Position: `top: var(--vt-space-1)`, `left: var(--vt-space-1)`.
  - Chip: background `var(--vt-surface)`, padding `0 var(--vt-space-1)`.
  - Text: `--vt-text-xs`, `--vt-text-muted`.
- `.visual-notice`: padding `var(--vt-space-2)`, `--vt-text-muted`.
- `.visual-master, .visual-analyzer-list`: gap `var(--vt-space-2)`.
- `.visual-display`: gap `var(--vt-space-1)`, `--vt-text-xs`,
  `--vt-text-muted`.
- `.visual-display canvas`: `var(--vt-bg-sunken)`.
- `.visual-readout`: font `var(--vt-text-sm)/1.3 var(--vt-font-mono)`,
  `var(--vt-text)`.

## Pitfalls

- `.visual-pane` relies on `hidden` in non-tile selection. Do not give it a
  `display` value.
- The `.visual-pane canvas` and `.visual-display canvas` selectors contain the
  type selector `canvas`. That is fine. Only color values are restricted.
- `white-space: pre-wrap` stays as is. It is a property, not a color.
- Other wave-1 plans (`UI-STYLE-SHELL`, `UI-STYLE-BIND-PARAMS`,
  `UI-STYLE-CODE`) edit sibling files in the same worktree at the same time.
  Never restore, checkout or reformat a file outside this plan's writePaths,
  even if `git status` shows it modified.
- No `rem` or `px` spacing literals remain, except `aspect-ratio` and canvas
  `width: 100%`.

## Invariants

- Selectors queried by JS and tests are unchanged:
  - `[data-role=*]`, `[data-output]`, `.visual-select`
  - `[data-pkg]`, `[data-pkg-action]`, `[data-pkg-path]`
  - `.midi-enable`, `.midi-devices`, `[data-input-id]`
- The three sheets contain no color literal, no generic font keyword, and no
  non-zero radius.

## Setup (not gating)

If `target/wasm32-unknown-unknown/debug/vactr.wasm` is missing, run from the
repository root:

`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`

## Verification (gating, run from `editor/`)

| Command | Must show |
|---|---|
| `npm run check` | Exit 0 |
| `./node_modules/.bin/vitest run` | Exit 0, test count not below the baseline |
| `npm run build` | Exit 0 |
| `grep -nE '#[0-9a-fA-F]{3,8}\b\|rgba?\(\|hsl' src/midi/midi.css src/pkg/pkg.css src/visual/visual.css` | No output |
| `grep -nE 'monospace\|system-ui\|sans-serif\|font-weight:\s*bold' src/midi/midi.css src/pkg/pkg.css src/visual/visual.css` | No output |
| Ownership check (see below) | This plan wrote only its owned files |

**Ownership check (replaces any whole-worktree diff expectation).**

Owned files are `editor/src/midi/midi.css`, `editor/src/pkg/pkg.css`,
`editor/src/visual/visual.css` and this plan file,
`impl-plans/active/ui-style-panels.md`.

The watched non-owned set is:

- `editor/src/app/app.css`
- `editor/src/app/theme.css`, if present
- `editor/index.html`
- `editor/src/app/song.ts`
- `editor/src/midi/mount.ts`
- `editor/src/bind/bind.css`
- `editor/src/params/params.css`
- `editor/src/code/code.css`

1. Before the first edit, record `git hash-object <path>` for every owned
   and watched file in the progress log. Use `absent` for a missing file.
2. After the final edit, record them again.
3. Pass when both of these hold:
   - All three owned CSS hashes changed.
   - Every watched non-owned hash that differs is owned by a sibling wave-1
     plan in `impl-plans/active/ui-style-dispatch.json`. Cite that planId.
4. Fail if a non-owned path changed that no sibling owns.

For human-readable evidence, use
`git diff --stat -- editor/src/midi/midi.css editor/src/pkg/pkg.css editor/src/visual/visual.css impl-plans/active/ui-style-panels.md`.
Do not expect a whole-worktree `git diff --stat` to list only this plan's
files.

## Test Cases

These are asserted later by `ui-style-verify.md`.

- The Visuals header has the select and the file input at least 8px apart,
  and the file input's selector button computes radius 0.
- The Packages Proxy input has a sunken fill, a 1px border and radius 0.
- Coarse pointer -> the MIDI enable button and the Visuals select are at
  least 36px tall.

## Completion Criteria

- [ ] All three sheets are token-only, with uniform section padding,
  subsection titles and header rows.
- [ ] Gating commands exit 0, with logs recorded.
- [ ] The ownership check passes and its hash table is in the progress log.

## Progress Log

Edit only this plan's log.

### Session: (implementer fills in)

- Pre-edit and post-edit `git hash-object` table for the owned and watched
  non-owned files, with the sibling planId cited for each differing
  non-owned hash.
- Command exit codes and log paths.
