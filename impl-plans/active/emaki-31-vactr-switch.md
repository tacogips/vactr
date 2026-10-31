# EMK-VAC-SWITCH: vactr consumes emaki (file:../emaki), adapters, deletion of moved engine files

**Status**: Ready (after wave 2)
**Plan ID**: EMK-VAC-SWITCH (wave 3; parallel with EMK-API)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 4.1-4.4, 9.3, 11.3 (consumer config), 13.1, 13.2, 13.7, 14.2, 15 (G5-G8); decisions EM-D3, EM-D4, EM-D15, EM-D17, EM-D18, EM-D19
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants vactr to load its editor from the emaki library. After this
plan:

- `editor/package.json` depends on `"emaki": "file:../emaki"`;
- the engine files moved by EMK-CORE are deleted from `editor/src/code`;
- vactr composes the `emaki/engine` blocks in `mount.ts`, as it does today;
- vact syntax is wrapped as an emaki span-kind language provider.

Behavior must be identical. Every existing vactr test keeps its assertions.
The only test edits allowed are import specifiers, and `as` aliases inside
import statements.

There is no beat glow or inline widget yet (EMK-VAC-PLUGINS).

## Non-goals

- No `PluginRuntime`, beat-glow plugin, sequencer, scope or toggle.
- No change to `editor/src/code/vim/**` (EM-D17), the protocol, the
  session, Rust, the scheduler or the grammar.
- No change to any `expect(...)`, fixture or tolerance.
- No emaki feature work. emaki files are touched only for parity repairs
  (see sharedPaths).

## Dependencies

- **dependsOn**: EMK-CORE, EMK-DECOR, EMK-WIDGETS, EMK-LANG.
- Runs in parallel with EMK-API, whose files are disjoint from this plan's.

## writePaths

Config:
- `editor/package.json`, `editor/package-lock.json`,
  `editor/vite.config.ts`, `editor/vitest.config.ts`, `editor/tsconfig.json`

vactr adapters and glue (rewritten):
- `editor/src/code/mount.ts`, `editor/src/code/surface.ts`,
  `editor/src/code/renderer.ts`, `editor/src/code/history.ts`
- `editor/src/code/perf-hook.ts`, `editor/src/code/vim-mount.ts`,
  `editor/src/code/momentary-pointer.ts`,
  `editor/src/code/unapplied-settings.ts`

New vactr files:
- `editor/src/code/decorations.ts`, `editor/src/code/theme.ts`,
  `editor/src/code/vact-language.ts`

Import-only updates:
- `editor/src/app/apis.ts`, `editor/src/bind/drag.ts`,
  `editor/src/params/mount.ts`
- `editor/src/visual/video.ts`, `editor/src/visual/mount.ts`,
  `editor/src/visual/background.ts`

Deleted engine sources:
- `editor/src/code/atlas.ts`, `frame.ts`, `geometry.ts`, `segments.ts`,
  `palette.ts`, `cursor-shape.ts`, `resources.ts`, `renderer-types.ts`
- `editor/src/code/layout.ts`, `advances.ts`, `input.ts`, `keyboard.ts`,
  `pointer.ts`, `accessibility.ts`, `view-host.ts`

Deleted moved tests:
- `editor/test/code/atlas-cells.test.ts`,
  `editor/test/code/glyph-placement.test.ts`,
  `editor/test/code/text-cells.test.ts`
- `editor/test/canvas/frame.test.ts`, `editor/test/canvas/cursor-shape.test.ts`

Test import updates (specifiers only):
- `editor/test/canvas/`: `edit-cost.test.ts`, `first-viewport.test.ts`,
  `gpu.test.ts`, `input-keymap-hooks.test.ts`, `input-vim-hook.test.ts`,
  `input.test.ts`, `mount.test.ts`, `renderer-hop.test.ts`,
  `view-host.test.ts`, `vim-input.test.ts`, `vim-keymap-input.test.ts`,
  `vim-keymap-mount.test.ts`, `vim-mount.test.ts`
- `editor/test/code/`: `recording-gl.ts`, `renderer-layers.test.ts`,
  `vim/controller-io.test.ts`
