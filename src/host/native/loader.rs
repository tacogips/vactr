//! Native sample and source loading (design 6.5.8, 12.8.10, 17).
//!
//! `NativeSampleLoader` reads only below its configured root directories:
//! every path is resolved (6.5.8: `./` and `../` against the directory of
//! the file that wrote the literal, the console and unknown files against
//! the base directory, `~/` against `$HOME`, `/` as is), canonicalized
//! (symbolic links resolved), and refused unless it lies inside a
//! canonical root. A bank `SampleSrc::Bank { kw, index }` is file
//! `index % count` of the lexically sorted `*.wav` files of `<root>/<kw>/`
//! in the first root that has that directory. A package asset bank
//! registered with `register_bank` (design 14.5.7 "Assets") comes first:
//! its files, indexed by `n` in bytewise path order, are package-cache
//! files and are not subject to the root check.
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
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::ns::load::SourceLoader;
use crate::reader::span::FileId;
use crate::value::intern::{name_of_kw, KwId};
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
    /// Registered package banks: keyword name and canonical files.
    banks: Vec<(Rc<str>, Vec<PathBuf>)>,
    song_sources: BTreeMap<PathBuf, (FileId, Rc<str>)>,
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
            banks: Vec::new(),
            song_sources: BTreeMap::new(),
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
        if let Some((_, files)) = self.0.borrow().banks.iter().find(|(k, _)| &**k == kw) {
            return Ok(files[index as usize % files.len()].clone());
        }
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
            // A captured or rendered buffer carries its own frames
            // (SS-ANALYSIS installs them); there is no file to load.
            SampleSrc::Buffer { id } => {
                return Err(Failure::new(
                    FailCode::HostUnavailable,
                    format!("buffer #{id} is not a file sample"),
                ))
            }
        };
        let bytes = read_limited(&file, MAX_WAV_BYTES, &text)?;
        parse_wav(&bytes)
            .map(Arc::new)
            .map_err(|why| fail(format!("cannot decode `{text}`: {why}")))
    }

    /// Registers a package asset bank. A name already used by a registered
    /// bank or by a sample directory in a root is refused with `load-failed`
    /// (the session reports it as `import-collision`; the first registration
    /// wins). Every file must be a regular file.
    fn register_bank(&mut self, kw: KwId, files: Vec<PathVal>) -> Result<(), Failure> {
        let name = name_of_kw(kw);
        let clash = {
            let st = self.0.borrow();
            st.banks.iter().any(|(k, _)| *k == name)
                || st.roots.iter().any(|r| r.join(&*name).is_dir())
        };
        if clash {
            return Err(Failure::new(
                FailCode::LoadFailed,
                format!("the sample bank `:{name}` already exists"),
            ));
        }
        if files.is_empty() {
            return Err(fail(format!("the sample bank `:{name}` has no file")));
        }
        let mut paths = Vec::with_capacity(files.len());
        for f in &files {
            let real = fs::canonicalize(&*f.text)
                .map_err(|e| fail(format!("cannot read `{}`: {e}", f.text)))?;
            if !real.is_file() {
                return Err(fail(format!("`{}` is not a file", f.text)));
            }
            paths.push(real);
        }
        paths.sort_by(|a, b| {
            a.as_os_str()
                .as_encoded_bytes()
                .cmp(b.as_os_str().as_encoded_bytes())
        });
        self.0.borrow_mut().banks.push((name, paths));
        Ok(())
    }
}

impl SourceLoader for NativeSampleLoader {
    fn song_asset_factory(&self) -> Option<Rc<dyn crate::song::assets::SongAssetFactory>> {
        Some(self.isolated_song_factory())
    }

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

    fn read_bytes(&mut self, path: &PathVal, limit: u64) -> Result<Vec<u8>, Failure> {
        let file = self.resolve(path)?;
        read_limited(&file, limit, &path.text)
    }
}

