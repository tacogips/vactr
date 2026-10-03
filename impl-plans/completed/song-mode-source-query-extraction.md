# Selected-source query extraction

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Authentic clock capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose

Move the cohesive selected-source query and its source identity helpers out of
the938-line parent before implementing sampled-source relations. Preserve actual
query order, original source authority, failure behavior and helper import paths.

## Related plans

- **Depends On**: [Clock frames](../completed/song-mode-canonical-clock-frames.md), independent gates and fresh frontend checks before source release.
- **Next**: [Sampling relations](song-mode-canonical-sampling-relations.md).

## Exact two-path manifest

| Path | Deliverable |
| --- | --- |
| `src/song/source.rs` | Thin child declaration and compatibility forwarding/re-exports; original public interfaces unchanged |
| `src/song/source/sampling.rs` | Cohesive extracted query_source and directly related source-trace/identity helpers |

No new timing semantics in the extraction. No dependencies, host or Git edits.
Keep both files below1000 lines. Do not move unrelated source resolution code.

## Tasks

### TASK-001: Source-grounded extraction

**Status**: Completed
**Parallelizable**: No

- [x] Inspect verified source and record the two-path baseline.
- [x] Move the complete cohesive query boundary and required helpers.
- [x] Preserve existing helper imports, visibility, query order and original tests.
- [x] Record extraction-only source before subsequent sampling changes.

### TASK-002: Acceptance

**Status**: Completed
**Parallelizable**: No; depends on TASK-001.

- [x] Confirm the extraction introduces no semantic changes.
- [x] Relevant source/query and original end-to-end tests pass.
- [x] Native/WASM, strict lint, scoped format and held input checks pass.

Acceptance may share one independent run with the following sampling phase,
provided extraction-only evidence is retained and all relevant original fixtures
run. Source implementation may proceed from the completed extraction into that
phase; neither plan is completed until its actual independent gates pass.

## Completion boundary

This extraction does not implement sampling authority or admission. It creates
the declared child seam for those changes without widening public APIs.

## Progress log

### 2026-10-03 — Cohesive parent split prepared

Read-only review identifies query_source as the actual selected Part boundary.
The parent needs extraction before growth. This two-path preparation lets the
following eight-path implementation include its clock regression test child,
whose Grid Unknown expectation must be reconsidered when a genuine Grid timing
relation is implemented. No Rust changes released by this entry.

### 2026-10-03 — Authorized source implementation begins

Clock final004 gates and927 unchanged inputs accepted by root. Current exact baselines recorded in canonical-sampling-source-intent-0001.json. Extraction preserves query_source and source framing, while shared identity cost helpers stay in parent because expand_event uses them. Sampling follows that extraction-only receipt; original owner, work, depth and completed clock tests remain required. No Cargo, dependencies, host or Git modifications.

### 2026-10-03 — Extraction and semantic handoff source ready

Extraction-only intermediate contents/hashes were sealed in source-query-extraction-only-0001.json before semantic hooks. query_source now resides in the ordinary sampling child; original source identity/expand_event helpers and original parent test namespace stay in source.rs. Genuine source boundary publication uses only successful actual returned rows. Extraction equivalence and semantic behavior await independent compilation/tests; no Cargo was run by author.

### 2026-10-03 — Fixture inventory companion

Root authorized the one-path sampling-fixture-inventory companion to make the actual capture_routing seam available solely to genuine tests. Combined verification now contains eleven Rust paths across bounded plans; query extraction and source framing remain unchanged by this repair. Native0002 passed; semantic source acceptance remains pending.

### 2026-10-03 — Bounded phase accepted

Root accepts held0004 after364 fresh distinct Rust tests (clock7, sampling9,
occupancy19, affected202, public127), native compilation, strict all-target
Clippy, pure WASM and eleven-file formatting/line checks. All928 inputs match
and all checker processes are terminal. Receipt:
/tmp/vactr-canonical-sampling-final-004.json, SHA256
e7fd38dee8e054a9aedbadf67e2b0ddeee51ca2342030ca4013df75e30b0de38.
Extraction-only stored source evidence was checked before acceptance.

Fresh WASM b24694ea0994b0d2d2e867f33f41959b682fbc124a8713d6dfe9f828fa78f2f1
passes590 editor tests in78 files and frontend build; dist matches. Root test
37866/fd5b50/0 and build43380/ecb391/0 are terminal. Root independently rechecks
all928 source hashes after frontend gates. Receipt:
tmp/song-mode-riela/ROOT-editor-canonical-sampling-20261003.json.
Historical compiler/fixture failures remain recorded. Full song mode remains
in progress: actual invocation retention, immutable geometry/admission,
production pre-Reserve integration and useful uniform varying-seed support
are mandatory subsequent phases.
