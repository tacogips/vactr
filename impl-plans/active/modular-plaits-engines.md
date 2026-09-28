# Plaits Audio Engine Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-27
**Last Updated**: 2026-09-27

## Design Document Reference

This plan tracks the pinned Plaits engine slices and their source-fidelity
work. The broader family plan is `modular-synth-engines.md`; the shared
parameter, output and memory foundation is `modular-audio-foundation.md`.
A tested analytic adaptation is not counted as a complete source port.

## Modules

### `src/dsp/ported/voice.rs`

```rust
pub struct PortedEngine {
    pub position: u8,
    pub parameter_names: &'static [&'static str],
    pub output_count: u8,
    pub fidelity: EngineFidelity,
}
pub enum EngineFidelity { Adaptation, SourcePort, ResourceSubstitution }
```

The registry remains planned. Existing UGen/template implementations are
listed in the progress log; their missing source behavior stays open.

### SYN-002A: Plaits position 0 oscillator and filter

The pinned `plaits/dsp/engine2/virtual_analog_vcf_engine.{h,cc}` is MIT
licensed and has no bundled waveform or preset asset. It uses a variable-shape
oscillator, sub-oscillator, two state-variable filter stages and interpolation.
The public engine parameters are note, morph (oscillator shape, pulse width and
sub level), timbre (cutoff), and harmonics (resonance, second-stage mix and
gain). Main output is low-pass; auxiliary output is high-pass. The exact
source dependencies and their notices must be checked before calling a Rust
implementation a source port; otherwise label it an independent adaptation.

**Deliverables**: A neutral `.vact` instrument with all three sound controls
and note frequency, independent main/aux outputs, editor metadata, native and
browser graph-codec support, and tests for each control and both channels.
Its oscillator and filter state must fit the preallocated MOD-005 budget.
Do not import a lookup table, patch bank or sound asset. The Plaits voice-level
trigger/LPG handling is a separate parity requirement and must stay visibly
open if this slice omits it.

### SYN-002B: Plaits position 1 phase distortion

The pinned `plaits/dsp/engine2/phase_distortion_engine.{h,cc}` is MIT and
renders synchronized and free-running phase-distortion paths as separate
outputs at a two-times internal rate. It exposes note, harmonics (modulator
frequency quantization), timbre (phase-modulation amount), and morph
(asymmetric-triangle width). Its `lut_fm_frequency_quantizer` comes from
the MIT `plaits/resources/lookup_tables.py`, but the aggregate generated
`resources.cc` also contains DX7-derived patch data. Recompute the small
frequency-ratio mapping from the source's published algorithm or use an
original analytic equivalent; do not import the aggregate resource file,
patch banks, or unrelated waveshaper tables.

**Deliverables**: A neutral `.vact` instrument with all controls and distinct
synchronized main/free-running auxiliary outputs, finite audio at supported
rates/blocks, native/browser codec and editor tests, and fixed preallocated
state. Record whether the DSP matches the upstream engine or is an
adaptation, including any oversampling or antialias differences.

### SYN-002, position 10: two-operator FM

The pinned `plaits/dsp/engine/fm_engine.{h,cc}` uses a carrier, modulator,
feedback and sub oscillator with main/sub auxiliary outputs. Note selects
pitch; harmonics selects a ratio from the same generated quantizer reviewed
for SYN-002B; timbre sets FM amount; morph sets positive modulator feedback
or negative phase feedback. The source oversamples four times and uses a
downsampler. All source files and needed `stmlib` dependencies must be audited
before translation; an independently simplified DSP implementation must be
marked as an adaptation. No aggregate `resources.cc` or DX7 patch data may
be copied.

**Deliverables**: A neutral `.vact` FM instrument with all source controls,
audible main and sub outputs, native/browser codec, preallocated state,
control-response and cross-rate/block tests. Document the quantizer,
oversampling and output fidelity relative to the source.

#### SYN-002C2 fidelity translation

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, MOD-006
source-comparison and position-10 resource paragraphs.

**Deliverables**: Refine `src/dsp/ugen/fm_pair.rs` with only the FM-specific
quantizer and four-tap downsampler data from the individually MIT-noticed
`plaits/resources/lookup_tables.py`; do not import aggregate `resources.cc`
or any sine/wave asset. Retain the public `fm-pair-voice` controls, main/sub
outputs, preallocated state and graph/wire shape. Update
`THIRD_PARTY_NOTICES.md`, the coverage row and raw-kernel reference metrics.

