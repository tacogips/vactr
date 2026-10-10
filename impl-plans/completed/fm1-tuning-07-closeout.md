# FM1 Tuning 07: Full Gates and Index Closeout

**Status**: Completed
**Plan ID**: fm1-tuning-07-closeout
**Wave**: 5 (depends on fm1-tuning-06-docs-examples)
**Design Reference**: design-docs/specs/design-tuning-and-strum.md sections 10, 11
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

Run every acceptance gate serially on the integrated branch, prove the
invariants (bit-identical default, untouched sibling-owned files, line
limits), and add the plan-set entry to `impl-plans/README.md`. A sibling
run (`wf/fm1-voices`) is active in another worktree, so the full nextest
and every browser command run under the measurement lock. All tests are
silent. Never change the macOS volume.

## Non-goals

- No feature code beyond the run-3 fixes below. If a gate fails, fix it
  only inside files already listed in this plan's writePaths, or report
  the failing plan and file with its log. Do not loosen any gate or
  threshold.
- Do not move plans to `impl-plans/completed/` or commit/push here. The
  workflow's final integration step owns that.

## Working Rules

Serial, no parallel workers. Fresh read plus sha256 before and after
editing `impl-plans/README.md`, append only. No git writes.

## Files

| Path | Change |
| --- | --- |
| `impl-plans/README.md` | append one paragraph: "FM1 microtonal tuning and chord performance (design-tuning-and-strum.md)", linking the 7 plans |
| `impl-plans/active/fm1-tuning-07-closeout.md` | this plan's progress log and the verification table |
| `src/pattern/combinators/strum.rs` | run-3 fix F1 (design 5.1): in `query_strum`, validate the sampled `curve` keyword next to the existing `match direction.as_str()` (imitate its `_ => return Err(type_err(..))` arm); anything other than `flat`, `fade`, `swell` returns `type_err("strum curve must be :flat, :fade, or :swell")` before any child is built. No other change; valid curves keep their exact output |
| `src/pattern/combinators/strum/tests.rs` | run-3 test T1 (design 10.8) |
| `tests/song_tuning.rs` | run-3 test T2 (design 4.9, 10.6) |

### Run-3 fixes (before the gates)

Intent: close the session-351 low findings. An unknown strum curve
currently falls into the swell branch silently (`strum.rs`, the
`if curve != "flat"` / `if curve == "fade"` else-branch). Song freeze
already skips an unmapped tone (`src/song/source.rs`, `freq(..)` is none
-> `continue`), but no song test pins it.

F1 pitfalls:
- Do not add the check inside the per-child loop (it would fault only when
  a child survives `sect`); check once right after `keyword_param(curve, ..)`.
- Do not turn the swell branch into a `_` catch-all, and do not change the
  multiplier math, the `velocity`/`gain` target choice, or `keyword_param`.
- Scalar-note and no-note events must still pass through without sampling
  `dir` or `curve` (they leave at `chord_values(&e)` before the closure).

T1 cases (in the style of
`strum/tests.rs:float_time_faults_and_scalar_notes_pass_through`, using
its `strum`, `base_chord`, `ratio`, `keyword`, `run`, `cycle`, `s`, `kw`
helpers; `strum` takes a trailing `None` span argument, omitted below):
- `strum(base_chord(), ratio(1, 32), keyword("up"), keyword("bogus"))` ->
  no events, exactly one fault, message contains `curve`.
- `strum(base_chord(), ratio(1, 32), keyword("sideways"), keyword("flat"))`
  -> no events, one fault, message contains `direction`.
- the same unknown curve on `s(kw("bd"))` (scalar) -> one event, no fault.
- `keyword("swell")` on `base_chord()` -> events, no fault (still valid).

T2 case (new `#[test]` in `tests/song_tuning.rs`, reuse its `frozen`,
`playback_events`, `freq` helpers). The inline Scala and keymap are the
ones already proven live in
`src/sched/tests/sched/tuning_natives.rs:345`:
- song source `song {part [lead: {s :analog > tune {scala "two\n2\n3/2\n2/1" kbm: "2\n60\n61\n60\n60\n261.6255653005986\n2\n0\nx"} > note [60 61] > gain 0.2}] duration: 1} tail-seconds: 0 > play-song`
  (Rust string escaping as at that line) -> `frozen` returns exactly 1 row
  with `note == Some(ResolvedNote::Int(60))`; `playback_events` returns 1
  event whose `freq` bits are within 1 ulp of
  `note_to_freq(60.0) as f32`.
