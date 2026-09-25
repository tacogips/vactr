# Vactrol Back End: DSP Engine, Voices, Effects, Granular, Buses, Arena (BE-DSP) Implementation Plan

**planId**: BE-DSP (vactrol-core.md TASK-008 pure-DSP deliverables)
**Status**: Completed (accepted by integration review; reconciled by BE-FINAL session 186; archive after the workflow commit)
**Design Reference**: design-docs/specs/design-implementation.md 12.1-12.7, 16, 16.1, 11.3 (cell reads), 11.7 (VoiceRelease, open voices), 17 (invariant 3), 12.8.5, 12.8.8, 12.8.9; design-music.md sections 4-6
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/3
**dependsOn**: BE-CONTRACTS
**Dispatch manifest**: impl-plans/active/be-backend-20260925-s181-dispatch.json

---

## Intent and Context

The audio side sees only POD events, installed graph templates, `f32` buffers and cells, and it never allocates, locks
or touches `Rc` in the callback (12.2, 17). This plan builds `dsp::Engine`, the ONE callback core that the native cpal
callback and the worklet `process()` both call (12.8.9), and everything it renders: the voice pool, the event ring, the
extended ugen catalog, the full effect catalog, the FFT, the granular engine, the bus graph with master, analyzers,
orbit effects, `VoiceRelease` handling, and the resource stores (native `Arc` table, browser `SampleArena` with the 16.1
slice install protocol). Inputs from BE-CONTRACTS: `InstDef`/`UGenSpec`/`EffectKind`/`BusDef`/`NODE_CAP`
(`dsp/graph.rs`), `CellRead`/`AtomicCells`/`Mirror` (`dsp/cells.rs`), `TagMap`/`Tombstones` (`dsp/release.rs`),
`CapabilitySet` (`dsp/caps.rs`), `alloc_probe`, and the wire records (`host/wire.rs`). Tests build `InstDef`s by hand;
BE-INST (in parallel) produces them from source later, and BE-FINAL joins the two.

## Non-Goals

- No language changes, no natives, no `InstDef` lowering (BE-INST). No scheduler (BE-SCHED). No cpal/worklet glue
  (BE-NATIVE/BE-WASM). No `HostManifest` wiring of the meta table (BE-FINAL).
- Effect sound quality is not an acceptance criterion (12.8.8, user-QA B4); each kind is a bounded approximation.
- No SIMD, no micro-optimization beyond what the zero-allocation and bounded-work rules need.

## writePaths (exclusive)

- `src/dsp/engine.rs`, `voice.rs`, `ring.rs`, `fft.rs`, `granular.rs`, `bus.rs`, `arena.rs`, `meta.rs`
- `src/dsp/ugen/mod.rs` and new files under `src/dsp/ugen/` (`osc.rs`, `filter.rs`, `env.rs`, `fm.rs`, `additive.rs`,
  `wavetable.rs`, `sample.rs`)
- `src/dsp/effects/mod.rs` and new files under `src/dsp/effects/` (`dynamics.rs`, `eq.rs`, `delay.rs`, `reverb.rs`,
  `saturation.rs`, `modulation.rs`, `lofi.rs`, `resonator.rs`, `spatial.rs`, `restoration.rs`, `utility.rs`,
  `analyzer.rs`, `prim.rs` for shared primitives)
- `src/dsp/tests/dsp.rs` and new files under `src/dsp/tests/dsp/`
- `impl-plans/active/vactrol-backend-dsp.md`

## sharedPaths

None.

## File-Level Changes (signatures only)

1. `ring.rs`: `EventRing` lock-free SPSC over `AudioEvent` (capacity from construction, default 1024), `split() ->
   (EventProducer, EventConsumer)`; push on full returns `Err` and the producer counts a drop (the host reports
   `ring-overflow`); the same generic ring (`SpscRing<T: Copy>`) carries `CtlMsg` and `HostMsg` records for the native
   tier (256 each).
