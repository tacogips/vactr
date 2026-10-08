# LP-EDITOR-MOMENTARY: Right-drag and two-finger momentary tweak, live value label, glide setting

**Status**: Ready (after LP-CONTRACT)
**Plan ID**: LP-EDITOR-MOMENTARY (wave 2; parallel with LP-ENGINE, LP-SESSION-STOP, LP-SESSION-MOMENTARY, LP-EDITOR-STOP)
**Design Reference**: `design-docs/specs/design-live-performance.md` 5.1-5.4, 5.8, 5.9, 8.2, D6, D7, D11
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Created**: 2026-10-08
**Last Updated**: 2026-10-08

## Intent and Context

The user wants to right-drag a bound number in the canvas code editor. The
value changes live, DJ-effect style, and the document text never changes.

- On release the value glides back over a glide time set in the controls
  pane (default 1 s). With Shift held at release it snaps back.
- On iPad a two-finger drag does the same, and several tweaks can run at
  once.
- The binding overlay shows the live value.
- The browser context menu is suppressed only when the press starts on a
  bound site.

LP-CONTRACT added
`client.momentary(file, id, formGen, target: number | null, rampMs)`. It
rate-limits drag updates to 16 ms per site and sends a release at once.
The session (LP-SESSION-MOMENTARY) does the audio-side work. The editor
only sends targets and draws a display copy of each ramp. It never writes
audio values once per frame.

Repository facts:

- **Left-drag:** `DragController` (editor/src/bind/drag.ts) has `siteAt`,
  `DRAG_PX = 200`, `DRAG_SLOP_PX = 3`, `valueAt` scaling with
  `toUnit`/`fromUnit`/`paramMeta`/`isIntegerLiteral`/`round6`
  (editor/src/bind/write.ts), and `attach(surface)` through
  `surface.registerNumericDrag`.
- **`PointerController`** (editor/src/code/pointer.ts):
  - returns early for `button !== 0` and non-primary pointers (:57);
  - has one gesture at a time;
  - handles touch long-press (500 ms) and the 8 px scroll threshold.
- **Animated rows** in code/mount.ts (about :228-250) are
  `playingRanges + evalRanges`. The renderer draws only the `playing` and
  `eval` kinds per frame. `binding` labels are static annotations, and
  every change increments `annotationsRevision`
  (editor/src/code/renderer.ts:214-270).
- **The frame scheduler** has `setActive(owner, active)` and `request()`
  (editor/src/code/frame.ts:136-141).
- **`BindArea`** (editor/src/bind/mount.ts):
  - owns the `SiteTable`, the `SiteWriter`, the `notice(m)` path and
    `changed(ids)`;
  - creates the controls pane with `buildLayout(root).right`;
  - routes `stale-binding` replies to `writer.onStale`.
- **`WireSite.tier`** is `'direct' | 'reeval' | 'manual'`.
  `entry.site.value` is the base (slot) value, because the momentary never
  writes the slot.

## Non-goals

- No change to left-drag behavior or to `DragController`'s write path.
- No protocol change (contract-owned).
- No transport or shortcut work (LP-EDITOR-STOP).
- No per-frame DOM writes for the live value. The label is GPU-drawn
  through animated rows.
- No slider-panel live value display. The design requires only the overlay
  label.

## Dependencies

- **dependsOn**: LP-CONTRACT.
- **Blocks**: LP-EVIDENCE.

## writePaths

