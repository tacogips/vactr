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
        Node::BassCore => (bass_voice::STATE_FLOATS, 0),
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
        Node::VactrolGate => (vactrol_gate::GATE_STATE_FLOATS, 0),
        Node::DecayMod => (vactrol_gate::DECAY_MOD_STATE_FLOATS, 0),
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

/// Seed positions after removing voice-layer gates and their private controls.
/// The returned array is indexed by compiled position (`order`), not raw node.
pub(super) fn seed_ordinals(
    n: usize,
    nodes: &[Node],
    edges: &[crate::dsp::graph::Edge],
    order: &[u16],
) -> Result<[u16; NODE_CAP], BuildError> {
    let mut out = [0u16; NODE_CAP];
    for (compiled, seed) in out.iter_mut().enumerate().take(n) {
        *seed = u16::try_from(compiled).map_err(|_| BuildError::TooManyNodes)?;
    }
    if !nodes[..n]
        .iter()
        .any(|node| matches!(node, Node::VactrolGate))
    {
        return Ok(out);
    }

    let mut keep = [true; NODE_CAP];
    for (raw, node) in nodes.iter().enumerate().take(n) {
        if matches!(node, Node::VactrolGate) {
            keep[raw] = false;
        }
    }

    // Gate-only controls are source nodes whose complete fan-out is to a
    // non-subject gate port. Subjects remain even when they are sources.
    for (raw, _) in nodes.iter().enumerate().take(n) {
        if edges.iter().any(|edge| usize::from(edge.to) == raw) {
            continue;
        }
        let mut has_output = false;
        let mut gate_only = true;
        for edge in edges.iter().filter(|edge| usize::from(edge.from) == raw) {
            has_output = true;
            if !matches!(nodes[usize::from(edge.to)], Node::VactrolGate) || edge.port == 0 {
                gate_only = false;
                break;
            }
        }
        if has_output && gate_only {
            keep[raw] = false;
        }
    }

    // Map every gate to the source/output feeding its subject port. Following
    // this map handles gate-to-gate chains without allocating.
    let mut subjects = [u16::MAX; NODE_CAP];
    let mut subject_outputs = [0u8; NODE_CAP];
    for (gate, node) in nodes.iter().enumerate().take(n) {
        if !matches!(node, Node::VactrolGate) {
            continue;
        }
        if let Some(subject) = edges
            .iter()
            .find(|edge| usize::from(edge.to) == gate && edge.port == 0)
        {
            subjects[gate] = subject.from;
            subject_outputs[gate] = subject.output;
        }
    }

    let mut raw_to_compact = [u16::MAX; NODE_CAP];
    let mut compact_to_raw = [0u16; NODE_CAP];
    let mut kept = 0usize;
    for raw in 0..n {
        if keep[raw] {
            raw_to_compact[raw] = u16::try_from(kept).map_err(|_| BuildError::TooManyNodes)?;
            compact_to_raw[kept] = u16::try_from(raw).map_err(|_| BuildError::TooManyNodes)?;
            kept += 1;
        }
    }

    let mut elided_edges = [crate::dsp::graph::Edge {
        from: 0,
        to: 0,
        port: 0,
        output: 0,
    }; MAX_EDGES];
    let mut edge_count = 0usize;
    for edge in edges {
        let target = usize::from(edge.to);
        if !keep[target] {
            continue;
        }
        let mut source = usize::from(edge.from);
        let mut output = edge.output;
        let mut hops = 0usize;
        while matches!(nodes[source], Node::VactrolGate) && hops < n {
            if subjects[source] == u16::MAX {
                source = n;
                break;
            }
            output = subject_outputs[source];
            source = usize::from(subjects[source]);
            hops += 1;
        }
        if source >= n || !keep[source] {
            continue;
        }
        let mapped = elided_edges
            .get_mut(edge_count)
            .ok_or(BuildError::TooManyEdges)?;
        *mapped = crate::dsp::graph::Edge {
            from: raw_to_compact[source],
            to: raw_to_compact[target],
            port: edge.port,
            output,
        };
        edge_count += 1;
    }

    let elided_order = topo_order(kept, &elided_edges[..edge_count])?;
    let mut ordinal_by_raw = [u16::MAX; NODE_CAP];
    for (ordinal, compact) in elided_order.iter().copied().enumerate().take(kept) {
        let raw = usize::from(compact_to_raw[usize::from(compact)]);
        ordinal_by_raw[raw] = u16::try_from(ordinal).map_err(|_| BuildError::TooManyNodes)?;
    }
    for (compiled, raw) in order.iter().copied().enumerate().take(n) {
        let raw = usize::from(raw);
        if keep[raw] {
            out[compiled] = ordinal_by_raw[raw];
        }
    }
    Ok(out)
}
