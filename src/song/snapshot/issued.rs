//! Private issued envelopes and the shared public descriptor freezing seam.
use super::{control_admission, FrozenControl, FrozenSongEvent, FrozenSound, SongSnapshot};
use crate::{
    pattern::{
        eval::{
            song_observation::{CanonicalIndexCollector, SharedIndexWork},
            song_provenance::{InvocationSeal, IssuedQueryTranscript},
            InputCells,
        },
        query::TimeSpan,
    },
    song::{
        query::issued::{query_part_issued, IssuedQueryBatch},
        source_uses::origin::FrozenIssuedSourceContribution,
        SongLimits,
    },
    vm::{
        fail::{FailCode, Failure},
        query_vm::MeteredSongQuery,
    },
};
use std::rc::Rc;

#[cfg_attr(not(test), allow(dead_code))] // Immutable route consumer is the mandatory following phase.
pub(crate) struct FrozenIssuedSongEvent {
    descriptor: FrozenSongEvent,
    invocations: Vec<Rc<InvocationSeal>>,
    source_contributions: Vec<FrozenIssuedSourceContribution>,
}
#[cfg_attr(not(test), allow(dead_code))] // Immutable route consumer is the mandatory following phase.
pub(crate) struct FrozenIssuedBatch {
    events: Vec<FrozenIssuedSongEvent>,
    transcript: Rc<IssuedQueryTranscript>,
}
impl FrozenIssuedSongEvent {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn descriptor(&self) -> &FrozenSongEvent {
        &self.descriptor
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn invocations(&self) -> &[Rc<InvocationSeal>] {
        &self.invocations
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn source_contributions(&self) -> &[FrozenIssuedSourceContribution] {
        &self.source_contributions
    }
}
impl FrozenIssuedBatch {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn events(&self) -> &[FrozenIssuedSongEvent] {
        &self.events
    }
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn transcript(&self) -> &Rc<IssuedQueryTranscript> {
        &self.transcript
    }
}
pub(super) fn freeze_public(
    rows: Vec<crate::song::SongEvent>,
    limits: &SongLimits,
    remaining: &mut u32,
) -> Result<Vec<FrozenSongEvent>, Failure> {
    control_admission(rows.len(), remaining)?;
    rows.into_iter()
        .map(|row| freeze_descriptor(row, limits.max_depth, remaining))
        .collect()
}
fn freeze_descriptor(
    row: crate::song::SongEvent,
    max_depth: u32,
    remaining: &mut u32,
) -> Result<FrozenSongEvent, Failure> {
    Ok(FrozenSongEvent {
        source_origin: row
            .event
            .song_source
            .as_deref()
            .map(|origin| crate::song::source_uses::copy_origin(origin, remaining, max_depth))
            .transpose()?,
        handle: row.handle,
        track: row.track,
        whole: row.event.whole,
        part: row.event.part,
        instrument: FrozenSound::from_sound(&row.instrument)?,
        note: row.tone,
        controls: {
            control_admission(row.event.controls.len(), remaining)?;
            row.event
                .controls
                .iter()
                .map(|(name, value)| {
                    Ok((*name, FrozenControl::copy(value, 0, max_depth, remaining)?))
                })
                .collect::<Result<_, Failure>>()?
        },
        route: row
            .route
            .map(|r| {
                control_admission(r.family.family().len(), remaining)?;
                Ok((
                    r.family
                        .family()
                        .iter()
                        .map(FrozenSound::from_sound)
                        .collect::<Result<_, Failure>>()?,
                    r.template,
                ))
            })
            .transpose()?,
    })
}
/// One caller ledger; copy routines borrow a local counter only while no collector borrow lives.
pub(crate) fn with_copy_budget<T>(
    work: &SharedIndexWork,
    depth: u32,
    copy: impl FnOnce(&mut u32, u32) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let (start, max_depth) = {
        let work = work.borrow();
        if depth >= work.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "issued freeze inherited depth",
            ));
        }
        (work.remaining(), work.limits.max_depth - depth)
    };
    let mut remaining = start;
    let result = copy(&mut remaining, max_depth);
    let spent = start
        .checked_sub(remaining)
        .ok_or_else(|| Failure::new(FailCode::Type, "copy counter increased"))?;
    work.borrow_mut().charge(u64::from(spent))?;
    result
}
fn query_issued_rows(
    snapshot: &mut SongSnapshot,
    span: TimeSpan,
    limits: &SongLimits,
    work: &SharedIndexWork,
    depth: u32,
) -> Result<IssuedQueryBatch, Failure> {
    let original = snapshot.song.clone();
    let replay = snapshot.replay.clone();
    let settings = *original.settings();
    let cells = InputCells::default();
    let (vm, ns) = snapshot.evaluator.vm_and_ns();
    let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
    query_part_issued(
        &original,
        span,
        &mut crate::song::SongQueryCtx {
            vm: &mut adapter,
            cells: &cells,
            seed: settings.seed,
            tempo: settings.tempo()?,
            limits,
        },
        work.clone(),
        depth,
        replay,
    )
}
fn freeze_batch(
    batch: IssuedQueryBatch,
    work: &SharedIndexWork,
    depth: u32,
) -> Result<FrozenIssuedBatch, Failure> {
    batch.transcript.check_rows(&batch.rows, work, depth)?;
    work.borrow_mut().charge(batch.rows.len() as u64 + 1)?;
    let mut events = Vec::with_capacity(batch.rows.len());
    for row in batch.rows {
        work.borrow_mut()
            .charge(row.source_contributions.len() as u64 + row.contributors.len() as u64 + 1)?;
        let mut sources = Vec::with_capacity(row.source_contributions.len());
        for contribution in &row.source_contributions {
            sources.push(crate::song::source::freeze_issued_source(
                contribution,
                &batch.transcript,
                work,
                depth,
            )?);
        }
        let descriptor = with_copy_budget(work, depth, |remaining, max_depth| {
            freeze_descriptor(row.row, max_depth, remaining)
        })?;
        events.push(FrozenIssuedSongEvent {
            descriptor,
            invocations: row.contributors,
            source_contributions: sources,
        });
    }
    work.borrow_mut().charge(1)?;
    Ok(FrozenIssuedBatch {
        events,
        transcript: Rc::new(batch.transcript),
    })
}
pub(super) fn query_issued_with_work(
    snapshot: &mut SongSnapshot,
    span: TimeSpan,
    work: &SharedIndexWork,
    depth: u32,
) -> Result<FrozenIssuedBatch, Failure> {
    let original = snapshot.song.clone();
    let limits = {
        let mut ledger = work.borrow_mut();
        if depth >= ledger.limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "issued query inherited depth",
            ));
        }
        match ledger.original.as_ref() {
            Some(attached) if !Rc::ptr_eq(attached, &original) => {
                return Err(Failure::new(
                    FailCode::Type,
                    "issued query collector belongs to another Song",
                ));
            }
            Some(_) => {}
            None => ledger.original = Some(original),
        }
        ledger.limits
    };
    (|| {
        let batch = query_issued_rows(snapshot, span, &limits, work, depth)?;
        freeze_batch(batch, work, depth)
    })()
}
pub(super) fn query_issued(
    snapshot: &mut SongSnapshot,
    span: TimeSpan,
    limits: &SongLimits,
    remaining: &mut u32,
    depth: u32,
) -> Result<FrozenIssuedBatch, Failure> {
    let work = CanonicalIndexCollector::new(*remaining, *limits)?;
    let result = query_issued_with_work(snapshot, span, &work, depth);
    *remaining = work.borrow().remaining();
    result
}

