# FM1 Tuning 05: Natives, Type Table, perform, and End-to-End Tests

**Status**: Ready
**Plan ID**: fm1-tuning-05-natives
**Wave**: 3 (depends on fm1-tuning-01-model, fm1-tuning-02-pattern-graph, fm1-tuning-03-resolution, fm1-tuning-04-music)
**Design Reference**: design-docs/specs/design-tuning-and-strum.md sections 4.1, 4.5, 5.4, 6, 10 (items 2, 4-7, 11)
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

This plan exposes the feature to vactr source. It adds nine natives
(`tune edo ratios scala load-scala strum harp inversion perform`), their
native-table entries (which are also the editor completion and hover
metadata), and the `perform` chord-literal check. It also adds the
end-to-end tests that need source code: tuned patterns through the real
scheduler, and the song freeze/encode path (review DR-1).

## Non-goals

- No change to `PatNode`, query semantics, commit, music.rs or the tuning
  model. If a bug shows up there, record it in the progress log and fix it
  only if the fix lies inside this plan's writePaths. Otherwise stop and
  report it as a dependency defect.
- No `bass` or `invert` native. No docs or examples (plan 06).
- No UGen, template, codec, control-table or golden-digest change.

## Working Rules

Follow the Working Rules in `fm1-tuning-01-model.md`: fresh read plus
sha256 before and after each edit, re-apply on drift, own paths only, no
git writes, no broad formatting, silent tests, no full nextest.
`src/sched/tests/sched.rs` was last edited by plan 03; append only the
one `mod tuning_natives;` line.

## Files

| Path | Change |
| --- | --- |
| `src/vm/natives/tuning.rs` | new: the 9 native fns and `register` |
| `src/vm/natives/mod.rs` | `mod tuning;` and `tuning::register(p);` as the LAST line of `register_domain` |
| `src/types/natives_domain.rs` | append 9 `f(..)` entries at the END of `DOMAIN` (after `play-song`) |
| `src/types/infer_call.rs` | the `"chord"` arm at :432 also handles `"perform"`: check argument index 1 with `chord_arg` |
| `src/sched/tests/sched.rs` | add `mod tuning_natives;` |
| `src/sched/tests/sched/tuning_natives.rs` | new scheduler end-to-end tests from source |
| `tests/song_tuning.rs` | new integration test for song freeze and encode |
| `tests/fixtures/tuning/slendro.scl` | new fixture |
| `tests/fixtures/tuning/slendro.kbm` | new fixture |

## Native table entries (append exactly; masks follow neighbors)

| Entry |
| --- |
| `f("tune", 2, 2, &["fn (pattern 'a) any -> pattern 'a"], &[V, L]).kw(&["root", "ref-key", "ref-freq"])` |
| `f("edo", 1, 1, &["fn int -> [keyword: any]"], &[V]).kw(&["period"])` |
| `f("ratios", 1, 1, &["fn [any] -> [keyword: any]"], &[V])` |
| `f("scala", 1, 1, &["fn str -> [keyword: any]"], &[V]).kw(&["kbm"])` |
| `f("load-scala", 1, 1, &["fn path -> [keyword: any]"], &[V]).kw(&["kbm"]).effect()` |
| `f("strum", 2, 4, &["fn (pattern 'a) any any any -> pattern 'a"], &[V, L, L, L])` |
| `f("harp", 2, 2, &["fn (pattern 'a) any -> pattern 'a"], &[V, L]).kw(&["strips", "base"])` |
| `f("inversion", 2, 2, &["fn (pattern 'a) any -> pattern 'a"], &[V, L])` |
| `f("perform", 2, 2, &["fn (pattern 'a) any -> pattern 'a"], &[V, L]).kw(&["mode", "inversion", "bass", "time", "dir", "curve", "arp", "pos", "strips", "base"])` |

