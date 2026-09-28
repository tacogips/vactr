//! The session side of loading: default prefixes, asset banks, and a bank
//! registered with `NativeSampleLoader` from a verified cache entry.

use std::rc::Rc;

use crate::host::caps::{SampleLoader, SampleSrc};
use crate::host::native::loader::NativeSampleLoader;
use crate::pkg::cache::get_all;
use crate::pkg::load::{asset_banks, default_prefix, locked_sources};
use crate::pkg::manifest::parse;
use crate::pkg::native::{DirStore, FsCache};
use crate::pkg::store::PkgSources;
use crate::pkg::tests::support::{
    as_refs, deps_manifest, dir_package, id, pads_files, v, wav, TempDir, PADS,
};
use crate::value::intern::intern_kw;
use crate::vm::fail::FailCode;

#[test]
fn default_prefixes() {
    assert_eq!(&*default_prefix(&id(PADS)), "pads");
    assert_eq!(&*default_prefix(&id("github.com/a/drums")), "drums");
    assert_eq!(&*default_prefix(&id("github.com/a/vactr-")), "");
}

#[test]
fn asset_banks_are_immediate_subdirectories_of_assets() {
    let mut files: Vec<(Rc<str>, Rc<[u8]>)> = pads_files()
        .into_iter()
        .map(|(p, b)| (Rc::from(p.as_str()), Rc::from(b)))
        .collect();
    for p in [
        "samples/cold/z.WAV",
        "samples/cold/n.txt",
        "samples/cold/sub/y.wav",
        "other/x/a.wav",
    ] {
        files.push((Rc::from(p), Rc::from(wav(4))));
    }
    let src = PkgSources::from_files(id(PADS), v("v1.0.0"), files.clone(), None).expect("sources");
    let banks = asset_banks(&src, "pads");
    let got: Vec<(&str, Vec<&str>)> = banks
        .iter()
        .map(|(k, fs)| (&**k, fs.iter().map(|f| &*f.text).collect()))
        .collect();
    assert_eq!(
        got,
        [
            ("pads-cold", vec!["samples/cold/z.WAV"]),
            (
                "pads-warm",
                vec!["samples/warm/a.wav", "samples/warm/b.wav"]
            ),
        ]
    );
    let rooted =
        PkgSources::from_files(id(PADS), v("v1.0.0"), files, Some(Rc::from("/c"))).expect("ok");
    assert_eq!(
        &*asset_banks(&rooted, "pads")[1].1[0].text,
        "/c/samples/warm/a.wav"
    );
}

#[test]
fn a_registered_bank_loads_through_the_native_sample_loader() {
    let tmp = TempDir::new();
    dir_package(&tmp.join("store"), PADS, "v1.0.0", &as_refs(&pads_files()));
    let mut store = DirStore::new(&tmp.join("store"));
    let mut cache = FsCache::new(&tmp.join("cache")).expect("cache");
    let root = parse(&deps_manifest(None, &[(PADS, "v1.0.0")])).expect("root");
    let lock = get_all(&root, &mut store, &mut cache).expect("get");
    let src = locked_sources(&lock, &cache, &id(PADS)).expect("verified");
    let banks = asset_banks(&src, &default_prefix(&id(PADS)));
    assert_eq!(banks.len(), 1);
    let (name, files) = banks.into_iter().next().expect("one bank");
    assert_eq!(&*name, "pads-warm");

    let roots = tmp.join("roots");
    std::fs::create_dir_all(roots.join("pads-cold")).expect("mkdir");
    let mut loader = NativeSampleLoader::new(&[roots], tmp.path());
    let kw = intern_kw(&name);
    loader.register_bank(kw, files.clone()).expect("registered");
    let frames = |l: &mut NativeSampleLoader, index| {
        l.load(&SampleSrc::Bank { kw, index })
            .expect("bank sample")
            .frames
            .len()
    };
    assert_eq!(frames(&mut loader, 0), 8);
    assert_eq!(frames(&mut loader, 1), 16);
    assert_eq!(frames(&mut loader, 2), 8);
    // The first registration wins; a clash is refused.
    let e = loader.register_bank(kw, files.clone()).expect_err("clash");
    assert_eq!(e.code, FailCode::LoadFailed);
    let e = loader
        .register_bank(intern_kw("pads-cold"), files)
        .expect_err("root clash");
    assert_eq!(e.code, FailCode::LoadFailed);
    assert!(loader
        .register_bank(intern_kw("pads-empty"), Vec::new())
        .is_err());
}
