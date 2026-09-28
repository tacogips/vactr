# vactrol Music Specification (working draft)

Supporting document to `lang-reference.md`: time, sound, and patterns.
Modeled on TidalCycles/Strudel and Overtone. Music and visuals are
specified separately (author, 2026-09-24); their coupling is deferred
(`notes.md`, "music-visual coupling").

Conventions are those of `lang-reference.md` (`# Decided`, `# => v`).

## 1. Time (Tidal / Overtone model)

```vactrol
# Time is data (principle 5). There is no `sleep`, no `sync`, no loop
# that waits: every sound is placed at a position in a cycle by a
# pattern, and the scheduler plays whatever the bound patterns say.

# global tempo: beats and BPM are what musicians read; a cycle is 4
# beats by default; cps is derived
use-bpm 120
use-cycle 4                      # beats per cycle
# Decided (principle 3): beats and BPM are what musicians read; a cycle is a
#   pattern length in beats, default 4; cps is derived

# the only event sources are patterns bound to slots (section 3)
# Decided (author, 2026-09-25): SOUND FIRST, one way only. A pattern chain
# starts with `s <sound>`; rhythm, structure and controls follow in the pipe.
# `n [..] > s :x` and `note [..] > s :x` are errors (`s` takes a sound, not a
# pattern). Structure rule: a single sound (`s :pluck`) has NO structure; the
# first list-valued step after it (`note [..]`, `n [..]`, `euclid`) defines
# the structure, and later list controls are sampled at the existing onsets;
# `s [..]` is structured from the start. MIDI out is an instrument
# (`s {midi 1}`), never a sink: d1..d9 are the only sinks. Sources (step
# lists, `midi-notes`, signals) never come before `s`.
s [:bd-haus :sn-dub] > d1        # a kick at 0 and a snare at 1/2, every cycle
s [:bd-haus nil :sn-dub nil] > d1  # the same, spelled with rests

# variation across cycles is a pattern operation, not a counter
s [:bd-haus :sn-dub]
	> every 4 {p -> fast p 2}      # every fourth cycle, double time
	> whenmod 8 6 rev              # cycles 6 and 7 of every 8, reversed
	> d1
s {alt :bd-haus :bd-tek} > d2    # alternate per cycle

# a one-shot: a pattern played once, now or at a beat offset
s :crash > once
s :crash > once at: 4            # four beats from now
at 4:                            # or schedule any block; `at` is a function
	s :crash > once

# live parameters: a top-level `var` is late-bound, so a pattern that
# names it reads the current value at every event; `upd` is heard at
# the next event with no re-binding (`lang-reference.md`, section 4)
var cutoff 800
s [:bd-haus :sn-dub] > lpf cutoff > d1
upd cutoff 400                   # the next event already uses 400

# ---- external clocks and MIDI (author, 2026-09-24) -------------------
use-clock :internal              # the default
use-clock :midi                  # follow incoming MIDI clock and transport
midi-clock-out true              # send MIDI clock and start/stop
use-clock :link                  # Ableton Link, same mechanism (planned)
# MIDI input as signals and events; controller binding without code is
# the editor's job (architecture.md, Editor Requirements)
s [:bd-haus :sn-dub] > lpf {range {cc 74} 200 2000} > d1   # `cc n` is a 0..1 signal
s :pluck > midi-notes channel: 1 > d1                       # MIDI in: a SOURCE step that gives structure and notes
s {midi 1} > note [:c :e :g] > d1                          # MIDI out is an instrument; d1 is the sink

# stopping
stop :drums                      # stop one slot (`d1` == `slot 1`)
hush                             # silence every slot; the session stays alive

# Withdrawn (author, 2026-09-24: "sleep is far from functional; make it
# declarative"): the Sonic Pi imperative layer -- live-loop, sleep,
# sync, cue, density, tick/look, and `_` as an iteration counter. Their
# jobs are done by patterns: position by sequence, alignment by the
# shared cycle clock, per-cycle change by every/whenmod/alt/iter. A
# consequence: no body ever suspends, so the VM needs no coroutines and
# a dry run of a pattern is complete validation.
```

## 2. Sound (Overtone / Tidal model)

