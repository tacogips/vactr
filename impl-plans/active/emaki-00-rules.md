# emaki plans: common execution rules (read-only for every worker)

**Applies to**: every plan in `impl-plans/active/emaki-dispatch.json`.
**Design**: `design-docs/specs/design-emaki.md`, accepted by review comm-011214
in workflow session 586. Open user questions are in
`design-docs/user-qa/pending-emaki-questions.md`. Workers apply the
recommended defaults EQ1-EQ6.
**Branch**: `wf/emaki`, one shared working directory. There are no private
branches and no worktrees.

## Ownership and drift

1. Edit only paths listed in your plan's `writePaths`. You may also edit
   `sharedPaths`, but only for the purpose stated in the plan's
   `sharedPathNotes`. Every other path is read-only.
2. Read each file fresh right before you edit it. Record
   `shasum -a 256 <file>` before and after each edit in your Progress Log
   (or in `tmp/emaki/<planId>/sha256.txt`, cited from the log).
3. Before the first edit of a file, write one sentence of intent to
   `tmp/emaki/<planId>/intent.txt`. This is your immutable intent snapshot.
4. Before your final verification, rerun `shasum -a 256` on every file you
   edited. If a hash differs from your recorded post-edit hash and you did
   not change the file, another worker drifted it. Stop, record the drift,
   and do not repair it. Repair happens serially after the wave joins.
5. If a fix is needed outside your writePaths, stop and report a scope
   amendment. Do not edit the file. The one exception is a mechanical
   import-specifier update in a test that is already in your writePaths.

## Commands and evidence

6. Run every command in the foreground, never with `&` or `nohup`. Log each
   one like this: `cmd > tmp/emaki/<planId>/<n>-<name>.log 2>&1; echo "exit=$?" >> <log>`.
   Do not name a shell variable `status` (zsh reserves it).
7. Heavy runs are serialized by the shared measurement lock. They are:
   - full `vitest` in `editor/`;
   - `npm run build` in `emaki/` and in `editor/`. These write the shared
     `emaki/dist` and `editor/dist`, so parallel plans would race;
   - any Playwright run (`npm run e2e`, `npm run e2e:examples`,
     `npm run test:style`, `emaki npm run perf`, `emaki npm run e2e`);
   - `mise run build-wasm-release`.

   Acquire the lock before a heavy run and remove it afterwards, also on
   failure:
   - acquire: `until mkdir /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock 2>/dev/null; do sleep 30; done; echo <planId> > /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock/owner`
   - release: `rm -rf /Users/taco/gits/tacogips/vactr-worktrees/.measure-lock`

   Never kill processes you did not start.
8. Rebuild right before any vactr e2e run:
   - `export CARGO_TERM_QUIET=true; cargo build --lib --target wasm32-unknown-unknown --no-default-features --features host-wasm`
   - `mise run build-wasm-release`
   - `cd editor && VACTR_REQUIRE_SESSION_ABI=1 npm run build`
9. Install packages only with `npm ci --prefer-offline` or
   `npm install --prefer-offline`. emaki may depend only on these exact
   versions: `@codemirror/state` 6.7.6, `@codemirror/commands` 6.11.1,
   `typescript` 7.0.2, `vite` 8.3.1, `vitest` 5.0.2, `jsdom` 30.1.1,
   `playwright` 1.62.1.
10. Use this verification record shape exactly:
    `{command, exitStatus: 0, testsRun, testsPassed, failureCount: 0, outcome: "passed", log, notes}`.
    - Planted negative controls and mutations go only in `mutationEvidence`.
      Restore byte-for-byte afterwards and record the matching sha256.
    - Repaired review findings go only in `addressedFeedback` with status
      `repaired`.
    - An incomplete log (no `exit=` line) is not a passing check.
11. Codex cannot bind localhost or launch browsers. When a Playwright
    command cannot run inside the sandbox, record it as
    `blocked-in-sandbox`, with the exact command. The orchestrator runs it
    outside the sandbox and appends the evidence. Never claim it passed.

## Wave-join gate and scoped per-plan gates (review EMK-R5-02)

