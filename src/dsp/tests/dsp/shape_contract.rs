use crate::dsp::graph::{
    assign_slices, decl_for_node, decl_for_spec, derive_shapes, select_output, voice_layout,
    AudioOutputShape, BankRef, Edge, EffectKind, EffectSpec, GranSrc, NodeAudioShape, OutputDecl,
    OutputSelector, SelectError, ShapeError, TableRef, UGenKind, UGenSpec, VoiceLayout, DISCARD,
    MAX_AUDIO_BUFFERS, MAX_OUTPUTS_PER_NODE, NODE_CAP,
};
use crate::dsp::ugen::Node;
use crate::sched::slots::CtlId;
use crate::value::intern::intern_kw;

fn edge(from: u16, to: u16, port: u8, output: u8) -> Edge {
    Edge {
        from,
        to,
        port,
        output,
    }
}

fn declarations(specs: &[UGenSpec]) -> impl Fn(usize) -> OutputDecl + '_ {
    |index| decl_for_spec(&specs[index])
}

fn specs_for_decl_parity() -> Vec<UGenSpec> {
    use UGenSpec::*;
    vec![
        SinOsc,
        Saw,
        Pulse,
        Tri,
        WhiteNoise,
        HostInputL,
        HostInputR,
        Lpf,
        Hpf,
        Bpf,
        Delay,
        Comb,
        EnvPerc,
        EnvAdsr,
        Line,
        SamplePlay(BankRef::new(0)),
        Mul,
        Add,
        Const(1.0),
        Param(CtlId::new(0)),
        Vco { unison_max: 1 },
        SubOsc,
        Ladder,
        Svf,
        FmOp,
        FmMod,
        FmDrum,
        FeedbackMetal,
        DigitalDrumCore,
        DigitalSnareCore,
        DigitalMetalCore,
        DigitalHatCore,
        AnalogPercussion,
        VaSource,
        VaFilter,
        PhasePair,
        FmPair,
        SixOpOriginal,
        SpeechOriginal,
        RingsPart,
        StringChoir,
        ElementsInternal,
        TidalFunction,
        TidalPoly,
        PeakFunction,
        StageSegment,
        StageChain,
        StageLinked { data: None },
        FrameLfo,
        FrameKeyframe { data: None },
        PeakPulse,
        NumberStation,
        SpectrumPair,
        ClockNoisePair,
        DualKick,
        SnarePair,
        HatPair,
        SwarmPair,
        ParticlePair,
        ModalPair,
        StringPair,
        ChipPair,
        AnalogPair,
        GrainPair,
        ShapePair,
        StringMachinePair,
        TerrainPair,
        TableTerrainPair,
        ChordPair,
        BraidsFive,
        BraidsSubSync,
        BraidsTriple,
        BraidsDigital,
        BraidsFilter,
        BraidsFormant,
        BraidsFm,
        BraidsPhysical,
        BraidsStruck,
        BraidsPercussion,
        BraidsWaveBank,
        BraidsWaveLine,
        BraidsNoise,
        BraidsCloud,
        AuxOut,
        Out3,
        Out4,
        FeedbackDrum,
        NoiseDrum,
        SineDrum,
        PhaseDistortion,
        Additive { partials_max: 1 },
        Wavetable(TableRef::new(0)),
        Granular(GranSrc::Bus),
        Effect(EffectSpec {
            kind: EffectKind::Gain,
            params: Box::new([]),
        }),
    ]
}

#[test]
fn every_constructible_spec_matches_its_compiled_node_declaration() {
    for spec in specs_for_decl_parity() {
        assert_eq!(
            decl_for_spec(&spec),
            decl_for_node(&Node::from_spec(&spec)),
            "{spec:?}"
        );
    }
}

