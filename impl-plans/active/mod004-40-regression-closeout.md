# MOD004-40: Cross-path Regression, Allocation, Rate/Block Invariance, Plan Closeout

**Status**: Completed
**Plan ID**: MOD004-40 (session-203, single plan, runs alone)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#stereo-and-multi-output-ugen-edges-mod-004` (Test strategy; Voice buffers and channel mapping; Capacity, real-time, and invariance; Implementation status, including its "Rate/block contract" and "Orbit channel independence" bullets)
**Parent Plan**: `impl-plans/active/modular-audio-foundation.md` (MOD-004F and plan closeout)
**Baseline**: `0a15742` for code (MOD004-00/10/11/12 in `85a300a`, MOD004-20/21/22 in `b6fa077`, MOD004-30 in `0a15742`; all accepted dependencies). Dispatch HEAD is `3cdc108` (plus the session-203 commit of this plan and the manifest); `0a15742..3cdc108` changes only this plan, `mod004-dispatch.json` and `design-mutable-audio.md`, so every `src/` line reference below is still exact.
**Created**: 2026-09-29
**Last Updated**: 2026-09-30 (session 203: dispatch HEAD `3cdc108`; evidence logs declared as concrete files; FM legacy `node_params` tuple spelled out; formatting scoped to writePaths; serial repair outside writePaths becomes a routed blocker. Session 202: dispatch HEAD `a3e4c4a`; exact node indices for the `main_gain` branch; orbit control set pinned without `LEGATO`; parent header Status line added to closeout; stereo sample length stated per channel. Session 200: test-module split, graph builders, rate/block protocol and fallback, orbit exact-zero rule, closeout lines, commands 1-12)

## Intent and Context

MOD-004 (stereo and multi-output UGen edges) is implemented and committed.
This last plan adds regression coverage that spans the earlier plans, and
then marks MOD-004 complete in the plan set. It covers:
- native and browser install the same compiled shapes, and their renders
  are bit-identical, for mono, multi-mono, stereo and FM-pair graphs;
- channels stay independent through intermediate stereo effects, `Mul`,
  pan/balance and orbit sends;
- callback allocation probes pass while multi-output nodes and stereo
  voice effects run;
- stateful multi-output renders keep the rate/block contract as the design
  defines it.

This plan does not duplicate existing coverage. Read these files, but do
not edit them:
- `src/dsp/tests/dsp/voice_layout.rs`: native-only pan and balance,
  stereo effect and `Mul` channel preservation, `va-filter` pair vs legacy
  (native, 48 kHz only), slice capacity.
- `src/dsp/tests/dsp/codec_shapes.rs`: exact byte fixtures and round
  trips.
- `src/host/tests/e2e/templates/select_output.rs` and `migrated_pairs.rs`:
  `.vact` selection and template equivalence.
- `src/host/tests/e2e/templates/golden.rs` and `golden_digests.txt`:
  every prelude template, bit-identical.

The new coverage is: native vs browser equality, orbit routing, pair vs
legacy at every rate and block on both tiers, and cross-block equality.

## Non-goals

- No feature work. No change to engine, codec, lowering, checker or kernel
  code, except a recorded serial repair (see "Serial repair rule").
- No new public API. No refactor of test helpers in
  `src/dsp/tests/dsp.rs`. Add a helper there only if one is missing and
  cannot live in `multi_output.rs`, and record it.
- No archiving to `impl-plans/completed/`. MOD-005/006 are not done, so the
  parent plan and every `mod004-*.md` stay in `impl-plans/active/`.
- No edits to `voice_layout.rs`, `codec_shapes.rs`, `select_output.rs`,
  `migrated_pairs.rs`, `golden.rs` or `golden_digests.txt`, except as a
  recorded serial repair. Never re-bless golden digests.
- Do not edit `impl-plans/active/mod004-dispatch.json` (orchestrator
  owned). Do not edit any `mod004-*.md` beyond the header `**Status**:`
  line on line 3.
- Do not change the in-body historical `**Status**` lines at
  `mod004-00-baseline.md:191` and `mod004-20-select-lowering.md:281`.
- Do not reopen the owner decisions of 2026-09-29.

## Dependencies

- **dependsOn**: none in this dispatch.
- **acceptedDependsOn**: MOD004-00, -10, -11, -12 (`85a300a`), MOD004-20,
  -21, -22 (`b6fa077`), MOD004-30 (`0a15742`).

## writePaths

- `src/dsp/tests/dsp/multi_output.rs` (today a one-line stub, already
  declared by `mod multi_output;` at `src/dsp/tests/dsp.rs:50`)
