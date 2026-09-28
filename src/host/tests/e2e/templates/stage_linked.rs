//! Typed, immutable Stages chain list through `.vact` and editor metadata.

use super::E2e;
use crate::dsp::arena::{decode_graph, encode_inst};
use crate::dsp::bus::BusTemplate;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::cells::Mirror;
use crate::dsp::engine::Engine;
use crate::dsp::ring::{
    encode_graph_record, ByteInbox, EngineConfig, EngineIo, EventRing, SpscRing,
};
use crate::dsp::ugen::{stage_linked, BuildEnv, BuildError, RawGraph, Template};
use crate::host::wire::{AudioEvent, Ctl};
use crate::sched::slots::{CtlId, SlotId};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

fn authored_36(last: f32) -> String {
    let mut rows = vec!["2 0 0.25 0".to_owned(); 35];
    rows.push(format!("2 0 {last} 0"));
    rows.join(" ")
}

fn authored_sequencer_36(last: f32) -> String {
    let mut rows = vec!["0 0 1 0.93".to_owned()];
    rows.extend(std::iter::repeat_n("1 0 0.25 0".to_owned(), 34));
    rows.push(format!("1 0 {last} 0"));
    rows.join(" ")
}

fn authored_source(name: &str, rows: &str) -> String {
    format!("inst {name} segments: [float] = [{rows}]:\n\tstage-linked-core freq segments: segments chain-gate: 1")
}

fn authored_native(last: f32) -> (Vec<f32>, std::sync::Arc<crate::dsp::graph::InstDef>) {
    let mut e = E2e::new();
    e.eval(&authored_source("authored-chain", &authored_36(last)));
    let def = e
        .reg
        .borrow()
        .entries()
        .find(|entry| &*name_of_kw(entry.name) == "authored-chain")
        .unwrap()
        .def
        .clone();
    e.eval("s :authored-chain > note [:a3] > d1 > once");
    let audio = e.run_for(0.25);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(e.committed > 0);
    (audio, def)
}

fn authored_sequencer_native(last: f32) -> (Vec<f32>, std::sync::Arc<crate::dsp::graph::InstDef>) {
    let mut e = E2e::new();
    e.eval(&authored_source(
        "authored-sequencer",
        &authored_sequencer_36(last),
    ));
    let def = e
        .reg
        .borrow()
        .entries()
        .find(|entry| &*name_of_kw(entry.name) == "authored-sequencer")
        .unwrap()
        .def
        .clone();
    e.eval("s :authored-sequencer > note [:a3] > d1 > once");
    let audio = e.run_for(0.25);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(e.committed > 0);
    (audio, def)
}

fn authored_browser(def: &crate::dsp::graph::InstDef) -> Vec<f32> {
    let mut bytes = Vec::new();
    encode_inst(def, &mut bytes).unwrap();
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    let mut cfg = EngineConfig::new(
        &CapabilitySet::browser(),
        48_000.0,
        256,
        crate::dsp::arena::StoreKind::Arena { bytes: 4 << 20 },
    );
    cfg.template_slots = 8;
    cfg.bus_slots = 4;
    cfg.voice_seconds = 0.1;
    cfg.bus_seconds = 1.0;
    cfg.orbits = 2;
    cfg.orbit_delay_seconds = 0.5;
    cfg.analysis_cells = 1024;
    cfg.event_capacity = 256;
    let mut engine = Engine::with_config(cfg);
    let (mut event_tx, mut event_rx) = EventRing::split(256);
    let (mut ack_tx, _ack_rx) = SpscRing::split(256);
    let (mut garbage_tx, _garbage_rx) = SpscRing::split(32);
    let mut cells = Mirror::new(64);
    let mut inbox = ByteInbox::new();
    assert!(inbox.push(&record));
    let mut process = |engine: &mut Engine| {
        let mut buffer = [0.0; 512];
        let mut io = EngineIo {
            events: &mut event_rx,
            controls: &mut inbox,
            acks: &mut ack_tx,
            cells: &mut cells,
            garbage: Some(&mut garbage_tx),
        };
        let (_, allocations) =
            crate::dsp::alloc_probe::armed(|| engine.process(&mut io, &mut buffer, 256));
        assert_eq!(allocations, 0);
        buffer
    };
    for _ in 0..6 {
        let _ = process(&mut engine);
    }
    let mut event = AudioEvent::new(engine.now(), SlotId::new(1), 1, def.id);
    event.push_ctl(CtlId::new(0), Ctl::Const(220.0)).unwrap();
    assert!(event_tx.push(event).is_ok());
    let mut output = Vec::with_capacity(48 * 256);
    for _ in 0..48 {
        let block = process(&mut engine);
        output.extend(block.iter().step_by(2));
    }
    output
}

