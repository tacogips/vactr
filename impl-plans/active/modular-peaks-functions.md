# Peaks Audio Functions and Percussion Coverage

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

The pinned Peaks `processors.h` registers twelve functions: envelope,
LFO, tap LFO, four drums, pulse shaper, pulse randomizer, bouncing ball,
mini sequencer and number station. This plan splits the audio-generating
and audio-rate modulation work from the sequencer-only exclusions in the
main Mutable scope. `low-drum`, `wire-drum` and `metal-hat` are playable
architectural adaptations; `phase-drum` translates source signal stages
with analytic resource replacements. None is a complete source port.
Alternate settings still need a checked fidelity inventory.

## Resource Boundary

`peaks/data/digits.bin` is consumed by Number Station through generated
resources. Its individual origin and redistribution terms are not
established by the firmware license. Do not import it or aggregate
`resources.cc`; a speech/number replacement needs original content or a
user-provided data contract. Other generated waveform/lookup tables
require per-generator audit. Sequencer-only output does not by itself
count as an audio synth or effect port.

## Modules

### `src/dsp/ported/peaks.rs`

```rust
pub struct PeaksFunctionSpec {
    pub function: u8,
    pub source_name: &'static str,
    pub vactr_name: Option<&'static str>,
    pub audio_role: bool,
    pub status: Fidelity,
    pub resources: ResourceState,
}
pub fn functions() -> &'static [PeaksFunctionSpec; 12];
```

Expose every source sound-relevant parameter, alternate mode and trigger
from `.vact` and editor metadata. Use the published source revision and
per-file notice for translated MIT stages; independently written DSP is
an adaptation. Keep the callback bounded and allocation-free.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| PKS-001 | Twelve-function source/alternate/resource inventory | SYN-007 | Checked twelve-row public manifest; later functions still need deeper audit |
| PKS-002 | Bass/snare/hat/FM source comparison and missing drum controls | PKS-001 | Numeric bass/snare/hat/FM comparison probe added and run (`compare-peaks-drums`); measured gaps recorded below, no fidelity label changed; presets and other drum fidelity still open |
| PKS-003 | Envelope, LFO and tap-LFO audio-rate functions | PKS-001 | Runnable analytic adaptations; source numerical parity pending |
| PKS-004 | Pulse shaper/randomizer and bouncing-ball audio-rate roles | PKS-001 | Runnable analytic adaptations; source numerical/buffer parity pending |
| PKS-005 | Number Station cleared original speech/data path or unavailable diagnostic | PKS-001 | Original ten-syllable tone/voice replacement runnable; source recording parity excluded |
| PKS-006 | Mini-sequencer exclusion decision and end-to-end verification | PKS-001..005 | Not started |

## Completion Criteria

- [x] Twelve source registrations have truthful audio/control/excluded status.
- [ ] Every audio-producing function, alternate mode and parameter is codeable in `.vact` and editor metadata.
- [x] Uncleared digits binary and generated aggregate resources are excluded.
- [x] Seven function adaptations pass native/browser rate, output, capacity and callback-allocation tests.
- [ ] Source comparisons and notices distinguish adaptations from ports.
- [x] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass for this slice.

## Progress Log

### Session: 2026-09-28 — source inventory

