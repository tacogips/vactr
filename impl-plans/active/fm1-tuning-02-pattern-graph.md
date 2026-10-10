# FM1 Tuning 02: Pattern Graph Nodes for tune, strum, harp, inversion

**Status**: Ready
**Plan ID**: fm1-tuning-02-pattern-graph
**Wave**: 2 (depends on fm1-tuning-01-model)
**Design Reference**: design-docs/specs/design-tuning-and-strum.md sections 3, 4.5, 4.6, 5.1-5.3, 5.5
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

This plan adds the four new pattern operators as `PatNode` variants, plus
their query semantics:

- `tune` sets the `tuning` event control.
- `strum` spreads chord tones in time.
- `harp` picks one strip of a chord-tone plate.
- `inversion` rotates chord tones, with an internal bass flag.

Every exhaustive `PatNode` match in the crate gets the new variants, so this
plan owns all of those files. It does NOT register natives (plan 05) and
does NOT change `scale`/`chord`/`voicing` (plan 04).

The tuning model comes from plan 01 (`crate::pattern::tuning::{Tuning, Mapping}`).

## Non-goals

- No natives, type-table entries, docs or examples.
- No change to `music.rs`, `commit.rs`, `song/source.rs` or `song/encode.rs`.
- Live MIDI input lanes (`src/pattern/combinators/input.rs`) do not support
  the new operators. Only add them to the exhaustive `contains_lane` match.
  `walk` keeps its `_ =>` reject ("cannot re-time live input").
- No `bass` public function. The bass flag only lives on the Inversion node.

## Working Rules

Follow the Working Rules in `fm1-tuning-01-model.md`: fresh read plus
sha256 before and after each edit, re-apply on drift, own paths only, no
git writes, no broad formatting, silent tests, no full nextest. Plans 03
and 04 run concurrently on disjoint files.

## Files

| Path | Change |
| --- | --- |
| `src/pattern/pat.rs` | append 4 variants to `PatNode`; append hash arms with tags 43-46 in `hash_pat` |
| `src/pattern/query.rs` | 4 dispatch arms |
| `src/pattern/combinators/mod.rs` | `pub mod tune; pub mod strum;` |
| `src/pattern/combinators/tune.rs` | new: `tune` constructor, `query_tune` |
| `src/pattern/combinators/tune/tests.rs` | new tests (`#[cfg(test)] mod tests;` in tune.rs) |
| `src/pattern/combinators/strum.rs` | new: `strum`, `harp`, `inversion` constructors and query fns |
| `src/pattern/combinators/strum/tests.rs` | new tests |
| `src/pattern/combinators/input.rs` | `contains_lane` arms only |
| `src/pattern/eval/song_clock/dispatch.rs` | Strum -> `true` (barrier); Tune, Harp, Inversion -> `false` |
| `src/ns/checked_callable.rs` | walk arms (imitate the Chord / Arp / Voicing arms at :437-:480) |
| `src/song/assets.rs` | walk arms; join existing or-patterns (file is 965 lines, must stay < 1000) |
| `src/song/source_uses.rs` | append `Tune, Strum, Harp, Inversion` to `FrozenUseOperation` (append only) |
| `src/session/song/source_uses.rs` | classification (table below) |
| `src/session/song/shape_preparation.rs` | walk children and pattern params like the analog nodes |
| `src/session/song/freeze.rs` | song-freeze reconstruction arms for the 4 variants (719 lines; `use PatNode::*` glob matches) |
| `src/session/song/source_uses/timing/capture.rs` | timing-capture arms for the 4 variants (644 lines; `use PatNode::*` glob matches) |

Inventory check: `grep -rln MidiNotes src` lists exactly the exhaustive-match
files (pat.rs, query.rs, input.rs, song_clock/dispatch.rs, checked_callable.rs,
song/assets.rs, song/source_uses.rs, session/song/source_uses.rs,
session/song/freeze.rs, session/song/source_uses/timing/capture.rs). Two of
them, freeze.rs and capture.rs, use glob-imported variant names (`Arp(..)`
without the `PatNode::` prefix), so a `PatNode::Arp` grep misses them. All
ten files plus shape_preparation.rs are in this table. If the compiler
reports a non-exhaustive match in any other file, stop and record it in the
progress log as an inventory defect. Do not fix it outside this table.

