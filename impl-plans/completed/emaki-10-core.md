# EMK-CORE: emaki package scaffold, shared contract types and engine extraction

**Status**: Completed (accepted in session 599 wave 1: test-integrity, adversarial review and branch evidence passed; evidence tmp/emaki/EMK-CORE/continuation-2-final-*.log and tmp/opus-terra-waves/opus-terra-design-and-implement-review-loop-session-599/wave-001/wave-summary.json)
**Plan ID**: EMK-CORE (wave 1, runs alone)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 2 (B3-B10, B15-B18), 3, 4.1, 4.4, 5.2, 6.3 (base order), 8.2, 11.3, 14.1; decisions EM-D2, EM-D3, EM-D7, EM-D8, EM-D17, EM-D18, EM-D19
**Rules**: `impl-plans/active/emaki-00-rules.md` (mandatory)
**Manifest**: `impl-plans/active/emaki-dispatch.json`
**Created**: 2026-10-31

## Intent and Context

The user wants vactr's canvas editor turned into a standalone library named
`emaki`, staged at `emaki/` and later copied to github.com/tacogips/emaki.
This plan creates the package and moves the engine into it. Later waves add
features on top.

1. **Package scaffold.** Config files, a lockfile and a boundary test. The
   package must work standalone with `cd emaki && npm ci && npm run check &&
   npm test && npm run build`.
2. **Shared contract types.** All type contracts that later parallel plans
   depend on, pinned in `emaki/src/types.ts` and `emaki/src/render/types.ts`.
3. **Engine copy.** Copy the engine files from `editor/src/code` into
   `emaki/src` (vactr is NOT modified in this wave). Decouple them from vactr
   as listed below.
4. **Uniform heightmap.** Route every fixed-line-height computation through a
   `Heightmap` (sparse prefix sums, full implementation). With no blocks it
   reproduces today's geometry exactly.
5. **Generic decoration input.** Replace the renderer's fixed annotation kinds
   with a decoration input. It covers today's feature set (tokens, selection,
   composition, static mark background/underline/label/minWidth, animated
   mark background/label, cursor, handles, hop labels) in today's draw order.
6. **Moved tests.** Move five engine-only test files, which change only their
   import specifiers.

vactr's own files stay untouched until EMK-VAC-SWITCH (wave 3). Until then
the engine exists in both places. This duplicate is transitional.

## Non-goals

- No marks color/opacity/font/strike/glow, no line-format rendering, no
  mark/line-format stores (EMK-DECOR).
- No widgets, widget layer or vertical-motion change (EMK-WIDGETS).
- No language provider runtime or JSON (EMK-LANG).
- No diff (EMK-DIFF).
- No `createEditor`, `src/index.ts`, plugin runtime or README (EMK-API).
- No demo content beyond placeholder pages.
- No edit to anything under `editor/`, `design-docs/` or `mise.toml`.
- Do not move `vim/**`, `sync.ts`, `line-bytes.ts`, `RevisionHistory`,
  `momentary-pointer.ts`, `perf-hook.ts`, `format*.ts`, `syntax*.ts`,
  `language.ts`, `unapplied*.ts` (EM-D4, EM-D17).
- No DOM renderer exists (EM-D1). Do not recover one.

## Dependencies

- **dependsOn**: none.
- **Blocks**: every other emaki plan.

## writePaths

Package files:
- `emaki/package.json`, `emaki/package-lock.json`, `emaki/tsconfig.json`,
  `emaki/vitest.config.ts`, `emaki/vite.config.ts`, `emaki/.gitignore`,
  `emaki/LICENSE`

Contract types and the engine barrel:
- `emaki/src/types.ts`, `emaki/src/engine.ts`

Scripts:
- `emaki/scripts/route-server.mjs`

`emaki/src/core/`:
- `index.ts`, `editor-core.ts`, `undo-budget.ts`

`emaki/src/layout/`:
- `index.ts`, `layout.ts`, `advances.ts`, `heightmap.ts`

`emaki/src/render/`:
- `index.ts`, `types.ts`, `atlas.ts`, `frame.ts`, `geometry.ts`,
  `segments.ts`, `palette.ts`, `cursor-shape.ts`, `resources.ts`
- `renderer.ts`, `text-pass.ts`, `decoration-pass.ts`

`emaki/src/input/`:
- `index.ts`, `input.ts`, `keyboard.ts`, `pointer.ts`, `accessibility.ts`,
  `drag-gesture.ts`

`emaki/src/view/`:
- `index.ts`, `view-host.ts`

Empty barrels for later plans:
- `emaki/src/decor/index.ts`, `emaki/src/plugin/index.ts`,
  `emaki/src/language/index.ts`

Demo shells:
- `emaki/examples/index.html`, `emaki/examples/shared/demo.css`
- `emaki/examples/plugin-showcase/index.html`,
  `emaki/examples/json-10k/index.html`, `emaki/examples/diff/index.html`

Tests:
- `emaki/test/boundary.test.ts`, `emaki/test/support/recording-gl.ts`
- `emaki/test/render/atlas-cells.test.ts`,
  `emaki/test/render/glyph-placement.test.ts`,
  `emaki/test/layout/text-cells.test.ts`, `emaki/test/render/frame.test.ts`,
  `emaki/test/render/cursor-shape.test.ts`
- `emaki/test/layout/heightmap.test.ts`,
  `emaki/test/render/decoration-order.test.ts`,
  `emaki/test/render/heightmap-geometry.test.ts`,
  `emaki/test/core/editor-core.test.ts`

Generated, plus this plan:
- `emaki/node_modules`, `emaki/dist` (artifactRoots)
- `impl-plans/active/emaki-10-core.md` (Progress Log only)

## sharedPaths

None. These are read-only sources to copy from:
- `editor/src/code/*.ts`
- `editor/test/code/atlas-cells.test.ts`, `glyph-placement.test.ts`,
  `text-cells.test.ts`
