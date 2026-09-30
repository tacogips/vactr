# FST-21: LSP `textDocument/formatting` on the Formatter

**Status**: Completed
**Plan ID**: FST-21 (wave 2; parallel with FST-20, FST-22, FST-23)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` 3.7.2; `design-docs/specs/design-implementation.md` 14.5.11 "Formatting" (Revised 2026-09-30)
**Dispatch**: `impl-plans/active/fst-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

`vactr lsp` already advertises `document_formatting_provider` (at
`src/lsp/server.rs:98`) and answers it with `lsp::analysis::format_edits`
(at `src/lsp/analysis.rs:519-560`). That function only strips
whitespace. This plan re-implements `format_edits` on `crate::fmt::format`,
so that the editor's LSP and `vactr fmt` produce identical text. The
signature stays `pub fn format_edits(text: &str) -> Vec<TextEdit>`, which
means `server.rs` does not change.

## Non-goals

- No change to `server.rs`, `convert.rs`, `src/fmt/**` or any analysis
  behavior other than formatting.
- No range formatting.
- No full-document replace edit: the edits must stay line-aligned and
  whitespace-only.

## Dependencies

- **dependsOn**: FST-10
- **Blocks**: FST-40

## writePaths

- `src/lsp/analysis.rs`: only `format_edits` and its helper(s). Remove
  the now-unused `is_blank` helper only if nothing else uses it (check
  with grep first).
- `src/lsp/tests/analysis.rs`: keep the existing
  `formatting_changes_only_whitespace_and_keeps_directive_lines`
  unchanged, and append new tests.
- This plan's Progress Log.

## sharedPaths (read-only)

- `src/fmt/mod.rs` (the FST-10 contract, `Outcome`)
- `src/fmt/tests/fixtures/` (FST-10 fixtures, read as test inputs)
- `src/lsp/convert.rs` (`range(text, start, end)` for UTF-16 ranges)
- `src/lsp/server.rs:98` (the capability, unchanged)

## Implementation Key Points

1. **Early exit.** Call `let f = crate::fmt::format(text)`. When the
   outcome is `Refused` or `Unchanged`, return `Vec::new()`. The server
   already wraps the result as `Some(vec)`.
2. **Edits.** Split `text` and `f.text` into lines exactly as FST-10 does
   (at `\n`, keeping a `\r` before it as part of the terminator). G7
   guarantees that output line n corresponds to input line n, for every
   n up to the last output content line. For each such line:
   - If the indent run differs, emit one edit that replaces the input
     indent run with the output indent run.
   - If trailing blanks were removed, emit one edit that deletes them.
   - After those, emit ONE tail edit that replaces everything from the
     end of the last output content line's body to the end of `text`
     with the output's final terminator. Skip it when the tail is
     already equal. The existing implementation has the same tail logic;
     mirror it, including the rule that per-line strips at or after
     `tail_start` are covered by the tail edit.
3. **Edit order and overlap.** Emit the edits in ascending document
   order, and never let two of them overlap. Test by applying them from
   the end, as the existing `apply` helper does.
4. **Invariant.** Every edit's removed text and inserted text is
   whitespace only. The existing test asserts this, and it must keep
   passing unchanged.
5. **Positions.** Use `convert::range(text, a, b)` for UTF-16 positions,
   and never compute columns by hand.

## Tests (append to `src/lsp/tests/analysis.rs`)

- For every committed `.vact` file (use `git ls-files '*.vact'`, with a
  directory-scan fallback, as in FST-10) and every
  `src/fmt/tests/fixtures/*.in`:
  - `apply(text, &format_edits(text)) == crate::fmt::format(text).text`;
  - every edit is whitespace-only;
  - committed files give an empty edit list.
- `s :bd\n\t\t\t> d1\n` (or a reader-clean equivalent) gives exactly one
  indent edit, and the result is `s :bd\n\t> d1\n`.
- A reader-error document, e.g. `s "abc\n` with trailing spaces added,
  gives no edits at all: the trailing spaces are NOT stripped, which is
  a deliberate change from the old behavior.
- A comment-only line with a non-ASCII comment, such as `\t\t\t# é日本`
  after a top-level statement, re-indents correctly. This checks the
  UTF-16 ranges.
