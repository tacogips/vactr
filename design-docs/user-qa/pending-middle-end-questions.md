# Pending: middle-end questions from TASK-004..006 (2026-09-25)

These questions came up while designing the checker, VM and pattern
engine (design section 7.1 in `design-docs/specs/design-implementation.md`).
Each one has a recommendation, and the implementation follows it until
the question is answered. Where a behavior needs pinning, a fixture in
`tests/fixtures/spec/manifest.toml` fixes it, so an answer shows up as a
visible test change. None of them blocks issue #2.

Already decided for issue #2, so not asked here: strict no-shadowing
(design 20 Q1), `grid` (Q3), `gain` and `amp` (Q4), and letter-first
chord qualities.

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
