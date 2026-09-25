# Vactrol Editor: Rust Wire Additions G2-G6 (ED-WIRE) Implementation Plan

**planId**: ED-WIRE (issue #5, TASK-010, wave 2; additive Session Protocol v1 fields and native-testable session,
scheduler and package additions for the editor)
**Status**: Ready
**Design Reference**: design-docs/specs/design-implementation.md 15.1.2 (G2, G3, G4, G5, G6; the `session_check`
and package-driver logic behind G1), 15.1.12; design-docs/specs/command.md ("call", "editor-decl", `manifest`,
`levels`, `tempo`, `0x72`/`0x73` payloads); 9.2-9.3 (uniform plans), 12.5 (analyzer cells), 13.5, 14.5.5-14.5.7
**Created**: 2026-09-26
**Issue**: https://github.com/tacogips/vactrol/issues/5
**dependsOn**: ED-SCAFFOLD (verification only: every ED plan's contract runs `npm run test`, which needs
`editor/package.json`; no Rust source depends on it)
**Dispatch manifest**: impl-plans/active/ed-editor-20260926-s186-dispatch.json

---

## Intent and Context

The editor cannot meet several TASK-010 criteria over today's protocol:
- the `manifest` has no EditorDecl/ParamMeta (`src/session/protocol.rs` `ManifestBody`);
- a site has no call identity (`WireSite`);
- `levels` carries only the master rms (`src/session/publish.rs`);
- `tempo` has no clock state;
- `Runtime::activate` discards the `UniformPlan`, and nothing calls `RenderHost::set_uniforms`
  (`src/sched/runtime.rs` activate);
- the browser has no in-memory package cache or package driver.

This plan adds exactly G2-G6 of design 15.1.2 as ADDITIVE, natively testable Rust. It also adds `Session::check` and
the package driver as core functions, so the wasm glue in ED-WASM only marshals JSON.
- `v` stays 1, and every new wire field is `Option` with `skip_serializing_if`.
- Native CLI, REPL and LSP behavior is unchanged.

## Non-Goals

- No `src/host/wasm/*` change (ED-WASM).
- No TypeScript.
- No new crate.
- No behavior change to eval, publication, authority, directives resolution or packages beyond the additions below.
- No browser taps (E2), per-slot levels (E3), session-side ExternalFile switching (E4), gain-reduction cells, or
  `use-fps`/`use-canvas` wiring.

## writePaths

- `src/session/protocol.rs`, `src/session/session.rs`, `src/session/publish.rs`, `src/session/eval.rs`,
  `src/session/mod.rs`
- `src/session/editors.rs` (new), `src/session/frontend.rs` (new: the additive `impl Session` methods, so
  `session.rs` (703 lines) barely grows)
- `src/session/tests/mod.rs`, `src/session/tests/editor_wire.rs` (new)
- `src/directives/attach.rs`, `src/directives/mod.rs` (only if the table must expose the call list),
  `src/directives/tests/attach.rs`
- `src/sched/runtime.rs`, `src/sched/render.rs` (new), `src/sched/mod.rs`, `src/sched/tests/mod.rs`,
  `src/sched/tests/render.rs` (new)
- `src/ns/insts.rs` (only for a read-only iterator over installed buses, needed by G4)
- `src/pkg/mem_cache.rs` (new), `src/pkg/driver.rs` (new), `src/pkg/mod.rs`, `src/pkg/tests/mod.rs`,
  `src/pkg/tests/mem_cache.rs` (new), `src/pkg/tests/driver.rs` (new)
