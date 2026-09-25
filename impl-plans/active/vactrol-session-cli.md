# Vactrol Session Layer: CLI, REPL Front End, Session Socket (SS-CLI) Implementation Plan

**planId**: SS-CLI (issue #4, wave 4; `vactrol repl|run|serve|get|lsp` in `src/main.rs` + `src/cli/`, the native WebSocket session socket in `src/cli/ws.rs`)
**Status**: Ready
**Design Reference**: design-docs/specs/command.md (verbs, flags, env, exit codes, files, Session Protocol v1); design-docs/specs/design-implementation.md 14.2, 14.5.2 (gating), 14.5.10 (REPL threads, CLI, socket rules), 14.5.7 (`vactrol get`), 17 (localhost + token); design-docs/user-qa/pending-session-questions.md S6
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: SS-SESSION
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

`src/main.rs` currently prints only the version. This plan wires the verbs to `Session` (SS-SESSION), `pkg::get_all`
with `DirStore`/`GitStore`/`FsCache` (SS-PKG), and the native hosts (`host::native::NativeHosts::open`, `TickSource`,
`TICK_PERIOD`). It adds the loopback session socket with tungstenite and getrandom, both behind `host-native` since
SS-CONTRACTS. It runs in wave 4 in parallel with SS-LSP. The only contact point is the CONTRACTS-seeded
`vactrol::lsp::run_stdio(session: Option<&str>) -> i32`, which the `lsp` verb calls under `#[cfg(feature = "lsp")]`.

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
- `impl-plans/active/vactrol-session-cli.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **`src/main.rs`.**
   - Non-wasm32: `fn main() { std::process::exit(vactrol::cli::main(std::env::args_os().collect())) }`.
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
     prints "vactrol was built without the lsp feature" and returns 1.
   - `build_session(host: HostChoice, cwd) -> (Session, HostClock)`:
     - reads `./vactrol.lock` if present (parse errors are printed and continue with no lock);
     - builds `FsCache` at `$VACTROL_HOME/pkg`;
     - `native`: `NativeHosts::open`; on failure, warn on stderr and fall back to `noop` (S6);
     - `noop`: `Hosts::noop()`, with a VIRTUAL clock.
4. **`cli/repl.rs`.**
   - A stdin reader thread sends lines over mpsc.
   - The main thread loops on `recv_timeout(TICK_PERIOD)`, ticking the session and printing `diag`s.
   - Lines are fed to `session::repl` logic (the same continuation rules). The prompt is `vactrol> `, and continuation
     lines show `....> `.
   - EOF ends with exit 0.
5. **`cli/run.rs`.**
   - Read the file (an IO error gives exit 1), then `Session::eval` the whole document at revision 1.
   - Print the diagnostics, console output and form failures.
   - Then tick: with `--cycles N`, until N cycles have elapsed (a virtual clock under `noop`, advancing
     `TICK_PERIOD` per step without sleeping); otherwise until the process is interrupted.
   - Exit 3 if the eval reported error-severity diagnostics or failed forms, else 0.
6. **`cli/get.rs`.**
   - Read or create `./vactrol.toml`, then `add_or_raise` the target. With no version, use `latest_release` of
     `list_versions`.
   - Run `pkg::get_all` with the chosen store (`GitStore::new("https://")` or `DirStore::new(root)`) and
     `FsCache`.
   - Write `vactrol.toml` and `vactrol.lock` through a temp file and a `rename` in the same directory.
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
- `tests/cli.rs` (integration; spawns `env!("CARGO_BIN_EXE_vactrol")`; `VACTROL_HOME` set to a temp dir; no network):
  - `vactrol --version` exits 0.
  - `vactrol run <tmp>/a.vact --host noop --cycles 2` over `s [:bd :sd] > d1` exits 0. A file with a type error
    exits 3 and prints the code.
  - `vactrol repl --host noop` with piped stdin (`let a 1`, `+ a 1`, `/ 1 0`, `_2`) prints `_1 = 1` and `_2 = 2`,
    then the failure diagnostic, then `_2`'s value, and exits 0 at EOF. The failed line binds no register.
  - `vactrol get github.com/test/vactrol-pads@v1.0.0 --store dir:<fixture>` in a temp cwd writes `vactrol.toml` and a
    `vactrol.lock` with the sha256. A second `get` of v1.1.0 raises it. A corrupted fixture (traversal entry) exits 1
    with `package-integrity`.
  - `vactrol serve --host noop --port 0` (`#[cfg(feature = "host-native")]`):
    - the test reads the URL from stderr and connects with a tungstenite client;
    - `subscribe` then `eval` receives `eval-result`;
    - garbage receives `protocol-error`;
    - a wrong token gets a 401 handshake failure;
    - the test kills the child it spawned at the end.
  - `vactrol lsp` without the `lsp` feature exits 1 with the message. The LSP smoke test is SS-LSP's.

## Invariants

- CLI modules are the only place that uses `std::process`, `std::net` and `std::thread`, besides `pkg/native` and
  `lsp` (V5 excludes `cli/`).
- The token is never written to disk or logged beyond the single stderr URL line.
- Tests never touch the public network. All children spawned in tests are waited on or killed by the test itself.
- No `.rs` file reaches 800 lines.

## Edit Protocol

The common protocol in `vactrol-session-contracts.md`. Evidence goes under
`tmp/ss-session-20260925-s183/SS-CLI/attempt-<n>/`. SS-LSP runs in parallel: a `--features lsp` failure inside
`src/lsp/` is sibling-caused and recorded, never fixed here.

## Verification (`<wave>` = `cli`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| L1 | LOG(`ss-cli-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(cli) \| test(/cli::tests/)'` | `exit=0`, run > 0 |
| L2 | `git --version` | recorded (not needed by these tests; `get` tests use `--store dir:`) |

## Completion Criteria

- [ ] Items 1-7 implemented
- [ ] `vactrol repl`, `vactrol run <file.vact>`, `vactrol serve` and `vactrol get <path>` work against NoopHost (the
      `tests/cli.rs` cases pass)
- [ ] Socket security rules asserted (loopback only, 401 on a bad token or path, connection and frame limits)
- [ ] V1-V9 and L1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-CLI implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-core.md. **Parallel**: vactrol-session-lsp.md
- **Next**: vactrol-session-finalize.md


### STEP6 OUTPUT NOTE (operator, 2026-09-25, after the SS-ANALYSIS attempt-1 failure)

- The step6-implement output contract requires `changedFiles` to be an ARRAY of
  path strings (SS-ANALYSIS attempt 1 failed with "$.changedFiles must be of type
  array"). Carry `planId`; leave `verificationGaps` empty when every automated
  command passed (manual checks go under `residualRisks`). Crate-wide test
  failures caused only by a sibling branch's in-progress files or by a
  pre-existing test outside every plan's ownership are reported in the
  progress log as a dependency blocker for the operator, never fixed by
  editing unowned files.
