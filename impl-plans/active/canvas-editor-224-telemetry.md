# CE-TELEMETRY: Canvas editor telemetry implementation plan

**planId**: CE-TELEMETRY
**planPath**: impl-plans/active/canvas-editor-224-telemetry.md
**Status**: In Progress — restart implemented; required Clippy gate blocked by unapproved paths
**Created / Last Updated**: 2026-09-30
**Design Reference**: design-docs/specs/design-implementation.md#153-gpu-canvas-code-editor-and-synchronized-composition-2026-09-30
**Issue**: codex-design-and-implement-review-loop-session-224
**workflowMode**: issue-resolution
**dependsOn**: CE-CONTRACT

## Intent and repository context
Produce revision-aware scheduled event end times and periodic timestamped transport snapshots on the real session ABI.

Baseline: existing browser Rust/Wasm ABI, CodeMirror state/history and native CPAL host; accepted design15.3 supersedes visible DOM source and receipt-anchored synchronization.
Read [execution contract](canvas-editor-224-execution.md) before editing; its ownership, snapshots, Git/review, Rust agents, logs and progress rules are mandatory.

## Non-goals
No DSP scheduling changes, native shell or frontend clock algorithm. The only scheduler change is observing successful transport restarts.

## Related plans
Previous/dependencies: CE-CONTRACT. Next: CE-AUDIO, CE-CLOCK.

## Write paths
- src/sched/midi_clock.rs
- src/session/protocol.rs
- src/session/codec.rs
- src/session/publish.rs
- src/session/session.rs
- src/host/wasm/session_half.rs
- src/session/tests/publish.rs
- src/session/tests/codec.rs
- editor/test/wasm/canvas-clock.test.ts
- impl-plans/active/canvas-editor-224-telemetry.md

## Shared paths and intended edits
None.

## File-level tasks and module status
| Task | Deliverables | Status |
|---|---|---|
| T1 | session/protocol.rs and codec.rs | Implemented; behavioral verification pending |
| T2 | session/publish.rs and session.rs | Implemented; final behavioral verification blocked |
| T3 | wasm/session_half.rs and tests | Implemented; behavioral verification blocked |
| T4 | midi_clock.rs generation, publisher consumption and restart regressions | Implemented; final behavioral verification blocked |

### T1: session/protocol.rs and codec.rs
**Status**: Implemented; verification pending
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Mirror CE-CONTRACT optional fields with defaults and old-client compatibility. Timing emission bounded20Hz independent of tempo changes, file/revision/span unchanged. Preserve dirty MusicDSP protocol hunks; one owner fresh-reads each edit. Validate epoch/sample/end/latency fields without rejecting old wire envelopes.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### T2: session/publish.rs and session.rs
**Status**: Implemented; final behavioral verification blocked by aggregate Clippy ownership
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
playing_wire end_time = scheduled event time plus original e.dur seconds (not recomputed from later BPM). Snapshot uses runtime cycle position and matching host_now; add publisher cadence and epoch state, pause/lost-clock handling and discontinuity invalidation. Avoid allocations/serialization in audio callback; publication on session control side. Maintain subscription routing and existing tempo behavior for old clients.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

### T3: wasm/session_half.rs and tests
**Status**: Implemented; verification blocked
**Parallelizable**: No within plan; execute tasks in listed order. Independent plans may run only after DAG predecessors finish.
Thread browser host sample time into same session timing semantics while preserving raw ABI and TAG_SESSION. RealWasm test init/eval/tick and read actual telemetry: no tempo-change needed for refreshed snapshots, duration unchanged by subsequent BPM, revision source retained, epoch reset and rate ceiling verified. Rust tests cover malformed/legacy records and subscription/cadence.

**Completion criteria**:
- [ ] File-level behavior above implemented with required invariants.
- [ ] Targeted tests prove behavior and complete logs show final exits.

## Invariants
- Single editing authority; visible source/feedbackGPU, DOM input/accessibility only.
- Source authority, revision/epoch, UTF conversion and flush-before-write preserved.
- Audio never waits for UI; processing/display clocks remain separate.
- Preserve user dirty hunks, latest predecessor contract and resource/queue ceilings.
- Device and measurement evidence remains incomplete unless actually captured.

