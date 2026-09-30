//! BE-CONTRACTS DSP tests: the cell mirror, capabilities, the control
//! table, the allocation probe, voice release and the effect catalog
//! (design 11.3, 11.7, 12.7, 12.8.7-12.8.9).

use std::collections::BTreeSet;

use crate::dsp::alloc_probe::armed;
use crate::dsp::caps::{Cap, CapabilitySet};
use crate::dsp::cells::{AtomicCells, CellId, CellRead, CellState, Mirror};
use crate::dsp::controls::{encode, row, row_by_id, CtlDomain, CtlRoute, ROWS};
use crate::dsp::graph::{AnalyzerKind, EffectKind};
use crate::dsp::release::{TagMap, Tombstones, TOMBSTONES};
use crate::host::wire::{HostMsg, VoiceTag};
use crate::reader::span::{FileId, Span};
use crate::sched::slots::SlotId;
use crate::types::diag::DiagCode;
use crate::value::value::Value;
use crate::vm::fail::FailCode;

const C: CellId = CellId::new(2);

#[test]
fn mirror_replayed_init_keeps_the_batch_value() {
    let mut m = Mirror::new(4);
    assert_eq!(
        m.apply_init(C, 1, 0.1),
        Some(HostMsg::CellInitAck { cell: C, epoch: 1 })
    );
    assert_eq!(
        m.apply_batch(1, [(C, 1, 0.9)]),
        Some(HostMsg::CellBatchAck { seq: 1 })
    );
    // A retransmitted init of the same epoch is acknowledge-only.
    assert_eq!(
        m.apply_init(C, 1, 0.1),
        Some(HostMsg::CellInitAck { cell: C, epoch: 1 })
    );
    assert_eq!(m.get(C), 0.9);
    assert_eq!(m.last_epoch(C), 1);
}

#[test]
fn mirror_stale_epoch_update_is_inert() {
    let mut m = Mirror::new(4);
    let _ = m.apply_init(C, 2, 0.5);
    let _ = m.apply_batch(1, [(C, 1, 0.8), (CellId::new(3), 2, 0.8)]);
    assert_eq!(m.get(C), 0.5);
    assert_eq!(m.state(CellId::new(3)), Some(CellState::Vacant));
    // A stale init (below the initialized epoch) is ack-only too.
    assert_eq!(
        m.apply_init(C, 1, 0.0),
        Some(HostMsg::CellInitAck { cell: C, epoch: 1 })
    );
    assert_eq!(m.get(C), 0.5);
}

#[test]
fn mirror_retire_then_reuse_with_a_greater_epoch() {
    let mut m = Mirror::new(4);
    let _ = m.apply_init(C, 1, 0.5);
    assert!(!m.retire(C, 2), "wrong epoch");
    assert!(m.retire(C, 1));
    assert_eq!(m.state(C), Some(CellState::Retiring { epoch: 1 }));
    assert_eq!(m.get(C), 0.0);
    assert_eq!(
        m.retired(C),
        Some(HostMsg::CellRetired { cell: C, epoch: 1 })
    );
    assert_eq!(m.state(C), Some(CellState::Vacant));
    assert_eq!(m.retired(C), None);
    let _ = m.apply_init(C, 2, 0.25);
    assert_eq!(
        m.state(C),
        Some(CellState::Live {
            epoch: 2,
            value: 0.25
        })
    );
    // An old-epoch update after reuse stays inert.
    let _ = m.apply_batch(1, [(C, 1, 1.0)]);
    assert_eq!(m.get(C), 0.25);
}

#[test]
fn mirror_supersedes_on_a_greater_epoch() {
    let mut m = Mirror::new(4);
    let _ = m.apply_init(C, 1, 0.5);
    assert!(m.retire(C, 1));
    // The retire ack was lost; a newer incarnation supersedes it.
    let _ = m.apply_init(C, 3, 0.75);
    assert_eq!(
        m.state(C),
        Some(CellState::Live {
            epoch: 3,
            value: 0.75
        })
    );
}

#[test]
fn mirror_rejects_a_non_increasing_batch_seq() {
    let mut m = Mirror::new(4);
    let _ = m.apply_init(C, 1, 0.0);
    let _ = m.apply_batch(5, [(C, 1, 0.5)]);
    // A replay of the last seq is re-acknowledged without applying.
    assert_eq!(
        m.apply_batch(5, [(C, 1, 0.9)]),
        Some(HostMsg::CellBatchAck { seq: 5 })
    );
    assert_eq!(m.apply_batch(4, [(C, 1, 0.9)]), None);
    assert_eq!(m.get(C), 0.5);
    assert_eq!(m.last_seq(), Some(5));
    let _ = m.apply_batch(6, [(C, 1, 0.9)]);
    assert_eq!(m.get(C), 0.9);
}

