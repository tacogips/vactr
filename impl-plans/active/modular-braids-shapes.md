# Braids Macro-Oscillator Shape Coverage

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Split SYN-004 in `modular-synth-engines.md`. The pinned MIT Braids
`settings.h` enumerates 47 accessible shapes before the question-mark
sentinel. `macro_oscillator.cc` dispatches analog and digital oscillator
implementations, with two timbre parameters, pitch and strike/sync. Every
shape needs a stable Vactrol position, `.vact` and editor controls, resource
provenance and test evidence. Do not import aggregate `resources.cc` tables
until each generator/data input is audited; independently synthesized wave
families must be labeled replacements, not source ports.
`resources/waveforms.py` reads both `braids/data/waves.bin` and
`braids/data/map.bin` for wave-bank data. Their individual origins remain
unaudited, so positions 37–40 retain an upstream data dependency.

## Modules

### `src/dsp/ported/braids.rs`

```rust
pub struct ShapeSpec {
    pub position: u8,
    pub source_name: &'static str,
    pub vactrol_name: &'static str,
    pub status: Fidelity,
    pub resources: ResourceState,
}
pub fn shapes() -> &'static [ShapeSpec; 47];
```

Shape kernels should use the existing typed instrument manifest and fixed
voice state. Preserve source position order and explicit pitch, two timbres,
strike and sync behavior. If a shape depends on unreviewed data, supply an
original procedural substitute with distinct provenance or report it
unavailable; never silently substitute or mark an adaptation as source parity.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| BRA-001 | Enumerate 47 accessible shapes, dispatcher, parameters and resources | SYN-004 | 47-position runtime manifest complete; per-shape dependency audit for 5–46 open |
| BRA-002 | Analog and compound oscillator shapes 0–16 | BRA-001 | 0–16 runnable analytic adaptations; source comparisons open |
| BRA-003 | Digital filter, formant, harmonics and FM shapes 17–27 | BRA-001 | 17–27 runnable adaptations; source comparisons open |
| BRA-004 | Physical-model and drum shapes 28–36 | BRA-001 | 28–36 runnable adaptations; source comparisons open |
| BRA-005 | Wave-bank/map/line/paraphonic shapes 37–40 with cleared or original data | BRA-001 | 37–40 runnable replacements; source comparisons open |
| BRA-006 | Noise, granular, particle and digital modulation shapes 41–46 | BRA-001 | 41–46 runnable adaptations; source comparisons open |
| BRA-007 | Source-control and deterministic signal comparisons per shape | BRA-002..006 | Not started |
| BRA-008 | Native/browser, capacity, callback and metadata verification | BRA-002..007 | Not started |

## Completion Criteria

- [x] All 47 accessible source positions have stable, truthful coverage entries.
- [ ] Each runnable shape exposes pitch, timbre pair, strike and supported sync in `.vact` and editor metadata.
- [ ] Wave/data provenance is audited or replaced with original procedures; notices are complete.
- [ ] Separate shape response and source comparisons cover every runnable position.
- [ ] Native/browser rates, bounded state and zero callback allocations pass.
- [ ] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass.

## Progress Log

### Session: 2026-09-28 — grain, particle and symbol shapes 44–46

`macro-cloud-voice` completes runnable coverage with four analytic-windowed
sine grains, sparse deterministic excitation of three resonators, and an
I/Q carrier driven by an original pilot/random symbol sequence. Pitch,
color, timbre, event strike and local sync reach audio. Fixed state is
allocated at install, and coefficient/timing calculations use host rate.
The source envelope-rate/table, sine table, resonator coefficient/scale
lookups, RNG, constellation array and symbol/training bytes are excluded.
These are Adaptation/Replacement, not source-equivalent ports. All 47
positions are now runnable; numerical/source comparisons and a true
external sync stream remain open. WAVES flags at 37–40 remain upstream
provenance facts; no such binary is imported.


### Session: 2026-09-28 — filtered, twin-peak and clocked noise 41–43

`macro-noise-voice` selects three independently implemented noise paths:
LP/BP/HP morphing, two tuned resonant peaks, and a deterministic cyclic
sample-and-hold source with two-to-32-level quantization. Pitch, color,
timbre, strike and event-local generated sync reach audio. Coefficients
use the host sample rate; per-voice state is fixed before the callback.
The source SVF lookup, overdrive, random generator, fixed-point quantizer
and numerical filters are not ported. Positions 0–43 are
Adaptation/Replacement; 44–46 are Pending/NeedsAudit. WAVES flags at
37–40 record upstream provenance only. Focused morph, peak, quantization,
control, native/browser, capacity and allocation tests cover this slice.


### Session: 2026-09-28 — procedural line and paraphonic chord 39–40

