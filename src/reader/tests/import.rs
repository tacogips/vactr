//! Imports, qualified names and `#@` trivia.

use super::{codes, rd, reads_as};
use crate::reader::{prescan_imports, read, AliasEnv, NodeKind, TriviaKind};

#[test]
fn import_forms() {
    reads_as(
        "import github.com/someone/vactr-pads",
        "(#import \"github.com/someone/vactr-pads\")",
    );
    reads_as(
        "import github.com/someone/vactr-pads as pd",
        "(#import \"github.com/someone/vactr-pads\" as pd)",
    );
    reads_as(
        "import github.com/someone/vactr-pads open",
        "(#import \"github.com/someone/vactr-pads\" open)",
    );
    let r = rd("import github.com/someone/vactr-pads as pd open  # comment");
    match &r.nodes[0].kind {
        NodeKind::Import(decl) => {
            assert_eq!(&*decl.prefix, "pd");
            assert!(decl.open);
            assert_eq!(decl.alias.as_deref(), Some("pd"));
        }
        other => panic!("not an import: {other:?}"),
    }
    let plain = rd("import github.com/someone/vactr-pads");
    match &plain.nodes[0].kind {
        NodeKind::Import(decl) => {
            assert_eq!(&*decl.prefix, "pads");
            assert!(!decl.open);
        }
        other => panic!("not an import: {other:?}"),
    }
}

#[test]
fn fresh_document_binds_its_own_imports() {
    reads_as(
        "import github.com/someone/vactr-pads\nnote [:c3] > s pads.warm > d1",
        "(#import \"github.com/someone/vactr-pads\")\n(d1 (s (note [:c3]) pads.warm))",
    );
    reads_as(
        "import github.com/someone/vactr-pads as pd\nnote [:c3] > s pd.warm > d1",
        "(#import \"github.com/someone/vactr-pads\" as pd)\n(d1 (s (note [:c3]) pd.warm))",
    );
    // A session alias from the caller's environment also binds.
    let mut env = AliasEnv::new();
    env.bind("pads".into(), "github.com/someone/vactr-pads".into());
    let r = read("s pads.warm", super::FILE, &env);
    assert!(r.diags.is_empty());
    // The caller's environment is not changed by `read`.
    let r2 = read(
        "import github.com/a/vactr-b\nb.c",
        super::FILE,
        &AliasEnv::new(),
    );
    assert!(r2.diags.is_empty());
}

#[test]
fn import_errors() {
    assert_eq!(codes("s pads.warm"), ["unbound-qualifier"]);
    // Use before the import is unbound.
    assert_eq!(
        codes("s pads.warm\nimport github.com/someone/vactr-pads"),
        ["unbound-qualifier"]
    );
    for bad in [
        "import github.com/Someone/pads",
        "import github.com/../pads",
        "import pads",
        "import github.com//pads",
        "import github.com/someone/pads extra",
        "import github.com/someone/pads as 1x",
        "import github.com/some one/pads",
        "import github.com/someone/x.y",
        "import",
    ] {
        assert_eq!(codes(bad), ["bad-import"], "{bad:?}");
    }
    assert_eq!(
        codes("f:\n\timport github.com/a/b"),
        ["import-not-top-level"]
    );
    // `_` is a path character (6.5.4), not a name character.
    reads_as(
        "import github.com/some_one/vactr-pads",
        "(#import \"github.com/some_one/vactr-pads\")",
    );
    // A failed import binds nothing.
    assert_eq!(
        codes("import github.com/Bad/pads\ns pads.warm"),
        ["bad-import", "unbound-qualifier"]
    );
}

#[test]
fn prescan_finds_every_top_level_import() {
    let doc = "import github.com/a/vactr-pads\n\
               # comment\n\
               import github.com/b/drums as dr open\n\
               s [:bd] > d1\n\
               fn f:\n\
               \timport github.com/c/inner\n\
               import github.com/Bad/x\n\
               import github.com/d/fx   # trailing comment\r\n";
    let decls = prescan_imports(doc);
    let prefixes: Vec<&str> = decls.iter().map(|d| &*d.prefix).collect();
    assert_eq!(prefixes, ["pads", "dr", "fx"]);
    assert!(decls[1].open);
    assert_eq!(&*decls[2].path, "github.com/d/fx");
}

#[test]
fn directives_land_in_trivia() {
    let r = rd("# plain\nlet g 1 #@ panel: gain\n\t#@ midi: cc 74\nprint g");
    let kinds: Vec<TriviaKind> = r.trivia.items.iter().map(|t| t.kind).collect();
    assert_eq!(
        kinds,
        [
            TriviaKind::Comment,
            TriviaKind::Directive,
            TriviaKind::Directive
        ]
    );
    let spans: Vec<(u32, u32)> = r
        .trivia
        .items
        .iter()
        .map(|t| (t.span.start, t.span.end))
        .collect();
    assert_eq!(spans, [(0, 7), (16, 30), (32, 46)]);
    assert!(r.diags.is_empty());
    assert_eq!(super::print_all(&r.nodes), "(let g 1)\n(print g)");
}
