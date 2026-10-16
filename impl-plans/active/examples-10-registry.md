# EX-10: Example Registry, Manifest, Showcase and Wasm Evaluation Test

**Status**: Ready
**Plan ID**: EX-10 (wave 1, parallel)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 3.1-3.4, 9.2
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Intent and Context

The user wants vactr to open a flashy example and to list examples in a
picker. This plan builds the build-time registry of repository examples
(`examples/**/*.vact`), writes the new built-in default
`examples/showcase.vact`, and adds the anti-rot test that evaluates every
registered example in the real wasm browser-tier session.

Context:
- Examples live at the repo root `examples/` (top level, `synth-labs/`,
  `effect-labs/`, `song-mode/`, `tracks/<genre>/`). Tracks metadata is in
  `examples/tracks/catalog.json` (`g`, `slug`, `title`, `bpm`, `key`, `desc`).
- No existing example has a visual chain; visual chains (`osc ... > out o0`)
  evaluate in wasm (`editor/test/wasm/abi.test.ts` ~line 231).
- The debug wasm cdylib is prebuilt at
  `target/wasm32-unknown-unknown/debug/vactr.wasm`; `editor/test/support/wasm.ts:loadVactrWasm`
  loads it (cwd is `editor/`).

## Non-goals

- Do NOT edit any existing file under `examples/`. If an example fails the
  evaluation test, EXCLUDE it with the recorded failure message as the
  reason. Never change the evaluator or `src/`.
- No picker UI, storage or startup code.
- Do not edit `editor/test/wasm/first-track.test.ts` or
  `editor/test/support/wasm.ts` (copy the rig helpers locally instead).
- Do not change `editor/tsconfig.json`.
- No native-tier-only or sample-dependent entries in the registry.

## Dependencies

- **dependsOn**: EX-00 (`ExampleEntry`, `ExampleSource`, `Requirement`,
  `EXAMPLE_ID_PATTERN` in `editor/src/documents/types.ts`; `glob.d.ts`)
- **Blocks**: EX-60, EX-70

## writePaths

- `editor/src/examples/registry.ts`
- `editor/src/examples/manifest.ts`
- `examples/showcase.vact`
- `editor/vite.config.ts`
- `editor/test/examples/registry.test.ts`
- `editor/test/wasm/examples.test.ts`
- `impl-plans/active/examples-10-registry.md`
- `target` (artifact root)
- `tree-sitter-vact/tree-sitter-vact.wasm` (artifact root)
- `editor/node_modules` (artifact root)
- `tmp/examples/EX-10` (artifact root; evidence logs)

## sharedPaths

- `editor/vite.config.ts` (single additive key; no other plan edits it)

## Patterns to Imitate

- Wasm session rig: `editor/test/wasm/first-track.test.ts` (`start`,
  `drain`, `ackControls`, `send`, `subscribe`, `evalDoc`, `tick`, `tickTo`,
  and the `readText` dynamic `node:fs` import). Copy these helpers into
  `examples.test.ts`.
- Render records: `editor/test/wasm/abi.test.ts` ~231-240 (`jsonOf<RenderRecord>(records, TAG_RENDER)`,
  `op === 'program'` arrives on a later `tick`, not in the eval reply).
- Example voice lines: `examples/acid-techno.vact`, `examples/wobble-techno.vact`,
  `examples/fm6-algorithms.vact`; visual vocabulary:
  `design-docs/specs/design-visual.md` section 1 (sources, geometry, color,
  modulate, `out o0`).

## File-level Changes

### `editor/vite.config.ts`

Add `server: { fs: { allow: ['.', '../examples'] } }` to the
`defineConfig` object. Nothing else changes.

### `editor/src/examples/manifest.ts`

