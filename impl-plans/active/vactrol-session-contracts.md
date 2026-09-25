# Vactrol Session Layer: Contracts and Seeds (SS-CONTRACTS) Implementation Plan

**planId**: SS-CONTRACTS (issue #4, wave 1; the dependencies, codes, shapes and seeds every TASK-009 wave builds on)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 14.5.2 (dependencies, gating), 14.5.3 (ownership rule, file table), 14.5.6 (`ChangeSet`), 14.5.9 (`Sound::Buffer`, taps, staged `Capture`/`Render`), 14.5.10 (console registers), 14.5.12 (codes, waves, verification); 6.5.7 (evidence rule)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: none
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

Issue #4 builds TASK-009 of `impl-plans/active/vactrol-core.md` (Session, protocol, packages, directives, REPL, LSP,
CLI, self-analysis) in eight waves (design 14.5.12). This plan runs ALONE in wave 1. It lands everything two or more
later waves share, so that PKG, DIRECTIVES and ANALYSIS can run at the same time in one working directory without
writing a common file:

- the Cargo dependencies and features;
- the `src/lib.rs` declarations with empty `mod.rs` stubs;
- `ChangeSet`;
- the closed code list;
- the `Sound::Buffer` / `SampleBuf` shape;
- the `StagedEffect::{Capture, Render}` shapes;
- the tap and analysis contracts in `host/caps.rs`;
- the `SourceLoader::analysis` hook;
- the `compile/compiler.rs` split plus console-register resolution;
- the `Namespace` console slots.

The tree it starts from is TASK-001..008 complete (HEAD `e5d2130`).
- `src/main.rs` prints the version.
- `Cargo.toml` has no regular dependencies and an empty `lsp` feature.
- `compile/compiler.rs` is 798 lines and `ns/evaluator.rs` is 777.
- `DiagCode` has 75 codes (warnings/hints assertion `(11, 1)`) and `FailCode` has 22.

## Non-Goals

- No session, package, directive, analysis, CLI or LSP behavior. Stub files hold one `//! Owned by SS-<WAVE> (design
  14.5.x).` line and no items.
- No runtime handling of `Capture`/`Render`. The new arms in `sched/runtime.rs` record a `host-unavailable` fault and
  do nothing else; ANALYSIS replaces them.
- No playback of `Sound::Buffer`. The route arm in `ns/insts.rs` returns `Failure(host-unavailable)`; ANALYSIS
  replaces it.
- No edit to `ns/evaluator.rs`, `dsp/engine.rs`, `dsp/build.rs`, `compile/matchc.rs`, `impl-plans/README.md` or
  `vactrol-core.md`.
- No crate-wide `cargo fmt` rewrite.

## writePaths (exclusive in wave 1)

- `Cargo.toml`, `Cargo.lock`, `src/lib.rs`
- `src/session/mod.rs` (seed), `src/session/changes.rs`
- stubs: `src/pkg/mod.rs`, `src/directives/mod.rs`, `src/cli/mod.rs`, `src/lsp/mod.rs`, `src/ns/eval_doc.rs`,
  `src/host/native/tap.rs`
- `src/ns/mod.rs`, `src/host/native/mod.rs`, `src/value/mod.rs`, `src/compile/mod.rs` (declarations only)
- `src/types/diag.rs`, `src/vm/fail.rs`
- `src/value/value.rs`, `src/value/sample.rs` (seed), `src/value/print.rs`, `src/value/eq.rs`, `src/value/tests/mod.rs`
- `src/ns/stage.rs`, `src/ns/load.rs`, `src/ns/namespace.rs`, `src/host/caps.rs`, `src/host/noop.rs`
- `src/compile/compiler.rs`, `src/compile/names.rs` (new), `src/compile/tests/compile.rs`
- Conditional (edit ONLY if the compiler reports a non-exhaustive match or a missing arm after the new variants, and
  record each edit in the progress log): `src/sched/runtime.rs`, `src/sched/commit.rs`, `src/ns/insts.rs`,
  `src/vm/natives/sound.rs`, `src/vm/natives/tex.rs`, `src/vm/tests/integrate_sound.rs`, `src/ns/tests/stage.rs`,
  `src/types/infer_call.rs`, `src/types/check.rs`, `src/host/native/loader.rs`, `src/host/testing.rs`,
  `src/host/tests/contracts.rs` (the `NoopHost` default-method test only)
