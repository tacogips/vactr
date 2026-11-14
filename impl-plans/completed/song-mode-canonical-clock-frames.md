# Canonical runtime clock frames

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Authentic clock capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose

Retain the actual issuer-to-owner clock relationship with original pre-subject
Index observations. Dynamic factors must come from the runtime query boundary;
an ordered callback journal or an output event cannot supply this relationship.
Empty subjects must retain the same timing evidence as nonempty subjects.

## Dependencies and boundaries

| Dependency | Required state |
| --- | --- |
| [Native shared meter](../completed/song-mode-canonical-native-meter.md) | Independently verified; preserve original ledger |
| [Ordinary replay](../completed/song-mode-canonical-query-replay.md) | Nested dependency repair and genuine independent gates pass before release |
| [Canonical occupancy](song-mode-canonical-index-occupancy.md) | Preserve original authenticated owner/address and transactional retention |

This phase captures affine and reversal boundaries only. Sampled structural
relations, selected SongSource boundaries and immutable geometry consumers
remain mandatory subsequent work. Uninstrumented clock-changing paths retain
an explicit Unknown barrier until their genuine hooks exist. Temporary refusal
does not redefine the final supported scope. No admission readiness follows
from merely storing these frames.

## Exact eight-path Rust manifest

| Path | Deliverable | Status |
| --- | --- | --- |
| `src/pattern/eval.rs` | Thin QState storage and scoped sampling Unknown boundary | VERIFIED |
| `src/pattern/eval/song_clock.rs` | New private checked frames, scoped restoration, projection evidence and genuine tests | VERIFIED |
| `src/pattern/eval/song_observation.rs` | Retain issuer context before subject query | VERIFIED |
| `src/pattern/eval/song_replay.rs` | Retain and charge original projection context during copies | VERIFIED |
| `src/pattern/query.rs` | Exhaustive preserving/instrumented/unsupported dispatch classification | VERIFIED |
| `src/pattern/combinators/time.rs` | Capture evaluated Fast/Slow, Iter and Rev relationships before child query | VERIFIED |
| `src/pattern/step.rs` | Capture actual squeeze width/base before child query | VERIFIED |
| `src/song/query.rs` | Establish and restore authenticated owner-local clock basis around pattern_rows | VERIFIED |

Each touched Rust file must remain below1000 lines. Substantial helpers and
private fixtures belong in song_clock.rs, not the eval parent. Reuse existing
query boundaries and collectors; do not change public QueryVm APIs or add
dependencies, host, resource, transport or Git changes. No undeclared Rust
path is authorized by this plan.

## Private interface specification

Names may be reconciled with existing private types before the source release;
these interfaces specify opaque authority and checked failure behavior.

```rust
pub(crate) enum ClockOrientation {
    Forward,
    Reversed,
}

pub(crate) struct CanonicalClockFrame {
    // Private original evaluated map, query piece and orientation.
}

pub(crate) struct CanonicalClockContext {
    // Private authenticated owner basis and checked frame sequence.
}

pub(crate) enum CanonicalClockProjection {
    Known(CanonicalClockContext),
    Unknown,
}
```

Known context is minted only from the original owner basis and actual runtime
boundaries. Copying or composing it charges the original cumulative ledger
before allocation. Absence of a frame cannot manufacture identity through an
unsupported transformation. A scoped child-query boundary restores its parent
context on both success and Result failure; faults must not leak a frame into
subsequent siblings. Nested pattern_rows establishes its own authenticated
owner-local basis and restores the parent; this reset does not certify a map
between sampled parent and child owners. That relationship remains Unknown
until the mandatory source-boundary phase. Replay retains owner-relative
evidence rather than transplanting an ancestor's absolute context.

## Actual runtime relationships

| Boundary | Child to parent relationship | Required evidence |
| --- | --- | --- |
| `time::fast_by` | `t / factor` | Actual evaluated positive factor and queried child piece |
| `step::squeeze` | `step.begin + width * (t - cycle)` | Actual nonzero width, cycle base and selected step |
| `time::query_iter` | `t - shift` | Actual signed shift; nonpositive count uses existing identity behavior |
| `time::rev_piece` | `mirror - t` | Actual mirror, piece, reversed orientation and endpoint semantics |

