# Vactrol Core Implementation Plan

**Status**: Completed (implementation; all eleven tasks COMPLETED as of 2026-09-27; TASK-011, the Solid.js editor UI, landed via issue #6). Earlier: all ten tasks COMPLETED as of 2026-09-26 across issues #1-#5 and commits 0bee1fb, 3ab64b6, 04afe7d, d6fdc4b, 8aad85f). Manual confirmations pending user sign-off: examples/beep.rs and the REPL audible gates on the native host, worklet audio and the WebGL2 pane in a real browser, cargo tauri build and app run. The plan stays under active/ until those are confirmed.
**Design Reference**: design-docs/specs/design-implementation.md (all sections; section numbers cited per task)
**Created**: 2026-09-24
**Last Updated**: 2026-09-25

---

## Design Document Reference

**Source**: design-docs/specs/design-implementation.md, backed by
design-docs/specs/lang-reference.md, design-music.md, design-visual.md,
architecture.md (Decided items are binding).

### Summary
Implement the Vactrol processor as one Rust core: reader, expander,
static checker with inference, bytecode VM, scheduler, pattern engine,
DSP graph, LSP sharing the checker; a REPL; a session protocol; and a
TypeScript editor (browser + Tauri, Wasm core, AudioWorklet DSP) with
playing-step highlighting, inline diagnostics, and controller binding
as an editor/runtime affordance (no language syntax).

### Scope
**Included**: Live mode end to end, web-first; native desktop dev host
(cpal); wasm32 host; REPL before editor; editor after language; the
Decided package system (imports, MVS resolution, lock/cache/proxy —
design 5.7); the Decided synthesis models, builtin effect catalog
with bus/master chains, analyzers, and granular (design 12.4-12.6);
MIDI input (`cc`, `midi-notes`, learn) and MIDI clock/transport sync
in and out (design 11.7); host-tier capability advertisement (12.7);
the right-pane slider panel over the Decided site set including inst
defaults, in source-edit and overlay modes (design 13); DAW-style
parameter editors from per-builtin `EditorDecl`/`ParamMeta`,
display-only step grid and piano roll, and the Decided `#@`
directive-comment system with labels and learned-CC write-back
(design 13.5 — the earlier authority conflict with the
no-source-annotation constraint is ADJUDICATED by the author: the
constraint is scoped to language syntax, `#@` directives are the
Decided persistence, the external session file stays an optional
mode behind the same interface; the directive VOCABULARY is the
13.5 PROPOSED grammar, pending ratification).
**Excluded**: Frozen mode codegen, Cranelift JIT, Swift GLUE and
UniFFI (native-tier milestone; the `cpal` native dev host stays),
Ableton Link implementation (`ClockSource::Link` represented,
selection is a diagnostic), native-tier capability extensions (long
IRs, heavy densities, multichannel, offline render — CapabilitySet
diagnostics in scope, extended limits later), music-visual coupling
(parked in notes.md), raw GLSL, user macros. The former "module
system" exclusion is removed (packages are Decided).

### Ordering policy
Language and REPL first (TASK-001..007, 009), editor second
(TASK-010), web-first (wasm host is part of TASK-008, before the
editor). No CoreAudio/Swift GLUE work anywhere in this plan; the
native tier's Tauri/CoreAudio milestone follows the browser tier per
architecture.md Host Tiers.

---

## Modules

### 1. Core values — `src/value/`

**Status**: COMPLETED — TASK-001, design sections 5.1–5.4

```rust
pub enum Value { Nil, Bool(bool), Int(i32), Int64(i64), Float(f32), Float64(f64),
    Ratio(Ratio64), Keyword(KwId), Str(Rc<str>), List(Rc<ListVal>),
    Dict(Rc<BTreeMap<Key, Value>>), Struct(Rc<StructVal>), Variant(Rc<VariantVal>),
    Fn(Rc<Closure>), Native(NativeId), Thunk(Rc<Closure>), VarRef(VarSlotRef),
    Pattern(Rc<Pat>), Signal(Rc<Sig>), Inst(InstId), Tex(Rc<TexNode>), Range(RangeVal) }
pub struct ListVal { pub items: Box<[Value]>, pub prov: Option<Rc<ListProv>> }
pub struct ListProv { pub form_gen: FormGen, pub doc_revision: u64, pub elems: Box<[Span]> }  // design 5.4
// Foundation scaffolding owned by TASK-001 so the crate builds from day one:
// id newtypes (KwId, SymId, NodeId, FileId, Span, SrcRef, FormGen, InstId, OutId,
// CtlId, SlotId, TweakId), buildable placeholder shells for Closure, VarSlotRef,
// Pat, Sig, TexNode in their final module paths, and the empty Cargo features
// host-native / host-wasm / lsp declared in Cargo.toml.
pub struct Ratio64 { /* num: i64, den: i64; normalized, den > 0 */ }
impl Ratio64 { pub fn new(n: i64, d: i64) -> Result<Self, Failure>;
    pub fn checked_add(self, o: Self) -> Result<Self, Failure>; // sub/mul/div alike
    pub fn floor(self) -> i64;  pub fn to_f64(self) -> f64; }
pub enum Key { Num(NumKey), Kw(KwId), Str(Rc<str>) }   // Ord: numbers < keywords < strings
pub struct Interner;   // intern(&str) -> u32 ids for keywords and symbols
pub fn truthy(v: &Value) -> bool;          // exactly Nil and false are falsy
pub fn deep_eq(a: &Value, b: &Value) -> Result<bool, Failure>;
```

### 2. Reader and expander — `src/reader/`, `src/expand/`

**Status**: COMPLETED — TASK-002, TASK-003, design section 6

```rust
pub struct Span { pub file: FileId, pub start: u32, pub end: u32 }
pub struct Node { pub id: NodeId, pub kind: NodeKind, pub span: Span, pub children: Box<[Node]> }
pub fn prescan_imports(src: &str) -> Vec<ImportDecl>;  // phase 1: split top-level
    // forms by layout, recognize `import` forms syntactically — no alias context,
    // no evaluation; feeds PackageStore fetches and the LSP (design 5.7, 6.2)
pub fn read(src: &str, file: FileId, env: &AliasEnv) -> ReadResult;  // phase 2:
    // error-recovering, trivia side table (directive `#@` lines kept as trivia);
    // each form reads against AliasEnv = session aliases + EARLIER same-document
    // imports, so import-then-use in one fresh file reads (design 5.7, 6.2)
pub struct ReadResult { pub nodes: Vec<Node>, pub diags: Vec<Diagnostic>, pub trivia: Trivia }
pub fn expand(n: &Node, cx: &mut ExpandCx) -> Result<Node, Diagnostic>; // sugar -> kernel forms
```

### 3. Checker — `src/types/`

**Status**: COMPLETED — TASK-004, design section 7

```rust
pub enum Ty { Int, Int64, Float, Float64, Ratio, Bool, Str, KeywordOf(KeySet), Nil,
    Opt(Box<Ty>), List(Box<Ty>), Dict(Box<Ty>, Box<Ty>), Fn(Box<[Ty]>, Box<Ty>),
    Pattern(Box<Ty>), Signal, Path, Url, Sound, Named(TypeId), Any, Var(TyVar) }
pub struct HostManifest { pub samples: KeySet, pub synths: KeySet, pub controls: KeySet }
pub struct CheckResult { pub types: HashMap<NodeId, Ty>, pub diags: Vec<Diagnostic> }
pub fn check(program: &[Node], env: &CheckEnv, manifest: &HostManifest) -> CheckResult;
pub fn infer_masks(f: &Node, env: &CheckEnv) -> ForcingMask;  // types/masks.rs: local
    // parameter inference (Value|Fn|Late|Undetermined, forwarding propagation) —
    // a MANDATORY compile dependency consumed by TASK-005; this task's first
    // deliverable so TASK-005 proceeds in parallel after it lands (design 5.5)
pub struct Diagnostic { pub span: Span, pub severity: Severity, pub code: DiagCode,
    pub message: String, pub origin: Option<RunOrigin> }
```

### 4. Namespace, compiler, VM — `src/ns/`, `src/compile/`, `src/vm/`

**Status**: COMPLETED — TASK-005, design sections 5.5–5.6, 8, 13 (tweak slots)

```rust
pub struct VarSlot { pub name: SymId, pub kind: SlotKind, pub value: RefCell<Value>,
    pub version: Cell<u64> }
pub enum SlotKind { Let, Var, Fn, Tweak }
pub struct Namespace;  // define/redefine/lookup; prelude (read-only) -> session -> fn/block
                       // scope chain (design 5.6, 20 Q1)
pub struct FnProto { pub arity: Arity, pub code: Vec<Op>, pub consts: Vec<Value>,
    pub locals: u16, pub spans: Vec<(u32, Span)>, pub name: Option<SymId> }
pub enum Op { LoadConst(u16), LoadLocal(u16), StoreLocal(u16), LoadGlobal(u32),
    LoadGlobalRef(u32), LoadTweak(u32), MakeList(u16), MakeDict(u16), MakeClosure(u16),
    MakeThunk(u16), Call(u8), CallKw(u8, u8), CallValue(u8), Force, Deref,
    Jump(i32), TestLit(u16), TestVariant(u16), TestLen(u16), SplitRest(u16),
    BindField(u16), JumpIfNoMatch(i32), Fail(u16), Pop, Ret }
pub fn compile(n: &Node, cx: &mut CompileCx) -> Result<Rc<FnProto>, Diagnostic>;
pub struct Vm { /* stack, frames, fuel, depth_limit, effect_mode */ }
impl Vm { pub fn run(&mut self, proto: Rc<FnProto>, ns: &Namespace) -> Result<Value, Failure>; }
pub struct Failure { pub code: FailCode, pub message: String, pub origin: Origin }
pub struct TweakSite { pub id: TweakId, pub span: Span, pub slot: Rc<VarSlot>,
    pub initial: Value, pub ty: NumTy, pub tier: SiteTier, pub form_gen: FormGen }
pub enum SiteTier { Direct, Reeval, Manual }  // design 13
pub enum EffectMode { Normal, Query }        // Query fails global writes, scheduling,
                                             // host sends with origin; print captured,
                                             // not emitted; scope-guarded (design 10.4)
pub struct FormGen(pub u64);
pub struct DepGraph;   // REACTIVE dependency graph (design 5.6 revised): form ->
    // EAGERLY dereferenced top-level slots of ANY kind (Late captures = no eager
    // edge); triggers = redefinition AND top-level `upd`, coalesced per tick;
    // ACTIVE-OWNER registry, whole-write-set eligibility, non-replayable marking,
    // whole-form transactions, topological ROUNDS with DEPENDENCY-SET REPLACEMENT and STALE-READ VALIDATION (DEFINITION-ORDER tie-break for incomparable forms; stale reads reachable only through owners UNSCHEDULED at read time);
    // PASS JOURNAL (pre-pass value/version/owner per written slot; in-pass commits PROVISIONAL) + pass-level staging of ALL host-visible effects, with end-of-pass VALIDATION/ROLLBACK restoring invalid forms' writes and discarding their intents from any round;
    // DEREF-TIME Failed/Blocked checks (dirty-read ABORT + re-schedule under the round bound; back-edge = cycle diagnostic); EVERY unsuccessful evaluation — abort OR ordinary runtime failure — contributes THREE records and nothing host-visible: the pass state (Blocked or Failed), RECOVERY SUBSCRIPTIONS on the blocking-read set (empty for an ordinary fault), and the ATTEMPT EDGE SET of the attempt's successful reads (wake-up for Failed AND Blocked forms = committed edges UNION attempt edges UNION subscriptions; replaced per attempt, cleared on commit, rollback-surviving); STATUS EVENTS in BOTH directions (->Failed re-schedules in-pass readers and dirty-marks committed-edge dependents; Failed->Recomputed wakes dependents and subscribers), both BYPASSING the equality cutoff; FAILURE-SENSITIVE end-of-pass validation (a read is valid only if its owner ends Clean/Recomputed — pre-pass reads included);
    // EQUALITY EARLY-CUTOFF for value propagation only, ONE post-pass VALIDATED `bindings` batch, override migration (5.6 revised, 13, 14.4)
pub enum MaskEntry { Value, Fn, Late,
    Forward { links: Box<[(CalleeRef, u16)]> },  // symbolic dependency SET (fan-out);
                       // chased to ONE effective entry at the WRAPPER'S OWN call
                       // boundary against current bindings: any Value -> force once
                       // there (repeated/conditional/ignored uses alike), all Fn/Late
                       // -> pass deferred, unresolvable -> Undetermined; downstream
                       // redefinition changes the next call (design 5.5)
    Undetermined }     // memoizing at-most-once cell + latent-forcing diagnostic
pub struct ForcingMask(pub Box<[MaskEntry]>); // per-parameter, from MANDATORY local
                       // inference (types/masks.rs); Value/Fn/Late/chased-Forward
                       // never memoize; consulted at every Call (design 5.5)
pub struct PkgNs { pub id: PackageId, pub ns: Namespace }   // one child ns per package
pub struct ImportBinding { pub prefix: SymId, pub pkg: PackageId, pub open: bool }
// lookup: locals > session ns > open imports (most recent wins; collision = LSP
// warning, qualified spelling always available) > prelude (design 5.7)
```

### 5. Pattern engine, signals, visual chains, clock — `src/pattern/`, `src/tex/`, `src/clock/`

**Status**: COMPLETED — TASK-006, design sections 9, 10, 11.1

```rust
pub struct TimeSpan { pub begin: Ratio64, pub end: Ratio64 }
pub struct Event { pub whole: Option<TimeSpan>, pub part: TimeSpan, pub value: Value,
    pub controls: BTreeMap<KwId, Value>, pub src: Option<SrcRef>, pub occ: OccKey }
    // src: ORIGINATING SrcRef from ListProv, carried end to end, never re-wrapped (10.2, 11.6)
pub struct OccKey { pub path: SmallVec<[(NodeId, u32); 8]>, pub anchor: Ratio64, pub cycle: i64 }
    // stable logical OCCURRENCE IDENTITY (structural path ordinals + whole.begin + cycle — design 10.2 revised): equal across query windows for the same note, DISTINCT for identical stack/branch twins; keys the 11.3 occurrence records (coverage-extension merge, never deleting) and committed ledger (at most one emission per occ per slot/gen)
pub struct Pat { pub node: PatNode, pub span: Option<Span>, pub structured: bool }
    // structured: false for a single-sound subject until the first list-valued step (SOUND FIRST, 10.1)
pub enum PatNode { Steps(Box<[Step]>), Signal(Rc<Sig>), Sound { src, kit }, Fast(Rc<Pat>, PParam),
    Slow(Rc<Pat>, PParam), Rev(Rc<Pat>), Every(PParam, Value, Rc<Pat>),
    WhenMod(PParam, PParam, Value, Rc<Pat>), SometimesBy(PParam, Value, Rc<Pat>),
    DegradeBy(Rc<Pat>, PParam), Stack(Box<[Pat]>), Cat(Box<[Pat]>), FastCat(Box<[Pat]>),
    Superimpose(Rc<Pat>, Value), Off(Rc<Pat>, PParam, Value), Jux(Rc<Pat>, Value),
    Iter(Rc<Pat>, PParam), Chop(Rc<Pat>, PParam), Ply(Rc<Pat>, PParam),
    Striate(Rc<Pat>, PParam), Slice { pat: Rc<Pat>, cuts: SliceCuts, index: Rc<Pat> },
    Splice { pat: Rc<Pat>, cuts: SliceCuts, index: Rc<Pat> },
    LoopAt(Rc<Pat>, PParam), Fit(Rc<Pat>),   // region ops -> begin/end/speed/loop
    // controls, values-only; WHOLE-SPAN-anchored, layer-1 COVER-EQUIVALENT (chop subdivides the source whole and intersects; striate ranks cycle onsets by stable identity via query widening; slice index sampled at whole.begin; splice/fit/loop-at rate from whole-span seconds at COMMIT; whole=None = event-local fault); layer-2 emission uniqueness is the scheduler's occ merge (design 10.1/11.3 revised)
    Chunk(Rc<Pat>, PParam, Value), Grid(Rc<Pat>, Rc<Pat>),
    Euclid(Rc<Pat>, PParam, PParam, PParam), Control(KwId, Rc<Pat>, Rc<Pat>),
    ScaleNotes(KwId, KwId, Rc<Pat>), Segment(Rc<Pat>, PParam),
    Range(Rc<Pat>, PParam, PParam),
    MidiNotes { subject, channel } }  // live input pattern (a structure-giving step after `s`): empty under pure
    // query/dry run; the bind-time INPUT-LANE walk classifies operators between
    // this node and the sink — control/per-note-probabilistic nodes apply per
    // arriving note in tree order, structural/time operators over the lane are a
    // bind-time diagnostic, never a silent bypass (design 10.1, 11.7)
pub enum PParam { Const(Value), Late(VarSlotRef), Fn(Value), Pat(Rc<Pat>) }
pub enum SliceCuts { Equal(PParam), Manual(Box<[PParam]>) }  // slice starts, sorted in [0,1]
pub struct QueryResult { pub events: Vec<Event>, pub faults: Vec<Failure>,
    pub output: Vec<(Origin, Rc<str>)> }   // captured print (design 10.4)
pub fn query(p: &Pat, span: TimeSpan, cx: &mut QueryCtx) -> QueryResult;
    // never Err: event-local and subtree-local faults recorded with origin,
    // sibling events survive (design 10.3); cx runs in Query effect mode;
    // deterministic relative to the read snapshot (design 10.4)
pub enum Sig { Sine, Saw, Tri, Square, Rand, Perlin, IRand(i64), Time, Beat, Phase,
    Cycle, Host(HostSig), Cc { controller: u8, channel: u8 },   // MIDI CC 0..1 (11.7)
    Analyzer(AnalyzerId),                                       // 12.5 analysis cells
    Ctrl(KwId, KwId), Hits(KwId), Lag(Rc<Sig>, f64), MapRange(/*…*/) }
pub struct Tempo { pub bpm: Ratio64, pub beats_per_cycle: Ratio64 }
pub enum ClockSource { Internal, MidiClock, Link }  // Link: represented, selection is
                                                    // a "not available" diagnostic in v1
pub struct Clock; // piecewise-linear host-seconds <-> Ratio64 cycles; anchors on tempo
                  // change; :midi slaves the anchor to smoothed MIDI clock (design 11.7)
pub struct TexNode { pub kind: TexKind, pub params: Box<[VParam]>, pub span: Option<Span> }
pub enum TexKind { /* Hydra sources, geometry, color, blend, modulate, chain — design 9.1 */ }
pub enum VParam { Const(f32), Late(VarSlotRef), Sig(Rc<Sig>), Pat(Rc<Pat>), Fn(Value) }
pub fn compile_tex(t: &TexNode) -> Result<(ShaderDesc, UniformPlan), Failure>;  // design 9.2
pub struct ShaderDesc { pub source: String, pub uniform_names: Box<[String]>,
    pub assets: Box<[TextAsset]> }   // RENDER-SAFE: strings + plain metadata only
pub struct TextAsset { pub id: u32, pub text: String }  // TexKind::Text(Rc<str>) payload
pub struct UniformPlan { pub specs: Box<[UniformSpec]> }  // EVALUATOR-owned, never crosses
pub struct UniformSpec { pub name: String, pub src: VParam }
```

### 6. Scheduler, slots, capability hosts — `src/sched/`, `src/host/`

**Status**: COMPLETED — TASK-007, design sections 11.2–11.6 (issue #3, plans BE-CONTRACTS/BE-SCHED/BE-MIDI, reconciled by BE-FINAL session 186)

```rust
pub struct SlotTable;  // d1..d9 == slot 1..9; out o0..o3; slot :name; hush/stop over all kinds
pub enum Binding { Pattern(Rc<Pat>), Texture(Rc<TexNode>) }      // design 9, 11.2
pub struct Slot { pub key: KwId, pub kind: SlotKind2, pub bound: Option<Binding>,
    pub pending: Option<Binding>, pub gen: u32, pub orbit: OrbitId, pub muted: bool }
pub struct StagedEvent;  // evaluator-side, Value-level controls; converted to POD only
                         // inside commit_lead; revocable until then (design 11.3)
pub enum SlotKind2 { Audio, Visual, Midi(u8), Osc(Rc<str>) }
pub struct Scheduler;  // tick(host_now): prospective boundary activation of pending
                       // bindings, boundary-split queries, staging invalidation on
                       // rebind and on every control write (structural liveness),
                       // commit at commit_lead, SlotControl revocation, captured
                       // query output forwarding, telemetry, at/once thunks (design 11.3)
#[repr(C)] pub struct AudioEvent { pub time: f64, pub slot: SlotId, pub gen: u32,
    pub inst: InstId, pub voice_hint: u32,
    pub n_ctl: u8, pub ctl: [(CtlId, Ctl); MAX_CTLS] }
