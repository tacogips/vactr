//! Bounded audio output declarations and allocation-free graph shape checks.

use crate::dsp::graph::{Edge, UGenKind, UGenSpec, NODE_CAP};
use crate::dsp::ugen::Node;
use crate::value::intern::name_of_kw;

pub const MAX_OUTPUTS_PER_NODE: usize = 4;
pub const MAX_AUDIO_BUFFERS: usize = 512;
pub const DISCARD_SLICES: usize = 2;
pub const DISCARD: u16 = u16::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioOutputShape {
    Mono,
    Stereo,
}

impl AudioOutputShape {
    #[must_use]
    pub const fn channels(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeAudioShape {
    count: u8,
    stereo_mask: u8,
}

impl NodeAudioShape {
    pub const MONO: Self = Self {
        count: 1,
        stereo_mask: 0,
    };

    #[must_use]
    pub const fn new(count: u8, stereo_mask: u8) -> Option<Self> {
        if count == 0 || count as usize > MAX_OUTPUTS_PER_NODE || stereo_mask >> count != 0 {
            None
        } else {
            Some(Self { count, stereo_mask })
        }
    }

    #[must_use]
    pub const fn count(self) -> usize {
        self.count as usize
    }

    #[must_use]
    pub const fn output(self, k: usize) -> Option<AudioOutputShape> {
        if k >= self.count as usize {
            None
        } else if self.stereo_mask & (1 << k) != 0 {
            Some(AudioOutputShape::Stereo)
        } else {
            Some(AudioOutputShape::Mono)
        }
    }

    #[must_use]
    pub const fn to_byte(self) -> u8 {
        (self.count - 1) | (self.stereo_mask << 2)
    }

    #[must_use]
    pub const fn from_byte(byte: u8) -> Option<Self> {
        if byte & 0b1100_0000 != 0 {
            return None;
        }
        let count = (byte & 0b11) + 1;
        let stereo_mask = (byte >> 2) & 0b1111;
        Self::new(count, stereo_mask)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputDecl {
    Fixed {
        shape: NodeAudioShape,
        names: &'static [&'static str],
    },
    FollowSubject,
    Elementwise,
    Control,
    Tap {
        aux: bool,
    },
}

const MONO: OutputDecl = OutputDecl::Fixed {
    shape: NodeAudioShape::MONO,
    names: &[],
};
const MAIN_AUX: OutputDecl = OutputDecl::Fixed {
    shape: NodeAudioShape {
        count: 2,
        stereo_mask: 0,
    },
    names: &["main", "aux"],
};
const SAMPLE_PLAY: OutputDecl = OutputDecl::Fixed {
    shape: NodeAudioShape {
        count: 2,
        stereo_mask: 0b10,
    },
    names: &["mono", "stereo"],
};

#[must_use]
pub const fn decl_for_spec(spec: &UGenSpec) -> OutputDecl {
    match spec {
        UGenSpec::VaFilter
        | UGenSpec::FmPair
        | UGenSpec::AnalogPair
        | UGenSpec::ChordPair
        | UGenSpec::TableTerrainPair
        | UGenSpec::TerrainPair
        | UGenSpec::StringMachinePair
        | UGenSpec::ShapePair
        | UGenSpec::StageChain => MAIN_AUX,
        UGenSpec::SamplePlay(_) => SAMPLE_PLAY,
        UGenSpec::Effect(_) => OutputDecl::FollowSubject,
        UGenSpec::Mul | UGenSpec::Add => OutputDecl::Elementwise,
        UGenSpec::Const(_) | UGenSpec::Param(_) => OutputDecl::Control,
        UGenSpec::AuxOut => OutputDecl::Tap { aux: true },
        UGenSpec::Out3 | UGenSpec::Out4 => OutputDecl::Tap { aux: false },
        _ => MONO,
    }
}

#[must_use]
pub const fn decl_for_node(node: &Node) -> OutputDecl {
    match node {
        Node::VaFilter
        | Node::FmPair
        | Node::AnalogPair
        | Node::ChordPair
        | Node::TableTerrainPair
        | Node::TerrainPair
        | Node::StringMachinePair
        | Node::ShapePair
        | Node::StageChain => MAIN_AUX,
        Node::SamplePlay(_) => SAMPLE_PLAY,
        Node::Effect { .. } => OutputDecl::FollowSubject,
        Node::Mul | Node::Add => OutputDecl::Elementwise,
        Node::Const(_) | Node::Param(_) => OutputDecl::Control,
        Node::AuxOut => OutputDecl::Tap { aux: true },
        Node::Out3 | Node::Out4 => OutputDecl::Tap { aux: false },
        _ => MONO,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputSelector {
    Name(crate::value::intern::KwId),
    Index(i64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectError {
    SingleOutput,
    Unknown,
}

pub fn select_output(kind: &UGenKind, selector: OutputSelector) -> Result<u8, SelectError> {
    let UGenKind::Ugen(spec) = kind else {
        return Err(SelectError::SingleOutput);
    };
    let OutputDecl::Fixed { shape, names } = decl_for_spec(spec) else {
        return Err(SelectError::SingleOutput);
    };
    if names.len() < 2 {
        return Err(SelectError::SingleOutput);
    }
    match selector {
        OutputSelector::Name(keyword) => names
            .iter()
            .position(|name| *name == &*name_of_kw(keyword))
            .and_then(|index| u8::try_from(index).ok())
            .ok_or(SelectError::Unknown),
        OutputSelector::Index(index) => {
            if index >= 0 && usize::try_from(index).is_ok_and(|i| i < shape.count()) {
                Ok(index as u8)
            } else {
                Err(SelectError::Unknown)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeError {
    BadEdge { edge: usize },
    BadOutput { edge: usize },
    Mismatch { node: u16, port: u8 },
    StereoAuxOut,
    Cycle,
    TooManyBuffers { need: usize },
}

const EMPTY_DECL: OutputDecl = MONO;

fn output_count(decl: OutputDecl) -> usize {
    match decl {
        OutputDecl::Fixed { shape, .. } => shape.count(),
        OutputDecl::FollowSubject
        | OutputDecl::Elementwise
        | OutputDecl::Control
        | OutputDecl::Tap { .. } => 1,
    }
}

fn output_shape(
    decl: OutputDecl,
    shape: NodeAudioShape,
    output: usize,
) -> Option<AudioOutputShape> {
    match decl {
        OutputDecl::Fixed { shape, .. } => shape.output(output),
        OutputDecl::FollowSubject | OutputDecl::Elementwise => {
            if output == 0 {
                shape.output(0)
            } else {
                None
            }
        }
        OutputDecl::Control | OutputDecl::Tap { .. } => {
            (output == 0).then_some(AudioOutputShape::Mono)
        }
    }
}

/// Derives every node's bounded output shape using a fixed-array Kahn walk.
pub fn derive_shapes(
    n: usize,
    decl: impl Fn(usize) -> OutputDecl,
    edges: &[Edge],
    out: &mut [NodeAudioShape; NODE_CAP],
) -> Result<(), ShapeError> {
    if n > NODE_CAP {
        return Err(ShapeError::TooManyBuffers { need: n });
    }
    let mut declarations = [EMPTY_DECL; NODE_CAP];
    let mut indegree = [0_u16; NODE_CAP];
    for (node, slot) in declarations.iter_mut().take(n).enumerate() {
        *slot = decl(node);
    }
    for (edge_index, edge) in edges.iter().enumerate() {
        let (from, to) = (usize::from(edge.from), usize::from(edge.to));
        if from >= n || to >= n {
            return Err(ShapeError::BadEdge { edge: edge_index });
        }
        if usize::from(edge.output) >= output_count(declarations[from]) {
            return Err(ShapeError::BadOutput { edge: edge_index });
        }
        indegree[to] += 1;
    }

    let mut queue = [0_u16; NODE_CAP];
    let mut read = 0;
    let mut written = 0;
    for (node, degree) in indegree.iter().enumerate().take(n) {
        if *degree == 0 {
            queue[written] = node as u16;
            written += 1;
        }
    }
    while read < written {
        let node = usize::from(queue[read]);
        read += 1;
        let node_id = node as u16;
        let node_decl = declarations[node];
        let mut shape = match node_decl {
            OutputDecl::Fixed { shape, .. } => shape,
            OutputDecl::FollowSubject => {
                let mut subject = None;
                for (edge_index, edge) in edges.iter().enumerate() {
                    if usize::from(edge.to) != node || edge.port != 0 {
                        continue;
                    }
                    let source = usize::from(edge.from);
                    let source_shape =
                        output_shape(declarations[source], out[source], usize::from(edge.output))
                            .ok_or(ShapeError::BadOutput { edge: edge_index })?;
                    if subject.is_some_and(|previous| previous != source_shape) {
                        return Err(ShapeError::Mismatch {
                            node: node_id,
                            port: 0,
                        });
                    }
                    subject = Some(source_shape);
                }
                match subject.unwrap_or(AudioOutputShape::Mono) {
                    AudioOutputShape::Mono => NodeAudioShape::MONO,
                    AudioOutputShape::Stereo => NodeAudioShape {
                        count: 1,
                        stereo_mask: 1,
                    },
                }
            }
            OutputDecl::Elementwise => {
                let mut has_stereo_audio = false;
                for (edge_index, edge) in edges.iter().enumerate() {
                    if usize::from(edge.to) != node {
                        continue;
                    }
                    let source = usize::from(edge.from);
                    if matches!(
                        declarations[source],
                        OutputDecl::Control | OutputDecl::Tap { .. }
                    ) {
                        continue;
                    }
                    let source_output =
                        output_shape(declarations[source], out[source], usize::from(edge.output))
                            .ok_or(ShapeError::BadOutput { edge: edge_index })?;
                    has_stereo_audio |= source_output == AudioOutputShape::Stereo;
                }
                if has_stereo_audio {
                    for (edge_index, edge) in edges.iter().enumerate() {
                        if usize::from(edge.to) != node {
                            continue;
                        }
                        let source = usize::from(edge.from);
                        if matches!(
                            declarations[source],
                            OutputDecl::Control | OutputDecl::Tap { .. }
                        ) {
                            continue;
                        }
                        let source_output = output_shape(
                            declarations[source],
                            out[source],
                            usize::from(edge.output),
                        )
                        .ok_or(ShapeError::BadOutput { edge: edge_index })?;
                        if source_output != AudioOutputShape::Stereo {
                            return Err(ShapeError::Mismatch {
                                node: node_id,
                                port: edge.port,
                            });
                        }
                    }
                    NodeAudioShape {
                        count: 1,
                        stereo_mask: 1,
                    }
                } else {
                    NodeAudioShape::MONO
                }
            }
            OutputDecl::Control | OutputDecl::Tap { .. } => NodeAudioShape::MONO,
        };

        for (edge_index, edge) in edges.iter().enumerate() {
            if usize::from(edge.to) != node {
                continue;
            }
            let source = usize::from(edge.from);
            let source_shape =
                output_shape(declarations[source], out[source], usize::from(edge.output))
                    .ok_or(ShapeError::BadOutput { edge: edge_index })?;
            match node_decl {
                OutputDecl::Fixed { .. } | OutputDecl::Control | OutputDecl::Tap { .. } => {
                    if source_shape != AudioOutputShape::Mono {
                        return Err(ShapeError::Mismatch {
                            node: node_id,
                            port: edge.port,
                        });
                    }
                }
                OutputDecl::FollowSubject => {
                    if edge.port != 0 && source_shape != AudioOutputShape::Mono {
                        return Err(ShapeError::Mismatch {
                            node: node_id,
                            port: edge.port,
                        });
                    }
                    if edge.port == 0 {
                        shape = match source_shape {
                            AudioOutputShape::Mono => NodeAudioShape::MONO,
                            AudioOutputShape::Stereo => NodeAudioShape {
                                count: 1,
                                stereo_mask: 1,
                            },
                        };
                    }
                }
                OutputDecl::Elementwise => {}
            }
        }
        out[node] = shape;

        for edge in edges.iter().filter(|edge| usize::from(edge.from) == node) {
            let target = usize::from(edge.to);
            indegree[target] -= 1;
            if indegree[target] == 0 {
                queue[written] = edge.to;
                written += 1;
            }
        }
    }
    if read != n {
        return Err(ShapeError::Cycle);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceLayout {
    Mono,
    MainAux,
    Stereo,
}

pub fn voice_layout(
    n: usize,
    decl: impl Fn(usize) -> OutputDecl,
    shapes: &[NodeAudioShape],
    edges: &[Edge],
) -> Result<VoiceLayout, ShapeError> {
    let mut has_aux = false;
    let mut stereo_sink = false;
    let mut first_mono_sink = None;
    for node in 0..n {
        let node_decl = decl(node);
        has_aux |= matches!(node_decl, OutputDecl::Tap { aux: true });
        let sink = !edges.iter().any(|edge| usize::from(edge.from) == node);
        if !sink || matches!(node_decl, OutputDecl::Tap { .. }) {
            continue;
        }
        let shape = shapes
            .get(node)
            .ok_or(ShapeError::TooManyBuffers { need: n })?;
        match shape.output(0) {
            Some(AudioOutputShape::Stereo) => stereo_sink = true,
            Some(AudioOutputShape::Mono) => {
                first_mono_sink.get_or_insert(node as u16);
            }
            None => {
                return Err(ShapeError::Mismatch {
                    node: node as u16,
                    port: 0,
                })
            }
        };
    }
    if stereo_sink {
        if let Some(node) = first_mono_sink {
            return Err(ShapeError::Mismatch { node, port: 0 });
        }
        if has_aux {
            return Err(ShapeError::StereoAuxOut);
        }
        Ok(VoiceLayout::Stereo)
    } else if has_aux {
        Ok(VoiceLayout::MainAux)
    } else {
        Ok(VoiceLayout::Mono)
    }
}

pub fn assign_slices(
    order: &[u16],
    shapes: &[NodeAudioShape],
    edges: &[Edge],
    out: &mut [[u16; MAX_OUTPUTS_PER_NODE]; NODE_CAP],
) -> Result<usize, ShapeError> {
    out.fill([DISCARD; MAX_OUTPUTS_PER_NODE]);
    let mut next = 0_usize;
    for &node_id in order {
        let node = usize::from(node_id);
        let shape = shapes.get(node).ok_or(ShapeError::TooManyBuffers {
            need: node.saturating_add(1),
        })?;
        for output in 0..shape.count() {
            let consumed = output == 0
                || edges
                    .iter()
                    .any(|edge| edge.from == node_id && usize::from(edge.output) == output);
            if !consumed {
                continue;
            }
            let channels = shape.output(output).map_or(0, AudioOutputShape::channels);
            if next + channels > MAX_AUDIO_BUFFERS {
                return Err(ShapeError::TooManyBuffers {
                    need: next + channels,
                });
            }
            out[node][output] = next as u16;
            next += channels;
        }
    }
    Ok(next)
}
