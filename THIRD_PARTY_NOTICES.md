# Third-party notices

## Optional local source-comparison probe

`verification/compare_plaits_fm.py` can compile the official Plaits FM engine
and its aggregate resource object from a separate, pinned checkout solely
for local numerical comparison. The original driver and Vactr-side probe
in this repository contain no upstream DSP, generated table, preset, ROM,
audio sample or output recording. The temporary executable is outside this
repository and is deleted after the run. The comparison does not change the
commercial distribution boundary or establish full source parity; the
published FM implementation in Vactr remains an independent adaptation.
`verification/probe_warps_vocoder.py` likewise builds the official Warps
vocoder and filter bank from the separate pinned checkout. It extracts
only the MIT-noticed filter coefficient definitions into a temporary
compilation unit; the aggregate source's oscillator wave tables are
excluded. Its repository driver contains no upstream DSP, coefficients,
oscillator waves, audio samples or output recording. It reports source-only
metrics for direct numerical comparison; the probe alone is not evidence
that Vactr's vocoder matches the source.

## Streams control-algorithm research reference

`stream-envelope`, `stream-vactr`, `stream-follower`, `stream-compressor`,
`stream-filter` and `stream-lorenz`
and the six-function coverage inventory refer to `streams/processor.{h,cc}`,
`envelope.{h,cc}`, `vactr.{h,cc}`, `follower.{h,cc}` and
`compressor.{h,cc}`, `filter_controller.h` and
`lorenz_generator.{h,cc}`
from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These source files carry
Emilie Gillet's MIT notice, with its permission and warranty terms reproduced
below. The firmware computes gain/frequency control voltages for external
analog VCA/VCF circuitry; Vactr's stereo digital gain/low-pass path is an
original adaptation, not a source audio DSP port or analog hardware model.
No generated `lut_env_increments`, `lut_lp_coefficients`, `wav_gompertz`,
source waveform, sample, or aggregate resource is imported. Envelope timing,
photoresponse, cutoff numerics and channel topology differ. The bus uses
independent stereo audio with an optional shared right-channel detector;
source two-pair audio/excite inputs and physical output behavior are not
reproduced. The follower has an independently authored three-band detector,
centroid, cutoff and filter-only alternate. The compressor has authored
threshold/ratio/makeup and hard/soft-knee curves; unlike the source's
five-second sidechain-presence detector, it falls back to local audio as soon
as the right input is quiet. Source coefficient/ratio/square-root/logarithm
tables are not imported. `stream-filter` uses an authored neutral-gain stereo
low-pass with signed excitation-to-cutoff control; source `FilterController`
outputs frequency CV and gain CV=0 instead. `stream-lorenz` uses original
bounded host-rate integration and stereo gain/filter stages informed by the
source's local rate, amount split and second-channel x/z CV swap. It imports
no `lut_lorenz_rate` or other generated data. Both source `Configure` methods
ignore alternate and global/link settings, so the corresponding Vactr
effects do not present inactive controls. All six Streams roles are runnable
adaptations, not source audio ports, physical module emulations or
numerically equivalent firmware-control translations.

## Braids oscillator-shape research reference

`macro-five-voice`, `macro-sub-sync-voice`, `macro-triple-voice`,
`macro-digital-voice`, `macro-filter-voice`, `macro-formant-voice`, `macro-fm-voice`, `macro-physical-voice`, `macro-struck-voice`, `macro-percussion-voice`, `macro-wave-grid-voice`, `macro-wave-line-voice`, `macro-noise-voice`, `macro-cloud-voice` and the
ordered coverage manifest use the published shape
names and control roles in `braids/settings.h`, `macro_oscillator.{h,cc}` and
`analog_oscillator.{h,cc}` and `digital_oscillator.cc` from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those source files carry
Copyright 2012 Emilie Gillet and MIT permission/warranty notices; the MIT
terms are reproduced below. Positions 0–46 are analytic adaptations with
event-local strike/sync mappings. Positions 32–33 translate small
MIT-licensed partial-pitch, amplitude and long/short decay arrays; their
waveform processing remains an adaptation.
Positions 5–6 preserve the roles of a variable base, one/two-octave square
sub selector and gain envelope; positions 7–8 preserve a master/slave
hard-sync pair, pitch offset and blend. Their wave and sync numerics differ
from source fixed-point code.
Positions 9–12 preserve three same-shape oscillator roles (saw, square,
triangle or sine), root pitch, two independent detunes and summed output.
Vactr computes an original continuous ±24-semitone interval curve with a
fine unison zone; it does not import the source's 65-value interval array.
They do not preserve the source's fixed-point oscillator, interpolation,
filter, fold or waveshaper numerics. No `braids/resources.cc`, generated
lookup/waveshaper table, `wav_sine`, upstream wave bank, LXR code, wave or
sample asset is imported. Positions 13–16 add analytic three-sine ring modulation, seven-saw swarm with
a one-pole high-pass, bounded saw comb and sample-held stepped digital toy.
These replace source overdrive, SVF lookup, fixed-point pitch, comb waveshaper,
toy oversampling and FIR decimation; the 40 ms comb ring is sized at install
from host sample rate. Positions 17–20 preserve a phase-reset analytic carrier, saw/triangle
window, pulse and bounded integrator with four distinct source-ordered output
roles. They replace the source sine table, fixed-point pitch/smoothing and
exact integrator arithmetic. Positions 21–24 add analytic VOSIM-style periodic formant pulses, original
continuous vowel trajectories, a pulse-grain formant voice and a 12-partial
harmonic bank. They do not import upstream phoneme frequency/amplitude
arrays, five-filter FOF data, `wav_formant_sine`, `wav_formant_square`,
`lut_bell` or `wav_sine`. The source VOWEL_FOF path actually uses five SVFs;
Vactr uses distinct analytic grains instead. Positions 25–27 add analytic two-operator phase modulation, previous-output
feedback on the modulator, and output-dependent bounded modulator rate.
They replace source `wav_sine`, fixed-point phase arithmetic and blockwise
control interpolation. Positions 28–31 add one event-local, host-rate-sized resonator with distinct
plucked-noise, bowed-friction, reed/breath and flute-edge exciters. This
replaces the source rotated pluck polyphony, bow bridge/neck, reed bore and
flute jet/bore waveguides, body filters and envelope/friction/jet lookup
tables. Vactr keeps one resonator per event, with an exact period down to
20 Hz in supported hosts; below 20 Hz pitch clamps. Source pitch-dependent
pluck oversampling and string update behavior are not reproduced. Positions
32–33 add an 11-partial bell and six-partial struck drum with
analytic sine and a deterministic filtered-noise cross-modulation path.
Source block-based decay is converted using the pinned 48 kHz/24-frame
reference duration; source partial retuning schedule, fixed-point
interpolation and generated filter/wave data are not reproduced. Positions
34–36 add procedural kick pulses into a tuned resonator, a
six-square/clocked-noise cymbal with original filters, and dual-resonance
filtered-noise snare. The cymbal ratios, noise, filter equations and
envelopes are independently designed; source pulse/filter classes and
lookups are not translated. Positions 37–38 use original procedural wave content: a 20-bank/16-step
scan and a bilinear 16×16 wave map. The published `RenderWavetables` and
`RenderWaveMap` read generated `wt_waves`/`wt_map`; their generator
`braids/resources/waveforms.py` reads `data/waves.bin` and `data/map.bin`.
Neither binary, source wave index list, generated resource, nor LXR wave
data is imported. Sonic content, source hysteresis and 2× naive
oversampling differ. Positions 39–40 use an original 64-node computed line with smooth/stepped
blend and a four-voice chord with three original interval families plus
inversion. No `wave_line`, `mini_wave_line`, source chord index array or
generated wave data is imported. Source timbre, interpolation and 2×
oversampling differ. Positions 41–43 use independently implemented
LP/BP/HP noise morphing, two tuned resonances, and cyclic sample-held
quantized noise. The source SVF coefficient lookup, overdrive, fixed-point
quantizer and RNG are not translated. Positions 44–46 use a fixed four-grain
analytic sine cloud, three stochastically excited resonances, and an original
deterministic I/Q pilot/random symbol stream. Grain envelope/rate tables,
source sine, resonator lookup, RNG, constellation arrays and training/data
bytes are not copied. Event-local lifecycle, source parameter curves and
fixed-point arithmetic differ. All 47 positions are runnable adaptations,
not source-equivalent ports; upstream wave-asset
flags 37–40 remain true as provenance facts, not runtime dependencies.