#[repr(C)] pub enum Ctl { Const(f32), Cell(CellId) }  // Cell read at voice start
    // (control-rate: continuously); TIER-SPECIFIC transport (design 11.3):
    // native = atomic f32 in shared process memory (next-event holds to one
    // buffer); browser = mirror table in the worklet updated by CellUpdate
    // batches on the priority port (delivery-dependent: heard after the batch
    // applies, ~ upd + L_ctl; the gap is flagged for adjudication)
pub struct ControlCells;  // evaluator-authoritative preallocated cells; also backs
    // cc input and inst-default sites (11.7, 13); mirror-side INCARNATION STATE
    // MACHINE per design 11.3 revised: Vacant/Live/Retiring + last_epoch, CellInit
    // epoch > last_epoch = the ONE authorized transition, INIT-ONCE per epoch
    // (replay acknowledge-only), updates only on the exact Live epoch; un-acked
    // ref commits as Ctl::Const (the stated bounded EXCEPTION, cell path resumes
    // after ack);
    // seq-numbered batches, 1 in-flight + 1 pending with latest-wins coalescing,
    // one batch per process() under the 16.1 credit, full-snapshot resync on
    // reconnect; retire-ack + epoch gates id reuse
pub struct VoiceRelease { pub tag: VoiceTag }   // NoteOff -> releases exactly the
    // tagged voice IF STILL SOUNDING, regardless of later slot-gen bumps; NoteOn
    // starts and releases share ONE FIFO priority channel, tombstone ring catches
    // release-before-start; session records EVERY NoteOn (filtered flag) for
    // off-matching; Natural on an OPEN voice = enter release (design 11.7 revised)
pub struct PlayingEvent { pub slot: KwId, pub beat: Ratio64, pub time: f64,
    pub src: Option<SrcRef>, pub dur: f64 }
pub struct SlotControl { pub slot: SlotId, pub new_gen: u32,
    pub effective_time: f64, pub release: Release }
pub enum Release { None, Natural, Panic }    // rebind / stop / hush; release governs
    // voices started BEFORE effective_time; stale-started voices (late delivery) are
    // ALWAYS short-gated on receipt. Per-slot channel: one immediate + one future
    // entry; the IMMEDIATE entry is a MONOTONE MERGE (gen = max, effective_time =
    // min, release = max severity None < Natural < Panic) so hush-then-stop keeps
    // Panic; the future entry replaces only itself; re-send until SlotControlAck,
    // idempotent under the merge. Event-batch gen piggyback enables ONLY the
    // default drop/gate policy — it cannot reconstruct effective_time or release
    // (design 11.3, narrowed)
pub struct SlotControlAck { pub slot: SlotId, pub gen: u32 }
pub trait AudioHost { fn send(&mut self, ev: AudioEvent);
    fn control(&mut self, c: SlotControl); fn now(&self) -> f64;
    fn swap_graph(&mut self, g: GraphHandle); fn analysis(&self) -> HostSigs; }
pub trait MidiInHost { fn poll(&mut self) -> &[MidiInEvent]; }  // cc/note/clock/
    // transport input, drained per tick on the evaluator (design 11.7)
pub trait RenderHost { fn set_program(&mut self, o: OutId, sh: ShaderDesc);
    fn set_uniforms(&mut self, o: OutId, u: &Uniforms); }
pub trait MidiHost { fn send(&mut self, ev: MidiEvent); fn control(&mut self, c: SlotControl); }
pub trait OscHost { fn send(&mut self, ev: OscEvent); fn control(&mut self, c: SlotControl); }
// MidiEvent and OscEvent carry slot + gen exactly as AudioEvent does (design 11.4)
pub trait SampleLoader { fn load(&mut self, bank: KwId) -> Result<Arc<SampleBank>, HostError>; }
pub struct NoopHost;   // implements all traits as no-ops; dry-run target
pub fn dry_run(b: &Binding, session: &Session) -> QueryResult;  // one cycle vs NoopHost in
    // Query effect mode; provably leaves live state unchanged (design 10.4, 11.5)
```

### 7. DSP graph and audio hosts — `src/dsp/`, `src/host/native/`, `src/host/wasm/`

**Status**: COMPLETED — TASK-008, design sections 12, 16 (issue #3, plans BE-CONTRACTS/BE-DSP/BE-INST/BE-NATIVE/BE-WASM, reconciled by BE-FINAL session 186; the audible `examples/beep.rs` check is pending user confirmation)

```rust
pub struct InstDef { pub id: InstId, pub params: Box<[(CtlId, Ctl)]>,  // defaults:
    // Const or Cell — cell-backed defaults ARE the inst-default slider sites,
    // reconciled with design 11.4/13 (was f32)
    pub nodes: Box<[UGenSpec]>, pub edges: Box<[Edge]> }
pub enum UGenSpec { SinOsc, Saw, Pulse, Tri, WhiteNoise, Lpf, Hpf, Bpf, Delay, Comb,
    EnvPerc, EnvAdsr, Line, SamplePlay(BankRef), Mul, Add, Const(f32), Param(CtlId),
    Vco { unison_max: u8 }, SubOsc, Ladder, Svf, FmOp, FmMod, PhaseDistortion,
    Additive { partials_max: u8 }, Wavetable(TableRef), Granular(GranSrc),
    Effect(EffectSpec) }   // Decided synthesis ugens + effects-as-ugens (design 12.4-12.6)
pub struct EffectSpec { pub kind: EffectKind, pub params: Box<[(CtlId, Ctl)]> }
pub enum EffectKind { /* the Decided builtin catalog: dynamics, eq-filters, delay,
    reverb (incl. Convolution(IrRef)), saturation, modulation, lo-fi, resonator,
    spatial, restoration, Granulate, utility, Analyzer(AnalyzerKind) — design 12.5 */ }
pub struct BusDef { pub id: BusId, pub chain: Box<[EffectSpec]> }  // bus :name: / master
pub struct BusGraph;   // preallocated bus units; slots route via the `bus` control;
                       // swap = same generation+refcount lifecycle as graphs
pub struct GrainPool;  // per-instance, ceil(max_grain_density x max_grain_size)
    // slots at graph install; shared by Granular ugen and Granulate effect; static
    // sources vs LIVE bus circular CAPTURE buffer (max_capture_seconds, single
    // writer), grains spawn only inside the sliding valid window (guard = grain
    // len + one block; violations skipped + counted); FREEZE stops the write head
    // (frozen content readable) / pins static position; phase-accumulator onsets,
    // deterministic under seed; pool-full or beyond-cap = skip + diagnostic,
    // never alloc or stale read (12.6 revised)
pub struct CapabilitySet { pub max_voices: u16, pub max_ir_seconds: f32,
    pub max_grain_density: f32, pub max_grain_size: f32,
    pub max_capture_seconds: f32,
    pub multichannel: u8, pub offline_render: bool,
    pub midi_in: bool, pub midi_out: bool, pub file_access: bool }  // 12.7: beyond-
    // capability use = diagnostic with origin, never a crash or silent truncation
pub struct VoicePool;  // preallocated; no alloc/lock/Rc in the audio callback
pub struct OrbitFx;    // per-orbit effect sends; params from controls, smoothed
pub struct EventRing;  // lock-free SPSC, POD AudioEvent; full = drop + count + report
pub struct NativeAudioHost; // cpal stream + timer tick + midir (session socket: TASK-009, B5)
pub struct WasmHost;   // raw extern "C" ABI main-thread half (no wasm-bindgen, B5); worklet half runs dsp only;
                       // postMessage handoff, transferables, worklet-as-timebase clock
pub struct SampleArena;  // worklet-side, preallocated fixed capacity; SENDER-PACED
                         // one-slice (64 KB) install window with SliceOk/Installed
                         // acks over the entangled port; AGGREGATE budget of
                         // INSTALL_BYTES_PER_QUANTUM copy work per process() call,
                         // credit replenished only on audio progress, acks withheld
                         // until deferred copy runs, deferred-queue-overflow diagnostic;
                         // admission checks; UNIFORM generation-aware refcounted
                         // retirement for samples AND graphs (queued events + voices +
                         // graph refs; Retired messages before free-list return); inst
                         // node-count cap; no memory growth after init (design 16.1)
```

### 8. Session, REPL, LSP, editor — `src/session/`, `src/lsp/`, `editor/`

**Status**: COMPLETED — TASK-009 COMPLETED (issue #4; the audible REPL gate is pending user confirmation); TASK-010 COMPLETED (issue #5; hearing worklet audio, the real-browser visual pane and the Tauri app run are pending user confirmation); design sections 13–16

```rust
pub struct Session { /* interner, namespace, checker env + manifest, slot table, clock,
    scheduler, rng seed, tweak table, diagnostics bus, telemetry, hosts */ }
impl Session {
    pub fn eval(&mut self, src: &str, file: FileId, doc_revision: u64) -> EvalOutcome;
    pub fn tick(&mut self, host_now: f64);
    pub fn apply(&mut self, msg: ClientMsg) -> Vec<ServerMsg>;
}
pub enum ClientMsg { Eval{/* code, file, span, doc_revision */},
    DocChanged{/* file, doc_revision, base doc_revision, new-revision dirty spans +
                  change set, edit_epoch — edit invalidation + coord mapping, 14.4 */},
    Hush, Stop{slot: String},
    SetVar{name: String, value: JsonValue, defining_form_gen: u64, edit_epoch: u64},
    SetTweak{id: TweakId, form_gen: u64, value: f64, edit_epoch: u64},
    Subscribe{..}, ManifestReq }
pub enum ServerMsg { EvalResult{/* value, diags, tweak sites with tier + form_gen */},
    Diag{..}, Playing{events: Vec<PlayingWire> /* SrcRef with doc_revision */},
    StaleBinding{id: TweakId, current_form_gen: u64},
    Levels{..}, Tempo{..}, TweakSites{sites: Vec<TweakWire>} }
pub fn repl_main() -> !;   // vactrol repl: console over Session; _1.._n console registers
pub fn lsp_main() -> !;    // vactrol lsp (feature "lsp"): tower-lsp over read()+check()
pub struct PackageId(pub Rc<str>);  // "github.com/owner/name" (design 5.7)
pub struct PkgManifest { pub package: PackageId, pub vactrol: Option<Rc<str>>,
    pub deps: Vec<(PackageId, Rc<str>)>, pub assets: Vec<Rc<str>> }  // vactrol.toml
pub struct LockEntry { pub id: PackageId, pub version: Rc<str>, pub sha256: [u8; 32] }
pub trait PackageStore {  // host capability, IO side: native = git tag fetch +
    // ~/.vactrol/pkg cache; browser = Go-proxy-shaped HTTPS + OPFS cache; ONE
    // canonical content digest, INJECTIVE length-prefixed records over the sorted
    // file tree (u32_be(len(path)) || path || raw sha256(contents); never archive
    // bytes or newline records — design 5.7 revised) shared by all stores; entry
    // validation FIRST (relative normalized UTF-8 paths, no control characters,
    // no ../ or absolute, no link/device entries, no duplicates incl. case-fold,
    // bounded count/size), assets
    // contained under the package root; staged extraction, digest verified, then
    // ATOMIC cache publication (no partial entries); any violation = integrity
    // diagnostic even when the digest matches (design 5.7 revised, 17)
    fn resolve(&mut self, roots: &[(PackageId, Rc<str>)]) -> Result<Vec<LockEntry>, HostError>;
    fn fetch(&mut self, e: &LockEntry) -> Result<PkgSources, HostError>; }
pub fn mvs_resolve(reqs: &[(PackageId, Rc<str>)]) -> Result<Vec<LockEntry>, ResolveError>;
    // minimal version selection over git-tag semver, as Go; `vactrol get` CLI verb
    // writes vactrol.lock; import failure = load diagnostic, session continues
pub struct Directive { pub span: Span, pub kind: DirectiveKind,
    pub attach: AttachTarget, pub body: DirectiveBody }  // `#@` trivia parsed by
    // the SESSION, never the language; Positional attaches per the Decided
    // indentation rule, Addressed is position-free (design 13.5)
pub enum DirectiveKind { Positional, Addressed }
pub struct DirectiveTable;   // per-document directives + file-level settings
pub struct LabelRegistry;    // explicit `#@ name` / trailing `#@ label:` labels +
    // implicit labels (let/fn/inst/bus/look, slots)
pub struct BindingKey { pub label: LabelId,
    pub site: Option<(SymId, u16)>,   // selected call-site name + 1-based ordinal;
    pub param: KwId }                 // None = declared-definition parameter.
    // FULL resolved selector path as the document-scoped binding identity —
    // hats.lpf / hats.hpf / lpf.1 / lpf.2 are DISTINCT keys; persistence,
    // overlays and MIDI maps all key on BindingKey ((label, param) withdrawn);
    // label survives line moves, ordinal re-derived via doc-changed span mapping
    // (reorder migrates, ambiguity = STALE); positional TweakId fallback (13.5)
pub trait BindingPersistence;  // TWO impls behind one interface: Directive
    // (ADJUDICATED default: setup + learned CCs written back into `#@`
    // comments) and ExternalFile (optional performer preference — publish a
    // file free of setup comments); the authority conflict is resolved (13.5)
pub struct EditorDecl { pub kind: EditorKind, pub params: Box<[ParamMeta]> }
pub enum EditorKind { /* EqCurve, FilterResponse, DynamicsTransfer, EnvelopeShape,
    DelayTaps, ReverbRoom, SamplerWave, WavetableFrames, GranularRegion, LfoShape,
    StereoField, XyPad, EuclidRing, ProbabilityDial, LengthHandle, Scalar —
    design 13.5 */ }
pub struct ParamMeta { pub ctl: CtlId, pub range: (f32, f32), /* curve, unit,
    group — ranges/kinds come from HERE, never written in directives */ }
