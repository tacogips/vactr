//! Actual original-snapshot q reuse, including fractional placement clocks.
use super::{
    retain_index_occupancy, tests::from_code, tests::request_window, CanonicalIndexRequest,
};
use crate::ns::namespace::VarSlotRef;
use crate::pattern::{eval::song_replay::ReplayView, query::TimeSpan};
use crate::song::{
    snapshot::FrozenSongEvent, source_uses::FrozenUseOperation, SongLimits, SongSnapshot,
};
use crate::value::{
    intern::{intern_kw, intern_sym},
    Ratio64,
};
use crate::vm::{
    fail::{FailCode, Failure},
    vm::ReadObserver,
};
use std::{cell::Cell, rc::Rc};

struct CutReads {
    count: Rc<Cell<u32>>,
    deny: bool,
}
impl ReadObserver for CutReads {
    fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
        if slot.name() == intern_sym("cut") {
            self.count.set(self.count.get() + 1);
            if self.deny {
                return Err(Failure::new(
                    FailCode::HostUnavailable,
                    "genuine callback read is forbidden after retention",
                ));
            }
        }
        Ok(())
    }
}
fn watch(snapshot: &mut SongSnapshot, deny: bool) -> Rc<Cell<u32>> {
    let count = Rc::new(Cell::new(0));
    snapshot
        .evaluator
        .vm_and_ns()
        .0
        .set_read_observer(Some(Box::new(CutReads {
            count: count.clone(),
            deny,
        })));
    count
}
fn span(a: i64, b: i64, denominator: i64) -> TimeSpan {
    TimeSpan::new(
        Ratio64::new(a, denominator).unwrap(),
        Ratio64::new(b, denominator).unwrap(),
    )
    .unwrap()
}
fn program(intro: bool, parallel: bool, subject: &str, ending: &str) -> String {
    let pattern = if parallel {
        format!("stack [{{slice {{beat -> {subject}}} 2 [cut nil]}} {{slice {{beat -> {subject}}} 2 [cut nil]}}]")
    } else {
        format!("slice {{beat -> {subject}}} 2 [cut nil]")
    };
    let intro = if intro {
        "let intro {part [drums: nil] duration: 1/2}\nlet arrangement {sequence [intro selected]}"
    } else {
        "let arrangement selected"
    };
    format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\t{pattern}\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog indexed}}\n{intro}\n{ending}")
}
fn single(snapshot: &SongSnapshot, window: TimeSpan, remaining: &mut u32) -> CanonicalIndexRequest {
    request_window(snapshot, 0, window, SongLimits::default(), remaining)
}
fn all_addresses(
    snapshot: &SongSnapshot,
    window: TimeSpan,
    remaining: &mut u32,
) -> Vec<CanonicalIndexRequest> {
    let scope = snapshot
        .routing
        .parts
        .iter()
        .rposition(|part| {
            matches!(
                part.node,
                crate::song::snapshot::FrozenPartNode::Edit {
                    edit: crate::song::snapshot::FrozenEdit::Transform { .. },
                    ..
                }
            )
        })
        .unwrap();
    let payload = match &snapshot.routing.parts[scope].node {
        crate::song::snapshot::FrozenPartNode::Edit {
            edit: crate::song::snapshot::FrozenEdit::Transform { payload, .. },
            ..
        } => payload,
        _ => unreachable!(),
    };
    let recipe = payload.index_timing().unwrap();
    let mut pending = vec![(recipe.root(), Vec::new())];
    let mut result = Vec::new();
    while let Some((node, path)) = pending.pop() {
        let node = &recipe.nodes()[node as usize];
        if node.operation() == FrozenUseOperation::Slice {
            result.push(
                snapshot
                    .canonical_index_request(
                        scope,
                        intern_kw("drums"),
                        node.issuer(),
                        &path,
                        window,
                        0,
                        SongLimits::default(),
                        remaining,
                    )
                    .unwrap(),
            );
        } else {
            for child in node.children() {
                let mut child_path = path.clone();
                child_path.extend_from_slice(child.trace());
                pending.push((child.child(), child_path));
            }
        }
    }
    result
}
type TestOwner = (
    crate::song::PartRevision,
    crate::value::intern::KwId,
    crate::reader::span::NodeId,
);
fn target(request: &CanonicalIndexRequest) -> TestOwner {
    (request.revision, request.track, request.root)
}
fn target_records(
    snapshot: &SongSnapshot,
    target: TestOwner,
) -> Vec<&Rc<crate::pattern::eval::song_replay::OwnerCycleExecution>> {
    snapshot
        .replay
        .as_ref()
        .unwrap()
        .executions()
        .iter()
        .filter(|record| {
            let owner = record.owner();
            (owner.revision, owner.track, owner.root) == target
        })
        .collect()
}
/// Independently inspect the frozen selected-source DAG, not raw event family
/// or the production permission list. Every extra execution must have an exact
/// immutable original payload owner in this ancestry.
fn assert_execution_graph(
    snapshot: &SongSnapshot,
    scope: usize,
    track: crate::value::intern::KwId,
) {
    use crate::song::snapshot::{FrozenEdit, FrozenPartNode};
    let mut pending = vec![(scope, track)];
    let mut visited = Vec::new();
    let mut owners = Vec::new();
    while let Some((index, track)) = pending.pop() {
        if visited.contains(&(index, track)) {
            continue;
        }
        visited.push((index, track));
        let part = &snapshot.routing.parts[index];
        let payload = match &part.node {
            FrozenPartNode::Capture(tracks) => tracks
                .iter()
                .find(|(name, _)| *name == track)
                .map(|(_, p)| p),
            FrozenPartNode::Sequence(children) => {
                pending.extend(children.iter().map(|(_, child)| (*child, track)));
                None
            }
            FrozenPartNode::Repeat { child, count, .. } => {
                if *count != 0 {
                    pending.push((*child, track));
                }
                None
            }
            FrozenPartNode::Edit { source, edit } => {
                if !matches!(edit, FrozenEdit::Replace { track: replaced, .. } if *replaced == track)
                {
                    pending.push((*source, track));
                }
                match edit {
                    FrozenEdit::Replace {
                        track: selected,
                        payload,
                    }
                    | FrozenEdit::Transform {
                        track: selected,
                        payload,
                        ..
                    }
                    | FrozenEdit::Overwrite {
                        track: selected,
                        payload,
                        ..
                    } if *selected == track => Some(payload),
                    _ => None,
                }
            }
        };
        if let Some(payload) = payload {
            owners.push(((part.revision, track, payload.id), part.duration));
            pending.extend(
                payload
                    .sources
                    .iter()
                    .map(|source| (source.root_part, source.track)),
            );
        }
    }
    let records = snapshot.replay.as_ref().unwrap().executions();
    for (index, record) in records.iter().enumerate() {
        let owner = record.owner();
        assert!(
            owners.contains(&((owner.revision, owner.track, owner.root), owner.duration)),
            "execution is an exact frozen source ancestor: {:?}",
            owner
        );
        assert_eq!(
            owner.window.end.checked_sub(owner.window.begin).unwrap(),
            Ratio64::ONE
        );
        assert!(record.entry_depth() <= SongLimits::default().max_depth);
        if snapshot.routing.parts.iter().any(|part| {
            part.revision == owner.revision && matches!(part.node, FrozenPartNode::Capture(_))
        }) {
            assert!(
                record.calls().iter().any(|call| matches!(&call.callable,
                crate::value::Value::VarRef(slot) if slot.name() == intern_sym("sound-kit"))),
                "genuine base Capture records retain actual sound-kit dereference"
            );
        }
        for prior in &records[..index] {
            assert!(
                prior.owner() != owner
                    || prior.seed() != record.seed()
                    || prior.producer_entry() != record.producer_entry()
                    || prior.entry_depth() != record.entry_depth(),
                "no duplicated complete original execution context"
            );
        }
    }
}
fn cuts(view: &ReplayView) -> usize {
    view.executions()
        .iter()
        .flat_map(|execution| execution.calls())
        .filter(|call| {
            matches!(&call.callable,
        crate::value::Value::Fn(f) if f.proto.name == Some(intern_sym("cut")))
        })
        .count()
}
fn compare(snapshot: &mut SongSnapshot, expected: &[FrozenSongEvent], window: TimeSpan) {
    let rows = snapshot.query(window, &SongLimits::default()).unwrap();
    assert_eq!(
        rows, expected,
        "full handle, source origins/timings, route, notes, controls and spans"
    );
}

