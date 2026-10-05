# UI Style: Tokens, Base Rules and Shell Implementation Plan

**Status**: In Progress (session-276 rerun: verification only)
**Plan ID**: UI-STYLE-SHELL (wave 1)
**Design Reference**: design-docs/specs/design-ui-style.md, sections 3 (including 3.1 Build output), 4.1-4.3, 4.5 (transport, side bar, section headers, status bar), 5 and 8.3
**Created**: 2026-10-05
**Last Updated**: 2026-10-05 (session 276 amendment)

## Session-276 Rerun (read this first)

The source work in this plan is already implemented on `wf/ui-style` at
`364889b`. The only blocker was two gates that fail on correct code (two
stylesheet links in `dist/index.html`, and a BSD-incompatible `\s` radius
grep). Both are amended below. This rerun re-verifies; it does not
re-implement.

1. Before anything else, record `git hash-object` for the five owned source
   files and the six watched sheets. The expected owned hashes are the
   post-edit column in the progress log (`theme.css` `ed05d7a7...`,
   `index.html` `d027edf2...`, `app.css` `b9f175a8...`, `song.ts`
   `4e50eb04...`, `midi/mount.ts` `de7b6332...`). The watched sheets must
   equal the accepted hashes in `tmp/ui-style/s275-integration-review.json`
   (`waveAcceptanceRecord.currentHashes`).
2. Run every gating command in "Verification" below, each with its full log
   in `tmp/ui-style/UI-STYLE-SHELL/attempt-2/`.
3. If every command passes, make no source edit. Tick the session-276
   completion boxes, set Status to `Completed`, and append a session-276
   progress entry (commands, exit codes, log paths, hash table).
4. Edit a source file only if a gate fails on the current tree. In that case,
   fix only within this plan's five owned source files, record the pre and
   post hashes, and explain the failure in the log.

Do not touch the three accepted plans' sheets, `vite.config.ts`, the
dispatch manifest, or any protected file.

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
| `npm run build` | Exit 0 |
| Source-order check (below) | Exit 0. Prints `source-order <t> <a>` with `0 <= t < a`. |
| Built-order check (below) | Exit 0. Prints `built-order <asset> <a> <b>` with `0 <= a < b`. |

Use the exact commands from the UI-STYLE-SHELL `verification` list in
`impl-plans/active/ui-style-dispatch.json`. They are read-only node one-liners:

- Source order: reads `index.html`, takes `indexOf('./src/app/theme.css')`
  and `indexOf('./src/app/app.css')`, and exits 1 unless theme comes first.
- Built order: finds the single CSS file in `dist/assets/` that contains
  `--vt-bg`. It exits 1 if there is not exactly one, or unless
  `indexOf('--vt-bg')` is at least 0 and less than
  `indexOf('.vact-icon-button')`.

Vite merges the two `index.html` sheets into one hashed asset, so
`dist/index.html` has exactly one stylesheet link (design 3.1). That is
correct. Do not count links, and do not change `index.html`,
`vite.config.ts` or any CSS to produce two links.

Mechanical checks (call `/usr/bin/grep` explicitly; plain `grep` may be
ugrep here, and BSD grep has no `\s`):

| Command | Must show |
|---|---|
| `! /usr/bin/grep -nE '#[0-9a-fA-F]{3,8}\b\|rgba?\(\|hsl' src/app/app.css` | Exit 0, no output |
| `! /usr/bin/grep -nE 'border-radius:[[:space:]]*[^[:space:]0v;]' src/app/theme.css src/app/app.css` | Exit 0, no output |
| `! /usr/bin/grep -nE 'system-ui\|sans-serif\|monospace' src/app/app.css` | Exit 0, no output |
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

- [x] `theme.css` is created with every listed token and the coarse block.
- [x] `index.html` links `theme.css` before `app.css`.
- [x] `app.css` holds the base rules, variants, icon buttons, transport
  groups, headers and status bar, and is token-only.
- [x] `song.ts` and `midi/mount.ts` class edits are made.
- [ ] Session 276: the source-order and built-order checks exit 0 on a fresh
  `npm run build` (replaces the withdrawn two-link criterion, amendment
  IR-SHELL-DIST-LINK).
- [ ] Session 276: the amended mechanical checks exit 0, and the current
  owned-file hashes are recorded in this log.
- [x] The gating build, type check and full Vitest suite exit 0; their logs
  and the emitted CSS order evidence are recorded below.
- [x] The ownership check passes and its hash table is in the progress log.

## Progress Log

Edit only this plan's log.

### Session: 2026-10-05 — Step 6 implementation

- Implemented the token layer, global zero-specificity form-control rules,
  shell and status styling, and the assigned Apply song / Enable MIDI primary
  classes. DOM order is unchanged. No files owned by sibling plans changed.
- Baseline Vitest: 90 files, 696 tests passed. Final Vitest: 90 files, 696
  tests passed (not below baseline).
