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
  is, so it does not validate names; the VM does.
- **Lowering**: lowering memoizes nodes by source pointer. `(p :main)` and
  `(p :aux)` therefore resolve to the same lowered node and emit edges with
  `output = 0` and `output = 1`. Lowering then checks channel shapes (see
  above) and the channel-buffer total (see Capacity). A failure is a typed
  lowering diagnostic on the `inst` definition, which is also definition
  time.

**Migration eligibility.** A duplicate main/aux template migrates only if
all of these hold:

- both instances have identical inputs apart from the selector;
- the selector only chooses which already computed signal is written, so
  no state update depends on it;
- the kernel reads no per-node seed, so merging nodes cannot change a seed;
- main and aux use constant complementary selector values.

The migrated form is `let` binding one node, then
`(p :main) > * amp > + {(p :aux) > * amp > aux-out}`. The voice boundary
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
contributes its output 0. An output that no edge consumes is still computed
(its state advances once per sample) and is then discarded; it is neither
an error nor folded into another channel. The template build fixes one
voice layout:

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

- **Record tag**: the instrument record gets a new tag byte, replacing
  `G_INST = b'I'`. This is the discriminator the owner decision allows. A
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

`src/dsp/engine.rs` is 966 lines. The voice mix and pan code it gains goes
into a split-out submodule first, per the Rust coding standards. Every
touched Rust file stays under 1000 lines.

All storage is allocated or assigned on graph build/install paths. The audio
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

## References

See `design-docs/references/README.md` for the official source and license.
