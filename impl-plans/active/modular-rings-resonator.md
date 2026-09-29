# Rings Resonator and String Instrument Coverage

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Split SYN-006 and FX-004. The pinned MIT Rings `dsp/part.h` declares six
`ResonatorModel` values, including three bonus variants, with up to four
voices, independent main/aux outputs and an external excitation input.
`string_synth_part.{h,cc}` is a separate string-synth path requiring its
own inventory. The four `Patch` controls and `PerformanceState` flags,
pitch/FM and chord choices must be mapped explicitly. No aggregate
resource table or audio sample should be imported before provenance audit.

## Modules

### `src/dsp/ported/rings.rs`

```rust
pub struct ResonatorSpec {
    pub model: u8,
    pub source_name: &'static str,
    pub voice_template: Option<&'static str>,
    pub effect_kind: Option<EffectKind>,
    pub status: Fidelity,
    pub resources: ResourceState,
}
pub fn resonator_models() -> &'static [ResonatorSpec; 6];
```

Voice and effect paths may share a bounded resonator kernel, but external
excitation cannot be claimed until an input is routed through the graph.
Main and aux must remain separately addressable. Any finite-memory
adaptation must distinguish its analytic strings/reverb from source delay,
filter, diffuser and generated-resource behavior.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| RNG-001 | Six-model, string-synth and resource/control inventory | SYN-006 | Six model enum values, patch/performance roles and initial transitive resource calls enumerated; per-table provenance pending |
| RNG-002 | Modal and string resonator modes with main/aux | RNG-001 | Six event-local analytic Part model roles runnable; source parity pending |
| RNG-003 | Sympathetic, quantized and FM bonus modes | RNG-001 | Original bounded adaptations runnable, full source stage comparison pending |
| RNG-004 | String-plus-reverb and separate string-synth path | RNG-001 | Short-diffuser Part and separate four-voice StringSynthPart adaptations runnable; source parity pending |
| RNG-005 | External-excitation effect routing, strum, polyphony, note/FM/chord | RNG-002..004 | Stereo bus/master `resonant-bank` and opt-in instrument graph excitation runnable; source parity pending |
| RNG-006 | Source comparison, notices, all-control and native/browser verification | RNG-002..005 | Adaptation tests/notices present; opt-in `compare-rings-part` source output comparison landed and run against the pinned checkout (see progress log); resource audit still pending |

## Completion Criteria

- [x] Six `Part` models have truthful coverage rows.
- [x] Separate string-synth Part has a truthful adaptation coverage row and implementation.
- [x] Internal Part structure, brightness, damping, position, strum, note/tonic/FM, chord, polyphony and model are codeable and shown in editor metadata.
- [x] Internal/external excitation and main/aux channels are tested independently for the bus adaptation.
- [ ] Memory, sample-rate, reset and callback-allocation limits hold natively and in browser.
- [ ] Every resource is cleared, originally replaced, or diagnosed as unavailable.
- [ ] Quiet Cargo check, strict Clippy, tests, rustfmt and diff checks pass.

## Progress Log

### Session: 2026-09-29 — RNG-006 source output comparison

