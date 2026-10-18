# Live performance integration with main (merge/live-perf)

**Status**: Implemented (2026-10-18, workflow session 438)
**Plans**: archived after the final integration review (workflow session
441) as `impl-plans/completed/live-perf-int-*.md` with
`live-perf-int-dispatch.json`. The legacy live-perf plans are in
`impl-plans/completed/live-perf-*.md`. Plan paths under `active/` below
record where the plans lived while this design was in progress.
**Branch**: `merge/live-perf`, at merge commit 9474491. This merges main
v0.1.1 (Vim mode, Vim keymap, hints, hop) with `wf/live-perf`.
**Baseline designs (unchanged; source of truth)**:
`design-live-performance.md` (D1-D11, sections 4-8) and the accepted plans
`impl-plans/active/live-perf-*.md` with `live-perf-dispatch.json`;
`design-vim-mode.md` and `design-vim-keymap-hints-hop.md`.

This addendum fixes the five merge regressions and the UF-1 style
regression, accepts the wave-1 work through independent review, and
finishes LP-EVIDENCE.
It does not redesign any accepted feature. Each change either:

- restores a contract that already exists on main, or
- reconciles two accepted designs where they meet.

Section 7 lists the one-line amendments this makes to other documents.

## 1. Scope

In scope:

1. The wire-tag collision between the song records and the new live-perf
   control records (I1).
2. The removed `hush` input option, which the Vim integration still passes
   (I2).
3. Stop and cut routing through the Vim keyboard fallback (I3).
4. Diagnosing and fixing the four LP-EVIDENCE behavior e2e checks (I4).
5. Diagnosing and fixing the missing playback onset after Run in
   e2e:examples (I5).
6. Reviewing the unreviewed session-349 work in commit 4ca8edc (I6).
7. Finishing LP-EVIDENCE: full gates, the section 12 record, archiving the
   plans and README updates (I7).
8. The `npm run test:style` regression UF-1 in the right pane (I8,
   section 10).
9. Independent review and acceptance of the wave-1 work committed at
   b327141 (LPI-WIRE, LPI-VIM-STOP, LPI-NS-INDEX) (I9, section 11).

Out of scope:

- New live-perf behavior, constants or thresholds.
- Changes to Vim behavior beyond section 4.
- Changes to song-mode semantics.
- Merging into main, and any physical-iPad validation (an existing stated
  limitation).

## 2. Evidence at 9474491 (2026-10-18; I8 and I9 at b327141)

