# Canvas Cutover OPT-TEXT: Advance Tables, Change-Set Layout, Deferred Syntax and Off-Keystroke Completion Implementation Plan

**Status**: In Progress (Step 6 implementation complete; downstream reviews pending)
**Plan ID**: CANVAS-OPT-TEXT (session 286, wave 11; runs alone after CANVAS-OPT-DOM is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.14 section 3 (text layout, syntax and request scheduling), section 6 (budgets), engine rules; 15.3.3 (as amended: measured advances, caret validated against the same shaping); 15.3.8.7 (allowed whole-text transfers, as amended); 15.3.8.10 (bounded edit path)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-OPT-TEXT`)
**Created**: 2026-10-06
**Last Updated**: 2026-10-06

---

## Intent and Context

Text work per keystroke must fit the 8 ms textWork gate in WebKit and Chromium. The static
inventory (`tmp/canvas-cutover/diag-shape/FABLE-STATIC-REPORT.md`) and the measured report
found these costs:

- `TextLayout.shape` calls `measureText(' ')` per shape (`layout.ts:251`), measures runs, and
  reassigns `ctx.font` in `advance`/`offsetInRun` (`layout.ts:320,334`). In WebKit the first
  `measureText` of a frame absorbs a style flush.
- `TextLayout.setText` compares every cached line as a string (`layout.ts:79-92`).
- `SyntaxSpans.spans` runs the incremental tree-sitter reparse synchronously inside the text
  frame (`syntax.ts:123-138`): about 12 ms per edit on the 20,000-line document. It also
  recaptures 3 viewports on every text-dirty frame, including caret-only frames
  (`mount.ts:178-186`).
- `CompletionPopup.request` runs `surface.text()` (`doc.toString()`), a 1 MiB encode and wasm
  `complete_source` in the keystroke microtask (`completion-popup.ts:117-159`,
  `completion-view.ts:27`). It then calls `text()` again to compare.

## Non-goals

- No renderer, atlas or geometry change. CANVAS-OPT-RENDER replaces the atlas later. This
  plan keeps the `ShapedLine`/`ShapedRun` contract the current renderer uses.
- No bind, store, history or accessibility change (earlier plans own those).
- No syntax Worker. Escalation S (15.3.8.14 section 7) is reported by CANVAS-EVIDENCE, never
  implemented here. Do not create `editor/src/code/syntax-worker.ts`.
- No change to the 16,384-span cap, `syntax-truncated`, `codePane.dataset.syntax`,
  `CHECK_DEBOUNCE_MS` (300), the completion item model or the wasm completion engine.
- No dependency, no Rust, no Session Protocol change.

## Ownership

writePaths:

- `editor/src/code/advances.ts` (new)
- `editor/src/code/layout.ts`
- `editor/src/code/syntax.ts`
- `editor/src/code/syntax-core.ts`
- `editor/src/code/completion-popup.ts`
- `editor/src/code/completion-view.ts`
- `editor/src/code/completion-types.ts`
- `editor/src/code/completion.ts`
- `editor/src/code/diagnostics.ts`
- `editor/src/code/frame.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/input.ts`
- `editor/src/code/view-host.ts`
- tests:
  - `editor/test/canvas/edit-cost.test.ts`
  - `editor/test/canvas/mount.test.ts`
  - `editor/test/canvas/state.test.ts`
  - `editor/test/canvas/frame.test.ts`
  - `editor/test/canvas/gpu.test.ts` (the "shaped UTF-16 layout" describe only)
  - `editor/test/canvas/view-host.test.ts`
  - `editor/test/canvas/input.test.ts`
  - `editor/test/code/syntax.test.ts`
  - `editor/test/code/syntax-core.test.ts`
  - `editor/test/code/syntax-fallback.test.ts`
  - `editor/test/code/completion-popup.test.ts`
  - `editor/test/code/completion-view.test.ts`
  - `editor/test/code/completion.test.ts`
  - `editor/test/code/diagnostics.test.ts`
- `impl-plans/active/canvas-cutover-opt-text.md` (checkboxes and progress log only)
- `tmp/canvas-cutover/opt-text` (artifact root)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`

sharedPaths:

- `editor/test/support/canvas.ts`: an additive fake member only (for example a
  `measureText` call counter), if a test needs one. Record the reason.
- Harness sharedPaths (session-288 ownership amendment; design 15.3.8.14 section 8). No
  edit is expected. They are listed so that a frame-timing harness fix never blocks this
  plan:
  - `editor/test/e2e/behavior.mjs`
  - `editor/test/e2e/measure.mjs`
  - `editor/test/e2e/run.mjs`
  - `editor/test/e2e/stats.mjs`
  - `editor/test/e2e/stats.test.ts`
  - `editor/test/e2e/ios-sim.mjs`
  - `editor/test/e2e/README.md`
  - `editor/test/style/ui-style.mjs`

### Harness sharedPaths (session 288)

The rules are identical to the "Harness sharedPaths (session 288)" section of
`impl-plans/active/canvas-cutover-opt-dom.md`. They are restated here so this plan stands
alone.

**Allowed edit.** Only frame alignment of harness sampling (wait for a presented frame,
then a bounded poll; reference 842cf6e), or documentation of that alignment.

**Never change:**
- a check name, assertion, expected value or comparison;
- `THRESHOLDS` or `TARGETS` (the accepted textWork gate is 8 ms with a 4 ms target);
- the workload, the fixtures or the silent sink;
- `run.mjs` exit codes or its gating refusal;
- the `ios-sim.mjs` predicate;
- the `ui-style.mjs` checks.

A non-alignment harness change is a blocker for a serial plan-author amendment.

**Recording.** Record the path, reason and sha256 values in the progress log and in
`tmp/canvas-cutover/opt-text/intent.json` and `receipt.json`.

**Check.** `git diff <START> -- <path>` shows no removed or changed line containing
`expect(`, `assert`, `THRESHOLDS`, `TARGETS` or a check comparison.

**Deferred work in harness checks.** OPT-TEXT defers the reparse (`setTimeout(0)`) and the
completion request (150 ms). A harness check that reads syntax or completion state after an
edit may need to wait for that state. Do this by frame-aligned or bounded polling only,
never by weakening the check.

### Session 289 resume (CANVAS-OPT-TEXT only)

**Why.** Session 288 blocked this plan only because the receipt listed the already
repaired OPTTEXT-TI-02 with severity `mid` under unresolved findings. The progress gate
(`scripts/implementation-progress-check.py`, material_findings) blocks any item whose
severity, or any string, starts with critical, high, mid or medium in `payload.risks`,
`payload.findings`, `authorSelfCheck.findings` or `authorSelfCheck.residualRisks`.

**Source state.** The source is final at HEAD 827b851. TASK-T1 to TASK-T6 and the
OPTTEXT-TI-01 and OPTTEXT-TI-02 repairs are all in that commit. Do not reimplement.
Change source only if a gate or a new review finding exposes a defect, and then fix it
inside this plan's writePaths.

**Base.** `BASE=1d17ced` (session-286 plan checkpoint) for the diff criteria. Record
`START` (the session-289 plan checkpoint commit) in `tmp/canvas-cutover/opt-text/intent.json`.
The harness `git diff <START>` check uses that START. OPT-TEXT edited no harness
sharedPath: `git diff --stat 842cf6e 827b851 -- editor/test/e2e editor/test/style` is
empty. The only change since ff49fd1 is the operator frame wait in `behavior.mjs`
(842cf6e), which the OPT-DOM review covers.

**Gates.** Rerun every row of the Verification section on the current HEAD, one heavy
suite at a time:
- focused canvas;
- `test/code`;
- default vitest;
- `npm run check`;
- `test:perf` (alone, `--maxWorkers=1`);
- strict clippy;
- full nextest (alone, `timeout 2400`);
- the host-wasm wasm32 build;
- the `editor/src-tauri` cargo check.

Rust sources are unchanged since the accepted Rust gates. Rerun the Rust rows on HEAD
anyway, because that gives the new fingerprint. Report only final passing runs, in the
mandatory record format. Expected counts: focused canvas 151, `test/code` 154, default
vitest 776 or more, nextest 2816 passed with 3 configured skips.

**Receipt rules.**
- OPTTEXT-TI-01 and OPTTEXT-TI-02 go only into `addressedFeedback`/`resolvedFindings` with
  status `repaired` and their evidence. OPTTEXT-TI-02 evidence: `changedRanges` calls
  `getChangedRanges` on the edited previous tree with the new tree as its argument.
  `editor/test/code/syntax.test.ts` asserts at most 4 recaptured lines for an inline edit,
  with an in-test no-`changedRanges` control that recaptures exactly 64 lines.
- The test path is `editor/test/code/syntax.test.ts`. The session-288 receipt wrongly
  cited `editor/src/code/syntax.test.ts`; that file does not exist.
- Never list a repaired finding, or the pending independent review, in risks, findings
  or residualRisks. A genuine residual note uses severity `low` or no severity.
- No mutation or negative-control command. Cite historical logs (for example
  `syntax-ti02-focused-01.log`) only in `notes`.

## Contracts and Key Points

### 1. `advances.ts` (new) and `TextLayout` measurement

- **`createMeasureContext(doc: Document): TextMetricsSource | null`.** Use
  `new OffscreenCanvas(1, 1).getContext('2d')` when `typeof OffscreenCanvas === 'function'`
  and the context is non-null. Otherwise use `doc.createElement('canvas').getContext('2d')`.
  `code/mount.ts` uses it in place of the `metricsCanvas` lines 70-72, keeping the existing
  length*8 stub when both are null.
- **`class AdvanceTable`**, built per (font string, font generation) from a
  `TextMetricsSource`:
  - It never assigns `source.font` itself. The table is built from the `TextLayout`
    metrics source after `TextLayout.setMetricsFont` (`layout.ts:102-106`) has set the
    font, and it reuses that guard (`metricsFont`). So the shared metrics object sees exactly
    one font assignment per font change. This keeps `edit-cost.test.ts:135-150`
    ("sets the canvas metrics font once ...": `assignments === 1` after shaping and `2`
    after `setFont`) passing unchanged.
  - `ascii: Float64Array(128)` holds the widths of U+0020..U+007E, from 95 `measureText`
    calls.
  - **Additivity probe.** At build, also measure the probes `['ffi', 'fi', 'fl', 'AV', 'To',
    '->', '==', 'www', 'Wa']` and compare each with the sum of its table widths. If any
    differs by more than 0.01 px, set `additive = false`. Probes count as measurement
    calls at build only.
  - `cluster(text: string): number` returns a cached width from an LRU of at most 8,192
    entries per table, measured on a miss.
  - `stats = { builds, measured }`.
  - `TextLayout.invalidate()` and `setFont()` drop the table, and the next shape rebuilds
    it. The renderer already calls `layout.invalidate()` on a DPR change, so the table is
    effectively keyed by (font, generation, effective DPR).
- **`TextLayout` line classes.**
  - *Additive lines:* the font is additive, the line contains no complex-script character
    (U+0590-U+08FF, U+0900-U+0DFF, U+0E00-U+0EFF, U+1000-U+109F, U+1780-U+17FF,
    U+FB1D-U+FEFF), and `clustersForLine` did not return `null`. Their run width is the sum
    of the ASCII table widths for code units below 0x80, plus `cluster(segment)` for every
    other grapheme. Graphemes come from the existing `clusters` pairs; a non-ASCII single
    code unit outside a pair is its own grapheme.
  - *Prefix widths* (`widthAt`, `offsetInRun`) on additive lines sum the same widths inside
    the existing 256-character chunk index. No `measureText` call and no per-character array
    are stored, so the "tiles long text" cache bound in `gpu.test.ts:183-186` stays.
  - *Other lines* (non-additive font, complex script, null clusters) keep today's
    measurement path unchanged.
  - *Tab stops* use `4 * ascii[0x20]` when the table exists, else one cached space
    measurement per table.
  - *`ctx.font`.* Remove the assignments in `advance` (`:320`) and `offsetInRun` (`:334`).
    `setMetricsFont` stays the only writer.
  - *Counter.* `stats.measuredTextCalls` and `measuredTextChars` are incremented on every
    measurement, not only when `phases` is set (`layout.ts:208-212`).
- **Why the probe exists.** 15.3.3 requires caret advances validated against the same
  shaping. The test stub in `gpu.test.ts:10-14` models an `ffi` ligature, and the row
  "uses whole-run shaping and measured Japanese/ligature caret advances" (`:163-168`)
  must keep passing unchanged. With the probe it does, because that stub is non-additive and
  keeps run measurement. Real monospace fonts are additive, so they use the table.

### 2. Change-set layout maintenance

- `TextLayout.setText(doc: Text, changes?: ChangeSet)`.
  - With `changes`, which must map the previously set `Text` to `doc`, compute the changed
    line ranges in old coordinates (`iterChangedRanges` plus `lineAt`). Drop cached lines
    inside those ranges. Renumber the cached lines after them by the line delta, shifting
    `line.from/to` and the runs' `from/to` by the position delta, as the existing shift code
    at `:86-91` does. Compare no strings.
  - Shift the shifted lines' existing `ShapedRun` objects in place (`run.from`/`run.to`
    += delta), as `layout.ts:86-91` does today, so run identity is preserved
    (`edit-cost.test.ts:129` asserts `reused.runs[0]` is the same object).
  - Without `changes`, or when `doc` does not continue the previous text, clear the cache.
    Do not compare line by line. This is the design rule. The accepted rows that relied on
    the old string-comparison fallback are ported to pass a change set (see "Ported rows"
    under Test Cases); their assertions stay byte-identical.