## Clouds granular texture architectural adaptation

`texture-grain` and `src/dsp/effects/texture.rs` adapt the stereo capture,
grain scheduling, position, size, pitch, density, texture/window, spread,
feedback, freeze, trigger and gate roles from
`clouds/dsp/granular_processor.{h,cc}`,
`granular_sample_player.h`, `grain.h`, `audio_buffer.h` and `parameters.h`
in `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those sources and their
used pinned `stmlib` DSP, units, random and parameter-interpolator headers
carry Emilie Gillet's MIT notices; the MIT permission and warranty terms
are reproduced below. Vactr imports no generated grain-size, sine,
window, crossfade or sample-rate-conversion table and no external audio
asset. It computes windows and pitch analytically. Its eight-grain pool,
host-rate capture, compact reverb, feedback law, timing and random
placement differ from the 32 kHz source. This is a playable granular-mode
adaptation, not a bit-exact source port.

## Clouds stretch texture architectural adaptation

`texture-stretch` and `src/dsp/effects/texture_stretch.rs` use the two-window
overlap-add and alignment-search architecture described in the MIT-licensed
`clouds/dsp/wsola_sample_player.h`, `window.h`, `correlator.{h,cc}`,
`granular_processor.{h,cc}`, `audio_buffer.h` and `parameters.h` at
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These source files and
their used pinned `stmlib` DSP and unit helpers carry Emilie Gillet MIT
notices; permission and warranty terms are reproduced below. Vactr
independently implements a bounded float correlation search instead of the
source's bit-sign correlator, two host-rate triangular windows instead of
its exact window scheduling, and an analytic texture low-pass/reverb tail.
For this mode, `density` sets alignment-search diffusion and
`stereo-spread` cross-pans the independently captured channels. Feedback,
freeze, trigger, gate and mix use the Vactr bus convention. No generated
source table, `resources.cc`, recorded sample, or external wave asset is
imported. This is an adaptation, not a bit-exact source port.

## Clouds looping texture architectural adaptation

`texture-loop` and `src/dsp/effects/texture_loop.rs` follow the live-delay,
freeze-loop, tap-synchronization and wrap-crossfade roles of
`clouds/dsp/looping_sample_player.h`, `granular_processor.{h,cc}`,
`fx/pitch_shifter.h` and `parameters.h` in `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those source files and
their used pinned `stmlib` unit/DSP helpers carry Emilie Gillet MIT
notices; permission and warranty terms are reproduced below. The source
`audio_buffer.h` credits Laurent de Soras for its Hermite read; Vactr
uses an independently derived four-point Lagrange polynomial, not that
source interpolator code. Vactr translates the loop player's host-scaled
delay glide, tap sync, cubic-size frozen loop geometry and pitch-dependent
wrap fade. It retains independent host-rate stereo capture, analytic
dual-tap live pitch resampling and compact filter/reverb tails. `density`
adds smooth read-location diffusion; `texture`
changes low-pass filtering; `stereo-spread` widens the independent channels.
`size` also controls the live delay span, so it remains meaningful before
freeze. These mappings and the Vactr gate differ from upstream. The
source's Hermite interpolation, exact pitch-shifter memory, filter,
diffuser, 32 kHz conversion and numerical timing are not reproduced. No
generated table, `resources.cc`, external wave or sample asset is imported.
This is an adaptation, not a bit-exact Clouds looping-delay port.

## Clouds spectral texture architectural adaptation

`texture-spectral` and `src/dsp/effects/texture_spectral.rs` adapt the
stereo STFT overlap-add, spectral history, magnitude warping, pitch shift,
quantization, phase decorrelation and gate-glitch roles in
`clouds/dsp/pvoc/{stft,frame_transformation,phase_vocoder}.{h,cc}` and
`clouds/dsp/granular_processor.{h,cc}` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those files and used
pinned `stmlib` FFT/DSP, units and random helpers carry Emilie Gillet MIT
notices; permission and warranty terms are reproduced below. Vactr uses
its existing 512-point FFT with a new allocation-free inverse, four fixed
history frames per channel, host-rate stereo capture, analytic Hann
windows and bounded overlap-add. `position` scans capture/history, `size`
warps bins, `pitch` shifts bins, `density` controls refresh and phase
decorrelation, `texture` quantizes magnitudes, and gate below 0.5 applies
a sparse spectral glitch; gate's default 1 is the normal path. Feedback,
reverb, stereo spread and mix follow Vactr's bus conventions. These
mappings and the floating-point phases differ from the source's texture
bank, fixed-point phase state, glitch algorithms, 32 kHz conversion and
generated windows. No `resources.cc`, lookup/window table, external wave
or sample asset is imported. This is a runnable adaptation, not a
source-exact port of the spectral mode or complete Clouds parity.

## Clouds quality-selector adaptation

