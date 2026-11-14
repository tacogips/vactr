//! Core natives: each registration matches its native-table entry, and the
//! lang-reference examples for core natives print as annotated.

use super::*;
use crate::types::natives::{NativeKind, NativeTable};

/// Every name this wave registers.
const CORE: &[&str] = &[
    "+",
    "-",
    "*",
    "/",
    "=",
    "<",
    ">",
    "<=",
    ">=",
    "..",
    "mod",
    "neg",
    "abs",
    "min",
    "max",
    "sin",
    "cos",
    "int",
    "int64",
    "float",
    "round",
    "len",
    "first",
    "last",
    "tail",
    "reverse",
    "sort",
    "map",
    "filter",
    "reduce",
    "find",
    "any",
    "all",
    "take",
    "take-while",
    "drop",
    "enumerate",
    "repeat",
    "put",
    "join",
    "dict",
    "is-nil",
    "is-list",
    "not",
    "or",
    "and",
    "print",
    "concat",
    "d1",
    "d2",
    "d3",
    "d4",
    "d5",
    "d6",
    "d7",
    "d8",
    "d9",
    "slot",
    "once",
    "at",
    "hush",
    "stop",
    "use-bpm",
    "use-cycle",
    "use-clock",
    "midi-clock-out",
];

#[test]
fn each_registration_matches_its_table_entry() {
    let p = Prelude::core();
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let table = NativeTable::global();
    let mut names = Vec::new();
    for (id, entry) in p.natives() {
        let sig = table.sig(id).expect("a table id");
        assert_eq!(entry.sig, *sig, "{}", sig.name);
        assert_eq!(entry.sig.forcing_mask(), sig.forcing_mask());
        assert_eq!(entry.sig.keywords, sig.keywords);
        assert_eq!(sig.kind, NativeKind::Function);
        names.push(sig.name);
    }
    names.sort_unstable();
    let mut want = CORE.to_vec();
    want.sort_unstable();
    assert_eq!(names, want);
}

#[test]
fn a_misregistration_is_recorded_not_a_panic() {
    fn f(_: &mut NativeCx<'_>, _: &[Value], _: &[(KwId, Value)]) -> Result<Value, Failure> {
        Ok(Value::Nil)
    }
    let mut p = Prelude::empty();
    assert!(p.register("no-such-native", f).is_none());
    assert!(p.register("sound-kit", f).is_none());
    p.register("len", f);
    p.register("len", f);
    assert_eq!(p.errors().len(), 3);
}

/// `(source, annotated print)` pairs from lang-reference sections 1-3 over
/// `let a 12`, `let arr [a 12 44]` and `let d [amp: 0.5 pan: -1]`.
///
/// Two annotations there disagree with their own bindings and are written
/// here with the value the bindings give: `map arr {x -> * x 2}` is
/// `[24 24 88]` (the text says `[2 24 88]`), and `put d gain: 1.0 pan: 0`
/// keeps `amp: 0.5` because `d` is immutable (the text shows 0.7).
const EXAMPLES: &[(&str, &str)] = &[
    ("+ 1 2 3", "6"),
    ("= a 12", "true"),
    ("and {> a 1} {< a 100}", "true"),
    ("print a", "12"),
    ("arr 1", "12"),
    ("repeat 4 3", "[4 4 4]"),
    ("d :amp", "0.5"),
    ("d :gain", "nil"),
    ("d :gain ? 1.0", "1.0"),
    ("map d {[k v] -> v}", "[0.5 -1]"),
    ("first d", "[:amp 0.5]"),
    ("put [1 2] 3 4", "[1 2 3 4]"),
    ("put d amp: 0.7", "[amp: 0.7 pan: -1]"),
    ("put d gain: 1.0 pan: 0", "[amp: 0.5 gain: 1.0 pan: 0]"),
    ("join [[1 2] [3]]", "[1 2 3]"),
    ("dict [[:amp 0.5] [:pan -1]]", "[amp: 0.5 pan: -1]"),
    ("[]", "[]"),
    ("let xs [1 2 3]\nxs 99", "nil"),
    ("take 0..8 100", "[0 1 2 3 4 5 6 7]"),
    ("map arr {x -> * x 2}", "[24 24 88]"),
    ("filter arr {x -> > x 20}", "[44]"),
    ("reduce arr 0 {acc x -> + acc x}", "68"),
    ("find arr {x -> > x 20}", "44"),
    ("any arr {x -> > x 20}", "true"),
    ("all arr {x -> > x 20}", "false"),
    ("take 0.. 3", "[0 1 2]"),
    ("take-while 0.. {x -> < x 3}", "[0 1 2]"),
    ("enumerate arr", "[[0 12] [1 12] [2 44]]"),
    ("len [1 2 3]", "3"),
    ("len nil", "0"),
    ("last [1 2 3]", "3"),
    ("reverse [1 2 3]", "[3 2 1]"),
    ("drop [1 2 3] 1", "[2 3]"),
    ("is-nil nil", "true"),
    ("is-list [1]", "true"),
    ("int64 3", "3"),
    ("float 2", "2.0"),
    ("neg 3", "-3"),
    ("abs -3", "3"),
    ("min 3 1 2", "1"),
    ("/ 1 2", "1/2"),
    ("+ 1 1/2", "3/2"),
    ("* 60 1.5", "90.0"),
    ("mod -1 3", "2"),
    ("not nil", "true"),
    ("or nil false 3", "3"),
    ("concat \"a\" 1 :k", "a1:k"),
];