- `impl-plans/active/vactrol-session-contracts.md`

## sharedPaths

None. This plan runs alone in wave 1. The seeded or stubbed files above pass to later sequential waves under the
ownership rule of design 14.5.3, and each later plan lists them again in its own writePaths.

## File-Level Changes (signatures and behavior; no code in this plan)

1. **`Cargo.toml` (14.5.2).**
   - Regular dependencies: `serde` (feature `derive`), `serde_json` and `miniz_oxide`.
   - Under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, all `optional = true`: `tungstenite` (default
     features, no TLS feature), `getrandom`, `tower-lsp`, `tokio` (features `rt`, `io-std`, `macros`, `sync`).
   - Features: `host-native = ["dep:cpal", "dep:midir", "dep:tungstenite", "dep:getrandom"]`,
     `lsp = ["host-native", "dep:tower-lsp", "dep:tokio"]`. The `default` and `host-wasm` features are unchanged.
   - Version policy (12.8.10): take the highest release whose whole resolved tree builds on rustc 1.83, and use
     `cargo update -p <crate> --precise <ver>` where a transitive crate needs a newer rustc. Record every chosen
     version and every pin.
2. **`src/lib.rs`.**
   - Add `pub mod session; pub mod pkg; pub mod directives;`.
   - Add `#[cfg(not(target_arch = "wasm32"))] pub mod cli;`.
   - Add `#[cfg(all(feature = "lsp", not(target_arch = "wasm32")))] pub mod lsp;`.
3. **`src/session/mod.rs` seed.** Exactly `//! Session layer (design 14, 14.5).` plus `pub mod changes;`. SESSION
   extends this file later.
4. **`src/session/changes.rs` (14.5.6).**
   - `Change { from: u32, to: u32, insert_len: u32 }` (base-revision byte offsets).
   - `ChangeSet { changes: Vec<Change> }`, built through `ChangeSet::new(Vec<Change>) -> Result<ChangeSet, ChangeError>`.
     It rejects changes that are unsorted, overlapping, or have `from > to`.
   - `enum Mapped { Moved(Span-like start,end), Touched }` and `fn map_span(&self, start: u32, end: u32) -> Mapped`.
     A span entirely before a change is unchanged. A span entirely after a change shifts by `insert_len - (to - from)`.
     A span that overlaps or touches a replaced range, or contains an insertion point strictly inside it, is `Touched`.
   - `fn compose(&self, later: &ChangeSet) -> ComposedMap`, or a `map_through(&[ChangeSet])` helper that applies the
     maps in order.
   - Inline `#[cfg(test)] mod tests` (the file stays well under 300 lines) with these cases:
     - insertion above: moved;
     - edit inside: touched;
     - deletion spanning: touched;
     - two successive insertions composed: moved by the sum;
     - insertion then an edit of the shifted span: touched;
     - invalid change sets rejected.
5. **Stubs.** `pkg/mod.rs`, `directives/mod.rs`, `cli/mod.rs`, `ns/eval_doc.rs` (declared in `ns/mod.rs` as
   `pub mod eval_doc;`) and `host/native/tap.rs` (declared in `host/native/mod.rs` as `pub mod tap;`). Each holds one
   `//!` line naming its owner wave. The ONE seeded item is in `lsp/mod.rs`:
   `pub fn run_stdio(session: Option<&str>) -> i32`, which prints "vactrol lsp: not implemented yet" to stderr and
   returns 1. It fixes the contract that the CLI's `lsp` verb calls, so that SS-CLI and SS-LSP can run in parallel in
   wave 4. SS-LSP replaces the body and keeps the signature.
