# MTX-91: Third-party notices, verification paths and final drop-in gates

**Status**: Completed (accepted in session 605, 2026-11-01)
**Plan ID**: MTX-91 (wave W5, alone)
**Design Reference**: `design-docs/specs/design-music-things-extraction.md` sections 7, 9.1 (gates 1-10), 10
**Protocol**: `impl-plans/active/music-things-protocol.md` (read first)
**Created**: 2026-10-31
**Last Updated**: 2026-10-31

## Intent and Context

The operator will copy `music-things/crates/*` and a staged notices file
into https://github.com/tacogips/music-things. This plan:

- writes `music-things/THIRD_PARTY_NOTICES.md` for all moved code;
- rewrites the moved-file paths in vactr's `THIRD_PARTY_NOTICES.md`;
- updates one informational path in `verification/compare_tides_poly.py`;
- runs the final gates: no duplication (gate 9) and the standalone
  drop-in build (gate 10), plus the full gate set on the finished tree.

## Non-goals

- Any Rust source change. A gate failure caused by an earlier plan is
  recorded as a blocker with file, cause and suggested fix, for serial
  repair by the review step.
- `mise.toml` (MTQ1).
- Rewriting historical design docs.
- Removing any section from vactr's notices.

## Dependencies

- dependsOn: MTX-S3, MTX-90 (MTX-90 transitively covers F1-F3; S3
  covers S1-S2)
- Blocks: none

## File-level changes

1. **`music-things/THIRD_PARTY_NOTICES.md` (new).**
   - Start with a title and a short preamble: what these crates contain,
     and that the notices cover adaptations and translations used in
     them.
   - Copy each section of `THIRD_PARTY_NOTICES.md` from "Streams
     control-algorithm research reference" (line 26) through the end,
     including the full Apache License 2.0 text. Copy the Mutable
     Instruments MIT permission text verbatim wherever it appears.
   - Omit "Optional local source-comparison probe" (lines 3-25).
   - Add a section "musicdsp research references". It states that the
     audit reference is `bdejong/musicdsp` at
     `d7c8525c9b3aa007b34caa682624f4e7a28c7264`, that the ideas were
     implemented independently, that no snippet or attachment was copied,
     and that one Univibe entry states GPL-2.0 and was not used. Source:
     `design-docs/specs/design-musicdsp-effects.md:11-17`.
   - Rewrite paths to be relative to the music-things repository root:
     - `src/dsp/ugen/<f>` -> `crates/music-things-synth/src/<f>`
     - `src/dsp/ugen/vactrol_gate.rs` -> `crates/music-things-synth/src/lpg.rs`
     - `src/dsp/effects/<f>` -> `crates/music-things-fx/src/<f>`
     - `src/dsp/granular.rs` -> `crates/music-things-fx/src/granular.rs`
     - `src/dsp/elements_ominous.rs` -> `crates/music-things-synth/src/elements_ominous.rs`
     - `src/dsp/effects/prim.rs` and `src/dsp/fft.rs` -> `crates/music-things-dsp/src/…`
   - References to `src/dsp/ported/*`, `verification/*`, `examples/*` or
     `mise` tasks become "the host application's coverage manifest and
     comparison tooling (not part of these crates)". Do not name the host.
   - Scrub banned tokens: "Vactr" or "Vactr's" -> "this implementation" or
     "the"; catalog names containing `vactr` such as `stream-vactr` ->
     describe the algorithm ("the stream optocoupler-style control").
     Keep every provenance statement (adapted versus original, what was
     not imported, pinned revisions, copyright lines) unchanged in
     meaning.
2. **vactr `THIRD_PARTY_NOTICES.md`.**
   - Rewrite each of the 33 `src/dsp/...` path references whose file
     moved, using the same mapping prefixed with `music-things/`. Paths
     under `src/dsp/ported/*` stay.
   - Add one sentence after the title: "Since 2026-10-31 the DSP sources
     covered below live in the in-tree crates under `music-things/crates/`;
     their staged notices are in `music-things/THIRD_PARTY_NOTICES.md`."
   - Remove no section.
3. **`verification/compare_tides_poly.py:444`.**
   - Change `"src/dsp/ugen/tidal_poly.rs lines 111-143 …"` to
     `"music-things/crates/music-things-synth/src/tidal_poly.rs lines A-B …"`.
     A-B are the new line numbers of the same clamp code; find them with
     `grep -n` in the crate file.
   - No other change to that file.

## Final gates (design 9.1)

- **Gate 9: no duplication.** For each module basename moved by MTX-00/S*/F*
  (the lists in those plans), check that no file of that name exists
  under `src/dsp/`, except the adapters and shims:
  - `ugen/{sample,wavetable,va_filter,stage_chain,fm_pair,analog_pair,chord_pair,table_terrain_pair,terrain_pair,string_machine_pair,shape_pair,dual_kick,snare_pair,hat_pair,voice_layer,vactrol_gate}.rs`
  - `ugen/digital_drum/mod.rs`
  - `src/dsp/granular.rs`

  For those adapters and shims, `grep -nE '^\s*(for|while|loop)\b'` must
  print nothing except the granular adapter's `match`. Log the review.
