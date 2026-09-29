# Two Tides Generations and Audio-Rate Function Coverage

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-29

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
| TID-006 | Source comparison, notices, native/browser and allocation verification | TID-002..005 | Native/browser/capacity/allocation checks complete; raw source comparisons and metrics are recorded below; fidelity remains adaptation |

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

### Session: 2026-09-29 — TID-006 pinned source comparison

**Tasks Completed**: Added `verification/tides_poly_reference.cc`,
`verification/compare_tides_poly.py`, `examples/tides_poly_reference.rs`
and `mise run compare-tides-poly`. The driver compiles separate Tides1 and
Tides2 executables in a temporary directory at Eurorack
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` and stmlib
`e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`; `-DTEST` removes only firmware
RAM section placement attributes. No source object, generated table or audio
is added to Vactr. The common render is 48 kHz in 24-frame blocks, with a
24-frame gate pulse and 24,000 total frames. Tides2 covers all three ramp
modes × four output roles × two ranges once, plus two extra PW/shape/
smoothness/shift settings for each of the eight Looping output/range rows.
PHASE/FREQUENCY follow `tides.cc` half-rate processing and duplicated output
samples. Tides1 covers AD/looping/AR at 12 Hz, shape/slope/smoothness 0.5,
high range and its default `WAVETABLE_HACK`-disabled unipolar/bipolar outputs.
Each lane reports RMS, correlation, phase-aligned normalized RMS error,
period/frequency estimate, peak/trough and 10–90% rise/fall times. Alignment
searches ±256 samples; a 16-sample average precedes period estimation to
reduce Tides1 render-block artifacts.

### TID-006 corrected pinned source comparison

The comparison harness converts Tides2 source voltages to each source lane's
own normalized range before computing level metrics: unipolar lanes map to
0..1 and bipolar lanes to -1..1. Correlation remains scale invariant.
Ordinary phase-aligned NRMSE uses these unit-converted samples. The diagnostic
gain-normalized NRMSE fits both a least-squares gain and offset after phase
alignment, then reports those fitted values per lane and the residual error;
this affine fit separates overall gain and DC offset from residual waveform
shape. Classification continues to use correlation >= 0.90 and ordinary
unit-converted NRMSE <= 0.35; the affine-fit metric is diagnostic only.

The per-mode/lane conversion is:

| Tides2 source output | Source path and voltage range | Conversion to normalized source lane |
|---|---|---|
| GATES lane 0, AD/AR | `Fold` multiplied by signed shift, -8..+8 V | divide by 8 (-1..1) |
| GATES lane 1, AD/AR | `Scale`, 0..8 V | divide by 8 (0..1) |
| GATES, lanes 0-1, LOOPING | `Fold`/`Scale`, -5..5 V | divide by 5 (-1..1) |
| GATES, lanes 2-3, all modes | EOA/EOR outputs multiplied by 8, 0..8 V | divide by 8 (0..1) |
| AMPLITUDE, all lanes, AD/AR | `Fold`, 0..8 V | divide by 8 (0..1) |
| AMPLITUDE, all lanes, LOOPING | `Fold`, -5..5 V | divide by 5 (-1..1) |
| PHASE and FREQUENCY, all lanes, AD/AR | `Fold`, 0..8 V | divide by 8 (0..1) |
| PHASE and FREQUENCY, all lanes, LOOPING | `Fold`, -5..5 V | divide by 5 (-1..1) |

The pinned source implements signed GATES lane 0 at `poly_slope_generator.h:295-299` (AD/AR spans -8..+8 V because shift is signed),
EOA/EOR lanes 2-3 at `:300-303`, AMPLITUDE at `:304-320`, PHASE at `:321-336`,
and FREQUENCY at `:337-345`. Its shared `Fold` and `Scale` voltage ranges are
explicit at `:369-393`. Vactr's `tidal_poly.rs:111-143` computes normalized
values clamped to -1..1; source unipolar lanes are normalized to 0..1 without
an offset, so polarity/offset differences remain visible in the comparison.
Tides1 needs no additional conversion: pinned `GeneratorSample` stores
`uint16_t unipolar` and `int16_t bipolar` (`tides/generator.h:67-69`), and the
C++ probe normalizes by 65535 and 32767 before printing
(`verification/tides_poly_reference.cc:120-124`).

The comparison covers all 24 Tides2 mode/output/range combinations at 48 kHz,
24-frame blocks, 24,000 frames and the same initial 24-frame gate. The eight
LOOPING mode/output/range rows pool three settings each (baseline plus two
PW/shape/smoothness/shift extras). AD/AR AMPLITUDE also retain the separate
shift-0.75 frame-zero triggered follow-up. In that follow-up, lane 0 is marked
not comparable because the source shift interpolator starts at zero and
creates an approximately 0.3 ms startup artifact (`poly_slope_generator.h:102,
233-234`); lane 3 is not comparable because its source gain is zero at shift
0.75. Only lanes 1-2 contribute to each follow-up classification. Tides1
covers AD, LOOPING and AR at
12 Hz, shape/slope/smoothness 0.5, high range, and both normalized sample
roles. Values below are medians across four lanes and, for LOOPING, its three
settings. RMS is Vactr/source; `--` means the baseline reference lane is
silent and its NRMSE is undefined. Gain+offset NRMSE is the residual after an
affine least-squares fit; the fitted gain and offset are present per lane in
the JSON.

| Mode / output | Range | Settings | Class | Vactr/source RMS | Correlation | Unit NRMSE | Gain+offset NRMSE |
|---|---|---:|---|---:|---:|---:|---:|
| AD / GATES | control | 1 | measured gap | 1.2013 | 0.8400 | 0.8705 | 0.2329 |
| AD / GATES | audio | 1 | measured gap | 1.2013 | 0.8400 | 0.8705 | 0.2329 |
| AD / AMPLITUDE | control | 1 | not comparable | -- | -- | -- | -- |
| AD / AMPLITUDE | audio | 1 | not comparable | -- | -- | -- | -- |
| AD / PHASE | control | 1 | measured gap | 2.2868 | 0.8443 | 2.3420 | 0.3865 |
| AD / PHASE | audio | 1 | measured gap | 2.2868 | 0.8443 | 2.3420 | 0.3865 |
| AD / FREQUENCY | control | 1 | measured gap | 0.9425 | 0.5202 | 1.0279 | 0.7185 |
| AD / FREQUENCY | audio | 1 | measured gap | 0.9425 | 0.5202 | 1.0279 | 0.7185 |
| LOOPING / GATES | control | 3 | measured gap | 1.3578 | 0.0765 | 1.7576 | 0.8052 |
| LOOPING / GATES | audio | 3 | measured gap | 1.4100 | 0.4360 | 1.4536 | 0.7730 |
| LOOPING / AMPLITUDE | control | 3 | measured gap | 4.5256 | 0.1107 | 4.5681 | 0.8874 |
| LOOPING / AMPLITUDE | audio | 3 | measured gap | 2.6298 | -0.0088 | 2.7227 | 0.9202 |
| LOOPING / PHASE | control | 3 | measured gap | 0.9231 | 0.5765 | 1.3456 | 0.6991 |
| LOOPING / PHASE | audio | 3 | measured gap | 0.9644 | 0.5397 | 1.0262 | 0.7326 |
| LOOPING / FREQUENCY | control | 3 | measured gap | 0.9362 | 0.0454 | 1.2794 | 0.7240 |
| LOOPING / FREQUENCY | audio | 3 | measured gap | 1.0574 | 0.0471 | 1.4721 | 0.9781 |
| AR / GATES | control | 1 | measured gap | 1.0797 | 0.9761 | 0.4673 | 0.0780 |
| AR / GATES | audio | 1 | measured gap | 1.0797 | 0.9761 | 0.4673 | 0.0780 |
| AR / AMPLITUDE | control | 1 | not comparable | -- | -- | -- | -- |
| AR / AMPLITUDE | audio | 1 | not comparable | -- | -- | -- | -- |
| AR / PHASE | control | 1 | measured gap | 4.9632 | 0.9672 | 4.9316 | 0.1406 |
| AR / PHASE | audio | 1 | measured gap | 4.9632 | 0.9672 | 4.9316 | 0.1406 |
| AR / FREQUENCY | control | 1 | measured gap | 1.3268 | -0.5746 | 2.1125 | 0.7564 |
| AR / FREQUENCY | audio | 1 | measured gap | 1.3268 | -0.5746 | 2.1125 | 0.7564 |

Baseline Tides2 has 20 measured-gap rows and four not-comparable AD/AR
AMPLITUDE rows because their source lanes are silent at shift 0.5. The four
separate gate-window follow-ups (AD/AR × control/audio, shift 0.75, frame zero
through frame 23,999) are measured gaps based on lanes 1-2. Lane 0's startup
artifact and lane 3's zero source gain are excluded as described above:

| Mode / range | RMS source / Vactr | Correlation | Unit NRMSE | Gain+offset NRMSE |
|---|---:|---:|---:|---:|
| AD / control | 0.1823 / 0.3334 | 0.8400 | 2.0381 | 0.3918 |
| AD / audio | 0.2725 / 0.3334 | 0.8399 | 1.5918 | 0.3918 |
| AR / control | 0.1298 / 0.3650 | 0.9688 | 2.8856 | 0.1457 |
| AR / audio | 0.1939 / 0.3650 | 0.9695 | 2.1527 | 0.1428 |

For Tides1, the two AD roles remain close and the other four roles remain
measured gaps. RMS is source / Vactr:

| Mode / lane | Class | Source / Vactr RMS | Correlation | Unit NRMSE | Gain+offset NRMSE |
|---|---|---:|---:|---:|---:|
| AD / unipolar | close | 0.2359 / 0.2223 | 0.9427 | 0.3121 | 0.3120 |
| AD / bipolar | close | 0.9424 / 0.9293 | 0.9427 | 0.1563 | 0.1562 |
| LOOPING / unipolar | measured gap | 0.5779 / 0.5490 | 0.8206 | 0.2850 | 0.2849 |
| LOOPING / bipolar | measured gap | 0.5762 / 0.4695 | 0.8206 | 0.5718 | 0.5715 |
| AR / unipolar | measured gap | 0.2359 / 0.1373 | 0.6307 | 0.6473 | 0.6016 |
| AR / bipolar | measured gap | 0.9424 / 0.9532 | 0.6308 | 0.3254 | 0.3024 |

### Behavior gaps identified by independent review

These are measured comparison gaps, not kernel changes:

- Vactr AMPLITUDE lanes have a `0.04 + 0.02 * lane` floor, so they remain
  non-silent where source gains are zero (`tidal_poly.rs:131-135`).
- AR FREQUENCY correlation is negative, ranging from -0.48 to -0.65 across
  the reviewed comparisons.
- LOOPING FREQUENCY lane 3 correlation is -0.22.
- The Vactr LOOPING GATES EOR lane is almost never high.
- Source GATES lane 0 is silent at shift 0.5 (`shift * 2 - 1 = 0`), so AD/AR
  GATES lane 0 is not compared in the baseline classification.

### Session: 2026-09-29 (unit correction)

Coordinator review identified that Tides2 source `OutputSample` channels are
voltages, while Vactr `tidal_poly` lanes are normalized. Direct comparison had
made source RMS, peaks/troughs and NRMSE invalid and made the previous Tides2
classification unreliable; correlation itself is scale invariant. The
harness now applies the per-mode/lane voltage conversions above before all
level metrics, and reports both unit-converted NRMSE and least-squares
gain-and-offset-normalized NRMSE with fitted parameters per lane. Source
references and Vactr ranges are cited above.
Tides1 was checked separately: its source sample types are integer levels and
the C++ probe already normalizes those to 0..1 and -1..1, so no extra scale
was needed. Classification counts after correcting units are 20 Tides2
baseline measured gaps, four silent baseline rows not comparable, and four
separate gate-window follow-up measured gaps; Tides1 remains two close and
four measured gaps. Before correction the plan's combined Tides2 headline
reported 24 measured gaps by folding the four triggered AMPLITUDE results into
those rows; this correction separates the silent baselines from their
follow-ups. No fidelity labels changed and no kernel files changed. The
corrected GATES rows also make a possible polarity translation difference
measurable: source EOA/EOR lanes normalize to 0/1 (`:300-303`), while Vactr
GATES lanes 2/3 emit -1/+1 (`tidal_poly.rs:115-128`). This is reported as an
adaptation gap only; no kernel behavior was changed. The updated comparison runs from the existing `compare-tides-poly` task
against the pinned checkout. Formatting, native cargo check, strict Clippy,
wasm library check, `mise tasks validate` (28 tasks), and the upstream audit
passed; the audit reported zero errors. Independent verification:
nextest 1532 passed, 1 skipped.
