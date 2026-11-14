//! Tokens for one physical line, including string interpolation (6.2, 6.5.4).
//!
//! The lexer works on byte offsets into the whole source, so every span is
//! byte-accurate. It never panics: every index goes through `byte`, and text
//! is only sliced at ASCII boundaries or at char boundaries it has walked.

use std::rc::Rc;

use crate::reader::node::{Op, TriviaItem, TriviaKind};
use crate::reader::pathlit::{scan_path_or_url, PathTok};
use crate::reader::span::{FileId, Span};
use crate::reader::MAX_NESTING;
use crate::types::diag::{DiagCode, Diagnostic};
use crate::value::ratio::Ratio64;

/// A token kind.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Tok {
    Int(i64),
    Float {
        value: f64,
        exact: Option<Ratio64>,
    },
    Ratio(Ratio64),
    /// A string literal; a `Str` with an `Expr` part reads as `Interp`.
    Str(Vec<StrPart>),
    Keyword(Rc<str>),
    Ident(Rc<str>),
    Qualified {
        prefix: Rc<str>,
        name: Rc<str>,
    },
    Wildcard,
    ConsoleReg(u32),
    /// An unquoted path literal, as written (6.5.8).
    Path(Rc<str>),
    /// A `scheme://` url literal, as written (6.5.8).
    Url(Rc<str>),
    Op(Op),
    /// `-` directly before a name or `{` in prefix position.
    Neg,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    /// A colon directly after a key token and followed by a value.
    PairColon,
    /// A colon with only whitespace or a comment after it on the line.
    BlockColon,
}

/// One piece of a string literal.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StrPart {
    /// Decoded text (never empty); the span covers the raw source text.
    Text { text: Rc<str>, span: Span },
    /// A `{..}` expression; the span covers the braces.
    Expr { tokens: Vec<Token>, span: Span },
}

/// A token with its span and whether whitespace (or the line start) precedes it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Token {
    pub kind: Tok,
    pub span: Span,
    pub space_before: bool,
}

impl Token {
    /// True for the tokens a pair colon may follow (identifier, string, number).
    pub(crate) fn is_key(&self) -> bool {
        matches!(
            self.kind,
            Tok::Ident(_) | Tok::Str(_) | Tok::Int(_) | Tok::Float { .. } | Tok::Ratio(_)
        )
    }

    /// True when the token is the operator `op`.
    pub(crate) fn is_op(&self, op: Op) -> bool {
        self.kind == Tok::Op(op)
    }
}

/// The lexed content of one physical line.
#[derive(Debug)]
pub(crate) struct LexedLine {
    pub tokens: Vec<Token>,
    pub diags: Vec<Diagnostic>,
    pub comment: Option<TriviaItem>,
    /// Where the code ends: the comment start, or the line end.
    pub code_end: usize,
}

/// Lexes `src[start..end]`, one line without its terminator or indentation.
pub(crate) fn lex_line(src: &str, start: usize, end: usize, file: FileId) -> LexedLine {
    let mut lx = Lexer {
        src,
        bytes: src.as_bytes(),
        end,
        line_start: start,
        file,
        diags: Vec::new(),
        comment: None,
        code_end: end,
        eol_failures: 0,
    };
    let scan = lx.scan(start, 0);
    LexedLine {
        tokens: scan.tokens,
        diags: lx.diags,
        comment: lx.comment,
        code_end: lx.code_end,
    }
}

/// The result of one scan: tokens, the position after it, and, for an
/// interpolation scan, whether the closing `}` was found.
struct Scan {
    tokens: Vec<Token>,
    next: usize,
    closed: bool,
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    end: usize,
    line_start: usize,
    file: FileId,
    diags: Vec<Diagnostic>,
    comment: Option<TriviaItem>,
    code_end: usize,
    /// Strings that gave up at the end of the line; a string around a failed
    /// nested string reports nothing more.
    eol_failures: u32,
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Characters that start or belong to some token, or are whitespace.
fn is_known_ascii(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b" \t#\"{}[]():_-+*/=&|?<>.".contains(&b)
}

/// True when `name` matches `[a-zA-Z][a-zA-Z0-9]*(-[a-zA-Z0-9]+)*`.
pub(crate) fn is_identifier(name: &str) -> bool {
    let b = name.as_bytes();
    if b.first().map_or(true, |c| !c.is_ascii_alphabetic()) {
        return false;
    }
    let mut prev_dash = false;
    for &c in b {
        if c == b'-' {
            if prev_dash {
                return false;
            }
            prev_dash = true;
        } else if c.is_ascii_alphanumeric() {
            prev_dash = false;
        } else {
            return false;
        }
    }
    !prev_dash
}

impl Lexer<'_> {
    fn byte(&self, i: usize) -> Option<u8> {
        if i < self.end {
            self.bytes.get(i).copied()
        } else {
            None
        }
    }

