# FM1V-21: `fm6-sysex` Native and `SourceLoader::read_bytes`

**Status**: In Progress (session 352 redispatch: fixture repair and verification)
**Plan ID**: FM1V-21 (run 2, serial wave 1 of 6)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("SysEx import" -> "Native", "Usage"; user question FV6)
**Created**: 2026-10-10
**Last Updated**: 2026-10-10 (session 352)

## Session 352 Redispatch (read first)

The source for this plan is already in the tree at `25d3e80` and is
unreviewed. Do not rewrite it. This redispatch has three jobs:

1. **Audit** the committed files against "File-level Changes" below, and
   fix any deviation inside writePaths.
2. **Repair the invalid fixture** at `src/ns/tests/fm6_sysex.rs:92-93`.
   Vactr has no `( )` grouping. The previous run failed with
   `error[paren-form] 1:4..5: ( ) is not Vactr syntax; use {}` (log
   `tmp/fm1-voices/FM1V-20/nextest-final-source-03.log`). Rewrite only
   these two assertions:
   - `len (fm6-sysex ./bank.syx)` -> `len {fm6-sysex ./bank.syx}` (expect
     `"32"`);
   - `len ((fm6-sysex ./bank.syx) 0)` -> `fm6-sysex ./bank.syx > first > len`
     (expect `"155"`).

   Both forms are valid. `{}` groups a call (lang-reference.md:27,
   `src/ns/tests/reactive_basic.rs:340`). `>` threads the left value in as
   the first argument. `first` is a list native
   (`src/types/natives.rs:302`), and `len` is `fn any -> int`.

   If either form fails for a reason other than syntax, find the cause in
   this plan's native code and fix it there. Do not change the expected
   values, and do not delete the assertion. Keep the existing
   `voices.items.len() == 32` and `assert_patch_value` checks.
3. **Verify.** Run Verification 1-7, then record them in the Progress Log.

Scope check for this redispatch:

- At start, save `git status --porcelain=v1` to
  `tmp/fm1-voices/FM1V-21/status-before.txt`.
- At the end, every path that is new or changed against that snapshot must
  be in writePaths.
- This replaces the bare `git diff --stat` check. Code from earlier waves
  is already committed at `25d3e80`.

The serial order for run 2 is FM1V-21 -> FM1V-20 -> FM1V-30 -> FM1V-40 ->
FM1V-50 -> FM1V-51. No other plan runs at the same time. A build failure
outside writePaths is therefore a real defect: record it in the Progress
Log with the log path, and do not wait for a sibling plan.

## Intent and Context

Users load their own six-operator SysEx files from `.vact` code (path
literal, no `=`):

```
let bank fm6-sysex ./my-patches.syx
```

`bank` is a list of voices, and each voice is a list of 155 ints. The bytes
are read on the evaluator thread through a new `SourceLoader::read_bytes`
method:

- only the native loader implements it, with the same path resolution and
  containment as `read` and a 64 KiB cap;
- the session loader delegates to its inner loader;
- every other loader, including the browser, keeps the default, which fails
  with `host-unavailable` (FV6).

The bytes are parsed by FM1V-12's `fm::sysex::parse`.

## Non-goals

- No UGen or registry change. Rendering a patch is FM1V-40.
- No browser file loading.
- No change to `load` behaviour.
- No song-mode support: song loaders keep the default `host-unavailable`.
- Do not edit `src/types/natives_domain.rs`, which belongs to the registry
  plans.

## Dependencies

- **dependsOn**: FM1V-12 (`parse`, `SysexError` and its `Display`, the
  test-only `encode_single`/`encode_bulk`)
- **Blocks**: FM1V-20 (its canonical filter includes this plan's
  `fm6_sysex` tests) and FM1V-40

## writePaths

