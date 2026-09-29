# MOD004-40: Cross-path Regression, Allocation, Rate/Block Invariance, Plan Closeout

**Status**: Ready
**Plan ID**: MOD004-40 (wave 4, final serial reconciliation)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Test strategy)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004F and plan closeout)
**Baseline**: `85a300a`, plus the accepted MOD004-20/21/22/30.
**Created**: 2026-09-29
**Last Updated**: 2026-09-29 (refined for session 192: Status-line scope, evidence rule)

Notes for session 192:
- The Status lines of `mod004-00-baseline.md` and
  `mod004-12-kernel-pairs.md` were set to `Completed (committed in
  85a300a)` during planning. This plan updates only the Status lines of
  MOD004-20/21/22/30/40.
- Evidence rule: every verification command writes its full output to
  `tmp/mod004/MOD004-40/<n>-<name>.log`, and its exit status to the
  Progress Log next to that log path. A missing or truncated log does not
  count as a pass. `tmp/` is gitignored and must stay untracked.
- `.vact` strings in this plan's tests use source form (`{p :aux}`), never
  `(p :aux)`. See MOD004-20.

## Intent and Context

The last wave proves the acceptance signals that span plans:
- native and browser install identical compiled shapes and render
  identically for mono, multi-mono and stereo graphs;
- channels stay independent through intermediate mono and stereo effects,
  pan/balance and orbit sends;
- callback allocation probes pass with multi-output nodes and stereo voice
  effects;
- stateful multi-output renders keep the rate and block-partition
  contract.

It then runs the full acceptance command set, reconciles formatting, and
updates the parent plan and index.

## Non-goals

- No feature work. If a check fails because of a defect in another
  plan's code, fix it here serially. Record each fix and its cause in the
  Progress Log, and never weaken a test to pass.
- No archiving to `impl-plans/completed/` unless every MOD-004 criterion
  and all of MOD-005/006 are done. They are not, so the parent stays
  active.

## Dependencies

- **dependsOn**: MOD004-20, MOD004-21, MOD004-22, MOD004-30

## writePaths

- `src/dsp/tests/dsp/multi_output.rs` (stub; fill it)
- Any file needed for a serial repair, with each one listed and justified
  in the Progress Log
- `impl-plans/active/modular-audio-foundation.md`: MOD-004 rows,
  criteria, progress log
- `impl-plans/active/mod004-*.md`: Status lines only
- `impl-plans/README.md`: the modular-audio-foundation row
- this plan's Progress Log

## Test Cases (`src/dsp/tests/dsp/multi_output.rs`)

Imitate `src/dsp/tests/dsp/stereo_contract.rs` (native plus browser
serialized installs), `rate_contract.rs:native_and_browser_drum_render_across_rates_and_blocks`,
and `Rig::native`/`Rig::browser`. `Rig::step` asserts zero allocations.

- **Cross-path shapes**: for three hand-built `InstDef`s,
  - (a) mono `SinOsc` -> `Mul`;
  - (b) `VaSource` -> `VaFilter` with `:main` to the sink and `:aux`
    through `AuxOut`;
  - (c) `SamplePlay` `:stereo` (a 2-channel resource with L != R) ->
    `Effect(gain)` -> `Mul` by `amp`;
  the native install (`Template::from_inst`) and the browser install
  (encode -> bytes -> `decode_graph` -> build) give templates with equal
  `nodes()[i].outs`, `nodes()[i].shape`, `stereo`, `has_aux` and
  `n_slices`, and equal L/R renders, bitwise.
- **Channel independence through intermediate effects**: in (c), an
  intermediate stereo effect (for example `polarity` or `ms`) followed by
  a mono-subject effect on the `VaFilter` aux path in a separate graph ->
  L carries only channel-0 content and R only channel-1 content. Check
  with a resource whose right channel is zero: R stays exactly 0.0.
- **Pan/balance**:
  - graph (c) at pan 0.0, 0.5 and 1.0 -> L/R scaled by
    `balance_gains(p)` exactly;
  - graph (b) the same;
  - graph (a) at pan 0.2 -> `pan_gains`.
- **Orbit routing**: with a nonzero `delay-send`/orbit send (use the
  control names from `src/dsp/tests/dsp/cut_group.rs` or other existing
  orbit tests), the orbit input receives the post-balance L/R. In graph
  (c) with right channel 0, the orbit right input stays 0.
- **Allocation probes**: every render above runs through `Rig::step`,
  which asserts zero allocations, including (c), which has a stereo
  voice-local effect.
- **Rate/block invariance** for `fm-pair` and `va-filter` pair graphs and
  graph (c), at 44 100, 48 000 and 96 000 Hz with callback blocks of 64,
  256 and 97:
  - (1) at each rate and block, the pair-node render equals the two-node
    legacy render, bitwise, on both native and browser;
  - (2) at each rate, renders with block 64, 256 and 97 are bitwise
    equal.
  If (2) fails for the legacy two-node form too, the dependence predates
  MOD-004. In that case record the evidence, assert (1) plus the existing
  `rate_contract` bounds (finite and audible), and file the finding in
  the Progress Log. Do not hide it.

## Closeout Edits

In `impl-plans/active/modular-audio-foundation.md`:
- Set MOD-004A..F statuses to COMPLETED, check each criterion that is met,
  and point each subtask to the fanout plan that delivered it.
- Mark the top-level MOD-004 row as completed.
- Check "Stereo input/output survives every graph and install/wire path"
  only if every test above passes.
- Add a dated Progress Log session with test counts and command results.
- Keep MOD-005/006 untouched.

In `impl-plans/README.md`, update the modular-audio-foundation row.

## Verification Commands (the full acceptance set; logs in `tmp/mod004/MOD004-40/`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0. If it fails, run
   `CARGO_TERM_QUIET=true cargo fmt` once here (the serial formatting
   reconciliation), then rerun it.
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the pass count
7. `VACTR_MI_REFERENCE=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack mise run audit-upstream` -> errors []
8. `git diff --name-only cf2ea37 -- '*.rs' | xargs wc -l` -> every touched Rust file below 1000
9. `git diff --check` -> exit 0
10. `git status --short` -> only paths under `src/`, `design-docs/`,
    `impl-plans/` and `tmp/` (tmp logs may stay untracked; do not commit
    them)

## Completion Criteria

- [ ] multi_output tests cover cross-path shapes, channel independence, pan/balance, orbit, allocation and rate/block.
- [ ] Commands 1-10 pass, with logs recorded.
- [ ] The parent plan's MOD-004A..F criteria are checked, with a dated progress entry; the README row is updated.

## Execution Protocol

This wave runs alone. Do not change git state (no commit, stash,
checkout, reset or branch); the orchestrator commits afterwards. Record
the pre/post hashes of every edited file and the command logs in this
Progress Log.

## Progress Log

(empty)
