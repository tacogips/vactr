# Published Modular Audio DSP in Vactr

This specification covers ports of the publicly released Mutable Instruments
Eurorack audio DSP into Vactr. It is linked from `design-music.md` section
4.2. The source inventory is the official `pichenettes/eurorack` tree at
commit `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`; any later source
revision requires a new audit.

## Scope and license boundary

Port every published Eurorack audio-generating or audio-processing algorithm
with MIT-compatible source. Include discontinued modules. Exclude hardware,
panel/UI firmware, MIDI routing, sequencer-only firmware, and control-only
modules that have no audio DSP. Keep upstream names in attribution and
engineering references only; user-facing instruments and effects have
Vactr names. The official repository says STM32 code is MIT, AVR code is
GPL-3.0, and the hardware has a separate license. Every ported file and
resource needs an individual provenance check, including `stmlib` and any
generated data. Preserve copyright and MIT notices for ported material.
The `stmlib` license lists STMicroelectronics and GPL-covered serial-programming
code as exceptions; neither belongs in the audio port.

The earlier request to avoid reusing LXR wave tables still applies. No LXR
source or data is in scope. For Mutable Instruments, inspect resources one by
one: code-level MIT statements do not prove that external sample recordings,
DX7 preset banks, or speech ROM extracts are cleared for distribution. A
resource-dependent engine may accept user-provided content or original
replacement data until provenance is established. Do not present a partially
populated mode as a complete port.

## Coverage inventory

### Live external excitation boundary

The engine now accepts opt-in stereo audio at the master bus before its
effect chain: interleaved L/R for native and planar L/R from the AudioWorklet.
Native mono input is duplicated to L/R; a disconnected worklet input is
silent. Short, malformed or nonfinite buffers are silenced with an
`InputBuffer` host-transport fault rather than read out of bounds. The
worklet copies at most its fixed quantum into preallocated wasm memory and
zeros missing channels every quantum. Existing output-only calls continue
to render with no external input; quad channels 3/4 remain direct stems.

This bus boundary lets `.vact` master effects process live excitation.
An arbitrary multi-input instrument graph still requires further work. Native
capture is opt-in through `--audio-in`; callers can also supply audio through
`AudioSide::render_with_input`, and Web Audio sources can connect to the
worklet node's stereo input.

Native device capture opens explicitly. It selects an f32
mono or stereo input configuration at the output stream's sample rate; if no
compatible device/configuration exists, report a host diagnostic. The input
callback writes bounded stereo frames to a preallocated single-producer,
single-consumer queue, duplicating mono and selecting the first two channels
of a wider device. The output callback drains exactly its render frame count,
substitutes zero for underruns, trims stale backlog, and counts overrun,
underrun, nonfinite and incomplete input. `NativeAudioHost::capture_stats`
exposes those counters; `take_diagnostics` reports increments. Independent
device clocks can drift, so this queue is a bounded timing bridge rather
than a promise of sample-exact synchronization. No resampling or adaptive
clock correction is provided; the output-only default remains unchanged.

`host-in-l 0` and `host-in-r 0` are read-only graph source UGens with stable
wire tags 90/91. The inert `0` is required by the current generic UGen call
syntax; it is not an audio input. They read the validated block without
per-voice copying, use the voice event's start offset, and emit zero when
disconnected. `examples/live-external-voices.vact` opts in to Rings mono
excitation by averaging L/R and Elements' separate left blow/right strike.
Its templates become editor-discoverable after loading the file; the default
internal templates remain available. The default preallocated pool has 72
slots for the 63 core definitions and opt-in instruments; an Elements pattern
may override its authored `el-gate 1` and input-level defaults. This is an authored event-local
adaptation and does not expose arbitrary bus taps or source-equivalent DSP.

| Source family | Vactr target | Coverage requirement |
|---|---|---|
| `plaits/dsp` | Multi-engine instrument | All 24 registered engines, both output channels, trigger/LPG behavior, and all engine-specific parameters; audit FM patches, speech words, wave resources separately |
| `braids` | Multi-model oscillator | Every accessible macro-oscillator shape, both timbre parameters, trigger/sync behavior and model resources |
| `elements/dsp` | Exciter and modal instrument | Exciter, resonator/string voices, stereo and reverb, with every patch control; audit bundled recordings |
| `rings/dsp` | Exciter/resonator instrument and effect | String and modal modes, strum behavior, sympathetic voices, stereo effects, and all patch controls |
| `clouds/dsp` | Stereo texture effect | Granular, stretch, looping-delay, spectral modes, freeze, quality, feedback, reverb and all mode controls |
| `warps/dsp` | Dual-input modulation effect | All seven published algorithms (crossfade, fold, analog/digital ring modulation, XOR, comparator, vocoder), continuous transitions, internal carrier and auxiliary output controls; audit the spectral Easter egg separately |
| `tides`, `tides2` | Audio-rate function instruments | Tides1 AD/loop/AR × three ranges and Tides2 24 ramp/output/range roles runnable as original adaptations; each has two default selected outputs or four opt-in outputs, with Tides1 EOA level and host-rate-scaled EOR hold adaptations |
| `peaks/drums` and audio-rate modulation | Percussion instruments and modulation units | Bass, snare, FM drum, hi-hat and audio-rate control generators with all controls |
| `streams` | Control algorithms plus digital audio adaptations | Envelope, vactr, follower, compressor, filter controller and chaos controls; distinguish firmware CV from Vactr's digital gain/filter path |
| `stages`, `frames` | Audio-rate segments and control-to-audio adaptations | Stages' segment oscillator paths and Frames' keyframe/poly-LFO CV roles; distinguish Frames' analog VCA mix path from firmware DSP |

