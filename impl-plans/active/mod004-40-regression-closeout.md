# MOD004-40: Cross-path Regression, Allocation, Rate/Block Invariance, Plan Closeout

**Status**: Ready
**Plan ID**: MOD004-40 (session-194 wave 2, final serial reconciliation)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Test strategy; Voice buffers and channel mapping; Capacity, real-time, and invariance; Implementation status)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004F and plan closeout)
**Baseline**: `b6fa077` plus the accepted MOD004-30 working-tree changes
**Created**: 2026-09-29
**Last Updated**: 2026-09-29 (refined for session 194: baseline `b6fa077`, concrete exemplars, block-invariance window, design status edit)

## Intent and Context

The last wave proves the acceptance signals that span plans, then closes
MOD-004 in the plan set:
- native and browser install identical compiled shapes and render
  bit-identically for mono, multi-mono and stereo graphs;
- channels stay independent through intermediate mono and stereo effects,
  pan/balance and orbit sends;
- callback allocation probes pass with multi-output nodes and stereo voice
  effects;
- stateful multi-output renders keep the rate and block-partition
  contract.

Existing coverage to build on, not duplicate:
- `src/dsp/tests/dsp/voice_layout.rs`: native-only balance/equal-power
  pan, stereo effect/`Mul`/`Add` channel preservation, `va-filter` pair vs
  two legacy nodes, output-0 sample downmix, slice capacity.
- `src/dsp/tests/dsp/codec_shapes.rs`: exact byte fixtures and round trips.
- `src/host/tests/e2e/templates/select_output.rs` and `migrated_pairs.rs`
  (MOD004-30): `.vact` selection and template equivalence.

This plan adds what those do not cover: native vs browser equality,
orbit routing, and rate/block partition invariance for multi-output nodes.

Evidence rule: every verification command writes its full output to
`tmp/mod004/MOD004-40/<n>-<name>.log`, and its exit status goes into the
Progress Log next to the log path. A missing or truncated log is not a
pass. `tmp/` is gitignored and must stay untracked.

## Non-goals

- No feature work. If a check fails because of a defect in another plan's
  code, fix it here serially, record the file, cause and fix in the
  Progress Log, and never weaken a test to pass.
- No new public API, no test-helper refactor in `src/dsp/tests/dsp.rs`
  unless a needed helper is missing (then add only that helper and record
  it).
- No archiving to `impl-plans/completed/`: MOD-005/006 are not done, so the
  parent plan stays in `impl-plans/active/`.
- No edits to `voice_layout.rs`, `codec_shapes.rs`, `select_output.rs` or
  `migrated_pairs.rs` except as a recorded serial repair.

## Dependencies

- **dependsOn**: MOD004-30 (accepted committed dependencies: MOD004-00, -10, -11, -12, -20, -21, -22)

## writePaths

- `src/dsp/tests/dsp/multi_output.rs` (currently a one-line stub; already
  declared by `mod multi_output;` in `src/dsp/tests/dsp.rs`). If it would
  reach 1000 lines, split into `src/dsp/tests/dsp/multi_output/`
  submodules declared from `multi_output.rs`.
- `impl-plans/active/modular-audio-foundation.md`: MOD-004 table row,
  MOD-004 fanout paragraph, MOD-004A..F Status lines and criteria, a new
  Progress Log session
- `impl-plans/active/mod004-20-select-lowering.md`,
  `impl-plans/active/mod004-21-codec.md`,
  `impl-plans/active/mod004-22-voice-runtime.md`,
  `impl-plans/active/mod004-30-template-migration.md`: the header
  `**Status**:` line (line 3) only
- `impl-plans/README.md`: the modular-audio-foundation row only
- `design-docs/specs/design-mutable-audio.md`: the "Implementation status
  (2026-09-29)" subsection only
- This plan's Progress Log and Status line
- `tmp/mod004/MOD004-40/` (evidence)

sharedPaths (serial, this wave runs alone): `src` for recorded serial
repairs and the one-time `cargo fmt` reconciliation.

## Test Cases (`src/dsp/tests/dsp/multi_output.rs`)

Exemplars to imitate:
- `src/dsp/tests/dsp/analog_pair.rs:browser_analog_pair_codec_survives_rates_and_blocks`
  (native `Template::from_inst` vs `encode_inst` -> `decode_graph` ->
  `Template::boxed().build(&raw, &env)`, and `BrowserRig::browser_with`
  plus `encode_graph_record` per rate/block);
- `src/dsp/tests/dsp/voice_layout.rs` (`graph`, `edge(from, to, port,
  output)`, `stereo_sample_graph`, `va_filter_graph(pair)`, `render(def,
  pan)`) for hand-built graphs; copy small builders, do not import private
  ones;
- `src/dsp/tests/dsp/templates.rs:247` (`encode_sample_begin`,
  `encode_slice`) for browser sample installs and `NativeRig::sample` for
  native ones;
- `src/dsp/tests/dsp/effects.rs:orbit_delay_sends_echo_into_master` for the
  orbit delay send controls (`CtlId::new(38)`, `39`, `40`).

`Rig::step` (and so `Rig::run`) asserts zero callback allocations; every
render in this file must go through it.

Graphs (hand-built `InstDef`s):
- (a) mono: `SinOsc` -> `Mul` by `amp`.
- (b) multi-mono: `VaSource` -> `VaFilter` (`mode` 0); output 0 -> `Mul`
  sink; output 1 -> `Mul` -> `AuxOut`.
- (c) stereo: `SamplePlay` output 1 (`:stereo`) -> a stereo voice-local
  `Effect` (same spec as `voice_layout.rs:stereo_effect_and_mul_preserve_each_sample_channel`)
  -> `Mul` by `amp`, with a two-channel resource.
