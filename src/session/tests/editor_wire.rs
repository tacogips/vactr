//! ED-WIRE required tests (design 15.1.2 G2-G6): editor metadata on
//! `manifest`, call identity on `site`, analysis and clock telemetry on
//! `levels`/`tempo`, wire compatibility, `Session::check` and the browser
//! package driver through `Session::drive_packages`.
//!
//! `pkg::tests` is private to `pkg` (its fixtures are not reachable from
//! here), so the `drive_packages` test below builds its own tiny in-memory
//! proxy fixture (a minimal stored-only ZIP writer) instead of reusing
//! `pkg::tests::support`.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::support::{site_at, Rig, DOC};
use crate::host::caps::HostSigs;
use crate::pattern::eval::AnalyzerId;
use crate::pkg::digest::{hex, sources_digest};
use crate::pkg::driver::{DriverReply, DriverRequest, Prefetched};
use crate::session::codec::decode_as;
use crate::session::protocol::{ClientMsg, Empty, ServerMsg, SubscribeBody};

fn subscribe_levels(rig: &mut Rig) {
    rig.send(ClientMsg::Subscribe(SubscribeBody {
        telemetry: false,
        levels: true,
        diagnostics: false,
    }));
}

#[test]
fn manifest_editors_cover_the_known_kinds_with_command_md_field_names() {
    let mut rig = Rig::new();
    let out = rig.send(ClientMsg::ManifestReq(Empty {}));
    let ServerMsg::Manifest(m) = &out[0] else {
        panic!("{out:?}")
    };
    let editors = m.editors.as_ref().expect("editors present");
    let find = |name: &str| {
        editors
            .iter()
            .find(|e| e.name == name)
            .unwrap_or_else(|| panic!("no `{name}` editor decl: {editors:?}"))
    };
    assert_eq!(find("peq").kind, "eq-curve");
    assert_eq!(find("env-adsr").kind, "envelope-shape");
    assert_eq!(find("lpf").kind, "filter-response");
    let euclid = find("euclid");
    assert_eq!(euclid.kind, "euclid-ring");
    assert!(
        euclid.params.iter().all(|p| p.ctl.is_none()),
        "pattern functions carry no ctl: {euclid:?}"
    );
    // JSON field names match `command.md`'s `editor-decl`/`param`.
    let json = serde_json::to_value(m).expect("json");
    let peq = json["editors"]
        .as_array()
        .expect("array")
        .iter()
        .find(|e| e["name"] == "peq")
        .expect("peq");
    assert_eq!(peq["kind"], "eq-curve");
    let p0 = &peq["params"][0];
    for key in ["name", "range", "curve", "unit", "group"] {
        assert!(p0.get(key).is_some(), "missing `{key}`: {p0}");
    }
    let euclid_json = json["editors"]
        .as_array()
        .expect("array")
        .iter()
        .find(|e| e["name"] == "euclid" && e["kind"] == "euclid-ring")
        .expect("euclid");
    assert!(
        euclid_json["params"][0].get("ctl").is_none(),
        "`ctl` is omitted, not null: {euclid_json}"
    );
}

