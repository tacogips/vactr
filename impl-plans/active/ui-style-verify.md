# UI Style: Static Token Check and Playwright Style Test Implementation Plan

**Status**: Ready
**Plan ID**: UI-STYLE-VERIFY (wave 2)
**Design Reference**: design-docs/specs/design-ui-style.md, sections 8.1, 8.2 and 8.3, plus 4.6 (tight groups) and 5 (coarse sizing)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

## Related Plans

**Depends On** (all must be complete and green):

- `ui-style-shell.md`
- `ui-style-bind-params.md`
- `ui-style-code.md`
- `ui-style-panels.md`

The static check below fails until every wave-1 sheet is token-only, so this
plan must not start earlier.

## Intent and Context

The user's acceptance needs automated evidence that the redesign holds:

- Radius is 0 everywhere.
- There is no native widget look.
- Gaps between controls are at least 8px (4px in tight groups).
- Targets are at least 36px under a coarse pointer.
- After screenshots exist in Chromium and WebKit.

There are two layers:

1. A static vitest check that runs in the gating suite, inside the sandbox.
2. A Playwright script that renders the built app. Workers write and
   syntax-check it inside the sandbox, but the Opus verification step runs it
   outside the sandbox, because the Codex sandbox cannot bind
   `127.0.0.1`.

Repository facts:

- `tsconfig.json` has `"types": []` and includes `test/`, so there are no
  Node typings. Read files with the dynamic-import idiom in
  `editor/test/wasm/format.test.ts:readText` (`const spec: string = 'node:fs'`,
  a local structural type, and `proc.cwd()` as the editor root).
- `vitest.config.ts` includes `test/**/*.test.ts(x)` only, so a `.mjs` file
  under `test/style/` never runs in the gating suite. `allowJs` is false, so
  tsc does not check `.mjs`.
- `editor/test/e2e/serve.mjs` exports `startServer({ dist })`, which returns
  `{ origin, close }`, plus `editorRoot` and `repoRoot`. It is protected:
  import it, never edit it.
- `playwright` 1.62.1 is already a devDependency.
- `tmp/ui-style/shot.mjs` is the reference harness. It uses Chromium with
  `--mute-audio`, waits, and takes a screenshot.

## Non-Goals

- Do not edit any CSS, TS or TSX source file, any wave-1 file, or anything
  under `editor/test/e2e/`.
- Do not add dependencies and do not regenerate `package-lock.json`.
- Do not add hover or visual-diff baselines, and do not add a Firefox run.
- Do not modify existing tests.

If an assertion exposes a defect in a wave-1 file, record the offending
selector and value in the progress log and report it. Do not fix it here; the
owning plan's worker repairs it serially after the join.

## File-Level Changes

### 1. `editor/test/ui/style-tokens.test.ts` (new vitest file)

The header comment is `// @vitest-environment node`. Imitate
`test/wasm/format.test.ts` for file reading.

To list the CSS files, use `readdirSync` with `withFileTypes` and walk
recursively from `src/`. Do not hard-code the 7 files, so that a new sheet is
caught. Expect at least the 7 known files and assert that.

Strip `/* ... */` comments before matching.

| # | `it` block | What it asserts |
|---|---|---|
| 1 | Radius | Every `border-radius` and `border-*-radius` declaration value, trimmed, is `0` or `var(--vt-radius)`. |
| 2 | No color literals outside `theme.css` | See below. |
| 3 | No generic font keywords outside `theme.css` | `monospace`, `sans-serif`, `system-ui`, `ui-monospace` and `ui-sans-serif` appear nowhere. |
| 4 | `theme.css` shape | See below. |
| 5 | Load order | In `index.html`, the index of `./src/app/theme.css` is greater than -1 and less than the index of `./src/app/app.css`. |
| 6 | Reduced motion | Any file containing a `transition` or `animation` declaration also contains `prefers-reduced-motion`. |

Check 2 details:

- Parse declarations as `property: value`, and only for color-bearing
  properties: `color`, `background`, `background-color`, `border*`,
  `outline*`, `fill`, `stroke`, `box-shadow`, `caret-color`,
  `text-decoration*` and `-webkit-text-fill-color`.
- Reject a value that matches `#[0-9a-f]{3,8}`, `rgba?(`, `hsla?(`, or a
  bare named color word. Use a closed list: `white`, `black`, `red`,
  `green`, `blue`, `gray`, `grey`, `yellow`, `orange`, `silver`.
- Allow `transparent`, `currentColor`, `inherit`, `none` and `var(...)`.
- Do not scan selectors or other properties. `white-space` and the `canvas`
  type selectors must not trip this check (Step 3 finding).

Check 4 details:

- After comments are stripped, every top-level block is `:root { ... }` or
  `@media (pointer: coarse) { :root { ... } }`.
- Every token from the `ui-style-shell.md` list is declared. Embed the list
  as a constant array in the test.

Test cases (the situation, then the expected outcome):

- The current tree after wave 1 -> all 6 pass.
- A sheet containing `border-radius: 4px` -> check 1 fails, naming the file
  and value.
