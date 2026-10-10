# FM1 Tuning 07: Full Gates and Index Closeout

**Status**: Ready
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

- No feature code. If a gate fails, fix it only inside files already
  listed in this plan's writePaths, or report the failing plan and file
  with its log. Do not loosen any gate or threshold.
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

- `git diff --exit-code 9ac8d1fdd7961b2a47bb000ef612742593cf131d -- src/host/tests/e2e/templates/golden_digests.txt src/dsp/ugen/catalog.rs src/dsp/ugen/catalog/codec.rs src/prelude/templates.vact src/dsp/controls.rs THIRD_PARTY_NOTICES.md src/sched/cells.rs src/host/caps.rs`
- `git diff 9ac8d1fdd7961b2a47bb000ef612742593cf131d -- src/types/natives_domain.rs` shows only added lines (`grep -c '^-[^-]'` on the diff = 0)
- `wc -l` on every Rust file changed since 9ac8d1f (`git diff --name-only 9ac8d1f -- '*.rs'`): each < 1000

Record every gate as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`. Intentional negative runs go under `mutationEvidence`, never in the gate list.

## Completion Criteria

- [ ] Gates 1-10 exit 0 with complete logs
- [ ] All invariant checks pass
- [ ] `impl-plans/README.md` entry added
- [ ] Progress log updated with the verification table

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Waits for plans 01-06