#[test]
fn output_declarations_match_the_mod004_table() {
    let main_aux = OutputDecl::Fixed {
        shape: NodeAudioShape::new(2, 0).expect("valid output shape"),
        names: &["main", "aux"],
    };
    let duals = [
        UGenSpec::VaFilter,
        UGenSpec::FmPair,
        UGenSpec::AnalogPair,
        UGenSpec::ChordPair,
        UGenSpec::TableTerrainPair,
        UGenSpec::TerrainPair,
        UGenSpec::StringMachinePair,
        UGenSpec::ShapePair,
        UGenSpec::StageChain,
    ];
    for spec in &duals {
        assert_eq!(decl_for_spec(spec), main_aux, "{spec:?}");
    }
    assert_eq!(
        decl_for_spec(&UGenSpec::SamplePlay(BankRef::new(0))),
        OutputDecl::Fixed {
            shape: NodeAudioShape::new(2, 0b10).expect("valid stereo output shape"),
            names: &["mono", "stereo"],
        }
    );
    assert_eq!(
        decl_for_spec(&UGenSpec::Effect(EffectSpec {
            kind: EffectKind::Gain,
            params: Box::new([]),
        })),
        OutputDecl::FollowSubject
    );
    assert_eq!(decl_for_spec(&UGenSpec::Mul), OutputDecl::Elementwise);
    assert_eq!(decl_for_spec(&UGenSpec::Add), OutputDecl::Elementwise);
    assert_eq!(decl_for_spec(&UGenSpec::Const(0.0)), OutputDecl::Control);
    assert_eq!(
        decl_for_spec(&UGenSpec::Param(CtlId::new(0))),
        OutputDecl::Control
    );
    assert_eq!(
        decl_for_spec(&UGenSpec::AuxOut),
        OutputDecl::Tap { aux: true }
    );
    assert_eq!(
        decl_for_spec(&UGenSpec::Out3),
        OutputDecl::Tap { aux: false }
    );
    assert_eq!(
        decl_for_spec(&UGenSpec::Out4),
        OutputDecl::Tap { aux: false }
    );
    for spec in [
        UGenSpec::VaSource,
        UGenSpec::PhasePair,
        UGenSpec::SpectrumPair,
    ] {
        assert_eq!(
            decl_for_spec(&spec),
            OutputDecl::Fixed {
                shape: NodeAudioShape::MONO,
                names: &[],
            }
        );
    }
    let mono = OutputDecl::Fixed {
        shape: NodeAudioShape::MONO,
        names: &[],
    };
    for spec in specs_for_decl_parity() {
        if matches!(
            spec,
            UGenSpec::VaFilter
                | UGenSpec::FmPair
                | UGenSpec::AnalogPair
                | UGenSpec::ChordPair
                | UGenSpec::TableTerrainPair
                | UGenSpec::TerrainPair
                | UGenSpec::StringMachinePair
                | UGenSpec::ShapePair
                | UGenSpec::StageChain
                | UGenSpec::SamplePlay(_)
                | UGenSpec::Effect(_)
                | UGenSpec::Mul
                | UGenSpec::Add
                | UGenSpec::Const(_)
                | UGenSpec::Param(_)
                | UGenSpec::AuxOut
                | UGenSpec::Out3
                | UGenSpec::Out4
        ) {
            continue;
        }
        assert_eq!(decl_for_spec(&spec), mono, "default declaration: {spec:?}");
    }
}

#[test]
fn node_audio_shape_encoding_is_bounded_and_round_trips() {
    for count in 1..=MAX_OUTPUTS_PER_NODE as u8 {
        for mask in 0..(1_u8 << count) {
            let shape = NodeAudioShape::new(count, mask).expect("valid shape");
            assert_eq!(NodeAudioShape::from_byte(shape.to_byte()), Some(shape));
        }
    }
    assert_eq!(NodeAudioShape::from_byte(0b0100_0000), None);
    assert_eq!(NodeAudioShape::new(1, 0b10), None);
    assert_eq!(NodeAudioShape::new(0, 0), None);
    assert_eq!(NodeAudioShape::new(5, 0), None);
    assert_eq!(NodeAudioShape::MONO.output(1), None);
}

