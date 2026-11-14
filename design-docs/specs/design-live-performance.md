# Live performance controls: momentary tweak and gentle stop

This document specifies two live-performance features for the browser host
(AudioWorklet/wasm) and the native host (CPAL):

- a right-drag (iPad: two-finger drag) **momentary tweak** of a bound numeric
  literal. It changes the live value without editing the document, and glides
  back (or snaps back) on release;
- **gentle stop** as the default stop, which lets voices release and effect
  tails ring out, plus an explicit **instant cut**.

**Status**: Accepted design (author, 2026-10-08), workflow run
opus-luna-design-and-implement-review-loop-session-307.
**Supersedes, for the items named in section 9 only**: design-implementation.md
11.3 "stop / hush", 15.1.5 "Eval" (`Mod-.`) and "Transport bar", and the 15.3.8
eval bullet (`Mod-.` (hush)). Everything else in those sections stands.

## 1. Overview

The user request, with the decisions recorded on 2026-10-08:

- **A. Momentary tweak.** Left-drag on a bound number keeps its current
  behavior (`writeSite`, overlay or source-edit; editor/src/bind/drag.ts).
  Right-drag on the same numbers changes the live value. The document text
  never changes. On release the value glides back to its base over a glide
  time the user sets (default 1 s, in the controls pane). Shift held at
  release snaps it back. On iPad a two-finger drag does the same. Several
  tweaks can run at once. A re-evaluation during a tweak re-bases it.
- **B. Stop semantics.** The stop button and `Mod-.` stop gently: no new
  events are scheduled, voices release through their envelopes, and the
  bus/master effects keep running until the output is below -80 dBFS or a
  15 s cap is reached. Then the engine goes idle. `Mod-Shift-.` and the hush
  button cut instantly, with a short anti-click fade of about 5 ms, and clear
  the effect state.

Both features run in the shared `dsp::Engine` and the shared session, so the
two hosts behave the same by construction (section 7).

## 2. Baseline (repository state at 9ac8d1f)

These are the facts the design builds on. Paths are relative to the
repository root.

**Stop and hush today**

- The protocol has `hush {}` and `stop {slot}` (src/session/protocol.rs:184,
  :406-407), mirrored in editor/src/protocol/types.ts and envelope.ts. There
  is no stop-all message.
- `Session` maps `hush` to `revoke(SlotKey::All)` and `stop` to
  `revoke(slot)` (src/session/session.rs:602-612). The language natives
  `hush` and `stop :x` stage the same `StagedEffect::Revoke`
  (src/vm/natives/effects.rs:128-143).
- `Runtime::revoke` uses `Release::Panic` for `All` and `Release::Natural`
  for a single slot (src/sched/control.rs:271-317). It bumps the generation,
  clears staging, and sends `SlotControl` on the immediate class.
- On the audio side (src/dsp/voice.rs:900-930):
  - `Panic` short-gates voices (3 ms).
  - `Natural` releases only *open* (live-input) voices. Scheduled voices
    play out their whole gate.
- Neither stop nor hush touches bus, orbit-delay or master effect memory.
  The engine renders buses and orbits every block forever
  (src/dsp/engine/render.rs:151-167). There is no idle state, no master
  fade, no clear operation and no tail detection.
- When a Retiring bus slot's user count reaches 0, `BusGraph::collect`
  frees it at once (src/dsp/bus.rs:626-642). This truncates that slot's
  tail without a fade.
- The toolbar has these buttons (editor/src/ui/transport-view.tsx:199-204,
  editor/src/code/transport.ts:181-189):
  - `.vact-hush`, labelled "hush: silence everything (Mod-.)";
  - `.vact-stop-all`, labelled "stop every slot". It sends one `stop` for
    each slot the editor has seen, so slots that never sounded are missed;
  - a per-slot stop button.
- `Mod-.` sends `hush` (editor/src/code/eval.ts:86-92). Keymaps resolve
  `Mod` to Meta on Apple platforms and Ctrl elsewhere
  (editor/src/code/surface.ts:156-174).
- Song mode:
  - The engine skips song-owned voices when applying `SlotControl`
    (src/dsp/engine.rs:507-516), so hush does not silence a song.
  - A song ends through `SongCommand::Endpoints {arrangement,
    tail_deadline}`, or through `cutoff()` on replacement.
  - The song transport states are Prepared, Playing, Draining, Ended and
    Failed (src/sched/song.rs). Private branch effects fade over 64 frames
    before the tail deadline (design-song-mode.md "Routing and tails").

**Bindings and live values today**

- Each numeric site is a tweak `VarSlot` with a tier: Direct, Reeval or
  Manual (design-implementation.md 13).
- `set-tweak {file,id,form_gen,value,edit_epoch}` is the overlay path. It
  is rate-limited to one message per 16 ms per site
  (`TWEAK_INTERVAL_MS`, editor/src/protocol/client.ts:23). It is validated
  in src/session/authority.rs and written by `Evaluator::set_tweak`
  (src/ns/evaluator.rs:560).
- A Direct site reaches the audio side as a control cell (11.3/11.4):
  - natively, through `AtomicCells`;
  - in the browser, through `CellInit`/`CellBatch` to the worklet `Mirror`.
