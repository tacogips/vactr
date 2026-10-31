# music-things extraction: reusable DSP crates staged in-tree

This document covers moving vactr's synth and effect DSP kernels out of
`src/dsp` into dependency-free, host-agnostic crates. The crates are staged at
`music-things/crates/<crate>/` and vactr depends on them by path.

Status: Proposed (2026-10-31, workflow run on branch `wf/music-things-extract`,
baseline `origin/main` `a6e5bea`).

## 1. Overview

### 1.1 Goal

On 2026-10-31 the user asked for vactr's synth and effect processing to be
made reusable outside vactr ("汎用性を高めよ"), in the new repository
`tacogips/music-things`. That repository has scaffold commit `a72a955`: a
virtual workspace with `members = ["crates/*"]` and an empty facade crate
`crates/music-things`.

This run does the extraction inside vactr, so that vactr's full suite proves
the audio is unchanged. Later the operator, not this run, copies
`music-things/crates/*` into `tacogips/music-things` and switches vactr to a
pinned git dependency. The staged directory therefore has to match the final
repository layout byte for byte.

### 1.2 Scope

In scope:

- Pure DSP code under `src/dsp/ugen/` (kernels), `src/dsp/effects/` (effect
  groups), `src/dsp/effects/prim.rs`, `src/dsp/fft.rs`, `src/dsp/granular.rs`
  and `src/dsp/elements_ominous.rs`.
- The pure tests inside those files.
- The root `Cargo.toml` and `Cargo.lock`.
- `THIRD_PARTY_NOTICES.md` and the new `music-things/THIRD_PARTY_NOTICES.md`.
- One path string in `verification/compare_tides_poly.py`.
- This design doc and the `impl-plans/` for this feature.

Out of scope (unchanged):

- Language semantics, editor UI, session protocol, scheduler and pattern
  code.
- The engine side of `src/dsp`: `graph`, `build`, `controls`, `cells`,
  `caps`, `arena` (apart from the `SampleView` re-export, 4.1), `engine`,
  `bus`, `voice`, `song`, `meta`, `ring`, `offline`, `release`, `ramp`, and
  `ported` (apart from one re-export, 4.4).
- The template and effect catalogs and the codec.
- Every test under `src/dsp/tests/**`, `src/dsp/ported/tests.rs`, `tests/`,
  `examples/` and `editor/`. The only addition is one relocated test file
  (6.3).
- The `mise.toml` tasks. No compare task reads a moved path; see open
  question MTQ1.

### 1.3 Corrections to the Step 1 intake (intentional divergences)

| Intake statement | Repository fact | Design decision |
|------------------|-----------------|-----------------|
| `src/dsp/ported/{braids,clouds,...}.rs` "move as engines" | These 11 files are coverage inventories. They hold revision pins, source paths, vactr voice-template names (`"resonator-voice"`) and `graph::EffectKind` cross-references. They contain no audio code. | All of `src/dsp/ported/` stays in vactr. This is the "ported coverage manifest" that the issue keeps in vactr. Only the plain data types `VoiceRegistration` and `Enveloped` move (4.4). |
| `src/dsp/ramp.rs` is a "pure helper" | It ramps `cells::CellId` values and reads `CellRead`, which are engine types. | `ramp.rs` stays in vactr. |
| 33 files reference `graph`/`CtlId`/`wire::Ctl` | Kernels and effect groups use only `graph::EffectKind`, `graph::AnalyzerKind`, `GranSrc`, `BankRef` and `TableRef`, and the context structs `Kx` and `FxCtx`. The `CtlId`/`Ctl`/`cells` users are `ugen/catalog*`, `ugen/template.rs`, `ugen/build_helpers.rs`, `ugen/mod.rs` and `effects/mod.rs`. Those are engine files and stay. | The adapter seams are the two context traits, the kind mirror and resource resolution (section 4). |

No touched Rust file reaches 1000 lines. The largest are `effects/mod.rs`
(799), `ugen/catalog.rs` (794) and `ugen/mod.rs` (481), and every one of
them shrinks or grows by fewer than 60 lines. So the split rule needs no
action. Any plan that would push a file to 1000 lines or more must split it
under `.agents/skills/rust-coding-standards/SKILL.md`.

## 2. Crate split

### 2.1 Crates and dependency graph

```
music-things-dsp   <-  music-things-synth  <-  music-things-fx
      ^                                              |
      +----------------------------------------------+
vactr  ->  music-things-dsp, music-things-synth, music-things-fx  (path deps)
```

| Directory (`music-things/crates/`) | Package / lib | Contents |
|------------------------------------|---------------|----------|
| `music-things-dsp/` | `music-things-dsp` / `music_things_dsp` | Shared primitives: `prim` (biquad, shapes, one-pole, follower, LFO, delay line, allpass, tanh/soft-clip, gain/pan helpers, `Rng`), `fft` (`Fft`, `FFT_SIZE`), `SampleView`/`SampleSource`/`NoSamples` |
| `music-things-synth/` | `music-things-synth` / `music_things_synth` | Kernel I/O types (`Inp`, `NodeState`, `KernelCtx`, `KernelContext`, `MAX_PORTS`) and every oscillator, filter, envelope, drum, voice and Mutable Instruments-derived synth kernel. Also `elements_ominous`, `voice_layer`, `lpg`, `sample`, `wavetable` |
| `music-things-fx/` | `music-things-fx` / `music_things_fx` | Effect types (`EffectKind`, `AnalyzerKind`, `ParamDef`, `FxState`, `FxStats`, `FxCtx`, `FxContext`, `GrainLimits`), every effect group, `granular`, and the kind dispatch (`params`, `mem_len`, `requires_full_memory`, `init`, `process`, `mix_index`) |

