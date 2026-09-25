# Vactrol Editor: DAW-Style Parameter Editors, Sampler Waveform, Display-Only Grid and Roll (ED-PARAMS) Implementation Plan

**planId**: ED-PARAMS (issue #5, TASK-010, wave 4; parameter editors opened from `site.call` groups by
`manifest.editors` kind, handles writing through `BindApi.writeSite`, the sampler waveform editor, the step grid and
piano roll as displays with no write-back path)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 15.1.7, 15.1.2 G2/G3, 13.5 ("DAW-style parameter
editors", sampler requirements, "Sequences are code-only"), 15.1.12 (criteria 4 and 9);
architecture.md Editor Requirements (Decided); design-docs/specs/command.md "call", "editor-decl"
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-BIND (`BindApi.writeSite`/`learn`/`mode`, `SiteTable` via `siteById`), ED-VISUAL
(`VisualApi.mountSpectrum`)
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

Every builtin call site with numeric sites can open a meaning-matched editor (13.5):
- a call site is the group of sites sharing `site.call` `{name, head, ordinal}` within one top-level form;
- the editor kind comes from `manifest.editors[name].kind` (G2), with `scalar` for anything else;
- each handle is bound to the site whose `call.param` (else `call.arg`) matches a declared param;
- handles write ONLY through `BindApi.writeSite` and learn through `BindApi.learn`, so every handle behaves exactly like
  a slider (source-edit or overlay) and is MIDI-learnable.

Sequences stay code-only: the step grid and piano roll render `playing` telemetry and import no write path.

## Non-Goals

- No new write path, and no text insertion for missing sites (a disabled handle).
- No live gain-reduction metering (no GR cell; 15.1.1).
- No sequence editing of any kind: no step add, remove or reorder.
- The sampler waveform is not drawn on the native tier (frames are browser-only).

## writePaths

- `editor/src/params/mount.ts` (fills the ED-SCAFFOLD stub), `editor/src/params/open.ts`,
  `editor/src/params/handles.ts`, `editor/src/params/curves.ts`, `editor/src/params/eq.ts`,
  `editor/src/params/filter.ts`, `editor/src/params/dynamics.ts`, `editor/src/params/envelope.ts`,
  `editor/src/params/delay.ts`, `editor/src/params/reverb.ts`, `editor/src/params/sampler.ts`,
  `editor/src/params/wavetable.ts`, `editor/src/params/granular.ts`, `editor/src/params/lfo.ts`,
  `editor/src/params/stereo.ts`, `editor/src/params/xy.ts`, `editor/src/params/euclid.ts`,
  `editor/src/params/probability.ts`, `editor/src/params/length.ts`, `editor/src/params/scalar.ts`,
  `editor/src/params/grid.ts`, `editor/src/params/roll.ts`, `editor/src/params/params.css`
- `editor/test/params/open.test.ts`, `editor/test/params/eq.test.ts`, `editor/test/params/envelope.test.ts`,
  `editor/test/params/euclid.test.ts`, `editor/test/params/handles.test.ts`, `editor/test/params/kinds.test.ts`,
  `editor/test/params/sampler.test.ts`, `editor/test/params/displays.test.ts`
- `impl-plans/active/vactrol-editor-params.md`

## sharedPaths

None.

## File-Level Changes (behavior and signatures; no code)

1. **`open.ts`.**
   - `callGroups(sites) -> CallGroup[]` groups by `(form, call.name, call.head, call.ordinal)`.
   - `editorFor(group, manifest) -> {kind, decl?}`.
   - The panel (ED-BIND rows) gets an "open editor" affordance via `mount.ts`, added as a click on the call head
     decoration in the code view and a button per call group in the params pane. ED-BIND files are not edited.
   - `bindHandles(group, decl) -> Map<paramName, siteId | null>`: `call.param` match first, else `call.arg` index into
     `decl.params`.
2. **`handles.ts`.** `Handle {param, siteId | null, range, curve, unit}`:
   - `drag(delta)` maps through `curves.ts` (linear/log/stepped) and calls `deps.bind.writeSite(siteId, value)`;
   - `learn()` calls `deps.bind.learn(siteId)`;
   - a null site renders disabled with a tooltip ("not in code").
3. **Kind components** (each `render(el, group, handles, deps) -> {update(), dispose()}`, drawn on canvas 2D and
   repainting on store `site:<id>` updates):
   - `eq.ts` (`eq-curve`): band nodes (freq x gain, q on wheel) with the summed curve, and the live spectrum behind it
     via `deps.visual.mountSpectrum(el, {bus})`. The bus is taken from a `bus` sibling site in the chain when present,
     else master.
   - `filter.ts` (`filter-response`): cutoff/res handles on the response curve.
   - `dynamics.ts` (`dynamics-transfer`, with multiband crossover handles when `multiband`): the transfer curve,
     threshold/ratio/knee handles, and the input level from a bus `level` analyzer when published. There is no GR
     meter.
   - `envelope.ts` (`envelope-shape`): stage handles for attack/decay/sustain/release (ADSR, perc, line).
   - `delay.ts` (`delay-taps`): beat-aligned taps (time snaps to a beat fraction from `tempo`), plus feedback.
   - `reverb.ts` (`reverb-room`), `stereo.ts` (`stereo-field`), `lfo.ts` (`lfo-shape`: the shape drawn from the call
     name, handles for the call's numeric sites and an adjacent `range` call's sites in the same chain),
     `wavetable.ts` (`wavetable-frames`), `granular.ts` (`granular-region`: a region over the sample frames when
     available), `probability.ts` (`probability-dial`), `length.ts` (`length-handle`), `euclid.ts` (`euclid-ring`:
     hits, steps and rotation handles on a ring), `xy.ts` (`xy-pad`: the user picks any two numeric sites of the
     group, or any two sites via the panel), `scalar.ts` (sliders).
4. **`sampler.ts`** (`sampler-wave`; 13.5 revised sampler requirements).
   - The waveform comes from `deps.code.samples.frames(bank, n)`, where `bank`/`n` are the values of the chain's
     `s`/`bank` and `n` sites. A missing value gives "no preview".
   - Start/end/loop handles are the `begin`/`end`/`loop` sites (`call.name`) in the same top-level form.
   - `slice n`, `chop n` and `striate n` counts draw as grid overlays.
   - MANUAL slice markers are the numeric sites of a `slice` point-list argument. Dragging a marker calls
     `writeSite`.
   - Clicking slice k: when `deps.code.selectedSiteId()` is an EXISTING numeric site of the index pattern (a site
     whose `call.name` is `n`, or the `slice` index argument), call `writeSite(selected, k)`. Otherwise do nothing and
     show the hint "select an index literal". Clicking never adds, removes or reorders steps.
   - The `n` site's "browse" opens `deps.code.samples.openBrowser(bank)`.
5. **`grid.ts` and `roll.ts`** (DISPLAYS).
   - Rendered from `playing` events per slot within the current cycle (beat / beats per cycle).
   - The roll's pitch comes from the source text at the event's mapped `src` span (`deps.code.mapWireSpan` plus the
     document text), read for display only: a note keyword (`:a4`, `:c#3`) or a number (MIDI note). Any other text
     goes to an unpitched lane.
   - These two modules import NOTHING from `bind/`, `protocol/client.ts` or `params/handles.ts`, register no pointer or
     key handler that writes, and expose no callback that produces a message or a text change.
6. **`mount.ts`.** Renders the params pane (the call-group list, the open editor, grid and roll tabs) and subscribes
   to the store (`sites`, `playing` via the client, `levels`, `tempo`).

## Required Tests

All use `RecordingTransport`, scripted `eval-result`s whose sites carry `call` and a `manifest` with `editors` (a
fixture copied from the ED-WIRE editor table shape), and a real `BindApi` from ED-BIND mounted in jsdom.
- `open.test.ts` (criterion 4):
  - a `peq` call site opens `eq-curve`, `env-adsr` opens `envelope-shape`, `euclid` opens `euclid-ring`, and an
    unknown name opens `scalar`;
  - handle binding by `call.param` and by `call.arg`.
- `handles.test.ts` (criterion 4):
  - for `peq`, `env-adsr` and `euclid`, a dragged handle records the IDENTICAL `set-tweak` (id, `form_gen`, value) as
    the equivalent slider move through ED-BIND's panel;
  - in source-edit mode it records the identical verified text edit plus form `eval`;
  - `learn()` on a handle sends the same `learn` as the slider's learn.
- `eq.test.ts`: `mountSpectrum` is called with the chain's bus (a fake `VisualApi`).
- `envelope.test.ts`, `euclid.test.ts`: the handles move the right sites; the euclid rotation handle wraps modulo
  steps.
- `kinds.test.ts`: every `EditorKind` in the ED-WIRE list renders without error for a minimal group; a missing site
  renders a disabled handle and sends nothing.
- `sampler.test.ts` (criterion 9):
  - the begin/end handles write their sites;
  - the `slice 8` / `chop 4` grid overlays;
  - a manual marker drag writes its point literal (a recording-transport assertion, in both modes);
  - clicking slice 3 with a selected index literal writes 3 into THAT literal only;
  - clicking with no selection sends nothing and shows the hint;
  - the `n` site opens the sample browser;
  - structural: the module exposes no function that adds, removes or reorders steps (its exported API is asserted).
- `displays.test.ts` (criterion 4 structural):
  - a static import scan (read the source text of `grid.ts` and `roll.ts` in the test) finds no import of `bind/`,
    `protocol/client`, `params/handles` or `platform/files`;
  - rendering scripted `playing` events draws them;
  - firing pointer, keyboard and wheel events on both components leaves `RecordingTransport` empty and the document
    text unchanged;
  - roll pitch lanes for `:a4`, `60` and an unpitched literal.

## Invariants

- Handles call only `BindApi`. There is no direct client or document access in `params/` except the read-only
  `mapWireSpan` and document text read in `roll.ts`.
- `grid.ts` and `roll.ts` have no write-back path (structurally asserted).
- Every TS file stays under 800 lines.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-PARAMS`.

## Verification (`<wave>` = `params`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4 and E0-E5, plus:

| # | Command | Evidence |
|---|---------|----------|
| P1 | LOG(`own`): `cd editor && npx vitest run test/params` | `exit=0`, 8 files passed |
| P2 | `grep -nE "from ['\"][./]*(bind\|protocol/client\|params/handles\|platform/files)" editor/src/params/grid.ts editor/src/params/roll.ts \|\| echo none` | prints `none` |

## Completion Criteria

- [ ] Items 1-6 implemented
- [ ] Required tests pass (criteria 4 and 9)
- [ ] Common rows and P1-P2 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-PARAMS implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-bind.md, vactrol-editor-visual.md. **Parallel**: vactrol-editor-pkg.md,
  vactrol-editor-tauri.md
- **Next**: vactrol-editor-finalize.md
