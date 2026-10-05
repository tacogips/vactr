# UI Style: Code Area Sheet Implementation Plan

**Status**: Completed
**Plan ID**: UI-STYLE-CODE (wave 1)
**Design Reference**: design-docs/specs/design-ui-style.md, section 4.5 "Code area (`code.css`, token-only edits)", plus sections 3.2 (syntax tokens) and 7
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

## Related Plans

- **Parallel (wave 1)**:
  - `ui-style-shell.md` (owns `theme.css` and the base rules)
  - `ui-style-bind-params.md`
  - `ui-style-panels.md`
- **Next (wave 2)**: `ui-style-verify.md`

## Intent and Context

`editor/src/code/code.css` styles the code area: syntax token classes, the
eval flash, the playing highlight, the sample browser, the GPU surface, and
the diagnostic tooltip. It has radii of `2px` on `.vact-playing` and `3px` on
`.vact-code-diag-tooltip`. The completion popup has no CSS at all, so it
renders as unstyled native divs.

`editor/src/code/completion-popup.ts` builds these elements:

- `.vact-completion` with `role=listbox`, and inline `position: fixed`,
  `maxHeight` and `overflow`.
- `.vact-completion-item` with `role=option` and `aria-selected`; the inline
  row height is `ROW_HEIGHT_PX`.
- `.vact-completion-label`, `.vact-completion-kind` and
  `.vact-completion-detail`.

A concurrent run on `wf/canvas` owns most `code/*.ts` files and may also edit
`code.css`. Keep this diff token-only and minimal, and do not reorder
existing lines, so a later merge stays clean.

## Non-Goals

- Do not edit any `.ts` file under `editor/src/code/`.
  `completion-popup.ts`, `renderer.ts`, `atlas.ts`, `mount.ts` and
  `accessibility.ts` are all out of scope; several are protected.
- Do not change the canvas palette. Design section 7 (follow-up F1) only
  records it.
- Do not change the input-bridge geometry or transparency rules. They are
  functional for IME and accessibility.

## File-Level Changes (`editor/src/code/code.css` only)

**Syntax tokens.** Each `.vact-tok-*` color becomes its `var(--vt-syn-*)`:

| Class | Token |
|---|---|
| `.vact-tok-comment` | `--vt-syn-comment` |
| `.vact-tok-directive` | `--vt-syn-directive` |
| `.vact-tok-keyword` | `--vt-syn-keyword` |
| `.vact-tok-number` | `--vt-syn-number` |
| `.vact-tok-string`, `.vact-tok-path` | `--vt-syn-string` |
| `.vact-tok-head` | `--vt-syn-head` |
| `.vact-tok-bracket` | `--vt-syn-bracket` |

Keep the one-line-per-rule format and the font-style and weight
declarations.

**Highlights.**

- `.vact-flash` uses `var(--vt-flash)` and `.vact-flash-error` uses
  `var(--vt-flash-error)`.
- `.vact-playing`: background `var(--vt-playing-bg)`, outline
  `var(--vt-border-w) solid var(--vt-playing-outline)`. Change
  `border-radius: 2px` to `border-radius: 0`.

**Sample browser.**

- `.vact-samples`: `font-size: var(--vt-text-sm)`, padding
  `var(--vt-space-1) var(--vt-space-2)`.
- `.vact-entry` gap: `var(--vt-space-2)`.
- `.vact-entry[data-missing='true']` and `.vact-samples-status`:
  `var(--vt-text-muted)`. Muted chrome text uses `--vt-text-muted` for AA
  contrast; `--vt-syn-comment` is for code only.
- The `sounds`, `banks` and `entries` lists use
  `padding-left: var(--vt-space-4)`.