The runnable `dual-mod` effect and separate `shift-pair` Easter-egg effect
are stereo bus/master adaptations. `shift-pair` exposes internal/external
carrier, signed shift pot/CV, external phase rotation, complementary
sidebands, timbre, feedback and wet/dry. Its authored 127-tap quadrature
filter adds 63 samples of latency and has different low-frequency rejection
from the pinned Warps pole bank; no source lookup or wave asset is imported.
Both outputs are addressable as bus L/R, while voice-local two-output
placement and source numerical parity remain open.
The current `dual-mod` fidelity pass translates the pinned MIT crossfade,
analog/digital ring, XOR and comparator signal equations and continuous
mode transitions using analytic functions. The crossfade shape may be
derived from its individually MIT-noticed generator; do not import its
lookup array. Keep the independent fold and the translated 20-band
source-rate vocoder, Vactr's seven `.vact` controls, stereo bus main/aux contract
and fixed callback state. The sixfold XMOD FIR conversion is translated
separately below. Exact amplifier smoothing, carrier oscillator numerics,
host-rate equivalence and generated
fold/vocoder resources remain explicit parity gaps rather than a complete
Warps port.

Vactr's `algorithm` 0..8 control maps onto the source's 0..1 modulation
range. Adjacent XMOD modes occupy positions 0..5; the comparator-to-vocoder
end passes through the source's raw-modulator transition around 5.4..5.8.
The source's modulation-parameter skew is applied before the cleared scalar
XMOD equations. Positions 6..8 drive the source-shaped release and freeze
control role in the translated 96-kHz decimated follower at every host
rate. Carrier choices remain adaptations; this
mapping alone does not imply numerical parity with the source firmware.

The FX-002D input-stage pass translates the pinned `SaturatingAmplifier`
scalar gate, drive curve and soft clip from `warps/dsp/modulator.h`, with
separate carrier and modulator drive controls. The existing `drive` remains
a shared multiplier for `.vact` compatibility; the new per-input controls
default to unity and make both source drive values independently reachable
when shared `drive` is one. Preserve fixed callback state, external-input
auxiliary routing and internal-carrier output. The host float input,
carrier lookup/filter numerics and vocoder conversion remain explicit
source differences.

The internal-carrier pass maps `carrier-wave` 1..3 to the pinned source
shape pairs: sine/triangle/saw in XMOD, saw/pulse/noise in vocoder, with
the source balance between carrier roles near the transition. It
translates the oscillator's table-free two-sample BLEP corrections for
triangle, saw and pulse, while generating sine analytically and retaining
Vactr's fixed-state noise filter. Carrier options 4..6 remain authored
extensions. All carrier frequency and shape controls stay codeable in
`.vact`; exact sine lookup, source SVF, block interpolation, host-rate FIR parity and
hardware output scaling remain open fidelity gaps.

The XMOD rate-conversion pass uses the pinned six-times up/down,
48-tap FIR structure from `warps/dsp/sample_rate_converter.h` and the
individually MIT-noticed coefficients in
`warps/dsp/sample_rate_conversion_filters.h`. These coefficients are
anti-aliasing filter data, not oscillator wavetables. Keep the symmetric
half-kernel with its copyright notice, streaming filter state in the
effect's preallocated memory, and no callback allocation. The resulting
XMOD latency and source-like alias rejection must be measured at
44.1/48/96 kHz and block boundaries. Vocoder filter-bank response and
the host's non-96-kHz timing remain separate fidelity gaps.
Bus installation must reject a converter region that is even one float
short before retiring a live chain; the XMOD processor requires the full
history to emit audio.

The vocoder fidelity pass keeps the 20-band filter bank separate from the
output limiter. The pinned MIT `warps/dsp/limiter.h` stage applies
1.4 pre-gain, a peak follower with fast attack and slow release, gain
reduction above unity, and `stmlib` soft limiting after 0.8 output gain.
It is translated as persistent, allocation-free state on vocoder audio
only, before the existing XMOD/vocoder bridge. Tests exercise onsets,
decay across host block boundaries, sustained overload, finite output,
and unaffected XMOD and auxiliary channels. The vocoder runs at an internal
96 kHz at every supported host rate; the limiter also runs at that rate.

