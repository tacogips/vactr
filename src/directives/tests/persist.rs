//! Both `BindingPersistence` implementations round-trip the same binding
//! set without cross-talk; overlays live only in ExternalFile mode (S4).

use crate::directives::key::{BindingIdent, BindingKey};
use crate::directives::persist::{
    apply_edits, BindingEntry, BindingPersistence, BindingSet, DirectivePersistence, DocInput,
    EditError, ExternalFilePersistence, Midi, Persisted, TextEdit,
};

use super::table;

const DOC: &str = "#@ midi ch: 1
s [:hh] > lpf 800 > hpf 100 > lpf 2000 > d2   #@ hats:
inst analog cutoff: float = 1200 res: float = 0.3:
\tvco :saw freq > ladder cutoff res > * amp
";

fn entry(key: BindingKey, midi: Option<(u8, Option<u8>)>, overlay: Option<f64>) -> BindingEntry {
    BindingEntry {
        key: BindingIdent::Key(key),
        panel: true,
        midi: midi.map(|(cc, ch)| Midi { cc, ch }),
        overlay,
    }
}

/// lpf/hpf and lpf.1/lpf.2 keys, panel and MIDI, and overlays.
fn the_set() -> BindingSet {
    let site = BindingKey::site;
    BindingSet {
        entries: vec![
            entry(
                site("hats", "lpf", 1, "cutoff"),
                Some((74, Some(1))),
                Some(900.0),
            ),
            entry(site("hats", "lpf", 1, "q"), Some((71, Some(1))), None),
            entry(site("hats", "lpf", 2, "cutoff"), None, Some(1500.0)),
            entry(site("hats", "lpf", 2, "q"), Some((31, Some(4))), None),
            entry(site("hats", "hpf", 1, "cutoff"), Some((30, Some(1))), None),
            entry(site("hats", "hpf", 1, "q"), None, None),
            entry(
                BindingKey::param("analog", "cutoff"),
                Some((1, Some(1))),
                Some(0.25),
            ),
            entry(BindingKey::param("analog", "res"), None, None),
        ],
    }
}

fn edits_of(p: Persisted) -> Vec<TextEdit> {
    match p {
        Persisted::Edits(e) => e,
        Persisted::File(_) => panic!("directive mode edits the source"),
    }
}

#[test]
fn directive_mode_round_trips_without_overlays() {
    let (t, _) = table(DOC);
    let doc = DocInput {
        text: DOC,
        table: &t,
    };
    let mut p = DirectivePersistence;
    assert!(p.load(&doc).entries.is_empty());
    let set = the_set();
    let edits = edits_of(p.save(&doc, &set));
    assert!(!edits.is_empty());
    // Overlay values never reach the source (13, S4).
    for e in &edits {
        for v in ["900", "1500", "0.25"] {
            assert!(!e.text.contains(v), "overlay {v} written: {e:?}");
        }
    }
    let saved = apply_edits(DOC, &edits).expect("applies");
    assert!(saved.contains("#@ hats.lpf.2 cc: _ 31 ch: 4"), "{saved}");
    let (t2, diags) = table(&saved);
    assert!(diags.is_empty(), "{diags:?}\n{saved}");
    let doc2 = DocInput {
        text: &saved,
        table: &t2,
    };
    assert_eq!(p.load(&doc2).sorted(), set.without_overlays().sorted());
    // Saving the loaded set again changes nothing.
    assert!(edits_of(p.save(&doc2, &set)).is_empty());
}

#[test]
fn directive_mode_rewrites_an_existing_directive_in_place() {
    let src = format!("{DOC}#@ hats.lpf.2 cc: 5 6\n");
    let (t, _) = table(&src);
    let doc = DocInput {
        text: &src,
        table: &t,
    };
    let mut p = DirectivePersistence;
    let mut set = p.load(&doc);
    for e in &mut set.entries {
        if e.key == BindingIdent::Key(BindingKey::site("hats", "lpf", 2, "q")) {
            e.midi = Some(Midi { cc: 7, ch: Some(1) });
        }
    }
    let edits = edits_of(p.save(&doc, &set));
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].expected, "#@ hats.lpf.2 cc: 5 6");
    assert_eq!(edits[0].text, "#@ hats.lpf.2 cc: 5 7");
    // Taking the site off the panel removes its exclusive directive line.
    set.entries.clear();
    let edits = edits_of(p.save(&doc, &set));
    let saved = apply_edits(&src, &edits).expect("applies");
    assert_eq!(saved, DOC);
}