6. **Codes (14.5.12), each with a message template and default severity.**
   - `DiagCode` warnings: `unknown-directive-site`, `unknown-parameter`, `unknown-label`, `duplicate-label`,
     `ambiguous-selector`, `cc-out-of-range`, `reserved-key`, `package-not-fetched`.
   - `DiagCode` errors: `package-not-locked`, `package-integrity`, `package-load-failed`, `package-resolve`.
   - Update the assertions: `DiagCode::ALL.len()` and the names length become 87, and `(warnings, hints)` becomes
     `(19, 1)`.
   - `FailCode`: `capture-pending`, `beyond-capability`. The assertions become 24.
7. **`src/value/sample.rs` seed (14.5.9).**
   - `SampleBuf { id: u64, rate: u32, state: RefCell<BufState> }`. `id` is unique per process, taken from a
     `thread_local` counter at construction. It is the buffer identity used for sample installation.
   - `enum BufState { Pending, Ready(Rc<[f32]> /* interleaved stereo */), Failed { code: FailCode, message: Rc<str> } }`.
   - Constructors `SampleBuf::pending(rate) -> Rc<SampleBuf>` and `SampleBuf::ready(rate, frames) -> Rc<SampleBuf>`.
   - Methods `state(&self) -> Ref<BufState>`, `fill(&self, frames)` and `fail(&self, code, msg)`.
   - `pub mod sample;` is added to `value/mod.rs`.
   - ANALYSIS later adds the read helpers.
8. **`Sound::Buffer(Rc<SampleBuf>)` in `value/value.rs`, with minimal arms.**
   - Print: `(sound buffer <frames> frames)`, or `(sound buffer pending)` / `(sound buffer failed)`.
   - `deep_eq`: pointer equality.
   - `value/tests/mod.rs`: the kind list, if it enumerates sounds.
   - The checker types it `sound` (no change expected; `types/*` edits are conditional).
9. **`StagedEffect` in `ns/stage.rs`.**
   - Add `Capture { buf: Rc<SampleBuf>, src: TapSrc, cycles: Ratio64, origin: Span }` and
     `Render { buf: Rc<SampleBuf>, cycles: Ratio64, origin: Span }`, with doc comments naming ANALYSIS as the consumer.
   - `RecordingSink` records them like every other variant.
   - `sched/runtime.rs` gets conditional arms that mark `buf` Failed(`host-unavailable`, "handled by SS-ANALYSIS").
10. **Taps and analysis context in `host/caps.rs` (14.5.9).**
    - `enum TapSrc { Master, Bus(KwId) }`.
    - `trait TapReader { fn snapshot(&mut self, src: &TapSrc, frames: usize, out: &mut Vec<f32>) -> Result<(), Failure>; }`.
    - `CaptureId(u32)` and `enum CapturePoll { Pending, Done, Failed(Failure) }`.
    - `AudioHost` default methods:
      - `tap_reader(&mut self) -> Option<Box<dyn TapReader>>` returns `None`;
      - `arm_capture(&mut self, src: &TapSrc, start: f64, frames: usize) -> Result<CaptureId, Failure>` returns
        `Err(host-unavailable)`;
      - `poll_capture(&mut self, id: CaptureId, out: &mut Vec<f32>) -> CapturePoll` returns
        `Failed(host-unavailable)`.
    - `SampleLoader::register_bank(&mut self, kw: KwId, files: Vec<PathVal>) -> Result<(), Failure>`, default
      `Err(host-unavailable)`.
    - `SampleSrc` gains `Buffer { id: u64 }`, the `SampleBuf::id` of a captured or rendered buffer. `SampleLoader::load`
      of a `Buffer` source returns `host-unavailable` in every existing loader; ANALYSIS supplies the data from the
      buffer itself at commit. Any exhaustive `SampleSrc` match gets that arm: `src/host/native/loader.rs` is a
      conditional path, which PKG later owns in wave 2.
    - `pub struct AnalysisCx { pub caps: CapabilitySet, pub taps: Option<Box<dyn TapReader>> }`.
    - `NoopHost` keeps every default.
