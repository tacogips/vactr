# Song source provenance and canonical context implementation plan

**Status**: Completed
**Plan ID**: SONG-04Q
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance)

## Intent and source evidence

Existing transforms carry ordinary pattern Event values. SongEvent routing alone
cannot survive converting a selected source into Event and reconstructing it with
segment. Typed immutable source provenance preserves original instrument family,
private route and already expanded tone identity through these transforms.
Dynamic step values currently resolve at the clipped piece onset; a slow sustained
note can consequently change its source payload across continuation windows.
Song-only canonical context samples dynamic step leaves at intrinsic whole onset.
Nested selected sources inherit the caller SongLimits and existing work/depth
state. Legacy query behavior remains unchanged.

## Manifest

```json
{
  "planId": "SONG-04Q",
  "planPath": "impl-plans/active/song-mode-source-provenance.md",
  "dependsOn": [
    "SONG-04P"
  ],
  "writePaths": [
    "src/song/source.rs",
    "src/pattern/query.rs",
    "src/pattern/eval.rs",
    "src/pattern/step.rs",
    "src/pattern/combinators/time.rs",
    "src/pattern/combinators/input.rs",
    "src/pattern/combinators/sound.rs",
    "tests/song_source_provenance.rs",
    "impl-plans/active/song-mode-source-provenance.md"
  ],
  "sharedPaths": [
    "src/song/source.rs",
    "src/pattern/query.rs",
    "src/pattern/eval.rs",
    "src/pattern/step.rs",
    "src/pattern/combinators/time.rs",
    "src/pattern/combinators/input.rs",
    "src/pattern/combinators/sound.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/source.rs",
      "intendedEdit": "Serial prerequisite after verified SONG-04P. No SONG-04 writes until independently verified SONG-04Q."
    },
    {
      "path": "src/pattern/query.rs",
      "intendedEdit": "Serial prerequisite after verified SONG-04P. No SONG-04 writes until independently verified SONG-04Q."
    },
    {
      "path": "src/pattern/eval.rs",
      "intendedEdit": "Serial prerequisite after verified SONG-04P. No SONG-04 writes until independently verified SONG-04Q."
    },
    {
      "path": "src/pattern/step.rs",
      "intendedEdit": "Serial prerequisite after verified SONG-04P. No SONG-04 writes until independently verified SONG-04Q."
    },
    {
      "path": "src/pattern/combinators/time.rs",
      "intendedEdit": "Serial prerequisite after verified SONG-04P. No SONG-04 writes until independently verified SONG-04Q."
    },
    {
      "path": "src/pattern/combinators/input.rs",
      "intendedEdit": "Serial prerequisite after verified SONG-04P. No SONG-04 writes until independently verified SONG-04Q."
    },
    {
      "path": "src/pattern/combinators/sound.rs",
      "intendedEdit": "Serial prerequisite after verified SONG-04P. No SONG-04 writes until independently verified SONG-04Q."
    }
  ]
}
```

## Related plans and dependencies

- **Previous / Depends On**: [SONG-04P](song-mode-source-contracts.md), independently verified before writes.
- **Next**: [SONG-04](song-mode-query-edits.md), starts after this phase is independently verified.

| Dependency | Required output | Status |
|---|---|---|
| SONG-04P | Immutable selected source, typed route, exhaustive dispatch | COMPLETE |

## Execution and preservation contract

Use the repository rust-coding agent and mandatory check-and-test-after-modify
agent. Implementation is authorized by the active song-mode goal. Use the current
workspace; preserve unrelated dirty files and do not commit, add dependencies or
run broad formatting. Read fresh exact sources and record immutable pre/post
SHA-256 intents under tmp/song-mode-riela/SONG-04Q before every batch. Compare
baselines immediately before writes; reconcile drift rather than restoring stale
copies. Only the eight Rust paths in the manifest may change. Additional matches
or splits require a serial manifest amendment. Every touched Rust file stays below
1000 lines. Author checks must reach terminal exit before readiness; independent
checker verification follows the sealed author batch. Update only this own plan;
shared index/archive reconciliation remains root-owned.

