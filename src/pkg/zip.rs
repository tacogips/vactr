//! A minimal, defensive ZIP reader for proxy archives (design 14.5.7).
//!
//! Only the end-of-central-directory record and the central directory are
//! trusted for names, kinds and sizes; local headers are read only to find
//! each entry's data. Refused: encryption, zip64, multi-disk archives and
//! any method other than stored (0) or deflate (8). A Unix symlink (mode
//! `0o120000` in the external attributes) is `EntryKind::Symlink`. The
//! whole entry list is validated (`validate_entries`) BEFORE any content is
//! inflated; each inflation is capped at its declared size, which must
//! equal the inflated length. Entry paths are relative to the package
//! root. Integrity is carried by the canonical digest, not by the ZIP
//! CRC-32, which is not checked. Nothing here panics on untrusted bytes.

use crate::pkg::validate::{validate_entries, Budget, EntryKind, IntegrityError, Limits};

const EOCD_SIG: u32 = 0x0605_4b50;
const ZIP64_LOCATOR_SIG: u32 = 0x0706_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const ARCHIVE: &str = "<archive>";

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn bad(reason: &str) -> IntegrityError {
    IntegrityError::new(ARCHIVE, reason)
}

struct Central {
    name: String,
    kind: EntryKind,
    method: u16,
    flags: u16,
    compressed: u32,
    size: u32,
    local: u32,
}

/// Finds the end-of-central-directory record (searching back over at most
/// the 64 KiB comment).
fn find_eocd(b: &[u8]) -> Result<usize, IntegrityError> {
    if b.len() < 22 {
        return Err(bad("too short to be a zip archive"));
    }
    let last = b.len() - 22;
    let first = last.saturating_sub(0xffff);
    (first..=last)
        .rev()
        .find(|&i| u32_at(b, i) == Some(EOCD_SIG))
        .ok_or_else(|| bad("no end-of-central-directory record"))
}

fn kind_of(made_by: u16, external: u32, dir_name: bool) -> EntryKind {
    // Unix (3) and macOS (19) store the mode in the high 16 bits.
    let unix = matches!(made_by >> 8, 3 | 19);
    let mode = external >> 16;
    if unix && mode != 0 {
        return match mode & 0o170_000 {
            0o120_000 => EntryKind::Symlink,
            0o040_000 => EntryKind::Dir,
            0o100_000 => EntryKind::File,
            _ => EntryKind::Other,
        };
    }
    if dir_name || external & 0x10 != 0 {
        EntryKind::Dir
    } else {
        EntryKind::File
    }
}

fn central_directory(b: &[u8], limits: &Limits) -> Result<Vec<Central>, IntegrityError> {
    let eocd = find_eocd(b)?;
    if eocd >= 20 && u32_at(b, eocd - 20) == Some(ZIP64_LOCATOR_SIG) {
        return Err(bad("zip64 archives are refused"));
    }
    let short = || bad("truncated end-of-central-directory record");
    let disk = u16_at(b, eocd + 4).ok_or_else(short)?;
    let cd_disk = u16_at(b, eocd + 6).ok_or_else(short)?;
    let on_disk = u16_at(b, eocd + 8).ok_or_else(short)?;
    let total = u16_at(b, eocd + 10).ok_or_else(short)?;
    let cd_size = u32_at(b, eocd + 12).ok_or_else(short)?;
    let cd_offset = u32_at(b, eocd + 16).ok_or_else(short)?;
    if total == 0xffff || cd_size == 0xffff_ffff || cd_offset == 0xffff_ffff {
        return Err(bad("zip64 archives are refused"));
    }
    if disk != 0 || cd_disk != 0 || on_disk != total {
        return Err(bad("multi-disk archives are refused"));
    }
    if usize::from(total) > limits.max_entries {
        return Err(bad(&format!("more than {} entries", limits.max_entries)));
    }
    let mut at = cd_offset as usize;
    let end = at
        .checked_add(cd_size as usize)
        .filter(|&e| e <= eocd)
        .ok_or_else(|| bad("the central directory is out of bounds"))?;
    let mut out = Vec::with_capacity(usize::from(total));
    for _ in 0..total {
        let short = || bad("truncated central directory");
        if at >= end || u32_at(b, at) != Some(CENTRAL_SIG) {
            return Err(short());
        }
        let made_by = u16_at(b, at + 4).ok_or_else(short)?;
        let flags = u16_at(b, at + 8).ok_or_else(short)?;
        let method = u16_at(b, at + 10).ok_or_else(short)?;
        let compressed = u32_at(b, at + 20).ok_or_else(short)?;
        let size = u32_at(b, at + 24).ok_or_else(short)?;
        let name_len = usize::from(u16_at(b, at + 28).ok_or_else(short)?);
        let extra_len = usize::from(u16_at(b, at + 30).ok_or_else(short)?);
        let comment_len = usize::from(u16_at(b, at + 32).ok_or_else(short)?);
        let disk_start = u16_at(b, at + 34).ok_or_else(short)?;
        let external = u32_at(b, at + 38).ok_or_else(short)?;
        let local = u32_at(b, at + 42).ok_or_else(short)?;
        let raw = b.get(at + 46..at + 46 + name_len).ok_or_else(short)?;
        let Ok(name) = std::str::from_utf8(raw) else {
            return Err(IntegrityError::new(
                &String::from_utf8_lossy(raw),
                "the name is not UTF-8",
            ));
        };
        if compressed == 0xffff_ffff || size == 0xffff_ffff || local == 0xffff_ffff {
            return Err(IntegrityError::new(name, "zip64 entries are refused"));
        }
        if disk_start != 0 {
            return Err(IntegrityError::new(name, "multi-disk archives are refused"));
        }
        let dir_name = name.ends_with('/');
        let kind = kind_of(made_by, external, dir_name);
        let name = if dir_name && kind == EntryKind::Dir {
            name.trim_end_matches('/')
        } else {
            name
        };
        out.push(Central {
            name: name.to_string(),
            kind,
            method,
            flags,
            compressed,
            size,
            local,
        });
        at = at + 46 + name_len + extra_len + comment_len;
    }
    Ok(out)
}

