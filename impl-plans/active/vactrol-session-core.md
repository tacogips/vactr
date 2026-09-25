# Vactrol Session Layer: Session, Protocol, Publication, Authority, REPL Loop (SS-SESSION) Implementation Plan

**planId**: SS-SESSION (issue #4, wave 3; the `Session`, eval pipeline, reactive `bindings` publication, JSON codec, write authority, console registers, REPL loop, and the end-to-end TASK-009 tests)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 14.1-14.4, 14.5.4 (pipeline), 14.5.5 (publication), 14.5.6 (codec, authority rules 1-7, coalescing, tiers), 14.5.7 (package failure contract, LSP-only analysis), 14.5.8 (session use of directives), 14.5.9 (`AnalysisCx`), 14.5.10 (REPL), 5.6 (reactive passes), 5.7 (two-phase frontend), 13 (tiers, override inheritance); design-docs/specs/command.md "Session Protocol (v1)"
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: SS-PKG, SS-DIRECTIVES, SS-ANALYSIS
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

This plan builds the `Session` that every front end uses: REPL, CLI `run`/`serve`, the session socket and (for
analysis helpers) the LSP. It runs ALONE in wave 3, after the three wave-2 libraries have joined, so it is also the
first serial join point. Before any edit, verify the wave-2 `final-hashes.txt` files and record the result.

What already exists:
- `Evaluator` (`eval_str`, `eval_form`, `queue_upd`, `run_pass -> PassReport`,
  `set_tweak -> Result<Option<PassReport>, Failure>`, `sites_of`, `form_state`, `vm_and_ns`).
- `sched::Runtime` (`new(hosts, resolver, caps, cfg) -> (Runtime, RuntimeSink)`, `drain`, `tick`, `telemetry`).
- `pkg::*` (SS-PKG), `directives::*` (SS-DIRECTIVES), the analysis surfaces and `Runtime::tap_reader` (SS-ANALYSIS),
  and `session::changes` (SS-CONTRACTS).
- `reader::{read, prescan_imports, AliasEnv}`.

The TASK-005 traces live in `src/ns/tests/reactive_traces.rs` and `reactive_recovery.rs`. Copy their program text;
their harness is private to `ns`.

## Non-Goals

- No socket, no CLI parsing and no process IO (CLI plan). No tower-lsp (LSP plan).
- No file IO. `SessionConfig` receives the lock contents, a `CacheBackend` and the inner `SourceLoader`/`SampleLoader`
  from the caller, so the session stays wasm-safe.
- No edit to `ns/evaluator.rs`. New evaluator entry points go in `ns/eval_doc.rs` as `impl Evaluator`, widening field
  visibility ONLY if strictly needed. If a field is private and inaccessible, record a dependency blocker; do not
  edit `evaluator.rs`.
- No new protocol messages beyond command.md v1.

## writePaths (exclusive in wave 3)

- `src/session/mod.rs` (extends the CONTRACTS seed), `src/session/session.rs`, `src/session/eval.rs`,
  `src/session/publish.rs`, `src/session/authority.rs`, `src/session/protocol.rs`, `src/session/codec.rs`,
  `src/session/console.rs`, `src/session/repl.rs`
- `src/session/tests/mod.rs`, `src/session/tests/support.rs`, `src/session/tests/codec.rs`,
  `src/session/tests/eval.rs`, `src/session/tests/publish.rs`, `src/session/tests/authority.rs`,
  `src/session/tests/tiers.rs`, `src/session/tests/packages.rs`, `src/session/tests/directives.rs`,
  `src/session/tests/repl.rs`, `src/session/tests/analysis.rs`
- `src/ns/eval_doc.rs` (fills the CONTRACTS stub)
- `src/types/manifest.rs` (ONLY a constructor that extends the sound key set with package banks)
- `impl-plans/active/vactrol-session-core.md`

## sharedPaths (serial join repair only; an intent snapshot before every edit)

These are exactly the files listed for SS-SESSION `sharedPaths` in the dispatch manifest: the `src/pkg/` and
`src/pkg/native/` library files, `src/ns/pkg.rs`, the `src/directives/` library files, `src/vm/natives/analysis.rs`,
`src/sched/{offline,tap}.rs` and `src/host/native/tap.rs`. They may be touched ONLY to repair a defect that this
plan's integration tests expose, in the smallest edit, each recorded with the file, the reason and the failing test.

## File-Level Changes (signatures and behavior; no code)

1. **`ns/eval_doc.rs`.**
   - `Evaluator::eval_form_in(&mut self, form: &Node, manifest: &HostManifest, doc_revision: u64) -> FormOutcome`: the
     same as `eval_form`, but it checks against `manifest` and stamps `doc_revision` into every `SrcRef` the form
     creates.
   - `Evaluator::has_queued_upd(&self) -> bool`, if one is not already exposed.
2. **`session/protocol.rs`.**
   - `Envelope<T> { v: u32, seq: u64, re: Option<u64>, kind, body: T }`.
   - `ClientMsg`: `Eval`, `Hush`, `Stop`, `SetVar`, `SetTweak`, `DocChanged`, `Learn`, `Subscribe`, `ManifestReq`.
   - `ServerMsg`: `EvalResult`, `StaleBinding`, `DirectiveEdit`, `Manifest`, `ProtocolError`, `Bindings`, `Diag`,
     `Playing`, `Levels`, `Tempo`.
   - The field names, kinds and shapes are EXACTLY command.md "Session Protocol (v1)".
   - `ServerMsg::routing() -> Route { Requester, Broadcast(Topic) }`, per command.md.
3. **`session/codec.rs` (serde + serde_json).**
   - `decode(text: &str) -> Result<Envelope<ClientMsg>, ProtocolError>`. It returns `bad-json`,
     `unsupported-version` (v != 1), `unknown-kind` or `bad-body`, and never panics.
   - `encode(&Envelope<ServerMsg>) -> String`.
   - Frames over 1 MiB give `bad-body`.
4. **`session/session.rs`.**
   - `SessionConfig { caps, lock: Option<LockFile>, cache: Option<Box<dyn CacheBackend>>, loader: Box<dyn
     SourceLoader>, persistence: PersistenceMode }`.
   - `Session::new(cfg, hosts: Hosts) -> Session`:
     - builds the `Evaluator` + `Runtime` pair as 12.8.3 describes;
     - wraps `cfg.loader` in a `SessionLoader` whose `analysis()` returns `AnalysisCx { caps, taps:
       runtime.tap_reader() }`.
   - `DocState { rev, text, epoch_reconciled, directives, keys: KeyTable, sites: BTreeMap<TweakId, SiteAuth>, defs:
     BTreeMap<SymId, DefAuth> }`, with the file-string → `FileId` table (the console is `FileId::CONSOLE`).
   - `eval(&mut self, src, file: &str, doc_revision, edit_epoch, span: Option<(u32,u32)>) -> (EvalOutcome,
     Vec<ServerMsg>)`.
   - `eval_console(&mut self, line) -> ConsoleOutcome`.
   - `tick(&mut self, host_now) -> Vec<ServerMsg>`.
   - `apply(&mut self, env: Envelope<ClientMsg>) -> Vec<Envelope<ServerMsg>>`.
   - `now(&self) -> f64`.
   - Subscription flags are per requester id: `apply_from(conn: u32, ..)`, used by the socket.
5. **`session/eval.rs` (14.5.4 steps 1-6).**
   - The pipeline:
     1. `prescan_imports`;
     2. load each package through the lock and cache (`CacheBackend::read_verified`) and `PkgNs::load`, then
        `Namespace::import`;
     3. read the whole document with `alias_env_for(session prefixes, prescan)`, then run the ORDER check (a
        qualified atom before its import becomes `unbound-qualifier`);
     4. expand;
     5. `directives::build_table`;
     6. per form in span: `eval_form_in` with the session manifest, which is the spec default plus package banks,
        and banks are registered through `SampleLoader::register_bank`; then `Runtime::drain`; then `run_pass` if an
        upd is queued (14.5.5).
   - A reader or expander error in one form never stops the others.
   - Package failures:
     - `package-not-locked`, `package-not-fetched`, `package-integrity` and `package-load-failed`, each at the import
       span;
     - the prefix stays bound but broken;
     - a bank collision gives `import-collision` (w);
     - re-import replaces the `PkgNs`.
   - `analyze(src, file, pkg_view) -> Analysis { diags, types, alias_env }` performs NO execution; the LSP uses it.
     - A fetched package's qualified names come from a check-only pass over its cached sources.
     - An unfetched package types as `any`, with the warning `package-not-fetched`.
6. **`session/publish.rs` (14.5.5).**
   - `bindings_from(report: &PassReport, ev: &Evaluator, pass: u64) -> Option<ServerMsg>`:
     - `changed` comes from `report.bindings`, with display prints;
     - `sites` holds the refreshed sites of every rebuilt form;
     - `states` holds EVERY scheduled form's final state (`ok`/`failed`+diag+value/`blocked`+`blocked_on`+value).
   - It returns `None` when nothing was scheduled.
   - `PassEvent`s are never forwarded.
7. **`session/authority.rs` (14.5.6 rules 1-7).**
   - `check_tweak(doc, id, form_gen, epoch) -> Result<(), StaleReason>` and
     `check_var(doc, name, defining_form_gen, epoch) -> Result<(), StaleReason>`.
   - `on_doc_changed(doc, base, new_rev, changes: ChangeSet, dirty, epoch)`: maps spans or invalidates everything; it
     also runs `KeyTable::migrate`.
   - `on_eval(doc, rebuilt forms, epoch)` refreshes sites.
   - `PendingWrites` is latest-wins per target. Writes are validated when they arrive and again when applied.
   - At the tick, all set-vars go through `queue_upd` plus ONE `run_pass`, then each set-tweak goes through
     `Evaluator::set_tweak`, and every returned report is published.
8. **`session/console.rs` + `session/repl.rs`.**
   - Console registers use the `Namespace` console slots: `_n` is written only for a completed, non-failing
     expression.
   - `run_repl(session: &mut Session, input: impl BufRead, out: impl Write, clock: &mut dyn FnMut() -> f64)` is the
     testable line loop:
     - continuation on a trailing block colon, and on indented or `>` lines after one; a blank line submits;
     - EOF ends the loop;
     - between lines it ticks via `clock`.
   - The CLI supplies the threads and the real clock.
9. **`learn` handling.**
   - Validate `edit_epoch` (rule 3).
   - Then `directives::writeback::learn_edit`.
   - Directive mode replies `DirectiveEdit`; ExternalFile mode updates the in-memory `BindingSet` and replies nothing.

## Required Tests (`src/session/tests/*.rs`; recording hosts from `host::testing`, `MockClock`)

**codec.rs**
- Every `ClientMsg`/`ServerMsg` round-trips through encode/decode.
- Each `protocol-error` code is produced by a malformed input.
- Truncating valid frames at every byte gives an error, never a panic.

**eval.rs (TASK-009 criterion 1 first half)**
- eval → `eval-result` with static diagnostics, tweak sites (tier, `form_gen`, `doc_revision`) and the directive
  table.
- set-tweak and set-var change state, asserted through the recording audio host.
- `playing` telemetry carries `SrcRef`s with the eval's `doc_revision`.
- `eval-result` precedes a triggered `bindings` batch.

**publish.rs (criterion 1, REACTIVE publication)**
Each trace below uses the TASK-005 program text and a subscriber that records every message:
- An `upd` through a derived binding emits exactly ONE `bindings` batch after ALL rounds, and nothing mid-pass or
  mid-round.
- CHANGING-EDGE: only final-round values appear; the round-1 stale value appears in no batch.
- FAILED-DIAMOND: `right`'s new value, `left`'s failure diagnostic, and `total` `blocked-on: left` with its previous
  value. The recovery pass publishes the unblocked recomputation.
- A subscriber never sees a partial set.
- PROVISIONAL-ROLLBACK (X/Z/Y): X Failed with its restored 7, Z/Y new values. The provisional 1/2 appears in no batch,
  and no staged bind, revocation or cell update of X reaches the recording host.
- ABORT/RETRY (A/B): A Failed with its previous value, B = 0, and no intermediate A.
- CONDITIONAL-UNBLOCKING (switch-away): `selected`'s new value with its badge CLEARED; `broken` still failed.
- SWITCH-TOWARD and STATUS-RECOVERY: blocking publishes `selected`'s badge. The `upd root` 1 -> 0 -> 1 repair
  publishes a recovery batch that clears the badges of `total` and `selected`, although the value is unchanged.
- LATE-FAILURE (trace 3): X `blocked-on: Y` with its restored 7, Y Failed with its retained value, and Z's new value.
  The provisional 2 appears nowhere, and no staged effect of X reaches the host.
- NEWLY-DISCOVERED-SELECTOR: `upd b false` publishes `selected` 7 and clears its badge; `broken` stays failed.
- ORDINARY-FAILURE RECOVERY (a/b division): the failing pass publishes `selected` Failed with its retained 5 and the
  diagnostic. `upd b 1` publishes the repaired 1 with the diagnostic cleared, with no manual re-eval in between.

**authority.rs (criterion 7)**
- A stale `form_gen` `set-tweak` is rejected (`stale-form-gen`).
- A debounce-race write with a newer `edit_epoch` is rejected (`unreconciled-edit`), and accepted after the matching
  `doc-changed`.
- `doc-changed` without re-eval invalidates intersecting sites (a delayed write is rejected) while unrelated sites
  stay valid.
- An insertion before a site, then an edit to the shifted site, invalidates the correct site through composed
  mapping.
- A mismatched base revision invalidates the whole file.
- A superseded `defining_form_gen` `set-var` is rejected.
- Writes within one tick coalesce latest-wins.

**tiers.rs (criterion 7)**
- `reeval` sites rebuild their owning form at the boundary with override inheritance.
- `manual` sites update the slot with no replay.
- `direct` sites, including a probabilistic parameter, follow the 11.3 per-tier cell contract:
  - native model: audible within the commit horizon, through `NativeTransport`;
  - browser model: audible after batch application, through `BrowserTransport`;
  - asserted per tier, never as one claim.

**packages.rs (criterion 2)**
- A fixture package goes into a temp `DirStore`, then `pkg::get_all` into a temp `FsCache` and a lock. The session
  loads from that lock and cache.
- MVS resolves competing requirements, and the lock pins version and sha256.
- Qualified names and `open` names follow 5.7 lookup. A collision gives `import-collision`.
- The sample asset bank `:pads-warm` is registered and plays through `run_repl` (recording host event).
- A FRESH-SESSION single file with `import ... vactrol-pads` and then `pads.warm` loads end to end, and so does the
  `as pd` variant.
- LSP-ONLY `analyze` of an unopened document builds the same `AliasEnv` without executing anything. An unfetched
  package gives `any` plus `package-not-fetched`.
- A hash mismatch, an unresolvable version and a package compile error each yield a load diagnostic while a playing
  slot keeps producing events.
- Re-import replaces the `PkgNs`.
- The proxy-shaped store is covered by SS-PKG's local HTTP fixture; this file cites that test.

**directives.rs (criteria 4-6, the session half)**
- Label-identified bindings survive a line move that re-keys a positional site (via `doc-changed`).
- Unknown site, param and label directives yield warnings and never change evaluation.
- `learn` produces a validated `directive-edit`, and a stale-epoch learn is rejected.
- In ExternalFile mode, `learn` produces no edit and updates the set.

**repl.rs (criteria 8 and 9)**
- A failed expression does not write `_1`. Registers count only completed expressions.
- The session survives every failure class: reader error, check error, runtime failure, dry-run failure, package
  failure, protocol garbage, and panic-free malformed input.
- AUDIBLE-GATE PROXY: a pattern bound from `run_repl` (`s :analog > note [:a4] > d1`) reaches `RecordingAudioHost` as
  committed events after ticks.

**analysis.rs (issue item 4)**
- From `run_repl` with the recording host: `scope :master 16` prints a 16-element list.
- `let out render 1` (native caps), then `rms out`, prints a float.
- Browser caps: `render 1` prints a `beyond-capability` diagnostic.

## Invariants

- One `bindings` batch per completed pass that scheduled a form. Nothing is published mid-pass.
- Session code is wasm-safe (V5): no fs, net, thread or process.
- `Session` never leaves the evaluator thread. `Value` stays `!Send`.
- No `.rs` file reaches 800 lines. Split `session.rs` or `eval.rs` early if needed, using only the listed files.

## Edit Protocol

The common protocol in `vactrol-session-contracts.md`. Evidence goes under
`tmp/ss-session-20260925-s183/SS-SESSION/attempt-<n>/`. Join integrity: before the first edit, run
`shasum -a 256 -c` against the SS-PKG, SS-DIRECTIVES and SS-ANALYSIS `final-hashes.txt`, and explain every mismatch
in `join-integrity.txt`.

## Verification (`<wave>` = `session`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| S1 | LOG(`ss-session-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests/)'` | `exit=0`, run > 0; the progress log lists each criterion-1 trace test name as passing |

## Completion Criteria

- [ ] Items 1-9 implemented
- [ ] Every required test passes: codec, eval, all reactive publication traces, authority, tiers, packages,
      directives, REPL, analysis, audible-gate proxy
- [ ] Join integrity recorded; every serial repair recorded
- [ ] V1-V9 and S1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-SESSION implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-pkg.md, vactrol-session-directives.md, vactrol-session-analysis.md
- **Next (wave 4)**: vactrol-session-cli.md, vactrol-session-lsp.md


### STEP6 OUTPUT NOTE (operator, 2026-09-25, after the SS-ANALYSIS attempt-1 failure)

- The step6-implement output contract requires `changedFiles` to be an ARRAY of
  path strings (SS-ANALYSIS attempt 1 failed with "$.changedFiles must be of type
  array"). Carry `planId`; leave `verificationGaps` empty when every automated
  command passed (manual checks go under `residualRisks`). Crate-wide test
  failures caused only by a sibling branch's in-progress files or by a
  pre-existing test outside every plan's ownership are reported in the
  progress log as a dependency blocker for the operator, never fixed by
  editing unowned files.
