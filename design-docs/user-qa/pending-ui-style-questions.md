# Pending: Editor UI Style Choices (2026-10-05, session 275)

Design: `design-docs/specs/design-ui-style.md`. Each item has a recommended
default. The implementation follows that default unless the user overrides
it, so none of these items blocks implementation.

## UQ1: Accent Hue

**Context**

The design needs one restrained accent color. It is used for the primary
button fill, the focus ring, the active tab indicator and the beat dot.

**Options**

- A: keep the existing focus/beat blue, `#5aa9ff`.
- B: a cooler cyan that matches the syntax number color, `#88c0d0`.

**Recommendation: A**

- It is already the app's focus color.
- It gives 7.9:1 contrast with `--vt-on-accent`.
- It stays distinct from the `--vt-data-1` info color.

## UQ2: Hush Treatment

**Context**

Hush silences everything but leaves slots running. Stop-all and per-slot stop
are destructive.

**Recommendation**

- All three get the danger hover (`--vt-danger-bg` fill and `--vt-danger`
  border).
- Hush keeps its amber `--vt-warn` icon so "silence" stays visually distinct
  from "stop".

**Alternative**

Hush uses the default treatment.

## UQ3: Global `[hidden] { display: none !important }`

**Context**

Base element rules set `display` on controls. Without this rule, author CSS
can override the `hidden` attribute. This is already true for
`.vact-icon-button`, which sets `display: inline-flex` and is used by the
hidden-capable `.bind-commit` button.

**Recommendation**

Add the rule to `app.css`.

- It restores the intended semantics of `hidden`.
- It changes no JS behavior.
- jsdom tests do not load the stylesheets, so it has no effect there.
- Watch for one visible change: elements that were wrongly shown while
  `hidden` will now disappear. This is expected to be only `.bind-commit` when
  no overlay commit is pending.
