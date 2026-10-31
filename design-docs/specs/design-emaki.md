# emaki: Standalone Canvas Editor Library and vactr Adoption

**Status:** Proposed (2026-10-31). Author proposal for adversarial review. No
implementation, performance or device acceptance is claimed here.

**Issue reference:** workflowInput issue "Extract vactr's canvas editor into
the standalone emaki library (staged at emaki/) with plugins, range/line
formats, virtualized JSON rendering and a realtime diff sample; vactr consumes
it" (user request dated 2026-10-31; no GitHub issue number). Workflow
execution `opus-terra-design-and-implement-review-loop-session-586`, intake
`comm-011210`. Target repository for the operator's later copy:
`https://github.com/tacogips/emaki` at `0d49bb0` (LICENSE, README.md,
.gitignore only).

**Branch:** `wf/emaki`, base `a6e5bea`. **Worktree:**
`/Users/taco/gits/tacogips/vactr-worktrees/wf-emaki`.

**Related design.** The canvas editor is specified in `design-implementation.md`
15.3 through 15.3.8.16. Measurement protocol and thresholds are in 15.3.8.8,
15.3.8.10, 15.3.8.11 and 15.3.8.14. Text rendering is in
`design-code-pane-text-rendering.md`. Vim is in `design-vim-mode.md` and
`design-vim-keymap-hints-hop.md`. Editor visualizers are in `design-visual.md`.
This document changes where the engine lives and adds new engine features. It
does not change any existing vactr behavior, threshold or measurement rule,
except for the additions listed in section 1.

---

## 1. Scope

**In scope**

1. A new framework-agnostic TypeScript ESM package `emaki`, staged at the
   top-level `emaki/` directory. Its layout is identical to the final
   `tacogips/emaki` repository.
2. Moving the editor engine out of `editor/src/code` into `emaki/src`, with
   the unit tests that exercise only engine code (section 4).
3. New engine features: marks, line formats, a plugin host, block and
   end-of-line widgets on a heightmap, an incremental tokenization provider
   interface, a built-in JSON language, and a diff module (sections 6 to 10).
4. Three demo pages, a perf script and a diff e2e script inside `emaki/`
   (sections 11 and 12).
5. vactr adoption: a `file:../emaki` dependency, vact syntax as an emaki
   language provider, beat glow and eval flash as emaki marks, inline
   sequencer and oscilloscope block-widget plugins, and a persisted show/hide
   toggle (section 13).
6. Boundary enforcement tests in both directions (section 14).

**Non-goals**

- No change to Rust sources, the session protocol, the scheduler, language
  semantics or the tree-sitter grammar.
- No soft wrap, no bidirectional text support beyond today's (mixed RTL stays
  "degraded"), no new renderer backend.
- Plugins cannot write the document. The host API is read-only toward the
  document (section 7.2).
- No inline display of bus analyzer units: `levels.analyzers[]` carry no
  source span (section 13.5).
- No publication to npm. The operator copies `emaki/` to `tacogips/emaki`
  and switches the dependency to `github:tacogips/emaki#<sha>` after this run.

## 2. Baseline check (repository state at `a6e5bea`)

Every fact below was read in the current source. Where the intake differs from
the repository, the repository wins and the difference is recorded.

| # | Fact | Where |
|---|------|-------|
| B1 | `editor/src/code` has 11,924 TS/TSX lines across its top-level files and the 24 files in `vim/` (not counting `vim/presets/`). | `wc -l editor/src/code/*.ts editor/src/code/*.tsx editor/src/code/vim/*` |
| B2 | **The DOM renderer backend does not exist.** It was removed on 2026-10-08 (`design-dom-renderer.md` status line; `design-renderer-comparison.md`; code only in commit `56601c2`). No `?renderer=dom` path exists in `editor/src`. The intake's "DOM renderer backend" item and the "keep `?renderer=dom` working" constraint are therefore empty. Nothing moves, and nothing has to keep working. | `grep -rn renderer=dom editor/src` returns nothing; `editor/test/e2e/README.md` line 57 |
| B3 | Rendering assumes a fixed line height: `line * lineHeight` in `layout.ts` (lines 361, 480, 485, 499-500), `view-host.ts` (80, 89), `renderer.ts` (253) and `mount.ts` (254-255). | source |
| B4 | The renderer has fixed annotation kinds: `syntax`, `selection`, `unapplied`, `playing`, `eval`, `diagnostic`, `composition`, `call-head`, `binding`, `momentary` (`CodeAnnotation.kind` in `editor/src/app/apis.ts:43`). `playing` is a background quad `[0.7, 0.58, 0.26, 0.5]`. `eval` is `[0.4, 0.64, 0.72, 0.5]`, or `[0.8, 0.24, 0.32, 0.5]` for error classes (`renderer.ts:273-276`). | source |
| B5 | `editor/test/canvas/gpu.test.ts` asserts exact draw colors and order: editor background, selection, playing, eval as the first four draws (line 400). It also asserts the playing and selection colors in the background layer and never in the text layer (lines 427-449). | test |
| B6 | Engine files import vactr modules: `../app/apis` types (`CodeRect`, `CodeRange`, `CodeAnnotation`, `CodeSurface`, `ResourceBudget`), `keyboard.ts` imports `../ui/stop-keys`, `renderer.ts` imports `./unapplied-settings`, `momentary-pointer.ts` imports `../bind/momentary`, and `history.ts`, `line-bytes.ts` and `sync.ts` import protocol `Span`, `DocSync`, `ByteChange` and `utf8`. | `grep -n "^import" editor/src/code/*.ts` |
| B7 | `vim/` has no imports outside `vim/` except `@codemirror/state`, but its **content is vactr-specific**. `vim/actions.ts:1-17` defines `ActionId` with `vactr.evaluate`, `vactr.evaluateAll`, `vactr.hush`, `vactr.format`, `vactr.diagnostic.next/prev` and `vactr.completion`. `vim/types.ts:26` defines `VimActions` (evalAtCursor, evalAll, hush, format, diagnosticNext/Prev, openCompletion), which `controller.ts:266-275` dispatches. `vim/ex.ts:110-115` maps `:eval`, `:evalall`, `:run`, `:hush` and `:format`. `controller.ts:642` emits "vactr syncs the ...". `vim/presets/mise-darwin.json` binds `vactr.*` actions. Persisted user keymaps (`vactr.vimKeymap`, Tauri `vim-keymap.json`) reference these IDs. Tests asserting this content: `test/code/vim/actions`, `keymap-schema`, `keymap-controller`, `presets`, `controller`, `search-ex`, `host-keymap`, `controller-io` and the helper `host.ts`. `purity.test.ts` reads the literal path `${cwd}/src/code/vim`. Engine files use only two vim types: `CursorShape` (`renderer.ts:11`, `cursor-shape.ts:2`) and `VimInputHook` (`input.ts:8`). | `grep -rln vactr editor/src/code/vim editor/test/code/vim`; `grep -n "vim" editor/src/code/{renderer,cursor-shape,input}.ts` |
| B17 | `pointer.ts:75` sets the event expando `vactrNumericGesture`. `momentary-pointer.ts:34,46` also sets it, and `params/mount.ts:166` reads it. No test references it. | `grep -rn vactrNumericGesture editor/src editor/test` |
| B18 | `keyboard.ts:89-93` branches on `fallbackStopShortcut(event)`, which returns `'stop-all' \| 'cut' \| null` (`ui/stop-keys.ts:20`). The result calls `options.stopAll` or `options.cut`. | source |
| B8 | `surface.ts` mixes the generic EditorState authority (dispatch, bounded undo, keymaps, composition deferral, bridge) with vactr parts: `DocumentSync` wiring, owner annotations, momentary and numeric drag providers, and animated annotation sources. `history.ts` mixes generic undo-byte trimming (`retainedBytes`, `trimUndoHistory`, `HISTORY_UNDO_BYTES`, `touches`) with the wire `RevisionHistory`. | source |
| B9 | `palette.ts` reads vactr CSS tokens (`--vt-*`) and maps `vact-tok-*` classes. `renderer.ts` exports `GPU_TOKEN_COLORS` keyed by `vact-tok-*`. | source |
| B10 | `format.ts` and `format-core.ts` are the vact source-formatter keymap (vactr glue). `perf-hook.ts` imports protocol types (vactr glue). `view-host.ts` is generic scroll/viewport code. | source |
| B11 | Existing canvas gates (`design-canvas-editor-evidence.md`, run-001): input latency p95 <= 50 ms, text-dirty frame work p95 <= 8 ms (target 4, recorded), animation frame work p95 <= 8 ms, frame interval p95 <= 20 ms. Baselines: Chromium input 15.8, text-dirty 4.3, frame interval 17.4; WebKit 16, 3, 18. **The intake's "text work p95 <= 4 ms" is the recorded target, not the gate.** The Chromium baseline (4.3 ms) already exceeds 4 ms. This design keeps the repository gate and records 4 ms as a target (decision EM-D14). | evidence doc lines 20-79 |
| B12 | The step grid's `CycleSteps` (`editor/src/params/grid.ts:22`) is a pure class. The file also imports solid-js for `StepGrid`. `playing` events carry `slot`, `beat`, `dur` and an optional `src: {file, span (UTF-8 bytes), doc_revision, form_gen}` (`protocol/types.ts:386-395`). | source |
| B13 | Oscilloscope sources: the `#@ editor {"visualizers":[{"kind":"oscilloscope",...}]}` directive is parsed per line by `editor/src/visual/settings.ts` (`PREFIX = /^\s*#@\s+editor(?:\s|$)/`). It draws master `levels.levels[].waveform` (256 samples) through `drawScope` in `visual/scopes.ts`. `WireAnalyzer` has `{bus, kind, id, cells}` and no source span. | source |
| B14 | Editor settings persist in `localStorage` under `vactr.*` keys (`vactr.vimMode`, `vactr.unappliedColor`). The editor options UI is `EditorOptions` in `editor/src/ui/transport-view.tsx:279` (vim toggle, unapplied color). | source |
| B15 | The vitest baseline is 2384 tests (intake). Both projects use TypeScript 7.0.2 with `moduleResolution: bundler`, `strict`, `isolatedModules`, `types: []` (`editor/tsconfig.json`). | config |
| B16 | The editor e2e harness serves `editor/dist`, launches Chromium with `--mute-audio`, and writes gating evidence only with `--write-evidence` under a quiet-host lock (`editor/test/e2e/README.md`). | README |