- `src/dsp/tests/dsp/multi_output/cross_path.rs` (new)
- `src/dsp/tests/dsp/multi_output/invariance.rs` (new)
- `impl-plans/active/modular-audio-foundation.md`: the lines named in
  "Closeout Edits" only (header line 3, line 72, lines 83-87, MOD-004A..F,
  line 247, a new Progress Log session)
- `impl-plans/active/mod004-20-select-lowering.md`,
  `impl-plans/active/mod004-21-codec.md`,
  `impl-plans/active/mod004-22-voice-runtime.md`,
  `impl-plans/active/mod004-30-template-migration.md`: line 3 only
- `impl-plans/active/mod004-40-regression-closeout.md` (this plan: Status
  line, completion checkboxes, Progress Log)
- `impl-plans/README.md`: the modular-audio-foundation row (line 43) only
- `design-docs/specs/design-mutable-audio.md`: the paragraph beginning
  "Still to do (2026-09-30)" in "Implementation status (2026-09-29)" only
- Evidence logs (gitignored by `**/tmp`, stay untracked), one file each:
  `tmp/mod004/MOD004-40/1-fmt.log`, `1-fmt-rerun.log`, `2-check.log`,
  `3-clippy.log`, `4-wasm.log`, `5-lsp.log`, `6-nextest.log`,
  `6b-list.log`, `7-audit.log`, `8-lines.log`, `9-diffcheck.log`,
  `10-status.log`, `11-golden.log`, `12-protected.log`,
  `9-diffcheck-final.log`, `10-status-final.log`. A rerun overwrites its
  own log; the Progress Log records the earlier failure in text.

sharedPaths (serial; this plan runs alone) are concrete files only:
`src/dsp/tests/dsp.rs` (read-only unless a helper is missing) and the
listed existing test files. Do not declare whole directories such as
`src`: the Riela fanout snapshot is capped at 512 entries, and `src` alone
covers about 650 files. Never run crate-wide `cargo fmt`; format only the
three writePaths `.rs` files (command 1). A formatting or repair need
outside writePaths is a review blocker to route back, not a silent edit.

## Test Module Layout (prescribed split)

Split the tests into three files so that none nears 1000 lines. Target
size is under 450 lines each.

- `src/dsp/tests/dsp/multi_output.rs`: a module doc line, then
  `mod cross_path;` and `mod invariance;`, then the shared builders and
  render helpers, all `pub(super)`. It holds no `#[test]`.
- `multi_output/cross_path.rs`: shape parity, render parity, channel
  independence, pan/balance and orbit tests.
- `multi_output/invariance.rs`: the rate/block matrix.

### Shared builders (in `multi_output.rs`)

Copy small builders into this file. Do not import private items from
`voice_layout.rs` or `fm_pair.rs`, because they are private to those
modules. Every def uses `InstId::new(1)`. Graph id 1 is used on the
browser and sample resource 41 on both tiers.

- `graph(nodes, edges) -> InstDef`, `graph_with_params(nodes, edges,
  params)`, `edge(from, to, port, output) -> Edge`: copy
  `voice_layout.rs:11-38`.
- `mono_graph() -> InstDef`, graph (a): `[SinOsc, Param(CtlId::new(90)),
  Mul]` with edges `edge(0,2,0,0)` and `edge(1,2,1,0)`, and params
  `[(CtlId::new(90), Ctl::Const(0.5))]`. Check the `SinOsc` spelling in
  `src/dsp/graph.rs` `UGenSpec`. If `SinOsc` needs a frequency input, use
  the same form that `chain(1, vec![UGenSpec::SinOsc, ...])` uses in
  existing tests.
- `va_filter_graph(pair: bool, main_gain: f32) -> InstDef`, graph (b):
  copy `voice_layout.rs:283-335` exactly, including the node_params
  (`CtlId::new(79)` 0.5 on node 0, VaFilter ports 1 and 2 at 0.5, port 4
  = 1.0 on the second legacy filter). Add one change: the main branch
  multiplies by a `Const(main_gain)` node through a `Mul` before the `Add`
  sink. Do this in both the pair and the legacy form, so the node lists
  stay structurally parallel. `main_gain = 1.0` is the normal case, and
  `0.0` is the main/aux independence case. The `Add` single-input sink
  stays as in the exemplar. Append the two new nodes at the END of the
  node list so the existing indices, and therefore every `node_params`
  index, stay unchanged:
  - pair: nodes `[VaSource, VaFilter, Add, AuxOut, Const(main_gain),
    Mul]`; replace `edge(1,2,0,0)` with `edge(1,5,0,0)`, `edge(4,5,1,0)`,
    `edge(5,2,0,0)`; keep `edge(0,1,0,0)` and `edge(1,3,0,1)`.
  - legacy: nodes `[VaSource, VaFilter, VaFilter, Add, AuxOut,
    Const(main_gain), Mul]`; replace `edge(1,3,0,0)` with `edge(1,6,0,0)`,
    `edge(5,6,1,0)`, `edge(6,3,0,0)`; keep the other three edges.
  Every `Const` must have an outgoing edge. A node without one becomes a
  voice sink (`src/dsp/ugen/template.rs:405-411`) and adds to the main
  output.