- the same song without the `tune {..}` step -> 2 frozen rows (60, 61), so
  the skip is caused by the keymap.
- If the song path rejects this source (not the expected skip), stop and
  report with the log; do not edit `src/song/source.rs` or other files.

Focused checks (two invocations, because a positional filter would also
filter the `song_tuning` binary and hide T2):
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib pattern::combinators::strum > tmp/fm1-tuning/p07/nextest-focused-strum.log 2>&1`
  -> exit 0, summary shows the new T1 test(s) passed.
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_tuning > tmp/fm1-tuning/p07/nextest-focused-song.log 2>&1`
  -> exit 0, testsRun is 3 (2 existing + T2).
- `rustfmt --edition 2021 --check src/pattern/combinators/strum.rs src/pattern/combinators/strum/tests.rs tests/song_tuning.rs` -> exit 0.
- `wc -l` on the three files -> each < 1000.
- Negative control for T1 (record under `mutationEvidence` only): with
  the curve validation temporarily removed, the unknown-curve case fails;
  restore and rerun the strum focused check green.
- `grep -rn 'curve' examples/ design-docs/specs/lang-reference.md` -> no
  example or fence uses a curve outside `:flat :fade :swell` (R3).

## Measurement lock

Wrap each locked command exactly like this (foreground; release
immediately afterwards):

```
bash -c 'until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done; trap "rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock" EXIT; echo fm1-tuning-closeout > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner; <COMMAND>'
```

## Gates (serial, in this order; logs in `tmp/fm1-tuning/p07/`)

| # | Command | Locked | Must show |
| --- | --- | --- | --- |
| 1 | `CARGO_TERM_QUIET=true cargo build > tmp/fm1-tuning/p07/build.log 2>&1` | no | exit 0 |
| 2 | `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings > tmp/fm1-tuning/p07/clippy.log 2>&1` | no | exit 0 |
| 3 | `cargo fmt -- --check > tmp/fm1-tuning/p07/fmt.log 2>&1` | no | exit 0 (if files untouched by this run fail, record the list and check only the touched files with `rustfmt --edition 2021 --check`) |
| 4 | `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/fm1-tuning/p07/nextest-full.log 2>&1` | yes | exit 0, failureCount 0; record testsRun/testsPassed from the summary |
| 5 | `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/fm1-tuning/p07/wasm-lib.log 2>&1` | no | exit 0 |
| 6 | `mise run build-wasm-release > tmp/fm1-tuning/p07/wasm-release.log 2>&1` | no | exit 0 |
| 7 | `cd editor && npm run check > ../tmp/fm1-tuning/p07/npm-check.log 2>&1` | no | exit 0 |
| 8 | `cd editor && ./node_modules/.bin/vitest run > ../tmp/fm1-tuning/p07/vitest-full.log 2>&1` | yes | exit 0, failureCount 0 |
| 9 | `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build > ../tmp/fm1-tuning/p07/dist-build.log 2>&1` | no | exit 0 |
| 10 | `cd editor && npm run test:style > ../tmp/fm1-tuning/p07/test-style.log 2>&1` | yes | exit 0 |

Invariant checks (each exit 0):

Baseline is the run-2 originalHead `dbad8c3b53f10262ee6113fa6efa9bbff17bca08`. It is source-identical to run 1's `9ac8d1f`: `git diff --stat 9ac8d1f dbad8c3 -- src` is empty.

- `git diff --exit-code dbad8c3b53f10262ee6113fa6efa9bbff17bca08 -- src/host/tests/e2e/templates/golden_digests.txt src/dsp/ugen/catalog.rs src/dsp/ugen/catalog/codec.rs src/prelude/templates.vact src/dsp/controls.rs THIRD_PARTY_NOTICES.md src/sched/cells.rs src/host/caps.rs`
- `git diff dbad8c3b53f10262ee6113fa6efa9bbff17bca08 -- src/types/natives_domain.rs` shows only added lines (`grep -c '^-[^-]'` on the diff = 0)
- `wc -l` on every Rust file changed since dbad8c3 or newly untracked (`git diff --name-only dbad8c3 -- '*.rs'` plus `git ls-files -o --exclude-standard -- '*.rs'`): each < 1000

