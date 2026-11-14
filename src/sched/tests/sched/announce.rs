//! Announced playing telemetry (design 15.3.8.16 C).

use super::{pat_of, Rig, DT};
use crate::clock::tempo::Tempo;
use crate::dsp::caps::CapabilitySet;
use crate::host::wire::AudioEvent;
use crate::ns::stage::SlotKey;
use crate::pattern::combinators::structure::stack;
use crate::pattern::combinators::time::fast;
use crate::pattern::pat::PParam;
use crate::sched::announce::ANNOUNCE_CAP;
use crate::sched::runtime::RuntimeConfig;
use crate::sched::telemetry::PlayingEvent;
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;
use std::rc::Rc;

type RunOutput = (
    Vec<(f64, AudioEvent)>,
    Vec<PlayingEvent>,
    f32,
    Vec<(f64, PlayingEvent)>,
    Vec<u64>,
);

fn run_pattern(telemetry_lead: f64, seconds: f64) -> RunOutput {
    let mut rig = Rig::with(
        RuntimeConfig {
            telemetry_lead,
            ..RuntimeConfig::default()
        },
        CapabilitySet::native(),
    );
    rig.run("s [:bd :sd :hh :cp] > d1");
    let mut committed = Vec::new();
    let mut ahead = Vec::new();
    let mut retract = Vec::new();
    let mut now = 0.0;
    while now <= seconds + 1e-9 {
        rig.tick_at(now);
        let preview = rig.rt.announced();
        ahead.extend(preview.ahead.into_iter().map(|event| (now, event)));
        retract.extend(preview.retract);
        committed.extend(rig.rt.telemetry());
        now += DT;
    }
    (
        rig.sent(),
        committed,
        rig.rt.input.hits(intern_kw("d1")),
        ahead,
        retract,
    )
}

#[test]
fn announces_at_least_one_hundred_ms_ahead_and_commit_lead_control_is_legacy() {
    assert_eq!(RuntimeConfig::default().telemetry_lead, 0.120);
    let (_, committed, _, ahead, retract) = run_pattern(0.120, 4.5);
    assert!(!ahead.is_empty());
    let mut last_id = 0;
    let mut resolved = std::collections::BTreeMap::<u64, usize>::new();
    for (announced_at, event) in &ahead {
        let id = event.id.expect("announcement id");
        assert!(id > last_id, "ids increase in announce order");
        last_id = id;
        assert!(event.time - announced_at >= 0.100 - 1e-9);
        *resolved.entry(id).or_default() += 0;
    }
    let mut start = 0;
    while start < ahead.len() {
        let tick = ahead[start].0;
        let end = ahead[start..]
            .iter()
            .position(|(at, _)| *at != tick)
            .map_or(ahead.len(), |offset| start + offset);
        assert!(ahead[start..end].windows(2).all(|pair| {
            (pair[0].1.time, pair[0].1.slot, pair[0].1.id)
                <= (pair[1].1.time, pair[1].1.slot, pair[1].1.id)
        }));
        start = end;
    }
    for event in committed
        .iter()
        .filter(|event| event.src.is_some() && event.time > 0.030)
    {
        let id = event.id.expect("committed event confirms its announcement");
        *resolved.entry(id).or_default() += 1;
        assert!(ahead.iter().any(|(announced_at, preview)| {
            preview.id == Some(id)
                && (preview.time - event.time).abs() <= 1e-9
                && event.time - announced_at >= 0.100 - 1e-9
        }));
    }
    for id in retract {
        *resolved.entry(id).or_default() += 1;
    }
    assert!(resolved.values().all(|count| *count == 1));

    let (_, _, _, legacy_ahead, legacy_retract) = run_pattern(0.030, 4.0);
    assert!(legacy_ahead.is_empty());
    assert!(legacy_retract.is_empty());
}

#[test]
fn telemetry_lead_does_not_change_audio_commits_or_hit_windows() {
    let (sent_short, committed_short, hits_short, _, _) = run_pattern(0.030, 20.0);
    let (sent_long, committed_long, hits_long, _, _) = run_pattern(0.120, 20.0);
    assert_eq!(sent_short, sent_long);
    let normalize = |mut events: Vec<PlayingEvent>| {
        for event in &mut events {
            event.id = None;
        }
        events
    };
    assert_eq!(normalize(committed_short), normalize(committed_long));
    assert_eq!(hits_short, hits_long);
}

