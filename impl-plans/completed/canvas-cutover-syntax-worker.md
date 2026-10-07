# Canvas Cutover SYNTAX-WORKER: Tree-sitter Parsing in a Worker, Post-Frame Fallback Implementation Plan

**Status**: Completed (2026-10-07)
**Plan ID**: CANVAS-SYNTAX-WORKER (session 303, wave 17; runs alone; first of the serial session-303 plans)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.16 part A (syntax Worker, Escalation S), part F (order and ownership); 15.3.8.14 section 3 rule 1 and section 7 (as amended); 15.3.8.7 allowed whole-text transfers (as amended)
**Manifest**: impl-plans/completed/canvas-cutover-dispatch.json (entry `CANVAS-SYNTAX-WORKER`)
**Created**: 2026-10-07
**Last Updated**: 2026-10-07

---

## Intent and Context

The canonical `run-001` fails only WebKit input p95 (51-59 ms against 50 ms) and WebKit non-stall
sync p99 (58 ms against 50 ms). The operator diagnosis is
`tmp/canvas-cutover/diag-webkit-input/REPORT.md` (gitignored, read-only). It shows that the
deferred tree-sitter reparse, `SyntaxSpans.schedule` → `setTimeout(0)` at
`editor/src/code/syntax.ts:157-160`, runs between a keystroke and its frame in 102 of 105 slow
keys. Incremental `parseDoc` takes 26 ms at p50 and 45 ms at p95.

This plan moves production parsing into a module Worker, so the main thread never parses after a
keystroke. When there is no Worker, the main-thread fallback reparse starts only after the next
presented frame.

Repository context:

- `editor/src/code/syntax.ts`: `SpanProvider`, `FallbackSpans`, `SyntaxSpans`, with the line cache
  and `remapLines`, and `stats { syncParses, deferredParses, captures, capturedLines }`.
- `editor/src/code/syntax-core.ts`: `treeEditFromDocs`, `CAPTURE_CLASSES`, `loadVactSyntax(base)`,
  `createVactSyntax(parser, query)`, `styleSpans`.
- `editor/src/code/mount.ts:98,282,322-325`: the provider is `FallbackSpans` until
  `deps.syntax()` resolves. `noteChanges` is called from the surface subscription.
  `codePane.dataset.syntax` is `'fallback' | 'tree-sitter'`.
- `editor/src/code/frame.ts`: `FrameScheduler` (rAF tick, `invalidateText`, hidden handling).
- `editor/src/app/main.ts:127`: `deps.syntax = () => loadVactSyntax(win.document.baseURI)`.
  `reportSelfCheck` is at `main.ts:142-170`.
- `editor/vite.config.ts`: the `vactr-assets` plugin serves `tree-sitter-vact.wasm` and
  `web-tree-sitter.wasm` at the site root. The Vite default `worker.format` is IIFE.
- The Tauri CSP (`editor/src-tauri/tauri.conf.json:22`) already allows `worker-src 'self' blob:`.

## Non-goals

- No diagnostics change. CANVAS-DIAG-OFFPATH owns that and uses the `afterPresent` hook added
  here.
- No telemetry change (CANVAS-TELEMETRY-LEAD).
- No change to these:
  - the 16,384-span cap;
  - `syntax-truncated`;
  - `CAPTURE_CLASSES`;
  - `highlights.scm`;
  - the grammar;
  - the renderer;
  - the layout;
  - the `CodeSurface` and `CodeApi` contracts.
- No new dependency. The worker uses `web-tree-sitter` and `@codemirror/state` `Text`, both
  already present. No `SharedArrayBuffer`. No `requestIdleCallback`.
- No Rust, no Session Protocol change, no threshold or workload change.
- Do not delete the main-thread `SyntaxSpans`. It stays as the fallback mode.
- Do not touch `editor/src-tauri/tauri.conf.json`, because the CSP already allows workers.

## Ownership

writePaths (concrete files; no directories except the artifact roots):

- `editor/src/code/syntax.ts`
- `editor/src/code/syntax-core.ts` (edit only if a pure helper must be exported for the worker
  core; otherwise unchanged)
