# Clock transaction fixture correction

**Status**: Completed
**Created**: 2026-10-03
**Last Updated**: 2026-10-03
**Design Reference**: [Current completion path](../../design-docs/specs/design-song-mode.md#design-and-implementation-review-current-completion-path)

## Purpose

Correct the failed clock fixtures using valid public DSL and real snapshot
transaction evidence. Pending collector records are not published snapshot
state. Keep exact cumulative work/depth and failure restoration assertions.

## Related plans

- **Depends On**: [Clock frames](song-mode-canonical-clock-frames.md), current held source0001.
- **Next**: Independent joined clock and occupancy checks; then structural sampling.

## Exact three-path manifest

| Path | Deliverable |
| --- | --- |
| `src/pattern/eval/song_clock.rs` | Correct genuine Unknown fixture; use real snapshot publication witness while retaining actual frame work/depth and restoration assertions |
| `src/pattern/eval/song_clock/tests.rs` | Extract the existing cohesive private test module, preserving fully qualified test names; add corrected clock witnesses here |
| `src/song/snapshot/occupancy.rs` | Private cfg(test) helper exercising real retain_index_occupancy and inspecting actual unchanged/committed snapshot authority |

The test child and occupancy path are explicitly authorized by this companion plan. It does not
expand the original eight production clock deliverables. No production API,
public field visibility, dependencies or Git changes. Each touched Rust file
must stay below1000 lines. The unchanged test-module extraction gives the
currently934-line clock module room for the subsequent sampling implementation.

## Tasks

### TASK-001: Actual publication witness

**Status**: Completed
**Parallelizable**: No

- [x] Implement a crate-private cfg(test) helper inside the snapshot occupancy module.
- [x] Construct or accept a genuine isolated candidate and mint real requests.
- [x] Measure real whole-transaction work before exact sufficient/one-less tests.
- [x] Assert failure leaves the original published view and occupancy unchanged.
- [x] Assert success publishes the complete batch, without forged identities.
- [x] Preserve consumed work, actual VM ledger restoration and inherited depth checks.

### TASK-002: Valid clock fixture and source hold

**Status**: Completed
**Parallelizable**: No; depends on TASK-001.

- [x] Replace invalid Segment pattern input with a genuinely typechecked sampling path.
- [x] Extract the existing private tests into the declared child without changing test identities.
- [x] Preserve Unknown evidence, real sibling frame restoration and callback constraints.
- [x] Separate scratch-state assertions from transaction publication assertions.
- [x] Scoped formatting, all touched files below1000, and fresh complete source hold.

### TASK-003: Independent verification

**Status**: Completed
**Parallelizable**: No; depends on TASK-002.

- [x] All six clock fixtures pass, plus any new transaction fixtures.
- [x] All original nineteen occupancy/replay fixtures pass.
- [x] Affected/public checks, native/WASM, strict lint and scoped format pass.
- [x] Record terminal exits and unchanged held inputs; preserve original101 history.

## Completion boundary

This correction proves the stated fixture contracts. It does not close clock,
sampling, admission, production integration or full song-mode completion unless
their own acceptance gates independently pass.

## Progress log

### 2026-10-03 — Source-grounded access boundary

Author inspection confirms the current clock module cannot inspect private
snapshot publication fields. A cfg(test) witness in the owning occupancy module
provides genuine transaction evidence without widening production visibility.
Root authorizes this bounded three-path correction after all prior checker handles
are terminal. Current six-clock result remains four passes and two failures.

### 2026-10-03 — Authorized implementation intent

Record current two existing baselines in clock-transaction-source-intent-0001.json before occupancy edits. Extract the original tests unchanged into the authorized child; correct Grid input and scratch/publication assertions only. A cfg(test) crate-private helper will mint full actual recipe paths, seed a genuine prior view, measure a subsequent retention transaction, and test exact/one-less work and real DivisionByZero against unchanged prior publication. Direct frame work/depth and VM restoration assertions remain separate. First checker history is native0, clock4PASS2FAIL/101; no Cargo is run by the author.

### 2026-10-03 — Corrected coherent source held

Implementation is written under the exact three-path companion. Clock implementation is331 lines; extracted tests603; occupancy999. Tests retain their original namespace and six names. The helper uses actual checked isolated candidates, original snapshot-issued full recipe paths, previous published view authority, complete transaction cost and real failure counters. Successful exact work commits the second window; one-less and genuine dynamic DivisionByZero leave the prior view Rc and occupancy record unchanged. Direct collector depth/work/restoration stays independent. Grid Unknown remains a genuine fixture awaiting independent validation. Original checkpoint native0/clock4PASS2FAIL/101 remains recorded. Scoped formatting/check exits0. No author Cargo; no passing behavioral claim for this repair before checker.

### 2026-10-03 — Transaction correction verified; legacy stack regression visible

All original six clock tests and occupancy19 pass on clock0002, with native0 and927 unchanged inputs. The transaction helper and Grid correction are now actual behavioral evidence. A later affected legacy pattern scope aborts on the unchanged200/300 reversal test; original production clock plan records the repair and added genuine observed deep-chain fixture. Companion cannot claim final gate completion while that regression remains. No occupancy helper changes made in this repair; occupancy remains999 and needs extraction before future growth.

### 2026-10-03 — Independent acceptance and fresh frontend artifact

Held0004 passes all seven clock fixtures, nineteen occupancy/replay fixtures,
202 affected library tests and127 public tests:355 distinct passes. Of these,
291 execute freshly and64 legacy pattern tests carry only through the proved
sole fixture-child delta from held0003. Native, strict all-target Clippy, pure
WASM and ten-file scoped format/line limits pass. All927 inputs are unchanged
and all checker processes are terminal. Receipt:
/tmp/vactr-canonical-clock-final-004.json, SHA256
8701477692890c7154c3be25bdbc54e5769361270162c46b6f93ff5f62682d41.

Fresh WASM f3e0e665eea52b9c68edb4a4874c119d175bcebc8420c509a523e6c01588b703
passes590 editor tests across78 files and frontend build; dist matches. Root
receipt: tmp/song-mode-riela/ROOT-editor-canonical-clock-20261003.json. All
frontend processes are terminal and root rechecks all927 source inputs. Prior
fixture101, SIGABRT101 and parser101 checkpoints remain historical evidence.
This closes only the stated bounded plan, not full song-mode completion.