- `editor/test/canvas/frame.test.ts`, `cursor-shape.test.ts`
- `editor/test/code/recording-gl.ts`
- `editor/tsconfig.json`, `editor/package-lock.json`
- `/Users/taco/gits/tacogips/vactr-worktrees/wf-emaki/LICENSE`

## Tasks

### T1: Package scaffold

**`emaki/package.json`**

| Field | Value |
|-------|-------|
| `name` | `emaki` |
| `version` | `0.1.0` |
| `license` | `MIT` |
| `type` | `module` |
| `private` | absent |
| `description` | one sentence, no vactr mention |
| `engines.node` | copied from `editor/package.json` |
| `files` | `["src", "README.md", "LICENSE"]` |
| `peerDependencies` | `@codemirror/state` `6.7.6`, `@codemirror/commands` `6.11.1` (exact strings, no ranges) |
| `devDependencies` | the same two, plus `typescript` `7.0.2`, `vite` `8.3.1`, `vitest` `5.0.2`, `jsdom` `30.1.1`, `playwright` `1.62.1` |

- `exports`, exactly:
  - `"."` -> `"./src/index.ts"`
  - `"./engine"` -> `"./src/engine.ts"`
  - `"./diff"` -> `"./src/diff/index.ts"`
  - `"./json"` -> `"./src/language/json.ts"`

  Targets that later plans create may be missing now. That is expected.
- `scripts`:

  | Script | Command |
  |--------|---------|
  | `check` | `tsc --noEmit` |
  | `test` | `vitest run` |
  | `build` | `vite build` |
  | `dev` | `vite` |
  | `perf` | `node scripts/perf.mjs` |
  | `e2e` | `node scripts/e2e.mjs` |

**Lockfile.** Operator amendment (after session 596): the Codex sandbox has
no network, and the npm cache lacked registry metadata for a lockfile-less
install. The operator therefore generated `emaki/package-lock.json` and
`emaki/node_modules` outside the sandbox from the `package.json` above
(exact pins verified: @codemirror/state 6.7.6, @codemirror/commands 6.11.1,
typescript 7.0.2, vite 8.3.1, vitest 5.0.2, jsdom 30.1.1, playwright
1.62.1). The lockfile is committed and counts as T1's lockfile deliverable.
Do not regenerate it, do not run `npm install` again, and do not change
`dependencies`/`devDependencies`/`peerDependencies`; use the existing
`emaki/node_modules`. `npm ci --prefer-offline` (verification step 1) is run
by the orchestrator outside the sandbox if the sandbox cannot run it; record
it as blocked-in-sandbox per rule 11 rather than as a blocker. Never write an
npm cache into `emaki/dist` or any other tracked path.

**`tsconfig.json`.** Copy `editor/tsconfig.json`'s compilerOptions exactly,
with these differences:
- remove `jsx` and `jsxImportSource`;
- `include` is `["src", "test", "examples", "vite.config.ts", "vitest.config.ts"]`.

Keep `strict`, `isolatedModules`, `types: []`, `moduleResolution: bundler`
and `lib` ES2022 + DOM + DOM.Iterable.

**`vitest.config.ts`.** Environment `jsdom`; include
`test/**/*.test.ts`. No plugins.

**`vite.config.ts`.** Multi-page build:
- `root: 'examples'`;
- `build.outDir: '../dist'`, `emptyOutDir: true`;
- `build.rollupOptions.input`: the four HTML files `examples/index.html`,
  `examples/plugin-showcase/index.html`, `examples/json-10k/index.html`,
  `examples/diff/index.html`, resolved relative to the config file;
- `base: './'`, `build.target: 'es2022'`.

Imitate `editor/vite.config.ts` for typing without `@types/node`: the
config can use `new URL('./examples/...', import.meta.url).pathname`.

**Route helper.** `emaki/scripts/route-server.mjs` (EM-D11) is shared
read-only by EMK-DIFF-DEMO, EMK-SHOWCASE and EMK-PERF. It is plain Node ESM
and imports only `node:fs`, `node:path` and `node:url`. Export:
- `ORIGIN = 'https://emaki.test'`;
- `async function routeDist(context, distDir)`, which calls
  `context.route(ORIGIN + '/**', handler)`. The handler maps the URL path to
  a file under `distDir`; `/` and directory paths map to `index.html`. It
  fulfils with the content type by extension (`.html`, `.js`, `.css`,
  `.json`, `.svg`, `.wasm`, `.map`). It returns 404 for missing files and
  for paths escaping `distDir` (reject `..` after decoding);
- `function distReady(distDir): boolean`.

No listening socket.

**Other package files.**
- `.gitignore`: `node_modules`, `dist`, `perf-results`.
- `LICENSE`: copy the repository-root `LICENSE` text.

**Demo shells.**
- `examples/index.html` links to the three pages.
- `examples/shared/demo.css` follows the UI rules:
  - `--emaki-*` token custom properties (bg, panel, text, muted, accent,
    `diff-add`, `diff-del`, `diff-add-strong`, `diff-del-strong`, space-2,
    space-3);
  - `* { border-radius: 0 }`, `button, input, select { appearance: none }`;
  - controls spaced at least 8 px apart.
- The three page HTMLs are placeholders: a heading and a "not implemented
  yet" paragraph, linking `../shared/demo.css`. No script yet.

### T2: Shared contract types (`emaki/src/types.ts`, `emaki/src/render/types.ts`)

These shapes are a contract for EMK-DECOR, EMK-WIDGETS, EMK-LANG, EMK-API and
EMK-VAC-SWITCH. Pin them exactly. Fields are optional unless stated.

**`src/types.ts`**