/// DDRUM-006: `ParamMeta`/`WireParamMeta` carry `default`, `label` and
/// `choices`, derived from the control-row manifest; `default` is a plain
/// row default for an ordinary builtin. (`lpf` names both a pattern-control
/// row-backed ugen AND a bus/orbit effect with its own, different `cutoff`
/// default, so `sin-osc`/`freq` is used here instead, unambiguous.)
#[test]
fn param_meta_carries_default_label_and_choices_from_the_control_row() {
    let mut rig = Rig::new();
    let out = rig.send(ClientMsg::ManifestReq(Empty {}));
    let ServerMsg::Manifest(m) = &out[0] else {
        panic!("{out:?}")
    };
    let editors = m.editors.as_ref().expect("editors present");
    let sin_osc = editors
        .iter()
        .find(|d| d.name == "sin-osc")
        .expect("sin-osc");
    let freq = sin_osc
        .params
        .iter()
        .find(|p| p.name == "freq")
        .expect("sin-osc/freq");
    let row = crate::dsp::controls::row("freq").expect("freq is a row");
    assert_eq!(freq.default, Some(row.default));
    assert_eq!(
        freq.label.as_deref(),
        Some(crate::dsp::meta::label_of("freq").as_str())
    );
    assert!(freq.choices.is_empty(), "freq is not an enum");

    let sampler = editors
        .iter()
        .find(|d| d.name == "sampler")
        .expect("sampler");
    let envelope = sampler.params.iter().find(|p| p.name == "envelope");
    // `sampler`'s header list (`bank, n, speed, begin, end, loop, attack,
    // release`) has no `envelope`; use `granular`'s instead, whose enum
    // `envelope` row carries named choices in domain order.
    assert!(envelope.is_none(), "sampler has no `envelope` header");
    let granular = editors
        .iter()
        .find(|d| d.name == "granular")
        .expect("granular");
    let envelope = granular
        .params
        .iter()
        .find(|p| p.name == "envelope")
        .expect("granular/envelope");
    assert_eq!(envelope.choices, vec!["hann", "tri", "trapezoid", "expo"]);
    assert_eq!(
        envelope.default,
        Some(0.0),
        "the row's default, index 0 (hann)"
    );
    assert_eq!(envelope.label.as_deref(), Some("Envelope"));
}

/// DDRUM-006: the four digital-drum-family prelude templates each start
/// every voice on the `sine` waveform and their own filter response
/// (`src/prelude/templates.vact`'s literal header defaults), not the
/// shared control row's general-purpose default (`saw`/`off`). The
/// manifest's static `EditorDecl` must show the TEMPLATE's own default,
/// cross-checked here against the same instrument registry's realized
/// header default so `dsp::meta::TEMPLATE_DEFAULT_OVERRIDES` cannot
/// silently drift from the prelude source.
#[test]
fn digital_drum_family_templates_show_their_own_header_default_not_the_shared_row() {
    use crate::host::wire::Ctl;
    use crate::value::intern::name_of_kw;

    let mut rig = Rig::new();
    let out = rig.send(ClientMsg::ManifestReq(Empty {}));
    let ServerMsg::Manifest(m) = &out[0] else {
        panic!("{out:?}")
    };
    let editors = m.editors.as_ref().expect("editors present").clone();
    let registry = rig
        .s
        .evaluator()
        .insts()
        .expect("a registry")
        .borrow()
        .entries()
        .cloned()
        .collect::<Vec<_>>();

    for (template, filter_default_kw, filter_default_index) in [
        ("digital-drum", "lp", 1.0_f32),
        ("digital-snare", "lp", 1.0_f32),
        ("digital-metal", "bp", 2.0_f32),
        ("digital-hat", "hp", 3.0_f32),
    ] {
        let decl = editors
            .iter()
            .find(|d| d.name == template)
            .unwrap_or_else(|| panic!("no `{template}` editor decl"));
        let wave = decl
            .params
            .iter()
            .find(|p| p.name == "wave")
            .unwrap_or_else(|| panic!("{template}/wave"));
        assert_eq!(
            wave.choices,
            vec!["saw", "pulse", "square", "tri", "sine"],
            "{template}"
        );
        assert_eq!(
            wave.default,
            Some(4.0),
            "{template}: `sine` (its own header default), not the row's `saw`"
        );
        let filter = decl
            .params
            .iter()
            .find(|p| p.name == "filter-type")
            .unwrap_or_else(|| panic!("{template}/filter-type"));
        assert_eq!(
            filter.choices,
            vec!["off", "lp", "bp", "hp", "notch"],
            "{template}"
        );
        assert_eq!(
            filter.default,
            Some(filter_default_index),
            "{template}: its own header default, not the row's `off`"
        );
        assert_eq!(
            filter.choices[filter_default_index as usize],
            filter_default_kw
        );

        // Cross-check against the realized header default the prelude's
        // `.vact` source actually installed.
        let entry = registry
            .iter()
            .find(|e| &*name_of_kw(e.name) == template)
            .unwrap_or_else(|| panic!("`{template}` is a prelude template"));
        for (name, want) in [("wave", wave.default), ("filter-type", filter.default)] {
            let row = crate::dsp::controls::row(name).expect("a control row");
            let live = entry
                .def
                .params
                .iter()
                .find_map(|(ctl, v)| (*ctl == row.ctl).then_some(*v))
                .map(|v| match v {
                    Ctl::Const(c) => c,
                    Ctl::Cell(_) => panic!("{template}/{name}: unexpectedly a tweak site"),
                });
            assert_eq!(
                live, want,
                "{template}/{name}: the editor default matches the realized header default"
            );
        }
    }
}