## 3. Architecture overview

```
+---------------------------- emaki (package, no vactr imports) ----------------------------+
| core/      EditorCore (EditorState authority, bounded undo, keymaps, composition defer)  |
| layout/    TextLayout, AdvanceTable, Heightmap                                            |
| render/    CanvasRenderer (WebGL2), GlyphAtlas, geometry, segments, palette, decoration  |
|            pass (marks, line formats, glow, labels), ResourceLedger, FrameScheduler       |
| input/     InputController, KeyboardController, PointerController, AccessibilityBridge   |
| view/      ViewHost (scroll, viewport, caret reveal), WidgetLayer (DOM/canvas widgets)    |
| decor/     MarkSet, AnimatedMarks, LineFormatSet, precedence/compose                      |
| plugin/    PluginHost (lifecycle, narrow host API)                                        |
| language/  LanguageProvider types, IncrementalHighlighter (line checkpoints), json       |
| diff/      line diff (Myers, prefix/suffix trim, cost cap), word diff, split alignment    |
| editor.ts  createEditor(host, options): composition of the parts above                    |
+--------------------------------------------------------------------------------------------+
        ^ "emaki", "emaki/engine", "emaki/diff", "emaki/json" (public entries only)
+---------------------------- vactr editor/src/code (glue) ---------------------------------+
| mount.ts composes emaki/engine parts with DocumentSync, HighlightScheduler, eval,          |
| transport, diagnostics, completion, samples, the Vim mode (vim/** core, settings, keymap   |
| persistence, views; attached through the emaki modal key hook), perf hook,                 |
| vact language provider, beat-glow/eval-flash mark plugin, inline sequencer and scope      |
| plugins, inline-widget toggle. CodeSurface contract (apis.ts) stays frozen.                |
+--------------------------------------------------------------------------------------------+
```

**Two public levels (EM-D2).** `emaki` exports `createEditor(host, options)`
for generic consumers and demos. `emaki/engine` exports the building blocks
that `createEditor` is composed from. vactr composes these blocks in
`mount.ts`, as it does today. This keeps its frame loop (audible clock,
`setActive` reasons, perf phases, `afterPresent`) and its perf budgets
unchanged. `createEditor` is a small composition, about 300 lines, of the same
blocks, so the demos prove the same code that vactr uses. Moving vactr onto
`createEditor` would mean rewriting the frame loop and re-baselining every
canvas gate. That is not required by the issue and is rejected.

## 4. File disposition

### 4.1 Engine files that move into `emaki/src`

| vactr file | emaki destination | Decoupling required |
|------------|-------------------|---------------------|
| `atlas.ts` | `src/render/atlas.ts` | none |
| `frame.ts` | `src/render/frame.ts` | none |
| `geometry.ts` | `src/render/geometry.ts` | `CodeRect` becomes emaki `Rect` type |
| `segments.ts` | `src/render/segments.ts` | none |
| `palette.ts` | `src/render/palette.ts` | Generic `Palette` and `readPalette(root, properties)` that takes a property map. The `--vt-*` and `vact-tok-*` maps and the legacy token values stay in vactr `editor/src/code/theme.ts` |
| `cursor-shape.ts` | `src/render/cursor-shape.ts` | emaki declares its own `CursorShape = 'bar' \| 'block' \| 'underline'` in `src/types.ts`. vactr's `vim/types.ts` `CursorShape` is structurally identical and stays |
| `resources.ts` | `src/render/resources.ts` | `ResourceBudget` declared in emaki. vactr's `apis.ts` interface stays and is satisfied structurally |
| `renderer.ts` (engine body) | `src/render/renderer.ts`, `src/render/text-pass.ts`, `src/render/decoration-pass.ts` | Fixed kinds replaced by the decoration input (section 6). No `unapplied-settings` import. `GPU_TOKEN_COLORS` stays in vactr `theme.ts`. Split so each file stays under 1000 lines |
| `renderer-types.ts` | `src/render/types.ts` | none |
| `layout.ts`, `advances.ts` | `src/layout/layout.ts`, `src/layout/advances.ts` | Every `line * lineHeight` goes through `Heightmap` (section 8). Font-variant advances for bold and italic |
| (new) | `src/layout/heightmap.ts` | section 8 |
| `input.ts`, `keyboard.ts`, `pointer.ts`, `accessibility.ts` | `src/input/*.ts` | **keyboard.ts (EM-D18):** the `fallbackStopShortcut` import is replaced by an injected option `fallbackShortcut?(event): 'stop-all' \| 'cut' \| null`. The existing `stopAll` and `cut` options and the branch at `keyboard.ts:89-93` are kept unchanged, so the discriminant still selects stop-all vs cut. emaki only names the two results. Their meaning (gentle stop vs hush) stays in vactr. **input.ts:** the `VimInputHook` type becomes the emaki `ModalKeyHook` interface, with the same members (`handleKey`, `allowsTextInput`, `overwriteRange`, `noteCompositionDiscarded`, optional `flushPendingInput`), and `setVim` becomes `setModalHook`. vactr's Vim controller implements it structurally. **pointer.ts (EM-D19):** the `vactrNumericGesture` expando becomes emaki-owned helpers `markDragGesture(event)` and `isDragGesture(event)` in `emaki/engine`, backed by a module WeakSet. `momentary-pointer.ts:34,46` and `params/mount.ts:166` switch to the helpers. No test reads the flag (B17). **All:** `CodeSurface` and `CodeAnnotation` types become emaki `EditorCore` and `Decoration` types. Vertical motion becomes line-index based (section 8.4) |
| `view-host.ts` | `src/view/view-host.ts` | Heightmap-based scroll range and caret reveal |
| `surface.ts` (generic part) | `src/core/editor-core.ts` | Keeps dispatch, bounded undo, keymaps, composition deferral, bridge, pointer/blur/composition listeners and drag-gesture providers. Adds a synchronous `onTransaction(tr)` hook that runs inside dispatch before subscribers. vactr's `DocumentSync.apply` attaches here, so it still runs "exactly once, before any surface subscriber" |
| `history.ts` (generic part) | `src/core/undo-budget.ts` | `retainedBytes`, `trimUndoHistory`, `HISTORY_UNDO_BYTES`, `touches` |

