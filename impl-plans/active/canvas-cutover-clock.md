# Canvas Cutover: Audible Clock Implementation Plan

**Status**: Ready
**Plan ID**: CANVAS-CLOCK (wave 1, no dependencies)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.4 (also 15.3.8.6 scopes fields, 15.3.8.9)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json
**Created**: 2026-10-05
**Last Updated**: 2026-10-05

---

## Intent and Context

User goal: playing-code highlights, the beat indicator, scopes and Hydra must follow the
audio actually heard, not message receipt or raw `AudioContext.currentTime`.
This plan adds the frontend audible clock (`AudibleClock`) with two correlation sources and
moves highlight and transport timing onto it.
It does not touch rendering, the canvas mount, or Rust.

Repository facts at c9e5a05:

- `editor/src/app/clock.ts` has only `AudioClock` (`currentTime`) and `PageClock`.
- `editor/src/code/highlight.ts:HighlightScheduler` uses `TimeAnchor`, and the native tier
  anchors at receipt.
- `editor/src/protocol/store.ts:Store.transportSample` already keeps the newest same-epoch
  `TransportSample`. See also `store.ts:91`, which rejects stale samples.
- The clock-probe wire contract already exists on the TypeScript side, and this plan keeps it
  unchanged:
  - request `{ kind: 'clock-probe', body: { page_send } }` (`protocol/client.ts:153`
    `Client.clockProbe`);
  - reply `ClockProbeReply` with fields `page_send`, `engine_receive`, `engine_send`, `epoch`,
    optional `correlation {engine_time, output_time}`, `latency_seconds`, `latency_kind` and
    `uncertainty_seconds` (`protocol/types.ts:204-215`);
  - validation in `protocol/envelope.ts:281-290`, with tests in
    `test/canvas/contracts.test.ts:84-96`.
- Design 15.3.8.4 names the fields `page_time_ms` / `processing_time` with reply kind
  `clock-probe-reply`. **Plan reconciliation (binding):** keep the existing tested names and the
  single kind `clock-probe`. The mapping is:
  - `page_send` is `performance.now()` in milliseconds, echoed by the engine;
  - `engine_receive` and `engine_send` are the engine processing time in seconds (they are equal
    when the engine has a single reading);
  - `processing_time` is the midpoint of `engine_receive` and `engine_send`.

  CANVAS-NATIVE produces exactly this shape.

## Non-goals

- No change to Rust, to the canvas renderer, or to `code/mount.ts` beyond clock wiring.
- No removal of the EditorView decoration code in `highlight.ts` (`setPlaying`, `playingField`,
  `highlightExtension`, `playingRanges`). CANVAS-MOUNT deletes it.
- No change to `deps.clock` semantics. It stays processing time, because MIDI forward and learn
  depend on it.
- No new dependency and no `package.json` change.

## Ownership

writePaths: see the manifest entry `CANVAS-CLOCK`. The files and their changes:

| File | Change |
|------|--------|
| editor/src/app/clock.ts | Add the `AudibleClock`, `OutputTimestampCorrelation` and `ProbeCorrelation` classes and the `uncorrelated(clock)` helper |
| editor/src/app/deps.ts | Add optional `audible?: AudibleClock` to `EditorDeps` |
| editor/src/app/main.ts | Create `deps.audible` per tier, start and dispose `ProbeCorrelation` on the `?session=` tier |
| editor/src/code/highlight.ts | Audible-time activity, `end_time`, epoch filter, 2 s horizon, hide when invalid |
| editor/src/code/transport.ts | Sample-based position, 80 ms beat flash window, stale hide |
| editor/src/code/mount.ts | Pass `deps.audible` and the rAF timestamp to highlight and transport only |
| editor/src/protocol/types.ts | `LevelsBody.time?: number`, `LevelsBody.epoch?: string` |
| editor/src/protocol/envelope.ts | Validate the optional levels `time` and `epoch` |
| editor/test/canvas/clock.test.ts (new) | Deterministic simulated-clock suite |
| editor/test/code/highlight.test.ts, editor/test/code/transport.test.ts | New audible cases; existing assertions kept |
| editor/test/canvas/contracts.test.ts | Levels `time`/`epoch` decode cases |
| editor/test/support/clock.ts | `SimulatedTime` helper (page ms plus context seconds, stall injection) |
| editor/test/app/main.test.ts | Boot and tier-wiring rows (see TASK-005). CANVAS-SHELL owns this file in wave 2 and must keep these rows. |

sharedPaths (conditional, expected unedited):

- `editor/test/code/reconcile.test.ts`: edit only if the `HighlightScheduler` option changes
  break it. Keep every assertion.
- `editor/src/protocol/store.ts`: edit only to expose the latest sample epoch if it is not
  reachable.

## Contracts (pin exactly; CANVAS-RENDER, MOUNT, VISUAL and SHELL consume them)