- `editor/src/code/syntax-worker.ts` (new: worker entry)
- `editor/src/code/syntax-worker-core.ts` (new: side-effect-free core and message types)
- `editor/src/code/frame.ts`
- `editor/src/code/mount.ts`
- `editor/src/app/deps.ts`
- `editor/src/app/main.ts`
- `editor/vite.config.ts`
- `editor/test/code/syntax.test.ts`
- `editor/test/code/syntax-worker.test.ts` (new)
- `editor/test/canvas/edit-cost.test.ts`
- `editor/test/canvas/mount.test.ts`
- `editor/test/canvas/frame.test.ts`
- `editor/test/app/main.test.ts` (only if a self-check row asserts the exact report keys)
- `editor/test/e2e/behavior.mjs`
- `editor/test/e2e/ios-sim.mjs`
- `impl-plans/completed/canvas-cutover-syntax-worker.md` (this plan; progress log only)
- `tmp/canvas-cutover/syntax-worker` (artifact root: logs, `intent.json`, `receipt.json`)
- `editor/dist` (artifact root: page build)
- `editor/node_modules/.vite` (artifact root)
- `target` (artifact root: wasm builds)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root: setup only)

sharedPaths: none.

## Contracts and Key Points

### 1. `FrameScheduler.afterPresent` (`frame.ts`)

- Signature: `afterPresent(cb: () => void): () => void`. It returns a cancel function.
- Semantics:
  - The callback joins a list.
  - After `opts.onFrame` returns, in the `finally` of `tick` (after `recordFrame`), the
    scheduler posts one `setTimeout(..., 0)` if the list is non-empty. Use the global
    `setTimeout` so vitest fake timers drive it.
  - That task runs, in registration order, exactly the callbacks registered before that frame
    ran. Callbacks registered during the task wait for the next frame.
- `afterPresent` calls `this.request()`, so an idle editor still produces the frame.
- While hidden, no frame runs, so the callbacks wait. On resume they run after the first frame.
- `dispose()` drops pending callbacks and clears a posted timer.
- Pitfalls:
  - Never run callbacks inside the rAF callback.
  - Never run them synchronously from `afterPresent`.
  - Never use `queueMicrotask`.
  - A cancel after the timer was posted must still prevent that callback.

### 2. Worker core (`syntax-worker-core.ts`)

Exported message types. These shapes form the contract with `syntax.ts`:

```ts
type Window = [first: number, last: number]; // 0-based lines, inclusive
type SyntaxWorkerRequest =
  | { type: 'init'; base: string }
  | { type: 'reset'; seq: number; text: string; window: Window }
  | { type: 'edit'; seq: number; changes: [fromA: number, toA: number, insert: string][]; window: Window }
  | { type: 'lines'; seq: number; first: number; last: number }
  | { type: 'dispose' };
type SyntaxWorkerReply =
  | { type: 'ready' }
  | { type: 'failed'; reason: string }
  | { type: 'spans'; seq: number; lines: Uint32Array; spans: Uint32Array; truncated: boolean };
```

- `lines` holds `[line, firstSpan, spanCount]` triples. `spans` holds
  `[fromInLine, toInLine, classIndex]` triples, where `classIndex` indexes
  `Object.keys(CAPTURE_CLASSES)` in key order. Export that order as a constant and use it on
  both sides.
- `export class SyntaxWorkerCore`, constructed as
  `new SyntaxWorkerCore({ post(reply, transfer), load(base): Promise<VactSyntax>, defer(cb): void })`.
  - `defer` defaults to `setTimeout(cb, 0)` in the entry.
  - Tests inject a manual queue.