Use exact checked Ratio64 operations. Preserve queried pieces independently
from uncut event wholes: queried piece boundaries must not truncate a retained
whole. Reversal must preserve half-open interval meaning and At/Before source
eligibility, including the existing point-query full-cycle behavior. A negative
affine factor alone is insufficient evidence for boundary orientation.
One reflection maps At to Before; two restore At. Preserve the original issuer
sample-start independently of this projected orientation.

Rev point queries issue a full child cycle before reverse_events filters the
returned events to the requested point. Retained pre-subject observations must
therefore carry the original query-piece/point applicability predicate.
Observation capture alone cannot certify membership at the requested point.

Capture frames before recursive q/query_child/q_child. map_times receives only
returned events and is too late for empty subjects. The existing pre-subject
Index hook reads the context without requiring notes from the subject.

QState::sample needs a scoped Unknown capture boundary: point content later
inherits a different structural whole. Do not blanket-mark Slice Unknown; its
Index edge preserves the current clock, whereas subject sampling is a separate
noninvertible relation. Dispatch classification must explicitly cover every
node. Uninstrumented Cat/FastCat/Off, Grid/Segment and other timing changes must
not inherit identity silently. Unknown affects certification evidence while
ordinary audio query behavior remains the existing behavior.

## Tasks

### TASK-001: Opaque contexts and owner scope

**Status**: Completed
**Parallelizable**: No

- [x] Seal source release after replay verification and inspect current hashes.
- [x] Define checked private frames, explicit Unknown and authenticated owner basis.
- [x] Install thin QState storage and scoped restoration without changing live queries.
- [x] Charge every actual frame push, context copy and composition before growth.

### TASK-002: Runtime boundaries and retained evidence

**Status**: Completed
**Parallelizable**: No; depends on TASK-001.

- [x] Instrument Fast/Slow with the actual evaluated factor before recursion.
- [x] Instrument squeeze and Iter with their actual selected step/shift.
- [x] Instrument Rev with orientation and half-open endpoint handling.
- [x] Retain Rev point applicability and original issuer sample-start separately.
- [x] Mark unsupported clock-changing dispatches Unknown explicitly.
- [x] Preserve Slice Index clocks while marking subject sampling Unknown.
- [x] Retain the actual context with observations and raw replay records.
- [x] Preserve distinct issuer, owner-local and root placement clocks.

### TASK-003: Genuine original execution evidence

**Status**: Completed
**Parallelizable**: No; depends on TASK-002.

- [x] Dynamic nonunit factor witness uses real frozen callback execution.
- [x] Squeezed returned Pattern witness preserves a whole beyond its queried piece.
- [x] Iter and reverse witnesses cover fractional and point/boundary queries.
- [x] Empty subject retains actual pre-subject clock evidence.
- [x] Nested owner basis and parent context restore correctly after failure.
- [x] Same raw retained execution replays without reading callbacks again.
- [x] Unsupported sampling path remains Unknown rather than guessed affine.
- [x] Exact sufficient/one-less work and inherited depth witnesses pass.
- [x] Independent relevant tests, native/WASM, strict lint and scoped format pass against held inputs.

## Completion boundary

This plan completes only the stated runtime frame capture. Full song completion
still requires sampled structural/source relations, immutable view lookup and
geometry consumption, production retention before Reserve, uniform varying-seed
resource bounds, and final Native/WASM/browser verification. Preserve original
full identities and callback exclusion throughout those phases.

## Progress log

### 2026-10-03 — Source-grounded next phase prepared

Read-only review identified actual boundaries in fast_by, squeeze, query_iter
and rev_piece. Root inspected their source and the existing QState/owner hooks.
The eight-path phase follows independently verified ordinary replay; the nested
dependency repair is still underway. No Rust release, frame implementation or
admission completion is claimed by this entry.

### 2026-10-03 — Specialized boundary review reconciled

Read-only review confirms all four evaluated maps are available before child
recursion and the eight paths have headroom. The plan now explicitly specifies
Rev point applicability, At/Before orientation, a scoped QState::sample Unknown
barrier, exhaustive dispatch classification and nested owner basis restoration
without an invented parent relation. Source phase remains unreleased until
ordinary replay gates pass.

### 2026-10-03 — Source phase released after verified replay

Replay is archived after348 distinct Rust passes, current native/strict lint,
WASM and eight-file format gates, plus590 editor tests and frontend build using
the exact fresh artifact. All checker/frontend processes are terminal. The
specialized author is released to this exact eight-path phase, preserving the
reviewed sampling barriers, reversal applicability, owner scope and cumulative
meter. No admission or uniform-bound acceptance follows from this release.


