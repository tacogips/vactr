//! Directives through the session (design 13.5, 14.5.8; TASK-009 criteria
//! 4-6, the session half): label keys survive a line move reported by
//! `doc-changed`, lint never changes evaluation, and `learn` in both
//! persistence modes.

use super::support::{offset, site_at, stales, Rig, DOC};
use crate::directives::key::{BindingIdent, BindingKey, KeyState};
use crate::directives::persist::{apply_edits, TextEdit};
use crate::dsp::caps::CapabilitySet;
use crate::session::protocol::{ClientMsg, LearnBody, LearnTarget, ServerMsg, StaleReason};
use crate::session::session::{PersistenceMode, SessionConfig};

fn learn(rig: &mut Rig, binding: LearnTarget, cc: u8, epoch: u64) -> Vec<ServerMsg> {
    rig.send(ClientMsg::Learn(LearnBody {
        file: DOC.to_string(),
        binding,
        cc,
        ch: None,
        edit_epoch: epoch,
    }))
}

#[test]
fn a_label_key_survives_a_line_move_that_re_keys_a_positional_site() {
    let mut rig = Rig::new();
    let old = "s [:hh] > lpf 800 > hpf 100 > d2   #@ hats:\n#@ hats.lpf cc: 5\n";
    let (r1, _) = rig.eval(old, 1, 1);
    let lpf = site_at(&r1, old, "800").clone();
    let hpf = site_at(&r1, old, "100").clone();
    assert_eq!(lpf.key.as_deref(), Some("hats.lpf.1.cutoff"));
    assert!(hpf.key.is_none(), "positional provenance");
    // Two lines inserted above move the labeled line down.
    let new = format!("let a 1\nlet b 2\n{old}");
    rig.doc_changed(2, 1, &[(0, 0, 16)], &[(0, 16)], 2);
    let (r2, _) = rig.eval(&new, 2, 2);
    let key = BindingKey::site("hats", "lpf", 1, "cutoff");
    let doc = rig.s.doc(DOC).expect("doc");
    let entry = doc.keys.get(&key).expect("the label key is tracked");
    assert_eq!(entry.state, KeyState::Live);
    let at = offset(&new, "lpf", 0);
    assert_eq!(entry.span, (at, at + 3), "migrated to the moved line");
    let lpf2 = site_at(&r2, &new, "800");
    assert_eq!(
        lpf2.key.as_deref(),
        Some("hats.lpf.1.cutoff"),
        "same identity"
    );
    // The positional site is re-keyed: a new id at the shifted span.
    let hpf2 = site_at(&r2, &new, "100");
    assert_ne!(hpf2.id, hpf.id);
    assert_eq!(hpf2.span.start, hpf.span.start + 16);
    // An edit touching the keyed site's head marks the key stale.
    let at = offset(&new, "lpf", 0);
    rig.doc_changed(3, 2, &[(at, at + 3, 3)], &[(at, at + 3)], 3);
    let edited = new
        .replacen("lpf", "hpf", 1)
        .replacen("hpf 100", "lpf 100", 1);
    rig.eval(&edited, 3, 3);
    let doc = rig.s.doc(DOC).expect("doc");
    let stale = doc
        .keys
        .iter()
        .chain(doc.keys.superseded().iter().map(|(k, e)| (k, e)))
        .any(|(_, e)| e.state == KeyState::Stale);
    assert!(stale, "a touched mapping is stale: {:?}", doc.keys);
}

