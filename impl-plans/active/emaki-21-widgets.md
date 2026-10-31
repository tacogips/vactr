# EMK-WIDGETS: block and end-of-line widgets, virtualization and vertical motion

**Status**: In Progress (implementation complete; review pending)
**Plan ID**: EMK-WIDGETS (wave 2; parallel with EMK-DECOR, EMK-LANG, EMK-DIFF)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 8.1-8.4; decision EM-D8
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants plugins that can embed video, animation and other content in
editor lines. vactr's inline sequencer and oscilloscope also use this. The
`Heightmap` (EMK-CORE) already maps lines and y positions with blocks. This
plan adds:

1. **`WidgetSet`.** Owner-keyed specs, mapped through edits and reconciled
   by `key`. It produces heightmap entries.
2. **`WidgetLayer`.** A DOM container over the canvas that mounts, positions,
   clips, virtualizes and draws widgets.
3. **Vertical motion.** ArrowUp/ArrowDown by line index. The current
   y-offset approach (`keyboard.ts:115-117`) would get stuck in a block band,
   because band hits map to the anchor line.

## Non-goals

- No plugin runtime. EMK-API calls `WidgetSet` and `WidgetLayer`.
- No marks or line formats.
- No PageUp/PageDown handling: `KeyboardController` does not handle them
  today, and this plan adds no new key behavior. Design 8.4's PageUp/Down
  sentence describes browser scrolling; there is nothing to implement.
- No widget content beyond what tests need. The demo widgets belong to
  EMK-SHOWCASE.

## Dependencies

- **dependsOn**: EMK-CORE.

## writePaths

Source:
- `emaki/src/plugin/index.ts`, `emaki/src/plugin/widgets.ts`
- `emaki/src/view/index.ts`, `emaki/src/view/widget-layer.ts`
- `emaki/src/input/keyboard.ts`

Tests:
- `emaki/test/plugin/widgets.test.ts`,
  `emaki/test/view/widget-layer.test.ts`,
  `emaki/test/input/vertical-motion.test.ts`

Generated, plus this plan:
- `emaki/node_modules`, `emaki/dist` (artifactRoots)
- `impl-plans/active/emaki-21-widgets.md`

## sharedPaths

None. Read-only: `emaki/src/layout/heightmap.ts`, `emaki/src/layout/layout.ts`,
`emaki/src/view/view-host.ts`, `emaki/src/render/resources.ts`,
`emaki/src/types.ts`.

If `heightmap.ts` or `layout.ts` needs a fix, stop and report a scope
amendment. EMK-DECOR owns `layout.ts` in this wave.

## Tasks

### W1: `plugin/widgets.ts`, class `WidgetSet`

**API**
- `set(owner, blocks: readonly BlockWidgetSpec[], eol: readonly EolWidgetSpec[] = [])`:
  - validate `pos` (0..length);
  - `height` is finite and between 0 and 10000;
  - `width` is finite and between 0 and 4000;
  - keys are unique per owner.

  Throw `RangeError` otherwise.
- `clear(owner)`.
- `map(changes: ChangeSet)`: anchors map with
  `mapPos(pos, -1, MapMode.TrackDel)`. A `null` result destroys the widget.
- `entries(doc): { line, above, below }[]`, ready for
  `Heightmap.setBlocks`. Sums per line.
- `blocks(doc)` and `eols(doc)`: resolved items
  `{ id: owner + '\u0000' + key, line, side?, height?, width?, content, interactive, label }`.
- `revision`: bumped on every effective change.
- `onRemoved(cb: (id) => void)`: called for each destroyed or removed
  widget.

**Reconcile rule.** The same owner, key and `content.kind` keep identity.
The content object may be replaced (the layer re-mounts only if
`content.kind` changed or the `content` object identity changed for
`dom`). Canvas `draw` replacement takes effect without remounting.

### W2: `view/widget-layer.ts`, class `WidgetLayer`

**Construction.** `new WidgetLayer(host: HTMLElement, layout: TextLayout, ledger: ResourceLedger)`
creates one absolutely positioned container:
- class `emaki-widget-layer`;
- `pointer-events: none`;
- `overflow: hidden`, clipped to the text area (`left = gutter width`);
- appended after the code canvas.

**Per frame.** `update({ view: LayoutViewport, dpr, frameMs, blocks, eols })`
does the following:

1. **Rects.**
   - A block rect is at `top = heightmap.lineTop(line) - blockAbove(line)`
     for `above`, or `lineTop(line) + lineHeight` for `below`.
   - Inside the container, subtract `view.scrollTop`.
   - Block width is the text-area width. Blocks do not scroll horizontally.
   - An EOL rect is at `x = layout.coordsAtPos(line end).right + 1 ch`, the
     line top, `width` wide and `lineHeight` tall. It does scroll
     horizontally.
2. **Mount.** If a rect intersects `[scrollTop - overscan, scrollTop + height + overscan]`
   with `overscan = view.height`, and the widget is not mounted, create a
   wrapper `div`:
   - class `emaki-widget`;
   - absolute position;
   - `pointer-events` is `auto` only if `interactive`;
   - `aria-hidden="true"` unless `label` is set, in which case set
     `role="group"` and `aria-label`.

   For `dom` content, call `mount(wrapper)` and keep the cleanup it returns.
   For `canvas` content, create a `<canvas>` sized `width x dpr` by
   `height x dpr`, `ctx.scale(dpr, dpr)`, and charge `width x height x dpr²
   x 4` bytes to the ledger. If the ledger refuses, show nothing and keep
   the widget unmounted.
