//! Runtime failures (design 8.4).

use std::fmt;

use crate::reader::span::Span;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;

/// Why an evaluation failed. TASK-005 adds codes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum FailCode {
    DivisionByZero,
    Overflow,
    Type,
    UnknownField,
}

impl FailCode {
    /// The kebab-case name of the code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            FailCode::DivisionByZero => "division-by-zero",
            FailCode::Overflow => "overflow",
            FailCode::Type => "type",
            FailCode::UnknownField => "unknown-field",
        }
    }
}

impl fmt::Display for FailCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a failure happened: source, playing slot and beat, when known.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Origin {
    pub span: Option<Span>,
    pub slot: Option<KwId>,
    pub beat: Option<Ratio64>,
}

impl Origin {
    /// An origin with nothing known.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            span: None,
            slot: None,
            beat: None,
        }
    }
}

/// A runtime failure. It unwinds through `Result` to the top level or the
/// current pattern event (design 8.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Failure {
    pub code: FailCode,
    pub message: String,
    pub origin: Origin,
}

impl Failure {
    /// A failure with no known origin.
    #[must_use]
    pub fn new(code: FailCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            origin: Origin::none(),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Failure {}
