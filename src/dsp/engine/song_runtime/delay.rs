//! Private branch delay control and immutable physical inspection.
use super::*;
use crate::dsp::bus::SongPrivateDelayLayout;
impl Engine {
    /// Inspects a full-key private delay without exposing mutable memory.
    /// # Errors
    /// Stale/foreign owner or invalid physical geometry.
    pub fn song_private_delay_layout(
        &self,
        owner: SongLeaseKey,
    ) -> Result<SongPrivateDelayLayout, SongRejectCode> {
        let index = self
            .buses
            .song_index(owner)
            .ok_or(SongRejectCode::StaleEpoch)?;
        self.buses.slots[index].song_private_layout(
            owner,
            u32::try_from(index).map_err(|_| SongRejectCode::Capacity)?,
        )
    }
    pub(in crate::dsp::engine) fn validate_private_delay_controls<C: CellRead + ?Sized>(
        audio: &crate::host::wire::AudioEvent,
        cells: &C,
    ) -> Result<(Option<f32>, Option<f32>), SongRejectCode> {
        let time = event_ctl(audio, DELAYTIME).map(|c| resolve(c, cells));
        let feedback = event_ctl(audio, DELAYFEEDBACK).map(|c| resolve(c, cells));
        let send = event_ctl(audio, CtlId::new(38)).map(|c| resolve(c, cells));
        if [time, feedback, send]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite())
        {
            return Err(SongRejectCode::Malformed);
        }
        Ok((time, feedback))
    }
}
