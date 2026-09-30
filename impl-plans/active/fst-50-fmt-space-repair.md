# FST-50: Formatter Space-Indent Repair

**Status**: Ready
**Plan ID**: FST-50 (wave 1; parallel with CMP-10, CMP-15, EDS-10, EDS-11, EDS-12)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 3.1 (amended), 3.6 (repaired-input guarantees), 3.8 (mutation amendment), 3.9; `design-docs/user-qa/pending-completion-questions.md` C4, C5
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

`vactr fmt` refuses space-indented files today. The user wants a
conservative repair: convert unambiguous space indentation to tabs, then
format. The repair must not change program structure. Anything uncertain
is refused unchanged, as today.

Repository facts:

- A space-indented line has zero tabs.
  - In the reader's `body()` at the inner level, such a line breaks the
    loop, and `block()` pushes `EmptyBlock` (`src/reader/layout.rs:288-296`).
  - The outer `body()` pushes `IndentSpace` (`layout.rs:163-171`).
  - So the gate allows exactly those two codes (design 3.9, C4).
- `src/fmt/lines.rs` `split` already gives each line's `indent_width`
  (tabs and spaces), `original_level` (tabs), `indent_end` and `kind`
  (`Blank`, `Comment` or `Code`). Reuse it.

## Non-goals

- No reader change. No change to the rules R1-R9 for clean input, and no
  API change (`Formatted`, `Outcome` and `format_bytes` keep their
  shapes).
- No change to `src/cli/fmt.rs`, `src/lsp/**` or `src/host/**`. They get
  the behavior through `fmt::format`.
- No repair of tab/space mixes, units of 1 or more than 8, or
  non-multiples.

## Dependencies

- **dependsOn**: none
- **Blocks**: CMP-40

## writePaths

- `src/fmt/mod.rs`
- `src/fmt/repair.rs` (new)
- `src/fmt/tests/mod.rs`
- `src/fmt/tests/repair.rs` (new)
- `src/fmt/tests/mutation.rs`
- `src/fmt/tests/fixtures/space2.in`, `src/fmt/tests/fixtures/space2.out`
- `src/fmt/tests/fixtures/space4.in`, `src/fmt/tests/fixtures/space4.out`
- `src/fmt/tests/fixtures/space-ambiguous.in`, `src/fmt/tests/fixtures/space-ambiguous.out`
- `src/fmt/tests/fixtures/space-mixed-lines.in`, `src/fmt/tests/fixtures/space-mixed-lines.out`
- `src/fmt/tests/fixtures/space-other-error.in`, `src/fmt/tests/fixtures/space-other-error.out`
- `src/cli/tests/fmt.rs` (append tests only)
- `impl-plans/active/fst-50-fmt-space-repair.md` (Progress Log only)

## sharedPaths (read only)

`src/fmt/lines.rs`, `src/fmt/levels.rs`, `src/reader/layout.rs`,
`src/types/diag.rs` (`DiagCode::IndentSpace`, `DiagCode::EmptyBlock`),
`src/cli/fmt.rs`, `src/cli/args.rs` (`FmtInput`), and the existing
fixtures `blocks.in`, `continuation.in`, `directives.in` and
`mixed-indent.in`.

## File-level Changes

1. `src/fmt/repair.rs`: add
   `pub(super) fn space_indent(src: &str, diags: &[Diagnostic]) -> Option<String>`.
   It returns the converted text only when every precondition of design
   3.9 holds. It does NOT re-read the text.
   - Every `Severity::Error` diagnostic has code `IndentSpace` or
     `EmptyBlock`, and at least one is `IndentSpace`.
   - On `lines::split(src, FileId::new(1))`, for each `Code` or
     `Comment` line with `indent_width > 0`: `original_level == 0` (no
     tab in the run). The widths `W` give `u = min(W)` with `2 <= u <= 8`,
     and every `w % u == 0`.
   - Build the output: for those lines, replace
     `src[start..indent_end]` with `w / u` tabs. Copy every other byte
     unchanged, including blank lines and all terminators.
2. `src/fmt/mod.rs`:
   - Add `mod repair;`.
   - Move today's clean-path body (from `lines::split` to building
     `Formatted`) into a private helper that takes the source to format
     and its reader diagnostics.
   - In `format`, when the gate refuses, call `repair::space_indent`.
     - If it returns `Some(c)`, re-read `c` with the same `FileId::new(1)`
       and empty `AliasEnv`.
     - If `c` has zero errors, return the helper's result on `c`, with
       `outcome` forced to `Changed` (the text differs from `src` by
       construction), and `diags` taken from `read(c)`.
     - In every other case, return the original `Refused` result
       unchanged: `text == src` and the ORIGINAL diagnostics.
