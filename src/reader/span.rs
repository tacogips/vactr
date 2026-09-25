//! Source positions and identities (design 6.5.1).

use crate::ns::namespace::FormGen;

id_newtype!(
    /// A source file. `FileId::CONSOLE` (id 0) is the REPL console.
    FileId(u32)
);

impl FileId {
    /// The console pseudo-file, where console registers (`_1`) are allowed.
    pub const CONSOLE: FileId = FileId(0);
}

id_newtype!(
    /// A reader node identity, unique within one read.
    NodeId(u32)
);

/// A byte range `start..end` in one file.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Span {
    pub file: FileId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// Builds a span over `start..end` in `file`.
    #[must_use]
    pub const fn new(file: FileId, start: u32, end: u32) -> Self {
        Self { file, start, end }
    }

    /// The length in bytes (0 for an inverted span).
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    /// True when the span covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// The smallest span covering both spans. Both spans are in the same file;
    /// the result keeps `self.file`.
    #[must_use]
    pub fn join(self, other: Span) -> Span {
        Span {
            file: self.file,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// A source reference with the document revision and form generation it
/// belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SrcRef {
    pub span: Span,
    pub doc_revision: u64,
    pub form_gen: FormGen,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_is_id_zero() {
        assert_eq!(FileId::CONSOLE.get(), 0);
        assert_eq!(FileId::new(0), FileId::CONSOLE);
    }

    #[test]
    fn span_len_and_join() {
        let f = FileId::new(1);
        let a = Span::new(f, 2, 5);
        let b = Span::new(f, 7, 9);
        assert_eq!(a.len(), 3);
        assert!(!a.is_empty());
        assert!(Span::new(f, 4, 4).is_empty());
        assert_eq!(a.join(b), Span::new(f, 2, 9));
        assert_eq!(b.join(a), Span::new(f, 2, 9));
    }
}
