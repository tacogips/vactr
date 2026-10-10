# Microtonal Tuning and Chord Performance

Design for first-class tuning systems (n-EDO, EDO of a non-octave period,
just-intonation ratio lists, Scala `.scl`/`.kbm`) and for the strum, harp
(strum-plate) and Orchid-style chord performance combinators. Branch
`wf/fm1-tuning`, language and pattern layer only.

## 1. Overview

### 1.1 Scope

In scope:

- Tuning values, the `tune` combinator, and frequency resolution through a
  tuning per track, instrument chain and event, with a reference pitch and
  a root key.
- Tuning-aware `scale`, `chord`, `voicing` (and `arp`, which is order-only).
- Microtonal scale presets (EDO subsets, maqam, slendro, pelog, Bohlen-Pierce
  lambda) and tuning presets (5-limit, 7-limit, Partch 43, Bohlen-Pierce).
- The MIDI-out policy for tuned notes.
- New pattern combinators `strum`, `harp`, `inversion`, `perform`.
- `lang-reference.md`, editor completion and hover metadata (the native
  table), and runnable examples.

Out of scope (owned by the sibling run `wf/fm1-voices`, or not requested):

- UGens, instrument templates, codec tags, `TEMPLATE_NAMES`, the template
  golden digests (`src/host/tests/e2e/templates/golden_digests.txt`). This
  design adds no UGen, no template, no codec tag and no golden digest line.
- New `dsp::controls` rows. The tuning travels as an ordinary event control
  with no control-table row (section 4.6).
- MIDI pitch bend or MPE output (section 4.9; decided as TQ1 (a)).
- Audio-thread code. All tuning work runs on the scheduler side.

### 1.2 Baseline (current behavior, verified in the tree at 9ac8d1f)

Re-checked at dbad8c3 (2026-10-10): that commit changed only design docs
and plans, so every source fact below still holds (the `PatNode` hash
tag maximum is still 42, `src/pattern/pat.rs:230`).

| Fact | Location |
| --- | --- |
| A note number becomes Hz only through `note_to_freq(n) = 440 * 2^((n - 69) / 12)` | `src/sched/commit.rs:43` |
| Audio commit converts each tone of the `note` (or `n`) control, a chord list giving one `AudioEvent` per tone; a var-driven note becomes a `CellMap::NoteToFreq` cell | `src/sched/commit.rs` `audio_events`, `src/sched/cells.rs:73` |
| Song mode converts the frozen `row.note` with the same function | `src/sched/song/encode.rs:212` |
| Note-name keywords become keys via `note_number` in `commit::note_of` (audio and MIDI) and in song freezing (`ResolvedNote::Int`) | `src/sched/commit.rs:49`, `src/song/source.rs:176-182` |
| MIDI out rounds the note number to `u8` (`n.round().clamp(0, 127)`); `MidiEvent::Note` has no bend field | `src/sched/commit.rs` `midi_events`, `src/host/caps.rs:354` |
| The scheduler carries pitch as a note number (f64, fractional allowed) until commit | `commit::note_of` |
| Note names are `12 * octave + pc`, octave 5 by default (`:c` = 60, A = 69 = 440 Hz is `:a5`) | `src/pattern/combinators/music.rs:56` |
| 17 named scales, all 12-tone; `scale_note` hard-codes `oct * 12`; `voice` hard-codes `rem_euclid(12)` and `[60, 72)` | `src/pattern/combinators/music.rs` |
| Frozen song controls admit only nil, bool, number, keyword, string and lists of these; a dict control fails freezing | `src/song/snapshot.rs:802` `FrozenControl::copy` |
| A step value that is a dict or string is one `pure` step; a list is a step sequence | `src/pattern/build.rs` `pattern_of` |
| Completion detail and hover types come from the native table type strings | `src/complete/sources.rs`, `src/types/natives_domain.rs`, `src/lsp/analysis.rs` |
| `invert` is already a texture native; `bass` is a common user binding (`examples/ode-to-joy.vact:34`) | `src/types/natives_domain.rs:302` |
| `load` reads files through the session `SourceLoader` (native and browser hosts) | `src/ns/load.rs` |
| Fenced `vactr` blocks of `lang-reference.md` and `design-music.md` are read and checked by tests | `src/reader/tests/mod.rs`, `src/types/tests/no_abort.rs` |

No existing design covers tuning or strum; `design-music.md` section 3 is
the baseline for `scale`/`chord`/`voicing`/`arp` and stays valid. This
document extends it and is linked from it.

## 2. Decisions

