//! Render-equivalence coverage for the nine eligible MOD-004 migrations.

use crate::dsp::graph::UGenSpec;
use crate::ns::insts::InstEntry;
use crate::value::intern::name_of_kw;

use super::super::E2e;

#[derive(Clone, Copy)]
enum Kernel {
    VaFilter,
    FmPair,
    AnalogPair,
    ChordPair,
    TableTerrainPair,
    TerrainPair,
    StringMachinePair,
    ShapePair,
    StageChain,
}

impl Kernel {
    fn matches(self, node: &UGenSpec) -> bool {
        matches!(
            (self, node),
            (Self::VaFilter, UGenSpec::VaFilter)
                | (Self::FmPair, UGenSpec::FmPair)
                | (Self::AnalogPair, UGenSpec::AnalogPair)
                | (Self::ChordPair, UGenSpec::ChordPair)
                | (Self::TableTerrainPair, UGenSpec::TableTerrainPair)
                | (Self::TerrainPair, UGenSpec::TerrainPair)
                | (Self::StringMachinePair, UGenSpec::StringMachinePair)
                | (Self::ShapePair, UGenSpec::ShapePair)
                | (Self::StageChain, UGenSpec::StageChain)
        )
    }
}

struct LegacyPair {
    name: &'static str,
    header: &'static str,
    old_body: &'static str,
    non_default_control: &'static str,
    kernel: Kernel,
}

const PAIRS: &[LegacyPair] = &[
    LegacyPair {
        name: "filter-voice",
        header: "morph: float = 0.5 timbre: float = 0.5 filter-harmonics: float = 0.5:",
        old_body: "va-source freq morph: morph > va-filter freq: freq timbre: timbre filter-harmonics: filter-harmonics mode: 0 > * amp\n\t\t> + {va-source freq morph: morph > va-filter freq: freq timbre: timbre filter-harmonics: filter-harmonics mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::VaFilter,
    },
    LegacyPair {
        name: "fm-pair-voice",
        header: "fm-harmonics: float = 0.5 timbre: float = 0.5 morph: float = 0.5:",
        old_body: "fm-pair freq fm-harmonics: fm-harmonics timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {fm-pair freq fm-harmonics: fm-harmonics timbre: timbre morph: morph mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::FmPair,
    },
    LegacyPair {
        name: "analog-pair-voice",
        header: "analog-detune: float = 0.5 timbre: float = 0.5 morph: float = 0.5:",
        old_body: "analog-pair-core freq analog-detune: analog-detune timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {analog-pair-core freq analog-detune: analog-detune timbre: timbre morph: morph mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::AnalogPair,
    },
    LegacyPair {
        name: "chord-layer-voice",
        header: "layer-chord: float = 0.5 timbre: float = 0.5 morph: float = 0.5:",
        old_body: "chord-layer-core freq layer-chord: layer-chord timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {chord-layer-core freq layer-chord: layer-chord timbre: timbre morph: morph mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::ChordPair,
    },
    LegacyPair {
        name: "wave-grid-voice",
        header: "wave-bank: float = 0.5 timbre: float = 0.5 morph: float = 0.5:",
        old_body: "wave-grid-core freq wave-bank: wave-bank timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {wave-grid-core freq wave-bank: wave-bank timbre: timbre morph: morph mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::TableTerrainPair,
    },
    LegacyPair {
        name: "terrain-voice",
        header: "terrain-select: float = 0.5 timbre: float = 0.5 morph: float = 0.5:",
        old_body: "terrain-pair-core freq terrain-select: terrain-select timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {terrain-pair-core freq terrain-select: terrain-select timbre: timbre morph: morph mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::TerrainPair,
    },
    LegacyPair {
        name: "string-machine-voice",
        header: "machine-chord: float = 0.5 timbre: float = 0.5 morph: float = 0.5:",
        old_body: "string-machine-core freq machine-chord: machine-chord timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {string-machine-core freq machine-chord: machine-chord timbre: timbre morph: morph mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::StringMachinePair,
    },
    LegacyPair {
        name: "shape-voice",
        header: "shape-harmonics: float = 0.5 timbre: float = 0.5 morph: float = 0.5:",
        old_body: "shape-pair-core freq shape-harmonics: shape-harmonics timbre: timbre morph: morph mode: 0 > * amp\n\t\t> + {shape-pair-core freq shape-harmonics: shape-harmonics timbre: timbre morph: morph mode: 1 > * amp > aux-out}",
        non_default_control: "timbre 0.9",
        kernel: Kernel::ShapePair,
    },
    LegacyPair {
        name: "stage-chain-voice",
        header: "chain-count: float = 3 chain-gate: float = 1 chain-trigger: float = 0 chain1-type: float = 0 chain1-loop: float = 0 chain1-primary: float = 0.2 chain1-secondary: float = 0.5 chain2-type: float = 2 chain2-loop: float = 1 chain2-primary: float = 0.7 chain2-secondary: float = 0.5 chain3-type: float = 0 chain3-loop: float = 0 chain3-primary: float = 0.2 chain3-secondary: float = 0.5 chain4-type: float = 0 chain4-loop: float = 0 chain4-primary: float = 0.2 chain4-secondary: float = 0.5 chain5-type: float = 0 chain5-loop: float = 0 chain5-primary: float = 0.2 chain5-secondary: float = 0.5 chain6-type: float = 0 chain6-loop: float = 0 chain6-primary: float = 0.2 chain6-secondary: float = 0.5:",
        old_body: "stage-chain-core freq chain-count: chain-count chain-gate: chain-gate chain-trigger: chain-trigger chain1-type: chain1-type chain1-loop: chain1-loop chain1-primary: chain1-primary chain1-secondary: chain1-secondary chain2-type: chain2-type chain2-loop: chain2-loop chain2-primary: chain2-primary chain2-secondary: chain2-secondary chain3-type: chain3-type chain3-loop: chain3-loop chain3-primary: chain3-primary chain3-secondary: chain3-secondary chain4-type: chain4-type chain4-loop: chain4-loop chain4-primary: chain4-primary chain4-secondary: chain4-secondary chain5-type: chain5-type chain5-loop: chain5-loop chain5-primary: chain5-primary chain5-secondary: chain5-secondary chain6-type: chain6-type chain6-loop: chain6-loop chain6-primary: chain6-primary chain6-secondary: chain6-secondary chain-channel: 0 > * amp\n\t\t> + {stage-chain-core freq chain-count: chain-count chain-gate: chain-gate chain-trigger: chain-trigger chain1-type: chain1-type chain1-loop: chain1-loop chain1-primary: chain1-primary chain1-secondary: chain1-secondary chain2-type: chain2-type chain2-loop: chain2-loop chain2-primary: chain2-primary chain2-secondary: chain2-secondary chain3-type: chain3-type chain3-loop: chain3-loop chain3-primary: chain3-primary chain3-secondary: chain3-secondary chain4-type: chain4-type chain4-loop: chain4-loop chain4-primary: chain4-primary chain4-secondary: chain4-secondary chain5-type: chain5-type chain5-loop: chain5-loop chain5-primary: chain5-primary chain5-secondary: chain5-secondary chain6-type: chain6-type chain6-loop: chain6-loop chain6-primary: chain6-primary chain6-secondary: chain6-secondary chain-channel: 1 > * amp > aux-out}",
        non_default_control: "chain1-primary 0.7",
        kernel: Kernel::StageChain,
    },
];

