# Original Vactr track collection

Original instrumentals with locally declared voices. Most genres have three
tracks; breakcore uses the licensed sample described below. Each has a 64-bar arrangement: intro, A, A+, contrast, B,
B+, reprise and ending. Source patterns repeat after the arrangement; use
`--cycles 64` for the complete song. Pop tracks use instrumental lead hooks.

| Genre | Title | BPM | Tonal center | Character |
|-------|-------|-----|--------------|-----------|
| ambient | [Tidal Glass](ambient/tidal-glass.vact) | 62 | D major / B minor | Slow suspended harmony, isolated glass notes, octave shimmer. |
| ambient | [Lichen Orbit](ambient/lichen-orbit.vact) | 68 | F Lydian | Warm drone, sparse asymmetric plucks and a gently swelling room. |
| ambient | [Night Transit](ambient/night-transit.vact) | 74 | C minor | Dark minor pads, low heartbeat and drifting high reflections. |
| chill | [Window Seat](chill/window-seat.vact) | 88 | A minor | Laid-back syncopated bass, warm electric keys and restrained drums. |
| chill | [Blue Hour](chill/blue-hour.vact) | 96 | E minor | Dusty broken beat, suspended chords, answering bell phrases. |
| chill | [Citrus Rain](chill/citrus-rain.vact) | 102 | G major | Light upbeat groove, bright plucks and spacious major-seventh harmony. |
| house | [Warm Current](house/warm-current.vact) | 122 | F minor | Deep four-on-the-floor, offbeat chords, rounded syncopated bass. |
| house | [Rooftop Signals](house/rooftop-signals.vact) | 126 | A Dorian | Bouncy organ-like stabs, percussion accents and a climbing hook. |
| house | [Velvet Floor](house/velvet-floor.vact) | 118 | C minor | Slow deep-house pocket, rich extended harmony and sparse high melody. |
| hiphop | [Paper Lanterns](hiphop/paper-lanterns.vact) | 82 | D minor | Sparse boom-bap, low sine bass and a wistful electric-key hook. |
| hiphop | [Concrete Garden](hiphop/concrete-garden.vact) | 92 | F-sharp minor | Tense minor motif, heavy backbeat and staggered bass answers. |
| hiphop | [Satellite Dust](hiphop/satellite-dust.vact) | 76 | B minor | Half-time drums, triplet hat flashes and a soft nocturnal motif. |
| pop | [Neon Postcard](pop/neon-postcard.vact) | 112 | C major | Bright chorus hook, octave bass and a clear verse/chorus contrast. |
| pop | [Small Sun](pop/small-sun.vact) | 104 | G major | Sunny offbeat keys, singable repeating motif and a softer middle eight. |
| pop | [After the Rain](pop/after-the-rain.vact) | 98 | B-flat major | Reflective verse, wide chorus pads and a rising instrumental refrain. |
| lofi-hiphop | [Amber Notebook](lofi-hiphop/amber-notebook.vact) | 78 | E minor | Warm minor ninths, a sparse backbeat and a descending electric-key answer. |
| lofi-hiphop | [Sidewalk Polaroid](lofi-hiphop/sidewalk-polaroid.vact) | 86 | A-flat major | Major-seventh warmth, clipped chord replies and a lopsided kick pocket. |
| lofi-hiphop | [Last Train Home](lofi-hiphop/last-train-home.vact) | 72 | G minor | A slow half-time pocket, low rounded bass and a nocturnal bell refrain. |
| lofi-ambient | [Faded Atlas](lofi-ambient/faded-atlas.vact) | 58 | A minor | Slow ninth chords, tape-softened plucks and long diffuse reflections. |
| lofi-ambient | [Moss Cassette](lofi-ambient/moss-cassette.vact) | 64 | D major | Gentle major harmony, a soft wandering drone and restrained cassette flutter. |
| lofi-ambient | [Snow on the Dial](lofi-ambient/snow-on-the-dial.vact) | 66 | F minor | Dark suspended pads, isolated high notes and a narrow drifting tape image. |
| lofi-chill | [Sunday Receipts](lofi-chill/sunday-receipts.vact) | 90 | C major | Relaxed major-seventh keys, a light broken beat and a rising plucked hook. |
| lofi-chill | [Tea-Stained Map](lofi-chill/tea-stained-map.vact) | 94 | B minor | Dorian-colored chords, syncopated bass and short spring-room answers. |
| lofi-chill | [Peach Window](lofi-chill/peach-window.vact) | 84 | F major | Soft sixth-like melodic colors, brushed noise percussion and tape echoes. |
| shimmer-ambient | [Aurora Veil](shimmer-ambient/aurora-veil.vact) | 58 | D Lydian | Slow luminous Lydian pads, high glass droplets and an octave-up halo. |
| shimmer-ambient | [Prismatic Shore](shimmer-ambient/prismatic-shore.vact) | 66 | E-flat major | Warm extended harmony, answering bell phrases and drifting crystalline reflections. |
| shimmer-ambient | [Stars Beneath Ice](shimmer-ambient/stars-beneath-ice.vact) | 72 | F-sharp minor | Dark slow-moving ninths, distant glass notes and a fifth-shifted frozen halo. |
| breakcore | [Glass Teeth](breakcore/glass-teeth.vact) | 190 | C minor | 190 BPM chopped CC0 Amen recreation, snare retriggers, reverse edits, half-time contrast and a glassy halo. |

