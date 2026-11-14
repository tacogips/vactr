# Canvas Renderer Tuning Proposal (GPUI-informed)

Status: Proposal (2026-10-07). Inputs: [renderer comparison rc-001](design-renderer-comparison.md), the WebKit input diagnosis (`tmp/canvas-cutover/diag-webkit-input/REPORT.md` on `wf/canvas`), the GPUI study (`tmp/canvas-cutover/diag-shape/GPUI-BRIEF.md` on `wf/canvas`), and design 15.3.8.6-15.3.8.16.

## 1. What the canvas-vs-DOM comparison shows

| Cell (median of 3) | Input p95 canvas / DOM | Frame p95 | Text work p95 | Sync p95 canvas / DOM | Peak DOM nodes | GPU ledger |
|---|---|---|---|---|---|---|
| Chromium 1,000 lines | 16.4 / 16.1 ms | 18.2 / 18.2 ms | 5.7 / 5.9 ms | 20.5 / 20.6 ms | 843 / 1,258 | 7.15 MB / 0 |
| Chromium 20,000 lines | 16.2 / 16.2 ms | 17.5 / 17.5 ms | 4.7 / 5.2 ms | 18.9 / 14.8 ms | 843 / 1,220 | 7.15 MB / 0 |
| WebKit 1,000 lines | 24 / 38 ms | 19 / 19 ms | 4 / 4 ms | 108 / 39 ms (contended) | 862 / 1,275 | 7.15 MB / 0 |
| WebKit 20,000 lines | 52 / 39 ms | 18 / 18 ms | 4 / 3 ms | 20 / 35 ms | 862 / 1,239 | 7.15 MB / 0 |

Conclusions:

1. The renderer is no longer the bottleneck. Both renderers spend about 3-6 ms of JavaScript per text frame and 0-0.4 ms per animation frame, and frame p95 is identical. The differences in input and sync percentiles are within run-to-run variance (WebKit canvas 20,000-line input p95 ranged 33-58 ms over three runs).
2. The remaining gate failures in both renderers come from shared main-thread work, not drawing: the deferred tree-sitter reparse (26-47 ms per edit in WebKit), the diagnostics check (about 32 ms), and Playing telemetry committed only 30 ms ahead. These are being fixed in the canvas branch now (design 15.3.8.16: syntax Worker, diagnostics off the keystroke path, earlier telemetry).
3. Canvas keeps its structural advantages: about one-third fewer DOM nodes, deterministic GPU-composited highlights and visuals on one surface, and no layout/style work for text. Its cost is about 7 MB of GPU memory (atlas plus layer buffers).

Recommendation: keep the canvas renderer (as the goal requires) and spend effort first on main-thread decongestion (P0), then on the GPUI-style items below, which reduce CPU churn and synchronous GPU round trips. These matter most on WebKit/iPad.

## 2. Ranked tuning points

Each item lists the GPUI technique, the vactr change, the expected benefit (WebKit first), effort/risk, and deterministic verification. Items marked [data] should be confirmed by measurement before committing.

### P0. Main-thread decongestion (in progress, design 15.3.8.16)
- GPUI/Zed analogue: Zed reparses on a background thread with a 1 ms synchronous budget and keeps highlighting through an interpolated tree.
- vactr: tree-sitter in a Worker with revision-tagged span batches and the mapped-span interim; `session_check` off the keystroke path; Playing telemetry published within the 120 ms scheduling lookahead.
- Benefit: removes the 26-47 ms reparse and the 32 ms check from the keystroke-to-frame path. These are the only measured causes of WebKit input p95 above 50 ms and of sync p99 outliers.
- Verify: counters showing no same-task parse after a keystroke, and the quiet-host canonical run.

### P1. GPU-driven highlight overlay (user request: overlay with minimal drawing)
- GPUI analogue: retained scene primitives replayed from cache, with only uniforms changing per frame.
- vactr: upload playing and beat highlight ranges once per evaluation or document revision as overlay instances `{rect, startAudible, endAudible, epoch, kind}`. Per frame, write one uniform (current audible time, beat phase) and let the overlay shader compute active, fade and pulse.
- Benefit: an animation-only frame becomes 1 uniform write plus 1-2 draws, with 0 instance writes and 0 buffer uploads, independent of voice count (64 voices today). Highlight presentation no longer depends on CPU work in the frame. This is terminal-style "redraw nothing that did not change" applied to animation.
- Effort M, risk low (shader plus range upload; keep the CPU path as fallback for context restore).
- Verify: bytes uploaded per animation-only frame == 0, uniform writes == 1, draws <= 2; existing audio-domain early-flash and stall-recovery gates unchanged.

