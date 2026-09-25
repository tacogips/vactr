//! Native sample and source loading (design 6.5.8, 12.8.10, 17).
//!
//! `NativeSampleLoader` reads only below its configured root directories:
//! every path is resolved (6.5.8: `./` and `../` against the directory of
//! the file that wrote the literal, the console and unknown files against
//! the base directory, `~/` against `$HOME`, `/` as is), canonicalized
//! (symbolic links resolved), and refused unless it lies inside a
//! canonical root. A bank `SampleSrc::Bank { kw, index }` is file
//! `index % count` of the lexically sorted `*.wav` files of `<root>/<kw>/`
//! in the first root that has that directory.
//!
//! Samples are RIFF WAV only, decoded by `parse_wav` (PCM 16/24/32-bit
//! integer and 32-bit float, mono or stereo, `WAVE_FORMAT_EXTENSIBLE`
//! included). Anything else, and any malformed file, is a
//! `host-unavailable` failure with the reason; nothing here panics. The
//! file's own rate is kept in `SampleData::rate` (the engine scales
//! playback speed).
//!
//! Clones share the table of loaded source files, so the sample loader in
//! `Hosts` resolves paths written in a file the evaluator's source loader
//! read.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::ns::load::SourceLoader;
use crate::reader::span::FileId;
use crate::value::intern::name_of_kw;
use crate::value::value::PathVal;
use crate::vm::fail::{FailCode, Failure};

/// The largest WAV file read.
pub const MAX_WAV_BYTES: u64 = 512 * 1024 * 1024;
/// The largest source file read.
pub const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;
/// The first `FileId` given to a loaded source file (below the prelude
/// template file id, above any session buffer id).
pub const FIRST_LOADED_FILE: u32 = 0x1000_0000;

fn fail(message: impl Into<String>) -> Failure {
    Failure::new(FailCode::HostUnavailable, message)
}

#[derive(Debug)]
struct State {
    roots: Vec<PathBuf>,
    base: PathBuf,
    files: Vec<(FileId, PathBuf)>,
    next: u32,
}

/// Reads WAV samples and source files below configured roots.
#[derive(Clone, Debug)]
pub struct NativeSampleLoader(Rc<RefCell<State>>);

impl NativeSampleLoader {
    /// A loader over `roots` (roots that do not exist are ignored);
    /// console-relative paths resolve against `base`.
    #[must_use]
    pub fn new(roots: &[PathBuf], base: &Path) -> Self {
        let roots = roots
            .iter()
            .filter_map(|r| fs::canonicalize(r).ok())
            .collect();
        Self(Rc::new(RefCell::new(State {
            roots,
            base: base.to_path_buf(),
            files: Vec::new(),
            next: FIRST_LOADED_FILE,
        })))
    }

    /// The canonical roots.
    #[must_use]
    pub fn roots(&self) -> Vec<PathBuf> {
        self.0.borrow().roots.clone()
    }

    /// Tells the loader where the source file `file` lives, so relative
    /// literals written in it resolve against its directory.
    pub fn register_file(&self, file: FileId, path: &Path) {
        let mut st = self.0.borrow_mut();
        st.files.retain(|(f, _)| *f != file);
        st.files.push((file, path.to_path_buf()));
    }

    /// Resolves a path literal and checks it lies inside a root.
    ///
    /// # Errors
    /// `host-unavailable` when the file does not exist or lies outside
    /// every root.
    pub fn resolve(&self, path: &PathVal) -> Result<PathBuf, Failure> {
        let st = self.0.borrow();
        let text: &str = &path.text;
        let raw = if let Some(rest) = text.strip_prefix("~/") {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| fail(format!("cannot resolve `{text}`: HOME is not set")))?;
            PathBuf::from(home).join(rest)
        } else if text.starts_with('/') {
            PathBuf::from(text)
        } else {
            let dir = path
                .file
                .and_then(|f| st.files.iter().find(|(id, _)| *id == f))
                .and_then(|(_, p)| p.parent().map(Path::to_path_buf))
                .unwrap_or_else(|| st.base.clone());
            dir.join(text)
        };
        let real =
            fs::canonicalize(&raw).map_err(|e| fail(format!("cannot read `{text}`: {e}")))?;
        contained(&st.roots, &real, text)?;
        Ok(real)
    }

    /// The file of a bank entry.
    fn bank_file(&self, kw: &str, index: u32) -> Result<PathBuf, Failure> {
        let bad = kw.is_empty() || kw.starts_with('.') || kw.contains(['/', '\\']);
        if bad {
            return Err(fail(format!("`:{kw}` cannot name a sample directory")));
        }
        let roots = self.roots();
        let dir = roots
            .iter()
            .map(|r| r.join(kw))
            .find(|d| d.is_dir())
            .ok_or_else(|| fail(format!("no sample directory `{kw}` in the sample roots")))?;
        let dir = fs::canonicalize(&dir).map_err(|e| fail(format!("cannot read `:{kw}`: {e}")))?;
        contained(&roots, &dir, kw)?;
        let mut wavs: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(|e| fail(format!("cannot read `:{kw}`: {e}")))?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("wav")))
            .collect();
        if wavs.is_empty() {
            return Err(fail(format!(
                "the sample directory `{kw}` has no .wav file"
            )));
        }
        wavs.sort();
        let i = index as usize % wavs.len();
        let file = fs::canonicalize(&wavs[i])
            .map_err(|e| fail(format!("cannot read `:{kw} {index}`: {e}")))?;
        contained(&roots, &file, kw)?;
        Ok(file)
    }
}