- `handle(request)`:
  - `init`: `load(base)`. On success post `ready`; on rejection post `failed` with the message.
  - `reset`:
    - build a `Text`;
    - drop any pending tree and full-parse on the next `defer`;
    - capture the whole `window`.
  - `edit` (S303-PR-02). Derive everything from one `ChangeSet`, exactly as
    `syntax.ts:noteChanges` does:
    1. `cs = ChangeSet.of(changes.map(([from, to, insert]) => ({ from, to, insert })), doc.length)`
       and `next = cs.apply(doc)`. `base` is the current `doc`.
    2. `cs.iterChanges((fromA, toA, fromB, toB) => ...)`:
       - collect `treeEditFromDocs(base, next, fromA, toA, fromB, toB)`;
       - add the touched lines `next.lineAt(fromB).number - 1` through
         `next.lineAt(Math.min(next.length, toB)).number - 1`. These are 0-based, in
         final-document (`next`) coordinates.
    3. Apply the collected tree edits to the current tree in REVERSE collection order
       (`edits.reverse()`, as `syntax.ts:126` does).
    4. Set `doc = next`, then set `seq` and `window`.
    5. **Coalesced edits.** If a parse is already pending, the touched-line set from the earlier
       edits is REMAPPED through `cs` before the new lines are added. Each earlier line `L`
       becomes `next.lineAt(cs.mapPos(base.line(L + 1).from, 1)).number - 1`. Drop lines that
       fall outside `next`. This keeps earlier lines correct when a later edit shifts line
       numbers. Do not use the plain union without remapping.
    6. Schedule ONE parse with `defer`, so edits that arrive before it coalesce.
  - Parse task (models `syntax.ts:reparse`):
    - run `parseDoc(doc, old)`;
    - if `changedRanges(old)` exists, capture the touched lines plus the changed ranges inside
      the window; otherwise capture the whole window;
    - delete the old tree;
    - post one `spans` reply with `seq` = the latest applied seq.
  - `lines`: if no parse is pending and `seq` matches, capture now and reply. Otherwise add the
    range to the pending parse's capture set.
  - Bound: stop adding spans at 16,384 per reply and set `truncated`.
  - `dispose`: delete the tree.
- The core must never throw out of `handle`. A parse exception falls back to
  `parseDoc(doc, null)`, as `syntax.ts:167-168` does. A second failure posts `failed`.
- `syntax-worker.ts` (entry): binds `self.onmessage` to a `SyntaxWorkerCore` that uses
  `loadVactSyntax` and transfers `[lines.buffer, spans.buffer]`. Guard the binding so it runs
  only when `typeof WorkerGlobalScope !== 'undefined' && self instanceof WorkerGlobalScope`.
  Tests import only the core.

### 3. Main side `WorkerSyntaxSpans` (`syntax.ts`)

- Implements `SpanProvider`.
- Constructor: `(worker: SyntaxWorkerPort, base: string, onSyntax: () => void, onFail: () => void)`.
  - `SyntaxWorkerPort` is the minimal `Worker` subset:
    `postMessage(msg, transfer?)`, `terminate()`, `addEventListener('message' | 'error' | 'messageerror', ...)`.
  - It posts `init` immediately and starts a 10 s `setTimeout` for `ready`.
- `noteChanges(changes, state)`:
  - reuse the existing `remapLines` logic (extract a shared helper inside `syntax.ts`; do not
    duplicate it);
  - `seq++`;
  - post `edit` with `changes.iterChanges` entries `[fromA, toA, inserted.toString()]`;
  - if there are more than 1,024 changes, or more than 65,536 inserted UTF-16 code units, post
    `reset` with `state.doc.toString()` instead;
  - nothing else runs in the keystroke task: no parse, no capture, no `toString` on the normal
    path.
- `spans(state, from, to, limit)`:
  - when `state.doc` differs from the last seen doc without `noteChanges` (a reset or a gap),
    `seq++` and post `reset`;
  - until the first current-seq reply after a reset, return `FallbackSpans` output, as today
    before a tree exists;
  - afterwards return the cached lines;
  - for window lines with no cache entry, post at most one `lines` request per call, and never
    repeat a request already in flight for the same seq and range;
  - remember the capture window and send it with the next `edit` or `reset`.
- Reply handling:
  - `ready`: clear the timeout.
  - `failed`, `error`, `messageerror`, or the timeout: `terminate()`, `stats.workerFailures++`,
    call `onFail` once. After that the provider is inert.
  - `spans` with `seq !== this.seq`: `stats.staleReplies++` and drop it.
  - Current `spans`: decode the triples into line-cache entries, which replace exactly those
    lines, then call `onSyntax`.
- `stats`: `posts`, `resets`, `replies`, `staleReplies`, `workerFailures`, plus `captures` and
  `capturedLines`, which stay 0 because the main thread never captures.
- `dispose()`: post `dispose`, `terminate()`, clear the timers and caches.

### 4. Main-thread fallback (`SyntaxSpans` in `syntax.ts`)

- Add an optional third constructor argument:
  `afterPresent?: (cb: () => void) => () => void`.
- The default is `requestAnimationFrame(() => setTimeout(cb, 0))` from `globalThis` when rAF
  exists, and `setTimeout(cb, 0)` otherwise.
- `schedule()` uses it instead of the immediate `setTimeout(0)`, keeping one pending callback at
  most.