/// DDRUM-006: an ordinary user `inst` header whose name is a control row
/// (`filter-type`, an enum; `cutoff`, a plain float) carries the header's
/// OWN default (not the shared row's), a human label and, for the enum,
/// the domain's choice names, exactly like the prelude templates above.
#[test]
fn a_user_inst_header_named_after_a_control_row_carries_its_own_default_and_choices() {
    let mut rig = Rig::new();
    rig.ok(
        "inst my-filtered-osc filter-type: keyword = :bp cutoff: float = 900:\n\tsin-osc 220\n",
        1,
    );
    let out = rig.send(ClientMsg::ManifestReq(Empty {}));
    let ServerMsg::Manifest(m) = &out[0] else {
        panic!("{out:?}")
    };
    let editors = m.editors.as_ref().expect("editors present");
    let decl = editors
        .iter()
        .find(|d| d.name == "my-filtered-osc")
        .expect("my-filtered-osc editor decl");
    let filter_type = decl
        .params
        .iter()
        .find(|p| p.name == "filter-type")
        .expect("filter-type param");
    assert_eq!(
        filter_type.default,
        Some(2.0),
        "the header's own `:bp` default (index 2), not the row's `:off`"
    );
    assert_eq!(filter_type.choices, vec!["off", "lp", "bp", "hp", "notch"]);
    assert_eq!(filter_type.label.as_deref(), Some("Filter type"));
    let cutoff = decl
        .params
        .iter()
        .find(|p| p.name == "cutoff")
        .expect("cutoff param");
    assert_eq!(cutoff.default, Some(900.0));
    assert_eq!(cutoff.range, [20.0, 20_000.0]);
    assert!(cutoff.choices.is_empty());
}

/// DDRUM-006: the new `default`/`label`/`choices` fields round-trip
/// through JSON and an old payload lacking them still decodes (protocol
/// backward compatibility: new optional fields, `v` stays 1).
#[test]
fn param_meta_new_fields_round_trip_and_stay_backward_compatible() {
    use crate::session::protocol::WireParamMeta;

    let meta = WireParamMeta {
        name: "filter-type".to_string(),
        ctl: Some(109),
        range: [0.0, 4.0],
        curve: "stepped".to_string(),
        unit: "none".to_string(),
        group: 0,
        default: Some(1.0),
        label: Some("Filter type".to_string()),
        choices: vec![
            "off".to_string(),
            "lp".to_string(),
            "bp".to_string(),
            "hp".to_string(),
            "notch".to_string(),
        ],
    };
    let json = serde_json::to_value(&meta).expect("json");
    assert_eq!(json["default"], 1.0);
    assert_eq!(json["label"], "Filter type");
    assert_eq!(json["choices"][1], "lp");
    let back: WireParamMeta = serde_json::from_value(json).expect("decodes");
    assert_eq!(back, meta);

    let old = serde_json::json!({
        "name": "cutoff",
        "range": [20.0, 20_000.0],
        "curve": "log",
        "unit": "hz",
        "group": 0,
    });
    let decoded: WireParamMeta = serde_json::from_value(old).expect("an old payload decodes");
    assert_eq!(decoded.default, None);
    assert_eq!(decoded.label, None);
    assert!(decoded.choices.is_empty());
    let re_encoded = serde_json::to_value(&decoded).expect("json");
    assert!(re_encoded.get("default").is_none(), "{re_encoded}");
    assert!(re_encoded.get("label").is_none(), "{re_encoded}");
    assert!(re_encoded.get("choices").is_none(), "{re_encoded}");
}

