# Caller-metered source and nested route preparation

**Status**: In Progress
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Immutable consumers and authentic capture](../../design-docs/specs/design-song-mode.md#immutable-consumers-and-authentic-clock-capture)

## Purpose and dependencies

Preserve original work and depth through source-use and nested preparation,
including errors. Supply metered primitives for the immutable routing bridge;
do not duplicate certification or claim production wiring in this precursor.

- **Previous**: [Retained execution membership](song-mode-retained-execution-membership.md).
- **Next**: [Immutable route authority](song-mode-immutable-route-authority.md).
- **Related**: [Issued route resolution](song-mode-issued-route-resolution.md).

The five-path scope is Ready after membership0003 full acceptance. Author and
independent reviewer confirm extraction visibility and owning fixture privacy.
Save complete original texts before source execution; no additional paths are
released implicitly.

## Proposed exact manifest

| Path | Deliverable | Status |
|---|---|---|
| `src/song/source_uses.rs` | Legacy wrapper and metered certification export | Not Started |
| `src/song/source_uses/certification.rs` (new) | Cohesive existing certification engine extraction | Not Started |
| `src/song/routing/nested.rs` | Legacy wrapper and preparation registration | Not Started |
| `src/song/routing/nested/preparation.rs` (new) | Cohesive existing nested preparation extraction | Not Started |
| `src/song/routing/nested/preparation/meter_tests.rs` (new) | Owning direct and nested fixtures | Not Started |

Five candidate paths. Current parents source_uses968/nested956 require cohesive
extraction before additions. Keep every touched source below1000. Declare any
additional path before edits. The fixture child belongs under nested preparation
because routing's nested module is private; tests outside that subtree would
require an additional registration/visibility path. Confirm exports within the
declared parents rather than silently adding routing.rs.

Retain the thin private Certification and CoverCacheKey declarations in the
source_uses parent so the existing source_free sibling impl can use unchanged
fields. Move the complete engine impl into certification; give only its
cross-sibling admit/enter/node methods the narrow parent visibility actually
required by existing callers. Parent helpers remain available to descendants.
Do not edit conditional.rs or source_free.rs implicitly during extraction.

## Required declarations

Keep the public certify_source_uses signature and legacy successful behavior.
Add the private companion through source_uses:

```rust
pub(crate) fn certify_source_uses_metered(
    inventory: &FrozenRoutingInventory, pattern: &FrozenPattern,
    window: TimeSpan, limits: SongLimits,
    remaining: &mut u32, depth: u32,
) -> Result<FrozenSourceUseCover, Failure>;
```

The existing Certification engine must use the actual caller counter and
starting depth. Cover consumed_work is actual starting-minus-ending work.
Errors preserve all debits through that same mutable counter; no successful
cover is fabricated for an unsupported dynamic mapping.

Keep the existing nested preparation wrapper. The declared child supplies:

```rust
pub(in crate::song::routing) fn prepare_nested_covers_metered(
    inventory: &FrozenRoutingInventory, active: &[SongSourceRouteCover],
    limits: SongLimits, remaining: &mut u32, depth: u32,
) -> Result<Vec<SongNestedSourceRouteCover>, Failure>;
```

Initialize its ResolutionBudget with actual remaining/max_depth, start genuine
root traversal at inherited depth plus one, and write remaining back after
both success and failure. Bridge certification via existing with_remaining;
do not subtract cover.consumed_work again after the direct counter debit.
Preserve the original supplied SongLimits for certification: budget.limits()
reconstructs other fields from defaults and cannot supply that policy. Only
bridge its remaining counter. Do not subtract inherited depth from max_depth
and also pass that depth as the node starting depth.
Retain symbolic Repeat traversal, original source groups and legacy cover order.
Use checked depth arithmetic. No new dependencies, callbacks or default quota
are introduced by either metered adapter.

Keep existing depth conventions explicit: certification rejects depth at or
above max_depth, while ResolutionBudget rejects depth above max_depth. Fixtures
must prove both boundaries and their actual nested composition.

## Tasks

### TASK-001: Extract and meter source certification

**Status**: Not Started
**Parallelizable**: No

- [x] Confirm private interfaces and fixture placement within five paths.
- [ ] Move the existing complete certification engine without semantic drift.
- [ ] Preserve public wrapper behavior and actual cover metadata.
- [ ] Thread original remaining/depth and retain consumed work on every error.

### TASK-002: Extract and meter nested preparation

**Status**: Not Started
**Parallelizable**: No; depends on TASK-001

- [ ] Extract existing preparation traversal and preserve its legacy wrapper.
- [ ] Carry actual inherited depth/max_depth into roots and certification.
- [ ] Use direct caller work without fresh allowances or double charging.
- [ ] Write back actual remaining after failures; publish no partial covers.

### TASK-003: Genuine owning evidence

**Status**: Not Started
**Parallelizable**: No; depends on TASK-002

- [ ] Actual evaluated/frozen nested selected sources construct certificates.
- [ ] Compare successful legacy and metered covers in full.
- [ ] Measure sufficient/exact/one-less complete direct and nested work.
- [ ] Prove actual inherited-depth boundary and hop exhaustion.
- [ ] A genuine unsupported dynamic graph fails after work with actual debits
  retained; do not forge policy data merely to obtain an error.
- [ ] Independent focused/regression/native/lint/WASM/format gates pass held inputs.

## Completion criteria

- [ ] Both adapters and owning fixtures pass with exact source/terminal evidence.
- [ ] Actual failure debits and inherited depth are preserved in both layers.
- [ ] Following RouteBuilder/prepare adopts these adapters; unused helpers alone
  do not complete routing, admission or playback.
- [ ] Unsupported geometry and useful varying-seed admission remain full-goal work.

## Progress log

### 2026-10-03 — Two preparation gaps verified in current sources

Author review and root reads confirm nested.rs97 creates ResolutionBudget with
default max_depth and initializes each root at1; its remaining writeback is
success-only at225. source_uses.rs243 initializes a local default allowance and
returns consumed_work only on success. Route8 cannot fix these files implicitly.
The author proposes two cohesive engine extractions and a separate bounded
metering precursor. Root moves the candidate fixture child into nested's owning
subtree to avoid an undeclared routing visibility edit. No source release yet.

### 2026-10-03 — Author and independent extraction review confirm five paths

Author confirms thin parent Certification/CoverCacheKey declarations preserve
existing source_free access while full implementation moves to the child.
Nested-owned fixtures and routing-subtree re-exports avoid a sixth routing.rs
visibility edit. Independent reviewer identifies ResolutionBudget::limits()
resetting all policy fields other than work/depth; metered nested certification
must retain the original supplied policy and bridge only remaining. Both
reviews confirm the actual depth conventions, direct error debits and closure
writeback obligations. Source release still awaits membership broad/frontend
acceptance and final Ready intent; no concurrent metering edits.

### 2026-10-03 — Ready after membership0003 full acceptance

Membership0003 passes417 distinct Rust tests, native, strict all-target lint,
fresh WASM and scoped formatting. Root frontend590/78files and build pass
that exact artifact; all941 source hashes and top/deps/dist hashes match after
every checker and root handle is terminal. Root frontend receipt SHA256
6b82a04b27f0f96e23e3e0adc18ec0dc0dcaba247912cb0ba62012c94e53335a.
Only the declared five paths may change, with three new children and two parent
edits. Preserve complete original texts before extraction. Original policy,
inherited depth, error debits, no double charging and genuine full-operation
fixtures are mandatory. Production RouteBuilder/prepare adoption remains the
following phase; no unused helper alone proves complete routing.

### Extraction-only intent checkpoint

Moved the existing public certification function and complete Certification
implementation to its declared child; thin state/key remain owning parent.
Only admit/enter/node visibility widens to parent subtree for untouched
source_free sibling. Moved existing nested preparation function to declared
child with routing-scoped visibility and parent re-export. No metering semantics
changed in this checkpoint; full extracted texts retained in
route-preparation-meter-extraction-only-0001.json before semantic edits.

### Semantic implementation and owning evidence authored

Extraction compile001 original76002/2a9e63/101 caught module declarations
before inner documentation; ordering corrected only after terminal. Compile002
original84193/a10fb8/0 verified extracted engine availability. Root normalized
extraction audit matched both moved bodies after declared paths/visibility/
whitespace substitutions, audit SHA
49fd6afa7da5af5679fdc1bbb443d36f56f2c4f90667bf59829788d9c044d384.
No historical byte-equality claim.

Direct certification now borrows caller remaining and starts at actual incoming
depth; cover work is exact initial-minus-final. Nested preparation carries the
original full SongLimits separately from ResolutionBudget, whose limits getter
reconstructs defaults; it uses only the getter's remaining for writeback. Root
traversal starts at checked incoming depth+1, direct certification uses that
absolute depth without a second depth subtraction. Direct failure debits remain
in caller storage; nested closure writes back on success and failure; no second
cover consumed-work debit. Four genuine owning fixtures use actual isolated
candidate/prepare routing inventories and authenticated cover metadata, with
original chord/music intact and no frozen graph mutations.
Compile003 original80296/71f974/0 reported pending reexport and obsolete helper
warnings. Old certification_limits is now cfg(test) for unchanged validation
fixtures; the single private metered reexport has a documented pending consumer
allowance. No blanket helper/module allowance. Runtime validation pending.

### Source held0001 — compile-only checkpoint

Quiet author compile004 original26019 terminated6cc3a9/0 with empty log; all
four author handles are terminal. Scoped five-file format/check0. Full944 has
exactly two changed parents and three declared additions versus accepted
membership941; untouched source_free.rs remains byte-identical. Four genuine
fixtures are authored; independent runtime/native/lint/WASM verification pending.
Source APIs and tests do not yet wire actual issued route preparation or claim
production admission/varying support.

### Review repair0002 — real duplicate debit and stronger inherited witnesses

Focused001 actual4 passed original30087/6a8b48/0, full944 unchanged. Read-only
review found that the newly added entry state.enter(depth) calls admit(1),
then nonempty node calls enter again. The author's earlier claim that enter
was noncharging was incorrect; authoritative file SHA
b8eae386be87ad27d4df31e9311774672e05ce8212f270a44ad6380ecf2efa99
confirmed self.admit(1). New entry validation is explicitly noncharging while
original node checks/debits remain intact. Empty nested entry now checks the
original exclusive boundary even when no roots are traversed.

The historical witness uses the actual saved extraction entry in a test-only
function (name/cfg/module path changes only), against the unchanged original
Certification engine; it compares complete original cover and exact consumed
work, rather than two wrappers sharing the changed entry. Additional genuine
fixtures find smallest sufficient direct/nested depth from nonzero inherited2
and check exact/oneLess; explicitly check zero-work empty nested boundary; and
certify a real valid first-cycle cover over Sequence[good,bad] before full nested
traversal reaches the later actual dynamic selected child. Positive child
certification and traversal debits are required on BeyondCapability, with no
returned cover. No public graph mutation or fabricated certificate.

Quiet compile005 original82333/b521dc/0 emptylog; all handles terminal, scoped5
format/check0. Eight fixture names now authored. Independent runtime acceptance
pending; issued builder/routing consumption remains mandatory downstream.

### Seam witness held0003 — exact child failure debit

Focused002 original32392/44b093/0 passed actual8, all944 unchanged. Reviewer
identified that totalspent > directchildcost alone could pass despite lost child
debit. Test-only owning thread-local trace now observes the actual with_remaining
child call before propagating its result: genuine scope/track/payload id/window/
depth and before/after/failure. The existing dynamic child fixture resets/drains
its own thread trace, directly certifies the exact same payload/window/depth
starting at captured before, and requires exact childdelta equality and final
nested remaining == childafter. No production alternate traversal or authority
constructor is added; eight names unchanged.
Compile006 original19045/d8a379/101 caught wrong NodeId module path; corrected
after terminal. Compile007 original67810/212324/0 emptylog. All author handles
terminal; scoped5format/check0; independent seam runtime confirmation pending.
