//! Opt-in quad `.vact` definitions remain discoverable after explicit load.

use super::E2e;
use crate::dsp::arena::{decode_graph, encode_inst};
use crate::dsp::bus::BusTemplate;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::ugen::{frame_lfo, tidal_function, BuildEnv, BuildError, Node, RawGraph, Template};
use crate::session::editors::instrument_decls;
use crate::value::intern::name_of_kw;

#[test]
fn new_quad_templates_compile_and_expose_every_header_control_to_editor() {
    let mut e = E2e::new();
    let results =
        e.ev.eval_str(
            include_str!("../../../../../examples/quad-stems.vact"),
            super::super::SOURCE,
        )
        .unwrap();
    assert!(
        results.iter().all(|result| result.value.is_ok()),
        "{results:?}"
    );
    let registry = e.reg.borrow();
    let decls = instrument_decls(&registry);
    for (name, state, controls) in [
        (
            "frame-lfo-quad-voice",
            frame_lfo::STATE_FLOATS,
            &[
                "frame-shape",
                "frame-spread",
                "frame-shape-spread",
                "frame-coupling",
                "frame-offset",
            ][..],
        ),
        (
            "tidal-quad-voice",
            tidal_function::STATE_FLOATS,
            &[
                "tide-shape",
                "tide-slope",
                "tide-smoothness",
                "tide-ratio",
                "tide-sync",
                "tide-gate",
                "tide-clock",
                "tide-freeze",
                "tide-mode",
                "tide-range",
                "tide-pitch",
            ][..],
        ),
    ] {
        let entry = registry
            .entries()
            .find(|entry| &*name_of_kw(entry.name) == name)
            .unwrap();
        let mut wire = Vec::new();
        encode_inst(&entry.def, &mut wire).unwrap();
        let mut raw = RawGraph::boxed();
        decode_graph(&wire, &mut raw, &mut BusTemplate::new()).unwrap();
        assert!(raw.nodes[..raw.n_nodes].contains(&Node::Out3));
        assert!(raw.nodes[..raw.n_nodes].contains(&Node::Out4));
        let selector_port = if name == "tidal-quad-voice" { 12 } else { 6 };
        let expected = if name == "tidal-quad-voice" {
            crate::dsp::graph::UGenSpec::TidalFunction
        } else {
            crate::dsp::graph::UGenSpec::FrameLfo
        };
        let core_indices: Vec<_> = entry
            .def
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| (node == &expected).then_some(index))
            .collect();
        assert_eq!(core_indices.len(), 4);
        for (lane, node) in core_indices.iter().enumerate() {
            let edge = entry
                .def
                .edges
                .iter()
                .find(|edge| {
                    usize::from(edge.to) == *node && usize::from(edge.port) == selector_port
                })
                .unwrap();
            assert_eq!(
                entry.def.nodes[usize::from(edge.from)],
                crate::dsp::graph::UGenSpec::Const(lane as f32)
            );
        }
        let decl = decls.iter().find(|decl| decl.name == name).unwrap();
        for control in controls {
            assert!(
                decl.params
                    .iter()
                    .any(|param| param.name == *control && param.ctl.is_some()),
                "{name}/{control}"
            );
        }
        for sr in [44_100.0, 48_000.0, 96_000.0] {
            let env = BuildEnv {
                sr,
                caps: CapabilitySet::native(),
                voice_mem: 4 * state,
            };
            let template = Template::from_inst(&entry.def, &env).unwrap();
            assert_eq!(template.mem_total, 4 * state, "{name}/{sr}");
            assert!(template.has_aux && template.has_quad);
            assert_eq!(
                Template::from_inst(
                    &entry.def,
                    &BuildEnv {
                        voice_mem: 4 * state - 1,
                        ..env
                    }
                )
                .unwrap_err(),
                BuildError::MemExceeded
            );
        }
    }
}
