//! Exact `int64/int64` ratios (design 5.3, 6.5.3).

use std::cmp::Ordering;
use std::fmt;

use crate::vm::fail::{FailCode, Failure};

/// An exact ratio. Invariant: `den > 0` and `gcd(num, den) == 1`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Ratio64 {
    num: i64,
    den: i64,
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn overflow() -> Failure {
    Failure::new(FailCode::Overflow, "ratio does not fit in int64/int64")
}

fn div_by_zero() -> Failure {
    Failure::new(FailCode::DivisionByZero, "division by zero")
}

impl Ratio64 {
    /// Zero.
    pub const ZERO: Ratio64 = Ratio64 { num: 0, den: 1 };
    /// One.
    pub const ONE: Ratio64 = Ratio64 { num: 1, den: 1 };

    /// Builds `n/d`, normalized.
    ///
    /// # Errors
    /// `DivisionByZero` when `d == 0`; `Overflow` when the normalized value
    /// does not fit (for example `i64::MIN / -1`).
    pub fn new(n: i64, d: i64) -> Result<Self, Failure> {
        Self::from_i128(i128::from(n), i128::from(d))
    }

    /// An integral ratio.
    #[must_use]
    pub const fn from_int(n: i64) -> Self {
        Self { num: n, den: 1 }
    }

    fn from_i128(n: i128, d: i128) -> Result<Self, Failure> {
        if d == 0 {
            return Err(div_by_zero());
        }
        let g = gcd(n.unsigned_abs(), d.unsigned_abs());
        // g >= 1 because d != 0, and g <= |d| <= 2^127, so it fits back in i128
        // except for g == 2^127, which only happens when n == d == i128::MIN.
        let g = i128::try_from(g).map_err(|_| overflow())?;
        let (mut n, mut d) = (n / g, d / g);
        if d < 0 {
            n = n.checked_neg().ok_or_else(overflow)?;
            d = d.checked_neg().ok_or_else(overflow)?;
        }
        let num = i64::try_from(n).map_err(|_| overflow())?;
        let den = i64::try_from(d).map_err(|_| overflow())?;
        Ok(Self { num, den })
    }

    /// The numerator (carries the sign).
    #[must_use]
    pub const fn num(self) -> i64 {
        self.num
    }

    /// The denominator (always positive).
    #[must_use]
    pub const fn den(self) -> i64 {
        self.den
    }

    /// Exact addition.
    ///
    /// # Errors
    /// `Overflow` when the result does not fit.
    pub fn checked_add(self, rhs: Self) -> Result<Self, Failure> {
        let (a, b, c, d) = self.wide(rhs);
        let n = (a * d).checked_add(c * b).ok_or_else(overflow)?;
        Self::from_i128(n, b * d)
    }

    /// Exact subtraction.
    ///
    /// # Errors
    /// `Overflow` when the result does not fit.
    pub fn checked_sub(self, rhs: Self) -> Result<Self, Failure> {
        let (a, b, c, d) = self.wide(rhs);
        let n = (a * d).checked_sub(c * b).ok_or_else(overflow)?;
        Self::from_i128(n, b * d)
    }

    /// Exact multiplication.
    ///
    /// # Errors
    /// `Overflow` when the result does not fit.
    pub fn checked_mul(self, rhs: Self) -> Result<Self, Failure> {
        let (a, b, c, d) = self.wide(rhs);
        Self::from_i128(a * c, b * d)
    }

    /// Exact division.
    ///
    /// # Errors
    /// `DivisionByZero` when `rhs` is zero; `Overflow` when the result does not fit.
    pub fn checked_div(self, rhs: Self) -> Result<Self, Failure> {
        let (a, b, c, d) = self.wide(rhs);
        if c == 0 {
            return Err(div_by_zero());
        }
        Self::from_i128(a * d, b * c)
    }

    // Each factor is at most 2^63 in magnitude, so every product of two fits in i128.
    fn wide(self, rhs: Self) -> (i128, i128, i128, i128) {
        (
            i128::from(self.num),
            i128::from(self.den),
            i128::from(rhs.num),
            i128::from(rhs.den),
        )
    }

    /// The largest integer not greater than the ratio.
    #[must_use]
    pub const fn floor(self) -> i64 {
        self.num.div_euclid(self.den)
    }

    /// The fractional part `self - floor(self)`, in `[0, 1)`.
    #[must_use]
    pub const fn frac(self) -> Ratio64 {
        // gcd(num mod den, den) == gcd(num, den) == 1, so this is normalized.
        Ratio64 {
            num: self.num.rem_euclid(self.den),
            den: self.den,
        }
    }

    /// The nearest-ish `f64` (host boundary only).
    #[must_use]
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }

    /// True when the denominator is 1.
    #[must_use]
    pub const fn is_integral(self) -> bool {
        self.den == 1
    }

    /// The exact value of a decimal literal such as `1.5` or `-0.25`.
    ///
    /// Returns `None` for malformed text or when the value does not fit.
    #[must_use]
    pub fn from_decimal(text: &str) -> Option<Ratio64> {
        let (neg, body) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let (int_part, frac_part) = match body.split_once('.') {
            Some((i, f)) => (i, f),
            None => (body, ""),
        };
        if int_part.is_empty() || !int_part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        if body.contains('.')
            && (frac_part.is_empty() || !frac_part.bytes().all(|b| b.is_ascii_digit()))
        {
            return None;
        }
        let mut n: i128 = 0;
        let mut d: i128 = 1;
        for b in int_part.bytes().chain(frac_part.bytes()) {
            n = n.checked_mul(10)?.checked_add(i128::from(b - b'0'))?;
        }
        for _ in frac_part.bytes() {
            d = d.checked_mul(10)?;
        }
        if neg {
            n = -n;
        }
        Self::from_i128(n, d).ok()
    }

    /// The exact value of a finite float, when it fits in `Ratio64`
    /// (a dyadic rational with a denominator of at most 2^62).
    #[must_use]
    pub fn from_f64_exact(x: f64) -> Option<Ratio64> {
        let (m, e) = decompose_f64(x)?;
        if m == 0 {
            return Some(Self::ZERO);
        }
        if e >= 0 {
            if e > 64 {
                return None;
            }
            let n = m.checked_mul(1i128 << e)?;
            Self::from_i128(n, 1).ok()
        } else {
            let k = -e;
            if k > 62 {
                return None;
            }
            Self::from_i128(m, 1i128 << k).ok()
        }
    }
}

/// Splits a finite float into `m * 2^e` with `m` odd (or zero). `None` for
/// NaN and infinities.
pub(crate) fn decompose_f64(x: f64) -> Option<(i128, i32)> {
    if !x.is_finite() {
        return None;
    }
    let bits = x.to_bits();
    let negative = bits >> 63 == 1;
    let exp = ((bits >> 52) & 0x7ff) as i32;
    let frac = bits & ((1u64 << 52) - 1);
    let (mut m, mut e) = if exp == 0 {
        (frac, -1074)
    } else {
        (frac | (1u64 << 52), exp - 1075)
    };
    if m == 0 {
        return Some((0, 0));
    }
    let tz = m.trailing_zeros();
    m >>= tz;
    e += tz as i32;
    let m = i128::from(m);
    Some((if negative { -m } else { m }, e))
}

impl Ord for Ratio64 {
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = i128::from(self.num) * i128::from(other.den);
        let rhs = i128::from(other.num) * i128::from(self.den);
        lhs.cmp(&rhs)
    }
}

impl PartialOrd for Ratio64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Ratio64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}
