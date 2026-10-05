# Canvas browser evidence harness

This is a plain Node.js Playwright-library harness. It does not use `@playwright/test`.

```sh
cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build
cd editor && npm run e2e -- --browser all --profile all --write-evidence
cd editor && ./node_modules/.bin/vitest run test/e2e
node editor/test/e2e/ios-sim.mjs --app <path-to-ios-app> --device "iPad Pro 11-inch (M5)"
```

`--browser` accepts `chromium`, `webkit`, or `all`; `--profile` accepts `behavior`, `measure`, or `all`. `--run-id` selects the evidence run directory (default `run-001`). Chromium starts with `--mute-audio` and hardware-GL flags. Before each page is created, the harness routes realtime audio through an analyser tap, a zero-gain node, and a post-sink analyser. Reports include pre-sink peak and RMS dBFS, onset counts/times, direct connection count, and post-sink peak; a run passes the audio gate only when post-sink peak is exactly zero and direct connections are zero. Automated runs never change system volume or use a system audio driver. `--headed-webkit` retries WebKit with a visible window. Exit 0 means every gated check passed, 1 means a check/threshold failed, and 2 means the run was blocked. A blocked result is never a pass.

`--write-evidence` writes environment, browser behavior, raw measurement JSONL, summary and harness-generated result rows beneath `design-docs/specs/evidence/canvas-cutover/run-001/`. Full stdout/stderr logs belong beneath `tmp/canvas-cutover/evidence/`. The iPad simulator record includes its self-check line and screenshot when available; it records an exact blocker when the app or simulator is unavailable.