### P2. Per-layer VAOs and cached attribute locations
- GPUI analogue: pipeline state created once, with per-batch binding only.
- vactr: create the 4 VAOs at init and remove the per-frame `getAttribLocation` and `vertexAttribPointer` calls (about 36 per frame). In WebKit's GPU-process WebGL these are synchronous round trips.
- Effort XS, risk none. Verify: fake-GL counters `getAttribLocation == 0` and `vertexAttribPointer == 0` over 100 frames after init.

### P3. Revision-based invalidation instead of per-frame key strings
- GPUI analogue: `cx.notify()` dirty views; cached views replay when not dirty.
- vactr: replace the per-frame `syntaxKey` (built over all spans), `backgroundKey`, `overlayKey` and `cacheKey` strings with numeric revisions (syntax, annotations, viewport struct, font generation, atlas generation); cache segment keys on rows.
- Benefit: removes O(spans) string work on every animation frame and reduces JSC GC pressure. Effort S, risk low.
- Verify: `stats.keyBuilds == 0` on animation-only frames.

### P4. Pre-resolved styles, colors and per-line spans
- GPUI analogue: arena discipline; no per-frame parsing.
- vactr: resolve palette colors to RGBA once; pass per-line span slices to `drawRun`/`styleHash`, replacing `styles.find` per cluster; cache the emoji check per atlas cell.
- Benefit: text-dirty frame CPU on both engines (text work p95 3-6 ms toward 1-2 ms). Effort S-M.
- Verify: zero `parseCssColor` calls on the hot path; span comparisons <= spans + visible lines.

### P5. Packed-bytes segment cache
- GPUI analogue: `reuse_paint` copies primitive ranges.
- vactr: store cached segments as final 32-byte instance bytes and replay with `Uint8Array.set`; remove the O(buffer) byte compare and `slice()` per text frame.
- Effort M, risk medium. Verify: instance pushes per text frame == newly built instances only.

### P6. Glyph atlas: R8 mask texture, numeric keys, batched shelf uploads [data]
- GPUI analogue: A8 monochrome atlas plus BGRA color atlas, struct keys, bucketed allocator, no per-frame eviction.
- vactr: R8 mask atlas (4x less memory and upload bandwidth; cuts most of the 7.15 MB ledger, which matters on iPad), an RGBA atlas for emoji only, numeric keys for ASCII cells, and one `texSubImage2D` per touched shelf.
- Benefit: memory and upload cost; fewer atlas resets. Effort M, risk medium (WebKit R8 upload probe).
- Verify: ledger bytes; uploads <= shelves touched.

### P7. Frame pacing for input
- GPUI analogue: keep presenting while input arrives at a high rate; render synchronously in ResizeObserver on web.
- vactr: present the edited-line frame in the same rAF as input handling (no extra frame hop), render synchronously on ResizeObserver for rotation and keyboard insets, and optionally request `desynchronized` (Chromium latency hint, ignored by WebKit).
- Benefit: removes up to one frame (16.7 ms) of input-to-photon latency in some paths. [data] Measure with the input-latency pairing.

### P8. Subpixel-quantized glyph placement (quality)
- GPUI analogue: 4x1 subpixel variants and integer origins.
- vactr: quantize glyph quads to device pixels or 1/4-px variants. Improves sharpness and makes per-glyph cost consistent. Schedule after P1-P5.

### P9. Buffer orphaning or double buffers for whole-layer rewrites [data]
- GPUI analogue: triple-buffered instance pool.
- vactr: `bufferData(size)` before full rewrites, or rotating buffers, to avoid implicit CPU-GPU sync. Only worth doing with a measured stall.

## 3. Not recommended
- GPUI's WebGL instance-texture transport: vactr's divisor instancing is the better WebGL2 primitive.
- Full per-frame scene rebuild: vactr's retained per-line segments suit JavaScript better.
- Sustained 120 Hz presentation: browsers own the rAF rate. iPad Safari is 60 Hz by default; WKWebView in the Tauri app may allow ProMotion. Treat this as a platform limitation and verify on physical hardware.

## 4. Suggested order
P0 (running) -> P1 -> P2 -> P3 -> P4 -> re-measure on a quiet host -> P5/P6/P7 by measured need -> P8 -> P9 only with evidence.