```vactrol
# Instruments (Overtone `definst`): a named signal chain over unit
# generators, the audio counterpart of a Hydra chain. The chain's value
# is the instrument's output.
inst pluck freq: float = 440 amp: float = 0.5 cutoff: float = 2000:
	saw freq
		> lpf cutoff
		> * {env-perc 0.01 0.3}
		> * amp
# unit generators (SuperCollider names, kebab-case): sin-osc saw pulse
#   tri white-noise lpf hpf bpf delay comb env-perc env-adsr line ...
# An instrument is played by a pattern through `s`; pattern controls map
# onto its parameters by name: note/n -> freq (through the scale),
# gain -> amp, any other control by its own name.
s :pluck > note [:e2 :g2 :b2] > gain 0.4 > cutoff {range sine 400 2000} > d1

# samples are instruments the host provides; keywords name them and the
# checker knows the set
s [:bd-haus :sn-dub] > d1
s :bd > n [0 3] > d1             # sample index; the list gives the structure

# ---- sound kits and sound packs (author, 2026-09-25) --------------------
# `s` resolves its keywords against the LATE-BOUND binding `sound-kit`, a
# dict keyword -> sound. The prelude binds `default-sound-kit` (the builtin
# kit, immutable) and `sound-kit` (initially the same value). A sound is an
# `inst` template, one sample, or a list of samples (`n i` picks the i-th).
# A sound pack is a .vact file whose value is such a dict; `load` reads it
# and `put` merges packs (later keys win). Because the session is a child
# scope of the prelude, binding `sound-kit` in the editor overrides the
# builtin kit from the next event on; `default-sound-kit` is never rebound.
let my-pack load ./soundpack/sound-pack.vact      # dict: [bd: ... sd: ...]
let sound-kit put default-sound-kit my-pack       # override :bd, keep the rest
let sound-kit put default-sound-kit [bd: {sample ./bd/909.wav}]   # one key
# Decided (author, 2026-09-25): choosing a kit per call is the named argument
# `kit:` (default: the late-bound `sound-kit`); a keyword names a sound in that
# kit, any other argument is a sound VALUE used as is. No `s-kit`, no options dict.
s [:bd :sd] kit: tr909 > d1                       # (d1 (s [:bd :sd] [:kit tr909]))
let kick sample ./kick.wav
s kick > d1                                       # a bound sound value
# sound-pack.vact (its last expression is the pack):
# [bd: [sample ./bd/1.wav sample ./bd/2.wav]   # `n 1` selects 2.wav
#  sd: sample ./sd.wav
#  pluck: {inst pluck ...}]

# ---- sample slicing (author, 2026-09-24) -------------------------------
# Every way of cutting a sample is a pattern control, so it is patternable
# and its numbers are editable from the waveform in the editor.
s :break > n 3 > d1                         # bank index
s :break > begin 0.25 > end 0.5 > d1        # start and end, 0..1 of the sample
s :break > chop 8 > d1                      # cut each event into 8 grains in order
s :break > striate 8 > d1                   # interleave 8 slices across events
s :break > slice 8 [0 2 4 7] > d1           # 8 equal slices, play these by index
s :break > splice 8 [0 2 4 7] > d1          # slice, and fit each to its step
s :break > slice [0 0.31 0.5 0.8] [2 0] > d1  # manual slice points, 0..1
s :break > loop-at 2 > d1                   # stretch to two cycles
s :break > fit > d1                         # fit the sample to the event length
s :break > cut 1 > d1                       # cut group: a new event stops the last

# effects are controls too (SuperDirt), so they are patternable
s [:bd-haus :sn-dub] > room 0.3 > size 0.8 > delay 0.25 > d1

# chords and scales, on patterns; a chord is [root quality]
s :pluck > n [0 2 4] > scale :c :minor > d1
s :piano > chord {alt [:c :maj7] [:d :m7] [:g :dom7]} > voicing > d1
# Decided (2026-09-25): chord qualities are letter-first keywords (:maj7 :m7 :dom7 :sus4), never :7
# Decided (2026-09-27): the full vocabulary is built in (src/types/chords.rs, one table for
#   the checker and the runtime): triads :maj :m :dim :aug :sus2 :sus4 :five; sixths :six
#   :m6 :six9 :m69; sevenths :maj7 :dom7 :m7 :mmaj7 :m7f5 :dim7 :aug7 :augmaj7 :m7s5
#   :dom7f5 :dom7sus2 :dom7sus4; ninths :add9 :madd9 :maj9 :dom9 :m9 :mmaj9 :dom7f9
#   :dom7s9 :m7f9 :dom9sus4 :dom9s5; elevenths :add11 :maj11 :dom11 :m11 :dom7s11
#   :maj7s11; thirteenths :add13 :maj13 :dom13 :m13; aliases :major :min :minor.
#   Naming: m minor, maj major-seventh family, dom dominant family, s sharp, f flat.
s :piano > chord [:c :m7] > arp :up > d1

# sound parameters available on any pattern:
#   gain pan speed lpf hpf resonance room size delay delaytime
#   delayfeedback crush shape vowel legato attack release sustain
#   begin end cut orbit velocity

# randomness is a signal (section 3): rand, irand, perlin; `choose`
# picks per cycle. Seeded per session, so a set is reproducible.
s {choose :bd-haus :bd-tek} > gain {range rand 0.6 1} > d1

# Decided (author, 2026-09-25): sources and destinations. An instrument (a
# builtin sound, an `inst`, or a MIDI out channel `{midi 1}`) is a DESTINATION
# that receives events; `s` selects it, always first. Sources supply
# structure and values: step lists, `midi-notes` (MIDI in, a structure-giving
# step) and signals (`cc n`, `fft`, `amp`). Neither midi nor osc is a sink.

# ---- self-analysis and loopback (author, 2026-09-25) ------------------------
# Every analyzer takes a SOURCE: a bus (`:master`, `:drums`, a slot `:d1`) or
# the host input `:in`. A bus tap reads the engine's own output INTERNALLY (a
# copy of the bus signal), never through the speakers or the input device, so
# analysing what you play cannot feed back. Monitoring the input device stays
# off unless asked for. The same taps feed the editor's meters, and they are
# how tests measure sound: render offline, then analyse numbers.
fft :master                       # spectrum bands of the master bus, as a signal
amp :d1                           # RMS of slot 1
scope :master 512                 # the last 512 samples, as a list (oscilloscope)
spectrum :master bins: 64         # one FFT frame as a list (spectrum analyzer)
let hit capture :drums 1          # record one cycle of a bus into a sample value (sampler)
let out render 2                  # offline render of the current bindings for two cycles
rms out  /  peak out  /  spectrum out bins: 64   # analysis over a sample value; no audio device needed
# Decided: analyzers are signals (never audible); `capture`/`render` produce
# ordinary sample values, so they compose with `s`, `chop`, `granular`. Offline
# `render` is a native-tier capability (browser: diagnostic, design 12.7).
s {midi 1} > note [:c :e :g] > d1   # MIDI channel 1 as the instrument
[1 0.5] > osc "/trigger"

# Withdrawn with the Sonic Pi layer: play/sample as "sound now",
# use-synth, with-synth, with-fx blocks, control/slides, rrand, one-in,
# play-pattern-timed. "Now" is `once`; effects and randomness are
# controls and signals.
```