- Cells are read at voice start (event controls) or once per block
  (template-default cells, `Src::Cell` ugen inputs, effect parameters).
  Pattern parameters (`PParam::Late`) are read on the evaluator side at
  query time.
- Effect parameters move halfway to their target each block
  (src/dsp/effects/mod.rs:735-739). Nothing else is smoothed.
- `migrate` (src/ns/evaluator.rs:843) carries a slot whose value differs from
  its literal into the next generation. Any momentary layer that wrote the
  slot would therefore leak into later re-evaluations.
- The left-drag gesture works as follows:
  - Constants: `DRAG_PX = 200`, `DRAG_SLOP_PX = 3`.
  - Scaling: `toUnit`/`fromUnit` over the ParamMeta range and curve, or
    1% of `max(|start|, 1)` per pixel when there is no ParamMeta.
  - The gesture is mouse-only, button 0 only, with no modifier keys
    (editor/src/code/pointer.ts:57-76, editor/src/bind/drag.ts:63-124).
  - Non-primary pointers are ignored.
  - Nothing handles `contextmenu`.
- Overlay values render as static `binding` annotations with a label. A
  label change increments the annotation revision and rebuilds the
  background and overlay layers (editor/src/code/renderer.ts:214-270).

## 3. Decisions

Each decision resolves an intake unknown.

| ID | Decision |
|----|----------|
| D1 | There are three stop scopes: per slot (`stop {slot}`), every slot (`stop-all`, new) and the whole output. "Stop every slot" is session-side and covers every slot, including ones the editor has not seen. |
| D2 | There are two stop modes. **Gentle** is the default for the stop button, `Mod-.`, `stop-all` and per-slot stop. **Cut** is used by `hush`: the hush button, `Mod-Shift-.` and the language `hush` native. |
| D3 | `Release::Natural` now also releases *scheduled* voices whose gate is still open: they enter their envelope release stage at once (note-off). Open voices behave as before. `Panic` and `None` are unchanged. |
| D4 | Tail detection runs on the **master output**: per-block peak of the final L/R mix, including the song master, after the cut gate. The threshold is -80 dBFS (1e-4 linear), held for 300 ms with no active voice. The cap is 15 s after the gentle stop is applied. The engine reports Running, Draining, Cutting or Idle to the frontend through `TransportSample.output`. |
| D5 | Song mode follows the same two modes. Gentle stop moves the song arrangement end to "now", which is the existing Draining path, and keeps the song's own tail cap. Cut uses the existing `cutoff()` path, with branch state reset. |
| D6 | The glide time is an editor-local performer preference. It is stored in localStorage `vactr.momentary.glideMs`. Default 1000 ms, range 0 to 10000 ms, step 50 ms, where 0 means snap. It is not part of the document or the session. |
| D7 | A momentary tweak is a **separate override layer**. It is never a `set-tweak` and never writes the tweak slot, so `migrate`, persistence, commit and the left-drag overlay never see it. Only **Direct**-tier sites accept it. |
| D8 | Glide and smoothing are ramps on the **audio clock**. The engine runs a bounded cell-ramp table, and the session evaluates the same ramp function for values read on the evaluator side. JS never writes audio values once per frame. |
| D9 | Effect state is cleared by zeroing in **bounded chunks** on the audio thread while the output gate is closed. Nothing is reallocated or reinstalled. |
| D10 | The stop shortcuts are handled by **one app-level keydown handler** that matches `event.code === 'Period'`, so it works on any keyboard layout and with Shift. The code keymap's `Mod-.` binding is removed. |
| D11 | A mouse right-press during an active left-drag (a chorded button press) does not start a momentary tweak. The context menu is still suppressed if the press is on a bound site. |

## 4. Stop semantics

### 4.1 Modes and entry points

| Entry point | Message | Slot revoke | Engine output | Song |
|-------------|---------|-------------|---------------|------|
| Stop button `.vact-stop-all`, `Mod-.` | `stop-all {}` (new) | every slot, `Natural` | `OutputStop::Gentle` (Draining) | arrangement end = now (Draining) |
| Per-slot stop button, `stop :x` native, `stop {slot}` | `stop {slot}` (unchanged) | that slot, `Natural` | unchanged (Running) | not applicable (slot revocation; songs are not slots) |
| Hush button `.vact-hush`, `Mod-Shift-.`, `hush` native | `hush {}` (meaning extended) | every slot, `Panic` | `OutputStop::Cut` (Cutting) | `cutoff()` now, branch state reset |
| MIDI clock `transport_stop` (11.7) | none | per slot, `Natural` (unchanged path) | unchanged | unchanged |

- `stop-all` and `hush` both clear the editor's highlight scheduler
  (`highlight.clear()`), as hush does today.
- A per-slot stop never changes the engine output state. Other slots may
  still be playing, and the shared buses keep running, so that slot's tails
  ring out naturally.

### 4.2 Engine output state machine

`OutputState` is one fixed-size field set inside `dsp::Engine`, which both
hosts run. It lives in a new module, `src/dsp/engine/output.rs`.

