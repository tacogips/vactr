# Vim Mode: Command Parser Implementation Plan

**Status**: Ready
**Plan ID**: VIM-18 (wave 1)
**Design Reference**: design-docs/specs/design-vim-mode.md sections 2.1, 4.1, 4.2, 4.6, 4.7
**Created**: 2026-10-16
**Last Updated**: 2026-10-16

## Related Plans

- **Depends On**: VIM-00
- **Parallel (wave 1)**: VIM-10..17
- **Consumer**: VIM-20, which executes `ParsedCommand`s

## Intent and Context

This plan turns a stream of normalized `VimKey`s (from `vim/keys.ts`)
into complete commands. It covers normal, visual, visual-line and the
operator-pending state. Insert and replace modes never call the parser;
the controller handles `<Esc>` there itself.
`editor/src/code/vim/parser.ts` exports `CommandParser` (signature pinned
in `vim-mode-00-contract.md`) and the types below. The common execution
rules in `vim-mode-00-contract.md` apply.

## Non-Goals

- No execution and no knowledge of the document.
- No visual block (`<C-v>` is never produced; VQ1).
- No `gv`, `gi`, `it`/`at`, or an operator followed by a `/`/`?` prompt
  (VQ4). These return `invalid` with a message.
- No macro stop logic. The controller intercepts `q` while recording,
  before calling the parser.

## Pinned Types (export from `parser.ts`)

```ts
export type ActionKind = 'insert' | 'append' | 'insert-line-start' | 'append-line-end' | 'open-below' | 'open-above'
  | 'replace-mode' | 'visual' | 'visual-line' | 'visual-swap' | 'put-after' | 'put-before' | 'replace-char'
  | 'toggle-case' | 'join' | 'undo' | 'redo' | 'repeat' | 'record' | 'play' | 'set-mark' | 'prompt' | 'escape';
export type Target =
  | { kind: 'motion'; motion: MotionSpec }
  | { kind: 'textobject'; spec: TextObjectSpec }
  | { kind: 'search'; which: 'n' | 'N' | '*' | '#' }
  | { kind: 'mark'; name: string; exact: boolean }
  | { kind: 'line' };
export interface ParsedCommand { register: string | null; count: number | null; operator: OperatorKind | null; target: Target | null; action: ActionKind | null; arg: string | null }
export type ParseResult = { kind: 'pending' } | { kind: 'invalid'; message: string | null } | { kind: 'command'; command: ParsedCommand };
```

`count` is the product of all typed counts, clamped to `COUNT_LIMIT`, or
`null` when no count was typed.

## Grammar (normal and operator-pending)

```
[count] ["{reg}] [count] (operator [count] (target | same-operator-again) | target | action)
```

- **Count.** `[1-9][0-9]*`. A `0` with no count digits typed is the `0`
  motion.
- **Register.** `"` followed by a key; `isRegisterName` from
  `vim/registers.ts` is not available in wave 1, so use the regex
  `/^[a-zA-Z0-9_+*"]$/` locally. An invalid name gives
  `invalid: 'E354 Invalid register name'`.
- **Operators.** `d c y > <`, plus `g~ gu gU` through the `g` prefix.
  Doubling (`dd`, `cc`, `yy`, `>>`, `<<`, `g~~` or `g~g~`, `guu` or
  `gugu`, `gUU` or `gUgU`) gives `target: { kind: 'line' }`.
- **Motion keys to `MotionSpec`.**
  - `h l j k 0 ^ $ w b e W B E G { } % H M L ; ,` map directly;
    `gg` -> `gg` and `ge` -> `ge`.
  - `f F t T` followed by one printable key (`<Esc>` cancels with
    `invalid: null`).
  - `<Left>` -> `h`, `<Right>` -> `l`, `<Up>` -> `k`, `<Down>` -> `j`,
    `<Home>` -> `0`, `<End>` -> `$`, `<BS>` -> `BS`, `<CR>` -> `+`,
    `<C-d>` -> `C-d`, and likewise for `<C-u>`, `<C-f>`, `<C-b>`.
- **Search targets.** `n N * #` give `{ kind: 'search' }`.
- **Mark targets.** `'` or `` ` `` followed by a key in `a-z`, `'` or
  `` ` `` gives `{ kind: 'mark', name, exact: key === '`' }`. The names
  `'` and `` ` `` mean the context mark.