- `src/ns/fm6_sysex.rs` (new)
- `src/ns/load.rs`
- `src/ns/mod.rs`
- `src/ns/tests/mod.rs`
- `src/ns/tests/fm6_sysex.rs` (new)
- `src/types/natives.rs`
- `src/host/native/loader.rs`
- `src/session/session.rs`
- `src/vm/tests/native_table.rs`
- `impl-plans/active/fm1-voices-21-sysex-native.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-21` (artifact root)

## sharedPaths

None. The sibling run wf/fm1-tuning also appends to `src/types/natives.rs`.
Put this entry directly after the `load` entry, not at the end of the
table, so the merge regions differ.

## Read-only References

- `src/ns/load.rs:76-140`: `register_load`, `load` (effect-mode check,
  `LoaderHost` downcast, failure codes).
- `src/host/native/loader.rs:165-265`: `contained`, `read_limited`,
  `NativeSampleLoader::read`/`resolve`.
- `src/session/session.rs:105-113`: `SessionLoader` delegation.
- `src/ns/tests/load.rs` and `src/ns/tests/reactive_basic.rs::MapLoader`:
  the test loader and evaluator harness.
- A list-returning native, for building `Value::List`: find one with
  `grep -rn '"enumerate"' src/vm`.

## File-level Changes

**`src/ns/load.rs`**

- Add a trait method to `SourceLoader`. Its doc comment states that the
  default fails with `host-unavailable`.

  ```rust
  fn read_bytes(&mut self, path: &PathVal, limit: u64) -> Result<Vec<u8>, Failure> { /* default: Err(HostUnavailable) */ }
  ```

- `register_load` also registers `fm6-sysex` (`crate::ns::fm6_sysex::fm6_sysex`),
  with the same `slot(..).is_none()` guard.

**`src/ns/fm6_sysex.rs`**

- `pub(crate) const MAX_SYSEX_BYTES: u64 = 64 * 1024;`
- `pub(crate) fn fm6_sysex(cx: &mut NativeCx<'_>, args: &[Value], kw: &[(KwId, Value)]) -> Result<Value, Failure>`.
  It mirrors `load`:
  - query mode gives `EffectInQuery` with the message
    "`fm6-sysex` is not allowed inside a query";
  - a non-path argument gives `FailCode::Type`;
  - with no `LoaderHost`, it gives `HostUnavailable`.
  - Otherwise it calls `loader.read_bytes(&path, MAX_SYSEX_BYTES)` and then
    `parse`.
  - A parse error gives
    `Failure::new(FailCode::LoadFailed, format!("fm6-sysex `{}`: {err}", path.text))`.
  - On success it returns a list of lists of `Value::Int`, one inner list
    per voice in file order, each holding 155 values in `patch.rs` order.

**`src/ns/mod.rs`**: add `pub mod fm6_sysex;`.

**`src/ns/tests/mod.rs`**: add `mod fm6_sysex;`.

**`src/types/natives.rs`**: right after the `load` entry, add
`f("fm6-sysex", 1, 1, &["fn path -> [[int]]"], &[V]).effect(),`.

**`src/host/native/loader.rs`**: implement `read_bytes` for
`NativeSampleLoader`:

- use `self.resolve(path)?` and then `read_limited(&file, limit, &path.text)`;
- do not register a `FileId`, because bytes are not source text.

**`src/session/session.rs`**: in `SessionLoader`,
`fn read_bytes(..) { self.inner.read_bytes(path, limit) }`.

**`src/vm/tests/native_table.rs`**: in
`the_full_prelude_is_the_table_minus_load`:

- the domain prelude now lacks both `load` and `fm6-sysex`;
- assert `table.len() - 2`;
- assert `p.slot(intern_sym("fm6-sysex")).is_none()`;
- rename the test to `the_full_prelude_is_the_table_minus_loader_natives`.

This is an exact count, not a loosened one.

## Pitfalls

