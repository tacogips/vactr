# LP-ENGINE: Output stop state machine, cut and clear, cell ramps, Natural release

**Status**: In Progress (implementation complete; awaiting serial integration review)
**Plan ID**: LP-ENGINE (wave 2; parallel with LP-SESSION-STOP, LP-SESSION-MOMENTARY, LP-EDITOR-STOP, LP-EDITOR-MOMENTARY)
**Design Reference**: `design-docs/specs/design-live-performance.md` 4.2, 4.3, 5.7, 7, 8.1(1)-(4), D3, D4, D8, D9
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Created**: 2026-10-08
**Last Updated**: 2026-10-08

## Intent and Context

Both hosts run the same `dsp::Engine::process` (src/dsp/engine.rs). This
plan adds every audio-side behavior of the design:

1. **`OutputStage`.** A fixed-size output state machine (internal phases
   Running, Draining, Cutting, ClearingCut, ClearingIdle and Idle) with:
   - a per-sample master cut gate;
   - a per-block peak meter of the final mix;
   - tail-hold and cap timers;
   - a chunked effect-memory clear;
   - deferred retirement of Retiring bus slots while tails ring;
   - `HostMsg::OutputState` reports on each wire-phase change.
2. **`CellRamps`.** A fixed 32-slot table that ramps cell values on the
   audio timeline, read through a `RampedCells` `CellRead` view, with
   `HostMsg::CellRampAck` acks.
3. **D3.** `Release::Natural` also releases scheduled voices whose gate is
   still open.

LP-CONTRACT already added the wire records (`CtlMsg::OutputStop`,
`CtlMsg::CellRamp`, `HostMsg::OutputState`, `HostMsg::CellRampAck`,
`OutputMode`, `OutputPhase`) and a no-op `record` arm in engine.rs. This
plan replaces that arm.

