# CE-PACKAGE dependency evidence

Issue: codex-design-and-implement-review-loop-session-224. Workflow mode: issue-resolution.

CE-CONTRACT admitted by runtime acceptedPlanIds. Committed CE-PACKAGE plan aligns with accepted design 15.3.6. Only initial dependency preparation is owned here. No Rust/Swift/TypeScript sources changed.

## Locked artifacts

Playwright and playwright-core are exactly 1.62.1. Install used --ignore-scripts. Existing npm package records are unchanged. Optional nested fsevents 2.3.2 is the only other added npm package.
- node_modules/playwright: 1.62.1; https://registry.npmjs.org/playwright/-/playwright-1.62.1.tgz; sha512-0M+L3LAD8/nm554LOla9Ayx0j0tmFZ0FBcoQ7F1VuVHpM/XpiC8RcDzBQB8W5+hA8L22THxELzeF+2WcUzvcLg==
- node_modules/playwright-core: 1.62.1; https://registry.npmjs.org/playwright-core/-/playwright-core-1.62.1.tgz; sha512-wPYSwEBJY9GHraISXqyqtx0na0LpO3XEX7jNDhntbex7tzUS7kLnZsOlFruFJB4Hi/rhDMjXGqHewDZ68nYZVw==
- node_modules/playwright/node_modules/fsevents: 2.3.2; https://registry.npmjs.org/fsevents/-/fsevents-2.3.2.tgz; sha512-xiqMQR4xAeHTuB9uWm+fFRcIOgKBMiOBP+eXiyT7jsgVCq1bkVygt00oASowB7EdtpOHaaPgKt812P9ab+DDKA==

Tauri 2.11.6, tauri-build 2.6.3, dialog 2.7.3 and fs 2.5.2 are pinned to their pre-existing locked versions. Local vactr path ../.. enables host-native explicitly with default features disabled. Root Cargo.toml and Cargo.lock remain byte-identical to initial snapshots.

Exact cargo generate-lockfile succeeded but refreshed unrelated transitives. Self-review corrected this with a contextual graph merge: retained every original package, added the root-native closure, and let metadata normalize disambiguation/prune unused entries. All 430 existing name/version/source/checksum identities remain; 31 identities were added, all attributable to vactr host-native. No new root dependencies. Final full cargo metadata --locked succeeds. Original refreshed graph comparison is retained in dependency-comparison.json; final comparison is dependency-comparison-final.json.

Native transitive tree: cargo-tree.log. Build-script/proc-macro inventory: native-supply-chain-inventory.json. Metadata/download commands do not compile or execute those build scripts; no comprehensive vulnerability audit is claimed.

## Command evidence

