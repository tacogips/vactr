# FM1V-30: Register the Six Voice Kernels and Templates

**Status**: In Progress (session 353 redispatch: audit and finish the partial edits, fix clippy)
**Plan ID**: FM1V-30 (run 3, serial wave 3 of 6; registry plan, before FM1V-40)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("Templates", "Registration and digest stability", "Implementation partition", "Verification" -> Voice kernels/E2e)
**Created**: 2026-10-10
**Last Updated**: 2026-10-10 (session 353)

## Session 353 Redispatch (run 3, read first)

Base is `48d651c`. FM1V-21 and FM1V-20 are accepted before this plan
starts. The session 352 jobs below still apply in full, with these run 3
changes:

- **Template slots are already fixed.** Commit `48d651c` set
  `template_slots: 102` (`src/dsp/ring.rs:811`, comment already says 79
  prelude + 2 live-input + 4 quad-stem) and `SONG_TEMPLATE_SLOTS = 134`
  (`src/host/song_profile.rs:12`). Do not change either value or the
  comment. Wherever this plan says "the value stays 96" or "fix the stale
  comment", read it as "the value stays 102 and the comment is already
  correct; verify only".
- **Fix strict clippy in `src/host/tests/e2e/templates/fm1_voices.rs`** and
  in any other writePath that has diagnostics. Fix the cause. Do not
  weaken an assertion, and do not add a blanket `#[allow]`. After FM1V-20,
  these should be the last WIP warnings in the tree. So this plan is the
  first where strict `cargo clippy --all-targets -- -D warnings` must exit
  0 (Verification 5).
- **Slot regression check.** Verification 4a (below) runs
  `tests/song_bus_memory_layout.rs` and `tests/song_host_profile.rs`. Both
  must pass with unchanged expectations (4/4 for `song_bus_memory_layout`).
  These files are read-only for this plan. If they fail, the template
  count drifted from 79. Fix the registry, not the test.
- Logs for this attempt go under `tmp/fm1-voices/FM1V-30/attempt-02/`.
  Save `status-before.txt` and `hashes-before.txt` fresh there.

## Session 352 Redispatch (historical; steps 1-6 still apply)

The previous attempt timed out. Its partial, unreviewed edits are committed
at `25d3e80`. A quick inventory, which you must verify and not trust:

- `TEMPLATE_NAMES: [&str; 79]` in `src/ns/insts.rs`;
- six `inst` blocks at the end of `src/prelude/templates.vact` (lines
  ~448-470);
- codec tags 106..111 in `src/dsp/ugen/catalog/codec.rs` (encode ~94-99,
  decode ~348-353);
- `TonewheelCore | HurdyGurdyCore` in `src/sched/commit.rs`
  `wants_tempo_anchor`;
- the corrected `template_slots` comment in `src/dsp/ring.rs`;
- `src/dsp/meta/templates/voices.rs`;
- `src/dsp/tests/dsp/fm1_voice_registry.rs` (110 lines);
- `src/host/tests/e2e/templates/fm1_voices.rs` (430 lines);
- 18 added lines in `golden_digests.txt` (0 removed against `9ac8d1f`).

Session 349 logs show compile errors in `fm1_voice_registry.rs` and
`fm1_voices.rs` at some point
(`tmp/fm1-voices/FM1V-21/nextest-final-attempt.log`).

Do this in order:

1. **Snapshot.** Save `git status --porcelain=v1` to
   `tmp/fm1-voices/FM1V-30/status-before.txt`. Record `shasum -a 256` of
   every writePath file to `tmp/fm1-voices/FM1V-30/hashes-before.txt`.
2. **Audit, file by file.** Compare each writePath against "File-level
   Changes" below. For each file, write one Progress Log line:
   `path -> done / missing <item> / wrong <item> -> action`. Use
   `git diff 9ac8d1f -- <path>` to see exactly what this run added.
