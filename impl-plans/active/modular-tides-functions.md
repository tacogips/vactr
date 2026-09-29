# Two Tides Generations and Audio-Rate Function Coverage

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Split SYN-008. The first pinned Tides `generator.h` exposes AD, looping
and AR modes, high/medium/low ranges, pitch, shape, slope, smoothness,
frequency ratio, sync, gate, clock and freeze, with unipolar/bipolar
sample outputs and attack/release flags. Tides2 `ramp_generator.h` and
`poly_slope_generator.h` expose three ramp modes, four output modes,
control/audio range, four channels, frequency, pulse width, shape,
smoothness, shift and clock/gate. Each generation needs its own typed
manifest and runtime path. Four Tides2 outputs require an explicit Vactr
channel contract; stereo downmix alone does not satisfy coverage.

## Resource Boundary

Tides1 `resources/wavetables.py` reads `resources/waves.bin`, but the pinned
default `Generator::Process` has `WAVETABLE_HACK` commented out and uses
audio/control-rate functions followed by filter/wavefolder. Thus the
unaudited wave file belongs to an optional hack, not a missing default
mode. The Vactr adaptation imports neither that file, generated fold
tables nor aggregate `resources.cc`. Tides2 resource tables still need a
generator-by-generator audit.

## Modules

### `src/dsp/ported/functions.rs`

```rust
pub struct FunctionSpec {
    pub generation: u8,
    pub mode: u8,
    pub output_mode: u8,
    pub vactr_template: Option<&'static str>,
    pub status: Fidelity,
    pub resources: ResourceState,
}
pub fn functions() -> &'static [FunctionSpec];
```

Keep phases, filters and ratios bounded and allocated before rendering.
The first generation's envelope flags should become codeable outputs or
events, and Tides2 four-channel output must be retained or diagnosed.
No mode should be called a source port before source-control and signal
comparison.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| TID-001 | Both generations' mode/control/output and table provenance inventory | SYN-008 | Public enums and Tides1 wave dependency audited; detailed data audit open |
| TID-002 | Tides1 AD/looping/AR analytic audio-rate generator | TID-001 | Runnable analytic adaptation with three ranges and main/aux |
| TID-003 | Tides1 ratio/sync/filter/wave modes with cleared resources | TID-001 | Ratio, clock/sync, smoothing and original fold runnable; exact source filter and optional hack parity open |
| TID-004 | Tides2 three ramp × four output modes at audio rate | TID-001 | All 24 mode/output/range roles runnable as original analytic adaptations |
| TID-005 | Four-channel routing, gate/clock and phase/ratio controls | TID-002..004 | Four lanes independently selectable as main/aux; opt-in quad direct stems runnable, external ramp contract pending |
| TID-006 | Source comparison, notices, native/browser and allocation verification | TID-002..005 | Notices and native/browser/capacity/allocation checks complete; source comparison remains open |

## Completion Criteria

- [x] Both generations' published modes and combinations have truthful coverage rows.
- [x] First-generation controls, trigger, ratio and four sample/flag selectors have `.vact` and editor mappings; default stereo multiplexes them and opt-in quad emits all four adaptation roles together.
- [x] Unverified `waves.bin` and aggregate resources are excluded from the runnable Tides1 path.
- [x] Opt-in quad routing emits all four Tides2 lanes or the four Tides1 sample/flag roles simultaneously, with direct-stem and flag-timing adaptation limits documented.
- [x] Native/browser rate, capacity and zero callback allocation tests pass for both adaptations.
- [x] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass for both adaptations.

## Progress Log

### Session: 2026-09-28 — Tides1 EOA/EOR role translation