## Contract (pinned; plan 05 calls these)

```rust
// PatNode variants (append after MidiNotes)
Tune { tunings: Rc<Pat>, subject: Rc<Pat>, mapping: Mapping },
Strum(Rc<Pat>, PParam /*time*/, PParam /*dir*/, PParam /*curve*/),
Harp { subject: Rc<Pat>, pos: PParam, strips: i64, base: Option<i64> },
Inversion { subject: Rc<Pat>, n: PParam, bass: bool },

// src/pattern/combinators/tune.rs
pub fn tune(tunings: Rc<Pat>, subject: Rc<Pat>, mapping: Mapping, span: Option<Span>) -> Pat;
// src/pattern/combinators/strum.rs
pub fn strum(p: Rc<Pat>, time: PParam, dir: PParam, curve: PParam, span: Option<Span>) -> Pat;
pub fn harp(p: Rc<Pat>, pos: PParam, strips: i64, base: Option<i64>, span: Option<Span>) -> Result<Pat, Failure>;
pub fn inversion(p: Rc<Pat>, n: PParam, bass: bool, span: Option<Span>) -> Pat;
```

`Mapping` must hash in `hash_pat`: hash `Option` presence, then the i64
values and `f64::to_bits` for `ref_freq`. `PatNode` derives only
`Clone, Debug` (`src/pattern/pat.rs:49`); plan 01's `Mapping` already
derives both. Do not edit `src/pattern/tuning/*`.

Structure flags (mirror `music.rs:chord` and `music.rs:voicing`):

- Tune: `subject.structured || tunings.structured`
- Strum, Harp, Inversion: `subject.structured`

## Semantics (design section 5; the key pitfalls)

- **query_tune**: use `control::query_mapped(kw("tuning"), tunings, subject, ...)`.
  The map is `Tuning::from_spec(v)?.with_mapping(&mapping)?.to_control()`.
  An invalid spec value is an event-local fault through `query_mapped`'s
  `Err` path. Do not use `query_control` (it would store the dict, which
  cannot freeze into song mode).
- **Period and root**: when an event carries a recognized tuning control
  (`Tuning::from_control` on `e.controls["tuning"]`), K is
  `keys_per_period()` and ROOT is `root()`. Otherwise K = 12 and ROOT = 60.
  A `Some(Err)` from `from_control` is an event fault.
- **Strum**:
  - Only events whose `note` control is a `Value::List` are affected; other
    events pass through unchanged.
  - No whole -> `no_whole()` fault (`combinators/mod.rs:no_whole`).
  - `time` must be Int or Ratio and >= 0; a Float is a Type fault with the
    message containing "use a ratio such as 1/32".
  - `dir` is one of `:up :down :alternate :random`; `curve` is one of
    `:flat :fade :swell`. Both are evaluated with `eval_param` at
    `e.anchor()` (imitate `music.rs:query_arp`).
  - Order: stable sort by numeric key (keywords via `note_number`); `:down`
    reverses it.
  - `:alternate`: `len = whole.end - whole.begin`; up iff
    `floor(whole.begin / len)` is even; len 0 -> up.
  - `:random`: Fisher-Yates with `crate::pattern::rng::below(st.cx.seed, p.id, whole.begin.floor(), seq, i + 1)`
    and `seq = Hasher::new(0x7374_7275).ratio(whole.begin).word(i).finish()`.
  - `dt = min(time, len / N)` with exact `Ratio64` arithmetic.
  - Child i: `whole = [begin + i*dt, end)` and
    `part = intersection(whole, e.part)` (use `crate::pattern::query::sect`); drop the
    child if the intersection is empty.
  - Each child keeps the parent's controls with `note` replaced by the
    single original tone `Value`. Do `child.occ.push(p.id, original_index)`
    and push `ProducerKind::GeneratedBranch` (copy from `query_arp`).
  - Curve multiplier `m_i` (design 5.1) applies to `velocity` if present,
    else to `gain` if present, else sets `gain`. `:flat` writes nothing.