- (d) fm pair: one `FmPair` (`mode` 0) with output 1 -> `AuxOut`, and its
  legacy twin with two `FmPair` nodes (`mode` 0 and 1).

Cases (`situation -> expected`):
- Cross-path shapes: for (a), (b), (c), (d) -> native and decoded
  templates have equal `nodes()` (includes `outs` and `shape`), `stereo`,
  `has_aux` and `n_slices`; (a) is `stereo == false && has_aux == false`,
  (b) and (d) `has_aux == true`, (c) `stereo == true`.
- Cross-path renders: each graph rendered on `NativeRig` and
  `BrowserRig` with the same event -> L and R equal by `to_bits`, and
  both non-silent where the graph has a signal on that side.
- Channel independence, stereo: (c) with a resource whose right channel is
  all 0.0 -> R is exactly 0.0 on native and browser; with left all 0.0 ->
  L exactly 0.0.
- Channel independence, main/aux: (b) with the main branch multiplied by
  `Const(0.0)` -> L is exactly 0.0 and R equals R of unmodified (b) by
  `to_bits`.
- Pan/balance cross-path: (b) and (c) at pan 0.0 and 1.0, (a) at pan 0.2
  -> native == browser by `to_bits`, and each channel equals the center-pan
  render scaled by `balance_gains(p)` ((b), (c)) or matches `pan_gains`
  ((a)) within the tolerance `voice_layout.rs` uses.
- Orbit: (b) and (c) with a nonzero orbit delay send -> native == browser
  by `to_bits`, output finite, and echo energy present after the dry burst
  (window as in `orbit_delay_sends_echo_into_master`). Also for (c) with a
  right-zero resource: if the orbit delay processes L and R without
  cross-feed (read `src/dsp/effects/` to confirm), assert R stays exactly
  0.0; if it cross-feeds, do not assert that, and record the reason in the
  Progress Log.
- Rate/block invariance for (b) pair vs legacy two-node, (d) pair vs
  legacy, and (c), at 44 100, 48 000 and 96 000 Hz with `max_block` 64,
  256 and 97, native and browser:
  - install, `step()` once, send the event at time `2048.0 / rate`
    seconds, then `run` until at least `2048 + 8192` frames have been
    rendered in total; compare only frames `[2048, 2048 + 8192)` (the
    first `step()` output is frames `[0, max_block)`);
  - (1) at each rate and block, pair render == legacy render by
    `to_bits`, on native and on browser;
  - (2) at each rate, the window is bitwise equal across blocks 64, 256
    and 97.
  - If (2) also fails for the legacy two-node form, the dependence
    predates MOD-004: record the evidence, keep (1) plus finite and
    `rms > 1e-5` checks, and file the finding in the Progress Log. Do not
    hide it and do not change engine code for it.

## Closeout Edits

`impl-plans/active/modular-audio-foundation.md`:
- MOD-004 table row: Status `Completed 2026-09-29 (fanout plans
  impl-plans/active/mod004-*.md)`; keep MOD-005/006 rows untouched.
- Fanout paragraph: state that MOD004-20/21/22 are committed in `b6fa077`
  and MOD004-30/40 complete in this session.
- MOD-004A..F: set `**Status**: COMPLETED`, check each criterion that the
  listed tests prove, and name the delivering plan per subtask.
- Add a dated Progress Log session `### Session: 2026-09-29, MOD-004
  closeout` with the nextest pass count and the exit status of commands
  1-10 below.

Status lines: `mod004-20`, `-21`, `-22` -> `**Status**: Completed
(committed in b6fa077)`; `mod004-30` and this plan -> `**Status**:
Completed`. Change no other line in those files.

`impl-plans/README.md`: the modular-audio-foundation row says MOD-004
complete, MOD-005/006 pending, date 2026-09-29.

`design-docs/specs/design-mutable-audio.md`: in "Implementation status
(2026-09-29)", replace the "Still to do" paragraph with one saying the
nine migrations and regression closeout are complete, naming
`migrated_pairs.rs` and `multi_output.rs`. Change nothing else in the
design.

## Verification Commands (the full acceptance set; logs in `tmp/mod004/MOD004-40/`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0. If it fails, run
   `CARGO_TERM_QUIET=true cargo fmt` once (serial formatting
   reconciliation), record the files it changed, then rerun -> exit 0.
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the pass count
7. `VACTR_MI_REFERENCE=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack mise run audit-upstream` -> errors []
8. `git diff --name-only cf2ea37 -- '*.rs' | xargs wc -l` and `git ls-files --others --exclude-standard -- '*.rs' | xargs wc -l` -> every touched or new Rust file below 1000 lines
9. `git diff --check` -> exit 0
10. `git status --short` -> only paths under `src/`, `design-docs/` and
    `impl-plans/` (no `tmp/` entries, since `tmp/` is ignored)

## Completion Criteria

- [ ] `multi_output.rs` covers cross-path shapes and renders, stereo and main/aux channel independence, pan/balance, orbit, allocation (via `Rig::step`) and rate/block invariance.
- [ ] Commands 1-10 pass, with exit statuses and log paths recorded.
- [ ] Parent MOD-004A..F are COMPLETED with checked criteria and a dated Progress Log session; the README row, the five Status lines and the design status note are updated.
- [ ] Every serial repair is listed with file, cause and fix.

## Execution Protocol

This wave runs alone after MOD004-30 is accepted. Do not change git state
(no commit, stash, checkout, reset, branch or worktree); the orchestrator
commits afterwards. Record pre/post `shasum -a 256` of every edited file,
re-read each file just before editing, stop on unexpected drift, and log
every command in this Progress Log.

## Progress Log

(empty)
