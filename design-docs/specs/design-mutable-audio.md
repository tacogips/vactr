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
may override its authored `ex-gate 1` and input-level defaults. This is an authored event-local
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
their pinned order; `resonator-voice` runs original fixed-state adaptations of
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
is imported. Opt-in `resonator-external-voice` now reads both validated host
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
and strike inputs and main/aux outputs. The neutral `exciter-voice` now
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
bus/master `exciter-bank` adaptation now
routes left audio to blow and right audio to strike excitation, then returns
main/aux on left/right. It exposes the same twenty Patch and four performance
roles, model, external/internal blend and dry/wet mix. It uses four bounded
rate-sized comb lines and procedural modal/reverb stages; its alternate
selector shares the authored FM/spatial kernel. `ex-space` extends to 2
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
not source-equivalent. Frames' mixer/VCA audio path is analog and outside
firmware DSP. The original `keyframe-mixer` bus/master effect adapts the
four-lane analytic poly-LFO control role to digital gain mixing; it does not
use the keyframe payload. The stereo bus input supplies correlated lanes
`L`, `R`, `M=(L+R)/2`, and `S=(L-R)/2`. Poly-LFO values `g0..g3` drive the
wet recombination `L'=(g0*L+g2*M+g3*S)/2`,
`R'=(g1*R+g2*M-g3*S)/2`. All-one gains preserve stereo input, and `mix`
applies the usual dry/wet blend. These four derived lanes come from two
inputs, not four independently routable sources. The effect reuses Vactr's
existing poly-LFO implementation and does not model the analog mixer/VCA.
It is an original audio adaptation, not a source audio DSP port. Stages does contain a
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

Source and resource rights are recorded per file in
`verification/upstream_inventory.toml`. Each upstream file that Vactr
names has its license (read from the pinned header), a kind (code,
generator, aggregate resource or binary asset) and a use: `source` for
consulted or translated MIT files named in a notice, `partial` for an
aggregate resource of which only the listed tables are translated, and
`excluded`, with a reason, for everything else. Run
`VACTR_MI_REFERENCE=/path/to/eurorack mise run audit-upstream` against a
separate checkout at the pinned Eurorack and `stmlib` revisions. The audit
fails when the checkout is modified, or when a declared license differs
from its header. It also fails in each of these cases:

- a non-MIT file or binary asset is used;
- Vactr names an upstream path that is missing from the pinned tree or
  from the inventory;
- a used file is named by no notice;
- a non-MIT file in the transitive `#include` closure, including
  `stmlib`, is not explicitly excluded;
- a tracked Vactr file is a byte-identical upstream copy;
- a numeric window from an excluded or partial resource table or binary
  asset appears among Vactr's literals without a declared import.

Floating-point windows are six values long. Integer and byte windows are
twelve values long, so that short musical interval lists do not match by
chance. The only declared table import is the twenty `fb_*` Warps
vocoder filter rows. The only non-MIT file in the closure is the
GPL-3.0 `stmlib/ui/event_queue.h`. It is reached through `frames/ui.h`
from the cited Frames firmware main, and it is excluded. The GPL-3.0
`elements/resources/samples.py`, all `waves.bin`, `map.bin` and
`digits.bin` assets, and the aggregate Braids, Clouds, Plaits and Tides
resources are excluded. The audit is a provenance and non-import check.
It does not establish that the unaudited wave assets are cleared, and it
does not establish source fidelity.

User-facing names must not reuse upstream module names.
`src/dsp/ported/tests.rs` rejects any template, effect or
editor-declared control whose hyphen tokens include a Mutable Instruments
module name. Thus Elements, Rings and Tides2 surface as `exciter-*`,
`resonator-*`/`reso-*` and `tidal-poly-*`, and Braids controls as
`macro-*`. Every published audio family has an ordered inventory in
`dsp::ported`, including Clouds (`texture-*`) and Warps (`dual-mod`,
`shift-pair`). Each inventory row must resolve to a registered name.

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
each mode's control and resource boundary. The outer voice layer has its
own design and comparison: see "Plaits voice-level trigger and low-pass
gate layer (PLV-001)" below.

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

## Stereo and multi-output UGen edges (MOD-004)

### Problem

The evaluator graph (`dsp/graph.rs`) represents a UGen output as a node
reference, and `Edge` identifies only `from`, `to`, and destination `port`.
Lowering in `dsp/build.rs` therefore cannot name which output of a source
node feeds an input. The compiled template (`dsp/ugen/template.rs`) stores
one `Src::Node` per input, and `dsp/voice.rs` renders one mono buffer per
node. `aux-out`, `out3`, and `out4` are special side-routing taps rather
than ordinary graph outputs.

This makes dual-output engines awkward: 36 prelude templates, such as
`filter-voice`, `phase-pair-voice`, and `resonator-voice`, instantiate an
engine twice, often with separate `mode` values, to recover main and aux
signals. Where the two instances are really one engine state, that
duplicates state and work. Stereo sampler output cannot
remain stereo through UGen edges. Voice-local `FxUnit` nodes accept stereo
kernel slices but the UGen path copies a mono input to both sides and
averages the result. A voice with main and aux outputs also bypasses the
ordinary pan mapping, while its voice-local post effects are applied to
each side separately only because of the special aux path.

### Goals

- Give every UGen node a bounded, declared set of audio outputs. Each
  output has a channel shape (`mono` or `stereo`); an edge selects the
  source output index and connects its whole channel shape to one typed
  destination audio input.
- Preserve main/aux and true stereo signals through arbitrary intermediate
  UGens and voice-local stereo effects without implicit downmixing.
- Keep mono templates and their current audio, pan, and control behavior
  unchanged unless a user edits the template to opt into a multi-output
  source.
- Keep all output buffers, effect state, and routing storage bounded and
  allocated before callback execution. Preserve rate and block partition
  invariance of stateful nodes.
- Keep native and browser graph installation interoperable when both peers
  are built from the same revision.

### Non-goals

- Dynamic channel counts, unbounded output lists, arbitrary audio buses per
  node, or implicit up/downmix matrices. This change caps channels per
  output at stereo and output ports per node at a fixed compile-time limit.
- Changing the public host event record or promising compatibility between
  mixed source revisions. The current host contract explicitly requires
  native/browser peers from the same revision; mixed revisions are
  unsupported.
- Replacing bus/master processing, adding surround layouts, or certifying
  Mutable source parity for an engine merely because its output topology is
  represented correctly.
- Automatically converting every old template to a new sound. Existing
  mono and explicit `aux-out` graphs remain supported during migration.

### Chosen design

#### Typed outputs and edges

The evaluator and compiled graph should carry a bounded output descriptor
for each node: output count plus a channel count for each output. A node may
publish multiple mono outputs (for example, main and aux), or a stereo
output (for example, a stereo sampler). Audio inputs declare an accepted
channel shape; control-valued inputs remain scalar and keep the existing
`MAX_PORTS` control/parameter budget. An edge records the source output
index in addition to `from`, `to`, and destination input `port`. Lowering
and template compilation validate that both indexes exist and that channel
shapes match. A shape mismatch, invalid output index, or output-capacity
overflow is a definition-time diagnostic; it must never silently select
output zero or drop a channel carried by an edge.

The initial fixed limits should be four output ports per node, two channels
per output, the existing `NODE_CAP = 256`, `MAX_PORTS = 48`,
`MAX_PARAMS = 48`, `MAX_EDGES = 1024`, and `MAX_VOICE_FX = 4`. Set the
initial engine-wide audio-buffer arena cap to `MAX_AUDIO_BUFFERS = 512`
channel slices (`NODE_CAP * 2`); at the existing maximum block of 8192
frames this bounds the arena to 16 MiB of `f32` storage. Keep output-port
count distinct from channel count:
`main` plus `aux` is two mono outputs; a stereo sampler is one two-channel
output. Output buffers are assigned densely at template-build time, with
the per-template total channel-buffer count checked against a fixed engine
capacity. Do not reserve the worst-case output count for every node in each
voice. Report the failing node and required/available count through a
specific build fault that the language layer can surface as `graph-too-large`
or a typed graph error. Never fall back to mono on exhaustion.

