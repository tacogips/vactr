//! TASK-007 criterion 11 (design 10.3): in one span with valid and failing
//! events, the valid events play, each fault carries slot and beat, and the
//! slot's diagnostics clear after a clean cycle. Query faults (a sound
//! missing from the kit) and commit faults (a control outside its domain)
//! are both event-local. One cycle is 2 s (4 beats).

use std::sync::Arc;

use super::{cval, Rig};
use crate::dsp::caps::CapabilitySet;
use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::host::testing::AudioCall;
use crate::host::wire::HostMsg;
use crate::ns::stage::SlotKey;
use crate::sched::runtime::RuntimeConfig;
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;
use crate::vm::fail::FailCode;

#[test]
fn valid_events_play_faults_carry_slot_and_beat_and_clear_after_a_clean_cycle() {
    let mut rig = Rig::new();
    rig.run("var k :sd\ns [:bd k :hh] > d1");
    rig.run_to(0.1);
    // From now on the middle step names a sound the kit lacks.
    rig.run("upd k :nope");
    rig.run_to(3.9);
    let faults = rig.faults();
    assert!(!faults.is_empty());
    for f in &faults {
        assert_eq!(f.code, FailCode::UnknownSound);
        assert_eq!(f.origin.slot, Some(intern_kw("d1")));
        let beat = f.origin.beat.expect("a beat");
        // The middle step of cycle c sits at beat 4c + 4/3.
        let in_cycle = beat.checked_sub(Ratio64::from_int(beat.floor() / 4 * 4));
        assert_eq!(in_cycle.expect("sub"), Ratio64::new(4, 3).expect("4/3"));
    }
    // Cycle 0's middle step and cycle 1's middle step, each reported.
    let beats: Vec<Ratio64> = faults.iter().filter_map(|f| f.origin.beat).collect();
    assert!(beats.contains(&Ratio64::new(4, 3).expect("b")));
    assert!(beats.contains(&Ratio64::new(16, 3).expect("b")));
    // The valid steps of every cycle played.
    let times: Vec<i64> = rig
        .sent()
        .iter()
        .map(|(_, e)| {
            #[allow(clippy::cast_possible_truncation)]
            let ms = (e.time * 3000.0).round() as i64;
            ms
        })
        .collect();
    assert_eq!(
        times,
        vec![0, 4000, 6000, 10000],
        "bd and hh of cycles 0 and 1"
    );
    let cleared_before: usize = rig.ticks.iter().map(|t| t.cleared.len()).sum();
    assert_eq!(cleared_before, 0, "still faulty");
    // Fixed: after one full clean cycle the diagnostics clear, once.
    rig.run("upd k :sd");
    rig.run_to(8.1);
    let cleared: Vec<SlotKey> = rig.ticks.iter().flat_map(|t| t.cleared.clone()).collect();
    assert_eq!(cleared, vec![SlotKey::D(1)]);
}

#[test]
fn a_commit_time_fault_drops_only_its_event() {
    let mut rig = Rig::new();
    rig.run("s [:bd :sd :hh] > vowel [:a :zz :e] > d1");
    rig.run_to(2.1);
    let faults = rig.faults();
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0].code, FailCode::Type);
    assert_eq!(faults[0].origin.slot, Some(intern_kw("d1")));
    assert_eq!(faults[0].origin.beat, Some(Ratio64::new(4, 3).expect("b")));
    assert!(faults[0].origin.span.is_some());
    // The other two steps of cycle 0 and the first of cycle 1 played.
    assert_eq!(rig.sent().len(), 3);
}

/// A loader that succeeds and records what it loaded.
#[derive(Clone, Default)]
struct Loader(std::rc::Rc<std::cell::RefCell<Vec<SampleSrc>>>);

impl SampleLoader for Loader {
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, crate::vm::fail::Failure> {
        self.0.borrow_mut().push(src.clone());
        Ok(Arc::new(SampleData {
            rate: 48_000,
            channels: 1,
            frames: Box::new([0.0; 4]),
        }))
    }
}

fn installs(rig: &Rig) -> Vec<u32> {
    rig.audio
        .calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            AudioCall::InstallSample(id, _) => Some(id),
            _ => None,
        })
        .collect()
}

#[test]
fn samples_are_requested_at_bind_and_an_unseen_one_reports_once_while_loading() {
    let loader = Loader::default();
    let mut rig = Rig::with_loader(
        RuntimeConfig::default(),
        CapabilitySet::native(),
        Box::new(loader.clone()),
    );
    rig.run("var k 0\ns :break > n [k k] > d1");
    // The bind's dry run saw bank index 0: requested before any commit.
    let break_kw = intern_kw("break");
    assert_eq!(
        *loader.0.borrow(),
        vec![SampleSrc::Bank {
            kw: break_kw,
            index: 0
        }]
    );
    assert_eq!(installs(&rig), vec![1]);
    rig.audio.reply(HostMsg::Installed {
        resource: 1,
        gen: 0,
    });
    rig.run_to(0.1);
    let bank = |e: &crate::host::wire::AudioEvent| cval(e, "bank");
    assert_eq!(rig.sent().len(), 1);
    assert_eq!(bank(&rig.sent()[0].1), Some(1.0));
    // Index 1 was never seen by a dry run: its first events drop with ONE
    // host-unavailable fault while the load is requested.
    rig.run("upd k 1");
    rig.run_to(2.1);
    let loading: Vec<_> = rig
        .faults()
        .into_iter()
        .filter(|f| f.code == FailCode::HostUnavailable)
        .collect();
    assert_eq!(loading.len(), 1, "{loading:?}");
    assert!(loading[0].message.contains("still loading"));
    assert_eq!(loading[0].origin.slot, Some(intern_kw("d1")));
    assert_eq!(installs(&rig), vec![1, 2]);
    assert_eq!(rig.sent().len(), 1, "dropped while loading");
    rig.audio.reply(HostMsg::Installed {
        resource: 2,
        gen: 0,
    });
    rig.run_to(4.1);
    let banks: Vec<Option<f32>> = rig.sent().iter().map(|(_, e)| bank(e)).collect();
    assert_eq!(banks, vec![Some(1.0), Some(2.0), Some(2.0)]);
}

#[test]
fn telemetry_publishes_playing_events_with_provenance_and_hits() {
    let mut rig = Rig::new();
    rig.run("s [:bd :sd] > d1");
    rig.run_to(1.1);
    let tel = rig.rt.telemetry();
    assert_eq!(tel.len(), 2);
    assert_eq!(tel[0].slot, intern_kw("d1"));
    assert_eq!(tel[1].beat, Ratio64::from_int(2));
    assert!(
        tel.iter().all(|p| p.src.is_some()),
        "step literal provenance"
    );
    assert!((tel[1].dur - 1.0).abs() < 1e-9);
    assert!(rig.rt.input_cells().hits(intern_kw("d1")) > 0.0);
}