## Shimmer ambient and sampled breakcore

The three shimmer ambient pieces use separate pitch-feedback and room buses.
Glass Teeth uses the [CC0 Amen-pattern recreation](../samples/amen-recreation/README.md)
from SampleLoom. All chopping, index rearrangement, retriggers and reverse
playback are expressed in the score, using the complete downloaded WAV.
`start-ms 123.456` denotes 123 milliseconds plus 456 microseconds; `stop-ms`
is exclusive. See [the timestamp and chop demo](../sample-timestamps.vact)
for straight chops, reordered/retriggered slices, reverse audio, and direct
position lists using the same source WAV.

## Lo-fi production

The nine lo-fi scores use the combined `lofi` bus effect for softened bandwidth,
subtle pitch drift, saturation, sample reduction and quiet surface texture.
Each has original harmony, melody, rhythm and a complete arrangement.
See [the effect demo](../lofi-lab.vact) for alternating clean and aged statements.

## Play and export

```sh
target/debug/vactr run examples/tracks/house/warm-current.vact --host native --cycles 64
CARGO_TERM_QUIET=true mise exec -- cargo run --release --example render_track -- examples/tracks/ambient/tidal-glass.vact tmp/genre-tracks/ambient/tidal-glass.wav --cycles 64 --bpm 62 --tail-seconds 8
```

Full stereo 48 kHz WAVs are exported under `tmp/genre-tracks/<genre>/`. Render
metrics and section energy evidence are in `tmp/genre-tracks/metrics.json`.
The WAV folder is ignored by Git; the editable scores are versionable.

Export the entire collection and rebuild the listening page:

```sh
CARGO_TERM_QUIET=true mise exec -- cargo build --release --example render_track
python3 examples/tracks/render_all.py
```

Open `tmp/genre-tracks/index.html` for all twenty-eight full songs and 24-second
refrain previews. `metrics.json` includes source hashes, measured headroom,
eight-bar section energy, stereo channel RMS and exact WAV frame counts.

## Verified export collection

The collection contains 28 full stereo WAVs (about 90 minutes), with a
24-second preview per song. Open the [local listening page](../../tmp/genre-tracks/index.html).
Exports use the actual production audio engine and reject install faults,
nonfinite samples, clipping and late/dropped/stolen/skipped events.
Source hashes match every exported score; the manifest records exact frame
counts, stereo levels, eight arrangement sections and ending/tail measurements.

Verification: 1699 project tests and eleven renderer tests passed, with no
failures and two pre-existing ignored tests. Formatting, Clippy, native and
wasm builds passed. Independent PCM auditing confirmed all 28 songs and previews, current source hashes,
and all 84 listening-page links.
