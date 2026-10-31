# EMK-VAC-PLUGINS: beat glow and eval flash as marks, inline sequencer and oscilloscope widgets, show/hide toggle

**Status**: Ready (after EMK-API and EMK-VAC-SWITCH)
**Plan ID**: EMK-VAC-PLUGINS (wave 4; parallel with EMK-DIFF-DEMO, EMK-JSON-DEMO, EMK-SHOWCASE)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 13.2-13.7; decisions EM-D10, EM-D13, EM-D16; user-QA defaults EQ2, EQ4, EQ5
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user asked for three things:
- the currently sounding beat glows through emaki's character-format
  feature;
- vactr's sequencer appears as an emaki line plugin;
- vactr's oscilloscope appears as an emaki line plugin.

After EMK-VAC-SWITCH, vactr composes the emaki engine in `mount.ts`, and its
renderer adapter accepts a `plugin` field (`decorations.ts` `RenderFeedback`).
This plan:

1. **Beat glow and eval flash.** `PLAYING_MARK` gains a glow. A beat-glow
   plugin supplies the playing and eval-flash animated marks through
   `PluginRuntime`. `mount.ts` stops passing playing and eval as annotation
   kinds.
2. **Inline sequencer.** A block widget below each slot's form, drawn from
   the same `playing` telemetry via `CycleSteps`, with the current step lit
   (EM-D13).
3. **Inline oscilloscope.** A block widget below each `#@ editor` line that
   has an `oscilloscope` visualizer, drawing the master waveform via
   `visual/scopes.ts` (EQ5).
4. **Toggle.** A persisted "Inline widgets" toggle, `vactr.inlineWidgets`,
   default on (EM-D16 / EQ2), in the editor options UI.

## Non-goals

- No change to `HighlightScheduler` timing, the active-set computation,
  `recordPresented` keys, the params-pane step grid, the background
  overlays, or the analyzer pane.
- No write-back from the sequencer. It is display only.
- No inline widget for bus analyzer units (EQ5).
- No change to the protocol, Rust or `vim/**`.

## Dependencies

- **dependsOn**: EMK-API (`PluginRuntime`), EMK-VAC-SWITCH (adapters and
  `RenderFeedback.plugin`).

## writePaths

Editor code:
- `editor/src/code/mount.ts`, `editor/src/code/decorations.ts`,
  `editor/src/code/beat-glow.ts`, `editor/src/code/inline-settings.ts`
- `editor/src/code/inline/anchors.ts`,
  `editor/src/code/inline/sequencer.ts`, `editor/src/code/inline/scope.ts`
- `editor/src/code/transport.ts`, `editor/src/code/code.css`

Other editor sources:
- `editor/src/ui/transport-view.tsx`, `editor/src/app/app.css`
- `editor/src/visual/settings.ts`

Tests:
- `editor/test/code/beat-glow.test.ts`, `editor/test/code/inline-anchors.test.ts`,
  `editor/test/code/inline-sequencer.test.ts`,
  `editor/test/code/inline-scope.test.ts`,
  `editor/test/code/inline-settings.test.ts`
- `editor/test/ui/inline-toggle.test.ts`

Generated:
- `editor/node_modules`, `editor/dist`, `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm` (artifactRoots)

This plan's own file:
- `impl-plans/active/emaki-43-vactr-plugins.md`

## sharedPaths

None. Read-only:
- `editor/src/params/grid.ts` (`CycleSteps`)
- `editor/src/visual/scopes.ts` (`pickFrame`, `drawScope`)
- `editor/src/visual/spectrum.ts` (`masterLevel`)
- `editor/src/visual/meters.ts` (`LevelsTimeline`)
- `editor/src/visual/background.ts:72` (the time rule)
- `editor/src/code/vim-settings.ts` (pattern)
- `editor/src/code/highlight.ts`
- all of `emaki/`

## Tasks

### V1: Glow and the beat-glow plugin

**`decorations.ts`**:
`PLAYING_MARK = { background: [0.7, 0.58, 0.26, 0.5], glow: { color: [0.7, 0.58, 0.26, 0.5], radius: 4 } }`.
`EVAL_MARK` and `EVAL_ERROR_MARK` stay unchanged, with no glow.

