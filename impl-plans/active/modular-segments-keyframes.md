# Audio-Rate Segments and Keyframe-Controlled Mixing

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Split SYN-008 and FX-005 for Stages and Frames. The pinned Stages
`segment_generator.h` supports ramp, step, hold and alternate segment
types, loops, per-segment primary/secondary controls, phase/value outputs
and an explicit `ProcessOscillator(audio_rate, ...)` path. The pinned
Frames firmware evaluates four-channel keyframe or poly-LFO DAC controls;
the physical mixer/VCA audio path is analog and absent from firmware DSP.
A digital keyframe-controlled audio mixer in Vactrol is therefore an
adaptation, not a source mixer port.

## Modules

### `src/dsp/ported/functions.rs`

```rust
pub struct SegmentSpec {
    pub source_type: u8,
    pub vactrol_template: Option<&'static str>,
    pub status: Fidelity,
}
pub fn segment_modes() -> &'static [SegmentSpec; 4];
```

Stages needs bounded segment count/delay and explicit gate/loop resets.
Frames needs a typed keyframe data contract, four independent control
outputs, easing/shape controls and a clearly separate digital gain/mixer
stage if offered as an effect. Read source-generated waveform tables only
after individual provenance review; otherwise use analytic shapes.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| SGF-001 | Stages segment/oscillator and Frames keyframer/LFO source inventory | SYN-008, FX-005 | Stages and Frames control paths inventoried; analog boundary audited |
| SGF-002 | Stages four segment types, audio-rate looping, CV delay, six-scalar and opt-in 36-record chains, value/phase output | SGF-001 | Bounded original adaptations runnable; source serial/slave and timing parity pending |
| SGF-002B | Detect source-shaped director-plus-step lists in the 36-record chain and adapt seven sequencer traversal modes | SGF-002 | Original event-local adaptation runnable; source numerics, slave and output quantization remain separate |
| SGF-003 | Frames four-channel keyframe interpolation and poly-LFO controls | SGF-001 | Poly-LFO and 64-keyframe analytic adaptations runnable; opt-in quad direct stems available, structured list editor pending |
| SGF-004 | Original digital four-input gain/mixer adaptation driven by Frames controls | SGF-003 | Not started |
| SGF-005 | `.vact`/editor routing, capacity and native/browser tests | SGF-002..004 | Stages/Frames adaptations covered; planned digital mixer and structured list editor remain |
| SGF-006 | Source comparisons, resource notices and fidelity review | SGF-002..005 | Not started |

## Completion Criteria

- [x] Four Stages types, loops and audio-rate path have truthful adaptation coverage and control schemas.
- [x] Four Frames outputs, easing/keyframes and poly-LFO controls are codeable without claiming analog VCA DSP; the structured list editor and source numerical parity remain open.
- [ ] Original mixer adaptation, if implemented, exposes its digital input/output and gain contract.
- [x] Stages slice resource provenance, bounded memory and zero callback allocation are verified.
- [x] Stages slice native/browser, quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass.
- [x] Source-shaped Stages step lists traverse up/down/ping-pong/alternating/random/no-repeat/addressable with event-local deterministic state, `.vact` controls, value/position outputs, and no callback allocation.

## Progress Log

### Session: 2026-09-28 — firmware/audio boundary

`stages/segment_generator.h` declares four segment types and an
audio-rate oscillator processor. `frames/frames.cc` writes four DAC values
from `Keyframer` or `PolyLfo`; it does not implement the analog mixer/VCA
audio path. No Stages or Frames source/data has been imported into Vactrol.

### Session: 2026-09-28 — Stages bounded audio adaptation

The 16-cell `ProcessFn` registration is inventoried in source order. Fourteen
cells have runnable, event-local analytic roles in `stage-voice`; two
intentional `ProcessZero` cells are excluded. Both Delay cells use one
secondary-controlled CV delay with primary as the delayed value; trigger,
gate and loop do not affect this source role. A 576-at-31.25k-equivalent
host-rate ring is preallocated per lane, with decimated writes for times
longer than the ring. At 96k the ring does not truncate the short delay.
The MultiSegment and Sequencer adaptations are recorded in later entries;
Slave remains pending, and the commented-out ClockedSampleAndHold registration
is excluded. Four
segment-type selections and decay, timed pulse, gate, sample/hold, free/tap
LFO, free/PLL oscillator choices use original host-rate curves and bounded
16-float state per output lane. Main and auxiliary represent unipolar value
and bipolar phase/wave from separate event-local instances. Their clocks and
resets are adaptations, not the source's 31.25 kHz linked segment timing.
The source wave, frequency and portamento tables, 16-bit delay representation
and interpolation are not imported. Frames tasks remain open.

### Session: 2026-09-28 — one-module multi-segment chain

`stage-chain-voice` has six independent type/loop/primary/secondary groups,
an active segment count, gate and trigger. It uses a separate 29-port node
that fits the existing typed port and event limits; no wire-size change is
needed. The main lane is value and the auxiliary lane is bipolar segment
phase. Completion advances ramps, holds and one-cycle oscillator segments;
step segments wait for an edge. Marked loop segments define a first-to-last
loop span, and gate fall can leave that span. The last segment control is
exercised through `.vact`, editor metadata and the graph codec. This is an
original host-rate adaptation: source cross-segment ramp target borrowing,
step search, sentinel routing, exact curves, 31.25 kHz schedule and 36
segments across chained modules remain unimplemented. The separate
Sequencer adaptation is recorded below; Slave and Frames tasks remain open.