Today (see the design's section 2) the engine renders orbits and buses every
block forever, and has no fade, no clear, no idle state and no peak meter.

## Non-goals

- No session or scheduler change. Who sends `OutputStop` and `CellRamp`, and
  when, is LP-SESSION-*.
- No master limiter or gain change in Running (production master stage
  unchanged).
- No change to the song runtime's own fade or retire rules. The session's
  song `cutoff()` resets song branches, so this plan does not clear song
  branch memory itself (see TASK-E4).
- No per-sample consumer smoothing beyond the ramp value at each block or
  voice-start read point (design 5.7 and section 11).
- No new dependency and no Cargo profile change.

## Dependencies

- **dependsOn**: LP-CONTRACT.
- **Blocks**: LP-EVIDENCE.

## writePaths

- `src/dsp/engine.rs`
- `src/dsp/engine/output.rs` (new)
- `src/dsp/engine/render.rs`
- `src/dsp/ramp.rs` (new)
- `src/dsp/mod.rs`
- `src/dsp/voice.rs`
- `src/dsp/bus.rs`
- `src/dsp/effects/mod.rs`
- `src/dsp/ring.rs` (`EngineConfig` fields and the `CellStore` epoch accessor)
- `src/dsp/cells.rs` (only if the `Mirror` epoch accessor lives here)
- `src/host/wasm/cells.rs` (only the `ProbeMirror` `CellStore` impl, if the trait gains a method)
- `src/dsp/engine/song_runtime.rs` (read-only accessor `has_live_branches` only)
- `src/dsp/tests/dsp.rs`
- `src/dsp/tests/dsp/output_stop.rs` (new)
- `src/dsp/tests/dsp/ramp.rs` (new)
- `src/dsp/tests/dsp/release.rs`
- `impl-plans/active/live-perf-engine.md` (Progress Log only)
- Artifact roots (also in the manifest's `artifactRoots`): `target`,
  `tmp/live-perf/engine`

## sharedPaths

None. Read-only: `src/host/wire.rs` (C1 types), `src/dsp/song.rs`
(`SongGainGate`, the 64-frame ramp pattern), `src/dsp/bus/song_runtime.rs`
(`reset_song_bus`, the reset pattern), `src/dsp/alloc_probe.rs`.

## Pinned Interfaces (consumed by LP-SESSION-* only through the wire)

- `EngineConfig` (src/dsp/ring.rs) gains these fields, which are defaults in
  every constructor and are validated finite and positive:
  - `silence_peak: f32 = 1e-4`
  - `tail_hold_seconds: f32 = 0.3`
  - `drain_cap_seconds: f32 = 15.0`
  - `cut_fade_seconds: f32 = 0.005`
  - `clear_samples_per_quantum: usize = 65_536`, prorated as
    `max(1, n * value / 128)` for a block of `n` frames.
- `pub const MAX_CELL_RAMPS: usize = 32;` in src/dsp/ramp.rs.
- **Wire phase mapping** (C1):
  - Running maps to `Running`.
  - Draining and ClearingIdle map to `Draining`.
  - Cutting and ClearingCut map to `Cutting`.
  - Idle maps to `Idle`.

  A report is pushed only when the *wire* phase changes, and it carries
  `frame = self.frame` at the transition.

## Tasks

### TASK-E1: Natural release extension (`src/dsp/voice.rs`)

- In `SlotGens::control`, the arm `Release::Natural` (voice.rs about
  :919-923) changes:
  - **Before:** it calls `v.release()` only when `v.open`.
  - **After:** it calls `v.release()` when `v.open`, or when the scheduled
    gate is still open (the gate-countdown field set in `Voice::start`,
    which is greater than 0).
- A voice already in release is not touched again, so its envelope never
  re-enters the release stage.
- **Tests** (`src/dsp/tests/dsp/release.rs`):
  - **Strengthened:** `natural_releases_open_voices_and_panic_gates_them`
    keeps every existing assertion, and adds: a scheduled voice with a long
    legato gate (2 s) receives Natural at 0.1 s, enters release (its
    envelope stage is release, or its output decays from that point), and
    ends well before the 2 s gate.
  - **New control row:** a scheduled voice whose gate has already closed is
    not re-released (its output is bit-identical to a run without the
    control).

### TASK-E2: Engine config fields (`src/dsp/ring.rs`)

- Add the five fields above to `EngineConfig`, with their defaults in
  `EngineConfig::new` and in every struct literal in the crate.
- `cargo build --all-targets` finds the literals.
- Validation follows the existing `valid_seconds` style. An invalid value
  gives the existing `ConfigError` variant used for state budgets.

### TASK-E3: `OutputStage` (`src/dsp/engine/output.rs`, `render.rs`, `engine.rs`)

**Fields.** `pub(super) struct OutputStage` is fixed-size and `Copy`-able
where possible. It holds:

- the internal phase enum;
- `gate: f32` and `gate_step: f32` (per sample);
- `since_frame: u64` (the Draining start);
- `below_frames: u64`;
- the `ClearCursor { orbit: usize, bus: usize, offset: usize }`;
- `last_reported: OutputPhase`;
- `pending_report: Option<(OutputPhase, u64)>`.

It is constructed in `Engine::allocate` at Running with `gate = 1`.

**Message handling.** `Record::Msg(CtlMsg::OutputStop { mode })` in
`Engine::record`:

- `Gentle`:
  - from Running, the phase becomes Draining (`since = frame`,
    `below = 0`);
  - from Draining, the timers restart;
  - from Idle, ClearingIdle, Cutting or ClearingCut, nothing changes, so a
    cut is never downgraded.
- `Cut`:
  - from any phase except ClearingCut, the phase becomes Cutting, with
    `gate_step = gate / max(1, round(cut_fade_seconds * sr))`;
  - from ClearingCut, nothing changes.

**`render()` hooks** (src/dsp/engine/render.rs):

- *Before the orbit and bus rendering:* when the phase is Idle, ClearingIdle
  or ClearingCut, skip `orbits` rendering, `buses.render` and the song
  branch rendering for this block, and zero `mix_l`, `mix_r`, `mix_3` and
  `mix_4`. Voices still render into bus inputs; that input is discarded.
- *After the final mix and before writing `out`:*
  - Apply the gate per sample to every channel. A Running gate of exactly
    1.0 must be a skipped multiply, so Running output stays bit-identical.
  - Compute `peak = max |sample|` over the block, after the gate.
  - Advance the state machine for this block:
    - **Draining:**
      - when `peak < silence_peak`, no voice is active
        (`pool.voices.iter().all(|v| !v.active)`) and
        `song_runtime.as_ref().is_none_or(|r| !r.has_live_branches())`,
        `below += n`; otherwise `below = 0`;
      - when `below >= round(tail_hold_seconds * sr)`, the phase becomes
        ClearingIdle;
      - when `frame - since >= round(drain_cap_seconds * sr)`, the phase
        becomes Cutting.
    - **Cutting:** when the gate reaches 0 (clamped at 0.0, never
      negative), the phase becomes ClearingCut.
    - **ClearingCut / ClearingIdle:** run one clear step with this block's
      budget. When the cursor is done, the phase becomes Idle. ClearingCut
      sets the gate to 0 and keeps it there until the clear is done.
    - **ClearingCut fade-in:** after a ClearingCut finishes, if a voice was
      admitted during it, the phase becomes Running with the gate ramping
      0 to 1 over `cut_fade_seconds`. Otherwise the phase becomes Idle.
- *Admission:* `Engine::start` (render.rs about :232), and the song voice
  start in the song runtime path, call `self.output.admit()`:
  - from Draining, ClearingIdle or Idle: the phase becomes Running, the
    gate becomes 1 and no reset happens;
  - from ClearingCut: a fade-in is flagged (handled above);
  - from Cutting: nothing changes; the cut continues.

**Reporting.** A wire-phase change sets `pending_report`. **Every received
`OutputStop` also sets `pending_report` to the wire phase that holds after
it is handled, even when that phase did not change.** This report is the
acknowledgement that LP-SESSION-STOP uses to stop re-sending. Tests
therefore compare **de-duplicated** phase sequences. In
`process_channels`, after `retire`, try `io.acks.push(HostMsg::OutputState { .. })`:

- on success, `last_reported` takes that phase;
- on `Err`, keep `pending_report` and retry next callback. This is the
  `song_ack` retry pattern (engine.rs about :409-420).

Never drop a transition silently.

**Retiring bus slots.** While the phase is Draining or ClearingIdle,
`BusGraph::collect` must not free a legacy `Retiring` slot whose
`users == 0`. Do this with a `pub(crate) fn hold_retiring(&mut self, hold: bool)`
flag on `BusGraph` (src/dsp/bus.rs). Once the phase leaves those states,
`collect` frees the slots normally.

### TASK-E4: Bounded clear primitives (`src/dsp/bus.rs`, `src/dsp/effects/mod.rs`)

- `OrbitDelay::clear_memory(&mut self, from: usize, max: usize) -> usize`
  zeroes up to `max` samples of its delay memory, starting at `from`, and
  returns the count zeroed. On reaching the end, it resets the delay-line
  read and write positions and the send accumulators.
- `BusSlot::clear_memory(&mut self, from: usize, max: usize) -> usize` does
  the same over the slot's carved memory region:
  - It uses the in-use prefix if the slot records one; otherwise it zeroes
    the whole slot `mem`.
  - On reaching the end, it calls `FxUnit::clear_state` for the room reverb
    and every chain unit, and zeroes `l` and `r`.
- `FxUnit::clear_state(&mut self)` (src/dsp/effects/mod.rs) resets
  `FxState` and sets `vals` to `target`, keeping the parameters. It does
  **not** re-run `configure` and does not allocate.
- **Cursor order:**
  1. every orbit in index order;
  2. then every legacy bus slot whose state is Live or Retiring, in index
     order (skip `song_key.is_some()` slots: song branches are reset by the
     song cutoff path);
  3. then done.
- **Budget:** the clear step consumes at most this block's budget, summed
  across calls.

### TASK-E5: `CellRamps` (`src/dsp/ramp.rs`, `engine.rs`, `render.rs`)

**Type.** `pub struct CellRamps` holds:

- `slots: [RampSlot; MAX_CELL_RAMPS]`, where `RampSlot` is
  `{ cell, epoch, seq, from, target, start: u64, frames: u32, release: bool, live: bool }`;
- `by_cell: Box<[u8]>`, sized to the engine's cell capacity at
  construction (`0xFF` means none).

**`apply`.** The signature is
`apply(&mut self, cell, epoch, seq, target, frames, release, frame: u64, current: f32) -> bool`:

- **Epoch check.** If the cell store reports a live epoch for `cell`
  (browser `Mirror`: add `fn live_epoch(&self, cell) -> Option<u32>` to
  `CellStore`, returning `None` for `AtomicCells` and `ProbeMirror`) and it
  differs from `epoch`, the message is inert: no slot changes and no ack.
- **Slot.** Reuse the cell's slot if it has one, else take a free slot.
  If no slot is free, the message is inert and a counter
  (`counters.dropped`) increments; there is no ack, so the session
  re-sends.
- **Values.** `from = current`, which is the current effective read from
  the `RampedCells` view, so a retarget never jumps. `start = frame`.
- **Ack.** Push `HostMsg::CellRampAck { cell, seq }`.

**Value at a frame.**
`value_at(f) = from + (target - from) * min(1, (f - start) / max(frames, 1))`.
`frames == 0` means `target` immediately.

**Reaping.** At each block start (`reap(frame)`), free the slots where
`release && f >= start + frames`.

**Drop.** `release && frames == 0` frees the cell's slot immediately at
`apply` and acks. Reads then fall through to the cell at once. The session
uses this to drop an override before a cell retires or is re-used; this
matters natively, where no `CellRetire` reaches the audio side.

**`CellRetire`.** On `Record::Msg(CtlMsg::CellRetire { cell, .. })` (the
browser path), also drop that cell's ramp slot.

**`RampedCells<'a, C: CellRead + ?Sized>`** holds
`{ ramps: &'a CellRamps, inner: &'a C, frame: u64 }`. It implements
`CellRead`: `get` returns the ramp value at `frame` when the cell has a live
slot, else `inner.get(cell)`.

**Wiring in `Engine::block`** (render.rs about :332-383):

- Build the view once per block with `frame = self.frame` (the block start)
  and pass it to every `start` call and to `render`.
- **Borrow pitfall:** `render` takes `&mut self`. Move the table out with
  `std::mem::take(&mut self.ramps)` and restore it after the block. This is
  valid because `CellRamps: Default` with an empty boxed slice, which does
  not allocate. Prove it with the alloc probe.
- Do not clone the table, and do not allocate a new box per block.

## Key Points a Careless Implementation Gets Wrong

- **Running stays unchanged.** With no `OutputStop` received, every
  existing DSP, render and golden test must stay bit-identical:
  - the gate multiply is skipped at exactly 1.0;
  - the peak scan does not alter samples;
  - `hold_retiring(false)` is the default.
- **No allocation in the callback.** Every new test renders through
  `Rig::step`, which arms the alloc probe and asserts 0.
- The clear must never touch song-owned bus slots (`song_key.is_some()`).
  It must never zero more than the budget per block.
- The cut gate must reach exactly 0.0 and stay there during ClearingCut.
  The output in ClearingCut and Idle is exact zeros, not denormals.
- Do not convert `Release::Natural` into a short gate, and do not touch the
  `Panic` or `None` semantics or the late-start recovery rule.
- **Files near the limit:** voice.rs is 946 lines, engine.rs 883, bus.rs
  882 and ring.rs 895. Put new logic in output.rs and ramp.rs. If a touched
  file would reach 1000 lines, split it by moving a cohesive block into a
  new submodule (for example `src/dsp/bus/clear.rs`), and record the new
  path in the Progress Log as a deviation.
- Run rustfmt on the touched files only.

## Tests to Add (input -> expected)

Use `NativeRig` and `BrowserRig` from src/dsp/tests/dsp.rs at 48 kHz with
128-frame blocks.

**`src/dsp/tests/dsp/output_stop.rs`:**

- *Gentle tail continues and decays.* Install a bus with `room` (or use an
  orbit delay with feedback 0.9), start one short voice, then post
  `OutputStop{Gentle}` 20 ms after onset:
  - the output after the stop is non-zero for at least 100 ms;
  - the per-50 ms peak decreases (monotone non-increasing within 1e-6);
  - the phase reaches Idle;
  - the reported sequence is `[Draining, Idle]`;
  - after Idle every sample is exactly 0.0.
- *Hold time.* Idle is reached no earlier than
  `tail_hold_seconds * sr` frames after the first below-threshold block
  with 0 voices.
- *Cap.* A self-sustaining fixture (orbit delay feedback 0.95 with a
  constant input, or `drain_cap_seconds` set to 0.2 s in the config with a
  long reverb) gives:
  - reports `[Draining, Cutting, Idle]`;
  - the Cutting entry at `since + cap` frames, within one block.
- *Cut.* Post `OutputStop{Cut}` during sustained output:
  - within the fade window `|out[n]| <= g(n) * P`, where `P` is the
    previous block's peak and `g` is the linear gate;
  - every sample after `cut_fade_seconds * sr` frames is exactly 0;
  - after Idle, every orbit and legacy bus slot memory sample is 0.0
    (inspect through a `#[cfg(test)]` accessor);
  - the phase reports `[Cutting, Idle]`.
- *Cut beats gentle.* A Cut followed by a Gentle in the same block stays on
  the cut path.
- *Restart during tail.* Render A: gentle stop, then a new voice at frame
  R. Render B: the same without the new voice:
  - A and B are bit-identical for every frame before the new voice's start
    frame;
  - A's phase returns to Running;
  - A reports `[Draining, Running]`.
- *Restart from Idle.* After Idle, a new voice gives Running. Its output
  contains no pre-stop tail: it matches a fresh-engine render of the same
  voice, within 1e-6.
- *Restart during ClearingCut.* A voice admitted during the clear gives
  exact zeros until the clear completes, then a 0-to-1 fade over
  `cut_fade_seconds`.
- *Retiring slot held.* A bus re-installed (old slot Retiring) and then a
  gentle stop: the old slot keeps rendering until the phase leaves
  Draining, and is collected after it.
- *Parity.* The native rig and the browser rig produce identical output and
  phase sequences for the gentle and cut fixtures.

**`src/dsp/tests/dsp/ramp.rs`:**

- A `CellRamp` from 0 to 1 over N = 4800 frames, read at successive block
  starts through a voice `Src::Cell` input or a template-default cell:
  - the reads are monotone;
  - each block step is at most `B / N + 1e-6`;
  - the final read is exactly 1.0.
- A retarget mid-ramp starts from the current value: the first read after
  the retarget differs from the previous read by at most one block step.
- `release: true` with `frames = 480`: after the end frame the reads equal
  the underlying cell value, and the slot is free (a capacity probe shows
  32 free slots).
- Browser rig, stale epoch: no change and no ack. Matching epoch: an ack
  with the same seq.
- With 33 distinct cells the 33rd ramp is inert and `dropped` increments.
- `CellRetire` drops the ramp: later reads return the mirror value.
- `frames = 0` with `release: false` gives the target at the next read and
  holds it.
- `frames = 0` with `release: true` drops the override immediately: the
  next read equals the cell, and the slot is free.
- An `OutputStop{Gentle}` received while Idle produces one `OutputState
  {Idle}` acknowledgement, and the phase does not change.

**`src/dsp/tests/dsp/release.rs`:** TASK-E1 rows.

Register the new modules in src/dsp/tests/dsp.rs.

## Verification (exact commands; all must exit 0)

1. `CARGO_TERM_QUIET=true cargo build --all-targets`
2. `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
3. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
4. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/dsp::tests::dsp::(output_stop|ramp|release|bus|cut_group|effects|cells)/)'`
   passes with testsRun > 0.
5. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/dsp::tests|host::tests::e2e::templates|host::native::tests/)'`
   passes. This proves Running behavior is unchanged, including the golden
   digests.
6. `rustfmt --edition 2021 --check <each touched .rs file>`

Write logs to `tmp/live-perf/engine/*.log`.

## Completion Criteria

- [x] TASK-E1 to TASK-E5 are implemented, and the implemented output, ramp,
  release and integration rows pass.
- [x] Existing DSP, golden and native tests pass with no assertion weakened
  (verification 5).
- [x] The alloc probe is armed in every new render test, with 0
  allocations.
- [x] Every touched file is under 1000 lines.
- [x] Verification 1-6 exit 0. The Progress Log is updated.
- [x] Post-clear effect initialization restores each carved FxState layout
  while preserving configured targets; Cut/Gentle restart wet tails match a
  fresh engine on native and browser rigs.
- [x] Clear work reinitializes at most one effect region per block and
  reports completion only after every eligible legacy slot is restored.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.

### Session: 2026-10-08 (LP-ENGINE implementation)
**Tasks Completed**: TASK-E1 through TASK-E5; deterministic output-stop,
ramp, Natural-release and browser/native parity coverage; verification 1-6.
**Verification**:
- `CARGO_TERM_QUIET=true cargo build --all-targets` — exit 0;
  `tmp/live-perf/engine/build-all-targets-final4.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  — exit 0; `tmp/live-perf/engine/clippy-final4.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  — exit 0; `tmp/live-perf/engine/build-wasm-final3.log`.
- Focused nextest command from verification 4 — 45/45 passed, exit 0;
  `tmp/live-perf/engine/focused-nextest-final4.log`.
- Broader nextest command from verification 5 — 792/792 passed, exit 0;
  `tmp/live-perf/engine/dsp-nextest-final3.log`.
- Exact-file `rustfmt --edition 2021 --check` — exit 0;
  `tmp/live-perf/engine/rustfmt-final3.log`.
- Touched Rust file line-count check — exit 0, maximum 959 lines;
  `tmp/live-perf/engine/rust-line-counts-final.log` and
  `tmp/live-perf/engine/rust-line-counts-final-check.log`.
**Notes**: The bounded wire inbox tries exact-length valid `CtlMsg` records
before song-record fallback because tags `0x1C` and `0x1D` overlap song
`GRAPH` and `BEGIN`. Config validation applies the sample-budget limit to
allocated DSP state; stop timers require finite positive durations, so the
15-second drain cap remains valid at 192 kHz. The serial integration review
and its required post-Rust-modification verifier remain downstream.

### Session: 2026-10-08 (LP-ENGINE assertion self-check and final verification)
**Tasks Completed**: Closed the assigned test assertions for tail decay,
cut-gate bounds, output-memory clearing, restart prefix parity and the
480-frame cell-ramp release; reran the full plan verification on stable
source.
**Verification**:
- `CARGO_TERM_QUIET=true cargo build --all-targets` — exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck4-build-all-targets.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  — exit 0; `tmp/live-perf/engine/verify-agent-selfcheck4-clippy.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  — exit 0; `tmp/live-perf/engine/verify-agent-selfcheck4-build-wasm.log`.
- Focused plan nextest — 47/47 passed, exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck4-focused-nextest.log`.
- Broader DSP/native/template nextest — 794/794 passed, exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck4-broad-nextest.log`.
- Exact-file rustfmt — exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck4-rustfmt.log`.
- Touched Rust line counts — exit 0, maximum 971 lines in `src/dsp/bus.rs`;
  `tmp/live-perf/engine/verify-agent-selfcheck4-line-counts-final.log`.
**Resolved exploratory attempts**: The first focused run stopped at compile
time because the test-only accessor used an unimported module name
(`focused-nextest-selfcheck.log`). The next run found two assertions that
needed refinement (`focused-nextest-selfcheck-rerun.log`): a sub-micro
boundary sample and orbit send accumulators that remain populated while
voices are intentionally discarded during Idle. Follow-up edge runs isolated
the memory detail (`selfcheck-edge-tests.log` and `memory-diagnostic.log`).
The final tests check persistent orbit ring memory and exact zero after the
gate transition. The initial line-count checker used the wrong awk input and
exited 1 (`verify-agent-selfcheck4-line-counts.log`); the corrected checker
passed as recorded above. None of these exploratory failures remain on the
final source.
**Notes**: Every new render assertion uses the allocation-probed rig. The
remaining serial integration review is downstream; this plan's implementation
and required behavioral verification are complete.

### Session: 2026-10-08 (LP-ENGINE exact cut-gate boundary)
**Tasks Completed**: Clamped accumulated f32 fade error after the final
configured cut step and verified exact zero output from the 5 ms boundary.
**Verification**:
- `CARGO_TERM_QUIET=true cargo build --all-targets` — exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck6-build-all-targets.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  — exit 0; `tmp/live-perf/engine/verify-agent-selfcheck6-clippy.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  — exit 0; `tmp/live-perf/engine/verify-agent-selfcheck6-build-wasm.log`.
- Focused plan nextest — 47/47 passed, exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck6-focused-nextest.log`.
- Broader DSP/native/template nextest — 794/794 passed, exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck6-broad-nextest.log`.
- Exact-file rustfmt — exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck6-rustfmt.log`.
- Touched Rust line counts — exit 0, maximum 971 lines in `src/dsp/bus.rs`;
  `tmp/live-perf/engine/verify-agent-selfcheck6-line-counts.log`.
**Resolved exploratory attempt**: The preceding exact-boundary gate revision
still emitted a `1.96e-7` sample at the 5 ms boundary. Focused and broad
commands both exposed the same test failure; complete logs are
`tmp/live-perf/engine/verify-agent-selfcheck5-focused-nextest.log` and
`tmp/live-perf/engine/verify-agent-selfcheck5-broad-nextest.log`. The final
post-subtraction clamp preserves the last linear fade sample and then sets
the following gain to zero. Both final-source suites pass as recorded above.
**Notes**: LP-ENGINE implementation and its assigned verification are
complete. Serial integration review and workflow closeout remain downstream.
**Audit command correction**: A separate final local `wc -l` checker initially
counted the aggregate `total` row as a file and exited 1
(`tmp/live-perf/engine/line-counts-final-selfcheck.log`). The corrected
predicate excludes that row and passed
(`tmp/live-perf/engine/line-counts-final-selfcheck-rerun.log`); the independent
selfcheck6 line-count gate also passed.

### Session: 2026-10-08 (LP-ENGINE-ADV-R1-FXSTATE-LAYOUT-LOST repair)
**Tasks Completed**: Reinitialized each cleared room/chain FxUnit with
`effects::init` over its own carved memory region, preserving configured
targets. Extended the clear cursor to charge one region initialization per
block and delay completion until all eligible legacy regions are restored.
Added a restart wet-tail parity regression for Cut and Gentle on NativeRig and
BrowserRig. During multi-block clearing, zeroed bus accumulators after voice
render so cut voices cannot repopulate memory while the effect layout is
restored.
**Resolved finding**: `LP-ENGINE-ADV-R1-FXSTATE-LAYOUT-LOST`; root cause was
that `FxState::default()` erased configure-time delay-line layout, and clear
must call `init` (not `configure`) to restore it. The regression now confirms
the wet delay survives both stop modes and matches a fresh engine.
**Verification**:
- `CARGO_TERM_QUIET=true cargo build --all-targets` — exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck10-build-all-targets.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  — exit 0; `tmp/live-perf/engine/verify-agent-selfcheck10-clippy.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  — exit 0; `tmp/live-perf/engine/verify-agent-selfcheck10-build-wasm.log`.
- Focused LP-ENGINE nextest filter — 48/48 passed, exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck10-focused-nextest.log`.
- Broad DSP/native/template nextest filter — 795/795 passed, exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck10-broad-nextest.log`.
- Exact-file rustfmt — exit 0;
  `tmp/live-perf/engine/verify-agent-selfcheck10-rustfmt.log`.
- Touched Rust line-count check — exit 0; all 14 files are under 1000 lines,
  with `src/dsp/bus.rs` at 999;
  `tmp/live-perf/engine/verify-agent-selfcheck10-line-counts.log`.
**Resolved exploratory attempts**: The first repair build exposed an
immutable test iterator (`verify-agent-selfcheck8-build-all-targets.log`),
then focused runs exposed an unused-region cursor boundary and accumulator
refill (`verify-agent-selfcheck9-focused-nextest.log` and
`repair-region-bound-focused-nextest.log`). The final-source 48/48 focused
and 795/795 broad runs above cover those corrections. An interrupted
selfcheck9 broad log is retained but is not verification evidence.
**Notes**: Assigned LP-ENGINE implementation and behavioral verification are
complete. Formal serial integration review and workflow closeout remain
downstream.

### Session: 2026-10-09 (LP-ENGINE per-stop acknowledgement repair)
**Tasks Completed**: Replaced the single stop acknowledgement slot with a
fixed-capacity FIFO, retained queued acknowledgements while the critical ack
ring is full, and paused control consumption when the FIFO has no free slot.
Added coverage for two acknowledgements from Cut then Gentle, unchanged
de-duplicated phase transitions, and backpressure/retry with a full ack ring.
**Resolved finding**: The prior single `pending_report` could overwrite
acknowledgements when multiple `OutputStop` messages were handled in one
callback. Every handled stop now has a retained report until delivery.
**Verification**:
- `CARGO_TERM_QUIET=true cargo build --all-targets` — exit 0;
  `tmp/live-perf/engine/verify-build-all-targets.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
  — exit 0; `tmp/live-perf/engine/verify-clippy-all-targets.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  — exit 0; `tmp/live-perf/engine/verify-build-wasm.log`.
- Focused LP-ENGINE nextest — 49/49 passed, exit 0;
  `tmp/live-perf/engine/verify-nextest-focused.log`.
- Broader DSP/native/template nextest — 796/796 passed, exit 0;
  `tmp/live-perf/engine/verify-nextest-broad.log`.
- `rustfmt --edition 2021 --check` on all 16 LP-ENGINE Rust files — exit 0;
  `tmp/live-perf/engine/step6-final-rustfmt-check.log`.
- Touched Rust line counts — exit 0, maximum 999 lines in `src/dsp/bus.rs`;
  `tmp/live-perf/engine/step6-final-line-counts.log`.
- `git diff --check` — exit 0;
  `tmp/live-perf/engine/step6-final-diff-check.log`.
**Resolved exploratory attempts**: The first regression run exposed a duplicate
Draining transition (`nextest-ack-fifo-attempt-1.log`); the backpressure test
then required corrections for a test-only ring-capacity lookup
(`nextest-ack-fifo-compile-failure.log`) and for keeping the ack ring full
(`nextest-ack-fifo-assertion-failure.log`). The final 49/49 focused run covers
the corrections. The original current-source repair audit and final gates are
documented separately from these resolved attempts.
**Notes**: LP-ENGINE implementation and required behavioral verification are
complete. Formal integration review and workflow closeout remain downstream.

### Session: 2026-10-09 (LP-ENGINE-TI-ACK-ORDER repair)
**Tasks Completed**: Merged pending phase reports and queued stop acknowledgements by
frame. Stop acknowledgements win ties, matching the phase ordering model. Unified
critical delivery consumes a report only after successful publication, preserving
it across backpressure. Added `stop_ack_and_transition_reports_stay_frame_ordered_under_ack_backpressure`
to cover a Cut acknowledgement queued while the output clear reaches Idle.
**Resolved finding**: `LP-ENGINE-TI-ACK-ORDER` (`comm-005391`). The regression
asserts non-decreasing report frames, delivery of the queued Cutting acknowledgement,
and final Idle after the 4096-entry ack ring is drained. Existing exact phase
sequence, native/browser parity, and backpressure assertions remain intact.
**Verification**:
```json
[
  {"command":"CARGO_TERM_QUIET=true cargo build --all-targets","exitStatus":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-build-all-final.log"},
  {"command":"CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings","exitStatus":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-clippy-final.log"},
  {"command":"CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm","exitStatus":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-wasm-final.log"},
  {"command":"CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/dsp::tests::dsp::(output_stop|ramp|release|bus|cut_group|effects|cells)/)'","exitStatus":0,"testsRun":50,"testsPassed":50,"failureCount":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-focused.log"},
  {"command":"CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/dsp::tests|host::tests::e2e::templates|host::native::tests/)'","exitStatus":0,"testsRun":797,"testsPassed":797,"failureCount":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-broad.log"},
  {"command":"rustfmt --edition 2021 --check src/dsp/engine.rs src/dsp/engine/output.rs src/dsp/tests/dsp/output_stop.rs","exitStatus":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-rustfmt.log"},
  {"command":"wc -l src/dsp/engine.rs src/dsp/engine/output.rs src/dsp/tests/dsp/output_stop.rs","exitStatus":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-line-counts.log"},
  {"command":"git diff --check","exitStatus":0,"outcome":"passed","log":"tmp/live-perf/engine/verify-frame-order-diff-check.log"}
]
```
**Resolved exploratory attempt**: The first ordering regression run reached the memory-clear predicate before the deferred Idle transition and passed 49/50; two bounded callbacks were added before draining. Its log is `tmp/live-perf/engine/nextest-frame-order-attempt-1.log`. The final focused and broad runs above are post-edit evidence.
**Notes**: Assigned LP-ENGINE implementation and behavioral verification are complete.
Formal integration review and workflow closeout remain downstream.
