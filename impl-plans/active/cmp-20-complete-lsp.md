# CMP-20: LSP Completion on the Engine

**Status**: In Progress
**Plan ID**: CMP-20 (wave 2; parallel with CMP-21, CMP-30, CMP-31)
**Design Reference**: `design-docs/specs/design-completion.md` 4.1, 4.2, 7.2; `design-docs/specs/design-implementation.md` 14.5 (Revised 2026-09-30)
**Dispatch**: `impl-plans/active/cmp-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

`textDocument/completion` currently runs `Analyzer::complete`
(`src/lsp/analysis.rs:337-420`), which filters with `starts_with`. This
plan makes it call `crate::complete::complete` (CMP-10), so that the LSP
gets:

- cursor contexts;
- scope-aware locals;
- ranking;
- a cap of 100 with `isIncomplete`;
- UTF-16 `textEdit`s.

The candidates must stay a superset of today's in every applicable
context (design 4.2).

## Non-goals

- No signature help, no new capability, and no change to the trigger
  characters (`:` and `.` stay).
- No change to `hover`, `format`, diagnostics, `package_names` behavior,
  or anything under `src/complete/`.

## Dependencies

- **dependsOn**: CMP-10
- **Blocks**: CMP-40

## writePaths

- `src/lsp/analysis.rs`
- `src/lsp/server.rs`: the `completion` handler only.
- `src/lsp/tests/mod.rs`: add `mod complete;`.
- `src/lsp/tests/analysis.rs`: edit only
  `completion_offers_prelude_names_keywords_and_document_names`.
- `src/lsp/tests/complete.rs` (new)
- `impl-plans/active/cmp-20-complete-lsp.md` (Progress Log only)

## sharedPaths (read only)

`src/complete/mod.rs` (the CMP-10 contract), `src/lsp/convert.rs`
(`range`, `offset`), `src/session/eval.rs` (`Analysis`).

## File-level Changes

1. `analysis.rs`:
   - Add
     `pub fn complete_list(&mut self, uri: &Url, pos: Position) -> CompletionList`.
     - Get the doc, or return an empty list with `is_incomplete: false`.
     - Compute `at = offset(&d.text, pos)`.
     - Build the `Snapshot`:
       - `manifest` is `d.analysis.manifest.clone()`;
       - `packages` holds a `PackageNames{prefix, path, names}` for each
         `alias_env.prefixes` entry, with `names = self.package_names(&path)`.
         Collect the prefixes first, as the current code does, to avoid
         holding a borrow of `d` across `&mut self`;
       - `document` holds a `DocName` for each top-level `defined_name`
         of `d.analysis.forms`, with the same kind mapping as today
         (`fn` -> `Function`, `struct`/`enum` -> `Type`, else
         `Variable`) and the same detail (the type of `children[2]`, or
         `"document"`).
     - Call `complete::complete(&text, at, &snap, complete::DEFAULT_LIMIT)`.
   - Map each candidate (design 4.1 table) to a `CompletionItem`:
     - `label` and `detail: Some(..)`;
     - `kind`;
     - `text_edit: Some(CompletionTextEdit::Edit(TextEdit::new(range(text, from, to), insert)))`;
     - `filter_text: Some(label)`;
     - `sort_text: Some(format!("{rank:04}"))`.
   - Return `CompletionList { is_incomplete: c.incomplete, items }`.
   - Keep `pub fn complete(&mut self, uri, pos) -> Vec<CompletionItem>` as
     a wrapper returning `self.complete_list(uri, pos).items`.
   - Delete the old body and the now-unused `is_word`. Check with grep
     that nothing else uses it.
   - `AnalysisReq::Complete`'s reply becomes
     `oneshot::Sender<CompletionList>`, and `handle` sends
     `self.complete_list(..)`.
2. `server.rs` `completion`: return
   `Ok(Some(CompletionResponse::List(list)))`. The rest of the handler is
   unchanged.
3. `tests/analysis.rs`: change the one test per design 4.2 items 1-3:
   - At `Position::new(1, 0)` of `"let tempo-x 12\n\n"`: `tempo-x` is
     present, and `complete_list(..).is_incomplete == true`.
   - At the end of `"si"`: `sine` is present.
   - At the end of `"s :"`: `:bd` and `:analog` are present.
   - At the end of `":b"`: `:bd` is present, every label starts with
     `:`, and every item whose label (without the `:`) starts with `b`
     ranks before every item that does not.

   Add a comment citing design-completion 4.2 as the justification.

## Pitfalls

- UTF-16: always use `convert::range`, and never compute character
  columns by hand.
- `package_names` needs `&mut self`. Build the prefix list before
  borrowing `self.docs` again, as today's code does at
  `analysis.rs:388-395`.
- Do not change `format_edits` or the format tests.
- `analysis.rs` must stay under 1000 lines. It is 618 lines today and
  should shrink.

## Tests (`src/lsp/tests/complete.rs`; reuse the `open` helper pattern of `tests/analysis.rs`, copying it if it is private)

- `fn f alpha:\n\tlet beta 1\n\tal` at the end: the first item's label is
  `alpha`, with kind VARIABLE.
- `s :an` at the end: the items include `:analog`, and every label
  starts with `:`.
- `s :analog > cu` at the end: the first item is `cutoff`.
- A Japanese comment line `# ` + U+65E5 U+672C, then `\nsi` at the end:
  the `text_edit` range equals `Range(Position(1,0), Position(1,2))`.
