# Vactr Session Layer: Reconciliation and Bookkeeping (SS-FINAL) Implementation Plan

**planId**: SS-FINAL (issue #4 serial reconciliation: join integrity, fixture reclassification, final-tree checks, vactr-core.md TASK-009 bookkeeping, README, commit staging list and message)
**Status**: Completed (session 186: join integrity, fixture reclassification, final-tree checks and TASK-009 bookkeeping done; formal review, commit and push belong to the workflow's later steps)
**Design Reference**: design-docs/specs/design-implementation.md 14.5.12 (FINAL row, verification), 14.5.3 (ownership), 6.5.7 (evidence rule), 7.1.7 (fixture classes); design-docs/user-qa/pending-session-questions.md (S1-S6 carried as residual)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactr/issues/4
**dependsOn**: SS-CLI, SS-LSP
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json (never edited by this plan)

---

## Intent and Context

Every implementation wave has joined. This plan runs alone and is the last serial repair point. It:
- reclassifies the one spec fixture block still deferred to TASK-009 (lang-reference.md ordinal 5, "State and
  modules": `import`, `pads.warm`, `load ./soundpack/..`);
- runs every issue #4 check on the whole tree, including `--features lsp` and the LSP smoke test;
- does the cross-plan bookkeeping: vactr-core.md TASK-009 checkboxes with evidence and status COMPLETED, with the
  audible gate "pending user confirmation" and its automated proxy; SS plan statuses; the README; the commit staging
  list; and the six-section commit message for the workflow's step 9.

## Non-Goals

- No new features. A defect is repaired serially in the smallest edit, with an intent snapshot, and recorded.
- Never edit the dispatch manifest. Never commit (the workflow commit step does, or the operator does if the
  git-commit node rejects the multi-line message).
- Never move plan files to `completed/` during the run; archiving is a separate docs commit after the workflow
  commit, as in issues #2 and #3.
- The audible REPL gate stays "pending user confirmation".

## writePaths (exclusive in wave 5)

- `tests/fixtures/spec/manifest.toml` (lang-reference ordinal 5, plus any multiset repin the joined tree needs)
- `tests/support/eval.rs` (a `Session`-based evaluation path for blocks that need package loading), `tests/spec_fixtures.rs`
  (ONLY if the runner must dispatch that path)
- `impl-plans/active/vactr-core.md`, `impl-plans/README.md`, `impl-plans/active/vactr-session-finalize.md`

## sharedPaths (serial; an intent snapshot before every edit)

- Status lines and a closing note only: `impl-plans/active/vactr-session-{contracts,pkg,directives,analysis,core,cli,lsp}.md`
- Serial repair and `rustfmt` only, when a check on the joined tree fails in that file: every file in any SS plan's
  writePaths, as enumerated for SS-FINAL in the dispatch manifest `sharedPaths`. Each repaired file and its reason is
  recorded.

## File-Level Changes

1. **Join integrity.** Before any edit, run `shasum -a 256 -c` against every SS plan's `final-hashes.txt`. Explain each
   mismatch (a later plan's legitimate edit, or drift) in
   `tmp/ss-session-20260925-s183/SS-FINAL/attempt-<n>/join-integrity.txt`.
2. **`tests/support/eval.rs` + `tests/fixtures/spec/manifest.toml`.**
   - Evaluate lang-reference ordinal 5 through `vactr::session::Session` with `NoopHost` and no lock.
   - Move it from `deferred` (`deferred_to = "TASK-009"`) to `diagnostic`, with exact `check_diags`/`run_fails`
     multisets (7.1.7). Expected: `package-not-locked` at the `import` line, `pads.warm` undefined, `load ./soundpack/..`
     `host-unavailable`, and the existing pins.
   - Its `note` says why.
   - `grep -c 'deferred_to = "TASK-009"'` must print 0.
3. **`impl-plans/active/vactr-core.md`, TASK-009.** Each checkbox is checked ONLY with cited evidence: the plan, the
   test file and test names, the log path and its `exit=`.
   - Criterion 1: `src/session/tests/{eval,publish}.rs`.
   - Criterion 2: `src/session/tests/packages.rs` + `src/pkg/tests/proxy.rs`.
   - Criterion 3: `src/pkg/tests/{validate,zip,digest,cache}.rs`.
   - Criteria 4-5: `src/directives/tests/*` + `tests/directive_fixtures.rs` + `src/session/tests/directives.rs`.
   - Criterion 6: `src/directives/tests/{key,persist}.rs`.
   - Criterion 7: `src/session/tests/{authority,tiers}.rs`.
   - Criterion 8, the audible gate: "PENDING USER CONFIRMATION (manual: `vactr repl` on a real output device, then
     `s :analog > note [:a4] > d1`). Automated proxy: `src/session/tests/repl.rs` audible-gate proxy". Per issue #4
     this does not block acceptance.
   - Criterion 9: `src/session/tests/repl.rs` + `tests/cli.rs`.
   - Criterion 10: `tests/lsp_smoke.rs`, log `ss-final-lsp-smoke`.
   - Criterion 11: the final build, build-lsp and nextest logs.

   Also update this file:
   - TASK-009 status: COMPLETED, only when every box except criterion 8 is checked.
   - Amend the deliverable text with the accepted divergences of design 14.5.1: offline `render` through
     `NativeAudioHost::headless` instead of `Engine::render`; the browser store's `fetch()`/OPFS backend in TASK-010;
     the session socket in `src/cli/ws.rs`.
   - Update the Module 8 status line and the Module Status table, and add a progress-log entry.
4. **`impl-plans/README.md`.** Update the vactr-core.md row and the eight SS plan rows (Completed) and the manifest
   row.
5. **Plan statuses.** Set the SS plan status lines to Completed, with a closing note citing the final logs.
6. **Commit staging list and message**, recorded in this plan's progress log for step 9.
   - Explicit paths only: `Cargo.toml`, `Cargo.lock`, `src/`, `tests/`, `design-docs/`, `impl-plans/`.
   - The message uses the six CLAUDE.md sections on separate lines, with no AI attribution and no Co-Authored-By line.
   - The operator commits and pushes if the git-commit node rejects the multi-line message.

## Invariants

- A checkbox is checked only with evidence. An unmet criterion is reported, never checked.
- Crate-wide `cargo fmt --check` passes. If it does not, `cargo fmt` is run once and every touched file is listed.
- No `.rs` file reaches 800 lines. No gated crate reaches wasm32.

## Edit Protocol

The common protocol in `vactr-session-contracts.md`, except rule 5: this plan may run crate-wide `cargo fmt` once,
recorded. Evidence goes under `tmp/ss-session-20260925-s183/SS-FINAL/attempt-<n>/`.

## Verification (the issue's final-tree contract; `<wave>` = `final`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, all on the final tree after the last
repair, plus:

| # | Command | Evidence |
|---|---------|----------|
| F1 | LOG(`ss-final-lsp-smoke`): `CARGO_TERM_QUIET=true cargo test --features lsp --test lsp_smoke` | `exit=0` |
| F2 | LOG(`ss-final-cli`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(cli)'` | `exit=0`, run > 0 |
| F3 | LOG(`ss-final-session`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests/) \| test(/pkg::tests/) \| test(/directives::tests/) \| binary(directive_fixtures)'` | `exit=0`, run > 0 |
| F4 | `grep -c 'deferred_to = "TASK-009"' tests/fixtures/spec/manifest.toml \|\| true` | prints `0` |
| F5 | `CARGO_TERM_QUIET=true cargo build --example beep` (LOG `ss-final-example`) | `exit=0` |
| F6 | `jq . impl-plans/active/ss-session-20260925-s183-dispatch.json > /dev/null` | exit 0 (read-only) |
| F7 | `git status --short` and `git diff --stat` | recorded |

## Completion Criteria

- [x] Join integrity checked and explained
- [x] Lang-reference ordinal 5 reclassified (F4 = 0); the fixtures are green
- [x] V1-V9 and F1-F7 pass on the final tree with logs cited
- [x] vactr-core.md TASK-009 bookkeeping done with evidence (criterion 8 pending user confirmation with its proxy);
      status COMPLETED; the README and SS plan statuses updated
- [x] Commit staging list and six-section message (no attribution lines) recorded; the dispatch manifest is not edited

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-FINAL)` entry. Edit only this log, plus the status lines
and closing notes allowed above.)

### Session: 2026-09-26 (session 186, SS-FINAL)

**Tasks completed**: every File-Level Change 1-6. Evidence: `tmp/ss-session-20260925-s183/SS-FINAL/attempt-1/`
(`intent.md`, `pre-edit-hashes.txt`, `post-edit-hashes-source.txt`, `join-integrity.txt`, `join-integrity-raw.txt`,
`v4-linecounts.txt`, `v5-grep.txt`, `f4-grep.txt`, `f6-jq.txt`, `git-status.txt`, `final-hashes.txt`).

1. **Join integrity**: clean for SS-ANALYSIS/2, SS-CLI/1, SS-LSP/1, SS-PKG/2, SS-SESSION/1. Every mismatch is
   explained in `join-integrity.txt`: the 12 SS-CONTRACTS source seeds were filled by their owning later wave (the
   contracts hash is in each owner's `pre-edit-hashes.txt`); the plan-md mismatches are the checkpoint commit, a
   superseding attempt-2, or the operator's 2026-09-26 note. No drift, no repair.
2. **Fixture**: `tests/support/eval.rs` gains `session_eval` (a `Session` over `Hosts::noop()`, native caps, no lock,
   no cache; the block is one document). `tests/spec_fixtures.rs` dispatches blocks with `eval_via = "session"` to it,
   and `evaluation_report` prints the session result. lang-reference #5 is now `diagnostic`:
   check_diags `package-not-locked@55/56/57`, `read-error-present@99`, `stray-char@100`, `undefined-name@58/81`,
   `unknown-keyword@23`, `upd-immutable@33`; run_fails `arity@14` x2, `host-unavailable@80`, `undefined-name@58/81`,
   `unknown-sound@23`, `upd-immutable@33`. F4 prints 0. Finding for the language author (not changed here): the
   runtime calls a fn-valued pattern parameter with the event time, so the spec's zero-parameter `fn kick-sound:`
   faults `arity`.
3. **vactr-core.md**: TASK-009 is COMPLETED. Criteria 1-7 and 9-11 are checked with the test names and logs.
   Criterion 8 stays unchecked: PENDING USER CONFIRMATION, with the proxy `audible_gate_proxy_a_repl_bound_pattern_reaches_the_audio_host`.
   The deliverable text is amended with the 14.5.1 divergences. The Module 8 status, the Module Status and Dependencies
   rows, the project criterion on the README and command.md, a progress-log entry and the session sub-plans in
   Related Plans are updated.
4. **README**: the vactr-core.md row, the eight SS rows (Completed) and the manifest row are updated.
5. **Plan statuses**: SS-CONTRACTS..SS-LSP are Completed, each with a closing note. This plan is Completed.
6. **Commit**: the staging list and message are below. The dispatch manifest is not edited:
   `git diff --quiet HEAD -- impl-plans/active/ss-session-20260925-s183-dispatch.json` exit=0, and F6 `jq` exit=0.

**Verification** (the final tree after the last source edit, `target/fe-logs/ss-final-<check>-s186-1.log`, each ending in
`exit=0`):
- V1 build; V1l build-lsp; V2 clippy; V2l clippy-lsp; V7 fmt (crate-wide `--check` passed, so no `cargo fmt` rewrite).
- V3 nextest: 984 run, 984 passed, 1 skipped.
- V3t cargotest: lib 963, cli 9, directive_fixtures 2, spec_fixtures 10 passed (1 ignored report), 0 failed.
- V3f fixtures: 10/10.
- F1 lsp-smoke: 1 passed. The lsp unit tests (`lsp-own`, `--features lsp`) pass 9/9.
- F2 cli: 9/9.
- F3 session: 132/132.
- F5 example.
- V6a wasm32; V6b wasm32-hostwasm.
- V9: `tree-wasm32` and `tree-wasm32-hostwasm` have 0 lines matching tungstenite, getrandom, tokio, tower-lsp, cpal
  or midir.
- V4: the largest file is `src/dsp/build.rs` at 799 lines.
- V5: none.
- F4: 0.
- F6: exit 0.
- F7: `git-status.txt`: 63 modified, 22 untracked entries (102 files), all under `src/` and `tests/`. `git diff --stat`:
  63 files changed, 2532 insertions, 290 deletions (before this log entry).

**Serial repairs**: none needed in this branch. No file outside this plan's writePaths was edited except the SS plan
status lines and closing notes that sharedPaths allows.

**Serial repair after integration review (session 185, IR-S185-W5-BUSNAMES)**: the integration review found that no
production code called `NativeAudioHost::set_bus_names`, so native named-bus taps (`scope`/`spectrum`/`capture :bus`)
failed with host-unavailable. The reconciler wired the session's instrument registry into the native host before
boxing, with no other change:
- `src/session/session.rs`: `SessionConfig::insts` (the shared registry; `None` builds a fresh one).
- `src/host/native/mod.rs`: `NativeHosts::open_with_bus_names`. `open` is unchanged and delegates.
- `src/cli/mod.rs`: `build_session_native` shares one registry between the host and the session.
- `src/session/tests/analysis.rs`: `a_named_bus_scope_resolves_through_the_native_host_sharing_the_session_registry`.
  The test has a positive case and a host-unavailable negative control. It fails if the sharing is removed.

The final tree was re-verified with logs at `target/fe-logs/ss-reconcile-<check>-s185-r1.log`, each ending in exit=0:
nextest 985/985, nextest --features lsp 994/994, cargo test with lib 964, cli 9, directive_fixtures 2 and spec_fixtures 10.
Evidence is in `tmp/ss-session-20260925-s183/reconcile-s185-repair1/`.

**Commit staging list** (explicit paths; `tmp/` and `target/` are gitignored):
`git add Cargo.toml Cargo.lock src/ tests/ design-docs/ impl-plans/`

**Commit message** (six sections on separate lines, no attribution lines):

```
feat: implement the session layer - Session, protocol, packages, directives, REPL, CLI, LSP (TASK-009, #4)

1. Primary Changes and Intent:
   Implements TASK-009 of impl-plans/active/vactr-core.md (issue #4): a Session wrapping Evaluator + Runtime with the
   per-form eval pipeline and one bindings batch per completed pass, the versioned ClientMsg/ServerMsg JSON protocol
   with doc_revision/edit_epoch/form_gen authority, end-to-end package loading, the #@ directive machinery, the
   self-analysis prelude surfaces, and the vactr CLI (repl, run, serve, get, lsp) with a loopback session socket and
   a tower-lsp language server behind the lsp feature.

2. Key Technical Concepts:
   - Reactive publication: bindings batches are built from the returned PassReport, never mid-pass (14.5.5)
   - Write authority: reconciled edit epoch per document, change-set coordinate mapping, stale-binding rejection (14.5.6)
   - Packages: minimal version selection, vactr.lock, injective length-prefixed canonical digest, archive and path
     validation before the digest, staged atomic cache publication; a running session never fetches (14.5.7)
   - Directives: DirectiveTable, LabelRegistry, BindingKey, Directive and ExternalFile persistence (13.5, 14.5.8)
   - Self-analysis: AnalysisCx on the SourceLoader hook; scope/spectrum/capture over native taps, offline render
     through NativeAudioHost::headless, rms/peak over sample values (12.3, 14.5.9)
   - Feature gating: tungstenite/getrandom behind host-native, tower-lsp/tokio behind lsp; both wasm32 builds stay clean

3. Files and Code Sections:
   - src/session/: Session, eval pipeline, publish, authority, protocol, codec, console, repl
   - src/pkg/: semver, manifest, lock, mvs, sha256, digest, validate, zip, store, proxy, cache, load; native dir/git
     stores and fs cache
   - src/directives/: parse, attach, labels, resolve, key, persist, writeback
   - src/cli/: args, repl, run, serve, get, ws (loopback socket, 32-byte token, 401, 8 connections, 1 MiB frames)
   - src/lsp/: stdio server, single-thread analysis, UTF-16 conversion
   - src/main.rs: calls vactr::cli::run on native targets
   - src/value/sample.rs, src/vm/natives/analysis.rs, src/sched/{tap,offline}.rs, src/dsp/offline.rs,
     src/host/native/tap.rs: self-analysis surfaces
   - src/ns, src/compile (compiler.rs split into names.rs), src/types, src/vm, src/sched, src/host, src/dsp: contract
     seams (StagedEffect, Sound::Buffer, diagnostic codes, console registers, tap reader)
   - tests/cli.rs, tests/lsp_smoke.rs, tests/directive_fixtures.rs, tests/fixtures/directives/vocabulary.toml
   - tests/support/eval.rs, tests/spec_fixtures.rs, tests/fixtures/spec/manifest.toml: lang-reference #5 evaluated
     through Session (deferred -> diagnostic)
   - Cargo.toml, Cargo.lock: serde, serde_json, miniz_oxide; tungstenite and getrandom (host-native); tower-lsp and
     tokio (lsp)
   - impl-plans/: vactr-core.md TASK-009 completed with evidence; eight SS wave plans; README

4. Problem Solving:
   src/main.rs only printed the version, and there was no session, protocol, package loading, directive machinery,
   REPL, LSP or CLI. Malicious but correctly hashed packages, digest forgery, debounce-race writes, provisional
   rollback leaks and stale bindings are now rejected by construction and asserted by tests. The native audio host
   shares the session's instrument registry (SessionConfig::insts, NativeHosts::open_with_bus_names), so named-bus
   taps resolve on the native host.

5. Impact:
   Vactr is usable as a live-coding tool from the terminal (repl, run, serve, get) and from editors through the
   LSP. The protocol is frozen for TASK-010 (editor). Final tree: build, build --features lsp, clippy -D warnings
   (default and lsp), fmt --check, nextest 985/985, cargo test, the lsp smoke test and both wasm32 builds pass.

6. Unresolved TODOs:
   - [ ] impl-plans/active/vactr-core.md TASK-009 criterion 8: audible REPL gate pending user confirmation
         (`vactr repl`, then `s :analog > note [:a4] > d1`)
   - [ ] impl-plans/active/vactr-core.md TASK-008: `cargo run --example beep` audible check pending user confirmation
   - [ ] design-docs/user-qa/pending-session-questions.md: S1-S6 await author decisions
   - [ ] design-docs/specs/lang-reference.md block 5 line 12: a zero-parameter fn passed as a pattern parameter faults
         arity (the runtime passes the event time); spec or runtime to be aligned by the author
   - [ ] impl-plans/active/: archive the SS plans and the manifest to completed/ in a separate docs commit
```

If the git-commit node rejects the multi-line message, the operator commits and pushes with it (issue #4, item 6).

## Related Plans

- **Parent**: impl-plans/active/vactr-core.md (TASK-009)
- **Previous**: vactr-session-cli.md, vactr-session-lsp.md


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
