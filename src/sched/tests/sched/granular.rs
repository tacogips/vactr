//! TASK-008 criteria 5 and 6, diagnostic half (design 12.6 "Admission",
//! 12.8.8): on the browser capabilities, a committed granular event whose
//! `density`, grain `size` or live capture depth exceeds the tier cap gives
//! ONE `beyond-capability` diagnostic whose origin carries the event's
//! span, slot and beat, and the event still reaches the audio host (the
//! audio side clamps spawning; BE-DSP proves the clamp-and-count half).

use super::{pat_of, with_ctl, Rig, GRAN};
use crate::dsp::caps::CapabilitySet;
use crate::ns::stage::SlotKey;
use crate::sched::runtime::RuntimeConfig;
use crate::types::diag::{DiagCode, RunOrigin};
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;

fn rig() -> Rig {
    let mut rig = Rig::with(RuntimeConfig::default(), CapabilitySet::browser());
    rig.run("let gr sample ./gran.wav");
    rig
}

#[test]
fn over_cap_granular_events_are_diagnosed_with_origin_and_still_commit() {
    let mut rig = rig();
    let base = pat_of(&mut rig, "s gr");
    let span = base.span.expect("the pattern has a source span");
    // d1: density 300 > 200 grains/s; d2: size 0.8 > 0.5 s; d3: a live
    // source read 12 s deep > the 8 s capture buffer; d4: within caps.
    let d1 = with_ctl(base.clone(), "density", Value::Int(300));
    let d2 = with_ctl(
        with_ctl(base.clone(), "density", Value::Int(24)),
        "size",
        Value::Float(0.8),
    );
    let d3 = with_ctl(
        with_ctl(
            with_ctl(base.clone(), "density", Value::Int(24)),
            "source",
            Value::Keyword(intern_kw("live")),
        ),
        "position",
        Value::Int(12),
    );
    let d4 = with_ctl(
        with_ctl(base, "density", Value::Int(100)),
        "size",
        Value::Float(0.25),
    );
    for (n, p) in [(1, d1), (2, d2), (3, d3), (4, d4)] {
        assert!(rig.bind(SlotKey::D(n), p).faults.is_empty());
    }
    rig.run_to(0.1);
    let diags: Vec<_> = rig
        .diags()
        .into_iter()
        .filter(|d| d.code == DiagCode::BeyondCapability)
        .collect();
    assert_eq!(diags.len(), 3, "{diags:#?}");
    let expect = [
        ("d1", "grain density of 300"),
        ("d2", "grain size of 0.8"),
        ("d3", "capture buffer of 12"),
    ];
    for (slot, text) in expect {
        let d = diags
            .iter()
            .find(|d| d.message.contains(text))
            .unwrap_or_else(|| panic!("no diagnostic for {slot}: {diags:#?}"));
        assert!(d.message.contains("not available on this host"));
        assert_eq!(d.span, span, "origin span of the event");
        assert_eq!(
            d.origin,
            Some(RunOrigin {
                slot: Some(intern_kw(slot)),
                beat: Some(Ratio64::ZERO),
            })
        );
    }
    // Every event, over the cap or not, reached the audio host once.
    let sent: Vec<u32> = rig
        .sent()
        .iter()
        .filter(|(_, e)| e.inst == GRAN)
        .map(|(_, e)| e.slot.get())
        .collect();
    assert_eq!(sent, vec![1, 2, 3, 4]);
    // One diagnostic per committed event: the next cycle's events are
    // checked again (beat 4), each once.
    rig.run_to(2.1);
    let second: Vec<_> = rig
        .diags()
        .into_iter()
        .filter(|d| {
            d.code == DiagCode::BeyondCapability
                && d.origin.as_ref().and_then(|o| o.beat) == Some(Ratio64::from_int(4))
        })
        .collect();
    assert_eq!(second.len(), 3);
}

#[test]
fn native_caps_admit_the_same_events_without_diagnostics() {
    let mut rig = Rig::new();
    rig.run("let gr sample ./gran.wav");
    let base = pat_of(&mut rig, "s gr");
    let p = with_ctl(
        with_ctl(base, "density", Value::Int(300)),
        "size",
        Value::Float(0.8),
    );
    assert!(rig.bind(SlotKey::D(1), p).faults.is_empty());
    rig.run_to(0.1);
    assert!(rig
        .diags()
        .iter()
        .all(|d| d.code != DiagCode::BeyondCapability));
    assert_eq!(rig.sent().len(), 1);
}
