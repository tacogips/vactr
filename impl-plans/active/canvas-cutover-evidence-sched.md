# Canvas Cutover: Fresh Engine-Time Tick and Diagnostics Check Scheduling (Decisions B and C) Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-EVIDENCE-SCHED (dispatch wave 6 of the session-277 run; runs alone after CANVAS-EVIDENCE-SCOPE is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.13 (scope records B and C; "Ownership and order"), 12.8.4 (`host_now` is the engine timebase), section 16 (late events play immediately, are counted and widen the latency window)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-EVIDENCE-SCHED`)
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

The user wants playing-code highlights synchronized with the audio. The operator diagnosis
(`tmp/canvas-cutover/diag-wasm/REPORT.md`, sections 3 and 6) found WebKit presented zero
highlights (TASK-402). Every workload onset reached the page 1.84 to 20.9 s after its audible
start, after the note had already ended.

The cause is `editor/worklet/host.js` `VactrHost.onWorklet` (lines 87-92). It calls the session
tick with the posted `m.t`, which goes stale while worklet messages queue behind main-thread
long tasks. Measured lag is up to 771 ms during edits; 339 of 2,350 ticks fell beyond the
120 ms lookahead.

Decision C removes avoidable long tasks: the typing-time `session_check` must not run again
for a revision that was already checked.

The two clocks (design 15.3.8.13 B "Two clocks"):

- `m.t` is `worklet_now()` (`editor/worklet/processor.js:144`), the engine render clock
  (`src/dsp/engine.rs:224` `Engine::now`, the 12.8.4 `host_now`). It counts frames rendered
  since worklet init.
- `ctx.currentTime` runs at the same rate, but its origin is earlier by `delta >= 0`, which
  covers the time before engine init plus any skipped quanta. `processor.js` posts
  `js: [slotDrops, tooLarge, errors, gaps, count]`, so `m.js[2]` is the error counter and
  `m.js[3]` is the gap counter.
- Raw `ctx.currentTime` must never be passed to the tick: that would put `host_now` `delta`
  ahead of the engine.

`Runtime::tick` (`src/sched/runtime.rs:533-551`) queries the spans not yet covered, starting
from its cursor. Fewer tick calls therefore never skip or double an event.

## Non-goals

- No edit to `editor/worklet/processor.js`, `editor/worklet/host.d.ts`, any Rust file
  (including `src/sched/commit.rs`; the optional past-event drop is not adopted), or the
  section 16 late-event semantics.
- No Worker or off-main-thread scheduling. That is escalation F, which needs its own
  amendment 15.3.8.14 and is not part of this plan.
- No change to `CHECK_DEBOUNCE_MS` (300 ms), the diagnostics merge or presentation, or the
  `DiagnosticsController` public API.
- No change to thresholds, the workload, the harness, or any file outside the writePaths.

## Ownership

writePaths:

- `editor/worklet/host.js`
- `editor/test/protocol/host-js.test.ts`
- `editor/src/code/diagnostics.ts`
- `editor/test/code/diagnostics.test.ts`
- `impl-plans/active/canvas-cutover-evidence-sched.md` (progress log and checkboxes only)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`, `tmp/canvas-cutover/sched`

sharedPaths: none. `editor/test/code/diagnostics.test.ts` is also a CANVAS-EVIDENCE path
(as a wave-8 sharedPath); this plan owns it during wave 6 and CANVAS-EVIDENCE does not run
until this plan is accepted.

## Contracts and Key Points

### B. Fresh engine-time tick (`editor/worklet/host.js`)

New private state on `VactrHost`, initialized in the constructor:

- `engineOffset`: `null` until the first timed message;
- `lastGaps` and `lastErrors`: `-1`.

In `onWorklet(m)`, when `typeof m.t === 'number'`:

1. `const ct = this.ctx?.currentTime`. If `ct` is not a finite number (tests construct
   `VactrHost(null, ...)`), set `fresh = m.t`; that is today's behavior.
2. Otherwise, compute `sample = ct - m.t`.
   - Reset `engineOffset = sample` if `engineOffset` is `null`, or if `m.js` is an array and
     `m.js[3] > lastGaps` or `m.js[2] > lastErrors` (only once a previous value exists, that
     is `lastGaps >= 0`).
   - Otherwise set `engineOffset = Math.min(engineOffset, sample)`.
   - Then update `lastGaps` and `lastErrors` from `m.js` when it is present.
   - `fresh = Math.max(m.t, ct - engineOffset)`.
3. The tick time is `Math.max(this.lastTick, fresh)`, so tick times never decrease
   (`lastTick` starts at `-1`). Set `this.now` to this value.
4. Tick only when `this.lastTick < 0 || now - this.lastTick >= this.tickEvery`. Then set
   `this.lastTick = now` and call `this.x[this.fn.tick](now)`. That gate coalesces a queued
   backlog into at most one catch-up tick.
5. Everything else in `onWorklet` stays in its current order: the `ready`/`error` early
   returns, `m.o` to inbox, `m.r`, `m.js`, then `flush()`.

Update the header comment (lines 7-9): the tick uses the fresh engine time, not the posted
frame time. Do not change `host.d.ts`; the new fields are private implementation state.

Why the minimum: queueing delay only adds to `ct - m.t`, so the minimum approximates `delta`.
After a gap or error the engine falls further behind, so the estimate resets to the current
sample. A conservative (too large) offset only makes the tick trail the engine slightly, which
stays within the lookahead.

### C. Check skip on an unchanged revision (`editor/src/code/diagnostics.ts`)

- Add a private `checkedRevision: number | null = null`.
- In `runCheck()`, after `disarm()` and the `checksEnabled` guard, read
  `rev = this.opts.sync.revision`. If `rev === this.checkedRevision`, return without calling
  `core.check`.
- Set `checkedRevision = rev` only after `core.check` returns, so a thrown check can be
  retried at the same revision.
- `scheduleCheck` (debounced, latest-wins through `disarm`) stays as it is.
- `DocSync` revisions only increase (`editor/src/protocol/document.ts:114, 158`; there is no
  reset). Revision equality therefore identifies identical text for one controller. Record
  that confirmation in the progress log.
- `dispose()` keeps its behavior; it does not need to clear `checkedRevision`.

## Tasks

### TASK-B1: host.js tick time
**Deliverables**: `editor/worklet/host.js`, `editor/test/protocol/host-js.test.ts`
**Completion criteria**: contract B implemented; the new tests pass; the existing host-js
tests pass unmodified.

### TASK-C1: diagnostics revision skip
**Deliverables**: `editor/src/code/diagnostics.ts`, `editor/test/code/diagnostics.test.ts`
**Completion criteria**: contract C implemented; the new tests pass; the existing diagnostics
tests pass unmodified.

### TASK-S1: Verification and progress log

## Test Cases

`editor/test/protocol/host-js.test.ts` (new `describe` block). Construct
`new VactrHost(fakeCtx, fakeNode(), fake.exports, { init: 'session', ...})` with
`fakeCtx = { currentTime }`, a mutable object cast through `unknown` to `AudioContext`. Read
tick calls with `fake.callsOf('session_tick')` and the time from `args[0]`. Imitate the
`host(...)` helper and the `FakeCore` usage at the top of the file.

- Offset 0.5 s: the engine starts 0.5 s after the context.
  - Set `currentTime = 10.0` and deliver `{ t: 9.5, js: [0,0,0,0,0] }` -> 1 tick at 9.5.
  - Advance `currentTime` to 11.0 and deliver 200 stale messages with `t` from 9.5 to 9.9
    (`js` unchanged) -> at most 1 new tick. Its time lies in
    `[(11.0 - 0.5) - tickEvery, 11.0 - 0.5]`.
  - All tick times never decrease.
- No `ctx` (`VactrHost(null, ...)`): ticks use `m.t` exactly as before. The existing
  `t: 0.5` and `t: 1.25` cases keep passing.
- A gap re-estimates the offset. Establish offset 0.5 and a tick at engine time 10.5
  (`currentTime = 11.0`, `t = 10.5`). Skipped quanta then advance the context 0.7 s further
  than the engine: deliver `{ t: 10.6, js: [0,0,0,1,0] }` at `currentTime = 11.8` (true offset
  now 1.2). Expect a tick at exactly 10.6, not 11.3 (which is what the stale 0.5 offset would
  give), and never earlier than the previous tick. Keep engine times monotonic in every
  fixture, because the real engine clock never goes backwards.
- A queued first message gives a conservative offset. The first message arrives with
  `ct - t = 0.8`; a later fresh message with `ct - t = 0.5` lowers the offset to 0.5.
- tickEvery gating is unchanged when there is no backlog: messages every 128/48000 s of
  engine time with `currentTime` in step tick at most once per `tickEvery` (0.005).

`editor/test/code/diagnostics.test.ts` (new `it` blocks in the existing `describe`, using the
existing `setup('browser', check)` and fake timers):

- 20 edits 50 ms apart, then 300 ms -> `core.check` called exactly once.
- `ctl.runCheck()` twice with no edit in between -> `core.check` called once.
- After one completed check, advancing fake timers by 10 s with `ctl.refresh()` calls and no
  edit -> 0 additional checks (frames and refreshes never check).
- A `check` that throws once is retried at the same revision on the next `runCheck()` -> 2
  calls total.

## Pitfalls

- Passing raw `ctx.currentTime` to the tick is wrong (DR-S277-B-TIMEBASE).
- Do not move or reorder the inbox, report or flush handling in `onWorklet`.
- `m.js` is absent in many existing tests. Treat it as "no counter change".
- `lastTick` doubles as the monotonic floor. Never assign it a smaller value.
- Do not add a timer, `setInterval`, Worker or rAF-driven tick. The worklet message remains
  the only trigger, so audio scheduling never depends on rendering.
- In `diagnostics.ts`, do not compare text strings (that would read the whole 1 MiB document).
  Compare revisions only.

## Verification

Setup (not gating):

- `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`

At start, record `BASE=$(git rev-parse HEAD)` and the fresh-read sha256 values in
`tmp/canvas-cutover/sched/intent.json`.

Gating, inside the sandbox (logs in `tmp/canvas-cutover/sched/`):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && ./node_modules/.bin/vitest run test/protocol/host-js.test.ts test/code/diagnostics.test.ts` | exit 0; the new cases are listed by name |
| `cd editor && ./node_modules/.bin/vitest run test/protocol test/code test/app test/midi` | exit 0 (MIDI clock, song mode and transport tests stay green) |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 (default config) |
| `cd editor && npm run check` | exit 0 |
| `git diff --name-only $BASE` | only this plan's writePaths; no `.rs` file |