#[cfg(test)]
mod shared_work_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        host::caps::SampleData,
        ns::namespace::VarSlotRef,
        session::song::{evaluate_song_candidate, CandidateBuildCtx},
        song::{
            assets::{DecodedSongAssetFactory, SongAssetLimits},
            prepare_song,
            source_uses::FrozenUseOperation,
            PreparedSong, SnapshotEpoch,
        },
        value::{
            intern::{intern_kw, intern_sym},
            Ratio64,
        },
        vm::vm::ReadObserver,
    };
    use std::{cell::Cell, collections::BTreeMap, sync::Arc};
    fn limits() -> SongLimits {
        SongLimits {
            max_nodes: 1_000_000,
            ..SongLimits::default()
        }
    }
    fn span(a: i64, b: i64, d: i64) -> TimeSpan {
        TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
    }
    fn from_code(code: &str) -> PreparedSong {
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
        prepare_song(
            evaluate_song_candidate(
                code,
                "frozen-issued.vact",
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
    fn code(body: &str, end: &str) -> String {
        format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog > chord [:c :five] > gain 0.25}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog indexed}}\n{end} > play-song")
    }
    struct Reads {
        denied: bool,
        count: Rc<Cell<u32>>,
    }
    impl ReadObserver for Reads {
        fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
            if slot.name() == intern_sym("cut") {
                self.count.set(self.count.get() + 1);
                if self.denied {
                    return Err(Failure::new(
                        FailCode::HostUnavailable,
                        "frozen issued callback reread",
                    ));
                }
            }
            Ok(())
        }
    }
    fn deny(song: &mut PreparedSong) -> Rc<Cell<u32>> {
        let count = Rc::new(Cell::new(0));
        song.snapshot
            .evaluator
            .vm_and_ns()
            .0
            .set_read_observer(Some(Box::new(Reads {
                denied: true,
                count: count.clone(),
            })));
        count
    }
    fn retain(song: &mut PreparedSong, window: TimeSpan) {
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
            let (index, prefix) = pending.pop().expect("genuine original Index site");
            let node = &recipe.nodes()[index as usize];
            if node.operation() == FrozenUseOperation::Slice {
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
                let mut path = prefix.clone();
                path.extend_from_slice(edge.trace());
                pending.push((edge.child(), path));
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
                window,
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
    fn query(song: &mut PreparedSong, window: TimeSpan) -> FrozenIssuedBatch {
        song.query_issued(window, &limits(), &mut limits().max_nodes, 0)
            .unwrap()
    }
    fn validate(batch: &FrozenIssuedBatch, original: &Rc<crate::song::Song>) {
        let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
        for event in batch.events() {
            for seal in event.invocations() {
                assert!(batch
                    .transcript()
                    .authentic(original, seal, &work, 0)
                    .unwrap());
                assert!(batch
                    .transcript()
                    .invocation(seal, &work, 0)
                    .unwrap()
                    .authentic_original(original));
            }
            for contribution in event.source_contributions() {
                let members = batch
                    .transcript()
                    .source_members(contribution.leaves(), &work, 0)
                    .unwrap();
                assert_eq!(members.len(), contribution.members().len());
                for copied in contribution.members() {
                    let actual = &members[copied.member_slot()];
                    assert!(actual.is_sealed_member(&work, 0).unwrap());
                    let expected = with_copy_budget(&work, 0, |remaining, max_depth| {
                        crate::song::source::copy_origin(actual.member(), remaining, max_depth)
                    })
                    .unwrap();
                    assert_eq!(copied.origin(), &expected);
                    assert!(contribution
                        .augmented_origin()
                        .preserves_raw_member(copied.origin()));
                }
            }
        }
    }
    #[test]
    fn actual_direct_chord_bank_and_controls_freeze_without_proof_in_public_equality() {
        let code="inst closed bank: keyword = :event-bank:\n\tsample-play bank n: 0 rate: 1 > * amp\nsong {part [drums: {s closed > chord [:c :five] > gain 0.25}] duration: 2} tail-seconds: 0 > play-song";
        let mut song = from_code(code);
        assert_eq!(
            song.snapshot
                .closed_bank_geometry(intern_kw("event-bank"))
                .unwrap()
                .0,
            1
        );
        let expected = song.query(span(0, 2, 1), &limits()).unwrap();
        let batch = query(&mut song, span(0, 2, 1));
        assert_eq!(
            batch
                .events()
                .iter()
                .map(|event| event.descriptor().clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(!batch.events().is_empty());
        assert!(batch
            .events()
            .iter()
            .all(|event| !event.descriptor().controls.is_empty()));
        assert!(batch
            .events()
            .iter()
            .all(|event| event.source_contributions().is_empty()));
        validate(&batch, &song.snapshot.song);
    }
    #[test]
    fn cached_nested_chord_retains_every_inherited_frame_and_sealed_member_without_reads() {
        let program="fn cut beat:\n\tfirst [0]\nfn inner p:\n\tslice {beat -> p} 2 [cut nil]\nfn outer p:\n\tslice {beat -> p} 2 [cut nil]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inside {transform-instrument base :drums :analog inner}\nlet selected {transform-instrument inside :drums :analog outer}\nsong selected tail-seconds: 0 > play-song";
        let mut song = from_code(program);
        let expected = song.query(span(0, 1, 1), &limits()).unwrap();
        retain(&mut song, span(0, 2, 1));
        let denied = deny(&mut song);
        let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
        work.borrow_mut().original = Some(song.snapshot.song.clone());
        let live =
            query_issued_rows(&mut song.snapshot, span(0, 1, 1), &limits(), &work, 0).unwrap();
        let mut inherited_depth_changes = Vec::new();
        for row in &live.rows {
            for pair in &row.source_contributions {
                let augmented = with_copy_budget(&work, 0, |remaining, depth| {
                    crate::song::source::copy_origin(&pair.augmented_origin, remaining, depth)
                })
                .unwrap();
                for member in live
                    .transcript
                    .source_members(&pair.leaves, &work, 0)
                    .unwrap()
                {
                    let raw = with_copy_budget(&work, 0, |remaining, depth| {
                        crate::song::source::copy_origin(member.member(), remaining, depth)
                    })
                    .unwrap();
                    assert!(augmented.preserves_raw_member(&raw), "actual nested fields/issued timing identity changed: augmented={augmented:?}, raw={raw:?}");
                    for (a, b) in augmented.inherited.iter().zip(&raw.inherited) {
                        for (a, b) in a.slice_timings().iter().zip(b.slice_timings()) {
                            if a.admission_depth() != b.admission_depth() {
                                assert!(a.admission_depth() > b.admission_depth());
                                assert!(a.admission_depth() <= limits().max_depth);
                                assert!(b.admission_depth() <= limits().max_depth);
                                inherited_depth_changes.push((
                                    a.admission_depth(),
                                    b.admission_depth(),
                                    a.issuer(),
                                    a.subject_handle().clone(),
                                ));
                            }
                        }
                    }
                }
            }
        }
        assert!(!inherited_depth_changes.is_empty(), "genuine nested append must demonstrate the full-chain depth stamp mismatch that made inherited PartialEq too strict");
        eprintln!("actual nested augmented/raw inherited certificates={inherited_depth_changes:?}");
        let batch = freeze_batch(live, &work, 0).unwrap();
        assert_eq!(
            batch
                .events()
                .iter()
                .map(|event| event.descriptor().clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(batch.events().iter().any(|event| event
            .source_contributions()
            .iter()
            .any(|origin| !origin.augmented_origin().inherited.is_empty())));
        validate(&batch, &song.snapshot.song);
        assert_eq!(denied.get(), 0);
    }
    #[test]
    fn fractional_union_keeps_discarded_actual_augmented_metadata_and_reordered_authority() {
        let mut song=from_code(&code("slow {slice {beat -> p} 1 [cut]} 2","let intro {part [drums: nil] duration: 1/2}\nsong {sequence [intro selected]} tail-seconds: 0"));
        let expected = song.query(span(0, 5, 2), &limits()).unwrap();
        retain(&mut song, span(0, 5, 2));
        let denied = deny(&mut song);
        let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
        work.borrow_mut().original = Some(song.snapshot.song.clone());
        let live =
            query_issued_rows(&mut song.snapshot, span(0, 5, 2), &limits(), &work, 0).unwrap();
        let mut discarded_postseal = false;
        let mut all_expected = Vec::new();
        for row in &live.rows {
            let mut expected_sources = Vec::new();
            for pair in &row.source_contributions {
                let copied = with_copy_budget(&work, 0, |remaining, depth| {
                    crate::song::source::copy_origin(&pair.augmented_origin, remaining, depth)
                })
                .unwrap();
                let mut chain = row.event.song_source.as_ref();
                let mut survives = false;
                while let Some(origin) = chain {
                    survives |= Rc::ptr_eq(origin, &pair.augmented_origin);
                    chain = origin.inherited.as_ref();
                }
                let members = live
                    .transcript
                    .source_members(&pair.leaves, &work, 0)
                    .unwrap();
                for member in members {
                    let raw = with_copy_budget(&work, 0, |remaining, depth| {
                        crate::song::source::copy_origin(member.member(), remaining, depth)
                    })
                    .unwrap();
                    discarded_postseal |=
                        !survives && copied.slice_timings().len() > raw.slice_timings().len();
                }
                expected_sources.push((pair.leaves.clone(), copied));
            }
            all_expected.push(expected_sources);
        }
        assert!(
            discarded_postseal,
            "actual equal-handle union retains a discarded origin with genuine post-seal timing"
        );
        let batch = freeze_batch(live, &work, 0).unwrap();
        assert_eq!(
            batch
                .events()
                .iter()
                .map(|event| event.descriptor().clone())
                .collect::<Vec<_>>(),
            expected
        );
        for (event, sources) in batch.events().iter().zip(all_expected) {
            assert_eq!(event.source_contributions().len(), sources.len());
            for (actual, (leaves, origin)) in event.source_contributions().iter().zip(sources) {
                assert!(Rc::ptr_eq(actual.leaves(), &leaves));
                assert_eq!(actual.augmented_origin(), &origin);
            }
        }
        validate(&batch, &song.snapshot.song);
        let last = query(&mut song, span(3, 5, 2));
        let first = query(&mut song, span(0, 3, 2));
        validate(&last, &song.snapshot.song);
        validate(&first, &song.snapshot.song);
        assert_eq!(denied.get(), 0);
    }
    #[test]
    fn actual_empty_selected_source_has_sealed_completion_and_deleted_tone_keeps_sibling() {
        let empty="fn cut beat:\n\tfirst [0]\nfn inner p:\n\tslice {beat -> nil} 2 [cut nil]\nfn outer p:\n\tslice {beat -> p} 2 [cut nil]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inside {transform-instrument base :drums :analog inner}\nlet selected {transform-instrument inside :drums :analog outer}\nsong selected tail-seconds: 0 > play-song";
        let mut song = from_code(empty);
        retain(&mut song, span(0, 2, 1));
        let denied = deny(&mut song);
        let batch = query(&mut song, span(1, 2, 2));
        assert!(batch.events().is_empty());
        let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
        assert!(
            batch
                .transcript()
                .empty_source_completions(&work, 0)
                .unwrap()
                > 0
        );
        assert_eq!(denied.get(), 0);
        let end="let events {part-events selected :drums 0 1}\nlet changed {delete-event selected {{first events} :handle}}\nsong changed tail-seconds: 0";
        let mut deleted = from_code(&code("slice {beat -> p} 2 [cut nil]", end));
        let expected = deleted.query(span(0, 1, 1), &limits()).unwrap();
        let actual = query(&mut deleted, span(0, 1, 1));
        assert_eq!(
            actual
                .events()
                .iter()
                .map(|event| event.descriptor().clone())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(actual.events().len(), 1);
        validate(&actual, &deleted.snapshot.song);
    }
    #[test]
    fn genuine_same_vary_repeat_placements_and_seed_identity_survive_freezing() {
        for mode in [":same", ":vary"] {
            let mut song = from_code(&code(
                "slice {beat -> p} 2 [cut nil]",
                &format!("song {{part-repeat selected 2 seed-mode: {mode}}} tail-seconds: 0"),
            ));
            retain(&mut song, span(0, 4, 1));
            let denied = deny(&mut song);
            let batch = query(&mut song, span(0, 4, 1));
            let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
            let owners: Vec<_> = batch
                .events()
                .iter()
                .map(|event| {
                    batch
                        .transcript()
                        .invocation(&event.invocations()[0], &work, 0)
                        .unwrap()
                })
                .collect();
            let first = owners[0];
            let second = owners
                .iter()
                .find(|owner| owner.owner().placement != first.owner().placement)
                .unwrap();
            assert_eq!(first.owner().revision, second.owner().revision);
            assert_eq!(first.owner().root, second.owner().root);
            if mode == ":same" {
                assert_eq!(first.seed(), second.seed());
            } else {
                assert_ne!(first.seed(), second.seed());
            }
            validate(&batch, &song.snapshot.song);
            assert_eq!(denied.get(), 0);
        }
    }
    fn live(song: &mut PreparedSong) -> (SharedIndexWork, IssuedQueryBatch) {
        let work = CanonicalIndexCollector::new(limits().max_nodes, limits()).unwrap();
        work.borrow_mut().original = Some(song.snapshot.song.clone());
        let batch =
            query_issued_rows(&mut song.snapshot, span(0, 1, 1), &limits(), &work, 0).unwrap();
        (work, batch)
    }
    #[test]
    fn actual_foreign_transcript_and_swapped_original_source_pairs_refuse_atomic_freeze() {
        let input = code(
            "stack [{slice {beat -> p} 2 [cut nil]} {slice {beat -> p} 2 [cut nil]}]",
            "song selected tail-seconds: 0",
        );
        let mut original = from_code(&input);
        let mut foreign = from_code(&input);
        let (work, mut batch) = live(&mut original);
        let (_, other) = live(&mut foreign);
        batch.transcript = other.transcript;
        assert_eq!(
            freeze_batch(batch, &work, 0).err().unwrap().code,
            FailCode::Type
        );
        let (work, mut batch) = live(&mut original);
        let genuine = batch
            .rows
            .iter()
            .flat_map(|row| &row.source_contributions)
            .find(|pair| {
                !Rc::ptr_eq(
                    &pair.augmented_origin,
                    &batch.rows[0].source_contributions[0].augmented_origin,
                )
            })
            .unwrap()
            .clone();
        batch.rows[0].source_contributions[0] = genuine;
        assert_eq!(
            freeze_batch(batch, &work, 0).err().unwrap().code,
            FailCode::Type
        );
        let accepted = query(&mut original, span(0, 1, 1));
        validate(&accepted, &original.snapshot.song);
    }
    #[test]
    fn complete_query_authentication_freeze_exact_one_less_depth_and_fault_keep_prior_view() {
        let mut song = from_code(&code(
            "slice {beat -> p} 2 [cut nil]",
            "song selected tail-seconds: 0",
        ));
        retain(&mut song, span(0, 2, 1));
        let view = song.snapshot.replay.clone().unwrap();
        let denied = deny(&mut song);
        let mut remaining = limits().max_nodes;
        let accepted = song
            .query_issued(span(0, 1, 1), &limits(), &mut remaining, 0)
            .unwrap();
        let cost = limits().max_nodes - remaining;
        assert!(cost > 1);
        let mut exact = cost;
        let again = song
            .query_issued(span(0, 1, 1), &limits(), &mut exact, 0)
            .unwrap();
        assert_eq!(exact, 0);
        assert_eq!(
            again
                .events()
                .iter()
                .map(|row| row.descriptor().clone())
                .collect::<Vec<_>>(),
            accepted
                .events()
                .iter()
                .map(|row| row.descriptor().clone())
                .collect::<Vec<_>>()
        );
        let mut short = cost - 1;
        assert_eq!(
            song.query_issued(span(0, 1, 1), &limits(), &mut short, 0)
                .err()
                .unwrap()
                .code,
            FailCode::FuelExhausted
        );
        assert!(short < cost - 1);
        assert!(Rc::ptr_eq(song.snapshot.replay.as_ref().unwrap(), &view));
        let mut depth_budget = limits().max_nodes;
        assert_eq!(
            song.query_issued(
                span(0, 1, 1),
                &limits(),
                &mut depth_budget,
                limits().max_depth
            )
            .err()
            .unwrap()
            .code,
            FailCode::DepthExceeded
        );
        assert!(depth_budget < limits().max_nodes);
        assert_eq!(denied.get(), 0);
        let mut uncached = from_code(&code(
            "slice {beat -> p} 2 [cut nil]",
            "song selected tail-seconds: 0",
        ));
        let reads = deny(&mut uncached);
        let mut budget = limits().max_nodes;
        assert_eq!(
            uncached
                .query_issued(span(0, 1, 1), &limits(), &mut budget, 0)
                .err()
                .unwrap()
                .code,
            FailCode::HostUnavailable
        );
        assert!(budget < limits().max_nodes);
        assert!(reads.get() > 0);
        assert!(uncached.snapshot.replay.is_none());
        let retry = query(&mut song, span(0, 1, 1));
        validate(&retry, &song.snapshot.song);
        assert_eq!(denied.get(), 0);
    }
}