- `flush()` stays synchronous: cancel the pending callback, then reparse.
- `dispose()` cancels it.

### 5. Wiring (`deps.ts`, `main.ts`, `mount.ts`, `vite.config.ts`)

- `EditorDeps.syntaxWorker?: () => SyntaxWorkerPort`.
  - `main.ts` sets it to
    `() => new Worker(new URL('../code/syntax-worker.ts', import.meta.url), { type: 'module' })`.
  - Guard it with `typeof Worker === 'function'`.
- `mount.ts`:
  - If `deps.syntaxWorker` exists, construct `WorkerSyntaxSpans` with `doc.baseURI`, inside
    `try`.
  - **Mode flag rule** (S303-PR-01; this is the only rule):
    - While Worker mode is pending, `codePane.dataset.syntax` stays `'fallback'`.
    - It becomes `'tree-sitter-worker'` only when `WorkerSyntaxSpans` APPLIES its first
      `spans` reply whose `seq` equals the current `seq`. The provider exposes this through
      an `onFirstApply` callback (or the `onSyntax` path with a "first applied" flag).
    - It is never set at construction or on `ready`.
    - On `onFail` it becomes `'tree-sitter'` (the `deps.syntax` path) or stays `'fallback'`.
  - `mount.ts:222` resets the flag to `'fallback'` only when the provider is a
    `FallbackSpans` instance. `WorkerSyntaxSpans` is not one, so the explicit set on the first
    applied reply is required.
  - Hand it the current `surface.state` through `spans()`.
  - `onFail` (or a construction throw) falls back to the existing `deps.syntax()` path. That
    path builds `new SyntaxSpans(syntax, onSyntax, (cb) => scheduler.afterPresent(cb))` and sets
    `'tree-sitter'`. If `deps.syntax` is absent, the provider stays `FallbackSpans` and the mode
    is `'fallback'`.
  - Without `deps.syntaxWorker`, keep today's `deps.syntax` path, adding the `afterPresent`
    argument.
- `vite.config.ts`: add `worker: { format: 'es' }`. Change nothing else in the plugin.
- `main.ts` `reportSelfCheck`: add `syntax: root.querySelector<HTMLElement>('[data-pane="code"]')?.dataset.syntax ?? 'absent'`
  to the JSON report. It is informational.
- `ios-sim.mjs`: copy `selfCheck.syntax` into the result. Do not add it to `result.pass`.
- `behavior.mjs`: after load, wait (bounded, at most 10 s) until
  `document.querySelector('[data-pane="code"]').dataset.syntax === 'tree-sitter-worker'`. Record
  this as a pass/fail check named `syntax-worker` in both browsers. All 18 existing checks stay.

### Patterns to imitate

- `editor/src/code/syntax.ts:SyntaxSpans.reparse`, `captureLines` and `remapLines` for the
  capture and remap logic, which the worker core mirrors.
- `editor/src/code/syntax-core.ts:treeEditFromDocs` for tree edits from `Text` coordinates.
- `editor/test/code/syntax.test.ts` (`// @vitest-environment node`, loading the real
  `web-tree-sitter` and grammar wasm through `node:fs`) for the worker-core equivalence tests.
- `editor/test/canvas/frame.test.ts` (fake `FrameHost`) for the `afterPresent` rows.

## Tasks

### TASK-SW1: `afterPresent` in `FrameScheduler` plus its rows in `frame.test.ts`

### TASK-SW2: `syntax-worker-core.ts`, `syntax-worker.ts`, and the core equivalence rows

### TASK-SW3: `WorkerSyntaxSpans` and the `SyntaxSpans` post-frame fallback in `syntax.ts`, plus their rows

### TASK-SW4: Wiring (`deps.ts`, `main.ts`, `mount.ts`, `vite.config.ts`), mount and edit-cost rows

### TASK-SW5: `behavior.mjs` and `ios-sim.mjs` mode recording, verification, progress log

Each task is done when its listed test rows pass and `npm run check` exits 0.

## Test Cases

`editor/test/canvas/frame.test.ts`:

- `afterPresent(cb)` and timers advanced by 0 with no frame → `cb` not called. One fake rAF frame
  → still not called inside the frame. Advance timers by 0 → called once.
- Cancel before the frame → never called. Cancel after the frame but before the timer → never
  called.
- Hidden document → no call. `visibilitychange` back to visible, then frame plus timer → called
  once.