2. `engine.rs`: `Engine::new(caps: &CapabilitySet, sample_rate: f32, max_block: usize, store: StoreKind) -> Engine`, where
   all allocation happens and `StoreKind { NativeArc, Arena { bytes: usize } }`;
   `Engine::process<C: CellRead>(&mut self, io: &mut EngineIo<'_, C>, out: &mut [f32], frames: usize)`. `EngineIo` holds the
   event consumer, the control-record source, the ack sink and the cells. Order per call: apply control records
   (SlotControl with release semantics and short-gating of stale-started voices, LiveNoteOn, VoiceRelease, cell records
   via `Mirror` on the browser tier, at most one `CellBatch`, graph/sample install work under the 16.1 credit), then
   events due in the block (start frame `round((time - block_start) * sr)`, late events at frame 0 and counted,
   stale-gen events dropped at dequeue), then render voices -> orbit effects -> buses -> master, then publish counters
   and analysis cells. `Engine::install_native(GraphHandle-equivalent POD + Arc data)` for the native triple-buffer swap
   (the retired structure is returned for dropping on the evaluator thread). Counters: late, dropped, stolen,
   grains skipped, bytes copied this quantum (max and total), memory capacity at init.
3. `voice.rs` (12.2, 11.7): `VoicePool` (capacity = `caps.max_voices`), `Voice` (template ref, slot, gen, tag, per-node
   state block sized at construction for `NODE_CAP` nodes, delay-line memory from a fixed per-voice pool), `Release`
   handling (`None` rings on; `Natural`: scheduled voices end naturally, OPEN-duration voices enter the release stage;
   `Panic`: 3 ms short gate); open-duration voices (live notes) keyed through `TagMap`; tombstone drop; pool exhaustion
   steals the oldest open input voice (short gate) and counts `stolen`.
4. `ugen/*`: one DSP kernel per `UGenSpec` variant (core set, `Vco` with unison up to `unison_max`, `SubOsc`, `Ladder`,
   `Svf`, `FmOp`/`FmMod`, `PhaseDistortion`, `Additive` up to `partials_max`, `Wavetable`, `SamplePlay` with
   begin/end/speed/loop controls and bound checks, `Granular` delegating to `granular.rs`, `Effect` delegating to
   `effects`); inputs read `Ctl::Const` or `Ctl::Cell` at voice start and control-rate cells continuously.
5. `effects/*` (12.5, 12.8.8): `EffectUnit` per `EffectKind` built from `prim.rs` (biquad, one-pole, delay line,
   allpass/FDN, envelope follower, waveshaper, LFO); the approximated kinds listed in 12.8.8 carry a doc line naming the
   approximation; convolution uses an installed IR resource; analyzers write `f32` cells (ring-buffered frames for
   spectrogram/oscilloscope) and pass audio through untouched. `fft.rs`: fixed-size radix-2 FFT with preallocated
   twiddles.
6. `granular.rs` (12.6): `GrainPool` sized `ceil(max_grain_density * max_grain_size * sr)`-bounded slots at install;
   static sources and the live capture buffer (`max_capture_seconds`), valid window with guard = grain length + one
   block, captured-extent intersection, freeze/unfreeze with the active-grain unfreeze re-check and short gate,
   phase-accumulator onsets, seeded per-instance RNG, envelopes `:hann :tri :trapezoid :expo` by table, skip-and-count for
   window violations and pool exhaustion.
7. `bus.rs` (12.5): `BusGraph` (preallocated bus units, master fed by every bus, per-slot routing by the `bus` control,
   `room` mapped to the bus reverb parameter), bus swap under the generation + refcount lifecycle (native: triple
   buffer; browser: graph slots in the arena).
