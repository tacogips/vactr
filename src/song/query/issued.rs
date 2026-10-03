//! Internal row envelopes; public query wrappers erase only these envelopes.
use crate::{
    pattern::eval::{
        song_observation::SharedIndexWork,
        song_provenance::{InvocationSeal, IssuedQueryTransaction, IssuedQueryTranscript},
        QState,
    },
    song::{EventHandle, Song, SongEvent},
    vm::fail::Failure,
};
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
    rc::Rc,
};
#[derive(Clone)]
pub(crate) struct IssuedSourceContribution {
    pub(crate) leaves: Rc<crate::pattern::eval::song_provenance::IssuedSourceLeaves>,
    pub(crate) augmented_origin: Rc<crate::song::source::SongEventOrigin>,
}
#[derive(Clone)]
pub(crate) struct IssuedQueryRow {
    pub(crate) row: SongEvent,
    pub(crate) contributors: Vec<Rc<InvocationSeal>>,
    original_contributors: Vec<Rc<InvocationSeal>>,
    pub(crate) sources: Vec<Rc<crate::pattern::eval::song_provenance::IssuedSourceLeaves>>,
    pub(crate) source_contributions: Vec<IssuedSourceContribution>,
    original_source_contributions: Vec<IssuedSourceContribution>,
}
impl Deref for IssuedQueryRow {
    type Target = SongEvent;
    fn deref(&self) -> &SongEvent {
        &self.row
    }
}
impl DerefMut for IssuedQueryRow {
    fn deref_mut(&mut self) -> &mut SongEvent {
        &mut self.row
    }
}
impl IssuedQueryRow {
    pub(crate) fn authentic_contributors(
        &self,
        work: &mut crate::pattern::eval::song_observation::CanonicalIndexCollector,
    ) -> Result<bool, Failure> {
        work.charge(self.contributors.len() as u64 + self.source_contributions.len() as u64 + 1)?;
        Ok(
            self.source_contributions.len() == self.original_source_contributions.len()
                && self
                    .source_contributions
                    .iter()
                    .zip(&self.original_source_contributions)
                    .all(|(a, b)| {
                        Rc::ptr_eq(&a.leaves, &b.leaves)
                            && Rc::ptr_eq(&a.augmented_origin, &b.augmented_origin)
                            && a.augmented_origin
                                .issued_leaves
                                .as_ref()
                                .is_some_and(|bag| Rc::ptr_eq(bag, &a.leaves))
                    })
                && self.contributors.len() == self.original_contributors.len()
                && self
                    .contributors
                    .iter()
                    .zip(&self.original_contributors)
                    .all(|(a, b)| Rc::ptr_eq(a, b)),
        )
    }
}
#[cfg_attr(not(test), allow(dead_code))] // Consumed by the required frozen handoff next.
pub(crate) struct IssuedQueryBatch {
    pub(crate) rows: Vec<IssuedQueryRow>,
    pub(crate) transcript: IssuedQueryTranscript,
}
pub(super) fn public(rows: Vec<IssuedQueryRow>) -> Vec<SongEvent> {
    rows.into_iter().map(|row| row.row).collect()
}
pub(super) fn expand(
    row: SongEvent,
    seal: Option<&Rc<InvocationSeal>>,
    state: &mut QState<'_, '_>,
) -> Result<IssuedQueryRow, Failure> {
    if !state.issuance_enabled() {
        return Ok(IssuedQueryRow {
            row,
            contributors: Vec::new(),
            original_contributors: Vec::new(),
            sources: Vec::new(),
            source_contributions: Vec::new(),
            original_source_contributions: Vec::new(),
        });
    }
    let mut contributors = Vec::new();
    let mut sources = Vec::new();
    let mut source_contributions = Vec::new();
    if let Some(seal) = seal {
        state.spend(2, None)?;
        contributors.push(seal.clone());
    }
    let mut origin = row.event.song_source.as_ref();
    while let Some(member) = origin {
        state.spend(1, None)?;
        if let Some(bag) = &member.issued_leaves {
            state.spend(3, None)?;
            sources.push(bag.clone());
            source_contributions.push(IssuedSourceContribution {
                leaves: bag.clone(),
                augmented_origin: member.clone(),
            });
            for seal in bag.contributors() {
                state.spend(contributors.len() as u64 + 1, None)?;
                if !contributors.iter().any(|prior| Rc::ptr_eq(prior, seal)) {
                    contributors.push(seal.clone());
                }
            }
        }
        origin = member.inherited.as_ref();
    }
    state.spend(
        contributors.len() as u64 + source_contributions.len() as u64 * 2,
        None,
    )?;
    Ok(IssuedQueryRow {
        row,
        original_contributors: contributors.clone(),
        contributors,
        sources,
        original_source_contributions: source_contributions.clone(),
        source_contributions,
    })
}
pub(super) fn union(
    rows: &mut BTreeMap<EventHandle, IssuedQueryRow>,
    row: IssuedQueryRow,
    work: Option<&SharedIndexWork>,
) -> Result<(), Failure> {
    if let Some(prior) = rows.get_mut(&row.handle) {
        for seal in &row.contributors {
            if let Some(work) = work {
                work.borrow_mut()
                    .charge(prior.contributors.len() as u64 + 1)?;
            }
            if !prior.contributors.iter().any(|old| Rc::ptr_eq(old, seal)) {
                prior.contributors.push(seal.clone());
            }
        }
        for bag in &row.sources {
            if let Some(work) = work {
                work.borrow_mut().charge(prior.sources.len() as u64 + 1)?;
            }
            if !prior.sources.iter().any(|old| Rc::ptr_eq(old, bag)) {
                prior.sources.push(bag.clone());
            }
        }
        for contribution in &row.source_contributions {
            if let Some(work) = work {
                work.borrow_mut()
                    .charge(prior.source_contributions.len() as u64 + 2)?;
            }
            if !prior.source_contributions.iter().any(|old| {
                Rc::ptr_eq(&old.leaves, &contribution.leaves)
                    && Rc::ptr_eq(&old.augmented_origin, &contribution.augmented_origin)
            }) {
                prior.source_contributions.push(contribution.clone());
            }
        }
        if let Some(work) = work {
            work.borrow_mut().charge(
                prior.contributors.len() as u64 + prior.source_contributions.len() as u64 * 2,
            )?;
        }
        prior.original_contributors = prior.contributors.clone();
        prior.original_source_contributions = prior.source_contributions.clone();
        prior.event.part.begin = prior.event.part.begin.min(row.event.part.begin);
        prior.event.part.end = prior.event.part.end.max(row.event.part.end);
    } else {
        rows.insert(row.handle.clone(), row);
    }
    Ok(())
}
#[cfg_attr(not(test), allow(dead_code))] // Actual owning issued entry; snapshot adapter is the next phase.
pub(crate) fn query_part_issued(
    original: &Rc<Song>,
    span: crate::pattern::query::TimeSpan,
    cx: &mut super::SongQueryCtx<'_>,
    work: SharedIndexWork,
    depth: u32,
    replay: Option<Rc<crate::pattern::eval::song_replay::ReplayView>>,
) -> Result<IssuedQueryBatch, Failure> {
    if let Some(view) = &replay {
        view.seed_collection(original, &work)?;
    }
    let tx = IssuedQueryTransaction::begin(original.clone(), work.clone(), depth)?;
    let saved_depth = work.borrow().depth;
    let result = super::query_part_impl(
        original.part(),
        span,
        cx,
        Some((work.clone(), depth)),
        replay,
        Some(tx.clone()),
    );
    work.borrow_mut().depth = saved_depth;
    let rows = result?;
    let transcript = IssuedQueryTransaction::publish(tx)?;
    transcript.check_rows(&rows, &work, depth)?;
    Ok(IssuedQueryBatch { rows, transcript })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ns::{
            evaluator::Evaluator,
            namespace::{Prelude, VarSlotRef},
            stage::RecordingSink,
        },
        pattern::{
            eval::{
                song_observation::CanonicalIndexCollector, song_replay::ReplayView, InputCells,
            },
            query::TimeSpan,
        },
        reader::span::FileId,
        session::song::{evaluate_song_candidate, freeze::Freeze, CandidateBuildCtx},
        song::{
            assets::{DecodedSongAssetFactory, SongAssetFactory, SongAssetLimits, SongSourceFile},
            source::copy_origin,
            SnapshotEpoch, SongLimits,
        },
        value::{intern::intern_sym, Ratio64, Value},
        vm::{
            fail::{FailCode, Failure},
            query_vm::{MeteredSongQuery, VmQuery},
            vm::ReadObserver,
        },
    };
    use std::{cell::Cell, collections::BTreeMap};
    struct Reads {
        count: Rc<Cell<u32>>,
        deny: bool,
    }
    impl ReadObserver for Reads {
        fn on_read(&mut self, slot: &VarSlotRef) -> Result<(), Failure> {
            if slot.name() == intern_sym("cut") {
                self.count.set(self.count.get() + 1);
                if self.deny {
                    return Err(Failure::new(
                        FailCode::HostUnavailable,
                        "callback reread after issued retention",
                    ));
                }
            }
            Ok(())
        }
    }
    struct Fixture {
        evaluator: Evaluator,
        original: Rc<Song>,
        reads: Rc<Cell<u32>>,
    }
    fn limits() -> SongLimits {
        SongLimits {
            max_nodes: 1_000_000,
            ..SongLimits::default()
        }
    }
    fn window(a: i64, b: i64, d: i64) -> TimeSpan {
        TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
    }
    fn program(body: &str, ending: &str) -> String {
        format!("fn cut beat:\n\tfirst [0]\nfn indexed p:\n\t{body}\nlet base {{part [drums: {{s :analog > chord [:c :five]}}] duration: 2}}\nlet selected {{transform-instrument base :drums :analog indexed}}\n{ending}")
    }
    impl Fixture {
        fn code(code: &str) -> Self {
            let factory =
                DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
            let assets = SongAssetLimits {
                max_resources: 64,
                max_pcm_bytes: 1_000_000,
                max_source_files: 32,
                max_source_bytes: 100_000,
                max_banks: 32,
                max_walk_nodes: 100_000,
                max_walk_depth: 256,
            };
            evaluate_song_candidate(
                &format!("{code} > play-song"),
                "issued.vact",
                7,
                SnapshotEpoch(91),
                &CandidateBuildCtx {
                    assets: &factory,
                    asset_limits: assets,
                    lock: None,
                    cache: None,
                },
            )
            .unwrap();
            let preparation = factory
                .begin(
                    SongSourceFile {
                        file: FileId::new(0),
                        path: crate::value::value::PathVal {
                            text: "issued.vact".into(),
                            file: None,
                        },
                    },
                    assets,
                )
                .unwrap();
            let mut evaluator = Evaluator::new(
                Prelude::core(),
                preparation.source_loader(),
                Box::new(RecordingSink::default()),
            );
            let forms = evaluator.eval_str(code, FileId::new(0)).unwrap();
            assert!(
                forms.iter().all(|form| form.value.is_ok()),
                "genuine forms: {:?}",
                forms.iter().map(|form| &form.value).collect::<Vec<_>>()
            );
            let closed = preparation.close().unwrap();
            let mut freeze = Freeze::new(&closed, assets);
            let Value::Song(original) = freeze
                .value(forms.last().unwrap().value.as_ref().unwrap())
                .unwrap()
            else {
                panic!("actual frozen Song")
            };
            crate::session::song::capture_original_test_routing(&evaluator, &original, assets)
                .unwrap();
            let reads = Rc::new(Cell::new(0));
            evaluator
                .vm_and_ns()
                .0
                .set_read_observer(Some(Box::new(Reads {
                    count: reads.clone(),
                    deny: false,
                })));
            Self {
                evaluator,
                original,
                reads,
            }
        }
        fn work(&self, remaining: u32) -> SharedIndexWork {
            let work = CanonicalIndexCollector::new(remaining, limits()).unwrap();
            work.borrow_mut().original = Some(self.original.clone());
            work
        }
        fn deny(&mut self) {
            self.reads.set(0);
            self.evaluator
                .vm_and_ns()
                .0
                .set_read_observer(Some(Box::new(Reads {
                    count: self.reads.clone(),
                    deny: true,
                })));
        }
        fn retain(&mut self) -> Rc<ReplayView> {
            self.retain_window(window(
                0,
                self.original.duration().num(),
                self.original.duration().den(),
            ))
        }
        fn retain_window(&mut self, span: TimeSpan) -> Rc<ReplayView> {
            let work = self.work(limits().max_nodes);
            let cells = InputCells::default();
            let (vm, ns) = self.evaluator.vm_and_ns();
            let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
            let policy = limits();
            super::super::observe_part(
                self.original.part(),
                span,
                &mut super::super::SongQueryCtx {
                    vm: &mut adapter,
                    cells: &cells,
                    seed: self.original.settings().seed,
                    tempo: self.original.settings().tempo().unwrap(),
                    limits: &policy,
                },
                work.clone(),
                0,
            )
            .unwrap();
            ReplayView::publish(self.original.clone(), &work).unwrap()
        }
        fn issued(
            &mut self,
            span: TimeSpan,
            view: Option<Rc<ReplayView>>,
            remaining: u32,
            depth: u32,
        ) -> (SharedIndexWork, Result<IssuedQueryBatch, Failure>) {
            let work = self.work(remaining);
            let cells = InputCells::default();
            let policy = limits();
            let (vm, ns) = self.evaluator.vm_and_ns();
            let mut adapter = MeteredSongQuery::new(vm, ns, work.clone());
            let result = query_part_issued(
                &self.original,
                span,
                &mut super::super::SongQueryCtx {
                    vm: &mut adapter,
                    cells: &cells,
                    seed: self.original.settings().seed,
                    tempo: self.original.settings().tempo().unwrap(),
                    limits: &policy,
                },
                work.clone(),
                depth,
                view,
            );
            (work, result)
        }
        fn ordinary(&mut self, span: TimeSpan) -> Vec<SongEvent> {
            let cells = InputCells::default();
            let policy = limits();
            let (vm, ns) = self.evaluator.vm_and_ns();
            let mut adapter = VmQuery::new(vm, ns);
            super::super::query_part(
                self.original.part(),
                span,
                &mut super::super::SongQueryCtx {
                    vm: &mut adapter,
                    cells: &cells,
                    seed: self.original.settings().seed,
                    tempo: self.original.settings().tempo().unwrap(),
                    limits: &policy,
                },
            )
            .unwrap()
        }
    }
    fn compare(rows: &[IssuedQueryRow], expected: &[SongEvent]) {
        assert_eq!(rows.len(), expected.len());
        for (actual, expected) in rows.iter().zip(expected) {
            assert_eq!(actual.handle, expected.handle);
            assert_eq!(actual.track, expected.track);
            assert_eq!(actual.instrument, expected.instrument);
            assert_eq!(actual.placement, expected.placement);
            assert_eq!(actual.tone, expected.tone);
            assert_eq!(actual.route, expected.route);
            assert_eq!(actual.commit_mode, expected.commit_mode);
            assert_eq!(actual.event.whole, expected.event.whole);
            assert_eq!(actual.event.part, expected.event.part);
            assert_eq!(actual.event.producer, expected.event.producer);
            assert_eq!(actual.event.occ, expected.event.occ);
            assert_eq!(actual.event.src, expected.event.src);
            match (&actual.event.late, &expected.event.late) {
                (Some(a), Some(b)) => assert!(a.same(b)),
                (None, None) => {}
                _ => panic!("late source changed"),
            }
            assert_eq!(actual.event.cells.len(), expected.event.cells.len());
            for ((ak, av), (bk, bv)) in actual.event.cells.iter().zip(&expected.event.cells) {
                assert_eq!(ak, bk);
                assert!(av.same(bv));
            }
            assert!(crate::value::eq::deep_eq(&actual.event.value, &expected.event.value).unwrap());
            assert_eq!(actual.event.controls.len(), expected.event.controls.len());
            for ((ak, av), (bk, bv)) in actual.event.controls.iter().zip(&expected.event.controls) {
                assert_eq!(ak, bk);
                assert!(crate::value::eq::deep_eq(av, bv).unwrap());
            }
            let origin = |row: &SongEvent| {
                row.event
                    .song_source
                    .as_ref()
                    .map(|origin| copy_origin(origin, &mut 1_000_000, 256).unwrap())
            };
            assert_eq!(origin(actual), origin(expected));
        }
    }
    #[test]
    fn genuine_direct_chord_issues_one_primary_invocation_independent_of_observer() {
        let mut fixture = Fixture::code(
            "song {part [drums: {s :analog > chord [:c :five]}] duration: 1} tail-seconds: 0",
        );
        let expected = fixture.ordinary(window(0, 1, 1));
        let (work, batch) = fixture.issued(window(0, 1, 1), None, limits().max_nodes, 0);
        let batch = batch.unwrap();
        compare(&batch.rows, &expected);
        assert_eq!(batch.rows.len(), 2);
        let seal = &batch.rows[0].contributors[0];
        assert!(Rc::ptr_eq(seal, &batch.rows[1].contributors[0]));
        assert!(batch
            .transcript
            .authentic(&fixture.original, seal, &work, 0)
            .unwrap());
        assert!(batch
            .transcript
            .invocation(seal, &work, 0)
            .unwrap()
            .observations()
            .is_empty());
    }
    #[test]
    fn required_cached_owner_shares_raw_execution_but_issues_fresh_invocation() {
        let mut fixture = Fixture::code(&program(
            "slice p 2 [cut nil]",
            "song selected tail-seconds: 0",
        ));
        let expected = fixture.ordinary(window(0, 1, 1));
        let partial = fixture.retain_window(window(0, 1, 1));
        let view = fixture.retain();
        let mut foreign = Fixture::code(&program(
            "slice p 2 [cut nil]",
            "song selected tail-seconds: 0",
        ));
        let foreign_view = foreign.retain();
        fixture.deny();
        let (_, rejected) =
            fixture.issued(window(0, 1, 1), Some(foreign_view), limits().max_nodes, 0);
        assert_eq!(rejected.err().unwrap().code, FailCode::Type);
        assert_eq!(fixture.reads.get(), 0);
        let (_, missing) = fixture.issued(window(1, 2, 1), Some(partial), limits().max_nodes, 0);
        assert_eq!(missing.err().unwrap().code, FailCode::Type);
        assert_eq!(fixture.reads.get(), 0);
        let (work, first) =
            fixture.issued(window(0, 1, 1), Some(view.clone()), limits().max_nodes, 0);
        let first = first.unwrap();
        compare(&first.rows, &expected);
        let (second_work, second) =
            fixture.issued(window(0, 1, 1), Some(view), limits().max_nodes, 0);
        let second = second.unwrap();
        compare(&second.rows, &expected);
        assert_eq!(fixture.reads.get(), 0);
        let a = &first.rows[0].contributors[0];
        let b = &second.rows[0].contributors[0];
        assert!(!Rc::ptr_eq(a, b));
        let a = first.transcript.invocation(a, &work, 0).unwrap();
        let b = second.transcript.invocation(b, &second_work, 0).unwrap();
        assert!(Rc::ptr_eq(a.execution(), b.execution()));
        assert_eq!(a.owner(), b.owner());
        assert_eq!(a.entry(), b.entry());
        assert_eq!(a.seed(), b.seed());
    }
    #[test]
    fn cached_outer_rebinds_all_nested_event_and_boundary_members_through_one_memo() {
        let code="fn cut beat:\n\tfirst [0]\nfn inner p:\n\tslice p 2 [cut nil]\nfn outer p:\n\tstack [{slice p 2 [cut nil]} {slice p 2 [cut nil]}]\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 2}\nlet inner-part {transform-instrument base :drums :analog inner}\nlet selected {transform-instrument inner-part :drums :analog outer}\nsong selected tail-seconds: 0";
        let mut fixture = Fixture::code(code);
        let expected = fixture.ordinary(window(0, 1, 1));
        let view = fixture.retain();
        fixture.deny();
        let (work, batch) = fixture.issued(window(0, 1, 1), Some(view), limits().max_nodes, 0);
        let batch = batch.unwrap();
        compare(&batch.rows, &expected);
        assert_eq!(fixture.reads.get(), 0);
        let mut members = 0;
        for row in &batch.rows {
            let mut origin = row.event.song_source.as_ref();
            while let Some(actual) = origin {
                let bag = actual.issued_leaves.as_ref().unwrap();
                for member in batch.transcript.source_members(bag, &work, 0).unwrap() {
                    assert!(
                        Rc::ptr_eq(
                            member.member().issued_leaves.as_ref().unwrap(),
                            actual.issued_leaves.as_ref().unwrap()
                        ),
                        "Slice metadata clone shares its authentic fresh original member bag"
                    );
                    assert!(member.is_sealed_member(&work, 0).unwrap());
                    fn names_member(
                        clock: &crate::pattern::eval::song_clock::CanonicalClockProjection,
                        boundary: &Rc<crate::pattern::eval::song_clock::SelectedSourceBoundary>,
                        member: &Rc<crate::song::source::SongEventOrigin>,
                        depth: u32,
                    ) -> bool {
                        assert!(depth <= limits().max_depth);
                        clock.source_evidence().iter().any(|evidence| {
                            (Rc::ptr_eq(&evidence.4, boundary)
                                && evidence
                                    .3
                                    .iter()
                                    .any(|returned| Rc::ptr_eq(returned, member)))
                                || names_member(&evidence.2, boundary, member, depth + 1)
                        })
                    }
                    assert!(
                        names_member(member.child().clock(), member.boundary(), member.member(), 0),
                        "repeated clock mapping and transcript binding share the exact sealed boundary/member Rc, including genuine parent chains"
                    );
                    let boundary = member.boundary();
                    assert!(batch
                        .transcript
                        .source_members(bag, &work, 0)
                        .unwrap()
                        .iter()
                        .any(|other| Rc::ptr_eq(other.boundary(), boundary)));
                    assert!(member.child().authentic_original(&fixture.original));
                    assert!(member.parent().authentic_original(&fixture.original));
                    members += 1;
                }
                origin = actual.inherited.as_ref();
            }
        }
        assert!(members > 0);
    }
    #[test]
    fn successful_empty_selected_source_has_completion_without_fabricated_members() {
        let mut fixture=Fixture::code(&program("slice {beat -> nil} 2 [cut nil]","song {transform-instrument selected :drums :analog {p -> slice p 2 [cut nil]}} tail-seconds: 0"));
        let view = fixture.retain();
        fixture.deny();
        let (work, batch) = fixture.issued(window(1, 2, 2), Some(view), limits().max_nodes, 0);
        let batch = batch.unwrap();
        assert!(batch.rows.is_empty());
        assert!(batch.transcript.empty_source_completions(&work, 0).unwrap() > 0);
        assert_eq!(fixture.reads.get(), 0);
    }
    #[test]
    fn fractional_reordered_continuations_union_every_authentic_contribution() {
        let mut fixture=Fixture::code(&program("slow {slice {beat -> p} 1 [cut]} 2","let intro {part [drums: nil] duration: 1/2}\nsong {sequence [intro selected]} tail-seconds: 0"));
        let expected = fixture.ordinary(window(0, 5, 2));
        let view = fixture.retain();
        fixture.deny();
        let (work, batch) =
            fixture.issued(window(0, 5, 2), Some(view.clone()), limits().max_nodes, 0);
        let batch = batch.unwrap();
        compare(&batch.rows, &expected);
        batch.transcript.check_rows(&batch.rows, &work, 0).unwrap();
        assert!(batch.rows.iter().any(|row| {
            let owners:Vec<_>=row.contributors.iter().map(|seal|batch.transcript.invocation(seal,&work,0).unwrap()).collect();
            owners.iter().enumerate().any(|(i,a)|owners[..i].iter().any(|b|a.owner().revision==b.owner().revision && a.owner().placement==b.owner().placement && a.entry()==b.entry() && a.owner().window!=b.owner().window))
        }),"same musical handle retains genuine outer q contributions from distinct canonical windows");
        assert!(
            batch.rows.iter().any(|row| {
                row.source_contributions.iter().enumerate().any(|(i, a)| {
                    row.source_contributions[..i].iter().any(|b| {
                        !Rc::ptr_eq(&a.augmented_origin, &b.augmented_origin)
                            && a.augmented_origin.handle == b.augmented_origin.handle
                    })
                })
            }),
            "equal-handle union retains the actual augmented origins from distinct genuine queries"
        );
        let mut discarded_augmented = 0;
        for row in &batch.rows {
            for contribution in &row.source_contributions {
                let augmented = &contribution.augmented_origin;
                assert!(Rc::ptr_eq(
                    augmented.issued_leaves.as_ref().unwrap(),
                    &contribution.leaves
                ));
                let members = batch
                    .transcript
                    .source_members(&contribution.leaves, &work, 0)
                    .unwrap();
                let actual: Vec<_> = members
                    .iter()
                    .filter(|member| {
                        member.member().handle == augmented.handle
                            && member.member().issued_handle == augmented.issued_handle
                            && member.member().entry_trace == augmented.entry_trace
                            && Rc::ptr_eq(
                                member.member().issued_leaves.as_ref().unwrap(),
                                &contribution.leaves,
                            )
                    })
                    .collect();
                let raw = actual.first().unwrap_or_else(|| panic!(
                    "no authentic raw member for origin handle={:?}, issued={:?}, entry={:?}, timings={:?}; candidates={:?}",
                    augmented.handle, augmented.issued_handle, augmented.entry_trace, augmented.slice_timings,
                    members.iter().map(|member| (&member.member().handle, &member.member().issued_handle, &member.member().entry_trace, &member.member().slice_timings)).collect::<Vec<_>>()
                )).member();
                assert!(
                    actual.iter().all(|member| Rc::ptr_eq(member.member(), raw)),
                    "all genuine contributor bindings name the same raw member"
                );
                let mut comparison_work = limits().max_nodes;
                let frozen =
                    copy_origin(augmented, &mut comparison_work, limits().max_depth).unwrap();
                let sealed = copy_origin(raw, &mut comparison_work, limits().max_depth).unwrap();
                assert_eq!(frozen.handle, sealed.handle);
                assert_eq!(frozen.issued_handle, sealed.issued_handle);
                assert_eq!(frozen.source_part, sealed.source_part);
                assert_eq!(frozen.source_whole(), sealed.source_whole());
                assert_eq!(frozen.original_instrument, sealed.original_instrument);
                assert_eq!(frozen.entry_trace, sealed.entry_trace);
                assert_eq!(frozen.inherited, sealed.inherited);
                assert_eq!(augmented.route, raw.route);
                assert_eq!(augmented.commit_mode, raw.commit_mode);
                assert!(
                    frozen.slice_timings().starts_with(sealed.slice_timings()),
                    "post-seal append preserves every original timing: augmented={:?}, sealed={:?}",
                    frozen.slice_timings(),
                    sealed.slice_timings()
                );
                let mut surviving = row.event.song_source.as_ref();
                let mut survives = false;
                while let Some(origin) = surviving {
                    survives |= Rc::ptr_eq(origin, augmented);
                    surviving = origin.inherited.as_ref();
                }
                if !survives && frozen.slice_timings().len() > sealed.slice_timings().len() {
                    discarded_augmented += 1;
                }
            }
        }
        assert!(discarded_augmented > 0,
            "equal-handle union must retain a genuine discarded post-seal timing contribution; actual contributions={:?}",
            batch.rows.iter().flat_map(|row| &row.source_contributions).map(|pair| (&pair.augmented_origin.handle, &pair.augmented_origin.issued_handle, &pair.augmented_origin.slice_timings)).collect::<Vec<_>>());
        let (_, last) = fixture.issued(window(3, 5, 2), Some(view.clone()), limits().max_nodes, 0);
        last.unwrap();
        let (_, first) = fixture.issued(window(0, 3, 2), Some(view), limits().max_nodes, 0);
        first.unwrap();
        assert_eq!(fixture.reads.get(), 0);
    }
    #[test]
    fn same_and_vary_placements_and_foreign_leafs_keep_exact_original_authority() {
        for mode in [":same", ":vary"] {
            let mut fixture = Fixture::code(&program(
                "stack [{slice p 2 [cut nil]} {slice p 2 [cut nil]}]",
                &format!("song {{part-repeat selected 2 seed-mode: {mode}}} tail-seconds: 0"),
            ));
            let view = fixture.retain();
            fixture.deny();
            let (work, batch) = fixture.issued(window(0, 4, 1), Some(view), limits().max_nodes, 0);
            let batch = batch.unwrap();
            assert!(batch.rows.iter().any(|row| !row.sources.is_empty()));
            let owners: Vec<_> = batch
                .rows
                .iter()
                .map(|row| {
                    batch
                        .transcript
                        .invocation(&row.contributors[0], &work, 0)
                        .unwrap()
                })
                .collect();
            let first = owners[0];
            let different = owners
                .iter()
                .find(|owner| owner.owner().placement != first.owner().placement)
                .expect("two actual repeat placements");
            assert_eq!(first.owner().revision, different.owner().revision);
            assert_eq!(first.owner().root, different.owner().root);
            if mode == ":same" {
                assert_eq!(first.seed(), different.seed());
            } else {
                assert_ne!(first.seed(), different.seed());
            }
            let members: Vec<_> = batch
                .rows
                .iter()
                .flat_map(|row| row.sources.iter())
                .flat_map(|bag| batch.transcript.source_members(bag, &work, 0).unwrap())
                .collect();
            assert!(members.iter().enumerate().any(|(i,a)|members[..i].iter().any(|b|a.member().entry_trace!=b.member().entry_trace && !Rc::ptr_eq(a.boundary(),b.boundary()))),"actual same-policy source uses keep distinct incoming producer traces and sealed boundaries");
            let foreign = Fixture::code(&program(
                "slice p 2 [cut nil]",
                "song selected tail-seconds: 0",
            ));
            assert!(!batch
                .transcript
                .authentic(&foreign.original, &batch.rows[0].contributors[0], &work, 0)
                .unwrap());
            let mut swapped = batch.rows[0].clone();
            swapped.contributors = vec![batch.rows.last().unwrap().contributors[0].clone()];
            assert_eq!(
                batch
                    .transcript
                    .check_rows(&[swapped], &work, 0)
                    .unwrap_err()
                    .code,
                FailCode::Type
            );
            let first = batch
                .rows
                .iter()
                .find(|row| !row.source_contributions.is_empty())
                .unwrap();
            let original = &first.source_contributions[0];
            let foreign_pair = batch
                .rows
                .iter()
                .flat_map(|row| &row.source_contributions)
                .find(|pair| {
                    !Rc::ptr_eq(&pair.leaves, &original.leaves)
                        || !Rc::ptr_eq(&pair.augmented_origin, &original.augmented_origin)
                })
                .expect("genuine distinct original source contribution");
            let mut swapped_pair = first.clone();
            swapped_pair.source_contributions[0] = foreign_pair.clone();
            assert_eq!(
                batch
                    .transcript
                    .check_rows(&[swapped_pair], &work, 0)
                    .unwrap_err()
                    .code,
                FailCode::Type
            );
            assert_eq!(fixture.reads.get(), 0);
        }
    }
    #[test]
    fn genuine_delete_overwrite_and_siblings_preserve_descriptor_and_binding_sets() {
        let mut fixture=Fixture::code(&program("slice p 2 [cut nil]","let rows {part-events selected :drums 0 1}\nlet handle {{first rows} :handle}\nlet removed {delete-event selected handle}\nlet changed {overwrite-region removed :drums 1/2 1 {s :analog > note 72}}\nsong changed tail-seconds: 0"));
        let expected = fixture.ordinary(window(0, 2, 1));
        let view = fixture.retain();
        fixture.deny();
        let (work, batch) = fixture.issued(window(0, 2, 1), Some(view), limits().max_nodes, 0);
        let batch = batch.unwrap();
        compare(&batch.rows, &expected);
        batch.transcript.check_rows(&batch.rows, &work, 0).unwrap();
        assert!(batch
            .rows
            .iter()
            .any(|row| row.tone.is_some_and(|note| note.to_f64() == 72.0)));
        assert_eq!(fixture.reads.get(), 0);
    }
    #[test]
    fn whole_issued_work_exact_one_less_depth_fault_and_leaf_drop_are_genuine() {
        let mut fixture = Fixture::code(&program(
            "slice p 2 [cut nil]",
            "song selected tail-seconds: 0",
        ));
        let view = fixture.retain();
        fixture.deny();
        let (work, measured) =
            fixture.issued(window(0, 1, 1), Some(view.clone()), limits().max_nodes, 0);
        let measured = measured.unwrap();
        let cost = limits().max_nodes - work.borrow().remaining();
        let leaf = Rc::downgrade(&measured.rows[0].contributors[0]);
        drop(measured);
        assert!(leaf.upgrade().is_none());
        let (exact, result) = fixture.issued(window(0, 1, 1), Some(view.clone()), cost, 0);
        result.unwrap();
        assert_eq!(exact.borrow().remaining(), 0);
        let (_, result) = fixture.issued(window(0, 1, 1), Some(view.clone()), cost - 1, 0);
        assert_eq!(result.err().unwrap().code, FailCode::FuelExhausted);
        let (_, result) = fixture.issued(
            window(0, 1, 1),
            Some(view.clone()),
            limits().max_nodes,
            limits().max_depth,
        );
        assert_eq!(result.err().unwrap().code, FailCode::DepthExceeded);
        let (fault_work, fault) = fixture.issued(window(0, 1, 1), None, limits().max_nodes, 0);
        assert_eq!(fault.err().unwrap().code, FailCode::HostUnavailable);
        assert!(fault_work.borrow().remaining() < limits().max_nodes);
        assert_eq!(fault_work.borrow().depth, 0);
        assert!(fixture.reads.get() > 0);
        fixture.deny();
        let (_, result) = fixture.issued(window(0, 1, 1), Some(view), limits().max_nodes, 0);
        result.unwrap();
        assert_eq!(fixture.reads.get(), 0);
    }
}
