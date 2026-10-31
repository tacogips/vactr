# EMK-DECOR: marks, glow and line formats

**Status**: Ready (after EMK-CORE)
**Plan ID**: EMK-DECOR (wave 2; parallel with EMK-WIDGETS, EMK-LANG, EMK-DIFF)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 6.1-6.3; decisions EM-D5, EM-D6, EM-D7
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user asked for two formatting features: changing the format of specific
characters, which drives the vactr beat glow, and per-line formats. EMK-CORE
pinned the `MarkStyle`, `AnimatedMarkStyle`, `LineFormatStyle` and
`RenderInput` types. It also rendered only these: mark background,
underline, label and minWidth; animated background and label. This plan
adds the rest:

1. **Mark styles.** `color`, `opacity`, `fontWeight`, `fontStyle`, `strike`
   and `glow`.
2. **Line formats.** All `LineFormatStyle` fields.
3. **Stores.** Per-owner `MarkSet` and `LineFormatSet`, which map through
   edits, plus an animated-provider registry, combined in one
   `DecorationStore`.
4. **Precedence.** The composition order of design 6.3.

## Non-goals

- No change to the draw order of the decorations EMK-CORE already renders.
  `gpu.test.ts` depends on it.
- No widgets, no heightmap change, no plugin runtime (EMK-API wires the
  stores to plugins).
- No shader change (EM-D6).

## Dependencies

- **dependsOn**: EMK-CORE.
- Runs in parallel with EMK-WIDGETS, EMK-LANG and EMK-DIFF. Their writePaths
  are disjoint from this plan's.

## writePaths

Decoration stores:
- `emaki/src/decor/index.ts`, `emaki/src/decor/marks.ts`,
  `emaki/src/decor/line-formats.ts`, `emaki/src/decor/store.ts`

Rendering:
- `emaki/src/render/decoration-pass.ts`, `emaki/src/render/text-pass.ts`,
  `emaki/src/render/atlas.ts`

Layout:
- `emaki/src/layout/advances.ts`, `emaki/src/layout/layout.ts`

Tests:
- `emaki/test/decor/marks.test.ts`, `emaki/test/decor/line-formats.test.ts`
- `emaki/test/render/precedence.test.ts`, `emaki/test/render/glow.test.ts`,
  `emaki/test/render/font-variants.test.ts`

Generated, plus this plan:
- `emaki/node_modules`, `emaki/dist` (artifactRoots)
- `impl-plans/active/emaki-20-decor.md`
- `emaki/src/render/renderer.ts` (operator amendment, see sharedPaths note)

## sharedPaths

None. Read-only: `emaki/src/types.ts`, `emaki/src/render/types.ts`,
`emaki/src/layout/heightmap.ts`.

Operator amendment (after session 599 wave 2): `emaki/src/render/renderer.ts`
is now in this plan's writePaths (EMK-CORE is complete and no other wave-2
plan writes it). The wave-2 blocker showed that the renderer's text cache key
excludes `decorationsKey` and `rebuildTextSegments` receives only tokens after
visible lines are shaped, so color/opacity/font-variant marks and touched-line
reshaping need a renderer seam change. Make the minimal seam change: include
the text-affecting decoration key in the per-line text cache identity (only
lines whose text-affecting marks changed are re-tinted or reshaped; quad-only
marks such as background/glow/underline must NOT invalidate text), and pass
the resolved text styles into the text pass. Keep everything else in
renderer.ts delegated to the pass files, keep existing tests unchanged, and
add tests for: a color-only mark change re-tints only the touched line, a
weight/style mark reshapes only the touched line, and a glow-only change does
not rebuild text.

## Tasks

### D1: Stores (`decor/marks.ts`, `decor/line-formats.ts`, `decor/store.ts`)

**`MarkSet`**
- `set(marks)`: validate integers and `0 <= from <= to <= doc.length`.
  Throw `RangeError` otherwise, imitating
  `editor/src/code/surface.ts:annotate` lines 108-110. Freeze the copies.
- `clear()`.
- `map(changes: ChangeSet)`: `from` maps with `mapPos(from, 1)` and `to`
  with `mapPos(to, -1)`. A mark that becomes empty is dropped unless its
  style has `minWidth`.
- `get marks()`.

**`LineFormatSet`**
- Same `set`/`clear` behavior.
- `map(changes)`: the anchor maps with
  `changes.mapPos(pos, -1, MapMode.TrackDel)`. A `null` result removes the
  format.
- `resolve(doc): ResolvedLineFormat[]`: 0-based line via
  `doc.lineAt(pos).number - 1`.

**`DecorationStore`**
- `marks(owner)` and `lineFormats(owner)` are get-or-create, in owner
  registration order.
- `addAnimated(provider): () => void`.
- `map(changes)` maps every set.
- `collect(frameMs, doc)` returns
  `{ marks: Mark[]; animatedMarks: AnimatedMark[]; lineFormats: ResolvedLineFormat[]; key: string }`.
  - The order is owners in registration order, then array order.
  - `key` changes exactly when any static mark or line format changed. Use
    a revision counter bumped by `set`/`clear`/`map`-with-changes. Never
    stringify every mark.
  - Animated providers are called in registration order, and their results
    are concatenated.

Export the classes from `decor/index.ts`.

### D2: Text-layer styles (`render/text-pass.ts`, `layout/*`, `render/atlas.ts`)

**Color and opacity**
- Text color precedence: `palette.text` < token color < line format `color`
  < static mark `color` (the later mark wins).
- Opacity: line `opacity` x mark `opacity` multiplies the glyph tint alpha.
- The text cache key gains a per-line style signature for the visible lines
  only, built from overlapping color, opacity and font marks and line
  formats. Compute it while building segments; do not scan all marks per
  frame. Pre-sort marks by `from` once per `decorationsKey` change, then
  binary-search each line.
