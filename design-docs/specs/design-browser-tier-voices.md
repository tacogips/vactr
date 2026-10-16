# Template Voices as Sounds in Every Tier Design

Status: accepted design for the wf/browser-voices workflow (2026-10-17,
session 386). It is traced against the code at `c15799a`. No user decision
is pending.

## Problem

The editor's example registry excludes nine repository examples because the
browser-tier evaluation reports `` `:<name>` is not a sound in the default
sound kit `` (`DiagCode::UnknownKeyword`, error severity):

| Example | Rejected keyword |
|---------|------------------|
| `acid-techno.vact`, `offbeat-bass-techno.vact`, `rumble-techno.vact`, `wobble-techno.vact` | `:phase-drum` (first hit; bass templates follow) |
| `bass-presets.vact` | `:analog-bass` |
| `experimental-oscillators.vact` | `:vosim` (then `:gendyn`, `:scanned`) |
| `hurdy-gurdy.vact`, `kalimba.vact`, `tonewheel-organ.vact` | the template of the same name |

The issue text says "8 examples", but its own list and
`editor/src/examples/manifest.ts:67-75` name nine. All nine are in scope.

## Root cause

The checker accepts a sound keyword when it is in `kit_keys`, in the
document's `inst_names`, or is a session global of kind `Inst`
(`src/types/manifest.rs:245-256` `sound_arg`;
`src/types/song_rules.rs:395-403`). None of the three sources contains the
prelude templates:

| Source | How it is filled | Contains `:kalimba`? |
|--------|------------------|----------------------|
| `kit_keys` | `Checker::new` takes `manifest.sound_kit_keys()` (`src/types/check.rs:185`) = `SPEC_SOUNDS` (13) union `SPEC_SYNTHS` (7: `sampler analog fm pd additive wavetable granular`, `src/types/manifest.rs:54`). The prelude `default-sound-kit` dict is built from the same keys (`src/vm/natives/sound.rs:45-53`). | no |
| `inst_names` | `inst` forms of the checked document only (`src/types/check.rs:226-229`). | no |
| session `Inst` globals | `Namespace::check_env` lists bound session slots (`src/ns/namespace.rs:720-729`). `load_templates` realizes `templates.vact` in a scratch `Namespace` (`src/ns/insts.rs:930-951`), so template slots never reach the session namespace. | no |
| `Evaluator::dynamic_manifest` | adds the registry's declared header parameter names as controls only (`src/ns/eval_doc.rs:36-64`). | no |

So the voices resolve and play (the runtime looks `Sound::Builtin(kw)` up
in the `InstRegistry` by name), but every checker path rejects them.

### Native and wasm-host assembly side by side

| Step | Native session (`CapabilitySet::native()`) | Wasm host session (`src/host/wasm/session_half.rs:111-123`) |
|------|--------------------------------------------|------------------------------------------------------------|
| Registry | `InstRegistry::shared()` with native caps (`src/session/session.rs:335`, or the test rigs) | `InstRegistry::shared()` with `caps = CapabilitySet::browser()`, passed as `cfg.insts` |
| Templates | `Evaluator::with_insts` -> `load_templates` (`src/ns/evaluator.rs:279`) | same code |
| Session manifest | `HostManifest::spec_default()` (`src/session/session.rs:359`) | same |
| Eval check | `eval_form_in(form, &self.manifest)` -> `dynamic_manifest` -> `check` (`src/session/eval.rs:333`, `src/ns/eval_doc.rs:68-70`) | same |
| Static check (`Session::check`, wasm `session_check`) | `analyze` over `spec_default()` plus package banks (`src/session/frontend.rs:66-72`, `src/session/eval.rs:532-538`) | same |
| `kit_keys` / `inst_names` / session `Inst` | identical: 20 spec keys / document `inst`s / document-defined slots | identical |

