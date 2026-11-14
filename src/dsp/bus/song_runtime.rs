//! Exact private song buses and retained reset recipes.
use super::*;
impl BusGraph {
    /// Configure a silent private bus while retaining its original owned box.
    /// # Errors
    /// Invalid kind/geometry or no individual free slot large enough.
    pub fn stage_song_bus<C: CellRead + ?Sized>(
        &mut self,
        key: crate::song::routing::SongLeaseKey,
        template: Box<BusTemplate>,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Result<(), Box<BusTemplate>> {
        use crate::song::routing::SongResourceKind;
        if !matches!(
            key.kind,
            SongResourceKind::PrivateFx | SongResourceKind::Track | SongResourceKind::Master
        ) || template.n > MAX_CHAIN
            || !template.valid_sidechains()
            || template
                .n_params
                .iter()
                .take(template.n)
                .any(|n| usize::from(*n) > MAX_FX_PARAMS)
            || self.slots.iter().any(|slot| slot.song_key == Some(key))
        {
            return Err(template);
        }
        let Ok(need) = Self::song_bus_need(key, &template, sr, caps) else {
            return Err(template);
        };
        let Some(index) = self.best_song_slot(key, need, sr, caps) else {
            return Err(template);
        };
        let slot = &mut self.slots[index];
        if key.kind == SongResourceKind::PrivateFx {
            if slot
                .configure_song_private(key, &template, cells, sr, caps)
                .is_err()
            {
                return Err(template);
            }
        } else {
            slot.configure(&template, cells, sr, caps);
        }

        slot.song_definition = Some(*template);
        slot.state = SlotState::Staged;
        slot.master = key.kind == SongResourceKind::Master;
        slot.bus = template.bus;
        slot.resource = key.resource.id;
        slot.gen = key.resource.generation;
        slot.song_key = Some(key);
        slot.song_template = Some(template);
        self.refresh_song_regions();
        Ok(())
    }

    pub(crate) fn stage_song_arena<C: CellRead + ?Sized>(
        &mut self,
        key: crate::song::routing::SongLeaseKey,
        template: &BusTemplate,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Result<(), crate::song::routing::SongRejectCode> {
        use crate::song::routing::{SongRejectCode, SongResourceKind};
        if !matches!(
            key.kind,
            SongResourceKind::PrivateFx | SongResourceKind::Track | SongResourceKind::Master
        ) || template.n > MAX_CHAIN
            || !template.valid_sidechains()
            || template
                .n_params
                .iter()
                .take(template.n)
                .any(|n| usize::from(*n) > MAX_FX_PARAMS)
        {
            return Err(SongRejectCode::Malformed);
        }
        let need = Self::song_bus_need(key, template, sr, caps)?;
        let index = self
            .best_song_slot(key, need, sr, caps)
            .ok_or(SongRejectCode::Capacity)?;
        let slot = &mut self.slots[index];
        if key.kind == SongResourceKind::PrivateFx {
            slot.configure_song_private(key, template, cells, sr, caps)?;
        } else {
            slot.configure(template, cells, sr, caps);
        }
        slot.song_definition = Some(*template);
        slot.state = SlotState::Staged;
        slot.master = key.kind == SongResourceKind::Master;
        slot.bus = template.bus;
        slot.resource = key.resource.id;
        slot.gen = key.resource.generation;
        slot.song_key = Some(key);
        self.refresh_song_regions();
        Ok(())
    }

    fn song_bus_need(
        key: SongLeaseKey,
        template: &BusTemplate,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Result<usize, SongRejectCode> {
        if key.kind == crate::song::routing::SongResourceKind::PrivateFx {
            let (room, chain, delay) = BusSlot::private_need(template, sr, caps)?;
            return room
                .checked_add(chain)
                .and_then(|n| delay.checked_mul(2).and_then(|d| n.checked_add(d)))
                .ok_or(SongRejectCode::Capacity);
        }
        let room = effects::mem_len(EffectKind::Room, sr, caps);
        template
            .kinds
            .iter()
            .take(template.n)
            .try_fold(room, |n, kind| {
                n.checked_add(effects::mem_len(*kind, sr, caps))
                    .ok_or(SongRejectCode::Capacity)
            })
    }
    fn best_song_slot(
        &self,
        key: SongLeaseKey,
        need: usize,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Option<usize> {
        let room = effects::mem_len(EffectKind::Room, sr, caps);
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| {
                let actual = if key.kind == crate::song::routing::SongResourceKind::PrivateFx {
                    Some(need)
                } else {
                    need.checked_sub(room)
                        .and_then(|chain| chain.checked_add(room.min(slot.mem.len() / 4)))
                };
                slot.state == SlotState::Free && actual.is_some_and(|n| slot.mem.len() >= n)
            })
            .min_by_key(|(index, slot)| (slot.mem.len(), *index))
            .map(|(index, _)| index)
    }

    pub(crate) fn song_take_template(
        &mut self,
        key: crate::song::routing::SongLeaseKey,
    ) -> Option<Box<BusTemplate>> {
        self.slots
            .iter_mut()
            .find(|slot| slot.song_key == Some(key))?
            .song_template
            .take()
    }

    pub(crate) fn song_finish_cancel(&mut self, key: crate::song::routing::SongLeaseKey) -> bool {
        let Some(slot) = self
            .slots
            .iter_mut()
            .find(|slot| slot.song_key == Some(key))
        else {
            return false;
        };
        if slot.song_template.is_some() || slot.state != SlotState::Staged {
            return false;
        }
        slot.song_delay.owner = None;
        slot.song_key = None;
        slot.song_definition = None;
        slot.state = SlotState::Free;
        slot.master = false;
        self.refresh_song_regions();
        true
    }
}
impl BusGraph {
    pub(crate) fn song_index(&self, key: crate::song::routing::SongLeaseKey) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.song_key == Some(key) && s.state != SlotState::Free)
    }
    pub(crate) fn activate_song_bus(
        &mut self,
        key: crate::song::routing::SongLeaseKey,
    ) -> Result<usize, crate::song::routing::SongRejectCode> {
        let i = self
            .song_index(key)
            .ok_or(crate::song::routing::SongRejectCode::StaleEpoch)?;
        if self.slots[i].state != SlotState::Staged {
            return Err(crate::song::routing::SongRejectCode::NotReady);
        }
        self.slots[i].state = SlotState::Live;
        Ok(i)
    }
    pub(crate) fn close_song_bus_for_return(
        &mut self,
        key: crate::song::routing::SongLeaseKey,
    ) -> Result<(), crate::song::routing::SongRejectCode> {
        let i = self
            .song_index(key)
            .ok_or(crate::song::routing::SongRejectCode::StaleEpoch)?;
        self.slots[i].state = SlotState::Staged;
        Ok(())
    }
    pub(crate) fn run_song<C: CellRead + ?Sized>(
        &mut self,
        index: usize,
        frames: usize,
        cells: &C,
        dry: &mut [f32],
        ctx: &mut FxCtx<'_>,
    ) {
        let epoch = self.slots[index].song_key.map(|k| k.epoch);
        for (source, snapshot) in self.slots.iter().zip(self.snapshots.iter_mut()) {
            snapshot.active = source.song_key.is_some_and(|k| Some(k.epoch) == epoch)
                && matches!(source.state, SlotState::Live | SlotState::Retiring);
            snapshot.bus = source.bus;
            if snapshot.active {
                snapshot.l[..frames].copy_from_slice(&source.l[..frames]);
                snapshot.r[..frames].copy_from_slice(&source.r[..frames]);
            }
        }
        // Private smoothing must follow the sample clock, independent of callback partitions.
        for frame in 0..frames {
            self.slots[index].run_range(
                frame..frame + 1,
                cells,
                dry,
                ctx,
                &self.snapshots,
                &mut self.key,
            );
        }
    }
    pub(crate) fn add_song_output(&mut self, from: usize, to: usize, frames: usize) {
        if from == to {
            return;
        }
        let (source, target) = if from < to {
            let (a, b) = self.slots.split_at_mut(to);
            (&a[from], &mut b[0])
        } else {
            let (a, b) = self.slots.split_at_mut(from);
            (&b[0], &mut a[to])
        };
        for k in 0..frames {
            target.l[k] += source.l[k];
            target.r[k] += source.r[k];
        }
    }
}

impl BusGraph {
    pub(crate) fn reset_song_bus<C: CellRead + ?Sized>(
        &mut self,
        key: SongLeaseKey,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Result<(), SongRejectCode> {
        let i = self.song_index(key).ok_or(SongRejectCode::StaleEpoch)?;
        let t = self.slots[i]
            .song_definition
            .ok_or(SongRejectCode::NotReady)?;
        let room = self.slots[i].room();
        let size = self.slots[i].room.target(ROOM_SIZE);
        if key.kind == crate::song::routing::SongResourceKind::PrivateFx {
            self.slots[i].configure_song_private(key, &t, cells, sr, caps)?;
        } else {
            self.slots[i].configure(&t, cells, sr, caps);
        }
        self.slots[i].set_room(Some(room), Some(size));
        self.slots[i].l.fill(0.0);
        self.slots[i].r.fill(0.0);
        Ok(())
    }
    pub(crate) fn add_song_output_gated(
        &mut self,
        from: usize,
        to: usize,
        frames: usize,
        first_frame: u64,
        gate: crate::dsp::song::SongGainGate,
    ) {
        if from == to {
            return;
        }
        let (source, target) = if from < to {
            let (a, b) = self.slots.split_at_mut(to);
            (&a[from], &mut b[0])
        } else {
            let (a, b) = self.slots.split_at_mut(from);
            (&b[0], &mut a[to])
        };
        for k in 0..frames {
            let gain = gate.gain(first_frame.saturating_add(k as u64));
            target.l[k] += source.l[k] * gain;
            target.r[k] += source.r[k] * gain;
        }
    }
}

impl BusGraph {
    pub(crate) fn can_reset_song_bus(&self, key: SongLeaseKey) -> bool {
        self.song_index(key)
            .is_some_and(|i| self.slots[i].song_definition.is_some())
    }
}

impl BusGraph {
    pub(crate) fn song_is_live(&self, key: SongLeaseKey, index: usize) -> bool {
        self.slots.get(index).is_some_and(|s| {
            s.song_key == Some(key) && matches!(s.state, SlotState::Live | SlotState::Retiring)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp::arena::{SampleStore, StoreKind};
    use crate::dsp::cells::AtomicCells;
    use crate::dsp::effects::FxStats;
    use crate::dsp::fft::{Fft, FFT_SIZE};
    use crate::dsp::song::{RuntimeTrack, SongEpochEnd};
    use crate::song::routing::{SongResourceKind, SongResourceRef};
    use crate::song::SnapshotEpoch;

    #[test]
    fn heterogeneous_actual_box_reset_return_and_claim_shielding() {
        use crate::dsp::ring::SongBusMemoryProfile;
        let caps = CapabilitySet::browser();
        let cells = AtomicCells::new(1);
        let mut graph = BusGraph::new_with_memory_profile(
            8,
            16,
            144000,
            Some(SongBusMemoryProfile {
                full_slots: 6,
                small_chain_seconds: 1.,
            }),
            &cells,
            8000.,
            &caps,
        )
        .unwrap();
        let key = SongLeaseKey {
            epoch: SnapshotEpoch(81),
            resource: SongResourceRef {
                id: 1,
                generation: 9,
            },
            kind: SongResourceKind::PrivateFx,
        };
        let original = Box::new(BusTemplate::new());
        let pointer = std::ptr::from_ref(original.as_ref());
        graph
            .stage_song_bus(key, original, &cells, 8000., &caps)
            .unwrap();
        assert_eq!(graph.song_index(key), Some(6));
        assert!(graph.slot_withheld_for_song(1, 1));
        assert_eq!(graph.legacy_slot_with_song_claims(1), Some(2));
        let (reset, count) =
            crate::dsp::alloc_probe::armed(|| graph.reset_song_bus(key, &cells, 8000., &caps));
        assert_eq!(reset, Ok(()));
        assert_eq!(count, 0);
        let returned = graph.song_take_template(key).unwrap();
        assert_eq!(std::ptr::from_ref(returned.as_ref()), pointer);
        assert!(graph.song_finish_cancel(key));
        assert!(!graph.song_finish_cancel(key));
        assert!(graph.song_regions()[6].frames > 0);
        drop(returned);
    }

    #[test]
    fn actual_native_and_arena_partial_return_reuse_cannot_reset_new_stage() {
        for native in [false, true] {
            let caps = CapabilitySet::browser();
            let cells = AtomicCells::new(1);
            let mut graph = BusGraph::new(4, 16, 8192, &cells, 48000., &caps);
            let key = |epoch, id, kind| SongLeaseKey {
                epoch: SnapshotEpoch(epoch),
                resource: SongResourceRef { id, generation: 3 },
                kind,
            };
            let old_track = key(1, 1, SongResourceKind::Track);
            let old_master = key(1, 2, SongResourceKind::Master);
            let mut plain = BusTemplate::new();
            plain.bus = BusId::new(1);
            for lease in [old_track, old_master] {
                if native {
                    graph
                        .stage_song_bus(lease, Box::new(plain), &cells, 48000., &caps)
                        .unwrap();
                } else {
                    graph
                        .stage_song_arena(lease, &plain, &cells, 48000., &caps)
                        .unwrap();
                }
                graph.activate_song_bus(lease).unwrap();
            }
            let old = RuntimeTrack {
                epoch: SnapshotEpoch(1),
                track: graph.song_index(old_track).unwrap(),
                master: graph.song_index(old_master).unwrap(),
                track_key: old_track,
                master_key: old_master,
            };
            graph.close_song_bus_for_return(old_track).unwrap();
            let returned = graph.song_take_template(old_track); // actual Native owner moved off callback
            assert!(graph.song_finish_cancel(old_track));
            assert!(old.owned_master(&graph));
            let new_key = key(2, 1, SongResourceKind::Track);
            let mut amplified = plain;
            amplified.push(EffectKind::Gain).unwrap();
            amplified.push_param(
                0,
                effects::param_ctl(EffectKind::Gain, "gain").unwrap(),
                Ctl::Const(6.),
            );
            if native {
                graph
                    .stage_song_bus(new_key, Box::new(amplified), &cells, 48000., &caps)
                    .unwrap();
            } else {
                graph
                    .stage_song_arena(new_key, &amplified, &cells, 48000., &caps)
                    .unwrap();
            }
            let index = graph.activate_song_bus(new_key).unwrap();
            assert_eq!(index, old.track);
            let endpoint = SongEpochEnd {
                epoch: old.epoch,
                arrangement: 0,
                deadline: 5,
                closed: true,
            };
            assert_eq!(old.renderable(&graph, 5, Some(&endpoint)), Ok(false));
            // Exact same eligibility used by Engine's deadline reset, not current index ownership.
            assert!(!old.owned_track(&graph));
            if old.owned_track(&graph) {
                graph
                    .reset_song_bus(old.track_key, &cells, 48000., &caps)
                    .unwrap();
            }
            let store = SampleStore::new(if native {
                StoreKind::NativeArc
            } else {
                StoreKind::Arena { bytes: 0 }
            });
            let fft = Fft::new(FFT_SIZE);
            let mut scratch = vec![0.; 4 * FFT_SIZE];
            let mut stats = FxStats::default();
            let mut ctx = FxCtx {
                sr: 48000.,
                store: &store,
                fft: &fft,
                caps: &caps,
                scratch: &mut scratch,
                analysis: &mut [],
                stats: &mut stats,
            };
            let mut dry = [0.; 16];
            graph.slots[index].l[..16].fill(1.);
            graph.slots[index].r[..16].fill(1.);
            graph.run_song(index, 16, &cells, &mut dry, &mut ctx);
            assert!(
                graph.frames(index, 16).0.iter().all(|v| *v > 1.9),
                "new actual gain preserved"
            );
            assert!(old.owned_master(&graph));
            drop(returned);
        }
    }
}
