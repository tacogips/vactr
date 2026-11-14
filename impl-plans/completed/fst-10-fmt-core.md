# FST-10: Formatter Core (`src/fmt/`)

**Status**: Completed
**Plan ID**: FST-10 (wave 1; parallel with FST-11)
**Design Reference**: `design-docs/specs/design-formatter-and-syntax.md` sections 3.1-3.6 and 3.8
**Dispatch**: `impl-plans/active/fst-dispatch.json`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

The user asked for a `.vact` formatter. The specs already promise one
(architecture.md "Pipeline", design-implementation 6.3 and 6.5.4,
lang-reference 1). This plan builds only the pure core: a function from
`&str` to the formatted text. It depends on nothing host-native, so it
builds for wasm32. The CLI (FST-20), LSP (FST-21) and wasm export
(FST-22) run in parallel in wave 2 against the contract pinned below, so
the signatures must match exactly.

The core changes only layout whitespace: the indentation of `>`
continuation lines, of blocks those lines open, and of comment-only
lines; trailing blanks; trailing blank lines; and the final newline.
It refuses (returns the input byte for byte) whenever the reader reports
an error.

## Non-goals

- No CLI, LSP, wasm or editor code (FST-20/21/22/30).
- No long-line wrapping and no change to spacing inside a line (R6, R9).
- No change to any reader, directive or lexer file. `lex_line` is already
  `pub(crate)`, which `src/fmt` can use because it is in the same crate.
- Do not reformat any committed `.vact` file.
- No new crate (no proptest, rand or similar). The mutation test uses a
  hand-written xorshift.

## Dependencies

- **dependsOn**: none
- **Blocks**: FST-20, FST-21, FST-22

## writePaths

- `src/lib.rs`: add exactly one line `pub mod fmt;` in alphabetical order
  (between `expand` and `host`). No `cfg`: the module must compile on
  every target.
- `src/fmt/mod.rs` (new): API, gate, output assembly, `#[cfg(test)] mod tests;`
- `src/fmt/lines.rs` (new): the line model (3.2)
- `src/fmt/levels.rs` (new): the code-line levels (3.4) and comment-line levels (3.5)
- `src/fmt/tests/mod.rs` (new), `src/fmt/tests/rules.rs`, `src/fmt/tests/corpus.rs`, `src/fmt/tests/mutation.rs` (new)
- `src/fmt/tests/fixtures/` (new directory holding `<name>.in` / `<name>.out` pairs)
- this plan's Progress Log

## sharedPaths (read-only)

- `src/reader/mod.rs` (`read`, `ReadResult`), `src/reader/lexer.rs` (`lex_line`, `LexedLine`, `Tok::BlockColon`), `src/reader/layout.rs` (`split_lines`, `one_line`: the model to mirror), `src/reader/node.rs` (`TriviaKind`), `src/reader/sexpr.rs` (`print_all`)
- `src/directives/attach.rs` (`attach`, `Doc::new`, `Placed`), `src/directives/mod.rs`, `src/types/manifest.rs` (`HostManifest::spec_default`), `src/types/diag.rs` (`Diagnostic`, `Severity`)
- `examples/*.vact`, `src/prelude/templates.vact` (test inputs only; never edit)

## Contract (pinned; FST-20/21/22 compile against it)

```rust
// src/fmt/mod.rs
pub enum Outcome { Unchanged, Changed, Refused }          // derive Clone, Copy, PartialEq, Eq, Debug
pub struct Formatted { pub text: String, pub outcome: Outcome, pub diags: Vec<Diagnostic> } // Clone, Debug
pub fn format(src: &str) -> Formatted;
pub const STATUS_FORMATTED: u32 = 0;
pub const STATUS_REFUSED: u32 = 1;
pub const STATUS_NOT_UTF8: u32 = 2;
pub fn format_bytes(input: &[u8]) -> (u32, Vec<u8>);
```

- `diags` is every reader diagnostic, of any severity, in every outcome.
- `Unchanged` if and only if `text == src`. `Refused` if and only if a
  diagnostic has `Severity::Error`, and then `text == src`.
- `format_bytes`: input that is not UTF-8 gives `(2, input.to_vec())`.
  `Refused` gives `(1, input.to_vec())`. Otherwise `(0, text bytes)`,
  also when the text is unchanged.
- Add `#[must_use]` and doc comments that cite the design sections. Use the
  same comment style as `src/reader/mod.rs`.

## Implementation Key Points

1. **Gate (3.1).** Call `crate::reader::read(src, FileId::new(1), &AliasEnv::new())`.
   Do NOT use `FileId::CONSOLE` (0): console registers `_1` must be
   errors, as in files. The >4 GiB case already comes back as an error
   diagnostic, so there is no special path for it.
