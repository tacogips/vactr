# Vactrol Back End: Contracts and Skeleton (BE-CONTRACTS) Implementation Plan

**planId**: BE-CONTRACTS (issue #3, wave 1; the contracts every TASK-007/TASK-008 wave builds on)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 12.8 (12.8.2 skeleton rule and enum shapes, 12.8.3, 12.8.5, 12.8.7, 12.8.9, 12.8.10 Cargo, 12.8.12 codes), 11.3, 11.4, 11.5, 11.7, 12.1, 12.5, 12.7, 6.5.7
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
**dependsOn**: none
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json

---

## Intent and Context

Issue #3 builds the runtime back end (vactrol-core.md TASK-007 scheduler/hosts and TASK-008 DSP/hosts) in eight waves.
This first wave adds every type, trait, code and file that later waves share, so that SCHED, DSP and INST can run at the
same time without writing a shared file. It adds the Cargo dependencies (cpal and midir, gated to non-wasm32 targets),
creates every `mod.rs` declaration and a stub for every file a later wave owns (12.8.2 skeleton rule), declares the new
`Value`/`Sound`/`StagedEffect` variants with their minimal arms, and adds the closed code list of 12.8.12. The current
tree is TASK-001..006 complete (commit 3ab64b6); `src/sched/{mod,slots}.rs` and `src/dsp/{mod,graph}.rs` are id-only
stubs; `NoopHost` lives in `src/ns/load.rs`.

## Non-Goals

- No scheduler, DSP, instrument, MIDI, native or wasm behavior. Stub files contain only a `//!` doc comment naming their
  owner wave (the `examples/beep.rs` stub is `fn main() {}`). Stub test files are empty modules.
- No `Ty::UGen`, natives, templates or checker changes (BE-INST). No `Runtime` (BE-SCHED).
- No session socket, no tungstenite, no wasm-bindgen/js-sys/web-sys (12.8.1, 12.8.10).
- No crate-wide `cargo fmt`; no edits to `impl-plans/README.md` or `vactrol-core.md` (BE-FINAL).

## writePaths (exclusive)

- `Cargo.toml`, `Cargo.lock`, `src/lib.rs`, `examples/beep.rs` (stub)
- `src/host/mod.rs`, `caps.rs`, `wire.rs`, `noop.rs`, `testing.rs`, `native/mod.rs` (stub), `wasm/mod.rs` (stub),
  `src/host/tests/mod.rs`, `src/host/tests/contracts.rs`, `src/host/tests/e2e.rs` (stub)
- `src/sched/mod.rs`; stubs `src/sched/{runtime,staging,ledger,commit,control,cells,dryrun,telemetry,oneshot,midi_in,midi_clock}.rs`;
  `src/sched/tests/mod.rs`, stubs `src/sched/tests/{sched,midi}.rs`
- `src/dsp/mod.rs`, `graph.rs`, `controls.rs`, `cells.rs`, `release.rs`, `caps.rs`, `alloc_probe.rs`; stubs
  `src/dsp/{engine,voice,ring,fft,granular,bus,arena,meta,build}.rs`, `src/dsp/ugen/mod.rs`, `src/dsp/effects/mod.rs`;
  `src/dsp/tests/mod.rs`, `src/dsp/tests/contracts.rs`, stub `src/dsp/tests/dsp.rs`
- `src/ns/mod.rs` (`pub mod insts;`), stub `src/ns/insts.rs`, `src/ns/load.rs` (NoopHost re-export), `src/ns/stage.rs`
  (new variant), `src/ns/tests/mod.rs` + stub `src/ns/tests/inst.rs`
- `src/vm/natives/mod.rs` (`pub mod dsp;`), stub `src/vm/natives/dsp.rs`, `src/vm/tests/mod.rs` + stub `src/vm/tests/inst.rs`
- `src/types/tests/mod.rs` + stub `src/types/tests/inst.rs`, `src/compile/tests/mod.rs` + stub `src/compile/tests/inst.rs`
- `src/types/diag.rs`, `src/vm/fail.rs` (codes and their count assertions)
- `src/pattern/query.rs`, `src/pattern/step.rs` (the two `Event` provenance fields, item 14)
- `src/value/value.rs`, `src/value/print.rs`, `src/value/eq.rs`, `src/value/tests/mod.rs`, `src/vm/call.rs`,
  `src/vm/tests/integrate_sound.rs` (minimal arms)
- Conditional (edit ONLY if the compiler reports a non-exhaustive match after the new variants; record each in the
  progress log): `src/ns/evaluator.rs`, `src/ns/tests/evaluator.rs`, `src/ns/tests/stage.rs`, `src/vm/natives/music.rs`,
  `src/vm/natives/tex.rs`, `src/vm/tests/integrate_tex.rs`
- `impl-plans/active/vactrol-backend-contracts.md`

## sharedPaths

None. This plan runs alone in wave 1.

## File-Level Changes (signatures only; names follow the design)

1. `Cargo.toml` (12.8.10): `rust-version = "1.83"`; `[lib] crate-type = ["rlib", "cdylib"]`; features
   `host-native = ["dep:cpal", "dep:midir"]`, `host-wasm = []` (default stays `["host-native"]`, `lsp = []` unchanged);
   `cpal` and `midir` as `optional = true` under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`;
   `[[example]] name = "beep", path = "examples/beep.rs", required-features = ["host-native"]`. Version choice: try
   cpal 0.16.x first, then 0.15.3; midir 0.10.x. Accept a version only if `cargo build` on the pinned 1.83 toolchain
   succeeds for the whole tree; pin transitive crates with `cargo update -p <crate> --precise <ver>` when a transitive
   release needs a newer rustc. Record chosen versions, every `--precise` pin, and the `Cargo.lock` diff stat.
2. `src/lib.rs`: `pub mod host;` and, under `#[cfg(test)]`, `#[global_allocator] static ALLOC: dsp::alloc_probe::Counting`.
3. `src/host/mod.rs`: `pub mod caps; pub mod noop; pub mod wire;` `#[cfg(test)] pub(crate) mod testing;`
   `#[cfg(all(feature = "host-native", not(target_arch = "wasm32")))] pub mod native;`
   `#[cfg(all(target_arch = "wasm32", feature = "host-wasm"))] pub mod wasm;` `#[cfg(test)] mod tests;`
4. `src/host/wire.rs` (11.4, 11.3, 12.8.5): `MAX_CTLS: usize = 24`; `#[repr(C)] #[derive(Clone, Copy)] AudioEvent
   { time: f64, slot: SlotId, gen: u32, inst: InstId, voice_hint: u32, n_ctl: u8, ctl: [(CtlId, Ctl); MAX_CTLS] }`;
   `Ctl { Const(f32), Cell(CellId) }`; `SlotControl { slot, new_gen, effective_time: f64, release: Release }`;
   `Release { None, Natural, Panic }` (ordered); `SlotControl::merge(self, other) -> SlotControl` (gen max, time min,
   release max: the 11.3 monotone merge); `SlotControlAck { slot, gen }`; `VoiceTag { slot, channel, pitch, seq }`;
   `CtlMsg { SlotControl(..), CellInit { cell, epoch, value }, CellBatch { seq, count } /* header; `count` entries
   `(CellId, epoch: u32, f32)` follow it in the byte stream; browser tier only, native never sends it */,
   CellRetire { cell, epoch }, LiveNoteOn { tag: VoiceTag, ev: AudioEvent }, VoiceRelease { tag }, GraphInstall { id, gen },
   GraphRetire { id }, SampleSlice { resource, offset, len }, SampleRetire { resource } }` (fixed-size POD, no heap);
   `HostMsg { SlotControlAck(..), CellInitAck { cell, epoch }, CellBatchAck { seq }, Retired { resource }, SliceOk { resource, offset },
   Installed { resource, gen }, Counters { late, dropped, stolen, skipped }, AnalysisCell { id, value } }`;
   `fn encode(&self, out: &mut [u8]) -> usize` and `fn decode(bytes: &[u8]) -> Result<(Self, usize), WireError>` for
   `AudioEvent`, `CtlMsg`, `HostMsg` (fixed little-endian layouts, a leading tag byte). Slice payload bytes follow a
   `SampleSlice` record as raw `f32` LE; `encode_batch(seq, &[(CellId, u32, f32)], out)` / `decode_batch` handle the
   entries after a `CellBatch` header.
5. `src/host/caps.rs` (11.5, 12.8.3): `trait AudioHost { send(AudioEvent); control(SlotControl); post(CtlMsg); drain(&mut Vec<HostMsg>); now() -> f64; swap_graph(GraphHandle); install_sample(id: u32, data: Arc<SampleData>); retire_sample(id: u32); analysis() -> HostSigs }`
   (`install_sample`: native hands the `Arc` over, browser runs the 16.1 sender-paced slice window; completion arrives as
   `HostMsg::Installed`, retirement as `HostMsg::Retired`);
   `trait MidiHost { send(MidiEvent); control(SlotControl) }`; `trait OscHost` (same with `OscEvent`);
   `trait RenderHost { set_program(OutId, ShaderDesc); set_uniforms(OutId, &Uniforms) }`;
   `trait MidiInHost { poll(&mut self) -> &[MidiInEvent] }` with `MidiInEvent` exactly as design 11.7;
   `trait SampleLoader { load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> }`, `SampleSrc { Bank { kw: KwId, index: u32 }, Path(PathVal) }` (a bank is a host sample set; `index` is the event's `n`,
   7.1.4 "the host's sample index"),
   `SampleData { rate: u32, channels: u8, frames: Box<[f32]> }`;
   `trait InstResolver { route(&self, &Sound) -> Result<Route, Failure>; inst(&self, InstId) -> Option<Arc<InstDef>>; signal_inputs(&self) -> Vec<SignalInput> }`,
   `Route { Audio { inst: InstId, sample: Option<SampleSrc> }, Midi { ch: u8 }, Osc { addr: Rc<str> } }` (`sample` set
   when the instrument plays a host bank or a sample file, so commit can gate on the resource), `SignalInput { cell: CellId, sig: Rc<Sig> }`;
   `GraphHandle { Inst { id: InstId, def: Arc<InstDef> }, Bus { id: BusId, def: Arc<BusDef> }, Master(Arc<BusDef>) }`;
   `MidiEvent { Note { time, slot, gen, ch, note, vel, dur }, NoteOff { time, slot, gen, ch, note }, Clock { time },
   Start { time }, Stop { time }, Continue { time } }` (NoteOff for stale-started notes; clock/transport for the 11.7
   clock master), `OscEvent { time, slot, gen, addr: Rc<str>, args: Vec<OscArg> }`,
   `OscArg { F(f32), I(i32), S(Rc<str>) }`; `HostSigs { amp: f32, fft: [f32; 8] }`;
   `struct Hosts { audio, midi, osc, render, midi_in, samples }` (each a `Box<dyn ..>`), `Hosts::noop()`.
6. `src/host/noop.rs`: `NoopHost` implementing every trait above plus `ns::load::SourceLoader` (reads fail
   `host-unavailable`, sends are dropped, `now()` is 0.0). `src/ns/load.rs`: delete the local struct and
   `pub use crate::host::noop::NoopHost;` (every existing use keeps compiling).
7. `src/host/testing.rs` (`#[cfg(test)]`, 12.8.5): `RecordingAudioHost`/`RecordingMidiHost`/`RecordingOscHost`/
   `RecordingRenderHost` (record every call in order, with the time it arrived), `MockClock`, `NativeTransport`
   (shared `AtomicCells`, immediate delivery) and `BrowserTransport` (a FIFO of `CtlMsg` with configurable delay in ticks,
   drop-next-n loss injection and stall/resume, delivering into a `dsp::cells::Mirror` and returning `HostMsg` acks
   through the same FIFO discipline; never reads evaluator memory).
8. `src/dsp/graph.rs` (12.1, 12.5, 12.6): keep `InstId`; add `BusId`, `BankRef(u32)`, `TableRef(u32)`, `IrRef(u32)`,
   `GranSrc { Sample(BankRef), Table(TableRef), Bus }`, `UGenSpec` exactly as design 12.1, `Edge { from: u16, to: u16, port: u8 }`,
   `InstDef { id, params: Box<[(CtlId, Ctl)]>, nodes: Box<[UGenSpec]>, edges: Box<[Edge]>, node_params: Box<[(u16, CtlId, Ctl)]> }`,
   `EffectKind` with ONE variant per design-music section 5 name (kebab-case to CamelCase, e.g. `MultibandCompressor`,
   `PingPong`, `Codec`, `Radio`) plus `Analyzer(AnalyzerKind)` (`Level, Spectrum, Spectrogram, NoteSpectrogram,
   Oscilloscope, PitchMeter, StereoMeter`) and `Granulate`; `EffectKind::from_name(&str) -> Option<EffectKind>` and
   `name()`; `EffectSpec { kind, params: Box<[(CtlId, Ctl)]> }`; `BusDef { id, chain: Box<[EffectSpec]> }`;
   `NODE_CAP: usize = 256`; the evaluator-side tree `UGenNode { kind: UGenKind, args: Box<[(Option<KwId>, UGenInput)]> }`,
   `UGenKind { Ugen(UGenSpec), Effect(EffectKind), BusInput }`, `UGenInput { Node(Rc<UGenNode>), Const(f32), Param(CtlId), Signal(Rc<Sig>), Keyword(KwId), List(Rc<[f32]>) }`
   (derive `Debug`; INST interprets it).
9. `src/dsp/controls.rs` (12.8.7): `ControlRow { name: &'static str, ctl: CtlId, default: f32, range: (f32, f32), route: CtlRoute, domain: CtlDomain }`,
   `CtlRoute { InstParam, OrbitFx { unit: u8, param: u8 }, BusUnit { param: u8 }, Scheduler }`, `CtlDomain { Float, Bool, Enum(&'static [&'static str]), Resource }`;
   `fn row(name: &str) -> Option<&'static ControlRow>`, `fn row_by_id(CtlId) -> Option<&'static ControlRow>`,
   `fn encode(row, &Value) -> Result<f32, Failure>` (keyword -> enum index, bool -> 0/1, number -> f32; resource keywords
   are encoded by the caller). Rows: every design-music "sound parameters available on any pattern" name, `freq`, `amp`,
   `note`, `n`, and every template header or body control of design-music sections 4 and 6 (`attack decay sustain release
   cutoff res wave unison detune drift ratio index algorithm position table bank loop size density spray pitch
   pitch-spray envelope reverse freeze stereo-spray source shape`). `gain` routes to `InstParam` of `amp`; `room` to
   `BusUnit`; `orbit`, `bus`, `cut`, `legato` to `Scheduler`.
10. `src/dsp/cells.rs` (11.3): `CellId(u32)`, `trait CellRead { fn get(&self, CellId) -> f32 }`, `AtomicCells`
    (`Arc<[AtomicU32]>`, release store / acquire load, fixed capacity), `Mirror` (fixed capacity) with per-id
    `Vacant | Live { epoch, value } | Retiring { epoch }`, `last_epoch`, highest initialized epoch, last applied `seq`;
    `apply_init(cell, epoch, value) -> HostMsg` (init-once, replay ack-only, stale drop with ack), `apply_batch(seq, entries) -> Option<HostMsg>`
    (seq must increase; entries apply only on the exact Live epoch), `retire(cell, epoch)`, `retired(cell)`.
11. `src/dsp/release.rs` (11.7): `TagMap` (preallocated `tag -> voice index`, capacity = voices, oldest-open lookup for
    stealing) and `Tombstones` (ring of 64 tags); no allocation after construction.
12. `src/dsp/caps.rs` (12.7, 12.8.8): `CapabilitySet` exactly as design 12.7; `CapabilitySet::browser()` (64 voices,
    2.0 s IR, 200 grains/s, 0.5 s grain, 8 s capture, 2 ch, offline false, midi in/out false, file false) and `native()`
    (256, 10.0, 1000, 2.0, 30, 2, offline false, midi in/out true, file true); `Cap { Voices(u16), IrSeconds(f32), GrainDensity(f32), GrainSize(f32), CaptureSeconds(f32), OfflineRender, MidiIn, MidiOut, FileAccess }`;
    `require(&self, cap, origin: Option<Span>) -> Result<(), Diagnostic>` (`beyond-capability`, message "not available
    on this host" plus the limit).
13. `src/dsp/alloc_probe.rs` (`#[cfg(test)]`, 12.8.9): `Counting` (`GlobalAlloc` over `System`, counts `alloc` and
    `realloc` on the current thread while a `const`-initialized thread-local flag is set), `fn armed<R>(f: impl FnOnce() -> R) -> (R, usize)`.
14. Enum shapes (12.8.2): `Value::UGen(Rc<UGenNode>)`, `Sound::Inst(InstId)`, `Sound::Osc(Rc<str>)`; arms: print
    `<ugen>`, `(sound inst N)`, `(sound osc "/a")`; `deep_eq` pointer equality for `UGen`, structural for the sounds;
    `kind_name` "a unit generator"; `value/tests/mod.rs` kind list; `integrate_sound.rs` `sound_str`.
    `StagedEffect::Install(GraphHandle)` in `ns/stage.rs` (doc: released by an `inst`/`bus`/`master` definition, BE-INST
    stages it, BE-SCHED applies it).
    Late-control provenance (11.3 "late-bound controls resolve at commit"; the pattern engine resolves late refs at query
    time, so commit needs the source): `pattern::query::Event` gains `late: Option<VarSlotRef>` (this event's value came
    from a directly referenced var or tweak) and `cells: BTreeMap<KwId, VarSlotRef>` (controls whose value came from
    one), both empty in `Event::new` (`src/pattern/step.rs`). BE-SCHED fills them; nothing else changes.
15. Codes (12.8.12): `DiagCode` + `graph-too-large`, `host-transport`, `ring-overflow`, `latency-widened` (w),
    `arena-exhausted`, `install-queue-overflow`, `grain-skip` (w), `voice-steal` (w), `clock-lost` (w), `clock-external`
    (10 new: the assertion becomes 75); `FailCode` + `inst-failed`, `too-many-controls` (assertion becomes 22).
16. Skeleton (12.8.2): `sched/mod.rs` declares `slots runtime staging ledger commit control cells dryrun telemetry oneshot midi_in midi_clock`
    and `#[cfg(test)] mod tests;` (`tests/mod.rs`: `mod sched; mod midi;`); `dsp/mod.rs` declares `graph controls cells
    release caps engine voice ring fft granular bus arena meta build ugen effects`, `#[cfg(test)] pub(crate) mod alloc_probe;`,
    `#[cfg(test)] mod tests;` (`contracts`, `dsp`); `host/tests/mod.rs`: `mod contracts; mod e2e;`; each language
    `tests/mod.rs` gains `mod inst;`. Stubs compile with no items (a `//!` line such as `//! Owned by BE-DSP (design 12.8.9).`).

## Required Tests (`src/host/tests/contracts.rs`, `src/dsp/tests/contracts.rs`)

- Wire: encode/decode round trip for every `CtlMsg`/`HostMsg` variant and an `AudioEvent` with 24 controls; a truncated
  buffer is `WireError`, never a panic; `SlotControl::merge` gives hush-then-stop Panic, hush-then-tempo Panic with the
  larger gen, and is idempotent (merging the same control twice changes nothing).
- Mirror: init then batch then replayed init keeps the batch value; stale epoch update is inert; retire -> retired ->
  init with a greater epoch reaches Live; a batch with a non-increasing seq is rejected.
- Caps: browser `require(OfflineRender, Some(span))` and an IR of 3 s give `beyond-capability` "not available on this
  host" whose origin span equals the `span` passed in (asserted: TASK-008 criterion 7 "with origin"); native accepts
  both limits it advertises.
- Controls: `row("gain")` routes to `amp`; `encode` of `:hann` on `envelope` gives its index; `true` gives 1.0; an
  unknown keyword is `Failure(type)`.
- Alloc probe: `armed(|| Vec::<u8>::with_capacity(8))` counts >= 1; `armed(|| 1 + 1)` counts 0.
- TagMap/Tombstones: insert, release by tag, oldest-open steal, tombstone hit; capacity never grows.

## Invariants

- Both wasm32 builds stay green: nothing native-only compiles for wasm32; `host-wasm` pulls no crate.
- No `std::{thread,fs,time,net,process}` outside `src/host/native/` (none exists yet). No panic on untrusted bytes.
- Existing behavior unchanged: all 489 existing tests keep passing; only the two code-count numbers change.
- No `.rs` file reaches 800 lines. Stubs contain no items, so later waves own every line of them.

## Edit Protocol (common to every BE plan)

1. Evidence directory: `tmp/be-backend-20260925-s181/<planId>/attempt-<n>/` (gitignored).
2. Before each edit, read the file fresh and append `shasum -a 256 <file>` to `pre-edit-hashes.txt`. Before the first edit
   of a sharedPath or conditional path, write a one-line intent snapshot (file, purpose) to `intent.md`. After each edit,
   append the post-edit hash to `post-edit-hashes.txt`.
3. Drift: if a pre-edit hash differs from this plan's last recorded post-edit hash for that file, stop, re-read, and
   re-apply the recorded intent; never revert another plan's hunk. Record the event.
4. Keep the tree compiling between edits. A build error in a file outside this plan's writePaths/sharedPaths is waited
   out and recorded, never fixed here; serial repair happens at the join and in BE-FINAL.
5. `rustfmt --edition 2021` on owned `.rs` files only; never crate-wide `cargo fmt` (BE-FINAL runs the check).
6. Never `git reset`, `git clean`, `git stash`, `git checkout -- <path>`, never create branches or worktrees, never commit.
7. Edit only this plan's own progress log among plan documents. Rust code is written through the rust-coding agent and
   followed by the check-and-test-after-modify agent (CLAUDE.md).
8. At the end write `final-hashes.txt` with the sha256 of every file this plan wrote.

## Verification (foreground; design 6.5.7 evidence rule, common to every BE plan)

`mkdir -p target/fe-logs` first. `LOG` is a new file `target/fe-logs/be-<wave>-<check>-s<S>-<n>.log` (`<wave>` =
`contracts` here; `<S>` = the Riela session number; `<n>` counts from 1 per check within the session; never overwrite).
Run each cargo row as `(set -o pipefail; CMD 2>&1 | tee LOG); echo "exit=$?" >> LOG`. The progress log cites the
counting log (last run after the final code change) with its `exit=`; V3/V3t also cite run/passed counts (non-zero). A
missing log, a log without `exit=`, or a truncated log fails the row. `\|` is a literal `|`.

| # | Command (`CMD`) | `<check>` | Evidence |
|---|---------|-----------|----------|
| V1 | `CARGO_TERM_QUIET=true cargo build` | `build` | `exit=0`, no warnings |
| V2 | `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` | `clippy` | `exit=0` |
| V3 | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run` | `nextest` | `exit=0`, run > 0, 0 failed |
| V3t | `CARGO_TERM_QUIET=true cargo test` | `cargotest` | `exit=0`, non-zero counts |
| V3f | `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'binary(spec_fixtures)'` | `fixtures` | `exit=0`, run > 0 |
| V6a | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown` | `wasm32` | `exit=0` |
| V6b | `CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm` | `wasm32-hostwasm` | `exit=0`; `target/wasm32-unknown-unknown/debug/vactrol.wasm` exists |
| V4 | `find src tests examples -name '*.rs' -exec wc -l {} + \| sort -n \| tail -5` | - | every file under 800 lines |
| V5 | `grep -rnE 'std::(thread\|fs\|time\|net\|process)' src/ --exclude-dir=native \|\| echo none` | - | prints `none` |
| V8 | `rustfmt --edition 2021 --check <owned .rs files>` | - | exit 0 |

Plan-specific rows:

| # | Command | Evidence |
|---|---------|----------|
| C1 | LOG(`be-contracts-audit`): `cargo audit` | `exit=0` or every advisory listed with the dependency path in the progress log (a vulnerability in cpal/midir's tree blocks the plan) |
| C2 | `git diff --stat -- Cargo.toml Cargo.lock` | only this plan's dependency change; versions and `--precise` pins recorded |
| C3 | `CARGO_TERM_QUIET=true cargo tree -e normal --target wasm32-unknown-unknown` | no `cpal`/`midir`/`coreaudio`/`alsa` in the output |

## Completion Criteria

- [ ] Cargo features, target-gated optional cpal/midir, `crate-type`, `rust-version`, `[[example]] beep`; C1-C3 recorded
- [ ] `host/{caps,wire,noop,testing}.rs` and `dsp/{graph,controls,cells,release,caps,alloc_probe}.rs` exist as listed
- [ ] The 12.8.2 skeleton exists: every declaration and stub of items 16 and the writePaths list; later waves need not
      edit any `mod.rs` they do not own
- [ ] `Value::UGen`, `Sound::Inst`, `Sound::Osc`, `StagedEffect::Install` declared with minimal arms; `NoopHost` re-exported
- [ ] 10 `DiagCode`s and 2 `FailCode`s added; count assertions 75 and 22
- [ ] Required tests pass; V1-V8 and C1-C3 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-CONTRACTS implementer)` entry with work done, chosen crate
versions, design differences, hash/intent paths, evidence per row, blockers. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-007, TASK-008)
- **Next**: vactrol-backend-sched.md, vactrol-backend-dsp.md, vactrol-backend-inst.md
