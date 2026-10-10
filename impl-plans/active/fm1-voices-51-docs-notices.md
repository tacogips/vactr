# FM1V-51: Documentation, Language Reference and msfa Third-Party Notice

**Status**: Ready
**Plan ID**: FM1V-51 (wave 4)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("License boundary", "Presets and examples" -> Documentation, "References")
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

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
  `src/dsp/ugen/fm/patch.rs` docs, and FM1V-10/11 attribution comments)
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
4. `git diff --stat` shows only writePaths.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

FM1V-50 runs in parallel on examples only. Fresh-read each document before
editing it.

## Done Criteria

- [ ] The notice, README, design-music, lang-reference and references edits
      are made.
- [ ] The kalimba citation is resolved.
- [ ] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (plan created)
**Tasks Completed**: none.
