//! Private candidate ownership and guarded preparation state contracts.
//! The certified constructor is crate-only: SONG-07 supplies fresh evaluation
//! and transitive dependency freezing. Neither this phase nor an interim session
//! request certifies an arbitrary active evaluator as a frozen song.
use std::rc::Rc;

use super::assets::PinnedSongAssets;
use super::{SnapshotEpoch, Song, SongSettings};
use crate::ns::evaluator::Evaluator;
use crate::value::ratio::Ratio64;
use crate::value::sample::SampleBuf;
use crate::vm::fail::{FailCode, Failure};

mod cells;
pub(crate) mod issued;
pub(crate) mod occupancy;
mod resources;
mod routing;
pub use super::source_uses::{FrozenPattern, FrozenSelectedSource};
pub use cells::{
    FrozenAnalysisBank, FrozenAnalysisRange, FrozenCellInventory, FrozenCellOwner,
    FrozenCellReference, FrozenCellSite, FrozenCellValue,
};
pub use resources::{FrozenGraphOwner, FrozenGraphResource, FrozenGraphResources};
pub use routing::{
    FrozenAudioRoute, FrozenEdit, FrozenInstrument, FrozenPart, FrozenPartNode,
    FrozenRoutingInventory, FrozenSource, FrozenTrackStage,
};