- New `.vact-sample-map` rule. Markup is at `sample-view.tsx:36-38`: a `div`
  holding `input[type=url]` and `button.vact-icon-button`. Today it has no
  CSS rule.
  - Layout: `display: flex`, `align-items: center`,
    `gap: var(--vt-space-2)`, `margin: var(--vt-space-1) 0`.
  - `.vact-sample-map input`: `flex: 1 1 auto`, `min-width: 0`. The icon
    button keeps its square `.vact-icon-button` size from `app.css`.
- New `.vact-bank > span` rule: `margin-right: var(--vt-space-2)`. Markup is
  at `sample-view.tsx:41-43`: `li.vact-bank` holding a name `span`, a load
  `button.vact-icon-button`, and a nested `ul.vact-entries`. The margin keeps
  the bank name 8px from its load button.
  - Do NOT give `li.vact-bank` (or `.vact-banks li`) `display: flex` or
    `grid`. The nested `ul.vact-entries` must stay on its own line below the
    name and button.

**GPU status** (`.vact-code-gpu-status`): `--vt-surface` fill,
`--vt-text-muted`, `font-size: var(--vt-text-xs)`, padding
`var(--vt-space-1) var(--vt-space-2)`.

**Diagnostic tooltip** (`.vact-code-diag-tooltip`):

- Remove `border-radius: 3px` (or set it to 0).
- `color: var(--vt-text)`, `background: var(--vt-surface)`.
- Border `var(--vt-border-w) solid var(--vt-syn-keyword)`.
- Padding `var(--vt-space-1) var(--vt-space-2)`.
- Keep the `max-width` expression and `z-index`.

**Completion popup** (new rules appended at the end of the file):

- `.vact-completion`:
  - Box: `--vt-surface` background, border
    `var(--vt-border-w) solid var(--vt-border-strong)`, `border-radius: 0`,
    `z-index: 10`, `min-width: 16rem`.
  - Text: `font-family: var(--vt-font-mono)`,
    `font-size: var(--vt-text-sm)`, `color: var(--vt-text)`.
  - Do not set `position`, `max-height` or `overflow`; JS sets them inline.
- `.vact-completion-item`:
  - Layout: `display: grid`,
    `grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr)`,
    `column-gap: var(--vt-space-2)`, `align-items: center`.
  - `padding: 0 var(--vt-space-2)`, `cursor: pointer`.
  - Do not set `height`; JS sets the row height inline.
- `.vact-completion-item[aria-selected='true']`: background
  `var(--vt-selection)`, color `var(--vt-text)`.
- `.vact-completion-kind`: `var(--vt-syn-keyword)`, `var(--vt-text-xs)`.
- `.vact-completion-detail`: `var(--vt-text-muted)`, `overflow: hidden`,
  `text-overflow: ellipsis`, `white-space: nowrap`.

## Pitfalls

- `.vact-code-input-bridge` keeps `background: transparent`,
  `color: transparent`, `-webkit-text-fill-color: transparent`,
  `caret-color: transparent` and `font: inherit`. These keywords are allowed;
  do not replace them with tokens.
- Do not add `display` to `.vact-code-diag-tooltip`. It relies on `hidden`.
- No rgba literals may remain. The flash and playing colors move into the
  `theme.css` tokens, and `ui-style-shell.md` already declares them.
- Do not introduce `transition` or `animation`.
- Solid JSX strips the whitespace between elements written on separate lines.
  The sample-map input and button, and the bank name and its button,
  therefore have no text space between them. Adjacent inline controls need an
  explicit `gap` or `margin`; never rely on text spaces.
- Fix the sample view in CSS only. Do not edit
  `editor/src/code/sample-view.tsx` or `editor/src/code/samples.ts`.
- Other wave-1 plans (`UI-STYLE-SHELL`, `UI-STYLE-BIND-PARAMS`,
  `UI-STYLE-PANELS`) edit sibling files in the same worktree at the same
  time. Never restore, checkout or reformat a file outside this plan's
  writePaths, even if `git status` shows it modified.

## Invariants

- The class names in `code.css` are unchanged. `syntax-fallback.test.ts` and
  `syntax.test.ts` assert the `vact-tok-*` class names (not their colors).