The source vocoder bank has 13 bands at 96 kHz / 12, six at 96 kHz / 3
and one at 96 kHz; two CrossoverSVF passes per band feed envelope
followers and a delay-compensated synthesis bank. The pinned 60-frame
hardware block is divisible by both decimation factors. Preserve that
cadence across arbitrary host callbacks when translating the bank, and
count its buffering in reported latency and effect memory. Source-rate
96-kHz parity is checked separately from 44.1/48-kHz behavior because
the latter crosses a generated host-rate FIR boundary. Coefficient tables in the individually
MIT-noticed `warps/resources/filter_bank.py`/`resources.cc` are filter
parameters; oscillator waves and other aggregate resources remain excluded.
At 96 kHz, both carrier and modulator traverse source 3× and 4× FIR
decimators before the 13 low-rate bands, while six mid bands read the
3× path and the top band reads full rate. Each band has two cascaded
`CrossoverSvf` passes: low-pass for the first band, normalized band-pass
for the interior and high-pass for the last. Source post-gains and
band-specific delay compensation precede 4× then 3× synthesis. The
follower observes absolute band audio scaled by `sqrt(20)`, chooses
attack or decay per sample, and smooths each 60-frame peak; freeze
suppresses both rates. Formant position interpolates those previous
peaks across bands while carrier/direct-vocoder gains ramp through the
next block. Preserve this ordering when comparing with the pinned
source; an analytic pitch-ratio calculation or host buffering may leave
small, separately measured differences.

The active 60-frame configuration needs 245 sample and 456 delay floats
per bank, versus source arrays reserved for a 96-frame maximum. With
converter histories, scratch and SVF state, the proposed compact bank
uses about 1,273 floats; two banks plus followers, gain and staging need
about 2,806 floats as a conservative original design bound. The compact
Rust layout uses 1,071 floats per bank and 2,402 for the complete
source-rate vocoder. With XMOD and the generated host-rate boundary,
`dual-mod` requires 23,833 floats at 8 kHz, 7,853 at 44.1 kHz, 7,513
at 48 kHz, 2,466 at the exact 96-kHz bypass, and 7,561 at 192 kHz.
Installation preflights the rate-specific region. Source-only 96-kHz numerical
metrics come from `mise run probe-warps-vocoder` against a separate pinned
checkout; its temporary compilation unit excludes oscillator waves.

Host-rate conversion feeds the same 96-kHz bank for every supported
engine rate (8–192 kHz). It generates 33-phase Blackman-windowed sinc
input and output tables at installation, with 48–576 taps according to
rate. Separate carrier/modulator input rings and one output ring have
bounded fractional positions stored as high/low words, so counters do
not grow with session duration. Cutoffs follow the smaller input/output
Nyquist rate. The symmetric filters add rate-dependent startup and group
delay around the internal 60-frame staging; tests bound the measured
impulse onset and verify unity, pitch and alias rejection at rate extremes.
Rendering performs no allocation or trigonometric coefficient work.
At 96 kHz, the boundary is bypassed exactly to retain the measured
source-rate sequence. Generated host FIR coefficients are Vactr data,
not upstream wave tables. Host-rate filtering, carrier generation,
amplifier smoothing and source hardware scaling remain numerical
adaptations, not firmware parity claims.

Rings `Part` has six registered models. `src/dsp/ported/rings.rs` publishes
their pinned order; `rings-voice` runs original fixed-state adaptations of
modal, sympathetic-string, string, FM, quantized sympathetic-string and
string-with-short-diffuser roles. Its 14 authored controls plus event pitch and
main/aux selection occupy 16 UGen ports, and all appear in `.vact`/editor.
The memory budget scales with the installed sample rate: 18,776 floats at
44.1 kHz, 20,336 at 48 kHz and 39,536 at 96 kHz per two-output voice.
Four preallocated delay lines track string fundamentals down to 20 Hz at each
rate; lower pitches clamp to 20 Hz. This four-line architecture and its
excitation/filter numerics differ from Rings' eight-string topology. Its event-local state also differs from shared Part voice
stealing. `string-choir-voice` separately adapts StringSynthPart with four
analytic chord/comb voices, authored registration and chord arithmetic, and
six distinct formant/chorus/ensemble/short-reverb FX selections. All four
Patch controls and the event-local performance controls are codeable. Its
two-output state uses 20,402/22,196/44,276 floats at 44.1/48/96 kHz and
tracks the root comb period down to 20 Hz. The source uses 12 divide-down
oscillators and up to four rotating groups; Vactr's four event-local voices,
short FX, and channel-independent processing are distinct adaptations.
The separate `resonant-bank` stereo bus/master effect now sums L/R input to a
mono external exciter, matching the source `Part::Process` mono input role, and
emits distinct main/aux on L/R. Its 17 codeable controls cover the six model
roles and explicit internal/external mixing, gate and dry/wet mix. The effect
uses four rate-sized comb lines and original modal/FM formulas; stereo summing,
the four-line limit and numerical behavior are Vactr adaptations, not a
source port. No generated Rings resources, registration array or chord table
is imported. Opt-in `rings-external-voice` now reads both validated host
input channels and averages them for its mono exciter; arbitrary bus taps
and source-equivalent shared voice state remain open.