- `afterPresent` on an idle scheduler requests a frame (fake host `requestAnimationFrame` call
  count +1).

`editor/test/code/syntax-worker.test.ts` (`@vitest-environment node`, real tree-sitter, the
in-process core behind a fake port with a manual delivery queue and a manual `defer` queue):

- *Equivalence:* 200 seeded random edits (ASCII, Japanese, emoji, newline insert and delete)
  applied to a multi-hundred-line `.vact` fixture, with replies delivered at random points.
  After the final drain, `WorkerSyntaxSpans.spans()` over the window equals
  `styleSpans(createVactSyntax(...).parseDoc(doc, null), ...)` mapped to the same class names.
- *Multi-change transactions* (S303-PR-02). The same seeded run includes at least 20
  transactions that are multi-change ChangeSets, built like `bind/write.ts:330`
  (`ChangeSet.of(specs)`), each with 2-5 non-overlapping changes, including newline inserts and
  deletes. Several are posted back to back before a single reply (a burst). After the final
  drain the spans equal a fresh parse.
  - Fixed reproduction row: doc `'a\nb\nc\nd'`, one transaction `[[2,3,'X\nY'],[6,7,'Z']]`.
    The worker's touched set is `{1, 2, 4}` in final-doc coordinates, and the spans equal a
    fresh parse.
  - Coalesced remap row: two edit messages before one parse. The first touches line 5; the
    second inserts a newline on line 1. The pending touched set contains line 6, not line 5.
- *Burst convergence* (review note on 15.3.8.16): 20 edits posted with no reply delivered, then
  one drain → exactly one current-seq reply is applied, `staleReplies` is 0 for that drain, and
  the spans equal a fresh parse.
- *Stale:* deliver the reply for seq n after edit n+1 was posted → the cache is unchanged (spans
  equal the mapped spans) and `staleReplies` is 1. Then deliver n+1 → applied.
- *Keystroke cost:* on the 20,000-line doc, one edit → `postMessage` count +1, main-thread
  `captures` 0, and 0 `Text.prototype.toString` calls on texts of 64 KiB or more. The spy
  filter matches `edit-cost.test.ts:66`, because the inserted text of a change may be
  stringified. No `parseDoc` call on the main side (the fake port records nothing parsed).
  Control branch in the same test: an edit with 65,537 inserted units posts `reset`, with
  exactly one whole-document `toString`.
- *Truncation:* a window whose captures exceed 16,384 → reply `truncated` true, and
  `spans().truncated` is true.
- *Failure:* port `error` event → `terminate` called, `workerFailures` 1, `onFail` called
  exactly once. No `ready` within 10 s (fake timers) → the same.
- *Core robustness:* `init` with a loader that rejects → `failed` posted, and `handle` does not
  throw.

`editor/test/code/syntax.test.ts`:

- *Fallback waits for a presented frame:* with an injected `afterPresent` queue, `noteChanges`
  and timers advanced by 0 → `deferredParses` 0, and `spans()` returns the mapped spans of the
  touched line. Run the queued callback → `deferredParses` 1, and `onSyntax` was called once.
- Existing rows that call `flush()` pass unchanged.

`editor/test/canvas/edit-cost.test.ts`: port the row at lines 41-72 into two rows, without
removing any other assertion:

- *Worker mode* (fake port): the edit gives 0 main-thread parses, 0 `captures`, 1 post, and 0
  large `toString` calls.
- *Fallback mode*: `syncParses` 0; `deferredParses` 0 after the task flush (timers by 0); 1
  after the injected presented-frame callback.

`editor/test/canvas/mount.test.ts`:

- *Mode flag timing* (S303-PR-01), in one test:
  - a fake port that has posted `ready` but no `spans` reply → `data-syntax === 'fallback'`;
  - a `spans` reply with a STALE seq (an edit was posted after it) → still `'fallback'`
    (control branch);
  - a `spans` reply with the current seq → `'tree-sitter-worker'`, and a `syntax`
    invalidation follows the reply.
- A fake port that emits `error` → the mount falls back to `deps.syntax` and
  `data-syntax === 'tree-sitter'`.
- Without `deps.syntaxWorker` → today's behavior.

## Pitfalls