- **Harp**:
  - Validate `strips` in 1..=64 in the constructor (Type failure otherwise).
  - `base` defaults to `ROOT - K` per event.
  - The plate is the first `strips` keys `k >= base`, ascending, whose
    `(k - ROOT).rem_euclid(K)` equals some chord tone's pc.
  - `pos` is evaluated per event, must be numeric, and NaN is a Type fault.
    Clamp it to [0, 1]; strip = `min(floor(pos * strips), strips - 1)`.
  - Output `note` = that Int key. Events without a list note pass through.
- **Inversion**:
  - `n` must be an Int.
  - Sort the tones ascending. With `n = q*len + r` (Euclidean), rotate up r
    times (remove the min, push min + K), then add `q*K` to every tone.
    Output is ascending.
  - If `bass`: root = the FIRST tone of the input list (before sorting);
    `bass = root - j*K` with the smallest j >= 1 such that
    `bass < min(output)`. Prepend it.
  - Keyword tones are converted with `note_number`, or with
    `Tuning::note_key` when the event is tuned.
- Do not touch the event `tuning` control in strum/harp/inversion; children
  inherit it.

Song freeze (`src/session/song/freeze.rs`). Every new variant must get an
explicit arm in each dispatcher. Several of them have a catch-all that
reports "freeze ... dispatch invariant" or collects no children, so a
missing arm would silently break song mode.

- `Freeze::pat` (:454, exhaustive): route Tune to `pat_complex` (next to
  `Chord(..)` at :482). Route Strum, Harp and Inversion to `pat_unary`
  (next to `Arp(..)` at :489).
- `pat_complex` (:556): imitate `Chord(p, q)` at :582. Rebuild `tunings`
  and `subject` with `self.pattern(..)` and copy `mapping` (it is `Copy`).
- `pat_unary` (:531): imitate `Arp(p, a)` at :550. Rebuild the subject with
  `self.pattern(..)`, pass each `PParam` through `self.param(..)`, and copy
  `strips`, `base` and `bass` as is.
- `finish_unary` (:212): imitate `Arp(_, a) => Arp(child, self.param(a)?)`
  at :231 for Strum, Harp and Inversion.
- `unary_child` (:621): return the subject for Strum, Harp and Inversion
  (like `Arp(p, _)` at :640).
- `children` (:673): Strum, Harp and Inversion give `[subject]` (like `Arp`
  at :699); Tune gives `[tunings, subject]` (like `Chord(p, q)` at :706).

Song timing capture (`src/session/song/source_uses/timing/capture.rs`):

- Param collection match (~:296-:310): Strum adds its three params. Harp
  adds `pos`. Inversion adds `n`. Imitate `Arp(_, k)` at :296 and
  `Range(_, a, b)` at :304.
- Child match (~:323-:389, exhaustive): Strum, Harp and Inversion go in the
  unary group (with `Arp(p, _)` / `Voicing(p)` at :355-:361). Tune goes
  with `Chord(p, q)` at :389.
- Node capture match (~:462-:565): Strum, Harp and Inversion follow the
  `self.param(..)` plus `self.unary(..)` shape used for `Arp(p, k)` at :503
  and `Range`/`Euclid` at :557. Tune goes in the two-child arm with
  `Grid(p, q) | Control(_, p, q) | Chord(p, q)` at :565.

Do not add `_ =>` catch-alls to an exhaustive match. Do not change any
existing arm.

Song classification (`src/session/song/source_uses.rs`):

| Node | Operation | Mapping |
| --- | --- | --- |
| Tune | `O::Tune` | the Control/Chord arm: `Restructure {timing 0, content 1}` if `gives_structure(subject, tunings)`, else `SelectContent {content: 1}` |
| Strum | `O::Strum` | `M::Preserve` (next to Arp at :319) |
| Harp | `O::Harp` | `M::Preserve` |
| Inversion | `O::Inversion` | `M::Preserve` (unary like Voicing at :86) |

## Tests

