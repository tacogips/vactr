# Canvas Cutover TELEMETRY-LEAD: Announced Playing Telemetry Within the Lookahead, With Retraction Implementation Plan

**Status**: In Progress
**Plan ID**: CANVAS-TELEMETRY-LEAD (session 303, wave 19; runs alone after CANVAS-DIAG-OFFPATH is accepted)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.16 part C; 15.3.8.4 "Playing highlights" (as amended); 15.3.8.7 Wasm-to-JS and Native IPC rows (as amended); 15.3.8.15 part D (audio-domain early-flash rule, unchanged)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-TELEMETRY-LEAD`)
**Created**: 2026-10-07
**Last Updated**: 2026-10-07

---

## Intent and Context

`playing` telemetry is published only at commit. `commit_all` (`src/sched/commit.rs:743-831`)
publishes a `PlayingEvent` when an event's onset enters `now + commit_lead`, and `commit_lead` is
0.030 s (`src/sched/runtime.rs:79`). In WebKit the batch reaches the page around the audible
time, so any main-thread task of 30 ms or more pushes a highlight past the 50 ms sync gate
(`tmp/canvas-cutover/diag-webkit-input/REPORT.md`).

Design 15.3.8.16 part C keeps commit, and so audio timing, unchanged. It adds an **announce
pass**: staged events within the lookahead (`telemetry_lead` = 0.120 s) are published early in a
new `PlayingBody.ahead` list, each with a session-unique id. Each id later ends in exactly one
confirmation (the committed event carries the same id) or one retraction
(`PlayingBody.retract`). Only `HighlightScheduler` reads `ahead` and `retract`. Every other
consumer still reads `body.events`, unchanged.

Repository context:

- `src/sched/runtime.rs`: `RuntimeConfig` (`lookahead` 0.120, `commit_lead` 0.030) and
  `Runtime::tick`, which calls `commit_all` at step (4) and has `frozen_tick`.
- `src/sched/commit.rs:743-831`: the commit loop. `self.telemetry.publish(pe, &ctls)` is at line
  807. The file is 869 lines.
- `src/sched/staging.rs`: `OccKey`, `OccRecord { key, whole, payload, onset_committed }`,
  `emittable(from, until)`, `mark_committed`, and `Lane { gen, staging, from, until, .. }` at
  line 418.
- `src/sched/slots.rs`: `Slot { id, gen, muted, lanes, .. }`.
- `src/sched/telemetry.rs`: `PlayingEvent`, `Telemetry::publish` (queue plus `hits` windows) and
  `drain`.
- `src/session/publish.rs:495-509`: drains `rt.telemetry()` into one `ServerMsg::Playing` with
  `take(4096)`, only when a subscriber has `telemetry`. `playing_wire` is at line 338.
- `src/session/protocol.rs:709-726`: `WirePlaying`, `PlayingBody`. The file is 940 lines.
- `editor/src/protocol/types.ts`, `envelope.ts:158` (`MAX_PLAYING_EVENTS = 4096`) and
  `envelope.ts:263-266` (playing validation).
- `editor/src/code/highlight.ts`: `HighlightScheduler.onPlaying`, `tick`, `onAccept`, `clear`.
- `editor/src/code/mount.ts:303-306`: routes `playing` to `highlight.onPlaying(env.body.events)`
  and `transport.onPlaying(env.body.events)`. `highlight.onAccept` feeds
  `recordOnset(perfApi, ...)` (`perf-hook.ts`).

## Non-goals

- Do not change `commit_lead`, `lookahead`, `widen`, `reduced_lead`, `send`, the staging, the
  ledger, the cells or anything sent to the audio, MIDI or OSC host.
- Do not change the `hits`/`ctrl` windows (`Telemetry::publish` keeps filling them at commit
  only).
- Do not announce live MIDI notes (`midi_in.rs`) or events without `src`. Without `src` the
  highlight cannot use an announcement, so those stay commit-only.
- Do not change the `transport.ts`, `params/*`, `app/main.ts` or self-check consumers. They keep
  reading `body.events`.
- Do not add a new envelope kind, a codec change or a dependency.
- No syntax, diagnostics or harness change.

## Ownership

writePaths:

- `src/sched/announce.rs` (new)
- `src/sched/mod.rs` (register `announce`)
- `src/sched/runtime.rs` (`RuntimeConfig.telemetry_lead`, `Runtime` field, tick call, drain
  accessor)
- `src/sched/commit.rs` (one confirmation hook at the publish site; keep the bookkeeping in
  `announce.rs` so `commit.rs` stays under 1,000 lines, per the step-3 note)
- `src/sched/telemetry.rs` (`PlayingEvent.id: Option<u64>`)
- `src/sched/midi_in.rs` (`id: None` in its `PlayingEvent` literal only)
- `src/session/protocol.rs` (three optional fields; keep it under 1,000 lines, about 10 lines
  added)
- `src/session/publish.rs`
- `src/sched/tests/sched.rs` (register `mod announce;`)
- `src/sched/tests/sched/announce.rs` (new)
- `src/session/tests/publish.rs`
- `src/session/tests/codec.rs` (the `WirePlaying`/`PlayingBody` literals gain the new fields;
  assertions unchanged)
- `editor/src/protocol/types.ts`
- `editor/src/protocol/envelope.ts`
- `editor/src/code/highlight.ts`
- `editor/src/code/perf-hook.ts`
- `editor/src/code/mount.ts` (pass `ahead`/`retract` to the highlight; wire `onRetract` to the
  perf hook)
- `editor/test/code/highlight.test.ts`
- `editor/test/canvas/clock.test.ts`
- `editor/test/protocol/envelope.test.ts` (new)
- `impl-plans/active/canvas-cutover-telemetry-lead.md` (this plan; progress log only)
- `tmp/canvas-cutover/telemetry-lead` (artifact root)
- `target` (artifact root)
- `editor/node_modules/.vite` (artifact root)

sharedPaths:

- `editor/test/wasm/canvas-clock.test.ts`
- `editor/test/app/main.test.ts`

sharedPathNotes: see the manifest. Edit these only if a row asserts the exact `playing` body
or `WirePlaying` shape, adding the new optional fields without weakening any assertion.

## Contracts and Key Points

### 1. Config and drain (`runtime.rs`)

- `RuntimeConfig.telemetry_lead: f64`, default `0.120`.
- `Runtime` owns `announcer: crate::sched::announce::Announcer`.
- `Runtime::tick` calls `self.announce_all(host_now)` right after `self.commit_all(host_now, &mut rep)`.
  `frozen_tick` does not call it.
- `pub fn announced(&mut self) -> AnnounceDrain`, with
  `AnnounceDrain { ahead: Vec<PlayingEvent>, retract: Vec<u64> }`. It drains both queues.
- Leave `pub fn telemetry()` (the committed queue) unchanged.

### 2. `announce.rs` (new): all bookkeeping lives here

- `pub const ANNOUNCE_CAP: usize = 4096;`
- `Announcer`:
  - `map: BTreeMap<AnnounceKey, Announced>`, where `AnnounceKey = (SlotId, u32 /* lane gen */, OccKey)`
    and `Announced { id: u64, time: f64, src: Option<SrcRef> }`;
  - `next_id: u64`, starting at 1 and staying below 2^53;
  - `ahead: Vec<PlayingEvent>`, bounded by `ANNOUNCE_CAP`. When it is full and undrained (as in
    `Rig` tests that never call `announced()`), no new announcement is made; `skipped`
    increments, and the record is published at commit.
  - `retract: Vec<u64>`. It is never dropped (S303-PR-L2). Its length cannot exceed the number
    of map entries ever announced and undrained, and production drains it every tick.
  - `skipped: u64` (`announceSkipped`).
- `impl Runtime { pub(crate) fn announce_all(&mut self, now: f64) }`. It runs inside
  `session_tick`, which is on the browser MAIN thread (`editor/worklet/host.js`), so its cost
  is bounded (S303-PR-03):
  1. **Reconcile: O(1) per entry, never `emittable()`.** Visit map entries in `BTreeMap` key
     order, `(SlotId, gen, OccKey)`, so entries of one slot and lane are contiguous. Resolve
     the slot and lane once per group, not once per entry. For each entry, call
     `lane.staging.get(&key)` (`staging.rs:161`). Retract (push the id to `retract`, remove the
     entry) when any of these holds:
     - the slot, lane or record is missing;
     - `rec.onset_committed`, because a published commit already removed its entry, so a
       committed entry still here was muted, `Dropped` or failed;
     - `slot.muted`;
     - the record is no longer emittable. Use the same predicate as `staging.rs:316-321`,
       evaluated directly on `rec`: `!rec.onset_bearing() || rec.whole.begin < lane.from || lane.until.is_some_and(|u| rec.whole.begin >= u)`;
     - `|clock.to_host(rec.whole.begin) - announced.time| > 1e-9`;
     - `rec.payload.src != announced.src`.
  2. **Announce.** `lead = commit_lead.max(cfg.telemetry_lead)`. If `lead <= commit_lead`, stop:
     nothing is announced and behavior is exactly today's. Otherwise, for each non-muted slot's
     lane, call `lane.staging.emittable(lane.from, lane.until)` AT MOST ONCE per lane per tick.
     Walk its keys in onset order, `continue` while `onset <= now + commit_lead`, and `break`
     as soon as `onset > now + lead`, mirroring `commit_all`'s break at `commit.rs:760`.
     Announce each record with all of these:
     - `onset > now + commit_lead` and `onset <= now + lead`;
     - `payload.src.is_some()`;
     - not already in the map.

     Each one builds a `PlayingEvent` exactly as `commit.rs:796-804` would: the same
     `slot` name, `beat = tempo.beats_at(whole.begin)`, `time = onset`,
     `dur = to_host(whole.end) - onset`, `kind = SlotKind::Pattern`, `reduced_lead = false`,
     `src`, plus `id = Some(next_id)`. Push it to `ahead` and insert it into the map.
  3. If the map holds `ANNOUNCE_CAP` entries, do not announce. Increment `skipped`; the record
     is published at commit as today.
  4. Read-only: never call `mark_committed`, `send`, `ledger.insert`, `Telemetry::publish` or a
     cell write.
- `pub(crate) fn confirm(&mut self, key: &AnnounceKey, onset: f64, src: Option<SrcRef>) -> Option<u64>`:
  - On a match (time within 1e-9 and same `src`), remove the entry and return `Some(id)`.
  - If the entry exists but differs, remove it, push the id to `retract`, and return `None`.
  - If absent, return `None`.
- `reset()` (on runtime reset, hush or stop, wherever all lanes are cleared): retract every
  entry. The reconcile also covers this on the next tick, so `reset()` is optional. Prefer the
  reconcile only if it already covers every path.

### 3. Confirmation hook (`commit.rs`)

At the publish site only (`commit.rs:796-807`), set
`pe.id = self.announcer.confirm(&(id, lane.gen, key.clone()), onset, ev.src)` before
`self.telemetry.publish`. That is one or two lines. Leave the borrow structure of the loop alone:
if `self.announcer` conflicts with the `lane` borrow, collect `(key, onset, src)` into a local
vector and confirm after the slot loop, but before `self.telemetry.publish`. Do not move the
publish call.

### 4. Wire (`protocol.rs`, `publish.rs`)

- `WirePlaying.id: Option<u64>` with `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- `PlayingBody.ahead: Vec<WirePlaying>` and `PlayingBody.retract: Vec<u64>`, both
  `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- `playing_wire` copies `e.id`.
- `tick_routed` drains `rt.telemetry()` and `rt.announced()` every tick, and emits one
  `ServerMsg::Playing` when any of the three lists is non-empty and a telemetry subscriber
  exists:
  - `events` = committed, `take(4096)`;
  - `ahead` = `take(4096 - events.len())`, sorted by `(time, slot, id)`;
  - `retract` = `take(4096)`;
  - each wire event gets the same `epoch` stamping as today.
- With no subscriber, drain and discard, as today.

### 5. Frontend

- `types.ts`: `WirePlaying.id?: number`, `PlayingBody.ahead?: WirePlaying[]`,
  `PlayingBody.retract?: number[]`.
- `envelope.ts` `playing` case. Reject the envelope when any of these holds:
  - `ahead` is present but not an array, or any element fails `validPlaying`;
  - `retract` is present but not an array of non-negative safe integers;
  - any `id` is present but not a non-negative safe integer;
  - `events.length + (ahead?.length ?? 0) > MAX_PLAYING_EVENTS`;
  - `retract.length > MAX_PLAYING_EVENTS`.
- `highlight.ts`:
  - `onPlaying(events, extra?: { ahead?: readonly WirePlaying[]; retract?: readonly number[] })`.
  - Order: retract, then events, then ahead.
  - **Retract.** Remove entries whose `id` is in the set. `stats.retracted += removed`, and
    call the `onRetract` listeners with each removed id.
  - **Events.** An event whose `id` is in `knownIds` is skipped, with
    `stats.confirmedSkipped++` and no `onAccept`/`accepted`. Others are accepted as today.
  - **Ahead.** Accept under the existing rules (file, epoch, 2 s horizon, mapping, pin). On
    acceptance set `entry.id`, add the id to `knownIds` (bounded FIFO of 4,096), increment
    `stats.aheadAccepted`, and call `onAccept` with `id`.
  - `clear()` empties `knownIds`.
  - The activation in `tick` is unchanged.
  - New `onRetract(cb: (id: number) => void): () => void`.
  - The `onAccept` payload gains an optional `id`.
- `perf-hook.ts`:
  - `OnsetRecord` gains optional `id?: number`;
  - export `retractOnset(api, id)`, which removes rows with that id from what `onsets()`
    returns (a filtered view, bounded by the ring);
  - change no member names.
- `mount.ts`:
  - `highlight.onPlaying(env.body.events, { ahead: env.body.ahead, retract: env.body.retract })`;
  - `transport.onPlaying(env.body.events)` is unchanged;
  - `setActive('playing', true)` is unchanged;
  - add `highlight.onRetract((id) => retractOnset(perfApi, id))` to `offs`.

### Patterns to imitate

- `src/sched/commit.rs:impl Runtime` (a `Runtime` impl in its own module) for `announce.rs`.
- `src/sched/tests/sched/faults.rs:telemetry_publishes_playing_events_with_provenance_and_hits`
  and `rebind.rs` (`Rig`, `run`, `run_to`, `sent`) for the Rust rows. Use
  `Rig::with(RuntimeConfig { telemetry_lead, ..Default::default() }, CapabilitySet::native())`
  for the two-config comparison.
- `src/session/tests/publish.rs:published_playing` for collecting published wire events.
- The 15.3.8.4 additive-field pattern (`LevelsBody.time`/`epoch`) for optional serde fields.

## Tasks

### TASK-TL1: `announce.rs`, config, tick call, drain, confirmation hook, `PlayingEvent.id`, `midi_in.rs` literal

### TASK-TL2: Wire fields and publisher batching (`protocol.rs`, `publish.rs`), plus the `codec.rs` and `publish.rs` test ports

### TASK-TL3: Rust rows in `src/sched/tests/sched/announce.rs` and `src/session/tests/publish.rs`

### TASK-TL4: Frontend types, validation, highlight, perf hook, mount, and their rows

### TASK-TL5: Verification and progress log

## Test Cases

`src/sched/tests/sched/announce.rs` (rig ticks every `DT`. Add a 5 ms variant if the existing `DT`
is 10 ms; state the tick used in the row. 120 bpm, 4 beats per cycle):

- *Lead.* `s [:bd :sd :hh :cp] > d1`, run 4 s. Every committed event in `rt.telemetry()` with
  `src` has a matching earlier `ahead` entry with the same id. The announce tick's
  `onset - now >= 0.100` s, asserted as the literal bound 0.100 (S303-PR-L1). *In-test
  control:* the same run with `telemetry_lead = 0.030` announces 0 events.
- *Audio unchanged.* Run 20 s with `telemetry_lead` 0.030 and again with 0.120. Then all of
  these hold:
  - `rig.sent()` is equal element by element (arrival time and `AudioEvent`);
  - `rt.telemetry()` is equal apart from `id`;
  - `input_cells().hits(d1)` is equal at the end.
- *Ordering and ids.* Within each tick's `ahead`, the order is by (time, slot, id). Ids increase
  strictly. Over the run, each id appears exactly once as a confirmation or a retraction, never
  both and never twice.
- *Rebind retraction.* A rebind applied when the boundary B is 60 ms ahead (beyond the commit
  lead, inside 120 ms): old-pattern ids announced for onsets >= B are retracted before
  `now >= B - commit_lead` and never confirmed.
- *Pass cost* (S303-PR-03). A 64-voice stack held for 4 s, with the map holding at least 100
  entries. A `#[cfg(test)]` counter in `announce.rs` (around its own `emittable()` calls)
  shows at most one call per lane per tick, with 0 calls made from the reconcile. The row
  asserts both counts. Control branch in the same test: a tick with nothing announced
  (`telemetry_lead = commit_lead`) makes 0 calls.
