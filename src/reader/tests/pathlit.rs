//! Path and url literals (design 6.5.8).

use super::{codes, rd, reads_as};
use crate::reader::node::{Atom, NodeKind, TriviaKind};

fn only_atom(src: &str) -> Atom {
    let r = rd(src);
    assert!(r.diags.is_empty(), "{src:?}: {:?}", r.diags);
    match &r.nodes[0].kind {
        NodeKind::Atom(a) => a.clone(),
        other => panic!("{src:?}: not an atom: {other:?}"),
    }
}

#[test]
fn the_four_path_prefixes_and_a_url() {
    reads_as("let a ./soundpack", "(let a ./soundpack)");
    reads_as("f ../x/y.wav", "(f ../x/y.wav)");
    reads_as("f ~/kits/a", "(f ~/kits/a)");
    reads_as("f /abs/x", "(f /abs/x)");
    reads_as(
        "f https://example.org/packs/x.vact",
        "(f https://example.org/packs/x.vact)",
    );
    reads_as("f ./a/../b/./c.wav", "(f ./a/../b/./c.wav)");
    assert_eq!(only_atom("./soundpack"), Atom::Path("./soundpack".into()));
    assert_eq!(
        only_atom("https://x.org/a"),
        Atom::Url("https://x.org/a".into())
    );
    assert_eq!(only_atom("~/a"), Atom::Path("~/a".into()));
}

#[test]
fn terminators() {
    reads_as("{load ./a.vact}", "{(load ./a.vact)}");
    reads_as("f [./a ./b]", "(f [./a ./b])");
    reads_as(
        "f [sample https://x.org/a.wav]",
        "(f [sample https://x.org/a.wav])",
    );
    let r = rd("f ./a # c");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert_eq!(super::printed("f ./a # c"), "(f ./a)");
    assert_eq!(r.trivia.items.len(), 1);
    assert_eq!(r.trivia.items[0].kind, TriviaKind::Comment);
    assert_eq!(r.trivia.items[0].span.start, 6);
    let r = rd("f https://x.org/a#frag");
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    assert_eq!(
        super::printed("f https://x.org/a#frag"),
        "(f https://x.org/a)"
    );
    assert_eq!(r.trivia.items.len(), 1);
    assert_eq!(r.trivia.items[0].span.start, 17);
    // A colon after a path follows the non-key colon classes.
    assert_eq!(codes("f ./x: y"), ["misplaced-colon"]);
    assert_eq!(codes("f https://x.org/a :b"), Vec::<&str>::new());
    reads_as("f ./a\tg", "(f ./a g)");
}

#[test]
fn a_url_keeps_its_port_and_query() {
    reads_as(
        "f https://x.org:8080/a?b=1&c=2",
        "(f https://x.org:8080/a?b=1&c=2)",
    );
    reads_as("f git+ssh://h/r.git", "(f git+ssh://h/r.git)");
}

#[test]
fn malformed_paths_and_urls() {
    assert_eq!(codes("f .//x"), ["bad-path"]);
    assert_eq!(codes("f ./dir/"), ["bad-path"]);
    assert_eq!(codes("f ./"), ["bad-path"]);
    assert_eq!(codes("f ~/"), ["bad-path"]);
    assert_eq!(codes("f ../"), ["bad-path"]);
    assert_eq!(codes("f https:// x"), ["bad-url"]);
    assert_eq!(codes("f https://"), ["bad-url"]);
    super::recovers(".//x", "bad-path");
    super::recovers("https:// ", "bad-url");
    let r = rd("f ./dir/");
    assert_eq!((r.diags[0].span.start, r.diags[0].span.end), (2, 8));
}

#[test]
fn unchanged_readings() {
    reads_as("/ a b", "(/ a b)");
    reads_as("f {/ a b}", "(f {(/ a b)})");
    reads_as("f 1/4 -1/4", "(f 1/4 -1/4)");
    reads_as("f a/b", "(f a / b)");
    reads_as("0..8", "(.. 0 8)");
    reads_as("f a..b", "(f (.. a b))");
    reads_as("f amp: 0.5", "(f [:amp 0.5])");
    reads_as("f https", "(f https)");
    assert_eq!(codes("f ~"), ["stray-char"]);
    assert_eq!(codes("f ~x"), ["stray-char"]);
    assert_eq!(codes("f a:b"), ["misplaced-colon"]);
    assert_eq!(codes("f https:x"), ["misplaced-colon"]);
    assert_eq!(codes("f //x"), Vec::<&str>::new());
    // `...` is not a path: its third character is not `/`.
    assert_eq!(codes("..."), ["stray-char"]);
    // A path ends at a character outside the path set.
    assert_eq!(codes("f ./a!b"), ["stray-char"]);
    let r = rd("f ./a!b");
    assert_eq!((r.diags[0].span.start, r.diags[0].span.end), (5, 6));
    // Not at a token start: `x./a` is not a path.
    assert_eq!(codes("f x./a"), ["stray-char"]);
    assert!(!super::printed("f [a./b]").contains("./b"));
}

#[test]
fn a_path_inside_interpolation() {
    reads_as(r#"f "at {./a}""#, r#"(f (#interp "at " ./a))"#);
}

#[test]
fn the_expander_passes_them_through() {
    use crate::expand::{expand, is_kernel, ExpandCx};
    use crate::reader::print;
    let src = "let a ./soundpack\nf [./a https://x.org/a] ~/b\ng x -> load ../c.vact\n";
    let r = rd(src);
    assert!(r.diags.is_empty(), "{:?}", r.diags);
    let mut cx = ExpandCx::new(r.next_node_id());
    let out: Vec<String> = r
        .nodes
        .iter()
        .map(|n| {
            let e = expand(n, &mut cx).unwrap_or_else(|d| panic!("{d}"));
            assert!(is_kernel(&e), "{}", print(&e));
            print(&e)
        })
        .collect();
    assert_eq!(
        out,
        [
            "(let a ./soundpack)",
            "(f [./a https://x.org/a] ~/b)",
            "(-> (g x) (load ../c.vact))",
        ]
    );
}

/// A tiny deterministic generator, so the no-panic sweep needs no crate.
fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state >> 33
}

#[test]
fn no_panic_on_prefixes_and_random_path_like_input() {
    let samples = [
        "let a ./soundpack",
        "f ../x/y.wav",
        "f ~/kits/a",
        "f /abs/x",
        "f https://example.org/packs/x.vact",
        "{load ./a.vact}",
        "[./a ./b]",
        "./a # c",
        "sample https://x.org/a.wav]",
        "https://x.org/a#frag",
        ".//x ./dir/ ./ ~/ https:// x",
        "/ a b 1/4 -1/4 0..8 a..b amp: 0.5 ~ f a:b ./a!b",
    ];
    for s in samples {
        for (k, _) in s.char_indices() {
            let _ = rd(&s[..k]);
            let _ = rd(&s[k..]);
        }
    }
    let alphabet = b"/.~:ab1 -[]{}#\"hpst\t";
    let mut state = 0x5eed_u64;
    for _ in 0..200 {
        let len = usize::try_from(next(&mut state) % 24).unwrap_or(0);
        let text: String = (0..len)
            .map(|_| {
                let k = usize::try_from(next(&mut state)).unwrap_or(0) % alphabet.len();
                char::from(alphabet[k])
            })
            .collect();
        let _ = rd(&text);
    }
}
