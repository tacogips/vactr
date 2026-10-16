# BV-10: Realized Template Voices as Checker Sounds (Rust) and Regression Test

**Status**: Ready
**Plan ID**: BV-10 (wave 1, parallel with BV-20)
**Design Reference**: `design-docs/specs/design-browser-tier-voices.md`
("Source of truth", "Design" components 1-5, "Validation"; decisions
BTV-D2..D6, D8, D9)
**Created**: 2026-10-17
**Last Updated**: 2026-10-17

## Intent and Context

`s :kalimba`, `s :phase-drum`, `s :analog-bass` and the other prelude
templates play in every tier. The checker still rejects them with
`unknown-keyword` "`` `:<name>` is not a sound in the default sound kit ``".
This happens in both tiers: native tests ignore check diagnostics, and the
browser example test does not. `kit_keys` holds only the 20 spec keys
(`src/types/check.rs:185`, `src/types/manifest.rs:36-91`).
`load_templates` realizes templates in a scratch namespace
(`src/ns/insts.rs:930-951`), so the checker never sees them.

This plan makes the checker accept every template that `load_templates`
realized into the session's `InstRegistry`. That registry set is the single
source of truth, and it already depends on the tier: a template beyond the
tier caps goes to `template_errors`. The plan also adds the Rust regression
test for the native and host-wasm configurations.

## Non-goals (do NOT do these)

- Do not change `SPEC_SOUNDS`, `SPEC_SYNTHS`, `HostManifest::sound_kit_keys`,
  `HostManifest::template_params`, `default_sound_kit()` /
  `digital_kit()` (`src/vm/natives/sound.rs`), or either `TEMPLATE_NAMES`
  array. The prelude `default-sound-kit` must keep exactly its 20 keys.
- Do not put template names into `HostManifest.synths` or `sounds`. Use the
  new `templates` field only.
- Do not add any literal list of template names, in source or in tests.
- Do not change `analyze`'s signature or behavior, the standalone LSP
  (`src/lsp/analysis.rs`), `src/ns/load.rs`, completion
  (`src/complete/`), `Session::manifest_body` / the wire `ManifestBody`, or
  the song-mode runtime fault (`src/sched/runtime.rs`).
- Do not edit the diagnostic message text or `src/types/song_rules.rs`. It
  reads `cx.kit_keys`, so it is fixed through the checker field.
- No DSP, codec, UGen, control-row or golden-digest change. Never set
  `VACTR_BLESS_GOLDEN`.
- No edits under `editor/` or `examples/`. Those belong to BV-20 and BV-30.

## Dependencies

- **dependsOn**: none
- **Blocks**: BV-30

## writePaths

