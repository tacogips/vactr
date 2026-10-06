# Canvas Cutover OPT-DOM: Bind Panel Virtualization, Indexed Store, Incremental Roll and Lazy Accessibility Bridge Implementation Plan

**Status**: Completed (accepted by the session-288 integration review, commit 827b851; manifest acceptedDependencies since session 289; archived at CANVAS-EVIDENCE closeout)
**Plan ID**: CANVAS-OPT-DOM (session 286, wave 10; runs alone after CANVAS-OPT-HISTORY is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 1 (bind panel, store and DOM) and section 6 (budgets); design-docs/specs/design-ui-style.md (tokens, square controls); 15.1.6 (binding UI, one-repaint rule)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-DOM`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

WebKit, the primary target, renders at about 8 fps once about 7,400 bind-panel rows carry
`opacity: 0.7` (`editor/src/bind/bind.css:75-78`).

- The A/B tests in `tmp/canvas-cutover/diag-shape/REPORT.md` show this is the dominant
  WebKit cost. Overriding only that opacity gave 525 keys and frame p95 21 ms.
- `SliderPanel` (`editor/src/bind/panel.ts`) keeps every row in the DOM. `syncRows`
  (lines 171-200) re-appends every row on each sync.
- `Store.notify` (`editor/src/protocol/store.ts:294-316`) scans all, about 10,000,
  subscriptions per message.
- `PianoRoll.draw` (`editor/src/params/roll.ts:96-105`) builds new display objects each
  batch. Solid `<For>` (`params/telemetry-view.tsx:35`) then recreates every note span,
  22,912 DOM mutations in the diagnosis.
- `AccessibilityBridge.refresh` (`editor/src/code/accessibility.ts:57-75`) rewrites the
  8 KiB textarea value and forces layout on every keystroke. `InputController` calls it from
  its surface subscription (`input.ts:58-61`).

CANVAS-OPT-HISTORY (accepted before this plan) pins the evaluated revision, so the stale
flip no longer happens in normal editing. The panel must still stay cheap when many rows
are stale or unbound.

## Non-goals

- No change to binding semantics: `SiteTable`, `SiteWriter`, `CcRouter`, drag, learn,
  persistence and the directive control panel (`directives.ts`, `control-view.tsx`).
- No change to the one-repaint rule for mounted rows (15.1.6). It keeps the table
  subscription before the rows.
- No change to the visual design beyond the fixed row height, `nowrap` and ellipsis, with
  the full badge text in `title`. No radius, no color literal and no native widget look.
- No grid view (`GridView`) change. No code canvas, renderer, syntax or history work.
- No Rust, no dependency and no protocol change.

## Ownership

writePaths:

- `editor/src/bind/panel.ts`
- `editor/src/bind/panel-view.tsx`
- `editor/src/bind/bind.css`
- `editor/src/bind/mount.ts`
- `editor/src/protocol/store.ts`
- `editor/src/params/roll.ts`
- `editor/src/params/telemetry-view.tsx`
- `editor/src/code/accessibility.ts`
- `editor/src/code/input.ts`
- `editor/src/code/mount.ts`
- tests:
  - `editor/test/bind/panel.test.ts`
  - `editor/test/bind/reactive.test.ts`
  - `editor/test/bind/reconcile.test.ts`
  - `editor/test/bind/multisite.test.ts`
  - `editor/test/bind/fixtures.ts`
  - `editor/test/protocol/store.test.ts`
  - `editor/test/params/displays.test.ts`
  - `editor/test/canvas/input.test.ts`
  - `editor/test/canvas/edit-cost.test.ts`
  - `editor/test/canvas/mount.test.ts`
- `impl-plans/active/canvas-cutover-opt-dom.md` (checkboxes and progress log only)
- `tmp/canvas-cutover/opt-dom` (artifact root; also receives the non-gating behavior e2e
  output)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules/.vite`, `editor/dist`, `tmp/ui-style`

sharedPaths:

- `editor/src/app/theme.css`: no edit expected. Add a token only if a needed value has none,
  and record it.
- `editor/test/ui/style-tokens.test.ts`: no edit. It must keep passing.
- Harness sharedPaths (session-288 ownership amendment; design 15.3.8.14 section 8). The
  binding rules are in "Harness sharedPaths (session 288)" below:
  - `editor/test/e2e/behavior.mjs`
  - `editor/test/e2e/measure.mjs`
  - `editor/test/e2e/run.mjs`
  - `editor/test/e2e/stats.mjs`
  - `editor/test/e2e/stats.test.ts`
  - `editor/test/e2e/ios-sim.mjs`
  - `editor/test/e2e/README.md`
  - `editor/test/style/ui-style.mjs`

### Harness sharedPaths (session 288)

These rules are identical in CANVAS-OPT-DOM, -TEXT, -BACKDROP and -RENDER.

**Why.** Session 287 blocked this plan only because `editor/test/e2e/behavior.mjs` sat
outside its paths. The `canvas-only-text` check sampled the input bridge before the frame
that positions it.

**Allowed edit.** Only frame alignment of harness sampling, or documentation of that
alignment. Frame alignment means waiting for a presented frame (requestAnimationFrame),
then polling a bounded number of frames before reading DOM, canvas or perf state. The
reference is 842cf6e: two rAFs, then a poll bounded at 30 frames.

**Never change:**
- a check name, assertion, expected value or comparison;
- `THRESHOLDS` or `TARGETS` in `stats.mjs`;
- the workload or the fixtures;
- the silent-sink install or its post-sink-peak-0 and direct-connection-0 checks;
- `run.mjs` exit codes 0/1/2 or its wasm gating refusal;
- the `ios-sim.mjs` self-check predicate or its silent launch;
- the square and token checks in `ui-style.mjs`.

A harness change that is not frame alignment (for example a new metric reader) is not
allowed under this amendment. Record it as a blocker for a serial plan-author amendment.

**Recording.** For each edit, record in this plan's progress log the path, the reason, and
the fresh-read and post-edit sha256 values (also in `tmp/canvas-cutover/<plan>/intent.json`
and `receipt.json`). Record the check that motivated it, with its failing log path in the
notes.

**Check.** For every edited harness path, `git diff <HARNESS_BASE> -- <path>` removes or
changes no line containing `expect(`, `assert`, `THRESHOLDS`, `TARGETS` or a check's
pass/fail comparison. Reviewers verify this.

`HARNESS_BASE` is the plan's `START` commit. For CANVAS-OPT-DOM it is ff49fd1, so the
842cf6e repair is reviewed too. It is not the plan-base 1d17ced, because the accepted
OPT-HARNESS legitimately tightened `THRESHOLDS.textWorkP95Ms` after 1d17ced.

### Session 288 resume (CANVAS-OPT-DOM only)

**Source state.** The source is final at HEAD 842cf6e: product changes in ff49fd1, plus
the operator harness repair 842cf6e to `behavior.mjs` (frame alignment, 18 insertions and
3 deletions, assertions unchanged). Do not reimplement TASK-D1 to TASK-D5. Change source
only if a gate or a review finding exposes a defect, and then fix it inside this plan's
writePaths.

**Base.** `BASE=1d17ced` (the session-286 plan checkpoint) for the diff criteria. Record a
separate `START` (the session-288 plan checkpoint commit) in
`tmp/canvas-cutover/opt-dom/intent.json`.

**Gates.** Run every row of the Verification tables on the current HEAD, one heavy suite
at a time:
- default vitest;
- the behavior e2e (Chromium and WebKit, silent);
- `test:style`;
- `test:perf`, run alone with `--maxWorkers=1`;
- `npm run check`;
- strict clippy;
- full nextest, run alone with `timeout 2400`;
- the wasm32 build;
- the `editor/src-tauri` cargo check.

Report only final passing runs, in the mandatory record format.

**Behavior run expectation.** 18/18 checks (Chromium 10/10, WebKit 8/8), exit 0.

## Contracts and Key Points

### 1. CSS (`bind.css`)

- Lines 75-78: `opacity: 0.7` becomes `color: var(--vt-text-muted)` on the stale and unbound
  rows. The `.bind-state` danger color rule (lines 69-73) stays.
- Line 163, `.bind-overlay { opacity: 0.85 }`: delete the `opacity` declaration. No element
  uses that class (the binding label is canvas-drawn; verified with grep). After this,
  `grep -c opacity editor/src/bind/bind.css` is 0.
- Line 25: `.bind-group:not(:has(.bind-row))` becomes `.bind-group[data-empty] { display: none; }`.
  The same applies to `.bind-names` if it needs an empty rule.
- `.bind-row`: add `height: calc(var(--vt-control-h-compact) + 2 * var(--vt-space-1))`,
  `box-sizing: border-box`, `overflow: hidden` and `white-space: nowrap`. `.bind-form` and
  `.bind-name-badge` get `overflow: hidden` and `text-overflow: ellipsis`.
- Add a `.bind-spacer` rule with no padding and no border. Its height is set inline by the
  panel.

### 2. Virtualized `SliderPanel` (`panel.ts`, `panel-view.tsx`)

- *Model and DOM.* The model keeps the ordered binding ids per origin group and the ordered
  names. The DOM keeps only mounted rows. `rows: Map<id, Row>` and
  `nameRows: Map<name, ValueRow>` hold mounted rows only.
- *Group structure.* Each group element keeps
  `<h3>`, `.bind-spacer[data-edge=top]`, the mounted rows in order, and
  `.bind-spacer[data-edge=bottom]`. The spacer heights are
  `rowsBefore * rowHeight` and `rowsAfter * rowHeight`. Set `data-empty` on a group with 0
  model rows.
- *Viewport.* `PanelHost.viewport?(): { top: number; height: number }` gives the visible
  range in panel-content pixels. When it is absent, the panel measures in one rAF callback,
  reading all of these before any write:
  - the scroll container (`this.el.closest('.pane-right')`, else the nearest ancestor
    with overflow auto or scroll): `scrollTop` and `clientHeight`;
  - `this.el` top relative to the container;
  - each group's top-spacer `offsetTop`.

  The window per group is the model rows that intersect `[top - 8*rowHeight,
  top + height + 8*rowHeight]` (`PANEL_OVERSCAN_ROWS = 8`), counted from that group's rows
  origin. With the seam, every group origin is taken as the seam's coordinate space: the
  group origin is the sum of the earlier groups' `headerHeight + count*rowHeight`, where
  `headerHeight` comes from an optional seam field and defaults to 0 under the seam.
- *Row height.* Measure `offsetHeight` of the first mounted row once. Measure again on a
  `matchMedia('(pointer: coarse)')` change and on `document.fonts` `loadingdone`. A value of
  0 or less (jsdom) or no measurement means 32 px.
- *Updates.*
  - A passive `scroll` listener on the container and a `ResizeObserver` on the container
    only mark the window dirty and request one rAF. In that rAF, all reads run first, then
    all writes.
  - A data change (`rebuild`, `applyRefresh`, `renderIds`, `addNames`) recomputes the
    window synchronously only when a seam viewport is present. Otherwise it schedules the
    rAF, so that tests with the seam stay synchronous.
  - When the window changes, newly visible rows get a Solid root and a store subscription
    under their keys (the existing `createRow` and `subscribe` logic). Rows leaving the
    window are unsubscribed (`off`), disposed and removed. A row entering renders once from
    the current model.
- *Order.* Compute the order key per binding: `table.currentRange(e)?.from ?? MAX_SAFE_INTEGER`,
  which is today's sort (line 180). Compare the new ordered id array per group with the
  previous one. If it is equal, move 0 nodes. Otherwise insert each mounted row before the
  next mounted sibling in model order. Never re-append rows that are already in place.
- *Existing semantics kept for mounted rows.* `rebuild()`, `applyRefresh(r, batch)` (render
  once; rows whose keys hold render from their own subscription), `renderIds(ids)`
  (mounted ids only), `addNames()`, `renderCount(key)` (renders of mounted rows),
  `row(id)` and `nameRow(name)` (`undefined` when not mounted), `onRender`, `dispose()` (also
  cancels the rAF and disconnects the observers).
- *`panel-view.tsx`.* The `title` attribute holds the full `form` badge text. The stale
  label stays `STALE`.

### 3. Seam wiring (`bind/mount.ts`, `fixtures.ts`)

- `BindOptions.panelViewport?: () => { top: number; height: number; headerHeight?: number }`
  is passed through to `PanelHost.viewport`.
- `editor/test/bind/fixtures.ts` `setup()` passes `panelViewport: () => ({ top: 0, height: 1e9 })`,
  so all existing bind tests keep their assertions unchanged with every row mounted.

### 4. Indexed `Store.notify` (`store.ts`)

- *Index.* `private readonly index = new Map<string, Set<Sub>>()` and
  `private seq = 0`. `Sub` gains `seq`.
- *Subscribe and unsubscribe.* `subscribe` assigns `seq` and adds the sub to the set of each
  of its keys. The unsubscribe function removes the sub from those sets, deleting empty
  sets, and marks it inactive. Drop the O(n) `this.subs.filter` (keep `subs` only if
  `dispose()` needs it; prefer iterating `index`).
- *`notify(changed)`.* Collect the union of `index.get(k)` for `k` in `changed` into an
  array, de-duplicate it, sort it by `seq`, then call each sub that is still active. A sub
  subscribed during this notification has `seq` greater than the snapshot maximum and is
  skipped. An unsubscribed sub is inactive and skipped. Errors still go to `onError`.
- Add the test counter `stats.notifyVisits`, incremented per sub examined.

### 5. Incremental roll (`roll.ts`, `telemetry-view.tsx`)

- *Stable identity.* `PianoRoll` keeps `display: Map<string, RollDisplayNote>`, keyed by
  `` `${slot}|${pos}|${len}|${text}|${occurrence}` ``, where `occurrence` counts identical
  keys within the cycle. `draw()` reuses an existing object for an unchanged key. A new
  cycle clears the map.
- *Lane geometry.* It moves into a signal: `RollView` props gain
  `range: Accessor<{ hi: number; lanes: number }>`, and each note's `top` style is computed
  in the view from `note.pitch` and `range()`. A pitch-range change then updates styles in
  place, with no element recreation. `data-lane` stays a static attribute (`unpitched` or
  `undefined`).
- *Public surface.* `notes()` (the `RollNote` list) and `parsePitch` keep their behavior.

### 6. Lazy accessibility bridge (`accessibility.ts`, `input.ts`, `code/mount.ts`)

- *`AccessibilityBridge` API.*
  - `markDirty()` is O(1).
  - `flush(opts?: { position?: boolean })` writes only what changed:
    - window bounds and value unchanged: no `value` write;
    - same `start` and an edit inside the window: `textarea.setRangeText(insert, a, b, 'preserve')`
      for the changed range only. Compute it from the old and new window values by common
      prefix and suffix, bounded by the window size (8 KiB);
    - window moved: one full `value` write.

    After the value, it writes the selection only when the projected selection differs,
    and calls `position()` only when `opts.position` is true.
  - `refresh()` stays as `markDirty(); flush({ position: true })` for callers that need a
    synchronous refresh: composition finish (`input.ts:118`) and the trailing-composition
    paths (`:124`, `:147`).
- *`InputController`.*
  - The surface subscription (`input.ts:58-61`) calls `markDirty()` instead of
    `refresh()`.
  - Before the handler body of `keydown`, `beforeinput`, `compositionstart`, `select`,
    `selectionchange` (on the document, only while the textarea is the active element),
    `copy`, `cut`, `paste` and `focus`, call `flush({ position: false })` when dirty. Today
    `input.ts` listens to `select` only. Add the `selectionchange` pre-flush listener; it
    reads nothing else and is removed on dispose.
  - Never flush inside the `input` handler or the `compositionupdate`/`compositionend`
    handlers. `input` must see the native mutation and compare it with the last flushed
    window.
  - Expose `flushBridge()` for the frame.
  - The `window` `resize`/`scroll` and `visualViewport` position listeners only set a
    `positionDirty` flag.
- *Frame.* `code/mount.ts` `onFrame` calls `input.flushBridge()` once at frame start, after
  `viewHost.refreshRect()`. It runs `flush({ position: true })` only when the bridge is
  dirty or `positionDirty` is set. `position()` uses `surface.coordsAtPos`, which reads the
  cached rect (15.3.8.13 item 3), so the frame performs no extra layout read.

### Patterns to imitate

- `panel.ts` `createRow`/`subscribe` for the row lifecycle.
- `store.ts` `drainChanges` for the notification re-entrancy guard.
- `test/canvas/edit-cost.test.ts` for counting rigs.
- `test/bind/reactive.test.ts` rows 60-120 for render-count assertions.

## Tasks

### TASK-D1: CSS
### TASK-D2: Virtualized panel and seam (`panel.ts`, `panel-view.tsx`, `bind/mount.ts`, `fixtures.ts`)
### TASK-D3: Indexed store
### TASK-D4: Incremental roll
### TASK-D5: Lazy accessibility bridge and frame flush
### TASK-D6: Tests, behavior e2e, verification and progress log

Each task's completion criterion is its contract plus its test rows, passing.

## Test Cases

`editor/test/bind/panel.test.ts` (new `describe('virtualization')`, using `setup()` with an
explicit `panelViewport`):

- 10,000 `binding`-origin sites, viewport `{ top: 0, height: 600 }`, row height falling back
  to 32 -> mounted rows `<= ceil(600/32) + 16` (35). `panel.row(firstId)` is defined and
  `panel.row(lastId)` is undefined.
- Viewport moved to `{ top: 160000, height: 600 }` -> the rows around index 5,000 are
  mounted; the first row is unmounted and its store subscription is released (a `name`
  batch for it renders 0 times).
- All 10,000 rows turned stale through `writer.onStale` or a refresh -> mounted rows stay
  `<= 35`. No mounted row has an inline or computed `opacity`
  (`getComputedStyle(row).opacity` is `''` or `'1'` in jsdom; also assert that the
  stylesheet text contains no `opacity`).
- A refresh with unchanged order -> 0 calls to
  `insertBefore`/`appendChild`/`append` on group elements (spy on
  `HTMLElement.prototype`). Control: reverse the order of two sites -> moves `> 0`.
- An empty group -> `data-empty` present and no `:has(` in `bind.css`.

`editor/test/bind/reactive.test.ts`, `reconcile.test.ts`, `multisite.test.ts`: unchanged
assertions; they pass through the fixture seam.

`editor/test/protocol/store.test.ts`:

- 10,000 subs, one per key `name:k<i>`, plus one `['sites']` sub; apply a message changing
  `name:k7` -> `stats.notifyVisits === 1` and exactly that callback runs. Control: a message
  changing 3 keys -> 3 visits.
- Subs A (`['x']`, registered first) and B (`['y']`); a change of `{y, x}` -> call order A
  then B.
- A sub that subscribes a new sub during a callback -> the new sub is not called in that
  notification. A sub unsubscribed by an earlier callback -> not called.

`editor/test/params/displays.test.ts`:

- Two batches in the same cycle, the second adding 3 notes -> 3 new `.params-roll-note`
  elements; the elements from the first batch are the same objects (`toBe`).
- A batch that widens the pitch range -> the same elements, with updated `style.top`.
- The existing lane test (`:a4`, `60`, unpitched) is unchanged.

`editor/test/canvas/input.test.ts` and `edit-cost.test.ts`:

- A single-character edit inside the window -> 0 assignments to `textarea.value`. Spy on the
  value setter with
  `Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value')`, and spy on
  `setRangeText`. Count 0 `getBoundingClientRect` calls in the keystroke task.
  `input.flushBridge()` then calls `setRangeText` once.
- A caret move inside the window -> 0 `value` writes and 0 `setRangeText` calls; the
  selection is updated on flush.
- An unflushed edit followed by a `beforeinput` -> the textarea value equals the new window
  before the handler runs (control: without the pre-handler flush the test would see the
  old value; assert both states inside one test).
- The existing IME composition, clipboard and accessibility rows are unchanged.

## Pitfalls

- *Order of reads and writes.* Never interleave DOM reads and writes in the window update.
  Read everything first.
- *Seam scope.* Do not make the panel depend on `getBoundingClientRect` in jsdom paths. The
  seam must fully determine mounting in tests.
- *Store order.* Keep the registration order across keys. The bind table subscription must
  run before the rows (`bind/mount.ts:129-145`).
- *No partial repaints.* Do not dispose a row while its own subscription callback is
  running (re-entrancy). Defer unmounts to the window update.
- *`setRangeText` and selection.* `setRangeText` with `'preserve'` changes the selection.
  Always re-apply the projected selection after a value change.
- *`input` handler.* Do not flush in it; that would erase the native mutation that
  `input.ts:144-170` reconciles.
- *Tokens only.* Keep CSS on tokens. `style-tokens.test.ts` forbids color literals.
- *Grid.* Do not change `GridView`.

## Verification

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`;
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
Record `BASE` and the fresh-read sha256 values in `tmp/canvas-cutover/opt-dom/intent.json`.

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/bind test/protocol/store.test.ts test/params/displays.test.ts test/canvas/input.test.ts test/canvas/edit-cost.test.ts test/canvas/mount.test.ts` | exit 0; the new rows are listed |
| `cd editor && ./node_modules/.bin/vitest run test/ui` | exit 0 (static token test) |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `grep -c opacity editor/src/bind/bind.css` and `grep -c ":has(" editor/src/bind/bind.css` | 0 and 0 (notes only) |

Outside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `mise run build-wasm-release` then `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | exit 0 (setup for the browser rows) |
| `cd editor && npm run test:style` (runs `node test/style/ui-style.mjs`; writes `tmp/ui-style/after`) | exit 0; square and token checks pass in Chromium and WebKit |
| `cd editor && npm run e2e -- --browser all --profile behavior --run-id s286-opt-dom --out ../tmp/canvas-cutover/opt-dom/behavior` | exit 0 (non-gating for evidence; behavior checks pass; no `--write-evidence`) |
| `cd editor && npm run test:perf` (alone) | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |

Use the session-286 record format: `exitStatus: 0`, `outcome: "passed"`, `testsRun > 0` and
`failureCount: 0` for tests, details in `notes`. Run no mutation or negative-control
command.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/opt-dom/intent.json`
and `receipt.json`. If a file drifted without an edit from this plan, stop editing it and
report. Edit only this plan's progress log.

## Completion Criteria

- [x] `bind.css` has no `opacity` and no `:has(`; stale and unbound rows are dimmed by `--vt-text-muted`; fixed token-derived row height
- [x] Mounted rows `<=` viewport rows plus 2x8 overscan, including after the mass stale transition; 0 node moves when order is unchanged
- [x] The production no-seam viewport follows `.pane-right` scrolling; the non-seam rAF path works without `matchMedia`
- [x] A non-seam eval-result render-all set is retained across a same-frame bindings refresh; each affected mounted row renders once
- [x] The fixture seam keeps every existing bind assertion unchanged
- [x] Store notify visits only the subs of changed keys, in registration order
- [x] The roll reuses elements across batches in a cycle and updates lanes in place
- [x] Accessibility bridge: 0 full value writes and 0 rect reads in the keystroke task; flush before input-reading handlers; one frame flush
- [x] `ui-style.mjs`, the style-token test and the behavior e2e pass in both browsers
- [x] Any harness sharedPath edit is frame alignment only and is recorded with its sha256 values; `git diff ff49fd1 -- editor/test/e2e editor/test/style` touches only `behavior.mjs` (the 842cf6e frame wait) and changes no assertion, threshold or check comparison
- [x] Default vitest, `npm run check`, `test:perf`, clippy, nextest, wasm32 build and src-tauri check pass
- [x] Progress log updated

## Progress Log

### Session: 2026-10-06 (session 286 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 1.
**Notes**: Wave 10, after HISTORY. The `.bind-overlay` opacity was named explicitly
(step3 review finding, low).


### Session: 2026-10-06 (session 286 implementation handoff)
**Tasks Completed**: TASK-D1 through TASK-D5 and focused regressions in TASK-D6.
**Notes**: Focused bind/store/roll/input/mount tests passed 161/161 (`tmp/canvas-cutover/opt-dom/focused-final.log`); UI tests passed 18/18 (`ui-final.log`); npm check passed (`check-final-rerun.log`); full vitest passed 766/766 before the final bridge input reconciliation (`vitest-full-final.log`); the post-change focused bridge suite passed 74/74 (`focused-repair5.log`). Style passed in Chromium and WebKit (`style-final.log`); release wasm and ABI frontend build passed (`wasm-release-final.log`, `frontend-build-repair2.log`). The behavior e2e still fails only `canvas-only-text` in both touch-enabled browsers (16/18 checks passed; see `e2e-behavior-retry2.log`): the bridge rectangle is sampled before the next canvas frame positions it, yielding a 1 px glyph sample. Fixing the test's frame synchronization requires editing `editor/test/e2e/behavior.mjs`, outside this plan's writePaths. Resume after a plan/manifest checkpoint authorizes that path or assigns the test update to its owner. Full aggregate/perf/native cargo gates remain pending because implementation is blocked at this required browser verification.

### Session: 2026-10-06 (session 288 plan amendment)
**Tasks Completed**: Ownership amendment only (design 15.3.8.14 section 8, "Harness
sharedPaths").
**Notes**:
- The 8 harness and style files are now concrete sharedPaths of this plan and of OPT-TEXT,
  OPT-BACKDROP and OPT-RENDER, here and in `impl-plans/active/canvas-cutover-dispatch.json`.
- Operator repair 842cf6e fixed the `canvas-only-text` frame wait in `behavior.mjs`
  (silent behavior e2e 18/18, assertions unchanged).
- Shared-path hash record for that frame-only adjustment: fresh read at `ff49fd1`,
  `3a03ac897a0ade62d19c2318cf1b4de96275d2dbbe68c36d547dd72541706f2a`; post-edit at
  `842cf6e` and current HEAD, `f9fbd8b50453e9d8993b3847c95208cb21e088aac57c79d2ed69464daad7b0bc`.
- Next: run the session 288 resume gates, then the test-integrity, adversarial and
  integration reviews. Scope, contracts and criteria are otherwise unchanged.

### Session: 2026-10-06 (session 288 final-source verification)
**Tasks Completed**: TASK-D6 final-source verification and progress record.
**Notes**:
- Focused bind/store/roll/input/mount suite: 161/161 (`focused-session288-final.log`);
  UI token suite: 18/18 (`ui-session288-final.log`); default vitest: 766/766
  (`vitest-full-session288-final.log`); `npm run check` exit 0
  (`check-session288-final.log`). The serial performance gate passed 1/1 with
  20k/5k ratio 2.80 (`test-perf-session288-final.log`).
- Release WASM and ABI frontend build passed (`wasm-release-session288-final.log`,
  `frontend-build-session288-final.log`). Browser style assertions passed in four
  Chromium/WebKit viewport combinations (`style-session288-final.log`). The behavior
  profile passed 18/18: Chromium 10/10 and WebKit 8/8
  (`e2e-behavior-session288-final.log`). WebKit clipboard and IME checks remain labeled
  synthetic-event limitations by the harness.
- Strict clippy and Tauri cargo check exited 0; full nextest passed 2816/2816 with
  3 configured skips, exit 0 after 1683.857 seconds; the standalone wasm32 build exited
  0 (`clippy-session288-final.log`, `nextest-session288-final.log`,
  `tauri-check-session288-final.log`, `wasm32-session288-final.log`).
- `grep -c opacity` and `grep -c ':has('` both returned 0 for `bind.css`. The shared
  harness diff from `ff49fd1` touches only `behavior.mjs` and aligns sampling to a
  presented frame; no check assertion or pass/fail comparison was changed.
- The implementation plan is complete for this step. Test-integrity, adversarial and
  integration reviews remain downstream workflow steps.

### Session: 2026-10-06 (OPTDOM-TI-01 test-integrity repair)
**Tasks Completed**: Corrected production viewport coordinates, guarded optional
`matchMedia`, added a fixture seam opt-out and a no-seam scroll regression.
**Notes**:
- `editor/src/bind/panel.ts`: fresh-read SHA-256
  `9903f80d71ab0f81f476e92a9ed0169e3c1add9b880c6b811a4c02ac746c3282`; post-edit
  `5c8ee385759fa0fa08bf5e09aed8d5ca5cda3bdce6ab251c335dd9bbb388d5f5`.
- `editor/test/bind/fixtures.ts`: fresh-read SHA-256
  `44fe11e998663bbb3a1c7b9a120724dcf027bdd6177312574c00aa4296b45ed4`; post-edit
  `fe5465ef0658c785c3ab698d339d5b5b729a180f0c0471aa95d2fc5da78ccd9a`.
- `editor/test/bind/panel.test.ts`: fresh-read SHA-256
  `82664e8843fb83c545feeaf3ab354f55b7a1b6cb9d4d223e2192bcece6d432fd`; post-edit
  `6f6adfcb095c70d92365715ca9b4cf6fce4695044c24bf212beb4d0ed86da6b1`.
- The new regression explicitly removes `matchMedia`, uses deterministic rAF callbacks
  and pane/panel rectangles, and verifies that scrolling to row 150 mounts row 151,
  unmounts row 1 and keeps mounted rows at or below 35. It appears by name in
  `panel-ti-01-verbose.log` (10/10 panel tests pass).
- Current-source gates: focused suite 162/162 (`focused-ti-01-final.log`), full vitest
  767/767 (`vitest-full-ti-01-final.log`), `npm run check` exit 0
  (`check-ti-01-final.log`), style 4/4 browser/viewport combinations
  (`style-ti-01-final.log`), behavior 18/18: Chromium 10/10 and WebKit 8/8
  (`e2e-behavior-ti-01-final.log`). Existing test assertions and the default seam
  remain unchanged.

### Session: 2026-10-06 (OPTDOM-ADV-01 adversarial repair)
**Tasks Completed**: Preserved deferred row renders across same-frame refreshes and
added a production-path eval-result/bindings regression.
**Notes**:
- `SliderPanel.syncRows` now unions requested IDs into `pendingRender`; `updateWindow`
  remains the single consumer and clears the accumulated set after the frame.
- The non-seam test evaluates site 1 from 0.5 to 0.9, applies a bindings batch for
  site 2 before rAF, then verifies site 1 displays 0.9 and renders exactly once more.
- Final repair-source SHA-256 values: `panel.ts`
  `60fdb23334daef37eda9738492c28553389499475dc341269c0d6053836a9873`,
  `panel.test.ts` `1b15375936eb0d33a9dbc3ca13a81c0bef93349e8aeb69dee6eef96b1f70c262`,
  and `fixtures.ts` `fe5465ef0658c785c3ab698d339d5b5b729a180f0c0471aa95d2fc5da78ccd9a`.
- Fresh-read and post-edit SHA-256 values and final-source gate logs are recorded in
  `tmp/canvas-cutover/opt-dom/intent.json` and `receipt.json`.
- Final-source verification passed: focused 163/163, full vitest 768/768,
  `npm run check`, style 4/4, behavior e2e 18/18, serial `npm run test:perf` 1/1,
  strict clippy, full nextest 2816 passed/3 configured skips, wasm32 build and
  src-tauri cargo check. Logs are `focused-adv-01-final.log`,
  `vitest-full-adv-01-final.log`, `check-adv-01-final.log`, `style-adv-01-final.log`,
  `e2e-behavior-adv-01-final.log`, `test-perf-adv-01-final.log`,
  `clippy-adv-01-final.log`, `nextest-adv-01-final.log`, `wasm32-adv-01-final.log`
  and `tauri-check-adv-01-final.log`. Review acceptance remains downstream.

### Session: 2026-10-06 (OPTDOM-INT-01 rebuilt-bundle repair)
**Tasks Completed**: Rebuilt the frontend bundle and reran the style and behavior
browser gates against the final OPT-DOM source.
**Notes**:
- Before verification, `panel.ts`, `panel.test.ts` and `fixtures.ts` SHA-256 values
  matched `receipt.json` exactly; no product source or test file changed.
- The rebuilt `editor/dist/index.html` timestamp is later than `panel.ts`, and
  `grep -rl clientTop editor/dist/assets` finds
  `editor/dist/assets/index-CUhTDdDO.js`. Bundle hashes and freshness proof are in
  `tmp/canvas-cutover/opt-dom/dist-freshness-int-01.log` and `receipt.json`.
- `npm run test:style` passed 4/4 engine/viewport combinations. Behavior e2e passed
  18/18: Chromium 10/10 and WebKit 8/8, using release wasm SHA-256
  `17c8630bb5d3f3acf90ea73b49ed475f1a497f3f1a363732e4136ccedf743a2d`.
- Logs: `frontend-build-int-01-rerun.log`, `dist-freshness-int-01.log`,
  `style-int-01-final.log`, and `e2e-behavior-int-01-final.log`. An initial shell
  wrapper had a zsh reserved-variable error after the build; that attempt is not
  counted as verification and its log is retained without overwriting.

### Session: 2026-10-06 (session 289 plan checkpoint)
**Tasks Completed**: Status bookkeeping only.
**Notes**: The session-288 integration review accepted CANVAS-OPT-DOM (commit 827b851).
Its manifest entry moved from `plans` to `acceptedDependencies`, so it is not
redispatched. No source, scope or criteria change.
