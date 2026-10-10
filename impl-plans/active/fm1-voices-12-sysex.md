# FM1V-12: Six-Operator SysEx Parser (Single Voice and 32-Voice Bulk)

**Status**: In Progress
**Plan ID**: FM1V-12 (wave 1)
**Design Reference**: `design-docs/specs/design-fm1-voices.md` ("SysEx import": formats, validation order, unpacking)
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

Users must be able to load their own six-operator patch files:

- a single-voice dump of 163 bytes;
- a 32-voice bulk dump of 4104 bytes.

Both need checksum validation and clear errors. This plan writes a pure
parser. It does no I/O and is never called on the audio thread. FM1V-21
wraps it in the `fm6-sysex` native, and FM1V-40 uses its test encoders.

**No factory or ROM patch data, ever.** Every fixture is built in test code
from original, hand-chosen values.

## Non-goals

- No file reading and no native function (FM1V-21).
- No engine code.
- No patch names in the output beyond the raw name bytes.
- Never add a `.syx` file to the repository.

## Dependencies

- **dependsOn**: FM1V-00 (`patch.rs`: `Fm6Patch`, `op_base`, `op::*`,
  `global::*`, `max_of`)
- **Blocks**: FM1V-21, FM1V-40

## writePaths

- `src/dsp/ugen/fm/sysex.rs`
- `src/dsp/tests/dsp/fm6_sysex.rs`
- `impl-plans/active/fm1-voices-12-sysex.md`
- `target` (artifact root)
- `tmp/fm1-voices/FM1V-12` (artifact root)

## sharedPaths

None.

## Read-only References

- `src/dsp/ugen/fm/patch.rs`.
- msfa `patch.cc` (`UnpackPatch`), at FM1V-00's SHA if available. Use it to
  confirm the bulk bit layout below. If it disagrees, msfa wins; record the
  difference in the Progress Log.

## File-level Changes

### `src/dsp/ugen/fm/sysex.rs`