#[test]
fn a_chained_call_gives_its_sites_call_identity_and_ordinal() {
    let mut rig = Rig::new();
    let src = "s :hats > lpf 800 > hpf 3000 > d1\n";
    let (r, _) = rig.eval(src, 1, 0);
    let lpf = site_at(&r, src, "800")
        .call
        .clone()
        .expect("lpf 800's call");
    assert_eq!(lpf.name, "lpf");
    assert_eq!(lpf.ordinal, 1);
    assert_eq!(lpf.arg, 0);
    assert_eq!(lpf.param.as_deref(), Some("cutoff"));
    let hpf = site_at(&r, src, "3000")
        .call
        .clone()
        .expect("hpf 3000's call");
    assert_eq!(hpf.name, "hpf");
    assert_eq!(hpf.ordinal, 1);
    assert_eq!(hpf.param.as_deref(), Some("cutoff"));
}

#[test]
fn a_second_same_named_call_in_the_same_top_level_form_gets_ordinal_two() {
    let mut rig = Rig::new();
    let src = "s :hats > lpf 800 > lpf 900 > d1\n";
    let (r, _) = rig.eval(src, 1, 0);
    let first = site_at(&r, src, "800").call.clone().expect("first lpf");
    let second = site_at(&r, src, "900").call.clone().expect("second lpf");
    assert_eq!(first.name, "lpf");
    assert_eq!(first.ordinal, 1);
    assert_eq!(second.name, "lpf");
    assert_eq!(second.ordinal, 2);
}

#[test]
fn a_list_argument_gives_every_literal_the_same_call() {
    let mut rig = Rig::new();
    let src = "s :bd > n [0 3 5] > d1\n";
    let (r, _) = rig.eval(src, 1, 0);
    for lit in ["0", "3", "5"] {
        let call = site_at(&r, src, lit)
            .call
            .clone()
            .unwrap_or_else(|| panic!("`{lit}` has no call"));
        assert_eq!(call.name, "n", "{lit}");
        assert_eq!(call.arg, 0, "{lit}");
    }
}

/// A synthetic `TweakSite` at the `needle` occurrence of `src`, for driving
/// `site_wire` directly. A named-argument literal is never itself a Decided
/// tweak site in the current compiler (`compile/sites.rs`: a keyword's
/// value always compiles at `SiteCtx::None` unless the whole call is
/// already nested under a `Late` argument), so
/// `a_named_argument_gives_its_param_from_the_keyword` below exercises the
/// call-resolution logic (`session::publish::site_wire`/`call_of`)
/// directly over a document model, rather than over a genuinely evaluated
/// site — the property under test ("the named keyword wins") belongs to
/// that resolution, not to site classification.
fn tweak_site_at(
    src: &str,
    file: crate::reader::span::FileId,
    needle: &str,
    value: crate::value::value::Value,
) -> crate::ns::tweak::TweakSite {
    use crate::ns::namespace::{FormGen, SlotKind, VarSlotRef};
    use crate::ns::tweak::{SiteOrigin, SiteTier, TweakId, TweakSite};
    use crate::reader::span::Span;
    use crate::value::intern::intern_sym;
    use crate::value::num::NumKind;
    let at =
        u32::try_from(src.find(needle).unwrap_or_else(|| panic!("`{needle}`"))).expect("small");
    let end = at + u32::try_from(needle.len()).unwrap_or(0);
    TweakSite {
        id: TweakId::new(0),
        span: Span::new(file, at, end),
        slot: VarSlotRef::new(intern_sym("tweak"), SlotKind::Tweak, value.clone()),
        initial: value.clone(),
        ty: NumKind::of(&value).expect("a number literal"),
        tier: SiteTier::Reeval,
        form_gen: FormGen::new(1),
        origin: SiteOrigin::PatternLiteral,
        index: 0,
    }
}

