# Selected-source transform type contract implementation plan

**Status**: Completed
**Plan ID**: SONG-07C
**Plan Path**: impl-plans/active/song-mode-transform-type-contract.md
**Created**: 2026-10-01
**Last Updated**: 2026-10-01
**Design Reference**: [Accepted instrument transforms](../../design-docs/specs/design-song-mode.md#editing-contract)

## Intent and authoritative evidence

Complete the original finite song playback/Apply/mute/export objective. Actual
SONG-07B public-tests-tenth, session38308 terminal101, exposes a checker mismatch:
`transform-instrument ... {p -> s p > note 66.75}` expects Fn(ctl)->ctl but infers
Fn(pattern sound)->ctl. The runtime passes a selected SongSource pattern whose
real events are sounds with retained provenance. Accept its sound wrapper and
ordinary control transformations without weakening global type rules or purity.
The source-use graph already analyzes known Sound PParam children; rejecting this
accepted expression at checking would prevent reaching that behavior.
This source-grounded repair has not received a new Riela review.

## Manifest

```json
{
  "planId": "SONG-07C",
  "planPath": "impl-plans/active/song-mode-transform-type-contract.md",
  "dependsOn": ["SONG-03", "SONG-05", "SONG-07A"],
  "writePaths": [
    "src/types/song_rules.rs",
    "src/types/infer_call.rs",
    "src/types/natives_domain.rs",
    "src/ns/checked_callable.rs",
    "tests/song_checker.rs",
    "src/types/check.rs",
    "src/ns/namespace.rs",
    "src/ns/evaluator.rs",
    "impl-plans/active/song-mode-transform-type-contract.md"
  ],
  "sharedPaths": [],
  "sharedPathNotes": []
}
```

## Related plans and dependencies

| Dependency | Evidence | Status |
|---|---|---|
| SONG-03 | Existing finite song checking | VERIFIED |
| SONG-05 | Actual selected-source transform native | VERIFIED |
| SONG-07A | Source policy/family semantics | VERIFIED |

Previous: [Song checker](song-mode-checker.md).
Consumer: [Source-use certification](song-mode-source-use-certification.md).
No dependency on incomplete07B: this repair's fixtures use the existing checker
and candidate runtime, while07B consumes its corrected checker behavior.
SONG-08 final clearance still requires independently verified07B and this repair.

## Execution and preservation

Use the required rust-coding agent and Rust coding standards. The08B author may
own this disjoint repair while holding every08/08B Rust write. Only the eight Rust
paths above and own plan are authorized. No global unify.rs changes, runtime
coercion changes, extra paths, dependencies, lockfile, Git, broad formatting,
canvas work, index or archive edits. Every touched source stays below1000 lines.
Before writes record fresh exact design/plan/source hashes in immutable numbered
intent files under tmp/song-mode-riela/SONG-07C/. Recheck immediately before writing;
never restore stale files. Request a prior exact manifest amendment for any truly
necessary additional path. Owner alone updates this plan.
Record exact refined declarations before implementation. Keep checker changes
local to transform callback admission and actual sound-position checking.
Independently verify completed modification batches under an all-Rust write hold.
Keep partial evidence distinct from final readiness. Every foreground command
must be retained and terminal; timeout does not authorize restart or success.
Cargo uses CARGO_TERM_QUIET=true; nextest also uses NEXTEST_STATUS_LEVEL=fail,
NEXTEST_FAILURE_OUTPUT=immediate-final and NEXTEST_HIDE_PROGRESS_BAR=1.
Preserve previous failed07B log and unrelated dirty work.

## Module declarations and deliverables

Existing public boundaries remain unchanged:

```rust
pub enum Ty {
    // Existing variants retained; no new global coercion or public type.
    Fn(Box<[Ty]>, Box<Ty>),
    Pattern(Box<Ty>),
    Sound,
    // Other existing variants unchanged.
}
```

The existing transform native remains four positional arguments and Fn forcing
for the fourth; callback purity remains mandatory. Author records the exact local
scheme/checking refinement before code, based on actual selected-source input
and accepted sound/control result patterns. No implementation bodies in this plan.

| File | Deliverable | Status |
|---|---|---|
| src/types/song_rules.rs | Local accepted transform input/result validation and unchanged purity diagnostics | VERIFIED |
| src/types/infer_call.rs | Integrate transform-specific callback checking without broad generic relaxation | VERIFIED |
| src/types/natives_domain.rs | Reconcile exact transform declaration with actual runtime input | VERIFIED |
| src/ns/checked_callable.rs | Bounded complete captured callable dependency certification with weak stamps | VERIFIED |
| tests/song_checker.rs | Accepted wrappers/controls plus precise negative/regression fixtures | VERIFIED |
| src/types/check.rs | Export portable owned generalized checked function declaration schemes | VERIFIED |
| src/ns/namespace.rs | Validate committed function scheme metadata against actual binding and dependencies | VERIFIED |
| src/ns/evaluator.rs | Install checked metadata only after successful committed form evaluation | VERIFIED |

## Tasks

### TASK-001: Actual contract and declarations
**Status**: Completed
**Parallelizable**: Yes, disjoint from07B;08B writes remain held.
- [x] Read actual transform runtime, checker/sound inference and failed fixture.
- [x] Record exact declaration refinement and fresh immutable source baselines.
- [x] Confirm no global Any/unification workaround or parser change is required.

### TASK-002: Local repair and fixtures
**Status**: Completed
**Depends On**: TASK-001
**Parallelizable**: Yes, disjoint from07B.
- [x] Accept direct and named pure `{p -> s p > note 66.75}` and ordinary control/rate transforms.
- [x] Preserve source-input pattern semantics and output pattern validation.
- [x] Preserve rejection of effectful callbacks, wrong arity/input/result forms and unsupported capabilities.
- [x] Preserve ordinary live sound-first, keyword-kit and pattern type diagnostics.
- [x] Add actual checker and candidate fixtures, retaining prior failures.

### TASK-003: Author and independent verification
**Status**: Completed
**Depends On**: TASK-002
**Parallelizable**: No, stable all-Rust joined source window required.
- [x] Native check, strict all-target clippy, wasm check, scoped fmt, diff check.
- [x] Nonzero song_checker, song_candidate, song_natives and legacy type/sound suites.
- [x] Execute exact07B sound-wrapper reproduction once source batch stable.
- [x] Quiet nextest repeat, exact source hashes/line counts/logs/terminal handles.
- [x] Independent checker verifies unchanged seal; no unresolved findings.
- [x] Own plan Completed only after evidence, no full-song completion claim.

## Completion criteria

- [x] Accepted sound wrapper reaches actual candidate/source realization.
- [x] Existing accepted control transforms remain valid and purity remains enforced.
- [x] Wrong callback forms and legacy sound-first errors remain diagnostics.
- [x] Author and independent gates prove corrected contract on unchanged sources.
- [x] Exact evidence/current hashes and terminal processes recorded.
- [x] Source-use consumer notified; original playback/Apply/mute/export scope remains pending.

## Progress log

### Session: 2026-10-01 — source-grounded repair authorization
Root inspected actual failed07B fixture, transform scheme, native inference and
sound_call/sound_arg. Existing module sizes permit five-path local repair.
No Rust edit or test was performed by this plan creation. Index/archive remains
reserved forSONG-16. This is an implementation prerequisite, not a scope reduction.

### Refined local callback declaration
The canonical transform scheme is `fn part keyword sound (fn (pattern sound) -> ctl) -> part`, reflecting the actual sound-valued selected source. Only this native fourth argument uses local admission of a one-argument function over either `ctl` or `pattern sound`, returning either `ctl` or `pattern sound`. Complete function-type alternatives are tried with unification rollback and exactly one compatible alternative is committed; shared type variables remain consistent. No generic Any or global unification rule is introduced. Existing callback purity checks, ordinary sound-first/kit inference and runtime pattern validation remain. Numeric/finite callbacks, invalid input/result and wrong callback arity remain diagnostics. infer_call delegates only this callback position to song_rules; the manifest sound checker needs no relaxation for the demonstrated expression.


### Root amendment: committed checked function metadata

Actual candidate evaluation checks each expanded form against current Namespace.
Namespace.global_info currently sets scheme=None for every prior function, while
the whole-document checker retains its generalized scheme in Checker.live.globals.
The intact200-step eager function fixture therefore fails before traversal even
though whole-document checking succeeds. Independent partial verification retains
this exact failure;245 distinct fixtures passed and13 hashes were unchanged.
Neither07B nor07C is cleared. Root authorizes the three exact additional paths above,
bringing this plan to eight modules; no scope.rs/unify.rs/slot-consumer edits.

Retain actual checked generalized declaration schemes across successful forms.
Do not disable per-form checking: expansion/macros/imports may depend on previously
evaluated forms. Preserve the exact program, failed transaction rollback and
existing legacy diagnostic-only checker behavior. The owner records the exact
CheckResult export-field and namespace/evaluator method declarations before code.
Only newly declared checked functions with portable primitive/pattern schemes
are exported from the existing live map; no run-local free variable or nominal
TypeId becomes a portable proof. Any function declaration carrying checker errors
is not certified by this metadata.

Attach metadata to exact committed binding identity, value/Closure identity,
version and owner. Reject stale records after replacement/replay/mutation; failed
forms preserve prior valid records. Use bounded weak dependency stamps for actual
prior typed globals whose schemes support the declaration, including transitive
staleness, so a late-bound dependency replacement cannot retain a false proof.
Avoid unrelated-binding invalidation, extra mutable music aliases, strong retention
of retired environments and wholesale cloning of a namespace. No callback allocation
is added; these are control-side checker records. Existing forcing masks remain.

Add actual evaluate_song_candidate regressions for named wrappers and20/100/200
unannotated eager function chains, plus stale dependent function replacement,
failed replacement/rollback and invalid/nonportable declaration metadata. Keep
explicitAny, wrong input/result/arity and purity diagnostics. Ordinary pattern
coercion must not certify annotated Signal/Nil callback input as a sound source;
reproduce that independent concern and add a local category check if confirmed.
Legacy whole-document and per-form checks must both remain functional.

The authoritative partial ledger is
/tmp/vactr-song07bc-partial-independent-001/final-partial-results.json.
Final checks require new author seals and an all-Rust hold after this repair and
remaining07B work. ROOT0050 records exact prior manifest authorization; all source
paths remain unchanged by this document amendment. Index/archive/Git deferred.


### Root amendment: portable complete callable schemas

The untouched manifest.rs path is removed from future writes and ty.rs replaces
it, retaining exactly eight authorized Rust modules. Existing sound inference is
unchanged. GlobalInfo fields remain unchanged: adding keyword fields there would
force unrelated constructor edits. Instead default-empty CheckEnv callable-schema
sidecars retain checked session/qualified callable metadata. Current construction
uses Default/empty, so no constructor fanout is authorized or needed.

Retain keyword names and one portable generalized full function scheme covering
positional inputs, keyword inputs and result together. Shared variables must not
be generalized or instantiated separately for positional/keyword views. The
owner records the exact copied schema type/export-field and resolution method
signatures before code. check.rs can use existing session Binding.extra::Fn and
checked generalized binding; no scope.rs modification is authorized. Namespace
provides matching validated sidecars alongside unchanged GlobalInfo. infer_call
resolves local, session, qualified and open-import function metadata using the
actual binding and instantiates the complete scheme once for consistent positional,
result and keyword checking. Do not manufacture keywords or resolve stale sidecars.

Preserve ordinary functions with optional keyword defaults and named pure
transform wrappers. Unknown keyword/invalid arity/input/result, explicitAny and
purity diagnostics remain. A required keyword not supplied by transform invocation
must receive an honest diagnostic; optional keyword functions are not declared
unsupported merely to avoid complete metadata. Add real candidate named/default
keyword functions plus wrong keyword/shared-variable and replacement cases. Keep
legacy qualified/open lookup behavior. These additions support accepted function
composition and do not introduce a new parser or general effect/type system.

ROOT0051 records exact prior path trade; unchanged manifest.rs checksum is retained.
ROOT0050's three metadata paths remain authorized. Partial checker is terminal,
all13 source hashes matched, but its245 passing tests and one reproduced failure
are historical partial evidence. New final author/independent seals must cover
this complete eight-path metadata repair and unchanged current source batch.

### Exact portable metadata declarations
`CallableSchema { positional: usize, keywords: Vec<Rc<str>>, signature: Scheme }` stores one generalized function whose inputs are positional followed by keyword parameters; its result shares those same quantifiers. `positional_scheme()` produces the unchanged GlobalInfo view without independently generalizing keyword types. `CheckEnv.global_callables` and `qualified_callables` are default-empty sidecars, resolved by existing lexical/session/qualified/open precedence. `CheckResult.callables: BTreeMap<Rc<str>, CallableSchema>` exports newly checked declarations with no errors and portable bound variables; nominal IDs and free variables are not exported.

`VarSlotRef::checked_callable()` returns copied validated metadata only when binding id, owner, version, Closure identity and weak dependency stamps remain current. Namespace captures scalar/weak prior binding stamps before checking; post-commit installation selects actual compiled proto global dependencies (including nested protos and transitive certified dependencies), never all unrelated globals as validity constraints. Bounds apply to traversal/copying; missing/stale dependencies fail certification, leaving ordinary diagnostic-only evaluation unchanged. Failed forms install nothing, retained old valid metadata remains available after rollback. Aliases sharing an actual certified Closure may reuse its complete schema, without using names or pointer hashes as event identity. Namespace exports validated positional schemes and complete sidecars together. Evaluator installs only after successful committed form evaluation and propagation, rechecking target and dependency stamps.

`infer_call::value_check` instantiates a persisted complete schema once, splits its positional/keyword views, and relates its positional/result view to the inferred callee before checking keyword values. Local lexical bindings retain precedence. The callback category check is local: continuous Signal, Nil and scalar annotations cannot be accepted as sound-valued source inputs merely via ordinary pattern-step lifting. Existing whole-pattern/thunk views and unannotated eager construction remain accepted; no global coercion is changed.

### Session: 2026-10-01 — complete callable metadata and actual candidate evidence
Portable complete schemas are exported from successful checked declarations and installed only after committed evaluated bindings. Exact weak binding/Closure/version/owner and compiled global/transitive stamps reject stale proofs; tests preserve unrelated bindings and restore prior proof after failed replacement/rollback. Default keyword/alias metadata shares full-function quantifiers; qualified/open consumer fixtures bind the reader AliasEnv and assert nonzero forms. Actual imported loader certification remains supporting SONG-07D, not proven by these consumer fixtures.

Author-metadata-checker-ninth completed terminal0 with26 distinct fixtures. Actual20/100/200 eager candidate preparation/query succeeds on the authorized07B iterative freeze repair with default thread stack; named/default-keyword/alias callbacks preserve fractional notes. The strict Signal probe has a clean reader/expander, exactly two forms, one precise category diagnostic and actual candidate rejection; Nil and explicitAny remain invalid inputs. Valid-pattern-returning effectful inline callback retains exact purity diagnostic; named effectful callback fails in actual Query mode. Two whole-function rollback/shared-variable unit fixtures previously passed. Initial compile, malformed fixture, source-size admission, missing AliasEnv, native freeze overflow and assertion-bound failures remain retained; no empty/dropped fixture is accepted as proof.

Temporary own depth stage markers are removed. A fresh source review identified pending captured-default VarRef dependencies: Closure.captures must participate in checked metadata validity alongside proto globals. That bounded owned correction is reported before final hold/seal. Full native/wasm/Clippy/legacy/nextest gates and independent review remain pending stable joined07B/07C source. No complete phase, actual package certification or playback/export claim is made.

### Captured dependency correction declaration
The owned namespace dependency helper iteratively visits actual Closure captures, nested Fn/Thunk bodies/captures, proto globals/protos/constants and aggregate List/Dict/Struct/Variant values. VarRefs retain exact weak slot/value/owner/version stamps and inherited relevant dependency stamps; no callback or lazy body is executed. Each queued child/copy is admitted against the same one-million work bound before allocation, and depth stays256. Shared pointer visitation only bounds traversal and is not an identity certificate. Captured cycles and repeated aliases cannot reset budgets or recurse on the Rust stack. Persistent records contain weak stamps/copied type schemas only, not strong environments. The actual declared-default VarRef fixture must prove captured state, invalidation and failed rollback restoration.


### Root amendment: complete retained callable dependency traversal

All07C Rust and own-plan writes are explicitly held. ROOT0055 records fresh exact
prior hashes. Replace completed src/types/ty.rs in FUTURE writes with the new
src/ns/checked_callable.rs, keeping eight future Rust modules. Preserve all prior
legitimate ty.rs sidecars/schema declarations and keep that exact source hash in
the final historical-source preservation/seal evidence. No further ty.rs writes
are authorized. Its earlier declarations/status are historical work, not an extra
future module. Namespace orchestration and module registration remain owned;
no ns/mod.rs, dependency or other new source path is authorized.

Move cohesive checked dependency/callable/input records and certification traversal
into the helper; keep Namespace/evaluator APIs stable. Author records exact types,
visibility/module declaration and helper signatures before code. Scope includes
bounded exhaustive retained Value/Pattern/PParam/Step/Part/Song traversal, prototype
constants/globals/nested bodies, Closure captures/memo values and relevant aggregate
containers. Walk real PParam::Late/Const/Fn/Pat references and selected source Part
payloads. Weak slot/version/owner/actualClosure identity and inherited dependency
stamps remain exact; no strong environment or global pointer registry is introduced.
No callback, pattern query or lazy body may execute to obtain these stamps.

Every queued item/copy and persistent stamp/height record is admitted before
allocation. Keep depth256 and one-million work bounds. Completed shared-subgraph
height validation accounts for the incoming caller depth, rather than silently
skipping deeper aliases. Active reference cycles have explicit bounded treatment;
shared deduplication cannot reset fuel/depth. Symbolic Part repeats visit their
child once and do not expand repeats. Unknown reference-bearing containers cannot
be treated as dependency-free to obtain a green certificate.

Actual Sig has immutable primitive/telemetry selectors and Lag/MapRange Rc<Sig>
children, with no VarRef/Closure payload. Traverse/account relevant immutable child
structure or justify exhaustive dependency-free handling; do not invent mutable
Sig dependencies. Live/resource purity remains separately enforced. Add actual
keyword-default Pattern retaining a late mutable control cell, followed by mutation,
failed replacement/rollback and restoration. Keep the actual declared-default
VarRef capture fixture with an unshadowed native call; a keyword parameter named
note cannot be used as the native note call head. Retain prior invalid fixture logs.

The author gate also exposed existing actual lazy-sound candidate failures in
song_candidate.rs159/210: successful persisted function schemas reveal
fn(time)->optionalKeyword results formerly hidden behind global Any. Preserve the
existing accepted lazy sound source behavior through an appropriate local Sound
call view in already-owned infer_call.rs, while keeping strict sound-first/kit and
transform input/result category rules. Establish actual existing named lazy source
positive and typed invalid source negatives; optional callback results represent
rest/event behavior according to the existing query contract. Do not relax global
unify.rs, use Any as a waiver, delete existing fixtures, or silently assign this
inference repair to07D (whose manifest covers actual loader publication only).
Record exact local checking declarations before code. Global nominal/coercion
rules remain unchanged.

Every touched Rust source remains below1000 lines; any further split needs prior
exact authorization. Remove temporary diagnostics; update own progress/tasks;
provide exact source seals including historical completedty.rs hash. Full native,
strictClippy, wasm, actualcandidate/scopedlegacy and independent gates still require
completed stable07B/07C and all-Rust hold. Prior26 focused passes and native/wasm/
Clippy success are historical partial evidence; publicgate101 remains retained.
No phase clearance or full playback/export completion follows this amendment.

### ROOT0055 helper API refinement (before implementation)

`namespace::checked_callable` is a private child module registered from the
owned namespace file. It owns `CheckedDependency`, `CheckedCallable`,
`CheckedInputs`, and `callable_dependencies(closure, inputs, own) -> Option<Vec<CheckedDependency>>`.
Namespace orchestration retains committed-binding installation and schema lookup.
The walker follows actual prototype constants/globals, captures/memo, aggregate
values, every Pattern parameter/step/child, symbolic Part/edit payload and Song
Part. It traverses immutable Sig children without inventing mutable-slot edges.
All graph work, depth, child admission and cached alias heights are bounded;
no callbacks are evaluated. Persistent certificates contain only weak slot and
Closure stamps plus copied schemas. Symbolic repeats never expand.
Local lazy `s` callback admission checks the actual Ratio time argument and a
bounded sound-valued result view; strict transform-source input checking and
ordinary sound-first/kit rules remain.

### Session: 2026-10-01 — ROOT0055 complete retained dependencies and Sound compatibility
The authorized checked_callable child helper now follows prototype globals/constants,
Closure captures/memo, aggregate values, every Pattern/PParam/Step payload, symbolic
Part/edit/Song payload, selected-source Part, visual parameters and UGen children.
Immutable Sig selectors have no mutable-slot edges; Lag/MapRange children remain
bounded. Embedded by-value Pattern arrays use borrowed Rc plus exact index paths,
not recursive tree clones. Completed-subgraph heights include scalar leaves and
are checked against each incoming alias depth before reuse. Queued child/key/path
copies are charged before allocation against the same bounded work account.
Persistent metadata remains weak slot/Closure stamps plus copied schemas.

Actual declared-default VarRef and eagerly captured default Pattern late-cell
fixtures prove staleness and failed-replacement rollback. Symbolic Part/Song and
selected-source dependency fixtures prove no extra strong slot ownership; shared
alias depth and owned Pattern arrays200/bounded wide admission also pass.
The local Sound view admits actual Ratio-time callbacks coherently with their
result, including optional keyword/rest sources. It rejects time identity, invalid
input/result/arity and explicit Any; global pattern coercion remains unchanged.
Original candidate lazy-source fixtures159/210 now pass alongside wrapper/purity
and named20/100/200 candidate preparation/query. The earlier Result/bool compile
error, invalid str spelling and invented strong-count assertion are retained as
failed evidence and corrected by source-grounded fixtures.

Author-helper-integration-second12519 terminal0:29 checker plus14 candidate
fixtures; author-helper-bounds-third:3 private helper fixtures. The completed
retired ty.rs SHA remains read-only as ROOT0055 specified. Complete stable native,
wasm, strict lint, legacy, consumer reproduction and nextest gates begin next;
independent clearance and full playback/export remain pending.

The first complete author runner90699 stopped terminal101 at the legacy type
suite (135/136): speculative non-function Sound probing duplicated a MIDI
channel diagnostic. The retained legacy fixture was unchanged. The owned local
one-source Sound syntax walker now infers each leaf once; ordinary keywords,
kit lookup, blocks, step constructors and sound-pattern checks are identical,
while resolved Fn leaves use the coherent time view. Zero/multiple/splat cases
still use ordinary sound_call. Focused full legacy type rerun35431 terminal0:
136/136. Final stable gates repeat against the new source batch.

### Session: 2026-10-01 — sealed complete author verification
Author-final-ledger-002.json records all13 gates exit0, retained runner48681
terminal0; no active author command remains. Exact distinct counts: checker29,
candidate14, song natives19, actual source-use consumer17, helper3, rollback2,
legacy types136, session70 and pattern64 =354 distinct fixtures. Quiet nextest
repeats60 (checker29+candidate14+source-use17), allpassed/no skips. Native,
host-wasm, strict all-target Clippy, scoped fmt and diff checks pass. Source
hashes match the pre-gate002 seal unchanged; max866 lines. Retired read-only
ty.rs remains SHA8f5f456408617d6986ffcb71c0aca137153e5c9ff50a1cbfbc1701ede474fb74.

All eight current Rust paths and own plan are now held for mandatory independent
joined07B/07C review. Previous compile, fixture, namespace-freeze and duplicate
diagnostic failures remain available in their original logs; none is substituted
with skipped/empty fixtures. Actual package-loader metadata handoff is separate
SONG-07D. This plan remains In Progress until independent clearance. No08B
integration, playback/Apply/mute/export completion or Ready claim is made.

### Session: 2026-10-01 — independent joined clearance
ROOT0058 accepts /tmp/vactr-song07bc-independent-final-001/final-results.json:
574 distinct fixtures plus60 nextest repeats, every gate exit0, retained process
70199 terminal0. Root freshly matched all18 joined source hashes and both plan
hashes against the sealed author batch. No active checker process or unresolved
finding remains. SONG-07C is Completed; its Rust sources and retired read-only
ty.rs remain held. Historical partial/failing evidence remains intact. Package
loader certification is SONG-07D; routing08B resumes separately. Full song
playback/Apply/mute/export is not complete. No archive/index/Git changes.