    fn span(&self, start: usize, end: usize) -> Span {
        // `read` rejects sources longer than `u32::MAX - 1`, so these fit.
        Span::new(
            self.file,
            u32::try_from(start).unwrap_or(u32::MAX),
            u32::try_from(end).unwrap_or(u32::MAX),
        )
    }

    fn diag(&mut self, code: DiagCode, start: usize, end: usize, msg: &str) {
        let span = self.span(start, end);
        self.diags.push(Diagnostic::error(code, span, msg));
    }

    /// The byte length of the char starting at `i` (1 when `i` is not on a
    /// char boundary, which the scan never produces).
    fn char_len(&self, i: usize) -> usize {
        self.src
            .get(i..)
            .and_then(|s| s.chars().next())
            .map_or(1, char::len_utf8)
    }

    fn space_before(&self, i: usize) -> bool {
        i == self.line_start || matches!(self.bytes.get(i.wrapping_sub(1)), Some(b' ' | b'\t'))
    }

    /// Start of line, or right after whitespace, `{` or `[` (negation and
    /// keyword position).
    fn prefix_position(&self, i: usize) -> bool {
        i == self.line_start
            || matches!(
                self.bytes.get(i.wrapping_sub(1)),
                Some(b' ' | b'\t' | b'{' | b'[')
            )
    }

    fn push(&self, tokens: &mut Vec<Token>, kind: Tok, start: usize, end: usize) {
        tokens.push(Token {
            kind,
            span: self.span(start, end),
            space_before: self.space_before(start),
        });
    }

    /// Scans tokens from `i`. With `depth > 0` this is an interpolation scan
    /// that stops at the `}` matching its opening brace.
    fn scan(&mut self, mut i: usize, depth: u32) -> Scan {
        let interp = depth > 0;
        let mut tokens = Vec::new();
        let mut braces = 0u32;
        let mut parens = 0u32;
        while let Some(b) = self.byte(i) {
            if matches!(b, b'.' | b'~' | b'/' | b'a'..=b'z') && self.prefix_position(i) {
                if let Some(next) = self.path_or_url(i, &mut tokens) {
                    i = next;
                    continue;
                }
            }
            match b {
                b' ' | b'\t' => i += 1,
                b'#' => {
                    if interp {
                        return Scan {
                            tokens,
                            next: self.end,
                            closed: false,
                        };
                    }
                    let kind = if self.byte(i + 1) == Some(b'@') {
                        TriviaKind::Directive
                    } else {
                        TriviaKind::Comment
                    };
                    self.comment = Some(TriviaItem {
                        span: self.span(i, self.end),
                        kind,
                    });
                    self.code_end = i;
                    i = self.end;
                }
                b'"' => {
                    let (tok, next) = self.string(i, depth);
                    if let Some(kind) = tok {
                        self.push(&mut tokens, kind, i, next);
                    }
                    i = next;
                }
                b'{' => {
                    braces += 1;
                    self.push(&mut tokens, Tok::LBrace, i, i + 1);
                    i += 1;
                }
                b'}' => {
                    if interp && braces == 0 {
                        return Scan {
                            tokens,
                            next: i + 1,
                            closed: true,
                        };
                    }
                    braces = braces.saturating_sub(1);
                    self.push(&mut tokens, Tok::RBrace, i, i + 1);
                    i += 1;
                }
                b'[' => {
                    self.push(&mut tokens, Tok::LBracket, i, i + 1);
                    i += 1;
                }
                b']' => {
                    self.push(&mut tokens, Tok::RBracket, i, i + 1);
                    i += 1;
                }
                b'(' => {
                    parens += 1;
                    self.diag(
                        DiagCode::ParenForm,
                        i,
                        i + 1,
                        "`( )` is not Vactr syntax; use `{}`",
                    );
                    i += 1;
                }
                b')' => {
                    if parens > 0 {
                        parens -= 1;
                    } else {
                        self.diag(
                            DiagCode::ParenForm,
                            i,
                            i + 1,
                            "`( )` is not Vactr syntax; use `{}`",
                        );
                    }
                    i += 1;
                }
                b':' => i = self.colon(i, &mut tokens),
                b'0'..=b'9' => i = self.number(i, &mut tokens),
                b'-' => i = self.minus(i, &mut tokens),
                b'a'..=b'z' | b'A'..=b'Z' | b'_' => i = self.word(i, &mut tokens),
                b'<' | b'>' => {
                    let eq = self.byte(i + 1) == Some(b'=');
                    let op = match (b, eq) {
                        (b'<', true) => Op::Le,
                        (b'<', false) => Op::Lt,
                        (_, true) => Op::Ge,
                        (_, false) => Op::Gt,
                    };
                    let len = if eq { 2 } else { 1 };
                    self.push(&mut tokens, Tok::Op(op), i, i + len);
                    i += len;
                }
                b'.' if self.byte(i + 1) == Some(b'.') => {
                    self.push(&mut tokens, Tok::Op(Op::Range), i, i + 2);
                    i += 2;
                }
                b'+' | b'*' | b'/' | b'=' | b'&' | b'|' | b'?' => {
                    let op = match b {
                        b'+' => Op::Add,
                        b'*' => Op::Mul,
                        b'/' => Op::Div,
                        b'=' => Op::Eq,
                        b'&' => Op::Amp,
                        b'|' => Op::Bar,
                        _ => Op::Question,
                    };
                    self.push(&mut tokens, Tok::Op(op), i, i + 1);
                    i += 1;
                }
                _ => i = self.stray(i),
            }
        }
        Scan {
            tokens,
            next: self.end,
            closed: !interp,
        }
    }