- **Long-running nextest (command timeout).**
  - Full nextest takes about 800 to 1700 s. The single test
    `complete::tests::robust::every_prefix_and_mutant_is_panic_free` takes
    about 500 s.
  - The previous FM1V-30 run was killed by SIGTERM at about 1200 s
    (`tmp/fm1-voices/FM1V-30/focused-final-blessed.log`, exitStatus 100).
  - Run the lock-wrapped full nextest in the foreground with the executor
    command timeout set to at least 3600 s, or to its maximum. Poll it
    until it exits, and never background it.
  - A SIGTERM or harness kill is neither a pass nor a code failure. Rerun
    the same command once with the long timeout. Keep both logs as
    `tmp/fm1-voices/<planId>/attempt-<n>/full.log` and record both
    attempts.
  - Never skip, ignore or filter out tests to beat the timeout.

- **Respect the file limit.** Reading over the limit must fail through
  `read_limited`'s existing error, not by truncating.
- **Keep the existing `read` path unchanged.** Only add.
- **Parse on the evaluator thread.** Never touch audio-thread code.
- **Check `Value::Int`.** Use the integer constructor that natives such as
  `len` use. If `int` in the type signature requires a specific variant,
  match it, so that `fn path -> [[int]]` type-checks.
- **Expect related test failures.** `infer_call.rs`/types tests may
  enumerate effectful natives. Run the focused filters below and fix only
  real failures inside writePaths. Do not weaken assertions.

## Tests (`src/ns/tests/fm6_sysex.rs`; names contain `fm6_sysex`)

The test loader is a `MapLoader`-like struct that implements both `read`
and `read_bytes`, serving bytes from FM1V-12's `encode_single`/`encode_bulk`.

- `fm6_sysex_native_returns_bulk_voices`: evaluating
  `len {fm6-sysex ./bank.syx}` gives 32, and
  `fm6-sysex ./bank.syx > first > len` gives 155. The first voice's values
  equal the fixture's `params`.
- `fm6_sysex_native_returns_single_voice`: the length is 1.
- `fm6_sysex_native_reports_checksum_error`: corrupted bytes give a failure
  whose message contains `fm6-sysex` and `checksum`.
- `fm6_sysex_native_without_bytes_support_is_host_unavailable`: a loader
  with only `read` gives `FailCode::HostUnavailable`.
- `fm6_sysex_native_rejects_query_mode`, if the harness can express a query.
  Otherwise test it the way `load`'s query test does in
  `src/ns/tests/load.rs`.
- `fm6_sysex_native_type_signature`: the native table entry is effectful
  and its type prints as `fn path -> [[int]]`.
- `fm6_sysex_native_loader_enforces_limit`. Use `NativeSampleLoader` over a
  unique directory under `std::env::temp_dir()`, removed at the end of the
  test. It holds a 70 KiB file and a 4104-byte file. Reading the 70 KiB file
  fails, and the 4104-byte file reads back exactly. Do not write test files
  inside the repository.

## Verification (logs under `tmp/fm1-voices/FM1V-21/`)

1. `rustfmt --edition 2021 --check` on every `.rs` in writePaths must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6_sysex|native_table|natives|load|infer|no_abort|session/)' > tmp/fm1-voices/FM1V-21/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 7 tests from this plan and 0 failed.
3. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm > tmp/fm1-voices/FM1V-21/wasm.log 2>&1; echo "exit=$?"`
   must print `exit=0`. This proves the default trait method compiles for
   the browser.
4. Paths changed against `status-before.txt` are all in writePaths. Run
   `wc -l src/ns/fm6_sysex.rs src/ns/load.rs src/ns/tests/fm6_sysex.rs src/types/natives.rs src/host/native/loader.rs src/session/session.rs src/vm/tests/native_table.rs | awk '$2 != "total" && $1 >= 1000 {bad=1} END {exit bad}'`;
   it must exit 0.
5. `test "$(grep -c -F -e '((' -e 'len (' src/ns/tests/fm6_sysex.rs)" = 0`
   must exit 0. This shows that no paren grouping is left in the fixture.
6. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6|fm_mod|fm_algo/)' > tmp/fm1-voices/FM1V-21/nextest-fm-filter.log 2>&1; echo "exit=$?"`
   must print `exit=0` with 0 failed. This is FM1V-20's canonical filter. It
   shows that this plan no longer leaves it red.