## Verification commands and required evidence
- `CARGO_TERM_QUIET=true cargo check`
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run session::tests::`
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
- `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm npm run test -- test/wasm/canvas-clock.test.ts`

These commands establish compile/type safety and the task behaviors listed above. Tests must assert concrete outputs, boundaries, revisions, lifetime or timing rather than mirror implementation. Real browser runner is planned code, not an existing passing command. iOS/sign/device commands are conditional on actual tooling; log missing prerequisites as unavailable.
Record complete logs/exit statuses per execution contract. No test/build is claimed executed by this plan author.

## Completion criteria
- [ ] Real native-session and Wasm telemetry share contracts
- [ ] Periodic timing/end-time/epoch/legacy tests pass
- [x] Own progress log includes command evidence, fixes and remaining gates.
- [ ] Independent review and improve self-review findings resolved.

## Progress Log
### Session: 2026-09-30 — planning
Tasks completed: code-free plan authored from accepted design.
Implementation: not started. Build/behavior/device criteria unchecked.
Next: independent Step5 plan review, accepted-plan commit/push, then dependency-ready execution.

### Session: 2026-09-30 — CE-TELEMETRY Step6 attempt-1
Dependency admission: runtime acceptedPlanIds includes CE-CONTRACT; committed plan matches design15.3.4. Skills: impl-plan, design-doc reference, Rust coding standards, improve. Required agent references: /root/rust_coding, /root/check_and_test_after_modify; TS owner /root/wasm_tests; bounded author review and one-line Rust repair /root/telemetry_selfcheck.

Implemented optional protocol-v1 playing epoch/end_time and transport fields, finite/latency/rational/source validation and legacy decoding. Publisher uses original scheduled duration, matching host sample/runtime position, independent <=20Hz cadence, legacy tempo change notices, explicit unavailable latency, observed epoch discontinuities and held paused/lost position. Invalid native/Wasm host samples are ignored. Six Rust behavioral tests and five real-Wasm tests added; actual ABI tempo/playing records pass through the accepted TypeScript decoder when executed. No behavioral pass is claimed.

Immutable intentions: tmp/canvas-editor-224/CE-TELEMETRY/attempt-1/edit-rust-001 through edit-rust-017, ts-edit-001 through ts-edit-002, edit-plan-001 through edit-plan-004; mirrored under /private/tmp/vactr-224-implementation/CE-TELEMETRY/. Read/prehash guards and post-edit captures preserve prior MusicDSP protocol unit comment. No Git mutations or unrelated edits.

Command evidence (complete foreground logs, final terminal exits):
- RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=tmp/canvas-editor-224/CE-TELEMETRY/attempt-1/target CARGO_TERM_QUIET=true cargo check --locked: exit0, cargo-check.log/.json. Stable production source; later edit-rust-017 changes only test fixture.
- RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=tmp/canvas-editor-224/CE-TELEMETRY/attempt-1/target CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --locked `session::tests::`: exit101, nextest.log/.json; compilation stopped before tests, counts unavailable. Missing SrcRef.doc_revision fixture fixed to42 in edit-rust-017; renewed gate remains unrun, not a pass.
- rustup target add wasm32-unknown-unknown --toolchain 1.98.1: exit0, wasm-target.log/.json. Toolchain override accommodates existing modern Rust source; isolated Cargo output avoids shared build mutation.
All command logs above reside under tmp/canvas-editor-224/CE-TELEMETRY/attempt-1/.

Author improve finding (mid, unresolved): sampled position/source/frozen/lost/time cannot identify every MIDI Start. Start at prior sampled position0 or between sparse ticks can reset Runtime while leaving publisher observations nondecreasing; restarted events then retain old epoch. Runtime::transport_start in src/sched/midi_clock.rs exposes no generation. A robust monotonic restart generation and observable accessor require src/sched/midi_clock.rs (possibly src/sched/runtime.rs), outside this plan's approved writePaths. No anchor heuristic or downstream host wiring substituted for the missing seam.

Resume criterion: serial ownership amendment authorizes this exact runtime seam, or its owning plan delivers an accepted generation accessor. Then consume generation in publisher and add zero-position/sparse-tick restart regressions, rerun nextest, isolated Wasm build, actual-Wasm tests, editor npm run check and exact-file format check. All these gates are incomplete; no narrower verification attempted after discovering ownership gap. All command sessions exited.

Implementation is incomplete and blocked. Formal integrity/adversarial review, accepted completion, shared indexes/archive, commit/push remain later workflow steps; their pending state is not the blocker. Native output-correlation and frontend clock algorithms remain CE-AUDIO/CE-CLOCK allocation. No hardware/device evidence claimed.

Snapshot author audit: attempt-1/snapshot-audit.json confirms all final Rust edit snapshots match current files and no semantic drift. Four within-node formatting transitions lack their own pre-edit intentions; adjacent immutable snapshots are rustfmt-equivalent. This is an evidence-protocol limitation, not proof of a complete per-edit history. Retain it for serial review; future edits must capture formatter intentions before writing. Handoff source identities and pre-node owned diff are recorded in final-handoff-source.json/final-handoff.diff.

## Session 227 restart generation contract (integration finding 2)
This amendment allocates `src/sched/midi_clock.rs` exclusively to CE-TELEMETRY.
`MidiClockState` and its `impl Runtime` already live there; no `src/sched/runtime.rs`
edit is necessary or authorized. No scheduler redesign or DSP timing change is intended.

### T4: Observe every successful MIDI restart, then consume its generation
**Status**: Implemented; final behavioral verification blocked by aggregate Clippy ownership
**Parallelizable**: No; implement before retrying T2/T3 verification.
- `src/sched/midi_clock.rs`: add a runtime-owned restart-generation counter and
  read-only `MidiClockState::restart_generation` accessor. Advance exactly once after
  each successful `transport_start`, including Start at zero and repeated Starts before
  any publisher sample. Rejected Start, normal pulses, ticks and Continue do not advance
  this counter. Enter/leave slave and stop/resume must not reset or fabricate it.
- `src/session/session.rs`: retain the last observed restart generation in publisher
  state, initialized without falsely reporting a first observation as a restart.
- `src/session/publish.rs`: compare generation before assigning epoch to playing
  events or snapshots; generation change invalidates the prior epoch even when sampled
  position/time/source/paused/lost appear unchanged. Preserve existing discontinuity
  detection and <=20 Hz snapshot cadence. A restart inside the cadence window must
  still give immediately published playing events the new epoch. Later snapshot and
  playing epochs must agree; no frontend generation field or v1 ABI change is needed.
- `src/session/tests/publish.rs`: use the real Rig/runtime MIDI transport, not injected
  epoch strings. Add regressions for Start at prior sampled position zero, restart
  between samples with nondecreasing sampled position, multiple Starts before next
  observation, restart inside 50 ms cadence with a playing event, and rejected Start/
  Continue counter neutrality. Assert epoch change, epoch consistency and unchanged
  source revision/end-time semantics. Retain existing pause/lost/rollback tests.
- `editor/test/wasm/canvas-clock.test.ts`: exercise the existing real Wasm MIDI input
  ABI for zero-position and between-sample Starts, decode original TAG_SESSION bytes
  through the accepted decoder, and assert new epoch on playing plus next snapshot.
  Preserve init/eval/tick, cadence, original-duration and source-revision assertions;
  loading a mock artifact or skipping tests cannot pass this gate.

**Acceptance and evidence**:
- [x] Generation changes on every successful restart, independent of sampled position (source inspection; behavioral gate pending).
- [x] Publisher consumes generation before any post-restart playing publication (source inspection; behavioral gate pending).
- [ ] Rust and real Wasm restart regressions fail against the former heuristic and pass
      against the generation implementation; record actual counts and complete exits.
- [ ] Required Rust agents run; native progress gate and formal workflow reviews accept.

## Retry verification commands (supersede the original commands for this attempt)
Use existing installed toolchain 1.98.1 from attempt-1; do not modify mise.toml.
Run serially from repository root except the explicit editor command. Isolated target
output below is a build directory, not a worktree or private branch.
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=tmp/canvas-editor-224/CE-TELEMETRY/attempt-2/target CARGO_TERM_QUIET=true cargo check --locked`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=tmp/canvas-editor-224/CE-TELEMETRY/attempt-2/target CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=tmp/canvas-editor-224/CE-TELEMETRY/attempt-2/target CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --locked --no-fail-fast session::tests::`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=tmp/canvas-editor-224/CE-TELEMETRY/attempt-2/target CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --locked --no-fail-fast sched::tests::`
- `RUSTUP_TOOLCHAIN=1.98.1 CARGO_TARGET_DIR=tmp/canvas-editor-224/CE-TELEMETRY/attempt-2/target CARGO_TERM_QUIET=true cargo build --locked --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
- `cd editor && VACTR_WASM=../tmp/canvas-editor-224/CE-TELEMETRY/attempt-2/target/wasm32-unknown-unknown/debug/deps/vactr.wasm npm run test -- test/wasm/canvas-clock.test.ts`
- `cd editor && npm run check`
- `RUSTUP_TOOLCHAIN=1.98.1 rustfmt --check --edition 2021 src/sched/midi_clock.rs src/session/protocol.rs src/session/codec.rs src/session/publish.rs src/session/session.rs src/host/wasm/session_half.rs src/session/tests/publish.rs src/session/tests/codec.rs`
Compile/clippy/format prove the actual modified bytes build cleanly; nextest proves real
session and scheduler restart compatibility; Wasm build plus tests prove the production
ABI/decoder pairing. Run post-modification verification agent after every Rust change.
Preserve attempt-1 nextest exit 101 and snapshot-history limitations; new logs must not
replace them. Missing toolchain/target or real-artifact failure remains an actual check
failure, with complete output; it is never a workflow provenance blocker.

### Session: 2026-09-30 — Step4 session 227 amendment
Generation seam now allocated in manifest and plan, within accepted design 15.3.4.
No Rust modified here. Native implementation result remains blocked pending T4, renewed
Rust/Wasm/type/format checks, progress admission and formal workflow reviews. Historical
attempt-1 exit 101 and incomplete format intention history retained without reclassification.

### Session: 2026-09-30 18:19 JST — CE-TELEMETRY Step6 attempt-2
Admission: runtime acceptedPlanIds contains CE-CONTRACT. Allocated midi_clock.rs seam
matches accepted design15.3.4; runtime.rs and shared manifest were not edited.
Required agents: /root/rust_coding and /root/check_and_test_after_modify.
Read-only independent inspection: /root/telemetry_review; improve author self-check applied.

Implemented runtime restart generation and initial session observation. Successful Start
increments once; rejected Start, pulses/ticks, Stop/Continue and slave transitions preserve
the counter. Publisher observes it before assigning playing/snapshot epochs, preserving
the existing discontinuity checks and 20Hz cadence. Added two Rust and two real-Wasm
restart regressions plus one Rust counter-neutrality test. Source revisions and original
scheduled end times remain asserted. Two owned Clippy cadence predicates were changed
to equivalent is_none_or calls. No unrelated source or Git mutations.

Fail-before evidence (old production heuristic, complete foreground commands):
- Rust restart filter: exit100, testsRun2/testsPassed0/failureCount2.
- Real host-Wasm build: exit0.
- Real-Wasm canvas-clock suite: exit1, testsRun7/testsPassed5/failureCount2.
Both suites failed only the expected unchanged-epoch assertions. Exact commands, full
logs, terminal metadata and source hashes: attempt-2/fail-before/{rust-restarts,
wasm-build,wasm-restarts}.{log,json} under tmp/canvas-editor-224/CE-TELEMETRY/.
These prove regression sensitivity, not final-source passes.

Final-source required gates: attempt-2/final-retry/summary.json records exact commands.
- cargo check --locked with RUSTUP_TOOLCHAIN=1.98.1, isolated attempt-2/target and
  CARGO_TERM_QUIET=true: exit0; final-retry/check.log and check.json.
- cargo clippy --locked --all-targets -- -D warnings with the same environment:
  exit101; final-retry/clippy.log and clippy.json. Two publish.rs warnings resolved;
  remaining 16 library / 27 library-test diagnostics are outside approved writePaths.
- Required session nextest, scheduler nextest, final Wasm build, final real-Wasm
  test, editor type check and exact-file format check: blocked-not-run at the safe
  ownership boundary. No post-fix behavioral pass is claimed.
Earlier check0/clippy101 logs remain in attempt-2/final/. Attempt-1 nextest exit101,
missing fixture repair and format-intention limitations are retained without erasure.

Blocker: canonical Clippy requires changes outside this plan's ownership. Exact diagnostic
locations and resume criterion: attempt-2/clippy-ownership-blocker.json. Examples include
src/cli/ws.rs, src/directives/writeback.rs, src/dsp/graph/shape.rs, src/dsp/release.rs,
src/host/native/audio.rs, src/reader/line.rs and src/sched/commit/timestamps.rs.
No source-matched baseline disposition was proven or claimed. Resume when the serial
coordinator allocates exact repair paths or authorized owners resolve the diagnostics,
then rerun all eight required gates on the preserved final source.

Immutable intentions: attempt-2/edit-rust-001..010, edit-ts-001 and edit-plan-001..002;
mirrors /private/tmp/vactr-224-implementation/CE-TELEMETRY/attempt-2/.
pre-node-comparison.json confirms prior protocol/codec/Wasm-host changes remain intact;
source changes add the intended restart seam/tests. independent-inspection.json records
no additional material code finding before the equivalent Clippy predicate fix.
Author self-check retains the unresolved mid verification/ownership blocker and six
blocked gates. All foreground sessions exited. Implementation remains incomplete/blocked.
Native progress admission, formal integrity/adversarial/integration reviews, accepted
completion record, shared indexes/archive and Git finalization remain downstream; their
pending status is separate from this implementation blocker. CE-STATE and CE-PACKAGE
findings belong to their owners. No physical iPad or hardware evidence is claimed.

## Session 235 bounded verification disposition and T5 native fixture repair
**Status**: Proposal for Step5 independent review; no lint waiver or native acceptance granted.
**Design trace**: 15.3.4 epoch/source identity, 15.3.7 behavioral and warnings-denied verification. Architecture unchanged; the scoped lint disposition below is an explicit verification exception proposal requiring independent review, not an invented passing global gate.
**Parallelizable**: Yes with CE-GPU/CE-PACKAGE after checkpoint, serial tasks inside this plan.

### T5: Repair source-bearing native restart fixtures
Fresh-read `src/session/tests/publish.rs` and real-Wasm fixtures. Operator `tmp/canvas-editor-224/root-observations/native-restart-fixture-diagnosis.json` and `telemetry-post-fix-verification/nextest-session.log` show both new restart tests fail unwrapping event.src at lines499/526. Scalar `s :analog` lacks pattern provenance; use actual source-bearing pattern fixtures compatible with the native Rig (inspect sample/instrument setup before selecting source). Keep real MIDI Start, zero-position/inside-cadence and repeated between-sample restarts, nonempty playing, changed and matching epochs, document revisions41/42, file/span bounds and original scheduled duration assertions. Do not remove assertions, inject SrcRef manually or waive tests. Avoid production edits unless fixture diagnosis proves a real owned-source defect. Required rust-coding and check-and-test-after-modify agents run after any Rust modification; snapshot every fixture/formatting edit.
Capture sensitivity again against the prior heuristic using fresh immutable snapshots and serial restoration of exact current owned bytes; run both native restart and real-Wasm tests, preserve failure logs and verify restored posthashes before final tests. Never disturb unrelated files or retained worker changes.

### Exact Clippy disposition proposed for independent review
Read FULL `tmp/canvas-editor-224/root-observations/telemetry-clippy-baseline/clippy.log` (terminal exit101), `clippy.json`, `diagnostic-comparison.json`, `snapshot-provenance.json`, `restored-paths.json`, `reconstruction-difference.json`, source-before/after records and `limitations.json`; also `clippy-source-ownership.json`. Same Rust1.98.1/current Cargo.lock reconstruction restores eight earliest verified telemetry paths. All27 current unique diagnostics match baseline in23 byte-identical diagnostic files; current-only0, baseline-only1 publisher unnecessary_map_or. This is a reconstructed pre-telemetry comparison, not a complete historical clean checkout.

Keep canonical `cargo clippy --locked --all-targets -- -D warnings` unchanged and its exit101 visible. Do not allocate 22 unrelated files for stylistic cleanup; existing native/audio.rs remains CE-AUDIO owned. Step5 must explicitly accept or reject the bounded proposal: require zero introduced diagnostics AND zero diagnostics in CE-TELEMETRY-owned paths, normalized by lint ID + relative path + complete source expression, with no unrecognized blocks. On every retry compare fresh canonical output and source hashes to the evidence. New/changed diagnostics fail. Any diagnostic-source hash drift invalidates reuse for that file and requires renewed baseline/source review before disposition. CE-AUDIO changing audio.rs cannot inherit its old hash disposition. Report global gate failed, scoped comparison outcome separately, and review decision/evidence. If rejected, keep blocked and request runner-owned bounded allocation; do not retry unchanged work or soften compiler flags. Do not claim accepted design compliance for an unreviewed verification exception.

Retain attempt-1 nextest exit101, attempt-2/final-retry Clippy exit101 and both ownership blocked fingerprints from prior attempts; redispatch audit fingerprint `b1c67f2e7a977c59fab3e9fa1673e941ea1553a2484a291ad5108a9c5da1340f` remains blocked history. The two earlier ownership blocked fingerprints are `1c47c931758a82cfa0d8d9eb5998961836d0f3c5b90b0cc6cbc8fa8b5f69debd` and `837f2e2a2e71b9ca68ef6e53907f35964cd986cb756f0b7241811265d248ec8c`, recorded in `tmp/canvas-editor-224/CE-TELEMETRY/ownership-rerun-20260930-184532-404133/ownership-audit.json`. Retain both alongside the later redispatch fingerprint.

### Verification continuation
After Step5 accepts this amendment/disposition, repair T5 and execute ALL EIGHT exact retry commands in the preceding section, even if Clippy fails. Tell required verification agent explicitly to continue independent commands after lint failure. Use exclusive `tmp/canvas-editor-224/CE-TELEMETRY/recovery-235/` logs (if already exists allocate a new attempt directory), retaining attempt-2 logs untouched; target directory may remain attempt-2/target as specified. Session and scheduler nextest commands additionally use `--no-fail-fast` to establish all selected results, not a partial run. Each command needs full log, terminal exit, positive behavioral counts where applicable and pre/post source identities. Real-Wasm tests must load the freshly built artifact; never skip/mock. Canonical lint failure is not a reason to omit native/Wasm verification. Check/build/format and all behavioral commands must actually exit0; Clippy is reported failed plus independently reviewed scoped comparison. Native progress/test-integrity/adversarial/integration decisions remain mandatory.

- [ ] T5 source-bearing restart tests retain all epoch/revision/file/span/duration assertions and pass; sensitivity remains demonstrated.
- [ ] All eight commands reach terminal exit with complete source-matched logs; behavioral failures and skipped tests cannot pass.
- [ ] Canonical lint result retained; scoped zero-introduced/zero-owned disposition independently accepted or unresolved block explicitly reported.
- [ ] Native progress and formal gates replace blocked status authoritatively.
### Session: 2026-09-30 — Step4 session235
T5 assigned to existing publish-test owner. Actual isolated lint evidence reviewed for this bounded proposal; no unrelated repair paths added. Historical failure/status preserved. No Rust modified and no implementation acceptance claimed.
