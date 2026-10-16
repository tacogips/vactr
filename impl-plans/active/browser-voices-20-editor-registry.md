# BV-20: Register the Nine Template-Voice Examples in the Picker

**Status**: Ready
**Plan ID**: BV-20 (wave 1, parallel with BV-10)
**Design Reference**: `design-docs/specs/design-browser-tier-voices.md`
("Editor registry", decision BTV-D7);
`design-docs/specs/design-example-picker-and-persistence.md` sections 3.2-3.3
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Intent and Context

`editor/src/examples/manifest.ts` `EXCLUDED` (lines 63-79) hides nine
examples because of the checker bug that BV-10 fixes. This plan removes
those nine rows and registers the examples in `CURATED`, consistent with
the existing rows. It also updates the registry unit test mechanically.
The wasm example-evaluation test (`editor/test/wasm/examples.test.ts`)
iterates `listExamples()`, so it covers the new entries automatically. It
can only pass after BV-10, so BV-30 runs it.

## Non-goals

- Do not edit `editor/test/wasm/examples.test.ts`,
  `editor/src/examples/registry.ts`, any `examples/*.vact` file, or any UI,
  CSS or `editor/src/code/*` file.
- Do not touch the six remaining `EXCLUDED` rows or their reasons:
  `sample-timestamps.vact`, `tracks/breakcore/glass-teeth.vact`,
  `quad-stems.vact`, `song-mode/generated-parts.vact`,
  `song-mode/requirement-song.vact`, `song-mode.vact`.
- No new category and no `requires`.
- No Rust changes.

## Dependencies

- **dependsOn**: none
- **Blocks**: BV-30

## writePaths

- `editor/src/examples/manifest.ts`
- `editor/test/examples/registry.test.ts`
- `impl-plans/active/browser-voices-20-editor-registry.md` (this plan's Progress Log only)
- `editor/node_modules` (artifact root)
- `tmp/browser-voices/BV-20` (artifact root; evidence logs)

## sharedPaths

- none

## File-level changes

1. `editor/src/examples/manifest.ts`
   - Delete the nine `EXCLUDED` rows for `acid-techno.vact`,
     `bass-presets.vact`, `experimental-oscillators.vact`,
     `hurdy-gurdy.vact`, `kalimba.vact`, `offbeat-bass-techno.vact`,
     `rumble-techno.vact`, `tonewheel-organ.vact` and `wobble-techno.vact`.
     Six rows remain, in their current order.
   - Add nine `CURATED` entries with `category: 'Examples'`, and no
     `requires` key (imitate the `lofi-lab` row). Insert each one so the `'Examples'` group stays in
     alphabetical id order: `acid-techno` and `bass-presets` before
     `canon-in-d`; `experimental-oscillators` after `digital-kit` and
     before `filter-lab`; `hurdy-gurdy` after `fm6-algorithms`;
     `kalimba` after `industrial-techno`; `offbeat-bass-techno` after
     `ode-to-joy`; `rumble-techno` after `rumble-kick`;
     `tonewheel-organ` after `techno-pattern`; `wobble-techno` after
     `voice-layer`. `showcase` stays first.
   - `title`: the title-cased id, as the existing rows do: `Acid Techno`,
     `Bass Presets`, `Experimental Oscillators`, `Hurdy Gurdy`, `Kalimba`,
     `Offbeat Bass Techno`, `Rumble Techno`, `Tonewheel Organ`,
     `Wobble Techno`.
   - `description`: the example's leading `#` comment text, using the same
     rule as the existing rows:
     - strip `# `;
     - join consecutive leading comment lines with one space;
     - skip lines that are only `#`;
     - cut to at most 140 characters, the same way existing rows are cut
       (mid-word cut allowed, no ellipsis).

     Read each file's header with `sed -n 1,8p examples/<id>.vact`. The
     first lines are, for example: acid-techno "Acid techno at 132 BPM:
     accented and sliding diode-ladder bass."; kalimba "A bright tine
     melody with beating pairs, a subtle bottle-cap buzz, and a hardness
     contrast.".
   - Escape quotes the way existing rows do (single quotes, or double
     quotes when the text has an apostrophe).
2. `editor/test/examples/registry.test.ts`
   - `expect(EXCLUDED.length).toBe(15)` becomes `toBe(6)`.
   - `loadExample('acid-techno')` rejects becomes
     `loadExample('song-mode')` rejects with `'unknown example song-mode'`.
   - Add one `it` asserting that each of the nine ids is registered through
     `findExample(id)` with `category: 'Examples'` and `requires: []`, and
     is absent from `excludedIds()`.

## Pitfalls

- An id is the path without `.vact`. All nine are top-level files, so
  there is no directory prefix.
- Description and title bounds (1..140 and 1..60) are already asserted.
  Count characters, not bytes.
- The partition test requires every glob file to be either registered or
  excluded. Removing a row without adding the `CURATED` entry fails that
  test.

## Edit protocol

Before each edit, re-read the file and record `shasum -a 256` in the
Progress Log; record it again after. On drift, stop, record, re-read and
re-apply only this plan's change. Do not commit or change git state.

## Verification (logs in `tmp/browser-voices/BV-20/<name>.log`, each ending with its exit status)

- `check.log`: `cd editor && npm run check` -> exit 0.
- `registry.log`: `cd editor && npx vitest run test/examples/registry.test.ts`
  -> exit 0, all tests passed (count recorded).
- The 6-row `EXCLUDED` count is proven by the `toBe(6)` assertion in
  `registry.log`.
- Not run here: `test/wasm/examples.test.ts`. BV-30 runs it after BV-10's
  wasm rebuild, under the lock.

## Completion Criteria

- [ ] `EXCLUDED` has exactly the six sample, native-only and song-mode rows
- [ ] Nine `CURATED` `'Examples'` entries with title-cased titles and header-derived descriptions of at most 140 characters
- [ ] registry.test.ts updated (6, song-mode probe, nine-id assertion) and passing
- [ ] `npm run check` passes
- [ ] Progress Log updated with hashes and results

## Progress Log

### Session: 2026-10-17 (plan authored, session 386)
**Tasks Completed**: none
**Tasks In Progress**: none
**Blockers**: none
**Notes**: Plan created from the accepted design.
