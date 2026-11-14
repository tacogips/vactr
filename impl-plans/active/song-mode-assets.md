# Isolated song assets implementation plan

**Status**: Completed
**Plan ID**: SONG-06A
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Snapshot application](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Intent and source evidence

NativeSampleLoader clones share mutable file IDs/banks; WasmSamples clones share
the page map. SourceLoader may expose live analysis. Snapshot ownership therefore
needs a configuration-only factory producing isolated preparation handles, then
closed pinned asset lookup. Language paths originate in literals, not a string-to-
Path native. Candidate FnProto constants/nested prototypes/globals and closure
captures admit bounded graph traversal without querying the complete arrangement.
SessionConfig has one constructor literal in the already owned session.rs. Existing
native roots/path checks and sorted modulo bank behavior remain authoritative;
browser sample lookup remains exact-key based. SampleStore has 256 resource entries
(arena.rs MAX_RESOURCES), but actual free resources/arena bytes and retiring graphs
must determine admission, rather than assuming all entries are available.

## Manifest

```json
{
  "planId": "SONG-06A",
  "planPath": "impl-plans/active/song-mode-assets.md",
  "dependsOn": [
    "SONG-04"
  ],
  "writePaths": [
    "src/song/assets.rs",
    "src/song/mod.rs",
    "src/ns/load.rs",
    "src/host/native/loader.rs",
    "src/host/wasm/messages.rs",
    "src/session/session.rs",
    "src/host/wasm/session_half.rs",
    "tests/song_assets.rs",
    "impl-plans/active/song-mode-assets.md"
  ],
  "sharedPaths": [
    "src/song/assets.rs",
    "src/song/mod.rs",
    "src/ns/load.rs",
    "src/host/native/loader.rs",
    "src/host/wasm/messages.rs",
    "src/session/session.rs",
    "src/host/wasm/session_half.rs"
  ],
  "sharedPathNotes": [
    {
      "path": "src/song/assets.rs",
      "intendedEdit": "Asset prerequisite after verified04, disjoint from05; before06/07 and later browser integration; read fresh hashes, preserve legacy behavior."
    },
    {
      "path": "src/song/mod.rs",
      "intendedEdit": "Asset prerequisite after verified04, disjoint from05; before06/07 and later browser integration; read fresh hashes, preserve legacy behavior."
    },
    {
      "path": "src/ns/load.rs",
      "intendedEdit": "Asset prerequisite after verified04, disjoint from05; before06/07 and later browser integration; read fresh hashes, preserve legacy behavior."
    },
    {
      "path": "src/host/native/loader.rs",
      "intendedEdit": "Asset prerequisite after verified04, disjoint from05; before06/07 and later browser integration; read fresh hashes, preserve legacy behavior."
    },
    {
      "path": "src/host/wasm/messages.rs",
      "intendedEdit": "Asset prerequisite after verified04, disjoint from05; before06/07 and later browser integration; read fresh hashes, preserve legacy behavior."
    },
    {
      "path": "src/session/session.rs",
      "intendedEdit": "Asset prerequisite after verified04, disjoint from05; before06/07 and later browser integration; read fresh hashes, preserve legacy behavior."
    },
    {
      "path": "src/host/wasm/session_half.rs",
      "intendedEdit": "Asset prerequisite after verified04, disjoint from05; before06/07 and later browser integration; read fresh hashes, preserve legacy behavior."
    }
  ]
}
```

## Related plans and dependencies

- **Previous / Depends On**: [SONG-04](song-mode-query-edits.md), independently verified before writes. SONG-05 can run concurrently because its six Rust paths are disjoint.
- **Next**: [SONG-06](song-mode-snapshot-contracts.md), then [SONG-07](song-mode-candidate-evaluation.md).

| Dependency | Required output | Status |
|---|---|---|
| SONG-04 | Verified finite song core and immutable queries/edits | Completed |

## Execution and preservation contract

Use the mandatory rust-coding and check-and-test-after-modify agents and repository
Rust standards. Native filesystem/configuration work also follows supply-chain-
secure-code skill. No dependency additions, commits or broad formatting. Record
fresh immutable before/after SHA-256 intents under tmp/song-mode-riela/SONG-06A,
compare each baseline immediately before editing and reconcile drift. Only listed
paths may change; additional exhaustive integration or a file split requires a
serial manifest amendment first. Every touched Rust file stays below 1000 lines.
Preserve all unrelated dirty work. Author gates must reach terminal exit and source
seal before independent checker review. Update only this plan; archive/index moves
remain root-owned.

## Modules and declarations

```rust
pub struct SongAssetLimits {
    pub max_resources: u32,
    pub max_pcm_bytes: u64,
    pub max_source_files: u32,
    pub max_source_bytes: u64,
    pub max_banks: u32,
    pub max_walk_nodes: u32,
    pub max_walk_depth: u32,
}
pub struct SongSourceFile { pub file: FileId, pub path: PathVal }
pub enum SongAssetSelector { Path(PathVal), Bank(KwId) }
pub trait SongAssetFactory {
    fn begin(&self, source: SongSourceFile, limits: SongAssetLimits)
        -> Result<SongAssetPreparation, Failure>;
}
pub struct SongAssetPreparation { /* private candidate-only state */ }
pub struct PinnedSongAssets { /* private closed inventory */ }
impl SongAssetPreparation {
    pub fn source_loader(&self) -> Box<dyn SourceLoader>;
    pub fn sample_loader(&self) -> Box<dyn SampleLoader>;
    pub fn pin(&mut self, selection: &[SongAssetSelector]) -> Result<(), Failure>;
    pub fn pin_buffer(&mut self, buffer: &SampleBuf) -> Result<Rc<SampleBuf>, Failure>;
    pub fn close(self) -> Result<PinnedSongAssets, Failure>;
}
impl SampleLoader for PinnedSongAssets {
    fn load(&mut self, source: &SampleSrc) -> Result<Arc<SampleData>, Failure>;
}
```

Add an optional default-unavailable SourceLoader factory capability and an optional
SessionConfig factory field. Native exposes a factory that copies configuration
into fresh state before SessionLoader wraps it; browser setup supplies a detached
factory from decoded page assets. All constructor/integration changes stay within
the manifest. The candidate source/sample handles may share preparation state with
each other, but never active state. Factory configuration remains immutable.

| Module | Deliverable | Status |
|---|---|---|
| song/assets.rs + song/mod.rs | Portable limits, factory/preparation/closed lookup contracts and exports | Implemented |
| ns/load.rs | Optional factory capability; candidate loaders expose no analysis | Implemented |
| host/native/loader.rs | Fresh isolated configuration, frozen resolution and native inventory | Implemented |
| host/wasm/messages.rs | Detached decoded page inventory with explicit catalog completeness | Implemented |
| session/session.rs | Optional factory ownership/configuration captured independently | Implemented |
| host/wasm/session_half.rs | Supply browser detached factory without active page aliases | Implemented |
| tests/song_assets.rs | Isolation, closure, missing asset and capacity fixtures | 18 independently verified fixtures passed |

## Tasks

### TASK-001: Portable preparation boundary
**Status**: Completed
**Parallelizable**: No
- [x] Checked limits and private preparation/closed ownership compile.
- [x] Candidate source/sample handles share only candidate state.

### TASK-002: Native and browser adapters
**Status**: Completed
**Parallelizable**: No
- [x] Native copies roots/base configuration; candidate file IDs/banks/import bytes are isolated.
- [x] Browser detaches immutable decoded assets and requires explicit complete bank catalog.
- [x] Default absent capability gives an explicit diagnostic; no active loader fallback.

### TASK-003: Pin and close
**Status**: Completed
**Parallelizable**: No
- [x] Pin requested literal sample paths and complete referenced banks, separately from source/import paths.
- [x] Validate ready buffer rate/stereo alignment/finite PCM; return private state with captured immutable backing and reject pending/failed buffers.
- [x] Close disables source reads/bank registration; sample load becomes lookup-only and unknown resources fail explicitly.

### TASK-004: Verify and seal
**Status**: Completed
**Parallelizable**: No
- [x] Behavioral fixtures, native/host-wasm builds, scoped formatting and strict Clippy terminal logs.
- [x] Independent checker, exact source seal and own progress recorded.

## Completion criteria

- [x] Mutating active file/bank maps, disk bytes, page assets and original buffer PCM/rate after close cannot change candidate lookup.
- [x] Candidate source resolution has stable file/import identities and pinned bytes; no live analysis/taps.
- [x] Missing/outside-root paths, incomplete catalogs, closed source reads/registration and checked limit exhaustion produce explicit failures.
- [x] Candidate dependency walking uses bounded finite literals/prototypes/captures with cycle detection, not callback evaluation or arrangement-wide sampling; SONG-07 consumes this contract.
- [x] Unknown future host/custom-native resources receive closed-inventory capability failure; never live disk fallback or discarded-event success.
- [x] Actual resource/PCM budgets account for current and retiring generations; native has explicit memory budget, browser uses available arena bytes.
- [x] All eight Rust files below 1000 lines, all selected fixtures nonzero, native/host-wasm/format/Clippy pass with independent evidence.
- [x] This phase supplies asset ownership only; snapshot freezing and host acknowledgments remain required later.

## Verification

All Cargo commands use CARGO_TERM_QUIET=true. Run song_assets fixtures, affected
legacy loader/session tests, native check, host-wasm check and all-target strict
Clippy. Nextest uses NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
NEXTEST_HIDE_PROGRESS_BAR=1. Retain actual commands, nonzero counts, terminal exits,
full logs and source hashes. Browser detached-map behavior needs actual fixtures;
compilation alone does not prove isolation.

## Progress log

### Session: 2026-10-01 — source-grounded bounded prerequisite
Read-only specialized-agent preflight and root reads establish factory/isolation
gaps and exact eight-module ownership. No Rust changes or test results claimed.

### Session: 2026-10-01 — complete browser catalogs and factory refresh
Bank selectors require an explicit complete catalog; no undeclared index-selection
exception is authorized. Browser lookup stays exact-key based. Session initialization
has an empty WasmSamples map, so freezing that factory once would miss later uploads.
Add a Session setter for an optional factory within owned session.rs; sample ingress
and candidate application replace it with a NEW immutable detached page snapshot.
Prepared candidates retain their prior immutable inventory and never observe later
page mutation. Owned session_half.rs supplies this refresh and explicit catalog
capability. An absent/incomplete catalog fails preparation instead of certifying
partial page data as complete. SONG-14 wires browser sample/catalog upload helpers.

### Session: 2026-10-01 — disjoint asset implementation scheduling
Source-grounded preflight found no dependency on native language wrappers or staging.
SONG-06A consumes existing song/core, source/sample loader, host and session APIs.
Its eight Rust paths are disjoint from SONG-05. Begin after independently verified
SONG-04; SONG-06 still waits both SONG-05 and SONG-06A. Neither phase claims playback
readiness. Coordinate joined workspace compile checks after concurrent mutation
batches settle; sealed independent review remains mandatory for each phase.

### Session: 2026-10-01 — authorized isolated asset implementation
Fresh immutable TASK-001-intent-001 records all eight Rust baselines and design/plan hashes. Implement configuration-only factories, candidate-only shared loaders, checked closed inventories and private buffer state copies. No snapshot readiness or host installation is claimed; those remain later phases.

### Session: 2026-10-01 — concrete adapter and discovery evidence
Portable/native/browser factory adapters compile; focused003 passes 16 nonzero fixtures. The bounded dependency visitor distinguishes ambiguous literal paths, known sample paths, sound-keyword candidates, private-ready-buffer candidates, live signal/external sound flags and native/instrument candidates. It never calls callbacks or expands repeats. SONG-07 classifies ambiguous dependencies with its candidate-only registry; unresolved dynamic/custom resources fail explicitly, with no closed-inventory disk fallback.

SongAssetLimits accepts remaining host capacity. after_reservations uses checked current+retiring resource/PCM subtraction. Browser HostState exposes its authoritative sample reservations until retirement acknowledgment; graph reservations are an additional host-preparation obligation. Native budget/residency inputs remain explicit. SONG-08/09/10 must prove actual combined native/browser generation capacity before Ready; this phase does not claim automatic native accounting or all 256 resources free.

WasmSamples delegates to the shared portable DecodedSongAssetFactory tested by native fixtures. Session sample ingress, explicit complete bank-catalog update and pre-candidate refresh create NEW detached factories; old preparations retain their prior inventory. Missing catalogs fail rather than infer completeness from partial page keys.

### Session: 2026-10-01 — author sources sealed, independent review pending
TASK-004-checks-001 records all ten terminal gates at exit 0; foreground 60851 is terminal. Native and host-wasm check, all-target strict Clippy, scoped rustfmt and diff checks passed. Focused assets 18 + legacy WAV 13 + source load 11 + session packages 7 = 49 distinct passing fixtures; nextest repeats the 18 assets fixtures and is not additional distinct coverage. Largest touched Rust file is assets.rs at 868 lines. TASK-005-post-001 records exact eight source hashes and plan hash. All author processes are closed; no further Rust writes are planned before an explicit independent finding.

Native supplied-code origins accept unsaved filenames only inside a canonical contained parent, console origins use configured base, and actual missing/outside reads still fail. Canonical source aliases retain pinned bytes and file IDs. close drops the candidate backend even while revoked loader handles survive, releasing unselected page inventory and source cache. Buffer replacements are memoized by original ID and have independent rate/readiness; PCM lookup is immutable. Native banks retain modulo indexing and browser banks require exact declared indices.

Initial early native check001 failed on the erroneous Fuel enum spelling and is retained at /tmp/vactr-song06a-early-check-001.log; it was corrected to FuelExhausted before all passing final gates. Prior type-complexity diagnostics reported by the concurrent SONG-05 author were corrected with source inventory aliases; final strict Clippy is green. Independent clearance remains mandatory, and this phase does not claim frozen whole-VM snapshots or playback readiness.

### Session: 2026-10-01 — mandatory independent clearance and phase completion
Independent evidence /tmp/vactr-song06a-independent-001/final-results.json confirms all ten gates exit 0, retained foreground 23393 terminal 0 with no active processes, 49 distinct fixtures and 18 nextest repeats. All eight pre/post source SHA-256 values match the author seal; largest file remains 868 lines. Native/host-wasm checks, all-target strict Clippy, scoped formatting and diff checks are clear. No source finding or checker edits remain. Tasks, module deliverables and phase criteria are Completed. TASK-006 immutable intent/post records preserve this doc-only completion; Rust sources are unchanged by this update. Archive/index movement remains root-owned and deferred.

Completion certifies this isolated asset boundary only. SONG-06/07 still must freeze and certify the complete candidate VM/value graph and classify ambiguous dependency candidates. SONG-08/09/10 still must measure and reserve actual native/browser current+retiring graph/resource capacity before Ready. Browser catalog/upload JavaScript wiring remains SONG-14; existing explicit complete-catalog/refresh hooks do not claim that later end-to-end transport is finished. Host acknowledgments, playback and export remain separate obligations.

### Session: 2026-10-01 — confirmed lazy sound keyword discovery repair
A read-only compiled probe `s {t -> :candidate-bank}` produces a sound PParam::Fn
whose prototype contains a Keyword constant. Generic dependency traversal ignored
that constant, so the previously sealed discovery omitted its possible bank. Add
ambiguous literal_keywords discovery, distinct from known sound_keywords, for
constants/captures/globals without evaluating callbacks. Verify the actual compiled
lazy source and an ordinary metadata keyword remain separately classified.
SONG-07 resolves candidates through its frozen registry/context; do not treat every
keyword as a bank. Repair only existing assets.rs/tests/song_assets.rs plus this
plan; no additional path. Fresh intents/gates/seal and mandatory independent
revalidation precede final SONG-06 join. Earlier proof remains historical and valid
for its original scope; this discovered missing edge is not called complete.
- [x] Lazy sound keyword constants are discovered without callback evaluation.
- [x] Metadata keywords remain ambiguous until candidate registry classification.
- [x] Repaired sources receive independent checks and exact hash clearance.

### Session: 2026-10-01 — narrow keyword repair author seal
Generic Value::Keyword traversal now appends ambiguous literal_keywords, separately from known sound_keywords. This covers prototype constants, captures and referenced global values without resolving their eventual sound meaning. Actual compiled `s {t -> :candidate-bank}` and named lazy source fixtures discover the keyword; a panic-on-call native inside the named callback proves it is not forced. A paired compiled metadata list remains ambiguous, while directly known sound keywords retain their separate collection. SONG-07 must classify ambiguous candidates through its isolated registry/context; neither this field nor this phase certifies all metadata keywords as banks.

TASK-007-checks-001 records all ten terminal author gates at exit 0; foreground 58107 terminal 0 and all author processes closed. Assets 20 + legacy WAV 13 + source load 11 + packages 7 = 51 distinct fixtures; nextest repeats 20, not additional distinct coverage. Native/host-wasm checks, strict all-target Clippy, scoped formatting and diff checks pass. The exact repaired files are assets.rs 871 lines and song_assets.rs 827 lines. TASK-008-post-001 seals these two hashes; the prior independent eight-source proof remains historical and other sources are untouched by this repair. Independent revalidation is pending before final phase join. No further source edits are planned absent an actual finding.

### Session: 2026-10-01 — narrow repair independent clearance
Immutable independent evidence /tmp/vactr-song06a-repair-independent-001/final-results.json confirms all ten gates exit 0, retained 46805 terminal 0 with no active processes, 51 distinct fixtures plus 20 nextest repeats, and both repaired source pre/post hashes matching TASK-008-post-001. Largest repaired source is 871 lines. The actual compiled lazy keyword, metadata separation and no-callback-execution boundary were independently reviewed without remaining findings. The asset plan and repair criteria are Completed again; TASK-009 records this doc-only status update. No Rust, archive/index or Git changes occur here.

The original eight-source independent proof remains historical, alongside this focused two-source repair clearance. Authorized SONG-06 shared-source changes are expected and must not be reverted to that earlier seal. Whole-candidate freezing/classification remains SONG-06/07, measured current+retiring host graph/resource admission remains SONG-08/09/10, browser JavaScript catalog/upload transport remains SONG-14, and host acknowledgments/playback/export remain later work. Phase completion does not certify those later obligations.

### Session: 2026-10-01 — measured closed PCM usage repair
Audio preparation cannot admit required PCM bytes from resource_count alone.
Reopen narrowly for assets.rs and tests/song_assets.rs only: copy the preparation's
checked charged PCM total into PinnedSongAssets at close and expose a read-only
pcm_bytes() scalar. No backend/PCM mutation access is added. Duplicate pinning
must not double charge; source text bytes remain separate. Empty inventories
report zero, and later source/buffer mutation cannot change closed usage.
SONG-07 may proceed concurrently on its disjoint eight paths, but final join
waits independent re-clearance of this exact repair. Preserve historical proofs
and all authorized shared-source changes. No dependency or Git edits.
- [x] Closed measured PCM bytes are exposed as immutable scalar metadata.
- [x] Empty, duplicate, mixed resource and stable-after-mutation usage fixtures pass.
- [x] Exact two-source repair independently cleared with hashes and terminal gates.

### Session: 2026-10-01 — closed PCM total repair, joined lint pending
ROOT0031 implementation copies Preparation.pcm_bytes into the closed inventory and exposes read-only pcm_bytes(), preserving existing charged-resource semantics. The mixed path/bank/buffer fixture proves duplicate pins do not double-charge, source text does not count toward PCM, and later original buffer mutation does not change the stored total or closed lookup. Assets21 + legacy WAV13 + load11 + packages7 =52 distinct passing fixtures; nextest repeats21.

TASK-010-checks-001 records nine passing gates and strictClippy exit101. The only lint errors are concurrently introduced SONG-07 PartPayloadMapper/freeze_payloads scaffolding in part.rs; the asset author did not change that outside-owned code. Failed log author-pcm-clippy-001.log is retained. Foreground38612 is terminal0 and no author processes remain. The SONG-07 author has connected that helper and is completing routing inventory integration; wait for its stable compile/lint window before rerunning gates. No final repair seal or independent success is claimed yet.

### Session: 2026-10-01 — measured PCM repair author seal
TASK-010-checks-002 records all ten author gates at exit 0 after the concurrent SONG-07 stable compilation window. Retained foreground 61072 is terminal 0 and all author processes are closed. Assets 21 + legacy WAV 13 + load 11 + packages 7 = 52 distinct fixtures; nextest repeats 21, without adding distinct coverage. Native/host-wasm checks, strict all-target Clippy, scoped formatting and diff checks pass. The earlier strict lint failure remains retained in author-pcm-clippy-001.log and is resolved by the other author connecting its helper; no outside-owned source was modified here.

TASK-012-post-001 seals only assets.rs (878 lines) and song_assets.rs (886 lines). The original eight-source proof and the previous literal-keyword two-source proof remain historical. This repair adds copied immutable charged PCM metadata without changing resource charges, duplicate behavior, source-byte accounting or closed lookup. Independent exact two-source revalidation is pending; no final phase completion, host capacity admission or Ready state is claimed. No further source writes are planned absent an actual independent finding.

### Session: 2026-10-01 — measured PCM repair independent completion
ROOT0031 mandatory evidence /tmp/vactr-song06a-pcm-independent-001/final-results.json confirms all ten gates exit 0, 52 distinct fixtures plus 21 nextest repeats, retained foreground 7179 terminal 0 and no active processes. Both repaired source pre/post SHA-256 values match TASK-012-post-001 exactly; maximum source size is 886 lines. Independent read-only review verifies immutable measured PCM metadata, duplicate charging, stability after original mutation and separation from source text bytes. No source finding remains. TASK-013 records this doc-only Completed update; Rust sources, index, archive and Git remain unchanged.

The original eight-source proof and narrow literal-keyword proof remain historical for their respective scopes. This current two-source clearance certifies copied charged PCM usage only. Whole-candidate certification remains SONG-07; actual current+retiring native/browser graph/resource admission remains SONG-08/09/10; browser JavaScript complete-catalog transport remains SONG-14. No Ready, host activation, playback or export claim follows from this asset completion. SONG-08 implementation remains gated on independent SONG-07 clearance.

### Session: 2026-10-01 — exact dependency work scalar
SONG-07 inventory invokes the existing exhaustive dependency walker for payloads
and family roots. The current output omits consumed work, preventing exact
aggregate admission across calls. Reopen only assets.rs/tests/song_assets.rs plus
this plan: retain DependencyWalk.nodes in a private output scalar after successful
traversal and expose read-only consumed_work() -> u32. Count actual enter calls,
including repeated alias checks; empty roots report zero, symbolic repetition
counts do not expand. Failure returns no claimed successful count. Do not change
existing traversal/classification, limits, no-callback-execution or shared-node
behavior. Verify exact small/shared counts, non-expanded large repeats and bounded
failures. Fresh exact two-source author seal and independent checks are required.
- [x] Successful dependency output exposes exact immutable consumed work.
- [x] Empty/shared/repeated symbolic/budget failure accounting fixtures pass.
- [x] Exact two-source repair independently cleared and all processes terminal.

### Session: 2026-10-01 — exact dependency work author seal
ROOT0033 copies the existing DependencyWalk.nodes counter into a private successful-output scalar and exposes read-only consumed_work(). No traversal, classification, count increment, depth/work budget or alias behavior changes. Exact fixtures prove empty 0, scalar 1, two-element list 3, shared-list roots 4, and silent symbolic repeats 1 versus one billion both 3. Exhaustion still returns FuelExhausted without a result. Existing fixtures are retained.

TASK-014-checks-001 contains ten author gates at exit 0, retained process 70387 terminal 0 with no active author processes. Assets 22 + legacy WAV 13 + load 11 + packages 7 = 53 distinct fixtures; nextest repeats 22. Native/host-wasm checks, strict all-target Clippy, scoped formatting and diff checks pass. TASK-015-post-001 seals only assets.rs (888 lines) and song_assets.rs (931 lines); independent exact-two-source clearance remains pending. Historical eight-source, keyword and PCM proofs retain their earlier scopes. No further Rust writes are planned absent an actual finding; later candidate, capacity, browser transport and activation obligations remain unchanged.

### Session: 2026-10-01 — dependency work repair independent completion
ROOT0033 mandatory evidence /tmp/vactr-song06a-work-independent-001/final-results.json confirms ten gates exit 0, 53 distinct fixtures plus 22 nextest repeats, terminal session 55445 exit 0 and no active processes. Both current repaired source hashes match TASK-015-post-001 before and after verification; maximum source size is 931 lines. Independent review confirms consumed_work exposes actual successful DependencyWalk.nodes without traversal changes or backend aliases. TASK-016 records this doc-only Completed update; no Rust, index, archive or Git changes occur.

Historical original eight-source, literal-keyword and measured-PCM evidence retains each earlier scope. Authorized SONG-07 shared-source drift is preserved. This scalar enables exact aggregate candidate metadata admission without claiming whole-candidate certification, actual host current+retiring capacity, browser catalog transport, Ready activation, playback or export. Those later obligations remain unchanged; SONG-08 implementation awaits independent SONG-07 clearance.
