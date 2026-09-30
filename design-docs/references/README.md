# Design References

This directory contains reference materials for system design and implementation.

## External References

### Rust

| Name | URL | Description |
|------|-----|-------------|
| The Rust Book | https://doc.rust-lang.org/book/ | Official Rust programming language book |
| Rust API Guidelines | https://rust-lang.github.io/api-guidelines/ | Rust API design best practices |
| Rust Design Patterns | https://rust-unofficial.github.io/patterns/ | Common Rust design patterns |

### Indent-based Lisp Syntax

| Name | URL | Description |
|------|-----|-------------|
| SRFI-119 (Wisp) | https://srfi.schemers.org/srfi-119/srfi-119.html | Minimal whitespace-to-list rules |
| SRFI-110 (Sweet-expressions) | https://srfi.schemers.org/srfi-110/srfi-110.html | Wisp-like rules plus `f(x)` and `{infix}` |
| Rhombus / Shrubbery | https://docs.racket-lang.org/shrubbery/ | Group-based extensible notation used by Rhombus |
| Rhombus paper | https://doi.org/10.1145/3622818 | "A New Spin on Macros without All the Parentheses" (OOPSLA 2023) |

### Live Coding and Runtime Design

| Name | URL | Description |
|------|-----|-------------|
| Sonic Pi | https://sonic-pi.net/ | Music live-coding environment; its imperative model was evaluated and withdrawn |
| Overtone | https://overtone.github.io/ | Clojure audio environment; instruments as unit-generator chains (`definst`) |
| Hydra | https://hydra.ojack.xyz/ | Browser visual live coding; texture chains |
| TidalCycles | https://tidalcycles.org/ | Pattern-based music live coding |
| Clojure Vars | https://clojure.org/reference/vars | Late-bound var indirection enabling live redefinition |
| Cranelift | https://cranelift.dev/ | Rust-native code generator for a future JIT backend |
| WebAssembly GC | https://github.com/WebAssembly/gc | GC proposal relevant to the Frozen-mode Wasm backend |
| DaisySP drum synthesis | https://github.com/daisyaudio/DaisySP/tree/master/Source/Drums | MIT-licensed synthetic bass drum, snare drum, and hi-hat modules; possible source for a commercial-use-compatible implementation |
| DaisySP license | https://github.com/daisyaudio/DaisySP/blob/master/LICENSE | MIT terms and notices for DaisySP, Plaits, and Soundpipe code; retain applicable notices if code is ported |
| Japan Agency for Cultural Affairs copyright Q19 | https://www.bunka.go.jp/seisaku/chosakuken/seidokaisetsu/toroku_seido/pdf/93977001_01.pdf | Ideas and methods are distinct from their protected expression |
| US Copyright Office: Computer Programs | https://www.copyright.gov/register/tx-programs.html | Program expression is protected; ideas, algorithms, and methods are not |
| Mutable Instruments Eurorack source | https://github.com/pichenettes/eurorack | Official module source and per-platform license statement: STM32 code MIT, AVR code GPL-3.0; also contains naming guidance for derivatives |
| Mutable Instruments module documentation | https://pichenettes.github.io/mutable-instruments-documentation/ | Official module inventory and control descriptions; use to check every exposed audio function |
| Warps vocoder and limiter source | https://github.com/pichenettes/eurorack/tree/08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4/warps/dsp | Individually MIT-noticed vocoder, filter bank, sample-rate converter and output limiter; preserve notices for translated stages |
| Warps filter-bank generator | https://github.com/pichenettes/eurorack/blob/08460a69a7e1f7a81c5a2abcc7189c9a6b7208d4/warps/resources/filter_bank.py | Individually MIT-noticed 20-band coefficient generator; distinguish filter coefficients from oscillator wavetable assets |
| Peaks FM drum source | https://github.com/pichenettes/eurorack/blob/master/peaks/drums/fm_drum.cc | MIT-licensed triggered sine FM percussion with pitch/amplitude envelopes, noise, and drive |
| Faust FM/noise/feedback percussion voice | https://gist.github.com/SpotlightKid/9e3b14d5cda5c763c9db7e5e24cfe352 | Separate FM drum candidate declaring MIT; its Faust-library dependencies require a license check before any code incorporation |
| Faust DSP library provenance | https://github.com/grame-cncm/faustlibraries | `stdfaust.lib` dependencies have function-specific license markers; the compiled-output exception does not automatically apply to manual Rust source rewrites |
| MD Drum Synth Emulator | https://github.com/ctag-fh-kiel/md-drum-synth | Seven FM drum models, but no repository license grant was found; source is excluded pending permission |
| EFM synthesis thesis | https://www.elektronauts.com/uploads/short-url/xc71Y6sXFDP4ZB0MqtkE8NjuEwx.pdf | Erik Larsson's 2000 thesis on FM percussion signal flow and voice families; research only, not a code or asset license |
| EFM thesis accessible PDF mirror | https://communiteq-eu5.nbg1.your-objectstorage.com/uploads/db8181/original/3X/e/8/e8a6485226730cb92bbeaebd6c8ae4b82359c0a1.pdf | Accessible scan of Larsson's 2000 Chalmers thesis; no reuse grant for its assembly, diagrams, text, or data |
| Official Machinedrum manual | https://www.elektron.se/wp-content/uploads/2024/09/machinedrum_manual_OS1.63.pdf | Product-level EFM family description; research context only, no firmware or asset rights |
| Machinedrum voices discussion | https://www.elektronauts.com/t/md-voices-diagram/173460 | Forum discussion linking the scanned EFM thesis and voice diagrams; explanatory reference, no code or diagram reuse |
| Gearmulator Machinedrum emulator | https://github.com/joelanders/gearmulator-md-mm | GPL-3.0 emulator that requires original firmware images; excluded from MIT code-port scope |
| eseq Machinedrum design | https://github.com/universalsequences/eseq/blob/main/docs/machinedrum.md | GPL-3.0 synth design; research reference only, source excluded from MIT code-port scope |
| MAME Machinedrum skeleton | https://github.com/mamedev/mame/blob/master/src/mame/elektron/elektronmono.cpp | BSD-3-Clause hardware driver, but no self-contained drum DSP to port and no rights to Elektron firmware |
| 0x808 synthesis engine | https://github.com/averagenative/0x808 | MIT-declared general four-operator FM synthesizer; a candidate for future source review, not a dedicated FM drum port |
| stmlib license | https://github.com/pichenettes/stmlib/blob/master/LICENSE | Mostly MIT, with separately licensed ST and serial-programming subtrees that must not enter Vactr's audio code |
| Upstream file inventory | `verification/upstream_inventory.toml` | Per-file license, kind and use for every Mutable Instruments and `stmlib` file Vactr names; checked by `mise run audit-upstream` against a pinned checkout. `elements/resources/samples.py` and `stmlib/ui/event_queue.h` are GPL-3.0 and excluded |