| ID | Observation | Root cause |
|----|-------------|------------|
| I1 | `tests/song_resource_carriers.rs` `graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind` fails at line 807. It also fails on `wf/live-perf` alone. | **Confirmed.** LP-CONTRACT gave `CtlMsg::OutputStop` tag `0x1C` and `CtlMsg::CellRamp` tag `0x1D` (src/host/wire.rs:351-352). These are already the ring song-record tags `GRAPH = 0x1c` and `BEGIN = 0x1d` (src/dsp/ring/song.rs:7-8; `SLICE = 0x1e`). Song tests pin them (`tests/song_hosts.rs:227`, `tests/song_resource_carriers.rs:420,835`). The LP-CONTRACT pre-check grep (`'0x1C\|0x1D\|0x4B\|0x4C'`) is case-sensitive, so it missed the lowercase song constants. LP-ENGINE then worked around the overlap: `ByteInbox::next` tries an exact-length `CtlMsg::decode` before `song::decode` for song tags (src/dsp/ring.rs:526-530). A corrupted song GRAPH record whose second byte is 0 or 1 now decodes as `OutputStop`, and the rejection contract is lost. |
| I2 | `npm run check` reports three errors: `hush` does not exist in `InputOptions` (editor/src/code/mount.ts:179, test/canvas/input-vim-hook.test.ts:20, test/canvas/vim-input.test.ts:23). | **Confirmed.** LP-EDITOR-STOP replaced `KeyboardOptions.hush` with `stopAll`/`cut` (editor/src/code/keyboard.ts:76-77). The Vim integration from main still passes `hush`. |
| I3 | test/canvas/vim-mount.test.ts:111 "Linux x86_64 / MacIntel routes alternate shortcut fallback while Vim is on" fails: 0 messages instead of 1. | **Confirmed.** main's Vim keyboard fallback (design-vim-mode.md 3.3.1 P2, 4.14) accepts `metaKey \|\| ctrlKey`. live-perf's `KeyboardController` now calls `stopShortcut(event, isApplePlatform())`, which accepts only the platform Mod (editor/src/ui/stop-keys.ts). So `Meta-.` on Linux and `Ctrl-.` on Apple match neither the platform-strict app-level handler (editor/src/code/transport.ts:198-215) nor the fallback. The mount also passes the dead `hush` option. Separately, Vim's P2 pass-through (editor/src/code/vim/keys.ts:31) lists key `'.'` but not `'>'`, so `Mod-Shift-.` with the alternate modifier is not passed through on a US layout. |
| I4 | The behavior checks momentary-right-drag, momentary-two-finger, stop-gentle-shortcut and cut-shortcut fail in Chromium and WebKit (44/52). They had never been run. | **Not yet diagnosed.** All four checks share `loadLiveFixture`, which fills the textarea, clicks `.vact-run` and waits for `momentaryHit`, so a shared failure point is likely. Run normally starts audio (editor/src/code/mount.ts:146: `onEval` calls `transport.audio.start()`, and the Run click is a user gesture). So "audio never starts" is only a candidate. It is not an established cause and must not be recorded as one unless the diagnosis run shows it. One check-side defect is established: the checks hard-code `Meta+.`, which is correct only where `navigator.platform` is Apple. |
| I5 | e2e:examples `run-plays-moderately` fails with "no playback onset within 10 seconds". It passes on main. | **Not yet confirmed.** The failure record (tmp/examples-e2e/result.json) shows that audio **is** produced: pre-sink peak -3.05 dBFS, RMS -22.26 dBFS, eval status `ok`, audio `running`, `onsetCount: 0`. The silent sink counts an onset only when a 10 ms block at >= -40 dBFS RMS follows at least five consecutive 10 ms blocks at <= -50 dBFS (editor/test/e2e/silent-sink.mjs:64-73). So on the merged tree, from audio start, no 50 ms quiet window comes before a loud block. Either the output is not quiet before the first note, or the first note rises through the -50..-40 dB band. Static review rules out the `OutputStage` initial state (Running, gate 1.0), the `VarSlot` momentary default (`None`) and the `RampedCells` fall-through. The mechanism needs the diagnosis in 5.2. |
| I6 | Commit 4ca8edc contains the perf-hook accessors, the four behavior checks, and a "tweak/depgraph scaling attempt" in src/ns/{depgraph,evaluator,tweak}.rs with src/ns/tests/migrate_scale.rs. It has not been reviewed. Full vitest large-eval scaling now passes (ratio 2.61; it failed at 7.59-7.83 in session 349). | Needs review (section 6). |
| I8 | UF-1: `npm run test:style` fails on the merged tree and on 4ca8edc (`wf/live-perf`), passes on main 8866a43. Gap check findings: `button -> input` (gap -563.6, minimum 8) at 1440x900 and `button -> button` (gap -459.6) at 1180x820-coarse, Chromium and WebKit, with and without Vim (tmp/ui-style/after/report.json; baselines tmp/live-perf-int/reconcile/7-main-style.log exit 0, 9-wf-live-perf-style.log exit 1). | **Partly established.** The reconcile probe (tmp/live-perf-int/reconcile/3-gap-probe.log) at 1440x900 pairs the `.vact-section-toggle` "visuals" header button (rect y 385-413, x 864-1440, full width) with the `.pkg-proxy` URL input (y 382-410). `.pane-right` is `overflow: auto` (editor/src/app/app.css:352-355) and the package area is appended inside it (editor/src/pkg/mount.ts:72). LP-EDITOR-MOMENTARY prepends the `.vact-glide-setting` row to `.pane-right` (editor/src/bind/mount.ts:168, editor/src/bind/glide-setting.ts:20), which shifts all right-pane content down by one row. The proxy input therefore sits below the right pane's bottom edge, where it is scrolled out of view, and its unclipped rect falls inside the visuals header band. The gap check's `isVisible` (editor/test/style/ui-style.mjs:42-50) checks display, visibility and opacity only, not scroll clipping. Whether the 1180x820-coarse pair is the same mechanism, and whether the input is fully clipped, is confirmed by the S0 probe in section 10. A second, independent defect is visible in tmp/ui-style/after/chromium-1440x900@2x.png: the glide row has no horizontal inset (`padding: var(--vt-space-2) 0`, bind.css:154) and runs flush with the pane edges, while the sibling `.bind-panel`/`.bind-controls` rows use `var(--vt-space-3)` (bind.css:7). |
| I9 | b327141 contains the wave-1 implementations: `TAG_OUTPUT_STOP = 0x1F`, `TAG_CELL_RAMP = 0x20` (src/host/wire.rs:351-352); the song arm `song::GRAPH \| song::BEGIN \| song::SLICE => song::decode(b)` (src/dsp/ring.rs:526); `fallbackStopShortcut` (editor/src/ui/stop-keys.ts:20); mount.ts:179-180 `stopAll`/`cut` gated on `vim.enabled()`; `input.code === 'Period'` pass-through (editor/src/code/vim/keys.ts:31); src/ns/tests/migrate_index_equivalence.rs. None has had an independent review. LPI-VIM-STOP has no test-integrity or adversarial review. Line counts: ring.rs 928, wire.rs 928, contracts.rs 604, evaluator.rs 957. | Accepted only through section 11. |

## 3. Wire tags (I1)

**Decision W1.** The song-record tags are the earlier, test-pinned main
contract, and they keep their values. The two live-perf control records move
to free tags:

| Record | Direction | Old tag | New tag |
|--------|-----------|---------|---------|
| `CtlMsg::OutputStop {mode}` | to audio | `0x1C` | `0x1F` |
| `CtlMsg::CellRamp {cell, epoch, seq, target, frames, release}` | to audio | `0x1D` | `0x20` |
| `HostMsg::OutputState`, `HostMsg::CellRampAck` | from audio | `0x4B`, `0x4C` | unchanged; no collision (song ack is `0x49`) |

