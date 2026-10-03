# Song sample classification implementation plan

**Status**: Completed
**Plan ID**: SONG-04R
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance)

## Intent and source evidence

Existing commit::notes selects explicit note first, otherwise n only when the
resolved audio route has no sample resource. Audio resources include banks, paths
and buffers. Sound::Inst may have a sample resource; Builtin may resolve to a
registered synthesizer without one. Sound enum tags alone cannot classify n.
Canonical mono realization must use the same route fact before expanding tones.

## Manifest

```json
{
  "planId": "SONG-04R",
  "planPath": "impl-plans/active/song-mode-sample-classification.md",
  "dependsOn": [
    "SONG-04Q"
  ],
  "writePaths": [
    "src/pattern/eval.rs",
    "src/vm/query_vm.rs",
    "tests/song_sample_classification.rs",
    "impl-plans/active/song-mode-sample-classification.md"
  ],
  "sharedPaths": [
    "src/pattern/eval.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/pattern/eval.rs",
      "intendedEdit": "Serial after verifiedQ: add default explicit query resolver capability; preserve current song state/legacy semantics.04consumes without editing eval."
    }
  ]
}
```

## Related plans and dependencies

- **Previous / Depends On**: [SONG-04Q](song-mode-source-provenance.md), independently verified.
- **Next**: [SONG-04](song-mode-query-edits.md), author may continue disjoint files but final seal waits verified classification.

| Dependency | Required output | Status |
|---|---|---|
| SONG-04Q | Existing bounded song query state and typed source origin | COMPLETE |

## Execution and preservation contract

Use specialized rust-coding and mandatory check-and-test-after-modify agents.
Only three Rust paths and own plan may change. Fresh reads and immutable before/
after SHA-256 intents belong under tmp/song-mode-riela/SONG-04R. Compare baseline
before writing; preserve unrelated work and reconcile drift. No dependencies,
commits, broad formatting or lowered limits. Every touched Rust file below1000
lines; split only after explicit serial manifest amendment. Author gates must
reach terminal exit with exact source seal before independent checker verification.
Root owns shared index/archive reconciliation.

## Modules and declarations

```rust
pub trait QueryVm {
    fn song_sample_backed(&mut self, sound: &Sound) -> Result<bool, Failure>;
}
```

This is an addition to the existing trait, with a compatibility-preserving default
explicit diagnostic when classification is unavailable. Preserve all other methods.
No Builtin/Inst approximation is allowed. Concrete VmQuery consults its actual
InstResolver::route; Audio returns sample.is_some(), nonaudio returns an explicit
song capability/type failure. This is an immediate query capability; snapshot
freezing still belongs06/07. A fixed library fixture may implement the capability
from its own declared immutable routing table.

| Module | Deliverable | Status |
|---|---|---|
| pattern/eval.rs | Default song-only route classification capability | IMPLEMENTED |
| vm/query_vm.rs | Actual registry audio-resource classification | IMPLEMENTED |
| tests/song_sample_classification.rs | Concrete resolver and unavailable capability fixtures | IMPLEMENTED |

## Tasks

### TASK-001: Query capability
**Status**: Completed
**Parallelizable**: No
- [x] Default diagnostic preserves legacy QueryVm implementations and behavior.
- [x] Concrete VmQuery resolves actual Audio resource presence without retained mutable aliases.

### TASK-002: Behavioral classification
**Status**: Completed
**Parallelizable**: No
- [x] Registered builtin non-sample synth returns false; resource-backed Inst returns true.
- [x] Bank/path/buffer resource cases and unknown/nonaudio routes have exact outcomes.
- [x] Existing traces/context/depth/query behavior remains compatible.

### TASK-003: Verify and seal
**Status**: Completed
**Parallelizable**: No
- [x] Native/host-wasm/scoped format/strict Clippy plus nonzero fixtures terminal exits recorded.
- [x] Exact hashes and independent checker approval recorded; archive deferred.

## Completion criteria

- [x] Sound tags never substitute for actual Audio.sample classification.
- [x] Concrete route fixtures prove registered Builtin and resource-backed Inst parity; missing capability fails explicitly.
- [x] No external-output side effect or mutable registry alias escapes through this query method.
- [x] Existing native/host-wasm builds, legacy fixtures, format and Clippy pass; all files below1000lines.
- [x] Independently verified current repair source seal retained.
- [x] Actual n chord expansion, numeric type preservation, explicit note precedence and bank-index behavior remain mandatory SONG-04 integration fixtures.

## Verification

All Cargo commands use CARGO_TERM_QUIET=true. Run nonzero song_sample_classification
fixtures, affected song context/legacy pattern tests, native check, host-wasm check,
scoped rustfmt and all-target Clippy -D warnings. Nextest uses required fail/immediate-
final/no-progress variables. Record actual commands, counts, terminal exits and full
logs. Compilation alone does not prove the classification boundary.

## Progress log

### Session: 2026-10-01 — source-grounded parity prerequisite
Checker and author identified Sound-tag approximation as insufficient. Root read
commit::notes and Audio route resource handling. No Rust changes/tests claimed.

