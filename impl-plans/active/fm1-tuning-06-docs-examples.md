# FM1 Tuning 06: lang-reference Section and Runnable Examples

**Status**: Ready
**Plan ID**: fm1-tuning-06-docs-examples
**Wave**: 4 (depends on fm1-tuning-05-natives)
**Design Reference**: design-docs/specs/design-tuning-and-strum.md sections 3, 4.8, 4.9 (MIDI limitation), 6, 9, 10 item 12
**Created**: 2026-10-10
**Last Updated**: 2026-10-10

## Intent and Context

Every new function must be documented in `lang-reference.md` and shown in
runnable examples. Completion and hover metadata already come from the
native table (plan 05).

Fenced `vactr` blocks in `lang-reference.md` are read and type-checked by
tests (`src/reader/tests/mod.rs`, `src/types/tests/no_abort.rs`). Committed
`.vact` files must be formatter fixed points (`src/fmt/tests/corpus.rs`)
and pass the LSP formatting test (`src/lsp/tests/analysis.rs`).

## Non-goals

- No Rust source changes, no `design-music.md` changes (the pointer already
  exists), no `THIRD_PARTY_NOTICES.md` change (design D8: no upstream used).
- No `load-scala` in examples or fences. Examples run on hosts without a
  source loader; show inline `scala` instead.

## Working Rules

Follow the Working Rules in `fm1-tuning-01-model.md`: fresh read plus
sha256 before and after each edit, own paths only, no git writes, silent
runs (`--host noop`), no full nextest. `lang-reference.md` is shared with
the sibling run: append at the end only.

## Files

| Path | Change |
| --- | --- |
| `design-docs/specs/lang-reference.md` | append one new top-level section at the END (after the current last section) |
| `examples/microtonal-tuning.vact` | new |
| `examples/strum-harp.vact` | new |

## lang-reference section content

Title: `## 6. Tuning and Chord Performance` (the current last section is
`## 5. Prelude: Core Vocabulary`). Do not renumber or edit section 5,
because `src/types/tests/natives.rs:204` cites its line numbers; appending
after it keeps them. It contains:

- one short prose paragraph per name: `tune`, `edo`, `ratios`, `scala`,
  `load-scala`, `strum`, `harp`, `inversion`, `perform`
- the preset keywords (`:ji-5 :ji-7 :partch-43 :bohlen-pierce`) and the 11
  microtonal scale names
- the ordering rule (put `tune` before `scale`/`chord`/`voicing`/`strum`/`harp`)
- the default anchor (key 60 at its 12-TET pitch; `ref-key:`/`ref-freq:`)
- the MIDI limitation (nearest note, no pitch bend)
- a link to `design-tuning-and-strum.md`

Use a few short ```vactr fences (each must read and check without abort),
for example:

- `s :pd > tune {edo 19} > n [0 2 4 7] > scale :c :edo19-major > d1`
- `s :pd > tune :ji-5 > chord [:c :maj] > voicing > strum 1/32 :down > d2`

Keep the existing text untouched. Write in English, no emojis, matching
the surrounding comment style (`#` comment lines inside fences are allowed;
see existing blocks).

## Examples

- `examples/microtonal-tuning.vact`: a header comment like
  `examples/first-track.vact`; `use-bpm`, then tracks d1-d5 showing
  19-EDO, 31-EDO, Bohlen-Pierce (`tune :bohlen-pierce > scale :c :bp-lambda`),
  `:ji-5` chords, `scale :c :maqam-rast`, and an inline `scala` slendro with
  modest gains. Use only existing templates (`:pd`, `:fm`, `:analog`,
  `:additive`).
- `examples/strum-harp.vact`: `strum` (up/down/alternate, a `:fade` curve),
  `harp` with `{range sine 0 1}`, `perform` with `mode: :strum`, `:arp`,
  `:harp`, `inversion: [0 1 2 1]`, `bass: true`, one track under
  `tune {edo 19}`.

SYNTAX: `( )` is a reader error in vactr (lang-reference.md:296, :332).
Write nested calls with braces (`tune {edo 19}`, `tune {alt {edo 19} {edo 31}}`,
`tune {scala "..."}`), even though the design doc writes `tune (edo 19)`
as notation.

After writing each file, run the formatter and copy its output back, so
the file is a fixed point.

## Verification

Evidence directory: `tmp/fm1-tuning/p06/`.

| Command | Must show |
| --- | --- |
| `CARGO_TERM_QUIET=true cargo run -- fmt --check examples/microtonal-tuning.vact examples/strum-harp.vact > tmp/fm1-tuning/p06/fmt.log 2>&1` | exit 0 (format first with `cargo run -- fmt <paths>`; the `fmt` verb is at `src/cli/args.rs:parse_fmt`) |
| `CARGO_TERM_QUIET=true cargo run -- run examples/microtonal-tuning.vact --host noop --cycles 2 > tmp/fm1-tuning/p06/run-microtonal.log 2>&1` | exit 0, no error diagnostics in the log (silent noop host) |
| `CARGO_TERM_QUIET=true cargo run -- run examples/strum-harp.vact --host noop --cycles 2 > tmp/fm1-tuning/p06/run-strum.log 2>&1` | exit 0, no error diagnostics |
| `CARGO_TERM_QUIET=true NEXTEST_STATUS_LEVEL=fail NEXTEST_FAILURE_OUTPUT=immediate-final NEXTEST_HIDE_PROGRESS_BAR=1 cargo nextest run --lib fmt::tests lsp::tests reader::tests types::tests > tmp/fm1-tuning/p06/nextest-docs.log 2>&1` | exit 0, failureCount 0 (corpus fixed point, LSP formatting, spec fences) |
| `git diff --stat design-docs/specs/lang-reference.md` | additions only |
| `grep -c "tune\|strum\|harp\|inversion\|perform\|edo\|ratios\|scala" design-docs/specs/lang-reference.md` | every new name present |

## Completion Criteria

- [ ] A lang-reference section documents all 9 names, the presets, the ordering rule, the anchor and the MIDI limitation
- [ ] Both examples run silently on the noop host and are formatter fixed points
- [ ] The docs-related test filters are green
- [ ] Progress log updated

## Progress Log

### Session: 2026-10-10
**Tasks Completed**: Plan created
**Notes**: Implementation not started
