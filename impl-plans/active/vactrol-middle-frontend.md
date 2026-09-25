# Vactrol Middle End: Path, Url and Sound Front-End Amendment (ME-FRONTEND) Implementation Plan

**planId**: ME-FRONTEND (front-end amendment of issue #2; amends TASK-001/002 outputs)
**Status**: Ready
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

- [ ] Path and url literals lex per 6.5.8, with `bad-path`/`bad-url` and the unchanged readings asserted
- [ ] `Atom::Path`/`Atom::Url` print as written; the expander passes them through
- [ ] `Value::Path`/`Url`/`Sound` with `PathVal` and `Sound` exist with the 6.5.8 eq, print and access rules
- [ ] The three PENDING manifest blocks read as stated; `verbatim_cases_appear_in_their_document` passes
- [ ] V1-V8 pass with logs cited in the progress log; `final-hashes.txt` written

## Progress Log

(Implementer: one entry per session: work done, design differences, hash/intent paths, evidence per row, blockers.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (issue #2 amendment wave)
- **Previous**: vactrol-middle-masks.md
- **Next**: ME-CHECK, ME-VM, ME-PATTERN (parallel)
