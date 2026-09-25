# vactrol Language Reference (working draft)

Reference code from which the language specification is reverse-engineered.
Section 1 is the original sketch; the rest is the core language. The two
domains are specified separately (author, 2026-09-24): `design-music.md`
(time, sound, patterns; TidalCycles/Strudel and Overtone) and
`design-visual.md` (Hydra). Sonic Pi's imperative layer was considered
and withdrawn (`design-music.md`). Their coupling is deferred (`notes.md`).

Conventions used in the code blocks:

| Marker | Meaning |
|--------|---------|
| `#` | comment |
| `# QA-nn:` | open question for the language author. Answer inline below it with `# A:`; the spec picks answers up from here. `grep '# QA-'` lists all questions. |
| `# Decided (...):` | a former question, answered; the basis (a principle, taste, consistency) is in the parentheses when it was not a direct answer |
| `# PROPOSED:` | notation that goes beyond the original sketch; a suggestion, not a decision |
| `# => v` | the value the expression evaluates to |
| `# ~ (...)` | the S-expression the line desugars to |

## 0. Principles

1. **Simple and easy to write.** Few forms, few rules. The language exists to
   be typed while it runs.
2. **Left to right.** Code is written in the order it is thought: the subject
   first, then what happens to it. Only three things require planning ahead:
   `if`, `match`, and a `{}` group. Everything else chains left to right.
   Library functions therefore take their subject first (`fast pat 2`,
   `d :amp`, `put d amp: 0.7`) so that pipes work without placeholders.
3. **Readable live code over type safety.** When a rule would make a chain
   longer or noisier in exchange for a static guarantee, the chain wins.
   Failures are reported by the runtime, not values threaded through every
   call and not a type in the language; types are optional annotations, never
   obligations.
4. **Typed, with inference.** vactrol is statically typed. Every expression
   has a type the checker can name and the LSP shows. Annotations are
   optional because inference makes them unnecessary, not because types are.
   In Live mode a type error is a diagnostic and never stops the music; in
   Frozen mode it is a compile error. `any` is an explicit escape hatch,
   never the default.
5. **Declarative.** Time is data. Nothing sleeps, waits, or ticks: patterns
   say where events fall in a cycle, instruments and visuals are chains
   that describe a signal, and the scheduler realizes them. Names are
   late-bound, so changing a value or a function is heard at the next event.
   Declarative is not the same as pure: `var`/`upd`, `print`, `once` are
   allowed. They are safe because every line of user code runs on one
   evaluator thread; the audio and render threads only ever receive values.

```vactrol
+ 43 32 > * 12 > print        # thought order: 43+32, times 12, print
* 12 {+ 43 32}               # same value; the group had to be planned
```

## 1. Basics (original sketch)

```vactrol
# this is a comment
# A comment line that starts with `#@` is an EDITOR DIRECTIVE;
# consecutive `#@` lines form one block, which applies to the nearest
# PRECEDING statement, block, or definition (fn, inst, bus) no deeper
# than the comment (same line =
# that line). Which parameters a control panel shows, MIDI mapping, etc.
# The language ignores it; see architecture.md, Editor Requirements.

# function definition: `fn name params:` opens an indented body block.
# The last expression of the body is the return value.
fn f a b:
	* a 12            # ~ (* a 12)

# function call: prefix, no parens, no commas
f 1 2               # => 24

# Decided (principle 1): tabs, one per level; no width parameter to agree
#   on, and the sketch uses them; the formatter normalizes

# binding (author, 2026-09-24): `let` is immutable, `var` is mutable,
# `upd` reassigns a var
let a 12            # ~ (let a 12); immutable
var hits 0          # mutable
upd hits {+ hits 1} # reassign; returns the new value; only a var may be upd'd
upd a 13            # error: a is a let
# No shadowing, in either direction: a name is bound once per scope
let a 13            # error: a is already bound in this scope
var hits 5          # error: hits exists; write `upd hits 5`
# Values are immutable; a var is REBOUND to a new value, never mutated
# in place (Clojure's model, one level simpler than atoms)
var arr [1 2]
upd arr {put arr 3}
# Scope by position: a top-level let/var is a namespace binding visible
# to every pattern and function; inside a body it is local. Re-evaluating a
# top-level `let` or `var` line while editing REPLACES that binding:
# that is live redefinition (loops see the new value), which is neither
# mutation nor shadowing.
# Decided: one `let`, scope by position; no `def`.
# Decided: no shadowing; rebinding is an error.
# Decided (author, 2026-09-25, amends the line above): "no shadowing" holds
# WITHIN ONE SCOPE, in either direction. Scopes form a chain:
#   prelude  (builtins, default sound kit; read-only)
#   -> session (the editor file / REPL top level; "main")
#   -> fn and block scopes
# A child scope MAY bind a name that a parent already binds; the inner
# binding wins for that scope. Diagnostics: shadowing a prelude name is a
# hint ("shadows prelude `scale`"); shadowing a user-defined parent name is
# a warning. Rebinding within the same scope stays an error, and the
# prelude itself cannot be rebound (it is the parent, not the session).
let sound-kit put default-sound-kit [bd: my-kick]   # session shadows the prelude's sound-kit
# Decided (implication 2026-09-24): yes; a top-level var is the live state
#   mechanism (late-bound, section 4), and upd returns the new value