## 3. Patterns (TidalCycles / Strudel model)

```vactrol
# A pattern is a function of time to events. Patterns are lazy and
# infinite; nothing sounds until a pattern is bound to an output slot.

# ---- sequences: no strings (author, 2026-09-24) ---------------------
# Tidal's mini-notation lives in strings; vactrol has no string
# notation. A LIST given where a pattern is expected is one cycle of
# steps; a nested list subdivides its step; nil is a rest. Steps are
# keywords (checked against the host's sample or synth set, so a typo
# is a diagnostic, not silence) or numbers. Every mini-notation
# operator is a function usable inside the list:
s [:bd :sd [:hh :hh] [:cp :cp] {alt :bd :sd} {maybe :bd} {euclid :bd 3 8} nil]
# `sound` is an alias of `s`, as in Strudel
#   Tidal:   bd sd hh*2 [cp cp] <bd sd> bd? bd(3,8) ~
#   hh*2     [:hh :hh]           integer repeat is just a nested list
#            {fast :hh 1.5}      non-integer repeat
#   <bd sd>  {alt :bd :sd}       alternate per cycle
#   bd?      {maybe :bd}         50%; {maybe :bd 0.3} for 30%
#   bd(3,8)  {euclid :bd 3 8}      also `rotation: 2`; DAW ring editor
#   bd@3     {hold :bd 3}        weight: three steps long
#   bd!2     {repeat :bd 2}         replicate, or write :bd :bd
#   bd:3     > n 3               sample index, as a control
#   a, b     stack [[..] [..]]
#   a | b    {choose :bd :sd}    random choice per cycle
#   t f t t  [true false true true]
# The same list is an ordinary list everywhere else (`first [:bd :sd]`
# is :bd); it becomes a pattern only where a pattern is expected, by
# the type at that position.
# Decided: no string notation; the list literal plus functions above
#   replaces it. A dedicated literal is withdrawn.
# Decided (principle 1): no `bd: 3` shorthand; the pair is not overloaded;
#   use `> n [3 0]`
# Decided (principle 3): `alt`

# binding a pattern to an output slot (Tidal d1, Strudel $:).
# `d1` is an ordinary function: pattern first, binds it to slot 1 at
# the next cycle boundary, and RETURNS the pattern it bound. Three
# spellings of the same call:
s [:bd :sd] > d1                   # pipe into the sink (principle 2)
d1 {s [:bd :sd]}                   # plain call
d1:                              # trailing block; the block's value is
	s [:bd :sd]                      # its last expression, i.e. the pattern
s [:bd :sd] > d1 > fast 2 > d2     # d1 returns the pattern, so it can go on
# Re-running any of these lines while editing calls d1 again, which IS
# the live replacement; nothing else is needed.
# Decided (author, 2026-09-24): `d1`..`d9` are plain sink functions, and
# a generic named slot exists alongside them: `d1 == slot 1`.
slot :drums:
	s [:bd :sd]

# transforming a pattern: a chain (Strudel: s("bd sd").fast(2).gain(0.8))
# ending in the sink, so the whole thing reads top to bottom
s [[:bd :bd] [:sd :cp]]
	> fast 2
	> gain 0.8
	> room 0.3
	> every 4 rev
	> sometimes {p -> fast p 2}
	> d1
# Decided: pattern first (principle 2)
# Decided (implication 2026-09-24): no auto-currying in a typed language;
#   write `{p -> fast p 2}`

# control patterns: every parameter is itself patternable
d1:
	s [:bd :sd]
		> n [0 1 2 3]
		> gain [1 0.8 0.6]
		> pan sine
		> speed {alt 1 2}
		> lpf {range sine 200 2000}

# combining patterns
stack [{s [:bd :bd :bd :bd]} {s [nil :sd]} {s {repeat :hh 8}}]
cat [{s [:bd :sd]} {s {repeat :hh 4}}]     # one per cycle, in turn
fastcat [{s [:bd :sd]} {s {repeat :hh 4}}] # all within one cycle
superimpose {s [:bd :sd]} {p -> fast p 2}
off {note [:c :e :g]} 0.25 {p -> add p 7}
jux {s [:bd :sd]} rev              # apply to one stereo side

# structure and probability
grid {s :bd} [true false true true]
degrade-by pat 0.3
sometimes-by pat 0.3 {p -> fast p 2}
iter pat 4
chop pat 4
ply pat 2
chunk pat 4 {p -> hurry p 2}
whenmod pat 8 6 {p -> fast p 2}
rarely pat {p -> fast p 2}
often pat rev

# continuous signals (0..1 unless ranged)
sine  saw  tri  square  rand  perlin
irand 8
segment sine 8                   # sample a signal 8 times per cycle
range sine 1 5

# notes, scales, chords (Strudel)
note [:c :e :g :b]
	> s :piano
n [0 2 4 {alt 6 7}]
	> scale :c :minor
	> s :sawtooth
chord {alt [:c :maj7] [:d :m7] [:g :dom7]}
	> voicing
	> s :piano
arp {chord [:c :m7]} :up

# sound parameters available on any pattern:
#   gain pan speed lpf hpf resonance room size delay delaytime
#   delayfeedback crush shape vowel legato attack release sustain
#   begin end cut orbit velocity

# tempo for the pattern engine: cps is derived from `use-bpm` and the
# cycle length in beats (default 4); there is no separate set-cps

# Decided (principle 5): patterns are the ONLY event source; the imperative
#   live-loop is withdrawn (section 1)
# Decided: sequences are patterns of ANY value type, so visuals use them
#   too: `color {alt :red :blue}`, `scale [1 1.5 2]`. No string is involved
#   anywhere.
```