### Session: 2026-09-28 — opt-in 36-record Stages chain

`stage-linked-voice` accepts one immutable flat list of 1–36 stride-four
records `[type loop primary secondary]`. Compilation and versioned graph
decoding reject malformed, nonfinite, non-discrete and over-capacity data;
two fixed payload slots serve independent main value and aux phase cores.
The list is discoverable in editor metadata after explicit example load. The
existing six-scalar prelude template remains unchanged. This extends the
bounded adaptation, not source cross-module serial parity: authored records
cannot be modulated live, and source slave/sentinel behavior and numeric
timing remain open. The source-shaped Sequencer adaptation is recorded below.

The public `.vact` path is tested with exactly 36 authored records. Changing
only record 36 changes native and browser audio; graph decoding retains its
last row, and record 37 produces a bounded-capacity diagnostic. Both host
render tests use armed callback-allocation probes. Source serial-group,
Slave and numeric comparison work remain open.

### Session: 2026-09-28 — source-shaped Stages Sequencer adaptation

An opt-in `stage-sequencer-voice` uses the same immutable 1–36-row payload as
`stage-linked-voice`. A non-looping non-STEP director followed only by STEP
rows selects the Sequencer path; all other lists retain the ordinary chain.
Director secondary selects seven traversal modes; immutable director primary
fixes an address or applies an event-onset reset. Gate rising edges clock;
trigger rising edges reset. Marked STEP rows bound the traversed span;
each STEP primary supplies the target
value and secondary sets a host-rate slew. Main emits smoothed value and aux
emits normalized position. The `.vact` example generates gate clock edges
within an event, and its list, gate and trigger controls are editor-visible.
Native and browser tests cover the seven modes, deterministic random and
no-repeat, marked bounds, exactly 36 rows and the last row's audible effect;
armed callbacks allocate nothing. The authored director reset is applied at
event onset so a constant immutable reset value does not inhibit later
clock edges. This, the original normalized-position aux, host-rate slew,
source hysteresis and clock inhibit, source zero phase, output quantization,
serial/slave behavior and numerical timing remain differences, not source
parity.

### Session: 2026-09-28 — Frames poly-LFO control generation

Pinned `frames/frames.cc` writes four PolyLfo or Keyframer DAC control codes;
the downstream VCA mixer is analog hardware. PolyLfo uses shape, signed
spread, shape spread and signed neighbor coupling. Its generated waveform and
increment tables are excluded. `frame-lfo-voice` uses original analytic
wave families and host-rate phase arithmetic, with `freq`, frame offset and
source-role controls codeable. Main/aux independently select any two of four
lanes; four simultaneous host outputs remain pending. The source Keyframer
stores at most 64 four-channel timestamped keyframes and provides step,
linear, in/out quartic, sine and bounce easing plus per-channel response.
The typed keyframe contract was delivered in the next slice. No firmware
digital audio mixer or analog VCA emulation is claimed.

### Session: 2026-09-28 — Frames immutable keyframe payload

`frame-keyframe-voice` accepts a flat `.vact` stride-five list of 1–64
timestamp/four-value rows. Compile and install reject nonfinite, out-of-range,
duplicate, unsorted and oversized data. A versioned graph tag carries the
bounded payload to native and browser hosts; at most four keyframe nodes are
installed per instrument. Rendering reads the immutable template payload
without callback allocation or copying. The six easing roles, including
midpoint step and endpoint clamps, and four response curves are original
analytic digital adaptations. Main/aux select two lanes independently.
A structured list editor, the source DAC/VCA response calibration and the
analog mixer remain open.

### Session: 2026-09-28 — opt-in four-lane direct stems

`examples/quad-stems.vact` defines separate Frames keyframe and Tides2
templates with four aligned core instances and explicit `out-3`/`out-4`
sinks. A four-output native or browser host emits all four lanes. Channels
1/2 retain stereo bus/master processing; channels 3/4 are direct
post-voice-gain stems, so they do not enter bus/master effects or taps.
Default two-output hosts reject quad template installation, and existing
stereo templates remain usable. The bounded Frames payload cap is four
nodes per instrument. Native/browser rate and block tests, independent
lane tests, two-voice isolation and callback allocation checks cover the
contract. The structured list editor and simultaneous firmware DAC/VCA
hardware behavior remain outside this adaptation.

### Session: 2026-09-28 — Frames poly-LFO quad example

The opt-in example now also defines `frame-lfo-quad-voice` with four
sample-aligned core instances and fixed selectors 0–3. Shape, spread,
shape-spread, coupling and offset reach all four instances. The template
is visible in the dynamic instrument editor declaration after explicit
load; the default stereo prelude remains usable on two-output hosts.
Channels 3/4 bypass stereo bus/master effects. Native/browser quad graph
and `.vact` template tests cover distinct lanes, exact four-node state
budget and one-short rejection. Source DAC calibration and analog VCA
remain outside this digital adaptation.