| ID | Decision | Rationale |
| --- | --- | --- |
| D1 | No tuning set means the exact existing code paths: `note_to_freq` is unchanged and is still the only conversion for an event without a tuning control; the 17 scales, `chord`, `voicing` and MIDI rounding keep their integer arithmetic. | Bit-identical default output and unchanged golden digests by construction, not by float coincidence. |
| D2 | A tuning SPEC is a dict value (section 4.1). On events it travels as the `tuning` control in a canonical list form (section 4.6). | A dict is one `pure` step in any pattern position (`alt`, `choose`), so tunings are patternable per event; the list form freezes into song mode (`FrozenControl::List`) without touching the snapshot codec. |
| D3 | `tune` is a control-like combinator (the `chord` shape): `subject > tune t`. It never gives structure to an unstructured subject unless `t` is itself a structured pattern. | Same first-structure rule as every control (design 10.1), so per-track (`s :x > tune t > ...`) and per-event (`tune (alt [..])`) both fall out. |
| D4 | Keys are numbers. Under a tuning, key `root` (default 60) is degree 0, and by default key `root` sounds at its 12-TET pitch `note_to_freq(root)`; `ref-key`/`ref-freq` override the anchor. | Switching tunings keeps the root in place (middle C stays 261.6256 Hz); matches the common synth default. Decided as TQ2 (a). |
| D5 | Frequency resolution runs only on the scheduler side (commit and song encode). The audio thread receives `f32` freq constants as today. | No allocation and no locks on the audio thread; no audio-thread code changes. |
| D6 | MIDI out under a tuning sends the nearest 12-TET MIDI note of the tuned frequency, no pitch bend. | Smallest correct policy; `MidiEvent` and both MIDI hosts stay unchanged. Limitation documented; decided as TQ1 (a), MPE out of scope. |
| D7 | 12-tone interval vocabularies (chord qualities, the 17 scales, note names) map into a tuning by the nearest-key rule (section 4.7), except when the tuning has 12 keys per period, where they apply directly. | Makes `chord`/`scale`/`voicing` work in any tuning, and gives just intonation on the familiar 12-key layout. |
| D8 | No permissive upstream is used: chord, scale and tuning tables come from public-domain music theory or are computed; the Scala format is implemented from its public format description. choralroot and Chordian are not used, so `THIRD_PARTY_NOTICES.md` is unchanged. | Removes the license-verification dependency; no GPL firmware is read. |
| D9 | New public names: `tune`, `edo`, `ratios`, `scala`, `load-scala`, `strum`, `harp`, `inversion`, `perform`. No public `bass` or `invert`. | `invert` is a texture native; `bass` is a common user `let` name. The chosen names collide with no prelude native and no `let`/`var`/`fn` in `examples/`, `src/` or `tests/` (grep at 9ac8d1f). |

## 3. Data flow

```
tuning spec (dict)          pattern chain                         scheduler side
edo / ratios / scala  --->  s :x > tune T > n [..] > scale ..  --> commit (audio): note -> Tuning::freq -> f32 freq
load-scala / :preset        > chord .. > voicing > strum ..        commit (MIDI):  note -> freq -> nearest MIDI note
                              |                                    song encode:   row.note -> Tuning::freq
                              +-- event control `tuning` =         (no tuning control: note_to_freq, unchanged)
                                  canonical list (4.6)
```

Tuning-aware combinators read the event's `tuning` control, so they must
come AFTER `tune` in the chain (`tune` is applied to the events they
receive). A `tune` placed after them still retunes the frequencies, but
the combinators before it used 12-tone arithmetic on plain keys.

## 4. Tuning model

### 4.1 Tuning specs

A tuning spec is a dict with a `kind` key:

| Kind | Keys | Meaning |
| --- | --- | --- |
| `:edo` | `steps` (int), `period` (exact int or ratio) | `steps` equal divisions of `period` (2 = octave, 3 = tritave) |
| `:degrees` | `degrees` (list), optional `description` (string), optional `keymap` (dict, section 4.4) | degree 1..N pitches; entry N is the period; degree 0 is the implicit 1/1 |

A `degrees` entry is an exact frequency ratio (int or ratio) or a cent
value (float). Constructors:

| Call | Result |
| --- | --- |
| `edo 19` | `{kind: :edo steps: 19 period: 2}` |
| `edo 13 period: 3` | Bohlen-Pierce equal tempered |
| `ratios [9/8 5/4 4/3 3/2 5/3 15/8 2]` | `:degrees` with exact ratios (1/1 is not listed; the last entry is the period) |
| `scala text` / `scala text kbm: text` | parsed `.scl` (and `.kbm`) text, section 4.4 |
| `load-scala path` / `load-scala path kbm: path` | the same, read through the session `SourceLoader` (an effect, like `load`; not allowed in a query); a host without a loader fails `host-unavailable`, inline `scala` works on every host including wasm |
| keyword preset (`:ji-5`, `:ji-7`, `:partch-43`, `:bohlen-pierce`) | accepted wherever a spec is (section 4.8) |