Record every gate as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`. Intentional negative runs go under `mutationEvidence`, never in the gate list.

## Completion Criteria

- [x] F1 strum curve validation, T1 and T2 tests added; `nextest-focused-strum-final.log` (12/12) and `nextest-focused-song-final.log` (3/3) exit 0; rustfmt check and `wc -l` < 1000 pass; T1 negative control fails without the guard and is recorded under mutationEvidence
- [x] Gates 1-10 exit 0 with complete logs
- [x] All invariant checks pass
- [x] `impl-plans/README.md` entry added (applied by serial shared-index update R-354-README; 9/9 links resolve, `tmp/fm1-tuning/reconcile-354c/readme-links.log`)
- [x] Progress log updated with the verification table

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Waits for plans 01-06

### Session: 2026-10-10 (run 2 re-plan, session-351)
**Tasks Completed**: Rebased the invariant checks onto the run-2 originalHead dbad8c3 (source-identical to 9ac8d1f)
**Notes**: Waits for plans 01-06

### Session: 2026-10-10 (run 3 design step, session-354)
**Tasks Completed**: Narrow writePaths amendment for the session-351 low findings: F1 (unknown strum curve is a type fault), T1, T2 (song-freeze unmapped-tone skip)
**Notes**: Waits for plan 06 acceptance

### Session: 2026-10-10 (run 3 plan step, session-354)
**Tasks Completed**: Pinned T1/T2 inputs (T2 reuses the keymap proven at tuning_natives.rs:345) and split the focused nextest run into strum and song_tuning invocations (design-review finding S3-L1)
**Notes**: Waits for plan 06 acceptance

### Session: 2026-10-11 (run 3 implementation, session-354)
**Tasks Completed**: Implemented F1, T1 and T2. Focused strum (12/12), song_tuning (3/3), rustfmt, and the <1000-line checks passed. The T1 mutation control failed as expected without the curve guard.
**Gate evidence**:

| Gate | Command | Exit | Result | Log |
| --- | --- | ---: | --- | --- |
| 1 | `CARGO_TERM_QUIET=true cargo build` | 0 | passed | `tmp/fm1-tuning/p07/build.log` |
| 2 | `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | 0 | passed | `tmp/fm1-tuning/p07/clippy.log` |
| 3 | `cargo fmt -- --check` | 0 | passed | `tmp/fm1-tuning/p07/fmt.log` |
| 4 | `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` | 100 | failed: 2,884 passed, 1 failed, 3 skipped; `spec_fixtures::every_spec_block_has_exactly_one_entry` sees new lang-reference blocks 6 and 7 without manifest entries | `tmp/fm1-tuning/p07/nextest-full.log` |

**Notes**: Gates 5-10 and invariant checks were not started because the full suite exposed a required repair in `tests/spec_fixtures.rs` and `tests/fixtures/spec/manifest.toml`, both outside this plan's `writePaths`. Resume after an authorized checkpoint adds those paths and updates the fixture manifest/count expectations. `impl-plans/README.md` remains for post-review integration bookkeeping.

### Session: 2026-10-11 (closeout rerun)
**Tasks Completed**: Re-ran gates 1-10 after the shared fixture manifest and block-count repair appeared in the worktree; all gates passed. All three invariant checks passed.
**Gate evidence**:

| Gate | Command | Exit | Result | Log |
| --- | --- | ---: | --- | --- |
| 1 | `CARGO_TERM_QUIET=true cargo build` | 0 | passed | `tmp/fm1-tuning/p07/build-rerun-20261011.log` |
| 2 | `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | 0 | passed | `tmp/fm1-tuning/p07/clippy-rerun-20261011.log` |
| 3 | `cargo fmt -- --check` | 0 | passed | `tmp/fm1-tuning/p07/fmt-rerun-20261011.log` |
| 4 | `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` | 0 | passed: 2,888/2,888; 3 skipped | `tmp/fm1-tuning/p07/nextest-full-rerun-20261011.log` |
| 5 | `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | 0 | passed | `tmp/fm1-tuning/p07/wasm-lib-rerun-20261011.log` |
| 6 | `mise run build-wasm-release` | 0 | passed | `tmp/fm1-tuning/p07/wasm-release-rerun-20261011.log` |
| 7 | `cd editor && npm run check` | 0 | passed | `tmp/fm1-tuning/p07/npm-check-rerun-20261011.log` |
| 8 | `cd editor && ./node_modules/.bin/vitest run` | 0 | passed: 841/841 across 95 files | `tmp/fm1-tuning/p07/vitest-full-rerun-20261011.log` |
| 9 | `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build` | 0 | passed | `tmp/fm1-tuning/p07/dist-build-rerun-20261011.log` |
| 10 | `cd editor && npm run test:style` | 0 | passed: 4/4 combinations | `tmp/fm1-tuning/p07/test-style-rerun-20261011.log` |