#[test]
fn external_file_mode_round_trips_with_overlays() {
    let (t, _) = table(DOC);
    let doc = DocInput {
        text: DOC,
        table: &t,
    };
    let set = the_set();
    let mut ext = ExternalFilePersistence::default();
    assert!(ext.load(&doc).entries.is_empty());
    let Persisted::File(json) = ext.save(&doc, &set) else {
        panic!("external mode writes the session file");
    };
    assert!(json.starts_with("{\"v\":1,\"bindings\":["), "{json}");
    assert!(json.contains("\"key\":\"hats.lpf.2.cutoff\""), "{json}");
    assert!(json.contains("\"overlay\":0.25"), "{json}");
    let reread = ExternalFilePersistence::with_file(json.clone());
    assert_eq!(reread.load(&doc).sorted(), set.sorted());
    assert_eq!(
        ExternalFilePersistence::parse(&json)
            .expect("parses")
            .sorted(),
        set.sorted()
    );
}

#[test]
fn no_cross_talk_between_modes() {
    // A document whose directives say one thing and whose session file says
    // another: each mode reads only its own store.
    let src = format!("{DOC}#@ hats.hpf cc: 99\n");
    let (t, _) = table(&src);
    let doc = DocInput {
        text: &src,
        table: &t,
    };
    let set = the_set();
    let mut ext = ExternalFilePersistence::default();
    let _ = ext.save(&doc, &set);
    assert_eq!(ext.load(&doc).sorted(), set.sorted());
    let from_directives = DirectivePersistence.load(&doc);
    assert_eq!(from_directives.entries.len(), 2);
    assert_eq!(
        from_directives.entries[0].midi,
        Some(Midi {
            cc: 99,
            ch: Some(1)
        })
    );
    // ExternalFile save leaves the source alone: the text is untouched and
    // Directive mode still reads the same table.
    assert_eq!(DirectivePersistence.load(&doc), from_directives);
}

#[test]
fn positional_bindings_persist_after_their_statement() {
    let src = "s [:bd] > lpf 800 > lpf 900 > d1\nlet a 1\n";
    let (t, _) = table(src);
    let doc = DocInput {
        text: src,
        table: &t,
    };
    let at = super::find(src, "lpf", 1);
    let set = BindingSet {
        entries: vec![BindingEntry {
            key: BindingIdent::positional(at, "cutoff"),
            panel: true,
            midi: Some(Midi { cc: 12, ch: None }),
            overlay: Some(3.0),
        }],
    };
    let edits = edits_of(DirectivePersistence.save(&doc, &set));
    let saved = apply_edits(src, &edits).expect("applies");
    assert_eq!(
        saved,
        "s [:bd] > lpf 800 > lpf 900 > d1\n#@ lpf.2 cc: 12\nlet a 1\n"
    );
    let (t2, _) = table(&saved);
    let loaded = DirectivePersistence.load(&DocInput {
        text: &saved,
        table: &t2,
    });
    assert_eq!(
        loaded
            .get(&BindingIdent::positional(at, "cutoff"))
            .map(|e| e.midi),
        Some(Some(Midi { cc: 12, ch: None }))
    );
}

#[test]
fn edits_are_verified_before_applying() {
    let edit = TextEdit {
        span: (0, 3),
        expected: "abc".into(),
        text: "x".into(),
    };
    assert_eq!(apply_edits("abcdef", &[edit.clone()]).expect("ok"), "xdef");
    assert_eq!(
        apply_edits("abXdef", &[edit.clone()]),
        Err(EditError::Mismatch { span: (0, 3) })
    );
    assert_eq!(
        apply_edits("abcdef", &[edit.clone(), edit]),
        Err(EditError::BadSpan { span: (0, 3) })
    );
    assert!(ExternalFilePersistence::parse("{\"v\":2,\"bindings\":[]}").is_err());
    assert!(ExternalFilePersistence::parse(
        "{\"v\":1,\"bindings\":[{\"key\":\"x\",\"panel\":true}]}"
    )
    .is_err());
}
