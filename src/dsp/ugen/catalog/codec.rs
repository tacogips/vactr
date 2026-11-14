//! Versioned node byte encoding for native and browser graph installs.

use super::*;
use crate::dsp::ugen::frame_keyframe;

// ---- node byte encoding (the graph codec's node layer, arena.rs) ----------

/// A little-endian graph-encoding writer.
pub(crate) struct Out<'a>(pub(crate) &'a mut Vec<u8>);

impl Out<'_> {
    pub(crate) fn u8(&mut self, x: u8) {
        self.0.push(x);
    }
    pub(crate) fn u16(&mut self, x: u16) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    pub(crate) fn u32(&mut self, x: u32) {
        self.0.extend_from_slice(&x.to_le_bytes());
    }
    pub(crate) fn f32(&mut self, x: f32) {
        self.u32(x.to_bits());
    }
    pub(crate) fn ctl(&mut self, id: CtlId, c: Ctl) {
        self.u16(id.get());
        match c {
            Ctl::Const(v) => {
                self.u8(0);
                self.f32(v);
            }
            Ctl::Cell(cell) => {
                self.u8(1);
                self.u32(cell.get());
            }
        }
    }
    pub(crate) fn len16(&mut self, n: usize) -> Result<(), BuildError> {
        self.u16(u16::try_from(n).map_err(|_| BuildError::TooManyNodes)?);
        Ok(())
    }
}

/// The catalog index of an effect kind.
pub(crate) fn effect_index(k: EffectKind) -> u8 {
    EffectKind::ALL
        .iter()
        .position(|x| *x == k)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or(0)
}

