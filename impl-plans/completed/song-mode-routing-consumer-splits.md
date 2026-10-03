# Routing module headroom for canonical consumers

**Status**: Completed
**Updated**: 2026-10-03
**Design Reference**: [Canonical consumer review](../../design-docs/specs/design-song-mode.md#canonical-collector-review-owner-selection-and-clocks)

## Purpose

Make room for the mandatory immutable occupancy consumer bridge without
crossing the repository's 1000-line Rust source limit. Current prepare.rs has
989 lines and components.rs has994. This is a behavior-preserving extraction,
not canonical admission implementation or a readiness certificate.

## Related plans

- **Previous**: [Canonical foundation](../active/song-mode-canonical-index-occupancy.md).
- **Concurrent, disjoint source**: [VM/native meter](song-mode-canonical-native-meter.md).
- **Next**: Canonical occupancy TASK-002 immutable consumers and pre-Reserve handoff.
- **Acceptance owner**: [Reconciliation](../active/song-mode-reconciliation.md).

## Exact five-path Rust manifest

| Path | Deliverable |
| --- | --- |
| `src/song/routing/prepare.rs` | Keep public prepare_routes and admission/resource arithmetic; thin RouteBuilder declaration and child module. |
| `src/song/routing/prepare/builder.rs` | New cohesive symbolic topology/branch traversal implementation extracted from the RouteBuilder impl. |
| `src/song/routing/components.rs` | Keep intrinsic component/lifecycle arithmetic; replace inline test bodies with explicit child declarations. |
| `src/song/routing/components/iterator_component_tests.rs` | Move existing iterator component fixtures unchanged. |
| `src/song/routing/components/affine_candidate_tests.rs` | Move existing genuine candidate/density/routing fixtures unchanged. |

Use the existing test module names via explicit child paths, preserving their
fully qualified fixture names. Keep RouteBuilder fields in the parent where
prepare_routes constructs and reads them. The extracted child can access parent
private fields; expose only walk to its parent with the minimum private module
visibility. PayloadOwner and the FxPolicy alias may move into the child if only
its methods use them. Relocate imports as needed without blanket lint allowances.

No signatures, quotas, errors, trace order, route identities, source bounds,
fixture bodies/assertions, dependencies or public APIs change. Every touched
source remains below1000 lines. The source scope does not overlap the active
seven-path VM/native meter implementation.

## Tasks

### TASK-001: Symbolic builder extraction

**Status**: Completed
**Parallelizable**: Yes; disjoint from VM/native meter.

- [x] Seal baseline hashes and enumerate all moved methods.
- [x] Move RouteBuilder impl into the declared child with minimal visibility.
- [x] Preserve public prepare_routes construction and all arithmetic behavior.

### TASK-002: Component fixture extraction

**Status**: Completed
**Parallelizable**: Yes; disjoint from TASK-001.

- [x] Move both existing test modules without altering bodies/assertions.
- [x] Preserve fully qualified fixture names and private helper access.
- [x] Keep production component code and import visibility unchanged.

### TASK-003: Joined verification

**Status**: Completed
**Parallelizable**: No; waits for both extraction and any other active source authors to hold.

- [x] Scoped formatting and every touched file below1000 lines.
- [x] Independent actual original component test names execute nonempty.
- [x] Existing public route/admission tests preserve their assertions.
- [x] Native, strict all-target lint and WASM checks pass against joined held inputs.
- [x] Record exact moved-source/hash evidence; no completion claim for canonical admission.

## Progress log

### 2026-10-03 — Source-grounded preparatory manifest

Root inspected production RouteBuilder and the two inline component fixture
modules. Their extraction supplies headroom for the known density/configuration
authority bridge. Ready is not a source release; specialized author coordination
and a joined checker hold remain required before execution and verification.

### 2026-10-03 — Authorized extraction declaration

Immutable baseline and method inventory are recorded in SONG-ROUTING-CONSUMER-SPLITS/0001. Move walk, payload, source_effects, nested_effects, policy_cover and branch into prepare/builder.rs; only walk becomes pub(super). PayloadOwner and FxPolicy move with their sole implementation consumer. Existing iterator_component_tests and affine_candidate_tests module bodies move verbatim apart from indentation/formatting; explicit child paths preserve original module names. No behavioral changes or author Cargo are authorized.

### 2026-10-03 — Source extraction held

Five Rust files are scoped-formatted and below1000 lines. The builder impl compares unchanged after whitespace normalization and reversal of the sole walk visibility addition. Each extracted fixture child exactly equals the separately formatted original body; no assertions or test names changed. Parent RouteBuilder fields remain unchanged. No Cargo ran; independent actual original component/route tests, Native, strict lint and WASM remain pending joined source holds. This extraction does not implement canonical consumers or a uniform birth proof.

### 2026-10-03 — Joined independent acceptance and archive

Actual joined checkpoint executes307 unique passing tests:10 occupancy,13
original component fixtures,189 affected library tests and95 public tests.
Native, strict all-target Clippy, pure WASM and twelve-file formatting/line
checks exit0. All923 held inputs are exact. Evidence:
/tmp/vactr-canonical-meter-final-001.json, SHA256
4447b1a96ee59709084cfe647cba0ff8116a4147c911d5a312aad93d29ef8b63.
All checker processes are terminal. The completed bounded phase is archived;
ordinary replay, immutable admission and uniform varying-seed authority remain
required in the active song-mode plans. No full song-mode completion is claimed.
