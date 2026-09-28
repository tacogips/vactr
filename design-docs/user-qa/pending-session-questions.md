# Pending: session-layer questions from TASK-009 (2026-09-25, issue #4)

These questions came up while designing the session layer: Session,
protocol, packages, directives, REPL, LSP, CLI and the self-analysis
surfaces (design section 14.5 in
`design-docs/specs/design-implementation.md`). Each one has a
recommendation, and the implementation follows it until the question
is answered. None of them blocks issue #4.

The following are already decided and are not asked here: the scope
chain; named-argument binding, including the positional/named proposal
already tracked for the author; directive attachment and the 13.5
vocabulary, which is tracked as authority questions in
`tests/fixtures/directives/vocabulary.toml`; the Go-style package model
with MVS, `vactr.lock` and the canonical digest; Q3 and Q4; and
`use-clock :link` staying a diagnostic.

## S1. When does `capture :bus cycles` record?

design-music.md says `let hit capture :drums 1` "record[s] one cycle of
a bus into a sample value". It does not say whether that means the
cycle just played or the next one.

- Recommendation: record the NEXT `cycles` cycles, starting at the next
  cycle boundary. The value is a sample that is pending until the
  recording completes. Reading it earlier fails with `capture-pending`.
  Offline `render` returns a sample value the same way, so both surfaces
  share one model.
- Alternative: capture after the fact, returning the last `cycles`
  cycles at once. This needs an always-on history of up to
  `max_capture_seconds` for every tapped bus.

## S2. Analyzer sources in v1

Two things are unresolved here.

1. Which sources can live taps read? v1 taps `:master` and named buses
   natively. Slots are not mixed separately in the engine, because
   voices go to orbit buses, and TASK-008 opened no input stream.
   - Recommendation: `:d1..:d9` and `:in` report "not available on
     this host" until a per-slot mix and an input stream exist. Browser
     taps are TASK-010.
2. `fft :master` and `amp :d1` (design-music lines 89-90) name a
   source. The current `fft n` and bare `amp` do not. Issue #4 lists
   `scope`, `spectrum`, `capture`, `render`, `rms` and `peak`, but not
   these two.
   - Recommendation: leave `fft n` and bare `amp` unchanged in issue
     #4. The spec fixture keeps its `type-mismatch` pins on those two
     lines. A follow-up issue can add the source forms: `amp :src`, and
     `fft :src n` for band n.

## S3. Keyword spelling for package asset banks

Keywords cannot contain `.`, so "package-qualified keywords" (design
5.7) needs a concrete spelling.

- Recommendation: `:<name>-<bank>`, where `<name>` is the package's
  default prefix (the last path segment without `vactr-`) and
  `<bank>` is an asset subdirectory. For example, `:pads-warm`. A clash
  gives `import-collision` (a warning), and the first registration wins.
- Alternative: expose banks only through package code, for example
  `let warm sample ./samples/warm.wav` inside the package, and register
  no keywords.

## S4. Overlay values in Directive persistence mode

The binding-identity criterion says both `hats.lpf` and `hats.hpf`
bindings "survive save and read-back in BOTH persistence modes". Design
13 also says overlay VALUES never reach the source without an explicit
commit.

- Recommendation: in Directive mode, save and read-back preserve keys,
  panel membership and MIDI mappings, and overlay values are not saved.
  ExternalFile mode also saves overlay values. The test asserts exactly
  this.
- Alternative: in Directive mode, keep overlay values in a sidecar file
  as well.

## S5. Semantic import versioning

In Go, a major version of 2 or higher changes the import path
(`/v2`).

- Recommendation: no `/vN` paths in v1. MVS compares majors like any
  other version and picks the maximum.
- Alternative: adopt Go's `/vN` rule now, before any package is
  published.

## S6. Default host when no audio device opens

- Recommendation: `repl`, `run` and `serve` default to `--host native`.
  If the device fails to open, they warn and fall back to `noop`, so
  the session stays usable (a live session never dies).
- Alternative: exit with code 1 and ask for `--host noop` explicitly.