**Completion Criteria**:
- [x] The finite 0..1 harmonics selector reaches the source's entire quantized ratio range; timbre and morph drive the source-shaped amount and signed feedback roles.
- [x] Four substeps, a persistent 24-frame-at-48-kHz (roughly 0.5 ms) control-interpolation clock scaled to host rate, two bounded FIR states and both outputs are preallocated; callbacks allocate nothing at 44.1/48/96 kHz and 64/256-frame blocks.
- [x] Native/browser `.vact` control and main/sub routing tests pass without changing the template or wire schema.
- [x] Pinned raw-kernel comparisons at more than one harmonics/timbre/morph setting show the remaining error; the manifest stays below `SourcePort` while analytic sine and voice/LPG differences remain.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### SYN-002, position 12: additive oscillator

The pinned `plaits/dsp/engine/additive_engine.{h,cc}` uses 24 integer-series
partials for main and eight organ-series partials for auxiliary, with
amplitude smoothing and distinct harmonic index sets. Note selects pitch;
timbre controls spectral centroid; morph controls slope; harmonics controls
spectral bumps. The oscillator depends on MIT `stmlib` DSP utilities but no
external recorded or DX7-derived resource. The small harmonic index sets
are source code data and may be ported with the required MIT notice, or
recomputed independently with any sonic difference documented.

**Deliverables**: A neutral `.vact` instrument with all four controls, two
audible outputs, bounded per-voice partial phase/amplitude state, and
native/browser, control-response, metadata and callback-allocation tests.

The subsequent fidelity pass must replace the provisional Gaussian/ripple
amplitude law with the pinned MIT engine's centroid/slope/bump envelope,
per-harmonic smoothing and normalization. Preserve independent integer and
organ paths, all four `.vact` controls, and fixed install-time state. Check
the high-frequency partial taper and distinguish any host-rate smoothing or
analytic sine differences from full source parity. Do not import generated
waveform resources.

### SYN-003, position 17: clocked noise filter

The pinned `plaits/dsp/engine/noise_engine.{h,cc}` and
`plaits/dsp/noise/clocked_noise.h` are MIT source and use no bundled sound
asset. Note sets the first filter center; harmonics selects the second
filter offset and low-pass/high-pass blend; timbre selects the sample/hold
clock; morph sets resonance. Main is a low-pass/high-pass blend; auxiliary
combines two band-pass paths. The source also resets the noise clock on a
trigger edge; `.vact` event starts need an equivalent reset contract.

**Deliverables**: A neutral `.vact` noise instrument with pitch and all three
controls, independent main/aux, deterministic per-voice random seed and
trigger reset, bounded state, native/browser and control-response tests.
Document any filter or antialias difference from upstream.

#### SYN-003A2 source-stage translation

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Plaits
position 17 paragraph, and the pinned engine/clocked-noise MIT sources.

**Deliverables**: Replace the provisional filters and hard sample-and-hold
edges in `src/dsp/ugen/clock_noise_pair.rs` with two quadratically
BLEP-corrected held-noise clocks, source-shaped frequency/gain/resonance
mapping, a multimode main SVF and two auxiliary band-pass SVFs. Preserve
the five-port `.vact`/graph contract, deterministic event restart, bounded
preallocated node state and all native/browser routing. Update coverage and
`THIRD_PARTY_NOTICES.md` without importing lookup tables or audio assets.

**Completion Criteria**:
- [x] Both clocks and three filter paths follow the source signal topology; each parameter reaches its documented role.
- [x] Both outputs are finite, audible and distinct at 44.1/48/96 kHz and 64/256-frame blocks; late starts and event retriggers retain deterministic behavior and callbacks allocate nothing.
- [x] Native/browser codec and `.vact` control/routing tests pass; coverage remains below `SourcePort` unless full voice/source parity is independently measured.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### SYN-003, position 21: dual bass-drum generators

The pinned `plaits/dsp/engine/bass_drum_engine.{h,cc}` renders an analog
808-style model to main and a synthetic bass drum to auxiliary. Its MIT
`plaits/dsp/drums/analog_bass_drum.h` and `synthetic_bass_drum.h` carry
separate pulse, resonator, transient, pitch-envelope and self-FM state.
Note sets pitch; timbre, morph and harmonics govern tone, decay, excitation,
FM and drive; trigger and accent alter each hit. This is distinct from the
Peaks-inspired `low-drum` already present. Source constants assume a fixed
sample rate, so timing must be converted to the host rate.