- The record bodies, field order, `CtlMsg::MAX_LEN` and the native
  `NativeRecord::Msg` path do not change. The native host passes typed
  messages, so only the browser byte path (`host/wire.rs` encode/decode and
  `ByteInbox`) is affected.
- Free tags are checked case-insensitively: `grep -rni '0x1f\|0x20' src/host
  src/dsp/ring.rs src/dsp/ring src/song`. Today it finds no wire use;
  `0x20` and `0x1f` appear only in unrelated sysex and pkg code.

**Decision W2.** `ByteInbox::next` goes back to main's dispatch. Song tags
decode only as song records:

- `song::GRAPH | song::BEGIN | song::SLICE => song::decode(b)`.
- The exact-length `CtlMsg`-first attempt is removed.
- `OutputStop` and `CellRamp` reach the default arm,
  `CtlMsg::decode(b) -> Record::Msg`, like every other plain control
  message.
- `ByteInbox::class` and `drop_held` stay unchanged.

**Decision W3.** One regression guard makes the tag sets provably disjoint.
It lives in `src/host/tests/contracts.rs` (537 lines), not in
tests/song_resource_carriers.rs (991 lines). It asserts:

- the first byte of encoded `OutputStop` and `CellRamp` records differs
  from the first byte of encoded song graph, begin and slice records
  (`ring::encode_song_graph_record`, `ring::encode_song_sample_begin` and
  `ring::encode_song_slice`);
- an encoded `OutputStop` and `CellRamp` pushed through `ByteInbox` come out
  as `Record::Msg` with equal fields.

The existing negative test at contracts.rs:216
(`CtlMsg::decode(&[0x1c, 9])` is `BadValue`) moves to the new `OutputStop`
tag (`[0x1f, 9]`). This is a mechanical update: the asserted meaning, an
unknown mode giving `BadValue`, stays the same.

**Acceptance.**

- `graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind`
  passes, together with the rest of tests/song_resource_carriers.rs and
  tests/song_hosts.rs.
- The LP-CONTRACT round-trip tests pass with the new tags.
- src/dsp/ring.rs gets shorter; src/host/wire.rs stays at 928 lines or
  fewer.
- No song test changes.

## 4. Stop and cut under Vim (I2, I3)

The accepted mapping stays as it is (design-live-performance.md D2, D10, 6):

- `Mod-.` sends `stop-all` (gentle).
- `Mod-Shift-.` sends `hush` (cut).
- One platform-strict, app-level window keydown handler in the capture
  phase owns both shortcuts. It calls `preventDefault` and
  `stopPropagation`, so a platform-Mod press never reaches the textarea,
  Vim or the keymaps.

What remains for Vim is the **fallback path**. Before the merge, Vim's P2
fallback routed `Ctrl-.` on Apple and `Meta-.` on Windows/Linux to `hush`.

**Decision V1: KeyboardController fallback matching.** The fallback keeps
main's P2 modifier tolerance (`metaKey || ctrlKey`; main's rule was
`mod && key === '.'`). It adds the live-perf Shift form and an Alt
exclusion, which makes it a slightly stricter superset of main's rule. A
keydown is a stop or cut fallback when:

- `metaKey || ctrlKey` is set, `altKey` is not;
- the key is a period: `code === 'Period'`, or `code === ''` and `key` is
  `'.'`, or `key` is `'>'` with Shift.

Shift selects `cut`; no Shift selects `stopAll`. The app-level handler
keeps `stopShortcut(event, apple)` with platform-strict matching (D10). The
fallback is a second, modifier-tolerant use of the same period and Shift
rule, in editor/src/ui/stop-keys.ts.

**Decision V2: mount wiring.** editor/src/code/mount.ts replaces
`hush: () => { if (vim.enabled()) evalCtl.hush(); }` with:

- `stopAll: () => { if (vim.enabled()) transport.stopAll(); }`
- `cut: () => { if (vim.enabled()) transport.hush(); }`

`transport` is the `TransportBar` created earlier in the same mount
(mount.ts:143). Its `stopAll()` and `hush()` send exactly one message and
clear highlights through `onHush`. The `vim.enabled()` gate keeps main's
behavior: with Vim off, the alternate modifier does nothing, exactly as on
main.

There is no double send:

- A platform-Mod press is consumed by the capture-phase window handler.
- An alternate-modifier press is not matched by that handler, so only the
  fallback fires.

**Decision V3: Vim P2 pass-through.** `isPassThrough`
(editor/src/code/vim/keys.ts:31) also passes `mod && code === 'Period'`.
This makes `Mod-Shift-.` (key `'>'`) reach the fallback in every Vim mode,
the same way `Mod-.` already does. Nothing else in the pass-through set
changes.

**Unchanged.** The following still send `hush` (cut) through
`EvalController.hush()`:

- the Vim `:hush` ex command;
- the `vactr.hush` action id;
- keymap presets that bind `vactr.hush`.

This is consistent with D2: the language `hush` and the client `hush` both
mean cut.

**Tests.**

- test/canvas/input-vim-hook.test.ts and test/canvas/vim-input.test.ts:
  their `InputController` rigs pass `stopAll` and `cut` spies instead of
  `hush`. This is mechanical; no assertion uses the removed option.
