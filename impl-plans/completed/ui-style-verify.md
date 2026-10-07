# UI Style: Static Token Check and Playwright Style Test Implementation Plan

**Status**: Completed
**Plan ID**: UI-STYLE-VERIFY (wave 2)
**Design Reference**: design-docs/specs/design-ui-style.md, sections 8.1, 8.2 and 8.3, plus 4.6 (tight groups) and 5 (coarse sizing)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05 (session-276 implementation and verification)

Session-276 note: the three residual items from the session-275 review are
settled by design 8.1 and 8.2. They are white/black word boundaries, the
`.pkg-proxy` element hidden on the native tier, and the Proxy input radius
and fill. This plan only tests them. It makes no CSS change.

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
- Match a named color only as a whole word inside the value (design 8.1).
  The characters on both sides must be neither a letter, a digit, `_` nor
  `-`, or must be the start or end of the value. Do not use plain JS `\b`
  for this, because `\b` treats `-` as a boundary. So `var(--vt-border)`,
  `--vt-danger-bg` and `blackout` never match, while `1px solid black`
  does.
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
- A sheet containing `outline: 1px solid black` -> check 2 fails.
- A sheet containing `border: var(--vt-border-w) solid var(--vt-border)` ->
  check 2 does not fail.
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
  11. `label.pkg-proxy` with the `hidden` attribute, wrapping the text
      `Proxy` and an `input[type=url]`. It copies the native-tier markup from
      `editor/src/pkg/pkg-view.tsx:24`. The served app is the browser tier
      (no `?session=`; `editor/src/app/main.ts:tierFromUrl`), so this
      fixture is the only way to test the native tier. Do not add
      `data-pkg="proxy"` to it, so no app code can pick it up.

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
7. **Packages Proxy (design 8.2 assertion 7).**
   - Browser tier: select the real input with
     `.pkg-proxy:not([data-style-probe] *) input[type=url]`. Equivalently,
     take the `.pkg-proxy input` that is not inside the probe.
     - It must exist and be visible. If it is absent or not visible, record
       a failure (`check: 'proxy-present'`), not a skip.
     - All four computed corner radii are `0px`.
     - `backgroundColor` equals the probe `input[type=text]`'s
       `backgroundColor`, which is the `--vt-bg-sunken` fill.
     - `borderTopWidth` is `1px`.
     - In viewport B, the bounding height is at least 36.
   - Native tier: the probe `label.pkg-proxy[hidden]` computes `display`
     `none`. That is the `[hidden] { display: none !important }` rule
     (`app.css:10`) beating `.pkg-proxy { display: flex }` (`pkg.css:20`).
     The hidden label and its input fail the visibility rule, so checks 2-5
     skip them on their own. Do not special-case them.
   - Record `proxy: {present, visible, radius, background, borderTop,
     height, nativeHiddenDisplay}` per engine and viewport in `report.json`.

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
- `.pkg-proxy input` would also match the hidden probe fixture. Always
  exclude `[data-style-probe]` descendants when selecting the real Proxy
  input.
- Do not "fix" a failing Proxy assertion by editing `pkg.css` or
  `pkg-view.tsx`. Report it per the Non-Goals rule.

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

- [x] `style-tokens.test.ts` is added and passes in the full vitest run (6 tests).
- [x] `ui-style.mjs` is added and passes `node --check`.
- [x] The `test:style` script is added as the only package script change; dependencies are unchanged.
- [x] Negative controls are run and reverted, with evidence logged.
- [x] The browser run is recorded: exit code, `report.json`, all four engine/viewport screenshots, `ui-full.png`, and coarse/file-pseudo notes per engine and viewport.
- [x] Check 2 uses whole-word named-color matching; `black` fails and `var(--vt-border)` passes in separate mutation runs.
- [x] `ui-style.mjs` contains probe child 11 and assertion 7; `report.json` carries the `proxy` object for every engine and viewport.

## Progress Log

Edit only this plan's log.

### Session: 2026-10-05 — session-276 UI-STYLE-VERIFY implementation