The shared `quality` parameter of `texture-grain`, `texture-stretch`,
`texture-loop` and `texture-spectral` follows the bit roles of MIT-licensed
`clouds/dsp/granular_processor.h::set_quality` at revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`: bit 0 selects mono and
bit 1 selects low fidelity. Copyright Emilie Gillet; the MIT permission and
warranty terms appear below. Vactr's mono path sums to dual mono. Its
low-fidelity path uses original host-rate two-sample hold and signed 8-bit
quantization on capture input and wet output. It does not implement the
published 32-to-16 kHz conversion, generated sample-rate-conversion filter,
or exact source storage formats. No Clouds resource table or source audio
asset is imported; quality remains an adaptation rather than source parity.

## Plaits position 14 chord-layer architectural adaptation

`chord-layer-voice` and `src/dsp/ugen/chord_pair.rs` adapt the four-note
chord bank, five-slot inversion crossfade, divide-down registration,
wave-layer blend, full-chord main and inversion-note auxiliary roles in
`plaits/dsp/engine/chord_engine.{h,cc}`,
`plaits/dsp/chords/chord_bank.{h,cc}`,
`plaits/dsp/oscillator/string_synth_oscillator.h` and
`wavetable_oscillator.h` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files and used
pinned `stmlib` DSP, polyBLEP and interpolator dependencies carry Emilie
Gillet MIT notices. The eleven chord interval sets and five fade points in
`chord_pair.rs` are MIT source-code data included with this notice. The MIT
permission and warranty terms are reproduced below.

The source selects fifteen integrated waves from `wav_integrated_waves`,
generated from unaudited `waves.bin`. Vactr substitutes fifteen
independently written procedural timbres and imports no source wave,
wavetable, aggregate resource, LXR data, or audio asset. The manifest marks
the runnable voice `Adaptation`/`ResourceState::Replacement` and retains
`ResourceFlags::WAVES` for upstream provenance. Its divide-down oscillators
omit source polyBLEP, while registration, procedural spectra, morph
interpolation and note-voice timing differ. This is not a bit-exact or
source-stage-complete port.

## Plaits position 13 procedural wave-grid adaptation

`wave-grid-voice` and `src/dsp/ugen/table_terrain_pair.rs` adapt the
three-axis 8×8 grid scan, mirrored bank navigation, smoothing and 1/32-step
auxiliary role in `plaits/dsp/engine/wavetable_engine.{h,cc}` at
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The engine and its used
`plaits/dsp/oscillator/wavetable_oscillator.h` and pinned `stmlib` DSP,
filter and interpolator dependencies carry Emilie Gillet MIT notices. The
MIT permission and warranty terms are reproduced below.

The source loads `wav_integrated_waves`, generated from `waves.bin`, whose
individual wave origins remain unaudited. Vactr evaluates three original
analytic wave families at 8×8 positions instead. It imports no source wave,
lookup table, `resources.cc`, LXR data, or audio asset. Manifest
`ResourceState::Replacement` records the runnable substitute while
`ResourceFlags::WAVES` retains upstream asset provenance. The source's
custom/user wave-map feature is unavailable until a cleared user-data
contract exists. Spectra, integrated-wave differentiation, smoothing and
bank contents differ. This is an adaptation, not a source-stage-complete or
bit-exact port.

## Plaits position 5 wave-terrain architectural adaptation

`terrain-voice` and `src/dsp/ugen/terrain_pair.rs` adapt the elliptical
quadrature path, five analytic terrain roles, terrain selection and
transformed auxiliary role from
`plaits/dsp/engine2/wave_terrain_engine.{h,cc}` at
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The engine, its used
`plaits/dsp/oscillator/sine_oscillator.h` and
`wavetable_oscillator.h` interfaces, and used pinned `stmlib` DSP and
interpolator dependencies carry Emilie Gillet MIT notices. The MIT
permission and warranty terms are reproduced below.

The source modes 5-7 read `wav_integrated_waves` generated from
`waves.bin`, whose individual wave origins remain unaudited. Vactr
instead supplies three independently written procedural surfaces and
imports no upstream wave, wavetable, sine table, aggregate resource, LXR
data or audio asset. Manifest `ResourceState::Replacement` records the
runnable replacement; `ResourceFlags::WAVES` retains the upstream asset
provenance. The source's user-terrain mode 8 is not available because Vactr
has no cleared user-terrain data contract. Analytic sine, direct path
calculation, mode interpolation and oversampling differ numerically from
source. This is an adaptation, not a source-stage-complete or bit-exact port.

## Plaits position 6 string-machine architectural adaptation

`string-machine-voice` and `src/dsp/ugen/string_machine_pair.rs` adapt the
four divide-down chord voices, stereo filters and ensemble roles from
`plaits/dsp/engine2/string_machine_engine.{h,cc}`,
`plaits/dsp/chords/chord_bank.{h,cc}`,
`plaits/dsp/oscillator/string_synth_oscillator.h`, and
`plaits/dsp/fx/ensemble.h`/`fx_engine.h` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files and the used
pinned `stmlib` DSP, polyBLEP, filter, parameter-interpolator and allocator
headers carry MIT notices. The 11 four-note chord interval sets in
`string_machine_pair.rs` are MIT source-code data included with this notice.
The MIT permission and warranty terms are reproduced below.

Symbol audit: `string_machine_engine.cc` and `ensemble.h` include
`resources.h`, but neither reads `wav_integrated_waves` from `waves.bin`.
`string_synth_oscillator.h` also reads no generated wave symbol.
`ensemble.h` *does* call `SineRaw` in `sine_oscillator.h`, which reads
`lut_sine`. Vactr replaces that LFO with analytic sine and imports no
sine table, integrated waves, aggregate `resources.cc`, LXR data, or audio
asset. Its registration blend, oscillator antialiasing, filter, delay
interpolation, and ensemble topology differ from source. This is an
architectural adaptation, not a source-stage-complete or bit-exact port.

## Plaits position 9 waveshaping architectural adaptation

`shape-voice` and `src/dsp/ugen/shape_pair.rs` adapt the slope-oscillator,
waveshaper, fold and overtone/sine output roles from
`plaits/dsp/engine/waveshaping_engine.{h,cc}` and
`plaits/dsp/oscillator/oscillator.h` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files,
`plaits/dsp/oscillator/sine_oscillator.h`, and used pinned `stmlib`
parameter-interpolator/DSP dependencies carry Emilie Gillet MIT notices.
The MIT permission and warranty terms are reproduced below.

The upstream generated waveshaper curves are labeled as borrowed from
Tides in `plaits/resources/lookup_tables.py`. Vactr does not import those
curves, fold tables, sine table, `resources.cc`, or any waveform/audio asset.
It uses independently written analytic transfer functions instead. Its
fold response, overtone spectrum, integrated-BLEP slope correction, and
block interpolation differ from the source. This is a commercially usable
architectural adaptation, not a source-stage-complete or bit-exact port.

## Plaits position 11 grain oscillator source-stage translation

`grain-pair-voice` and `src/dsp/ugen/grain_pair.rs` translate the two-grainlet
main, Z-oscillator auxiliary and separate high-pass output roles in
`plaits/dsp/engine/grain_engine.{h,cc}` and
`plaits/dsp/oscillator/grainlet_oscillator.h`, `z_oscillator.h` at
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These source files,
`oscillator.h`, `sine_oscillator.h`, and the used `stmlib` DSP,
parameter-interpolator and polyBLEP headers at submodule revision
`e3bd7c9cc00e4364166f9905c0509b6ffd0535ec` carry Emilie Gillet
MIT notices. The MIT permission and warranty terms are reproduced below.

Vactr uses analytic sine in place of the source `lut_sine`. It translates
the source's quadratic two-frame BLEP correction at both grainlet resets and
the Z oscillator's half-cycle discontinuity, and the source dirty-tangent
one-pole high-pass output stage. Per-sample controls replace source block
interpolation and crossing-time control interpolation; numeric parity and
voice/LPG behavior remain unverified. The source VOSIM field is commented
out in `Render` and is not part of this implementation. This is a source-
stage translation, not a validated full port. No aggregate resource,
waveform table, DX7/LXR data, or external audio asset is imported.

## Plaits position 8 virtual-analog architectural adaptation

`analog-pair-voice` and `src/dsp/ugen/analog_pair.rs` implement the
`VA_VARIANT 2` roles in `plaits/dsp/engine/virtual_analog_engine.{h,cc}`:
variable square plus variable saw on main, and the difference between two
synchronized variable-shape oscillators on auxiliary. The source and
`plaits/dsp/oscillator/variable_shape_oscillator.h` and
`variable_saw_oscillator.h` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` carry Emilie Gillet MIT
notices, as do their used pinned `stmlib` DSP, parameter-interpolator and
polyBLEP dependencies. The MIT permission and warranty terms are reproduced
below. The five detuning intervals in `analog_pair.rs` are MIT source-code
data included with this attribution.

Vactr uses original analytic oscillator equations without the source's
polyBLEP edge correction or exact block interpolation. No generated
resource, wavetable, external audio asset, DX7/LXR data or source oscillator
implementation is imported. This is an adaptation, not a bit-exact or
source-stage-complete port, and it is distinct from position 0's filter voice.

## Plaits position 7 chiptune architectural adaptation

`chip-voice` and `src/dsp/ugen/chip_pair.rs` adapt the chord/arpeggio,
five-square main and stepped-triangle bass auxiliary roles from
`plaits/dsp/engine2/chiptune_engine.{h,cc}`,
`plaits/dsp/chords/chord_bank.{h,cc}`,
`plaits/dsp/engine2/arpeggiator.h`,
`plaits/dsp/oscillator/super_square_oscillator.h` and
`nes_triangle_oscillator.h` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files and their
used `stmlib` DSP, polyBLEP, interpolator, quantizer, random and allocator
dependencies carry Emilie Gillet MIT notices. The MIT permission and
warranty terms are reproduced below. The 11 four-note interval sets in
`chip_pair.rs` are MIT source-code data copied with this attribution.

