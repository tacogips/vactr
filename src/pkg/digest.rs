//! The canonical content digest (design 5.7 revised, 14.5.7).
//!
//! The lock digest is taken over the package's FILE TREE, never over an
//! archive: for every regular file, in bytewise order of its normalized
//! relative path, the record `u32_be(len(path)) || path || sha256(contents)`;
//! the digest is the SHA-256 of the concatenated records. The length prefix
//! makes the encoding injective (no file name can forge a record boundary),
//! so a git checkout, a local directory and an extracted proxy zip of the
//! same sources give the same digest.

use std::rc::Rc;

use crate::pkg::sha256::sha256;

/// The canonical record bytes of `files` (`(path, content hash)`), sorted
/// bytewise by path. Paths are bounded by validation, far below `u32::MAX`.
#[must_use]
pub fn canonical_bytes(files: &[(Rc<str>, [u8; 32])]) -> Vec<u8> {
    let mut sorted: Vec<&(Rc<str>, [u8; 32])> = files.iter().collect();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let mut out = Vec::with_capacity(sorted.iter().map(|(p, _)| 36 + p.len()).sum());
    for (path, hash) in sorted {
        let len = u32::try_from(path.len()).unwrap_or(u32::MAX);
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(path.as_bytes());
        out.extend_from_slice(hash);
    }
    out
}

/// The lock digest of `files` (`(path, content hash)`).
#[must_use]
pub fn tree_digest(files: &[(Rc<str>, [u8; 32])]) -> [u8; 32] {
    sha256(&canonical_bytes(files))
}

/// The lock digest of files given with their contents.
#[must_use]
pub fn sources_digest(files: &[(Rc<str>, Rc<[u8]>)]) -> [u8; 32] {
    let hashed: Vec<(Rc<str>, [u8; 32])> = files
        .iter()
        .map(|(p, bytes)| (Rc::clone(p), sha256(bytes)))
        .collect();
    tree_digest(&hashed)
}

/// Lowercase hex.
#[must_use]
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from(DIGITS[usize::from(b >> 4)]));
        s.push(char::from(DIGITS[usize::from(b & 0xf)]));
    }
    s
}

/// Parses exactly 64 LOWERCASE hex digits.
#[must_use]
pub fn parse_hex(s: &str) -> Option<[u8; 32]> {
    fn digit(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            _ => None,
        }
    }
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in b.chunks_exact(2).enumerate() {
        out[i] = (digit(pair[0])? << 4) | digit(pair[1])?;
    }
    Some(out)
}