Every existing mono UGen is described as one mono output and keeps its
current kernel semantics. Existing `aux-out`, `out3`, and `out4` remain
the voice-boundary taps they are today (single mono input, silent graph
output); they are not output ports and never make an output-index edge
ambiguous. Multi-output nodes publish all outputs from one state instance
per sample.

Output declarations are fixed per node kind. They are never inferred from a
resource, an event, or the callback:

| Node kind | Outputs (index `:name` shape) |
|---|---|
| Every kind not listed below | 0 mono (no names; selection is an error) |
| `va-filter`, `fm-pair`, `analog-pair-core`, `chord-layer-core`, `wave-grid-core`, `terrain-pair-core`, `string-machine-core`, `shape-pair-core`, `stage-chain-core` | 0 `:main` mono, 1 `:aux` mono |
| `sample-play` | 0 `:mono` mono, 1 `:stereo` stereo |
| Effect used as a voice node | 0, mono or stereo, equal to its subject input's shape |
| `*` (`Mul`), `+` (`Add`) | 0, stereo when any audio operand is stereo, otherwise mono |

For each dual kernel in the table, output 0 is exactly today's
single-output signal: the path chosen by the node's existing selector port
(`mode` or `chain-channel`), read at the same per-block or per-sample point
as today. Output 1 is the complementary path (the one that selector value
would not choose), computed in the same sample loop from the same state. So
a legacy `mode: 1` instance still emits the auxiliary path on output 0. A
kernel belongs in this table only if it passes the eligibility rule under
"`.vact` selection and template migration" below.

`sample-play` output 0 is today's output unchanged: all resource channels
averaged. Output 1 is stereo from the same playback cursor, speed, region,
and loop state. A mono resource writes its single channel to both sides; a
resource with two or more channels writes channel 0 left and channel 1
right. The per-event `bank` override keeps working because no shape depends
on the resource.

Effect nodes accept a mono or a stereo subject. With a mono subject they
behave exactly as today (copy to both kernel sides, then average). With a
stereo subject they pass left/right to the kernel and publish its stereo
result. Effect parameter inputs stay scalar. For `Mul` and `Add`, once any
audio operand is stereo, every other audio node operand must also be
stereo; `Const`, `Param`, and signal-control nodes apply to both channels.
Any other stereo edge into a mono-only input is a shape-mismatch diagnostic.

Lowering invariant: for a graph that uses no selection and no stereo
source, lowering produces the same nodes, node order, edges, edge order,
and ports as before, with `output = 0` on every edge. This keeps
topological order, and therefore the per-node noise seed
(`voice.seed + node index`), unchanged for every unmigrated template.

#### `.vact` selection and template migration

Output selection calls the multi-output UGen value with an output name
(owner decision, 2026-09-29). This is consistent with how Vactr already
accesses dicts and variants by calling them with a key, so no new reader
syntax is needed. Examples are `(p :main)` and `(p :aux)` for
`let p (fm-pair ...)`, or `(s :stereo)` for `let s (sample-play ...)`. A
numeric index such as
`(p 1)` is the unambiguous fallback. The selected value remains a UGen
expression and can be passed through any existing arithmetic, effect, or
input port. An output name is valid only when the node kind declares it in
the table above (`:left`/`:right` in the owner-decision examples are
illustrative; no node in this scope declares them). Using a multi-output
node directly, without selection, keeps today's meaning: its first output.

Notation: this section writes calls in s-expression form, such as
`(p :aux)`. In `.vact` source, `( )` is not syntax; the lexer reports
`paren-form`. The same call is written `p :aux`, or `{p :aux}` where
grouping is needed, just as a dict key call is written `{d :gain}`. So the
migrated form below is, in source,
`{p :main} > * amp > + {{p :aux} > * amp > aux-out}`, with the node bound
by `let p {...}`.

Selection data flow:

- **VM**: calling a `Value::UGen` is added to the existing
  collection-call path (the one that handles `(d :k)` for dicts). It takes
  exactly one argument, a keyword or an int, and returns a new
  `Value::UGen` that wraps the source node reference and the output index.
  The wrapper is evaluator-only. It never becomes a graph node, and it does
  not count toward `NODE_CAP`.
- **Definition-time failures** are raised while the `inst` body evaluates,
  so the `inst` definition fails:
  - `not-callable` for selection from a single-output node, including a
    value that was already selected;
  - `unknown-field` for an undeclared name or an out-of-range index;
  - `type` for an argument that is not a keyword or an int;
  - `arity` for zero or more than one argument.
- **Static checker**: a `ugen`-typed callee with one keyword or int
  argument has type `ugen`. The checker does not know which kind of node it
  is, so it does not validate names; the VM does. Any other argument count
  or type is a `type-mismatch` diagnostic, and the result is still `ugen`.
- **Lowering**: lowering memoizes nodes by source pointer. `(p :main)` and
  `(p :aux)` therefore resolve to the same lowered node and emit edges with
  `output = 0` and `output = 1`. Lowering then checks channel shapes (see
  above), the voice layout (see "Voice buffers and channel mapping") and
  the channel-buffer total (see Capacity). A failure is a typed lowering
  diagnostic on the `inst` definition, which is also definition time. A
  buffer-total failure uses the existing `graph-too-large` path.
- **Selected root**: an `inst` body whose value is a selection of output
  `k` lowers the source node as usual. If `k = 0`, that node is the root,
  as if unselected. If `k > 0`, lowering appends one `Add` node fed by
  output `k` on port 0, with port 1 unconnected. That node is the sink,
  because a sink contributes only its output 0. This is the only case in
  which a selection adds a graph node, and that node counts toward
  `NODE_CAP`.
- **Bus bodies**: selection is valid only in an `inst` body. A selection
  reached while lowering a `bus` body is a lowering type error.

**Migration eligibility.** A duplicate main/aux template migrates only if
all of these hold:

- both instances have identical inputs apart from the selector;
- the selector only chooses which already computed signal is written, so
  no state update depends on it;
- the kernel reads no per-node seed, so merging nodes cannot change a seed;
- main and aux use constant complementary selector values.

The migrated form is `let` binding one node, then
`(p :main) > * amp > + {(p :aux) > * amp > aux-out}`. The bound node keeps
the main instance's inputs verbatim, including its selector value (`mode: 0`
or `chain-channel: 0`), so output 0 is exactly the old main signal and
output 1 the old aux signal. The voice boundary
still uses `aux-out`, so it is unchanged. Nine templates qualify and
migrate, each with a render-equivalence test:

- `filter-voice` (one `va-source` and one `va-filter`)
- `fm-pair-voice`
- `analog-pair-voice`
- `chord-layer-voice`
- `wave-grid-voice`
- `terrain-voice`
- `string-machine-voice`
- `shape-voice`
- `stage-chain-voice`

The other 27 duplicate templates keep their current form and sound:

| Reason | Templates |
|---|---|
| The selector changes state evolution | `phase-pair-voice`, `spectrum-voice`, `grain-pair-voice` |
| The kernel is seeded, so the two instances diverge or merging shifts seeds | `speech-voice`, `clock-noise-voice`, `dual-kick-voice`, `dual-snare-voice`, `dual-hat-voice`, `swarm-voice`, `string-voice`, `particle-voice`, `resonator-voice`, `string-choir-voice`, `exciter-voice`, `chip-voice`, `stage-voice` |
| Seed-dependent only under some parameter settings | `modal-voice`, `peak-motion-voice`, `peak-pulse-voice`, `number-station-voice` |
| A patterned header control chooses the lane, so a merged node would need a second selector port | `tidal-voice`, `tidal-poly-voice`, `frame-lfo-voice`, `frame-keyframe-voice` |
| Identical instances with no selector: a deduplication, not a multi-output kernel | `six-bank-a-voice`, `six-bank-b-voice`, `six-bank-c-voice` |