#[test]
fn half_cycle_owner_executes_once_across_root_windows_and_reordered_queries() {
    let code = program(
        true,
        false,
        "p",
        "song arrangement tail-seconds: 0 > play-song",
    );
    let mut song = from_code(&code);
    let snapshot = &mut song.snapshot;
    let full = span(0, 2, 1);
    let windows = [span(3, 4, 4), span(1, 2, 2), span(3, 4, 2)];
    let reads = watch(snapshot, false);
    let expected = snapshot.query(full, &SongLimits::default()).unwrap();
    assert_eq!(
        expected.len(),
        4,
        "two original chord tones at each of two onsets"
    );
    let pieces: Vec<_> = windows
        .iter()
        .map(|window| snapshot.query(*window, &SongLimits::default()).unwrap())
        .collect();
    assert!(pieces.iter().all(|rows| rows.len() == 2));
    assert!(reads.get() > 0, "ordinary original q really reads callback");
    snapshot.evaluator.vm_and_ns().0.set_read_observer(None);
    let mut remaining = SongLimits::default().max_nodes;
    let requests = vec![
        single(snapshot, TimeSpan::cycle(0).unwrap(), &mut remaining),
        single(snapshot, TimeSpan::cycle(1).unwrap(), &mut remaining),
    ];
    let owner = target(&requests[0]);
    let scope = requests[0].scope;
    let actual_reads = watch(snapshot, false);
    retain_index_occupancy(snapshot, requests, SongLimits::default(), &mut remaining).unwrap();
    assert_eq!(
        actual_reads.get(),
        2,
        "actual target cut reads, distinct from copied journals"
    );
    assert_execution_graph(snapshot, scope, owner.1);
    let target_rows = target_records(snapshot, owner);
    let view = snapshot.replay.as_ref().unwrap();
    assert_eq!(
        target_rows.len(),
        2,
        "unique TARGET owner-local cycles0/1, not three root-window visits"
    );
    assert_eq!(
        target_rows
            .iter()
            .map(|record| record.owner().window)
            .collect::<Vec<_>>(),
        vec![TimeSpan::cycle(0).unwrap(), TimeSpan::cycle(1).unwrap()]
    );
    assert_eq!(
        cuts(view),
        2,
        "original execution journals; copied addressed journals are not invocations"
    );
    assert!(target_rows
        .iter()
        .all(|record| record.owner().offset == Ratio64::new(1, 2).unwrap()));
    let blocked = watch(snapshot, true);
    compare(snapshot, &expected, full);
    for index in [2, 0, 1, 2] {
        compare(snapshot, &pieces[index], windows[index]);
    }
    assert_eq!(
        blocked.get(),
        0,
        "required output replay never reads callback"
    );
}

