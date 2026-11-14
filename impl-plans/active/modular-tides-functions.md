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

**Tides2 measured metrics**: Values below are medians across all four lanes
and each row's settings (the three Looping settings are pooled). RMS is the
Vactr/reference RMS ratio. `--` means the level/correlation metrics are
undefined because the baseline reference and candidate lane are silent.

| Mode / output | Range | Settings | Class | RMS ratio | Correlation | Aligned NRMSE | Reference / Vactr estimated Hz |
|---|---|---:|---|---:|---:|---:|---:|
| AD / GATES | control | 1 | measured gap | 0.1502 | 0.8400 | 0.8997 | -- / -- |
| AD / GATES | audio | 1 | measured gap | 0.1502 | 0.8400 | 0.8997 | -- / -- |
| AD / AMPLITUDE | control | 1 | not comparable | -- | -- | -- | -- / -- |
| AD / AMPLITUDE | audio | 1 | not comparable | -- | -- | -- | -- / -- |
| AD / PHASE | control | 1 | measured gap | 0.2859 | 0.8443 | 0.9947 | -- / -- |
| AD / PHASE | audio | 1 | measured gap | 0.2859 | 0.8443 | 0.9947 | -- / -- |
| AD / FREQUENCY | control | 1 | measured gap | 0.1178 | 0.5202 | 0.9539 | -- / -- |
| AD / FREQUENCY | audio | 1 | measured gap | 0.1178 | 0.5202 | 0.9539 | -- / -- |
| LOOPING / GATES | control | 3 | measured gap | 0.2272 | 0.0765 | 0.9964 | 5.00 / 5.00 |
| LOOPING / GATES | audio | 3 | measured gap | 0.2272 | 0.4360 | 0.9658 | 5.00 / 5.00 |
| LOOPING / AMPLITUDE | control | 3 | measured gap | 0.9051 | 0.1107 | 1.3863 | 14.90 / 5.00 |
| LOOPING / AMPLITUDE | audio | 3 | measured gap | 0.5260 | -0.0088 | 1.0640 | 48.57 / 5.00 |
| LOOPING / PHASE | control | 3 | measured gap | 0.1846 | 0.5765 | 1.0142 | 5.00 / 5.00 |
| LOOPING / PHASE | audio | 3 | measured gap | 0.1929 | 0.5397 | 0.8981 | 16.67 / 5.00 |
| LOOPING / FREQUENCY | control | 3 | measured gap | 0.1872 | 0.0454 | 0.9905 | 8.75 / 5.36 |
| LOOPING / FREQUENCY | audio | 3 | measured gap | 0.2115 | 0.0471 | 1.0197 | 22.65 / 5.36 |
| AR / GATES | control | 1 | measured gap | 0.1350 | 0.9761 | 0.8793 | -- / -- |
| AR / GATES | audio | 1 | measured gap | 0.1350 | 0.9761 | 0.8793 | -- / -- |
| AR / AMPLITUDE | control | 1 | not comparable | -- | -- | -- | -- / -- |
| AR / AMPLITUDE | audio | 1 | not comparable | -- | -- | -- | -- / -- |
| AR / PHASE | control | 1 | measured gap | 0.6204 | 0.9672 | 1.1058 | -- / -- |
| AR / PHASE | audio | 1 | measured gap | 0.6204 | 0.9672 | 1.1058 | -- / -- |
| AR / FREQUENCY | control | 1 | measured gap | 0.1659 | -0.5746 | 1.1133 | -- / -- |
| AR / FREQUENCY | audio | 1 | measured gap | 0.1659 | -0.5746 | 1.1133 | -- / -- |

The baseline AD/AR AMPLITUDE rows are not comparable because the source
amplitude lanes are silent at shift 0.5. Independent review (2026-09-29)
found that this is a limitation of the probe design, not a DSP property.
The gate is high for only the first 24-frame block, and the comparison
window starts after a 48-frame warm-up, so it covers only the post-decay
tail. These four rows are therefore unmeasured. A follow-up scenario with
no warm-up, or with a gate held or retriggered inside the window, is still
needed before they can be classified. Twenty other combinations are
measured gaps and none are classified close. The source and Vactr looping
GATES/PHASE estimates are both about 5 Hz at baseline; several other output
roles estimate different periodic rates. Across every lane, the raw JSON
emitted by the comparison task also records source/candidate peak and trough
levels and 10–90% rise/fall times; aggregate waveform metrics cannot establish
matching control curves or all phase behavior.

**Tides1 measured metrics**: Six mode/lane rows compare source unipolar and
bipolar samples with the corresponding `tidal_function` selectors.

| Mode / lane | Class | Source / Vactr RMS | Correlation | Aligned NRMSE | Source / Vactr Hz | Source / Vactr peak; trough | Source / Vactr rise / fall ms |
|---|---|---:|---:|---:|---:|---|---|
| AD / unipolar | close | 0.2359 / 0.2223 | 0.9427 | 0.3121 | -- / -- | 1.000 / 0.835; 0.000 / 0.000 | 33.33 / 31.48; 41.65 / 35.40 |
| AD / bipolar | close | 0.9424 / 0.9293 | 0.9427 | 0.1563 | -- / -- | 0.999 / 0.670; -1.000 / -1.000 | 33.33 / 31.48; 41.65 / 35.40 |
| LOOPING / unipolar | measured gap | 0.5779 / 0.5490 | 0.8206 | 0.2850 | 12.00 / 12.00 | 1.000 / 0.837; 0.000 / 0.001 | 33.33 / 31.48; 41.65 / -- |
| LOOPING / bipolar | measured gap | 0.5762 / 0.4695 | 0.8206 | 0.5718 | 12.00 / 12.00 | 0.999 / 0.674; -0.999 / -0.998 | 33.33 / 31.48; 41.65 / -- |
| AR / unipolar | measured gap | 0.2359 / 0.1373 | 0.6307 | 0.6473 | -- / -- | 1.000 / 0.605; 0.000 / 0.000 | 33.33 / 9.44; 41.65 / 30.15 |
| AR / bipolar | measured gap | 0.9424 / 0.9532 | 0.6308 | 0.3254 | -- / -- | 0.999 / 0.211; -1.000 / -1.000 | 33.33 / 9.44; 41.65 / 30.15 |

The Tides1 AD sample roles are close at these settings, while Looping and AR
remain measured gaps. The matching Looping frequency estimate does not imply
waveform or flag parity. These probes exercise raw kernels only: they do not
verify `.vact` event routing, editor controls, host graph/bus behavior,
browser execution or listening quality. The results do not justify changing
any fidelity label to `SourceStage` or `SourcePort`.