- Never apply an `edit` message change by change against intermediate documents, and never
  record line numbers against an intermediate document. Build one `ChangeSet`, use
  `iterChanges(fromA, toA, fromB, toB)` against the final `next`, apply the tree edits in
  reverse collection order, and remap the pending touched set through each later `ChangeSet`
  (S303-PR-02).
- Never set `data-syntax = 'tree-sitter-worker'` at construction or on `ready`. The behavior
  check must prove that a real parsed reply was applied (S303-PR-01). Do not copy the
  synchronous flag set at `mount.ts:68`/`:324` for Worker mode.
- Accepting a reply by `seq >= current` instead of `===` shows spans for an older document.
- Posting the whole text on every keystroke defeats the plan. Only reset, gap, recovery and the
  oversized-edit rule may send text.
- Forgetting `worker: { format: 'es' }` breaks `npm run build`: IIFE cannot code-split the
  dynamic `web-tree-sitter` import.
- Do not resolve the tree-sitter wasm against the worker's own URL. Always use `base` from
  `init` (`document.baseURI`).
- Do not add a `requestIdleCallback` path or a Chromium-only branch.
- Line numbers in replies refer to document `seq`. Applying them after another edit misplaces
  styles, which is what the `===` rule prevents.
- Keep every touched file under 1,000 lines. `syntax.ts` is 244 lines and `mount.ts` 358 today.

## Verification

The measurement lock and quiet-host gate (design 15.3.8.16 part D) are required for every
browser run and for full nextest:

```sh
until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done
echo CANVAS-SYNTAX-WORKER > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner
trap 'rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock' EXIT
```

Wait for load average below 9, with at most 2 busy build or test processes, before taking the
lock. Release it right after the run.

Inside the sandbox (implementer):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run check > ../tmp/canvas-cutover/syntax-worker/npm-check.log 2>&1` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/code/syntax-worker.test.ts test/code/syntax.test.ts test/code/syntax-core.test.ts test/code/syntax-fallback.test.ts test/canvas/edit-cost.test.ts test/canvas/mount.test.ts test/canvas/frame.test.ts test/app/main.test.ts > ../tmp/canvas-cutover/syntax-worker/focused.log 2>&1` | exit 0; every row in Test Cases is present and passes |
| `cd editor && ./node_modules/.bin/vitest run > ../tmp/canvas-cutover/syntax-worker/vitest-full.log 2>&1` | exit 0, failureCount 0, testsRun at least 810 plus the new rows |
| `node --check editor/test/e2e/behavior.mjs` then `node --check editor/test/e2e/ios-sim.mjs` (one file per invocation; S303-PR-L3) | both exit 0 |
| `git diff --check` | exit 0 |