Old duplicate forms, including the pre-migration text of the nine migrated
templates, stay valid and are never rewritten automatically. Manifest
`main_outputs`/`aux_outputs` fields in `src/dsp/ported/*.rs` describe the
upstream module and do not change. `aux-out` keeps working unchanged.

#### Voice buffers and channel mapping

The callback keeps its existing node-buffer arena model but allocates a
template-validated dense range of channel buffers for each compiled output.
An edge reads the selected output's one or two slices, and a node writes
each declared output into its assigned slice(s). Runtime loop bounds come
from the compiled descriptors; no allocation, lock, graph traversal, or
shape inference occurs in `process()`.

Sinks keep today's rule: a node with no outgoing edge is a sink, and a sink
contributes its output 0. An output that no edge consumes is discarded; it
is neither an error nor folded into another channel. The node's shared
state still advances exactly once per sample. When a dual-output node's
output 1 has no consumer, the node runs its existing single-output kernel
path, unchanged. The paired kernel path (both outputs from one state
update) runs only when output 1 is consumed. Eligibility guarantees that
the two paths evolve state identically, and the unchanged path is what
keeps every unmigrated template bit-identical by construction. The same
rule applies to `sample-play`: its stereo path runs only when `:stereo` is
consumed. The template build fixes one voice layout:

- **Mono voice**: every sink's output 0 is mono and there is no
  `aux-out`. It uses today's equal-power `pan`, unchanged.
- **Main/aux voice**: the graph contains `aux-out`, as today's `has_aux`
  templates do. Main maps to left and aux to right.
- **Stereo voice**: some sink's output 0 is stereo. Every other audible
  sink must then be stereo too; `aux-out`, `out3`, and `out4` taps are
  silent in the graph. Left maps to left and right to right. A stereo voice
  that also contains `aux-out` is a shape-mismatch diagnostic, because it
  has no free auxiliary channel.

For main/aux and stereo voices, `pan` is unity-center balance (owner
decision). With `p` clamped to `[0, 1]` exactly as `pan_gains` clamps it,
the left gain is `min(1, 2(1 - p))` and the right gain is `min(1, 2p)`. At
the default `p = 0.5` both gains are exactly `1.0`. This covers today's
`aux-out` voices too, which currently ignore `pan`. They render
bit-identically at center pan, including templates that read `pan`
themselves, since those keep the voice at 0.5. At any other pan they now
attenuate the opposite side, which is the owner-approved behavior change.
The law is a new public helper beside `pan_gains` in
`src/dsp/effects/prim.rs` that takes the same `0..1` voice `pan`. It is
separate from the private `-1..1` balance control of the spatial effects
in `src/dsp/effects/spatial.rs`, which is unchanged.
Voice gain, the implicit envelope, the release fade, the nonfinite guard,
and voice post effects already run on both channel buffers of a
two-channel voice; the right-channel post effect now runs for stereo voices
as well as main/aux voices. The orbit send takes the post-pan left/right
pair, as it does for mono voices today. `out3`/`out4` stems are unchanged.

Voice-local effects follow their subject input's shape (see the
declaration table). A stereo subject gives independent left/right slices,
including the wet/dry blend, exactly as the bus/master path does today,
with no clone-and-average downmix. A mono subject keeps today's mono
behavior bit for bit. There is no implicit mono-to-stereo upmix at an edge.
The only stereo source in this scope is `sample-play`'s `:stereo` output
and nodes derived from it.

#### Codec and compatibility

Extend the graph codec shared by native and browser (`dsp/arena.rs` and
`dsp/ugen/catalog/codec.rs`) so output shapes and selected source output
indexes are encoded deterministically. The evaluator encoder and the
single decoder used by both native and browser change together, in one
same-revision change. The instrument record changes as follows:

- **Record tag**: the instrument record tag changes from `G_INST = b'I'`
  to `G_INST = b'V'`. This is the discriminator the owner decision allows. A
  payload carrying the old tag, or any unknown tag, is rejected with
  `FaultCode::BadRecord`. There is no legacy decoder, and nothing is
  guessed from record length.
- **Shape byte**: one byte follows each node record. Bits 0-1 hold the
  output count minus one. Bits 2-5 are the per-output stereo flags for
  outputs 0-3. Bits 6-7 must be zero, and flags beyond the output count
  must be zero.
- **Edge record**: `from u16, to u16, port u8, output u8`.

The decoder cross-checks every shape byte against the shape that template
build derives from the node kind and its input edges. A mismatch, a
reserved bit, or an out-of-range `output` is `BadRecord`. Bus and master
records are unchanged. Tests add exact byte fixtures and
encode/decode/re-encode round trips for mono, multi-mono (`va-filter`
`:main`/`:aux`), and stereo (`sample-play :stereo` through a stereo voice
effect) graphs. Native and browser peers built from the same revision stay
compatible. Mixed revisions remain unsupported, as the existing wire policy
already states.

#### Capacity, real-time, and invariance

The compile phase checks node count, output-port count, total channel-buffer
count, edge count, effect count, and per-voice effect memory before
installation. Preserve the current 48 input/control slots, 256 node cap,
1024 edge cap, and `MAX_VOICE_FX` limit unless measured fixed storage
requires an owner-approved change. Include output scratch and stereo effect
scratch in engine memory budgeting; reject an install with a capacity
diagnostic rather than partially compiling it. Memory assigned by
`mem_need` remains per-node state and must account for any newly shared
multi-output kernel state once, not once per output.

The node-buffer arena is `NODE_CAP * max_block` today. It becomes
`(MAX_AUDIO_BUFFERS + 2) * max_block`, assigned densely in topological
order: one slice per mono output and two adjacent slices per stereo output.
Slices are assigned only to output 0 of every node and to other outputs
that at least one edge consumes. An unconsumed output with index 1 or
higher writes to two shared discard slices that sit outside the 512-slice
cap. A graph without selection therefore needs at most 256 slices, so no
previously valid template can hit the new limit, not even one made of
many `sample-play` nodes. Lowering checks the total
and reports `graph-too-large` at definition time. Template build re-checks
it with a dedicated `BuildError` variant that maps to
`FaultCode::GraphTooLarge`, so a hostile or corrupt payload cannot overrun
the arena. Invalid output indexes and shape mismatches that get past
lowering are `BuildError::BadEdge` at build time.

The voice mix and pan code lives in `src/dsp/engine/render.rs`, which was
split out of `src/dsp/engine.rs` before the render-path change. Every
touched Rust file stays under 1000 lines; `src/dsp/voice.rs`,
`src/dsp/build.rs` and `src/dsp/ugen/template.rs` move new helpers into
submodules if they would otherwise pass that limit.

All storage is allocated before the engine runs. `decode_graph` and
template build run inside the engine on install, so they derive and check
shapes and slices in fixed-size arrays and do not allocate. The audio
callback only clears and processes already assigned channel slices. There
are no callback allocations and no variable-sized collections. Stateful
multi-output kernels advance shared state once per sample, regardless of
how many outputs are consumed. Rendering the same event/control sequence
at supported sample rates and under different callback block partitions
must preserve the existing rate/block invariance contract.

### Alternatives considered

- **Continue duplicating dual-output UGens and add more taps**: rejected as
  the general design because state and work are duplicated, arbitrary
  intermediate selection remains impossible, and stereo sampler/effect
  routing still needs a separate mechanism. Kept as a valid legacy idiom.
- **One stereo buffer for every node**: rejected because mono nodes pay
  double bandwidth and storage, and main/aux are distinct signals rather
  than necessarily left/right channels.
- **Implicitly downmix or upmix at each edge**: rejected because it loses
  channel identity, hides graph errors, and makes an unchanged template's
  behavior depend on where a node is connected.
- **Only retain special `aux-out`/`out3`/`out4` side taps**: rejected as
  the endpoint because they cannot feed selected outputs into another UGen
  or preserve stereo through intermediate processing. Keep them for
  compatibility while migrating templates.