11. **`ns/load.rs`.** `SourceLoader` gains `fn analysis(&mut self) -> Option<&mut AnalysisCx> { None }`. This is how
    the design's "registered the same way `register_load` is, no new VM field" is carried out: the Session's loader
    returns its context through the existing `LoaderHost` slot. A native reaches it by downcasting the VM host to
    `LoaderHost` and calling `.0.analysis()`.
12. **`compile/compiler.rs` split (it is at 798 lines, 14.5.3).**
    - Move name and atom resolution (identifier, qualified, console register) into the new `compile/names.rs`, a
      pure move with no behavior change, and declare it in `compile/mod.rs`.
    - Then `Atom::ConsoleReg(n)` compiles to a read of the namespace console slot `n`. An unset register is
      `Failure(undefined-name)` with the message "console register `_n` is not set". The checker keeps typing it
      `any`.
    - A test in `compile/tests/compile.rs` evaluates `_1` after `set_console_register(1, 42)` in a console
      `FileId::CONSOLE` read and gets `42`.
13. **`ns/namespace.rs`.** Add `set_console_register(&self, n: u32, v: Value)` and
    `console_register(&self, n: u32) -> Option<Value>`. Console registers are kept apart from session names: they
    are not in `session_names`, never cause `rebinding`, and are invisible to a non-console file.

## Required Tests

- `session/changes.rs` inline tests (item 4).
- The code-count assertions in `types/diag.rs` and `vm/fail.rs` (item 6).
- The console-register compile test (item 12).
- A `Sound::Buffer` print and pointer-equality test in the existing `value/tests` module.
- A `NoopHost` default-method test: `tap_reader` is `None`, `arm_capture` is `host-unavailable`, and `register_bank`
  is `host-unavailable`. Put it in the existing `src/host/tests/contracts.rs` ONLY if that file needs no other change;
  otherwise put it inline in `host/noop.rs` under `#[cfg(test)]`.
- Every existing test keeps passing (count recorded before and after).

## Invariants

- Both wasm32 builds stay green. `cargo tree -e normal --target wasm32-unknown-unknown` (default features, and
  `--no-default-features --features host-wasm`) lists none of `tungstenite`, `getrandom`, `tokio`, `tower-lsp`,
  `cpal`, `midir`.
- No `std::{thread,fs,time,net,process}` in core modules (V5).
- No `.rs` file reaches 800 lines; after the split `compile/compiler.rs` is well under 800.
- Behavior is unchanged except for the console-register resolution. The spec fixtures stay green with unchanged pins.

## Edit Protocol (common to every SS plan)

1. Evidence directory: `tmp/ss-session-20260925-s183/<planId>/attempt-<n>/` (gitignored by `**/tmp`).
2. Before each edit, read the file fresh and append `shasum -a 256 <file>` to `pre-edit-hashes.txt`. Before the first
   edit of a file this plan did not create, and of every conditional or shared path, write a one-line intent
   (file, purpose) to `intent.md`. After each edit, append the post-edit hash to `post-edit-hashes.txt`.
3. Drift: if a pre-edit hash differs from this plan's last recorded post-edit hash for that file, stop, re-read, and
   re-apply the recorded intent. Never revert another plan's hunk. Record the event in `notes.md`.
4. Keep the tree compiling between edits. A build or test error in a file outside this plan's writePaths/sharedPaths
   is waited out and recorded as sibling-caused, never fixed here. Serial repair happens at the join (SESSION after
   wave 2, FINAL at the end).
5. `rustfmt --edition 2021` on owned `.rs` files only. Never run crate-wide `cargo fmt` (the FINAL plan may, once).
6. Never run `git reset`, `git clean`, `git stash` or `git checkout -- <path>`. Never create branches or worktrees.
   Never commit.
7. Among plan documents, edit only this plan's own progress log and status line. Write Rust through the rust-coding
   agent, then run the check-and-test-after-modify agent (CLAUDE.md).
8. Do not create a file that is not listed in writePaths. If one is truly needed, extend a listed file instead and
   record the omission for FINAL.