Why fx depends on synth: three fx modules call synth code.

- `quad_mixer` uses `frame_lfo`.
- `elements_bank` uses `elements_ominous`.
- `granular` serves both the granulate effect and the granular ugen, so it
  needs `Inp`, `NodeState` and `KernelCtx`.

Synth never uses fx code. Kernels only use `prim`, so the graph has no cycle.
Granular, texture and Clouds code all sit in fx, so no fourth crate is
needed.

Directory names equal package names. That matches the facade convention
`crates/music-things` and is what the inter-crate `../<dir>` paths need.
The reserved names `music-things-airwindows`, `music-things-soundfont` and
`music-things-pitch` are not used. `music-things/crates/music-things` is not
created.

### 2.2 Crate manifests (self-contained)

Each `music-things/crates/<dir>/Cargo.toml` has only these keys:

- `[package]`:
  - `name`
  - `version = "0.1.0"`
  - `edition = "2021"`
  - `rust-version = "1.83"`
  - `license = "MIT"`
  - `repository = "https://github.com/tacogips/music-things"`
  - a one-line `description`
- `[dependencies]`: only sibling crates, as `music-things-dsp = { path = "../music-things-dsp" }`.

There is:

- no `workspace = true` inheritance,
- no `[workspace]` table,
- no `[profile]` table,
- no `[features]`,
- no external dependency of any kind,
- no `build.rs`.

The crates use `std` only. The APIs in use are `f32` math, `std::array`,
`std::ops::Range`, `std::cmp`, `std::mem`, `std::fmt` and
`std::error::Error`, and all of them are clean on `wasm32-unknown-unknown`.
Each `src/lib.rs` starts with a crate doc comment and `#![warn(missing_docs)]`.

### 2.3 vactr root manifest

- Add `[workspace]`, `members = [".", "music-things/crates/*"]`,
  `resolver = "2"`. Edition 2021 already implies resolver 2 for the root
  package, so feature unification does not change.
- Leave `default-members` unset. Plain `cargo` commands at the root keep
  targeting only `vactr`. The acceptance commands pass `--workspace`
  explicitly.
- Under `[dependencies]`, add path dependencies on all three crates with no
  features. `host-native`, `host-wasm` and `lsp` are unchanged.
- `Cargo.lock` gains three path packages and nothing else.
- The target directory stays `<root>/target`, so the editor's wasm path
  `target/wasm32-unknown-unknown/debug/deps/vactr.wasm` is unchanged.

## 3. Public API per crate

Every public item has a doc comment. The process path never allocates,
locks or panics. Memory is caller-provided (`mem: &mut [f32]`, sized by
the `*_STATE_FLOATS`/`mem_len` functions). Audio is `f32` and the sample
rate is always explicit (`sr` in `KernelCtx` and `FxCtx`, or an argument).

### 3.1 music-things-dsp

| Item | Origin | Notes |
|------|--------|-------|
| `pub mod prim` | `src/dsp/effects/prim.rs` (moved verbatim) | `Biquad`, `Shape`, `OnePole`, `Follower`, `Lfo`, `DelayLine`, `allpass`, `tanh`, `soft_clip`, `clampf`, `db_to_gain`, `gain_to_db`, `pan_gains`, `balance_gains`, `Rng` |
| `pub mod fft` | `src/dsp/fft.rs` (moved verbatim) | `Fft::new` allocates its tables once at setup; `forward`/`inverse` never allocate |
| `pub struct SampleView<'a> { pub data: &'a [f32], pub channels: u8, pub rate: u32 }` and `fn frames()` | `src/dsp/arena.rs` lines 61-76 (moved) | Interleaved, read-only audio |
| `pub trait SampleSource { fn sample(&self, id: u32) -> Option<SampleView<'_>>; }` | new | Resource lookup for convolution impulse responses |
| `pub struct NoSamples;` implementing `SampleSource` (always `None`) | new | Lets standalone hosts and moved tests build an `FxCtx` |

### 3.2 music-things-synth

Core types (in `src/lib.rs`, moved from `src/dsp/ugen/mod.rs` and
`ugen/catalog.rs`):

- `pub const MAX_PORTS: usize = 48;`
- `pub enum Inp<'a> { Buf(&'a [f32]), Val(f32) }`, with `at` and `first`.
- `pub struct NodeState { pub s: [f32; 8], pub bq: [Biquad; 2], pub dl: DelayLine, pub u: [u32; 4] }`,
  with `finish` and `done`.
- `pub struct KernelCtx { pub sr: f32, pub gate: usize, pub seed: u32 }`
  (Copy). `gate` is the number of frames in this block during which the
  gate is open.
- `pub trait KernelContext { fn kernel_ctx(&self) -> KernelCtx; }`,
  implemented for `KernelCtx` (returns `*self`).

Kernel entry points keep their current names, argument order and port
tables (`PORTS`, `PORT_COUNT`, `port::*`, `*_STATE_FLOATS`). The one change
is the context parameter. A public function that took `kx: &Kx<'_>` (or
`&mut Kx<'_>`) becomes generic `<C: KernelContext + ?Sized>(…, kx: &C)`. Its
first statement is `let kx = &kx.kernel_ctx();`, and the body is otherwise
unchanged, because the fields `sr`, `gate` and `seed` keep their names.
Private helpers take `&KernelCtx` concretely.

Modules (destination `music-things/crates/music-things-synth/src/<file>`;
same file names unless noted):

