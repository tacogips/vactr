# Pending Bass Voice Questions

Open decisions for the bass voices work
([`design-bass-voices.md`](../specs/design-bass-voices.md)). Implementation
follows each recommendation by default. None of them blocks the accepted
scope.

## BQ1: Automatic previous-note slide

The chosen mechanism is explicit and per event: the pattern writes
`slide-from` as the interval from the previous note, and `cut 1` chokes the
previous voice. A TB-303-style "slide flag" that finds the previous pitch by
itself would need an engine-side handoff. `Engine::start` would read the
previous same-cut voice's pitch and envelope state before `choke_cut_group`.

- Option A: keep explicit `slide-from` only. This is the current scope.
- Option B: add the engine handoff later as a separate change, with `slide`
  as a flag.

Recommendation: A now, and revisit B after the bass voices ship.

## BQ2: Held-key sustain for bass templates

`bass-core` owns its note-off through `gate-length`, in tempo-relative
sixteenth-steps, because audio events carry no duration. A live-input or MIDI
voice that holds a key therefore does not sustain past `gate-length`.

- Option A: pattern-first; the held-key limitation is documented.
- Option B: note-off at the later of `gate-length` and the voice gate
  (`kx.gate`).

Recommendation: A. Option B would change how scheduled voices behave when
`attack + decay` exceeds `gate-length`.

## BQ3: Rumble as a template

"Rumble" is built in `examples/rumble-techno.vact` from `sub-bass` or
`reese-bass` and existing effects. It is not a seventh template.

Recommendation: keep it as an example recipe unless a dedicated template is
requested.
