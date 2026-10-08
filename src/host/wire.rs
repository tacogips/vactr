//! POD records that cross to the audio side, and their byte codec (design
//! 11.3, 11.4, 11.7, 12.8.5, 16.1).
//!
//! Every record is fixed-size, `Copy` and heap-free: the audio thread never
//! sees a language value. Natively the records travel through lock-free
//! rings as they are; in the browser `encode`/`decode` turn each one into a
//! fixed little-endian layout with a leading tag byte, and JS only moves the
//! bytes between the two wasm instances. Decoding untrusted bytes never
//! panics: a short buffer or an unknown tag is a `WireError`.

use std::fmt;

use crate::dsp::cells::CellId;
use crate::dsp::graph::InstId;
use crate::sched::slots::{CtlId, SlotId};
use crate::vm::fail::{FailCode, Failure};

/// The most resolved controls one event carries (design 12.8.7).
/// Changing this changes `AudioEvent::ENCODED_LEN`. Native peers and both
/// browser wasm instances must come from the same build; this byte layout is
/// not a versioned persistence or cross-release protocol.
pub const MAX_CTLS: usize = 32;

/// A control value: a constant, or a late-bound cell read at voice start
/// (control-rate parameters read it continuously) (design 11.4).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ctl {
    Const(f32),
    Cell(CellId),
}

/// One committed audio event (design 11.4).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct AudioEvent {
    /// Host seconds on the audio timebase.
    pub time: f64,
    pub slot: SlotId,
    pub gen: u32,
    pub inst: InstId,
    pub voice_hint: u32,
    /// The number of used entries of `ctl`.
    pub n_ctl: u8,
    pub ctl: [(CtlId, Ctl); MAX_CTLS],
}

impl AudioEvent {
    /// The encoded size, including the leading tag byte.
    pub const ENCODED_LEN: usize = 1 + EVENT_BODY_LEN;

    /// An event with no controls.
    #[must_use]
    pub const fn new(time: f64, slot: SlotId, gen: u32, inst: InstId) -> Self {
        Self {
            time,
            slot,
            gen,
            inst,
            voice_hint: 0,
            n_ctl: 0,
            ctl: [(CtlId::new(0), Ctl::Const(0.0)); MAX_CTLS],
        }
    }

    /// The used controls.
    #[must_use]
    pub fn controls(&self) -> &[(CtlId, Ctl)] {
        &self.ctl[..usize::from(self.n_ctl).min(MAX_CTLS)]
    }

    /// The cut group `sched::commit` packed into `voice_hint`
    /// (`orbit | cut << 8`, design-music.md "cut group":
    /// `s :break > cut 1 > d1`); 0 means no group. Read by the engine's
    /// cut-group choke (DDRUM-004B).
    #[must_use]
    pub fn cut_group(&self) -> u8 {
        (self.voice_hint >> 8) as u8
    }

    /// The orbit `sched::commit` packed into `voice_hint`, read alongside
    /// `cut_group` to scope choke to voices sharing both. Independent of
    /// any `orbit`-routed `CtlId` control on the event.
    #[must_use]
    pub fn hint_orbit(&self) -> u8 {
        self.voice_hint as u8
    }

    /// Appends a control.
    ///
    /// # Errors
    /// `too-many-controls` when the event already carries `MAX_CTLS`.
    pub fn push_ctl(&mut self, id: CtlId, ctl: Ctl) -> Result<(), Failure> {
        let n = usize::from(self.n_ctl);
        if n >= MAX_CTLS {
            return Err(Failure::new(
                FailCode::TooManyControls,
                format!("an event carries at most {MAX_CTLS} controls"),
            ));
        }
        self.ctl[n] = (id, ctl);
        self.n_ctl += 1;
        Ok(())
    }

    /// Writes the event (with its tag byte) to `out`; returns the bytes
    /// written, or 0 when `out` is shorter than `ENCODED_LEN`.
    #[must_use]
    pub fn encode(&self, out: &mut [u8]) -> usize {
        let mut w = Writer::new(out);
        w.u8(TAG_AUDIO_EVENT);
        put_event(&mut w, self);
        w.finish()
    }

    /// Reads an event written by `encode`; returns it and the bytes read.
    ///
    /// # Errors
    /// `WireError` for a short buffer, a wrong tag or an invalid field.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), WireError> {
        let mut r = Reader::new(bytes);
        let tag = r.u8()?;
        if tag != TAG_AUDIO_EVENT {
            return Err(WireError::BadTag(tag));
        }
        let ev = get_event(&mut r)?;
        Ok((ev, r.pos))
    }
}

