# Lo-fi production effect and track collection

## Overview

Add a convenient `lofi` bus effect and nine original instrumental scores: three each for lo-fi hip-hop, lo-fi ambient, and lo-fi chill. Preserve the existing fifteen genre tracks. Each new score has a complete 64-bar arrangement and a full stereo WAV export.

## Effect design

Existing bitcrush, decimate, tape, cassette, vinyl, and wow-flutter effects already cover individual degradation processes. The new effect combines independently controlled bandwidth reduction, level-normalized saturation, shared stereo tape modulation, sample-and-hold, amplitude quantization, and optional surface noise into one bus slot. This makes musical lo-fi treatment possible without exhausting the six-slot bus capacity.

Public controls: `tone` in Hz, `drive`, `wow`, `flutter`, `bits`, `rate` in Hz, `hiss` (0 to 1), `crackle` (0 to 1), and `mix`. Default processing is subtle. Hiss and crackle default to zero and must be exactly disableable. Dry mix is transparent. Delay memory is bounded and allocated during installation. Audio processing must remain allocation free and block partition invariant. Controls are clamped to supported bounds, with nonfinite values handled through existing effect conventions. Stereo channels have independent filter, delay, and sample-hold histories; tape timing is shared to avoid image instability. No external dependency or third-party source code is required.

This is a stylized production effect, not a physical magnetic hysteresis model. Implement independently from researched algorithm descriptions. Preserve all existing serialized effect tags and integrate graph names, metadata, lowering, native and browser hosts.

## Compositions

Create nine distinct scores under `examples/tracks/lofi-hiphop`, `lofi-ambient`, and `lofi-chill`. Use new melodies and harmony, genre-appropriate rhythm, soft percussion, audible but restrained degradation, contrasting sections, and a tapered ending. Include the effect on melodic or drum buses; keep sub-bass and kick weight controlled. Extend the catalog and listening page to all twenty-four scores.

## References

- MusicDSP, [Lo-Fi Crusher](https://www.musicdsp.org/en/latest/Effects/139-lo-fi-crusher.html): amplitude quantization and sample-and-hold reduction.
- MusicDSP, [Decimator](https://www.musicdsp.org/en/latest/Effects/124-decimator.html): fractional-rate accumulation and held samples.
- Jatin Chowdhury, [Real-time Physical Modelling for Analog Tape Machines](https://www.dafx.de/paper-archive/2019/DAFx2019_paper_3.pdf), DAFx 2019, CC BY 3.0: tape bandwidth losses, saturation, hiss, and delay-based wow/flutter. Conceptual reference only; no plugin code copied.

## Control defaults

| Control | Default | Range |
|---------|---------|-------|
| tone | 6500 Hz | 200–20000 Hz |
| drive | 1.3 | 0.1–12 |
| wow | 0.08 | 0–1 |
| flutter | 0.03 | 0–1 |
| bits | 16 | 2–24 |
| rate | 32000 Hz | 500–96000 Hz |
| hiss | 0 | 0–1 |
| crackle | 0 | 0–1 |
| mix | 1 | 0–1 |

[Clean/aged demo](../../examples/lofi-lab.vact) alternates four-bar statements.