If a type string fails to parse in the table tests
(`src/types/tests/natives.rs`), use the nearest accepted form from an
existing entry (for example `any` for the result) and record that in the
progress log. Native ids are table indices, and `DOMAIN` is the last list
(`src/types/natives.rs:193`), so appending at its end keeps every existing
id.

## Native behavior (imitate `src/vm/natives/music.rs`, `pattern.rs:euclid` for `named(kw, ..)`)

- **`tune`**:
  - Subject `pat(a, 0)`. The tunings value is `pat(a, 1)`, but force a
    Thunk first, like `music.rs:chord`.
  - Mapping from kwargs: `root` (Int, or note keyword via `note_number`),
    `ref-key` (same), `ref-freq` (number > 0). Each is read once; a wrong
    type is a Type failure naming the kwarg.
  - Build with `combinators::tune::tune`.
  - If the tunings value is a constant (not a pattern), validate it now
    with `Tuning::from_spec`, so a bad constant fails at the call.
- **`edo`**: `edo_spec(&a[0], named(kw, "period"))`. **`ratios`**:
  `ratios_spec(&a[0])`. **`scala`**: `Value::Str` text plus optional `kbm`
  Str -> `scala_spec`.
- **`load-scala`**:
  - Copy the effect-mode check and the `LoaderHost` take/put-back sequence
    from `src/ns/load.rs:load` (EffectMode::Query ->
    `effect-in-query`).
  - Read the `.scl` path and the optional `kbm:` path through
    `loader.read(&path)`, then call `scala_spec`.
  - No loader -> `host-unavailable`. Always put the host back, including
    on error paths.
- **`strum`**:
  - `param(a, 1)` time.
  - `dir` = `param(a, 2)` if present, else `PParam::Const(:up)`.
  - `curve` = `param(a, 3)` if present, else `PParam::Const(:flat)`.
- **`harp`**: `param(a, 1)` pos; `strips` Int kwarg (default 12); `base`
  kwarg (Int or note keyword, default `None`).
- **`inversion`**: `param(a, 1)`, bass `false`.
- **`perform`** (no node of its own; compose):
  1. `chord(chord_pattern_of(forced a[1]), subject)`
  2. `voicing`
  3. `inversion(param_of(inversion kw) or PParam::int(0), bass kw bool default false)`
  4. by `mode` (keyword, read once, default `:block`):
     - `:block`: nothing more
     - `:strum`: `strum(time default Ratio 1/32, dir default :up, curve default :flat)`
     - `:arp`: `music::arp(arp kw default :up)`
     - `:harp`: `harp(pos default the saw signal value, strips default 12, base)`

     Get the saw signal value from the prelude slot `saw`, or construct it
     as `src/vm/natives/signal.rs` does.
  5. Any other mode -> a Type failure listing the four modes.

## Tests

`src/sched/tests/sched/tuning_natives.rs` (use `Rig`, `pat_of`, `cval`,
`sent`, `midi_calls`):

SYNTAX: vactr has no `( )` (a reader error, lang-reference.md:296/:332).
Nested calls use braces, as in `gain {range sine 0.3 0.55}` and
`chord {alt [:c :m7] [:f :maj7]}`. The design doc's `tune (edo 19)`
notation means `tune {edo 19}` in source.

