# Vactrol Back End: DSP Engine, Voices, Effects, Granular, Buses, Arena (BE-DSP) Implementation Plan

**planId**: BE-DSP (vactrol-core.md TASK-008 pure-DSP deliverables)
**Status**: Ready
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

- [ ] Engine, voices, ring, ugens, effect catalog, FFT, granular, buses, analyzers, arena, meta implemented as listed
- [ ] Criterion 1 proven; the render/engine halves of 5 and 6 (bit-compare, pool, grain behavior, clamp-and-count)
      proven (their diagnostic half is BE-SCHED's); the DSP halves of 3, 4, 7 and the engine-level `VoiceRelease` half
      of 8 proven
- [ ] Every render test asserts zero callback allocation via `alloc_probe`
- [ ] V1-V8 and D1 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: one `### Session: <date> (session <S>, BE-DSP implementer)` entry: work done, per-kind approximation notes,
state sizing, design differences, hash/intent paths, evidence per row, blockers. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-008)
- **Previous**: vactrol-backend-contracts.md
- **Next**: vactrol-backend-midi.md, vactrol-backend-native.md, vactrol-backend-wasm.md, vactrol-backend-finalize.md