- A sheet containing `color: #fff` -> check 2 fails.
- A sheet containing `white-space: nowrap` -> check 2 does not fail.
- `index.html` with the links swapped -> check 5 fails.

Run the negative cases as separate, non-gating mutation runs (below). Do not
commit fixture copies.

### 2. `editor/test/style/ui-style.mjs` (new Playwright script)

**Imports.**

- `chromium` and `webkit` from `playwright`.
- `startServer`, `editorRoot` and `repoRoot` from `../e2e/serve.mjs`.
- `node:fs` and `node:path`.

**Output.**

- `outDir = path.join(repoRoot, 'tmp/ui-style/after')`; create it
  recursively.
- Write `report.json` there with the per-engine, per-viewport results and
  the `coarse-mode` and `file-pseudo` notes.

**Silence (hard rules).**

- Chromium launches with `args: ['--mute-audio']`.
- WebKit launches with no args, because the flag is unknown to WebKit.
- Never call `click`, `tap`, `dblclick` or `check`.
- Use the keyboard only for `Tab` inside the probe.
- Never touch macOS volume.

**Runs.** For each engine in Chromium and WebKit, use two contexts:

| Viewport | Size | Context options |
|---|---|---|
| A | 1440x900 | `deviceScaleFactor: 2` |
| B | 1180x820 | `deviceScaleFactor: 2`, `hasTouch: true`, `isMobile: true` |

`isMobile` is supported by Chromium and WebKit in Playwright. If context
creation throws for that option, retry without it and log this.

**Settle.**

- `page.goto(origin + '/')`.
- Wait for `.vact-transport` and `.pane-side`.
- Then wait for fonts: `document.fonts.ready`.
- Then `waitForTimeout(1000)`.

**Coarse mode (viewport B).**

- Evaluate `matchMedia('(pointer: coarse)').matches`.
- If false, collect from every `document.styleSheets` entry (inside
  try/catch) each `CSSMediaRule` whose `conditionText` includes
  `pointer: coarse`. Join their inner `cssRules` `cssText`, and
  `page.addStyleTag({ content })`.
- Record `coarse-mode: emulated|forced`.

**Screenshots.** Taken before the probe is added:

- `<engine>-1440x900@2x.png`
- `<engine>-1180x820-coarse@2x.png`

**Open the samples panel.** Do this after the screenshots and before the
probe and assertions:

- Run `page.evaluate` to set `open = true` on every `details.vact-samples`.
- Set the property only. Never click or tap the `summary`. `samples.ts` and
  `sample-view.tsx` have no toggle listener, so opening is silent and has no
  side effects.
- Record in `report.json`, per engine and viewport:
  - `samplesOpened`: the count of opened elements.
  - `sampleMapPresent`: whether `.vact-sample-map` is present.
  - `bankCount`: the number of `.vact-bank` elements.
- `.vact-sample-map` renders only in the browser tier with a sample library
  (`samples.ts` `browserLibrary`). Its absence is recorded, not a failure.
- When present, its controls go through every assertion below (gap, radius,
  appearance, coarse height). Do not exclude them.

**Probe.**

- Append `<div data-style-probe>` to `.pane-right` with `display: flex`,
  `flex-wrap: wrap` and `gap: 8px` as inline style. This is test-only DOM,
  not source CSS.
- Children, in order:
  1. `input[type=text]`
  2. `button` "probe"
  3. `button[disabled]`
  4. `button.vact-primary`
  5. `button.vact-icon-button`
  6. `select` with 2 options
  7. `input[type=url]`
  8. `label` wrapping `input[type=checkbox]` and text
  9. `input[type=range]`
  10. `input[type=file]`

**Visibility.** A control is visible when its bounding box is non-zero, its
computed `display` is not `none`, its `visibility` is not `hidden`, and the
element and every ancestor have opacity above 0. This excludes the canvas
input-bridge `textarea`.

**Assertions.** Collect failures as `{engine, viewport, check, selector,
value}`; never stop at the first one.

1. **Radius.** For `document.querySelectorAll('*')`, all four computed
   corner radii equal `0px`.
2. **Appearance.**
   - Every visible `button`, `select` and `input` computes `appearance` (or
     `webkitAppearance`) as `none`.
   - The probe `select` has a computed `backgroundImage` other than `none`.
   - File pseudo: `getComputedStyle(fileInput, '::file-selector-button')`. If
     `borderTopLeftRadius` is the empty string, record `file-pseudo:
     unsupported` for the engine. Otherwise assert `0px`, and assert that
     `backgroundColor` equals the probe button's.
3. **Native fill.**
   - Parse the computed `backgroundColor` of every visible `button` and
     `select` as rgb(a).
   - Skip fully transparent fills (alpha 0).
   - The sRGB relative luminance must be at most 0.5.