## Reference Documents

Reference documents should be organized by topic:

```
references/
├── README.md              # This index file
├── rust/                  # Rust patterns and practices
└── <topic>/               # Other topic-specific references
```

## Adding References

When adding new reference materials:

1. Create a topic directory if it does not exist
2. Add reference documents with clear naming
3. Update this README.md with the reference entry

## Rumble compressor research

| Source | URL | Use |
|--------|-----|-----|
| Citizen Chunk, Simple Compressor class | https://www.musicdsp.org/en/latest/Effects/204-simple-compressor-class-c.html | Author's explanation of feed-forward peak detection before gain reduction and stereo linking; research only |
| Bram, asymmetric envelope follower | https://www.musicdsp.org/en/latest/Analysis/136-envelope-follower-with-different-attack-and-release.html | Attack/release coefficient derivation and explicit 99 percent versus 63.2 percent settling-time distinction; research only |

## MusicDSP effects expansion

See [pinned source audit](musicdsp-audit.md) for all Effects/Filters/Synthesis entries,
individual notices, overlap decisions and primary spring/reverb references.
The [expansion design](../specs/design-musicdsp-effects.md) defines the selected
independent algorithms and verification requirements.

## Lo-fi DSP research

- [MusicDSP Lo-Fi Crusher](https://www.musicdsp.org/en/latest/Effects/139-lo-fi-crusher.html): quantization and held samples; independent implementation.
- [MusicDSP Decimator](https://www.musicdsp.org/en/latest/Effects/124-decimator.html): fractional sample-rate reduction; independent implementation.
- [Jatin Chowdhury, DAFx 2019 tape model](https://www.dafx.de/paper-archive/2019/DAFx2019_paper_3.pdf): bandwidth losses, saturation, hiss and delay-based flutter; conceptual reference, CC BY 3.0.

## Breakcore sample source

[SampleLoom synthetic Amen recreation](https://sampleloom.com/sample/jungle-amen-break-slice-165bpm-4-bars-262cb3) lists CC0. The original download and checksum are retained in examples/samples/amen-recreation; the track edits the complete source in Vact.