- *Mute retraction.* Mute `d1` after an announcement and before its commit → the id is
  retracted. Set `muted` through the field in test code, `rig.rt.slots` and `Slot.muted` being
  `pub(crate)`/`pub`, if no user-level mute form reaches the rig.
- *Tempo re-anchor.* Change the tempo after an announcement and before commit → the id is
  retracted, and the committed event has `id == None`. Apply the change through the existing
  `StagedEffect::Tempo` path (`runtime.rs:404`), for example the tempo form the evaluator
  already stages.
- *Invalidation.* `rt.invalidate(slot, span)` covering an announced, uncommitted event that
  re-stages with the same key, onset and `src` → confirmed once. If the key changes →
  retracted, then re-announced.
- *Cap.* A pattern that stages more than 4,096 events inside 120 ms gives a map of at most
  4,096, `skipped` > 0, and the skipped events are published at commit without an id.

`src/session/tests/publish.rs`:

- A literal JSON round trip of a `playing` body with `events` (one with `id`), `ahead` and
  `retract` serializes and deserializes equal.
- A tick that announces and retracts nothing emits JSON byte-identical to today: no `ahead`,
  `retract` or `id` keys.
- Batch bound: 4,000 committed plus 500 announced in one tick → `events` 4,000 and `ahead` 96.
- No telemetry subscriber → no `playing` message.
- The existing `scheduled_end_time_uses_original_seconds_and_preserves_source_revision` and
  `midi_restart_*` rows pass. Port literals only by adding `id: None`.