- test/canvas/vim-mount.test.ts:111 is **intentionally changed**. The
  reason is that merged behavior differs by accepted design D2/D10: the
  unshifted alternate-modifier period now means gentle stop. For each
  platform, the case asserts:
  - two `eval` messages;
  - `Mod-.` with the alternate modifier gives exactly one `stop-all`;
  - the alternate modifier with `key: '>'`, `code: 'Period'` and Shift
    gives exactly one `hush`;
  - no extra messages of either kind.

  No assertion is removed.
- test/ui/stop-keys.test.ts gains fallback-rule cases for both
  modifiers, Shift `'>'` and Alt rejection.
- A vim keys test covers the `code === 'Period'` pass-through.

**Acceptance.**

- `npm run check` reports zero errors.
- The vim-mount, vim-input, input-vim-hook, stop-keys, transport and eval
  vitest pass.
- Vim e2e and keymap e2e stay green.

## 5. Behavior and playback diagnosis (I4, I5)

Both are diagnosed with real runs before anything is fixed. The fix goes
where the root cause is.

- **Rule D-a.** A check, harness or fixture defect is fixed in the check.
  Examples: the harness never starts audio, a platform-wrong modifier, or a
  wait that contradicts a design constant.
- **Rule D-b.** A product behavior that deviates from
  design-live-performance.md 4-6, or from main's playback, is fixed in
  product code.
- **Rule D-c.** No threshold, design constant, silent-sink rule or
  examples.mjs assertion is relaxed. No check is deleted.

### 5.1 Behavior checks (I4)

**Diagnosis run.**

1. Do a fresh build: `mise run build-wasm-release`, then
   `VACTR_REQUIRE_SESSION_ABI=1 npm run build`. The editor build
   embeds `target/wasm32-unknown-unknown/release/vactr.wasm`.
2. Under the measurement lock:
   `npm run e2e -- --browser all --profile behavior --run-id live-perf-int-diag --out ../tmp/live-perf-int/e2e/diag`.
   Capture stdout and stderr to a log. Without `--write-evidence`, the
   runner writes no files; the result JSON is printed on stdout.
3. Classify each failing check from its `detail` in the stdout JSON
   (`summary.browsers[].checks[]`).

**Required check-side corrections** (rule D-a):

- **B1.** `loadLiveFixture` makes sure audio is running before it waits
  for live state. If `.vact-audio-state[data-state]` is not `running`
  after Run, it clicks `.vact-audio` and waits up to 10 s for `running`,
  as examples.mjs:84-85 does. This is an idempotent precondition, not a
  presumed root cause: Run normally starts audio (mount.ts:146). The
  context already has the silent sink, so this stays silent (post-sink
  peak 0).
- **B2.** The stop and cut presses use the platform Mod derived in the
  page from `navigator.platform`, with the app's `/Mac|iP/` rule, instead
  of a hard-coded `Meta`.
- **B3.** Each of the four checks leaves the engine stopped. On a failure
  path it clicks `.vact-hush` before rethrowing, so later checks (the
  dispose-ledger check and the Vim checks) start from silence.

**Decision table** for anything still failing after B1-B3:

| Failure point | Owner |
|---------------|-------|
| `momentaryHit` never true after Run with audio running and eval `ok` | product: site table or Direct-tier eligibility (design-live-performance.md 5.1) |
| label, document or context-menu assertions | product, if the behavior contradicts 5.2, 5.3 or 5.9; otherwise the check |
| `outputHistory` lacks `draining` then `idle` (or `idle` after a cut) with audio running | product: engine `OutputState` reporting or session `TransportSample.output` (4.2, 4.5) |
| timing waits | the check may follow the design constants only: Shift snap within 10 frames, glide 1000 ms, drain cap 15 s |

### 5.2 Playback onset after Run (I5)

This is a product regression (the intake states it passes on main). The fix
goes in the engine, session or editor runtime, never in examples.mjs or
silent-sink.mjs.

**Diagnosis procedure.**

1. Write a temporary probe, `tmp/live-perf-int/diag/onset-probe.mjs`. It is
   an artifact and is not committed. It:
   - installs the same silent sink;
   - follows examples.mjs steps 1-2 (open showcase, start audio, Run);
   - records the pre-sink 10 ms block RMS and peak series, from the first
     destination connect to 5 s after Run;
   - records the transport output history and the eval status.

   It runs under the measurement lock, against a fresh dist:
   `mise run build-wasm-release`, then
   `VACTR_REQUIRE_SESSION_ABI=1 npm run build`. Re-run e2e:examples on
   the same fresh dist first. A pass there means the failure was a stale
   build, and that is recorded.
2. Classify the series:
   - **(a)** The output is not quiet before Run (blocks above -50 dBFS
     between audio start and Run).
   - **(b)** The first sound rises through -50..-40 dBFS: a soft or late
     first note, or dropped first events.
   - **(c)** Some other shape.
3. Locate the mechanism by comparing against main's behavior:
   - Use a read-only export of main into
     `tmp/live-perf-int/main-ref` (`git archive main | tar -x -C ...`),
     built there and probed the same way. This never touches main's
     checkout or other worktrees.
   - Or bisect on the merged tree with throwaway local reverts of the
     live-perf audio-path changes, one at a time. Candidates, in order:
     1. the control-intake `stop_acks_full()` gate (src/dsp/engine.rs);
     2. `OutputStage` gating and `skips_effects`;
     3. the `RampedCells` read path and the `CellRamps` sizing from
        `cfg.analysis_cells`;
     4. `hold_retiring`;
     5. the D3 `Release::Natural` extension (src/dsp/voice.rs:938);
     6. the session `stage_now` and `revoke_with` paths.

     Reverts are never committed.