- **Use a standalone `split`/`channel-split` node for every multi-output
  source**: rejected as the primary graph representation because the
  selector should be an edge property and a selector node adds graph nodes,
  node-cap pressure, and extra copy/work. It remains a possible syntax
  sugar that lowers to the same source-output index.

### Test strategy

- Unit tests for output descriptors, edge lowering, index/shape validation,
  sink routing, mixed mono/stereo paths, and typed diagnostics at every
  capacity boundary.
- Codec exact-byte fixtures and encode/decode/re-encode round trips for
  mono, multi-mono, and stereo-output graphs. Verify that native and
  browser decoders install identical compiled shapes, and that an old-tag
  or corrupt shape byte is rejected with `BadRecord`.
- `.vact` selection tests:
  - `(p :aux)` and `(p 1)` lower to one node with an `output = 1` edge;
  - an unselected multi-output node yields output 0;
  - each failure code listed under "`.vact` selection" occurs at `inst`
    definition;
  - the static checker accepts a `ugen` callee with one keyword or int
    argument.
- Native and browser end-to-end renders that route a dual-output source
  through intermediate mono and stereo effects, exercise pan/balance and
  orbit sends, and verify each output remains independent.
- Callback allocation probes with multi-output nodes and voice-local
  stereo effects active; no callback allocation is permitted.
- Render every existing prelude template with fixed events, controls,
  sample rate, and block sequence. Record the output bit patterns (digests)
  at the pre-change revision `cf2ea37`, before any render-path change, and
  check them into the test suite. After the change:
  - mono templates must be bit-identical at center and off-center pan;
  - unmigrated `aux-out` templates must be bit-identical at center pan,
    and off-center pan must match the balance formula;
  - `sampler` must be bit-identical with a mono and a stereo resource.
- Each of the nine migrated templates renders its main and aux channels,
  each compared independently against the old two-node form, which stays
  valid and is rendered in the same test. Any difference fails the test;
  there is no approved-difference path, because eligibility excludes
  kernels whose state could change.
- Rate/block partition tests for stateful multi-output kernels at the
  supported host rates and multiple callback sizes.

### Owner decisions (resolved 2026-09-29)

- **`.vact` selector spelling:** call the node value with an output name,
  such as `(p :aux)`, with a numeric index such as `(p 1)` as the fallback.
  There is no dot syntax, so the grammar does not change.
- **Stereo `pan` law:** use unity-center balance for stereo and main/aux
  pairs. Center leaves both channels at unity, and moving pan attenuates
  only the opposite side. Mono voices keep equal-power pan.
- **Migration scope:** migrate only kernels whose main and aux are
  simultaneous outputs of one engine state. Each migration needs a
  render-equivalence test. Every other template keeps its duplicate-node
  form, and the old forms stay valid.
- **Legacy graph bytes:** no legacy decoder. Graph payloads are exchanged
  only between native and browser peers built from the same revision, and
  mixed revisions are unsupported under the existing wire policy. A version
  discriminator may still be added for diagnostics.

### Pre-implementation corrections (2026-09-29)

This review checked the design against the code at `cf2ea37`. It applies
the owner decisions above without reopening them. The corrections are:

- **Codec**: the text about preserving a legacy decoder is removed, since
  it contradicted the no-legacy-decoder decision. A new instrument-record
  tag is the permitted discriminator.
- **Migration list**: the list is now explicit. `phase-pair-voice` is not
  eligible, because its `mode` changes modulator-phase state.
  `resonator-voice` is not eligible, because its seeded excitation differs
  per node.
- **`sample-play` shape**: the shape is now fixed per node kind (`:mono`
  default, plus `:stereo`) instead of being derived from the resource. The
  prelude `sampler` picks its bank per event, and deriving the shape from
  the resource would have changed its sound.
- **Voice-local effects**: an effect now follows its subject's shape,
  instead of requiring a stereo subject. Existing mono voice effects stay
  bit-identical.
- **Voice layout and unused outputs**: unconsumed outputs are discarded
  rather than rejected. This matches the existing sink rule and the rule
  that an unselected use means output 0. The voice layout rules are now
  explicit, and legacy `aux-out` voices get balance pan, bit-identical at
  center.

### Implementation status (2026-09-29)

Commit `85a300a` landed the foundation without changing any render:

- the golden render and graph digests, recorded at `cf2ea37`, in
  `src/host/tests/e2e/templates/golden.rs` and `golden_digests.txt`;
- the output-shape contract in `src/dsp/graph/shape.rs`: the declaration
  table, `select_output`, `derive_shapes`, `voice_layout`,
  `assign_slices`, the shape byte, `MAX_AUDIO_BUFFERS` and the discard
  slices. It also added `Edge.output`, the evaluator-only
  `UGenKind::Output` wrapper and `BuildError::TooManyBuffers`;
- the `src/dsp/engine/render.rs` split;
- the paired-output kernel entry points and `sample::play_stereo`.

Commit `b6fa077` landed the VM selection call, checker typing and
lowering (`src/host/tests/e2e/templates/select_output.rs` fixes the source
form `let p {...}` then `{p :main} > * amp > + {{p :aux} > * amp > aux-out}`),
the `b'V'` codec, and the slice-based voice runtime with stereo effects and
balance pan.

Commit `0a15742` landed the nine template migrations (`mod004-30`). Each
one is proven bit-identical in main and aux against its old two-node form
by `src/host/tests/e2e/templates/migrated_pairs.rs`. Only the nine graph
lines in `golden_digests.txt` changed; no render digest changed.
`va-source` reads no per-node seed, so merging the two `filter-voice`
sources satisfies the eligibility rule, and the bitwise test is the proof.

Regression closeout (`mod004-40`) completed on 2026-09-30.
`src/host/tests/e2e/templates/migrated_pairs.rs` covers the migrated
templates, and `src/dsp/tests/dsp/multi_output.rs` (with
`multi_output/cross_path.rs` and `multi_output/invariance.rs`) covers the
native and browser end-to-end, orbit and rate/block items of the test
strategy above. Check (3) is bitwise equal across blocks 64, 256 and 97.
The two points below define the contract the tests enforce:

- **Rate/block contract.** The existing contract has two parts. Events are
  sample-accurate: the engine starts a voice at
  `round((time - block start) * rate)` inside the block. Every stateful
  kernel renders finite, non-silent output at 44 100, 48 000 and 96 000 Hz
  with any `max_block`. The MOD-004 addition is this: a multi-output node
  renders bitwise equal to its legacy two-node form at every rate and
  block, on native and browser. `multi_output.rs` also checks that the
  frames `[2048, 2048 + 8192)` are bitwise equal across blocks 64, 256 and
  97. That check may fail if the legacy two-node form differs across
  blocks in the same way at that rate, because such a difference predates
  MOD-004. It is then recorded as a finding and the engine is not changed.
  If the pair differs from the legacy form, or only the pair differs
  across blocks, the test fails and the cause is a MOD-004 regression.
- **Orbit channel independence.** `OrbitDelay::run` in `src/dsp/bus.rs`
  keeps separate L and R delay lines with per-channel feedback and no
  cross-feed. Voices send `left * delay_send` and `right * delay_send`
  separately (`src/dsp/engine/render.rs`). So with the default bus, a
  stereo voice whose right channel is exactly 0.0 keeps R exactly 0.0
  through the orbit send.

The rules above are the authority for this work; this status note does not
change them.

## Plaits voice-level trigger and low-pass gate layer (PLV-001)

Source of the requirement: `impl-plans/active/modular-audio-handoff.md`
priority 4 and the "voice/LPG parity pending" status of every position in
`impl-plans/active/modular-plaits-engines.md` and its subplans.

### Problem

The 24 Plaits templates run their engine kernels directly. Upstream wraps
every engine in `Voice::Render` (`plaits/dsp/voice.cc`, `voice.h`), which
adds trigger detection, a decay envelope, a vactrol-style low-pass gate
(`plaits/dsp/envelope.h` `LPGEnvelope`, `plaits/dsp/fx/low_pass_gate.h`),
level/accent handling, and a per-engine out/aux post stage with gain,
limiter and clipping. Vactr has none of this. Today each Plaits voice
sounds for its gate time and then the implicit `release` fade ends it
(`src/dsp/voice.rs`, `implicit` in `render`), which is not the upstream
ping or level behavior.

