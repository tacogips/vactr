# Vactrol Middle End: Path, Url and Sound Front-End Amendment (ME-FRONTEND) Implementation Plan

**planId**: ME-FRONTEND (front-end amendment of issue #2; amends TASK-001/002 outputs)
**Status**: Completed (implemented, gate-verified, adversarial review 0 blocking, integration review accepted in sessions 175/176; removed from the dispatch manifest by the session-177 amendment; source rides in the single workflow commit; archiving to impl-plans/completed/ after the workflow commit, on user confirmation)
**Design Reference**: design-docs/specs/design-implementation.md section 6.5.8 (normative), 6.5.4 (lexer rules), 6.5.3 (eq/print/access), 7.1.7 (FRONTEND wave); lang-reference.md section 3 "path and url literals"; design-music.md "sound kits"
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/2
**dependsOn**: ME-MASKS (the `bad-path` and `bad-url` codes live in `src/types/diag.rs`, added by ME-MASKS)
**Dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json

---

## Intent and Context

The author decided (2026-09-25) that unquoted Nix-style paths (`./x ../x ~/x /abs/x`) and `scheme://...` urls are
literal types, and that `s` selects sounds, which may be sample files or MIDI-out channels. The reader and the value
model do not know any of this yet. This plan implements design 6.5.8 exactly: lexing, `Atom::Path`/`Atom::Url`,
`Value::Path`/`Value::Url`/`Value::Sound`, their equality, printing and accessor behavior. It also returns the three spec
blocks now marked PENDING in the fixture manifest to their true reading. The checker, VM and pattern waves import these
variants, so this wave lands before them.

## Non-Goals

- No path resolution, normalization, `~` expansion or file I/O anywhere (host work, TASK-007/008).
- No checker typing (ME-CHECK) and no natives (`load`, `sample`, `midi` are ME-REACTIVE / ME-INTEGRATE).
- No change to any other lexer rule, colon class or diagnostic. No new syntax beyond 6.5.8.
- No `Sound::Inst` variant (TASK-008 adds it).

## writePaths (exclusive)

- `src/reader/pathlit.rs` (new): path and url token scanning called from `lexer.rs`
- `src/reader/lexer.rs` (call site only), `src/reader/node.rs` (`Atom::Path`, `Atom::Url`), `src/reader/sexpr.rs` (print as written)
- `src/reader/tests/pathlit.rs` (new), `src/reader/tests/mod.rs` (declare it)
- `src/value/value.rs` (`PathVal`, `Sound`, the three `Value` variants), `src/value/eq.rs`, `src/value/print.rs`, `src/value/access.rs`
- `src/value/tests/pathval.rs` (new), `src/value/tests/mod.rs` (declare it)
- `impl-plans/active/vactrol-middle-frontend.md` (this plan)

## sharedPaths

- `tests/fixtures/spec/manifest.toml` (ME-INTEGRATE owns it later): ONLY the three PENDING `[[block]]` entries
  (lang-reference ordinals 3 and 5, design-music ordinal 2), keys `reader`, `reader_diags`, `note`. Pre/post hash it.

Edit the following only if the compiler forces it:

- `src/reader/line.rs`: only if the item grammar must accept the new atoms (an atom is an item; expect zero or a few
  lines). It is 741 lines and must stay under 800.
- `src/reader/mod.rs` (declare `mod pathlit;`).
- `src/value/key.rs`: only if a `match` on `Value` becomes non-exhaustive; the new variants are NOT keys and give
  `Failure(Type)` like any non-key value.
- `src/expand/expander.rs`, `src/expand/sugar.rs`, `src/expand/kernel.rs`: only if a `match` on `Atom` becomes
  non-exhaustive; the new atoms pass through unchanged and are not names.

## File-Level Changes (signatures only)

- `pathlit.rs`: `pub(crate) fn scan_path_or_url(src: &str, at: usize) -> Option<(PathTok, usize /*end*/)>` with
  `PathTok { Path, Url, BadPath, BadUrl }`. The lexer calls it only at a token start where a keyword may start (line
  start, after whitespace, `{` or `[`), before the number, operator and name rules. Grammar (6.5.8):
  - path: prefix `./`, `../`, `~/` or `/` DIRECTLY followed by a path char `[A-Za-z0-9._~-]`; the token then runs over
    path chars and `/`; it must match `PREFIX SEG ("/" SEG)*`; an empty segment, trailing `/`, or bare prefix (`./`,
    `~/`) is `bad-path`. A `/` followed by whitespace or end of line is NOT a path (division stays `Op::Div`).
  - url: `[a-z][a-z0-9+.-]*` then `://` then one or more chars from printable ASCII minus whitespace, `"`, `#`, `{`,
    `}`, `[`, `]`, `\`, `<`, `>`, `|`, `^`, backtick; empty rest is `bad-url`. Without `://` return `None`.
  - A path or url is not a pair-key token (colon after it follows 6.5.4 as after a non-key token).
- `node.rs`: `Atom::Path(Rc<str>)`, `Atom::Url(Rc<str>)` (text as written).
- `sexpr.rs`: both print as their text.
- `value.rs`: `PathVal { pub text: Rc<str>, pub file: Option<FileId> }` (`file` is `Some` for `./` and `../`, `None` for
  `/` and `~/`); `Sound { Builtin(KwId), Sample(PathVal), MidiOut(u8) }`; `Value::Path(Rc<PathVal>)`,
  `Value::Url(Rc<str>)`, `Value::Sound(Rc<Sound>)`; constructors `Value::path(text, file)`, `Value::url(text)`.
- `eq.rs`: `Path` equal iff same text and same `file`; `Url` by text; `Sound` structurally; cross-kind never equal.
- `print.rs`: path and url print as text; `(sound :bd)`, `(sound ./bd/1.wav)`, `(sound midi 1)`.
- `access.rs`: all three are truthy; `len` and `index` on them are `Failure(Type)`.

## Required Tests

- `src/reader/tests/pathlit.rs`:
  - `let a ./soundpack` reads `(let a ./soundpack)`; also `../x/y.wav`, `~/kits/a`, `/abs/x`, `https://example.org/packs/x.vact`.
  - Terminators: `{load ./a.vact}` reads `{(load ./a.vact)}`; `[./a ./b]` reads `[./a ./b]`; `./a # c` gives a comment
    trivia item; `sample https://x.org/a.wav]` stops before `]`; `https://x.org/a#frag` is the url `https://x.org/a` plus a
    comment.
  - `bad-path` for `.//x`, `./dir/`, `./` and `~/`; `bad-url` for `https://` followed by a space.
  - Unchanged readings: `/ a b` is `(/ a b)`; `1/4` and `-1/4` are ratios; `0..8` and `a..b` are ranges; `amp: 0.5`
    is `[:amp 0.5]`; `~` alone is `stray-char`; `f a:b` is still `misplaced-colon`; `./a!b` reads `./a` then `stray-char`.
  - No panic: every prefix of the strings above and 200 random ASCII strings containing `/ . ~ :` read without panic.
- `src/value/tests/pathval.rs`: eq (same text, different `file` -> not equal; `/abs` with `None` equal to itself), print
  forms, access failures, and a dict key attempt giving `Failure(Type)`.
- Spec fixtures: after the change, lang-reference block 3 and design-music block 2 are `reader = "clean"` with
  `reader_diags = []`; lang-reference block 5 keeps exactly one `stray-char` at the `fn pluck ...` line (`...` is `..`
  plus a stray `.`). Drop the "PENDING (issue #2 ...)" sentence from the three notes. If any remaining diagnostic in those
  blocks is not explained by 6.5.8, stop and report it in the progress log; do not bend a rule.

## Invariants

- All existing reader, expander and value tests pass unchanged (V3), except the three manifest blocks listed.
- `lexer.rs` stays under 800 lines (the logic lives in `pathlit.rs`); `line.rs` stays under 800.
- No panic on any input; no `unwrap`/`expect` on input-derived data.
- `tests/spec_fixtures.rs::verbatim_cases_appear_in_their_document` passes.

## Edit Protocol

Identical to ME-MASKS "Edit Protocol (common to every ME plan)", with evidence under
`tmp/me-middle-20260925-s175/ME-FRONTEND/attempt-<n>/`. Record pre/post hashes for every sharedPath edit (especially
`tests/fixtures/spec/manifest.toml`, which ME-INTEGRATE edits later).

## Verification

The ME-MASKS verification table and log rule apply with `<plan>` = `frontend` (logs
`target/fe-logs/frontend-<check>-s<S>-<n>.log`): V1, V2, V3, V3t, V3f, V6a, V6b, V4 (every `.rs` under 800 lines), V5,
V7, V8 (rustfmt on owned files). V3f must show the spec_fixtures binary ran with 0 failed.

## Completion Criteria

- [x] Path and url literals lex per 6.5.8, with `bad-path`/`bad-url` and the unchanged readings asserted
- [x] `Atom::Path`/`Atom::Url` print as written; the expander passes them through
- [x] `Value::Path`/`Url`/`Sound` with `PathVal` and `Sound` exist with the 6.5.8 eq, print and access rules
- [x] The three PENDING manifest blocks read as stated; `verbatim_cases_appear_in_their_document` passes
- [x] V1-V8 pass with logs cited in the progress log; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

### Session: 2026-09-25 (session 175, ME-FRONTEND implementer)

**Tasks Completed**: every ME-FRONTEND deliverable. New: `src/reader/pathlit.rs` (79 lines, `PathTok`,
`scan_path_or_url`), `src/reader/tests/pathlit.rs`, `src/value/tests/pathval.rs`. Edited: `src/reader/lexer.rs`
(`Tok::Path`/`Tok::Url`; `path_or_url` is tried at a keyword position for a `.`, `~`, `/` or `a-z` byte before every
other rule; 721 lines), `src/reader/node.rs` (`Atom::Path`/`Atom::Url`), `src/reader/sexpr.rs` (print as written),
`src/value/value.rs` (`PathVal`, `Sound`, `Value::Path`/`Url`/`Sound`, `Value::path`, `Value::url`), `src/value/eq.rs`,
`src/value/print.rs`, `src/value/tests/mod.rs` (declares `pathval`; the exhaustive tagging test gains the three
variants), `src/reader/tests/mod.rs` (declares `pathlit`). sharedPaths the compiler forced: `src/reader/mod.rs`
(`pub(crate) mod pathlit;`), `src/reader/line.rs` (two `leaf` arms mapping `Tok::Path`/`Tok::Url` to the atoms; 743
lines). `tests/fixtures/spec/manifest.toml`: only the three PENDING blocks (see below). Not touched: `src/value/key.rs`
(its `_` arm already gives `Failure(Type)`), the expander files (no exhaustive `Atom` match), `src/value/access.rs`
(`len`/`index`/`get` already fall to `Failure(Type)` for the new variants and `truthy` is already true; asserted in
`pathval.rs`), `src/value/mod.rs` (not a writePath; `PathVal`/`Sound` are reached as `crate::value::value::{PathVal,
Sound}`, and ME-INTEGRATE may add a re-export).

**Design and plan differences**:
- `scan_path_or_url` returns `None` for a `/` not directly followed by a path character, so `//x` stays two `/`
  operators (unchanged) and `/ a b` stays division. `./`, `../` and `~/` with nothing valid after them are `bad-path`
  (bare prefix), as the plan's test list requires.
- The lexer passes `src[..line_end]` to the scanner so a token never runs past the line.
- A bad path or url produces no token (like `bad-number`); the statement reads as `(#error)` and the next line recovers.
- Manifest: lang-reference 3 and design-music 2 are `reader = "clean"`, `reader_diags = []`. lang-reference 5 keeps
  exactly `stray-char@100`: block line 100 is the tab-indented `...` body under `fn pluck ...` (line 99). The old note
  said "block line 92"; it now says line 100. All three notes dropped the PENDING sentence. No remaining diagnostic in
  those blocks is unexplained by 6.5.8.

**Tests** (`src/reader/tests/pathlit.rs`, 8 tests): the four prefixes and a url; terminators (whitespace, `}`, `]`,
`#` comment trivia, `#frag`, `:` giving `misplaced-colon`); a url with a port, query and `git+ssh` scheme; `bad-path`
for `.//x`, `./dir/`, `./`, `~/`, `../` and `bad-url` for `https:// x` and `https://`, with recovery and spans; the
unchanged readings of `/ a b`, `{/ a b}`, `1/4`, `-1/4`, `a/b`, `0..8`, `a..b`, `amp: 0.5`, `https`, `~`, `~x`, `f a:b`,
`https:x`, `//x`, `...` and `./a!b` (`stray-char` at 5..6); a path inside interpolation; the expander passes paths and
urls through as kernel output; no panic on every prefix/suffix of the sample strings and 200 deterministic random
strings over `/ . ~ :` and brackets. `src/value/tests/pathval.rs` (6 tests): path eq by text and file (different
`file` not equal, `/abs` with `None` equal to itself, no normalization), url and sound eq, cross-kind never equal,
print forms (`(sound :bd)`, `(sound ./bd/1.wav)`, `(sound midi 1)`, nested in a list), accessors are `Failure(Type)` and
truthy, and `Key::from_value` / `put` on a dict give `Failure(Type)`.

**Verification** (session 175, n = 1, after the final code change; all `exit=0`):
- V1 `target/fe-logs/frontend-build-s175-1.log` exit=0, no warnings
- V2 `target/fe-logs/frontend-clippy-s175-1.log` exit=0
- V3 `target/fe-logs/frontend-nextest-s175-1.log` exit=0, 203 run, 203 passed, 0 failed
- V3t `target/fe-logs/frontend-cargotest-s175-1.log` exit=0, 195 unit + 8 spec_fixtures passed, 0 failed
- V3f `target/fe-logs/frontend-fixtures-s175-1.log` exit=0, 8 run, 8 passed (includes
  `verbatim_cases_appear_in_their_document` and `blocks_read_per_classification`)
- V6a `target/fe-logs/frontend-wasm32-s175-1.log` exit=0; V6b `target/fe-logs/frontend-wasm32-hostwasm-s175-1.log` exit=0
- V4 largest `.rs` files: `src/reader/line.rs` 743, `src/reader/lexer.rs` 721 (all under 800); V5 `none`; V7 empty;
  V8 `rustfmt --edition 2021 --check` on the owned and touched `.rs` files exit 0
  (`tmp/me-middle-20260925-s175/ME-FRONTEND/attempt-1/inline-checks.txt`)

**Evidence**: `tmp/me-middle-20260925-s175/ME-FRONTEND/attempt-1/{intent.md,pre-edit-hashes.txt,post-edit-hashes.txt,
final-hashes.txt,inline-checks.txt}`. The pre-edit hash of the manifest matched the checkpoint content (no drift).

**Blockers**: none. Review, the ME-INTEGRATE serial verification and the workflow commit are downstream.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (issue #2 amendment wave)
- **Previous**: vactrol-middle-masks.md
- **Next**: ME-CHECK, ME-VM, ME-PATTERN (parallel)
