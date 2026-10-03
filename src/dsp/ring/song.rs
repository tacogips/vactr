//! Full-key staging transport; decoding borrows preallocated inbox storage.
use super::{NativeInstall, Record};
use crate::dsp::arena::SLICE_BYTES;
use crate::song::routing::{SongLeaseKey, SongResourceKind, SongResourceRef};
use crate::song::SnapshotEpoch;

pub(super) const GRAPH: u8 = 0x1c;
pub(super) const BEGIN: u8 = 0x1d;
pub(super) const SLICE: u8 = 0x1e;
/// An existing native allocation transferred with its complete song lease.
pub struct NativeSongInstall {
    pub lease: SongLeaseKey,
    pub payload: NativeInstall,
}
impl NativeSongInstall {
    /// Checks payload identity without modifying or consuming its allocation.
    #[must_use]
    pub fn valid(&self) -> bool {
        let (resource, generation, kind) = match &self.payload {
            NativeInstall::Inst { resource, gen, .. } => (
                *resource,
                *gen,
                self.lease.kind == SongResourceKind::Instrument,
            ),
            NativeInstall::Bus {
                resource,
                gen,
                master,
                ..
            } => (
                *resource,
                *gen,
                if *master {
                    self.lease.kind == SongResourceKind::Master
                } else {
                    matches!(
                        self.lease.kind,
                        SongResourceKind::PrivateFx | SongResourceKind::Track
                    )
                },
            ),
            NativeInstall::Sample { resource, gen, .. } => {
                (*resource, *gen, self.lease.kind == SongResourceKind::Sample)
            }
        };
        resource == self.lease.resource.id && generation == self.lease.resource.generation && kind
    }
}
fn key(lease: SongLeaseKey, out: &mut [u8]) {
    out[..8].copy_from_slice(&lease.epoch.0.to_le_bytes());
    out[8..12].copy_from_slice(&lease.resource.id.to_le_bytes());
    out[12..16].copy_from_slice(&lease.resource.generation.to_le_bytes());
    out[16] = lease.kind as u8;
}
fn word(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn get_key(b: &[u8]) -> Option<SongLeaseKey> {
    let kind = match *b.get(16)? {
        0 => SongResourceKind::Instrument,
        1 => SongResourceKind::PrivateFx,
        2 => SongResourceKind::Track,
        3 => SongResourceKind::Master,
        4 => SongResourceKind::Sample,
        7 => SongResourceKind::ControlCells,
        8 => SongResourceKind::AnalysisBank,
        _ => return None,
    };
    Some(SongLeaseKey {
        epoch: SnapshotEpoch(u64::from_le_bytes(b.get(..8)?.try_into().ok()?)),
        resource: SongResourceRef {
            id: word(b, 8)?,
            generation: word(b, 12)?,
        },
        kind,
    })
}
/// Encodes a graph upload into caller-owned storage; zero means invalid/full.
pub fn encode_song_graph_record(lease: SongLeaseKey, bytes: &[u8], out: &mut [u8]) -> usize {
    if !matches!(
        lease.kind,
        SongResourceKind::Instrument
            | SongResourceKind::PrivateFx
            | SongResourceKind::Track
            | SongResourceKind::Master
    ) || bytes.len() > SLICE_BYTES
    {
        return 0;
    }
    let Some(end) = 22usize.checked_add(bytes.len()) else {
        return 0;
    };
    let Some(dst) = out.get_mut(..end) else {
        return 0;
    };
    let Ok(len) = u32::try_from(bytes.len()) else {
        return 0;
    };
    dst[0] = GRAPH;
    key(lease, &mut dst[1..18]);
    dst[18..22].copy_from_slice(&len.to_le_bytes());
    dst[22..].copy_from_slice(bytes);
    end
}
/// Encodes the complete sample reservation identity.
pub fn encode_song_sample_begin(
    lease: SongLeaseKey,
    frames: u32,
    channels: u8,
    rate: u32,
    out: &mut [u8],
) -> usize {
    if lease.kind != SongResourceKind::Sample
        || !(1..=2).contains(&channels)
        || !(8000..=192000).contains(&rate)
        || frames.checked_mul(u32::from(channels)).is_none()
    {
        return 0;
    }
    let Some(dst) = out.get_mut(..27) else {
        return 0;
    };
    dst[0] = BEGIN;
    key(lease, &mut dst[1..18]);
    dst[18..22].copy_from_slice(&frames.to_le_bytes());
    dst[22] = channels;
    dst[23..27].copy_from_slice(&rate.to_le_bytes());
    27
}
/// Encodes one bounded sample slice; every chunk retains epoch and generation.
pub fn encode_song_slice(
    lease: SongLeaseKey,
    offset: u32,
    samples: &[f32],
    out: &mut [u8],
) -> usize {
    let Ok(count) = u32::try_from(samples.len()) else {
        return 0;
    };
    if lease.kind != SongResourceKind::Sample
        || samples.len() > SLICE_BYTES / 4
        || offset.checked_add(count).is_none()
        || samples.iter().any(|v| !v.is_finite())
    {
        return 0;
    }
    let Some(size) = samples.len().checked_mul(4).and_then(|n| n.checked_add(26)) else {
        return 0;
    };
    let Some(dst) = out.get_mut(..size) else {
        return 0;
    };
    dst[0] = SLICE;
    key(lease, &mut dst[1..18]);
    dst[18..22].copy_from_slice(&offset.to_le_bytes());
    dst[22..26].copy_from_slice(&count.to_le_bytes());
    for (v, b) in samples.iter().zip(dst[26..].chunks_exact_mut(4)) {
        b.copy_from_slice(&v.to_le_bytes());
    }
    size
}
pub(super) fn decode(b: &[u8]) -> Option<Record<'_>> {
    let lease = get_key(b.get(1..18)?)?;
    match b[0] {
        GRAPH
            if matches!(
                lease.kind,
                SongResourceKind::Instrument
                    | SongResourceKind::PrivateFx
                    | SongResourceKind::Track
                    | SongResourceKind::Master
            ) =>
        {
            let n = usize::try_from(word(b, 18)?).ok()?;
            if n > SLICE_BYTES || b.len() != 22usize.checked_add(n)? {
                return None;
            }
            Some(Record::SongGraph {
                lease,
                bytes: &b[22..],
            })
        }
        BEGIN if lease.kind == SongResourceKind::Sample && b.len() == 27 => {
            let frames = word(b, 18)?;
            let channels = b[22];
            let rate = word(b, 23)?;
            if !(1..=2).contains(&channels) || !(8000..=192000).contains(&rate) {
                return None;
            }
            frames.checked_mul(u32::from(channels))?;
            Some(Record::SongSampleBegin {
                lease,
                frames,
                channels,
                rate,
            })
        }
        SLICE if lease.kind == SongResourceKind::Sample => {
            let offset = word(b, 18)?;
            let count = word(b, 22)?;
            offset.checked_add(count)?;
            let n = usize::try_from(count).ok()?.checked_mul(4)?;
            if n > SLICE_BYTES || b.len() != 26usize.checked_add(n)? {
                return None;
            }
            if b[26..]
                .chunks_exact(4)
                .any(|w| !f32::from_le_bytes([w[0], w[1], w[2], w[3]]).is_finite())
            {
                return None;
            }
            Some(Record::SongSlice {
                lease,
                offset,
                data: &b[26..],
            })
        }
        _ => None,
    }
}

const _: () = assert!(27 <= super::INBOX_SLOT_BYTES - SLICE_BYTES);
const _: () = assert!(crate::song::routing::SONG_COMMAND_MAX_LEN >= 74);
