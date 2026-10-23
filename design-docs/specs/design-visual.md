# vactr Visual Specification (working draft)

Supporting document to `lang-reference.md`: visuals as Hydra-style
texture chains. Music and visuals are specified separately (author,
2026-09-24); their coupling is deferred (`notes.md`, "music-visual
coupling").

Conventions are those of `lang-reference.md` (`# Decided`, `# => v`).

## 1. Visuals (Hydra model)

```vactr
# hydra style: source -> transforms -> output. A chain fits directly,
# and `out o0` is the sink (the visual counterpart of `d1`), so no
# wrapper form is needed. `out` returns the chain it bound.
osc 20 0.1 0.8
	> rotate 0.5
	> kaleid 4
	> modulate {noise 3} 0.2
	> color 1 0.5 0.2
	> out o0
# sources:    osc noise voronoi shape gradient solid src
# geometry:   rotate scale pixelate tile tile-x tile-y kaleid scroll
#             (tile is Hydra's repeat; `repeat` here means replicate)
# color:      posterize shift invert contrast brightness luma thresh
#             color saturate hue colorama
# blend:      add sub layer blend mult diff mask
# modulate:   modulate modulate-tile modulate-kaleid modulate-scroll
#             modulate-rotate modulate-scale modulate-pixelate
# outputs:    o0 o1 o2 o3; `render o0` shows one, `render` shows all
# Decided (author, 2026-09-24): sound sinks (`d1`..`d9`/`slot`) and
# visual sinks (`out o0`..`o3`) keep their familiar Tidal/Hydra names,
# both implemented over ONE slot table, so `hush`/`stop` treat any slot
# alike regardless of which name created it.
hush            # silences every slot, sound and visual
stop :drums     # stops the slot named :drums, whichever kind it holds

# time-varying parameters: any number may be a lambda of time
osc {* 20 {sin time}}            # `time` is the seconds signal
# or a signal, the same signals as `design-music.md`, section 3
osc {range sine 10 30}
	> out
# Decided (implication 2026-09-24): a param is number | signal | pattern |
#   fn of time; sequences are patterns of any type

# audio-reactive (Hydra-native): the host supplies audio input analysis
# as signals; `fft n` is a band level (0..1), `amp` overall amplitude.
# Coupling to vactr's own music slots is a separate, deferred topic
# (notes.md, "music-visual coupling").
shape 4
	> scale {+ 1 {fft 0}}            # bass band, 0..1
	> out

'# on-screen text is a source, the one place a string is a string
text "hello" > scale 0.5 > out o1
# Withdrawn with the imperative layer: the p5-style `draw:` frame loop.
# A visual is always a chain; motion comes from signals (`time`, `sine`,
# `fft`), never from a loop body.

# Decided (author, 2026-09-24): no raw GLSL and no `"""` multi-line
# string in v1; visuals are built from the Hydra-style operator chains
# above only. Raw shader source, and whatever multi-line string syntax
# it needs, is deferred to a later host capability.

# browser and native share this API; the host supplies the canvas.
# Decided (author, 2026-09-24): canvas/window configuration is a
# language-level call, mirroring `use-bpm` for audio; the host
# capability implements it underneath.
use-fps 60
use-canvas 800 600
```


## 2. Vocabulary

| Area | Functions |
|------|-----------|
| visuals (Hydra names) | `osc`, `noise`, `voronoi`, `shape`, `gradient`, `solid`, `src`, `text`, `rotate`, `scale`, `pixelate`, `tile`, `kaleid`, `scroll`, `posterize`, `shift`, `invert`, `contrast`, `brightness`, `luma`, `thresh`, `color`, `saturate`, `hue`, `colorama`, `add`, `sub`, `layer`, `blend`, `mult`, `diff`, `mask`, `modulate` family, `out`, `render`, `use-fps`, `use-canvas` |

## Editor background comments (2026-10-18)

The editor accepts own-line `#@ editor` comments followed by a JSON object.
These are editor presentation settings, separate from musical evaluation and
parameter/MIDI directives. Editing, loading, undoing or removing the comments
updates the presentation immediately without evaluating or rewriting code.
Multiple comments merge their top-level keys in source order; a later
`background` or `visualizers` value replaces that entire setting. Removing all
comments restores the normal editor appearance. Invalid lines report their
location in the Visuals panel and do not execute any code.

```vactr
#@ editor {"background":{"kind":"image","src":"/media/cover.png","opacity":0.7,"scrim":0.55}}
#@ editor {"visualizers":[{"kind":"oscilloscope","opacity":0.6,"position":"bottom"}]}
```

Background kinds are `image`, `video`, `shader` and `none`. Image/video take
`src` (relative, HTTP(S), blob, or an appropriate image/video data URL),
`fit` (`cover` or `contain`), `opacity` (0..1), `blur` (0..32 CSS pixels),
and `scrim` (0..1, darkness behind code; default 0.55). Shader takes `output`
(0..3) and uses an existing visual chain's rendered output; it does not
create a shader program. Defaults are opacity 1, cover, and no blur. Videos
are muted, looped and inline, suspend when the document is hidden, and
release their source when replaced/removed. Failed decoding/loading or an
unavailable shader source is visible in the Visuals panel. Unsafe URL schemes
are rejected. Relative URLs resolve against the editor page: local media must
be served or packaged with the app; source comments do not grant filesystem
access. Native CSP allows the same HTTPS/media sources as the browser.

`visualizers` is an array of up to four master-output overlays. Kinds are
`oscilloscope`, `spectrum`, `spectrogram` and `level`; each takes `opacity`
(0..1, default 0.6) and `position` (`bottom`, `top`, `full`; default bottom).
The scope uses real final-output samples, spectrum uses the master eight FFT
bands, spectrogram retains a bounded history of those bands, and level shows
master RMS. Missing/stale waveform data is reported instead of drawing a fake
scope. These overlays do not require explicit analyzer units in user code and
work for live and finite-song audio. Existing bus-specific analyzer displays
continue to work independently.

The DSP already records the final gated/clamped combined output, including
finite-song branches, in a 512-frame ring. Publish its newest 256 mono samples
chronologically through the existing host-signals path and optional
`levels.levels[].waveform` telemetry. No new graph, analyzer IDs, render-source
code, microphone input, or frame-time audio allocation is needed. Older peers
may omit waveform; other existing master telemetry remains usable.

DOM media and a bounded 2D overlay canvas sit below the transparent GPU code
canvas. Background frames are never copied into the text renderer, and changing
background settings does not evict text geometry. The code scrim is configurable
and resets to its existing default when comments are removed. Media lifecycle,
source replacement, resource reservations, listeners and animation frames must
be released on editor disposal. Parsing should inspect changed lines before
rescanning settings, preserving ordinary large-document edit cost.