3. **Backward-compatibility audit.** Each command below must exit 0. Base
   `9ac8d1f` is the pre-run tree.
   - `test "$(git diff -U0 9ac8d1f -- src/host/tests/e2e/templates/golden_digests.txt | grep -c '^-[^-]')" = 0`
   - `test "$(git diff -U0 9ac8d1f -- src/prelude/templates.vact | grep -c '^-[^-]')" = 0`
     No pre-run template line changed. FM1V-40's later `fm` header edit is
     not yet present.
   - `test "$(git diff -U0 9ac8d1f -- src/dsp/ugen/catalog/codec.rs | grep -c '^-[^-]')" = 0`
     No codec arm was removed or renumbered.
   - The appended `TEMPLATE_NAMES` and `UGEN_NAMES` entries come after every
     pre-run entry. The `fm1_voices_registered_after_bass` test proves this.
4. **Finish or redo.** Complete every missing or wrong item inside
   writePaths. Redoing a file is allowed. The result must still pass step 3.
5. **Golden lines.** The 18 new lines may be re-blessed only if a new
   template's graph or render changed during step 4. Never re-bless because
   a pre-run line differs; fix the cause instead. After blessing, run the
   step 3 checks again. Also confirm that exactly 18 lines are added against
   `9ac8d1f`:
   `test "$(git diff -U0 9ac8d1f -- src/host/tests/e2e/templates/golden_digests.txt | grep -c '^+[^+]')" = 18`.
6. **Run Verification 1-8** (as revised below) and record it. Read the
   "Long-running nextest" pitfall before running the focused run (3) or the
   full run (4).

Optional early run: right after step 2, you may run the full nextest once
(same lock wrapper, log `tmp/fm1-voices/FM1V-30/attempt-0/full.log`). This
finds real failures before the finish/redo work, so a late timeout does not
lose all evidence. It does not replace Verification 4.

This plan runs alone. FM1V-20 and FM1V-21 are accepted before it starts,
so any failure in the full suite is real. Fix it if it is inside
writePaths. Otherwise record it with the log path and set the plan Blocked.

## Intent and Context

This plan makes the six new kernels (FM1V-13..18) reachable from `.vact`:

- graph nodes, catalog ports, codec tags 106..111, lowering names, natives
  entries and editor metadata;
- six prelude templates, appended at the end of `templates.vact`;
- `cps` delivery to the organ and the hurdy-gurdy;
- 18 new golden digest lines.

**No existing template's graph or render digest may change.** Every ordered
registry is append-only. Mirror how `BassCore` was registered:
`grep -rn -e BassCore -e bass-core src` lists every site, and
`impl-plans/completed/bass-30-registry.md` is the precedent.

Mapping from kernel to `UGenSpec`/`Node`, UGen name, codec tag and
template:

| Kernel | Variant | UGen name | Tag | Template |
|--------|---------|-----------|----:|----------|
| kalimba | `KalimbaCore` | `kalimba-core` | 106 | `kalimba` |
| tonewheel | `TonewheelCore` | `tonewheel-core` | 107 | `tonewheel-organ` |
| hurdy_gurdy | `HurdyGurdyCore` | `hurdy-gurdy-core` | 108 | `hurdy-gurdy` |
| vosim | `VosimCore` | `vosim-core` | 109 | `vosim` |
| gendyn | `GendynCore` | `gendyn-core` | 110 | `gendyn` |
| scanned | `ScannedCore` | `scanned-core` | 111 | `scanned` |

Tag 105 is reserved for `fm6-core` (FM1V-40). Never renumber an existing
tag.

## Non-goals

- No FM changes: `fm-mod`, `fm6-core`, row 21 and the `fm` template belong
  to FM1V-40.
- No examples or docs (FM1V-50/51).
- No `controls.rs` rows.
- No change to `template_slots` (102) or `SONG_TEMPLATE_SLOTS` (134). Both
  were set in `48d651c`, so the free-slot headroom from before the six
  voices is kept. 79 + 2 + 4 = 85, or 87 with the stage-linked example.

## Dependencies

- **dependsOn**: FM1V-13, FM1V-14, FM1V-15, FM1V-16, FM1V-17, FM1V-18
  (accepted at `25d3e80`); FM1V-20 (serial order of run 2, so the full suite
  is green when this plan is accepted)
- **Blocks**: FM1V-40

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
- `src/dsp/build/names/table/voices.rs` (create it only if `table.rs`
  would reach 1000 lines)
