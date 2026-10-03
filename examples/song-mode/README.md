# Finite song examples

`generated-parts.vact` declares self-contained digital drum instruments, so no external
sample banks are needed. A function returns a four-cycle multi-track Part;
another function calls it and returns a copy with only the bass drum filtered.
The arrangement plays the original twice and the modified Part four times,
then stops automatically. The original Part remains reusable.

At 120 BPM and four beats per cycle, its 24 cycles last 48 seconds. The declared
two-second tail makes the complete output 50 seconds.

From the repository root:

```sh
vactr run examples/song-mode/generated-parts.vact
vactr render examples/song-mode/generated-parts.vact generated-parts.wav --sample-rate 48000
```

Run requires an available native audio output and sufficient measured capacity.
A finite Song rejects `--cycles`, `--host noop`, and live audio input. Render
uses configured native headless audio and writes the complete stereo PCM16 WAV,
including its declared tail, without requiring an audio device. Errors leave an
existing output intact; the score itself cannot be used as the output path.

Call a zero-argument generator with `{drums & []}`. A bare `drums` refers to the
function itself. `part-repeat` repeats a complete Part a finite number of times;
`sequence` places Parts consecutively. No manual cycle count is needed.

## Editing a generated Part

Functions can accept a Part, edit a copy, and return it to another function.
For example, with the instruments and `drums` generator in the example:

```text
fn cut p:
	let events {part-events p :drums 0 2}
	let handle {{first events} :handle}
	let changed {delete-event p handle}
	overwrite-region changed :drums 1 2 {s :hat}

fn variation:
	let base {drums & []}
	cut base
```

This removes the first realized drum event, then replaces drum onsets in cycles
`[1, 2)` with a hat pattern starting at local zero. Other tracks remain in the
returned Part, and `base` remains reusable. `delete-event` requires the opaque
handle returned by `part-events` for that Part; a numeric event index is not a
handle. Enumerate again after an edit to obtain handles for the resulting Part.

| Operation | Arguments after the Part | Result |
|---|---|---|
| `part-repeat` | Count; optional `seed-mode: :same` or `:vary` | Finite repeated Part |
| `replace-track` | Track keyword, pattern | Copy with that track replaced |
| `transform-instrument` | Track, sound selector, pattern function | Copy with only matching sounds transformed |
| `part-events` | Track, start cycle, end cycle | Realized events with opaque handles |
| `delete-event` | Event handle | Copy with that event removed |
| `overwrite-region` | Track, start cycle, end cycle, pattern | Copy with onsets in that region replaced |
| `instrument-fx` | Track, sound selector, declared bus-chain keyword | Copy with a private effect route for that family |

Cycle positions accept integers and exact ratios such as `3/2`. Region endpoints
are half open: an onset at the end belongs to the following region. Repetition
and sequencing restart each placed Part at its own local zero.

The repository CLI fixture executed this file through `render --sample-rate
8000` and checked the complete 400000-frame (50-second) WAV, nonzero PCM, and
actual `Ended` completion. This headless witness does not substitute for a live
audio-device playback check.
