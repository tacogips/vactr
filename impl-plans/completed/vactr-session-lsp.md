# Vactr Session Layer: Language Server (SS-LSP) Implementation Plan

**planId**: SS-LSP (issue #4, wave 4; `vactr lsp` over stdio with tower-lsp behind the `lsp` feature)
**Status**: Completed (accepted by the session-185 integration review; SS-FINAL re-verified the joined tree in session 186)
**Design Reference**: design-docs/specs/design-implementation.md 14.3, 14.5.11 (threading, features, attach, smoke test), 14.5.7 (LSP-only analysis without execution), 14.5.8 (directive lint), 14.5.2 (gating); design-docs/specs/command.md (`vactr lsp [--session <ws-url>]`)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactr/issues/4
**dependsOn**: SS-SESSION
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

TASK-009 requires `vactr lsp` behind the `lsp` feature (tower-lsp), with these features:
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
- `impl-plans/active/vactr-session-lsp.md`

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
   (severity map, `code` = kebab string, `source` = "vactr").
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
  - spawns `env!("CARGO_BIN_EXE_vactr") lsp` with piped stdin/stdout and Content-Length framing;
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

The common protocol in `vactr-session-contracts.md`. Evidence goes under
`tmp/ss-session-20260925-s183/SS-LSP/attempt-<n>/`. SS-CLI runs in parallel. A failure inside `src/cli/` or
`src/main.rs` is sibling-caused and recorded.

## Verification (`<wave>` = `lsp`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| G1 | LOG(`ss-lsp-smoke`): `CARGO_TERM_QUIET=true cargo test --features lsp --test lsp_smoke` | `exit=0`, test count > 0 |
| G2 | LOG(`ss-lsp-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run --features lsp -E 'test(/lsp::tests/)'` | `exit=0`, run > 0 |

## Completion Criteria

- [x] Items 1-5 implemented
- [x] The LSP smoke test over stdio passes (`publishDiagnostics` plus hover on spec examples)
- [x] V1-V9 and G1-G2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-LSP implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 185, SS-LSP implementer)

**Dependency**: SS-SESSION is in the dispatch `acceptedPlanIds`. Evidence:
`tmp/ss-session-20260925-s183/SS-LSP/attempt-1/` (`pre-edit-hashes.txt`, `intent.md` with one line per edit,
`final-hashes.txt`, `scratch-logs/`).

**Delivered** (only writePaths were edited):
- `src/lsp/mod.rs`: `run_stdio(session: Option<&str>) -> i32` keeps the seeded signature. It runs a current-thread
  tokio runtime and tower-lsp over stdin/stdout, and returns 0 only when `shutdown` preceded the end of input.
- `src/lsp/analysis.rs`: the ONE analysis thread (`spawn`) owns an `Analyzer` holding every `!Send` value. It
  talks over `std::sync::mpsc` using `AnalysisReq` (Configure/Open/Change/Close/Hover/Complete/Format/
  RuntimeDiags), each with a tokio `oneshot` reply.
  - Per document it runs `session::eval::analyze` plus `directives::build_table`.
  - Hover shows the innermost typed node.
  - Completion offers NativeTable names, top-level names, the manifest `:sound`/`:synth`/`:control` keywords,
    package prefixes and qualified names from fetched packages, filtered by the word before the cursor.
  - `format_edits` is whitespace-only: it skips every line containing `#@`.
  - `PkgConfig::load` reads `<root>/vactr.lock`. It opens the package cache only when the cache and its
    `.staging` directory already exist, so analysis never creates files.
- `src/lsp/convert.rs`: byte offset and UTF-16 `Position` conversion, `Diagnostic`/`WireDiag` to
  `lsp_types::Diagnostic` (severity map, kebab code, source `vactr`).
- `src/lsp/server.rs`: `Backend` (`initialize` advertises full sync, hover, completion triggered by `:` and `.`,
  and formatting). Handlers hold only `Send` values, so no `Rc` crosses an `.await`.
  - Attach: `attach` starts a tungstenite client thread. It sends `subscribe {diagnostics: true}` and forwards
    `diag` bodies over a tokio channel to `forward_runtime_diags`, which merges them (add per file, clear per
    slot) and re-publishes. A failure logs one stderr line and the server continues.
  - `ExitAwareStdin` exists because tower-lsp 0.20's `Server::serve` returns only at end of input, or on the
    NEXT message after `exit`. The wrapper reports end of input right after the `exit` notification, so a
    client that keeps the pipe open still sees the process exit. `tower` is not a direct dependency and
    `Cargo.toml` is not in this plan's writePaths, so a Service wrapper was not possible.
- Tests:
  - `src/lsp/tests/analysis.rs` (9 tests): UTF-16 positions, hover int/float, completion (`sine`, `:bd`),
    whitespace-only formatting with `#@` lines byte-identical, an unfetched import giving
    `package-not-fetched` with nothing written, undefined-name and duplicate-label lint, runtime diag
    merge/clear, `ExitAwareStdin`, and attach against a loopback tungstenite server.
  - `tests/lsp_smoke.rs`: lang-reference block 1 over stdio gives exactly one publishDiagnostics (clean).
    Hover gives `int` on a literal and `total: int` on a bound name; a second document gets `undefined-name`;
    shutdown/exit returns code 0 within 10 s.

**Deviations (plan-level)**:
- `AnalysisReq::RuntimeDiags` carries the whole `DiagBody` (`add` plus `clear`) instead of `{file, diags}`,
  so slot clears are honored.
- The "recording sink stays empty" proof: `Analyzer` owns no host or runtime by construction. The test proves
  "no execution or fetch" through the file system instead: the cache and workspace are unchanged after analysis.

**Verification** (real tree, attempt 2, after SS-CLI's files compiled; all `exit=0`):
- V1 `target/fe-logs/ss-lsp-build-s185-2.log`; V1l `ss-lsp-build-lsp-s185-2.log`.
- V2 `ss-lsp-clippy-s185-2.log`; V2l `ss-lsp-clippy-lsp-s185-2.log`.
- V3 `ss-lsp-nextest-s185-2.log`: 984 passed, 1 skipped.
- V3t `ss-lsp-cargotest-s185-2.log`: 984 passed, 0 failed.
- V3f `ss-lsp-fixtures-s185-2.log`: 10 passed.
- V6a `ss-lsp-wasm32-s185-2.log`; V6b `ss-lsp-wasm32-hostwasm-s185-2.log`.
- V7 `ss-lsp-fmt-s185-2.log`.
- V9 `ss-lsp-tree-wasm32-s185-2.log` and `ss-lsp-tree-wasm32-hostwasm-s185-2.log`: 0 matches for
  tokio/tower-lsp/tungstenite/getrandom/cpal/midir.
- V4 `ss-lsp-linecount-s185-2.log`: largest file 799 lines, `src/dsp/build.rs`, not owned; the largest owned
  file is `src/lsp/analysis.rs` at 564.
- V5 `ss-lsp-io-grep-s185-2.log`: one hit, `src/main.rs:9 std::process::exit`. That is SS-CLI's native binary
  entry, not owned here, recorded for SS-CLI/FINAL.
- G1 `ss-lsp-smoke-s185-2.log`: 1 passed. G2 `ss-lsp-own-s185-2.log`: 9 passed.
- Attempt-1 logs (`*-s185-1.log`, exit=101) failed only on SS-CLI's in-progress `src/cli/repl.rs:29` and the
  missing `src/cli/tests/ws.rs`. They are sibling-caused, and the attempt-2 rerun supersedes them.

**Pending downstream**: formal test-integrity/adversarial/integration review; marking the plan Completed is SS-FINAL's.


## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-009)
- **Previous**: vactr-session-core.md. **Parallel**: vactr-session-cli.md
- **Next**: vactr-session-finalize.md


### STEP6 OUTPUT NOTE (operator, 2026-09-25, after the SS-ANALYSIS attempt-1 failure)

- The step6-implement output contract requires `changedFiles` to be an ARRAY of
  path strings (SS-ANALYSIS attempt 1 failed with "$.changedFiles must be of type
  array"). Carry `planId`; leave `verificationGaps` empty when every automated
  command passed (manual checks go under `residualRisks`). Crate-wide test
  failures caused only by a sibling branch's in-progress files or by a
  pre-existing test outside every plan's ownership are reported in the
  progress log as a dependency blocker for the operator, never fixed by
  editing unowned files.

### INTEGRATION REVIEW OUTPUT NOTE (operator, 2026-09-26, after two adapter rejections in session 185)

- The integration-review step output MUST be an ENVELOPE with two top-level
  keys: `"when"` (the routing flags `needs_revision`, `redispatch_required`,
  `repair_in_place`, `plans_remaining`) and `"payload"` (an OBJECT holding the
  review itself: `needs_revision`, `loopGate`, `acceptedPlanIds`, `findings`,
  `recoveryDiagnostic`, summaries, evidence paths). Two attempts were rejected
  with "payload must be an object when when is provided" because the review
  fields were emitted at the top level next to `when` instead of inside
  `payload`. `acceptedPlanIds` lists only plans present in the manifest's
  `plans[]`.

### CLOSING NOTE (SS-FINAL, session 186, 2026-09-26)

- Status set to Completed by SS-FINAL. Join integrity: tmp/ss-session-20260925-s183/SS-FINAL/attempt-1/join-integrity.txt.
- Final-tree evidence: target/fe-logs/ss-final-<check>-s186-1.log (build, build-lsp, clippy, clippy-lsp, fmt, nextest 984/984, cargotest 984, fixtures 10/10, lsp-smoke 1/1, cli 9/9, session 132/132, example, wasm32, wasm32-hostwasm; all exit=0).
