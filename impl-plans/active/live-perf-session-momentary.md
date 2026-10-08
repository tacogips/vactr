# LP-SESSION-MOMENTARY: Session momentary table, evaluator override, cell ramps

**Status**: Ready (after LP-CONTRACT)
**Plan ID**: LP-SESSION-MOMENTARY (wave 2; parallel with LP-ENGINE, LP-SESSION-STOP, LP-EDITOR-STOP, LP-EDITOR-MOMENTARY)
**Design Reference**: `design-docs/specs/design-live-performance.md` 5.1, 5.5, 5.6, 5.7, 5.8 (constants), 8.1(5), D7, D8, section 11
**Manifest**: `impl-plans/active/live-perf-dispatch.json`
**Created**: 2026-10-08
**Last Updated**: 2026-10-08

## Intent and Context

A right-drag momentary tweak sends `momentary {file, id, form_gen,
edit_epoch, target: number | null, ramp_ms}`. LP-CONTRACT pinned:

- the message and its body `MomentaryBody`;
- the session dispatch `Session::on_momentary`;
- the per-tick hook `Session::momentary_tick(now)`, called in
  `tick_routed` before `rt.drain`;
- `Session::momentary_clear()`, called on hush;
- the field `Session::momentary: MomentaryTable`;
- the stub `Runtime::ramps: RampSender` with `on_ack(cell, seq)`.

This plan implements the design's separate override layer:

- it **never writes the tweak slot**, so `migrate`, persistence, commit and
  the `set-tweak` overlay never see it;
- only Direct-tier sites accept it;
- each change becomes a ramp on the audio clock:
  - audio side: one `CtlMsg::CellRamp` per fed cell per ramp change, applied
    by LP-ENGINE's `CellRamps`;
  - evaluator side: a `VarSlot` momentary value refreshed at most once per
    16 ms.

Repository facts the work relies on:

- `VarSlot` is in src/ns/namespace.rs:69, with `get()` at :116 and `set()`
  at :175. `PParam::Late` reads go through `vm.deref`, which returns
  `get()` (src/pattern/eval.rs:499).
- `CellTable::write_slot(slot)` (src/sched/cells.rs:277) encodes
  `slot.get()` per cell with `encode_value(row, map, &value)`.
- Inst-default cells are reached through `registry.entries()` with
  `entry.cells` slot pairs and `entry.default_cell_value(cell)`
  (src/sched/runtime.rs:385-405).
- `Evaluator::migrate` (src/ns/evaluator.rs:843) copies `p.slot.get()`
  into the new slot when it differs from the literal.
- `on_set_tweak`/`check_tweak` (src/session/authority.rs:133-156, :322)
  validate file, id, form_gen and epoch.
- On the browser tier, cell records travel through the `CellPort`. Only
  acked incarnations (`Entry.acked`) are readable by the worklet.

## Non-goals

- No engine work (LP-ENGINE), no stop logic (LP-SESSION-STOP) and no editor
  work.
- Eager dependents of a `var` (derived bindings recomputed through the
  reactive pass) do **not** follow a momentary value; only readers of the
  slot itself do (design section 11). Record this as a stated limitation;
  it is not a defect.
- Query-time evaluator reads use the ramp evaluated at the tick's `now`, not
  at each event's time. The bounded offset is at most the lookahead
  (120 ms). This answers the Step-3 review note: state it in the Progress
  Log and do not build per-event-time evaluation.

## Dependencies

- **dependsOn**: LP-CONTRACT.
- **Blocks**: LP-EVIDENCE.

## writePaths

