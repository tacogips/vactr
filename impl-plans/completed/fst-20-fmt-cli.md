# FST-20: `vactr fmt` CLI Verb

**Status**: Completed
**Plan ID**: FST-20 (wave 2; parallel with FST-21, FST-22, FST-23)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 3.7.1; `design-docs/specs/command.md` (the `fmt` verb, `--check` flag, exit codes 1/2/3)
**Dispatch**: `impl-plans/active/fst-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan exposes the FST-10 formatter as `vactr fmt [--check] <PATH>...`
and `vactr fmt [--check] -`:
- it rewrites a file in place only when the file changes;
- `-` reads stdin and writes the result to stdout;
- `--check` reports the files that would change and writes nothing.

`command.md` already documents the verb and its exit codes. This plan
implements them exactly.

## Non-goals

- No directory walking, no globbing and no `.vact` extension filter. A
  directory argument is an IO error.
- No change to other verbs, to `src/fmt/**` (FST-10), or to `mise.toml`
  (FST-11 already defines `fmt-vact-check`).
- No new crates, no atomic-rename machinery, no parallelism.
- Do not reformat any committed `.vact` file (with `--check`, the
  verification only reads them).

## Dependencies

- **dependsOn**: FST-10 (it uses `crate::fmt::{format, Formatted, Outcome}`)
- **Blocks**: FST-40

## writePaths

- `src/cli/args.rs`:
  - the `FmtInput` enum;
  - the `Command::Fmt` variant;
  - `parse_fmt`;
  - the `"fmt"` arm in `parse`;
  - the `fmt` lines of `USAGE`.
- `src/cli/mod.rs`: `mod fmt;` and the `Command::Fmt` dispatch arm, only.
- `src/cli/fmt.rs` (new): the verb.
- `src/cli/tests/mod.rs`: add `mod fmt;` only.
- `src/cli/tests/args.rs`: append the `fmt` parse tests.
- `src/cli/tests/fmt.rs` (new): the verb tests.
- This plan's Progress Log.

## sharedPaths (read-only)

- `src/fmt/mod.rs` (FST-10 contract)
- `src/cli/args.rs:parse_lsp`, which is the flag-loop pattern to imitate,
  and `flag_matches` and `usage`.
- `src/cli/get.rs` and `src/cli/run.rs`: how a verb takes `cwd` and
  returns an `i32`.
- `src/session/console.rs:format_diag`: the diagnostic text.
- `src/types/diag.rs:Diagnostic` (`span.start`, `message`, `code`, `severity`).

## Contract

```rust
// src/cli/args.rs
pub enum FmtInput { Stdin, Paths(Vec<PathBuf>) }        // derive like Command (PartialEq, Debug, Clone)
Command::Fmt { check: bool, input: FmtInput }
// src/cli/fmt.rs
pub(crate) fn main(check: bool, input: &FmtInput, cwd: &Path) -> i32;
pub(crate) fn run(check: bool, input: &FmtInput, cwd: &Path,
                  stdin: &mut dyn Read, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32;
```

`main` locks the real stdio and calls `run`. Tests call `run` directly.

## Implementation Key Points

1. **Parsing (`parse_fmt`).**
   - `--check` may appear anywhere, but only once; a second one is a
     usage error.
   - `-` alone means `Stdin`.
   - Any other argument starting with `-` is an unknown flag, which is a
     usage error (exit 2).
   - No input at all is a usage error.
   - `-` mixed with paths, or `-` given twice, is a usage error.
   - Keep paths in the given order and do not dedupe them.
   - The messages follow the existing style:
     `` unknown flag `--x` for `fmt` ``, `` `fmt` needs a path or `-` ``,
     `` `-` cannot be combined with paths for `fmt` ``.
2. **Paths.** Join relative paths onto `cwd`. Print a path exactly as
   the user gave it.
3. **Per file.**
   - Read the bytes and convert them to UTF-8. Invalid UTF-8 is an IO
     error, reported as `vactr: <path>: not UTF-8`.
   - Call `format`, then act on the outcome:
     - `Changed`, without `--check`: write the output back with
       `std::fs::write`.
     - `Changed`, with `--check`: print the path and a newline to stdout.
     - `Unchanged`: do nothing.
     - `Refused`: print each ERROR diagnostic to stderr and leave the
       file untouched.
   - Process every input even after errors.
4. **Diagnostic line.** Print it as
   `vactr: <path>:<line>:<col>: <severity>[<code>] <message>`.
   `line` and `col` are 1-based and computed from `span.start`, and
   `col` counts chars. Use `-` as the path for stdin. Print only the
   Error-severity diagnostics.
5. **Stdin.**
   - Read all of stdin.
   - Without `--check`: write `text` to stdout. That is the input
     unchanged when refused, and nothing extra is added.
   - With `--check`: write nothing to stdout.
   - Invalid UTF-8 on stdin is an IO error, and nothing goes to stdout.
6. **Exit code.** Decide it in this order:
   1. 1 if any IO error occurred.
   2. Otherwise 3 if any input was refused.
   3. Otherwise 1 if `--check` found a change.
   4. Otherwise 0.

   A usage error returns 2 through the existing `Err(UsageError)` arm,
   so do not duplicate it.
7. **Dispatch.** In `cli::main`, add
   `Ok(Command::Fmt { check, input }) => fmt::main(check, &input, &cwd)`.
   `fmt` is not feature-gated.
8. **USAGE.** Add the synopsis lines
   `vactr fmt [--check] <PATH>...` and `vactr fmt [--check] -`.
   Do not reorder the existing lines.
9. **Pitfalls.**
   - Never write the file when the outcome is `Unchanged` or
     `Refused`: the mtime must not change.
   - Never print the formatted text for file inputs.
   - Do not call `std::process::exit` inside `fmt.rs`.

## Tests

`src/cli/tests/args.rs` (append):
- `fmt a.vact` gives `Fmt { check: false, input: Paths([a.vact]) }`.
- `fmt --check a b` and `fmt a --check b` give `check: true` with paths `[a, b]`.
- `fmt -` gives `Stdin`, and `fmt --check -` gives `check: true, Stdin`.
- `fmt`, `fmt - a`, `fmt - -`, `fmt --bogus a` and `fmt --check --check a` give `Err`.

`src/cli/tests/fmt.rs` (new). Use a temp dir helper in the style of the
existing `TempDir` in `src/lsp/tests/analysis.rs`, or write a local
minimal one under `std::env::temp_dir()` with a unique name. Pass
`Cursor` and `Vec<u8>` for stdio.
- A file with a deep continuation (`s :bd\n\t\t\t> d1\n`), without
  `--check`: exit 0 and the file is now `s :bd\n\t> d1\n`. First confirm
  that the input is reader-clean with `crate::reader::read`; if it is
  not, choose a reader-clean continuation from `examples/`, deepen its
  `>` line and use that instead. The same rule applies to every sample
  below.
- The same file with `--check`: exit 1, stdout is `<path>\n`, and the
  file is unchanged.
- An already formatted file, with and without `--check`: exit 0, stdout
  is empty, and the file bytes and mtime are unchanged.
- An unterminated string (`s "abc\n`): exit 3, stderr contains
  `vactr: ` and `:1:`, and the file is unchanged.
- Stdin `s :bd\n\t\t\t> d1\n`: exit 0 and stdout is the formatted
  text. With `--check`: exit 1 and stdout is empty.
- Stdin with a reader error: exit 3 and stdout equals the input.
- A missing path together with a good file: exit 1, the good file is
  still formatted, and stderr names the missing path.
- A path that is a directory: exit 1.
- Invalid UTF-8 bytes in a file: exit 1 and the file is unchanged.

## Execution Protocol

- Before each edit to `args.rs`, `mod.rs` and `tests/mod.rs` (shared by
  history with earlier work), re-read the file, record its sha256 and
  your intent in the Progress Log, and record the post-edit sha256
  afterwards.
- If the file drifted from the recorded pre-hash, stop.
- Make no git state changes.
- Write logs to `tmp/fst/FST-20/<n>-<name>.log`, each ending with
  `exit=<status>`.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build`
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run cli::` must pass all tests, including the new `fmt` tests.
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
4. `cargo fmt --check` must pass. If only this plan's files fail, run `rustfmt --edition 2021` on them and rerun.
5. `mise run fmt-vact-check` must exit 0: all 10 committed `.vact` files are already formatted. If FST-11 has not yet landed the task, run the same command directly instead: `git ls-files -z '*.vact' | xargs -0 cargo run --quiet -- fmt --check`. If it exits 1, it prints the offending paths. Record them as a finding and do not reformat the files.
6. `printf 's :bd\n\t\t\t> d1\n' | cargo run --quiet -- fmt -` must print `s :bd` followed by a line `\t> d1`.
7. `wc -l src/cli/args.rs src/cli/fmt.rs src/cli/tests/args.rs src/cli/tests/fmt.rs` must show every file under 1000 lines.

## Completion Criteria

- [x] `Command::Fmt` and `FmtInput` match the contract, and `USAGE` lists `fmt`.
- [x] Every test above exists and passes.
- [x] The exit-code precedence matches `command.md`.
- [x] Verification 1-7 pass, each with a log.
- [x] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 222 step 4).
**Blockers**: Waits for FST-10.

### Session: 2026-09-30 (FST-20 implementation)
**Tasks Completed**: Added `FmtInput`, `Command::Fmt`, `parse_fmt`, usage synopsis and CLI dispatch; implemented file/stdin formatting, check mode, UTF-8 and I/O errors, reader-refusal diagnostics, process-all behavior and exit-code precedence; added parser and CLI behavioral tests. FST-10 dependency was accepted by the runtime fanout contract.
**Verification**: All required gates passed. `CARGO_TERM_QUIET=true cargo build` (exit 0); `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run cli::` (30 passed, 0 failed); `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` (exit 0); `CARGO_TERM_QUIET=true cargo fmt --check` (exit 0); `mise run fmt-vact-check` (exit 0); stdin continuation example (exit 0); all four Rust files under 1000 lines. Logs: `tmp/fst/FST-20/90-check-after-modify.log`, `91-clippy.log`, `92-cargo-fmt.log`, `93-vact-check.log`, `94-stdin-example.log`, `95-line-count.log`, and `13-rustfmt-check-final.log`.
**Codex-agent references**: `rust-coding` implemented the non-overlapping verb and behavioral test files; `check-and-test-after-modify` ran the build and CLI test suite.
**Blockers**: None for FST-20 implementation. FST-40 owns formal review, combined-tree integration and workflow closeout.

## Related Plans

- **Depends On**: FST-10
- **Next**: FST-40