`tune/tests.rs` (use the helpers in `src/pattern/tests/mod.rs`: `run`,
`ctl_of`, `s`, `kw`, `cycle`, `konst`). Tuning dicts and canonical lists
in tests go through `konst` (pure). Never use `ctl(..)` or `pat(..)` on a
list, because `pattern_of` splits lists into steps. Strum/harp/inversion
tests build a chord event with
`control(intern_kw("note"), konst(list), subject, None)`, where `control`
is the combinator `crate::pattern::combinators::control::control`.

- a `tune` node over `s :x` with `konst(<edo 19 spec dict>)` -> each event's `tuning` control equals `Tuning::edo(19, 2).to_control()`
- a patterned tunings value (two dicts in steps) -> alternating controls per step
- an invalid dict (steps 0) -> the event is dropped with one fault
- `mapping.ref_key = 69, ref_freq = 440` -> encoded in the control (decodes back with those values)
- an unstructured subject plus a pure dict -> the subject structure is unchanged (no new onsets)

`strum/tests.rs`:

- chord [60 64 67] on a 1-cycle event, `strum 1/32 :up` -> onsets 0, 1/32, 2/32; all wholes end at 1; notes 60, 64, 67
- `:down` -> 67, 64, 60
- `:alternate` on four 1/4 events -> up, down, up, down
- `:random` -> same order on two queries; the result is a permutation
- clamp: 3 tones, event length 1/4, time 1/2 -> dt = 1/12
- `:fade` with no gain -> gain 1, 0.75, 0.5; with velocity 0.8 -> 0.8, 0.6, 0.4
- float time 0.1 -> Type fault containing "ratio"
- a scalar note passes through unchanged
- harp [60 64 67] untuned, defaults -> plate 48 52 55 60 64 67 72 76 79 84 88 91; pos 0 -> 48, 0.5 -> 72, 1 -> 91, 1.7 -> 91
- harp under an edo 19 tuning control -> the plate uses K = 19 and base `60 - 19`
- harp strips 0 -> constructor Err(Type)
- inversion [60 64 67]: n 1 -> [64 67 72]; n -1 -> [55 60 64]; n 3 -> [72 76 79]; bass with n 0 -> [48 60 64 67]; bass with n 1 -> [48 64 67 72]
- patterned n `[0 1 2 1]` -> walks per step

## Verification

Evidence directory: `tmp/fm1-tuning/p02/`.

| Command | Must show |
| --- | --- |
| `CARGO_TERM_QUIET=true cargo build > tmp/fm1-tuning/p02/build.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib combinators::tune combinators::strum > tmp/fm1-tuning/p02/nextest-focused.log 2>&1` | exit 0, failureCount 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib pattern:: song:: session:: ns:: > tmp/fm1-tuning/p02/nextest-regression.log 2>&1` | exit 0 (existing pattern/song/session behavior unchanged) |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/fm1-tuning/p02/clippy.log 2>&1` | exit 0 |
| `rustfmt --edition 2021 --check` on every file in the Files table | exit 0 |
| `rustfmt --edition 2021 --check src/session/song/freeze.rs src/session/song/source_uses/timing/capture.rs` | exit 0 (explicitly included in the rustfmt gate) |
| `wc -l src/song/assets.rs src/ns/checked_callable.rs src/session/song/source_uses.rs src/session/song/shape_preparation.rs src/session/song/freeze.rs src/session/song/source_uses/timing/capture.rs src/pattern/combinators/strum.rs` | each < 1000 |

Invariant check: all existing hash tags 0-42 are unchanged
(`git diff src/pattern/pat.rs` shows only additions).

## Completion Criteria

- [ ] Four variants appended; hash tags 43-46; no existing arm changed
- [ ] Every exhaustive match compiles with explicit new arms (no new `_ =>` added to an exhaustive match), including `src/session/song/freeze.rs` and `src/session/song/source_uses/timing/capture.rs`
- [ ] Every freeze.rs dispatcher (`pat`, `pat_complex`, `pat_unary`, `finish_unary`, `unary_child`, `children`) has explicit arms for the variants it handles. Plan 05's `tests/song_tuning.rs` strum and tune cases exercise these arms end to end
- [ ] All listed tests pass; the regression filter is green
- [ ] clippy, rustfmt and line limits pass; only the Files-table paths changed
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Implementation not started
