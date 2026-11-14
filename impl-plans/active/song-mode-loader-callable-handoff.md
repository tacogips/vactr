# Loader callable metadata handoff implementation plan

**Status**: Completed
**Plan ID**: SONG-07D
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Function composition](../../design-docs/specs/design-song-mode.md#proposed-language-surface) and [Snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent and current evidence

Ordinary reusable functions produce Parts and transform selected patterns. Actual
verified package loading uses PkgNs::load, while source loading uses run_source.
Both discard CheckResult.callables and execute VM forms directly. SONG-07C installs
portable checked callable schemas through Evaluator; its implementation alone does
not certify functions in these actual loading paths. Preserve successful namespace
commits, existing whole-document diagnostics, bounded weak dependency stamps and
one shared positional/keyword scheme. This supporting refinement is source-grounded
and has not received a separate Riela review. The accepted design and full goal
remain authoritative.

## Manifest

```json
{
  "planId": "SONG-07D",
  "planPath": "impl-plans/active/song-mode-loader-callable-handoff.md",
  "dependsOn": ["SONG-07C", "SONG-07"],
  "writePaths": [
    "src/ns/pkg.rs",
    "src/ns/load.rs",
    "tests/song_callable_loading.rs",
    "impl-plans/active/song-mode-loader-callable-handoff.md"
  ],
  "sharedPaths": []
}
```

## Related plans

- **Previous**: [Transform types and callable schemas](song-mode-transform-type-contract.md).
- **Depends On**: SONG-07C final independent clearance; existing candidate isolation.
- **Next**: Complete route preparation, DSP, hosts, transport and export.

## Ownership and execution

Required rust-coding author and independent check-and-test-after-modify agent.
Do not begin Rust edits until root releases this phase after07C clearance. Record
fresh immutable before/after checksums and declared intentions under
`tmp/song-mode-riela/SONG-07D/`; never restore stale files. Only three Rust paths
and own plan are authorized. Namespace/evaluator/checker source are dependencies,
not writable paths. Any API gap requires an exact prior amendment. Touched Rust
files remain below1000 lines; plan remains below1000 lines/eight modules/ten tasks.
No dependency, lockfile, Git, shared index or archive edits. Preserve unrelated work.
All Cargo commands use CARGO_TERM_QUIET=true and repository nextest quiet settings.

## Modules and contracts

### src/ns/pkg.rs

Preserve the existing API:

```rust
pub fn load(
    id: PackageId,
    prelude: Rc<Prelude>,
    vm: &mut Vm,
    sources: &PkgSources,
    manifest: &HostManifest,
    next_file: &mut dyn FnMut(&str) -> FileId,
    imports: &mut dyn FnMut(&ImportDecl) -> Option<Rc<PkgNs>>,
) -> (Rc<PkgNs>, Vec<Diagnostic>);
```

Successful per-form checked metadata publication uses the actual committed form
owner and pre-form validated inputs. Prior same-file helpers and transitive imported
helpers must remain usable. Keep whole-document diagnostic spans, package failure
aggregation, import order and direct VM staging behavior. A check, compile or run
failure cannot yield a usable certificate for the affected declaration. Later
replacement cannot validate metadata from an earlier same-name definition. Do not
install all document exports against one initial namespace snapshot.

### src/ns/load.rs

Keep SourceLoader and load native APIs unchanged. Retain fresh prelude-only source
namespace, whole-document diagnostic reporting, last-value semantics, observer and
host restoration, effect-in-query checks and depth/load error handling. Publish
successful checked declarations for internal same-file composition using the actual
form owner. Preserve ordinary diagnostic-only loading behavior; certification
failure must not manufacture a successful type proof. Do not reopen closed song
assets or introduce active namespace/resource aliases.

### tests/song_callable_loading.rs

Use real verified MemCache/LockFile package sources and actual
`evaluate_song_candidate`/`prepare_song`, plus real source loader fixtures. No network
or disk dependency is necessary. Synthetic CheckEnv sidecars alone are insufficient.

## Module status

| Module | Deliverable | Status | Tests |
|---|---|---|---|
| src/ns/pkg.rs | Actual package checked-metadata publication | IMPLEMENTED | Independently verified |
| src/ns/load.rs | Actual internal loaded-source composition | IMPLEMENTED | Independently verified |
| tests/song_callable_loading.rs | Production loading and stale/failure fixtures | IMPLEMENTED | Independently verified |

## Dependencies

| Dependency | Required evidence | Status |
|---|---|---|
| SONG-07C | Stable complete callable schemas and independent clearance | VERIFIED |
| SONG-07 | Private verified source/sample loading | VERIFIED |

## Tasks

### TASK-001: Confirm exact loader declarations and fixtures
**Status**: Completed
**Parallelizable**: No

- [x] Re-read current loader/metadata APIs and record exact fresh hashes.
- [x] Record signatures of any additional declarations before implementation.
- [x] Establish real package callback failure and same-file dependency case.

### TASK-002: Publish actual committed declarations
**Status**: Completed
**Parallelizable**: No
**Depends On**: TASK-001 and independently cleared07C

- [x] Package qualified/open functions retain complete portable schemas.
- [x] Earlier same-file/cross-file/transitive helper dependencies are stamped.
- [x] Internal loaded-source function composition retains checked metadata.
- [x] Failed/replaced definitions never receive stale usable certificates.
- [x] Legacy diagnostics and loader effect/resource behavior remain intact.

### TASK-003: Author and independent verification
**Status**: Completed
**Parallelizable**: No
**Depends On**: TASK-002

- [x] Actual package Part generator and selected-pattern callback both succeed.
- [x] Qualified/open lookup, keyword defaults and shared-variable negatives pass.
- [x] Actual source-load composition returns a finite editable Part correctly.
- [x] Invalid keyword/arity/input/result and stale dependency fixtures reject.
- [x] Failed check/compile/run and package replacement preserve honest metadata.
- [x] Scoped fmt/diff, native check, strict Clippy, wasm and scoped regression gates pass.
- [x] Retain exact terminal commands/results, source seals and test counts.
- [x] Independent checker verifies a stable author-sealed batch with all Rust held.

## Explicit evidence boundary

A loaded file returning a Fn by value is a distinct transport boundary: the current
certificate lives on a namespace slot, while run_source returns only Value. Merely
publishing inner declarations does not prove that a caller's new let slot receives
valid metadata. Record a real returned-Fn callback probe and its evidence. If the
accepted callable use requires that boundary, prepare a separately bounded opaque
certificate handoff plan before any extra source changes; do not silently claim it
supported, waive type checks, or mark the full song-mode goal complete with an
unresolved required composition case. This plan does not change Closure layout or
introduce a global pointer registry.

## Completion criteria

- [x] All three modules and actual production loading fixtures implemented.
- [x] Same-document and imported dependencies are valid, bounded and weakly retained.
- [x] Complete schemes preserve positional/keyword quantifier sharing.
- [x] Author and independent final gates pass with exact current source seals.
- [x] Returned-Fn boundary evidence is recorded and any required follow-up remains tracked.
- [x] Full playback/Apply/mute/export scope remains intact and incomplete phases tracked.

## Progress log

### Session: 2026-10-01 — source-grounded loader handoff

Root and independent checker inspected pkg.rs119–137 and load.rs173–187, confirming
that document checking discards callable exports before direct VM runs. The checker
also established the returned-Closure certificate boundary. This plan adds no Rust
edits and does not release implementation before07C clearance. ROOT0052 records
fresh dependency hashes. No Git/index/archive changes.


### Session: 2026-10-01 — authorized loader publication

SONG-07B/07C independently cleared in ROOT0058:574 distinct fixtures+60 repeats,
all gates0, retained70199 terminal0. Own07B plan completed doc-only; cleared Rust
remains held. Current07D exact three-path implementation is authorized. Fresh
immutable0001 records actual design/plan/source baselines and missing fixture file.
Loader public APIs stay unchanged. Per-form checking produces eligibility only:
existing whole-document diagnostics remain exactly once and do not newly gate
legacy execution. Pre-run checked inputs plus the successful actual committed
FormGen certify only that form's declarations; compile/check/run errors skip
publication. Package generations retain current behavior; source FormGen0 remains
unchanged with exact version/Closure dependency stamps. No new public declaration,
Closure layout, source ownership or active/global registry bridge is introduced.
Behavioral fixtures and all final/independent gates are still pending.


### Author declaration: private committed source-form helper

Extract the existing source execution loop into private
`run_forms(vm: &mut Vm, ns: &Namespace, path: &PathVal, forms: &[Node]) -> Result<Value, Failure>`
in owned load.rs. run_source still creates its fresh prelude-only namespace, reads
forms and reports whole-document diagnostics exactly once, then delegates to the
same loop. This private split lets crate-local fixtures inspect the actual owned
source namespace's certificates/default dependency stamps without exposing it to
the source caller or adding a public/global certificate transport. Source owner
FormGen0 remains unchanged; no active namespace is passed in production.


### Session: 2026-10-01 — focused loader implementation ready

Production publication and private source-loop extraction are complete within the
three-path manifest. Public `song_callable_loading` now passes all ten fixtures
unfiltered (retained16552 terminal0, tests-sixth.log). Two crate-local committed
source namespace fixtures pass (38118 terminal0, source-private-tests-third.log).
These cover exact FormGen0 ownership, default dependency mutation, same-owner
replacement invalidation, and failed replacement preserving prior proof. Earlier
private failures were caused by core-only test Prelude lacking domain natives;
fixtures now register the same domain natives as production. No ownership or
certification assertions were weakened. Source candidate failed at64944 terminal101
because detached page keys are exact PathVal text; the inventory now uses the real
`./loaded.vact` key and the complete ten-fixture rerun passes. All failure logs remain.

Public evidence includes real verified qualified/open/transitive package generators
and callbacks, shared positional/keyword schemes, independent check/compile/run
failures with later package continuation, exact whole-document diagnostic counts
and spans, source-returned editable Part and isolated closed-source candidate.
A file-returned Fn is invocable by value to obtain a Part but its caller slot lacks
portable callback certification; that separate transport boundary remains explicit
and is not claimed supported by this phase. Full song playback remains incomplete.

Scoped formatting was applied only to the three owned Rust files. Final native,
wasm, strict Clippy, legacy regressions, nextest and independent verification remain
pending a root-coordinated stable all-author write hold. This is focused readiness,
not phase completion or final verification.


### Session: 2026-10-01 — final author verification and sealed hold

ROOT0064 authorized a stable all-author window; routing and all loader Rust remained
held. Author runner17031 exited0. All eleven gates in
`tmp/song-mode-riela/SONG-07D/author-final-results.json` exited0: native check,
host-wasm check, strict all-target Clippy, public loader tests, private source
certificates, namespace/session legacy, candidate/assets/checker/source-use
regressions, nextest loader repeats, scoped rustfmt check and scoped diff check.
243 distinct tests passed: loader10, source-private2, namespace70, session78,
assets23, candidate14, checker29 and source-use17. These disjoint test namespaces
and integration binaries have no overlaps in that sum; nextest's ten loader tests
are separately counted repeats. No empty filters, modified stack sizes or lowered
limits were used. The runner verified every three-source and own-plan SHA against
0005 before this documentation-only evidence update. All earlier failed logs remain.

Final author seal records exact unchanged Rust hashes, line counts, gate ledger SHA,
terminal17031 exit0 and the updated plan SHA. All loader Rust and own plan are held
again for mandatory independent verification. TASK003 and phase completion remain
pending independent clearance. File-returned callable values still lack a portable
caller callback certificate; zero-argument returned generator invocation producing
a Part is tested, not misreported as full callback transport support. Full song
routing/playback/Apply/mute/export remains active beyond this loader phase.


### Session: 2026-10-01 — independent phase completion

Root accepted independent clearance in ROOT0066 after fresh hash verification.
`/tmp/vactr-song07d-independent-001/final-results.json` records eleven gates exit0,
243 distinct fixtures plus ten separately counted nextest repeats; retained14341
terminal0 with no active process and no concrete findings. Loader and held routing
hashes remained unchanged. This documentation-only completion is recorded in
0008-independent-completion before/after intent; all three cleared Rust sources
remain held and unchanged from0007 final author seal. SONG-07D is Completed only
for loader callable publication. The tested returned-Fn callback-certificate
transport limitation remains explicit; full routing/playback/Apply/mute/export
continues under subsequent plans. No archive, index, Git or other source edits.
