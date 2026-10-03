use super::tests::{sampling_exact_work, sampling_with_replay};
use super::{CanonicalClockProjection, ClockOrientation};
use crate::pattern::eval::song_observation::{CanonicalIndexObservation, SharedIndexWork};

fn indexed_song(body: &str) -> String {
    let template = "fn cut beat:\n\tfirst [0]\nfn indexed p:\n\tBODY\nlet base {part [drums: {s :analog > chord [:c :five]}] duration: 4}\nlet selected {transform-instrument base :drums :analog indexed}\nsong selected tail-seconds: 0";
    template.replace("BODY", body)
}

fn rows(work: &SharedIndexWork) -> Vec<CanonicalIndexObservation> {
    let ledger = work.borrow();
    let Some(target) = ledger.target.as_ref() else {
        panic!("genuine target was retained");
    };
    ledger
        .observations
        .iter()
        .filter(|row| {
            row.owner.revision == target.revision
                && row.owner.track == target.track
                && row.owner.root == target.root
        })
        .cloned()
        .collect()
}

fn assert_unchanged_projection(work: &SharedIndexWork) {
    let observations = rows(work);
    assert!(!observations.is_empty(), "actual Slice observations");
    for row in observations {
        assert!(matches!(row.clock, CanonicalClockProjection::Known(_)));
        let whole = row.event.whole.expect("actual Index whole");
        let footprint = row
            .clock
            .project_for(&row.owner, whole, row.issuer_sample_start, work)
            .unwrap()
            .expect("canonical structural clock");
        assert_eq!(footprint.whole, whole);
        assert_eq!(footprint.sample_start, row.issuer_sample_start);
        assert_eq!(footprint.orientation, ClockOrientation::At);
        assert!(footprint.applicable);
    }
}

#[test]
fn seven_structural_operators_preserve_slice_clocks() {
    for body in [
        "euclid {slice {beat -> p} 2 [cut nil]} 1 2 0",
        "ply {slice {beat -> p} 2 [cut nil]} 2",
        "arp {slice {beat -> p} 2 [cut nil]} :up",
        "chop {slice {beat -> p} 2 [cut nil]} 2",
        "striate {slice {beat -> p} 2 [cut nil]} 2",
        "loop-at {slice {beat -> p} 2 [cut nil]} 2",
        "fit {slice {beat -> p} 2 [cut nil]}",
    ] {
        sampling_with_replay(&indexed_song(body), assert_unchanged_projection);
    }
}

#[test]
fn euclid_empty_subject_has_no_false_index_observation() {
    let code = indexed_song("euclid {slice {beat -> nil} 2 [cut nil]} 1 2 0");
    sampling_with_replay(&code, |work| assert!(rows(work).is_empty()));
}

#[test]
fn nested_fast_rev_weighted_euclid_and_chop_keep_clock_orientation() {
    let body = "euclid {chop {fast [{hold {rev {slice {beat -> p} 2 [cut nil]}} 3} nil] 2} 3} 3 0";
    sampling_with_replay(&indexed_song(body), |work| {
        assert_unchanged_projection(work);
        let observations = rows(work);
        assert!(observations.iter().any(|row| {
            matches!(&row.clock, CanonicalClockProjection::Known(_))
                && !row.clock.sampling_evidence().is_empty()
        }));
    });
}

#[test]
fn split_queries_retain_the_single_query_index_rows() {
    sampling_with_replay(
        &indexed_song("euclid {chop {slice {beat -> p} 2 [cut nil]} 2} 2 4 0"),
        assert_unchanged_projection,
    );
}

#[test]
fn striate_ranking_reuses_retained_work_without_callback_reads() {
    sampling_with_replay(
        &indexed_song("striate {slice {beat -> p} 2 [cut nil]} 3"),
        assert_unchanged_projection,
    );
}

#[test]
fn chunk_slice_clock_stays_unknown() {
    sampling_with_replay(
        &indexed_song("chunk {slice {beat -> p} 2 [cut nil]} 2"),
        |work| {
            let observations = rows(work);
            assert!(!observations.is_empty());
            for row in observations {
                assert_eq!(row.clock, CanonicalClockProjection::Unknown);
                assert!(row
                    .clock
                    .project_for(
                        &row.owner,
                        row.event.whole.expect("actual Index whole"),
                        row.issuer_sample_start,
                        work,
                    )
                    .unwrap()
                    .is_none());
            }
        },
    );
}

#[test]
fn euclid_slice_query_charges_exact_and_one_less_work() {
    sampling_exact_work(&indexed_song(
        "euclid {slice {beat -> p} 2 [cut nil]} 3 8 1",
    ));
}
