# Pending: middle-end questions from TASK-004..006 (2026-09-25)

These questions came up while designing the checker, VM and pattern
engine (design section 7.1 in `design-docs/specs/design-implementation.md`).
Each one has a recommendation, and the implementation follows it until
the question is answered. Where a behavior needs pinning, a fixture in
`tests/fixtures/spec/manifest.toml` fixes it, so an answer shows up as a
visible test change. None of them blocks issue #2.

Already decided for issue #2, so not asked here: the scope model
(design 20 Q1; the strict rule first recorded here was superseded the
same day, see Answers), `grid` (Q3), `gain` and `amp` (Q4), and
letter-first chord qualities. M1-M3 are answered below; M4-M6 are open
and follow their recommendations.

## M1. Prelude overloading on the subject (design 20 Q2, author question)

With one global scope and strict no-shadowing, a name has one binding.
The design-music and design-visual vocabularies both use `scale`
(scale notes and the visual transform) and `shape` (the sound control
and the visual source).

- Recommendation (followed by default, design 7.1.4): overload a fixed
  set, `scale` and `shape`, on the first argument's type. The checker
  resolves the overload when that type is known; otherwise the VM
  switches on the runtime tag. User code cannot declare overloads.
- Alternative: rename one side, as Hydra `repeat` became `tile`.
- The same question returns in TASK-008 for DSP unit names that match
  pattern controls or signals (`lpf`, `hpf`, `delay`, `saw`, `tri`).
  Those are out of scope for issue #2.

## M2. `range` argument order (spec consistency)

`design-music.md` section 3 writes `lpf {range 200 2000 sine}`, which
is Tidal's order. Every other use in the specs puts the subject first
(`range sine 1 5`, `{range rand 0.6 1}`, `osc {range sine 10 30}`), as
principle 2 decides.

- Recommendation: `range` is subject first, and the section 3 line
  becomes `lpf {range sine 200 2000}`.
- Until answered: the signature is subject first, and the fixtures pin
  the section 3 line as a checker `type-mismatch`. The spec text is not
  changed.

## M3. `amp` as a signal and as an instrument parameter (TASK-008 question)

Q4 keeps `amp` as the instrument parameter name. The signal vocabulary
also has an `amp` signal (host amplitude analysis), and Q1 makes
shadowing a prelude name an error. So `inst pluck ... amp: float = 0.5`
would be `shadowing`.

- Recommendation: decide this before TASK-008. Either rename the
  analysis signal (for example `level`), or treat `inst` header
  parameters as control names rather than bindings.
- Issue #2 is not affected. Blocks that use `inst` are `deferred` to
  TASK-008, and their check diagnostics are not pinned.

## M4. A single sound with no structure-giving step (SOUND FIRST)

design-music.md says `s :pluck` has no structure until a list-valued
step gives it one. It does not say what `s :crash > once` or
`s :break > begin 0.25 > end 0.5 > d1` play, since no step gives
structure there.

- Recommendation (followed, design 10.1): an unstructured sound that
  reaches a sink, `once`, or an operator such as `fast` or `every`
  plays one event per cycle spanning the cycle (Tidal's `pure`).
- Alternative: a checker diagnostic ("no structure; add a step list").
  That would reject several spec examples.

## M5. `#` inside a url literal

lang-reference.md says a url runs "up to whitespace", and `#` starts a
comment anywhere outside a string.

- Recommendation (followed, design 6.5.8): `#` ends a url and starts a
  comment, so url fragments are not supported in v1. Loading a file has
  no use for a fragment. Closing brackets `}` and `]` also end a url, so
  `{load https://x.org/p.vact}` reads as expected.
- Alternative: keep `#` inside a url and require whitespace before a
  comment that follows one.

## M6. Two `let sound-kit` lines in one design-music block

design-music.md section 2 shows two alternative overrides one after the
other:
`let sound-kit put default-sound-kit my-pack` and
`let sound-kit put default-sound-kit [bd: {sample ./bd/909.wav}]`.
Read as one document, the second line is `rebinding` (same scope).

- Recommendation: mark the second line as an alternative in the spec
  (for example, comment it out with "# or:"). Until answered, the block
  is `deferred` to TASK-008 (it defines an `inst`), so no fixture pins
  the `rebinding`; non-verbatim cases check each line on its own
  (design 7.1.7).

## Answers (author via architect, 2026-09-25)

- **Q1 superseded**: shadowing is now per scope with the chain prelude ->
  session -> fn/block; a child scope may shadow a parent (prelude shadow =
  hint, user-parent shadow = warning). See lang-reference.md and design 20 Q1.
- **M1**: follow the recommendation (overload `scale` and `shape` on the
  subject type; no user-declared overloads).
- **M2**: accepted; `range` is subject first and the section 3 line is
  corrected to `lpf {range sine 200 2000}`.
- **M3**: `inst` header parameters are CONTROL NAMES, not bindings, so
  `amp: float = 0.5` never shadows the `amp` signal; no rename.