```

`editor/`: TypeScript, CodeMirror 6, Vite; browser build loads the Wasm
core + AudioWorklet; Tauri shell wraps the same frontend (design 15).
Modules: `core-bridge.ts` (wasm/socket, one ClientMsg/ServerMsg codec),
`highlight.ts` (playing-step decorations scheduled on the audio clock),
`diagnostics.ts`, `transport.ts` (tempo/levels/hush, clock-sync
status), `bindings.ts` (the RIGHT-PANE SLIDER PANEL over every
enumerated site incl. inst defaults, grouped by site origin, each
slider in source-edit or overlay mode — design 13; mouse-drag on
literals, WebMIDI learn, position-tracked bindings re-keyed per form
generation, validated write-back edits), `meters.ts` (analyzer
meters/scopes from 12.5 cells), `packages.ts` (import UI, `vactrol
get`, proxy config, package diagnostics — design 5.7), `render.ts`
(WebGL2 RenderHost, ping-pong
feedback buffers — design 9.4), `samples.ts` (bank browser), Tauri
config with minimal allowlist (design 17).

---

## Tasks

### TASK-001: Core value model
**Status**: COMPLETED | **Parallelizable**: Yes (foundation; nothing precedes it)
**Depends on**: —
**Deliverables**: Module 1 (`src/value/*`) including `ListVal`/`ListProv`;
the foundation scaffolding: id newtypes (`KwId`, `SymId`, `NodeId`,
`FileId`, `Span`, `SrcRef`, `FormGen`, `InstId`, `OutId`, `CtlId`,
`SlotId`, `TweakId`) and buildable placeholder shells for `Closure`,
`VarSlotRef`, `Pat`, `Sig`, `TexNode` in their final module paths
(placeholders are compilation scaffolding only, replaced by their
owning tasks and never depended on for behavior); the empty Cargo
features `host-native`/`host-wasm`/`lsp` declared so every
verification command is valid from this task onward. Unit tests for
ratio arithmetic/overflow-failure, Key ordering (numbers < keywords <
strings), dict put/join/iteration order, truthiness, nil-punning, deep
equality.
**Design ref**: design-implementation.md sections 5.1–5.4.
**Completion criteria**:
- [x] `Value`, `Ratio64`, `Key`, dict ops, `Interner` implemented as specified
- [x] Ratio ops exact and self-reducing; i64 overflow and zero denominator are `Failure`
- [x] Dict iteration is key-ordered; duplicate literal key keeps last pair
- [x] Foundation newtypes, placeholder shells, and the three feature declarations compile natively and for wasm32
- [x] `CARGO_TERM_QUIET=true cargo build` and nextest pass (session 175 final logs)
- [x] `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown` passes

### TASK-002: Reader
**Status**: COMPLETED | **Parallelizable**: Yes (with TASK-001; only Span/ids shared)
**Depends on**: TASK-001 (interner, literal values)
**Deliverables**: Module 2 reader half: lexer (decided identifier rule,
operators, ratio literals, string interpolation scan, tabs-only indent),
layout rules (one line = one call; trailing `:` block; `{}` inline;
leading-`>` continuation; head-position `>` is greater-than; pair vs
block `:`; `( )` reader error; `_1` console-only), the `import` form
and QUALIFIED NAMES gated on the session-supplied alias set (design
5.7, 6.2), error recovery, trivia table. Creates the spec fixture
manifest skeleton (`tests/fixtures/spec/manifest.toml`, see
Verification) with reader-level classification of every spec code
block.
**Design ref**: sections 5.7, 6.1–6.3.
**Completion criteria**:
- [x] All manifest fixtures parse per their classification (console forms under `FileId::Console`); `illustrative-excluded` blocks are excluded or carry expected reader diagnostics in the manifest — no blanket clean-parse claim
- [x] Every negative example (`( )`, `_tmp`, `foo-`, `_1` in a file) yields a reader diagnostic, and following lines still parse
- [x] `import path`, `as alias`, `open` read correctly; the TWO-PHASE frontend makes a FRESH-SESSION file containing `import ... vactrol-pads` followed by `pads.warm` read correctly (and its `as pd` variant); a qualified name with no session or earlier-form import remains a reader error; `prescan_imports` finds every import without evaluation; `#@` lines land in trivia with positions
- [x] Spans are byte-accurate (asserted in golden tests)
- [x] Build + nextest pass (session 175 final logs)

### TASK-003: Expander and kernel forms
**Status**: COMPLETED | **Parallelizable**: No
**Depends on**: TASK-002
**Deliverables**: Module 2 expander half: if/elif -> match (single
truthiness site), binding if -> match, for -> map (value discarded),
call-site pairs and `& opts` splat, `x ? d` -> `or`, `-x` -> `{neg x}`,
string interpolation -> concat; kernel-form validation (`match fn let
var upd -> {} enum` + calls only); span preservation on synthesized nodes.
**Design ref**: section 6.4.
**Completion criteria**:
- [x] Desugarings match the `# ~ (...)` and `# ==` annotations in lang-reference.md
- [x] Output tree contains kernel forms only (asserted structurally)
- [x] Build + nextest pass (session 175 final logs)

### TASK-004: Static checker with inference
**Status**: COMPLETED | **Parallelizable**: Yes (with TASK-005; both consume TASK-003 output)
**Depends on**: TASK-003
**Deliverables**: Module 3: HM-lite unification with let-polymorphism,
numeric literal kind adaptation + decided widening lattice, `?T` flow
and `?` defaults, keyword typing against `HostManifest`,
`types/masks.rs` as the FIRST deliverable — the mandatory local mask
inference (Value|Fn|Late|Forward-links|Undetermined, design 5.5) that
TASK-005's compiler consumes, including the static `mixed-forcing`
check over fan-out link sets,
`any` narrowing rule, `types/deps.rs` — ADVISORY static dependency
edges per top-level form (free variables split eager/late via the
mask analysis) for LSP hover and subscription precomputation;
runtime recording stays authoritative (design 5.6 revised),
no-shadowing enforcement PER SCOPE with the scope chain prelude ->
session -> fn/block (design 20 Q1 DECIDED 2026-09-25: a child scope may
shadow a parent binding; prelude shadow = hint, user-parent shadow =
warning; same-scope rebinding = error; the prelude is read-only), `path`
and `url` literal types with file-relative path resolution, qualified-name typing
through `PkgNs` and package keyword sets in `HostManifest` (design
5.7), diagnostics list (undefined names, literal `/ x 0`, missing enum
variant in match, annotation mismatch, duplicate dict key,
open-import collision warning, beyond-capability use (12.7), purity
warning for effectful calls in pattern arguments — the static
complement of the enforced Query effect mode).
**Design ref**: section 7; architecture.md Typing, Realtime Validation.
**Completion criteria**:
- [x] Fixtures per the spec fixture manifest type-check according to their classification; annotated negatives (`+ 1 "a"`, `?T` used as `T`, rebinding) produce the expected diagnostic codes in isolated sessions (spec_fixtures `blocks_evaluate_per_classification`, `cases_evaluate_per_expectation`; cases `eval-neg-type-add`, `eval-neg-optional-as-value`, `eval-neg-rebinding`; `types/tests/check_basic.rs::annotated_negatives`)
- [x] Checker never aborts evaluation in Live mode (returns diagnostics + best-effort types) (`types/tests/no_abort.rs` 5 tests; `vm/tests/integrate_sound.rs::check_diagnostics_of_a_loaded_file_keep_their_spans_and_never_fail_the_load`)
- [x] Same `check()` entry consumed by tests for LSP reuse (no LSP dependency in `types/`) (`types::check` used by `types/tests`, `tests/support/eval.rs` and `Evaluator::eval_form`; no lsp/tokio code in `src/types`, `[dependencies]` empty)
- [x] Build + nextest pass (session 183: `target/fe-logs/final-build-s183-1.log`, `final-nextest-s183-1.log` 489 passed, `final-cargotest-s183-1.log`, all exit=0)

### TASK-005: Namespace, compiler, bytecode VM, tweak slots
**Status**: COMPLETED | **Parallelizable**: Yes (with the rest of TASK-004)
**Depends on**: TASK-003 and TASK-004's `types/masks.rs` deliverable (mask inference is a MANDATORY compile dependency, not an opportunistic input; the remainder of TASK-004 proceeds in parallel)
**Deliverables**: Module 4: `Namespace` (single global scope including
the prelude as a READ-ONLY PARENT SCOPE of the session namespace;
no-shadowing per scope, child scopes may shadow — design 20 Q1 decided
2026-09-25; `sound-kit`/`default-sound-kit` prelude bindings and
`load path` per lang-reference.md and design-music.md), late-bound `VarRef` load rules (var/fn
ref, let snapshot), redefinition, `FormGen` stamping and the
REACTIVE `DepGraph` machinery (eager-read recording over ALL
top-level slot kinds via `Deref` instrumentation, Late captures
excluded; triggers = redefinition AND top-level `upd`; write-set
ownership eligibility, non-replayable marking, WHOLE-FORM
all-or-nothing transactions, DAG topological propagation with diamond dedup and cycle diagnostics,
the PASS JOURNAL with end-of-pass VALIDATION/ROLLBACK and pass-level staging of ALL host-visible effects, DEREF-TIME Failed/Blocked checks with dependency REDISCOVERY under failure, EQUALITY EARLY-CUTOFF,
override migration — design 5.6 revised, 13);
compiler (match compilation via pattern ops, keyword args/splats,
per-parameter `ForcingMask` obtained from the mandatory
`types/masks.rs` inference — `Forward` link SETS chased to one
effective entry at the WRAPPER'S OWN call boundary against current
bindings, design 5.5; the rev-4 inner-call-site deferral and the
earlier syntactic fallback are both withdrawn);
VM (frames, fuel, depth limit, call-boundary forcing per the callee's
CURRENT mask incl. `Late` capture, boundary-chased `Forward`, and
memoized `Undetermined` cells, `Deref` demand points,
`EffectMode::Query` enforcement failing prohibited effects with origin
and capturing `print`, failure unwind with `Origin`); `upd` for locals
and globals; `LoadTweak` + per-`FormGen` `TweakId` assignment and
site-tier classification (direct vs reeval, design 13) for the
Decided site set — pattern/control literals, top-level bindings, and
`inst` PARAMETER DEFAULTS as `Ctl::Cell`-backed sites (function-body
literals stay out of scope); `PkgNs` child namespaces, `ImportBinding`
alias table, qualified-name compilation and the 5.7 lookup order
(package LOADING pipeline is TASK-009's; this task owns the namespace
and compile side); list-literal provenance (`ListProv`); core prelude
natives (lists/dicts/values/console from lang-reference section 5,
with fuel-metered iteration and lazy ranges), each carrying its
forcing mask.
**Design ref**: sections 5.5–5.6, 8, 13.
**Completion criteria**:
- [x] All `positive` fixtures in the spec fixture manifest produce their annotated `# => v` values via read->expand->compile->run; `diagnostic` fixtures produce their expected codes in isolated sessions; `authority-question` and `illustrative-excluded` fixtures are tracked with dispositions, never silently reinterpreted (see Verification) (evidence: spec_fixtures `cases_evaluate_per_expectation`, `blocks_evaluate_per_classification`, `authority_questions_are_pinned`, all through `Evaluator::eval_form`; spec errata are pinned with TOML comments, e.g. `eval-fn-call` = 12 for `* a 12`, while the arity question stays pinned as `aq-fn-arity`/`aq-let-upd-base`)
- [x] Forcing-mask tests: an ignored effectful thunk argument runs exactly once; a doubly-read `Value` parameter forces once; a wrapper that forwards its parameter TWICE to a numeric consumer runs an effectful counter thunk EXACTLY ONCE (boundary chase, not per-use forcing); a CONDITIONAL wrapper whose forwarding call is skipped STILL forces exactly once at its boundary; boundary effect ORDER is left-to-right; a wrapper forwarding to `at` passes the thunk through unevaluated; a time-varying visual argument (`osc {* 20 {sin time}}`) stays deferred via its `Late` mask; a FAN-OUT forward whose destinations currently disagree (`Value` vs `Fn`) forces once and carries the `mixed-forcing` diagnostic; an `Undetermined` parameter runs at most once (memoized) with the `latent-forcing` diagnostic; mask derivation with the diagnostics pass disabled yields identical masks; redefining the DIRECTLY-called callee between `Value`/`Fn` switches at the next call; redefining only a DOWNSTREAM callee an unchanged wrapper forwards to (change `at` among `Value`/`Fn`/`Late`, `later` untouched) switches `later`'s forcing at its next call (transitive-staleness regression) (evidence: `src/vm/tests/forcing.rs` one test per clause; `src/vm/tests/integrate_tex.rs::a_time_varying_visual_argument_stays_deferred_via_its_late_mask` (real `osc`, counting stand-in for `sin`); the `mixed-forcing` warning on the same fan-out source is asserted by `types/tests/diags.rs` and by the ME-FINAL probe P6 through the evaluator)
- [x] Package namespaces: qualified `pads.warm` compiles through `PkgNs`; the 5.7 lookup order holds (locals > session > open imports most-recent-wins with collision warning > prelude); re-import replaces the whole `PkgNs` (evidence: `src/ns/tests/pkg.rs` qualified-name, lookup-order and re-import tests; the collision warning is `types/tests/diags.rs::import_collision_warns_on_prelude_and_open_overlap`)
- [x] Query effect mode: `upd` on a global inside a queried closure fails with `effect-in-query` and origin; frame-local state remains legal; `print` is captured into `QueryResult.output` and never written to a host from Query mode; the scope guard restores Normal mode after an unwinding failure (evidence: `src/vm/tests/query_mode.rs`, `src/vm/tests/integrate_query.rs::fn_param_calls_back_in_query_mode`, `query_mode_is_restored_after_a_failing_call`)
- [x] Rebuild tests: a PARTIALLY superseded multi-output form (binds d1 and d2, then another form replaces d1) is INELIGIBLE for automatic rebuild and cannot steal d1 back — its sites report `manual`; a form that ran `once` is excluded and reports `manual`; a bind-then-fail form retracts its STAGED pending bind (no half-installed bind survives) along with its namespace overlay and keeps the old binding; an acyclic diamond dependency rebuilds each shared descendant once in topological order (dedup, not flagged) while a true back-edge terminates with a cycle diagnostic; repeated controller updates on a computed literal converge via override inheritance (evidence: `src/ns/tests/rebuild.rs` one test per clause; namespace-overlay rollback `a_failed_standalone_form_rolls_back_its_namespace_writes` and `reactive_traces.rs::trace_2_*`)
- [x] Reactive propagation (design 5.6 revised; the unambiguous VAR-based case, not the authority's let/upd example): `var root 60`; `let raised + root 7`; a pattern form eagerly reading `raised` bound to a slot — `upd root 62` recomputes `raised` to 69, rebuilds the pattern form, stages the slot re-bind for the boundary, and marks the display batch (asserted end to end); the TRANSITIVE chain and a DIAMOND over a var propagate once per node in topological order; EQUALITY CUTOFF: an `upd` producing an unchanged derived value schedules no dependents, and unrelated branches are untouched (recompute counters asserted); a var read ONLY through Late captures triggers NO rebuild on `upd` (heard via the cell path instead — no-eager-edge asserted); a FAILED recomputation keeps the previous committed value and binding playing and reports with origin; CHANGING-EDGE case (design 5.6 rounds, abort-policy trace): Astra's conditional pair (`A = if switch then B else root`, `B = if switch then root else A+1`; coalesce `switch=true, root=20`) — A's deref of the scheduled B is a DIRTY-READ ABORT (asserted: no stale value read, no intermediate A commit), B commits 20, A re-runs after B; ends with A=20 AND B=20, final dependency sets {A -> switch,B; B -> switch,root}, and only final slot intents staged; a change making the edges genuinely cyclic terminates with the cycle diagnostic and previous values retained; FAILED-DIAMOND case (design 5.6 joins): `left = 1/root` fails at root=0 while `right = root+1` commits, and `total = left + right` is BLOCKED — previous committed total retained, NO slot intent staged for it, state `blocked-on: left`; repairing root recomputes left, unblocks and recomputes total, and stages its intent (recovery asserted end to end); controller-rate `upd` streams coalesce latest-wins per tick; ABORT/RETRY-FAILURE case (design 5.6 trace 1 — the prior claim that this pair commits a provisional A=1/3 is WITHDRAWN as unreachable under the abort gate): `A = if switch then 1/B else root`, `B = if switch then 0 else A+1`, `root = 2` — after `upd switch true`, assert A's DIRTY-READ ABORT on B (nothing commits, A never holds 1/3, no A intent is ever staged), B=0 commits, A's retry FAILS on 1/0, A reports Failed with its previous committed 2 standing (asserted that NO journal restore occurs, because none is needed); REACHABLE PROVISIONAL-ROLLBACK case (design 5.6 trace 2 — the journal's end-to-end regression, reachable because the invalidating owner is UNSCHEDULED at read time): definition order `X = if n > 0 then 1/(12 - Y) else 7`, `Z = if n > 0 then 5 else 3`, `Y = Z + 7` over `var n 0` — `upd n 1` schedules only X and Z; assert X reads the UNSCHEDULED Y (10) and PROVISIONALLY COMMITS 1/2 with a staged slot intent (the provisional value's existence asserted at the transaction layer), Z commits 5, Y recomputes to 12 and dirty-marks X, round-end validation flags X's STALE read, X's retry FAILS on 1/(12-12), the journal RESTORES X to the pre-pass 7 (value, version, owner asserted), ALL of X's staged intents and effects from any round are REMOVED (no bind, revocation, or cell update reaches the recording host), X reports Failed, and Z=5 and Y=12 commit as valid independent branches; CONDITIONAL-UNBLOCKING case (design 5.6 deref-time joins): `root = 1`, `switch = true`, `broken = 1/root`, `selected = if switch then broken else 7` — `upd root 0` fails broken and blocks selected AT ITS ACTUAL DEREF of broken; `upd switch false` re-runs selected, which never dereferences broken: selected commits 7, its edge set is REPLACED by {switch}, and its Blocked state CLEARS while broken stays Failed and unrepaired (final values, states, and dependency sets asserted); SWITCH-TOWARD case (design 5.6 recovery subscriptions): with broken already Failed and selected committed 7 over edges {switch}, `upd switch true` re-runs selected, which ABORTS at its newly discovered deref of broken and blocks with a RECOVERY SUBSCRIPTION on broken (asserted: broken is absent from selected's committed edge set, the subscription is present, and the last successful values/edges/read-versions survive the abort); repairing broken WAKES selected through the subscription and it commits — both directions, toward and away, asserted; STATUS-RECOVERY BYPASSES EQUALITY CUTOFF: `upd root` 1 -> 0 -> 1 makes broken fail then recover to its retained pre-failure value 1 — assert the Failed->Recomputed STATUS EVENT re-runs blocked dependents and recovery subscribers despite value equality (total and the blocked-toward selected retry, commit, and clear their badges); LATE-FAILURE case (design 5.6 trace 3 — failure-sensitive validation plus the ->Failed invalidation event; the producer fails with NO value or version change): definition order `X = if n > 0 then Y + 1 else 7`, `Z = if n > 0 then 0 else 1`, `Y = 1/Z` over `var n 0` (initially X=7, Z=1, Y=1) — `upd n 1` schedules X and Z; assert X legally reads the UNSCHEDULED Y=1 and PROVISIONALLY COMMITS 2 with a staged intent, Z commits 0 and dirty-marks Y, Y FAILS on 1/0 retaining value 1 and writing NO version, Y's ->Failed transition RE-SCHEDULES X (in-pass reader), X's retry ABORTS at its deref of the Failed Y, validation finds X invalid (consumed read's owner ended Failed), the journal restores X to the pre-pass 7, X's staged intents are removed, and final states are X Blocked-on Y with 7, Y Failed with retained 1, Z=0 committed (the provisional 2 asserted absent from every host-visible surface); NEWLY-DISCOVERED-SELECTOR case (design 5.6 attempt edge set): `broken` already Failed, `a = false`, `b = true`, `selected = if a then (if b then broken else 7) else 5` committed 5 over edges {a} — `upd a true` re-runs selected, which successfully reads a and b then ABORTS on broken; assert the attempt edge set is {a, b} and the subscription is {broken}; `upd b false` (a slot in NO committed edge set) WAKES selected through the attempt edge set, it re-runs, avoids broken, commits 7, and clears Blocked while broken stays unrepaired; also assert the attempt edge set is cleared on commit and replaced by the new attempt on a repeated abort; ORDINARY-FAILURE RECOVERY case (design 5.6 unsuccessful-evaluation metadata — the abort-only scoping is withdrawn): `var a false`, `var b 0`, `selected = if a then 1/b else 5` committed 5 over edges {a} — `upd a true` runs selected, which SUCCESSFULLY dereferences a and b (no Failed/Blocked/dirty owner anywhere) and then FAILS on division by zero; assert selected is Failed with its retained 5, its attempt edge set is {a, b}, and its blocking-read set/subscriptions are EMPTY; `upd b 1` (a slot in NO committed edge set and NO subscription) WAKES selected through the attempt edge set, it recomputes 1, commits, clears the Failed state and diagnostic, replaces its committed edges with {a, b}, and drops the failure metadata — automatically, with no manual re-evaluation and no change to a; also assert the failed attempt published no write or effect merely to retain its dependencies (evidence: `src/ns/tests/reactive_basic.rs`, `reactive_traces.rs`, `reactive_recovery.rs`, one named test per case; the end-to-end case with a real pattern form (`d1 {s :pluck > note [raised]}`: note 69, one rebind, one batch) and trace 2's no-revoke/no-bind host surface are additionally shown by ME-FINAL probes P4/P5)
- [x] Failure cases (division by zero, `+ 1 "a"`, no matching clause, unbounded `for x 0..:`, recursion depth) unwind, report origin, and leave the session usable (evidence: `src/vm/tests/failures.rs` six tests with code, origin span, clean unwind and a follow-up `+ 1 2`; `src/ns/tests/evaluator.rs::a_failing_form_leaves_the_session_usable`)
- [x] Redefinition + `upd` observable through `VarRef` demand semantics; `direct`-tier tweak writes observable without re-eval; `reeval`-tier writes trigger dependency-aware re-evaluation of the owning form (evidence: `src/ns/tests/namespace.rs::upd_is_observed_through_var_ref_demand`, `redefinition_is_observed_by_the_next_call`; `src/vm/tests/tweak.rs::a_direct_write_is_heard_at_the_next_read_without_re_evaluation`; `src/ns/tests/rebuild.rs::a_reeval_tweak_write_re_evaluates_the_owning_form`)
- [x] Build + nextest pass; wasm32 build passes (session 183: `final-build`, `final-nextest` 489 passed, `final-cargotest`, `final-wasm32`, `final-wasm32-hostwasm` `-s183-1.log`, all exit=0)

### TASK-006: Pattern engine, signals, clock
**Status**: COMPLETED | **Parallelizable**: Yes (with TASK-004/005; needs TASK-001 types, a VM handle trait for closure params)
**Depends on**: TASK-001 (integration tests additionally need TASK-005)
**Deliverables**: Module 5: `Pat`/`PatNode`/`Event`/`TimeSpan`, `query`
with exact `Ratio64` positions, step lists (nested subdivision, nil
rest), combinators from design-music.md section 3 + vocabulary table
(fast, slow, rev, every, whenmod, sometimes/rarely/often, alt, maybe,
euclid, hold, repeat, choose, stack, cat, fastcat, superimpose, off,
jux, iter, chop, ply, chunk, hurry, segment, range, scale, chord,
voicing, arp, grid [recommended rename of Tidal `struct`, design 20 Q3],
degrade-by, sometimes-by), the SAMPLE-REGION operators (striate,
slice/splice, loop-at, fit — WHOLE-SPAN-anchored, layer-1 COVER-EQUIVALENT region controls at query, commit-time speed from whole-span seconds, validation and index faults per design 10.1 revised),
SOUND-FIRST chains and the FIRST-STRUCTURE rule (design-music.md Decided
2026-09-25: `s` takes a sound; a single-sound subject takes structure from
the first list-valued step, later list controls are sampled at onsets;
`s [..]` is structured; MIDI out is an instrument `s {midi 1}`; the
checker rejects `n [..] > s :x`), `OccKey` occurrence identity accumulated during query (structural ordinals + whole.begin + cycle, design 10.2 revised),
controls as `Control` nodes, pure
per-(seed, node, cycle) hash RNG, signals (`Sig`), `Tempo`/`Clock`
with piecewise-linear anchors and derived cps, event provenance spans
sourced from `ListProv` with the stated fallback for derived lists;
`QueryResult` with event-local and subtree-local fault recording
(design 10.3); every numeric combinator parameter — including
`maybe`/`degrade-by`/`sometimes-by` probabilities — as `PParam`;
`PatNode::MidiNotes` (empty under pure query/dry run, realized live by
the scheduler — design 10.1, 11.7) and the input/analysis signals
`Sig::Cc` and `Sig::Analyzer` reading their `f32` cells (design 10.5);
`ClockSource` with the `:midi`-slaved anchor math (smoothing keeps
`Ratio64` positions exact — design 11.1, 11.7; the host plumbing is
TASK-007's). Visual chains (design 9): `TexNode`/`VParam` construction
from the Hydra prelude, `compile_tex` GLSL ES 3.0 codegen from
per-operator snippets, `ShaderDesc`/`UniformSpec`, per-frame uniform
resolution (headless: values only).
**Design ref**: sections 9, 10, 11.1, 11.7.
**Completion criteria**:
- [x] `[:bd :sd [:hh :hh]]` and the mini-notation equivalence table in design-music.md section 3 query to exact ratio positions (golden tests) (evidence: `src/pattern/tests/steps.rs::nested_subdivision_lands_on_exact_ratios`, one test per table row, `the_full_mini_notation_line`)
- [x] `euclid 3 8`, `maybe`, `choose`, `degrade-by` are reproducible under a fixed seed and pure (same span -> same events) (evidence: `src/pattern/tests/random.rs` four tests)
- [x] Sample-region goldens (design 10.1 revised): `chop 8` subdivides each event into 8 exact-ratio sub-events with contiguous regions; `striate 8` leaves timing unchanged and interleaves regions i mod 8 across events; `slice 8 [0 2 4 7]` yields begin/end at i/8 for the indexed slices and `slice [0 0.31 0.5 0.8] [2 0]` the manual spans (last to 1.0); an out-of-range index is an EVENT-LOCAL fault (siblings survive), unsorted or out-of-[0,1] manual points are diagnostics (literal = checker, dynamic = query fault); `splice`/`loop-at 2`/`fit` mark commit-time speed resolution, asserted with a mock bank duration and tempo (speed = slice/sample seconds over WHOLE-SPAN/cycle seconds); COVER EQUIVALENCE (design 10.1 layer 1), against a frozen read snapshot: for `chop 2`, `chop 8`, `striate 8`, `splice`, and `fit`, a full-cycle query versus adjacent subqueries ([0,1/2) + [1/2,1)) versus OVERLAPPING subqueries ([0,3/4) + [1/4,1)) versus a REPEATED window is cover-equivalent — the union of results DEDUPLICATED BY `occ` equals the whole-span result occurrence for occurrence (onsets, regions, rate-fit speeds); the occurrence shared by an overlap is returned by BOTH windows with the SAME OccKey (asserted — pure queries report it twice; layer-2 dedup is TASK-007's, not the query's); a boundary-crossing source event's continuation is emitted with `part.begin > whole.begin` and carries its onset's region controls; a two-branch stack of IDENTICAL simultaneous notes yields two DISTINCT OccKeys (branch ordinals); Astra's chop-2 counterexample asserted exactly (adjacent partition: two onsets at 0 and 1/2, never four, region sequence not restarting); striate ordinals do not restart across query windows (query-widening rank asserted); a `whole = None` source event under any region operator records an event-local fault instead of partition-dependent output (evidence: `src/pattern/tests/region.rs` (chop, striate, slice equal/manual, event-local index fault, dynamic bad points, commit-time speed from a fixed sample length and tempo standing in for the TASK-007 bank, whole=None fault) and `src/pattern/tests/cover.rs` (`cover_equivalence_matrix`, `astra_chop_2_counterexample`, `identical_stack_twins_have_distinct_occurrences`); the literal checker side is `types/tests/diags.rs` (unsorted) plus ME-FINAL probe P3 (unsorted, 1.5 and -0.2 all `bad-slice-points`))
- [x] `PParam::Late` re-reads a var slot per query; `PParam::Fn` calls back into the evaluator in Query effect mode (evidence: `src/vm/tests/integrate_query.rs::late_param_rereads_a_var_per_query`, `fn_param_calls_back_in_query_mode`)
- [x] Mixed valid/failing-event query keeps sibling events, records one origin-carrying fault per failure, and never yields an all-or-nothing error (evidence: `src/pattern/tests/faults.rs::mixed_valid_and_failing_events_keep_siblings` and the subtree-local cases)
- [x] A tweaked `PParam` probability changes behavior for ALL uncommitted events via staging invalidation and re-query (design 11.3) — in BOTH named directions: `maybe` 0 -> 1 RESTORES previously absent events, `degrade-by` 0 -> 1 REMOVES events (evidence: `src/vm/tests/integrate_pattern.rs::maybe_zero_to_one_restores_absent_events`, `degrade_by_zero_to_one_removes_events` through real tweak sites. The re-query semantics are tested (ME-INTEGRATE). The 11.3 staging invalidation of already-staged events lands with TASK-007 and is re-asserted there)
- [x] `PatNode::MidiNotes` yields no events under pure query and dry run; the bind-time INPUT-LANE walk classifies the path: `midi-notes > degrade-by 1` drops EVERY arriving note (filtering applied, never bypassed), control/scale nodes decorate per note in tree order, a `stack` of an input lane with queried branches realizes both independently, and a structural/time operator over the lane (`fast`, `rev`, `every`, `chop`, …) is a BIND-TIME diagnostic with the old binding kept; `Sig::Cc` and `Sig::Analyzer` read their cells at query/frame time; the `:midi`-slaved clock anchor keeps logical positions exact under jittered pulse timestamps (unit test with synthetic pulses) (evidence: `src/pattern/tests/input.rs` (empty under query, degrade-by 1 drops all, per-note decoration order, stack realization, bind-time diagnostics), `src/vm/tests/integrate_pattern.rs::a_retiming_operator_on_live_input_is_rejected_at_bind` (old binding kept), `src/pattern/tests/signals.rs::cc_analyzer_and_host_signals_read_their_cells`, `src/clock/tests/clock.rs::midi_anchor_keeps_logical_positions_exact_under_jitter`; the dry run itself is TASK-007's, and the query path it uses is what is asserted)
- [x] Visual goldens: `osc 20 > rotate 0.5 > out o0` compiles to stable shader source; `text "hello" > out o1` produces its `TextAsset`; uniforms resolve per frame from the evaluator-owned `UniformPlan`; `ShaderDesc` is structurally render-safe (serializes as plain data, no evaluator-owned payload); a failing chain leaves the previous program in place (evidence: `src/tex/tests/goldens.rs` (byte-exact `osc 20 > rotate 0.5` source, `text "hello"` TextAsset), `src/tex/tests/mod.rs` (per-frame uniform resolution, render-safe plain-data `ShaderDesc`: Send + 'static, strings and plain metadata only, no serde because the crate has no dependencies), `src/vm/tests/integrate_tex.rs::a_failing_visual_chain_keeps_the_previous_binding`; ME-FINAL probe P1/P2: the VM-built chains with `> out o0`/`> out o1` evaluate, their source is byte-stable, and `text "hello"` yields asset (0, "hello"))
- [x] Build + nextest pass; wasm32 build passes (session 183: `final-build`, `final-nextest` 489 passed, `final-cargotest`, `final-wasm32`, `final-wasm32-hostwasm` `-s183-1.log`, all exit=0)

### TASK-007: Scheduler, slot table, capability hosts, dry run
**Status**: COMPLETED | **Parallelizable**: No (integration point)
**Depends on**: TASK-005, TASK-006
**Deliverables**: Module 6: `SlotTable` (d1..d9/slot/out over one
table holding `Binding::Pattern|Texture`; hush/stop any kind; mute),
cycle-boundary swap via `pending`, the TWO-HORIZON scheduler (staging
buffer; boundary-split queries with PROSPECTIVE activation of pending
bindings; staging invalidation on rebind and on every control write
so structural parameters stay live; the OCCURRENCE MERGE — ONE
occurrence RECORD per occ per (slot, gen) holding whole, controls,
and a coalesced COVERED EXTENT; coverage-extension merge (fragments
UNION, never delete — a continuation cannot erase an uncommitted
onset); removal AND SEMANTIC PAYLOAD REPLACEMENT only under scoped
semantic invalidation (a surviving same-key record whose
whole.begin lies in the invalidated span takes the NEW snapshot's
whole/value/derived controls/src wholesale — one snapshot per
emission; onsets outside the span are never refreshed by it); and
the per-(slot, gen) committed-occ ledger bounded to the query
horizon (design 11.3 layer 2 revised); commit at `commit_lead` with
commit-time conversion of late-bound controls to `Ctl::Cell` entries
over the `ControlCells` table (values read at voice start — the
decided next-event rule for audio value controls, design 11.3/11.4);
slot generations with `SlotControl` (release semantics + always-on
late-start recovery), the TWO-CLASS per-slot control channel
(immediate + future, ordered, never collapsed), `SlotControlAck`
re-send with transport-diagnostic threshold, gen piggyback on event
batches; captured query-output held per event and forwarded at its
COMMIT point; tempo-change re-commit — design 11.3), POD conversion
(`AudioEvent` with slot + gen, control mapping incl. gain->amp and
note-through-scale, orbit/bus routing; `MidiEvent`/`OscEvent`),
`once`/`at` (ephemeral slot, scheduled thunks in Normal effect mode),
capability traits + `NoopHost`, read-only `dry_run` in Query effect
mode (failure keeps the old binding), telemetry (`PlayingEvent` queue
with `SrcRef`, levels), `use-bpm`/`use-cycle`/`use-clock`/
`midi-clock-out` prelude, `MidiInHost` draining (cc cells,
`midi-notes` live realization with NoteOff matching, MIDI clock
slave/master and Start/Stop/Continue transport — design 11.7,
`:link` selection = diagnostic), event-local fault forwarding and
per-slot diagnostic clearing.
**Design ref**: sections 10.3, 10.4, 11.2–11.7, 18.
**Completion criteria**:
- [x] Deterministic scheduler tests (mock clock, recording hosts): exact ratio positions; queries split at every cycle boundary; a SUFFICIENT-lead re-bind swaps only at the boundary with no stale-pattern event past it (the unconditional form of this claim is withdrawn — the three-case bullet governs late arrivals); REGION PARTITION INVARIANCE THROUGH STAGING (design 10.1/11.3): a `chop`ped and a `striate`d pattern staged incrementally across several ticks and cycle-boundary splits — and re-staged after a control write invalidates uncommitted spans — commit the same logical notes (onsets, regions, speeds) as a single whole-cycle staging, asserted against the full-cycle dry run; OCCURRENCE-MERGE EMISSION UNIQUENESS AND ONSET PRESERVATION (design 11.3 layer 2 revised), asserted as EMITTED MULTIPLICITY at the recording host — never set equality: staging through OVERLAPPING windows [0,3/4) then [1/4,1) commits onset 1/2 EXACTLY ONCE, including the committed-ledger variant where the first window's work partially commits before the second query arrives; ONSET PRESERVATION UNDER CLIPPING (Astra's future-span case): source whole [1,2) under `chop 2`, queries [1,7/4) and [5/4,2) BOTH staged before ANY commit — onsets 1 and 3/2 each emit EXACTLY ONCE, asserted in BOTH query orders (the second window's continuation fragment of child 0 merges into the record's covered extent and does not erase its onset); BOUNDARY-CROSSING FRAGMENTS across disjoint adjacent spans, onset fragment first AND continuation fragment first, merge into one record with one emission; PARTIAL INVALIDATION: invalidating only [5/4,2) discards coverage inside that span, preserves child 0's onset at 1 (whole.begin outside the span — never dropped, only re-extended), and drops an uncommitted record only when its whole.begin lies in the invalidated span and its key is absent from the re-query; a REPEATED window commits nothing twice; a two-branch stack of IDENTICAL simultaneous notes commits BOTH voices (distinct OccKeys — the merge never erases real notes); SEMANTIC PAYLOAD REPLACEMENT (design 11.3 revised — Astra's slice-index case): an uncommitted occurrence with whole [1,2) under `slice` with two equal regions and late index 0 (region [0,1/2)) — `upd` the index to 1 and invalidate/re-query a span containing the onset: assert the SAME OccKey and generation, EXACTLY ONE emission at the recording host, its region is [1/2,1), and NO old-region [0,1/2) output ever appears; and INVALIDATING ONLY A CONTINUATION span does not rewrite the unaffected onset's payload (the onset's region is unchanged while the in-span coverage rebuilds) — **Evidence**: `src/sched/tests/sched/merge.rs` (12 tests: `onsets_sit_at_exact_ratio_positions_and_queries_split_at_boundaries`, `chop_and_striate_staged_incrementally_equal_the_full_cycle_query`, `overlapping_windows_staged_before_commit_emit_each_onset_once`, `a_partially_committed_window_never_re_emits_through_the_ledger`, `onsets_survive_clipping_in_both_query_orders`, `boundary_crossing_fragments_merge_into_one_emission_either_way`, `partial_invalidation_keeps_onsets_outside_the_span`, `a_repeated_window_commits_nothing_twice`, `identical_stack_twins_both_commit`, `semantic_invalidation_replaces_the_payload_of_the_same_occurrence`, `invalidating_only_a_continuation_keeps_the_onset_payload`), `src/sched/tests/sched/rebind.rs` `sufficient_lead_swaps_exactly_at_the_boundary_with_full_lead` (BE-SCHED); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Deadline-aware rebind, THREE cases with explicit cycle duration and transport delay: SUFFICIENT lead (`B - now >= commit_lead + L_ctl`) yields zero old-pattern events past the boundary and full-lead new boundary events; INSUFFICIENT lead commits new boundary events with reduced lead (flagged) rather than dropping them; control-delivery failure is DENSITY-BOUNDED: all old-gen events starting within the delivery delay may begin and each is cut on control receipt (three simultaneous boundary events = three artifacts, each within `D` + gate), with the missed deadline reported — **Evidence**: `src/sched/tests/sched/rebind.rs` `sufficient_lead_swaps_exactly_at_the_boundary_with_full_lead`, `insufficient_lead_commits_new_boundary_events_with_reduced_lead`, `control_delivery_failure_is_density_bounded_and_reported` (BE-SCHED); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] stop/hush/tempo/set-tweak (control value and structural probability: `maybe` 0→1 restores, `degrade-by` 0→1 removes) behave as bounded across boundaries, `once`/`at`, and audio/MIDI/OSC sinks with slot+gen identity; the immediate-class MONOTONE MERGE preserves the strongest release: HUSH-THEN-STOP keeps Panic (never downgraded to Natural), HUSH-THEN-TEMPO keeps Panic while the tempo gen bump rides the merged entry, HUSH-THEN-REBIND keeps both entries in order; re-send after a duplicate delivery is idempotent under the merge; a MIDI stale-started note gets an immediate note-off; OSC revocation applies only to the untransmitted queue — **Evidence**: `src/sched/tests/sched/control.rs` (`hush_then_stop_keeps_panic`, `hush_then_tempo_keeps_panic_with_the_tempo_generation`, `hush_then_rebind_keeps_both_entries_in_order`, `duplicate_delivery_is_idempotent_under_the_merge`, `a_stale_started_midi_note_gets_an_immediate_note_off`, `osc_revocation_reaches_only_the_untransmitted_queue`, `a_structural_tweak_restores_and_removes_uncommitted_events`, `once_and_at_play_on_ephemeral_slots`, `a_tempo_change_re_anchors_and_re_commits_under_a_new_generation`), `src/host/tests/contracts.rs` `merge_keeps_panic_through_stop_and_tempo`; once/at revocation added by BE-FINAL in `src/host/tests/e2e/sched_gaps.rs` (`revoking_an_uncommitted_once_slot_never_starts_it_and_removes_the_slot`, `hush_gates_a_sounding_once_voice_and_removes_its_ephemeral_slot`); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] A LOST FUTURE-boundary control with a gen-carrying event batch arriving first triggers only the DEFAULT piggyback policy (drop stale-gen at dequeue + short-gate stale-started voices — over-eager before the boundary is accepted and asserted), full boundary/release semantics land when re-send delivers; repeated loss exercises ack/re-send and the transport-diagnostic threshold — **Evidence**: `src/sched/tests/sched/control.rs` `a_lost_future_control_leaves_only_the_default_piggyback_policy`, `repeated_loss_resends_and_raises_the_transport_diagnostic_once`; full semantics after the re-send: `src/host/tests/e2e/sched_gaps.rs` `the_resent_future_control_carries_the_same_full_boundary_semantics_as_its_first_send` (BE-FINAL); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Cell semantics per tier: NATIVE `upd` on a `Ctl::Cell` control is heard at the next STARTED event even when already committed (atomic store/load); BROWSER-model test with a DELAYED CellUpdate batch: an event starting inside the delivery hop plays the previous value and the next voice start after batch application plays the new one — the delivery-dependent guarantee, asserted exactly, with the mock transport modeling the isolated mirror (no shared-memory shortcut); FIRST-USE-BEFORE-INIT: an event referencing a new cell incarnation before `CellInitAck` commits with the `Const` downgrade and sounds the correct value, and the cell path resumes after the ack; INIT REPLAY AFTER UPDATE: a retransmitted `CellInit(E, v0)` arriving after a batch set v1 is acknowledge-only and v1 stays (init-once per epoch); REUSE INITIALIZATION: a retired-then-reused id passes Vacant -> Live with its new epoch while old-epoch updates stay inert; PRE-ACK CONST + POST-ACK BATCH: a `Const`-downgraded event whose start follows the ack and a newer batch still plays its committed constant, asserted as the documented Const-fallback exception with the next commit back on the cell path; STALLED-CONSUMER BURST: writes while the mirror applies nothing never exceed one in-flight + one pending batch, coalesce latest-wins, and on resume the mirror equals the latest values; STALE-UPDATE-AFTER-REUSE: an update carrying a retired epoch is dropped by the mirror; RECONNECT replays the full snapshot before any new voice start reads the mirror; MIDI/OSC value baking at transmission asserted as the documented sink-specific deviation — **Evidence**: `src/sched/tests/sched/cells.rs` (9 tests, native and `BrowserTransport` tiers, every sub-clause), `src/host/tests/contracts.rs` `browser_transport_*`, `src/dsp/tests/contracts.rs` `mirror_*` (BE-SCHED, BE-CONTRACTS); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] MIDI clock slave: synthetic jittered 24-ppq pulses drive a smoothed anchor with exact logical positions; Start/Stop/Continue freeze and resume the scheduler; clock loss freewheels with a diagnostic; clock master emits Clock/Start/Stop with commit discipline; `midi-notes` input sounds a voice within one drain tick + commit path and never appears in dry runs — **Evidence**: `src/sched/tests/midi/clock.rs`, `src/sched/tests/midi/transport.rs`, `src/sched/tests/midi/input.rs` (`a_note_sounds_a_voice_within_one_drain_tick`, `midi_notes_never_produces_events_in_dry_runs`) (BE-MIDI); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Note lifetime (design 11.7 revised): a held note releases on its NoteOff via `VoiceRelease` tag matching; repeated same-pitch NoteOns release EARLIEST-open-first; a held OLD-GENERATION voice surviving a rebind (`None`) is ACTUALLY TERMINATED by its later NoteOff via its tag (no stuck note), while the new binding's voices stay untouched (tags differ — both halves asserted); `stop`'s Natural puts every open input voice into its release stage and closes its session records (no indefinite sustain); a FILTERED NoteOn consumes its own NoteOff without releasing a different same-pitch held voice; NoteOn/NoteOff share the FIFO priority channel and a forced release-before-start in the mock transport hits the tombstone (the late NoteOn is dropped, no unreleasable voice); tag-map exhaustion steals the oldest open voice with a diagnostic, no allocation; hush Panic-gates open input voices — **Evidence**: `src/sched/tests/midi/lifetime.rs` (8 tests fed into a real `dsp::Engine`), `src/host/native/tests/midi.rs` (BE-MIDI, BE-NATIVE); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Live captured `print` reaches the console only AT its event's commit point: a control write restaging the still-uncommitted span replaces the held output (no duplicate, none for removed work); a write after commit leaves emitted output untouched — **Evidence**: `src/sched/tests/sched/output.rs` `a_print_reaches_the_console_once_at_commit`, `a_restage_before_commit_replaces_held_output_and_a_later_write_does_not`; removed work, both orderings: `src/host/tests/e2e/sched_gaps.rs` `a_restage_that_removes_staged_events_drops_their_held_print` (filter before the printing control) and `a_filter_wrapping_the_printing_control_drops_removed_work_output` (filter wrapping it, then `upd q 1`; failed before serial repair R7 with 320 duplicated lines, passes after) (BE-FINAL); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed); BE-FINAL serial repair R7: `degrade-by`/`maybe` drop the captured output of the events they remove (`src/pattern/combinators/random.rs` `query_degrade`, `QueryVm::put_output`)
- [x] Dry-run success AND failure leave namespace, slots, staging, pending queues, and host outputs unchanged (state-snapshot assertion); failure keeps the old binding playing; captured dry-run `print` output appears in the report, never on the console — **Evidence**: `src/sched/tests/sched/dryrun.rs` `successful_and_failed_dry_runs_leave_everything_unchanged`, `a_failed_bind_keeps_the_old_binding_and_its_print_stays_in_the_report`; control channel and cells snapshot: `src/host/tests/e2e/sched_gaps.rs` `dry_run_leaves_the_control_channel_and_control_cells_unchanged` (BE-FINAL); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] `hush`/`stop` across audio, visual, midi, osc slots — per release class, never a blanket silence assertion: NEW events cease within control-message latency (not lookahead latency); hush's Panic gates sounding voices within it; stop's Natural lets sounding voices end naturally (open input voices enter release, 11.7); already-transmitted OSC is irrevocable and asserted so — **Evidence**: `src/sched/tests/sched/control.rs` `stop_reaches_every_sink_with_one_identity_and_new_events_cease`, `hush_panic_gates_sounding_voices_and_clears_texture_outputs`, `osc_revocation_reaches_only_the_untransmitted_queue`; `src/sched/tests/midi/lifetime.rs` `stop_puts_open_input_voices_into_release_and_closes_their_records`, `hush_panic_gates_open_input_voices`; `src/host/tests/e2e/sched_gaps.rs` `stop_targets_only_a_texture_slot_and_clears_its_output`, `hush_bounds_new_audio_events_and_revokes_only_the_untransmitted_osc_queue` (BE-FINAL); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Mixed valid/failing events in one span: valid events play, each fault carries slot + beat, diagnostics clear after a clean cycle — **Evidence**: `src/sched/tests/sched/faults.rs` `valid_events_play_faults_carry_slot_and_beat_and_clear_after_a_clean_cycle`, `a_commit_time_fault_drops_only_its_event` (BE-SCHED); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Build + nextest pass (planned) — **Evidence**: final tree (BE-FINAL session 186): `target/fe-logs/be-final-build-s186-2.log` exit=0, `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed), `target/fe-logs/be-final-cargotest-s186-2.log` exit=0 (lib 769 passed, spec_fixtures 10 passed)

### TASK-008: DSP graph, native audio host, wasm/AudioWorklet host
**Status**: COMPLETED | **Parallelizable**: Yes (pure-DSP voice/graph code needs only TASK-001 and can proceed alongside TASK-007; host integration needs TASK-007's traits and event/gen contract)
**Depends on**: TASK-001, TASK-007
**Deliverables**: Module 7: `inst` evaluation to `InstDef` templates
over the EXTENDED ugen catalog (design 12.4: vco/sub-osc/ladder/svf,
fm-op/fm-mod, phase-distortion, additive, wavetable, sample-play) with
the prelude synthesis TEMPLATES (`sampler`, `analog`, `fm`, `pd`,
`additive`, `wavetable`, `granular`) shipped as prelude `.vact` source
through the same path; the Decided EFFECT catalog as `EffectSpec` in
all three positions (pattern controls, inst ugens, `bus :name:` /
`master` chains — design 12.5) with `BusGraph` routing, bus swap under
the standard generation+refcount lifecycle, and ANALYZER units writing
`f32` analysis cells; the GRANULAR engine (one implementation, two
faces; static sample/table sources and the LIVE-bus circular capture
buffer with its sliding valid-read window and guard, freeze/unfreeze
write-head semantics, phase-accumulator onset scheduling, pools
sized ceil(max_grain_density x max_grain_size), skip-and-count
admission — design 12.6 revised);
`CapabilitySet` tier advertisement (incl. `max_grain_size` and
`max_capture_seconds`) with beyond-capability diagnostics
(design 12.7); the `EditorDecl`/`ParamMeta` table for every builtin
(editor kinds, ranges, curves, units, groups — the metadata
directives never restate, design 13.5), carried through
`HostManifest`; voice pool, orbit effects, event ring, CONTROL-CELL
TRANSPORT per tier (native: atomic f32 in shared process memory;
browser: the design 11.3 initialized/ordered/bounded mirror protocol
— `CellInit`/`CellInitAck` before first use with the commit-time
`Const` downgrade, epoch-checked `CellUpdate` entries in
sequence-numbered batches, one in-flight + one pending with
latest-wins coalescing while blocked, one batch applied per
`process()` under the 16.1 credit, full-snapshot resync on
reconnect, pooled `CellId`s retired through the 16.1
lifecycle and reuse gated on retire-ack + epoch — design
11.3/11.4), the `VoiceRelease` tag->voice map (preallocated,
bounded), sample banks + wavetable storage via `SampleLoader`
(+ manifest for the checker), analysis taps (amp, fft) publishing
`f32` cells; `NativeAudioHost`
(cpal callback, timer tick, midir; AMENDED 2026-09-25 per B5: the
session socket moved to TASK-009); `WasmHost`
(raw `extern "C"` exports instead of wasm-bindgen, AMENDED per B5
so the browser build needs no CLI step; worklet JS glue under `editor/worklet/`,
postMessage handoff with transferables, worklet-as-timebase clock,
latency-window auto-widen + report, and the design 16.1 resource
lifecycle: preallocated `SampleArena` and graph slots, off-thread
decode, the sender-paced one-slice (64 KB) install window with
`SliceOk`/`Installed` acks, admission checks, uniform
generation-aware refcounted retirement for samples AND graphs with
`Retired` messages, `inst` node-count caps, capacity-exhaustion
diagnostics); a MINIMAL standalone browser dev harness
(`editor/dev-harness/`: plain HTML + JS, wasm module + worklet, no
editor code) that drives the 16.1 lifecycle checks; a `host-native`
example binary (`examples/beep.rs`) that plays one event end to end
without the REPL.
No allocation/locking on the audio callback path (asserted by design
review + debug-mode counters) and bounded worklet handler work by
construction.
**Design ref**: sections 12, 16, 17.
**Completion criteria**:
- [x] Headless render test: schedule events into the ring, run the callback over N buffers, assert sample-accurate voice starts and no allocation counter hits (debug instrumentation) — **Evidence**: `src/dsp/tests/dsp/render.rs` `voice_starts_land_on_the_exact_frame`, `many_events_over_many_buffers`, `late_event_starts_at_frame_zero_and_is_counted`; every `Rig::step` and every `src/host/tests/e2e/*` render asserts 0 callback allocations via `dsp::alloc_probe::armed`; `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Each synthesis TEMPLATE (`sampler`, `analog`, `fm`, `pd`, `additive`, `wavetable`, `granular`) compiles from prelude source to an `InstDef` and renders headlessly; template parameters arrive as ordinary pattern controls — **Evidence**: compile: `src/types/tests/inst/templates.rs` (`seven_templates_realize_at_session_start`, `each_template_is_built_from_its_model_ugens`, `template_parameters_are_controls`); render end to end (Evaluator + InstRegistry -> Runtime -> NativeAudioHost ring -> Engine): `src/host/tests/e2e/templates.rs` `every_template_installs_and_renders_non_silent` (all seven), `a_template_parameter_is_an_ordinary_pattern_control` (`cutoff 100` vs `12000`), `every_builtin_has_editor_metadata_through_the_host_manifest`; BE-FINAL serial repairs R1 (inst header parameter as a pattern control at run time) and R2 (ugen port wiring by catalog name, template resource defaults); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed), `target/fe-logs/be-final-e2e-s186-2.log` exit=0 (28/28)
- [x] Sample-region handoff (values only, design 10.1 revised): a sliced event renders exactly its begin/end sample region headlessly; `splice` renders the slice rate-fitted to its step and `loop-at 2` stretches the sample across two cycles with loop enabled; `fit` matches the event length; regions at the sample bounds render without overrun (bounds asserted); all via existing begin/end/speed/loop `Ctl` values — no new audio-thread machinery (asserted structurally); regions, onsets, and rate-fit durations originating from PARTITIONED staging queries (multi-tick lookahead, boundary splits) render identically to the full-cycle render — partition invariance holds through to the host boundary; OVERLAPPED staging ([0,3/4) + [1/4,1)) and a repeated window render each voice start EXACTLY ONCE while the identical two-branch stack renders BOTH voices (emitted multiplicity asserted at the audio ring); the future-span onset-preservation case (whole [1,2) under `chop 2`, both windows staged pre-commit, both query orders) renders BOTH voices at 1 and 3/2 exactly once each — no onset lost to a continuation fragment; the slice-index payload-refresh case (index 0 -> 1 with invalidation covering the onset) renders exactly one voice playing the NEW region [1/2,1) at the audio renderer with no old-region output — **Evidence**: DSP: `src/dsp/tests/dsp/region.rs` (exact begin/end region, bounds, `sample_play_reads_only_its_region_controls` = no new audio-thread machinery); end to end at the audio renderer: `src/host/tests/e2e/regions.rs` (`begin_end_renders_exactly_its_region`, `chop_renders_each_half_s_own_region`, `splice_rate_fits_its_step_unlike_plain_slice`, `loop_at_plays_with_loop_enabled_past_one_pass`, `fit_matches_the_event_length`, `region_bounds_never_exceed_the_ramp_and_never_produce_nan`, `partition_invariance_incremental_staging_matches_one_shot`, `overlapped_staging_gives_each_voice_start_exactly_once` (exactly 4 AudioEvents committed, peak equal to one clean pass), `stack_twins_render_both_voices` (exactly 2, peak ratio 2.0 < 2.5), `future_span_chop_renders_each_onset_once_in_both_query_orders` (exactly 2 per order), `slice_index_refresh_renders_only_the_new_region`); BE-FINAL serial repairs R4 (commit-time `speed-fit`) and R5 (`loop-at` writes a bool `loop`); `target/fe-logs/be-final-e2e-s186-2.log` exit=0
- [x] Bus chains: `bus :drums:` compiles, a slot routes into it via the `bus` control, `master` receives every bus, a bus-chain swap follows the generation+refcount lifecycle without callback allocation; the SuperDirt-style `room` control maps onto the corresponding bus unit parameter — **Evidence**: `src/dsp/tests/dsp/bus.rs`, `src/types/tests/inst/buses.rs`, end to end `src/host/tests/e2e/buses.rs` (`a_bus_definition_compiles_and_installs`, `a_routed_voice_is_measurably_quieter_than_unrouted` (ratio 0.1259 = -18 dB), `master_lowers_both_a_routed_and_an_unrouted_slot`, `a_bus_redefinition_swaps_to_the_new_gain_with_zero_allocation_and_no_dropout`, `room_maps_onto_the_bus_unit_parameter`); BE-FINAL serial repairs R3 (commit writes the `bus` routing control) and R6 (effect parameters keyed in the effect-local id space); `target/fe-logs/be-final-e2e-s186-2.log` exit=0
- [x] Analyzers write their cells without altering the rendered signal (bit-compare with and without an analyzer in the chain); granular renders from a preallocated grain pool with zero callback allocation, and a density above the tier cap is a diagnostic, not a dropout — **Evidence**: render half `src/dsp/tests/dsp/analyzer.rs` `every_analyzer_is_bit_transparent`, `src/dsp/tests/dsp/granular.rs` `a_full_pool_skips_with_a_count`, `caps_clamp_and_count_with_audio_continuing`; diagnostic half `src/sched/tests/sched/granular.rs` `over_cap_granular_events_are_diagnosed_with_origin_and_still_commit` (BE-DSP, BE-SCHED); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Granular behavior (design 12.6 revised), deterministic under a fixed seed: onset COUNT and TIMING over N blocks match the phase-accumulator model for fixed density/size; a LIVE-bus instance never reads a sample overwritten during a grain's life across capture-buffer WRAP (guard asserted); FREEZE retains the frozen audio while live input keeps arriving (frozen-grain output invariant) and unfreeze resumes reading live content; ACTIVE-GRAIN UNFREEZE: a long frozen grain reading the oldest region is re-checked at the unfreeze boundary against the live rule for its remaining span and SHORT-GATED before capture overwrites it (no sample it plays is rewritten during its life — the invariant holds across the transition), while grains whose spans stay valid continue; PARTIAL-FILL FREEZE: the valid window is always intersected with the captured extent — freezing a partially filled buffer exposes only recorded audio and freezing immediately after install spawns no grains (skip-counted, never uncaptured storage); `size` above `max_grain_size` and position depth beyond `max_capture_seconds` are diagnostics; pool exhaustion skips grains with a count and no allocation — **Evidence**: `src/dsp/tests/dsp/granular.rs` (`onsets_follow_the_phase_accumulator_model`, `a_fixed_seed_renders_identically`, `live_grains_never_read_overwritten_samples_across_wraps`, `freeze_holds_the_buffer_while_input_continues_and_unfreeze_resumes`, `unfreeze_short_gates_only_violating_grains`, `partial_fill_freeze_exposes_only_captured_audio`, `freeze_right_after_install_spawns_nothing`, `capture_depth_beyond_the_cap_is_clamped_and_counted`, `a_full_pool_skips_with_a_count`) and the size/depth diagnostics in `src/sched/tests/sched/granular.rs` (BE-DSP, BE-SCHED); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] `CapabilitySet` gating: an IR beyond `max_ir_seconds` and offline render on the browser tier produce "not available on this host" diagnostics with origin — **Evidence**: `src/dsp/tests/dsp/caps.rs` `an_ir_over_the_tier_limit_is_beyond_capability_at_its_origin`, `src/dsp/tests/contracts.rs` `browser_refuses_offline_render_and_long_irs_with_origin`, `src/types/tests/inst/capability.rs` `offline_render_is_not_available_in_the_browser` (BE-DSP, BE-CONTRACTS, BE-INST); `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed)
- [x] Cell transport in the dev harness (real worklet): a batch applied between quanta changes the value read by the next voice start; a delayed batch leaves the earlier value audible for events starting inside the hop; a FIRST event before `CellInitAck` sounds its `Const`-downgraded value (never an uninitialized mirror read); a replayed `CellInit` after a newer batch leaves the newer value in place (init-once), and a reused id initializes through Vacant -> Live in the real worklet; a stalled-worklet burst is bounded to one in-flight + one pending batch and converges to the latest values on resume; an update with a retired epoch after id reuse is inert; reconnect snapshot completes before new voice starts read the mirror; `VoiceRelease` releases exactly its tagged voice even after a slot-gen bump, and a forced release-before-start hits the tombstone — **Evidence**: real worklet, check ids `cell-first-before-ack`, `cell-batch-next-voice`, `cell-delayed-hop`, `cell-init-replay`, `cell-reuse`, `cell-stale-epoch`, `cell-stall-burst`, `cell-reconnect`, `release-after-genbump`, `release-tombstone` all PASS: `target/fe-logs/be-final-harness-s186-2.log` exit=0, report `target/fe-logs/be-final-harness-s186-2.json` (F8 final-tree run, HeadlessChrome 154, 18/18 PASS, memoryStable true)
- [x] `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` passes (planned) — **Evidence**: `target/fe-logs/be-final-wasm32-hostwasm-s186-2.log` exit=0 (and default-feature `target/fe-logs/be-final-wasm32-s186-2.log` exit=0; `cargo tree -e normal --target wasm32-unknown-unknown` has no cpal/midir, `target/fe-logs/be-final-tree-s186-2.log`)
- [ ] `examples/beep.rs` plays a sample end to end on the native host (manual check, recorded in progress log); the audible REPL gate lives in TASK-009 — **Evidence**: PENDING USER CONFIRMATION (manual audible check on a real output device, `cargo run --example beep`). Automated proxy: `src/host/tests/e2e/beep.rs` `the_beep_program_renders_headlessly` (the example's program through Runtime + NativeAudioHost headless + Engine, non-silent, finite, 0 callback allocations); build: `target/fe-logs/be-final-example-s186-2.log` exit=0. Per issue #3 this does not block acceptance
- [x] Browser lifecycle checks run in this task's OWN dev harness (no editor or downstream-task dependency): sample-bank load and graph replacement during active playback without dropout; arena-exhaustion, oversized-graph, and deferred-queue-overflow diagnostics; unload-while-playing holds storage until queued events and voices release it; an install burst copies at MOST `INSTALL_BYTES_PER_QUANTUM` on the rendering thread PER `process()` call — measured as total bytes copied per quantum across the whole burst, not merely unacked-slice count — with acks withheld until the deferred copy runs; the headless `process()` test alone is not accepted as proof — **Evidence**: real worklet, check ids `load-during-playback`, `graph-replace-during-playback`, `arena-exhausted`, `graph-too-large`, `deferred-queue-overflow`, `unload-while-playing`, `install-burst-credit` (max 65536 bytes copied per process(), SliceOk withheld), `memory-stable` all PASS: `target/fe-logs/be-final-harness-s186-2.log` exit=0, report `target/fe-logs/be-final-harness-s186-2.json` (F8 final-tree run, HeadlessChrome 154, 18/18 PASS, memoryStable true)
- [x] nextest pass (planned) — **Evidence**: `target/fe-logs/be-final-nextest-s186-2.log` exit=0 (779 run / 779 passed), `target/fe-logs/be-final-e2e-s186-2.log` exit=0 (28/28), `target/fe-logs/be-final-fixtures-s186-2.log` exit=0 (10/10)

### TASK-009: Session, REPL, LSP
**Status**: COMPLETED (criterion 8, the audible REPL gate, is pending user confirmation; see below) | **Parallelizable**: No
**Depends on**: TASK-004, TASK-005, TASK-007; TASK-008 for the audible gate (the REPL itself runs against NoopHost without it)
**Deliverables**: Module 8 Rust half: `Session` (eval pipeline, tick,
diagnostics bus, tweak table, form generations, REACTIVE
re-evaluation via the 5.6 graph — `upd` coalescing per tick and
the post-pass `bindings` batch of 14.4 (changed names + display
values + refreshed tweak sites + per-form failures; one batch per
completed pass, never mid-pass), apply/protocol); PACKAGE loading
end to end (design
5.7): `PkgManifest`/`vactrol.lock` parse and write, `mvs_resolve`
(minimal version selection over git-tag semver), `PackageStore`
implementations (native: git tag fetch + `~/.vactrol/pkg` cache;
browser: Go-proxy-shaped HTTPS + OPFS cache) behind the host
capability, the CANONICAL content digest (INJECTIVE length-prefixed
records over the bytewise-sorted file tree, design 5.7 revised —
identical across git/local-dir/proxy stores), entry validation and
containment (UTF-8 paths without control characters, no
traversal/absolute paths, no link/device entries, no duplicates,
bounded extraction; assets contained under the package root),
staged extraction with ATOMIC verified cache publication,
integrity check against the lock, import-time
compile into `PkgNs` with package-attributed diagnostics, asset
registration with `SampleLoader`, `vactrol get` CLI verb, and the
failure contract (unresolvable/network/hash-mismatch/package
diagnostics = load diagnostic, session continues, evaluator never
blocks on the network) with the TWO-PHASE document frontend
(`prescan_imports` feeding fetches and the per-form `AliasEnv`);
DIRECTIVE machinery (design 13.5): session-side `#@` parsing from
trivia into the `DirectiveTable` (Positional/Addressed, the Decided
attachment rule, consecutive-line blocks), the `LabelRegistry`
(explicit + implicit labels; label-identified binding provenance
preferred over positional TweakIds), editor-validation diagnostics
(unknown call sites/params/labels — lint, never language errors),
learned-CC write-back into the directive comment through the
edit_epoch-validated edit path, and the `BindingPersistence`
interface with BOTH impls (Directive as the ADJUDICATED default;
ExternalFile as an optional mode — design 13.5, conflict resolved);
the concrete directive vocabulary implements the 13.5 PROPOSED
grammar (file defaults `#@ midi ch:`, `#@ name X` and trailing
`label:` short-form label definitions, positional `cc:` lists in
declared parameter order with `_` skips, `ch:` overrides, addressed
`label.param`, reserved-key rejection) and is tracked in the
fixture manifest's authority-question channel until the author
ratifies it; ClientMsg/ServerMsg
JSON codec
(versioned envelope; `doc_revision` on eval, `doc-changed` edit
invalidation with base revision + change set, `edit_epoch` on
set-tweak/set-var with runtime debounce-race rejection and
change-set coordinate mapping, `form_gen` on set-tweak,
`defining_form_gen` authority on set-var, `stale-binding` rejection,
tweak-site tiers incl. `manual`), WebSocket server = the native
session socket moved here from TASK-008 (B5) (host-native,
localhost + token), `vactrol repl` (console registers `_1..`,
transcript semantics), `vactrol lsp` behind `lsp` feature (tower-lsp:
diagnostics, hover from `TypedInfo`, completion from prelude +
manifest, formatting via reader trivia; attaches to a live session
socket for runtime diagnostics when present), CLI wiring in
`src/main.rs` (`repl`, `run <file.vact>`, `lsp`, `serve`,
`get <package-path>`) — to be reflected in design-docs/specs/command.md
when implementation starts.
**Amended (issue #4, accepted divergences of design 14.5.1)**: offline
`render` goes through `NativeAudioHost::headless` + `AudioSide::render`,
not a new `Engine::render` API (14.5.9; `dsp/engine.rs` untouched); the
browser package store ships only its Rust half (`ProxyTransport` +
`CacheBackend`, exercised against a local HTTP fixture), and the
`fetch()` transport and the OPFS backend move to TASK-010; the native
stores are a `git`-shelling tag store and a local-directory store, with
no native HTTP client; a running session never fetches (`vactrol get`
is the only native fetch path); the session socket lives in
`src/cli/ws.rs` (loopback, 32-byte token, 401, 8 connections, 1 MiB
frames, 14.5.10); live self-analysis taps are native-tier for `:master`
and named buses only (browser taps are TASK-010, slot and `:in` sources
report not-available, S2). The CLI and Session Protocol v1 are in
design-docs/specs/command.md. Work split: plans SS-CONTRACTS, SS-PKG,
SS-DIRECTIVES, SS-ANALYSIS, SS-SESSION, SS-CLI, SS-LSP, SS-FINAL
(`impl-plans/active/vactrol-session-*.md`, manifest
`ss-session-20260925-s183-dispatch.json`).
**Design ref**: sections 5.7, 14, 17.
**Completion criteria**:
- [x] Protocol round-trip tests: eval -> diagnostics + tweak sites; set-tweak/set-var state change asserted via recording host; playing telemetry carries `SrcRef`s with `doc_revision`; REACTIVE publication: an `upd` through a derived binding emits exactly ONE `bindings` batch after ALL rounds complete (changed names, display values, refreshed sites, PER-FORM STATES), no message mid-pass or mid-round; the CHANGING-EDGE pass publishes only final-round values (the round-1 stale value never appears in any batch); the FAILED-DIAMOND pass publishes right's new value, left's failure diagnostic, and total's `blocked-on: left` with its previous value, and the recovery pass publishes the unblocked recomputation; a subscriber never observes a partially updated set; PROVISIONAL-ROLLBACK publication (TASK-005's REACHABLE X/Z/Y trace, end to end): the pass publishes X as Failed with its RESTORED pre-pass display value 7 and Z/Y with their new values — the provisional 1/2 appears in NO batch, and no staged bind, revocation, or cell update of X reaches the recording host or any subscriber; ABORT/RETRY publication (TASK-005's A/B trace): A publishes Failed with its previous value standing and B publishes 0, with no intermediate A value in any batch; CONDITIONAL-UNBLOCKING publication (switch-away): the pass publishes selected's new value and CLEARS its `blocked-on` badge while broken remains failed with its diagnostic; SWITCH-TOWARD and STATUS-RECOVERY publication: blocking toward broken publishes selected's `blocked-on` badge, and the equal-value repair (`upd root` 1 -> 0 -> 1) publishes a recovery batch clearing the badges of total and the blocked-toward selected even though the repaired value is unchanged; LATE-FAILURE publication (TASK-005's trace-3 case): the pass publishes X as `blocked-on: Y` with its RESTORED pre-pass 7, Y as Failed with its retained value, and Z's new value — the late-failure-derived provisional 2 appears in NO batch and no staged effect of X reaches the recording host or any subscriber; NEWLY-DISCOVERED-SELECTOR publication: `upd b false` publishes selected's new value 7 and CLEARS its `blocked-on` badge while broken remains failed with its diagnostic (unblocking without repairing broken, asserted end to end); ORDINARY-FAILURE RECOVERY publication (TASK-005's a/b division case): the failing pass publishes selected as Failed with its retained 5 and the diagnostic, and the `upd b 1` pass publishes the repaired value 1 with the diagnostic CLEARED — with no manual re-evaluation between the two batches — **Evidence**: (asserted exactly by SS-SESSION `src/session/tests/eval.rs`: `eval_result_carries_diagnostics_sites_and_the_directive_table`, `set_var_and_set_tweak_change_state_heard_at_the_recording_host`, `playing_telemetry_carries_srcrefs_with_the_evals_revision`, `eval_result_precedes_the_bindings_batch_it_triggered`; `src/session/tests/publish.rs`: `changing_edge_publishes_one_batch_after_all_rounds_with_final_values`, `failed_diamond_publishes_failure_block_and_recovery`, `provisional_rollback_publishes_restored_x_and_nothing_reaches_the_host`, `abort_retry_publishes_a_failed_with_its_previous_value_and_b_zero`, `conditional_unblocking_clears_the_badge_and_broken_stays_failed`, `switch_toward_publishes_the_badge_and_repair_clears_it`, `status_recovery_publishes_ok_badges_although_the_value_is_unchanged`, `late_failure_publishes_x_blocked_on_y_and_never_the_provisional_two`, `newly_discovered_selector_publishes_selected_and_clears_its_badge`, `ordinary_failure_publishes_the_failure_then_the_repair_without_re_eval`; logs `target/fe-logs/ss-final-session-s186-1.log` 132/132 passed and `target/fe-logs/ss-final-nextest-s186-1.log` 984/984 passed, both exit=0)
- [x] Packages: a fixture package (local-directory store) imports, resolves via MVS against competing requirements, pins version + sha256 in `vactrol.lock`, exposes qualified and `open` names with the 5.7 lookup and collision warnings, registers its sample assets, and plays through the REPL; a FRESH-SESSION single file containing `import` followed by `pads.warm` (and its `as pd` variant) loads end to end; an LSP-ONLY analysis of an unopened document builds the same `AliasEnv` from `prescan_imports` without executing anything (unfetched package = `any` + "package not fetched" diagnostic); hash mismatch, unresolvable version, and a package's own compile error each yield a load diagnostic while the session keeps playing; re-import replaces the `PkgNs`; the proxy-shaped store is exercised against a local HTTP fixture — **Evidence**: (asserted exactly by `src/session/tests/packages.rs`: `mvs_and_the_lock_pin_versions_and_digests_the_session_loads`, `collisions_are_import_collision_warnings`, `the_asset_bank_plays_through_run_repl`, `a_fresh_session_loads_an_import_and_pads_warm_end_to_end`, `lsp_only_analyze_builds_the_same_alias_env_without_executing`, `load_failures_are_diagnostics_at_the_import_while_a_slot_plays`, `re_import_replaces_the_pkg_ns`; `src/pkg/tests/mvs.rs` (3 tests); `src/pkg/tests/proxy.rs` `proxy_store_over_the_local_http_fixture`; `src/pkg/tests/stores.rs` `dir_store_lists_versions_and_reads_manifests`, `git_store_over_a_local_bare_repository`; the spec block lang-reference #5 through `Session` without a lock pins package-not-locked@55-57 (`tests/spec_fixtures.rs`, `target/fe-logs/ss-final-fixtures-s186-1.log` 10/10); logs `target/fe-logs/ss-final-session-s186-1.log` exit=0)
- [x] Package archive safety (design 5.7 revised): CORRECTLY-HASHED malicious fixtures — a parent-traversal entry, an absolute path, a symlink escaping the package root (also listed as a manifest asset), duplicate (incl. case-fold) entries, and an over-limit archive — are each REJECTED with an integrity diagnostic naming the entry, with NO cache mutation; a native local-directory tree and a proxy zip of the SAME sources produce the SAME lock digest (portability asserted); SERIALIZATION INJECTIVITY (Astra's two-tree counterexample as a fixture): a tree containing a file whose name embeds a newline plus a hash-lookalike suffix versus the two-file tree it imitates — the malicious tree is REJECTED by control-character path validation, and independently the length-prefixed canonical bytes (and digests) of the two trees DIFFER; an interrupted extraction leaves no partial cache entry (staging + atomic publication asserted) — **Evidence**: (asserted exactly by `src/pkg/tests/validate.rs` `malicious_entries_are_rejected_naming_the_entry`, `caps_are_enforced`; `src/pkg/tests/zip.rs` `malicious_archives_are_rejected_naming_the_entry`, `bombs_sizes_methods_and_formats_are_refused`; `src/pkg/tests/cache.rs` `each_malicious_fixture_is_rejected_and_leaves_the_cache_unchanged`, `a_directory_symlink_escaping_the_root_is_rejected`, `an_interrupted_extraction_leaves_no_entry`; `src/pkg/tests/digest.rs` `portability_a_directory_tree_and_a_zip_give_the_same_digest`, `injectivity_the_newline_name_tree_is_rejected_and_its_bytes_differ`; log `target/fe-logs/ss-final-session-s186-1.log` exit=0)
- [x] Directives: the spec's own examples parse into the `DirectiveTable` with correct attachment (same-line trailing, consecutive-line block to the nearest preceding statement/block at no-deeper indentation, whole-`inst` binding at column 0, Addressed `hats.hpf` and file-level `#@ midi ch: 1`); labels resolve (explicit and implicit) and label-identified bindings survive a line move that would re-key a positional site; unknown call-site/param/label directives yield editor-validation diagnostics and never affect evaluation; a MIDI-learn writes the CC number back into the directive text via the validated edit path; both `BindingPersistence` impls round-trip the same binding set — **Evidence**: (asserted exactly by `src/directives/tests/attach.rs` `spec_examples_attach`, `spec_examples_resolve`; `src/directives/tests/labels.rs` (4 tests); `src/directives/tests/writeback.rs` (4 tests); `src/directives/tests/persist.rs` `directive_mode_round_trips_without_overlays`, `external_file_mode_round_trips_with_overlays`; `src/session/tests/directives.rs` `a_label_key_survives_a_line_move_that_re_keys_a_positional_site`, `unknown_site_param_and_label_directives_warn_and_never_change_evaluation`, `learn_produces_a_validated_directive_edit_and_a_stale_epoch_is_rejected`, `external_file_learn_produces_no_edit_and_updates_the_set`; log `target/fe-logs/ss-final-session-s186-1.log` exit=0)
- [x] Directive vocabulary (13.5 PROPOSED grammar): `cc: 74 _ 30` skips the second declared parameter; fewer `cc:` numbers than parameters leaves the remainder panel-only; a directive `ch:` overrides the file-level `#@ midi ch:` default; a `duplicate-label` collision (explicit vs explicit AND explicit vs implicit) is diagnosed and its addressed references rejected, with the affected sites falling back to positional provenance; ADDRESSED SELECTOR RESOLUTION (design 13.5): `analog.cutoff` selects the inst's declared PARAMETER, `hats.hpf cc: 30` selects the hpf CALL SITE on the hats line with 30 mapping onto its first declared parameter (the authority's example, asserted exactly), a selector matching both a parameter and a call-site name yields `ambiguous-selector`, a repeated same-named call site is `ambiguous-selector` bare and selectable via the ordinal (`hats.lpf.2`, and positional `#@ lpf.2`); a reserved `range:` key yields `reserved-key`; each vocabulary fixture carries its authority-question disposition in the manifest until ratified — **Evidence**: (asserted exactly by `src/directives/tests/resolve.rs` `addressed_parameter_selects_the_inst_parameter`, `addressed_call_site_maps_onto_its_first_declared_parameter`, `parameter_and_call_site_clash_is_ambiguous`, `repeated_call_sites_need_an_ordinal`, `cc_underscore_skips_and_fewer_numbers_leave_panel_only`, `ch_overrides_the_file_default`, `reserved_and_unknown`; `src/directives/tests/labels.rs` `duplicate_explicit_labels`, `explicit_label_colliding_with_an_implicit_one`; `tests/directive_fixtures.rs` `every_case_is_in_the_authority_question_channel`, `vocabulary_cases_resolve_as_expected`; log `target/fe-logs/ss-final-session-s186-1.log` exit=0)
- [x] Binding identity (design 13.5 `BindingKey`): simultaneous INDEPENDENT mappings and overlays on `hats.lpf` and `hats.hpf` (same `cutoff` keyword) never cross-talk or overwrite each other, and likewise for repeated `hats.lpf.1` / `hats.lpf.2`; both survive save and read-back in BOTH persistence modes; moving the labeled line keeps every binding attached (label component position-free); reordering the two same-named `lpf` sites MIGRATES each binding with its site through change-set span mapping, and an edit that breaks the mapping marks the binding STALE for re-confirmation instead of guessing (positional-fallback recovery asserted) — **Evidence**: (asserted exactly by `src/directives/tests/key.rs` `keys_are_the_full_selector_path`, `moving_the_labeled_line_keeps_every_key`, `reordering_same_named_sites_migrates_each_binding`, `a_broken_mapping_marks_the_key_stale`; `src/directives/tests/persist.rs` `no_cross_talk_between_modes`, `directive_mode_round_trips_without_overlays`, `external_file_mode_round_trips_with_overlays`; log `target/fe-logs/ss-final-session-s186-1.log` exit=0)
- [x] Stale `set-tweak` (old `form_gen`) is rejected; a debounce-race write stamped with an `edit_epoch` newer than the session's last `doc-changed` for its form is rejected; an edit WITHOUT re-evaluation (`doc-changed`) invalidates intersecting sites — a delayed write for them is rejected while unrelated sites stay valid; an insertion before a site followed by an edit to the shifted site invalidates the correct site through composed change-set coordinate mapping in the runtime; `set-var` with a superseded `defining_form_gen` is rejected; `reeval` sites rebuild their owning form at the boundary with override inheritance; `manual` sites update the slot without any replay; `direct` sites (including a probabilistic parameter) update per the 11.3 per-tier cell contract — within the commit horizon natively, after batch application on the browser model (asserted per tier, never as one unconditional claim) — **Evidence**: (asserted exactly by `src/session/tests/authority.rs` `a_stale_form_gen_set_tweak_is_rejected`, `a_debounce_race_write_is_unreconciled_until_the_matching_doc_changed`, `doc_changed_invalidates_intersecting_sites_only_and_rejects_a_delayed_write`, `an_insertion_then_an_edit_to_the_shifted_site_uses_composed_mapping`, `a_mismatched_base_revision_invalidates_the_whole_file`, `a_superseded_defining_form_gen_set_var_is_rejected`; `src/session/tests/tiers.rs` `reeval_sites_rebuild_their_form_with_override_inheritance`, `manual_sites_update_the_slot_with_no_replay`, `direct_native_a_committed_voice_hears_the_write_within_the_commit_horizon`, `direct_browser_the_write_is_heard_after_the_batch_applies`, `direct_probabilistic_parameter_native_model`, `direct_probabilistic_parameter_browser_model`; log `target/fe-logs/ss-final-session-s186-1.log` exit=0)
- [ ] Audible integration gate: a pattern bound from the REPL sounds on the native host (TASK-008), recorded in the progress log — **Evidence**: PENDING USER CONFIRMATION (manual: `vactrol repl` on a real output device, then `s :analog > note [:a4] > d1`). Automated proxy: `src/session/tests/repl.rs` `audible_gate_proxy_a_repl_bound_pattern_reaches_the_audio_host` (log `target/fe-logs/ss-final-session-s186-1.log` exit=0). Per issue #4 this does not block acceptance
- [x] REPL: failed expression does not write `_1`; session survives all failure classes — **Evidence**: (asserted exactly by `src/session/tests/repl.rs` `a_failed_expression_binds_no_register`, `the_session_survives_every_failure_class`, `continuation_lines_form_one_entry`; `tests/cli.rs` `repl_with_noop_host_evaluates_piped_stdin`, `run_with_noop_host_and_cycles_exits_zero`, `run_with_a_type_error_exits_three_and_prints_the_code`, `get_with_dir_store_writes_manifest_and_lock_then_raises_the_version`, `get_with_a_corrupted_fixture_exits_one_with_package_integrity`, `serve_with_noop_host_round_trips_over_the_session_socket`; logs `target/fe-logs/ss-final-session-s186-1.log` exit=0 and `target/fe-logs/ss-final-cli-s186-1.log` 9/9 exit=0)
- [x] LSP smoke test over stdio: publishDiagnostics + hover on spec examples — **Evidence**: (`tests/lsp_smoke.rs` `lsp_over_stdio_publishes_diagnostics_hovers_and_exits_cleanly`: publishDiagnostics for a lang-reference spec block and a hover containing the type, clean shutdown/exit; log `target/fe-logs/ss-final-lsp-smoke-s186-1.log` 1 passed, exit=0; LSP unit tests `target/fe-logs/ss-final-lsp-own-s186-1.log` 9/9 exit=0)
- [x] Build (+ `--features lsp`) + nextest pass — **Evidence**: (`target/fe-logs/ss-final-build-s186-1.log`, `target/fe-logs/ss-final-build-lsp-s186-1.log`, clippy `target/fe-logs/ss-final-clippy-s186-1.log` and `target/fe-logs/ss-final-clippy-lsp-s186-1.log`, `target/fe-logs/ss-final-fmt-s186-1.log`, nextest `target/fe-logs/ss-final-nextest-s186-1.log` 984 run/984 passed/1 skipped, cargo test `target/fe-logs/ss-final-cargotest-s186-1.log` 984 passed/0 failed, wasm32 `target/fe-logs/ss-final-wasm32-s186-1.log` and `target/fe-logs/ss-final-wasm32-hostwasm-s186-1.log`, all exit=0)

### TASK-010: Editor (browser + Tauri), visual feedback, controller binding
**Status**: COMPLETED (issue #5; every automated criterion is evidenced below; hearing worklet audio, the real-browser visual pane, `cargo tauri build` and the Tauri app run are PENDING USER CONFIRMATION) | **Parallelizable**: Partially (UI shell can start once TASK-009 message shapes are frozen)
**Depends on**: TASK-008, TASK-009
**Deliverables**: `editor/` TypeScript app per Module 8 editor half:
CodeMirror 6 with `.vact` mode fed by reader diagnostics; eval
keybindings + flash; inline static/runtime diagnostics (slot + beat);
playing-step highlighting scheduled on the audio clock, with
`doc_revision` span mapping through the editor change history
(unmappable highlights dropped); transport bar (tempo, cycle/beat,
hush/panic, per-slot mute/level meters); sample bank browser from
manifest; visual output panes o0..o3 (the WebGL2 `RenderHost` of
design 9.4 with ping-pong feedback buffers and compile-failure
recovery); controller binding UI per the Decided Editor Requirements:
the RIGHT-PANE SLIDER PANEL enumerating every numeric site from the
checker's AST — pattern/control literals, top-level `let`/`var`
numbers, and `inst` parameter defaults — grouped by site origin, each
slider switchable between SOURCE-EDIT mode (validated text edit +
live redefinition) and OVERLAY mode (runtime tweak, text untouched,
overlay value rendered beside the literal); DAW-STYLE PARAMETER
EDITORS rendered from `EditorDecl`/`ParamMeta` for call sites chosen
from the same enumeration (EQ band curve with the live spectrum
behind it, filter response, dynamics transfer with live gain
reduction, envelope stages, delay taps, sampler/wavetable/granular
views, LFO shape, XY pad, euclid ring, probability dial, length
handle — every handle writes to the same sites and is
MIDI-learnable, design 13.5); the STEP GRID and PIANO ROLL as
DISPLAY-ONLY renderings of playing telemetry (no write-back path
exists in these components); the directive-backed control panel
(directives/labels from the session's `DirectiveTable`, learned-CC
write-back into the comment); mouse drag on literals; WebMIDI learn
onto sliders or sites; `cc`/note-input device picker; MIDI clock
sync status in the transport bar; analyzer meters and scopes from
the 12.5 cells; package import UI with `vactrol get` and proxy
configuration; bindings keyed to labels where available, else
tracked text positions re-keyed per form generation, write-back
commit as a validated text edit; browser delivery (Vite, wasm
main-thread core + worklet); Tauri shell wrapping the same frontend
with minimal allowlist. No language syntax anywhere in bindings.
**Design ref**: sections 5.7, 9, 11.7, 12.5, 13, 15, 16, 17.
**Implementation amendment (issue #5, design 15.1.1 dispositions)**: delivered as eleven wave plans under
`impl-plans/active/` (`vactrol-editor-{scaffold,wire,code,midi,wasm,bind,visual,params,pkg,tauri,finalize}.md`)
dispatched by `impl-plans/active/ed-editor-20260926-s186-dispatch.json` (evidence root `tmp/ed-editor-20260926-s186/`).
- Rust additions limited to the additive gaps G1-G6 (design 15.1.2), protocol `v` stays 1: G1 the browser Session over
  the raw ABI (`src/host/wasm/session_half.rs`, `session_hosts.rs`, `abi.rs`); G2 `manifest.editors` on the wire
  (`src/session/editors.rs`); G3 site call identity (`site.call`); G4 analysis and clock telemetry; G5 per-frame
  uniforms and render records (`src/sched/render.rs`, `src/session/frontend.rs`); G6 browser packages
  (`src/pkg/driver.rs`, `mem_cache.rs`, `src/session/frontend.rs`).
- E2: browser self-analysis taps, browser MIDI out and raw-frame oscilloscope taps are deferred ("not available on this
  host"); meters and scopes render from the published analyzer cells only (no `getUserMedia`).
- E3: the transport bar shows per-slot ACTIVITY (event-driven), not per-slot levels; the master level is shown.
- E4: ExternalFile persistence is kept editor-side in the 14.5.8 `EditorBindingSet` format; the session is not switched.
- The Tauri shell is the standalone crate `editor/src-tauri/` (dialog-scoped fs only, 5 permissions) wrapping
  `editor/dist`.
**Completion criteria**:
- [x] Browser: eval a pattern, hear audio via worklet, see the sounding step highlighted within one lookahead window (manual + automated protocol test with mock clock)
  - **Evidence (automated proxy)**: `editor/test/wasm/criteria.test.ts` "criterion 1: ... stamps every event with the eval revision, within one lookahead of the tick that emitted it" and "HighlightScheduler on the mock clock activates exactly the playing step" (real host-wasm artifact `target/ed-wasm/ED-FINAL.wasm`); `editor/test/code/highlight.test.ts` (6 tests, mock clock); `editor/test/wasm/abi.test.ts` "eval yields an eval-result with sites; playing carries the eval doc_revision". Logs: target/fe-logs/ed-final-wasm-tests-s188-1.log and target/fe-logs/ed-final-wasm-tests-verbose-s188-1.log (3 files, 20 tests, exit=0); target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
  - **PENDING USER CONFIRMATION**: hearing audio through the worklet in a real browser (manual: `cd editor && npm ci && npm run dev`, open the page, evaluate `s [:bd :sd :hh :sd] > d1`).
- [x] Dragging a literal / MIDI CC on a `direct` site changes sound with no re-eval, per the 11.3 per-tier cell contract (browser: after batch application — the delivery-dependent guarantee, not an unconditional commit-horizon claim); on a `reeval` site it rebuilds the owning form at the boundary (or reports `manual`); in OVERLAY mode no source change occurs until "commit"; in SOURCE-EDIT mode the text updates with validated live redefinition
  - **Evidence**: `criteria.test.ts` "a direct write needs no eval-result; the next site table carries its value", "a reeval write rebuilds its form in one bindings batch; the old form_gen is then stale", "a doc-changed touching a site makes a later write edit-invalidated"; `editor/test/bind/write.test.ts` overlay mode (one `set-tweak`, text unchanged; commit = verified edit + one eval; manual badge) and source-edit mode (validated edit + form eval; declines on drift); `editor/test/bind/drag.test.ts` "a drag on a site literal produces the same set-tweak as the slider"; `editor/test/bind/routing.test.ts` (CC routing). Logs: target/fe-logs/ed-final-wasm-tests-s188-1.log and target/fe-logs/ed-final-wasm-tests-verbose-s188-1.log; target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log. The audible change stays with the criterion-1 manual gate.
- [x] The slider panel lists every Decided site kind — a pattern literal, a top-level `let` number, and an `inst` parameter default — and an inst-default slider is heard, without re-evaluation, at the next voice using the default under the per-tier `Ctl::Cell` contract (browser: after its update batch applies); MIDI learn maps a CC to a slider; analyzer meters render from the analysis cells; the package UI imports the fixture package and surfaces its diagnostics
  - **Evidence**: `criteria.test.ts` "reports pattern-literal, binding and inst-default sites; lpf carries its call" and "an inst-default write is accepted with no eval-result and no rebuild of the inst form" (per-voice cell behavior: TASK-009 `src/session/tests/tiers.rs`); `editor/test/bind/panel.test.ts` "lists the three sites under their origin groups", "learn maps a CC to the inst-default slider and later CC events move it"; `editor/test/midi/learn.test.ts`; `editor/test/visual/meters.test.ts` "renders master rms, bands and analyzer entries"; `editor/test/visual/scopes.test.ts`; `editor/test/wasm/packages.test.ts` "an unlocked import is listed; importing it makes the next eval load it with no package-not-locked", "a tampered zip is package-integrity, shown by the pane"; `editor/test/pkg/ui.test.ts`. Logs: target/fe-logs/ed-final-wasm-tests-s188-1.log and target/fe-logs/ed-final-wasm-tests-verbose-s188-1.log; target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log. Hearing the inst default stays with the criterion-1 manual gate.
- [x] Parameter editors: a `peq` call site opens the EQ band editor with the live spectrum behind it; an `env-adsr` site opens the envelope editor; a `euclid` site opens the ring with draggable hits/steps/rotation; each dragged handle changes the same numeric site a slider would (asserted via the recording host) and is MIDI-learnable; the step grid and piano roll render sounding events and expose NO editing affordance for sequences (structural assertion: no write-back path)
  - **Evidence**: `editor/test/params/open.test.ts` "peq opens eq-curve, env-adsr envelope-shape, euclid euclid-ring"; `editor/test/params/eq.test.ts` "draws the bus analyzer cells behind the bands with the real spectrum mount"; `envelope.test.ts`; `euclid.test.ts` (hits/steps/rotation); `editor/test/params/handles.test.ts` (15 tests: each handle records the identical `set-tweak` / verified edit as the slider, learn identical); `editor/test/params/kinds.test.ts` (every ED-WIRE kind); `editor/test/params/displays.test.ts` "grid.ts imports no write-back module and registers no input handler", "roll.ts imports no write-back module ...", "pointer, keyboard and wheel events on both displays send nothing". Log: target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
- [x] Directive round trip in the editor: opening a file with the spec's example directives shows the declared control panel; MIDI-learning a mapped parameter updates the `#@` comment text; a dangling label reference renders as an editor diagnostic; switching `BindingPersistence` to ExternalFile leaves the source untouched while preserving the panel
  - **Evidence**: `editor/test/bind/directives.test.ts` (criterion 5: declared panel; learn applies the verified directive-edit; a directive diagnostic marks its entry (dangling label); switching to ExternalFile leaves the text byte-identical and preserves the panel; never applies a directive-edit in ExternalFile mode); `editor/test/bind/persistence.test.ts` (EditorBindingSet, E4). Log: target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
- [x] Saving is MODE-SCOPED (design 13/13.5 — two tests, no unconditional claim): in DIRECTIVE mode the saved `.vact` retains its `#@` comments including learned CC mappings, and contains no binding data outside comments (no language construct changes); in EXTERNALFILE mode the saved `.vact` contains no binding artifacts at all; in BOTH modes overlay values are absent from the source until an explicit commit
  - **Evidence**: `editor/test/bind/save.test.ts` "Directive mode: the saved .vact is the buffer with the learned #@ comment, and there is no sidecar", "ExternalFile mode: the saved .vact has no editor-written binding text and the sidecar holds the bindings"; overlay absence until commit: `write.test.ts` "sends exactly one set-tweak ...; the text is unchanged", "commit makes a verified edit". Log: target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
- [x] Multi-site binding round trip in the editor: learned CCs on `hats.lpf` and `hats.hpf` and on `hats.lpf.1`/`hats.lpf.2` operate their own sites simultaneously (recording-host assertion of independence), persist and read back per `BindingKey` in both modes, follow a line move of the labeled statement, migrate across a same-named site reorder, and surface the STALE re-confirmation state when the mapping is broken
  - **Evidence**: `editor/test/bind/multisite.test.ts` (7 tests, criterion 7: interleaved CCs reach only their own sites in both modes, read back per key, line move in both modes, same-named reorder migration, STALE on a broken mapping); `editor/test/bind/sites.test.ts`. Log: target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
- [x] Reactive displays: after `upd` on a var feeding a derived binding, the panel and value displays repaint from the single `bindings` batch — the derived value, its sliders, and the affected slot indicator update together, unrelated displays do not repaint (batch-driven, no per-form flicker); a failed recompute leaves the previous coherent values displayed with the form's diagnostic, and a BLOCKED form shows its previous value with the `blocked-on` badge until the recovery batch clears it; after a FAILED RETRY (TASK-005's REACHABLE X/Z/Y provisional-rollback trace) the panel shows X's restored pre-pass 7 — the provisional 1/2 is never rendered — a conditional-unblocking pass clears the badge and shows the new value while the failed upstream form keeps its diagnostic, and the equal-value STATUS-RECOVERY batch clears the badges of blocked dependents (including one blocked TOWARD the repaired form) even though the repaired display value is unchanged; a LATE FAILURE (trace 3) surfaces the reader's `blocked-on` badge with its restored pre-pass value even though the failed producer changed no value, and changing the attempt-edge selector clears it without repairing the failed form; an ORDINARY runtime failure (TASK-005's a/b division case) shows the retained value with its diagnostic and then, after `upd b 1` alone, the repaired value with the diagnostic cleared — no manual re-evaluation affordance is needed for recovery
  - **Evidence**: `editor/test/bind/reactive.test.ts` (13 tests, criterion 8: one repaint per affected row from the single `bindings` batch for provisional rollback, abort/retry, conditional unblocking, status recovery, switch-toward, late failure, changing edge, ordinary failure, `upd b 1` recovery, failed diamond (+recovery), newly discovered selector; no provisional value rendered). Log: target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
- [x] Sampler waveform editor (design 13.5 revised, per the Decided sampler item): start/end/loop handles drag the `begin`/`end` literal sites; `slice n` and `chop`/`striate` counts render as grid overlays; MANUAL slice markers are draggable and each drag writes its point literal through the standard source-edit/overlay path (recording-host assertion); clicking a slice writes the clicked index INTO the currently selected, existing index literal via validated write-back and NEVER adds/removes/reorders steps (structural assertion: no sequence-editing affordance; no selection = no-op with hint); the bank index `n` opens the sample browser with per-entry waveform previews
  - **Evidence**: `editor/test/params/sampler.test.ts` (8 tests, criterion 9: begin/end handles, slice 8 / chop 4 overlays, manual markers in overlay and source-edit, slice click writes the selected index literal only, no-selection hint, no step add/remove/reorder API, `n` opens the sample browser with previews); `editor/test/code/samples.test.ts`. Log: target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
- [x] Edit-reconciliation tests: insertion/deletion/reordering above and inside a playing form keeps highlights and bindings attached or drops them cleanly; duplicate literals bind independently; an edit without re-evaluation invalidates its sites (`doc-changed`) and a delayed controller message is rejected as stale; write-back declines when the mapped text no longer matches; a stored list evaluated under an earlier document revision highlights against that revision through the event's own `SrcRef`
  - **Evidence**: `editor/test/code/reconcile.test.ts` (7 tests, highlight half: insertion above, reorder, deletion/edit inside dropped, duplicates independent, an earlier `doc_revision` highlighted through its own src); `editor/test/bind/reconcile.test.ts` (7 tests, bindings half: edit without re-eval then delayed CC -> STALE on edit-invalidated, write-back declines on mismatch, duplicates independent); `editor/test/code/history.test.ts`, `sync.test.ts`; `criteria.test.ts` "a doc-changed touching a site makes a later write edit-invalidated". Logs: target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log; target/fe-logs/ed-final-wasm-tests-s188-1.log and target/fe-logs/ed-final-wasm-tests-verbose-s188-1.log.
- [x] Visual pane: `osc 20 > rotate 0.5 > out o0` renders and activates at the cycle boundary; `text "hello"` renders via its rasterized `TextAsset`; a broken chain keeps the previous frame rendering and shows the diagnostic (TASK-008's lifecycle checks already passed in its own dev harness; this task may re-run them in the full editor)
  - **Evidence (automated proxy)**: `criteria.test.ts` "osc > rotate renders from the cycle boundary; text renders its asset; a broken chain keeps the previous program" (real artifact + `GlRenderHost` over `RecordingGL`); `editor/test/visual/render-host.test.ts` (ping-pong feedback, compile/link failure keeps the previous program, empty program clears to black); `text-asset.test.ts`; `panes.test.ts` (diagnostic banner). Logs: target/fe-logs/ed-final-wasm-tests-s188-1.log and target/fe-logs/ed-final-wasm-tests-verbose-s188-1.log; target/fe-logs/ed-final-npm-test-s188-1.log (54 files, 336 tests, exit=0) and target/fe-logs/ed-final-npm-test-verbose-s188-1.log.
  - **PENDING USER CONFIRMATION**: the real-browser visual pane (manual: `npm run dev`, evaluate `osc 20 > rotate 0.5 > out o0`).
- [x] Tauri build runs the identical frontend; `npm run build` and frontend tests pass
  - **Evidence**: target/fe-logs/ed-final-tauri-fetch-s188-1.log, target/fe-logs/ed-final-tauri-check-s188-1.log (`cargo check --manifest-path editor/src-tauri/Cargo.toml`, exit=0), target/fe-logs/ed-final-tauri-fmt-s188-1.log (exit=0), target/fe-logs/ed-final-tauri-perms-s188-1.log (5 permissions, `true`), target/fe-logs/ed-final-root-cargo-untouched-s188-1.log (exit=0); `tauri.conf.json` `frontendDist: ../dist` (the identical frontend); target/fe-logs/ed-final-npm-ci-s188-1.log, target/fe-logs/ed-final-npm-check-s188-1.log, target/fe-logs/ed-final-npm-test-s188-1.log, target/fe-logs/ed-final-npm-build-s188-1.log (`VACTROL_REQUIRE_SESSION_ABI=1`), target/fe-logs/ed-final-dist-check-s188-1.log, all exit=0.
  - **PENDING USER CONFIRMATION**: `cargo tauri build` and running the Tauri app (manual: `cd editor && npm run build && cargo tauri build`, open a `.vact` file).

---

### TASK-011: Editor UI on Solid.js, audio start, icons, foldable pane
**Status**: Completed (2026-09-27: transport, code mount point, foldable shell, and right-pane views are Solid components; the protocol, write-back, CodeMirror internals, canvas drawing, and WebGL2 render host remain framework-free) | **Parallelizable**: Partially
**Depends on**: TASK-010
**Deliverables**: design 15.2. `editor/` views ported to Solid.js
(`vite-plugin-solid`, `solid-js`; tests with `@solidjs/testing-library`
under jsdom): app shell and layout, transport bar with the audio-state
control and inline-SVG icons (metronome/tempo, cycle.beat ring, clock
source glyph, hush, panic) each with aria-label and tooltip, toolbar Run
button, eval outcome status, boot-failure status surface, foldable right
pane (rail, shortcut, per-viewer persistence, per-section folds), and the
right-pane panels (slider panel, directive control panel, parameter
editors, step grid, piano roll, meters/scopes, sample browser, package
pane) as Solid components over signals derived from the protocol client
(one repaint per `bindings` batch). CodeMirror 6 mounted from a
component; protocol, transports, bind write-back and render host stay
framework-free modules.
**Design ref**: section 15.2 (and 13, 14.4, 15.1 behaviors unchanged).
**Completion criteria**:
- [x] Every TASK-010 vitest suite passes against the Solid views (ported, none deleted or weakened); `npm run check`, `npm run test`, `npm run build` exit 0 — **Evidence** (2026-09-27): 350/350 vitest (349 before the new reactive-granularity test; 336 TASK-010 tests kept; the four pre-existing transport assertions that read visible words now read accessible names), `npx tsc --noEmit -p .` clean, `VACTROL_REQUIRE_SESSION_ABI=1 npm run build` ok
- [x] Audio state: before a gesture the control shows *off*; clicking it or evaluating resumes the context and shows *running*; a failed resume or boot failure (missing session ABI, worklet load error) is rendered in the status surface with its reason (tests with a fake AudioContext and failing loaders)
- [x] Eval outcome: Mod-Enter / Mod-Shift-Enter / Run each flash the range and show ok / n diagnostics / not delivered; the transport tempo updates from the session's `tempo` message after `use-bpm` (test through the real host-wasm artifact: evaluating examples/first-track.vact shows 124 bpm)
- [x] Transport renders icons only (no visible words for tempo label, clock source, hush, panic); each icon has an accessible name and tooltip (structural test)
- [x] Right pane folds to a rail and back via button and shortcut; the state survives reload (localStorage, try/catch); folding keeps MIDI-learned bindings and slider state (test); each pane section folds independently
- [x] Reactive granularity: one Solid signal per slider site and named value display; `test/bind/reactive.test.ts` proves that a one-site `bindings` batch updates only that site's render count and preserves neighboring row/value DOM nodes. Directive controls, parameter pane and handle rows, display-only grid and roll, analyzer displays, visual panes, sample browser, and package pane are Solid views.
- [x] Rust verification unchanged: no Rust source, Cargo manifest, or lockfile changed for TASK-011; prior build, clippy -D warnings, fmt, nextest, cargo test, both wasm32 builds, and Tauri `cargo check` evidence is recorded below and remains applicable. The requested editor verification was rerun on 2026-09-27.
- Evidence notes: `test/ui/controls.test.ts` (audio state off/running/suspended/failed/unavailable, eval pending/ok/diagnostics/not-delivered/superseded, Run button, icon-only transport, side fold by button and `Mod-\\`, persistence, per-section folds, throwing storage); `test/wasm/first-track.test.ts` (real host-wasm session: examples/first-track.vact evaluates with no errors, `tempo` 124 bpm, events on d1..d6, transport label `tempo 124.0 bpm`); headless Chrome against the dev server: boot, audio *running* after a click, Run -> *evaluated: ok*, tempo 124.0 bpm, slots d1..d6, no console errors. Hearing it remains a manual confirmation.

## Module Status

| Module | File Path | Status | Tests | Task |
|--------|-----------|--------|-------|------|
| Core values | `src/value/` | COMPLETED | 57 unit | TASK-001 |
| Reader | `src/reader/` | COMPLETED | 33 unit + 8 spec_fixtures (shared) | TASK-002 |
| Expander | `src/expand/` | COMPLETED | 55 unit + 8 spec_fixtures (shared) | TASK-003 |
| Checker | `src/types/` | COMPLETED | 79 unit + spec_fixtures (shared) | TASK-004 |
| Namespace/compiler/VM | `src/ns/ src/compile/ src/vm/` | COMPLETED | 161 unit (incl. integrate_*) + spec_fixtures (shared) | TASK-005 |
| Pattern engine + visuals + clock | `src/pattern/ src/tex/ src/clock/` | COMPLETED | 80 unit (+ integrate_pattern/integrate_tex under src/vm/tests) | TASK-006 |
| Scheduler + hosts | `src/sched/ src/host/` | COMPLETED | sched 47 + midi 17 + host contracts/native + e2e sched_gaps 8 (779 crate-wide) | TASK-007 |
| DSP + audio hosts | `src/dsp/ src/host/{native,wasm}/` | COMPLETED (beep audible check pending user confirmation) | dsp 87 + e2e (beep, templates, regions, buses) 20 + dev harness 18 checks | TASK-008 |
| Session/REPL/LSP | `src/session/ src/pkg/ src/directives/ src/cli/ src/lsp/ src/main.rs` | COMPLETED (audible REPL gate pending user confirmation) | 984 nextest crate-wide (session 132 incl. pkg/directives/directive_fixtures, cli 9, lsp 9 + lsp_smoke 1 under `--features lsp`) | TASK-009 |
| Editor | `editor/` (+ G1-G6 in `src/host/wasm/ src/session/ src/sched/ src/pkg/ src/directives/`, `editor/src-tauri/`) | COMPLETED (worklet audio, real-browser visual pane and Tauri app run pending user confirmation) | vitest 54 files / 336 tests (incl. real-wasm abi 8, criteria 9, packages 3); 1007 nextest crate-wide; Tauri `cargo check` | TASK-010 |

## Dependencies

| Task | Depends On | Parallelizable | Status |
|------|------------|----------------|--------|
| TASK-001 | — | Yes | COMPLETED |
| TASK-002 | TASK-001 | Yes (with 001 tail) | COMPLETED |
| TASK-003 | TASK-002 | No | COMPLETED |
| TASK-004 | TASK-003 | Yes (with 005) | COMPLETED |
| TASK-005 | TASK-003; TASK-004 `types/masks.rs` | Yes (with the rest of 004) | COMPLETED |
| TASK-006 | TASK-001 (int. 005) | Yes (with 004/005) | COMPLETED |
| TASK-007 | TASK-005, TASK-006 | No | COMPLETED |
| TASK-008 | TASK-001, TASK-007 | Yes (pure DSP alongside 007) | COMPLETED |
| TASK-009 | TASK-004, 005, 007; 008 for the audible gate | No | COMPLETED |
| TASK-010 | TASK-008, TASK-009 | Partially | COMPLETED |

Crate dependencies (added per task, not up front): serde + serde_json
(TASK-009), tower-lsp + tokio behind `lsp` (TASK-009), cpal + midir
behind `host-native` (TASK-008, optional under the non-wasm32 target
table; tungstenite moves to TASK-009 with the session socket, B5);
`host-wasm` pulls no crate (raw `extern "C"` ABI, B5). Core modules stay
dependency-free.

## Verification

The Build, Tests, Lint and Wasm rows (plus the default-feature wasm32
build) were executed for TASK-001..003 in session 175 and for
TASK-004..006 in session 183 and for TASK-007..008 in session 186 (run 2) (both
wasm32 builds, plus `cargo test`, crate-wide `cargo fmt --check`, and for
TASK-008 the headless-Chrome dev harness; see the Progress Log); the LSP row
ran for TASK-009 and the Editor row (`npm ci && npm run check && npm run test && npm run build`) ran for
TASK-010 in session 188 (`target/fe-logs/ed-final-*-s188-1.log`).
Cargo runs use `CARGO_TERM_QUIET=true`; nextest runs use
`NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
NEXTEST_HIDE_PROGRESS_BAR=1`.

**Spec fixture manifest** (`tests/fixtures/spec/manifest.toml`, created
in TASK-002): every code block from lang-reference.md sections 1-4 and
design-music.md becomes fixtures classified `positive` (expected
value), `diagnostic` (expected code; run in an isolated session so
intentional errors such as rebinding or `upd` on a `let` do not poison
later cases), `illustrative-excluded` (placeholders such as `...`
bodies and undefined illustration names), or `authority-question`
(contradictory annotations recorded for the language author). Known at
planning time: lang-reference section 1 defines `fn f a b: * a 12` yet
annotates `f 1 2` as 24 — this ships as an authority question with
both candidate readings, and decided normative behavior remains the
implementation target; the implementer must not silently choose an
answer. Likewise the lang-reference section 4 reactive example
(`let base` then `upd base`) conflicts with the immutable-`let`/
`upd`-on-`var` rule: it ships as an authority-question fixture; the
required reactive tests are var-based (TASK-005), and `upd` on a
`let` is not silently legalized. Blanket "all spec examples pass" criteria are replaced by
manifest-scoped criteria in TASK-004/005/006. Each fixture's
disposition is tracked in the manifest.

| Check | Command | Expected |
|-------|---------|----------|
| Build | `CARGO_TERM_QUIET=true cargo build` | success, no warnings |
| Tests | `CARGO_TERM_QUIET=true cargo nextest run` (env above) | all pass |
| Lint | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` | clean |
| Wasm | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` | success (from TASK-001 on, core modules only until TASK-008) |
| LSP | `CARGO_TERM_QUIET=true cargo build --features lsp` | success |
| Editor | `npm run build && npm test` in `editor/` | success |

## Completion Criteria

- [x] All ten tasks complete with their per-task criteria checked, except the manual audible/browser/Tauri confirmations listed in the plan status (operator verification 2026-09-26 on 8aad85f: build, clippy -D warnings, fmt, nextest 1007/1007, both wasm32 builds incl. the host-wasm --lib session ABI, editor tsc/vitest 336/336/vite build, Tauri cargo check)
- [x] The spec fixture manifest is complete; all `positive` and `diagnostic` fixtures pass; every `authority-question` fixture has a recorded disposition
- [x] Visual chain golden and render-recovery tests pass (design 9) (in nextest 1007/1007)
- [ ] REPL performs an audible pattern session natively; browser build performs the same via Wasm + AudioWorklet
- [x] Editor shows inline diagnostics, playing-step highlighting, and controller binding with no language construct involved (vitest suites with mock clock and recording host; real-browser run pending user confirmation)
- [x] cargo build, clippy, nextest, wasm32 build all pass (commands above) (operator verification 2026-09-26 on 8aad85f: build, clippy -D warnings, fmt, nextest 1007/1007, both wasm32 builds incl. the host-wasm --lib session ABI, editor tsc/vitest 336/336/vite build, Tauri cargo check)
- [x] impl-plans/README.md index and design-docs/specs/command.md CLI table updated (deferred from planning phase, where only the two planning documents could be edited) — **Evidence**: design-docs/specs/command.md (Subcommands, Flags, Environment, Session Protocol v1; issue #4 design) and impl-plans/README.md (updated by SS-FINAL, session 186)

## Progress Log

### Sessions: 2026-09-24/25 (revisions 1-15, condensed; detail in the workflow discussion history)
**Tasks Completed**: None (planning)
**Blockers**: None at the time
**Notes**: Rev 1 authored both documents (Q1-Q4 remain recommendations). Rev 2 answered ASTRA-001..011. Revs 3-5: masks, activation, rebuild transactions, install credit, edit_epoch, Forward sets, control channel, Ctl::Cell reads.
Rev 6: cell transport, control merge, ParamMeta, `#@` directives, input-lane walk, two-phase imports. Rev 7: directive authority ADJUDICATED; PROPOSED vocabulary. Rev 8: cell bounds, per-tier timing, mode-scoped persistence, ordinals, tag-keyed release, granular buffers, dirhash.
Rev 9: incarnation state machine + INIT-ONCE + Const exception; `BindingKey`; unfreeze re-check; injective digest. Rev 10: reactive graph (eager edges, upd triggers, cutoff, post-pass batch, advisory deps); `let base`/`upd base` = authority-question fixture.
Rev 11: stale-read rounds with dependency-set replacement; all-path Failed/Blocked; authority re-pinned (architecture.md 434 lines a0bf8dbc…, design-music.md 380 lines d90db3b7…); sample-region operators in 10.1; 13.5 sampler waveform editor (slice-click writes only an existing index literal — structure stays code-only).
Rev 12: PASS JOURNAL + pass-level staging of all host-visible effects + end-of-pass VALIDATION/ROLLBACK; deref-time Failed/Blocked checks replacing the pre-execution gate; 10.1 whole-span region operators; invariance criteria in TASK-006/007/008.
Rev 13: unreachable provisional A=1/3 trace withdrawn; abort/retry + reachable X/Z/Y rollback traces; recovery subscriptions and status-recovery cutoff bypass; layer-1 cover equivalence vs layer-2 emission uniqueness; Event.occ/OccKey; occurrence merge with occ-keyed replacement + committed ledger.
Rev 14: FAILURE-SENSITIVE validation (pre-pass-read exemption withdrawn); ->Failed/Blocked as invalidation status events; late-failure trace 3; ATTEMPT EDGE SET on aborts (a/b/broken selector case); occurrence RECORDS with covered extents replacing occ-keyed replacement (future-span both-order onset preservation, boundary-crossing fragments, partial invalidation).
Rev 15: ASTRA-017 — every unsuccessful evaluation (failed-deref, dirty-read or ordinary runtime failure) retains the ATTEMPT EDGE SET of its successful reads (a/b division walked in 5.6); ASTRA-018 — SEMANTIC PAYLOAD REPLACEMENT for a surviving same-key uncommitted record whose whole.begin lies in the invalidated span (slice-index 0 -> 1 walked in 11.3).

### Session: 2026-09-25 (revision 16, TASK-001..003 implemented)
**Tasks Completed**: TASK-001 (FE-VALUE), TASK-002 (FE-READER), TASK-003 (FE-EXPAND), reconciled by FE-FINAL; the single front-end commit on `main` is left to the workflow commit step.
**Verification** (session 175, `target/fe-logs/final-<check>-s175-1.log`, all `exit=0`): build; clippy `--all-targets -- -D warnings`; nextest 155 run / 155 passed; cargo test (lib 147 + spec_fixtures 8 passed); fixtures 8/8; fmt `--check`; wasm32 and wasm32 `host-wasm` builds. Largest `.rs` file: `src/reader/line.rs`, 741 lines.
**User QA**: U1-U5 answered 2026-09-25 (`design-docs/user-qa/pending-frontend-questions.md`). TASK-004 pointer: the checker must diagnose a bare field-less variant used as a binding-if pattern (U5; fixture `u5-bare-variant-binding-if`).
**Plan revisions**: Lint is now `clippy --all-targets` (stricter); TASK-001..003 ran through the four FE-* sub-plans; design 6.5.4 records `misplaced-arrow` (plan choice) and `empty-pipe` (added by FE-READER); the expander's own depth cap of 512 (`nesting-too-deep`) is not yet written into 6.5.5.
**Open**: the `fn f a b` arity and `let`/`upd base` authority questions stay pending; design-music `[:g :7]` conflicts with the identifier rule and reads as `misplaced-colon` (fixture `music-chord-seven-conflict`) until the language author decides.

### Session: 2026-09-25 (issue #2, TASK-004..006 implemented)
**Tasks Completed**: TASK-004, TASK-005 and TASK-006, through the eight ME plans (dispatch manifest `me-middle-20260925-s175-dispatch.json`). ME-FINAL (session 183) reconciled them. The single implementation commit on `main` is left to the workflow commit step.
**Waves**:
- ME-MASKS: `types/masks.rs` mask inference, done first because the compiler consumes it.
- ME-FRONTEND: `path`/`url` literals in the reader, `Value` and the checker; `[:g :dom7]` chord keywords.
- ME-CHECK (TASK-004): HM-lite checker, the Q1 scope chain, SOUND FIRST, U5.
- ME-VM (TASK-005): namespace, compiler, VM, tweak sites, packages, Query mode.
- ME-PATTERN (TASK-006): pattern engine, region operators, occurrences, signals, clock, `tex/`.
- ME-REACTIVE (TASK-005): `DepGraph`, pass journal, rounds, traces, `Evaluator`, `load` behind `NoopHost`.
- ME-INTEGRATE: `QueryVm`, domain natives, prelude `sound-kit`/`default-sound-kit`, the check stage in `eval_form`/`load`, all fixtures classified. It also applied serial repairs R1 (`put` merges a dict element) and R2 (`shape_of` binds a non-variant name).
- ME-FINAL: join integrity, final gates, bookkeeping.
**Verification** (session 183, `target/fe-logs/final-<check>-s183-1.log`, all `exit=0`):
- build; clippy `--all-targets -- -D warnings`; crate-wide `cargo fmt --check` (no rewrite needed)
- nextest 489 run / 489 passed / 1 skipped
- cargo test: lib 479 passed; spec_fixtures 10 passed, 1 ignored (report)
- fixtures 10/10 (1 skipped)
- wasm32 and wasm32 `host-wasm` builds
- Largest `.rs` file: `src/compile/compiler.rs`, 792 lines. `std::(thread|fs|time|net|process)` in `src/`: none. `Cargo.toml`/`Cargo.lock` unchanged.
- Join integrity: `tmp/me-middle-20260925-s175/ME-FINAL/attempt-1/join-explanations.md`. ME-INTEGRATE final-hashes 140/140. Every earlier-plan mismatch is a hand-off to a later owner, or a checkpoint rewrite of a plan document.
- Additional read-only probes: `tmp/.../ME-FINAL/attempt-1/probe/`, run log `probe-run-3.log`.
**Fixture classes** (`tests/fixtures/spec/manifest.toml`):
- 11 blocks: 1 positive, 5 diagnostic, 5 deferred; 0 `unclassified`.
- 120 cases: 62 pinned values, 7 pinned failures, 2 `authority-question` (`aq-fn-arity`, `aq-let-upd-base`).
- Spec errata are pinned with TOML comments (ME-INTEGRATE log).
**Module tests** (`#[test]`): `types/` 79, `ns/`+`compile/`+`vm/` 161, `pattern/`+`tex/`+`clock/` 80.
**Residuals (accepted, not blocking)**:
- TASK-006 11.3 staging invalidation: only the re-query semantics are asserted. The staged-event invalidation is TASK-007's.
- The VM-built `osc 20 > rotate 0.5` passes its literals as `Late` uniforms (`u0`, `u1`; `osc` is all-Late). Its source is byte-stable but differs from the TexNode golden, which pins constants (probe P1). Pinning a VM-path golden is a follow-up for the visual owner.
- The real core `sin` does not lift signals, so the literal `osc {* 20 {sin time}}` fails `type` at uniform resolution. The Late-deferral test uses a counting stand-in.
- `kit:` given a `var` is heard through the reactive pass, not through `PParam::Late`.
- `map-range` supports `s lo hi` only.
**User QA**: M1-M3 answered. M4 (an unstructured sound plays one event per cycle) and M5 (`#` ends a url) follow the recommendations. M6 (two `let sound-kit` lines in one design-music block) is deferred and not pinned. The `fn f a b` arity and `let`/`upd base` authority questions stay pending (`aq-fn-arity`, `aq-let-upd-base`).
**Corrections to the revision-16 notes**:
- Design 6.5.5 now records the expander `MAX_DEPTH = 512` (`nesting-too-deep`).
- The design-music `[:g :7]` conflict is resolved by the letter-first chord decision: `[:g :dom7]`. `src/reader/tests/lexer.rs:90` stays the negative case.
**Design doc**: `design-implementation.md` is not edited. The ME progress logs record implementation refinements under "the design wins" (op encodings, `Arity` shape, `FormState::Blocked { on: SymId }`, checker splits), not accepted amendments to the design.

### Session: 2026-09-25 (issue #3, TASK-007..008 implemented)
**Tasks Completed**: TASK-007 and TASK-008 through the eight BE plans (dispatch manifest `be-backend-20260925-s181-dispatch.json`; waves CONTRACTS -> {SCHED, DSP, INST} -> {MIDI, NATIVE, WASM} -> FINAL). BE-FINAL (session 186) reconciled them; the single implementation commit on `main` is left to the workflow commit step.
**Verification** (session 186 run 2, after the test-integrity revision, `target/fe-logs/be-final-<check>-s186-2.log`, all `exit=0`; run 1 logs `-s186-1` superseded):
- build; clippy `--all-targets -- -D warnings`; crate-wide `cargo fmt --check` (no rewrite needed)
- nextest 779 run / 779 passed / 1 skipped; cargo test: lib 769 passed, spec_fixtures 10 passed (1 ignored report)
- fixtures 10/10; e2e (`host::tests::e2e`) 28/28; `cargo build --example beep`
- wasm32 and wasm32 `host-wasm` builds; `cargo tree -e normal --target wasm32-unknown-unknown` has no cpal/midir
- F8 dev harness on the final tree: `be-final-harness-s186-2.log` exit=0, report `be-final-harness-s186-2.json` (HeadlessChrome 154, 18/18 PASS, memoryStable true)
- Largest `.rs` file: `src/dsp/build.rs`, 799 lines; `std::(thread|fs|time|net|process)` outside `src/host/native`: none.
- Join integrity: `tmp/be-backend-20260925-s181/BE-FINAL/attempt-1/join-integrity.txt` (every mismatch is a later owner's recorded edit).
**Serial repairs in BE-FINAL** (intent and hashes in `tmp/be-backend-20260925-s181/BE-FINAL/attempt-1/`):
- R1: an `inst` header parameter at a call head is a pattern control at run time (`inst control` native; the checker's inst controls are seeded from the host manifest's template parameters). Before, `s :analog > cutoff 800` failed `undefined-name` at run time.
- R2: ugen inputs land on the runtime catalog port of the same name; template resource defaults (`bank`/`table`/`source`) route through the sample table. Before, `sampler`, `wavetable` and `granular` rendered silent and vco/svf were mis-wired.
- R3: commit writes the `bus` routing control. R4: commit resolves `speed-fit` (`splice`/`loop-at`/`fit`). R5: `loop-at` writes a bool `loop`. R6: effect parameters use effect-local ids (bus/master chain parameters were ignored). R7 (test-integrity revision): `degrade-by`/`maybe` drop the captured print output of the events they remove, so removed work never reaches the console (before, it was forwarded as keyless output once per query window).
- `HostManifest` carries the `dsp::meta` editor metadata (`editor_decl`, `editor_decls`, `template_params`, 13.5).
**Fixture classes**: design-music ordinals 2, 4, 6 are `diagnostic` and 5 is `positive` (none deferred to TASK-008); 11 blocks: 2 positive, 8 diagnostic, 1 deferred (lang-reference #5, TASK-009).
**Pending user confirmation**: the audible `cargo run --example beep` check (TASK-008 criterion 10).
**Residuals (not blocking)**: one-line `inst NAME: BODY` is not a definition head at run time (design-music #4 lines 28-29); `[1 0.5] > osc "/x"` fails `type` at run time while the checker accepts it (#2 line 100); `effect_ports` names for `balance` and ~36 single-parameter effects do not match their parameter names (now an explicit lowering error); bus parameters given patterns fail `inst-failed` (#6 line 7).
**Design doc**: not edited; TASK-008 deliverable text amended here per B5 (session socket to TASK-009, raw `extern "C"` ABI).

### Session: 2026-09-26 (issue #4, TASK-009 implemented)
**Tasks Completed**: TASK-009 through the eight SS plans (dispatch manifest `ss-session-20260925-s183-dispatch.json`, checkpoint 9d6db6e; waves CONTRACTS -> {PKG, DIRECTIVES, ANALYSIS} -> SESSION -> {CLI, LSP} -> FINAL). SS-FINAL (session 186) reconciled them; the single implementation commit on `main` is left to the workflow commit step.
**Verification** (SS-FINAL final tree, `target/fe-logs/ss-final-<check>-s186-1.log`, all `exit=0`):
- build; build `--features lsp`; clippy `--all-targets -- -D warnings` (default and `--features lsp`); crate-wide `cargo fmt --check` (no rewrite needed)
- nextest 984 run / 984 passed / 1 skipped; cargo test: lib 963, cli 9, directive_fixtures 2, spec_fixtures 10 passed (984), 0 failed (1 ignored report)
- fixtures 10/10; cli 9/9; session + pkg + directives + directive_fixtures 132/132; lsp_smoke 1/1 and lsp unit 9/9 (`--features lsp`); `cargo build --example beep`
- wasm32 and wasm32 `host-wasm` builds; `cargo tree -e normal --target wasm32-unknown-unknown` (both) has no tungstenite, getrandom, tokio, tower-lsp, cpal or midir
- Largest `.rs` file: `src/dsp/build.rs`, 799 lines; `std::(thread|fs|time|net|process)` outside native/cli/lsp/tests: none.
- Join integrity: `tmp/ss-session-20260925-s183/SS-FINAL/attempt-1/join-integrity.txt` (every mismatch is a later owner's seed fill, a checkpoint commit, or an operator note).
**Fixture classes**: lang-reference #5 moves from `deferred` (TASK-009) to `diagnostic`, evaluated through `vactrol::session::Session` (`eval_via = "session"`, `tests/support/eval.rs` `session_eval`); no block is deferred (`grep -c 'deferred_to = "TASK-009"'` prints 0). 11 blocks: 2 positive, 9 diagnostic.
**Pending user confirmation**: the audible REPL gate (TASK-009 criterion 8; proxy `audible_gate_proxy_a_repl_bound_pattern_reaches_the_audio_host`) and the TASK-008 beep check.
**Residuals (not blocking)**: a fn-valued pattern parameter is called with the event time, so the spec's zero-parameter `fn kick-sound:` (lang-reference #5 line 12) faults `arity` (recorded for the language author); pending-session-questions S1-S6 stay open; LSP code is built only under `--features lsp` (covered by the lsp gates above).
**Design doc**: not edited; TASK-009 deliverable text amended here per design 14.5.1.
**Serial repair after integration review (IR-S185-W5-BUSNAMES)**: the native audio host now shares the session's
instrument registry (`SessionConfig::insts`, `NativeHosts::open_with_bus_names`, wired in `cli::build_session_native`),
so named-bus taps and captures resolve on the native host. Before this repair they always failed with host-unavailable.
Test: `src/session/tests/analysis.rs`
`a_named_bus_scope_resolves_through_the_native_host_sharing_the_session_registry`, with a positive case and a negative
control. Re-verified final tree (`target/fe-logs/ss-reconcile-<check>-s185-r1.log`, all `exit=0`): nextest 985/985,
nextest `--features lsp` 994/994, cargo test lib 964, cli 9, directive_fixtures 2 and spec_fixtures 10, session+pkg+directives
133/133, lsp_smoke 1/1, lsp unit 9/9, and every build, clippy, fmt, example and wasm32 gate.

### Session: 2026-09-26 (issue #5, TASK-010 implemented; sessions 186-188)
**Tasks Completed**: TASK-010 (COMPLETED; manual gates pending user confirmation)
**Notes**: Eleven wave plans (`impl-plans/active/vactrol-editor-*.md`) dispatched by
`impl-plans/active/ed-editor-20260926-s186-dispatch.json`: SCAFFOLD -> {WIRE, CODE, MIDI} -> {WASM, BIND, VISUAL} ->
{PARAMS, PKG, TAURI} -> FINAL. SCAFFOLD, CODE, MIDI, VISUAL and PKG were accepted in session 186; WIRE, WASM, BIND,
PARAMS and TAURI in session 187. The ED-FINAL session-187 blocker (ED-CODE `highlight.ts` read `playing.dur` as cycles;
the wire sends beats) was repaired by the operator before session 188 (checkpoint ea95f1b).
- ED-FINAL session 188 (attempt-2, `tmp/ed-editor-20260926-s186/ED-FINAL/attempt-2/`): join integrity re-checked (184
  OK; every mismatch explained); full-tree verification, all exit=0 (`target/fe-logs/ed-final-*-s188-1.log`):
  - build, build-lsp, clippy and clippy-lsp (`-D warnings`), fmt --check;
  - nextest 1007 run / 1007 passed / 1 skipped; cargo test (lib 986, cli 9, directive_fixtures 2, spec_fixtures 10 + 1
    ignored);
  - both wasm32 builds, host-wasm `--lib` re-uplift, ED-FINAL.wasm with 48 exports and `session_init`, clippy wasm32
    host-wasm, dependency trees clean, largest `.rs` 799 lines and largest `.ts` 547 lines;
  - npm ci / check / test (54 files, 336 tests) / build (`VACTROL_REQUIRE_SESSION_ABI=1`), dist check, real-wasm
    vitest (abi, criteria, packages: 3 files, 20 tests);
  - Tauri fetch / check / fmt, 5 permissions, root Cargo files untouched;
  - session subset 233 passed, lsp_smoke 1 passed, spec fixtures 10 passed; frozen files unchanged; manifest valid JSON;
    no `getUserMedia` call in `editor/src` (one doc comment only).
- PENDING USER CONFIRMATION: hearing worklet audio in a real browser, the real-browser visual pane, `cargo tauri build`
  and the Tauri app run.

## Related Plans

- **Previous**: none (first plan of the project)
- **Next**: Frozen-mode codegen and Swift/UniFFI shell (future plans, out of scope here); TASK-010 was split into the `vactrol-editor-*.md` wave plans (issue #5)
- **Depends On**: none
- **Front-end sub-plans (TASK-001..003)**: vactrol-frontend-value.md (FE-VALUE), vactrol-frontend-reader.md (FE-READER), vactrol-frontend-expander.md (FE-EXPAND), vactrol-frontend-finalize.md (FE-FINAL), archived in impl-plans/completed/
- **Middle-end sub-plans (TASK-004..006, issue #2; all Completed, archiving to impl-plans/completed/ after the workflow commit)**: vactrol-middle-masks.md (ME-MASKS), vactrol-middle-frontend.md (ME-FRONTEND), vactrol-middle-check.md (ME-CHECK), vactrol-middle-vm.md (ME-VM), vactrol-middle-pattern.md (ME-PATTERN), vactrol-middle-reactive.md (ME-REACTIVE), vactrol-middle-integrate.md (ME-INTEGRATE), vactrol-middle-finalize.md (ME-FINAL), in impl-plans/active/
- **Back-end sub-plans (TASK-007..008, issue #3; all Completed, archiving to impl-plans/completed/ after the workflow commit)**: vactrol-backend-contracts.md (BE-CONTRACTS), vactrol-backend-sched.md (BE-SCHED), vactrol-backend-dsp.md (BE-DSP), vactrol-backend-inst.md (BE-INST), vactrol-backend-midi.md (BE-MIDI), vactrol-backend-native.md (BE-NATIVE), vactrol-backend-wasm.md (BE-WASM), vactrol-backend-finalize.md (BE-FINAL), in impl-plans/active/; dispatch manifest impl-plans/active/be-backend-20260925-s181-dispatch.json
- **Session sub-plans (TASK-009, issue #4; all Completed, archiving to impl-plans/completed/ after the workflow commit)**: vactrol-session-contracts.md (SS-CONTRACTS), vactrol-session-pkg.md (SS-PKG), vactrol-session-directives.md (SS-DIRECTIVES), vactrol-session-analysis.md (SS-ANALYSIS), vactrol-session-core.md (SS-SESSION), vactrol-session-cli.md (SS-CLI), vactrol-session-lsp.md (SS-LSP), vactrol-session-finalize.md (SS-FINAL); manifest impl-plans/active/ss-session-20260925-s183-dispatch.json
- **Editor sub-plans (TASK-010, issue #5; all Completed, archiving to impl-plans/completed/ after the workflow commit)**: vactrol-editor-scaffold.md (ED-SCAFFOLD), vactrol-editor-wire.md (ED-WIRE), vactrol-editor-code.md (ED-CODE), vactrol-editor-midi.md (ED-MIDI), vactrol-editor-wasm.md (ED-WASM), vactrol-editor-bind.md (ED-BIND), vactrol-editor-visual.md (ED-VISUAL), vactrol-editor-params.md (ED-PARAMS), vactrol-editor-pkg.md (ED-PKG), vactrol-editor-tauri.md (ED-TAURI), vactrol-editor-finalize.md (ED-FINAL); dispatch manifest impl-plans/active/ed-editor-20260926-s186-dispatch.json
- **Middle-end dispatch manifest**: impl-plans/active/me-middle-20260925-s175-dispatch.json (waves: MASKS -> FRONTEND -> CHECK/VM/PATTERN -> REACTIVE -> INTEGRATE -> FINAL)