The NES triangle header includes `resources.h` and a wavetable oscillator
header but reads no generated resource symbol. Vactr imports no aggregate
resource, waveform table, DX7/LXR data or audio asset. Its square and
triangle are original analytic oscillators without the source polyBLEP
correction or exact parameter interpolation. `chip-clocked` and `chip-rate`
provide an internal arpeggio clock; the source advances on an external
trigger in a persistent engine, which Vactr's per-event voice lifecycle
does not reproduce. Chord inversion and pattern selection are simplified.
This is an adaptation, not a source-stage or bit-exact port.

## Plaits position 19 three-string architectural adaptation

`string-voice` and `src/dsp/ugen/string_pair.rs` adapt the filtered noise or
dust excitation, three Karplus–Strong delay and stretch lines, nonlinear
bridge/dispersion roles and separate resonant main / filtered-excitation
auxiliary outputs from `plaits/dsp/engine/string_engine.{h,cc}` and
`plaits/dsp/physical_modelling/string_voice.{h,cc}`, `string.{h,cc}` and
`delay_line.h` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files,
`plaits/dsp/noise/dust.h`, the `lut_svf_shift` generator in
`plaits/resources/lookup_tables.py`, and used pinned `stmlib` DSP, filter,
interpolator, units, random and allocator dependencies carry Emilie Gillet
MIT notices. The MIT permission and warranty terms are reproduced below.

Vactr evaluates the MIT shift formula analytically and imports no
`resources.cc`, generated lookup table, waveform or external audio asset.
The source rotates three strings in persistent engine state across triggers;
Vactr events create separate voices, each with three bounded string states
and a seeded active-string selection. Its linear delay interpolation, one-pole
excitation and damping filters, per-voice RNG, host-rate timing, and simplified
stretch/bridge behavior also differ. This is an adaptation, not a source-stage
or bit-exact port.

## Plaits position 20 modal MIT source-stage translation

