use crate::complete::{complete, CandidateKind, Snapshot};
fn run(src: &str) -> crate::complete::Completion {
    let p = src.find('|').unwrap_or(src.len());
    complete(&src.replace('|', ""), p, &Snapshot::builtin(), 100)
}
#[test]
fn function_and_lambda_parameters_are_visible() {
    let r = run("fn f alpha beta:\n\tal|");
    assert_eq!(r.items.first().map(|x| x.label.as_str()), Some("alpha"));
    assert_eq!(r.items.first().map(|x| x.kind), Some(CandidateKind::Local));
    let tab_only = run("fn f alpha:\n\t|");
    assert!(tab_only.items.iter().any(|item| item.label == "alpha"));
    assert!(run("map xs {x -> * x |}")
        .items
        .iter()
        .any(|x| x.label == "x"));
    assert!(run("x ->:\n\t+ x |").items.iter().any(|x| x.label == "x"));
}
#[test]
fn block_binders_and_locals_obey_cursor_order() {
    let loop_scope = run("for [k v] d:\n\t|");
    assert!(loop_scope.items.iter().any(|item| item.label == "k"));
    assert!(loop_scope.items.iter().any(|item| item.label == "v"));
    let r = run("fn f a:\n\tlet y 1\n\t|");
    assert!(r.items.iter().any(|x| x.label == "y"));
    let r = run("fn f a:\n\t|\n\tlet y 1");
    assert!(!r.items.iter().any(|x| x.label == "y"));
    let r = run("fn f a:\n\tfn g a:\n\t\t|");
    assert_eq!(r.items.iter().filter(|x| x.label == "a").count(), 1);
    assert_eq!(
        r.items
            .iter()
            .find(|x| x.label == "a")
            .map(|x| x.detail.as_str()),
        Some("g")
    );

    let sibling = run("fn f a:\n\tif true:\n\t\tlet y 1\n\tif false:\n\t\t|");
    assert!(!sibling.items.iter().any(|item| item.label == "y"));
    let outside = run("fn f a:\n\tlet y 1\nz|");
    assert!(!outside.items.iter().any(|item| item.label == "y"));
    assert!(!outside.items.iter().any(|item| item.label == "a"));
}
#[test]
fn header_defaults_and_match_pair_patterns_bind_only_values() {
    let instance = run("inst i cutoff: float = 1200 res: float = 0.3:\n\t|");
    let labels = instance
        .items
        .iter()
        .filter(|item| item.kind == CandidateKind::Local)
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"cutoff"));
    assert!(labels.contains(&"res"));
    assert!(!labels.contains(&"float"));

    let matched = run("match v:\n\tvoice [note: p] ->:\n\t\t|");
    assert!(matched.items.iter().any(|item| item.label == "p"));
    assert!(!matched.items.iter().any(|item| item.label == "note"));
}
#[test]
fn sibling_headers_and_earlier_lambdas_do_not_leak() {
    let has_local = |r: &crate::complete::Completion, name: &str| {
        r.items
            .iter()
            .any(|item| item.kind == CandidateKind::Local && item.label == name)
    };
    let r = run("fn a x:\n\tx\nfn b y:\n\t|");
    assert!(has_local(&r, "y"));
    assert!(!has_local(&r, "x"));
    let r = run("for i xs:\n\tpr i\nfor j ys:\n\t|");
    assert!(has_local(&r, "j"));
    assert!(!has_local(&r, "i"));
    let r = run("map xs {q -> * q 2}\n|");
    assert!(!has_local(&r, "q"));
    let r = run("fn f a:\n\tmap xs {q -> q}\n\t|");
    assert!(has_local(&r, "a"));
    assert!(!has_local(&r, "q"));
}
#[test]
fn typing_lambda_params_stay_visible() {
    let has_local = |r: &crate::complete::Completion, name: &str| {
        r.items
            .iter()
            .any(|item| item.kind == CandidateKind::Local && item.label == name)
    };
    let r = run("x b -> + x |");
    assert!(has_local(&r, "x"));
    assert!(has_local(&r, "b"));
    let r = run("map xs {x -> * x |");
    assert!(has_local(&r, "x"));
    let r = run("fn f a:\n\tmap xs {x -> * x |");
    assert!(has_local(&r, "x"));
    assert!(has_local(&r, "a"));
    let r = run("map xs {x -> x}\n|");
    assert!(!has_local(&r, "x"));
}
