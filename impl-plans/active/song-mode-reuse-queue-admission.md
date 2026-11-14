# Song reuse queue admission implementation plan

**Plan ID**: SONG-REUSE-QUEUE
**Status**: Completed
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails), [Playback and export](../../design-docs/specs/design-song-mode.md#playback-and-export)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose

Allow a finite scheduler to submit Release, Rebind and events together before the
audio callback reaches their frame. Future generations must be justified by the
actual bounded queue and admitted reusable slot. Admission must not change live
or staged branch generations, reset private effects, or emit an applied receipt.

## Related plans

- Depends on `song-mode-branch-reuse.md` for PODs, keyed slot metadata and actual
  atomic audio-side reset and receipts.
- Consumed by `song-mode-transport.md` and later host preparation.
- Independent of `song-mode-freeze-work-accounting.md` source ownership.
- Full playback, preparation and export remain separate unfinished work.

## Exact write manifest

| Module | File | Status |
|---|---|---|
| Engine registration | `src/dsp/engine.rs` (932 observed lines) | Completed |
| Queue admission | new `src/dsp/engine/song_queue.rs` | Completed |
| Private queue tests | new `src/dsp/engine/song_queue/tests.rs` | Completed |

Only these three Rust paths and this plan may be edited by this author. Extract
the existing runtime-record and queue methods cohesively into the child before
growth; preserve ancestor visibility and existing public activation/mute methods.
Every touched Rust file remains below 1000 lines. No dependency or Git edits.
The branch reuse author exclusively owns its existing eight paths and public
integration fixture. Cross-module consumers may read its new metadata but must
not edit those paths.

## Interfaces

Existing signatures and error channels remain unchanged:

```rust
pub(super) fn song_runtime_record(
    &mut self, command: SongCommand, acks: &mut AckProducer,
) -> bool;
pub(super) fn queue_song_runtime(
    &mut self, command: SongCommand,
) -> Result<(), SongRejectCode>;
```

Methods relocated under a child use explicit engine-ancestor visibility as
required by actual callers. Rebind is a timed runtime record. No new public API,
unbounded collection or allocation in the callback.

## Admission requirements

- Preserve current preparation-ready/cancelling, activation uniqueness, past
  frame, malformed POD and bounded queue capacity checks.
- Ordinary branches retain existing same-generation admission behavior.
- For a reusable branch, start from the authentic staged/live full branch key
  and generation. Inspect actual queued transitions in their established frame,
  rank and FIFO order up to the proposed event. Reject failed queued commands.
- Each generation advance requires that same epoch/branch's valid Rebind with
  expected generation equal to the projected current generation, checked next
  generation and the admitted keyed slot's last-generation bound. No gaps,
  wraps, duplicate transitions, foreign branch or epoch authority.
- Event proof through each queued Rebind requires a projected matching Release whose tail deadline has been
  reached, or an actual ended branch with its existing deadline reached. Respect
  release timing and each generation's transition/deadline. Do not assume a
  queued reset succeeds physically; execution still performs all live-key,
  private users, voice, exclusivity and receipt-capacity checks.
- A queued final endpoint before the event closes the epoch and forbids reopen.
  Same-frame Release precedes Rebind and Rebind precedes Event by existing rank;
  same-rank commands remain FIFO. An event before its proving Rebind is refused.
- Queue admission is read-only until the existing bounded queue insertion.
  Staged/live configuration and reusable metadata do not advance at enqueue.
- Failure or pressure at execution cannot let a new-generation event render
  before an actual successful reset and BranchRebound receipt commitment.

## Tasks and dependencies

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| TASK-001 | Cohesive queue extraction and runtime whitelist | Existing engine | Completed |
| TASK-002 | Bounded future-generation admission | TASK-001, reuse metadata | Completed |
| TASK-003 | Private admission tests and joined verification | TASK-002, reuse held | Completed |

All tasks are sequential. Source author may work concurrently with the disjoint
branch-reuse source author; no Cargo until both and the freeze author are held.

## Completion criteria

- [x] Runtime ingress recognizes Rebind as timed, preserving protected cancels.
- [x] Same-frame and future-frame chains admit legitimate new events.
- [x] Missing release/rebind, wrong keys, stale/skipped/duplicate generations,
  overflow, ceiling, future transition and final endpoints are refused.
- [x] Current generation events ordered after an intervening rebind are refused.
- [x] Multiple successive births use bounded queue state without early mutation.
- [x] Failed queued transition is never accepted as proof.
- [x] Enqueue leaves staged/live generation and private history unchanged.
- [x] All source stays below 1000 lines and scoped formatting passes.
- [x] Actual Native/Arena reuse integration and old one-shot regressions pass.
- [x] Independent native/wasm builds, strict Clippy and selected tests pass.

## Progress log

### Session: 2026-10-02

Root inspected the current 932-line engine parent and actual ranked bounded queue.
The original Event preflight requires the already-staged generation and rejects
legitimate queued future births. The original runtime whitelist also omits Rebind.
This prior companion plan owns those callers explicitly; the core reuse plan's
eight paths stay unchanged. Verification remains pending and full song playback
is not claimed.

### ROOT0390 initial declarations

Private `fn timed_order(command: SongCommand) -> Option<(u64, u8)>` mirrors existing frame/rank order; candidate follows queued commands of equal rank FIFO. `fn validate_queued_song_generation(&self, command: SongCommand) -> Result<(), SongRejectCode>` projects authentic current staged/live generation through bounded queued Release/Rebind/Endpoints without mutation or allocation. Methods retain original ancestor visibility via pub(in crate::dsp). Genuine test helper `fn prepared(last_generation: u32) -> Engine` constructs measured Engine, begins/reserves actual four fullkey graphs, builds native templates/buses, stages reusable metadata and seals through production methods; no injected ready flag. Helpers event/release/rebind return actual POD commands for generation/frame/deadline. No future Event can be admitted before its proving commands are actually queued. Before intent0001; no Cargo.

### Event-proof clarification before source correction

Root clarifies that projected Release/deadline/generation-chain proof applies when admitting an Event. Rebind insertion retains normal POD, preparation, past-frame and queue-capacity validation and its actual audio-side semantic rejection/receipt. Thus Rebind→Release→Event ingress is valid when Event-time sorted projection proves Release0→Rebind1→Event3. Rebind alone cannot prove an Event or mutate generation/history. Missing physical exclusivity/reset/users/receipt proof remains at actual execution; admission never pretends a future reset succeeded. Private prepared(last_generation) helper uses0 for ordinary genuine branch staging and positive values for actual registered reusable staging.

### Already-executed Release refinement

After authentic staged/live generation equality, projected birth/deadline comes from live.config when present, since actual Release shortens runtime deadline independently of staged config. New test-only `fn advance(engine: &mut Engine, frames: usize) -> Vec<HostMsg>` constructs real EngineIo rings/cells/output and executes Engine::process; genuine Activate/Release reaches a shortened deadline before Rebind/Event admission. No ready/config/live flags are fabricated.

### ROOT0390 coherent source-ready hold

Engine methods relocated to song_queue child with explicit unchanged dsp-ancestor visibility. Runtime whitelist includes Rebind; Rebind insertion retains ordinary POD/readiness/past/capacity checks and does not require premature physical proof. Event projection scans the actual ranked/FIFO queue, authentic staged/live generation and reusable ceiling; honors actual shortened live deadline, failed outcomes, foreign identities, duplicated/skipped generation, final endpoints and no early mutation. Ordinary wrong-generation/missing branch retain StaleEpoch.

Seven private fixtures use production begin/reserve/build/stage/reusable/seal APIs with four real fullkey graph owners. Actual Activate/Release processing proves shortened deadline rather than fabricating live state; failure-outcome injection is confined to a retained-command negative. Source lengths engine858/queue183/tests401 (format may adjust final receipt). Exact scopedfmt/check0, no Cargo/live handles and no behavioral passing claims. Required actual Native/Arena core integration, no-allocation probes and independent regression matrix remain pending. Getter sources/plan unchanged and held.

### ROOT0401: Scoped queue companion completion

Independent ROOT0400 acceptance (SHA659866f5b3bd880416f18bbb28e72c0411707a8d484366835f08c0b0c2b0f3d7) verified67 command gates,1026 exact unit/integration names plus4 privacy executions =1030 distinct across the joined matrix. Matrix `/tmp/vactr-freeze-reuse-independent-004/final-results.json` SHA f34310087d469cdf8de2edadf2d68000cc25ddc8c50e81b2ea40812855c8953e; original27304 terminal0. Supplemental `/tmp/vactr-freeze-reuse-nextest-independent-001/final-results.json` SHA d405431dc16956e4eae985688518ccedd788b3077f93774d603849c4485ccbd2, foreground4f743a terminal0:18 selected PASS,0 selected skips,1934 unselected skips. Exact inventory/run identities and all102 current hashes matched. Joined totals are not child-only counts. Native/wasm/strict Clippy/scoped format/diff and relevant actual regressions passed. Earlier failed attempts and repair receipts remain retained.

Seven private queue tests actually passed; nine public Native/Arena reuse tests separately proved the integrated ranked reset/receipt/audio behavior, owner pressure, zero callback allocation/deallocation and partition invariance. Actual shortened live deadline and ordinary StaleEpoch compatibility passed. Rebind insertion is nonmutating; future Event admission relies on authentic queued Release/Rebind proof. Completed refers to this queue companion only, not the broader branch pool, full preparation, scheduler, playback or export. Existing failure logs retained. Sources remain held and no Cargo is run for this documentation entry.
