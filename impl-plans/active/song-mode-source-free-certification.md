# Source-free configuration certification implementation plan

**Status**: Completed
**Plan ID**: SONG-07F
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Routing and tails](../../design-docs/specs/design-song-mode.md#routing-and-tails) and [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance)

## Intent and actual evidence

The actual SONG-08B long-source Iterate route fixture fails during preparation,
before route resolution. A captured ordinary slow pattern has no selected-source
dependencies, but Certification::part_inner traverses its timing graph and rejects
Slow DynamicRate. Such a captured track still owns one finite configuration. The
same unconditional traversal occurs for generated Replace/Overwrite/Transform
payloads. Failure is retained in SONG-08B/author-actual-iterator-route-001.log
(author handle 71678, terminal 101).

This is a source-grounded supporting refinement of the accepted design, not a new
Riela review. Empty dependency lists do not prove public DTO graph validity.
Validate dependency-free structure before excluding irrelevant ordinary timing
from selected-source configuration counting. Full playback, Apply/mute, transport,
export and browser/editor requirements remain unchanged.

## Related plans

- **Previous / Depends On**: [Weighted source layout](song-mode-weighted-source-layout.md), independently cleared SONG-07E.
- **Next / Consumer**: [Route preparation](song-mode-route-preparation.md), SONG-08B in progress.
- **Following**: Parent SONG-08 joined clearance, then SONG-09 actual DSP playback.

## Exact manifest and ownership

```json
{
  "planId": "SONG-07F",
  "planPath": "impl-plans/active/song-mode-source-free-certification.md",
  "writePaths": [
    "src/song/source_uses.rs",
    "src/song/source_uses/source_free.rs",
    "tests/song_source_free.rs",
    "impl-plans/active/song-mode-source-free-certification.md"
  ],
  "sharedPaths": [],
  "ownershipNotes": "rust_coding owns these three Rust paths and own plan after root release. SONG-08B author owns disjoint routing paths and reads this interface."
}
```

Fresh immutable numbered SHA intents precede each batch under
tmp/song-mode-riela/SONG-07F/. Preserve failed logs and poll every live handle to
terminal; observation timeout is not permission to restart. No dependencies,
lockfiles, broad formatting, Git, index or archive changes. Do not restore cleared
07E files. Any extra path requires prior root amendment. Each touched Rust file
must remain below 1000 lines; the new cohesive private helper provides room.

## Deliverables and contract

| Path | Deliverable | Status |
|---|---|---|
| src/song/source_uses.rs | Integrate ordinary source-free payload certification in owned Capture and generated Edit counting | Completed |
| src/song/source_uses/source_free.rs | Private bounded structural validation with shared certification admission | Completed |
| tests/song_source_free.rs | Public certification fixtures covering genuine ordinary timing and adversarial DTOs | Completed |

No public API change is required. Record exact private declarations before coding.
Source-free validation accepts ordinary Uncertifiable timing only after proving
the relevant graph has no selected Source dependency. It must reject invalid
roots/edges, malformed typed layouts or copy bounds, illegal Source references,
Source leaves with children, Empty nodes with edges, and reachable cycles. Retain
the established unreachable-node contract; report explicitly what is validated.
No forged graph becomes a successful certificate merely because sources is empty.

Use the existing shared work/depth budget and cycle/admission discipline. DAG cache
reuse must preserve caller depth/height obligations and must not reuse a semantically
different cached result. Do not raise limits or stack size. Source-free captures
retain family-dependent zero/one configuration counts; generated edit bounds keep
their established conservative owner/inheritance arithmetic. Nonempty selected
dependencies retain full mapping certification and addressed dynamic diagnostics.
Closed resources and original/current family authentication remain intact.

## Tasks

### TASK-001: Structural source-free certification
**Status**: Completed
**Parallelizable**: No

- [x] Record fresh source/design/plan hashes and exact private interface.
- [x] Implement bounded structural validation and Capture/Edit integration.
- [x] Preserve selected dependency certification, family bounds and limits.

### TASK-002: Behavioral evidence and author seal
**Status**: Completed
**Parallelizable**: No, depends on TASK-001

- [x] Genuine ordinary slow/long source Capture and generated edit payloads certify.
- [x] Empty-family and nonempty-family configuration bounds remain correct.
- [x] Selected dynamic timing still fails with its addressed diagnostic.
- [x] Forged Source, bad root/edge/layout/copy, cycles, shared DAG and tiny work/depth cases retain checked behavior.
- [x] Focused public source/layout/candidate regressions and exact inventories pass.
- [x] Native/wasm checks, strict scoped Clippy and scoped formatting pass on held tree.
- [x] Fresh author seal records logs, exact commands/counts, failures and terminal handles.

### TASK-003: Independent verification and consumer handoff
**Status**: Completed
**Parallelizable**: No, depends on TASK-002

- [x] All Rust writers held for joined verification on exact hashes.
- [x] Independent checker executes adequate focused and legacy gates with retained evidence.
- [x] Root reconciles changed07E baseline and accepts only this supporting contract.
- [x] Owner updates own status/progress; full08B and full song goal remain pending.

## Completion criteria

- [x] All three tasks complete with independent evidence on unchanged source hashes.
- [x] Actual08B Iterate preparation can reach its resolver; its geometry still requires separate08B proof.
- [x] No malformed public DTO bypass, weakened selected diagnostic or budget/depth relaxation.
- [x] Unrelated work preserved; no full playback readiness claim.

## Progress log

### Session: 2026-10-01

Root created this bounded plan after the actual public route failure and independent
read-only confirmation. ROOT/0097 records prior exact manifest intent and baselines.
Implementation awaits explicit root release; no source changes are authorized by
the plan text alone.

### Author declaration: 2026-10-01

Private `Certification::payload_bound(&FrozenPattern, TimeSpan, u32) -> Result<u64, Failure>` dispatches nonempty selected dependencies to the existing mapping certification. For empty dependencies, `validate_source_free(&FrozenPattern, u32) -> Result<(), Failure>` validates reachable structure with an iterative Enter/Finish stack. A private `(payload address, node index) -> subtree height` cache validates caller remaining depth on every reuse. The helper charges pending stack entries, graph edges, trace/layout collections and cached entries before allocation against the existing shared work budget. Empty graph root zero remains the existing empty-graph convention; unreachable nodes remain unvisited as in mapping certification. Capture retains family zero/one; generated Edit retains its established conservative inheritance arithmetic. No timing callback, arrangement query or repeat expansion is performed.

ROOT0098 released the exact three Rust paths. TASK001 implementation is active; independent verification and full08B remain pending.

### Reason boundary clarification: 2026-10-01

Root and independent read-only review restrict the ordinary bypass to geometry-only `DynamicRate`, `DynamicCount`, and `DynamicTiming`. `DynamicSource`, `UncertifiedCallback`, `UnclosedResource`, and `UnsupportedLiveInput` retain their addressed `BeyondCapability` diagnostic: an empty source list cannot certify unknown callback/source closure or live/resource capability. This refinement precedes the scoped helper/negative fixture correction; no API or limit changes.

### ROOT0103 content-role refinement: 2026-10-01

Author final gate001 (73445 terminal101) retained six previously cleared source-use regressions: ordinary structured note/control timing paths carry `Steps DynamicSource`. Full reachable structural validation remains mandatory. Only typed non-content paths gain a `DynamicSource` exception: SelectContent/Restructure/SampleCycles select their content ordinal; Slices selects ordinal zero; descendants retain false relevance. All other edges inherit relevance. Source references remain illegal in every role; UncertifiedCallback/UnclosedResource/UnsupportedLiveInput remain addressed failures in every role. The source-free cache now keys `(payload address, node index, content relevance)` so timing-first aliases cannot hide an audible unknown-source path. This is a source-grounded narrow amendment to the reason clarification, not a broad unknown source allowance. New fixtures preserve real note timing, reject timing/audible aliases and reject malformed ignored edges.

### Independent completion: 2026-10-01

TASK001–003 and the phase criteria are complete. Author final003 passed all 12 gates on unchanged 13 held inputs: 133 binary-qualified public fixtures, 39 library song fixtures, and 64 pattern fixtures (236 distinct, zero overlap), plus 38 nextest repetitions. Original author process84141 exited0; the exact actual Iterate consumer passed separately on process87248. Author readiness is recorded in `tmp/song-mode-riela/SONG-07F/0010-final-author-readiness-seal.json`. The earlier six compatibility failures (73445 exit101) and quiet-inventory footer parser failure (25903 exit1) remain retained with their scoped fixes.

Independent verification `/tmp/vactr-song07f-independent-001/final-results.json` proved 237 distinct fixtures (including the actual Iterate consumer), 38 nextest repetitions, 10 execution gates plus 11 inventory commands, all 21 command exits0. Original independent process47368 exited0. Manifest SHA-256: `c516f15974828fed2ef5c08e3ff081fdf94d9af488039ebe4fdf594fe647aa46`. ROOT0108 accepted the phase after checking log hashes and all 13 current inputs. No active author processes remain.

Only this plan is changed for completion; the three certified Rust hashes remain unchanged and held. Reachable-only validation, content-role distinction, cache height/depth and aggregate admission are preserved. Actual Iterate preparation now reaches its resolver; weighted Iterate geometry, nested policies, full08B, parent08 and production playback/export/editor remain unfinished. Archive/index updates are deferred to root.
