//! TASK-008 criterion 11 mechanics (headless; the real-worklet proof is
//! BE-WASM's dev harness): per `process()` at most
//! `INSTALL_BYTES_PER_QUANTUM` bytes are copied across a burst, `SliceOk`
//! is withheld until the copy ran, the deferred queue overflows with a
//! fault, admission refuses what does not fit, an unload while playing
//! holds the storage until the voices release it, and the arena capacity
//! never changes after `Engine::new`.

use super::{chain, ctl, event, BrowserRig};
use crate::dsp::arena::{encode_inst, FaultCode, INSTALL_BYTES_PER_QUANTUM, SLICE_BYTES};
use crate::dsp::graph::{BankRef, UGenSpec};
use crate::dsp::ring::{encode_graph_record, encode_sample_begin, encode_slice};
use crate::host::wire::{CtlMsg, HostMsg};

const SLICE: usize = SLICE_BYTES / 4;
const ARENA: usize = 4 << 20;

fn begin(rig: &mut BrowserRig, resource: u32, frames: usize) {
    let b = encode_sample_begin(resource, 1, u32::try_from(frames).unwrap(), 1, 48_000);
    rig.push(&b);
}

fn slice(rig: &mut BrowserRig, resource: u32, offset: usize, value: f32) {
    let mut rec = Vec::new();
    encode_slice(
        resource,
        u32::try_from(offset).unwrap(),
        &vec![value; SLICE],
        &mut rec,
    );
    rig.push(&rec);
}

fn slice_oks(acks: &[HostMsg]) -> usize {
    acks.iter()
        .filter(|m| matches!(m, HostMsg::SliceOk { .. }))
        .count()
}

#[test]
fn a_burst_copies_at_most_one_quantum_credit_per_process() {
    let mut rig = BrowserRig::browser(ARENA);
    let cap = rig.engine.store().capacity_bytes();
    let base = rig.engine.store().arena_ptr();
    for r in 1..=3 {
        begin(&mut rig, r, 3 * SLICE);
    }
    for r in 1..=3 {
        for k in 0..3 {
            slice(&mut rig, r, k * SLICE, 0.25);
        }
    }
    let mut total_oks = 0;
    let mut installed = Vec::new();
    for _ in 0..12 {
        let _ = rig.step();
        let c = *rig.engine.counters();
        assert!(
            c.bytes_copied_quantum <= INSTALL_BYTES_PER_QUANTUM,
            "per-quantum credit"
        );
        let acks = rig.acks();
        let oks = slice_oks(&acks);
        assert!(oks <= 1, "one slice acked per quantum");
        assert_eq!(
            oks * SLICE_BYTES,
            c.bytes_copied_quantum,
            "an ack only after its copy"
        );
        total_oks += oks;
        installed.extend(acks.iter().filter_map(|m| match m {
            HostMsg::Installed { resource, .. } => Some(*resource),
            _ => None,
        }));
    }
    assert_eq!(total_oks, 9);
    assert_eq!(installed, [1, 2, 3]);
    let c = rig.engine.counters();
    assert_eq!(c.bytes_copied_max, INSTALL_BYTES_PER_QUANTUM);
    assert_eq!(c.bytes_copied_total, 9 * SLICE_BYTES as u64);
    assert_eq!(
        rig.engine.store().capacity_bytes(),
        cap,
        "capacity is fixed"
    );
    assert_eq!(rig.engine.store().arena_ptr(), base, "no reallocation");
    assert_eq!(c.memory_capacity, ARENA);
    assert!(rig
        .engine
        .store()
        .get(2)
        .is_some_and(|v| v.data.iter().all(|x| *x == 0.25)));
}

#[test]
fn deferred_queue_overflow_is_a_fault() {
    let mut rig = BrowserRig::browser(ARENA);
    begin(&mut rig, 1, 12 * SLICE);
    for k in 0..12 {
        slice(&mut rig, 1, k * SLICE, 1.0);
    }
    let _ = rig.step();
    let mut overflow = 0;
    while let Some(f) = rig.engine.pop_fault() {
        assert_eq!(f.resource, 1);
        if f.code == FaultCode::InstallQueueOverflow {
            overflow += 1;
        }
    }
    // 12 queued, 1 copied, 8 may wait: 3 overflow.
    assert_eq!(overflow, 3);
}

#[test]
fn an_arena_install_that_does_not_fit_faults() {
    let mut rig = BrowserRig::browser(1 << 16);
    begin(&mut rig, 1, 1 << 20);
    let _ = rig.step();
    let f = rig.engine.pop_fault().expect("a fault");
    assert_eq!(f.code, FaultCode::ArenaExhausted);
    assert_eq!(f.code.diag(), crate::types::diag::DiagCode::ArenaExhausted);
}

#[test]
fn unload_while_playing_holds_storage_until_the_voice_releases_it() {
    let mut rig = BrowserRig::browser(ARENA);
    begin(&mut rig, 5, SLICE);
    slice(&mut rig, 5, 0, 0.5);
    let mut bytes = Vec::new();
    encode_inst(
        &chain(1, vec![UGenSpec::SamplePlay(BankRef::new(5))]),
        &mut bytes,
    )
    .unwrap();
    let mut rec = Vec::new();
    encode_graph_record(100, 1, &bytes, &mut rec);
    rig.push(&rec);
    let _ = rig.step();
    let _ = rig.step();
    let free_before = rig.engine.store().free_floats();
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::PAN, 0.0)]));
    let _ = rig.step();
    rig.post(CtlMsg::SampleRetire { resource: 5 });
    rig.post(CtlMsg::GraphRetire { id: 100 });
    let (l, _) = rig.run(4);
    assert!(
        l.iter().any(|v| *v == 0.5),
        "the voice still reads its sample"
    );
    let acks = rig.acks();
    assert!(
        !super::retired(&acks).contains(&5),
        "held while the voice plays"
    );
    assert_eq!(rig.engine.store().free_floats(), free_before);
    // The 16384-frame sample ends after ~128 blocks.
    let _ = rig.run(140);
    let acks = rig.acks();
    let r = super::retired(&acks);
    assert!(r.contains(&100), "the template retired");
    assert!(r.contains(&5), "the sample retired after its last user");
    assert_eq!(rig.engine.store().free_floats(), free_before + SLICE);
}

#[test]
fn graph_bytes_install_through_the_credit_and_play() {
    let mut rig = BrowserRig::browser(ARENA);
    let mut bytes = Vec::new();
    encode_inst(&chain(1, vec![UGenSpec::Const(1.0)]), &mut bytes).unwrap();
    assert!(bytes.len() < SLICE_BYTES);
    let mut rec = Vec::new();
    encode_graph_record(7, 3, &bytes, &mut rec);
    rig.push(&rec);
    let _ = rig.step();
    assert!(rig.acks().contains(&HostMsg::Installed {
        resource: 7,
        gen: 3
    }));
    assert_eq!(rig.engine.counters().bytes_copied_quantum, bytes.len());
    let t = rig.engine.now();
    rig.send(event(1, t, &[(ctl::PAN, 0.0)]));
    let (l, _) = rig.run(1);
    assert_eq!(l[0], 1.0);
}
