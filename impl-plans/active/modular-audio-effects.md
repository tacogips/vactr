# Modular Audio Effect Ports Implementation Plan

**Status**: In Progress
**Design Reference**: `design-docs/specs/design-mutable-audio.md#coverage-inventory`
**Created**: 2026-09-27
**Last Updated**: 2026-09-27

## Design Document Reference

Port every eligible published audio effect and audio-processing mode from
Clouds, Warps, Streams, Rings, Elements and the audio-rate utility modules.
Stereo and sidechain behavior must survive Vactr's graph and bus paths.

## Modules

### `src/dsp/ported/effect.rs`

```rust
pub enum EffectFamily { Texture, Modulation, Dynamics, Resonator, Utility }
pub struct EffectSpecMeta {
    pub family: EffectFamily,
    pub mode: u16,
    pub input_count: u8,
    pub output_count: u8,
    pub parameter_count: u16,
}
pub fn effect_specs() -> &'static [EffectSpecMeta];
```

The concrete effect kernels may share the existing `FxUnit` lifecycle after
its capacity and port limits are checked. Install-time memory is bounded by
host capabilities; no audio callback allocates or fetches resources.

## Subtasks

| Task | Deliverable | Depends on | Status |
|---|---|---|---|
| FX-001 | Port Clouds granular, stretch, looping delay and spectral modes | MOD-004, MOD-005 | Four runnable adaptations; source parity review pending |
| FX-002 | Adapt seven Warps modulation modes, morph and internal carrier; audit spectral Easter egg | MOD-004 stereo bus boundary | All comparable XMOD positions now measure close after startup-ramp and aux-gain fixes; fold remains Adaptation; full parity pending |
| FX-002A | Translate cleared Warps cross-modulation equations and analytic mode crossfade | FX-002 | Completed; oversampling, fold/vocoder and full parity remain separate |
| FX-002B | Translate source algorithm-to-vocoder transition and XMOD parameter skew | FX-002A | Completed; vocoder kernel and carrier remain adaptations |
| FX-002C | Expose the remaining vocoder release/freeze algorithm range | FX-002B | Completed; vocoder filter-bank parity remains open |
| FX-002D | Translate Warps per-channel saturation amplifiers and expose both drives | FX-002C | Completed; input quantization and oversampling parity remain open |
| FX-002E | Translate source internal carrier shape pairs and table-free BLEP oscillator stages | FX-002D | Completed; sine table, noise SVF and SRC parity remain open |
| FX-002F | Port the MIT-noticed 6×/48-tap Warps XMOD sample-rate conversion path | FX-002E | Completed; vocoder filter bank and host-rate equivalence remain open |
| FX-003 | Translate Streams' six control functions and add clearly labeled digital audio adaptations | MOD-004 | Source boundary audited; see `modular-streams-controls.md` |
| FX-004 | Expose Rings resonator and Elements exciter/effect paths as processors | SYN-005, SYN-006 | `resonant-bank` and `exciter-bank` adaptations runnable; source parity pending |
| FX-005 | Translate Stages audio segments and Frames controls; implement a labeled digital mixer adaptation | SYN-008 | Source boundary audited; see `modular-segments-keyframes.md` |
| FX-006 | Validate bus/master/per-voice placement and native/browser parity | FX-001..FX-005 | Not started |

### FX-002F XMOD sample-rate conversion

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Warps
rate-conversion paragraph; pinned MIT `warps/dsp/sample_rate_converter.h`
and `warps/dsp/sample_rate_conversion_filters.h` (2015 Emilie Gillet).

**Deliverables**: Implement streaming six-times upsampling of external or
internal carrier and modulator, run cleared XMOD equations on those
subsamples, and downsample through the source 48-tap FIR. Use the
symmetry-reduced MIT coefficient half-kernel with notice, not an
oscillator wavetable. Store converter history in preallocated effect
memory, preserve continuity at arbitrary host block boundaries, and
keep vocoder and auxiliary routing as separately documented paths.
The source FIR's host-sample delay must be explicit; no callback
allocation or unbounded work.