# lambda (anonymous function): params before `->`, body after.
x b -> * x b        # like |x, b| x * b in Rust; lexical scope; prefix body
# Decided (implication 2026-09-24): prefix only, `x b -> * x b`; from
#   `{}`-only nesting and principle 2
# Decided (implication 2026-09-24): multi-line lambda: `->:` opens an indented body:
#   x ->:
#     let y {* x 2}
#     + y 1

# Every expression has a value. A value that is not referenced is
# discarded immediately.
# `_1` (the previous value) exists ONLY in the console, where a session
# reads like a transcript. In a file it is a reader error; files chain
# with `>` instead (below). (Author, 2026-09-24.)
let a 12
print a              # => 12; print returns its argument

# naming convention: kebab-case, words divided by '-'
fn fn1 a b:
	* a b             # return a * b

fn fn2 b c d:
	+ b c d           # b + c + d

# list literal: whitespace-separated, no commas. A list is callable
# with an index, as a dict is with a key (section 2).
let arr [a 12 44]
arr 1               # => 12

repeat 4 3          # => [4 4 4]; the same `repeat` as in sequences
# Decided (principle 2, revised 2026-09-24): `repeat value count`, subject
#   first, one function for lists and sequences. Clojure's `(repeat n x)`
#   order is not followed: names and orders need not follow Clojure.

# chaining (decided 2026-09-24): `>` is the pipe. The value on its left
# becomes the FIRST argument of the call on its right (thread-first). A
# line that BEGINS with `>` continues the previous expression, so a
# chain reads top to bottom with no wrapper form; the seed is simply
# the first line.
fn some-fn a:
	fn1 a 12
		> fn2 32 43     # == fn2 {fn1 a 12} 32 43
a > fn1 12 > fn2 32 43   # inline, the same thing
# `>` in HEAD position of a line or `{}` group is greater-than
# (`{> a 10}`); anywhere else it is the pipe. To pass greater-than as a
# value write `gt` (every comparison operator has a word alias).
# Decided: `>` is the pipe, head-position rule above.
# Decided: subject first everywhere (principle 2); Tidal's pattern-last
#   order is not followed.
```

## 2. Literals and Data

```vactrol
# numbers (author, 2026-09-24): fixed widths, Zig/WGSL style
#   int     i32        int64    i64
#   float   f32        float64  f64
#   ratio   int64/int64, exact
# i32 and f32 are Wasm's native scalars and f32 is the shader float, so
# visuals pay no conversion.
1  -3               # int, when nothing constrains it
2.5  0.25           # float, when nothing constrains it
let big: int64 1    # a literal adapts to its context (Zig comptime style)
slow pat 1.5        # `slow` takes a ratio; a decimal literal where a
                    # ratio is expected becomes the exact decimal (3/2)
1/4                 # ratio, exact: three steps of 1/3 land on 0, 1/3, 2/3
/ 1 3               # => 1/3; int / int is a ratio (Clojure)
# ratio is the MUSICAL number (author, 2026-09-24): beats, step lengths,
# and cycle positions are ratios inside the scheduler, so a subdivided
# sequence lands exactly (0, 1/3, 1/2, 2/3 ...) and `hold :bd 1/3` three
# times is exactly one beat. Seconds appear only at the host boundary.
+ 1/4 1/8           # => 3/8; ratio arithmetic is exact and self-reducing
* 1/4 4             # => 1; an integral ratio prints as an int
/ 6 4               # => 3/2
* 1/3 0.5           # => 0.16666667 float; a float in the expression
                    # makes the result float (exactness is gone anyway)
float 1/3           # => 0.33333334, explicit
# widening is implicit only where it reads naturally:
#   int -> int64,  float -> float64,  int -> float64   (exact)
#   int -> ratio                                        (exact)
#   ratio -> float / float64                            (lossy; only when
#                                                        a float is already
#                                                        in the expression)
#   int -> float                                        (lossy past 2^24,
#                                                        accepted so that
#                                                        `* 60 1.5` works)
# narrowing is always an explicit call: `int x`, `int64 x`, `round x`
+ 1 2.5             # => 3.5 float
* 60 1.5            # => 90.0 float
int 90.7            # => 90 (truncates); `round 90.7` => 91
# overflow of int / int64 is a failure, like division by zero
# Decided: ratio literal is in.
# Decided: fixed widths as above; no bignum.
# Decided (consistency): int64 -> float64 is implicit, like int -> float
# Decided (principle 3): a literal too large for i32 becomes int64

# strings (author, 2026-09-24): ONLY for text a human will read or see:
# console messages, on-screen text, file paths, OSC addresses, shader
# source. Names and notation are never strings: samples, synths, notes,
# chords, and scales are keywords; sequences are lists (`design-music.md`).
"note {n} at beat {beat}"        # `{}` interpolates: `{}` already means
                                 # "evaluate this" (Python f-string taste)
text "hello" > out o1            # on-screen text (`design-visual.md`)
# Decided: `{}` interpolation in every string literal