4. The fix must keep every accepted live-perf semantic (gentle drain to
   idle, cut fade and clear, D3). If the only fix would change an accepted
   semantic, stop. Record it in
   `design-docs/user-qa/pending-live-perf-integration-questions.md` and
   report it as blocked rather than redesign.

**Acceptance.**

- e2e:examples passes 8/8.
- A deterministic regression test reproduces the root cause and fails
  before the fix. It is a Rust DSP or session test with `NativeRig` or
  `BrowserRig`, or a vitest, as fits the mechanism.
- The DSP tests 8.1(1)-(4) of design-live-performance.md stay green.

## 6. Session-349 review (I6)

Commit 4ca8edc is adopted only after these checks pass:

- **Perf hooks** (editor/src/code/perf-hook.ts, mount.ts). They are
  active only with `?perf=1`. Momentary label sampling is guarded by
  `perfEnabled`. The mount.test.ts row asserts plain data. Keep them.
- **Behavior checks** (editor/test/e2e/behavior.mjs). Apply B1-B3 and the
  results of 5.1. Keep `dispose-ledger` after the four checks.
- **Name-space index** (src/ns/depgraph.rs `current_by_span`,
  src/ns/tweak.rs `by_generation`/`by_span_start`, src/ns/evaluator.rs
  `migrate`). This is kept because full vitest large-eval scaling depends
  on it. It is a pure performance change, so its results must equal the
  linear lookups it replaces:
  - `migrate` picks the same predecessor (the first by position for
    binding id or name, and by `(origin, index, ty)`).
  - `sites_of` orders by `(index, id)`.
  - `retire` and `insert` keep both indexes consistent.
  - `current_gen_at_span` equals the old
    `graph.ids().find_map(is_current && span ==)`.

  The reviewer must settle one divergence candidate. The old scan treated
  a form with an empty write set as current even if it was never claimed.
  The index registers forms only in `claim`. Either show that such a form
  cannot reach the migration lookup, or register vacuously-current forms
  at `add`. A differential test runs the old and new lookups over the
  same sequence of evaluations, including an empty-write form, and
  asserts identical results. The `#[cfg(test)]` instrumentation
  (`migration_lookup_work`) must not change production behavior.
  src/ns/evaluator.rs is 957 lines and must stay under 1000.

## 7. Closeout of LP-EVIDENCE (I7) and document amendments

The LP-EVIDENCE plan's TASK-V3 to TASK-V5 run as specified, with these
integration adjustments:

- **Gates.** The union of the intake gates and LP-EVIDENCE TASK-V3:
  - `CARGO_TERM_QUIET=true cargo build --all-targets`;
  - `cargo fmt --check`;
  - `cargo clippy --all-targets -- -D warnings`;
  - the wasm debug build and `mise run build-wasm-release`;
  - `cargo test --manifest-path editor/src-tauri/Cargo.toml` (this also
    covers the plan's tauri `cargo check`);
  - `npm run check`;
  - full vitest (lock);
  - `npm run test:style`;
  - `npm run test:perf`;
  - `VACTR_REQUIRE_SESSION_ABI=1 npm run build`, immediately followed by
    `npm run e2e -- --browser all --profile behavior --run-id live-perf-int-001 --out ../tmp/live-perf-int/e2e/live-perf-int-001`
    (lock);
  - the build again, immediately followed by `npm run e2e:examples`
    (lock);
  - `cargo nextest run --no-fail-fast` (lock, timeout 2400 s or more).

  `mise run build-wasm-release` runs before each editor build, because
  the build embeds the release wasm. Heavy gates run one at a time, and
  each logs to
  `tmp/live-perf-int/gates/<n>-<name>.log`. There is no `--write-evidence`
  run, and the canvas `run-001` evidence stays untouched.
- **Section 12.** design-live-performance.md gains exactly one
  "## 12. Verification record (integration, workflow session 438)". The
  LPI-CLOSEOUT plan and the manifest use this same heading string. It
  records:
  - each gate's command, exit status, tests run and passed, and log path;
  - the four behavior results per browser;
  - the e2e output path;
  - the integration fixes I1-I6 and I8, each with a one-line pointer to
    this document, quoting only established root causes;
  - the wave-1 acceptance decisions of section 11 (reviewer, verdict and
    any repair commit);
  - the stated limitation: synthetic multi-touch only, physical iPad check
    pending.
- **Archive.** Set each live-perf plan's status to Completed, with a final
  Progress Log entry. Move the seven plans and live-perf-dispatch.json from
  impl-plans/active/ to impl-plans/completed/, one file at a time.
  The live-perf-int plans and their manifest stay in impl-plans/active/
  while this workflow run references them (the precedent is the
  vim-keymap dispatch note in impl-plans/README.md). After the final
  integration review accepts every LPI plan, the workflow's integration
  step in this same run sets each LPI plan to Completed and moves the
  seven `live-perf-int-*.md` plans and live-perf-int-dispatch.json to
  impl-plans/completed/, one file at a time, in the integration commit.
  impl-plans/README.md then links only completed paths for both plan
  sets. In
  impl-plans/README.md, the "Live performance controls" section becomes
  Completed and links to the completed paths and this addendum.
- **README.md.**
  - The transport list: stop is gentle (effect tails ring out), the
    `Mod-.` stop button; cut (hush) is `Mod-Shift-.`.
  - A short "Live performance" paragraph: right-drag or two-finger
    momentary tweak, the glide setting, and Shift to snap.
  - The Vim "Live-coding shortcuts" bullet (README.md:280) names
    `Mod-.` (stop) and `Mod-Shift-.` (cut).
- **Commit.** The workflow's integration step commits on
  `merge/live-perf`, with no tool attribution. Merging into main is out of
  scope.

**Amendments to other documents** (one line each; no content is deleted):

- design-live-performance.md, in the header: a pointer to this addendum.
- design-vim-mode.md 4.13 (`:hush` row) and 4.14 (shortcut matrix): an
  amendment note. `Mod-.` is now gentle `stop-all`, owned by the app-level
  handler. `Mod-Shift-.` and `:hush` cut. The P2 fallbacks route the same
  pair with Vim on.
- architecture.md decision log: one row for this integration.
- impl-plans/active/live-perf-contract.md and live-perf-engine.md
  (archived with the rest): a Progress Log note recording the tag
  reassignment W1 and the removal of the exact-length workaround W2.

## 8. Plan decomposition

Plans use the prefix `impl-plans/active/live-perf-int-`. Every plan
declares these artifactRoots, identically in its plan file and in its
manifest entry. A bare `tmp` or `tmp/` root is never declared, so the
fanout evidence directories are never inside an artifact root:

- `target`
- `tree-sitter-vact/tree-sitter-vact.wasm`
- `editor/node_modules`
- `tmp/live-perf-int/<plan>`, where `<plan>` is the plan's short name
  (`wire`, `vim-stop`, `ns-index`, `playback`, `style`, `behavior`,
  `closeout`)
- plans that build the editor or run Playwright also declare
  `editor/dist` and the specific e2e output directories
  `tmp/canvas-cutover`, `tmp/code-render`, `tmp/canvas`,
  `tmp/examples-e2e` and `tmp/ui-style`; LPI-PLAYBACK adds
  `tmp/live-perf-int/main-ref` and `tmp/live-perf-int/main-ref-target`;
  LPI-CLOSEOUT adds `editor/src-tauri/target`

| Plan | Covers | Depends on | Wave |
|------|--------|------------|------|
| live-perf-int-wire | I1 (section 3); review-and-repair rerun (section 11) | none | 1 |
| live-perf-int-vim-stop | I2, I3 (section 4); review-and-repair rerun with test-integrity and adversarial review (section 11) | none | 1 |
| live-perf-int-ns-index | I6 name-space index part (section 6); review-and-repair rerun (section 11) | none | 1 |
| live-perf-int-playback | I5 (5.2) | all wave-1 plans (it builds the dist and the wasm from the combined tree) | 2 |
| live-perf-int-style | I8 (section 10) | all wave-1 plans (its editor build includes the reviewed wave-1 editor code) and playback (serialization edge only) | 3 |
| live-perf-int-behavior | I4 (5.1), the I6 perf-hook and check part | playback and style (its e2e run needs the fixed product and the fixed right pane; it shares editor/dist and the lock) | 4 |
| live-perf-int-closeout | I7 (section 7) | all of the above, including style | 5 |

LPI-PLAYBACK and LPI-STYLE have disjoint source writePaths, and LPI-STYLE
uses none of LPI-PLAYBACK's content. Both rebuild editor/dist, write the
e2e output roots and take the measurement lock, so they must not run at
the same time. Native fanout starts every dependency-ready plan of a wave
at once, so the serialization is an explicit `dependsOn` edge
(LPI-STYLE after LPI-PLAYBACK), not only a concurrency note. The
manifest's `executionModel` states the reason.

The manifest is impl-plans/active/live-perf-int-dispatch.json. It is
revised in place: same `taskId` `live-perf-int`, same `evidenceRoot`
`tmp/live-perf-int`, a new LPI-STYLE entry, LPI-STYLE added to the
`dependsOn` of LPI-BEHAVIOR and LPI-CLOSEOUT, and `workflowExecutionId`
and `originalHead` updated to this run (session-438, b327141). No second
manifest is created.

## 9. Risks

- **Active plans on the same paths.** song-mode-live-isolation and
  modular-live-input also touch the ring and record paths. W1/W2 restore
  main's song decoding and only renumber live-perf tags, so those plans'
  song contract stays valid.
- **Shared measurement lock.** Other worktrees contend for the lock, so
  timing-sensitive suites (large-eval scaling, perf) may need a re-run.
  A re-run is recorded as such, never as a relaxed threshold.
- **I5 may touch accepted engine code.** Section 5.2 step 4 bounds this:
  the fix keeps every accepted semantic, or the plan stops with a user QA
  entry.
- **The I4 hypothesis may be wrong** (feature defects behind the
  check-side ones). The decision table in 5.1 assigns ownership, so this
  cannot stall.
- **The UF-1 fix touches a gate.** S2 in section 10 excludes from gap
  pairing only a non-probe control that is mostly clipped out by an
  overflow ancestor (not painted). It is allowed only on the S0
  classification (a). It keeps every threshold and every per-control
  check, keeps painted pairs checked in the same container and across
  containers, and carries mutation evidence that both kinds of painted
  overlap still fail.
- **The right-pane fix can shift other layouts.** S1 changes only the
  glide row's own padding and width. LPI-BEHAVIOR (canvas, Vim and keymap
  e2e) and LPI-CLOSEOUT (all gates) run after LPI-STYLE and catch a
  shift.