**Completion Criteria**:
- [x] FIR/polyphase tests verify gain, symmetry, phase, impulse latency, continuous chunking and reduced aliasing against the pre-SRC XMOD path.
- [x] Seven modes, seven `.vact` controls and stereo main/aux remain functional at native/browser 44.1/48/96 kHz × 64/256 blocks, including late start and armed zero allocation.
- [x] Strict bus preflight rejects a one-float-short converter region before retiring a live bus, while exact-fit installation succeeds.
- [x] Notices identify the imported MIT filter coefficients exactly and keep oscillator wavetables/source vocoder/SVF/hardware parity separate.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### FX-002E internal carrier oscillator stages

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Warps
internal-carrier paragraph; pinned MIT `warps/dsp/oscillator.{h,cc}` and
`warps/dsp/modulator.cc` carrier selection and crossfade.

**Deliverables**: Preserve `carrier-wave` 0..6 and `carrier-frequency`
`.vact` controls. For waves 1..3, provide independent XMOD and vocoder
carrier states and the source roles sine/triangle/saw versus
saw/pulse/noise, with source transition balance and XMOD carrier gain.
Translate table-free `ThisBlepSample`/`NextBlepSample`, triangle
integration and saw/pulse filtering with fixed state. Use analytic sine
instead of the source lookup. Keep host-rate noise generation and filter
as a labeled adaptation, and preserve options 4..6 as authored
extensions. No generated tables, wave assets or callback allocations.

**Completion Criteria**:
- [x] Scalar/state tests cover wrap and half-cycle BLEP, shape mapping at cross-modulation, transition and vocoder positions, and frequency/phase-input response.
- [x] All existing seven algorithms and seven controls stay codeable in `.vact`; native/browser 44.1/48/96 kHz × 64/256 blocks, stereo main/aux and armed allocation tests pass.
- [x] Notices distinguish translated BLEP/shape roles from analytic sine, authored noise filter, extra shapes and unported SRC/hardware scaling.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### FX-002D per-channel input amplifier

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Warps
input-stage paragraph; pinned MIT `warps/dsp/modulator.h`
`SaturatingAmplifier` and `stmlib/dsp/dsp.h` `SoftClip`.

**Deliverables**: Translate the source's per-input energy gate, drive
pre-gain, post-gain normalization, soft clipping and XMOD/vocoder blend
with fixed per-effect state. Add `.vact`/editor `carrier-drive` and
`modulator-drive` controls, each 0..1 with unity default, applied after
the existing shared `drive` control so existing files remain valid. The
two source channel drives must be independently reachable with shared
`drive: 1`. External-carrier aux should sum gated raw inputs with drive
modulation; internal carrier aux remains its generated carrier. Do not
import resources or claim source ADC quantization/oversampling parity.

**Completion Criteria**:
- [x] Scalar tests pin gate, gain, clip and limit behavior, both independent drives, and external/internal auxiliary routing.
- [x] Existing `.vact` remains valid, two new controls are addressable, and all seven modes render finite native/browser stereo audio at 44.1/48/96 kHz and 64/256 blocks with zero callback allocation.
- [x] Notice distinguishes translated amplifier equations from host float input, authored oscillator/vocoder and unported oversampling.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### FX-002C vocoder release and freeze range

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Warps
vocoder-range paragraph; pinned MIT `warps/dsp/modulator.cc` release mapping
and `warps/dsp/vocoder.{h,cc}` follower freeze boundary.

**Deliverables**: Extend the existing `.vact`/editor `algorithm` control
from 0..6 to 0..8, preserving its 0..6 meanings. Positions 6..8 map
the source normalized 0.75..1.0 vocoder-only region into a bounded
release value `r = clamp(4 * (algorithm/8 - 0.75), 0, 1)` and its source
curve `r*(2-r)`. Apply that value to the authored twenty-band vocoder's
decay and freeze role, keeping `timbre` for formant shift. Do not claim
source follower/filter-bank parity or import generated resources.