#[test]
fn mirror_out_of_range_is_never_acknowledged() {
    let mut m = Mirror::new(2);
    assert_eq!(m.apply_init(CellId::new(2), 1, 1.0), None);
    assert!(!m.retire(CellId::new(9), 1));
    assert_eq!(m.get(CellId::new(9)), 0.0);
    assert_eq!(m.capacity(), 2);
}

#[test]
fn atomic_cells_share_values() {
    let cells = AtomicCells::new(3);
    let reader = cells.clone();
    assert!(cells.set(CellId::new(1), -0.5));
    assert!(!cells.set(CellId::new(3), 1.0));
    assert_eq!(reader.get(CellId::new(1)), -0.5);
    assert_eq!(reader.get(CellId::new(3)), 0.0);
    assert_eq!(reader.capacity(), 3);
}

#[test]
fn browser_refuses_offline_render_and_long_irs_with_origin() {
    let span = Span::new(FileId::new(4), 10, 22);
    let browser = CapabilitySet::browser();
    for cap in [Cap::OfflineRender, Cap::IrSeconds(3.0)] {
        let d = browser.require(cap, Some(span)).expect_err("refused");
        assert_eq!(d.code, DiagCode::BeyondCapability);
        assert!(d.message.contains("not available on this host"), "{d}");
        assert_eq!(d.span, span);
    }
    let d = browser
        .require(Cap::IrSeconds(3.0), Some(span))
        .expect_err("refused");
    assert!(d.message.contains("limit 2s"), "{d}");
    assert!(browser.require(Cap::IrSeconds(f32::NAN), None).is_err());
    assert!(browser.require(Cap::MidiIn, None).is_err());
    assert!(browser.require(Cap::Voices(64), None).is_ok());
    assert!(browser.require(Cap::Voices(65), None).is_err());
}

#[test]
fn native_accepts_its_advertised_limits() {
    let native = CapabilitySet::native();
    assert!(native.require(Cap::IrSeconds(3.0), None).is_ok());
    assert!(native.require(Cap::IrSeconds(10.0), None).is_ok());
    assert!(native.require(Cap::Voices(256), None).is_ok());
    assert!(native.require(Cap::GrainDensity(1000.0), None).is_ok());
    assert!(native.require(Cap::GrainSize(2.0), None).is_ok());
    assert!(native.require(Cap::CaptureSeconds(30.0), None).is_ok());
    for cap in [Cap::MidiIn, Cap::MidiOut, Cap::FileAccess] {
        assert!(native.require(cap, None).is_ok());
    }
    // Offline `render` is a native-tier capability (design 12.3 self-analysis
    // amendment, 2026-09-25); the browser tier still reports it unavailable.
    assert!(native.require(Cap::OfflineRender, None).is_ok());
    assert!(native.require(Cap::IrSeconds(10.5), None).is_err());
}

#[test]
fn gain_routes_to_amp() {
    let gain = row("gain").expect("gain");
    let amp = row("amp").expect("amp");
    assert_eq!(gain.ctl, amp.ctl);
    assert_eq!(gain.route, CtlRoute::InstParam);
    assert_eq!(row_by_id(gain.ctl).map(|r| r.name), Some("amp"));
    assert_eq!(
        row("room").map(|r| r.route),
        Some(CtlRoute::BusUnit { param: 0 })
    );
    for name in ["orbit", "bus", "cut", "legato"] {
        assert_eq!(
            row(name).map(|r| r.route),
            Some(CtlRoute::Scheduler),
            "{name}"
        );
    }
}

#[test]
fn control_rows_cover_the_design_names_once() {
    let names: BTreeSet<&str> = ROWS.iter().map(|r| r.name).collect();
    assert_eq!(names.len(), ROWS.len(), "names are unique");
    let pattern = "gain pan speed lpf hpf resonance room size delay delaytime \
        delayfeedback crush shape vowel legato attack release sustain begin end cut orbit velocity";
    let templates = "attack decay sustain release cutoff res wave unison detune drift ratio \
        index algorithm position table bank loop size density spray pitch pitch-spray envelope \
        reverse freeze stereo-spray source shape";
    for name in pattern
        .split_whitespace()
        .chain(templates.split_whitespace())
        .chain(["freq", "amp", "note", "n", "bus"])
    {
        assert!(row(name).is_some(), "missing control `{name}`");
    }
    // Ids are unique except `gain`, which shares `amp`'s.
    let ids: BTreeSet<u16> = ROWS.iter().map(|r| r.ctl.get()).collect();
    assert_eq!(ids.len(), ROWS.len() - 1);
    for r in ROWS {
        assert_eq!(row_by_id(r.ctl).map(|x| x.ctl), Some(r.ctl));
    }
}