7. Full nextest under the measurement lock:
   `bash -c 'L=/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock; until mkdir $L 2>/dev/null; do sleep 30; done; trap "rm -rf $L" EXIT; echo FM1V-21 > $L/owner; CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/fm1-voices/FM1V-21/full.log 2>&1; echo "exit=$?"'`
   must print `exit=0` with 0 failed. The full behavioral suite must be
   green before acceptance.
   - If a failure is in a test owned by a later plan (FM1V-30's
     `fm1_voice*` or FM1V-40's `fm6_registry`/`fm_mod_algorithm` stubs),
     record the test name and log path, and leave it for that plan. Do
     this only if the failure cannot be fixed inside this plan's
     writePaths.
   - The reviewer then decides acceptance with that evidence. Never loosen
     or skip a test to pass.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

- Fresh-read each file before editing it.
- Record the `shasum -a 256` before and after for `natives.rs`, `load.rs`,
  `loader.rs`, `session.rs` and `src/ns/tests/fm6_sysex.rs`.
- This plan runs alone (serial wave 1 of run 2). Never commit, stash,
  checkout, reset or push.

## Done Criteria

- [x] `read_bytes`, the native and its registration are implemented.
- [ ] The committed source is audited against this plan, and any
      deviations are fixed.
- [ ] The fixture at `src/ns/tests/fm6_sysex.rs:92-93` is rewritten with
      `{}` and `>` forms (Verification 5 prints 0).
- [ ] Verification 1-7 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.

### Session: 2026-10-10 (FM1V-21 implementation)
**Tasks Completed**: `SourceLoader::read_bytes`, native bounded byte reads,
`SessionLoader` delegation, effectful `fm6-sysex` registration and signature,
the exact loader-native table count test, and eight focused evaluator tests.
The evaluator tests cover synthetic single/bulk patches, nested integer-list
results, checksum errors, unsupported byte loading, query mode, path type,
signature and the native loader's 64 KiB limit using a temporary directory.

**Verification**:
- Rust formatting passed for every Rust file in this plan's `writePaths`;
  see `tmp/fm1-voices/FM1V-21/rustfmt-final.log`.
- Scoped whitespace/status/line-count checks passed. All changed Rust files
  are under 1000 lines; `session.rs` is the longest at 893 lines. SHA-256
  before/after values for the four required shared files are recorded in
  `tmp/fm1-voices/FM1V-21/source-hashes.log`.
- The focused nextest command is not yet verified: attempts in
  `nextest.log`, `nextest-postcheck-agent.log`, `nextest-retry-01.log`,
  `nextest-retry-02.log`, `nextest-retry-03.log` and
  `nextest-final-attempt.log` exited 101 before tests. Earlier compile
  failures were in sibling-owned `src/dsp/ugen/fm/engine.rs` and
  `src/dsp/tests/dsp/fm6_engine.rs`; the latest errors are in
  `src/dsp/tests/dsp/fm1_voice_registry.rs` and
  `src/host/tests/e2e/templates/fm1_voices.rs`. No FM1V-21 tests ran. The
  full output is in the corresponding logs, and the errors changed as
  sibling files were edited.
- The first wasm attempt (`wasm.log`) exited 101 during concurrent sibling
  compilation. The retry `wasm-retry-01.log` passed with exit 0; the build
  emitted only dead-code warnings from sibling metadata constants.
- Resume when sibling test compile errors are fixed and the shared tree
  compiles, then rerun the exact focused nextest filter. The wasm criterion
  is satisfied by `wasm-retry-01.log`.

**Tasks Remaining**: focused behavioral tests after sibling compile errors
are repaired. The wasm build, formatting, and scoped diff/file-limit checks
are complete.