### Goals

- One shared, original-Rust translation of the upstream voice layer that
  every one of the 24 Plaits templates wires in the same way.
- Neutral `.vact` controls with editor metadata, and an explicit mapping
  from Vactr's event model (one event starts one voice, velocity, gate
  length) to the upstream trigger/level conventions.
- Default rendering of every existing template stays bit-identical: all
  golden render digests in
  `src/host/tests/e2e/templates/golden_digests.txt` are unchanged.
- A local mise comparison against the pinned upstream voice layer, and
  truthful manifest, inventory and notice updates.

### Non-goals

- Engine selection, the engine-CV hysteresis quantizer, user-data reload
  and `previous_note_` averaging. In Vactr each template is one fixed
  position, so there is nothing to select.
- Changing any engine kernel, its `velocity` accent mapping, or its
  existing sustain/continuous/clocked control. The engine-level
  trigger-unpatched convention stays on those controls.
- Speech `internal_envelope_amplitude` and the chiptune engine's built-in
  envelope shape. These are engine-specific and stay recorded gaps.
- Any `SourcePort` claim. Engine coverage labels do not change.

### Upstream behavior translated

All per-block constants below assume the upstream 12-sample block at
48 kHz (`kBlockSize`, `kSampleRate` in `plaits/dsp/dsp.h`), so one control
block is 0.25 ms. `d` is the decay control and `h` the colour control,
both in `[0, 1]`.

- **Decay timing** (voice.cc): `short_decay = 0.05 * 2^(-8d)` and
  `decay_tail = 0.005 * 2^(-6d + h) - short_decay`. These are the source
  `SemitonesToRatio` terms (`-96d`, and `-72d + 12h`). Compute them
  analytically with `2^x`; do not use the stmlib pitch-ratio tables.
- **Decay envelope** (`DecayEnvelope`): it is set to 1 on a trigger and
  multiplied by `1 - 2 * short_decay` once per block.
- **Vactrol envelope** (`LPGEnvelope`): state `s` starts at 0. Each block
  it moves toward the input level with coefficient 0.6 when rising, and
  `short_decay + (1 - s^4) * decay_tail` when falling. Its outputs are
  `gain = s`, `frequency = 0.003 + 0.3 s^4 + 0.04 h`, and
  `hf_bleed = (t^2 + (1 - t^2) h) h^2` with `t = 1 - s`. In ping mode a
  trigger sets `ramp_up`. While `ramp_up` is set, `s` grows by `attack`
  per block, clamped at 1, which clears `ramp_up`. The level input is
  `s` while ramping and 0 afterwards. Upstream `attack` is
  `NoteToFrequency(note) * 24`; with Vactr's Hz input this is
  `24 * freq / 48000` per block.
- **Low-pass gate** (`LowPassGate`): the gain is linearly interpolated
  across the block from the previous block's target
  (`ParameterInterpolator`). The input is multiplied by that gain and fed
  to a low-pass `stmlib::Svf` with `FREQUENCY_DIRTY` and `q = 0.4`. The
  dirty tangent is `g = f * (pi + 0.3736 * pi^3 * f^2)`, from
  `OnePole::tan` in `stmlib/dsp/filter.h`. The output is
  `lp + (s - lp) * hf_bleed`.
- **Level** (voice.cc): `compressed = clamp(1.3 l / (0.3 + |l|), 0, 1)`.
  When level is patched, it drives `ProcessLP` directly, and the vactrol
  is not pinged.
- **Bypass rule** (voice.cc): the LPG is bypassed when the engine is
  already enveloped, or when neither trigger nor level is patched.
- **Post stage** (`ChannelPostProcessor` in voice.h): a negative
  registered gain `G` first runs `stmlib::Limiter` (`stmlib/dsp/limiter.h`)
  with pre-gain `-G`, then uses a post gain of 1. A positive `G` is the
  post gain. The LPG gain is multiplied by the post gain, and the result
  is clipped to int16.
- **Registration** (`Voice::Init`): each position has an
  `(already_enveloped, out_gain, aux_gain)` tuple:

| Positions | Already enveloped | Out gain | Aux gain |
|---|---|---|---|
| 0 | no | 1.0 | 1.0 |
| 1, 5 | no | 0.7 | 0.7 |
| 2, 3, 4 | yes | 1.0 | 1.0 |
| 6, 8, 12, 14 | no | 0.8 | 0.8 |
| 7 | when clocked (the engine sets it at runtime) | 0.5 | 0.5 |
| 9, 11 | no | 0.7 | 0.6 |
| 10, 13 | no | 0.6 | 0.6 |
| 15 | no (see divergences) | -0.7 | 0.8 |
| 16 | no | -3.0 | 1.0 |
| 17 | no | -1.0 | -1.0 |
| 18 | no | -2.0 | 1.0 |
| 19, 20 | yes | -1.0 | 0.8 |
| 21, 22, 23 | yes | 0.8 | 0.8 |

### Chosen design

#### Modules and nodes

- `src/dsp/ugen/voice_layer.rs`: pure, allocation-free DSP structs and
  functions. It contains the control clock, decay-timing function,
  decay envelope, vactrol envelope, level compression, low-pass gate SVF
  with gain interpolation, the post limiter, and the per-lane post stage.
  It has no graph or catalog knowledge, so it can be unit-tested and
  implemented in parallel.
- `src/dsp/ugen/vactrol_gate.rs`: the kernels for two new mono UGen kinds
  that wrap `voice_layer.rs`:
  - `vactrol-gate`. Ports, in order: subject `in` (audio), `freq`,
    `velocity`, `lpg-mode`, `lpg-decay`, `lpg-color`, `slot` (constant
    Plaits position 0..23), `lane` (constant 0 = main, 1 = aux), and
    `clocked` (default 0). It has one mono output.
  - `decay-mod`. Ports: `lpg-decay`, `amount` (-1..1, default 0),
    `target` (0 = unit offset, 1 = pitch ratio). It has one mono output.
- Neither kind is a multi-output kernel, so both fall under the MOD-004
  row "every kind not listed: 0 mono". Each gets a new wire tag, a
  catalog entry, a checker domain entry and editor port metadata.
- The registration tuples live in `src/dsp/ported/manifest.rs` as a new
  per-row `voice` field (an enveloped rule of never, always or when
  clocked, plus the signed out and aux gains). `vactrol-gate` reads them
  by `slot`, so the upstream constants exist in exactly one place.

#### Controls and editor metadata

Three new instrument-parameter rows go in `src/dsp/controls.rs`, with
neutral names:

| Control | Domain | Default | Meaning |
|---|---|---|---|
| `lpg-mode` | enum `off`, `ping`, `level` | `off` | Voice-layer mode (below) |
| `lpg-decay` | float 0..1 | 0.5 | Upstream `decay` |
| `lpg-color` | float 0..1 | 0.5 | Upstream `lpg_colour` |

The spelling `color` follows the existing `macro-color` and `peak-color`
controls. All 24 Plaits templates declare the three controls in their
headers with these defaults (`lpg-mode: keyword = :off`). They are listed
in each template's editor metadata (`src/dsp/meta/templates.rs`,
`src/dsp/ugen/catalog/voice_ports.rs`, `src/dsp/build/names/table.rs`)
with label, default and, for `lpg-mode`, choices. The naming test in
`src/dsp/ported/tests.rs` covers the new template controls and the UGen
names `vactrol-gate` and `decay-mod`. The mode is latched once, at the
voice's first control block. Changing `lpg-mode` affects the next event,
not a sounding voice. `lpg-decay` and `lpg-color` are read at every
control block.

#### Event mapping

