# PLV-20: `vactrol-gate` and `decay-mod` UGen Kinds, Controls and Registry

**Status**: Ready
**Plan ID**: PLV-20 (wave 2; parallel with PLV-21)
**Design Reference**: `design-docs/specs/design-mutable-audio.md#plaits-voice-level-trigger-and-low-pass-gate-layer-plv-001` (Modules and nodes; Controls and editor metadata; Event mapping; Audio path per lane; `decay-mod`; Real-time, capacity and invariance)
**Parent Plan**: `impl-plans/active/modular-plaits-engines.md`
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan turns the PLV-10 DSP into two mono graph UGens, `vactrol-gate`
and `decay-mod`, and adds the three neutral controls `lpg-mode`,
`lpg-decay` and `lpg-color`. It is the one serialized registry-touching
plan: every node-kind table (spec, node, catalog, codec, names, checker
domain, editor node map, kernel dispatch, memory sizing) and the
control table change here, and only here. PLV-30 (voice lifetime) and
PLV-31 (template wiring) build on the contract below.

## Non-goals

- No template (`src/prelude/templates.vact`), editor template list
  (`src/dsp/meta/templates.rs`), golden digest, voice lifetime
  (`src/dsp/voice.rs`, `src/dsp/ugen/template.rs`), manifest, inventory
  or notice change. The manifest (`src/dsp/ported/manifest.rs`) stays
  untouched; the only edit under `src/dsp/ported/` is the naming-test
  extension listed in writePaths.
- `vactrol-gate` and `decay-mod` are not envelopes. Do not add them to
  `Node::is_env` or to the `envs` counter.
- No multi-output declaration. Both are mono and fall under the MOD-004
  default row "every kind not listed: 0 mono".

## Dependencies

- **dependsOn**: PLV-10 (the `voice_layer` API), PLV-12 (`plaits_voice`, `VoiceRegistration`)
- **Blocks**: PLV-30, PLV-31

## writePaths

- `src/dsp/ugen/vactrol_gate.rs` (new; both kernels)
- `src/dsp/graph.rs` (the `UGenSpec::VactrolGate` and `UGenSpec::DecayMod` variants)
- `src/dsp/graph/shape.rs` (arms only where a match is exhaustive; both are mono)
- `src/dsp/ugen/mod.rs` (the `Node::VactrolGate` and `Node::DecayMod` variants, and `pub mod vactrol_gate;`)
- `src/dsp/ugen/catalog.rs` (`node_of`, `ugen_name`, name list, port table mapping)
- `src/dsp/ugen/catalog/voice_ports.rs` (`VACTROL_GATE` and `DECAY_MOD` port tables)
- `src/dsp/ugen/catalog/codec.rs` (wire tags 96 = VactrolGate, 97 = DecayMod, encode and decode)
- `src/dsp/ugen/mixer.rs` (mono dispatch arms)
- `src/dsp/ugen/build_helpers.rs` (`mem_need` arms)
- `src/dsp/build/names.rs` (the `UGENS` entries `("vactrol-gate", ...)` and `("decay-mod", ...)`)
- `src/dsp/build/names/table.rs` (lowering port lists, identical order to `voice_ports.rs`)
- `src/dsp/meta.rs` (the name-to-`Node` editor map arms only)
- `src/types/natives_domain.rs` (`dsp("vactrol-gate")`, `dsp("decay-mod")`)
- `src/dsp/controls.rs` (the three new rows and the `LPG_MODES` constant)
- `src/dsp/tests/dsp/vactrol_gate.rs` (new)
- `src/dsp/tests/dsp.rs` (one `mod vactrol_gate;` line)
- `src/dsp/ported/tests.rs` (only extend `no_user_facing_name_carries_an_upstream_module_token` so it also calls the existing `assert_neutral_name` on `"vactrol-gate"` and `"decay-mod"`, preferably taken from the catalog/UGen name list rather than literals if a public list exists, literals are acceptable; no other edit in that file)
- `impl-plans/active/plv-20-gate-nodes.md` (Progress Log only)
- `tmp/plv/PLV-20/hashes.txt`, `tmp/plv/PLV-20/1-fmt.log`, `tmp/plv/PLV-20/2-check.log`, `tmp/plv/PLV-20/3-clippy.log`, `tmp/plv/PLV-20/4-wasm.log`, `tmp/plv/PLV-20/5-lsp.log`, `tmp/plv/PLV-20/6-focused.log`, `tmp/plv/PLV-20/7-nextest.log`, `tmp/plv/PLV-20/8-lines.log`

