# Live External Audio Input for Modular Effects

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#live-external-excitation-boundary`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Rings, Elements, Clouds and Warps effect kernels can process their stereo bus
signal, but an external source cannot currently reach that bus from a host.
Feed bounded live stereo input into the master bus ahead of its effect chain.
Keep the existing output-only API and silence-on-disconnect behavior. The
bus path is a transport feature; it does not establish source numerical
parity. Opt-in instrument source nodes now expose the same validated block.

## Modules

### `src/dsp/engine.rs`

```rust
pub fn process_with_input<C: CellStore, S: ControlSource>(
    &mut self,
    io: &mut EngineIo<'_, C, S>,
    input_interleaved: &[f32],
    out: &mut [f32],
    frames: usize,
);
```

The engine must use no allocation in a callback and must reject or safely
silence malformed/short input. Its existing `process` continues to mean no
external input. A four-output call, if extended, keeps channels 3/4 as
direct stems and routes only stereo input into channels 1/2's bus path.

### `src/host/native/audio.rs`, `src/host/wasm/worklet_half.rs`, `editor/worklet/processor.js`

```rust
pub fn render_with_input(&mut self, input: &[f32], out: &mut [f32], channels: usize);
```

The native host exposes a zero-allocation input/render API. The browser
worklet copies its connected planar input into a fixed wasm staging buffer
before processing; disconnected or missing channels are zero. Native device
capture is now opt-in.

### `src/host/native/audio.rs`, `src/host/native/mod.rs`, `src/cli/`

```rust
pub struct NativeConfig {
    pub audio_in: bool,
    // existing fields
}
```

LIN-005B opens an optional f32 input stream matched to the output's sample
rate and bridges its callback to the output callback through a fixed SPSC
stereo-frame queue. Mono is duplicated; wider devices use the first two
channels. Underrun samples are zero, overflow samples are dropped and both
conditions are observable as counters. The output callback cannot lock,
allocate, or block for input. Expose the opt-in mode in the native CLI;
keep the established output-only default and error clearly if a requested
compatible input device is unavailable. Hardware testing is conditional
on a device being present; deterministic ring/callback tests are required.

### `src/dsp/graph.rs`, `src/dsp/voice.rs`, `src/dsp/ugen/`

```rust
pub enum UGenSpec {
    HostInputL,
    HostInputR,
    // existing variants
}
```

The two source nodes read the same validated callback input without storing
it per voice. A voice beginning within a block reads the matching host
sample at its start offset. Rings receives one external excitation port;
Elements receives independent blow and strike ports. Both retain their
internal defaults, main/aux outputs and existing public templates. Graph
and browser codecs need stable appended node tags, and installation must
reject unsupported port capacities rather than truncating controls.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| LIN-001 | Engine stereo master-bus input and backward-compatible output-only path | MOD-004 | Implemented and tested |
| LIN-002 | Native render-with-input API, 2/4-output mapping, malformed-input diagnostics | LIN-001 | Implemented and tested |
| LIN-003 | Browser worklet planar input and wasm fixed-buffer bridge | LIN-001 | Implemented and tested |
| LIN-004 | `.vact` effect E2E, JS worklet, native/browser rate/block and callback tests | LIN-001..003 | Implemented and tested |
| LIN-005A | Named instrument input source UGens and Rings/Elements external-excitation templates | LIN-001..004 | Implemented and tested |
| LIN-005B | Native device capture, bounded drift bridge and CLI opt-in | LIN-001..004 | Implemented and tested (hardware audition pending) |

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Engine input | `src/dsp/engine.rs` | Opt-in stereo master-bus input | Native/browser rate/block, malformed and quad tests |
| Native host | `src/host/native/audio.rs` | Exact stereo or mono duplication, fixed scratch | `.vact` master effect and stereo/quad tests |
| Browser bridge | `src/host/wasm/worklet_half.rs`, `editor/worklet/processor.js` | Fixed input pointer, planar copy and disconnect zeroing | Wasm check and Vitest |
| Instrument-local input ports | `host-in-l/r`, voice renderer, opt-in Rings/Elements templates | Native/browser event-offset and E2E tests |
| Device capture | Native input stream and ring paths | Device-free callback tests pass; hardware audition pending | Queue, rate/block, stereo/quad and CLI parse tests |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Master-bus input | Existing stereo bus and effect contracts | Ready |
| Instrument-local external excitation | Validated engine input and source UGens | Ready |
| Native hardware capture | Matched-rate f32 selection and bounded callback queue implemented; independent-clock resampling and physical-device audition remain open | In Progress |