```
Running --Gentle--> Draining --(peak < -80 dBFS for 300 ms and 0 voices)--> Clearing(idle) --> Idle
Draining --(15 s cap)--> Cutting
Running|Draining|Idle --Cut--> Cutting (5 ms fade to 0) --> Clearing(cut) --> Idle
Draining|Idle|Clearing(idle) --voice admitted--> Running
Clearing(cut) --voice admitted--> stays Clearing(cut); Running (5 ms fade-in) when the clear completes
```

**Running.** The current behavior, with no change in cost.

**Draining** (gentle stop applied):

- Voices, buses, orbit delays and the master chain keep rendering.
- After all processing, the engine computes the block peak
  `max(|mix_l|, |mix_r|)`. It allocates nothing and touches only the
  current block.
- A counter of consecutive below-threshold frames advances only while the
  pool has no active voice and no song branch is still Playing or Draining.
- When the counter reaches `TAIL_HOLD`, the state becomes Clearing(idle).
- While the engine is Draining, `BusGraph::collect` does not free Retiring
  slots whose user count is 0. They keep rendering their tails until the
  engine leaves Draining. This removes the tail truncation described in
  section 2 for the stop case.
- When `DRAIN_CAP` elapses, the state becomes Cutting. This covers a
  self-oscillating feedback delay or similar, which never decays.

**Cutting:**

- A master gain gate ramps linearly from its current value to 0 over
  `CUT_FADE`. The ramp is applied per sample to every output channel,
  including the stem channels.
- The voices receive `Panic` through the session's revoke, as a short gate
  that finishes inside the fade.
- When the gate reaches 0, the state becomes Clearing(cut).

**Clearing** (cut or idle):

- The output is exact zeros (gate = 0).
- A clear cursor zeroes effect memory, at most
  `CLEAR_SAMPLES_PER_QUANTUM` f32 values per 128 frames (prorated for
  other block sizes). It covers, in a fixed order:
  1. every orbit delay (memory and positions);
  2. every Live or Retiring bus slot (room reverb, chain units, block
     accumulators);
  3. song branch buses, through the existing `reset_song_bus` and
     `SongPrivateDelay::reset`.

  Each `FxUnit` is cleared over the carved memory region it actually uses.
  Its runtime state (`FxState`, smoothing values) is reset and its
  parameters are kept. This requires a new `FxUnit::clear` and an
  `OrbitDelay` memory clear; no template is re-run.
- Per-voice effect memory needs no clear: a voice zeroes its memory when it
  starts.
- Deferred `collect` frees Retiring slots with no users once the clear
  finishes.
- Clearing(idle) runs at a gate that is already below -80 dBFS, so nobody
  can hear it. If a voice is admitted during Clearing(idle), the clear stops
  and the state becomes Running. The partly cleared residue is below the
  threshold.
- Clearing(cut) does not stop. A voice admitted during the clear starts
  normally, but the output gate stays at 0 until the clear completes, then
  ramps 0 to 1 over `CUT_FADE`. In the worst case this swallows the first
  part of a note, bounded by the clear time. A note can only arrive there
  if the user re-evaluates within that window, because a hush revokes
  every slot.

**Idle:**

- Buses, orbits and the master chain are not processed. The output is
  exact zeros.
- Analyzers and levels report silence.
- Any admitted voice start (scheduled event, live MIDI note, song
  activation) returns the engine to Running. The effect state is already
  clear, so restarting from Idle cannot replay stale content.

**Restart during a tail** (a voice admitted while Draining):

- The state becomes Running. Nothing is reset, faded or skipped.
- The tail keeps decaying and mixes with the new voices, with no gain
  discontinuity. This is the glitch-free requirement.
- A gentle stop that arrives while the engine is Running again starts a
  fresh Draining period with the counter reset.

**Message ordering and merging.**

- `CtlMsg::OutputStop {mode}` travels on the same FIFO priority channel as
  `SlotControl`, after the revoke the session sends in the same tick.
- If several `OutputStop` messages arrive, Cut wins, the same way `Panic`
  wins in a `SlotControl` merge.
- While a song install is pending, `Engine::controls` already delays every
  control message (src/dsp/engine.rs:407-433). `OutputStop` inherits that
  existing bound and adds no new delay.

### 4.3 Constants

All constants below are `Runtime`/engine configuration fields. Tests read
them through those fields.

| Name | Value | Note |
|------|-------|------|
| `SILENCE_PEAK` | 1e-4 (-80 dBFS) | block peak of the final mix |
| `TAIL_HOLD` | 300 ms | consecutive below-threshold time, with 0 voices |
| `DRAIN_CAP` | 15 s | from the frame where gentle stop is applied |
| `CUT_FADE` | 5 ms (240 frames at 48 kHz) | linear, per sample; also the fade-in after a cut clear |
| `CLEAR_SAMPLES_PER_QUANTUM` | 65,536 f32 per 128 frames | bounds the clear's callback cost; the clear time scales with the carved effect memory |

### 4.4 Song mode