## sharedPaths (read-only)

- `src/dsp/ugen/voice_layer.rs` (PLV-10 API; do not edit)
- `src/dsp/ported/manifest.rs` (PLV-12 `plaits_voice`; do not edit)
- `src/dsp/ugen/shape_pair.rs:render` and `STATE_FLOATS` (kernel signature and memory pattern)
- `src/dsp/tests/dsp/shape_pair.rs` (kernel-level test pattern with `Inp`, `NodeState`, `Kx`)
- `src/dsp/tests/dsp/analog_pair.rs` (native vs decoded `Template` equality and browser install pattern)
- `src/dsp/voice.rs` (read only: `Kx.gate` semantics and `finished`; PLV-30 owns edits)

## Contract (pin exactly; PLV-30 and PLV-31 depend on it)

- `.vact` names: `vactrol-gate`, `decay-mod`. Spec/node variants:
  `UGenSpec::VactrolGate`, `UGenSpec::DecayMod`, `Node::VactrolGate`,
  `Node::DecayMod`. Wire tags: 96 and 97, both with a zero payload.
- The `vactrol-gate` port order is identical in `voice_ports.rs` and
  `names/table.rs`:
  `in` (0.0), `freq` (use the existing `FREQ` port), `velocity` (1.0),
  `lpg-mode` (0.0), `lpg-decay` (0.5), `lpg-color` (0.5), `slot` (0.0),
  `lane` (0.0), `clocked` (0.0).
- The `decay-mod` port order is: `lpg-decay` (0.5), `amount` (0.0),
  `target` (0.0).
- Kernel entry points in `src/dsp/ugen/vactrol_gate.rs`, with the
  `shape_pair::render` signature:
  `pub fn render_gate(ins, st, mem, out, kx)`,
  `pub fn render_decay_mod(ins, st, mem, out, kx)`,
  `pub const GATE_STATE_FLOATS: usize` (at most 24), and
  `pub const DECAY_MOD_STATE_FLOATS: usize` (at most 8). `mem_need`
  returns `(X_STATE_FLOATS, 0)`.
- `NodeState` flags written by `render_gate` (the lifetime contract read
  by PLV-30):
  - `u[1] = 1` after the first call (latched);
  - `u[2] = 1` if and only if the latched mode is Ping or Level, the slot
    is valid, and the lane is not bypassed ("shaping");
  - `u[0] = 1` once the gate has closed (`kx.gate < out.len()` in any
    call);
  - `st.finish()` (u[3]) when shaping, `u[0] == 1` and
    `VactrolEnvelope::is_done()`.
  - Non-shaping gates never call `finish`.
- Control rows in `src/dsp/controls.rs`, appended after `onset-time`:
  - `mk("lpg-mode", 153, 0.0, (0.0, 2.0), CtlRoute::InstParam, CtlDomain::Enum(LPG_MODES))`
    with `LPG_MODES = &["off", "ping", "level"]`;
  - `param("lpg-decay", 154, 0.5, (0.0, 1.0))`;
  - `param("lpg-color", 155, 0.5, (0.0, 1.0))`.

## Kernel behavior (`render_gate`)

1. **First call** (`u[1] == 0`):
   - Latch `mode = LpgMode::from_control(ins[3].first())`,
     `slot = round(ins[6].first())`, `lane = ins[7].first() >= 0.5`,
     `clocked = ins[8].first() >= 0.5`, and
     `reg = plaits_voice(slot)` (None when negative, non-finite or
     >= 24).
   - Identity applies when `mode == Off` or `reg` is None. Otherwise
     `G = reg.gain(lane)` and `bypass = reg.is_enveloped(clocked)`.
   - Store the latched values in `st.s`.
   - Initialize the mem state: `ControlClock` with carry 0, a
     `VactrolEnvelope::new()` followed by `trigger()` when Ping,
     `LowPassGate` zeros, `PostLimiter::new()`, and 0 remaining samples.
     Zero-initialized memory is not a valid initial state for these
     types.
2. **Identity**: copy the input to `out` bit for bit (`Inp::Buf` via
   `copy_from_slice` over `out.len()`, `Inp::Val` via fill). No other
   state work. This is the golden-digest guarantee.
