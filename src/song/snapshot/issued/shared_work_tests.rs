use super::*;
use crate::{
    host::caps::SampleData,
    ns::namespace::VarSlotRef,
    pattern::eval::song_observation::CanonicalIndexCollector,
    session::song::{evaluate_song_candidate, CandidateBuildCtx},
    song::{
        assets::{DecodedSongAssetFactory, SongAssetLimits},
        prepare_song, PreparedSong, SnapshotEpoch,
    },
    value::{
        intern::{intern_kw, intern_sym},
        Ratio64,
    },
    vm::vm::ReadObserver,
};
use std::{cell::Cell, collections::BTreeMap, rc::Rc, sync::Arc};

fn limits() -> SongLimits {
    SongLimits {
        max_nodes: 1_000_000,
        ..SongLimits::default()
    }
}

fn span(start: i64, end: i64) -> TimeSpan {
    TimeSpan::new(
        Ratio64::new(start, 1).unwrap(),
        Ratio64::new(end, 1).unwrap(),
    )
    .unwrap()
}

fn from_code() -> PreparedSong {
    let samples = BTreeMap::from([(
        "a.wav".into(),
        Arc::new(SampleData {
            rate: 48000,
            channels: 2,
            frames: vec![0.25, 0.5].into(),
        }),
    )]);
    let factory = DecodedSongAssetFactory::new(
        samples,
        BTreeMap::from([(intern_kw("event-bank"), vec!["a.wav".into()])]),
        BTreeMap::new(),
    );
    let code = "fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tslice {beat -> p} 2 [cut nil]\nlet base {part [drums: {s :analog > chord [:c :five] > gain 0.25}] duration: 2}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0 > play-song";
    prepare_song(
        evaluate_song_candidate(
            code,
            "shared-work.vact",
            7,
            SnapshotEpoch(191),
            &CandidateBuildCtx {
                assets: &factory,
                asset_limits: SongAssetLimits {
                    max_resources: 128,
                    max_pcm_bytes: 1_000_000,
                    max_source_files: 32,
                    max_source_bytes: 100_000,
                    max_banks: 32,
                    max_walk_nodes: 1_000_000,
                    max_walk_depth: 256,
                },
                lock: None,
                cache: None,
            },
        )
        .unwrap(),
    )
    .unwrap()
}

fn retain(song: &mut PreparedSong) {
    let snapshot = &mut song.snapshot;
    let (scope, payload) = snapshot
        .routing
        .parts
        .iter()
        .enumerate()
        .rev()
        .find_map(|(scope, part)| match &part.node {
            super::super::FrozenPartNode::Edit {
                edit: super::super::FrozenEdit::Transform { payload, .. },
                ..
            } => Some((scope, payload)),
            _ => None,
        })
        .unwrap();
    let recipe = payload.index_timing().unwrap();
    let mut pending = vec![(recipe.root(), Vec::new())];
    let (issuer, prefix) = loop {
        let (index, prefix) = pending.pop().unwrap();
        let node = &recipe.nodes()[index as usize];
        if node.operation() == crate::song::source_uses::FrozenUseOperation::Slice {
            let subject = node
                .children()
                .iter()
                .find(|edge| edge.role() == 0)
                .unwrap();
            let index = node
                .children()
                .iter()
                .find(|edge| edge.role() == 1)
                .unwrap();
            if !recipe.nodes()[subject.child() as usize].structured()
                && recipe.nodes()[index.child() as usize].structured()
            {
                break (node.issuer(), prefix);
            }
        }
        for edge in node.children() {
            let mut child = prefix.clone();
            child.extend_from_slice(edge.trace());
            pending.push((edge.child(), child));
        }
    };
    let policy = limits();
    let mut remaining = policy.max_nodes;
    let request = snapshot
        .canonical_index_request(
            scope,
            intern_kw("drums"),
            issuer,
            &prefix,
            span(0, 2),
            0,
            policy,
            &mut remaining,
        )
        .unwrap();
    super::super::occupancy::retain_index_occupancy(
        snapshot,
        vec![request],
        policy,
        &mut remaining,
    )
    .unwrap();
}

struct Reads {
    reads: Rc<Cell<u32>>,
}
impl ReadObserver for Reads {
    fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
        if slot.name() == intern_sym("cut") {
            self.reads.set(self.reads.get() + 1);
        }
        Ok(())
    }
}

fn observe(song: &mut PreparedSong) -> Rc<Cell<u32>> {
    let reads = Rc::new(Cell::new(0));
    song.snapshot
        .evaluator
        .vm_and_ns()
        .0
        .set_read_observer(Some(Box::new(Reads {
            reads: reads.clone(),
        })));
    reads
}

fn same_ptrs<T>(left: &[Rc<T>], right: &[Rc<T>]) -> bool {
    left.len() == right.len() && left.iter().zip(right).all(|(a, b)| Rc::ptr_eq(a, b))
}

#[test]
fn two_queries_keep_first_query_executions_and_debit_monotonically() {
    let mut song = from_code();
    retain(&mut song);
    let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
    let first = song.query_issued_with_work(span(0, 1), &work, 0).unwrap();
    assert!(!first.events().is_empty());
    let first_executions = work.borrow().executions.clone();
    assert!(!first_executions.is_empty());
    let after_first = work.borrow().remaining();
    let second = song.query_issued_with_work(span(1, 2), &work, 0).unwrap();
    assert!(!second.events().is_empty());
    let ledger = work.borrow();
    assert!(ledger.remaining() < after_first);
    assert!(first_executions.iter().all(|first| ledger
        .executions
        .iter()
        .any(|execution| Rc::ptr_eq(first, execution))));
}

