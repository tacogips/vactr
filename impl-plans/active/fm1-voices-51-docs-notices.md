# FM1V-51: Documentation, Language Reference and msfa Third-Party Notice

**Status**: Ready
**Plan ID**: FM1V-51 (run 3, serial wave 6 of 6)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("License boundary", "Algorithm topologies" -> "Algorithms 4 and 6", "SysEx import" -> Usage, "Presets and examples" -> Documentation, "References")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10 (session 353)

## Session 353 Note (run 3)

- Base is `48d651c`. FM1V-50 is accepted before this plan starts.
- This plan edits documents only. The editor and wasm integration gates
  (`mise run build-wasm-release`, `npm run check`, `npm run test`,
  `VACTR_REQUIRE_SESSION_ABI=1 npm run build`, `npm run test:style`,
  `mise run fmt-vact-check`) and moving accepted plans to
  `impl-plans/completed/` are done by the workflow's serial final
  integration after this plan (dispatch manifest
  `executionModel.finalIntegration`). They are not done here.

## Session 352 Revision (read first)

- **Serial order.** This plan runs alone, after FM1V-50.
- **Start snapshot.** Save `git status --porcelain=v1` to
  `tmp/fm1-voices/FM1V-51/status-before.txt` before any edit.
- **Vact syntax.** Any inline usage in README or lang-reference prose uses
  valid forms: `let bank fm6-sysex ./my.syx`, `patch: {bank 0}`, no `( )`
  grouping and no `let =`.
- **msfa notice.** The "not imported" or "modification" text states one
  divergence: algorithms 4 and 6 render their cross-operator feedback edge,
  which msfa does not (design "Algorithms 4 and 6").

## Intent and Context

This plan records the shipped features where users and auditors look:

- `README.md` status;
- the `design-music.md` template list and sound vocabulary;
- a `lang-reference.md` section 5 row;
- a `THIRD_PARTY_NOTICES.md` section for msfa, which is Apache-2.0 and the
  only third-party code reference used.

It also fixes the `design-music.md` `epiano` sketch to `algorithm: int = 0`,
because a declared non-zero `algorithm` now selects a topology (FV2). And it
resolves the unverified kalimba citation.

## Non-goals

- No Rust or `.vact` source changes.
- Do not edit existing "DX7 data excluded" statements in
  `six_op_original.rs`, `design-mutable-audio.md`, impl-plans or the
  existing notice sections. They stay true.
- Do not change the FM-1 firmware references: they are concept-only and
  have no notice.

## Dependencies

- **dependsOn**: FM1V-40 (the final names, the msfa SHA in
  `src/dsp/ugen/fm/patch.rs` docs, and FM1V-10/11 attribution comments);
  FM1V-50 (serial order of run 2; README links to its examples)
- **Blocks**: none

## writePaths

- `README.md`
- `design-docs/specs/design-music.md`
- `design-docs/specs/lang-reference.md`
- `THIRD_PARTY_NOTICES.md`
- `design-docs/specs/design-fm1-voices.md`. Edit only the "References"
  kalimba citation, and only if verification fails.
- `design-docs/references/README.md`
- `impl-plans/active/fm1-voices-51-docs-notices.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-51` (artifact root)

## sharedPaths

None. The sibling run wf/fm1-tuning may also add a `lang-reference.md`
row. Add this run's content as its own separate row or sentence, so a merge
keeps both.

## Read-only References

- `THIRD_PARTY_NOTICES.md` section format: a `##` heading per source,
  files, pinned revision, what was and was not imported, and the license
  text or a pointer to it. See the MIT text at about line 885.
- The msfa checkout `tmp/fm1-voices/msfa` (FM1V-00) for the license file
  (`LICENSE`/`COPYING`) and any `NOTICE` file.
- Attribution comments in `src/dsp/ugen/fm/algorithms.rs`, `envelope.rs`,
  `scaling.rs`, `sysex.rs` and `src/dsp/tests/dsp/fm_algorithms.rs`.

## File-level Changes

**`THIRD_PARTY_NOTICES.md`**

Append the section `## music-synthesizer-for-android (msfa), Apache-2.0`,
containing:

- the repository URL and pinned SHA;
- the files consulted (from the FM1V-00/10/11/12 Progress Logs);
- what was adapted: the algorithm bus-flag bytes (in the test fixture
  only), the EG rate/level formulas and level curve, keyboard level/rate
  scaling, velocity scaling, the operator frequency formulas, feedback
  scaling, and the bulk-dump unpack layout;
- the modification statement: translated to Rust, floating-point frequency
  and amplitude paths, analytic sine instead of tables;
- what was not imported: no patch data, no sine/exp tables beyond the level
  curve, no LFO or pitch EG, no Dexed code;
- the statement that `fm6-core` loads only user-supplied SysEx at run time
  and bundles no factory patches;
