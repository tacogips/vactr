# Indent-to-S-expression Rule Set

**Status**: Resolved (superseded, 2026-09-24)

**Created**: 2026-09-24

**Category**: Language Syntax

## Resolution

Superseded. None of the three families was adopted; the reader rules were
decided piecemeal in `specs/lang-reference.md`: one line is one prefix call;
a trailing `:` opens an indented block (the multi-line form of a `{}` group);
`{}` is the only inline nesting form and `( )` is a reader error; a line that
begins with `>` continues the previous expression (the pipe); tabs, one per
level. Kept below for the record.

## Original Decision Needed

Which rule set maps indented source text to S-expressions? The user chose to
defer this on 2026-09-24. Everything below the reader is independent of it,
but the reader, formatter, and LSP cannot start until it is fixed.

## Background

vactrol programs are written without most parentheses; the reader reconstructs
lists from line and indentation structure. Three established families exist.

## Alternatives

All three produce the same S-expression for this program:

```
(define (play-beat n)
  (sample :kick)
  (sleep (/ 1 n)))
```

### Option A: Wisp-style minimal (SRFI-119)

```
define : play-beat n
    sample :kick
    sleep : / 1 n
```

- One line is one list; indented lines are appended as elements.
- `:` opens a nested list to end of line; leading `.` marks "elements, not a call".
- Inline `( )` always allowed.
- Smallest rule set; simplest reader and LSP.
- Reads unusually for non-Lispers; no infix.

### Option B: Sweet-expressions (SRFI-110)

```
define play-beat(n)
    sample(:kick)
    sleep{1 / n}
```

- Wisp-like line rules plus `f(x)` call sugar and `{a op b}` infix groups.
- More familiar surface; more reader cases and two ways to write every call.

### Option C: Shrubbery-like (Rhombus)

```
def play_beat(n):
  sample(#'kick)
  sleep(1 / n)
```

- Groups, `:` blocks, `|` alternatives, and operator precedence tables.
- Most expressive and most familiar; reader is a substantial subsystem and
  macros must understand groups, not just lists.

## Impact

- `specs/architecture.md` Pipeline section (reader stage)
- Annotation syntax for optional types
- Formatter and LSP incremental parsing strategy

## Recommendation

Option A, given the "simplicity first" priority. Option B is the fallback if
early users find A too foreign.

## Awaiting

User decision after seeing sample programs in each style.
