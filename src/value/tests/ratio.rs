use super::r;
use crate::value::Ratio64;
use crate::vm::fail::FailCode;

fn code(res: Result<Ratio64, crate::vm::fail::Failure>) -> FailCode {
    res.expect_err("expected a failure").code
}

#[test]
fn normalization() {
    assert_eq!(r(2, 4), r(1, 2));
    assert_eq!((r(2, 4).num(), r(2, 4).den()), (1, 2));
    assert_eq!((r(1, -2).num(), r(1, -2).den()), (-1, 2));
    assert_eq!((r(-3, -6).num(), r(-3, -6).den()), (1, 2));
    assert_eq!(r(0, -5), Ratio64::ZERO);
}

#[test]
fn exact_arithmetic_self_reduces() {
    assert_eq!(r(1, 4).checked_add(r(1, 8)).unwrap(), r(3, 8));
    let prod = r(1, 4).checked_mul(r(4, 1)).unwrap();
    assert!(prod.is_integral());
    assert_eq!(prod.to_string(), "1");
    assert_eq!(r(6, 1).checked_div(r(4, 1)).unwrap().to_string(), "3/2");
    assert_eq!(r(1, 2).checked_sub(r(1, 3)).unwrap(), r(1, 6));
    assert_eq!(r(3, 8).to_string(), "3/8");
    assert_eq!(r(-6, 3).to_string(), "-2");
}

#[test]
fn overflow_is_failure() {
    let max = Ratio64::from_int(i64::MAX);
    assert_eq!(code(max.checked_add(Ratio64::ONE)), FailCode::Overflow);
    assert_eq!(code(max.checked_mul(r(2, 1))), FailCode::Overflow);
    assert_eq!(
        code(Ratio64::from_int(i64::MIN).checked_sub(Ratio64::ONE)),
        FailCode::Overflow
    );
    // Denominators overflow too.
    assert_eq!(
        code(r(1, i64::MAX).checked_mul(r(1, 2))),
        FailCode::Overflow
    );
}

#[test]
fn i64_min_cases_overflow() {
    assert_eq!(code(Ratio64::new(i64::MIN, -1)), FailCode::Overflow);
    assert_eq!(code(Ratio64::new(1, i64::MIN)), FailCode::Overflow);
    let min = Ratio64::from_int(i64::MIN);
    // Negation of i64::MIN.
    assert_eq!(code(Ratio64::ZERO.checked_sub(min)), FailCode::Overflow);
    assert_eq!(code(min.checked_div(r(-1, 1))), FailCode::Overflow);
    assert_eq!(code(min.checked_mul(r(-1, 1))), FailCode::Overflow);
    // Reduction keeps an in-range result representable.
    assert_eq!(
        Ratio64::new(i64::MIN, 2).unwrap(),
        Ratio64::from_int(i64::MIN / 2)
    );
}

#[test]
fn zero_denominator_is_division_by_zero() {
    assert_eq!(code(Ratio64::new(1, 0)), FailCode::DivisionByZero);
    assert_eq!(
        code(r(1, 2).checked_div(Ratio64::ZERO)),
        FailCode::DivisionByZero
    );
}

#[test]
fn floor_and_frac_of_negative_values() {
    assert_eq!(r(-7, 2).floor(), -4);
    assert_eq!(r(-7, 2).frac(), r(1, 2));
    assert_eq!(r(-1, 3).floor(), -1);
    assert_eq!(r(-1, 3).frac(), r(2, 3));
    assert_eq!(r(-2, 1).floor(), -2);
    assert_eq!(r(-2, 1).frac(), Ratio64::ZERO);
    assert_eq!(r(7, 2).floor(), 3);
    assert_eq!(r(7, 2).frac(), r(1, 2));
}

#[test]
fn ordering_is_exact() {
    assert!(r(1, 3) < r(1, 2));
    assert!(r(-1, 2) < r(-1, 3));
    assert!(r(i64::MAX - 1, i64::MAX) < Ratio64::ONE);
}

#[test]
fn from_decimal() {
    assert_eq!(Ratio64::from_decimal("1.5"), Some(r(3, 2)));
    assert_eq!(Ratio64::from_decimal("-0.25"), Some(r(-1, 4)));
    assert_eq!(Ratio64::from_decimal("42"), Some(r(42, 1)));
    assert_eq!(Ratio64::from_decimal("0.1"), Some(r(1, 10)));
    assert_eq!(Ratio64::from_decimal("1."), None);
    assert_eq!(Ratio64::from_decimal(".5"), None);
    assert_eq!(Ratio64::from_decimal("1.2.3"), None);
    assert_eq!(Ratio64::from_decimal("99999999999999999999"), None);
}

#[test]
fn from_f64_exact() {
    assert_eq!(Ratio64::from_f64_exact(0.5), Some(r(1, 2)));
    assert_eq!(Ratio64::from_f64_exact(-0.0), Some(Ratio64::ZERO));
    assert_eq!(Ratio64::from_f64_exact(3.0), Some(r(3, 1)));
    assert_eq!(
        Ratio64::from_f64_exact(-9_223_372_036_854_775_808.0),
        Some(Ratio64::from_int(i64::MIN))
    );
    assert_eq!(Ratio64::from_f64_exact(1e30), None);
    assert_eq!(Ratio64::from_f64_exact(1e-30), None);
    assert_eq!(Ratio64::from_f64_exact(f64::NAN), None);
    assert_eq!(Ratio64::from_f64_exact(f64::INFINITY), None);
}
