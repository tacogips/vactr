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
| PKS-002 | Bass/snare/hat/FM source comparison and missing drum controls | PKS-001 | FM source stages verified; numeric comparison, presets and other drum fidelity open |
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