- the full Apache License 2.0 text. Copy it verbatim from the msfa
  checkout's license file. If that is unavailable, use
  `https://www.apache.org/licenses/LICENSE-2.0.txt`. Also copy msfa's
  `NOTICE` content if one exists.

Also add one sentence under that section: "The kalimba, tonewheel organ,
hurdy-gurdy, VOSIM, GENDYN and scanned-synthesis voices are original code
written from published papers and physics; no third-party code was used."

**`README.md`** `## Status`

Add one bullet group, matching the bass bullet's style:

- six-operator FM algorithms for `fm` (`algorithm 0` legacy, 1..32
  topologies);
- `fm6-core` with `patch:` and `fm6-sysex` (native host only);
- the six templates, each with a short description;
- links to the design and examples.

**`design-docs/specs/design-music.md`**

- Change line 355, the `inst epiano algorithm: int = 5`, to `= 0`. Add a
  trailing comment noting that 1..32 select six-operator topologies.
- In section 7, in the sound vocabulary row, append the six template names,
  `fm6-core` and the six `-core` UGens to the synthesis list.
- Section 4.4 already exists. Do not duplicate it.

**`design-docs/specs/lang-reference.md`** section 5

Add a row `| sound voices (fm1) | ... |`. It lists:

- `fm` `algorithm` 0..32;
- `fm6-core` (`patch:` with 155 values);
- `fm6-sysex path` (a list of voices, each a list of 155 ints; native host
  only);
- the templates `kalimba`, `tonewheel-organ`, `hurdy-gurdy`, `vosim`,
  `gendyn` and `scanned`.

It points to `design-fm1-voices.md`. Do not add fenced code blocks; this
file is `include_str!`'d by reader and type tests.

**Kalimba citation**

Verify D. Chapman, "The tones of the kalimba (African thumb piano)",
JASA 131(4), 2012, using one web search or the JASA site.

- If it is confirmed, add a row to the references README "Six-Operator FM
  and New Voices" table.
- If it is not, remove that bullet from the design "References" list and
  record that in the Progress Log. Fletcher and Rossing already cover the
  model.

## Pitfalls

- **Long-running nextest (command timeout).**
  - Full nextest (Verification 6) takes about 800 to 1700 s. The single
    test `complete::tests::robust::every_prefix_and_mutant_is_panic_free`
    takes about 500 s.
  - The previous FM1V-30 run was killed by SIGTERM at about 1200 s
    (`tmp/fm1-voices/FM1V-30/focused-final-blessed.log`, exitStatus 100).
  - Run the lock-wrapped full nextest in the foreground with the executor
    command timeout set to at least 3600 s, or to its maximum. Poll it
    until it exits, and never background it.
  - A SIGTERM or harness kill is neither a pass nor a code failure. Rerun
    the same command once with the long timeout. Keep both logs as
    `tmp/fm1-voices/FM1V-51/attempt-<n>/full.log` and record both
    attempts.
  - Never skip, ignore or filter out tests in the full run.

- **Use trademarks only in prose.** Write "DX7" only where the notices and
  design already do.
- **Keep the `lang-reference.md` row a single table row.** The type tests
  quote section 5 line ranges in comments. If line numbers shift, the
  literal name lists in `src/types/tests/natives.rs` do not change, so no
  test edit is needed.
- **Copy the Apache text verbatim.** Do not paraphrase it.

## Verification (logs under `tmp/fm1-voices/FM1V-51/`)

1. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/reader::tests|no_abort|types::tests::natives/)' > tmp/fm1-voices/FM1V-51/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with tests run and 0 failed.
2. `grep -c "Apache License" THIRD_PARTY_NOTICES.md` is at least 1, and the
   notice contains the 40-hex msfa SHA that matches `patch.rs`.
3. `grep -n "algorithm: int = 0" design-docs/specs/design-music.md` matches
   the `epiano` line.
4. Every path that is new or changed against `status-before.txt` is in
   writePaths.
5. `grep -n -i "algorithms 4 and 6" THIRD_PARTY_NOTICES.md`
   matches at least one line in the msfa section.
6. Full nextest under the measurement lock:
   `bash -c 'L=/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock; until mkdir $L 2>/dev/null; do sleep 30; done; trap "rm -rf $L" EXIT; echo FM1V-51 > $L/owner; CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run > tmp/fm1-voices/FM1V-51/full.log 2>&1; echo "exit=$?"'`
   must print `exit=0` with 0 failed. `lang-reference.md` is `include_str!`'d
   by tests, so this run is part of acceptance.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

This plan runs alone (serial wave 6 of run 2). Fresh-read each document
before editing it. Never commit, stash, checkout, reset or push.

## Done Criteria

- [ ] The notice, README, design-music, lang-reference and references edits
      are made.
- [ ] The kalimba citation is resolved.
- [ ] Verification 1-6 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.