# keywords evaluate to themselves; used for sample, synth, fx, and note
# names and for named arguments. Typed like Zig enum literals: `:minor`
# where a `scale-kind` is expected checks against that enum, and
# `s :bd-haus` checks against the host's sample set, so the LSP
# completes them.
:kick  :bd-haus  :minor  :e3

# nil and booleans
nil  true  false

# ---- nil: Clojure semantics (author, 2026-09-24) --------------------
# Option IS nil. `?T` in a type means "T or nil"; there is no
# some/none wrapper and no constructor.
d :missing                       # => nil
first []                         # => nil when empty
if false "x"                     # => nil (no else)
# Truthiness: nil and false are falsy; EVERYTHING else is truthy,
# including 0, "", and []. One rule for every form:
# plain `if`, binding `if` (section 3), `and`, `or`.
# nil-punning: accessors on nil give nil, so a chain that goes nil
# stays nil instead of failing
first nil                        # => nil
nil 3                            # => nil (a list is callable with an index)
nil :k                           # => nil
len nil                        # => 0
# `and` / `or` return values, not booleans
or nil 0 5                       # => 0   (first truthy)
and 1 2 nil 3                    # => nil (first falsy, else last)
d :gain ? 1.0                    # `x ? d` == `or x d`, left to right
d :gain ? 1.0 > * 2              # `?` applies to the expression just to
                                 # its left and binds tighter than `>`
# A `?T` value must be given a default with `?` before it is used as a
# `T`: `+ {d :gain} 1` is a type diagnostic, `+ {d :gain ? 1.0} 1` is
# not. Accessors pun nil (above); arithmetic and calls do not. (Author,
# 2026-09-24: "supply a default value when nil".)
is-nil x                         # predicate (`is-` prefix)
# Decided (principle 1): no `is-some`; write `if x`

# ---- identifiers (author, 2026-09-24) --------------------------------
# identifier = [a-zA-Z][a-zA-Z0-9]* ( "-" [a-zA-Z0-9]+ )*
#   - letters and digits only, joined by "-"; no "_" anywhere
#   - no leading digit; digits allowed afterwards (fn1, d1, o0)
#   - no leading "-": that is negation, the `-` function, or `->`
#   - "-" inside a name must be followed by a name character, so
#     `a->b` lexes as `a` `->` `b` and `foo-` is not a name
#   - ASCII only in v1; uppercase allowed, kebab-lowercase by convention
# keyword    = ":" identifier            (:kick, :bd-haus)
# qualified  = identifier "." identifier  (pads.warm; only after `import`)
# language tokens (never identifiers):
#   "_"          reserved: the wildcard in patterns
#   "_" digits   previous values (_1), console only
# operators  = + - * / = < > <= >= .. -> & | ?
#   a separate token class: callable like functions, but not
#   user-definable; word aliases (gt, lt, add ...) exist for passing
#   them as values (see chaining, section 1)
fn1 12              # ok
d1 pat              # ok
my-long-name        # ok
# 1st   -foo   a->b   foo?   swap!   my_name   _tmp     # not identifiers
# `_` is the pattern wildcard and nothing else. (A 2026-09-24 decision
# making it a live-loop iteration counter is withdrawn with live-loop,
# `design-music.md`; per-cycle variation is a pattern operation.)
# Decided: no `?`/`!` in names; `is-` prefix; `?` is reserved for types and
#   the nil fallback
# Decided (principle 2): `-x` is `{neg x}`; `- x` with a space is the
#   function

# ---- path and url literals (author, 2026-09-25; Nix-style) ---------------
# A path is a first-class value, not a string: unquoted, and it must
# contain a `/`. Forms: ./x  ../x  ~/x  /abs/x  (chars [A-Za-z0-9._~-] and `/`).
# A url is `scheme://...` up to whitespace. Types: `path`, `url`.
# No grammar conflict: division is `/ a b` with a space, and a qualified
# name `pkg.name` never contains `/`.
# A relative path resolves against the FILE that contains the literal
# (as in Nix), so a sound pack can name its own samples. In the browser
# tier paths resolve through the project root.
let pack-dir ./soundpack                 # path
let kick sample ./soundpack/bd/1.wav     # path -> loaded sample (host I/O, cached by path)
let remote https://example.org/packs/x.vact   # url
# ---- blocks (author proposal, 2026-09-24) ---------------------------
# `{ ... }` is an inline block. It is the only nesting form; it replaces
# the `( )` of Lisp; `( )` is a reader error.
* 12 {+ 43 32}      # ~ (* 12 (+ 43 32))  => 900
# `:` followed by an indented body is the multi-line spelling of the
# same block, so these two are identical:
if {> a 10} {print "big"}
if {> a 10}:
	print "big"
# A block with parameters is a lambda; `{}` with no arrow has none.
map arr {x -> * x 2}
# Semantics (decided 2026-09-24, revised): a block is a zero-argument
# lambda and nothing more. `{e}` == `{-> e}`, and a trailing `:` block
# is that lambda passed as the last argument. There is no separate
# "block parameter" concept; whether a callee receives the closure or
# its value follows from the parameter's TYPE, which is inferred:
#   - a parameter the callee CALLS (`body` on a line of its own) has a
#     function type and receives the closure: `at` runs it later,
#     `maybe-do` below runs it sometimes.
#   - a parameter of any other type receives the block's VALUE: the
#     compiler forces the thunk at the call site, as in `* 12 {+ 43 32}`,
#     and inlines it when the callee is known.
# A `{}` on a statement line or in a clause body simply runs there.
# So user-defined control structures need no declaration at all:
fn maybe-do p body:             # body: fn -> any, inferred from the call
	if {< {rand} p}:
		body
