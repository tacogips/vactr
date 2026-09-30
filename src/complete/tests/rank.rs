use crate::complete::{
    complete, Candidate, CandidateKind, ContextKind, DocName, PackageNames, Snapshot,
};
#[test]
fn ranking_is_stable_tiered_and_capped() {
    let mut s = Snapshot::builtin();
    s.document.push(DocName {
        label: "delta-doc".into(),
        kind: CandidateKind::Value,
        detail: "doc".into(),
    });
    let a = complete("de", 2, &s, 100);
    let b = complete("de", 2, &s, 100);
    assert_eq!(a.items, b.items);
    assert_eq!(a.items.first().map(|x| x.label.as_str()), Some("delta-doc"));
    let full = complete("", 0, &s, 0);
    assert_eq!(full.items.len(), 100);
    assert!(full.incomplete);
    assert_eq!(complete("", 0, &s, 3).items.len(), 3);
}
#[test]
fn keyword_controls_and_qualified_candidates_exist() {
    let s = Snapshot::builtin();
    let k = complete("s :", 3, &s, 100);
    assert_eq!(k.context, ContextKind::Keyword);
    assert!(k.items.iter().any(|x| x.label == ":analog"));
    let first_control = k
        .items
        .iter()
        .position(|item| item.detail == "control")
        .expect("builtin controls are available");
    assert!(k.items[..first_control].iter().all(|item| {
        matches!(
            item.detail.as_str(),
            "sound" | "synth" | "template" | "document synth"
        )
    }));
    let p = complete("s :analog > cu", 14, &s, 100);
    assert_eq!(
        p.items.first().map(|x| x.kind),
        Some(CandidateKind::Control)
    );
    let mut s = Snapshot::builtin();
    s.packages.push(PackageNames {
        prefix: "drums".into(),
        path: "repo/drums".into(),
        names: vec!["kick".into(), "snare".into()],
    });
    assert!(complete("drums.s", 7, &s, 100)
        .items
        .iter()
        .any(|x| x.label == "drums.snare"));
}

#[test]
fn match_classes_tiers_and_label_ties_are_ordered() {
    let mut snapshot = Snapshot::builtin();
    for label in ["delta-prefix", "lpg-decay", "d-x-e-fuzzy"] {
        snapshot.document.push(DocName {
            label: label.to_owned(),
            kind: CandidateKind::Value,
            detail: "document".to_owned(),
        });
    }
    let ranked = complete("de", 2, &snapshot, 100).items;
    let prefix = ranked
        .iter()
        .position(|item| item.label == "delta-prefix")
        .unwrap_or(usize::MAX);
    let subword = ranked
        .iter()
        .position(|item| item.label == "lpg-decay")
        .unwrap_or(usize::MAX);
    let fuzzy = ranked
        .iter()
        .position(|item| item.label == "d-x-e-fuzzy")
        .unwrap_or(usize::MAX);
    assert!(prefix < subword && subword < fuzzy);

    snapshot
        .document
        .extend(["al-z", "al-a"].into_iter().map(|label| DocName {
            label: label.to_owned(),
            kind: CandidateKind::Value,
            detail: "document".to_owned(),
        }));
    let local_tiers = complete("fn f alpha:\n\tal", 15, &snapshot, 100).items;
    let local = local_tiers
        .iter()
        .position(|item| item.label == "alpha")
        .unwrap_or(usize::MAX);
    let document = local_tiers
        .iter()
        .position(|item| item.label == "al-z")
        .unwrap_or(usize::MAX);
    let prelude = local_tiers
        .iter()
        .position(|item| item.label == "all")
        .unwrap_or(usize::MAX);
    assert!(local < document && document < prelude);

    let tie_order = complete("al", 2, &snapshot, 100).items;
    let earlier = tie_order
        .iter()
        .position(|item| item.label == "al-a")
        .unwrap_or(usize::MAX);
    let later = tie_order
        .iter()
        .position(|item| item.label == "al-z")
        .unwrap_or(usize::MAX);
    assert!(earlier < later);
}
#[test]
fn document_header_keys_follow_arity_and_prior_pair_rules() {
    let snapshot = Snapshot::builtin();
    let function = run_completion("fn f alpha gain: 0.5:\nf 1 g|", &snapshot);
    assert_eq!(function.context, ContextKind::PairKey);
    assert!(function.items.iter().any(|item| item.label == "gain:"));

    let instance = run_completion(
        "inst voice cutoff: 1.0 color: 0.5:\nvoice cutoff: 1 co|",
        &snapshot,
    );
    assert_eq!(instance.context, ContextKind::PairKey);
    assert!(instance.items.iter().any(|item| item.label == "color:"));

    let before_key_position = run_completion("inst voice cutoff: 1.0:\nvoice c|", &snapshot);
    assert_eq!(before_key_position.context, ContextKind::Argument);
    assert!(!before_key_position
        .items
        .iter()
        .any(|item| item.label == "cutoff:"));
}

fn run_completion(source: &str, snapshot: &Snapshot) -> crate::complete::Completion {
    let cursor = source.find('|').unwrap_or(source.len());
    complete(&source.replace('|', ""), cursor, snapshot, 100)
}
#[test]
fn candidate_contract_shape() {
    let c = Candidate {
        label: "a".into(),
        kind: CandidateKind::Local,
        detail: "fn".into(),
        insert: "a".into(),
    };
    assert_eq!(c.label, "a");
}