- `export const DEFAULT_EXAMPLE_ID = 'showcase';`
- `export const CURATED: readonly ManifestEntry[]` with
  `ManifestEntry = { id: string; title: string; description: string; category: string; requires?: readonly Requirement[] }`
  for every registered NON-track file, in display order:
  `Showcase` (only `showcase`), then `Examples` (top-level files, alphabetical),
  `Synth labs`, `Effect labs`, `Song mode` (`song-mode` top-level file and
  `song-mode/*`; put `song-mode.vact` under `Song mode`).
- `title`: 1-60 chars (human title, e.g. `Acid techno`). `description`:
  1-140 chars, paraphrased from the file's first header comment lines; files
  with no header comment (for example `song-mode/requirement-song.vact`) get
  a short factual description.
- `live-external-voices` has `requires: ['audio-input']`.
- `export const EXCLUDED: readonly { path: string; reason: string }[]`,
  `path` relative to `examples/` with `.vact`. Start with:
  - `sample-timestamps.vact`: "needs ./samples/amen-recreation/source.wav, which is not bundled"
  - `tracks/breakcore/glass-teeth.vact`: "needs the examples/samples/amen-recreation WAV, which is not bundled"
  - `quad-stems.vact`: "needs a four-output native host"
  - plus one entry per example that fails `examples.test.ts`, with reason
    `"browser-tier evaluation fails: <first error message>"`.
- `export const TRACK_CATEGORY_PREFIX = 'Tracks: ';`

### `editor/src/examples/registry.ts`

- `const SOURCES = import.meta.glob<string>('../../../examples/**/*.vact', { query: '?raw', import: 'default' })`
  (lazy). Key -> id: strip `../../../examples/` and `.vact`.
- `const CATALOG = import.meta.glob<string>('../../../examples/tracks/catalog.json', { query: '?raw', import: 'default', eager: true })`;
  parse once with `JSON.parse`. For each catalog row not in `EXCLUDED`:
  `id = tracks/<g>/<slug>`, `title`, `description = "<desc> <bpm> BPM."`
  (truncate to 140 chars), `category = 'Tracks: ' + g`, `requires = []`.
  Track categories follow catalog order after the curated categories.
- Exports: `listExamples(): readonly ExampleEntry[]`, `findExample(id)`,
  `loadExample(id): Promise<string>`, `exampleSource: ExampleSource`
  (`defaultId = DEFAULT_EXAMPLE_ID`), `REGISTRY_FILES` (the glob ids, for
  tests), `excludedIds()`.
- `loadExample` rejects with `Error('unknown example <id>')` for ids not in
  the registered list (even if a glob loader exists for an excluded file).
- Build the entry list once at module load; freeze it.

### `examples/showcase.vact`

New, hand-written, MIT, project-owned. Bounds (mechanically tested):
- First lines: comment naming it the vactr showcase and
  `# Press Run (Mod-Shift-Enter) to play.`
- `use-bpm` between 126 and 136.
- 5-8 sound slots (`d1`..`d8`) with built-in sample-free voices proven in
  the examples above (`:phase-drum`, `:dual-hat-voice`, `:acid-bass` with
  `accent`/`slide-from`, `:wobble-bass`, `:fm` with `algorithm` patterns).
- Every `gain <number>` at most 0.32; sum of all `gain` values at most 1.2.
- One visual chain ending in `> out o0`, built from design-visual.md
  section 1 sources and transforms and driven by time (for example a
  `sine` or `time` parameter). Start from `osc 20 > rotate 0.5 > out o0`
  (known to evaluate) and add transforms one at a time, keeping only ones
  that evaluate without error.
- At most 40 lines. No `sample`, no host input, no MIDI.

### `editor/test/examples/registry.test.ts` (jsdom)

Tests (input -> expected):
- every `CURATED` id and every generated track id has a loader in the glob -> true.
- the set of glob ids == registered ids UNION excluded paths (minus
  `.vact`), with no id in both -> equal sets, empty intersection.
- ids unique; every id matches `EXAMPLE_ID_PATTERN`; titles 1-60,
  descriptions 1-140 chars.
