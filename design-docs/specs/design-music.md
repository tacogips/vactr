# nagamu Music Specification (working draft)

Supporting document to `lang-reference.md`: time, sound, and patterns.
Modeled on TidalCycles/Strudel and Overtone. Music and visuals are
specified separately (author, 2026-09-24); their coupling is deferred
(`notes.md`, "music-visual coupling").

Conventions are those of `lang-reference.md` (`# Decided`, `# => v`).

## 1. Time (Tidal / Overtone model)

```nagm
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

```nagm
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
note [:e2 :g2 :b2] > s :pluck > gain 0.4 > cutoff {range sine 400 2000} > d1

# samples are instruments the host provides; keywords name them and the
# checker knows the set
s [:bd-haus :sn-dub] > d1
n [0 3] > s :bd > d1             # sample index

# effects are controls too (SuperDirt), so they are patternable
s [:bd-haus :sn-dub] > room 0.3 > size 0.8 > delay 0.25 > d1

# chords and scales, on patterns; a chord is [root quality]
n [0 2 4] > scale :c :minor > s :pluck > d1
chord {alt [:c :maj7] [:d :m7] [:g :7]} > voicing > s :piano > d1
arp {chord [:c :m7]} :up > s :piano > d1

# sound parameters available on any pattern:
#   gain pan speed lpf hpf resonance room size delay delaytime
#   delayfeedback crush shape vowel legato attack release sustain
#   begin end cut orbit velocity

# randomness is a signal (section 3): rand, irand, perlin; `choose`
# picks per cycle. Seeded per session, so a set is reproducible.
s {choose :bd-haus :bd-tek} > gain {range rand 0.6 1} > d1

# midi and osc are sinks, like d1
note [:c :e :g] > midi 1         # channel 1
[1 0.5] > osc "/trigger"

# Withdrawn with the Sonic Pi layer: play/sample as "sound now",
# use-synth, with-synth, with-fx blocks, control/slides, rrand, one-in,
# play-pattern-timed. "Now" is `once`; effects and randomness are
# controls and signals.
```

## 3. Patterns (TidalCycles / Strudel model)

```nagm
# A pattern is a function of time to events. Patterns are lazy and
# infinite; nothing sounds until a pattern is bound to an output slot.

# ---- sequences: no strings (author, 2026-09-24) ---------------------
# Tidal's mini-notation lives in strings; nagamu has no string
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
#   bd(3,8)  {euclid :bd 3 8}
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
		> lpf {range 200 2000 sine}

# combining patterns
stack [{s [:bd :bd :bd :bd]} {s [nil :sd]} {s {repeat :hh 8}}]
cat [{s [:bd :sd]} {s {repeat :hh 4}}]     # one per cycle, in turn
fastcat [{s [:bd :sd]} {s {repeat :hh 4}}] # all within one cycle
superimpose {s [:bd :sd]} {p -> fast p 2}
off {note [:c :e :g]} 0.25 {p -> add p 7}
jux {s [:bd :sd]} rev              # apply to one stereo side

# structure and probability
struct {s :bd} [true false true true]
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
chord {alt [:c :maj7] [:d :m7] [:g :7]}
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


## 4. Vocabulary

| Area | Functions |
|------|-----------|
| patterns (Tidal/Strudel names) | `s`/`sound`, `n`, `note`, `gain`, `pan`, `speed`, `lpf`, `hpf`, `room`, `size`, `delay`, `fast`, `slow`, `rev`, `every`, `whenmod`, `sometimes`, `rarely`, `often`, `alt`, `maybe`, `euclid`, `hold`, `repeat`, `choose`, `stack`, `cat`, `fastcat`, `superimpose`, `off`, `jux`, `iter`, `chop`, `ply`, `chunk`, `hurry`, `segment`, `range`, `scale`, `chord`, `voicing`, `arp`, and Tidal's `struct` (rename pending, see review) |
| signals | `sine`, `saw`, `tri`, `square`, `rand`, `irand`, `perlin`, `time`, `beat`, `phase`, `cycle`, `fft`, `amp`, `env`, `hits`, `ctrl` (per-slot event signals), `looks`, `lag`, `map-range` |
| sound (SuperCollider names) | `inst`, `look` (a visual keyed to a sound), `sin-osc`, `saw`, `pulse`, `tri`, `white-noise`, `lpf`, `hpf`, `bpf`, `delay`, `comb`, `env-perc`, `env-adsr`, `line`, `midi`, `osc` |