Validation (failure code `type` with a message naming the field; at the
constructor call, or event-local when the spec arrives per event):

- `steps` in 1..=1200; `period` exact and > 1.
- `degrees` has 1..=1024 entries; ratio entries > 0; cent entries finite;
  the period (last entry) is > 0 cents. Entries need not be ascending.
- A hand-written dict is accepted if and only if it passes the same rules.

### 4.2 Keys, degrees and frequency

`K` is the tuning's keys per period: `steps` for `:edo`, N for `:degrees`
without a keymap, the keymap size `m` with a keymap (`m = 0` means linear,
so `K = N`).

Mapping parameters: `root` (key of degree 0), `ref-key`, `ref-freq`.
Defaults: `root` 60; `ref-key` = `root`; `ref-freq` = `note_to_freq(ref-key)`.
A keymap supplies its own middle note, reference note and frequency as the
defaults; `tune` keyword arguments override each one individually.

For an integer key `k`:

- `:edo`: `f(k) = ref-freq * exp2((k - ref-key) * log2(period) / steps)`.
  With `period` 2 the factor is `exp2((k - ref-key) / steps)`.
- `:degrees`: `deg(k)` is `k - root` without a keymap (section 4.4 with one);
  `R(d) = P^(d div N) * r[d mod N]` with `r[0] = 1`, `r[j]` the ratio of
  degree j (a cent entry c is `exp2(c / 1200)`) and `P` = degree N;
  `f(k) = ref-freq * R(deg(k)) / R(deg(ref-key))`.

A fractional key `k = i + t` (0 < t < 1) interpolates in log frequency:
`f(i) * (f(i + 1) / f(i))^t` (for `:edo` this equals the closed form).
A key with no mapping (keymap `x`, or outside the keymap range) yields no
tone: that tone is skipped silently, as in Scala. A non-finite or
non-positive result is an event-local `type` failure.

### 4.3 Analytic examples (the test oracle)

With defaults (`root` = `ref-key` = 60, `ref-freq` = `note_to_freq(60)`):

| Tuning | Key | Frequency |
| --- | --- | --- |
| `edo 19` | 60 + s | `note_to_freq(60) * 2^(s/19)` |
| `edo 31` | 60 + s | `note_to_freq(60) * 2^(s/31)` |
| `edo 13 period: 3` | 60 + s | `note_to_freq(60) * 3^(s/13)` |
| `ratios [..12 entries..]` | 60 + 12q + j | `note_to_freq(60) * 2^q * r[j]` |
| `edo 19` with `ref-key: 69 ref-freq: 440` | 69 | exactly 440 |

### 4.4 Scala text

`.scl` (public format description): lines starting with `!` are comments.
The first non-comment line is the description (may be empty). The next is
the note count N (leading whitespace allowed, trailing text ignored). The
next N non-comment lines are pitches: leading whitespace ignored, the value
ends at the first whitespace; a value containing `.` is cents (sign
allowed), otherwise `a/b` or `a` (= a/1) with positive integers. Text after
the value is ignored. Fewer than N pitch lines, N = 0, a malformed value,
or a non-positive period is a `type` failure; lines after the N pitches
are ignored. Line endings `\n` and `\r\n` are both accepted.

`.kbm`: comment lines start with `!`. Non-comment lines in order: map size
m, first key, last key, middle key, reference key, reference frequency
(float), formal-octave degree (0 means N), then up to m entries, each a
degree or `x` (missing trailing entries are `x`). Key k is unmapped when
outside `[first, last]` or when its entry is `x`; otherwise
`offset = k - middle`, `deg(k) = (offset div m) * O + map[offset mod m]`
with O the formal-octave degree. `m = 0` is the linear mapping
`deg(k) = k - middle`. An unmapped reference key is a `type` failure at
construction.

Inline strings use the existing string escapes (`\n`), for example
`scala "! slendro\nslendro approximation\n5\n240.0\n480.0\n720.0\n960.0\n2/1\n"`.
Strings with `{` must escape it (`\{`), per the string literal rules.

### 4.5 Applying a tuning: `tune`

`tune p t` with optional keyword arguments `root:` (int key or note
keyword, converted by `note_number`), `ref-key:` (same), `ref-freq:`
(number > 0). Keyword arguments are read once at the call. `t` is a spec,
a preset keyword, or a pattern of them (`tune (alt [(edo 19) (edo 31)])`),
sampled at each subject event like any control value. An invalid spec in a
pattern is an event-local fault for that event.

Scope: the tuning applies to the events of the chain it is applied to,
which is per track (the chain bound to `d1`) and per instrument (put it
right after `s :inst`). Per event: a patterned `t`.

### 4.6 The `tuning` event control (internal form)