2. **Line model (3.2).** Split exactly the way `layout::split_lines`
   does: at `\n`. The line body excludes a `\r` right before the `\n`.
   A text ending in `\n` has one final empty "line" with no terminator.
   Classify each line after its indent run (leading `\t` and ` `) with
   `lex_line(src, content_start, body_end, file)`:
   Blank = no tokens and no comment. Comment-only = no tokens and a
   comment (keep its `TriviaKind`). Code = at least one token. For a code
   line record: `o` = the tab count of its indent run; `opens_block` =
   the last token is `Tok::BlockColon`; `has_directive` = its comment is
   `TriviaKind::Directive`. Record in the model the byte ranges for the
   indent run, the content and the trailing blanks.
3. **Code levels (3.4).** One pass with a frame stack
   `(lo, lf, current: Option<(o, f)>)`, starting from `[(0, 0, None)]`.
   Pop while `top.lo > o`. If `o == top.lo`, `f = top.lf` and the line
   becomes the current statement. Otherwise `f = top.lf + 1`, which is a
   continuation. When the line opens a block, push `(o + 1, f + 1, None)`.
   On reader-clean input `o < top.lo` cannot happen after popping. Still
   handle it without panicking: treat it as a statement at `top.lf`.
   Never index with unchecked arithmetic; use `saturating_*`.
4. **Comment levels (3.5).** Here `b` (the formatted level of the NEXT
   code line) is needed, so comment lines must be resolved after the
   next code line's `f` is known. Either buffer the pending comment lines
   together with a snapshot of the chain taken after the previous code
   line, or run two passes. The chain is the `current` of each frame,
   from bottom to top, with `None` skipped. Width
   `w = tabs + spaces` of the comment's indent run (each counts 1, as in
   `directives::attach::Lines`). With no preceding code line, `r = 0`.
   Otherwise: `i` = the largest index with `oi <= w`, and there is always
   one because `T0` has `o = 0`, unless the chain is empty, in which case
   `r = 0`. `hi = f(i+1) - 1` if `i < k`, else `max(fk, b)` (`b = 0` at
   EOF). `r = min(fi + (w - oi), hi)`. Output: `r` tabs, then the comment
   text.
5. **Assembly (R5, R7, R8).**
   - Code line: new indent `f` tabs + content. Trailing blanks are
     stripped unless `has_directive`, in which case every byte after the
     indent run is kept.
   - Comment-only line: stripped the same way; a `Directive` keeps its
     trailing bytes.
   - Blank line: it becomes empty, keeping its own terminator.
   - Trailing blank lines at EOF are removed. The last content line ends
     with its own terminator, or with `\n` when it had none. A text with
     no content line gives `""`.
   - Lines are never reordered, merged or added.
   - Code lines whose `o == f` keep their original indent bytes. On
     reader-clean input these are tabs only.
6. **Panics.** The formatter must not panic for any input
   (`#![deny(clippy::indexing_slicing)]` is NOT required). Slice only at
   offsets that come from `lex_line` spans or ASCII bytes (`\t`, ` `,
   `\r`, `\n`), so every slice is on a char boundary.
7. **wasm32.** The code uses only `crate::reader`, `crate::types::diag`
   and `std` collections and strings: no `std::fs`, no threads, no time.
   Do not use `crate::directives` in non-test code (it is only a test
   oracle for G6).
8. **File size.** Keep every file under 1000 lines. The expected sizes
   are `mod.rs` about 200, `lines.rs` about 150, `levels.rs` about 200,
   and each test file under 400.

## Tests (`src/fmt/tests/`)

Register them with `#[cfg(test)] mod tests;` in `src/fmt/mod.rs`, and in
`tests/mod.rs` add `mod rules; mod corpus; mod mutation;`. Put the shared
helpers in `tests/mod.rs`:

- `check_guarantees(src)`, which asserts G1-G7 for one input:
  - G3: `print_all(&read(out).nodes) == print_all(&read(src).nodes)`,
    and `read(out)` has no error.
  - G4: `format(out).text == out` and its outcome is `Unchanged`.
  - G5: the trivia sequences have equal kinds and order. `#@` text is
    byte-equal, and `#` text is equal after trimming trailing blanks.
  - G6: `directives::attach::attach(src, &Doc::new(src, FileId::new(1), &nodes, &HostManifest::spec_default()), &trivia, &mut vec![])`
    gives the same `(attach, trailing)` per directive, for input and
    output.
  - G7: output line n differs from input line n only in its indent run,
    trailing blanks or terminator.

