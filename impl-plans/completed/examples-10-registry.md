# EX-10: Example Registry, Manifest, Showcase and Wasm Evaluation Test

**Status**: Completed (session 382; IR-EX10-VITEST-FS repaired; Step 7 test-integrity and adversarial review accepted; archived 2026-10-17)
**Plan ID**: EX-10 (wave 1, parallel)
**Design Reference**: `design-docs/specs/design-example-picker-and-persistence.md` sections 3.1-3.4, 9.2
**Created**: 2026-10-16
**Last Updated**: 2026-10-17

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
- `loadExample('first-track')` resolves to text containing `use-bpm 124`; excluded `acid-techno`, `sample-timestamps` and unknown `nope` reject.
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

Evidence in `tmp/examples/EX-10/`, every log ending with its exit status. Vitest's config allows `../examples` so the prescribed commands run from `editor/` without a `--root` override.

1. `cd editor && npx vitest run test/examples/registry.test.ts > ../tmp/examples/EX-10/registry-review-final.log 2>&1` -> all pass.
2. Under the measurement lock (`until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done`,
   write an `owner` file `EX-10`, run, then remove ONLY the lock you
   created):
   `cd editor && npx vitest run --root .. editor/test/wasm/examples.test.ts > ../tmp/examples/EX-10/wasm-examples-final3.log 2>&1`
   -> all pass; testsRun equals registered count + showcase test.
3. `cd editor && npm run check > ../tmp/examples/EX-10/check-final2.log 2>&1` -> exit 0. If errors appear only in files owned by another
   wave-1 plan, record them and re-run after that plan lands; files in this
   plan's writePaths must have zero errors.

Record each as `{command, exitStatus: 0, testsRun, testsPassed,
failureCount: 0, outcome: "passed", log, notes}`; notes list every
exclusion added. Negative controls (for example temporarily raising a
showcase gain to 0.5 to see the gain test fail) go only in
`mutationEvidence` and must be reverted.

## Completion Criteria

- [x] `examples/showcase.vact` meets every bound and evaluates cleanly.
- [x] Registry exports and manifest exist; coverage test passes.
- [x] `test/wasm/examples.test.ts` passes for every registered example.
- [x] `vite.config.ts` has `server.fs.allow` and nothing else changed.
- [x] Progress Log: verification records, exclusions with reasons,
      pre/post sha256 of each written file.

## Edit Protocol

Read each file fresh before editing; record `shasum -a 256` before and
after each write. Edit only writePaths. On unexpected drift, stop, record,
re-read. No git index or branch operations.

## Progress Log

### Session: 2026-10-16
**Tasks Completed**: Plan authored.
**Blockers**: None.


### Session: 2026-10-17
**Tasks Completed**: EX-10 registry, manifest, showcase bound assertions, browser-tier exclusion list, registry coverage test, wasm evaluation test and additive Vite dev-server allowlist.
**Blockers**: None for EX-10 implementation. Formal integration/adversarial review is downstream.

**Verification** (final source; logs under `tmp/examples/EX-10/`):

- `{command: "cd editor && npx vitest run --root .. editor/test/examples/registry.test.ts > ../tmp/examples/EX-10/registry-root-final3.log 2>&1", exitStatus: 0, testsRun: 5, testsPassed: 5, failureCount: 0, outcome: "passed", log: "tmp/examples/EX-10/registry-root-final3.log", notes: "Root-scoped Vitest run lets Vite include the repository examples directory; 15 exclusions are disjoint from the curated and generated registry."}`
- `{command: "lock=/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock; until mkdir \"$lock\" 2>/dev/null; do sleep 30; done; trap 'unlink \"$lock/owner\"; rmdir \"$lock\"' EXIT; printf '%s\n' EX-10 > \"$lock/owner\"; cd editor && npx vitest run --root .. editor/test/wasm/examples.test.ts > ../tmp/examples/EX-10/wasm-examples-final3.log 2>&1", exitStatus: 0, testsRun: 75, testsPassed: 75, failureCount: 0, outcome: "passed", log: "tmp/examples/EX-10/wasm-examples-final3.log", notes: "Ran under the measurement lock; each of 74 registered examples used a fresh wasm session, plus the showcase activity/bounds test. Twelve failing examples were excluded with first diagnostic reasons; showcase passed."}`
- `{command: "cd editor && npm run check > ../tmp/examples/EX-10/check-final2.log 2>&1", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/examples/EX-10/check-final2.log", notes: "TypeScript check passed on the final shared tree."}`

**Exclusions added after wasm evaluation**:
`acid-techno.vact` (`:phase-drum` unavailable), `bass-presets.vact` (`:analog-bass` unavailable), `experimental-oscillators.vact` (`:vosim` unavailable), `hurdy-gurdy.vact` (`:hurdy-gurdy` unavailable), `kalimba.vact` (`:kalimba` unavailable), `offbeat-bass-techno.vact` (`:phase-drum` unavailable), `rumble-techno.vact` (`:phase-drum` unavailable), `tonewheel-organ.vact` (`:tonewheel-organ` unavailable), `wobble-techno.vact` (`:phase-drum` unavailable), and the three `song-mode` files (finite song playback unavailable until song transport is installed). Full first diagnostic messages are stored in `editor/src/examples/manifest.ts`.