/// Encodes one node spec.
pub(crate) fn put_spec(o: &mut Out<'_>, spec: &UGenSpec) -> Result<(), BuildError> {
    let (tag, payload): (u8, u32) = match spec {
        UGenSpec::SinOsc => (0, 0),
        UGenSpec::Saw => (1, 0),
        UGenSpec::Pulse => (2, 0),
        UGenSpec::Tri => (3, 0),
        UGenSpec::WhiteNoise => (4, 0),
        UGenSpec::HostInputL => (90, 0),
        UGenSpec::HostInputR => (91, 0),
        UGenSpec::Lpf => (5, 0),
        UGenSpec::Hpf => (6, 0),
        UGenSpec::Bpf => (7, 0),
        UGenSpec::Delay => (8, 0),
        UGenSpec::Comb => (9, 0),
        UGenSpec::EnvPerc => (10, 0),
        UGenSpec::EnvAdsr => (11, 0),
        UGenSpec::Line => (12, 0),
        UGenSpec::SamplePlay(b) => (13, b.get()),
        UGenSpec::Mul => (14, 0),
        UGenSpec::Add => (15, 0),
        UGenSpec::Const(v) => (16, v.to_bits()),
        UGenSpec::Param(c) => (17, u32::from(c.get())),
        UGenSpec::Vco { unison_max } => (18, u32::from(*unison_max)),
        UGenSpec::SubOsc => (19, 0),
        UGenSpec::Ladder => (20, 0),
        UGenSpec::Svf => (21, 0),
        UGenSpec::FmOp => (22, 0),
        UGenSpec::FmMod => (23, 0),
        UGenSpec::FmDrum => (31, 0),
        UGenSpec::FeedbackMetal => (88, 0),
        UGenSpec::DigitalDrumCore => (92, 0),
        UGenSpec::DigitalSnareCore => (93, 0),
        UGenSpec::DigitalMetalCore => (94, 0),
        UGenSpec::DigitalHatCore => (95, 0),
        UGenSpec::AnalogPercussion => (35, 0),
        UGenSpec::VaSource => (37, 0),
        UGenSpec::VaFilter => (38, 0),
        UGenSpec::PhasePair => (39, 0),
        UGenSpec::FmPair => (40, 0),
        UGenSpec::SixOpOriginal => (72, 0),
        UGenSpec::SpeechOriginal => (73, 0),
        UGenSpec::RingsPart => (74, 0),
        UGenSpec::StringChoir => (75, 0),
        UGenSpec::ElementsInternal => (76, 0),
        UGenSpec::TidalFunction => (77, 0),
        UGenSpec::TidalPoly => (78, 0),
        UGenSpec::PeakFunction => (79, 0),
        UGenSpec::StageSegment => (82, 0),
        UGenSpec::StageChain => (83, 0),
        UGenSpec::StageLinked { data } => {
            let data = data.as_ref().ok_or(BuildError::BadStageData)?;
            data.validate().map_err(|_| BuildError::BadStageData)?;
            o.u8(89);
            o.u8(super::super::stage_linked::WIRE_VERSION);
            o.u8(data.len);
            for row in data.rows.iter().take(usize::from(data.len)) {
                for &value in row {
                    o.f32(value);
                }
            }
            return Ok(());
        }
        UGenSpec::FrameLfo => (84, 0),
        UGenSpec::FrameKeyframe { data } => {
            let data = data.as_ref().ok_or(BuildError::BadFrameData)?;
            data.validate().map_err(|_| BuildError::BadFrameData)?;
            o.u8(85);
            o.u8(frame_keyframe::WIRE_VERSION);
            o.u8(data.len);
            for row in data.points.iter().take(usize::from(data.len)) {
                for &value in row {
                    o.f32(value);
                }
            }
            return Ok(());
        }
        UGenSpec::PeakPulse => (80, 0),
        UGenSpec::NumberStation => (81, 0),
        UGenSpec::SpectrumPair => (41, 0),
        UGenSpec::ClockNoisePair => (42, 0),
        UGenSpec::DualKick => (43, 0),
        UGenSpec::SnarePair => (44, 0),
        UGenSpec::HatPair => (45, 0),
        UGenSpec::SwarmPair => (46, 0),
        UGenSpec::ParticlePair => (47, 0),
        UGenSpec::ModalPair => (48, 0),
        UGenSpec::StringPair => (49, 0),
        UGenSpec::ChipPair => (50, 0),
        UGenSpec::AnalogPair => (51, 0),
        UGenSpec::GrainPair => (52, 0),
        UGenSpec::ShapePair => (53, 0),
        UGenSpec::StringMachinePair => (54, 0),
        UGenSpec::TerrainPair => (55, 0),
        UGenSpec::TableTerrainPair => (56, 0),
        UGenSpec::ChordPair => (57, 0),
        UGenSpec::BraidsFive => (58, 0),
        UGenSpec::BraidsSubSync => (59, 0),
        UGenSpec::BraidsTriple => (60, 0),
        UGenSpec::BraidsDigital => (61, 0),
        UGenSpec::BraidsFilter => (62, 0),
        UGenSpec::BraidsFormant => (63, 0),
        UGenSpec::BraidsFm => (64, 0),
        UGenSpec::BraidsPhysical => (65, 0),
        UGenSpec::BraidsStruck => (66, 0),
        UGenSpec::BraidsPercussion => (67, 0),
        UGenSpec::BraidsWaveBank => (68, 0),
        UGenSpec::BraidsWaveLine => (69, 0),
        UGenSpec::BraidsNoise => (70, 0),
        UGenSpec::BraidsCloud => (71, 0),
        UGenSpec::AuxOut => (36, 0),
        UGenSpec::Out3 => (86, 0),
        UGenSpec::Out4 => (87, 0),
        UGenSpec::FeedbackDrum => (32, 0),
        UGenSpec::NoiseDrum => (33, 0),
        UGenSpec::SineDrum => (34, 0),
        UGenSpec::PhaseDistortion => (24, 0),
        UGenSpec::Additive { partials_max } => (25, u32::from(*partials_max)),
        UGenSpec::Wavetable(t) => (26, t.get()),
        UGenSpec::Granular(GranSrc::Sample(b)) => (27, b.get()),
        UGenSpec::Granular(GranSrc::Table(t)) => (28, t.get()),
        UGenSpec::Granular(GranSrc::Bus) => (29, 0),
        UGenSpec::Effect(e) => {
            o.u8(30);
            o.u8(effect_index(e.kind));
            o.len16(e.params.len())?;
            for &(id, c) in e.params.iter() {
                o.ctl(id, c);
            }
            return Ok(());
        }
    };
    o.u8(tag);
    o.u32(payload);
    Ok(())
}