- `editor/test/visual/`: `background.test.ts`, `overlays.test.ts`

New tests:
- `editor/test/code/emaki-boundary.test.ts`,
  `editor/test/code/emaki-single-instance.test.ts`,
  `editor/test/code/decorations.test.ts`

Generated:
- `editor/node_modules`, `editor/dist`, `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm` (artifactRoots)

This plan's own file:
- `impl-plans/active/emaki-31-vactr-switch.md`

## sharedPaths

Parity repair only (see sharedPathNotes):
- `emaki/src/render/renderer.ts`, `emaki/src/render/text-pass.ts`,
  `emaki/src/render/decoration-pass.ts`
- `emaki/src/layout/layout.ts`
- `emaki/src/input/input.ts`, `emaki/src/input/keyboard.ts`,
  `emaki/src/input/pointer.ts`
- `emaki/src/view/view-host.ts`
- `emaki/src/core/editor-core.ts`

**sharedPathNotes (the same note for every path above).** Edit only to fix a
behavior-parity defect that an unchanged vactr test exposes. Each fix
records:
- the failing test name;
- the pre/post sha256;
- one emaki regression test, added only if it fits an existing emaki test
  file this plan may not own. Otherwise describe it in the Progress Log for
  the review step.

No feature changes. EMK-API does not edit these files in wave 3.

## Tasks

### S1: Baseline before any edit

Run under the lock:
`cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run`,
after building the wasm lib (rule 8, first command). Record:
- `testsRun`, expected 2384;
- the per-file counts of the 5 moved test files, in
  `tmp/emaki/EMK-VAC-SWITCH/baseline-vitest.log`.

### S2: Dependency and config (design 11.3)

- **`editor/package.json`**: add `"emaki": "file:../emaki"` to
  `dependencies`. Then run `cd editor && npm install --prefer-offline`. No
  other version changes: `git diff editor/package-lock.json` may show only
  the emaki link entries.
- **`vite.config.ts`** and **`vitest.config.ts`**:
  - `resolve.dedupe: ['@codemirror/state', '@codemirror/commands']`;
  - `server.fs.allow` gains `'../emaki'`;
  - in vitest, keep the existing `resolve.conditions` and add
    `test.server.deps.inline: ['emaki']`.
- **`tsconfig.json`**: add `"preserveSymlinks": true`.

### S3: vactr adapters

**`theme.ts`** gets the vactr-specific contents of today's
`editor/src/code/palette.ts`: `TOKEN_PROPERTIES`, `FIELD_PROPERTIES` (incl.
`diagnostic` and `callHead`), `LEGACY_TOKENS` and `LEGACY_FIELDS`. It exports:
- `type VactrPalette = Palette & { diagnostic: Rgba; callHead: Rgba }`;
- `FALLBACK_PALETTE: VactrPalette`, with today's exact values. Token map:
  `vact-tok-*`. `selection` and `background` use the emaki defaults;
- `readPalette(root): VactrPalette`, with today's semantics: `callHead` alpha
  forced to 0.6, invalid values fall back. It calls the emaki generic
  `readPalette` with the vactr property maps, then adds `diagnostic` and
  `callHead`;
- `rgbaCss` and `type Rgba`, re-exported from `emaki/engine`;
- `GPU_TOKEN_COLORS`, moved from `renderer.ts` with identical values.

`gpu.test.ts:869-885` must pass unchanged, with only its import line
switched to `../../src/code/theme`.

**`decorations.ts`** has
`toRenderInput(feedback: RenderFeedback, palette: VactrPalette): RenderInput`,
following design 13.2:

| vactr kind | emaki input |
|------------|-------------|
| `syntax` | `tokens` (`tag = className ?? ''`) |
| `selection` | `selection` |
| `composition` | `composition` |
| `unapplied` | static mark `{ background: unappliedRgba(feedback.unappliedColor ?? DEFAULT), minWidth: 6 }` |
| `diagnostic` | static mark `{ underline: { color: palette.diagnostic, thickness: 2 } }` |
| `call-head` | static mark `{ underline: { color: palette.callHead, thickness: 1 } }` |
| `binding` (with label) | static mark `{ label: { text } }` |
| `playing` | animated mark `PLAYING_MARK` (no glow in this plan) |
| `eval` | animated mark `EVAL_MARK` or `EVAL_ERROR_MARK` |
| `momentary` (with label) | animated mark `{ label: { text } }` |

