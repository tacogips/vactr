//! Opaque original-snapshot canonical observations; admission follows TASK-002.
use super::SongSnapshot;
#[cfg(test)]
mod geometry_tests;
pub(crate) mod route_view;
pub(crate) use route_view::RouteAuthorityView;
pub(crate) mod lookup;
#[cfg(test)]
mod lookup_tests;
use crate::pattern::eval::{
    song_observation::{
        event_copy_work, CanonicalIndexCollector, CanonicalIndexObservation, CanonicalIndexTarget,
        RetainedQueryCall,
    },
    InputCells,
};
use crate::pattern::occ::ProducerTrace;
use crate::pattern::query::{sect, TimeSpan};
use crate::reader::span::NodeId;
use crate::song::source_uses::{timing::FrozenIndexTiming, FrozenUseTraceTerm};
use crate::song::{PartRevision, Song, SongLimits};
use crate::value::intern::KwId;
use crate::vm::fail::{FailCode, Failure};
use std::rc::Rc;

pub(crate) struct CanonicalIndexRequest {
    original: Rc<Song>,
    scope: usize,
    track: KwId,
    revision: PartRevision,
    root: NodeId,
    recipe: Rc<FrozenIndexTiming>,
    issuer: NodeId,
    prefix: Vec<FrozenUseTraceTerm>,
    window: TimeSpan,
    depth: u32,
}
impl CanonicalIndexRequest {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn mint(
        snapshot: &SongSnapshot,
        scope: usize,
        track: KwId,
        revision: PartRevision,
        root: NodeId,
        recipe: Rc<FrozenIndexTiming>,
        issuer: NodeId,
        prefix: &[FrozenUseTraceTerm],
        window: TimeSpan,
        depth: u32,
    ) -> Result<Self, Failure> {
        TimeSpan::new(window.begin, window.end)?;
        if window.begin < crate::value::Ratio64::ZERO || window.end > snapshot.duration() {
            return Err(invalid("canonical window outside original song"));
        }
        Ok(Self {
            original: snapshot.song.clone(),
            scope,
            track,
            revision,
            root,
            recipe,
            issuer,
            prefix: prefix.to_vec(),
            window,
            depth,
        })
    }
    fn same_execution(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.original, &other.original)
            && Rc::ptr_eq(&self.recipe, &other.recipe)
            && self.scope == other.scope
            && self.track == other.track
            && self.revision == other.revision
            && self.root == other.root
            && self.window == other.window
            && self.depth == other.depth
    }
    fn matches(&self, row: &CanonicalIndexObservation) -> bool {
        row.owner.revision == self.revision
            && row.owner.track == self.track
            && row.owner.root == self.root
            && row.issuer == self.issuer
            && trace_matches(&self.prefix, &row.prefix)
    }
}
pub(crate) struct RetainedCanonicalIndex {
    request: CanonicalIndexRequest,
    observations: Vec<CanonicalIndexObservation>,
    #[allow(dead_code)] // Genuine bound invocation authority consumed by next immutable lookup.
    invocations: Vec<Rc<crate::pattern::eval::song_observation::OwnerInvocation>>,
    // Execution-ordered journal scoped to this complete request/window. It is
    // not a callable+arguments cache and never coalesces distinct query sites.
    #[allow(dead_code)] // TASK-002 consumes the original execution journal.
    calls: Vec<RetainedQueryCall>,
    peak_depth: u32,
    #[allow(dead_code)] // Metered cost evidence for the next immutable consumer.
    vm_instructions: u64,
    admitted_depth: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CanonicalIndexReadiness {
    #[allow(dead_code)] // Uniform consumer proof is not supplied by observation.
    OriginalOwnerComplete,
    RequiresUniformBound,
}

#[allow(dead_code)] // TASK-002 installs this original pre-Reserve entrypoint.
pub(crate) fn retain_index_occupancy(
    snapshot: &mut SongSnapshot,
    requests: Vec<CanonicalIndexRequest>,
    limits: SongLimits,
    remaining: &mut u32,
) -> Result<CanonicalIndexReadiness, Failure> {
    limits.validate()?;
    let work = CanonicalIndexCollector::new(*remaining, limits)?;
    work.borrow_mut().original = Some(snapshot.song.clone());
    let result = (|| {
        if let Some(view) = &snapshot.replay {
            view.seed_collection(&snapshot.song, &work)?;
        }
        work.borrow_mut().charge(requests.len() as u64 + 1)?;
        let mut pending = Vec::<RetainedCanonicalIndex>::new();
        for request in requests {
            work.borrow_mut().charge(1)?;
            if !Rc::ptr_eq(&request.original, &snapshot.song) {
                return Err(invalid("foreign canonical snapshot authority"));
            }
            let part = snapshot
                .routing
                .parts
                .get(request.scope)
                .ok_or_else(|| invalid("canonical topology owner missing"))?;
            if part.revision != request.revision || !part.tracks.contains(&request.track) {
                return Err(invalid("canonical topology owner changed"));
            }
            // A successful mint was authenticated through the original prepared
            // path. Strong immutable Song and recipe identities are retained.
            let mut cached = None;
            for record in snapshot.occupancy.iter().chain(&pending) {
                work.borrow_mut().charge(request.prefix.len() as u64 + 1)?;
                if record.request.same_execution(&request) {
                    cached = Some(record);
                    break;
                }
            }
            if let Some(record) = cached {
                if record.peak_depth > limits.max_depth || record.admitted_depth > limits.max_depth
                {
                    return Err(Failure::new(
                        FailCode::DepthExceeded,
                        "cached canonical depth exceeds caller",
                    ));
                }
                continue;
            }
            work.borrow_mut().target = Some(CanonicalIndexTarget {
                revision: request.revision,
                track: request.track,
                root: request.root,
            });
            crate::pattern::eval::song_replay::prepare_owner_dependencies(
                &snapshot.routing,
                request.scope,
                request.track,
                request.depth,
                &work,
            )?;
            let cells = InputCells::default();
            let settings = *snapshot.song.settings();
            let (vm, ns) = snapshot.evaluator.vm_and_ns();
            let mut adapter = crate::vm::query_vm::MeteredSongQuery::new(vm, ns, work.clone());
            let mut context = crate::song::SongQueryCtx {
                vm: &mut adapter,
                cells: &cells,
                seed: settings.seed,
                tempo: settings.tempo()?,
                limits: &limits,
            };
            crate::song::query::observe_part(
                snapshot.song.part(),
                request.window,
                &mut context,
                work.clone(),
                request.depth,
            )?;
            let mut collector = work.borrow_mut();
            let rows = std::mem::take(&mut collector.observations);
            let calls = std::mem::take(&mut collector.calls);
            let invocations = std::mem::take(&mut collector.invocations);
            collector.charge(rows.len() as u64 + 1)?;
            let mut observations = Vec::new();
            for row in rows {
                collector.charge(request.prefix.len() as u64 + 1)?;
                if row.owner.revision == request.revision
                    && row.owner.track == request.track
                    && row.owner.root == request.root
                {
                    collector.charge(1)?;
                    observations.push(row);
                }
            }
            limits.check_events(
                snapshot
                    .occupancy
                    .len()
                    .checked_add(pending.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| invalid("canonical cache length overflow"))?,
            )?;
            collector.charge(1)?;
            pending.push(RetainedCanonicalIndex {
                request,
                observations,
                invocations,
                calls,
                peak_depth: collector.peak_depth,
                vm_instructions: collector.vm_instructions,
                admitted_depth: limits.max_depth,
            });
        }
        // Publish only the complete successful batch. Moves retain strong
        // immutable payloads; no callback result is reconstructed or forged.
        work.borrow_mut().charge(pending.len() as u64)?;
        let view =
            crate::pattern::eval::song_replay::ReplayView::publish(snapshot.song.clone(), &work)?;
        snapshot.occupancy.extend(pending);
        snapshot.replay = Some(view);
        Ok(CanonicalIndexReadiness::RequiresUniformBound)
    })();
    *remaining = work.borrow().remaining();
    result
}
impl RetainedCanonicalIndex {
    #[allow(dead_code)] // Next consumer reads original canonical records.
    pub(crate) fn replay(
        &self,
        window: TimeSpan,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<Vec<CanonicalIndexObservation>, Failure> {
        self.replay_site(&self.request, window, limits, remaining)
    }
    #[allow(dead_code)] // Addressed immutable consumer, TASK-002.
    pub(crate) fn replay_site(
        &self,
        request: &CanonicalIndexRequest,
        window: TimeSpan,
        limits: SongLimits,
        remaining: &mut u32,
    ) -> Result<Vec<CanonicalIndexObservation>, Failure> {
        if !self.request.same_execution(request) {
            return Err(invalid("foreign canonical replay address"));
        }
        limits.validate()?;
        if self.peak_depth > limits.max_depth || self.admitted_depth > limits.max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "canonical replay depth exceeded",
            ));
        }
        if window.begin < self.request.window.begin || window.end > self.request.window.end {
            return Err(invalid("replay outside retained original window"));
        }
        let mut result = Vec::new();
        for row in &self.observations {
            debit(remaining, request.prefix.len() as u64 + 1)?;
            if !request.matches(row) {
                continue;
            }
            // Observation clocks are issuer-local; map only the owner window
            // to original root time for canonical-window selection. Replay
            // returns the original uncut issuer-clock record, not guessed
            // affine event geometry. Later consumers authenticate both clocks.
            let root_part = row.owner.window.map(|t| t.checked_add(row.owner.offset))?;
            if sect(root_part, window).is_some() {
                limits.check_events(result.len() + 1)?;
                debit(
                    remaining,
                    event_copy_work(&row.event)?
                        .checked_add(row.prefix.steps.len() as u64)
                        .and_then(|n| n.checked_add(row.owner.placement.0.len() as u64 + 1))
                        .ok_or_else(|| invalid("canonical copy overflow"))?,
                )?;
                result.push(row.clone());
            }
        }
        Ok(result)
    }
}
pub(super) fn query_rows(
    snapshot: &mut SongSnapshot,
    span: TimeSpan,
    limits: &SongLimits,
    remaining: &mut u32,
) -> Result<Vec<crate::song::SongEvent>, Failure> {
    let Some(view) = snapshot.replay.clone() else {
        return snapshot.with_query(|vm, song| {
            let cells = InputCells::default();
            let settings = *song.settings();
            crate::song::query_part(
                song.part(),
                span,
                &mut crate::song::SongQueryCtx {
                    vm,
                    cells: &cells,
                    seed: settings.seed,
                    tempo: settings.tempo()?,
                    limits,
                },
            )
        });
    };
    let work = CanonicalIndexCollector::new(*remaining, *limits)?;
    let result = (|| {
        view.seed_collection(&snapshot.song, &work)?;
        let cells = InputCells::default();
        let settings = *snapshot.song.settings();
        let (vm, ns) = snapshot.evaluator.vm_and_ns();
        let mut adapter = crate::vm::query_vm::MeteredSongQuery::new(vm, ns, work.clone());
        let mut cx = crate::song::SongQueryCtx {
            vm: &mut adapter,
            cells: &cells,
            seed: settings.seed,
            tempo: settings.tempo()?,
            limits,
        };
        crate::song::query::query_part_with_replay(
            snapshot.song.part(),
            span,
            &mut cx,
            work.clone(),
            view,
        )
    })();
    *remaining = work.borrow().remaining();
    result
}