- **Text objects** (only after an operator, or in visual mode): `i` or
  `a` followed by an object key. The key normalizes as `w` -> `w`,
  `W` -> `W`, `"` `'` `` ` `` as is, `(` `)` `b` -> `(`, `[` `]` -> `[`,
  `{` `}` `B` -> `{`, `<` `>` -> `<`. Any other key gives
  `invalid: null`.
- **Shortcuts.** These expand into operator and target:

  | Key | Expands to |
  |---|---|
  | `x` | `d` + motion `l` |
  | `<Del>` | `d` + motion `l` |
  | `X` | `d` + motion `h` |
  | `s` | `c` + motion `l` |
  | `S` | `c` + line |
  | `D` | `d` + motion `$` |
  | `C` | `c` + motion `$` |
  | `Y` | `y` + line |
- **Actions.**

  | Key | Result |
  |---|---|
  | `i` | `insert` |
  | `a` | `append` |
  | `I` | `insert-line-start` |
  | `A` | `append-line-end` |
  | `o` | `open-below` |
  | `O` | `open-above` |
  | `R` | `replace-mode` |
  | `v` | `visual` |
  | `V` | `visual-line` |
  | `p` | `put-after` |
  | `P` | `put-before` |
  | `r` + one key | `replace-char`, with `arg` the char (`<CR>` gives `'\n'`; `<Esc>` cancels) |
  | `~` | `toggle-case` |
  | `J` | `join` |
  | `u` | `undo` |
  | `<C-r>` | `redo` |
  | `.` | `repeat` |
  | `q` + `[a-zA-Z]` | `record`, `arg` = register |
  | `@` + `[a-z]` or `@` | `play`, `arg` = register or `'@'` |
  | `m` + `[a-z]` | `set-mark` |
  | `:` `/` `?` | `prompt`, `arg` = the key |
  | `<Esc>` | `escape` (always completes; resets pending state) |

  An action key after an operator is `invalid` (for example `dp`), except
  that `<Esc>` gives `escape`.
- **Errors.** `/` or `?` after an operator gives
  `invalid: 'search prompt cannot follow an operator'`. `<Tab>`,
  `<Other>` and unknown keys give `invalid: null`.

## Visual and visual-line modes (`mode` argument)

- Motions, search and mark targets give a command with `operator: null`;
  the controller moves the head.
- `i`/`a` + object gives a `textobject` target with `operator: null`.
- `d x <Del> c s y > < ~` and `g~ gu gU` complete **immediately** with
  `target: null` (the selection). `~` gives the operator `g~`.
- `D`, `X`, `C`, `S`, `R` and `Y` complete as the linewise forms
  `d`/`d`/`c`/`c`/`c`/`y`, with `target: { kind: 'line' }`.
- `r` + char gives `replace-char`; `J` gives `join`; `p`/`P` give
  `put-after`/`put-before`; `o` gives `visual-swap`; `v`/`V` give
  `visual`/`visual-line` (the controller toggles); `:` gives `prompt`.
- The register and count prefixes work as in normal mode.

## `pending()` and `operatorPending()`

- `pending()` returns the raw keys typed for the current incomplete
  command (named keys as is), and `''` after completion or reset.
- `operatorPending()` is true while an operator has been typed but no
  target yet.

## Tests: `editor/test/code/vim/parser.test.ts` (at least 50 cases)

The tests are table-driven: `{ mode: 'normal', keys: ['2','"','a','d','3','w'], expect: { register: 'a', count: 6, operator: 'd', target: motion w } }`.

- `dd` -> line; `3dd`; `d2w`; `"Ayy`; `"%` -> E354;
  `gUiw` -> textobject `w` inside; `g~~`; `gugu`.
- `0` motion versus `10j`; a count of 99999 clamped to 10000.
- `fx`, `dt)`, `f<Esc>` -> invalid null.
- `ci(`, `cib`, `da<`, `di}`, `yaW`, `diq` -> invalid.
- `x` / `D` / `Y` / `S` / `<Del>` expansions; `dp` -> invalid;
  `d<Esc>` -> escape.
- `r<CR>` -> `'\n'`; `qa`; `qA`; `q1` -> invalid; `@a`; `@@`; `ma`;
  `'a`; `` `a ``; `''`.
- `:`, `/` and `?` give prompts; `d/` -> invalid with the message.
- Visual: `iw` -> textobject; `d` -> immediate target null; `D` -> line;
  `~` -> `g~`; `o` -> visual-swap; `"ay` -> register `a`.
- `pending()` after `"`, `a` and `2` -> `'"a2'`; `operatorPending()` after
  `d`.
- Arrows map to motions, `<CR>` to `+`, `<BS>` to `BS`.

## Pitfalls

- Reset fully after `command` and `invalid`, so no stale register or
  count leaks into the next command.
- A count typed after an operator multiplies: `2d3w` is 6.
- In visual mode, `i` and `a` are text objects, not insert or append.

## Acceptance Criteria

- [ ] The pinned types and `CommandParser` methods are exported.
- [ ] At least 50 test cases pass.
- [ ] `parser.ts` is under 350 lines, and the purity test passes.

## Verification

1. A scoped typecheck per common rule 5 (`src/code/vim/parser.ts`,
   `test/code/vim/parser.test.ts`):
   `./node_modules/.bin/tsc --noEmit -p ../tmp/vim-mode/VIM-18/tsconfig.json`
   must exit 0.
2. `./node_modules/.bin/vitest run test/code/vim/parser.test.ts test/code/vim/purity.test.ts`
   must exit 0 with at least 50 tests and 0 failed.

## Progress Log

### Session: (not started)
