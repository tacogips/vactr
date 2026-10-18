# LPI-WIRE: make the song-record tags and live-perf control tags unambiguous

**Status**: Ready
**Plan ID**: LPI-WIRE (wave 1)
**Design Reference**: `design-docs/specs/design-live-perf-integration.md` sections 2 (I1) and 3 (W1-W3)
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

- [ ] The tag pre-check log shows no prior use of `0x1f`/`0x20`.
- [ ] `TAG_OUTPUT_STOP == 0x1F` and `TAG_CELL_RAMP == 0x20`.
- [ ] `ByteInbox::next` song arm equals main's `song::decode(b)`.
- [ ] contracts.rs: the mechanical `0x1f` update and the new
  disjointness/round-trip test are in place.
- [ ] Verification steps 1-6 exit 0 with positive test counts. Mutation
  evidence is recorded separately.
- [ ] The Progress Log records pre/post sha256 for every edited file.

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