`tune` sets the event control `tuning` to a canonical list built from the
spec and the mapping parameters:

```
[:edo STEPS PERIOD ROOT REF-KEY REF-FREQ]
[:degrees [D1 .. DN] ROOT REF-KEY REF-FREQ KEYMAP]
KEYMAP = nil | [FIRST LAST OCTAVE-DEGREE [M0 .. Mm-1]]     (an `x` entry is nil)
```

`STEPS`, `ROOT`, `REF-KEY`, `FIRST`, `LAST`, `OCTAVE-DEGREE`, `Mi` are ints;
`PERIOD` and ratio entries are exact; cent entries and `REF-FREQ` are
floats. The form is internal (visible in telemetry), not a public API.

Recognition rule (backward compatibility): a `tuning` control is a tuning
if and only if its value is a list whose first item is the keyword `:edo`
or `:degrees`. Any other `tuning` value (for example a number for a user
`inst` with a `tuning` parameter) keeps today's meaning everywhere. A
recognized but invalid list is an event-local `type` failure.

The single Rust type `Tuning` decodes this form, so the pattern layer,
commit and song encode share one implementation and one set of rules.

### 4.7 Tuning-aware music combinators

All rules below apply only when the event carries a recognized `tuning`
control (D1); otherwise the existing code runs unchanged.

