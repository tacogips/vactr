//! `BindingKey`: full selector-path identity, and migration across
//! revisions through the `doc-changed` change set (design 13.5, 14.5.8).

use std::collections::BTreeSet;

use crate::directives::key::{BindingKey, KeyState, KeyTable};
use crate::session::changes::{Change, ChangeSet};

use super::{find, table};

fn keys_of(src: &str) -> BTreeSet<String> {
    let (t, _) = table(src);
    t.resolved
        .iter()
        .filter_map(|r| r.key.as_ref().map(ToString::to_string))
        .collect()
}

/// The change set turning `old` into `new` by one replacement of the
/// differing middle.
fn diff(old: &str, new: &str) -> ChangeSet {
    let pre = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let post = old[pre..]
        .bytes()
        .rev()
        .zip(new[pre..].bytes().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let to = old.len() - post;
    let insert = new.len() - post - pre;
    let n = |x: usize| u32::try_from(x).expect("small");
    ChangeSet::new(vec![Change {
        from: n(pre),
        to: n(to),
        insert_len: n(insert),
    }])
    .expect("valid")
}

#[test]
fn keys_are_the_full_selector_path() {
    let src = "s [:hh] > lpf 800 > hpf 100 > lpf 2000 > d2   #@ hats:
#@ hats.lpf.1 cc: 1
#@ hats.lpf.2 cc: 2
#@ hats.hpf cc: 3
";
    let keys = keys_of(src);
    let lpf1 = BindingKey::site("hats", "lpf", 1, "cutoff");
    let lpf2 = BindingKey::site("hats", "lpf", 2, "cutoff");
    let hpf = BindingKey::site("hats", "hpf", 1, "cutoff");
    // The shared `cutoff` keyword does not collide.
    assert_ne!(lpf1, hpf);
    assert_ne!(lpf1, lpf2);
    for k in [&lpf1, &lpf2, &hpf] {
        assert!(keys.contains(&k.to_string()), "{k} in {keys:?}");
    }
    assert_eq!(keys.len(), 6);
    // Display and FromStr round-trip.
    for k in [&lpf1, &BindingKey::param("analog", "cutoff")] {
        assert_eq!(&k.to_string().parse::<BindingKey>().expect("parses"), k);
    }
    assert_eq!(lpf2.to_string(), "hats.lpf.2.cutoff");
    assert!("hats.lpf.cutoff".parse::<BindingKey>().is_err());
    assert!("hats.lpf.0.cutoff".parse::<BindingKey>().is_err());
    assert!("hats".parse::<BindingKey>().is_err());
}

#[test]
fn moving_the_labeled_line_keeps_every_key() {
    let old = "s [:hh] > lpf 800 > hpf 100 > d2   #@ hats:
#@ hats.lpf cc: 5
#@ hats.hpf cc: 6
inst analog cutoff: float = 1200:
\tvco :saw freq > ladder cutoff 0.3
#@ analog.cutoff cc: 7
";
    // Lines inserted above move the labeled line and the inst down.
    let new = format!("let a 1\nlet b 2\n{old}");
    let (t_old, _) = table(old);
    let (t_new, _) = table(&new);
    let mut keys = KeyTable::from_table(&t_old, 1);
    let before: Vec<BindingKey> = keys.iter().map(|(k, _)| k.clone()).collect();
    let moves = keys.migrate(&diff(old, &new), &t_new);
    assert_eq!(keys.revision(), 2);
    assert!(moves.iter().all(|(_, s)| *s == KeyState::Live), "{moves:?}");
    let after: Vec<BindingKey> = keys.iter().map(|(k, _)| k.clone()).collect();
    assert_eq!(before, after);
    let lpf = keys
        .get(&BindingKey::site("hats", "lpf", 1, "cutoff"))
        .expect("tracked");
    let at = find(&new, "lpf", 0);
    assert_eq!((lpf.span, lpf.revision), ((at.start, at.end), 2));
}

#[test]
fn reordering_same_named_sites_migrates_each_binding() {
    let old =
        "s [:hh] > lpf 800 > lpf 2000 > d2   #@ hats:\n#@ hats.lpf.1 cc: 1\n#@ hats.lpf.2 cc: 2\n";
    // A new `lpf` inserted ahead of both renumbers them: 1 -> 2, 2 -> 3.
    let new = old.replacen("lpf 800", "lpf 300 > lpf 800", 1);
    let at = find(old, "lpf 800", 0).start;
    let insert = ChangeSet::new(vec![Change {
        from: at,
        to: at,
        insert_len: 10,
    }])
    .expect("valid");
    let (t_old, _) = table(old);
    let (t_new, _) = table(&new);
    let mut keys = KeyTable::from_table(&t_old, 1);
    let moves = keys.migrate_moves(&insert, &t_new);
    let mut got: Vec<(String, String, KeyState)> = moves
        .iter()
        .map(|m| (m.from.to_string(), m.to.to_string(), m.state))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            (
                "hats.lpf.1.cutoff".into(),
                "hats.lpf.2.cutoff".into(),
                KeyState::Live
            ),
            ("hats.lpf.1.q".into(), "hats.lpf.2.q".into(), KeyState::Live),
            (
                "hats.lpf.2.cutoff".into(),
                "hats.lpf.3.cutoff".into(),
                KeyState::Live
            ),
            ("hats.lpf.2.q".into(), "hats.lpf.3.q".into(), KeyState::Live),
        ]
    );
    // Each key now points at its own site's head.
    let first = keys
        .get(&BindingKey::site("hats", "lpf", 2, "cutoff"))
        .expect("migrated");
    let at = find(&new, "lpf", 1);
    assert_eq!(first.span, (at.start, at.end));

    // Swapping the two sites: the moved site's text was deleted and
    // re-inserted, so its binding is Stale (never guessed); the other
    // migrates with its site, 1 -> 2.
    let swapped =
        "s [:hh] > lpf 2000 > lpf 800 > d2   #@ hats:\n#@ hats.lpf.1 cc: 1\n#@ hats.lpf.2 cc: 2\n";
    let old_b = old;
    let a = find(old_b, "lpf 800", 0).start;
    let b_start = find(old_b, " > lpf 2000", 0).start;
    let b_end = find(old_b, " > lpf 2000", 0).end;
    let changes = ChangeSet::new(vec![
        Change {
            from: a,
            to: a,
            insert_len: 11,
        },
        Change {
            from: b_start,
            to: b_end,
            insert_len: 0,
        },
    ])
    .expect("valid");
    let (t_swapped, _) = table(swapped);
    let mut keys = KeyTable::from_table(&t_old, 1);
    let moves = keys.migrate_moves(&changes, &t_swapped);
    let mut got: Vec<(String, String, KeyState)> = moves
        .iter()
        .map(|m| (m.from.to_string(), m.to.to_string(), m.state))
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            (
                "hats.lpf.1.cutoff".into(),
                "hats.lpf.2.cutoff".into(),
                KeyState::Live
            ),
            ("hats.lpf.1.q".into(), "hats.lpf.2.q".into(), KeyState::Live),
            (
                "hats.lpf.2.cutoff".into(),
                "hats.lpf.2.cutoff".into(),
                KeyState::Stale
            ),
            (
                "hats.lpf.2.q".into(),
                "hats.lpf.2.q".into(),
                KeyState::Stale
            ),
        ]
    );
    // The Live migration owns the spelling; the stale data is kept aside.
    assert_eq!(
        keys.state(&BindingKey::site("hats", "lpf", 2, "cutoff")),
        Some(KeyState::Live)
    );
    assert_eq!(keys.superseded().len(), 2);
}