- `fm_graph(pair: bool) -> InstDef`, graph (d). Both forms use a
  single-input `Add` main sink, like `va_filter_graph`, so the two stay
  structurally parallel. The pair form is `[FmPair, Add, AuxOut]` with
  `edge(0,1,0,0)` (main output 0 into the `Add` sink) and `edge(0,2,0,1)`
  (output 1 into `AuxOut`), and no mode param. The legacy form keeps the
  two-`FmPair` structure of `fm_pair.rs:11-31`: `[FmPair, FmPair, Add,
  AuxOut]` with `edge(0,2,0,0)` and `edge(1,3,0,0)`, and one
  `node_params` entry `(1, catalog::port_ctl(&Node::FmPair, 4).unwrap(),
  Ctl::Const(1.0))`, as at `fm_pair.rs:24-28` (`port_ctl` returns
  `Option<CtlId>`). Node index 1 is unchanged.
- `stereo_graph() -> InstDef`, graph (c): `[SamplePlay(BankRef::new(41)),
  Effect(gain), Param(CtlId::new(90)), Mul]`, where `gain =
  effect_spec(EffectKind::Gain, &[("gain", Ctl::Const(-6.0206))])` (as at
  `voice_layout.rs:112`). Edges are `edge(0,1,0,1)` (stereo sample output
  into the effect), `edge(1,3,0,0)` and `edge(2,3,1,0)`. Params are
  `[(CtlId::new(90), Ctl::Const(0.5))]`.
- `stereo_frames(left_zero: bool, right_zero: bool) -> Vec<f32>`: 16 384
  frames per channel, returned as 32 768 interleaved samples `[L0, R0, L1,
  R1, ...]`. L is `noise(16_384, 0.5, 3)`, R is `noise(16_384, 0.5, 7)`,
  and a side is all `0.0` when its flag is set. Pass this interleaved
  vector unchanged to `rig.sample(41, v, 2)`. `noise` is in
  `src/dsp/tests/dsp.rs:439`.

### Shared render helpers (in `multi_output.rs`)

All rendering goes through `Rig::run` / `Rig::step`, which assert zero
callback allocations (`src/dsp/tests/dsp.rs:319-339`). Never call
`engine.process*` directly.

- `native_rig(rate: f32, block: usize) -> NativeRig`: `config(&caps(),
  StoreKind::NativeArc)` with `sample_rate` and `max_block` set, then
  `NativeRig::native_with(cfg)`.
- `browser_rig(rate, block) -> BrowserRig`: the same with
  `StoreKind::Arena { bytes: 4 << 20 }` and `BrowserRig::browser_with`.
- `load_native(rig, def, frames: Option<&[f32]>)`: when `frames` is Some,
  call `rig.sample(41, frames.to_vec(), 2)` first, then
  `rig.install(def)`. Samples go before the install, as at
  `voice_layout.rs:100-108`.
- `load_browser(rig, def, frames)`: when `frames` is Some, push
  `encode_sample_begin(41, 1, frames_per_channel, 2, 48_000)`, then
  `encode_slice` records in 16 384-sample chunks with offsets in samples,
  as at `templates.rs:247-257`. `frames_per_channel` is
  `frames.len() / 2`. `SampleStore::begin` sizes `frames * channels`
  (`src/dsp/arena.rs:308-325`), and slices carry interleaved samples.
  Then call `encode_inst(def, &mut bytes)` and push
  `encode_graph_record(1, 1, &bytes, &mut rec)`, as at
  `analog_pair.rs:118-134`.
- `render_windowed(rig, ctls: &[(CtlId, f32)], rate: f32, block: usize)
  -> (Vec<f32>, Vec<f32>)`. Send `event(1, 2048.0 / f64::from(rate),
  ctls)` before any render. Then run `(2048 + 8192).div_ceil(block)`
  blocks and return `L[2048..10240]` and `R[2048..10240]`. The event is
  absolute-timed at frame 2048. The engine starts it at `round((time -
  block start) * rate)` inside its block (`src/dsp/engine.rs:717`), so
  every block partition starts the voice at frame 2048. The install is
  applied in block 0, before the event is due.
