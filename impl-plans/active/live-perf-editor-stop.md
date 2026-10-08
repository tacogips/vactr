# LP-EDITOR-STOP: Stop and cut shortcuts, toolbar labels, output state display

**Status**: Ready (after LP-CONTRACT)
**Plan ID**: LP-EDITOR-STOP (wave 2; parallel with LP-ENGINE, LP-SESSION-STOP, LP-SESSION-MOMENTARY, LP-EDITOR-MOMENTARY)
**Design Reference**: `design-docs/specs/design-live-performance.md` 4.1, 4.5, 6, 8.2 (shortcuts, toolbar), D1, D2, D10
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Created**: 2026-10-08
**Last Updated**: 2026-10-08

## Intent and Context

The user wants:

- the stop button and `Mod-.` to stop gently;
- `Mod-Shift-.` and the hush button to cut at once;
- `Mod` to mean Meta on Mac and iPad, and Ctrl elsewhere.

Today:

- `Mod-.` is a code-keymap binding that sends `hush`
  (editor/src/code/eval.ts:90). It fires only while the code textarea has
  focus.
- The stop-every-slot button sends one `stop {slot}` per slot the editor
  has seen (editor/src/code/transport.ts:187).

LP-CONTRACT added `client.stopAll()` (it sends `stop-all {}`) and
`TransportSample.output?: 'running' | 'draining' | 'cutting' | 'idle'`.

## Non-goals

- No momentary gesture work (LP-EDITOR-MOMENTARY).
- No changes to `editor/src/code/mount.ts`, `pointer.ts`, `surface.ts`,
  `renderer.ts`, `editor/src/bind/*` or `editor/src/app/apis.ts`. Those
  belong to LP-EDITOR-MOMENTARY.
- No protocol changes (contract-owned).
- No new icon, and no change to the `.vact-hush`/`.vact-stop-all` class
  names. The e2e `hushQuiet` check depends on `.vact-hush`.

## Dependencies

- **dependsOn**: LP-CONTRACT.
- **Blocks**: LP-EVIDENCE.

## writePaths