Outside the sandbox (verification step, serially, one heavy command at a time):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run test:perf > ../tmp/canvas-cutover/syntax-worker/test-perf.log 2>&1` (alone) | exit 0 |
| `mise run build-wasm-release` then `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build > ../tmp/canvas-cutover/syntax-worker/build.log 2>&1` | exit 0; `ls editor/dist/assets` lists a `syntax-worker` chunk |
| Under the lock: `cd editor && npm run e2e -- --browser all --profile behavior --run-id s303-syntax-worker --out ../tmp/canvas-cutover/syntax-worker/behavior > ../tmp/canvas-cutover/syntax-worker/behavior.log 2>&1` | exit 0; Chromium and WebKit both pass all checks, including `syntax-worker` |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0 (no Rust touched; green-after-every-plan rule) |
| Under the lock: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |

Every verification record uses the mandatory format: numeric `exitStatus: 0`,
`outcome: "passed"`, `testsRun > 0` and `failureCount: 0` for tests, and a `log` path; details go
in `notes`. No mutation or negative-control commands are run. Sensitivity is shown only by the
in-test control branches above.

## Overwrite and Drift Protocol

Before each edit, record the fresh-read sha256 of the target in
`tmp/canvas-cutover/syntax-worker/intent.json`. Record the post-edit hash in `receipt.json`. If
a not-yet-edited file changed since its fresh read, stop editing that file and record the drift.
Repair is serial. Edit only this plan's progress log among the plan files.

## Completion Criteria

- [x] `FrameScheduler.afterPresent` exists with the four `frame.test.ts` rows passing.
- [x] `editor/src/code/syntax-worker-core.ts` and `editor/src/code/syntax-worker.ts` exist; the
  entry binds only in a worker scope.
- [x] `WorkerSyntaxSpans` posts exactly one `edit` per transaction. `reset` is sent only for
  reset, gap, recovery, more than 1,024 changes, or more than 65,536 inserted units.
- [x] Replies with a non-current seq are dropped (`staleReplies`); after a stale drop, the next
  current-seq reply triggers a current-window refresh while in-window mapped spans stay visible.
  These rows pass: equivalence (including at least 20 multi-change transactions and bursts), the
  `{1,2,4}` reproduction, coalesced remap, burst convergence, stale interleave, keystroke cost,
  truncation and failure.
- [x] The worker derives tree edits and touched lines from one `ChangeSet` in final-document
  coordinates, and remaps the pending touched set across coalesced edits.
- [x] `data-syntax` becomes `tree-sitter-worker` only after the first current-seq reply is
  applied. The mount row shows `'fallback'` after `ready`, no flip on a stale reply, and the
  flip on a current reply.
- [x] The `SyntaxSpans` fallback reparses only after a presented frame, and `flush()` stays
  synchronous.
- [x] `vite.config.ts` has `worker: { format: 'es' }`; the release page build emits the worker
  chunk.
- [x] `data-syntax` is `tree-sitter-worker` in Chromium and WebKit in the behavior profile; the
  self-check and `ios-sim.json` record `syntax` (informational).
- [x] The full default vitest run, `npm run check`, `test:perf`, strict clippy, full nextest, the
  wasm32 build and the Tauri check all exit 0.
- [x] No touched file reaches 1,000 lines. No dependency, Rust or threshold change
  (`git diff --name-only` stays within the writePaths).
- [x] The progress log records the hashes, logs and results.

## Progress Log

### Session: 2026-10-07 (session 303 plan authoring)

**Tasks Completed**: Plan authored from design 15.3.8.16 part A (accepted by step 3,
comm-004816). The step-3 low note on burst convergence is covered by the "Burst convergence"
row.

### Session: 2026-10-07 (session 303 plan revision after step 5, comm-004818)

**Tasks Completed**: Plan repaired for the step-5 findings.

- S303-PR-01: there is now a single mode-flag rule. The flag becomes `tree-sitter-worker` only
  after the first current-seq reply is applied. A mount row with a stale-reply control and a
  pitfall were added.
- S303-PR-02: the worker edit path uses one `ChangeSet`, `iterChanges(fromA, toA, fromB, toB)`
  against the final document, reversed tree edits, and a remapped coalesced touched set.
  Multi-change, reproduction and remap rows were added, and the descending per-change pitfall
  was replaced.
- S303-PR-L3: `node --check` now runs one file per invocation.

### Session: 2026-10-07 (session 303 implementation)

**Tasks Completed**: Implemented `FrameScheduler.afterPresent`, the side-effect-free worker
core and worker-only entry, main-thread `WorkerSyntaxSpans`, post-frame fallback parsing, Vite
worker format, mount mode reporting, and the behavior/iOS informational syntax fields. Added
deterministic frame, worker equivalence/burst/stale/failure/cost, fallback, edit-cost and mount
mode rows. The multi-change worker edit path constructs one `ChangeSet`, computes touched lines
in final-document coordinates, reverses tree edits, and remaps coalesced touched lines.

**Final-source passing verification**:

- `cd editor && npm run check` — exit 0; `tmp/canvas-cutover/syntax-worker/session-303/npm-check-final-04.log`.
- Focused Vitest command from the verification table — exit 0, 8 files / 88 tests;
  `tmp/canvas-cutover/syntax-worker/session-303/focused-final.log`.
- Full `cd editor && ./node_modules/.bin/vitest run` — exit 0, 94 files / 823 tests;
  `tmp/canvas-cutover/syntax-worker/session-303/vitest-full-final.log`.
- `node --check` for each E2E module and `git diff --check` — exit 0; logs in the same
  session-303 evidence directory.
- Strict clippy, wasm32 build, and Tauri check — each exit 0; `clippy-final.log`,
  `wasm-build-final.log`, and `tauri-check-final.log` in the session-303 evidence directory.
- Every touched file is below 1,000 lines; no Rust, dependency, lockfile, threshold or
  workload changes were made.

**State at the session-303 checkpoint**: release page build and worker chunk inspection,
Chromium/WebKit behavior profile, full nextest, and a passing serial `test:perf` were pending.
Session 304 resolved these gates on the final source; see its progress record and logs under
`tmp/canvas-cutover/syntax-worker/session-304/`.


### Session: 2026-10-07 (session 304 final-source verification)

**Tasks Completed**: Re-ran the assigned implementation gates on checkpoint `a8b209b` after the measurement lock became available. Confirmed the release page emits `syntax-worker-BVsDP44l.js`; Chromium (11/11 checks) and WebKit (9/9 checks plus two separately labeled synthetic-event limitations) both applied current tree-sitter Worker spans. Full default Vitest passed 823/823, the focused suite passed 88/88, and the dedicated serial perf gate passed. Full nextest passed 2,817 tests with 3 existing skips. Strict clippy, wasm32 build, Tauri check, npm check, both E2E syntax checks, and diff check exited 0. No source changes were needed in this continuation.

**Final-source passing verification** (logs under `tmp/canvas-cutover/syntax-worker/session-304/`):

- `cd editor && npm run check`: exit 0.
- Focused Vitest command from the Verification table: exit 0; 8 files / 88 tests passed.
- `cd editor && ./node_modules/.bin/vitest run`: exit 0; 94 files / 823 tests passed.
- `cd editor && npm run test:perf`: exit 0; 1 test passed; ratio 2.65.
- `mise run build-wasm-release` and `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`: both exit 0; worker chunk present.
- Behavior E2E command: exit 0; Chromium 11/11 and WebKit 7/7 automated checks passed, including `syntax-worker` in each browser.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`: exit 0.
- Locked `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`: exit 0; 2,817 passed, 3 existing skips.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`, `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml`, both `node --check` commands, and `git diff --check`: exit 0.
- Every touched source file remains below 1,000 lines. No dependency, Rust source, workload, or threshold change.
- Quiet-host load observations: `behavior-host-load.txt` and `nextest-host-load.txt`.
- Plan fresh-read SHA-256: `16b2d8059cb5abe4b5df1ec0a90073c314fe328994ea919f07392142b5d69c9d`; post-edit SHA-256 is recorded in `receipt-session-304.sha256`.


### Session: 2026-10-07 (session 305 adversarial repair)

**Finding addressed: ADV-S304-SW-STALE-REFRESH.** `WorkerSyntaxSpans` now remembers dropped stale spans replies. After the next current-sequence reply it clamps and refreshes the current capture window, evicts only out-of-window cache entries, preserves in-window mapped spans during the refresh, and tracks the in-flight window so later `spans()` calls do not duplicate covered line requests. Added the production-path stale-interleave row; it performs no manual full-document `lines` recapture and requires eventual whole-document equality with a fresh parse. Existing equivalence and stale-drop assertions remain unchanged.

**Final-source verification** (logs under `tmp/canvas-cutover/syntax-worker/session-305/`):

- `cd editor && ./node_modules/.bin/vitest run test/code/syntax-worker.test.ts test/code/syntax.test.ts test/canvas/edit-cost.test.ts test/canvas/mount.test.ts test/canvas/frame.test.ts`: exit 0; 5 files / 70 tests passed.
- `cd editor && npm run check`: exit 0.
- `cd editor && ./node_modules/.bin/vitest run`: exit 0; 94 files / 824 tests passed.
- `mise run build-wasm-release` and `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`: both exit 0; worker chunk emitted.
- Locked `cd editor && npm run e2e -- --browser all --profile behavior --run-id s305-syntax-worker --out ../tmp/canvas-cutover/syntax-worker/session-305/behavior`: exit 0; aggregate 20/20, `syntax-worker` passes in Chromium and WebKit. WebKit: 9/9 automated checks, plus two separately labeled limitations.
- Locked `cd editor && npm run test:perf`: exit 0; 1/1, ratio 3.10.
- `git diff --check`: exit 0.
- Final source SHA-256: `editor/src/code/syntax.ts` eae60c71462978bdebe0daf8eb34c169f588c3be2f06846e2684c0eddce2928c; `editor/test/code/syntax-worker.test.ts` 8a532cf9b5e7bf848aed1eccf390ff5c07170bb10d2f1f535db1710f22f9686c.

Regression authoring produced two corrected intermediate test attempts: the initial stale-line expectation was too strict about the parser's changed-range capture, and the next row called `spans()` on the fake port. Their complete logs are `focused-review-fix.log` and `stale-worker-regression.log`; the corrected dedicated row passes in `stale-worker-regression-02.log` and the final focused/full suites above.
