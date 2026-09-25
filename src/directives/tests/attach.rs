//! The Decided attachment rule over the spec's own examples
//! (architecture.md Editor Requirements; design 13.5).

use super::{codes, shown, table, SPEC};

/// Each directive's text and the first line of its attach target.
fn attachments(src: &str) -> Vec<(String, Option<String>)> {
    let (t, _) = table(src);
    t.entries
        .iter()
        .map(|p| {
            let d = &src[p.directive.span.start as usize..p.directive.span.end as usize];
            let target = p.attach.map(|k| {
                let tg = &t.doc.targets[k];
                let start = t.doc.lines.start(tg.first_line).expect("line") as usize;
                src[start..]
                    .lines()
                    .next()
                    .expect("line")
                    .trim()
                    .to_string()
            });
            (d.split("  ").next().expect("text").to_string(), target)
        })
        .collect()
}

#[test]
fn spec_examples_attach() {
    let got = attachments(SPEC);
    let line1 = "s [:bd :sd] > lpf 800 res: 0.4 > hpf 120 > d1";
    let inst = "inst analog cutoff: float = 1200 res: float = 0.3:";
    let bass = "s [:bd :sd] > lpf 800 res: 0.4 > d1        #@ bass-filter: lpf cc: 74 71";
    let hats = "s [:hh] > hpf 2000 > d2                    #@ hats:";
    let want: Vec<(&str, Option<&str>)> = vec![
        // File level: addressed, position-free.
        ("#@ midi ch: 1", None),
        // A consecutive own-line block binds to the preceding statement.
        ("#@ lpf cc: 74 71", Some(line1)),
        ("#@ hpf cc: 30", Some(line1)),
        // Column 0 after an `inst` body: the whole `inst`.
        ("#@ cutoff res", Some(inst)),
        // Same-line trailing directives bind to their line.
        ("#@ bass-filter: lpf cc: 74 71", Some(bass)),
        ("#@ hats:", Some(hats)),
        // Addressed directives are position-free.
        ("#@ hats.hpf cc: 30", None),
        ("#@ analog.cutoff cc: 1", None),
    ];
    let got: Vec<(&str, Option<&str>)> = got
        .iter()
        .map(|(d, t)| (d.as_str(), t.as_deref()))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn spec_examples_resolve() {
    let (t, diags) = table(SPEC);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(t.file_level.midi_ch, Some(1));
    assert_eq!(
        shown(SPEC, &t),
        [
            "@2:15.cutoff cc:74 ch:1",
            "@2:15.q cc:71 ch:1",
            "@2:34.cutoff cc:30 ch:1",
            "@2:34.q cc:- ch:-",
            "analog.cutoff cc:1 ch:1",
            "analog.res cc:- ch:-",
            "bass-filter.lpf.1.cutoff cc:74 ch:1",
            "bass-filter.lpf.1.q cc:71 ch:1",
            "hats.hpf.1.cutoff cc:30 ch:1",
            "hats.hpf.1.q cc:- ch:-",
        ]
    );
}

#[test]
fn inst_body_block_binds_to_previous_body_line() {
    let src = "inst pluck freq: float = 440:
\tsaw freq
\t\t> lpf 900
\t#@ lpf cc: 20
#@ freq cc: 21
";
    let got = attachments(src);
    assert_eq!(got[0].1.as_deref(), Some("saw freq"));
    assert_eq!(got[1].1.as_deref(), Some("inst pluck freq: float = 440:"));
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    // A body line keys by the enclosing definition's label.
    assert_eq!(
        shown(src, &t),
        [
            "pluck.lpf.1.cutoff cc:20 ch:-",
            "pluck.lpf.1.q cc:- ch:-",
            "pluck.freq cc:21 ch:-",
        ]
    );
}

#[test]
fn trailing_on_a_continuation_line_binds_to_its_statement() {
    let src = "s [:hh]\n\t> lpf 900   #@ lpf cc: 5\n\t> d1\n";
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        shown(src, &t),
        ["@2:4.cutoff cc:5 ch:-", "@2:4.q cc:- ch:-"]
    );
}

#[test]
fn a_blank_line_starts_a_new_block() {
    // The second block starts after a blank line: it attaches on its own,
    // to the same nearest preceding statement.
    let src = "s [:hh] > lpf 1 > hpf 2 > d1\n#@ lpf cc: 1\n\n#@ hpf cc: 2\n";
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(t.entries[0].attach, t.entries[1].attach);
}

#[test]
fn positional_without_target_and_duplicate_file_default() {
    let (_, diags) = table("#@ lpf cc: 74\ns [:hh] > lpf 1 > d1\n");
    assert_eq!(codes(&diags), ["unknown-directive-site"]);
    let (t, diags) = table("#@ midi ch: 1\n#@ midi ch: 2\ns [:hh] > lpf 1 > d1\n#@ lpf cc: 3\n");
    assert_eq!(codes(&diags), ["duplicate-key"]);
    assert_eq!(t.file_level.midi_ch, Some(2));
    assert_eq!(t.resolved[0].ch, Some(2));
}

#[test]
fn call_site_args_carry_their_spans_and_named_keywords() {
    // `lpf 800 res: 0.4`: a positional argument and a named one, TASK-010
    // G3's `CallSite::args`.
    let (t, diags) = table(SPEC);
    assert!(diags.is_empty(), "{diags:?}");
    let lpf = t
        .doc
        .sites
        .iter()
        .find(|s| &*s.name == "lpf")
        .expect("an `lpf` call site");
    assert_eq!(lpf.args.len(), 2, "{:?}", lpf.args);
    let (kw, span) = &lpf.args[0];
    assert!(kw.is_none(), "`800` is positional");
    assert_eq!(&SPEC[span.start as usize..span.end as usize], "800");
    let (kw, span) = &lpf.args[1];
    assert_eq!(kw.as_deref(), Some("res"), "`res:` is named");
    assert_eq!(&SPEC[span.start as usize..span.end as usize], "res: 0.4");
}

#[test]
fn call_site_args_cover_a_full_list_argument() {
    let src = "s :bd > n [0 3 5] > d1\n";
    let (t, diags) = table(src);
    assert!(diags.is_empty(), "{diags:?}");
    let n = t
        .doc
        .sites
        .iter()
        .find(|s| &*s.name == "n")
        .expect("an `n` call site");
    assert_eq!(n.args.len(), 1, "{:?}", n.args);
    let (kw, span) = &n.args[0];
    assert!(kw.is_none());
    assert_eq!(&src[span.start as usize..span.end as usize], "[0 3 5]");
}

#[test]
fn table_summary_serializes_for_the_protocol() {
    let (t, _) = table(SPEC);
    let json = serde_json::to_string(&t.summary()).expect("serializes");
    assert!(json.starts_with("{\"midi_ch\":1,"), "{json}");
    assert!(json.contains("\"key\":\"hats.hpf.1.cutoff\""), "{json}");
    assert!(json.contains("\"name\":\"bass-filter\""), "{json}");
    // Positional provenance has no key.
    assert!(json.contains("\"key\":null"), "{json}");
}