**Deliverables**: A neutral `.vact` drum instrument exposing pitch and all
three timbre controls plus accent/velocity and retrigger behavior, with
independent analog/synthetic outputs. State must be preallocated and the
native/browser behavior checked at multiple rates/blocks. Source code is
MIT; preserve notice, avoid table-bearing helpers and disclose any reduced
fidelity. Keep touched Rust source files below 1000 lines by splitting tests
and implementation modules when needed.

### SYN-003C: source-fidelity pass for position 21

Replace the provisional analog and synthetic kernels with implementations
of the pinned MIT source stages. The analog path must retain the diode
response, trigger and FM pulse shapes, SVF resonator/low-pass feedback,
exciter leakage, tone filtering and overdrive. The synthetic path must
retain its FM/body/transient envelopes, click and filtered attack-noise
generators, distorted sine, transistor VCA, tone filter and sustain path.
Allocate extra state at template installation as needed. Convert source
sample-count timing to the host rate without callback allocation. Keep the
existing `.vact` control names and both output channels. Add an end-to-end
test that schedules two hits so retrigger behavior is observed through the
scheduler, not solely by resetting `NodeState` in a kernel test. Record any
remaining numerical parity difference against the pinned source.

### SYN-003D: position 22 dual snare-drum generators

The pinned MIT `plaits/dsp/engine/snare_drum_engine.{h,cc}` renders
`analog_snare_drum.h` to main and `synthetic_snare_drum.h` to auxiliary.
Both paths take note/pitch, timbre, morph, harmonics, accent and trigger;
the source also has an unpatched-trigger sustain convention. Translate their
signal stages into separate bounded Rust state modules, using host-rate
timing and a per-voice seeded random stream in place of the global upstream
stream. Keep the existing Peaks-inspired `wire-drum` separate. No generated
resource or sine table is imported; document analytic-sine and filter
differences. Add a scheduler two-hit test and cross-rate native/browser tests.

### SYN-003E: position 23 dual hi-hat generators

The pinned MIT `plaits/dsp/engine/hi_hat_engine.{h,cc}` instantiates
`plaits/dsp/drums/hi_hat.h` twice: a square-noise/SwingVCA main path and a
ring-mod-noise/LinearVCA auxiliary path. Translate both variants' excitation,
filtering, envelope and accent/trigger behavior with host-rate timing and
preallocated per-voice state. Expose note/pitch, timbre, morph, harmonics,
velocity/accent and sustain through neutral `.vact` controls. Keep the
existing Peaks-inspired `metal-hat` separate. No upstream oscillator table,
aggregate resource or unrelated asset may be imported. Test the two
channels, scheduled retrigger, parameter response and callback budget on
native and browser tiers.

### SYN-003F: position 16 swarm oscillator