#[test]
fn a_named_argument_gives_its_param_from_the_keyword() {
    use crate::directives::build_table;
    use crate::reader::span::FileId;
    use crate::reader::{read, AliasEnv};
    use crate::session::publish::site_wire;
    use crate::types::diag::Severity;
    use crate::types::manifest::HostManifest;
    use crate::value::value::Value;

    let src = "s [:bd :sd] > lpf 800 res: 0.4 > hpf 120 > d1\n";
    let file = FileId::new(9);
    let r = read(src, file, &AliasEnv::new());
    let (table, diags) = build_table(
        src,
        file,
        &r.nodes,
        &r.trivia,
        &HostManifest::spec_default(),
    );
    assert!(
        diags.iter().all(|d| d.severity != Severity::Error),
        "{diags:?}"
    );
    let rig = Rig::new();

    let cutoff_site = tweak_site_at(src, file, "800", Value::Int(800));
    let cutoff = site_wire(rig.s.evaluator(), &cutoff_site, None, &table.doc)
        .call
        .expect("`800` has an enclosing call");
    assert_eq!(cutoff.name, "lpf");
    assert_eq!(cutoff.arg, 0, "the piped subject is filtered out of `args`");
    assert_eq!(
        cutoff.param.as_deref(),
        Some("cutoff"),
        "the positional argument falls back to the declared parameter"
    );

    let res_site = tweak_site_at(src, file, "0.4", Value::Float(0.4));
    let res = site_wire(rig.s.evaluator(), &res_site, None, &table.doc)
        .call
        .expect("`0.4` has an enclosing call");
    assert_eq!(res.name, "lpf");
    assert_eq!(res.arg, 1);
    assert_eq!(
        res.param.as_deref(),
        Some("res"),
        "the named keyword wins over the declared position"
    );
}

#[test]
fn a_top_level_let_literal_has_no_enclosing_call() {
    let mut rig = Rig::new();
    let src = "let x 3\n";
    let (r, _) = rig.eval(src, 1, 0);
    let site = site_at(&r, src, "3");
    assert!(site.call.is_none(), "{site:?}");
}

#[test]
fn levels_carries_master_bands_and_analyzer_cells() {
    let mut rig = Rig::new();
    subscribe_levels(&mut rig);
    rig.ok("bus :meter:\n\tlevel id: 5\n", 1);
    rig.audio.set_sigs(HostSigs {
        amp: 0.5,
        fft: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8],
    });
    rig.s.rt.input.set_analyzer(AnalyzerId::new(5), 0.3);
    rig.s.rt.input.set_analyzer(AnalyzerId::new(6), 0.7);
    let msgs = rig.tick();
    let ServerMsg::Levels(levels) = msgs
        .iter()
        .find(|m| matches!(m, ServerMsg::Levels(_)))
        .unwrap_or_else(|| panic!("no levels message: {msgs:?}"))
    else {
        unreachable!()
    };
    let master = &levels.levels[0];
    assert_eq!(master.source, ":master");
    assert_eq!(master.bands, Some([0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]));
    let analyzers = levels.analyzers.as_ref().expect("an analyzer is installed");
    let level = analyzers
        .iter()
        .find(|a| a.kind == "level")
        .unwrap_or_else(|| panic!("no `level` analyzer: {analyzers:?}"));
    assert_eq!(level.bus, "meter");
    assert_eq!(level.id, 5);
    assert_eq!(level.cells.len(), 2, "AnalyzerKind::Level owns 2 cells");
    assert_eq!(level.cells, vec![0.3, 0.7]);
}