**Fixtures.** Add `src/fmt/tests/fixtures/<name>.in` and `<name>.out`. Do
not use the `.vact` extension. Read them with
`include_str!` or with `std::fs` under `env!("CARGO_MANIFEST_DIR")`.
Each `.out` must be written by hand from the rules, and never produced by
running the formatter. Every `.in` must be reader-clean except
`mixed-indent` and `reader-error`.
Required fixtures, each checked as `input -> expected`:

- `continuation`:
  - `x = foo` followed by `\t\t\t> bar` gives `\t> bar`.
  - A deep continuation that opens a block, `\t\t\t> f:` with body
    `\t\t\t\ts`, gives `\t> f:` / `\t\ts`.
  - An `inst` body continuation already at level 2 stays unchanged.
- `blocks`: nested `inst`/`if` blocks already at canonical levels stay
  unchanged. This fixture is a fixed point.
- `pairs`: `slot :drums gain: 0.8:` with a block, and `name: value`
  pairs, stay unchanged except for layout.
- `lambda`: `f = ->:` followed by a body, and a deep continuation with `->:`.
- `interp`: nested interpolation strings, `#` inside strings, and
  trailing spaces inside a string before its closing `"`. The string
  bytes must be unchanged.
- `comments`: a stray deep comment after a top-level statement moves to
  level 0. A space-indented comment (`    # c`, `w = 4`, inside a level-1
  block) becomes `\t# c`. Trailing `#` comments lose their trailing
  blanks.
- `directives`: own-line `#@` lines at widths 0..6 in a region with a
  continuation-opened block. A trailing `#@` with trailing blanks keeps
  them. A comment between `elif:` and its body. Placement is checked
  by G6.
- `blank-lines`: leading blanks are kept. An interior run of 3 blank
  lines is kept. Whitespace-only lines become empty. Trailing blank
  lines are dropped.
- `crlf`: CRLF lines stay CRLF. A final line without a terminator gets
  `\n`.
- `empty`: `""` gives `""`, and `"\n\n\t\n"` gives `""`.
- `mixed-indent`: a code line indented with `\t ` gives `Refused`, and
  `.out` == `.in`.
- `reader-error`: an unterminated string gives `Refused`, `.out` ==
  `.in`, and at least one error diagnostic.

**`rules.rs`.** One test per fixture. The inline cases from the design's
3.5 table give exactly the table's "After" column. `format_bytes`:
`[0xff, 0xfe]` gives `(2, same bytes)`, the reader-error fixture gives
`(1, same)`, and continuation gives `(0, the .out bytes)`.

**`corpus.rs`.** Run `git ls-files '*.vact'` through `std::process::Command`
from `CARGO_MANIFEST_DIR`. If git fails, fall back to reading
`examples/` and `src/prelude/` for `*.vact`. Assert that at least 10
files are found. For each file assert `format(src).text == src` (fixed
point), the outcome `Unchanged`, and `check_guarantees(src)`. If a
committed file is NOT a fixed point, do not edit it: record it in the
Progress Log as a finding and stop.

**`mutation.rs`.** A deterministic xorshift64 with a fixed seed. Run
2000 mutants built from the fixtures and the committed files. Each
mutation inserts or deletes one of `\t`, ` `, `>`, `:`, `#@`, `\n`, `"`,
`{` at a random char boundary. G1 always holds (just call `format`).
G2 must hold when the result is refused. G3-G7 must hold when the mutant
reads clean. It must run in under 10 s in a debug build.

## Execution Protocol

Before each edit, re-read the target file and record its sha256
(`shasum -a 256 <file>`) and a one-line intent in the Progress Log.
Afterwards, record the post-edit sha256. If `src/lib.rs` changed
unexpectedly between reads, stop and report the drift. Do not touch git
state. Write the evidence logs to `tmp/fst/FST-10/<n>-<name>.log`,
ending each with an `exit=<status>` line.

## Verification (each must show exit 0 and a complete log)

