//! Packages through the session (design 5.7, 14.5.7; TASK-009 criterion
//! 2): a fixture package in a temp `DirStore`, `pkg::get_all` into a temp
//! `FsCache` and a lock, then the session loading from that lock and
//! cache. The proxy-shaped store is covered by SS-PKG's local HTTP fixture
//! test `pkg::tests::proxy::proxy_store_over_the_local_http_fixture`.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::support::{Rig, DOC};
use crate::dsp::caps::CapabilitySet;
use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::host::testing::AudioCall;
use crate::host::wire::HostMsg;
use crate::ns::pkg::PackageId;
use crate::pkg::cache::get_all;
use crate::pkg::digest::sources_digest;
use crate::pkg::load::locked_sources;
use crate::pkg::lock::{LockEntry, LockFile};
use crate::pkg::manifest::parse;
use crate::pkg::native::{DirStore, FsCache};
use crate::pkg::semver::Version;
use crate::pkg::store::PkgError;
use crate::reader::span::FileId;
use crate::reader::{prescan_imports, AliasEnv};
use crate::session::eval::{alias_env_for, analyze, PackageView};
use crate::session::repl::run_repl;
use crate::session::session::SessionConfig;
use crate::types::diag::DiagCode;
use crate::value::intern::{intern_kw, intern_sym, KwId};
use crate::value::value::PathVal;
use crate::vm::fail::{FailCode, Failure};

const PADS: &str = "github.com/someone/vactr-pads";
const DRUMS: &str = "github.com/someone/vactr-drums";
const CLASH: &str = "github.com/someone/vactr-clash";
const BD: &str = "github.com/someone/vactr-bd";
const BROKEN: &str = "github.com/someone/vactr-broken";

/// A unique directory under the system temp dir, removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> TempDir {
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "vactr-session-test-{}-{n}-{:x}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        fs::create_dir_all(&dir).expect("temp dir");
        TempDir(dir)
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn v(tag: &str) -> Version {
    Version::parse_tag(tag).expect("version")
}

fn id(path: &str) -> PackageId {
    PackageId::new(path)
}

/// Writes the store package `<root>/<path>@<version>/`.
fn package(root: &Path, path: &str, version: &str, files: &[(&str, &str)]) {
    let dir = root.join(format!("{path}@{version}"));
    for (rel, text) in files {
        let p = dir.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(&p, text).expect("write");
    }
}

fn manifest(path: &str, assets: bool, deps: &[(&str, &str)]) -> String {
    let mut s = format!("[package]\npath = \"{path}\"\n");
    if assets {
        s.push_str("assets = [\"samples\"]\n");
    }
    s.push_str("\n[deps]\n");
    for (d, ver) in deps {
        s.push_str(&format!("\"{d}\" = \"{ver}\"\n"));
    }
    s
}

/// The fixture store: pads v1.0.0 and v1.1.0 (with the `warm` bank), drums
/// v1.0.0 (requiring pads v1.1.0), a package exposing a prelude name, one
/// whose bank collides with a builtin sound, and one that does not compile.
fn store(tmp: &TempDir) -> PathBuf {
    let root = tmp.join("store");
    for (ver, level) in [("v1.0.0", "1"), ("v1.1.0", "2")] {
        package(
            &root,
            PADS,
            ver,
            &[
                ("vactr.toml", &manifest(PADS, true, &[])),
                (
                    "pads.vact",
                    &format!("let warm s :pads-warm\nlet lvl {level}\n"),
                ),
                ("samples/warm/a.wav", "RIFF"),
            ],
        );
    }
    package(
        &root,
        DRUMS,
        "v1.0.0",
        &[
            ("vactr.toml", &manifest(DRUMS, false, &[(PADS, "v1.1.0")])),
            ("drums.vact", "let kick s :analog\n"),
        ],
    );
    package(
        &root,
        CLASH,
        "v1.0.0",
        &[
            ("vactr.toml", &manifest(CLASH, false, &[])),
            ("clash.vact", "let fast 3\nlet mine 4\n"),
        ],
    );
    package(
        &root,
        BD,
        "v1.0.0",
        &[
            ("vactr.toml", &manifest(BD, true, &[])),
            ("bd.vact", "let one 1\n"),
            ("samples/haus/a.wav", "RIFF"),
        ],
    );
    package(
        &root,
        BROKEN,
        "v1.0.0",
        &[
            ("vactr.toml", &manifest(BROKEN, false, &[])),
            ("broken.vact", "let x / 1 0\nlet y 2\n"),
        ],
    );
    root
}