/// What a `SlotControl` does to voices that started before its effective
/// time: rebind lets them ring, stop lets them end naturally, hush gates
/// them. Ordered by severity (design 11.3).
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Release {
    None,
    Natural,
    Panic,
}

/// The one revocation message every sink implements (design 11.3).
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SlotControl {
    pub slot: SlotId,
    pub new_gen: u32,
    pub effective_time: f64,
    pub release: Release,
}

impl SlotControl {
    /// The monotone merge of two immediate controls for the same slot
    /// (design 11.3): the larger generation, the earlier effective time and
    /// the more severe release. Merging is idempotent, so re-sending the
    /// merged entry until it is acknowledged changes nothing. `self.slot`
    /// is kept.
    #[must_use]
    pub fn merge(self, other: SlotControl) -> SlotControl {
        SlotControl {
            slot: self.slot,
            new_gen: self.new_gen.max(other.new_gen),
            effective_time: self.effective_time.min(other.effective_time),
            release: self.release.max(other.release),
        }
    }
}

/// A sink's acknowledgment of a `SlotControl`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SlotControlAck {
    pub slot: SlotId,
    pub gen: u32,
}

/// The identity of one live-input voice (design 11.7).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct VoiceTag {
    pub slot: SlotId,
    pub channel: u8,
    pub pitch: u8,
    pub seq: u32,
}

/// Whole-output stop behavior requested by the session.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputMode {
    Gentle = 0,
    Cut = 1,
}

/// Externally visible whole-output state.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputPhase {
    Running = 0,
    Draining = 1,
    Cutting = 2,
    Idle = 3,
}

/// Evaluator -> audio records on the priority control channel (design
/// 12.8.5). Resource ids name installed samples and graph templates (16.1).
///
/// Every record is inline and fixed-size (a channel slot holds the largest,
/// `LiveNoteOn`): boxing it would put a heap allocation on the audio path.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CtlMsg {
    Song(crate::song::routing::SongCommand),
    OutputStop {
        mode: OutputMode,
    },
    CellRamp {
        cell: CellId,
        epoch: u32,
        seq: u32,
        target: f32,
        frames: u32,
        release: bool,
    },
    SlotControl(SlotControl),
    CellInit {
        cell: CellId,
        epoch: u32,
        value: f32,
    },
    /// A batch header; in the byte stream `count` entries `(CellId, epoch,
    /// f32)` follow it (`encode_batch`). Browser tier only: the native tier
    /// shares `AtomicCells` and never sends it.
    CellBatch {
        seq: u32,
        count: u32,
    },
    CellRetire {
        cell: CellId,
        epoch: u32,
    },
    LiveNoteOn {
        tag: VoiceTag,
        ev: AudioEvent,
    },
    VoiceRelease {
        tag: VoiceTag,
    },
    GraphInstall {
        id: u32,
        gen: u32,
    },
    GraphRetire {
        id: u32,
    },
    /// A sample slice header; in the byte stream `len` raw `f32` LE samples
    /// follow it (BE-WASM writes and reads the payload, 16.1).
    SampleSlice {
        resource: u32,
        offset: u32,
        len: u32,
    },
    SampleRetire {
        resource: u32,
    },
}

/// Audio -> evaluator records (design 12.8.5).
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HostMsg {
    Song(crate::song::routing::SongHostAck),
    OutputState {
        phase: OutputPhase,
        frame: u64,
    },
    CellRampAck {
        cell: CellId,
        seq: u32,
    },
    SlotControlAck(SlotControlAck),
    CellInitAck {
        cell: CellId,
        epoch: u32,
    },
    CellBatchAck {
        seq: u32,
    },
    /// A retired cell incarnation has no audio-side user left (11.3: the
    /// `Retired` ack for cells, carrying the epoch so it is idempotent).
    CellRetired {
        cell: CellId,
        epoch: u32,
    },
    /// A resource (sample or graph template) has no audio-side user left.
    Retired {
        resource: u32,
    },
    SliceOk {
        resource: u32,
        offset: u32,
    },
    Installed {
        resource: u32,
        gen: u32,
    },
    Counters {
        late: u32,
        dropped: u32,
        stolen: u32,
        skipped: u32,
    },
    AnalysisCell {
        id: u32,
        value: f32,
    },
}

