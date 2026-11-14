# Retained execution membership for issued queries

**Status**: Ready
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Immutable consumers and authentic capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose and dependencies

Allow the immutable routing bridge to authenticate an issued invocation against
a genuine retained execution without exporting raw records or seal constructors.

- **Previous**: [Frozen issued events](song-mode-frozen-issued-events.md).
- **Next**: [Immutable route authority](song-mode-immutable-route-authority.md).
- **Related**: [Issued query authority](song-mode-issued-query-authority.md).

The exact two-path scope is Ready after frozen0002 acceptance. Exact retained membership handles observed
executions; permitted first executions and varying seeds remain required in
later admission work, not permanently refused by this primitive's scope.

## Proposed exact manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/pattern/eval/song_provenance.rs` | Borrowed metered membership API | Not Started |
| `src/pattern/eval/song_provenance/retained_tests.rs` (new) | Genuine owning fixtures | Not Started |

The current provenance parent is954 lines. Keep it strictly below1000; bulk
fixtures belong in the declared child. Author feasibility and independent
read-only review confirm this scope. Declare any additional path/extraction
before edits.

## Private signature

```rust
impl IssuedQueryTranscript {
    pub(crate) fn authentic_retained_invocation(
        &self, original: &Rc<Song>, seal: &Rc<InvocationSeal>,
        retained: &OwnerInvocation, work: &SharedIndexWork, depth: u32,
    ) -> Result<bool, Failure>;
}
```

Validate the original transcript/seal relation and actual inherited work/depth,
then use the private completed execution pointer and retained invocation's
genuine membership method. No scalar owner-key reconstruction, invocation
pointer comparison, callback execution, new ledger or allocation is needed.
Charge every record scan before comparison. Preserve source/owner/seed/entry
checks in the following lookup adapter; this narrow boolean does not certify
an arbitrary copied site, source member or complete geometry address.

Use the existing authentic leaf boundary consistently (depth greater than
max_depth is refused). Successful membership does not certify projection depth:
the downstream lookup must enforce actual admitted depth before geometry access
and use the issued invocation's placement, seed, entry and rebound clock.

## Tasks

### TASK-001: Feasibility and owning API

**Status**: Not Started
**Parallelizable**: No

- [x] Author confirms two-path scope/signature and genuine fixture construction.
- [x] Expose exact original/retained execution membership with inherited meter.
- [x] Preserve raw record/seal privacy and all existing query behavior.

### TASK-002: Genuine membership evidence

**Status**: Not Started
**Parallelizable**: No; depends on TASK-001

- [x] Actual observation/replay supplies retained invocation; actual issued query
  supplies a fresh invocation seal sharing that raw execution and is accepted.
- [x] A genuine different execution of the same original Song is refused.
- [x] Foreign original/retained invocation and swapped actual seals are refused.
- [x] Callback denial proves authentication does not query or read the VM.
- [x] Complete exact/one-less work and inherited depth failures are metered.
- [x] Independent focused and regression/native/lint/WASM/format gates pass
  against unchanged held inputs and actual nonzero fixture names.

## Completion criteria

- [ ] Both tasks and owning fixtures pass with source/terminal receipts.
- [ ] Immutable bridge actually consumes this API in its subsequent phase;
  an unused helper does not prove full production integration.
- [ ] Pre-Reserve admission, varying domains and full playback remain required.

## Progress log

### 2026-10-03 — Missing private read interface identified

Read-only review confirms OwnerInvocation::authentic_execution is pub(super)
and execution() is test-only. Provenance owns the actual completed pointer and
can authenticate without exposing either. Separate two-path draft prevents
hiding this prerequisite as a ninth immutable-route module. Frozen5 is still
being implemented; no source release here.

### 2026-10-03 — Two-path feasibility confirmed under frozen hold

Author confirms the metered scan and owning method fit the954-line parent with
the child declaration below1000. The completed record already owns the exact
raw execution pointer; authenticate original/seal then charge the actual scan
and use retained.authentic_original plus authentic_execution. No allocation,
callback, new ledger or exported raw execution accessor is required.

The child uses genuine independently evaluated original Song, actual Freeze,
observe_part and published ReplayView with the same isolated evaluator and
closed asset loader. Capture the retained actual OwnerInvocation, then issue
the real query against that replay. Also issue a genuine no-replay execution
on the same original to prove descriptor equality cannot substitute for raw
execution membership. Include distinct owner/execution, foreign original,
swapped seals, denied reads, full exact/one-less work and inherited depth.
No snapshot-private fixture access or extra source path is needed. This plan
stays Planning while frozen940 focused checks run; no concurrent edits.

### 2026-10-03 — Independent read-only primitive review

