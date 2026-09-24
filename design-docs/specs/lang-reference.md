# nagamu Language Reference (working draft)

Reference code from which the language specification is reverse-engineered.
Section 1 is the original sketch. Later sections add what music and visual
live coding need, modeled on Sonic Pi, TidalCycles, Strudel, and Hydra.

Conventions used in the code blocks:

| Marker | Meaning |
|--------|---------|
| `#` | comment |
| `# QA-nn:` | open question for the language author. Answer inline below it with `# A:`; the spec picks answers up from here. `grep '# QA-'` lists all questions. |
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
   `d :amp`, `assoc d :amp 0.7`) so that pipes work without placeholders.
3. **Readable live code over type safety.** When a rule would make a chain
   longer or noisier in exchange for a static guarantee, the chain wins.
   Failures are reported by the runtime, not values threaded through every
   call and not a type in the language; types are optional annotations, never
   obligations.
4. **Typed, with inference.** nagamu is statically typed. Every expression
   has a type the checker can name and the LSP shows. Annotations are
   optional because inference makes them unnecessary, not because types are.
   In Live mode a type error is a diagnostic and never stops the music; in
   Frozen mode it is a compile error. `any` is an explicit escape hatch,
   never the default.

```nagm
+ 43 32 > * 12 > echo        # thought order: 43+32, times 12, print
* 12 {+ 43 32}               # same value; the group had to be planned
```

## 1. Basics (original sketch)

```nagm
# this is a comment

# function definition: `fn name params:` opens an indented body block.
# The last expression of the body is the return value.
fn f a b:
	* a 12            # ~ (* a 12)

# function call: prefix, no parens, no commas
f 1 2               # => 24

# QA-01: the body above is indented with a tab; later blocks use spaces.
#        Accept both? Mixed? Recommendation: spaces only, any consistent
#        width per block; a tab character is a reader error.

# binding (author, 2026-09-24): `let` is immutable, `var` is mutable,
# `upd` reassigns a var
let a 12            # ~ (let a 12); immutable
var n 0             # mutable
upd n {+ n 1}       # reassign; returns the new value; only a var may be upd'd
upd a 13            # error: a is a let
# No shadowing, in either direction: a name is bound once per scope
let a 13            # error: a is already bound in this scope
var n 5             # error: n exists; write `upd n 5`
# Values are immutable; a var is REBOUND to a new value, never mutated
# in place (Clojure's model, one level simpler than atoms)
var arr [1 2]
upd arr {conj arr 3}
# Scope by position: a top-level let/var is a namespace binding visible
# to every live-loop; inside a body it is local. Re-evaluating a
# top-level `let` or `var` line while editing REPLACES that binding:
# that is live redefinition (loops see the new value), which is neither
# mutation nor shadowing.
# QA-02: (resolved 2026-09-24) one `let`, scope by position; no `def`.
# QA-03: (resolved 2026-09-24) no shadowing; rebinding is an error.
# QA-89: `upd` from inside a live-loop body may target a top-level var
#        (this is what replaces Sonic Pi `set`/`get`). Confirm, and
#        confirm `upd` returns the new value so it can end a body.

# lambda (anonymous function): params before `->`, body after.
x b -> x * b        # like |x, b| x * b in Rust; lexical scope
# QA-04: this body is infix (`x * b`) while every other call in the
#        sketch is prefix (`* a 12`). Is infix allowed anywhere, only in
#        lambda bodies, or was it shorthand? Recommendation: prefix only
#        in v1, i.e. `x b -> * x b`. Infix can be added later as a
#        reader feature without changing the AST.
# QA-05: multi-line lambda. PROPOSED: `->:` opens a block:
#   x ->:
#     let y {* x 2}
#     + y 1
# QA-06: zero-parameter lambda. A plain block `{sample :bd-haus}` is one
#        (section 2), so no arrow form is needed. Withdrawn:
#   -> sample :bd-haus

# Every expression has a value. A value that is not referenced is
# discarded immediately.
# `_1` (the previous value) exists ONLY in the console, where a session
# reads like a transcript. In a file it is a reader error; files chain
# with `>` instead (below). (Author, 2026-09-24.)
let a 12
echo a              # => 12; echo returns its argument
# QA-07, QA-08, QA-09: (superseded) `_1` scope, `_2`.., and laziness no
#        longer arise in files. In the console `_1` is the last
#        top-level value, eager, and `_2` does not exist.

# naming convention: kebab-case, words divided by '-'
fn fn1 a b:
	* a b             # return a * b

fn fn2 b c d:
	+ b c d           # b + c + d

# array literal: whitespace-separated, no commas
let arr [a 12 44]

repeat 3 4          # (from Strudel)
# QA-10: what did this line mean: Strudel's `.repeat`, "repeat 4 three
#        times", or a placeholder for the pattern section?

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
# QA-11: (superseded) the `>:` block form and its seed question are
#        withdrawn; the first line of a chain is the seed.
# QA-12: (decided 2026-09-24) `>` is the pipe, head-position rule above.
# QA-13: (decided 2026-09-24) subject first everywhere (principle 2);
#        Tidal's pattern-last order is not followed.
```

## 2. Literals and Data