`macro-wave-line-voice` selects a computed 64-node periodic line scan with
host-rate-smoothed position and smooth-to-stepped blend or a four-voice paraphonic chord. The latter uses
three original interval families and a lower-note inversion range, not the
source 17-row chord array. Pitch, scan/timbre or wave/chord controls,
strike and generated local sync reach audio. No `wave_line`,
`mini_wave_line`, `wt_waves`, source chord indices, `waves.bin`, `map.bin`
or LXR data is imported. Source timbres, rough interpolation, chord
voicing and 2× oversampling differ. Positions 0–40 are
Adaptation/Replacement; 41–46 remain Pending/NeedsAudit. Upstream WAVES
flags 37–40 remain provenance facts, not runtime requirements.


### Session: 2026-09-28 — procedural wave bank and map 37–38

`macro-wave-grid-voice` selects a 20-bank/16-step original procedural
scan or a 16×16 computed periodic wave map with bilinear four-corner
interpolation. Pitch, scan/bank or X/Y controls, event strike and local sync
are codeable. The pinned `braids/resources/waveforms.py` reads both
`braids/data/waves.bin` and `map.bin`; neither binary, source wave index
array, generated `wt_waves`/`wt_map` or LXR data is imported. Rows 37–38
are Adaptation/Replacement while retaining their upstream WAVES flags;
39–46 remain Pending/NeedsAudit. Source timbre, bank hysteresis, 2×
naive oversampling and numeric parity differ. Corner, bilinear, bank
endpoint, control and native/browser checks cover this bounded slice.


### Session: 2026-09-28 — kick, cymbal and snare 34–36

`macro-percussion-voice` adds three distinct procedural voices. Kick uses
an onset plus delayed 1/4 ms pulses, a swept tuned resonator and tone lowpass;
cymbal uses six metallic squares and clocked noise through separate
high-pass paths; snare uses two tuned resonators plus a decaying filtered
noise path. Pitch, two timbres, event strike and local sync respond in each.
The six ratios, noise generator, resonator/filter equations and envelopes
are independently designed, not source numerical ports. Rows 0–36 are
Adaptation/Replacement, 37–46 Pending/NeedsAudit; upstream wave-bank flags
37–40 remain. Source pulse/filter classes, exact filter coefficients,
anti-aliasing and numerical comparisons remain open.


### Session: 2026-09-28 — struck bell/drum positions 32–33

`macro-struck-voice` provides an 11-partial bell and six-partial drum with
separate filtered-noise cross-modulation. Small source partial-pitch,
amplitude and long/short decay arrays are translated from the pinned MIT
`digital_oscillator.cc`, credited in `THIRD_PARTY_NOTICES.md`. Analytic sine
replaces `wav_sine`, and decay maps from the 48 kHz/24-frame reference to
host-rate samples independent of callback block size. Pitch, decay/color,
detune-or-noise timbre, event strike and generated local sync are codeable.
Source blockwise retuning, fixed-point interpolation and generated filter
lookup are omitted, so both rows remain Adaptation/Replacement. Rows 0–33
are runnable, 34–46 Pending/NeedsAudit; source numerical comparison remains
open.


### Session: 2026-09-28 — string and wind positions 28–31

`macro-physical-voice` selects pluck, bow, blown reed and flute-edge roles.
One preallocated event-local resonator holds at least one 20 Hz period at
each host rate, including 96 kHz. Exciters differ through decaying pluck
noise, sustained friction, breath/reed nonlinearity and flute edge/DC block;
both timbres, strike and local sync alter every mode. Failed budget checks
leave an already installed native voice playable. This is an original
adaptation: upstream pluck rotates multiple delay voices and varies update
and oversampling with pitch; bow, reed and flute use separate waveguides and
lookup-driven excitation/body filters. Vactrol uses one delay per event,
clamps pitch below 20 Hz and imports none of those tables. Rows 0–31 are
Adaptation/Replacement, 32–46 Pending/NeedsAudit. Source numerical and
polyphony parity remain open.


### Session: 2026-09-28 — FM positions 25–27

`macro-fm-voice` selects ordinary two-operator phase modulation, feedback
from the previous carrier output into the modulator, and output-dependent
modulator rate. Pitch, index/color, ratio/timbre, event-onset strike and
local sync are codeable and audibly responsive. The original analytic
implementation bounds phase increments and output; it replaces the source
`wav_sine` table, fixed-point arithmetic and blockwise parameter
interpolation. Rows 0–27 are Adaptation/Replacement, 28–46
Pending/NeedsAudit. Numeric source parity and external per-sample sync
remain open.


### Session: 2026-09-28 — formant and harmonic roles 21–24