- Classic and FM: `osc`, `filter`, `env`, `fm` (+ `fm/{algorithms,engine,envelope,patch,scaling,sysex}.rs`),
  `additive`, `musicdsp_synth` (+ `musicdsp_synth/chaos.rs`), `va_filter`, `phase_pair`,
  `analog_percussion`, `feedback_metal`, `fm_drum`, `fusion_drum`, `sample`, `wavetable`.
- Plaits-derived pairs and voice layer:
  - `analog_pair`, `chip_pair`, `chord_pair`, `clock_noise_pair`, `dual_kick` (+ `dual_kick/{analog,synthetic}.rs`),
  - `fm_pair`, `grain_pair`, `hat_pair` (+ `hat_pair/{ring,square}.rs`), `modal_pair`, `particle_pair`,
  - `shape_pair`, `snare_pair` (+ `snare_pair/{analog,synthetic}.rs`), `spectrum_pair`, `string_machine_pair`,
  - `string_pair`, `swarm_pair`, `table_terrain_pair`, `terrain_pair`, `six_op_original`, `speech_original`,
  - `number_station`, `voice_layer` (+ `voice_layer/tests.rs`), and `lpg` (from `vactrol_gate.rs`, renamed).
- Braids: `braids_cloud`, `braids_digital`, `braids_filter`, `braids_five`, `braids_fm`,
  `braids_formant`, `braids_noise`, `braids_percussion`, `braids_physical`, `braids_struck`,
  `braids_subsync`, `braids_triple`, `braids_wave_bank`, `braids_wave_line`.
- Other Mutable Instruments roles: `rings_part`, `string_choir`, `elements_internal`, `elements_ominous`
  (from `src/dsp/elements_ominous.rs`), `tidal_function`, `tidal_poly`, `peak_function`, `peak_pulse`,
  `stage_segment`, `stage_chain`, `stage_linked`, `frame_lfo`, `frame_keyframe`.
- Voice cores: `bass_voice` (+ `bass_voice/{diode,fm,ladder,mods,osc}.rs`), `digital_drum`
  (+ `digital_drum/{mod,chain,metal,tonal}.rs`), `kalimba`, `tonewheel`, `hurdy_gurdy`
  (+ `hurdy_gurdy/{bowed,buzz}.rs`), `vosim`, `gendyn`, `scanned`.

Kernels that read resources take an already-resolved view. They never see
bank or table ids, or `SampleStore`:

- `sample::play_view(view: Option<SampleView<'_>>, ins, st, out, kx: &C)`
  and `sample::play_stereo_view(view, ins, st, mono, left, right, kx: &C)`.
  These are the bodies of `play_worker` from the resolved view onward. The
  `None` branch (zero outputs, `st.finish()`) is inside the crate function.
  `frame_region` and `region` are unchanged.
- `wavetable::render_view(view: Option<SampleView<'_>>, ins, st, out, kx: &C)`.
  The `None` branch is in the crate. `FRAME` is unchanged.

`lpg` (Plaits low-pass gate and decay modulation):

- `pub struct VoiceRegistration { pub enveloped: Enveloped, pub out_gain: f32, pub aux_gain: f32 }`
  with `gain` and `is_enveloped`, and `pub enum Enveloped { Never, Always, WhenClocked }`.
  Both move verbatim from `src/dsp/ported/manifest.rs`.
- Each entry that reads a registration takes an extra trailing argument
  `registry: fn(usize) -> Option<VoiceRegistration>`. The
  `VOICE_REGISTRATIONS` table and `plaits_voice` stay in vactr.

### 3.3 music-things-fx

Core types (`src/lib.rs` plus `src/kind.rs`, `src/ctx.rs`):

- `pub enum EffectKind` and `pub enum AnalyzerKind`, deriving
  `Clone, Copy, PartialEq, Eq, Hash, Debug`.
  - The variants mirror `vactr::dsp::graph::{EffectKind, AnalyzerKind}` one
    for one, with one rename: `StreamVactr` becomes `StreamOpto`.
  - There are no catalog names, no `ALL` order and no `from_name`. Those
    stay in vactr as the language catalog and codec.
- `ParamDef` (with `new`, `unit`, `clamp`), `FxState` and `FxStats`, moved
  verbatim from `src/dsp/effects/mod.rs`.
- `pub struct GrainLimits { pub max_grain_density: f32, pub max_grain_size: f32, pub max_capture_seconds: f32 }`
  (Copy). These are the three tier caps that granular reads.
- `pub struct FxCtx<'a>` with these fields:
  - `pub sr: f32`
  - `pub samples: &'a dyn SampleSource`
  - `pub fft: &'a Fft`
  - `pub limits: GrainLimits`
  - `pub scratch: &'a mut [f32]`
  - `pub analysis: &'a mut [f32]`
  - `pub stats: &'a mut FxStats`
- `pub trait FxContext { fn fx_ctx(&mut self) -> FxCtx<'_>; }`, implemented
  for `FxCtx` (field-wise reborrow).

Group modules (destination `music-things/crates/music-things-fx/src/<file>`,
same names):

- `dynamics`, `eq`, `music_filters`, `musicdsp`, `resonator`, `resonator_extra`, `restoration`, `utility`, `saturation`
- `delay`, `reverb`, `creative_reverb` (+ `creative_reverb/{common,diffuse}.rs`), `creative_delay`, `modulation`,
  `spatial`, `lofi`, `composite_lofi`
- `cross_mod` (+ the 9 `cross_mod_*.rs` files at the same flat level, `#[path]` attributes unchanged),
  `resonant_bank`, `elements_bank`, `shift_pair`, `quad_mixer`