- **Gate 10: standalone drop-in.**
  1. `mkdir -p tmp/music-things/MTX-91/standalone/crates`, then
     `cp -R music-things/crates/music-things-dsp music-things/crates/music-things-synth music-things/crates/music-things-fx tmp/music-things/MTX-91/standalone/crates/`.
  2. Write `tmp/music-things/MTX-91/standalone/Cargo.toml`
     containing only `[workspace]`, `members = ["crates/*"]` and
     `resolver = "2"`.
  3. With
     `CARGO_TARGET_DIR=tmp/music-things/MTX-91/standalone/target`,
     run:
     - `cargo build --workspace --manifest-path tmp/music-things/MTX-91/standalone/Cargo.toml`
     - the same with `--target wasm32-unknown-unknown`
     - `cargo test --workspace --manifest-path …`

     All must exit 0, and the test count must equal the number of crate
     tests in `cargo nextest list --workspace` for the three
     `music-things-*` binaries.
  4. `diff -r music-things/crates/music-things-fx tmp/music-things/MTX-91/standalone/crates/music-things-fx`
     (and the same for the other two) reports no difference.

## Pitfalls

- **Banned tokens in notices.** The staged notices file is inside
  `music-things/`, so V-grep applies to it. Words like "Vactr's" appear
  many times in the source sections. Scrub each one and preserve the
  meaning.
- **Licence texts.** Do not paraphrase licence texts or copyright lines.
- **Standalone copy.** Do not run cargo in `music-things/crates/*`
  without `--manifest-path` pointing at the standalone copy. A plain
  `cargo build` there uses the vactr workspace.
- **Lockfile.** The standalone `Cargo.lock` is generated inside the tmp
  dir (artifact root) and must not be copied back.

## Verification (logs under `tmp/music-things/MTX-91/`)

1. V-grep, which now covers the staged notices.
2. `grep -n 'src/dsp/' THIRD_PARTY_NOTICES.md`: every remaining hit is a
   `src/dsp/ported/…` or other non-moved path. Log the list.
3. `grep -c 'Permission is hereby granted' THIRD_PARTY_NOTICES.md music-things/THIRD_PARTY_NOTICES.md`:
   the staged count is at least 1, and the `Apache License` heading is
   present in both.
4. `python3 -I -m py_compile verification/compare_tides_poly.py`: exit 0.
5. Gate 9 and gate 10 (above).
6. The full gate set on the final tree:
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - V-nextest (total and V-names `unmatched=0`)
   - V-wasm, and V-crate-wasm for all three crates
   - V-examples
   - `cargo check --features lsp`
   - `cargo build --bin vactr`
   - V-protected
   - V-vitest (at least 2384)
7. `git diff --stat a6e5bea -- mise.toml editor src/pattern src/sched src/host/wire*`:
   empty (scope guard).

## Completion Criteria

- [x] The staged notices exist, are scrubbed, and contain the MIT and
      Apache texts and the musicdsp note.
- [x] vactr's notices paths are accurate.
- [x] The `compare_tides_poly.py` path and line numbers are updated.
- [x] Gates 9 and 10 pass, and the full gate set passes, with logs.
- [x] Any earlier-plan defect is recorded as a blocker, not fixed here.
      The Progress Log is updated.

## Progress Log

### Session 2026-11-01 (MTX-91)
- Rewrote all 33 moved notice targets and staged the license and provenance sections; `tmp/music-things/MTX-91/edit-intentions.jsonl` records pre-edit hashes and intentions.
- The informational Tides clamp reference now names line 146; Python compilation passed using `-X pycache_prefix=tmp/music-things/MTX-91/pycache` to keep generated output in the approved artifact root.
- Preliminary results: workspace clippy, banned-token grep, notice-target and license-text checks, gate 9, standalone native and wasm builds, standalone tests (74 passed), standalone source diff, baseline-name comparison (unmatched=0), host wasm, crate wasm, protected paths and scope guard passed. Logs are under `tmp/music-things/MTX-91/`.
- Final gates: `nextest-retry.log` exits 0 with 3113 passed and 3 skipped; `names.log` has unmatched=0. `vitest.log` exits 0 with 2399 passed and one protocol skip. `builds.log`, `clippy.log`, `wasm.log`, `crate-wasm.log`, `protected.log`, `scope.log`, `gate9.log`, `standalone-{native,wasm,test,diff}.log`, `grep-final.log`, `notices-final.log`, `license-text.log`, `notice-targets.log`, and `pycompile.log` all exit 0. Standalone cargo test ran 74 tests, matching the crate nextest inventory.
- The first `nextest.log` exits 100 after `tests/cli.rs::lsp_without_the_feature_exits_one` failed while an LSP-feature build modified the shared binary; 648 tests were not run. The clean rerun above supersedes it and is the only counted workspace pass.
- Informational `large-eval-informational.log` exits 0, 1 test passed; recorded load averages 52.66, 48.59, 31.67.
- Earlier-plan blockers: none found. Formal test-integrity and adversarial reviews, integration finalization, staging and commit remain downstream.


## Operator amendment (2026-10-31)

Run `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run test/e2e/large-eval.test.ts`
once and record its outcome (pass, or timeout with the load average at the time) in `notes`
as informational evidence. It is not a `verification` entry; see protocol section 7.