**`beat-glow.ts`**:
`createBeatGlowPlugin(src: { playing(): readonly Range[]; flashes(): readonly { from: number; to: number; error: boolean }[] }): EmakiPlugin`.
- Its `attach` registers one `animatedMarks` provider. The provider returns
  `PLAYING_MARK` for each playing range, then the eval mark for each flash,
  in that order.
- `frame` returns `true` while either list is non-empty.

**`mount.ts`**
- Create a `DecorationStore`, a `WidgetSet`,
  `new WidgetLayer(hostEl, layout, ledger)`, and a `PluginRuntime` (from
  `emaki/engine`).
- Add the beat-glow plugin. Its `playing()` closure returns the current
  `playingRanges`. Its `flashes()` closure returns `evalRanges` mapped with
  `error: className?.includes('error')`.
- Remove `playingRanges` and `evalRanges` from the `animatedRows` and
  `animated` arrays at today's `mount.ts:274` and `:291`. Keep the momentary
  rows.
- In `onFrame`, before rendering:
  1. `runtime.viewportChanged(...)`, with lines from
     `layout.heightmap.lineAtHeight`;
  2. `runtime.frame(ctx.frameMs)`;
  3. `const d = store.collect(ctx.frameMs, surface.state.doc)`;
  4. pass `plugin: { marks: d.marks, animatedMarks: d.animatedMarks, lineFormats: d.lineFormats, key: d.key }`
     into both `renderer.render(...)` calls (the text-dirty branch and the
     animation-only branch);
  5. when `widgets.revision` changed, call
     `layout.heightmap.setBlocks(widgets.entries(doc))` and
     `scheduler.invalidateText('view')`.
- After render: `widgetLayer.update(...)`, then
  `scheduler.setActive('plugins', runtimeActive || layerAnimating)`.
  Add `'plugins'` to the reason set if `setActive` requires a known key.
  Read `emaki/src/render/frame.ts` first, and do not edit emaki.
- In the `surface.subscribe` callback (today line 326), on `docChanged` call
  `runtime.docChanged(update)`.
- `dispose` runs `runtime.destroy()` and `widgetLayer.dispose()` before
  `renderer.dispose()`.

**Pitfall.** The glow color must never be drawn into the text layer.
`gpu.test.ts` asserts the playing color is absent from text. EMK-DECOR
guarantees this; do not route glow through `annotations`.

### V2: `inline-settings.ts`

Imitate `editor/src/code/vim-settings.ts`:
- `INLINE_WIDGETS_KEY = 'vactr.inlineWidgets'`;
- `readInlineWidgets(storage)`: `true` unless the stored value is
  `'false'`. Storage errors give `true`;
- `writeInlineWidgets(storage, on)`: writes `'true'` or `'false'`, and
  swallows storage errors.

### V3: Anchors (`inline/anchors.ts`, pure)

`slotAnchors(input: { firstSrc: ReadonlyMap<string, { span: { start: number; end: number }; rev: number }>; mapSpan(span, rev): { from: number; to: number } | null; forms: readonly { from: number; to: number }[]; doc: Text }): Map<number, string[]>`.
- Keys are 0-based anchor lines; values are slot names, sorted.
- **Rule (EM-D13):**
  1. Map each slot's earliest src span.
  2. If it does not map, skip the slot.
  3. If a top-level form range contains the mapped `from`, the anchor is the
     line of `form.to`, or of `form.to - 1` when `form.to` sits at a line
     start. Otherwise it is the line of the mapped `from`.
- Forms are passed already mapped. `mount.ts` maps
  `store.forms(DOC_FILE)` spans with `sync.mapWireSpan(form.span, evalRev)`,
  and drops unmappable or `null`-`evalRev` forms.

