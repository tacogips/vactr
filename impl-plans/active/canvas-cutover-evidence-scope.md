# Canvas Cutover: O(1) Scope and Label Lookups (Decision A) Implementation Plan

**Status**: In Progress
**Plan ID**: CANVAS-EVIDENCE-SCOPE (dispatch wave 5 of the session-277 run; runs alone; first of the serial chain SCOPE -> SCHED -> FRAMECOST -> CANVAS-EVIDENCE)
**Design Reference**: design-docs/specs/design-implementation.md#15.3.8.13 (scope record A; "Ownership and order"), section 20 Q1 (scope chain rules, unchanged)
**Manifest**: impl-plans/active/canvas-cutover-dispatch.json (entry `CANVAS-EVIDENCE-SCOPE`)
**Created**: 2026-10-05
**Last Updated**: 2026-10-06

---

## Intent and Context

The user wants the canvas editor to stay responsive on a 20,000-line document while audio
plays. The operator profile (`tmp/canvas-cutover/diag-wasm/REPORT.md`, sections 1-3) found that
`session_check` is quadratic in the number of bindings, and the shared `analyze` makes eval
(Run) slow for the same reason. Release `session_check` times: 2,500 lines 7.2 ms, 5,000 lines
28 ms, 10,000 lines 123 ms, 20,000 lines 182 ms. A comments-only 20,000-line document takes
2.1 ms. Two linear scans cause this:

1. `src/types/scope.rs`: `Scope.names` is a `Vec<(Rc<str>, Binding)>`. `Scopes::classify`
   (line 166), `Scopes::bind` (line 194), `Scopes::session_lookup` (line 204),
   `Scopes::lookup` (line 215) and `Scopes::update` (line 227) each scan it linearly per call.
   `Checker::declare` (line 342) calls `classify` and then `bind` for every binding. The caller
   `src/types/check.rs:68` calls `session_lookup` for every top-level `fn`/`let`.
2. `src/directives/attach.rs`: `Doc::top_of` (line 363) runs
   `self.targets.iter().position(|x| x.top == top)` for every call.
   `LabelRegistry::build` (`src/directives/labels.rs:89`) calls it once per target.

Design 15.3.8.13 A fixes both with O(1) lookups. Resolution order, the section 20 Q1 rules and
the order and text of diagnostics must stay identical. The design lifts the 15.3.8.12
"no Rust edit" rule for the files listed under Ownership, and only for them.

## Non-goals

- No change to any diagnostic code, message, severity, span or order. The 7.1.4 / Q1 scope
  model (prelude -> session -> fn/block; one binding per name per scope; child scopes may
  shadow) does not change.
- No change to `Doc::target_on_line`, `Doc::preceding`, the call-site `defs.iter().find` in
  `Doc::new`, or any other function not named here. Do not optimize anything the operator
  profile did not name.
- No change to `src/types/check.rs` unless the compiler requires it. No edit is expected
  there; it is listed only because the design authorizes it.
- No edit to `src/directives/labels.rs` unless the compiler requires it. No edit is expected
  there.
- No new dependency, no `Cargo.toml` or `Cargo.lock` change, no new `allow` or `expect`
  attribute, no crate-wide `cargo fmt`, no edit of `.agents/settings.local.json`.
- No TypeScript, harness, design-doc or manifest edit.

## Ownership

writePaths (concrete files; the directories are gitignored artifact roots):