`editor/test/protocol/envelope.test.ts` (new): a valid body with `ahead`/`retract` passes. Each of
these fails with `bad-shape`:

- `ahead: {}`;
- `retract: [-1]`;
- `retract: [1.5]`;
- `id: 2 ** 53`;
- `events + ahead = 4,097`;
- `retract` length 4,097.

A legacy body without the new fields passes.

`editor/test/code/highlight.test.ts` and `editor/test/canvas/clock.test.ts` (simulated audible
clock, 60 Hz frames, random drops, an injected 250 ms stall):

- An `ahead` event that arrives 120 ms before `time` → no frame with `sample.time < time` shows
  it, and the first frame with `sample.time >= time` shows it.
- Its confirmation in `events` with the same id → `accepted` unchanged, no extra `onAccept`,
  `confirmedSkipped` 1, still one entry.
- A retraction before the onset → never shown, `retracted` 1, and `onRetract` called with the
  id. A retraction of an active entry → absent at the next tick. An unknown id → no effect.
- An `ahead` event with a stale epoch → `epochDrops` +1, not shown.
- A legacy batch with only `events` → identical to today. Every existing row passes unchanged.

## Pitfalls

- Announcing inside `commit_all` before commits happen would announce events that commit in the
  same tick. Run the pass after `commit_all`, and only for `onset > now + commit_lead`.