`modal-voice` and `src/dsp/ugen/modal_pair.rs` translate the filtered strike or
continuous dust excitation, 24-mode resonator and separate resonant main /
filtered-excitation auxiliary architecture from
`plaits/dsp/engine/modal_engine.{h,cc}`,
`plaits/dsp/physical_modelling/modal_voice.{h,cc}` and `resonator.{h,cc}`,
and `plaits/dsp/noise/dust.h` at `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files, the
stiffness generator in `plaits/resources/lookup_tables.py`, and their used
`stmlib` filter, DSP, units, random and cosine-oscillator dependencies at
revision `e3bd7c9cc00e4364166f9905c0509b6ffd0535ec` carry Emilie
Gillet MIT notices. The MIT permission and warranty terms are reproduced
below.

Vactr evaluates the MIT stiffness curve analytically and does not import
the generated `lut_stiffness`, aggregate `resources.cc`, or any audio asset.
It uses exact tangent, an analytic cosine amplitude curve and deterministic
per-voice RNG instead of the source's fast tangent, approximate cosine and
global RNG. Host-rate coefficients replace fixed-48 kHz normalization; the
source's block smoothing of harmonics is not reproduced. This is a source-
stage translation, not a bit-exact source port.

## Plaits position 18 particle architectural adaptation

`particle-voice` and `src/dsp/ugen/particle_pair.rs` are a bounded Rust
adaptation of the six-particle impulse/resonator roles and main/aux output
roles in `plaits/dsp/engine/particle_engine.{h,cc}` and
`plaits/dsp/noise/particle.h` from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The engine and its
`plaits/dsp/fx/diffuser.h`/`fx_engine.h` dependencies carry copyright 2016
Emilie Gillet and MIT notices. The used `stmlib` DSP, filter, units, cosine
oscillator, random and buffer allocator dependencies at submodule revision
`e3bd7c9cc00e4364166f9905c0509b6ffd0535ec` carry Emilie Gillet MIT
notices. The MIT permission and warranty terms are reproduced below.

Vactr uses an independently written two-line analytic allpass diffuser,
TPT particle filters and a per-voice seeded RNG in place of the source's
seven-stage 8192-word granular diffuser, dirty-frequency SVF and global
random stream. It imports no source implementation, generated resources,
waveform table, patch bank, LXR data or external audio assets. Density and
filter coefficients are converted for the host sample rate. This is not a
bit-exact or source-stage-complete port.

## Plaits position 16 swarm MIT translation

`swarm-voice` and `src/dsp/ugen/swarm_pair.rs` translate the eight ranked
grains, spread/density/size controls, random grain scheduling, two-sample
BLEP saw and auxiliary sine in `plaits/dsp/engine/swarm_engine.{h,cc}` from
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files, and the
referenced sine/oscillator headers, are copyright 2016 Emilie Gillet and
MIT licensed. The BLEP polynomial is from pinned MIT `stmlib/dsp/polyblep.h`
(copyright 2017 Emilie Gillet); the source's units and random helpers in
pinned `stmlib` revision `e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`
also carry Emilie Gillet MIT notices. The MIT permission and warranty notice
appears below.

`swarm-spread` names the authored harmonics role without colliding with the
existing `harmonics` effect. `swarm-continuous` explicitly selects the
source unpatched-trigger convention; a Vactr event retriggers burst mode
and its gate ends continuous mode. No aggregate resource, lookup table,
DX7/LXR data or external audio asset is imported. Analytic sine replaces
the grain-envelope sine lookup and recursive fast-sine oscillator, and a
per-voice seeded RNG replaces global `stmlib` randomness. Density follows
host sample rate and block length; pitch control uses the host rate rather
than the source corrected 47,872.34 Hz constant. This is a source-stage
translation, not a bit-exact firmware build.

## Plaits position 23 dual hi-hat MIT translation

`dual-hat-voice` and `src/dsp/ugen/hat_pair/` translate the square-noise
and ring-mod-noise architectures in `plaits/dsp/engine/hi_hat_engine.{h,cc}`
and `plaits/dsp/drums/hi_hat.h` from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files and the
referenced `plaits/dsp/oscillator/oscillator.h` are copyright 2016 Emilie
Gillet and MIT licensed. The six square-noise ratios and three ring-pair
frequency maps are small source data translated under that notice. The
shared coloration SVF, clocked-noise blend, SwingVCA/LinearVCA, one/two-stage
envelopes and output highpass follow the same MIT source. Pinned `stmlib`
revision `e3bd7c9cc00e4364166f9905c0509b6ffd0535ec` supplies MIT
filter/DSP/units/interpolation/random references with Emilie Gillet
copyright 2012–2015. The MIT permission and warranty notice appears below.

Event `velocity` supplies accent. `hat-harmonics` supplies source noisiness
without colliding with the existing `harmonics` effect; `hat-sustain`
selects unpatched-trigger sustain within Vactr's event gate. No upstream
oscillator or lookup table, aggregate resource, DX7/LXR data or external
audio asset is imported. Float phases replace the source uint32 square
phases; analytic square/saw edges replace the source anti-aliased oscillator;
per-voice seeded randomness replaces the global source stream; exact tangent
replaces its filter approximation. Envelope and cutoff timing are converted
to the host rate. This is a source-stage translation, not a bit-exact build.

## Plaits position 22 dual snare-drum MIT translation

`dual-snare-voice` and `src/dsp/ugen/snare_pair/` translate the analog-main
and synthetic-auxiliary stages in `plaits/dsp/engine/snare_drum_engine.{h,cc}`
and `plaits/dsp/drums/{analog_snare_drum,synthetic_snare_drum}.h` from
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. These files are copyright
2016 Emilie Gillet and MIT licensed. The five analog shell-mode ratios
(1, 2, 3.18, 4.16, 5.62), pulse/filter topology, snare-noise envelope,
synthetic oscillator coupling and 40–70 ms hold stage are translated under
that notice. Pinned `stmlib` revision
`e3bd7c9cc00e4364166f9905c0509b6ffd0535ec` provides the MIT SVF,
one-pole, soft-clip, and interpolation reference: `dsp/filter.h` and
`dsp/units.h` (copyright 2014 Emilie Gillet), `dsp/dsp.h` and
`utils/random.h` (copyright 2012 Emilie Gillet), and
`dsp/parameter_interpolator.h` (copyright 2015 Emilie Gillet). The MIT
permission and warranty notice is reproduced below.

Event `velocity` supplies source accent; `snare-harmonics` supplies snappy
while avoiding the existing `harmonics` effect name; `snare-sustain`
explicitly selects the source's unpatched-trigger sustain behavior within
Vactr's event gate. No upstream sine table, aggregate resource, DX7/LXR
data or external audio asset is imported. Analytic sine replaces the source
table, per-voice seeded randomness replaces the global `stmlib` generator,
and exact tangent replaces the source fast filter approximation. Source
48 kHz timing is converted to the host rate. This is a source-stage
translation, not a bit-exact firmware build.

## Plaits position 21 dual bass-drum MIT translation

`dual-kick-voice` and `src/dsp/ugen/dual_kick/` translate the analog-main
and synthetic-auxiliary stages in `plaits/dsp/engine/bass_drum_engine.{h,cc}`,
`plaits/dsp/drums/{analog_bass_drum,synthetic_bass_drum}.h`, and
`plaits/dsp/fx/overdrive.h` from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The bass-drum files are
copyright 2016 Emilie Gillet; the overdrive is copyright 2014 Emilie Gillet.
Translated `stmlib` SVF and soft-clip equations and source filter timing come
from pinned revision `e3bd7c9cc00e4364166f9905c0509b6ffd0535ec`:
`dsp/filter.h` and `dsp/units.h` (copyright 2014 Emilie Gillet), and
`dsp/dsp.h` (copyright 2012 Emilie Gillet). All named sources carry MIT
notices; the permission and warranty notice is reproduced below. Event
`velocity` supplies source accent; `kick-harmonics` avoids the existing
`harmonics` effect name. `kick-sustain` explicitly selects the source's
unpatched-trigger sustain behavior within Vactr's event gate.

No upstream sine lookup, aggregate resource, DX7/LXR data, or external audio
asset is imported. The analytic sine differs numerically from the source LUT.
Vactr uses a per-voice deterministic RNG instead of the global `stmlib`
stream and converts the source's 48 kHz timing/filter coefficients to host
rates. The source's dirty-tangent SVF approximation and inter-block pitch
interpolation and gated sustain-gain smoothing also differ numerically. Thus this is a source-stage translation,
not a bit-exact firmware build.

## Plaits position 0 architectural reference

`filter-voice` and `src/dsp/ugen/va_filter.rs` are independent analytic Rust
adaptations informed by the oscillator/filter roles and four control inputs
in `plaits/dsp/engine2/virtual_analog_vcf_engine.{h,cc}` from
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The authored Plaits
`harmonics` input is called `filter-harmonics` in Vactr because
`harmonics` already names an audio effect. Main and auxiliary paths are
low-pass and high-pass, respectively. Vactr does not copy Plaits'
oscillator, filter, interpolator, code, or resource data. The referenced
`variable_shape_oscillator.h`, `variable_saw_oscillator.h`, and pinned
`stmlib` DSP dependencies carry Emilie Gillet MIT notices; no `stmlib`
implementation or table is imported. This is not a bit-exact source port,
and Plaits voice-level trigger/LPG behavior is not included in this slice.

Copyright 2021 Emilie Gillet (Plaits engine source); copyright 2012 Emilie
Gillet (`stmlib` dependencies). The MIT permission and warranty notice is
reproduced below under Peaks FM drum design reference.

## Plaits position 1 architectural reference

`phase-pair-voice` and `src/dsp/ugen/phase_pair.rs` independently adapt the
control roles and synchronized main/free-running auxiliary architecture of
`plaits/dsp/engine2/phase_distortion_engine.{h,cc}` from
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The engine and referenced
oscillator headers are copyright 2021 Emilie Gillet and MIT licensed;
`plaits/resources/lookup_tables.py` is copyright 2016 Emilie Gillet and MIT
licensed; pinned `stmlib` DSP dependencies are copyright 2012 Emilie Gillet
and MIT licensed. The MIT permission and warranty notice is reproduced below.
Vactr copies none of their code, aggregate `resources.cc`, quantizer table,
DX7 patch data, or other lookup/waveform assets. Its `phase-harmonics` control
replaces the authored harmonics quantizer with an original equal-step ratio
map because `harmonics` is already an effect name. It averages two analytic
substeps per host sample but omits Plaits oscillator antialiasing and exact
ratio, phase-distortion, and downsampling behavior. This is an adaptation,
not a bit-exact source port; voice-level trigger/LPG behavior remains open.

## Plaits positions 2–4 six-operator architectural reference

`six-bank-{a,b,c}-voice` and `src/dsp/ugen/six_op_original.rs` use only the
control and output roles of the MIT-licensed
`plaits/dsp/engine2/six_op_engine.{h,cc}` at Eurorack revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` (copyright 2021 Emilie
Gillet). Each bank has 32 independently authored arithmetic configurations,
six analytic sine operators, and a distinct routing topology. `six-patch`
selects one of those configurations, `timbre` changes brightness, `morph`
changes envelope time, event velocity sets accent, and `six-sustain` holds the
envelope while the event gate is high. Event start resets every operator.
Main and auxiliary deliberately carry the same signal, as in the pinned
source engine. These are Vactr adaptations, not reconstructions of the
published DX7-derived patches: no factory-patch bytes, SYX data, patch
ratios/levels, aggregate `resources.cc`, sine table or LXR data is imported.
Upstream DX7 flags remain provenance facts. Source LFO scrub, staggered
multi-voice scheduling, patch envelope/numeric behavior and user-data loading
are not implemented. The MIT permission and warranty notice is reproduced
below.

## Plaits position 15 speech architectural reference