- `src/types/scope.rs`
- `src/types/check.rs` (no edit expected)
- `src/types/tests/mod.rs` (one added line: `mod scope_cost;`)
- `src/types/tests/scope_cost.rs` (new)
- `src/directives/attach.rs`
- `src/directives/labels.rs` (no edit expected)
- `src/directives/tests/attach.rs` (tests appended; existing tests untouched)
- `src/directives/tests/labels.rs` (tests appended; existing tests untouched)
- `impl-plans/active/canvas-cutover-evidence-scope.md` (this plan's progress log and checkboxes only)
- artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules/.vite`, `tmp/canvas-cutover/scope`

sharedPaths: none.

Plan note: design 15.3.8.13 names "a new `src/types/tests/scope.rs` registered in
`src/types/tests/mod.rs`". `src/types/tests/scope.rs` already exists (109 lines of the
rebinding, shadowing and prelude tests that must stay unchanged). The new counter tests go
into a new file, `src/types/tests/scope_cost.rs`, so the existing file stays byte-identical.

## Contracts and Key Points

### A1. Per-scope name index (`src/types/scope.rs`)

- `struct Scope` keeps `names: Vec<(Rc<str>, Binding)>` (insertion order, used by `Debug`) and
  gains `index: HashMap<Rc<str>, usize>`. The index maps each name to its slot in `names`.
  Within one scope a name occurs at most once. `bind` already replaces in place, so the
  invariant holds today. `HashMap` is already imported in this file (it is used by
  `query_effects`).
- Add a private constructor (for example `Scope::empty()`) and use it in `Scopes::new` and
  `Scopes::push`. Do not change `truncate`: dropping a scope drops its index.
- Method behavior, which must be identical to today:
  - `classify(name, env)`:
    - inner scope `index.get(name)` -> `DeclNote::Rebinding(names[slot].1.span)`;
    - any outer scope `index.contains_key(name)` -> `Shadowing`;
    - then the unchanged `env.global` / `NativeTable` branches.
  - `bind(name, b)`: on an index hit, replace `names[slot].1 = b` (the slot does not move).
    On a miss, push and insert `(name, names.len() - 1)`.
  - `session_lookup(name)`: `stack[0].index.get(name)`.
  - `lookup(name)`: scan scopes innermost-first; the first scope whose index has the name
    wins.
  - `update(name, ...)`: the same innermost-first scope search, then mutate that slot.
- Keys are `Rc<str>`. Look up with `&str` through `HashMap::get(name)`; `Rc<str>: Borrow<str>`
  makes this work without allocation. Do not allocate a new `Rc<str>` per lookup.
- Test-only counter. Add one `thread_local!` cell, for example
  `static NAME_PROBES: Cell<u64>`, and two functions, `pub(crate) fn name_probes() -> u64` and
  `pub(crate) fn reset_name_probes()`, all under `#[cfg(test)]`. Every per-scope index probe in
  `classify`, `bind`, `session_lookup`, `lookup` and `update` adds 1. Production code must
  carry no counter. Use a `#[cfg(test)] fn note_probe()` that increments, plus a
  `#[cfg(not(test))] #[inline(always)] fn note_probe() {}`. Strict clippy must stay clean
  without `allow`. Imitate the `thread_local!` with a `const` initializer at
  `src/complete/mod.rs:149`.

### A2. Constant-time `top_of` (`src/directives/attach.rs`)

- `Doc` gains a private field, for example `top_first: Vec<usize>`. It is indexed by the
  top-level node index `top`. Each entry is the index of the first target with that `top` in
  the final, sorted `targets`, or `usize::MAX` when there is none.
- Compute it at the very end of `Doc::new`, after
  `doc.targets.sort_by_key(|t| (t.extent.start, t.first_line))` (line 287). A table computed
  before the sort is wrong.
- Rewrite `top_of(t)`:
  - When `top_first` covers `targets[t].top` with a real index, return it. This is one step.
  - Otherwise (a `Doc` built through `Doc::default()` and filled by hand), fall back to the
    current linear `position` scan, counting each element examined. The fallback keeps
    behavior identical for any hand-built `Doc`.
- `Doc` derives `Clone, Debug, Default`. The new field must keep all three derives compiling.
  `Doc` is built with a struct literal only inside `attach.rs` (line 228, `..Doc::default()`).
  `src/lsp/analysis.rs:221` builds a different `Doc` type and is unaffected.
- Test-only counter: `#[cfg(test)]` `TOP_OF_STEPS` with `top_of_steps()` and
  `reset_top_of_steps()`, in the same `note_*` pattern as A1. The fast path adds 1 per call;
  the fallback adds 1 per element examined.

## Tasks

### TASK-A1: Scope index
**Deliverables**: `src/types/scope.rs`, `src/types/tests/scope_cost.rs`, `src/types/tests/mod.rs`
**Completion criteria**: the A1 contract is implemented; the new tests pass; all existing
`types::tests::*` tests pass unmodified.

### TASK-A2: top_of table
**Deliverables**: `src/directives/attach.rs`, `src/directives/tests/attach.rs`, `src/directives/tests/labels.rs`
**Completion criteria**: the A2 contract is implemented; the new tests pass; all existing
`directives::tests::*` tests pass unmodified.

### TASK-A3: Verification and progress log
**Deliverables**: this plan's progress log, plus logs under `tmp/canvas-cutover/scope/`.

## Test Cases (add exactly these; all are counter-based, with no wall-clock assertion)

`src/types/tests/scope_cost.rs` (use the existing helpers in `src/types/tests/mod.rs`, such as
`check_src`):

- 4,000 versus 8,000 unique session `let`s. The source is lines of the form
  `let v<i> <i>`, joined with `\n`. Reset, check, read `c4`; reset, check, read `c8`.
  Expect `c4 > 0`, `c8 <= 4 * 8000` and `c8 as f64 <= 2.2 * c4 as f64`. Also expect the check
  of each document to produce no error diagnostics; imitate `assert_clean` in `mod.rs`.
- A rebinding still reports the first span after the index change. `let a 1\nlet a 2\nlet a 3`
  yields two `rebinding` diagnostics, at lines 2 and 3. The line-3 message names the byte offset
  of the line-2 binding, because `bind` replaced the slot in place. Assert the line-3 message
  text equals the one produced before the change. The implementer records it from the
  unmodified code first (record the pre-change output in the progress log).
- Shadowing across scopes still works: a `fn` parameter named like a session `let` produces
  one `shadowing` warning (imitate the existing cases in `scope.rs`, without editing that
  file).

`src/directives/tests/attach.rs` (appended):

- A document with nested targets: top-level `fn`, `inst` and `bus` forms with body targets, plus
  plain lines. For every target index `k`, `doc.top_of(k)` equals a reference computed in the
  test the old way (first index `j` with `targets[j].top == targets[k].top`).
- `Doc::default()` with targets pushed by hand: `top_of` still returns the reference answer
  through the fallback.

`src/directives/tests/labels.rs` (appended):

- 20,000 top-level `let a<i> <i>` lines. Reset `top_of_steps`, build the table through the
  existing `table(src)` helper, and expect `steps <= 2 * table.doc.targets.len()` and
  `table.doc.targets.len() >= 20000`. Before the fix this sum is quadratic, so the mutation run
  must fail.

## Pitfalls

- Do not replace `Vec` with `HashMap` alone. `Debug` and insertion order are kept by the
  `Vec`; the index sits beside it.
- `bind` must not push a second entry for an existing name, or `classify` and `update` would
  diverge from today.
- `lookup` and `update` must stay innermost-first across scopes.
- The `top_first` table must be built after the sort.
- Counters must not exist in non-test builds, including the wasm32 build. Run the wasm
  build to prove it.
- Do not run `cargo fmt`. Run `rustfmt --edition 2021 --check` on the touched files only.
  rustfmt follows `mod` declarations from `src/types/tests/mod.rs` into sibling test modules.
  If an untouched sibling reports a diff, record it in the progress log and leave that file
  alone; do not reformat it.

## Verification

Setup (not gating): `test -f tree-sitter-vact/tree-sitter-vact.wasm || mise run ts-build-wasm`.
At start, record `BASE=$(git rev-parse HEAD)` and the fresh-read sha256 values of every
writePath in `tmp/canvas-cutover/scope/intent.json`.

Gating, inside the sandbox (logs in `tmp/canvas-cutover/scope/`, each with its exit code):

| Command | Required evidence |
|---------|-------------------|
| `CARGO_TERM_QUIET=true cargo build` | exit 0 |
| `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings` | exit 0, no warnings, no new `allow`/`expect` (`git diff $BASE -- src \| grep -E '^\+.*#\[(allow\|expect)'` prints nothing) |
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run types::tests directives::tests` | exit 0; the new tests appear in the run (record their names and the counter values `c4`, `c8`, `steps`) |
| `rustfmt --edition 2021 --check src/types/scope.rs src/types/tests/scope_cost.rs src/directives/attach.rs src/directives/tests/attach.rs src/directives/tests/labels.rs` | exit 0 |
| `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` | exit 0 (also the debug wasm the vitest suite loads) |
| `cd editor && ./node_modules/.bin/vitest run` | exit 0 (default config; perf files excluded) |
| `git diff --name-only $BASE` | lists only this plan's writePaths |
| `git diff --exit-code $BASE -- src/types/tests/scope.rs src/types/tests/check_basic.rs src/types/tests/diags.rs` | exit 0 (existing scope, shadowing and prelude tests untouched) |

Outside the sandbox (verification and review step):

| Command | Required evidence |
|---------|-------------------|
| `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run` | exit 0 (a timeout kill is neither a pass nor a failure) |
| `cd editor && npm run test:perf` (alone on the host) | exit 0 |
| `cd editor && npm run check` | exit 0 |

Mutation evidence (reported separately, never gating): in a scratch copy under
`tmp/canvas-cutover/scope/`, or by temporarily restoring the linear scans in the working tree
and then restoring the fix (record the sha256 values before and after), run the two
counter tests. Each must exit nonzero. Record the command, the actual exit code and the log
path.

Optional diagnostic (not gating): copy `tmp/canvas-cutover/diag-wasm/scale.mjs` to
`tmp/canvas-cutover/scope/scale.mjs` and point its import at
`editor/test/e2e/fixtures/large-doc.mjs` (the original imports a removed worktree path). Run it
against a release wasm (`CARGO_PROFILE_RELEASE_STRIP=debuginfo cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features host-wasm`)
and record the four `session_check` medians next to the REPORT.md values.

## Overwrite and Drift Protocol

Before each edit, record the fresh-read sha256 in `tmp/canvas-cutover/scope/intent.json`;
after it, record the post-edit sha256 in `receipt.json`. If a file changed since the
fresh read and this plan did not change it, stop editing that file and report the drift. Repair
is serial after the join. Edit only this plan's progress log.

## Completion Criteria

- [x] `Scope` has the name index; `classify`, `bind`, `session_lookup`, `lookup` and `update` probe each scope in O(1)
- [x] `Doc::new` builds `top_first` after the sort; `top_of` is O(1), with the counted fallback for hand-built docs
- [x] New counter tests pass: `c8 <= 32000`, `c8 / c4 <= 2.2`, label build `steps <= 2 * targets`
- [x] Existing `types::tests` and `directives::tests` pass unmodified
- [x] Strict clippy, rustfmt check, cargo build, the wasm32 build and the default vitest suite pass on the current source
- [x] Outside the sandbox, full nextest and `npm run test:perf` pass
- [x] Mutation runs exit nonzero and are recorded separately
- [x] Progress log updated with commands, exit codes, log paths and sha256 values

## Progress Log

### Session: 2026-10-05 (session 277 plan authoring)
**Tasks Completed**: Plan authored from design 15.3.8.13 A and the operator profile. No source
edits.

### Session: 2026-10-06 (CANVAS-EVIDENCE-SCOPE implementation)
**Tasks Completed**: Added a per-scope `HashMap<Rc<str>, usize>` while retaining ordered `names`,
and built `Doc::top_first` after sorting. Added the counter tests and compatibility cases. The
existing scope/rebinding/shadowing/prelude test files remain unchanged. Source hashes after the
negative controls were restored exactly to their pre-mutation values: `scope.rs`
`59fd855cf277f883f79f7ea7a1793f20c0016564b93eb39ae889b64d728be84f`; `attach.rs`
`28e041a2ac1e6e1cb857c51cb547ca9125e846f7e9e992ea614664fbe62240b0`; `scope_cost.rs`
`c26f8a517e506c14634fdb5606ad81de651b345a406c6078ae57a969de4cf460`; attachment tests
`8bf2a2980f70db47afa04a753d7b222e8312702496792b995d26ad99837a2b8d`; labels tests
`e11a8bc9a6c8f7c17963e0dde204b46fea4d4b7eee01383218e29c3b250ded35`.

The rebinding message contract from the unmodified `61c9216` implementation is
`` `a` is already bound in this scope (at byte 12); a name is bound once per scope ``: the
diagnostic uses the second line's identifier span, and `bind` replaces that slot. The final
behavior test confirms both diagnostics and that exact message.

**Verification** (complete logs are under `tmp/canvas-cutover/scope/`):
- `CARGO_TERM_QUIET=true cargo build`: exit 0, `agent-build-final.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`: exit 0,
  `agent-clippy-final.log`; no added `allow`/`expect` attributes.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-capture types::tests directives::tests`:
  exit 0, 184 passed/0 failed, `agent-nextest-final.log`; scope probes `c4=16000`,
  `c8=32000`; label build `steps=20000`, `targets=20000`.
- `rustfmt --edition 2021 --check src/types/scope.rs src/types/tests/scope_cost.rs src/directives/attach.rs src/directives/tests/attach.rs src/directives/tests/labels.rs`:
  exit 0, `agent-rustfmt-final.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`:
  exit 0 on final production source, `wasm-final.log`.
- `git diff --exit-code 61c9216ef1409324e47a832dcd5c6864464629cb -- src/types/tests/scope.rs src/types/tests/check_basic.rs src/types/tests/diags.rs`:
  exit 0. Changed tracked paths are only `src/types/scope.rs`, `src/types/tests/mod.rs`,
  `src/directives/attach.rs`, `src/directives/tests/attach.rs`, and
  `src/directives/tests/labels.rs`; the new test file is `src/types/tests/scope_cost.rs`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`:
  exit 0, 2816 passed/0 failed, 3 skipped, `full-nextest.log` (1859.138 s).
- `cd editor && npm run check`: exit 0, `npm-check.log`.
- `cd editor && npm run test:perf`: exit 0, 1 passed/0 failed, `test-perf.log` (serial gate; ratio 2.81).
- Mutation controls, separately from gating runs: scope linear-scan mutation failed with
  `c8=95996000` (exit 100, `mutation-scope.log`); `top_of` linear-scan mutation failed with
  `steps=200010000` for 20000 targets (exit 100, `mutation-topof.log`). Both production files
  were restored to the hashes listed above.
- Default `cd editor && ./node_modules/.bin/vitest run` did not pass in three attempts:
  `vitest.log` (exit 1, 702/703; native probe stale-sample assertion), `vitest-rerun.log`
  (exit 1, 701/703; two tree-sitter edit tests timed out), and `vitest-rerun-2.log` (exit 1,
  699/703; native probe stale-sample assertion, the two tree-sitter timeouts, and the first-track
  test timeout). These tests are outside this plan's `writePaths`; no frontend assertions were
  removed or altered. This required gate remains open for the owner of those test seams.

**Outstanding**: Default vitest is a required gate and is failing in out-of-plan editor tests.
No source changes were made outside this plan's write paths. Resume acceptance after the owner
resolves the observed test failures and a full default vitest run exits 0. Initial offset and
formatting failures are retained in `agent-nextest.log` and `agent-rustfmt.log`; corrected final
source gates are recorded separately above.

### Session: 2026-10-06 (session 282 resume: re-gate only)
**Context**: The implementation is committed in `c69314d`. Operator repair `acf33e2` (outside
this plan) made the three load-sensitive tests robust: the `main.test.ts` native probe runs on
fake timers, and the syntax/syntax-core 200-edit equivalence and first-track tests got
load-tolerant timeouts. No assertion was weakened. The operator then ran full vitest 703/703
three times in a row.

**Session-282 scope (gates only):**
- Re-run the gating commands of the Verification section on the current HEAD (`acf33e2`, or
  the session-282 checkpoint commit on top of it). Logs go to
  `tmp/canvas-cutover/scope/s282-*.log`.
- Set `BASE=61c9216ef1409324e47a832dcd5c6864464629cb`, the session-277 checkpoint before
  `c69314d`. Use it only for these two gates, which are unchanged:
  - `git diff $BASE -- src | grep -E '^\+.*#\[(allow|expect)'` prints nothing;
  - `git diff --exit-code $BASE -- src/types/tests/scope.rs src/types/tests/check_basic.rs src/types/tests/diags.rs`
    exits 0.
- The single-BASE row `git diff --name-only $BASE` is replaced, for this session only, by the
  three exact checks below. Two commits sit between BASE and HEAD that are not this plan's
  work: operator repair `acf33e2` and the session-282 checkpoint. Record each command's output
  in the log.
  - (a) Implementation scope: `git diff --name-only 61c9216 c69314d` prints exactly these 7
    paths, all in this plan's writePaths:
    - `impl-plans/active/canvas-cutover-evidence-scope.md`
    - `src/directives/attach.rs`
    - `src/directives/tests/attach.rs`
    - `src/directives/tests/labels.rs`
    - `src/types/scope.rs`
    - `src/types/tests/mod.rs`
    - `src/types/tests/scope_cost.rs`

    The author verified this output on 2026-10-06.
  - (b) Later changes: `git diff --name-only c69314d` (HEAD plus working tree) prints a subset
    of the 9 paths below, and nothing else:
    - the 5 operator-repair test files from `acf33e2` (all 5 must appear):
      - `editor/test/app/main.test.ts`
      - `editor/test/code/syntax-core.test.ts`
      - `editor/test/code/syntax.test.ts`
      - `editor/test/wasm/abi.test.ts`
      - `editor/test/wasm/first-track.test.ts`
    - the 4 session-282 checkpoint files:
      - `design-docs/specs/design-implementation.md`
      - `impl-plans/active/canvas-cutover-dispatch.json`
      - `impl-plans/active/canvas-cutover-evidence-framecost.md`
      - `impl-plans/active/canvas-cutover-evidence-scope.md`

    The author verified `git diff --name-only c69314d acf33e2` = exactly the 5 test files.
  - (c) No new files: `git ls-files --others --exclude-standard` prints nothing (gitignored
    `tmp/`, `target/` and the excluded `.agents/settings.local.json` do not appear).
- Produce a new fingerprint: `git rev-parse HEAD` plus the sha256 of each of these 6 source
  files (the code `c69314d` touched):
  - `src/types/scope.rs`
  - `src/types/tests/mod.rs`
  - `src/types/tests/scope_cost.rs`
  - `src/directives/attach.rs`
  - `src/directives/tests/attach.rs`
  - `src/directives/tests/labels.rs`

  They must equal the `c69314d` blobs: `git diff --exit-code c69314d -- <the 6 files>` exits 0.
- No source edit is expected. If a gate fails inside this plan's writePaths, fix it there and
  record it. If a failure is outside the writePaths, report it with the log path and do not
  fix it.
- The default vitest gate must exit 0. Outside the sandbox, full nextest (`timeout 2400`) and
  `npm run test:perf` (alone on the host) must exit 0.
- Then the test-integrity, adversarial and integration reviews run. Mutation controls stay
  reported separately, and the c69314d logs may be cited for them.

**Done when**: every gating command above exits 0 on the current HEAD, with the complete log
path recorded here, and the reviewers accept. Then tick the open completion criterion.

### Session: 2026-10-06 (session 283: implementation gate closeout)
**Tasks Completed**: Re-gated the assigned scope implementation on checkpoint `2d0a748` plus
the local test-only correction. The focused review found the shadowing row asserted the
presence of a warning but not that it was the only diagnostic. `src/types/tests/scope_cost.rs`
now asserts the exact ordered result `shadowing@2` and Warning severity. Its final SHA-256 is
`53a3b90f9be1fd3305c955b290361b1f93f8490c84449e5440384813113eb1f3`; `src/types/scope.rs`,
`src/directives/attach.rs`, and the other test files retain their accepted implementation
hashes. Pre-edit and post-edit intent records are under `tmp/canvas-cutover/scope/`.

**Current-source verification** (complete logs under `tmp/canvas-cutover/scope/`):
- `CARGO_TERM_QUIET=true cargo build`: exit 0, `s282-final-build.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`: exit 0,
  `s282-final-clippy.log`; `s282-final-allow-expect.log` confirms no added `allow`/`expect`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run types::tests directives::tests`:
  exit 0, 184 passed/0 failed, `s282-final-nextest.log`; same-source post-modification
  confirmation is also in `postmod-final/nextest.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-capture types::tests directives::tests`:
  exit 0, 184 passed/0 failed; current-source counters are `c4=16000`, `c8=32000`,
  `steps=20000`, `targets=20000`, `s283-final-counter-nextest.log`.
- `rustfmt --edition 2021 --check src/types/scope.rs src/types/tests/scope_cost.rs src/directives/attach.rs src/directives/tests/attach.rs src/directives/tests/labels.rs`:
  exit 0, `s282-final-rustfmt.log`; post-modification confirmation is in
  `postmod-final/rustfmt.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`:
  exit 0, `s282-final-wasm-build.log`.
- `cd editor && ./node_modules/.bin/vitest run`: exit 0, 90 files and 703 tests passed,
  `s282-final-vitest.log`.
- `cd editor && npm run check`: exit 0, `s282-final-npm-check.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`:
  exit 0, 2816 passed/0 failed and 3 skipped in 1105.531 seconds,
  `s282-final-full-nextest.log`.
- `cd editor && npm run test:perf` (alone after the full nextest run): exit 0, 1 passed,
  `s282-final-test-perf.log` (5k median 844.2 ms, 20k median 2275.2 ms, ratio 2.70).
- Scope checks: `git diff --name-only 61c9216 c69314d` exits 0 and lists the seven original
  SCOPE paths (`s282-final-impl-scope.log`). `git diff --name-only c69314d` exits 0 and lists
  the five operator-repair tests, four session-282 checkpoint files, and the in-scope
  `src/types/tests/scope_cost.rs` assertion correction (`s282-final-post-impl-scope.log`).
  `git ls-files --others --exclude-standard` is empty (exit 0,
  `s282-final-untracked.log`); existing scope/rebinding/shadowing/prelude test files remain
  unchanged (exit 0, `s282-final-existing-tests.log`).
- Current-source mutation controls are separate from gating runs and both fail as expected:
  the linear `classify` mutant (`src/types/scope.rs` SHA-256
  `0327027d07aec4536b90d0dbe970d4222b640c46a7e3425f45f8b3d2d7521e90`) reports
  `c4=8010000`, `c8=32020000` and fails its counter assertion (command exit 100,
  `s283-mutation-scope.log`); the linear `top_of` mutant (`src/directives/attach.rs`
  SHA-256 `ba5720ef8230afd78cc97df676dc1ed11cdfb3462a3295bbf4de9b9a66d3bcb5`) reports
  `steps=200010000` for 20000 targets and fails its assertion (command exit 100,
  `s283-mutation-topof.log`). Both files were restored to their pre-mutation SHA-256 values
  (`scope.rs` `59fd855cf277f883f79f7ea7a1793f20c0016564b93eb39ae889b64d728be84f`,
  `attach.rs` `28e041a2ac1e6e1cb857c51cb547ca9125e846f7e9e992ea614664fbe62240b0`). After
  restoration, the current-source counter suites pass: scope tests 3/3 with `c4=16000`,
  `c8=32000` (`postmutation-final/scope-counter.log`); label tests 5/5 with 20000 steps
  for 20000 targets (`postmutation-final/labels-counter.log`); rustfmt check exits 0
  (`postmutation-final/rustfmt.log`). Earlier controls are retained in `mutation-scope.log`
  and `mutation-topof.log`.

Implementation gates for this plan are complete. Status stays `In Progress` until the
downstream test-integrity, adversarial and integration reviews; later review-dependent
documentation, archive/index updates, commit and push are also downstream workflow steps.

### Session: 2026-10-06 (session 283 resume: re-gate on 5e58d04, no negative controls)
**Why this amendment**: Commit `5e58d04` changed `src/types/tests/scope_cost.rs` (stricter
shadowing row: exact `["shadowing@2"]` plus Warning severity). Two session-282 checks
therefore no longer hold literally: check (b) does not list that file, and
`git diff --exit-code c69314d -- <the 6 files>` now exits 1. Running them unchanged would put
a failing command into the gating list. This section replaces checks (b) and the c69314d
byte-identity check for session 283. Every other session-282 rule stays.

**Hard rule (workflowInput, session 283)**: run no mutation or negative-control command. The
existing controls are historical evidence. Cite them in prose only, never in `verification[]`
or `priorVerification[]`: `tmp/canvas-cutover/scope/s283-mutation-scope.log`,
`s283-mutation-topof.log`, `mutation-scope.log` and `mutation-topof.log`. The completion
criterion "Mutation runs exit nonzero and are recorded separately" is satisfied by those logs.
Structured verification lists only final-source commands that exited 0 with positive test
counts. Superseded, typo, timed-out or failed attempts never appear there. If a gate fails,
fix it inside the writePaths, rerun it, and report only the final passing run.

**Gating checks on the current HEAD** (the session-283 checkpoint commit on top of `5e58d04`;
logs go to `tmp/canvas-cutover/scope/s283-gate-*.log`):
- The Verification-section commands: cargo build, strict clippy, focused nextest
  `types::tests directives::tests` (184 passed; `--no-capture` shows `c4=16000`, `c8=32000`,
  `steps=20000`, `targets=20000`), rustfmt check on the 5 listed files, host-wasm wasm32
  build, default vitest (703 or more passed), `npm run check`.
- The BASE=`61c9216` gates are unchanged: the allow/expect grep prints nothing, and
  `git diff --exit-code 61c9216 -- src/types/tests/scope.rs src/types/tests/check_basic.rs src/types/tests/diags.rs`
  exits 0.
- (a) is unchanged: `git diff --name-only 61c9216 c69314d` prints exactly the 7 SCOPE paths.
- (b') `git diff --name-only c69314d 5e58d04` prints exactly these 10 paths (author-verified
  2026-10-06): the 5 acf33e2 test files, the 4 session-282 checkpoint files, and
  `src/types/tests/scope_cost.rs`.
- (b'') `git diff --name-only 5e58d04 -- src editor Cargo.toml Cargo.lock mise.toml` prints
  nothing. Only the session-283 plan checkpoint files may differ from `5e58d04`.
- (c) `git ls-files --others --exclude-standard` prints nothing.
- Byte identity: `git diff --exit-code 5e58d04 -- src/types/scope.rs src/types/tests/mod.rs src/types/tests/scope_cost.rs src/directives/attach.rs src/directives/tests/attach.rs src/directives/tests/labels.rs`
  exits 0. Record `git rev-parse HEAD` and the sha256 of each of the 6 files as the new
  fingerprint. `scope_cost.rs` must be `53a3b90f9be1fd3305c955b290361b1f93f8490c84449e5440384813113eb1f3`.
- Outside the sandbox: full nextest with `timeout 2400` (exit 0, at least 2816 passed), then
  `cd editor && npm run test:perf` alone on the host (exit 0).
- Then the test-integrity, adversarial and integration reviews run.

### Session: 2026-10-06 (Step 6 current-source execution on checkpoint dfe5f6b)
**Source identity**: `git rev-parse HEAD` is
`dfe5f6b676241f4ea0994e7aec0c65664058d953`; the six implementation/test source files are
byte-identical to `5e58d04`. SHA-256 values: `src/types/scope.rs`
`59fd855cf277f883f79f7ea7a1793f20c0016564b93eb39ae889b64d728be84f`,
`src/types/tests/mod.rs` `8356d5489a72176e31cfdbdf0c41a23ea85b5194668c1c2c96b43ca8624b6322`,
`src/types/tests/scope_cost.rs`
`53a3b90f9be1fd3305c955b290361b1f93f8490c84449e5440384813113eb1f3`,
`src/directives/attach.rs` `28e041a2ac1e6e1cb857c51cb547ca9125e846f7e9e992ea614664fbe62240b0`,
`src/directives/tests/attach.rs`
`8bf2a2980f70db47afa04a753d7b222e8312702496792b995d26ad99837a2b8d`,
`src/directives/tests/labels.rs`
`e11a8bc9a6c8f7c17963e0dde204b46fea4d7b7eee01383218e29c3b250ded35`.

**Final-source verification** (complete logs under `tmp/canvas-cutover/scope/`):
- `CARGO_TERM_QUIET=true cargo build`: exit 0, `s283-gate-build-current.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`: exit 0,
  `s283-gate-clippy-final.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-capture types::tests directives::tests`:
  exit 0, 184 passed/0 failed; counters `c4=16000`, `c8=32000`, `steps=20000`,
  `targets=20000`; `s283-gate-focused-nextest.log`.
- `rustfmt --edition 2021 --check src/types/scope.rs src/types/tests/scope_cost.rs src/directives/attach.rs src/directives/tests/attach.rs src/directives/tests/labels.rs`:
  exit 0, `s283-gate-rustfmt.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`:
  exit 0, `s283-gate-wasm-build.log`.
- `cd editor && ./node_modules/.bin/vitest run`: exit 0, 90 files and 703 tests passed,
  `s283-gate-vitest.log`.
- `cd editor && npm run check`: exit 0, `s283-gate-npm-check.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`:
  exit 0, 2816 passed/0 failed and 3 skipped in 1010.466 seconds,
  `s283-gate-full-nextest.log`.
- `cd editor && npm run test:perf` (run alone after full nextest): exit 0, 1 passed/0 failed;
  5k median 804.9 ms, 20k median 2262.5 ms, ratio 2.81;
  `s283-gate-test-perf.log`.
- Scope identity and hygiene checks all exit 0: seven original SCOPE paths only in
  `s283-gate-original-scope.log`; ten expected resume paths in
  `s283-gate-resume-scope.log`; no source drift from `5e58d04` in
  `s283-gate-no-product-drift.log`; exact source identity in
  `s283-gate-source-identity.log`; existing scope/rebinding/shadowing/prelude tests unchanged
  in `s283-gate-existing-tests.log`; no untracked files in `s283-gate-untracked.log`; no added
  `allow`/`expect` attributes in `s283-gate-allow-expect.log`.
- Mutation commands were not run in this resume, per the workflow hard rule. Historical
  sensitivity evidence remains in `s283-mutation-scope.log` and `s283-mutation-topof.log`;
  those paths are cited here as prose evidence only.

Implementation gates are complete on this source. Status remains `In Progress` pending the
downstream test-integrity, adversarial and integration reviews; review-dependent documentation,
archive/index updates, commit and push remain owned by later workflow steps.


### Session: 2026-10-06 (session 284: current-source implementation re-gate)
**Source identity**: `git rev-parse HEAD` is
`8d65ddc7f74437def874a43ba0d310da32f7b7f3`. No implementation source was changed;
the six implementation/test files remain byte-identical to `5e58d04`. SHA-256 values:
`src/types/scope.rs` `59fd855cf277f883f79f7ea7a1793f20c0016564b93eb39ae889b64d728be84f`,
`src/types/tests/mod.rs` `8356d5489a72176e31cfdbdf0c41a23ea85b5194668c1c2c96b43ca8624b6322`,
`src/types/tests/scope_cost.rs` `53a3b90f9be1fd3305c955b290361b1f93f8490c84449e5440384813113eb1f3`,
`src/directives/attach.rs` `28e041a2ac1e6e1cb857c51cb547ca9125e846f7e9e992ea614664fbe62240b0`,
`src/directives/tests/attach.rs` `8bf2a2980f70db47afa04a753d7b222e8312702496792b995d26ad99837a2b8d`,
`src/directives/tests/labels.rs` `e11a8bc9a6c8f7c17963e0dde204b46fea4d7b7eee01383218e29c3b250ded35`.
Fresh-read hashes for every plan writePath and the intended progress hunk are recorded in
`tmp/canvas-cutover/scope/intent-session-284.json` and
`tmp/canvas-cutover/scope/intent-plan-session-284.md`.

**Final-source verification** (complete logs include `exitStatus=0` and are under
`tmp/canvas-cutover/scope/`):
- `CARGO_TERM_QUIET=true cargo build`: exit 0, `s284-build.log`.
- `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`: exit 0,
  `s284-clippy.log`; no added `allow`/`expect` attributes, `s284-allow-expect.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --no-capture types::tests directives::tests`:
  exit 0, 184 passed/0 failed; `c4=16000`, `c8=32000`, `steps=20000`, `targets=20000`;
  `s284-focused-nextest.log`.
- `rustfmt --edition 2021 --check src/types/scope.rs src/types/tests/scope_cost.rs src/directives/attach.rs src/directives/tests/attach.rs src/directives/tests/labels.rs`:
  exit 0, `s284-rustfmt.log`.
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`:
  exit 0, `s284-wasm-build.log`.
- `cd editor && ./node_modules/.bin/vitest run`: exit 0, 90 files and 703 tests passed,
  `s284-vitest.log`.
- `cd editor && npm run check`: exit 0, `s284-npm-check.log`.
- `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true timeout 2400 cargo nextest run`:
  exit 0, 2816 passed/0 failed and 3 skipped in 1634.296 seconds,
  `s284-full-nextest-final.log`. An earlier invocation's tests also completed green, but its
  zsh wrapper could not capture status because `status` is reserved; this corrected rerun is
  authoritative.
- `cd editor && npm run test:perf` (run alone after full nextest): exit 0, 1 passed/0 failed;
  5k median 1310.8 ms, 20k median 3907.8 ms, ratio 2.98; `s284-test-perf.log`.
- Scope identity and hygiene checks all passed: seven original SCOPE paths in
  `s284-original-scope.log`; ten expected resume paths in `s284-resume-scope.log`; no source
  drift from `5e58d04` in `s284-no-product-drift.log`; byte identity in
  `s284-source-identity.log`; existing scope/rebinding/shadowing/prelude tests unchanged in
  `s284-existing-tests.log`; no untracked files in `s284-untracked.log`; no added
  `allow`/`expect` attributes in `s284-allow-expect.log`.
- No mutation or negative-control command was run. Historical sensitivity evidence remains in
  `s283-mutation-scope.log`, `s283-mutation-topof.log`, `mutation-scope.log` and
  `mutation-topof.log`, cited as prose only.

Implementation gates are complete on this source. Status remains `In Progress` pending the
independent test-integrity, adversarial and integration reviews; review-dependent documentation,
archive/index updates, commit and push remain later workflow steps.