**Completion Criteria**:
- [x] The algorithm 0..8 control survives `.vact`, editor, codec and host validation; old 0..6 values retain their modes.
- [x] Scalar and audio tests demonstrate release changes at 6/7/8 and frozen envelope behavior at the upper endpoint without non-finite output or callback allocations.
- [x] Native/browser rates, blocks and stereo outputs pass; provenance distinguishes source control mapping from authored follower/filter bank.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### FX-002B algorithm and vocoder transition

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Warps
transition paragraph; pinned MIT `warps/dsp/modulator.cc`,
`warps/dsp/parameters.h` and `warps/dsp/modulator.h`.

**Deliverables**: Preserve the five `.vact` controls, seven endpoint modes,
the original fold/vocoder kernels and dual bus outputs. Map the public
`algorithm` 0..6 scale to the source 0..0.75 range for six XMOD positions
and the vocoder endpoint. Apply the pinned timbre skew to XMOD scalar
equations. Near the vocoder end, blend comparator with the source NOP
(raw modulator), then use the source triangular dry-modulator bridge before
the existing vocoder output. Keep all state fixed-size and make source
versus adaptation boundaries explicit in tests and notice.

**Completion Criteria**:
- [x] Deterministic scalar tests cover XMOD skew, comparator-to-NOP balance and both sides of the 5.4..5.8 raw-modulator bridge.
- [x] Existing seven modes, five controls, stereo/aux and `.vact` routes remain functional across native/browser rates and block sizes without callback allocation.
- [x] Notice documents the transition as a translated stage, while vocoder/filter/carrier and full source parity stay open.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

### FX-002A cleared cross-modulation stages

**Design Reference**: `design-docs/specs/design-mutable-audio.md`, Warps
fidelity paragraph; pinned MIT `warps/dsp/modulator.cc`,
`warps/resources/lookup_tables.py` and `stmlib/dsp/dsp.h`.

**Deliverables**: Refine `src/dsp/effects/cross_mod.rs` with source-shaped
analytic crossfade, analog/digital ring, XOR and four-way comparator
equations, preserving Vactr's original fold/vocoder implementations,
five named controls, continuous algorithm position, preallocated state,
dual bus outputs and existing codec/editor schema. Do not import generated
fold, sine, crossfade, oscillator or other lookup arrays.

**Completion Criteria**:
- [x] Cleared algorithm equations and adjacent-mode transitions have focused deterministic tests, including endpoint and intermediate timbre behavior.
- [x] All seven modes and five `.vact` controls still change finite audio; native/browser 44.1/48/96 kHz × 64/256 blocks, channel routing and zero callback allocation pass.
- [x] Notices distinguish translated modes from original fold/vocoder and unported oversampling/amp/carrier stages without a full source-port claim.
- [x] Quiet Cargo, strict Clippy, rustfmt, wasm, full tests and independent Rust review pass.

## Module Status

| Module | File path | Status | Tests |
|---|---|---|---|
| Effect registry | `src/dsp/ported/effect.rs` | Not started | - |
| Effect kernels | `src/dsp/effects/cross_mod.rs`, `cross_mod_osc.rs`, `cross_mod_src.rs`, `cross_mod_vocoder*.rs`, `shift_pair.rs` | Seven modes, translated scalar/BLEP/XMOD FIR and 96-kHz 20-band vocoder stages with generated host-rate FIR boundary; separate quadrature shifter | Native/browser DSP tests |
| Stereo granular texture | `src/dsp/effects/texture.rs` | Eight-grain adaptation; source parity pending | Native/browser/e2e controls, memory and allocation tests |
| Stereo stretch texture | `src/dsp/effects/texture_stretch.rs` | Two-window alignment adaptation; source parity pending | Native/browser/e2e controls, memory and allocation tests |
| Stereo looping texture | `src/dsp/effects/texture_loop.rs` | Live/frozen-loop adaptation; source parity pending | Native/browser/e2e controls, memory and allocation tests |
| Stereo spectral texture | `src/dsp/effects/texture_spectral.rs` | Four-frame STFT adaptation; source parity pending | Native/browser/e2e controls, memory and allocation tests |
| Effect metadata | Existing effect catalog | `dual-mod` 7 controls; `shift-pair` 8 controls | Editor declaration test |
| End-to-end | `src/host/tests/e2e/buses.rs` | `.vact` bus install/routing | Algorithm response |