/// `get` of `deps` into the cache: the lock.
fn get(tmp: &TempDir, deps: &[(&str, &str)]) -> LockFile {
    let mut s = DirStore::new(&store(tmp));
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let mut text = String::from("[deps]\n");
    for (d, ver) in deps {
        text.push_str(&format!("\"{d}\" = \"{ver}\"\n"));
    }
    get_all(&parse(&text).expect("root manifest"), &mut s, &mut cache).expect("get")
}

/// A sample loader with banks: every registered bank loads a short buffer.
#[derive(Clone, Default)]
struct Banks(Rc<RefCell<BTreeMap<KwId, Vec<PathVal>>>>);

impl SampleLoader for Banks {
    fn load(&mut self, src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        match src {
            SampleSrc::Bank { kw, .. } if self.0.borrow().contains_key(kw) => {
                Ok(Arc::new(SampleData {
                    rate: 48_000,
                    channels: 1,
                    frames: vec![0.25; 8].into_boxed_slice(),
                }))
            }
            _ => Err(Failure::new(FailCode::HostUnavailable, "no such bank")),
        }
    }

    fn register_bank(&mut self, kw: KwId, files: Vec<PathVal>) -> Result<(), Failure> {
        let mut b = self.0.borrow_mut();
        if b.contains_key(&kw) || files.is_empty() {
            return Err(Failure::new(FailCode::LoadFailed, "bank clash"));
        }
        b.insert(kw, files);
        Ok(())
    }
}

/// A session over the lock and the cache at `tmp/cache`.
fn session(tmp: &TempDir, lock: LockFile) -> (Rig, Banks) {
    let mut cfg = SessionConfig::new(CapabilitySet::native());
    cfg.lock = Some(lock);
    cfg.cache = Some(Box::new(FsCache::new(&tmp.join("cache")).expect("cache")));
    let banks = Banks::default();
    (Rig::with_samples(cfg, Box::new(banks.clone())), banks)
}

fn codes(r: &crate::session::protocol::EvalResultBody) -> Vec<String> {
    r.diagnostics.iter().map(|d| d.code.clone()).collect()
}

fn last_value(r: &crate::session::protocol::EvalResultBody) -> Option<String> {
    r.forms.last().and_then(|f| f.value.clone())
}

#[test]
fn mvs_and_the_lock_pin_versions_and_digests_the_session_loads() {
    let tmp = TempDir::new();
    let lock = get(&tmp, &[(PADS, "v1.0.0"), (DRUMS, "v1.0.0")]);
    // Competing requirements: the maximum of the minimums.
    let pads = lock.get(&id(PADS)).expect("pads locked");
    assert_eq!(pads.version, v("v1.1.0"));
    let cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let src = locked_sources(&lock, &cache, &id(PADS)).expect("verified");
    assert_eq!(
        pads.sha256,
        sources_digest(&src.files),
        "the lock pins the digest"
    );
    let (mut rig, _) = session(&tmp, lock);
    let (r, _) = rig.eval(&format!("import {PADS}\npads.lvl\n"), 1, 0);
    assert!(codes(&r).is_empty(), "{:?}", r.diagnostics);
    assert_eq!(last_value(&r).as_deref(), Some("2"), "v1.1.0 is loaded");
    // `as` and `open` follow 5.7 lookup.
    let (r, _) = rig.eval(&format!("import {PADS} as pd\npd.lvl\n"), 2, 0);
    assert_eq!(last_value(&r).as_deref(), Some("2"), "{:?}", r.diagnostics);
    let (r, _) = rig.eval(&format!("import {DRUMS} open\nkick\n"), 3, 0);
    assert_eq!(
        last_value(&r).as_deref(),
        Some("<pattern>"),
        "{:?}",
        r.diagnostics
    );
}

#[test]
fn collisions_are_import_collision_warnings() {
    let tmp = TempDir::new();
    let lock = get(&tmp, &[(CLASH, "v1.0.0"), (BD, "v1.0.0")]);
    let (mut rig, _) = session(&tmp, lock);
    let (r, _) = rig.eval(&format!("import {CLASH} open\nmine\n"), 1, 0);
    let clash = r
        .diagnostics
        .iter()
        .find(|d| d.code == "import-collision")
        .expect("an opened name collides with the prelude `fast`");
    assert_eq!(clash.severity, "warning");
    assert_eq!(last_value(&r).as_deref(), Some("4"));
    // `vactr-bd`'s bank `haus` is `:bd-haus`, a builtin sound.
    let (r, _) = rig.eval(&format!("import {BD}\nbd.one\n"), 2, 0);
    let d = r
        .diagnostics
        .iter()
        .find(|d| d.code == "import-collision")
        .expect("a bank collision");
    assert!(d.message.contains("bd-haus"), "{}", d.message);
    assert_eq!(last_value(&r).as_deref(), Some("1"));
}

