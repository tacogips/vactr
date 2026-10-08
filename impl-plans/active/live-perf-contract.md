# LP-CONTRACT: Live-performance wire, protocol and wiring contract

**Status**: Ready
**Plan ID**: LP-CONTRACT (wave 1; no dependencies)
**Design Reference**: `design-docs/specs/design-live-performance.md` sections 4.1, 4.5, 5.5, 5.7, 7 (wire records), D1-D2, D7-D8, D10
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Baseline**: `9ac8d1f` plus the committed design/plan checkpoint.
**Created**: 2026-10-08
**Last Updated**: 2026-10-08

## Intent and Context

The user wants two live-performance features:

- a right-drag momentary tweak with glide-back;
- gentle stop as the default, with an instant cut on hush.

Both must work in the browser host (worklet/wasm) and the native host
(CPAL). Five wave-2 plans then run in parallel, on one branch and one working
directory:

- LP-ENGINE
- LP-SESSION-STOP
- LP-SESSION-MOMENTARY
- LP-EDITOR-STOP
- LP-EDITOR-MOMENTARY

They can only do that if every shared file and data shape is pinned first.
This plan does exactly that. It adds:

- the new wire records and their codec;
- the new protocol messages, the TS types and the client methods;
- the new `StagedEffect` variants;
- the dispatch arms and call sites in the shared files (`session.rs`,
  `runtime.rs`, `publish.rs`);
- stub modules with final signatures and **behavior-preserving** bodies,
  which the wave-2 plans then fill in.

After this plan the repository must build, test green and behave
**exactly as before**, with these exceptions:

- the new message kinds are accepted and do nothing yet;
- the `hush` native stages `Cut`, whose stub behaves exactly like today's
  `Revoke(All)`.

## Non-goals

- No engine DSP behavior: no fade, clear, ramp or Natural change. That is
  LP-ENGINE.
- No scheduler stop modes or song stop. That is LP-SESSION-STOP.
- No momentary table logic. That is LP-SESSION-MOMENTARY.
- No editor UI, shortcut, gesture or renderer change. Those are
  LP-EDITOR-STOP and LP-EDITOR-MOMENTARY.
- No change to existing message encodings or existing tag values.

## Dependencies

- **dependsOn**: none.
- **Blocks**: LP-ENGINE, LP-SESSION-STOP, LP-SESSION-MOMENTARY,
  LP-EDITOR-STOP, LP-EDITOR-MOMENTARY.

## writePaths

