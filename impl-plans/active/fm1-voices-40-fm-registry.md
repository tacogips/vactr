# FM1V-40: Wire `fm-mod` Algorithm Mode and Register `fm6-core` With Patch Payload

**Status**: Ready
**Plan ID**: FM1V-40 (run 3, serial wave 4 of 6; registry plan, after FM1V-30)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("`fm` template backward compatibility (chosen mechanism)", "`fm6-core` UGen", "SysEx import" usage, "Registration and digest stability", "Implementation partition", "Verification" -> `fm` template and Native)
**Created**: 2026-10-10
**Last Updated**: 2026-10-10 (session 353)

## Session 353 Note (run 3)

- Base is `48d651c`. FM1V-21, FM1V-20 and FM1V-30 are accepted before this
  plan starts, so strict clippy is already green. Verification 6 must keep
  it at exit 0.
- `fm6-core` gets no prelude template (FV7), so the template count stays
  79. Do not touch `template_slots` (102, `src/dsp/ring.rs:811`) or
  `SONG_TEMPLATE_SLOTS` (134, `src/host/song_profile.rs:12`). The full
  nextest run includes `tests/song_bus_memory_layout.rs`, which must stay
  4/4.
- Algorithms 4 and 6 are settled by the design (delayed-history
  cross-operator edge). The `fm` algorithm mode inherits that behaviour from
  the FM1V-20 engine. Do not change it here.

## Session 352 Revision (read first)

- **Vact syntax.** Vactr has no `( )` grouping and `let` takes no `=` (design
  "SysEx import" -> Usage). Every `.vact` snippet in this plan's tests uses
  `let name value`, `{}` grouping, `>` pipes and path literals. A list is
  called with an index: `{bank 1}`.
- **Start snapshot.** Before any edit:
  - save `git status --porcelain=v1` to
    `tmp/fm1-voices/FM1V-40/status-before.txt`;
  - copy `src/host/tests/e2e/templates/golden_digests.txt` and
    `src/prelude/templates.vact` to
    `tmp/fm1-voices/FM1V-40/golden_digests.before` and
    `tmp/fm1-voices/FM1V-40/templates.vact.before`.

  Verification 2, 3 and 9 compare against these copies, not against `HEAD`.
  This keeps the checks correct whether or not FM1V-30's work is committed.
- **Tag 105** is free, because FM1V-30 used 106..111. Assign it here. It is
  out of numeric order in the encode and decode arms, which is expected. Do
  not renumber anything.
- This plan runs alone. Stub files `src/dsp/tests/dsp/fm6_registry.rs` and
  `src/host/tests/e2e/templates/fm_mod_algorithm.rs` exist from FM1V-00
  (one `//!` line each). Replace their bodies.

## Intent and Context

After this plan, the existing `fm` template's `algorithm` control works:

- `s :fm > algorithm 7 > note ...` renders six-operator topology 7;
- `algorithm 0`, the new default, keeps today's exact output and graph
  digest.

The mechanism, already proven in the design:

- **Append implicit ports.** The new `fm-mod` ports are named after
  existing `InstParam` control rows (`algorithm` 21, `freq` 0, `ratio` 19,
  `velocity` 6). `Template::compile_node` binds them to the voice's live
  controls with no new graph nodes or edges.
- **Graph digest.** The golden graph digest hashes only `def.nodes` Debug
  and the edge triples, so it is unchanged.
- **Render digest.** Row 21 and the `fm` header default become 0, so the
  default render takes the legacy branch.

This plan also registers `fm6-core` (codec tag 105). It takes an optional
constant `patch:` list of 155 values, following the `frame-keyframe-core
frames:` payload precedent.

## Non-goals

- No change to the existing `fm` graph. Do not pass `algorithm:` in the
  `fm` body, and do not add a non-row header name.
- No `fm6-core` prelude template (FV7).
- No examples or docs (FM1V-50/51). The `design-music.md` epiano line
  belongs to FM1V-51.