- **A wave-1 review finding may need a change outside the plan's
  writePaths.** Section 11 R5 treats that like any other plan: stop and
  report a scope amendment, except mechanical test or fixture updates.

## 10. Right-pane style regression (I8, UF-1)

The glide setting is accepted LP-EDITOR-MOMENTARY behavior
(design-live-performance.md D6 and line 401: a "momentary glide" number
field with an adjacent range in the controls pane, stored under
`vactr.momentary.glideMs`). Its position as
the first row of the right pane (editor/src/bind/mount.ts:168) stays as it
is. This section fixes only how the row is styled and how the regression
is classified.

**S0. Diagnosis (required, before any change).** A temporary probe,
`tmp/live-perf-int/style/probe/style-probe.mjs` (an artifact, not
committed; modelled on tmp/live-perf-int/reconcile/probe/gap-probe.mjs),
runs under the measurement lock against a fresh dist
(`mise run build-wasm-release`, then
`VACTR_REQUIRE_SESSION_ABI=1 npm run build`). It uses the same page
setup, viewports (1440x900, 1180x820-coarse) and engines (Chromium,
WebKit) as editor/test/style/ui-style.mjs. For each failing gap pair it
records:

- both controls' rects, classes and parents;
- for each control, its nearest ancestor whose computed `overflow-x` or
  `overflow-y` is not `visible` (its clipping container), that
  container's rect, `clientHeight`, `scrollHeight` and `scrollTop`;
- the fraction of each control's height that lies inside its clipping
  container's rect.

It classifies each pair:

- **(a)** The two controls have different clipping containers, and one
  of them lies at least half outside its container's rect (it is scrolled
  out of view and not painted where the check measures it).
- **(b)** Both controls are painted in the overlapping band (a real
  overlap a user sees).

The expected outcome from the I8 evidence is (a) for the 1440x900 pair.
The 1180x820-coarse `button -> button` pair is classified the same way,
not assumed.

**S1. Glide row styling (always applied; product, bind.css).** The
screenshot shows a real spacing defect independent of the gap finding:

- `.vact-glide-setting` gets the same horizontal inset as the sibling
  right-pane rows: `padding: var(--vt-space-2) var(--vt-space-3)`, as in
  `.bind-panel, .bind-controls` (bind.css:7). The label no longer starts
  at the pane edge and the range no longer runs to it.
- The row stays solid, flat and square (no radius, no native look); the
  coarse-pointer 36 px minimum control height stays.
- The gaps between title, number input and range meet the test:style
  minimum (8 px between controls).
- editor/src/bind/glide-setting.ts changes only if the class or element
  structure must change for this; the DOM contract tested by
  editor/test/bind/glide-setting.test.ts (labelled number and range,
  clamp, round, persist, `.vact-glide-setting`) stays.

**S2. Painted-visibility rule for gap pairing (only on S0
classification (a); test, editor/test/style/ui-style.mjs).** The gap
check pairs controls that sit side by side in one painted row. The I8
defect is that `isVisible` (ui-style.mjs:42-50) treats a control that is
scrolled out of view as visible, so its unpainted rect can be paired
with a painted control that happens to share the band. S2 fixes exactly
that, and nothing broader:

- **Rule.** A new predicate is used only in gap-candidate selection, in
  the generic pairing loop (ui-style.mjs:111-122). A control is excluded
  from gap pairing when it is mostly clipped out. That means: intersect
  its rect with the rect of every ancestor whose computed `overflow-x`
  or `overflow-y` is not `visible`; if the remaining painted part is
  less than half of the control's height, or less than half of its
  width, the control is excluded. This is the same half fraction S0 uses
  for classification (a). Conservative guard: a control that has
  computed `position` `fixed` or `absolute`, or has such an ancestor, is
  never excluded. Such a control's painting can escape ancestor clipping
  (for example the document picker popover), so it stays checked.
- **Painted pairs stay checked, whatever their container.** Two painted
  controls in different clipping containers (for example `.pane-code`,
  app.css:337-342, and `.pane-right`, app.css:352-355, or a right-pane
  control and a `.pane-section-header` toggle) are still paired and
  gap-checked exactly as today. Container identity is never a reason to
  skip a pair.
- **Probe fixture unchanged.** `[data-style-probe]` controls
  (`inProbe === true`) are never excluded by the rule. Their pairs are
  measured exactly as today, because their gaps relative to each other do
  not depend on scroll clipping.