fn debit(remaining: &mut u32, cost: u64) -> Result<(), Failure> {
    let cost = u32::try_from(cost).map_err(|_| invalid("canonical copy cost overflow"))?;
    *remaining = remaining
        .checked_sub(cost)
        .ok_or_else(|| Failure::new(FailCode::FuelExhausted, "canonical replay work exhausted"))?;
    Ok(())
}
fn trace_matches(expected: &[FrozenUseTraceTerm], actual: &ProducerTrace) -> bool {
    expected.len() == actual.steps.len()
        && expected
            .iter()
            .zip(&actual.steps)
            .all(|(term, step)| match term {
                FrozenUseTraceTerm::Exact(exact) => exact == step,
                FrozenUseTraceTerm::Copies { kind, count } => {
                    *kind == step.kind && step.ordinal < *count
                }
            })
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}

/// Test-only access to the owning snapshot's genuine publication transaction.
#[cfg(test)]
pub(crate) fn assert_clock_retention_transaction(fault: bool) {
    use crate::song::source_uses::FrozenUseOperation;
    use crate::value::intern::intern_kw;
    let cut = if fault { "/ 0 {- 2 beat}" } else { "first [0]" };
    let code = format!("fn cut beat:\n\t{cut}\nfn factor beat:\n\tfirst [2]\nfn indexed p:\n\tfast {{rev {{slice {{beat -> p}} 2 [cut nil]}}}} factor\nlet base {{part [drums: {{s :analog}}] duration: 4}}\nlet selected {{transform-instrument base :drums :analog indexed}}\nsong selected tail-seconds: 0 > play-song");
    let limits = SongLimits::default();
    // This fixture traversal finds a genuine address; request minting itself
    // charges the production authentication work into the tested counter.
    let issue = |snapshot: &SongSnapshot, cycle, remaining: &mut u32| {
        let (scope, payload) = snapshot
            .routing
            .parts
            .iter()
            .enumerate()
            .find_map(|(i, part)| match &part.node {
                super::FrozenPartNode::Edit {
                    edit: super::FrozenEdit::Transform { payload, .. },
                    ..
                } => Some((i, payload)),
                _ => None,
            })
            .expect("actual transformed owner");
        let recipe = payload.index_timing().unwrap();
        let mut pending = vec![(recipe.root(), Vec::new())];
        let (issuer, prefix) = loop {
            let (index, prefix) = pending.pop().expect("actual Slice recipe address");
            let node = &recipe.nodes()[index as usize];
            if node.operation() == FrozenUseOperation::Slice {
                break (node.issuer(), prefix);
            }
            for edge in node.children() {
                let mut next = prefix.clone();
                next.extend_from_slice(edge.trace());
                pending.push((edge.child(), next));
            }
        };
        snapshot
            .canonical_index_request(
                scope,
                intern_kw("drums"),
                issuer,
                &prefix,
                TimeSpan::cycle(cycle).unwrap(),
                0,
                limits,
                remaining,
            )
            .unwrap()
    };
    let seeded = || {
        let mut prepared = tests::from_code(&code);
        let mut remaining = limits.max_nodes;
        let request = issue(&prepared.snapshot, 0, &mut remaining);
        retain_index_occupancy(
            &mut prepared.snapshot,
            vec![request],
            limits,
            &mut remaining,
        )
        .unwrap();
        assert_eq!(prepared.snapshot.occupancy.len(), 1);
        assert!(!prepared.snapshot.occupancy[0].observations.is_empty());
        prepared
    };
    let mut measured = seeded();
    let mut remaining = limits.max_nodes;
    let request = issue(&measured.snapshot, 1, &mut remaining);
    let outcome = retain_index_occupancy(
        &mut measured.snapshot,
        vec![request],
        limits,
        &mut remaining,
    );
    if fault {
        assert_eq!(outcome.unwrap_err().code, FailCode::DivisionByZero);
    } else {
        outcome.unwrap();
    }
    let required = limits.max_nodes - remaining;
    assert!(required > 1);
    for budget in if fault {
        vec![limits.max_nodes]
    } else {
        vec![required, required - 1]
    } {
        let mut prepared = seeded();
        let prior = prepared
            .snapshot
            .replay
            .clone()
            .expect("genuine prior published view");
        let prior_window = prepared.snapshot.occupancy[0].request.window;
        let prior_whole = prepared.snapshot.occupancy[0].observations[0].event.whole;
        let mut remaining = budget;
        let request = issue(&prepared.snapshot, 1, &mut remaining);
        let outcome = retain_index_occupancy(
            &mut prepared.snapshot,
            vec![request],
            limits,
            &mut remaining,
        );
        if !fault && budget == required {
            outcome.unwrap();
            assert_eq!(remaining, 0, "exact complete transaction work");
            assert_eq!(prepared.snapshot.occupancy.len(), 2);
            assert!(!Rc::ptr_eq(
                &prior,
                prepared.snapshot.replay.as_ref().unwrap()
            ));
        } else {
            let expected = if fault {
                FailCode::DivisionByZero
            } else {
                FailCode::FuelExhausted
            };
            assert_eq!(outcome.unwrap_err().code, expected);
            assert!(
                remaining < budget,
                "failed transaction keeps actual consumed work"
            );
            assert!(Rc::ptr_eq(
                &prior,
                prepared.snapshot.replay.as_ref().unwrap()
            ));
            assert_eq!(prepared.snapshot.occupancy.len(), 1);
            assert_eq!(prepared.snapshot.occupancy[0].request.window, prior_window);
            assert_eq!(
                prepared.snapshot.occupancy[0].observations[0].event.whole,
                prior_whole
            );
        }
        assert!(prepared
            .snapshot
            .evaluator
            .vm_and_ns()
            .0
            .song_work()
            .is_none());
    }
}

#[cfg(test)]
mod replay_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod invocation_tests;
