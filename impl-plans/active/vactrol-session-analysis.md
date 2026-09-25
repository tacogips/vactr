# Vactrol Session Layer: Self-Analysis Surfaces (SS-ANALYSIS) Implementation Plan

**planId**: SS-ANALYSIS (issue #4, wave 2; `scope`/`spectrum`/`capture`/`render`/`rms`/`peak`, sample buffers, native live taps, offline render)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 12.3 (self-analysis amendment), 14.5.1 (tap scope, no `Engine::render`), 14.5.9 (all rules), 14.5.3 (ownership), 14.5.12, 7.1.4 (M1 subject-overload group), 12.7/12.8.8 (`CapabilitySet::require`), 12.8.9 (allocation probe), 11.3 (tick steps); design-music.md "self-analysis and loopback"; design-docs/user-qa/pending-session-questions.md S1, S2
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: SS-CONTRACTS
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

Issue #4 item 4 asks for the prelude surfaces of the 2026-09-25 self-analysis amendment, typed by the checker and
callable from the REPL:
- `scope :bus n`, `spectrum :bus bins:`, `capture :bus cycles` and `render cycles` (native tier; a diagnostic in the
  browser);
- `rms`, `peak`, `spectrum` and `scope` over sample values.

What exists today:
- `spectrum` is a DSP analyzer unit, and `render` is the visual setting (`fn keyword -> nil`, in `vm/natives/tex.rs`).
- `scope`, `capture`, `rms` and `peak` are undefined.
- There is no public `Engine::render`. The offline path is `NativeAudioHost::headless(sr, caps, cells) -> (host,
  AudioSide)` with `AudioSide::render(out, channels)`.
- `CapabilitySet::native()` has `offline_render: false`.

SS-CONTRACTS supplies:
- `Sound::Buffer(Rc<SampleBuf>)` and `SampleSrc::Buffer { id }`;
- `StagedEffect::{Capture, Render}` with stub runtime arms;
- `TapSrc`, `TapReader`, `CaptureId`, `CapturePoll`, and the `AudioHost` default methods;
- `AnalysisCx` and `SourceLoader::analysis`;
- `FailCode::{capture-pending, beyond-capability}`.

## Non-Goals

- No `fft :src` or `amp :src` forms (S2): `fft n` and bare `amp` stay unchanged.
- No browser taps (TASK-010).
- No slot or `:in` taps: they are `beyond-capability` "not available on this host" (S2).
- No `Session` (SESSION registers the real `AnalysisCx`; this plan's tests use a test `SourceLoader`).
- Do not touch `dsp/engine.rs` (786 lines), `dsp/build.rs` or `ns/evaluator.rs`.

## writePaths (exclusive in wave 2)

- `src/value/sample.rs` (fills the CONTRACTS seed)
- `src/types/natives_domain.rs`, `src/types/natives.rs`, `src/types/infer_call.rs`
- Conditional split, ONLY if `infer_call.rs` would reach 800 lines: `src/types/infer_overload.rs` and
  `src/types/mod.rs` (declaration)
- `src/vm/natives/mod.rs`, `src/vm/natives/analysis.rs` (new), `src/vm/natives/tex.rs`
- `src/dsp/mod.rs`, `src/dsp/offline.rs` (new), `src/dsp/caps.rs`, `src/dsp/bus.rs`, `src/dsp/fft.rs`,
  `src/dsp/effects/analyzer.rs` (visibility of helpers only)
- `src/sched/mod.rs`, `src/sched/offline.rs` (new), `src/sched/tap.rs` (new), `src/sched/runtime.rs`,
  `src/sched/commit.rs`
- Conditional split, ONLY if `runtime.rs` or `commit.rs` would reach 800 lines: `src/sched/runtime_effects.rs`
- `src/ns/insts.rs`
- `src/host/native/audio.rs`, `src/host/native/tap.rs` (fills the CONTRACTS stub), `src/host/testing.rs`
- Tests: `src/types/tests/mod.rs`, `src/types/tests/analysis.rs`, `src/vm/tests/mod.rs`, `src/vm/tests/analysis.rs`,
  `src/dsp/tests/mod.rs`, `src/dsp/tests/offline.rs`, `src/sched/tests/mod.rs`, `src/sched/tests/analysis.rs`,
  `src/host/native/tests/mod.rs`, `src/host/native/tests/tap.rs`
- `tests/fixtures/spec/manifest.toml` (design-music ordinal 2 repin ONLY)
- `impl-plans/active/vactrol-session-analysis.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **`value/sample.rs`.**
   - `mono_frames(&self) -> Result<Rc<[f32]>, Failure>`: `(l + r) / 2`.
   - `ready_frames(&self) -> Result<Rc<[f32]>, Failure>`: `Pending` gives `Failure(capture-pending)`, and `Failed`
     gives its own failure.
   - `to_sample_data(&self) -> Result<Arc<SampleData>, Failure>`, for installation.
2. **Checker rows (`natives_domain.rs`, `natives.rs`, `infer_call.rs`), per the 14.5.9 table.**
   - `scope`, `spectrum` and `render` join the M1 fixed subject-overload group (`is_overloaded`). Overloads resolve on
     the first argument's type: keyword → tap form; `sound` → buffer form; number → offline `render`; keyword or no
     argument → visual `render`.
   - The existing DSP `spectrum` (analyzer unit inside `bus`/`inst` bodies) keeps its 12.8.6 rule.
   - New rows:
     - `scope` `fn keyword int -> [float]` | `fn sound int -> [float]`;
     - `spectrum` `fn keyword -> [float]` | `fn sound -> [float]`, keyword `bins`;
     - `capture` `fn keyword any -> sound`, `.effect().needs(&[HostCap::Analysis])`;
     - `render` `fn any -> sound`, effectful, for a number subject;
     - `rms` and `peak` `fn sound -> float`.
3. **`vm/natives/analysis.rs` (registered from `vm/natives/mod.rs`).**
   - The natives reach `AnalysisCx` by downcasting the VM host to `LoaderHost` and calling `.0.analysis()`. When there
     is no context, every analysis native fails `host-unavailable`.
   - `scope :src n` (n in 1..=8192) and `spectrum :src bins: b` (b a power of two in 8..=2048, default 64; otherwise
     `Failure(type)`) read a `TapReader` snapshot. `spectrum` returns `b` linear-bin magnitudes of one FFT of the last
     `2b` samples.
   - `scope buf n` returns the first n mono frames. `spectrum buf bins: b` averages over consecutive `2b` frames.
   - `rms buf` and `peak buf` are computed over both channels, using `dsp::offline`.
   - Sources: `:master` → `TapSrc::Master`; a keyword naming a defined bus → `TapSrc::Bus(kw)`; `:d1..:d9` and `:in` →
     `Failure(beyond-capability)` "not available on this host"; anything else → `Failure(type)`.
   - `capture :src cycles`:
     - `cycles` > 0, and `cycles` × the current cycle length must not exceed `caps.max_capture_seconds`, else
       `beyond-capability`;
     - stages `StagedEffect::Capture { buf: SampleBuf::pending, src, cycles, origin }` and returns
       `Sound::Buffer(buf)`.
   - `render cycles`:
     - when `caps.offline_render` is false (browser, or a build without host-native), fails AT THE CALL with
       `Failure(beyond-capability)` "not available on this host";
     - otherwise stages `StagedEffect::Render { buf, cycles, origin }` and returns the pending buffer.
     - `vm/natives/tex.rs` routes a number subject to this native and keeps the visual behavior for keyword or no
       argument.
4. **`dsp/offline.rs`.** Pure functions over `&[f32]`, reusing `fft.rs` and the analyzer helpers made `pub(crate)` in
   `effects/analyzer.rs`: `rms`, `peak`, `spectrum(frames, bins) -> Vec<f32>`, `scope(frames, n) -> Vec<f32>`.
5. **`dsp/caps.rs`.** `CapabilitySet::native().offline_render = true`. The browser preset is unchanged. Update any
   existing assertion of the native flag in `src/dsp/tests/contracts.rs` ONLY through a new assertion in
   `dsp/tests/offline.rs`. If an existing test contradicts the change, record it as a dependency blocker for FINAL;
   do not edit it.
6. **`dsp/bus.rs`.** A read-only accessor `BusGraph::frames(&self, index) -> (&[f32], &[f32])` for the last rendered
   block, and a name → index lookup if one does not exist.
7. **`host/native/tap.rs` + `host/native/audio.rs`.**
   - Preallocated per-source history rings (8192 mono frames; master plus up to `MAX_TAP_BUSES = 16` named buses)
     behind `Arc` with a seqlock sequence counter.
   - After each `engine.process`, `AudioSide::render` copies the master output and each bus block into its ring. The
     copy is lock-free and allocation-free.
   - `NativeTapReader: TapReader` reads a consistent snapshot, retrying while the sequence is odd or changed.
   - Capture: `arm_capture` posts `(src, start_frame, frames)` to the audio side through a preallocated SPSC slot.
     The audio side streams the source into a preallocated capture ring. `poll_capture` drains it on the evaluator
     side.
   - `NativeAudioHost` implements `tap_reader`, `arm_capture` and `poll_capture`.
8. **`host/testing.rs`.** `RecordingAudioHost` returns a deterministic synthetic `TapReader` (a sine at a fixed
   frequency per source) and completes captures after `frames` worth of `now()` advance.
9. **`sched/tap.rs` + `sched/runtime.rs`.**
   - `Runtime::drain` replaces the CONTRACTS stub arms:
     - `Capture`: arm on the audio host at the next cycle boundary (S1: the next `cycles` cycles), then keep a
       pending-capture list.
     - `Render`: call `sched::offline::render`.
   - `Runtime::tick` polls pending captures, fills the buffer (`SampleBuf::fill`) or fails it (`SampleBuf::fail`).
   - `Runtime::tap_reader()` passes through to the audio host, for SESSION to put into `AnalysisCx`.
10. **`sched/offline.rs`** (native: `all(feature = "host-native", not(target_arch = "wasm32"))`; otherwise the Render
    arm fails the buffer with `beyond-capability`).
    - `render(ev: &mut Evaluator, live: &Runtime, cycles, caps) -> Result<Rc<[f32]>, Failure>` builds
      `NativeAudioHost::headless` and a second `Runtime` with the live resolver and caps.
    - It installs the live instrument and bus graphs and the live ACTIVE slot bindings.
    - It ticks a virtual clock from cycle 0 at the live tempo through 11.3 steps 1-5 ONLY: no `at`/`once` thunk runs
      (a tick option or a separate method in `runtime.rs`).
    - It renders `cycles` cycles and returns interleaved stereo at the headless sample rate.
    - The live runtime, clock and hosts are untouched. The frame count is bounded by `max_capture_seconds`.
11. **`sched/commit.rs` + `ns/insts.rs` (playback).**
    - `route` of `Sound::Buffer(b)` gives `Route::Audio { inst: sampler, sample: Some(SampleSrc::Buffer { id: b.id }) }`
      and records `b` in a per-registry `BTreeMap<u64, Weak<SampleBuf>>`.
    - The commit sample table installs `SampleSrc::Buffer` data from the buffer through `to_sample_data` instead of
      `SampleLoader`.
    - A `Pending` buffer gives an event-local `Failure(capture-pending)`.
12. **`tests/fixtures/spec/manifest.toml`.** Repin ONLY design-music ordinal 2 (lines 89-95 of the block): update
    `check_diags` and `run_fails` to the exact new multisets under the fixture `NoopHost`, which has no `AnalysisCx`,
    so every analysis native is `host-unavailable`. `fft :master` and `amp :d1` keep `type-mismatch`/`type` (S2).
    The `note` is extended accordingly.

## Required Tests

- `types/tests/analysis.rs`:
  - every overload types as in the table;
  - `render :o0` and bare `render` stay visual;
  - `spectrum` inside a `bus` body stays the analyzer unit;
  - `capture` inside a pattern query is `effect-in-pattern`.
- `vm/tests/analysis.rs`, with a test `SourceLoader` whose `analysis()` returns an `AnalysisCx` over the recording
  tap reader:
  - `scope :master 512` returns 512 floats;
  - `spectrum :master bins: 64` returns 64 floats peaking at the synthetic frequency's bin;
  - bad `bins` gives `type`;
  - `:d1` and `:in` give `beyond-capability`;
  - no context gives `host-unavailable`;
  - browser caps: `render 2` gives `beyond-capability` at the call with origin;
  - `rms` and `peak` of a known ready buffer match the closed-form values;
  - reading a pending buffer gives `capture-pending`.
- `dsp/tests/offline.rs`: `rms`/`peak`/`spectrum`/`scope` against closed-form signals; the native preset has
  `offline_render` true.
- `sched/tests/analysis.rs` (real Evaluator + Runtime + recording hosts + `MockClock`):
  - `let hit capture :master 1` is pending until one cycle after the next boundary, then `Ready` with the expected
    frame count;
  - `let out render 2` (native caps, headless) gives a non-silent, finite buffer of 2 cycles of frames for
    `s :analog > note [:a4] > d1`;
  - an `at`/`once` thunk present in the session does NOT run during the render;
  - the live runtime's slot table and clock are unchanged;
  - `s out > once` commits one event whose sample source is `SampleSrc::Buffer` and installs the data;
  - a pending buffer's event fails `capture-pending`.
- `host/native/tests/tap.rs`: `AudioSide::render` under `alloc_probe::armed` counts 0 allocations with taps active and
  a capture armed; the seqlock snapshot equals the last rendered frames; a capture completes with the exact frame
  count.
- The spec fixture runner stays green with the repinned block.

## Invariants

- No allocation, lock or `Rc` traffic on the audio callback (12.8.9).
- Analyzers never feed back: taps are copies.
- Offline render never mutates the live runtime and never runs one-shots.
- Both wasm32 builds stay green: `sched/offline.rs` and `host/native/tap.rs` are gated.
- No `.rs` file reaches 800 lines. Split `infer_call.rs` or `runtime.rs` in this wave if needed, using the listed
  conditional files.

## Edit Protocol

The common protocol in `vactrol-session-contracts.md`. Evidence goes under
`tmp/ss-session-20260925-s183/SS-ANALYSIS/attempt-<n>/`. Sibling-caused crate-wide failures are recorded, never fixed.

## Verification (`<wave>` = `analysis`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| A1 | LOG(`ss-analysis-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/analysis/) \| test(/offline/) \| test(/native::tests::tap/)'` | `exit=0`, run > 0 |
| A2 | `wc -l src/types/infer_call.rs src/sched/runtime.rs src/sched/commit.rs src/vm/natives/tex.rs` | each under 800 |

## Completion Criteria

- [ ] Items 1-12 implemented
- [ ] The surfaces are typed by the checker and callable (VM tests). Live taps, capture and offline render are proven
      with zero callback allocation
- [ ] Design-music ordinal 2 repinned; the fixtures are green
- [ ] V1-V9 and A1-A2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-ANALYSIS implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-contracts.md. **Parallel**: vactrol-session-pkg.md, vactrol-session-directives.md
- **Next**: vactrol-session-core.md (registers the real `AnalysisCx`; REPL reachability)