#[test]
fn parallel_slice_addresses_share_original_execution_and_raw_identity() {
    let code = program(
        false,
        true,
        "p",
        "song arrangement tail-seconds: 0 > play-song",
    );
    let mut song = from_code(&code);
    let snapshot = &mut song.snapshot;
    let window = TimeSpan::cycle(0).unwrap();
    let expected = snapshot.query(window, &SongLimits::default()).unwrap();
    assert_eq!(expected.len(), 4);
    let mut remaining = SongLimits::default().max_nodes;
    let requests = all_addresses(snapshot, window, &mut remaining);
    assert_eq!(requests.len(), 2, "two genuine issued Slice addresses");
    assert_ne!(requests[0].prefix, requests[1].prefix);
    let owner = target(&requests[0]);
    let scope = requests[0].scope;
    let actual_reads = watch(snapshot, false);
    retain_index_occupancy(snapshot, requests, SongLimits::default(), &mut remaining).unwrap();
    assert_eq!(
        actual_reads.get(),
        2,
        "actual target cut reads, distinct from copied journals"
    );
    assert_execution_graph(snapshot, scope, owner.1);
    assert_eq!(target_records(snapshot, owner).len(), 1);
    assert_eq!(
        cuts(snapshot.replay.as_ref().unwrap()),
        2,
        "one q evaluates its two distinct Slice index sites"
    );
    assert_eq!(snapshot.occupancy[0].observations.len(), 2);
    let reads = watch(snapshot, true);
    compare(snapshot, &expected, window);
    assert_eq!(reads.get(), 0);
}