- `src/session/momentary.rs`
- `src/session/authority.rs`
- `src/session/publish.rs` (the `site.value` publication only, which must use `base()`)
- `src/ns/namespace.rs`
- `src/ns/evaluator.rs`
- `src/sched/cells.rs`
- `src/sched/ramps.rs`
- `src/session/tests/momentary.rs`
- `src/sched/tests/sched/ramps.rs`
- `impl-plans/active/live-perf-session-momentary.md` (Progress Log only)
- Artifact roots (also in the manifest's `artifactRoots`): `target`,
  `tmp/live-perf/session-momentary`

## sharedPaths

None. Read-only:

- `src/sched/runtime.rs` (`invalidate_uncommitted`, the inst-registry cell
  loop)
- `src/session/session.rs`
- `src/session/protocol.rs`
- `src/dsp/controls.rs` (`ControlRow` domains)
- `src/sched/commit.rs` (`encode_value`)
- `src/host/testing.rs`

## Pinned Shapes

- `src/session/momentary.rs` constants:
  - `pub const MAX_MOMENTARY: usize = 16;`
  - `pub const MIN_RAMP_S: f64 = 0.005;`
  - `pub const EVAL_PUSH_S: f64 = 0.016;`
  - `pub const MAX_RAMP_MS: u32 = 10_000;`
- `MomentaryTable { entries: Vec<Entry> }` (bounded by `MAX_MOMENTARY`).
  Each `Entry` holds:
  - `id: TweakId`, `slot: VarSlotRef`, `index`, `ty`, `origin`,
    `form_gen`;
  - `ramp: Ramp { from: f64, to: Target, start: f64, dur: f64 }` in audio
    seconds, with `Target = Value(f64) | Base`;
  - `releasing: bool`, `stepped: bool`, `last_push: f64`;
  - `cells: SmallVec or Vec<(CellId, u32 /*epoch*/)>`;
  - `alias: Option<TweakId>`: the entry's id before its most recent
    re-key. It holds one old id at most, so the total is bounded by
    `MAX_MOMENTARY`.
- `VarSlot` gains `momentary: RefCell<Option<Value>>`:
  - `get()` returns the momentary value when set, else the value;
  - new `base()` returns the stored value;
  - new `set_momentary(Option<Value>)` does **not** bump `version`.
- `RampSender` (src/sched/ramps.rs) holds, per cell:
  - `last: (CtlMsg, end_frame: u64, waited: u32, acked: bool)`;
  - `seq: u32` (wrapping).

  Its methods are:
  - `post(...)`
  - `on_ack(cell, seq)`
  - `tick(...)`, which re-posts unacked ramps after `cfg.resend_ticks`
    with `frames = end_frame - now_frame`, saturating at 0;
  - `drop_cell(cell)`, which posts a `release: true, frames: 0` drop.
- **Rejections.**
  - A Reeval or Manual tier, or a non-numeric site, gets
    `ServerMsg::StaleBinding` with `StaleReason::MomentaryIneligible`.
  - A full table gets `StaleReason::MomentaryCapacity`.

  Both reasons were pinned by LP-CONTRACT.
  - A **stale drag** (`target` is a number, with a wrong form_gen, a wrong
    epoch, or an edit-invalidated site) is **dropped silently, with no
    reply**. Count it in a debug counter. The editor re-sends held targets
    after its site table re-keys, and a `stale-form-gen` reply would make
    `SiteWriter.onStale` re-send an unrelated overlay value.
- **Release rule** (`target: null`; fixes LP-PLAN-R1-STALE-RELEASE).
  - **Lookup:** resolve the release by `file` + `id` against either an
    entry's current `id` or an entry's `alias` (the id before its most
    recent re-key).
  - **Found:** apply the release without the form_gen, epoch and
    edit-invalidated checks:
    - `to = Base`, `releasing = true`;
    - `dur = max(ramp_ms / 1000, MIN_RAMP_S)`, with `ramp_ms` clamped to
      `MAX_RAMP_MS`;
    - `from` is the current effective value;
    - post release `CellRamp`s to the entry's **current** cells.
  - **Not found:** a silent no-op, with no reply and no `CtlMsg`.
  - **Why this is safe:** a release only returns to the base value, so it
    can never apply a stale value. Rejecting it would leave the entry held
    forever, because the editor forgets a gesture at `end()`.
  - **Scope:** the tier and capacity checks do not apply to a release that
    resolves to an existing entry.

## Tasks

### TASK-M1: `VarSlot` override (`src/ns/namespace.rs`, `src/ns/evaluator.rs`, `src/sched/cells.rs`, `src/session/publish.rs`, `src/session/authority.rs`)

Add `momentary`, `base()` and `set_momentary()`. Every read that means
"stored, document or overlay value" switches to `base()`:

- `Evaluator::migrate`'s `p.slot.get()`;
- `CellTable::write_slot`'s `slot.get()`;
- the site value published in `bindings`/`eval-result`
  (src/session/publish.rs around :159);
- every `slot.get()` in src/session/authority.rs that reads a tweak value
  for coercion, restore or rollback.

Find these with
`grep -n '\.get()' src/ns/evaluator.rs src/sched/cells.rs src/session/authority.rs src/session/publish.rs`
and classify each hit. Reads that mean "what plays now" keep `get()`:
VM deref, pattern evaluation and texture uniforms.

Also record the migration mapping. In `migrate`, push `(old_id, new_id)`
for every matched pair, including `Binding`-origin sites matched by
`VarSlotRef::same` or by name, into a bounded
`last_migrations: Vec<(TweakId, TweakId)>` on the evaluator. It is cleared
at the start of each eval, and read with
`pub fn take_migrations(&mut self) -> Vec<(TweakId, TweakId)>`.

### TASK-M2: Cell targets and the Const-downgrade override (`src/sched/cells.rs`)

- **`pub fn site_cells(&self, slot_id: u64) -> Vec<(CellId, u32, &'static ControlRow, CellMap, bool /*acked*/)>`**
  lists the cells a slot feeds.
- **`pub fn encode_for(&self, cell, value: &Value) -> Option<f32>`**
  encodes with the same `encode_value(row, map, value)` that `write_slot`
  uses. `None` means out of domain: that cell is skipped.
- **Stepped rows:** a row whose encoding is an enum index, a boolean or an
  integer domain is `stepped`. Inspect `ControlRow` and add a small
  predicate.
- **Const-downgrade override:** each `Entry` gains `momentary:
  Option<f32>`, set and cleared by the session. The pre-ack `Const`
  downgrade in `ctl_for` (cells.rs:165) uses `momentary.unwrap_or(value)`.
  `write_slot` never touches `momentary`.
- **Inst-default cells** (external cells): compute the encoded target by
  temporarily setting the slot momentary to the target, calling
  `entry.default_cell_value(cell)`, and restoring it. Do this in the
  session code that has the registry (src/session/momentary.rs, through
  `self.ev.insts()`). This covers the Binding-origin and inst-default
  Direct sites that the Step-3 review asked about.

### TASK-M3: `RampSender` (`src/sched/ramps.rs`)

**Posting.**

- Browser: `CellTable`'s port (expose `pub fn post_ctl(&mut self, msg: CtlMsg, audio: &mut dyn AudioHost)`
  in cells.rs; the browser uses `port.post`).