/// A bounds-checked graph-encoding reader.
pub(crate) struct In<'a> {
    pub(crate) b: &'a [u8],
    pub(crate) pos: usize,
}

impl In<'_> {
    pub(crate) fn take<const N: usize>(&mut self) -> Result<[u8; N], FaultCode> {
        let s = self
            .b
            .get(self.pos..self.pos + N)
            .ok_or(FaultCode::BadRecord)?;
        self.pos += N;
        let mut a = [0; N];
        a.copy_from_slice(s);
        Ok(a)
    }
    pub(crate) fn u8(&mut self) -> Result<u8, FaultCode> {
        Ok(self.take::<1>()?[0])
    }
    pub(crate) fn u16(&mut self) -> Result<u16, FaultCode> {
        Ok(u16::from_le_bytes(self.take()?))
    }
    pub(crate) fn u32(&mut self) -> Result<u32, FaultCode> {
        Ok(u32::from_le_bytes(self.take()?))
    }
    pub(crate) fn ctl(&mut self) -> Result<(CtlId, Ctl), FaultCode> {
        let id = CtlId::new(self.u16()?);
        let c = match self.u8()? {
            0 => Ctl::Const(f32::from_bits(self.u32()?)),
            1 => Ctl::Cell(CellId::new(self.u32()?)),
            _ => return Err(FaultCode::BadRecord),
        };
        Ok((id, c))
    }
}