- `src/ns/insts.rs`
- `src/ns/insts` (only if the BTV-D8 split is needed; new child module directory)
- `src/types/manifest.rs`
- `src/types/check.rs`
- `src/ns/eval_doc.rs`
- `src/ns/evaluator.rs`
- `src/session/eval.rs`
- `src/session/frontend.rs`
- `src/session/tests/template_sounds.rs` (new)
- `src/session/tests/mod.rs`
- `impl-plans/active/browser-voices-10-checker-templates.md` (this plan's Progress Log only)
- `target` (artifact root)
- `tmp/browser-voices/BV-10` (artifact root; evidence logs)

## sharedPaths

- none

## Contract (pinned shapes; BV-30 relies on them)

- `InstRegistry` (in `src/ns/insts.rs`) gets a private field
  `template_sounds: Rc<BTreeSet<Rc<str>>>`. It starts empty in
  `InstRegistry::new`. Add an accessor
  `pub fn template_sounds_rc(&self) -> Rc<BTreeSet<Rc<str>>>` that clones
  the `Rc`. Imitate `InstRegistry::declared_names_rc` (`src/ns/insts.rs:325`).
- `HostManifest` (in `src/types/manifest.rs`) gets a field
  `pub templates: KeySet` with the doc "Realized prelude templates playable
  as sounds; not part of `default-sound-kit`". `spec_default()` sets
  `templates: KeySet::of([] as [&str; 0])`. Add
  `pub fn with_templates<I, S>(&self, extra: I) -> HostManifest`
  (`I: IntoIterator<Item = S>, S: AsRef<str>`), which unions into
  `templates` and keeps everything else (imitate `with_sounds`,
  `src/types/manifest.rs:131-140`).
- `DynamicManifestCacheEntry` (in `src/ns/evaluator.rs:199-203`) gets
  `pub(super) templates: Rc<BTreeSet<Rc<str>>>`.
- `src/session/eval.rs` gets
  `pub fn analyze_in(src: &str, file: FileId, view: &PackageView<'_>, base: &HostManifest) -> Analysis`.
  `analyze(src, file, view)` keeps its exact signature and becomes a
  one-line delegate with `&HostManifest::spec_default()`.

## File-level changes

1. `src/ns/insts.rs`
   - `load_templates` collects the name of every form whose `load_one`
     returned `Ok`, in the existing success branch next to
     `vm.effects_mut().release(sink)`.
   - At the end, where `template_errors.extend(errors)` already takes
     `reg.borrow_mut()`, set `template_sounds` to a new `Rc` holding the
     union of the old set and the collected names. Use a union because one
     registry may serve more than one `Evaluator`.
   - Take the name the same way `load_one` does (child 1 `sym_name`). The
     simplest way is to return it from `load_one` or to compute it in the
     loop. Never insert `"?"`.
   - Pitfall: do not `borrow_mut` the registry inside the loop while
     `load_one` runs. `realize_inst` borrows it.
   - Line budget: the file is 970 lines. If it would reach 1000, move
     `TEMPLATES`, `TEMPLATE_FILE`, `TEMPLATE_NAMES`, `load_templates` and
     `load_one` into `src/ns/insts/templates.rs`
     (`mod templates; pub use templates::{...};` in `insts.rs`).
     `crate::ns::insts::{TEMPLATES, TEMPLATE_FILE, TEMPLATE_NAMES, load_templates}`
     must keep resolving everywhere. Check with
     `grep -rn "insts::" src`.
2. `src/types/manifest.rs`: the field, `spec_default`, `with_templates` and
   their docs. `sound_kit_keys()` is unchanged. Update its doc only if
   needed to say that it excludes `templates`.
3. `src/types/check.rs:185`: set
   `kit_keys: manifest.sound_kit_keys().union(&manifest.templates)`. Update
   the field doc (line 149, "The keys of `default-sound-kit`") to say "plus
   the session's realized prelude templates". Nothing else changes.
4. `src/ns/eval_doc.rs` `dynamic_manifest`:
   - read both `declared_names_rc()` and `template_sounds_rc()` from the
     same `insts.borrow()`;
   - return `base.clone()` early only when both are empty;
   - when there is no registry, keep returning `base.clone()`;
   - build `base.with_controls(controls.iter()).with_templates(templates.iter())`;
   - the cache lookup also needs `Rc::ptr_eq(&entry.templates, &templates)`;
   - store `templates` in the new cache entry;
   - keep the 4-entry eviction.
5. `src/ns/evaluator.rs`: the cache entry field only.
   `dynamic_manifest` stays `pub` (it is already `pub fn` in eval_doc.rs).
6. `src/session/eval.rs`:
   - split `analyze` into `analyze_in`, so that
     `let mut manifest = base.clone();` replaces
     `HostManifest::spec_default()` at line 538;
   - package banks are still added on top;
   - keep the doc comment and add one line saying `analyze_in` takes the
     base manifest.
7. `src/session/frontend.rs` `Session::check`: call
   `analyze_in(text, fid, &view, &self.ev.dynamic_manifest(&HostManifest::spec_default()))`.
   `dynamic_manifest` returns `Rc<HostManifest>`, so pass `&*rc`. Import
   `HostManifest`. The `build_table` lint call keeps `&self.manifest`.
8. `src/session/tests/mod.rs`: add `mod template_sounds;` in alphabetical
   position.
9. `src/session/tests/template_sounds.rs` (new, under 300 lines). It uses
   `super::support::{Rig, DOC}`. The module doc cites
   design-browser-tier-voices.md "Validation".

## Regression tests to add (`src/session/tests/template_sounds.rs`)

Helpers:

- `template_names() -> BTreeSet<String>`: run
  `crate::ns::load::read_forms(TEMPLATES, TEMPLATE_FILE)`, keep the forms
  whose head is `inst` (`crate::types::check::def_head(form) == Some("inst")`),
  and map them to `children[1].sym_name()`. Assert the set is non-empty.
  Do NOT use `TEMPLATE_NAMES`.
- `native_rig()`: `Rig::with(SessionConfig::new(CapabilitySet::native()))`.
- `wasm_host_rig()`: mirror `src/host/wasm/session_half.rs:111-123`:
  - `let reg = InstRegistry::shared(); reg.borrow_mut().caps = CapabilitySet::browser();`
  - `let mut cfg = SessionConfig::new(CapabilitySet::browser()); cfg.insts = Some(Rc::clone(&reg));`
  - then `Rig::with(cfg)`.
  - Add a comment pointing at session_half.rs for the mirroring.

Cases, each run for both rigs (loop over `[("native", native_rig), ("wasm-host", wasm_host_rig)]`):

- Registry state -> `template_errors()` is empty, and the
  `template_sounds_rc()` names equal `template_names()`. Read the registry
  through `rig.s.evaluator().insts()`.
- Each name `n` in `template_names()`:
  - `rig.eval(&format!("s :{n} > d1"), rev, 0)` -> `r.diagnostics` has no
    entry with `code == "unknown-keyword"`. Increase `rev` on every call.
    Assertion messages name the tier and the template.
  - `rig.s.check(DOC, &format!("s :{n} > d1"))` -> no `unknown-keyword`.
- `s :not-a-template-voice > d1` -> exactly one `unknown-keyword`, both
  from `rig.eval` and from `rig.s.check`. This is a gating assertion that
  the rule stays closed.
- Each of the 9 examples
  (`include_str!("../../../examples/<name>.vact")` for acid-techno,
  offbeat-bass-techno, rumble-techno, wobble-techno, bass-presets,
  experimental-oscillators, hurdy-gurdy, kalimba, tonewheel-organ) ->
  `rig.s.check(DOC, src)` has no diagnostic with `severity == "error"`.
  Check the relative `include_str!` path from `src/session/tests/`.
- `default-sound-kit` unchanged ->
  `HostManifest::spec_default().templates` is the empty closed set, and
  `sound_kit_keys()` has 20 keys.

If the example static-check case reports an error that is NOT
`unknown-keyword` and is not caused by this change, do not weaken the
test. Record the diagnostic in the Progress Log and stop for serial
reconciliation (BV-30 owns example fixes).

## Invariants

- `HostManifest::spec_default()` is unchanged except that it now has
  `templates` empty. Every existing `==` comparison
  (`eval_form_in` `manifest == spec`, the cache key) still behaves the same
  when no templates are loaded.
- `Evaluator`s without a registry, or with templates not loaded, accept
  exactly what they accepted before. `:not-a-sound` stays
  `unknown-keyword` (`src/vm/tests/integrate_sound.rs`,
  `src/types/tests/sound.rs`).
- A template is accepted in a tier only if it realized there.

## Edit protocol

Before each edit, re-read the file and record `shasum -a 256 <file>` in the
Progress Log; record it again after the edit. On drift, stop, record,
re-read and re-apply only this plan's change. Never edit outside
writePaths. Do not commit, stash, checkout, reset or push.

## Verification (each log goes to `tmp/browser-voices/BV-10/<name>.log` and ends with the exit status)

- `fmt.log`: `CARGO_TERM_QUIET=true cargo fmt --check` -> exit 0.
- `clippy.log`: `CARGO_TERM_QUIET=true cargo clippy --all-targets -- -D warnings` -> exit 0.
- `focused.log`:
  `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(template_sounds) | test(sound) | test(templates) | test(analysis) | test(packages)'`
  -> exit 0, 0 failures; the new tests are listed as run.
- `wasm-lib.log`:
  `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
  -> exit 0. This also refreshes the debug cdylib for BV-30.
- `lines.log`: `wc -l` over every touched `.rs` file -> each is under 1000.
- `mutationEvidence` only (not gating), `tmp/browser-voices/BV-10/mutation.log`:
  - temporarily revert step 3 (the `kit_keys` union) and run
    `cargo nextest run -E 'test(template_sounds)'` -> it must fail;
  - restore step 3 and re-run -> it must pass;
  - record both hashes.

The full nextest run is BV-30's job, under the measurement lock.

## Completion Criteria

- [ ] `InstRegistry::template_sounds_rc` is filled only from successful `load_templates` realizations
- [ ] `HostManifest.templates` and `with_templates` added; `spec_default().templates` is empty
- [ ] Checker `kit_keys` is the union of `sound_kit_keys()` and `templates`
- [ ] `dynamic_manifest` adds templates, and the cache keys on both `Rc`s
- [ ] `analyze_in` added; `analyze` unchanged for callers; `Session::check` uses the evaluator's dynamic manifest
- [ ] `src/session/tests/template_sounds.rs` passes in both configurations, with no literal name list
- [ ] fmt, clippy, focused nextest and the host-wasm lib build pass, with logs
- [ ] Every touched file is under 1000 lines
- [ ] Progress Log updated with hashes, commands and results

## Progress Log

### Session: 2026-10-17 (plan authored, session 386)
**Tasks Completed**: none
**Tasks In Progress**: none
**Blockers**: none
**Notes**: Plan created from the accepted design (Step 3 accepted, no findings).