```ts
export type Provenance = 'measured' | 'estimate' | 'unavailable';
export interface CorrelatedTime { time: number; uncertainty: number; provenance: Provenance }
export interface ClockCorrelation { at(pageMs: number): CorrelatedTime | null }
export interface AudibleSample { time: number; targetMs: number; epoch: string | null; provenance: Provenance; uncertainty: number; valid: boolean }
export class AudibleClock implements Clock {
  constructor(correlation: ClockCorrelation, opts?: { pageNow?: () => number; epoch?: () => string | null });
  now(): number;                       // audible processing seconds at pageNow()
  sample(frameMs: number): AudibleSample; // at targetMs = frameMs + lead; cached per frameMs
}
export function uncorrelated(clock: Clock): AudibleClock; // provenance 'unavailable', time = clock.now()
export function audibleFor(tier: 'browser' | 'native', src: { ctx?: OutputContextLike; client?: Pick<Client, 'clockProbe'>; timers?: Timers }):
  { audible: AudibleClock; start(): void; dispose(): void };  // browser: OutputTimestampCorrelation; native: ProbeCorrelation
```

- `lead` is an EMA (alpha 0.1) of the deltas between successive distinct `frameMs` values,
  clamped to [8.3, 33.4] ms, with a default of 16.7. `targetMs = frameMs + lead`.
- `OutputContextLike` is
  `{ currentTime: number; sampleRate: number; outputLatency?: number; state?: AudioContextState; getOutputTimestamp?(): { contextTime?: number; performanceTime?: number } }`.

Validity is independent of provenance. Provenance says how well latency is compensated;
validity says whether a time can be shown at all.

| Source | `valid` is true when | `valid` is false when |
|--------|----------------------|-----------------------|
| `OutputTimestampCorrelation` | `ctx.state` is absent or `'running'`. This includes provenance `estimate` and `unavailable`. | `ctx.state` is `'suspended'` or `'closed'`; `at()` returns null |
| `ProbeCorrelation` | A sample younger than 3000 ms exists, for any `latency_kind`, including `'unavailable'` | No fresh sample, so `at()` returns null |
| `uncorrelated(clock)` | Always, with provenance `unavailable`, labeled unsynchronized | Never |

`AudibleClock.sample().valid === (correlation.at(targetMs) !== null)`.

## Read-only accessors (pinned; CANVAS-MOUNT reads them for `window.__vactrPerf`)

```ts
// highlight.ts
class HighlightScheduler {
  readonly stats: { received: number; accepted: number; overflow: number; horizonDrops: number; epochDrops: number; unmapped: number };
  active(): readonly Range16[];   // the last set passed to apply (already exists; keep)
  onAccept(cb: (e: { time: number; end: number; from: number; to: number; epoch: string | null }) => void): () => void;
}  // onAccept fires once per accepted event at receipt, with the range mapped at receipt; it is not called for dropped or unmappable events
// transport.ts
class TransportBar { readonly state: { cycle: number | null; beatFlash: boolean; hidden: boolean } }  // updated by every tick(frameMs)
// clock.ts
class ProbeCorrelation { readonly stats: { samples: number; rejected: number; failures: number } }
```

Accessors are plain fields updated in place. They allocate nothing per frame, and an
`onAccept` with no listeners costs one emptiness check.

## Tasks

### TASK-001: Correlation sources and AudibleClock (app/clock.ts, app/deps.ts)

**Status**: Not Started.
**Imitate**: the `editor/src/midi/forward.ts:21-33` `getOutputTimestamp` guard style, and the
small class style of `editor/src/app/clock.ts`.

`OutputTimestampCorrelation(ctx)` takes `ctx: OutputContextLike`. It returns `null` when
`ctx.state` is `'suspended'` or `'closed'`. Otherwise it resolves in this order:

1. If `getOutputTimestamp()` gives finite `contextTime > 0` and `performanceTime > 0`,
   `|pageMs - performanceTime| <= 1000` and `contextTime <= currentTime`, return
   `contextTime + (pageMs - performanceTime) / 1000` with provenance `measured` and uncertainty
   `128 / sampleRate`.
2. Otherwise, if `outputLatency` is finite and > 0, return `currentTime - outputLatency` with
   provenance `estimate` and uncertainty `outputLatency / 2`.
3. Otherwise return `currentTime` with provenance `unavailable`.

Never subtract latency in the `measured` branch.

`ProbeCorrelation(probe, opts)` works as follows:

- `probe` is `(pageSend: number) => Promise<ServerEnvelope>` (bind `client.clockProbe`).
- `start()` sends one probe immediately and then one every 1000 ms through injected timers
  (`protocol/document.ts:Timers`).