at 4:
	maybe-do 0.5:
		s :crash > once
# This makes `at`, `once`, and user-defined control structures ordinary
# functions. `if` and `elif` are sugar over `match` (section 3), so they
# are neither primitives nor functions.
# KERNEL (what the spec must define; everything else is a function or
# sugar):  match  fn  let  var  upd  ->  {}  enum
# (`if`/`elif` are sugar over match; `for` is sugar over map)
# Decided (author, 2026-09-24): no block-parameter declaration; a block
#   is a thunk literal and the parameter's inferred type decides forcing
# Decided: `( )` is removed from the surface syntax; a reader error
# Decided (implication 2026-09-24): no multi-line `{}`; `:`+indent is the
#   multi-line form
# Decided (principle 1): no `;`; an inline `{}` holds one expression, more
#   goes in a `:` block
# Decided (principle 2): yes; `let name` reads the rest of its line, so no
#   plan-ahead `{}` is needed

# ---- pairs and dict (author, 2026-09-24, revised) -------------------
# `key: value` is reader sugar for the two-element list [:key value].
amp: 0.5                         # ~ [:amp 0.5]
"a": 1                           # any literal key: ~ ["a" 1]
# A dict LOOKS like a list of pairs and is typed as one by its shape: a
# list whose elements are all pairs has type `dict`. That is syntax
# only: at runtime it is a SORTED MAP (a B-tree; `BTreeMap` in Rust)
# with unique keys (author, 2026-09-24). Every list function still
# works on it and yields pairs, in KEY ORDER, not insertion order.
# Keys are ordered within a type naturally (numbers numerically,
# keywords and strings alphabetically) and across types as
# numbers < keywords < strings.
[amp: 0.5 pan: -1]               # a dict; ~ [[:amp 0.5] [:pan -1]]
dict [[:amp 0.5] [:pan -1]]      # explicit constructor for a computed pair list
[]                               # empty dict == empty list
[amp: 1 amp: 2]                  # duplicate key in a literal: a diagnostic;
                                 # the last pair wins at runtime
# lookup: a dict is callable with a key. No new syntax.
let d [amp: 0.5 pan: -1]
d :amp                           # => 0.5
d :gain                          # => nil for a missing key
d :gain ? 1.0                    # => 1.0; `?` is the default (no `get`)
# list functions
for [k v] d:                     # :amp then :pan: key order
	print k v
map d {[k v] -> v}               # => [0.5 -1]
first d                          # => [:amp 0.5], the smallest key
# ONE collection builder (author, 2026-09-24): `put` adds elements to
# a list; on a dict an element is a pair, so `put` is also assoc and
# merge. Values are immutable: it returns a new collection (a list
# keeps its order; a dict is always key-sorted).
put [1 2] 3 4                   # => [1 2 3 4]
put d amp: 0.7                  # => [amp: 0.7 pan: -1]; replaced
put d gain: 1.0 pan: 0          # => [amp: 0.7 gain: 1.0 pan: 0]; key order
put d & other                   # splat another dict: the merge
# joining MANY at once is the other builder: `join` takes one list of
# collections (on dicts, a merge of all of them)
join [[1 2] [3]]               # => [1 2 3]
# named arguments are the same sugar: `once pat gain: 0.5` carries the
# pair [:gain 0.5], so a saved parameter set splats naturally
let p [gain: 0.5 pan: -1]
once {s :crash} & p              # == once {s :crash} gain: 0.5 pan: -1
# Decided (author, 2026-09-24): `dict` is a type of its own with list
#   syntax over a sorted map (B-tree); `put` is the only update (it
#   is assoc and merge); `join` joins many; no `assoc`, `merge`, `get`,
#   or index assignment in v1
# Decided: a fn declares keyword parameters with defaults in its header,
#   Python-style (section 4, types); `& opts` remains for pass-through.
# Decided: nil, per the Clojure nil semantics

# ---- struct (author, 2026-09-24: dict and struct both accepted) ------
# A struct is a DECLARED dict (a named, closed list of pairs) with
# optional defaults. Access, patterns, list functions, and key-ordered
# iteration are identical (the checker may lay a struct out as fixed
# slots, since its keys are static);
# the declaration adds LSP completion, misspelling checks, and a name
# to match on. `[key: value]` stays for anonymous, open dicts.
struct voice:
	amp 1.0                        # field with default
	pan 0
	note                           # required field
let v voice note: 60 pan: -1     # construct: a plain call; `let` reads the
                                 # rest of its line
v :amp                           # => 1.0; same access as a dict
v :nope                          # => error: not a field of voice (closed)
put v amp: 0.5                  # => a voice; an unknown key is an error
match v:
	voice [note: p] -> note p > s :piano > once   # struct matches dict patterns
	voice [amp: a pan: p] -> [a p]