- **Everything else unchanged.** `isVisible` itself, the explicit
  `gapCheck` calls (song-apply, sample map, bank), the gap minimums 4 and
  8, viewports, engines, and the radius, fill, border, coarse-height,
  native-fill and proxy checks stay as they are. Every control is still
  inspected by every per-control check. No check is deleted.
- This is an intentional test change for a merged-behavior difference:
  the accepted glide row makes the right pane overflow at the two tested
  viewports, which main's content did not. The reason is recorded in the
  LPI-STYLE plan.
- Mutation evidence (in `mutationEvidence`, never in the gating list):
  1. with S2 locally reverted, test:style fails with the UF-1 findings;
  2. a temporary fixture with two painted, overlapping controls inside
     one container (for example the existing `[data-style-probe]` fixture
     with a negative margin) still fails the gap check with S2 applied;
  3. a temporary fixture with two painted, overlapping controls in
     different clipping containers (for example a control positioned
     inside `.pane-code` so it overlaps a painted `.pane-right` control
     in the same band, or a painted right-pane control overlapping a
     section-header toggle) still fails the gap check with S2 applied.

**On classification (b)**, S2 is not applied. The overlap is a product
layout defect, fixed in bind.css or glide-setting.ts. If the fix needs a
file outside the LPI-STYLE writePaths, the plan stops and reports a scope
amendment.

**Forbidden fixes.** Resizing or reshaping the glide row only so the
clipped proxy input leaves the header band (a pixel-coincidence fix);
moving or hiding the glide row; changing viewports or thresholds; removing
the gap check or the proxy check.

**LPI-STYLE writePaths.** editor/src/bind/glide-setting.ts,
editor/src/bind/bind.css, editor/test/bind/* (glide-setting.test.ts),
editor/test/style/ui-style.mjs (S2 only), the plan file, and
tmp/live-perf-int/style. Artifact roots per section 8, including
`editor/dist` and `tmp/ui-style`.

**Acceptance.**

- The S0 probe output and classification per pair are in the plan's
  Progress Log.
- `npm run test:style` exits 0 ("Style assertions passed for 16
  engine/viewport combinations"), Chromium and WebKit, with and without
  Vim, keymap and hints, immediately after
  `VACTR_REQUIRE_SESSION_ABI=1 npm run build`.
- The 1440x900 screenshot shows the glide row inset like the sibling
  rows.
- `npm run check` reports zero errors; `vitest run test/bind` passes.
- No touched file reaches 1000 lines.

## 11. Wave-1 acceptance (I9)

The wave-1 code at b327141 implements sections 3, 4 and 6 but is not yet
accepted. LPI-WIRE, LPI-VIM-STOP and LPI-NS-INDEX stay remaining plans and
run as **review-and-repair reruns**. Their status becomes
"In Progress (implemented at b327141; review-and-repair rerun pending)".
A rerun does not re-implement. It:

- **R1. Checks the deliverables.** The committed code matches the plan's
  deliverables and this design's sections 3, 4 or 6, file by file.
- **R2. Verifies.** It runs the plan's verification commands on the
  current tree and records each in the exact verification-record shape,
  with logs under `tmp/live-perf-int/<plan>/rerun-438/`. Full nextest or
  vitest runs take the measurement lock.
- **R3. Records mutation evidence.** Local, never committed:
  - LPI-WIRE: the CtlMsg-first song arm restored makes
    `graph_and_begin_reject_every_truncation_trailing_bytes_and_wrong_kind`
    fail; a colliding tag makes the disjointness guard fail.
  - LPI-VIM-STOP: `fallbackStopShortcut` made platform-strict makes both
    vim-mount alternate-shortcut cases fail; the `code === 'Period'`
    pass-through removed makes the vim keys test fail.
  - LPI-NS-INDEX: a perturbed index lookup makes
    migrate_index_equivalence fail.
- **R4. Is independently reviewed.** A read-only reviewer that did not
  author the commit reviews correctness against the design and the
  1000-line rule. For LPI-VIM-STOP the review also includes:
  - **test-integrity review:** every changed test file in b327141
    (vim-mount, vim-input, input-vim-hook, stop-keys, vim keys tests).
    The vim-mount change is the intentional one of section 4 and removes
    no assertion; the vim-input and input-vim-hook changes are mechanical
    option renames; the new cases cover both alternate modifiers, Shift
    `'>'`, Alt rejection and the `code === 'Period'` pass-through; no
    test was weakened to pass.
  - **adversarial review** of the V1-V3 edge cases: no double send for a
    platform-Mod press with Vim on; the alternate modifier does nothing
    with Vim off; Alt-period is ignored; `Mod-Shift-.` passes through in
    every Vim mode; `:hush`, `vactr.hush` and keymap bindings still cut.
- **R5. Repairs only findings.** Every high or mid finding is repaired
  inside the plan's writePaths, with a failing-first test for a behavior
  defect, and is listed in `addressedFeedback` with status repaired. A
  finding that needs a change outside writePaths stops the plan (scope
  amendment), except mechanical test or fixture updates.
- **R6. Accepts.** The plan becomes Completed only with the reviewer's
  verdict recorded in its Progress Log. For LPI-VIM-STOP this includes
  both the test-integrity and the adversarial verdicts.

Wave 2 (LPI-PLAYBACK) starts only after all three wave-1 plans are
accepted.