fn sequencer_example(direction: f32) -> (Vec<f32>, std::sync::Arc<crate::dsp::graph::InstDef>) {
    let mut e = E2e::new();
    let source = include_str!("../../../../../examples/stage-linked.vact")
        .replace("0 0 0 0.07", &format!("0 0 0 {direction}"));
    e.eval(&source);
    let def = e
        .reg
        .borrow()
        .entries()
        .find(|entry| &*name_of_kw(entry.name) == "stage-sequencer-voice")
        .unwrap()
        .def
        .clone();
    e.eval("s :stage-sequencer-voice > note [:a3] > legato 0.3 > once");
    let audio = e.run_for(0.3);
    assert!(e.faults.is_empty(), "{:?}", e.faults);
    assert!(e.committed > 0);
    (audio, def)
}

#[test]
fn public_sequencer_example_clocks_distinct_modes_on_native_and_browser() {
    let (up, up_def) = sequencer_example(0.07);
    let (down, down_def) = sequencer_example(0.22);
    assert!(up.iter().chain(&down).all(|x| x.is_finite()));
    let difference: f32 = up.iter().zip(&down).map(|(a, b)| (a - b).abs()).sum();
    assert!(difference > 1.0, "native direction response: {difference}");
    for def in [&up_def, &down_def] {
        let mut bytes = Vec::new();
        encode_inst(def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        assert_eq!(raw.n_stage_payloads, 2);
        assert!(raw.stage_payloads[0].is_sequencer());
    }
    let browser_up = authored_browser(&up_def);
    let browser_down = authored_browser(&down_def);
    let difference: f32 = browser_up
        .iter()
        .zip(&browser_down)
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(difference > 1.0, "browser direction response: {difference}");
}

#[test]
fn exact_36th_authored_row_survives_public_lowering_and_changes_audio() {
    let (low, low_def) = authored_native(0.1);
    let (high, high_def) = authored_native(0.9);
    assert!(low.iter().chain(&high).all(|v| v.is_finite()));
    let tail = 6_000..9_000;
    let difference: f32 = low[tail.clone()]
        .iter()
        .zip(&high[tail])
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(
        difference > 10.0,
        "36th row must affect the rendered tail: {difference}"
    );
    let browser_low = authored_browser(&low_def);
    let browser_high = authored_browser(&high_def);
    assert!(browser_low
        .iter()
        .chain(&browser_high)
        .all(|v| v.is_finite()));
    let browser_difference: f32 = browser_low[6_000..9_000]
        .iter()
        .zip(&browser_high[6_000..9_000])
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(
        browser_difference > 10.0,
        "browser row-36 response: {browser_difference}"
    );
    for (def, expected) in [(low_def, 0.1), (high_def, 0.9)] {
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        assert_eq!(raw.n_stage_payloads, 1);
        assert_eq!(raw.stage_payloads[0].len, 36);
        assert_eq!(raw.stage_payloads[0].rows[35][2], expected);
        let template = Template::from_inst(
            &def,
            &BuildEnv {
                sr: 48_000.0,
                caps: CapabilitySet::browser(),
                voice_mem: stage_linked::STATE_FLOATS,
            },
        )
        .unwrap();
        assert_eq!(template.stage_payloads[0].rows[35][2], expected);
    }
}

#[test]
fn exact_36th_sequencer_step_survives_vact_native_and_browser() {
    let (low, low_def) = authored_sequencer_native(0.1);
    let (high, high_def) = authored_sequencer_native(0.9);
    let response: f32 = low.iter().zip(&high).map(|(a, b)| (a - b).abs()).sum();
    assert!(response > 10.0, "native row-36 response: {response}");
    let browser_low = authored_browser(&low_def);
    let browser_high = authored_browser(&high_def);
    let response: f32 = browser_low
        .iter()
        .zip(&browser_high)
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(response > 10.0, "browser row-36 response: {response}");
    for (def, expected) in [(low_def, 0.1), (high_def, 0.9)] {
        let mut bytes = Vec::new();
        encode_inst(&def, &mut bytes).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
        assert_eq!(raw.n_stage_payloads, 1);
        assert_eq!(raw.stage_payloads[0].len, 36);
        assert!(raw.stage_payloads[0].is_sequencer());
        assert_eq!(raw.stage_payloads[0].rows[35][2], expected);
    }
}

#[test]
fn opt_in_list_template_is_typed_discoverable_and_exactly_budgeted() {
    let mut e = E2e::new();
    let forms =
        e.ev.eval_str(
            include_str!("../../../../../examples/stage-linked.vact"),
            super::super::SOURCE,
        )
        .unwrap();
    assert!(forms.iter().all(|form| form.value.is_ok()), "{forms:?}");
    let registry = e.reg.borrow();
    let entry = registry
        .entries()
        .find(|entry| &*name_of_kw(entry.name) == "stage-linked-voice")
        .unwrap();
    let decl = instrument_decls(&registry)
        .into_iter()
        .find(|decl| decl.name == "stage-linked-voice")
        .unwrap();
    let list = decl
        .params
        .iter()
        .find(|param| param.name == "segments")
        .unwrap();
    assert_eq!(list.ctl, None);
    assert_eq!(list.curve, "immutable-list");
    assert_eq!(list.range, [1.0, 36.0]);
    for name in ["chain-gate", "chain-trigger"] {
        assert!(decl
            .params
            .iter()
            .any(|param| param.name == name && param.ctl.is_some()));
    }
    let mut bytes = Vec::new();
    encode_inst(&entry.def, &mut bytes).unwrap();
    let mut raw = RawGraph::boxed();
    decode_graph(&bytes, &mut raw, &mut BusTemplate::new()).unwrap();
    assert_eq!(raw.n_stage_payloads, 2);
    assert_eq!(raw.stage_payloads[0].len, 3);
    assert_eq!(raw.stage_payloads[0], raw.stage_payloads[1]);
    for sr in [44_100.0, 48_000.0, 96_000.0] {
        let env = BuildEnv {
            sr,
            caps: CapabilitySet::native(),
            voice_mem: 2 * stage_linked::STATE_FLOATS,
        };
        let template = Template::from_inst(&entry.def, &env).unwrap();
        assert_eq!(template.mem_total, 2 * stage_linked::STATE_FLOATS);
        assert!(template.has_aux);
        assert_eq!(
            Template::from_inst(
                &entry.def,
                &BuildEnv {
                    voice_mem: 2 * stage_linked::STATE_FLOATS - 1,
                    ..env
                }
            )
            .unwrap_err(),
            BuildError::MemExceeded
        );
    }
}

#[test]
fn malformed_authored_lists_fail_before_install() {
    let mut e = E2e::new();
    let row = "2 0 0.3 0.5";
    let valid = vec![row; 36].join(" ");
    let bad = [
        String::new(),
        "2 0 0.3".to_owned(),
        format!("{valid} {row}"),
        "1.5 0 0.3 0.5".to_owned(),
        "2 0.5 0.3 0.5".to_owned(),
        "2 0 -0.1 0.5".to_owned(),
        "2 0 0.3 1.1".to_owned(),
    ];
    let src = |name: &str, list: &str| {
        format!(
        "inst {name} segments: [float] = [{list}]:\n\tstage-linked-core freq segments: segments"
    )
    };
    let valid_result =
        e.ev.eval_str(&src("valid-linked", &valid), super::super::SOURCE)
            .unwrap();
    assert!(
        valid_result.iter().all(|r| r.value.is_ok()),
        "{valid_result:?}"
    );
    for (i, list) in bad.iter().enumerate() {
        let result =
            e.ev.eval_str(&src(&format!("bad-linked-{i}"), list), super::super::SOURCE)
                .unwrap();
        let message = result
            .iter()
            .filter_map(|r| r.value.as_ref().err())
            .map(|failure| failure.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        assert!(!message.is_empty(), "{list}");
        if i == 2 {
            assert!(message.contains("1..36"), "37th row diagnostic: {message}");
        }
    }
}