#[test]
fn required_missing_local_cycle_fails_before_callback_and_foreign_view_refuses() {
    let code = program(
        false,
        false,
        "p",
        "song arrangement tail-seconds: 0 > play-song",
    );
    let mut first = from_code(&code);
    let mut remaining = SongLimits::default().max_nodes;
    let request = single(&first.snapshot, TimeSpan::cycle(0).unwrap(), &mut remaining);
    retain_index_occupancy(
        &mut first.snapshot,
        vec![request],
        SongLimits::default(),
        &mut remaining,
    )
    .unwrap();
    let reads = watch(&mut first.snapshot, true);
    let error = first
        .snapshot
        .query(TimeSpan::cycle(1).unwrap(), &SongLimits::default())
        .unwrap_err();
    assert_eq!(error.code, FailCode::Type);
    assert!(error.message.contains("owner-local execution missing"));
    assert_eq!(reads.get(), 0);
    let mut second = from_code(&code);
    // The original opaque view is genuinely issued, but foreign to this second
    // snapshot. Reusing it must fail, rather than matching same code/NodeIds.
    second.snapshot.replay = first.snapshot.replay.clone();
    assert_eq!(
        second
            .snapshot
            .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
            .unwrap_err()
            .code,
        FailCode::Type
    );
}

#[test]
fn empty_subject_retains_prefilter_index_without_ordinary_callback_replay() {
    let code = program(
        false,
        false,
        "nil",
        "song arrangement tail-seconds: 0 > play-song",
    );
    let mut song = from_code(&code);
    let mut remaining = SongLimits::default().max_nodes;
    let request = single(&song.snapshot, TimeSpan::cycle(0).unwrap(), &mut remaining);
    let owner = target(&request);
    let scope = request.scope;
    let actual_reads = watch(&mut song.snapshot, false);
    retain_index_occupancy(
        &mut song.snapshot,
        vec![request],
        SongLimits::default(),
        &mut remaining,
    )
    .unwrap();
    assert_eq!(
        actual_reads.get(),
        1,
        "actual target callback before subject suppression"
    );
    assert_eq!(song.snapshot.occupancy[0].observations.len(), 1);
    assert_execution_graph(&song.snapshot, scope, owner.1);
    assert_eq!(target_records(&song.snapshot, owner).len(), 1);
    let reads = watch(&mut song.snapshot, true);
    assert!(song
        .snapshot
        .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
        .unwrap()
        .is_empty());
    assert_eq!(reads.get(), 0);
}