A Vactr voice start is the trigger's rising edge, at the voice's
sample-accurate start offset. The trigger stays high while the gate is
held (`legato`, or `attack + decay`, or until an open voice is released).
It goes low when the gate closes. The level input is
`compressed(velocity)` while the gate is held and 0 after it closes.
`velocity` is the voice's resolved control: the event value, else the
template or control-table default.

| `lpg-mode` | Upstream state represented | Vactrol | Post stage |
|---|---|---|---|
| `off` (default) | trigger and level unpatched: LPG bypass | none; output is the input bit for bit | none: no gain, limiter or clip |
| `ping` | trigger patched, level unpatched | pinged at voice start; `ProcessPing` each block | registered gain or limiter, then clip to [-1, 1] |
| `level` | trigger and level patched | `ProcessLP(level)` each block | registered gain or limiter, then clip to [-1, 1] |

In `ping` and `level`, a lane is bypassed when its position is already
enveloped. For that lane, `clocked >= 0.5` counts as enveloped at
position 7. A bypassed lane applies only the post stage. For every
position, the engine's own trigger convention stays on its existing
template control (`*-sustain`, `swarm-continuous`, `chip-clocked`). So
the upstream trigger-unpatched engine state is reached by setting that
control, independently of `lpg-mode`. Accent stays the kernels' existing
`velocity` port. The voice layer does not change engine accent.

#### Control clock and host rate

Each `vactrol-gate` and `decay-mod` instance keeps a persistent control
clock of 0.25 ms (`12 * sr / 48000` samples, fractional). It is anchored
at the voice's first sample and carried across callbacks. It uses the
same approach as the FM position-10 interpolation clock. The envelope,
vactrol and decay terms update at each tick, so the per-block
coefficients stay identical at every host rate. Gain interpolation runs
over the integer length of the current tick. The SVF frequency is
converted as `f_host = f * 48000 / sr`, following the existing Plaits
comparison convention that upstream normalized frequencies refer to
nominal 48 kHz. Tick boundaries depend only on voice time, never on the
callback size, so output is partition-invariant.

#### Audio path per lane

For each sample `x` in a non-bypassed `ping` or `level` lane:

1. If `G < 0`, `x` passes through the limiter with pre-gain `-G`, and
   `P = 1`. Otherwise `P = G`.
2. `s = x * (interpolated vactrol gain * P)`.
3. `y = lp + (s - lp) * hf_bleed`, where `lp` is the SVF low-pass of `s`.
4. The output is `y` clamped to [-1, 1].

A bypassed lane outputs `clamp(limited_or_x * P, -1, 1)`. The limiter
peak starts at 0.5 in each voice. Its per-sample slope coefficients
(0.05 attack, 0.00002 release at 48 kHz) are converted to the host rate
by equal time constant. The upstream `-32767` int16 scale and polarity
inversion are omitted, because Vactr works in float at unit full scale.

#### `decay-mod`

`decay-mod` computes the decay envelope from voice start with the same
control clock. It shapes its amount as upstream `ApplyModulations` does:
`a' = 1.05 * a * max(|a| - 0.05, 0.05)`. Target 0 outputs `a' * e`,
which is added to a 0..1 control such as `timbre` or `morph`. Target 1
outputs the ratio `2^(a' * e^2 * 48 / 12)`, which multiplies `freq`. It
is opt-in and not pre-wired into the 24 templates, because wiring it
before a kernel would shift that kernel's node index and seed (see the
next section). The example `examples/voice-layer.vact` shows it
modulating one Plaits kernel's `timbre` together with `vactrol-gate`,
and an e2e test renders that example.

#### Template wiring and seed preservation

Lowering is post-order (`src/dsp/build.rs`), and a kernel's seed is
`voice.seed + node index` (`src/dsp/voice.rs`). The gates must therefore
be appended after every existing node. Each template keeps its current
body and changes in two places. The aux path becomes
`... > * amp > vactrol-gate ... lane: 1 > aux-out`. The whole existing
main expression is then piped into
`> vactrol-gate ... lane: 0` as the new root. In source form, a legacy
duplicate template becomes:

```
A-kernel ... mode: 0 > * amp
	> + {B-kernel ... mode: 1 > * amp > vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: N lane: 1 > aux-out}
	> vactrol-gate freq velocity lpg-mode: lpg-mode lpg-decay: lpg-decay lpg-color: lpg-color slot: N lane: 0
```

A migrated MOD-004 template is changed in the same way, around
`{p :main} > * amp > + {{p :aux} > * amp > ... > aux-out}`. Position 7
also passes `clocked: chip-clocked`. In the lowered `InstDef` the node
order is: every existing node up to and including the aux `*`, then the
new param/const nodes and the aux gate, then `aux-out`, `+` and the root
gate. `vactrol-gate` in `off` mode copies its input exactly. `amp` is a
per-voice constant, so applying the gate after `* amp` is the same
linear stage upstream applies to raw engine output.

**Lowering order is not the seed order.** `Template::build` reorders the
lowered nodes with Kahn's algorithm (`topo_order` in
`src/dsp/ugen/build_helpers.rs`): every source node (no inputs: params
and constants) comes first, in lowering order, and the rest follow in
queue order. A kernel's seed is `voice.seed + compiled index`
(`src/dsp/voice.rs`, both `Kx` sites). The new gate-only source nodes
(`lpg-mode`, `lpg-decay`, `lpg-color`, and `velocity` or `slot`/`lane`
constants when no existing node shares them) therefore push every
non-source node, including every kernel, to a later compiled index.
Appending the gates after the existing nodes keeps lowering indices but
not compiled indices. This is why session 205 changed the render
digests of the eight templates whose kernels read `kx.seed`
(`clock-noise`, `dual-hat`, `dual-kick`, `dual-snare`, `particle`,
`speech`, `string`, `swarm`), while seed-free kernels were unaffected.
Its seed test compared `InstDef` indices and so missed the shift.

**Gate-elided seed order (required).** Each compiled node carries a
*seed ordinal*, and both `Kx` sites use `voice.seed + seed ordinal`
instead of the compiled index. The ordinal is the node's position in
`topo_order` of the *gate-elided graph*, derived from the same raw graph
at build time:

1. Remove every `vactrol-gate` node. Each edge that leaves a gate is
   redirected to the gate's port-0 (subject) source, meaning that edge's
   `from` node and output index, and keeps its place in the edge list.
2. Remove every source node that has at least one out-edge and whose
   out-edges all end at a non-subject port (port 1 or higher) of a
   `vactrol-gate` node. A node that feeds a gate's port 0 is that gate's
   subject and is never removed, even when it is a source such as
   `white-noise`. Edges into a removed gate are dropped.
3. Run `topo_order` on what remains, with the surviving nodes in their
   original relative order. Each surviving node's ordinal is its
   position there. A removed node keeps its compiled index as its
   ordinal; `vactrol-gate` reads no seed.

When the graph has no `vactrol-gate`, the ordinal equals the compiled
index. This is checked directly, so every graph without a gate, including
every non-Plaits template, keeps its seeds by construction. For a
template wired by the edit rule, the gate-elided graph has the same
nodes in the same lowering order, and the same edges in the same order,
as the pre-wiring template. Its ordinals are therefore exactly the
pre-wiring compiled indices. A shared source such as `freq`, a
`velocity` param that a kernel also reads, or a `slot`/`lane` constant
that deduplicates with a kernel's `mode:` constant is not gate-only. It
is kept, and it already existed in the pre-wiring graph at the same
lowering position. The rule applies to any graph, so a user who appends
`vactrol-gate` to their own instrument also keeps its kernel seeds.
`decay-mod` is not elided: it feeds a kernel input, so it is opt-in and
may change that kernel's seed, as stated under `decay-mod`.

The build uses fixed `NODE_CAP`/`MAX_EDGES` arrays, with no allocation
and no new graph traversal in the callback, and one extra bounded
`topo_order` pass per build. Native and browser installs share
`Template::build`, so both get the same ordinals.