/// Candidate ownership is transferred once, never shallow-cloned from a session.
pub struct SongCandidate {
    evaluator: Evaluator,
    song: Rc<Song>,
    assets: PinnedSongAssets,
    samples: Vec<Rc<SampleBuf>>,
    file: String,
    revision: u64,
    edit_epoch: u64,
    epoch: SnapshotEpoch,
    routing: FrozenRoutingInventory,
    warnings: Vec<String>,
}
impl SongCandidate {
    /// Only the isolated whole-code builder may certify this ownership transfer.
    /// All retained buffers and closure/registry slots must already be private.
    #[allow(dead_code, clippy::too_many_arguments)]
    pub(crate) fn from_isolated_evaluation(
        evaluator: Evaluator,
        song: Rc<Song>,
        assets: PinnedSongAssets,
        samples: Vec<Rc<SampleBuf>>,
        file: String,
        revision: u64,
        edit_epoch: u64,
        epoch: SnapshotEpoch,
    ) -> Result<Self, Failure> {
        if file.is_empty() || file.contains('\0') {
            return Err(invalid("candidate file is invalid"));
        }
        song.settings().validate()?;
        Ok(Self {
            evaluator,
            song,
            assets,
            samples,
            file,
            revision,
            edit_epoch,
            epoch,
            routing: FrozenRoutingInventory::default(),
            warnings: Vec::new(),
        })
    }
    pub(crate) fn set_warnings(&mut self, warnings: Vec<String>) {
        self.warnings = warnings;
    }
    pub(crate) fn set_routing(&mut self, routing: FrozenRoutingInventory) {
        self.routing = routing;
    }
    #[must_use]
    pub const fn epoch(&self) -> SnapshotEpoch {
        self.epoch
    }
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}
/// Owns the candidate VM/registry, closed inventory and private captured buffers.
/// No public evaluator, registry, Part/Song Rc or buffer accessor can leak them.
/// ```compile_fail
/// fn cannot_leak(snapshot: &mut vactr::song::SongSnapshot) {
///     let _ = &mut snapshot.evaluator;
/// }
/// ```
pub struct SongSnapshot {
    evaluator: Evaluator,
    song: Rc<Song>,
    assets: PinnedSongAssets,
    samples: Vec<Rc<SampleBuf>>,
    file: String,
    revision: u64,
    edit_epoch: u64,
    epoch: SnapshotEpoch,
    routing: FrozenRoutingInventory,
    warnings: Vec<String>,
    occupancy: Vec<occupancy::RetainedCanonicalIndex>,
    replay: Option<Rc<crate::pattern::eval::song_replay::ReplayView>>,
}
impl SongSnapshot {
    pub(crate) fn closed_sample(
        &self,
        source: &crate::host::caps::SampleSrc,
    ) -> Result<std::sync::Arc<crate::host::caps::SampleData>, Failure> {
        self.assets.closed_sample(source)
    }
    pub(crate) fn closed_bank_geometry(
        &self,
        bank: crate::value::intern::KwId,
    ) -> Option<(u32, bool)> {
        self.assets.closed_bank_geometry(bank)
    }
    /// Accepted isolated candidate warnings, retained without evaluator access.
    #[must_use]
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    #[must_use]
    pub const fn epoch(&self) -> SnapshotEpoch {
        self.epoch
    }
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }
    #[must_use]
    pub fn duration(&self) -> Ratio64 {
        self.song.duration()
    }
    #[must_use]
    pub fn settings(&self) -> SongSettings {
        *self.song.settings()
    }
    #[must_use]
    pub fn resource_count(&self) -> usize {
        self.assets.resource_count()
    }
    #[must_use]
    pub fn captured_buffer_count(&self) -> usize {
        self.samples.len()
    }
    pub fn routing(&self) -> &FrozenRoutingInventory {
        &self.routing
    }
    pub fn pcm_bytes(&self) -> u64 {
        self.assets.pcm_bytes()
    }
    /// Immutable decoded PCM from the certified closed inventory only.
    pub fn sample(
        &mut self,
        source: &crate::host::caps::SampleSrc,
    ) -> Result<std::sync::Arc<crate::host::caps::SampleData>, Failure> {
        crate::host::caps::SampleLoader::load(&mut self.assets, source)
    }
    /// Public realized descriptors copy identities and notes, never mutable
    /// VM, registry, pattern or buffer values. Queries remain lazy and bounded.
    pub fn query(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        limits: &super::SongLimits,
    ) -> Result<Vec<FrozenSongEvent>, Failure> {
        let mut remaining = limits.max_nodes;
        let rows = occupancy::query_rows(self, span, limits, &mut remaining)?;
        issued::freeze_public(rows, limits, &mut remaining)
    }
    #[cfg_attr(not(test), allow(dead_code))] // Required issued routing handoff follows this stage.
    pub(crate) fn query_issued(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        limits: &super::SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<issued::FrozenIssuedBatch, Failure> {
        issued::query_issued(self, span, limits, remaining, depth)
    }
    pub(crate) fn query_issued_with_work(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        work: &crate::pattern::eval::song_observation::SharedIndexWork,
        depth: u32,
    ) -> Result<issued::FrozenIssuedBatch, Failure> {
        issued::query_issued_with_work(self, span, work, depth)
    }
    /// Query access is kept crate-private; only a certified snapshot may enter.
    /// The caller receives an effect-restricted immediate borrow, not the VM.
    #[allow(dead_code)]
    pub(crate) fn with_query<R>(
        &mut self,
        f: impl FnOnce(&mut dyn crate::pattern::eval::QueryVm, &Song) -> R,
    ) -> R {
        let (vm, ns) = self.evaluator.vm_and_ns();
        let mut query = crate::vm::query_vm::VmQuery::new(vm, ns);
        f(&mut query, &self.song)
    }
}
/// A generation-qualified reservation, not proof that a host has acknowledged it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SongResourceLease {
    pub resource: u32,
    pub generation: u32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongPreparationState {
    Evaluating,
    Preparing,
    Ready,
    Applied,
    Failed,
}
/// An actual application acknowledgement is distinct from candidate readiness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SongApplyAck {
    pub epoch: SnapshotEpoch,
    pub application_frame: u64,
    pub doc_revision: u64,
}
/// State is private: a caller cannot turn preparation into applied playback.
pub struct PreparedSong {
    snapshot: SongSnapshot,
    resources: Vec<SongResourceLease>,
    reservations_initialized: bool,
    state: SongPreparationState,
    failure: Option<Failure>,
    application: Option<SongApplyAck>,
}
/// Transfers a certified candidate into Preparing, never Ready or Applied.
/// Actual route admission and host acknowledgements remain subsequent phases.
pub fn prepare_song(candidate: SongCandidate) -> Result<PreparedSong, Failure> {
    let SongCandidate {
        evaluator,
        song,
        assets,
        samples,
        file,
        revision,
        edit_epoch,
        epoch,
        routing,
        warnings,
    } = candidate;
    Ok(PreparedSong {
        snapshot: SongSnapshot {
            occupancy: Vec::new(),
            replay: None,
            evaluator,
            song,
            assets,
            samples,
            file,
            revision,
            edit_epoch,
            epoch,
            routing,
            warnings,
        },
        resources: Vec::new(),
        reservations_initialized: false,
        state: SongPreparationState::Preparing,
        failure: None,
        application: None,
    })
}
impl PreparedSong {
    pub fn sample(
        &mut self,
        source: &crate::host::caps::SampleSrc,
    ) -> Result<std::sync::Arc<crate::host::caps::SampleData>, Failure> {
        self.snapshot.sample(source)
    }
    pub fn snapshot(&self) -> &SongSnapshot {
        &self.snapshot
    }
    /// Lazy descriptor query; no mutable snapshot internals are exposed.
    pub fn query(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        limits: &super::SongLimits,
    ) -> Result<Vec<FrozenSongEvent>, Failure> {
        self.snapshot.query(span, limits)
    }
    #[cfg_attr(not(test), allow(dead_code))] // Required scheduler handoff consumes these envelopes next.
    pub(crate) fn query_issued(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        limits: &super::SongLimits,
        remaining: &mut u32,
        depth: u32,
    ) -> Result<issued::FrozenIssuedBatch, Failure> {
        self.snapshot.query_issued(span, limits, remaining, depth)
    }
    pub(crate) fn query_issued_with_work(
        &mut self,
        span: crate::pattern::query::TimeSpan,
        work: &crate::pattern::eval::song_observation::SharedIndexWork,
        depth: u32,
    ) -> Result<issued::FrozenIssuedBatch, Failure> {
        self.snapshot.query_issued_with_work(span, work, depth)
    }
    #[must_use]
    pub const fn state(&self) -> SongPreparationState {
        self.state
    }
    #[must_use]
    pub fn epoch(&self) -> SnapshotEpoch {
        self.snapshot.epoch
    }
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.snapshot.revision
    }
    #[must_use]
    pub fn resources(&self) -> &[SongResourceLease] {
        &self.resources
    }
    #[must_use]
    pub fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }
    #[must_use]
    pub const fn application(&self) -> Option<SongApplyAck> {
        self.application
    }
    /// Reserve before Ready. Duplicate IDs, even with different generations,
    /// cannot represent independent current reservations for one resource slot.
    #[allow(dead_code)]
    pub(crate) fn reserve(&mut self, resources: Vec<SongResourceLease>) -> Result<(), Failure> {
        self.require(SongPreparationState::Preparing)?;
        if self.reservations_initialized {
            return Err(invalid("song reservations already initialized"));
        }
        if resources.len() > crate::dsp::arena::MAX_RESOURCES {
            return Err(invalid("too many song resource leases"));
        }
        let mut ids = std::collections::BTreeSet::new();
        if resources.iter().any(|r| !ids.insert(r.resource)) {
            return Err(invalid("duplicate song resource lease"));
        }
        self.resources = resources;
        self.reservations_initialized = true;
        Ok(())
    }
    /// Reserve complete leases under the owner's original resource and work limits.
    /// Failure preserves snapshot ownership and reservation state; work is not refunded.
    #[allow(dead_code)]
    pub(crate) fn reserve_bounded(
        &mut self,
        resources: Vec<SongResourceLease>,
        max_resources: u32,
        remaining: &mut u32,
    ) -> Result<(), Failure> {
        let exhausted = || Failure::new(FailCode::FuelExhausted, "song reservation work exhausted");
        *remaining = remaining.checked_sub(1).ok_or_else(exhausted)?;
        self.require(SongPreparationState::Preparing)?;
        if self.reservations_initialized {
            return Err(invalid("song reservations already initialized"));
        }
        let count =
            u32::try_from(resources.len()).map_err(|_| invalid("too many song resource leases"))?;
        if count > max_resources {
            return Err(invalid("too many song resource leases"));
        }
        let count = u64::from(count);
        let comparisons = count
            .checked_mul(count.saturating_sub(1))
            .and_then(|product| product.checked_div(2))
            .ok_or_else(exhausted)?;
        let work = count
            .checked_mul(2)
            .and_then(|scalars| scalars.checked_add(comparisons))
            .and_then(|cost| u32::try_from(cost).ok())
            .ok_or_else(exhausted)?;
        *remaining = remaining.checked_sub(work).ok_or_else(exhausted)?;
        for (index, lease) in resources.iter().enumerate() {
            if resources[..index]
                .iter()
                .any(|previous| previous.resource == lease.resource)
            {
                return Err(invalid("duplicate song resource lease"));
            }
        }
        self.resources = resources;
        self.reservations_initialized = true;
        Ok(())
    }
    /// Called by the host adapter after route admission and actual acknowledgements.
    #[allow(dead_code)]
    pub(crate) fn acknowledge_ready(
        &mut self,
        acknowledgements: &[SongResourceLease],
    ) -> Result<(), Failure> {
        self.require(SongPreparationState::Preparing)?;
        if !self.reservations_initialized {
            return Err(invalid("song route reservations are not initialized"));
        }
        if acknowledgements.len() != self.resources.len()
            || acknowledgements.iter().any(|r| !self.resources.contains(r))
        {
            return Err(invalid(
                "song resource acknowledgements do not match reservations",
            ));
        }
        let unique: std::collections::BTreeSet<_> = acknowledgements.iter().collect();
        if unique.len() != self.resources.len() {
            return Err(invalid("duplicate song resource acknowledgement"));
        }
        self.state = SongPreparationState::Ready;
        Ok(())
    }
    #[allow(dead_code)]
    pub(crate) fn acknowledge_applied(&mut self, ack: SongApplyAck) -> Result<(), Failure> {
        self.require(SongPreparationState::Ready)?;
        if ack.epoch != self.epoch() || ack.doc_revision != self.revision() {
            return Err(invalid("stale song application acknowledgement"));
        }
        self.application = Some(ack);
        self.state = SongPreparationState::Applied;
        Ok(())
    }
    /// Edits invalidate pending candidates, while an already applied song keeps
    /// playing until a separately acknowledged replacement. Reservations remain
    /// retained for the host's generation-aware cleanup, even after failure.
    #[allow(dead_code)]
    pub(crate) fn cancel(&mut self, epoch: SnapshotEpoch) -> Result<(), Failure> {
        if self.epoch() != epoch
            || !matches!(
                self.state,
                SongPreparationState::Preparing | SongPreparationState::Ready
            )
        {
            return Err(invalid("song cancellation is stale or not pending"));
        }
        self.state = SongPreparationState::Failed;
        self.failure = Some(Failure::new(
            FailCode::HostUnavailable,
            "song candidate cancelled",
        ));
        Ok(())
    }
    pub(crate) fn cancel_if_changed(&mut self, file: &str, revision: u64, edit_epoch: u64) -> bool {
        if self.snapshot.file == file
            && matches!(
                self.state,
                SongPreparationState::Preparing | SongPreparationState::Ready
            )
            && (self.revision() != revision || self.snapshot.edit_epoch != edit_epoch)
        {
            self.failure = Some(invalid(
                "song candidate invalidated by document revision/edit epoch",
            ));
            self.state = SongPreparationState::Failed;
            return true;
        }
        false
    }
    /// Muting is valid only for the addressed already-applied generation.
    pub fn validate_mute_epoch(&self, epoch: SnapshotEpoch) -> Result<(), Failure> {
        if epoch != self.epoch() {
            return Err(invalid("stale song mute epoch"));
        }
        self.require(SongPreparationState::Applied)
    }
    fn require(&self, state: SongPreparationState) -> Result<(), Failure> {
        if self.state != state {
            return Err(Failure::new(
                FailCode::HostUnavailable,
                "song preparation is not in the required state",
            ));
        }
        Ok(())
    }
}
fn invalid(message: &str) -> Failure {
    Failure::new(FailCode::Type, message)
}