### 4.2 Files that stay in vactr

These files stay and are rewritten only where they used moved modules:
`mount.ts`, `code-view.tsx`, `code.css`, `eval.ts`, `transport.ts`,
`highlight.ts`, `unapplied.ts`, `unapplied-settings.ts`, `samples.ts`,
`sample-view.tsx`, `completion*.ts`, `diagnostics.ts`, `syntax.ts`,
`syntax-core.ts`, `syntax-worker*.ts`, `language.ts`, `format.ts`,
`format-core.ts`, `tool-wasm.ts`, `perf-hook.ts`, `momentary-pointer.ts`,
`sync.ts`, `line-bytes.ts`, `vim-mount.ts`, `vim-settings.ts`,
`vim-clipboard.ts`, `vim-keymap-store.ts`, `vim-*-view.tsx`, and **all of
`vim/**` including `presets/`**.

**Vim stays in vactr (EM-D17).** The intake left the Vim boundary to the
design ("design decides, recommended" optional emaki module). B7 shows that
the core carries vactr app semantics in its action IDs, host action
interface, Ex commands, message text and presets. Persisted user keymaps
depend on the `vactr.*` IDs, and nine test files assert them.

Moving Vim would need a new host action-table contract, test-helper changes
and an emaki boundary exception. None of that is required by the user's
request. So `vim/**` stays unchanged in `editor/src/code/vim`, and emaki ships
no Vim module in this run.

The engine keeps exactly today's Vim seams as generic interfaces:
- `ModalKeyHook` on the input controller;
- `CursorShape` and `cursorEnd` in the render input;
- `hopLabels` and `hopRevision` overlay labels.

`vim-mount.ts` wires vactr's Vim controller to them. A later extraction of a
generic Vim module into emaki remains possible, and is EQ6.

Three files stay with reduced content:

- `history.ts` keeps only `RevisionHistory`, `Range16` and the wire pieces.
  It imports the undo-budget helpers from `emaki/engine`.
- `surface.ts` becomes the vactr `CodeSurface` adapter (the contract in
  `apis.ts` is frozen by 15.3.8.2). It wraps an emaki `EditorCore` and keeps
  the vactr-only parts: `sync`, owner `annotate`/`annotationRanges` with
  today's drop-on-touch mapping, animated sources, and momentary providers.
  Numeric drag uses the emaki drag-gesture provider hook.
- `renderer.ts` becomes an adapter of at most 150 lines. Its class
  `CanvasRenderer` extends the emaki renderer and overrides only
  `render(feedback)`. The override maps vactr `CodeAnnotation` kinds to emaki
  decorations through `decorations.ts`. It contains no GL calls, shader
  sources or atlas code. A vactr boundary test enforces this (section 14.2).
  The adapter exists so the vactr annotation semantics (B4, B5) keep one
  definition, and the vactr renderer tests keep their assertions unchanged.

### 4.3 New vactr files

| File | Purpose |
|------|---------|
| `editor/src/code/decorations.ts` | `CodeAnnotation` to emaki decoration mapping. Mark style constants `PLAYING_MARK`, `EVAL_MARK`, `EVAL_ERROR_MARK`, `UNAPPLIED_MARK(color)`, `DIAGNOSTIC_MARK`, `CALL_HEAD_MARK`, `BINDING_LABEL`, `MOMENTARY_LABEL` |
| `editor/src/code/theme.ts` | vact token classes, `--vt-*` property maps, legacy fallback tokens, `GPU_TOKEN_COLORS` |
| `editor/src/code/vact-language.ts` | Wraps the existing `SpanProvider`s as an emaki span-kind language provider (section 9.3) |
| `editor/src/code/beat-glow.ts` | Plugin that supplies the playing-step and eval-flash animated marks |
| `editor/src/code/inline/anchors.ts` | Anchor resolution for slot and scope widgets (sections 13.4 and 13.5) |
| `editor/src/code/inline/sequencer.ts` | Inline sequencer block-widget plugin |
| `editor/src/code/inline/scope.ts` | Inline oscilloscope block-widget plugin |
| `editor/src/code/inline-settings.ts` | `vactr.inlineWidgets` read/write |

### 4.4 Test disposition rule

A test file **moves** to `emaki/test` when all of the following hold:

- every `src/` import it has is a module that moves (section 4.1);
- it uses no vactr annotation kind;
- it uses no protocol, store or session object.

Otherwise it **stays** in vactr. Its import paths are updated to the vactr
adapter or to `emaki/engine`. Moving a test changes only import specifiers.
No `expect` line, fixture or tolerance is changed.

Disposition already fixed by the rule:

- **Move:** `test/code/atlas-cells`, `glyph-placement`, `text-cells`;
  `test/canvas/frame`, `cursor-shape`.
- **Stay:**
  - `test/canvas/`: `gpu`, `mount`, `first-viewport`, `state`, `input*`,
    `view-host`, `edit-cost`, `vim-*mount*`, `vim-input`,
    `vim-keymap-input`, `vim-keymap-view`, `vim-hints-view`,
    `vim-status-keymap`, `contracts`, `clock`, `momentary-pointer`.
  - `test/code/`: `history`, `sync`, `eval`, `highlight`, `reconcile`,
    `diagnostics`, `completion*`, `format`, `samples`, `syntax*`,
    `language`, `transport`, `tool-wasm`, `perf-hook-*`, `vim-clipboard`,
    `vim-keymap-store`, `vim-settings`.
  - **Every file under `test/code/vim/`** (32 entries, including `host.ts`
    and `purity.test.ts`), because `vim/**` stays in place (EM-D17). Their
    imports of `src/code/vim/*` are unchanged. `purity.test.ts` keeps its
    `${cwd}/src/code/vim` literal. Files that import moved engine modules
    (`controller-io`, via `input`/`surface`) only change those import
    specifiers.

  No test file containing the string `vactr` moves to emaki.
- **Decided by the plan:** `test/code/renderer-layers` and
  `test/canvas/renderer-hop`, using the same rule.

Each project keeps its own copy of shared test helpers such as
`recording-gl.ts`. Test helpers are not engine code, and vactr may not import
emaki test internals.

**Count reconciliation (EM-D15).** The plan records
`tmp/<evidenceRoot>/<planId>/test-disposition.json`. For every moved file it
lists the vactr path, the emaki path, and the test count before and after.
The gate is: vactr vitest count + emaki vitest count >= 2384 + new tests, and
every moved file's count is unchanged.

## 5. Public API

`emaki/README.md` documents everything in this section. Names are normative.
Exact TypeScript signatures are deliverables of the implementation plans.

### 5.1 `emaki` (index)