/// Decodes one node into `raw`.
pub(crate) fn get_node(i: &mut In<'_>, raw: &mut RawGraph) -> Result<(), FaultCode> {
    let tag = i.u8()?;
    if tag == 30 {
        let kind = *EffectKind::ALL
            .get(usize::from(i.u8()?))
            .ok_or(FaultCode::BadRecord)?;
        let n = raw
            .push_node(Node::Effect { kind, fx: 0 })
            .map_err(|_| FaultCode::GraphTooLarge)?;
        for _ in 0..i.u16()? {
            let (id, c) = i.ctl()?;
            raw.push_node_param(n, id, c)
                .map_err(|_| FaultCode::GraphTooLarge)?;
        }
        return Ok(());
    }
    if tag == 85 {
        if i.u8()? != frame_keyframe::WIRE_VERSION {
            return Err(FaultCode::BadRecord);
        }
        let len = i.u8()?;
        if len == 0 || usize::from(len) > frame_keyframe::MAX_FRAMES {
            return Err(FaultCode::BadRecord);
        }
        let mut data = frame_keyframe::FrameData::EMPTY;
        data.len = len;
        for row in data.points.iter_mut().take(usize::from(len)) {
            for value in row {
                *value = f32::from_bits(i.u32()?);
            }
        }
        data.validate().map_err(|_| FaultCode::BadRecord)?;
        let slot = raw
            .push_frame_payload(data)
            .map_err(|_| FaultCode::GraphTooLarge)?;
        raw.push_node(Node::FrameKeyframe { slot })
            .map_err(|_| FaultCode::GraphTooLarge)?;
        return Ok(());
    }
    if tag == 89 {
        if i.u8()? != super::super::stage_linked::WIRE_VERSION {
            return Err(FaultCode::BadRecord);
        }
        let len = i.u8()?;
        if len == 0 || usize::from(len) > super::super::stage_linked::MAX_SEGMENTS {
            return Err(FaultCode::BadRecord);
        }
        let mut data = super::super::stage_linked::StageData::EMPTY;
        data.len = len;
        for row in data.rows.iter_mut().take(usize::from(len)) {
            for value in row {
                *value = f32::from_bits(i.u32()?);
            }
        }
        data.validate().map_err(|_| FaultCode::BadRecord)?;
        let slot = raw
            .push_stage_payload(data)
            .map_err(|_| FaultCode::GraphTooLarge)?;
        raw.push_node(Node::StageLinked { slot })
            .map_err(|_| FaultCode::GraphTooLarge)?;
        return Ok(());
    }
    let p = i.u32()?;
    let small = u8::try_from(p).unwrap_or(u8::MAX);
    let node = match tag {
        0 => Node::SinOsc,
        1 => Node::Saw,
        2 => Node::Pulse,
        3 => Node::Tri,
        4 => Node::WhiteNoise,
        90 => Node::HostInputL,
        91 => Node::HostInputR,
        5 => Node::Lpf,
        6 => Node::Hpf,
        7 => Node::Bpf,
        8 => Node::Delay,
        9 => Node::Comb,
        10 => Node::EnvPerc,
        11 => Node::EnvAdsr,
        12 => Node::Line,
        13 => Node::SamplePlay(BankRef::new(p)),
        14 => Node::Mul,
        15 => Node::Add,
        16 => Node::Const(f32::from_bits(p)),
        17 => Node::Param(CtlId::new(
            u16::try_from(p).map_err(|_| FaultCode::BadRecord)?,
        )),
        18 => Node::Vco { unison_max: small },
        19 => Node::SubOsc,
        20 => Node::Ladder,
        21 => Node::Svf,
        22 => Node::FmOp,
        23 => Node::FmMod,
        31 => Node::FmDrum,
        88 => Node::FeedbackMetal,
        92 => Node::DigitalDrumCore,
        93 => Node::DigitalSnareCore,
        94 => Node::DigitalMetalCore,
        95 => Node::DigitalHatCore,
        35 => Node::AnalogPercussion,
        37 => Node::VaSource,
        38 => Node::VaFilter,
        39 => Node::PhasePair,
        40 => Node::FmPair,
        72 => Node::SixOpOriginal,
        73 => Node::SpeechOriginal,
        74 => Node::RingsPart,
        75 => Node::StringChoir,
        76 => Node::ElementsInternal,
        77 => Node::TidalFunction,
        78 => Node::TidalPoly,
        79 => Node::PeakFunction,
        82 => Node::StageSegment,
        83 => Node::StageChain,
        84 => Node::FrameLfo,
        80 => Node::PeakPulse,
        81 => Node::NumberStation,
        41 => Node::SpectrumPair,
        42 => Node::ClockNoisePair,
        43 => Node::DualKick,
        44 => Node::SnarePair,
        45 => Node::HatPair,
        46 => Node::SwarmPair,
        47 => Node::ParticlePair,
        48 => Node::ModalPair,
        49 => Node::StringPair,
        50 => Node::ChipPair,
        51 => Node::AnalogPair,
        52 => Node::GrainPair,
        53 => Node::ShapePair,
        54 => Node::StringMachinePair,
        55 => Node::TerrainPair,
        56 => Node::TableTerrainPair,
        57 => Node::ChordPair,
        58 => Node::BraidsFive,
        59 => Node::BraidsSubSync,
        60 => Node::BraidsTriple,
        61 => Node::BraidsDigital,
        62 => Node::BraidsFilter,
        63 => Node::BraidsFormant,
        64 => Node::BraidsFm,
        65 => Node::BraidsPhysical,
        66 => Node::BraidsStruck,
        67 => Node::BraidsPercussion,
        68 => Node::BraidsWaveBank,
        69 => Node::BraidsWaveLine,
        70 => Node::BraidsNoise,
        71 => Node::BraidsCloud,
        36 => Node::AuxOut,
        86 => Node::Out3,
        87 => Node::Out4,
        32 => Node::FeedbackDrum,
        33 => Node::NoiseDrum,
        34 => Node::SineDrum,
        24 => Node::PhaseDistortion,
        25 => Node::Additive {
            partials_max: small,
        },
        26 => Node::Wavetable(TableRef::new(p)),
        27 => Node::Granular(GranSrc::Sample(BankRef::new(p))),
        28 => Node::Granular(GranSrc::Table(TableRef::new(p))),
        29 => Node::Granular(GranSrc::Bus),
        _ => return Err(FaultCode::BadRecord),
    };
    raw.push_node(node).map_err(|_| FaultCode::GraphTooLarge)?;
    Ok(())
}