/// A malformed record.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WireError {
    /// The buffer ends inside the record.
    Truncated,
    /// The leading tag byte names no record.
    BadTag(u8),
    /// A field holds a value its type does not allow.
    BadValue,
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::Truncated => f.write_str("truncated wire record"),
            WireError::BadTag(t) => write!(f, "unknown wire record tag {t}"),
            WireError::BadValue => f.write_str("invalid wire record field"),
        }
    }
}

impl std::error::Error for WireError {}

const TAG_AUDIO_EVENT: u8 = 0x01;

const TAG_SLOT_CONTROL: u8 = 0x10;
const TAG_CELL_INIT: u8 = 0x11;
const TAG_CELL_BATCH: u8 = 0x12;
const TAG_CELL_RETIRE: u8 = 0x13;
const TAG_LIVE_NOTE_ON: u8 = 0x14;
const TAG_VOICE_RELEASE: u8 = 0x15;
const TAG_GRAPH_INSTALL: u8 = 0x16;
const TAG_GRAPH_RETIRE: u8 = 0x17;
const TAG_SAMPLE_SLICE: u8 = 0x18;
const TAG_SAMPLE_RETIRE: u8 = 0x19;
const TAG_OUTPUT_STOP: u8 = 0x1C;
const TAG_CELL_RAMP: u8 = 0x1D;

const TAG_SLOT_CONTROL_ACK: u8 = 0x40;
const TAG_CELL_INIT_ACK: u8 = 0x41;
const TAG_CELL_BATCH_ACK: u8 = 0x42;
const TAG_CELL_RETIRED: u8 = 0x43;
const TAG_RETIRED: u8 = 0x44;
const TAG_SLICE_OK: u8 = 0x45;
const TAG_INSTALLED: u8 = 0x46;
const TAG_COUNTERS: u8 = 0x47;
const TAG_ANALYSIS_CELL: u8 = 0x48;
const TAG_OUTPUT_STATE: u8 = 0x4B;
const TAG_CELL_RAMP_ACK: u8 = 0x4C;

const CTL_CONST: u8 = 0;
const CTL_CELL: u8 = 1;

/// One control entry: id, kind, 4-byte payload.
const CTL_LEN: usize = 2 + 1 + 4;
/// time, slot, gen, inst, voice_hint, n_ctl, every control entry.
const EVENT_BODY_LEN: usize = 8 + 4 + 4 + 4 + 4 + 1 + MAX_CTLS * CTL_LEN;
const TAG_LEN: usize = 4 + 1 + 1 + 4;
/// One batch entry: cell, epoch, value.
const BATCH_ENTRY_LEN: usize = 4 + 4 + 4;

impl CtlMsg {
    /// The largest encoded record (`LiveNoteOn`), including the tag byte.
    pub const MAX_LEN: usize =
        if crate::song::routing::SONG_COMMAND_MAX_LEN > 1 + TAG_LEN + EVENT_BODY_LEN {
            crate::song::routing::SONG_COMMAND_MAX_LEN
        } else {
            1 + TAG_LEN + EVENT_BODY_LEN
        };