Pinned high/control-rate flag paths set EOA as a level after the attack or
while inactive; AR sustain also holds it. EOR remains high while inactive
and holds for 48 generator samples after a low-frequency loop wrap, or one
generator sample above the fixed-point threshold. Vactr now translates
these roles with bounded existing voice state and host-rate scaling: about
1 ms at normal rates or 4 ms in low range below roughly 500/125 Hz,
respectively. High-range AR sustains at half phase; medium/low use their
authored slope split. Gate/sync retrigger clears a prior release hold,
and freeze replays the previous flag. Boundary, rate, retrigger and quad
tests cover the adaptation. A direct low-range render test reaches real
25.6/256 Hz wraps at 44.1/48/96 kHz and checks the full held runs on
either side of the effective 125 Hz boundary. Source slope compression,
fixed-point phase, block delay and exact edge semantics remain open.

### Session: 2026-09-28 — Tides1 opt-in four-role stems

Added `tidal-quad-voice` to the explicit four-output example. Its four
synchronized core instances send unipolar, bipolar, end-of-attack and
end-of-release roles to channels 1–4. Channels 3/4 are post-voice-gain
direct stems and bypass stereo bus/master FX. The default `tidal-voice`
remains stereo and available without loading the example; two-output hosts
reject the quad template at install. The initial flag outputs were
one-sample adaptation pulses; the EOA/EOR role translation above supersedes
that first behavior. All eleven
header controls remain codeable and dynamically discoverable by the editor
after the opt-in file loads. Native/browser quad graph tests cover rates,
blocks, codec, budget, distinct outputs and callback allocation.

### Session: 2026-09-28 — second-generation four-lane adaptation

Added `tidal-poly-voice` as a separate original 16-float-per-node ramp kernel.
All three ramp modes, four output-mode roles and two ranges (24 combinations)
are runnable and inventoried as `Adaptation/Replacement`. The template
exposes frequency, pulse width, shape, smoothness, shift, gate, clock,
mode and range. `poly-main-channel` and `poly-aux-channel` each select one
of four independently computed lanes; both are codeable and editor-visible.
The default host graph/voice/bus remains two-output. An opt-in four-output
host and `examples/quad-stems.vact` now emit all four lanes as two stereo
outputs plus two direct stems. The direct stems bypass stereo bus/master FX;
this remains an adaptation, not a full four-channel source port. Source ratio arrays, generated waveform/fold
tables and polyBLEP implementation are excluded. Vactr uses original
continuous shift-to-ratio mapping and clock-edge reset; source external
ramp synchronization, exact channel voltages, source timing/waveshaping and
firmware DAC/output-voltage parity remain open. Native/browser, selector/control,
capacity and callback-allocation tests cover the runnable adaptation.

### Session: 2026-09-28 — first-generation analytic function slice

Added `tidal-voice` with AD, looping and AR modes and high/medium/low
ranges. Frequency, pitch, shape, slope, smoothness, ratio, sync, gate,
clock and freeze are codeable in `.vact` and editor metadata. The main
output selects unipolar, bipolar, end-of-attack or end-of-release with
`tide-output` 0..3; auxiliary defaults to bipolar. Their initial one-sample
behavior was superseded by the level/hold translation above.
This multiplexes outputs and flags, unlike source simultaneous samples
and flag bits. The nine Tides1 mode/range rows are `Adaptation/Replacement`;
all 24 Tides2 mode/output/range rows remain Pending. The kernel uses fixed
16-float state per output node, original curves and a small analytic fold.
Source fixed-point timing, high-range audio versus medium/low control-rate
split, discrete sync ratio list, block delay, PLL/pattern predictor, exact
filter/wavefolder and Tides2 four-channel parity remain open. The optional
wavetable hack is not part of the pinned default path and imports no data.

### Session: 2026-09-28 — source inventory

The pinned first-generation `Generator` declares three modes and three
ranges, while Tides2 dispatches three ramp modes × four output modes ×
two ranges (24 combinations) and four channels. Tides1's wavetable
generator reads an unaudited `waves.bin`. This plan records source scope
only; no Tides implementation or data has been imported into Vactr.