The pinned MIT `plaits/dsp/engine/swarm_engine.{h,cc}` uses eight ranked
voices with randomized grain envelopes, a BLEP-corrected additive saw main
output and a sine auxiliary output. Note sets pitch; harmonics sets spread;
timbre sets grain density; morph sets size/texture. A trigger starts a burst,
while an unpatched trigger yields a continuous swarm. Translate those
states with fixed per-voice storage and deterministic per-voice randomness;
use an analytic sine in place of the source lookup table. Keep both outputs
independent and test burst versus continuous behavior, all `.vact` controls,
state-budget rejection, native/browser rates/blocks and callback allocation.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| SYN-002A | Implement position 0 oscillator/filter slice with main/aux and control-response tests | MOD-003, MOD-004 bounded, MOD-005 bounded | Bounded adaptation complete; LPG/fidelity pending |
| SYN-002B | Implement position 1 phase-distortion slice with two outputs and cleared quantizer | MOD-003, MOD-004 bounded, MOD-005 bounded | Bounded adaptation complete; source parity/LPG pending |
| SYN-002C | Implement position 10 two-operator FM with carrier/sub and signed feedback | MOD-003, MOD-004 bounded, MOD-005 bounded | Bounded adaptation complete; source parity/LPG pending |
| SYN-002C2 | Translate individually audited MIT FM quantizer/FIR and source control, feedback and downsampling stages; compare raw outputs | SYN-002C, MOD-006A | Completed and independently verified; full voice/LPG/source parity remains separate |
| SYN-002D | Implement position 12 additive integer/organ main/aux voice | MOD-003, MOD-004 bounded, MOD-005 bounded | Bounded adaptation complete; source parity/LPG pending |
| SYN-002D2 | Translate position 12 amplitude stages and verify main/aux response | SYN-002D | Source-stage adaptation complete; source numerical comparison and LPG remain separate tasks |
| SYN-003A | Implement position 17 clocked-noise main/aux with event reset | MOD-003, MOD-004 bounded, MOD-005 bounded | Bounded adaptation complete; source parity pending |
| SYN-003A2 | Translate position 17 BLEP clocks and three SVF signal paths | SYN-003A | Completed and independently verified; numerical comparison and voice/LPG parity remain separate |
| SYN-003B | Implement position 21 dual bass-drum main/aux with accent and retrigger | MOD-003, MOD-004 bounded, MOD-005 bounded | Bounded adaptation complete; source parity pending |
| SYN-003C | Replace position 21 approximations with source stages and scheduled retrigger test | SYN-003B | Source-stage translation complete; bit-exact parity not claimed |
| SYN-003D | Translate position 22 analog/synthetic snare stages and verify main/aux | MOD-003, MOD-004 bounded, MOD-005 bounded | Source-stage translation complete; bit-exact parity not claimed |
| SYN-003E | Translate position 23 square/ring-mod hi-hat variants and verify main/aux | MOD-003, MOD-004 bounded, MOD-005 bounded | Source-stage translation complete; bit-exact parity not claimed |
| SYN-003F | Translate position 16 eight-voice swarm with saw/sine outputs | MOD-003, MOD-004 bounded, MOD-005 bounded | Source-stage translation complete; bit-exact parity not claimed |

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| UGen kernels | `src/dsp/ugen/` | Fifteen runnable adaptations and nine source-stage translations across the Plaits inventory | Native/browser render |
| .vact templates | `src/prelude/templates.vact` | Nine neutral instruments | Control response and editor metadata |
| Provenance | `THIRD_PARTY_NOTICES.md` | Nine audited references | Source and asset review |
| Fidelity registry | `src/dsp/ported/voice.rs` | Not started | - |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Full Plaits ports | `modular-audio-foundation.md` | Partial shared foundation |
| Source comparison | `MOD-006` | Not started |
| Cleared resource engines | `MOD-001` resource audit | Partial |

## Completion Criteria

- [ ] All 24 registered positions have a cleared renderer or explicit unavailable-resource status.
- [ ] Every sound parameter and main/aux output is accessible through .vact and the editor.
- [ ] Trigger/LPG and source algorithm behavior have comparison evidence.
- [ ] Asset and source notices are complete for each included engine.
- [ ] Native/browser tests pass with bounded callback state and no allocation.

## Progress Log

### Session: 2026-09-28, SYN-002C2 FM source-stage translation

The pinned `plaits/resources/lookup_tables.py` has an individual Emilie
Gillet MIT notice. Its 23 FM ratio anchors and four downsampler FIR
coefficients are the only source numeric data translated into Vactrol;
compile-time expansion matches the pinned generated FM quantizer within
0.000001 semitone. No aggregate resource, sine wavetable, ROM, preset, wave
or LXR data is imported. The FM kernel now follows the source ratio,
high-frequency amount, signed-feedback, four-substep and two-output FIR
stages using preallocated state and analytic sine.

The pinned 48 kHz/24-frame raw-kernel comparison runs three control
settings. Values below are correlation / normalized RMS error, before →
after SYN-002C2:

| Harmonics / timbre / morph | Main | Aux |
|---|---|---|
| 0.10 / 0.25 / 0.20 | 0.844 / 0.559 → 0.999990 / 0.004558 | 0.980 / 0.203 → 0.9999998 / 0.000580 |
| 0.50 / 0.50 / 0.50 | -0.094 / 1.525 → 0.996815 / 0.07998 | 0.847 / 0.587 → 0.999083 / 0.04282 |
| 0.90 / 0.75 / 0.80 | 0.078 / 1.424 → 0.9999986 / 0.001636 | 0.603 / 0.977 → 0.9999990 / 0.001405 |

These are raw-kernel measurements at one note, not full `.vact` voice,
trigger, LPG, browser, source-bit-exact or all-rate comparisons. The middle
setting still has numerical error. Retain analytic-sine and phase
quantization differences in the coverage inventory.
The first independent review found that control interpolation incorrectly
used host callback length rather than the source's fixed 24-frame period.
The corrected internal clock persists across callbacks and scales to 22/24/48
frames at 44.1/48/96 kHz. A regression varies all four audio-rate controls
at nonaligned sample positions and confirms exact equality of both outputs
across 64/256-frame and shortened-first-call partitions. Separate native and
browser host tests schedule late starts and assert zero callback allocations
across the rate/block matrix. The second independent review reproduced all
three pinned comparisons and passed formatting, native/no-default/wasm checks,
strict Clippy, Taplo, 19 mise task validations and the full Cargo suite
(1,424 passed, one ignored). This completes SYN-002C2, not full Plaits voice
or LPG parity.