    /// A path or url literal at a keyword position (6.5.8); `None` when the
    /// text there is not one.
    fn path_or_url(&mut self, i: usize, tokens: &mut Vec<Token>) -> Option<usize> {
        let (kind, end) = scan_path_or_url(self.src.get(..self.end)?, i)?;
        // The scan stops only at ASCII bytes, so this slice is on char boundaries.
        let text: Rc<str> = Rc::from(self.src.get(i..end).unwrap_or(""));
        match kind {
            PathTok::Path => self.push(tokens, Tok::Path(text), i, end),
            PathTok::Url => self.push(tokens, Tok::Url(text), i, end),
            PathTok::BadPath => self.diag(
                DiagCode::BadPath,
                i,
                end,
                "malformed path: empty segment, trailing `/`, or nothing after the prefix",
            ),
            PathTok::BadUrl => self.diag(DiagCode::BadUrl, i, end, "a url needs text after `://`"),
        }
        Some(end)
    }

    /// A run of characters outside the token set (`! ; , @ $ ~`, a lone `.`,
    /// control characters, non-ASCII) is one `stray-char`.
    fn stray(&mut self, start: usize) -> usize {
        let mut i = start + self.char_len(start);
        while let Some(b) = self.byte(i) {
            if b.is_ascii() && is_known_ascii(b) {
                break;
            }
            i += self.char_len(i);
        }
        self.diag(
            DiagCode::StrayChar,
            start,
            i,
            "character outside the token set",
        );
        i
    }

    fn minus(&mut self, i: usize, tokens: &mut Vec<Token>) -> usize {
        match self.byte(i + 1) {
            Some(b'0'..=b'9') => self.number(i, tokens),
            Some(b'>') => {
                self.push(tokens, Tok::Op(Op::Arrow), i, i + 2);
                i + 2
            }
            Some(c) if (c.is_ascii_alphabetic() || c == b'{') && self.prefix_position(i) => {
                self.push(tokens, Tok::Neg, i, i + 1);
                i + 1
            }
            _ => {
                self.push(tokens, Tok::Op(Op::Sub), i, i + 1);
                i + 1
            }
        }
    }

    fn digits(&self, mut i: usize) -> usize {
        while matches!(self.byte(i), Some(b'0'..=b'9')) {
            i += 1;
        }
        i
    }