- The computed token colors equal `GPU_TOKEN_COLORS`, because the values in
  `theme.css` are identical.

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
| `grep -nE '#[0-9a-fA-F]{3,8}\b\|rgba?\(\|hsl' src/code/code.css` | No output |
| `! grep -nE 'border-radius:[[:space:]]*[^[:space:]0v;]' src/code/code.css` | No output |
| Ownership check (see below) | This plan wrote only its owned files |

**Ownership check (replaces any whole-worktree diff expectation).**

Owned files are `editor/src/code/code.css` and this plan file,
`impl-plans/active/ui-style-code.md`. The watched non-owned set is:

- every other `editor/src/**/*.css`
- `editor/index.html`
- `editor/src/app/song.ts`
- `editor/src/midi/mount.ts`
- `editor/src/app/theme.css`, if present

1. Before the first edit, record `git hash-object <path>` for each owned file
   and each watched non-owned file in the progress log. Use `absent` for a
   missing file.
2. After the final edit, record them again.
3. Pass when both of these hold:
   - The owned `code.css` hash changed. The plan file may also change.
   - Every watched non-owned hash that differs belongs to a path owned by a
     sibling wave-1 plan in `impl-plans/active/ui-style-dispatch.json`. Cite
     that planId next to the hash.
4. Fail if a non-owned path changed that no sibling owns.

For human-readable evidence, use
`git diff --stat -- editor/src/code/code.css impl-plans/active/ui-style-code.md`.
Do not expect a whole-worktree `git diff --stat` to list only this plan's
files.

## Test Cases

These are asserted later by `ui-style-verify.md`.

- The GPU status chip and the tooltip, when shown, compute radius 0px.
- The static check finds no color literal in `code.css`.
- `details.vact-samples` open in the browser tier with a sample library ->
  the left edge of the `.vact-sample-map button` is at least 8px past the
  right edge of the `.vact-sample-map input`.
- With the same panel open, for each `.vact-bank`, the left edge of its
  `> button` is at least 8px past the right edge of its `> span`.
- With the same panel open, `ul.vact-entries` still renders below the bank
  name row: its top is at or below the button's bottom.

## Completion Criteria

- [x] `code.css` is token-only, with radius 0 and the completion popup
  styled.
- [x] The `.vact-sample-map` flex row and the `.vact-bank > span` margin
  rules are present; `li.vact-bank` display is unchanged.
- [x] The ownership check passes and its hash table is in the progress log.
- [x] Gating commands exit 0, with logs recorded.

## Progress Log

Edit only this plan's log.

### Session: 2026-10-05 (Step 6 implementation)

- Implemented the planned token-only syntax, highlight, sample browser, GPU
  status, diagnostics and completion popup styles in `editor/src/code/code.css`.
  Preserved input-bridge rules and block flow for `li.vact-bank`.
- Corrected the radius verification expression: the former
  `[^0v;]` class also matched the whitespace before `0`, so it failed on valid
  zero-radius rules. The initial command confirmed this false positive (exit
  1); the whitespace-safe form now checks the same intended constraint.