8. `arena.rs` (16.1): `SampleArena` (fixed capacity, free list), graph template slots, `InstDef`/`BusDef` byte codec
   (templates stay within one 64 KB slice because of `NODE_CAP`), the sender-paced slice install (`SliceOk` only after the
   copy ran; `INSTALL_BYTES_PER_QUANTUM = 65536` credit per `process()`, replenished only by `process()`; deferred slice
   queue with a bound, overflow -> `install-queue-overflow`), admission (larger than free space -> `arena-exhausted`),
   uniform refcounted retirement for samples AND graphs (queued events + voices + template refs; `Retired` only at
   zero), `Installed { resource, gen }` on completion. Oversized graph bytes -> `graph-too-large`.
9. `meta.rs` (13.5): `EditorKind` (the 13.5 list), `ParamMeta { ctl or param name, range, curve, unit, group }`,
   `EditorDecl { kind, params }`, `fn decl_for(name: &str) -> Option<&'static EditorDecl>` covering every `EffectKind`, every
   ugen name, and the seven template names.

## Required Tests (`src/dsp/tests/dsp/*.rs`; every render runs inside `alloc_probe::armed` and asserts 0)

- `render.rs` (TASK-008 criterion 1): events pushed into the ring, N buffers rendered, voice starts land on the exact
  sample frame; late event counted and started at frame 0; stale-gen event dropped; allocation count 0.
- `region.rs` (criterion 3, DSP half): a `SamplePlay` event renders exactly its begin/end region; speed and loop
  (`loop-at`-style controls) render stretched and looped; regions at the sample bounds never read out of bounds
  (asserted); no new audio-thread machinery beyond begin/end/speed/loop controls (asserted structurally: only those
  `CtlId`s are read by `SamplePlay`).
- `bus.rs` (criterion 4, DSP half): a slot event routed to a bus reaches master; bus swap retires the old chain only at
  refcount zero; `room` maps to the bus reverb parameter; zero allocation during swap.
- `analyzer.rs` + `granular.rs` (criteria 5, 6): bit-identical output with and without an analyzer; granular onset count
  and timing match the phase-accumulator model under a fixed seed; live-bus wrap guard (no grain reads an overwritten
  sample); freeze keeps frozen output invariant while input continues; unfreeze resumes live; active-grain unfreeze
  short-gates only violating grains; partial-fill freeze exposes only captured audio; freeze right after install spawns
  none (skip-counted); density above `max_grain_density`, size above `max_grain_size` and capture depth beyond the cap
  are clamped and counted with audio continuing (no dropout); the diagnostic half is BE-SCHED's
  `src/sched/tests/sched/granular.rs`; pool exhaustion skips with a count.
- `caps.rs` (criterion 7, DSP half): installing an IR longer than `max_ir_seconds` fails with the `beyond-capability`
  diagnostic and the diagnostic's origin span equals the span passed with the install request (asserted); the
  offline-render half is BE-CONTRACTS' `require(OfflineRender)` test (cited, not duplicated).
- `catalog.rs`: every `EffectKind` renders finite, bounded output on noise and silence; `mix 0` (where the kind has a mix)
  and `section` off are bit-identical to the input; zero allocation; `meta::decl_for` has exactly one entry per effect,
  ugen and template name.
