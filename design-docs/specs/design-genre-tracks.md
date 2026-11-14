# Original genre track collection

## Scope

Create three original instrumental songs in each requested category: ambient,
chill, house, hip-hop and pop. Fifteen standalone sample-free `.vact` scores
under `examples/tracks/<genre>/`, each with a title, BPM, tonal center, distinct
melody/harmony, genre-appropriate rhythm, at least three independent parts and
a 64-bar arrangement with introduction, development, contrasting section and
ending. Pop uses an instrumental lead hook; no vocals or lyrics requested.

Use current Vactr engines/effects. Preserve existing worktree changes and prior
techno examples. Scores declare their voices locally so checker/resolver and
rendering share definitions. Keep at most four named buses plus master, leaving
a free slot for master replacement. Gain automation over a 64-cycle phrase
controls section entrances/exits independently from note timing. Vary patterns
and timbres across songs; do not generate transposed copies of one loop.

## Deliverables

- Fifteen scores and `examples/tracks/README.md` with title/BPM/key/character
  and playback commands.
- Dependency-free headless `examples/render_track.rs` renderer using public
  production evaluator/runtime/native headless host APIs; no sound device.
  Accept score/output/duration or cycles; write stereo 16-bit PCM WAV at 48 kHz
  with a tail after scheduling stops, reject evaluation/install/runtime faults
  and nonfinite output, report frames/events/peak/RMS. Explicit header sizing,
  checked lengths and error handling; no silent normalization or clipping.
- Full-length WAVs in ignored `tmp/genre-tracks/<genre>/` and a metrics manifest.

## Verification

Every score evaluates, installs and renders its entire arrangement with
nonzero finite stereo sound and safe peak headroom. Check section-by-section
energy and tail, duration/header/sample rate; catch silent parts/overlap/clips.
Use listening previews and waveform evidence to revise poor balance. Required
Rust coding and check-and-test agents handle renderer source and checks. All
Cargo commands use CARGO_TERM_QUIET=true, no dependencies, all touched Rust
below 1000 lines. Existing frontend changes are unrelated work; preserve them.

## Shimmer ambient addition

Add three original 64-bar ambient scores under `examples/tracks/shimmer-ambient`.
Pitch-shifted reverb feedback supplies the central sustained upper texture;
soft pads, sparse glass notes and a clean low drone provide contrast. Use
separate shimmer and room buses so lead attacks stay intelligible. Two-bar
harmonic pacing, distinct tonal palettes, staged entrances, a quieter bridge,
and a tapered ending give each piece a complete arrangement. Reuse existing
DSP; extend the catalog and export listening page without changing old scores.

## Sampled breakcore addition

Create an original 64-bar breakcore score at 190 BPM using a downloaded,
explicitly CC0 synthetic Amen-pattern recreation. Preserve the upstream WAV,
source URL, license statement and checksum under
`examples/samples/amen-recreation`. The sample is a recreated drum loop, not
the original Winstons recording. Express all slicing and rearrangement in Vact using the intact source WAV,
with snare retriggers,
reverse edits, half-time contrast and distorted bass to develop the piece.

Enable the existing renderer's sample and source loaders through the public
NativeSampleLoader API, resolving paths relative to the score and bounded
project root. Reject missing or unauthorized paths. Keep existing synthesis,
scheduling, fault reporting and no-clipping guarantees. No new dependencies.
