# Bass Voices Design

Dedicated bass synthesis for techno: six prelude bass templates backed by one
original kernel UGen, a named-patch preset library, four techno example tracks,
and the tests that render them offline. Linked from
[`design-music.md` section 4.3](design-music.md#43-dedicated-bass-voices-author-2026-09-30).

Status: accepted design for the wf/bass workflow (2026-09-30). Unresolved
decisions are recorded in
[`../user-qa/pending-bass-questions.md`](../user-qa/pending-bass-questions.md);
their recommendations are followed by default.

## Overview

The prelude (`src/prelude/templates.vact`) has 67 `inst` templates. None of them
is built for bass. The nearest ones (`analog`, `fm`, `macro-sub-sync-voice`,
`fm-pair-voice`, `analog-pair-voice`) cannot make the core techno bass
archetypes well, for four reasons checked against the code:

| Gap | Evidence |
|-----|----------|
| No resonant diode-ladder low-pass, and no filter cutoff that moves every sample | `ladder` in `src/dsp/ugen/filter.rs` is four one-pole stages with a one-sample feedback delay. It reads `cutoff`/`res` once per block (`ins[1].first()`), has no drive input and its self-oscillation is marginal. `svf`, `lpf` and the other biquads also set coefficients once per block. |
| No feedback FM | `fm-op` has no feedback port and graphs reject cycles (`ShapeError::Cycle`). Operator self-feedback exists only inside `fm-pair` and `feedback-metal-core`. |
| No note length for audio voices | `legato` is a scheduler-route control that `src/sched/commit.rs` drops for audio events. The voice gate is `max(legato, attack + decay)` seconds (`src/dsp/voice.rs`), so it has nothing to do with the step length. |
| No glide, slide or accent | Searching the codebase finds no glide, portamento or slide kernel. Each event starts a new voice with state zeroed (`Voice::start`), and a kernel cannot read the previous voice. |

Tempo sync exists: the hidden `cps` and `onset-time` controls. `commit.rs`
fills them for kernels listed in `wants_tempo_anchor`, and
`digital_drum/chain.rs` uses them for `lfo-sync`. The bass design reuses that
mechanism unchanged.

## Scope

In scope:

- One new kernel UGen, `bass-core` (module `src/dsp/ugen/bass_voice`), with six
  models.
- Six new prelude templates: `analog-bass`, `acid-bass`, `fm-bass`,
  `wobble-bass`, `sub-bass`, `reese-bass`. They are registered and
  editor-visible like the existing templates.
- `examples/bass-presets.vact` with at least three named patches per template.
- Four techno example tracks: acid, rolling/rumble, minimal/offbeat, and wobble.
- Kernel unit tests, e2e template tests, new golden digest lines, and offline
  render tests that write WAVs to `tmp/`.
- README update.

Out of scope, and not changed:

- The scheduler.
- Note-length handling for existing templates.
- Any existing template's output.
- The canvas editor.
- The wf/syntax-fmt areas: `src/fmt`, `src/complete`, `src/lsp`,
  `src/host/wasm`, `editor/`, `tree-sitter-vact/`.

A "rumble" bass is made in the rumble example from `sub-bass`/`reese-bass`
routed through existing effects (section "Presets and examples"). It is not a
seventh template.

## License boundary

All bass DSP is original Rust written from the published papers and books in
[References](#references) and from general signal-processing knowledge.

- No source code of any TB-303, ladder, diode-ladder, FM or wavefolder
  emulation is consulted, translated or adapted. This includes GPL/LGPL
  projects, code of unknown license, and book or app-note example code.
- The license rule in
  [`design-mutable-audio.md`](design-mutable-audio.md#scope-and-license-boundary)
  still applies: an MIT source may be adapted only with a
  `THIRD_PARTY_NOTICES.md` entry.
- This design adapts no MIT source, so `THIRD_PARTY_NOTICES.md` is expected to
  stay unchanged. If a plan later adapts MIT code, it must add the notice in
  the same change.
- Template names, control names and comments are generic ("acid", "diode
  ladder", "accent", "slide"). No product trademarks are used as identifiers.

## Research notes: bass archetypes

| Archetype | What defines it | Main references |
|-----------|-----------------|-----------------|
| Analog-modeling bass | One saw or pulse oscillator plus a square sub one octave down. A 24 dB/oct transistor ladder low-pass with a fast filter-envelope pluck, mild input drive, and a short amp release. The "Moog/SH" family. | Huovilainen 2004; Valimaki and Huovilainen 2006; D'Angelo and Valimaki 2013; Zavalishin (ladder chapter) |
| Acid bass | One saw or square into a 4-pole diode ladder. The stages are coupled, not buffered, which gives a softer, less steep slope that is often described as about 18 dB/oct. Strong resonance. Per-step accent raises the filter envelope and loudness and shortens the decay. Per-step slide glides the pitch through an RC lag without retriggering the envelopes. | Stinchcombe 2008 (diode ladder analysis); Zavalishin (diode ladder); Pirkle (VA diode ladder); TB-303 owner's manual (accent/slide programming model) |
| Digital/FM bass | A DX-style 2-operator stack whose modulator feeds back on itself. It has a decaying index envelope for the pluck, and optional wavefolding and bit-depth reduction for "digital grit". | Chowning 1973; Tomisawa 1981 (operator feedback); Esqueda et al. 2017 (wavefolding); Parker et al. 2016 and Bilbao et al. 2017 (ADAA) |
| Wobble bass | Detuned saws plus a sub, with the low-pass cutoff swept by a tempo-synced LFO. The usual divisions are 1/4, 1/8 and triplets. The LFO phase restarts per note by default, or follows the transport. | Tempo arithmetic (cycles per second times divisions); Puckette 2007 (phasor/LFO) |
| Sub bass | A sine, or a triangle for a little more edge, below about 120 Hz. Light saturation makes it audible on small speakers, and an optional short attack click adds definition. Little or no resonance. | Valimaki et al. 2012 and Esqueda et al. 2016 (band-limited triangle via polyBLAMP) |
| Reese bass | Two saws detuned by a fraction of a semitone, so they beat slowly. Plus a sub, a 24 dB low-pass with slow envelope movement, and drive. Named after the bass sound of Kevin Saunderson's Reese project. | Valimaki and Huovilainen 2007 (polyBLEP saws); ladder references above |

Oscillator anti-aliasing: saw, pulse and square use polyBLEP
(Valimaki/Huovilainen 2007; Valimaki/Pekonen/Nam 2012). Triangle uses polyBLAMP
(Esqueda/Valimaki/Bilbao 2016). Sine is analytic. Bass fundamentals are low,
so these corrections are enough without oversampling.

## Architecture decision: one kernel with six models

**Decision.** Add one kernel UGen, `bass-core`. It has a per-template literal
`mode` port (0..5) that selects the oscillator section and the filter. All six
models share one post chain: filter, drive, amp envelope, gate length, accent,
slide and LFO.

This mirrors existing families that fix a mode or shape literal in the
template body: the pair templates' `mode: 0/1`, `analog-percussion ... mode: N`,
and the `macro-*-voice` shapes.

Why not six kernels:

- Each kernel needs a `Node` variant, a codec tag, a catalog port list, a
  names-table entry, a natives entry and meta arms, so six kernels would
  multiply edits to the shared registry.
- One kernel means one codec tag, one port list and one natives entry.

Why not prelude-only templates for sub, reese and analog:

- Existing UGens can approximate them.
- They cannot give tempo-relative note length, slide, per-sample filter
  modulation, or the shared denormal/clamp guarantees (see the gap table).
- Once `bass-core` exists, making sub and reese models costs a model index,
  not a new UGen.

This satisfies the "new UGens only where existing ones cannot express the
sound" rule. The one new UGen is justified by the acid diode ladder, feedback
FM, slide, gate length and per-sample wobble. The six templates are thin
bodies over it.

**Kernel module layout.** No new crates. Every file is under 400 lines,
except `mods.rs`. Its cap is 450 lines, raised in session 232 for the
drift-free LFO.

| File | Responsibility |
|------|----------------|
| `src/dsp/ugen/bass_voice.rs` | Port layout indices, `STATE_FLOATS`, and `render`. Reads ports once per block and dispatches per-sample processing by model. |
| `src/dsp/ugen/bass_voice/osc.rs` | Band-limited oscillators (polyBLEP saw/pulse/square, polyBLAMP triangle, sine), sub-octave square, detuned pair, click transient |
| `src/dsp/ugen/bass_voice/ladder.rs` | TPT/ZDF 4-pole transistor ladder with input nonlinearity |
| `src/dsp/ugen/bass_voice/diode.rs` | TPT/ZDF 4-pole diode ladder with coupled stages |
| `src/dsp/ugen/bass_voice/fm.rs` | 2-operator feedback FM, ADAA sine wavefolder, bit-depth quantizer |
| `src/dsp/ugen/bass_voice/mods.rs` | Amp/filter envelopes, gate-length timer, accent, slide glide, tempo-synced LFO |
| `src/dsp/tests/dsp/bass_voice.rs` (plus submodules if needed) | Kernel unit tests (section "Verification") |

The digital-drum LFO helpers are `pub(super)` in `digital_drum/chain.rs`, so
they are not visible here. `mods.rs` implements the same two formulas itself,
without touching `digital_drum`:

- Synced rate: `rate_hz = lfo_rate * cps`.
- Transport-locked initial phase: `frac(rate_hz * onset_time)`.

## Signal flow

Shared chain, per voice, per sample:

```
pitch = freq * 2^((slide_offset(t) + click(t)) / 12)
osc   = model oscillator section (table below)
x     = osc * accent_gain                      # accent boosts level
fc    = cutoff * 2^(env_mod * fenv(t) + accent_oct * fenv(t) + lfo_oct(t))
y     = filter_model(x, fc, res, drive)         # ladder or diode, TPT/ZDF
y     = amp_env(t) * y
out   = soft_limit(y), clamped to [-1, 1]       # finite, bounded
```

| Model (`mode`) | Template | Oscillator section | Filter | Modulation |
|---|---|---|---|---|
| 0 | `analog-bass` | One `wave` oscillator plus a sub square at `sub-level` | transistor ladder | filter envelope, accent, slide |
| 1 | `acid-bass` | One `wave` oscillator (saw or square are the intended choices) | diode ladder | filter envelope, accent, slide |
| 2 | `fm-bass` | Carrier phase-modulated by a feedback modulator (`ratio`, `index`, `fm-feedback`), then the folder (`fold`), then bit depth (`bit-depth`) | transistor ladder | Index envelope follows the filter envelope; accent; slide |
| 3 | `wobble-bass` | Detuned `wave` pair (`detune`) plus a sub at `sub-level` | transistor ladder | tempo-synced LFO on cutoff, slide |
| 4 | `sub-bass` | One `wave` oscillator (sine by default) plus an attack click (`click-level`) | transistor ladder (low cutoff, no resonance) | slide |
| 5 | `reese-bass` | Detuned `wave` pair (`detune`) plus a sub at `sub-level` | transistor ladder | slow filter envelope, slide |

The oscillator phases of the detuned pair and the random LFO start from
`kx.seed`. That keeps renders deterministic and gives each voice its own
phases. Every other model starts its phases at zero.

## DSP components

**Transistor ladder (`ladder.rs`)**

- Four identical trapezoidal (TPT) one-pole low-pass stages with global
  negative feedback `k`.
- The zero-delay feedback loop is solved in closed form each sample, following
  Zavalishin's linear ladder solution.
- The only nonlinearity is a `tanh` on the input-minus-feedback term. It is
  evaluated on the linear solution with no iteration, which keeps CPU per
  sample bounded. This is a simplification of Huovilainen's per-stage `tanh`
  model, recorded in "Intentional simplifications".
- `res` 0..1 maps to `k` in `[0, K_LADDER_MAX]`. `K_LADDER_MAX` is a tuned
  constant just above 4, so `res = 1` rings audibly and saturation bounds it.
- Passband gain is partly compensated as `(1 + comp * k)`.
- The cutoff is recomputed every sample, `g = tan(pi * fc / sr)`, with `fc`
  clamped to `[20 Hz, 0.45 * sr]`.

**Diode ladder (`diode.rs`)**

- Four TPT integrators coupled as in the diode-ladder topology analysed by
  Stinchcombe and given in ZDF form by Zavalishin and Pirkle. Each stage's
  capacitor sees its neighbours, so stages are not buffered.
- The per-sample instantaneous linear system is tridiagonal and is solved in
  closed form by elimination (no matrix allocation, no iteration).
- There is one input `tanh`, as in the transistor ladder.
- A fixed first-order high-pass in the resonance feedback path keeps low-cutoff
  settings from thinning the bass. This follows the feedback-path coupling
  described in the circuit analysis; the corner is a documented tuned constant.
- `res` maps to `k` in `[0, K_DIODE_MAX]`. The diode ladder needs a higher
  feedback gain than the transistor ladder for the same resonance, so its
  constant is tuned separately.
- Cutoff clamping and per-sample updates are the same as the transistor
  ladder.

**Feedback FM (`fm.rs`)**

- The modulator phase is advanced at `ratio * pitch`. The modulator output is
  `sin(phase + beta * fb)`, where `fb` is the mean of the modulator's last two
  outputs. Two-sample averaging is the standard way to tame feedback-FM
  hunting.
- `beta = FB_MAX * fm-feedback`, with `FB_MAX` about 1.5 rad.
- The carrier is `sin(carrier_phase + index_eff * modulator)`, where
  `index_eff = index * (INDEX_FLOOR + (1 - INDEX_FLOOR) * fenv(t))`. This is the
  decaying brightness of a DX bass pluck.
- Folder: `fold` 0..1 sets the pre-gain `1 + FOLD_GAIN * fold` of a sine
  folder, computed with first-order antiderivative anti-aliasing (ADAA). When
  consecutive inputs are nearly equal it falls back to the direct value.
  `fold = 0` is an exact bypass.
- Bit depth: `bit-depth` 2..16 quantizes to `2^(bits - 1)` levels. 16 or more
  is an exact bypass.
- The quantizer lives in the kernel rather than in an existing `bitcrush`
  effect in the template body. That keeps every template knob a `bass-core`
  port, so editor metadata comes from one port table (see "Registration").

**Oscillators (`osc.rs`)**

- `wave` uses the existing `wave` row enum: `saw pulse square tri sine`.
  `pulse` uses a fixed 0.3 width.
- The sub is a polyBLEP square one octave down.
- The detuned pair runs two oscillators at plus and minus `detune / 2`
  semitones, each scaled by `1/sqrt(2)`.
- Click: a pitch blip that decays exponentially from `+CLICK_SEMIS` over about
  5 ms, scaled by `click-level`. `0` is exact bypass.
- Pitch is clamped to `[8 Hz, sr / 4]`.

**Envelopes and timing (`mods.rs`)**

- Gate length: `gate-length` is measured in sixteenth-steps of the current
  cycle. The note-off time is `gate-length / (16 * cps)` seconds, with `cps`
  clamped to the row range `[0.03, 50]`. See "Note length and slide" for why
  the kernel owns its note-off.
- Amp envelope:
  - linear `amp-attack` to 1;
  - exponential decay toward `sustain` with time constant `amp-decay`;
  - hold until note-off;
  - exponential `release` to -80 dB, then `finish()`.
- Filter envelope `fenv`: instantaneous peak 1, exponential decay with
  `env-decay`, retriggered per note except on slide notes.
- Accent `a` in `[0, 1]`:
  - raises the filter-envelope peak by `a * ACCENT_OCT` octaves, with a larger
    effect at higher `res`;
  - shortens the filter decay to `min(env-decay, ACCENT_DECAY)`;
  - multiplies the level by `1 + ACCENT_GAIN * a`, where `ACCENT_GAIN` is about
    0.4, so `a = 1` is about +3 dB.
- Slide: `slide-from` (semitones, `0` = none) offsets the start pitch.
  - The offset decays exponentially to 0; `slide-time` is the time to settle
    within 1% of the interval.
  - A note with `slide-from != 0` is a legato continuation: the amp starts at
    the sustain level through a fixed 2 ms anti-click ramp, and the filter
    envelope is not retriggered, so it starts decayed.
- LFO (wobble):
  - `rate_hz = lfo-sync ? lfo-rate * cps : lfo-rate`.
  - Initial phase `phi0` is `lfo-offset` when `lfo-retrigger` is true, and
    `frac(rate_hz * onset-time)` otherwise.
  - The phase is **derived, not accumulated** (revision 2026-09-30, session
    232). Adding `rate_hz / sr` to an f32 phase every sample rounds on every
    step. The measured error was 85265 samples per cycle instead of 85333 at
    0.5625 Hz, so a synced wobble slid off the beat. Instead, the LFO keeps
    an exact integer count `n` of the samples since its anchor. It then
    computes the position in f64:
    `pos = phi0 + n * rate_hz / sr`, `phase = frac(pos)`,
    `cycle = anchor_cycles + floor(pos)`. Phase error does not grow with
    playback time, and consecutive wraps are `sr / rate_hz` samples apart,
    within 1 sample.
  - Rate change: if the per-block `rate_hz` differs bit-for-bit from the
    anchored rate, the LFO re-anchors. It sets
    `phi0 = frac(pos)` and `anchor_cycles += floor(pos)` at the current `n`,
    then `n = 0` and stores the new rate. The phase stays continuous under
    `lfo-rate` modulation. `cps` is fixed per voice, so a synced LFO with a
    constant `lfo-rate` never re-anchors.
  - State (`Lfo::FLOATS = 6`, all slots exact in f32):
    `anchor_phase` in `[0, 1)`, `anchor_cycles` (a whole number up to 2^24),
    `count_lo` (a whole number below 2^20), `count_hi` (a whole number up to
    2^24, so `n = count_hi * 2^20 + count_lo`), `rate` (the anchored
    `rate_hz`), and `smooth`. An integer below 2^24 is exact in f32. Splitting
    the count keeps it exact far beyond any voice lifetime: 2^24 samples is
    only about 349 s at 48 kHz, while the `gate-length` maximum at the
    minimum `cps` is about 133 s plus release.
  - CPU: one f64 multiply, one divide and one floor per sample. There is no
    loop and no allocation.
  - Shapes come from the existing `lfo-wave` enum: `sine tri saw ramp square
    random`. `random` is sample-and-hold, seeded from `kx.seed`, one value per
    LFO cycle.
  - The unipolar LFO value `u` in `[0, 1]` goes through a one-pole smoother of
    about 2 ms, so square and random shapes do not click.
  - `lfo_oct = WOBBLE_OCT * lfo-depth * u`, with `WOBBLE_OCT = 5`. Negative
    depth sweeps downward.

**Drive and output**

- `drive` 0..1 sets the pre-filter gain into the filter's input nonlinearity:
  0 is unity, 1 is about +18 dB, with partial output compensation.
- The output goes through a soft limiter: identity below a knee, saturating
  above it. It is then clamped to `[-1, 1]` and multiplied by the template's
  `> * amp`.

**Real-time safety**

- All state lives in the voice's arena memory: `mem_need` returns
  `(bass_voice::STATE_FLOATS, 0)`.
- No allocation, no locks, no iteration beyond fixed loops, and one `tan` per
  filter per sample.
- At the end of each block, filter and glide states below `1e-20` in magnitude
  are flushed to 0. Any non-finite state resets that component to zero, as the
  existing `ladder` does.
- The kernel calls `finish()` once its release reaches -80 dB, so the voice
  ends through the existing envelope lifetime (`Node::is_env`).

## Controls

Kernel ports. Names that match a control row reuse the row's id, range, editor
kind and implicit-control behaviour. Every other name is a template-local
custom control. Its id comes from `CUSTOM_CTL_BASE` in first-use order, which
is safe because the new templates are appended (see "Registration").

| Port | Row or custom | Unit and range | Kernel default |
|------|---------------|----------------|----------------|
| `freq` | row | Hz | 55 |
| `mode` | literal in body | 0..5 | 0 |
| `cps`, `onset-time` | hidden rows (filled by commit) | row ranges | 0.5, 0 |
| `wave` | row enum `saw pulse square tri sine` | keyword | `:saw` |
| `cutoff` | row | 20..20000 Hz | 800 |
| `res` | row | 0..1 | 0.3 |
| `drive` | row | 0..1 | 0.2 |
| `detune` | row | 0..1 semitone spread | 0.15 |
| `ratio`, `index` | rows | 0..32 | 1, 1 |
| `amp-attack`, `amp-decay` | rows | s (row ranges) | 0.002, 0.3 |
| `sustain`, `release` | rows | 0..1, s | 1, 0.05 |
| `lfo-wave`, `lfo-rate`, `lfo-depth`, `lfo-offset`, `lfo-retrigger`, `lfo-sync` | rows (digital-drum LFO rows, same meaning) | row ranges | `:sine`, 4, 0, 0, true, true |
| `gate-length` | custom | 0.05..64 sixteenth-steps | 1 |
| `env-mod` | custom | 0..8 octaves | 2 |
| `env-decay` | custom | 0.01..4 s | 0.2 |
| `accent` | custom | 0..1 | 0 |
| `slide-from` | custom | -24..24 semitones | 0 |
| `slide-time` | custom | 0.005..1 s | 0.06 |
| `sub-level` | custom | 0..1 | 0 |
| `fm-feedback` | custom | 0..1 | 0 |
| `fold` | custom | 0..1 | 0 |
| `bit-depth` | custom | 2..16 | 16 |
| `click-level` | custom | 0..1 | 0 |

That is 32 ports, within `MAX_PARAMS = 48`.

- None of the custom names collides with an existing control row, template
  header name, effect parameter, or pattern native. Names were checked against
  `controls.rs` `ROWS`, every header in `templates.vact`, and
  `natives_domain.rs`. `slide`, `hold`, `gate`, `sub`, `crush`, `feedback`
  and `spread` were rejected for that reason.
- Row-named ports that a template does not expose remain implicit controls,
  per existing behaviour B2. For example, `> lfo-depth 0.3` on `acid-bass`
  would work. The kernel defaults keep the default sound unchanged.

Template headers are the editor-visible subset, with musically tuned defaults
for 125-140 BPM techno:

| Template | Header controls (default) |
|----------|---------------------------|
| `analog-bass` | `wave :saw`, `cutoff 420`, `res 0.35`, `env-mod 2.5`, `env-decay 0.18`, `accent 0`, `sub-level 0.35`, `drive 0.25`, `gate-length 0.9`, `amp-attack 0.002`, `amp-decay 0.4`, `sustain 0.85`, `release 0.04`, `slide-from 0`, `slide-time 0.06` |
| `acid-bass` | `wave :saw`, `cutoff 320`, `res 0.72`, `env-mod 3.2`, `env-decay 0.22`, `accent 0`, `slide-from 0`, `slide-time 0.06`, `drive 0.3`, `gate-length 0.55`, `release 0.02` |
| `fm-bass` | `ratio 1`, `index 2.5`, `fm-feedback 0.35`, `fold 0`, `bit-depth 16`, `cutoff 2400`, `res 0.1`, `env-mod 1`, `env-decay 0.15`, `accent 0`, `drive 0.1`, `gate-length 0.8`, `amp-decay 0.25`, `sustain 0.45`, `release 0.05`, `slide-from 0`, `slide-time 0.05` |
| `wobble-bass` | `wave :saw`, `detune 0.12`, `sub-level 0.4`, `cutoff 180`, `res 0.45`, `drive 0.35`, `lfo-wave :sine`, `lfo-rate 4`, `lfo-depth 0.8`, `lfo-offset 0`, `lfo-retrigger true`, `lfo-sync true`, `gate-length 4`, `release 0.08`, `slide-from 0`, `slide-time 0.08` |
| `sub-bass` | `wave :sine`, `cutoff 300`, `res 0`, `drive 0.15`, `click-level 0.2`, `gate-length 1.5`, `amp-attack 0.003`, `release 0.06`, `slide-from 0`, `slide-time 0.05` |
| `reese-bass` | `wave :saw`, `detune 0.18`, `sub-level 0.3`, `cutoff 650`, `res 0.2`, `env-mod 0.8`, `env-decay 0.6`, `drive 0.3`, `gate-length 3.5`, `amp-attack 0.01`, `release 0.12`, `slide-from 0`, `slide-time 0.1` |

- Each template body passes its header controls by name to
  `bass-core freq ... mode: N` and ends in `> * amp`, as
  `src/types/tests/inst/templates.rs` requires.
- A header default that differs from its row default is listed in
  `TEMPLATE_DEFAULT_OVERRIDES` (`src/dsp/meta.rs`), for example
  (`wobble-bass`, `lfo-sync`, `true`).
- The defaults are starting points. The registry plan may retune a default
  only with the audibility and low-band evidence described in "Verification".

## Tempo sync and wobble divisions

`lfo-rate` under `lfo-sync true` means LFO cycles per pattern cycle, exactly as
in the digital drums. With the default 4-beat cycle
(`cps = bpm / 60 / beats_per_cycle`):

| Division | `lfo-rate` | Division | `lfo-rate` |
|----------|-----------:|----------|-----------:|
| 1/1 | 1 | 1/2 triplet | 3 |
| 1/2 | 2 | 1/4 triplet | 6 |
| 1/4 | 4 | 1/8 triplet | 12 |
| 1/8 | 8 | 1/16 triplet | 24 |
| 1/16 | 16 | dotted 1/8 | 16/3 |

- The LFO period is `1 / (lfo-rate * cps)` seconds.
- With `lfo-sync false`, `lfo-rate` is in Hz (free-rate mode).
- With a non-default `beats_per_cycle`, the divisions are relative to the
  cycle. The README and the preset comments say so.
- `cps` is captured when an event is committed, so a tempo change reaches the
  next note, not a note that is already sounding. This is the same as the
  digital drums.
- `lfo-retrigger false` locks the phase to host seconds
  (`frac(rate_hz * onset-time)`), not to bar position. `onset-time` wraps at
  3600 s. Both limitations are shared with the digital drums and are
  documented, not fixed here.

`src/sched/commit.rs` fills `cps` and `onset-time` only for instruments whose
nodes match `wants_tempo_anchor`. Adding `Node::BassCore` to that `matches!`
list is the only scheduler-crate change. It is a one-line node-kind addition,
not a scheduler rewrite, and it cannot affect existing instruments.

## Note length and slide

**Note length.** Audio events carry no duration, and `legato` never reaches
audio voices. Passing the pattern duration to every audio voice would change
the gate of every `env-adsr` template, and so every existing digest. That is
forbidden. Instead:

- `bass-core` owns its note-off through `gate-length`, in tempo-relative
  sixteenth-steps (above).
- It ignores `kx.gate` for note-off timing, as the self-enveloped drum cores
  already do.
- A cut-group choke still works, because `short_gate` is a voice-level 3 ms
  fade that does not depend on the kernel.
- Limitation: a live-input or MIDI voice that holds a key does not sustain
  past `gate-length`. The bass templates are designed for patterns. Held-key
  sustain is recorded as a user question.

**Slide (chosen mechanism: per-event `slide-from`, applied inside the new
voice).**

- A pattern sets `slide-from` to the interval from the previous note, for
  example `note [:c2 :c3] > slide-from [0 -12]`, and optionally `slide-time`.
- The new voice starts at the offset pitch, glides to its own pitch, and does
  not retrigger the amp or filter envelopes.
- Bass lines use `> cut 1` so the new voice chokes the previous one with the
  existing 3 ms fade.
- Mapping from TB-303-style programming: a slide flag on step N becomes
  `slide-from = note(N) - note(N+1)` on step N+1, plus a `gate-length` of at
  least 1 on step N, so the notes do not leave a gap.

Trade-offs, documented in the template comment and README:

- The user writes the interval; the voice does not infer it.
- During the 3 ms crossfade the old and new voices' oscillator phases and
  filter states are independent.
- Accent does not build up across consecutive accented notes, as a hardware
  accent capacitor would.
- The filter envelope on a slide note starts fully decayed rather than at the
  previous note's exact level.

Considered and not chosen:

1. **Engine-side handoff.** `Engine::start` would copy the previous same-cut
   voice's pitch and envelope state into the new voice before
   `choke_cut_group`. It gives automatic slide, but it adds cross-voice state
   reads in `engine.rs` and a new node-state contract.
2. **Monophonic legato voice mode.** One voice would be re-pitched instead of
   restarted. This needs voice-pool and scheduler changes.

Both are outside the minimal scope. Option 1 is recorded as a user question
for a later change.

## Registration and digest stability

All ordered registries are **append-only**:

- New `inst` blocks go at the end of `src/prelude/templates.vact`.
- New names go at the end of `UGEN_NAMES`, both `TEMPLATE_NAMES` lists, and the
  `UGENS` names table.
- New custom controls get the next custom ids in first-use order.
- The codec tag is the next free explicit tag, `98`. Existing tags are never
  renumbered.

No new control row is added, so no existing template's custom-id assignment
can shift.

Files touched by registration (one serialized plan owns all of them):

- **Graph and node:**
  - `src/dsp/graph.rs`: `UGenSpec::BassCore`.
  - `src/dsp/ugen/mod.rs`: `pub mod bass_voice`, `Node::BassCore`, and
    `is_env` membership.
  - `src/dsp/ugen/template.rs`: add to the self-enveloped count.
  - `src/dsp/ugen/build_helpers.rs`: the `mem_need` arm.
  - `src/dsp/ugen/mixer.rs`: the render arm.
- **Catalog and names:**
  - `src/dsp/ugen/catalog.rs`: `ports`, `ugen_name`, `UGEN_NAMES`,
    `TEMPLATE_NAMES` and `node_of`.
  - `src/dsp/ugen/catalog/voice_ports.rs`: the `BASS_CORE` port list.
  - `src/dsp/ugen/catalog/codec.rs`: tag 98 encode and decode.
  - `src/dsp/build/names.rs` and `names/table.rs`: the name and the port
    order, matching the catalog port names.
  - `src/types/natives_domain.rs`: `dsp("bass-core")`.
- **Metadata and templates:**
  - `src/dsp/meta.rs`: the `ugen_node` arm, the template-to-node arm, custom
    port ranges, and `TEMPLATE_DEFAULT_OVERRIDES`.
  - `src/dsp/meta/templates.rs`: `TEMPLATE_PARAMS` gains
    `.chain(bass::TEMPLATE_PARAMS)` or the equivalent entry point.
  - New `src/dsp/meta/templates/bass.rs`, mirroring `templates/plaits.rs`, so
    `meta/templates.rs` (747 lines) does not grow toward 1000.
  - `src/ns/insts.rs`: `TEMPLATE_NAMES` array length 67 becomes 73.
  - `src/prelude/templates.vact`.
- **Runtime:**
  - `src/sched/commit.rs`: `wants_tempo_anchor`.
  - `src/dsp/ring.rs`: raise `template_slots` from 72 to 80 and fix the stale
    "63 definitions" comment. With 73 prelude, 2 live-input and 4 quad-stem
    definitions, the count is 79. At 72 the 73rd template would fail to
    install with `BadResource`.
- **Tests:**
  - `src/dsp/tests/dsp.rs` and `src/host/tests/e2e/templates.rs`: test module
    lines.
  - `src/host/tests/e2e/templates/golden_digests.txt`: new lines only.

Digest guard:

- Each new template adds one `graph`, one `render ... center` and one
  `render ... pan02` line (the kernel is mono).
- Lines are regenerated with `VACTR_BLESS_GOLDEN=1` and the ignored bless test,
  which rewrites the whole file.
- The plan must then show that `git diff -U0` of `golden_digests.txt` contains
  no removed digest line (`^-[^-]` is empty) and exactly 18 added lines.
- Existing kernel seeds cannot shift. The seed is
  `voice_seed + seed_ordinal(i)`: the voice counter plus the node position
  within its own template, with no global input. Graph digests of existing
  templates cannot shift either, because custom ids and name tables are
  append-only.
- Raising `template_slots` must be shown not to change any digest; the same
  diff check covers it.

Line budgets: every touched Rust file stays under 1000 lines. `ns/insts.rs`
(829) and `meta.rs` (676) grow by small amounts. The bass metadata goes in
the new `templates/bass.rs`. If any touched file would reach 1000 lines, the
plan splits it as the rust-coding standard requires.

## Presets and examples

**`examples/bass-presets.vact`**

- Named patches are `fn <family>-<name> notes:` definitions. Each returns
  `s :<template> > note notes > ...controls`, so a caller writes
  `acid-squelch [:c2 :c2 :eb2 :c3] > accent [0 1 0 0] > cut 1 > d1`.
- `<family>` is one of `analog`, `acid`, `fm`, `wobble`, `sub`, `reese`, and
  identifies the template for tests.
- At least three patches per family, including:

| Family | Patches |
|--------|---------|
| `acid` | `acid-squelch`, `acid-rolling`, `acid-deep` |
| `sub` | `sub-deep`, `sub-driven`, `sub-long` |
| `reese` | `reese-hoover`, `reese-dark`, `reese-rumble` |
| `wobble` | `wobble-quarter` (rate 4), `wobble-eighth` (8), `wobble-triplet` (6), `wobble-free` (sync off) |
| `fm` | `fm-pluck`, `fm-digital-grit` (fold and `bit-depth 6`), `fm-metal` |
| `analog` | `analog-pluck`, `analog-driven`, `analog-square` |

- The file ends with commented demo lines.
- Plan checkpoint: first confirm that a `fn`-returned pattern can be piped to a
  sink. If it cannot, presets become zero-argument functions that return a
  complete demo pattern, and the family-prefix rule stays.

**Example tracks.** Each is `use-bpm` 125-138 and combines existing drum
templates with the new basses.

| File | Content |
|------|---------|
| `examples/acid-techno.vact` | `acid-bass` with accent and slide patterns under `cut 1`, plus kick and hats |
| `examples/rumble-techno.vact` | Rolling 16th `sub-bass`/`reese-bass` rumble, shaped with existing effects (design-music section 5, such as a reverb and low-pass on the bass orbit), plus kick |
| `examples/offbeat-bass-techno.vact` | Minimal offbeat `analog-bass`/`sub-bass` against a four-on-the-floor kick |
| `examples/wobble-techno.vact` | `wobble-bass` switching 1/8 and triplet divisions per bar |

Levels in all examples are set so the mix peak stays at or below 1.0
without a master limiter. There is no default master limiter; `guard()` only
zeroes non-finite samples.

`examples/industrial-techno.vact` and every other existing example are left
unchanged.

**README.** Add a short bass bullet group under `## Status`: the six
templates, the slide mechanism, the wobble division table pointer, the presets
file, and the four examples.

## Verification

All test names contain `bass`, so the gate filter is
`cargo nextest run -E 'test(/bass/)'`. The count must be positive.

**Kernel unit tests** (`src/dsp/tests/dsp/bass_voice*.rs`). They call the
component structs and the kernel `render` directly at 48 kHz.

- Filter response:
  - Transistor ladder and diode ladder at `res = 0`: gain at `fc/4` within
    3 dB of the gain at `fc/16`; attenuation at `4*fc` of at least 30 dB
    (ladder) and at least 20 dB (diode).
  - The peak near `fc` grows monotonically over `res` in
    {0, 0.25, 0.5, 0.75, 1}. At `res = 1` it is at least 12 dB above `res = 0`.
- Accent: `accent = 1` against `0` on the same note gives a higher peak level
  (at least 2 dB) and a higher spectral centroid in the first 50 ms.
- Slide: with `slide-from = -12`, the first-cycle period is about twice the
  target period, and the period settles within 1% by `slide-time`. A slide
  note starts at sustain level (no attack ramp beyond 2 ms). The filter
  envelope is not retriggered.
- Gate length: the note-off time equals `gate-length / (16 * cps)` within one
  sample, for cps in {0.5, 0.5625}.
- Wobble period: at `cps = 0.5625` (135 BPM), `lfo-sync true`, and `lfo-rate`
  in {1, 2, 4, 8, 3, 6, 12}, the LFO phase wraps every
  `sr / (lfo-rate * cps)` samples, within 1 sample. Tolerances are not
  loosened.
- Free-rate mode: `lfo-sync false`, `lfo-rate 3.2` gives a period of 3.2 Hz.
  The first wrap from phase 0 is at sample 15000 +/-1.
- Drift-free: run `next` 5,760,000 times (120 s at 48 kHz) with
  `cps 0.5625` and `lfo-rate 4` (2.25 Hz). The cycle count is then exactly
  270, and the phase is within 1e-6 of 0 by circular distance. The old
  accumulating LFO ended about 0.2 cycles off here. A state loaded with `count_hi` greater than 0
  (an elapsed count above 2^20) gives the same phase as the closed form.
- Rate change: switching `rate_hz` mid-stream moves the phase by at most one
  new-rate increment across the switch, and the cycle count never
  decreases.
- Phase: `lfo-retrigger false` starts at `frac(rate_hz * onset-time)`.
- Stability: a grid of extreme controls, including
  - `freq` 20 and 2000;
  - `cutoff` 20 and 20000; `res` 1; `drive` 1;
  - `index` 32; `fm-feedback` 1; `fold` 1; `bit-depth` 2;
  - `lfo-depth` plus and minus 1; `slide-from` plus and minus 24.

  For every model, 2 s renders produce only finite samples with `|y| <= 1`.
  After the voice finishes, all filter states are exactly 0 (the denormal
  flush).
- Determinism: two renders with the same seed are bit-identical.
- Bypass exactness: `fold 0`, `bit-depth 16` and `click-level 0` are
  bit-identical to the same render with those stages removed.

**E2e template tests** (`src/host/tests/e2e/templates/bass.rs`):

- Every bass template is realized and audible at `note [:c4]`. The existing
  generic audibility test (`RMS > 1e-3`) covers this automatically.
- The commit stage delivers `cps` to `bass-core`: a wobble render at two
  `use-bpm` values shows the expected brightness-modulation period.
- Golden lines exist for all six templates.
- A seed-order case in `src/dsp/tests/dsp/seed_order.rs` shows that a bass
  template's seed ordinals equal plain node positions (no gates).

**Offline render tests** (`src/host/tests/e2e/templates/bass_renders.rs`). They
use the existing `E2e` rig (`E2e::new`, `eval`, `run_for`) and
`dsp::offline::{rms, peak, spectrum}`, which already assert zero allocations
per block.

- Every preset fn found by scanning `^fn ` lines in
  `examples/bass-presets.vact` is rendered for 2 s with a family-specific demo
  phrase. The test asserts at least 18 presets and at least 3 per family.
- Every example track is rendered for at least 2 cycles.
- Each render is written to `tmp/bass/<name>.wav` by a small test-only 16-bit
  PCM WAV writer. `tmp/` is gitignored; no WAV writer exists in the codebase
  today. A write failure fails the test.
- Assertions:
  - All samples are finite.
  - `rms > 1e-3` (not silent).
  - `peak <= 1.0` (no clipping, since there is no master limiter).
  - Low-band energy ratio: the share of spectrum power between 20 and 250 Hz,
    from `spectrum` with the DC bin excluded.

    | Render | Minimum low-band share |
    |--------|------------------------|
    | `sub-*` presets | 0.8 |
    | `analog-*`, `reese-*`, `wobble-*`, `fm-*` presets | 0.5 |
    | Example tracks | 0.3 |

    `acid-*` presets are exempt from this check, because resonant squelch is
    intentionally mid-heavy, but still pass the other assertions.
- Thresholds may be tuned only with the measured values recorded in the plan's
  progress log, and never below 0.5 for `sub-*`.

**Gates.**

- Each plan runs rustfmt check on the files it owns and its nextest filter.
- Reconcile and integration run the full gate:
  - `CARGO_TERM_QUIET=true cargo build`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
  - full `cargo nextest run`
  - `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  - `mise run lint`

## Implementation partition

These are recommendations for the plan step.

| Plan | Owns | Depends on | Parallel |
|------|------|------------|----------|
| BASS-01 kernel | `src/dsp/ugen/bass_voice.rs`, `src/dsp/ugen/bass_voice/*`, `src/dsp/tests/dsp/bass_voice*.rs`, plus the one-line `pub mod bass_voice;` in `src/dsp/ugen/mod.rs` and the test module line in `src/dsp/tests/dsp.rs` | none | May split into filters (`ladder.rs`, `diode.rs`) and sources/mods (`osc.rs`, `fm.rs`, `mods.rs`) sub-plans. Only the plan that owns `bass_voice.rs` edits `ugen/mod.rs` and `tests/dsp.rs`. |
| BASS-02 registry and templates | Every registration file above, `templates.vact`, `commit.rs`, `ring.rs`, golden digests, `src/host/tests/e2e/templates/bass.rs`, seed-order case | BASS-01 | No. Registry files are serialized. |
| BASS-03 presets, examples, renders, README | `examples/bass-presets.vact`, the four example `.vact` files, `src/host/tests/e2e/templates/bass_renders.rs` (plus a test-only WAV helper), its module line in `src/host/tests/e2e/templates.rs`, `README.md` | BASS-02 | No |

Fanout manifests list only these tracked source paths. They never list
`target/`, `tmp/`, `editor/node_modules` or `editor/dist`.

## Intentional simplifications

- The input `tanh` is solved once per sample instead of with per-stage
  nonlinearities and Newton iteration. This bounds CPU. The circuit character
  comes from topology (transistor ladder vs diode ladder), resonance, and
  drive.
- There is no oversampling. polyBLEP/polyBLAMP oscillators and a low
  fundamental keep aliasing low. ADAA handles the folder. The filters'
  saturation at high drive can alias; this is accepted for bass.
- Slide and accent are per-voice approximations (see "Note length and slide").
- The LFO phase comes from an exact per-voice sample count, so it cannot
  drift within a voice. Only the `rate_hz` value itself is rounded to f32.
  Bar alignment across voices still depends on `onset-time`, as the "Tempo
  sync and wobble divisions" section describes.
- The kernel is mono. Reese and wobble width come from effects in examples,
  not from the kernel.

## Risks

- **Digest drift.** It is mitigated by append-only registries, no new control
  rows, a per-template seed ordinal, and the `git diff` removed-line check
  after blessing.
- **Filter tuning.** Diode ladder `K_DIODE_MAX` and the feedback high-pass
  corner are tuned constants. The unit tests bound resonance and stability,
  but the sound is judged by ear from the WAVs in `tmp/bass/`.
- **Automated checks do not prove "sounds good".** Non-silence, peak and
  low-band share only partly capture it. The WAV files support a manual
  listening pass at review.
- **Merge overlap with wf/syntax-fmt.** `README.md` is the only file that might
  overlap. Keep the README edit to one contiguous bullet group.

## References

See also [`../references/README.md`](../references/README.md#bass-synthesis).

- T. E. Stinchcombe, "Analysis of the Moog Transistor Ladder and Derivative
  Filters", 2008 (includes the TB-303 diode ladder), timstinchcombe.co.uk.
- V. Zavalishin, *The Art of VA Filter Design*, rev. 2.1.2, Native Instruments,
  2020. TPT integrators, zero-delay feedback, transistor ladder, diode ladder.
- W. Pirkle, *Designing Software Synthesizer Plugins in C++*, 2nd ed., Focal
  Press/Routledge, 2021, and his VA diode ladder application note. Equations
  only; no book or app-note code is used.
- A. Huovilainen, "Non-linear digital implementation of the Moog ladder
  filter", Proc. DAFx-04, 2004.
- V. Valimaki and A. Huovilainen, "Oscillator and filter algorithms for
  virtual analog synthesis", Computer Music Journal 30(2), 2006.
- S. D'Angelo and V. Valimaki, "An improved virtual analog model of the Moog
  ladder filter", Proc. IEEE ICASSP, 2013.
- V. Valimaki and A. Huovilainen, "Antialiasing oscillators in subtractive
  synthesis", IEEE Signal Processing Magazine 24(2), 2007 (polyBLEP).
- V. Valimaki, J. Pekonen and J. Nam, "Perceptually informed synthesis of
  bandlimited classical waveforms using integrated polynomial interpolation",
  JASA 131(1), 2012.
- F. Esqueda, V. Valimaki and S. Bilbao, "Rounding corners with BLAMP", Proc.
  DAFx-16, 2016 (polyBLAMP).
- J. M. Chowning, "The synthesis of complex audio spectra by means of frequency
  modulation", JAES 21(7), 1973.
- N. Tomisawa, "Tone production method for an electronic musical instrument",
  US Patent 4,249,447, 1981 (operator self-feedback FM).
- F. Esqueda, H. Pontynen, J. D. Parker and S. Bilbao, "Virtual analog models
  of the Lockhart and Serge wavefolders", Applied Sciences 7(12), 2017.
- F. Esqueda, H. Pontynen, V. Valimaki and J. D. Parker, "Virtual analog Buchla
  259 wavefolder", Proc. DAFx-17, 2017.
- J. D. Parker, V. Zavalishin and E. Le Bivic, "Reducing the aliasing of
  nonlinear waveshaping using continuous-time convolution", Proc. DAFx-16, 2016
  (ADAA).
- S. Bilbao, F. Esqueda, J. D. Parker and V. Valimaki, "Antiderivative
  antialiasing for memoryless nonlinearities", IEEE Signal Processing Letters
  24(7), 2017.
- M. Puckette, *The Theory and Technique of Electronic Music*, World
  Scientific, 2007 (phasors and LFOs; tempo-synced rate is `cycles * cps`).
- Roland TB-303 Owner's Manual, 1982 (user-level accent and slide programming
  model only).