- `findExample('showcase')` exists, category `Showcase`, `requires` empty.
- `findExample('live-external-voices')?.requires` equals `['audio-input']`
  (unless excluded with a recorded reason; then assert it is in `EXCLUDED`).
- `loadExample('acid-techno')` resolves to text containing `use-bpm 132`.
- `loadExample('sample-timestamps')` and `loadExample('nope')` reject.
- 28 catalog rows -> 27 track entries (glass-teeth excluded), minus any
  recorded track exclusions.

### `editor/test/wasm/examples.test.ts` (`// @vitest-environment node`)

- For each `listExamples()` entry: read `../examples/<id>.vact` from disk,
  start a FRESH wasm session (`loadVactrWasm`, `session_init(48000, 0)`),
  subscribe, eval the whole text as file `main.vact`, tick 1 s while
  acknowledging worklet-bound records. Fail with the example id in the
  message on any `severity === 'error'` diagnostic or any form with
  `failure`. Do not keep references to earlier instances.
- Showcase test: tick 4 s; assert last `tempo.bpm` in 126..136, `playing`
  events on at least 4 distinct slots, at least one `TAG_RENDER` record
  with `op === 'program'`; parse the showcase text with
  `/gain\s+([0-9.]+)/g`: each value <= 0.32 and the sum <= 1.2.
- Per-test timeout 60 s. If total wall time exceeds 10 min or memory
  fails, switch to one instance with `hush` between examples and record
  the change and reason in the Progress Log.

## Invariants

- Registered == evaluates without error in the browser tier.
- `EXCLUDED` reasons are concrete (file need or first error message).
- Main bundle does not inline example text (lazy glob); only
  `catalog.json` is eager.
- Tests are silent (no AudioContext; wasm session only).

## Pitfalls

- Glob keys are relative to `registry.ts`; `../../../examples/` is the
  repo root `examples/` from `editor/src/examples/`. Verify with the
  coverage test, not by eye.
- `import: 'default'` with `query: '?raw'` yields the string directly; do
  not use the deprecated `as: 'raw'`.
- Render `program` records come on ticks after eval.
- A failing example is excluded, never "fixed" by editing it; the only new
  example file is `showcase.vact`.
- Keep `manifest.ts` and `registry.ts` each under 1000 lines.

## Verification

Evidence in `tmp/examples/EX-10/`, every log ending with its exit status.

1. `cd editor && npx vitest run test/examples/registry.test.ts > ../tmp/examples/EX-10/registry.log 2>&1` -> all pass.
2. Under the measurement lock (`until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done`,
   write an `owner` file `EX-10`, run, then remove ONLY the lock you
   created):
   `cd editor && npx vitest run test/wasm/examples.test.ts > ../tmp/examples/EX-10/wasm-examples.log 2>&1`
   -> all pass; testsRun equals registered count + showcase test.
3. `cd editor && npm run check > ../tmp/examples/EX-10/check.log 2>&1` -> exit 0. If errors appear only in files owned by another
   wave-1 plan, record them and re-run after that plan lands; files in this
   plan's writePaths must have zero errors.

Record each as `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`; notes list every
exclusion added. Negative controls (for example temporarily raising a
showcase gain to 0.5 to see the gain test fail) go only in
`mutationEvidence` and must be reverted.

## Completion Criteria

- [ ] `examples/showcase.vact` meets every bound and evaluates cleanly.
- [ ] Registry exports and manifest exist; coverage test passes.
- [ ] `test/wasm/examples.test.ts` passes for every registered example.
- [ ] `vite.config.ts` has `server.fs.allow` and nothing else changed.
- [ ] Progress Log: verification records, exclusions with reasons,
      pre/post sha256 of each written file.

## Edit Protocol

Read each file fresh before editing; record `shasum -a 256` before and
after each write. Edit only writePaths. On unexpected drift, stop, record,
re-read. No git index or branch operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored.
**Blockers**: None.