4. **Gaps.**
   - Take the visible controls (`button`, `select`, and `input` except
     checkbox; use the checkbox's `label` instead).
   - For each control `a`, find the nearest control `b` whose left edge is at
     or past `a`'s left, that is not `a`, and whose vertical overlap with `a`
     is at least half the smaller height. "Nearest" means the smallest
     `b.left - a.right` among candidates.
   - Required: `gap >= 8`, or `gap >= 4` when both share a closest ancestor
     matching `.vact-slot, .params-tabs, .params-xy-pick, .vact-eval-status`.
   - The required gap also means no pair overlaps horizontally.
   - Exclude controls inside `[data-style-probe]` from pairing with controls
     outside it.
   - Named check: the `[data-song-controls] [role=status]` left edge minus
     the `.song-apply` right edge is at least 8.
   - Named check, sample map (when present): the `.vact-sample-map button`
     left edge minus the `.vact-sample-map input` right edge is at least 8.
   - Named check, banks: for each visible `.vact-bank`, the left edge of its
     `> button` minus the right edge of its `> span` is at least 8.
5. **Coarse targets (viewport B only).** Every visible `button`, `select`,
   `input` other than checkbox, and the probe checkbox `label` has
   `getBoundingClientRect().height >= 36`.
6. **Focus ring.**
   - Call `.focus()` on the probe text input, then
     `page.keyboard.press('Tab')`.
   - The active element must be the probe button, with computed
     `outlineStyle` `solid` and `outlineWidth` `2px`.

**Teardown and exit.**

- Remove the probe.
- Set `open = false` again on the `details.vact-samples` elements opened
  earlier, again by property and never by click.
- Close the browsers and the server in `finally`.
- Print a summary line per engine and viewport.
- Set `process.exitCode = 1` when any failure exists, else 0.

### 3. `editor/package.json`

Add exactly one script: `"test:style": "node test/style/ui-style.mjs"`.
Do not touch dependencies or other scripts. Do not run `npm install`.

## Pitfalls

- Do not import Node typings or add `@types/node`. Follow the
  `format.test.ts` dynamic-import idiom, or `npm run check` fails.
- Do not place the script in `test/e2e/` (protected), and do not name it
  `*.test.mjs`, so that vitest never picks it up.
- WebKit rejects `--mute-audio`, so pass it only to Chromium.
- A raw `#` in a `--vt-icon-*` data URI belongs to `theme.css`. Check 2
  skips `theme.css`; do not "fix" it there.
- `getComputedStyle(el).appearance` may be empty in WebKit. Fall back to
  `webkitAppearance`.

## Invariants

- Existing tests are unchanged. The new vitest file adds 6 tests.
- The script never interacts with transport, audio, run or song controls.

## Setup (not gating)

If `target/wasm32-unknown-unknown/debug/vactr.wasm` is missing, run from the
repository root:

`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`

## Verification

### Gating, inside the sandbox (run from `editor/`)

| Command | Must show |
|---|---|
| `npm run check` | Exit 0 |
| `./node_modules/.bin/vitest run` | Exit 0. It includes `test/ui/style-tokens.test.ts` with 6 passing tests, and the total equals the baseline plus 6. |
| `npm run build` | Exit 0 |
| `node --check test/style/ui-style.mjs` | Exit 0 |

### Gating, outside the sandbox (Opus verification step, run from `editor/` after `npm run build`)

| Command | Must show |
|---|---|
| `npm run test:style` | Exit 0 |
| `node ../tmp/ui-style/shot.mjs ../tmp/ui-style/after` | Exit 0, with `tmp/ui-style/after/ui-full.png` written |

For `npm run test:style`, these files must exist:

- `tmp/ui-style/after/report.json`
- `tmp/ui-style/after/chromium-1440x900@2x.png`
- `tmp/ui-style/after/chromium-1180x820-coarse@2x.png`
- `tmp/ui-style/after/webkit-1440x900@2x.png`
- `tmp/ui-style/after/webkit-1180x820-coarse@2x.png`

### Negative controls (non-gating; report separately and revert immediately)

1. Temporarily set `border-radius: 4px` on `.pkg-pane` in `pkg.css`.
   - `./node_modules/.bin/vitest run test/ui/style-tokens.test.ts` must fail
     check 1.
   - After a rebuild, `npm run test:style` must report radius failures.
   - Restore the file and confirm its hash matches the pre-mutation hash.
2. Temporarily swap the two links in `index.html`. Check 5 must fail.
   Restore the file and confirm its hash.

## Completion Criteria

- [ ] `style-tokens.test.ts` is added and passes in the gating vitest run.
- [ ] `ui-style.mjs` is added and passes `node --check`.
- [ ] The `test:style` script is added.
- [ ] Negative controls are run and reverted, with evidence logged.
- [ ] The outside-sandbox run is recorded by the verification step: exit
  code, `report.json` path, the screenshot list, and the `coarse-mode` and
  `file-pseudo` notes per engine.

## Progress Log

Edit only this plan's log.

### Session: (implementer fills in)

- Baseline and final vitest counts.
- Hashes of the files created.
- Mutation-run outcomes.
- Command exit codes and log paths.
