use super::*;
use crate::song::assets::{DecodedSongAssetFactory, SongAssetFactory, SongSourceFile};

fn limits() -> SongAssetLimits {
    SongAssetLimits {
        max_resources: 8,
        max_pcm_bytes: 1024,
        max_source_files: 8,
        max_source_bytes: 1024,
        max_banks: 8,
        max_walk_nodes: 100_000,
        max_walk_depth: 256,
    }
}
fn assets() -> PinnedSongAssets {
    DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new())
        .begin(
            SongSourceFile {
                file: crate::reader::span::FileId::new(0),
                path: crate::value::value::PathVal {
                    text: "score.vact".into(),
                    file: None,
                },
            },
            limits(),
        )
        .unwrap()
        .close()
        .unwrap()
}
fn wrap(mut pattern: Rc<Pat>, count: usize) -> Rc<Pat> {
    for _ in 0..count {
        pattern = Rc::new(Pat::new(PatNode::Fast(pattern, PParam::int(1)), None, true));
    }
    pattern
}
#[test]
fn cached_height_respects_new_parent_depth_and_cleanup() {
    let assets = assets();
    let mut freeze = Freeze::new(&assets, limits());
    let child = wrap(Rc::new(Pat::silence()), 200);
    let copied = freeze.copy_pattern(&child).unwrap();
    assert_eq!(copied.id, child.id);
    assert!(Rc::ptr_eq(&copied, &freeze.copy_pattern(&child).unwrap()));
    let deep_alias = wrap(child.clone(), 100);
    assert_eq!(
        freeze.copy_pattern(&deep_alias).unwrap_err().code,
        FailCode::DepthExceeded
    );
    assert_eq!(freeze.depth, 0);
    assert!(freeze.visiting.is_empty());
    assert!(Rc::ptr_eq(&copied, &freeze.copy_pattern(&child).unwrap()));
    // The same admitted alias fits when its complete height fits the caller.
    assert!(freeze.copy_pattern(&wrap(child, 20)).is_ok());
}
#[test]
fn aggregate_cache_height_cannot_hide_a_deep_pattern_alias() {
    fn list(value: Value) -> Value {
        Value::List(Rc::new(ListVal {
            items: vec![value].into(),
            prov: None,
        }))
    }
    let assets = assets();
    let pattern = wrap(Rc::new(Pat::silence()), 100);
    let child = list(Value::Pattern(pattern));
    let mut freeze = Freeze::new(&assets, limits());
    freeze.value(&child).unwrap();
    let mut alias = child.clone();
    for _ in 0..160 {
        alias = list(alias);
    }
    assert_eq!(
        freeze.value(&alias).unwrap_err().code,
        FailCode::DepthExceeded
    );
    assert_eq!(freeze.depth, 0);
    assert!(freeze.visiting.is_empty());
    let mut allowed = child;
    for _ in 0..20 {
        allowed = list(allowed);
    }
    assert!(freeze.value(&allowed).is_ok());
}
#[test]
fn cached_part_and_prototype_retain_descendant_heights() {
    let assets = assets();
    let pattern = wrap(Rc::new(Pat::silence()), 100);
    let part = Rc::new(
        crate::song::capture_part(
            BTreeMap::from([(crate::value::intern::intern_kw("drums"), pattern.clone())]),
            crate::value::Ratio64::ONE,
        )
        .unwrap(),
    );
    let proto = Rc::new(FnProto {
        arity: Default::default(),
        code: Vec::new(),
        consts: vec![Value::Pattern(pattern)],
        locals: 0,
        spans: Vec::new(),
        name: None,
        globals: Vec::new(),
        defs: Vec::new(),
        form_gen: crate::ns::namespace::FormGen::new(0),
        protos: Vec::new(),
        masks: Vec::new(),
        call_sites: Vec::new(),
        list_sites: Vec::new(),
        shapes: Vec::new(),
        captures: 0,
        span: crate::reader::span::Span::new(crate::reader::span::FileId::CONSOLE, 0, 0),
        ctor: None,
    });
    let mut freeze = Freeze::new(&assets, limits());
    let copied = freeze.proto(&proto).unwrap();
    assert!(Rc::ptr_eq(&copied, &freeze.proto(&proto).unwrap()));
    freeze.depth = 160;
    assert_eq!(
        freeze.proto(&proto).unwrap_err().code,
        FailCode::DepthExceeded
    );
    assert_eq!(freeze.depth, 160);
    freeze.depth = 0;
    freeze.value(&Value::Part(part.clone())).unwrap();
    freeze.depth = 160;
    assert_eq!(
        freeze.value(&Value::Part(part)).unwrap_err().code,
        FailCode::DepthExceeded
    );
    assert_eq!(freeze.depth, 160);
    freeze.depth = 0;
    assert!(freeze.visiting.is_empty());
}
#[test]
fn spine_preserves_fields_params_and_admits_storage_before_growth() {
    let assets = assets();
    let leaf = Rc::new(Pat::silence());
    let original = Rc::new(Pat::new(PatNode::Slow(leaf, PParam::int(3)), None, true));
    let mut freeze = Freeze::new(&assets, limits());
    let copied = freeze.copy_pattern(&original).unwrap();
    assert_eq!(
        (copied.id, copied.span, copied.structured),
        (original.id, original.span, original.structured)
    );
    assert!(matches!(
        &copied.node,
        PatNode::Slow(_, PParam::Const(Value::Int(3)))
    ));
    let mut tiny = limits();
    tiny.max_walk_nodes = 1;
    let mut freeze = Freeze::new(&assets, tiny);
    assert_eq!(
        freeze.copy_pattern(&original).unwrap_err().code,
        FailCode::FuelExhausted
    );
    assert_eq!(freeze.depth, 0);
    assert!(freeze.visiting.is_empty());
}

