# BASS-00: Bass Kernel Scaffold and Port Contract

**Status**: Completed
**Plan ID**: BASS-00 (wave 0; blocks every other BASS plan)
**Design Reference**: `design-docs/specs/design-bass-voices.md` (sections "Architecture decision", "Controls")
**Baseline**: wf/bass at `4fc3741` plus the accepted design commit
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The bass work adds one kernel UGen, `bass-core`, in a new module
`src/dsp/ugen/bass_voice`. Three wave-1 plans write the module's component
files in parallel:

- BASS-10: filters.
- BASS-11: oscillators and FM.
- BASS-12: envelopes, glide and LFO.

These components only compile if the module tree, the test module tree and
the port contract already exist. This plan creates them and nothing else.
It is small on purpose.

## Non-goals

- No DSP: `render` is a stub that writes silence.
- No registry changes. That means none of: `catalog.rs`, `codec.rs`,
  `names.rs`, `graph.rs`, `mixer.rs`, `build_helpers.rs`, `template.rs`,
  `meta*`, `templates.vact`, `commit.rs`, `ring.rs`.
- The kernel is not added to the `Node` enum. It stays unreachable from
  `.vact` until BASS-30.
- Do not touch `src/dsp/ugen/osc.rs`, `filter.rs`, `digital_drum/`, or any
  wf/syntax-fmt area (`src/fmt`, `src/complete`, `src/lsp`,
  `src/host/wasm`, `editor/`, `tree-sitter-vact/`).

## Dependencies

- **dependsOn**: none
- **Blocks**: BASS-10, BASS-11, BASS-12, BASS-20

## writePaths

- `src/dsp/ugen/bass_voice.rs`
- `src/dsp/ugen/bass_voice/ladder.rs`
- `src/dsp/ugen/bass_voice/diode.rs`
- `src/dsp/ugen/bass_voice/osc.rs`
- `src/dsp/ugen/bass_voice/fm.rs`
- `src/dsp/ugen/bass_voice/mods.rs`
- `src/dsp/tests/dsp/bass_voice.rs`
- `src/dsp/tests/dsp/bass_filters.rs`
- `src/dsp/tests/dsp/bass_sources.rs`
- `src/dsp/tests/dsp/bass_mods.rs`
- `impl-plans/active/bass-00-scaffold.md`

## writePathNotes

- path: `src/dsp/ugen/bass_voice.rs` | intendedEdit: new file
- path: `src/dsp/ugen/bass_voice/ladder.rs` | intendedEdit: new placeholder
- path: `src/dsp/ugen/bass_voice/diode.rs` | intendedEdit: new placeholder
- path: `src/dsp/ugen/bass_voice/osc.rs` | intendedEdit: new placeholder
- path: `src/dsp/ugen/bass_voice/fm.rs` | intendedEdit: new placeholder
- path: `src/dsp/ugen/bass_voice/mods.rs` | intendedEdit: new placeholder
- path: `src/dsp/tests/dsp/bass_voice.rs` | intendedEdit: new file
- path: `src/dsp/tests/dsp/bass_filters.rs` | intendedEdit: new placeholder
- path: `src/dsp/tests/dsp/bass_sources.rs` | intendedEdit: new placeholder
- path: `src/dsp/tests/dsp/bass_mods.rs` | intendedEdit: new placeholder
- path: `impl-plans/active/bass-00-scaffold.md` | intendedEdit: Progress Log only

## sharedPaths

- `src/dsp/ugen/mod.rs`
- `src/dsp/tests/dsp.rs`

## sharedPathNotes

- path: `src/dsp/ugen/mod.rs` | intendedEdit: add exactly one line, `pub mod bass_voice;`, in alphabetical position among the `pub mod` lines (after `analog_pair`)
- path: `src/dsp/tests/dsp.rs` | intendedEdit: add exactly four lines in alphabetical position: `mod bass_filters;`, `mod bass_mods;`, `mod bass_sources;`, `mod bass_voice;`

## Read-only References

- `src/dsp/ugen/mod.rs`: `Inp`, `Kx`, `NodeState`, `MAX_PORTS`.
- `src/dsp/ugen/analog_pair.rs`: the `render` signature with `mem`.
- `src/dsp/tests/dsp/analog_pair.rs`: `both_paths_reset_and_controls_respond`
  (the direct-render test harness).

## File-level Changes

### `src/dsp/ugen/bass_voice.rs`

Start with a module doc comment. It cites
`design-docs/specs/design-bass-voices.md` and the license boundary:
original code from papers, no emulation source consulted.

Declare the component modules:
`pub mod diode; pub mod fm; pub mod ladder; pub mod mods; pub mod osc;`.

Port contract (pin exactly). Components and the registry depend on it.