The graph digest lines for the 24 templates in `golden_digests.txt`
change, and are updated with this section as the reason. No render line
may change. If the bless output differs in any render line, that is a
defect to fix, not a fixture to update. `migrated_pairs.rs` and
`select_output.rs` fixtures are updated only where they embed the
template text. Their old two-node comparison forms stay valid, and at
default settings the two forms stay bitwise equal, because the
seed-order rule preserves kernel seeds.

#### Voice lifetime

A voice is *layer-shaped* when its template contains at least one
`vactrol-gate` and every gate in it has latched a non-bypassed `ping` or
`level` mode. For a layer-shaped voice:

- the implicit gate fade (`ienv`) is not applied, because the vactrol
  already shapes the tail;
- the voice ends once the gate has closed and every gate reports done
  (vactrol state and interpolated gain both below 1e-4, and no pending
  ping attack).

Every other voice keeps today's rules, including `off` voices and voices
whose lanes are all bypassed for already-enveloped positions. Cut-group
and steal fades still apply to all voices. `src/dsp/ugen/template.rs`
records the gate count. The lifetime check moves into a new
`src/dsp/voice/lifetime.rs` submodule, because `src/dsp/voice.rs`
(918 lines) must stay under 1000 lines.

#### Real-time, capacity and invariance

As delivered in PLV-20, each gate latches its mode, slot, lane, gain and
bypass flags in `NodeState`. It keeps `GATE_STATE_FLOATS` (16) floats in
its fixed `mem_need` region, and `decay-mod` keeps
`DECAY_MOD_STATE_FLOATS` (4). This layout is not changed. Fixed needs are
carved in the first `assign_mem` pass, before any flexible delay line.
So each wired template's `mem_total` grows by exactly
`2 * GATE_STATE_FLOATS` (32 floats), including in `off` mode. This must
be memory only. A kernel's region offset may move if a gate is compiled
before it, and at the test budget (24 000) and the production budget
(`voice_seconds * sr`) no flexible delay line may be clamped by the
extra 32 floats. The unchanged golden render lines are the check.

Existing tests that pin a wired template's `mem_total` must change. Each
is updated to its old figure plus `2 * vactrol_gate::GATE_STATE_FLOATS`,
and its "budget minus one" `MemExceeded` boundary moves with it. These
are the only intended `mem_total` expectation changes. The files, all
under `src/host/tests/e2e/templates/`, are `analog_pair.rs`, `chip.rs`, `chord_pair.rs`, `grain_pair.rs`,
`modal.rs`, `particle.rs`, `shape_pair.rs`, `six_op_original.rs`,
`speech_original.rs`, `string_machine_pair.rs`, `string_voice.rs`,
`table_terrain_pair.rs`, `terrain_pair.rs` and `voice_engines.rs`. In
`voice_engines.rs`, spectrum 96, clock-noise 0, dual-kick 48,
dual-snare 48, dual-hat 32 and swarm 224 each become +32; the
clock-noise message "node state is inline" is reworded to name the gate
state. The plan that owns the template wiring lists all 14 files in its
writePaths and records this reason. Session 205 missed all but the first
of these files because nextest stopped at the first failure.

One structural expectation also changes. In
`src/types/tests/inst/templates.rs`, `templates_realize_at_session_start`
asserts the last lowered node of every template is `mul` or `add`. For the
24 Plaits templates the last lowered node becomes `vactrol-gate` (the root
gate), so that test expects `vactrol-gate` for them. Non-Plaits templates
keep their current expectation. Together with the 14 `mem_total` files,
this is the only intended test-expectation change outside the new
voice-layer tests. The plan that owns the template wiring lists this file
in its writePaths and records this reason.

The callback allocates nothing and runs no graph traversal. The math is
plain `f32` using the same std operations as existing kernels, so native
and wasm32 share one code path. A template adds at most two gate nodes
and five parameter/constant nodes, well within `NODE_CAP`, `MAX_PORTS`,
`MAX_PARAMS` and `MAX_AUDIO_BUFFERS`.

#### Provenance

`verification/upstream_inventory.toml` changes as follows:

- `plaits/dsp/voice.cc` changes to `use = "source"`, and its
  registry-only reason is removed.
- New `source` entries: `plaits/dsp/voice.h`, `plaits/dsp/envelope.h`,
  `plaits/dsp/fx/low_pass_gate.h`, `stmlib/dsp/limiter.h`, and
  `plaits/dsp/engine/engine.h` (for `NoteToFrequency`, the trigger flags
  and the post-processing settings). Add `plaits/dsp/dsp.h` only if Vactr
  names it.
- The existing `stmlib/dsp/filter.h` and `parameter_interpolator.h`
  entries already cover the SVF and interpolator.

Making `voice.cc` a used file brings `plaits/user_data.h` and
`stmlib/system/flash_programming.h` into the audited `#include` closure.
Any non-MIT file reached this way must get an explicit `excluded` entry.
Rerun the audit until it reports 0 errors.

`THIRD_PARTY_NOTICES.md` gets one new section, "Plaits voice-level
trigger and low-pass gate translation". It names these files with their
pinned copyright lines (voice, envelope and engine 2016; low-pass gate
2014; limiter 2015) and the MIT notice. No lookup table, resource or
`resources.cc` data is imported.

#### Comparison probe

Add `verification/plaits_voice_reference.cc`,
`verification/compare_plaits_voice.py`, `examples/plaits_voice_reference.rs`
and the mise task `compare-plaits-voice`. The task follows the
`compare-plaits-*` pattern, requires `VACTR_MI_REFERENCE`, and sets
`CARGO_TERM_QUIET=true`. The C++ side compiles the unmodified pinned
`Voice` in a temporary compilation unit, as the existing probes do. It
may use a probe-local accessor to read `lpg_envelope_`,
`decay_envelope_` and the raw engine buffers. The checkout must stay
clean, and objects and outputs stay under `tmp/`. The probe reports:

1. **Control trajectories** at 48 kHz: per-block vactrol gain,
   frequency, hf_bleed and decay value. It runs `ping` and `level` at
   `d` in {0.2, 0.5, 0.8}, `h` in {0, 0.5, 1}, notes 48 and 69 (using the
   same 47,872.34/48,000 note correction as `compare-plaits-osc`),
   velocity in {1, 0.5}, and a finite gate. Upstream is aligned at its
   detected rising-edge block. That block lags the raised trigger by 4
   blocks (1 ms): `kTriggerDelay` is 5, but `stmlib::DelayLine::Write`
   decrements the write pointer before `Read(5)`, so the read returns
   the sample written four blocks earlier.
2. **Audio path**: upstream `LowPassGate` plus the post stage, against
   Vactr's lane path. Both get the same deterministic harness signal and
   the recorded trajectories. It covers gains 0.8, 0.6, -1 and -2.
3. **Bypass**: an already-enveloped position (19 or 21) in both modes,
   where only the post stage runs.
4. **Host rate** (Vactr-only): trajectory timing at 44.1 and 96 kHz
   against 48 kHz.

Metrics are max-abs and RMS error and correlation, per lane. The voice
layer may be labeled `SourceStage` only when, at 48 kHz, the trajectory
max-abs error is at most 1e-4 and the audio-path correlation is at least
0.999 in every scenario. Otherwise the measured gap is recorded and the
label stays `Pending`.

#### Fidelity labels

Each row of `src/dsp/ported/manifest.rs` gets a `voice_layer:
CoverageState`. It starts at `Pending` and moves to `SourceStage` for all
24 rows only when the probe meets the thresholds above. It is never
`SourcePort`, because the event-model differences below remain.
`plaits_coverage_summary()` also reports the voice-layer count. The
existing engine `coverage` values are unchanged. The
"voice/LPG parity pending" wording in the Plaits plans is replaced with
the measured result.

### Intentional divergences from upstream

1. There is no trigger delay. Upstream delays the trigger by an
   effective 4 blocks (1 ms, `kTriggerDelay = 5` read after write) to
   cover CV lag. Vactr events carry pitch and onset together,
   sample-accurately, so a voice-layer onset sits at the voice's first
   sample and default render timing does not move.
2. The 0.3/0.1 trigger hysteresis is not applied, because the event gate
   is boolean.