`speech-voice` and `src/dsp/ugen/speech_original.rs` independently adapt the
naive, SAM-like and LPC-like roles and main/formant versus auxiliary/excitation
output relationship of MIT-licensed `plaits/dsp/engine/speech_engine.{h,cc}`
and its `dsp/speech` dependencies at Eurorack revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` (copyright 2016 Emilie
Gillet). The upper `speech-harmonics` range selects four original Vactr
procedural syllable tokens, `ava`, `omi`, `era`, and `unu`; these are not the
published TI-derived word banks. `timbre` changes formant register, `morph`
changes articulation and envelope time, `velocity` changes accent, and
`speech-sustain` holds the envelope with an open gate. Event start resets
state. Source phoneme/formant arrays, LPC frame/energy/coefficient data,
excitation-pulse table, TI ROM word bytes, aggregate `resources.cc`, and LXR
assets are excluded. Filter, phoneme, prosody, timing and numeric fidelity
differ. This is an Adaptation/Replacement, not a source port. The MIT
permission and warranty notice is reproduced below.

## Plaits position 10 FM architectural reference

`fm-pair-voice` and `src/dsp/ugen/fm_pair.rs` translate the
carrier/modulator, signed-feedback, and carrier/sub output stages in
`plaits/dsp/engine/fm_engine.{h,cc}` from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The engine and its sine
oscillator header are copyright 2016 Emilie Gillet and MIT licensed; the
fourfold downsampler header is copyright 2021 Emilie Gillet and MIT licensed;
used `stmlib` DSP dependencies carry Emilie Gillet's MIT notice. The
FM-specific scalar ratio anchors and four FIR coefficients are translated
from Emilie Gillet's 2016 MIT-noticed
`plaits/resources/lookup_tables.py`; its ratio-expansion algorithm generates
the 130-value quantizer at compile time. The MIT permission and warranty
notice is reproduced below. Vactr imports no generated sine wavetable,
aggregate `resources.cc`, DX7 patch bank, LXR code or wave/audio data. The
authored harmonics ratio control is named `fm-harmonics` because `harmonics`
already names an effect. The kernel now translates the four-substep ratio,
amount, signed-feedback and downsampler stages, with analytic sine and
floating-point phase arithmetic. Upstream table interpolation, exact phase
quantization and voice-level trigger/LPG behavior still differ. It is a
source-stage translation, not a bit-exact or complete Plaits port.

## Plaits position 12 additive MIT source-stage translation

`spectrum-voice` and `src/dsp/ugen/spectrum_pair.rs` translate the
centroid/margin, cubic slope, bump gain, normalization and high-frequency
taper stages of `plaits/dsp/engine/additive_engine.{h,cc}` in
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The source engine,
`plaits/dsp/oscillator/harmonic_oscillator.h`, sine oscillator header and
`stmlib/dsp/cosine_oscillator.h` carry Emilie Gillet MIT notices (2016 for
the Plaits engine; 2012 for `stmlib`). The eight organ harmonic indices
`[1, 2, 3, 4, 6, 8, 10, 12]` are the one-based form of the source's small
MIT-licensed index map; the MIT permission and warranty notice is reproduced
below. Vactr imports no generated sine table,
aggregate resource, DX7 preset or LXR data. The source harmonics bump role
is named `spectrum-bumps` because `harmonics` already names an effect.
Vactr evaluates analytic sine partials rather than the source lookup and
Chebyshev oscillator. Its host-rate per-sample smoothing is derived from the
source 0.001 coefficient per 12-sample 48 kHz block, but it retains raw
smoothed amplitudes rather than the source's normalized-state recurrence and
does not linearly interpolate oscillator amplitude across source blocks.
It is a source-stage adaptation, not bit-exact firmware; voice-level
trigger/LPG behavior remains open.

## Plaits position 17 clocked-noise source-stage translation

`clock-noise-voice` and `src/dsp/ugen/clock_noise_pair.rs` translate the two
clocked-noise sources, quadratic BLEP edge correction, three state-variable
filter paths, source control/gain roles and signed LP/BP/HP main sweep in
`plaits/dsp/engine/noise_engine.{h,cc}` and
`plaits/dsp/noise/clocked_noise.h` from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those Plaits files are
copyright 2016 Emilie Gillet and MIT licensed; their pinned `stmlib` filter,
parameter-interpolator, polyblep and random dependencies carry Emilie
Gillet MIT notices. The MIT permission and warranty notice appears below.
Vactr translates the MIT-noticed BLEP and filter equations but imports no
source implementation files, random implementation, lookup tables, aggregate
resources, DX7 or LXR data.
The authored harmonics role is called `noise-harmonics` because `harmonics`
already names an effect. Vactr uses per-voice seeded randomness and exact
tangent SVF coefficients in place of the source's global random stream and
polynomial coefficient approximation. Its event-local reset, host-rate timing,
control interpolation and bounded output also differ from the original voice.
This is a source-stage translation, not a validated full source port.

## Peaks FM drum design reference

`peak-motion-voice` is an independently written analytic adaptation of the
Envelope, LFO and Tap LFO function roles registered in `peaks/processors.h`
at Eurorack revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`.
The reference implementation in `peaks/modulations/multistage_envelope.{h,cc}`
and `peaks/modulations/lfo.{h,cc}` is copyright Emilie Gillet under the MIT
notice reproduced below. Vactr imports no generated waveform/lookup table,
`digits.bin`, source random sequence or numeric source envelope/LFO algorithm.
Full/half parameter roles are adapted; the independent auxiliary waveform,
tap edge lifecycle and floating-point curves are Vactr behavior, not a
source-equivalent port.

`peak-pulse-voice` is an independently written analytic adaptation of the
Pulse Shaper, Pulse Randomizer and Bouncing Ball roles in
`peaks/pulse_processor/{pulse_shaper,pulse_randomizer}.{h,cc}` and
`peaks/modulations/bouncing_ball.h` at the same pinned revision. Those files
are copyright 2013 Emilie Gillet under the MIT notice reproduced below.
Vactr retains full/half parameter roles but uses host-rate arithmetic,
event-local seeded randomness and one replaceable pulse train rather than
the source 32-entry overlap buffers, generated delay/gravity lookup tables,
control-tick quantization and global random state. Its auxiliary onset/
impact output is an original Vactr extension. No source resource table,
random data or digit binary is imported; no source-equivalent port is claimed.

`number-station-voice` is an original procedural replacement for the tone
and digit-voice roles of `peaks/number_station/number_station.{h,cc}` at the
same pinned revision. The source files are copyright 2013 Emilie Gillet
under the MIT notice reproduced below, but source voice mode reads
`wav_digits` generated from `peaks/data/digits.bin`, whose individual asset
provenance remains unaudited. Vactr imports no recording, `wav_digits`,
source digit offsets, generated sine/fold table or source numeric filter
implementation. Its ten English-like digit gestures use authored procedural
formant targets and deterministic per-voice noise; the dry auxiliary output
and explicit digit selector are Vactr extensions. No source voice-content
or numerical equivalence is claimed.

The `fm-drum` kernel in `src/dsp/ugen/fm_drum.rs` translates the
single-phase sine, FM/auxiliary pitch and amplitude envelopes, previous-
sample pitch feedback, noise mix and overdrive signal stages from
`peaks/drums/fm_drum.h` and `peaks/drums/fm_drum.cc` from
`pichenettes/eurorack` revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`.
The individually MIT-noticed `peaks/resources/lookup_tables.py` and
`waveforms.py` inform continuous analytic envelope/sine/soft-drive
replacements. Vactr contains no Peaks lookup or waveform table, preset
map, or generated resource. It keeps separately authored noise, drive and
pitch-sweep controls, and its floating-point and host-rate timing differ
from the source; numerical source parity is not claimed.

Copyright 2013–2014 Emilie Gillet.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.

## Peaks bass, snare and high-hat design reference

`low-drum`, `wire-drum`, and `metal-hat` use original analytic Rust DSP
informed by the signal families and control roles in
`peaks/drums/bass_drum.{h,cc}`, `snare_drum.{h,cc}`, and
`high_hat.{h,cc}` from `pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. The original sources
are copyright 2013 Emilie Gillet and carry the MIT permission and warranty
notice reproduced above. The original bass/snare files depend on Peaks
`excitation.h` and `svf.h`, `stmlib/utils/dsp.h`, and (for snare)
`stmlib/utils/random.h`; the high-hat uses those filter/excitation and
random utilities too. Vactr imports none of those implementations,
`peaks/resources.*`, lookup tables, sample data, or presets. In the original
high-hat, `Configure` has no authored controls; Vactr's hat frequency,
tone, decay, and metal knobs are extensions. These are architectural
adaptations, not sample-exact copies of the original firmware.

## Three-component percussion design reference