- Implemented the six static token checks and silent Chromium/WebKit harness. `editor/package.json` gains only `test:style`; no dependency or existing test changes.
- Source SHA-256: `editor/test/ui/style-tokens.test.ts` `261be5c857951aefbfbe8467bb17f45f4c334db68ff8b4a6bb92fb698b4bbe82`; `editor/test/style/ui-style.mjs` `0c31d8b10651789b5aa29068654ebf8ffcef9239696b51071b1dd7c3de3834bb`; `editor/package.json` `d36c7cf11e5027c49df404210a5d5fa1db874d7f6c7a1ddb99988c8bda8b8ea5`.
- Vitest baseline from the accepted wave-1 run: 90 files, 696 tests. Final full run with `./node_modules/.bin/vitest run --maxWorkers=1`: 91 files, 702 passed, exit 0; log `tmp/ui-style/UI-STYLE-VERIFY/attempt-1/vitest-serial-final-source.log`. The default parallel command was attempted three times and produced worker-load timeouts and a transient fake-clock assertion (complete logs `tmp/ui-style/UI-STYLE-VERIFY/attempt-1/vitest.log`, `tmp/ui-style/UI-STYLE-VERIFY/attempt-1/vitest-final-rerun.log`, and `tmp/ui-style/UI-STYLE-VERIFY/attempt-1/vitest-final-source.log`); the failed files passed in isolation (logs `focused-main.log`, `focused-first-track.log`, `focused-syntax-core.log`, `focused-syntax.log`). No failure remains on the final serialized full suite.
- Final gating commands: `npm run check` exit 0 (`npm-check-final.log`); `npm run build` exit 0 (`npm-build-final.log`); `node --check test/style/ui-style.mjs` exit 0 (`node-check-final.log`); `npm run test:style` exit 0 (`npm-test-style-final-source.log`); `node ../tmp/ui-style/shot.mjs ../tmp/ui-style/after` exit 0 (`shot-harness-final.log`).
- Browser report: `tmp/ui-style/after/report.json`, zero failures. Chromium and WebKit each passed 1440x900 and 1180x820 coarse; coarse mode was native and file pseudo was checked for all four runs. The real Proxy input was present/visible with four `0px` radii, `rgb(11, 13, 16)` fill, `1px` top border, and height 28px fine / 40px coarse; hidden probe display was `none`. Screenshots: `chromium-1440x900@2x.png`, `chromium-1180x820-coarse@2x.png`, `webkit-1440x900@2x.png`, `webkit-1180x820-coarse@2x.png`, plus `ui-full.png` from the screenshot harness.
- Negative controls (non-gating): `.pkg-pane { border-radius: 4px }` failed static check 1 (1 failed, 5 passed), rebuilt successfully, and failed browser radius assertions; `pkg.css` was restored to its pre-mutation SHA-256 `625427d49dafc54e2ab694aadf2da2eccd4da2bf6f6ac6a97360efd974e38be5` (`mutation-radius-vitest.log`, `mutation-radius-build.log`, `mutation-radius-playwright.log`, pre/post hash files). `color: black` failed check 2; tokenized `border: var(--vt-border-w) solid var(--vt-border)` passed (mutation logs). Swapping theme/app stylesheet links failed check 5 and restored the exact original `index.html` hash (mutation-index-order log and pre/post hash files).
- Browser interactions remained probe-only; sample panels were opened and reset by property. Chromium launched with `--mute-audio`; no app audio, transport, run, or song controls were activated.

### Session 276 gate rerun — review finding IR-VERIFY-GATE-EVIDENCE

- Readiness: `UI-STYLE-SHELL` is an accepted dependency. This was an evidence-only rerun; no test, harness, package, CSS, TS, or TSX source was edited.
- Before the checks, `git hash-object` was recorded for the three VERIFY source files and eleven wave-1 source files in `tmp/ui-style/UI-STYLE-VERIFY/attempt-2/hash-pre.txt`. After verification, the matching hashes were recorded in `hash-post.txt`; `cmp` confirmed equality. The VERIFY hashes remain `552f4353f6a2382074ce33ac490faa540b5a14f0`, `5b0691d171d18ab9de2bba67daa617da8409b7f9`, and `c94ff1b9560306692b1c2c81bca3365daaa1a360`.
- `cd editor && npm run check`: exit 0; complete log `tmp/ui-style/UI-STYLE-VERIFY/attempt-2/npm-check.log`.
- Gate evidence: `cd editor && ./node_modules/.bin/vitest run` with default workers passed on attempt 1, 91 files and 702/702 tests; exit 0. Complete log `tmp/ui-style/UI-STYLE-VERIFY/attempt-2/vitest-default-1.log`. No failing retry occurred in this rerun. The earlier failed attempt-1 logs remain preserved and list only known pre-existing timeout/fake-clock failures; the prior serial 702/702 run is supplementary evidence, not the gate.
- `cd editor && npm run build`: exit 0; complete log `tmp/ui-style/UI-STYLE-VERIFY/attempt-2/npm-build.log`.
- `cd editor && node --check test/style/ui-style.mjs`: exit 0; complete log `tmp/ui-style/UI-STYLE-VERIFY/attempt-2/node-check.log`.
- Browser evidence remains source-matched because the three VERIFY source hashes above are unchanged: `npm run test:style` exited 0 (`tmp/ui-style/UI-STYLE-VERIFY/attempt-1/npm-test-style-final-source.log`), and the screenshot harness exited 0 (`tmp/ui-style/UI-STYLE-VERIFY/attempt-1/shot-harness-final.log`). The report and all required screenshots are recorded above.
- The exact default-worker suite now passes on the final source. Earlier attempt-1 failures and their logs are retained as historical flaky-run evidence, not unresolved gate failures.


### Session: 2026-10-05 operator closeout (after session 276)
**Tasks Completed**: Operator re-verification of UI-STYLE-VERIFY on the final source, replacing the blocked branch gate result.
**Evidence**: `cd editor && npm run check` exit 0; `npm run build` exit 0; `npm run test:style` exit 0 (style assertions passed for Chromium and WebKit at 1440x900 and 1180x820 coarse); default `vitest run` results recorded in tmp/ui-style/operator-vitest-{1,2,3}.log. Four syntax/wasm suites timed out once under load average about 134 caused by a concurrent canvas workflow run and passed 21/21 in isolation; the default suite was rerun after the load dropped.
**Notes**: Integration review accepted UI-STYLE-SHELL, UI-STYLE-BIND-PARAMS, UI-STYLE-CODE and UI-STYLE-PANELS. Follow-up F1 (canvas renderer palette mapping to --vt-syn-* tokens) stays open for after the wf/canvas merge.
Default vitest operator runs: run 1 exit 1 (698/702; timeouts in syntax-core, syntax, first-track and the main.test.ts audible-probe timing test while load average was about 47); run 2 exit 0 (702/702); run 3 exit 0 (702/702). Logs: tmp/ui-style/operator-vitest-{1,2,3}.log. The load-sensitive timeouts are pre-existing tests outside this redesign; the main.test.ts probe test should move to fake timers (canvas CLOCK owner follow-up).