| Command | Exit | Complete log |
|---|---:|---|
| `node tmp/canvas-editor-224/CE-PACKAGE/attempt-1/browser-smoke.mjs` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/browser-smoke.log` |
| `CARGO_TERM_QUIET=true cargo generate-lockfile --manifest-path editor/src-tauri/Cargo.toml` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/cargo-lock.log` |
| `CARGO_TERM_QUIET=true cargo metadata --locked --no-deps --manifest-path editor/src-tauri/Cargo.toml --format-version 1` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/cargo-metadata-final.log` |
| `CARGO_TERM_QUIET=true cargo metadata --locked --no-deps --manifest-path editor/src-tauri/Cargo.toml --format-version 1` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/cargo-metadata.log` |
| `CARGO_TERM_QUIET=true cargo metadata --offline --manifest-path editor/src-tauri/Cargo.toml --format-version 1` | 101 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/cargo-normalize.log` |
| `CARGO_TERM_QUIET=true cargo metadata --locked --manifest-path editor/src-tauri/Cargo.toml --format-version 1` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/cargo-resolved-final.log` |
| `CARGO_TERM_QUIET=true cargo tree --locked --manifest-path editor/src-tauri/Cargo.toml -p vactr` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/cargo-tree.log` |
| `cd editor && npm exec -- playwright install chromium` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/chromium-install.log` |
| `mise exec -- python3 tmp/canvas-editor-224/CE-PACKAGE/attempt-1/dependency-check.py` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/dependency-check.log` |
| `cd editor && npm install --save-dev --save-exact playwright@1.62.1 --ignore-scripts` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/npm-install.log` |
| `mise exec -- rustup target list --installed` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/rust-targets.log` |
| `CARGO_TERM_QUIET=true cargo tauri --version` | 101 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/tauri-version.log` |
| `xcodebuild -showsdks` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/xcode-sdks.log` |
| `xcodebuild -version` | 0 | `tmp/canvas-editor-224/CE-PACKAGE/attempt-1/xcode-version.log` |

## Browser and native inventory

Chromium: 151.0.7922.34. Executable: /Users/taco/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing. Local-import smoke launched headless browser, created WebGL2 and read a green GPU pixel: 3 assertions passed, 0 failed. Browser closed in finally. This is preparation evidence, not application rendering/device evidence.

Xcode 26.6 (17F113) and iOS 26.5 device/simulator SDKs available. Installed Rust targets: aarch64-apple-darwin and wasm32-unknown-unknown. No iOS Rust target; cargo-tauri unavailable (exit 101). These conditional inventory commands do not claim native compilation; required downstream tool installation needs serial/environment ownership. No global tools installed by this worker.

The offline full metadata normalization failed while fetching uncached alsa 0.9.1; subsequent full online --locked metadata completed on the final lock and resolved the fetch limitation. Preserve both logs. Missing cargo-tauri is an explicit unavailable inventory result, not a passing check.

## Ownership and review

Before-edit snapshots and intentions: attempt-1/edit-001 through edit-004, mirrored read-only under /private/tmp/vactr-224-implementation/CE-PACKAGE/attempt-1/. Only approved four dependency files and these two plan-local documents changed. Initial fanout snapshot: tmp/riela-fanout/B052487A-9E60-45CC-BAE2-03865CD2D44A/8E5F495A-19EE-4504-8AEB-D58800B4511C.json. No worker Git mutations, root lock edits or unrelated changes.

Improve author self-review fixed resolver upgrades; dependency assertions pass 8/8. Read-only /root/package_review investigates and reviews scope; formal integrity/adversarial approval and serial join/Git finalization remain downstream. CE-SHELL consumes this lock with --locked; workers must not regenerate it.

## Session 227 Step 6 retry — 2026-09-30

Portable runners now live in `editor/test/canvas/package-browser-smoke.mjs` and
`editor/test/canvas/package-dependencies.py`. CE-CONTRACT is admitted by runtime
acceptedPlanIds; preparation matches accepted design 15.3.6. No dependency installs,
lock generation, Rust/Swift/TypeScript edits or Git mutations occurred in this retry.
System Python 3.9 lacks tomllib; the dependency runner re-executes with the installed
mise-managed Python 3.12.14 when needed. The assigned bare-python command passes.

| Command | Exit | Assertions | Complete log under tmp/canvas-editor-224/CE-PACKAGE/attempt-2/ |
|---|---:|---|---|
| `node editor/test/canvas/package-browser-smoke.mjs` | 0 | 4 passed, 0 failed | `browser-smoke.log` |
| `python3 editor/test/canvas/package-dependencies.py --baseline tmp/canvas-editor-224/CE-PACKAGE/attempt-1 --output tmp/canvas-editor-224/CE-PACKAGE/attempt-2/dependency-comparison.json` | 0 | 10 passed, 0 failed | `dependencies.log` |
| `CARGO_TERM_QUIET=true cargo metadata --locked --manifest-path editor/src-tauri/Cargo.toml --format-version 1` | 0 | inventory, not behavioral test | `cargo-metadata.log` |
| `CARGO_TERM_QUIET=true cargo tree --locked --manifest-path editor/src-tauri/Cargo.toml -p vactr` | 0 | inventory, not behavioral test | `cargo-tree.log` |

Each same-named JSON records command, cwd, start/end, terminal exit and hashes of
both root manifests/locks, shell manifests/locks and runners before/after. All eight
identities match across all four checks. `dependency-comparison.json` authenticates
attempt-1 baseline snapshots, preserves 430 Cargo identities/checksums and existing
npm records, checks exact automation pins and allowed sources, and attributes all
31 added native identities to the local vactr closure. Root drift since preparation
is false; later unrelated root changes would be reported, not reverted.

Chromium 151.0.7922.34 launched from the already installed executable recorded in
`browser-smoke.log`, created WebGL2 and read [0,255,0,255]. Browser closed in finally.
This proves installed preparation only; application and device evidence belongs to
CE-FINAL. Historical attempt-1 offline metadata exit 101 and missing cargo-tauri
exit 101 remain above and in their original complete logs.

Read-only `/root/package_review` and improve author self-check found no material issue
following the Python portability correction. Review record: `attempt-2/independent-review.json`.
Before each edit, fresh bytes/diff/intended content were captured under
`attempt-2/edit-*/` and `/private/tmp/vactr-224-implementation/CE-PACKAGE/attempt-2/edit-*/`.
Native progress evidence is submitted through this Step 6 output for the runner's
subsequent gate; no native accepted decision is claimed. Formal test-integrity,
adversarial and integration review, review-dependent completion records and Git
finalization remain downstream. No hand-written acceptance substitutes for those gates.