- `src/host/wire.rs`
- `src/host/tests/contracts.rs` (wire round-trip rows only)
- `src/session/protocol.rs`
- `src/session/codec.rs` (only if `KINDS` validation lives there)
- `src/session/tests/codec.rs`
- `src/session/session.rs`
- `src/session/publish.rs`
- `src/session/mod.rs`
- `src/session/momentary.rs` (new stub)
- `src/sched/mod.rs`
- `src/sched/runtime.rs`
- `src/sched/runtime/live.rs` (new stub)
- `src/sched/ramps.rs` (new stub)
- `src/session/protocol_live.rs` (new; only used if protocol.rs would reach 1000 lines)
- `src/session/tests/mod.rs`
- `src/session/tests/live_stop.rs` (new, empty module)
- `src/session/tests/momentary.rs` (new, empty module)
- `src/sched/tests/sched.rs`
- `src/sched/tests/sched/live.rs` (new, empty module)
- `src/sched/tests/sched/ramps.rs` (new, empty module)
- `src/ns/stage.rs`
- `src/ns/evaluator.rs` (the `one_shot` list only)
- `src/ns/tests/stage.rs`
- `src/vm/natives/effects.rs` (the `hush` native only)
- `src/dsp/engine.rs` (no-op `record` arms only)
- `src/host/wasm/worklet_half.rs` (only if a non-exhaustive match needs an arm)
- `src/host/wasm/main_half.rs` (only if a match needs an arm)
- `src/host/testing.rs` (only if a match needs an arm)
- `editor/src/protocol/types.ts`
- `editor/src/protocol/envelope.ts`
- `editor/src/protocol/client.ts`
- `editor/test/protocol/client.test.ts`
- `editor/test/protocol/envelope.test.ts`
- `impl-plans/active/live-perf-contract.md` (this plan's Progress Log only)
- Artifact roots (gitignored build or test outputs, also in the manifest's
  `artifactRoots`): `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules/.vite`, `tmp/live-perf/contract`

## sharedPaths

None. Read-only references: `src/song/routing.rs` (tag table),
`src/dsp/ring.rs` (`TAG_SAMPLE_BEGIN`), `src/dsp/cells.rs` (`CellId`).

## Pinned Contracts (wave-2 plans rely on these exactly)

### C1. Wire records (`src/host/wire.rs`)

New enums, all `Copy`, `Clone`, `Debug`, `PartialEq` and `Eq`:

- `pub enum OutputMode { Gentle = 0, Cut = 1 }`
- `pub enum OutputPhase { Running = 0, Draining = 1, Cutting = 2, Idle = 3 }`

**The wire phase is the only externally visible phase.** The engine's
internal Clearing(cut) is reported as `Cutting`, and its internal
Clearing(idle) as `Draining`. This is the Step-3 review finding about
deterministic e2e transitions.

`CtlMsg` gains:

- `OutputStop { mode: OutputMode }`. Tag `0x1C`. Body: `mode u8`.
- `CellRamp { cell: CellId, epoch: u32, seq: u32, target: f32, frames: u32, release: bool }`.
  Tag `0x1D`. Body: `cell u32, epoch u32, seq u32, target f32 bits,
  frames u32, release u8`.

`HostMsg` gains, using the existing 4-word encoding:

- `OutputState { phase: OutputPhase, frame: u64 }`. Tag `0x4B`. Words:
  `[phase, frame_lo, frame_hi, 0]`, count 3.
- `CellRampAck { cell: CellId, seq: u32 }`. Tag `0x4C`. Words:
  `[cell, seq, 0, 0]`, count 2.

Codec and size rules:

- An unknown `mode` or `phase` byte decodes to `WireError` (the existing
  bad-record variant). It must never panic.
- `CtlMsg::MAX_LEN` must stay a valid upper bound: the new bodies are
  smaller than `LiveNoteOn`, so a compile-time assert or test is enough.

Before writing, verify that tags `0x1C`, `0x1D`, `0x4B` and `0x4C` are
unused:

```
grep -rn '0x1C\|0x1D\|0x4B\|0x4C' src/host src/dsp/ring.rs src/song
```

The grep must find no match before your edit. `0x1A`, `0x1B` and `0x49` are
taken by the song and sample records.

### C2. Protocol (`src/session/protocol.rs`)

- **`ClientMsg::StopAll(Empty)`:** kind `"stop-all"`.
- **`ClientMsg::Momentary(MomentaryBody)`:** kind `"momentary"`, with
  `pub struct MomentaryBody { pub file: String, pub id: u32, pub form_gen: u64, pub edit_epoch: u64, pub target: Option<WireNum>, pub ramp_ms: u32 }`.
  - `target` serializes as JSON `null` when it is `None`. Use
    `#[serde(default)]`; do **not** use `skip_serializing_if`, because the
    field must be present.
- **`KINDS`:** grows to 14, and `kind()` covers both new variants.
- **`StaleReason`:** the enum used by `StaleBindingBody.reason` gains
  `MomentaryIneligible` (wire `"momentary-ineligible"`) and
  `MomentaryCapacity` (wire `"momentary-capacity"`). Mirror both in the TS
  `StaleReason` union and in any envelope reason validation.
  `SiteWriter.onStale` (editor/src/bind/write.ts) already ignores unknown
  reasons in its `switch`; leave it unchanged.
- **`TransportSample` gains**
  `#[serde(default, skip_serializing_if = "Option::is_none")] pub output: Option<String>`.
  - The values are exactly `"running"`, `"draining"`, `"cutting"` and
    `"idle"`.
  - Absence means running.
  - Every struct literal gains the field (`grep -rn 'TransportSample {' src`):
    tests use `output: None`, and publish.rs uses C5.

### C3. Staged effects (`src/ns/stage.rs`)

Add `StagedEffect::StopAll`, the gentle stop of every slot plus the output
and the song, and `StagedEffect::Cut`, the instant cut. Keep
`Revoke(SlotKey)` unchanged; it remains the per-slot Natural / All-Panic
primitive.

- `evaluator.rs::one_shot` adds both variants next to `Revoke(_)`.
- In `src/ns/tests/stage.rs`, the formatter arm maps `StopAll` to
  `"stop-all"` and `Cut` to `"cut"`.
- The `hush` native (`src/vm/natives/effects.rs:hush`) stages
  `StagedEffect::Cut` instead of `Revoke(SlotKey::All)`.
- The `stop :x` native is unchanged.

### C4. Runtime wiring (`src/sched/runtime.rs`, new `src/sched/runtime/live.rs`, new `src/sched/ramps.rs`)

- **`runtime.rs`:**
  - declare `mod live;` next to `mod song;`;
  - `src/sched/mod.rs` declares `pub(crate) mod ramps;`;
  - add the fields
    `pub(crate) output: live::OutputTracker` and
    `pub(crate) ramps: crate::sched::ramps::RampSender`, both
    `Default`-constructed in every `Runtime` constructor.
- **`Runtime::apply` arms:**
  - `StagedEffect::StopAll => self.stop_all(ev, rep)`;
  - `StagedEffect::Cut => self.cut(ev, rep)`.
- **HostMsg drain loop** (`runtime.rs` around :612): add
  - `HostMsg::OutputState { phase, frame } => self.output.observe(phase, frame)`;
  - `HostMsg::CellRampAck { cell, seq } => self.ramps.on_ack(cell, seq)`.
- **`Runtime::tick`:** call `self.output_tick();` once per tick, after the
  host-message drain. It is a stub in `live.rs`:
  `pub(crate) fn output_tick(&mut self) {}`.
- **`live.rs` stubs**, owned by LP-SESSION-STOP in wave 2:
  - `#[derive(Debug, Default)] pub(crate) struct OutputTracker { /* phase: Option<OutputPhase>, frame: u64 */ }`;
  - `fn observe(&mut self, phase: OutputPhase, frame: u64)` stores both;
  - `pub(crate) fn wire(&self) -> Option<String>` maps the phase to C2
    strings, with `None` before any report;
  - `pub(crate) fn cuts(&self) -> u64` returns a counter of cut requests.
    The stub returns 0. LP-SESSION-STOP increments it in `cut`, and
    LP-SESSION-MOMENTARY reads it so a `hush` evaluated in code also clears
    momentary entries;
  - `impl Runtime { pub(crate) fn stop_all(&mut self, ev: &mut Evaluator, rep: &mut DrainReport); pub(crate) fn cut(&mut self, ev: &mut Evaluator, rep: &mut DrainReport); }`.

  **The stub bodies must reproduce today's behavior exactly.** Both
  `stop_all` and `cut` run `self.revoke(SlotKey::All); let now =
  self.hosts.audio.now(); self.close_live_notes(SlotKey::All, now);`,
  which is the current `Revoke(All)` arm.
- **`ramps.rs` stub**, owned by LP-SESSION-MOMENTARY:
  `#[derive(Debug, Default)] pub(crate) struct RampSender {}` with
  `pub(crate) fn on_ack(&mut self, cell: CellId, seq: u32) {}`.

### C5. Session wiring (`src/session/session.rs`, `publish.rs`, new `momentary.rs`)

- **`ClientMsg::Hush`:** calls `self.momentary_clear()` and then
  `self.stage_now(StagedEffect::Cut)`.
- **`ClientMsg::StopAll`:** `self.stage_now(StagedEffect::StopAll)`.
- **`stage_now(e)`** is the existing `revoke` body generalized: it applies
  `e` to the sink, drains, and extends the console.
- **`ClientMsg::Stop`:** still stages `Revoke(key)`. Keep `revoke` as a thin
  wrapper over `stage_now` for it.
- **`ClientMsg::Momentary(b)`:** `self.on_momentary(conn, env.seq, b)`.
- **`Session`** gains the field
  `pub(super) momentary: super::momentary::MomentaryTable`, initialized
  with `Default`.
- **`publish.rs::tick_routed`:** right after the `apply_pending` loop and
  before `self.rt.drain`, call `self.momentary_tick(host_now);`.
- **`TransportSample` literal:** `output: self.rt.output.wire()`.
- **`momentary.rs` stubs**, owned by LP-SESSION-MOMENTARY:
  - `#[derive(Debug, Default)] pub struct MomentaryTable {}`;
  - `impl Session { pub(super) fn on_momentary(&mut self, conn: u32, seq: u64, b: MomentaryBody) -> Vec<ServerMsg> { Vec::new() } pub(super) fn momentary_tick(&mut self, now: f64) {} pub(super) fn momentary_clear(&mut self) {} }`.
  - Register it as `mod momentary;` in `src/session/mod.rs`.

### C6. Engine arms (`src/dsp/engine.rs`)

In `Engine::record`, add one arm that matches both
`Record::Msg(CtlMsg::OutputStop { .. })` and
`Record::Msg(CtlMsg::CellRamp { .. })`. It is a no-op: no ack and no state.
LP-ENGINE replaces it.

### C8. Test module registration (avoids wave-2 overlap)

- `src/session/tests/mod.rs` registers `mod live_stop;` and
  `mod momentary;`. Create `src/session/tests/live_stop.rs` and
  `src/session/tests/momentary.rs`, each containing only a module doc
  comment. They are owned by LP-SESSION-STOP and LP-SESSION-MOMENTARY.
- `src/sched/tests/sched.rs` registers `mod live;` and `mod ramps;`. Create
  `src/sched/tests/sched/live.rs` and `src/sched/tests/sched/ramps.rs` with
  a module doc comment only.

### C7. Editor protocol (`editor/src/protocol/{types,envelope,client}.ts`)

**types.ts:**

- `ClientMsg` gains `{ kind: 'stop-all'; body: EmptyBody }` and
  `{ kind: 'momentary'; body: MomentaryBody }`, with
  `export interface MomentaryBody { file: string; id: number; form_gen: number; edit_epoch: number; target: number | null; ramp_ms: number }`.
- The client-kind list adds `'stop-all'` and `'momentary'`.
- `TransportSample` gains
  `output?: 'running' | 'draining' | 'cutting' | 'idle'`.

**envelope.ts:**

- `CLIENT_FIELDS['stop-all'] = {}`.
- `CLIENT_FIELDS.momentary = { file, id, form_gen, edit_epoch, ramp_ms }`
  (the `target` null-or-number check is done explicitly).
- `transport()` accepts an optional `output` that must be one of the four
  strings; any other value is rejected.

**client.ts:**

- `stopAll(): void` sends `{ kind: 'stop-all', body: {} }`.
- `momentary(file, id, formGen, target: number | null, rampMs: number): void`:
  - **Drag updates** (`target !== null`) are rate-limited per `(file, id)`
    in their own map, separate from `tweaks`. The limit is
    `TWEAK_INTERVAL_MS`, latest wins, with a trailing send, imitating
    `setTweak`.
  - **A release** (`target === null`) cancels any queued drag message and
    pending timer for that key, then sends at once. A stale drag frame must
    never arrive after the release.
  - `edit_epoch` comes from `this.document(file).epoch`.
  - `ramp_ms` is clamped to the integer range 0-10000.

## Key Points a Careless Implementation Gets Wrong

- Do not change any existing tag value or field order. The song tags
  `0x1A`, `0x1B` and `0x49` are taken.
- **Exhaustive matches.** Run `cargo build --all-targets` and
  `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
  Add a minimal arm wherever a `match` on `CtlMsg`, `HostMsg`, `ClientMsg`
  or `StagedEffect` stops compiling. Never use a wildcard `_` arm on these
  enums: it would hide the arms that wave-2 plans must handle.
- The stubs must not change behavior:
  - **Hush:** emits exactly the same `SlotControl` (Panic, every slot) as
    before.
  - **Existing tests:** pass unchanged. Run the listed suites.
- `momentary` and `stop-all` must pass the session's envelope/kind
  validation. A malformed `momentary` body must give the existing
  `bad-body` path, never a panic.
- Do not edit `src/dsp/engine/render.rs`, `src/dsp/voice.rs`,
  `src/sched/control.rs`, `src/sched/cells.rs`, `src/session/authority.rs`
  or any `editor/src/{code,bind,ui}` file. Wave-2 plans own them.
- Run rustfmt on the touched Rust files only. Never run crate-wide
  `cargo fmt`.

## Tests to Add (input -> expected)

- **`src/host/tests/contracts.rs`:**
  - every new `CtlMsg` and `HostMsg` value round-trips through
    `encode`/`decode`, including `frame = u64::MAX` and `target = -0.5`;
  - a mode or phase byte of 9 gives `Err`;
  - a truncated body gives `Err`.
- **`src/session/tests/codec.rs`:**
  - `{"kind":"stop-all","body":{}}` decodes to `StopAll`;
  - a `momentary` with `"target":null` and one with `"target":0.25`
    round-trip;
  - `momentary` missing `ramp_ms` gives a decode error;
  - a `TransportSample` without `output` deserializes to `None` and
    serializes without the key.
- **`src/ns/tests/stage.rs`:** the formatter covers `StopAll` and `Cut`.
- **`editor/test/protocol/client.test.ts`:**
  - `stopAll()` sends kind `stop-all` with body `{}`;
  - `momentary()` drag calls within 16 ms coalesce to the latest, with a
    trailing send (fake timers);
  - a release right after a queued drag sends the release, and the queued
    drag is never sent later (advance the timers by 100 ms and the kinds
    stay `[momentary(drag), momentary(null)]`);
  - `ramp_ms` 20000 is clamped to 10000.
- **`editor/test/protocol/envelope.test.ts`:** a tempo with
  `transport.output: 'draining'` is accepted; `output: 'paused'` is
  rejected; client frames `stop-all` and `momentary` validate.

## Verification (exact commands; all must exit 0)

1. `CARGO_TERM_QUIET=true cargo build --all-targets`
2. `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
3. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/host::tests::contracts|session::tests::codec|ns::tests::stage|sched::tests::sched::control|host::tests::e2e::sched_gaps/)'`
   passes with testsRun > 0.
5. `rustfmt --edition 2021 --check <each touched .rs file>`
6. `cd editor && npm run check`
7. `cd editor && ./node_modules/.bin/vitest run test/protocol`

Write logs to `tmp/live-perf/contract/*.log` and cite them.

## Completion Criteria

- [ ] C1-C7 exist exactly as pinned. `grep -n 'OutputStop\|CellRamp\|OutputState\|CellRampAck' src/host/wire.rs` shows the encode and decode arms.
- [ ] Hush, stop and every existing test behave as before (verification 4 green; no assertion edited).
- [ ] New round-trip and client tests pass.
- [ ] Verification 1-7 exit 0. Every touched Rust file is under 1000 lines
  (`wc -l`). `protocol.rs` is 946 lines today: if the additions would push
  it to 1000 lines or more, move the two new body structs into a new
  `src/session/protocol_live.rs`, re-exported from `protocol.rs`, and add
  that path to this plan's progress log as a recorded deviation.
- [ ] The Progress Log is updated.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.