The `fusion-drum` instrument and `src/dsp/ugen/fusion_drum.rs` were informed
by Christopher Arndt's `fbnfm_drumvoice.dsp`:
https://gist.github.com/SpotlightKid/9e3b14d5cda5c763c9db7e5e24cfe352
(last active 2024-11-19). The author declares the Faust source as MIT License.
The Vactr code independently implements its feedback-noise,
filtered-noise, and sine-FM signal flow and exposes their sound parameters.
No Faust-generated source or standard-library implementation is included.
In particular, the source's `fi.resonbp` has a separate
`LicenseRef-STK-4.3` notice in Faust's `filters.lib`; Vactr uses its own
bandpass primitive and does not translate that function. The source also
calls `no.noise`, `en.adsre`, `en.ahdsre`, `os.osc`, `ba.db2linear`, and
`ba.cent2ratio`; none of those library implementations is copied.

## FM percussion research references

`feedback-metal-drum` is an original Vactr coupled-FM voice informed at the
idea level by Erik Larsson's 2000 Chalmers thesis, *Syntes av trum- och
percussionljud med hjälp av frekvensmodulering*:
https://communiteq-eu5.nbg1.your-objectstorage.com/uploads/db8181/original/3X/e/8/e8a6485226730cb92bbeaebd6c8ae4b82359c0a1.pdf
The official Elektron manual provides product-level context:
https://www.elektron.se/wp-content/uploads/2024/09/machinedrum_manual_OS1.63.pdf
Neither reference grants permission to reuse its expression, firmware or
assets. This implementation imports no source text, assembly, diagrams,
equations as expressed, numeric arrays, wavetables, samples, presets or
firmware from them. Its original floating-point signal path and Vactr
controls are not a claim of sound parity, hardware emulation or endorsement.

## Warps modulation design reference

`dual-mod`, `shift-pair` and their separate Rust kernels are analytic
adaptations informed by `warps/dsp/modulator.{h,cc}`, `oscillator.{h,cc}`,
`parameters.h`, `sample_rate_converter.h`,
`sample_rate_conversion_filters.h`, `limiter.h`, `quadrature_transform.h`,
`quadrature_oscillator.h`, `vocoder.{h,cc}` and `filter_bank.{h,cc}` from
`pichenettes/eurorack` revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those sources are
copyright 2014–2015 Emilie Gillet and carry the MIT permission and warranty
notice reproduced above. The individually MIT-noticed
`warps/resources/filter_bank.py`, `warps/resources.cc` filter entries and
`stmlib/dsp/filter.h` are additional sources for the 96 kHz vocoder bank.
`dual-mod` translates the cleared crossfade,
analog and digital ring, XOR and comparator equations into its own
floating-point Rust kernel. Its crossfade is evaluated analytically from
the separately MIT-noticed `warps/resources/lookup_tables.py` generator;
no generated lookup values are imported. The diode approximation follows
the model credited by the source to Julian Parker's DAFx-11 paper.
The public algorithm scale also translates the source XMOD timbre skew,
comparator-to-raw-modulator balance and triangular raw-modulator bridge
into the vocoder range. At every supported host rate, the bridge reaches
the translated 96-kHz filter bank through Vactr's own host-rate FIR
boundary; 96 kHz bypasses that boundary.
The same public algorithm control extends to the source's release and
freeze region (positions 6..8); its release curve and freeze threshold
drive the translated decimated follower at every host rate.
The cleared `SaturatingAmplifier` and `stmlib` soft-clip equations inform
the per-input energy gate and gain path. `carrier-drive` and
`modulator-drive` expose independent channel drives through the existing
shared `drive` control. Host floating-point inputs are bounded to the
source's signed-sample range, and the smoothing follows Vactr's host
sample rate rather than the source block interpolator.
The internal carrier settings 1..3 pair source-role XMOD
sine/triangle/saw with vocoder saw/pulse/noise. The table-free two-frame
BLEP corrections and oscillator filters are translated into fixed Rust
state. Sine is analytic instead of using `lut_sin`; noise uses a Vactr
random stream and one-pole filter instead of the source SVF. The extra
carrier settings 4..6 are original extensions. Oscillator smoothing and
hardware auxiliary scaling are not source-equivalent.
The XMOD path imports the 24-coefficient symmetric halves of the pinned
six-times/48-tap up and down FIRs from the individually MIT-noticed
`sample_rate_conversion_filters.h` (copyright 2015 Emilie Gillet; its
full permission and warranty notice is retained in
`src/dsp/effects/cross_mod_src.rs`).
Vactr's streaming Rust converter uses fixed effect memory; these FIR
coefficients are anti-aliasing filter data, not oscillator wavetables.
The XMOD FIR runs at the host rate, while the vocoder bank runs internally
at 96 kHz. Source blockwise parameter interpolation is not reproduced
at the host control boundary.
The vocoder output limiter translates the fixed peak follower, pre/post
gain and `stmlib/dsp/dsp.h` soft-limit equation from the individually
MIT-noticed `warps/dsp/limiter.h` (copyright 2015 Emilie Gillet). Its
state is fixed and runs at the source's 96 kHz rate after the vocoder
bank and before host-rate output conversion. It does not change the
XMOD or auxiliary paths.
At 96 kHz, Vactr translates the pinned 20-band analysis/synthesis
topology, two-pass `CrossoverSvf`, per-band gains and capped delay
compensation, 3×/36 and 4×/48 FIR conversion, envelope followers,
formant interpolation and 60-frame gain ramps. It imports only the 20
seven-scalar `fb_*` filter rows from MIT `warps/resources.cc` and the
3×/4× FIR coefficient halves from the individually MIT-noticed
`sample_rate_conversion_filters.h`; their full notices are retained in
`cross_mod_vocoder_coeffs.rs` and `cross_mod_vocoder_fir_coeffs.rs`.
These values are filter data, not oscillator wavetables. An additional
60-frame Vactr staging delay and analytic pitch-ratio computation can
prevent bit-exact firmware output. At other host rates, Vactr uses its
own install-time generated, phase-interpolated Blackman-windowed sinc
filters to and from the same 96-kHz bank. These rate-dependent filters
add latency and are Vactr-generated anti-alias data, not upstream
oscillator wavetables or source converter coefficients. The previous
one-pole-pair fallback has been removed. The fold equation remains authored.
No upstream oscillator source file, oscillator lookup table, wave asset,
or other `warps/resources.*` data is imported. The auxiliary
output and seven-mode vocabulary are architectural references; `dual-mod`
is not bit-exact Warps firmware. `shift-pair` adds the hidden
frequency-shifter role with an original 127-tap windowed Hilbert FIR
generated into fixed install-time memory, analytic internal I/Q carriers,
stereo main/aux sidebands and
bounded feedback. It imports no source quadrature poles, sine/crossfade
lookups or generated oscillator waveforms. Its 63-sample latency, shift
curve, low-frequency rejection and feedback numerics differ from source;
this is an adaptation, not a source-equivalent port.

## Tides first-generation function architectural reference

`tidal-voice` is an original analytic AD, looping and AR function adaptation
informed by Mutable Instruments `tides/generator.{h,cc}` at Eurorack revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those sources carry
copyright 2013 Emilie Gillet and the MIT permission and warranty notice
reproduced above. Vactr uses authored curves, host-rate smoothing and a
small analytic soft fold, not the source fixed-point ramp, discrete clock
ratio list, high-range audio/medium-low control-rate split, 16-sample block
delay, antialias/filter stages or generated wavefolder tables. The source's
`WAVETABLE_HACK` is
disabled in the default generator build; Vactr does not use optional
`waves.bin`, `resources.cc` or any source wavetable. `tide-output` multiplexes
unipolar, bipolar, end-of-attack and end-of-release samples onto the main
output; the template's auxiliary defaults to bipolar. This differs from
the source's simultaneous two samples plus flag bits and is not a source
port. `examples/quad-stems.vact` can emit those four roles together on an
opt-in four-output host. Vactr now translates the EOA level and EOR
idle/loop-hold roles with host-rate scaling, but does not reproduce source
fixed-point edges, exact block timing or numerical flag duration; outputs
3/4 bypass bus/master FX.
Tides2 is not implemented by this template.

