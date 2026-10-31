# Pending: music-things extraction

Decisions for
[`design-music-things-extraction.md`](../specs/design-music-things-extraction.md)
(2026-10-31, branch `wf/music-things-extract`).

All three items are open. The run follows the recommendation unless the owner
says otherwise. Neither item blocks implementation.

## MTQ1. Should the `mise` test and clippy tasks cover the staged crates?

**Question.** `mise run test` and `mise run clippy` (`cargo clippy
--all-targets -- -D warnings`) run at the root. With no `default-members`,
they target only `vactr`. The crate tests that move into
`music-things/crates/*` then run only with `--workspace`, which is what the
acceptance commands pass.

- (a) **Recommended for this run.** Leave `mise.toml` unchanged. The issue
  limits `mise.toml` edits to compare tasks whose paths move. The in-tree
  crates are transitional: the operator moves them to
  `tacogips/music-things`, which runs them in its own CI.
- (b) Add `--workspace` to the `test`, `test-cargo` and `clippy` tasks
  while the crates are in-tree.
- (c) Set root `default-members = [".", "music-things/crates/*"]`.
  Rejected: commands that select features, such as
  `--features host-wasm` and `--features lsp`, would then apply to every
  member.

## MTQ2. vactr design-section references in crate comments

**Question.** Moved kernels keep comments such as "design 12.8.8" or
"design-music 5". These point to vactr design documents. They pass the
acceptance grep but mean nothing in the music-things repository.

- (a) **Recommended.** Keep them in this run, since the code moves verbatim
  for bit identity. The operator or a music-things follow-up rewrites them
  after the copy.
- (b) Rewrite them in this run. This adds comment churn to about 150 files.

## MTQ3. How far should typed parameter setters go?

**Question.** The issue asks for "typed parameter setters". Kernels and
effects take positional inputs (`[Inp; MAX_PORTS]`, `p: &[f32]`). Changing
the processing signatures would risk bit identity and force edits to every
vactr caller.

- (a) **Recommended.** Add the generic setters `PortValues` (synth) and
  `ParamSet` (fx). They are keyed by the existing typed constants
  (`<kernel>::port::*`, `ParamDef` order). Each kernel's doc comment lists
  its ports. Named per-parameter methods are a follow-up in the
  music-things repository.
- (b) Generate named setter methods (for example `set_cutoff`) for about
  220 kernels and effect kinds in this run. This adds a large API surface
  that nothing in vactr uses or tests.