# An enum variant IS a struct tagged with the enum name:
#   enum shape:                ==   struct circle: r     (tag shape)
#     circle r                      struct rect: w h     (tag shape)
#     rect w h
# so variants may also declare defaults and be built with named args.
# Decided: plain call `voice note: 60`; `{}` is only a block
# Decided (consistency): a struct or variant constructor follows the fn
#   header rule: fields without defaults are positional or named, fields
#   with defaults are named
# Decided (principle 3): no; controls like `gain 0.5` stay, structs are for
#   user data

# named (keyword) arguments (decided 2026-09-24): trailing `name: value`
# pairs, which are the pairs of the dict section
once {s :crash} at: 4 gain: 0.5  # ~ (once {s :crash} [:at 4] [:gain 0.5])
# `:` at end of line opens a block; `name:` followed by a value on the
# same line is a pair. `slot :drums gain: 0.8:` is therefore legal.
# The callee declares keyword parameters in its header (section 4).
# Decided: `name: value`; `:name value` rejected.

# ---- errors (author, 2026-09-24; principle 3) ------------------------
# There is no error TYPE in the language. This is live coding, not a
# system: a failure is something the runtime reports, not a value the
# program handles.
# A failing expression UNWINDS: it leaves the expression, the chain,
# and the enclosing calls up to the top level (or the current pattern
# event), where the runtime reports it with its origin (source
# position, loop name, beat) and the session continues. Chains need no
# annotation and no unwrapping; results are never wrapped.
parse "x" > * 2 > d1             # parse fails; nothing after it runs;
                                 # d1's slot is untouched; the console
                                 # and the editor show the failure
/ 1 0                            # fails (division by zero)
+ 1 "a"                          # fails (type)
# "absent" cases are nil, not failures
let xs [1 2 3]
xs 99                            # => nil (index past the end)
d :missing                       # => nil
d :gain ? 1.0                    # `?` supplies a fallback for nil
# In the console, `_1` is not written by an expression that failed; it
# keeps the value of the last expression that completed.
# No try/catch, no error object, no user-raised errors in v1. Failures
# are surfaced by tooling instead: see architecture.md "Realtime
# Validation" (static LSP checks, a dry run against a no-op host
# before a pattern is swapped in, runtime diagnostics pushed back to
# the editor with slot and beat).
# in types (section 4): `?int` means "may be nil". Nothing marks failure.
# Decided: exceptions are a runtime mechanism only; no error type, no
#   try/catch, no error/fail call in v1
# Decided (principle 1): no `x ! fallback`; with no error type there is
#   nothing to catch, and failures are surfaced by tooling. `!` stays unused
# Decided (principle 3): a failing event is dropped and reported; the slot
#   keeps playing
# Decided: arithmetic and type faults fail; misses are nil

# ranges
0..8                # => [0 1 2 3 4 5 6 7]
0..                 # open range: 0, 1, 2, ... lazy and infinite; consume
                    # it with take / take-while / find (section 3)
# Decided (taste): exclusive end, as in Python, Rust, and Zig

# comparison and arithmetic are ordinary prefix calls
+ 1 2 3             # => 6
= a 12              # => true
and {> a 1} {< a 100}
```

## 3. Control Flow: `if`, `match`, and Iteration

Decisions so far (author, 2026-09-24): iteration is declarative (`map`,
`filter`, `reduce`, `find`; `for` as sugar over `map`), never a loop with
`break`, and never recursion-based. Function recursion remains allowed for
recursive data. Everything here is an expression with a value.

```vactrol
# ---- if ----------------------------------------------------------

# `if` is SUGAR for `match` (author, 2026-09-24). The expander rewrites
# it; the evaluator never sees `if`. This also fixes truthiness
# in exactly one place: the desugaring.
if {> a 10} "big" "small"        # => "big" or "small"
# ==  match {> a 10}:
#       false | nil -> "small"
#       _ -> "big"
if {> a 10} {once {s :bd}} {once {s :sd}}  # a `{}` in clause-body position runs
                                 # where it stands; only the chosen
                                 # clause runs, so no block parameter
                                 # is needed (see section 2)
# `if c X` with no else  ==  `_ -> X`, otherwise nil
# `elif`                 ==  a nested match in the else clause
# Decided: if/elif are sugar over match. `when` and `unless` are withdrawn
#   (author, 2026-09-24): `when c:` is `if c:` with no else, and
#   `unless c:` is `if {not c}:`.

# ---- binding `if` (Zig-style capture; author, 2026-09-24) ----------
# A pattern between the subject and `->` makes the then-branch a match
# clause: it runs with the pattern's names bound. This is how `if`
# handles optionals (nil-able values), not only booleans.
if {d :gain} g -> once {s :bd} gain: g   # g bound when {d :gain} is not nil
if {d :gain} g ->:
	once {s :bd} gain: g
else:
	once {s :bd}
# ==  match {d :gain}:
#       false | nil -> once {s :bd}
#       g -> once {s :bd} gain: g
# Any pattern is allowed, so enum values unwrap the same way:
if {parse s} ok v ->:
	print v
