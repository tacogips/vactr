//! Early, retractable previews of staged playing telemetry (design 15.3.8.16 C).

use std::collections::{BTreeMap, BTreeSet};

use crate::pattern::occ::OccKey;
use crate::reader::span::SrcRef;
use crate::sched::runtime::Runtime;
use crate::sched::slots::{SlotId, SlotKind};
use crate::sched::staging::Lane;
use crate::sched::telemetry::PlayingEvent;
use crate::value::ratio::Ratio64;

/// Maximum unresolved announcements, matching the playing envelope bound.
pub const ANNOUNCE_CAP: usize = 4096;

/// An unresolved event's stable staging identity.
pub(crate) type AnnounceKey = (SlotId, u32, OccKey);
type LaneKey = (SlotId, u32);

#[derive(Clone, Copy, Debug)]
struct Announced {
    id: u64,
    time: f64,
    src: Option<SrcRef>,
}

/// Announcements waiting for their commit or retraction.
#[derive(Debug)]
pub(crate) struct Announcer {
    map: BTreeMap<AnnounceKey, Announced>,
    next_id: u64,
    ahead: Vec<PlayingEvent>,
    retract: Vec<u64>,
    blocked_until: BTreeMap<LaneKey, f64>,
    blocked_keys: BTreeSet<AnnounceKey>,
    skipped: u64,
    #[cfg(test)]
    pass_stats: PassStats,
}

impl Default for Announcer {
    fn default() -> Self {
        Self {
            map: BTreeMap::new(),
            next_id: 1,
            ahead: Vec::new(),
            retract: Vec::new(),
            blocked_until: BTreeMap::new(),
            blocked_keys: BTreeSet::new(),
            skipped: 0,
            #[cfg(test)]
            pass_stats: PassStats::default(),
        }
    }
}

/// The preview queues drained by the session publisher.
#[derive(Debug, Default)]
pub struct AnnounceDrain {
    pub ahead: Vec<PlayingEvent>,
    pub retract: Vec<u64>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PassStats {
    pub reconcile_emittable_calls: usize,
    pub announce_emittable_calls: usize,
    pub map_len: usize,
    pub blocked_lanes: usize,
    pub blocked_keys: usize,
    pub skipped: u64,
}

impl Announcer {
    fn next_id(&mut self) -> Option<u64> {
        const MAX_SAFE_ID: u64 = (1_u64 << 53) - 1;
        let id = self.next_id;
        if id > MAX_SAFE_ID {
            return None;
        }
        self.next_id = id.saturating_add(1);
        Some(id)
    }

    /// Resolves a published event to its prior announcement, if it matches.
    pub(crate) fn confirm(
        &mut self,
        key: &AnnounceKey,
        onset: f64,
        src: Option<SrcRef>,
    ) -> Option<u64> {
        self.blocked_keys.remove(key);
        let announced = self.map.remove(key)?;
        if (announced.time - onset).abs() <= 1e-9 && announced.src == src {
            Some(announced.id)
        } else {
            self.retract.push(announced.id);
            None
        }
    }

    /// Resolves announcements whose staged identity or payload is no longer valid.
    fn reconcile(
        &mut self,
        slots: &crate::sched::slots::SlotTable,
        clock: &crate::clock::clock::Clock,
        commit_horizon: f64,
    ) {
        let mut stale = Vec::new();
        let mut changed = Vec::new();
        let mut group = None;
        let mut selected: Option<(&Lane, bool)> = None;
        for (key, announced) in &self.map {
            let current_group = (key.0, key.1);
            if group != Some(current_group) {
                group = Some(current_group);
                selected = slots
                    .iter()
                    .find(|slot| slot.id == key.0)
                    .map(|slot| (slot.lanes.iter().find(|lane| lane.gen == key.1), slot.muted))
                    .and_then(|(lane, muted)| lane.map(|lane| (lane, muted)));
            }
            let current = selected.and_then(|(lane, muted)| {
                if muted {
                    return None;
                }
                let rec = lane.staging.get(&key.2)?;
                let staged = !rec.onset_committed
                    && rec.onset_bearing()
                    && rec.whole.begin >= lane.from
                    && lane.until.is_none_or(|until| rec.whole.begin < until);
                staged.then(|| (clock.to_host(rec.whole.begin), rec.payload.src))
            });
            let matches = current.is_some_and(|(time, src)| {
                (time - announced.time).abs() <= 1e-9 && src == announced.src
            });
            if !matches {
                stale.push(key.clone());
                if current.is_some() {
                    changed.push(key.clone());
                }
            }
        }
        for key in stale {
            if let Some(announced) = self.map.remove(&key) {
                self.retract.push(announced.id);
            }
        }
        self.blocked_until.retain(|(slot_id, gen), until| {
            *until > commit_horizon
                && slots.iter().any(|slot| {
                    slot.id == *slot_id
                        && !slot.muted
                        && slot.lanes.iter().any(|lane| lane.gen == *gen)
                })
        });
        let mut pending_keys = BTreeSet::new();
        let mut blocked_group = None;
        let mut blocked_lane: Option<(&Lane, bool)> = None;
        for key in &self.blocked_keys {
            let current_group = (key.0, key.1);
            if blocked_group != Some(current_group) {
                blocked_group = Some(current_group);
                blocked_lane = slots
                    .iter()
                    .find(|slot| slot.id == key.0)
                    .map(|slot| (slot.lanes.iter().find(|lane| lane.gen == key.1), slot.muted))
                    .and_then(|(lane, muted)| lane.map(|lane| (lane, muted)));
            }
            let Some((lane, false)) = blocked_lane else {
                continue;
            };
            let Some(rec) = lane.staging.get(&key.2) else {
                continue;
            };
            if !rec.onset_committed
                && rec.onset_bearing()
                && rec.whole.begin >= lane.from
                && lane.until.is_none_or(|until| rec.whole.begin < until)
            {
                pending_keys.insert(key.clone());
            }
        }
        self.blocked_keys = pending_keys;
        for key in changed {
            self.block_key(key);
        }
        #[cfg(test)]
        {
            self.pass_stats.reconcile_emittable_calls = 0;
            self.pass_stats.blocked_lanes = self.blocked_until.len();
            self.pass_stats.blocked_keys = self.blocked_keys.len();
        }
    }