Geometry and basics:
- `type Rgba = [number, number, number, number]` (0..1 channels; same as
  today's `palette.ts` `Rgba`)
- `interface Rect { left; right; top; bottom }` (the `CodeRect` shape)
- `interface Range { from; to }`
- `type CursorShape = 'bar' | 'block' | 'underline'`

Modal keys:
- `interface ModalKeyInput { key: string; code?: string; ctrlKey: boolean; metaKey: boolean; altKey: boolean; shiftKey: boolean }`
  (structurally equal to vactr `VimKeyInput`)
- `interface ModalKeyHook { handleKey(input: ModalKeyInput): boolean; allowsTextInput(): boolean; overwriteRange(text: string): Range | null; noteCompositionDiscarded(): void; flushPendingInput?(): void }`

Marks:
- `interface MarkStyle`:
  - `color?: Rgba; background?: Rgba`
  - `fontWeight?: 'normal' | 'bold'; fontStyle?: 'normal' | 'italic'`
  - `underline?: { color: Rgba; thickness: 1 | 2 | 3 }`
  - `strike?: { color: Rgba }`
  - `opacity?: number`
  - `glow?: { color: Rgba; radius: number }`
  - `label?: { text: string; fill?: Rgba; color?: Rgba }`
  - `minWidth?: number`
- `type AnimatedMarkStyle = Pick<MarkStyle, 'background' | 'glow' | 'underline' | 'strike' | 'label' | 'minWidth'>`
- `interface Mark extends Range { style: MarkStyle }`
- `interface AnimatedMark extends Range { style: AnimatedMarkStyle }`

Line formats:
- `interface LineFormatStyle`:
  - `background?: Rgba; color?: Rgba`
  - `fontWeight?: 'normal' | 'bold'; fontStyle?: 'normal' | 'italic'`
  - `opacity?: number`
  - `bar?: { color: Rgba; width: number }`
  - `gutterMarker?: { text: string; color: Rgba }`
  - `gutterBackground?: Rgba`
- `interface LineFormat { pos: number; style: LineFormatStyle }`
- `interface ResolvedLineFormat { line: number; style: LineFormatStyle }`
  (`line` is 0-based)

Tokens:
- `interface TokenSpan extends Range { tag: string }`
- `interface LineToken { from: number; to: number; tag: string }`
  (line-relative offsets)

Widgets:
- `type WidgetContent = { kind: 'dom'; mount(el: HTMLElement): () => void } | { kind: 'canvas'; draw(ctx: CanvasRenderingContext2D, info: { width: number; height: number; dpr: number; frameMs: number }): boolean }`
- `interface BlockWidgetSpec { key: string; pos: number; side: 'above' | 'below'; height: number; content: WidgetContent; interactive?: boolean; label?: string }`
- `interface EolWidgetSpec { key: string; pos: number; width: number; content: WidgetContent; interactive?: boolean; label?: string }`

Language:
- `interface LineLanguage<S> { kind: 'lines'; name: string; startState(): S; tokenizeLine(text: string, state: S): { tokens: readonly LineToken[]; state: S }; sameState(a: S, b: S): boolean }`
- `interface SpanLanguage { kind: 'spans'; name: string; spans(state: EditorState, from: number, to: number, limit: number): { spans: readonly TokenSpan[]; truncated: boolean }; noteChanges?(changes: ChangeSet, state: EditorState): void; setInvalidate?(cb: () => void): void; dispose?(): void }`
- `type LanguageProvider = LineLanguage<unknown> | SpanLanguage`

Plugins:
- `interface PluginViewport { fromLine: number; toLine: number; top: number; height: number }`
- `interface PluginUpdate { state: EditorState; changes: ChangeSet; docChanged: boolean; selectionSet: boolean }`
- `interface PluginInstance { docChanged?(u: PluginUpdate): void; viewportChanged?(v: PluginViewport): void; frame?(frameMs: number): boolean; destroy?(): void }`
- `interface PluginHost`:
  - `readonly state: EditorState`
  - `marks(owner: string): { set(marks: readonly Mark[]): void; clear(): void }`
  - `animatedMarks(provider: (frameMs: number) => readonly AnimatedMark[]): () => void`
  - `lineFormats(owner: string): { set(formats: readonly LineFormat[]): void; clear(): void }`
  - `widgets(owner: string): { set(blocks: readonly BlockWidgetSpec[], eol?: readonly EolWidgetSpec[]): void; clear(): void }`
  - `requestFrame(): void; setActive(active: boolean): void`
  - `coordsAtPos(pos: number): Rect | null; lineTop(line: number): number`
  - `readonly viewport: PluginViewport`
  - `readonly theme: { palette: Palette; tokens: Readonly<Record<string, Rgba>> }`
  - `reportError(error: unknown): void`
- `interface EmakiPlugin { name: string; attach(host: PluginHost): PluginInstance | void }`

Import `Palette` from `./render/palette` as a type only. Import
`EditorState` and `ChangeSet` as types from `@codemirror/state`.

**`src/render/types.ts`**

- `RenderInput`:
  - `textRevision?: number`
  - `decorationsKey?: string`: a caller-supplied cache key for all static
    decorations, replacing today's `annotationsRevision` and
    `unappliedColor` key parts
  - `tokens?: readonly TokenSpan[]`
  - `selection?: readonly Range[]; composition?: readonly Range[]`
  - `marks?: readonly Mark[]; animatedMarks?: readonly AnimatedMark[]`
  - `lineFormats?: readonly ResolvedLineFormat[]`
  - `cursor?: number | null; cursorShape?: CursorShape; cursorEnd?: number | null; cursorVisible?: boolean`
  - `handles?: readonly { pos: number; end?: boolean }[]`
  - `hopLabels?: readonly { pos: number; text: string }[]; hopRevision?: number`
- `CodeRenderer`: the same members as today's
  `editor/src/code/renderer-types.ts`, but `render(input?: RenderInput): boolean`.
- `GpuStatus`: moved unchanged.

### T3: Engine copy with decoupling

Copy each file and change only what is listed. Keep logic, constants,
comments and line structure otherwise identical. Imitate the existing code
exactly.

**Render files**
- `atlas.ts`, `frame.ts`, `segments.ts`: copy unchanged. Only import paths
  change.
- `geometry.ts`: `CodeRect` becomes `Rect` from `../types`.
- `cursor-shape.ts`: `CodeRect` becomes `Rect`, `CursorShape` comes from
  `../types`, and `Rgba` from `./palette`.
- `resources.ts`: declare
  `export interface ResourceBudget { readonly limitBytes: number; readonly usedBytes: number; reserve(bytes: number): boolean; release(bytes: number): void }`
  in `render/resources.ts`. These are the same members as
  `editor/src/app/apis.ts:33-37`. Remove the `../app/apis` import.
  `RESOURCE_LIMITS`, `ResourceLedger`, `effectiveSize` and `Reservation`
  are unchanged.
- `palette.ts`: keep `Rgba`, `parseColor`, `rgbaCss`.
  - `Palette` becomes
    `{ token: Record<string, Rgba>; text; gutter; cursor; composition; labelFill; labelText; handle; selection; background }`.
    That is, `diagnostic` and `callHead` are removed, and `selection`
    (`[0.218, 0.4, 0.582, 0.55]`) and `background` (`[0.047, 0.059, 0.082, 1]`)
    are added.
  - `FALLBACK_PALETTE` keeps today's legacy values for the shared fields and
    has an **empty** token map.
  - `readPalette(root, properties: { token?: Record<string, string>; fields?: Partial<Record<keyof Omit<Palette, 'token'>, string>> }): Palette`
    reads only the given CSS custom properties. No `--vt-*` or `vact-tok-*`
    literal remains in emaki.

**`renderer.ts`, split into three files.** Each stays under 1000 lines.

- `renderer.ts`:
  - the `CanvasRenderer` class: GL lifecycle, context loss/restore, resize,
    status, the font-change listener;
  - cache keys and `render(input)` orchestration;
  - `setText`/`setDocument`/`setPalette`/`setBackgroundOpacity`/`setViewport`/`setPhases`/`dispose`.
- `text-pass.ts`: today's segment building, glyph runs, gutter line numbers
  and token tinting (`rebuildTextSegments`, `drawRun`, run styles). Tint uses
  `palette.token[tag] ?? palette.text`.
- `decoration-pass.ts`: all background and overlay quads and labels.

The text cache key replaces `syntaxKey` with a key built from `tokens`
(`from:to:tag`). The static background key uses
`viewportKey|textRevision|decorationsKey|selectionKey`.

**Draw order.** This must reproduce today's order exactly
(`editor/src/code/renderer.ts:250-323`):

Background layer:
1. The editor background rect: `palette.background` with alpha replaced by
   `backgroundOpacity`.
2. Static mark backgrounds, in input order. An empty range with `minWidth`
   becomes `rect.left + minWidth`.
3. Selection rects in `palette.selection`.
4. Each frame (not cached): animated mark backgrounds in input order, using
   today's `animatedRects` geometry.

Overlay layer (cached as today):
1. Static mark underlines (rect top = bottom - thickness, mark color) and
   composition underlines (2 px, `palette.composition`), interleaved in input
   order. Use one merged pass ordered by `from`, then source list (marks
   before composition).
2. The cursor.
3. Static mark labels: today's binding-badge geometry
   (`x = coords(to).left - viewLeft + 3`, width
   `min(256, max(16, len * 8 + 8))`, fill `label.fill ?? palette.labelFill`,
   text `label.color ?? palette.labelText`, text at `x + 4`).
4. Hop labels (unchanged geometry).
5. Handles.

Per frame: animated mark labels, with the same geometry as static labels.

Ignore `lineFormats`, and mark `color`/`opacity`/`font*`/`strike`/`glow`,
in this plan. EMK-DECOR implements them. Do not throw on them.

**Renderer heightmap sites (EMK-R5-01).** `renderer.ts` has no feature
owner after this wave, so EMK-CORE must route every renderer row position
through `layout.heightmap` now. Each item names today's line in
`editor/src/code/renderer.ts` and its replacement.

- **Text slot y** (`renderer.ts:374`): `line.number * lineHeight - scrollTop`
  becomes `layout.heightmap.lineTop(line.number) - scrollTop`. It lives in
  `text-pass.ts`. Gutter line-number runs share that slot y, so they follow
  the text row automatically.
- **Visible rows** (`renderer.ts:409-411`, `visibleFirst`/`visibleLast` in
  `rebuildTextSegments`) become
  `heightmap.lineAtHeight(scrollTop).line` and
  `heightmap.lineAtHeight(scrollTop + height).line + 1`. The `+ 1` keeps
  today's `ceil` semantics. Check this with the zero-block parity test
  below.
- **Animated mark rect top** (`renderer.ts:608`, `animatedRects`):
  `line.number * lineHeight` becomes `heightmap.lineTop(line.number)`, and
  `bottom = top + lineHeight`. It lives in `decoration-pass.ts`.
- **Selection, composition, underline and label rects** come from
  `layout.rangeRects` and `layout.coordsAtPos`, which already use the
  heightmap (see "Layout and view" below). Block bands therefore get no
  selection or animated paint, because rects exist only per text row.
- **Overscan** (`renderer.ts:253-256`) stays a pixel amount
  (`ceil(height / lineHeight) * lineHeight`) passed to `layout.visible`.
  Do not convert it to lines.

**Cache keys.** Add `heightmap.revision` to `viewportKey`, which feeds the
text cache `key`, `backgroundKey` and `overlayKey` (`renderer.ts:232-237`).
A `setBlocks` that changes the effective entries, with no text, viewport or
decoration change, must then rebuild the text, background and overlay
layers on the next `render()`. When the revision is unchanged, idle frames
still reuse every layer. `editor/test/canvas/gpu.test.ts:410-421` (the idle
reuse test) must keep passing.

**Pitfall.** With zero blocks, `lineTop(n)` must equal `n * lineHeight`
exactly (the same multiplication, no accumulated float sum). Every emitted
rect and slot position is then identical to today's. Do not compute
`lineTop` by summing per-line heights.

**Layout and view**
- `layout.ts`: `CodeRect`/`CodeRange` become `Rect`/`Range`.
  - Add `readonly heightmap: Heightmap`. `setText`/`setDocument` set the line
    count, and `setFont` sets the line height.
  - Replace every `* this.font.lineHeight` and `/ this.font.lineHeight`
    (lines 361-362, 480-481, 485, 499-500) with `heightmap.lineTop`,
    `lineAtHeight` and `totalHeight`.
  - `posAtCoords` in a block band returns a position on the band's anchor
    line. The x rule is the same as a text row.
- `advances.ts`: copy unchanged.
- `view-host.ts`:
  - rename the class `CodeViewHost` to `ViewHost` (design 5.2). vactr
    imports it with an `as CodeViewHost` alias;
  - `scrollCaret` uses `heightmap.lineTop(line)` and that plus `lineHeight`
    (today: line 80);
  - max scroll uses `heightmap.totalHeight()` (today: line 89);
  - the `CodeSurface` type becomes `EditorCore`.

**Input**
- `input.ts`:
  - `VimInputHook` becomes `ModalKeyHook`. **Keep the method name `setVim`**,
    because vactr tests call `input.setVim(...)` and may only change import
    specifiers (rule 15).
  - Add `setModalHook` as the same method under a neutral name. Both set the
    same field.
  - `InputPresentation.annotations` keeps its field name and today's
    element shape. Type it as
    `readonly { from: number; to: number; kind: 'composition' }[]`
    (export the type as `CompositionAnnotation`).
    `editor/test/canvas/input.test.ts:98` asserts
    `toEqual([{ from: 1, to: 3, kind: 'composition' }])`, and
    `input-vim-hook.test.ts:116-142` asserts `[]`. Do not rename the field
    or reshape it.
- `keyboard.ts` (EM-D18): remove the `../ui/stop-keys` import. Add
  `KeyboardOptions.fallbackShortcut?: (event: KeyboardEvent) => 'stop-all' | 'cut' | null`,
  and keep `stopAll` and `cut`. Line 89 becomes
  `const stopAction = this.options.fallbackShortcut?.(event) ?? null;`. The
  branch at lines 91-93 is unchanged. `InputOptions` passes the option
  through, as it does `stopAll`/`cut` today.
- `pointer.ts` (EM-D19): replace the expando at line 75 with
  `markDragGesture(event)` from the new `input/drag-gesture.ts`. That module
  exports `markDragGesture(e: Event): void` and
  `isDragGesture(e: Event): boolean`, backed by a module-level `WeakSet<Event>`.
  `NumericGesture` and `SelectionHandle` stay exported from `pointer.ts`.
- `accessibility.ts`: the `CodeSurface` type becomes `EditorCore`.

**Core**
- `core/undo-budget.ts`: copy from `editor/src/code/history.ts`
  `retainedBytes`, `trimUndoHistory`, `HISTORY_UNDO_BYTES`, `INDEX_BYTES`,
  `HISTORY_LIMIT` and `touches`, unchanged. Do not copy `RevisionHistory`,
  `LineTable` or `Span`.
- `core/editor-core.ts`: the generic part of `editor/src/code/surface.ts`.

  Constructor
  `new EditorCore({ doc: Text | string; extensions?: Extension; historyByteLimit?: number; externalHistory?: ExternalHistoryBudget })`, where
  `ExternalHistoryBudget = { readonly retainedBytes: number; readonly reducedDepth: boolean; trimToBytes(bytes: number): void }`.
  This replaces the direct `sync.history` use in `boundHistory` and
  `retentionStatus`.

  `dispatch` order is pinned:
  1. assign the state;
  2. run `onTransaction(tr)` hooks synchronously in registration order (new
     API: `onTransaction(cb: (tr: Transaction) => void): () => void`);
  3. remap the composition range;
  4. `boundHistory`;
  5. publish to subscribers.

  Keep the reentrancy and "Stale or mixed transaction" guards. Keep these
  members, with today's semantics:
  - `subscribe`, `attachBridge`, `focus`, `posAtCoords`, `coordsAtPos`
  - `onPointer`/`notifyPointer`, `onBlur`/`notifyBlur`,
    `onCompositionStart`/`notifyCompositionStart`
  - `addKeymap`/`runKeymaps`/`matchesKeymap`, and the private `matchesKey`
  - `registerNumericDrag`/`numericDrag`
  - `compositionRange`/`setCompositionRange`/`deferSourceWrite`
  - `undo`/`redo`/`retentionStatus`/`dispose`

  Do NOT copy `annotate`, `annotationRanges`, the animated sources,
  `registerMomentaryDrag`/`momentaryHit`/`momentaryDrag`, or `sync`. Those
  stay in the vactr adapter.

  **Subclassing contract.** EMK-VAC-SWITCH implements vactr's
  `CodeSurface` as `class CodeSurface extends EditorCore`. Today's
  `input.ts`, `keyboard.ts`, `pointer.ts` and `view-host.ts` type the
  surface as the concrete class, and the vactr tests pass the vactr surface
  into the emaki `InputController`. Therefore:
  - `EditorCore` must not be `final`-like. Use no `#private` fields; use the
    TypeScript `private`/`protected` modifiers.
  - `publish(update: EditorUpdate)` is `protected`. The subclass uses it
    for `annotate()`.
  - `EditorUpdate` is
    `{ state: EditorState; changes: ChangeSet; docChanged: boolean; selectionSet: boolean; userEvent: string | null }`,
    the same shape as vactr's `CodeSurfaceUpdate`.
  - All moved input and view modules type their surface parameter as
    `EditorCore`.
  - `undo()` and `redo()` must dispatch through `this.dispatch`, so that
    subclass hooks run.

**Barrels**
- `engine.ts`:
  `export * from './core/index'; export * from './layout/index'; export * from './render/index'; export * from './input/index'; export * from './view/index'; export * from './decor/index'; export * from './plugin/index'; export * from './language/index';`.
  Also export the contract types from `./types`.
- Each area `index.ts` re-exports its modules.
- `decor`, `plugin` and `language` barrels contain only `export {};`.
- Barrel ownership after this wave:
  - `decor/index.ts`: EMK-DECOR;
  - `language/index.ts`: EMK-LANG;
  - `view/index.ts` and `plugin/index.ts`: EMK-WIDGETS in wave 2, then
    `plugin/index.ts` passes to EMK-API in wave 3.

### T4: Heightmap (`layout/heightmap.ts`), full implementation (EM-D8)

API (0-based lines):
- `new Heightmap(lineHeight: number)`
- `setLineHeight(px)`, `setLineCount(n)`
- `setBlocks(entries: readonly { line: number; above: number; below: number }[])`:
  entries may be unsorted. Merge duplicates by summing. Drop lines `>= count`
  and zero entries. Throw `RangeError` on negative or non-finite heights.
- `lineTop(line)`, `blockAbove(line)`, `blockBelow(line)`
- `lineAtHeight(y): { line; region: 'text' | 'above' | 'below' }`
- `totalHeight()`, `readonly blockCount`
- `readonly revision: number`: starts at 0. It is bumped only on an
  effective change:
  - `setLineHeight` with a different value;
  - `setLineCount` with a different count, or one that drops blocks;
  - `setBlocks` whose merged, sorted entries differ from the stored ones.

  Setting identical blocks again must not bump it. The renderer cache keys
  include it (T3).

Store sorted entries with a prefix-sum `Float64Array`. Lookups are binary
search. With zero blocks, `lineTop` is exactly `line * lineHeight`.

`lineAtHeight` rules:
- `y < 0`: line 0, region `above` if line 0 has an above block, else `text`;
- `y >= totalHeight`: the last line, region `text`;
- `y` inside a band: that band's anchor line and side.

### T5: Moved tests, plus new tests

**Moved tests.** Copy these five files, changing only their import specifiers:

| From | To |
|------|----|
| `editor/test/code/atlas-cells.test.ts` | `emaki/test/render/atlas-cells.test.ts` |
| `editor/test/code/glyph-placement.test.ts` | `emaki/test/render/glyph-placement.test.ts` |
| `editor/test/code/text-cells.test.ts` | `emaki/test/layout/text-cells.test.ts` |

Operator amendment (after session 597): `text-cells.test.ts` reads
`examples/kalimba.vact`, which is outside the package. Copy that file
byte-identically to `emaki/test/fixtures/kalimba.vact` (added to this plan's
writePaths) and change only the path expression in the moved test so it
reads the package-local copy (resolve it relative to the test file, e.g.
`new URL('../fixtures/kalimba.vact', import.meta.url)`). The test name may
keep mentioning kalimba.vact; assertions and counts stay unchanged. Record
the sha256 of both files.
| `editor/test/canvas/frame.test.ts` | `emaki/test/render/frame.test.ts` |
| `editor/test/canvas/cursor-shape.test.ts` | `emaki/test/render/cursor-shape.test.ts` |

- Copy `editor/test/code/recording-gl.ts` to
  `emaki/test/support/recording-gl.ts`, adjusting imports only.
- Record each file's `it(` count before and after in
  `tmp/emaki/EMK-CORE/test-disposition.json` as
  `[{from, to, before, after}]`. The counts must be equal.
- Do NOT delete the editor originals. EMK-VAC-SWITCH does that.

**`emaki/test/boundary.test.ts`** (design 14.1). Header
`// @vitest-environment node`. Load `node:fs` through a non-literal dynamic
import, imitating `editor/test/code/vim/purity.test.ts`. Walk `emaki/`
excluding `node_modules` and `dist`, with the root from `process.cwd()`.

For `.ts`, `.mjs` and `.html` files, fail when:
- a relative import specifier resolves outside `emaki/`;
- a file under `src/` imports a bare package other than `@codemirror/state`
  or `@codemirror/commands`;
- a test, script or example imports a bare package other than those,
  `vitest`, `jsdom`, `playwright` or `node:*`;
- any file contains the substrings `solid-js`, `vactr` or `/editor/`.

**Self-match pitfall (review low).** The forbidden substrings and the
classifier samples must not appear literally in `boundary.test.ts` or any
other emaki file. Otherwise the scan, and Verification step 5's grep, match
the test itself. Build them by concatenation, for example `'solid' + '-js'`,
`'vac' + 'tr'` and `'/' + 'editor/'`, both in the rule list and in the
in-memory samples. Do NOT add a file exclusion for `boundary.test.ts`.

Tests:
- the scan over the real tree -> no violations;
- a unit test of the classifier on in-memory samples:
  `import x from '../../editor/src/a'` -> escaping;
  `'solid-js'` -> violation;
  `'@codemirror/view'` in `src` -> violation;
  `'./ok'` -> fine.

**`emaki/test/layout/heightmap.test.ts`**
- no blocks, lineHeight 18 -> `lineTop(5) = 90`, `lineAtHeight(91)` gives
  `{5, text}`, `totalHeight(10 lines) = 180`;
- block below line 2 of 40 px -> `lineTop(3) = 54 + 40`,
  `lineAtHeight(60)` gives `{2, below}`, `lineAtHeight(94)` gives `{3, text}`;
- block above line 0 of 10 -> `lineAtHeight(5)` gives `{0, above}`,
  `lineTop(0) = 10`;
- duplicate entries are summed; a negative height throws;
- 100k lines with 5k random blocks -> `lineAtHeight(lineTop(i) + 1)` gives
  `i` for 1000 sampled `i` (round trip);
- `setLineCount` smaller drops blocks beyond the end.

**`emaki/test/render/decoration-order.test.ts`.** Uses `recording-gl` and the
renderer, imitating `editor/test/canvas/gpu.test.ts` fixture lines 377-381.

Input:
- `selection [0,3]`;
- `animatedMarks` `[{4,5,{background:[0.7,0.58,0.26,0.5]}},{0,9,{background:[0.4,0.64,0.72,0.5]}}]`;
- `tokens [{0,3,'t'}]`;
- a static mark `{4,5,{underline:{color:[0.75,0.2,0.25,1],thickness:2}}}`;
- `composition [6,9]`;
- a label mark `{4,5,{label:{text:'CC 1'}}}`;
- `cursor 5`, `handles`.

Expected:
- the first four draws are `[bg 0.85 alpha, selection, 0.7-color, 0.4-color]`;
- the underline index is greater than 3;
- the cursor index is greater than the underline index;
- the label fill index is greater than the cursor index;
- the 0.7 color is never in the text layer.

**`emaki/test/render/heightmap-geometry.test.ts`** (EMK-R5-01; recording GL
fixture as in decoration-order). Setup: lineHeight 18, doc `'a\nb\nc'`,
scrollTop 0.
- `layout.heightmap.setBlocks([{ line: 0, above: 0, below: 40 }])`, then
  render with an animated mark `{2,3}` on line 1 and selection `[2,3]` ->
  the line-1 text slot y is 58 (read the slot table upload through
  recording-gl), the animated mark rect top is 58, the selection rect top
  is 58, and the line-2 slot y is 76;
- then change the blocks to `below: 20` and render an otherwise identical
  input -> the text buffer is rewritten (`bufferSubData` count on the text
  buffer increases, or the slot table is re-uploaded), the line-1 y is 38,
  and the background and overlay layers rebuild (`heightmap.revision`
  changed);
- the same `setBlocks` twice -> the revision is unchanged, and the second
  render reuses all layers (no new text writes);
- zero-blocks control: render the decoration-order input with a fresh
  heightmap and record the full draw list (`rect`, `color`, `layer`). It
  must equal the draw list computed with `lineTop` replaced by
  `n * lineHeight` arithmetic expectations: the line-n slot y is exactly
  `n * 18`, and the animated rect tops are exactly `n * 18`;
- a 3-line doc with a 1000 px block below line 0, scrollTop 500 (inside the
  band) -> no glyph row for line 0 is drawn inside the band's y range, and
  lines 1-2 are the visible rows.

**`emaki/test/core/editor-core.test.ts`**
- `onTransaction` runs before `subscribe` callbacks, and exactly once per
  dispatch;
- a reentrant dispatch from a subscriber throws;
- a stale transaction throws;
- `externalHistory.trimToBytes` is called during `boundHistory`;
- `deferSourceWrite` defers inside the composition range and runs on clear.
- A test subclass `class Sub extends EditorCore` that calls
  `this.publish(...)` from a method -> subscribers receive the update, and
  `new InputController(new Sub(...), el)` type-checks.

## Invariants

- Nothing under `editor/` changes. `cd editor && npm run check` and vitest
  stay exactly as at `a6e5bea`.
- With no blocks and the vactr kinds mapped as in design 13.2, emaki
  rendering produces today's draw list. EMK-VAC-SWITCH proves this with the
  unchanged `gpu.test.ts`.
- **Zero-blocks parity (EMK-R5-01).** With zero heightmap blocks, every
  emitted rect, slot position and visible-row range is identical to today's
  `line.number * lineHeight` geometry.
- **Blocks honored.** With blocks, glyph rows, gutter numbers, animated mark
  rects, selection, composition and underline rects all sit at
  `heightmap.lineTop`.
- **Cache invalidation.** Any heightmap revision change invalidates the
  text, background and overlay caches.
- `emaki/src` contains no `vactr`, `vact-tok`, `--vt-`, `solid-js`,
  `../app/`, `../protocol/` or `../ui/` string.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-CORE/`.

1. `npm ci --prefer-offline`: exit 0.
2. `npm run check`: exit 0, zero errors.
3. `./node_modules/.bin/vitest run`: exit 0. testsRun is greater than 0
   and includes the 5 moved files with unchanged counts, plus boundary,
   heightmap, decoration-order and editor-core.
4. `npm run build`: exit 0. `dist/index.html` exists.
5. `grep -rn "vactr\|vact-tok\|--vt-\|solid-js" src test examples || true`:
   no matches. Record the output.
6. `wc -l src/**/*.ts test/**/*.ts`: every file under 1000 lines.
7. From the repo root: `git status --porcelain editor` -> empty.
8. `node --check scripts/route-server.mjs`: exit 0.

**mutationEvidence.** Do not create any new file for this (operator
amendment after wave 1: a scratch file outside writePaths is not allowed).
Temporarily append the line `import '../../editor/src/app/apis';` to
`emaki/src/types.ts` (already in this plan's writePaths). The boundary test
fails. Remove the line again (the file must be byte-identical to its
pre-mutation content) and confirm the boundary test passes.

## Completion Criteria

- [x] T1-T5 deliverables exist at the listed paths. `package.json` matches
  T1 exactly (check with `node -e` printing `exports` and `peerDependencies`).
- [ ] Verification steps 1-7 pass and are recorded in the record shape.
  Mutation evidence is recorded.
- [x] `test-disposition.json` shows equal counts for all 5 moved files.
- [x] The Progress Log has pre/post sha256 for every written file.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
**Notes**: Ready for wave 1.

### Session: 2026-11-01 (partial implementation)
**Tasks Completed**: None fully. Implemented the sparse heightmap in
`emaki/src/layout/heightmap.ts`; pre-write state was absent, post-write SHA-256
`3539e29717ed8ec701d43f8ddd8aedeff5940062e4591b65af03319134fbe87c`.
**Verification**: `node --test tmp/emaki/EMK-CORE/heightmap-smoke.mjs`
passed 2 tests (log `tmp/emaki/EMK-CORE/heightmap-smoke-final.log`). This
is supplemental evidence, not a substitute for the required package gates.
**Remaining**: T1-T3 and T5, heightmap integration and required T4 tests,
package verification, mutation evidence, and per-file hashes. Formal review,
commit and integration remain downstream workflow steps.

### Session: 2026-11-01 (continuation; blocked)
**Progress**: Added the package scaffold, contracts, draft core/input/view,
layout and rendering files, five copied test files and initial focused tests.
The copied test counts are recorded in `tmp/emaki/EMK-CORE/test-disposition.json`.
The renderer pass separation and integration remain incomplete; none of the
T1-T5 completion criteria is asserted complete. Per-file intent evidence and
source hashes are under `tmp/emaki/EMK-CORE/`.
**Blocker**: An offline `npm install --prefer-offline --offline --no-audit
--no-fund` using a copy of the existing cache failed with `ENOTCACHED` for the
`@codemirror/commands` registry response. See
`tmp/emaki/EMK-CORE/npm-install.log`. T1 requires stopping on a cache miss.
Supply the missing cached registry response and package dependencies, or amend
the plan to permit fetching, before resuming installation, implementation,
and all required package checks. No package installation completed.

### Session: 2026-11-01 (fixture ownership blocker)
**Progress**: Read the accepted design and plan and confirmed the assigned plan has no dependencies. No source files were changed. The initial `cd emaki && npm run check` passed (exit 0; `tmp/emaki/EMK-CORE/step6-check-initial.log`).
**Blocker**: `emaki/test/layout/text-cells.test.ts` reads `../examples/kalimba.vact`, outside the standalone package. The moved-test rule permits only import-specifier changes, and no package-local fixture path is authorized. Amend the plan to authorize a self-contained fixture and the necessary test reference change, or otherwise reconcile the standalone requirement with the unchanged moved test. Renderer pass separation, required rendering tests, barrels and final verification remain incomplete.
**Evidence**: `tmp/emaki/EMK-CORE/step6-fixture-blocker-sha256.txt` records the plan hashes. Formal review and integration remain downstream.

### Session: 2026-11-01 (EMK-CORE continuation)
**Progress**: Added engine and area barrels, copied the package-local fixture and updated the moved test fixture path, and added initial renderer order and heightmap geometry tests. The fixture hashes match: `40968a7beccd3413ff08ce3aa8472f2cc0aa01608969b3fe464f085e350e21b7`. Per-edit intentions and pre/post hashes are recorded in `tmp/emaki/EMK-CORE/step6-597-intentions.txt` and `step6-597-hashes.txt`.
**Verification**: `npm run check` passed (exit 0, `step6-check-final.log`); full vitest passed 62 tests (exit 0, `step6-vitest-final.log`); locked `npm run build` passed (exit 0, `step6-build-final.log`); route syntax, forbidden strings, source line limits, and unchanged editor status passed (`step6-route.log`, `step6-forbidden.log`, `step6-line-count.log`, `step6-editor-status.log`). Boundary negative control failed as intended (exit 1, `step6-mutation.log`), and byte-identical restoration and two passing boundary tests are recorded in `step6-mutation-hash.txt` and `step6-boundary-restored.log`. An earlier band test failed due to an offscreen overscan assertion and was corrected to inspect visible nonempty geometry (`step6-band-attempt.log`, `step6-band-final.log`).
**Remaining**: Full renderer pass separation and the complete prescribed heightmap geometry/cache and zero-block draw-list parity assertions remain. Audit remaining T1-T5 details and complete required final-source gates before marking criteria complete. The plan prohibits reinstalling the operator-provided dependencies; `npm ci --prefer-offline` remains for the orchestrator. No drift detected. Formal review and integration remain downstream.

### Session: 2026-11-01 (renderer and geometry continuation)
**Progress**: Moved text slots, segment construction and glyph rendering into `emaki/src/render/text-pass.ts`, and static/animated decoration construction into `emaki/src/render/decoration-pass.ts`; `renderer.ts` retains orchestration and cache keys. Expanded geometry tests for static background/overlay invalidation, idle reuse, block positions and full fixed-height draw-list parity. Exposed the render area exports and corrected `ViewHost` caret caching to include heightmap revision, with a regression test. Per-edit intentions and hashes are in `tmp/emaki/EMK-CORE/continuation-2-intentions.txt`, `continuation-2-hashes.txt` and `continuation-2-final-hashes.txt`; no drift detected.
**Verification**: `npm run check` exit 0 (`continuation-2-final-check.log`); `./node_modules/.bin/vitest run` exit 0, 63 tests passed (`continuation-2-final-vitest.log`); locked `npm run build` exit 0 (`continuation-2-final-build.log`). Route syntax, forbidden text scan, line counts, package exports/peers, and unchanged editor status exited 0 (`continuation-2-final-route.log`, `continuation-2-final-forbidden.log`, `continuation-2-final-line-count.log`, `continuation-2-final-package.log`, `continuation-2-final-editor-status.log`). All logs are under `tmp/emaki/EMK-CORE/`.
**Mutation evidence**: Escaping import control exited 1 (`continuation-2-mutation.log`); `continuation-2-mutation-hashes.txt` proves byte-identical restoration; boundary test then passed 2 tests (`continuation-2-restored-boundary.log`). The first check during extraction failed on an intermediate missing parameter and was repaired (`continuation-2-check-attempt.log`).
**Remaining**: The operator amendment prohibits reinstalling the supplied dependencies; `npm ci --prefer-offline` is reserved for the orchestrator outside the sandbox. Formal review, integration and commit are downstream.