- `createEditor(host: HTMLElement, options: EditorOptions): EditorHandle`.
- `EditorOptions`:
  - `doc` (string)
  - `language` (`LanguageProvider`)
  - `plugins` (`EmakiPlugin[]`)
  - `theme` (`{ palette?: Partial<Palette>; tokens?: Record<tag, color> }`)
  - `font` (`{ family, size, lineHeight, baseline }`)
  - `readOnly` (boolean; user edits are ignored, selection works)
  - `lineNumbers` (boolean, default true)
  - `extensions` (CodeMirror state extensions)
  - `modalKeys` (an optional `ModalKeyHook`, the same seam vactr's Vim uses)
  - test injection: `gl`, `frameHost`, `createCanvas`
- `EditorHandle`:
  - `state`, `dispatch(spec)`, `undo()`, `redo()`, `setDoc(text)`
  - `onUpdate(cb)`, `onScroll(cb)`
  - `scrollTop` (get), `scrollTo({ top } | { pos })`
  - `lineTop(line)`, `lineAtHeight(y)`
  - `addPlugin(plugin)` returns a remover
  - `focus()`, `requestFrame()`, `stats`, `destroy()`
- Re-exported types: `Mark`, `MarkStyle`, `AnimatedMarkStyle`, `LineFormat`,
  `BlockWidgetSpec`, `EolWidgetSpec`, `WidgetContent`, `EmakiPlugin`,
  `PluginHost`, `LanguageProvider`, `Rect`, `Range`, `Palette`.

### 5.2 `emaki/engine`

Exports `EditorCore`, `TextLayout`, `Heightmap`, `CanvasRenderer`,
`GlyphAtlas`, `ResourceLedger`, `FrameScheduler`, `PhaseTimer`, `ViewHost`,
`InputController`, `KeyboardController`, `PointerController`,
`AccessibilityBridge`, `DecorationStore`, `PluginHost`, `WidgetLayer`,
`IncrementalHighlighter`, `readPalette`, `createMeasureContext`, the
undo-budget helpers, `markDragGesture`, `isDragGesture`, and the types
including `ModalKeyHook` and `CursorShape`.

### 5.3 Other entries

- `emaki/diff` exports `diffLines`, `diffWords` and `alignSplit`.
- `emaki/json` exports `jsonLanguage` and `generateJsonFixture`. The fixture
  generator is exported so the demo, the perf script and the tests share one
  definition.

## 6. Formatting model

### 6.1 Marks (character ranges)

`Mark = { from, to, style: MarkStyle }`. `MarkStyle` has these optional fields:

| Field | Values |
|-------|--------|
| `color` | rgba |
| `background` | rgba |
| `fontWeight` | `normal` or `bold` |
| `fontStyle` | `normal` or `italic` |
| `underline` | `{ color, thickness 1..3 }` |
| `strike` | `{ color }` |
| `opacity` | 0..1 |
| `glow` | `{ color, radius 1..12 px }` |
| `label` | `{ text <= 64 chars, fill, color }`, a badge after `to` |
| `minWidth` | px, so empty ranges remain visible |

There are two mark sources:

- **Static marks.** `host.marks(owner).set(marks)` stores them per owner. They
  are mapped through every document change: `from` with assoc +1, `to` with
  assoc -1. A mark that becomes empty is dropped unless it has `minWidth`.
  `set` replaces the owner's marks. `clear()` removes them.
- **Animated marks.** `host.animatedMarks(provider)` registers a provider that
  `provider(frameMs)` calls on every frame while the plugin is active. The
  result is not retained. `AnimatedMarkStyle` allows only `background`,
  `glow`, `underline`, `strike`, `label` and `minWidth`. The type enforces this
  at compile time. Animated marks never touch the retained text layer, so
  updating them at audio rate costs only quad generation for the visible
  ranges.

**Relayout rule (EM-D5).**

- Only `fontWeight` and `fontStyle` change shaping, and only for the lines
  they overlap. Atlas cells and advances are keyed by font variant.
- `color` and `opacity` re-tint only the retained text segments they overlap.
  A per-line style signature joins the segment cache key, like today's syntax
  key. There is no reshaping.
- `background`, `glow`, `underline`, `strike` and `label` generate quads only.

**Glow (EM-D6).** A glow draws three concentric rects around each range rect,
expanded by r/3, 2r/3 and r. Their alpha is `color.a x [0.45, 0.25, 0.12]`.
They go in the background layer after all mark backgrounds and before the
text layer, so glyphs stay on top. There is no shader change. This works
identically in WebKit and Chromium.

### 6.2 Line formats

`LineFormat = { pos, style }` applies to the line that contains `pos`. The
anchor maps with assoc -1. A format is removed when its line is deleted
(`MapMode.TrackDel`). `LineFormatStyle` has these optional fields:

- `background` (full text-area width)
- `color`, `fontWeight`, `fontStyle`, `opacity`
- `bar` (`{ color, width 1..6 px }` at the left edge of the text area)
- `gutterMarker` (`{ text <= 2 chars, color }`)
- `gutterBackground`

`host.lineFormats(owner).set(...)` stores them per owner.

### 6.3 Precedence (EM-D7)

Within one category, the later entry wins (higher values in the list below
draw later or override). Providers are ordered by plugin registration, and
specs within a provider by array order.

- **Text color:** default text < syntax token color < line `color` < static
  mark `color`.
- **Font weight and style:** default < line format < static mark.
- **Opacity:** line opacity x mark opacity (multiplied).
- **Background layer draw order:** editor background, line backgrounds, static
  mark backgrounds, selection, animated mark backgrounds (input order), glow
  halos.
- **Overlay layer draw order:** static underline and strike, composition
  underline, cursor, static labels, Vim hop labels, touch handles, animated
  labels.
- **Gutter:** gutter background, line numbers, gutter markers. The left bar is
  drawn at the text-area edge.

This order reproduces today's draw order exactly when vactr maps its kinds
(section 13.2). B5 assertions: the first four draws are background,
selection, playing and eval. Glow quads come after them. The playing color
never appears in the text layer.

## 7. Plugin model

### 7.1 Lifecycle

`EmakiPlugin = { name, attach(host): PluginInstance | void }`. A
`PluginInstance` has these optional hooks:

- `docChanged(update)`: runs after the transaction, with `changes`, `state`
  and `docChanged`.
- `viewportChanged({ fromLine, toLine, top, height })`.
- `frame(frameMs)`: return `true` to keep the plugin active for continuous
  frames.
- `destroy()`.

Hooks run in this order on the frame that observes an edit: `docChanged`,
then `viewportChanged` (if changed), then `frame`. A hook that throws is
caught, reported once through `host.reportError`, and the plugin is detached.
The editor keeps working.

### 7.2 Host API (narrow)

`PluginHost` provides:

- `state` (read-only `EditorState`)
- `marks(owner)`, `animatedMarks(provider)`, `lineFormats(owner)`, `widgets(owner)`
- `requestFrame()`, `setActive(active)`
- `coordsAtPos(pos)`, `lineTop(line)`, `viewport`
- `theme` (resolved palette and token colors)
- `reportError`

It has no `dispatch` and no access to the input bridge. Plugins display state.
They do not edit. The vactr plugins and the diff demo need nothing more.

## 8. Widgets, heightmap and virtualization

### 8.1 Widget specs

- `BlockWidgetSpec = { key, pos, side: 'above' | 'below', height, content,
  interactive? }`. It spans the full text-area width and sits above or below
  the line that contains `pos`. `height` is in CSS px and is fixed until the
  spec is set again.
- `EolWidgetSpec = { key, pos, width, content }`. It is a box after the end of
  the line's text with a 1 ch gap, as tall as one line. It does not change
  text layout or line height.
- `WidgetContent` is one of:
  - `{ kind: 'dom', mount(el: HTMLElement): () => void }`, for arbitrary DOM
    such as `<video>`, images or HTML;
  - `{ kind: 'canvas', draw(ctx: CanvasRenderingContext2D, info: { width, height, dpr, frameMs }): boolean }`.
    `draw` returns `true` to keep animating.
- `host.widgets(owner).set(specs)` reconciles by `key`. A widget with an
  unchanged key and content kind keeps its mounted element. Changed `height`
  or `side` updates the heightmap. Anchors map with assoc -1. A widget is
  destroyed (unmount callback, element removed) when `MapMode.TrackDel`
  reports its anchor deleted, when its owner sets a list without its key,
  when the plugin is removed, or when the editor is destroyed.

### 8.2 Heightmap (EM-D8)

Text lines share one height (`font.lineHeight`). Block widgets add extra height
before or after their line. The `Heightmap` keeps a sorted array of
`{ line, above, below }` entries, one per line that has block widgets, with
prefix sums of the extra height.

- `lineTop(line) = line * lineHeight + extraBefore(line)`. This is a binary
  search over the entries: O(log w) with w = number of lines with widgets, and
  w <= n.
- `lineAtHeight(y)` binary-searches the entries, then divides by
  `lineHeight`. It returns `{ line, region: 'text' | 'above' | 'below' }`.
- `totalHeight` = `lines * lineHeight` + all extra height.
- Rebuilding after an edit or a widget set change costs O(w log w). It does not
  depend on document length. With no widgets, every query is O(1)
  arithmetic.

### 8.3 Virtualization

- **Text.** The overscan rule is unchanged: one viewport height above and
  below. Visible lines come from `lineAtHeight` at the top and bottom of the
  overscanned viewport.
- **Widgets.** A widget is **mounted** when its rect intersects the
  overscanned viewport. It is **unmounted** (DOM removed, canvas released)
  when its rect is outside viewport +/- 2 x overscan. The gap between the two
  bounds avoids mount/unmount churn while scrolling.
- **Canvas widgets.** `draw` runs only while the widget is mounted and its rect
  intersects the actual viewport (not the overscan), and only while it
  returned `true` last time or its spec changed.
- **WidgetLayer.** This is one absolutely positioned container above the code
  canvas, clipped to the text area. In the frame callback, after the WebGL
  render, it receives a `translate` equal to `-scrollTop` and
  `-scrollLeft` for EOL widgets (block widgets do not scroll horizontally).
  Canvas and DOM content therefore move in the same frame.
- **Canvas backing.** Each canvas widget has its own `<canvas>`, sized
  `width x dpr`. Its pixels are charged to the `ResourceLedger` and released
  on unmount.

### 8.4 Caret, selection, hit testing and motion

- `posAtCoords` in a block band maps to the band's anchor line using the same
  x rule as a text row. A drag that crosses a band therefore selects
  continuously. Pointer events on `interactive` DOM content go to the widget,
  because the widget element is above the canvas. Non-interactive widget
  elements have `pointer-events: none`.
- ArrowUp/ArrowDown and Vim `j`/`k` move by line index with the goal column
  kept, so bands are skipped. PageUp and PageDown move by viewport height and
  then snap to the nearest text line in the direction of motion.
- Selection and composition rects are drawn per text row at `lineTop`.
  Bands get no selection paint.
- Caret reveal (`ViewHost.scrollCaret`), the completion popup anchor and the
  diagnostic tooltip all use `coordsAtPos`, which uses the heightmap.
- Gutter: line numbers sit at text rows. Bands have an empty gutter.
- Accessibility: unchanged. The textarea bridge window is text only. Widget
  elements are `aria-hidden` unless the spec supplies `label`.

## 9. Language providers and incremental tokenization

### 9.1 Provider interface (EM-D9)

`LanguageProvider` is a union of two kinds.

- **Line kind:** `{ kind: 'lines', name, startState(), tokenizeLine(text,
  state): { tokens: { from, to, tag }[], state }, sameState(a, b) }`. emaki
  drives it through `IncrementalHighlighter`.
- **Span kind:** `{ kind: 'spans', spans(state, from, to, limit): { spans,
  truncated }, noteChanges?(changes, state), dispose?() }`, plus a host-given
  `invalidate()` callback. This is the shape of vactr's existing
  `SpanProvider` (tree-sitter worker, main-thread tree-sitter, fallback).
  Providers that compute asynchronously call `invalidate()` when results
  arrive.

Tags are strings. The theme maps tags to colors. emaki contains no vact tags.

### 9.2 IncrementalHighlighter (line kind)

- **Checkpoints.** One state checkpoint is stored every 32 lines, in an array.
  An edit invalidates checkpoints from the edited line onward. When the line
  count changes, the checkpoint array is spliced, which costs n/32 entries.
- **Retokenizing after an edit.** Tokenizing starts from the nearest
  checkpoint at or above the edited line. It stops at the first checkpoint
  line where the new state equals the cached state (convergence). Without
  convergence it marks the rest dirty. Dirty checkpoints are recomputed
  lazily, only when needed to reach a requested line.
- **Visible-first.** Before each paint, the visible lines plus overscan are
  tokenized. If reaching the first visible line needs more than 2,000
  uncached lines in one frame, those lines are painted using the nearest known
  checkpoint state. The rest catches up over later frames, at most 2,000 lines
  or 2 ms per frame. An edit far below the viewport costs no tokenizing until
  that region is needed.
- **Token cache.** An LRU of 4,096 lines keyed by line text and start state.

### 9.3 Built-in JSON and vact provider

- **JSON (`emaki/json`).** A line-kind provider. Tags: `json-key`,
  `json-string`, `json-number`, `json-bool`, `json-null`, `json-punct`,
  `json-invalid`. The state is a compact string: the container stack plus an
  "expect key" flag, so `sameState` is string equality. JSON strings cannot
  span lines, so a wrong start state resynchronizes within one line.
- **vact (vactr side).** `vact-language.ts` wraps the existing `FallbackSpans`,
  `SyntaxSpans` and `WorkerSyntaxSpans` as one span-kind provider. The worker
  and fallback switch rules, `data-syntax` values and the 16,384-span cap
  stay unchanged. The tokens keep their `vact-tok-*` tags, mapped by vactr
  `theme.ts`.

## 10. Diff engine (`emaki/diff`)

- **`diffLines(a, b, { maxCost = 2000 })`**
  1. Trim the common prefix and suffix by line string equality.
  2. Intern the remaining lines to integer ids.
  3. Run Myers O(ND) on the middle.
  4. If D exceeds `maxCost`, stop and emit the remaining middle as one replace
     hunk. Pathological edits stay bounded.
  5. Output hunks `{ aFrom, aTo, bFrom, bTo }` (line indexes).
- **`diffWords(a, b)`.** Tokenizes into words, whitespace runs and single
  punctuation characters, then runs Myers on the tokens. Lines over 2,000
  characters return "whole line changed".
- **`alignSplit(hunks, aLines, bLines)`** returns per-side rows:
  - unchanged lines;
  - removed lines (left), added lines (right);
  - modified pairs: the first `min(del, ins)` lines of a hunk;
  - filler requirements `{ side, afterLine, count }`, which make both sides
    the same height.

### 10.1 Diff demo (`emaki/examples/diff`)

The demo uses only the public mark, line-format and widget APIs.

- **Editors.** Two editors side by side. The left is master (`readOnly: true`).
  The right is the working copy. Both show line numbers.
- **Recompute.** On a change in the right editor, the diff plugin schedules
  one recompute in the next `frame` hook. Several edits in one frame coalesce
  into one diff.
- **Line formats.** Removed lines get a red background and a `-` gutter
  marker on the left. Added lines get a green background and a `+` marker on
  the right. Colors come from tokens (`--emaki-diff-del`, `--emaki-diff-add`).
- **Word marks.** Intra-line word marks use stronger red or green
  backgrounds. They are computed lazily, only for modified pairs inside
  viewport +/- overscan, in the `viewportChanged` and `frame` hooks.
- **Fillers.** Filler rows are block widgets (`height = count x lineHeight`,
  canvas content with a flat diagonal hatch) placed after `afterLine`, or
  above line 1.
- **Synced scroll.** Each editor's `onScroll` sets the other's `scrollTo`,
  with a re-entrancy guard. Fillers make the total heights equal.

## 11. Demos, fixture and packaging

### 11.1 Demos

`npm run dev` (vite) serves `examples/index.html`, which links to:

- **`examples/plugin-showcase/`.**
  - A video block widget: a `<video>` element whose `srcObject` is a
    `MediaStream` from an animated offscreen canvas (`captureStream`), so no
    binary media is committed.
  - An animated canvas widget (waveform).
  - An EOL badge widget.
  - Range marks including glow.
  - Line formats (bar, background, gutter marker).
- **`examples/json-10k/`.** The 10,000-line fixture in an editable editor with
  JSON highlighting. Query `?lines=1000|10000|100000`.
- **`examples/diff/`.** Section 10.1.

All demo chrome follows `design-ui-style.md`:

- flat, `border-radius: 0`, no native control look (`appearance: none`);
- `--emaki-*` token custom properties;
- controls spaced at least 8 px apart.

### 11.2 Deterministic fixture

`generateJsonFixture({ lines, seed })` uses a seeded PRNG (mulberry32) and
emits pretty-printed JSON with 2-space indentation:

- nested objects and arrays;
- strings with escapes;
- numbers, booleans and null.

The output has exactly `lines` lines. Unit tests assert the following for
`lines = 10000, seed = 1`:

- exactly 10,000 lines;
- `JSON.parse` succeeds;
- byte length between 300 KB and 600 KB;
- a fixed SHA-256.

The fixture is generated in-page. No large fixture file is committed.

### 11.3 Packaging (EM-D3): TypeScript-source exports

**`emaki/package.json`**

| Field | Value |
|-------|-------|
| `name` | `emaki` |
| `version` | `0.1.0` |
| `license` | `MIT` |
| `type` | `module` |
| `exports` | `.` -> `./src/index.ts`, `./engine` -> `./src/engine.ts`, `./diff` -> `./src/diff/index.ts`, `./json` -> `./src/language/json.ts` |
| `files` | `src`, `README.md`, `LICENSE` |
| `peerDependencies` (exact) | `@codemirror/state` 6.7.6, `@codemirror/commands` 6.11.1 |
| `devDependencies` (exact) | the two peers, `typescript` 7.0.2, `vite` 8.3.1, `vitest` 5.0.2, `jsdom` 30.1.1, `playwright` 1.62.1 |

There are no other dependencies. `emaki/package-lock.json` is generated with
`npm install --prefer-offline`.

**Scripts**

| Script | Command |
|--------|---------|
| `check` | `tsc --noEmit` |
| `test` | `vitest run` |
| `build` | `vite build` (demos to `emaki/dist`; also proves the sources bundle) |
| `dev` | `vite` |
| `perf` | `node scripts/perf.mjs` |
| `e2e` | `node scripts/e2e.mjs` |

**Why source exports.**

- `file:` folder dependencies are symlinked and do not run `prepare`. A
  dist-only package would need a manual build in vactr's clean `npm ci`.
- Git dependencies would need devDependencies installed on the consumer side
  just to build.
- Source exports need no build step for either install mode.
- Peer dependencies keep a single `@codemirror/state` instance. CodeMirror
  requires this, and `Text` and `EditorState` cross the boundary.

**Consumer configuration in vactr (proved by a clean install):**

- `editor/package.json`: `"emaki": "file:../emaki"`. Lock updated with
  `npm install --prefer-offline`.
- `editor/vite.config.ts` and `editor/vitest.config.ts`:
  `resolve.dedupe: ['@codemirror/state', '@codemirror/commands']`, and
  `server.fs.allow` adds `'../emaki'`.
- `editor/vitest.config.ts`: `test.server.deps.inline: ['emaki']`. Node does not
  strip types under `node_modules`, which matters for a later `github:`
  install.
- `editor/tsconfig.json`: `preserveSymlinks: true`. The emaki sources then
  resolve `@codemirror/*` from `editor/node_modules`, whether or not
  `emaki/node_modules` exists.

`emaki/tsconfig.json` uses the same strictness flags as `editor/tsconfig.json`
(`strict`, `isolatedModules`, `types: []`, `lib` ES2022 + DOM). Any emaki
source therefore type-checks under both.

**Proof.** Run `cd editor && rm -rf node_modules && npm ci --prefer-offline &&
npm run check && vitest`, once with `emaki/node_modules` absent and once with
it present. Add a vactr test asserting a single CodeMirror instance: an emaki
`EditorCore` built from vactr's `Text` reports `state instanceof EditorState`
against vactr's import. The later `github:` switch is the operator's step. It
needs no change beyond the dependency string, because the same exports and
inline config apply.

**Repository files.** `emaki/` ships the following, which replace the target
repository's initial `README.md` and `.gitignore` when copied:

- `LICENSE` (MIT, same text as the target repository);
- `README.md`;
- `.gitignore` (`node_modules`, `dist`, `perf-results`, Playwright output).

## 12. Perf and e2e scripts (emaki)

### 12.1 Serving without a listening socket (EM-D11)

`scripts/perf.mjs` and `scripts/e2e.mjs` launch Chromium and WebKit with the
Playwright library (no `@playwright/test`). They serve `emaki/dist` through
`context.route('https://emaki.test/**')`, so no localhost port is bound. Both
scripts require a prior `npm run build` and fail with exit 2 if `dist` is
missing. Chromium uses `--mute-audio`. The demos produce no audio.

### 12.2 Metric definitions (EM-D12)

All raw samples go to `--out <dir>` as JSONL plus a `summary.json`. Every run
is on the 10k fixture in both browsers unless noted.

| Metric | Definition | Gate |
|--------|------------|------|
| First paint | `performance.now()` before `handle.setDoc(text)` (the text is pre-generated) to the end of the first frame callback whose visible lines are all tokenized and drawn. Measured on 5 page loads, all recorded | every load <= 200 ms (the maximum is gated) |
| Scroll frame work | Frame callback work time (phase timer, includes GL submit) during 300 wheel steps of 120 px across the document | p95 <= 16.7 ms |
| Keystroke-to-paint | Two numbers per key, for 200 printable keys typed at random positions across visible lines: (a) keydown `timeStamp` to the end of the presenting frame callback (recorded raw), and (b) input handler work plus presenting frame work | (b) p95 <= 16.7 ms, and >= 95% of keys are presented by the first frame after the event |
| Diff visible latency | Diff page, right editor: keydown `timeStamp` to the end of the frame in which both editors show diff decorations for that revision. 100 edits | p95 <= 100 ms |
| Size scaling | Metric (b) at 1k, 10k and 100k lines | p95(100k) <= max(2 x p95(1k), p95(1k) + 2 ms) in each browser |
| rAF interval during scroll | informational | none |

Metric (a) includes the wait for the next vsync, up to 16.7 ms. Gating it at
16.7 ms would fail even for zero-cost work, because WebKit headless control
pages already show an 18 ms frame interval (B11). The gate therefore checks
the two things that the user's budget means: the editor's own work fits in one
frame, and the edit lands on the next frame. Metric (a) is recorded raw for
review. This interpretation is listed for confirmation in
`design-docs/user-qa/pending-emaki-questions.md` (EQ1).

### 12.3 Diff e2e

`scripts/e2e.mjs` opens the diff page in Chromium and WebKit, then:

1. types into the right editor;
2. asserts through a test hook (`window.__emakiDiff`, demo-only) that:
   - the added-line count and filler count change as expected;
   - word marks appear on the modified line;
   - both scrollTops stay equal after a wheel scroll.

Exit 0 means pass. Exit 1 means a check failed. Exit 2 means a browser or
`dist` is unavailable.

**Where it runs.** Heavy runs (perf, e2e, vactr e2e) are serialized. Perf
runs use the same quiet-host lock procedure as `editor/test/e2e/README.md`,
applied by the operator around the command. emaki's scripts contain no vactr
path. Codex cannot launch browsers or bind sockets, so these runs execute
outside the Codex sandbox and the evidence is reported.

## 13. vactr integration

### 13.1 Composition

`mount.ts` imports only `emaki/engine`, and the vactr Vim from
`./vim-mount`. It builds the same pipeline as today:

- `EditorCore` behind the vactr `CodeSurface` adapter, with `DocumentSync` on
  `onTransaction`;
- `TextLayout` with `Heightmap`;
- the vactr `CanvasRenderer` adapter;
- `FrameScheduler` and `ViewHost`;
- `InputController` with `fallbackShortcut: fallbackStopShortcut` (the
  same tri-state function, unchanged in `ui/stop-keys.ts`), plus today's
  `stopAll` and `cut` options (`transport.stopAll()` and `transport.hush()`,
  as in `mount.ts:204-205`);
- `PointerController`;
- the `PluginHost` and `WidgetLayer`.

`vim-mount.ts` attaches the vactr Vim controller through
`InputController.setModalHook`, so `Mod-.` gentle stop-all and
`Mod-Shift-.` cut (hush) route exactly as before (EM-D18). The stop/cut keys
are covered unchanged by these tests, all of which stay in vactr with their
assertions:
- `test/canvas/input.test.ts`
- `input-keymap-hooks.test.ts`
- `input-vim-hook.test.ts`
- `vim-input.test.ts`
- `vim-keymap-input.test.ts`
- the behavior e2e (G8)

The syntax window computation (`mount.ts:254-257`) uses `Heightmap` instead of
`/ lineHeight`. The `?perf=1` hook, phase names, `setActive` reasons and the
`afterPresent` semantics are unchanged.

### 13.2 Annotation mapping (`decorations.ts`)

| vactr kind | emaki decoration |
|------------|------------------|
| `syntax` | token span with `className` as the tag |
| `selection` | built-in selection |
| `composition` | built-in composition underline |
| `unapplied` | static mark `{ background: unappliedRgba(color), minWidth: 6 }` |
| `diagnostic` | static mark `{ underline: { color: palette.diagnostic, thickness: 2 } }` |
| `call-head` | static mark `{ underline: { color: palette.callHead, thickness: 1 } }` |
| `binding` | static mark `{ label }` |
| `momentary` | animated mark `{ label }` |
| `playing` | animated mark `PLAYING_MARK = { background: [0.7, 0.58, 0.26, 0.5], glow: { color: [0.7, 0.58, 0.26, 0.5], radius: 4 } }` |
| `eval` | animated mark `EVAL_MARK` `[0.4, 0.64, 0.72, 0.5]`, or `EVAL_ERROR_MARK` `[0.8, 0.24, 0.32, 0.5]` when `className` includes `error` |

The colors equal B4. The label geometry equals today's (8 px per char + 8,
min 16, max 256). With the order rules in 6.3, every assertion in
`gpu.test.ts` holds unchanged.

