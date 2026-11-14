//! Voice lifetime decisions for layer-shaped Plaits templates.

use crate::dsp::ugen::{Node, NodeState, Template};

/// True once every gate has latched a valid, non-bypassed shaping mode.
pub(super) fn layer_shaped(t: &Template, states: &[NodeState]) -> bool {
    if t.gates == 0 {
        return false;
    }
    let mut found = false;
    for (spec, state) in t.nodes().iter().zip(states) {
        if matches!(spec.node, Node::VactrolGate) {
            found = true;
            if state.u[1] != 1 || state.u[2] != 1 {
                return false;
            }
        }
    }
    found
}

/// Whether the voice's implicit release fade applies.
pub(super) fn implicit_fade(t: &Template, states: &[NodeState]) -> bool {
    t.envs == 0 && t.players == 0 && !layer_shaped(t, states)
}

/// Whether the voice has reached its natural end.
pub(super) fn finished(t: &Template, states: &[NodeState], ienv: f32) -> bool {
    if t.envs > 0 {
        return t
            .nodes()
            .iter()
            .zip(states)
            .filter(|(spec, _)| spec.node.is_env())
            .all(|(_, state)| state.done());
    }
    if t.players > 0 {
        return t
            .nodes()
            .iter()
            .zip(states)
            .filter(|(spec, _)| matches!(spec.node, Node::SamplePlay(_)))
            .all(|(_, state)| state.done());
    }
    if layer_shaped(t, states) {
        return t
            .nodes()
            .iter()
            .zip(states)
            .filter(|(spec, _)| matches!(spec.node, Node::VactrolGate))
            .all(|(_, state)| state.done());
    }
    ienv <= 0.0
}