- `editor/src/bind/momentary.ts` (new)
- `editor/src/bind/glide-setting.ts` (new)
- `editor/src/bind/mount.ts`
- `editor/src/bind/bind.css`
- `editor/src/code/momentary-pointer.ts` (new)
- `editor/src/code/pointer.ts`
- `editor/src/code/surface.ts`
- `editor/src/app/apis.ts`
- `editor/src/code/mount.ts`
- `editor/src/code/renderer.ts`
- `editor/test/bind/momentary.test.ts` (new)
- `editor/test/bind/glide-setting.test.ts` (new)
- `editor/test/canvas/momentary-pointer.test.ts` (new)
- `editor/test/canvas/gpu.test.ts` (new rows only)
- `editor/test/canvas/mount.test.ts` (new rows only)
- `impl-plans/active/live-perf-editor-momentary.md` (Progress Log only)
- Artifact roots (also in the manifest's `artifactRoots`): `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`,
  `tmp/live-perf/editor-momentary`

## sharedPaths

None. Read-only:

- `editor/src/bind/drag.ts`, `editor/src/bind/write.ts`,
  `editor/src/bind/sites.ts`
- `editor/src/code/frame.ts`
- `editor/src/protocol/client.ts`, `editor/src/protocol/types.ts`
- `editor/src/app/theme.css` (tokens)

## Pinned Interfaces (this plan owns both sides)

**`editor/src/app/apis.ts`:**

- `CodeAnnotation.kind` adds `'momentary'`.
- `CodeSurface` adds:
  - `registerMomentaryDrag(provider: MomentaryProvider): () => void`
  - `momentaryHit(pos: number): boolean`
  - `momentaryDrag(start: { pos: number; clientY: number; kind: 'mouse' | 'touch' }): MomentaryGesture | null`
  - `registerAnimated(owner: string, rows: (frameMs: number) => readonly CodeAnnotation[]): { wake(): void; dispose(): void }`
- With:
  - `MomentaryGesture { move(clientY: number): void; end(snap: boolean): void }`
  - `MomentaryProvider { hit(pos: number): boolean; begin(start): MomentaryGesture | null }`

**`editor/src/bind/momentary.ts`:**

- `export const DRAG_SMOOTH_MS = 30, MIN_SNAP_MS = 0, MAX_GESTURES = 16, CONTEXT_WINDOW_MS = 500, TWO_FINGER_WINDOW_MS = 250;`
- `export class MomentaryController`, with
  `constructor(host: { table: SiteTable; editors(): readonly EditorDecl[] | undefined; send(siteId: number, formGen: number, target: number | null, rampMs: number): void; glideMs(): number; now(): number; notice(m: string): void })`.

**`editor/src/bind/glide-setting.ts`:**

- `export const GLIDE_KEY = 'vactr.momentary.glideMs', GLIDE_DEFAULT_MS = 1000, GLIDE_MAX_MS = 10000, GLIDE_STEP_MS = 50;`
- `export function readGlide(storage?: Storage): number`
- `export function writeGlide(ms: number, storage?: Storage): number` (clamp, then round to the step; returns the stored value)
- `export function mountGlideSetting(parent: HTMLElement, onChange: (ms: number) => void): { dispose(): void }`

## Tasks

### TASK-U1: `MomentaryController` (`editor/src/bind/momentary.ts`)

**Gesture map.** Keyed by gesture key (`mouse:<pointerId>` or
`touch:<a>+<b>`), bounded to `MAX_GESTURES`. Each gesture holds
`{ bindingId, y0, moved, unitOffset?: number, valueOffset?: number, lastTarget }`.

**`begin(start)`:**

- Find the site with the same innermost bound-site rule as
  `DragController.siteAt`. Copy the logic or extract a shared pure helper
  inside momentary.ts; do **not** edit drag.ts.
- Require `entry.state === 'bound'` and `entry.site.tier === 'direct'`.
  Otherwise call
  `notice('momentary tweak needs a live site (re-evaluate or use a direct literal)')`
  and return null.
- Return null at the `MAX_GESTURES` cap.

**`move(clientY)`:**

- `dy = y0 - clientY`. Nothing is sent before `|dy| >= DRAG_SLOP_PX`.
- With ParamMeta: `unitOffset = dy / DRAG_PX` and
  `target = fromUnit(clamp01(toUnit(base) + unitOffset), meta)`.
- Without ParamMeta: `valueOffset = dy * 0.01 * max(|base at begin|, 1)`
  and `target = base + valueOffset`.
- Round integer literals with `Math.round`. Pass others through `round6`.
- `base` is **always the current** `entry.site.value`, looked up by
  `bindingId` at call time.
- Send `send(site.id, site.form_gen, target, DRAG_SMOOTH_MS)`.

**`end(snap)`:**

- If the gesture moved, send
  `send(site.id, site.form_gen, null, snap ? 0 : glideMs())`.
- Start the display ramp from the last target to `base` over `glideMs()`
  (or 0 for a snap), and remove the gesture.
- If it never moved, send nothing.

**`refresh()`** is called after every site-table change. For each held
gesture whose site id, form_gen or base changed, re-send the re-based
target from the stored offset. This is how a tweak follows a
re-evaluation.

**`rows(frameMs)`** returns, for each held gesture and each still-gliding
display ramp, one annotation
`{ from: r.to, to: r.to, kind: 'momentary', label: ` ~ ${format}` }`:

- `r` is `table.currentRange(entry)`;
- the value is quantized to the ParamMeta step, or else 3 significant
  digits;
- finished glides are dropped.

It returns `[]` when nothing is active.

### TASK-U2: Glide setting (`editor/src/bind/glide-setting.ts`, `bind.css`)

**Control.** A labelled `<input type="number">` plus an
`<input type="range">`:

- min 0, max 10000, step 50;
- label text: "momentary glide (ms)".

**Persistence.** Persist with `writeGlide`. All storage access is wrapped
in try/catch, like the existing `vactr.*` keys in editor/src/ui/shell.tsx.

**CSS.** Add `.vact-glide-setting` rules in bind.css:

- existing `--vt-*` tokens only;
- `border-radius: 0`;
- spacing at least 8px between controls;
- a 36px target height under `pointer: coarse`, following
  design-ui-style.md.

### TASK-U3: Pointer routing (`editor/src/code/pointer.ts`, new `editor/src/code/momentary-pointer.ts`)

Keep pointer.ts small by putting the pairing and right-button state machine
in `momentary-pointer.ts`, as `class MomentaryPointer`. It holds its own
`Map<pointerId, …>`; it never shares `PointerController.gesture`.

**Mouse.**

- On `pointerdown` with `pointerType === 'mouse'` and `button === 2`,
  before the existing `button !== 0` return:
  - compute `pos` with `surface.posAtCoords`;
  - call `surface.momentaryDrag({ pos, clientY, kind: 'mouse' })`;
  - if it returns a gesture: capture the pointer, call `preventDefault()`,
    and route moves and ups for that `pointerId` to it;
  - `end(event.shiftKey)` runs on `pointerup`; `pointercancel` and
    `lostpointercapture` run `end(false)`.
- A chorded right press during a left drag arrives as `pointermove`, not
  `pointerdown`, so it starts nothing (D11). Add a test.

**Touch.**

- Every touch `pointerdown` on the canvas, primary or not, is recorded as
  pending `{ id, x, y, t, pos }`.
- **Pairing:** a new touch down pairs with the nearest pending unpaired
  touch when all of these hold:
  - it went down within `TWO_FINGER_WINDOW_MS`;
  - it moved less than 8 px;
  - `surface.momentaryHit(pending.pos)`.
- **On pairing:**
  - call `surface.momentaryDrag({ pos: pending.pos, clientY: centroidY, kind: 'touch' })`;
  - if the primary finger is part of the pair, call
    `PointerController.cancel()` for the existing gesture (no tap caret,
    no long-press selection), through a hook that pointer.ts exposes to
    `MomentaryPointer`;
  - capture both pointers.
- **During the pair:** moves update the centroid's `clientY`.
- **End:** lifting either finger calls `end(event.shiftKey)` and ends the
  pair.
- An unpaired, non-primary touch does nothing else, as today.

**Context menu.** Add a `contextmenu` listener on the canvas element. Call
`preventDefault()` only when `surface.momentaryHit(posAtCoords(event))` is
true, or when a mouse momentary gesture is active or ended less than
`CONTEXT_WINDOW_MS` ago. Otherwise leave the event alone.

### TASK-U4: Surface and animated rows (`editor/src/code/surface.ts`, `apis.ts`, `editor/src/code/mount.ts`)

- **`surface.ts`** implements the apis.ts additions:
  - a provider registry, imitating `registerNumericDrag`/`numericDrag`;
  - an animated-source registry. `wake()` calls a mount-supplied callback;
    the surface stores one listener set by mount through a new
    `onAnimatedWake(cb)`.
- **`code/mount.ts`:**
  - on wake, `scheduler.setActive('momentary', true); scheduler.request()`;
  - in **both** render branches, append `rows(ctx.frameMs)` from every
    animated source to the `animated` / `animatedRows` array;
  - when all sources return empty, `scheduler.setActive('momentary', false)`.
  - Never add these rows to `staticRows`, and never increment
    `annotationsRevision` for them.

### TASK-U5: Renderer label (`editor/src/code/renderer.ts`)

Draw `kind: 'momentary'` animated annotations with a label: the same quad
plus `drawRun(this.labelRun(...))` as the static `binding` label
(renderer.ts:263-268), but on every frame.

**Layer pitfall.** The `overlay` and `overlayText` writers are cleared only
when `overlayDirty`. Do not just append, or labels would pile up across
frames. Instead:

- record `overlayStaticCount` and `overlayTextStaticCount` after building
  the static overlay;
- on every frame, `setCount(static)`, then append the momentary label
  quads and runs.

This mirrors `backgroundStaticCount` (renderer.ts:228/247). A frame with
no momentary rows must leave the counts and the upload work identical to
today.

### TASK-U6: Wiring (`editor/src/bind/mount.ts`)

In `BindArea`:

- create `MomentaryController` with
  `send = (id, fg, t, ms) => client.momentary(this.file, id, fg, t, ms)`,
  `glideMs = () => this.glide` and `notice = (m) => this.notice(m)`;
- register it with `surface.registerMomentaryDrag`;
- register `surface.registerAnimated('momentary', (f) => ctl.rows(f))`,
  and call `wake()` from `begin` and `end`;
- call `ctl.refresh()` after `table.applySites`/`applyEval`, in the
  existing `changed` path;
- on `stale-binding` with reason `momentary-ineligible` or
  `momentary-capacity`, call `notice(...)`;
- mount the glide setting at the top of the controls pane, using the
  `buildLayout(root).right` container that `SliderPanel` already uses, and
  dispose all of it in `dispose()`.

## Key Points a Careless Implementation Gets Wrong

- **Never call `writeSite`, `setTweak` or `surface.dispatch`** from the
  momentary path. The document revision must stay unchanged during and
  after a tweak.
- **Read the base live, never cached at begin.** Otherwise re-evaluation
  would not re-base the tweak.
- **Do not suppress the context menu globally.** Only on a site hit or
  within the post-gesture window.
- **Touch pairing must not break existing behavior:** single-finger tap,
  long-press selection, handle drag and native scroll. The canvas already
  has `touch-action: none`.
- **The animated label must not mark the static layers dirty.** Assert
  with counters: no `annotationsRevision` increment and no text rebuild
  (`stats`).
- **Bound everything:** gestures at 16, display ramps at 16, and the label
  string quantized.
- `code/mount.ts` is 382 lines and `renderer.ts` 668. Stay under 1000.

## Tests to Add (input -> expected)

**`editor/test/bind/momentary.test.ts`** (fake `now`, fake `send`):

- A Direct site with ParamMeta `lpf.cutoff`, range [20, 20000] on a log
  curve: a move of `dy = 100` sends
  `fromUnit(toUnit(800) + 0.5)` with `rampMs = 30`; a 2 px move sends
  nothing.
- Without ParamMeta (base 2): `dy = 50` sends `2 + 50 * 0.01 * 2 = 3`.
- An integer literal sends integral targets.
- Release without Shift sends `(null, 1000)` (default glide). With
  `glideMs = 250` it sends `(null, 250)`. A Shift release sends
  `(null, 0)`.
- A gesture that never moved sends nothing.
- A Reeval or Manual site, or an unbound site, returns null, calls
  `notice` once and sends nothing.
- Re-base: during a held gesture, change the table entry's
  `site.value` from 800 to 600 and its `site.id`/`form_gen`, then call
  `refresh()`. The controller sends a new target computed from 600 with
  the new id.
- `rows()` while held gives one `momentary` row whose label shows the
  quantized target. During a 1000 ms glide at `now + 500` the label is
  between the target and the base. After 1000 ms, `rows()` returns `[]`.
- The 17th concurrent gesture returns null.

**`editor/test/bind/glide-setting.test.ts`:**

- `readGlide` with empty storage gives 1000;
- `writeGlide(12345)` gives 10000;
- `writeGlide(333)` gives 350;
- a storage that throws gives the default with no throw;
- the mounted input change calls `onChange` with the clamped value;
- `getComputedStyle(...).borderRadius` is `'0px'` where jsdom computes it,
  otherwise the CSS rule text contains `border-radius: 0`.

**`editor/test/canvas/momentary-pointer.test.ts`** (synthetic
`PointerEvent`s on a test surface with a fake provider):

- A right-button mouse down on a hit starts a gesture: moves call
  `move(clientY)`, and a Shift up calls `end(true)`.
- A right down on a non-hit does nothing.
- A `contextmenu` on a hit is `defaultPrevented`.
- A `contextmenu` elsewhere is not prevented.
- A `contextmenu` 100 ms after a gesture ended is prevented, and 600 ms
  after it is not (fake timers or a fake clock).
- A left-drag in progress, followed by a `pointermove` with `buttons = 3`,
  starts no momentary gesture.
- Touch A down on a hit, then touch B down 100 ms later within 8 px: a pair
  forms, moves use the centroid Y, and lifting B ends the tweak. No caret
  is placed and no long-press selection happens.
- Touch B down 300 ms after A: no pair forms, and the tap behavior is
  unchanged.
- Two pairs (A+B on site 1, C+D on site 2) produce two independent
  gestures.

**`editor/test/canvas/gpu.test.ts`** (new rows, with the counting fakes):

- One `momentary` animated annotation draws a label.
- Two consecutive frames with different labels do not accumulate quads:
  the overlay count is constant.
- A frame with no momentary rows has the same upload counts as the
  baseline.

**`editor/test/canvas/mount.test.ts`** (new rows):

- A registered animated source with one row: over 5 frames,
  `annotationsRevision` does not change and the text-build counter stays
  0.
- An empty source deactivates the `momentary` owner.

## Verification (exact commands; all must exit 0)

1. `cd editor && npm run check`
2. `cd editor && ./node_modules/.bin/vitest run test/bind test/canvas/momentary-pointer.test.ts test/canvas/gpu.test.ts test/canvas/mount.test.ts`
   passes with testsRun > 0.
3. `cd editor && ./node_modules/.bin/vitest run test/canvas test/bind test/code`
   (left-drag, pointer, IME and renderer regressions unchanged).

`npm run test:style` and `test:perf` are run serially in LP-EVIDENCE.

Write logs to `tmp/live-perf/editor-momentary/*.log`.

## Completion Criteria

- [ ] TASK-U1 to TASK-U6 are done, and all listed tests pass.
- [ ] Existing drag, pointer, input, mount and gpu tests pass unchanged.
- [ ] No momentary code path calls `writeSite`, `setTweak` or `dispatch`
  (`grep -n "writeSite\|setTweak\|dispatch" editor/src/bind/momentary.ts`
  shows none).
- [ ] Verification 1-3 exit 0. The Progress Log is updated.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.
