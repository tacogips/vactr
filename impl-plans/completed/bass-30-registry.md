# BASS-30: Register `bass-core` and the Six Bass Templates

**Status**: Completed
**Plan ID**: BASS-30 (wave 3; the only registry-owning plan)
**Design Reference**: `design-docs/specs/design-bass-voices.md`, sections "Registration and digest stability", "Controls", "Tempo sync" and "Verification" (e2e template tests)
**Created**: 2026-09-30
**Last Updated**: 2026-09-30

## Intent and Context

This plan makes `bass-core` (BASS-20) reachable from `.vact`. It appends six
prelude templates, makes them editor-visible with tuned defaults, feeds them
`cps`/`onset-time`, and adds their golden digests. It must not change any
existing template's output or graph digest.

Every ordered registry is **append-only**. Mirror how
`DigitalDrumCore`/`FeedbackMetal` are registered. Running
`grep -rn -e DigitalDrumCore -e digital-drum-core src` lists every site.

This plan also creates the offline-render test helper module that BASS-40
and BASS-41 use, plus their empty test files, so that wave 4 can run in
parallel without touching `templates.rs`.

## Non-goals

- No change to existing templates, rows in `controls.rs`, the scheduler
  logic, voice/engine code, `osc.rs`/`filter.rs`, or wf/syntax-fmt areas.
- No presets, examples or README (BASS-40/41).
- `THIRD_PARTY_NOTICES.md` stays unchanged.

## Dependencies

- **dependsOn**: BASS-20
- **Blocks**: BASS-40, BASS-41

## writePaths

- `src/dsp/graph.rs`
- `src/dsp/ugen/mod.rs`
- `src/dsp/ugen/template.rs`
- `src/dsp/ugen/build_helpers.rs`
- `src/dsp/ugen/mixer.rs`
- `src/dsp/ugen/catalog.rs`
- `src/dsp/ugen/catalog/voice_ports.rs`
- `src/dsp/ugen/catalog/codec.rs`
- `src/dsp/build/names.rs`
- `src/dsp/build/names/table.rs`
- `src/types/natives_domain.rs`
- `src/sched/commit.rs`
- `src/dsp/ring.rs`
- `src/dsp/meta.rs`
- `src/dsp/meta/templates.rs`
- `src/dsp/meta/templates/bass.rs`
- `src/ns/insts.rs`
- `src/prelude/templates.vact`
- `src/dsp/tests/dsp/seed_order.rs`
- `src/host/tests/e2e/templates.rs`
- `src/host/tests/e2e/templates/bass.rs`
- `src/host/tests/e2e/templates/bass_render.rs`
- `src/host/tests/e2e/templates/bass_presets.rs`
- `src/host/tests/e2e/templates/bass_examples.rs`
- `src/host/tests/e2e/templates/golden_digests.txt`
- `impl-plans/active/bass-30-registry.md`

## writePathNotes

Graph and node:

- path: `src/dsp/graph.rs` | intendedEdit: append the `UGenSpec::BassCore` unit variant as the last variant.
- path: `src/dsp/ugen/mod.rs` | intendedEdit: append `Node::BassCore` as the last variant, and add it to the `is_env` match.
- path: `src/dsp/ugen/template.rs` | intendedEdit: add `Node::BassCore` to the self-enveloped `envs += 1` arm (about line 474).
- path: `src/dsp/ugen/build_helpers.rs` | intendedEdit: `Node::BassCore => (bass_voice::STATE_FLOATS, 0)`.
- path: `src/dsp/ugen/mixer.rs` | intendedEdit: `Node::BassCore => bass_voice::render(ins, st, mem, out, kx)`.

Catalog and names:

- path: `src/dsp/ugen/catalog.rs` | intendedEdit: add a `Node::BassCore => BASS_CORE` arm in `ports()`, the `ugen_name` arm `"bass-core"`, and a `node_of` arm. Append `"bass-core"` to the end of `UGEN_NAMES`. Append the six template names to the end of `TEMPLATE_NAMES`, in this order: `analog-bass`, `acid-bass`, `fm-bass`, `wobble-bass`, `sub-bass`, `reese-bass`.
- path: `src/dsp/ugen/catalog/voice_ports.rs` | intendedEdit: add `pub(super) const BASS_CORE: &[Port]`, 32 `p(name, default)` entries in exactly the `bass_voice::PORTS` order and defaults.
- path: `src/dsp/ugen/catalog/codec.rs` | intendedEdit: encode `UGenSpec::BassCore => (98, 0)` and decode `98 => Node::BassCore`. First verify 98 is unused with `grep -n "98" src/dsp/ugen/catalog/codec.rs`; if it is taken, use the next free tag and record it.
- path: `src/dsp/build/names.rs` | intendedEdit: append `("bass-core", UGenSpec::BassCore)` to the end of `UGENS`.
- path: `src/dsp/build/names/table.rs` | intendedEdit: `UGenSpec::BassCore => &[...]`, the 32 port names in `PORTS` order.
- path: `src/types/natives_domain.rs` | intendedEdit: add `dsp("bass-core"),` next to `dsp("digital-drum-core")`.

Scheduler and runtime:

- path: `src/sched/commit.rs` | intendedEdit: add `| crate::dsp::graph::UGenSpec::BassCore` to the `wants_tempo_anchor` `matches!` (lines 529-539). This is the only edit in that file.
- path: `src/dsp/ring.rs` | intendedEdit: change `template_slots: 72` to `80`, and correct the stale comment to 73 prelude, 2 live-input and 4 quad-stem definitions.

Editor metadata:

- path: `src/dsp/meta.rs` | intendedEdit:
  - `ugen_node` arm `"bass-core" => Node::BassCore`.
  - In `template_meta`, map the six templates to `Node::BassCore` and add them to the custom-range branch. The exclusion chain is the list of `template != ...`; mirror `feedback-metal-drum`.
  - Add range entries for the custom names:
    - `gate-length` (0.05, 64);
    - `env-mod` (0, 8);
    - `env-decay` (0.01, 4);
    - `accent` (0, 1);
    - `slide-from` (-24, 24);
    - `slide-time` (0.005, 1);
    - `sub-level` (0, 1);
    - `fm-feedback` (0, 1);
    - `fold` (0, 1);
    - `bit-depth` (2, 16);
    - `click-level` (0, 1).
  - None are stepped.
  - Default overrides (see the pitfall below).
- path: `src/dsp/meta/templates.rs` | intendedEdit: add `mod bass;` and append six `TEMPLATE_PARAMS` entries, `("acid-bass", bass::ACID_BASS)` and so on.
- path: `src/dsp/meta/templates/bass.rs` | intendedEdit: new file.
  - one `pub(super) const` param list per template: `"freq"`, the header names in header order, then `"amp"` (imitate `src/dsp/meta/templates/plaits.rs`);
  - `pub(super) const DEFAULT_OVERRIDES: &[(&str, &str, &str)]`, the header defaults.
- path: `src/ns/insts.rs` | intendedEdit: change `TEMPLATE_NAMES: [&str; 67]` to `73` and append the same six names.

Prelude:

- path: `src/prelude/templates.vact` | intendedEdit: append six `inst` blocks at the **end** of the file.

Tests:

- path: `src/dsp/tests/dsp/seed_order.rs` | intendedEdit: add one test.
- path: `src/host/tests/e2e/templates.rs` | intendedEdit: add exactly four module lines in alphabetical position: `mod bass;`, `mod bass_examples;`, `mod bass_presets;`, `mod bass_render;`.
- path: `src/host/tests/e2e/templates/bass.rs` | intendedEdit: new file.
- path: `src/host/tests/e2e/templates/bass_render.rs` | intendedEdit: new file; shared helpers.
- path: `src/host/tests/e2e/templates/bass_presets.rs` | intendedEdit: new file; `//!` placeholder owned by BASS-40.
- path: `src/host/tests/e2e/templates/bass_examples.rs` | intendedEdit: new file; `//!` placeholder owned by BASS-41.
- path: `src/host/tests/e2e/templates/golden_digests.txt` | intendedEdit: blessed; 18 lines added, none removed.
- path: `impl-plans/active/bass-30-registry.md` | intendedEdit: Progress Log.

## sharedPaths

None.

## Read-only References

- `src/dsp/ugen/bass_voice.rs` (`PORTS`, `STATE_FLOATS`, `render`). This plan runs alone in wave 3.