### Session: 2026-09-27, public 24-position coverage inventory

`src/dsp/ported/manifest.rs` records the exact pinned Plaits registry order and a checked fidelity/resource status for every position. The current count is five runnable adaptations, four source-stage translations, eleven pending positions and four unavailable published ROM-backed modes; zero positions are verified full source ports. DX7 ROM patch-bank and TI ROM speech-word modes are explicitly flagged. Position 13 wavetable remains pending asset audit: the MIT generator reads `waves.bin`, whose individual origin remains unaudited. Four wave-asset positions and other pending engines retain individual resource-audit work. A public summary and template/editor/dual-output consistency tests make partial progress inspectable without claiming complete Plaits coverage.

### Session: 2026-09-27, SYN-003F position 16 eight-voice swarm

**Source and license**: Pinned Plaits `swarm_engine.{h,cc}`, oscillator headers and pinned MIT `stmlib` BLEP/units/random dependencies were inspected. `THIRD_PARTY_NOTICES.md` preserves Emilie Gillet copyright and MIT permission/warranty terms. No aggregate resources, sine lookup, DX7/LXR data or external audio assets are imported.
**Implementation**: `swarm-voice` exposes note/frequency, `swarm-spread` (authored harmonics role), timbre/density, morph/grain size and explicit `swarm-continuous` trigger convention through `.vact` and editor metadata. Each main/aux output node owns eight ranked grain envelopes and fixed oscillator state; main sums the source two-sample BLEP saw, auxiliary sums analytic sine with the source high-frequency gain taper. Per-node state is 112 floats, 224 per two-output voice, allocated before callbacks; a 223-float install budget fails. Event start retriggers bursts; continuous mode starts a free-running cloud and ends with the Vactrol event gate.
**Verification**: Focused both-path control, burst/continuous, native/browser 44.1/48/96 kHz and 64/256-frame, scheduled two-hit `.vact`, editor, aux and callback allocation tests pass. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1054 library tests plus integration suites), rustfmt and diff checks pass. Every touched Rust source file remains below 1000 lines.
**Numerical differences**: Analytic sine replaces the source grain-envelope lookup and recursive fast-sine oscillator; per-voice seeded randomness replaces global `stmlib` randomness; host-rate pitch/density differs slightly from source corrected-rate tuning. This is a source-stage MIT translation, not bit-exact firmware. Other Plaits positions remain open.

### Session: 2026-09-27, SYN-003E position 23 dual hi-hat translation

**Source and license**: Pinned Plaits hi-hat engine, `drums/hi_hat.h`, oscillator header and exact pinned MIT `stmlib` DSP dependencies were inspected. `THIRD_PARTY_NOTICES.md` preserves Emilie Gillet copyright and MIT permission/warranty terms. The six square ratios and three ring-pair frequency maps are MIT source data; no upstream oscillator table, aggregate resource, DX7/LXR data or external audio asset is imported.
**Implementation**: `dual-hat-voice` has a square-noise/SwingVCA main with the source six ratios and resonant metallic SVF; its auxiliary has three square/saw ring-modulation pairs, nonresonant metallic SVF, two-stage cut decay and LinearVCA. Both include clocked-noise blend from `hat-harmonics`, pitch, timbre-controlled cutoff, morph decay, velocity/accent, explicit gated `hat-sustain`, event retrigger and final highpass. Each node reserves 16 floats before audio callbacks (32 per voice), with explicit 31-float budget rejection. Source 48 kHz coefficients and 150 Hz–16 kHz cutoff map to the host rate.
**Verification**: Focused both-path control, native/browser 44.1/48/96 kHz and 64/256-frame, scheduled two-hit `.vact`, editor, aux and callback allocation tests pass. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1048 library tests plus integration suites), rustfmt and diff checks pass. Every touched Rust source file remains below 1000 lines.
**Numerical differences**: Float phases replace source uint32 square phases; analytic square/saw edges replace the source anti-aliased oscillator; a per-voice RNG replaces global randomness; exact tangent replaces the source filter approximation. This is a source-stage MIT translation, not bit-exact firmware. Other Plaits positions remain open.

