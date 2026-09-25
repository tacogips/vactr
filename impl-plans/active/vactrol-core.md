# Vactrol Core Implementation Plan

**Status**: In Progress
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

**Status**: NOT_STARTED — TASK-004, design section 7

```rust
pub enum Ty { Int, Int64, Float, Float64, Ratio, Bool, Str, KeywordOf(KeySet), Nil,
    Opt(Box<Ty>), List(Box<Ty>), Dict(Box<Ty>, Box<Ty>), Fn(Box<[Ty]>, Box<Ty>),
    Pattern(Box<Ty>), Signal, Named(TypeId), Any, Var(TyVar) }
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

**Status**: NOT_STARTED — TASK-005, design sections 5.5–5.6, 8, 13 (tweak slots)

```rust
pub struct VarSlot { pub name: SymId, pub kind: SlotKind, pub value: RefCell<Value>,
    pub version: Cell<u64> }
pub enum SlotKind { Let, Var, Fn, Tweak }
pub struct Namespace;  // define/redefine/lookup; SINGLE global scope incl. prelude,
                       // no-shadowing per scope; prelude = read-only parent scope (design 20 Q1 decided 2026-09-25)
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

**Status**: NOT_STARTED — TASK-006, design sections 9, 10, 11.1

```rust
pub struct TimeSpan { pub begin: Ratio64, pub end: Ratio64 }
pub struct Event { pub whole: Option<TimeSpan>, pub part: TimeSpan, pub value: Value,
    pub controls: BTreeMap<KwId, Value>, pub src: Option<SrcRef>, pub occ: OccKey }
    // src: ORIGINATING SrcRef from ListProv, carried end to end, never re-wrapped (10.2, 11.6)
pub struct OccKey { pub path: SmallVec<[(NodeId, u32); 8]>, pub anchor: Ratio64, pub cycle: i64 }
    // stable logical OCCURRENCE IDENTITY (structural path ordinals + whole.begin + cycle — design 10.2 revised): equal across query windows for the same note, DISTINCT for identical stack/branch twins; keys the 11.3 occurrence records (coverage-extension merge, never deleting) and committed ledger (at most one emission per occ per slot/gen)
pub struct Pat { pub node: PatNode, pub span: Option<Span> }
pub enum PatNode { Steps(Box<[Step]>), Signal(Rc<Sig>), Fast(Rc<Pat>, PParam),
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
    MidiNotes { channel: Option<u8> } }  // live input pattern: empty under pure
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

**Status**: NOT_STARTED — TASK-007, design sections 11.2–11.6

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

**Status**: NOT_STARTED — TASK-008, design sections 12, 16

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
pub struct NativeAudioHost; // cpal stream + timer tick + midir + tungstenite session socket
pub struct WasmHost;   // wasm-bindgen main-thread half; worklet half runs dsp only;
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

**Status**: NOT_STARTED — TASK-009, TASK-010, design sections 13–16

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
**Status**: NOT_STARTED | **Parallelizable**: Yes (with TASK-005; both consume TASK-003 output)
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
- [ ] Fixtures per the spec fixture manifest type-check according to their classification; annotated negatives (`+ 1 "a"`, `?T` used as `T`, rebinding) produce the expected diagnostic codes in isolated sessions
- [ ] Checker never aborts evaluation in Live mode (returns diagnostics + best-effort types)
- [ ] Same `check()` entry consumed by tests for LSP reuse (no LSP dependency in `types/`)
- [ ] Build + nextest pass (planned)

### TASK-005: Namespace, compiler, bytecode VM, tweak slots
**Status**: NOT_STARTED | **Parallelizable**: Yes (with the rest of TASK-004)
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
- [ ] All `positive` fixtures in the spec fixture manifest produce their annotated `# => v` values via read->expand->compile->run; `diagnostic` fixtures produce their expected codes in isolated sessions; `authority-question` and `illustrative-excluded` fixtures are tracked with dispositions, never silently reinterpreted (see Verification)
- [ ] Forcing-mask tests: an ignored effectful thunk argument runs exactly once; a doubly-read `Value` parameter forces once; a wrapper that forwards its parameter TWICE to a numeric consumer runs an effectful counter thunk EXACTLY ONCE (boundary chase, not per-use forcing); a CONDITIONAL wrapper whose forwarding call is skipped STILL forces exactly once at its boundary; boundary effect ORDER is left-to-right; a wrapper forwarding to `at` passes the thunk through unevaluated; a time-varying visual argument (`osc {* 20 {sin time}}`) stays deferred via its `Late` mask; a FAN-OUT forward whose destinations currently disagree (`Value` vs `Fn`) forces once and carries the `mixed-forcing` diagnostic; an `Undetermined` parameter runs at most once (memoized) with the `latent-forcing` diagnostic; mask derivation with the diagnostics pass disabled yields identical masks; redefining the DIRECTLY-called callee between `Value`/`Fn` switches at the next call; redefining only a DOWNSTREAM callee an unchanged wrapper forwards to (change `at` among `Value`/`Fn`/`Late`, `later` untouched) switches `later`'s forcing at its next call (transitive-staleness regression)
- [ ] Package namespaces: qualified `pads.warm` compiles through `PkgNs`; the 5.7 lookup order holds (locals > session > open imports most-recent-wins with collision warning > prelude); re-import replaces the whole `PkgNs`
- [ ] Query effect mode: `upd` on a global inside a queried closure fails with `effect-in-query` and origin; frame-local state remains legal; `print` is captured into `QueryResult.output` and never written to a host from Query mode; the scope guard restores Normal mode after an unwinding failure
- [ ] Rebuild tests: a PARTIALLY superseded multi-output form (binds d1 and d2, then another form replaces d1) is INELIGIBLE for automatic rebuild and cannot steal d1 back — its sites report `manual`; a form that ran `once` is excluded and reports `manual`; a bind-then-fail form retracts its STAGED pending bind (no half-installed bind survives) along with its namespace overlay and keeps the old binding; an acyclic diamond dependency rebuilds each shared descendant once in topological order (dedup, not flagged) while a true back-edge terminates with a cycle diagnostic; repeated controller updates on a computed literal converge via override inheritance
- [ ] Reactive propagation (design 5.6 revised; the unambiguous VAR-based case, not the authority's let/upd example): `var root 60`; `let raised + root 7`; a pattern form eagerly reading `raised` bound to a slot — `upd root 62` recomputes `raised` to 69, rebuilds the pattern form, stages the slot re-bind for the boundary, and marks the display batch (asserted end to end); the TRANSITIVE chain and a DIAMOND over a var propagate once per node in topological order; EQUALITY CUTOFF: an `upd` producing an unchanged derived value schedules no dependents, and unrelated branches are untouched (recompute counters asserted); a var read ONLY through Late captures triggers NO rebuild on `upd` (heard via the cell path instead — no-eager-edge asserted); a FAILED recomputation keeps the previous committed value and binding playing and reports with origin; CHANGING-EDGE case (design 5.6 rounds, abort-policy trace): Astra's conditional pair (`A = if switch then B else root`, `B = if switch then root else A+1`; coalesce `switch=true, root=20`) — A's deref of the scheduled B is a DIRTY-READ ABORT (asserted: no stale value read, no intermediate A commit), B commits 20, A re-runs after B; ends with A=20 AND B=20, final dependency sets {A -> switch,B; B -> switch,root}, and only final slot intents staged; a change making the edges genuinely cyclic terminates with the cycle diagnostic and previous values retained; FAILED-DIAMOND case (design 5.6 joins): `left = 1/root` fails at root=0 while `right = root+1` commits, and `total = left + right` is BLOCKED — previous committed total retained, NO slot intent staged for it, state `blocked-on: left`; repairing root recomputes left, unblocks and recomputes total, and stages its intent (recovery asserted end to end); controller-rate `upd` streams coalesce latest-wins per tick; ABORT/RETRY-FAILURE case (design 5.6 trace 1 — the prior claim that this pair commits a provisional A=1/3 is WITHDRAWN as unreachable under the abort gate): `A = if switch then 1/B else root`, `B = if switch then 0 else A+1`, `root = 2` — after `upd switch true`, assert A's DIRTY-READ ABORT on B (nothing commits, A never holds 1/3, no A intent is ever staged), B=0 commits, A's retry FAILS on 1/0, A reports Failed with its previous committed 2 standing (asserted that NO journal restore occurs, because none is needed); REACHABLE PROVISIONAL-ROLLBACK case (design 5.6 trace 2 — the journal's end-to-end regression, reachable because the invalidating owner is UNSCHEDULED at read time): definition order `X = if n > 0 then 1/(12 - Y) else 7`, `Z = if n > 0 then 5 else 3`, `Y = Z + 7` over `var n 0` — `upd n 1` schedules only X and Z; assert X reads the UNSCHEDULED Y (10) and PROVISIONALLY COMMITS 1/2 with a staged slot intent (the provisional value's existence asserted at the transaction layer), Z commits 5, Y recomputes to 12 and dirty-marks X, round-end validation flags X's STALE read, X's retry FAILS on 1/(12-12), the journal RESTORES X to the pre-pass 7 (value, version, owner asserted), ALL of X's staged intents and effects from any round are REMOVED (no bind, revocation, or cell update reaches the recording host), X reports Failed, and Z=5 and Y=12 commit as valid independent branches; CONDITIONAL-UNBLOCKING case (design 5.6 deref-time joins): `root = 1`, `switch = true`, `broken = 1/root`, `selected = if switch then broken else 7` — `upd root 0` fails broken and blocks selected AT ITS ACTUAL DEREF of broken; `upd switch false` re-runs selected, which never dereferences broken: selected commits 7, its edge set is REPLACED by {switch}, and its Blocked state CLEARS while broken stays Failed and unrepaired (final values, states, and dependency sets asserted); SWITCH-TOWARD case (design 5.6 recovery subscriptions): with broken already Failed and selected committed 7 over edges {switch}, `upd switch true` re-runs selected, which ABORTS at its newly discovered deref of broken and blocks with a RECOVERY SUBSCRIPTION on broken (asserted: broken is absent from selected's committed edge set, the subscription is present, and the last successful values/edges/read-versions survive the abort); repairing broken WAKES selected through the subscription and it commits — both directions, toward and away, asserted; STATUS-RECOVERY BYPASSES EQUALITY CUTOFF: `upd root` 1 -> 0 -> 1 makes broken fail then recover to its retained pre-failure value 1 — assert the Failed->Recomputed STATUS EVENT re-runs blocked dependents and recovery subscribers despite value equality (total and the blocked-toward selected retry, commit, and clear their badges); LATE-FAILURE case (design 5.6 trace 3 — failure-sensitive validation plus the ->Failed invalidation event; the producer fails with NO value or version change): definition order `X = if n > 0 then Y + 1 else 7`, `Z = if n > 0 then 0 else 1`, `Y = 1/Z` over `var n 0` (initially X=7, Z=1, Y=1) — `upd n 1` schedules X and Z; assert X legally reads the UNSCHEDULED Y=1 and PROVISIONALLY COMMITS 2 with a staged intent, Z commits 0 and dirty-marks Y, Y FAILS on 1/0 retaining value 1 and writing NO version, Y's ->Failed transition RE-SCHEDULES X (in-pass reader), X's retry ABORTS at its deref of the Failed Y, validation finds X invalid (consumed read's owner ended Failed), the journal restores X to the pre-pass 7, X's staged intents are removed, and final states are X Blocked-on Y with 7, Y Failed with retained 1, Z=0 committed (the provisional 2 asserted absent from every host-visible surface); NEWLY-DISCOVERED-SELECTOR case (design 5.6 attempt edge set): `broken` already Failed, `a = false`, `b = true`, `selected = if a then (if b then broken else 7) else 5` committed 5 over edges {a} — `upd a true` re-runs selected, which successfully reads a and b then ABORTS on broken; assert the attempt edge set is {a, b} and the subscription is {broken}; `upd b false` (a slot in NO committed edge set) WAKES selected through the attempt edge set, it re-runs, avoids broken, commits 7, and clears Blocked while broken stays unrepaired; also assert the attempt edge set is cleared on commit and replaced by the new attempt on a repeated abort; ORDINARY-FAILURE RECOVERY case (design 5.6 unsuccessful-evaluation metadata — the abort-only scoping is withdrawn): `var a false`, `var b 0`, `selected = if a then 1/b else 5` committed 5 over edges {a} — `upd a true` runs selected, which SUCCESSFULLY dereferences a and b (no Failed/Blocked/dirty owner anywhere) and then FAILS on division by zero; assert selected is Failed with its retained 5, its attempt edge set is {a, b}, and its blocking-read set/subscriptions are EMPTY; `upd b 1` (a slot in NO committed edge set and NO subscription) WAKES selected through the attempt edge set, it recomputes 1, commits, clears the Failed state and diagnostic, replaces its committed edges with {a, b}, and drops the failure metadata — automatically, with no manual re-evaluation and no change to a; also assert the failed attempt published no write or effect merely to retain its dependencies
- [ ] Failure cases (division by zero, `+ 1 "a"`, no matching clause, unbounded `for x 0..:`, recursion depth) unwind, report origin, and leave the session usable
- [ ] Redefinition + `upd` observable through `VarRef` demand semantics; `direct`-tier tweak writes observable without re-eval; `reeval`-tier writes trigger dependency-aware re-evaluation of the owning form
- [ ] Build + nextest pass (planned); wasm32 build passes (planned)

### TASK-006: Pattern engine, signals, clock
**Status**: NOT_STARTED | **Parallelizable**: Yes (with TASK-004/005; needs TASK-001 types, a VM handle trait for closure params)
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
- [ ] `[:bd :sd [:hh :hh]]` and the mini-notation equivalence table in design-music.md section 3 query to exact ratio positions (golden tests)
- [ ] `euclid 3 8`, `maybe`, `choose`, `degrade-by` are reproducible under a fixed seed and pure (same span -> same events)
- [ ] Sample-region goldens (design 10.1 revised): `chop 8` subdivides each event into 8 exact-ratio sub-events with contiguous regions; `striate 8` leaves timing unchanged and interleaves regions i mod 8 across events; `slice 8 [0 2 4 7]` yields begin/end at i/8 for the indexed slices and `slice [0 0.31 0.5 0.8] [2 0]` the manual spans (last to 1.0); an out-of-range index is an EVENT-LOCAL fault (siblings survive), unsorted or out-of-[0,1] manual points are diagnostics (literal = checker, dynamic = query fault); `splice`/`loop-at 2`/`fit` mark commit-time speed resolution, asserted with a mock bank duration and tempo (speed = slice/sample seconds over WHOLE-SPAN/cycle seconds); COVER EQUIVALENCE (design 10.1 layer 1), against a frozen read snapshot: for `chop 2`, `chop 8`, `striate 8`, `splice`, and `fit`, a full-cycle query versus adjacent subqueries ([0,1/2) + [1/2,1)) versus OVERLAPPING subqueries ([0,3/4) + [1/4,1)) versus a REPEATED window is cover-equivalent — the union of results DEDUPLICATED BY `occ` equals the whole-span result occurrence for occurrence (onsets, regions, rate-fit speeds); the occurrence shared by an overlap is returned by BOTH windows with the SAME OccKey (asserted — pure queries report it twice; layer-2 dedup is TASK-007's, not the query's); a boundary-crossing source event's continuation is emitted with `part.begin > whole.begin` and carries its onset's region controls; a two-branch stack of IDENTICAL simultaneous notes yields two DISTINCT OccKeys (branch ordinals); Astra's chop-2 counterexample asserted exactly (adjacent partition: two onsets at 0 and 1/2, never four, region sequence not restarting); striate ordinals do not restart across query windows (query-widening rank asserted); a `whole = None` source event under any region operator records an event-local fault instead of partition-dependent output
- [ ] `PParam::Late` re-reads a var slot per query; `PParam::Fn` calls back into the evaluator in Query effect mode
- [ ] Mixed valid/failing-event query keeps sibling events, records one origin-carrying fault per failure, and never yields an all-or-nothing error
- [ ] A tweaked `PParam` probability changes behavior for ALL uncommitted events via staging invalidation and re-query (design 11.3) — in BOTH named directions: `maybe` 0 -> 1 RESTORES previously absent events, `degrade-by` 0 -> 1 REMOVES events
- [ ] `PatNode::MidiNotes` yields no events under pure query and dry run; the bind-time INPUT-LANE walk classifies the path: `midi-notes > degrade-by 1` drops EVERY arriving note (filtering applied, never bypassed), control/scale nodes decorate per note in tree order, a `stack` of an input lane with queried branches realizes both independently, and a structural/time operator over the lane (`fast`, `rev`, `every`, `chop`, …) is a BIND-TIME diagnostic with the old binding kept; `Sig::Cc` and `Sig::Analyzer` read their cells at query/frame time; the `:midi`-slaved clock anchor keeps logical positions exact under jittered pulse timestamps (unit test with synthetic pulses)
- [ ] Visual goldens: `osc 20 > rotate 0.5 > out o0` compiles to stable shader source; `text "hello" > out o1` produces its `TextAsset`; uniforms resolve per frame from the evaluator-owned `UniformPlan`; `ShaderDesc` is structurally render-safe (serializes as plain data, no evaluator-owned payload); a failing chain leaves the previous program in place
- [ ] Build + nextest pass (planned); wasm32 build passes (planned)

### TASK-007: Scheduler, slot table, capability hosts, dry run
**Status**: NOT_STARTED | **Parallelizable**: No (integration point)
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
- [ ] Deterministic scheduler tests (mock clock, recording hosts): exact ratio positions; queries split at every cycle boundary; a SUFFICIENT-lead re-bind swaps only at the boundary with no stale-pattern event past it (the unconditional form of this claim is withdrawn — the three-case bullet governs late arrivals); REGION PARTITION INVARIANCE THROUGH STAGING (design 10.1/11.3): a `chop`ped and a `striate`d pattern staged incrementally across several ticks and cycle-boundary splits — and re-staged after a control write invalidates uncommitted spans — commit the same logical notes (onsets, regions, speeds) as a single whole-cycle staging, asserted against the full-cycle dry run; OCCURRENCE-MERGE EMISSION UNIQUENESS AND ONSET PRESERVATION (design 11.3 layer 2 revised), asserted as EMITTED MULTIPLICITY at the recording host — never set equality: staging through OVERLAPPING windows [0,3/4) then [1/4,1) commits onset 1/2 EXACTLY ONCE, including the committed-ledger variant where the first window's work partially commits before the second query arrives; ONSET PRESERVATION UNDER CLIPPING (Astra's future-span case): source whole [1,2) under `chop 2`, queries [1,7/4) and [5/4,2) BOTH staged before ANY commit — onsets 1 and 3/2 each emit EXACTLY ONCE, asserted in BOTH query orders (the second window's continuation fragment of child 0 merges into the record's covered extent and does not erase its onset); BOUNDARY-CROSSING FRAGMENTS across disjoint adjacent spans, onset fragment first AND continuation fragment first, merge into one record with one emission; PARTIAL INVALIDATION: invalidating only [5/4,2) discards coverage inside that span, preserves child 0's onset at 1 (whole.begin outside the span — never dropped, only re-extended), and drops an uncommitted record only when its whole.begin lies in the invalidated span and its key is absent from the re-query; a REPEATED window commits nothing twice; a two-branch stack of IDENTICAL simultaneous notes commits BOTH voices (distinct OccKeys — the merge never erases real notes); SEMANTIC PAYLOAD REPLACEMENT (design 11.3 revised — Astra's slice-index case): an uncommitted occurrence with whole [1,2) under `slice` with two equal regions and late index 0 (region [0,1/2)) — `upd` the index to 1 and invalidate/re-query a span containing the onset: assert the SAME OccKey and generation, EXACTLY ONE emission at the recording host, its region is [1/2,1), and NO old-region [0,1/2) output ever appears; and INVALIDATING ONLY A CONTINUATION span does not rewrite the unaffected onset's payload (the onset's region is unchanged while the in-span coverage rebuilds)
- [ ] Deadline-aware rebind, THREE cases with explicit cycle duration and transport delay: SUFFICIENT lead (`B - now >= commit_lead + L_ctl`) yields zero old-pattern events past the boundary and full-lead new boundary events; INSUFFICIENT lead commits new boundary events with reduced lead (flagged) rather than dropping them; control-delivery failure is DENSITY-BOUNDED: all old-gen events starting within the delivery delay may begin and each is cut on control receipt (three simultaneous boundary events = three artifacts, each within `D` + gate), with the missed deadline reported
- [ ] stop/hush/tempo/set-tweak (control value and structural probability: `maybe` 0→1 restores, `degrade-by` 0→1 removes) behave as bounded across boundaries, `once`/`at`, and audio/MIDI/OSC sinks with slot+gen identity; the immediate-class MONOTONE MERGE preserves the strongest release: HUSH-THEN-STOP keeps Panic (never downgraded to Natural), HUSH-THEN-TEMPO keeps Panic while the tempo gen bump rides the merged entry, HUSH-THEN-REBIND keeps both entries in order; re-send after a duplicate delivery is idempotent under the merge; a MIDI stale-started note gets an immediate note-off; OSC revocation applies only to the untransmitted queue
- [ ] A LOST FUTURE-boundary control with a gen-carrying event batch arriving first triggers only the DEFAULT piggyback policy (drop stale-gen at dequeue + short-gate stale-started voices — over-eager before the boundary is accepted and asserted), full boundary/release semantics land when re-send delivers; repeated loss exercises ack/re-send and the transport-diagnostic threshold
- [ ] Cell semantics per tier: NATIVE `upd` on a `Ctl::Cell` control is heard at the next STARTED event even when already committed (atomic store/load); BROWSER-model test with a DELAYED CellUpdate batch: an event starting inside the delivery hop plays the previous value and the next voice start after batch application plays the new one — the delivery-dependent guarantee, asserted exactly, with the mock transport modeling the isolated mirror (no shared-memory shortcut); FIRST-USE-BEFORE-INIT: an event referencing a new cell incarnation before `CellInitAck` commits with the `Const` downgrade and sounds the correct value, and the cell path resumes after the ack; INIT REPLAY AFTER UPDATE: a retransmitted `CellInit(E, v0)` arriving after a batch set v1 is acknowledge-only and v1 stays (init-once per epoch); REUSE INITIALIZATION: a retired-then-reused id passes Vacant -> Live with its new epoch while old-epoch updates stay inert; PRE-ACK CONST + POST-ACK BATCH: a `Const`-downgraded event whose start follows the ack and a newer batch still plays its committed constant, asserted as the documented Const-fallback exception with the next commit back on the cell path; STALLED-CONSUMER BURST: writes while the mirror applies nothing never exceed one in-flight + one pending batch, coalesce latest-wins, and on resume the mirror equals the latest values; STALE-UPDATE-AFTER-REUSE: an update carrying a retired epoch is dropped by the mirror; RECONNECT replays the full snapshot before any new voice start reads the mirror; MIDI/OSC value baking at transmission asserted as the documented sink-specific deviation
- [ ] MIDI clock slave: synthetic jittered 24-ppq pulses drive a smoothed anchor with exact logical positions; Start/Stop/Continue freeze and resume the scheduler; clock loss freewheels with a diagnostic; clock master emits Clock/Start/Stop with commit discipline; `midi-notes` input sounds a voice within one drain tick + commit path and never appears in dry runs
- [ ] Note lifetime (design 11.7 revised): a held note releases on its NoteOff via `VoiceRelease` tag matching; repeated same-pitch NoteOns release EARLIEST-open-first; a held OLD-GENERATION voice surviving a rebind (`None`) is ACTUALLY TERMINATED by its later NoteOff via its tag (no stuck note), while the new binding's voices stay untouched (tags differ — both halves asserted); `stop`'s Natural puts every open input voice into its release stage and closes its session records (no indefinite sustain); a FILTERED NoteOn consumes its own NoteOff without releasing a different same-pitch held voice; NoteOn/NoteOff share the FIFO priority channel and a forced release-before-start in the mock transport hits the tombstone (the late NoteOn is dropped, no unreleasable voice); tag-map exhaustion steals the oldest open voice with a diagnostic, no allocation; hush Panic-gates open input voices
- [ ] Live captured `print` reaches the console only AT its event's commit point: a control write restaging the still-uncommitted span replaces the held output (no duplicate, none for removed work); a write after commit leaves emitted output untouched
- [ ] Dry-run success AND failure leave namespace, slots, staging, pending queues, and host outputs unchanged (state-snapshot assertion); failure keeps the old binding playing; captured dry-run `print` output appears in the report, never on the console
- [ ] `hush`/`stop` across audio, visual, midi, osc slots — per release class, never a blanket silence assertion: NEW events cease within control-message latency (not lookahead latency); hush's Panic gates sounding voices within it; stop's Natural lets sounding voices end naturally (open input voices enter release, 11.7); already-transmitted OSC is irrevocable and asserted so
- [ ] Mixed valid/failing events in one span: valid events play, each fault carries slot + beat, diagnostics clear after a clean cycle
- [ ] Build + nextest pass (planned)