## Template Bodies

Each body is one kernel call:

```
bass-core freq <every header name as name: name> mode: N
	> * amp
```

Header defaults (design "Controls" table):

- `analog-bass` (mode 0): `wave: keyword = :saw cutoff: float = 420 res: float = 0.35 env-mod: float = 2.5 env-decay: float = 0.18 accent: float = 0 sub-level: float = 0.35 drive: float = 0.25 gate-length: float = 0.9 amp-attack: float = 0.002 amp-decay: float = 0.4 sustain: float = 0.85 release: float = 0.04 slide-from: float = 0 slide-time: float = 0.06`
- `acid-bass` (mode 1): `wave :saw, cutoff 320, res 0.72, env-mod 3.2, env-decay 0.22, accent 0, slide-from 0, slide-time 0.06, drive 0.3, gate-length 0.55, release 0.02`
- `fm-bass` (mode 2): `ratio 1, index 2.5, fm-feedback 0.35, fold 0, bit-depth 16, cutoff 2400, res 0.1, env-mod 1, env-decay 0.15, accent 0, drive 0.1, gate-length 0.8, amp-decay 0.25, sustain 0.45, release 0.05, slide-from 0, slide-time 0.05`
- `wobble-bass` (mode 3): `wave :saw, detune 0.12, sub-level 0.4, cutoff 180, res 0.45, drive 0.35, lfo-wave :sine, lfo-rate 4, lfo-depth 0.8, lfo-offset 0, lfo-retrigger: bool = true, lfo-sync: bool = true, gate-length 4, release 0.08, slide-from 0, slide-time 0.08`
- `sub-bass` (mode 4): `wave :sine, cutoff 300, res 0, drive 0.15, click-level 0.2, gate-length 1.5, amp-attack 0.003, release 0.06, slide-from 0, slide-time 0.05`
- `reese-bass` (mode 5): `wave :saw, detune 0.18, sub-level 0.3, cutoff 650, res 0.2, env-mod 0.8, env-decay 0.6, drive 0.3, gate-length 3.5, amp-attack 0.01, release 0.12, slide-from 0, slide-time 0.1`

The file also carries the header comment block from the design:

- each template name and its model;
- that notes glide with `slide-from` (semitones from the previous note)
  under `cut 1`;
- that `gate-length` is in sixteenth-steps;
- the wobble `lfo-rate` division table (1/4 = 4, 1/8 = 8, 1/4T = 6,
  1/8T = 12).

## Pitfalls

- **Append only.**
  - The `inst` blocks, `UGEN_NAMES`, both `TEMPLATE_NAMES`, `UGENS` and the
    enum variants all go at the end.
  - Custom control ids are handed out in first-use order across
    `templates.vact`. Inserting a template mid-file shifts other templates'
    ids and changes their graph digests.
  - Never add a `controls.rs` row.
- **Default overrides.** `template_default_override` (`meta.rs` about
  line 181) only handles `Enum` rows, and the custom-port branch uses the
  catalog port default. Without a change the editor would show wrong
  defaults, such as `cutoff` 1200 on `acid-bass`. Make a minimal
  generalization:
  - Also search `bass::DEFAULT_OVERRIDES`. Expose it from
    `meta/templates.rs` with `pub(super) use`.
  - For `Float` rows and custom ports, parse the value as `f32`.
  - For `Bool` rows, `"true"` is 1.0 and `"false"` is 0.0.
  - Existing entries are all `Enum`, so their results must stay identical.
  - Only list overrides whose header default differs from the row or port
    default.
- **Implicit row-named ports.** A template that does not pass a row-named
  port gets the **row** default through `catalog::implicit_ctl`, not the
  `PORTS` default. For example `lfo-sync` has row default 0. This is
  harmless because `lfo-depth` has row default 0, but do not "fix" it by
  adding literals. Write it down in the `templates.vact` comment.
- Do not touch `src/dsp/graph/shape.rs`. The kernel is mono, which is the
  default.
- `src/dsp/ported/tests.rs` forbids module tokens in names. Run it; the
  bass names should pass.