- Native: `audio.post`.
- Only post for **acked** incarnations. An unacked cell is skipped; its
  Const-downgrade override covers commits.

**Frames.** `frames = round(dur * cfg.sample_rate)`, and `end_frame` is
derived from `hosts.audio.now()` frames.

**Bounds.**

- Concurrent ramp cells must not exceed 32 (`MAX_CELL_RAMPS`). A request
  over the limit returns `Err(Capacity)`, which the session turns into
  `momentary-capacity`.
- Clearing a cell (drop) frees its count once acked, or immediately if the
  table no longer tracks it.

### TASK-M4: The momentary table (`src/session/momentary.rs`)

**`on_momentary(conn, seq, b)`:**

- If `b.target` is null, apply the **Release rule** (Pinned Shapes) and
  return. No staleness, tier or capacity check runs.
- For drags: validate with the same helper `on_set_tweak` uses (stale
  form_gen, stale epoch, edit-invalidated site). A stale drag is dropped
  silently.
- Then require tier Direct and a numeric site; otherwise reply with
  `MomentaryIneligible`.
- Then check capacity; otherwise reply with `MomentaryCapacity`.
- `ramp_ms` is clamped to `MAX_RAMP_MS`.
- `dur = max(ramp_ms / 1000, MIN_RAMP_S)`.
- `now = self.last_host_now`.

**New or updated entry.**

- `from = current effective value`: the ramp value at `now`, else
  `slot.base()`.
- `to = Value(coerce to the literal's NumTy)` when `target` is a number.
  `target: null` means `to = Base` and `releasing = true`.
- Post a `CellRamp` for every fed cell:
  - `target = encode_for(cell, to)`;
  - `release = releasing`;
  - for stepped cells, `frames = 0` with the rounded value;
  - for continuous cells, the frames above.
- Set `slot.set_momentary(Some(value at now))`, call
  `self.rt.invalidate_uncommitted()`, and set `last_push = now`.

**`momentary_tick(now)`.** First, if `self.rt.output.cuts()` differs from
the last value the table saw, call `momentary_clear()` and store the new
value. This covers a `hush` evaluated as code; the client `hush` already
calls `momentary_clear()` directly. Then, for each entry:

- When `now - last_push >= EVAL_PUSH_S`, or when the ramp has ended:
  - recompute `v = ramp(now)`;
  - if it changed, `set_momentary(Some(v))`, update the cell overrides
    (TASK-M2) and invalidate;
  - for stepped cells, post a `frames = 0` ramp when the rounded value
    changed.
- When `releasing && now >= start + dur`:
  - `set_momentary(None)`;
  - clear the cell overrides;
  - invalidate;
  - remove the entry. No drop message is needed: the engine frees release
    ramps at their end frame.
- Then `self.rt.ramps.tick(...)` handles re-sends.

**Base change.** If the base changes during a releasing ramp (the slot
`version` changed), re-issue the ramp from the current value to the new
base over the remaining time.

**Re-base after eval.** Call `momentary_rebase()` from the authority
refresh path (src/session/authority.rs `refresh_auth`, after the site map
is rebuilt). It uses `ev.take_migrations()`:

- **Matched:**
  - Store the old `id` in `alias`, overwriting any earlier alias, so only
    one step of history is kept.
  - Re-key `id`, `slot`, `form_gen` and `cells`.
  - For each old cell that is no longer fed, `drop_cell`.
  - Held entries: post hold ramps (`frames = 0`, current value) to the new
    cells once they are acked.
  - Releasing entries: post the ramp to the new `Base` over the remaining
    duration.
- **Unmatched:** `set_momentary(None)` on the old slot, `drop_cell` for
  each of its cells, and remove the entry together with its alias. A later
  release for that id is therefore a no-op.

**`momentary_clear()` (hush):** for every entry, `set_momentary(None)` and
`drop_cell` for each cell, then empty the table.

## Key Points a Careless Implementation Gets Wrong

- **Never call `slot.set(...)`.** That would bump the version, trigger
  `migrate` carry-over and publish an overlay. The momentary never appears
  in `site.value`, `bindings` or persistence.
- `write_slot` must encode `base()`. Otherwise a slider move during a tweak
  would bake the momentary value into the cell base.
- **Never post a `CellRamp` for an unacked browser incarnation.** The
  worklet would ramp from 0.
- **Releases bypass staleness; drags never do.** A `target: null` message
  that resolves by current id or alias is always applied, because it only
  returns to base. A drag target is never applied when its form_gen or
  epoch is stale or the site is invalidated; it is dropped silently. Do not
  merge these two paths, and never apply a stale drag value.
- A ramp always starts from the current effective value (no jumps). The
  snap minimum is 5 ms.
- Every new entry must respect `MAX_MOMENTARY` and the 32-cell bound. All
  `Vec`s here are bounded by these constants.
- Do not edit `session.rs`, `runtime.rs` or `protocol.rs` (contract-owned).
  If a hook is missing or wrong, record it in the Progress Log for serial
  repair.
- Keep every touched file under 1000 lines:
  - namespace.rs is 752 lines and evaluator.rs is 887; keep the migrate
    mapping small;
  - authority.rs is 715; only one call to `momentary_rebase` is added
    there.

## Tests to Add (input -> expected)

**`src/session/tests/momentary.rs`** uses the session test support with the
native test host recording posted `CtlMsg`. Fixture:
`d1 s :analog > note [:a4] > lpf 800 > gain 0.2`. Find the `800` site id
and its form_gen from the `eval-result`.

- `momentary {target: 1200, ramp_ms: 30}`:
  - one `CellRamp` per lpf cell, with `target = encode(1200)`,
    `frames = round(0.03 * sr)` and `release: false`;
  - the published site value is still 800;
  - after ticking 20 ms, `slot.get()` is 1200 and `slot.base()` is 800.
- `{target: null, ramp_ms: 1000}`:
  - a `CellRamp` with `target = encode(800)`, `frames = sr` and
    `release: true`;
  - after ticking past 1 s, the entry is gone, `slot.get()` is 800 and
    no further `CellRamp` is posted.
- `{target: null, ramp_ms: 0}` gives `frames = round(0.005 * sr)` (snap).
- A Reeval site (`{* 0.5 2}` form) or a Manual site gives
  `momentary-ineligible` and no `CtlMsg`.
- A stale form_gen on a drag gives no reply, no `CtlMsg` and no table
  change.