    /// Writes the record to `out`; returns the bytes written, or 0 when `out`
    /// is too short. Batch entries (`encode_batch`) and slice samples are
    /// written separately.
    #[must_use]
    pub fn encode(&self, out: &mut [u8]) -> usize {
        if let Self::Song(command) = *self {
            return crate::host::caps::song_codec::encode_command(command, out);
        }
        let mut w = Writer::new(out);
        match *self {
            CtlMsg::Song(_) => unreachable!("song command encoded above"),
            CtlMsg::OutputStop { mode } => {
                w.u8(TAG_OUTPUT_STOP);
                w.u8(mode as u8);
            }
            CtlMsg::CellRamp {
                cell,
                epoch,
                seq,
                target,
                frames,
                release,
            } => {
                w.u8(TAG_CELL_RAMP);
                w.u32(cell.get());
                w.u32(epoch);
                w.u32(seq);
                w.f32(target);
                w.u32(frames);
                w.u8(u8::from(release));
            }
            CtlMsg::SlotControl(c) => {
                w.u8(TAG_SLOT_CONTROL);
                put_slot_control(&mut w, &c);
            }
            CtlMsg::CellInit { cell, epoch, value } => {
                w.u8(TAG_CELL_INIT);
                w.u32(cell.get());
                w.u32(epoch);
                w.f32(value);
            }
            CtlMsg::CellBatch { seq, count } => {
                w.u8(TAG_CELL_BATCH);
                w.u32(seq);
                w.u32(count);
            }
            CtlMsg::CellRetire { cell, epoch } => {
                w.u8(TAG_CELL_RETIRE);
                w.u32(cell.get());
                w.u32(epoch);
            }
            CtlMsg::LiveNoteOn { tag, ev } => {
                w.u8(TAG_LIVE_NOTE_ON);
                put_tag(&mut w, &tag);
                put_event(&mut w, &ev);
            }
            CtlMsg::VoiceRelease { tag } => {
                w.u8(TAG_VOICE_RELEASE);
                put_tag(&mut w, &tag);
            }
            CtlMsg::GraphInstall { id, gen } => {
                w.u8(TAG_GRAPH_INSTALL);
                w.u32(id);
                w.u32(gen);
            }
            CtlMsg::GraphRetire { id } => {
                w.u8(TAG_GRAPH_RETIRE);
                w.u32(id);
            }
            CtlMsg::SampleSlice {
                resource,
                offset,
                len,
            } => {
                w.u8(TAG_SAMPLE_SLICE);
                w.u32(resource);
                w.u32(offset);
                w.u32(len);
            }
            CtlMsg::SampleRetire { resource } => {
                w.u8(TAG_SAMPLE_RETIRE);
                w.u32(resource);
            }
        }
        w.finish()
    }

    /// Reads one record; returns it and the bytes read (the header only for
    /// `CellBatch` and `SampleSlice`).
    ///
    /// # Errors
    /// `WireError` for a short buffer, an unknown tag or an invalid field.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), WireError> {
        if bytes.first() == Some(&crate::song::routing::SONG_COMMAND_TAG) {
            return crate::host::caps::song_codec::decode_command(bytes)
                .map(|(c, n)| (Self::Song(c), n));
        }
        let mut r = Reader::new(bytes);
        let msg = match r.u8()? {
            TAG_OUTPUT_STOP => CtlMsg::OutputStop {
                mode: match r.u8()? {
                    0 => OutputMode::Gentle,
                    1 => OutputMode::Cut,
                    _ => return Err(WireError::BadValue),
                },
            },
            TAG_CELL_RAMP => CtlMsg::CellRamp {
                cell: CellId::new(r.u32()?),
                epoch: r.u32()?,
                seq: r.u32()?,
                target: r.f32()?,
                frames: r.u32()?,
                release: match r.u8()? {
                    0 => false,
                    1 => true,
                    _ => return Err(WireError::BadValue),
                },
            },
            TAG_SLOT_CONTROL => CtlMsg::SlotControl(get_slot_control(&mut r)?),
            TAG_CELL_INIT => CtlMsg::CellInit {
                cell: CellId::new(r.u32()?),
                epoch: r.u32()?,
                value: r.f32()?,
            },
            TAG_CELL_BATCH => CtlMsg::CellBatch {
                seq: r.u32()?,
                count: r.u32()?,
            },
            TAG_CELL_RETIRE => CtlMsg::CellRetire {
                cell: CellId::new(r.u32()?),
                epoch: r.u32()?,
            },
            TAG_LIVE_NOTE_ON => CtlMsg::LiveNoteOn {
                tag: get_tag(&mut r)?,
                ev: get_event(&mut r)?,
            },
            TAG_VOICE_RELEASE => CtlMsg::VoiceRelease {
                tag: get_tag(&mut r)?,
            },
            TAG_GRAPH_INSTALL => CtlMsg::GraphInstall {
                id: r.u32()?,
                gen: r.u32()?,
            },
            TAG_GRAPH_RETIRE => CtlMsg::GraphRetire { id: r.u32()? },
            TAG_SAMPLE_SLICE => CtlMsg::SampleSlice {
                resource: r.u32()?,
                offset: r.u32()?,
                len: r.u32()?,
            },
            TAG_SAMPLE_RETIRE => CtlMsg::SampleRetire { resource: r.u32()? },
            tag => return Err(WireError::BadTag(tag)),
        };
        Ok((msg, r.pos))
    }
}

