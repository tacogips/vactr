//! Genuine lowering and installed graph authority precede closed PCM preparation.
use std::{rc::Rc, sync::Arc};
use vactr::dsp::{
    build::{
        lower_bus, lower_bus_with_resources, lower_inst, lower_inst_with_resources,
        DeclaredGraphResource, GraphResourceCaptureMode, GraphResourceInput, GraphResourceSite,
        Lowering,
    },
    caps::CapabilitySet,
    cells::CellId,
    effects,
    graph::{BusDef, BusId, EffectKind, InstId, UGenInput, UGenKind, UGenNode, UGenSpec, NODE_CAP},
};
use vactr::ns::{evaluator::Evaluator, load::NoopHost, namespace::Prelude, stage::RecordingSink};
use vactr::reader::span::{FileId, Span};
use vactr::sched::slots::CtlId;
use vactr::value::intern::{intern_kw, KwId};

fn evaluator(closed: bool) -> Evaluator {
    let ev = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    if closed {
        ev.insts()
            .unwrap()
            .borrow_mut()
            .enable_closed_song_resources();
    }
    ev
}
fn clean(ev: &mut Evaluator, code: &str) {
    let rows = ev.eval_str(code, FileId::CONSOLE).unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(row.value.is_ok(), "{code}: {:?}", row.value);
        assert!(
            row.diags
                .iter()
                .all(|d| d.severity != vactr::types::Severity::Error),
            "{code}: {:?}",
            row.diags
        );
    }
}
fn node(kind: UGenKind, args: Vec<(Option<KwId>, UGenInput)>) -> Rc<UGenNode> {
    Rc::new(UGenNode {
        kind,
        args: args.into_boxed_slice(),
    })
}
fn oscillator() -> Rc<UGenNode> {
    node(UGenKind::Ugen(UGenSpec::SinOsc), Vec::new())
}
fn ir(subject: Rc<UGenNode>, value: UGenInput) -> Rc<UGenNode> {
    node(
        UGenKind::Effect(EffectKind::Convolution),
        vec![
            (None, UGenInput::Node(subject)),
            (Some(intern_kw("ir")), value),
        ],
    )
}
fn lower(
    root: &Rc<UGenNode>,
    mode: GraphResourceCaptureMode,
) -> (vactr::dsp::graph::InstDef, vactr::dsp::build::Extras) {
    let caps = CapabilitySet::native();
    let mut alloc = || Some(CellId::new(1));
    lower_inst_with_resources(
        InstId::new(4),
        root,
        &[],
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc,
        },
        mode,
    )
    .unwrap()
}
fn bank_record<'a>(records: &'a [DeclaredGraphResource], bank: &str) -> &'a DeclaredGraphResource {
    records
        .iter()
        .find(|r| r.input() == &GraphResourceInput::Bank(intern_kw(bank)))
        .unwrap()
}
#[test]
fn closed_capture_preserves_original_ir_banks_at_actual_sites() {
    let mut ev = evaluator(true);
    clean(&mut ev, "inst captured bank: keyword = :event-a table: keyword = :event-b:\n\tsin-osc freq > convolution ir: :fixed-a\nbus :captured-bus:\n\tconvolution ir: :fixed-b\nmaster:\n\tconvolution ir: :fixed-c");
    let registry = ev.insts().unwrap();
    let r = registry.borrow();
    let id = r.id_of(intern_kw("captured")).unwrap();
    let entry = r.entry(id).unwrap();
    let declared = r.inst_resources(id).unwrap();
    assert!(Arc::ptr_eq(declared.graph(), &entry.def));
    assert_eq!(
        entry.resource,
        Some(intern_kw("event-b")),
        "old last-header route is unchanged"
    );
    assert_eq!(declared.resources().len(), 3);
    for bank in ["event-a", "event-b"] {
        assert!(matches!(
            bank_record(declared.resources(), bank).site(),
            GraphResourceSite::Header { .. }
        ));
    }
    let GraphResourceSite::EmbeddedEffect { node, parameter } =
        bank_record(declared.resources(), "fixed-a").site()
    else {
        panic!("embedded site")
    };
    let UGenSpec::Effect(effect) = &entry.def.nodes[usize::from(*node)] else {
        panic!("real effect node")
    };
    assert_eq!(effect.kind, EffectKind::Convolution);
    assert!(effect
        .params
        .iter()
        .any(|(id, v)| id == parameter && *v == vactr::host::wire::Ctl::Const(0.)));
    for (name, bank) in [(Some("captured-bus"), "fixed-b"), (None, "fixed-c")] {
        let bus = if let Some(name) = name {
            r.bus(intern_kw(name)).unwrap()
        } else {
            r.master().unwrap()
        };
        let declared = r.bus_resources(bus.id).unwrap();
        assert!(Arc::ptr_eq(declared.graph(), &bus.def));
        assert!(matches!(
            bank_record(declared.resources(), bank).site(),
            GraphResourceSite::BusEffect { effect: 0, .. }
        ));
    }
}
#[test]
fn embedded_shared_dag_and_reversed_bus_chain_keep_site_indices() {
    let shared = ir(oscillator(), UGenInput::Keyword(intern_kw("shared")));
    let root = node(
        UGenKind::Ugen(UGenSpec::Add),
        vec![
            (None, UGenInput::Node(Rc::clone(&shared))),
            (None, UGenInput::Node(shared)),
        ],
    );
    let (def, extras) = lower(&root, GraphResourceCaptureMode::ClosedSong);
    assert_eq!(extras.resources.len(), 1, "same Rc lowers once");
    let GraphResourceSite::EmbeddedEffect {
        node: node_index,
        parameter,
    } = extras.resources[0].site()
    else {
        panic!("actual DAG node")
    };
    assert_eq!(
        *parameter,
        effects::param_ctl(EffectKind::Convolution, "ir").unwrap()
    );
    assert!(matches!(
        def.nodes[usize::from(*node_index)],
        UGenSpec::Effect(_)
    ));
    let bus = ir(
        ir(
            node(UGenKind::BusInput, Vec::new()),
            UGenInput::Keyword(intern_kw("first")),
        ),
        UGenInput::Keyword(intern_kw("second")),
    );
    let caps = CapabilitySet::native();
    let mut alloc = || Some(CellId::new(1));
    let (def, extras) = lower_bus_with_resources(
        BusId::new(7),
        &bus,
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc,
        },
        GraphResourceCaptureMode::ClosedSong,
    )
    .unwrap();
    assert_eq!(def.chain.len(), 2);
    assert!(matches!(
        bank_record(&extras.resources, "first").site(),
        GraphResourceSite::BusEffect { effect: 0, .. }
    ));
    assert!(matches!(
        bank_record(&extras.resources, "second").site(),
        GraphResourceSite::BusEffect { effect: 1, .. }
    ));
}
#[test]
fn replacement_and_legacy_install_invalidate_old_arc_metadata() {
    let mut ev = evaluator(true);
    clean(&mut ev,"inst replaced:\n\tsin-osc freq > convolution ir: :old\nbus :replacement:\n\tconvolution ir: :old");
    let registry = ev.insts().unwrap();
    let (id, old_inst, old_bus) = {
        let r = registry.borrow();
        let id = r.id_of(intern_kw("replaced")).unwrap();
        (
            id,
            r.inst_resources(id).unwrap().clone(),
            r.bus_resources(r.bus(intern_kw("replacement")).unwrap().id)
                .unwrap()
                .clone(),
        )
    };
    clean(&mut ev,"inst replaced:\n\tsin-osc freq > convolution ir: :new\nbus :replacement:\n\tconvolution ir: :new");
    {
        let r = registry.borrow();
        let current = r.inst_resources(id).unwrap();
        assert!(!Arc::ptr_eq(old_inst.graph(), current.graph()));
        assert_eq!(
            bank_record(current.resources(), "new").input(),
            &GraphResourceInput::Bank(intern_kw("new"))
        );
    }
    let prior = registry.borrow().entry(id).unwrap().def.clone();
    let failed = ev
        .eval_str(
            "inst replaced:\n\tsin-osc freq > convolution ir: :new > nonexistent-ugen",
            FileId::CONSOLE,
        )
        .unwrap();
    assert!(failed.iter().any(|row| row.value.is_err()
        || row
            .diags
            .iter()
            .any(|d| d.severity == vactr::types::Severity::Error)));
    assert!(Arc::ptr_eq(
        &prior,
        &registry.borrow().entry(id).unwrap().def
    ));
    let bus_id = old_bus.graph().id;
    registry.borrow_mut().install_bus(
        Some(intern_kw("replacement")),
        BusDef {
            id: bus_id,
            chain: Box::new([]),
        },
        Vec::new(),
    );
    assert!(registry.borrow().bus_resources(bus_id).is_none());
    assert!(!Arc::ptr_eq(
        old_bus.graph(),
        &registry.borrow().bus(intern_kw("replacement")).unwrap().def
    ));
}
#[test]
fn legacy_ir_keywords_and_event_resource_arguments_keep_old_semantics() {
    let caps = CapabilitySet::native();
    let root = ir(
        oscillator(),
        UGenInput::Keyword(intern_kw("no-legacy-extension")),
    );
    let mut alloc = || None;
    assert!(lower_inst(
        InstId::new(1),
        &root,
        &[],
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc
        }
    )
    .is_err());
    let bus = ir(
        node(UGenKind::BusInput, Vec::new()),
        UGenInput::Keyword(intern_kw("no-legacy-extension")),
    );
    assert!(lower_bus(
        BusId::new(1),
        &bus,
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc
        }
    )
    .is_err());
    let (_, extras) = lower(
        &ir(oscillator(), UGenInput::Const(-1.)),
        GraphResourceCaptureMode::ClosedSong,
    );
    assert!(extras.resources.is_empty());
    let mut ev = evaluator(false);
    clean(&mut ev,"inst events bank: keyword = :event-bank:\n\tsample-play bank n: 0 rate: 1\ninst ordinary:\n\tsin-osc freq > convolution ir: -1");
    let registry = ev.insts().unwrap();
    let r = registry.borrow();
    assert_eq!(r.resource_capture_mode(), GraphResourceCaptureMode::Legacy);
    let decl = r
        .inst_resources(r.id_of(intern_kw("events")).unwrap())
        .unwrap();
    assert_eq!(decl.resources().len(), 1);
    assert!(matches!(
        decl.resources()[0].site(),
        GraphResourceSite::Header { .. }
    ));
    assert!(r
        .inst_resources(r.id_of(intern_kw("ordinary")).unwrap())
        .unwrap()
        .resources()
        .is_empty());
    for name in ["sampler", "wavetable", "granular"] {
        let declaration = r.inst_resources(r.id_of(intern_kw(name)).unwrap()).unwrap();
        assert_eq!(
            declaration.resources().len(),
            1,
            "{name} retains only its resource header"
        );
        assert!(matches!(
            declaration.resources()[0].site(),
            GraphResourceSite::Header { .. }
        ));
    }
}
#[test]
fn unresolved_fixed_inputs_never_issue_bank_authority() {
    for (value, expected) in [
        (UGenInput::Const(0.), GraphResourceInput::UnresolvedNumeric),
        (
            UGenInput::Param(CtlId::new(0)),
            GraphResourceInput::UnresolvedDynamic,
        ),
    ] {
        let (_, extras) = lower(
            &ir(oscillator(), value),
            GraphResourceCaptureMode::ClosedSong,
        );
        assert_eq!(extras.resources.len(), 1);
        assert_eq!(extras.resources[0].input(), &expected);
    }
    let args = std::iter::once((None, UGenInput::Node(oscillator())))
        .chain((0..NODE_CAP + vactr::dsp::ugen::MAX_PARAMS + 1).map(|_| {
            (
                Some(intern_kw("ir")),
                UGenInput::Keyword(intern_kw("bounded")),
            )
        }))
        .collect();
    let root = node(UGenKind::Effect(EffectKind::Convolution), args);
    let caps = CapabilitySet::native();
    let mut alloc = || None;
    assert!(lower_inst_with_resources(
        InstId::new(1),
        &root,
        &[],
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc
        },
        GraphResourceCaptureMode::ClosedSong
    )
    .is_err());
}