else:
	print "parse failed"
# ==  match {parse s}:
#       ok v -> print v
#       _ -> print "parse failed"
# The binding form uses the SAME truthiness as the plain form: nil and
# false go to else, anything else binds (Clojure `if-let`). One rule.
# Decided: the Zig-style split, where the binding form tested presence and
#   let `false` bind, is withdrawn. An optional bool cannot be unwrapped
#   with `if`; use `match` on nil if that ever matters.
# Decided (principle 1): the inline capture form has no else; use the block
#   form
# Decided (implication 2026-09-24): yes; Zig-style capture was requested

# ---- binding `let` (destructuring) ----------------------------------
let [a b] arr                      # irrefutable patterns
let [amp: a pan: p] d
# Decided (principle 1): `let` destructures bracket patterns only; variants
#   go through if/match

# multi-line form; `else:` and `elif cond:` align with `if`
if {> a 10}:
	print "big"
elif {> a 5}:
	print "medium"
else:
	print "small"

# `if` is an expression; a missing else yields nil
let label if {> a 10} "big" "small"   # `let` reads the rest of its line
let maybe if {> a 100} "huge"         # => nil when false
# Decided (implication 2026-09-24): `elif` yes; Python taste

# ---- match and pattern matching (author, 2026-09-24) ---------------
# `match` is one of the three plan-ahead forms (principle 2): the
# subject comes first, then clauses. A clause is `pattern -> body` or
# `pattern ->:` with an indented body. Clauses are tried top to bottom;
# the first pattern that matches binds its names, and its body is the
# value of the whole `match`.
match x:
	:kick -> :bd-haus
	:snare -> :sn-dub
	_ -> nil

# A clause has exactly the shape of a lambda whose parameters are a
# pattern. So lambdas accept patterns too:
map pairs {[k v] -> v}

# the pattern language
match v:
	0 -> "zero"                    # literal: number, string, keyword, nil, bool
	_ -> "anything"                # wildcard; binds nothing
	x -> x                         # bare name; binds the whole value
	[] -> "empty"                  # empty list
	[a b] -> + a b                 # list of exactly two
	[first & rest] -> first        # head and tail; `&` as in Rhombus and Clojure
	[amp: a] -> a                  # dict with key :amp; other keys ignored
	[amp: a pan: p] -> [a p]
	circle r -> * 3.14 r r         # enum variant with fields (enum below)
	rect w _ -> w                  # patterns nest anywhere
	rect [x y] h -> * x h          # field itself destructured
	x if {> x 100} -> "big"        # guard: pattern, then `if`, then a block
	:kick | :snare -> "drum"       # or-pattern
	circle r if {> r 10} -> "big circle"

# multi-line bodies
match msg:
	hit num vel ->:
		note num > gain vel > s :piano > once
	halt ->:
		hush

# no clause matches -> error, reported like any other; the
# session continues
# Decided (implication 2026-09-24): no `cond`; `elif` and `match` guards
#   cover it
# Decided (implication 2026-09-24): no matching clause is a failure,
#   reported like any other
# Decided (principle 3): guards and or-patterns are both in v1
# Decided (principle 1): no multi-clause fn in v1; write match

# ---- one pattern language, two uses (author, 2026-09-24) -------------
# `->` appears exactly where a pattern can FAIL.
#   binding forms put the name first and take only patterns that cannot
#   fail (a name, `_`, `[...]` list and dict patterns with `&`):
#     let [a b] arr
#     for [k v] d:
#   testing forms (`match`, `if`) put the subject first (principle 2)
#   and write the pattern as a clause, `pattern -> body`; any pattern:
#     match v:  circle r -> ...
#     if {parse s} ok v ->:        (one clause plus else)
#   a lambda is a one-clause match, so its parameters are patterns:
#     {[k v] -> v}
# Guards (`x if {...} ->`) exist only in `match`; an `if` that needs a
# guard is a `match`. `each` is withdrawn: `for` is the statement form
# and `map` the pipe form, so nothing iterates subject-first AND
# name-first.

# ---- enum -----------------------------------------------------------
# An enum declares variants. A variant may carry positional fields.
enum shape:
	circle r
	rect w h
	none

# variants are constructor functions; a field-less variant is a value
let c circle 5
let r rect 2 3
let u none
= {circle 5} {circle 5}          # => true; structural equality

# field access outside match: a variant is callable with a field name,
# exactly like a dict (subject first, principle 2)
c :r                             # => 5

# dispatch
fn area sh:
	match sh:
		circle r -> * 3.14 r r
		rect w h -> * w h
		none -> 0

circle 5 > area > print           # left to right: build, measure, print

# enums as pattern data: map variants to sounds
enum msg:                        # used by the match above; names chosen
	hit num vel                    # not to collide with the builtins
	halt                           # `note` and `stop`
enum drum:
	kick
	snare
	hat
fn sound-of d:
	match d:
		kick -> :bd-haus
		snare -> :sn-dub
		hat -> :hh