## Dependencies

| Feature | Depends on | Status |
|---|---|---|
| Stereo ports and install memory | `modular-audio-foundation.md` | Pending |
| Resonator as effect | `modular-synth-engines.md` | Pending |

## Completion Criteria

- [ ] Every eligible source effect/mode is mapped and renders real stereo audio.
- [ ] All mode controls, sidechains and secondary outputs are addressable.
- [ ] Resource-backed modes diagnose missing data and honor host limits.
- [ ] Provenance, notices and user-facing neutral names are complete.
- [ ] Deterministic, allocation, and browser/native parity checks pass.
- [ ] Quiet cargo check, clippy and tests pass.

## Progress Log

### Session: 2026-09-29, FX-002G Warps XMOD numeric comparison

Added `verification/warps_xmod_reference.cc`,
`verification/compare_warps_xmod.py`,
`examples/warps_xmod_reference.rs` and `mise run compare-warps-xmod`. The
reference compiles the pinned upstream `Modulator::Process` and its local
dependencies in a temporary directory; the Vactr probe calls
`cross_mod::process` directly. Both consume the same Python-generated 96 kHz
stereo float32 carrier sine (220 Hz) and noise-free modulator saw (137 Hz),
processed in 60-frame blocks for 12,000 frames. The source's normalized
algorithm value is the XMOD position divided by eight; Vactr's public
algorithm position is unchanged. The 264 scenarios cover positions 0..5 in
half-step increments, three timbres, two independent-drive pairs, and
external plus internal carrier shapes 1..3. The internal source note is 48.

Classification uses the mean main-output metrics over the 24 timbre/drive/
carrier scenarios at each position: close means NRMSE <= 0.10 and correlation
>= 0.95; measured gap means the translated stage exceeds either threshold.
Positions 0.5, 1.0 and 1.5 are not comparable because they include Vactr's
authored fold contribution. The source scales every aux output by 0.5 when
converted to signed 16-bit samples (`output->r = Clip16(aux * 16384)`). The
original aux gap was only for internal carriers: external-carrier Vactr aux
already averages its raw inputs, matching the source's summed aux followed by
half output gain.

| XMOD position | Classification | Main NRMSE | Main correlation | Aux NRMSE | Aux correlation | Estimated latency, main / aux (samples) |
|---:|---|---:|---:|---:|---:|---:|
| 0.0 | Close | 0.045 | 0.997 | 0.757 | 0.997 | -7..0 / -7..0 |
| 0.5 | Not comparable | 1.044 | 0.508 | 0.757 | 0.997 | -28..48 / -7..0 |
| 1.0 | Not comparable | 1.392 | 0.099 | 0.757 | 0.997 | -48..48 / -7..0 |
| 1.5 | Not comparable | 0.864 | 0.678 | 0.757 | 0.997 | 0..1 / -7..0 |
| 2.0 | Close | 0.074 | 0.995 | 0.757 | 0.997 | 0..0 / -7..0 |
| 2.5 | Measured gap | 0.106 | 0.991 | 0.757 | 0.997 | -5..0 / -7..0 |
| 3.0 | Close | 0.093 | 0.992 | 0.757 | 0.997 | -4..0 / -7..0 |
| 3.5 | Measured gap | 0.104 | 0.991 | 0.757 | 0.997 | -2..0 / -7..0 |
| 4.0 | Measured gap | 0.134 | 0.984 | 0.757 | 0.997 | -5..0 / -7..0 |
| 4.5 | Measured gap | 0.110 | 0.990 | 0.757 | 0.997 | -5..0 / -7..0 |
| 5.0 | Close | 0.097 | 0.991 | 0.757 | 0.997 | -4..0 / -7..0 |