`scopeAnchors(doc: Text): number[]` returns the 0-based lines whose text
parses (through V5's helper) to settings that include an `oscilloscope`
visualizer.

### V4: Sequencer (`inline/sequencer.ts`)

`createSequencerPlugin(deps): { plugin: EmakiPlugin; onPlaying(events: readonly WirePlaying[]): void; clear(): void }`, where `deps` is:
- `file: string`
- `beatsPerCycle(): number`: `store.tempo?.beats_per_cycle ?? 4`, the same
  source as `params/mount.ts:153`
- `mapSpan`
- `forms(): readonly { from: number; to: number }[]`
- `cycles(): number | null`: `transport.cycles()`
- `running(): boolean`
- `enabled(): boolean`

**State**
- One `CycleSteps` (import from `../../params/grid`).
- `firstSrc` per slot: the lowest `src.span.start` among events with
  `src.file === file` in the slot's current cycle. Reset it when the slot's
  cycle advances, using the same cycle rule as `CycleSteps.add`.

**`onPlaying`** calls `steps.add(events, beatsPerCycle())`, updates
`firstSrc`, recomputes anchors, and sets widgets. While `enabled()`, call
`host.widgets('seq').set(...)` with one block per anchor line:
- `key 'seq:' + line`, `side 'below'`;
- `height = rows x 14 + 6`;
- canvas content.

`draw`:
- One row per slot. Each step is a flat filled rect at
  `x = pos x width`, `w = max(2, len x width - 1)`.
- The step containing `phase = cycles() - floor(cycles())` is drawn in the
  accent color, the others in a muted color.
- Colors are read once from `--vt-*` tokens: `--vt-accent`,
  `--vt-text-muted`, `--vt-raised`.
- Return `running()`.

**`clear()`** clears `CycleSteps` and the widgets. `mount.ts` calls it
wherever `highlight.clear()` is called today: the `EvalController` and
`TransportBar` `onHush` callbacks.

### V5: Scope (`inline/scope.ts`, `visual/settings.ts` helper)

**`visual/settings.ts`.** Add the exported function
`editorSettingLines(doc: Text): { line: number; settings: EditorSettings }[]`,
where `line` is 1-based. It reuses the existing private `settingLines` and
calls `parseLines([entry])` per line. Errors are ignored. No behavior
change to existing exports.

**Plugin.**
`createScopePlugin(deps: { time(): number; latestOnReceipt(): boolean; enabled(): boolean }): { plugin: EmakiPlugin; onLevels(body: LevelsBody): void; clear(): void }`.
- Keep a `LevelsTimeline` and the latest body with its receipt time,
  mirroring `visual/overlays.ts:40-56`.
- Anchors: recompute in the `docChanged` hook, debounced to at most once per
  250 ms (check `performance.now()` in `frame`).
- Widgets: one block per anchor, `key 'scope:' + line`, height 64, canvas.
- `draw`:
  - pick the body like `overlays.present` (`latestOnReceipt` rule, else
    `timeline.at(time)`);
  - `master = masterLevel(body)`;
  - if `master?.waveform?.length === 256`, call
    `drawScope(ctx, w, h, master.waveform)`; otherwise draw a flat muted
    "awaiting master output" baseline;
  - return `true` only while the last receipt is within 2 s.
- `mount.ts` wiring:
  - `onLevels` is fed from today's `store.subscribe(['levels'])` callback
    (line 373), alongside `transport.onLevels`;
  - `time` is `clock.now()`;
  - `latestOnReceipt` is `tier === 'native' && !deps.audible`, the same
    rule as `visual/background.ts:72`.

### V6: Toggle

**`transport.ts`.** `TransportBar` options gain
`inline?: { enabled: Accessor<boolean>; toggle: () => void }`, passed
through to `TransportView`, the same way as `vim` (`mount.ts:164`).

**`ui/transport-view.tsx`.** In `EditorOptions` (line 279), next to the vim
toggle, add a `vact-editor-option` row with
`IconButton class="vact-inline-toggle"`:
- label `Inline widgets: on/off`;
- `ariaPressed`;
- text `inline`.

**`app/app.css`.** Duplicate the `.vact-vim-toggle` rules (lines 284-290)
for `.vact-inline-toggle`: flat, square, token colors.

**`mount.ts`**
- `const [inlineOn, setInlineOn] = createSignal(readInlineWidgets(localStore(win)))`.
- `toggle` flips the signal, calls `writeInlineWidgets`, and either clears
  both plugins' widgets (off) or recomputes anchors (on).
- The plugins check `enabled()` before setting widgets.

**`code.css`.** Add `.emaki-widget` canvas styling, if any: `display: block`
and no border radius.

### V7: Tests (`input -> expected`)

**`beat-glow.test.ts`**
- playing `[{0,3}]` plus flash `{4,9, error: true}` -> the provider returns
  `[{0,3,PLAYING_MARK}, {4,9,EVAL_ERROR_MARK}]`;
- both empty -> `frame` returns `false`.

**`inline-anchors.test.ts`**
- a slot span inside a 3-line form -> anchored at the form's last line;
- two slots in the same form -> one entry with both slots, sorted;
- a span outside every form -> anchored at its own line;
- an unmappable span -> the slot is absent;
- a `form.to` exactly at a line start -> the previous line;
- `scopeAnchors` with `#@ editor {"visualizers":[{"kind":"oscilloscope"}]}`
  on line 4 -> `[3]`; a `spectrum`-only line -> `[]`.

**`inline-sequencer.test.ts`** (fake host from a recording `PluginHost`
stub)
- `onPlaying` with 4 events in cycle 0 with src -> one block widget,
  height `1 x 14 + 6`;
- `draw` with `cycles 0.3` and steps at 0, 0.25, 0.5, 0.75 -> the second
  step uses the accent color (canvas recording mock);
- `clear` -> widgets set to empty;
- `enabled false` -> no widgets set;
- no events have `src` -> no widget.

**`inline-scope.test.ts`**
- `onLevels` with a 256-sample waveform -> `draw` calls `drawScope` (spy
  via a module mock) and returns `true`;
- no levels for 3 s (fake `performance.now`) -> `draw` returns `false`;
- a non-oscilloscope directive -> no widget.

**`inline-settings.test.ts`**
- missing key -> `true`;
- `'false'` -> `false`;
- a throwing storage -> `true`, and write does not throw.

**`ui/inline-toggle.test.ts`** (imitate `editor/test/ui/vim-toggle.test.ts`)
- the button renders with `aria-pressed='true'` by default;
- a click calls `toggle`;
- the label switches to off.

## Invariants

- Every existing vactr test passes unchanged. In particular `gpu.test.ts`
  draw-order and text-layer assertions hold with the glow.
- With the toggle off, nothing changes geometry: the heightmap has no
  blocks.
- The step grid and background overlays behave as before.
- The e2e suites pass with the default on (EQ2). If an e2e check fails
  because of widget geometry, fix the coordinate path in vactr code. If the
  fix needs emaki, stop and report a scope amendment. Never edit the e2e
  scripts.

## Verification

Logs go to `tmp/emaki/EMK-VAC-PLUGINS/`. Heavy steps take the lock.

1. `cd editor && npm run check`: exit 0.
2. `cd editor && ./node_modules/.bin/vitest run test/code/beat-glow.test.ts test/code/inline-anchors.test.ts test/code/inline-sequencer.test.ts test/code/inline-scope.test.ts test/code/inline-settings.test.ts test/ui/inline-toggle.test.ts test/canvas/gpu.test.ts test/canvas/mount.test.ts test/visual`:
   exit 0.
3. Wasm lib build (rule 8, first command), then full vitest under the lock:
   `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run`.
   Expect exit 0, failures 0, and testsRun of at least the
   EMK-VAC-SWITCH final count plus the new tests.
4. Under the lock:
   - `mise run build-wasm-release`
   - `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`
   - `cd editor && npm run test:style` (16/16)
   - `cd editor && npm run e2e -- --browser all --profile behavior --out ../tmp/emaki/EMK-VAC-PLUGINS/e2e`
     (failureCount 0, both browsers)
   - `cd editor && npm run e2e:examples` (8/8)
5. `wc -l` on every written file: each under 1000. `mount.ts` must stay
   under 1000; if it would exceed, move the plugin wiring into
   `editor/src/code/inline/wiring.ts`, added under this plan's writePaths by
   the reviewer amendment rule. Report it first.

**mutationEvidence**
- Drop the glow from `PLAYING_MARK` -> no existing test fails, so record the
  coverage note and run the beat-glow unit test, which asserts the glow
  field, and see it fail.
- Lit-step off-by-one -> the sequencer test fails.

Restore afterwards.

## Completion Criteria

- [ ] V1-V7 delivered. `mount.ts` no longer passes `kind: 'playing'` or
  `kind: 'eval'` rows (check with
  `grep -n "kind: 'playing'\|kind: 'eval'" editor/src/code/mount.ts` ->
  none).
- [ ] Verification steps 1-5 pass, with e2e in both browsers. Mutation
  evidence is recorded. Hashes are logged.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