#[test]
fn unknown_site_param_and_label_directives_warn_and_never_change_evaluation() {
    let plain = "s [:analog] > note [60 64] > lpf 800 > d1\nlet x 5\n";
    let noisy = "s [:analog] > note [60 64] > lpf 800 > d1   #@ lead:\n#@ lpf cc: 1 2 3\n#@ nobody.lpf cc: 1\n#@ lead.zzz cc: 2\nlet x 5\n";
    let mut a = Rig::new();
    let (ra, _) = a.eval(plain, 1, 0);
    let mut b = Rig::new();
    let (rb, _) = b.eval(noisy, 1, 0);
    let codes: Vec<&str> = rb.diagnostics.iter().map(|d| d.code.as_str()).collect();
    for code in [
        "unknown-label",
        "unknown-directive-site",
        "unknown-parameter",
    ] {
        assert!(codes.contains(&code), "{code} in {codes:?}");
    }
    for d in &rb.diagnostics {
        assert_eq!(d.severity, "warning", "{d:?}");
    }
    let values = |r: &crate::session::protocol::EvalResultBody| -> Vec<Option<String>> {
        r.forms.iter().map(|f| f.value.clone()).collect()
    };
    assert_eq!(values(&ra), values(&rb));
    a.run_to(2.0);
    b.run_to(2.0);
    let times = |r: &Rig| -> Vec<f64> { r.sent().iter().map(|e| e.time).collect() };
    assert_eq!(times(&a), times(&b), "the same events");
}

const LEARN: &str =
    "#@ midi ch: 1\ns [:bd :sd] > lpf 800 > d1        #@ bass-filter: lpf cc: 74 71\n";

#[test]
fn learn_produces_a_validated_directive_edit_and_a_stale_epoch_is_rejected() {
    let mut rig = Rig::new();
    let (r, _) = rig.eval(LEARN, 1, 1);
    let out = learn(
        &mut rig,
        LearnTarget::Key("bass-filter.lpf.1.q".to_string()),
        72,
        1,
    );
    let [ServerMsg::DirectiveEdit(e)] = out.as_slice() else {
        panic!("one directive-edit: {out:?}");
    };
    assert_eq!((e.file.as_str(), e.doc_revision), (DOC, 1));
    assert_eq!(e.expected, "#@ bass-filter: lpf cc: 74 71");
    assert_eq!(e.text, "#@ bass-filter: lpf cc: 74 72");
    let edit = TextEdit {
        span: (e.span.start, e.span.end),
        expected: e.expected.clone(),
        text: e.text.clone(),
    };
    let saved = apply_edits(LEARN, &[edit]).expect("the expected text matches");
    assert!(saved.contains("cc: 74 72"));
    // By tweak id: the `800` literal is the labeled site's cutoff.
    let cutoff = site_at(&r, LEARN, "800");
    let out = learn(&mut rig, LearnTarget::Id(cutoff.id), 75, 1);
    assert!(
        matches!(out.as_slice(), [ServerMsg::DirectiveEdit(e)] if e.text == "#@ bass-filter: lpf cc: 75 71"),
        "{out:?}"
    );
    // A learn stamped with an unreconciled epoch is rejected.
    let out = learn(
        &mut rig,
        LearnTarget::Key("bass-filter.lpf.1.q".to_string()),
        70,
        2,
    );
    assert_eq!(stales(&out)[0].reason, StaleReason::UnreconciledEdit);
    // So is one after an edit the session has not re-evaluated.
    rig.doc_changed(2, 1, &[(0, 0, 1)], &[(0, 1)], 2);
    let out = learn(
        &mut rig,
        LearnTarget::Key("bass-filter.lpf.1.q".to_string()),
        70,
        2,
    );
    assert_eq!(stales(&out)[0].reason, StaleReason::EditInvalidated);
}

#[test]
fn external_file_learn_produces_no_edit_and_updates_the_set() {
    let mut cfg = SessionConfig::new(CapabilitySet::native());
    cfg.persistence = PersistenceMode::ExternalFile;
    let mut rig = Rig::with(cfg);
    rig.eval(LEARN, 1, 1);
    let out = learn(
        &mut rig,
        LearnTarget::Key("bass-filter.lpf.1.q".to_string()),
        72,
        1,
    );
    assert!(out.is_empty(), "no directive-edit: {out:?}");
    let doc = rig.s.doc(DOC).expect("doc");
    let ident = BindingIdent::Key(BindingKey::site("bass-filter", "lpf", 1, "q"));
    let e = doc.bindings.get(&ident).expect("the set holds the mapping");
    assert!(e.panel);
    assert_eq!(e.midi.map(|m| m.cc), Some(72));
    assert_eq!(doc.text, LEARN, "the source is untouched");
}