/// Immutable root/base configuration; no active file IDs or registered banks.
pub struct NativeSongAssetFactory {
    roots: Vec<PathBuf>,
    base: PathBuf,
}
impl crate::song::assets::SongAssetFactory for NativeSongAssetFactory {
    fn begin(
        &self,
        source: crate::song::assets::SongSourceFile,
        limits: crate::song::assets::SongAssetLimits,
    ) -> Result<crate::song::assets::SongAssetPreparation, Failure> {
        let loader = NativeSampleLoader::new(&self.roots, &self.base);
        let text = &*source.path.text;
        let raw = if text == "<console>" {
            self.base.join("__console_origin__.vact")
        } else if let Some(rest) = text.strip_prefix("~/") {
            PathBuf::from(std::env::var_os("HOME").ok_or_else(|| fail("HOME is not set"))?)
                .join(rest)
        } else {
            self.base.join(text)
        };
        let parent = raw
            .parent()
            .ok_or_else(|| fail("source origin has no parent"))?;
        let parent =
            fs::canonicalize(parent).map_err(|e| fail(format!("source origin parent: {e}")))?;
        contained(&loader.roots(), &parent, text)?;
        let name = raw
            .file_name()
            .ok_or_else(|| fail("source origin has no filename"))?;
        // Resolve only the parent: an entry symlink never grants an escaped read.
        let path = parent.join(name);
        loader.register_file(source.file, &path);
        crate::song::assets::SongAssetPreparation::new(Box::new(loader), limits)
    }
}
impl NativeSampleLoader {
    /// Copies configuration, never the mutable active file/bank state.
    #[must_use]
    pub fn isolated_song_factory(&self) -> Rc<dyn crate::song::assets::SongAssetFactory> {
        let state = self.0.borrow();
        Rc::new(NativeSongAssetFactory {
            roots: state.roots.clone(),
            base: state.base.clone(),
        })
    }
}
impl crate::song::assets::SongAssetBackend for NativeSampleLoader {
    fn bank_wraps(&self) -> bool {
        true
    }
    fn read_bounded(
        &mut self,
        path: &PathVal,
        remaining: u64,
    ) -> Result<(FileId, Rc<str>), Failure> {
        let file = self.resolve(path)?;
        if let Some(source) = self.0.borrow().song_sources.get(&file) {
            return Ok(source.clone());
        }
        let size = fs::metadata(&file).map_err(|e| fail(e.to_string()))?.len();
        if size > remaining {
            return Err(Failure::new(
                FailCode::FuelExhausted,
                "remaining source bytes exhausted",
            ));
        }
        let bytes = read_limited(&file, remaining.min(MAX_SOURCE_BYTES), &path.text)?;
        let text = String::from_utf8(bytes).map_err(|_| fail("song source is not UTF-8"))?;
        let mut state = self.0.borrow_mut();
        let id = FileId::new(state.next);
        state.next = state
            .next
            .checked_add(1)
            .ok_or_else(|| Failure::new(FailCode::Overflow, "song file identity overflow"))?;
        let source = (id, Rc::from(text));
        state.files.push((id, file.clone()));
        state.song_sources.insert(file, source.clone());
        Ok(source)
    }
    fn load_bounded(
        &mut self,
        src: &SampleSrc,
        remaining: u64,
    ) -> Result<Arc<SampleData>, Failure> {
        let file = match src {
            SampleSrc::Path(p) => self.resolve(p)?,
            SampleSrc::Bank { kw, index } => self.bank_file(&name_of_kw(*kw), *index)?,
            SampleSrc::Buffer { .. } => return Err(fail("buffer is not a file sample")),
        };
        let bytes = read_limited(&file, MAX_WAV_BYTES, "song WAV")?;
        parse_wav_limited(&bytes, remaining)
            .map(Arc::new)
            .map_err(fail)
    }

    fn bank_len(&mut self, kw: KwId, remaining: u32) -> Result<u32, Failure> {
        let name = name_of_kw(kw);
        if let Some((_, files)) = self.0.borrow().banks.iter().find(|(n, _)| *n == name) {
            let count = u32::try_from(files.len()).map_err(|_| fail("bank count overflow"))?;
            if count > remaining {
                return Err(Failure::new(
                    FailCode::FuelExhausted,
                    "complete bank exceeds remaining resources",
                ));
            }
            return Ok(count);
        }
        if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
            return Err(fail("invalid song bank name"));
        }
        let roots = self.roots();
        let directory = roots
            .iter()
            .map(|r| r.join(&*name))
            .find(|p| p.is_dir())
            .ok_or_else(|| fail(format!("no song sample bank `:{name}`")))?;
        let directory = fs::canonicalize(directory).map_err(|e| fail(e.to_string()))?;
        contained(&roots, &directory, &name)?;
        let mut files = Vec::new();
        for entry in fs::read_dir(directory).map_err(|e| fail(e.to_string()))? {
            let path = entry.map_err(|e| fail(e.to_string()))?.path();
            if !path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("wav"))
            {
                continue;
            }
            if files.len() >= remaining as usize {
                return Err(Failure::new(
                    FailCode::FuelExhausted,
                    "complete bank exceeds remaining resources",
                ));
            }
            let path = fs::canonicalize(path).map_err(|e| fail(e.to_string()))?;
            contained(&roots, &path, &name)?;
            if !path.is_file() {
                return Err(fail("bank entry is not a regular file"));
            }
            files.push(path);
        }
        if files.is_empty() {
            return Err(fail("song bank has no WAV files"));
        }
        files.sort_by(|a, b| {
            a.as_os_str()
                .as_encoded_bytes()
                .cmp(b.as_os_str().as_encoded_bytes())
        });
        let count = u32::try_from(files.len()).map_err(|_| fail("bank count overflow"))?;
        self.0.borrow_mut().banks.push((name, files));
        Ok(count)
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
    parse_wav_limited(b, u64::MAX)
}

fn parse_wav_limited(b: &[u8], remaining: u64) -> Result<SampleData, String> {
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
            let pcm_bytes = u64::try_from(data.len() / align)
                .ok()
                .and_then(|n| n.checked_mul(4))
                .ok_or("decoded song PCM size overflow")?;
            if pcm_bytes > remaining {
                return Err("decoded song PCM exceeds remaining capacity".into());
            }
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