- `InputPresentation` (`input.ts:66-80`) gains `changes?: ChangeSet`, the change from the
  previously published presentation's `doc`:
  - in the non-composing case, the surface update's `changes` when the previous
    presentation's `doc` is the update's start doc;
  - while composing with the same `stateDoc` and `range.from`, a single replacement of the
    old preedit by the new one;
  - otherwise `undefined`.

  `code/mount.ts` `onPresentation` passes it to `layout.setText(doc, changes)`.

### 3. Deferred syntax with span reuse (`syntax.ts`, `syntax-core.ts`)

- **`ParsedVact`** gains an optional
  `changedRanges?(previous: ParsedVact): { from: number; to: number }[]`. `createVactSyntax`
  implements it with `tree.getChangedRanges(previousTree)`, and indices stay UTF-16, as in
  `treeEdits`. Call it before the old tree is deleted.
- **`SyntaxSpans` state:**
  - `parsed`, plus a `fallback: FallbackSpans`, which is used until the first tree exists;
  - `pending: ReturnType<typeof setTimeout> | null`;
  - `lineCache: Map<lineNumber, { spans: { from: number; to: number; className: string }[] }>`,
    with positions relative to the line start, for the capture window;
  - `revision` (the syntax revision);
  - `stats = { syncParses, deferredParses, captures, capturedLines }`.