- `src/types/natives_domain.rs`
- `src/sched/commit.rs`
- `src/dsp/ring.rs`
- `src/dsp/meta.rs`
- `src/dsp/meta/templates.rs`
- `src/dsp/meta/templates/voices.rs` (new)
- `src/ns/insts.rs`
- `src/prelude/templates.vact`
- `src/dsp/tests/dsp/fm1_voice_registry.rs`
- `src/host/tests/e2e/templates/fm1_voices.rs`
- `src/host/tests/e2e/templates/bass.rs`
- `src/host/tests/e2e/templates/golden_digests.txt`
- `impl-plans/active/fm1-voices-30-voice-registry.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-30` (artifact root)

## sharedPaths

None. This plan is the only owner of these files in its wave.

## Read-only References

- The kernels' `PORTS`, `STATE_FLOATS` and `render`.
- `impl-plans/completed/bass-30-registry.md`, for the pitfalls and the
  bless procedure.
- `src/dsp/tests/dsp/braids_struck.rs:95-130`: native-versus-browser codec
  parity at 44.1/48/96 kHz with 64/256-frame blocks.
- `src/host/tests/e2e/templates/six_op_original.rs:89-105`: the
  `MemExceeded` at one-less-float test.
- `src/complete/tests/rank.rs`: `complete(text, cursor, &Snapshot::builtin(), limit)`.

## File-level Changes

**Graph and node:**

- `graph.rs`: append the six unit variants at the end of `UGenSpec`.
- `ugen/mod.rs`:
  - append the six `Node` variants;
  - add `KalimbaCore | TonewheelCore | HurdyGurdyCore` to `is_env`.
- `template.rs`: add the same three to the self-enveloped `envs += 1` arm
  (near line 474).
- `build_helpers.rs`: `Node::X => (x::STATE_FLOATS, 0)` for all six.
- `mixer.rs`: `Node::X => x::render(ins, st, mem, out, kx)` for all six.

**Catalog and names:**

- `catalog.rs`:
  - `ports()` arms;
  - `ugen_name` arms;
  - `node_of` arms;
  - append the six UGen names to the end of `UGEN_NAMES`;
  - append `kalimba`, `tonewheel-organ`, `hurdy-gurdy`, `vosim`, `gendyn`,
    `scanned`, in that order, to the end of `TEMPLATE_NAMES`.
- `voice_ports.rs`: six `pub(super) const` port lists. Each is `p(name,
  default)` in exactly the kernel `PORTS` order. Use the existing `FREQ`
  constant for `freq` only if its default is 440.
- `codec.rs`: `UGenSpec::X => (tag, 0)` in `put_spec`, and the `tag =>
  Node::X` decode arms.
- `names.rs`: append six `("name", UGenSpec::X)` entries to the end of
  `UGENS`.
- `names/table.rs`: six arms listing the `PORTS` names in order.
  - If the file would reach 1000 lines, move the new arms into
    `src/dsp/build/names/table/voices.rs` and record that. The path is
    already declared in writePaths.
- `natives_domain.rs`: add six `dsp("...")` entries directly after
  `dsp("granular")`, before the effects block.

**Runtime:**

- `commit.rs`: add `| UGenSpec::TonewheelCore | UGenSpec::HurdyGurdyCore` to
  the `wants_tempo_anchor` `matches!`. This is the only edit in this file.
- `ring.rs`: verify only. The `template_slots` comment already says 79
  prelude, 2 live-input and 4 quad-stem definitions, and the value is 102
  (`48d651c`). Do not edit.

**Metadata:**