`processors.h` and `processors.cc` at revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` register twelve
functions. Four drum architectural adaptations already exist in Vactr;
their source-level parity is unverified. The `digits.bin` asset has not
been cleared and is excluded. No additional Peaks source/data was
imported by this inventory step.

### Session: 2026-09-28 — first three functions

`peaks_functions()` inventories all twelve source registrations. Positions
0–2 use `peak-motion-voice`: full ADSR / half AD, full LFO shape/parameter/
reset and half seven-preset LFO, plus tapped-period LFO with half/full roles.
The two host outputs are an analytic main and a Vactr bipolar/quarter-cycle
auxiliary extension; they are not claimed to match simultaneous Peaks output
timing or phase exactly. `peak-trigger`, `peak-gate`, `peak-tap`, and
`peak-sync` are event-local controls; tap tempo requires two rising edges in
one event. Analytic curves and seeded random motion replace source fixed-point
processing, generated waveform/lookup tables and noise state. Drum positions
3–6 remain architectural adaptations pending source parity review. Pulse
functions 7–9 remain pending, Mini Sequencer is excluded from audio scope,
and Number Station remains unavailable because `digits.bin` provenance is
unresolved.

### Session: 2026-09-28 — pulse and bounce functions

`peak-pulse-voice` now runs positions 7–9: a triggered pulse shaper with
pre-delay, duration, interval and repetition; a seeded probability/delay
randomizer; and a gravity/rebound bouncing ball. Full and half configurations
follow the source control-role mappings. The half shaper derives interval from
duration; the half randomizer fixes acceptance and delay randomness; the half
ball fixes height and initial velocity. Separate gate and trigger rising edges
can restart an event-local function. Main emits gate/height, while auxiliary
emits a short onset/impact pulse as a Vactr extension. Source 32-pulse
overlap queues, tick-rate quantization, generated delay/gravity LUTs and
random sequence are not translated: one new trigger replaces the current
bounded pulse train, timing is host-rate analytic, and randomness is seeded
per voice. Rows 7–9 are `Adaptation/Replacement`, not source ports. PKS-002,
PKS-005 and PKS-006 remained open at this step.

### Session: 2026-09-28 — original Number Station replacement

`number-station-voice` provides a tone mode and a voice mode with ten
independently authored procedural digit syllables. A digit selector makes all
ten accessible; optional event-local automatic selection applies the exposed
transition probability. `station-tone`, `station-noise`, `station-drive`,
gate and trigger are codeable, while half mode fixes noise/drive to their
source middle-control roles. Main emits processed audio and auxiliary emits
the dry generated tone/voice as a Vactr extension. These original formant
trajectories are not a reconstruction of the recorded source digits, language
timing, offsets or timbres. No `digits.bin`, `wav_digits`, generated sine/fold
table or source digit offset is imported. The coverage row is
`Adaptation/Replacement` with an explicit upstream digit-asset provenance
flag; the recording's origin remains unaudited. PKS-002 and PKS-006 remain
open, and full source parity is not claimed.

### Session: 2026-09-29 — PKS-002 bass/snare/hat/FM drum comparison probe

An opt-in `mise run compare-peaks-drums` task adds a local numeric
comparison for the four drum kernels, following the same pattern as
MOD-006A: `verification/peaks_drums_reference.cc` (an original driver that
`#include`s the pinned `peaks/drums/{bass_drum,snare_drum,high_hat,
fm_drum}.{h,cc}` and links the aggregate `peaks/resources.cc` and
`stmlib/utils/random.cc` from a separate checkout, never into Vactr),
`examples/peaks_drums_reference.rs` (drives `analog_percussion::render`
modes 0/1/2 and `fm_drum::render` directly), and
`verification/compare_peaks_drums.py`. Both probes trigger one hit at the
source's own 48 kHz rate (`peaks/io_buffer.h`'s 4-frame block size does not
change per-sample output, so both probes render one continuous 2.5 s
buffer) and report RMS, peak, correlation and normalized RMS error after
onset alignment, spectral centroid, and -20/-40 dB decay times.

Frequency scenarios convert each source drum's own linear pitch-code
formula (`set_frequency` in `bass_drum.h`/`snare_drum.h`/`fm_drum.h`, a
128-units-per-semitone code referenced from A4 = MIDI 69 = 440 Hz) to Hz
for the Vactr side; this is arithmetic on the visible source formula, not
a value read from a generated table. Punch/tone/snappy/metal share a 0..1
range by naming convention only, since the underlying resonance/lowpass
curves differ. Decay has no closed-form correspondence (the source sets
SVF resonance, not a time constant), so both sides use independent
low/medium/high representative settings. The source high-hat's `Configure`
takes no controls at all, so its three scenarios vary only the Vactr side
against one fixed source render. The source FM drum's single `noise`
parameter is dual-purpose (values above its midpoint mix noise, below it
add overdrive); scenarios hold it at each end and at the shared midpoint
boundary against a Vactr `drum-noise`-only or `drive`-only setting.
Vactr's `pitch-sweep` and the FM drum's `fm-amount` absolute semitone
range have no source counterpart and are documented as extensions/rescalings
in `verification/compare_peaks_drums.py`'s module docstring.

Measured metrics (low/mid/high or clean/noisy/driven scenarios; `corr` and
`nrmse` are computed after onset alignment; centroid in Hz; decay times in
seconds, `null` = not reached within the 2.5 s probe):

| Drum | Scenario | ref/vactr RMS | corr | nrmse | ref/vactr centroid | ref/vactr -20 dB | ref/vactr -40 dB |
|---|---|---|---|---|---|---|---|
| bass (low-drum) | low | 0.065 / 0.123 | 0.204 | 1.948 | 51 / 172 | 0.125 / 0.375 | 0.255 / 0.710 |
| bass | mid | 0.112 / 0.200 | -0.274 | 2.274 | 61 / 382 | 0.370 / 0.935 | 0.835 / 1.895 |
| bass | high | 0.174 / 0.315 | -0.005 | 2.072 | 138 / 635 | 1.425 / 2.420 | null / null |
| snare (wire-drum) | low | 0.025 / 0.052 | 0.004 | 2.321 | 5141 / 9853 | 0.070 / 0.335 | 0.160 / 0.695 |
| snare | mid | 0.029 / 0.069 | 0.013 | 2.568 | 8240 / 11619 | 0.065 / 0.575 | 0.155 / 1.135 |
| snare | high | 0.044 / 0.142 | 0.013 | 3.388 | 9370 / 12468 | 0.120 / 1.760 | 0.210 / null |
| hat (metal-hat) | low | 0.016 / 0.032 | 0.016 | 2.203 | 12506 / 12605 | 0.075 / 0.185 | 0.135 / 0.365 |
| hat | mid | 0.016 / 0.022 | 0.009 | 1.662 | 12506 / 12665 | 0.075 / 0.285 | 0.135 / 0.555 |
| hat | high | 0.016 / 0.030 | 0.022 | 2.107 | 12506 / 12269 | 0.075 / 0.685 | 0.135 / 1.380 |
| fm (phase-drum) | clean | 0.064 / 0.078 | 0.307 | 1.312 | 202 / 197 | 0.095 / 0.135 | 0.155 / 0.225 |
| fm | noisy | 0.062 / 0.073 | -0.037 | 1.566 | 11405 / 11990 | 0.215 / 0.215 | 0.355 / 0.360 |
| fm | driven | 0.315 / 0.255 | 0.048 | 1.255 | 2676 / 1951 | 0.650 / 0.425 | 0.805 / 0.530 |

These are measured gaps, not test failures or evidence of source parity.
The bass, snare and hat correlations are near zero or negative, and RMS,
spectral centroid and decay times differ by roughly 2-5x in most
scenarios, consistent with `low-drum`/`wire-drum`/`metal-hat` being
independent analytic adaptations (different exciter/resonator
architectures), not source-stage translations; their coverage rows stay
`Adaptation`. The FM drum tracks closer on RMS, spectral centroid and
decay for the `noisy`/`driven` scenarios (its shared sine/envelope/noise
architecture is a source-stage translation), but correlation stays low and
the `clean` scenario shows the source's few-sample excitation-delay onset
that Vactr's kernel does not reproduce; its coverage row stays
`SourceStage`, not `SourcePort`. No kernel change was made from this
probe: the differences match the already-documented adaptation/
source-stage boundaries rather than an implementation bug, so no fidelity
label changed. `CARGO_TERM_QUIET=true cargo fmt --check`, `cargo check -q`,
strict Clippy, `cargo check -q --target wasm32-unknown-unknown --lib`,
`cargo nextest run`, `mise run compare-peaks-drums`, `mise tasks validate`
and `mise run audit-upstream` (219 files, zero errors) pass.