- `playing`, `eval` and `momentary` are animated whether they arrive in
  `feedback.annotations` or `feedback.animated`. The order is annotations
  first, then animated (`renderer.ts:273`).
- `decorationsKey` = `${annotationsRevision ?? 0}|${unappliedColor ?? DEFAULT_UNAPPLIED_COLOR}`.
- Pass through cursor, shape, end, visibility, handles, hop labels,
  revisions and `textRevision`.
- Export the constants:
  - `PLAYING_MARK = { background: [0.7, 0.58, 0.26, 0.5] }`;
  - `EVAL_MARK = { background: [0.4, 0.64, 0.72, 0.5] }`;
  - `EVAL_ERROR_MARK = { background: [0.8, 0.24, 0.32, 0.5] }`.

  EMK-VAC-PLUGINS adds the glow to `PLAYING_MARK`. Do not add it here.
- `RenderFeedback` is today's interface from `renderer.ts:18-28`, moved here
  unchanged, plus one optional field:
  `plugin?: { marks?: readonly Mark[]; animatedMarks?: readonly AnimatedMark[]; lineFormats?: readonly ResolvedLineFormat[]; key?: string }`.
  Plugin entries are appended after the converted entries in each list, and
  `plugin.key` is appended to `decorationsKey`. This is the contract for
  EMK-VAC-PLUGINS.

**`renderer.ts`** (at most 150 lines): `class CanvasRenderer extends EmakiCanvasRenderer`
(import `{ CanvasRenderer as EmakiCanvasRenderer } from 'emaki/engine'`).
- The constructor has the same signature as today
  (`canvas, layout, options`). The `palette` option is a `VactrPalette`,
  default `FALLBACK_PALETTE` from `theme.ts`.
- Override `setPalette(p: VactrPalette)`: keep `diagnostic` and `callHead`,
  then call `super.setPalette(p)`.
- Override `render(feedback?: RenderFeedback): boolean`:
  `return super.render(toRenderInput(feedback ?? {}, this.vactrPalette))`.
- Re-export `GPU_TOKEN_COLORS` from `./theme`, and `type GpuStatus`,
  `type RenderFeedback`.
- No GL code.

If TypeScript rejects the narrowed override parameter, use composition:
- a class `CanvasRenderer` that holds an `EmakiCanvasRenderer` and forwards
  every public member that tests use;
- grep `editor/test` and `editor/src` for `renderer.` and `f.renderer.`
  member names, and forward each one;
- record which approach you used.

**`surface.ts`**: `class CodeSurface extends EditorCore implements SurfaceContract`.
- The constructor takes `{ sync, extensions?, historyByteLimit? }`, calls
  `super({ doc: sync.history.text(sync.history.current), extensions, historyByteLimit, externalHistory: sync.history })`,
  then registers two `onTransaction` hooks in order:
  1. `sync.apply(tr)`;
  2. the owner-annotation remap from today's `surface.ts:70-72`.
- It keeps from today's code: `sync`, `annotate`/`annotationRanges` (use
  `this.publish`), `registerAnimated`/`animatedRows`/`onAnimatedWake`,
  `registerMomentaryDrag`/`momentaryHit`/`momentaryDrag`, and the
  `retentionStatus` getter. Today's `retentionStatus` reads `undoBytes`
  from history trimming; read it from `super.retentionStatus` and merge
  `sync.history.retainedBytes`.
- `dispose()` calls `super.dispose()` and then `sync.doc.dispose()`.
- The order inside `dispatch` must equal today's: state, then `sync.apply`,
  then the annotation remap, then the composition remap, then
  `boundHistory`, then publish. EMK-CORE pinned this.

**`history.ts`**:
- remove `retainedBytes`, `trimUndoHistory`, `HISTORY_UNDO_BYTES`,
  `INDEX_BYTES`, `HISTORY_LIMIT` and `touches`, and re-export them from
  `emaki/engine`, so existing imports keep working;
- keep `RevisionHistory`, `Range16` and `LineTable` use.