#[test]
fn varying_repeat_seed_and_full_placement_are_distinct_execution_authority() {
    let code = program(
        false,
        false,
        "p",
        "song {part-repeat arrangement 2 seed-mode: :vary} tail-seconds: 0 > play-song",
    );
    let mut song = from_code(&code);
    let limits = SongLimits::default();
    let mut remaining = limits.max_nodes;
    let request = single(&song.snapshot, TimeSpan::cycle(0).unwrap(), &mut remaining);
    let owner = target(&request);
    let scope = request.scope;
    let actual_reads = watch(&mut song.snapshot, false);
    retain_index_occupancy(&mut song.snapshot, vec![request], limits, &mut remaining).unwrap();
    assert_eq!(
        actual_reads.get(),
        1,
        "one actual callback for this original varying placement"
    );
    let reads = watch(&mut song.snapshot, true);
    assert_eq!(
        song.snapshot
            .query(TimeSpan::cycle(2).unwrap(), &limits)
            .unwrap_err()
            .code,
        FailCode::Type
    );
    assert_eq!(
        reads.get(),
        0,
        "missing placement must not execute callback"
    );
    song.snapshot
        .evaluator
        .vm_and_ns()
        .0
        .set_read_observer(None);
    let request = single(&song.snapshot, TimeSpan::cycle(2).unwrap(), &mut remaining);
    let actual_reads = watch(&mut song.snapshot, false);
    retain_index_occupancy(&mut song.snapshot, vec![request], limits, &mut remaining).unwrap();
    assert_eq!(
        actual_reads.get(),
        1,
        "one actual callback for this original varying placement"
    );
    assert_execution_graph(&song.snapshot, scope, owner.1);
    let records = target_records(&song.snapshot, owner);
    assert_eq!(records.len(), 2);
    assert_eq!(cuts(song.snapshot.replay.as_ref().unwrap()), 2);
    assert_ne!(records[0].owner().placement, records[1].owner().placement);
    assert_ne!(records[0].seed(), records[1].seed());
    let reads = watch(&mut song.snapshot, true);
    for window in [TimeSpan::cycle(2).unwrap(), TimeSpan::cycle(0).unwrap()] {
        assert_eq!(song.snapshot.query(window, &limits).unwrap().len(), 2);
    }
    assert_eq!(reads.get(), 0);
}

#[test]
fn raw_replay_keeps_nested_source_origin_and_later_delete_overwrite_order() {
    let ending = "let rows {part-events arrangement :drums 0 1}\nlet handle {{first rows} :handle}\nlet removed {delete-event arrangement handle}\nlet changed {overwrite-region removed :drums 1/2 1 {s :analog > note 72}}\nsong changed tail-seconds: 0 > play-song";
    let mut song = from_code(&program(false, false, "p", ending));
    let limits = SongLimits::default();
    let window = TimeSpan::cycle(0).unwrap();
    let expected = song.snapshot.query(window, &limits).unwrap();
    assert_eq!(
        expected.len(),
        2,
        "one chord tone removed, later overwrite emits its own note"
    );
    assert_eq!(
        expected
            .iter()
            .filter(|row| row.source_origin.is_some())
            .count(),
        1
    );
    assert!(expected
        .iter()
        .any(|row| row.note == Some(crate::song::ResolvedNote::Int(72))));
    let mut remaining = limits.max_nodes;
    let request = single(&song.snapshot, window, &mut remaining);
    retain_index_occupancy(&mut song.snapshot, vec![request], limits, &mut remaining).unwrap();
    let reads = watch(&mut song.snapshot, true);
    compare(&mut song.snapshot, &expected, window);
    compare(&mut song.snapshot, &expected, window);
    assert_eq!(reads.get(), 0);
}

