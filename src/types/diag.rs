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
        /// A closed set of diagnostic codes. TASK-004 adds checker codes.
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
        assert_eq!(DiagCode::ALL.len(), 38);
        let mut names: Vec<&str> = DiagCode::ALL.iter().map(|c| c.as_str()).collect();
        assert!(names
            .iter()
            .all(|n| n.chars().all(|c| c.is_ascii_lowercase() || c == '-')));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 38);
        assert_eq!(DiagCode::MisplacedArrow.as_str(), "misplaced-arrow");
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
