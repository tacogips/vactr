use super::SongTransport;
use crate::dsp::arena::StoreKind;
use crate::dsp::caps::CapabilitySet;
use crate::host::caps::{
    AudioHost, SongHostPreparation, SongPreparationLimits, SongPreparationProgress, SongReadyBundle,
};
use crate::host::native::audio::{NativeAudioHost, MAX_BLOCK};
use crate::host::wire::HostMsg;
use crate::pattern::eval::song_observation::{CanonicalIndexCollector, SharedIndexWork};
use crate::pattern::TimeSpan;
use crate::song::{prepare_song, PreparedSong, SnapshotEpoch, SongLimits};
use crate::value::ratio::Ratio64;

const PROGRAM: &str = "song {part [drums: {s :analog > note 60 > gain 0.2} hats: {s :analog > note 67 > gain 0.2}] duration: 1} tail-seconds: 0 > play-song";

fn prepared() -> PreparedSong {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(30_000);
    let assets = crate::song::assets::DecodedSongAssetFactory::new(
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::new(),
    );
    let context = crate::session::song::CandidateBuildCtx {
        assets: &assets,
        asset_limits: crate::song::assets::SongAssetLimits {
            max_resources: 64,
            max_pcm_bytes: 1_000_000,
            max_source_files: 16,
            max_source_bytes: 100_000,
            max_banks: 16,
            max_walk_nodes: 10_000,
            max_walk_depth: 128,
        },
        lock: None,
        cache: None,
    };
    prepare_song(
        crate::session::song::evaluate_song_candidate(
            PROGRAM,
            "realize-tests.vact",
            42,
            SnapshotEpoch(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            &context,
        )
        .unwrap(),
    )
    .unwrap()
}

fn limits() -> SongPreparationLimits {
    SongPreparationLimits {
        capabilities: CapabilitySet::native(),
        song: SongLimits::default(),
        max_resources: 64,
        max_pending_records: 512,
        max_graph_bytes: 1_000_000,
        max_work: 8_000_000,
    }
}

fn ready() -> SongReadyBundle {
    let caps = CapabilitySet::native();
    let config = crate::host::song_profile::song_engine_config(
        8000.,
        MAX_BLOCK,
        caps,
        StoreKind::NativeArc,
        2,
    )
    .unwrap();
    let (mut host, mut side) = NativeAudioHost::headless_with_config(config, 4096).unwrap();
    let mut owner = SongHostPreparation::begin(prepared(), limits())
        .unwrap_or_else(|refusal| panic!("preparation refused: {}", refusal.failure));
    let mut silence = [0.0; MAX_BLOCK * 2];
    for _ in 0..2048 {
        owner.submit(&mut host).unwrap();
        side.render(&mut silence, 2);
        let mut messages = Vec::new();
        host.drain(&mut messages);
        for message in messages {
            if let HostMsg::Song(ack) = message {
                if let Err(unhandled) = owner.receive(ack) {
                    panic!("preparation returned an unowned acknowledgement: {unhandled:?}");
                }
            }
        }
        if owner.progress() == SongPreparationProgress::Ready {
            return owner.take_ready().unwrap();
        }
    }
    panic!("preparation did not reach Ready: {:?}", owner.progress());
}

fn span(ready: &SongReadyBundle) -> TimeSpan {
    TimeSpan::new(Ratio64::ZERO, ready.prepared().snapshot().duration()).unwrap()
}

fn work(limit: u32, policy: SongLimits) -> SharedIndexWork {
    CanonicalIndexCollector::new(limit, policy).unwrap()
}

fn pending_snapshot(transport: &SongTransport) -> Vec<String> {
    transport
        .pending
        .iter()
        .map(|command| format!("{command:?}"))
        .collect()
}

#[test]
fn realize_failure_after_staging_preserves_pools_queue_cursor_and_receipts() {
    let mut transport = SongTransport::new(ready(), 0, SongLimits::default())
        .unwrap_or_else(|refusal| panic!("transport refused: {}", refusal.failure));
    transport.pools.inject_assign_fault(Some(1));
    let pools_before = transport.pools.projection();
    let pending_before = pending_snapshot(&transport);
    let cursor_before = transport.cursor;
    let error = transport.realize(transport.duration).unwrap_err();
    assert_eq!(error.message, "injected staging fault");
    assert_eq!(transport.pools.projection(), pools_before);
    assert_eq!(pending_snapshot(&transport), pending_before);
    assert_eq!(transport.cursor, cursor_before);

    transport.pools.inject_assign_fault(None);
    transport.realize(transport.duration).unwrap();
    assert!(!transport.pending.is_empty());
    let recovered = transport.pools.projection();

    let mut control = SongTransport::new(ready(), 0, SongLimits::default())
        .unwrap_or_else(|refusal| panic!("control transport refused: {}", refusal.failure));
    control.realize(control.duration).unwrap();
    let expected = control.pools.projection();
    assert_eq!(
        recovered
            .iter()
            .map(|slot| (slot.1, slot.2, slot.3, slot.4.len()))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|slot| (slot.1, slot.2, slot.3, slot.4.len()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn realize_work_failure_preserves_state() {
    let policy = SongLimits {
        max_nodes: 1,
        ..SongLimits::default()
    };
    let mut transport = SongTransport::new(ready(), 0, policy)
        .unwrap_or_else(|refusal| panic!("transport refused: {}", refusal.failure));
    let pools_before = transport.pools.projection();
    let pending_before = pending_snapshot(&transport);
    let cursor_before = transport.cursor;
    assert!(transport.realize(transport.duration).is_err());
    assert_eq!(transport.pools.projection(), pools_before);
    assert_eq!(pending_snapshot(&transport), pending_before);
    assert_eq!(transport.cursor, cursor_before);
}

#[test]
fn foreign_issued_batch_is_refused_by_ready() {
    let owner = ready();
    let mut foreign = ready();
    let owner_work = work(SongLimits::default().max_nodes, SongLimits::default());
    let foreign_work = work(SongLimits::default().max_nodes, SongLimits::default());
    let batch = foreign
        .query_issued_with_work(span(&foreign), &foreign_work, 0)
        .unwrap();
    assert!(owner.resolve_issued(&batch, 0, &owner_work, 0).is_err());
    assert!(foreign.resolve_issued(&batch, 0, &foreign_work, 0).is_ok());
}

fn consume_window(
    ready: &mut SongReadyBundle,
    limit: u32,
) -> Result<u32, crate::vm::fail::Failure> {
    let work = work(limit, SongLimits::default());
    let remaining_before = work.borrow().remaining();
    let batch = ready.query_issued_with_work(span(ready), &work, 0)?;
    let after_query = work.borrow().remaining();
    assert!(after_query < remaining_before);
    for index in 0..batch.events().len() {
        let before = work.borrow().remaining();
        ready.resolve_issued(&batch, index, &work, 0)?;
        assert!(work.borrow().remaining() <= before);
    }
    let spent = limit - work.borrow().remaining();
    Ok(spent)
}

#[test]
fn one_collector_spans_query_and_every_resolution() {
    let policy = SongLimits::default();
    let budget = policy.max_nodes;
    let mut measured = ready();
    let exact_work = consume_window(&mut measured, budget).unwrap();
    assert!(exact_work > 0);

    let mut exact = ready();
    assert_eq!(consume_window(&mut exact, exact_work).unwrap(), exact_work);

    let mut short = ready();
    assert!(consume_window(&mut short, exact_work - 1).is_err());
}
