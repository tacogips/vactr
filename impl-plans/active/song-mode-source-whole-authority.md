# Authentic selected-source whole authority implementation plan

**Status**: Completed
**Plan ID**: SONG-SOURCE-WHOLE
**Created / Last Updated**: 2026-10-02
**Session target**: 1–3 sessions
**Design Reference**: [Identity and query invariance](../../design-docs/specs/design-song-mode.md#identity-and-query-invariance), [Bounds and diagnostics](../../design-docs/specs/design-song-mode.md#bounds-and-diagnostics)

## Scope and evidence

Copy the actual pre-outer-transform optional whole span together with the exact
privately retained issuing handle through every selected-source origin. Preserve
public handle/import compatibility while preventing public handle replacement from
claiming another occurrence's whole duration. No score reconstruction, VM replay,
compact fingerprint, event-onset configuration key or quota reset.

Actual checker007 retained sampled inherited Rev failure, while38/39 route cases
passed. Evidence: `tmp/song-mode-riela/SONG-09/0031-source-whole-authority-preflight.md`
and `/tmp/vactr-song09-audible-cycle-independent-007/routes-tests.log`.
Current source selection retains event.part and original onset, but a subsequent
Euclid replaces emitted whole with its timing cell. Nonaligned fast3/grid4 and
long releases show why an inherited pre-sampler whole end is an independent scalar.
This prerequisite supplies authenticated metadata; it does not implement sampled
ReflectCycles or complete SONG-08D, SONG-08B, SONG-09 or finite playback.

**DOCUMENT ONLY**: Ready is a reviewed specification, not source authorization.
Root must issue a separate exact-hash seven-path source release. All existing Rust,
other plans, index/archive and Cargo remain held during drafting.

## Manifest

```json
{
  "planId": "SONG-SOURCE-WHOLE",
  "planPath": "impl-plans/active/song-mode-source-whole-authority.md",
  "dependsOn": [
    "SONG-07B",
    "SONG-07E",
    "SONG-07I"
  ],
  "writePaths": [
    "src/song/source.rs",
    "src/song/source_uses.rs",
    "src/song/source_uses/origin.rs",
    "src/song/routing/source.rs",
    "tests/song_source_whole.rs",
    "src/song/routing/nested.rs",
    "src/song/routing/components.rs",
    "impl-plans/active/song-mode-source-whole-authority.md"
  ]
}
```

## Module status and projected split

| Path | Current lines | Deliverable | Status |
|---|---:|---|---|
| src/song/source.rs |680|live authority, query admission, iterative chain copy|Completed; ROOT0334|
| src/song/source_uses.rs |976|origin child facade and borrowed matcher validation|Completed; ROOT0334|
| src/song/source_uses/origin.rs |absent|DTOs/getters/public wrappers/authority costs|Completed; ROOT0334|
| src/song/routing/source.rs |744|shared matcher authority reservation|Completed; ROOT0334|
| tests/song_source_whole.rs |absent|genuine query and hostile authority fixtures|Completed; ROOT0334|
| src/song/routing/nested.rs |715|pass each actual borrowed origin to reservation|Completed; ROOT0334|
| src/song/routing/components.rs |994|adapt three real reservation regression call sites|Completed; ROOT0334|

Split source_uses.rs **before additive growth**: move existing FrozenSourceOrigin,
FrozenSourceOriginFrame and public resolve_source_use/resolve_source_use_frame
wrappers intact to origin.rs, reexport their old paths. Parent retains cover, graph,
OriginView and UseSearch. OriginView/resolve_origin use pub(super) visibility for
child wrappers. Target parent<950, child<300, source<950, routing source<950 and
public tests<700 after formatting. No eighth source is implicitly writable; stop
and obtain a prior manifest amendment if any touched Rust would reach1000.

## Exact declaration contracts

Imports: existing EventHandle, TimeSpan, ProducerStep, Sound/FrozenSound,
InstrumentRoute, NoteCommitMode, SongLimits, Failure, Rc and existing cover/identity.
Struct declarations retain every existing field; only private authority is additive.

### src/song/source.rs

```rust
pub struct SongEventOrigin {
    pub source_part: TimeSpan,
    pub entry_trace: Vec<ProducerStep>,
    pub inherited: Option<Rc<SongEventOrigin>>,
    pub handle: EventHandle,
    pub original_instrument: Sound,
    pub route: Option<InstrumentRoute>,
    pub commit_mode: NoteCommitMode,
    pub(crate) issued_handle: EventHandle,
    pub(crate) source_whole: Option<TimeSpan>,
}
impl SongEventOrigin {
    #[must_use]
    pub const fn source_whole(&self) -> Option<TimeSpan>;
}
pub(crate) fn copy_origin(
    origin: &SongEventOrigin, remaining: &mut u32, max_depth: u32,
) -> Result<FrozenSourceOrigin, Failure>;
```

Capture event.whole at successful query_source's row conversion, before replacing
song_source and before any outer timing transformation. Precharge issued-handle
clone and optional whole scalars before cloning or allocating its Rc. Original
handle and privately issued handle initially agree in every frame. Inherited Rc
frames preserve their own original clock and whole. Existing scalar/routes/trace
and source seed semantics remain unchanged. All literal constructors currently
occur only in this owned file; update the internal provenance fixture faithfully.

copy_origin validates each live private pair during the complete iterative chain
admission pass, before allocating inherited Vec or copying any handle/trace/sound.
One bad inner frame fails the entire copy. No mutable aliases, fabricated opaque
handles or synthetic whole endpoints enter frozen DTOs. Depth200/300 behavior and
point/None representation remain explicit.

### src/song/source_uses/origin.rs and parent facade

```rust
pub struct FrozenSourceOrigin {
    pub source_part: TimeSpan,
    pub original_instrument: FrozenSound,
    pub handle: EventHandle,
    pub entry_trace: Vec<ProducerStep>,
    pub inherited: Vec<FrozenSourceOriginFrame>,
    pub(crate) issued_handle: EventHandle,
    pub(crate) source_whole: Option<TimeSpan>,
}
pub struct FrozenSourceOriginFrame {
    pub source_part: TimeSpan,
    pub original_instrument: FrozenSound,
    pub handle: EventHandle,
    pub entry_trace: Vec<ProducerStep>,
    pub(crate) issued_handle: EventHandle,
    pub(crate) source_whole: Option<TimeSpan>,
}
impl FrozenSourceOrigin {
    #[must_use]
    pub const fn source_whole(&self) -> Option<TimeSpan>;
    pub(crate) fn borrowed_view(&self) -> OriginView<'_>;
}
impl FrozenSourceOriginFrame {
    #[must_use]
    pub const fn source_whole(&self) -> Option<TimeSpan>;
    pub(crate) fn borrowed_view(&self) -> OriginView<'_>;
}
pub fn resolve_source_use(
    cover: &FrozenSourceUseCover, origin: &FrozenSourceOrigin, limits: SongLimits,
) -> Result<FrozenSourceUseIdentity, Failure>;
pub fn resolve_source_use_frame(
    cover: &FrozenSourceUseCover, origin: &FrozenSourceOriginFrame, limits: SongLimits,
) -> Result<FrozenSourceUseIdentity, Failure>;
pub(crate) fn source_whole_work(
    handle: &EventHandle, whole: Option<TimeSpan>,
) -> Result<u32, Failure>;
pub(crate) fn source_authority_validation_work(
    handle: &EventHandle, issued: &EventHandle, whole: Option<TimeSpan>,
) -> Result<u32, Failure>;
pub(crate) fn validate_source_whole(
    handle: &EventHandle, issued: &EventHandle, part: TimeSpan,
    whole: Option<TimeSpan>, remaining: &mut u32,
) -> Result<(), Failure>;
```

Keep Clone/Debug/PartialEq derives, no public whole setter/constructor/Default.
Public getters expose immutable issued scalar only. A clone retains authority;
replacing its public handle cannot replace issued_handle or source_whole.
Crate visibility exists solely for actual source issuance/copy and borrowed matcher.

Parent private borrowed view refinement:
```rust
#[derive(Clone, Copy)]
pub(crate) struct OriginView<'a> {
    pub(super) handle: &'a EventHandle,
    pub(super) issued_handle: &'a EventHandle,
    pub(super) original_instrument: &'a FrozenSound,
    pub(super) entry_trace: &'a [ProducerStep],
    pub(super) source_part: TimeSpan,
    pub(super) source_whole: Option<TimeSpan>,
}
impl OriginView<'_> {
    pub(crate) fn handle(&self) -> &EventHandle;
    pub(crate) fn authority_work(&self) -> Result<u32, Failure>;
}
pub(crate) const SOURCE_AUTHORITY_INSPECTION_WORK: u32 = 16;
pub(super) fn resolve_origin(
    cover: &FrozenSourceUseCover, origin: OriginView<'_>, limits: SongLimits,
) -> Result<FrozenSourceUseIdentity, Failure>;
```

After limits validation, debit validation_work and compare the complete public and
issued handles before UseSearch allocations or whole-dependent matching. Full
revision/track/placement/producer path/cycle/onset/tone equality is required. Some
whole must be nonreversed, start at issued occurrence onset, and intersect the
source-local part with exact point/half-open semantics. A source-local point at
whole.end is excluded; zero-width authentic whole uses its exact point semantics.
Do not constrain release end to finite owner duration or caller cover/window.
None remains None: validate identity and legal part without inventing a duration.
Failures are addressed Type/Overflow/FuelExhausted/DepthExceeded, not success.

### src/song/routing/source.rs

Actual caller inventory: production nested.rs:310 and the three private
components.rs regression calls at842/859/876. Include both real caller paths in the
manifest; no mutable authority sidecar, frame cursor or inferred producer lengths.
Reuse the existing OriginView instead of adding a conceptual authority type:
```rust
pub(super) fn reserve_source_search(
    cover: &FrozenSourceUseCover, payload: &FrozenPattern,
    origin: OriginView<'_>, base_depth: u32, budget: &mut ResolutionBudget,
) -> Result<SongLimits, Failure>;
```

Nested passes `origin.borrowed_view()` or the actual frame.borrowed_view() at each
outer→inner iteration. It retains original handle/sound/trace and existing geometry
clocks. Reservation derives placement length and complete private/public validation
cost from that borrowed view; it cannot pair one frame's allowance with another
frame's whole. Core wrappers use the same borrowed_view to avoid reconstructed
origins, cloned handles or synthetic scalars.

Debit SOURCE_AUTHORITY_INSPECTION_WORK before computing checked validation cost.
Take matcher-delta start AFTER this spent inspection. Charge returned authority_work
as reserved execution fuel along with existing symbolic search allowance; subtract
only the independently spent grid probe from the returned matcher delta. Therefore
actual caller debit=authority inspection+grid probe+returned matcher allowance.
Core validates again using that reserved allowance before allocations. No quota
reset/refund, default-budget increase or caller-context lookup.

components.rs remains994 at the current baseline. Adapt its three calls to borrowed
views and its debit assertion to add actual inspection overhead, preserve real
matcher execution and exact/one-less/cumulative assertions. Keep formatted file
strictly below1000. If it cannot fit, stop before source growth and request an
explicit eighth path `src/song/routing/components/source_authority_tests.rs` for
cohesive relocation of the existing source-reservation test with its needed helper
scope; this path is NOT authorized or manifested by this seven-path draft.

## Work and authenticity invariant

- All usize sums/conversions/debits checked before clone, Rc/Vec allocation or scan.
- Additional issued-handle copy words: both vector lengths plus their framing words,
  full revision/track/cycle/onset numerator+denominator/tone scalars, Option tag and
  four endpoint words for Some. Preserve existing entry-trace/sound/source_part cost.
- Validation separately charges both full public+issued vector lengths, all fixed
  handle scalars, part endpoints, whole option/endpoints and bounded comparisons.
  Comparing an attacker-swapped long path is not charged as a fixed-size scalar.
- Exact logical-word formulas: let P(h)=placement.len, O(h)=producer_ordinals.len,
  H(h)=8+P(h)+O(h) (six scalar words and two vector-length framing words), and
  W(None)=1 / W(Some)=5 (tag plus four rational endpoint words).
  source_whole_work(h,w)=H(h)+W(w), the additional private copy admission.
  source_authority_validation_work(h,i,w)=24+P(h)+O(h)+P(i)+O(i)
  +12*isSome(w). Fixed24 covers both handles'16 fixed words, part4, option1 and
  bounded validation bookkeeping3. Some12 covers endpoint4, onset2 and bounded
  span/part comparisons6. This is a conservative declared logical-word budget,
  not a wall-clock instruction count. Both path lengths debit before equality.
  Core validates/debits this V; reservation receives the actual borrowed view
  and reserves the exact same V. Fixed inspection16 is four vector-length reads,
  one Option-tag read, four checked size conversions and seven checked additions;
  this constant-time scalar inspection is separately spent, not reserved twice.
  No authority vector or new allocation is introduced.
- All-chain copying preadmits both validation and actual private handle/scalar copies;
  matcher charges validation once per real call; bounded scalar inspection itself
  consumes work in addition to reserved matcher execution. No spent probe included
  twice in a reusable matcher allowance, refund, fingerprint or new quota ceiling.
- Exact/one-less tests measure actual new consumed quota, not historic old totals.

## Dependencies and related plans

| Dependency | Required output | State |
|---|---|---|
|07B/07E/07I|opaque source identities, inherited frames and mapping covers|verified baseline|
|08D sampled Rev|actual007 failure and preflight036|retained, consumer still incomplete|
|Exact clock author|disjoint mutation cohort|join only after all writers held|

**Previous**: [Source-use certification](song-mode-source-use-certification.md), where available.
**Next**: [Route geometry](song-mode-route-geometry.md) separately amends its sampling
split to reflection.rs and consumes authenticated source_whole; no implicit edits
in that six-path future phase. Parent playback/host plans retain their status.

## Tasks

### TASK-001: Cohesive origin split and immutable live authority

**Status**: Completed
**Parallelizable**: No — shared DTO/copy ownership.
**Deliverables**: source.rs, source_uses.rs, origin.rs.

- [x] Fresh exact root release and before SHA receipt.
- [x] Split976-line source_uses before growth, preserve imports/signatures.
- [x] Capture actual whole plus privately issued handle before outer mapping.
- [x] Getter/private authority and live all-chain copy rejection before allocation.
- [x] Exact actual clone/scalar/comparison declarations, no source quota reset.

### TASK-002: Borrowed matcher and shared routing reservation

**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: No — same matcher/copy types.
**Deliverables**: parent matcher, origin wrappers, routing/source.rs, nested caller, components test.

- [x] Full identity and legal Some/None support checked before matcher allocation.
- [x] Outer/inherited borrowed wrappers retain original imports and no reconstruction.
- [x] Exact real frame work reservations cumulatively share caller fuel/depth.
- [x] Explicit actual-origin inspection cost is separate from reusable matcher allowance.
- [x] Unrelated direct routes, grid budgets and source certification stay unchanged.

### TASK-003: Genuine provenance/hostile budget proof and independent seal

**Status**: Completed
**Depends On**: TASK-001, TASK-002
**Parallelizable**: No — final held joined source.
**Deliverables**: tests/song_source_whole.rs and bounded owned private fixtures.

Exact test declarations (no bodies):
```rust
fn nonaligned_sampled_reflection_retains_authentic_pre_grid_whole();
fn long_release_whole_survives_point_and_reordered_queries();
fn same_onset_different_duration_handle_swap_is_rejected();
fn every_inherited_frame_rejects_swapped_issued_identity();
fn source_authority_copy_and_matcher_share_exact_quota();
fn absent_whole_is_faithful_and_external_authority_is_private();
```

- [x] Genuine frozen candidate base fast3→Rev→Euclid4/4: pre-grid whole[0,1/3)
  differs final cell[0,1/4), no getter derived from caller clipping.
- [x] Slow2 long whole, finite owner/clipped continuation, direct point, fractional,
  disjoint and reordered queries compare full issued handles and whole authority.
- [x] Same snapshot/revision genuine simultaneous rows with same onset and differing
  durations (e.g static stacked differently held analog streams) provide authentic
  swap; changing public handle fails even same-onset, both live copy and frozen
  matcher, outer and inherited cases. No forged opaque handle is evidence.
- [x] Exact/one-less query copy, chain copy, matcher and cumulative route work/depth;
  long public replacement producer paths are charged or rejected before allocation.
- [x] Real None if query supports it; otherwise explicitly labeled internal query
  fixture using genuine issued handle, plus compile-fail external privacy doctests.
- [x] Preserve200/300-depth, source-use/layout/sampling/routing/carrier regressions.
- [x] Mandatory independent quiet native/browser checks, strict Clippy, fresh nonempty
  inventories/unfiltered tests, supplemental nextest, scoped formatting/diff and
  pre/post/current hashes under stable all-writer hold. Retain every failure.

## Completion criteria

- [x] All three tasks and seven modules independently verified, every touched Rust<1000.
- [x] Public handle/getter/import compatibility retained; no mutation can forge whole.
- [x] Live/frozen authority mismatch rejected before clone/allocation or whole use.
- [x] Query/copy/matcher/routes use honest shared checked work and semantic depth.
- [x] No dependencies, wire changes, DSP changes, VM replay or full geometry waiver.
- [x] Final status requires actual independent terminal evidence and source seal;
  source-whole completion alone never marks sampled Rev or full Song playback done.

## Verification and execution rules

Use required rust-coding specialized author and automatic independent checker.
CARGO_TERM_QUIET=true via mise for all future Cargo. Nextest additionally
NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
NEXTEST_HIDE_PROGRESS_BAR=1. No Cargo or Rust edits are authorized by this document.
Future author records exact numbered immutable declaration intents before mutation,
scoped formatting, held sources/plans and terminal original process handles/logs.
Do not restart a missing observation or erase failed evidence. No Git/index/archive,
new paths/dependencies/parent-plan edits; any required path amendment is prior.

## Progress log

### 2026-10-02 — document-only Ready specification

Read accepted identity/bounds design, actual007 trace/preflight031 and proposal036;
inspected every actual origin literal, query copy, matcher wrapper and route caller.
Seven future Rust paths, three tasks; current source hashes captured in
`tmp/song-mode-riela/SONG-SOURCE-WHOLE/0001-document-before-intent.json`.
No Rust, other plans or Cargo changed. Root review and separate source release
remain required. Sampled Rev split/consumer repair and broader geometry unfinished.

### Document revision — actual caller inventory

Root rejected the initial mutable sidecar proposal before any Rust implementation.
This version uses the existing borrowed OriginView and manifests all seven actual
Rust paths, including production nested and three components regression callers.
No source/Cargo authorization. Additional current caller SHA/line receipts are
SONG-SOURCE-WHOLE/0005–0006; original inspected inputs remain unchanged.

### Implementation: ROOT0331 released

0007 records fresh exact source baselines before the owned seven-path implementation. Required Rust coding and implementation plan skills apply. DTO split precedes additive growth; all source and authority paths remain under shared checked work/depth. No Cargo until held mandatory review.

Private fixture declaration before batch0008: `authority_tests::origins() -> Vec<Rc<SongEventOrigin>>` builds real captured pure/Slow/Stack selected Parts and calls canonical `query_part` using a callback-refusing QueryVm. Internal None/end-boundary/chain-quota variants are labeled unit validation cases using genuine query-issued handles; public hostile swaps retain genuine simultaneous origins. Query charge converts the declared u32 cost exactly with u64::from, without quota reset.

### Implementation held readiness

All seven authorized modules now implement actual query-issued private handle/whole authority, iterative live chain validation before copying, public immutable getters, metered full-path matcher comparison, and borrowed real outer/inherited route reservation. Origin child preserves old public imports. Existing components remains994 lines; no extra path needed. Five public candidate fixtures and three new private canonical-query authority fixtures plus two compile-fail privacy examples are written, NOT executed. Fixtures retain nonaligned fast3/grid4 authentic original wholes, long release/point/reordered windows, genuine same-onset differing-duration public/live/inherited swaps, exact/one-less/cumulative query/copy/matcher quotas, and explicitly internal None/zero/end-boundary validation. Existing many-Copies shared matcher regression uses actual authority-aware reservation. Scoped formatting is checked; mandatory native/wasm/Clippy/behavioral verification pending. No author Cargo or source identity fingerprints. This metadata does not itself repair the sampled Reflection locator or complete routing/playback. Final held receipt00010 records exact seven Rust and own plan.

### 2026-10-02 — independently verified SourceWhole child

ROOT0334 reconciles raw public5/private3 nextest identities, privacy2 rustdoc
compiler checks and all71 held hashes. Native/host-wasm/strict Clippy and
actual candidate, provenance, reservation, use/layout/depth, trace, sampling,
preparation, clock7, DSP648 and audio14 gates pass. Original001 session52172
exit1 was a harness-only rustdoc suffix comparison; exact normalized names
prove both privacy tests passed, without rerunning successful commands.
Routes-only002 session95216 exit101 retains actual38PASS/1FAIL known sampled
Rev joint geometry failure. Separate finish/unique-nextest exit0 chunka1e6c6
proves scoped lines/format/diff and actual8PASS (public5/private3;1919 excluded
by filter). Raw receipts live in /tmp/vactr-sourcewhole-independent-001,
/tmp/vactr-sourcewhole-independent-002 and
/tmp/vactr-sourcewhole-finish-independent-001. No original processes remain.
SourceWhole is Completed; geometry consumer remains separate, as do full09,
neutral stages, actual preparation, bounded reuse, transport and export.
No full Song goal claim; archive/index changes deferred to SONG16.