#[test]
fn output_selection_requires_declared_multi_output_names_or_valid_index() {
    let kind = UGenKind::Ugen(UGenSpec::VaFilter);
    assert_eq!(
        select_output(&kind, OutputSelector::Name(intern_kw("main"))),
        Ok(0)
    );
    assert_eq!(
        select_output(&kind, OutputSelector::Name(intern_kw("aux"))),
        Ok(1)
    );
    assert_eq!(select_output(&kind, OutputSelector::Index(1)), Ok(1));
    assert_eq!(
        select_output(&kind, OutputSelector::Index(2)),
        Err(SelectError::Unknown)
    );
    assert_eq!(
        select_output(&kind, OutputSelector::Name(intern_kw("left"))),
        Err(SelectError::Unknown)
    );
    assert_eq!(
        select_output(
            &UGenKind::Ugen(UGenSpec::SinOsc),
            OutputSelector::Name(intern_kw("main"))
        ),
        Err(SelectError::SingleOutput)
    );
    assert_eq!(
        select_output(&UGenKind::Output(1), OutputSelector::Index(0)),
        Err(SelectError::SingleOutput)
    );
    assert_eq!(
        select_output(
            &UGenKind::Ugen(UGenSpec::SamplePlay(BankRef::new(0))),
            OutputSelector::Name(intern_kw("stereo")),
        ),
        Ok(1)
    );
    assert_eq!(
        select_output(
            &UGenKind::Effect(EffectKind::Gain),
            OutputSelector::Index(0)
        ),
        Err(SelectError::SingleOutput)
    );
    assert_eq!(
        select_output(&UGenKind::BusInput, OutputSelector::Index(0)),
        Err(SelectError::SingleOutput)
    );
}