impl HostMsg {
    /// The largest encoded record (`Counters`), including the tag byte.
    pub const MAX_LEN: usize = crate::song::routing::SONG_ACK_MAX_LEN;

    /// Writes the record to `out`; returns the bytes written, or 0 when `out`
    /// is too short. Every field is one little-endian 32-bit word.
    #[must_use]
    pub fn encode(&self, out: &mut [u8]) -> usize {
        if let Self::Song(ack) = *self {
            return crate::host::caps::song_codec::encode_ack(ack, out);
        }
        let (tag, words, n) = self.words();
        let mut w = Writer::new(out);
        w.u8(tag);
        for &x in &words[..n] {
            w.u32(x);
        }
        w.finish()
    }

    /// Reads one record; returns it and the bytes read.
    ///
    /// # Errors
    /// `WireError` for a short buffer or an unknown tag.
    pub fn decode(bytes: &[u8]) -> Result<(Self, usize), WireError> {
        if bytes.first() == Some(&crate::song::routing::SONG_ACK_TAG) {
            return crate::host::caps::song_codec::decode_ack(bytes)
                .map(|(a, n)| (Self::Song(a), n));
        }
        let mut r = Reader::new(bytes);
        let tag = r.u8()?;
        let n = host_arity(tag).ok_or(WireError::BadTag(tag))?;
        let mut words = [0u32; 4];
        for x in &mut words[..n] {
            *x = r.u32()?;
        }
        let msg = match HostMsg::from_words(tag, words) {
            Some(msg) => msg,
            None if tag == TAG_OUTPUT_STATE => return Err(WireError::BadValue),
            None => return Err(WireError::BadTag(tag)),
        };
        Ok((msg, r.pos))
    }

    /// The tag, the field words and how many of them are used.
    #[rustfmt::skip]
    fn words(&self) -> (u8, [u32; 4], usize) {
        match *self {
            HostMsg::Song(_) => unreachable!("song ack encoded above"),
            HostMsg::OutputState { phase, frame } => (
                TAG_OUTPUT_STATE,
                [
                    phase as u32,
                    u32::try_from(frame & u64::from(u32::MAX)).unwrap_or_default(),
                    u32::try_from(frame >> 32).unwrap_or_default(),
                    0,
                ],
                3,
            ),
            HostMsg::CellRampAck { cell, seq } => {
                (TAG_CELL_RAMP_ACK, [cell.get(), seq, 0, 0], 2)
            }
            HostMsg::SlotControlAck(a) =>
                (TAG_SLOT_CONTROL_ACK, [a.slot.get(), a.gen, 0, 0], 2),
            HostMsg::CellInitAck { cell, epoch } =>
                (TAG_CELL_INIT_ACK, [cell.get(), epoch, 0, 0], 2),
            HostMsg::CellBatchAck { seq } => (TAG_CELL_BATCH_ACK, [seq, 0, 0, 0], 1),
            HostMsg::CellRetired { cell, epoch } =>
                (TAG_CELL_RETIRED, [cell.get(), epoch, 0, 0], 2),
            HostMsg::Retired { resource } => (TAG_RETIRED, [resource, 0, 0, 0], 1),
            HostMsg::SliceOk { resource, offset } => (TAG_SLICE_OK, [resource, offset, 0, 0], 2),
            HostMsg::Installed { resource, gen } => (TAG_INSTALLED, [resource, gen, 0, 0], 2),
            HostMsg::Counters { late, dropped, stolen, skipped } =>
                (TAG_COUNTERS, [late, dropped, stolen, skipped], 4),
            HostMsg::AnalysisCell { id, value } =>
                (TAG_ANALYSIS_CELL, [id, value.to_bits(), 0, 0], 2),
        }
    }