- **`noteChanges(changes, state)`:**
  - apply the tree edits as today;
  - remap `lineCache`: renumber the untouched lines after the change, and map each touched
    line's spans through `changes`. Inserted text gets no style unless it falls inside a
    span;
  - schedule one `setTimeout(0)` reparse when none is pending. Do not parse here.
- **Reparse task:**
  1. Parse the current document incrementally (`parseDoc(doc, old)`), or in full when
     `fullParse` is set.
  2. Read `next.changedRanges?.(old)`.
  3. Recapture the touched lines plus the changed-range lines inside the current window.
     When `changedRanges` is missing, recapture the whole window.
  4. Delete the old tree.
  5. Bump `revision`, then call the `onSyntax` callback (constructor option), which
     `mount.ts` wires to `scheduler.invalidateText('syntax')`.

  The first parse (no tree yet) also runs in this task.
- **`spans(state, from, to, limit)`.** Never parses. It returns the cached lines in range.
  Lines missing from the cache are captured incrementally (`stats.captures += 1` per call
  that captures, `capturedLines += n`), but only when the tree is current. Otherwise they
  are returned unstyled. Before the first tree, it returns `fallback.spans`. Apply the
  limit and the truncation flag through `bounded()` as today.
- **`flush()`** runs the pending reparse synchronously, for tests and disposal ordering.
  **`dispose()`** clears the timer and deletes the tree.