### Session: 2026-09-27, SYN-003D position 22 dual snare translation

**Source and license**: Pinned Plaits snare engine, analog and synthetic snare headers, and exact pinned MIT `stmlib` filter/DSP helpers were inspected. `THIRD_PARTY_NOTICES.md` preserves Emilie Gillet copyrights and the MIT permission/warranty terms. No source sine table, aggregate resources, DX7/LXR data or audio assets are imported.
**Implementation**: `dual-snare-voice` has an analog main with five source-ratio SVF shell modes, tone-dependent gains, pulse/exciter leak, positive-half-wave snare noise envelope, bandpass and soft clip. Its synthetic auxiliary has two coupled 1:1.47 oscillators, FM/drum/snare envelopes, a 40–70 ms snare hold, and source-style drum lowpass plus snare low/high filters. Note/frequency, `snare-harmonics` (snappy), timbre, morph, velocity/accent and explicit gated `snare-sustain` reach both paths. Each path reserves 24 floats at install (48 per voice), and a 47-float budget fails explicitly. Trigger resets are driven by the scheduler's new-event voice start.
**Verification**: Focused both-path control, retrigger, native/browser 44.1/48/96 kHz and 64/256-frame, scheduled two-hit `.vact`, editor, aux and allocation tests pass. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1042 library tests plus integration suites), rustfmt and diff checks pass. Every touched Rust source file remains below 1000 lines.
**Numerical differences**: Analytic sine replaces the source lookup, per-voice RNG replaces global randomness, exact tangent replaces the source fast tangent approximation, source inter-block pitch interpolation is absent, and some filter coefficients are converted to host-rate equivalents. This is a source-stage MIT translation, not bit-exact firmware. Other Plaits positions remain open.

### Session: 2026-09-27, SYN-003C position 21 source-stage translation

**Source and license**: Pinned Plaits bass-drum generators and overdrive, plus exact pinned MIT `stmlib` SVF and DSP helpers, were inspected. `THIRD_PARTY_NOTICES.md` preserves their Emilie Gillet copyright, MIT permission and warranty terms. No source sine table, aggregate resources, DX7/LXR data or audio assets were imported.
**Implementation**: Replaced provisional analog path with trigger/FM pulse counters, diode, TPT SVF bandpass/lowpass feedback, exciter leakage, tone stage and source overdrive transfer. Replaced provisional synthetic path with FM/body/transient envelopes, phase hold, click SVF, filtered attack noise, distorted analytic sine, transistor VCA, tone and gated sustain. `kick-sustain` is a `.vact` and editor-visible parameter; event velocity remains accent. Each path reserves 24 floats at installation (48 per dual-output voice), and install rejects a 47-float budget. Timing and source one-pole rates follow the host sample rate.
**Verification**: Focused node reset/accent, both-path sound-port response, native/browser rate-block and scheduled two-hit retrigger tests pass. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1036 library tests plus integration suites), rustfmt and diff checks pass. The real-time render and browser tests retain zero-allocation probes; touched Rust source files remain below 1000 lines.
**Numerical differences**: Analytic sine replaces the source lookup; a per-voice RNG replaces global `stmlib` random; exact `tan` replaces the source dirty-tangent approximation; Vactrol does not interpolate source pitch across a render block, and its gated sustain gain follows a host-rate one-pole rather than the source block interpolator. This is a source-stage MIT translation, not bit-exact firmware. Other Plaits engines and global LPG behavior remain open.

### Session: 2026-09-27, SYN-003 position 21 dual bass-drum slice

**Source and license**: Pinned Plaits `bass_drum_engine.{h,cc}`, `analog_bass_drum.h`, and `synthetic_bass_drum.h` are Emilie Gillet MIT sources. `THIRD_PARTY_NOTICES.md` records the source and copyright. No upstream implementation, sine table, aggregate resources, DX7 data, or LXR data is imported.
**Implementation**: `dual-kick-voice` uses separate triggered analog-style main and synthetic auxiliary nodes. Note/frequency, `kick-harmonics`, timbre, morph, and event velocity/accent route into both paths. A new event clears node state and retriggers both. Pulse and envelope times use host sample rate. Fixed inline node state requires no additional voice memory or callback allocation. Existing `templates.rs` tests were split before adding new tests, keeping touched Rust files below 1000 lines.
**Verification**: Kernel tests cover retrigger and accent response for each path; native and browser codec tests cover 44.1/48/96 kHz and 64/256-frame blocks, finite distinct outputs and allocation probes. End-to-end `.vact` tests cover control response, editor metadata and aux routing. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1034 library tests plus integration suites), rustfmt and diff checks pass; all touched Rust files are below 1000 lines.
**Remaining**: This is a bounded analytic adaptation, not a source-faithful port. The source diode, state-variable resonator, transistor VCA, exact envelope/drive equations and sustain mode remain. The full SYN-003 task and other Plaits positions remain open.