    /// The record of a tag and its field words (the inverse of `words`).
    #[rustfmt::skip]
    fn from_words(tag: u8, [a, b, c, d]: [u32; 4]) -> Option<Self> {
        Some(match tag {
            TAG_OUTPUT_STATE => HostMsg::OutputState {
                phase: match a {
                    0 => OutputPhase::Running,
                    1 => OutputPhase::Draining,
                    2 => OutputPhase::Cutting,
                    3 => OutputPhase::Idle,
                    _ => return None,
                },
                frame: u64::from(b) | (u64::from(c) << 32),
            },
            TAG_CELL_RAMP_ACK => HostMsg::CellRampAck {
                cell: CellId::new(a),
                seq: b,
            },
            TAG_SLOT_CONTROL_ACK =>
                HostMsg::SlotControlAck(SlotControlAck { slot: SlotId::new(a), gen: b }),
            TAG_CELL_INIT_ACK => HostMsg::CellInitAck { cell: CellId::new(a), epoch: b },
            TAG_CELL_BATCH_ACK => HostMsg::CellBatchAck { seq: a },
            TAG_CELL_RETIRED => HostMsg::CellRetired { cell: CellId::new(a), epoch: b },
            TAG_RETIRED => HostMsg::Retired { resource: a },
            TAG_SLICE_OK => HostMsg::SliceOk { resource: a, offset: b },
            TAG_INSTALLED => HostMsg::Installed { resource: a, gen: b },
            TAG_COUNTERS => HostMsg::Counters { late: a, dropped: b, stolen: c, skipped: d },
            TAG_ANALYSIS_CELL => HostMsg::AnalysisCell { id: a, value: f32::from_bits(b) },
            _ => return None,
        })
    }
}

/// The number of field words of a `HostMsg` tag.
const fn host_arity(tag: u8) -> Option<usize> {
    match tag {
        TAG_CELL_BATCH_ACK | TAG_RETIRED => Some(1),
        TAG_SLOT_CONTROL_ACK | TAG_CELL_INIT_ACK | TAG_CELL_RETIRED | TAG_SLICE_OK
        | TAG_INSTALLED | TAG_ANALYSIS_CELL => Some(2),
        TAG_OUTPUT_STATE => Some(3),
        TAG_CELL_RAMP_ACK => Some(2),
        TAG_COUNTERS => Some(4),
        _ => None,
    }
}

/// The encoded size of a `CellBatch` header plus `entries` entries.
#[must_use]
pub const fn batch_len(entries: usize) -> usize {
    1 + 8 + entries * BATCH_ENTRY_LEN
}

/// Writes a `CellBatch { seq, count }` header followed by its entries;
/// returns the bytes written, or 0 when `out` is too short (or there are
/// more than `u32::MAX` entries).
#[must_use]
pub fn encode_batch(seq: u32, entries: &[(CellId, u32, f32)], out: &mut [u8]) -> usize {
    let Ok(count) = u32::try_from(entries.len()) else {
        return 0;
    };
    let mut w = Writer::new(out);
    w.u8(TAG_CELL_BATCH);
    w.u32(seq);
    w.u32(count);
    for &(cell, epoch, value) in entries {
        w.u32(cell.get());
        w.u32(epoch);
        w.f32(value);
    }
    w.finish()
}

/// A decoded batch whose entries are read in place (no allocation).
#[derive(Clone, Copy, Debug)]
pub struct BatchView<'a> {
    pub seq: u32,
    entries: &'a [u8],
}

impl<'a> BatchView<'a> {
    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len() / BATCH_ENTRY_LEN
    }

    /// True when the batch has no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The `(cell, epoch, value)` entries, in order.
    pub fn iter(&self) -> impl Iterator<Item = (CellId, u32, f32)> + 'a {
        self.entries.chunks_exact(BATCH_ENTRY_LEN).map(|e| {
            (
                CellId::new(le_u32(&e[0..4])),
                le_u32(&e[4..8]),
                f32::from_bits(le_u32(&e[8..12])),
            )
        })
    }
}

