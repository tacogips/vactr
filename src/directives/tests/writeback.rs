//! Learned-CC write-back into directive text (design 13.5, 14.5.8).

use crate::directives::key::{BindingIdent, BindingKey, KeyTable};
use crate::directives::persist::apply_edits;
use crate::directives::writeback::{learn_edit, LearnError};
use crate::session::changes::{Change, ChangeSet};

use super::{find, shown, table};

const SRC: &str = "#@ midi ch: 1
s [:bd :sd] > lpf 800 res: 0.4 > d1        #@ bass-filter: lpf cc: 74 71
s [:hh] > lpf 500 > hpf 2000 > d2          #@ hats:
#@ hats.hpf cc: 30
";

fn key(site: &str, param: &str) -> BindingIdent {
    let (label, site) = site.split_once('.').expect("label.site");
    BindingIdent::Key(BindingKey::site(label, site, 1, param))
}

#[test]
fn learn_replaces_the_number_at_the_parameter_position() {
    let (t, _) = table(SRC);
    let keys = KeyTable::from_table(&t, 1);
    let e = learn_edit(SRC, &t, &keys, &key("bass-filter.lpf", "q"), 72, None).expect("edit");
    assert_eq!(e.expected, "#@ bass-filter: lpf cc: 74 71");
    assert_eq!(e.text, "#@ bass-filter: lpf cc: 74 72");
    assert_eq!(
        e.span,
        (
            find(SRC, "#@ bass-filter", 0).start,
            find(SRC, "74 71", 0).end
        )
    );
    // A position past the list appends.
    let e = learn_edit(SRC, &t, &keys, &key("hats.hpf", "q"), 31, None).expect("edit");
    assert_eq!(
        (e.expected.as_str(), e.text.as_str()),
        ("#@ hats.hpf cc: 30", "#@ hats.hpf cc: 30 31")
    );
    let saved = apply_edits(SRC, &[e]).expect("applies");
    let (t2, _) = table(&saved);
    assert!(shown(&saved, &t2).contains(&"hats.hpf.1.q cc:31 ch:1".to_string()));
    // A new channel for a directive mapping only this parameter is set on
    // the directive itself.
    let e = learn_edit(SRC, &t, &keys, &key("hats.hpf", "cutoff"), 40, Some(3)).expect("edit");
    assert_eq!(e.text, "#@ hats.hpf cc: 40 ch: 3");
}

#[test]
fn learn_on_a_shared_channel_adds_an_override_line() {
    let (t, _) = table(SRC);
    let keys = KeyTable::from_table(&t, 1);
    let e = learn_edit(SRC, &t, &keys, &key("bass-filter.lpf", "q"), 72, Some(5)).expect("edit");
    let saved = apply_edits(SRC, &[e]).expect("applies");
    let (t2, diags) = table(&saved);
    assert!(diags.is_empty(), "{diags:?}\n{saved}");
    let got = shown(&saved, &t2);
    assert!(
        got.contains(&"bass-filter.lpf.1.cutoff cc:74 ch:1".to_string()),
        "{got:?}"
    );
    assert!(
        got.contains(&"bass-filter.lpf.1.q cc:72 ch:5".to_string()),
        "{got:?}"
    );
}

#[test]
fn learn_without_a_directive_appends_a_block() {
    let (t, _) = table(SRC);
    let keys = KeyTable::from_table(&t, 1);
    let e = learn_edit(SRC, &t, &keys, &key("hats.lpf", "cutoff"), 20, Some(1)).expect("edit");
    assert_eq!(e.expected, "");
    let saved = apply_edits(SRC, &[e]).expect("applies");
    assert_eq!(
        saved,
        "#@ midi ch: 1
s [:bd :sd] > lpf 800 res: 0.4 > d1        #@ bass-filter: lpf cc: 74 71
s [:hh] > lpf 500 > hpf 2000 > d2          #@ hats:
#@ hats.hpf cc: 30
#@ lpf cc: 20
"
    );
    let (t2, diags) = table(&saved);
    assert!(diags.is_empty(), "{diags:?}");
    assert!(shown(&saved, &t2).contains(&"hats.lpf.1.cutoff cc:20 ch:1".to_string()));
}

#[test]
fn learn_refuses_stale_and_unbacked_bindings() {
    let (t, _) = table(SRC);
    let mut keys = KeyTable::from_table(&t, 1);
    // Renaming the hats `hpf` head breaks its mapping.
    let renamed = SRC.replacen("hpf 2000", "lpf 2000", 1);
    let at = find(SRC, "hpf 2000", 0).start;
    let cs = ChangeSet::new(vec![Change {
        from: at,
        to: at + 1,
        insert_len: 1,
    }])
    .expect("valid");
    let (t2, _) = table(&renamed);
    let _ = keys.migrate(&cs, &t2);
    assert_eq!(
        learn_edit(&renamed, &t2, &keys, &key("hats.hpf", "cutoff"), 1, None),
        Err(LearnError::Stale)
    );
    assert_eq!(
        learn_edit(SRC, &t, &keys, &key("nosuch.lpf", "cutoff"), 1, None),
        Err(LearnError::Unknown)
    );
    // A literal is not a call site or parameter: positional TweakId
    // provenance, not a directive.
    let lit = find(SRC, "800", 0);
    assert_eq!(
        learn_edit(
            SRC,
            &t,
            &keys,
            &BindingIdent::positional(lit, "cutoff"),
            1,
            None
        ),
        Err(LearnError::NotDirectiveBacked)
    );
}