### Session: 2026-09-28, SYN-003A2 position 17 clocked-noise stages

The pinned Plaits noise and clocked-noise sources and `stmlib` BLEP/SVF helpers carry Emilie Gillet MIT notices. The kernel now has two quadratic-BLEP held-noise clocks, source-shaped clock/filter frequencies, gain/Q, a signed triangular LP/BP/HP main SVF and two separate band-pass auxiliary SVFs. All five `.vact` ports and deterministic event restart remain; fixed inline state persists a rate-scaled 0.5 ms control clock without callback allocation. No lookup, aggregate resource or other sound asset was imported.
Independent review verified source control equations, BLEP signs, three filter states and the node-local state layout. Exact partition tests with changing audio-rate controls, native/browser 44.1/48/96 kHz × 64/256 late starts, armed allocation probes, two-hit retrigger, editor and `.vact` controls pass. Quiet native/no-default/wasm checks, strict Clippy, rustfmt, Taplo, 19 mise task validations and full Cargo tests pass (1,429 passed, one ignored). The manifest is `SourceStage`: per-voice RNG, exact-tangent versus polynomial SVF, host timing, event-local sync, output bound and voice/LPG behavior still differ from firmware.

### Session: 2026-09-28, SYN-002D2 additive amplitude stages

The position-12 kernel now follows the pinned MIT source's centroid/margin,
cubic morph/harmonics slope, rectified bump gain, normalization, organ index
map and Nyquist partial taper. Analytic sine replaces the generated sine
resource, and a host-rate one-pole coefficient replaces the source 0.001
once-per-12-sample 48 kHz block update. Vactrol keeps unnormalized smoothed
amplitudes and does not reproduce the source's normalized-state recurrence,
linear block interpolation or Chebyshev oscillator, so the manifest advances
only to `SourceStage`; exact numerical comparison, trigger/LPG and full voice
parity remain open. Main/aux and the 96-float paired install budget remain.

### Session: 2026-09-27, SYN-002 position 12 analytic additive slice

**Source and license**: Pinned Plaits `additive_engine.{h,cc}`, `harmonic_oscillator.h`, sine oscillator header and pinned `stmlib/dsp/cosine_oscillator.h` carry Emilie Gillet MIT notices. The eight-stop organ harmonic map is preserved in one-based form with notice in `THIRD_PARTY_NOTICES.md`. No source oscillator, sine table, aggregate resource, DX7 patch or LXR data is imported.
**Implementation**: `spectrum-voice` exposes pitch/note, timbre/centroid, morph/slope and the original harmonics bump role as `spectrum-bumps` (avoiding the existing `harmonics` effect name). Two fixed-state `spectrum-pair` UGen nodes emit 24 integer partials to main and eight organ partials to `aux-out`. Each node reserves 48 floats for phase and smoothed amplitudes at template install; the paired voice requires 96 floats and fails explicitly below that budget. The original Gaussian/ripple weighting differs from source amplitude equations.
**Verification**: Native/browser serialized graph tests cover finite, audible, distinct main/aux spectra at 44.1/48/96 kHz and 64/256-frame blocks with callback allocation probes. An install test rejects 95-float voice memory and accepts 96. End-to-end `.vact` tests exercise pitch and all three sound controls, editor metadata and prelude aux routing. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1026 library tests plus integration suites), rustfmt and diff checks pass; touched Rust files stay below 1000 lines.
**Remaining**: Source-exact amplitude normalization/smoothing, harmonic oscillator response, and Plaits voice-level trigger/LPG behavior remain open. Other Plaits positions are not claimed.

### Session: 2026-09-27, SYN-002 position 10 analytic FM slice