3. **Unmount.** When a rect is outside `scrollTop +/- 2 x overscan`, call the
   cleanup, remove the wrapper and release the canvas bytes.
4. **Position.** Set `transform: translate(x, y)` on the wrapper. Use one
   style write per widget per frame, and only when the value changed.
5. **Draw.** For mounted canvas widgets that intersect the real viewport (no
   overscan), call `draw(ctx, { width, height, dpr, frameMs })` only if the
   last draw returned `true`, or the spec or size changed. Clear the canvas
   before drawing.
6. **Return** `{ animating: boolean }`, true if any visible canvas widget
   returned `true`.

`removeAll()` and `dispose()` unmount everything and release all bytes.

**Pitfalls**
- Do not read layout in loops. Read `scrollTop` once.
- Never call `getBoundingClientRect` per frame.
- Destroyed widgets (`WidgetSet.onRemoved`) must be unmounted on the next
  `update` even if they are off-screen.

### W3: Vertical motion (`input/keyboard.ts`)

Replace the `else` branch at today's lines 114-123 (ArrowUp/ArrowDown
without modifier). Compute:
- `line = doc.lineAt(head)`;
- the target `n = line.number +/- 1`, clamped;
- `x = coordsAtPos(head).left`;
- `position = posAtCoords({ x, y: <center of target text row> })`.

Get the center of the target row by adding
`(target line top - current line top)` to the current caret rect center.
Both tops come from the heightmap, reached through `coordsAtPos` of the
target line start. Do not add a layout dependency to `KeyboardController`.

If `coordsAtPos` returns `null`, keep today's fallback (lines 118-121). Keep
`Mod`+Arrow, Home/End and Shift handling unchanged.

### W4: Tests

**`plugin/widgets.test.ts`**
- insert a line above an anchor -> its line increments;
- delete the anchor's line -> removed, and `onRemoved` fires;
- set the same key twice with a new height -> one widget, `revision`
  bumped;
- two widgets below the same line -> `entries` sums the heights;
- invalid height -> `RangeError`.

**`view/widget-layer.test.ts`** (jsdom, fake ledger)

Setup: 10k lines and widgets at lines 10, 5000 and 9990.
- viewport at top -> only line 10 is mounted;
- scroll to line 5000 -> 10 is unmounted (beyond 2x overscan) and 5000 is
  mounted;
- a canvas `draw` returning `false` -> called once, then not again until
  the spec changes;
- a widget offscreen inside the overscan -> mounted but `draw` not called;
- `dispose` -> ledger `usedBytes` is 0;
- a `dom` content mount callback receives the wrapper, and its cleanup runs
  on unmount;
- a non-interactive wrapper has `pointer-events: none`.

**`input/vertical-motion.test.ts`** (imitate
`editor/test/canvas/input.test.ts` setup, with an emaki `EditorCore` bridged
to a `TextLayout`):
- a 60 px block below line 2, caret on line 2 col 3, ArrowDown -> caret on
  line 3 col 3;
- ArrowUp from line 3 -> line 2;
- with no blocks, behavior is identical to before (lines 1 to 2 keep the
  column).

## Invariants

- With no widgets nothing is mounted, and keyboard behavior equals today's.
  vactr's `input.test.ts` arrow cases must still pass in EMK-VAC-SWITCH.
- Widget DOM never receives document text. The textarea bridge stays the
  only text input.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-WIDGETS/`.

1. `npm run check`: passes per rule 12b (errors outside own writePaths are
   drift).
2. `./node_modules/.bin/vitest run test/plugin test/view test/input`:
   exit 0, testsRun > 0. **This is the pass gate.** `test/layout` is owned
   by EMK-DECOR's files in this wave; run it informationally.
3. `./node_modules/.bin/vitest run`: informational per rule 12b. The wave-2
   join gate (rule 12a) is authoritative.
4. `wc -l` on every written file: each under 1000.

**mutationEvidence.** Make unmount use the 1x overscan bound -> the scroll
test still passes, but the hysteresis test (scroll by 1.5x overscan back and
forth: no remount) fails. Add that hysteresis test. Restore afterwards.

## Completion Criteria

- [x] W1-W4 delivered.
- [x] Barrels updated: `plugin/index.ts` exports `WidgetSet`, and
  `view/index.ts` exports `WidgetLayer` and `ViewHost`.
- [x] Scoped verification, type check and line counts pass; mutation evidence and hashes are logged. Full-suite result is informational under rule 12b.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.

### Session: 2026-11-01 (EMK-WIDGETS implementation)
**Tasks Completed**: W1-W4, barrels and scoped tests. Widget anchors map with TrackDel; layer virtualizes with hysteresis and accounts for canvas allocations; arrows use adjacent text-row coordinates.
**Verification**: scoped vitest 5/5 and npm run check pass (scoped-final-2.log, check-final-2.log); layout informational 15/15. Full vitest informational 92/92 before final redraw-state adjustment. Earlier check reported sibling test/language/fixture.test.ts drift, resolved on rerun. Hysteresis mutation failed as expected (mutation-2.log), and restored hashes match (mutation-hashes-2.txt). Review and serial wave join remain downstream.