#[test]
fn a_broken_mapping_marks_the_key_stale() {
    let old = "s [:hh] > lpf 800 > d2   #@ hats:\n#@ hats.lpf cc: 5\n";
    let (t_old, _) = table(old);
    // Rewriting the call-site name touches the head span.
    let renamed = old.replacen("lpf 800", "hpf 800", 1);
    let (t_new, _) = table(&renamed);
    let mut keys = KeyTable::from_table(&t_old, 1);
    let moves = keys.migrate(&diff(old, &renamed), &t_new);
    assert!(
        moves.iter().all(|(_, s)| *s == KeyState::Stale),
        "{moves:?}"
    );
    let key = BindingKey::site("hats", "lpf", 1, "cutoff");
    assert_eq!(keys.state(&key), Some(KeyState::Stale));
    // Its data is kept: the span of the last resolution, at revision 1.
    assert_eq!(keys.get(&key).expect("kept").revision, 1);

    // The label moved to another line: the mapped head no longer lies
    // under `hats`.
    let relabeled =
        "s [:hh] > lpf 800 > d2   #@ kick:\ns [:oh] > d3   #@ hats:\n#@ hats.lpf cc: 5\n";
    let (t_rel, _) = table(relabeled);
    let mut keys = KeyTable::from_table(&t_old, 1);
    let moves = keys.migrate(&diff(old, relabeled), &t_rel);
    assert!(
        moves.iter().all(|(_, s)| *s == KeyState::Stale),
        "{moves:?}"
    );

    // Re-confirmation brings a stale key back once it resolves again.
    let mut keys = KeyTable::from_table(&t_old, 1);
    let _ = keys.migrate(&diff(old, &renamed), &t_new);
    assert!(!keys.confirm(&key, &t_new));
    assert!(keys.confirm(&key, &t_old));
    assert_eq!(keys.state(&key), Some(KeyState::Live));
}