RMS, correlation, latency, spectral centroid, harmonic-energy ratio and
intermodulation-energy ratio are emitted separately for main and aux in every
scenario. In this initial run, position 2.0 main RMS was 0.562/0.562
(source/Vactr), centroid was 422.2/421.6 Hz, harmonic ratio 0.494/0.494 and
intermodulation ratio 0.358/0.357. At position 0.0, main RMS was
0.326/0.326 and centroid was 350.9/350.7 Hz. The initial metrics identified
two translated-stage gaps documented and fixed in the next session entry;
fold-containing positions remain incomparable. These scenarios do not
demonstrate full firmware parity or arbitrary-input fidelity.

### Session: 2026-09-29 (start-up ramp and aux gain)

Translated the upstream oscillator's 100 Hz initial phase increment and
linear ramp to the target over 60/96,000 seconds. The elapsed duration stays
constant at 44.1, 48 and 96 kHz, with completion on the first available host
sample and fixed progress state preserved across callback partitions. The
internal-carrier aux now returns half the raw carrier, matching the source's
half-gain signed-16-bit output conversion; this is an intentional -6 dB
user-audible change. External-carrier aux was already at the correct level.
The reference `Modulator` now has static storage so its amplifier state is
zero-initialized before `Init`.

The table compares per-position means over the same 24 scenarios. Each cell
shows normalized RMS error / correlation. The initial main metrics exposed the
startup phase offset; the initial aux metrics exposed only the internal
carrier gain mismatch.

| XMOD position | Main before | Main after | Aux before | Aux after |
|---:|---:|---:|---:|---:|
| 0.0 | 0.045 / 0.997 | 0.003035 / 0.999995 | 0.757 / 0.997 | 0.000306 / 1.000000 |
| 2.0 | 0.074 / 0.995 | 0.000282 / 1.000000 | 0.757 / 0.997 | 0.000306 / 1.000000 |
| 2.5 | 0.106 / 0.991 | 0.000340 / 1.000000 | 0.757 / 0.997 | 0.000306 / 1.000000 |
| 3.0 | 0.093 / 0.992 | 0.000265 / 1.000000 | 0.757 / 0.997 | 0.000306 / 1.000000 |
| 3.5 | 0.104 / 0.991 | 0.000818 / 0.999999 | 0.757 / 0.997 | 0.000306 / 1.000000 |
| 4.0 | 0.134 / 0.984 | 0.001789 / 0.999997 | 0.757 / 0.997 | 0.000306 / 1.000000 |
| 4.5 | 0.110 / 0.990 | 0.001230 / 0.999999 | 0.757 / 0.997 | 0.000306 / 1.000000 |
| 5.0 | 0.097 / 0.991 | 0.000545 / 1.000000 | 0.757 / 0.997 | 0.000306 / 1.000000 |

All comparable positions now meet the existing main-channel close rule; aux
is close at all eight positions as well. Fold positions 0.5, 1.0 and 1.5
remain not comparable because Vactr's fold is an adaptation. Keep positions
0, 2, 3, 4 and 5 at `SourceStage`, never `SourcePort`; the fold remains
`Adaptation`. The comparison validates these measured signals only, not full
firmware parity, arbitrary-input behavior or every host-rate operating mode.
The startup ramp and aux gain tests pass at 44.1/48/96 kHz and across saved
state partitions. `cargo fmt --check`, `cargo check -q`, strict Clippy and the
wasm library check pass. Nextest ran 1,008 tests: 1,006 passed, one was
skipped, and two HTTP fixtures could not bind loopback under the sandbox.

### Session: 2026-09-28, FX-002F XMOD sample-rate conversion

Translated the pinned MIT six-times/48-tap XMOD converter using its
24-coefficient up and down FIR half-kernels, with the full source notice
in `cross_mod_src.rs`. Fixed per-effect history keeps processing continuous
across host blocks without callback allocation. The vocoder remains on its
separate authored path. A review found that bus installation could accept
less than the new 164-float memory requirement; strict preflight now rejects
one-float-short capacity before retiring a live bus, while exact-fit
installation renders audio. Source-coefficient, FIR, phase, latency,
chunking, alias, seven-mode/control, native/browser matrix, browser-wire,
stereo and allocation checks passed. Independent review found no remaining
blocker; quiet native/no-default/wasm checks, strict Clippy, rustfmt and
full tests passed: 1,468 passed, one ignored. The source's fixed 96 kHz
operating point, vocoder filter bank and hardware scaling remain open.