## 4. Synthesis Models (author, 2026-09-24)

Every synthesis model is expressible in code as an `inst` chain over
builtin unit generators, and each also ships as a ready template with
named parameters, so a performer can type `s :analog` and refine later.

```vact
# ---- sampler ----------------------------------------------------------
# sample playback with pitch, start/end, loop, and an envelope; the host
# sample bank is addressed by keyword, sample index by `n`
inst drum: sampler bank: :bd-haus begin: 0 end: 1 loop: false:
	sample-play bank n: n rate: {pitch-to-rate note} begin: begin end: end loop: loop
		> * {env-perc attack release}
s :drum > bank [:bd-haus :sn-dub] > n [0 3] > d1  # or the template directly:
s [:bd-haus :sn-dub] > d1                          # every sample IS a sampler

# ---- analog modeling ----------------------------------------------------
# oscillators with drift and pulse width, unison/detune, ladder or state-
# variable filter, ADSR, noise, sub oscillator
inst analog wave: :saw cutoff: float = 1200 res: float = 0.3 unison: int = 1 detune: float = 0.1:
	vco wave freq unison: unison detune: detune drift: 0.002
		> + {* {sub-osc freq} 0.3}
		> ladder cutoff res
		> * {env-adsr attack decay sustain release}
		> * amp
s :analog > note [:e2 :g2] > cutoff {range sine 400 2000} > d1

# ---- digital: FM, phase distortion, additive ----------------------------
# operators with ratio and index, wired by an algorithm number or an
# explicit graph; phase distortion and additive partials as ugens
inst epiano algorithm: int = 5 ratio: float = 14 index: float = 2:
	fm-op freq ratio: 1 index: {* index {env-perc 0.01 0.8}}
		> fm-mod {fm-op freq ratio: ratio}
		> * {env-adsr 0.01 0.6 0.3 1.2}
inst pd: phase-distortion freq shape: {range sine 0 1} > * {env-perc 0.01 0.4}
inst organ: additive freq partials: [1 0.5 0.33 0.25 0.2] > * amp

# ---- wavetable ----------------------------------------------------------
# a table of frames scanned by `position` (0..1), morphing between
# frames; tables from the host bank or built from a sample
inst wt table: :basic-shapes position: float = 0:
	wavetable table freq position: position
		> svf lowpass cutoff res
		> * {env-adsr 0.005 0.3 0.6 0.5}
s :wt > note [:c3 :e3] > position {range sine 0 1} > d1
```