```nagm
# numbers (author, 2026-09-24): fixed widths, Zig/WGSL style
#   int     i32        int64    i64
#   float   f32        float64  f64
#   ratio   int64/int64, exact
# i32 and f32 are Wasm's native scalars and f32 is the shader float, so
# visuals pay no conversion.
1  -3               # int, when nothing constrains it
2.5  0.25           # float, when nothing constrains it
let big: int64 1    # a literal adapts to its context (Zig comptime style)
sleep 0.25          # `sleep` takes float64; the literal is float64 here
1/4                 # ratio, exact: `sleep 1/3` never drifts
/ 1 3               # => 1/3; int / int is a ratio (Clojure)
# widening is implicit only where it reads naturally:
#   int -> int64,  float -> float64,  int -> float64   (exact)
#   int -> float                                        (lossy past 2^24,
#                                                        accepted so that
#                                                        `* 60 1.5` works)
# narrowing is always an explicit call: `int x`, `int64 x`, `round x`
+ 1 2.5             # => 3.5 float
* 60 1.5            # => 90.0 float
int 90.7            # => 90 (truncates); `round 90.7` => 91
# overflow of int / int64 is a failure, like division by zero
# QA-14: (decided 2026-09-24) ratio literal is in.
# QA-90: (decided 2026-09-24) fixed widths as above; no bignum.
# QA-94: confirm the widening table, and whether int64 -> float64 is
#        implicit (lossy past 2^53). Recommendation: implicit, by the
#        same argument as int -> float.
# QA-95: literal defaults: an unconstrained `1` is int, `1.0` is float.
#        Should a literal that does not fit i32 (`3000000000`) become
#        int64 automatically or be an error? Recommendation: int64.

# strings; `{}` inside a string interpolates, because `{}` already
# means "evaluate this" (Python f-string taste, without the prefix)
"bd sd hh"
"note {n} at beat {beat}"
# QA-91: interpolation in every string literal (recommended; `\{`
#        escapes), or only behind a prefix? Mini-notation contains no
#        braces, so there is no conflict there.

# keywords evaluate to themselves; used for sample, synth, fx, and note
# names and for named arguments. Typed like Zig enum literals: `:minor`
# where a `scale-kind` is expected checks against that enum, and
# `sample :bd-haus` checks against the host's sample set, so the LSP
# completes them.
:kick  :bd-haus  :minor  :e3

# nil and booleans
nil  true  false

# ---- nil: Clojure semantics (author, 2026-09-24) --------------------
# Option IS nil. `?T` in a type means "T or nil"; there is no
# some/none wrapper and no constructor.
d :missing                       # => nil
next it                          # => nil when exhausted
if false "x"                     # => nil (no else)
# Truthiness: nil and false are falsy; EVERYTHING else is truthy,
# including 0, "", and []. (Settles QA-44.) One rule for every form:
# plain `if`, binding `if`/`while` (section 3), `and`, `or`, `when`.
# nil-punning: accessors on nil give nil, so a chain that goes nil
# stays nil instead of failing
first nil                        # => nil
nth nil 3                        # => nil
nil :k                           # => nil
count nil                        # => 0
# `and` / `or` return values, not booleans
or nil 0 5                       # => 0   (first truthy)
and 1 2 nil 3                    # => nil (first falsy, else last)
d :gain ? 1.0                    # `x ? d` == `or x d`, left to right
is-nil x                         # predicate (QA-79 naming)
# QA-88: `is-some x` as the complement of `is-nil`, or just `x`
#        (truthy)? Recommendation: neither; write `if x` and only add
#        `is-some` when an optional bool actually shows up.

# ---- identifiers (author, 2026-09-24) --------------------------------
# identifier = [a-zA-Z][a-zA-Z0-9]* ( "-" [a-zA-Z0-9]+ )*
#   - letters and digits only, joined by "-"; no "_" anywhere
#   - no leading digit; digits allowed afterwards (fn1, d1, o0)
#   - no leading "-": that is negation, the `-` function, or `->`
#   - "-" inside a name must be followed by a name character, so
#     `a->b` lexes as `a` `->` `b` and `foo-` is not a name
#   - ASCII only in v1; uppercase allowed, kebab-lowercase by convention
# keyword    = ":" identifier            (:kick, :bd-haus)
# language tokens (never identifiers):
#   "_"          reserved: wildcard in patterns, and a live-loop meaning
#                to be decided (QA-81)
#   "_" digits   previous values (_1)
# operators  = + - * / = < > <= >= .. -> & |
#   a separate token class: callable like functions, but not
#   user-definable; word aliases (gt, lt, add ...) exist for passing
#   them as values (QA-12)
fn1 12              # ok
d1 pat              # ok
my-long-name        # ok
# 1st   -foo   a->b   foo?   swap!   my_name   _tmp     # not identifiers
# QA-81: what does `_` mean inside a live-loop? Candidates:
#          (a) the iteration counter, read-only per iteration:
#                live-loop :arp:
#                  play {nth [60 64 67] _}
#                  sleep 0.25
#              replaces Sonic Pi `tick`/`look` and their ordering rules
#          (b) the loop's beat position within its cycle
#          (c) the loop handle, for `stop _` / `sync _`
#          (d) the previous iteration's last value (QA-50 said no)
#        Recommendation: (a); it is the most typed thing in a loop.
#        Note the `match` wildcard is also `_`; a pattern is a separate
#        context, so `_ -> ...` stays a wildcard even inside a loop
#        whose body uses `_` as a value. Confirm that this overlap is
#        acceptable, or choose another wildcard spelling (`else`).
# QA-79: predicate suffix `?` (`even?`, Lisp/Ruby style) is excluded by
#        the rule. This document was renamed to the `is-` prefix
#        (`is-even`). Keep `is-`, or add `?` as an allowed trailing
#        character? Recommendation: keep `is-`; one fewer rule.
#        Consequence: `!` is also out, which settles QA-36 in favour of
#        `var` + `upd` rather than `swap!`.
# QA-80: lexing `-`: `-3` is a number, `- a b` is subtraction, and `-x`
#        (minus directly before a name) needs a meaning. Recommendation:
#        `-x` is sugar for `{neg x}`; `- x` with a space is the function.

# ---- blocks (author proposal, 2026-09-24) ---------------------------
# `{ ... }` is an inline block. It is the only nesting form; it replaces
# the `( )` used elsewhere in this document (see QA-55).
* 12 {+ 43 32}      # ~ (* 12 (+ 43 32))  => 900
# `:` followed by an indented body is the multi-line spelling of the
# same block, so these two are identical:
if {> a 10} {echo "big"}
if {> a 10}:
	echo "big"
# A block with parameters is a lambda; `{}` with no arrow has none.
each arr {x -> echo x}
# Semantics (PROPOSED): a block is a zero-argument closure. The callee's
# PARAMETER decides when it runs:
#   - ordinary (strict) parameters: the block is run before the call and
#     the parameter receives its value, as in `* 12 {+ 43 32}`. For a
#     known-strict callee the compiler inlines the block; no allocation.
#   - block parameters: the callee receives the closure and runs it when
#     and as often as it likes: `live-loop` runs its body forever,
#     `with-fx` runs it inside an effect, `at` runs it later.
#   - a block parameter also accepts a plain value (`at 4 nil`);
#     a value is simply a block that has already run.
# A `{}` anywhere else (a statement line, a clause body, a strict
# argument) runs where it stands; it is just a group. Only a declared
# block parameter receives the closure.
# This makes live-loop / with-fx / at / density and user-defined control
# structures ordinary functions. `if`, `when`, `unless`, `elif`, and
# `while` are sugar over `match` and `loop` (section 3), so they are
# neither primitives nor functions.
# KERNEL (what the spec must define; everything else is a function or
# sugar):  match  loop/break  fn  let  var  upd  ->  {}  enum
# (plus `for`, unless it is written as `each` with a lambda; QA-45).
# QA-54: confirm block semantics above. Alternative: `{}` is always
#        eager grouping and only `:`-indent blocks are closures; simpler
#        to implement, but then `while` cannot be a function and inline
#        `if` branches always run.
# QA-55: DECIDED 2026-09-24: `( )` is removed from the surface syntax and
#        is a reader error. Original question: reader error in v1 (reserves
#        parens for tuples or an S-expression escape later), or an alias
#        of a single-expression `{}`? Recommendation: reader error.
# QA-56: multi-line `{` ... `}`? Recommendation: no; `{}` is single-line
#        only and `:`-indent is the one multi-line form, so the
#        formatter never has to choose.
# QA-57: several expressions in an inline block: `{let y 2; * x y}` with
#        `;`, or one expression only? Recommendation: allow `;`.
# QA-58: should `let` (and `->`) read the rest of the line as one
#        expression, the way `x -> body` already does? Then
#          let label if {> a 10} "big" "small"
#          let double x -> * x 2
#        need no outer braces. Recommendation: yes for `let`; it keeps
#        `let a 12` unchanged and reads naturally.

# ---- pairs and dict (author, 2026-09-24: `{}` is only ever a block; a
# dict IS a list of pairs) -------------------------------------------
# `key: value` is reader sugar for the two-element array [:key value].
amp: 0.5                         # ~ [:amp 0.5]
"a": 1                           # any literal key: ~ ["a" 1]
# A dict is a list whose elements are pairs. Nothing else.
[amp: 0.5 pan: -1]               # ~ [[:amp 0.5] [:pan -1]]
[[:amp 0.5] [:pan -1]]           # the same value
[]                               # empty dict == empty list
# lookup: a dict is callable with a key; finds the first pair with
# that head. No new syntax.
let d [amp: 0.5 pan: -1]
d :amp                           # => 0.5
d :gain                          # => nil for a missing key
get d :gain 1.0                  # => 1.0 with a default
# every list function already works on a dict
for [k v] d:
	echo k v
map d {[k v] -> v}               # => [0.5 -1]
first d                          # => [:amp 0.5]
# update returns a new dict (values are immutable)
assoc d :amp 0.7                 # replaces the first :amp pair
merge d [gain: 1.0]              # == concat with later keys winning
# named arguments are the same sugar: `play 60 amp: 0.5` carries the
# pair [:amp 0.5], so a saved parameter set splats naturally
let p [amp: 0.5 pan: -1]
play 60 & p                      # == play 60 amp: 0.5 pan: -1
# Performance is a runtime detail: a pair-list is indexed on first
# lookup (immutable, so the index never goes stale); small parameter
# dicts are never indexed, large tables are.
# QA-59: (revised) confirm "dict == list of pairs, pair == [:key value]".
#        Consequence: `[[:a 1]]` written by hand is indistinguishable
#        from `[a: 1]`, which is intended.
# QA-77: duplicate keys are legal in a list (`[amp: 1 amp: 2]`).
#        Recommendation: lookup returns the first match, `assoc`
#        replaces the first, `merge` appends (so later merges win on
#        lookup only if `merge` prepends). Pick: `merge` PREPENDS the
#        new pairs, so the newest value is found first and old pairs
#        stay reachable via `rest`.
# QA-78: (decided 2026-09-24) a fn declares keyword parameters with
#        defaults in its header, Python-style (section 8, types);
#        `& opts` remains for pass-through.
# QA-60: missing key -> nil (Clojure, Sonic Pi `get`) or error
#        (Rhombus)? Recommendation: nil; live code should not throw on
#        a lookup, and `get d k default` covers the rest.
# QA-61: (superseded) dict patterns `[amp: a]` are in v1; see section 3.

# ---- struct (author, 2026-09-24: dict and struct both accepted) ------
# A struct is a DECLARED dict (a named, closed list of pairs) with
# optional defaults. Access, patterns, and list functions are identical;
# the declaration adds LSP completion, misspelling checks, and a name
# to match on. `[key: value]` stays for anonymous, open dicts.
struct voice:
	amp 1.0                        # field with default
	pan 0
	note                           # required field
let v {voice note: 60 pan: -1}   # construct: a plain call (the group is only
                                 # needed because `let` takes one value)
v :amp                           # => 1.0; same access as a dict
v :nope                          # => error: not a field of voice (closed)
merge v [amp: 0.5]               # => a voice; unknown key is an error
match v:
	voice [note: n] -> play n      # struct matches dict patterns
	voice [amp: a pan: p] -> [a p]
# An enum variant IS a struct tagged with the enum name:
#   enum shape:                ==   struct circle: r     (tag shape)
#     circle r                      struct rect: w h     (tag shape)
#     rect w h
# so variants may also declare defaults and be built with named args.
# QA-74: struct construction spelling: `{voice note: 60}` (type then
#        named args), `voice note: 60` as a plain call (the struct name
#        is a constructor function; recommended), or both?
# QA-75: positional construction and patterns for structs in
#        declaration order (`voice 1.0 0 60`, `voice a p n ->`)? Enum
#        variants use positional today (QA-66). Recommendation: allow
#        positional for variants with <= 3 fields, named always; the
#        LSP shows the order.
# QA-76: is a struct with defaults the right home for synth parameter
#        sets (`struct voice` above), replacing ad-hoc named args on
#        `play`? Recommendation: no; `play 60 amp: 0.5` stays, structs
#        are for user data.

# named (keyword) arguments (decided 2026-09-24): trailing `name: value`
# pairs, which are the pairs of the dict section
play 60 amp: 0.5 release: 2      # ~ (play 60 [:amp 0.5] [:release 2])
# `:` at end of line opens a block; `name:` followed by a value on the
# same line is a pair. `with-fx :reverb mix: 0.5:` is therefore legal.
# The callee declares keyword parameters in its header (section 8).
# QA-15: (decided) `name: value`; `:name value` rejected.

# ---- errors (author, 2026-09-24; principle 3) ------------------------
# There is no error TYPE in the language. This is live coding, not a
# system: a failure is something the runtime reports, not a value the
# program handles.
# A failing expression UNWINDS: it leaves the expression, the chain,
# and the enclosing calls up to the top level (or the current live-loop
# iteration), where the runtime reports it with its origin (source
# position, loop name, beat) and the session continues. Chains need no
# annotation and no unwrapping; results are never wrapped.
parse "x" > * 2 > d1             # parse fails; nothing after it runs;
                                 # d1's slot is untouched; the console
                                 # and the editor show the failure
/ 1 0                            # fails (division by zero)
+ 1 "a"                          # fails (type)
# "absent" cases are nil, not failures
nth [1 2 3] 99                   # => nil
d :missing                       # => nil
d :gain ? 1.0                    # `?` supplies a fallback for nil
# In the console, `_1` is not written by an expression that failed; it
# keeps the value of the last expression that completed.
# No try/catch, no error object, no user-raised errors in v1. Failures
# are surfaced by tooling instead: see architecture.md "Realtime
# Validation" (static LSP checks, a dry run against a no-op host
# before a live-loop or pattern is swapped in, runtime diagnostics
# pushed back to the editor with loop and beat).
# in types (QA-40): `?int` means "may be nil". Nothing marks failure.
# QA-82: (revised) confirm: exceptions as a runtime mechanism only, with
#        no error type, no `try`/`catch`, and no `error`/`fail` call in
#        v1. Earlier models (error values with `!T`; exceptions with an
#        error struct and try/catch) are withdrawn.
# QA-83: keep `x ! fallback` as a one-line "if this fails, use that"?
#        It needs no error value. Recommendation: yes, optional and
#        cheap; `?` (nil fallback) is independent and recommended.
# QA-85: a failure mid-iteration in a live-loop: skip to the next
#        iteration immediately (QA-38) or keep the loop's timing by
#        sleeping until the next scheduled beat, as Sonic Pi does?
#        Recommendation: sleep to the next beat; a broken loop must not
#        spin.
# QA-87: confirm the split: arithmetic and type faults fail; index and
#        key misses are nil. Open case: `nth` on a RING (Sonic Pi wraps
#        around) vs an array (nil past the end). Recommendation: arrays
#        return nil; `ring arr` gives the wrapping view when wanted.

# ranges. PROPOSED
0..8                # => [0 1 2 3 4 5 6 7]
# QA-16: exclusive end (0..8 = 0..7) or inclusive? Recommendation:
#        exclusive, like Rust.

# comparison and arithmetic are ordinary prefix calls
+ 1 2 3             # => 6
= a 12              # => true
and {> a 1} {< a 100}
```

## 3. Control Flow: `if`, `match`, and Loops

Decisions so far (author, 2026-09-24): loops are an explicit construct, not
recursion. Function recursion remains allowed. Everything here is an
expression with a value, consistent with section 1.

```nagm
# ---- if ----------------------------------------------------------

# `if` is SUGAR for `match` (author, 2026-09-24). The expander rewrites
# it; the evaluator never sees `if`. This also fixes truthiness (QA-44)
# in exactly one place: the desugaring.
if {> a 10} "big" "small"        # => "big" or "small"
# ==  match {> a 10}:
#       false | nil -> "small"
#       _ -> "big"
if {> a 10} {play 60} {play 62}  # a `{}` in clause-body position runs
                                 # where it stands; only the chosen
                                 # clause runs, so no block parameter
                                 # is needed (see section 2)
# `if c X` with no else  ==  `_ -> X`, otherwise nil
# `elif`                 ==  a nested match in the else clause
# `when c: ...`          ==  `if c {...}`
# `unless c: ...`        ==  `if c nil {...}`
# QA-69: confirm `if`/`elif`/`when`/`unless` as expander sugar over
#        `match`, not primitives and not functions. Consequence: error
#        messages and the LSP must report in terms of `if`, not of the
#        expanded `match` (source maps through the expander).

# ---- binding `if` (Zig-style capture; author, 2026-09-24) ----------
# A pattern between the subject and `->` makes the then-branch a match
# clause: it runs with the pattern's names bound. This is how `if`
# handles optionals (nil-able values), not only booleans.
if {d :gain} g -> play 60 amp: g   # g bound when {d :gain} is not nil
if {d :gain} g ->:
	play 60 amp: g
else:
	play 60
# ==  match {d :gain}:
#       false | nil -> play 60
#       g -> play 60 amp: g
# Any pattern is allowed, so enum values unwrap the same way:
if {parse s} ok v ->:
	echo v
else:
	echo "parse failed"
# ==  match {parse s}:
#       ok v -> echo v
#       _ -> echo "parse failed"
# The binding form uses the SAME truthiness as the plain form: nil and
# false go to else, anything else binds (Clojure `if-let`). One rule.
# QA-70: (resolved 2026-09-24, Clojure semantics) the Zig-style split,
#        where the binding form tested presence and let `false` bind,
#        is withdrawn. An optional bool cannot be unwrapped with `if`;
#        use `match` on nil if that ever matters.
# QA-71: inline binding `if` with an else: `->` takes the rest of the
#        line, so `if x v -> A else B` cannot be parsed without an `else`
#        keyword that terminates the body. Recommendation: inline
#        binding form has no else (yields nil); use the block form.
# QA-72: `else` may bind the whole subject (`else e ->:`), which is the
#        Zig `else |err|` form. Recommendation: yes; it is one more
#        clause pattern and costs nothing.

# ---- binding `let` (destructuring) ----------------------------------
let [a b] arr                      # irrefutable patterns
let [amp: a pan: p] d
# QA-73: `let pattern value` conflicts with QA-58 (`let name` reads the
#        rest of the line as the value) when the pattern is a variant
#        (`let ok v {parse s}`: is `v` part of the pattern or the
#        value?). Recommendation: `let` destructures only bracket
#        patterns (`[...]`, which covers dicts), which cannot be mistaken for a
#        name; refutable variant patterns go through `if`/`match`.

# multi-line form; `else:` and `elif cond:` align with `if`
if {> a 10}:
	echo "big"
elif {> a 5}:
	echo "medium"
else:
	echo "small"

# `if` is an expression; a missing else yields nil
let label {if {> a 10} "big" "small"}
let maybe {if {> a 100} "huge"}  # => nil when false
# with QA-58 (let reads the rest of the line) the outer braces go:
#   let label if {> a 10} "big" "small"
# QA-17: (superseded) this asked for a lighter nesting form than `( )`.
#        Answered by the `{}` block proposal in section 2: `{> a 10}`
#        is the nesting form, and no special condition-segment rule is
#        needed. The earlier idea of reading `if > a 10:` without any
#        brackets is withdrawn; `{}` is uniform for inline and block use.
#        Resolved once QA-54 is confirmed.
# QA-41: is `elif` wanted, or is a chain of `else: if` enough? Sonic Pi
#        has none; Python users expect it. Recommendation: `elif`.

# single-branch forms (PROPOSED; from Clojure / Ruby); plain functions
when {> a 10}:
	echo "big"
unless {> a 10}:
	echo "small"

# ---- match and pattern matching (author, 2026-09-24) ---------------
# `match` is one of the three plan-ahead forms (principle 2): the
# subject comes first, then clauses. A clause is `pattern -> body` or
# `pattern ->:` with an indented body. Clauses are tried top to bottom;
# the first pattern that matches binds its names, and its body is the
# value of the whole `match`.
match x:
	:kick -> sample :bd-haus
	:snare -> sample :sn-dub
	_ -> nil

# A clause has exactly the shape of a lambda whose parameters are a
# pattern. So lambdas accept patterns too:
each pairs {[k v] -> echo k v}

# the pattern language
match v:
	0 -> "zero"                    # literal: number, string, keyword, nil, bool
	_ -> "anything"                # wildcard; binds nothing
	n -> n                         # bare name; binds the whole value
	[] -> "empty"                  # empty array
	[a b] -> + a b                 # array of exactly two
	[first & rest] -> first        # head and tail; `&` as in Rhombus and Clojure
	[amp: a] -> a                  # dict with key :amp; other keys ignored
	[amp: a pan: p] -> [a p]
	circle r -> * 3.14 r r         # enum variant with fields (enum below)
	rect w _ -> w                  # patterns nest anywhere
	rect [x y] h -> * x h          # field itself destructured
	n if {> n 100} -> "big"        # guard: pattern, then `if`, then a block
	:kick | :snare -> "drum"       # or-pattern
	circle r if {> r 10} -> "big circle"

# multi-line bodies
match msg:
	note n vel ->:
		play n amp: vel
		sleep 0.25
	stop ->:
		hush

# no clause matches -> error, reported like any other (QA-38); the
# session continues
# QA-42: (superseded) destructuring is in from v1, as listed above.
# QA-43: `cond:` (Lisp multi-branch on arbitrary tests) in addition to
#        `elif`? Recommendation: no; `elif` covers it, and `match` with
#        guards covers the rest.
# QA-62: no matching clause: error (recommended; a silent nil hides a
#        missing variant) or nil?
# QA-63: are guards (`if`) and or-patterns (`|`) both in v1?
#        Recommendation: yes; both are cheap in the matcher.
# QA-64: multi-clause `fn` as in Rhombus (`fun | fib(0): 1 | fib(n): ...`):
#          fn area:
#            circle r -> * 3.14 r r
#            rect w h -> * w h
#        i.e. a fn with no parameter list whose body is clauses matches
#        on its arguments. Recommendation: later; write `match` in v1.

# ---- enum -----------------------------------------------------------
# An enum declares variants. A variant may carry positional fields.
enum shape:
	circle r
	rect w h
	none

# variants are constructor functions; a field-less variant is a value
let s {circle 5}
let t {rect 2 3}
let u none
= {circle 5} {circle 5}          # => true; structural equality

# field access outside match: a variant is callable with a field name,
# exactly like a dict (subject first, principle 2)
s :r                             # => 5

# dispatch
fn area s:
	match s:
		circle r -> * 3.14 r r
		rect w h -> * w h
		none -> 0

circle 5 > area > echo           # left to right: build, measure, print

# enums as messages between loops
enum msg:
	note n vel
	rest beats
	stop
live-loop :player:
	match {recv :player}:
		note n vel -> play n amp: vel
		rest b -> sleep b
		stop -> stop :player
# QA-65: are variant names global functions (Rust `use Shape::*` style)
#        or namespaced (`shape.circle 5`)? Recommendation: global in v1;
#        short names matter when typing live, and the LSP can warn on a
#        collision.
# QA-66: (decided 2026-09-24) fields carry `name: type` and an optional
#        default with the same header syntax as fn parameters
#        (`circle r: float`); see section 8, types.
# QA-67: exhaustiveness: the LSP warns when a `match` on an enum misses
#        a variant; the runtime never refuses to run. Confirm.
# QA-68: can a variant with no fields be spelled `none` and also matched
#        as `none`, given a bare name in a pattern is a BINDING?
#        The matcher must know `none` is a variant. Rule: a name that
#        resolves to a variant is a variant pattern; anything else binds.
#        Alternative: require `none {}` or `:none`. Recommendation: the
#        resolution rule; it is what Rust does.

# truthiness: only nil and false are false; 0, "", [] are true
# QA-44: (decided 2026-09-24) confirmed; Clojure rule. See section 2, nil.

# ---- loops -------------------------------------------------------
# Loops are explicit constructs. They are not implemented as recursion
# and do not require the reader to think in recursion.

# for: bind each element in turn
for x arr:
	echo x
for i 0..8:
	play {+ 60 i}
	sleep 0.25
for x arr i ->:                  # with index (PROPOSED; second name)
	echo i x
# QA-45: order of the `for` line. Candidates:
#          for x arr:        (variable first, prefix-call-like)
#          for x in arr:     (`in` keyword, Python-like)
#          for arr x ->:     (trailing lambda, same shape as `each`)
#        Recommendation: `for x arr:`; no keyword, no lambda, and the
#        variable is visually first.
# QA-46: what does `for` evaluate to? Candidates: nil (side-effect
#        loop; use `map` to collect), or an array of body values
#        (comprehension). Recommendation: nil; `map`/`filter` collect,
#        which keeps `for` cheap and its intent obvious.
# QA-47: iterating dicts: `for [k v] d:` (destructuring) or `for k d:`
#        with `d k`? Recommendation: `for k v d:` two names.

# while: test before each iteration. Sugar over loop + match:
#   while c: body  ==  loop:
#                        match c:
#                          false | nil -> break
#                          _ -> body
# binding while (Zig `while (it.next()) |item|`): runs while the
# subject is present, with the pattern bound each iteration
while {next it} item ->:
	echo item
#   ==  loop:
#         match {next it}:
#           false | nil -> break
#           item -> echo item
var n 0
while {< n 10}:
	upd n {inc n}

# loop: infinite until break
loop:
	let v {poll-input}
	if {= v :quit} {break}
	echo v

# break and continue; `break v` makes the loop evaluate to v. To keep
# that value, `let` reads the rest of its line (QA-58), and a trailing
# `:` block is the last argument, exactly as in `d1:`
let found loop:
	let v {next-item}
	if {is-good v} {break v}
# QA-48: is `break v` wanted, or should a searching loop be written with
#        `find`? Recommendation: keep `break v`; it costs nothing and
#        avoids reaching for recursion.
# QA-49: (revised) `let found loop:` above depends on QA-58 (`let` reads
#        the rest of the line). Without it, a `var found nil` before the
#        loop and `upd found v` inside is the fallback. A braced
#        multi-line block (`let found {loop: ... }`) stays illegal;
#        `{}` is single-line (QA-56).
# QA-50: (superseded) `_1` is console-only, so no implicit accumulator
#        exists; a loop that accumulates uses a `var` and `upd`.

# iteration functions still exist for collecting results
each arr x ->:
	echo x
map arr {x -> * x 2}             # => [2 24 88]
filter arr {x -> > x 20}         # => [44]
reduce arr 0 {acc x -> + acc x}
# QA-18: is a trailing lambda block after other arguments allowed, as
#        in `each arr x ->:`? This is the key ergonomic form for
#        live-loop, with-fx, and at in the next sections.

# ---- loops and time ----------------------------------------------
# A plain loop inside a live session that never calls `sleep` or
# `sync` blocks the scheduler. Sonic Pi raises "loop without sleep".
live-loop :bad:
	loop:
		sample :hh                 # runtime error after N iterations
# QA-51: adopt the Sonic Pi rule (error after a bounded number of
#        iterations with no time advance) for `loop`, `while`, and
#        `for`? Recommendation: yes; a live session must never hang.

# ---- recursion ---------------------------------------------------
# Functions may call themselves. Loops are preferred for iteration;
# recursion is for recursive data (trees, nested patterns).
fn fact n:
	if {<= n 1} 1 {* n {fact {- n 1}}}

fn flatten-notes tree:
	if {is-array tree}:
		map tree flatten-notes
			> concat
	else:
		[tree]
# QA-52: tail-call optimization. Since loops exist, TCO is not needed
#        for iteration. Recommendation: no TCO guarantee in v1; the
#        evaluator enforces a depth limit and reports "recursion too
#        deep" as an ordinary error, so the session survives.
# QA-53: mutual recursion between top-level fns is fine with late-bound
#        vars (architecture.md). Local (`let`-bound) lambdas cannot
#        refer to themselves by name without a `letrec`-style form.
#        Recommendation: no local recursion in v1; lift to a top-level
#        fn.
```

## 4. Time and Scheduling (Sonic Pi model)

```nagm
# global tempo
use-bpm 120
# QA-19: Tidal/Strudel use cycles per second and measure time in cycles;
#        Sonic Pi uses BPM and beats. Which is the core unit?
#        Recommendation: beats and BPM in core; a cycle is a pattern's
#        length in beats (default 4). cps = bpm / 60 / beats-per-cycle.

# live-loop: a named loop. Redefining it replaces the body at the start
# of the next iteration; the loop's clock position is preserved.
live-loop :drums:
	sample :bd-haus
	sleep 0.5
	sample :sn-dub
	sleep 0.5
# QA-20: `live-loop :drums:` ends in two colons. Alternatives:
#          live-loop drums:       (bare name, the form quotes it)
#          live-loop "drums":
# QA-21: is `live-loop` a special form, or a function taking a trailing
#        block lambda (`live-loop :drums ->:`)? Prefer function plus
#        trailing block if QA-18 is yes; fewer special forms.

# sleep advances this loop's logical clock; it never blocks the CPU
sleep 0.25          # beats

# synchronization between loops
live-loop :bass:
	sync :drums       # wait for :drums to begin its next iteration
	play :e2 release: 1
	sleep 1

cue :beat           # broadcast an event; `sync :beat` elsewhere resumes

# one-shot scheduling relative to now
at 4 ->:            # in 4 beats
	sample :crash
at [0 1 2 3] t ->:  # several times; t is bound to each offset
	play {+ 60 t}

# stopping
stop :drums         # stop one loop
hush                # silence everything; the session stays alive

# per-iteration counters: `tick` advances a per-loop counter,
# `look` reads it without advancing
live-loop :arp:
	play {nth [60 64 67] {tick}}
	sleep 0.25
# QA-22: Sonic Pi spells this `(ring 60 64 67).tick`. In prefix style
#        pick one idiom: `nth arr {tick}` (arrays wrap around) or a
#        `ring` type with `tick arr`.

# density: run a block n times faster within the same time
density 2:
	sample :hh
	sleep 0.5
```

## 5. Sound (Sonic Pi model)

```nagm
# notes: midi numbers or note-name keywords
play 60
play :e3
play :e3 amp: 0.5 release: 2 pan: -1

# synth selection
use-synth :prophet
with-synth :tb303:
	play 40 cutoff: 80

# samples
sample :loop-amen beat-stretch: 4 rate: 1
sample :bd-haus amp: 2 rate: -1
sample "path/to/file.wav" start: 0.25 finish: 0.5

# chords and scales evaluate to arrays; an array passed to play is
# played as a chord
chord :e3 :minor                 # => [52 55 59]
scale :c :major num-octaves: 2
play {chord :e3 :minor}
play-pattern-timed {scale :c :major} [0.25]

# effects wrap a block
with-fx :reverb mix: 0.5:
	play 60
	sleep 0.5
	play 67
# QA-23: same trailing double colon as QA-20 (`mix: 0.5:`). If QA-15
#        picks `:name value` this becomes `with-fx :reverb :mix 0.5:`,
#        which is no better. Consider `with-fx :reverb mix: 0.5 ->:`.

# controlling a running sound (slides)
let n {play 60 sustain: 4 note-slide: 0.5}
sleep 1
control n note: 72

# randomness; seeded per run so a loop repeats deterministically
rrand 0.1 0.9
rand
choose [60 64 67]
one-in 4
use-random-seed 2

# midi and osc output
midi-note-on 60 velocity: 100 channel: 1
osc "/trigger" 1 0.5
```

## 6. Patterns (TidalCycles / Strudel model)

```nagm
# A pattern is a function of time to events. Patterns are lazy and
# infinite; nothing sounds until a pattern is bound to an output slot.

# mini-notation inside strings, interpreted by pattern functions
s "bd sd hh*2 [cp cp] <bd sd> bd? bd(3,8) bd:3 ~ bd@3 bd!2"
#    |  |  |     |       |      |     |      |   |  |    |
#    |  |  |     |       |      |     |      |   |  |    +- replicate step
#    |  |  |     |       |      |     |      |   |  +- weight (3 steps)
#    |  |  |     |       |      |     |      |   +- rest
#    |  |  |     |       |      |     |      +- sample index
#    |  |  |     |       |      |     +- euclidean 3 hits in 8
#    |  |  |     |       |      +- 50% chance
#    |  |  |     |       +- alternate per cycle
#    |  |  |     +- subsequence (subdivides its step)
#    |  |  +- repeat within the step
#    +- each word is one step of the cycle
# Also inside mini-notation: `,` stacks ("bd*2, hh*4"), `|` chooses
# randomly per cycle, `.` groups without brackets.
# QA-24: plain strings, or a dedicated literal so the reader and LSP can
#        lint mini-notation? PROPOSED: plain strings in v1, parsed by
#        `s`, `n`, `note`, etc. A `p"..."` literal is a later option.

# binding a pattern to an output slot (Tidal d1, Strudel $:).
# `d1` is an ordinary function: pattern first, binds it to slot 1 at
# the next cycle boundary, and RETURNS the pattern it bound. Three
# spellings of the same call:
s "bd sd" > d1                   # pipe into the sink (principle 2)
d1 {s "bd sd"}                   # plain call
d1:                              # trailing block; the block's value is
	s "bd sd"                      # its last expression, i.e. the pattern
s "bd sd" > d1 > fast 2 > d2     # d1 returns the pattern, so it can go on
# Re-running any of these lines while editing calls d1 again, which IS
# the live replacement; nothing else is needed.
# QA-25: (revised) confirm `d1`..`d9` as plain sink functions rather
#        than a special block form. Also: is a generic `slot :drums`
#        wanted alongside the numbered Tidal names? Recommendation:
#        both; `d1` == `slot 1`.

# transforming a pattern: a chain (Strudel: s("bd sd").fast(2).gain(0.8))
# ending in the sink, so the whole thing reads top to bottom
s "bd*2 [sd cp]"
	> fast 2
	> gain 0.8
	> room 0.3
	> every 4 rev
	> sometimes {fast 2}
	> d1
# QA-26: `every 4 rev` receives the pattern first, so its signature is
#        {every pat n f} and `rev` is passed as a function value that
#        takes one pattern. Confirm with QA-13.
# QA-27: `sometimes {fast 2}` needs `fast 2` to be a partially applied
#        function (pattern still missing). Rule candidates: auto-curry
#        every pattern function, or write `{p -> fast p 2}`.
#        Recommendation: auto-partial for pattern-first functions when
#        called with one argument fewer; keeps Tidal idioms writable.

# control patterns: every parameter is itself patternable
d1:
	s "bd sd"
		> n "0 1 2 3"
		> gain "1 0.8 0.6"
		> pan sine
		> speed "<1 2>"
		> lpf {range 200 2000 sine}

# combining patterns
stack [{s "bd*4"} {s "~ sd"} {s "hh*8"}]
cat [{s "bd sd"} {s "hh*4"}]     # one per cycle, in turn
fastcat [{s "bd sd"} {s "hh*4"}] # all within one cycle
superimpose {s "bd sd"} {fast 2}
off {note "c e g"} 0.25 {add 7}
jux {s "bd sd"} rev              # apply to one stereo side

# structure and probability
struct {s "bd"} "t f t t"
degrade-by pat 0.3
sometimes-by pat 0.3 {fast 2}
iter pat 4
chop pat 4
ply pat 2
chunk pat 4 {hurry 2}
whenmod pat 8 6 {fast 2}
rarely pat {fast 2}
often pat rev

# continuous signals (0..1 unless ranged)
sine  saw  tri  square  rand  perlin
irand 8
segment sine 8                   # sample a signal 8 times per cycle
range sine 1 5

# notes, scales, chords (Strudel)
note "c e g b"
	> s "piano"
n "0 2 4 <6 7>"
	> scale "C:minor"
	> s "sawtooth"
chord "<C^7 Dm7 G7>"
	> voicing
	> s "piano"
arp {chord "Cm7"} "up"

# sound parameters available on any pattern:
#   gain pan speed lpf hpf resonance room size delay delaytime
#   delayfeedback crush shape vowel legato attack release sustain
#   begin end cut orbit velocity

# tempo for the pattern engine
set-cps 0.5                      # see QA-19: whether this exists or
                                 # is derived from use-bpm

# QA-28: relation between the two time models. Sonic Pi loops are
#        imperative (sleep advances time); Tidal patterns are
#        declarative (a function of time). Support both in v1, or pick
#        one core and express the other on top of it? Recommendation:
#        patterns are the core abstraction; `live-loop` + `sleep` is
#        sugar that emits one event stream per iteration. Needs its own
#        architecture section once decided.
# QA-29: should mini-notation be usable outside sound, e.g. as a
#        general sequencing literal for visuals (`color "<red blue>"`)?
#        Recommendation: yes; a pattern of any value type.
```

## 7. Visuals (Hydra and frame-loop models)

```nagm
# hydra style: source -> transforms -> output. A chain fits directly,
# and `out o0` is the sink (the visual counterpart of `d1`), so no
# wrapper form is needed. `out` returns the chain it bound.
osc 20 0.1 0.8
	> rotate 0.5
	> kaleid 4
	> modulate {noise 3} 0.2
	> color 1 0.5 0.2
	> out o0
# sources:    osc noise voronoi shape gradient solid src
# geometry:   rotate scale pixelate repeat repeat-x repeat-y kaleid scroll
# color:      posterize shift invert contrast brightness luma thresh
#             color saturate hue colorama
# blend:      add sub layer blend mult diff mask
# modulate:   modulate modulate-repeat modulate-kaleid modulate-scroll
#             modulate-rotate modulate-scale modulate-pixelate
# outputs:    o0 o1 o2 o3; `render o0` shows one, `render` shows all
# QA-30: (revised; `v1:` withdrawn) sound sinks are `d1`..`d9`, visual
#        sinks are `out o0`..`o3`. Unify under one `slot` namespace, or
#        keep the Tidal and Hydra names that users already know?
#        Recommendation: keep both names, both implemented over one
#        slot table so `hush`/`stop` treat them alike.

# time-varying parameters: any number may be a lambda of time
osc {t -> * 20 {sin t}}
# or a signal, the same signals as section 6
osc {range sine 10 30}
	> out
# QA-31: unify parameters across music and visuals. PROPOSED: a param
#        is number | signal | pattern | (t -> number), with one coercion
#        rule shared by both domains. This keeps the spec small and lets
#        a pattern drive a visual directly.

# audio-reactive
shape 4
	> scale {+ 1 {fft 0}}            # bass band, 0..1
	> out
d1:
	s "bd*4"
		> on-trigger {flash o1}   # run visual code on each event
# QA-32: names for audio accessors: `fft n`, `amp`, `beat`? And the
#        hook name for pattern events into visuals (`on-trigger`).

# frame-loop style (Processing / p5) for an imperative alternative
draw:
	background 0
	fill 255 0 0
	circle {* 100 {sin frame-time}} 200 50
	rect 10 10 50 50
# QA-33: `draw:` is a hot-swappable block like `live-loop`. Same
#        mechanism? Recommendation: `draw` == `live-loop :draw` whose
#        sleep is one frame on the frame clock.

# shaders
shader :plasma:
	"""
	precision mediump float;
	uniform float time;
	void main() { ... }
	"""
src {shader :plasma}
	> out
# QA-34: multi-line string syntax (`"""`), and whether raw GLSL is in
#        scope for v1 or Hydra-style operators only. Recommendation:
#        operators only in v1; GLSL via a host capability later.

# browser and native share this API; the host supplies the canvas
# QA-35: window/canvas configuration (size, fps) is a host capability
#        setting or a language-level call (`use-fps 60`)?
```

## 8. State, Redefinition, Modules

```nagm
# mutable state that survives redefinition of the code that uses it: a
# top-level `var` (section 1). Re-evaluating the `var` line resets it;
# re-evaluating the loop that uses it does not.
var counter 0
live-loop :count:
	upd counter {+ counter 1}
	sleep 1
# QA-36: (resolved 2026-09-24) neither atoms nor Sonic Pi `set`/`get`;
#        `var` + `upd` is the one state mechanism.

# redefining a fn while a live-loop calls it takes effect on the next call
fn kick:
	sample :bd-haus
live-loop :k:
	kick
	sleep 1
fn kick:
	sample :bd-tek                 # :k plays :bd-tek from its next iteration
# QA-37: when a live-loop body itself is redefined mid-iteration: swap
#        at the next iteration start (Sonic Pi) or immediately?
#        Recommendation: next iteration start.

# errors inside a loop are reported; the loop continues its schedule
live-loop :safe:
	play :not-a-note               # error printed, loop keeps running
	sleep 1
# QA-38: after an error, does the iteration continue to the next line
#        or skip to the next iteration? Recommendation: skip to the next
#        iteration so `sleep` timing is never half-applied.

# modules
use :nagamu.music
# QA-39: module system scope for v1. Recommendation: none; music and
#        visual functions are globals in a live session, and `use` is
#        added when Frozen mode needs explicit dependencies.

# ---- types (principle 4: typed, with inference; author, 2026-09-24) --
# Every expression has a type the checker can name; the LSP shows it on
# hover. Annotations are optional because inference makes them
# unnecessary, not because types are. Type errors are diagnostics in
# Live mode and compile errors in Frozen mode. `any` is an explicit
# escape hatch.
fn add a b:                      # inferred from the body and its uses
	+ a b
# Annotations live in the fn HEADER, Python-style: `name: type`, and
# `= default` for keyword parameters. A header contains no calls, so
# `name:` there cannot be mistaken for a pair. A parameter with a
# default is a keyword parameter; one without is positional.
fn play note: int amp: float = 1.0 pan: float = 0 -> voice:
	...
let bpm: int 120                 # annotated binding
var count: int 0
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
# QA-40: (decided 2026-09-24) header annotations as above; the `sig`
#        line and `{a: int}` groups are withdrawn.
# QA-92: inference model: monomorphic signatures inferred per fn (Zig-
#        like, no generics) or Hindley-Milner with let-polymorphism, so
#        `map`, `filter`, `first` are typed once for every T?
#        Recommendation: HM-lite; the library needs polymorphism and HM
#        adds no syntax.
# QA-93: `any`: may a value typed `any` be used directly, or only after
#        narrowing through `match`/`if` patterns? Recommendation:
#        narrowing only; otherwise `any` leaks everywhere.
```
