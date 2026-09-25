//! Unquoted path and url literals (design 6.5.8).
//!
//! The lexer calls `scan_path_or_url` at a token start where a keyword may
//! start, before the number, operator and name rules. The scan only looks at
//! ASCII bytes, so every end it returns is a char boundary.

/// The kind of a scanned path or url token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathTok {
    Path,
    Url,
    /// A path prefix with an empty segment, a trailing `/`, or nothing after it.
    BadPath,
    /// `scheme://` with nothing after it.
    BadUrl,
}

/// `[A-Za-z0-9._~-]`.
fn is_path_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-')
}

/// Printable ASCII other than whitespace and `" # { } [ ] \ < > | ^` and backtick.
fn is_url_char(b: u8) -> bool {
    b.is_ascii_graphic() && !b"\"#{}[]\\<>|^`".contains(&b)
}

fn is_scheme_char(b: u8) -> bool {
    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'+' | b'.' | b'-')
}

/// Scans a path or url starting at byte `at` of `src`, which must end at the
/// line end. Returns the token kind and the byte after it, or `None` when
/// the text at `at` is not a path or url (it is then lexed as before).
pub(crate) fn scan_path_or_url(src: &str, at: usize) -> Option<(PathTok, usize)> {
    let bytes = src.as_bytes();
    let at_byte = |i: usize| bytes.get(i).copied();
    let prefix_len = match (at_byte(at), at_byte(at + 1), at_byte(at + 2)) {
        (Some(b'.'), Some(b'.'), Some(b'/')) => 3,
        (Some(b'.' | b'~'), Some(b'/'), _) => 2,
        (Some(b'/'), Some(c), _) if is_path_char(c) => 1,
        (Some(b'a'..=b'z'), _, _) => return scan_url(bytes, at),
        _ => return None,
    };
    let mut end = at + prefix_len;
    while at_byte(end).is_some_and(|c| is_path_char(c) || c == b'/') {
        end += 1;
    }
    let rest = bytes.get(at + prefix_len..end).unwrap_or(&[]);
    let valid = !rest.is_empty() && rest.split(|&c| c == b'/').all(|seg| !seg.is_empty());
    let kind = if valid {
        PathTok::Path
    } else {
        PathTok::BadPath
    };
    Some((kind, end))
}

/// `SCHEME "://" REST`; `None` without the `://`.
fn scan_url(bytes: &[u8], at: usize) -> Option<(PathTok, usize)> {
    let mut i = at + 1;
    while bytes.get(i).copied().is_some_and(is_scheme_char) {
        i += 1;
    }
    if bytes.get(i..i + 3) != Some(b"://".as_slice()) {
        return None;
    }
    let rest = i + 3;
    let mut end = rest;
    while bytes.get(end).copied().is_some_and(is_url_char) {
        end += 1;
    }
    let kind = if end > rest {
        PathTok::Url
    } else {
        PathTok::BadUrl
    };
    Some((kind, end))
}