#[test]
fn actual_replay_copy_budget_and_conservative_vm_depth_cannot_be_waived() {
    let mut functions = String::from("fn deep0 beat:\n\tfirst [0]\n");
    for index in 1..=12 {
        functions.push_str(&format!("fn deep{index} beat:\n\tdeep{} beat\n", index - 1));
    }
    let code = program(
        false,
        false,
        "p",
        "song arrangement tail-seconds: 0 > play-song",
    )
    .replacen(
        "fn cut beat:\n\tfirst [0]",
        &format!("{functions}fn cut beat:\n\tdeep12 beat"),
        1,
    );
    let mut song = from_code(&code);
    let limits = SongLimits::default();
    let window = TimeSpan::cycle(0).unwrap();
    let expected = song.snapshot.query(window, &limits).unwrap();
    assert_eq!(expected.len(), 2);
    let mut remaining = limits.max_nodes;
    let request = single(&song.snapshot, window, &mut remaining);
    retain_index_occupancy(&mut song.snapshot, vec![request], limits, &mut remaining).unwrap();
    let reads = watch(&mut song.snapshot, true);
    let mut low = 1;
    let mut high = limits.max_nodes;
    while low < high {
        let mid = low + (high - low) / 2;
        let bounded = SongLimits {
            max_nodes: mid,
            ..limits
        };
        match song.snapshot.query(window, &bounded) {
            Ok(rows) => {
                assert_eq!(rows, expected);
                high = mid;
            }
            Err(error) => {
                assert_eq!(error.code, FailCode::FuelExhausted);
                low = mid + 1;
            }
        }
    }
    assert!(low > 1);
    compare(&mut song.snapshot, &expected, window);
    assert_eq!(
        song.snapshot
            .query(
                window,
                &SongLimits {
                    max_nodes: low,
                    ..limits
                }
            )
            .unwrap(),
        expected
    );
    assert_eq!(
        song.snapshot
            .query(
                window,
                &SongLimits {
                    max_nodes: low - 1,
                    ..limits
                }
            )
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    assert_eq!(
        song.snapshot
            .query(
                window,
                &SongLimits {
                    max_depth: limits.max_depth - 1,
                    ..limits
                }
            )
            .unwrap_err()
            .code,
        FailCode::DepthExceeded,
        "VM-only peak is not recorded: cache conservatively requires original admitted depth"
    );
    assert_eq!(reads.get(), 0);
}

#[test]
fn q_fault_keeps_raw_events_observations_and_journal_unpublished() {
    let code = program(
        false,
        false,
        "p",
        "song arrangement tail-seconds: 0 > play-song",
    )
    .replacen(
        "fn cut beat:\n\tfirst [0]",
        "fn cut beat:\n\t/ 0 {- 1 beat}",
        1,
    );
    let mut song = from_code(&code);
    let limits = SongLimits::default();
    let reads = watch(&mut song.snapshot, false);
    let mut remaining = limits.max_nodes;
    let requests = vec![
        single(&song.snapshot, TimeSpan::cycle(0).unwrap(), &mut remaining),
        single(&song.snapshot, TimeSpan::cycle(1).unwrap(), &mut remaining),
    ];
    assert_eq!(
        retain_index_occupancy(&mut song.snapshot, requests, limits, &mut remaining)
            .unwrap_err()
            .code,
        FailCode::DivisionByZero
    );
    assert!(
        reads.get() >= 2,
        "first original q succeeded before genuine second q fault"
    );
    assert!(song.snapshot.occupancy.is_empty());
    assert!(
        song.snapshot.replay.is_none(),
        "no raw-output or journal partial publication"
    );
    let before = reads.get();
    assert_eq!(
        song.snapshot
            .query(TimeSpan::cycle(0).unwrap(), &limits)
            .unwrap()
            .len(),
        2
    );
    assert!(
        reads.get() > before,
        "failed batch did not publish hidden replay authority"
    );
}

#[test]
fn nested_selected_part_keeps_inherited_slice_authority_and_raw_inner_owner() {
    let code = "fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {beat -> p} 2 [cut nil]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inner {transform-instrument base :drums :analog indexed}\nlet selected {transform-instrument inner :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song";
    let mut song = from_code(code);
    let limits = SongLimits::default();
    let window = TimeSpan::cycle(0).unwrap();
    let partial = span(1, 3, 8);
    let expected = song.snapshot.query(window, &limits).unwrap();
    let pieces = song.snapshot.query(partial, &limits).unwrap();
    assert_eq!(expected.len(), 2);
    assert_eq!(pieces.len(), 2);
    for row in &expected {
        let origin = row
            .source_origin
            .as_ref()
            .expect("genuine selected source origin");
        assert!(!origin.slice_timings().is_empty());
        assert!(
            origin
                .inherited
                .iter()
                .any(|frame| !frame.slice_timings().is_empty()),
            "inner Index timing remains authenticated"
        );
    }
    let mut remaining = limits.max_nodes;
    let requests = all_addresses(&song.snapshot, window, &mut remaining);
    assert_eq!(requests.len(), 1, "actual outer owner address");
    let outer_revision = requests[0].revision;
    let outer_root = requests[0].root;
    let track = requests[0].track;
    let source_index = match &song.snapshot.routing.parts[requests[0].scope].node {
        crate::song::snapshot::FrozenPartNode::Edit { source, .. } => *source,
        _ => panic!("actual outer transform topology"),
    };
    let inner = &song.snapshot.routing.parts[source_index];
    let inner_revision = inner.revision;
    let inner_root = match &inner.node {
        crate::song::snapshot::FrozenPartNode::Edit {
            edit: crate::song::snapshot::FrozenEdit::Transform { payload, .. },
            ..
        } => payload.id,
        _ => panic!("actual earlier selected transform topology"),
    };
    retain_index_occupancy(&mut song.snapshot, requests, limits, &mut remaining).unwrap();
    let scope = song
        .snapshot
        .routing
        .parts
        .iter()
        .position(|part| part.revision == outer_revision)
        .unwrap();
    assert_execution_graph(&song.snapshot, scope, track);
    let records = song.snapshot.replay.as_ref().unwrap().executions();
    let outer: Vec<_> = records
        .iter()
        .filter(|r| r.owner().revision == outer_revision)
        .collect();
    assert_eq!(outer.len(), 1, "one actual outer owner-local execution");
    assert_eq!(outer[0].owner().root, outer_root);
    let prerequisites: Vec<_> = records
        .iter()
        .filter(|r| r.owner().revision == inner_revision)
        .collect();
    assert!(
        prerequisites.len() >= 2,
        "earlier source walk and nested selected-source depth contexts: {:?}",
        prerequisites
            .iter()
            .map(|r| (r.owner(), r.entry_depth()))
            .collect::<Vec<_>>()
    );
    assert!(
        prerequisites.iter().any(|a| prerequisites
            .iter()
            .any(|b| a.entry_depth() != b.entry_depth())),
        "genuine shallow/deep execution contexts"
    );
    for record in records {
        let owner = record.owner();
        assert_eq!(owner.track, track);
        assert_eq!(owner.window, window);
        assert_eq!(owner.offset, Ratio64::ZERO);
        assert_eq!(owner.duration, Ratio64::from_int(2));
        if owner.revision == inner_revision {
            assert_eq!(owner.root, inner_root);
        }
        // Base Capture prerequisites are also required by the same frozen DAG;
        // assert_execution_graph binds their full owner roots independently.
    }
    assert!(outer[0]
        .observations()
        .iter()
        .any(|row| row.owner.revision == outer_revision));
    assert!(
        outer[0]
            .observations()
            .iter()
            .any(|row| row.owner.revision == inner_revision),
        "raw inner-owner authority retained without relabeling"
    );
    let reads = watch(&mut song.snapshot, true);
    compare(&mut song.snapshot, &pieces, partial);
    compare(&mut song.snapshot, &expected, window);
    compare(&mut song.snapshot, &pieces, partial);
    assert_eq!(
        reads.get(),
        0,
        "outer raw replay skips both actual Slice callbacks"
    );
}