#[test]
fn pass_cost_is_bounded_and_mute_retracts_a_pending_announcement() {
    let mut rig = Rig::new();
    let base = Rc::new(pat_of(&mut rig, "s [:bd]"));
    let voice = fast(base, PParam::int(64), None);
    let voices = (0..64).map(|_| voice.clone()).collect();
    rig.bind(SlotKey::D(1), stack(voices, None));
    let mut largest_map = 0;
    let mut id = None;
    let mut now = 0.0;
    while now <= 4.0 + 1e-9 {
        rig.tick_at(now);
        let drain = rig.rt.announced();
        id = id.or_else(|| drain.ahead.first().and_then(|event| event.id));
        let stats = rig.rt.announcer.pass_stats();
        largest_map = largest_map.max(stats.map_len);
        assert_eq!(stats.reconcile_emittable_calls, 0);
        assert!(stats.announce_emittable_calls <= 1);
        now += DT;
    }
    assert!(largest_map >= 100, "map held {largest_map} entries");
    assert!(id.is_some());
    assert!(ANNOUNCE_CAP >= largest_map);

    let mut controlled = Rig::with(
        RuntimeConfig {
            telemetry_lead: 0.030,
            ..RuntimeConfig::default()
        },
        CapabilitySet::native(),
    );
    let voice = pat_of(&mut controlled, "s :bd");
    controlled.bind(SlotKey::D(1), voice);
    controlled.tick_at(0.0);
    assert_eq!(
        controlled
            .rt
            .announcer
            .pass_stats()
            .announce_emittable_calls,
        0
    );

    let mut muted = Rig::new();
    muted.run("s [:bd :sd :hh :cp] > d1");
    let mut id = None;
    let mut now = 0.0;
    while now < 0.39 {
        muted.tick_at(now);
        if id.is_none() {
            id = muted
                .rt
                .announced()
                .ahead
                .into_iter()
                .find(|event| event.time > 0.03)
                .and_then(|event| event.id);
        }
        now += DT;
    }
    let id = id.expect("pending announcement");
    muted.rt.slots.iter_mut().next().expect("d1 slot").muted = true;
    muted.tick_at(0.39);
    let retracted = muted.rt.announced().retract;
    assert!(retracted.contains(&id));
}

#[test]
fn same_key_invalidation_confirms_while_rebind_retracts_and_reannounces() {
    let mut rig = Rig::new();
    rig.run("s [:bd :sd :hh :cp] > d1");
    let mut id = None;
    let mut now = 0.0;
    while now < 0.39 {
        rig.tick_at(now);
        id = id.or_else(|| {
            rig.rt
                .announced()
                .ahead
                .into_iter()
                .find(|event| (event.time - 0.5).abs() < 1e-9)
                .and_then(|event| event.id)
        });
        now += DT;
    }
    let id = id.expect("event at 0.5 was announced");
    rig.rt
        .invalidate(SlotKey::D(1), super::span(super::r(1, 4), super::r(1, 2)));
    assert!(rig.rt.requery_dirty(&mut rig.ev, SlotKey::D(1)).is_empty());
    rig.tick_at(0.39);
    let same_key = rig.rt.announced();
    assert!(!same_key.retract.contains(&id));
    assert!(!same_key.ahead.iter().any(|event| event.id == Some(id)));
    let mut confirmation = None;
    let mut now = 0.40;
    while now <= 0.50 + 1e-9 {
        rig.tick_at(now);
        if let Some(id) = rig
            .rt
            .telemetry()
            .into_iter()
            .find(|event| (event.time - 0.5).abs() < 1e-9)
            .and_then(|event| event.id)
        {
            confirmation = Some(id);
        }
        now += DT;
    }
    assert_eq!(confirmation, Some(id));

    let mut rebind = Rig::new();
    rebind.run("s [:bd :sd :hh :cp] > d1");
    let mut old_id = None;
    let mut now = 0.0;
    while now <= 1.94 + 1e-9 {
        rebind.tick_at(now);
        old_id = old_id.or_else(|| {
            rebind
                .rt
                .announced()
                .ahead
                .into_iter()
                .find(|event| (event.time - 2.0).abs() < 1e-9)
                .and_then(|event| event.id)
        });
        now += DT;
    }
    let old_id = old_id.expect("boundary event was announced");
    rebind.run("s [:hh :hh :hh :hh] > d1");
    rebind.tick_at(1.95);
    let resolved = rebind.rt.announced();
    assert!(resolved.retract.contains(&old_id));
    let new_id = resolved
        .ahead
        .iter()
        .find(|event| (event.time - 2.0).abs() < 1e-9)
        .and_then(|event| event.id)
        .expect("new generation preview");
    assert_ne!(new_id, old_id);
}

#[test]
fn tempo_reanchor_retracts_old_time_and_commits_without_an_id() {
    let mut rig = Rig::new();
    rig.run("s [:bd :sd :hh :cp] > d1");
    let mut announced = None;
    let mut now = 0.0;
    while now < 0.39 {
        rig.tick_at(now);
        if announced.is_none() {
            announced = rig
                .rt
                .announced()
                .ahead
                .into_iter()
                .find(|event| (event.time - 0.5).abs() < 1e-9);
        }
        now += DT;
    }
    let announced = announced.expect("0.5 event announced before tempo change");
    let old_id = announced.id.expect("announcement id");
    rig.run("use-bpm 2400");
    rig.tick_at(0.39);
    let retracted = rig.rt.announced().retract;
    assert!(retracted.contains(&old_id));
    let committed = rig.rt.telemetry();
    let boundary_event = committed
        .iter()
        .find(|event| event.src == announced.src && event.time > 0.03)
        .expect("replacement event commits on the tempo-change tick");
    assert!(boundary_event.time <= 0.42);
    assert!(
        boundary_event.id.is_none(),
        "announced={announced:?}, committed={boundary_event:?}"
    );
}