## Completion Criteria

- [x] A live stereo source reaches the master effect without graph mutation during callbacks.
- [x] Existing output-only rendering remains backward compatible and silent on external input.
- [x] Native and browser connected/disconnected input, mono fallback and malformed buffer behavior are verified.
- [x] `.vact` can route an input-sensitive master effect with stereo output and no callback allocation.
- [x] Named Rings/Elements instrument inputs react to independent live channels at the correct event offset.
- [x] Arbitrary multi-input graph and sample-exact device-clock synchronization gaps remain explicit in the coverage inventory.
- [x] Opt-in native device input feeds `.vact` effects and instrument sources through a bounded queue with observable underflow/overflow.
- [x] Quiet Cargo check, strict Clippy, full tests, rustfmt, wasm check, JS tests and diff check pass.

## Progress Log

### Session: 2026-09-28 — boundary assessment

`Engine::process`, native `AudioSide::render`, and the browser worklet
currently render output only. The existing stereo master bus gives a bounded
injection point ahead of its effect chain. This plan records the full host
input work while retaining the separate native device and instrument-graph
tasks.

### Session: 2026-09-28 — live stereo master input

`Engine::process_with_input` and its quad counterpart add validated L/R to
the master accumulator after clear and before effects. `AudioSide` accepts
exact interleaved stereo or mono frames (duplicated in fixed scratch);
malformed/nonfinite input is silenced and diagnosed. The worklet now exposes
one stereo Web Audio input, copies planar samples into a fixed wasm array,
and clears absent channels each quantum. `VactrolHost.connectInput(source)`
connects a Web Audio source to that node. Channels 3/4 remain direct stems.
Tests cover `.vact`-authored resonant master processing, native/browser
rate/block stereo separation, quad isolation, host mono mapping and zero
callback allocation. At that session native device capture and named
instrument-local input ports remained LIN-005; source DSP fidelity claims
were unchanged.

### Session: 2026-09-28 — instrument-local validated input

Stable `host-in-l` and `host-in-r` source nodes use appended graph tags 90/91
and a read-only reference to the validated engine input block. Voice start
offsets index the correct host sample without copying, and disconnected or
rejected input yields zero. Rings Part gained one mono excitation port (the
opt-in example averages L/R); Elements Internal gained separate blow and
strike ports at indices 28/29. `MAX_PORTS` is 30, with original indices and
internal templates preserved. The opt-in `live-external-voices.vact` file
registers editor-discoverable, codeable templates. Native/browser and
stereo/quad tests cover both channels, codec, event-offset alignment,
nonfinite rejection and callback allocation. The default template pool now
preallocates 72 slots: the 63-definition core prelude plus both opt-in
templates fit before callbacks on native and browser hosts. Tests assert
both are installed before note events and subtract the dry master input to
verify a real instrument response. A shared scheduler fix now initializes
tweak-backed header cells from their authored defaults (including custom
Elements gate and level controls) and refreshes them after a tweak. The
opt-in Elements voice therefore opens its gate and input levels at their
declared defaults, while patterns can override them.
The two instrument DSPs are
still original adaptations; native device capture and arbitrary host input
port layouts remain open.

### Session: 2026-09-28 — opt-in native device capture

`--audio-in` on repl/run/serve requests a default f32 input stream matched
to the output rate; unavailable or incompatible capture exits with a host
error instead of silently choosing noop. The default remains output-only.
The input callback normalizes mono/stereo/wider frames into a bounded SPSC
stereo queue without allocation, locks or blocking. The output callback
fills fixed scratch, silences underruns, trims excessive backlog and calls
the existing validated input renderer for stereo or quad outputs. Atomic
overrun, underrun, nonfinite, incomplete-frame and stale-frame counts are
inspectable through `capture_stats` and incremental host diagnostics.
Simulated callbacks and a public Rings/Elements `.vact` example exercise
the queue without opening hardware. Independent device clocks still drift;
there is no resampling or exact input/output timestamp alignment. A physical
device audition remains unverified.