Braids has 47 accessible positions before its question-mark sentinel.
`src/dsp/ported/braids.rs` exposes their pinned order and coverage. Positions
0–4 have the neutral `macro-five-voice` template and positions 5–8 have
`macro-sub-sync-voice`; positions 9–12 have `macro-triple-voice`, and
positions 13–16 have `macro-digital-voice`; positions 17–20 have
`macro-filter-voice`; positions 21–24 have `macro-formant-voice`; positions 25–27 have `macro-fm-voice`; positions 28–31 have `macro-physical-voice`; positions 32–33 have `macro-struck-voice`; positions 34–36 have `macro-percussion-voice`; positions 37–38 have `macro-wave-grid-voice`; positions 39–40 have
`macro-wave-line-voice`; positions 41–43 have `macro-noise-voice` and
positions 44–46 have `macro-cloud-voice`.
Each exposes pitch, a source-position selector,
two timbres, an event-local strike transient and a generated hard-sync clock.
The sub/sync group adds one/two-octave sub roles and master/slave sync roles.
The triple group sums three same-shape analytic oscillators with an original
continuous detune curve in place of the source interval array. The digital
group uses independent three-sine ring, seven-saw swarm and high-pass,
bounded saw comb, and stepped sample-hold toy implementations, with no
source overdrive, filter LUT or toy FIR. The filter group uses phase-reset
carrier, saw/triangle window, pulse and bounded integral roles rather than a
generic analog filter. Its oscillator and integrator numerics differ from source. The formant group
uses original continuous vowel trajectories, analytic bell/formant pulses,
a pulse-grain FOF replacement and a 12-partial harmonic bank, without
source phoneme arrays or generated tables. The FM group uses analytic
two-operator phase modulation, previous-output modulator feedback and
output-dependent bounded modulator rate, with no source sine table or
fixed-point arithmetic. The physical group has one preallocated resonator
per event, with distinct pluck, bow, reed/breath and flute-edge excitation;
it does not preserve source multi-voice pluck rotation or separate waveguides.
The struck group has an 11-partial bell and a six-partial drum with filtered
noise cross-modulation; source fixed-point interpolation and blockwise
retuning differ. The percussion group preserves timed-pulse kick,
six-square and clocked-noise cymbal, and dual-resonance/noise snare roles
with original analytic filters and envelopes. The first two wave shapes
use original periodic functions: a 20-bank scan and a computed 16×16
bilinear grid, rather than source wave samples or index data. The other
two wave shapes use a computed 64-node line and four-voice chord with
original interval families/inversion, not the source wave or chord arrays.
These first forty-one are analytic adaptations, not source-equivalent ports;
positions 32–33 translate small MIT partial/decay maps, while their
synthesis uses analytic sine and host-rate decay. The published fixed-point
oscillators and generated sine/fold/filter tables are excluded. Positions
41–43 adapt LP/BP/HP filtered noise, twin resonant peaks and clocked
held/quantized cyclic noise with original filters and deterministic RNG.
Their source SVF, overdrive, RNG and quantization numerics differ. Positions
44–46 adapt four-grain sine clouds, sparse three-resonator particles and
I/Q symbol modulation with original deterministic sequences. These omit
source envelope/resonator tables, fixed-point arithmetic, symbol data and
numeric parameter curves. Thus all 47 positions are runnable adaptations,
with source fidelity comparisons still open.
Upstream wave-bank positions 37–40
retain an explicit unaudited-wave-asset flag, though no such asset is in the
Vactr implementation. A true per-sample external sync input and source
numeric comparisons remain open.
The upstream `braids/resources/waveforms.py` reads both `data/waves.bin` and
`data/map.bin`; neither binary asset has an individual provenance decision. The runnable
37–40 replacements import neither asset; the manifest keeps their upstream
wave flags as provenance, while marking them Adaptation/Replacement.

The current `texture-grain`, `texture-stretch`, `texture-loop` and
`texture-spectral` bus/master effects cover all four published mode roles
as bounded stereo adaptations with thirteen codeable controls each. Their
`quality` selector uses source bit roles (bit 0 mono, bit 1 low fidelity)
through Vactr dual-mono summing, host-rate half-sample hold and original
8-bit quantization. Source 32/16 kHz conversion, generated SRC filter,
exact diffuser/correlator/pitch-shifter, spectral history and phase
behavior remain unported. Generated
source tables are not imported.
For `texture-loop`, translate the pinned looping player's delay glide,
trigger tap synchronization, freeze loop point/duration, pitch-dependent
64-frame wrap crossfade and four-point cubic capture reads with a bounded,
original interpolator. Preserve Vactr's thirteen control names, stereo
capture and host-rate memory preflight. Its separate live pitch shifter,
density diffusion, texture filter, reverb, gate, quality conversion and
source 32 kHz timing remain disclosed adaptations rather than complete
Clouds parity. No source table, recorded audio or aggregate resource enters
the build.

Streams' published STM32 firmware has six control processor functions, but
the processor returns gain and frequency CV to DAC/PWM outputs for external
analog VCA/VCF hardware. Its firmware does not contain that analog audio
path. Vactr may translate the MIT control algorithms and add its own
digital stereo gain/filter stage, but that combined effect is an adaptation,
not a port of source audio DSP or a complete model of the physical module.
The source `audio` and `excite` inputs, alternate mode, channel linking and
global/local parameters remain part of the control coverage contract; see
`impl-plans/active/modular-streams-controls.md`.
The first two Vactr effects, `stream-envelope` and `stream-vactr`, are
original stereo gain/filter adaptations with AD/AR and damped/plucked modes.
Their optional right-input detector controls both lanes while preserving
separate left/right audio output. The stereo bus cannot yet accept the two
independent audio/excite input pairs of Streams hardware; its authored
scalar excite/trigger controls and right sidechain are a bounded substitute.
The `stream-follower` and `stream-compressor` adaptations add three-band
tracking/filter-only routing and hard/soft-knee compression, respectively.
Linked globals replace local timing or threshold/amount roles. The firmware
still emits CV, and Vactr's filters, ratio curves and two-channel sidechain
mapping are independent digital audio designs. The remaining
`stream-filter` and `stream-lorenz` effects use independently written stereo
filter and bounded chaos DSP to make the source CV roles audible. The filter
role keeps neutral gain while signed amount and offset move cutoff; the
Lorenz role balances gain/filter modulation and swaps x/z control roles
between channels. Source functions 4–5 ignore alternate and linked/global
settings, so these are documented rather than exposed as inactive controls.
All six firmware control roles now have runnable digital adaptations, with
source CV numerics, analog hardware and full two-pair I/O still outside
coverage.