### TASK-008: DSP graph, native audio host, wasm/AudioWorklet host
**Status**: NOT_STARTED | **Parallelizable**: Yes (pure-DSP voice/graph code needs only TASK-001 and can proceed alongside TASK-007; host integration needs TASK-007's traits and event/gen contract)
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
(cpal callback, timer tick, midir, session socket); `WasmHost`
(wasm-bindgen exports, worklet JS glue under `editor/worklet/`,
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
- [ ] Headless render test: schedule events into the ring, run the callback over N buffers, assert sample-accurate voice starts and no allocation counter hits (debug instrumentation)
- [ ] Each synthesis TEMPLATE (`sampler`, `analog`, `fm`, `pd`, `additive`, `wavetable`, `granular`) compiles from prelude source to an `InstDef` and renders headlessly; template parameters arrive as ordinary pattern controls
- [ ] Sample-region handoff (values only, design 10.1 revised): a sliced event renders exactly its begin/end sample region headlessly; `splice` renders the slice rate-fitted to its step and `loop-at 2` stretches the sample across two cycles with loop enabled; `fit` matches the event length; regions at the sample bounds render without overrun (bounds asserted); all via existing begin/end/speed/loop `Ctl` values — no new audio-thread machinery (asserted structurally); regions, onsets, and rate-fit durations originating from PARTITIONED staging queries (multi-tick lookahead, boundary splits) render identically to the full-cycle render — partition invariance holds through to the host boundary; OVERLAPPED staging ([0,3/4) + [1/4,1)) and a repeated window render each voice start EXACTLY ONCE while the identical two-branch stack renders BOTH voices (emitted multiplicity asserted at the audio ring); the future-span onset-preservation case (whole [1,2) under `chop 2`, both windows staged pre-commit, both query orders) renders BOTH voices at 1 and 3/2 exactly once each — no onset lost to a continuation fragment; the slice-index payload-refresh case (index 0 -> 1 with invalidation covering the onset) renders exactly one voice playing the NEW region [1/2,1) at the audio renderer with no old-region output
- [ ] Bus chains: `bus :drums:` compiles, a slot routes into it via the `bus` control, `master` receives every bus, a bus-chain swap follows the generation+refcount lifecycle without callback allocation; the SuperDirt-style `room` control maps onto the corresponding bus unit parameter
- [ ] Analyzers write their cells without altering the rendered signal (bit-compare with and without an analyzer in the chain); granular renders from a preallocated grain pool with zero callback allocation, and a density above the tier cap is a diagnostic, not a dropout
- [ ] Granular behavior (design 12.6 revised), deterministic under a fixed seed: onset COUNT and TIMING over N blocks match the phase-accumulator model for fixed density/size; a LIVE-bus instance never reads a sample overwritten during a grain's life across capture-buffer WRAP (guard asserted); FREEZE retains the frozen audio while live input keeps arriving (frozen-grain output invariant) and unfreeze resumes reading live content; ACTIVE-GRAIN UNFREEZE: a long frozen grain reading the oldest region is re-checked at the unfreeze boundary against the live rule for its remaining span and SHORT-GATED before capture overwrites it (no sample it plays is rewritten during its life — the invariant holds across the transition), while grains whose spans stay valid continue; PARTIAL-FILL FREEZE: the valid window is always intersected with the captured extent — freezing a partially filled buffer exposes only recorded audio and freezing immediately after install spawns no grains (skip-counted, never uncaptured storage); `size` above `max_grain_size` and position depth beyond `max_capture_seconds` are diagnostics; pool exhaustion skips grains with a count and no allocation
- [ ] `CapabilitySet` gating: an IR beyond `max_ir_seconds` and offline render on the browser tier produce "not available on this host" diagnostics with origin
- [ ] Cell transport in the dev harness (real worklet): a batch applied between quanta changes the value read by the next voice start; a delayed batch leaves the earlier value audible for events starting inside the hop; a FIRST event before `CellInitAck` sounds its `Const`-downgraded value (never an uninitialized mirror read); a replayed `CellInit` after a newer batch leaves the newer value in place (init-once), and a reused id initializes through Vacant -> Live in the real worklet; a stalled-worklet burst is bounded to one in-flight + one pending batch and converges to the latest values on resume; an update with a retired epoch after id reuse is inert; reconnect snapshot completes before new voice starts read the mirror; `VoiceRelease` releases exactly its tagged voice even after a slot-gen bump, and a forced release-before-start hits the tombstone
- [ ] `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` passes (planned)
- [ ] `examples/beep.rs` plays a sample end to end on the native host (manual check, recorded in progress log); the audible REPL gate lives in TASK-009
- [ ] Browser lifecycle checks run in this task's OWN dev harness (no editor or downstream-task dependency): sample-bank load and graph replacement during active playback without dropout; arena-exhaustion, oversized-graph, and deferred-queue-overflow diagnostics; unload-while-playing holds storage until queued events and voices release it; an install burst copies at MOST `INSTALL_BYTES_PER_QUANTUM` on the rendering thread PER `process()` call — measured as total bytes copied per quantum across the whole burst, not merely unacked-slice count — with acks withheld until the deferred copy runs; the headless `process()` test alone is not accepted as proof
- [ ] nextest pass (planned)

### TASK-009: Session, REPL, LSP
**Status**: NOT_STARTED | **Parallelizable**: No
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
tweak-site tiers incl. `manual`), WebSocket server (host-native,
localhost + token), `vactrol repl` (console registers `_1..`,
transcript semantics), `vactrol lsp` behind `lsp` feature (tower-lsp:
diagnostics, hover from `TypedInfo`, completion from prelude +
manifest, formatting via reader trivia; attaches to a live session
socket for runtime diagnostics when present), CLI wiring in
`src/main.rs` (`repl`, `run <file.vact>`, `lsp`, `serve`,
`get <package-path>`) — to be reflected in design-docs/specs/command.md
when implementation starts.
**Design ref**: sections 5.7, 14, 17.
**Completion criteria**:
- [ ] Protocol round-trip tests: eval -> diagnostics + tweak sites; set-tweak/set-var state change asserted via recording host; playing telemetry carries `SrcRef`s with `doc_revision`; REACTIVE publication: an `upd` through a derived binding emits exactly ONE `bindings` batch after ALL rounds complete (changed names, display values, refreshed sites, PER-FORM STATES), no message mid-pass or mid-round; the CHANGING-EDGE pass publishes only final-round values (the round-1 stale value never appears in any batch); the FAILED-DIAMOND pass publishes right's new value, left's failure diagnostic, and total's `blocked-on: left` with its previous value, and the recovery pass publishes the unblocked recomputation; a subscriber never observes a partially updated set; PROVISIONAL-ROLLBACK publication (TASK-005's REACHABLE X/Z/Y trace, end to end): the pass publishes X as Failed with its RESTORED pre-pass display value 7 and Z/Y with their new values — the provisional 1/2 appears in NO batch, and no staged bind, revocation, or cell update of X reaches the recording host or any subscriber; ABORT/RETRY publication (TASK-005's A/B trace): A publishes Failed with its previous value standing and B publishes 0, with no intermediate A value in any batch; CONDITIONAL-UNBLOCKING publication (switch-away): the pass publishes selected's new value and CLEARS its `blocked-on` badge while broken remains failed with its diagnostic; SWITCH-TOWARD and STATUS-RECOVERY publication: blocking toward broken publishes selected's `blocked-on` badge, and the equal-value repair (`upd root` 1 -> 0 -> 1) publishes a recovery batch clearing the badges of total and the blocked-toward selected even though the repaired value is unchanged; LATE-FAILURE publication (TASK-005's trace-3 case): the pass publishes X as `blocked-on: Y` with its RESTORED pre-pass 7, Y as Failed with its retained value, and Z's new value — the late-failure-derived provisional 2 appears in NO batch and no staged effect of X reaches the recording host or any subscriber; NEWLY-DISCOVERED-SELECTOR publication: `upd b false` publishes selected's new value 7 and CLEARS its `blocked-on` badge while broken remains failed with its diagnostic (unblocking without repairing broken, asserted end to end); ORDINARY-FAILURE RECOVERY publication (TASK-005's a/b division case): the failing pass publishes selected as Failed with its retained 5 and the diagnostic, and the `upd b 1` pass publishes the repaired value 1 with the diagnostic CLEARED — with no manual re-evaluation between the two batches
- [ ] Packages: a fixture package (local-directory store) imports, resolves via MVS against competing requirements, pins version + sha256 in `vactrol.lock`, exposes qualified and `open` names with the 5.7 lookup and collision warnings, registers its sample assets, and plays through the REPL; a FRESH-SESSION single file containing `import` followed by `pads.warm` (and its `as pd` variant) loads end to end; an LSP-ONLY analysis of an unopened document builds the same `AliasEnv` from `prescan_imports` without executing anything (unfetched package = `any` + "package not fetched" diagnostic); hash mismatch, unresolvable version, and a package's own compile error each yield a load diagnostic while the session keeps playing; re-import replaces the `PkgNs`; the proxy-shaped store is exercised against a local HTTP fixture
- [ ] Package archive safety (design 5.7 revised): CORRECTLY-HASHED malicious fixtures — a parent-traversal entry, an absolute path, a symlink escaping the package root (also listed as a manifest asset), duplicate (incl. case-fold) entries, and an over-limit archive — are each REJECTED with an integrity diagnostic naming the entry, with NO cache mutation; a native local-directory tree and a proxy zip of the SAME sources produce the SAME lock digest (portability asserted); SERIALIZATION INJECTIVITY (Astra's two-tree counterexample as a fixture): a tree containing a file whose name embeds a newline plus a hash-lookalike suffix versus the two-file tree it imitates — the malicious tree is REJECTED by control-character path validation, and independently the length-prefixed canonical bytes (and digests) of the two trees DIFFER; an interrupted extraction leaves no partial cache entry (staging + atomic publication asserted)
- [ ] Directives: the spec's own examples parse into the `DirectiveTable` with correct attachment (same-line trailing, consecutive-line block to the nearest preceding statement/block at no-deeper indentation, whole-`inst` binding at column 0, Addressed `hats.hpf` and file-level `#@ midi ch: 1`); labels resolve (explicit and implicit) and label-identified bindings survive a line move that would re-key a positional site; unknown call-site/param/label directives yield editor-validation diagnostics and never affect evaluation; a MIDI-learn writes the CC number back into the directive text via the validated edit path; both `BindingPersistence` impls round-trip the same binding set
- [ ] Directive vocabulary (13.5 PROPOSED grammar): `cc: 74 _ 30` skips the second declared parameter; fewer `cc:` numbers than parameters leaves the remainder panel-only; a directive `ch:` overrides the file-level `#@ midi ch:` default; a `duplicate-label` collision (explicit vs explicit AND explicit vs implicit) is diagnosed and its addressed references rejected, with the affected sites falling back to positional provenance; ADDRESSED SELECTOR RESOLUTION (design 13.5): `analog.cutoff` selects the inst's declared PARAMETER, `hats.hpf cc: 30` selects the hpf CALL SITE on the hats line with 30 mapping onto its first declared parameter (the authority's example, asserted exactly), a selector matching both a parameter and a call-site name yields `ambiguous-selector`, a repeated same-named call site is `ambiguous-selector` bare and selectable via the ordinal (`hats.lpf.2`, and positional `#@ lpf.2`); a reserved `range:` key yields `reserved-key`; each vocabulary fixture carries its authority-question disposition in the manifest until ratified
- [ ] Binding identity (design 13.5 `BindingKey`): simultaneous INDEPENDENT mappings and overlays on `hats.lpf` and `hats.hpf` (same `cutoff` keyword) never cross-talk or overwrite each other, and likewise for repeated `hats.lpf.1` / `hats.lpf.2`; both survive save and read-back in BOTH persistence modes; moving the labeled line keeps every binding attached (label component position-free); reordering the two same-named `lpf` sites MIGRATES each binding with its site through change-set span mapping, and an edit that breaks the mapping marks the binding STALE for re-confirmation instead of guessing (positional-fallback recovery asserted)
- [ ] Stale `set-tweak` (old `form_gen`) is rejected; a debounce-race write stamped with an `edit_epoch` newer than the session's last `doc-changed` for its form is rejected; an edit WITHOUT re-evaluation (`doc-changed`) invalidates intersecting sites — a delayed write for them is rejected while unrelated sites stay valid; an insertion before a site followed by an edit to the shifted site invalidates the correct site through composed change-set coordinate mapping in the runtime; `set-var` with a superseded `defining_form_gen` is rejected; `reeval` sites rebuild their owning form at the boundary with override inheritance; `manual` sites update the slot without any replay; `direct` sites (including a probabilistic parameter) update per the 11.3 per-tier cell contract — within the commit horizon natively, after batch application on the browser model (asserted per tier, never as one unconditional claim)
- [ ] Audible integration gate: a pattern bound from the REPL sounds on the native host (TASK-008), recorded in the progress log
- [ ] REPL: failed expression does not write `_1`; session survives all failure classes
- [ ] LSP smoke test over stdio: publishDiagnostics + hover on spec examples
- [ ] Build (+ `--features lsp`) + nextest pass (planned)

### TASK-010: Editor (browser + Tauri), visual feedback, controller binding
**Status**: NOT_STARTED | **Parallelizable**: Partially (UI shell can start once TASK-009 message shapes are frozen)
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
**Completion criteria**:
- [ ] Browser: eval a pattern, hear audio via worklet, see the sounding step highlighted within one lookahead window (manual + automated protocol test with mock clock)
- [ ] Dragging a literal / MIDI CC on a `direct` site changes sound with no re-eval, per the 11.3 per-tier cell contract (browser: after batch application — the delivery-dependent guarantee, not an unconditional commit-horizon claim); on a `reeval` site it rebuilds the owning form at the boundary (or reports `manual`); in OVERLAY mode no source change occurs until "commit"; in SOURCE-EDIT mode the text updates with validated live redefinition
- [ ] The slider panel lists every Decided site kind — a pattern literal, a top-level `let` number, and an `inst` parameter default — and an inst-default slider is heard, without re-evaluation, at the next voice using the default under the per-tier `Ctl::Cell` contract (browser: after its update batch applies); MIDI learn maps a CC to a slider; analyzer meters render from the analysis cells; the package UI imports the fixture package and surfaces its diagnostics
- [ ] Parameter editors: a `peq` call site opens the EQ band editor with the live spectrum behind it; an `env-adsr` site opens the envelope editor; a `euclid` site opens the ring with draggable hits/steps/rotation; each dragged handle changes the same numeric site a slider would (asserted via the recording host) and is MIDI-learnable; the step grid and piano roll render sounding events and expose NO editing affordance for sequences (structural assertion: no write-back path)
- [ ] Directive round trip in the editor: opening a file with the spec's example directives shows the declared control panel; MIDI-learning a mapped parameter updates the `#@` comment text; a dangling label reference renders as an editor diagnostic; switching `BindingPersistence` to ExternalFile leaves the source untouched while preserving the panel
- [ ] Saving is MODE-SCOPED (design 13/13.5 — two tests, no unconditional claim): in DIRECTIVE mode the saved `.vact` retains its `#@` comments including learned CC mappings, and contains no binding data outside comments (no language construct changes); in EXTERNALFILE mode the saved `.vact` contains no binding artifacts at all; in BOTH modes overlay values are absent from the source until an explicit commit
- [ ] Multi-site binding round trip in the editor: learned CCs on `hats.lpf` and `hats.hpf` and on `hats.lpf.1`/`hats.lpf.2` operate their own sites simultaneously (recording-host assertion of independence), persist and read back per `BindingKey` in both modes, follow a line move of the labeled statement, migrate across a same-named site reorder, and surface the STALE re-confirmation state when the mapping is broken
- [ ] Reactive displays: after `upd` on a var feeding a derived binding, the panel and value displays repaint from the single `bindings` batch — the derived value, its sliders, and the affected slot indicator update together, unrelated displays do not repaint (batch-driven, no per-form flicker); a failed recompute leaves the previous coherent values displayed with the form's diagnostic, and a BLOCKED form shows its previous value with the `blocked-on` badge until the recovery batch clears it; after a FAILED RETRY (TASK-005's REACHABLE X/Z/Y provisional-rollback trace) the panel shows X's restored pre-pass 7 — the provisional 1/2 is never rendered — a conditional-unblocking pass clears the badge and shows the new value while the failed upstream form keeps its diagnostic, and the equal-value STATUS-RECOVERY batch clears the badges of blocked dependents (including one blocked TOWARD the repaired form) even though the repaired display value is unchanged; a LATE FAILURE (trace 3) surfaces the reader's `blocked-on` badge with its restored pre-pass value even though the failed producer changed no value, and changing the attempt-edge selector clears it without repairing the failed form; an ORDINARY runtime failure (TASK-005's a/b division case) shows the retained value with its diagnostic and then, after `upd b 1` alone, the repaired value with the diagnostic cleared — no manual re-evaluation affordance is needed for recovery
- [ ] Sampler waveform editor (design 13.5 revised, per the Decided sampler item): start/end/loop handles drag the `begin`/`end` literal sites; `slice n` and `chop`/`striate` counts render as grid overlays; MANUAL slice markers are draggable and each drag writes its point literal through the standard source-edit/overlay path (recording-host assertion); clicking a slice writes the clicked index INTO the currently selected, existing index literal via validated write-back and NEVER adds/removes/reorders steps (structural assertion: no sequence-editing affordance; no selection = no-op with hint); the bank index `n` opens the sample browser with per-entry waveform previews
- [ ] Edit-reconciliation tests: insertion/deletion/reordering above and inside a playing form keeps highlights and bindings attached or drops them cleanly; duplicate literals bind independently; an edit without re-evaluation invalidates its sites (`doc-changed`) and a delayed controller message is rejected as stale; write-back declines when the mapped text no longer matches; a stored list evaluated under an earlier document revision highlights against that revision through the event's own `SrcRef`
- [ ] Visual pane: `osc 20 > rotate 0.5 > out o0` renders and activates at the cycle boundary; `text "hello"` renders via its rasterized `TextAsset`; a broken chain keeps the previous frame rendering and shows the diagnostic (TASK-008's lifecycle checks already passed in its own dev harness; this task may re-run them in the full editor)
- [ ] Tauri build runs the identical frontend; `npm run build` and frontend tests pass (planned)

---

## Module Status

| Module | File Path | Status | Tests | Task |
|--------|-----------|--------|-------|------|
| Core values | `src/value/` | COMPLETED | 57 unit | TASK-001 |
| Reader | `src/reader/` | COMPLETED | 33 unit + 8 spec_fixtures (shared) | TASK-002 |
| Expander | `src/expand/` | COMPLETED | 55 unit + 8 spec_fixtures (shared) | TASK-003 |
| Checker | `src/types/` | NOT_STARTED | - | TASK-004 |
| Namespace/compiler/VM | `src/ns/ src/compile/ src/vm/` | NOT_STARTED | - | TASK-005 |
| Pattern engine + visuals + clock | `src/pattern/ src/tex/ src/clock/` | NOT_STARTED | - | TASK-006 |
| Scheduler + hosts | `src/sched/ src/host/` | NOT_STARTED | - | TASK-007 |
| DSP + audio hosts | `src/dsp/ src/host/{native,wasm}/` | NOT_STARTED | - | TASK-008 |
| Session/REPL/LSP | `src/session/ src/lsp/ src/main.rs` | NOT_STARTED | - | TASK-009 |
| Editor | `editor/` | NOT_STARTED | - | TASK-010 |

## Dependencies

| Task | Depends On | Parallelizable | Status |
|------|------------|----------------|--------|
| TASK-001 | — | Yes | COMPLETED |
| TASK-002 | TASK-001 | Yes (with 001 tail) | COMPLETED |
| TASK-003 | TASK-002 | No | COMPLETED |
| TASK-004 | TASK-003 | Yes (with 005) | NOT_STARTED |
| TASK-005 | TASK-003; TASK-004 `types/masks.rs` | Yes (with the rest of 004) | NOT_STARTED |
| TASK-006 | TASK-001 (int. 005) | Yes (with 004/005) | NOT_STARTED |
| TASK-007 | TASK-005, TASK-006 | No | NOT_STARTED |
| TASK-008 | TASK-001, TASK-007 | Yes (pure DSP alongside 007) | NOT_STARTED |
| TASK-009 | TASK-004, 005, 007; 008 for the audible gate | No | NOT_STARTED |
| TASK-010 | TASK-008, TASK-009 | Partially | NOT_STARTED |

Crate dependencies (added per task, not up front): serde + serde_json
(TASK-009), tower-lsp + tokio behind `lsp` (TASK-009), cpal + midir +
tungstenite behind `host-native` (TASK-008), wasm-bindgen + js-sys +
web-sys behind `host-wasm` (TASK-008). Core modules stay dependency-free.

## Verification

The Build, Tests, Lint and Wasm rows (plus the default-feature wasm32
build) were executed for TASK-001..003 in session 175 (see the Progress
Log); the LSP and Editor rows remain planned for their later tasks.
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

- [ ] All ten tasks complete with their per-task criteria checked
- [ ] The spec fixture manifest is complete; all `positive` and `diagnostic` fixtures pass; every `authority-question` fixture has a recorded disposition
- [ ] Visual chain golden and render-recovery tests pass (design 9)
- [ ] REPL performs an audible pattern session natively; browser build performs the same via Wasm + AudioWorklet
- [ ] Editor shows inline diagnostics, playing-step highlighting, and controller binding with no language construct involved
- [ ] cargo build, clippy, nextest, wasm32 build all pass (commands above)
- [ ] impl-plans/README.md index and design-docs/specs/command.md CLI table updated (deferred from planning phase, where only the two planning documents could be edited)

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

## Related Plans

- **Previous**: none (first plan of the project)
- **Next**: Frozen-mode codegen and Swift/UniFFI shell (future plans, out of scope here); TASK-010 may split into `vactrol-editor.md` when work starts if it approaches size limits
- **Depends On**: none
- **Front-end sub-plans (TASK-001..003)**: vactrol-frontend-value.md (FE-VALUE), vactrol-frontend-reader.md (FE-READER), vactrol-frontend-expander.md (FE-EXPAND), vactrol-frontend-finalize.md (FE-FINAL), all in impl-plans/active/