There is no tier divergence. Both tiers emit the same diagnostic. The
native tests pass only because they never look at check diagnostics:
"check diagnostics never gate compile or run (7.1.1)"
(`src/ns/evaluator.rs:357-367`), and `E2e::eval` asserts only that each
form's value is `Ok` (`src/host/tests/e2e.rs:123-135`;
`src/host/tests/e2e/templates/bass_examples.rs` renders the four techno
examples through it). The browser test
(`editor/test/wasm/examples.test.ts`) rejects any error-severity
diagnostic, so the browser is where the bug shows. The curated examples
that use these DSP cores work around it by declaring local `inst`s
(`examples/industrial-techno.vact:7`: "Declare the voices locally so the
checker and sound resolver both know them").

The working hypothesis from intake (native exposes templates through
session `Inst` globals) is wrong. The fix belongs in the shared checker
manifest assembly, not in a wasm-only path.

## Source of truth

The source of truth is **the set of `templates.vact` `inst` forms that
`load_templates` realized into this session's `InstRegistry`**.

- It comes from `src/prelude/templates.vact`, the file a new voice is
  already appended to, so there is no new name list.
- It already depends on the tier. Realization checks every template against
  `InstRegistry::caps` (12.8.8), and a failure goes to `template_errors`
  instead of the registry. A template whose DSP is not available in a tier
  is therefore never accepted as a sound in that tier.
- Today every template realizes in both tiers. The regression test (below)
  asserts this, so a future template that fails in the browser shows up as
  a test failure, not as a silent change in what the checker accepts.

`SPEC_SYNTHS`, `SPEC_SOUNDS`, `default_sound_kit()` and the two existing
`TEMPLATE_NAMES` arrays do not change. They are not read for this rule.

## Design

### Data flow

```
templates.vact --load_templates--> InstRegistry
                                     |- names / insts      (runtime resolution, unchanged)
                                     |- template_errors    (unchanged)
                                     '- template sounds    (NEW: realized template names, Rc<BTreeSet<Rc<str>>>)
                                                |
Evaluator::dynamic_manifest(base) = base.with_controls(declared names)
                                        .with_templates(template sounds)   (NEW)
                                                |
                 +------------------------------+------------------------------+
                 |                                                             |
   eval check (eval_form / eval_form_in)                    Session::check (static, editor squiggles)
                 |                                                             |
                 '--------> Checker::new: kit_keys = sound_kit_keys() U templates
                                     -> sound_arg and song_rules accept `s :<template>`
```

### Components

1. **`InstRegistry` template sounds** (`src/ns/insts.rs`). A new field holds
   the names of templates that `load_one` realized (`Rc<BTreeSet<Rc<str>>>`),
   with an `Rc` accessor shaped like `declared_names_rc`. `load_templates`
   inserts each name only on the success branch, next to `release(sink)`.
   The field starts empty, so a registry where templates were never loaded
   (some unit-test harnesses) accepts nothing new. `src/ns/insts.rs` has 970
   lines. If the change would reach 1000, move the template loading section
   (`TEMPLATES`, `TEMPLATE_FILE`, `TEMPLATE_NAMES`, `load_templates`,
   `load_one`) into a child module and re-export it, so that
   `crate::ns::insts::{TEMPLATES, TEMPLATE_NAMES, load_templates}` keep
   resolving.
2. **`HostManifest.templates`** (`src/types/manifest.rs`). A new
   `templates: KeySet` field holds the realized prelude templates that are
   playable as sounds. `spec_default()` sets it to the empty closed set.
   `with_templates(names)` mirrors `with_sounds` and `with_controls`.
   `sound_kit_keys()` keeps its meaning (the keys of the prelude
   `default-sound-kit`) and does not include `templates`. A separate field,
   instead of widening `synths`, keeps four things byte-identical:
   - the prelude dict;
   - `template_params()`;
   - the wire `ManifestBody.synths` (`src/session/session.rs:669-690`);
   - completion (`src/complete/sources.rs:62`).
3. **Checker** (`src/types/check.rs:185`). `kit_keys` becomes
   `manifest.sound_kit_keys()` union `manifest.templates`. Its field doc
   says so. `sound_arg` and `song_rules` already read `kit_keys`, so both
   rules change together with no edit at those two sites. The message text
   stays the same. When `sound-kit` is rebound, the existing
   `sound_kit_is_prelude` gate still turns the check off.
4. **`Evaluator::dynamic_manifest`** (`src/ns/eval_doc.rs:36-64`). It applies
   `with_templates` with the registry's template sounds. The early return
   applies only when both the declared names and the template sounds are
   empty. The cache entry (`DynamicManifestCacheEntry`,
   `src/ns/evaluator.rs:199-203`) adds the template-sounds `Rc` to its key,
   compared with `Rc::ptr_eq` like `controls`.
5. **`Session::check`** (`src/session/frontend.rs:66-72`). It checks against
   the same manifest as eval. A new `analyze_in(src, file, view, base)`
   in `src/session/eval.rs` takes the base manifest. `analyze` keeps its
   signature and delegates with `HostManifest::spec_default()`, so the
   standalone LSP and `src/session/tests/packages.rs` stay unchanged.
   `Session::check` passes `self.ev.dynamic_manifest(&HostManifest::spec_default())`.
   Package banks are still added on top inside `analyze_in`. This also
   makes static checks see the registry's declared template parameters
   (for example `kalimba-beat`), which eval already sees, so the editor's
   check squiggles and the eval-result diagnostics agree for these
   examples.

### Out of scope (stated boundaries)

- The **standalone LSP** without a session (`src/lsp/analysis.rs:191`) has
  no `InstRegistry`, so it keeps the spec-default behavior. It was not
  reported, and covering it would need a second, registry-free source for
  the names.
- **`load ./file.vact`** (`src/ns/load.rs:192,208`) checks nested files
  against `spec_default()` in a scratch namespace. Its check diagnostics
  are not the reported path, and this design leaves them unchanged.
- **Song-mode exclusions** (`song-mode.vact`, `song-mode/generated-parts.vact`,
  `song-mode/requirement-song.vact`) are a runtime limitation, not a check
  limitation. `Runtime::apply` turns every `StagedEffect::PlaySong` into the
  `BeyondCapability` fault "finite song playback is unavailable until song
  transport is installed" (`src/sched/runtime.rs:372-377`). They stay
  excluded unchanged.
- No DSP, codec, UGen, control-row or golden-digest change. The
  append-only registry rules are not touched. `VACTR_BLESS_GOLDEN` is not
  used.

### Editor registry

- `editor/src/examples/manifest.ts`: remove the nine template-voice rows
  from `EXCLUDED`. Six remain: `sample-timestamps.vact`,
  `tracks/breakcore/glass-teeth.vact`, `quad-stems.vact`, `song-mode.vact`,
  `song-mode/generated-parts.vact`, `song-mode/requirement-song.vact`. Their
  reasons do not change.
- Add nine `CURATED` entries in the existing `'Examples'` category, inserted
  in alphabetical id order among the other `'Examples'` rows. No new
  category is needed.
  - `title`: the title-cased id, as the current rows do (`Acid Techno`,
    `Bass Presets`, `Experimental Oscillators`, `Hurdy Gurdy`, `Kalimba`,
    `Offbeat Bass Techno`, `Rumble Techno`, `Tonewheel Organ`,
    `Wobble Techno`).
  - `description`: the file's leading comment text, cut to 140 characters,
    as the current rows do.
  - `requires`: none. All nine are sample-free and have no host input or
    MIDI.
- `editor/test/examples/registry.test.ts` (mechanical): `EXCLUDED.length`
  goes from 15 to 6. The excluded-id rejection probe changes from
  `acid-techno` to a still-excluded id (`song-mode`). Add an assertion that
  the nine ids are registered.
- `editor/test/wasm/examples.test.ts` needs no edit. It iterates
  `listExamples()`, so the nine new entries are evaluated automatically.
- Example sources are not edited, unless the browser evaluation then fails
  for a different reason (for example a browser cap). In that case the
  implementer fixes the example with moderate levels and every slot `gain`
  at most its current value, records the change, and does not restore the
  exclusion.

### Documentation touch-ups

- `design-example-picker-and-persistence.md` section 3.3: add a note that
  the template-voice exclusions were a checker bug, fixed by this design.
- `design-bass-voices.md` "Registration and digest stability": add one
  bullet. A new template becomes a valid `s :name` sound in every tier where
  it realizes, with no list to update. The regression test enforces this.

## Validation

### Regression test (Rust)

Add a new file `src/session/tests/template_sounds.rs` and register it in
`src/session/tests/mod.rs`. Build two session configurations:

- **native**: `SessionConfig::new(CapabilitySet::native())`;
- **host-wasm**: mirror `session_half.rs:111-123`. That means a fresh
  `InstRegistry::shared()` with `caps = CapabilitySet::browser()` set
  before construction, `SessionConfig::new(CapabilitySet::browser())`, and
  `cfg.insts = Some(reg)`.

Template names come from `read_forms(TEMPLATES, TEMPLATE_FILE)`: the
`sym_name` of child 1 of every form whose head is `inst`. The test does not
use a literal list and does not use `TEMPLATE_NAMES`. It asserts that the
set is not empty.

For each configuration:

1. `template_errors()` is empty, and the registry's template sounds equal
   the parsed set. This proves every template's DSP realizes in that tier.
2. For every parsed name, `s :<name> > d1`:
   - evaluated through the session, gives an eval result with no
     `unknown-keyword` diagnostic;
   - checked with `Session::check` (`DOC`), gives no `unknown-keyword`
     diagnostic.
3. `s :not-a-template-voice > d1` still gives exactly one `unknown-keyword`,
   both evaluated and through `Session::check`, so the rule stays closed.
4. For each of the nine examples (`include_str!`), `Session::check` gives no
   error-severity diagnostic. This covers the editor's static check path.

`default-sound-kit` must keep its current 20 keys. The existing prelude and
sound tests (`src/types/tests/sound.rs`, `src/vm/tests/integrate_sound.rs`)
must pass unchanged.

### Gating commands

- `CARGO_TERM_QUIET=true cargo fmt --check`
- `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings`
- Focused: `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(sound)'`
- Full nextest (same environment), run under the measurement lock
- `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm` (the debug cdylib for vitest)
- `mise run build-wasm-release`
- In `editor/`:
  - `npm run check`
  - the example-evaluation vitest (`test/wasm/examples.test.ts`), focused
  - the full vitest under the lock
  - `npm run test:style`
  - `npm run build`
  - `npm run e2e:examples`, with the dev server on a free port other than
    5173 and 5174, under the lock

Measurement lock: `mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`
before each heavy suite and `rmdir` after. If the lock already exists,
wait and retry.

Artifact roots: `target/`, `tree-sitter-vact.wasm`, `editor/node_modules`,
`tmp/`, Playwright outputs.

## Risks

- **Static check widening.** `Session::check` now also accepts the
  registry's declared controls, which matches eval. A session test that
  pinned a static-only diagnostic for a template parameter would need a
  mechanical update. The implementer lists any such update.
- **Hidden second failure.** One of the nine examples may fail in the
  browser for another reason (voice count, cap, runtime fault) once the
  check passes. The focused vitest shows this early; the handling rule is
  in "Editor registry".
- **File size.** `src/ns/insts.rs` is at 970 lines. The split rule in
  component 1 applies.
- **Cost.** Adding to the dynamic manifest is a set union per cache miss.
  The cache keys on stable `Rc`s, so it hits after the first form.
  Browser startup does not change, because templates were already
  realized at `session_init`.