- `meta.rs`:
  - `ugen_node` arms;
  - `template_meta` template-to-node arms;
  - add the six templates to the exclusion chain near lines 309-328;
  - custom port ranges (mark integer selectors as stepped):

    | Port | Range | Stepped |
    |------|-------|---------|
    | `kalimba-beat` | 0..8 | |
    | `kalimba-hardness`, `kalimba-damping`, `kalimba-body`, `kalimba-buzz` | 0..1 | |
    | `kalimba-decay` | 0.1..10 | |
    | `drawbar1`..`drawbar9` | 0..8 | yes |
    | `organ-click` | 0..1 | |
    | `organ-perc` | 0..2 | yes |
    | `organ-perc-slow`, `organ-perc-soft`, `organ-perc-trigger` | 0..1 | yes |
    | `organ-vibrato` | 0..6 | yes |
    | `gate-length` | 0.05..64 | reuse the bass entry if keyed by name |
    | `gurdy-wheel`, `gurdy-pressure`, the four string levels, `gurdy-buzz`, `gurdy-buzz-threshold`, `gurdy-stroke-depth` | 0..1 | |
    | `gurdy-drone-key` | 24..72 | yes |
    | `gurdy-strokes` | 0..16 | yes |
    | `vosim-formant` | 100..8000 | |
    | `vosim-pulses` | 1..8 | yes |
    | `vosim-decay` | 0..1 | |
    | `gendyn-points` | 3..32 | yes |
    | `gendyn-amp-step`, `gendyn-dur-step`, `gendyn-spread` | 0..1 | |
    | `gendyn-dist` | 0..3 | yes |
    | `scan-stiffness`, `scan-damping`, `scan-centering`, `scan-position` | 0..1 | |
    | `scan-hammer` | 0.02..1 | |
    | `scan-update` | 50..2000 | |

  - Only if a header default differs from its port default, wire a
    `voices::DEFAULT_OVERRIDES` into both lookups, as bass does. The headers
    below equal the port defaults, so none is expected.
- `meta/templates.rs`: add `mod voices;` and six `TEMPLATE_PARAMS` entries.
- `meta/templates/voices.rs`: six `pub(super) const` lists. Each is
  `"freq"`, the header names in header order, `"attack"`/`"decay"`/
  `"sustain"`/`"release"` for `vosim`/`gendyn`/`scanned` only, then `"amp"`.
- `insts.rs`: `TEMPLATE_NAMES: [&str; 79]`, with the same six names
  appended.

**Prelude** (`templates.vact`): append the six `inst` blocks at the end of
the file. Each block has a one-line `#` comment, and its header defaults
equal the `PORTS` defaults:

- `inst kalimba kalimba-beat: float = 1.5 kalimba-hardness: float = 0.5 kalimba-decay: float = 2.5 kalimba-damping: float = 0.5 kalimba-body: float = 0.3 kalimba-buzz: float = 0:`
  with body `kalimba-core freq kalimba-beat: kalimba-beat ... > * amp`,
  where every header is passed as `name: name`;
- `tonewheel-organ`: headers `drawbar1`..`drawbar9` (8 8 8 0 0 0 0 0 0),
  `organ-click` 0.3, `organ-perc` 0, `organ-perc-slow` 0, `organ-perc-soft`
  0, `organ-perc-trigger` 1, `organ-vibrato` 0, `gate-length` 4.
  Body: `tonewheel-core freq ... > * amp`.
- `hurdy-gurdy`: every `gurdy-*` header with its `PORTS` default, plus
  `gate-length` 8. Body: `hurdy-gurdy-core freq ... > * amp`.
- `vosim`: `vosim-formant` 900, `vosim-pulses` 3, `vosim-decay` 0.7. Body:
  `vosim-core freq ... > * {env-adsr attack decay sustain release} > * amp`.
- `gendyn`: the five `gendyn-*` headers, with the same body shape as
  `vosim`.
- `scanned`: the six `scan-*` headers, with the same body shape as `vosim`.

Do not pass `cps`, `onset-time` or `velocity`. They bind implicitly to the
row controls.

## Pitfalls

- **Long-running nextest (command timeout). This is why the previous
  attempt timed out.**
  - Full nextest takes about 800 to 1700 s. The single test
    `complete::tests::robust::every_prefix_and_mutant_is_panic_free` takes
    about 500 s.
  - The previous attempt's focused run (filter term `complete`) was killed
    by SIGTERM at 1203.6 s with 569 of 570 passed
    (`tmp/fm1-voices/FM1V-30/focused-final-blessed.log`, exitStatus 100).
    Verification 3 now excludes the robust test.
  - Run the lock-wrapped full nextest in the foreground with the executor
    command timeout set to at least 3600 s, or to its maximum. Poll it
    until it exits, and never background it.
  - A SIGTERM or harness kill is neither a pass nor a code failure. Rerun
    the same command once with the long timeout. Keep both logs as
    `tmp/fm1-voices/FM1V-30/attempt-<n>/full.log` and record both
    attempts.
  - Never skip, ignore or filter out tests in the full run.

