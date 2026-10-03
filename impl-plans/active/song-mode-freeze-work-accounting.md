# Freeze work accounting implementation plan

**Plan ID**: SONG-FREEZE-WORK
**Status**: Completed
**Design Reference**: [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and scope

Expose immutable actual successful Freeze work so isolated candidate closure can hand off one cumulative budget across once-only callback discovery, copying and frozen validation. This prerequisite changes no quota, cache, depth, cycle, copy or error semantics. Failed attempts dispose the candidate. Getter availability alone does not implement phase2 or close PCM. ROOT0386 releases this document only; source requires separate authorization.

## Related plans and dependencies

- [Capture](song-mode-graph-resource-capture.md): independently accepted ROOT0385.
- [Closure](song-mode-graph-resource-closure.md): Planning consumer; retains eight separate future paths.
- Existing namespace/reachable Freeze guarantees remain unchanged.

## Exact two-Rust manifest

| Path | Current lines | Deliverable |
|---|---:|---|
| `src/session/song/freeze.rs` | 980 | Readonly getter and cfg(test) child registration |
| `src/session/song/freeze/spine_tests.rs` | absent | Four verbatim existing tests plus meaningful work tests |

Extract the current cfg(test) spine_tests module body818–980 into the standard child with `use super::*`. Parent replaces its inline test module with cfg(test) mod spine_tests; public imports stay unchanged. Expected parent approximately823, child initially162. Every touched Rust stays below1000. No other files, dependencies, index, archives or protocol changes.

## Exact declarations

```rust
impl Freeze<'_> {
    #[must_use]
    pub(crate) const fn consumed_work(&self) -> u32;
}
// Existing name and qualified test paths remain stable:
#[cfg(test)]
mod spine_tests;
```

Getter returns only current self.work. It does not reset/charge/validate/cache/clone anything. Existing work grows through enter and admit with checked arithmetic; attempted failed admission can leave work above the configured quota. Only a completed successful copy supplies a debit to the later candidate consumer. Overflow does not wrap.

### Existing fixture preservation

Move all four functions and their existing helpers/assertions verbatim:

```rust
fn cached_height_respects_new_parent_depth_and_cleanup();
fn aggregate_cache_height_cannot_hide_a_deep_pattern_alias();
fn cached_part_and_prototype_retain_descendant_heights();
fn spine_preserves_fields_params_and_admits_storage_before_growth();
```

### New exact meaningful fixtures

```rust
fn empty_and_cached_list_report_actual_work();
fn prototype_payload_work_includes_full_copy_arrays();
fn exact_work_quota_and_one_less_preserve_admission();
fn collection_refusal_precedes_child_clone();
fn checked_work_overflow_never_wraps_or_copies();
```

Fresh Freeze reports0. Copy a real ListVal of two primitive integers: enter1 + collection admission2 + scalar enter2 =5. A second identical Rc cache hit adds1, retains exact copied Rc identity and changes no collection; repeated getter calls leave work/caches unchanged.

Prototype fixture uses actual FnProto construction/compiled payload and existing prototype_copy_work: include code/spans plus masks/call/list sites and their nested lengths, constants and prototype children. Establish actual delta from enter + full helper charge + recursively copied values; a dependency-node-only count must be observably insufficient. Existing private helper prototype_copy_work remains readonly in session/song.rs.

Quota5 accepts the exact list; quota4 honestly refuses without installing its failed cache entry. A large genuine list under maxwork1 must fail collection admission before children are visited/cloned: retained nested Rc strong counts unchanged, no child/root copied-cache entries and existing depth/visiting cleanup preserved. Assert actual failure code as existing guards produce; do not add new errors or change limits.

Private arithmetic boundary fixture sets existing work to u32MAX, calls actual enter/value path and verifies checked overflow retains MAX and produces no copy/cache. This is an arithmetic invariant test, not a claim that a real successful candidate consumes MAX. No production counter mutation or external forged accounting.

## Integration handoff

Later closure constructs Freeze with current remaining max_walk_nodes and the existing depth limit. Internal enter/admit guards preadmit each existing allocation. After success it reads consumed_work, retains result values, explicitly drops Freeze's immutable asset borrow, then checked-subtracts once before the next stage. Namespace and reachable passes have separate actual debits; one reachable cache copies Song plus retained callback/input/output values. Failed passes dispose all private candidate state; no quota refund/retry using failed cost. Final ClosedShapeCtx needs vm/ns and mutable closed assets only after Freeze is gone.

Do not modify closure sources in this prerequisite or claim its implementation/PCM proof. Existing constructor work and every cache/depth/error behavior remain exact.

## Tasks

### TASK-001: Cohesive test extraction and readonly getter

**Status**: Completed
**Parallelizable**: No

- [x] Record immutable exact source baselines before edits.
- [x] Move four old tests verbatim and preserve qualified paths.
- [x] Getter readonly const crate API implemented; no behavior mutation.
- [x] Both files remain below1000; scoped formatting passes.

### TASK-002: Actual admitted work proof

**Status**: Completed
**Parallelizable**: No (depends on TASK-001)

- [x] All five new meaningful fixtures written and actually pass.
- [x] Full prototype/collection admissions and cached alias increments are exact.
- [x] Exact/one-less, preclone refusal and overflow prove existing guards.
- [x] All four prior spine/cache-height tests pass unchanged.

### TASK-003: Independent verification and consumer handoff

**Status**: Completed
**Parallelizable**: No (depends on TASK-002)

- [x] Held source/plan SHA receipts and nonempty qualified inventories captured.
- [x] Root-coordinated native/browser, strict Clippy, scoped fmt/diff and affected default-depth/candidate/source regressions pass; failures retained.
- [x] Independent actual execution accepts getter scope.
- [x] Document exact successful-pass debit contract; closure source remains separately authorized.

## Module status

| Module | Status | Evidence |
|---|---|---|
| Getter/extraction | Completed | ROOT0400 exact readonly getter and old-body SHA |
| Work fixtures | Completed | Nine actual qualified freeze fixtures ROOT0400 |
| Closure consumer | Pending | Separate eight-path plan |

## Completion criteria

- [x] All three prerequisite tasks independently verified.
- [x] No quota/cache/depth/copy semantics changed and old tests retained.
- [x] Genuine success/failure/overflow work evidence retained with exact hashes.
- [x] No closed PCM/full playback claim; later consumer still pending.

## Progress log

### 2026-10-02: ROOT0386 document-only declaration

Read freeze.rs980 and inline spine_tests818–980, existing prototype_copy_work and allocation admission sites. Getter plus cohesive extraction fits two paths. No Rust/Cargo or other plans/index/archive mutated by this new plan. Source release and mandatory checker remain required.

### ROOT0388 source batch declarations

Private test helpers `fn work_pair() -> Value` constructs a genuine two-Int ListVal; `fn work_proto() -> Rc<FnProto>` constructs existing Arity/FnProto/CallSite/ListSite/Shape/ForcingMask values with code, spans, constants, nested proto and nested mask links. No VM executes this immutable copy payload. Getter has a narrowly justified `#[allow(dead_code)]` only while the separately authorized closure consumer is pending; it remains crate-private rather than widening authority merely to suppress warnings. Four existing tests/helpers are moved verbatim, with original dedented body SHA8431a458b1d46dc157261ff2dcdf0418e1a6ea3c8c0eb770e6689cab5dc30cc9. Before intent0005; no Cargo.

### ROOT0388 source-ready held milestone

Parent827/child364 lines, four old tests/helpers dedented SHA8431a458b1d46dc157261ff2dcdf0418e1a6ea3c8c0eb770e6689cab5dc30cc9 unchanged. Five new work fixtures written; actual overflow source helper remains HostUnavailable with raw freeze work overflow message, not an invented diagnostic. Exact two-file scoped rustfmt/check0. No Cargo/live handles, no behavioral passing claim. All task verification checkboxes remain pending until root-coordinated mandatory joined checker. Getter reports existing work only; closure successful-pass debit remains future source work.

### ROOT0401: Scoped prerequisite completion

Independent ROOT0400 acceptance (SHA659866f5b3bd880416f18bbb28e72c0411707a8d484366835f08c0b0c2b0f3d7) verified67 command gates,1026 exact unit/integration names plus4 privacy executions =1030 distinct across the joined matrix. Matrix `/tmp/vactr-freeze-reuse-independent-004/final-results.json` SHA f34310087d469cdf8de2edadf2d68000cc25ddc8c50e81b2ea40812855c8953e; original27304 terminal0. Supplemental `/tmp/vactr-freeze-reuse-nextest-independent-001/final-results.json` SHA d405431dc16956e4eae985688518ccedd788b3077f93774d603849c4485ccbd2, foreground4f743a terminal0:18 selected PASS,0 selected skips,1934 unselected skips. Exact inventory/run identities and all102 current hashes matched. Joined totals are not child-only counts. Native/wasm/strict Clippy/scoped format/diff and relevant actual regressions passed. Earlier failed attempts and repair receipts remain retained.

All nine actual freeze fixtures passed, covering existing default-stack/cache-height/depth behavior, getter0, exact list5/cache+1, prototype full arrays, exact5/one-less4, preclone collection refusal and checked overflow. The four old test bodies remain verbatim. Only getter/extraction/work-proof scope is Completed. The separately owned closure remains unimplemented and does not yet debit successful Freeze work in candidate construction. Sources stay held; this entry changes documentation only.