Nearest-key rule: `nearest(T, from, cents)` is the integer offset `o`
minimizing `|c(from + o) - c(from) - cents|` where `c(k) = 1200 * log2(f(k))`,
searched over `o` in `[o0 - K, o0 + K]` with `o0 = round(cents * K / Pc)`,
where `Pc = 1200 * log2(period)` (`:edo`) or the cents of the formal-octave
degree (`:degrees`: degree N, or the keymap's O); unmapped keys are skipped;
ties go to the smaller `|o|`, then the smaller `o`. If `K == 12` the rule
is not used: 12-tone offsets apply directly.

- Note names: a note-name keyword (in `note`/`n`, a chord root, a scale
  root) denotes key
  `ROOT + nearest(T, ROOT, 100 * (note_number(name) - ROOT))`, with ROOT
  the event tuning's root key; it is `note_number(name)` when `K == 12`.
  With the default ROOT 60 this is `60 + nearest(T, 60, ..)` (19-EDO:
  `:d` -> 63); with `root: :d` (ROOT 62) `:d` is key 62, the root itself.
  Numbers are keys as written. This single rule (`Tuning::note_key`) is
  used by every keyword-to-key site, listed in 4.9; "root key as above"
  in the `scale` and `chord` bullets means this rule.
- `scale p root name`: every preset has a native size P and period
  (P = 12, octave, for the 17 existing scales; section 4.8 for the new
  ones). Degree d: `oct = d div len`, `i = d mod len`. If `K == P`:
  `key = root-key + oct * K + steps[i]`. Otherwise, with `Sc` the preset's
  own period in cents (1200, or `1200 * log2(3)` for `:bp-lambda`),
  `key = root-key + nearest(T, root-key, oct * Sc + steps[i] * Sc / P)`.
  A new (non-12) preset applied to an event WITHOUT a tuning control first
  sets `tuning` to its own default tuning (its EDO, default mapping) and
  then uses `K == P`. The existing 17 scales never set a tuning.
- `chord`: root key as above; each 12-tone interval s gives `root + s` if
  `K == 12`, else `root + nearest(T, root, 100 * s)`.
- `voicing`: close position over the tuning period: with `pc(n) = (n - ROOT) mod K`,
  the first tone is `ROOT + pc` (in `[ROOT, ROOT + K)`), each next tone the
  lowest key above the previous one with its pc. With no tuning this is the
  existing `[60, 72)` / mod 12 rule.
- `arp`: order only; unchanged and tuning-independent.

### 4.8 Presets

Tuning presets (accepted by `tune` and by any spec position):

| Keyword | Spec |
| --- | --- |
| `:bohlen-pierce` | `edo 13 period: 3` |
| `:ji-5` | `ratios [16/15 9/8 6/5 5/4 4/3 45/32 3/2 8/5 5/3 9/5 15/8 2]` |
| `:ji-7` | `ratios [15/14 8/7 6/5 5/4 4/3 7/5 3/2 8/5 5/3 7/4 15/8 2]` |
| `:partch-43` | `ratios [81/80 33/32 21/20 16/15 12/11 11/10 10/9 9/8 8/7 7/6 32/27 6/5 11/9 5/4 14/11 9/7 21/16 4/3 27/20 11/8 7/5 10/7 16/11 40/27 3/2 32/21 14/9 11/7 8/5 18/11 5/3 27/16 12/7 7/4 16/9 9/5 20/11 11/6 15/8 40/21 64/33 160/81 2]` (43 degrees, symmetric about 1/1 and 2/1) |

Microtonal scale presets (new `scale` names; steps in the preset's own
EDO; P and period give its default tuning):

| Name | P | Period | Steps | Note |
| --- | --- | --- | --- | --- |
| `:edo19-major` | 19 | 2 | 0 3 6 8 11 14 17 | whole 3, half 2 |
| `:edo19-minor` | 19 | 2 | 0 3 5 8 11 13 16 | |
| `:edo31-major` | 31 | 2 | 0 5 10 13 18 23 28 | whole 5, half 3 |
| `:edo53-major` | 53 | 2 | 0 9 18 22 31 40 49 | whole 9, half 4 |
| `:maqam-rast` | 24 | 2 | 0 4 7 10 14 18 21 | quarter-tone approximation |
| `:maqam-bayati` | 24 | 2 | 0 3 6 10 14 16 20 | |
| `:maqam-saba` | 24 | 2 | 0 3 6 8 14 16 20 | |
| `:maqam-hijaz` | 24 | 2 | 0 2 8 10 14 16 20 | |
| `:slendro` | 5 | 2 | 0 1 2 3 4 | equal-pentatonic approximation |
| `:pelog` | 9 | 2 | 0 1 2 4 5 6 7 | 9-EDO approximation (small, small, large, small, small, small, large) |
| `:bp-lambda` | 13 | 3 | 0 2 3 4 6 7 9 10 12 | Bohlen-Pierce lambda mode |

These are approximations from general public-domain theory (equal
temperaments, the Arabic 24-tone quarter-tone notation, the equal
pentatonic slendro model), not measurements of a specific ensemble.

### 4.9 Resolution sites

Keyword-to-key conversion sites. Today three places turn a note-name
keyword into a key with `note_number`: `commit::note_of`
(`src/sched/commit.rs:49`, used by `notes()` for audio and by
`midi_events`), and song freezing (`src/song/source.rs:176-182`,
`ResolvedNote::Int(note_number(..))`). When the event carries a recognized
`tuning` control, each of these applies the 4.7 note-name rule
(`Tuning::note_key`) to keyword tones (scalar or chord list) instead;
numbers stay keys as written. Without a recognized tuning, all three keep
`note_number` unchanged (D1).

- Audio commit (`src/sched/commit.rs`): read the `tuning` control once per
  event; skip it in the generic control loop (it has no control row and is
  never an instrument parameter when recognized); per tone, keywords
  mapped as above, then
  `hz = match tuning { None => note_to_freq(n), Some(t) => t.freq(n) }`.
  Under a tuning a var-driven note is read at commit and sent as
  `Ctl::Const` (no `NoteToFreq` cell, which assumes 12-TET); TQ4. An
  explicit `freq` control still wins over notes, as today.
- Song freezing (`src/song/source.rs`): the keyword mapping happens HERE,
  so the frozen `ResolvedNote` is already the tuned key (encode only sees
  the frozen number, so it cannot remap a keyword). Live and song output
  therefore agree.
- Song encode (`src/sched/song/encode.rs`): the same recognition on the
  frozen `tuning` list; skip it in the control loop; `row.note` (already a
  key) goes through `t.freq`.
- MIDI (`midi_events`): no tuning: unchanged `n.round()`. Tuning: keyword
  tones are mapped as above first; the note sent is
  `round(69 + 12 * log2(f / 440))` clamped to 0..=127, where f is the
  tuned frequency; unmapped tones are skipped. Pitch deviations up to 50
  cents are lost; no pitch bend is sent (D6).
- OSC: unchanged; keys are sent as `note`, and the `tuning` list is
  skipped as every list control is today.

## 5. Chord performance combinators

All four work on events whose `note` control is a list (a chord, after
`chord`/`voicing`); events with a scalar note or no note pass through
unchanged. They are tuning-independent except where K is named (period
size: the event tuning's K, else 12). They drive existing instruments:
the result is ordinary note events for any template or MIDI.

### 5.1 `strum p time [dir [curve]]`

- `time`: exact non-negative number (int or ratio) of cycles between
  successive tones, patterned (sampled at the event anchor). A float is a
  `type` fault ("use a ratio such as 1/32").
- `dir`: `:up` (default; ascending key order), `:down`, `:alternate`,
  `:random`; patterned.
  - `:alternate`: with `len = whole.end - whole.begin`, up when
    `floor(whole.begin / len)` is even, else down (len 0: up).
  - `:random`: a permutation by Fisher-Yates driven by the pure hash RNG
    (design 10.3) keyed by the session seed, the node id and
    `whole.begin`; the same query gives the same order.
  - Ties in key order keep chord order (stable sort; keywords order by
    their note number).
- `curve`: `:flat` (default), `:fade` (`m_i = 1 - 0.5 * i / (N - 1)`),
  `:swell` (`m_i = 0.5 + 0.5 * i / (N - 1)`), i the play-order index,
  N the tone count (N = 1: m = 1). Applied to `velocity` if present, else
  to `gain` if present, else sets `gain = m_i`. `:flat` writes nothing.
- Timing: `dt = min(time, len / N)`; tone i gets
  `whole = [begin + i * dt, end)` (all tones end together),
  `part = intersection(whole, parent.part)`, dropped when empty. Each child carries one
  `note` (the original value), `occ` pushes `(node id, original chord
  index)` and the producer trace gets `GeneratedBranch` (as `arp`).
- An event with a list note and no whole is a `no-whole` fault.

### 5.2 `harp p pos` (strum-plate)

Keyword arguments (read once): `strips:` (int 1..=64, default 12),
`base:` (key or note keyword, default `ROOT - K`, which is 48 without a
tuning). The plate is the first `strips` keys `k >= base`, ascending, whose
`pc(k)` is the pc of some chord tone (pc as in 4.7; mod 12 from 60 without
a tuning). `pos` is a number, patterned and sampled at each event anchor
(signals such as `sine`/`saw` give sweeps); it is clamped to [0, 1] and
selects strip `min(floor(pos * strips), strips - 1)`. Each input event
becomes one event with that scalar `note`. NaN is a `type` fault.

Example: `[:c :maj]`, no tuning, defaults: plate 48 52 55 60 64 67 72 76 79
84 88 91; pos 0 -> 48, 0.5 -> 72, 1 -> 91.

### 5.3 `inversion p n`

`n` is an int, patterned (so `inversion p [0 1 2 1]` walks inversions).
The tones are sorted ascending; with `n = q * len + r` (Euclidean), rotate
up r times (remove the lowest, add it plus K) and then add `q * K` to
every tone; negative n therefore rotates down. Output is ascending.
Example: `[60 64 67]`: n 1 -> `[64 67 72]`, n -1 -> `[55 60 64]`,
n 3 -> `[72 76 79]`.

Internal bass flag (used by `perform bass: true`, not a public name):
the chord root is the first tone of the input list (the `chord`/`voicing`
convention); `bass = root - j * K` for the smallest j >= 1 with
`bass < min(output)`, prepended to the output.

### 5.4 `perform subject chords` (Orchid-style helper)

A native that composes existing and new nodes; no node of its own:

```
subject > chord chords > voicing > inversion INV [bass] > MODE
```

Keyword arguments: `mode:` (`:block` default, `:strum`, `:arp`, `:harp`;
read once), `inversion:` (patterned int, default 0), `bass:` (bool,
default false), `time:` `dir:` `curve:` (to `strum`; `time` default 1/32),
`arp:` (to `arp`, default `:up`), `pos:` (to `harp`, default the `saw`
signal), `strips:` `base:` (to `harp`). `chords` is read with the `chord`
rule (`[root quality]` is one chord; `chord_pattern_of`), and the static
checker applies the existing chord-literal check to it as it does for
`chord`. Root plus quality selection is the chord value; tempo comes from
the subject structure (`fast`, step lists), as for every pattern.

### 5.5 Pattern-graph integration

New `PatNode` variants for `tune`, `strum`, `harp` and `inversion`,
appended to the enum; hash tags appended after the current maximum
(42 at 9ac8d1f), never renumbered. Every exhaustive `PatNode` match gets
the new variants, classified like the nearest existing operator:

| Node | Like | Song source-use mapping | Song-clock dispatch |
| --- | --- | --- | --- |
| tune (value, subject) | `Chord` | `Restructure`/`SelectContent` by `gives_structure` | supported |
| inversion | `Voicing` | `Preserve` | supported |
| harp | `Voicing` | `Preserve` | supported |
| strum | `Arp` | `Preserve` | timing barrier (`with_clock_unknown`), because onsets move |

Song source-use operation enum values are appended, not renumbered.

## 6. Language surface

Appended to the native table (`src/types/natives_domain.rs`) and to the
registrations, at the end, so existing native ids and order are unchanged:

| Name | Arity | Type string | Keywords |
| --- | --- | --- | --- |
| `tune` | 2 | `fn (pattern 'a) any -> pattern 'a` | `root` `ref-key` `ref-freq` |
| `edo` | 1 | `fn int -> [keyword: any]` | `period` |
| `ratios` | 1 | `fn [any] -> [keyword: any]` | |
| `scala` | 1 | `fn str -> [keyword: any]` | `kbm` |
| `load-scala` | 1 | `fn path -> [keyword: any]` (effect) | `kbm` |
| `strum` | 2..4 | `fn (pattern 'a) any any any -> pattern 'a` | |
| `harp` | 2 | `fn (pattern 'a) any -> pattern 'a` | `strips` `base` |
| `inversion` | 2 | `fn (pattern 'a) any -> pattern 'a` | |
| `perform` | 2 | `fn (pattern 'a) any -> pattern 'a` | `mode` `inversion` `bass` `time` `dir` `curve` `arp` `pos` `strips` `base` |

These entries are the editor completion and hover metadata (the browser
completion builds its builtin snapshot from the same table). No editor
TypeScript or tree-sitter change is needed.

`lang-reference.md` gets one appended section documenting every name above,
the presets, the ordering rule (section 3) and the MIDI limitation; its
`vactr` fences must read and check without abort (existing tests).

## 7. Realtime safety and performance

- No audio-thread code changes. `Tuning` decoding allocates only during
  pattern queries and on the scheduler (commit, song encode).
- With no tuning control the added cost is one control lookup per event in
  the tuning-aware combinators and at commit.
- Editor code is untouched, so canvas editor performance cannot regress.

## 8. Licensing and provenance

- vactr is MIT. FM-1 firmwares (Felucca, SLOOP, sloopDX, FoMni, ChoralRoot,
  Melodee, FiMba-1, GHOULBOX, Jangada and forks) are GPL-3.0; only their
  feature descriptions (Melodee: many scales addressed as degrees; FoMni /
  ChoralRoot: strum, harp and chord performance) motivated this design.
  No firmware code, table or patch data was read or used.
- Tables: EDO steps are computed; JI ratios (5-limit, 7-limit, Partch's 43
  ratios), maqam quarter-tone positions, slendro/pelog approximations and
  the Bohlen-Pierce scale are public-domain music theory; preset selection
  and naming are vactr's own.
- Scala `.scl`/`.kbm`: implemented from the public format description;
  no Scala code.
- choralroot (Lua) and Chordian are not used (D8), so
  `THIRD_PARTY_NOTICES.md` needs no entry. If a later change uses either,
  it must verify the license and add the notice in the same change.

## 9. File layout and shared-file policy

New files (proposed; each under 1000 lines):

- `src/pattern/tuning/mod.rs`: `Tuning`, spec parsing and validation,
  canonical control encode/decode, `freq`, `nearest`, `keys_per_period`.
- `src/pattern/tuning/scala.rs`: `.scl`/`.kbm` parsing.
- `src/pattern/tuning/presets.rs`: tuning presets and microtonal scale presets.
- `src/pattern/combinators/tune.rs`: `tune` constructor and query.
- `src/pattern/combinators/strum.rs`: `strum`, `harp`, `inversion`.
- `src/vm/natives/tuning.rs`: the nine natives.
- Tests: `src/pattern/tests/tuning.rs`, `src/pattern/tests/strum.rs`,
  `src/sched/tests/tuning.rs` (commit audio/MIDI and song encode).
- `examples/microtonal-tuning.vact`, `examples/strum-harp.vact` (inline
  `scala` only, so they run on every host).
- `tests/fixtures/tuning/slendro.scl`, `tests/fixtures/tuning/slendro.kbm`
  (for the `load-scala` test).

Modified (existing files): `src/pattern/mod.rs`,
`src/pattern/combinators/mod.rs`, `src/pattern/combinators/music.rs`,
`src/pattern/pat.rs`, `src/pattern/query.rs`,
`src/pattern/combinators/input.rs`, `src/pattern/eval/song_clock/dispatch.rs`,
`src/ns/checked_callable.rs`, `src/song/assets.rs` (965 lines: the new arms
join existing or-patterns and must keep it under 1000),
`src/song/source_uses.rs`, `src/session/song/source_uses.rs`,
`src/session/song/shape_preparation.rs`, `src/sched/commit.rs` (873 lines),
`src/sched/song/encode.rs`, `src/song/source.rs` (972 lines: the keyword
mapping is a single helper call, keeping it under 1000),
`src/vm/natives/mod.rs` or
`src/vm/natives/music.rs` (registration), `src/types/natives_domain.rs`,
`src/types/infer_call.rs` (`perform` chord-literal check),
`design-docs/specs/lang-reference.md`. Exact per-task writePaths are fixed
by the implementation plans.

Shared with the sibling run (append-only edits, no reordering):
`src/types/natives_domain.rs`, native registration, `lang-reference.md`,
`design-music.md`, `design-docs/user-qa/README.md`. Not touched:
`src/dsp/ugen/catalog.rs`, `src/dsp/ugen/catalog/codec.rs`,
`src/prelude/templates.vact`, `TEMPLATE_NAMES`, `src/dsp/controls.rs`,
`golden_digests.txt`, `THIRD_PARTY_NOTICES.md`.

## 10. Verification

Behavioral tests (all silent; no audio device):

1. Bit identity: for keys including fractional values, an event without a
   tuning commits `freq == note_to_freq(n) as f32` (bit equality) and MIDI
   notes equal `n.round()`; the 17 scales, `chord`, `voicing` and `arp`
   produce the same events as before for untuned events (existing tests
   stay green); `git diff --exit-code src/host/tests/e2e/templates/golden_digests.txt`.
2. Analytic frequencies (section 4.3) for 19-EDO, 31-EDO, Bohlen-Pierce,
   a JI ratio list, an inline `.scl` with cents and ratios, an inline
   `.kbm` (including an `x` key that yields no tone), and a loaded `.scl`
   through a fixture `SourceLoader`: f64 relative error <= 1e-12, committed
   `f32` within 1 ulp of the analytic value rounded to `f32`; `ref-key: 69
   ref-freq: 440` gives exactly 440.
3. Parser negatives: `.scl` count mismatch, bad value, zero period;
   `.kbm` unmapped reference; spec validation bounds.
4. Tuning-aware music: `scale` with `K == P`, with mapping across sizes,
   and a microtonal preset with no `tune` (sets its tuning); `chord` and
   `voicing` in 19-EDO and in `:ji-5`; note names under 19-EDO (`:d` -> 63);
   non-default root: under `tune (edo 19) root: :d`, `:d` is key 62 and
   sounds at `note_to_freq(62)` (also as a `scale :d` and `chord` root).
5. Bare keyword note on every path: `s :pd > tune (edo 19) > note :d`
   commits key 63 (`note_to_freq(60) * 2^(3/19)`) on the audio commit path,
   sends the nearest MIDI note of that frequency on the MIDI path, and
   freezes `ResolvedNote` 63 and encodes the same frequency on the song
   path; untuned `note :d` stays key 62 on all three.
6. Song encode: a frozen tuned row encodes the tuned freq.
7. MIDI: tuned nearest note; untuned unchanged.
8. `strum`: onsets, shared end, `dt` clamp, each direction including
   `:alternate` parity and `:random` determinism and permutation, curves
   on `velocity`/`gain`/absent, `no-whole` fault, pass-through.
9. `harp`: plate contents, pos 0 / 0.5 / 1, clamping, tuned plate in 19-EDO.
10. `inversion`: positive, negative, multi-period, bass flag.
11. `perform`: each mode yields the expected notes and onsets, and drives
    an existing template (`s :pd`, `s :fm`) to committed audio events with
    tuned frequencies under `tune (edo 19)`.
12. Examples are formatter fixed points and pass the existing corpus,
    LSP and spec-fence tests.

Gates (serially, heavy suites under the measurement lock): `cargo build`,
strict `cargo clippy`, full `cargo nextest run`, wasm builds
(`release-wasm` and `--lib`), `npm run check`, `vitest`, frontend build,
`test:style`. Commands use `CARGO_TERM_QUIET=true` and the nextest
environment from `AGENTS.md`.

## 11. Rollout constraints for the implementation plans

1. The `Tuning` model, Scala parser and presets (pure, unit-tested) come
   first; nothing else depends on later work.
2. `tune` with the commit, MIDI, song-freeze and song-encode resolution
   (including the keyword-to-key sites of 4.9) next; the bit-identity
   tests (10.1) and the bare-keyword tests (10.5) must be green before any
   later plan.
3. Tuning-aware `scale`/`chord`/`voicing` and the microtonal presets.
4. `strum`, `harp`, `inversion` (independent of 3), then `perform`.
5. Natives, type table, `lang-reference.md` and examples last for each
   name, with the full behavioral suite green before each acceptance.

## 12. Open questions

None open. The owner decided all four on 2026-10-10 as recommended
([pending-tuning-questions.md](../user-qa/pending-tuning-questions.md)):
TQ1 (a) nearest 12-TET MIDI note, no pitch bend or MPE (D6, 4.9); TQ2 (a)
the root key keeps its 12-TET pitch (D4, 4.2); TQ3 12 strips starting one
period below the root (5.2); TQ4 yes, tuned var-driven notes are read at
commit and sent as constants (4.9).

## References

- Scala scale file format, Huygens-Fokker Foundation:
  https://www.huygens-fokker.org/scala/scl_format.html
- Scala keyboard mapping (`.kbm`) description, Scala help, "Mappings":
  https://www.huygens-fokker.org/scala/help.htm#mappings
- H. Partch, *Genesis of a Music*, 1949 (the 43-tone ratio set).
- H. Bohlen, "13 Tonstufen in der Duodezime", *Acustica* 39 (1978);
  M. Mathews, J. Pierce et al., "Theoretical and experimental explorations
  of the Bohlen-Pierce scale", JASA 84 (1988).
- `design-music.md` section 3 (patterns, chords, scales); design 10.1
  (first-structure rule) and 10.3 (pure hash RNG).
