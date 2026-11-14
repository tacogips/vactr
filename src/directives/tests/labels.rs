//! Labels: explicit and implicit in one namespace; `duplicate-label` for
//! explicit/explicit and explicit/implicit collisions, with rejected
//! references and positional fallback.

use crate::directives::attach::{reset_top_of_steps, top_of_steps};
use crate::directives::labels::{LabelKind, LabelLookup};

use super::{codes, shown, table};

#[test]
fn explicit_and_implicit_labels_resolve() {
    let src = "let tempo 120
fn f a b:
\t* a b
inst analog cutoff: float = 1200 res: float = 0.3:
\tvco :saw freq > ladder cutoff res > * amp
bus :verb:
\tdelay 0.3
s [:hh] > slot :lead
s [:bd] > lpf 800 > d1                     #@ kick:
s [:sd] > hpf 300 > d2
#@ name snare
";
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    for name in ["tempo", "f", "analog", "verb", "lead", "kick", "snare"] {
        assert!(
            matches!(t.labels.resolve(name), LabelLookup::Target(_)),
            "{name} resolves"
        );
    }
    let LabelLookup::Target(analog) = t.labels.resolve("analog") else {
        panic!("analog");
    };
    assert_eq!(
        analog.kind,
        LabelKind::Definition {
            params: vec!["cutoff".into(), "res".into()]
        }
    );
    let LabelLookup::Target(f) = t.labels.resolve("f") else {
        panic!("f");
    };
    assert_eq!(
        f.kind,
        LabelKind::Definition {
            params: vec!["a".into(), "b".into()]
        }
    );
    assert_eq!(t.labels.resolve("nosuch"), LabelLookup::Unknown);
    // Exact match only.
    assert_eq!(t.labels.resolve("kic"), LabelLookup::Unknown);

    // The block-following `#@ name` labels the preceding statement, and an
    // addressed reference resolves through it.
    let src2 = format!("{src}#@ snare.hpf cc: 9\n#@ kick.lpf cc: 8\n");
    let (t, diags) = table(&src2);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        shown(&src2, &t),
        [
            "snare.hpf.1.cutoff cc:9 ch:-",
            "snare.hpf.1.q cc:- ch:-",
            "kick.lpf.1.cutoff cc:8 ch:-",
            "kick.lpf.1.q cc:- ch:-",
        ]
    );
}

#[test]
fn duplicate_explicit_labels() {
    let src = "s [:hh] > hpf 2000 > d2   #@ hats:
s [:oh] > hpf 3000 > d3   #@ hats:
#@ hpf cc: 30
#@ hats.hpf cc: 31
";
    let (t, diags) = table(src);
    // The collision, then the rejected addressed reference.
    assert_eq!(codes(&diags), ["duplicate-label", "duplicate-label"]);
    assert_eq!(t.labels.resolve("hats"), LabelLookup::Ambiguous);
    // The positional directive still binds, by positional provenance.
    assert_eq!(
        shown(src, &t),
        ["@2:11.cutoff cc:30 ch:-", "@2:11.q cc:- ch:-"]
    );
    assert!(t.resolved.iter().all(|r| r.key.is_none()));
}

#[test]
fn explicit_label_colliding_with_an_implicit_one() {
    let src = "let hats 1
s [:hh] > hpf 2000 > d2   #@ hats: hpf cc: 30
#@ hats.hpf cc: 31
";
    let (t, diags) = table(src);
    assert_eq!(codes(&diags), ["duplicate-label", "duplicate-label"]);
    assert_eq!(t.labels.resolve("hats"), LabelLookup::Ambiguous);
    assert_eq!(
        shown(src, &t),
        ["@2:11.cutoff cc:30 ch:-", "@2:11.q cc:- ch:-"]
    );

    // Renaming one side restores the key.
    let fixed = src.replace("let hats 1", "let hat 1");
    let (t, diags) = table(&fixed);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        shown(&fixed, &t),
        ["hats.hpf.1.cutoff cc:31 ch:-", "hats.hpf.1.q cc:- ch:-"]
    );
}

#[test]
fn unknown_label() {
    let (_, diags) = table("s [:hh] > hpf 1 > d1\n#@ nosuch.hpf cc: 1\n");
    assert_eq!(codes(&diags), ["unknown-label"]);
}

#[test]
fn building_labels_for_twenty_thousand_targets_uses_bounded_top_of_steps() {
    let src = (0..20_000)
        .map(|index| format!("let a{index} {index}"))
        .collect::<Vec<_>>()
        .join("\n");
    reset_top_of_steps();
    let (table, diags) = table(&src);
    let steps = top_of_steps();
    assert!(diags.is_empty(), "{} diagnostics", diags.len());
    let targets = table.doc.targets.len();
    println!("label top_of steps: steps={steps}, targets={targets}");
    assert!(targets >= 20_000, "targets={targets}");
    assert!(
        steps <= 2 * targets as u64,
        "steps={steps}, targets={targets}"
    );
}
