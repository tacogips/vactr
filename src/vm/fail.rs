//! Runtime failures (design 8.4).

use std::fmt;

use crate::reader::span::Span;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;

macro_rules! fail_codes {
    ($($(#[$meta:meta])* $variant:ident => $name:literal,)*) => {
        /// Why an evaluation failed (8.4, 7.1.6).
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        pub enum FailCode {
            $($(#[$meta])* $variant,)*
        }

        impl FailCode {
            /// Every code, in declaration order.
            pub const ALL: &'static [FailCode] = &[$(FailCode::$variant,)*];

            /// The kebab-case name of the code.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(FailCode::$variant => $name,)*
                }
            }
        }
    };
}

fail_codes! {
    DivisionByZero => "division-by-zero",
    Overflow => "overflow",
    Type => "type",
    UnknownField => "unknown-field",
    // Middle end (7.1.6).
    NoMatch => "no-match",
    FuelExhausted => "fuel-exhausted",
    DepthExceeded => "depth-exceeded",
    EffectInQuery => "effect-in-query",
    EffectInRebuild => "effect-in-rebuild",
    UndefinedName => "undefined-name",
    NotCallable => "not-callable",
    Arity => "arity",
    /// A form reported `blocked-on: X` (5.6).
    Blocked => "blocked",
    SliceIndex => "slice-index",
    /// Dynamic manual slice points.
    BadSlicePoints => "bad-slice-points",
    /// A region operator got an event with `whole = None`.
    NoWhole => "no-whole",
    UpdImmutable => "upd-immutable",
    /// A key missing from the current `sound-kit` (event-local).
    UnknownSound => "unknown-sound",
    /// `NoopHost`.
    HostUnavailable => "host-unavailable",
    LoadFailed => "load-failed",
    // Back end (12.8.12).
    /// An `inst` body failed at definition time; the previous definition
    /// stays installed (12.8.6).
    InstFailed => "inst-failed",
    /// An event resolved more than `MAX_CTLS` controls (event-local, 12.8.7).
    TooManyControls => "too-many-controls",
    // Session layer: self-analysis (14.5.9, 14.5.12).
    /// A `Pending` sample buffer (a capture or render not yet finished) was
    /// read for analysis or playback.
    CapturePending => "capture-pending",
    /// The host tier cannot do this (`render n` on the browser, a capture
    /// longer than `max_capture_seconds`): "not available on this host".
    BeyondCapability => "beyond-capability",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_names_are_unique_kebab_case() {
        assert_eq!(FailCode::ALL.len(), 24);
        let mut names: Vec<&str> = FailCode::ALL.iter().map(|c| c.as_str()).collect();
        assert!(names
            .iter()
            .all(|n| n.chars().all(|c| c.is_ascii_lowercase() || c == '-')));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 24);
        assert_eq!(FailCode::LoadFailed.as_str(), "load-failed");
        assert_eq!(FailCode::DivisionByZero.to_string(), "division-by-zero");
    }
}