## Modules and declarations

```rust
pub struct SongEventOrigin {
    pub handle: EventHandle,
    pub original_instrument: Sound,
    pub route: Option<InstrumentRoute>,
    pub commit_mode: NoteCommitMode,
}
pub struct Event { pub song_source: Option<Rc<SongEventOrigin>> }
// QState remains crate-private; signatures are prerequisite additions only.
pub(crate) fn new_song<'c, 'a>(cx: &'c mut QueryCtx<'a>, limits: SongLimits) -> Result<QState<'c, 'a>, Failure>;
```

Declaration blocks list additions only. Preserve existing fields. Constructor may
be an associated method. The original handle already contains full source track,
placement, occurrence and tone (read handle.tone()); do not replace that identity with a hash or a
filtered-result index. Current note controls remain transformable. Final edited
Part realization reissues its root revision handle while retaining source origin.
The context is not a transitive VM snapshot; that remains SONG-06/07.

| Module | Deliverable | Status |
|---|---|---|
| source.rs | Immutable typed origin contract via public source module | IMPLEMENTED |
| query.rs / step.rs | Optional Event origin, default absent on fresh events | IMPLEMENTED |
| eval.rs / step.rs | Song-only canonical onset policy and inherited limits, same counters | IMPLEMENTED |
| time.rs / input.rs / sound.rs | Preserve typed origin when rebuilding/sampling existing content Event | IMPLEMENTED |
| tests/song_source_provenance.rs | Fresh-event and public contract fixtures; Some-origin reconstruction tested in owned crate-local modules | IMPLEMENTED |

## Tasks

### TASK-001: Typed provenance
**Status**: Completed
**Parallelizable**: No
- [x] Immutable origin and Event field compile without reserved controls.
- [x] Constructor defaults absent; sampled content reconstruction preserves origin.

### TASK-002: Canonical context
**Status**: Completed
**Parallelizable**: No
- [x] Song dynamic step resolution uses intrinsic whole onset.
- [x] Nested source can read identical caller limits without resetting counters.
- [x] Legacy traced and untraced queries retain their previous sampling behavior.

### TASK-003: Verify and seal
**Status**: Completed
**Parallelizable**: No
- [x] Unit/public fixtures, legacy patterns, native/wasm, scoped format, Clippy terminal logs.
- [x] Independent checker approval, final hashes and accurate own progress.

## Completion criteria

- [x] Slow-four dynamic leaf returns stable instrument/control payload for continuations in different integer windows under song context; paired legacy test proves unchanged behavior.
- [x] Original family, route and tone identity survive segment reconstruction and ordinary cloned/mutated transforms.
- [x] Strict caller SongLimits remain visible through nested context; no default substitution, new QState or budget/depth reset.
- [x] Depth 200 succeeds and depth 300 fails explicitly under existing limits; no larger stack or reduced limits.
- [x] Every Rust file is below 1000 lines; all owned checks pass with retained terminal logs and independent verification.
- [x] This prerequisite does not claim full realization/playback or frozen transport state.

## Verification commands

Use CARGO_TERM_QUIET=true for all Cargo commands. Run scoped public/unit fixtures,
existing pattern tests, cargo check, host-wasm cargo check and all-target strict
Clippy. Use NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
NEXTEST_HIDE_PROGRESS_BAR=1 for any nextest invocation. Record actual commands,
selected nonzero fixture counts, exit codes and log paths.

## Progress log

### Session: 2026-10-01 — source-grounded prerequisite
Root inspected step_events, Event::new, segment reconstruction and QState. No Rust
changes or verification results claimed by this plan creation.

### Session: 2026-10-01 — sampled sound metadata ownership
The public source module already exports the origin type by module path. Replace
redundant mod.rs ownership with sound.rs to preserve typed metadata when a callable
sound parameter returns a selected source pattern. Keep the eight-module bound.
The certified tone ordinal is read from the opaque source handle, not duplicated.