#[test]
fn encode_keywords_bools_and_numbers() {
    let envelope = row("envelope").expect("envelope");
    let CtlDomain::Enum(names) = envelope.domain else {
        panic!("envelope is an enum");
    };
    let hann = names.iter().position(|n| *n == "hann").expect("hann");
    assert_eq!(encode(envelope, &Value::kw("hann")), Ok(hann as f32));
    assert_eq!(encode(envelope, &Value::kw("expo")), Ok(3.0));
    let freeze = row("freeze").expect("freeze");
    assert_eq!(encode(freeze, &Value::Bool(true)), Ok(1.0));
    assert_eq!(encode(freeze, &Value::Bool(false)), Ok(0.0));
    let cutoff = row("cutoff").expect("cutoff");
    assert_eq!(encode(cutoff, &Value::Int(800)), Ok(800.0));
    assert_eq!(encode(cutoff, &Value::Float64(0.5)), Ok(0.5));
    for (r, v) in [
        (envelope, Value::kw("square")),
        (envelope, Value::Int(1)),
        (freeze, Value::Int(1)),
        (cutoff, Value::kw("hann")),
        (row("bank").expect("bank"), Value::kw("bd-haus")),
    ] {
        let err = encode(r, &v).expect_err("outside the domain");
        assert_eq!(err.code, FailCode::Type, "{}", r.name);
    }
}

#[test]
fn alloc_probe_counts_only_armed_allocations() {
    let (v, n) = armed(|| Vec::<u8>::with_capacity(8));
    assert!(n >= 1);
    assert_eq!(v.capacity(), 8);
    let (x, n) = armed(|| 1 + 1);
    assert_eq!((x, n), (2, 0));
    // Nested arming restores the outer state and counts per call.
    let ((_, inner), outer) = armed(|| armed(|| Box::new(5u32)));
    assert_eq!((inner, outer), (1, 1));
}

fn vtag(pitch: u8, seq: u32) -> VoiceTag {
    VoiceTag {
        slot: SlotId::new(1),
        channel: 0,
        pitch,
        seq,
    }
}

#[test]
fn tag_map_insert_release_and_steal() {
    let mut map = TagMap::new(3);
    let (_, allocs) = armed(|| {
        assert!(map.insert(vtag(60, 1), 0));
        assert!(map.insert(vtag(62, 2), 1));
        assert!(map.insert(vtag(64, 3), 2));
        assert!(!map.insert(vtag(65, 4), 0), "full");
        assert_eq!(map.oldest(), Some((vtag(60, 1), 0)));
        assert_eq!(map.release(vtag(62, 2)), Some(1));
        assert_eq!(map.release(vtag(62, 2)), None);
        assert_eq!(map.voice_of(vtag(64, 3)), Some(2));
        // Steal the oldest: close its tag, reuse its voice.
        let (_, voice) = map.oldest().expect("open voice");
        assert_eq!(map.remove_voice(voice), Some(vtag(60, 1)));
        assert!(map.insert(vtag(65, 4), voice));
        assert!(map.insert(vtag(67, 5), 1));
        assert_eq!(map.oldest(), Some((vtag(64, 3), 2)));
    });
    assert_eq!(allocs, 0, "no allocation after construction");
    assert_eq!((map.len(), map.capacity()), (3, 3));
    assert!(!map.is_empty());
}

#[test]
fn tombstones_drop_a_note_on_after_its_release() {
    let mut t = Tombstones::new();
    let (_, allocs) = armed(|| {
        t.push(vtag(60, 9));
        assert!(t.contains(vtag(60, 9)));
        assert!(t.take(vtag(60, 9)));
        assert!(!t.take(vtag(60, 9)));
        for seq in 0..u32::try_from(TOMBSTONES + 1).expect("small") {
            t.push(vtag(61, seq));
        }
    });
    assert_eq!(allocs, 0);
    assert!(!t.contains(vtag(61, 0)), "the oldest was overwritten");
    assert!(t.contains(vtag(61, 1)));
    assert!(t.contains(vtag(61, u32::try_from(TOMBSTONES).expect("small"))));
}

#[test]
fn effect_catalog_names_round_trip() {
    let names: BTreeSet<&str> = EffectKind::ALL.iter().map(|k| k.name()).collect();
    assert_eq!(names.len(), EffectKind::ALL.len());
    // Eighteen MusicDSP families and composite lo-fi append after the legacy catalog.
    // The frozen byte-ID fixture separately verifies every legacy position.
    assert_eq!(
        EffectKind::ALL.len(),
        103 + 7 + AnalyzerKind::ALL.len() + 19
    );
    for k in EffectKind::ALL {
        assert_eq!(EffectKind::from_name(k.name()), Some(*k));
    }
    assert_eq!(
        EffectKind::from_name("multiband-compressor"),
        Some(EffectKind::MultibandCompressor)
    );
    assert_eq!(
        EffectKind::from_name("ping-pong"),
        Some(EffectKind::PingPong)
    );
    assert_eq!(
        EffectKind::from_name("pitch-meter"),
        Some(EffectKind::Analyzer(AnalyzerKind::PitchMeter))
    );
    assert_eq!(
        EffectKind::from_name("granulate"),
        Some(EffectKind::Granulate)
    );
    assert_eq!(EffectKind::from_name("reverb"), None);
}
