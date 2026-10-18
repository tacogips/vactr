# LPI-WIRE: make the song-record tags and live-perf control tags unambiguous

**Status**: In Progress (implemented at b327141; review-and-repair rerun pending)
**Plan ID**: LPI-WIRE (wave 1)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 2 (I1, I9), 3 (W1-W3) and 11 (R1-R6)
**Manifest**: `impl-plans/active/live-perf-int-dispatch.json`
**Branch**: `merge/live-perf` (shared working directory; no private branch)
**Created**: 2026-10-18
**Last Updated**: 2026-10-18

## Intent and Context

On the merged tree, `tests/song_resource_carriers.rs`
`graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind`
fails at line 807. The cause is a tag collision:

- LP-CONTRACT gave `CtlMsg::OutputStop` tag `0x1C` and `CtlMsg::CellRamp`
  tag `0x1D` (`src/host/wire.rs:351-352`).
- Main's ring song records already use `GRAPH = 0x1c`, `BEGIN = 0x1d` and
  `SLICE = 0x1e` (`src/dsp/ring/song.rs:7-9`). Tests pin those values
  (`tests/song_hosts.rs:227`, `tests/song_resource_carriers.rs:420,835`).
- LP-ENGINE worked around the overlap in `ByteInbox::next`
  (`src/dsp/ring.rs:526-530`). It tries an exact-length `CtlMsg::decode`
  before `song::decode`. A corrupted song record whose second byte is 0 or
  1 therefore decodes as `OutputStop` instead of being rejected.

The fix keeps main's song tags. It moves the two newer live-perf records to
free tags and restores main's song-only decode.

## Non-goals

- No change to song record layout, song tags, song tests or the
  `song_resource_carriers` assertions.
- No change to the bodies or field order of `OutputStop` and `CellRamp`.
- No change to `CtlMsg::MAX_LEN`, the HostMsg tags (`0x4B`, `0x4C`), the
  native typed `NativeRecord::Msg` path, `ByteInbox::class` or
  `ByteInbox::drop_held`.
- No editor, session or engine behavior change.
- Do not edit `tests/song_resource_carriers.rs`. It has 991 lines and is
  near the 1000-line limit.

## Dependencies

- **dependsOn**: none (wave 1).
- Runs alongside LPI-VIM-STOP (TypeScript only) and LPI-NS-INDEX (only
  `src/ns`). All three share `target/`. If a compile error appears in a file
  outside this plan's writePaths, it is the other plan's work in progress:
  - wait 2 minutes and re-run, at most 3 times;
  - then record it as drift in the Progress Log and stop;
  - never edit that file.

## writePaths

- `src/host/wire.rs`
- `src/dsp/ring.rs`
- `src/host/tests/contracts.rs`
- `impl-plans/active/live-perf-int-wire.md` (this plan; Progress Log only)
- Artifact roots: `target`, `tree-sitter-vact/tree-sitter-vact.wasm`,
  `editor/node_modules`, `tmp/live-perf-int/wire`

## sharedPaths

None. Read-only references: `src/dsp/ring/song.rs`,
`tests/song_resource_carriers.rs`, `tests/song_hosts.rs`,
`src/dsp/tests/dsp.rs`, `src/dsp/tests/dsp/output_stop.rs`.

## Tasks

### TASK-W1: Free-tag pre-check and renumbering (`src/host/wire.rs`)

1. Pre-check that the new tags are unused, case-insensitively:
   `! grep -rniE '0x1f\b|0x20\b' src/host src/dsp/ring.rs src/dsp/ring src/song`
   - It must print nothing and exit 0 (the `!` inverts grep's no-match
     exit code). Save the output and exit code to
     `tmp/live-perf-int/wire/0-tag-precheck.log`.
   - If it finds a wire tag constant, stop and report. Do not pick another
     value on your own.
2. Change `const TAG_OUTPUT_STOP: u8 = 0x1C;` to `0x1F`, and
   `const TAG_CELL_RAMP: u8 = 0x1D;` to `0x20`. Nothing else in the file
   changes.
3. If any comment in `wire.rs` names `0x1C`/`0x1D` for these records,
   update it to the new values.

**Pitfalls:**

- Do not renumber the song constants in `src/dsp/ring/song.rs`; they are
  main's contract.
- Do not touch `TAG_OUTPUT_STATE` (`0x4B`) or `TAG_CELL_RAMP_ACK`
  (`0x4C`).

### TASK-W2: Restore song-only decode (`src/dsp/ring.rs`, `ByteInbox::next`)