Pinned contract. FM1V-21 and FM1V-40 depend on it.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysexError {
    Empty, NotSysex, Manufacturer, SubStatus, Format, ByteCount,
    Length { expected: usize, actual: usize },
    DataByte { offset: usize, value: u8 },
    Checksum { expected: u8, actual: u8 },
    MissingEnd,
}
impl std::fmt::Display for SysexError { /* one readable sentence per variant */ }
pub const SINGLE_LEN: usize = 163;
pub const BULK_LEN: usize = 4104;
pub fn parse(bytes: &[u8]) -> Result<Vec<Fm6Patch>, SysexError>
#[cfg(test)] pub(crate) fn encode_single(patch: &Fm6Patch) -> Vec<u8>
#[cfg(test)] pub(crate) fn encode_bulk(patches: &[Fm6Patch; 32]) -> Vec<u8>
```

The `Display` messages use hex for bytes. Examples:

- `checksum mismatch: expected 0x2a, found 0x2b`
- `expected 4104 bytes for a 32-voice bulk dump, found 4105`

**Validation order.** Stop at the first failure:

1. `Empty`: the input has length 0.
2. `NotSysex`: `b[0] != 0xF0`.
3. `Manufacturer`: `b[1] != 0x43`.
4. `SubStatus`: `b[2] & 0xF0 != 0`. The device number nibble is ignored.
5. `Format`: `b[3]` is not `0x00` (single) or `0x09` (bulk).
6. `ByteCount`: `(b[4], b[5])` is not `(0x01, 0x1B)` for single, or not
   `(0x20, 0x00)` for bulk.
7. `Length { expected, actual }`: the total length is not 163 or 4104. Any
   trailing byte is a length error.
8. `DataByte { offset, value }`: the first payload byte in `6..len-2` that
   is greater than `0x7F`. `offset` is the absolute index.
9. `Checksum { expected, actual }`: `actual = b[len-2]` and
   `expected = (128 - (sum(payload) % 128)) % 128`.
10. `MissingEnd`: `b[len-1] != 0xF7`.

**Truncated headers.** If the input ends before the byte a check needs,
return `Length` at that point:

- before the format byte is known, or when the format is single:
  `expected: 163`;
- when the format is bulk: `expected: 4104`.

Each check runs only if its byte exists. For example, `[0xF0]` returns
`Length { expected: 163, actual: 1 }`, and `[0xF0, 0x41]` returns
`Manufacturer`.

**Single voice.** The 155 payload bytes map directly to `params`. Clamp each
value to `max_of(i)`.

**Bulk voice.** 32 packed records of 128 bytes each. Voice `v` starts at
`6 + 128*v`.

Per operator `k` in 0..6, where k=0 is operator 6, the operator block is at
`17*k`:

| Byte | Contents |
|------|----------|
| 0..=3 | R1..R4 |
| 4..=7 | L1..L4 |
| 8 | BP |
| 9 | LD |
| 10 | RD |
| 11 | LC = bits0-1, RC = bits2-3 |
| 12 | RS = bits0-2, DET = bits3-6 |
| 13 | AMS = bits0-1, KVS = bits2-4 |
| 14 | OL |
| 15 | MODE = bit0, COARSE = bits1-5 |
| 16 | FINE |

Global fields:

| Byte | Contents |
|------|----------|
| 102..=105 | PR1..4 |
| 106..=109 | PL1..4 |
| 110 | ALG = bits0-4 |
| 111 | FB = bits0-2, OKS = bit3 |
| 112 | LFS |
| 113 | LFD |
| 114 | LPMD |
| 115 | LAMD |
| 116 | LKS = bit0, LFW = bits1-3, LPMS = bits4-6 |
| 117 | TRNSP |
| 118..=127 | NAME |

Unpack into the 155 order: operator `k` goes to `op_base(6-k)`, and the
globals go to `global::*`. Clamp every field to `max_of`. For example, a
DET of 15 becomes 14, an OL of 120 becomes 99, and an LFW of 7 becomes 5.

**Encoders** (test-only): `encode_single` and `encode_bulk` are the exact
inverse of the above. They compute the checksum and append `F7`.

## Pitfalls

- **Checksum scope.** The checksum covers the payload only: bytes
  `6..len-2`. It does not cover the header.
- **Order matters.** A bad checksum and a missing `F7` in the same input
  report `Checksum`.
- **Clamping is not an error.** It is documented in the module doc comment.
- **`parse` allocates its output `Vec`.** That is fine, because it runs on
  the evaluator side. Never call it from a kernel.
- **No ROM data.** Fixture values must be visibly synthetic, for example
  `params[i] = (i * 7) % (max_of(i) + 1)`, or hand-picked round numbers.

## Tests (`src/dsp/tests/dsp/fm6_sysex.rs`; names contain `fm6`)

Fixtures:

- `synthetic_patch(seed)`: a patch with every value in range, built with
  the formula in Pitfalls. Different seeds give different patches.
- `synthetic_bank()`: 32 of them.

Tests:

- `fm6_sysex_single_round_trips`:
  `parse(&encode_single(&p)) == Ok(vec![p])`.
- `fm6_sysex_bulk_round_trips`: 32 patches, all equal and in order.
- `fm6_sysex_device_nibble_ignored`: `b[2] = 0x05` still parses. Recompute
  nothing; the checksum does not cover the header.
- `fm6_sysex_errors_in_order`. Mutate a valid single encoding, and a bulk
  one where relevant. Each case gives exactly one variant:

  | Input | Expected |
  |-------|----------|
  | empty | `Empty` |
  | `b[0]=0xF1` | `NotSysex` |
  | `b[1]=0x41` | `Manufacturer` |
  | `b[2]=0x10` | `SubStatus` |
  | `b[3]=0x02` | `Format` |
  | `b[5]=0x1C` | `ByteCount` |
  | one extra trailing byte | `Length { expected: 163, actual: 164 }` |
  | bulk with one byte removed | `Length { expected: 4104, actual: 4103 }` |
  | `b[10]=0x80` | `DataByte { offset: 10, value: 0x80 }` |
  | checksum + 1 | `Checksum` |
  | last byte `0x00` | `MissingEnd` |
  | `[0xF0]` | `Length { expected: 163, actual: 1 }` |

- `fm6_sysex_error_messages_are_readable`: every variant's `to_string()`
  is non-empty, and the checksum message contains "checksum".
- `fm6_sysex_bulk_clamps_out_of_range_fields`. Hand-build a packed record
  with DET 15, OL 120 and LFW 7, recompute the checksum, and check the
  unpacked values are 14, 99 and 5.
- `fm6_sysex_single_clamps_out_of_range`: a single-voice byte of 120 for an
  OL clamps to 99.
- `fm6_sysex_bulk_layout_spot_check`. Set distinct values in voice 3 for
  op1 OL, op6 R1, ALG, FB, OKS, LFW, LPMS and TRNSP. Check that each lands
  at its `patch.rs` offset.

## Verification (logs under `tmp/fm1-voices/FM1V-12/`)

1. `rustfmt --edition 2021 --check src/dsp/ugen/fm/sysex.rs src/dsp/tests/dsp/fm6_sysex.rs`
   must exit 0.
2. `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6_sysex/)' > tmp/fm1-voices/FM1V-12/nextest.log 2>&1; echo "exit=$?"`
   must print `exit=0`, with at least 8 tests run and 0 failed.