- **`stats.syncParses`** counts parses run outside the scheduled task. It must stay 0.

### 4. Completion off the keystroke task

- **`completion-types.ts`.** `CompletionSurface` gains `version(): unknown`, the document
  identity. Export `COMPLETION_DEBOUNCE_MS = 150`.
- **`completion-view.ts`.** `version: () => code.state.doc`.
- **`completion-popup.ts`:**
  - `request(kind: 'typing' | 'explicit')`. Typing triggers (`handleChange`) clear and re-arm
    a `setTimeout(COMPLETION_DEBOUNCE_MS)`. `Ctrl-Space` arms `setTimeout(0)`. The timer
    callback runs the existing in-flight or dirty logic.
  - `runRequest` reads `text()` once, records `version()`, and after the await compares
    `version()` (identity) instead of `text()`. Latest-wins `requestSequence`, the
    composition skip and `close()` stay.
  - While a request is pending, the panel keeps its items. The existing rule "close if
    `selection.head < result.from`" stays.
  - `dispose()` clears the timer.
- **`completion.ts`.** No behavior change. Keep its per-request conversion; it now runs at
  most once per debounced request.

### 5. Check scheduling (`diagnostics.ts`)

No behavior change is expected. Verify, and add a test showing, that the check runs only
from its 300 ms timer: never inside the dispatch call stack and never from a frame. If a
synchronous path is found, move it behind the timer.

### 6. Dirty reasons (`frame.ts`, `mount.ts`, `view-host.ts`)

- **`frame.ts`.**
  - `export type TextDirtyReason = 'doc' | 'selection' | 'view' | 'syntax' | 'annotations' | 'gpu'`.
  - `invalidateText(reason: TextDirtyReason = 'doc')` accumulates a set.
  - `FrameContext` gains `reasons: ReadonlySet<TextDirtyReason>`; `textDirty` stays
    `reasons.size > 0`.
  - The internal callers are DPR, visibility and viewport: use `gpu` or `view`.
- **Callers.**
  - `mount.ts` surface subscription: `docChanged` gives `doc`; else `selectionSet` gives
    `selection`; else `annotations`.
  - The pointer `onHandles` callback gives `selection`.
  - The syntax provider load gives `syntax`.
  - `onPresentation` gives `doc`.
  - The `viewHost` scroll callback gives `view`.