#[test]
fn the_asset_bank_plays_through_run_repl() {
    let tmp = TempDir::new();
    let lock = get(&tmp, &[(PADS, "v1.0.0")]);
    let (mut rig, banks) = session(&tmp, lock);
    let clock = rig.clock.clone();
    let mut out = Vec::new();
    let input = format!("import {PADS}\ns :pads-warm > d1\n");
    let mut tick = || {
        clock.advance(0.01);
        clock.now()
    };
    run_repl(&mut rig.s, input.as_bytes(), &mut out, &mut tick).expect("repl");
    let transcript = String::from_utf8(out).expect("utf8");
    assert!(!transcript.contains("error["), "{transcript}");
    assert!(banks.0.borrow().contains_key(&intern_kw("pads-warm")));
    // The bank's sample reached the recording host...
    let installed: Vec<u32> = rig
        .audio
        .calls()
        .into_iter()
        .filter_map(|(_, c)| match c {
            AudioCall::InstallSample(id, _) => Some(id),
            _ => None,
        })
        .collect();
    assert_eq!(installed.len(), 1, "{transcript}");
    // ...and once installed, its events play.
    rig.audio.reply(HostMsg::Installed {
        resource: installed[0],
        gen: 0,
    });
    let now = rig.clock.now();
    rig.run_to(now + 2.0);
    assert!(!rig.sent().is_empty(), "the bank plays");
}

#[test]
fn a_fresh_session_loads_an_import_and_pads_warm_end_to_end() {
    for (code, prefix) in [
        (format!("import {PADS}\npads.warm > d1\n"), "pads"),
        (format!("import {PADS} as pd\npd.warm > d1\n"), "pd"),
    ] {
        let tmp = TempDir::new();
        let lock = get(&tmp, &[(PADS, "v1.0.0")]);
        let (mut rig, _) = session(&tmp, lock);
        let (r, _) = rig.eval(&code, 1, 0);
        assert!(
            r.diagnostics.iter().all(|d| d.severity != "error"),
            "{prefix}: {:?}",
            r.diagnostics
        );
        assert!(
            r.forms.iter().all(|f| f.failure.is_none()),
            "{prefix}: {r:?}"
        );
        assert!(rig
            .s
            .runtime()
            .slot_gen(crate::ns::stage::SlotKey::D(1))
            .is_some());
        rig.run_to(0.5);
        assert!(
            rig.audio
                .calls()
                .iter()
                .any(|(_, c)| matches!(c, AudioCall::InstallSample(..))),
            "{prefix}: the bank sample is requested"
        );
    }
}

#[test]
fn lsp_only_analyze_builds_the_same_alias_env_without_executing() {
    let tmp = TempDir::new();
    let lock = get(&tmp, &[(PADS, "v1.0.0")]);
    let cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let code = format!("import {PADS}\npads.warm > d1\nlet x pads.nope\n");
    let view = PackageView {
        lock: Some(&lock),
        cache: Some(&cache),
    };
    let a = analyze(&code, FileId::new(3), &view);
    let expect = alias_env_for(&AliasEnv::new(), &prescan_imports(&code));
    assert_eq!(a.alias_env, expect);
    let undefined: Vec<_> = a
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::UndefinedName)
        .collect();
    assert_eq!(undefined.len(), 1, "{:?}", a.diags);
    assert!(undefined[0].message.contains("nope"));
    assert!(!a.types.is_empty());
    // The same alias environment as a session that evaluated the document.
    let (mut rig, _) = session(&tmp, lock.clone());
    rig.eval(&code, 1, 0);
    assert_eq!(rig.s.session_aliases(), a.alias_env);
    // An unfetched package: `any` plus `package-not-fetched`, and nothing ran.
    let empty = FsCache::new(&tmp.join("empty-cache")).expect("cache");
    let view = PackageView {
        lock: Some(&lock),
        cache: Some(&empty),
    };
    let a = analyze(&code, FileId::new(3), &view);
    let d = a
        .diags
        .iter()
        .find(|d| d.code == DiagCode::PackageNotFetched)
        .expect("package-not-fetched");
    assert_eq!(d.severity, crate::types::diag::Severity::Warning);
    assert!(
        a.diags.iter().all(|d| d.code != DiagCode::UndefinedName),
        "{:?}",
        a.diags
    );
    assert!(
        !empty_dir_has_entries(&tmp.join("empty-cache")),
        "analysis never fetches"
    );
}