    /// `12`, `-3`, `2.5`, `-0.25`, `1/4`, `-1/4`.
    fn number(&mut self, start: usize, tokens: &mut Vec<Token>) -> usize {
        let body = if self.byte(start) == Some(b'-') {
            start + 1
        } else {
            start
        };
        let mut j = self.digits(body);
        let mut sep = None;
        let next_is_digit = |lx: &Self, k: usize| matches!(lx.byte(k), Some(b'0'..=b'9'));
        match self.byte(j) {
            Some(b'.') if next_is_digit(self, j + 1) => {
                sep = Some((b'.', j));
                j = self.digits(j + 1);
            }
            Some(b'/') if next_is_digit(self, j + 1) => {
                sep = Some((b'/', j));
                j = self.digits(j + 1);
            }
            _ => {}
        }
        let trailing_bad = match self.byte(j) {
            Some(c) if is_word_byte(c) || c == b'/' => true,
            Some(b'.') => self.byte(j + 1) != Some(b'.'),
            _ => false,
        };
        if trailing_bad {
            while matches!(self.byte(j), Some(c) if is_word_byte(c) || c == b'.' || c == b'/') {
                j += 1;
            }
            self.diag(DiagCode::BadNumber, start, j, "malformed number literal");
            return j;
        }
        // The literal is ASCII, so this slice is on char boundaries.
        let text = self.src.get(start..j).unwrap_or("");
        let kind = match sep {
            None => text.parse::<i64>().ok().map(Tok::Int),
            Some((b'.', _)) => text
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .map(|value| Tok::Float {
                    value,
                    exact: Ratio64::from_decimal(text),
                }),
            Some((_, slash)) => {
                let num = self
                    .src
                    .get(start..slash)
                    .and_then(|t| t.parse::<i64>().ok());
                let den = self
                    .src
                    .get(slash + 1..j)
                    .and_then(|t| t.parse::<i64>().ok());
                match (num, den) {
                    (Some(n), Some(d)) => Ratio64::new(n, d).ok().map(Tok::Ratio),
                    _ => None,
                }
            }
        };
        match kind {
            Some(kind) => self.push(tokens, kind, start, j),
            None => self.diag(DiagCode::BadNumber, start, j, "number literal out of range"),
        }
        j
    }

    /// The end of a word: `[A-Za-z0-9_]` runs joined by `-` when a word
    /// character follows the dash.
    fn word_end(&self, mut i: usize) -> usize {
        loop {
            match self.byte(i) {
                Some(c) if is_word_byte(c) => i += 1,
                Some(b'-') if self.byte(i + 1).is_some_and(is_word_byte) => i += 1,
                _ => return i,
            }
        }
    }

    /// Identifiers, `_`, console registers, qualified names, and the
    /// `bad-identifier` forms (`_tmp`, `my_name`, `foo-`).
    fn word(&mut self, start: usize, tokens: &mut Vec<Token>) -> usize {
        let mut j = self.word_end(start);
        let trailing_dash =
            self.byte(j) == Some(b'-') && !matches!(self.byte(j + 1), Some(b'>' | b'{'));
        if trailing_dash {
            while matches!(self.byte(j), Some(c) if is_word_byte(c) || c == b'-') {
                j += 1;
            }
            self.diag(
                DiagCode::BadIdentifier,
                start,
                j,
                "a name may not end in `-`",
            );
            return j;
        }
        let text = self.src.get(start..j).unwrap_or("");
        if text == "_" {
            self.push(tokens, Tok::Wildcard, start, j);
            return j;
        }
        if let Some(digits) = text.strip_prefix('_') {
            let reg = digits
                .strip_prefix(|c: char| ('1'..='9').contains(&c))
                .filter(|rest| rest.bytes().all(|c| c.is_ascii_digit()))
                .and_then(|_| digits.parse::<u32>().ok());
            match reg {
                Some(_) if self.file != FileId::CONSOLE => self.diag(
                    DiagCode::ConsoleRegisterInFile,
                    start,
                    j,
                    "console registers exist only in the console; chain with `>`",
                ),
                Some(n) => self.push(tokens, Tok::ConsoleReg(n), start, j),
                None => self.diag(
                    DiagCode::BadIdentifier,
                    start,
                    j,
                    "`_` is not a name character",
                ),
            }
            return j;
        }
        if !is_identifier(text) {
            self.diag(
                DiagCode::BadIdentifier,
                start,
                j,
                "`_` is not a name character",
            );
            return j;
        }
        let first: Rc<str> = Rc::from(text);
        if self.byte(j) == Some(b'.') && self.byte(j + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            let k = self.word_end(j + 1);
            let name = self.src.get(j + 1..k).unwrap_or("");
            let bad_tail =
                self.byte(k) == Some(b'-') && !matches!(self.byte(k + 1), Some(b'>' | b'{'));
            if is_identifier(name) && !bad_tail {
                let kind = Tok::Qualified {
                    prefix: first,
                    name: Rc::from(name),
                };
                self.push(tokens, kind, start, k);
                return k;
            }
            let mut k = k;
            while matches!(self.byte(k), Some(c) if is_word_byte(c) || c == b'-') {
                k += 1;
            }
            self.diag(
                DiagCode::BadIdentifier,
                start,
                k,
                "malformed qualified name",
            );
            return k;
        }
        self.push(tokens, Tok::Ident(first), start, j);
        j
    }