fn work_pair() -> Value {
    Value::List(Rc::new(ListVal {
        items: vec![Value::Int(11), Value::Int(22)].into(),
        prov: None,
    }))
}
fn work_proto() -> Rc<FnProto> {
    use crate::compile::proto::{ArgKind, Arity, CallSite, ItemKind, ListSite, Shape};
    use crate::dsp::controls::ScalarType;
    use crate::reader::span::{FileId, Span};
    use crate::types::masks::{CalleeRef, ForcingMask, MaskEntry};
    use crate::value::intern::{intern_kw, intern_sym};
    use crate::vm::ops::Op;
    let span = Span::new(FileId::CONSOLE, 0, 1);
    let child = Rc::new(FnProto {
        arity: Arity::fixed(0),
        code: vec![],
        consts: vec![],
        locals: 0,
        spans: vec![],
        name: None,
        globals: vec![],
        defs: vec![],
        form_gen: crate::ns::namespace::FormGen::new(0),
        protos: vec![],
        masks: vec![],
        call_sites: vec![],
        list_sites: vec![],
        shapes: vec![],
        captures: 0,
        span,
        ctor: None,
    });
    Rc::new(FnProto {
        arity: Arity {
            fixed: 1,
            names: vec![intern_kw("x")].into(),
            scalar_types: vec![ScalarType::Float].into(),
            keys: 0,
        },
        code: vec![Op::LoadConst(0), Op::Ret],
        consts: vec![Value::Int(7)],
        locals: 1,
        spans: vec![(0, span), (1, span)],
        name: Some(intern_sym("copy-work")),
        globals: vec![],
        defs: vec![],
        form_gen: crate::ns::namespace::FormGen::new(0),
        protos: vec![child],
        masks: vec![ForcingMask(
            vec![
                MaskEntry::Value,
                MaskEntry::Forward {
                    links: vec![
                        (CalleeRef::Global("f".into()), 0),
                        (CalleeRef::Global("g".into()), 1),
                    ]
                    .into(),
                },
            ]
            .into(),
        )],
        call_sites: vec![CallSite {
            args: vec![ArgKind::Pos, ArgKind::Pair].into(),
        }],
        list_sites: vec![ListSite {
            items: vec![ItemKind::Item, ItemKind::Splat, ItemKind::Item].into(),
            prov: None,
        }],
        shapes: vec![Shape {
            ty: intern_sym("Point"),
            tag: intern_sym("Point"),
            fields: vec![intern_kw("x"), intern_kw("y")].into(),
            is_struct: true,
        }],
        captures: 0,
        span,
        ctor: None,
    })
}
#[test]
fn empty_and_cached_list_report_actual_work() {
    let assets = assets();
    let mut freeze = Freeze::new(&assets, limits());
    assert_eq!(freeze.consumed_work(), 0);
    let original = work_pair();
    let Value::List(first) = freeze.value(&original).unwrap() else {
        panic!("list")
    };
    assert_eq!(freeze.consumed_work(), 5);
    assert_eq!(freeze.consumed_work(), 5, "getter never charges or resets");
    let Value::List(second) = freeze.value(&original).unwrap() else {
        panic!("list")
    };
    assert!(Rc::ptr_eq(&first, &second));
    assert_eq!(
        freeze.consumed_work(),
        6,
        "cached root still admits one visit"
    );
}
#[test]
fn prototype_payload_work_includes_full_copy_arrays() {
    let assets = assets();
    let proto = work_proto();
    let payload = super::super::prototype_copy_work(&proto).unwrap();
    assert_eq!(
        payload, 23,
        "includes nested sites, masks and forward links"
    );
    let mut freeze = Freeze::new(&assets, limits());
    let copied = freeze.proto(&proto).unwrap();
    assert_eq!(
        freeze.consumed_work(),
        26,
        "root + payload23 + const + child"
    );
    assert_eq!(copied.code, proto.code);
    assert_eq!(copied.spans, proto.spans);
    assert_eq!(copied.masks, proto.masks);
    assert_eq!(copied.call_sites, proto.call_sites);
    assert_eq!(copied.list_sites[0].items, proto.list_sites[0].items);
    assert_eq!(copied.shapes, proto.shapes);
    let closure = Value::Fn(Rc::new(Closure {
        proto: Rc::clone(&proto),
        captures: Box::new([]),
        mask: Default::default(),
        memo: None,
    }));
    let deps = crate::song::assets::song_asset_dependencies(&[closure], limits()).unwrap();
    assert!(
        deps.consumed_work() < freeze.consumed_work(),
        "dependency visits omit copied arrays"
    );
    assert!(Rc::ptr_eq(&copied, &freeze.proto(&proto).unwrap()));
    assert_eq!(freeze.consumed_work(), 27);
}
#[test]
fn exact_work_quota_and_one_less_preserve_admission() {
    let assets = assets();
    let original = work_pair();
    let mut exact = limits();
    exact.max_walk_nodes = 5;
    let mut freeze = Freeze::new(&assets, exact);
    assert!(freeze.value(&original).is_ok());
    assert_eq!(freeze.consumed_work(), 5);
    let mut short = exact;
    short.max_walk_nodes = 4;
    let mut failed = Freeze::new(&assets, short);
    assert_eq!(
        failed.value(&original).unwrap_err().code,
        FailCode::FuelExhausted
    );
    assert_eq!(
        failed.consumed_work(),
        5,
        "failed attempted charge is not refunded"
    );
    assert!(failed.copied.is_empty());
    assert!(failed.visiting.is_empty());
    assert_eq!(failed.depth, 0);
}
#[test]
fn collection_refusal_precedes_child_clone() {
    let assets = assets();
    let child = Rc::new(ListVal {
        items: vec![Value::Int(1)].into(),
        prov: None,
    });
    let root = Value::List(Rc::new(ListVal {
        items: (0..4096).map(|_| Value::List(Rc::clone(&child))).collect(),
        prov: None,
    }));
    let count = Rc::strong_count(&child);
    let mut tiny = limits();
    tiny.max_walk_nodes = 1;
    let mut freeze = Freeze::new(&assets, tiny);
    assert_eq!(
        freeze.value(&root).unwrap_err().code,
        FailCode::FuelExhausted
    );
    assert_eq!(freeze.consumed_work(), 4097);
    assert_eq!(Rc::strong_count(&child), count);
    assert!(freeze.copied.is_empty());
    assert!(freeze.visiting.is_empty());
    assert_eq!(freeze.depth, 0);
}
#[test]
fn checked_work_overflow_never_wraps_or_copies() {
    let assets = assets();
    let mut freeze = Freeze::new(&assets, limits());
    freeze.work = u32::MAX;
    let error = freeze.value(&work_pair()).unwrap_err();
    assert_eq!(
        error.code,
        FailCode::HostUnavailable,
        "existing freeze overflow classification"
    );
    assert_eq!(error.message, "freeze work overflow");
    assert_eq!(freeze.consumed_work(), u32::MAX);
    assert!(freeze.copied.is_empty());
    assert!(freeze.visiting.is_empty());
    assert_eq!(freeze.depth, 0);
}