fn contained(roots: &[PathBuf], real: &Path, text: &str) -> Result<(), Failure> {
    let inside = roots.iter().any(|r| real.starts_with(r));
    if inside {
        Ok(())
    } else {
        Err(fail(format!(
            "`{text}` is outside the configured sample directories"
        )))
    }
}

fn read_limited(path: &Path, limit: u64, text: &str) -> Result<Vec<u8>, Failure> {
    let meta = fs::metadata(path).map_err(|e| fail(format!("cannot read `{text}`: {e}")))?;
    if !meta.is_file() {
        return Err(fail(format!("`{text}` is not a file")));
    }
    if meta.len() > limit {
        return Err(fail(format!("`{text}` is larger than {limit} bytes")));
    }
    fs::read(path).map_err(|e| fail(format!("cannot read `{text}`: {e}")))
}

impl SampleLoader for NativeSampleLoader {
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let (file, text) = match src {
            SampleSrc::Path(p) => (self.resolve(p)?, p.text.to_string()),
            SampleSrc::Bank { kw, index } => {
                let kw = name_of_kw(*kw);
                (self.bank_file(&kw, *index)?, format!(":{kw} {index}"))
            }
        };
        let bytes = read_limited(&file, MAX_WAV_BYTES, &text)?;
        parse_wav(&bytes)
            .map(Arc::new)
            .map_err(|why| fail(format!("cannot decode `{text}`: {why}")))
    }
}

impl SourceLoader for NativeSampleLoader {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        let file = self.resolve(path)?;
        let bytes = read_limited(&file, MAX_SOURCE_BYTES, &path.text)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| fail(format!("`{}` is not UTF-8 text", path.text)))?;
        let mut st = self.0.borrow_mut();
        let id = FileId::new(st.next);
        st.next = st.next.saturating_add(1);
        st.files.push((id, file));
        Ok((id, Rc::from(text)))
    }
}

/// The sample encoding of a `fmt ` chunk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Encoding {
    Int(u16),
    Float,
}

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Decodes a RIFF WAV file into interleaved `f32` frames.
///
/// # Errors
/// The reason, for anything that is not a well-formed mono or stereo
/// PCM 16/24/32 or float 32 WAV.
pub fn parse_wav(b: &[u8]) -> Result<SampleData, String> {
    if b.len() < 12 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" {
        return Err("not a RIFF WAVE file".into());
    }
    let mut at = 12;
    let mut fmt: Option<(Encoding, u8, u32, usize)> = None;
    while at + 8 <= b.len() {
        let id = &b[at..at + 4];
        let size = u32_at(b, at + 4).ok_or("truncated chunk header")? as usize;
        let body = at + 8;
        let end = body.checked_add(size).ok_or("chunk size overflows")?;
        if id == b"fmt " {
            let chunk = b.get(body..end).ok_or("truncated fmt chunk")?;
            fmt = Some(parse_fmt(chunk)?);
        } else if id == b"data" {
            let (enc, channels, rate, align) = fmt.ok_or("data chunk before fmt chunk")?;
            let data = b.get(body..end).ok_or("truncated data chunk")?;
            let frame = align * usize::from(channels);
            let data = &data[..data.len() - data.len() % frame];
            return Ok(SampleData {
                rate,
                channels,
                frames: decode(data, enc, align),
            });
        }
        at = end + (size & 1);
    }
    Err("no data chunk".into())
}

fn parse_fmt(c: &[u8]) -> Result<(Encoding, u8, u32, usize), String> {
    let short = || "truncated fmt chunk".to_string();
    let mut tag = u16_at(c, 0).ok_or_else(short)?;
    let channels = u16_at(c, 2).ok_or_else(short)?;
    let rate = u32_at(c, 4).ok_or_else(short)?;
    let align = u16_at(c, 12).ok_or_else(short)?;
    let bits = u16_at(c, 14).ok_or_else(short)?;
    if tag == 0xFFFE {
        tag = u16_at(c, 24).ok_or("truncated extensible fmt chunk")?;
    }
    let enc = match (tag, bits) {
        (1, 16 | 24 | 32) => Encoding::Int(bits),
        (3, 32) => Encoding::Float,
        (1 | 3, _) => return Err(format!("{bits}-bit samples are not supported")),
        _ => {
            return Err(format!(
                "WAV format {tag} is not supported (PCM or float only)"
            ))
        }
    };
    if !(1..=2).contains(&channels) {
        return Err(format!(
            "{channels} channels are not supported (mono or stereo only)"
        ));
    }
    if rate == 0 {
        return Err("a sample rate of 0".into());
    }
    if usize::from(align) != usize::from(channels) * usize::from(bits / 8) {
        return Err("the block alignment does not match the format".into());
    }
    #[allow(clippy::cast_possible_truncation)]
    Ok((enc, channels as u8, rate, usize::from(bits / 8)))
}

fn decode(data: &[u8], enc: Encoding, width: usize) -> Box<[f32]> {
    data.chunks_exact(width)
        .map(|s| {
            let v = match enc {
                Encoding::Int(16) => f32::from(i16::from_le_bytes([s[0], s[1]])) / 32_768.0,
                Encoding::Int(24) => {
                    let x = i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8;
                    #[allow(clippy::cast_precision_loss)]
                    let y = x as f32 / 8_388_608.0;
                    y
                }
                Encoding::Int(_) => {
                    let x = i32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                    #[allow(clippy::cast_precision_loss)]
                    let y = x as f32 / 2_147_483_648.0;
                    y
                }
                Encoding::Float => f32::from_le_bytes([s[0], s[1], s[2], s[3]]),
            };
            if v.is_finite() {
                v
            } else {
                0.0
            }
        })
        .collect()
}