**Machine-readable gate records**:

```json
[
  {"command":"CARGO_TERM_QUIET=true cargo build","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/build-rerun-20261011.log","notes":"non-test gate (no test cases); build completed successfully."},
  {"command":"CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/clippy-rerun-20261011.log","notes":"non-test gate (no test cases); strict Clippy completed successfully."},
  {"command":"cargo fmt -- --check","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/fmt-rerun-20261011.log","notes":"non-test gate (no test cases); formatting check passed."},
  {"command":"CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run","exitStatus":0,"testsRun":2888,"testsPassed":2888,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/nextest-full-rerun-20261011.log","notes":"The 3 skipped tests are pre-existing #[ignore] tests at both dbad8c3 and HEAD (tmp/fm1-tuning/reconcile-354c/skipped-tests.log); the preceding attempt is retained above as historical failure evidence."},
  {"command":"CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/wasm-lib-rerun-20261011.log","notes":"non-test gate (no test cases); WASM library build completed successfully."},
  {"command":"mise run build-wasm-release","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/wasm-release-rerun-20261011.log","notes":"non-test gate (no test cases); release WASM build completed successfully."},
  {"command":"cd editor && npm run check","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/npm-check-rerun-20261011.log","notes":"non-test gate (no test cases); editor check completed successfully."},
  {"command":"cd editor && ./node_modules/.bin/vitest run","exitStatus":0,"testsRun":841,"testsPassed":841,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/vitest-full-rerun-20261011.log","notes":"95 test files passed."},
  {"command":"cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build","exitStatus":0,"testsRun":0,"testsPassed":0,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/dist-build-rerun-20261011.log","notes":"non-test gate (no test cases); frontend build completed successfully."},
  {"command":"cd editor && npm run test:style","exitStatus":0,"testsRun":4,"testsPassed":4,"failureCount":0,"outcome":"passed","log":"tmp/fm1-tuning/p07/test-style-rerun-20261011.log","notes":"Style assertions passed for 4 engine/viewport combinations (chromium and webkit x 2 viewports); run under the measurement lock."}
]
```

**Invariant evidence**:

| Check | Exit | Log |
| --- | ---: | --- |
| Baseline-owned files unchanged versus dbad8c3 | 0 | `tmp/fm1-tuning/p07/invariant-unchanged-rerun-20261011.log` |
| `natives_domain.rs` additions only | 0 | `tmp/fm1-tuning/p07/invariant-natives-additions-only-rerun-20261011.log` |
| Every changed Rust file below 1,000 lines | 0 | `tmp/fm1-tuning/p07/invariant-rust-lines-rerun-20261011.log` |

**Notes**: The previous failed full-nextest attempt remains recorded above as historical evidence; the rerun passed after serial fixture repair R-354-SPECFIX added the entries for the new lang-reference blocks. The README index paragraph was applied by serial shared-index update R-354-README; 9/9 links resolve (`tmp/fm1-tuning/reconcile-354c/readme-links.log`).


### Session: 2026-10-11 (record correction)
**Tasks Completed**: Reconciled INT-354-P07-RECORD without changing source, tests, fixtures, or README.
**Notes**: Original plan-record SHA-256 before correction: `b8ae668e757fe447982b80abbe5f7f277d16e4bf291743ce7db90c407e62750c`; corrected record SHA-256 before this audit entry: `f3bcda451263452084539b1d25811b0a6cb9bf5d6fd44254d067facaa566b5d1`. test:style records 4/4 (`tmp/fm1-tuning/p07/test-style-rerun-20261011.log`); three pre-existing ignored nextest tests are attributed in `tmp/fm1-tuning/reconcile-354c/skipped-tests.log`. R-354-README and R-354-SPECFIX ownership is recorded above. All completion criteria are checked; status is Completed.
