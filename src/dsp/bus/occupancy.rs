//! Conservative available bus geometry under outstanding unadopted claims.
use super::*;
impl BusGraph {
    pub(crate) fn song_contains(&self, key: SongLeaseKey) -> bool {
        self.slots
            .iter()
            .any(|s| s.song_key == Some(key) && s.state != SlotState::Free)
    }
    pub(super) fn slot_withheld_for_song(&self, index: usize, unadopted: u32) -> bool {
        let slot = &self.slots[index];
        let rank = self
            .slots
            .iter()
            .enumerate()
            .filter(|(i, s)| {
                s.state == SlotState::Free
                    && (s.mem.len() > slot.mem.len()
                        || (s.mem.len() == slot.mem.len() && *i < index))
            })
            .count();
        rank < unadopted as usize
    }
    pub(super) fn legacy_slot_with_song_claims(&self, unadopted: u32) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .find(|(i, s)| {
                s.state == SlotState::Free
                    && s.legacy_eligible
                    && !self.slot_withheld_for_song(*i, unadopted)
            })
            .map(|(i, _)| i)
    }
    pub(crate) fn song_available_frames(&self, unadopted: u32) -> Result<u64, SongRejectCode> {
        let free = self
            .slots
            .iter()
            .filter(|s| s.state == SlotState::Free)
            .count();
        if free < unadopted as usize {
            return Err(SongRejectCode::Capacity);
        }
        self.slots
            .iter()
            .enumerate()
            .filter(|(i, s)| {
                s.state == SlotState::Free && !self.slot_withheld_for_song(*i, unadopted)
            })
            .try_fold(0_u64, |n, (_, s)| {
                n.checked_add(u64::try_from(s.mem.len()).map_err(|_| SongRejectCode::Capacity)?)
                    .ok_or(SongRejectCode::Capacity)
            })
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn install_with_song_claims<C: CellRead + ?Sized>(
        &mut self,
        t: &BusTemplate,
        master: bool,
        resource: u32,
        gen: u32,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
        unadopted: u32,
    ) -> bool {
        if !t.valid_sidechains() {
            return false;
        }
        let Some(free) = self.legacy_slot_with_song_claims(unadopted) else {
            return false;
        };
        // Effects with fixed, required state reject a short region before
        // retiring the live chain. Capture effects also require capture
        // allowance; ordinary effects keep their clamped-memory behavior.
        let total = self.slots[free].mem.len();
        let room = effects::mem_len(EffectKind::Room, sr, caps).min(total / 4);
        let mut remaining = total - room;
        for kind in t.kinds.iter().take(t.n.min(MAX_CHAIN)) {
            let want = effects::mem_len(*kind, sr, caps);
            let capture_effect = matches!(
                *kind,
                EffectKind::TextureGrain
                    | EffectKind::TextureStretch
                    | EffectKind::TextureLoop
                    | EffectKind::TextureSpectral
            );
            if capture_effect && caps.max_capture_seconds < effects::texture::CAPTURE_SECONDS {
                return false;
            }
            if effects::requires_full_memory(*kind) && want > remaining {
                return false;
            }
            remaining = remaining.saturating_sub(want);
        }
        for s in self.slots.iter_mut() {
            let same = if master {
                s.master
            } else {
                !s.master && s.bus == t.bus
            };
            if same && s.state == SlotState::Live {
                s.state = SlotState::Retiring;
            }
        }
        let s = &mut self.slots[free];
        s.configure(t, cells, sr, caps);
        s.state = SlotState::Live;
        s.master = master;
        s.bus = t.bus;
        s.resource = resource;
        s.gen = gen;
        s.users = 0;
        self.refresh_song_regions();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Zero;
    impl CellRead for Zero {
        fn get(&self, _: crate::dsp::cells::CellId) -> f32 {
            0.0
        }
    }
    #[test]
    fn actual_unequal_slot_memory_is_withheld_and_individual_fit_still_applies() {
        let caps = CapabilitySet::browser();
        let mut graph = BusGraph::new(4, 16, 1024, &Zero, 48_000., &caps);
        for (index, size) in [(1, 128), (2, 256), (3, 512)] {
            graph.slots[index] = BusSlot::new(16, size);
        }
        graph.refresh_song_regions();
        assert_eq!(graph.song_available_frames(1), Ok(384));
        assert_eq!(graph.legacy_slot_with_song_claims(1), Some(1));
        assert!(graph.slot_withheld_for_song(3, 1));
        let mut t = BusTemplate::new();
        let kind = EffectKind::SpringReverb;
        assert!(effects::requires_full_memory(kind));
        let selected = graph.legacy_slot_with_song_claims(1).unwrap();
        let total = graph.slots[selected].mem.len();
        let room = effects::mem_len(EffectKind::Room, 48_000., &caps).min(total / 4);
        assert!(effects::mem_len(kind, 48_000., &caps) > total - room);
        t.push(kind).unwrap();
        assert!(!graph.install_with_song_claims(&t, false, 40, 1, &Zero, 48_000., &caps, 1));
        assert_eq!(graph.song_available_frames(1), Ok(384));
        let plain = BusTemplate::new();
        assert!(graph.install_with_song_claims(&plain, false, 40, 1, &Zero, 48_000., &caps, 1));
        assert_eq!(graph.song_available_frames(1), Ok(256));
        assert_eq!(graph.slots[3].state, SlotState::Free);
    }
}