**`vact-language.ts`**: `createVactLanguage(initial: SpanProvider): { language: SpanLanguage; replace(next: SpanProvider): void }`.
- `language.spans(state, from, to, limit)` delegates to the current
  provider and maps its `CodeAnnotation` rows (`kind 'syntax'`) to
  `TokenSpan` (`tag = className`). The adapter maps them back to `syntax`
  for the renderer.
- `noteChanges` and `dispose` delegate.
- `mount.ts` replaces its `syntaxProvider` variable with this wrapper. The
  `instanceof FallbackSpans` check and the `data-syntax` values stay
  byte-identical.

### S4: `mount.ts` and other glue

**`mount.ts`**
- Imports come from `emaki/engine`, plus local adapters.
- `InputController` options add
  `fallbackShortcut: fallbackStopShortcut` (import from `../ui/stop-keys`).
  Keep `stopAll` and `cut` exactly as today (lines 204-205).
- The syntax window (lines 254-257) uses
  `layout.heightmap.lineAtHeight(view.scrollTop - view.height).line` and
  `lineAtHeight(view.scrollTop + view.height * 2).line`, clamped. With no
  blocks these equal today's values.
- `readPalette` comes from `./theme`, and `CodeViewHost` from
  `import { ViewHost as CodeViewHost } from 'emaki/engine'`.
- Keep everything else unchanged.

**Other glue files**
- `momentary-pointer.ts`: replace the expando writes (lines 34 and 46) with
  `markDragGesture(e)`.
- `params/mount.ts:166`: read `isDragGesture(event)`.
- `apis.ts`, `bind/drag.ts`: `NumericGesture` comes from `emaki/engine`.
- `visual/video.ts`, `visual/mount.ts`, `visual/background.ts`:
  `RESOURCE_LIMITS` and `ResourceLedger` come from `emaki/engine`.
- `perf-hook.ts`: `PERF_PHASES`, `PerfPhase`, `PerfRecorder` and
  `ResourceLedger` come from `emaki/engine`. `CodeRenderer` becomes
  `CanvasRenderer` from `./renderer`.
- `vim-mount.ts`: type imports come from `emaki/engine`. Attach the Vim
  hook with `input.setVim(...)` (unchanged).
- `unapplied-settings.ts`: `Rgba` comes from `emaki/engine`.

### S5: Tests

**Delete** the 5 moved test files. They now live in `emaki/test`, and
EMK-CORE recorded their counts.

**Update import specifiers** in the files listed under writePaths. Map each
old module to:
- `emaki/engine` for atlas, frame, geometry, segments, resources, layout,
  advances, input, keyboard, pointer, accessibility and view-host (use
  `ViewHost as CodeViewHost`);
- `../../src/code/theme` for palette;
- `../../src/code/renderer` for renderer (unchanged path) and
  renderer-types.

Then:
- `git diff -U0 editor/test` must show only lines that begin with
  `import`, or lines inside import statements;
- record a grep-based check in the Progress Log:
  `git diff -U0 editor/test | grep '^[+-]' | grep -v '^[+-][+-]' | grep -v 'import\|from '`
  must print nothing, other than deleted-file markers.

**New tests**
- **`emaki-boundary.test.ts`** (design 14.2; node env; fs walk as in
  `editor/test/code/vim/purity.test.ts`). It fails when any of these holds:
  - an `emaki...` specifier is outside the four entries;
  - a relative path contains `/emaki/` or `node_modules/emaki`;
  - any of the 15 deleted source files or 5 moved test files exists;
  - `editor/src/code` contains `getContext('webgl2'`, `#version 300 es`
    or `class GlyphAtlas`.

  Also include a classifier unit test with in-memory samples.
- **`emaki-single-instance.test.ts`**: build a `DocumentSync` and the vactr
  `CodeSurface`, imitating `editor/test/canvas/state.test.ts` setup. Assert
  `surface.state instanceof EditorState` and
  `surface.state.doc instanceof Text`, where `EditorState` and `Text` are
  imported from `@codemirror/state` in the test. Also assert
  `surface instanceof EditorCore` (from `emaki/engine`).