- **Append only.** Inserting a template mid-file shifts other templates'
  custom control ids, and therefore their graph digests.
- **Run the formatter on the prelude.** `templates.vact` must stay a
  formatter fixed point:
  `CARGO_TERM_QUIET=true cargo run --quiet -- fmt --check src/prelude/templates.vact`.
- **Bless procedure.**
  1. `CARGO_TERM_QUIET=true VACTR_BLESS_GOLDEN=1 NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --run-ignored ignored-only -E 'test(/bless_golden_digests/)'`
  2. `git diff -U0 9ac8d1f -- src/host/tests/e2e/templates/golden_digests.txt | grep -c '^-[^-]'`
     must print `0`.
  3. `git diff -U0 9ac8d1f -- src/host/tests/e2e/templates/golden_digests.txt | grep -c '^+[^+]'`
     must print `18`.

  The base is `9ac8d1f`, because the first 18 lines are already committed
  at `25d3e80`. If any pre-run line changed, restore the file from
  `tmp/fm1-voices/FM1V-30/golden_digests.before` (copy it there in step 1).
  Never use `git checkout`. Find the cause and fix it. Never re-bless over
  a changed line.
- **Name rules.** `src/dsp/ported/tests.rs` bans upstream module tokens in
  names. Run it.
- **Leave `fm` alone.** Do not touch `FM_MOD`, `fm.rs`, `controls.rs` or the
  `fm` template here.

## Tests

`src/host/tests/e2e/templates/fm1_voices.rs` (names contain `fm1_voices`):

- `fm1_voices_registered_after_bass`. In both `TEMPLATE_NAMES` lists, the
  last six entries are the new names in order, and the six before them are
  the bass names.
- `fm1_voices_catalog_ports_match_kernels`. For each node, the catalog
  `(name, default)` pairs equal the kernel `PORTS`, and `names::ports(spec)`
  lists the same names.
- `fm1_voices_templates_render_audible_and_bounded`. For each template,
  render `s :<t> > note [:c4 :e4 :g4 :c5] > d1` for 2 s. There are no
  faults, every sample is finite, `rms > 1e-3` and `peak <= 1.0`.
- `fm1_voices_organ_and_gurdy_follow_tempo`. Render
  `s :tonewheel-organ > note [:c4] > gate-length 8 > once` at `use-bpm 120`
  and at `use-bpm 150`. The last sample with `|y| > 1e-4` lies at about
  1.0 s and about 0.8 s respectively, within 20 ms. Repeat for
  `hurdy-gurdy`. Because of the string decay tail, assert something
  different there: the 120 BPM end time minus the 150 BPM end time is
  0.2 s +/- 50 ms.
- `fm1_voices_mem_exceeded_one_float_less`. For each template,
  `Template::from_inst` succeeds with `voice_mem = mem_total` and fails with
  `MemExceeded` at `mem_total - 1`. `mem_total` is at most 24_000.
- `fm1_voices_editor_metadata`. Each template's editor params list matches
  `voices.rs`. Each header default equals the editor default. The stepped
  flags match the table above.
- `fm1_voices_completion_offers_templates_and_ugens`:
  `complete("s :kal", 6, ..)` contains `:kalimba`, and
  `complete("scanned-c", 9, ..)` contains `scanned-core`. Check the same for
  all six templates and UGens.

`src/host/tests/e2e/templates/bass.rs`: rename
`bass_templates_are_registered_last` to
`bass_templates_precede_fm1_voices`. It asserts the six bass names occupy
positions `len-12..len-6` of both lists, so the guarantee is unchanged but
moved.

`src/dsp/tests/dsp/fm1_voice_registry.rs` (names contain `fm1_voice`):

