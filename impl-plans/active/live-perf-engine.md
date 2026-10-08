# LP-ENGINE: Output stop state machine, cut and clear, cell ramps, Natural release

**Status**: Ready (after LP-CONTRACT)
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

- [ ] TASK-E1 to TASK-E5 are implemented, and every listed test passes.
- [ ] Existing DSP, golden and native tests pass with no assertion weakened
  (verification 5).
- [ ] The alloc probe is armed in every new render test, with 0
  allocations.
- [ ] Every touched file is under 1000 lines.
- [ ] Verification 1-6 exit 0. The Progress Log is updated.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.