- `texture`, `texture_quality`, `texture_loop`, `texture_stretch`, `texture_spectral`, `granular`
  (from `src/dsp/granular.rs`), `dynamic_control`, `stream_dynamics`, `stream_cv`, `analyzer`

Signature rules for group modules:

- A public function that takes `ctx: &mut FxCtx<'_>` becomes
  `<C: FxContext + ?Sized>(…, ctx: &mut C)`. Its first statement is
  `let ctx = &mut ctx.fx_ctx();`.
- A public function that takes `kind: EffectKind` takes
  `kind: impl Into<EffectKind>`. Its first statement is
  `let kind: EffectKind = kind.into();`.
- Each item that the vactr dispatch, `FxUnit`, the engine, tests or
  examples reach becomes `pub`, with a doc comment. That includes the
  `params`, `mem_len`, `init` and `process` of every group, the
  `composite_lofi` and `texture_quality` modules, `texture_quality::mono`
  and `dynamics::compress`.
- In convolution (`reverb`), `ctx.store.get(resource)` becomes
  `ctx.samples.sample(resource)`.
- `granular` takes `&GrainLimits` wherever it took `&CapabilitySet`. The
  functions affected are `pool_slots`, `capture_len`, `ugen_mem_len`,
  `effect_mem_len` and `effect_init`; `effect_process` reads `ctx.limits`.
- `granular::ugen_process(view: SampleView<'_>, ins, st, mem, out, kx: &C, limits: &GrainLimits, stats: &mut FxStats)`
  is the old body from `Ctl::read` onward.

Kind dispatch (`src/dispatch.rs`, wave W4, MTX-90): `group`, `params`, `mem_len(kind, sr, &GrainLimits)`,
`requires_full_memory`, `init(kind, st, mem, sr, &GrainLimits)`,
`process<C: FxContext + ?Sized>(kind, p, st, mem, l, r, ctx)` and
`mix_index`. They move verbatim from `src/dsp/effects/mod.rs`, with
`&CapabilitySet` replaced by `&GrainLimits`.

### 3.4 Parameter setters

Kernels and effects keep their positional parameter inputs (`[Inp; MAX_PORTS]`
and `p: &[f32]`), so the processing code and vactr's callers stay unchanged.
Two small types, added in W0, give typed setters keyed by the existing
constants:

- `music_things_synth::PortValues<'a>`:
  - `new()`: all `Inp::Val(0.0)`
  - `from_defaults(&[(&str, f32)])`: fills from a kernel `PORTS` table
  - `set(index: usize, value: f32)`
  - `set_buffer(index: usize, buf: &'a [f32])`
  - `inputs(&self) -> &[Inp<'a>; MAX_PORTS]`
  - Out-of-range indices are ignored.
  - Example: `PortValues::from_defaults(&kalimba::PORTS).set(kalimba::port::KALIMBA_DECAY, 2.5)`.
- `music_things_fx::ParamSet`:
  - `new(defs: &'static [ParamDef])`: holds the defaults in a fixed
    `[f32; MAX_PARAMS]`, with `pub const MAX_PARAMS: usize = 28`. The
    table comes from `params(kind)` or a group's `params`. It takes a
    table rather than a kind because the kind dispatch only arrives in W4
    (MTX-90).
  - `set(index, value)` clamps with `ParamDef::clamp`, the same rule as
    vactr's `FxUnit::set`.
  - `set_named(name, value) -> bool`
  - `values(&self) -> &[f32]`, which is passed as `p`.

Neither type allocates. vactr does not use them; `FxUnit` keeps its own
storage. Each has crate unit tests: defaults, clamping, NaN to default, and
out-of-range indices.

Each kernel entry point's doc comment lists its input port indices and
meanings. For classic ugens that have no `PORTS` table, these are copied
from vactr's `ugen/catalog/voice_ports.rs` and catalog rows. Each
effect's parameters are described by its `ParamDef` table. Per-parameter
named methods (for example `set_cutoff`) for all ~220 kernels and kinds are
not added; see MTQ3.

### 3.5 Kept in vactr

Parameter smoothing, the `mix` blend, sidechain-bus resolution and cell
sources stay in vactr's `FxUnit`. Crate users get fully wet `process`
output and blend `mix` themselves. A crate-side unit wrapper is a non-goal.

## 4. vactr adapter boundary

vactr keeps every module path used by engine code, tests and examples, for
example `crate::dsp::ugen::kalimba`, `crate::dsp::effects::texture`,
`crate::dsp::effects::prim`, `crate::dsp::fft` and
`vactr::dsp::ugen::voice_layer`. Each path becomes either a
`pub use music_things_*::<module>;` re-export or a thin adapter module. So
`src/dsp/tests/**`, `examples/*.rs` and engine call sites
(`voice.rs:540/561/575`, `build.rs:851-868`, `graph.rs:147-229`) compile
with no edits.

### 4.1 Context traits (why generics)

61 vactr test files build `ugen::Kx { sr, gate, bank, store, caps, stats, seed }`
by struct literal. 17 build `effects::FxCtx { sr, store, fft, caps, scratch, analysis, stats }`.
They call kernel and group functions directly. A concrete crate context type
would force edits to all of them. Instead:

- `src/dsp/ugen/mod.rs` keeps `Kx` unchanged and adds
  `impl music_things_synth::KernelContext for Kx<'_>`, copying `sr`, `gate`
  and `seed`.