#[test]
fn tempo_clock_reports_internal_then_midi_then_unlocked_after_the_loss_timeout() {
    let mut rig = Rig::new();
    let clock_of = |msgs: &[ServerMsg]| {
        msgs.iter().find_map(|m| match m {
            ServerMsg::Tempo(t) => t.clock.clone(),
            _ => None,
        })
    };
    let first = rig.tick();
    let c0 = clock_of(&first).expect("tempo carries `clock` on the first tick");
    assert_eq!(c0.source, "internal");
    assert!(c0.locked.is_none());

    rig.ok("use-clock :midi\n", 1);
    rig.clock.advance(0.01);
    let after = rig.tick();
    let c1 = clock_of(&after).expect("a clock change alone emits `tempo`");
    assert_eq!(c1.source, "midi");
    assert_eq!(c1.locked, Some(true));

    // No MIDI clock pulses for longer than the loss timeout
    // (`RuntimeConfig::midi_clock_timeout`, 0.5 s by default): the slave
    // freewheels, and that alone changes `locked`, so `tempo` fires again.
    rig.clock.advance(0.6);
    let after_loss = rig.tick();
    let c2 = clock_of(&after_loss).expect("the clock loss emits `tempo`");
    assert_eq!(c2.source, "midi");
    assert_eq!(c2.locked, Some(false));
}

#[test]
fn old_json_without_the_new_fields_still_decodes() {
    let text =
        r#"{"v":1,"seq":1,"kind":"manifest","body":{"sounds":["bd"],"synths":[],"controls":[]}}"#;
    let env = decode_as::<ServerMsg>(text).expect("decodes");
    let ServerMsg::Manifest(m) = env.body else {
        panic!("{env:?}")
    };
    assert!(m.editors.is_none());

    let text =
        r#"{"v":1,"seq":2,"kind":"tempo","body":{"bpm":120.0,"beats_per_cycle":4,"cycle":[1,1]}}"#;
    let env = decode_as::<ServerMsg>(text).expect("decodes");
    let ServerMsg::Tempo(t) = env.body else {
        panic!("{env:?}")
    };
    assert!(t.clock.is_none());

    let text =
        r#"{"v":1,"seq":3,"kind":"levels","body":{"levels":[{"source":":master","rms":0.1}]}}"#;
    let env = decode_as::<ServerMsg>(text).expect("decodes");
    let ServerMsg::Levels(l) = env.body else {
        panic!("{env:?}")
    };
    assert!(l.levels[0].bands.is_none());
    assert!(l.analyzers.is_none());
}

#[test]
fn check_reports_the_checker_diagnostic_and_never_changes_session_state() {
    let rig = Rig::new();
    let before = rig.audio.calls().len();
    let diags = rig.s.check(DOC, "+ 1 \"a\"\n");
    assert!(diags.iter().any(|d| d.code == "type-mismatch"), "{diags:?}");
    assert_eq!(diags[0].file, DOC);
    assert_eq!(
        rig.audio.calls().len(),
        before,
        "check never reaches the host"
    );
    assert!(
        rig.s.doc(DOC).is_none(),
        "check never opens or mutates a document: {:?}",
        rig.s.doc(DOC)
    );
}