#[cfg(test)]
#[path = "snapshot/reservations_tests.rs"]
mod reservations_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
    use crate::ns::load::SourceLoader;
    use crate::ns::namespace::Prelude;
    use crate::ns::stage::RecordingSink;
    use crate::reader::span::FileId;
    use crate::song::assets::{SongAssetBackend, SongAssetLimits, SongAssetPreparation};
    use crate::song::{capture_part, SongSettings};
    use crate::value::intern::{intern_sym, KwId};
    use crate::value::value::{PathVal, Value};
    use std::collections::BTreeMap;
    use std::sync::Arc;
    struct Empty;
    impl SourceLoader for Empty {
        fn read(&mut self, _: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
            Err(invalid("no source"))
        }
    }
    impl SampleLoader for Empty {
        fn load(&mut self, _: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
            Err(invalid("no samples"))
        }
    }
    impl SongAssetBackend for Empty {
        fn bank_len(&mut self, _: KwId, _: u32) -> Result<u32, Failure> {
            Err(invalid("no banks"))
        }
    }
    pub(super) fn candidate() -> SongCandidate {
        let mut evaluator = Evaluator::new(
            Prelude::core(),
            Box::new(crate::host::noop::NoopHost),
            Box::new(RecordingSink::default()),
        );
        let outcomes = evaluator.eval_str("var value 7", FileId::new(1)).unwrap();
        assert!(outcomes.iter().all(|o| o.value.is_ok()));
        let assets = SongAssetPreparation::new(
            Box::new(Empty),
            SongAssetLimits {
                max_resources: 0,
                max_pcm_bytes: 0,
                max_source_files: 0,
                max_source_bytes: 0,
                max_banks: 0,
                max_walk_nodes: 16,
                max_walk_depth: 4,
            },
        )
        .unwrap()
        .close()
        .unwrap();
        let song = Rc::new(
            Song::new(
                Rc::new(capture_part(BTreeMap::new(), Ratio64::ONE).unwrap()),
                SongSettings::default(),
            )
            .unwrap(),
        );
        SongCandidate::from_isolated_evaluation(
            evaluator,
            song,
            assets,
            Vec::new(),
            "score.vact".into(),
            4,
            8,
            SnapshotEpoch(9),
        )
        .unwrap()
    }
    #[test]
    fn preparing_is_not_ready_and_reservations_cannot_be_lost() {
        let mut song = prepare_song(candidate()).unwrap();
        assert_eq!(song.state(), SongPreparationState::Preparing);
        assert!(song.application().is_none());
        assert!(song.acknowledge_ready(&[]).is_err());
        let leases = vec![
            SongResourceLease {
                resource: 3,
                generation: 7,
            },
            SongResourceLease {
                resource: 8,
                generation: 2,
            },
        ];
        song.reserve(leases.clone()).unwrap();
        assert!(song.reserve(vec![]).is_err());
        assert_eq!(song.resources(), leases);
        assert!(song.acknowledge_ready(&[leases[0]]).is_err());
        assert!(song.acknowledge_ready(&[leases[0], leases[0]]).is_err());
        assert!(song
            .acknowledge_ready(&[
                leases[0],
                SongResourceLease {
                    resource: 8,
                    generation: 3
                }
            ])
            .is_err());
        assert_eq!(song.state(), SongPreparationState::Preparing);
        song.acknowledge_ready(&leases).unwrap();
        assert_eq!(song.state(), SongPreparationState::Ready);
        assert!(song.application().is_none());
        assert!(song.validate_mute_epoch(SnapshotEpoch(9)).is_err());
    }
    #[test]
    fn application_requires_ready_and_current_epoch_revision() {
        let mut song = prepare_song(candidate()).unwrap();
        let ack = SongApplyAck {
            epoch: SnapshotEpoch(9),
            application_frame: u64::MAX,
            doc_revision: 4,
        };
        assert!(song.acknowledge_applied(ack).is_err());
        song.reserve(Vec::new()).unwrap();
        song.acknowledge_ready(&[]).unwrap();
        for invalid in [
            SongApplyAck {
                epoch: SnapshotEpoch(10),
                ..ack
            },
            SongApplyAck {
                doc_revision: 3,
                ..ack
            },
        ] {
            assert!(song.acknowledge_applied(invalid).is_err());
        }
        assert_eq!(song.state(), SongPreparationState::Ready);
        song.acknowledge_applied(ack).unwrap();
        assert_eq!(song.state(), SongPreparationState::Applied);
        assert_eq!(song.application(), Some(ack));
        assert!(song.acknowledge_applied(ack).is_err());
        assert!(song.validate_mute_epoch(SnapshotEpoch(8)).is_err());
        song.validate_mute_epoch(SnapshotEpoch(9)).unwrap();
        assert!(!song.cancel_if_changed("score.vact", 5, 9));
    }
    #[test]
    fn pending_revision_or_edit_epoch_invalidates_without_losing_leases() {
        for (revision, edit) in [(5, 8), (4, 9)] {
            let mut song = prepare_song(candidate()).unwrap();
            let leases = vec![SongResourceLease {
                resource: 4,
                generation: 3,
            }];
            song.reserve(leases.clone()).unwrap();
            assert!(!song.cancel_if_changed("other.vact", revision, edit));
            assert!(!song.cancel_if_changed("score.vact", 4, 8));
            song.acknowledge_ready(&leases).unwrap();
            assert!(song.cancel_if_changed("score.vact", revision, edit));
            assert_eq!(song.state(), SongPreparationState::Failed);
            assert!(song.failure().is_some());
            assert_eq!(song.resources(), leases);
            assert!(song.acknowledge_ready(&leases).is_err());
            assert!(song.validate_mute_epoch(SnapshotEpoch(9)).is_err());
        }
    }
    #[test]
    fn empty_reservations_are_explicit_and_invalid_ones_do_not_consume_admission() {
        let mut song = prepare_song(candidate()).unwrap();
        assert!(song
            .reserve(vec![
                SongResourceLease {
                    resource: 1,
                    generation: 1
                },
                SongResourceLease {
                    resource: 1,
                    generation: 2
                }
            ])
            .is_err());
        assert!(song.resources().is_empty());
        assert!(song
            .reserve(vec![
                SongResourceLease {
                    resource: 1,
                    generation: 1
                };
                257
            ])
            .is_err());
        song.reserve(vec![]).unwrap();
        assert!(song.reserve(vec![]).is_err());
        song.acknowledge_ready(&[]).unwrap();
    }
    #[test]
    fn fresh_fixture_metadata_and_query_borrow_do_not_alias_active_namespace() {
        let mut active = Evaluator::new(
            Prelude::core(),
            Box::new(crate::host::noop::NoopHost),
            Box::new(RecordingSink::default()),
        );
        active.eval_str("var value 100", FileId::new(1)).unwrap();
        let mut prepared = prepare_song(candidate()).unwrap();
        assert!(!Rc::ptr_eq(
            &active.insts().unwrap(),
            &prepared.snapshot.evaluator.insts().unwrap()
        ));
        active
            .ns()
            .session_slot(intern_sym("value"))
            .unwrap()
            .set(Value::Int(200));
        let slot = prepared
            .snapshot
            .evaluator
            .ns()
            .session_slot(intern_sym("value"))
            .unwrap();
        let result = prepared
            .snapshot
            .with_query(|vm, _| vm.deref(&slot))
            .unwrap();
        assert!(matches!(result, Value::Int(7)));
        let mut settings = prepared.snapshot.settings();
        settings.seed = 999;
        assert_eq!(prepared.snapshot.settings().seed, 0);
        assert_eq!(prepared.snapshot.file(), "score.vact");
        assert_eq!(prepared.snapshot.duration(), Ratio64::ONE);
        assert_eq!(prepared.snapshot.resource_count(), 0);
        assert_eq!(prepared.snapshot.captured_buffer_count(), 0);
    }
    #[test]
    fn private_captured_pcm_survives_original_buffer_mutation() {
        let original = SampleBuf::ready(48000, Rc::<[f32]>::from([0.25, 0.5]));
        let mut preparation = SongAssetPreparation::new(
            Box::new(Empty),
            SongAssetLimits {
                max_resources: 1,
                max_pcm_bytes: 8,
                max_source_files: 0,
                max_source_bytes: 0,
                max_banks: 0,
                max_walk_nodes: 16,
                max_walk_depth: 4,
            },
        )
        .unwrap();
        let copy = preparation.pin_buffer(&original).unwrap();
        let assets = preparation.close().unwrap();
        let captured_id = copy.id;
        let mut owned = candidate();
        owned.assets = assets;
        owned.samples = vec![copy];
        let mut prepared = prepare_song(owned).unwrap();
        original.fill_at(24000, Rc::<[f32]>::from([0.75, 0.9]));
        original.fail(FailCode::Type, "changed original");
        let data = prepared
            .snapshot
            .assets
            .load(&SampleSrc::Buffer { id: captured_id })
            .unwrap();
        assert_eq!(data.rate, 48000);
        assert_eq!(&*data.frames, &[0.25, 0.5]);
        assert_eq!(prepared.snapshot.samples[0].rate(), 48000);
        assert_eq!(
            &*prepared.snapshot.samples[0].ready_frames().unwrap(),
            &[0.25, 0.5]
        );
    }
}