- **Frame rule (`mount.ts` `onFrame`).** Compute the capture window `[first, last]` as today
  (lines 179-183). Call `syntaxProvider.spans(...)` only when `reasons` has `doc`, `syntax`
  or `gpu`, or when the window differs from the last call. Otherwise reuse the last result.
  The static rows are still assembled from the reused spans plus the annotations, selection
  and composition.

### Patterns to imitate

- `layout.ts` `runWidthIndex`/`widthAt` for the chunk structure.
- `syntax.ts` `FallbackTokenizerCache.noteChanges` for line remapping.
- `test/canvas/edit-cost.test.ts` `setup()` and spies.
- `test/code/diagnostics.test.ts` fake-timer `setup('browser', check)`.

## Tasks

### TASK-T1: `advances.ts`, table, probe and layout summation
### TASK-T2: Change-set `setText` and presentation `changes`
### TASK-T3: Deferred syntax, span cache and `changedRanges`
### TASK-T4: Completion debounce and version identity
### TASK-T5: Dirty reasons and the frame spans rule; check verification
### TASK-T6: Tests, verification and progress log

Each task's completion criterion is its contract plus its test rows, passing.

## Test Cases

**Ported rows.** Each port changes only the call that supplies the change set. Every
`expect()` line stays byte-identical, and each port is recorded in the progress log with
its file:line.

