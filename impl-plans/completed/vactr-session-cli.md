# Vactr Session Layer: CLI, REPL Front End, Session Socket (SS-CLI) Implementation Plan

**planId**: SS-CLI (issue #4, wave 4; `vactr repl|run|serve|get|lsp` in `src/main.rs` + `src/cli/`, the native WebSocket session socket in `src/cli/ws.rs`)
**Status**: Completed (accepted by the session-185 integration review; SS-FINAL re-verified the joined tree in session 186)
**Design Reference**: design-docs/specs/command.md (verbs, flags, env, exit codes, files, Session Protocol v1); design-docs/specs/design-implementation.md 14.2, 14.5.2 (gating), 14.5.10 (REPL threads, CLI, socket rules), 14.5.7 (`vactr get`), 17 (localhost + token); design-docs/user-qa/pending-session-questions.md S6
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactr/issues/4
**dependsOn**: SS-SESSION
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

`src/main.rs` currently prints only the version. This plan wires the verbs to `Session` (SS-SESSION), `pkg::get_all`
with `DirStore`/`GitStore`/`FsCache` (SS-PKG), and the native hosts (`host::native::NativeHosts::open`, `TickSource`,
`TICK_PERIOD`). It adds the loopback session socket with tungstenite and getrandom, both behind `host-native` since
SS-CONTRACTS. It runs in wave 4 in parallel with SS-LSP. The only contact point is the CONTRACTS-seeded
`vactr::lsp::run_stdio(session: Option<&str>) -> i32`, which the `lsp` verb calls under `#[cfg(feature = "lsp")]`.

## Non-Goals

- No line-editor crate, no CLI-parser crate, no signal-handling crate. Ctrl-C ends the process by default.
- No change to `Session` behavior. A defect found in SESSION code is recorded as a dependency blocker for FINAL, not
  fixed here.
- No native HTTP proxy client, no TLS, no non-loopback bind.
- No LSP code (SS-LSP).

## writePaths (exclusive in wave 4)

- `src/main.rs`
- `src/cli/mod.rs` (fills the CONTRACTS stub), `src/cli/args.rs`, `src/cli/repl.rs`, `src/cli/run.rs`,
  `src/cli/serve.rs`, `src/cli/get.rs`, `src/cli/ws.rs`
- `src/cli/tests/mod.rs`, `src/cli/tests/args.rs`, `src/cli/tests/ws.rs`
- `tests/cli.rs`
- `impl-plans/active/vactr-session-cli.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **`src/main.rs`.**
   - Non-wasm32: `fn main() { std::process::exit(vactr::cli::main(std::env::args_os().collect())) }`.
   - wasm32: an empty `fn main() {}`, so the default-feature wasm32 build still links the bin.
2. **`cli/args.rs`.**
   - A hand-written parser into `Command { Repl { host }, Run { file, host, cycles }, Serve { file, host, port, bind },
     Get { target: Option<(PackageId, Option<Version>)>, store: StoreSpec { Git, Dir(PathBuf) } }, Lsp { session:
     Option<String> }, Version, Help }`.
   - `--host native|noop` defaults to `native`. `--cycles` is a positive int. `--port` is a u16 (default 0). `--bind`
     must be a loopback IP, otherwise a usage error.
   - Unknown verbs or flags, and missing arguments, give `UsageError` → exit 2.
3. **`cli/mod.rs`.**
   - `pub fn main(args: Vec<OsString>) -> i32` dispatches the verbs.
   - Exit codes: 0, 1 (IO, host, package, bind), 2 (usage), 3 (`run` document errors), exactly as in command.md.
   - `lsp` calls `crate::lsp::run_stdio(session.as_deref())` under `#[cfg(feature = "lsp")]`. Without the feature it
     prints "vactr was built without the lsp feature" and returns 1.
   - `build_session(host: HostChoice, cwd) -> (Session, HostClock)`:
     - reads `./vactr.lock` if present (parse errors are printed and continue with no lock);
     - builds `FsCache` at `$VACTR_HOME/pkg`;
     - `native`: `NativeHosts::open`; on failure, warn on stderr and fall back to `noop` (S6);
     - `noop`: `Hosts::noop()`, with a VIRTUAL clock.
4. **`cli/repl.rs`.**
   - A stdin reader thread sends lines over mpsc.
   - The main thread loops on `recv_timeout(TICK_PERIOD)`, ticking the session and printing `diag`s.
   - Lines are fed to `session::repl` logic (the same continuation rules). The prompt is `vactr> `, and continuation
     lines show `....> `.
   - EOF ends with exit 0.
5. **`cli/run.rs`.**
   - Read the file (an IO error gives exit 1), then `Session::eval` the whole document at revision 1.
   - Print the diagnostics, console output and form failures.
   - Then tick: with `--cycles N`, until N cycles have elapsed (a virtual clock under `noop`, advancing
     `TICK_PERIOD` per step without sleeping); otherwise until the process is interrupted.
   - Exit 3 if the eval reported error-severity diagnostics or failed forms, else 0.
6. **`cli/get.rs`.**
   - Read or create `./vactr.toml`, then `add_or_raise` the target. With no version, use `latest_release` of
     `list_versions`.
   - Run `pkg::get_all` with the chosen store (`GitStore::new("https://")` or `DirStore::new(root)`) and
     `FsCache`.
   - Write `vactr.toml` and `vactr.lock` through a temp file and a `rename` in the same directory.
   - Print the resolved versions. A `PkgError` prints its code and message and exits 1.
7. **`cli/serve.rs` + `cli/ws.rs`** (`all(feature = "host-native", not(target_arch = "wasm32"))`; without it,
   `serve` prints an error and exits 1).
   - Bind `127.0.0.1:<port>`.
   - Token: 32 bytes from `getrandom`, hex-encoded. Print `ws://127.0.0.1:<port>/session?token=<hex>` once to stderr,
     and never write it anywhere else.
   - Accept with a `tungstenite` handshake callback:
     - the path must be `/session` and the `token` query must match under a constant-time compare, else HTTP 401;
     - at most 8 live connections, a 9th gets HTTP 503;
     - `WebSocketConfig` max message and frame size 1 MiB.
   - One thread per connection reads with a 5 ms socket timeout, forwards `(conn_id, text)` over mpsc to the main
     thread, and flushes its own outbound mpsc queue.
   - The main thread owns the `Session`:
     - `codec::decode`, then `Session::apply_from(conn, ..)`;
     - route each reply to the requester and each broadcast to subscribed connections;
     - `tick` on timeout;
     - a decode error gives `protocol-error` to the requester only.
   - An optional file argument is evaluated before listening.

## Required Tests

- `src/cli/tests/args.rs`: every verb and flag; non-loopback `--bind` gives a usage error; bad `--cycles` gives a
  usage error.
- `src/cli/tests/ws.rs`: the constant-time token compare; the handshake callback rejects a wrong path, a wrong token
  and a missing token with 401; the connection limit.
- `tests/cli.rs` (integration; spawns `env!("CARGO_BIN_EXE_vactr")`; `VACTR_HOME` set to a temp dir; no network):
  - `vactr --version` exits 0.
  - `vactr run <tmp>/a.vact --host noop --cycles 2` over `s [:bd :sd] > d1` exits 0. A file with a type error
    exits 3 and prints the code.
  - `vactr repl --host noop` with piped stdin (`let a 1`, `+ a 1`, `/ 1 0`, `_2`) prints `_1 = 1` and `_2 = 2`,
    then the failure diagnostic, then `_2`'s value, and exits 0 at EOF. The failed line binds no register.
  - `vactr get github.com/test/vactr-pads@v1.0.0 --store dir:<fixture>` in a temp cwd writes `vactr.toml` and a
    `vactr.lock` with the sha256. A second `get` of v1.1.0 raises it. A corrupted fixture (traversal entry) exits 1
    with `package-integrity`.
  - `vactr serve --host noop --port 0` (`#[cfg(feature = "host-native")]`):
    - the test reads the URL from stderr and connects with a tungstenite client;
    - `subscribe` then `eval` receives `eval-result`;
    - garbage receives `protocol-error`;
    - a wrong token gets a 401 handshake failure;
    - the test kills the child it spawned at the end.
  - `vactr lsp` without the `lsp` feature exits 1 with the message. The LSP smoke test is SS-LSP's.

## Invariants

- CLI modules are the only place that uses `std::process`, `std::net` and `std::thread`, besides `pkg/native` and
  `lsp` (V5 excludes `cli/`).
- The token is never written to disk or logged beyond the single stderr URL line.
- Tests never touch the public network. All children spawned in tests are waited on or killed by the test itself.
- No `.rs` file reaches 800 lines.

## Edit Protocol

The common protocol in `vactr-session-contracts.md`. Evidence goes under
`tmp/ss-session-20260925-s183/SS-CLI/attempt-<n>/`. SS-LSP runs in parallel: a `--features lsp` failure inside
`src/lsp/` is sibling-caused and recorded, never fixed here.

## Verification (`<wave>` = `cli`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| L1 | LOG(`ss-cli-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(cli) \| test(/cli::tests/)'` | `exit=0`, run > 0 |
| L2 | `git --version` | recorded (not needed by these tests; `get` tests use `--store dir:`) |

## Completion Criteria

- [x] Items 1-7 implemented
- [x] `vactr repl`, `vactr run <file.vact>`, `vactr serve` and `vactr get <path>` work against NoopHost (the
      `tests/cli.rs` cases pass)
- [x] Socket security rules asserted (loopback only, 401 on a bad token or path, connection and frame limits)
- [x] V1-V9 and L1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-CLI implementer)` entry. Edit only this log.)

### Session: 2026-09-26 (session 185, SS-CLI implementer)

**Tasks completed**: items 1-7 and the required tests. Evidence: `tmp/ss-session-20260925-s183/SS-CLI/attempt-1/`
(`intent.md`, `pre-edit-hashes.txt`, `post-edit-hashes*.txt`, `notes.md`, `final-hashes.txt`).

- Dependency: SS-SESSION is in the dispatch `acceptedPlanIds`.
- Files: `src/main.rs`, `src/cli/{mod,args,repl,run,serve,get,ws}.rs`, `src/cli/tests/{mod,args,ws}.rs`, `tests/cli.rs`.
- `cli::main(Vec<OsString>) -> i32` dispatches the verbs; exit codes 0/1/2/3 per command.md. `build_session`:
  lock from `./vactr.lock` (a parse error is printed, then ignored), `FsCache` at `$VACTR_HOME/pkg`
  (a failure is a warning), `NativeHosts::open` falling back to noop with a warning (S6), and a virtual clock under noop.
  `lsp` calls `crate::lsp::run_stdio` under `lsp`; otherwise it prints "vactr was built without the lsp feature" and
  exits 1.
- REPL: a stdin reader thread reads bytes (a line that is not UTF-8 is evaluated lossily), and the main loop runs
  `recv_timeout(TICK_PERIOD)`, ticking while idle. This closes the SS-SESSION deferred item on continuous REPL ticking.
  The prompts `vactr> ` / `....> ` are shown only on a terminal.
- `serve` / `ws`:
  - loopback-only bind; the token is 32 bytes from `getrandom` (no weaker fallback: `serve` exits 1);
  - one stderr URL line;
  - the handshake callback checks `/session` and the token with `ct_eq`, else 401; the handshake read timeout is 5 s;
  - 8 connections (the 9th gets a raw 503 before the handshake); 1 MiB message and frame caps; 5 ms connection reads;
  - `Session` stays on the main thread, which ticks every `TICK_PERIOD` even under steady traffic.
- Deviations (recorded in `notes.md`):
  - `src/main.rs` calls `vactr::cli::run()`, which does `std::process::exit(cli::main(..))`, so V5 stays `none`.
  - `NativeSampleLoader` is the session source loader under `--host noop` too.
  - The CLI keeps its own 5 ms `TICK_PERIOD` copy, so it builds without `host-native`.
- Verification (final source; all `exit=0`):
  - V1 `target/fe-logs/ss-cli-build-s185-3.log`; V1l `ss-cli-build-lsp-s185-3.log`; extra `ss-cli-build-nodefault-s185-2.log`;
  - V2 `ss-cli-clippy-s185-3.log`; V2l `ss-cli-clippy-lsp-s185-3.log`;
  - V3 `ss-cli-nextest-s185-3.log` (984 run, 984 passed, 1 skipped); V3t `ss-cli-cargotest-s185-3.log` (984 passed,
    0 failed); V3f `ss-cli-fixtures-s185-3.log` (10 run, 10 passed);
  - L1 `ss-cli-own-s185-3.log` (30 run, 30 passed);
  - V6a `ss-cli-wasm32-s185-3.log`; V6b `ss-cli-wasm32-hostwasm-s185-3.log`; V7 `ss-cli-fmt-s185-3.log`;
  - V4 `ss-cli-linecount-s185-2.log` (largest 799, `dsp/build.rs`, pre-existing; the largest SS-CLI file is
    `tests/cli.rs`, 378 lines);
  - V5 `ss-cli-io-grep-s185-2.log` (`none`);
  - V9 `ss-cli-tree-wasm32-s185-1.log` and `ss-cli-tree-wasm32-hostwasm-s185-1.log` (both exit 0, 0 gated crates);
  - L2 `attempt-1/git-version.txt` (git 2.55.0).
- Earlier runs `-1`/`-2` also passed and were superseded by later source edits (review fixes, V5, URL).
- Pending (later workflow steps): test-integrity, adversarial and integration review; the status goes to Completed at
  SS-FINAL.

## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-009)
- **Previous**: vactr-session-core.md. **Parallel**: vactr-session-lsp.md
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
