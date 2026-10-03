use super::*;
use crate::pattern::TimeSpan;
fn span(a: i64, b: i64, d: i64) -> TimeSpan {
    TimeSpan::new(Ratio64::new(a, d).unwrap(), Ratio64::new(b, d).unwrap()).unwrap()
}
#[test]
fn true_end_reentry_is_distinct_from_filtered_note_holes() {
    let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
    let source = span(0, 41, 10);
    assert_eq!(
        iterator_component(source, 4, source, Ratio64::ZERO, &mut budget).unwrap(),
        span(0, 67, 20)
    );
    assert_eq!(
        iterator_component(source, 4, source, Ratio64::from_int(4), &mut budget).unwrap(),
        span(40, 41, 10)
    );
    let unchanged = span(0, 64, 1);
    assert_eq!(
        iterator_component(unchanged, 2, unchanged, Ratio64::ZERO, &mut budget).unwrap(),
        span(0, 127, 2)
    );
}
#[test]
fn identity_count_clips_owner_checks_birth_and_charges_work() {
    let source = span(0, 4, 1);
    let owner = span(1, 2, 1);
    let mut budget = ResolutionBudget::new(crate::song::SongLimits::default());
    assert_eq!(
        iterator_component(source, 0, owner, Ratio64::ONE, &mut budget).unwrap(),
        owner
    );
    assert_eq!(
        iterator_component(source, 0, owner, Ratio64::from_int(3), &mut budget)
            .unwrap_err()
            .code,
        crate::vm::fail::FailCode::Type
    );
    let mut tiny = ResolutionBudget::new(crate::song::SongLimits {
        max_nodes: 1,
        ..Default::default()
    });
    iterator_component(source, 0, owner, Ratio64::ONE, &mut tiny).unwrap();
    assert_eq!(
        iterator_component(source, 0, owner, Ratio64::ONE, &mut tiny)
            .unwrap_err()
            .code,
        crate::vm::fail::FailCode::FuelExhausted
    );
}
#[test]
fn billion_cycle_configuration_uses_constant_work_and_rejects_tiny_budget() {
    let source = span(0, 1_000_000_000, 1);
    let mut budget = ResolutionBudget::new(crate::song::SongLimits {
        max_nodes: 10,
        ..Default::default()
    });
    assert_eq!(
        iterator_component(source, 4, source, Ratio64::ZERO, &mut budget).unwrap(),
        span(0, 3_999_999_997, 4)
    );
    let mut tiny = ResolutionBudget::new(crate::song::SongLimits {
        max_nodes: 5,
        ..Default::default()
    });
    assert_eq!(
        iterator_component(source, 4, source, Ratio64::ZERO, &mut tiny)
            .unwrap_err()
            .code,
        crate::vm::fail::FailCode::FuelExhausted
    );
}