/// Reads a zip archive's regular files `(path, contents)`, in archive
/// order. Directory entries are validated and dropped.
///
/// # Errors
/// `package-integrity` naming the archive or the offending entry.
pub fn read_zip(bytes: &[u8], limits: &Limits) -> Result<Vec<(String, Vec<u8>)>, IntegrityError> {
    let entries = central_directory(bytes, limits)?;
    for e in &entries {
        if e.flags & 0x41 != 0 {
            return Err(IntegrityError::new(
                &e.name,
                "encrypted entries are refused",
            ));
        }
    }
    let listed: Vec<(String, EntryKind, u64)> = entries
        .iter()
        .map(|e| (e.name.clone(), e.kind, u64::from(e.size)))
        .collect();
    validate_entries(&listed, limits)?;
    let mut budget = Budget::new(*limits);
    let mut out = Vec::new();
    for e in entries.iter().filter(|e| e.kind == EntryKind::File) {
        let name = e.name.as_str();
        let short = || IntegrityError::new(name, "truncated local header");
        let at = e.local as usize;
        if u32_at(bytes, at) != Some(LOCAL_SIG) {
            return Err(short());
        }
        if u16_at(bytes, at.saturating_add(6)).ok_or_else(short)? & 0x41 != 0 {
            return Err(IntegrityError::new(name, "encrypted entries are refused"));
        }
        let name_len = usize::from(u16_at(bytes, at.saturating_add(26)).ok_or_else(short)?);
        let extra_len = usize::from(u16_at(bytes, at.saturating_add(28)).ok_or_else(short)?);
        let data = at
            .checked_add(30 + name_len + extra_len)
            .and_then(|start| Some(start..start.checked_add(e.compressed as usize)?))
            .and_then(|range| bytes.get(range))
            .ok_or_else(|| IntegrityError::new(name, "the entry data is out of bounds"))?;
        budget.admit(name, u64::from(e.size))?;
        let declared = e.size as usize;
        let contents = match e.method {
            0 => data.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(data, declared).map_err(
                |_| {
                    IntegrityError::new(
                        name,
                        "the deflate stream is corrupt or larger than declared",
                    )
                },
            )?,
            m => {
                return Err(IntegrityError::new(
                    name,
                    format!("compression method {m} is refused (stored or deflate only)"),
                ))
            }
        };
        if contents.len() != declared {
            return Err(IntegrityError::new(
                name,
                format!(
                    "{} bytes after extraction, {declared} declared",
                    contents.len()
                ),
            ));
        }
        out.push((e.name.clone(), contents));
    }
    Ok(out)
}