- `fm1_voice_native_and_browser_codec_parity`. For each of the six
  single-node instruments:
  - `mem_total == STATE_FLOATS`;
  - the decoded browser template's nodes equal the native ones;
  - native and browser renders at 44.1/48/96 kHz with 64/256-frame blocks
    are finite and audible (`rms > 1e-4`; give vosim, gendyn and scanned a
    held gate);
  - the rigs' built-in zero-allocation assertion holds.
- `fm1_voice_codec_tags_are_106_to_111`: encode each, and check the tag
  byte.

## Verification (logs under `tmp/fm1-voices/FM1V-30/`)

1. `rustfmt --edition 2021 --check` on every `.rs` in writePaths must exit 0,
   and `cargo run --quiet -- fmt --check src/prelude/templates.vact` must
   exit 0.
2. Against `9ac8d1f`, the bless counts are `0` removed and `18` added.
   Also, `golden_digests.txt`, `templates.vact` and `codec.rs` have `0`
   removed lines (Redispatch step 3).
2a. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/golden/)' > tmp/fm1-voices/FM1V-30/golden.log 2>&1; echo "exit=$?"`
   must print `exit=0`, without `VACTR_BLESS_GOLDEN`. This shows the
   committed digests match the rendered output.
3. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm1_voice|bass|e2e::templates|catalog|codec|seed_order|ported::tests|inst|meta|complete|live_input|contracts/) - test(/complete::tests::robust/)' > tmp/fm1-voices/FM1V-30/focused.log 2>&1; echo "exit=$?"`
   The `- test(/complete::tests::robust/)` set difference keeps the
   roughly 500 s robust test out of the focused run. It still runs in the
   full nextest (Verification 4). The focused run should finish in under
   15 minutes.
   must print `exit=0`. Record the test count; it must include the golden
   test.
4. Run the full suite under the measurement lock, in one shell:
   `bash -c 'L=/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock; until mkdir $L 2>/dev/null; do sleep 30; done; trap "rm -rf $L" EXIT; echo FM1V-30 > $L/owner; CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/fm1-voices/FM1V-30/full.log 2>&1; echo "exit=$?"'`.
   It must print `exit=0` with 0 failed. No other plan is in flight. A
   failure outside writePaths is a real defect: record it with the log
   path and set the plan Blocked. Do not edit other plans' files.
4a. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --test song_bus_memory_layout --test song_host_profile > tmp/fm1-voices/FM1V-30/attempt-02/slots.log 2>&1; echo "exit=$?"`
   must print `exit=0`. `song_bus_memory_layout` must show 4 run and 4
   passed. Also `grep -n 'template_slots: 102' src/dsp/ring.rs` and
   `grep -n 'SONG_TEMPLATE_SLOTS: usize = 134' src/host/song_profile.rs`
   must each print one line.
5. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings > tmp/fm1-voices/FM1V-30/attempt-02/clippy.log 2>&1; echo "exit=$?"`
   must print `exit=0`. This is the first plan where strict clippy is
   gating (session 353).
6. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   must exit 0.
7. `wc -l` of `catalog.rs`, `voice_ports.rs`, `codec.rs`, `names/table.rs`,
   `meta.rs`, `meta/templates.rs`, `insts.rs`, `template.rs` and `mod.rs`
   must each be below 1000.
8. Every path that is new or changed against `status-before.txt` is in
   writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

- Fresh-read each registry file right before editing it.
- Record pre- and post-edit `shasum -a 256` for `templates.vact`,
  `golden_digests.txt`, `catalog.rs` and `codec.rs`.
- This plan runs alone (serial wave 3 of run 2). Never commit, stash,
  checkout, reset or push.

## Done Criteria

- [ ] The per-file audit of the `25d3e80` partial edits is recorded in the
      Progress Log.
- [ ] Six kernels and templates are registered, editor-visible, completable
      and audible.
- [ ] Against `9ac8d1f`: 18 golden lines added and 0 removed; 0 removed
      lines in `templates.vact` and `codec.rs`; tag 105 unused.
- [ ] Clippy diagnostics in `fm1_voices.rs` and the other writePaths are
      fixed; strict clippy exits 0.
- [ ] Verification 1-8 (and 2a, 4a) pass on base `48d651c` and are recorded
      under `tmp/fm1-voices/FM1V-30/attempt-02/` (session 353).

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.