- No change to `six-op-original` or the `six-bank-*` templates.

## Dependencies

- **dependsOn**: FM1V-20 (`fm::modulate_with_algorithm`,
  `engine::fm6_render`, `engine::STATE_FLOATS`, `fm_mod_port`,
  `FM6_PORTS`), FM1V-21 (`fm6-sysex` native), FM1V-30 (the registry files
  in their post-voice state)
- **Blocks**: FM1V-50, FM1V-51

## writePaths

- `src/dsp/graph.rs`
- `src/dsp/ugen/mod.rs`
- `src/dsp/ugen/template.rs`
- `src/dsp/voice.rs`
- `src/dsp/ugen/build_helpers.rs`
- `src/dsp/ugen/mixer.rs`
- `src/dsp/ugen/catalog.rs`
- `src/dsp/ugen/catalog/voice_ports.rs`
- `src/dsp/ugen/catalog/codec.rs`
- `src/dsp/build.rs`
- `src/dsp/build/names.rs`
- `src/dsp/build/names/table.rs`
- `src/dsp/controls.rs`
- `src/dsp/meta.rs`
- `src/types/natives_domain.rs`
- `src/prelude/templates.vact`
- `src/dsp/tests/dsp/fm6_registry.rs`
- `src/host/tests/e2e/templates/fm_mod_algorithm.rs`
- `impl-plans/active/fm1-voices-40-fm-registry.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-40` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/ugen/template.rs:124-182`: `load`, `push_frame_payload`, the
  payload fields at lines 23-26 and 209-212, and the copy at 401-402.
- `src/dsp/voice.rs:538-560`: payload-node dispatch, with `Kx` built for
  `StageLinked`.
- `src/dsp/ugen/catalog/codec.rs:109-135` (encode) and `:255-296` (decode
  before the main match).
- `src/dsp/build.rs:765-770` (skip the payload arg as an edge) and
  `:846-853` (payload post-pass).
- `src/dsp/build/names.rs:362-388`: `catalog_alias` and `catalog_port`.
- `src/host/tests/e2e/templates/golden.rs`: `render_record` and
  `graph_record`.

## File-level Changes

### `fm-mod` algorithm mode

- `catalog.rs`: `FM_MOD` becomes
  `[IN, p("mod", 0.0), p("index", 1.0), p("algorithm", 0.0), FREQ, p("ratio", 1.0), p("velocity", 1.0)]`.
  - Use the same `IN` and `FREQ` constants the file already uses.
  - Check that `FREQ`'s default is 440. If it is not, use
    `p("freq", 440.0)`.
  - The indices must equal `engine::fm_mod_port`.
- `names/table.rs`: `UGenSpec::FmMod => &["carrier", "modulator", "algorithm", "freq", "ratio", "velocity"]`.
  The first two are unchanged and the rest are appended.
- `mixer.rs`: `Node::FmMod => fm::modulate_with_algorithm(ins, st, mem, out, kx)`.
- `build_helpers.rs`: `Node::FmMod => (fm::engine::STATE_FLOATS, 0)`.
- `controls.rs`: `param("algorithm", 21, 0.0, (0.0, 32.0))`.
- `templates.vact`: in the `fm` header only, change `algorithm: int = 5` to
  `algorithm: int = 0`. No other character changes in that block.

### `fm6-core`

- **`graph.rs`**: append
  `Fm6Core { patch: Option<Box<crate::dsp::ugen::fm::patch::Fm6Patch>> }`
  at the end of `UGenSpec`.
- **`ugen/mod.rs`**:
  - append `Fm6Core { slot: u8 }` to `Node`;
  - add `pub const NO_FM6_PATCH: u8 = u8::MAX;`;
  - add `Node::Fm6Core { .. }` to `is_env`;
  - add `BuildError::BadFm6Patch` with message
    `"invalid six-operator patch"`.
- **`template.rs`**:
  - add `n_fm6_payloads` and `fm6_payloads: [Fm6Patch; MAX_FM6_PAYLOADS]`
    to both `RawGraph` and `Template`, initialized with `Fm6Patch::EMPTY`
    and cleared in `clear`;
  - add `push_fm6_payload`. It returns `TooManyParams` past 4, mirroring
    `push_frame_payload`.
  - In `load`:
    - `UGenSpec::Fm6Core { patch: Some(p) }` validates
      (`BadFm6Patch` on error) and becomes `Node::Fm6Core { slot }`;
    - `patch: None` becomes `Node::Fm6Core { slot: NO_FM6_PATCH }`. This is
      valid: it is macro mode.
  - Copy the payloads in `build`.
  - Add `Node::Fm6Core { .. }` to the self-enveloped `envs` arm.
- **`voice.rs`**: before the generic mixer call, next to `StageLinked`,
  dispatch `Node::Fm6Core { slot }`:
  - build `Kx` the same way;
  - `patch = (slot != NO_FM6_PATCH).then(|| t.fm6_payloads.get(usize::from(slot))).flatten()`;
  - call `fm::engine::fm6_render(&ins, patch, &mut v.nodes[i], mem, out, &kx)`.
  - The file is 946 lines; keep it below 1000.
- **`mixer.rs`**: `Node::Fm6Core { .. } => out.fill(0.0)`. This is
  unreachable, as for `FrameKeyframe`.
- **`build_helpers.rs`**: `Node::Fm6Core { .. } => (fm::engine::STATE_FLOATS, 0)`.
- **`catalog.rs`**:
  - `ports` arm `Node::Fm6Core { .. } => FM6_CORE`;
  - `ugen_name` `"fm6-core"`;
  - append `"fm6-core"` to the end of `UGEN_NAMES`;
  - `node_of` maps to `Node::Fm6Core { slot: NO_FM6_PATCH }`.
- **`voice_ports.rs`**: `FM6_CORE` from `FM6_PORTS`.
- **`codec.rs`**:
  - Encode: `o.u8(105); o.u8(patch::WIRE_VERSION); o.u8(present as u8)`,
    then the 155 bytes if present. Validate first, returning `BadFm6Patch`.
  - Decode tag 105 before the main match:
    - check the version;
    - a present flag of 0 gives `NO_FM6_PATCH`;
    - a present flag of 1 reads 155 bytes, validates them
      (`FaultCode::BadRecord`) and pushes the payload
      (`FaultCode::GraphTooLarge`);
    - any other flag byte gives `BadRecord`.
- **`names.rs`**: append `("fm6-core", UGenSpec::Fm6Core { patch: None })`
  to the end of `UGENS`.
- **`names/table.rs`**: `Fm6Core { .. } => &["freq", "velocity", "algorithm", "ratio", "index", "fm6-feedback", "patch"]`.
- **`build.rs`**:
  - Extend the payload skip at about line 765 with
    `(matches!(spec, UGenSpec::Fm6Core { .. }) && &*pname == "patch")`.
    A non-list `patch:` gives `"patch: requires a constant numeric list"`.
  - In the post-pass:
    - `UGenSpec::Fm6Core { patch }` with `arg("patch")` as a `List(xs)`
      sets `Some(Box::new(Fm6Patch::from_flat(xs).map_err(LowerError::ty)?))`;
    - no `patch:` gives `None`.
- **`meta.rs`**:
  - `ugen_node` `"fm6-core" => Node::Fm6Core { slot: NO_FM6_PATCH }`;
  - range `fm6-feedback` 0..7, stepped;
  - `algorithm` uses the row range 0..32, stepped if the meta API supports
    it. Check the existing `fm` template meta still shows `algorithm`.
- **`natives_domain.rs`**: `dsp("fm6-core")` after the six FM1V-30 entries.

## Pitfalls

- **Long-running nextest (command timeout).**
  - Full nextest takes about 800 to 1700 s. The single test
    `complete::tests::robust::every_prefix_and_mutant_is_panic_free` takes
    about 500 s. Verification 4 excludes it; Verification 5 runs it.
  - The previous FM1V-30 run was killed by SIGTERM at about 1200 s
    (`tmp/fm1-voices/FM1V-30/focused-final-blessed.log`, exitStatus 100).
  - Run the lock-wrapped full nextest in the foreground with the executor
    command timeout set to at least 3600 s, or to its maximum. Poll it
    until it exits, and never background it.
  - A SIGTERM or harness kill is neither a pass nor a code failure. Rerun
    the same command once with the long timeout. Keep both logs as
    `tmp/fm1-voices/FM1V-40/attempt-<n>/full.log` and record both
    attempts.
  - Never skip, ignore or filter out tests in the full run.

- **Golden lines must stay byte-identical.** After this plan, run the
  golden test without blessing. `golden_digests.txt` must have no diff.
  These lines are pinned:
  - `graph fm baseline 29a87235be1d7b30 0`
  - `render fm center f84c6215540e7cb1 24064`
  - `render fm pan02 013b4c5a6d6773d5 24064`
  - every `six-bank-*` line

  If a pinned line fails, the cause is in this plan. Never bless.
- **Port order in `FM_MOD`.** Never insert before index 3; the existing edge
  port indices must not move.
- **Changing `table.rs` for `FmMod`** must not change positional mapping:
  `carrier` and `modulator` stay at 0 and 1.
- **Row 21 range.** It is `(0, 32)`. An explicit `> algorithm -3` takes the
  legacy branch either way:
  - FM1V-20's `modulate_with_algorithm` treats `round(a) <= 0` as legacy;
  - the event path may or may not clamp to the row range, which was not
    verified.

  Test the observable result (identical to the default). Do not rely on
  clamping.
- **Memory.** Adding fixed memory to `FmMod` shifts the clampable memory of
  later nodes only in instruments that also contain delays. The full suite
  proves that nothing shipped changes. Record the observation in the
  Progress Log.
- **The `patch:` payload must never become an edge** or a `node_params`
  entry.

## Tests

`src/host/tests/e2e/templates/fm_mod_algorithm.rs` (names contain `fm_mod`
or `fm6`):

- `fm_mod_unset_and_zero_are_identical`:
  `s :fm > note [:a3] > once` and `s :fm > algorithm 0 > note [:a3] > once`
  render bit-identically. `> algorithm -3` is also identical.
- `fm_mod_algorithms_render_distinct_and_bounded`: for N in 1..=32,
  `s :fm > algorithm N > note [:a3] > once` renders for 0.5 s. All 32 are
  pairwise distinct, finite, `rms > 1e-3` and `peak <= 1.0`.
- `fm_mod_user_inst_without_algorithm_stays_legacy`. Define
  `inst probe: fm-op freq ratio: 1 index: 1 > fm-mod {fm-op freq ratio: 2} > * amp`.
  It renders bit-identically with and without `> algorithm 0`, and differs
  from `> algorithm 5`.
- `fm6_core_let_bound_patch_renders`:
  - a top-level `let p [ ...155 synthetic ints... ]`;
  - then `inst t: fm6-core freq patch: p > * amp`;
  - then `s :t > note [:c4] > once`.

  The output is audible and bounded, and the voice finishes. Repeat with
  `let bank [p p]` and `patch: {bank 1}`. If the let-bound form cannot
  lower as a constant, record it, switch the test to a literal list, and
  report it for FM1V-50/51 (design "SysEx import" fallback).
- `fm6_core_bad_patch_reports_index`: 154 values gives an error containing
  `expected 155`. An OL of 100 gives an error naming the index.
- `fm6_core_macro_mode_without_patch`: `inst m: fm6-core freq > * amp` is
  audible. `algorithm: 5` and `algorithm: 9` differ.
- `fm6_completion_offers_core_and_native`: completion contains `fm6-core`
  and `fm6-sysex`.

`src/dsp/tests/dsp/fm6_registry.rs` (names contain `fm6`):

- `fm6_registry_fm_mod_ports_append_only`: the first three `FM_MOD` port
  names and defaults are unchanged, and indices 3..=6 equal `fm_mod_port`.
- `fm6_registry_fm_mod_implicit_binding_matches_kernel`. Install a
  single-node `FmMod` instrument, send algorithm 7, freq 220, ratio 14,
  index 2 and velocity 1, and render 64-frame blocks. The output equals
  `fm::modulate_with_algorithm` rendered directly with the same values.
- `fm6_registry_fm_template_memory`: `Template::from_inst` of the prelude
  `fm` gives `mem_total == engine::STATE_FLOATS`, and `MemExceeded` at one
  float less.
- `fm6_registry_codec_parity_with_and_without_patch`, following the
  `braids_struck.rs` pattern. The decoded nodes and payload equal the
  native ones, and the native and browser renders are finite and audible.
- `fm6_registry_payload_limit`: four patched `fm6-core` nodes build, and a
  fifth gives `TooManyParams`.
- `fm6_registry_codec_tag_is_105`.

## Verification (logs under `tmp/fm1-voices/FM1V-40/`)

1. `rustfmt --edition 2021 --check` on every `.rs` in writePaths, and
   `cargo run --quiet -- fmt --check src/prelude/templates.vact`. Both must
   exit 0.
2. `cmp tmp/fm1-voices/FM1V-40/golden_digests.before src/host/tests/e2e/templates/golden_digests.txt`
   must exit 0. Also,
   `test "$(git diff -U0 9ac8d1f -- src/host/tests/e2e/templates/golden_digests.txt | grep -c '^-[^-]')" = 0`
   must exit 0.
3. `test "$(diff tmp/fm1-voices/FM1V-40/templates.vact.before src/prelude/templates.vact | grep -c '^<')" = 1 && test "$(diff tmp/fm1-voices/FM1V-40/templates.vact.before src/prelude/templates.vact | grep -c '^>')" = 1`
   must exit 0. The one changed line must be the `fm` header, with
   `algorithm: int = 5` changed to `algorithm: int = 0`.
4. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6|fm_mod|fm_algo|golden|e2e::templates|catalog|codec|meta|controls|inst|complete|cross_mod/) - test(/complete::tests::robust/)' > tmp/fm1-voices/FM1V-40/focused.log 2>&1; echo "exit=$?"`
   The robust test is excluded here only. It still runs in the full nextest
   (Verification 5). The focused run should finish in under 15 minutes.
   must print `exit=0`, and the golden tests must pass.
5. The full nextest under the measurement lock (the same `bash -c` lock
   wrapper as FM1V-30 Verification 4, owner `FM1V-40`) must print `exit=0`
   with 0 failed.
6. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` must
   exit 0.
7. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   must exit 0.
8. `wc -l src/dsp/voice.rs src/dsp/build.rs src/dsp/ugen/template.rs src/dsp/ugen/catalog.rs src/dsp/ugen/catalog/codec.rs src/dsp/meta.rs`
   shows each below 1000.
9. Every path that is new or changed against `status-before.txt` is in
   writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

This plan runs alone (serial wave 4 of run 2). Never commit, stash,
checkout, reset or push.

- Fresh-read every file before editing it.
- Record pre- and post-edit `shasum -a 256` for `templates.vact`,
  `controls.rs`, `voice.rs` and `codec.rs`.
- If a file differs from what FM1V-30's Progress Log recorded, re-read it
  and apply only this plan's intent.

## Done Criteria

- [ ] `fm` algorithm 1..=32 is functional, and 0 or unset is bit-identical
      to before.
- [ ] `fm6-core` is registered with patch payloads, and native and browser
      parity holds.
- [ ] The golden file is unchanged.
- [ ] Verification 1-9 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.