Elements' DSP `Patch` has twenty controls and its `Part` has separate blow
and strike inputs and main/aux outputs. The neutral `elements-voice` now
exposes all twenty Patch fields, gate/note/modulation/strength, three
published internal resonator roles and a separate alternate selector through
a 30-port two-output node contract including optional host blow/strike.
It uses original procedural bow, blow and strike excitation, eight modal
resonances, a single string or four authored strings and short feedback
space. This is an adaptation: source Strings uses five strings, source
exciter/resonator/reverb numerics and sample playback differ. Separate
external blow/strike inputs are now available to opt-in instrument graphs. A
distinct original fourfold-oversampled dual-FM/spatial alternate replaces
the source eightfold/101-tap voice in the same template. A separate stereo
bus/master `elements-bank` adaptation now
routes left audio to blow and right audio to strike excitation, then returns
main/aux on left/right. It exposes the same twenty Patch and four performance
roles, model, external/internal blend and dry/wet mix. It uses four bounded
rate-sized comb lines and procedural modal/reverb stages; its alternate
selector shares the authored FM/spatial kernel. `el-space` extends to 2
and freezes the bounded echo at or above 1.75. Exact source spatial/filter and
oversampling numerics remain pending.
The bundled `elements/resources/samples.py` generator
declares GPL-3.0-or-later and packages ten WAV recordings whose individual
rights have not been verified. Neither that generator nor its bundled
samples may enter the MIT core. Procedural replacement or user-provided
resources must be disclosed; see `impl-plans/active/modular-elements-model.md`.

Frames' `frames.cc` evaluates a keyframer or poly LFO and writes four DAC
codes. `frame-lfo-voice` provides an original analytic four-lane poly-LFO
control adaptation with shape, signed spread, shape spread and coupling,
and independently selectable main/aux lanes. An opt-in four-output host can
load `examples/quad-stems.vact` to emit all four generated lanes at once.
The same opt-in file also provides Tides1's unipolar, bipolar,
end-of-attack and end-of-release roles at the same sample positions; its
EOA remains high after attack/sustain or while idle; EOR stays high while
idle and holds for a host-rate-scaled ~1 ms after a low-frequency wrap
(~4 ms in low range), shortening above the source-derived threshold.
Source fixed-point edge and block timing still differ.
Channels 1/2 follow stereo bus/master processing; channels 3/4 are direct
post-voice-gain stems and bypass those effects. A two-output host rejects
these quad templates at install while retaining the default stereo templates.
`frame-keyframe-voice` accepts 1–64 strictly ordered
timestamp/four-value rows as a flat `.vact` list, exposes six analytic easing
choices and per-lane digital response, and independently selects two lanes
by default. The quad example uses four bounded keyframe payload slots, one
per generated lane.
The list has no structured editor yet, and the numeric DAC/VCA response is
not source-equivalent. Its mixer/VCA audio path is analog, outside the firmware. A Vactr
digital mixer driven by translated frame controls is therefore an original
audio adaptation, not a source audio DSP port. Stages does contain a
segment generator with an explicit audio-rate oscillator path. The
`stage-voice` adaptation exposes four segment types and selectable decay,
timed-pulse, gate, sample/hold, free/tap LFO and free/PLL audio oscillator
roles. Fourteen of its 16 registered `ProcessFn` cells have analytic runnable
counterparts; two intentional silent cells are excluded. The two source
Delay cells share a host-rate, secondary-controlled CV-delay adaptation:
primary is the delayed value, and gate/loop do not affect that mode.
Separately selected linked modes remain pending. Its main value
and auxiliary phase/wave outputs are event-local, not source-equivalent
simultaneous multi-segment firmware outputs. A separate `stage-chain-voice`
adapts one six-segment module with independently codeable per-segment type,
loop, primary and secondary settings, plus gate/trigger and separate
value/phase lanes. The exact source transition table remains pending. The opt-in `stage-linked-voice` takes an
immutable 1–36-row flat list, each row `[type loop primary secondary]`, and
emits value and phase through independently addressable main/aux lanes. It
replaces hardware serial chaining with one event-local group. A source-shaped
sequencer layout has at least three rows: a non-looping ramp, hold, or
alternate director followed only by step rows. Row 0 primary fixes an
address or applies an event-onset reset; secondary chooses up, down,
ping-pong, alternating, random, random-without-repeat, or addressable
traversal. A gate rising edge clocks
the selected step; trigger can be a separate reset. Loop flags on step rows
bound the traversal range. Step primary controls the value and secondary its
slew; main emits the smoothed value and auxiliary reports bounded step
position. This event-local, deterministic path is runnable through the opt-in
`stage-sequencer-voice` example. It preallocates state, exposes the immutable
list and gate/trigger controls in `.vact`, and uses authored within-event
clock edges. A constant director reset value acts at event onset, permitting
subsequent clocks; the source instead has held reset/inhibit behavior. The
normalized-position auxiliary also differs from the source's zero phase.
Source slave monitoring, sentinel transitions, per-step live modulation,
exact hysteresis/clock-inhibit, quantized output and numerical timing remain
separate fidelity gaps.