9. At the end, write `final-hashes.txt` with the sha256 of every file this plan wrote.

## Verification (foreground; design 6.5.7 evidence rule, common to every SS plan)

`mkdir -p target/fe-logs` first. `LOG` is a new file `target/fe-logs/ss-<wave>-<check>-s<S>-<n>.log`:
- `<wave>` is this plan's short name (`contracts` here);
- `<S>` is the Riela session number;
- `<n>` counts from 1 per check within the session, and a log is never overwritten.

Run each cargo row as `(set -o pipefail; CMD 2>&1 | tee LOG); echo "exit=$?" >> LOG`. The progress log cites the
counting log (the last run after the final code change) and its `exit=`. V3 and V3t also cite run and passed counts,
which must be non-zero. A missing log, a log without `exit=`, or a truncated log fails the row.

| # | Command (`CMD`) | `<check>` | Evidence |
|---|---------|-----------|----------|
| V1 | `CARGO_TERM_QUIET=true cargo build` | `build` | `exit=0` |
| V1l | `CARGO_TERM_QUIET=true cargo build --features lsp` | `build-lsp` | `exit=0` |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` | `clippy` | `exit=0` |
| V2l | `CARGO_TERM_QUIET=true cargo clippy --all-targets --features lsp -- -D warnings` | `clippy-lsp` | `exit=0` |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run` | `nextest` | `exit=0`, run > 0, 0 failed |
| V3t | `CARGO_TERM_QUIET=true cargo test` | `cargotest` | `exit=0`, non-zero counts |
| V3f | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `fixtures` | `exit=0`, run > 0 |
| V6a | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown` | `wasm32` | `exit=0` |
| V6b | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` | `wasm32-hostwasm` | `exit=0` |
| V7 | `CARGO_TERM_QUIET=true cargo fmt --check` | `fmt` | `exit=0`. In a parallel wave, a diff only in another in-flight plan's file is recorded as sibling-caused. |
| V4 | `find src tests examples -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every file under 800 lines |
| V5 | `grep -rnE 'std::(thread\|fs\|time\|net\|process)' src/ --exclude-dir=native --exclude-dir=cli --exclude-dir=lsp --exclude-dir=tests \|\| echo none` | - | prints `none` |
| V9 | `CARGO_TERM_QUIET=true cargo tree -e normal --target wasm32-unknown-unknown` and the same with `--no-default-features --features host-wasm` | - | neither output contains `tungstenite`, `getrandom`, `tokio`, `tower-lsp`, `cpal` or `midir` |

Plan-specific rows:

| # | Command | Evidence |
|---|---------|----------|
| C1 | LOG(`ss-contracts-audit`): `cargo audit` | `exit=0`, or every advisory listed with its dependency path (a vulnerability in a new crate's tree blocks the plan) |
| C2 | `git diff --stat -- Cargo.toml Cargo.lock` | only this plan's dependency change; versions and `--precise` pins recorded |
| C3 | `wc -l src/compile/compiler.rs src/compile/names.rs` | both under 800 |

## Completion Criteria

- [ ] Dependencies and features per item 1; C1-C3 recorded; V9 clean
- [ ] `src/lib.rs` declarations, the `session/mod.rs` seed and every stub of item 5 exist
- [ ] `ChangeSet` and its tests (item 4)
- [ ] 12 `DiagCode`s and 2 `FailCode`s; assertions 87, `(19, 1)`, 24
- [ ] `Sound::Buffer`/`SampleBuf` seed, `StagedEffect::{Capture, Render}`, the tap and analysis contracts, and
      `SourceLoader::analysis`
- [ ] `compile/compiler.rs` split, console registers resolved, `Namespace` console slots
- [ ] Required tests pass; V1-V9 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-CONTRACTS implementer)` entry covering the work done,
crate versions and pins, each conditional path edited, any design differences, the hash and intent file paths,
evidence per row, and blockers. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Next (wave 2)**: vactrol-session-pkg.md, vactrol-session-directives.md, vactrol-session-analysis.md