### 13.3 Beat glow and eval flash (EM-D10)

The `beat-glow.ts` plugin registers one `animatedMarks` provider. It returns
`PLAYING_MARK` for each range of `highlight.tick(frameMs)` and the eval mark
for each active flash range. `mount.ts` no longer passes playing or eval rows
as annotation kinds. The renderer adapter maps those kinds only for callers
that still pass them (tests), and it uses the same constants. The visible
change is the glow halo the user asked for. Timing, the active-set
computation and `recordPresented` keys are unchanged.

### 13.4 Inline sequencer plugin (`inline/sequencer.ts`)

**Data.** A local `CycleSteps` (imported from `params/grid.ts`) is fed from
`client.on('playing')` with the same `beats_per_cycle` source as the params
grid. The plugin is display-only and has no write-back path.

**Anchor rule (EM-D13):**

1. For each slot, take the src span with the lowest start offset among the
   current cycle's events that have `src.file === DOC_FILE`.
2. Map it with `sync.mapWireSpan(src.span, src.doc_revision)`.
3. If a top-level form from `store.forms(DOC_FILE)` (mapped at the eval
   revision) contains it, the anchor is the **last line of that form**.
   Otherwise it is the span's own line.
4. Slots whose span does not map, or that have no `src`, get no inline widget.
   They remain in the params pane.