- Bless procedure:
  1. `VACTR_BLESS_GOLDEN=1 cargo nextest run --run-ignored ignored-only -E 'test(/bless_golden_digests/)'`
  2. `git diff -U0 src/host/tests/e2e/templates/golden_digests.txt | grep -c '^-[^-]'`
     must print `0`.
  3. `git diff -U0 src/host/tests/e2e/templates/golden_digests.txt | grep -c '^+[^+]'`
     must print `18`.

  If any existing line changed, stop and find the cause. Never re-bless
  over it.

## `bass_render.rs` Helper Contract (used by BASS-40 and BASS-41; pin exactly)

- `pub(super) const SR: u32 = 48_000;`
- `pub(super) fn write_wav(rel: &str, left: &[f32], right: &[f32]) -> std::path::PathBuf`:
  - writes 16-bit PCM stereo to
    `<CARGO_MANIFEST_DIR>/tmp/bass/<rel>.wav`;
  - calls `create_dir_all` first;
  - clamps samples to +/-1;
  - panics with the path on an IO error.
- `pub(super) fn low_band_share(mono: &[f32]) -> f32` uses
  `crate::dsp::offline::spectrum(mono, 8192)`. Bin `k` is at
  `k * SR / (2 * 8192)` Hz (the FFT size is a power of two). It returns the
  power in 20..250 Hz divided by the power in 20 Hz..Nyquist, excluding DC.
- `pub(super) struct RenderCheck { pub rms: f32, pub peak: f32, pub low: f32, pub finite: bool }`
  and `pub(super) fn check(left: &[f32], right: &[f32]) -> RenderCheck`. It
  analyses the mono sum `(l + r) / 2`, but takes the peak over both
  channels.
- Tests in this file:
  - a 100 Hz sine gives `low_band_share >= 0.95`;
  - a 2 kHz sine gives `<= 0.05`;
  - `write_wav("selftest", ..)` creates a file whose first 4 bytes are
    `RIFF` and whose size is 44 + 4 * frames.
  - `check` on a 0.5-amplitude 60 Hz stereo sine gives `finite`,
    `peak` of 0.5 +/-0.01, and `low >= 0.95`.

  Every helper is used by a test, so no `dead_code` warning appears before
  wave 4.

## Tests in `src/host/tests/e2e/templates/bass.rs`

Use `E2e` from `src/host/tests/e2e.rs`.

- `bass_templates_are_registered_last`: the last six entries of both
  `TEMPLATE_NAMES` lists are the six names in order.
- `bass_catalog_ports_match_kernel_contract`: the catalog ports for
  `Node::BassCore`, `(name, default)` for i in 0..32, equal
  `bass_voice::PORTS`. `names/table.rs` has the same 32 names.
- `bass_templates_render_audible_and_bounded`: each template with
  `s :<t> > note [:c2 :c2 :g1 :c2] > d1`, 4 s via `run_stereo_for`, gives
  no faults, all finite, `rms > 1e-3` and `peak <= 1.0`.
- `bass_wobble_follows_tempo`: `use-bpm 120` against `use-bpm 150`, with
  `s :wobble-bass > note [:c1] > gate-length 64 > lfo-depth 1 > d1`, 4 s.
  The lag of the autocorrelation peak of 10 ms frame RMS matches
  `1 / (4 * cps)` s within 3 frames: 0.5 s at 120 BPM, 0.4 s at 150 BPM.
  This proves `commit.rs` delivers `cps`.
- `bass_slide_pattern_renders`:
  `s :acid-bass > note [:c2 :c3 :c2 :eb2] > slide-from [0 -12 12 -3] > accent [1 0 0 1] > cut 1 > d1`,
  2 s: no faults, finite, audible.
- `bass_editor_defaults_match_headers`: for every bass template, the editor
  default of each header param equals the header default. Use the metadata
  entry point that `src/dsp/tests/dsp/catalog.rs` exercises, and compare
  against a literal table copied from the header list above.

In `seed_order.rs`, add `bass_core_graph_uses_plain_ordinals`: a gate-free
`def(vec![UGenSpec::BassCore], vec![])` has `seed_ordinal(0) == 0`. Imitate
`gate_free_graph_uses_compiled_indices_as_ordinals`.

## Verification (evidence required)

Save logs under `tmp/logs/`.