`edges` contains a digital oscillator but is GPL-3.0 AVR source; do not copy
or port that implementation into the MIT codebase. A separate, independently
written chiptune oscillator may cover its musical function, but is not an
Edges code port. `beads` has no published source in the audited official
tree, so no source port can be claimed. Analog-only Blades, Blinds, Ripples,
Shelves, and Veils have no firmware DSP to port; Vactr may model their
audio behavior separately, but that is a new design rather than a code port.

The Peaks FM drum is the first concrete vertical slice. Its published source
has a twelve-position function registry. The current public Peaks inventory
marks envelope, LFO and tap-LFO as original analytic adaptations through
`peak-motion-voice`, with full/half controls, edge-triggered tap/sync and two
Vactr output roles. It also counts three drum adaptations and the FM drum
source-stage translation; none is a source-equivalent port. Pulse shaper,
pulse randomizer and bouncing ball run as separate modes of
`peak-pulse-voice`: shaper delay/duration/
repetitions, randomized acceptance/repetition/delay, and ball gravity/loss/
height/velocity retain their full/half control roles. One trigger replaces
the active train; source pulse overlap queues, generated delay/gravity tables
and quantized tick timing are absent, so all three remain adaptations. Mini
Sequencer is outside the audio-DSP scope. Number Station now has an original
tone and ten-syllable procedural voice replacement with its full/half control
roles and event-local digit transitions. Its recorded digit asset remains
unaudited and unused; speech content and timing differ from the source. The new motion
voice uses analytic waves/curves and seeded noise, never Peaks generated
waveforms or binary data; output phase and numerical behavior differ.

The Peaks FM drum is the first concrete percussion vertical slice. Its published source
exposes frequency, FM amount, decay and noise/drive. Vactr must expose
those controls separately where the source combines them, along with the
trigger and envelope state; no hidden fixed preset may replace a user-settable
control. A second FM/noise/feedback percussion voice has been located in an
MIT-declared Faust source. Audit its Faust library dependencies before porting
any code; if dependencies do not permit MIT distribution, implement the
described synthesis structure independently. Both voices need distinct
`.vact` names and parameter schemas.
For the Peaks FM drum fidelity pass, preserve the source's single sine phase,
FM and auxiliary pitch envelopes, amplitude envelope, delayed pitch feedback,
noise mix and soft overdrive stages. Keep Vactr's separately authored
`fm-amount`, `pitch-sweep`, `decay`, `drum-noise` and `drive` controls and
event reset; expose each through `.vact`. Recreate only continuous analytic
curves from the individually MIT-noticed Peaks generators, never copy their
generated sine, envelope, pitch, or overdrive tables or preset maps. Scale
time and pitch to the host rate with fixed callback state and no allocation.
The source's 16-bit quantization, every-four-sample pitch update, trigger
flags and numerical parity require a separate reference comparison before a
full source-port claim.
The source imports `stdfaust.lib`: its oscillator, envelope, noise, and basic
math library files carry LGPL terms with an exception for Faust-generated
compiled output, while `fi.resonbp` is separately marked `LicenseRef-STK-4.3`.
A manual Rust rewrite is not Faust-generated output, so write these primitives
independently and retain only the gist's MIT-declared composition and control
intent. Record all upstream credit and notices when that voice lands.

The `ctag-fh-kiel/md-drum-synth` repository contains several further FM drum
models, but its inspected root has no license grant. Do not incorporate its
source without a license from its copyright holder. The MIT-licensed `0x808`
repository has a general four-operator FM synthesizer, rather than a separate
FM drum engine; it can inform a future configurable FM instrument only after
individual source and bundled-asset checks. These findings do not count as
implemented percussion voices.

Erik Larsson's 2000 Chalmers EFM thesis and the Elektronauts voice-diagram
discussion are useful research references for an original FM percussion
family. The thesis covers bass drum, snare, tom, clap, rimshot, cowbell and
cymbal designs and discusses FM signal flow. Treat the papers and posted
diagrams as explanatory material: do not copy diagrams, assembly, tables,
presets or sample data into Vactr. Implement and test original Rust DSP with
Vactr parameter names, and attribute the research references in the design
record. Do not imply an official Elektron implementation or endorsement.
The official Elektron manual describes the EFM family at product level; it
does not license firmware or sound assets. `feedback-metal-drum` is a distinct
Vactr coupled-FM percussion voice: an analytic carrier and self-feedback
modulator, separate body/modulator/noise decays, noise excitation, filter and
drive. Every declared sound parameter is `.vact`-codeable. It is neither a
translation of the thesis' assembly/diagrams nor a hardware emulation.