#[test]
fn derive_shapes_propagates_stereo_and_checks_inputs_edges_and_cycles() {
    let sample_effect = [
        UGenSpec::SamplePlay(BankRef::new(0)),
        UGenSpec::Effect(EffectSpec {
            kind: EffectKind::Gain,
            params: Box::new([]),
        }),
    ];
    let mut shapes = [NodeAudioShape::MONO; NODE_CAP];
    derive_shapes(
        2,
        declarations(&sample_effect),
        &[edge(0, 1, 0, 1)],
        &mut shapes,
    )
    .unwrap();
    assert_eq!(shapes[1].output(0), Some(AudioOutputShape::Stereo));

    assert_eq!(
        derive_shapes(
            2,
            declarations(&sample_effect),
            &[edge(0, 1, 1, 1)],
            &mut shapes
        ),
        Err(ShapeError::Mismatch { node: 1, port: 1 })
    );

    let sample_lpf = [UGenSpec::SamplePlay(BankRef::new(0)), UGenSpec::Lpf];
    assert_eq!(
        derive_shapes(
            2,
            declarations(&sample_lpf),
            &[edge(0, 1, 0, 1)],
            &mut shapes
        ),
        Err(ShapeError::Mismatch { node: 1, port: 0 })
    );

    let sample_control = [UGenSpec::SamplePlay(BankRef::new(0)), UGenSpec::Const(0.0)];
    assert_eq!(
        derive_shapes(
            2,
            declarations(&sample_control),
            &[edge(0, 1, 0, 1)],
            &mut shapes
        ),
        Err(ShapeError::Mismatch { node: 1, port: 0 })
    );

    let sample_tap = [UGenSpec::SamplePlay(BankRef::new(0)), UGenSpec::AuxOut];
    assert_eq!(
        derive_shapes(
            2,
            declarations(&sample_tap),
            &[edge(0, 1, 0, 1)],
            &mut shapes
        ),
        Err(ShapeError::Mismatch { node: 1, port: 0 })
    );

    let effect_without_subject = [UGenSpec::Effect(EffectSpec {
        kind: EffectKind::Gain,
        params: Box::new([]),
    })];
    derive_shapes(1, declarations(&effect_without_subject), &[], &mut shapes).unwrap();
    assert_eq!(shapes[0].output(0), Some(AudioOutputShape::Mono));

    let effect_with_mono_aux_input = [UGenSpec::SinOsc, effect_without_subject[0].clone()];
    derive_shapes(
        2,
        declarations(&effect_with_mono_aux_input),
        &[edge(0, 1, 1, 0)],
        &mut shapes,
    )
    .unwrap();
    assert_eq!(shapes[1].output(0), Some(AudioOutputShape::Mono));

    let stereo_param_mul = [
        UGenSpec::SamplePlay(BankRef::new(0)),
        UGenSpec::Param(CtlId::new(0)),
        UGenSpec::Mul,
    ];
    derive_shapes(
        3,
        declarations(&stereo_param_mul),
        &[edge(0, 2, 0, 1), edge(1, 2, 1, 0)],
        &mut shapes,
    )
    .unwrap();
    assert_eq!(shapes[2].output(0), Some(AudioOutputShape::Stereo));

    let stereo_mono_mul = [
        UGenSpec::SamplePlay(BankRef::new(0)),
        UGenSpec::SinOsc,
        UGenSpec::Mul,
    ];
    assert!(matches!(
        derive_shapes(
            3,
            declarations(&stereo_mono_mul),
            &[edge(0, 2, 0, 1), edge(1, 2, 1, 0)],
            &mut shapes,
        ),
        Err(ShapeError::Mismatch { node: 2, .. })
    ));

    let stereo_tap_add = [
        UGenSpec::SamplePlay(BankRef::new(0)),
        UGenSpec::AuxOut,
        UGenSpec::Add,
    ];
    derive_shapes(
        3,
        declarations(&stereo_tap_add),
        &[edge(0, 2, 0, 1), edge(1, 2, 1, 0)],
        &mut shapes,
    )
    .unwrap();
    assert_eq!(shapes[2].output(0), Some(AudioOutputShape::Stereo));

    let pair_filter = [UGenSpec::VaFilter, UGenSpec::Lpf];
    assert_eq!(
        derive_shapes(
            2,
            declarations(&pair_filter),
            &[edge(0, 1, 0, 2)],
            &mut shapes
        ),
        Err(ShapeError::BadOutput { edge: 0 })
    );
    let cycle = [UGenSpec::SinOsc, UGenSpec::Lpf];
    assert_eq!(
        derive_shapes(
            2,
            declarations(&cycle),
            &[edge(0, 1, 0, 0), edge(1, 0, 0, 0)],
            &mut shapes
        ),
        Err(ShapeError::Cycle)
    );
    assert_eq!(
        derive_shapes(2, declarations(&cycle), &[edge(2, 1, 0, 0)], &mut shapes),
        Err(ShapeError::BadEdge { edge: 0 })
    );
}

fn derive(specs: &[UGenSpec], edges: &[Edge]) -> [NodeAudioShape; NODE_CAP] {
    let mut shapes = [NodeAudioShape::MONO; NODE_CAP];
    derive_shapes(specs.len(), declarations(specs), edges, &mut shapes).unwrap();
    shapes
}

