//! Fixed node-memory and topological build helpers.

use super::*;

/// `(fixed, clampable)` memory a node wants, in floats.
pub(super) fn mem_need(node: &Node, env: &BuildEnv) -> (usize, usize) {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let secs = |s: f32| (s * env.sr) as usize;
    match node {
        Node::Vco { unison_max } => (usize::from((*unison_max).max(1)) * 2, 0),
        Node::Additive { partials_max } => (usize::from((*partials_max).max(1)), 0),
        Node::Delay => (0, secs(0.5)),
        Node::FeedbackDrum => (1001, 0),
        Node::DigitalDrumCore | Node::DigitalSnareCore => (digital_drum::STATE_FLOATS, 0),
        Node::DigitalMetalCore | Node::DigitalHatCore => (digital_drum::metal::STATE_FLOATS, 0),
        Node::SpectrumPair => (48, 0),
        Node::DualKick => (dual_kick::STATE_FLOATS, 0),
        Node::SnarePair => (snare_pair::STATE_FLOATS, 0),
        Node::HatPair => (hat_pair::STATE_FLOATS, 0),
        Node::SwarmPair => (swarm_pair::STATE_FLOATS, 0),
        Node::ParticlePair => (particle_pair::STATE_FLOATS, 0),
        Node::ModalPair => (modal_pair::STATE_FLOATS, 0),
        Node::StringPair => (string_pair::STATE_FLOATS, 0),
        Node::ChipPair => (chip_pair::STATE_FLOATS, 0),
        Node::AnalogPair => (analog_pair::STATE_FLOATS, 0),
        Node::GrainPair => (grain_pair::STATE_FLOATS, 0),
        Node::ShapePair => (shape_pair::STATE_FLOATS, 0),
        Node::StringMachinePair => (string_machine_pair::STATE_FLOATS, 0),
        Node::TerrainPair => (terrain_pair::STATE_FLOATS, 0),
        Node::TableTerrainPair => (table_terrain_pair::STATE_FLOATS, 0),
        Node::ChordPair => (chord_pair::STATE_FLOATS, 0),
        Node::BraidsFive => (braids_five::STATE_FLOATS, 0),
        Node::BraidsSubSync => (braids_subsync::STATE_FLOATS, 0),
        Node::BraidsTriple => (braids_triple::STATE_FLOATS, 0),
        Node::BraidsDigital => (braids_digital::mem_len(env.sr), 0),
        Node::BraidsFilter => (braids_filter::STATE_FLOATS, 0),
        Node::BraidsFormant => (braids_formant::STATE_FLOATS, 0),
        Node::BraidsFm => (braids_fm::STATE_FLOATS, 0),
        Node::BraidsPhysical => (braids_physical::mem_len(env.sr), 0),
        Node::BraidsStruck => (braids_struck::STATE_FLOATS, 0),
        Node::BraidsPercussion => (braids_percussion::STATE_FLOATS, 0),
        Node::BraidsWaveBank => (braids_wave_bank::STATE_FLOATS, 0),
        Node::BraidsWaveLine => (braids_wave_line::STATE_FLOATS, 0),
        Node::BraidsNoise => (braids_noise::STATE_FLOATS, 0),
        Node::BraidsCloud => (braids_cloud::STATE_FLOATS, 0),
        Node::SixOpOriginal => (six_op_original::STATE_FLOATS, 0),
        Node::SpeechOriginal => (speech_original::STATE_FLOATS, 0),
        Node::RingsPart => (rings_part::mem_len(env.sr), 0),
        Node::StringChoir => (string_choir::mem_len(env.sr), 0),
        Node::ElementsInternal => (elements_internal::mem_len(env.sr), 0),
        Node::TidalFunction => (tidal_function::STATE_FLOATS, 0),
        Node::TidalPoly => (tidal_poly::STATE_FLOATS, 0),
        Node::PeakFunction => (peak_function::STATE_FLOATS, 0),
        Node::StageSegment => (stage_segment::mem_len(env.sr), 0),
        Node::StageChain => (stage_chain::STATE_FLOATS, 0),
        Node::StageLinked { .. } => (stage_linked::STATE_FLOATS, 0),
        Node::FrameLfo => (frame_lfo::STATE_FLOATS, 0),
        Node::PeakPulse => (peak_pulse::STATE_FLOATS, 0),
        Node::NumberStation => (number_station::STATE_FLOATS, 0),
        Node::Comb => (0, secs(0.1)),
        Node::Granular(_) => (0, crate::dsp::granular::ugen_mem_len(env.sr, &env.caps)),
        Node::Effect { kind, .. } => (0, effects::mem_len(*kind, env.sr, &env.caps)),
        _ => (0, 0),
    }
}

/// Kahn's algorithm over fixed arrays.
pub(super) fn topo_order(
    n: usize,
    edges: &[crate::dsp::graph::Edge],
) -> Result<[u16; NODE_CAP], BuildError> {
    let mut indeg = [0u16; NODE_CAP];
    for e in edges {
        let (from, to) = (usize::from(e.from), usize::from(e.to));
        if from >= n || to >= n {
            return Err(BuildError::BadEdge);
        }
        indeg[to] += 1;
    }
    let mut order = [0u16; NODE_CAP];
    let mut len = 0;
    for (i, d) in indeg.iter().enumerate().take(n) {
        if *d == 0 {
            order[len] = u16::try_from(i).map_err(|_| BuildError::TooManyNodes)?;
            len += 1;
        }
    }
    let mut head = 0;
    while head < len {
        let v = order[head];
        head += 1;
        for e in edges.iter().filter(|e| e.from == v) {
            let to = usize::from(e.to);
            indeg[to] -= 1;
            if indeg[to] == 0 {
                order[len] = e.to;
                len += 1;
            }
        }
    }
    if len < n {
        return Err(BuildError::Cycle);
    }
    Ok(order)
}