3. `git diff --stat` shows only writePaths.
4. `git ls-files '*.syx'` is empty.

Record results as `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.

## Concurrency and Drift Protocol

Same as FM1V-10.

## Done Criteria

- [x] `parse` and the error type are implemented with the pinned contract.
- [x] Verification 1-4 pass and are recorded.

## Progress Log

### Session: 2026-10-10 (FM1V-12 implementation)
**Tasks Completed**: Implemented `SysexError`, ordered single/bulk framing validation, truncated-header lengths, checksum and data-byte validation, single and bulk unpacking with range clamping, and test-only inverse encoders. Added eight synthetic tests covering round trips, validation errors, clamping and bulk layout offsets. Read the permitted MSFA `patch.cc` at the pinned FM1V-00 SHA; its bit layout agrees with the plan table.

**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/fm/sysex.rs src/dsp/tests/dsp/fm6_sysex.rs` passed (final log `tmp/fm1-voices/FM1V-12/rustfmt-final.log`). Focused `cargo nextest run -E 'test(/fm6_sysex/)'` passed on the latest source (8 run, 8 passed, 0 failed; log `tmp/fm1-voices/FM1V-12/nextest-postcheck-20261010-latest.log`). `git ls-files '*.syx'` returned no tracked files (final log `tmp/fm1-voices/FM1V-12/tracked-sysex-final.log`). Scoped diff/status evidence is in `tmp/fm1-voices/FM1V-12/diff-scope.log`; global status also contains concurrent changes owned by other fanout plans. Two earlier nextest attempts failed before compiling this plan because the shared tree was moving; complete logs remain at `tmp/fm1-voices/FM1V-12/nextest.log` and `tmp/fm1-voices/FM1V-12/nextest-rerun.log`. Two postcheck attempts likewise captured transient compiler failures before a final source-matched passing run; their logs are retained at `tmp/fm1-voices/FM1V-12/nextest-postcheck-20261010.log` and `tmp/fm1-voices/FM1V-12/nextest-postcheck-20261010-rerun.log`.

### Session: 2026-10-10 (bulk algorithm field correction and stable verification)
**Tasks Completed**: Corrected bulk ALG unpacking to mask bits 0..4 (rather than saturating the full byte), and strengthened the layout spot-check with `0x73` to prove high bits are ignored. The edit intent is recorded at `tmp/fm1-voices/FM1V-12/edit-intent-algorithm-mask.md`.

**Verification**: Final rustfmt check exited 0 (`rustfmt-final.log`). The exact focused nextest command exited 0 with 8 tests run, 8 passed, 0 failed (`nextest-postcheck-20261010-latest.log`). `git ls-files '*.syx'` exited 0 with no tracked `.syx` files (`tracked-sysex-final.log`). Scoped status identifies only this plan's three `writePaths`; other global changes belong to concurrent fanout plans. Independent formal review and integration gates remain downstream workflow steps.

### Session: 2026-10-10 (Step 6 source-matched verification)
**Tasks Completed**: Rechecked the FM1V-12 parser, synthetic fixtures, scoped working-tree state and accepted `FM1V-00` dependency. No parser or test code changes were needed. The assigned implementation criteria remain satisfied; formal review and combined-tree integration are downstream.

**Verification**: `rustfmt --edition 2021 --check src/dsp/ugen/fm/sysex.rs src/dsp/tests/dsp/fm6_sysex.rs` exited 0 (`tmp/fm1-voices/FM1V-12/rustfmt-step6.log`). `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run -E 'test(/fm6_sysex/)'` exited 0 (8 run, 8 passed, 0 failed; `tmp/fm1-voices/FM1V-12/nextest-step6.log`). `git ls-files '*.syx'` exited 0 and returned no tracked SysEx files (`tmp/fm1-voices/FM1V-12/tracked-sysex-step6.log`).