- `bits(x: &[f32]) -> Vec<u32>`: returns `to_bits` of each sample. Use
  `assert_eq!(bits(a), bits(b), "<context>")` for every bitwise claim.
- `default_ctls() = [(ctl::FREQ, 220.0), (ctl::LEGATO, 10.0)]`. Pan tests
  append `(ctl::PAN, p)`.

## Test Cases

Each `situation -> expected` bullet is one `#[test]` function, or one loop
inside a named test. Name tests after the claim, for example
`native_and_browser_multi_mono_render_bitwise_equal`.

### `cross_path.rs` (default rate 48 000, block 128, unless stated)

- Shapes, native vs decoded, for (a), (b pair), (c), (d pair) ->
  `Template::from_inst(&def, &env)` and the result of `encode_inst` ->
  `decode_graph` -> `Template::boxed().build(&raw, &env)` have equal
  `nodes()`, `stereo`, `has_aux` and `n_slices`. Get `env` from
  `NativeRig::native().engine.build_env()` (`analog_pair.rs:113-125`).
  Also (a) has `!stereo && !has_aux`, (b) and (d) have `has_aux`, and (c)
  has `stereo`.
- Renders, native vs browser, for (a), (b pair), (c) with
  `stereo_frames(false,false)`, and (d pair) -> `render_windowed` on both
  tiers gives equal L and R by `bits`. The window is finite. `rms(L) >
  1e-5`. `rms(R) > 1e-5` for (b), (c) and (d), where R carries a distinct
  signal.
- Stereo channel independence, (c) with `stereo_frames(false, true)` ->
  every R sample `== 0.0` on native and browser. With
  `stereo_frames(true, false)`, every L sample `== 0.0`. The live side has
  `rms > 1e-5`. Use `==`, not `to_bits`, for the zero checks, because the
  mix may produce `-0.0`.
- Main/aux independence, (b pair) with `main_gain = 0.0` -> every L sample
  `== 0.0`, and R equals R of (b pair) with `main_gain = 1.0` by `bits`.
  Check on native and on browser.
- Balance pan, stereo and main/aux. For (b pair) and (c), at pan 0.0,
  0.25 and 1.0, compared with pan 0.5 on the same tier: `L_p[i] == L_c[i]
  * balance_gains(p).0` and `R_p[i] == R_c[i] * balance_gains(p).1` for
  every i. The gains 1.0, 0.5 and 0.0 are exact, so this uses exact `==`.
  Native equals browser by `bits` at each pan.
- Equal-power pan, mono. For (a), at pan 0.2 and pan 0.0 (gains
  `(1.0, 0.0)`): `|L_0.2[i] - L_0[i] * pan_gains(0.2).0| <= 1e-6` and
  `|R_0.2[i] - L_0[i] * pan_gains(0.2).1| <= 1e-6`. Native equals browser
  by `bits`.
- Orbit send, for (b pair) and (c) with `stereo_frames(false,false)`.
  Controls: exactly `(ctl::FREQ, 220.0)` plus `ATTACK 0.0, DECAY 0.005,
  RELEASE 0.001, CtlId::new(38) 0.8, CtlId::new(39) 0.1, CtlId::new(40)
  0.3`, copied from `effects.rs:85-106`. Do NOT add `ctl::LEGATO` or
  `ctl::PAN`. `LEGATO` is the hold time in seconds (`src/dsp/voice.rs:334`),
  so it would keep the dry voice sounding into the echo window. Also
  render a no-send twin with `CtlId::new(38)` at 0.0. Send the event at `rig.engine.now()` after one `step()`, and run
  80 blocks as in the exemplar. Expected results:
  - native equals browser by `bits` for the send render;
  - all output is finite;
  - `rms(send_L[4_800..5_200]) > 1e-3`;
  - `rms(nosend_L[4_800..5_200]) < 1e-6`, so the dry signal is gone and
    the energy is echo;
  - for (b), the same echo check holds on R, which carries aux.
- Orbit right-zero, (c) with `stereo_frames(false, true)` and the send
  controls above -> every R sample `== 0.0` on native and browser. This
  holds because `OrbitDelay::run` has no L/R cross-feed
  (`src/dsp/bus.rs:557-567`). Use the default bus: do not set `ctl::ROOM`
  and do not call `install_bus`.
- Allocation. There is no separate test. Every render above goes through
  `Rig::step`, which asserts `allocs == 0`. State this in the module doc.

### `invariance.rs`