Templates shipped in the prelude: `sampler`, `analog`, `fm`, `pd`,
`additive`, `wavetable`, `granular` (section 6). Parameters are pattern
controls like any other, so every knob of every model is patternable.

### 4.1 Programmable digital drums (author, 2026-09-27)

The generic FM/phase-distortion/noise/effect graph above is a foundation for
digital drums, but the current seven templates do not provide a dedicated
programmable drum synth. Add an original digital percussion kit with tonal,
noise, metallic, and hat voice families. DaisySP's MIT-licensed synthetic
bass drum, snare drum, and hi-hat modules are compatible starting points for
DSP study or a documented Rust port. If their code is used, retain the
applicable MIT copyright and license notices. Do not import a third-party
wave table, sample, preset, numeric lookup table, or asset-derived trace.
Design Vactrol's tables (if any), defaults, kit layout, and naming
independently. Product text describes Vactrol's digital drum synth.

Each lane is an ordinary named `inst` with an explicit signal graph. The
prelude provides `digital-drum`, `digital-snare`, `digital-metal`, and
`digital-hat` templates. A kit maps ordinary sound keywords to those
instruments. Patterns
trigger the voices and set **every declared sound parameter** by name, with
constant, per-event, signal, or cell-backed values. A parameter absent from an
event retains the instrument default. An unsupported name is a diagnostic,
not a silently ignored control. Names should describe the audio function
rather than expose firmware `PAR_*` numbers.

The shared parameter surface is `freq`, `amp`, `pan`, `velocity`, `wave`,
`coarse`, `fine`, `amp-attack`, `amp-decay`, `amp-slope`, `filter-type`,
`cutoff`, `res`, `filter-drive`, `drive`, `decimation`, `volume-velocity`,
`velocity-depth`, `velocity-target`, `transient-wave`, `transient-freq`,
`transient-level`, `lfo-wave`, `lfo-rate`, `lfo-depth`, `lfo-target`,
`lfo-retrigger`, `lfo-sync`, and `lfo-offset`. The tonal drum and snare
families additionally expose `mod-wave`, `mod-freq`, `mod-level`, `fm-depth`,
`pitch-decay`, `pitch-depth`, `pitch-slope`, and `osc-mix`. The snare adds
`noise-freq` and `noise-mix`; cymbal and hat provide a second modulator via
`mod2-wave`, `mod2-freq`, and `mod2-level`. Family-specific controls such as
retrigger/repeat, open and closed hat decays, and modulation-envelope shape
are named in each template's parameter metadata. The parameter manifest is
the authoritative inventory: every exposed knob has a type, unit, range,
default, enum domain where applicable, editor label, and voice destination.