- `release.rs` (criterion 8's `VoiceRelease` half, engine level): a release reaches exactly the tagged voice after a
  slot-gen bump; release-before-start hits the tombstone and the late start is dropped; oldest-open steal counts;
  `Natural` puts open voices into release; `Panic` short-gates them.
- `cells.rs`: a voice reads a `Mirror` cell at voice start; a batch applied between two quanta changes the next voice's
  value; `AtomicCells` store is visible to the next voice start.
- `arena.rs` (criterion 11 mechanics, headless; the real-worklet proof is BE-WASM's): per `process()` at most
  `INSTALL_BYTES_PER_QUANTUM` bytes copied across a burst (total per quantum measured); acks withheld until the copy
  runs; deferred-queue overflow diagnostic; admission failure; unload while playing holds storage until events and
  voices release it; arena capacity never changes after `Engine::new`.

## Invariants

- No allocation, lock, `Rc` or closure call inside `Engine::process` and everything it calls (asserted by the probe).
- `dsp/` has no `std::{thread,fs,time,net,process}`. The files this plan writes never use `Value`, `Rc`, `vm`, `ns` or
  `pattern` types (the evaluator-side `UGenNode` in `graph.rs` is BE-INST's input, never the engine's).
- Output is finite for every input (NaN/inf guarded at bus and master). No `.rs` file reaches 800 lines.

## Edit Protocol

Common protocol of `vactrol-backend-contracts.md`; evidence under `tmp/be-backend-20260925-s181/BE-DSP/attempt-<n>/`.
Runs concurrently with BE-SCHED and BE-INST on disjoint files.

## Verification

Common table with `<wave>` = `dsp`: V1, V2, V3, V3t, V3f, V6a, V6b, V4, V5, V8. Plus D1:
`NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/dsp::tests/)'`
as LOG(`be-dsp-own`), run count per test file cited.

## Completion Criteria (map to vactrol-core.md TASK-008)

- [x] Engine, voices, ring, ugens, effect catalog, FFT, granular, buses, analyzers, arena, meta implemented as listed
- [x] Criterion 1 proven; the render/engine halves of 5 and 6 (bit-compare, pool, grain behavior, clamp-and-count)
      proven (their diagnostic half is BE-SCHED's); the DSP halves of 3, 4, 7 and the engine-level `VoiceRelease` half
      of 8 proven
- [x] Every render test asserts zero callback allocation via `alloc_probe`
- [x] V1-V8 and D1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-DSP implementer)` entry: work done, per-kind approximation notes,
state sizing, design differences, hash/intent paths, evidence per row, blockers. Edit only this log.)

### Session: 2026-09-25 (session 182, BE-DSP implementer)

**Work done.** All owned stubs replaced. `ring.rs` (generic SPSC `SpscRing<T: Send>`, `EventRing` 1024, control/ack
channels 256, `ControlSource`/`Budget`/`Record`, `NativeInstall`/`NativeRecord`/`Garbage`, browser `ByteInbox`,
`CellStore` for `Mirror`/`AtomicCells`, `EngineIo`, `EngineConfig`, `Counters`, browser record encoders);
`engine.rs` (`Engine::new/with_config/process`, the single callback core); `voice.rs` (`VoicePool`, `Voice`,
`SlotGens` 11.3 (a)-(c) + gen piggyback, per-voice orbit post-fx, 3 ms short gate); `ugen/` (`RawGraph`,
allocation-free `Template::build`, port catalog with implicit control names, kernels in osc/filter/env/fm/additive/
wavetable/sample/mixer); `effects/` (`FxState`/`FxUnit`/`FxCtx`, `mix` blend with bit-identical `mix 0` bypass,
`prim.rs`, one file per group, `catalog.rs`); `fft.rs`; `granular.rs`; `bus.rs` (`BusGraph`, `BusTemplate`,
`OrbitDelay`, built-in `room` reverb, `section` bypass); `arena.rs` (`SampleStore` native `Arc` table + browser arena
with first-fit free list, admission, slice install, refcounted retirement, graph byte codec, `Templates` slots);
`meta.rs` (`EditorKind`/`ParamMeta`/`EditorDecl`, `decl_for`). Effect group files were written by parallel
rust-coding workers on disjoint files and integrated here.

**Approximations (B4, doc lines in code).** linear-phase-eq/group-delay-eq: min-phase shelves + allpass; fir-crossover:
LR-style biquad cascade; pitch-shift-hq: four overlapping grains; denoise: per-band downward expander; codec: band-limit
+ bit/rate reduction + pre-echo smear; spatial-map: M/S + allpass decorrelation; crosstalk-cancel: single delayed
inverted low-passed crossfeed; convolution: <=512-tap stride-decimated time-domain FIR over the installed IR;
dsd-imd: quadratic IMD + shaped noise; fm-mod: `sin(asin(in) + index*mod)`.

**State sizing (defaults, `EngineConfig`).** Voices = `caps.max_voices`, each NODE_CAP `NodeState`s, 4 voice
`FxUnit`s, 0.5 s delay memory; 64 template slots (arena tier: preallocated templates); 6 bus slots of 4 s +
granulate capture (`max_capture_seconds`) + pool; 4 orbit delays of 4 s; 4096 analysis cells; pending events 1024;
grain pool `ceil(max_grain_density * max_grain_size)`; tag map = voices; tombstones 64; deferred starts 16; install
deferral bound `DEFERRED_MAX` 8 held records.

**Design differences / notes for BE-FINAL, BE-SCHED, BE-INST, BE-NATIVE, BE-WASM.**
- No `Engine::install_native` method: native installs travel as `NativeRecord::Install` on the control ring
  (`Template::from_inst` / `BusTemplate::from_def` built on the evaluator thread); retired boxes/`Arc`s come back
  on a `Producer<Garbage>` for dropping off the audio thread. Resource ids are one namespace for samples and graphs
  (`Installed`/`Retired { resource }`).
- Browser byte stream: `CtlMsg` records as encoded by `host/wire.rs`, plus `SampleBegin` (tag 0x1A,
  `ring::encode_sample_begin`), `SampleSlice` header + `len` f32 LE samples (`encode_slice`), `GraphInstall` header
  + u32 length + `arena::encode_inst`/`encode_bus` bytes (`encode_graph_record`). Install faults
  (`arena-exhausted`, `install-queue-overflow`, `graph-too-large`) are drained with `Engine::pop_fault`.
- Effect parameters use effect-local ids `CtlId(0x4000 + i)` (`effects::param_ctl`), since most names are not
  control rows; ugen ports use the row id of the port name, else `0x4000 + port`. Unconnected ports named after an
  `InstParam` row read that voice control (B2 implicit names).
- AudioEvent has no duration: a scheduled voice holds its gate for the `legato` control in seconds when present,
  else `attack + decay`. When a template does not read `amp`/`pan`, the voice applies the event's `amp` (1 when
  absent) and `pan` (0.5). `lpf hpf resonance crush shape vowel` run as per-voice post effects, `delay/delaytime/
  delayfeedback` as per-orbit delays into the master, `room`/`size` set the routed bus's built-in reverb.
- `Additive { partials_max }` carries no amplitude list: ports are `freq count tilt` (amp_k = 1/k^tilt); BE-INST
  maps a `partials` list onto `count`/`tilt`.
- `granular` with a `:bus` source inside a voice renders silence (the live face is the `granulate` effect).
- Output is interleaved stereo; `process` splits callbacks longer than `max_block`.
- Manifest-listed files left unused (harmless): `ugen/{vco,subosc,ladder,svf,fmop,fmmod,pd,sampleplay,noise,lfo}.rs`,
  `tests/dsp/{mod,regions,alloc}.rs` (code lives in the listed `osc/filter/fm/sample.rs`, `region.rs`, and every
  render asserts the probe via `Rig::step`).

**Evidence (session 182, final tree, logs under `target/fe-logs/`, each ends with `exit=`).**
- V1 build: `be-dsp-build-s182-1.log` exit=0
- V2 clippy `--all-targets -D warnings`: `be-dsp-clippy-s182-1.log` exit=0
- V3 fmt `--check`: `be-dsp-fmt-s182-1.log` exit=0
- V3t nextest: `be-dsp-nextest-s182-1.log` exit=0, 691 run / 691 passed
- V3f plain cargo test: `be-dsp-cargotest-s182-1.log` exit=0, 681 + 10 passed, 0 failed
- V6a fixtures `binary(spec_fixtures)`: `be-dsp-fixtures-s182-1.log` exit=0, 10 run / 10 passed
- D1 own `test(/dsp::tests/)`: `be-dsp-own-s182-1.log` exit=0, 84 run / 84 passed (BE-DSP 68: render 7, region 7,
  bus 7, analyzer 4, granular 11, caps 3, catalog 4, release 4, cells 5, arena 5, templates 3, ugens 4, effects 4;
  plus 16 BE-CONTRACTS)
- V4 wasm32 default: `be-dsp-wasm32-s182-1.log` exit=0; V5 wasm32 host-wasm: `be-dsp-wasm32-hostwasm-s182-1.log` exit=0
- V8: largest `.rs` is `src/dsp/engine.rs` 785 lines (< 800); `std::{thread,fs,time,net,process}` grep outside
  `native/`: none.
- Hashes: `tmp/be-backend-20260925-s181/BE-DSP/attempt-1/{pre-edit-hashes.txt,intent.md,final-hashes.txt}`.

**Blockers.** None. Formal test-integrity/adversarial/integration review and the Completed status are downstream.

### Session: 2026-09-25 (session 182, BE-DSP implementer, revision after step7 adversarial review)

**Finding addressed (mid, src/dsp/engine.rs:727).** `Engine::retire` acked `CellRetired` while a template or bus
still read the cell through a node port, an effect-node parameter or a bus-chain parameter. The cell in-use check
now covers every reader: `Template::reads_cell` (control defaults, node-port `Src::Cell` inputs, effect-node
`fx_params`) via `Templates::reads_cell` over live and retiring slots, plus the new `BusGraph::reads_cell` (room unit
and chain units of live and retiring slots, `FxUnit::reads_cell`). Optional item also done: the convolution `ir`
resource counts as a reference (`Template::reads_resource`, `BusGraph::references`, `FxUnit::reads_resource`), so
`SampleRetire` waits for a retiring tail that still convolves it. New tests in `src/dsp/tests/dsp/cells.rs`:
`a_port_only_cell_is_not_retired_while_its_template_is_live_or_retiring`,
`a_bus_only_cell_is_not_retired_while_its_chain_is_installed`, `an_impulse_response_is_held_while_a_bus_convolves_it`.
Low residuals from the review are left for BE-NATIVE, BE-WASM and BE-FINAL, which own ack-ring sizing and drain
cadence, the garbage ring supply and dealloc counting.
Correction: the effect catalog has 102 `EffectKind`s, not the 98 stated earlier.

**Evidence (final tree, `target/fe-logs/`, each ends with exit=0).** build `be-dsp-build-s182-4.log`; clippy
`be-dsp-clippy-s182-4.log`; fmt `be-dsp-fmt-s182-4.log`; nextest `be-dsp-nextest-s182-4.log` 694/694; cargo test
`be-dsp-cargotest-s182-4.log` 684 + 10 passed; fixtures `be-dsp-fixtures-s182-4.log` 10/10; D1
`be-dsp-own-s182-4.log` 87/87 (BE-DSP 71 + CONTRACTS 16); wasm32 `be-dsp-wasm32-s182-4.log`; host-wasm
`be-dsp-wasm32-hostwasm-s182-4.log`. Intermediate runs: s182-2 (clean, but arena.rs was 802 lines) and s182-3 (clippy
exit=101 from an unused import, fixed). Largest `.rs` is engine.rs at 786 lines; std-module grep: none. Hashes:
`tmp/be-backend-20260925-s181/BE-DSP/attempt-2/{pre-edit-hashes.txt,intent.md,final-hashes.txt}`.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-008)
- **Previous**: vactrol-backend-contracts.md
- **Next**: vactrol-backend-midi.md, vactrol-backend-native.md, vactrol-backend-wasm.md, vactrol-backend-finalize.md

### Closing note (BE-FINAL, session 186)

Accepted by the integration review (acceptedPlanIds) and reconciled by BE-FINAL on the joined tree: every final-tree gate exits 0 (`target/fe-logs/be-final-<check>-s186-1.log`), and the TASK-007/008 checkboxes in vactrol-core.md cite this plan's tests. BE-FINAL serial repair R2d: the wavetable kernel reads the voice's `bank` resource like sample-play and granular. Archive to impl-plans/completed/ in the separate docs commit after the workflow commit.