### 2026-10-03 — Exact source intent before implementation

The eight-path baseline is recorded in
`tmp/song-mode-riela/canonical-clock-source-intent-0001.json`. Implement opaque
owner-relative frame sequences in song_clock, with checked pre-child maps and
point applicability. Frame sequence copying, push and observation retention use
the original shared collector. No affine certificate crosses sampling; nested
owners reset to their own actual basis and restore the parent's context. Runtime
q dispatch explicitly distinguishes preserving and unsupported nodes. The
current phase does not implement sampled-source authority or admission.


### 2026-10-03 — Runtime clock implementation and genuine fixture source hold

All eight released Rust paths are implemented; independent compilation and
behavioral gates are pending. No author Cargo/dependencies/host/admission edits.
The new child owns exhaustive dispatch classification, projection and sampling
policy. eval.sample supplies actual pattern and point to with_clock_sampling;
Stage2 can replace only that child policy with a precise single-use permit.
Current sampling remains Unknown, including Slice subject sampling; Index q
preserves its actual current clock. Owner resets/restores use the authentic
pattern_rows owner frame and do not certify a sampled parent relationship.

Frames record the actual evaluated factor, width/base/step, signed Iter shift
and Rev mirror plus queried child/parent pieces BEFORE recursion. They preserve
the runtime arithmetic order, avoiding a premature affine intercept that could
overflow when a representable runtime difference does not. Constructors are
lazy at observed Known boundaries, so no new projection arithmetic is forced
on ordinary unobserved queries. Extending immutable Rc context precharges the
original ledger and allocates final frame capacity once. Retained observation
and replay clones share immutable frame payloads, with shallow accounting.
project_for precharges exact owner comparison and frame traversal, rejects a
foreign owner basis, and returns a borrowed owner-associated footprint. It keeps
issuer sample_start separately from projected At/Before orientation. Query-piece
and Rev point predicates never clip the uncut whole; they are not complete
structural/source eligibility or a note-emission proof.

Six genuine private fixtures are written in song_clock::tests:

- actual_dynamic_fast_frames_empty_subject_and_replay_are_owner_bound
- squeezed_actual_returned_pattern_keeps_uncut_whole_beyond_slot
- iter_rev_point_predicates_and_two_reflections_keep_original_sample_start
- unsupported_sampling_is_unknown_and_faulted_frame_restores_sibling
- nested_owner_clock_resets_without_parent_sampling_certificate
- actual_frame_work_and_inherited_depth_exact_one_less_are_not_refunded

Each input is additionally checked through evaluate_song_candidate. The actual
execution fixture uses its OWN fresh evaluator, real Freeze and original frozen
Song Rc with MeteredSongQuery: no cross-build identity equivalence is asserted.
Successful actual raw q records publish ReplayView; DenyCut guards fractional,
reordered queries, and the original isolated evaluator/Song/assets subsequently
transfer into a real consuming candidate. Point tests query the actual frozen
payload with its authentic basis derived from the earlier canonical observation;
all frames are minted by the real recursive q boundaries, not fabricated maps.
Sufficient/one-less work uses actual cumulative VM/query/storage cost, and depth
includes inherited entry depth. Failure checks assert VM ledger restoration and
no raw transaction publication. Nested owner rows retain their own basis and
reject projection against the parent's authenticated basis.

Scoped rustfmt/check exits0 and all eight files remain below1000. These fixtures
have not yet compiled or passed. Source is held for the independent checker;
sampled Stage2, immutable geometry/admission and uniform varying-seed bounds
remain explicitly unfinished full-song requirements.


### 2026-10-03 — First independent checkpoint: four genuine pass, two fixture issues

Native72540 exits0. Clock60369 exits101: dynamic Fast/empty/replay, squeezed
returned Pattern, Iter/Rev point/double reflection and nested owner tests pass.
Two fixtures fail before their remaining intended assertions. The public segment
signature requires Signal, so its Pattern input is invalid; the Unknown witness
now uses supported Grid with actual boolean structure and point sampling.

The exact-work test directly calls observe_part and inspects pending collector
records. A later fuel failure may legitimately leave an earlier successful q in
that unpublished scratch ledger. It is incorrect to equate scratch emptiness
with snapshot transaction publication. The same distinction applies to the
DivisionByZero assertion. Original exact/one-less work and VM scope restoration
must stay; a genuine retain_index_occupancy snapshot transaction test is required
to demonstrate no published view on either failure. Snapshot fields and mutable
PreparedSong snapshot access are private outside its module; author reported the
smallest cfg(test) seam/companion requirement before editing any undeclared file.
No production repair or assertion deletion is made at this boundary. Fixture
fixes remain in progress; no clock completion claimed, no author Cargo run.