The Gearmulator Machinedrum emulator is GPL-3.0 and requires separately
copyrighted original firmware images to reproduce the machine. Its source and
firmware cannot be incorporated into Vactr's MIT DSP core. The `eseq`
Machinedrum-oriented work is likewise GPL-3.0. Both may be studied as public
documentation, but neither supplies a permissively licensed emulator port.
The EFM-inspired family is an independent synthesizer implementation, not a
claim of hardware emulation or sound-for-sound equivalence.
MAME's Machinedrum file is a BSD-3-Clause skeleton hardware driver, not a
standalone drum synthesis engine; its permissive header does not make the
original firmware redistributable or provide a DSP voice to port.

## Vactr interface and execution

An instrument is triggered by `s :<vactr-name>` in a pattern; an effect
can be placed in an `inst` graph, a bus, or `master` as appropriate. Stereo
processors retain two audio inputs and two outputs. Engine/model selection is
an explicit typed parameter. Every sound-relevant knob, mode, trigger,
external modulation input, and secondary output has a named Vactr port or
parameter. The editor obtains the same schema used by the checker and event
commit; unknown or unsupported controls cause diagnostics. Pattern and live
cell values reach the correct engine without silent drops.

Port state is preallocated at install time. Audio callbacks allocate nothing,
do no file/network I/O, and remain bounded by host capability limits. Sample
rate conversion and block-size adaptation are explicit where original DSP
assumes a fixed rate/block. State resets on triggers and template swaps are
defined so live redefinition cannot leak stale state. Resource-backed modes
load through the existing sample/table host path, with missing-resource
diagnostics. Browser and native hosts use the same Rust core and expose the
same parameters; a tier may decline a mode with `beyond-capability`, never
silently omit it.

## Completeness evidence

For each source engine/mode, keep a mapping from upstream file and revision
to a Vactr instrument/effect, parameter manifest, resource provenance,
and tests. Compare deterministic output properties (finite samples, trigger
response, parameter effect, channel behavior) with the upstream engine where
legal resources are available. Audible review supplements automated checks.
Every declared parameter must be settable from `.vact` and shown by editor
metadata. A coverage report lists ported, independently substituted, and
unavailable entries; the project cannot call this task complete while an
eligible engine or control remains unaccounted for.

The first checked inventory is `dsp::ported::plaits_algorithms()`: 24 entries
in the pinned Plaits `Voice::Init` order, with separate fidelity and asset
statuses. `dsp::ported::plaits_coverage_summary()` provides a readable count.
Run `CARGO_TERM_QUIET=true cargo run -q --example plaits_coverage` to print it.
The MOD-006 source comparison runs pinned upstream DSP from a separate local
checkout, never from the shipped Vactr binary. A first, repeatable Plaits
position-10 probe compares raw FM carrier/sub kernels at 48 kHz in 24-frame
blocks for the same note and control values. The upstream probe may compile
its local aggregate resource object, but the harness must neither copy that
object nor its generated tables, presets or output samples into this
repository. The Vactr side runs its current independent FM kernel. Report
both channels' level and waveform error after warm-up; mismatches are
measured gaps, not an automatic test failure or evidence of source parity.
This kernel comparison does not verify the outer Plaits voice, LPG, trigger
or `.vact` event behavior. Extend it to other modes only after matching
each mode's control and resource boundary.

The FM probe now covers three harmonics/timbre/morph settings with the same
note and reports each channel separately.

For Plaits FM position 10, the pinned MIT `plaits/resources/lookup_tables.py`
defines the quantized modulator-ratio sequence and four scalar downsampling
coefficients without reading external binary assets. These two small numeric
resources may be translated individually with their MIT attribution; do not
import the aggregate `resources.cc`, generated sine wavetable, DX7 bank or
other embedded data. Translate the source's ratio interpolation, frequency
and amount curves, signed feedback, four phase advances and two-output FIR
state into preallocated Vactr state. Keep analytic sine and document its
remaining interpolation difference, as well as host-rate and outer-voice
behavior. The source's 24-frame parameter-interpolation period at 48 kHz
must retain its roughly 0.5 ms duration at other host rates and continue
across arbitrary callback and late event boundaries rather than use callback
length as glide duration. Re-run the separate comparison
at several control settings before
changing any fidelity label. A raw-kernel match alone cannot certify a full
Plaits source port.

`SourceStage` means the source signal stages are present but source/host
numerical parity remains unverified; only `SourcePort` would indicate a
validated full port. Plaits positions 2–4 now use three original six-operator
FM banks (`six-bank-{a,b,c}-voice`), each with 32 procedural configurations,
as Adaptation/Replacement. The upstream DX7 preset banks remain excluded,
and the DX7 flags in the manifest record source provenance only. Position 15
now provides `speech-voice`, an Adaptation/Replacement with three procedural
naive/SAM/LPC-like modes and four original upper-range Vactr syllable
tokens (`ava`, `omi`, `era`, `unu`). Its TI ROM flag records upstream
provenance only; no speech-ROM words or source phoneme tables are imported.
All 24 Plaits positions are runnable adaptations or source-stage
translations, with no validated source-parity port. The MIT wavetable generator reads
`waves.bin`; its individual asset origin is still unaudited, while position
13 uses a separate original procedural replacement.
Position 12 `spectrum-voice` translates the MIT additive engine's spectral
gain/normalization and high-frequency taper stages, but uses analytic sine
and host-rate smoothing instead of its generated sine table, 12-sample block
interpolation and normalized-state recurrence; it is `SourceStage`, not a
numerically validated port.