- The existing test, unchanged, still passes.

## Execution Protocol

- Before each edit to either file, re-read it, then record its sha256
  and your intent in the Progress Log. Record the post-edit sha256
  afterwards. Stop if the file drifted.
- Make no git state changes.
- `--features lsp` also compiles `src/cli`, which FST-20 edits
  concurrently. A compile error in an FST-20 file is transient: never
  edit it. Wait and retry as described in the `retryPolicy` of
  `fst-dispatch.json`.
- Write logs to `tmp/fst/FST-21/<n>-<name>.log`, each ending with
  `exit=<status>`.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build --features lsp`
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --features lsp lsp::`: all tests pass, including the existing formatting test.
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings`
4. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` (without `lsp`).
5. `cargo fmt --check`. If only this plan's files fail, run `rustfmt` on them and rerun.
6. `wc -l src/lsp/analysis.rs src/lsp/tests/analysis.rs`: both files are under 1000 lines. `analysis.rs` is about 564 lines today.

## Completion Criteria

- [x] `format_edits` delegates to `crate::fmt::format`, keeps its signature, and returns no edits when the outcome is `Refused`.
- [x] Applying the edits reproduces the formatter's output exactly, for every committed file and fixture.
- [x] The existing formatting test passes unchanged.
- [x] Verification steps 1-6 pass, each with a log.
- [x] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 222 step 4).
**Blockers**: Waits for FST-10.

### Session: 2026-09-30 (FST-21 implementation)
**Tasks Completed**: Reimplemented `format_edits` on `crate::fmt::format`; `Refused` and `Unchanged` return no edits. Changed documents receive nonoverlapping UTF-16-ranged indentation, trailing-blank, and final-tail edits. Whitespace-only input formatted to empty text uses one whitespace-only deletion per physical line, preserving the line-aligned edit contract. Kept the existing formatting test unchanged and added exact-output coverage over tracked `.vact` files and formatter fixtures, continuation indentation, reader-error refusal, UTF-16 positions after a non-ASCII comment, and whitespace-only input. Both Rust files remain under 1000 lines.
**Edit evidence**: Per-edit source hashes and intent are recorded in `tmp/fst/FST-21/01-analysis-intent.md`, `02-tests-intent.md`, `03-tests-edit-intent.md`, `13-utf16-test-fix-intent.md`, `17-utf16-test-fix2-intent.md`, `27-empty-output-intent.md`, and `37-line-aligned-fix-intent.md`; rustfmt records include `04-*`, `14-*`, `18-*`, `28-*`, and `38-*`. Final Rust source hashes are in `tmp/fst/FST-21/39-after-rustfmt.sha256` and the branch evidence snapshot.
**Review repair**: Scoped FST-21 review found the empty-output case violated the no-full-document-edit constraint. Replaced it with line-aligned whitespace deletions and asserted one edit per physical line; no open findings remain in the assigned hunks.
**Verification**: `tmp/fst/FST-21/41-build-final.log` (exit 0); `40-nextest-final.log` (14/14 passed, exit 0); `42-clippy-lsp-final.log` and `43-clippy-default-final.log` (exit 0); `44-fmt-check-final.log` (exit 0); `45-lines-final.log` (analysis.rs 626, tests/analysis.rs 533, exit 0).
**Resolved attempts**: `12-nextest-lsp.log` and `16-nextest-lsp-rerun.log` identified the UTF-16 test's broad selector; `20-nextest-lsp-final.log` confirmed the corrected assertion. `lsp_review` identified the whole-document empty-output replacement; the line-aligned repair passed `40-nextest-final.log`. A concurrent FST-20 formatting diff observed by the check agent was absent in final `cargo fmt --check`.
**Downstream**: FST-40 owns combined-tree integration review and workflow closeout; neither is claimed complete here.

### Session: 2026-09-30 (test-integrity repair)
**Tasks Completed**: Test-integrity repair (2026-09-30): trailing-blank strips on non-last lines were dropped by format_edits; strips are now emitted per line and the tail replacement prunes overlaps; added middle-line and trailing-blank-variant regression tests. Logs tmp/fst/FST-21/59-65.

## Related Plans

- **Depends On**: FST-10
- **Next**: FST-40