[kick snare kick hat] > map sound-of > s > d1
# Decided (principle 3): variant names are global; short names when typing
#   live, LSP warns on collision
# Decided: fields carry `name: type` and an optional default with the same
#   header syntax as fn parameters (`circle r: float`); see section 4,
#   types.
# Decided (implication 2026-09-24): LSP warning; failures are surfaced by
#   tooling
# Decided (principle 4): the checker knows which names are variants; a name
#   resolving to a variant is a variant pattern, any other binds

# ---- iteration (declarative; author, 2026-09-24: no break) ----------
# There is no `break`, `continue`, `while`, or `loop`. Iteration says
# WHAT it wants: `map` transforms, `filter` selects, `reduce` folds,
# `find` searches, `take`/`take-while` bound an infinite source. Nothing
# leaves a loop early because nothing is a loop.
map arr {x -> * x 2}             # => [2 24 88]
filter arr {x -> > x 20}         # => [44]
reduce arr 0 {acc x -> + acc x}  # => 68
find arr {x -> > x 20}           # => 44, the first match, or nil
any arr {x -> > x 20}            # => true
all arr {x -> > x 20}            # => false
take 0.. 3                       # => [0 1 2]; `0..` is lazy and infinite
take-while 0.. {x -> < x 3}      # => [0 1 2]
enumerate arr                    # => [[0 a] [1 12] [2 44]]

# `for` is SUGAR: `for pattern source: body` == `map source {pattern ->
# body}` with the result discarded, for bodies that exist for their
# effect (`print`, `once`). It is a statement and yields nil.
for x arr:
	print x
for i 0..8:
	print {* i i}
for [i x] {enumerate arr}:       # with index; the pattern is irrefutable
	print i x
for [k v] d:                     # a dict is a list of pairs
	print k v
# Decided (principle 2): `for x arr:`, name first like `let name value`
# Decided (principle 5, 2026-09-24): no `break`; `while`, `loop`, `each`
#   withdrawn; `for` is sugar over `map`, so the kernel has no loop form
# Decided: capturing a loop's value (`let found loop: ... break v`) went
#   with break; write `let found find items is-good`

# ---- iteration and time ------------------------------------------
# Iteration never waits (there is no sleep). Iterating an infinite
# source without `take`/`take-while`/`find` is a failure after a bounded
# number of steps, so a live session cannot hang.
for x 0..:
	print x                        # failure: unbounded source
# Decided (principle 3): yes; a live session must never hang

# ---- recursion ---------------------------------------------------
# Functions may call themselves. map/filter/reduce are preferred for
# iteration; recursion is for recursive data (trees, nested patterns).
fn fact k:
	if {<= k 1} 1 {* k {fact {- k 1}}}

fn flatten-notes tree:
	if {is-list tree}:
		map tree flatten-notes
			> join
	else:
		[tree]
# Decided (implication 2026-09-24): no TCO; loops are the iteration tool; a
#   depth limit is a failure
# Decided (principle 1): no local recursion; lift to a top-level fn
```

## 4. State, Redefinition, Modules

```vactrol
# live parameters: a top-level `var` referenced by a bound pattern is
# late-bound and read at each event (`design-music.md`, section 1)
var cutoff 800
s [:bd-haus :sn-dub] > lpf cutoff > d1
upd cutoff 400                   # heard at the next event; no re-binding
# Decided: neither atoms nor Sonic Pi `set`/`get`; `var` + `upd` is the one
#   state mechanism.

# functions are late-bound the same way: a fn passed as a pattern
# parameter is called at every event (a param may be a fn of time), so a
# redefinition is heard at the next event
fn kick-sound:                   # block form; an inline body is not allowed
	:bd-haus
s kick-sound > d1                # `kick-sound` in argument position is the fn
fn kick-sound:
	:bd-tek                        # d1 plays :bd-tek from its next event
# re-binding a slot itself (`... > d1` again) switches at the next cycle
# boundary, so a running phrase is never cut mid-way
# Decided (principle 3): late-bound names change at the next event; a
#   re-bound slot at the next cycle boundary

# a failing event is dropped and reported; the slot keeps playing
s [:bd-haus :not-a-sample] > d1  # diagnostic on :not-a-sample; kicks still play

# ---- reactive dependency graph (author, 2026-09-24) ------------------
# Changing a value changes everything computed from it: bindings that
# use it, patterns that name it, the slots those patterns are bound to,
# and the editor's displays and sliders -- and only the affected part
# is recomputed. The runtime keeps the reference structure explicitly.
let base 60
let line note [base {+ base 7}] > s :pluck   # depends on base
line > d1                                    # d1 depends on line
upd base 62                                  # line and d1's pattern update;
                                             # d1 re-binds at the cycle boundary
# Recommended structure (not signals as the substrate): a demand-driven
# incremental graph with revision stamps and early cutoff, in the Salsa
# / Adapton family, because patterns are PULLED per cycle by the
# scheduler. Dependency edges are STATIC, taken from the checker's free
# variables of each definition; dynamic edges (function values passed
# as parameters, signals) are re-evaluated per event as before. A
# change bumps the revision and marks dependents dirty; a dependent
# recomputes when demanded, and if its value is unchanged (values are
# immutable, so equality is cheap) its own dependents are left alone.
# Pull is glitch-free without height bookkeeping. PUSH is used only for
# notifications: slots, displays, and sliders subscribe to nodes and
# pull on wake. SolidJS-style signals are the mental model; the
# implementation design chooses the exact structure.
# Decided (author): the requirement. Recommended: the Salsa-style graph.