- A gentle stop while a song is Playing sends
  `SongCommand::Endpoints {arrangement: now + commit_lead, tail_deadline:
  arrangement + song.tail_seconds}`. This is the existing early end:
  - no song onsets at or after the new end;
  - held voices are released;
  - private branch tails run to the song's tail deadline, with the existing
    64-frame tail fade.

  The song transport reaches Draining, then Ended. The engine leaves
  Draining only after the song is no longer Playing or Draining (D4), or at
  `DRAIN_CAP`. If the cap comes first, the cut path also resets the song
  branches.
- A cut while a song is active uses `cutoff()` at the next callback frame,
  and Clearing resets the private branch and track state. The song
  transport reports Ended.
- The song code restriction stays: `hush`/`stop` remain invalid inside song
  code (src/session/song.rs:552-566). The user-level `stop-all` and `hush`
  are client messages, not song code, so they are allowed.
- Static export (`vactr render`) ignores live stop. Its tail rule ("render
  exactly through end plus tail cap") is unchanged.

### 4.5 Telemetry and frontend state

- The engine reports `HostMsg::OutputState {state, frame}` once per
  transition. Natively it goes on the ack ring; in the browser it goes from
  the worklet to the session. Bound: one record per transition, at most
  four transitions per stop.
- The session keeps the latest state and publishes it as
  `TransportSample.output: "running" | "draining" | "cutting" | "idle"`.
  The field is optional on the wire, and its absence means "running".
- The transport bar shows the state on the stop button as
  `data-output`. While the state is `draining` the stop tooltip adds
  "tails ringing out". This is styled with existing tokens only.

### 4.6 Language natives and existing tests

- The `hush` native stages a cut, the same as the client `hush`. The
  `stop :x` native stays a per-slot Natural stop.
- Existing tests that assert `Panic` for hush, `Natural` for stop, merge
  order, or immediate silence after hush keep their assertions; the
  semantics they assert are unchanged.
- Tests that assert scheduled voices play out after `stop`
  (src/dsp/tests/dsp/release.rs `natural_releases_open_voices_and_panic_gates_them`
  and src/sched/tests/sched/control.rs) are **strengthened**: they now also
  assert that scheduled voices enter release. No assertion is weakened.
- Editor tests that expect `Mod-.` to send `hush`
  (editor/test/code/eval.test.ts, editor/test/ui/controls.test.ts,
  editor/test/code/transport.test.ts) move to the new mapping. They assert
  `Mod-.` sends `stop-all`, `Mod-Shift-.` sends `hush`, and the stop-all
  body is `{}`.
- The e2e `hushQuiet` check (editor/test/e2e/measure.mjs) clicks
  `.vact-hush`, which remains the cut button. It needs no change.

## 5. Momentary tweak

### 5.1 Eligibility

A momentary tweak starts only on a **bound, Direct-tier numeric site**:

- bound means a `SiteTable` entry that is not stale;
- the eligibility rule is the same `siteAt` hit test that left-drag uses.

On a Reeval or Manual site the gesture does not start. The editor shows a
transient status hint ("momentary tweak needs a live site"). Two reasons:

- On a Reeval site, every write rebuilds the form.
- On a Manual site, nothing is heard.

Left-drag on any site is unchanged.

### 5.2 Desktop gesture

1. `pointerdown` from a mouse with `button === 2` on an eligible site:
   - `PointerController` routes it to a new `surface.momentaryDrag`
     provider registry, alongside `numericDrag`;
   - the provider is `editor/src/bind/momentary.ts`;
   - the pointer is captured, and the event is tagged so the parameter
     editors ignore it, as `vactrNumericGesture` already does.

   A right-press that is not on an eligible site keeps today's behavior,
   which is no gesture.
2. The value is scaled with the same rules as left-drag:
   - **With ParamMeta:** the gesture accumulates a unit offset
     `u = dy / DRAG_PX`, with `dy` signed the same way as left-drag.
     `target = fromUnit(clamp01(toUnit(base) + u))`.
   - **Without ParamMeta:** the offset is in value space,
     `dy * 0.01 * max(|base at gesture start|, 1)`.
   - Below `DRAG_SLOP_PX` nothing is sent.
   - Integer literals round, as in left-drag.
3. **Release** (`pointerup`):
   - without Shift: `momentary {target: null, ramp_ms: glideMs}`;
   - with Shift held: `momentary {target: null, ramp_ms: 0}` (snap).
   - `pointercancel` and `lostpointercapture` count as release without
     Shift.
4. **Context menu:**
   - A `contextmenu` listener on the code canvas calls `preventDefault()`
     only when the event position hits a bound site, or when a right-press
     gesture is active or ended within the last 500 ms. The time window
     covers platforms that fire `contextmenu` on mouseup.
   - Everywhere else the native menu appears.
5. **Chorded press:** a right-press during an active left-drag arrives as a
   chorded `pointermove`. It starts no gesture (D11).

### 5.3 Touch (iPad and other touch devices)

- A two-finger drag on a number performs the momentary tweak:
  1. The first touch pointer goes down on an eligible site.
  2. A second touch pointer goes down within `TWO_FINGER_WINDOW` (250 ms)
     while the first has moved less than the existing 8 px scroll
     threshold.
  3. The two pointers become one tweak, anchored to the first pointer's
     site.
- `dy` is the vertical movement of the pair's centroid. Lifting either
  finger releases the tweak. The release glides; Shift is honored if a
  hardware keyboard reports it on `pointerup`.
- The first finger's long-press selection timer and scroll detection are
  cancelled once the pair forms. A single finger keeps the existing tap,
  long-press and scroll behavior.
- **Multiple tweaks:** every pair of fingers is an independent gesture,
  keyed by its pointer ids. A new second finger pairs with the nearest
  unpaired first finger that qualifies.
- On desktop, independent tweaks are sequential, because a mouse is one
  pointer. Several glides can still run at once, since each release keeps
  gliding while the next right-drag starts.
- `PointerController` keeps its single-primary-pointer model for selection.
  The momentary module tracks the extra touch pointers itself.

### 5.4 Glide-time setting

- The controls pane has a "momentary glide" number field with an adjacent
  range input:
  - 0 to 10000 ms, step 50, default 1000;
  - it is placed with the slider panel controls;
  - it uses the flat, square token styles (border-radius 0);
  - it is covered by test:style.
- The value is read when a gesture is released.
- Persistence follows D6. Storage access is wrapped in try/catch, like the
  existing `vactr.*` keys.

### 5.5 Protocol

**Client to session (new):**

```
momentary { file, id: TweakId, form_gen, edit_epoch,
            target: WireNum | null,   // null = return to base
            ramp_ms: u32 }            // capped at 10000
```

- **Validation:** the same `check_tweak` as `set-tweak`. A stale form_gen
  or epoch produces the existing `stale-binding` notice, and the editor
  re-sends after re-keying. A site that is not Direct is rejected with a
  new `momentary-ineligible` notice.
- **Send rate:** while dragging, the editor sends at most one message per
  16 ms per site, latest wins, with a trailing send. This is the
  `TWEAK_INTERVAL_MS` rule. Each drag message has
  `ramp_ms = DRAG_SMOOTH` (30 ms), so an abrupt mouse jump becomes a short
  ramp instead of a step.
- **Release:** sent once and never coalesced away.

### 5.6 Session momentary table

The session keeps a new `MomentaryTable` in a new module,
`src/session/momentary.rs`. It is bounded at `MAX_MOMENTARY = 16` entries,
keyed by `TweakId`. Each entry holds:

- the site;
- a ramp `{from, to: Value(v) | Base, start_time, duration}` on the
  **audio clock**;
- a `releasing` flag.

The table behaves as follows:

- **Effective value:** the site's effective value at audio time `t` is the
  ramp evaluated at `t` while an entry exists, and the slot's base
  otherwise. The base is the tweak slot value, which includes any
  `set-tweak` overlay. The tweak slot is **never written** (D7).
- **New ramp:** a new message starts a ramp from the *current* effective
  value at the application time, to the new target, over
  `max(ramp_ms, MIN_RAMP)`. `MIN_RAMP` is 5 ms, so a snap is audibly
  instant but has no click. The value never jumps.
- **End of a release:** when a releasing ramp ends, the entry is removed.
  Reads fall back to the base.
- **Evaluator-side reads.** `VarSlot` gets a momentary field next to its
  value. `get()` returns the momentary value when it is set; a new
  `base()` returns the slot value. The momentary field covers query-time
  `PParam::Late` parameters, commit-time `Const` downgrades, MIDI/OSC
  values and visual uniforms. `migrate`, publication of `site.value`,
  commit, persistence and overlay logic use `base()`. The session updates
  the field from the ramp at most once per 16 ms per entry, and calls
  `invalidate_uncommitted` only when the value changed. That is the same
  cost bound as one `set-tweak` stream.
- **Audio-side reads.** For every cell that the site's slot feeds, the
  session sends one `CtlMsg::CellRamp` per ramp change; it never sends one
  per step. Natively the record goes on the existing SPSC control ring; in
  the browser it goes over the worklet port. The `AtomicCells`/`Mirror`
  base value is untouched.
- **Integer sites:** sites whose `NumTy` is Int, or whose ParamMeta curve
  is `stepped`, are never ramped fractionally. The session sends
  zero-length ramps (`frames = 0`) holding the rounded value, re-evaluated
  at most once per 16 ms. A fractional value never reaches an integer
  control.
- **Base changes:** when the base changes during a releasing ramp
  (`set-tweak` from a slider, MIDI CC or left-drag overlay), the session
  re-issues the ramp from the current value to the new base over the
  remaining duration.
- **Re-evaluation re-bases.** On site refresh, each entry is matched to the
  new generation with the `migrate` matching rule (index, numeric type,
  origin):
  - **Match:** the entry re-keys to the new `TweakId` and its cells.
    - A held entry keeps its absolute target until the editor re-sends.
      The editor re-sends immediately when the new site table arrives,
      recomputing the target from the new base and its stored offset
      (5.2), so the tweak follows the new base.
    - A releasing entry glides to the new base over its remaining time.
  - **No match:** the entry is dropped. Its cells retire with the old
    generation.
- **Capacity:** an entry that would exceed `MAX_MOMENTARY`, or the engine
  ramp capacity, is rejected with `momentary-capacity`. The editor caps
  concurrent gestures at 16, so this is a defensive bound.
- **Hush:** a hush drops every momentary entry and engine ramp. The cells
  return to their bases with the cut.

### 5.7 Audio-side ramp table

`src/dsp/ramp.rs` (new) holds `CellRamps`, a fixed array of
`MAX_CELL_RAMPS = 32` slots allocated at engine construction.

- **Message:**
  `CellRamp {cell, epoch, seq, target: f32, frames: u32, release: bool}`
  sets a slot as follows:
  - `from` is the slot's current value. For a cell with no slot yet, it is
    the underlying cell value.
  - `start` is the application frame.
  - The value at frame `f` is `from + (target - from) * min(1, (f - start)
    / frames)`. It is computed from frame counters, so the ramp is
    sample-accurate on the audio timeline and not quantized to ticks.
- **Epoch:** the epoch check is the `CellUpdate` rule, so a stale epoch is
  inert.
- **Release:** a `release` ramp frees its slot at its end frame. Later
  reads fall through to the cell.
- **Acknowledgement:** the engine acks with `CellRampAck {cell, seq}`. The
  session re-sends the latest ramp under the standard unacked-threshold
  rule (3 ticks). A ramp dropped on worklet slot overflow is therefore
  repaired.
- **Reads:** cells are read through a `RampedCells` view, which implements
  `CellRead` and is consulted before the `AtomicCells`/`Mirror`. Every
  existing read point keeps its granularity:
  - an event `Ctl::Cell` reads at voice start;
  - template-default cells, `Src::Cell` inputs and effect parameters read
    the ramp value at the block start.

  The per-block step is therefore bounded by `|Δ| * B / N` (B is the block
  length and N the ramp frames). Effect units still apply their existing
  per-block smoothing on top.
- **Cost:** no allocation and no lock. Each read costs one array lookup
  through a cell-indexed slot map sized to the cell pool at construction.

### 5.8 Momentary constants

| Name | Value | Note |
|------|-------|------|
| glide default / range / step | 1000 ms / 0 to 10000 ms / 50 ms | 0 means snap; editor-local (D6) |
| `MIN_RAMP` | 5 ms | the shortest audio ramp; used for a snap |
| `DRAG_SMOOTH` | 30 ms | the ramp attached to every drag update |
| send rate | 16 ms per site, latest wins, trailing send | reuses `TWEAK_INTERVAL_MS` |
| `TWO_FINGER_WINDOW` | 250 ms | touch pairing window |
| context-menu window | 500 ms after a right-press gesture | covers platforms that fire `contextmenu` on mouseup |
| `MAX_MOMENTARY` | 16 | per session; the editor caps gestures at the same number |
| `MAX_CELL_RAMPS` | 32 | per engine |

### 5.9 Live value display

- The live value shows next to the literal as ` ~ <value>`, in the same
  label style as the overlay ` = <value>`.
- The label is drawn through the renderer's **animated rows** path, as the
  new animated kind `momentary` with a label, the same way `playing` and
  `eval` are drawn. It is not a static annotation, so a moving value never
  increments `annotationsRevision`, never rebuilds text, background or
  overlay geometry, and adds no DOM or layout work.
- The editor computes the displayed value from its own copy of each ramp:
  the start time is when the message is sent on the editor clock, and the
  same formula is used. This is display only; the error is at most one
  control hop. The value is quantized to the ParamMeta step (or 3
  significant digits), and it is recomputed at most once per animation
  frame.
- Animation frames are requested only while a gesture or glide is active.
  An animation frame with no momentary activity does no momentary work.

## 6. Shortcuts and toolbar

**Shortcuts.** One app-level `keydown` listener on the window (D10) owns
both stop shortcuts:

- **Matching:** `Mod` is Meta when `navigator.platform` matches `/Mac|iP/`,
  and Ctrl otherwise. These are the same rules as `matchesKey`. The key is
  `event.code === 'Period'`, which still matches when Shift turns `.` into
  `>`.
- **Mod-. :** sends `stop-all` (gentle) and clears highlights.
  `preventDefault()` is called.
- **Mod-Shift-. :** sends `hush` (cut) and clears highlights.
  `preventDefault()` is called.
- **Removed binding:** `EvalController`'s `Mod-.` binding
  (editor/src/code/eval.ts) is removed, so a shortcut sends exactly one
  message.
- **Fallback option:** the `keyboard.ts` fallback `hush` option is replaced
  by `stopAll`/`cut` options with the same mapping (tests only).
- **Focus:** the shortcuts work whether or not the code textarea has
  focus. This matters during performance, when focus may be on the
  controls pane.

**Toolbar** (editor/src/ui/transport-view.tsx). The classes stay the same,
and the labels serve as both `aria-label` and `title`.

| Button | Label | Message |
|--------|-------|---------|
| `.vact-stop-all` (StopSquare) | "stop: stop every slot, let effects ring out (Mod-.)" | `stop-all` |
| `.vact-hush` (Hush) | "cut: silence everything now and clear effects (Mod-Shift-.)" | `hush` |
| per-slot `.vact-mute` | "stop {slot}: release its notes" | `stop {slot}` |

The stop button carries `data-output` (4.5). No new icon is needed. Styling
uses the existing tokens and border-radius 0.

## 7. Host parity and real-time safety

- All new audio behavior lives in `dsp::Engine`, which both hosts call:
  `OutputState`, the cut gate, clearing, `CellRamps`, the Natural release
  extension, deferred `collect` and the peak meter.
- Both hosts receive the same `CtlMsg` and return the same `HostMsg`.
  `host/wire.rs` encodes the new records for the browser. The native host
  sends them as `NativeRecord::Msg` on its existing SPSC rings.
- The Tauri transport passes the new JSON message kinds through unchanged.
- **New wire records:**

  | Direction | Record | Fields |
  |-----------|--------|--------|
  | to audio | `OutputStop` | `{mode: Gentle \| Cut}` |
  | to audio | `CellRamp` | `{cell, epoch, seq, target, frames, release}` |
  | from audio | `OutputState` | `{state, frame}` |
  | from audio | `CellRampAck` | `{cell, seq}` |

- **Callback invariants** (12.2) hold:
  - no allocation;
  - no lock;
  - fixed-size state only (counters, gains, the clear cursor, the 32 ramp
    slots);
  - bounded work per block (clear budget; constant-time peak and ramps).
- **Session bounds:**
  - at most 16 momentary entries;
  - at most one `CellRamp` per fed cell per ramp change;
  - evaluator updates at most once per 16 ms per entry;
  - `OutputStop` at most once per user action.
- **Native tests** use `NativeAudioHost::headless`, so they never open an
  audible device.
- **Browser tests** use the silent sink harness (post-sink peak 0).
- **File-size rule:** src/session/protocol.rs (946 lines),
  src/dsp/voice.rs (946), src/dsp/engine.rs (883) and src/dsp/bus.rs (882)
  are near the 1000-line limit. New logic therefore goes into the new
  modules named above (`dsp/engine/output.rs`, `dsp/ramp.rs`,
  `session/momentary.rs`, and a protocol submodule or sibling file for the
  new bodies). A touched file that would reach 1000 lines is split in the
  same plan.

## 8. Verification

### 8.1 Deterministic Rust tests

These use the `NativeRig`/`BrowserRig` in src/dsp/tests/dsp.rs, with the
allocation probe armed around every `process`, and scheduler tests with the
mock clock.

1. **Ramp/glide.**
   - A `CellRamp` over N frames read at successive block starts is
     monotone, ends exactly at the target, and changes by at most
     `|Δ|·B/N + 1e-6` per block. This is the no-zipper bound.
   - A new ramp issued mid-ramp starts from the current value, with no
     discontinuity.
   - A `release` ramp falls back to the cell.
   - A stale epoch is inert.
   - Integer sites never deliver a fractional value.
   - The browser and native rigs produce identical output.
2. **Gentle-stop tail.**
   - Fixture: a voice through a high-feedback orbit delay and a bus reverb.
   - After `stop-all`, the scheduled voice enters release (D3) and no new
     onset is admitted.
   - The output after the stop is non-zero, which proves the tail
     continues, and it decays.
   - The state reaches Idle after the peak stays below 1e-4 for 300 ms,
     and the output is then exact zeros.
   - A fixture with self-sustaining feedback reaches Cutting at
     `DRAIN_CAP` and then Idle.
   - A Retiring bus slot is not truncated during Draining.
3. **Instant cut.**
   - After `hush`, every output sample inside the `CUT_FADE` window
     satisfies `|out[n]| <= g(n) * P`. Here `g` is the linear gate and `P`
     is the peak of the block before the cut.
   - Every sample after the fade is exactly 0.
   - Orbit, bus and song effect memory is all zero once Clearing
     completes, within the computed number of quanta.
   - A voice admitted after the cut produces no tail content from before
     the cut.
4. **Restart during a tail.**
   - A voice admitted while Draining returns the state to Running.
   - Two renders are compared: A restarts at frame R during the tail, and
     B never restarts. They are bit-identical up to the new voice's start
     frame. This shows that admission resets, fades and skips nothing.
   - Restart from Idle and restart during Clearing(cut) follow 4.2.
5. **Session and protocol.**
   - Codec round-trip and malformed-input no-panic tests for `stop-all` and
     `momentary`.
   - `revoke` mode mapping: `stop-all` gives Natural + Gentle, `hush` gives
     Panic + Cut, and the natives map the same way.
   - `momentary` validation (stale, ineligible, capacity).
   - The tweak slot is never written: after a full tweak and glide,
     `migrate` sees an unchanged slot.
   - A re-evaluation during a held tweak and during a glide re-bases it.
   - `TransportSample.output` follows the engine transitions.
   - Song gentle stop reaches Draining, then Ended; song cut reaches Ended
     with the branch reset.

### 8.2 Frontend vitest

These use fake timers, counting fakes and no wall clock.

- **Right-drag:** sends `momentary` with ParamMeta and with relative
  scaling, respects `DRAG_SLOP_PX`, applies the 16 ms latest-wins rate, and
  never changes the document text (the doc revision is unchanged).
- **Release:** a release sends `ramp_ms = glideMs`. A Shift release sends
  `0`.
- **Context menu:** suppressed on a bound site and within the post-gesture
  window, and not elsewhere.
- **Touch:** a two-finger pair forms within 250 ms and does not form after
  it. Two concurrent pairs drive independent sites. A single finger keeps
  tap, long-press and scroll.
- **Display:** the overlay `momentary` animated row shows the live value.
  A momentary frame does not increment `annotationsRevision` or
  `textRevision`.
- **Re-base:** a site-table refresh during a gesture re-keys it and
  re-sends a re-based target.
- **Eligibility:** Reeval and Manual sites do not start a gesture.
- **Shortcuts:** `Mod-.` and `Mod-Shift-.` are tested with Meta (Mac
  platform) and with Ctrl (other platforms), including the Shift `>` case
  and focus outside the code pane.
- **Glide setting:** default, clamp, step and localStorage round-trip.
- **Toolbar:** the toolbar labels and the `data-output` states.

### 8.3 Browser behavior e2e (silent)

The existing silent harness runs in Chromium and WebKit. It gains the
following checks:

- a right-drag on a bound site changes the overlay label, with the
  document text unchanged;
- no context menu appears on the site;
- `Mod-.` leads to `TransportSample.output` reaching `draining` and then
  `idle`;
- `Mod-Shift-.` leads to `cutting` and then `idle`.

These run under the measurement lock.

### 8.4 Gates

The gates are those listed in the intake:

- cargo build;
- `cargo clippy --locked --all-targets -- -D warnings`;
- the full nextest run (timeout 2400 s or more, run alone, under the lock);
- the wasm debug build and `mise run build-wasm-release`;
- `cargo check` in editor/src-tauri;
- `npm run check`, vitest, and `VACTR_REQUIRE_SESSION_ABI=1 npm run build`;
- `npm run test:style` and `npm run test:perf`;
- the canvas behavior e2e in Chromium and WebKit.

No existing threshold or canvas budget changes.

## 9. Changes to other design documents

- **design-implementation.md 11.3, "stop / hush":** an amendment line
  records D2/D3 (the Natural release now covers scheduled voices; hush adds
  the output cut and clear; there is a new `stop-all`) and points here.
- **design-implementation.md 15.1.5:** an amendment line maps `Mod-.` to
  `stop-all` and `Mod-Shift-.` to `hush`, and records the new toolbar
  labels.
- **design-implementation.md 15.3.8:** an amendment line maps `Mod-.` to
  `stop-all` and `Mod-Shift-.` to `hush`.
- **design-song-mode.md:** a short "Live stop and cut" note records D5.
- **design-implementation.md section 13:** gains a cross-reference noting
  that the momentary tweak is a third front end beside overlay and
  source-edit. It never writes the tweak slot.

## 10. Suggested plan decomposition

The plan author may refine this. The plans run serially because they share
wire and protocol files.

1. **LP-ENGINE** (Rust DSP):
   - `dsp/engine/output.rs` (state machine, cut gate, peak, clear cursor);
   - `FxUnit::clear` and the `OrbitDelay` memory clear;
   - deferred `collect`;
   - the Natural release extension;
   - `dsp/ramp.rs` with the `RampedCells` read path;
   - the new wire records in host/wire.rs and native record handling;
   - the DSP tests 8.1(1)-(4).
2. **LP-SESSION** (Rust session and scheduler):
   - the protocol messages, codec and TS types/envelope;
   - the revoke stop modes and native mapping;
   - `session/momentary.rs` and the `VarSlot` momentary field;
   - the CellRamp emission and ack/re-send;
   - the song gentle end and cut;
   - `TransportSample.output`;
   - the wasm and native host plumbing;
   - the tests 8.1(5).
3. **LP-EDITOR-STOP:** the app-level shortcuts, the eval keymap change, the
   transport toolbar labels, `data-output`, and the client
   `stopAll()`/`momentary()`, with their vitest.
4. **LP-EDITOR-MOMENTARY:**
   - `bind/momentary.ts`;
   - the pointer routing for right press and touch pairs;
   - context-menu scoping;
   - the animated `momentary` row in the renderer;
   - the glide setting in the controls pane;
   - vitest and style.
5. **LP-EVIDENCE:** the e2e additions in 8.3, the full gates, and the
   closeout.

## 11. Limitations (stated, not deferred defects)

- Momentary values reach audio at each consumer's existing read
  granularity: voice start for event controls, block start for continuous
  ones. A note that is already sounding does not follow a change to an
  event-level control. This is the 11.3/11.4 cell contract and is
  unchanged.
- Desktop tweaks are sequential, because a mouse is one pointer. Only
  their glides overlap. Concurrent tweaks need touch.
- MIDI and OSC sinks keep their current `Natural` behavior. External synths
  get no extra note-offs from gentle stop.
- A cut issued while a song install is pending waits for the existing
  control-drain gate.

## References

- design-docs/specs/design-implementation.md 11.3, 11.4, 12.2, 12.5, 12.8,
  13, 15.1.5, 15.1.6, 15.3.8.
- design-docs/specs/design-song-mode.md, sections "Routing and tails",
  "Live controls and snapshot application" and "Playback and export".
- design-docs/specs/design-ui-style.md (flat, square, token-based UI).
- See `design-docs/references/README.md` for external references.