3. `off` mode skips the upstream post gain, limiter and clip, even though
   upstream applies them in its unpatched state. This keeps existing
   template levels and digests.
4. The int16 scale, the polarity inversion and the Clip16 one-LSB offset
   are omitted. `ping` and `level` clip at ±1.
5. The control block is anchored at voice start, lasts 0.25 ms at every
   rate, and scales the SVF frequency to the host rate.
6. Level is velocity times gate, not a continuous CV. The
   trigger-unpatched level-patched state and the trigger-patched
   level-patched state behave the same at the LPG, so both use `level`.
7. The internal decay-envelope modulation of note, timbre and morph is
   the opt-in `decay-mod` node, not pre-wired, to keep kernel seeds.
8. Position 15 is treated as never enveloped, because Vactr's procedural
   token mode has no source prosody replay. Position 7 is enveloped when
   `chip-clocked` is at least 0.5, but has no source chiptune envelope.
9. The limiter resets for each voice. Upstream resets only the out-lane
   limiter, and only on an engine change.
10. A layer-shaped voice's tail is governed by the vactrol decay, not by
    the implicit `release` fade.
11. Engine accent is unchanged. Upstream sets the engine accent to the
    compressed level when level is patched, and to 0.8 otherwise. Vactr
    kernels keep their existing `velocity` accent port in every mode.
12. `decay-mod` models only the trigger-patched branch of
    `ApplyModulations`. The upstream trigger-unpatched constant offset
    (`default_internal_modulation = 1` for the note) is not reproduced,
    because a Vactr event always triggers.

### Test strategy

- Unit tests (`voice_layer.rs`): decay timing at d = 0, 0.5 and 1; ping
  attack reaching 1 and clearing `ramp_up`; the level compression curve;
  rise and fall coefficients; SVF DC gain and stability; gain
  interpolation endpoints; limiter peak behavior; bypass and `off`
  identity.
- `off` identity: for all 24 templates, each golden render digest is
  unchanged. For several templates, the `off` render with explicit
  `lpg-mode :off` is bitwise equal to the render with no control.
- Seed order: a graph with no gate gets ordinal == compiled index for
  every node. A graph with a seeded kernel (for example the clock-noise
  pair) followed by the gate pattern of the edit rule, including
  gate-only params and constants, gives that kernel the same ordinal as
  its gate-free copy has compiled index, and renders bitwise equal in
  `off` mode. At the prelude level, for `clock-noise-voice`,
  `dual-snare-voice`, `swarm-voice` and `speech-voice`, the *compiled*
  seed ordinal of every kernel equals the compiled index in a
  test-local copy of the pre-wiring template text. The test must use
  compiled `Template` values, not `InstDef` indices.
- Memory: the 14 pinned `mem_total` expectations listed under
  "Real-time, capacity and invariance" read old + 32 and keep their
  `MemExceeded` boundary.
- Mode behavior: `ping` output rises and decays with `lpg-decay` and
  darkens with a low `lpg-color`. `level` output tracks velocity and
  releases after the gate. Enveloped positions stay unfiltered, with
  only gain applied. Main and aux lanes use their own gains and limiters.
- Lifetime: a `ping` voice outlives its default gate by the vactrol tail
  and ends once it is done. An `off` voice keeps its implicit fade. A cut
  group still chokes.
- Partition and rate: bitwise-equal output across blocks 64, 256 and 97
  at 44.1, 48 and 96 kHz. A native/browser render matches for one
  `ping` template. The alloc probe shows no callback allocation.
- Codec and editor: round trip of both new kinds, template metadata
  listing the three controls with `lpg-mode` choices, and the naming
  test.
- The example `examples/voice-layer.vact` loads and renders finite,
  non-silent audio.

### Deferred, non-blocking items

- Speech `internal_envelope_amplitude`, the speech prosody/speed CV roles,
  and the chiptune built-in envelope shape remain engine-specific gaps in
  the per-engine plans.
- Pre-wiring `decay-mod` into the 24 templates would need a seed-stable
  wiring or a documented digest change. It is left to a separate decision.
- Audible review is outside automated verification and is recorded as
  pending in the handoff.

### Rollout

Delivered in `f5e623b`: the pure `voice_layer.rs` (PLV-10), the manifest
registration tuples and `voice_layer` state (PLV-12), the `vactrol-gate`
and `decay-mod` kinds with their registry, controls and checker entries
(PLV-20), and the probe files with the `compare-plaits-voice` task
(PLV-21). These are accepted dependencies and are not redone.

The remaining work runs as three **serial** plans. Session 205 ran
lifetime and wiring in parallel with split ownership of the shared
compile path, and each stopped on a failure in the other's files. Each
path below has exactly one owner:

1. **PLV-30, voice lifetime and seed order.** It owns
   `src/dsp/ugen/template.rs` (gate count, per-node seed ordinal),
   `src/dsp/ugen/build_helpers.rs` (the gate-elided order helper),
   `src/dsp/voice.rs` (both `Kx` seed sites, the `implicit` predicate
   and `finished` delegation), the new `src/dsp/voice/lifetime.rs`, and
   its own test modules under `src/dsp/tests/dsp/`. It changes no
   template text, so every golden line, graph and render, must stay
   unchanged after it.
2. **PLV-31, template wiring.** It depends on PLV-30. It owns
   `src/prelude/templates.vact`, `src/dsp/meta/templates.rs` and the new
   `src/dsp/meta/templates/plaits.rs`, the 24 Plaits graph lines of
   `golden_digests.txt`, the 14 `mem_total` test files listed above,
   `src/types/tests/inst/templates.rs` (the final-node expectation of the
   24 Plaits templates),
   `src/host/tests/e2e/templates.rs` (one module line), the new
   `src/host/tests/e2e/templates/voice_layer.rs`, and
   `examples/voice-layer.vact`. It owns `migrated_pairs.rs` and
   `select_output.rs` only where they embed template text. Its golden
   bless must show 24 graph changes and zero render changes. Any render
   difference is a blocker to diagnose, not a fixture edit.
3. **PLV-40, evidence closeout.** It depends on PLV-31. It runs
   `compare-plaits-voice` and `audit-upstream` against the pinned
   checkout and sets the `voice_layer` labels in
   `src/dsp/ported/manifest.rs` from the measured thresholds. It updates
   `verification/upstream_inventory.toml` and `THIRD_PARTY_NOTICES.md`,
   records the results in `modular-plaits-engines.md` and its four
   subplans, and in `modular-audio-handoff.md` marks MOD-004 stereo
   edges done under priority 7 and records this priority-4 progress. It
   also adds a status note to this section.

The dispatch manifest for this run lists only these three plans, with
PLV-10, PLV-12, PLV-20 and PLV-21 as accepted dependencies, and runs
them in three single-plan waves. writePaths and sharedPaths are concrete
files only. A full-suite nextest run that fails is re-run with
`--no-fail-fast` so that every failing test is listed before any repair.
Session 205 saw only the first of its 14 `mem_total` failures. Write
evidence logs under `tmp/`. The saved session-205 attempt
(`tmp/plv-wave3-saved/`) is reference only.


### Implementation status (2026-09-30)

PLV-10/12/20/21 landed in commit `f5e623b`; PLV-30/31/40 completed in
session 209. All 24 Plaits templates now include two `vactrol-gate` nodes;
`lpg-mode` defaults to `off`, preserving the default render, while the gate
is opt-in. `golden_digests.txt` records exactly 24 changed graph lines and
zero changed render lines. Each wired template reserves 32 additional memory
floats for the two fixed gate states.

The final `compare-plaits-voice` probe reports
`voice_layer_label_eligible: true`: trajectory, audio path, end-to-end,
bypass and host-rate scenarios A–E meet their thresholds. The manifest label
is `SourceStage` for all 24 wired positions; the layer is not a bit-exact
`SourcePort`. Probe evidence is `tmp/plv/s209/PLV-40/7-compare.json`.

## References

See `design-docs/references/README.md` for the official source and license.
