//! Source descriptor contracts and canonical-context admission.
use std::collections::BTreeMap;
use std::rc::Rc;
use vactr::ns::namespace::VarSlotRef;
use vactr::pattern::combinators::input::{input_lane_walk, midi_notes};
use vactr::pattern::eval::{InputCells, QueryCtx, QueryVm};
use vactr::pattern::pat::{Pat, PatNode};
use vactr::pattern::query::{query, query_traced, TimeSpan};
use vactr::pattern::step::Step;
use vactr::song::{capture_part, InstrumentRoute, InstrumentSelector, Part, PartEdit, SongSource};
use vactr::value::intern::intern_kw;
use vactr::value::ratio::Ratio64;
use vactr::value::sample::SampleBuf;
use vactr::value::value::{Sound, Value};
use vactr::vm::fail::{FailCode, Failure, Origin};

fn base() -> Rc<Part> {
    let pat = Rc::new(Pat::new(
        PatNode::Pure(Step::bare(Value::Int(3))),
        None,
        false,
    ));
    Rc::new(capture_part(BTreeMap::from([(intern_kw("drums"), pat)]), Ratio64::ONE).unwrap())
}
fn family(name: &str) -> InstrumentSelector {
    InstrumentSelector::new(vec![Sound::Builtin(intern_kw(name))]).unwrap()
}
fn source(part: Rc<Part>, selector: InstrumentSelector) -> Rc<SongSource> {
    Rc::new(SongSource::new(part, intern_kw("drums"), selector).unwrap())
}
fn pattern(source: Rc<SongSource>) -> Pat {
    Pat::new(PatNode::SongSource(source), None, true)
}

#[derive(Default)]
struct TestVm {
    calls: usize,
}
impl QueryVm for TestVm {
    fn call(&mut self, _: &Value, _: &[Value]) -> Result<Value, Failure> {
        self.calls += 1;
        Err(Failure::new(FailCode::Type, "unexpected source callback"))
    }
    fn deref(&mut self, _: &VarSlotRef) -> Result<Value, Failure> {
        self.calls += 1;
        Err(Failure::new(FailCode::Type, "unexpected source read"))
    }
    fn take_output(&mut self) -> Vec<(Origin, Rc<str>)> {
        Vec::new()
    }
    fn put_output(&mut self, _: Vec<(Origin, Rc<str>)>) {}
    fn sound_kit(&mut self) -> Result<Value, Failure> {
        self.calls += 1;
        Err(Failure::new(FailCode::Type, "unexpected source kit read"))
    }
}

#[test]
fn descriptor_rejects_missing_tracks_and_retains_immutable_source() {
    let part = base();
    let selector = family("bd");
    assert_eq!(
        SongSource::new(Rc::clone(&part), intern_kw("hats"), selector.clone())
            .unwrap_err()
            .code,
        FailCode::UnknownField
    );
    let selected = source(Rc::clone(&part), selector.clone());
    assert!(Rc::ptr_eq(selected.part(), &part));
    assert_eq!(selected.track(), intern_kw("drums"));
    assert_eq!(selected.selector(), &selector);
    let edited = Rc::clone(&part)
        .edit(PartEdit::InstrumentFx {
            track: intern_kw("drums"),
            selector,
            template: intern_kw("dark"),
        })
        .unwrap();
    assert_ne!(edited.revision(), selected.part().revision());
    assert_eq!(selected.part().revision(), part.revision());
}

#[test]
fn complete_families_and_typed_routes_use_full_identity() {
    let bd = Sound::Builtin(intern_kw("bd"));
    let sd = Sound::Builtin(intern_kw("sd"));
    let selector = InstrumentSelector::new(vec![bd.clone(), sd.clone(), bd.clone()]).unwrap();
    assert_eq!(selector.family(), &[bd, sd]);
    let selected = source(base(), selector.clone());
    let route = InstrumentRoute {
        family: selector,
        template: intern_kw("dark"),
    };
    assert_eq!(&route.family, selected.selector());
    assert_eq!(route, route.clone());
    assert_ne!(
        route,
        InstrumentRoute {
            family: family("bd"),
            template: route.template
        }
    );
    assert_ne!(
        route,
        InstrumentRoute {
            family: route.family.clone(),
            template: intern_kw("bright")
        }
    );
}

#[test]
fn selected_sources_have_no_live_input_lane() {
    let selected = pattern(source(base(), family("bd")));
    assert!(input_lane_walk(&selected).unwrap().lanes.is_empty());
    let sound = Rc::new(Pat::new(
        PatNode::Pure(Step::bare(Value::kw("bd"))),
        None,
        false,
    ));
    let live = midi_notes(sound, Some(1), None);
    let mixed = Pat::new(
        PatNode::Stack(vec![selected, live].into_boxed_slice()),
        None,
        true,
    );
    assert_eq!(input_lane_walk(&mixed).unwrap().lanes.len(), 1);
}