- **`decorations.test.ts`**:
  - every kind maps per the table;
  - `playing` in `annotations` and in `animated` keeps that order;
  - `eval` with `className: 'eval-error'` gives `EVAL_ERROR_MARK`;
  - an `unapplied` empty range gives `minWidth: 6`;
  - `plugin` lists are appended after the converted entries.

**Test-disposition record.** Write
`tmp/emaki/EMK-VAC-SWITCH/test-disposition.json` with:
- the S1 baseline;
- the final vactr count;
- the emaki count from `cd emaki && ./node_modules/.bin/vitest run`;
- the moved-file counts (from EMK-CORE's record).

Gate: `vactr + emaki >= 2384 + new tests`, and the moved files are equal.

### S6: Packaging proof, present case (G5 part 1)

With `emaki/node_modules` present, run:
`cd editor && rm -rf node_modules && npm ci --prefer-offline && npm run check && ./node_modules/.bin/vitest run test/code/emaki-single-instance.test.ts`.
It must exit 0.

The absent-`emaki/node_modules` case runs serially in EMK-CLOSEOUT, because
deleting `emaki/node_modules` here would break EMK-API, which runs in
parallel. Do NOT delete `emaki/node_modules` in this plan.

## Invariants

- Every assertion in `editor/test` is unchanged. Vim behavior is unchanged.
- stop-all vs cut routing is unchanged:
  - `test/canvas/input.test.ts`, `input-vim-hook.test.ts`,
    `vim-input.test.ts` and `vim-keymap-input.test.ts` pass;
  - `vim-mount.test.ts` alternate-shortcut cases pass.
- `data-syntax` values and the perf hook (`?perf=1`) are unchanged.

## Verification

Logs go to `tmp/emaki/EMK-VAC-SWITCH/`. Heavy steps take the lock.

0. **Precondition.** The wave-2 join gate (rule 12a) is green: cite
   `tmp/emaki/wave-2-join/`.
1. `cd editor && npm run check`: exit 0. This plan is the only editor writer
   in wave 3, so this is a full pass gate. An error inside an emaki file
   that EMK-API owns is drift per rule 12b; record it and wait for the
   wave-3 join.
2. `export CARGO_TERM_QUIET=true; cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`:
   exit 0.
3. Full vitest (lock):
   `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run`.
   Expect exit 0, failures 0, and testsRun as reconciled in S5.
4. `cd emaki && ./node_modules/.bin/vitest run test/render test/layout test/input test/core`:
   exit 0. These cover the parity sharedPaths this plan may touch.
   `cd emaki && npm run check` passes per rule 12b: EMK-API's files
   (`src/plugin/*`, `src/editor*.ts`, `src/index.ts`) may be mid-edit, and
   errors there are drift. The wave-3 join gate is authoritative for emaki.
5. `cd editor && npm run test:style` (lock), after step 6's build. Expect
   16/16 combinations.
6. Rebuild and e2e (lock):
   - `mise run build-wasm-release`
   - `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`
   - `cd editor && npm run e2e -- --browser all --profile behavior --out ../tmp/emaki/EMK-VAC-SWITCH/e2e`
   - `cd editor && npm run e2e:examples`

   Expect failureCount 0, Chromium and WebKit both, and examples 8/8.
7. S6 packaging proof (present case): one exit-0 record.
8. `wc -l` on every written source file: each under 1000.
9. `ls editor/src/code`: none of the 15 deleted files is listed.

**mutationEvidence**
- Add `import '../../../emaki/src/render/atlas'` to a scratch file
  `editor/src/code/zz-mutation.ts` -> `emaki-boundary.test.ts` fails. Delete
  the file afterwards.
- Swap `PLAYING_MARK` and `EVAL_MARK` colors -> `gpu.test.ts` "draws all
  required feedback in order" fails. Restore afterwards.

## Completion Criteria

- [ ] S1-S6 done. The 20 files are deleted.
- [ ] The test diff is limited to import lines (recorded check).
- [ ] Verification steps 1-9 pass and are recorded. Mutation evidence is
  recorded.
- [ ] Every sharedPath edit is justified by a failing unchanged test, with
  hashes.
- [ ] The Progress Log is complete.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