- `pub const PORT_COUNT: usize = 32;`
- `pub const PORTS: [(&str, f32); PORT_COUNT]`, in this order with these
  defaults:

  | Index | Port | Default | Index | Port | Default |
  |---|---|---|---|---|---|
  | 0 | `freq` | 55 | 16 | `lfo-rate` | 4 |
  | 1 | `mode` | 0 | 17 | `lfo-depth` | 0 |
  | 2 | `cps` | 0.5 | 18 | `lfo-offset` | 0 |
  | 3 | `onset-time` | 0 | 19 | `lfo-retrigger` | 1 |
  | 4 | `wave` | 0 | 20 | `lfo-sync` | 1 |
  | 5 | `cutoff` | 800 | 21 | `gate-length` | 1 |
  | 6 | `res` | 0.3 | 22 | `env-mod` | 2 |
  | 7 | `drive` | 0.2 | 23 | `env-decay` | 0.2 |
  | 8 | `detune` | 0.15 | 24 | `accent` | 0 |
  | 9 | `ratio` | 1 | 25 | `slide-from` | 0 |
  | 10 | `index` | 1 | 26 | `slide-time` | 0.06 |
  | 11 | `amp-attack` | 0.002 | 27 | `sub-level` | 0 |
  | 12 | `amp-decay` | 0.3 | 28 | `fm-feedback` | 0 |
  | 13 | `sustain` | 1 | 29 | `fold` | 0 |
  | 14 | `release` | 0.05 | 30 | `bit-depth` | 16 |
  | 15 | `lfo-wave` | 0 | 31 | `click-level` | 0 |

- `pub mod port { pub const FREQ: usize = 0; ... pub const CLICK_LEVEL: usize = 31; }`
  gives one SCREAMING_SNAKE constant per port with the index above. For
  example `ONSET_TIME = 3` and `GATE_LENGTH = 21`.
- `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Model { Analog, Acid, Fm, Wobble, Sub, Reese }`,
  with `Model::from_port(v: f32) -> Model`. It rounds, clamps to 0..=5 and
  maps non-finite values to `Analog`.
- `pub fn sanitize(x: f32) -> f32` returns:
  - 0.0 for a non-finite `x`;
  - 0.0 when `|x| < 1e-20` (the denormal flush);
  - `x` otherwise.

  Every component uses it; that is its contract with BASS-10/11/12.
- `pub const STATE_FLOATS: usize = 1;` is a placeholder. BASS-20 sets the
  real value.
- `pub fn render(ins: &[Inp<'_>; MAX_PORTS], st: &mut NodeState, mem: &mut [f32], out: &mut [f32], kx: &Kx<'_>)`
  is a stub that fills `out` with `0.0`. Mark unused arguments with a
  leading underscore or `let _ = ...`, whichever clippy accepts. BASS-20
  replaces the body.

### Placeholder files

The five component files and the three `bass_filters`/`bass_sources`/
`bass_mods` test files each contain only a `//!` doc line that names their
owning plan, for example `//! Transistor ladder (owned by BASS-10).`.
Clippy must not flag them.

### `src/dsp/tests/dsp/bass_voice.rs`

Add four tests:

- `bass_voice_ports_are_unique_and_within_max_ports`
- `bass_voice_model_from_port_rounds_and_clamps`
- `bass_voice_sanitize_flushes_denormals_and_non_finite`
- `bass_voice_render_is_finite_and_bounded`

## Pitfalls

- Port names and order are a cross-plan contract. BASS-30 builds the
  catalog list from this table and tests it against `PORTS`, so do not
  reorder or rename anything.
- `wave`, `lfo-wave`, `lfo-retrigger` and `lfo-sync` values are enum or
  bool indices as floats:
  - `wave`: saw 0, pulse 1, square 2, tri 3, sine 4.
  - `lfo-wave`: sine 0, tri 1, saw 2, ramp 3, square 4, random 5.
- Do not add the `Node` variant here, even though it would be convenient.
  Doing so forces `catalog.rs`/`codec.rs`/`mixer.rs` match arms and so
  breaks the registry serialization.

## Tests (input -> expected)

- `PORTS` has 32 unique names, and `PORT_COUNT <= MAX_PORTS` (48).
- `port::FREQ == 0`, `port::CPS == 2`, `port::GATE_LENGTH == 21`, and
  `port::CLICK_LEVEL == 31`.
- `PORTS[port::X].0` equals the expected name for every `X`: iterate over a
  local table.
- `Model::from_port` maps:
  - `1.4` to `Acid`, `5.0` to `Reese`;
  - `9.0` to `Reese`, `-3.0` to `Analog`;
  - `NaN` to `Analog`.
- `sanitize`:
  - `1e-25` to `0.0`, `f32::NAN` to `0.0`, `f32::INFINITY` to `0.0`;
  - `0.5` to `0.5`, `-1e-10` to `-1e-10`.
- `render` of a 256-frame block with `PORTS` defaults writes only finite
  samples with `|y| <= 1.0` (test `bass_voice_render_is_finite_and_bounded`).
  Build `Kx` exactly as `analog_pair.rs` does. It must NOT assert all zeros:
  BASS-20 replaces the stub, and this test must keep passing afterwards.