#[test]
fn source_without_canonical_song_context_is_an_explicit_type_fault() {
    let selected = pattern(source(base(), family("bd")));
    let cells = InputCells::new();
    for traced in [false, true] {
        let mut vm = TestVm::default();
        let mut cx = QueryCtx::new(&mut vm, &cells, 17);
        let result = if traced {
            query_traced(&selected, TimeSpan::cycle(0).unwrap(), &mut cx)
        } else {
            query(&selected, TimeSpan::cycle(0).unwrap(), &mut cx)
        };
        assert!(result.events.is_empty());
        assert_eq!(result.faults.len(), 1);
        assert_eq!(result.faults[0].code, FailCode::Type);
        assert!(result.faults[0].message.contains("song query context"));
        assert_eq!(vm.calls, 0);
    }
}

#[test]
fn source_hash_excludes_revisions_and_rc_allocation_addresses() {
    let first = base();
    let rebuilt = base();
    assert_ne!(first.revision(), rebuilt.revision());
    let edited = Rc::new(
        Rc::clone(&first)
            .edit(PartEdit::InstrumentFx {
                track: intern_kw("drums"),
                selector: family("bd"),
                template: intern_kw("dark"),
            })
            .unwrap(),
    );
    let original = pattern(source(first, family("bd")));
    assert_eq!(original.id, pattern(source(rebuilt, family("bd"))).id);
    assert_eq!(original.id, pattern(source(edited, family("bd"))).id);
    assert_ne!(original.id, pattern(source(base(), family("sd"))).id);
}

#[test]
fn opaque_buffer_hashes_do_not_forge_family_or_route_identity() {
    let first = SampleBuf::ready(48_000, vec![0.0_f32, 0.0]);
    let second = SampleBuf::ready(48_000, vec![0.0_f32, 0.0]);
    let a = InstrumentSelector::new(vec![Sound::Buffer(Rc::clone(&first))]).unwrap();
    let b = InstrumentSelector::new(vec![Sound::Buffer(second)]).unwrap();
    assert_ne!(a, b);
    let x = pattern(source(base(), a.clone()));
    let y = pattern(source(base(), b.clone()));
    assert_eq!(x.id, y.id, "seed fingerprints are not certified identity");
    assert_ne!(
        InstrumentRoute {
            family: a,
            template: intern_kw("dark")
        },
        InstrumentRoute {
            family: b,
            template: intern_kw("dark")
        }
    );
    first.fill(vec![1.0_f32, 1.0]);
    assert_eq!(
        x.id,
        Pat::new(x.node.clone(), None, true).id,
        "buffer state cannot reroll source seeds"
    );
}

#[test]
fn legacy_patterns_still_realize_alongside_the_explicit_source_fault() {
    let old = Pat::new(PatNode::Pure(Step::bare(Value::Int(7))), None, false);
    let stacked = Pat::new(
        PatNode::Stack(vec![old, pattern(source(base(), family("bd")))].into_boxed_slice()),
        None,
        true,
    );
    let cells = InputCells::new();
    let mut vm = TestVm::default();
    let mut cx = QueryCtx::new(&mut vm, &cells, 1);
    let result = query(&stacked, TimeSpan::cycle(0).unwrap(), &mut cx);
    assert_eq!(result.events.len(), 1);
    assert_eq!(result.events[0].value.to_string(), "7");
    assert_eq!(result.faults.len(), 1);
}

#[test]
fn actual_canonical_context_realizes_public_source_origin_without_callback_or_live_reads() {
    use vactr::clock::tempo::Tempo;
    use vactr::song::{query_part, SongLimits, SongQueryCtx};
    let audio = Pat::new(
        PatNode::Pure(Step::bare(Value::Sound(Rc::new(Sound::Builtin(
            intern_kw("bd"),
        ))))),
        None,
        false,
    );
    let original = Rc::new(
        capture_part(
            BTreeMap::from([(intern_kw("drums"), Rc::new(audio))]),
            Ratio64::ONE,
        )
        .unwrap(),
    );
    let selected = pattern(source(original.clone(), family("bd")));
    let parent = capture_part(
        BTreeMap::from([(intern_kw("drums"), Rc::new(selected))]),
        Ratio64::ONE,
    )
    .unwrap();
    let mut vm = TestVm::default();
    let cells = InputCells::new();
    let limits = SongLimits::default();
    let rows = query_part(
        &parent,
        TimeSpan::cycle(0).unwrap(),
        &mut SongQueryCtx {
            vm: &mut vm,
            cells: &cells,
            seed: 17,
            tempo: Tempo::default(),
            limits: &limits,
        },
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].instrument, Sound::Builtin(intern_kw("bd")));
    assert_eq!(
        rows[0]
            .event
            .song_source
            .as_ref()
            .unwrap()
            .handle
            .revision(),
        original.revision()
    );
    assert_eq!(vm.calls, 0);
}