Reviewer confirms eval privacy permits the exact retained execution check and
the second completed-record scan must be charged before lookup. Shared raw
execution alone does not authenticate copied site/source policy or fresh
placement/seed/entry/clock; those remain mandatory route adapter checks. The
genuine replay versus fresh no-replay executions, foreign/swapped seals,
callback denial and budget/depth fixtures cover this primitive. Source release
still awaits frozen repair acceptance; no source edits or Cargo in this review.

### 2026-10-03 — Ready after frozen0002 full acceptance

Frozen0002 passes413 distinct Rust tests, native, strict lint, fresh WASM and
scoped format. Root frontend590/78files and build pass that exact WASM;
all940 source hashes and top/deps/dist artifact hashes match after every handle
is terminal. Root receipt SHA256
cd8cef3de20b34e2bbc0b7007da65079cfd7af06970c3097032149b7d79ec4f4.
Release only the declared two paths after saving their full original texts.
Use owning actual capture/replay fixtures, preserving narrow boolean authority:
different placements sharing the same genuine raw execution can pass this
primitive; their site/placement/seed/entry/clock checks belong in the following
trusted route lookup. An unrelated actual raw execution must fail, even when
public descriptors match. No new dependencies or additional Rust paths.

### Implementation intent — released after frozen acceptance

Frozen002 accepted actual413 Rust tests plus590 frontend tests and build,
native/strict/WASM/format all0 and full940 unchanged. The two-path API now
performs original/seal authentication followed by a separately precharged
completed-record scan and exact retained execution/original membership. Its
boolean intentionally does not prove site/placement/seed/clock policy. Owning
fixtures use actual isolated evaluation/capture/replay; no raw constructor or
callback fallback. Independent runtime acceptance remains pending.

### Source held0001 — author compile-only checkpoint

Four genuine owning fixtures cover fresh replay execution membership without
reads; genuine new same-Song execution refusal with equal output handles; real
foreign/swapped seals; full two-scan exact/one-less and depth equality/excess.
The fixture owns the actual isolated evaluator, original Freeze and collector
invocations before real ReplayView publication. No authority is fabricated.
Different invocation Rc can authenticate the same raw execution; the API does
not certify copied placement/seed/entry/source policy. No same-execution distinct
placement construction is claimed by these fixtures.
Compile001 original10868/f136ec/101 caught four private collector field accesses;
replaced them with the existing remaining() accessor. Compile002 original15427
terminatedfb8176/0 with empty log. Both handles terminal; two-file format/check0,
parent978/child386 lines. Independent behavioral/gate acceptance pending.

Before publishing held0001, the same-Song noReplay witness was strengthened
to compare actual whole/part/music/control/route/commit/producer and genuine
frozen original source metadata as well as handles. Its separate comparison
copy allowance is test evidence, not membership operation work. Quiet compile003
original45057 terminated9d5e65/0 with empty log; all three handles are terminal.
Scoped two-file format check passed. Independent runtime still pending.

### Repair0002 — preserve actual constructor debit in whole cost

Independent focused001 original45342/9a0507/101 executed four tests, three
passed; cost assertion measured15 versus expected14 before boundary assertions.
CanonicalIndexCollector::new genuinely debits1 before the two actual7-unit
scans. Whole operation expectation includes that constructor cost; exact15 and
one-less14 inputs remain original caller allowances. Depth refusal preserves
post-constructor remaining14, not the pre-constructor15. Production API unchanged.
Full941 exact and all checker handles terminal; owning backup retained.

Repair author compile004 original73077 terminatedf0bd45/0 with an empty log.
All handles terminal; two-file scoped format/check0. Held0002 changes only
the expected full constructor-plus-two-scan cost and post-constructor depth
remaining assertion in the declared fixture child. Independent retry pending.

### Repair0003 — strict fixture borrow only

Broad002 executed actual417 distinct tests and native check successfully;
strict Clippy then rejected one needless borrow in the fresh invocation Rc
comparison. Original65019/c3e5d1/101 terminal; full941 unchanged. Removing that
one leading borrow changes no runtime authority or assertion. Quiet compile005
original83679 terminatedfa9e44/0; all author handles terminal; scoped2fmt/check0.
WASM/checker-format remained unexecuted after lint failure; retry acceptance
pending. Four fixture bodies and production API unchanged.

### Accepted membership003 source gates; actual consumer pending

Fresh417 distinct Rust tests/native/strict/WASM/scoped2format passed, all941
unchanged, focused52370/95b924/0 and broad70458/a527c4/0 terminal. Final SHA
3d6f3e25a8825f68ca228b315704e354177085244e582eef1d6a3beba6308a99.
Root590 frontend tests/build and source/artifact equality accepted, receipt
6b82a04b27f0f96e23e3e0adc18ec0dc0dcaba247912cb0ba62012c94e53335a.
The primitive and owning evidence are verified. Overall remains In Progress
until the explicit immutable route consumer criterion is implemented.
