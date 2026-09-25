# Vactrol Session Layer: Packages (SS-PKG) Implementation Plan

**planId**: SS-PKG (issue #4, wave 2; package manifest, lock, MVS, stores, canonical digest, archive safety, staged cache publication, package evaluation into `PkgNs`)
**Status**: Completed (accepted by the session-185 integration review; SS-FINAL re-verified the joined tree in session 186)
**Design Reference**: design-docs/specs/design-implementation.md 5.7 (Decided package model, revised digest and archive safety), 14.5.1 (fetch only in `vactrol get`; browser store is Rust-half only), 14.5.2 (in-crate SHA-256 and TOML subset; miniz_oxide), 14.5.7 (all rules), 14.5.12; command.md (`vactrol.toml`, `vactrol.lock`, `VACTROL_HOME`)
**Created**: 2026-09-25
**Issue**: https://github.com/tacogips/vactrol/issues/4
**dependsOn**: SS-CONTRACTS
**Dispatch manifest**: impl-plans/active/ss-session-20260925-s183-dispatch.json

---

## Intent and Context

This plan builds the package half of TASK-009 as a library that SESSION (wave 3) and CLI (wave 4, `vactrol get`) call.
It runs in wave 2 at the same time as SS-DIRECTIVES and SS-ANALYSIS, in the same working directory. The three plans
share no file.

What already exists:
- `ns/pkg.rs` has `PackageId`, `PkgNs::new` and `ImportBinding`.
- `Namespace::import` and `lookup_qualified` exist.
- `reader/import.rs` has `prescan_imports` and `AliasEnv`.
- `ns/load.rs` shows how a file is run through read -> expand -> check -> compile -> run into a fresh namespace.

After SS-CONTRACTS, `pkg/mod.rs` is a stub, `SampleLoader::register_bank` has a `host-unavailable` default, and the
package `DiagCode`s exist.

## Non-Goals

- No `Session` wiring, AliasEnv threading or diagnostics routing (SESSION). No CLI verb (CLI).
- No native HTTP client and no browser `fetch()`/OPFS backend (14.5.1, TASK-010). The proxy store is exercised only
  through the test transport against a local HTTP fixture.
- No public-network access anywhere, including tests.
- No `/vN` semantic import versioning (S5).
- No edits to `ns/evaluator.rs` or `reader/`.

## writePaths (exclusive in wave 2)

- `src/pkg/mod.rs` (fills the CONTRACTS stub), `src/pkg/semver.rs`, `src/pkg/manifest.rs`, `src/pkg/lock.rs`,
  `src/pkg/mvs.rs`, `src/pkg/sha256.rs`, `src/pkg/digest.rs`, `src/pkg/validate.rs`, `src/pkg/zip.rs`,
  `src/pkg/store.rs`, `src/pkg/proxy.rs`, `src/pkg/cache.rs`, `src/pkg/load.rs`
- `src/pkg/native/mod.rs`, `src/pkg/native/dir_store.rs`, `src/pkg/native/git_store.rs`, `src/pkg/native/fs_cache.rs`
- `src/pkg/tests/mod.rs`, `src/pkg/tests/support.rs`, `src/pkg/tests/http_fixture.rs`, `src/pkg/tests/semver.rs`,
  `src/pkg/tests/manifest.rs`, `src/pkg/tests/lock.rs`, `src/pkg/tests/mvs.rs`, `src/pkg/tests/sha256.rs`,
  `src/pkg/tests/digest.rs`, `src/pkg/tests/validate.rs`, `src/pkg/tests/zip.rs`, `src/pkg/tests/proxy.rs`,
  `src/pkg/tests/cache.rs`, `src/pkg/tests/stores.rs`, `src/pkg/tests/load.rs`
- `src/ns/pkg.rs`, `src/ns/tests/pkg.rs`
- `src/host/native/loader.rs`
- `impl-plans/active/vactrol-session-pkg.md`

## sharedPaths

None.

## File-Level Changes (signatures and behavior; no code)

1. **`pkg/mod.rs`.**
   - Declares every module above.
   - Declares `native` under `#[cfg(not(target_arch = "wasm32"))]`.
   - Declares `#[cfg(test)] mod tests;`.
   - Re-exports the public API used by SESSION and CLI.
2. **`semver.rs`.**
   - `Version { major, minor, patch, pre: Option<Rc<str>> }`.
   - `Version::parse_tag("v1.2.3[-pre]") -> Option<Version>`: a non-semver tag gives `None`.
   - `Ord` follows semver 2.0 precedence (a prerelease sorts below its release; prerelease identifiers compare
     numeric-then-lexical).
   - `latest_release(&[Version]) -> Option<&Version>`: the highest non-prerelease version.
3. **`manifest.rs` (strict TOML subset).**
   - `PkgManifest { package: Option<PackageMeta { path: PackageId, vactrol: Option<Version>, assets: Vec<Rc<str>> }>,
     deps: Vec<(PackageId, Version)> }`.
   - `parse(text) -> Result<PkgManifest, ManifestError{line, message}>` accepts tables `[package]`/`[deps]`, basic
     strings, string arrays and `#` comments. Anything else is an error.
   - `render(&PkgManifest) -> String` is deterministic (deps sorted by path).
   - `add_or_raise(&mut self, id, version)` keeps the max.
   - A package path must be lowercase `github.com/<owner>/<name>`; anything else is an error.
4. **`lock.rs`.**
   - `LockEntry { id: PackageId, version: Version, sha256: [u8; 32] }` and `LockFile { entries }`.
   - `parse`: the first line must be exactly `# vactrol.lock v1`, followed by `<path> <version> sha256:<64 lowercase
     hex>` lines. It rejects duplicates and unsorted input.
   - `render`: the header plus lines sorted bytewise by path.
   - `get(&PackageId) -> Option<&LockEntry>`.
5. **`mvs.rs`.**
   - `mvs_resolve(roots: &[(PackageId, Version)], store: &mut dyn PackageStore) -> Result<Vec<(PackageId, Version)>,
     PkgError>`.
   - It walks breadth-first over each visited `(path, version)` manifest (`store.manifest`) and picks the maximum of
     the minimum requirements per path.
   - Output is sorted by path. An unknown version in the store is `PkgError::Unresolvable`.
6. **`sha256.rs`.** `sha256(&[u8]) -> [u8; 32]` and an incremental `Sha256 { update, finish }`, both from FIPS 180-4
   and in-crate.
7. **`digest.rs`.**
   - `canonical_bytes(files: &[(Rc<str> /*path*/, [u8; 32] /*content hash*/)]) -> Vec<u8>`: records
     `u32_be(len) || path || hash`, sorted bytewise by path.
   - `tree_digest(files) -> [u8; 32]` is the sha256 of those bytes.
   - `hex` / `parse_hex` helpers.
8. **`validate.rs` (5.7 revised).**
   - `EntryKind { File, Dir, Symlink, Other }`.
   - `validate_entries(entries: &[(String, EntryKind, u64 /*size*/)], limits: &Limits) -> Result<(), IntegrityError{entry,
     reason}>`. Paths must be relative, use `/` and be UTF-8, with no byte below 0x20 and no 0x7f, no empty, `.` or
     `..` segment, and no drive or absolute prefix. Only File and Dir are allowed. Duplicates are refused, exactly
     and under Unicode simple case folding (`char::to_lowercase` per char is acceptable as the fold).
   - `Limits { max_entries: 10_000, max_bytes: 256 MiB }` (defaults). It is also checked incrementally during
     extraction.
   - `validate_assets(root_entries, assets) -> Result<Vec<Rc<str>>, IntegrityError>`: each asset path is normalized
     and must lie strictly under the root.
9. **`zip.rs`.**
   - `read_zip(bytes, limits) -> Result<Vec<(String, Vec<u8>)>, IntegrityError>` reads the end-of-central-directory
     and the central directory.
   - Refused: encryption flags, zip64, multi-disk archives, and any method other than 0 (stored) or 8 (deflate,
     through `miniz_oxide::inflate`).
   - A Unix symlink (external attributes mode `0o120000`) is `EntryKind::Symlink`, so it is refused by `validate.rs`.
   - The inflated length must equal the declared size, and a size bomb is aborted during inflation.
   - `validate_entries` runs on the entry list BEFORE any content is returned.
10. **`store.rs`.**
    - `trait PackageStore { list_versions(&mut self, &PackageId) -> Result<Vec<Version>, PkgError>;
      manifest(&mut self, &PackageId, &Version) -> Result<PkgManifest, PkgError>; fetch_into(&mut self, &PackageId,
      &Version, staging: &mut dyn StagingSink) -> Result<(), PkgError> }`.
    - `PkgSources { id, version, files: Vec<(Rc<str>, Rc<[u8]>)>, manifest: PkgManifest }`.
    - `PkgError { Unresolvable, Network(String), Integrity(IntegrityError), Manifest(ManifestError), NotLocked,
      NotFetched, Io(String) }`, each mapped to the 14.5.12 `DiagCode` by `PkgError::code()`.
11. **`proxy.rs`.**
    - `trait ProxyTransport { fn get(&mut self, url: &str) -> Result<Vec<u8>, PkgError>; }`.
    - `ProxyStore<T> { base: String, transport: T }` implements `PackageStore` over `{base}/{path}/@v/list`
      (newline-separated tags), `{base}/{path}/@v/{version}.toml` and `{base}/{path}/@v/{version}.zip`.
    - `fetch_into` = `read_zip` + validate, then write into the staging sink.
12. **`cache.rs`.**
    - `trait CacheBackend { create_staging(&mut self) -> Result<StagingId, PkgError>; write(&mut self, StagingId, path,
      bytes); remove_staging(&mut self, StagingId); publish(&mut self, StagingId, id, version, stamp: [u8; 32]) ->
      Result<(), PkgError>  /* atomic rename */; read_verified(&self, id, version, expect: [u8; 32]) ->
      Result<PkgSources, PkgError> }`.
    - `fetch_and_publish(store, cache, id, version, expect: Option<[u8;32]>) -> Result<[u8;32], PkgError>` runs the
      exact 14.5.7 sequence: staging -> validate -> digest -> compare (when `expect`) -> stamp -> publish. On ANY
      error it calls `remove_staging` and returns the error.
    - `get_all(manifest, store, cache) -> Result<LockFile, PkgError>` is `vactrol get`'s core: MVS, then fetch and
      publish each selected version, then build the lock.
13. **`native/` (non-wasm32).**
    - `dir_store.rs` `DirStore::new(root)`: `<root>/<path>@<version>/`. It lists versions from the directory names,
      reads the manifest from `vactrol.toml`, and walks the tree with `symlink_metadata` (a symlink is
      `EntryKind::Symlink`, a device is `Other`).
    - `git_store.rs` `GitStore::new(base)`:
      - runs `git` through `std::process::Command` with fixed argv arrays, no shell, and `--` before operands;
      - sets `GIT_TERMINAL_PROMPT=0` and `GIT_CONFIG_NOSYSTEM=1`, and passes
        `-c core.hooksPath=/dev/null -c protocol.file.allow=user`;
      - lists versions with `ls-remote --tags <base>/<path>`;
      - fetches with `clone --depth 1 --branch <tag> --single-branch -- <url> <dir>`, then removes `.git/` before
        validating.
    - `fs_cache.rs` `FsCache::new(root)`: `<root>/.staging/<random>` created with `create_dir` (it fails if the path
      exists; the random name comes from the time plus pid plus a counter), `std::fs::rename` into
      `<root>/<path>@<version>`, and the stamp file `.vactrol-digest` (hex).
    - `vactrol_home() -> PathBuf` returns `$VACTROL_HOME`, else `$HOME/.vactrol`. The cache root is `<home>/pkg`.
14. **`load.rs` + `ns/pkg.rs` (package evaluation, 14.5.7 "Loading").**
    - `ns/pkg.rs` gains `PkgNs::load(id, prelude: Rc<Prelude>, vm: &mut Vm, sources: &PkgSources, manifest:
      &HostManifest, next_file: &mut dyn FnMut(&str) -> FileId) -> (Rc<PkgNs>, Vec<Diagnostic>)`.
    - The `.vact` files run in bytewise path order through read -> expand -> check -> compile -> run into the
      package's namespace, following the `ns/load.rs` pipeline. `ns/evaluator.rs` is NOT edited.
    - Package diagnostics carry the package's own `FileId`s. A failing form is recorded and the load continues.
    - The package's own imports resolve through the same lock; the caller passes a resolver closure.
    - `pkg/load.rs` `default_prefix(&PackageId) -> Rc<str>` is the last segment without `vactrol-`.
    - `pkg/load.rs` `asset_banks(&PkgSources, prefix) -> Vec<(Rc<str> /*":<name>-<bank>"*/, Vec<PathVal>)>`: each
      immediate subdirectory of each validated asset directory becomes one bank.
    - `host/native/loader.rs` implements `register_bank`: the files of a bank are indexed by `n` in bytewise order,
      and a clash with an existing bank returns an error that SESSION reports as `import-collision`.

## Required Tests (`src/pkg/tests/*.rs`, `src/ns/tests/pkg.rs`; no network, no files outside the temp dir)

- `support.rs`:
  - a temp-dir helper under `std::env::temp_dir()` with a unique name that removes itself on drop;
  - a test-only stored/deflate ZIP WRITER (fixture zips are generated at test time; `*.zip` is gitignored);
  - a local-directory fixture builder.
- `http_fixture.rs`: a `std::net::TcpListener` on `127.0.0.1:0` serving an in-memory path->bytes map on a thread
  (HTTP/1.1 GET, 404 otherwise, stopped on drop), plus a `ProxyTransport` test client over `TcpStream` that accepts
  only `http://127.0.0.1:<port>/` URLs.
- `sha256.rs`: the FIPS 180-4 vectors (empty, `abc`, the 448-bit message, one million `a`).
- `semver.rs`, `manifest.rs`, `lock.rs`: parse, render and round-trip; reject malformed input with a line number;
  lock ordering and header version.
- `mvs.rs`: a competing-requirements graph (A needs C v1.1, B needs C v1.3, so C v1.3 wins), a transitive dependency,
  and an unresolvable version.
- `digest.rs`:
  - PORTABILITY: a `DirStore` tree and a test-written zip of the same sources give the SAME digest.
  - INJECTIVITY (Astra's counterexample): a tree whose file name embeds `\n` plus a hash-lookalike suffix, versus the
    two-file tree it imitates. `validate_entries` REJECTS the malicious tree, and independently the canonical bytes
    and the digests of the two trees differ.
- `validate.rs` and `zip.rs`, each against CORRECTLY-HASHED malicious fixtures: a parent-traversal entry, an absolute
  path, a symlink escaping the root (also listed as a manifest asset), duplicate and case-fold duplicate entries, and
  an over-limit archive. Each is rejected with `package-integrity` naming the entry.
- `cache.rs`:
  - each malicious fixture above leaves the cache directory byte-identical (no mutation);
  - interrupted extraction: a `CacheBackend` wrapper that fails `write` after half the entries leaves no
    `<path>@<version>` and no staging directory;
  - `read_verified` with a wrong stamp is `package-integrity`;
  - a hash mismatch against the lock is `package-integrity`.
- `proxy.rs`: `ProxyStore` over the local HTTP fixture lists, reads the manifest, fetches, validates and publishes. Its
  lock digest equals the `DirStore` digest of the same sources.
- `stores.rs`: `GitStore` against a `file://<tmp>` base holding a local bare repository created with the `git` CLI
  (two tags; `list_versions`, then fetch v1.1.0; `.git/` never enters the digest). This test requires a local `git`
  binary. If `git` is missing, the test FAILS with a clear message; it is never silently skipped.
- `load.rs` and `ns/tests/pkg.rs`:
  - a fixture package loads into `PkgNs` and exposes `warm` under the prefix `pads`;
  - a package compile error yields `package-load-failed` plus its own diagnostics at the package `FileId`;
  - re-loading replaces the `PkgNs`;
  - `asset_banks` gives `:pads-warm`, and `NativeSampleLoader::register_bank` then loads index 0.

## Invariants

- Validation runs before hashing and before any cache write. The cache never holds unverified content.
- Everything in `pkg/` outside `native/` and `tests/` is wasm-safe: no `std::fs`, `std::net`, `std::process` or
  `std::thread`.
- No panic on untrusted bytes: malformed zip, lock or manifest input returns an error.
- No `.rs` file reaches 800 lines.

## Edit Protocol

The common protocol in `vactrol-session-contracts.md` "Edit Protocol (common to every SS plan)". Evidence goes under
`tmp/ss-session-20260925-s183/SS-PKG/attempt-<n>/`. In this parallel wave, a crate-wide failure caused only by a
sibling's in-flight file is recorded as sibling-caused and left for the join.

## Verification (`<wave>` = `pkg`)

The common rows V1, V1l, V2, V2l, V3, V3t, V3f, V6a, V6b, V7, V4, V5 and V9 of `vactrol-session-contracts.md`, plus:

| # | Command | Evidence |
|---|---------|----------|
| P1 | LOG(`ss-pkg-own`): `NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 CARGO_TERM_QUIET=true cargo nextest run -E 'test(/pkg::tests/) \| test(/ns::tests::pkg/)'` | `exit=0`, run > 0 |
| P2 | `git --version` | recorded (a prerequisite of the git store test) |

## Completion Criteria

- [x] Items 1-14 implemented as specified
- [x] Every required test passes, including the portability, injectivity, malicious-fixture, interrupted-extraction,
      local-HTTP proxy and local-git tests
- [x] V1-V9 and P1-P2 pass with logs cited; `final-hashes.txt` written (session 185: every row exits 0 after the operator's
      amendment of the two sibling-scope tests; see the session-185 log)

## Progress Log

(Implementer: add one `### Session: <date> (session <S>, SS-PKG implementer)` entry covering the work done, any design
differences, the hash and intent file paths, evidence per row, sibling-caused failures, and blockers. Edit only this
log.)

### Session: 2026-09-25 (session 184, SS-PKG implementer)

**Work done** (evidence root `tmp/ss-session-20260925-s183/SS-PKG/attempt-1/`: `intent.md`, `notes.md`,
`pre-edit-hashes.txt`, `post-edit-hashes.txt`, `final-hashes.txt`):
- Items 1-12: `src/pkg/{mod,semver,manifest,lock,mvs,sha256,digest,validate,zip,store,proxy,cache,load}.rs`. Everything
  outside `native/` and `tests/` is wasm-safe (V5 prints `none`; both wasm32 builds exit 0).
- Item 13: `src/pkg/native/{mod,dir_store,git_store,fs_cache}.rs` (`DirStore`, `GitStore` with the fixed argv/env,
  `FsCache` with `.staging/<unique>` + `create_dir` + `rename` + `.vactrol-digest`, `vactrol_home`, `pkg_cache_root`).
- Item 14: `PkgNs::load` in `src/ns/pkg.rs`; `default_prefix`, `asset_banks`, `locked_sources` in `src/pkg/load.rs`;
  `NativeSampleLoader::register_bank` in `src/host/native/loader.rs` (bytewise index order; a clash fails
  `load-failed` for SESSION to report as `import-collision`; the first registration wins).
- Tests: `src/pkg/tests/{support,http_fixture,sha256,semver,manifest,lock,mvs,digest,validate,zip,proxy,cache,stores,load}.rs`
  and four new tests in `src/ns/tests/pkg.rs` (the three existing tests are unchanged). 47 tests in the P1 set.

**Design differences** (plan-level refinements, detailed in `notes.md`): `PkgNs::load` has a 7th `imports` resolver
parameter; `PkgError` variants carry payloads; `PkgSources` gains `root` (native cache dir) and `from_files`;
`asset_banks` returns names without `:`; lock parse errors reuse `ManifestError`; `ProxyStore` has `limits` and zip
paths are root-relative; `FsCache::new` creates `.staging` eagerly; the staging sink refuses `.vactrol-digest` and
case-fold duplicates before each write (a case-insensitive file system would otherwise report an I/O error); the ZIP
CRC-32 is not checked (the digest carries integrity); hard-linked files are `EntryKind::Other`.

**Verification** (shared tree, `target/fe-logs/`, counting logs):
- P2 `git --version`: git version 2.55.0 (`git-version.txt`).
- V1 `ss-pkg-build-s184-1.log` exit=0; V1l `ss-pkg-build-lsp-s184-1.log` exit=0.
- V2 `ss-pkg-clippy-s184-2.log` exit=0; V2l `ss-pkg-clippy-lsp-s184-2.log` exit=0 (the `-1` runs exited 101 only on
  `clippy::match_like_matches_macro` at `src/types/infer_call.rs:358`, SS-ANALYSIS, since fixed by that plan).
- P1 `ss-pkg-own-s184-1.log` exit=0: 47 run, 47 passed.
- V3f `ss-pkg-fixtures-s184-1.log` exit=0: 10 run, 10 passed, 1 skipped.
- V3 `ss-pkg-nextest-s184-2.log` exit=100 (fail-fast at 83/901) and V3t `ss-pkg-cargotest-s184-2.log` exit=101
  (887 passed, 2 failed). Supplementary `ss-pkg-nextest-nofailfast-s184-1.log`: 901 run, 899 passed, 2 failed.
  The two failures are SIBLING-CAUSED and outside every plan's writePaths:
  `dsp::tests::contracts::native_accepts_its_advertised_limits` (asserts `Cap::OfflineRender` is refused natively,
  which SS-ANALYSIS's `dsp/caps.rs` now grants) and `types::tests::natives::only_scale_and_shape_are_overloaded`
  (SS-ANALYSIS adds overloaded `scope`/`spectrum`/`render`). These are pre-existing tests that contradict the
  ANALYSIS scope, so they are a dependency blocker for the operator or the join, not an SS-PKG defect. The join
  re-verifies V3/V3t.
- V6a `ss-pkg-wasm32-s184-1.log` exit=0; V6b `ss-pkg-wasm32-hostwasm-s184-1.log` exit=0.
- V7 `ss-pkg-fmt-s184-1.log` exit=0.
- V4 (`v4-linecounts.txt`): the largest `.rs` file is 799 lines (`src/dsp/build.rs`); the largest SS-PKG file is
  `src/pkg/manifest.rs` at 424 lines.
- V5 (`v5-grep.txt`): `none`.
- V9 (`tree-wasm32.txt`, `tree-wasm32-hostwasm.txt`): 0 matches for tungstenite|getrandom|tokio|tower-lsp|cpal|midir.

**Parallel-wave notes**: while SS-DIRECTIVES was mid-edit, the shared lib-test build failed (missing
`src/directives/tests/{key,persist,writeback}.rs`). SS-PKG tests were type-checked and run meanwhile in a scratch COPY
(`attempt-1/scratch/tree`, directives tests stubbed in the copy only, separate target dir). All final evidence above
comes from the shared tree. Another writer rustfmt-formatted several new SS-PKG files mid-attempt (formatting only; no
behavior lost; later edits used fresh content). There was no drift against `post-edit-hashes.txt` at the end.

**Blockers**: none owned by SS-PKG. The two sibling-scope test failures above are left for the join.

### Session: 2026-09-25 (session 185, SS-PKG implementer)

**Work done** (evidence root `tmp/ss-session-20260925-s183/SS-PKG/attempt-2/`: `intent.md`, `pre-edit-hashes.txt`,
`final-hashes.txt`, `git-version.txt`, `tree-wasm32*.txt`, `v4-linecounts.txt`, `v5-grep.txt`, `git-status.txt`):
- Re-dispatch after the operator amended `src/types/tests/natives.rs` and `src/dsp/tests/contracts.rs` to the 12.3
  amendment (the two session-184 sibling-scope failures). No SS-PKG source edit was needed: `pre-edit-hashes.txt`,
  `final-hashes.txt` and `attempt-1/final-hashes.txt` are identical for all 35 owned source files. Only this plan file
  was edited (Status, criterion 3, this entry).

**Verification** (shared tree, `target/fe-logs/*-s185-1.log`, each ending with `exit=`):
- P2 `git --version`: git version 2.55.0.
- V1 `ss-pkg-build` exit=0; V1l `ss-pkg-build-lsp` exit=0; V2 `ss-pkg-clippy` exit=0; V2l `ss-pkg-clippy-lsp` exit=0.
- P1 `ss-pkg-own` exit=0: 47 run, 47 passed.
- V3 `ss-pkg-nextest` exit=0: 901 run, 901 passed, 1 skipped. V3t `ss-pkg-cargotest` exit=0: 901 passed, 0 failed,
  1 ignored. V3f `ss-pkg-fixtures` exit=0: 10 run, 10 passed, 1 skipped.
- V6a `ss-pkg-wasm32` exit=0; V6b `ss-pkg-wasm32-hostwasm` exit=0; V7 `ss-pkg-fmt` exit=0.
- V4: the largest `.rs` file is `src/dsp/build.rs` at 799 lines (not edited). V5: `none`. V9: 0 matches for
  tungstenite|getrandom|tokio|tower-lsp|cpal|midir in both wasm32 trees.

**Blockers**: none. Formal test-integrity/adversarial/integration review and the commit are downstream steps.

## Related Plans

- **Parent**: impl-plans/active/vactrol-core.md (TASK-009)
- **Previous**: vactrol-session-contracts.md. **Parallel**: vactrol-session-directives.md, vactrol-session-analysis.md
- **Next**: vactrol-session-core.md (SESSION), vactrol-session-cli.md (`vactrol get`)


### STEP6 OUTPUT NOTE (operator, 2026-09-25, after the SS-ANALYSIS attempt-1 failure)

- The step6-implement output contract requires `changedFiles` to be an ARRAY of
  path strings (SS-ANALYSIS attempt 1 failed with "$.changedFiles must be of type
  array"). Carry `planId`; leave `verificationGaps` empty when every automated
  command passed (manual checks go under `residualRisks`). Crate-wide test
  failures caused only by a sibling branch's in-progress files or by a
  pre-existing test outside every plan's ownership are reported in the
  progress log as a dependency blocker for the operator, never fixed by
  editing unowned files.

### INTEGRATION REVIEW OUTPUT NOTE (operator, 2026-09-26, after two adapter rejections in session 185)

- The integration-review step output MUST be an ENVELOPE with two top-level
  keys: `"when"` (the routing flags `needs_revision`, `redispatch_required`,
  `repair_in_place`, `plans_remaining`) and `"payload"` (an OBJECT holding the
  review itself: `needs_revision`, `loopGate`, `acceptedPlanIds`, `findings`,
  `recoveryDiagnostic`, summaries, evidence paths). Two attempts were rejected
  with "payload must be an object when when is provided" because the review
  fields were emitted at the top level next to `when` instead of inside
  `payload`. `acceptedPlanIds` lists only plans present in the manifest's
  `plans[]`.

### CLOSING NOTE (SS-FINAL, session 186, 2026-09-26)

- Status set to Completed by SS-FINAL. Join integrity: tmp/ss-session-20260925-s183/SS-FINAL/attempt-1/join-integrity.txt.
- Final-tree evidence: target/fe-logs/ss-final-<check>-s186-1.log (build, build-lsp, clippy, clippy-lsp, fmt, nextest 984/984, cargotest 984, fixtures 10/10, lsp-smoke 1/1, cli 9/9, session 132/132, example, wasm32, wasm32-hostwasm; all exit=0).