5. Slots that resolve to the same line share one block widget below it, with
   one row per slot, in sorted slot order.

**Widget.**

- Canvas content, height = rows x 14 px + 6 px padding.
- Square, flat step cells. Colors come from `--vt-*` tokens.
- The lit step is the one containing `TransportBar.cycles() mod 1`.
- `draw` returns `true` while the transport is running, so it animates only
  while visible.

**Lifecycle.**

- Hush and stop-all clear `CycleSteps` and remove the widgets.
- An edit re-resolves anchors on the next `playing` batch.
- Widget keys are `seq:<line>`.

### 13.5 Inline oscilloscope plugin (`inline/scope.ts`)

**Anchor.**

- Each line that matches the `#@ editor` prefix of `visual/settings.ts` and
  whose parsed `visualizers` contain `kind: 'oscilloscope'` gets one block
  widget below it.
- The plugin rescans in the `docChanged` hook, debounced to at most one scan
  per 250 ms, in `afterPresent`. Only lines whose first non-blank characters
  are `#@` are parsed.
- Bus analyzer units are not anchored (non-goal: no span on the wire).

**Widget.**

- Canvas content, 64 px tall.
- Draws `drawScope` from `visual/scopes.ts` with the master waveform. The
  frame is chosen by `pickFrame` on the levels timeline, as the background
  overlay does.