- A color-only change re-tints only the segments of the touched lines. It
  must not call layout shaping. `layout.stats.builds` must not increase
  (test D5).

**Font weight and style**
- Atlas cells and advances are keyed by a font-variant string
  (`normal|bold|italic|bold italic`) appended to today's cache key.
- `TextLayout` shapes a line with the variant runs of the marks and line
  formats that overlap it. Only those lines are reshaped when their variant
  set changes.
- Pass the variant ranges into `layout` through a new method
  `setFontVariants(lineRanges: ReadonlyMap<number, readonly { from: number; to: number; variant: string }[]>)`.
  `text-pass` calls it before shaping visible lines.

**Pitfall.** Today's monospace advances assume one font. Measure bold and
italic advances with the same `measureText` source (`advances.ts`), and
fall back to the regular advance when the measured width is not finite.

### D3: Quad-only styles (`render/decoration-pass.ts`)

**Strike.** A 1 px rect at the vertical middle of each range rect, in the
overlay layer, after the static underlines.

**Glow (EM-D6).** For each animated mark with `glow`, and also for static
marks with `glow`, add three rects around each range rect.
- Expand by `r/3`, `2r/3` and `r`, with `r` clamped to 1..12.
- Colors are `[c0, c1, c2, c3 x k]` for `k` = 0.45, 0.25, 0.12.
- They go in the background layer **after all animated mark backgrounds**:
  static glows first, then animated glows.
- Both are emitted in the per-frame (uncached) part of the background layer.
  Static glow quads must NOT go in the cached static prefix, because that
  prefix is drawn before the animated backgrounds. Static glows are
  re-emitted each frame from the static mark list. Pre-filter the marks that
  have `glow` once per `decorationsKey`.
- The first background draws keep the EMK-CORE order. Test D5 asserts the
  first four colors are unchanged when the playing mark also has a glow.

**Line formats**
- `background`: a full text-area width rect at `heightmap.lineTop(line)`,
  `lineHeight` tall. In the background layer it goes **before** static mark
  backgrounds and after the editor background (design 6.3).
- `bar`: a rect at the text-area left edge with the given width, drawn
  after line backgrounds.
- `gutterBackground`: in the gutter, before line numbers.
- `gutterMarker`: text drawn in the gutter's left 2 ch, after line numbers.
  Reuse the label run cache, imitating
  `editor/src/code/renderer.ts:labelRun`.

**Caching.** Static line-format quads and static-mark background quads are
part of the cached static background. Their dirtiness follows
`RenderInput.decorationsKey`. All glow quads, static and animated, are
rebuilt every frame after the animated backgrounds.

### D4: Precedence documentation

Add a block comment at the top of `decoration-pass.ts` listing the design
6.3 order verbatim.

### D5: Tests (`input -> expected`)

**`decor/marks.test.ts`**
- insert before a mark -> the mark shifts;
- insert at `from` -> the mark does not grow at the start;
- insert at `to` -> the mark does not grow at the end;
- delete covering the whole mark -> dropped;
- delete covering a `minWidth` mark -> kept as empty;
- `set` with `to > length` -> `RangeError`.

**`decor/line-formats.test.ts`**
- insert a line above -> the format follows its line;
- delete the line -> removed;
- `resolve` returns 0-based lines.

**`render/precedence.test.ts`** (recording GL, imitating
`emaki/test/render/decoration-order.test.ts`):
- token color, line color and mark color all set -> the glyph tint equals
  the mark color;
- line opacity 0.5 x mark opacity 0.5 -> alpha 0.25 x base;
- line background draws before static mark backgrounds, which draw before
  selection;
- a gutter marker draw comes after a line-number draw;
- a color-only mark change between two renders -> `layout.stats.builds` is
  unchanged, and only the touched line's segment is rewritten.

**`render/glow.test.ts`**
- the `decoration-order` input plus a glow on the playing mark -> the first
  four background colors are identical to the no-glow case;
- three extra quads follow, with alphas 0.225, 0.125 and 0.06 (playing alpha
  0.5) and rects expanded by `r/3`, `2r/3` and `r`;
- the glow color is never in the text layer.

**`render/font-variants.test.ts`**
- a bold mark on line 3 -> only line 3 is reshaped (`layout.stats.builds`
  delta = 1);
- the atlas request key contains `bold`;
- removing the mark -> line 3 is reshaped back.

## Invariants

- Inputs without the new fields render byte-identically to EMK-CORE.
  `emaki/test/render/decoration-order.test.ts` stays green, unchanged.
- No per-frame work proportional to the total mark count. Animated providers
  are bounded by their own output.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-DECOR/`.

1. `npm run check`: passes per rule 12b. It exits 0, or every error lies
   outside this plan's writePaths, recorded as drift.
2. `./node_modules/.bin/vitest run test/decor test/render test/layout`:
   exit 0, testsRun > 0, failures 0. **This is the pass gate.**
3. `./node_modules/.bin/vitest run`: informational per rule 12b. The
   package-wide result is the wave-2 join gate (rule 12a).
4. `wc -l` on every written file: each under 1000.

**mutationEvidence.** Swap the line-background and static-mark background
order -> `precedence.test.ts` fails. Restore and record hashes.

## Completion Criteria

- [ ] D1-D5 delivered. The `decor/index.ts` barrel exports `MarkSet`,
  `LineFormatSet` and `DecorationStore`.
- [ ] Verification steps 1-4 pass. Mutation evidence is recorded.
- [ ] `decoration-order.test.ts` is unchanged and passing.
- [ ] The Progress Log has hashes.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