The instrument inventory also includes audio-output selection and MIDI-note
assignment. Vactrol represents those as pattern bus/slot routing and MIDI
input mapping, not per-voice DSP parameters. Sequencer step volume,
probability, note, Euclidean length/steps, pattern selection, shuffle, tempo,
and automation map to existing Vactrol pattern and clock concepts; these
must remain expressible in `.vact` alongside the kit. This separation makes
the full musical configuration codeable without inventing dead audio knobs.

The control pipeline must resolve an event key against the selected `InstDef`'s
declared parameter names before falling back to the built-in global control
table. Its per-instrument name-to-wire-ID map, type/domain validation, cell
support, and editor metadata must share one parameter manifest. No control
may be silently truncated by the fixed 48-parameter voice template or the
128-cell instrument-default pool. Registration and commit must report capacity
errors before audio is changed. LFO target selection should use typed target
names from the same manifest, and modulation must run in the audio engine at
the declared rate. All voice DSP remains bounded and allocation-free in the
audio callback. Native and browser hosts should render the same kit graph.

Example target usage (the names become live when the kit is implemented):

```text
s :digital-drum > note [:c2 :g2] > fm-depth [0.1 0.7] > d1
s :digital-snare > noise-mix {range sine 0.2 0.8} > d2
s :digital-hat > amp-decay [0.05 0.25] > d3
```

Completion requires a checked inventory of the original design's controls,
real audio differences for every exposed parameter
family, editor visibility, and end-to-end `.vact` rendering of the four
templates. Existing FM/PD/additive/wavetable support alone does not satisfy
this section.

### 4.2 Published modular audio DSP ports (author, 2026-09-27)

The complete source and license inventory, module coverage matrix, audio
interface, and verification rules are in
[`design-mutable-audio.md`](design-mutable-audio.md). The target is every
published Eurorack **audio DSP** engine whose source can be included under
Vactrol's MIT distribution terms. Each engine's meaningful controls must be
available in `.vact`; upstream brand and module names are provenance only,
not product names. This effort supersedes using one external drum machine as
the design basis for the digital percussion kit above.

## 5. Effects (builtin catalog; author, 2026-09-24)

Effects are builtins, usable in three positions: as per-event pattern
controls (`> room 0.3`, SuperDirt style), as unit generators inside an
`inst` chain, and on a **bus**: a declared effect chain that a slot
routes into, plus `master`. Names are plain English, kebab-case; each
takes named parameters with defaults.

```vact
bus :drums:                      # a declared chain; slots route into it
	compressor threshold: -18 ratio: 4 attack: 0.01 release: 0.1
		> tape drive: 0.3
		> plate size: 0.6 mix: 0.2
s [:bd-haus :sn-dub] > bus :drums > d1
master:                          # the main output chain
	multiband-compressor > limiter ceiling: -0.3
```