# ---- modules and packages (author, 2026-09-24; supersedes "no modules") --
# External Vactrol libraries are imported by repository path, Go style.
# A package is a git repository (or a directory of one) with a
# `vactrol.toml` manifest and `.vact` files; it can provide instruments,
# effects (chains of builtins), patterns, looks, and sample assets.
import github.com/someone/vactrol-pads                  # qualified: pads.warm
import github.com/someone/vactrol-pads as pd            # alias
import github.com/someone/vactrol-pads open             # names unqualified
note [:c3] > s pads.warm > d1
# qualified name = identifier "." identifier; the reader accepts it only
# after an import bound the prefix. Unqualified opening is for the live
# set; the LSP warns on a collision with the prelude or another package.
# Versions: git tags (semver); `vactrol get` resolves and records them in
# `vactrol.lock` (minimal version selection, as Go); packages are cached
# under ~/.vactrol/pkg/<path>@<version>. In the browser the same paths
# are fetched through a package proxy (no git); the lock file pins the
# content hash either way. Packages are Vactrol code only in v1 -- no
# native extensions -- so a package can never touch the audio thread
# except through builtins.
# Decided (author, 2026-09-24): modules and a GitHub-path package system
#   are in; the earlier "no module system in v1" decision is withdrawn.
#   Manifest fields, proxy protocol, and MVS details are for the
#   implementation design.

# ---- load: a file as a value (author, 2026-09-25) --------------------------
# `load path` evaluates a .vact file in a FRESH child scope of the prelude
# (never the caller's scope) and returns its last expression, like Nix's
# import. Relative paths inside that file resolve against that file.
# `import` stays for named code packages (Go style, above); `load` is for
# one file that IS a value, typically a sound pack (see design-music.md).
let my-pack load ./soundpack/sound-pack.vact
let sound-kit put default-sound-kit my-pack   # later keys win; shadows the prelude's sound-kit
# ---- types (principle 4: typed, with inference; author, 2026-09-24) --
# Every expression has a type the checker can name; the LSP shows it on
# hover. Annotations are optional because inference makes them
# unnecessary, not because types are. Type errors are diagnostics in
# Live mode and compile errors in Frozen mode. `any` is an explicit
# escape hatch.
# How that works (decided 2026-09-24): Live mode runs on a dynamically
# checked VM, so a redefinition that changes a signature under a
# running loop never crashes it; the static checker is realtime
# validation on top (architecture.md). Only Frozen mode uses types for
# unboxed code generation.
fn add a b:                      # inferred from the body and its uses
	+ a b
# Annotations live in the fn HEADER, Python-style: `name: type`, and
# `= default` for keyword parameters. A header contains no calls, so
# `name:` there cannot be mistaken for a pair. A parameter with a
# default is a keyword parameter; one without is positional.
fn pluck pitch: int amp: float = 1.0 pan: float = 0 -> voice:
	...
let bpm: int 120                 # annotated binding
var hits: int 0
struct voice:
	amp: float 1.0                 # field: name, type, default
	pan: float 0
	note: int
enum shape:
	circle r: float
	rect w: float h: float
# type expressions:
#   int int64 float float64 ratio bool string keyword nil
#   ?T          T or nil
#   [T]         list of T          [K: V]  dict
#   fn T U -> V function           pattern T   signal   any
# Decided: header annotations as above; the `sig` line and `{a: int}` groups
#   are withdrawn.
# Decided (principle 4): HM-lite with let-polymorphism;
#   `map`/`filter`/`first` are typed once and no syntax is added
# Decided (principle 4): `any` is used only after narrowing through match/if
#   patterns
```

## 5. Prelude: Core Vocabulary

Clojure supplies the semantics (immutability, nil, pairs, late binding);
the names are plain English (author, 2026-09-24: "names need not follow
Clojure"). Domain vocabulary keeps the names its users already know:
TidalCycles/Strudel for patterns, SuperCollider for unit generators,
Hydra for visuals. Where two domains used one word for two things, the
less common one was renamed (Hydra `repeat` -> `tile`).

| Area | Functions |
|------|-----------|
| lists and dicts | `put` (add elements, set keys, merge with `&`), `join` (join many), `len`, `first`, `last`, `tail`, `reverse`, `sort`, `map`, `filter`, `reduce`, `find`, `any`, `all`, `take`, `take-while`, `drop`, `enumerate`, `repeat value count`, `dict`; a list is callable with an index and a dict with a key; `0..8` is the range literal and `0..` a lazy infinite one |
| values | `is-nil`, `is-list`, `int`, `int64`, `float`, `round`, `neg`, `mod`, `sin`, `cos`, `min`, `max`, `abs` |
| console | `print` |
| time and slots | `use-bpm`, `use-cycle`, `use-clock`, `midi-clock-out`, `once`, `at`, `stop`, `hush`, `d1`..`d9`, `slot`, `bus`, `master` |

Domain vocabulary lives with its specification: `design-music.md`
(patterns, signals, sound) and `design-visual.md` (Hydra names).