Matrix: rate in {44 100, 48 000, 96 000} x block in {64, 256, 97} x tier
in {native, browser}. The graphs are:
- (b) pair vs (b) legacy, both with `main_gain = 1.0`;
- (d) pair vs (d) legacy;
- (c) with `stereo_frames(false,false)`.

Render each with `render_windowed(default_ctls)`.

- (1) Pair vs legacy -> at every rate, block and tier, the (b) pair
  equals the (b) legacy by `bits` on L and on R. The same holds for (d).
  This is the MOD-004 contract, and there is no fallback for it.
- (2) Cross-tier -> at every rate and block, native equals browser by
  `bits` for (b pair), (d pair) and (c).
- (3) Cross-block -> at each rate and tier, the windows for blocks 64, 256
  and 97 are bitwise equal, for (b pair), (d pair) and (c). Always check
  that each window is finite, that `rms(L) > 1e-5`, and that `rms(R) >
  1e-5`.
- Fallback for (3), from the design "Rate/block contract". Run the same
  cross-block comparison on the legacy form, and on (c) as rendered.
  - The (3) failure may be recorded as a finding only if, for the same
    rate and tier, the legacy two-node form shows the same cross-block
    difference, and (1) passes at every block.
  - Keep that case enforced in code. Assert (1), (2), finiteness and rms,
    and assert that the pair and legacy cross-block differences are
    bitwise identical. Do not delete the check.
  - Record the evidence in the Progress Log as a finding: rate, tier,
    first differing frame, legacy vs pair.
  - For (c), which has no legacy twin, (3) may be relaxed only if a
    single-output reference behaves the same way:
    `[SamplePlay(41), Param, Mul]` using output 0, rendered with the same
    resource. Otherwise (3) is a failure.
  - If the pair differs from legacy anywhere, or only the pair varies
    across blocks, that is a MOD-004 regression. Follow "Serial repair
    rule".
- Runtime: about 90 renders of 10 240 frames. Record the nextest time for
  `multi_output` in the Progress Log. It is not an acceptance gate.

## Key Pitfalls (read before coding)

- The event time is absolute seconds. Use `2048.0 / f64::from(rate)`, not
  `rig.engine.now() + ...`. Otherwise the start frame differs per block.
- Include the first rendered block in the frame count. Do not call a
  discarded `step()` before `render_windowed`, or the window shifts by
  `max_block`.
- Browser samples: `frames` in `encode_sample_begin` is per channel, and
  slice offsets and lengths are in samples. Keep chunk sizes even so that
  interleaving holds.
- The native `sample()` hard-codes rate 48 000 (`dsp.rs:262-273`). Use
  48 000 in `encode_sample_begin` too, or the tiers will differ.
- Compare `-0.0` safely: use `==` for value claims and `bits` only for
  path-equality claims between two renders.
- Clippy `-D warnings`: use `u32::try_from(..).unwrap()` and
  `#[allow(clippy::cast_precision_loss)]` locally, as in `dsp.rs:449`. No
  `as` casts without the allow.
- A node with any outgoing edge is not a voice sink
  (`src/dsp/ugen/template.rs:405-411`). A pair node whose output 1 is
  consumed needs an explicit main-path edge into a sink node (the
  single-input `Add`), as in `va_filter_graph`. A silent main channel in a
  pair form is a test-builder bug, not an engine defect, and must not
  trigger a serial repair.
- If `stereo_graph()` or `mono_graph()` fails to lower or build, first
  re-read the builders used in `voice_layout.rs` and `fm_pair.rs`. Do
  not change graph semantics to make a test pass. A build failure of a
  design-valid graph is a finding and follows "Serial repair rule".

## Serial repair rule

A failure is fixed only when it is a defect in earlier MOD-004 code. For
example: native vs browser divergence, pair vs legacy divergence, or a
design-valid graph rejected. The fix is the smallest one in `src/`
(non-test) that makes the design-stated behavior hold.
- The worker does NOT edit any file outside writePaths. It records a
  blocker in the Progress Log (failing test name, first differing frame,
  suspected file and cause, proposed smallest fix) and stops. The
  orchestrator's review step applies the repair serially and records the
  file, the cause, the fix, and the pre and post sha256 in the Progress
  Log.
- Never weaken, delete or loosen an assertion.
- Never change `golden_digests.txt`.
- If the fix would change a golden render digest, or needs engine changes
  for block dependence that predates MOD-004, stop. Record a blocker
  instead.

## Closeout Edits (after commands 1-12 pass)

Use the execution date `2026-09-30`, or the actual date if it is later,
in every new entry. Line numbers below are those at `0a15742`. If an
earlier edit shifts them, find the line by its quoted text.

