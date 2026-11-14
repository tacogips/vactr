# Vactr Product Rename Implementation Plan

**Status**: Completed
**Design Reference**: `design-docs/specs/architecture.md#product-identifier-contract`
**Created**: 2026-09-28
**Last Updated**: 2026-09-28

## Design Document Reference

Apply the Vactr identifier contract across source, manifests, package
metadata, editor, examples, plans, notices and local/remote repository
locations. Preserve `.vact` source syntax and extension. This is an
identifier and location change, not a DSP or language-semantics change.

## Modules and interfaces

The existing package, executable and library names in `Cargo.toml` become
`vactr`. The existing public CLI entry points and native package APIs keep
their signatures while using the new product identifiers and default paths.
No new Rust trait or data type is introduced.

| Area | Deliverable | Status |
|---|---|---|
| Rust | Crate imports, CLI strings, config/cache paths, environment variables and tests use Vactr identifiers | Completed |
| Package/editor | Cargo and npm names/locks, Tauri app name and identifier, worklet/editor links use Vactr | Completed |
| Documentation | English-only README, linked Japanese `README.jd.md`, examples, plans, notices and plan filenames use Vactr; optical component description remains accurate | Completed |
| Repository | Rename `tacogips/vactr`, update `origin`, rename the local checkout directory | Completed |

## Dependencies

| Feature | Depends On | Status |
|---|---|---|
| Rust verification | Renamed manifests and Rust identifiers | Satisfied |
| GitHub and local directory rename | Source and path audit | Satisfied |

## Completion Criteria

- [x] No tracked source content or path contains the former product identifier.
- [x] Cargo package, binary, editor, Tauri, configuration/cache paths and documentation consistently say Vactr.
- [x] `README.md` is English; `README.jd.md` provides Japanese documentation and reciprocal links.
- [x] `.vact` files and language syntax remain functional.
- [x] Quiet Rust and editor checks, tests, formatting and independent Rust review pass.
- [x] GitHub repository, `origin` URL and local checkout directory use `vactr`; remote access works.

## Progress Log

### Session: 2026-09-28

The rename started from a clean `main` checkout. The repository was
private and the authenticated account had admin permission. Non-Rust
source identifiers and 40 plan filenames were renamed; the required
Rust agent updated 105 Rust files, and independent review passed.
The crate, binary, Tauri and editor package, CLI/config/cache identifiers,
repository metadata, `origin` and local checkout now use Vactr. The
app cache and pinned reference directory were moved to their Vactr
paths. The `.vact` format remains unchanged. Quiet native/no-default/
wasm and Tauri checks, strict Clippy, formatting, 1,483 Rust tests
(one ignored), LSP smoke, 353 editor tests, TypeScript check, editor
build and the two pinned source probes passed.