- Using `>=` instead of the 1e-9 tolerance for time equality retracts every event on float
  noise.
- Putting the ahead events into the committed queue (`Telemetry::publish`) changes `hits`
  windows and every existing telemetry test. Use the separate `Announcer` queue.
- Forgetting `skip_serializing_if` breaks byte-identical legacy JSON.
- Do not let `transport.ts` or `params` see `ahead`: pass only `body.events` to them.
- `announce_all` runs inside the browser main-thread `session_tick`, every tick. Never call
  `Staging::emittable()` per map entry, because it allocates and sorts every record of the lane
  (`staging.rs:312-326`). Never allocate or sort per entry. Use the O(1) record predicate for
  reconcile, and at most one `emittable()` per lane per tick for the announce walk, with an
  early break (S303-PR-03).
- Never drop a pending retraction, even when the queues are undrained (S303-PR-L2).
- `protocol.rs` is 940 lines. Add only the three fields.
- `commit.rs` is 869 lines. Keep logic in `announce.rs`, and split if any touched Rust file
  reaches 1,000 lines.
- No `#[allow]`/`#[expect]` to silence clippy. Dead code gets removed or `#[cfg(test)]`.

## Verification

Inside the sandbox:

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build > tmp/canvas-cutover/telemetry-lead/build.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/canvas-cutover/telemetry-lead/clippy.log 2>&1` | exit 0, 0 warnings |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/sched::tests::sched::/) \| test(/sched::tests::midi/) \| test(/session::tests::publish/) \| test(/session::tests::codec/)' > tmp/canvas-cutover/telemetry-lead/nextest-focused.log 2>&1` | exit 0; the announce rows above pass; every existing `rebind`, `faults` and `midi` telemetry row passes |
| `rustfmt --edition 2021 --check src/sched/announce.rs src/sched/mod.rs src/sched/runtime.rs src/sched/commit.rs src/sched/telemetry.rs src/sched/midi_in.rs src/session/protocol.rs src/session/publish.rs src/sched/tests/sched.rs src/sched/tests/sched/announce.rs src/session/tests/publish.rs src/session/tests/codec.rs` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 |
| `cd editor && npm run check > ../tmp/canvas-cutover/telemetry-lead/npm-check.log 2>&1` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run test/code/highlight.test.ts test/canvas/clock.test.ts test/protocol/envelope.test.ts test/canvas/mount.test.ts test/wasm test/app/main.test.ts > ../tmp/canvas-cutover/telemetry-lead/vitest-focused.log 2>&1` | exit 0 |
| `cd editor && ./node_modules/.bin/vitest run > ../tmp/canvas-cutover/telemetry-lead/vitest-full.log 2>&1` | exit 0, failureCount 0 |
| `wc -l src/sched/commit.rs src/sched/runtime.rs src/session/protocol.rs src/session/publish.rs` | each under 1,000 |
| `git diff --check` | exit 0 |

Outside the sandbox (serial; for full nextest and any browser run, take the measurement lock per
design 15.3.8.16 part D with `CANVAS-TELEMETRY-LEAD` written to `owner` and released by a trap):

| Command | Required evidence |
|---------|-------------------|
| Under the lock: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | exit 0 |
| `cd editor && npm run test:perf` (alone) | exit 0 |
| `CARGO_TERM_QUIET=true cargo check --manifest-path editor/src-tauri/Cargo.toml` | exit 0 |

Records use the mandatory format (numeric `exitStatus: 0`, `outcome: "passed"`, `testsRun > 0`,
`failureCount: 0`, `log`; details in `notes`). No mutation or negative-control commands. The
two-config "audio unchanged" row and the lead row's control branch are the only sensitivity
evidence.

## Overwrite and Drift Protocol

Record fresh-read and post-edit sha256 values in `tmp/canvas-cutover/telemetry-lead/intent.json`
and `receipt.json`. `mount.ts` was last written by CANVAS-DIAG-OFFPATH, so read it fresh. Drift
in a not-yet-edited file stops edits to that file, and repair is serial.

## Completion Criteria

- [x] `src/sched/announce.rs` holds the map, ids, queues, reconcile, announce and `confirm`.
  `commit.rs` gains only the confirmation hook.
- [x] The reconcile uses the O(1) record predicate, with no `emittable()` per entry. The
  announce walk makes at most one `emittable()` call per lane per tick, with an early break.
  The pass-cost row proves both with its control branch.
- [x] `RuntimeConfig.telemetry_lead` defaults to 0.120. `telemetry_lead <= commit_lead` announces
  nothing.
- [x] `PlayingBody.ahead`, `PlayingBody.retract` and `WirePlaying.id` are optional and omitted
  when empty; legacy JSON is byte-identical.
- [x] The lead (with control), audio-unchanged, ordering/ids, rebind, mute, tempo, invalidation,
  cap, wire round-trip, legacy-bytes, batch-bound and no-subscriber rows pass.
- [x] Envelope validation and the highlight and clock rows pass. Early-arriving events never show
  before `time`. `transport.ts`, `params/*` and `main.ts` are unchanged.
- [x] Strict clippy, focused and full nextest, rustfmt on the touched files, the wasm32 build,
  `npm run check`, full vitest, `test:perf` and the Tauri check all exit 0.
- [x] No touched file reaches 1,000 lines. No dependency change. `git diff --name-only` stays
  within writePaths plus the noted sharedPaths.
- [x] The progress log records the hashes, logs and results.

## Progress Log

### Session: 2026-10-07 (session 303 plan authoring)

**Tasks Completed**: Plan authored from design 15.3.8.16 part C (accepted by step 3,
comm-004816). Plan decisions within the design:

- only events with `src` are announced, because the highlight is the only consumer;
- `src/sched/midi_in.rs` and `src/session/tests/codec.rs` are added as writePaths for the struct
  literal fields;
- `editor/src/code/mount.ts` is added to pass `ahead`/`retract` and wire `onRetract`.

### Session: 2026-10-07 (session 303 plan revision after step 5, comm-004818)

**Tasks Completed**: Plan repaired for the step-5 findings.

- S303-PR-03: the reconcile now uses the O(1) predicate from `staging.rs:316-321` on
  `staging.get`, with no `emittable()` per entry. Entries are visited grouped by
  `(SlotId, gen)`. The announce walk makes at most one `emittable()` per lane per tick, with an
  early break (`commit.rs:760` pattern). A pass-cost counter row with a control branch and a
  main-thread pitfall were added.
- S303-PR-L1: the Lead row asserts the literal bound of 0.100 s.
- S303-PR-L2: retractions are never dropped, and an undrained `ahead` stops announcing, so
  those records are published at commit.

### Session: 2026-10-07 (session 304 Step 6 implementation)

**Tasks Completed**: TASK-TL1 through TASK-TL5 implemented and verified on the final combined
source. The `CANVAS-DIAG-OFFPATH` predecessor is accepted in the dispatch and its existing edits
in `editor/src/code/mount.ts` and `editor/test/canvas/mount.test.ts` were preserved.

- Added `src/sched/announce.rs` with bounded announcement ids, grouped reconciliation, one
  `emittable()` pass per lane, confirmations, retractions, and bounded suppression for overflow
  and changed same-key records. `commit.rs` only adds the confirmation hook; committed audio,
  telemetry, and hit windows remain unchanged.
- Added optional wire `id`, `ahead`, and `retract` fields with omission for empty legacy batches;
  publisher keeps committed events first and enforces the combined batch bounds.
- Added frontend validation and highlight routing. `transport.onPlaying`, `transport.ts`, params,
  and `editor/src/app/main.ts` remain on committed events only. Added audible-time, confirmation,
  retraction, stale-epoch, legacy, envelope, and mounted perf-onset rows.
- Final verification logs under `tmp/canvas-cutover/telemetry-lead/`: build, strict clippy,
  focused nextest (107/107), full nextest (2,828/2,828; 3 existing configured exclusions),
  rustfmt, wasm32 build, npm check, focused frontend tests, full vitest (837/837), test:perf
  (1/1, ratio 2.08), Tauri check, line counts, and `git diff --check` all pass. The full nextest
  run recorded the quiet-host precondition and held the measurement lock.
- Final source hashes are recorded in `receipt.json`; per-edit intentions are in
  `intent-frontend-001.json` and `intents.jsonl`.
- Author self-check grouped the suppressed-key cleanup by `(slot, generation)`, resolving each
  lane once and checking each key through `staging.get`; focused announce tests (8/8), check,
  strict clippy, and rustfmt pass after that correction.
- Formal review, shared documentation/index updates, archive, commit, and push remain assigned to
  downstream workflow steps.