- `src/dsp/effects/mod.rs` keeps `FxCtx` unchanged. The new
  `src/dsp/effects/fx_adapter.rs` holds:
  - `impl music_things_fx::FxContext for FxCtx<'_>`, where
    `samples = self.store`, `limits = grain_limits(self.caps)` and the
    other fields are reborrowed;
  - `pub fn grain_limits(&CapabilitySet) -> GrainLimits`;
  - the kind conversions (4.2).
- `src/dsp/arena.rs` replaces its `SampleView` definition with
  `pub use music_things_dsp::SampleView;` and adds
  `impl music_things_dsp::SampleSource for SampleStore`, delegating to
  `get`.
- Dispatch call sites (`ugen/mixer.rs`, `voice.rs`) pass `kx` as before.
  Reborrow coercion `&mut Kx -> &C` infers `C = Kx`. If a site does not
  compile, write `&*kx` at that site; nothing else changes.
- **Fn-pointer wrappers (added 2026-10-31 during Step 4 planning).** Some
  vactr callers coerce kernel functions to higher-ranked fn pointers over
  `&Kx<'a>`:
  - `src/dsp/tests/dsp/kernel_pairs.rs:16-20`
  - `src/dsp/tests/dsp/digital_drum.rs:75`
  - `src/dsp/tests/dsp/digital_metal.rs:76`
  - `examples/plaits_drums_reference.rs:32`

  A generic `fn render<C>` cannot be coerced to such a pointer, because
  type parameters are early-bound. For each of the 27 entry points below,
  vactr therefore keeps a forwarding wrapper with the exact original
  signature (`kx: &Kx<'_>`). The wrapper lives in a vactr shim module
  (`pub use music_things_synth::<m>::*;`, with the local wrapper `fn`
  shadowing the glob). Its body is one call to the crate function, with no
  DSP.
  - `va_filter::{filter, filter_pair}`
  - `fm_pair`, `analog_pair`, `chord_pair`, `table_terrain_pair`,
    `terrain_pair`, `string_machine_pair`, `shape_pair` and `stage_chain`:
    `{render, render_pair}` each
  - `digital_drum::{render_drum, render_snare}` and
    `digital_drum::metal::{render_metal, render_hat}`
  - `dual_kick::render`, `snare_pair::render` and `hat_pair::render`
  - `sample::{play, play_stereo}` are already adapters (4.3).

  Effect-group functions are never used as fn pointers, so no fx wrapper
  is needed.

### 4.2 Kind mirror

`fx_adapter.rs` holds exhaustive
`impl From<graph::EffectKind> for music_things_fx::EffectKind` and
`impl From<graph::AnalyzerKind> for music_things_fx::AnalyzerKind`. Each
variant maps to the variant of the same name, except
`StreamVactr -> StreamOpto`. Adding a vactr variant without a crate
counterpart fails to compile.

A new vactr test, `src/dsp/tests/dsp/fx_kind_map.rs`, checks
`format!("{:?}", fx::EffectKind::from(k)) == format!("{:?}", k)` for every
`graph::EffectKind::ALL` entry, with the one rename excepted.

### 4.3 vactr residue per area

| vactr path | After extraction |
|------------|------------------|
| `src/dsp/mod.rs` | `pub mod fft;` -> `pub use music_things_dsp::fft;`; `pub mod elements_ominous;` -> `pub use music_things_synth::elements_ominous;`; `pub mod granular;` stays (adapter) |
| `src/dsp/ugen/mod.rs` | Keeps `Node`, `Src`, `NodeSpec`, `Kx`, `BuildError`, the `MAX_*` limits, `AMP`/`PAN`/`BANK`, `NO_FM6_PATCH` and the `template` re-exports. `Inp`/`NodeState`/`MAX_PORTS` re-exported from synth. Each moved kernel: `pub mod x;` -> `pub use music_things_synth::x;` |
| `src/dsp/ugen/catalog.rs`, `catalog/`, `build_helpers.rs`, `template.rs`, `mixer.rs` | Stay. `catalog.rs` `MAX_PORTS` becomes a re-export. Other edits only if a moved item needs a new path |
| `src/dsp/ugen/sample.rs`, `wavetable.rs` | Adapters. They keep their current signatures. They resolve `kx.bank.unwrap_or(ref.get())` through `kx.store.get`, then call `play_view`/`play_stereo_view`/`render_view`, and re-export `frame_region`, `region` and `FRAME` |
| `src/dsp/ugen/vactrol_gate.rs` | Adapter. Re-exports `music_things_synth::lpg` items, including `GATE_STATE_FLOATS` and `DECAY_MOD_STATE_FLOATS`. Wraps `render_gate`/`render_decay_mod` with their current signatures, passing `registry = crate::dsp::ported::plaits_voice` |
| `src/dsp/ugen/{va_filter,fm_pair,analog_pair,chord_pair,table_terrain_pair,terrain_pair,string_machine_pair,shape_pair,stage_chain,dual_kick,snare_pair,hat_pair}.rs`, `src/dsp/ugen/digital_drum/mod.rs` | Fn-pointer shims (4.1). `pub use music_things_synth::<m>::*;` plus the listed wrappers with the original `&Kx<'_>` signature. `digital_drum/mod.rs` also gets an inline `pub mod metal` with `render_metal`/`render_hat` wrappers. The submodule files (`dual_kick/*`, `snare_pair/*`, `hat_pair/*`, `digital_drum/{chain,metal,tonal}.rs`) move to the crate |
| `src/dsp/ugen/voice_layer.rs` | Shim: `pub use music_things_synth::voice_layer::*;` and `pub use music_things_synth::voice_layer::OptoEnvelope as VactrolEnvelope;`, because `examples/plaits_voice_reference.rs` imports `VactrolEnvelope` |
| `src/dsp/granular.rs` | Adapter, under 80 lines. Re-exports `PARAMS` and `GRAIN_FLOATS`. Keeps the `&CapabilitySet` signatures of `pool_slots`, `capture_len`, `ugen_mem_len`, `effect_mem_len` and `effect_init` via `grain_limits`. `effect_process` forwards to the crate. `ugen_process(src: GranSrc, …, kx: &mut Kx)` keeps the current resolution order (`GranSrc::Bus` -> zeros; missing resource -> zeros; then crate `ugen_process` with `grain_limits(kx.caps)` and `kx.stats`) |
| `src/dsp/effects/mod.rs` | Keeps `SIDECHAIN_BUS_CTL`, `MAX_SIDECHAIN_BUS`, `EFFECT_PARAM_BASE`, `MAX_FX_PARAMS`, `sidechain_bus`, `FxCtx`, `param_ctl`, `param_index`, `ir_is` and `FxUnit`. `ParamDef`/`FxState`/`FxStats`/`prim` are re-exported. Each group is a module re-export. After W4 (MTX-90), `params`/`mem_len`/`requires_full_memory`/`init`/`process`/`mix_index` are vactr wrappers that take `graph::EffectKind` (and `&CapabilitySet`) and call the crate dispatch with `kind.into()` (and `grain_limits(caps)`) |
| `src/dsp/effects/catalog.rs` | Stays |
| `src/dsp/effects/fx_adapter.rs` | New (4.1, 4.2) |
| `src/dsp/ported/manifest.rs` | `VoiceRegistration`/`Enveloped` definitions become `pub use music_things_synth::lpg::{Enveloped, VoiceRegistration};`. The table stays |