`macro-formant-voice` selects VOSIM, VOWEL, VOWEL_FOF and HARMONICS.
Original procedural paths use two reset analytic formants under a bell
window, three continuous vowel formants, a five-partial pulse grain, and a
12-partial harmonic bank with moving peaks. Color and timbre remain distinct
sound controls, with event-onset strike and local sync. Source phoneme
arrays, formant frequency/amplitude maps, waveform/LUT tables and aggregate
resources are excluded. In particular, source VOWEL_FOF uses five SVFs,
while this adaptation uses an analytic grain, so numerical and architecture
parity is open. Rows 0–24 are Adaptation/Replacement, 25–46 Pending/NeedsAudit.


### Session: 2026-09-28 — digital-filter roles 17–20

`macro-filter-voice` selects DIGITAL_FILTER_LP, PK, BP and HP. Its original
analytic implementation keeps the source's reset sine carrier, saw/triangle
window, polarity-switched pulse and bounded integral roles, with separate
output combinations for all four positions. Color changes the carrier ratio;
timbre changes window family and pulse balance. Strike is an event-onset
transient and sync is an event-local slower reset clock, allowing root
half-cycle phase changes. Source `wav_sine`, fixed-point pitch/glide and
integrator arithmetic are replaced. Rows 0–20 are Adaptation/Replacement,
21–46 Pending/NeedsAudit. Numeric source parity and per-sample external sync
remain open.


### Session: 2026-09-28 — compound/digital positions 13–16

`macro-digital-voice` makes TRIPLE_RING_MOD, SAW_SWARM, SAW_COMB and TOY
runnable with pitch, two timbres, event-onset strike and event-local sync.
The independently written paths use three analytic sines and soft drive,
seven analytic saws with a one-pole high-pass, a host-rate-sized 40 ms comb
ring, and a stepped sample-hold oscillator. Source overdrive, SVF filter
lookup, comb waveshaper, toy 4x oversampling/FIR and fixed-point pitch
numerics are not reproduced. The comb's feedback knob becomes audible after
its delay recirculates. Rows 0–16 are Adaptation/Replacement, 17–46 remain
Pending/NeedsAudit. Native/browser, control response and memory-budget tests
cover this bounded slice; source numeric comparisons remain open.


### Session: 2026-09-28 — triple-oscillator positions 9–12

`macro-triple-voice` selects source positions 9–12: saw, square, triangle
and sine, each summed from a root and two independently detuned analytic
oscillators. The two timbres map to an original continuous ±24-semitone
curve with a fine unison zone rather than the source 65-entry interval
array. Event-onset strike and event-local generated sync remain codeable;
the fixed phase state is allocated before rendering. Rows 0–12 are
Adaptation/Replacement, 13–46 Pending/NeedsAudit. Source table interpolation,
fixed-point waveform and per-sample external sync parity remain open.

### Session: 2026-09-28 — sub-octave and dual-sync positions 5–8

`macro-sub-sync-voice` selects source positions 5–8 with the same explicit
pitch, two timbres, event-onset strike and event-local sync controls.
Original analytic functions implement square/variable-saw base plus a
one- or two-octave square sub whose gain reaches zero at the octave switch;
the two dual-sync shapes use a pitch-shifted slave reset by the master cycle
and a timbre blend. No generated pitch table, oscillator wave, wavetable or
waveshaper is imported. These four rows are Adaptation/Replacement;
positions 9–46 remain pending. Source fixed-point, interpolation and
per-sample external sync parity remain open.

### Session: 2026-09-28 — first five analytic shapes

Added a public ordered 47-row manifest at the pinned revision. Positions
0–4 select five original analytic shape families through `macro-five-voice`;
positions 5–46 remain Pending/NeedsAudit. The source's first-five path
reads generated pitch, sine, folding, cutoff and overdrive data; none was
imported. Upstream wave-bank positions 37–40 retain an unaudited asset flag.
The Vactrol voice exposes pitch, two timbres, an event-onset strike transient
and an event-local generated hard-sync clock. Neither per-sample external
sync nor fixed-point/source-table numerical parity is claimed. Focused tests
cover distinct shape/control response, installed `.vact` editor IDs,
native/browser codec, 44.1/48/96 kHz by 64/256 frames, fixed-state budget
rejection and callback allocation probing. Further source comparisons and
positions 5–46 remain open.

### Session: 2026-09-28 — source enumeration

`braids/settings.h` at revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`
has 47 shapes before `MACRO_OSC_SHAPE_QUESTION_MARK`; the question-mark
entry is a sentinel, not a playable shape. The accessible set spans 17
analog/compound, 11 digital/formant/FM, nine physical/drum, four wave-bank,
and six noise/granular/modulation positions. `macro_oscillator.h` exposes
two timbre values, pitch, strike and a sync buffer. No Braids source or
resources have yet been imported into Vactrol. A per-file data audit is
required before BRA-002..006 can be called a port.
