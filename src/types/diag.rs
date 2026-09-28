//! Diagnostics (design section 7, 6.5.4 and 6.5.5).

use std::fmt;

use crate::reader::span::Span;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;

/// How serious a diagnostic is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

impl Severity {
    /// The lowercase name of the severity.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Hint => "hint",
        }
    }
}

/// The playing slot and beat a runtime diagnostic came from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct RunOrigin {
    pub slot: Option<KwId>,
    pub beat: Option<Ratio64>,
}

macro_rules! diag_codes {
    ($($(#[$meta:meta])* $variant:ident => $name:literal,)*) => {
        /// A closed set of diagnostic codes (6.5.4, 6.5.5, 7.1.6).
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub enum DiagCode {
            $($(#[$meta])* $variant,)*
        }

        impl DiagCode {
            /// Every code, in declaration order.
            pub const ALL: &'static [DiagCode] = &[$(DiagCode::$variant,)*];

            /// The kebab-case name of the code.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(DiagCode::$variant => $name,)*
                }
            }
        }
    };
}

diag_codes! {
    // Reader codes (6.5.4).
    SourceTooLarge => "source-too-large",
    BadNumber => "bad-number",
    ConsoleRegisterInFile => "console-register-in-file",
    BadIdentifier => "bad-identifier",
    ParenForm => "paren-form",
    StrayChar => "stray-char",
    UnterminatedString => "unterminated-string",
    BadEscape => "bad-escape",
    MisplacedColon => "misplaced-colon",
    IndentSpace => "indent-space",
    EmptyBlock => "empty-block",
    ContinuationAfterBlock => "continuation-after-block",
    UnexpectedIndent => "unexpected-indent",
    ImportNotTopLevel => "import-not-top-level",
    BadImport => "bad-import",
    BindingWithoutValue => "binding-without-value",
    ArrowWithoutBody => "arrow-without-body",
    /// `->` inside `[..]` (design 6.5.4: "`->` inside a list is a diagnostic").
    MisplacedArrow => "misplaced-arrow",
    MisplacedFallback => "misplaced-fallback",
    /// A pipe `>` with no call after it (added by FE-READER; FE-FINAL records
    /// it in 6.5.4).
    EmptyPipe => "empty-pipe",
    BadPair => "bad-pair",
    MisplacedSplat => "misplaced-splat",
    EmptyGroup => "empty-group",
    UnclosedGroup => "unclosed-group",
    UnclosedBracket => "unclosed-bracket",
    UnboundQualifier => "unbound-qualifier",
    NestingTooDeep => "nesting-too-deep",
    // Expander codes (6.5.5).
    ReadErrorPresent => "read-error-present",
    SugarInPattern => "sugar-in-pattern",
    ReservedWord => "reserved-word",
    IfGuard => "if-guard",
    MalformedIf => "malformed-if",
    ElseWithoutIf => "else-without-if",
    MalformedFor => "malformed-for",
    MalformedMatch => "malformed-match",
    MalformedFn => "malformed-fn",
    MalformedBinding => "malformed-binding",
    MalformedEnum => "malformed-enum",
    // Reader codes for path and url literals (FRONTEND, 6.5.8).
    BadPath => "bad-path",
    BadUrl => "bad-url",
    // Checker: types and names (7.1.6).
    TypeMismatch => "type-mismatch",
    AnnotationMismatch => "annotation-mismatch",
    OptionalAsValue => "optional-as-value",
    AnyNotNarrowed => "any-not-narrowed",
    UndefinedName => "undefined-name",
    Rebinding => "rebinding",
    /// Warning: a binding shadows a user binding of an enclosing scope.
    Shadowing => "shadowing",
    /// Hint: a binding shadows a prelude name.
    ShadowsPrelude => "shadows-prelude",
    UpdImmutable => "upd-immutable",
    LiteralDivisionByZero => "literal-division-by-zero",
    UnknownKeyword => "unknown-keyword",
    MissingVariant => "missing-variant",
    BareVariantBinding => "bare-variant-binding",
    SoundNotFirst => "sound-not-first",
    // Checker: hygiene (7.1.6).
    DuplicateKey => "duplicate-key",
    ImportCollision => "import-collision",
    BeyondCapability => "beyond-capability",
    EffectInPattern => "effect-in-pattern",
    UnboundedSource => "unbounded-source",
    // Forcing (5.5).
    MixedForcing => "mixed-forcing",
    LatentForcing => "latent-forcing",
    // Patterns and clock.
    BadSlicePoints => "bad-slice-points",
    InputLaneOperator => "input-lane-operator",
    ClockSourceUnavailable => "clock-source-unavailable",
    // Reactive (5.6).
    DependencyCycle => "dependency-cycle",
    // Back end: scheduler, hosts and DSP (12.8.12).
    /// An instrument or bus graph over `NODE_CAP` nodes (16.1).
    GraphTooLarge => "graph-too-large",
    /// Controls unacknowledged past the threshold (11.3, 12.8.4).
    HostTransport => "host-transport",
    RingOverflow => "ring-overflow",
    /// Warning: the commit lead widened after late events (12.8.4).
    LatencyWidened => "latency-widened",
    ArenaExhausted => "arena-exhausted",
    InstallQueueOverflow => "install-queue-overflow",
    /// Warning: sustained grain skipping (12.6).
    GrainSkip => "grain-skip",
    /// Warning: a voice was stolen on pool exhaustion (11.7).
    VoiceSteal => "voice-steal",
    /// Warning: the external MIDI clock stopped arriving (11.7).
    ClockLost => "clock-lost",
    /// `use-bpm` while the clock follows `:midi` (11.7).
    ClockExternal => "clock-external",
    // Session layer: directive lint (13.5, 14.5.12), all warnings.
    /// Warning: "`#@` block attaches to no statement, block or definition".
    UnknownDirectiveSite => "unknown-directive-site",
    /// Warning: "`<param>` is not a parameter of `<site>`".
    UnknownParameter => "unknown-parameter",
    /// Warning: "no site is labeled `<label>`".
    UnknownLabel => "unknown-label",
    /// Warning: "label `<label>` is already used at <span>".
    DuplicateLabel => "duplicate-label",
    /// Warning: "selector `<selector>` matches more than one site".
    AmbiguousSelector => "ambiguous-selector",
    /// Warning: "cc `<n>` is outside 0..127" (or channel outside 1..16).
    CcOutOfRange => "cc-out-of-range",
    /// Warning: "`<key>` is a reserved directive key".
    ReservedKey => "reserved-key",
    // Session layer: packages (5.7, 14.5.7).
    /// Warning: "package `<path>@<version>` is locked but not fetched; run
    /// `vactr get`".
    PackageNotFetched => "package-not-fetched",
    /// "package `<path>` is not in `vactr.lock`".
    PackageNotLocked => "package-not-locked",
    /// "package `<path>@<version>` digest mismatch: expected <a>, got <b>"
    /// (or an unsafe archive entry, named).
    PackageIntegrity => "package-integrity",
    /// "package `<path>` failed to load: <reason>".
    PackageLoadFailed => "package-load-failed",
    /// "cannot resolve `<path>`: <reason>" (`vactr get`: an unresolvable
    /// version or a store or network error).
    PackageResolve => "package-resolve",
}

impl DiagCode {
    /// The severity the code is reported with (7.1.6): warnings and the one
    /// hint are listed, every other code is an error.
    #[must_use]
    pub const fn default_severity(self) -> Severity {
        match self {
            DiagCode::Shadowing
            | DiagCode::DuplicateKey
            | DiagCode::ImportCollision
            | DiagCode::EffectInPattern
            | DiagCode::UnboundedSource
            | DiagCode::MixedForcing
            | DiagCode::LatentForcing
            | DiagCode::LatencyWidened
            | DiagCode::GrainSkip
            | DiagCode::VoiceSteal
            | DiagCode::ClockLost
            | DiagCode::UnknownDirectiveSite
            | DiagCode::UnknownParameter
            | DiagCode::UnknownLabel
            | DiagCode::DuplicateLabel
            | DiagCode::AmbiguousSelector
            | DiagCode::CcOutOfRange
            | DiagCode::ReservedKey
            | DiagCode::PackageNotFetched => Severity::Warning,
            DiagCode::ShadowsPrelude => Severity::Hint,
            _ => Severity::Error,
        }
    }
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A diagnostic attached to a source span.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub span: Span,
    pub severity: Severity,
    pub code: DiagCode,
    pub message: String,
    pub origin: Option<RunOrigin>,
}

impl Diagnostic {
    /// An error diagnostic with no run origin.
    #[must_use]
    pub fn error(code: DiagCode, span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            severity: Severity::Error,
            code,
            message: message.into(),
            origin: None,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}] {}:{}..{}: {}",
            self.severity.as_str(),
            self.code,
            self.span.file.get(),
            self.span.start,
            self.span.end,
            self.message
        )
    }
}