### Session: 2026-09-28, FX-002E internal carrier shape pairs

Source carrier settings 1..3 now pair XMOD sine/triangle/saw with vocoder
saw/pulse/noise. Independent fixed oscillator state carries phase, pending
two-sample BLEP correction, filters and noise-ducking energy. The pinned
table-free BLEP, triangle integrator, saw/pulse filtering and carrier
transition gains are translated. Analytic sine replaces the source table;
noise uses Vactr's RNG and one-pole filter; settings 4..6 remain
authored extensions. The previous amplifier-gate time scaling was also
corrected from 32 kHz to the pinned application rate of 96 kHz, and the
triangle integrator avoids double host-rate scaling. Tests cover BLEP
events, role changes, `.vact` and native/browser 44.1/48/96 kHz by
64/256-frame stereo routing and armed allocation. Independent review
confirmed source-rate correction and provenance; quiet native/no-default/
wasm checks, strict Clippy, rustfmt and full tests passed: 1,463 passed,
one ignored. Exact sine table, noise SVF, SRC/oversampling and hardware
auxiliary scaling remain open.

### Session: 2026-09-28, FX-002D per-input amplifier

Translated the pinned per-channel noise gate, pre/post gain curve,
`stmlib` soft clip and XMOD/vocoder limit equation into fixed Rust state.
`carrier-drive` and `modulator-drive` are independently addressable
through `.vact` and editor metadata; the existing shared `drive` and old
files still work with unity defaults. External carrier mode sums gated
raw input contributions in aux, while an internal carrier keeps its own
aux output. Host float bounding and host-rate smoothing differ from the
source signed-sample and block interpolation path. Scalar, `.vact`,
native/browser 44.1/48/96 kHz by 64/256-frame stereo and armed allocation
tests passed. Independent review confirmed equations and provenance;
quiet native/no-default/wasm checks, strict Clippy, rustfmt and full tests
passed: 1,457 passed, one ignored. Source carrier, oversampling, exact
filter bank and hardware output scaling remain open.

### Session: 2026-09-28, FX-002C vocoder release and freeze range

Extended the `.vact` and editor `dual-mod algorithm` control from 0..6 to
0..8 without changing positions 0..6. The source's normalized 0.75..1.0
algorithm range now maps positions 6..8 to release curve
`r*(2-r)`, with strict `>0.995` freeze. The authored twenty-band follower
uses the release role independently of the timbre/formant control; it is
not the source decimated filter bank. Tests prime the bands, remove the
modulator and compare fast, slow and frozen envelope/audio tails, and
verify `.vact` algorithm 8, editor/codec, native/browser 44.1/48/96 kHz
by 64/256-frame late-start stereo routing and armed callback allocation.
Independent review confirmed the mapping, adapted follower boundary and
notice. Quiet native/no-default/wasm checks, strict Clippy, rustfmt and
full tests passed: 1,453 passed, one ignored. Source filter-bank,
oscillator, amplifier and oversampling parity remain open.

### Session: 2026-09-28, FX-002B algorithm and vocoder transition

The public algorithm position 0..6 now maps to the pinned source's 0..0.75
XMOD/vocoder range. XMOD timbre follows the source skew and adjacent mode
interpolation; its last slot balances comparator with NOP/raw modulator.
The source's triangular raw-modulator bridge is continuous through the
5.4, 5.6 and 5.8 transition points into Vactr's independent vocoder.
Five controls, seven endpoint modes, stereo main/aux, `.vact` and browser
routing remain available. Tests cover scalar boundaries, audible transition,
native/browser rates and blocks, late starts and armed zero callback
allocations. Independent review confirmed the mapping and provenance;
quiet native/no-default/wasm checks, strict Clippy, rustfmt and full tests
passed: 1,451 passed, one ignored. Source input amplifiers, oversampling,
carrier and vocoder/filter-bank response remain open.

### Session: 2026-09-28, FX-002A analytic Warps modulation stages