- `editor/src/ui/stop-keys.ts` (new)
- `editor/src/code/transport.ts`
- `editor/src/ui/transport-view.tsx`
- `editor/src/code/eval.ts`
- `editor/src/code/keyboard.ts`
- `editor/src/app/app.css` (one `[data-output]` rule only)
- `editor/test/ui/stop-keys.test.ts` (new)
- `editor/test/code/transport.test.ts`
- `editor/test/code/eval.test.ts`
- `editor/test/ui/controls.test.ts`
- `editor/test/canvas/input.test.ts` (the eval/hush keyboard row only)
- `impl-plans/active/live-perf-editor-stop.md` (Progress Log only)
- Artifact roots (also in the manifest's `artifactRoots`): `target`,
  `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`,
  `tmp/live-perf/editor-stop`

## sharedPaths

None. Read-only:

- `editor/src/code/surface.ts:156-174` (`matchesKey`, the Apple rule
  `/Mac|iP/.test(navigator.platform)`)
- `editor/src/ui/shell.tsx:135-142` (the window-level `Mod-\` handler
  pattern)
- `editor/src/protocol/client.ts` (`stopAll`, `hush`)

## Tasks

### TASK-T1: Pure shortcut matcher (`editor/src/ui/stop-keys.ts`)

Export
`stopShortcut(event: Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>, apple: boolean): 'stop-all' | 'cut' | null`,
and also `isApplePlatform(platform = navigator.platform): boolean`.

**Matching rules:**

- **Period key:** `event.code === 'Period'`. Fallback when `code` is empty:
  `key === '.'`, or `key === '>'` with Shift.
- **Mod:** exactly `metaKey && !ctrlKey` when apple, and
  `ctrlKey && !metaKey` otherwise.
- **Alt:** must be false.
- **Result:** Shift gives `'cut'`, no Shift gives `'stop-all'`, and
  anything else gives `null`.

### TASK-T2: App-level handler and stop-all (`editor/src/code/transport.ts`)

**`TransportOptions` changes:**

- add `shortcuts?: boolean` (default true);
- add `platform?: string` (tests only).

**The handler.** In the constructor, when `shortcuts !== false`, add one
`keydown` listener on `parent.ownerDocument.defaultView`, in the capture
phase:

- When `stopShortcut(...)` returns a value:
  - call `event.preventDefault()` and `event.stopPropagation()`;
  - call `this.stopAll()` or `this.hush()`.
- Ignore `event.repeat`, so a held key sends one message.
- Remove the listener in `dispose()`.

**`stopAll()`** becomes `this.opts.client.stopAll(); this.opts.onHush?.();`.
That is one `stop-all` message, not one `stop` per slot, plus a highlight
clear. `hush()` is unchanged.

**Output state.**

- In `tick()`, read `this.opts.sample?.()?.output ?? 'running'` into a
  signal `output`. Only set the signal when the value changes, so no DOM
  write happens per frame.
- Expose `outputHistory(): readonly string[]` holding the distinct values
  observed, in order, bounded to the last 16. The evidence harness uses it.

### TASK-T3: Toolbar (`editor/src/ui/transport-view.tsx`, `app.css`)

**Labels.** Each label is used for both `aria-label` and `title`:

- `.vact-stop-all`: `"stop: stop every slot, let effects ring out (Mod-.)"`.
  While `output() === 'draining'` it becomes
  `"stop: stop every slot, let effects ring out (Mod-.) - tails ringing out"`.
- `.vact-hush`: `"cut: silence everything now and clear effects (Mod-Shift-.)"`.
- per-slot `.vact-mute`: `` `stop ${slot.name}: release its notes` ``.

**`data-output`.** `TransportViewProps` gains
`output: Accessor<'running' | 'draining' | 'cutting' | 'idle'>`, rendered
as `data-output={props.output()}` on the stop button.

**CSS.** `editor/src/app/app.css` gets one rule:
`.vact-stop-all[data-output='draining'] { color: var(--vt-accent); }`.

- `--vt-accent` already exists in editor/src/app/theme.css:14. Do not add a
  token.
- No border radius.
- No other style change.

### TASK-T4: Remove the code-keymap stop (`editor/src/code/eval.ts`, `editor/src/code/keyboard.ts`)

- **`eval.ts`:**
  - delete the `{ key: 'Mod-.', ... }` binding;
  - keep the `hush()` method and the `onHush` option, since other callers
    may use them;
  - update the header comment.
- **`keyboard.ts`:**
  - replace the fallback option `hush?` with
    `stopAll?: () => void; cut?: () => void`, mapped through
    `stopShortcut` with the same platform rule;
  - update the header comment.

  This fallback is not wired in production (code/mount.ts does not pass
  it), so the app-level handler stays the single owner.

## Key Points a Careless Implementation Gets Wrong

- **Shift changes the key.** Shift+`.` produces `key === '>'` on US
  layouts, and `matchesKey` with `'Mod-Shift-.'` does not match. Match on
  `event.code === 'Period'`.
- **No double sends.** A shortcut pressed while the code textarea has focus
  must send exactly one message. The code keymap must no longer bind
  `Mod-.`.
- **Platform.** On Mac, `Ctrl+.` must not trigger. On Windows and Linux,
  `Meta+.` must not trigger.
- **Bounded reactivity.** Setting `data-output` happens only on change.
  The tick runs every frame.
- **Do not change** `controls.test.ts`'s check that the toolbar text has no
  words. Labels live in attributes only.

## Tests to Add or Update (input -> expected)

**`editor/test/ui/stop-keys.test.ts`:**

- apple, `{code:'Period', metaKey:true}` gives `stop-all`;
- apple, `{code:'Period', metaKey:true, shiftKey:true, key:'>'}` gives
  `cut`;
- apple, `{code:'Period', ctrlKey:true}` gives `null`;
- non-apple, `{code:'Period', ctrlKey:true}` gives `stop-all`;
- non-apple, Ctrl+Shift gives `cut`;
- non-apple, Meta gives `null`;
- Alt held gives `null`;
- `{code:'', key:'.'}` with Mod gives `stop-all`;
- `{code:'KeyA'}` gives `null`.

**`editor/test/code/transport.test.ts`:**

- A window keydown (Ctrl+`.`, `platform: 'Linux'`), dispatched while
  focus is on a non-code element, gives exactly one `stop-all` envelope
  and one `onHush` call.
- Ctrl+Shift+`.` gives exactly one `hush`.
- `repeat: true` gives no message.
- After `dispose()`, no message is sent.
- With `platform: 'MacIntel'`, Meta works and Ctrl does not.
- Clicking `.vact-stop-all` sends one `stop-all {}`. With two known slots,
  `transport.of('stop')` is empty.
- With `sample()` returning `output: 'draining'`, the stop button gets
  `data-output="draining"` and the draining tooltip after `tick()`.
  `outputHistory()` equals `['draining']`, and then
  `['draining', 'idle']` after the sample changes.
- The existing hush and per-slot stop rows still pass, with the per-slot
  label updated.

**`editor/test/ui/controls.test.ts`:** the stop-all click expectation
changes from `transport.of('stop')` to `transport.of('stop-all')` bodies
`[{}]`. Every other assertion is unchanged.

**`editor/test/code/eval.test.ts`:** the row "Mod-. sends hush" becomes
"Mod-. is not a code keymap binding (handled app-level)". The key returns
false and no envelope is sent. Mod-Enter and Mod-Shift-Enter rows are
unchanged.

**`editor/test/canvas/input.test.ts`:** the "dispatches eval/hush" row uses
the new `stopAll`/`cut` options. It asserts that Meta+`.` calls `stopAll`
once and Meta+Shift+`.` (`code: 'Period'`) calls `cut` once. The eval
assertions and composition behavior are unchanged.

## Verification (exact commands; all must exit 0)

1. `cd editor && npm run check`
2. `cd editor && ./node_modules/.bin/vitest run test/ui test/code/transport.test.ts test/code/eval.test.ts test/canvas/input.test.ts`
   passes with testsRun > 0.
3. `cd editor && ./node_modules/.bin/vitest run test/code test/ui test/protocol`

`npm run test:style` needs a browser and the measurement lock, so it is
run in LP-EVIDENCE, not here.

Write logs to `tmp/live-perf/editor-stop/*.log`.

## Completion Criteria

- [ ] TASK-T1 to TASK-T4 are done, and all listed tests pass.
- [ ] `grep -n "Mod-\\." editor/src/code/eval.ts` shows no keymap binding.
- [ ] Toolbar labels match the design section 6 table exactly.
- [ ] Verification 1-3 exit 0. The Progress Log is updated.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.