| Group | Builtins |
|-------|----------|
| dynamics | `compressor`, `expander`, `gate`, `limiter`, `multiband-compressor`, `multiband-expander`, `transient` (attack/sustain), `multiband-transient`, `auto-level` (loudness normalizer), `sag` (power-amp sag) |
| eq and filters | `peq` (n bands), `geq`, `dynamic-eq`, `tilt`, `tone`, `loudness-eq`, `lpf`, `hpf`, `bpf`, `notch`, `comb`, `narrow`, `linear-phase-eq`, `group-delay-eq`, `crossover` |
| delay | `delay`, `ping-pong`, `multitap`, `time-align` |
| reverb | `plate`, `fdn`, `convolution` (impulse responses), `scatter` (random-scattering diffusion), `room` (the SuperDirt control) |
| saturation | `saturate`, `tube`, `clip`, `harmonics` (2nd-5th order), `exciter`, `multiband-saturate`, `sub-synth`, `bandwidth-extend`, `dynamic-saturate` |
| modulation | `chorus`, `flanger`, `phaser`, `tremolo`, `auto-pan`, `auto-filter`, `pitch-shift`, `pitch-shift-hq`, `freq-shift` (and ring modulation), `rotary`, `wow-flutter`, `doppler`, `vibrato` |
| lo-fi | `bitcrush`, `decimate`, `jitter`, `noise-blend`, `hum`, `tape`, `cassette`, `vinyl`, `vinyl-artifacts`, `codec` (kind: `:mp3`, `:gsm`, `:sbc`, `:atrac`, `:g726`), `radio` (kind: `:am`, `:fm`, `:sw`), `tv-audio`, `digital-error`, `dsd-imd` |
| resonator | `modal` (up to n resonators), `horn` |
| spatial | `width` (stereo blend), `balance`, `multiband-balance`, `ms` (mid/side), `crossfeed`, `crosstalk-cancel`, `phase-select-eq`, `spatial-map` (direct/diffuse/residual), `pan`, `matrix` |
| restoration | `declick`, `declip`, `dehum`, `denoise` |
| granular | `granulate` (section 6) |
| utility | `gain`, `mute`, `polarity`, `dc-offset`, `dry-wet`, `section` (bypass a group), `channel-divider`, `fir-crossover` |
| analyzers (signals, for the editor's visual feedback, never audible) | `level`, `spectrum`, `spectrogram`, `note-spectrogram`, `oscilloscope`, `pitch-meter`, `stereo-meter` |

Effects are ordinary functions, so a package (lang-reference.md,
modules) can define new ones as chains of these builtins.

## 6. Granular (builtin; author, 2026-09-24)

One engine, two faces: an instrument template that granulates a sample
or a wavetable, and an effect that granulates a bus or a live input.

```vact
inst cloud source: :pad-loop:    # granular instrument template
	granular source size: 0.08 density: 24 position: {range sine 0 1} spray: 0.05
		pitch: 0 pitch-spray: 0.02 envelope: :hann reverse: 0.1 freeze: false
		> * {env-adsr 0.2 0.5 0.8 1.5}
s :cloud > note [:c3 :g3] > position {range perlin 0 1} > d1

bus :texture:                    # granular effect on a bus
	granulate size: 0.05 density: 40 spray: 0.2 pitch-spray: 0.1 freeze: {alt false true}
s [:vocal] > bus :texture > d1
```

Parameters: `size` (seconds), `density` (grains per second), `position`
(0..1 in the source), `spray` (position randomness), `pitch`,
`pitch-spray`, `envelope` (`:hann`, `:tri`, `:trapezoid`, `:expo`),
`reverse` (probability), `freeze` (hold the read position), `stereo-spray`.
All are pattern controls.

## 7. Vocabulary

| Area | Functions |
|------|-----------|
| patterns (Tidal/Strudel names) | `s`/`sound`, `n`, `note`, `gain`, `pan`, `speed`, `lpf`, `hpf`, `room`, `size`, `delay`, `fast`, `slow`, `rev`, `every`, `whenmod`, `sometimes`, `rarely`, `often`, `alt`, `maybe`, `euclid`, `hold`, `repeat`, `choose`, `stack`, `cat`, `fastcat`, `superimpose`, `off`, `jux`, `iter`, `chop`, `striate`, `slice`, `splice`, `begin`, `end`, `loop-at`, `fit`, `cut`, `ply`, `chunk`, `hurry`, `segment`, `range`, `scale`, `chord`, `voicing`, `arp`, `grid` (Tidal's `struct`, renamed because `struct` declares a struct; Decided 2026-09-25) |
| signals | `sine`, `saw`, `tri`, `square`, `rand`, `irand`, `perlin`, `time`, `beat`, `phase`, `cycle`, `fft`, `amp`, `cc` (MIDI controller, 0..1), `midi-notes` (note input as a pattern), `lag`, `map-range` |
| sound (SuperCollider names) | `inst`, `look` (a visual keyed to a sound), `sin-osc`, `saw`, `pulse`, `tri`, `white-noise`, `lpf`, `hpf`, `bpf`, `delay`, `comb`, `env-perc`, `env-adsr`, `line`, `midi`, `osc` ; synthesis: `sampler`, `analog`, `fm`, `pd`, `additive`, `wavetable`, `granular`, ugens `vco`, `sub-osc`, `ladder`, `svf`, `fm-op`, `fm-mod`, `phase-distortion`, `sample-play`, `env-perc`, `env-adsr`; buses `bus`, `master`; effects: see section 5 |
