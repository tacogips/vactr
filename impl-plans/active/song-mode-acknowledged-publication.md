# Acknowledged song state publication

**Status**: In Progress
**Created**: 2026-10-02
**Design Reference**: [Live controls](../../design-docs/specs/design-song-mode.md#live-controls-and-snapshot-application)

## Purpose and dependencies

Publish acknowledged instrument mute and finite transport state without treating
mute as a new song Apply. Existing CandidateReady/Applied/Failed messages do not
express these outcomes. Runtime/session integration proceeds independently;
this small protocol phase supplies its remaining publication types.

- Depends on: [Transport integration](song-mode-transport-integration.md).
- Consumer: [Browser session](song-mode-browser-session.md).
- Full song-mode scope, including CLI and export, remains unchanged.

## Modules

| File | Deliverable | Status |
|---|---|---|
| `src/session/protocol.rs` | Add acknowledged mute and transport-state messages, kinds and routing | Not started |
| `src/session/codec.rs` | Exact integer frame/epoch wire handling if existing helpers need extension | Not started |
| `tests/song_protocol.rs` | Round-trip, exact large integers, routing and legacy compatibility | Not started |
| `src/sched/runtime/song.rs` | Retain full selector mute requests, aggregate actual family receipts, publish finite state | Not started |
| `src/session/song/transport.rs` | Route acknowledged mute to its requester and transport changes to existing topics | Not started |
| `src/session/session.rs` | Connect mute requests to the active original owner | Not started |
| `tests/song_transport.rs` | Real host mute acknowledgement and finite state witnesses | Not started |

Seven Rust files; keep each below 1000 lines. The Runtime/session author owns
this phase after the pending-candidate cancellation checkpoint, so the four
overlapping integration paths have one writer. No additional paths are released.

## Public contracts

```rust
pub struct SongInstrumentMutedBody {
    pub epoch: crate::song::SnapshotEpoch,
    pub selector: WireInstrumentSelector,
    pub muted: bool,
    pub application_frame: u64,
}
pub enum WireSongTransportState { Prepared, Playing, Draining, Ended, Failed }
pub struct SongTransportStateBody {
    pub epoch: crate::song::SnapshotEpoch,
    pub state: WireSongTransportState,
    pub instruments: Vec<WireInstrumentSelector>,
}
```

Add corresponding ServerMsg variants using existing serialization conventions.
Mute publication requires actual matching host acknowledgements for the complete
selected family; submission alone is insufficient. Preserve epoch and frame
exactness across JavaScript. Correlate the original selector, including multiple
instrument families, rather than inventing it from an integer instrument ID.
Use requester routing for acknowledged requests and existing telemetry routing
for ongoing finite state changes. Keep every existing message kind compatible.
The state message additionally supplies a bounded certified selector inventory
from the original active owner's frozen families for actual editor mute controls.
Serialize an empty inventory by default/omission for compatibility if necessary;
never parse source text or infer authority from runtime physical instrument IDs.

## Tasks and completion

1. Define wire types and exact serialization; all existing messages remain valid.
2. Connect actual acknowledged Runtime outcomes through integration publication.
3. Run focused protocol and integration checks after a source hold.

- [ ] Exact integer round-trip and routing are verified.
- [ ] No mute reply is emitted before all addressed host acknowledgements.
- [ ] Failed/replaced epochs cannot publish stale success.
- [ ] User-visible state follows actual finite transport progress.

## Progress

2026-10-02: Existing protocol.rs (864 lines) was inspected. The missing outcome
types were confirmed during integration. This plan releases the three paths
after the integration author reaches a coherent source checkpoint; no protocol
implementation or verification is claimed yet.

2026-10-03: Initial consuming session playback and streaming export passed their
focused tests. Extend this phase from declarations to complete production mute
and transport publication in seven explicit modules. Finish pending-candidate
cancellation first, then use the existing Runtime/session author. Root owns the
corresponding declared browser protocol modules; no unrelated editor changes.

### First coherent production checkpoint

Defined exact decimal epoch/frame mute acknowledgement and finite-state DTOs.
Runtime retains the full mute request, admits actual certified families, retries
host pressure, and aggregates matching per-family DSP receipts before a success
notice. Session keeps original selector/request correlation. State publication
is queued after actual Applied, with a bounded original frozen selector inventory
for UI controls; Ended notice is retained before owner cleanup. Existing exact
integer codec helpers suffice without another frame API.

Standalone preparation failure now preserves Failed even after actual cleanup;
a currently active transport remains authoritative on replacement preparation
failure. Added direct consuming measured insufficient-capacity fixture. This is
a coherent compile checkpoint, not publication acceptance: mute/codec fixtures,
stale-failure cleanup and full overlay/Apply semantics remain required. No author
Cargo. Scoped format succeeds and all touched Rust remain below1000.

### Actual family acknowledgement and wire fixtures ready

Wrote a real Native Session two-instrument fixture. A forwarding host retains an
actual second DSP Muted receipt while forwarding the first, proving no success
until all addressed resolved families acknowledge. Releasing that original receipt
produces one requester-correlated full selector reply. Actual PCM then reaches
silence, unmute acknowledges and later scheduled notes resume; finite Ended is
published and retired-epoch mute refuses. No fabricated receipt or origin is used.

Added exact fullwidth epoch/frame codec roundtrip, malformed decimal refusal,
all finite state variants and requester/telemetry routing tests. State inventory
contains original frozen sound selectors, not parsed code guesses. Outstanding
mute requests get original-request failure if the addressed active epoch ends,
fails or is replaced; successful stale family receipts cannot aggregate then.
Standalone preparation failure fixture independently passed1/1 in the CLI focused
checkpoint. All publication fixtures await their independent focused run.

Focused initial publication checks: real mute fixture failed its initial Apply
assertion before any audio/ACK assertion, and protocol fixture incorrectly called
unwrap on encode's actual String return. Corrected encode use, removed unused
trait import, and corrected multi-track DSL from comma-separated entries to
whitespace-separated entries (reader lexer treats comma as stray; existing
multi-track fixtures use whitespace). Added exact response Debug to the unchanged
empty-response assertion. All real ACK/audio/state expectations remain intact.
Raw failures: /tmp/vactr-song-family-mute-initial-001.log and
/tmp/vactr-song-mute-protocol-initial-001.log, both terminal. No production edit.

### Focused publication acceptance

Independent exact repaired fixtures both passed: real family mute command
original78226 terminal0/chunkdb3df2, raw
/tmp/vactr-song-family-mute-repaired-001.log; exact protocol command
original96330 terminal0/chunka3b952, raw
/tmp/vactr-song-mute-protocol-repaired-001.log. Real second DSP ACK aggregation,
requester correlation, PCM mute/unmute and finite Ended, fullwidth codec/routing
are proven in this focused scope. No full integration or atomic Apply acceptance
is implied. Cross-snapshot overlay remapping and compound replacement remain
required in the integration plan. Rust sources remain held unchanged.