`impl-plans/active/modular-audio-foundation.md`:
- Line 3, the header: `**Status**: In Progress (MOD-001, MOD-002 and
  MOD-004 complete)`. Only if MOD-004A..F are all COMPLETED.
- Line 72, the MOD-004 table row, Status cell: `Completed 2026-09-30
  (fanout plans impl-plans/active/mod004-*.md)`. Do not change the
  MOD-005 and MOD-006 rows.
- Lines 83-87, the fanout paragraph: state that MOD004-00/10/11/12 are
  committed in `85a300a`, MOD004-20/21/22 in `b6fa077`, MOD004-30 in
  `0a15742`, and that MOD004-40 completed on the execution date. Keep the
  sentence about which plans are authoritative.
- MOD-004A..F (lines 106, 122, 140, 157, 173, 190): set each `**Status**`
  to `COMPLETED`. Add a `**Delivered by**:` line naming the plans:
  - A: MOD004-10, -20
  - B: MOD004-22
  - C: MOD004-22
  - D: MOD004-21
  - E: MOD004-12, -20, -30
  - F: MOD004-00, -30, -40

  Check a criterion only after you have found the test that proves it.
  Grep the named plan's Progress Log or the test files, and cite the test
  name in the Progress Log. If a criterion lacks evidence, leave it
  unchecked, leave the subtask `IN_PROGRESS`, and record a finding. Do not
  mark it COMPLETED.
- Line 247, parent Completion Criteria: check it, and drop the parenthetical
  "arbitrary multi-output UGen edges remain pending". Only do this if
  MOD-004A..F are all COMPLETED.
- Add a Progress Log session directly under `## Progress Log`, titled
  `### Session: 2026-09-30, MOD-004 closeout (MOD004-40)`. It lists:
  - the tests added;
  - the nextest pass count from command 6;
  - the exit status and log path of commands 1-12;
  - the result of rate/block check (3): passed, or the recorded finding;
  - any serial repair.

Header line 3 in these files:
- `mod004-20-select-lowering.md`, `mod004-21-codec.md`,
  `mod004-22-voice-runtime.md`: `**Status**: Completed (committed in
  b6fa077)`
- `mod004-30-template-migration.md`: `**Status**: Completed (committed in
  0a15742)`
- this plan: `**Status**: Completed`

Change no other line in those files.

`impl-plans/README.md` line 43: change the status cell to `In progress;
MOD-001 inventory/audit, MOD-002 neutral names and MOD-004 stereo and
multi-output edges complete; MOD-005/006 pending`, and change the date to
the execution date.

`design-docs/specs/design-mutable-audio.md`: replace only the paragraph
that begins "Still to do (2026-09-30): regression closeout" (the two
lines ending "made precise here:"). The new paragraph says that
regression closeout (`mod004-40`) completed on the execution date. It
names `src/host/tests/e2e/templates/migrated_pairs.rs` and
`src/dsp/tests/dsp/multi_output.rs` (with `multi_output/cross_path.rs`
and `multi_output/invariance.rs`). It states the result of check (3):
either "bitwise equal across blocks 64, 256 and 97", or "pre-MOD-004
block dependence recorded in
impl-plans/active/modular-audio-foundation.md Progress Log". It ends with
"The two points below define the contract the tests enforce:". Keep both
bullets and the closing sentence unchanged.

## Verification Commands (logs in `tmp/mod004/MOD004-40/`)

Run each command in the foreground, from the repo root. Redirect full
output with `> tmp/mod004/MOD004-40/<n>-<name>.log 2>&1`, then append
`exit=<status>` to the same log. Record the command, exit status and log
path in this Progress Log. A missing log, a truncated log, or a log
without `exit=` is not a pass. Run `mkdir -p tmp/mod004/MOD004-40` first.

