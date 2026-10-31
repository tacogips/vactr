# EMK-SHOWCASE: plugin showcase page with video, animation and format demo plugins

**Status**: Ready (after EMK-API)
**Plan ID**: EMK-SHOWCASE (wave 4; parallel with EMK-DIFF-DEMO, EMK-JSON-DEMO, EMK-VAC-PLUGINS)
**Design Reference**: `design-docs/specs/design-emaki.md` sections 8.1, 11.1; requirement 3 (demo plugins: video widget, animated canvas widget, mark plugin)
**Rules**: `impl-plans/active/emaki-00-rules.md`
**Manifest**: `impl-plans/active/emaki-dispatch.json`

## Intent and Context

The user wants emaki plugins that embed video, animation and other content
in editor lines, plus character and line formats. This page shows:
- a `<video>` block widget, fed from a canvas `captureStream` so no binary
  asset is committed;
- an animated canvas waveform widget;
- an EOL badge;
- range marks including glow;
- line formats (bar, background, gutter marker).

A Playwright smoke script checks that the three demo pages load cleanly.

## Non-goals

- No engine change.
- No audio: the video stream has no audio track.

## Dependencies

- **dependsOn**: EMK-API.

## writePaths

Page and plugins:
- `emaki/examples/plugin-showcase/index.html`,
  `emaki/examples/plugin-showcase/main.ts`
- `emaki/examples/plugin-showcase/video-widget.ts`,
  `emaki/examples/plugin-showcase/wave-widget.ts`,
  `emaki/examples/plugin-showcase/format-plugin.ts`

Script:
- `emaki/scripts/smoke.mjs`

Test:
- `emaki/test/examples/showcase-plugins.test.ts`

Generated, plus this plan:
- `emaki/dist`, `emaki/node_modules` (artifactRoots)
- `impl-plans/active/emaki-42-showcase.md`

## sharedPaths

None. Read-only: `emaki/scripts/route-server.mjs`,
`emaki/examples/shared/demo.css`.

## Tasks

### P1: Demo plugins (each exports a factory returning `EmakiPlugin`)

**`videoWidgetPlugin({ pos, height = 180 })`**
- Block widget below the line of `pos`, `interactive: true`,
  `label: 'Demo video'`.
- `dom` content: creates a `<video muted playsinline autoplay loop>`. Its
  `srcObject` is `captureStream(30)` of an offscreen 320x180 canvas, animated
  by a `requestAnimationFrame` loop that draws a moving bar.
- The mount cleanup stops the tracks and the rAF loop.
- If `captureStream` is unavailable, fall back to showing the offscreen
  canvas itself and record the fallback in a `data-fallback` attribute.

**`waveWidgetPlugin({ pos, height = 64 })`**
- `canvas` content that draws a sine waveform whose phase advances with
  `frameMs`.
- `draw` returns `true` (animating only while visible: the WidgetLayer
  guarantees this).

**`formatPlugin()`**
- Static marks: a bold keyword, an underlined span, a struck span, a colored
  span.
- One animated mark with `glow` that moves across a line every 500 ms, via
  `animatedMarks`.
- Line formats on 3 lines: bar, background, `gutterMarker '*'`.
- An EOL badge widget (`canvas`, width 48) drawing a small flat label.

### P2: Page

- A short sample document (about 60 lines of code-like text in `main.ts`).
- `createEditor` with the three plugins.
- A side panel with a flat checkbox-like toggle per plugin
  (`appearance: none`, square) that calls the plugin remover or re-adds the
  plugin.

### P3: `scripts/smoke.mjs`

- Arguments: `--browser chromium|webkit|all` and `--out <dir>`. Exit 2 if
  `dist` is missing or a browser is unavailable.
- For each page in `index.html`, `plugin-showcase/`, `json-10k/?lines=1000`
  and `diff/?lines=400`, load it via `routeDist` and wait for one editor
  canvas (or, for index, the links).
- Fail on any `console.error` or `pageerror`.
- For the showcase, also assert:
  - one `.emaki-widget video` exists, or a `[data-fallback]` exists;
  - the wave canvas pixel content changes between two frames.

  Use `toDataURL` comparison, and evaluate it in the page.
- Write `result.json` in the same shape as `e2e.mjs`, and exit 0 only on all
  pass.

`json-10k` and `diff` pages may still be placeholders if their plans have
not finished. Smoke checks those two only when
`document.querySelector('canvas')` appears within 5 s; otherwise record
`skipped (placeholder)`. EMK-CLOSEOUT reruns smoke after all demos exist,
with no skips allowed.

### P4: `test/examples/showcase-plugins.test.ts` (jsdom, createEditor with recording GL)

- `formatPlugin` -> the next render input contains the bold, underline,
  strike and color marks, three line formats, and one animated glow mark;
- `waveWidgetPlugin` -> a block entry with height 64, and `draw` is called
  when visible;
- `videoWidgetPlugin` with a jsdom canvas lacking `captureStream` -> the
  fallback attribute is set; unmount stops the loop (spy on
  `cancelAnimationFrame`).

## Invariants

- The demo uses public APIs only (`../../src/index`).
- No network fetch and no committed media.

## Verification

Run in `emaki/`. Logs go to `tmp/emaki/EMK-SHOWCASE/`.

1. `npm run check`: passes per rule 12b.
2. `./node_modules/.bin/vitest run test/examples/showcase-plugins.test.ts`:
   exit 0. **This is the pass gate.**
3. `./node_modules/.bin/vitest run`: informational per rule 12b.
4. `node --check scripts/smoke.mjs`: exit 0.
5. **Smoke is produced at the wave-4 join** (rule 12a), once every demo
   page exists:
   `cd emaki && npm run build && node scripts/smoke.mjs --browser all --out ../tmp/emaki/wave-4-join/smoke`.
   It runs outside the sandbox and is appended to this log. Expect exit 0,
   with the showcase checks passing in both browsers. Because the join runs
   after all wave-4 demos are done, the `skipped (placeholder)` path must
   not occur there.
6. `wc -l` on every written file: each under 1000.

**mutationEvidence.** Make the wave `draw` ignore `frameMs` (it draws the
same frame each time). The P4 unit case must fail. Add that case: two
`draw` calls with different `frameMs` produce different recorded `lineTo`
coordinates on a recording 2D context. Restore afterwards. The smoke
pixel-change check runs at the wave-4 join.

## Completion Criteria

- [ ] P1-P4 delivered. Style follows rule 16.
- [ ] Verification steps pass. Mutation evidence is recorded. Hashes are
  logged.

## Progress Log

### Session: 2026-10-31 (plan authored, workflow session 586 step 4)
**Tasks Completed**: none.