/// Reads a batch written by `encode_batch`; returns it and the bytes read.
///
/// # Errors
/// `WireError` for a short buffer or a wrong tag.
pub fn decode_batch(bytes: &[u8]) -> Result<(BatchView<'_>, usize), WireError> {
    let (header, head_len) = CtlMsg::decode(bytes)?;
    let CtlMsg::CellBatch { seq, count } = header else {
        return Err(WireError::BadTag(bytes[0]));
    };
    let body = usize::try_from(count)
        .ok()
        .and_then(|n| n.checked_mul(BATCH_ENTRY_LEN))
        .ok_or(WireError::Truncated)?;
    let end = head_len.checked_add(body).ok_or(WireError::Truncated)?;
    let entries = bytes.get(head_len..end).ok_or(WireError::Truncated)?;
    Ok((BatchView { seq, entries }, end))
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn put_event(w: &mut Writer<'_>, ev: &AudioEvent) {
    w.f64(ev.time);
    w.u32(ev.slot.get());
    w.u32(ev.gen);
    w.u32(ev.inst.get());
    w.u32(ev.voice_hint);
    w.u8(ev.n_ctl);
    for &(id, ctl) in &ev.ctl {
        w.u16(id.get());
        match ctl {
            Ctl::Const(x) => {
                w.u8(CTL_CONST);
                w.f32(x);
            }
            Ctl::Cell(c) => {
                w.u8(CTL_CELL);
                w.u32(c.get());
            }
        }
    }
}

fn get_event(r: &mut Reader<'_>) -> Result<AudioEvent, WireError> {
    let mut ev = AudioEvent::new(
        r.f64()?,
        SlotId::new(r.u32()?),
        r.u32()?,
        InstId::new(r.u32()?),
    );
    ev.voice_hint = r.u32()?;
    ev.n_ctl = r.u8()?;
    if usize::from(ev.n_ctl) > MAX_CTLS {
        return Err(WireError::BadValue);
    }
    for entry in &mut ev.ctl {
        let id = CtlId::new(r.u16()?);
        let ctl = match r.u8()? {
            CTL_CONST => Ctl::Const(r.f32()?),
            CTL_CELL => Ctl::Cell(CellId::new(r.u32()?)),
            _ => return Err(WireError::BadValue),
        };
        *entry = (id, ctl);
    }
    Ok(ev)
}

fn put_slot_control(w: &mut Writer<'_>, c: &SlotControl) {
    w.u32(c.slot.get());
    w.u32(c.new_gen);
    w.f64(c.effective_time);
    w.u8(c.release as u8);
}

fn get_slot_control(r: &mut Reader<'_>) -> Result<SlotControl, WireError> {
    Ok(SlotControl {
        slot: SlotId::new(r.u32()?),
        new_gen: r.u32()?,
        effective_time: r.f64()?,
        release: match r.u8()? {
            0 => Release::None,
            1 => Release::Natural,
            2 => Release::Panic,
            _ => return Err(WireError::BadValue),
        },
    })
}

fn put_tag(w: &mut Writer<'_>, t: &VoiceTag) {
    w.u32(t.slot.get());
    w.u8(t.channel);
    w.u8(t.pitch);
    w.u32(t.seq);
}

fn get_tag(r: &mut Reader<'_>) -> Result<VoiceTag, WireError> {
    Ok(VoiceTag {
        slot: SlotId::new(r.u32()?),
        channel: r.u8()?,
        pitch: r.u8()?,
        seq: r.u32()?,
    })
}

/// A bounds-checked little-endian writer. Once a write does not fit, every
/// later write is skipped and `finish` returns 0.
struct Writer<'a> {
    out: &'a mut [u8],
    pos: usize,
    overflow: bool,
}

impl<'a> Writer<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Self {
            out,
            pos: 0,
            overflow: false,
        }
    }

    fn bytes<const N: usize>(&mut self, b: [u8; N]) {
        if self.overflow {
            return;
        }
        match self.out.get_mut(self.pos..self.pos + N) {
            Some(dst) => {
                dst.copy_from_slice(&b);
                self.pos += N;
            }
            None => self.overflow = true,
        }
    }

    fn u8(&mut self, x: u8) {
        self.bytes([x]);
    }

    fn u16(&mut self, x: u16) {
        self.bytes(x.to_le_bytes());
    }

    fn u32(&mut self, x: u32) {
        self.bytes(x.to_le_bytes());
    }

    fn f32(&mut self, x: f32) {
        self.bytes(x.to_bits().to_le_bytes());
    }

    fn f64(&mut self, x: f64) {
        self.bytes(x.to_bits().to_le_bytes());
    }

    fn finish(self) -> usize {
        if self.overflow {
            0
        } else {
            self.pos
        }
    }
}

/// A bounds-checked little-endian reader.
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        let end = self.pos.checked_add(N).ok_or(WireError::Truncated)?;
        let src = self.bytes.get(self.pos..end).ok_or(WireError::Truncated)?;
        let mut b = [0; N];
        b.copy_from_slice(src);
        self.pos = end;
        Ok(b)
    }

    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.bytes::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, WireError> {
        Ok(u16::from_le_bytes(self.bytes()?))
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }

    fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_bits(self.u32()?))
    }

    fn f64(&mut self) -> Result<f64, WireError> {
        Ok(f64::from_bits(u64::from_le_bytes(self.bytes()?)))
    }
}
