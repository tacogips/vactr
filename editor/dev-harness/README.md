# Vactr dev harness (TASK-008 real-worklet checks)

The standalone browser harness for the two TASK-008 criteria that only a real AudioWorklet can prove (design
`design-docs/specs/design-implementation.md` 12.8.11, 16.1): criterion 8 (control-cell transport and
`VoiceRelease`) and criterion 11 (the 16.1 resource lifecycle). The headless `Engine::process` tests never
substitute for these checks.

The page loads the `host-wasm` module twice: on the main thread (`editor/worklet/host.js`, wasm #1: evaluator
and scheduler) and in the `AudioWorkletProcessor` (`editor/worklet/processor.js`, wasm #2: DSP only). It drives
the main half with Vactr source, injects faults on the record path between the halves (the host's `filter`
hook can hold, drop, replace or reorder any record), and reads the worklet's report counters.

## Build

```sh
CARGO_TERM_QUIET=true cargo build --target wasm32-unknown-unknown --no-default-features --features host-wasm
```

The module is `target/wasm32-unknown-unknown/debug/vactr.wasm`. The `vactr` bin target links to the same
uplifted file name, so when the bin overwrote it the runner serves the cdylib from `.../debug/deps/vactr.wasm`
instead (or build the library alone with `cargo build --lib ...`).

## Run headless (primary)

```sh
node editor/dev-harness/run-headless.mjs
```

Node built-ins only. It serves the repository root on `127.0.0.1:<free port>`, launches Chrome (`$CHROME`, or
`/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`) with `--headless=new
--autoplay-policy=no-user-gesture-required`, receives the page's JSON report by POST, and writes it to
`target/fe-logs/be-wasm-harness-s182-<n>.json` (or `--out <path>`). Exit codes:

- `0`: every check passed and the worklet memory size did not change after init.
- `1`: a check failed, the report is malformed, no report arrived within 120 s (`--timeout <s>`), or the wasm
  module is missing.
- `2`: BLOCKED: no Chrome, or the `AudioContext` never reached `running`. A blocked run is never a pass.

## Run headed (operator path, B1)

When the sandbox cannot run Chrome or open an audio device, the operator runs the same page in a visible window:

```sh
node editor/dev-harness/run-headless.mjs --headed
```

The report it writes is the evidence for criteria 8 and 11. The page can also be opened by hand from any static
server at `/editor/dev-harness/index.html` (press Start); only `?auto=1` posts the report.

## Checks

| id | criterion | what is asserted |
|----|-----------|------------------|
| `cell-first-before-ack` | 8 | with every `CellInit` held, events commit as `Const(0.5)`, voices sound, and the mirror cell is never read; after the ack, reads see 0.5 and none is uninitialized |
| `cell-batch-next-voice` | 8 | voice starts before the batch read 0.5, every start after it 0.8 |
| `cell-delayed-hop` | 8 | while a batch is held, starts inside the hop read the earlier value; after it lands, the new one |
| `cell-init-replay` | 8 | a replayed `CellInit` after newer batches leaves the newer value (init-once) |
| `cell-reuse` | 8 | a spare cell goes Live(1) -> Retiring -> Vacant (`CellRetired` ack) -> Live(2) |
| `cell-stale-epoch` | 8 | a batch entry with the retired epoch is inert while its live entries apply |
| `cell-stall-burst` | 8 | with nothing delivered, at most one batch in flight and one pending (one distinct seq sent); converges to the last value on resume |
| `cell-reconnect` | 8 | after lost cell records and `main_resync`, every voice start after the commit lead reads the current value |
| `release-after-genbump` | 8 | `VoiceRelease` releases exactly its tagged voice after a slot generation bump |
| `release-tombstone` | 8 | a release before its note-on hits the tombstone: the note never starts and is counted dropped |
| `load-during-playback` | 11 | a sample installs through the slice window while a tone sounds, with no silent quantum and no frame gap |
| `graph-replace-during-playback` | 11 | redefining the sounding instrument installs the new graph with no silent quantum; the voice keeps playing |
| `arena-exhausted` | 11 | an oversized sample is refused before sending (`arena-exhausted`); an oversized `SampleBegin` faults on the worklet |
| `graph-too-large` | 11 | an instrument over 256 nodes is `graph-too-large` |
| `deferred-queue-overflow` | 11 | install records beyond the deferred bound fail with `install-queue-overflow` |
| `unload-while-playing` | 11 | an unloaded sample stays `Retiring` while a voice uses it; `Retired` comes only after the voice ended |
| `install-burst-credit` | 11 | a burst of four installs copies at most 64 KB per `process()`; a slice arriving with the credit spent is copied and acked in a later quantum |
| `memory-stable` | 11 | the worklet memory size at the end of the run equals the size at the end of init |

## Report

`{checks: [{id, pass, detail}], memoryStable, userAgent, notes, report, js, console, log, seconds}`. `report` is
the worklet's `f64` report array (indices `R_*` in `src/host/wasm/worklet_half.rs`); `js` holds the processor's
counters `[slot drops, oversized records, process errors, frame gaps, records waiting]`.