- `draw` returns `true` only while levels frames arrive (within 2 s of the
  last one).
- The background overlay (`visual/overlays.ts`) is unchanged and both coexist.

### 13.6 Toggle

- **Storage.** `inline-settings.ts` reads and writes `localStorage`
  `vactr.inlineWidgets` (`'true'` or `'false'`; default `true`; storage
  failures fall back to the default), in the same pattern as
  `vim-settings.ts`.
- **UI.** `EditorOptions` in `ui/transport-view.tsx` gets an `IconButton`
  `vact-inline-toggle` ("Inline widgets: on/off", `aria-pressed`), styled
  like the vim toggle.
- **Off state.** The sequencer and scope plugins clear their widgets. The
  heightmap returns to uniform heights.
- **Default.** The default is on because the user asked for inline widgets
  (EQ2). The existing e2e suites must pass with the default. If an existing
  e2e depends on fixed line geometry during playback, that is an engine defect
  to fix (coordinate APIs go through the heightmap), never a test edit.

### 13.7 Unchanged behavior that the gates must show

The following must behave as before:

- unapplied feedback, diagnostics and the tooltip;
- the completion popup position;
- Vim, its keymaps, hints and hop;
- transport, eval keys, momentary and numeric drag;
- samples, the background/visualizer overlay, the `?perf=1` hook;
- the iPad and Tauri paths.

There is no DOM renderer (B2).

## 14. Boundary enforcement

### 14.1 emaki boundary test

`emaki/test/boundary.test.ts` (`@vitest-environment node`, node APIs loaded
through a non-literal dynamic import as `editor/vite.config.ts` does) scans
every `.ts`, `.mjs` and `.html` file under `emaki/`, excluding
`node_modules` and `dist`. It fails when:

- a relative specifier resolves outside `emaki/`;
- a `src/` file imports a bare package other than `@codemirror/state` and
  `@codemirror/commands`;
- a test or script imports anything other than those, `vitest`, `jsdom`,
  `playwright` or `node:*`;
- any file references `solid-js`, `vactr` or `/editor/`.

This rule has no allowlist. It holds because no `vim/**` file moves
(EM-D17), the moved tests in 4.4 contain no `vactr` string, the pointer flag
becomes `markDragGesture`/`isDragGesture` (EM-D19), and the vact tokens and
`--vt-*` maps stay in vactr `theme.ts`. `scripts/perf.mjs` and
`scripts/e2e.mjs` contain no vactr path (12.3).