#[test]
fn voice_layout_classifies_mono_main_aux_and_stereo_sinks() {
    let mono = [UGenSpec::SinOsc, UGenSpec::Lpf];
    let mono_edges = [edge(0, 1, 0, 0)];
    assert_eq!(
        voice_layout(
            mono.len(),
            declarations(&mono),
            &derive(&mono, &mono_edges),
            &mono_edges
        ),
        Ok(VoiceLayout::Mono)
    );

    let main_aux = [UGenSpec::VaFilter, UGenSpec::Lpf, UGenSpec::AuxOut];
    let main_aux_edges = [edge(0, 1, 0, 0), edge(0, 2, 0, 1)];
    assert_eq!(
        voice_layout(
            main_aux.len(),
            declarations(&main_aux),
            &derive(&main_aux, &main_aux_edges),
            &main_aux_edges,
        ),
        Ok(VoiceLayout::MainAux)
    );

    let stereo = [
        UGenSpec::SamplePlay(BankRef::new(0)),
        UGenSpec::Effect(EffectSpec {
            kind: EffectKind::Gain,
            params: Box::new([]),
        }),
    ];
    let stereo_edges = [edge(0, 1, 0, 1)];
    assert_eq!(
        voice_layout(
            stereo.len(),
            declarations(&stereo),
            &derive(&stereo, &stereo_edges),
            &stereo_edges
        ),
        Ok(VoiceLayout::Stereo)
    );

    let stereo_aux = [
        stereo[0].clone(),
        stereo[1].clone(),
        UGenSpec::SinOsc,
        UGenSpec::AuxOut,
    ];
    let stereo_aux_edges = [edge(0, 1, 0, 1), edge(2, 3, 0, 0)];
    assert_eq!(
        voice_layout(
            stereo_aux.len(),
            declarations(&stereo_aux),
            &derive(&stereo_aux, &stereo_aux_edges),
            &stereo_aux_edges,
        ),
        Err(ShapeError::StereoAuxOut)
    );

    let stereo_mono_sinks = [stereo[0].clone(), stereo[1].clone(), UGenSpec::SinOsc];
    assert_eq!(
        voice_layout(
            stereo_mono_sinks.len(),
            declarations(&stereo_mono_sinks),
            &derive(&stereo_mono_sinks, &stereo_edges),
            &stereo_edges,
        ),
        Err(ShapeError::Mismatch { node: 2, port: 0 })
    );
}

#[test]
fn slice_assignment_is_dense_bounded_and_discards_unconsumed_outputs() {
    let mut shapes = [NodeAudioShape::MONO; NODE_CAP];
    let mono_order: Vec<u16> = (0..NODE_CAP as u16).collect();
    let mut slices = [[0; MAX_OUTPUTS_PER_NODE]; NODE_CAP];
    assert_eq!(
        assign_slices(&mono_order, &shapes, &[], &mut slices),
        Ok(256)
    );
    for (node, node_slices) in slices.iter().enumerate() {
        assert_eq!(node_slices[0], node as u16);
        assert_eq!(node_slices[1..], [DISCARD; MAX_OUTPUTS_PER_NODE - 1]);
    }

    let sample_shape = NodeAudioShape::new(2, 0b10).unwrap();
    shapes.fill(sample_shape);
    let sample_order: Vec<u16> = (0..171_u16).collect();
    let consumed_aux: Vec<Edge> = (0..171_u16)
        .map(|node| edge(node, (node + 1) % 171, 0, 1))
        .collect();
    assert_eq!(
        assign_slices(&sample_order, &shapes, &consumed_aux, &mut slices),
        Err(ShapeError::TooManyBuffers { need: 513 })
    );

    let pair = NodeAudioShape::new(2, 0).unwrap();
    shapes[0] = pair;
    assert_eq!(assign_slices(&[0], &shapes, &[], &mut slices), Ok(1));
    assert_eq!(slices[0][0], 0);
    assert_eq!(slices[0][1], DISCARD);

    let stereo = NodeAudioShape::new(1, 1).unwrap();
    shapes[0] = stereo;
    shapes[1] = NodeAudioShape::MONO;
    assert_eq!(assign_slices(&[1, 0], &shapes, &[], &mut slices), Ok(3));
    assert_eq!(slices[1][0], 0);
    assert_eq!(slices[0][0], 1);
    assert_eq!(slices[0][1], DISCARD);
    assert_eq!(MAX_AUDIO_BUFFERS, 512);
}