fn empty_dir_has_entries(p: &Path) -> bool {
    fs::read_dir(p)
        .map(|rd| {
            rd.flatten()
                .any(|e| !e.file_name().to_string_lossy().starts_with('.'))
        })
        .unwrap_or(false)
}

/// Plays `s :analog > note [60] > d1`, then evaluates `code`; returns the
/// diagnostics codes and whether d1 kept producing events.
fn while_playing(rig: &mut Rig, code: &str) -> Vec<String> {
    rig.ok("s :analog > note [60 64] > d1\n", 1);
    rig.run_to(1.0);
    let before = rig.sent().len();
    let (r, _) = rig.eval(code, 2, 0);
    let now = rig.clock.now();
    rig.run_to(now + 2.0);
    assert!(
        rig.sent().len() > before,
        "the playing slot keeps producing events"
    );
    codes(&r)
}

#[test]
fn load_failures_are_diagnostics_at_the_import_while_a_slot_plays() {
    let tmp = TempDir::new();
    let lock = get(&tmp, &[(PADS, "v1.0.0"), (BROKEN, "v1.0.0")]);
    // Hash mismatch.
    let mut bad = lock.clone();
    let mut entries: Vec<LockEntry> = bad.entries.clone();
    for e in &mut entries {
        if e.id == id(PADS) {
            e.sha256[0] ^= 0xff;
        }
    }
    bad = LockFile::new(entries);
    let (mut rig, _) = session(&tmp, bad);
    let code = format!("import {PADS}\npads.lvl\n");
    let c = while_playing(&mut rig, &code);
    assert!(c.contains(&"package-integrity".to_string()), "{c:?}");
    assert!(
        c.contains(&"undefined-name".to_string()),
        "bound but broken: {c:?}"
    );
    // A version locked but never fetched, and an import not in the lock.
    let mut entries = lock.entries.clone();
    for e in &mut entries {
        if e.id == id(PADS) {
            e.version = v("v9.9.9");
        }
    }
    let (mut rig, _) = session(&tmp, LockFile::new(entries));
    let c = while_playing(&mut rig, &code);
    assert!(c.contains(&"package-not-fetched".to_string()), "{c:?}");
    let (mut rig, _) = session(&tmp, lock.clone());
    let c = while_playing(&mut rig, &format!("import {DRUMS}\ndrums.kick\n"));
    assert!(c.contains(&"package-not-locked".to_string()), "{c:?}");
    // An unresolvable version fails `vactr get` with `package-resolve`.
    let mut s = DirStore::new(&store(&tmp));
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let e = get_all(
        &parse(&format!("[deps]\n\"{PADS}\" = \"v7.0.0\"\n")).expect("root"),
        &mut s,
        &mut cache,
    )
    .expect_err("unresolvable");
    assert!(matches!(e, PkgError::Unresolvable(_)));
    assert_eq!(e.code(), DiagCode::PackageResolve);
    // A package compile error.
    let (mut rig, _) = session(&tmp, lock);
    let code = format!("import {BROKEN}\nbroken.y\n");
    let c = while_playing(&mut rig, &code);
    assert!(c.contains(&"package-load-failed".to_string()), "{c:?}");
    let (r, _) = rig.eval(&code, 3, 0);
    let at = r
        .diagnostics
        .iter()
        .find(|d| d.code == "package-load-failed")
        .expect("load failed");
    assert_eq!(
        (at.file.as_str(), at.span.start),
        (DOC, 0),
        "at the import span"
    );
}

#[test]
fn re_import_replaces_the_pkg_ns() {
    let tmp = TempDir::new();
    let lock = get(&tmp, &[(PADS, "v1.0.0")]);
    let (mut rig, _) = session(&tmp, lock);
    let code = format!("import {PADS}\npads.lvl\n");
    rig.eval(&code, 1, 0);
    let first = rig
        .s
        .evaluator()
        .ns()
        .package(intern_sym("pads"))
        .expect("bound");
    rig.eval(&code, 2, 0);
    let second = rig
        .s
        .evaluator()
        .ns()
        .package(intern_sym("pads"))
        .expect("bound");
    assert!(!Rc::ptr_eq(&first, &second), "a fresh PkgNs");
    assert_eq!(first.id, second.id);
}