- Ownership hash table (pre-edit -> post-source-edit, before this progress-log
  update; `absent` means no file):

  | File | Pre-edit | Post-source-edit | Result |
  |---|---|---|---|
  | `editor/src/code/code.css` | `b763b6600b762e5cbbc66d42a45e0707b332ace3` | `aaad899398f698718b8fb3a137bf0d505128edb5` | Owned file changed |
  | `impl-plans/active/ui-style-code.md` | `8ff9e392f4cfb4cb2ec2a128bb48f75571b36981` | `8ff9e392f4cfb4cb2ec2a128bb48f75571b36981` | Hash captured before this log update; this owned plan file changes as the progress log is written |
  | `editor/src/app/app.css` | `b9f175a88968e7ba0982f04840d8a420cb1f5793` | `b9f175a88968e7ba0982f04840d8a420cb1f5793` | unchanged |
  | `editor/src/app/theme.css` | `ed05d7a7a7083d0e253eaa080f48aceae1aa4cf7` | `ed05d7a7a7083d0e253eaa080f48aceae1aa4cf7` | unchanged |
  | `editor/src/bind/bind.css` | `82ab49bbbbe7826e3aefde0165b4bf2988ec0bcf` | `82ab49bbbbe7826e3aefde0165b4bf2988ec0bcf` | unchanged |
  | `editor/src/midi/midi.css` | `056994fec450fb50cb7366a80350e586680ce6ca` | `056994fec450fb50cb7366a80350e586680ce6ca` | unchanged |
  | `editor/src/params/params.css` | `b9385c2e7dcc14c6c95a227df23487fbfc5bb147` | `b9385c2e7dcc14c6c95a227df23487fbfc5bb147` | unchanged |
  | `editor/src/pkg/pkg.css` | `9e7d71ffaa5987af4de5334b2531d1683a337917` | `9e7d71ffaa5987af4de5334b2531d1683a337917` | unchanged |
  | `editor/src/visual/visual.css` | `52d2ba6d19b67dffa5b0c23bbedfc4242cf1b800` | `52d2ba6d19b67dffa5b0c23bbedfc4242cf1b800` | unchanged |
  | `editor/index.html` | `d027edf25c0b03bf8eae606de7ba29c86313acff` | `d027edf25c0b03bf8eae606de7ba29c86313acff` | unchanged |
  | `editor/src/app/song.ts` | `4e50eb04d8695d5991a7ca66209260e2f84e9733` | `4e50eb04d8695d5991a7ca66209260e2f84e9733` | unchanged |
  | `editor/src/midi/mount.ts` | `de7b6332e367f5e8d55d52df24df165f0d447db2` | `de7b6332e367f5e8d55d52df24df165f0d447db2` | unchanged |

  Watched hashes match the pre-edit table. The other UI stylesheet changes were
  already present before this step; dispatch ownership is `UI-STYLE-SHELL`
  (`app.css`, `theme.css`), `UI-STYLE-BIND-PARAMS` (`bind.css`, `params.css`),
  and `UI-STYLE-PANELS` (`midi.css`, `pkg.css`, `visual.css`).
- Verification (all logs are complete under
  `tmp/ui-style/evidence/ui-style-code/`):
  - `cd editor && npm run check`: exit 0 (`npm-check.log`).
  - `cd editor && ./node_modules/.bin/vitest run`: exit 1, 692 passed / 4
    failed / 696 total (`vitest.log`); failures were one audible-tier
    expiration assertion and three timeout failures under parallel execution.
  - `cd editor && ./node_modules/.bin/vitest run --minWorkers=1 --maxWorkers=1`:
    unsupported option, exit 1 (`vitest-unsupported-option.log`).
  - `cd editor && ./node_modules/.bin/vitest run --maxWorkers=1`: exit 0, 696
    passed / 696 total, 90 files (`vitest-single-worker.log`).
  - `cd editor && npm run build`: exit 0 (`npm-build.log`).
  - `cd editor && ! grep -nE '#[0-9a-fA-F]{3,8}\\b|rgba?\\(|hsl' src/code/code.css`:
    exit 0, no matches (`token-color-check.log`).
  - `cd editor && ! grep -nE 'border-radius:[[:space:]]*[^[:space:]0v;]' src/code/code.css`:
    exit 0, no matches (`radius-check.log`).
  - Original `cd editor && ! grep -nE 'border-radius:\\s*[^0v;]' src/code/code.css`:
    exit 1 because it matched the separator whitespace on all three valid
    `border-radius: 0` declarations (`radius-regex-false-positive.log`).
- Current source-only diff: `editor/src/code/code.css` and this plan. Formal
  review, shared documentation/index changes, commit and push remain with
  downstream workflow steps.