`tides2-voice` separately adapts the 2017 Emilie Gillet MIT DSP architecture
in `tides2/{ramp_generator,ramp_shaper,poly_slope_generator}.h` at the same
pinned Eurorack revision. It runs original analytic AD/loop/AR ramps, four
output-mode roles (gates, amplitude, phase, frequency) and two rate ranges.
It does not import the source ratio arrays, generated shape/fold tables,
polyBLEP code or aggregate resources. Its frequency-mode ratios are original
continuous functions of shift, and its `poly-clock` is an edge reset rather
than the source's external ramp/ratio synchronization. The four authored
lanes are each codeable with `poly-main-channel` and `poly-aux-channel`.
The separate `examples/quad-stems.vact` opt-in template emits four lanes on
a four-output host; channels 3/4 are direct stems that bypass stereo bus and
master FX. Source output voltages, waveshaping and timing are not reproduced.

## Elements internal instrument architectural reference

`elements-voice` is an original procedural adaptation of the bow, blow,
strike and three resonator roles in Mutable Instruments
`elements/dsp/{patch,part,voice,exciter,resonator}.{h,cc}` at Eurorack
revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those DSP files
carry copyright 2014 Emilie Gillet and the MIT permission and warranty
notice reproduced above. Vactr uses authored noise, oscillators, modal
and four-string recurrences, a short feedback space and analytic controls;
it imports no source sample recording, generated array, chord map or LUT.
The source's `elements/resources/samples.py` is GPL-3.0-or-later and bundles
WAV recordings with unverified individual rights; neither the generator nor
those WAV bytes are used. The source five-string topology, its exact DSP,
separate external blow/strike source-DSP behavior remains pending. The
opt-in `elements-external-voice` now routes validated host left/right to
named blow/strike instrument ports using the existing authored kernel.
The separate `elements-bank` bus/master effect
maps its left input to blow and right input to strike excitation and outputs
main and auxiliary channels on left/right. It uses distinct authored filters,
four-string/modal states and short feedback space, not source numerical DSP.
It imports no GPL generator, bundled WAV, lookup or aggregate resource.
Both paths now provide an authored alternate dual-FM/spatial voice selected
by `el-alternate`. This replaces, rather than translates, the source
`elements/dsp/ominous_voice.{h,cc}` eightfold oversampling, 101-tap FIR,
lookup oscillators and multimode filters with fourfold analytic oscillators,
a bounded one-pole antialias stage and a short spatial echo. `el-space`
accepts 0..2 and freezes echo writes from 1.75 upward. Source spatial,
filter and freeze numerics are not reproduced.

## Rings Part architectural reference

`rings-voice` and `src/dsp/ugen/rings_part.rs` are original bounded
adaptations of the six model roles in Mutable Instruments
`rings/dsp/part.{h,cc}`, `resonator`, `string`, and `fm_voice`, at Eurorack
revision `08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4`. Those source files
are copyright 2015 Emilie Gillet and carry the MIT permission and warranty
notice reproduced above. Vactr uses authored interval arithmetic, analytic
oscillators, fixed comb/resonator state, seeded excitation, and a short
feedback diffuser. It does not import Rings' source chord arrays, generated
lookup tables, wave/sample content, source noise sequences, or aggregate
resources. Source shared voice allocation and numerical/model fidelity remain
outstanding. The voice is an adaptation, not a source-equivalent port.
The opt-in `rings-external-voice` averages validated host L/R into a named
event-local mono excitation port. This routing and its numerical response
are Vactr adaptations; the source's shared Part state remains unmatched.

`resonant-bank` is a separate original stereo bus/master adaptation of the
same six `Part::Process` model and external-excitation roles. It sums stereo
input to a mono exciter, emits main and auxiliary outputs on left and right,
and uses authored modal, four-comb-string, FM and short-feedback formulas.
All 17 bus controls are codeable, including internal/external excitation,
strum, gate and dry/wet mix. Its external input, state topology, filter and
polyphony numerics differ from the source. No Rings lookup, chord, waveform
or binary resource is imported.

`string-choir-voice` separately adapts architectural roles from
`rings/dsp/string_synth_part.{h,cc}`, `string_synth_voice.h`,
`string_synth_oscillator.h`, `string_synth_envelope.h`, and the
`rings/dsp/fx/{chorus,ensemble,reverb}.h` files at the same revision.
These files carry Emilie Gillet's MIT notice, with the permission and
warranty terms reproduced above. Vactr uses original chord and registration
formulas, analytic oscillators, four rate-sized comb lines and distinct
formant, chorus, ensemble, and short feedback-delay FX formulas. It imports
no source chord or registration array, `lut_sine`, generated resource,
oscillator/FX implementation, or audio asset. The six source FX selections
are represented but their numerical effect and source cross-channel paths
differ. The source has twelve oscillators and persistent rotating groups;
Vactr has four event-local voices and independent main/aux calculations.
External audio input remains pending. This is not a source-equivalent port.

## Stages-inspired analytic segment and oscillator adaptation

`stage-voice` uses the published Stages `segment_generator.{h,cc}` and
`variable_shape_oscillator.{h,cc}` at Eurorack revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` as architectural references.
These sources carry Emilie Gillet's MIT notice and permission/warranty terms
reproduced above. Vactr implements original analytic curves, deterministic
event-local control state and host-rate timing. It imports no Stages waveform,
frequency, portamento or delay lookup table, generated resource, or binary
audio data. Its original CV delay uses a host-rate-sized float ring and
decimated writes for longer times, in place of the source 576-word quantized
delay and interpolation. Both source Delay dispatch cells map to this same
role; gate/loop are source-ignored there. The 31.25 kHz source scheduler,
cross-module links and slave remain unavailable. The separate
`stage-chain-voice` implements an original six-segment host-rate chain;
its ramp targets, step search, sentinel transition and oscillator numerics
differ from the source `ProcessMultiSegment` implementation.
The opt-in `stage-linked-voice` is a separate original analytic 1–36-record
event-local adaptation. Its immutable authored list replaces source module
serial state. A source-shaped director-plus-STEP list now selects an original
seven-mode Sequencer adaptation, exposed by `stage-sequencer-voice`: gate edges
clock bounded marked steps, with authored deterministic random traversal,
host-rate slew and normalized-position auxiliary output. A constant director
reset takes effect at event onset rather than holding the source's inhibit;
the source phase output is zero rather than Vactr's step position. Source
slave behavior, per-step live control, hysteresis, output quantization and
exact transition/timing parity are not claimed. No source waveform or table
data is included.

## Frames poly-LFO and keyframer control adaptations

`frame-lfo-voice` uses the published `frames/poly_lfo.{h,cc}`,
`frames/keyframer.{h,cc}` and `frames/frames.cc` at Eurorack revision
`08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4` as MIT-licensed
architectural references (Emilie Gillet, 2013; permission and warranty terms
reproduced above). Vactr uses original analytic oscillators, phase spread,
detune and neighbor coupling. It imports no `wt_lfo_waveforms`, increment,
response, VCA-linearization, palette/rainbow table or generated resource.
`frame-keyframe-voice` additionally provides an immutable, strictly ordered
64-frame, four-lane digital interpolation payload in `.vact`, six original
analytic easing curves and per-lane digital response curves. It does not
reproduce the source DAC/VCA response calibration or numerical interpolation.
The default templates select two lanes; `examples/quad-stems.vact` provides
four simultaneous poly-LFO or keyframe digital lanes on an opt-in
four-output host, with 3/4 as direct stems. The frame list is authored in `.vact` but is not yet editable
through scalar sliders.
Frames' analog mixer/VCA audio path is outside firmware DSP and is not ported.
