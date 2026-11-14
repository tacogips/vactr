# Closed preparation obsolete helper removal implementation plan

**Plan ID**: SONG-CLOSURE-OBSOLETE-HELPERS
**Status**: In Progress
**Design Reference**: [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)
**Created**: 2026-10-02
**Last Updated**: 2026-10-02

## Purpose and related plans

The actual once-only retained-shape path replaces the old extra-root traversal
and routing facade. Strict Clippy after ROOT0408 found those private freeze
helpers unused in both production and tests. Remove only that obsolete code;
the closure plan retains its eight paths and owns its separate import and
test-only compatibility fixes. Freeze copy accounting and cache behavior remain
the accepted ROOT0400 prerequisite.

## Exact future write manifest

| Path | Observed size | Deliverable | Status |
|---|---:|---|---|
| `src/session/song/freeze.rs` | 827 | Remove unused private facade and old extra-root traversal | Not Started |

This companion and its one Rust path require a separate source release. No spine
test, additional source, index, archive or dependency edit. Keep the file below
1000 lines. No blanket dead-code suppression or second work counter is allowed.

## Preservation and interface contract

Remove only the unreferenced `capture_routing`, `extra_roots` and its sole helper
`append_family`. Fresh repository search must confirm no remaining callers.
`children` is still required by inventory/discovery and remains unchanged.
Remove the getter's narrow pending-consumer dead-code allowance because actual
production now calls it. Preserve the declaration and behavior:

```rust
pub(crate) const fn consumed_work(&self) -> u32;
```

Every existing Freeze copy/cache/depth/admission/prototype/slot/mapper function
and all nine spine test bodies remain unchanged. Remove an import only if made
unused by these exact deletions. Preserve current successful-pass accounting:
read actual work, drop Freeze, debit once; failure disposes the whole candidate.

## Tasks and dependencies

| Task | Deliverable | Dependency | Status |
|---|---|---|---|
| TASK-001 | Fresh complete caller/byte audit | ROOT0408 terminal failure | Completed |
| TASK-002 | Exact obsolete helper deletion | TASK-001 | Completed |
| TASK-003 | Joined mandatory regression proof | Closure and pool held | Not Started |

All tasks are sequential. Author records before/after hashes and exact removals.
Independent verification preserves the failed ROOT0408 logs and uses a fresh
output directory. No Cargo until every source author is held and root releases
the new cohort.

## Completion criteria

- [ ] Only the obsolete private helpers and pending-consumer allowance are removed.
- [ ] All retained Freeze production functions and spine test hashes are preserved.
- [ ] Fresh native/browser and strict Clippy checks pass on the joined source.
- [ ] Nine exact spine tests and affected whole session/source/closure regressions
  execute with nonempty actual inventory/run agreement.
- [ ] Scoped format and line checks pass; source hashes remain fixed during checks.
- [ ] No resource closure, geometry, host Ready or full playback claim is inferred
  from unused-code removal.

## Progress log

### Session: 2026-10-02

ROOT0408 original process78949 exited101 at strict Clippy after native/browser
builds passed. No behavioral test ran; all112 hashes matched. Root freshly
confirmed the three freeze helpers have no caller outside their obsolete block,
while inventory test wrappers remain used by real private tests. This prior
companion owns the otherwise ninth freeze source path explicitly.

### ROOT0410 source repair (independent rerun pending)

Immutable 0001 verified all exact baselines and saved normalized retained Freeze prefix, children and complete nine-test spine hashes. Removed only the three obsolete freeze helpers and pending-consumer getter allowance; children, getter body and retained Freeze logic are unchanged. Closure separately marks its existing inventory compatibility wrappers and local legacy consumed_work field cfg(test), retaining actual test accounting. Native/wasm passed in the prior attempt; strict Clippy exited101 before behavioral tests. No Cargo ran in this repair. Completion and joined behavior evidence remain pending.