- The reply is accepted only when `env.kind === 'clock-probe'`. Then:
  - `rtt = pageReceive - page_send - (engine_send - engine_receive) * 1000`;
  - the sample is rejected if `rtt > 100` or `rtt < 0`;
  - at most 8 samples are kept.
- On an `epoch` change the store is cleared and keeps the new sample.
- Offset from the minimum-RTT sample:
  `offset = (engine_receive + engine_send) / 2 - (page_send + rtt / 2) / 1000`.
- `at(pageMs)` returns `pageMs / 1000 + offset - (latency_seconds ?? 0)`, with uncertainty
  `rtt / 2000 + (uncertainty_seconds ?? 0)` and the reply's `latency_kind` as provenance.
- `at()` returns null when no sample is younger than 3000 ms.
- `dispose()` cancels the timers and ignores late replies.
- Probe failures (busy or closed) are swallowed and counted in `stats.failures`.

### TASK-002: Highlight timing (code/highlight.ts)

- Add the optional `HighlightOptions.audible?: AudibleClock` and `epoch?: () => string | null`.
  When `audible` is set:
  - `tick(frameMs?)` uses `audible.sample(frameMs ?? performance.now())`;
  - an invalid sample applies `[]` (hide) and keeps the entries;
  - an entry is active while `time <= t < end`, with `end = end_time ?? time + durSeconds(dur, tempo at receipt)`;
  - entries whose `ev.epoch` is set and differs from `epoch()` are dropped at receipt and at
    tick;
  - entries with `time > t + 2` are dropped at receipt and counted in `stats.horizonDrops`;
  - `MAX_ENTRIES` stays at 4096.
- Without `audible`, the existing `TimeAnchor` path stays byte-for-byte behaviorally identical,
  so legacy tests keep passing.
- Never advance by counting frames. Expired entries are removed and never re-applied.

### TASK-003: Transport bar (code/transport.ts)

- Add the optional `TransportOptions.audible?` and `sample?: () => TransportSample | null`
  (`store.transportSample`). With both set, the position comes from the latest sample in the
  current epoch with `sample_time <= t`; otherwise the earliest sample is used, with backward
  extrapolation of at most 1 s:
  - running: `cycle = s.cycle + (t - s.sample_time) * bpm / 60 / beats_per_cycle`;
  - paused (`running: false`): hold the cycle.
- When the sample age `t - s.sample_time` is over 2 s, or the sample is invalid, set
  `data-sync="hidden"` on the position element and show no beat.
- The beat flash (`data-beat-flash="on"`) shows only while `t - beatStart` is in [0, 0.08).
- Without `audible`, the existing `tempoAt` extrapolation stays as it is.

### TASK-004: Wiring (code/mount.ts, app/main.ts, protocol types)

`code/mount.ts`:

- Pass `deps.audible` and `epoch: () => store.transportSample?.epoch ?? null` to
  `HighlightScheduler`, and `audible`/`sample` to `TransportBar`.
- The rAF loop calls `highlight.tick(frameMs)` and `transport.tick(frameMs)` with the rAF
  timestamp argument.
- Do not change anything else in this file.

`app/main.ts` `boot()` uses `audibleFor` from `app/clock.ts`:

- Browser: `audibleFor('browser', { ctx })`.
- `?session=`: `audibleFor('native', { client })`.
- Set `deps.audible = result.audible`, call `result.start()` after `createEditor`, and wrap the
  returned `Editor.dispose` so that it calls `result.dispose()` first.
- CANVAS-SHELL reuses `audibleFor('native', ...)` for the Tauri tier.

Protocol types:

- `LevelsBody` gains `time?: number` and `epoch?: string`.
- In `envelope.ts` `levels`, reject a non-finite or negative `time` and an `epoch` that fails
  `epoch()`.

### TASK-005: Tests

`editor/test/canvas/clock.test.ts` uses `SimulatedTime` from `test/support/clock.ts`: page ms
and context seconds advance together, and `getOutputTimestamp` is driven by those values.