### 14.2 vactr boundary test

`editor/test/code/emaki-boundary.test.ts` fails when any of these holds:

- a file in `editor/src` or `editor/test` imports an `emaki` specifier other
  than the four public entries (`emaki`, `emaki/engine`, `emaki/diff`,
  `emaki/json`);
- a file imports a relative path into `../emaki` or `node_modules/emaki`;
- any file listed as moved in section 4.1 still exists in `editor/src/code`;
- `editor/src/code` contains `getContext('webgl2'`, `#version 300 es` or a
  `GlyphAtlas` definition.

## 15. Verification gates

Evidence goes under `tmp/<evidenceRoot>/<planId>/`. Records use the format
`{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome:
"passed", log, notes}`. Negative controls (for example, a planted escaping
import that the boundary test must reject) go only in `mutationEvidence`.

| Gate | Command |
|------|---------|
| G1 wasm | `export CARGO_TERM_QUIET=true; cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` |
| G2 emaki standalone | `cd emaki && npm ci --prefer-offline && npm run check && npm test && npm run build` |
| G3 emaki perf (serial, quiet host) | `cd emaki && npm run perf -- --browser all --sizes 1000,10000,100000 --out ../tmp/<evidenceRoot>/<planId>/perf` |
| G4 emaki e2e | `cd emaki && npm run e2e -- --browser all` |
| G5 vactr clean install | `cd editor && rm -rf node_modules && npm ci --prefer-offline && npm run check`, run with and without `emaki/node_modules` |
| G6 vactr vitest | `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run` (count reconciliation per 4.4) |
| G7 style | `cd editor && npm run test:style` |
| G8 vactr e2e | `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build && npm run e2e && npm run e2e:examples` |
| G9 canvas budgets | `mise run build-wasm-release`, then `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build && npm run e2e -- --browser all --profile measure --write-evidence --run-id emaki-001` under the README lock procedure. The run id keeps `run-001` intact. Passes when the thresholds in B11 hold and the text-dirty p95 is recorded against the 4 ms target |
| G10 Tauri | `CARGO_TERM_QUIET=true cargo test --manifest-path editor/src-tauri/Cargo.toml` (the command used by `impl-plans/completed/live-perf-int-closeout.md:100`). The test count must equal the pre-run baseline recorded in the same plan's evidence |

Rebuild (G1 and the editor build) before any e2e run. Never run G3, G4, G8 or
G9 concurrently with each other or with other gating runs.

## 16. Rollout and wave order

1. **W1, emaki core (serial).** Scaffold `emaki/` (package, configs, boundary
   test, packaging proof). Copy the engine files in with the section 4.1
   decoupling and the moved tests. vactr is untouched, so `editor` stays
   green.
2. **W2, new engine APIs.** Decorations, then heightmap with widgets and the
   plugin host, then the language provider with incremental highlighting and
   JSON. These share `render/*` and `layout/*`, so plans that touch the same
   files run serially. Each lands with unit tests: mapping through edits,
   heightmap y-to-line with widgets, mount/unmount virtualization, format
   precedence, tokenizer convergence.
3. **W3, in parallel once W2's API is fixed:**
   - (a) the vactr switch: dependency, configs, adapters, deletion of moved
     files and tests, and test import updates in one plan, so `editor` is
     green at its end;
   - (b) the diff module and diff demo with its e2e;
   - (c) the JSON demo, fixture and perf script;
   - (d) the plugin showcase.
4. **W4, vactr plugins:** beat glow and eval flash marks, sequencer, scope,
   toggle. Depends on W3(a).
5. **W5, gates:** G1 to G10, evidence, README and `mise.toml` tasks
   (`emaki-check`, `emaki-test`, `emaki-build`, `emaki-perf`, `emaki-e2e`).

During W1 and W2 the engine exists in both places. The duplicate is
transitional and removed by W3(a). The final state has no duplicate engine
(enforced by 14.2).

## 17. Decisions

| ID | Decision |
|----|----------|
| EM-D1 | The DOM renderer was already removed (B2). Nothing moves, and `?renderer=dom` is not a preserved path |
| EM-D2 | Two public levels: `createEditor`, and `emaki/engine` blocks. vactr composes the blocks |
| EM-D3 | TypeScript-source exports with `@codemirror/*` as exact-pinned peers. Consumer config uses dedupe, inline and `preserveSymlinks` |
| EM-D4 | `sync.ts`, `line-bytes.ts`, `RevisionHistory`, `momentary-pointer.ts`, `perf-hook.ts`, `format*.ts`, the `syntax*` providers and the vact language stay in vactr. `surface.ts` and `renderer.ts` stay as thin adapters |
| EM-D5 | Only font weight and style reshape. Color and opacity re-tint segments. Background, glow, underline, strike and labels are quads only. Animated marks are limited by type to the quad-only fields |
| EM-D6 | Glow is three expanded quads in the background layer after all mark backgrounds. No shader change |
| EM-D7 | Precedence and draw order as in 6.3, which reproduces today's order |
| EM-D8 | Sparse heightmap with prefix sums: O(log w) lookup, O(w log w) rebuild, O(1) with no widgets |
| EM-D9 | Language providers are of line kind (incremental checkpoints every 32 lines) or span kind (vact) |
| EM-D10 | Beat glow and eval flash are an animated-mark plugin. `PLAYING_MARK` adds a glow over today's color |
| EM-D11 | Playwright request routing instead of a localhost server for emaki scripts |
| EM-D12 | Keystroke and scroll gates measure editor work per frame plus next-frame presentation. Raw keydown-to-present is recorded |
| EM-D13 | Sequencer anchor: lowest `src` span per slot, placed below the last line of its containing top-level form |
| EM-D14 | Canvas budgets keep the repository gates (B11). 4 ms text-dirty work is a recorded target |
| EM-D15 | Moved tests keep their counts. vactr + emaki >= 2384 + new tests |
| EM-D16 | Inline widgets default to on, key `vactr.inlineWidgets` |
| EM-D17 | `vim/**` (core, action IDs, Ex commands, presets) stays unchanged in vactr, and all `test/code/vim/*` stay. emaki keeps generic seams only (`ModalKeyHook`, `CursorShape`, hop overlay labels). Persisted `vactr.*` keymap IDs are untouched |
| EM-D18 | `keyboard.ts` takes an injected `fallbackShortcut?(event): 'stop-all' \| 'cut' \| null` and keeps the `stopAll`/`cut` branch, so stop-all vs cut routing is preserved |
| EM-D19 | The `vactrNumericGesture` expando becomes the emaki helpers `markDragGesture`/`isDragGesture` (a WeakSet). vactr `momentary-pointer.ts` and `params/mount.ts` use them |

## 18. Risks

- **Heightmap regressions.** Hit testing, vertical motion and caret reveal
  can regress on iPad/WKWebView. Unit tests cover band hit-testing and
  line-index motion, and G8 runs Chromium and WebKit. Physical iPad checks
  remain an operator activity, as in 15.3.
- **The 100k scaling gate.** It may expose O(n) work outside tokenization,
  such as the syntax key join or the checkpoint splice. The 1k/10k/100k
  comparison is the gate. A failure is fixed in the engine, never by changing
  the gate.
- **Two `@codemirror/state` copies.** If dedupe or `preserveSymlinks`
  misbehaves under TypeScript 7.0.2 or vite 8.3.1, there will be two copies.
  G5 checks both install states, and the single-instance test catches it.
- **Default-on inline widgets.** These change line geometry during playback.
  G8 exposes any coordinate path that still assumes fixed height.
- **Large Myers cost.** Replacing many lines can push the cost high. The cap
  turns it into one replace hunk, which may show a coarser diff than GitHub
  for huge rewrites. This is acceptable for the 100 ms visible-latency gate.
