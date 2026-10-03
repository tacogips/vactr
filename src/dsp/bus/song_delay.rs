//! Branch-private delay over strict, constructor-owned physical memory.
use super::*;
use crate::dsp::engine::SongFrameRegion;
use crate::song::routing::SongResourceKind;

/// Actual full-key regions in one physical private bus slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongPrivateDelayLayout {
    pub owner: SongLeaseKey,
    pub room: SongFrameRegion,
    pub chain_frames: u64,
    pub left: SongFrameRegion,
    pub right: SongFrameRegion,
    pub storage_frames: u64,
    pub send_frames_per_channel: u32,
}
#[derive(Debug)]
pub(crate) struct SongPrivateDelay {
    pub(crate) owner: Option<SongLeaseKey>,
    pub(crate) left: DelayLine,
    pub(crate) right: DelayLine,
    pub(crate) time: f32,
    pub(crate) feedback: f32,
    pub(crate) send_l: Box<[f32]>,
    pub(crate) send_r: Box<[f32]>,
}
impl SongPrivateDelay {
    pub(crate) fn new(max_block: usize) -> Self {
        // Same infallible allocation boundary as BusSlot; checked total scratch sizing.
        assert!(max_block
            .checked_mul(2)
            .and_then(|n| n.checked_mul(std::mem::size_of::<f32>()))
            .is_some());
        Self {
            owner: None,
            left: DelayLine::default(),
            right: DelayLine::default(),
            time: 0.25,
            feedback: 0.5,
            send_l: vec![0.; max_block].into_boxed_slice(),
            send_r: vec![0.; max_block].into_boxed_slice(),
        }
    }
    pub(crate) fn reset(
        &mut self,
        owner: SongLeaseKey,
        mem: &mut [f32],
    ) -> Result<(), SongRejectCode> {
        for line in [self.left, self.right] {
            let end = (line.off as usize)
                .checked_add(line.len as usize)
                .ok_or(SongRejectCode::Capacity)?;
            if line.len == 0 || end > mem.len() {
                return Err(SongRejectCode::Capacity);
            }
        }
        for line in [self.left, self.right] {
            mem[line.off as usize..line.off as usize + line.len as usize].fill(0.);
        }
        self.owner = Some(owner);
        self.left.pos = 0;
        self.right.pos = 0;
        self.send_l.fill(0.);
        self.send_r.fill(0.);
        Ok(())
    }
}
impl BusSlot {
    pub(super) fn private_need(
        template: &BusTemplate,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Result<(usize, usize, usize), SongRejectCode> {
        if !sr.is_finite()
            || !(8000.0..=192000.0).contains(&sr)
            || sr.fract() != 0.
            || f64::from(sr) > f64::from(u32::MAX)
        {
            return Err(SongRejectCode::Capacity);
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rate = sr as u32;
        if f64::from(rate) != f64::from(sr) {
            return Err(SongRejectCode::Capacity);
        }
        let delay = usize::try_from(rate)
            .map_err(|_| SongRejectCode::Capacity)?
            .checked_mul(4)
            .and_then(|n| n.checked_add(4))
            .ok_or(SongRejectCode::Capacity)?;
        let room = effects::mem_len(EffectKind::Room, sr, caps);
        let chain = template
            .kinds
            .iter()
            .take(template.n)
            .try_fold(0usize, |n, kind| {
                n.checked_add(effects::mem_len(*kind, sr, caps))
                    .ok_or(SongRejectCode::Capacity)
            })?;
        let total = room
            .checked_add(chain)
            .and_then(|n| delay.checked_mul(2).and_then(|d| n.checked_add(d)))
            .ok_or(SongRejectCode::Capacity)?;
        u32::try_from(total).map_err(|_| SongRejectCode::Capacity)?;
        Ok((room, chain, delay))
    }
    pub(super) fn configure_song_private<C: CellRead + ?Sized>(
        &mut self,
        owner: SongLeaseKey,
        template: &BusTemplate,
        cells: &C,
        sr: f32,
        caps: &CapabilitySet,
    ) -> Result<(), SongRejectCode> {
        let (room, chain, delay) = Self::private_need(template, sr, caps)?;
        let need = room
            .checked_add(chain)
            .and_then(|n| n.checked_add(delay.checked_mul(2)?))
            .ok_or(SongRejectCode::Capacity)?;
        if owner.kind != SongResourceKind::PrivateFx
            || need > self.mem.len()
            || template.n > MAX_CHAIN
        {
            return Err(SongRejectCode::Capacity);
        }
        let mut cursor = room;
        self.room_region = (0, room);
        self.room.configure(
            EffectKind::Room,
            &[(
                effects::param_ctl(EffectKind::Room, "mix").unwrap_or(CtlId::new(0)),
                Ctl::Const(0.),
            )],
            cells,
            &mut self.mem[..room],
            sr,
            caps,
        );
        self.n = template.n;
        for k in 0..template.n {
            let len = effects::mem_len(template.kinds[k], sr, caps);
            self.regions[k] = (cursor, len);
            self.units[k].configure(
                template.kinds[k],
                &template.params[k][..usize::from(template.n_params[k])],
                cells,
                &mut self.mem[cursor..cursor + len],
                sr,
                caps,
            );
            cursor += len;
        }
        self.song_delay.left = DelayLine::carve(&mut cursor, self.mem.len(), delay);
        self.song_delay.right = DelayLine::carve(&mut cursor, self.mem.len(), delay);
        // All offsets/lengths were proved representable and contained before configure.
        if self.song_delay.owner != Some(owner) {
            self.song_delay.time = 0.25;
            self.song_delay.feedback = 0.5;
        }
        self.song_delay.reset(owner, &mut self.mem)
    }
    pub(crate) fn song_private_layout(
        &self,
        owner: SongLeaseKey,
        slot_index: u32,
    ) -> Result<SongPrivateDelayLayout, SongRejectCode> {
        if self.song_key != Some(owner)
            || self.song_delay.owner != Some(owner)
            || owner.kind != SongResourceKind::PrivateFx
        {
            return Err(SongRejectCode::StaleEpoch);
        }
        let region = |off: usize, len: usize| -> Result<SongFrameRegion, SongRejectCode> {
            if off.checked_add(len).is_none_or(|end| end > self.mem.len()) {
                return Err(SongRejectCode::Capacity);
            }
            Ok(SongFrameRegion {
                slot: slot_index,
                offset: u64::try_from(off).map_err(|_| SongRejectCode::Capacity)?,
                frames: u64::try_from(len).map_err(|_| SongRejectCode::Capacity)?,
            })
        };
        let chain = self.regions[..self.n]
            .iter()
            .try_fold(0u64, |n, (_, len)| {
                n.checked_add(u64::try_from(*len).map_err(|_| SongRejectCode::Capacity)?)
                    .ok_or(SongRejectCode::Capacity)
            })?;
        Ok(SongPrivateDelayLayout {
            owner,
            room: region(self.room_region.0, self.room_region.1)?,
            chain_frames: chain,
            left: region(
                self.song_delay.left.off as usize,
                self.song_delay.left.len as usize,
            )?,
            right: region(
                self.song_delay.right.off as usize,
                self.song_delay.right.len as usize,
            )?,
            storage_frames: u64::try_from(self.mem.len()).map_err(|_| SongRejectCode::Capacity)?,
            send_frames_per_channel: u32::try_from(self.song_delay.send_l.len())
                .map_err(|_| SongRejectCode::Capacity)?,
        })
    }
    pub(crate) fn set_song_delay(
        &mut self,
        owner: SongLeaseKey,
        time: Option<f32>,
        feedback: Option<f32>,
    ) -> Result<(), SongRejectCode> {
        if self.song_key != Some(owner) || self.song_delay.owner != Some(owner) {
            return Err(SongRejectCode::StaleEpoch);
        }
        if time.is_some_and(|v| !v.is_finite()) || feedback.is_some_and(|v| !v.is_finite()) {
            return Err(SongRejectCode::Malformed);
        }
        if let Some(v) = time {
            self.song_delay.time = v.clamp(0., 4.);
        }
        if let Some(v) = feedback {
            self.song_delay.feedback = v.clamp(0., 0.95);
        }
        Ok(())
    }
    pub(crate) fn run_song_delay(
        &mut self,
        owner: SongLeaseKey,
        first: usize,
        frames: usize,
        sr: f32,
    ) -> Result<(), SongRejectCode> {
        if self.song_key != Some(owner) || self.song_delay.owner != Some(owner) {
            return Err(SongRejectCode::StaleEpoch);
        }
        let end = first.checked_add(frames).ok_or(SongRejectCode::Malformed)?;
        if end > self.l.len() || end > self.song_delay.send_l.len() {
            return Err(SongRejectCode::Malformed);
        }
        let d = self.song_delay.time * sr;
        for k in first..end {
            let yl = self.song_delay.left.read(&self.mem, d);
            let yr = self.song_delay.right.read(&self.mem, d);
            self.song_delay.left.write(
                &mut self.mem,
                self.song_delay.send_l[k] + self.song_delay.feedback * yl,
            );
            self.song_delay.right.write(
                &mut self.mem,
                self.song_delay.send_r[k] + self.song_delay.feedback * yr,
            );
            self.l[k] += yl;
            self.r[k] += yr;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_kernel_max_time_reset_and_foreign_owner_are_exact() {
        let caps = CapabilitySet::browser();
        let cells = crate::dsp::cells::AtomicCells::new(8);
        let owner = SongLeaseKey {
            epoch: crate::song::SnapshotEpoch(91),
            resource: crate::song::routing::SongResourceRef {
                id: 8,
                generation: 3,
            },
            kind: SongResourceKind::PrivateFx,
        };
        let template = BusTemplate::new();
        let (r, f, d) = BusSlot::private_need(&template, 8000., &caps).unwrap();
        let mut slot = BusSlot::new(1, r + f + 2 * d);
        slot.configure_song_private(owner, &template, &cells, 8000., &caps)
            .unwrap();
        slot.song_key = Some(owner);
        slot.set_song_delay(owner, Some(4.), Some(0.5)).unwrap();
        for frame in 0..64002 {
            slot.l[0] = 0.;
            slot.r[0] = 0.;
            slot.song_delay.send_l[0] = if frame == 0 { 1. } else { 0. };
            slot.song_delay.send_r[0] = if frame == 0 { -0.5 } else { 0. };
            slot.run_song_delay(owner, 0, 1, 8000.).unwrap();
            if frame == 32000 {
                assert_eq!(slot.l[0], 1.);
                assert_eq!(slot.r[0], -0.5);
            } else if frame == 64000 {
                assert_eq!(slot.l[0], 0.5);
                assert_eq!(slot.r[0], -0.25);
            } else {
                assert_eq!(slot.l[0], 0.);
                assert_eq!(slot.r[0], 0.);
            }
        }
        let mut foreign = owner;
        foreign.resource.generation += 1;
        let history = slot.mem.to_vec();
        assert_eq!(
            slot.set_song_delay(foreign, Some(1.), Some(0.)),
            Err(SongRejectCode::StaleEpoch)
        );
        assert_eq!(slot.mem.as_ref(), history);
        slot.configure_song_private(owner, &template, &cells, 8000., &caps)
            .unwrap();
        assert_eq!((slot.song_delay.time, slot.song_delay.feedback), (4., 0.5));
        assert!(slot.mem.iter().all(|v| *v == 0.));
        assert!(slot
            .song_delay
            .send_l
            .iter()
            .chain(slot.song_delay.send_r.iter())
            .all(|v| *v == 0.));
        assert_eq!(
            (slot.song_delay.left.pos, slot.song_delay.right.pos),
            (0, 0)
        );
        slot.configure_song_private(foreign, &template, &cells, 8000., &caps)
            .unwrap();
        assert_eq!(
            (slot.song_delay.time, slot.song_delay.feedback),
            (0.25, 0.5)
        );
        for sr in [7999., 192001., 48000.5, f32::INFINITY, f32::NAN] {
            assert_eq!(
                BusSlot::private_need(&template, sr, &caps),
                Err(SongRejectCode::Capacity)
            );
        }
    }
}