| Situation | Expected outcome |
|-----------|------------------|
| Output timestamp `{contextTime: 10, performanceTime: 5000}`, `pageMs = 5016`, `outputLatency = 0.05` | `time = 10.016`, provenance `measured`; latency not subtracted |
| No `getOutputTimestamp`, `currentTime = 3`, `outputLatency = 0.04` | `2.96`, provenance `estimate` |
| Neither available | `currentTime`, provenance `unavailable` |
| 60 Hz frames for 5 simulated minutes, random 1-3 frame drops, a 250 ms stall every 10 s, bpm 120, beats_per_cycle 4 | Every frame's computed cycle equals the analytic value within 1e-9 |
| Same run | The flash count equals the number of beats whose [beat, beat + 0.08) window contains a frame target time; no flash for a skipped beat |
| Highlight events across a stall | The first frame after the stall shows exactly the analytically active ranges; `apply` never receives a range whose `end <= t` |
| Epoch change in samples | Pending entries of the old epoch are gone on the next tick |
| Probe RTTs 30, 10, 150 ms | Offset from the 10 ms sample; the 150 ms sample is rejected |
| No probe sample for over 3 s | `sample().valid === false`, and highlight `apply([])` is called |
| Two `sample(frameMs)` calls with the same `frameMs` | Identical objects (cached); `targetMs === frameMs + lead` |
| Probe reply with `latency_kind: 'unavailable'` and `latency_seconds: null` | `sample().valid === true`, provenance `unavailable`, no latency subtracted |
| Fake ctx with `state: 'suspended'` | `sample().valid === false`; highlight `apply([])`; beat hidden |
| Same ctx changed to `state: 'running'` | Next frame shows exactly the analytic active set (no replay) |
| `HighlightScheduler.onAccept` | Fires once per accepted event with the mapped `{from, to}`; not for an epoch-mismatched event (`stats.epochDrops` + 1) or one past the horizon (`stats.horizonDrops` + 1) |
| 4,100 events received | `stats.overflow === 4` |
| `TransportBar.state` after a tick inside a beat window | `beatFlash: true`, `cycle` is the analytic value; stale sample gives `hidden: true`, `cycle: null` |
| `ProbeCorrelation.stats` after replies of 10 ms, 150 ms and a rejected promise | `{ samples: 1, rejected: 1, failures: 1 }` |

Boot wiring tests in `editor/test/app/main.test.ts` (new rows; the existing `createEditor` and
`tierFromUrl` assertions stay unchanged):

| Situation | Expected outcome |
|-----------|------------------|
| `audibleFor('browser', { ctx })`, where the fake ctx gives a valid `getOutputTimestamp` | `audible.sample(f).provenance === 'measured'` |
| `audibleFor('native', { client, timers })` then `start()` | `client.clockProbe` called once immediately and again after 1000 ms of fake timers |
| Same, then `dispose()` | No further `clockProbe` calls after advancing fake timers 5000 ms |
| `boot()` with `?session=` (`SocketTransport` mocked through `vi.mock('../../src/protocol/socket')`) | `editor.deps.audible` is set; disposing the editor stops probe calls |

Also:

- `test/canvas/contracts.test.ts`: levels with `time: 1.5, epoch: 'e1'` decode ok; `time: -1`
  is rejected.
- `test/code/transport.test.ts`: mounted with `deps.audible` and a stale sample gives
  `data-sync="hidden"`.

## Pitfalls (a careless implementation gets these wrong)

- Double-subtracting latency in the `measured` path.
- Using `performance.now()` inside `sample()` instead of the `frameMs` argument. That breaks the
  same-frame identity guarantee.
- Moving `deps.clock` to audible time. It must stay processing time.
- Replaying expired highlights after a stall. `apply` must only ever receive currently active
  ranges.
- Deleting the legacy `TimeAnchor` path or the decoration exports. Other tests and
  `code/mount.ts` still use them until CANVAS-MOUNT.
- Renaming the probe wire fields (see the reconciliation above).
- Before any code change, read `editor/src/protocol/wasm.ts`, `editor/worklet/host.js` and
  `src/host/wasm/session_half.rs:209` to confirm that browser `playing` and `TransportSample`
  times use the AudioContext processing-time domain (the value passed to `session_tick`).
  Record the evidence in the progress log. If it is a different domain, stop and report a
  blocking design contradiction.

## Verification (run from the repository root; record the exit codes in tmp/canvas-cutover/clock/checks.log)

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 (provides the wasm artifact for vitest) |
| `cd editor && npm run check` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/canvas/clock.test.ts test/code/highlight.test.ts test/code/transport.test.ts test/canvas/contracts.test.ts test/code/reconcile.test.ts test/app/main.test.ts` | all pass |
| `cd editor && ./node_modules/.bin/vitest run` | all pass (full suite; at least 590 tests, none removed) |

## Overwrite and Drift Protocol

- Before editing each file: read it fresh and record its sha256 in `tmp/canvas-cutover/clock/intent.json`.
- After editing: record the post-edit sha256 in `receipt.json`.
- If a pre-hash differs from the base commit for a file you have not yet edited, another writer
  touched it: stop editing that file, record the drift, and repair it serially after the join.
- Edit only this plan's progress log. Never edit other plans, `impl-plans/README.md` or lockfiles.

## Completion Criteria

- [ ] TASK-001 to TASK-005 done, with the contracts above exactly as pinned
- [ ] Time-domain confirmation recorded in the progress log
- [ ] `npm run check` exit 0; full vitest passes with no assertion deleted
- [ ] No edits outside writePaths, except recorded sharedPaths edits

## Progress Log

### Session: 2026-10-05
**Tasks Completed**: Plan authored
**Notes**: Wire field names reconciled with the existing `ClockProbeReply` contract.