### Session: 2026-10-01 — author implementation
Implemented only the three owned Rust files. QueryVm defaults to explicit
HostUnavailable classification; VmQuery reads actual InstResolver::route and
returns Audio.sample.is_some(). Missing resolver, unknown instrument and nonaudio
routes report explicit errors; no Sound-tag approximation, I/O, resource install
or retained mutable alias was added. Actual n/explicit-note/chord integration stays
owned by SONG-04; the classification capability is available to that author.

First public route fixture run: retained session 27621 terminal exit 0, 9 passed,
`tmp/song-mode-riela/SONG-04R/tests-first.log`. Serial author batch session 51646
terminated at Clippy exit 101; ledger `author-check-results.json` in that directory.
Route tests 9, song context tests 6, legacy pattern tests 64, VM query-mode tests 7,
native cargo check and host-wasm cargo check all exited 0. Exact commands/logs are
recorded in the ledger. All Cargo commands used CARGO_TERM_QUIET=true via mise.
The sole Clippy finding is the parallel SONG-04 test loop in tests/song_query.rs:233;
its owner was notified. No out-of-manifest fix performed; final Clippy/nextest seal
is pending that owner correction. Rust sizes: eval.rs 852, query_vm.rs 108, test208.
Immutable own source intent/post hashes are 0001-route-classification{,-after}.json.
Independent checker approval, final hash reconciliation and archive remain pending.

### Session: 2026-10-01 — terminal author seal
The parallel SONG-04 owner corrected its own loop lint; no R source repair needed.
Final retained session 90282 was polled to terminal exit 0: all-target strict Clippy
and nextest route fixtures (9 passed) both exited 0. Full ledger/logs:
`tmp/song-mode-riela/SONG-04R/author-sealed-check-results.json`,
`clippy-sealed.log`, `nextest-sealed.log`. The previous failure remains retained.
Together with the earlier terminal ledger, every required author Cargo gate passed.
Scoped rustfmt check and git diff check exited 0, recorded by `author-source-seal.json`.
All three Rust source hashes reconcile with initial post-edit intent; final own
source/plan hashes are in `0003-author-seal-after.json`. No Cargo sessions remain.
Independent mandatory checker approval is pending. Actual n chord expansion,
fractional notes, explicit-note precedence and bank-index integration remain
mandatory SONG-04 tests; R proves their route-classification capability only.

### Session: 2026-10-01 — independent verification
Mandatory checker approved `/tmp/vactr-song04r-checker-results.json`: 86 distinct
fixtures and nextest 9 repeat passed; all 10 gates exited 0, including native,
host-wasm, scoped format, strict Clippy and diff checks. All three source hashes
matched before/after; largest file 852 lines. Terminal session 13381 exited 0.
Full logs `/tmp/vactr-song04r-checker/*.log`. R classification boundary is complete;
actual chord/n/note bank-index behavior remains SONG-04 integration evidence.
Archive/index reconciliation remains root-owned.

### Session: 2026-10-01 — noncloning producer-length repair
Initial classification gates independently passed. A narrow additional accessor in
already owned pattern/eval.rs is now required for SONG-04 identity work admission:
`pub(crate) fn producer_len(&self) -> Option<usize>` reads the active trace length
without cloning or mutating it. This allows charging inherited QState work BEFORE
producer() allocation. Legacy/untraced returns None; traced/song contexts report
exact current length. Existing three Rust paths remain unchanged. Add focused
accessor/trace restoration evidence, fresh intent/seal and mandatory independent
verification. Prior initial proof remains historical; this repair is not complete
until current source and gates are verified.

### Session: 2026-10-01 — accessor author seal
Added the approved noncloning producer_len accessor in the existing eval.rs only.
It returns None for untraced state or the current usize length for traced/song
state using an immutable borrow. One focused fixture covers all three contexts,
read-only repeated access, eight pushes/pops, nested scope restoration, unchanged
producer data and unchanged work/depth/reentry/fault state. SONG-04 consumes the
length before copying its prefix; no default trace behavior changed.

Current final author batch terminal session 95462 exit 0; every gate exited 0.
Ledger `tmp/song-mode-riela/SONG-04R/author-repair-check-results.json` records exact
quiet Cargo commands and full `*-repair.log` paths: length fixture 1, route fixtures
9, song context 6, legacy pattern 64, VM query-mode 7, native check, host-wasm check,
all-target strict Clippy and nextest route fixtures 9. Initial focused session
23261 also reached exit 0. Scoped format/diff checks and final exact source hashes
are recorded in `author-repair-source-seal.json` and `0006-accessor-author-seal-after.json`.
Largest file eval.rs is 902 lines. No foreground Cargo sessions remain. Current
repair is author complete, mandatory independent verification pending; historical
classification approval remains recorded above. Archive/index remain deferred.

### Session: 2026-10-01 — independent accessor repair verification
Mandatory checker approved `/tmp/vactr-song04r-repair-checker-results.json`: 87
distinct tests and nextest 9 repeat passed; all 11 gates exited 0, retained checker
session 5975 terminal exit 0. All three current source hashes matched the repair
seal before/after, largest file 902 lines. Accessor scope/counter fixture passed;
source review confirms immutable noncloning/no-allocation access. Full logs:
`/tmp/vactr-song04r-repair-checker/*.log`. R is again Completed for its exact
current sources, including the accessor amendment. Archive/index remain root-owned.