#[test]
fn resource_header_bound_and_sparse_dynamic_input_are_admitted_honestly() {
    let caps = CapabilitySet::native();
    let root = oscillator();
    let mut alloc = || None;
    let mut header =
        vec![(CtlId::new(0), vactr::host::wire::Ctl::Const(0.)); vactr::dsp::ugen::MAX_PARAMS];
    let (def, extras) = lower_inst_with_resources(
        InstId::new(1),
        &root,
        &header,
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc,
        },
        GraphResourceCaptureMode::ClosedSong,
    )
    .unwrap();
    assert_eq!(def.params.len(), header.len());
    assert!(
        extras.resources.is_empty(),
        "numeric header data is not bank authority"
    );
    header.push((CtlId::new(u16::MAX), vactr::host::wire::Ctl::Const(0.)));
    assert!(lower_inst_with_resources(
        InstId::new(1),
        &root,
        &header,
        Lowering {
            caps: &caps,
            span: Span::new(FileId::CONSOLE, 0, 0),
            alloc: &mut alloc
        },
        GraphResourceCaptureMode::ClosedSong
    )
    .is_err());
    let (_, extras) = lower(
        &ir(oscillator(), UGenInput::Param(CtlId::new(u16::MAX))),
        GraphResourceCaptureMode::ClosedSong,
    );
    assert_eq!(extras.resources.len(), 1);
    assert_eq!(
        extras.resources[0].input(),
        &GraphResourceInput::UnresolvedDynamic
    );
}