12a. **Wave-join gate.** The orchestrator runs this serially, not a worker.
     It runs after every plan of wave n has reported, and before any plan
     of wave n+1 starts. Under the lock (rule 7), with logs in
     `tmp/emaki/wave-<n>-join/`:
     - `cd emaki && npm run check`
     - `cd emaki && ./node_modules/.bin/vitest run`
     - `cd emaki && npm run build`
     - from wave 3 on, also `cd editor && npm run check`

     Wave-specific additions:

     | Wave | Extra commands | Where the evidence goes |
     |------|----------------|-------------------------|
     | 3 | `cd editor && VACTR_WASM=../target/wasm32-unknown-unknown/debug/deps/vactr.wasm ./node_modules/.bin/vitest run` (EMK-VAC-SWITCH is the only editor writer in wave 3, so its own full-suite run is attributable; the join repeats it on the joined tree) | `tmp/emaki/wave-3-join/` |
     | 4 | `cd emaki && npm run e2e -- --browser all --out ../tmp/emaki/wave-4-join/e2e` (G4), `cd emaki && node scripts/smoke.mjs --browser all --out ../tmp/emaki/wave-4-join/smoke`, and `test -f emaki/dist/json-10k/index.html && test -f emaki/dist/diff/index.html && test -f emaki/dist/plugin-showcase/index.html`. These Playwright runs happen outside the Codex sandbox (rule 11) | appended by the orchestrator to the Progress Logs of EMK-DIFF-DEMO (e2e), EMK-SHOWCASE (smoke) and EMK-JSON-DEMO (dist page check) |

     **Routing.** A failure goes to the plan that owns the failing file path
     (by writePaths). The next wave does not start until the join is green.
     A repair reruns the whole join.
12b. **Scoped per-plan gates.** A plan that shares its wave with siblings
     is judged only on its own scope:
     - **Scoped vitest is the pass gate.** Run the test directories or files
       listed in the plan.
     - **`npm run check`** passes when it exits 0, or when every reported
       error lies in a file outside this plan's writePaths and sharedPaths.
       In that case record the errors as sibling drift (rule 4) and do not
       count them as a failure. An error in your own file is always a
       failure.
     - **The full `vitest run` and `npm run build`** are informational for
       wave-sibling plans. Record their output, but a failure caused by a
       file outside your writePaths is drift, not a failure. The
       authoritative package-wide result is the wave-join gate (12a).
     - Plans that run alone in their wave (EMK-CORE, EMK-PERF,
       EMK-CLOSEOUT), and the sole editor writer per wave (EMK-VAC-SWITCH in
       wave 3, EMK-VAC-PLUGINS in wave 4) for the `editor/` checks, keep
       full-suite pass gates.

## Code rules

12. TypeScript only: strict, ESM, no `any` in exported signatures.
    - English only, no emojis.
    - Every touched or new source file stays under 1000 lines. Split it
      before it reaches 1000.
    - Comment density and naming follow the surrounding file.
13. emaki must never import outside `emaki/`. It must not mention `vactr`,
    `solid-js` or `/editor/`, and it must not depend on any package other
    than the peers and dev tools listed in rule 9.
14. vactr imports emaki only through `emaki`, `emaki/engine`, `emaki/diff`
    and `emaki/json`.
15. Never edit test expectations, fixtures or tolerances. Moved tests change
    only their import specifiers.
    - A vactr test that stays may change only its import specifiers, as
      listed in its plan.
    - If a test fails, fix the product code.
16. UI (emaki demos, vactr inline widgets and toggle): flat, `border-radius: 0`,
    token custom properties, controls spaced at least 8 px apart,
    `appearance: none`, no native widget look.
17. Never change the Mac system volume. Chromium runs with `--mute-audio`.
    The emaki demos produce no audio.
18. No git writes: no commit, stash, branch, checkout or reset. Read-only
    `git diff`, `git status` and `git show` are allowed.

## Progress Log

19. Edit only your own plan file, and only its `## Progress Log` section
    and its completion-criteria checkboxes. Each session entry lists:
    - tasks done;
    - files changed, with pre/post sha256;
    - every verification record, with log path, exit code and counts;
    - `mutationEvidence`;
    - drift found;
    - blocked items.

    Set Status to `Implemented; awaiting review` when every completion
    criterion is met. Only the review step sets `Completed`.