### 4.4 Generality rules for crate code

Inside `music-things/`, the case-insensitive grep
`grep -rniE 'crate::|vactr|CtlId|InstDef|wire::Ctl' music-things/` must
return nothing. This covers code, comments, tests and the staged notices.
It follows that:

- **Intra-crate paths.** Use `super::` (or `super::super::`) and sibling
  crate names. Never use `crate::`, and never use `$crate` macros.
  `pub(crate)` is allowed.
- **Renames.**
  - `vactrol_gate` -> `lpg`
  - `VactrolEnvelope` -> `OptoEnvelope`
  - identifiers containing `VACTROL` -> `OPTO`
  - `dynamic_control::vactr` -> `opto`
  - `EffectKind::StreamVactr` -> `StreamOpto`
  - Comments that say "Vactr" are reworded to "this implementation" or
    "original". The provenance claim (original versus adapted, and what
    was not imported) must stay the same.
- **References that may stay.** Port names, parameter names and other
  strings are unchanged. None of them contains a banned token.
  vactr design-section references in comments (for example "design
  12.8.8") may stay; MTQ2 records the clean-up.

## 5. Code-move rules (bit identity)

Code moves verbatim. Implementers cannot write `.git`, so a move means:
copy the content to the destination, apply the allowed edits, then delete
the source. Git's rename detection keeps history at commit time. Rust performs no
FMA contraction or fast-math, so inlining and crate boundaries cannot change
`f32` results. Only text edits can. A plan may make these edits and no
others:

1. `use` paths and module declarations.
2. The context prologue and generic signature (3.2, 3.3) and the kind
   prologue.
3. Visibility widening (`pub(super)`/`pub(crate)` -> `pub`) for items
   vactr reaches, plus their doc comments.
4. Doc comments added or reworded, including the vactr-term rewording in
   4.4.
5. The identifier renames in 4.4.
6. The `FxCtx` field changes (`store` -> `samples.sample`, `caps` ->
   `limits`).
7. The resource-resolution split for `sample`, `wavetable` and the
   granular ugen.
8. The `lpg` registry argument.
9. In moved tests, imports and context construction only. For example,
   `Kx {..}` becomes `KernelCtx { sr, gate, seed }`, and `FxCtx {..}` uses
   `NoSamples`, `GrainLimits` and crate `FxStats`.

Forbidden:

- reordering arithmetic or loop order;
- changing literal types or `f32`/`f64` promotion;
- introducing `mul_add`;
- replacing one helper with another;
- changing defaults, ranges, port order, `*_STATE_FLOATS` or `mem_len`
  values;
- editing any assertion, expected value, tolerance, golden file or
  fixture.

## 6. Tests

### 6.1 Placement

- In-file `#[cfg(test)]` items, and `voice_layer/tests.rs`, move with
  their module into the crate. At baseline, 28 of the moved files carry
  `#[cfg(test)]` items.
- `src/dsp/tests/**`, `src/dsp/ported/tests.rs`, root `tests/`,
  `examples/` and `editor/` stay byte-identical.

### 6.2 Count

`cargo nextest run --workspace` runs vactr and crate tests together. The
total must be at least 3093 plus the new `fx_kind_map` test. Evidence comes
from `cargo nextest list --workspace` before (W0 start) and after each wave.

Every baseline test name must appear after the move, matched by its
`<module>::tests::<name>` suffix. A test may only move between binaries;
none may be dropped.

### 6.3 Relocated test

`effects/quad_mixer.rs::tests::supports_rates_and_arbitrary_block_sizes_without_allocations`
uses vactr's test-only `alloc_probe`. It moves unchanged into the new file
`src/dsp/tests/dsp/quad_mixer_alloc.rs`, registered in
`src/dsp/tests/dsp.rs`. It calls `crate::dsp::effects::quad_mixer::{process, PARAMS}`
through the re-export. The other `quad_mixer` tests move into the crate.

## 7. Third-party notices

- **New file `music-things/THIRD_PARTY_NOTICES.md`.**
  - It copies every section of `THIRD_PARTY_NOTICES.md` whose files move:
    the Streams, Braids, Clouds, Plaits, Peaks, percussion, Warps, Tides,
    Elements, Rings, Stages and Frames sections, msfa (Apache-2.0, full
    license text) and the original-voices statement. It also includes the
    Mutable Instruments MIT permission text.
  - It adds a short "musicdsp research references" note. This mirrors
    `design-musicdsp-effects.md`: audit reference `bdejong/musicdsp`
    `d7c8525c`, ideas implemented independently, no snippet copied.
  - Paths are rewritten to `crates/music-things-<x>/src/<file>`, relative
    to the music-things repository root.
  - Wording is scrubbed under 4.4. The section about the local
    source-comparison probe stays vactr-only.
- **vactr `THIRD_PARTY_NOTICES.md`.** Every `src/dsp/...` path of a moved
  file (33 references) is rewritten to its
  `music-things/crates/<dir>/src/...` path. One sentence is added saying
  the DSP sources now live under `music-things/crates` and naming the
  staged notices file. No section is removed.
- **Verification script.** `verification/compare_tides_poly.py:444`
  changes its informational path to
  `music-things/crates/music-things-synth/src/tidal_poly.rs`. The quoted
  line numbers are refreshed if the move shifts them.
  `compare_braids_shapes.py` refers to `src/dsp/ported/braids.rs`, which
  stays, so it is unchanged.

## 8. Waves and plan partition

vactr shares one test crate, so a compile error anywhere blocks every plan.
Every plan therefore leaves `cargo check --workspace --all-targets` green
at each handoff. Each wave is accepted only after the full gate set (9.1)
passes.

Revised 2026-10-31 in Step 4. Concurrent plans never share a file. Every
synth move edits `music-things-synth/src/lib.rs` and `src/dsp/ugen/mod.rs`,
and every fx move edits `music-things-fx/src/lib.rs` and
`src/dsp/effects/mod.rs`. So each wave pairs at most one synth plan with
one fx plan, and the two chains run side by side.

The paired plans are still build-coupled (fx -> synth, and vactr -> both),
so every move plan works under two rules from
`impl-plans/active/music-things-protocol.md`:

- **One module at a time.** Each module is moved, wired and checked with
  `cargo check -p <crate>` and `cargo check -p vactr` before the next one
  starts (protocol section 2.1).
- **Sibling waits.** A failure located only in the sibling's paths waits
  for the sibling's `EDITS-COMPLETE` marker. A blocker is recorded only
  when the sibling's files have been stable for 15 minutes, or when a
  120-minute cap is reached (protocol section 5.1).

| Wave | Plan | Moves / creates |
|------|------|-----------------|
| W0 | MTX-00 foundation | Root `Cargo.toml` workspace and path deps, plus `Cargo.lock`. Three crate manifests and `lib.rs` files. dsp: `prim.rs`, `fft.rs` and the sample-view module. synth core types and `PortValues`. fx core types (`kind.rs`, `ctx.rs`, `ParamDef`/`FxState`/`FxStats`) and `ParamSet` with its tests (3.4). vactr `fx_adapter.rs`, the `KernelContext` impl, the `arena.rs` re-export and `impl SampleSource`, `effects/mod.rs`/`ugen/mod.rs`/`catalog.rs`/`dsp/mod.rs` re-exports, the `fx_kind_map` test and the baseline test list |
| W1 | MTX-S1 classic, FM & MI roles | classic and FM list plus other-Mutable-Instruments list (3.2), with `sample`/`wavetable` adapters, `va_filter`/`stage_chain` shims and `elements_ominous` (`src/dsp/mod.rs`) |
| W1 | MTX-F1 dynamics, eq, filters, time, space, lofi | `dynamics`, `eq`, `music_filters`, `musicdsp`, `resonator`, `resonator_extra`, `restoration`, `utility`, `saturation`, `delay`, `reverb`, `creative_reverb/*`, `creative_delay`, `modulation`, `spatial`, `lofi`, `composite_lofi` |
| W2 | MTX-S2 Plaits pairs & LPG | the pairs and voice-layer list (3.2), `lpg` and its registry; `vactrol_gate.rs` adapter, `voice_layer.rs` shim, fn-pointer shims, `ported/manifest.rs` re-export |
| W2 | MTX-F2 texture, granular, stream, analyzer | `texture*` (5 files), `granular` (vactr adapter at the same path), `dynamic_control`, `stream_dynamics`, `stream_cv`, `analyzer` |
| W3 | MTX-S3 Braids & voice cores | the 14 `braids_*` files and the voice-core list (3.2), with the `digital_drum` shim |
| W3 | MTX-F3 cross-mod & banks | `cross_mod` and its 9 `cross_mod_*` files, `resonant_bank`, `elements_bank`, `shift_pair`, `quad_mixer`, and the relocated `quad_mixer_alloc` test (needs `frame_lfo`/`elements_ominous` from MTX-S1) |
| W4 | MTX-90 fx dispatch | crate `dispatch.rs`; vactr wrappers in `effects/mod.rs` |
| W5 | MTX-91 notices, paths & final drop-in (after MTX-90, because the drop-in copy must see the final fx crate) | `music-things/THIRD_PARTY_NOTICES.md`, vactr `THIRD_PARTY_NOTICES.md`, `verification/compare_tides_poly.py`, gates 9 and 10 |

Plans must also meet these constraints:

- **writePaths and sharedPaths.** Plans list concrete file paths: both the
  source and the destination of each move, and never whole directories.
  The largest plan has fewer than 120 entries, well under the 512 cap.
- **artifactRoots.** Every plan declares `target/`,
  `tree-sitter-vact/tree-sitter-vact.wasm`, `editor/node_modules` and its
  own `tmp/music-things-extraction/<plan-id>/`.
- **Docs and generality in each plan.** Each plan documents its own public
  items and passes the 4.4 grep for its own files before handoff, so
  clippy never sees accumulated `missing_docs` warnings.

## 9. Verification

### 9.1 Gate set

Run before every wave acceptance, with
`CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1`:

1. `cargo nextest run --workspace`: passes, with the count from 6.2.
2. `cargo clippy --workspace --all-targets -- -D warnings`: passes.
3. `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`.
4. For each crate `c` in `music-things/crates/*`: `cargo build --manifest-path $c/Cargo.toml --target wasm32-unknown-unknown`, and `cargo build -p <package> --target wasm32-unknown-unknown`.
5. `(cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run)`: at least 2384 passing. This may run outside the Codex sandbox, with evidence reported.
6. `cargo build --examples` (all 15 examples compile unchanged), and `cargo build --features lsp`.
7. `grep -rniE 'crate::|vactr|CtlId|InstDef|wire::Ctl' music-things/` returns nothing.
8. `git diff --stat a6e5bea -- src/dsp/tests tests examples editor src/dsp/ported/tests.rs` is empty, except the two lines and file from 6.3 and the `fx_kind_map` test.

The final wave (MTX-91, W5) adds two gates:

9. **No duplication.** None of the moved file names in sections 3.2 and
   3.3 exists under `src/dsp` except the adapters listed in 4.3. The
   adapter files contain no DSP loops (a review check).
10. **Standalone drop-in.** Copy `music-things/crates/*` into
    `tmp/music-things-extraction/MTX-91/standalone/crates/`, next to a
    virtual workspace `Cargo.toml` (`members = ["crates/*"]`,
    `resolver = "2"`). Then
    `cargo build --workspace --manifest-path …/standalone/Cargo.toml` and
    the same with `--target wasm32-unknown-unknown`, plus
    `cargo test --workspace --manifest-path …`, all pass.

### 9.2 Evidence format

Verification records use
`{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
Each `log` is a complete log path under the plan's
`tmp/music-things-extraction/<plan-id>/`. Intentional negative controls
go only in `mutationEvidence`. Example: a deliberately swapped kind
mapping must fail `fx_kind_map` and golden tests.

## 10. Acceptance mapping

| Acceptance criterion | Design element |
|----------------------|----------------|
| Crates in `music-things/crates/*`, native and wasm builds | 2.1, 2.2, gates 4 and 10 |
| No duplicated DSP in `src/dsp`; nodes and FX wired through the crates | 4.3, gate 9 |
| No vactr references inside `music-things/` | 4.4, gate 7 |
| nextest of at least 3093 with no test dropped | 6, gate 1 |
| wasm build and vitest of at least 2384 | 2.3, gates 3 and 5 |
| clippy with no new warnings | 2.2 (`missing_docs`), gate 2 |
| Workspace manifest, resolver 2, path deps, features intact | 2.3, gate 6 |
| Self-contained crate manifests | 2.2 |
| Plain APIs (explicit sample rate, `f32`, block process, typed setters, docs, no process-path allocation) | 3.1-3.4, 2.2 (`missing_docs`) |
| Notices | 7 |
| Design doc and plans | this document, section 8 |
| Files of 1000+ lines split | 1.3 (none affected) |
| Bit-identical audio | 5, gate 8 |

## 11. Risks

- **Kind mirror mistakes.** An edit could map a variant to the wrong
  algorithm. Mitigations: the exhaustive match by identical name, the
  `fx_kind_map` test and the unchanged golden tests.
- **Generic bloat.** Monomorphization grows compile time slightly; release
  audio is unaffected. Cross-crate inlining in release already depends on
  the existing `lto = true` and `codegen-units = 1`.
- **Stale paths in older design docs.** Historical design docs that name
  `src/dsp/...` paths are not rewritten. This document is the path map.
- **Apache-2.0 code under MIT.** msfa-derived code in `fm/` sits in an
  MIT-licensed crate under the same arrangement vactr uses today (MIT
  crate plus Apache notice). The operator must carry the notices when
  merging.

## 11.1 Open questions

These are recorded in
[`../user-qa/pending-music-things-extraction-questions.md`](../user-qa/pending-music-things-extraction-questions.md).
Each defaults to its recommendation:

- **MTQ1.** Should `mise run test`/`clippy` gain `--workspace` while the
  crates live in-tree? Default: not in this run.
- **MTQ2.** Should vactr design-section references be stripped from crate
  comments in the music-things repository later? Default: an operator
  follow-up.
- **MTQ3.** Are generic typed setters keyed by constants (3.4) enough, or
  should each kernel get named setter methods? Default: generic setters
  now; named methods as a music-things follow-up.

## References

- Issue source: workflowInput for workflow session
  `opus-terra-design-and-implement-review-loop-session-577`. No GitHub issue
  number was supplied.
- Target repository: https://github.com/tacogips/music-things (scaffold
  `a72a955`)
- `design-musicdsp-effects.md`, `design-musicdsp-synths.md`,
  `design-mutable-audio.md`, `design-implementation.md` 12.5 and 12.8.8
- `.agents/skills/rust-coding-standards/SKILL.md`