/// A minimal stored-only ZIP writer for the `drive_packages` proxy fixture
/// below: `pkg::tests::support::write_zip` is private to `pkg`, so this
/// builds its own tiny archive (no compression, one entry layout) rather
/// than reaching into another module's test-only code.
fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    fn u16le(b: &mut Vec<u8>, x: u16) {
        b.extend_from_slice(&x.to_le_bytes());
    }
    fn u32le(b: &mut Vec<u8>, x: u32) {
        b.extend_from_slice(&x.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let name = name.as_bytes();
        let size = u32::try_from(data.len()).expect("a small fixture file");
        let offset = u32::try_from(out.len()).expect("a small fixture archive");
        u32le(&mut out, 0x0403_4b50);
        u16le(&mut out, 20);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u32le(&mut out, 0);
        u32le(&mut out, 0);
        u32le(&mut out, size);
        u32le(&mut out, size);
        u16le(
            &mut out,
            u16::try_from(name.len()).expect("a short fixture name"),
        );
        u16le(&mut out, 0);
        out.extend_from_slice(name);
        out.extend_from_slice(data);
        u32le(&mut central, 0x0201_4b50);
        u16le(&mut central, 20);
        u16le(&mut central, 20);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u32le(&mut central, 0);
        u32le(&mut central, 0);
        u32le(&mut central, size);
        u32le(&mut central, size);
        u16le(
            &mut central,
            u16::try_from(name.len()).expect("a short fixture name"),
        );
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u32le(&mut central, 0);
        u32le(&mut central, offset);
        central.extend_from_slice(name);
    }
    let cd_offset = u32::try_from(out.len()).expect("a small fixture archive");
    let cd_size = u32::try_from(central.len()).expect("a small fixture archive");
    let count = u16::try_from(files.len()).expect("few fixture entries");
    out.extend_from_slice(&central);
    u32le(&mut out, 0x0605_4b50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, count);
    u16le(&mut out, count);
    u32le(&mut out, cd_size);
    u32le(&mut out, cd_offset);
    u16le(&mut out, 0);
    out
}

#[test]
fn drive_packages_resolves_over_the_proxy_and_the_import_then_loads() {
    const PATH: &str = "github.com/someone/vactr-wire-fixture";
    let manifest = format!("[package]\npath = \"{PATH}\"\n\n[deps]\n").into_bytes();
    let module = b"let greeting 42\n".to_vec();
    let files: Vec<(&str, &[u8])> = vec![("vactr.toml", &manifest), ("mod.vact", &module)];
    let list_url = format!("/{PATH}/@v/list");
    let toml_url = format!("/{PATH}/@v/v1.0.0.toml");
    let zip_url = format!("/{PATH}/@v/v1.0.0.zip");
    let zip = zip_of(&files);

    let mut rig = Rig::new();
    let mut supplied = Prefetched::new();
    let req = || DriverRequest::Resolve {
        proxy: String::new(),
        requirements: BTreeMap::from([(PATH.to_string(), "v1.0.0".to_string())]),
    };

    let DriverReply::Need { url: n1 } = rig.s.drive_packages(req(), &mut supplied) else {
        panic!("expected the version list first")
    };
    assert_eq!(n1, list_url);
    supplied.supply(n1, b"v1.0.0\n".to_vec());

    let DriverReply::Need { url: n2 } = rig.s.drive_packages(req(), &mut supplied) else {
        panic!("expected the manifest next")
    };
    assert_eq!(n2, toml_url);
    supplied.supply(n2, manifest.clone());

    let DriverReply::Need { url: n3 } = rig.s.drive_packages(req(), &mut supplied) else {
        panic!("expected the zip last")
    };
    assert_eq!(n3, zip_url);
    supplied.supply(n3, zip);

    let done = rig.s.drive_packages(req(), &mut supplied);
    let DriverReply::Done { resolved, .. } = done else {
        panic!("{done:?}")
    };
    let (_, _, sha256_hex) = resolved
        .iter()
        .find(|(p, ..)| p == PATH)
        .expect("the resolved entry");
    let expect = sources_digest(
        &files
            .iter()
            .map(|(p, b)| (Rc::from(*p), Rc::from(*b)))
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        *sha256_hex,
        hex(&expect),
        "the driver's digest is the canonical tree digest of the same sources"
    );

    // The lock `drive_packages` installed lets the next `import` load it.
    let (r, _) = rig.eval(&format!("import {PATH}\nwire-fixture.greeting\n"), 1, 0);
    assert!(
        r.diagnostics.iter().all(|d| d.severity != "error"),
        "{:?}",
        r.diagnostics
    );
    assert_eq!(
        r.forms.last().and_then(|f| f.value.clone()).as_deref(),
        Some("42")
    );
}
