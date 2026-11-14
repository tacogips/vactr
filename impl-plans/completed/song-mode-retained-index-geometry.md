# Retained Index configuration geometry

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Current completion path](../../design-docs/specs/design-song-mode.md#design-and-implementation-review-current-completion-path), [Immutable consumers](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose and dependencies

Consume genuine retained invocation evidence in an actual configuration
geometry function. Keep source START eligibility independent of owner clipping;
union eligible uncut wholes only within the exact authenticated configuration.

| Relation | Plan | Condition |
|---|---|---|
| Previous | song-mode-retained-invocation-lookup.md | Genuine addresses and projection supplied |
| Related | song-mode-canonical-index-occupancy.md | Full immutable integration still required |
| Next | Routing provenance and production admission companions | Must thread these results to actual consumers before completion |

The two companion manifests together touch at most eleven distinct Rust paths;
overlapping occupancy.rs, lookup.rs and song_clock_projection.rs are shared;
source mutations must be serial.

## Exact Rust manifest

| Module | Path | Status |
|---|---|---|
| Configuration entry | `src/song/routing/configuration/index.rs` | Completed |
| Actual retained consumer | `src/song/routing/configuration/index/canonical.rs` | Completed |
| Owning bound-site fixture wrapper | `src/song/routing/index.rs` | Completed |
| Opaque bound-site validation seam | `src/song/snapshot/occupancy/lookup.rs` | Completed |
| Actual per-source projection evidence | `src/pattern/eval/song_clock_projection.rs` | Completed |
| Thin geometry fixture declaration | `src/song/snapshot/occupancy.rs` | Completed |
| Genuine owning geometry fixtures | `src/song/snapshot/occupancy/geometry_tests.rs` | Completed |
| Remaining domain/copy/refusal fixtures | `src/song/snapshot/occupancy/geometry_tests/domains.rs` | Completed |

Eight paths maximum; all below1000 lines. routing/index.rs currently936 lines:
keep forwarding thin. If further growth requires extraction, stop and declare
a separate cohesive split before exceeding the bound. routing/source.rs is983
lines and is excluded: reuse existing ResolutionBudget::with_remaining and
limits().max_depth without adding a getter or resetting its allowance.

## Declarations

```rust
pub(in crate::song::routing) fn canonical_index_configuration(
    bound: &BoundSliceOperands<'_>, request: &CanonicalIndexRequest,
    address: &RetainedIndexAddress<'_>, selected: Option<&FrozenSelectedSource>,
    source: TimeSpan, owner: TimeSpan, depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<Option<TimeSpan>, Failure>;
```

Canonical addresses remain opaque. The owning routing wrapper reuses actual
output_operand_owner, prepare_slice_operands and bind_slice_operands; it mints
CanonicalIndexRequest through the original SongSnapshot method and authenticates
the already-issued timing against the address. The configuration consumer must
validate through lookup-owned methods rather than accessing private token fields.
The current lookup interface does not supply that validation method. Add a
narrow owning method accepting the original privately minted request and
checking its complete site/recipe against the existing address. Do not expose
token fields or accept caller-reconstructed execution keys. BoundSliceOperands
remain routing-private; the owning routing wrapper issues the request.
Fixtures call this actual consumer through that owning wrapper. They must not
construct alternative evaluator values or forge timing flags.

Empty or suppressed output cannot honestly supply BoundSliceOperands because
no subject handle/FrozenSliceTiming was issued. Provide a separate owning
prepared-policy entry using PreparedSliceAddress, the genuine request and the
opaque retained address. It must keep sealed source membership separate from
pre-subject Index geometry: absent members cannot be fabricated, while retained
rows do not disappear merely because notes were suppressed. Specify the exact
return contract before source release; continuous-whole absence is a truthful
failure, never an invented empty success.

### Prepared-policy entry and result

Declare these routing-private types in the canonical consumer child; fields are
private and only the authenticated consumer constructs results:

```rust
pub(in crate::song::routing) enum RetainedSourceMembership { Empty, NonEmpty }
pub(in crate::song::routing) struct PreparedIndexGeometry;
pub(in crate::song::routing) fn canonical_prepared_index_geometry(
    prepared: &PreparedSliceAddress<'_>, request: &CanonicalIndexRequest,
    address: &RetainedIndexAddress<'_>, selected: Option<&FrozenSelectedSource>,
    source: TimeSpan, owner: TimeSpan, depth: u32,
    budget: &mut ResolutionBudget,
) -> Result<PreparedIndexGeometry, Failure>;
```

PreparedIndexGeometry borrows/exposes authenticated connected components as a
slice of TimeSpan and a separate optional source-membership classification.
Some(Empty) requires a genuine sealed selected boundary with zero returned
members; Some(NonEmpty) requires authentic nonempty sealed membership. None in
this classification means this is a direct issuer policy with no selected-source
boundary, not missing required evidence. Supplying selected requires exact
original parent payload descriptor and full producer/use matching through lookup.
Missing/ambiguous required boundary refuses. It never constructs a subject handle,
FrozenSliceTiming or BoundSliceOperands. A suppressed subject can retain nonempty
Index components; an empty selected membership cannot erase those pre-subject
components. An actual empty successful Index invocation yields an empty component
slice. Continuous whole absence/Unknown relation refuses rather than becoming
empty success. The bound entry returns None only if its authenticated addressed
configuration has no eligible component after owner intersection; prepared entry
returns all eligible connected components, without selecting a fabricated row.

### Owning validation and per-hop projection

The owning routing wrapper prepares the actual site and privately mints request
using the original snapshot. Both entries call a lookup-owned validation method
accepting that request and the existing opaque address. Routing separately checks
prepared owner/recipe/issuer/prefix against the minted request, and the bound entry
keeps its existing bind_slice_operands subject/timing checks. No scalar execution
key constructor or seed reconstruction is introduced.

Projection retains a precharged borrowed record per actual sealed source hop:
its owner basis, original incoming START/orientation, authenticated finite selected
source domain, actual query piece (which may be a point), projected outgoing
START/orientation, uncut projected whole and applicability. The selected source
domain is [0, original selected Part.duration) in selected-root coordinates;
actual owner offset is applied before this domain check. Query applicability is
sect(uncut selected whole, actual query piece), independently including point
queries. Never treat a zero-width request as a finite source-domain extent.
Evaluate source START predicates in their genuine finite domain basis: At uses begin <= START < end;
Before uses begin < START <= end. Structural sample alone resets At, and affine
parents preserve incoming orientation. Original issuer START remains separate.
Intervals are grouped only by original owner/execution seed/placement, actual
issuer/full producer entry, selected descriptor and complete use/copy trace.
Chord tones do not create additional groups or bridge gaps. Union overlapping or
touching eligible uncut wholes within that group, then clip to the actual owner;
no hull across gaps or semantic-equal different parents. Charge hop records,
interval retention, comparisons/sort/union and depth using the original counter
via with_remaining before storage growth; never replenish a failed lookup.

### Concrete owning fixture matrix

The existing original-code freeze helpers and routing owning wrapper must execute
both actual consumer entries, with cut reads denied after retention:

- Reuse genuine two-parent sampled inner Slice and nil-subject programs from
  lookup fixtures; compare same raw owner under distinct sealed parent bindings,
  partition/reordered requests and actual full trace/copy/seed distinctions.
- Reuse inner Rev + affine outer versus second Rev; assert each intermediate
  source [0,1) boundary uses Before/At correctly, then structural Grid resets At.
- Genuine composed Fast/Slow and fractional Sequence/Repeat owners provide
  eligible START before owner head, long uncut whole beyond source end, overlapping
  and touching intervals plus a separate gap. Keep source eligibility independent
  of final owner clipping and select the bound event's actual component.
- Empty Index success has zero components; nil subject with nonempty Index retains
  components; sealed empty membership remains separately Empty even where an
  independently retained pre-subject row exists. Unknown/continuous absence refuses.
- Actual chord [:c :five] verifies shared authenticated component without tone
  duplication; foreign original same NodeId, wrong recipe/issuer/prefix/placement
  and different actual varying seeds cannot substitute a token.
- Measure whole wrapper/lookup/geometry operation cost on a genuine original
  retained view; exact succeeds, one-less fails without new ledger/publication
  mutation. Actual nested source-hop depth exhaustion remains required.

## Tasks

### TASK-001: Authenticate bound configuration

**Status**: Completed
**Parallelizable**: No

- [x] Bind actual issuer, complete symbolic/producer paths, selected source
  policy, source identity and actual invocation key through opaque authority.
- [x] Keep distinct source/use/configuration groups separate; reject foreign,
  missing and ambiguous records with no VM/callback access.
- [x] Sealed empty membership yields Empty classification and no source member; retained pre-subject Index components remain independently available, without fabricated handles.

### TASK-002: Exact connected component consumer

**Status**: Completed
**Parallelizable**: No; depends on TASK-001

- [x] Apply source eligibility using original issuer_sample_start, then actual
  source/sampling/affine/reversal projection in the appropriate owner basis.
- [x] Demonstrate every intermediate source-parent eligibility predicate with
  its actual projected START/orientation and source window. A final footprint
  plus original issuer START alone is insufficient evidence for this criterion.
- [x] Preserve At/Before half-open boundary semantics and query applicability.
- [x] Union eligible uncut overlapping/touching wholes, preserve genuine gaps,
  then clip to owner. Chord tones may share only their authenticated group.
- [x] Use the same remaining/depth via with_remaining; precharge storage and
  sorting/union operations; no fresh work ceiling or note-onset inference.

### TASK-003: Actual consumer fixtures and acceptance

**Status**: Completed
**Parallelizable**: No; depends on TASK-002

- [x] Genuine composed dynamic rates/sampling and two-parent rebinding agree
  across reordered/partitioned queries with callback reads denied.
- [x] START outside owner head but eligible, long whole beyond source end,
  overlap/touch/gap, two reversals and fractional sequence/repeat offsets.
- [x] Actual empty success, suppressed notes and continuous-whole absence
  retain truthful geometry; source membership is never invented.
- [x] Foreign same-NodeId, wrong recipe/issuer/prefix/placement/seed and missing
  records fail; exact/one-less shared work and actual depth are verified.
- [x] Independent native/tests/strict lint/WASM/format and sealed input cohort.

## Completion criteria

- [x] All three tasks complete and actual geometry consumer is exercised.
- [x] Lookup and geometry companions jointly pass genuine owning fixtures.
- [x] No unused cloned view is counted as production integration.
- [x] Later query-issued route provenance, density/admission, pre-Reserve
  retention and useful uniform varying-seed support remain explicit work.

## Progress log

### 2026-10-03 — Actual immutable consumer specified

Source review finds index_configuration refusing RequiresRealization before
connected geometry. This companion supplies the actual retained consumer;
production event provenance and cover handoff remain downstream requirements.
Implementation follows lookup, with no concurrent mutations of their shared
occupancy parent or held predecessor source.

### 2026-10-03 — Original consumer text saved for reproducible review

Root saved the two unchanged existing routing files with full text and hashes
in retained-geometry-unchanged-originals-0001.json, SHA256
5f977e50642410ceb60ca7e9db0fdb8e4f1daa09eb74f6534ad1e3d2b95d5afd.
Both match the accepted invocation0006 cohort. The shared occupancy parent must
be saved after lookup hold before geometry release; this partial artifact does
not authorize geometry edits or claim a complete baseline.


### Geometry implementation intent

Source release follows accepted lookup380 Rust and590 frontend gates. Point query
pieces are explicitly separated from real selected source-domain eligibility.
Exact seven-file manifest retained; original text saved before changes below.
No production provenance, admission, host or uniform-seed phase is included.


### Bound membership and group refinement before source implementation

Parent approved bound entry selected:Option<&FrozenSelectedSource>. Some binds
actual timing.subject_handle through genuine bind_member; None is accepted only
when actual source chain is empty. Prepared results preserve groups separately
instead of flattening all components together. Within one original invocation,
groups compare full actual owner/placement/context, concrete issuer prefix,
observation seed and actual sealed source/use/copy identity; raw Index trace picks
its own group before connected union. No fabricated subject or reconstructed key.


### Cross-cycle authenticated configuration coverage

A token's one raw execution is not the whole configuration domain. The owning
lookup collects related published invocations under the original issued request:
exact original Song/recipe/site, intrinsic owner revision/track/root/placement,
offset/duration, actual seed and producer entry. Each execution's actual window,
clock/query applicability, source chain and admitted depth remains independently
validated. Configuration equality does not equate execution query windows or Rc
certificate allocations with intrinsic identity. Actual source policy/use/copy and
parent intrinsic owner identity remain distinct, with every scan/comparison charged.
No VM/requery or coverage reconstruction from NodeIds occurs. Add actual Part4
slow {slice {beat -> nil} 1 [cut]} 2, retained root0..4, issuer source0..2 and
owner0..4; the genuine touching0..2 and2..4 wholes must form component0..4.
Repeated coverage may duplicate the same sealed key/row evidence, which must not
cause a false ambiguity; distinct actual keys/configurations are never collapsed.


### 2026-10-03 — Intermediate geometry implementation SOURCE_READY_HELD0001

Implemented the two genuine configuration entries, exact prepared-site validation,
sealed source-member classification, independent source-domain/point applicability,
per-hop START/orientation checks and precharged connected-component unions. Bound
selection first authenticates its own original invocation row; repeated continuation
rows in related cycles do not falsely become scalar-address ambiguity. Related
rows authenticate original record/request/invocation attachments and intrinsic
source/use/copy configuration independently of per-row frame values. Root coverage
is a charged interval union of relevant published partitions over the authentic
final owner offset plus caller owner extent; partial retention refuses. Whole Song
duration is not required or expanded. All source and geometry scans use the original
counter; final coverage checks are charged too.

Ten owning fixture bodies now call the real consumer: suppressed versus empty
Index; actual chord bound timing and owner clipping; exact/one-less/foreign view;
point-source Rev/affine/Grid boundary; sealed empty membership with retained Index;
dynamic-rate reordered windows; whole-Part cross-cycle touching union; genuine
partition/reorder coverage and partial refusal; addressed ineligible START; and
rest-gap preservation. These are authored, compile-checked witnesses, not runtime
PASS claims. Remaining matrix requirements (full fractional/copy/seed separation,
explicit overlap, full per-hop orientation values, continuous refusal and additional
wrong-site/member cases) remain open pending focused diagnostic execution and later
completion. No production provenance, host admission or varying-seed ceiling is
claimed.

Author compilation only, CARGO_TERM_QUIET=true mise exec -- cargo check --tests:
001 original8687 exit101 (declaration before inner doc); 002 original22952 exit0;
003 original31328 exit101 (routing child import privacy); 004 original70351 exit0;
005 original29592 exit0; 006 original26438 exit0; 007 original35155 exit0;
008 original30047 exit0 terminalcad134. Logs are
/tmp/vactr-retained-geometry-author-compile-001.log through -008.log. Every handle
was terminal before subsequent edits. The genuine routing helper implementation
was moved into the already declared canonical child with thin inherent forwarding,
keeping routing/index.rs965 lines. Compile008 reports narrowly located dead-code
warnings at currently unwired production entries/helpers; these remain visible
for the checker rather than blanket-suppressing a module. All seven files below
1000 lines; scoped seven-file rustfmt --check exit0. Independent behavior/native/
strict/WASM verification is pending; no author behavioral tests were executed.


### 2026-10-03 — Intermediate0001 runtime diagnosis and fixture repair0002

Independent original70932/e262be exit101 executed45: old35 PASS, geometry7 PASS,
three fixture failures. Complete935 pre/post hashes unchanged. Raw
/tmp/vactr-retained-geometry-occupancy-001.log SHA256
d575d42b37f44ea31aac1ee3a60330c62e24d88c8725fb1756444d1e3c50936d;
final001 SHA256 dba4d5ca39768c86d9c2433f7437bdd3a0d4c4594d57f26c9d0fc19866b68416.
The bound chord fixture requested a component across the explicit nil rest in
[cut nil]; the dynamic Slow2 fixture likewise incorrectly demanded output in its
rest window. Both positive continuous witnesses now genuinely use [cut], while
the independent rest-gap fixture and all timing/component assertions remain.
The source fixture incorrectly called all selected-source traversals point queries:
Fast/Rev directly traverse a finite source piece. It now checks finite [0,1) for
those cases and genuine point requests for structural Grid and a new actual outer
Slice subject sample, preserving the required point/domain distinction. No
production changes, frozen flags, minted records or query pieces were fabricated.
Seven complete original texts saved before repair in
retained-geometry-before-repair-originals-0001.json SHA256
215b853abd4cd4110cfbf9147fe13f94e8a9fe69eabd9c4a72c5a8048d347e21.
Author quiet compile-only009 original1009 exit0 terminalb1b7d0; no edits while live.
Scoped7 rustfmt check0. Runtime retry remains independent/pending; remaining
matrix remains open and no full geometry/admission completion is claimed.


### 2026-10-03 — Intermediate0002 genuine membership diagnosis / fixture0003

Independent original15308/020faf exit101: old35 and geometry9 PASS, one remaining
positive source fixture expected NonEmpty where structural sampling reached the
reflected rest of inner Rev [cut nil]. Source full935 unchanged; raw002 SHA256
f6e69ccb6de1e3109d389c42306ac869f8838ff9c1c5dd6797c48b4b094b0d2a,
final002 SHA256 64d3d952987292858046890ea49ed4ce9fcbc26c73fb04d84596249c43bc2790.
The positive inner source now genuinely uses Rev [cut], covering the complete
original cycle at actual finite and point queries. The separate real Empty source
membership and rest-gap witnesses remain unchanged. NonEmpty, genuine point query,
component and inherited-depth assertions remain intact, with case/query diagnostics.
Production untouched. Full seven pre-repair texts saved in
retained-geometry-before-repair-originals-0002.json SHA256
2f41229b7cfddd55fc002a3240893a095f1c18f08465adcbf95f0c54ec086c19.
Author quiet compile-only010 original14070 terminalbcbc80 exit0; scoped childfmt
check0; all author handles terminal. Independent runtime retry pending; matrix
and production integration requirements remain open.


### Final matrix extension intent / bounded eighth child

Focused0003 original53782/a59c4c exit0 verifies all45 (old35 plus10 geometry).
All935 source hashes exact; final SHA256
150b66709b10bfa1ba573f84aa767f76b39c9962210e27652fca049f4c9cb70b.
The remaining substantial domain/copy/refusal fixtures belong in the cohesive
geometry_tests/domains.rs child, declared as the eighth path before source writes;
existing verified tests stay in their owning parent. This avoids parent growth
past1000. New fixtures call the same actual geometry consumer and owning helpers,
not an alternate algorithm. Expected full cohort936 (one additional child).
Pre-extension seven full texts saved as retained-geometry-before-extension-originals-0003.json.
No production provenance phase is included. Real lookup/configuration entrypoints
remain unwired in ordinary routing until that mandatory downstream phase.


### Intermediate0004 diagnostic hold: remaining geometry matrix authored

Added four genuine owning cases in declared eighth domains child: dynamic affine
factors produce overlapping distinct original wholes and source START0 remains
eligible before a clipped owner head; fractional Sequence plus real :vary Repeat
retains original offsets/placements/seeds; symbolic Index repeats preserve concrete
prefix groups; actual public Euclid/continuous Index inputs must truthfully refuse.
The continuous case is deliberately a diagnostic of a possible real missing mode
certificate: an unstructured Index can select the empty Subject branch and produce
no Index observations. It must not be silently called empty Index success. No
mode/source flags were changed, no record fabricated and no speculative production
certificate path added. Additional final orientation-value/wrong-site/member
witnesses remain open after this diagnostic; earlier genuine source finite/point,
NonEmpty/Empty and depth checks are unchanged.
Only the named unused validate_site method has a documented dead_code allowance:
its narrow-window API remains for later issuance integration, while this consumer
uses the genuine validate_prepared_site method. No module-wide lint mask added.
Author quiet compile011 original72661 terminal4bf806 exit0; compile012 original5481
terminal3109ff exit0 after that specific annotation; all handles terminal before
edits. Scoped eight-file rustfmt --check exit0. Earlier45 actual PASS remain valid
on their unchanged fixture lineage; four new behavior scopes are unexecuted until
independent focused49. All eight files below1000. Full cohort expected936 with
only declared new child added. No full geometry/admission completion claim.


### Intermediate0004 actual failures / bounded fixture repair0005

Independent original97783/9bb5ab exit101:49 selected,46 PASS and3 fixture failures;
old35 PASS, geometry11 PASS. Full936 unchanged. Raw004 SHA256
e1a75051f8aec0f3876941e9ee7a3a09df458142a0a4b94fced68d98bdca1d55;
final004 SHA256 ab6c3318fd91772354c786f1c8cd32f214d714938e7afe7bc2ee599f40ef0528.
The fractional Repeat fixture now preserves raw continuation evidence and compares
unique genuine placement/offset/seed tuples: duplicate coverage does not become a
third occurrence, while different seeds/placements remain distinct. The public
repeat native constructs a list, not PatNode::Repeat; its two independently issued
Slice sites are now both traversed from the genuine frozen recipe and retained as
actual requests. Each request consumes its own full original prefix, while the
fixture separately verifies two distinct real prefixes across the two sites.
This fixture is renamed honestly; it does not claim symbolic PatRepeat coverage.
The continuous Index already refuses at canonical_index_request with the actual
first-structure-rule diagnostic, before retention or geometry. The witness now
asserts that genuine refusal and no publication/callback access. It does not claim
a missing-mode production defect or fabricated empty success. Euclid Unknown still
must reach the actual geometry consumer and refuse. No production changes.
All eight pre-repair texts saved in retained-geometry-before-repair-originals-0004.json
SHA25613954109365762701a4d00efe1e6b16f7ac45f2ada5c02f1c0c2046f78d05fde.
Author quiet compile013 original86916 terminalb76b98 exit0; scoped childfmt check0.
All handles terminal. Independent focused49 retry pending; remaining final matrix
stays open, including genuine symbolic-copy and explicit orientation/member cases.


### Final matrix source checkpoint0006 — genuine authority fixtures

Focused0005 original42452/cad741 exit0 verified49 distinct tests, all936 exact;
raw SHA256 a567c0564654e28de947db31dadef0d2b8c171b9758803dac6906eb4932bb694,
final SHA256 1d7abe3d351a17d72ad7bc7809e5ad58ab7dc14cadd304d8da30a4861d4e0e6b.
Saved all eight full texts before the extension as
retained-geometry-before-extension-originals-0005.json SHA256
d5625137189c57fa83292d6e4d1ba9e7a205447c97481bf86906da1cfa28bb90.

The final three authored witnesses/strengthenings are compiled, not runtime PASS:
- Existing source finite/point fixture now reads exact genuine incoming/outgoing
  source-hop values via a cfg(test) borrowed iterator. Inner Rev maps START0 to
  Before1; actual affine parent preserves Before1, outer Rev yields At0, and real
  Grid/Slice structural sampling resets At. Every source domain/request and genuine
  projected eligibility is asserted, rather than only final endpoints.
- Public Rust Pattern::Repeat construction wraps an actually evaluated original
  Slice. A fresh isolated evaluator's real slots and original Song are copied by
  the production Freeze; the same evaluator/Song supplies actual routing inventory,
  candidate and prepare_song. No frozen flags, IDs, timing or invocation is minted
  manually. Genuine frozen Copies(count2) and distinct actual prefix groups must
  survive real retention/consumer. The older list-native repeat witness remains
  accurately named and unchanged.
- Wrong real sibling Slice site and foreign original subject timing must refuse;
  separate real selected-source binding checks that an actual base subject handle
  cannot masquerade as a returned inner Part member, through bind_member itself.

| Required criterion | Actual owning geometry fixture |
|---|---|
| Original recipe/issuer/full site; chord timing; owner clipping | genuine_bound_chord_uses_original_timing_and_owner_clip_after_union; wrong_original_site_and_foreign_subject_member_timing_refuse_geometry |
| Exact source/use and member authority | selected_source_member_binding_rejects_a_genuine_other_original_subject; real_point_source_requests_keep_domain_rev_and_sample_eligibility_separate |
| Suppressed/empty Index versus sealed empty membership | prepared_suppressed_and_empty_success_are_distinct_without_subject_handles; genuinely_empty_membership_does_not_erase_pre_subject_index_components |
| Source START outside owner head; long uncut source-eligible whole; actual overlap | dynamic_overlapping_wholes_keep_source_start_before_owner_and_long_extent |
| Touching cross-cycle components; complete related coverage | whole_part_configuration_unions_touching_original_cross_cycle_wholes; authentic_partition_union_covers_owner_and_partial_retention_refuses |
| Genuine gaps and addressed row's own source eligibility | genuine_index_rest_gaps_are_not_replaced_by_a_configuration_hull; addressed_source_start_outside_window_cannot_borrow_other_eligible_component |
| Actual intermediate Rev/affine/Sample At/Before; point request distinct from finite domain; inherited depth | real_point_source_requests_keep_domain_rev_and_sample_eligibility_separate (strengthened exact hop values) |
| Fractional Sequence/Repeat geometry; actual seeds/placements | fractional_sequence_vary_repeats_preserve_actual_placements_seeds_and_groups |
| Concrete genuine list sites and symbolic Copies groups | actual_list_repeated_slice_sites_keep_full_prefix_groups_distinct; genuine_public_pattern_repeat_retains_symbolic_copy_configuration_prefixes |
| Dynamic rates; fractional/reordered windows and callback denial | actual_dynamic_rate_reordered_windows_keep_groups_and_callbacks_immutable |
| Continuous first-structure refusal and Unknown geometry refusal | unsupported_and_continuous_index_evidence_refuse_without_callback_fallback |
| Exact/one-less work, foreign original same-code, depth and unchanged publication | real_geometry_exact_one_less_and_foreign_original_preserve_view |

Missing/partial coverage refusal is actual, not a fabricated empty result. Genuine
first-structure continuous refusal happens at issuance, not by pretending that
consumer whole=None was observed. Different original use/copy/seed identities do
not become one group. Unreachable fake ambiguous records are not forged; accepted
lookup invariant/tests remain prerequisite. Source/result authority is private and
acyclic. Broader final task checkboxes remain unchecked until independent52 and
380-predecessor/native/lint/WASM gates, plus root frontend. Later production
issuance/provenance, density/admission and varying-seed envelope remain mandatory.

Compile-only014 original34193 terminal8f516c exit101 (foreign query fixture mut);
015 original58926 terminal9dd7e8 exit0;016 original47751 terminal771bc2 exit101
(fixture retained descriptor borrow crossed observer mutation);017 original47439
terminalebbcee exit0 with empty log. Every handle terminal before edits. Scoped
8-file fmt check0, all below1000. Narrow native-only dead_code annotations identify
actual currently pending consumer/type/owning methods; no module-wide allowance
or synthetic production calls were introduced. Author performed no runtime tests,
Clippy or WASM. Source ready for independent52-test diagnostic then final gates.


### Final matrix0006 diagnostic / original inherited timing selection0007

Independent original13641/98348c exit101:52 actual tests,51 PASS; the new selected
member witness failed before its consumer assertion when finding inner timing.
All936 unchanged. Raw006 SHA256 c1cc793de3b1c2fff2ae9a9754b0aaf48cd7e5e0d622a5d8e9c8ede83d73e47c;
final006 SHA256 0d1a5d61ce7e162c394d67aa079898a0b100ff05ce7abfe0240a98f883e6fb15.
The fixture searched only FrozenSourceOrigin.slice_timings, which is the outer
origin frame. Actual original inner timings are retained in inherited frames with
their own slice_timings getter. Selection now traverses both original outer and
inherited timing records and selects the genuine inner issuer. Original subject
revision distinction and exact Type/membership-message assertion are unchanged.
Production and every other fixture remain unchanged; no timing/member synthesized.
Root full eight pre-repair texts retained-geometry-before-repair-originals-0006.json
SHA256 0bbd7684e45d13c417f36bb839d9439baed9d43a884ff4aeb181793c1df0197d.
Author quiet compile018 original97739 terminal3a079f exit0 with empty log; scoped
childfmt check0 and all handles terminal. Independent52 retry pending, followed
by final broader gates; no geometry/full-song completion claim yet.


### Broad0007 actual397 PASS / strict lint-only0008 repair

Focused0007 original55347/d04b91 exit0 verifies all52. Broad original5753/3e143c
exit101 verifies397 distinct behavioral tests and native0; strict Clippy then
reports only collapsible_if in canonical.rs217. Full936 unchanged, final broad
SHA256 65ed6b5a8f854ae8c5ca54ddbd032e99e203b9cf8a1065ae6c9e543e586baa05.
The nested exact-row match now uses the equivalent left-to-right conjunction with
matched.replace(row).is_some as its last short-circuit operand. The first match
still stores the row without error; only the second exact match refuses. Budget
charge, producer/whole/START tests and all other behavior are unchanged. No lint
suppression added. Root full eight pre-lint texts SHA256
6ea8491a260a369027547b6e4b406a98caffba1a04ce5128d0e5c78750557459.
Author quiet compile019 original30123 terminala63723 exit0/emptylog, scoped child
fmt check0, all author/checker handles terminal before mutation. Final fresh broad
native/behavior/strict/WASM verification pending, so plan acceptance remains open.

### 2026-10-03 — Final independent acceptance, archive-ready

All three bounded geometry tasks are complete. Held0008 independent native,
strict all-target Clippy, pure WASM and eight-path scoped formatting passed.
Fresh 397 distinct behavioral tests passed, including all 52 owning occupancy
tests (35 predecessors plus 17 genuine geometry witnesses). Original checker
34928 terminated 8ed24a/0; all 936 source hashes remained exact. Final receipt:
`/tmp/vactr-retained-geometry-broad-final-008.json`, SHA256
`c9cbfb51522e9ec5b1ac537074fb0e3a780fb0936b0a65625827444be987b1fc`.

Root independently accepted fresh 590 editor tests across 78 files and build,
with produced dist WASM matching fresh artifact
`a3f42f0be82e5c3d5351c6be04bd79e5542b164d8805b9a08a7dcedde6073dbe`.
Original frontend handles 96354/874a8c/0 and 26983/fc36190/0 are terminal;
all 936 inputs unchanged. Root receipt:
`tmp/song-mode-riela/ROOT-editor-retained-geometry-20261003.json`, SHA256
`b02d037327cad32ae9dcb82deba66b82260c6ccb8b82548ca508a53365b48f20`.

Acceptance covers actual private retained configuration consumer calls, original
authority, source eligibility, grouping, projection, coverage and budgets. It does
not assert production playback/admission now uses this consumer. Query-issued
provenance, frozen proof handoff, immutable route authority, density/admission,
pre-Reserve retention and useful uniform varying-seed support remain mandatory
downstream. Continuous first-structure refusal is tested at its real request
boundary; no fabricated empty continuous geometry is claimed. Root owns archive
movement and README links. No Rust changes during this documentation closure.
