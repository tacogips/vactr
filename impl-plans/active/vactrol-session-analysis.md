# Vactrol Session Layer: Self-Analysis Surfaces (SS-ANALYSIS) Implementation Plan

**planId**: SS-ANALYSIS (issue #4, wave 2; `scope`/`spectrum`/`capture`/`render`/`rms`/`peak`, sample buffers, native live taps, offline render)
**Status**: Completed (accepted by the session-185 integration review; SS-FINAL re-verified the joined tree in session 186)
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

- [x] Items 1-12 implemented
- [x] The surfaces are typed by the checker and callable (VM tests). Live taps, capture and offline render are proven
      with zero callback allocation
- [x] Design-music ordinal 2 repinned; the fixtures are green
- [x] V1-V9 and A1-A2 pass with logs cited; `final-hashes.txt` written (session 185: all rows exit 0,
      `attempt-2/final-hashes.txt`)

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-ANALYSIS implementer)` entry. Edit only this log.)

### Session: 2026-09-25 (session 184, SS-ANALYSIS implementer)

**Outcome**: items 1-12 implemented and their tests pass (A1: 31 run / 31 passed). The plan is BLOCKED on two
pre-existing test assertions outside every plan's writePaths that contradict design 14.5.9 (dependency blockers for the
operator / FINAL, per item 5 and the edit protocol; not edited here):
- `src/dsp/tests/contracts.rs:169` (`native_accepts_its_advertised_limits`) asserts
  `native.require(Cap::OfflineRender, None).is_err()`; item 5 sets `CapabilitySet::native().offline_render = true`.
  Resume: change that line to `.is_ok()` (the new `dsp/tests/offline.rs` test asserts the new flag).
- `src/types/tests/natives.rs:78` (`only_scale_and_shape_are_overloaded`) asserts the overload group is exactly
  `["shape", "scale"]`; item 2 adds `scope`, `spectrum` and `render` (table order: `shape`, `scale`, `scope`,
  `spectrum`, `render`). Resume: widen the expected list to those five names.
Every other failure count is zero; after the two amendments V3/V3t are expected to pass unchanged.

**Implementation** (evidence `tmp/ss-session-20260925-s183/SS-ANALYSIS/attempt-1/`):
1. `value/sample.rs`: `rate` is a `Cell` set at fill (`rate()`, `fill_at`), because a capture's rate is the host's;
   `ready_frames` (`capture-pending` / the buffer's own failure), `mono_frames`, `to_sample_data`.
2. Checker: `natives_domain.rs` rows `scope`, `spectrum` (`fn ugen -> ugen` | `fn keyword -> [float]` |
   `fn sound -> [float]`, keywords = `DSP_KEYWORDS` + `bins`), `capture` (effect, needs `Analysis`), `render`
   (`fn keyword -> nil` | `fn any -> sound`), `rms`, `peak`. `infer_call.rs` `overload` resolves keyword, sound, ugen
   and number subjects (a number takes a `float` or `any` first parameter); a call without a positional argument
   takes the first scheme (bare `render` stays visual). 762 lines, no split needed.
3. `vm/natives/analysis.rs`: the natives reach `AnalysisCx` through `LoaderHost` (`host-unavailable` without it).
   Sources `:master` / a defined bus (via `vm.dsp.registry`); `:d1..:d9`, `:in` are `beyond-capability`.
   `vm/natives/mod.rs` registers the DSP natives through a scratch prelude minus `spectrum`, so the one `spectrum`
   native keeps the analyzer-unit meaning (`dsp::collision`) for any non-keyword, non-buffer subject.
   `tex.rs` routes a number subject of `render` to the offline form (browser caps: `beyond-capability` at the call).
4. `dsp/offline.rs`: `rms`, `peak`, `scope`, `spectrum` over slices with `Fft::magnitudes` (the analyzer's window and
   scale); `OFFLINE_RATE` = 48 kHz. `effects/analyzer.rs` and `fft.rs` needed no change.
5. `dsp/caps.rs`: `native().offline_render = true` (browser unchanged).
6. `dsp/bus.rs`: `BusGraph::find(BusId)` and `BusGraph::frames(index, n)` (read-only).
7. `host/native/tap.rs`: seqlocked history rings (master + 16 buses, 8192 frames, relaxed atomics, no `unsafe`), 4
   capture slots with SPSC stereo rings, `NativeTapReader`. `audio.rs`: `AudioSide::render` processes in
   `MAX_BLOCK` pieces and records each block; `NativeAudioHost` implements `tap_reader`/`arm_capture`/
   `poll_capture` and gains `set_bus_names(Rc<dyn InstResolver>)` (bus keyword -> bus id).
8. `host/testing.rs`: `SynthTaps` (3 kHz master, 6 kHz buses, amplitude 0.5 at 48 kHz) and synthetic captures that
   complete once `frames` of mock time have passed.
9. `sched/tap.rs`: `Runtime::tap_reader`, `pending_captures`; `Capture` arms at the next cycle boundary for `cycles`
   cycles at the current tempo (frames at `RuntimeConfig::sample_rate`, new, default 48 kHz); `tick` polls captures
   first (both the normal and frozen paths) and fills or fails the buffer with the call's origin.
10. `sched/offline.rs`: headless `NativeAudioHost` + second `Runtime` over the live resolver/caps/seed, live graphs
    (`InstRegistry::graphs`), live tweak-cell values, live samples (the loader is lent and handed back), ACTIVE
    non-ephemeral pattern bindings; `run_thunks = false` (new `Runtime` field) so step (6) never runs; ticks from
    cycle 0 at the live tempo and renders `MAX_BLOCK` pieces. Non-native builds fail with `beyond-capability`.
11. `ns/insts.rs`: `Sound::Buffer` routes to the sampler with `SampleSrc::Buffer { id }`; `graphs()`.
    `sched/commit.rs`: `SampleTable` keeps `BTreeMap<u64, Weak<SampleBuf>>` (plan-level refinement: the table, not
    the registry, holds the map, because `request` has no resolver) and installs a Ready buffer's own frames; a
    pending or failed buffer fails each event (`capture-pending`) and is never requested; `preload` for the render.
12. `tests/fixtures/spec/manifest.toml`: design-music ordinal 2 only. Check: `rebinding@32`, `type-mismatch@89`,
    `@90`, `@95`, `undefined-name@36`. Run: `arity@95`, `host-unavailable@30/@91/@92/@93/@94`, `not-callable@90`,
    `type@100`, `type@89`, `undefined-name@31/@36`. Line 95 is prose for three alternatives read as one `rms` call.

**Plan-level refinements (recorded for review)**:
- The `capture` length bound (`cycles` x cycle length <= `max_capture_seconds`) is enforced in `Runtime::drain`, where
  the tempo is known (the native has no tempo); the buffer fails `beyond-capability` with the call's origin in the
  same drain the eval pipeline runs per form.
- Native bus taps need `NativeAudioHost::set_bus_names` (the host sees bus ids, not names); SESSION/CLI wiring: call it
  with the session's `InstRegistry` and set `RuntimeConfig::sample_rate` from the native host's clock. Without names
  only `:master` is tapped (a bus tap is `host-unavailable`).
- A buffer `capture` created by the native has rate 0 until filled (`fill_at` sets the host rate).

**Verification** (session 184; `target/fe-logs/ss-analysis-<check>-s184-<n>.log`, each ending in `exit=`):
- V1 build-1 exit=0; V1l build-lsp-1 exit=0; V2 clippy-1 exit=0; V2l clippy-lsp-1 exit=0.
- V3 nextest-1 exit=100 (fail-fast: 85/901 run, 84 passed, 1 failed `native_accepts_its_advertised_limits`);
  nextest-nff-1 (`--no-fail-fast`) exit=100: 901 run, 899 passed, 2 failed (the two blockers above), 1 skipped.
- V3t cargotest-2 (plain `cargo test`) exit=101: lib 887 passed, 2 failed (the same two); cargotest-1 was run with
  `--no-fail-fast`: lib 887 passed / 2 failed, spec_fixtures 10 passed / 1 ignored, other binaries ok.
- V3f fixtures-1 exit=0 (10 run, 10 passed, 1 skipped). A1 own-1 exit=0 (31 run, 31 passed).
- V6a wasm32-1 exit=0; V6b wasm32-hostwasm-1 exit=0; V7 fmt-1 exit=0.
- V4: largest `.rs` 799 (`dsp/build.rs`, pre-existing); A2: `infer_call.rs` 762, `runtime.rs` 776, `commit.rs` 743,
  `tex.rs` 323. V5: `none`. V9: `attempt-1/tree-wasm32*.txt` contain 0 gated crates.
- Shared tree: sibling SS-DIRECTIVES/SS-PKG modules were mid-edit early in the session (missing modules); a sibling
  formatter pass reformatted two of this plan's files (content intact, `notes.md`).

### Session: 2026-09-26 (session 185, SS-ANALYSIS implementer)

**Outcome**: the session-184 blockers are resolved by the operator amendment (checkpoint `9d6db6e`):
`src/dsp/tests/contracts.rs` now asserts `native.require(Cap::OfflineRender, None).is_ok()` and
`src/types/tests/natives.rs` expects `["shape", "scale", "scope", "spectrum", "render"]`. No source edit was needed in
this session: all 31 owned source, test and fixture files are hash-identical to `attempt-1/final-hashes.txt`
(`attempt-2/start-hashes.txt`) and did not change during verification (`attempt-2/final-hashes.txt`). Every
verification row passes. Formal test-integrity, adversarial and integration review are the downstream workflow steps.

**Verification** (session 185; `target/fe-logs/ss-analysis-<check>-s185-1.log`, each ending in `exit=`; evidence
`tmp/ss-session-20260925-s183/SS-ANALYSIS/attempt-2/`):
- V1 build exit=0; V1l build-lsp exit=0; V2 clippy exit=0; V2l clippy-lsp exit=0; V7 fmt exit=0.
- V3 nextest exit=0: 901 run, 901 passed, 1 skipped.
- V3t cargotest (plain `cargo test`) exit=0: lib 889 passed / 0 failed; spec_fixtures 10 passed / 1 ignored; the other
  binaries ok.
- V3f fixtures exit=0: 10 run, 10 passed, 1 skipped. A1 own exit=0: 31 run, 31 passed.
- V6a wasm32 exit=0; V6b wasm32-hostwasm exit=0. V9: `attempt-2/tree-wasm32*.txt` (cargo tree exit 0) contain 0 gated
  crates (tungstenite, getrandom, tower-lsp, tokio).
- A2 (`attempt-2/a2-wc.txt`): `infer_call.rs` 762, `runtime.rs` 776, `commit.rs` 743, `tex.rs` 323, each under 800.
  V4 (`attempt-2/v4-largest.txt`): largest `.rs` 799 (`dsp/build.rs`, pre-existing, not edited). V5: `none`.
- Shared tree: SS-PKG and SS-DIRECTIVES run in the same workspace; the tree was stable across this run (owned hashes
  unchanged), and the crate-wide rows above are the join's baseline for this wave.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-contracts.md. **Parallel**: vactrol-session-pkg.md, vactrol-session-directives.md
- **Next**: vactrol-session-core.md (registers the real `AnalysisCx`; REPL reachability)


### STEP6 OUTPUT NOTE (operator, 2026-09-25, after the SS-ANALYSIS attempt-1 failure)

- The step6-implement output contract requires `changedFiles` to be an ARRAY of
  path strings (SS-ANALYSIS attempt 1 failed with "$.changedFiles must be of type
  array"). Carry `planId`; leave `verificationGaps` empty when every automated
  command passed (manual checks go under `residualRisks`). Crate-wide test
  failures caused only by a sibling branch's in-progress files or by a
  pre-existing test outside every plan's ownership are reported in the
  progress log as a dependency blocker for the operator, never fixed by
  editing unowned files.

### INTEGRATION REVIEW OUTPUT NOTE (operator, 2026-09-26, after two adapter rejections in session 185)

- The integration-review step output MUST be an ENVELOPE with two top-level
  keys: `"when"` (the routing flags `needs_revision`, `redispatch_required`,
  `repair_in_place`, `plans_remaining`) and `"payload"` (an OBJECT holding the
  review itself: `needs_revision`, `loopGate`, `acceptedPlanIds`, `findings`,
  `recoveryDiagnostic`, summaries, evidence paths). Two attempts were rejected
  with "payload must be an object when when is provided" because the review
  fields were emitted at the top level next to `when` instead of inside
  `payload`. `acceptedPlanIds` lists only plans present in the manifest's
  `plans[]`.

### CLOSING NOTE (SS-FINAL, session 186, 2026-09-26)

- Status set to Completed by SS-FINAL. Join integrity: tmp/ss-session-20260925-s183/SS-FINAL/attempt-1/join-integrity.txt.
- Final-tree evidence: target/fe-logs/ss-final-<check>-s186-1.log (build, build-lsp, clippy, clippy-lsp, fmt, nextest 984/984, cargotest 984, fixtures 10/10, lsp-smoke 1/1, cli 9/9, session 132/132, example, wasm32, wasm32-hostwasm; all exit=0).