Replace the `song::GRAPH | song::BEGIN | song::SLICE` arm. Today it calls
`CtlMsg::decode(b)`, filters for exact length and falls back with
`.or_else(|| song::decode(b))`. The new arm is exactly main's form:

```
song::GRAPH | song::BEGIN | song::SLICE => song::decode(b),
```

Check against `git show main:src/dsp/ring.rs` (line 526).

- `OutputStop` and `CellRamp` now reach the existing default arm,
  `_ => CtlMsg::decode(b).ok().map(|(m, _)| Record::Msg(m))`. Leave that
  arm unchanged.
- Leave the `SONG_COMMAND_TAG` arm and every other arm unchanged.

**Pitfalls:**

- Do not add a length filter or a fallback to the default arm.
- Do not change the "undecodable record is dropped" counting.

### TASK-W3: Regression guard and mechanical update (`src/host/tests/contracts.rs`)

1. **Mechanical update.** In `bad_tags_and_fields_are_wire_errors`, change
   `CtlMsg::decode(&[0x1c, 9])` to `CtlMsg::decode(&[0x1f, 9])`, with the
   same expected `Err(WireError::BadValue)`. The meaning is unchanged: an
   unknown OutputStop mode is a `BadValue`. Record this in the Progress Log
   as a mechanical update caused by W1.
2. **New test** `live_perf_control_tags_are_disjoint_from_song_record_tags`.
   - Build a song lease the same way as `tests/song_resource_carriers.rs`
     fn `lease`:
     `SongLeaseKey { epoch: SnapshotEpoch(u64::MAX), resource: SongResourceRef { id: u32::MAX, generation: u32::MAX }, kind }`.
   - Encode one song graph record
     (`crate::dsp::ring::encode_song_graph_record`, kind `Track`, bytes
     `&[0, 1, 2]`), one sample begin (`encode_song_sample_begin`, kind
     `Sample`, `1, 1, 48000`) and one slice (`encode_song_slice`, kind
     `Sample`, offset 0, `&[0.0]`).
   - Collect their first bytes, `{0x1c, 0x1d, 0x1e}`.
   - For every message in the existing `every_ctl_msg()`, encode it into a
     `[0u8; CtlMsg::MAX_LEN]` buffer. Assert the first byte is not in the
     song set.
   - Push the encoded `OutputStop {Gentle}`, `OutputStop {Cut}` and the
     `CellRamp` from `every_ctl_msg()` into a fresh
     `crate::dsp::ring::ByteInbox`. Call
     `next(Budget { install_bytes: usize::MAX, batch: true })` for each.
     Each must yield `Record::Msg(m)` with `m` equal to the original
     message.
   - Imitate `src/host/tests/contracts.rs` fn
     `bad_tags_and_fields_are_wire_errors` for style, and
     `tests/song_resource_carriers.rs:766-773` for the inbox calls.

**Test cases (situation -> expected outcome):**

- Encoded `OutputStop` and `CellRamp` -> first byte is `0x1F` / `0x20`,
  outside `{0x1c, 0x1d, 0x1e}`.
- Encoded OutputStop or CellRamp pushed through `ByteInbox` ->
  `Record::Msg` equal to the input.
- `CtlMsg::decode(&[0x1f, 9])` -> `Err(WireError::BadValue)`.
- Song graph record truncated, given trailing bytes, or with a wrong kind
  (the existing carriers test) -> `inbox.next(budget)` is `None` (no new
  code; it must pass again).

## Invariants

- The song record tags and encodings are byte-identical to main.
- All existing `src/dsp/tests/dsp/output_stop.rs` and
  `src/dsp/tests/dsp/ramp.rs` browser-rig tests pass unchanged. They post
  encoded records through `ByteInbox`, so they prove the new tags work end
  to end.
- `src/dsp/ring.rs` gets shorter. `src/host/wire.rs` stays at 928 lines or
  fewer. `src/host/tests/contracts.rs` stays under 1000 lines.

## Verification (run from the repository root; every command exits 0; logs under `tmp/live-perf-int/wire/`)