1. `CARGO_TERM_QUIET=true cargo build` must complete.
2. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run fmt::` must show the fmt tests all passing, with at least 15 tests.
3. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` must report no warnings.
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` must succeed, which proves `src/fmt` is wasm32-clean.
5. `cargo fmt --check` must pass. If it fails only in `src/fmt/**` or `src/lib.rs`, run `rustfmt --edition 2021` on those files only, then rerun.
6. `wc -l src/fmt/*.rs src/fmt/tests/*.rs` must show every file under 1000 lines.
7. The full `cargo nextest run` is run by the orchestrator outside the Codex sandbox. Record it as "deferred to reconciliation" if it cannot run here.

## Completion Criteria

- [x] The `src/fmt` contract matches the pinned signatures exactly.
- [x] All the fixtures listed above exist with hand-written `.out` files.
- [x] G1-G7 are asserted over every committed `.vact` file and every clean fixture. The fixed-point check passes, or a finding is recorded.
- [x] The mutation test (2000 mutants) passes.
- [x] Verification commands 1-6 pass with their logs.
- [x] The Progress Log is updated.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: Plan authored (session 222 step 4).
**Tasks In Progress**: None.
**Blockers**: None.
**Notes**: The contract is pinned for FST-20/21/22.

### Session: 2026-09-30 (FST-10 implementation)
**Tasks Completed**: Implemented the pinned formatter API, reader gate, byte API, line model, code/comment indentation, output assembly, required hand-written fixtures, G1-G7 checks, committed `.vact` corpus fixed-point checks, and 2,000 deterministic mutation cases. Added `pub mod fmt;` in `src/lib.rs`.
**Verification**: `CARGO_TERM_QUIET=true cargo build` (exit 0, `tmp/fst/FST-10/45-final-build.log`); formatter nextest suite (15 passed, exit 0, `tmp/fst/FST-10/44-final-nextest.log`, independently rerun by check-and-test verifier); strict Clippy (exit 0, `tmp/fst/FST-10/46-final-clippy.log`); wasm32 host-wasm build (exit 0, `tmp/fst/FST-10/47-final-wasm.log`); `cargo fmt --check` (exit 0, `tmp/fst/FST-10/48-final-fmt.log`); line counts all below 1000 (exit 0, `tmp/fst/FST-10/49-final-lines.log`).
**Resolved Attempts**: Intermediate source/test/clippy failures were corrected and retained in logs 10, 23, and 30; the complete final-source suite and gates pass.
**Deferred**: Full `cargo nextest run` is owned by orchestrator reconciliation outside the Codex sandbox. CLI, LSP, wasm ABI, tree-sitter and editor work belong to downstream plans.
**Blockers**: None for FST-10.

### Session: 2026-09-30 (FST-10 test-integrity repair)
**Tasks Completed**: Rewrote `crlf.in` with real CRLF bytes (two CRLF code lines, a deep `>` continuation, a blank CRLF line, unterminated final line) and hand-wrote `crlf.out` with printf (CRLF kept, continuation at one tab, final `\n` added). `fixture_test!` now asserts the outcome is not `Refused` so fixed-point fixtures cannot pass vacuously. Added `all_blank_input_formats_to_empty` (`"\n\n\t\n"` -> `""` Changed, `""` -> `""` Unchanged).
**Verification**: `cargo nextest run fmt::` (16 passed, exit 0, `tmp/fst/FST-10/61-repair-nextest.log`); strict Clippy (exit 0, `tmp/fst/FST-10/62-repair-clippy.log`); `rustfmt --edition 2021 --check src/fmt/tests/rules.rs` (exit 0, `tmp/fst/FST-10/63-repair-fmt.log`). Edit intent and sha256 record: `tmp/fst/FST-10/60-repair-intent.log`. No fixture was reader-refused.
**Blockers**: None.

### Session: 2026-09-30 (FST-10 adversarial-review repair)
**Tasks Completed**: Fixed a G6 violation where an `elif`/`else` line popped deeper frames and dropped still-nearest attach targets from the comment chain (repro `"if true:\n\tinst a:\n\t\ts :bd\nelse:\n\t\t#@ label: x\n\tlet b 2\n"` rebound the directive from `s :bd` to `inst a:`). `format()` now computes the sorted attach-target start offsets from `read.nodes` (mirroring `directives::attach::Doc::new`/`body_targets`) and `levels::assign_code_levels` builds the comment chain from a monotonic target stack (pop `original >= o`, push on target lines only); the code-level frame pass is unchanged and the unused frame `current` was removed. Added two regression cases (`else` after a deeper branch, `elif`) to `design_comment_level_examples_match_the_spec`; the first failed before the change (`tmp/fst/FST-10/90-adv-repair-before.log`).
**Verification**: `cargo nextest run fmt::` (16 passed, exit 0, `tmp/fst/FST-10/91-adv-repair-nextest.log`); strict Clippy (exit 0, `92-adv-repair-clippy.log`); wasm32 host-wasm lib build (exit 0, `93-adv-repair-wasm.log`); rustfmt check (exit 0, `94-adv-repair-fmt.log`); line counts under 1000 (`95-adv-repair-lines.log`). Edit intent: `90-adv-repair-intent.log`.
**Blockers**: None.

## Related Plans

- **Depends On**: none
- **Next**: FST-20 (CLI), FST-21 (LSP), FST-22 (wasm export)
