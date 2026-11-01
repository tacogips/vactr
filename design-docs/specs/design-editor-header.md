# Editor header and menus

Status: Implemented, 2026-10-18 (direct user request).

## Behavior and hierarchy

The header presents document navigation first, playback second, musical position third, and output/settings last. Preserve the flat, square token system in design-ui-style.md. Group related controls with visible spacing and concise labels. Keep the primary transport on one row where space permits; wrap whole groups on small screens without clipping menus.

- Documents opens the existing document/example library. Show the current document title and an unsaved indicator alongside it. File actions stay in the document dialog.
- One primary button toggles Play and Stop. Play evaluates the document from stopped state; Stop releases every slot and lets effect tails finish. While paused, the primary button remains Stop, and the adjacent Pause button toggles Resume, preserving existing bindings and musical position.
- Update is a separate secondary action for reevaluating edits while playback continues. It is disabled while stopped and cannot start playback. Existing evaluation shortcuts retain their meaning.
- Show Stopped, Playing, Paused, or effects-tail status in words. Audio-context readiness is a separate output status, never evidence of playback.
- Display cycle/beat and BPM with explicit units. Do not advance the visible position from a free-running engine clock when no playback content is active. Pause freezes musical position and scheduled audio; visual shaders keep animating, as explained by the tooltip. Stop clears the position. Beat flashes only while playing.
- Keep an immediate Cut control available with a clear tooltip. Move Vim and shortcut help into an Editor disclosure, retaining keyboard access and Escape/outside-click dismissal.
- Keep whole-song application and instrument mute controls in a subordinate full-width song row, rather than interleaving them with the live transport. Hide empty instrument rows. Finite-song pause must be disabled with an explanation until the finite-song host supports preserving playback position; do not simulate it by stopping or muting.

## Runtime contract

Add `transport-pause` and `transport-continue` empty-body session messages using the existing scheduler freeze/release/resume machinery, extended to the internal clock. Pause must not revoke bindings or replay missed onsets; continue reanchors time at the frozen musical position. Reject unsupported finite-song pauses with an explicit protocol error. Existing MIDI transport behavior is preserved.

The frontend distinguishes transport clock progression from active playback content. Authoritative output state and playback activity control the visible counter; an initial running clock alone never starts it. Successful eval with no playable forms must not mark the editor playing. Stop/Cut clear local activity immediately and authoritative idle telemetry keeps it stopped. Paused views retain their position. Stale telemetry must not reactivate a locally stopped transport.

## Comparable applications

The design draws on functional grouping in Ableton Live, transport/display adjacency and secondary controls in Logic Pro, and explicit evaluate/update operations in Strudel. These are adapted to Vactr's live evaluation and effect-tail semantics, rather than copied literally.

See [external references](../references/README.md#editor-header-and-transport-2026-10-18).

## Unapplied edit highlights

Changes since the last successful evaluation have distinct markers: additions use translucent green, modifications use translucent yellow, and pure deletions use a red vertical line at the deletion position. Undoing an edit back to its applied text removes its highlight. Loading an unevaluated document marks it as pending.

Play, Update, and Evaluate form clear only edits confirmed by their successful reply. Failed forms stay pending. Edits made after submitting an evaluation remain pending, even when an older reply arrives. Partial evaluation clears only its successful region. Replacements crossing a partial-evaluation boundary stay highlighted until the entire replacement is successfully evaluated. Stop and Pause do not change the applied baseline.

Editor settings include a Modified text color picker and reset action. The six-digit RGB color persists locally, defaults to yellow (#facc15), and renders with fixed translucent opacity behind syntax text and selection. Additions remain green and deletions red. Storage failures fall back safely.

## File loading and edit history

Loading or replacing a file starts a fresh undo/redo history, including when the loaded text equals the current text. Vim's grouped undo ledger and pending insert session reset at the same boundary. Undo and redo continue to work within the newly loaded file, and saving a file preserves its editing history.

Vim yanks copy their text to the system clipboard as well as Vim registers, including normal-mode motions, line yanks and visual selections. The explicit black-hole register suppresses copying. Deletions retain existing register behavior. If clipboard access fails, keep the internal yank and report the failure.

## Rest playback highlights

In live scheduler bindings, literal `nil` rest steps remain silent, but their own source spans light up over their scheduled musical intervals. Playback queries capture rests separately from musical events so rests never supply sound/control content or change event counts, audio, MIDI, OSC, or hit signals. Nested steps and timing transforms map these intervals using the same query frames as musical events. Ordinary query/dry-run results retain their existing rest semantics.

Rest markers use normal bounded, revision-scoped playing announcements and confirmations, with an optional `rest` flag. Stopping, pausing, muting, rebindings, and retractions follow the same scheduling lifecycle as notes. The editor uses rest markers for source highlighting, without flashing a note hit.

## Removed live bindings on whole-document Update

A successful whole-document evaluation reconciles the evaluated file's live sinks. Slots previously bound by that file but absent from the successfully evaluated forms are naturally stopped, including when their source became comments, was deleted, or the document is empty. Existing tails may finish. Cleanup uses individual slot revocation and never globally stops or restarts other slots/files. Retired source forms lose reactive sink ownership, so later variable changes cannot resurrect removed music.

Reconciliation runs only if reading, expansion, checking, evaluation and runtime binding all succeed. Failed submissions preserve existing live bindings; successful individual forms retain the existing incremental-evaluation behavior. Console/REPL entries and partial-form evaluation remain incremental and do not infer removal of the document's other sinks. Ownership is recorded at runtime binding acceptance across all evaluation, tick-time reactive and immediate drain paths. Obsolete failed attempts also lose reactive sink ownership, without stopping another file’s audible binding. Accepted runtime provenance, rather than failed attempted rebindings, determines revocation, so one file cannot stop another file's sound after an unsuccessful takeover.
