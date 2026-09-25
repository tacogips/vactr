# Vactrol Session Layer: Language Server (SS-LSP) Implementation Plan

**planId**: SS-LSP (issue #4, wave 4; `vactrol lsp` over stdio with tower-lsp behind the `lsp` feature)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 14.3, 14.5.11 (threading, features, attach, smoke test), 14.5.7 (LSP-only analysis without execution), 14.5.8 (directive lint), 14.5.2 (gating); design-docs/specs/command.md (`vactrol lsp [--session <ws-url>]`)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: SS-SESSION
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

TASK-009 requires `vactrol lsp` behind the `lsp` feature (tower-lsp), with these features:
- diagnostics;
- hover from the checker types (`CheckResult.types`, the design's `TypedInfo`);
- completion from the prelude and the manifest;
- formatting;
- optional attach to a live session socket.

The criterion is the stdio smoke test: `publishDiagnostics` and hover on spec examples.

What exists:
- SS-SESSION provides `session::eval::analyze(src, file, pkg_view) -> Analysis { diags, types, alias_env }`, which
  performs no execution.
- SS-DIRECTIVES provides the lint diagnostics through `build_table`.
- SS-CONTRACTS seeded `lsp/mod.rs` with `pub fn run_stdio(session: Option<&str>) -> i32`, which SS-CLI's `lsp` verb
  calls. Keep that exact signature.

The plan runs in wave 4 in parallel with SS-CLI.

## Non-Goals

- Formatting changes whitespace only: it strips trailing whitespace and ensures one final newline. It never reflows
  code, comments or directives.
- No semantic tokens, rename, code actions or go-to-definition.
- No edits outside `src/lsp/` and `tests/lsp_smoke.rs`. A defect in `session::eval::analyze` is recorded as a
  dependency blocker for FINAL.

## writePaths (exclusive in wave 4)

- `src/lsp/mod.rs` (fills the CONTRACTS seed; keeps the `run_stdio` signature), `src/lsp/server.rs`,
  `src/lsp/analysis.rs`, `src/lsp/convert.rs`
- `src/lsp/tests/mod.rs`, `src/lsp/tests/analysis.rs`
- `tests/lsp_smoke.rs`
- `impl-plans/active/vactrol-session-lsp.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **`lsp/mod.rs`.** `run_stdio(session)` builds a current-thread tokio runtime and serves `tower_lsp::Server` over
   stdin and stdout. It returns 0 on a clean `shutdown`/`exit`, else 1.
2. **`lsp/analysis.rs`: the ONE analysis thread (14.5.11).**
   - A `std::thread` owns every `!Send` state: interner, documents (`uri → (version, text)`) and the package view
     (read-only lock plus `FsCache`, when present in the workspace root).
   - Requests arrive over a `std::sync::mpsc` channel as `AnalysisReq { Open, Change, Close, Hover{uri,pos},
     Complete{uri,pos}, Format{uri}, RuntimeDiags{file, diags} }`, each carrying a `tokio::sync::oneshot` reply.
   - Per document it runs `session::eval::analyze` plus `directives::build_table` and caches the diagnostics and the
     node type map.
   - Hover: the innermost node whose span contains the offset, rendered as its `Ty` display.
   - Completion: `NativeTable` names, document top-level names, manifest sound/synth/control keywords (with `:`),
     package prefixes and qualified names.
3. **`lsp/server.rs`.**
   - `impl LanguageServer`: `initialize` advertises full text sync, hover, completion (trigger `:` and `.`) and
     formatting. `did_open`, `did_change` and `did_close` forward to the analysis thread and then publish
     diagnostics. `hover`, `completion` and `formatting` forward and await the reply.
   - No `Rc` crosses an `.await`.
4. **`lsp/convert.rs`.** Byte offset ↔ LSP `Position` (UTF-16 columns), `Diagnostic` → `lsp_types::Diagnostic`
   (severity map, `code` = kebab string, `source` = "vactrol").
5. **Attach (`--session <url>`).** A background `std::thread` connects with a `tungstenite` client, sends `subscribe
   {diagnostics: true}`, and forwards `diag` messages as `RuntimeDiags`. They merge into the matching document (the
   `file` string compared to the URI path), and diagnostics are re-published. A connection failure logs once to
   stderr and the server continues standalone.

## Required Tests

- `src/lsp/tests/analysis.rs`:
  - UTF-16 position conversion around multibyte strings;
  - hover over a `let`-bound number gives `int`/`float` text;
  - completion contains `sine` and `:bd`;
  - formatting yields only whitespace edits and leaves `#@` lines byte-identical;
  - an unfetched import gives `package-not-fetched` and no execution (a recording sink stays empty).
- `tests/lsp_smoke.rs` (`#![cfg(feature = "lsp")]`):
  - spawns `env!("CARGO_BIN_EXE_vactrol") lsp` with piped stdin/stdout and Content-Length framing;
  - sends `initialize`/`initialized`, then `textDocument/didOpen` of the lang-reference.md block (ordinal 1,
    extracted at test time from `CARGO_MANIFEST_DIR/design-docs/specs/lang-reference.md`, the same way as the spec
    runner);
  - asserts one `textDocument/publishDiagnostics` for that URI;
  - a `textDocument/hover` over a bound name returns contents containing its type;
  - a second document with a known `undefined-name` error gets that code in its diagnostics;
  - `shutdown`/`exit` ends the process with code 0 within 10 s (the test kills the child on timeout and fails).

## Invariants

- The `lsp` code compiles only under `all(feature = "lsp", not(target_arch = "wasm32"))`. The default and wasm32
  builds never pull in tokio or tower-lsp (V9).
- Analysis never executes user code, never touches the network, and never writes files.
- No `.rs` file reaches 800 lines.

## Edit Protocol

The common protocol in `vactrol-session-contracts.md`. Evidence goes under
`tmp/ss-session-20260925-s183/SS-LSP/attempt-<n>/`. SS-CLI runs in parallel. A failure inside `src/cli/` or
`src/main.rs` is sibling-caused and recorded.

## Verification (`<wave>` = `lsp`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| G1 | LOG(`ss-lsp-smoke`): `CARGO_TERM_QUIET=true cargo test --features lsp --test lsp_smoke` | `exit=0`, test count > 0 |
| G2 | LOG(`ss-lsp-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --features lsp -E 'test(/lsp::tests/)'` | `exit=0`, run > 0 |

## Completion Criteria

- [ ] Items 1-5 implemented
- [ ] The LSP smoke test over stdio passes (`publishDiagnostics` plus hover on spec examples)
- [ ] V1-V9 and G1-G2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-LSP implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-core.md. **Parallel**: vactrol-session-cli.md
- **Next**: vactrol-session-finalize.md