Translated the MIT-cleared crossfade, analog and digital ring, XOR and
comparator scalar equations with the pinned modulator/carrier order. The
crossfade uses the source generator's analytic angle and gains without
importing its table. The authored fold, twenty-band vocoder, carrier and
oversampling path remain adaptations; no full source parity is claimed.
Tests cover equation endpoints and intermediate timbres, the asymmetric
ring/comparator choices, all seven modes and five controls, isolated
carrier-frequency response, stereo bus main/aux routing, native/browser
44.1/48/96 kHz with 64/256-frame blocks and late event starts, editor
metadata, codec and armed callback allocation. The independent reviewer
found and confirmed fixes for a reversed crossfade endpoint and a
confounded carrier-frequency test. Final quiet native/no-default/wasm
checks, strict Clippy, rustfmt and full tests passed: 1,448 passed, one
ignored. `THIRD_PARTY_NOTICES.md` distinguishes the five translated
equations from independent fold/vocoder work.

### Session: 2026-09-28, FX-002 Easter-egg frequency-shifter adaptation

`shift-pair` is a separate stereo bus/master effect with external or authored
analytic internal carrier, signed shift pot/CV, external-carrier phase
rotation, complementary up/down sidebands, timbre crossfade, bounded
feedback, source-role dry/wet and host-effect mix. A 127-tap original
windowed Hilbert FIR is generated at installation into 381 preallocated
floats; it adds 63 samples of latency. This replaces the source generated
quadrature pole/sine/crossfade resources and oscillator, and changes low-note
rejection, feedback, shift scaling and numerical response. External carrier
mode uses phase-shift; shift pot/CV affect only internal carrier, matching
the source role. L/R bus outputs carry main/aux. Tests cover shift direction,
near-zero shift, external phase, distinct outputs, control response,
feedback bounds, `.vact`/editor, native/browser rates and blocks, codec,
capacity rejection without retiring a live bus, and callback allocations.
FX-002 remains partial because all Warps algorithms and carrier/filter
numerics, oversampling, and voice-local two-output placement lack source
parity; this is an Adaptation, not a source port.

### Session: 2026-09-27

**Tasks Completed**: Scope and source inventory documented.
**Tasks In Progress**: None.
**Blockers**: Foundation interfaces and resource provenance.

### Session: 2026-09-27, FX-002 modulation bus slice

**Source audit**: The pinned MIT Warps source has six cross-modulation algorithms and a 20-band vocoder path. `modulator.cc` also depends on oscillator, quadrature transform, sample-rate conversion, limiter, filter-bank, `stmlib` and generated resources; none is copied. A `dual-mod` bus effect maps stereo L/R input to carrier/modulator and writes main/aux back to L/R. External and internal carrier modes include analytic sine, triangle, saw, pulse, filtered noise and carrier-input phase modulation; input drive, timbre, carrier frequency/wave, and continuous algorithm position are exposed.
**Implementation**: A fixed 100-float per-bus memory region holds 20 independent analytic bands, with coefficients computed for the host sample rate. Native/browser codec tests and a `.vact` bus test cover the interface and distinct main/aux outputs without callback allocations.
**Remaining at this session**: The filter-bank, diode-like nonlinearity, transitions, and carrier shaping are original approximations rather than parameter- or sample-exact Warps algorithms. The spectral Easter egg and the published oversampled/quadrature details were not yet implemented at this point; the 2026-09-28 `shift-pair` adaptation above adds a separate runnable Easter-egg role. Bus/master placement preserves main/aux as L/R, but a voice-local effect node still downmixes its two outputs in `voice.rs`; therefore `dual-mod` is a two-output bus effect only. FX-002 remains partial until semantic comparison and full stereo voice-graph routing are resolved; no full Warps port is claimed.
**Verification**: Native/browser mode and main/aux routing tests, editor metadata, and `.vact` bus evaluation pass. `CARGO_TERM_QUIET=true cargo check -q`, strict Clippy, and full `cargo test -q` (1007 library tests and integration suites) pass. All touched Rust sources are under 1000 lines; rustfmt and diff checks pass.