- An empty line: `is_incomplete == true`.
- Inside a string `"s|x"` and a comment `# s|`: no items.
- Superset: a test-local `legacy_labels(text, at)` copies today's
  filter. It uses the removed `is_word` word, `starts_with` over
  document names, natives, the manifest `:keys` and the prefixes, with no
  packages.
  - Over the cursors `si`, `ga`, `te`, `:b`, `:a` and the empty line in
    `let tempo-x 12\n`, call `complete::complete` with `MAX_LIMIT`.
  - Assert that every legacy label that fits the context (design 4.2) is
    present.

## Execution Protocol

Follow `cmp-dispatch.json` `editProtocol` and its `retryPolicy`. A
`--features lsp` build also compiles `src/host`, which CMP-21 does not
touch on native builds, and `src/complete`, which is complete by this
wave. Write logs to `tmp/cmp/CMP-20/attempt-<n>/`.

## Verification (exit 0, complete logs)

1. `CARGO_TERM_QUIET=true cargo build --features lsp`
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --features lsp lsp::`:
   every test passes, including the unchanged format tests and the new
   `lsp::tests::complete` ones.
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings`
4. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
5. `rustfmt --edition 2021 --check src/lsp/analysis.rs src/lsp/server.rs src/lsp/tests/mod.rs src/lsp/tests/analysis.rs src/lsp/tests/complete.rs`
6. `wc -l src/lsp/analysis.rs src/lsp/tests/analysis.rs src/lsp/tests/complete.rs`:
   each is under 1000 lines.

## Completion Criteria

- [x] The LSP completion path calls `complete::complete`, and the
      response is `CompletionResponse::List` with `is_incomplete`.
- [x] The items carry UTF-16 `text_edit`s, a rank `sort_text` and the
      mapped kinds.
- [x] The changed test matches design 4.2, and the new tests (local,
      keyword, control, UTF-16, incomplete, string and comment, superset)
      pass.
- [x] Steps 1-6 are logged, and the Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 226, step 4).
**Dependency**: CMP-10 accepted for the implementation wave.

### Session: 2026-09-30 (session 226, step 6 implementation)
**Tasks Completed**: CMP-10 was accepted in the runtime dependency set. Replaced the LSP's prefix-only completion with the shared engine and `CompletionList`; mapped candidates to LSP kinds, UTF-16 `text_edit`s, `filter_text` and rank `sort_text`; preserved the vector wrapper; updated the design 4.2 test and added seven focused LSP tests. The new test module owns an `open` helper because the existing helper is private to the sibling `analysis` module.
**Verification**:
- `CARGO_TERM_QUIET=true cargo build --features lsp` — exit=0 (`tmp/cmp/CMP-20/attempt-1/05-build-lsp.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --features lsp lsp::` — exit=0; 23 passed, 0 failed (`tmp/cmp/CMP-20/attempt-1/12-nextest-lsp-quiet.log`; independently rerun after helper repair, 23 passed, 0 failed, `tmp/cmp/CMP-20/attempt-1/11-check-agent-nextest.log`).
- `CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings` — exit=0 (`tmp/cmp/CMP-20/attempt-1/07-clippy-lsp.log`).
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` — exit=0 (`tmp/cmp/CMP-20/attempt-1/08-clippy-default.log`).
- `rustfmt --edition 2021 --check src/lsp/analysis.rs src/lsp/server.rs src/lsp/tests/mod.rs src/lsp/tests/analysis.rs src/lsp/tests/complete.rs` — exit=0 (`tmp/cmp/CMP-20/attempt-1/09-rustfmt.log`).
- `wc -l src/lsp/analysis.rs src/lsp/tests/analysis.rs src/lsp/tests/complete.rs` — exit=0; 622, 644 and 176 lines (`tmp/cmp/CMP-20/attempt-1/10-line-count.log`).
**Repair evidence**: Initial nextest compilation failed because the new module imported a private sibling test helper (`tmp/cmp/CMP-20/attempt-1/02-nextest-lsp.log`, exit=101). Added a local helper as the plan permits; the source-matched reruns above pass. The initial feature build and formatting/line checks are recorded in `01-build.log`, `03-rustfmt.log` and `04-line-count.log`.
**Handoff**: Implementation criteria are complete. Combined-tree integration review and final plan closeout remain with CMP-40.

## Related Plans

- **Depends On**: CMP-10
- **Next**: CMP-40