#[test]
fn changed_time_for_same_staging_key_retracts_and_commits_without_reannouncement() {
    let mut rig = Rig::new();
    rig.run("s [:bd :sd :hh :cp] > d1");
    let mut announced = None;
    let mut now = 0.0;
    while now < 0.39 {
        rig.tick_at(now);
        if announced.is_none() {
            announced = rig
                .rt
                .announced()
                .ahead
                .into_iter()
                .find(|event| (event.time - 0.5).abs() < 1e-9);
        }
        now += DT;
    }
    let announced = announced.expect("0.5 event announced before clock re-anchor");
    let old_id = announced.id.expect("announcement id");
    let at = rig.rt.clock.pos();
    rig.rt
        .clock
        .set_tempo(
            Tempo::new(Ratio64::from_int(90), Ratio64::from_int(4)).expect("valid tempo"),
            at,
        )
        .expect("internal clock accepts tempo");
    rig.tick_at(0.39);
    let reanchored = rig.rt.announced();
    assert!(reanchored.retract.contains(&old_id));
    assert!(!reanchored
        .ahead
        .iter()
        .any(|event| event.src == announced.src && event.time > announced.time));

    let mut committed = Vec::new();
    let mut now = 0.40;
    while now <= 0.60 + 1e-9 {
        rig.tick_at(now);
        committed.extend(rig.rt.telemetry());
        now += DT;
    }
    let changed_event = committed
        .iter()
        .filter(|event| event.src == announced.src)
        .min_by(|left, right| {
            (left.time - announced.time)
                .abs()
                .total_cmp(&(right.time - announced.time).abs())
        })
        .expect("same-key re-anchored event commits");
    assert!(
        changed_event.id.is_none(),
        "announced={announced:?}, committed={changed_event:?}"
    );
}

#[test]
fn announce_map_cap_skips_excess_and_leaves_it_for_commit() {
    let mut rig = Rig::new();
    let base = Rc::new(pat_of(&mut rig, "s [:bd]"));
    let dense = fast(base, PParam::int(4096), None);
    let voices = (0..64).map(|_| dense.clone()).collect();
    rig.bind(SlotKey::D(1), stack(voices, None));
    let mut commit_only = Vec::new();
    let mut committed_count = 0;
    let mut idless_times = Vec::new();
    let mut now = 0.0;
    while now <= 0.60 + 1e-9 {
        rig.tick_at(now);
        let stats = rig.rt.announcer.pass_stats();
        assert!(stats.map_len <= ANNOUNCE_CAP);
        assert!(stats.blocked_lanes <= ANNOUNCE_CAP);
        assert!(stats.blocked_keys <= ANNOUNCE_CAP);
        let events = rig.rt.telemetry();
        committed_count += events.len();
        for event in events.into_iter().filter(|event| event.id.is_none()) {
            if idless_times.len() < 8 {
                idless_times.push(event.time);
            }
            if event.time > 0.03 {
                commit_only.push(event);
            }
        }
        let _ = rig.rt.announced();
        if !commit_only.is_empty() {
            break;
        }
        now += DT;
    }
    assert!(rig.rt.announcer.pass_stats().skipped > 0);
    assert!(
        !commit_only.is_empty(),
        "overflow events commit without ids: committed={committed_count}, idless_times={idless_times:?}, stats={:?}",
        rig.rt.announcer.pass_stats()
    );
}

#[test]
fn frozen_tick_leaves_announcement_resolution_for_the_next_normal_tick() {
    let mut rig = Rig::new();
    rig.run("s [:bd :sd :hh :cp] > d1");
    let mut id = None;
    let mut now = 0.0;
    while now < 0.39 {
        rig.tick_at(now);
        if id.is_none() {
            id = rig
                .rt
                .announced()
                .ahead
                .into_iter()
                .find(|event| (event.time - 0.5).abs() < 1e-9)
                .and_then(|event| event.id);
        }
        now += DT;
    }
    let id = id.expect("pending announcement");
    let pending_before = rig.rt.announcer.pass_stats().map_len;
    rig.run("use-clock :midi");
    rig.rt.transport_stop(0.38);
    assert!(rig.rt.midi_clock().is_frozen());
    rig.tick_at(0.39);
    assert_eq!(rig.rt.announcer.pass_stats().map_len, pending_before);
    assert!(!rig.rt.announced().retract.contains(&id));
    rig.run("use-clock :internal");
    rig.tick_at(0.40);
    assert!(rig.rt.announced().retract.contains(&id));
}