- **Release with the old id after a re-key** (LP-PLAN-R1-STALE-RELEASE (a)):
  1. Hold at 1200 (`momentary {id: N1, form_gen: G1, target: 1200}`).
  2. Re-eval the unchanged text; the entry re-keys to N2/G2 with
     `alias = N1`.
  3. Send `momentary {id: N1, form_gen: G1, target: null, ramp_ms: 1000}`.

  Expected:
  - a release `CellRamp` (`release: true`, `target = encode(800)`,
    `frames = sr`) is posted to the entry's current (N2) cells;
  - after ticking past 1 s, the entry is gone and `slot.get() == 800`.
- **Release after an edit invalidation** (case (b)):
  1. Hold at 1200.
  2. Send a `doc-changed` that touches the literal's span, so the site is
     marked invalid (`check_tweak` would return edit-invalidated).
  3. Send `momentary {id: N1, form_gen: G1, target: null, ramp_ms: 0}`.

  Expected: a release `CellRamp` with
  `frames = round(0.005 * sr)` targeting `encode(800)`, and the entry is
  removed after 5 ms.
- **A stale drag after a re-key is still dropped** (case (c)):
  1. Hold, then re-eval (re-keyed to N2/G2).
  2. Send `momentary {id: N1, form_gen: G1, target: 1500, ramp_ms: 30}`.

  Expected: no reply, no `CtlMsg`, and the entry keeps its target 1200
  (`slot.get()` stays 1200).
- **A release for an unknown id** (case (d)):
  `momentary {id: 9999, form_gen: 1, target: null, ramp_ms: 1000}` with no
  matching entry or alias gives no reply, no `CtlMsg` and no table change.
- The 17th concurrent entry gives `momentary-capacity`.
- **Re-eval, held, unchanged text:** the entry re-keys to the new id; the
  new slot's `base()` equals the literal (`migrate` did not see the
  momentary); a `momentary` with the new id is accepted.
- **Re-eval, releasing, literal edited from 800 to 600:** a re-issued
  `CellRamp` targets `encode(600)` with the remaining frames.
- **Binding-origin Direct site** (`var cutoff 800` with `> lpf cutoff`): a
  `CellRamp` is posted for the cell the var feeds.
- **Inst-default site:** a `CellRamp` is posted for the external cell, with
  the target encoded by `default_cell_value`.
- **Integer site** (an integer literal Direct site): only `frames = 0`
  ramps with integral targets.
- **Hush:** the table is empty, a drop ramp (`release: true, frames: 0`) is
  posted for each cell, and `slot.get() == slot.base()`.
- **`hush` evaluated as code** (an eval of the line `hush`): after the next
  tick the table is empty.

**`src/sched/tests/sched/ramps.rs`:**

- With no `CellRampAck`, the ramp is re-posted after `resend_ticks` with
  `frames` reduced by the elapsed frames; an ack stops the re-sends.
- With the browser tier, an unacked incarnation gives no post. A commit in
  that window uses the momentary override for the `Const` downgrade.
- The 33rd concurrent cell gives `Err(Capacity)`.

## Verification (exact commands; all must exit 0)

1. `CARGO_TERM_QUIET=true cargo build --all-targets`
2. `CARGO_TERM_QUIET=true cargo clippy --locked --all-targets -- -D warnings`
3. `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib -E 'test(/session::tests::(momentary|authority|publish|eval)|sched::tests::sched::(ramps|cells)|ns::tests/)'`
   passes with testsRun > 0.
4. `CARGO_TERM_QUIET=true cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
5. `rustfmt --edition 2021 --check src/session/momentary.rs src/session/authority.rs src/session/publish.rs src/ns/namespace.rs src/ns/evaluator.rs src/sched/cells.rs src/sched/ramps.rs src/session/tests/momentary.rs src/sched/tests/sched/ramps.rs`

Write logs to `tmp/live-perf/session-momentary/*.log`.

## Completion Criteria

- [ ] TASK-M1 to TASK-M4 are implemented, and every listed test passes.
- [ ] The tweak slot is never written by the momentary path (test
  asserted).
- [ ] Releases resolve by current id or the one-step alias and bypass
  staleness checks. Stale drags are still dropped silently. The four
  release and stale-drag tests pass.
- [ ] Existing authority, publish, eval, cells and ns tests pass unchanged.
- [ ] Verification 1-5 exit 0. Touched files are under 1000 lines. The
  Progress Log records the query-time and eager-dependent limitations.

## Progress Log

### Session: 2026-10-08 (plan authored)
**Tasks Completed**: plan authored (step 4).
**Notes**: Not started.