3. `src/fmt/tests/mod.rs`: add `mod repair;` only.
   - Keep the FIXTURES list and `check_guarantees` unchanged. Every
     existing caller passes reader-clean input or a fixture that stays
     refused.
   - The new fixtures are used by `repair.rs` directly.
4. `src/fmt/tests/mutation.rs`: the reader-error branch accepts either:
   - `Refused` with `text == mutant`; or
   - `Changed`, where `read(out)` has no error and every code line's
     indent run of `out` contains no space.
5. Fixtures (they do NOT use the `.vact` extension):
   - `space2.in` and `space4.in`: the content of `blocks.in`,
     `continuation.in` and `directives.in` concatenated, with each tab of
     indentation replaced by 2 or 4 spaces.
     - `.out` is `format` of the tab original. The test asserts that too.
     - If a directive or comment line in the source has a width that does
       not convert cleanly, adjust the fixture, not the rule, and record
       that in the Progress Log.
   - `space-ambiguous.in`: code at widths 4 and 6.
   - `space-mixed-lines.in`: one line tab-indented and another
     space-indented.
   - `space-other-error.in`: 4-space indentation plus an unclosed `{`.
   - For all three, `.out == .in` (refused).
6. `src/cli/tests/fmt.rs`: append tests that use the existing `TempDir`
   and `invoke`.

## Invariants

- Clean input produces byte-identical output to today. The existing
  `fmt::` tests and the `cli::tests::fmt` tests pass unchanged.
- `mixed-indent` and `reader-error` stay `Refused`.
- G2 holds: `Refused` always means `text == src`.
- Repaired output changes only indent runs, trailing blanks and the
  final terminator (G7), so LSP `format_edits` stays whitespace-only
  with no LSP change.
- Every file stays under 1000 lines.

## Tests (`src/fmt/tests/repair.rs` plus the CLI tests)

- `space2.in` and `space4.in`:
  - `format(x).outcome == Changed`;
  - `format(x).text == format(tab original).text == <fixture>.out`;
  - the `print_all(read(out).nodes)` of the output equals that of the
    tab original;
  - `format(out)` is `Unchanged` (idempotent);
  - trivia kinds and `#@` texts equal those of the tab original.
- A 2-space block `fn f a:\n  let b 1\n  + a b\n` -> exactly
  `fn f a:\n\tlet b 1\n\t+ a b\n`.
- A 4-space continuation `x\n    > f 1\n` -> `x\n\t> f 1\n`.
- These are refused with `text == input` and the original diagnostics
  kept (non-empty, including `indent-space`):
  - `space-ambiguous`;
  - `space-mixed-lines`;
  - `space-other-error`;
  - `mixed-indent` (`let x 1\n\t let y 2\n`);
  - a 1-space file `fn f a:\n let b 1\n`;
  - a 10-space unit file.
- `space_indent` returns `None` for a clean tab file. (`format` never
  calls it then, but the helper must also be safe.)
- CLI:
  - A 4-space file is rewritten to the tab form with exit 0, and a
    second run leaves it unchanged with exit 0.
  - `--check` on a fresh 4-space file prints its path and exits 1, and
    the file is untouched.
  - A `space-ambiguous` file is untouched, prints `vactr: <path>:` on
    stderr, and exits 3.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol`: fresh read, sha256 before and
after each edit, intent line, stop on drift, no git state changes. Write
logs to `tmp/cmp/FST-50/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build`
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run fmt::`
   (includes `fmt::tests::repair` and the mutation test; a positive
   count, all passing)
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run cli::tests::fmt`
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
5. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
6. `rustfmt --edition 2021 --check src/fmt/mod.rs src/fmt/repair.rs src/fmt/tests/mod.rs src/fmt/tests/repair.rs src/fmt/tests/mutation.rs src/cli/tests/fmt.rs`
7. `mise run fmt-vact-check`: the committed `.vact` files are still fixed
   points.

## Completion Criteria

- [ ] The repair runs only under the design 3.9 preconditions. The re-read
      gate is enforced, and refusal returns the original text and
      diagnostics.
- [ ] 2- and 4-space fixtures convert, are idempotent and equal the tab
      version, and the ambiguous, mixed and other-error cases are refused
      unchanged.
- [ ] The CLI tests pass, and the existing fmt and CLI tests pass
      unchanged.
- [ ] Verification steps 1-7 are logged with `exit=0`.
- [ ] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Blockers**: None.

## Related Plans

- **Next**: CMP-40