- `editor/test/canvas/edit-cost.test.ts:104-113` ("retains shaped lines before an edit at
  line 10,000").
  - Build `changes = ChangeSet.of({ from: doc.line(10_001).from, insert: 'x' }, doc.length)`
    and `changed = changes.apply(doc)`.
  - Call `layout.setText(changed, changes)` instead of `layout.setText(changed)`.
  - The replacement text is identical to today's `doc.replace(...)`.
- `editor/test/canvas/edit-cost.test.ts:115-133` ("reuses unchanged shaped lines after a
  prefix edit shifts their document offsets"): the same port, with
  `ChangeSet.of({ from: 0, insert: 'x' }, doc.length)`.
- `editor/test/canvas/view-host.test.ts:22-31` (the rig): `onPresentation` becomes
  `(presentation) => layout.setText(presentation.doc, presentation.changes)`, using the new
  `InputPresentation.changes`. The rows at `:50-60` and `:92-100` stay unchanged.
- `editor/test/canvas/edit-cost.test.ts:135-150` (metrics font assigned once): no edit. It
  must pass unchanged through the guard in contract 1.

`editor/test/canvas/gpu.test.ts`, describe "shaped UTF-16 layout": every existing row passes
unchanged, including the ligature row and the long-line `cacheBytes < 50_000` row. These
rows call `setDocument(text)` or a first `setText` on an empty cache, so clearing the cache
does not affect them. New rows:

- An additive stub (`width = length * 8`) -> after the first `shape()`,
  `layout.stats.measuredTextCalls` stays constant across `shape()` of 50 more ASCII lines
  and `coordsAtPos`/`posAtCoords` on them.
- The `ffi` stub -> the table reports `additive === false`, and the shape result equals the
  run-measured result (control).
- A Japanese line on an additive stub -> each distinct character is measured once; a second
  line with the same characters makes 0 calls.

`editor/test/canvas/edit-cost.test.ts` (20,000-line `setup()` plus a mounted rig or
`TextLayout` and `SyntaxSpans` with a fake `VactSyntax`). One `replaceSelection('x')` on line
10,001, then one frame:

- layout `measuredTextCalls` delta 0;
- `shape()` builds `<= 1` plus newly exposed lines;
- `Text.prototype.toString` called 0 times on a document of 64 KiB or more;
- `syntax.stats.syncParses === 0`; after `vi.advanceTimersByTime(0)` (or `flush()`),
  `deferredParses === 1`;
- completion `engine.complete` called 0 times before 150 ms and exactly 1 after
  `advanceTimersByTime(150)`;
- `core.check` called 0 times before 300 ms.

Each row includes a control (an emoji edit that must measure a new cluster once, or a
scroll that must capture new lines).

`editor/test/canvas/mount.test.ts`:

- A caret-only move (selection dispatch) and one frame -> `syntax.stats.captures` delta 0
  and `spans()` calls 0.
- A scroll by one viewport -> `captures` `> 0` (control).

`editor/test/code/syntax.test.ts` and `syntax-core.test.ts`:

- The existing equivalence rows (including the 200-edit equivalence) call `provider.flush()`
  (or advance fake timers) before reading `spans()`. The expected spans are unchanged.
- New rows:
  - edit, then `spans()` before the flush -> touched-line spans are the mapped old spans and
    `syncParses === 0`;
  - after the flush, the result is identical to a full-parse reference;
  - `changedRanges` is used: an edit inside one line recaptures only lines in the changed
    ranges (assert `capturedLines` is at most the changed lines plus the touched lines).

`editor/test/code/completion-popup.test.ts` and `completion-view.test.ts`:

- Existing rows advance fake timers by `COMPLETION_DEBOUNCE_MS` before asserting. Their
  assertions are unchanged.
- New rows:
  - 5 identifier keystrokes 30 ms apart -> 1 request, 150 ms after the last;
  - Ctrl-Space -> a request after `advanceTimersByTime(0)`, not synchronously;
  - an edit during an in-flight request -> the result is discarded by version identity, and
    `text()` is called once per request.

`editor/test/canvas/frame.test.ts`:

- `invalidateText('selection')` -> `ctx.reasons` contains only `selection`.
- `invalidateText()` defaults to `doc`.

`editor/test/canvas/state.test.ts` or `edit-cost.test.ts`:

- `layout.setText(doc, changes)` after an Enter at line 10,000 -> cached lines 0..9,998 keep
  their identity, and lines after it are renumbered without a string comparison (spy on
  `Text.prototype.line` count `<= cached lines + small constant`).

## Pitfalls

- A missing change set clears the layout cache by design. Every in-repo caller of
  `layout.setText` on an incremental path must pass `changes`:
  - `code/mount.ts:145` `onPresentation` uses `presentation.changes`;
  - `renderer.setText(doc)` at `mount.ts:175` stays a no-op for the layout, because
    `TextLayout.setText` returns early when `doc` is the same object (`layout.ts:76`). Keep
    that early return before any cache logic.

  Grep `setText(` in `editor/src` and `editor/test` and port each incremental caller.
- Never parse inside `spans()` or `noteChanges()`. `syncParses` must stay 0.
- Call `changedRanges(old)` before `old.delete()`. The web-tree-sitter tree is invalid after
  deletion.
- The reparse timer must read the current document from the last `noteChanges` state,
  latest wins. Do not capture a stale `state` in the closure.
- Do not break the ligature and long-line rows. The additivity probe exists for them.
- Do not call `getComputedStyle` or read `ctx.font` back on hot paths.
- Completion: a timer callback after `dispose()` must be a no-op.
- `FrameContext.textDirty` must stay for existing consumers and tests.
- Keep `layout.ts` under 1000 lines. Put the table and probe in `advances.ts`.

## Verification

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`;
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
Record `BASE` and the fresh-read sha256 values in `tmp/canvas-cutover/opt-text/intent.json`.

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/canvas/gpu.test.ts test/canvas/edit-cost.test.ts test/canvas/mount.test.ts test/canvas/frame.test.ts test/canvas/state.test.ts test/canvas/view-host.test.ts test/canvas/input.test.ts` | exit 0; the new rows are listed |
| `cd editor && ./node_modules/.bin/vitest run test/code` | exit 0 (syntax, completion and diagnostics rows) |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 |
| `cd editor && npm run check` | exit 0 |
| `wc -l editor/src/code/layout.ts editor/src/code/advances.ts editor/src/code/syntax.ts editor/src/code/mount.ts` | each under 1000 (notes only) |

Outside the sandbox: `cd editor && npm run test:perf` (alone); `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`;
`NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`;
`CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`;
`CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml`. Each must
exit 0.

Use the session-286 record format: `exitStatus: 0`, `outcome: "passed"`, `testsRun > 0` and
`failureCount: 0` for tests, details in `notes`. Run no mutation or negative-control
command.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/opt-text/intent.json`
and `receipt.json`. If a file drifted without an edit from this plan, stop editing it and
report. Edit only this plan's progress log.

## Completion Criteria

- [x] `advances.ts`: OffscreenCanvas-first measure context, the ASCII table, the cluster LRU and the additivity probe
- [x] A single ASCII edit makes 0 layout `measureText` calls; `ctx.font` is never assigned on hot paths
- [x] `setText(doc, changes)` is change-proportional; no line string comparison; `edit-cost.test.ts:104-133` and the `view-host.test.ts` rig ported to pass change sets with byte-identical assertions; `edit-cost.test.ts:135-150` passes unchanged
- [x] Syntax: 0 synchronous parses and 1 deferred parse per edit; caret moves make 0 captures; only changed lines are recaptured
- [x] Completion is debounced to 150 ms (Ctrl-Space in the next task); version-identity staleness; one `text()` per request
- [x] Check runs only from its timer (test row)
- [x] Dirty reasons in place; `spans()` is skipped on selection-only frames
- [x] All existing assertions unchanged (timer advances added where needed); default vitest, `npm run check`, `test:perf`, clippy, nextest, wasm32 build and src-tauri check pass
- [x] Harness sharedPaths unedited
- [x] Progress log updated

## Progress Log

### Session: 2026-10-06 (session 286 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.14 section 3.
**Plan repair (step5 S286-PLAN-TEXT-SETTEXT-PORT, mid)**:

- Named the `setText` ports: `edit-cost.test.ts:104-113`, `:115-133` and the
  `view-host.test.ts:22-31` rig, with byte-identical `expect()` lines.
- Added the AdvanceTable font guard that keeps `edit-cost.test.ts:135-150`.
- Added the setText-caller pitfall.

**Notes**: Wave 11, after DOM. The additivity probe is a plan-level safeguard required by
15.3.3 ("caret boundary advances must be validated against the same shaping") and by the
existing ligature row in `gpu.test.ts`.

### Session: 2026-10-06 (session 288 plan amendment)
**Tasks Completed**: Ownership amendment only (design 15.3.8.14 section 8, "Harness
sharedPaths").
**Notes**: Added the 8 harness and style files as concrete sharedPaths, here and in the
manifest, with the frame-alignment-only rule. Scope, contracts, tasks and criteria are
otherwise unchanged. Status stays Ready; this plan starts after CANVAS-OPT-DOM is accepted.

### Session: 2026-10-06 (session 286 implementation)
**Tasks Completed**: TASK-T1 through TASK-T6 implemented. AdvanceTable provides an
OffscreenCanvas-first measurement surface, ASCII advances, an additivity probe and bounded
cluster LRU; production Text documents use ChangeSet-based layout reuse and pass presentation
deltas through the input bridge. Tree-sitter work is deferred, reuses line spans, and compares
changed ranges against a pre-edit tree snapshot. Completion is latest-wins and delayed 150 ms
for typing (Ctrl-Space schedules a zero-delay task). Frame dirty reasons keep caret-only frames
from requesting syntax spans. Selection-only Text identity changes no longer invalidate layout.

**Assertion ports**: `edit-cost.test.ts` setText rows (original lines 104-113 and 115-133)
now pass their ChangeSets; all `expect()` expressions remain unchanged. The
`view-host.test.ts:22-31` rig passes `presentation.changes`; its assertions remain unchanged.
The metrics-font assignment row (original `edit-cost.test.ts:135-150`) was not edited.
Additional coverage includes a 20,000-line ASCII edit, IME presentation deltas, deferred syntax
equivalence over 200 edits, the caret/scroll capture control, and completion debounce/version
identity. No harness sharedPath changed.

**Verification** (final source; logs under `tmp/canvas-cutover/opt-text/`): focused canvas
tests 151/151 (`focused-final-03.log`), code tests 153/153 (`code-progress-03.log`), full
Vitest 775/775 (`vitest-full-final-02.log`), `npm run check` exit 0
(`check-final-02.log`), serial `npm run test:perf` 1/1 (`test-perf-final-02.log`), strict
Clippy exit 0 (`clippy-final-01.log`), full nextest 2816 passed / 3 configured skips
(`nextest-final-01.log`), wasm32 build exit 0 (`wasm32-final-01.log`) and Tauri cargo check
exit 0 (`tauri-check-final-01.log`). The later test-only addition was included in the final
Vitest and check runs; Rust sources did not change after the recorded Cargo gates.

**Notes**: Formal test-integrity, adversarial and integration review remain downstream. The
plan stays In Progress until those workflow-owned decisions are recorded.

### Session: 2026-10-06 (session 288 test-integrity repair OPTTEXT-TI-01)
**Tasks Completed**: Clipped tree-sitter changed-range recapture to the active capture window
and invalidated cached line entries intersecting changed ranges outside that window by walking
existing cache keys. Added a real tree-sitter regression over 2,400 lines with two disjoint
cached 64-line windows and one in-line edit. It asserts zero synchronous parses, one deferred
parse, a captured-line delta no greater than the active window, and fresh-parse equivalence for
both windows. The pre-existing 200-edit equivalence and missed-identity expectation lines were
left unchanged.

**Verification** (final repair source; logs under `tmp/canvas-cutover/opt-text/`): focused
canvas tests 151/151 (`focused-ti01-final.log`), code tests 154/154 (`code-ti01-final.log`),
full Vitest 776/776 (`vitest-full-ti01-final.log`), `npm run check` exit 0
(`check-ti01-final.log`) and serial `npm run test:perf` 1/1 (`test-perf-ti01-final.log`).
The targeted new regression also passed (`syntax-ti01-focused-03.log`).

**Notes**: Rust sources were unchanged; earlier Rust gate evidence remains source-matched.
OPTTEXT-TI-01 is implemented and awaits independent test-integrity, adversarial and integration
review.

### Session: 2026-10-06 (session 288 test-integrity repair OPTTEXT-TI-02)
**Tasks Completed**: Corrected `ParsedVact.changedRanges` to call `getChangedRanges` on the
edited previous tree with the new tree as its argument; removed the redundant `beforeEdits`
tree copy. Tightened the real-tree-sitter inline-edit capture bound to at most 4 lines and
added an in-test provider control without `changedRanges` that recaptures exactly 64 window
lines. The regression checks the edited window and a cached window after it against a fresh
parse. Existing 200-edit equivalence and missed-identity expectation lines remain unchanged.

**Verification** (final repair source; logs under `tmp/canvas-cutover/opt-text/`): syntax
tests 9/9 (`syntax-ti02-focused-01.log`), code tests 154/154 (`code-ti02-final.log`), focused
canvas 151/151 (`focused-ti02-final.log`), full Vitest 776/776
(`vitest-full-ti02-final.log`), `npm run check` exit 0 (`check-ti02-final.log`) and serial
`npm run test:perf` 1/1 (`test-perf-ti02-final.log`).

**Notes**: The historical OPTTEXT-TI-01 counter-calibration logs remain under the plan evidence
directory but are not gating records. Rust sources remain unchanged. Independent test-integrity,
adversarial and integration review are pending.

### Session: 2026-10-06 (session 289 plan checkpoint)
**Tasks Completed**: Added the "Session 289 resume" section: re-gate on HEAD, receipt rules
for repaired findings, and the corrected test path `editor/test/code/syntax.test.ts`.
**Notes**: No scope, contract, writePaths or criteria change. Next: the final-source re-gate,
then test-integrity, adversarial and integration review.

### Session: 2026-10-06 (session 289 final-source re-gate)
**Tasks Completed**: Re-ran every assigned final-source verification gate on HEAD
`19c17989876f17cf32cc489166e2432b9c2f7370`. No product source or test changes were needed.

**Verification**: focused canvas 151/151 (`focused-session289-final.log`); `test/code`
154/154 (`code-session289-final.log`); default Vitest 776/776
(`vitest-session289-final.log`); `npm run check` exit 0 (`check-session289-final.log`);
serial `npm run test:perf` 1/1, median ratio 2.83 (`test-perf-session289-final.log`);
strict Clippy exit 0 (`clippy-session289-final.log`); full nextest 2816/2816 passed,
3 configured skips, exit 0 (`nextest-session289-final.log`); wasm32 build exit 0
(`wasm32-session289-final.log`); and Tauri cargo check exit 0
(`tauri-check-session289-final.log`). All logs are under `tmp/canvas-cutover/opt-text/`.
The required line-count check passed: `layout.ts` 429, `advances.ts` 93, `syntax.ts` 244,
`mount.ts` 328.

**Notes**: Source HEAD is unchanged, all implementation criteria remain complete, and harness
sharedPaths remain unchanged. OPTTEXT-TI-01 and OPTTEXT-TI-02 are repaired with their evidence
recorded under `addressedFeedback`; independent test-integrity, adversarial and integration
review remain downstream workflow steps.

### Session: 2026-10-06 (session 289 adversarial repair)
**Tasks Completed**: Repaired both adversarial findings inside the authorized OPT-TEXT paths.
`InputController.publish` now assigns the pending delta on every publish and records its base
Text identity; the surface subscriber supplies only a delta for its matching previous
presentation, and composition updates identify their prior presentation. `TextLayout.setText`
falls back to full invalidation when the delta's old or new length does not match, before
changing cache state. `mount.ts` again detects document changes by Text identity and passes a
delta only when its recorded base is the currently displayed document.

**Regression coverage**: `editor/test/canvas/input.test.ts` covers typing before a trailing
empty line, composition start clearing the old delta, and mismatched-delta full invalidation
with cache-byte reset. `editor/test/canvas/mount.test.ts` covers Japanese preedit rendering,
empty composition-end cancellation rendering the state document, and successful subsequent
typing. Existing 20k edit-cost and caret-only/scroll controls were retained.

**Verification** (final repair source; logs under `tmp/canvas-cutover/opt-text/`): focused
canvas 153/153 (`focused-adv-repair-02.log`); `test/code` 154/154
(`code-adv-repair.log`); full Vitest 778/778 (`vitest-adv-repair.log`);
`npm run check` exit 0 (`check-adv-repair-final.log`); serial `npm run test:perf` 1/1,
ratio 2.87 (`test-perf-adv-repair-final.log`). Rust sources were unchanged; session-289
strict Clippy, nextest, wasm32 and Tauri checks remain applicable and passed.

**Notes**: The first targeted attempt (`focused-adv-repair.log`) showed the new unit-test
fixture had no `onPresentation` subscriber, leaving its presentation cache uninitialized. The
fixture was corrected to capture presentations; the final focused and full suites pass. ADV-01
and ADV-02 are reported as repaired, with independent adversarial re-review pending.
