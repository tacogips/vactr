//! Test support: a self-removing temp dir, a stored/deflate ZIP writer
//! (fixture zips are generated at test time), local-directory fixtures, an
//! in-memory store and transport, and a tree snapshot.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ns::pkg::PackageId;
use crate::pkg::manifest::PkgManifest;
use crate::pkg::proxy::ProxyTransport;
use crate::pkg::semver::Version;
use crate::pkg::sha256::sha256;
use crate::pkg::store::{parse_manifest, PackageStore, PkgError, StagingSink, MANIFEST_FILE};

pub(super) const PADS: &str = "github.com/someone/vactr-pads";
pub(super) const DRUMS: &str = "github.com/someone/vactr-drums";

pub(super) fn id(path: &str) -> PackageId {
    PackageId::new(path)
}

pub(super) fn v(tag: &str) -> Version {
    Version::parse_tag(tag).expect("a test version")
}

/// A unique directory under `std::env::temp_dir()`, removed on drop.
pub(super) struct TempDir(PathBuf);

impl TempDir {
    pub(super) fn new() -> TempDir {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir = std::env::temp_dir().join(format!(
            "vactr-pkg-test-{}-{nanos:x}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("create the temp dir");
        TempDir(dir)
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }

    pub(super) fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Writes `files` under `root`, creating directories.
pub(super) fn write_tree(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, bytes) in files {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
        fs::write(&path, bytes).expect("write a fixture file");
    }
}

/// Writes a local-directory store package `<root>/<path>@<version>/`.
pub(super) fn dir_package(
    root: &Path,
    path: &str,
    version: &str,
    files: &[(&str, &[u8])],
) -> PathBuf {
    let dir = root.join(format!("{path}@{version}"));
    fs::create_dir_all(&dir).expect("mkdir");
    write_tree(&dir, files);
    dir
}

/// Every entry under `root` (directories as `path/`), with file contents:
/// a byte-level snapshot for "no mutation" checks.
pub(super) fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn go(dir: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(rd) = fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let name = format!("{rel}{}", e.file_name().to_string_lossy());
            let meta = fs::symlink_metadata(e.path()).expect("metadata");
            if meta.is_dir() {
                out.push((format!("{name}/"), Vec::new()));
                go(&e.path(), &format!("{name}/"), out);
            } else {
                out.push((name, fs::read(e.path()).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    go(root, "", &mut out);
    out.sort();
    out
}

/// A mono 16-bit PCM WAV of `frames` frames.
pub(super) fn wav(frames: u16) -> Vec<u8> {
    let data_len = u32::from(frames) * 2;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&44_100u32.to_le_bytes());
    b.extend_from_slice(&88_200u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        b.extend_from_slice(&(i16::try_from(i % 100).unwrap_or(0) * 100).to_le_bytes());
    }
    b
}

/// The `vactr-pads` fixture sources: a manifest with an asset directory,
/// one source file and a two-file bank `warm`.
pub(super) fn pads_files() -> Vec<(String, Vec<u8>)> {
    vec![
        (
            MANIFEST_FILE.into(),
            format!("[package]\npath = \"{PADS}\"\nassets = [\"samples\"]\n\n[deps]\n")
                .into_bytes(),
        ),
        (
            "pads.vact".into(),
            b"let warm 3\nlet hot + warm 1\n".to_vec(),
        ),
        ("samples/warm/a.wav".into(), wav(8)),
        ("samples/warm/b.wav".into(), wav(16)),
        ("samples/README".into(), b"not a bank".to_vec()),
    ]
}

pub(super) fn as_refs(files: &[(String, Vec<u8>)]) -> Vec<(&str, &[u8])> {
    files
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect()
}

/// `(path, sha256)` records of `files`.
pub(super) fn hashed(files: &[(&str, &[u8])]) -> Vec<(Rc<str>, [u8; 32])> {
    files
        .iter()
        .map(|(p, b)| (Rc::from(*p), sha256(b)))
        .collect()
}

/// A manifest text with only `[deps]`.
pub(super) fn deps_manifest(pkg: Option<&str>, deps: &[(&str, &str)]) -> String {
    let mut s = String::new();
    if let Some(p) = pkg {
        s.push_str(&format!("[package]\npath = \"{p}\"\n\n"));
    }
    s.push_str("[deps]\n");
    for (d, ver) in deps {
        s.push_str(&format!("\"{d}\" = \"{ver}\"\n"));
    }
    s
}

/// One entry of a test zip.
#[derive(Clone)]
pub(super) struct ZipSpec {
    pub name: Vec<u8>,
    pub data: Vec<u8>,
    pub deflate: bool,
    pub flags: u16,
    pub method: Option<u16>,
    /// The declared uncompressed size (default: the data length).
    pub declared: Option<u32>,
    /// A Unix mode in the external attributes (made by Unix).
    pub mode: Option<u32>,
}

impl ZipSpec {
    pub(super) fn file(name: &str, data: &[u8]) -> ZipSpec {
        ZipSpec {
            name: name.as_bytes().to_vec(),
            data: data.to_vec(),
            deflate: false,
            flags: 0,
            method: None,
            declared: None,
            mode: None,
        }
    }

    pub(super) fn deflated(mut self) -> ZipSpec {
        self.deflate = true;
        self
    }

    pub(super) fn dir(name: &str) -> ZipSpec {
        let mut z = ZipSpec::file(&format!("{name}/"), b"");
        z.mode = Some(0o040_755);
        z
    }

    pub(super) fn symlink(name: &str, target: &str) -> ZipSpec {
        let mut z = ZipSpec::file(name, target.as_bytes());
        z.mode = Some(0o120_777);
        z
    }
}

/// A zip of `(path, contents)` (stored).
pub(super) fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let specs: Vec<ZipSpec> = files.iter().map(|(p, b)| ZipSpec::file(p, b)).collect();
    write_zip(&specs, 0)
}

/// Writes a zip archive (no CRC; the reader does not check it). `disk` is
/// written as the end record's disk number.
pub(super) fn write_zip(entries: &[ZipSpec], disk: u16) -> Vec<u8> {
    fn u16le(b: &mut Vec<u8>, x: u16) {
        b.extend_from_slice(&x.to_le_bytes());
    }
    fn u32le(b: &mut Vec<u8>, x: u32) {
        b.extend_from_slice(&x.to_le_bytes());
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for e in entries {
        let body = if e.deflate {
            miniz_oxide::deflate::compress_to_vec(&e.data, 6)
        } else {
            e.data.clone()
        };
        let method = e.method.unwrap_or(if e.deflate { 8 } else { 0 });
        let size = e
            .declared
            .unwrap_or(u32::try_from(e.data.len()).expect("small"));
        let comp = u32::try_from(body.len()).expect("small");
        let name_len = u16::try_from(e.name.len()).expect("short name");
        let offset = u32::try_from(out.len()).expect("small");
        u32le(&mut out, 0x0403_4b50);
        u16le(&mut out, 20);
        u16le(&mut out, e.flags);
        u16le(&mut out, method);
        u32le(&mut out, 0);
        u32le(&mut out, 0);
        u32le(&mut out, comp);
        u32le(&mut out, size);
        u16le(&mut out, name_len);
        u16le(&mut out, 0);
        out.extend_from_slice(&e.name);
        out.extend_from_slice(&body);
        let (made_by, external) = match e.mode {
            Some(m) => ((3u16 << 8) | 20, m << 16),
            None => (20, 0),
        };
        u32le(&mut central, 0x0201_4b50);
        u16le(&mut central, made_by);
        u16le(&mut central, 20);
        u16le(&mut central, e.flags);
        u16le(&mut central, method);
        u32le(&mut central, 0);
        u32le(&mut central, 0);
        u32le(&mut central, comp);
        u32le(&mut central, size);
        u16le(&mut central, name_len);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u16le(&mut central, 0);
        u32le(&mut central, external);
        u32le(&mut central, offset);
        central.extend_from_slice(&e.name);
    }
    let cd_offset = u32::try_from(out.len()).expect("small");
    let cd_size = u32::try_from(central.len()).expect("small");
    let count = u16::try_from(entries.len()).expect("few entries");
    out.extend_from_slice(&central);
    u32le(&mut out, 0x0605_4b50);
    u16le(&mut out, disk);
    u16le(&mut out, 0);
    u16le(&mut out, count);
    u16le(&mut out, count);
    u32le(&mut out, cd_size);
    u32le(&mut out, cd_offset);
    u16le(&mut out, 0);
    out
}

/// A package's files, `(path, contents)`.
pub(super) type Files = Vec<(String, Vec<u8>)>;

/// An in-memory store: `(path, version)` -> files. It writes files to the
/// staging sink unchecked, so the cache's own validation is exercised.
#[derive(Default)]
pub(super) struct MemStore {
    pub pkgs: BTreeMap<(PackageId, Version), Files>,
}

impl MemStore {
    pub(super) fn add(&mut self, path: &str, version: &str, files: Vec<(String, Vec<u8>)>) {
        self.pkgs.insert((id(path), v(version)), files);
    }

    /// A package with only a manifest requiring `deps`.
    pub(super) fn add_req(&mut self, path: &str, version: &str, deps: &[(&str, &str)]) {
        let m = deps_manifest(Some(path), deps);
        self.add(path, version, vec![(MANIFEST_FILE.into(), m.into_bytes())]);
    }
}

impl PackageStore for MemStore {
    fn list_versions(&mut self, pkg: &PackageId) -> Result<Vec<Version>, PkgError> {
        let out: Vec<Version> = self
            .pkgs
            .keys()
            .filter(|(p, _)| p == pkg)
            .map(|(_, ver)| ver.clone())
            .collect();
        if out.is_empty() {
            Err(PkgError::Unresolvable(format!("no package `{}`", pkg.0)))
        } else {
            Ok(out)
        }
    }

    fn manifest(&mut self, pkg: &PackageId, version: &Version) -> Result<PkgManifest, PkgError> {
        let files = self
            .pkgs
            .get(&(pkg.clone(), version.clone()))
            .ok_or_else(|| PkgError::Unresolvable("no such version".into()))?;
        match files.iter().find(|(p, _)| p == MANIFEST_FILE) {
            Some((_, b)) => parse_manifest(pkg, b),
            None => Ok(PkgManifest::default()),
        }
    }

    fn fetch_into(
        &mut self,
        pkg: &PackageId,
        version: &Version,
        staging: &mut dyn StagingSink,
    ) -> Result<(), PkgError> {
        let files = self
            .pkgs
            .get(&(pkg.clone(), version.clone()))
            .ok_or_else(|| PkgError::Unresolvable("no such version".into()))?;
        for (p, b) in files {
            staging.write(p, b)?;
        }
        Ok(())
    }
}

/// An in-memory proxy transport: URL -> body; anything else is not found.
#[derive(Default)]
pub(super) struct MapTransport(pub BTreeMap<String, Vec<u8>>);

impl ProxyTransport for MapTransport {
    fn get(&mut self, url: &str) -> Result<Vec<u8>, PkgError> {
        self.0
            .get(url)
            .cloned()
            .ok_or_else(|| PkgError::Unresolvable(format!("`{url}`: not found")))
    }
}

/// The proxy layout for one package version: `(url path, body)`.
pub(super) fn proxy_entries(
    path: &str,
    version: &str,
    files: &[(&str, &[u8])],
) -> Vec<(String, Vec<u8>)> {
    let manifest = files
        .iter()
        .find(|(p, _)| *p == MANIFEST_FILE)
        .map_or_else(Vec::new, |(_, b)| b.to_vec());
    vec![
        (format!("/{path}/@v/{version}.toml"), manifest),
        (format!("/{path}/@v/{version}.zip"), zip_of(files)),
    ]
}