Outside the sandbox (verification step):

| Command | Required evidence |
|---------|-------------------|
| `cd editor && npm run test:perf` (alone on the host) | exit 0 |
| `git diff --name-only $BASE -- '*.rs' Cargo.toml Cargo.lock` | empty (the Rust suite is unchanged since SCOPE's accepted full nextest run, so it is not re-run here) |

Mutation evidence (separate, never gating): with the B change reverted (tick at `m.t`), the
200-stale-message case fails. With the C skip removed, the `runCheck` twice case fails. Record
the command, the nonzero exit code and the log path.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/sched/intent.json` and
`receipt.json`. If a file drifted from its fresh read without an edit from this plan, stop
editing it and report. Edit only this plan's progress log.

## Completion Criteria

- [x] host.js ticks at the fresh, monotonic engine time with the offset estimate and gap/error reset; `ctx === null` behaves as before
- [x] The 0.5 s offset test: one catch-up tick for 200 stale messages, tick time inside `[(ct - 0.5) - tickEvery, ct - 0.5]`, monotonic
- [x] diagnostics: 20 edits -> 1 check; unchanged revision -> 0 checks; refreshes -> 0 checks; retry after a throw
- [x] Existing host-js and diagnostics assertions remain present; default vitest and `npm run check` pass
- [x] `npm run test:perf` passes (outside the sandbox); no Rust change
- [x] Sensitivity shown by in-test controls (session 283); no mutation command run
- [x] Progress log updated

## Progress Log

### Session: 2026-10-05 (session 277 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.13 B (as revised for DR-S277-B-TIMEBASE)
and C. No source edits.

### Session: 2026-10-06 (session 283 amendment: in-test controls replace mutation runs)
**Hard rule (workflowInput, session 283)**: run no mutation or negative-control command.
Structured `verification[]` and `priorVerification[]` list only final-source commands that
exited 0 with positive test counts. Superseded, typo, timed-out or failed attempts never
appear there. If a gate fails, fix it inside the writePaths, rerun it, and report only the
final passing run.

The "Mutation evidence" paragraph in Verification and the criterion "Mutation runs recorded
separately" are replaced by sensitivity controls inside the passing tests. Each control
asserts both branches in one `it` block:
- **200-stale-message test (B)**: also assert the fixture sensitivity. The latest posted `t`
  (9.9) lies below the window floor `(11.0 - 0.5) - tickEvery`, so a tick at `m.t` would fail
  the window assertion. The actual tick time lies inside the window.
- **Gap test (B)**: assert both that the tick is exactly 10.6 and that the stale-offset
  candidate `11.8 - 0.5 = 11.3` differs from it. A host that kept the stale offset would
  therefore fail.
- **runCheck-twice test (C)**: after the second `runCheck()` (0 new checks), make one edit
  and call `runCheck()` again -> exactly 1 new check. That proves the skip keys on the
  revision and does not disable checking.
- **20-edits test (C)**: after the single check, one more edit plus 300 ms -> exactly 1
  more check.

Tick the criterion as "Sensitivity shown by in-test controls (session 283)". Do not edit the
existing test rows. Everything else in this plan (contracts B and C, writePaths, the
no-Rust rule and gating commands) is unchanged.

### Session: 2026-10-06 (session 285 implementation)
**Tasks Completed**: Implemented fresh monotonic engine-time scheduling in `editor/worklet/host.js` using the minimum observed context/engine offset, with reset on increased gap/error counters. Added fake-context coverage for stale-message coalescing, offset reset, minimum estimation and tick cadence. Added diagnostics `checkedRevision` tracking after successful checks, plus coverage for edit bursts, unchanged revisions, refresh/frame activity and retry after a throw.

**Verification**: Final-source logs in `tmp/canvas-cutover/sched/`: `focused-vitest-postfinal.log` (20/20), `sched-suites-vitest-postfinal.log` (254/254), `full-vitest-postfinal.log` (710/710), `npm-check-postfinal.log` (exit 0), `test-perf-postfinal.log` (1/1, ratio 2.76), and `source-diff-audit.log` (diff check clean, Rust/Cargo files empty). All five execution logs record the numeric status observed from the completed foreground command. No Rust, Cargo manifest or lockfile changed. The initial worktree already contained `impl-plans/active/canvas-cutover-evidence-scope.md` from the accepted predecessor; that concurrent plan progress was preserved. Formal reviews and later workflow integration remain downstream.

### Session: 2026-10-06 (session 285 review repair: SCHED-TI-1)
**Finding**: The earlier minimum-offset test could not distinguish a retained 0.8 offset from the intended 0.5 minimum because `max(m.t, ...)` returned `m.t` in both cases.

**Correction**: Added `lowers a queue-inflated first offset to the minimum sample` in `editor/test/protocol/host-js.test.ts`. Monotonic samples `(ct,t)=(1.8,1.0)` and `(1.9,1.4)` establish offsets 0.8 then 0.5; a stale `(2.3,1.41)` message ticks at 1.8. The same test rejects both the stale-offset result 1.5 and raw posted time 1.41 by more than `tickEvery`. No product code or prior assertions changed.

**Verification**: Current-source focused tests pass 21/21 (`tmp/canvas-cutover/sched/ti-1-focused.log`), scoped suites pass 255/255 (`ti-1-scoped-vitest.log`), full Vitest passes 711/711 (`ti-1-full-vitest.log`), `npm run check` exits 0 (`ti-1-npm-check.log`), and serial `npm run test:perf` passes 1/1 at ratio 2.97 (`ti-1-test-perf.log`). `append-only-audit.log` proves the test file differs from the captured pre-repair snapshot only by inserted lines. `receipt.json` records refreshed source hashes and current verification logs. The earlier logging-wrapper exit 1 occurred after its Vitest run reported 20/20; it was a zsh reserved-variable issue, resolved by the successful logged rerun. No mutation command was run.