    fn block_key(&mut self, key: AnnounceKey) {
        if self.blocked_keys.len() < ANNOUNCE_CAP || self.blocked_keys.contains(&key) {
            self.blocked_keys.insert(key);
        }
    }

    fn block_lane_until(&mut self, lane: LaneKey, until: f64) {
        if self.blocked_until.len() < ANNOUNCE_CAP || self.blocked_until.contains_key(&lane) {
            self.blocked_until
                .entry(lane)
                .and_modify(|blocked| *blocked = blocked.max(until))
                .or_insert(until);
        }
    }

    #[cfg(test)]
    pub(crate) fn pass_stats(&self) -> PassStats {
        self.pass_stats
    }

    pub(crate) fn drain(&mut self) -> AnnounceDrain {
        let mut ahead = std::mem::take(&mut self.ahead);
        ahead.sort_by(|a, b| {
            a.time
                .total_cmp(&b.time)
                .then_with(|| {
                    crate::value::intern::name_of_kw(a.slot)
                        .as_ref()
                        .cmp(crate::value::intern::name_of_kw(b.slot).as_ref())
                })
                .then_with(|| a.id.cmp(&b.id))
        });
        AnnounceDrain {
            ahead,
            retract: std::mem::take(&mut self.retract),
        }
    }
}

impl Runtime {
    /// Announces staged events between the commit and telemetry horizons.
    pub(crate) fn announce_all(&mut self, now: f64) {
        let commit_horizon = now + self.commit_lead;
        self.announcer
            .reconcile(&self.slots, &self.clock, commit_horizon);
        #[cfg(test)]
        {
            self.announcer.pass_stats.announce_emittable_calls = 0;
        }
        let lead = self.commit_lead.max(self.cfg.telemetry_lead);
        if lead <= self.commit_lead {
            #[cfg(test)]
            {
                self.announcer.pass_stats.map_len = self.announcer.map.len();
                self.announcer.pass_stats.skipped = self.announcer.skipped;
            }
            return;
        }
        let announce_horizon = now + lead;
        let (slots, clock, announcer) = (&self.slots, &self.clock, &mut self.announcer);
        let tempo = clock.tempo();
        for slot in slots.iter() {
            if slot.muted || slot.kind != SlotKind::Pattern {
                continue;
            }
            for lane in &slot.lanes {
                #[cfg(test)]
                {
                    announcer.pass_stats.announce_emittable_calls += 1;
                }
                let keys = lane.staging.emittable(lane.from, lane.until);
                for key in keys {
                    let Some(rec) = lane.staging.get(&key) else {
                        continue;
                    };
                    let onset = clock.to_host(rec.whole.begin);
                    if onset <= commit_horizon {
                        continue;
                    }
                    if onset > announce_horizon {
                        break;
                    }
                    if rec.onset_committed || rec.payload.src.is_none() {
                        continue;
                    }
                    let announce_key = (slot.id, lane.gen, key);
                    if announcer.map.contains_key(&announce_key) {
                        continue;
                    }
                    if announcer.blocked_keys.contains(&announce_key) {
                        announcer.skipped = announcer.skipped.saturating_add(1);
                        continue;
                    }
                    let lane_key = (slot.id, lane.gen);
                    if announcer
                        .blocked_until
                        .get(&lane_key)
                        .is_some_and(|blocked| onset <= *blocked)
                    {
                        announcer.skipped = announcer.skipped.saturating_add(1);
                        continue;
                    }
                    if announcer.map.len() >= ANNOUNCE_CAP || announcer.ahead.len() >= ANNOUNCE_CAP
                    {
                        announcer.skipped = announcer.skipped.saturating_add(1);
                        announcer.block_lane_until(lane_key, announce_horizon);
                        continue;
                    }
                    let Some(id) = announcer.next_id() else {
                        announcer.skipped = announcer.skipped.saturating_add(1);
                        continue;
                    };
                    let beat = tempo.beats_at(rec.whole.begin).unwrap_or(Ratio64::ZERO);
                    let dur = (clock.to_host(rec.whole.end) - onset).max(0.0);
                    let src = rec.payload.src;
                    announcer.map.insert(
                        announce_key,
                        Announced {
                            id,
                            time: onset,
                            src,
                        },
                    );
                    announcer.ahead.push(PlayingEvent {
                        slot: slot.name(),
                        beat,
                        time: onset,
                        src,
                        dur,
                        kind: SlotKind::Pattern,
                        reduced_lead: false,
                        id: Some(id),
                    });
                }
            }
        }
        #[cfg(test)]
        {
            announcer.pass_stats.map_len = announcer.map.len();
            announcer.pass_stats.blocked_lanes = announcer.blocked_until.len();
            announcer.pass_stats.blocked_keys = announcer.blocked_keys.len();
            announcer.pass_stats.skipped = announcer.skipped;
        }
    }
}