impl std::error::Error for Diagnostic {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::span::FileId;

    #[test]
    fn code_names_are_unique_kebab_case() {
        assert_eq!(DiagCode::ALL.len(), 87);
        let mut names: Vec<&str> = DiagCode::ALL.iter().map(|c| c.as_str()).collect();
        assert!(names
            .iter()
            .all(|n| n.chars().all(|c| c.is_ascii_lowercase() || c == '-')));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 87);
        assert_eq!(DiagCode::MisplacedArrow.as_str(), "misplaced-arrow");
    }

    #[test]
    fn default_severity_per_code() {
        assert_eq!(DiagCode::Rebinding.default_severity(), Severity::Error);
        assert_eq!(DiagCode::ParenForm.default_severity(), Severity::Error);
        assert_eq!(DiagCode::Shadowing.default_severity(), Severity::Warning);
        assert_eq!(DiagCode::MixedForcing.default_severity(), Severity::Warning);
        assert_eq!(DiagCode::ShadowsPrelude.default_severity(), Severity::Hint);
        let warnings = DiagCode::ALL
            .iter()
            .filter(|c| c.default_severity() == Severity::Warning)
            .count();
        let hints = DiagCode::ALL
            .iter()
            .filter(|c| c.default_severity() == Severity::Hint)
            .count();
        assert_eq!((warnings, hints), (19, 1));
        assert_eq!(DiagCode::VoiceSteal.default_severity(), Severity::Warning);
        assert_eq!(DiagCode::ClockExternal.default_severity(), Severity::Error);
    }

    #[test]
    fn error_display() {
        let d = Diagnostic::error(
            DiagCode::ParenForm,
            Span::new(FileId::new(1), 3, 5),
            "no parens",
        );
        assert_eq!(d.severity, Severity::Error);
        assert_eq!(d.to_string(), "error[paren-form] 1:3..5: no parens");
    }
}