/// Pure audio identity. Buffers are opaque private copied IDs, never handles.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrozenSound {
    Builtin(crate::value::intern::KwId),
    Instrument(crate::dsp::graph::InstId),
    Sample {
        file: Option<crate::reader::span::FileId>,
        path: Rc<str>,
    },
    Buffer(u64),
}
impl FrozenSound {
    pub(crate) fn from_sound(sound: &crate::value::value::Sound) -> Result<Self, Failure> {
        use crate::value::value::Sound;
        Ok(match sound {
            Sound::Builtin(k) => Self::Builtin(*k),
            Sound::Inst(id) => Self::Instrument(*id),
            Sound::Sample(p) => Self::Sample {
                file: p.file,
                path: p.text.clone(),
            },
            Sound::Buffer(b) => Self::Buffer(b.id),
            _ => return Err(invalid("external sound cannot enter frozen metadata")),
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct FrozenSongEvent {
    pub source_origin: Option<super::source_uses::FrozenSourceOrigin>,
    pub handle: super::EventHandle,
    pub track: crate::value::intern::KwId,
    pub whole: Option<crate::pattern::query::TimeSpan>,
    pub part: crate::pattern::query::TimeSpan,
    pub instrument: FrozenSound,
    pub note: Option<super::ResolvedNote>,
    pub controls: Vec<(crate::value::intern::KwId, FrozenControl)>,
    pub route: Option<(Vec<FrozenSound>, crate::value::intern::KwId)>,
}

/// Resolved controls contain no callable, signal, global cell or buffer alias.
#[derive(Clone, Debug, PartialEq)]
pub enum FrozenControl {
    Nil,
    Bool(bool),
    Number(super::ResolvedNote),
    Keyword(crate::value::intern::KwId),
    String(Rc<str>),
    List(Vec<FrozenControl>),
}
impl FrozenControl {
    fn copy(
        value: &crate::value::Value,
        depth: u32,
        max_depth: u32,
        remaining: &mut u32,
    ) -> Result<Self, Failure> {
        use crate::value::Value;
        if depth >= max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "frozen control depth exhausted",
            ));
        }
        *remaining = remaining.checked_sub(1).ok_or_else(|| {
            Failure::new(FailCode::FuelExhausted, "frozen control work exhausted")
        })?;
        Ok(match value {
            Value::Nil => Self::Nil,
            Value::Bool(v) => Self::Bool(*v),
            Value::Keyword(v) => Self::Keyword(*v),
            Value::Str(v) => Self::String(v.clone()),
            Value::List(v) => {
                if v.items.len() > *remaining as usize {
                    return Err(Failure::new(
                        FailCode::FuelExhausted,
                        "control collection exceeds remaining work",
                    ));
                }
                Self::List(
                    v.items
                        .iter()
                        .map(|v| Self::copy(v, depth + 1, max_depth, remaining))
                        .collect::<Result<_, _>>()?,
                )
            }
            Value::Int(_)
            | Value::Int64(_)
            | Value::Float(_)
            | Value::Float64(_)
            | Value::Ratio(_) => Self::Number(super::ResolvedNote::try_from(value)?),
            _ => {
                return Err(invalid(
                    "song control is not a resolved immutable scalar/list",
                ));
            }
        })
    }
}

fn control_admission(count: usize, remaining: &mut u32) -> Result<(), Failure> {
    let count = u32::try_from(count).map_err(|_| invalid("control collection length overflow"))?;
    *remaining = remaining.checked_sub(count).ok_or_else(|| {
        Failure::new(
            FailCode::FuelExhausted,
            "frozen control collection work exhausted",
        )
    })?;
    Ok(())
}
#[cfg(test)]
mod control_tests {
    use super::*;
    use crate::value::{value::ListVal, Value};
    fn nested(depth: usize) -> Value {
        (0..depth).fold(Value::Int(1), |v, _| {
            Value::List(Rc::new(ListVal {
                items: vec![v].into(),
                prov: None,
            }))
        })
    }
    #[test]
    fn control_depth_uses_caller_policy_on_normal_stack() {
        assert!(FrozenControl::copy(&nested(200), 0, 256, &mut 1000).is_ok());
        assert_eq!(
            FrozenControl::copy(&nested(300), 0, 256, &mut 1000)
                .unwrap_err()
                .code,
            FailCode::DepthExceeded
        );
        assert_eq!(
            FrozenControl::copy(&nested(3), 0, 3, &mut 1000)
                .unwrap_err()
                .code,
            FailCode::DepthExceeded
        );
    }
    #[test]
    fn controls_share_work_and_admit_wide_lists_before_allocation() {
        let shared = nested(10);
        let mut remaining = 15;
        FrozenControl::copy(&shared, 0, 256, &mut remaining).unwrap();
        assert_eq!(
            FrozenControl::copy(&shared, 0, 256, &mut remaining)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        let wide = Value::List(Rc::new(ListVal {
            items: vec![Value::Int(1); 1000].into(),
            prov: None,
        }));
        assert_eq!(
            FrozenControl::copy(&wide, 0, 256, &mut 16)
                .unwrap_err()
                .code,
            FailCode::FuelExhausted
        );
        assert!(control_admission(20, &mut 16).is_err());
    }
}
