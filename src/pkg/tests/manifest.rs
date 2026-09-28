//! `vactr.toml`: the strict subset, rendering, round trips and errors.

use std::rc::Rc;

use crate::pkg::manifest::{parse, render, valid_package_path, PackageMeta, PkgManifest};
use crate::pkg::tests::support::{id, v, DRUMS, PADS};

const FULL: &str = "# the pads package\n[package]\npath = \"github.com/someone/vactr-pads\" # trailing\nvactr = \"v0.1.0\"\nassets = [\"samples\", \"wt/#1\",]\n\n[deps]\n\"github.com/someone/vactr-drums\" = \"v1.2.0\"\n\"github.com/a/b\" = \"v0.3.1-rc.1\"\n";

#[test]
fn parses_the_subset() {
    let m = parse(FULL).expect("parses");
    let p = m.package.as_ref().expect("[package]");
    assert_eq!(p.path, id(PADS));
    assert_eq!(p.vactr, Some(v("v0.1.0")));
    let assets: Vec<&str> = p.assets.iter().map(|a| &**a).collect();
    assert_eq!(assets, ["samples", "wt/#1"]);
    // Sorted by path.
    assert_eq!(
        m.deps,
        vec![
            (id("github.com/a/b"), v("v0.3.1-rc.1")),
            (id(DRUMS), v("v1.2.0"))
        ]
    );
}

#[test]
fn renders_deterministically_and_round_trips() {
    let m = parse(FULL).expect("parses");
    let text = render(&m);
    assert_eq!(parse(&text).expect("re-parses"), m);
    assert_eq!(render(&parse(&text).expect("again")), text);
    assert!(text.find("github.com/a/b").unwrap() < text.find(DRUMS).unwrap());
    // A root manifest may omit `[package]`.
    let root = parse("[deps]\n").expect("deps only");
    assert_eq!(root, PkgManifest::default());
    assert_eq!(render(&root), "[deps]\n");
}

#[test]
fn add_or_raise_keeps_the_maximum() {
    let mut m = PkgManifest::default();
    m.add_or_raise(id(PADS), v("v1.1.0"));
    m.add_or_raise(id(DRUMS), v("v0.2.0"));
    m.add_or_raise(id(PADS), v("v1.0.0"));
    assert_eq!(
        m.deps,
        vec![(id(DRUMS), v("v0.2.0")), (id(PADS), v("v1.1.0"))]
    );
    m.add_or_raise(id(PADS), v("v1.3.0"));
    assert_eq!(m.deps[1].1, v("v1.3.0"));
    let meta = PackageMeta {
        path: id(PADS),
        vactr: None,
        assets: vec![Rc::from("s\"q")],
    };
    let with = PkgManifest {
        package: Some(meta),
        deps: m.deps.clone(),
    };
    assert_eq!(parse(&render(&with)).expect("escapes"), with);
}

#[test]
fn rejects_malformed_input_with_a_line_number() {
    let cases: &[(&str, u32)] = &[
        ("[package]\npath = \"github.com/a/b\"\n[tools]\n", 3),
        ("path = \"github.com/a/b\"\n", 1),
        ("[package]\npath = \"github.com/A/b\"\n", 2),
        ("[package]\npath = \"gitlab.com/a/b\"\n", 2),
        ("[deps]\n\"github.com/a/b\" = \"1.0\"\n", 2),
        ("[package]\npath = \"github.com/a/b\"\nname = \"x\"\n", 3),
        (
            "[package]\npath = \"github.com/a/b\"\npath = \"github.com/a/b\"\n",
            3,
        ),
        ("[deps]\n\"github.com/a/b\" = { v = \"v1.0.0\" }\n", 2),
        ("[deps]\n\"github.com/a/b\" = 'v1.0.0'\n", 2),
        ("[deps]\n\"github.com/a/b\" = \"v1.0.0\" extra\n", 2),
        ("[package]\nvactr = \"v0.1.0\"\n", 1),
        ("[deps]\n[deps]\n", 2),
        ("[deps]\n\"github.com/a/b\" = \"v1.0.0\n", 2),
        (
            "[package]\npath = \"github.com/a/b\"\nassets = \"samples\"\n",
            3,
        ),
        ("\n\n[package]\npath = [\"github.com/a/b\"]\n", 4),
    ];
    for (text, line) in cases {
        let e = parse(text).expect_err(text);
        assert_eq!(e.line, *line, "{text:?}: {e}");
    }
}

#[test]
fn package_paths() {
    assert!(valid_package_path(PADS));
    assert!(valid_package_path("github.com/a-b/c.d_e"));
    for bad in [
        "github.com/a",
        "github.com/a/b/c",
        "github.com/../b",
        "Github.com/a/b",
        "github.com/a/",
    ] {
        assert!(!valid_package_path(bad), "{bad}");
    }
}