### 2026-10-03 — Transaction fixture repair source held

Companion clock-transaction-fixtures declares the additional owning occupancy seam and cohesive test child. All six fully qualified clock test names remain unchanged. Invalid Segment input becomes Grid; pending scratch execution emptiness is replaced by an actual snapshot retention transaction witness. Exact direct frame work/one-less, inherited depth, genuine sibling restoration and VM scope restoration remain. The owning helper mints the complete genuine Fast/Rev/Slice path, seeds a prior published view, measures request+retention work, proves exact success publishes a second record and one-less fuel failure retains the identical prior Rc view and record. A typechecked dynamic zero denominator produces actual DivisionByZero on the next window and checks the same publication invariant. No runtime production behavior changed. Scoped ten-path format/check exits0; independent execution pending. Occupancy is999 lines and requires cohesive extraction before future growth.

### 2026-10-03 — Legacy recursive stack repair held

Clock0002 independent evidence: native32722/0, clock77458/0 six passes, occupancy19/0; all927 held inputs unchanged. Affected pattern scope then aborts with real stack overflow in unchanged deep_chains_work_and_too_deep_chains_fault after17 visible passes; this is not a complete64 pass. Log canonical-clock-affected-lib-002-1.log and final-002.json retain the actual101/SIGABRT.

Unmetered callers now bypass clock dispatch/frame/sampling wrappers before recursive calls. Observed-only dispatcher and Rev child construction are separate non-inlined functions; frame allocation/copy temporaries finish in a separate install function before child recursion. The predicate checks actual observation ledger presence, preserving computational native-query shared charges even when is_observed excludes them. Clock debit order, owner evidence, supported dispatch, sampling child/point seam and restoration remain unchanged. No legacy test/depth/stack environment is altered.

A seventh genuine clock fixture builds200 reversals through actual evaluator/candidate/Freeze, uses unchanged max_depth256 at both default work16,384 and allowed work1,000,000, requires success at the latter and authentic FuelExhausted only at the default if insufficient. It also verifies an actual smaller caller depth refuses with DepthExceeded. This prevents quadratic work from hiding stack failure at a permitted larger work budget. Fixture execution pending; scoped ten-path rustfmt/check0 and all files below1000. Source held0003; no author Cargo.

### 2026-10-03 — Shallow public construction for observed deep fixture

Clock0003 native88875/0 and affected legacy pattern51417/0 all64 pass, including unchanged200/300 reversals on normal stack. Clock scope passes the original six but new deep fixture fails candidate parsing before observation: nesting deeper than128. All927 inputs match final-003.json; all commands terminal. The test now uses public reduce0..200 with a genuine Slice seed and `{acc k -> rev acc}` callback, following the checked bus construction precedent and actual reduce native cx.call iteration. This constructs200 nested Pattern nodes through the original evaluated/frozen candidate without200 syntax nesting. Frame count200, default/allowed1M work, unchanged depth256 and actual smaller-depth refusal assertions remain. No production changes in this repair. Scoped ten-path format/check0; held0004, execution pending; no author Cargo.

### 2026-10-03 — Independent acceptance and fresh frontend artifact

Held0004 passes all seven clock fixtures, nineteen occupancy/replay fixtures,
202 affected library tests and127 public tests:355 distinct passes. Of these,
291 execute freshly and64 legacy pattern tests carry only through the proved
sole fixture-child delta from held0003. Native, strict all-target Clippy, pure
WASM and ten-file scoped format/line limits pass. All927 inputs are unchanged
and all checker processes are terminal. Receipt:
/tmp/vactr-canonical-clock-final-004.json, SHA256
8701477692890c7154c3be25bdbc54e5769361270162c46b6f93ff5f62682d41.

Fresh WASM f3e0e665eea52b9c68edb4a4874c119d175bcebc8420c509a523e6c01588b703
passes590 editor tests across78 files and frontend build; dist matches. Root
receipt: tmp/song-mode-riela/ROOT-editor-canonical-clock-20261003.json. All
frontend processes are terminal and root rechecks all927 source inputs. Prior
fixture101, SIGABRT101 and parser101 checkpoints remain historical evidence.
This closes only the stated bounded plan, not full song-mode completion.
