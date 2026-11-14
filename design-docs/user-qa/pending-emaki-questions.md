# Pending: emaki extraction

Decisions for [`design-emaki.md`](../specs/design-emaki.md) (2026-10-31,
branch `wf/emaki`).

All items are open. None of them blocks implementation. Until the owner
answers, implementation follows the recommended option, which the design
already uses.

## EQ1. How to gate the keystroke-to-paint and scroll budgets of 16.7 ms

**Question.** Should "keystroke-to-paint p95 <= 16.7 ms" be gated on the raw
time from keydown to presented frame? The alternative is to gate on the
editor's own work and on next-frame presentation.

- (a) **Recommended.** Gate on the following, and record the raw keydown to
  present time for review:
  - input handler work + presenting frame work, p95 <= 16.7 ms;
  - at least 95% of keys presented on the first frame after the event.

  Scroll is gated on frame work p95 <= 16.7 ms, with the rAF interval
  recorded. The raw time includes the wait for vsync, up to 16.7 ms. The
  headless WebKit control page already shows an 18 ms frame interval, so the
  raw gate would fail even with zero editor work.
- (b) Gate on the raw keydown to present time. This is expected to fail in
  headless WebKit whatever the editor does.

## EQ2. Inline sequencer and scope widgets on by default

**Question.** Should inline widgets be on by default? The setting key is
`vactr.inlineWidgets`.

- (a) **Recommended.** On by default, which matches the request ("from now on
  vactr shows ..."). The existing e2e suites must still pass.
- (b) Off by default. The user opts in through the toggle.

## EQ3. Canvas text-dirty work budget

**Question.** The intake states "text work p95 <= 4 ms". The repository
gate is "<= 8 ms (target 4, recorded)". The Chromium baseline is already
4.3 ms.

- (a) **Recommended.** Keep the repository gate. Record the 4 ms target.
- (b) Make 4 ms a hard gate. The current baseline would fail it.

## EQ4. Anchor of the inline sequencer

**Question.** Where should a slot's step widget sit?

- (a) **Recommended.** Below the last line of the top-level form that contains
  the slot's earliest `playing.src` span. Fall back to that span's line when no
  form contains it.
- (b) Directly below the line of the earliest `src` span. This can split a
  multi-line form.

## EQ5. Inline oscilloscope sources

**Question.** Should bus analyzer units of kind scope also appear inline?

- (a) **Recommended.** No. Only `#@ editor` lines with an `oscilloscope`
  visualizer get an inline scope. Analyzer units have no source span on the
  wire, so they stay in the analyzer pane.
- (b) Yes. This needs a protocol change, which is out of scope for this run.

## EQ6. Vim mode inside emaki

**Question.** The intake recommended moving Vim into emaki as an optional
module. Should it move?

The current Vim core is not generic. It carries `vactr.*` action IDs, the
`:eval`, `:hush` and `:format` Ex commands, a vactr message and the
`mise-darwin` preset. Saved user keymaps reference those IDs, and nine test
files assert them.

- (a) **Recommended.** Keep `vim/**` in vactr for this run. emaki exposes
  only the generic seams it already has (modal key hook, cursor shape, hop
  labels). No IDs or tests change.
- (b) Move a generic Vim module into emaki in a follow-up. This needs a
  host-supplied action table so that the `vactr.*` IDs stay stable in
  vactr, and the tests that assert vactr IDs remain in vactr.

## EQ7. Parity blockers in the emaki/vim migration (2026-11-11, revised after DR-001)

**Context.** The accepted requirement is no user-visible behavior loss.
`design-emaki-vim-migration.md` section 7 lists APIs that emaki `9dba0a3`
lacks. emaki is read-only in this run, and vactr must not re-implement engine
logic. Without these APIs, parity is lost:

- **VMB1, document-boundary reset.** Insert or visual mode, `.`, and undo
  ledger state carry over across a file load. Confirmed. Blocks completion
  (parity check P1).
- **VMB2, `ex` status message.** `:w`, `:q`, `:wq` and `:x` lose the visible
  "vactr syncs the document live" message. Confirmed. Blocks completion
  (P3).
- **VMB3, meta-key routing.** Blocks completion only if P4 fails.
- **VMB4, exported `normalizeKey`.** Persisted or user-authored `cmdline`
  remaps stop working in the status prompt. Confirmed. Blocks completion
  (P2).

This question is about authorizing a parity reduction. It is separate from
push timing. `gitPush=false` applies in every option.

**Question.** Do you amend the scope to accept any of these losses?

- (a) **Default (applies when there is no answer).** No amendment. Parity
  stays required. The run reports `blocked`, leaves the partial migration
  uncommitted, and names the missing APIs and failing parity checks. To
  finish, emaki adds `resetDocument` (or `isDocumentBoundary`), an `ex`
  message return value and a `normalizeKey` export, the operator bumps the
  pin, and the run resumes.
- (b) Explicitly accept named losses, for example "accept VMB2 only". Only
  the losses you name may be committed locally, and the commit message lists
  them. The parity checks for losses you do not accept still gate
  completion.
- (c) Abandon the migration until emaki ships the APIs. Discard the partial
  work.

No option allows weakening, skipping or deleting an assertion.