## Verification (evidence required)

Run from the repository root and save every log under `tmp/logs/`.

1. `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice.rs src/dsp/ugen/bass_voice/*.rs src/dsp/tests/dsp/bass_voice.rs src/dsp/tests/dsp/bass_filters.rs src/dsp/tests/dsp/bass_sources.rs src/dsp/tests/dsp/bass_mods.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_voice::/)' > tmp/logs/bass-00-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`, and the log's summary must show at least 4 tests
   run with 0 failed.
3. `git diff --stat` shows only the writePaths and the two sharedPaths.

## Concurrency and Drift Protocol

- Fresh-read each file right before editing it.
- For the two sharedPaths, record the `shasum -a 256` before and after the
  edit in the Progress Log.
- If a pre-hash differs from `git show HEAD:<path> | shasum -a 256`
  (drift), re-read and apply only the one-line intent. Never revert
  someone else's lines.
- No git operations other than `git diff` and `git status`.

## Done Criteria

- [x] All writePaths exist, and the sharedPaths changed by exactly 1 and 4
      lines.
- [x] Verification 1-3 pass, with the exit codes and test count recorded.
- [x] Progress Log updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none
**Notes**: Awaiting implementation.

### Session: 2026-09-30 (BASS-00 implementation)
**Tasks Completed**: Kernel port scaffold, component/test placeholders, shared module declarations, and four port-contract tests.
**Verification**:
- `rustfmt --edition 2021 --check src/dsp/ugen/bass_voice.rs src/dsp/ugen/bass_voice/*.rs src/dsp/tests/dsp/bass_voice.rs src/dsp/tests/dsp/bass_filters.rs src/dsp/tests/dsp/bass_sources.rs src/dsp/tests/dsp/bass_mods.rs` — exit 0; log: `tmp/logs/bass-00-rustfmt.log`.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass_voice::/)'` — exit 0; 4 passed, 0 failed; log: `tmp/logs/bass-00-nextest.log`.
- `git diff --stat` — exit 0; the plan progress file and both shared paths appear in the full diff; shared source hunks are exactly 1 line in `src/dsp/ugen/mod.rs` and 4 lines in `src/dsp/tests/dsp.rs`, with no Node variant or registry edits; log: `tmp/logs/bass-00-diff-stat.log`.
**Shared-path SHA-256**:
- `src/dsp/ugen/mod.rs`: before `4e53f4fb20efa3c50e5b487fc975d2ab9e37093e7412412259c7f1c602b535df`; after `e54421034f6b5dd8522ab16abb5a0908a662623b7708180e84d2ac08f954792b`.
- `src/dsp/tests/dsp.rs`: before `301119efe5808c079c892722f3d463829a65504482c714883eced72965072b44`; after `25831d329056191cdaec23bf69e2d4512b2f29f10d7e026f94cea581fca2f818`.
**Notes**: The scaffold remains unreachable from the UGen registry. DSP implementations and registry/template wiring are owned by downstream plans.

### Session: 2026-09-30 (serial reconcile)
**Tasks Completed**: Combined-tree reconcile gates for BASS-00.
**Repair**: `cargo clippy --all-targets -- -D warnings` failed on `clippy::assertions_on_constants` at `src/dsp/tests/dsp/bass_voice.rs:11`. The `PORT_COUNT <= MAX_PORTS` check is now a compile-time `const _: () = assert!(...)`, which keeps the check.
**Correction**: The adversarial review moved `pub mod bass_voice;` after `analog_percussion`. The current `src/dsp/ugen/mod.rs` SHA-256 is `76a980db3c5094b3c293193252ad69695d83f94efff9c7cd7cbea7280d1470e4`, which replaces the earlier "after" hash. Canonical logs are under `tmp/bass-voices-229/BASS-00/` and `tmp/bass-voices-229/reconcile/`.
**Verification** (logs in `tmp/bass-voices-229/reconcile/attempt-2/`): cargo build, cargo fmt --check, clippy -D warnings, wasm32 host-wasm build, and mise run lint all exit 0. Full nextest: 1627 passed, 2 skipped, exit 0. The `test(/bass_voice::/)` filter: 4 passed, exit 0.

### Session: 2026-09-30 (session 232 closeout)
**Tasks Completed**: The scaffold was adopted by BASS-20 and BASS-30. The whole bass voice set was accepted by serial integration review (comm-003043).
**Verification**: Combined-tree reconcile gates in `tmp/bass-voices-232/reconcile/wave-7/` all exit 0 (build, clippy, fmt check, mise lint, full nextest 1697 passed, wasm32 build).
**Remaining (non-blocking)**: The Step 8 move to `impl-plans/completed/` was denied by the sandbox and is still pending.
**Status**: Completed.