fn entry(e: &E2e, name: &str) -> InstEntry {
    e.reg
        .borrow()
        .entries()
        .find(|item| *name_of_kw(item.name) == *name)
        .expect("instrument definition is registered")
        .clone()
}

fn render(template: &str, definition: Option<&str>, control: &str) -> (Vec<f32>, Vec<f32>) {
    let mut e = E2e::new();
    if let Some(source) = definition {
        e.eval(source);
    }
    let name = if definition.is_some() {
        format!("old-{template}")
    } else {
        template.to_owned()
    };
    let event = if control.is_empty() {
        format!("s :{name} > note [:a3] > once")
    } else {
        format!("s :{name} > note [:a3] > {control} > once")
    };
    e.eval(&event);
    let output = e.run_stereo_for(0.5);
    assert!(e.faults.is_empty(), "{name} {control}: {:?}", e.faults);
    assert!(e.committed > 0, "{name} {control}: event committed");
    output
}

fn assert_bit_identical(name: &str, channel: &str, new: &[f32], old: &[f32]) {
    assert_eq!(new.len(), old.len(), "{name} {channel}: frame count");
    let new_rms = super::super::rms(new);
    let old_rms = super::super::rms(old);
    assert!(new_rms > 1.0e-5, "{name} new {channel}: rms {new_rms}");
    assert!(old_rms > 1.0e-5, "{name} old {channel}: rms {old_rms}");
    for (index, (new_sample, old_sample)) in new.iter().zip(old).enumerate() {
        assert_eq!(
            new_sample.to_bits(),
            old_sample.to_bits(),
            "{name} {channel} sample {index}"
        );
    }
}

#[test]
fn migrated_pairs_match_the_legacy_two_node_renders() {
    for pair in PAIRS {
        let old_definition = format!(
            "inst old-{} {}\n\t{}",
            pair.name, pair.header, pair.old_body
        );
        for control in ["", pair.non_default_control] {
            let (new_left, new_right) = render(pair.name, None, control);
            let (old_left, old_right) = render(pair.name, Some(&old_definition), control);
            assert_bit_identical(pair.name, "main/left", &new_left, &old_left);
            assert_bit_identical(pair.name, "aux/right", &new_right, &old_right);
        }
    }
}

#[test]
fn migrated_and_excluded_template_graphs_keep_their_expected_shapes() {
    let e = E2e::new();
    for pair in PAIRS {
        let definition = entry(&e, pair.name);
        let kernel_nodes: Vec<usize> = definition
            .def
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| pair.kernel.matches(node).then_some(index))
            .collect();
        assert_eq!(kernel_nodes.len(), 1, "{}: one paired kernel", pair.name);
        if matches!(pair.kernel, Kernel::VaFilter) {
            assert_eq!(
                definition
                    .def
                    .nodes
                    .iter()
                    .filter(|node| matches!(node, UGenSpec::VaSource))
                    .count(),
                1,
                "filter-voice has one VaSource"
            );
        }
        let kernel = u16::try_from(kernel_nodes[0]).expect("node index fits");
        let mut outputs: Vec<u8> = definition
            .def
            .edges
            .iter()
            .filter(|edge| edge.from == kernel)
            .map(|edge| edge.output)
            .collect();
        outputs.sort_unstable();
        assert_eq!(outputs, [0, 1], "{}: main and aux edges", pair.name);
    }

    for name in ["phase-pair-voice", "resonator-voice"] {
        let definition = entry(&e, name);
        let count = definition
            .def
            .nodes
            .iter()
            .filter(|node| match name {
                "phase-pair-voice" => matches!(node, UGenSpec::PhasePair),
                "resonator-voice" => matches!(node, UGenSpec::RingsPart),
                _ => false,
            })
            .count();
        assert_eq!(count, 2, "{name}: excluded duplicate kernel nodes");
        assert!(
            definition.def.edges.iter().all(|edge| edge.output == 0),
            "{name}: legacy graph edges select only output 0"
        );
    }
}