- `impl-plans/active/vactrol-editor-wire.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **G2: editor metadata.**
   - `session/editors.rs`: `pub fn editor_decls() -> Vec<WireEditorDecl>`. Each `dsp::meta::all()` entry is
     converted as follows:
     - `EditorKind` to its kebab-case name, with `multiband` for `DynamicsTransfer`;
     - `Curve` to `linear|log|stepped`;
     - `Unit` to `none|db|s|ms|hz|st`;
     - `ctl` to `Some(ctl.get())`.
   - It appends the fixed pattern-function table, with `ctl: None`:
     - `euclid` -> `euclid-ring`, params `hits`, `steps`, `rotation`;
     - `maybe` and `degrade-by` -> `probability-dial`, param `probability` (range 0..1);
     - `hold`, `fast` and `slow` -> `length-handle`, param `factor`;
     - `sine`, `saw`, `tri`, `square`, `rand` and `perlin` -> `lfo-shape`, no params.

     Verify each name against `design-docs/specs/lang-reference.md` / `design-music.md`. A name the prelude does not
     define is left out and recorded in `notes.md`.
   - `protocol.rs`: add `WireEditorDecl {name, kind, multiband: Option<bool>, params: Vec<WireParamMeta>}`,
     `WireParamMeta {name, ctl: Option<u16>, range: [f32; 2], curve, unit, group}`, and
     `ManifestBody.editors: Option<Vec<WireEditorDecl>>`.
   - `session.rs` `manifest_body` (or a function moved to `frontend.rs`) fills `editors: Some(editor_decls())`.
2. **G3: site call identity.**
   - `directives/attach.rs` `CallSite` gains `args: Vec<(Option<Rc<str>>, Span)>` (named-argument keyword, argument
     extent), filled where call sites are already enumerated. `NOT_SITES` heads are never calls (unchanged).
   - `protocol.rs`: `WireCall {name, head: WireSpan, ordinal: u16, arg: u16, param: Option<String>}` and
     `WireSite.call: Option<WireCall>`.
   - `publish.rs` `site_wire` gains a `calls: &[CallSite]` argument (the `DirectiveTable.doc.sites` of the site's
     file). Both callers pass it: `eval.rs:434` and `publish.rs:209` (look up the site's file `DocState`).
   - The innermost call is chosen by the smallest `args` extent containing the site span, directly or inside a
     list/pattern argument.
   - `ordinal` is the 1-based count of same-named `CallSite`s within the same top-level form, in source order.
   - `param`: the named keyword; else `params[arg]` when in range; else `None`.
   - A site with no enclosing call gets `call: None`.
3. **G4: analysis and clock telemetry** (in the tick publication of `publish.rs`).
   - `WireLevel.bands: Option<[f32; 8]>` from `hosts.audio.analysis().fft` for `:master`.
   - `LevelsBody.analyzers: Option<Vec<WireAnalyzer {bus, kind, id, cells: Vec<f32>}>>`:
     - list every `EffectKind::Analyzer` unit of every installed bus and `master` (the `InstRegistry`, with an
       iterator in `ns/insts.rs` if none exists);
     - `id` is taken from the unit's constant `id` parameter; skip the unit otherwise;
     - `cells` holds `dsp::effects::analyzer::cells(kind)` values from `Runtime::input_cells().analyzer(..)`.
   - The same 10 per second limit and subscribers-only rule apply.
   - `TempoBody.clock: Option<WireClock {source: "internal"|"midi", locked: Option<bool>}>`:
     - `source` from `rt.clock().source()`;
     - `locked` = `!midi_clock.is_lost()` when `source` is `midi`.

     The `last_tempo` change key includes the clock value, so a clock change emits `tempo`.
4. **G5: per-frame uniforms.**
   - `sched/runtime.rs` `activate`: keep the `UniformPlan` from `compile_tex` in a new per-output field
     (`[Option<UniformPlan>; 4]` or a small map). Clear it when the empty program is sent by hush/stop
     (`sched/control.rs` is NOT edited; clear it at `activate` and in `render_frame` when the slot is no longer
     bound).
   - `sched/render.rs`: `impl Runtime { pub fn render_frame(&mut self, ev: &mut Evaluator, host_now: f64) -> Vec<Failure> }`.
     - It resolves every stored plan through the existing `tex::uniforms` resolution (9.3) at the current position.
     - It calls `hosts.render.set_uniforms(out, &Uniforms)`.
     - A failing uniform keeps its previous value and is reported.
   - `session/frontend.rs`: `Session::render_frame(&mut self, host_now: f64) -> Vec<ServerMsg>`, which returns a
     `diag` message when failures occur.
5. **G6: browser packages.**
   - `pkg/mem_cache.rs`: `MemCache` implements `CacheBackend` fully in memory:
     - `create_staging` never reuses an id;
     - `publish` is an atomic move;
     - a failed staging leaves no entry;
     - the stamp works as in `FsCache`.
   - `pkg/driver.rs`:
     - `Prefetched` implements `ProxyTransport`. It holds `url -> Result<bytes, status>` supplied by JS and records the
       FIRST missing URL.
     - `pub enum DriverRequest { Resolve { proxy, requirements: BTreeMap<String, String> }, Restore { proxy, lock:
       String } }`.
     - `pub enum DriverReply { Need { url }, Done { lock: String, resolved: Vec<(path, version, sha256_hex)> },
       Error { code, message } }`.
     - `pub fn drive(req, supplied: &Prefetched, cache: &mut dyn CacheBackend) -> DriverReply`:
       - `Resolve` builds a root `PkgManifest` from the requirements. An empty version means `latest_release` of
         `list_versions`. It then runs `get_all`.
       - `Restore` parses the lock and runs `fetch_and_publish(.., Some(expected))` per entry.
       - When the transport recorded a missing URL, the reply is `Need`, and staging was already discarded by the
         existing pipeline.
       - A `PkgError` becomes `Error` with its `DiagCode` string.
   - `session/frontend.rs`:
     - `Session::drive_packages(&mut self, req, supplied) -> DriverReply` lends `self.cache` (installing a `MemCache`
       when it is `None`) and, on `Done`, replaces `self.lock`;
     - `Session::check(&self, file: &str, text: &str) -> Vec<WireDiag>`: `session::eval::analyze` plus the directive
       lint (`directives::build_table` over the read nodes and trivia), converted with the existing wire helpers. It
       NEVER executes.
   - JSON helpers for the `0x73` reply (`DriverReply::to_json`) live in `pkg/driver.rs`.
6. **Declarations.**
   - `session/mod.rs`: `pub mod editors; mod frontend;`.
   - `sched/mod.rs`: `pub mod render;`.
   - `pkg/mod.rs`: `pub mod mem_cache; pub mod driver;` plus re-exports.
   - The three `tests/mod.rs` files add the new test modules.

## Required Tests

- `session/tests/editor_wire.rs`:
  - `manifest?` reply: `editors` contains `peq` (`eq-curve`), `env-adsr` (`envelope-shape`), `lpf`
    (`filter-response`) and `euclid` (`euclid-ring`, `ctl` absent), and JSON field names match command.md.
  - Sites of `s :hats > lpf 800 > hpf 3000` carry `call.name` `lpf`/`hpf`, `ordinal` 1, and `param` `cutoff` for both.
  - A second `lpf` gets ordinal 2. `n [0 3 5]` gives three sites with `call.name` `n`.
  - Named arguments give `param` from the keyword. A top-level `let x 3` site has `call` absent.
  - `levels` (recording host with synthetic analysis) carries `bands` and an `analyzers` entry for a bus with a
    `level` analyzer of constant `id`, with `cells` of the right length.
  - `tempo.clock` is `internal`; after `use-clock :midi` it is `midi`, and it flips `locked` to false after the
    clock-loss timeout (reuse the `sched/tests/midi.rs` pulse helpers).
  - The old JSON without the new fields still decodes (compatibility).
  - `Session::check` of text with a type error returns the checker diagnostic, and the evaluator state and slot table
    are unchanged: the recording host saw no event.
  - `drive_packages` over the local-directory proxy fixture (`pkg/tests/http_fixture.rs` bodies, supplied in memory):
    first `Need(list)`; after supplying everything, `Done` with a lock whose digest equals the native `get` digest;
    the next `eval` of `import` loads the package.
- `sched/tests/render.rs`: after binding `osc 20 > rotate 0.5 > out o0`, the recording render host receives no program
  before the cycle boundary and one program after it. Then `render_frame` calls `set_uniforms` with one value per
  `uniform_names` entry, and the values are time-varying for a signal uniform. After `hush`, `render_frame` sends no
  uniforms.
- `pkg/tests/mem_cache.rs`: staging ids are unique; publish is atomic; a failure midway leaves no entry.
- `pkg/tests/driver.rs`:
  - the `Need` sequence is deterministic;
  - 404 gives `Error{package-resolve}`;
  - a supplied zip with a traversal entry gives `Error{package-integrity}` naming the entry, and the cache is
    unchanged;
  - `Restore` with a tampered body gives `package-integrity`.
- `directives/tests/attach.rs`: `CallSite.args` spans and named keywords for the spec directive examples. The
  existing attach, resolve and label tests stay green unchanged.

## Invariants

- The wire is additive. Every existing session, directives, pkg, cli and lsp test passes unchanged.
- No `.rs` file reaches 800 lines. Watch `session/session.rs` (703), `sched/runtime.rs` (776) and
  `session/eval.rs` (610). If one would reach 800, move the new code into `frontend.rs`/`render.rs`.
- Every new module is core and wasm-safe: no `std::{fs,net,process,thread,time}`. V9 shows no gated crate.
- `Session::check` and `drive_packages` never evaluate user forms. The driver never performs I/O.

## Edit Protocol

The common protocol in `vactrol-editor-scaffold.md`, with `<planId>` = `ED-WIRE`. Rust goes through the rust-coding
agent, then check-and-test-after-modify.

## Verification (`<wave>` = `wire`)

The common rows V1, V2, V3, V3t, V7, V6a, V6b, V6c, V4, E0-E5, plus V1l, V2l and V9, plus:

| # | Command | Evidence |
|---|---------|----------|
| W1 | LOG(`own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests::editor_wire/) \| test(/sched::tests::render/) \| test(/pkg::tests::mem_cache/) \| test(/pkg::tests::driver/) \| test(/directives::tests::attach/)'` | `exit=0`, run > 0 |
| W2 | LOG(`session`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/session::tests/) \| test(/pkg::tests/) \| test(/directives::tests/) \| binary(directive_fixtures) \| binary(cli)'` | `exit=0` (no regression) |
| W3 | LOG(`lsp-smoke`): `CARGO_TERM_QUIET=true cargo test --features lsp --test lsp_smoke` | `exit=0` |
| W4 | `wc -l src/session/*.rs src/sched/runtime.rs src/sched/render.rs src/directives/attach.rs src/pkg/driver.rs src/pkg/mem_cache.rs` | all under 800 |

## Completion Criteria

- [ ] Items 1-6 implemented
- [ ] Required tests pass; no existing test changed
- [ ] Common rows and W1-W4 pass with logs cited; `final-hashes.txt` written

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, ED-WIRE implementer)` entry. Edit only this log.)

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-010)
- **Previous**: vactrol-editor-scaffold.md. **Parallel**: vactrol-editor-code.md, vactrol-editor-midi.md
- **Next**: vactrol-editor-wasm.md


### OWNERSHIP AMENDMENT (operator, 2026-09-26, resolves ED-WIRE-B1)

- `src/session/tests/codec.rs` is added to this plan's writePaths (manifest
  session187Amendment). Permitted edit: mechanical literal completion only
  (`call: None` in `site()`, `editors: None` in the ManifestBody sample,
  `bands: None` in the WireLevel sample, `analyzers: None` in the LevelsBody
  sample, `clock: None` in the TempoBody sample); no assertion changes.