1. `rustfmt --edition 2021 --check` on every `.rs` in writePaths must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass/) | test(/e2e::templates/) | test(/seed_order/) | test(/catalog/) | test(/live_input/) | test(/ported::tests/) | test(/inst/) | test(/codec/) | test(/contracts/)' > tmp/logs/bass-30-nextest.log 2>&1; echo "exit=$?"`
   must give `exit=0`. Record the test count; it must include the golden
   tests and `every_template_installs_and_renders_non_silent`.
3. The bless diff counts: `0` removed and `18` added.
4. `git diff --stat` touches only writePaths.
5. Line counts: `wc -l` of `ns/insts.rs`, `meta.rs`, `catalog.rs`,
   `voice_ports.rs`, `names/table.rs` and `meta/templates.rs`, each below
   1000.

## Done Criteria

- [x] All six templates are appended, registered, editor-visible and
      audible.
- [x] 18 golden lines added and 0 removed.
- [x] Verification 1-5 pass and are recorded.

## Progress Log

### Session: 2026-09-30 (plan created)
**Tasks Completed**: none


### Session: 2026-09-30 (BASS-30 implementation)
**Tasks Completed**: All BASS-30 Done Criteria.
**Notes**: Registered `BassCore` through graph/node, fixed-state allocation, mixer dispatch, catalog ports, codec tag 98, build names, native identifiers, and tempo/onset controls. Appended the six templates in the accepted order and raised template capacity to 80. Added bass editor parameter tables, header-default overrides for Float/Bool/Enum values, and the specified custom ranges. Added the bass-core seed-order test, registration/ports/render/wobble/slide/editor e2e coverage, shared offline WAV helper and its four self-tests, plus BASS-40/BASS-41 placeholder modules. Blessed exactly 18 new golden lines and removed none; `THIRD_PARTY_NOTICES.md` is unchanged.

**Verification**:
- `rustfmt --edition 2021 --check` over all BASS-30 owned Rust paths: passed, exit 0 (`tmp/logs/bass-30-registry-rustfmt.log`).
- `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/bass/) | test(/e2e::templates/) | test(/seed_order/) | test(/catalog/) | test(/live_input/) | test(/ported::tests/) | test(/inst/) | test(/codec/) | test(/contracts/)'`: passed, 439 run, 439 passed, 0 failed (`tmp/logs/bass-30-registry-nextest.log`).
- `CARGO_TERM_QUIET=true VACTR_BLESS_GOLDEN=1 NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --run-ignored ignored-only -E 'test(/bless_golden_digests/)'`: passed, 1 run, 1 passed, 0 failed (`tmp/logs/bass-30-registry-bless.log`). Final golden diff: 0 removed, 18 added (`tmp/bass-voices-232/BASS-30/final-audit.log`).
- `git diff --check`: exit 0. `THIRD_PARTY_NOTICES.md` diff: empty. All recorded Rust files are under 1000 lines; registry files are under their stricter limits.

**Repairs**: The first compile exposed a visibility issue for the bass default-override table and an editor-test reference mismatch; both were fixed. The first integrated e2e run then found `fm-bass.res` defaulted to the generic row value (0.3) instead of the pinned header value (0.1); added its override and reran all 439 selected tests successfully. Earlier complete logs are `tmp/bass-voices-232/BASS-30/bless.log` and `tmp/bass-voices-232/BASS-30/nextest.log`.

**Downstream**: Full workspace build/clippy/fmt/nextest and wasm gates belong to serial reconcile. Formal integrity/adversarial/integration review, BASS-40/BASS-41 implementation, final documentation reconciliation, commit, and push remain downstream workflow steps.

### Session: 2026-09-30 (session 232 closeout)
**Tasks Completed**: Accepted by fanout review and serial integration review (comm-003043). BASS-40 and BASS-41 are complete.
**Verification**: Combined-tree reconcile gates in `tmp/bass-voices-232/reconcile/wave-7/` all exit 0 (build, clippy, fmt check, mise lint, full nextest 1697 passed, wasm32 build). Digests 9/9 passed. `golden-readme-diff.log` shows 18 golden lines added, 0 removed.
**Remaining (non-blocking)**: No browser check of the editor template palette. The Step 8 move to `impl-plans/completed/` was denied by the sandbox and is still pending.
**Status**: Completed.