    /// The colon classes of 6.5.4: block opener, pair key, keyword, or
    /// `misplaced-colon`.
    fn colon(&mut self, i: usize, tokens: &mut Vec<Token>) -> usize {
        let mut rest = i + 1;
        while matches!(self.byte(rest), Some(b' ' | b'\t')) {
            rest += 1;
        }
        if matches!(self.byte(rest), None | Some(b'#')) {
            self.push(tokens, Tok::BlockColon, i, i + 1);
            return i + 1;
        }
        let after_key = tokens
            .last()
            .is_some_and(|t| t.is_key() && t.span.end as usize == i);
        if after_key && matches!(self.byte(i + 1), Some(b' ' | b'\t')) {
            self.push(tokens, Tok::PairColon, i, i + 1);
            return i + 1;
        }
        if self.prefix_position(i) && self.byte(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) {
            let j = self.word_end(i + 1);
            let trailing_dash =
                self.byte(j) == Some(b'-') && !matches!(self.byte(j + 1), Some(b'>' | b'{'));
            let name = self.src.get(i + 1..j).unwrap_or("");
            if is_identifier(name) && !trailing_dash {
                self.push(tokens, Tok::Keyword(Rc::from(name)), i, j);
                return j;
            }
            let mut j = j;
            while matches!(self.byte(j), Some(c) if is_word_byte(c) || c == b'-') {
                j += 1;
            }
            self.diag(
                DiagCode::BadIdentifier,
                i,
                j,
                "a keyword name must be an identifier",
            );
            return j;
        }
        self.diag(DiagCode::MisplacedColon, i, i + 1, "misplaced `:`");
        i + 1
    }

    /// A one-line string with escapes and `{}` interpolation. Returns `None`
    /// (after recording diagnostics) for a malformed string.
    fn string(&mut self, start: usize, depth: u32) -> (Option<Tok>, usize) {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut text_start = start + 1;
        let mut ok = true;
        let mut j = start + 1;
        loop {
            let Some(b) = self.byte(j) else {
                return self.give_up(start, DiagCode::UnterminatedString, 0);
            };
            match b {
                b'"' => {
                    self.flush_text(&mut parts, &mut text, text_start, j);
                    j += 1;
                    break;
                }
                b'\\' => {
                    let decoded = match self.byte(j + 1) {
                        Some(b'"') => Some('"'),
                        Some(b'\\') => Some('\\'),
                        Some(b'n') => Some('\n'),
                        Some(b't') => Some('\t'),
                        Some(b'{') => Some('{'),
                        Some(b'}') => Some('}'),
                        _ => None,
                    };
                    match decoded {
                        Some(c) => {
                            text.push(c);
                            j += 2;
                        }
                        None if self.byte(j + 1).is_none() => j += 1,
                        None => {
                            let len = self.char_len(j + 1);
                            self.diag(DiagCode::BadEscape, j, j + 1 + len, "unknown escape");
                            ok = false;
                            j += 1 + len;
                        }
                    }
                }
                b'{' => {
                    self.flush_text(&mut parts, &mut text, text_start, j);
                    if depth + 1 > MAX_NESTING {
                        return self.give_up(j, DiagCode::NestingTooDeep, 0);
                    }
                    let failures = self.eol_failures;
                    let scan = self.scan(j + 1, depth + 1);
                    if !scan.closed {
                        return self.give_up(start, DiagCode::UnterminatedString, failures);
                    }
                    parts.push(StrPart::Expr {
                        tokens: scan.tokens,
                        span: self.span(j, scan.next),
                    });
                    j = scan.next;
                    text_start = j;
                }
                _ => {
                    let len = self.char_len(j);
                    if let Some(s) = self.src.get(j..j + len) {
                        text.push_str(s);
                    }
                    j += len;
                }
            }
        }
        (ok.then_some(Tok::Str(parts)), j)
    }

    /// Ends a string at the end of the line. The diagnostic is skipped when a
    /// nested string already gave up (`eol_failures` grew past `seen`).
    fn give_up(&mut self, start: usize, code: DiagCode, seen: u32) -> (Option<Tok>, usize) {
        if self.eol_failures <= seen || code == DiagCode::NestingTooDeep {
            let msg = if code == DiagCode::NestingTooDeep {
                "nesting deeper than 128"
            } else {
                "unterminated string"
            };
            self.diag(code, start, self.end, msg);
        }
        self.eol_failures += 1;
        (None, self.end)
    }

    fn flush_text(&self, parts: &mut Vec<StrPart>, text: &mut String, start: usize, end: usize) {
        if !text.is_empty() {
            parts.push(StrPart::Text {
                text: Rc::from(std::mem::take(text).as_str()),
                span: self.span(start, end),
            });
        }
    }
}