- `s :analog > tune {edo 19} > note [60 66 79]` -> freqs `note_to_freq(60) * 2^(s/19)` for s = 0, 6, 19 (f32 within 1 ulp)
- `tune {edo 31}` and `tune {edo 13 period: 3}` -> analytic values (design 4.3)
- `tune {ratios [16/15 9/8 6/5 5/4 4/3 45/32 3/2 8/5 5/3 9/5 15/8 2]}`, note 64 -> ref * 5/4
- inline `tune {scala "..."}` text with cents and a ratio -> analytic; with `kbm: "..."` and an `x` key -> no event for that key
- `tune {edo 19} ref-key: 69 ref-freq: 440 > note 69` -> exactly `440.0f32`
- `tune {edo 19} > note :d` -> key 63 (audio); same on `s {midi 1}` -> midi note 62 (nearest)
- `tune {edo 19} root: :d > note :d` -> `note_to_freq(62)`
- `tune {alt {edo 19} {edo 31}}` -> per-cycle tuning alternates
- `tune {edo 19} > chord [:c :maj] > strum 1/32` -> three onsets 1/32 apart, freqs of keys 60, 66, 71
- `perform` with each mode on `s :pd` and `s :fm` (existing templates) -> the expected note count, onsets and tuned freqs under `tune {edo 19}`
- `perform ... mode: :nope` -> Type failure
- `load-scala` with a fixture loader reading `tests/fixtures/tuning/slendro.scl` (and `kbm: ...slendro.kbm`) -> analytic freqs. Use `Rig::with_loader` or a custom `SourceLoader` like the existing loader tests.
- `load-scala` with no loader -> `host-unavailable`
- untuned `s :analog > note [60 :d 61.5]` -> bit-equal to `note_to_freq` (regression)

`tests/song_tuning.rs` (copy only the needed helpers from
`tests/song_end_to_end.rs`: `limits`, `candidate`, `query_identity`, `Rig`,
`play`):

- program `song {part [lead: {s :analog > tune {edo 19} > note :d > gain 0.2}] duration: 1} tail-seconds: 0 > play-song`
  -> the frozen event `note == ResolvedNote::Int(63)`, and its controls contain the `tuning` list
- the played `SongCommand::Event(SongAudioEvent { event, .. })` `freq` ctl -> `(note_to_freq(60.0) * (3.0/19.0).exp2()) as f32` within 1 ulp
- the same program without `tune` -> note 62, freq `note_to_freq(62) as f32` (bit equal)
- strum song case (proves plan 02's freeze.rs/capture.rs arms for a non-tune node):
  `song {part [lead: {s :analog > chord [:c :maj] > strum 1/32 > gain 0.2}] duration: 1} tail-seconds: 0 > play-song`
  -> preparation and freezing succeed (no "dispatch invariant" failure), exactly 3 frozen events with notes 60, 64 and 67, and onsets 0, 1/32 and 2/32 cycles. Playback emits 3 `SongCommand::Event`s.
- tuned strum song case: the same with `tune {edo 19} >` before `chord` -> 3 events with keys 60, 66 and 71, and freqs equal to the analytic edo-19 values (within 1 ulp)

## Verification

Evidence directory: `tmp/fm1-tuning/p05/`.

| Command | Must show |
| --- | --- |
| `CARGO_TERM_QUIET=true cargo build > tmp/fm1-tuning/p05/build.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib sched::tests types:: vm:: ns:: complete:: lsp:: > tmp/fm1-tuning/p05/nextest-focused.log 2>&1` | exit 0, failureCount 0 (includes the native-table consistency tests) |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_tuning --test song_end_to_end --test song_natives > tmp/fm1-tuning/p05/nextest-song.log 2>&1` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/fm1-tuning/p05/wasm-lib.log 2>&1` | exit 0 (`load-scala` compiles for wasm) |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/fm1-tuning/p05/clippy.log 2>&1` | exit 0 |
| `rustfmt --edition 2021 --check src/vm/natives/tuning.rs src/vm/natives/mod.rs src/types/natives_domain.rs src/types/infer_call.rs src/sched/tests/sched.rs src/sched/tests/sched/tuning_natives.rs tests/song_tuning.rs` | exit 0 |
| `git diff src/types/natives_domain.rs` | additions only, after `play-song` |

## Completion Criteria

- [ ] 9 natives registered; table entries appended at the end of `DOMAIN`
- [ ] All listed tests pass, including the song freeze/encode `:d -> 63` case
- [ ] The wasm lib build, clippy and rustfmt pass; each file < 1000 lines
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Implementation not started
