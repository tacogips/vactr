//! Genuine isolated candidates exercise the bounded complete-lease projection.
use super::*;

const COUNT: u32 = 257;
const WORK: u32 = 33_411;

fn leases() -> Vec<SongResourceLease> {
    (0..COUNT)
        .map(|resource| SongResourceLease {
            resource: u32::MAX - resource,
            generation: resource.wrapping_mul(17),
        })
        .collect()
}
fn prepared() -> PreparedSong {
    prepare_song(super::tests::candidate()).unwrap()
}

#[test]
fn bounded_257_reservations_keep_allocation_generations_and_ack_authority() {
    let mut song = prepared();
    let input = leases();
    let pointer = input.as_ptr();
    let expected = input.clone();
    let epoch = song.epoch();
    let revision = song.revision();
    let mut remaining = WORK;
    song.reserve_bounded(input, COUNT, &mut remaining).unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(song.resources().as_ptr(), pointer);
    assert_eq!(song.resources(), expected);
    assert_eq!(song.epoch(), epoch);
    assert_eq!(song.revision(), revision);
    assert_eq!(song.state(), SongPreparationState::Preparing);
    let mut wrong = expected.clone();
    wrong[256].generation += 1;
    assert!(song.acknowledge_ready(&wrong).is_err());
    assert!(song.acknowledge_ready(&expected[..256]).is_err());
    assert_eq!(song.resources(), expected);
    song.acknowledge_ready(&expected).unwrap();
    assert_eq!(song.state(), SongPreparationState::Ready);
}

#[test]
fn bounded_resource_limit_failure_preserves_state_and_allows_retry() {
    let mut song = prepared();
    let epoch = song.epoch();
    let mut remaining = WORK + 1;
    assert!(song
        .reserve_bounded(leases(), COUNT - 1, &mut remaining)
        .is_err());
    assert_eq!(remaining, WORK);
    assert_eq!(song.state(), SongPreparationState::Preparing);
    assert_eq!(song.epoch(), epoch);
    assert!(song.resources().is_empty());
    assert!(!song.reservations_initialized);
    song.reserve_bounded(leases(), COUNT, &mut remaining)
        .unwrap();
    assert_eq!(remaining, 0);
}

#[test]
fn bounded_work_exact_one_short_and_shared_counter() {
    let mut song = prepared();
    let mut remaining = WORK - 1;
    let error = song
        .reserve_bounded(leases(), COUNT, &mut remaining)
        .unwrap_err();
    assert_eq!(error.code, FailCode::FuelExhausted);
    assert_eq!(remaining, WORK - 2);
    assert!(song.resources().is_empty());
    assert!(!song.reservations_initialized);
    assert_eq!(song.state(), SongPreparationState::Preparing);
    let mut empty = prepared();
    empty.reserve_bounded(vec![], 0, &mut remaining).unwrap();
    assert_eq!(remaining, WORK - 3);
    let mut retry = WORK;
    song.reserve_bounded(leases(), COUNT, &mut retry).unwrap();
    assert_eq!(retry, 0);
    let mut zero = 0;
    assert_eq!(
        prepared()
            .reserve_bounded(vec![], 0, &mut zero)
            .unwrap_err()
            .code,
        FailCode::FuelExhausted
    );
    assert_eq!(zero, 0);
}

#[test]
fn bounded_duplicates_preserve_uninitialized_snapshot() {
    for generation in [1, 2] {
        let mut song = prepared();
        let mut remaining = 6;
        assert!(song
            .reserve_bounded(
                vec![
                    SongResourceLease {
                        resource: u32::MAX,
                        generation: 1
                    },
                    SongResourceLease {
                        resource: u32::MAX,
                        generation
                    },
                ],
                2,
                &mut remaining
            )
            .is_err());
        assert_eq!(remaining, 0);
        assert_eq!(song.state(), SongPreparationState::Preparing);
        assert!(song.resources().is_empty());
        assert!(!song.reservations_initialized);
        let mut retry = 3;
        song.reserve_bounded(
            vec![SongResourceLease {
                resource: u32::MAX,
                generation,
            }],
            1,
            &mut retry,
        )
        .unwrap();
        assert_eq!(song.resources()[0].generation, generation);
        assert_eq!(retry, 0);
    }
}

#[test]
fn bounded_empty_is_once_only_and_legacy_cap_remains() {
    let mut song = prepared();
    assert!(song.reserve(leases()).is_err());
    assert!(!song.reservations_initialized);
    let mut remaining = 2;
    song.reserve_bounded(vec![], 0, &mut remaining).unwrap();
    assert!(song.reserve_bounded(vec![], 0, &mut remaining).is_err());
    assert_eq!(remaining, 0);
    assert!(song.reserve(vec![]).is_err());
    song.acknowledge_ready(&[]).unwrap();
    assert_eq!(song.state(), SongPreparationState::Ready);
}

#[test]
fn bounded_reservations_reject_ready_applied_and_failed_states() {
    for state in [
        SongPreparationState::Ready,
        SongPreparationState::Applied,
        SongPreparationState::Failed,
    ] {
        let mut song = prepared();
        if state == SongPreparationState::Failed {
            song.cancel(song.epoch()).unwrap();
        } else {
            let mut work = 3;
            song.reserve_bounded(
                vec![SongResourceLease {
                    resource: 19,
                    generation: u32::MAX,
                }],
                1,
                &mut work,
            )
            .unwrap();
            let acknowledged = song.resources().to_vec();
            song.acknowledge_ready(&acknowledged).unwrap();
            if state == SongPreparationState::Applied {
                song.acknowledge_applied(SongApplyAck {
                    epoch: song.epoch(),
                    doc_revision: song.revision(),
                    application_frame: 73,
                })
                .unwrap();
            }
        }
        let expected = song.resources().to_vec();
        let application = song.application();
        let mut work = 1;
        assert!(song.reserve_bounded(vec![], 0, &mut work).is_err());
        assert_eq!(work, 0);
        assert_eq!(song.state(), state);
        assert_eq!(song.resources(), expected);
        assert_eq!(song.application(), application);
    }
}