Position 18 `particle-voice` is a runnable adaptation of the six-particle
resonant/impulse architecture with a shorter original diffuser. Its manifest
status is `Adaptation` until the upstream diffuser, filters and event timing
are compared and source-stage parity is established.

Position 17 `clock-noise-voice` must preserve the pinned MIT engine's two
independent sample-and-hold clocks, quadratic BLEP correction at each clock
edge, frequency-dependent input gain, one multimode state-variable filter
for main and two band-pass state-variable filters for auxiliary. Pitch sets
the first filter center, harmonics offsets the second center and sweeps the
main low-pass/band-pass/high-pass mix, timbre sets both clock frequencies,
and morph sets resonance. Each `.vact` event resets both clocks and filter
states with deterministic per-voice randomness. Convert source-normalized
frequencies and clock timing to the host rate, including 44.1/48/96 kHz and
64/256-frame operation; persist state across callbacks without allocation.
The upstream global random stream, fixed-block parameter interpolation,
trigger-unpatched behavior and full voice/LPG rendering need separate
comparison before `SourcePort` can be claimed.

Position 20 `modal-voice` translates its MIT 24-mode resonator and strike or
dust exciter with separate main and auxiliary outputs. Its stiffness curve is
evaluated from the MIT generator formula without importing aggregate resource
data. The manifest marks it `SourceStage`; interpolation, filter, random and
block-smoothing differences still require upstream comparison.

Position 14 `chord-layer-voice` provides a four-note/five-slot inversion
chord with divide-down registration blended into fifteen original analytic
timbres. Main carries the full chord and aux emphasizes selected inversion
notes. The source's fifteen integrated waves come from unaudited
`waves.bin`; Vactr imports none. The manifest marks the runnable voice
`Adaptation`/`Replacement` and retains WAVES as upstream provenance.

Position 13 `wave-grid-voice` scans an original analytic 8×8×3 wave
family with mirrored z navigation, a continuous main path and a 1/32-step
auxiliary. The source reads `wav_integrated_waves` from unaudited
`waves.bin`; Vactr imports neither and marks its runnable implementation
`Adaptation`/`Replacement`, retaining WAVES solely as upstream
provenance. Custom/user wave mapping is unavailable pending a data contract.

Position 5 `terrain-voice` renders the five source analytic terrain roles
and three original procedural replacements for upstream modes that read
`wav_integrated_waves` from unaudited `waves.bin`. It has distinct terrain
main and transformed auxiliary paths. The manifest classifies the runnable
voice as `Adaptation`/`Replacement` while retaining the WAVES flag solely
as upstream provenance. Source user-terrain mode 8 remains unavailable
until a cleared user-data contract exists; no upstream table is imported.

Position 6 `string-machine-voice` is a runnable four-voice divide-down
chord and stereo ensemble adaptation. Its source does not read the
integrated-wave assets despite broad `resources.h` includes. The ensemble
indirectly reads the generated sine table through `SineRaw`; Vactr uses
analytic sine instead. The manifest clears the wave-asset flag and labels
the bounded implementation `Adaptation` because registration, filters,
oscillator antialiasing and ensemble topology differ.

Position 9 `shape-voice` is a runnable variable-slope/fold main and
sine/overtone auxiliary adaptation. Its manifest status is `Adaptation`:
independent analytic transfers replace the upstream generated waveshaper
and fold tables, while source integrated-BLEP and interpolation differ.
The upstream generator labels its curves as borrowed from Tides; no such
curves or aggregate resources are imported.

Position 11 `grain-pair-voice` has a two-grainlet main and Z-oscillator
auxiliary. Both grainlet resets and the Z half-cycle discontinuity use the
pinned MIT quadratic two-frame BLEP method with crossing-time values and
separate pending-sample state. Each output uses the source dirty-tangent
one-pole high-pass. All note/harmonics/timbre/morph controls remain in
`.vact`, and state stays fixed and partition-invariant at 44.1/48/96 kHz.
The manifest is `SourceStage`: analytic sine replaces the generated sine
table, while per-sample controls omit source block and crossing-time
interpolation. Source VOSIM is commented out and not rendered. Numerical
comparison and full voice/LPG parity remain before `SourcePort`.

Position 19 `string-voice` is a bounded three-string adaptation with filtered
noise/dust excitation and two outputs. Vactr's per-event voices cannot
preserve upstream shared-string rotation and pitch history across events;
the manifest labels it `Adaptation` until that lifecycle difference and DSP
details are addressed.

Position 7 `chip-voice` offers a cleared chord or internally clocked
arpeggio main output and stepped-triangle bass auxiliary output. It retains
MIT chord interval source data with notice and imports no generated audio
resource. The internal clock and analytic oscillators differ from the
source's persistent external clock and BLEP oscillators, so it is an
`Adaptation`.

Position 8 `analog-pair-voice` follows the pinned `VA_VARIANT 2` routing:
variable square/saw main and synchronized oscillator-difference auxiliary.
Its analytic transitions omit source polyBLEP and exact interpolation, so
the manifest classifies it as an `Adaptation` distinct from position 0.

## References

See `design-docs/references/README.md` for the official source and license.