**Prior attempts**: the prescribed Vitest command without `--root ..` failed before collection with Vite `Denied ID` for `examples/tracks/catalog.json?raw`; `--config vite.config.ts` could not start because the release wasm artifact is absent. Initial wasm evaluation had 14 failures (12 examples plus the first unsupported showcase); after replacing showcase voices, 12 examples remained; after moving them out of CURATED, the final wasm suite passed 75/75. Initial `npm run check` encountered concurrent EX-30/EX-50 TypeScript errors; the final run passed.

**SHA-256 evidence**: pre-dispatch snapshot showed all EX-10 sources absent and `editor/vite.config.ts` at `0721bb9a03be3ac2209505f8bf5b56b7631ca5418fbf38bf025d7712bc2b45ab`; active plan pre-edit hash was `143547226bb492e13da802569b4e75121ae0728707bd80ce618761b56df6e000`. Final source hashes are in `tmp/examples/EX-10/source-sha256.txt`; the plan's pre-update hashes are in `tmp/examples/EX-10/pre-plan-update-sha256-final.txt` and post-update hash is in `tmp/examples/EX-10/plan-sha256.txt`.

### Session: 2026-10-17 (IR-EX10-VITEST-FS repair)
**Tasks Completed**: Added the one authorized mechanical test-infrastructure setting `server.fs.allow: ['.', '../examples']` to `editor/vitest.config.ts`, matching the existing Vite dev-server allowlist. No discovery, environment, plugin, or performance settings changed. The prescribed registry command now collects and passes tests without `--root`.
**Blockers**: None for EX-10 implementation. Formal independent re-review and downstream integration gates remain owned by later workflow steps.

**Verification** (review repair source; complete logs in `tmp/examples/EX-10/`):

- `{command: "cd editor && npx vitest run test/examples/registry.test.ts > ../tmp/examples/EX-10/registry-review-final.log 2>&1", exitStatus: 0, testsRun: 5, testsPassed: 5, failureCount: 0, outcome: "passed", log: "tmp/examples/EX-10/registry-review-final.log", notes: "Default editor Vitest config; passed without --root. The prior Denied ID was resolved."}`
- `{command: "lock=/Users/taco/gits/tacogips/vactr-worktrees/.measure-lock; until mkdir \"$lock\" 2>/dev/null; do sleep 30; done; trap 'unlink \"$lock/owner\"; rmdir \"$lock\"' EXIT; printf '%s\\n' EX-10 > \"$lock/owner\"; cd editor && npx vitest run test/wasm/examples.test.ts > ../tmp/examples/EX-10/wasm-examples-review-final.log 2>&1", exitStatus: 0, testsRun: 75, testsPassed: 75, failureCount: 0, outcome: "passed", log: "tmp/examples/EX-10/wasm-examples-review-final.log", notes: "Measurement lock acquired by EX-10; 74 registered examples plus showcase bounds/activity passed."}`
- `{command: "cd editor && npm run check > ../tmp/examples/EX-10/check-review-final.log 2>&1", exitStatus: 0, testsRun: 0, testsPassed: 0, failureCount: 0, outcome: "passed", log: "tmp/examples/EX-10/check-review-final.log", notes: "Whole editor TypeScript check on the current shared tree."}`
- `{command: "cd editor && npx vitest run test/documents test/ui/picker-view.test.tsx test/ui/style-tokens.test.ts > ../tmp/examples/EX-10/regression-review-final.log 2>&1", exitStatus: 0, testsRun: 62, testsPassed: 62, failureCount: 0, outcome: "passed", log: "tmp/examples/EX-10/regression-review-final.log", notes: "EX-20/30/40/50 regression assertion set passed."}`

**Prior attempt**: The initial post-repair shell wrapper used zsh's read-only special variable `status`; it stopped while appending the exit marker after Vitest. It was superseded by the complete rerun above using `rc`; the complete registry log ends in `EXIT_STATUS=0`.

**SHA-256 evidence**: Before the repair, `editor/vitest.config.ts` was `407cea91fad831247432f9a0132d65ec8de3ee15d44b4e7ca0624f6babca07ec`. After the repair it is `334049728fae21f6d0f802eead36f92d2191ee96a9eb1799bc3117e7add8ef08`. The EX-10 registry, manifest, showcase, tests and Vite config retained their prior source hashes recorded above. This progress-log update started from plan hash `879ac8ae342a3bba79ddd1e880a3ac835635d88ecce01a638d9e910f932e9458`; updated plan hash is recorded in `tmp/examples/EX-10/plan-sha256-review-final.txt`.
