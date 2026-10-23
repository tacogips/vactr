# Editor header and menus

Status: Implemented, 2026-10-18 (direct user request).

## Behavior and hierarchy

The header presents document navigation first, playback second, musical position third, and output/settings last. Preserve the flat, square token system in design-ui-style.md. Group related controls with visible spacing and concise labels. Keep the primary transport on one row where space permits; wrap whole groups on small screens without clipping menus.

- Documents opens the existing document/example library. Show the current document title and an unsaved indicator alongside it. File actions stay in the document dialog.
- One primary button toggles Play and Stop. Play evaluates the document from stopped state; Stop releases every slot and lets effect tails finish. While paused, the primary button remains Stop, and the adjacent Pause button toggles Resume, preserving existing bindings and musical position.
- Update is a separate secondary action for reevaluating edits while playback continues. Existing evaluation shortcuts retain their meaning.
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

Changes since the last successful evaluation have a translucent yellow background. Insertions and replacements highlight their changed text; pure deletions show a wider marker at the deletion position. Undoing an edit back to its applied text removes its highlight. Loading an unevaluated document marks it as pending.

Play, Update, and Evaluate form clear only edits confirmed by their successful reply. Failed forms stay pending. Edits made after submitting an evaluation remain pending, even when an older reply arrives. Partial evaluation clears only its successful region. Replacements crossing a partial-evaluation boundary stay highlighted until the entire replacement is successfully evaluated. Stop and Pause do not change the applied baseline.

Editor settings include an Unapplied edit color picker and reset action. The six-digit RGB color persists locally, defaults to yellow (#facc15), and renders with fixed translucent opacity behind syntax text and selection. Storage failures fall back safely.