#[test]
fn lang_reference_examples_print_as_annotated() {
    let mut bad = Vec::new();
    for (src, want) in EXAMPLES {
        let mut s = Sess::new();
        s.eval("let a 12\nlet arr [a 12 44]\nlet d [amp: 0.5 pan: -1]")
            .expect("bindings");
        let got = s.show(src);
        if got != *want {
            bad.push(format!("{src:?}: got {got}, want {want}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn print_returns_its_argument_and_stages_a_console_line() {
    let mut s = Sess::new();
    assert_eq!(s.show("let a 12\nprint a"), "12");
    assert_eq!(s.sink.console(), vec!["12".to_string()]);
}

#[test]
fn integer_overflow_fails() {
    let mut s = Sess::new();
    assert_eq!(s.show("+ 2147483647 1"), "fail: overflow");
    assert_eq!(s.show("int 3000000000.0"), "fail: overflow");
}

/// Whole-pipeline cases over the kernel forms and the core natives.
#[test]
fn pipeline_cases() {
    let cases = [
        ("+ 1 2 3", "6"),
        ("* 12 {+ 43 32}", "900"),
        ("fn f a b:\n\t* a 12\nf 1 2", "12"),
        ("let a 12\nprint a", "12"),
        ("var hits 0\nupd hits {+ hits 1}\nhits", "1"),
        ("let arr [1 12 44]\narr 1", "12"),
        ("repeat 4 3", "[4 4 4]"),
        ("put [1 2] 3 4", "[1 2 3 4]"),
        ("let d [amp: 0.5 pan: -1]\nd :amp", "0.5"),
        ("let d [amp: 0.5 pan: -1]\nput d amp: 0.7", "[amp: 0.7 pan: -1]"),
        ("let d [amp: 0.5 pan: -1]\nmap d {[k v] -> v}", "[0.5 -1]"),
        ("let arr [1 12 44]\nmap arr {x -> * x 2}", "[2 24 88]"),
        ("let arr [1 12 44]\nreduce arr 0 {acc x -> + acc x}", "57"),
        ("take 0.. 3", "[0 1 2]"),
        ("take-while 0.. {x -> < x 3}", "[0 1 2]"),
        ("fn fact k:\n\tif {<= k 1} 1 {* k {fact {- k 1}}}\nfact 5", "120"),
        ("match 3:\n\t0 -> \"zero\"\n\tx -> x", "3"),
        ("match [1 2 3]:\n\t[a & rest] -> rest", "[2 3]"),
        ("enum shape:\n\tcircle r\n\trect w h\n\tnone\nfn area sh:\n\tmatch sh:\n\t\tcircle r -> * 3 r r\n\t\trect w h -> * w h\n\t\tnone -> 0\narea {rect 2 3}", "6"),
        ("/ 1 0", "fail: division-by-zero"),
        ("+ 1 \"a\"", "fail: type"),
        ("for x 0..:\n\tprint x", "fail: fuel-exhausted"),
        ("/ 1 3", "1/3"),
        ("/ 6 3", "2"),
        ("if {> 12 10} \"big\" \"small\"", "big"),
        ("let x nil\nx ? 5", "5"),
        ("let n 3\n\"n is {n}\"", "n is 3"),
        ("1 2", "fail: not-callable"),
        ("fn f a:\n\ta\nf 1 2", "fail: arity"),
        ("fn adder n:\n\tx -> + x n\nlet add3 adder 3\nadd3 4", "7"),
        ("fn total xs:\n\tvar t 0\n\tfor x xs:\n\t\tupd t {+ t x}\n\tt\ntotal [1 2 3]", "6"),
        ("fn f a b = 2:\n\t+ a b\nf 1", "3"),
        ("fn f a b = 2:\n\t+ a b\nf 1 b: 5", "6"),
        ("fn f a b = 2:\n\t+ a b\nlet o [b: 10]\nf 1 & o", "11"),
        ("fn g a b:\n\t- a b\ng & [5 2]", "3"),
        ("match :snare:\n\t:kick | :snare -> \"drum\"\n\t_ -> nil", "drum"),
        ("match 150:\n\tx if {> x 100} -> \"big\"\n\t_ -> \"small\"", "big"),
        ("match [amp: 1 pan: 2]:\n\t[amp: a pan: p] -> [a p]", "[1 2]"),
        ("struct voice:\n\tamp 1.0\n\tpan 0\n\tnote\nlet v voice note: 60 pan: -1\nv :amp", "1.0"),
        ("struct voice:\n\tamp 1.0\n\tnote\nlet v voice 60\nmatch v:\n\tvoice [note: p] -> p", "60"),
        ("enum shape:\n\tcircle r\n\tnone\nlet c circle 5\nc :r", "5"),
        ("enum shape:\n\tcircle r\n\tnone\n= {circle 5} {circle 5}", "true"),
        ("enum shape:\n\tcircle r\n\tnone\nlet u none\nmatch u:\n\tcircle r -> r\n\tnone -> 0", "0"),
        ("inst pluck freq amp = 0.5:\n\t+ amp 0\npluck 440", "0.5"),
        ("hush", "nil"),
        ("let [a b] [1 2]\n+ a b", "3"),
        ("fn f [a b]:\n\t+ a b\nf [3 4]", "7"),
        ("x b -> * x b", "<fn>"),
        ("let sq x -> * x x\nsq 5", "25"),
        ("sort [3 1 2]", "[1 2 3]"),
        ("mod 7 3", "1"),
        ("max 1 5 3", "5"),
        ("round 2.5", "3"),
        ("int 2.7", "2"),
        ("fn maybe-do body:\n\tbody\nmaybe-do {+ 1 2}", "3"),
        ("print ./a/b.wav", "./a/b.wav"),
        ("enumerate [:a :b]", "[[0 :a] [1 :b]]"),
        ("let d [amp: 0.5]\nfirst d", "[:amp 0.5]"),
        ("dict [[:amp 0.5] [:pan -1]]", "[amp: 0.5 pan: -1]"),
        ("join [[1 2] [3]]", "[1 2 3]"),
        ("0..8", "0..8"),
        ("let xs [1 2 3]\nxs 99", "nil"),
        ("tail [1 2 3]", "[2 3]"),
        ("filter [1 30 40] {x -> > x 20}", "[30 40]"),
    ];
    let mut bad = Vec::new();
    for (src, want) in cases {
        let got = show(src);
        if got != want {
            bad.push(format!("{src:?}: got {got}, want {want}"));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