3. **Bypass**: per sample, `x1 = G < 0 ? limiter.process(-G, sr, x) : x`;
   the output is `clip_unit(x1 * post_gain(G))`.
4. **Active (Ping or Level)**: per sample `i`. When `remaining == 0`:
   - `len = clock.next_len(sr)`;
   - `open = i < kx.gate`;
   - `(short, tail) = decay_terms(ins[4].at(i), ins[5].at(i))`;
   - Ping: `process_ping(ping_attack(ins[1].at(i)), short, tail, color)`;
     Level: `process_lp(open ? compress_level(ins[2].at(i)) : 0, short, tail, color)`;
   - `lpg.begin(vactrol.gain() * post_gain(G), vactrol.frequency(), vactrol.hf_bleed(), len, sr)`;
   - `remaining = len`.
   Then `x1` is the limiter output as in bypass, the output is
   `clip_unit(lpg.tick(x1))`, and `remaining -= 1`.
5. **End of call**: update `u[0]` and `finish` as in the contract, then
   store all state back to `mem`.

## Kernel behavior (`render_decay_mod`)

- The first call triggers a `DecayEnvelope` (value 1) and initializes a
  `ControlClock`.
- At each control-block start:
  - `(short, _) = decay_terms(ins[0].at(i), 0.5)`, then `env.process(short)`;
  - `a = shaped_amount(ins[1].at(i))`, `e = env.value()`;
  - the held output is `a * e` when `ins[2].at(i) < 0.5`, else
    `exp2(a * e * e * 4.0)` (48 semitones / 12).
- Write the held value to every sample of the block. This models the
  trigger-patched `ApplyModulations` branch only (design divergence 12).

## Pitfalls

- Identity must not read, clamp, guard or rescale. Even NaN and -0.0
  pass through unchanged; the voice guard handles them later.
- Read `lpg-mode`, `slot`, `lane` and `clocked` only on the first call.
  Read `freq`, `velocity`, `lpg-decay` and `lpg-color` at each
  control-block start, via `Inp::at(i)` rather than `first()`.
- The control clock is anchored at the voice's first sample and carried
  across calls. Never reset `remaining` at a callback boundary.
  Otherwise the partition-invariance tests in PLV-30 fail.
- No allocation, no `format!`, no panics on bad input. Use saturating or
  clamped conversions for `slot`.
- `src/dsp/controls.rs` ids 153-155 are the next free control-table ids.
  They numerically overlap `EXTRA_CTL_BASE + k` ids, as ids 128-152
  already do. Do not renumber existing rows.
- `src/dsp/meta.rs` is 674 lines and `names/table.rs` is 711 lines. Add
  only the needed arms, and keep every touched file below 1000 lines.

## Test Cases (`src/dsp/tests/dsp/vactrol_gate.rs`)

- Off, slot 10: a 512-sample input with pseudo-random values, 0.0,
  -0.0, 1e-30, 3.0 and NaN gives output bits equal to the input bits,
  over callbacks of 64 and 97; `u[2] == 0`; `finish` is never called.
- Invalid slot 24 with Ping -> identity.
- Ping, slot 0, lane 0, 48 kHz, constant input 0.5: output starts near 0,
  peaks within the first 5 ms, then decays; with `lpg-decay` 0.8 the
  energy after 100 ms is greater than with 0.2. `u[2] == 1`.
- Ping with `lpg-color` 0 vs 1 on white-noise input: color 0 has a lower
  high-band energy ratio.
- Level, slot 0, gate open for 2400 samples, velocity 1 vs 0.5: the
  steady output RMS ratio is roughly 1 : 0.8125 (+-15%). After the gate
  closes the output decays, `u[0] == 1`, and `done()` eventually becomes
  true.
- Bypass, slot 21, Ping: output equals `clip_unit(x * 0.8)` for every
  sample (within 1e-7); `u[2] == 0`.
- Bypass with limiter, slot 19, lane 0 (-1.0): output magnitude <= 0.8
  + 1e-3 for a 4.0 input after settling. Lane 1 (0.8) gives
  `clip_unit(x * 0.8)`.
- Slot 7 with `clocked` 1 -> bypass (gain 0.5); `clocked` 0 -> shaping.
- Callback partition: Ping on the same input, rendered with calls of 64,
  256 and 97 at 44100/48000/96000, is bitwise equal within each rate.
