# FM1 Tuning 01: Tuning Model, Scala Parser and Presets

**Status**: Completed
**Plan ID**: fm1-tuning-01-model
**Wave**: 1 (no dependencies)
**Design Reference**: design-docs/specs/design-tuning-and-strum.md sections 4.1-4.8, 4.9 (keyword sites use `note_key`), 8
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

The user wants microtonal tuning (n-EDO, non-octave EDO such as Bohlen-Pierce,
just-intonation ratio lists, Scala `.scl`/`.kbm`) in vactr. This plan builds
the pure model that every later plan uses: one `Tuning` type that parses
specs, validates them, encodes/decodes the canonical `tuning` event control,
resolves key -> frequency, and implements the nearest-key and note-name rules.
It is pure Rust with no I/O and no pattern-graph changes, so it can be unit
tested alone.

Today note -> Hz is only `crate::sched::commit::note_to_freq`
(`src/sched/commit.rs:43`). Note names come from
`crate::pattern::combinators::music::note_number` (`:c` = 60).

## Non-goals

- No change to `src/sched/commit.rs`, `music.rs`, `PatNode`, natives or docs.
- No reading of files (the `load-scala` native in plan 05 reads text and calls
  this plan's parser).
- No GPL firmware material. Every table comes from the design (section 4.8),
  which takes it from public-domain theory. Do not look anything up in FM-1
  firmware sources.
- No `Value` variant, no control-table row, no codec tag.

## Working Rules (same branch and directory as parallel workers)

- Before each edit, re-read the file and record `shasum -a 256 <file>` in
  the progress log. After the edit, record it again.
- If the hash does not match your last read, re-read and re-apply only
  your intended change. Never revert another plan's lines.
- Edit only the paths in this plan's Files table, plus your own progress
  log.
- No `git` writes (no commit, stash, checkout or reset), no worktrees, no
  broad formatting (`cargo fmt` without paths), and no lockfile changes.
- Tests are silent. Never change the macOS volume. Do not run the full
  nextest or any browser command (plan 07 does that under the measurement
  lock).

## Starting Point (run 2, session-351)

A previous attempt (session-348) was killed by a backend stream hang. Its
uncommitted partial code is saved as
`tmp/fm1-tuning/p01-wip-session348.patch`. It is a 709-line patch, with
about 679 added lines. It touches exactly the five Files-table paths below
and contains 13 `#[test]` functions. Treat it as UNTRUSTED draft code. The
session-348 logs in `tmp/fm1-tuning/p01/` (`build.log`, `nextest-focused.log`
and the `*-retry-01.log` files) show 8 compile errors on the first build
(for example E0507 at `tuning/mod.rs:372`, moving a non-`Copy` `Pitch` out
of a reference) and a failing assertion (`left: 1, right: 0`) on the last
focused run. They are stale and are NOT evidence for this run.

Choose exactly one path and record the choice in the Progress Log:

1. Apply the patch: run `git apply --check tmp/fm1-tuning/p01-wip-session348.patch`
   and then `git apply tmp/fm1-tuning/p01-wip-session348.patch`. Apply to
   the working tree only, with no `--index` and no `--3way`. Do this only
   while `git status --porcelain -- src/pattern` is empty. Then review
   every line against the Contract and Key rules below. Fix the compile
   errors, add any missing tests from the Tests list, and remove anything
   outside the Contract.
2. Rewrite from scratch, if the check fails or the draft diverges from the
   Contract. You may read the patch for ideas, but the Contract and the
   design are the source of truth.

Either way, the result is verified like new code: every Tests bullet
exists and passes, and every gate below runs fresh into
`tmp/fm1-tuning/p01/run2/`.

## Files

| Path | Change |
| --- | --- |
| `src/pattern/mod.rs` | add one line `pub mod tuning;` next to the other `pub mod` lines |
| `src/pattern/tuning/mod.rs` | new: `Tuning`, `Mapping`, spec parsing/validation, control codec, resolution |
| `src/pattern/tuning/scala.rs` | new: `.scl` and `.kbm` text parsers producing spec dicts |
| `src/pattern/tuning/presets.rs` | new: tuning presets and microtonal scale presets |
| `src/pattern/tuning/tests.rs` | new: unit tests (`#[cfg(test)] mod tests;` in `mod.rs`) |

Each file must stay under 1000 lines (target: mod.rs under 600).

## Contract (pinned; plans 02-05 depend on these exact names)

```rust
// src/pattern/tuning/mod.rs
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mapping { pub root: Option<i64>, pub ref_key: Option<i64>, pub ref_freq: Option<f64> }

#[derive(Clone, Debug, PartialEq)]
pub struct Tuning { /* private fields */ }

impl Tuning {
    pub fn from_spec(spec: &Value) -> Result<Tuning, Failure>;        // dict or preset keyword
    pub fn edo(steps: i64, period: Ratio64) -> Result<Tuning, Failure>; // default mapping
    pub fn with_mapping(self, m: &Mapping) -> Result<Tuning, Failure>;
    pub fn to_control(&self) -> Value;                                 // canonical list, design 4.6
    pub fn from_control(v: &Value) -> Option<Result<Tuning, Failure>>; // None = not recognized
    pub fn keys_per_period(&self) -> i64;                              // K
    pub fn root(&self) -> i64;                                         // ROOT
    pub fn period_cents(&self) -> f64;                                 // Pc
    pub fn freq(&self, key: f64) -> Result<Option<f64>, Failure>;     // None = unmapped key
    pub fn nearest(&self, from: i64, cents: f64) -> Result<i64, Failure>; // offset o
    pub fn note_key(&self, name: &str) -> Result<Option<i64>, Failure>;   // None = not a note name
}
pub fn edo_spec(steps: &Value, period: Option<&Value>) -> Result<Value, Failure>;
pub fn ratios_spec(list: &Value) -> Result<Value, Failure>;
pub const TUNING_CONTROL: &str = "tuning";

// src/pattern/tuning/scala.rs
pub fn scala_spec(scl: &str, kbm: Option<&str>) -> Result<Value, Failure>;

// src/pattern/tuning/presets.rs
pub struct ScalePreset { pub name: &'static str, pub size: i64, pub period: (i64, i64), pub steps: &'static [i64] }
pub fn tuning_preset(name: &str) -> Option<Value>;   // spec dict, design 4.8 table 1
pub fn scale_preset(name: &str) -> Option<&'static ScalePreset>; // design 4.8 table 2 (11 entries)
```

`Value` is `crate::value::value::Value`, `Failure` is `crate::vm::fail::Failure`
(use `FailCode::Type` for every validation failure), and `Ratio64` is
`crate::value::ratio::Ratio64`.

Spec dict shapes (keys are keywords, built with `Value::dict` and `Key::Kw`,
see `src/value/value.rs:95` and `src/value/key.rs:149`):

- `{kind: :edo steps: N period: P}`
- `{kind: :degrees degrees: [..] description: "..." keymap: {...}}`, where
  `description` and `keymap` are optional
- keymap dict: `{first: a last: b middle: c ref-key: d ref-freq: f octave-degree: o map: [m0 .. or nil]}`.
  `map` is empty for kbm size 0 (linear).

Canonical control list (design 4.6, exact order):
`[:edo STEPS PERIOD ROOT REF-KEY REF-FREQ]` or
`[:degrees [D1..DN] ROOT REF-KEY REF-FREQ KEYMAP]`, with
`KEYMAP = nil | [FIRST LAST OCTAVE-DEGREE [M0..]]` and `x` entries as `nil`.
Ints are `Value::Int` (or `Int64` when out of i32 range; use the
`exact_value(Ratio64::from_int(..))` idiom from
`src/pattern/combinators/music.rs:note_value`). Ratio entries are exact
(`Ratio`/`Int`), cent entries and REF-FREQ are `Value::Float64`.

## Key rules (do not get these wrong)

1. Default mapping: `root = 60`, `ref_key = root`, and
   `ref_freq = crate::sched::commit::note_to_freq(ref_key as f64)`. CALL
   that function; do not copy the expression. A keymap supplies
   middle/ref-key/ref-freq as the defaults; `with_mapping` overrides each
   `Some` field individually.
2. EDO frequency: `ref_freq * exp2((k - ref_key) * log2(P) / steps)`. When
   P == 2, use exactly `ref_freq * ((k - ref_key) / steps).exp2()` so that
   it matches the analytic oracle bit-for-bit. Fractional keys use the same
   closed form.
3. Degrees: `R(d) = P^(d div N) * r[d mod N]` (Euclidean div/rem), with
   `r[0] = 1` and cents `c` giving `(c / 1200).exp2()`; then
   `f = ref_freq * R(deg(k)) / R(deg(ref_key))`. A fractional key
   interpolates in log frequency between floor and floor+1. If either
   neighbor is unmapped, the result is `Ok(None)`.
4. Keymap: offset = k - middle; `deg = (offset div m) * O + map[offset mod m]`
   (O = octave-degree, 0 means N). Keys outside `[first, last]` or with a
   `nil` entry are unmapped (`Ok(None)`). An unmapped reference key is a
   `Type` failure at construction.
5. K: `steps` for EDO, N for degrees without a keymap, map length m with a
   keymap (m = 0 means K = N).
6. `nearest(from, cents)`: minimize `|c(from+o) - c(from) - cents|`, where
   `c(k) = 1200 * log2(f(k))`, over `o` in `[o0 - K, o0 + K]` with
   `o0 = round(cents * K / Pc)`. Skip unmapped keys. Break ties by smaller
   `|o|`, then smaller `o`. An unmapped `from` is a `Type` failure.
7. `note_key(name)`: `n = note_number(name)?`. If K == 12 return `n`; else
   return `ROOT + nearest(ROOT, 100 * (n - ROOT))` (DR-2, anchored at
   ROOT).
8. `from_control` recognizes a value ONLY if it is a list whose first item
   is the keyword `:edo` or `:degrees`. For anything else return `None`, so
   numbers stay ordinary user-instrument parameters. A recognized but
   malformed list returns `Some(Err(Type))`.
9. Validation (design 4.1): `steps` 1..=1200; `period` exact (Int/Ratio)
   and > 1; degrees count 1..=1024; ratio entries > 0; cents finite; the
   period (last entry) > 0 cents. Non-ascending entries are allowed. Any
   result frequency that is non-finite or <= 0 is a `Type` failure.
10. Scala (design 4.4):
    - `!` lines are comments. The first non-comment line is the
      description. The next is the count. A pitch value ends at the first
      whitespace; containing `.` means cents (sign allowed); otherwise
      `a/b` or `a`.
    - Accept `\r\n`. Ignore trailing text after the value and lines after
      the N pitches.
    - Fewer lines than N, N = 0, or a malformed value is a `Type` failure.
    - kbm: size, first, last, middle, ref key, ref freq, octave degree,
      then up to m entries (`x` = unmapped; missing entries are unmapped).
11. Presets: copy the design 4.8 tables exactly (Partch: 43 entries; `:ji-5`
    and `:ji-7`: 12 each; `:bohlen-pierce` = edo 13 period 3). The scale
    presets have exactly the 11 rows of the table, with `period` (2,1) or
    (3,1) for `:bp-lambda`.

Imitate: the error style of `src/pattern/combinators/music.rs:type_err`, the
doc-comment density of `music.rs`, and the dict building in
`src/vm/natives/dict.rs`.

## Tests (`src/pattern/tuning/tests.rs`)

- `edo 19`, key 60+s for s in -19..=38 -> `note_to_freq(60.0) * (s/19).exp2()` (relative error <= 1e-12)
- `edo 31`, same with 31
- `edo 13 period 3` -> `note_to_freq(60) * 3^(s/13)`
- `:ji-5` -> key 64 = ref * 5/4; key 72 = ref * 2; key 48 = ref / 2
- `:partch-43` -> 43 degrees; key 60+43 = 2 * ref
- `with_mapping(ref_key 69, ref_freq 440)` on edo 19 -> `freq(69) == 440.0` exactly
- inline scl `"! c\nslendro approx\n5\n240.0\n480.0\n720.0\n960.0\n2/1\n"` -> key 61 = ref * 2^(240/1200)
- inline kbm with an `x` entry -> that key gives `Ok(None)`; the reference key mapped -> no error
- kbm with an unmapped reference key -> `Err(Type)`
- scl count mismatch / `abc` pitch / count 0 / period `0.0` -> `Err(Type)`
- spec bounds: steps 0, 1201, period 1, a negative ratio -> `Err(Type)`
- `to_control` then `from_control` round trip -> equal `Tuning` for edo, degrees and keymap variants
- `from_control(Int 3)` and `from_control([1 2])` -> `None`; `[:edo]` -> `Some(Err)`
- `note_key("d")` edo 19 default root -> 63; with root 62 -> 62; on `:ji-5` (K = 12) -> 62
- `nearest` ties -> the smaller |o|; fractional key 60.5 in edo 19 -> the closed form
- `keys_per_period`: edo 19 -> 19; degrees 43 -> 43; keymap of size 7 -> 7
- `scale_preset("maqam-rast")` -> size 24, steps `[0 4 7 10 14 18 21]`; `tuning_preset("nope")` -> None

## Verification

Evidence directory: `tmp/fm1-tuning/p01/run2/`. Run `mkdir -p tmp/fm1-tuning/p01/run2` first. The session-348 files directly under `tmp/fm1-tuning/p01/` are stale. Run from the repository root:

| Command | Must show |
| --- | --- |
| `CARGO_TERM_QUIET=true cargo build > tmp/fm1-tuning/p01/run2/build.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib pattern::tuning > tmp/fm1-tuning/p01/run2/nextest-focused.log 2>&1` | exit 0, failureCount 0, testsRun >= the number of Tests bullets (at least 17) |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/fm1-tuning/p01/run2/clippy.log 2>&1` | exit 0 |
| `rustfmt --edition 2021 --check src/pattern/mod.rs src/pattern/tuning/mod.rs src/pattern/tuning/scala.rs src/pattern/tuning/presets.rs src/pattern/tuning/tests.rs` | exit 0 |
| `wc -l src/pattern/tuning/*.rs` | every file < 1000 |

Record each as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Completion Criteria

- [x] The Progress Log records the choice between applying the WIP patch and rewriting, with the `git apply --check` exit status if it was applied
- [x] Contract symbols exist with the exact signatures above
- [x] All listed tests exist and pass
- [x] build, strict clippy and rustfmt pass; the line limits hold
- [x] No file outside the Files table changed (`git status --porcelain`)
- [x] Progress log below updated

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Implementation not started

### Session: 2026-10-10 (run 2 re-plan, session-351)
**Tasks Completed**: Added the Starting Point section (WIP patch, apply or rewrite) and moved the evidence directory to `tmp/fm1-tuning/p01/run2/`
**Notes**: Session-348 implementation was interrupted (stream hang) with compile errors in the draft. Plan 01 is not started in run 2

### Session: 2026-10-10 (run 2 implementation)
**Tasks Completed**: Applied and reviewed the WIP patch; completed the pinned tuning model, Scala parsers, presets and 17 focused tests.
**Notes**: Chose patch application. `git status --porcelain -- src/pattern` was empty and `git apply --check tmp/fm1-tuning/p01-wip-session348.patch` exited 0; applied to the working tree only, without `--index` or `--3way`. The session-348 logs were not used as evidence. Fresh review corrected degree-zero and fractional EDO resolution, keymap/control round-tripping, root mapping, and Scala final-period validation. First fresh gates exposed an invalid KBM root fixture and strict-Clippy `type_complexity`; both were corrected before final verification. Initial failed logs: `tmp/fm1-tuning/p01/run2/nextest-focused.log` (exit 100, 14 run, 13 passed, 1 failed) and `tmp/fm1-tuning/p01/run2/clippy.log` (exit 101). Final source-matched gates all passed; logs are under `tmp/fm1-tuning/p01/run2/final/`. Focused nextest: 17 run, 17 passed, 0 failed. Line counts: mod.rs 760, presets.rs 198, scala.rs 163, tests.rs 308. The only tracked changes are the five Files-table paths; no Git writes were made.

### Session: 2026-10-10 (adversarial repair P01-ADV-1)
**Tasks Completed**: Re-derived default anchors in `Tuning::with_mapping` for non-keymap tunings and added exact-frequency, `from_spec` equality, and control-round-trip assertions for EDO and JI mappings.
**Notes**: Root-only mapping now follows the root with the default reference key/frequency; reference-key-only mapping re-anchors to `note_to_freq(ref_key)`. Explicit frequencies and keymap anchor defaults retain their prior behavior. Initial `repair1` rustfmt check exited 1 for a formatting-only issue; formatted the pattern and reran every required gate without overwriting evidence. Final source-matched build, focused nextest (17 run, 17 passed, 0 failed), strict Clippy, rustfmt and line-count logs are in `tmp/fm1-tuning/p01/run2/repair1/retry-01/`; all tuning files remain under 1000 lines. Finding P01-ADV-1 is addressed in code and tests; independent re-review remains pending.