1. `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0 (`1-fmt.log`).
2. `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` ->
   exit 0 (`2-clippy.log`).
3. Focused nextest, as two records. Filtered runs need no lock. Prefix
   both with
   `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true`.
   - 3a, `3a-nextest-song.log`:
     `cargo nextest run --test song_resource_carriers --test song_hosts --test song_audio_wire`
     - Exit 0. testsRun > 0 and failures 0.
     - `graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind`
       is listed as passed (use `--status-level pass` or
       `cargo nextest list` to show it if needed).
   - 3b, `3b-nextest-lib.log`:
     `cargo nextest run --lib host::tests::contracts dsp::tests::dsp::output_stop dsp::tests::dsp::ramp`
     - Exit 0. testsRun > 0 and failures 0.
     - `live_perf_control_tags_are_disjoint_from_song_record_tags` is
       included.
     - If the module paths differ, use `cargo nextest list --lib` to get
       the real prefixes and record the substitution.
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   -> exit 0 (`4-wasm-debug.log`). This refreshes the debug wasm the
   frontend tests use.
5. `mise run build-wasm-release` -> exit 0 (`5-wasm-release.log`). This
   refreshes the release wasm the editor build embeds.
6. `wc -l src/dsp/ring.rs src/host/wire.rs src/host/tests/contracts.rs` ->
   every count below 1000 (`6-line-counts.log`).
7. **Mutation evidence** (report only as `mutationEvidence`, never as a
   gating record). Temporarily restore the old `0x1C` value for
   `TAG_OUTPUT_STOP`. Show that the new disjointness test fails. Then
   restore `0x1F` and re-run step 3 green. Do not leave the mutation in
   place.

The full `cargo nextest run` runs in LPI-CLOSEOUT under the measurement
lock.

## Completion Criteria

- [x] The tag pre-check log shows no prior use of `0x1f`/`0x20`.
- [x] `TAG_OUTPUT_STOP == 0x1F` and `TAG_CELL_RAMP == 0x20`.
- [x] `ByteInbox::next` song arm equals main's `song::decode(b)`.
- [x] contracts.rs: the mechanical `0x1f` update and the new
  disjointness/round-trip test are in place.
- [x] Verification steps 1-6 exit 0 with positive test counts. Mutation
  evidence is recorded separately.
- [x] The Progress Log records pre/post sha256 for every edited file.

## Rerun (workflow session 438): verify and review the committed work

TASK-W1 to TASK-W3 are implemented in commit b327141 (see the session
nested-v1-c50154ad Progress Log entry). Design section 11 requires that
this work is accepted only after an independent review in this run. This
rerun does **not** re-implement anything. It checks, verifies, records
mutation evidence, and then repairs only what the review finds. All logs go
to `tmp/live-perf-int/wire/rerun-438/` (inside this plan's artifact root).

### TASK-WR1: Deliverable check (no edit)

Run each command and save the output to `rerun-438/r1-deliverables.log`.
The expected result is in brackets.

- `grep -n 'const TAG_OUTPUT_STOP: u8 = 0x1F;' src/host/wire.rs` [1 match]
- `grep -n 'const TAG_CELL_RAMP: u8 = 0x20;' src/host/wire.rs` [1 match]
- `grep -n 'song::GRAPH | song::BEGIN | song::SLICE => song::decode(b),' src/dsp/ring.rs`
  [1 match; same text as `git show main:src/dsp/ring.rs`]
- `grep -n 'fn live_perf_control_tags_are_disjoint_from_song_record_tags' src/host/tests/contracts.rs`
  [1 match]
- `grep -n '0x1f, 9' src/host/tests/contracts.rs` [1 match]
- `git diff 9474491 HEAD -- tests/song_resource_carriers.rs tests/song_hosts.rs src/dsp/ring/song.rs`
  [empty: no song test or song tag change]
- `grep -rniE '0x1c|0x1d' editor/src` [no TypeScript encoder of the
  OutputStop or CellRamp tags. The browser path encodes through the Rust
  wasm, so host and wasm cannot drift.]

If any result differs, record it and stop. Do not edit to make it match;
the review step decides.

### TASK-WR2: Verification rerun

Run verification steps 1-6 of this plan again, unchanged, on the current
tree. Use logs `rerun-438/1-fmt.log` to `rerun-438/6-line-counts.log`.
Report each as a verification record. Step 3a must list
`graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind`
as passed. Step 3b must include
`live_perf_control_tags_are_disjoint_from_song_record_tags`.

### TASK-WR3: Mutation evidence (only in `mutationEvidence`)

Take `shasum -a 256` of the file before each mutation. Restore it
byte-for-byte afterwards and record the matching hash.

- M1: temporarily set `TAG_OUTPUT_STOP` back to `0x1C` -> the
  disjointness test fails (`rerun-438/m1.log`).
- M2: temporarily restore the old song arm (exact-length `CtlMsg::decode`
  first, `.or_else(|| song::decode(b))`, as in `git show 9474491:src/dsp/ring.rs`)
  -> `graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind`
  fails (`rerun-438/m2.log`).
- After both, rerun step 3 green (`rerun-438/m-restore-3a.log`,
  `rerun-438/m-restore-3b.log`).

When WR1-WR3 are done, set Status to
`Implemented (b327141); awaiting independent review`.

### TASK-WR4: Independent review (performed by the workflow review step, not by the implementer)

A read-only reviewer that did not author b327141 checks:

- W1: only the two live-perf tags changed. `0x4B` and `0x4C`, the record
  bodies and `CtlMsg::MAX_LEN` are unchanged.
- W2: the song arm is byte-identical to main, and no length filter or
  fallback was added to the default arm.
- W3: the guard covers every `every_ctl_msg()` message and the
  OutputStop/CellRamp inbox round-trip. The `0x1f, 9` change keeps the
  `BadValue` meaning.
- No song test changed. `wc -l`: ring.rs is no longer than
  `git show 9474491:src/dsp/ring.rs | wc -l`; wire.rs is at most 928
  lines; contracts.rs is under 1000.
- The WR2 records are complete (exit 0, positive counts, log paths), and
  WR3 shows both mutations failing and restored.

### TASK-WR5: Repair only findings

Fix each high or mid finding inside writePaths. A behavior defect gets a
failing-first test. List each fix in `addressedFeedback` with its
`findingId` and status `repaired`, then rerun WR2. If a finding needs a
file outside writePaths, stop and report a scope amendment. The exception
is a mechanical test or fixture update caused by this plan's change.

### TASK-WR6: Acceptance

After the review step accepts, add a Progress Log entry with the reviewer
step id, the verdict and any repair. Then set Status to `Completed`.

### Rerun Completion Criteria

- [x] WR1 log shows every expected result (`rerun-438/r1-deliverables.log`).
- [x] WR2 steps 1-6 exit 0 with positive test counts (`rerun-438/final-source/`).
- [x] WR3 M1 and the legacy-collision regression guard fail as expected; restore hashes match.
- [ ] Independent review verdict recorded (WR4/WR6); every high or mid
  finding repaired (WR5).
- [ ] Status is Completed only after the recorded acceptance.

## Execution Rules (all plans in this manifest)

- Read each file fresh immediately before editing it. Record
  `shasum -a 256 <file>` before and after the edit in the Progress Log.
- Edit only writePaths. Report drift in any other file; never repair it.
- Run commands in the foreground only. Write each to its log with
  `cmd > log 2>&1; echo "exit=$?" >> log`.
- Do not commit, stash, branch or run any mutating git command. Read-only
  `git diff`, `git status` and `git show` are allowed.
- Use English only, with no emojis. Keep every touched source file under
  1000 lines.
- Verification record shape is exactly `{command, exitStatus: 0, testsRun,
  testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
  - Negative-control and mutation runs go only in `mutationEvidence`.
  - Repaired review findings go only in `addressedFeedback`, with status
    `repaired`.
- Edit only this plan's own Progress Log.

## Progress Log

### Session: 2026-10-18 (plan authored, workflow session 433 step 4)
**Tasks Completed**: none. Plan authored from the accepted design.
**Notes**: Ready for wave 1.

### Session: 2026-10-18 (Step 6 implementation, workflow session nested-v1-c50154ad)
**Tasks Completed**: TASK-W1, TASK-W2 and TASK-W3; verification steps 1-6 and mutation evidence.
**Notes**:
- W1: case-insensitive pre-check printed no matches (`tmp/live-perf-int/wire/0-tag-precheck.log`, exit 0). `TAG_OUTPUT_STOP` is `0x1F`; `TAG_CELL_RAMP` is `0x20`.
- W2: `ByteInbox::next` now dispatches song GRAPH/BEGIN/SLICE only through `song::decode(b)`.
- W3: updated the malformed OutputStop fixture to `0x1f`; added `live_perf_control_tags_are_disjoint_from_song_record_tags`, checking song/control tag disjointness and OutputStop/CellRamp inbox round-trips.
- Verification: final-source fmt (`1-fmt-final.log`) exit 0; clippy (`2-clippy-rerun.log`) exit 0; song nextest (`3a-nextest-song-final.log`) exit 0, 47/47 passed, including `graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind`; library nextest (`3b-nextest-lib-final.log`) exit 0, 38/38 passed, including the new guard and output_stop/ramp suites; debug wasm (`4-wasm-debug.log`) exit 0; release wasm (`5-wasm-release.log`) exit 0; line counts (`6-line-counts.log`) are 928, 928 and 604.
- Mutation evidence: temporarily restored OutputStop to `0x1C`; the new test failed as expected (1 run, 0 passed, 1 failed) in `mutation-1-test.log`; restored `0x1F` and reran both nextest groups successfully. Mutation edit intent and restoration are in `mutation-1-output-stop.intent` and `mutation-1-restore.intent`.
- A first clippy attempt (`2-clippy.log`) found the new test needed the `ControlSource` trait in scope. Added that local import and reran clippy successfully; no product behavior changed.
- SHA-256 pre/post: `src/host/wire.rs` 7da604f7d10c0ee073580fc50ec7ab26f9f3998cafd09ac84572169ff02f6968 -> 612e31b53190d1c81a02c1f62fe4c346cd39a092103fbe30a8d8b229e592c3b8; `src/dsp/ring.rs` affe8d90f8b1c0e968ea1f0d7013ed72ffad918b12da1549c055298be404ad19 -> c0dca46babd1af2329a400ccc03b327215885ee1da229d1e94968d3e1dce201e; `src/host/tests/contracts.rs` 8b18c082192f6b9c3d388e90d7ce17dece6b0b8d06dc28fec6174bc5ca43f3b9 -> 8a8e024d8cb5f580b3452435ad8da521c6fd5997c02c183c578c85a97ff5d8df; this plan 23cd300e799226efb0ed9981841961306f8e8f22632d1e3bdd3c9b0011050f3e -> final hash recorded in `tmp/live-perf-int/wire/plan-post-sha256.log`.

### Session: 2026-10-18 (plan revised, workflow session 438 step 4)
**Tasks Completed**: none (plan revision only).
**Notes**: Converted to a review-and-repair rerun (design section 11). Added TASK-WR1 to TASK-WR6 and the Rerun Completion Criteria. Status is In Progress until the independent review verdict is recorded. The original completion criteria remain as the implementation record.

### Session: 2026-10-18 (Step 6 implementation, workflow session 438)
**Tasks Completed**: TASK-WR1 to TASK-WR3; final-source verification steps 1-6.
**Notes**:
- WR1 matches the expected tags, song-only decode arm, guard, mechanical `0x1f` fixture, unchanged song tests/tags, and no editor-side `0x1c`/`0x1d` encoder. Evidence: `tmp/live-perf-int/wire/rerun-438/r1-deliverables.log`.
- Added a two-byte truncated graph case with epoch zero to the existing contracts guard. The planned carrier-test mutation was not discriminating: that fixture uses epoch `u64::MAX`, so the byte read as the old OutputStop mode is `0xFF`. The added `[GRAPH, 0]` case directly exercises the malformed-record contract inside this plan's `writePaths` and fails under the legacy colliding tag plus CtlMsg-first arm.
- Final-source verification: fmt and clippy passed; nextest song suites 47/47; nextest contracts/output-stop/ramp 38/38; wasm debug and release builds passed; line counts were ring.rs 928, wire.rs 928, contracts.rs 628. Logs: `tmp/live-perf-int/wire/rerun-438/final-source/1-fmt.log` through `6-line-counts.log`.
- Mutation evidence: M1 fails the disjointness guard (1 run, 0 passed, 1 failed). The legacy-collision guard mutation fails (1 run, 0 passed, 1 failed). The arm-only and original carrier-test controls passed because their current fixture/tag values do not trigger a collision; preserved in `m2.log`, `m2-legacy-pair.log`, and `m2-forced-rebuild.log`. Both modified production files were restored byte-for-byte; before/after hashes match in `m1-before.sha256`/`m1-after.sha256` and `m2-guard-before.sha256`/`m2-guard-after.sha256`.
- `src/host/tests/contracts.rs` SHA-256 changed from `8a8e024d8cb5f580b3452435ad8da521c6fd5997c02c183c578c85a97ff5d8df` to `2c0390414f0e040fa08a9ab026b871bd0b16addb8b1562285d9d2eb08cd2b9c8`. Independent review and acceptance remain pending; no verdict is claimed.
- Plan SHA-256 before this audit note: `22abdacf42c7c4b3f23f8a08fea842d3aa940a82811058415e118bd60c7e37f7`; final post-edit hash is recorded in `tmp/live-perf-int/wire/rerun-438/plan-final.sha256`.