- `decay-mod`: amount 0, target 0 -> all 0.0; amount 0, target 1 -> all
  1.0; amount 1, target 0 -> first block value `0.9975 * (1 - 2 short)`
  within 1e-6, and monotonically decreasing block to block.
- Codec: a graph with `VactrolGate` and `DecayMod` encodes, decodes and
  re-encodes to identical bytes, and the native and decoded `Template`s
  are equal (following `src/dsp/tests/dsp/analog_pair.rs`).
- Lowering: a `UGenNode` tree equivalent to
  `sin-osc 220 > vactrol-gate freq velocity slot: 10 lane: 1`, lowered
  with `crate::dsp::build::lower_inst` as existing dsp tests build trees,
  puts each argument on the port with its contract index. If dsp tests
  cannot build such a tree, skip this case here: PLV-31 covers `.vact`
  lowering end to end. Record which applies.
- The control rows exist: `controls::row("lpg-mode")` has the Enum
  domain with the three choices, and default 0.
- Naming (`src/dsp/ported/tests.rs`): the extended
  `no_user_facing_name_carries_an_upstream_module_token` calls
  `assert_neutral_name` on `"vactrol-gate"` and `"decay-mod"` and passes.
  This is the design's "Controls and editor metadata" naming-test
  requirement for the two UGen names.

## Verification Commands (logs in `tmp/plv/PLV-20/`, each ending with `exit=<n>`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (fix only your files with `rustfmt --edition 2021`)
2. `CARGO_TERM_QUIET=true cargo check -q --all-targets` -> exit 0
3. `CARGO_TERM_QUIET=true cargo clippy -q --all-targets -- -D warnings` -> exit 0
4. `CARGO_TERM_QUIET=true cargo check -q --target wasm32-unknown-unknown --lib` -> exit 0
5. `CARGO_TERM_QUIET=true cargo check -q --all-targets --features lsp` -> exit 0
6. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run vactrol_gate golden ported` -> exit 0 (golden must pass unchanged; no template uses the new nodes yet)
7. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run` -> exit 0; record the count
8. `wc -l` on every writePath `.rs` file -> each below 1000

## Completion Criteria

- [ ] Both kinds are registered in every kind table that lists `ShapePair` outside tests (`graph.rs`, `graph/shape.rs` where exhaustive, `ugen/mod.rs`, `catalog.rs`, `catalog/voice_ports.rs`, `catalog/codec.rs`, `mixer.rs`, `build_helpers.rs`, `build/names.rs`, `build/names/table.rs`, `meta.rs`) and in `types/natives_domain.rs`, with wire tags 96 and 97 and the contract port orders. `src/dsp/voice.rs` `run_pair` keeps its wildcard and is not edited.
- [ ] The kernel flags `u[0..=3]` follow the contract, as the tests show.
- [ ] The three control rows exist with the pinned ids, domains and defaults.
- [ ] `no_user_facing_name_carries_an_upstream_module_token` in `src/dsp/ported/tests.rs` also asserts `vactrol-gate` and `decay-mod` are neutral names, with no other edit in that file (verification command 6 already includes the `ported` filter).
- [ ] `golden_digests.txt` is untouched, and the golden tests pass.
- [ ] Commands 1-8 pass with logs.

## Execution Protocol (same-branch fanout)

1. Do not change git state: no commit, stash, checkout, reset, branch,
   worktree or push. Read-only `git diff` and `git status` are allowed.
2. Before the first edit, record `shasum -a 256` of each existing
   writePath and a one-line intent per file in `tmp/plv/PLV-20/hashes.txt`
   and in the Progress Log.
3. Re-read each file just before editing it. If it changed in a way you
   did not make, re-read it and re-apply only your own change, and record
   the drift.
4. Never edit outside writePaths. PLV-21 runs at the same time and owns
   `mise.toml`, `verification/` and `examples/plaits_voice_reference.rs`.
   PLV-21 does not touch `src/dsp/ported/tests.rs`, so wave 2 stays
   disjoint (PLV-12 edited that file in wave 1; PLV-40 edits it in wave 4).
   If cargo fails only inside its files, wait about 60 s and retry, up to
   10 times, then record a blocker and stop. If another file needs a new
   exhaustive-match arm, stop and record it as a blocker; do not edit it.
5. Run each command as `cmd > log 2>&1; echo "exit=$?" >> log`. Record
   post-edit hashes, exit statuses and log paths in the Progress Log. A
   missing or truncated log is not a pass.

## Progress Log

(none yet)