### Session: 2026-10-01 — opaque-handle fixture boundary
EventHandle has no public constructor and actual public issuance starts in SONG-04.
Some-origin reconstruction fixtures therefore live in the owned crate-local
modules and use existing crate-only issuance. External tests cover fresh None and
public metadata behavior without forging handles. SONG-04 must prove actual public
issuance and preservation through selected-source transforms. No interim dispatch
success or new public forging API is authorized.

### Session: 2026-10-01 — author seal
Implemented the exact eight Rust paths after independently verified SONG-04P.
The source origin carries the certified handle, original instrument, typed route
and Mono marker; tone comes only from the handle. Fresh Event constructors set
None. Segment/input sampling carries the origin through reconstruction. Callable
Sound samples preserve current fractional mono note, gain, source reference and
late/cell metadata when origin is Some; paired None-origin sampling retains its
existing control-free legacy behavior. Ordinary event cloning/mutation retains
the shared immutable origin. No public opaque-handle issuance API was added.

`QState::new_song` validates supplied limits and returns Result rather than allowing
invalid depth policy; this checked declaration refinement was approved by root.
It retains the exact copied limits, tracing and existing work/depth/reentry state.
Dynamic step leaves use intrinsic whole onset only in song context. Legacy traced
and untraced queries retain clipped sampling. Canonical realization and actual
public selected-source issuance remain SONG-04 work; this phase does not certify
frozen VM state or usable playback.

Final serial author batch reached terminal session 47653 exit 0. Ledger:
`tmp/song-mode-riela/SONG-04Q/author-sealed-check-results.json`. Every Cargo command
used `CARGO_TERM_QUIET=true mise exec -- cargo`; full logs are in that directory:
- `cargo test --lib song_context_tests`: 6 passed (`context-sealed.log`).
- `cargo test --lib provenance_tests`: 5 passed (`provenance-sealed.log`).
- `cargo test --lib song_sound_depth_tests`: 1 passed (`sound-depth-sealed.log`).
- `cargo test --lib legacy_sampling_tests`: 1 passed (`legacy-sealed.log`).
- `cargo test --test song_source_provenance`: 4 passed (`public-sealed.log`).
- `cargo test --lib pattern::tests`: 64 passed (`pattern-sealed.log`).
- `cargo test --test song_trace_core --test song_trace_combinators`: 9+12 passed
  (`trace-sealed.log`), including depth-200/depth-300 fast/slow/reverse cases.
- `cargo test --test song_source_contracts`: 7 passed (`contracts-sealed.log`).
- Native cargo check, host-wasm cargo check and all-target strict Clippy each exited
  0 (`native-sealed.log`, `wasm-sealed.log`, `clippy-sealed.log`).
- Nextest selected song_source_provenance: 4 passed (`nextest-sealed.log`), with
  prescribed nextest quiet/status environment variables.

The first real point-sample budget test incorrectly expected two units from one
Pure point emission. Actual accounting spends one; only the fixture pre-spend was
corrected to leave one unit. Its second actual nested sample then raises
FuelExhausted with unchanged depth/reentry/exact limits. Failed evidence is retained
in `author-check-results.json`/`context-final.log`; no production spend change.

Scoped rustfmt check and git diff check passed; full logs `format-sealed.log` and
`diff-sealed.log`. All eight files are below 1000 lines (largest eval.rs 838).
Immutable final source hashes are recorded by `0008-author-seal-after.json`.
Independent mandatory checker approval is pending; archive/index remain root-owned.

### Session: 2026-10-01 — independent verification
Mandatory checker sealed `/tmp/vactr-song04q-checker-results.json`: 117 fixtures
and nextest 4 passed; all native/host-wasm/format/strict-Clippy/diff gates exited 0.
All eight owned Rust hashes matched the author seal. Every Q criterion is verified;
public canonical selected-source issuance remains SONG-04 evidence. Archive/index
reconciliation is deferred to root.