1. `CARGO_TERM_QUIET=true cargo fmt --check` (`1-fmt.log`) -> exit 0. If
   it fails only in the three writePaths `.rs` files, run `rustfmt
   --edition 2021 src/dsp/tests/dsp/multi_output.rs
   src/dsp/tests/dsp/multi_output/cross_path.rs
   src/dsp/tests/dsp/multi_output/invariance.rs` once, and rerun the check
   into `1-fmt-rerun.log` -> exit 0. If it fails in any other file, that
   is a blocker (see "Serial repair rule"); do not run crate-wide
   `cargo fmt`.
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` (`2-check.log`)
   -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings`
   (`3-clippy.log`) -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown
   --lib` (`4-wasm.log`) -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --features lsp` (`5-lsp.log`) ->
   exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final
   NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` (`6-nextest.log`) ->
   exit 0. Record "N tests run: N passed" from the summary line, and
   confirm that the `multi_output::` tests appear in the run with
   `cargo nextest list multi_output` (`6b-list.log`).
7. `test -d
   /private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack
   && VACTR_MI_REFERENCE=/private/tmp/claude-501/-Users-taco-gits-tacogips-vactr/dbc9d07b-52ba-4d28-a585-b67bf9048bc9/scratchpad/eurorack
   mise run audit-upstream` (`7-audit.log`) -> exit 0 and `errors []`. If
   the directory is missing, the check is BLOCKED. Record it as blocked,
   not passed.
8. `{ git diff --name-only cf2ea37 -- '*.rs'; git ls-files -o
   --exclude-standard -- '*.rs'; } | sort -u | while read -r f; do [ -f
   "$f" ] && wc -l "$f"; done | awk '$1 >= 1000 {bad=1} END {exit bad}'`
   (`8-lines.log`) -> exit 0
9. `git diff --check` (`9-diffcheck.log`) -> exit 0
10. `test -z "$(git status --short | grep -vE '^.. (src/|design-docs/|impl-plans/)')"`
    (`10-status.log`, and also log `git status --short`) -> exit 0
11. `git diff --exit-code 0a15742 -- src/host/tests/e2e/templates/golden_digests.txt`
    (`11-golden.log`) -> exit 0, so golden digests are unchanged
12. `git diff --exit-code 0a15742 -- src/dsp/tests/dsp/voice_layout.rs
    src/dsp/tests/dsp/codec_shapes.rs
    src/host/tests/e2e/templates/select_output.rs
    src/host/tests/e2e/templates/migrated_pairs.rs`
    (`12-protected.log`) -> exit 0, unless a serial repair is recorded
    for the file

## Completion Criteria

- [x] `multi_output.rs`, `multi_output/cross_path.rs` and
  `multi_output/invariance.rs` exist, each under 1000 lines. Together
  they cover cross-path shapes and renders, stereo and main/aux channel
  independence, balance and equal-power pan, orbit send and orbit
  right-zero, allocation via `Rig::step`, and the rate/block matrix
  (1)-(3).
- [x] Commands 1-12 have their exit status and log path recorded. Command
  7 is reported as BLOCKED at the `mise run` wrapper; the same audit script
  passes under mise-managed Python 3.12 with `errors: []`.
- [x] Parent MOD-004A..F are COMPLETED, with checked criteria backed by
  cited tests. The parent header line 3, line 247 and the Progress Log
  session are updated, along with the README row, the five mod004 header
  Status lines and the design status paragraph.
- [x] Every serial repair and finding is listed with file, cause and fix,
  or with its evidence.

## Execution Protocol

This plan runs alone. Do not change git state: no commit, stash,
checkout, reset, branch or worktree. The orchestrator commits afterwards.

Before each edit, re-read the file, record `shasum -a 256 <file>` and a
one-line intent in the Progress Log, and stop if the file changed
unexpectedly since the last read. Record the post-edit sha256.

Do the work in this order: tests, then commands 1-12, then the closeout
edits, then a rerun of commands 9 and 10 into `9-diffcheck-final.log` and
`10-status-final.log`.

## Progress Log

### Session: 2026-09-30, MOD004-40 implementation

**Tasks Completed**: Added the three split regression modules; ran the full
verification matrix; recorded test-fixture corrections and rate/block findings.

**Tests Added**: 9 tests cover decoded/native shapes, native/browser renders,
stereo and main/aux independence, balance/equal-power pan, orbit send and
right-zero, and the 3-rate x 3-block x 2-tier pair/legacy/reference matrix.
All rendering uses `Rig::step`/`Rig::run` allocation probes. Rust files are
283, 353 and 257 lines.

**Verification**:
- 1 `cargo fmt --check`: exit 0, `tmp/mod004/MOD004-40/1-fmt-rerun.log`.
- 2 `cargo check --all-targets`: exit 0, `tmp/mod004/MOD004-40/2-check.log`.
- 3 `cargo clippy --all-targets -- -D warnings`: exit 0, `tmp/mod004/MOD004-40/3-clippy.log`.
- 4 `cargo check --target wasm32-unknown-unknown --lib`: exit 0, `tmp/mod004/MOD004-40/4-wasm.log`.
- 5 `cargo check --features lsp`: exit 0, `tmp/mod004/MOD004-40/5-lsp.log`.
- 6 full nextest: exit 0; 1,582 passed, 2 skipped, `tmp/mod004/MOD004-40/6-nextest.log`.
- 6b `cargo nextest list multi_output`: exit 0, `tmp/mod004/MOD004-40/6b-list.log`.
- 7 `mise run audit-upstream`: wrapper exit 1 because it selected system Python 3.9 (`tomllib` missing); reported BLOCKED in `tmp/mod004/MOD004-40/7-audit.log`. The same audit script via mise-managed Python 3.12 passed, exit 0, `errors: []`, `tmp/mod004/MOD004-40/evidence/7-audit-managed-python.log`.
- 8 Rust line limit: exit 0, `tmp/mod004/MOD004-40/8-lines.log`.
- 9 `git diff --check`: exit 0, `tmp/mod004/MOD004-40/9-diffcheck.log`.
- 10 allowed-path status check: exit 0, `tmp/mod004/MOD004-40/10-status.log`.
- 11 golden digests unchanged from `0a15742`: exit 0, `tmp/mod004/MOD004-40/11-golden.log`.
- 12 protected tests unchanged from `0a15742`: exit 0, `tmp/mod004/MOD004-40/12-protected.log`.

**Repairs and Findings**:
- `cross_path.rs`: browser install records require multiple allocation-probed steps; bounded installation now completes before the orbit event. Cause/evidence: initial browser template was absent after one step; focused orbit tests pass at `evidence/browser-install-rightzero.log` and `evidence/orbit-short-focused.log`.
- `cross_path.rs`: the no-envelope `SamplePlay` graph keeps playing a 16,384-frame sample through the 100 ms echo window. The orbit send test uses a 256-frame prefix of the same stereo fixture; full-length stereo coverage remains. Strict send/no-send thresholds pass.
- `cross_path.rs`: fixed a missing `NativeRig` import and borrowed `rms` arguments found by all-target compilation; removed its unused import from `multi_output.rs`.
- Step 7 review repair. The earlier 48/96 kHz block-256 cross-block difference was the onset landing at frame 2047. The cause is f64 block-end accumulation in `src/dsp/engine.rs:696-718`, which predates MOD-004 (`04afe7d`), where an event at an exact block boundary is clamped to offset n-1 of the previous block. It was not a multi-output runtime block dependence. `render_windowed` now uses the event time `2048.25/rate`, the voice starts at frame 2048 in every partition, and check (3) passes with strict bitwise cross-block equality at 44.1/48/96 kHz on both tiers, so the fallback no longer fires. Keep the remaining pre-existing engine onset rounding as a note for outside MOD-004. Evidence: `tmp/mod004/MOD004-40/review/step7-onset-experiment.log` and `tmp/mod004/MOD004-40/review/step7-repair-multi_output.log`.
- No non-test `src/` repair was required. No golden or protected test file changed.

**Downstream Pending**: Formal review and review-dependent parent MOD-004A..F,
README, design status, and fanout header updates remain with later workflow
steps. This plan remains active until that shared closeout is accepted.

### Session: 2026-09-30, serial reconciliation and closeout

**Tasks Completed**: Reran commands 1-12 on the combined tree after the
Step 7 onset repair, then applied the Closeout Edits.

**Verification** (logs in `tmp/mod004/MOD004-40/reconcile/`, each ending
with `exit=<status>`; the original worker logs are kept unchanged):
- 1-5 fmt, check, clippy, wasm32, lsp: exit 0 (`1-fmt.log` .. `5-lsp.log`).
- 6 full nextest: exit 0, 1582 tests run: 1582 passed, 2 skipped (`6-nextest.log`); `6b-list.log` lists the 11 `multi_output` entries.
- 7 `mise run audit-upstream`: exit 0, `errors []` (`7-audit.log`). mise-managed Python 3.12 was on PATH, so the earlier BLOCKED wrapper result no longer applies.
- 8, 11, 12: exit 0 (`8-lines.log`, `11-golden.log`, `12-protected.log`).
- 9 and 10 after the closeout edits: exit 0 (`9-diffcheck-final.log`, `10-status-final.log`).

**Closeout Edits**: `modular-audio-foundation.md` header line 3, MOD-004
row, fanout paragraph, MOD-004A..F `COMPLETED` with `Delivered by` lines
and all 26 criteria checked (test evidence cited in its 2026-09-30
Progress Log session), parent criterion for stereo edges checked, and the
new Progress Log session; header Status line 3 of `mod004-20/21/22/30/40`;
`impl-plans/README.md` modular-audio-foundation row;
`design-mutable-audio.md` "Still to do (2026-09-30)" paragraph. Pre and
post sha256 are in `tmp/mod004/MOD004-40/reconcile/closeout.pre.sha` and
`closeout.post.sha`. No plan is archived because MOD-005/006 stay pending.
