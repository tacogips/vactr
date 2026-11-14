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