- Pre-edit and post-edit ownership hashes (`git hash-object`):

  | Path | Pre-edit | Post-edit |
  |---|---|---|
  | `editor/src/app/theme.css` | absent | `ed05d7a7a7083d0e253eaa080f48aceae1aa4cf7` |
  | `editor/index.html` | `a5c317e405af39166809f6463794c72a8041ccb4` | `d027edf25c0b03bf8eae606de7ba29c86313acff` |
  | `editor/src/app/app.css` | `959e75b7bff9bf00b2e3c25454966fdc1a0264a6` | `b9f175a88968e7ba0982f04840d8a420cb1f5793` |
  | `editor/src/app/song.ts` | `2b1da647df7e4beda98d6d15e4a496bfb33efd9b` | `4e50eb04d8695d5991a7ca66209260e2f84e9733` |
  | `editor/src/midi/mount.ts` | `5f7019c7bce22dd4b2bd0bc4ba0f3e887b3c7fdb` | `de7b6332e367f5e8d55d52df24df165f0d447db2` |
  | `impl-plans/active/ui-style-shell.md` | `b4fdcaa67fe69faff7a95ed1a47b0281c41de22d` | captured after this log update in `tmp/ui-style/evidence/ui-style-shell/hash-post-final.txt` |
  | `editor/src/bind/bind.css` | `e8160f4aeb36a14d4ef328d803156b7480203126` | `e8160f4aeb36a14d4ef328d803156b7480203126` |
  | `editor/src/code/code.css` | `b763b6600b762e5cbbc66d42a45e0707b332ace3` | `b763b6600b762e5cbbc66d42a45e0707b332ace3` |
  | `editor/src/midi/midi.css` | `056994fec450fb50cb7366a80350e586680ce6ca` | `056994fec450fb50cb7366a80350e586680ce6ca` |
  | `editor/src/params/params.css` | `0d47bbf2f3d1462f4d307875e51fa49625202aa7` | `0d47bbf2f3d1462f4d307875e51fa49625202aa7` |
  | `editor/src/pkg/pkg.css` | `9e7d71ffaa5987af4de5334b2531d1683a337917` | `9e7d71ffaa5987af4de5334b2531d1683a337917` |
  | `editor/src/visual/visual.css` | `52d2ba6d19b67dffa5b0c23bbedfc4242cf1b800` | `52d2ba6d19b67dffa5b0c23bbedfc4242cf1b800` |

  Every changed source path is owned by `UI-STYLE-SHELL`; watched paths are
  unchanged. Pre-edit table is also preserved at
  `tmp/ui-style/evidence/ui-style-shell/hash-pre.txt`; the earlier
  pre-audit snapshot remains at
  `tmp/ui-style/evidence/ui-style-shell/hash-post-before-progress.txt`. The
  final audited table, including the plan-file hash after this log update, is
  captured at `tmp/ui-style/evidence/ui-style-shell/hash-post-final.txt`.
- Verification (all commands were run in the foreground):
  - `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` — exit 0; complete log: `tmp/ui-style/evidence/ui-style-shell/wasm-build-baseline.log`.
  - `cd editor && ./node_modules/.bin/vitest run` before edits — exit 0, 696/696 tests; log: `tmp/ui-style/evidence/ui-style-shell/vitest-baseline.log`.
  - `cd editor && npm run check` on final source — exit 0; log: `tmp/ui-style/evidence/ui-style-shell/npm-check-final.log`.
  - `cd editor && ./node_modules/.bin/vitest run` on final source — exit 0, 696/696 tests; log: `tmp/ui-style/evidence/ui-style-shell/vitest-final-audited.log`.
  - `cd editor && npm run build` on final source — exit 0; log: `tmp/ui-style/evidence/ui-style-shell/npm-build-final.log`.
  - Final CSS/token/source contract checks — exit 0; log: `tmp/ui-style/evidence/ui-style-shell/source-contract-final.log`.
  - Built CSS verification — exit 0: theme tokens begin at byte 0, app base rules at byte 1849, all six file/range pseudo selectors are present, and no empty `:where()` selector remains; log: `tmp/ui-style/evidence/ui-style-shell/built-css-final-evidence.log`.
  - Output link count — check exit 1: `dist/index.html` contains one link to Vite's bundled `assets/index-BXJd7dqC.css`, not two distinct stylesheet links; complete output: `tmp/ui-style/evidence/ui-style-shell/dist-stylesheet-order-final.log`.
- A read-only audit caught that the CSS optimizer removes pseudo-elements nested
  inside `:where()`. The source selectors now place pseudo-elements outside
  `:where()` and put state selectors on the origin control; the final build
  confirms the file and range rules survive minification.
- The production CSS preserves effective cascade order, but the plan's
  distinct-link assertion is not met because the existing Vite build bundles
  linked CSS. Keeping separate links would require build configuration or a
  different asset-loading approach; `editor/vite.config.ts` is outside this
  plan's `writePaths`. Request a checkpoint amendment or acceptance clarification
  before making that change. This is the sole implementation blocker; formal
  review, wave-2 style testing and workflow finalization remain downstream.

### Session: 2026-10-05 — session-276 plan-author amendment

- IR-SHELL-DIST-LINK: the two-link criterion is withdrawn. It is replaced by
  the source-order and built-order checks (design 3.1 and 8.3). The current
  build has one link and the built order holds: `index-BXJd7dqC.css` has
  `--vt-bg` at 6 and `.vact-icon-button` at 9969 (step-3 reviewer run).
- IR-MANIFEST-RADIUS-GREP: the radius gate is now
  `border-radius:[[:space:]]*[^[:space:]0v;]`, run with `/usr/bin/grep`.
- No source change is requested. The implementer appends the rerun entry
  below.