**Source and license**: The pinned `plaits/dsp/engine/fm_engine.{h,cc}` and sine oscillator/downsampler dependencies carry Emilie Gillet MIT notices; relevant pinned `stmlib` DSP dependencies are MIT. The source sine and downsampler depend on generated `resources.h` tables, so Vactrol imports no source code, FIR coefficients, sine/quantizer table, aggregate `resources.cc`, DX7 patch data or LXR asset. `THIRD_PARTY_NOTICES.md` records provenance.
**Implementation**: `fm-pair-voice` exposes note/frequency, `fm-harmonics` (the authored harmonics ratio role, renamed to avoid the existing `harmonics` effect), timbre/FM amount and morph/signed feedback. Two fixed-inline-state UGen nodes run the same fourfold analytic carrier/modulator/sub process; one emits carrier main and the other sends sub to `aux-out`. The original equal-step ratio map from SYN-002B replaces the source quantizer. A simple box average replaces the source FIR; there is no callback allocation or resource dependency.
**Verification**: Native/browser serialized graph tests cover finite, audible, distinct main/sub outputs at 44.1/48/96 kHz and 64/256-frame blocks with callback allocation probes. End-to-end `.vact` tests exercise pitch, ratio, timbre and both signs of feedback plus editor metadata and prelude aux routing. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1022 library tests plus integration suites), rustfmt and diff checks pass; touched Rust files stay below 1000 lines.
**Remaining**: Source-exact quantizer, sine/FIR assets, interpolation, anti-aliasing, output equivalence and Plaits voice-level trigger/LPG behavior remain open; the other Plaits positions are not claimed.

### Session: 2026-09-27, SYN-002B analytic phase-distortion slice

**Source and license**: The pinned Plaits `phase_distortion_engine.{h,cc}`, variable-shape/sine oscillator headers, `lookup_tables.py`, and used `stmlib` DSP dependencies carry Emilie Gillet MIT notices. No generated `resources.cc`, DX7 patch bank, lookup table, oscillator or `stmlib` implementation is imported. `THIRD_PARTY_NOTICES.md` records provenance.
**Implementation**: `phase-pair-voice` exposes pitch/note, `phase-harmonics` (the original harmonics role, renamed to avoid the existing `harmonics` effect), timbre and morph. An original analytic 25-step ratio map replaces the Plaits quantizer; two inline-state nodes average two substeps/sample, with the free-running path sent to `aux-out` and the synced path to main. The graph codec needs only a new UGen kind, no external resource or callback allocation.
**Verification**: Native/browser serialized graph tests show finite, audible, distinct main/aux outputs at 44.1/48/96 kHz and 64/256-frame blocks with allocation-free callbacks. End-to-end `.vact` and editor tests show pitch and all three sound controls respond, and the prelude graph contains a free-running aux branch. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1019 library tests plus integration suites), rustfmt and diff checks pass; touched Rust files stay below 1000 lines.
**Remaining**: This is not bit-exact Plaits DSP: curated frequency ratios, oscillator antialiasing, interpolation/downsampling details, and voice-level trigger/LPG behavior are not reproduced. Other Plaits positions remain pending.

### Session: 2026-09-27, SYN-002A analytic oscillator/filter slice

**Source and license**: The pinned Plaits `virtual_analog_vcf_engine.{h,cc}` and its variable oscillator headers carry Emilie Gillet MIT notices. The pinned `stmlib` submodule DSP filter, interpolator and polyblep dependencies are MIT; the unrelated STM and serial-programming exceptions are not used. `THIRD_PARTY_NOTICES.md` records provenance and copyright. The Vactrol implementation independently uses finite harmonic synthesis and simple analytic filters; it imports no code, waveform table or asset and is not bit-exact.
**Implementation**: `filter-voice` exposes note/frequency, morph, timbre and the original harmonics role as `filter-harmonics` (avoiding the existing `harmonics` effect name). Two synchronized source/filter branches feed low-pass main and high-pass `aux-out`; fixed inline node state is preallocated with voices, and the existing graph codec carries both UGen kinds. Scalar rows and editor metadata include all sound controls.
**Verification**: Native/browser graph install, codec, distinct finite audible outputs, allocation-free callbacks, end-to-end `.vact` parameter response and editor metadata tests pass. `CARGO_TERM_QUIET=true cargo check`, strict all-target Clippy, full `cargo test` (1016 library tests plus integration suites), rustfmt and diff checks pass; touched Rust files stay below 1000 lines.
**Remaining**: Plaits' voice-level trigger/LPG behavior, anti-aliasing/filter response parity, and the other 23 Plaits positions are not covered by this adaptation. Generic arbitrary dual-output node edges remain a separate MOD-004 limitation.