An opt-in `mise run compare-rings-part` task compiles the pinned upstream
`rings/dsp/part.{h,cc}` (six `ResonatorModel` values, polyphony fixed to 1)
and `rings/dsp/string_synth_part.{h,cc}` from a separate local checkout —
`verification/rings_part_reference.cc` also links `resonator.cc`, `string.cc`,
`fm_voice.cc`, `rings/resources.cc`, and `stmlib`'s `units.cc`/`random.cc` —
and compares them against Vactr's `resonator-part-core`/`string-choir-core`
kernels via `examples/rings_part_reference.rs` and
`verification/compare_rings_part.py`. Both sides render at the confirmed
source rate/block (`rings/dsp/dsp.h`: `kSampleRate = 48000`,
`kMaxBlockSize = 24`) for 2.0 s, with an internal strum/exciter trigger
firing once at the start and a fixed note (69, i.e. A4/440 Hz with
tonic/fm at zero, no external excitation). Each of the six models and the
string-synth path is driven at three structure/brightness/damping/position
settings (0.15/0.25/0.20/0.30, 0.50/0.50/0.50/0.50, 0.85/0.75/0.80/0.70).
The script verifies Eurorack revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`
and `stmlib` revision `e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`, requires a
clean tracked tree, aligns each channel pair on its onset (first sample at
5% of that channel's peak) before scoring, and reports per-channel reference/
Vactr RMS, correlation, normalized RMS error, spectral centroid, -20/-40 dB
decay time and a time-domain-autocorrelation fundamental estimate. Run it as
`VACTR_MI_REFERENCE=/path/to/eurorack mise run compare-rings-part`.

Verified against a real pinned checkout (cloned to the exact two revisions
above; `git status` clean) rather than left unexercised: 21 model/scenario
comparisons (6 models plus string-synth, 3 settings, main and aux) all ran
and produced finite, non-degenerate audio on both sides. Correlation is
near zero for every model/scenario (average magnitude well under 0.1,
several negative), confirming this is not a source-equivalent port at the
sample level, consistent with `THIRD_PARTY_NOTICES.md`'s existing
"architectural reference" wording. Per-model classification (averaged over
the three settings, main and aux):

| Model | Classification | Notes |
|---|---|---|
| `MODAL_RESONATOR` | not comparable | Vactr's internal exciter keeps adding a small continuous sine term scaled by a never-zero decaying envelope (`src/dsp/ugen/rings_part.rs`'s `internal` excitation term), so the adaptation sustains rather than freely decaying; source RMS varies over two decades across settings while Vactr's stays within a narrow band. Average normalized RMS error is roughly 37 (source `Resonator` is materially louder and freely decaying; Vactr's four-mode filter bank sustains). |
| `SYMPATHETIC_STRING` | measured gap | Normalized RMS error is close to 1 (near-total mismatch) with near-zero correlation; the mid-setting fundamental estimate agrees exactly (440.4 Hz both sides) even though the waveform does not. |
| `STRING` | measured gap | Similar to above; mid-setting fundamentals are close (440.4 Hz source vs 466.0 Hz Vactr, about 6% apart). |
| `FM_VOICE` | measured gap | Highest average correlation of the six models (~0.07) but still far from source-equivalent; error grows at low structure/brightness/damping/position. |
| `SYMPATHETIC_STRING_QUANTIZED` | measured gap | Source's quantized-chord fundamental (220.2 Hz at the mid setting, an octave-below reading from the `BRYAN_CHORDS` table) diverges sharply from Vactr's unquantized 440.4 Hz; Vactr's `chord_ratio` is an authored interval map, not the source `chords[]` table, as already documented. |
| `STRING_AND_REVERB` | measured gap | Source's short diffuser network and `position`-weighted L/R mix give a materially different spectral balance (mid-setting fundamental estimate 888.9 Hz vs Vactr's 466.0 Hz, likely a harmonic pick on the source side given its added reverb tail) from Vactr's four-line network plus 512-sample feedback echo. |
| `STRING_SYNTH_PART` (string-choir path) | measured gap | Highest average correlation among all seven targets (~0.10) at low settings; mid-setting fundamentals agree exactly (440.4 Hz both sides). Vactr's four analytic chord voices and six authored FX formulas replace the source's twelve-oscillator rotation and `chorus`/`ensemble`/`reverb` FX headers, as already documented. |

No kernel translation error was found while building this comparison:
`src/dsp/ugen/rings_part.rs` and `src/dsp/ugen/string_choir.rs` are
unchanged. The `MODAL_RESONATOR` sustain behavior above is a deliberate
event-local adaptation choice (an always-on, decaying internal exciter
rather than a one-shot pluck), not an accidental mistranslation, so it was
left as is and reported as a gap rather than "fixed." This is a raw-kernel
comparison only: no `.vact` event, host callback, LPG, voice-stealing,
external-excitation routing, four-voice polyphony, or browser pathway is
exercised, and no upstream object, table, preset, or audio sample is
stored in Vactr — `rings/resources.cc` and the other linked upstream `.cc`
files are compiled only inside the task's temporary build directory.
`verification/upstream_inventory.toml` gained six sorted entries for the
upstream files this session newly names (`rings/dsp/dsp.h` as `source`,
confirmed for its rate/block constants; `rings/dsp/resonator.cc`,
`rings/dsp/string.cc`, `rings/dsp/fm_voice.cc`, `rings/resources.cc`
(`aggregate-resource`), and `stmlib/utils/random.cc` as `excluded`,
probe-only), and `THIRD_PARTY_NOTICES.md`'s "Rings Part architectural
reference" section gained a paragraph describing the probe. `python3
verification/audit_upstream.py --source <checkout>` passes with 0 errors
against the pinned checkout. Independent review of this session's diff,
including the `mise run compare-rings-part` output above, is recommended
before closing RNG-006.

### Session: 2026-09-28 — opt-in instrument graph excitation

`resonator-external-voice` routes the validated host L/R average through
`host-in-l/r` source nodes to appended Part port 16. Existing control indices
and the default internal template remain unchanged. Native/browser codec and
rate/block tests cover both input lanes, distinct main/aux and event start
offsets. Event-local state and authored modal/string/FM numerics remain
adaptations rather than source-equivalent shared Part behavior.

### Session: 2026-09-28 — external audio resonator bus adaptation

Added `resonant-bank` as a stereo bus/master effect. Source `Part::Process`
accepts mono excitation; Vactr sums L/R input, then returns resonator main
and auxiliary channels on L/R. Six model roles, structure, brightness,
damping, position, note/tonic/FM, chord, polyphony, strum, the three internal
performance flags, external mix, gate and dry/wet mix are codeable. The 17th
effect parameter and 18th voice-effect input port required bounded capacity
increases; existing effect kind ordinals remain stable by appending the new
kind. Four rate-sized comb lines and a short feedback tail need 9,340,
10,120 and 19,720 floats at 44.1/48/96 kHz respectively, checked before
replacing a live bus. Native/browser rendering, independent outputs, source
sensitivity, control/codec response and callback allocation are tested.
Independent review caught an install preflight error: the resonator was
incorrectly tied to Clouds' capture-duration allowance. It now checks only
its own fixed audio-memory requirement; a zero-capture host installs and
renders it, while insufficient memory still rejects without retiring a live
bus. Clouds capture effects retain their duration gate.

This is an original adaptation, not Rings `Part` parity: source has a mono
external input, distinct voice sharing and resonator/string/filter numerics.
The remaining source-equivalent shared voice behavior is separate from the working
bus path. No source tables or binary assets are imported.

### Session: 2026-09-28 — separate StringSynthPart adaptation

Added `string-choir-voice`/`string-choir-core` and a separate
`StringSynthSpec` row. The source has twelve oscillators, up to four
persistent rotating groups, original registration/chord arrays, and six FX
selections. Vactr independently authors four analytic chord/comb voices,
original registration/interval arithmetic and six distinct bounded FX
formulas: two formant colors, chorus, ensemble, and two short feedback
reverbs. All four Patch values, note/tonic/FM, chord, polyphony, strum,
internal performance flags and FX selection fit the 16-port interface.
The source `internal_*` flags are not consumed directly in
`StringSynthPart::Process`; Vactr gives them documented event-local roles.

Memory is allocated before the callback: 20,402/22,196/44,276 floats per
two-output voice at 44.1/48/96 kHz. A seeded rendered root-comb recurrence
test covers 20–100 Hz at all three rates. Native/browser codec, independent
outputs, all control and six-FX response, capacity rejection, and callback
allocation tests pass. Source twelve-voice rotation, exact registration,
polyBLEP, full FX delay/cross-channel topology, and external audio input
remain open; the manifest says `Adaptation`, not `SourcePort`.

### Session: 2026-09-28 — rate-sized low-note correction

Independent review found that the initial 1,024-sample comb line raised the
96 kHz string pitch floor to about 94 Hz. Replaced it with four per-output
lines sized at installation for a 20 Hz period plus two guard samples.
The two-output memory budget is 18,776/20,336/39,536 floats at
44.1/48/96 kHz; budget rejection remains explicit before rendering.
A seeded-impulse test verifies the rendered root-line recurrence period
for 20, 35, 50 and 100 Hz across all three rates and all four string-based
models. Four-line sympathetic polyphony is an intentional adaptation of the
source's eight-string network; below 20 Hz still clamps.

### Session: 2026-09-28 — first six-model internal instrument slice

Added `resonator-voice`, `resonator-part-core`, and `src/dsp/ported/rings.rs` with
six ordered `Adaptation` rows and independent main/aux output. The graph's
fixed port array grew from 12 to 16 to carry all Part `Patch` values, four
`PerformanceState` flags, pitch/tonic/FM, chord, polyphony and model without
dropping any control. The wire still carries `u8` port IDs and typed control
IDs; the ordinal 74 is appended. Same-build native/browser codec tests cover
port 15. The rate-sized four-line network allocates 9,388/10,168/19,768
floats per output at 44.1/48/96 kHz respectively; insufficient memory
fails installation.

This is not a source port: source resonator and string filters, quantizer,
FM sine tables, diffuser/reverb and shared Part voice stealing differ. Four
rate-sized delay lines track string fundamentals to 20 Hz at 44.1/48/96 kHz;
notes below 20 Hz clamp. Eight-string sympathetic topology is approximated
with four lines.
External audio input and the separate string-synth Part remain open.



### Session: 2026-09-28 — source inventory

At revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`, `part.h`
enumerates modal, sympathetic-string, string, FM voice, quantized
sympathetic-string, and string-with-reverb models. `Patch` carries four
continuous values; `PerformanceState` carries strum, internal input/note
selection, tonic/note/FM and chord. The source headers carry Emilie
Gillet's MIT terms. This is a source map only: no Rings kernel, routing,
data or source-parity port has yet landed in Vactr.

The source dependency pass finds generated lookup calls in
`resonator.cc` (`lut_stiffness`, `lut_4_decades`), `string.cc`
(`lut_svf_shift`) and `fm_voice.{h,cc}` (`lut_sine`,
`lut_fm_frequency_quantizer`). `string_synth_part.h` also depends on
chorus, ensemble, reverb, limiter, note filter, envelope and voice
components. These calls expand the audit boundary beyond `part.cc`;
no aggregate `rings/resources` table should be copied while its
individual generator inputs and licenses remain unexamined. Analytic
curves and oscillators may cover the same controls as disclosed
adaptations, but cannot be counted as numerically equivalent ports.