#[test]
fn seeding_existing_executions_does_not_duplicate_and_uses_checked_scan_charge() {
    let mut song = from_code();
    retain(&mut song);
    let view = song.snapshot.replay.clone().unwrap();
    assert!(!view.executions().is_empty());
    let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
    {
        let mut ledger = work.borrow_mut();
        ledger.original = Some(song.snapshot.song.clone());
        ledger.executions.extend(view.executions().iter().cloned());
    }
    let prior = view.executions().to_vec();
    let before = work.borrow().remaining();
    view.seed_collection(&song.snapshot.song, &work).unwrap();
    let after = work.borrow();
    let expected_charge = view.executions().len() * (prior.len() + 1);
    assert_eq!(before - after.remaining(), expected_charge as u32);
    assert!(same_ptrs(&after.executions, &prior));
}

#[test]
fn fresh_collector_seed_matches_view_and_keeps_original_fresh_charge() {
    let mut song = from_code();
    retain(&mut song);
    let view = song.snapshot.replay.clone().unwrap();
    let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
    let before = work.borrow().remaining();
    view.seed_collection(&song.snapshot.song, &work).unwrap();
    let after = work.borrow();
    assert_eq!(before - after.remaining(), view.executions().len() as u32);
    assert!(same_ptrs(&after.executions, view.executions()));
    assert!(after.original.is_none());
}

#[test]
fn foreign_collector_refuses_before_query_callback_or_debit() {
    let mut song = from_code();
    retain(&mut song);
    let reads = observe(&mut song);
    let foreign = from_code();
    let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
    work.borrow_mut().original = Some(foreign.snapshot.song.clone());
    let before = work.borrow().remaining();
    assert_eq!(
        song.query_issued_with_work(span(0, 1), &work, 0)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    assert_eq!(work.borrow().remaining(), before);
    assert_eq!(reads.get(), 0);

    let view = song.snapshot.replay.clone().unwrap();
    assert_eq!(
        view.seed_collection(&song.snapshot.song, &work)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    assert_eq!(work.borrow().remaining(), before);
}

#[test]
fn unattached_collector_with_prior_executions_refuses_without_mutation() {
    let mut song = from_code();
    retain(&mut song);
    let view = song.snapshot.replay.clone().unwrap();
    let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
    work.borrow_mut()
        .executions
        .push(view.executions()[0].clone());
    let prior = work.borrow().executions.clone();
    let before = work.borrow().remaining();
    assert_eq!(
        view.seed_collection(&song.snapshot.song, &work)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    let after = work.borrow();
    assert_eq!(after.remaining(), before);
    assert!(after.original.is_none());
    assert!(same_ptrs(&after.executions, &prior));
}

#[test]
fn attached_collector_rejects_foreign_execution_without_synthetic_repair() {
    let mut song = from_code();
    retain(&mut song);
    let view = song.snapshot.replay.clone().unwrap();
    let mut foreign_song = from_code();
    retain(&mut foreign_song);
    let foreign_view = foreign_song.snapshot.replay.clone().unwrap();

    let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
    let foreign_execution = foreign_view.executions()[0].clone();
    {
        let mut ledger = work.borrow_mut();
        ledger.original = Some(song.snapshot.song.clone());
        ledger.executions.push(foreign_execution.clone());
    }
    let before = work.borrow().remaining();
    let charge = view.executions().len() * 2;
    assert_eq!(
        view.seed_collection(&song.snapshot.song, &work)
            .err()
            .unwrap()
            .code,
        FailCode::Type
    );
    let after = work.borrow();
    assert_eq!(before - after.remaining(), charge as u32);
    assert!(after
        .original
        .as_ref()
        .is_some_and(|attached| Rc::ptr_eq(attached, &song.snapshot.song)));
    assert_eq!(after.executions.len(), 1);
    assert!(Rc::ptr_eq(&after.executions[0], &foreign_execution));
}

#[test]
fn legacy_query_writes_back_exact_and_one_less_work_on_success_and_error() {
    let mut measured_song = from_code();
    retain(&mut measured_song);
    let mut remaining = limits().max_nodes;
    measured_song
        .query_issued(span(0, 1), &limits(), &mut remaining, 0)
        .unwrap();
    let exact_cost = limits().max_nodes - remaining;
    assert!(exact_cost > 1);

    let mut exact_song = from_code();
    retain(&mut exact_song);
    let mut exact = exact_cost;
    exact_song
        .query_issued(span(0, 1), &limits(), &mut exact, 0)
        .unwrap();
    assert_eq!(exact, 0);

    let mut short_song = from_code();
    retain(&mut short_song);
    let mut short = exact_cost - 1;
    assert_eq!(
        short_song
            .query_issued(span(0, 1), &limits(), &mut short, 0)
            .err()
            .unwrap()
            .code,
        FailCode::FuelExhausted
    );
    assert_eq!(short, 0);
}

#[test]
fn legacy_query_output_matches_shared_engine_output_on_fresh_collector() {
    let mut song = from_code();
    retain(&mut song);
    let mut legacy_remaining = limits().max_nodes;
    let legacy = song
        .query_issued(span(0, 2), &limits(), &mut legacy_remaining, 0)
        .unwrap();

    let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
    let shared = song.query_issued_with_work(span(0, 2), &work, 0).unwrap();
    assert_eq!(
        legacy
            .events()
            .iter()
            .map(|event| event.descriptor().clone())
            .collect::<Vec<_>>(),
        shared
            .events()
            .iter()
            .map(|event| event.descriptor().clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        legacy
            .events()
            .iter()
            .map(|event| event.invocations().len())
            .collect::<Vec<_>>(),
        shared
            .events()
            .iter()
            .map(|event| event.invocations().len())
            .collect::<Vec<_>>()
    );
}
