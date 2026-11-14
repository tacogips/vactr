//! Isolated assets are prepared without active aliases and closed before queries.
use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;
use vactr::host::caps::{SampleData, SampleLoader, SampleSrc};
use vactr::ns::load::SourceLoader;
use vactr::reader::span::FileId;
use vactr::song::assets::*;
use vactr::value::intern::{intern_kw, intern_sym, KwId};
use vactr::value::sample::SampleBuf;
use vactr::value::value::{PathVal, Sound, Value};
use vactr::vm::fail::{FailCode, Failure};

fn limits() -> SongAssetLimits {
    SongAssetLimits {
        max_resources: 8,
        max_pcm_bytes: 4096,
        max_source_files: 8,
        max_source_bytes: 4096,
        max_banks: 4,
        max_walk_nodes: 1024,
        max_walk_depth: 64,
    }
}
fn path(text: &str) -> PathVal {
    PathVal {
        text: Rc::from(text),
        file: Some(FileId::new(7)),
    }
}
fn source() -> SongSourceFile {
    SongSourceFile {
        file: FileId::new(7),
        path: path("score.vact"),
    }
}
fn pcm(value: f32) -> Arc<SampleData> {
    Arc::new(SampleData {
        rate: 48000,
        channels: 2,
        frames: vec![value; 8].into_boxed_slice(),
    })
}
fn page(
    samples: BTreeMap<String, Arc<SampleData>>,
    banks: BTreeMap<KwId, Vec<String>>,
) -> DecodedSongAssetFactory {
    DecodedSongAssetFactory::new(
        samples,
        banks,
        BTreeMap::from([("lib.vact".into(), (FileId::new(9), Rc::from("let x = 1")))]),
    )
}

#[test]
fn detached_page_pcm_survives_active_mutation_and_exact_bank_lookup() {
    let bank = intern_kw("kit");
    let mut active = BTreeMap::from([("kick".into(), pcm(0.25)), ("snare".into(), pcm(0.5))]);
    let factory = page(
        active.clone(),
        BTreeMap::from([(bank, vec!["kick".into(), "snare".into()])]),
    );
    active.insert("kick".into(), pcm(0.75));
    let mut preparation = factory.begin(source(), limits()).unwrap();
    preparation.pin(&[SongAssetSelector::Bank(bank)]).unwrap();
    let mut closed = preparation.close().unwrap();
    assert_eq!(
        closed
            .load(&SampleSrc::Bank { kw: bank, index: 0 })
            .unwrap()
            .frames[0],
        0.25
    );
    assert_eq!(
        closed
            .load(&SampleSrc::Bank { kw: bank, index: 1 })
            .unwrap()
            .frames[0],
        0.5
    );
    assert_eq!(
        closed
            .load(&SampleSrc::Bank { kw: bank, index: 2 })
            .err()
            .unwrap()
            .code,
        FailCode::HostUnavailable
    );
    assert_eq!(closed.resource_count(), 2);
}

#[test]
fn page_catalog_requires_complete_declared_pcm_and_closed_paths_do_not_fallback() {
    let bank = intern_kw("kit");
    let samples = BTreeMap::from([("kick".into(), pcm(0.25))]);
    let no_catalog = page(samples.clone(), BTreeMap::new());
    let mut prep = no_catalog.begin(source(), limits()).unwrap();
    assert_eq!(
        prep.pin(&[SongAssetSelector::Bank(bank)])
            .err()
            .unwrap()
            .code,
        FailCode::HostUnavailable
    );
    let incomplete = page(
        samples,
        BTreeMap::from([(bank, vec!["kick".into(), "missing".into()])]),
    );
    let mut prep = incomplete.begin(source(), limits()).unwrap();
    assert_eq!(
        prep.pin(&[SongAssetSelector::Bank(bank)])
            .err()
            .unwrap()
            .code,
        FailCode::HostUnavailable
    );
    let mut closed = prep.close().unwrap();
    assert!(closed.load(&SampleSrc::Path(path("kick"))).is_err());
}

#[test]
fn private_ready_buffer_has_stable_rate_pcm_and_consistent_replacement() {
    let factory = page(BTreeMap::new(), BTreeMap::new());
    let mut prep = factory.begin(source(), limits()).unwrap();
    let original = SampleBuf::ready(44100, vec![0.125; 8]);
    let copy = prep.pin_buffer(&original).unwrap();
    assert!(!Rc::ptr_eq(&copy, &original));
    assert!(Rc::ptr_eq(&copy, &prep.pin_buffer(&original).unwrap()));
    original.fill_at(96000, vec![0.75; 8]);
    let mut closed = prep.close().unwrap();
    assert_eq!(copy.rate(), 44100);
    assert_eq!(copy.ready_frames().unwrap()[0], 0.125);
    assert!(Rc::ptr_eq(&copy, &closed.buffer_copy(original.id).unwrap()));
    let loaded = closed.load(&SampleSrc::Buffer { id: copy.id }).unwrap();
    assert_eq!((loaded.rate, loaded.frames[0]), (44100, 0.125));
    assert!(closed.load(&SampleSrc::Buffer { id: original.id }).is_err());
}

#[test]
fn pending_failed_misaligned_nonfinite_and_zero_rate_buffers_are_rejected() {
    let factory = page(BTreeMap::new(), BTreeMap::new());
    let mut prep = factory.begin(source(), limits()).unwrap();
    assert_eq!(
        prep.pin_buffer(&SampleBuf::pending(48000))
            .err()
            .unwrap()
            .code,
        FailCode::CapturePending
    );
    let failed = SampleBuf::pending(48000);
    failed.fail(FailCode::Type, "failed");
    assert_eq!(prep.pin_buffer(&failed).err().unwrap().code, FailCode::Type);
    for b in [
        SampleBuf::ready(0, vec![0.; 2]),
        SampleBuf::ready(48000, vec![0.; 3]),
        SampleBuf::ready(48000, vec![f32::NAN; 2]),
    ] {
        assert!(prep.pin_buffer(&b).is_err());
    }
    assert_eq!(prep.close().unwrap().resource_count(), 0);
}

#[test]
fn close_revokes_all_preparation_handles_without_analysis_or_registration() {
    let factory = page(
        BTreeMap::from([("x.wav".into(), pcm(0.1))]),
        BTreeMap::new(),
    );
    let mut prep = factory.begin(source(), limits()).unwrap();
    let mut loader = prep.source_loader();
    let mut samples = prep.sample_loader();
    assert!(loader.analysis().is_none());
    let first = loader.read(&path("lib.vact")).unwrap();
    assert_eq!(first.0, loader.read(&path("lib.vact")).unwrap().0);
    prep.pin(&[SongAssetSelector::Path(path("x.wav"))]).unwrap();
    let mut closed = prep.close().unwrap();
    assert_eq!(closed.source_count(), 1);
    assert!(loader.read(&path("lib.vact")).is_err());
    assert!(samples
        .register_bank(intern_kw("new"), vec![path("x.wav")])
        .is_err());
    assert!(samples.load(&SampleSrc::Path(path("x.wav"))).is_err());
    assert_eq!(
        closed.load(&SampleSrc::Path(path("x.wav"))).unwrap().frames[0],
        0.1
    );
    assert!(closed.register_bank(intern_kw("new"), vec![]).is_err());
}

struct CountingBackend(Rc<Cell<u32>>);
impl SourceLoader for CountingBackend {
    fn read(&mut self, _: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        self.0.set(self.0.get() + 1);
        Ok((FileId::new(8), Rc::from("0123456789")))
    }
}
impl SampleLoader for CountingBackend {
    fn load(&mut self, _: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        self.0.set(self.0.get() + 1);
        Ok(pcm(0.1))
    }
}
impl SongAssetBackend for CountingBackend {
    fn bank_len(&mut self, _: KwId, _: u32) -> Result<u32, Failure> {
        Ok(2)
    }
}
#[test]
fn zero_resource_and_source_capacity_admit_before_backend_io() {
    let count = Rc::new(Cell::new(0));
    let mut bound = limits();
    bound.max_resources = 0;
    bound.max_source_files = 0;
    let mut prep =
        SongAssetPreparation::new(Box::new(CountingBackend(count.clone())), bound).unwrap();
    assert_eq!(
        prep.pin(&[SongAssetSelector::Path(path("x.wav"))])
            .err()
            .unwrap()
            .code,
        FailCode::FuelExhausted
    );
    assert_eq!(
        prep.source_loader()
            .read(&path("lib.vact"))
            .err()
            .unwrap()
            .code,
        FailCode::FuelExhausted
    );
    assert_eq!(count.get(), 0);
}

#[test]
fn remaining_pcm_source_bank_and_resource_capacity_are_checked() {
    let factory = page(
        BTreeMap::from([("a".into(), pcm(0.1)), ("b".into(), pcm(0.2))]),
        BTreeMap::from([(intern_kw("kit"), vec!["a".into(), "b".into()])]),
    );
    for bound in [
        {
            let mut x = limits();
            x.max_resources = 1;
            x
        },
        {
            let mut x = limits();
            x.max_pcm_bytes = 31;
            x
        },
        {
            let mut x = limits();
            x.max_banks = 0;
            x
        },
    ] {
        let mut prep = factory.begin(source(), bound).unwrap();
        assert_eq!(
            prep.pin(&[SongAssetSelector::Bank(intern_kw("kit"))])
                .err()
                .unwrap()
                .code,
            FailCode::FuelExhausted
        );
    }
    let mut bound = limits();
    bound.max_source_bytes = 1;
    let prep = factory.begin(source(), bound).unwrap();
    assert_eq!(
        prep.source_loader()
            .read(&path("lib.vact"))
            .err()
            .unwrap()
            .code,
        FailCode::FuelExhausted
    );
}

#[test]
fn invalid_decoded_pcm_is_never_certified() {
    for data in [
        SampleData {
            rate: 0,
            channels: 2,
            frames: vec![0.; 2].into(),
        },
        SampleData {
            rate: 48000,
            channels: 3,
            frames: vec![0.; 3].into(),
        },
        SampleData {
            rate: 48000,
            channels: 2,
            frames: vec![0.; 3].into(),
        },
        SampleData {
            rate: 48000,
            channels: 1,
            frames: vec![f32::INFINITY].into(),
        },
    ] {
        let factory = page(
            BTreeMap::from([("x".into(), Arc::new(data))]),
            BTreeMap::new(),
        );
        let mut prep = factory.begin(source(), limits()).unwrap();
        assert!(prep.pin(&[SongAssetSelector::Path(path("x"))]).is_err());
        assert_eq!(prep.close().unwrap().resource_count(), 0);
    }
}

#[test]
fn dependency_walk_separates_literals_samples_keywords_and_buffers() {
    use vactr::pattern::pat::{PParam, Pat, PatNode};
    let sound = Value::Pattern(Rc::new(Pat::new(
        PatNode::Sound {
            src: PParam::Const(Value::list(vec![
                Value::Path(Rc::new(path("x.wav"))),
                Value::kw("bd"),
            ])),
            kit: None,
        },
        None,
        false,
    )));
    let buffer = SampleBuf::ready(48000, vec![0.; 2]);
    let deps = song_asset_dependencies(
        &[
            Value::Path(Rc::new(path("lib.vact"))),
            sound,
            Value::Sound(Rc::new(Sound::Buffer(buffer.clone()))),
        ],
        limits(),
    )
    .unwrap();
    assert_eq!(deps.literal_paths[0].text.as_ref(), "lib.vact");
    assert_eq!(deps.sample_paths[0].text.as_ref(), "x.wav");
    assert_eq!(deps.sound_keywords, vec![intern_kw("bd")]);
    assert!(Rc::ptr_eq(&deps.buffers[0], &buffer));
}

#[test]
fn dependency_walk_shared_nodes_cycles_and_depth_are_bounded() {
    use vactr::ns::namespace::{SlotKind, VarSlotRef};
    let slot = VarSlotRef::new(intern_sym("cycle"), SlotKind::Var, Value::Nil);
    *slot.0.value.borrow_mut() = Value::list(vec![
        Value::VarRef(slot.clone()),
        Value::path("x.wav", None),
    ]);
    let deps = song_asset_dependencies(
        &[Value::VarRef(slot.clone()), Value::VarRef(slot.clone())],
        limits(),
    )
    .unwrap();
    assert_eq!(deps.literal_paths.len(), 1);
    *slot.0.value.borrow_mut() = Value::Nil;
    let mut value = Value::Nil;
    for _ in 0..70 {
        value = Value::list(vec![value]);
    }
    assert_eq!(
        song_asset_dependencies(&[value], limits())
            .err()
            .unwrap()
            .code,
        FailCode::DepthExceeded
    );
    let mut bound = limits();
    bound.max_walk_nodes = 2;
    assert_eq!(
        song_asset_dependencies(&[Value::list(vec![Value::Nil; 10])], bound)
            .err()
            .unwrap()
            .code,
        FailCode::FuelExhausted
    );
    for bad in [
        {
            let mut x = limits();
            x.max_walk_depth = 257;
            x
        },
        {
            let mut x = limits();
            x.max_walk_nodes = 1_000_001;
            x
        },
    ] {
        assert!(bad.validate().is_err());
    }
}

#[test]
fn dependency_walk_reports_live_signals_and_external_sounds_without_evaluation() {
    use vactr::pattern::signal::Sig;
    let live = Rc::new(Sig::Lag(
        Rc::new(Sig::MapRange(
            Rc::new(Sig::Cc {
                controller: 1,
                channel: 0,
            }),
            0.,
            1.,
        )),
        0.1,
    ));
    let deps = song_asset_dependencies(
        &[
            Value::Signal(live),
            Value::Sound(Rc::new(Sound::MidiOut(1))),
            Value::Sound(Rc::new(Sound::Osc(Rc::from("/x")))),
        ],
        limits(),
    )
    .unwrap();
    assert!(deps.has_live_signals && deps.has_external_sounds);
    assert!(
        !song_asset_dependencies(&[Value::Signal(Rc::new(Sig::Time))], limits())
            .unwrap()
            .has_live_signals
    );
}

#[cfg(feature = "host-native")]
mod native {
    use super::*;
    use std::path::PathBuf;
    use vactr::host::native::loader::NativeSampleLoader;
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "vactr-song-assets-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn wav(value: i16) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(b"RIFF");
        b.extend(40u32.to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(48000u32.to_le_bytes());
        b.extend(96000u32.to_le_bytes());
        b.extend(2u16.to_le_bytes());
        b.extend(16u16.to_le_bytes());
        b.extend(b"data");
        b.extend(4u32.to_le_bytes());
        b.extend(value.to_le_bytes());
        b.extend(value.to_le_bytes());
        b
    }
    #[test]
    fn native_fresh_roots_source_ids_complete_bank_and_disk_pinning() {
        let directory = Directory::new();
        std::fs::write(directory.0.join("score.vact"), "score").unwrap();
        std::fs::write(directory.0.join("lib.vact"), "old").unwrap();
        std::fs::create_dir(directory.0.join("kit")).unwrap();
        std::fs::write(directory.0.join("kit/a.wav"), wav(8192)).unwrap();
        std::fs::write(directory.0.join("kit/b.wav"), wav(16384)).unwrap();
        let loader = NativeSampleLoader::new(&[directory.0.clone()], &directory.0);
        let factory = loader.isolated_song_factory();
        let mut prep = factory.begin(source(), limits()).unwrap();
        let mut source_loader = prep.source_loader();
        let first = source_loader.read(&path("lib.vact")).unwrap();
        std::fs::write(directory.0.join("lib.vact"), "new").unwrap();
        assert_eq!(&*source_loader.read(&path("lib.vact")).unwrap().1, "old");
        assert_eq!(first.0, source_loader.read(&path("lib.vact")).unwrap().0);
        let alias = source_loader.read(&path("./lib.vact")).unwrap();
        assert_eq!(alias.0, first.0);
        assert_eq!(&*alias.1, "old");
        loader.register_file(FileId::new(7), std::path::Path::new("/outside/score.vact"));
        prep.pin(&[SongAssetSelector::Bank(intern_kw("kit"))])
            .unwrap();
        let mut closed = prep.close().unwrap();
        std::fs::write(directory.0.join("kit/a.wav"), wav(24576)).unwrap();
        assert_eq!(
            closed
                .load(&SampleSrc::Bank {
                    kw: intern_kw("kit"),
                    index: 2
                })
                .unwrap()
                .frames[0],
            0.25
        );
        assert_eq!(
            closed
                .load(&SampleSrc::Bank {
                    kw: intern_kw("kit"),
                    index: 1
                })
                .unwrap()
                .frames[0],
            0.5
        );
        assert!(source_loader.read(&path("lib.vact")).is_err());
    }
    #[test]
    fn native_outside_roots_and_oversized_pcm_are_rejected() {
        let directory = Directory::new();
        let outside = Directory::new();
        std::fs::write(directory.0.join("score.vact"), "").unwrap();
        std::fs::write(outside.0.join("x.wav"), wav(1)).unwrap();
        std::fs::write(directory.0.join("x.wav"), wav(1)).unwrap();
        let loader = NativeSampleLoader::new(&[directory.0.clone()], &directory.0);
        let mut bound = limits();
        bound.max_pcm_bytes = 4;
        let mut prep = loader
            .isolated_song_factory()
            .begin(source(), bound)
            .unwrap();
        assert!(prep
            .pin(&[SongAssetSelector::Path(path(
                outside.0.join("x.wav").to_str().unwrap()
            ))])
            .is_err());
        assert!(prep.pin(&[SongAssetSelector::Path(path("x.wav"))]).is_err());
        assert_eq!(prep.close().unwrap().resource_count(), 0);
    }
}

#[test]
fn dependency_walk_traverses_closure_constants_captures_and_shared_global_cycles() {
    use vactr::host::noop::NoopHost;
    use vactr::ns::evaluator::Evaluator;
    use vactr::ns::namespace::Prelude;
    use vactr::ns::stage::RecordingSink;
    let mut evaluator = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    let outcomes = evaluator
        .eval_str(
            "let asset ./x.wav\nfn voice t:\n\tlet private ./nested.wav\n\ts asset",
            FileId::new(7),
        )
        .unwrap();
    for outcome in &outcomes {
        assert!(outcome.value.is_ok(), "{:?}", outcome.value);
    }
    let value = evaluator.ns().session_value("voice").unwrap();
    let deps = song_asset_dependencies(&[value], limits()).unwrap();
    assert!(deps
        .literal_paths
        .iter()
        .any(|p| p.text.as_ref() == "./nested.wav"));
    assert!(
        deps.literal_paths
            .iter()
            .any(|p| p.text.as_ref() == "./x.wav")
            || deps
                .sample_paths
                .iter()
                .any(|p| p.text.as_ref() == "./x.wav")
    );
}

#[test]
fn capacity_subtracts_active_and_retiring_generations_with_checked_overflow() {
    let remaining = limits().after_reservations(3, 2, 1024, 512).unwrap();
    assert_eq!(remaining.max_resources, 3);
    assert_eq!(remaining.max_pcm_bytes, 2560);
    assert_eq!(
        limits()
            .after_reservations(u32::MAX, 1, 0, 0)
            .err()
            .unwrap()
            .code,
        FailCode::Overflow
    );
    assert_eq!(
        limits()
            .after_reservations(0, 0, u64::MAX, 1)
            .err()
            .unwrap()
            .code,
        FailCode::Overflow
    );
    assert_eq!(
        limits().after_reservations(8, 1, 0, 0).err().unwrap().code,
        FailCode::FuelExhausted
    );
    let mut prep = page(BTreeMap::from([("x".into(), pcm(0.1))]), BTreeMap::new())
        .begin(source(), limits().after_reservations(7, 1, 0, 0).unwrap())
        .unwrap();
    assert_eq!(
        prep.pin(&[SongAssetSelector::Path(path("x"))])
            .err()
            .unwrap()
            .code,
        FailCode::FuelExhausted
    );
}

#[test]
fn default_loader_has_no_isolated_capability_and_session_factory_refresh_is_detached() {
    use vactr::dsp::caps::CapabilitySet;
    use vactr::host::caps::Hosts;
    use vactr::host::noop::NoopHost;
    use vactr::session::{Session, SessionConfig};
    assert!(NoopHost.song_asset_factory().is_none());
    let hosts = Hosts {
        audio: Box::new(NoopHost),
        midi: Box::new(NoopHost),
        osc: Box::new(NoopHost),
        render: Box::new(NoopHost),
        midi_in: Box::new(NoopHost),
        samples: Box::new(NoopHost),
    };
    let mut session = Session::new(SessionConfig::new(CapabilitySet::browser()), hosts);
    assert_eq!(
        session
            .begin_song_assets(source(), limits())
            .err()
            .unwrap()
            .code,
        FailCode::HostUnavailable
    );
    session.set_song_asset_factory(Some(Rc::new(page(
        BTreeMap::from([("x".into(), pcm(0.1))]),
        BTreeMap::new(),
    ))));
    let mut old = session.begin_song_assets(source(), limits()).unwrap();
    old.pin(&[SongAssetSelector::Path(path("x"))]).unwrap();
    session.set_song_asset_factory(Some(Rc::new(page(
        BTreeMap::from([("x".into(), pcm(0.9))]),
        BTreeMap::new(),
    ))));
    let mut new = session.begin_song_assets(source(), limits()).unwrap();
    new.pin(&[SongAssetSelector::Path(path("x"))]).unwrap();
    assert_eq!(
        old.close()
            .unwrap()
            .load(&SampleSrc::Path(path("x")))
            .unwrap()
            .frames[0],
        0.1
    );
    assert_eq!(
        new.close()
            .unwrap()
            .load(&SampleSrc::Path(path("x")))
            .unwrap()
            .frames[0],
        0.9
    );
}

#[cfg(feature = "host-native")]
#[test]
fn native_supplied_code_accepts_unsaved_origins_and_console_but_not_missing_parents() {
    use vactr::host::native::loader::NativeSampleLoader;
    let directory = std::env::temp_dir().join(format!("vactr-song-unsaved-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let loader = NativeSampleLoader::new(&[directory.clone()], &directory);
    let factory = loader.isolated_song_factory();
    let unsaved = SongSourceFile {
        file: FileId::new(7),
        path: path("unsaved.vact"),
    };
    let prep = factory.begin(unsaved, limits()).unwrap();
    assert!(prep.source_loader().read(&path("missing.vact")).is_err());
    let console = SongSourceFile {
        file: FileId::CONSOLE,
        path: PathVal {
            text: Rc::from("<console>"),
            file: None,
        },
    };
    assert!(factory.begin(console, limits()).is_ok());
    for text in [
        "missing-directory/unsaved.vact",
        "/outside-does-not-exist/unsaved.vact",
    ] {
        assert!(factory
            .begin(
                SongSourceFile {
                    file: FileId::new(7),
                    path: path(text)
                },
                limits()
            )
            .is_err());
    }
    let outside_origin = std::env::temp_dir().join("outside-unsaved.vact");
    assert!(factory
        .begin(
            SongSourceFile {
                file: FileId::new(7),
                path: path(outside_origin.to_str().unwrap())
            },
            limits()
        )
        .is_err());
    std::fs::write(directory.join("library.vact"), "inside").unwrap();
    assert_eq!(
        &*prep.source_loader().read(&path("library.vact")).unwrap().1,
        "inside"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn close_drops_unselected_backend_inventory_while_revoked_handles_survive() {
    struct Backend(Rc<Cell<bool>>);
    impl Drop for Backend {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    impl SourceLoader for Backend {
        fn read(&mut self, _: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
            panic!("closed source backend accessed")
        }
    }
    impl SampleLoader for Backend {
        fn load(&mut self, _: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
            panic!("closed sample backend accessed")
        }
    }
    impl SongAssetBackend for Backend {
        fn bank_len(&mut self, _: KwId, _: u32) -> Result<u32, Failure> {
            panic!("closed bank backend accessed")
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let prep = SongAssetPreparation::new(Box::new(Backend(dropped.clone())), limits()).unwrap();
    let mut source_handle = prep.source_loader();
    let mut sample_handle = prep.sample_loader();
    assert!(!dropped.get());
    let closed = prep.close().unwrap();
    assert!(dropped.get());
    assert!(source_handle.read(&path("lib.vact")).is_err());
    assert!(sample_handle.load(&SampleSrc::Path(path("x.wav"))).is_err());
    assert_eq!(closed.resource_count(), 0);
}

#[test]
fn compiled_lazy_sound_keywords_are_ambiguous_and_callbacks_are_never_forced() {
    use vactr::host::noop::NoopHost;
    use vactr::ns::{evaluator::Evaluator, namespace::Prelude, stage::RecordingSink};
    use vactr::vm::call::NativeCx;
    fn forbidden(_: &mut NativeCx<'_>, _: &[Value], _: &[(KwId, Value)]) -> Result<Value, Failure> {
        panic!("dependency discovery must not invoke sound callbacks")
    }
    for code in [
        "s {t -> :candidate-bank}",
        "fn lazy t:\n\tkeyword-probe\n\tfirst [:candidate-bank]\ns lazy",
    ] {
        let mut prelude = Prelude::core();
        prelude.register("keyword-probe", forbidden);
        let mut evaluator = Evaluator::new(
            prelude,
            Box::new(NoopHost),
            Box::new(RecordingSink::default()),
        );
        let outcomes = evaluator.eval_str(code, FileId::new(7)).unwrap();
        for outcome in &outcomes {
            assert!(outcome.value.is_ok(), "{:?}", outcome.value);
        }
        let value = outcomes.last().unwrap().value.as_ref().unwrap();
        let deps = song_asset_dependencies(std::slice::from_ref(value), limits()).unwrap();
        assert!(deps.literal_keywords.contains(&intern_kw("candidate-bank")));
        assert!(deps.sound_keywords.is_empty());
        assert!(deps.sample_paths.is_empty());
        assert!(deps.literal_paths.is_empty());
    }
}

#[test]
fn compiled_metadata_keywords_remain_separate_from_known_sound_keywords() {
    use vactr::host::noop::NoopHost;
    use vactr::ns::{evaluator::Evaluator, namespace::Prelude, stage::RecordingSink};
    let mut evaluator = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    let outcomes = evaluator
        .eval_str(
            "[:meter :scale :metadata]\ns :candidate-bank",
            FileId::new(7),
        )
        .unwrap();
    for outcome in &outcomes {
        assert!(outcome.value.is_ok(), "{:?}", outcome.value);
    }
    let metadata = song_asset_dependencies(
        std::slice::from_ref(outcomes[0].value.as_ref().unwrap()),
        limits(),
    )
    .unwrap();
    assert_eq!(
        metadata.literal_keywords,
        vec![
            intern_kw("meter"),
            intern_kw("scale"),
            intern_kw("metadata")
        ]
    );
    assert!(metadata.sound_keywords.is_empty());
    let known = song_asset_dependencies(
        std::slice::from_ref(outcomes[1].value.as_ref().unwrap()),
        limits(),
    )
    .unwrap();
    assert_eq!(known.sound_keywords, vec![intern_kw("candidate-bank")]);
    assert!(known.literal_keywords.is_empty());
}

#[test]
fn closed_pcm_bytes_preserve_empty_duplicate_and_mixed_asset_charges() {
    let empty = page(BTreeMap::new(), BTreeMap::new());
    let source_only = empty.begin(source(), limits()).unwrap();
    source_only.source_loader().read(&path("lib.vact")).unwrap();
    let closed = source_only.close().unwrap();
    assert_eq!(closed.pcm_bytes(), 0);
    assert_eq!(closed.resource_count(), 0);
    assert_eq!(closed.source_count(), 1);

    let bank = intern_kw("kit");
    let factory = page(
        BTreeMap::from([("a".into(), pcm(0.1)), ("b".into(), pcm(0.2))]),
        BTreeMap::from([(bank, vec!["a".into(), "b".into()])]),
    );
    let mut prep = factory.begin(source(), limits()).unwrap();
    let selected = [
        SongAssetSelector::Path(path("a")),
        SongAssetSelector::Bank(bank),
    ];
    prep.pin(&selected).unwrap();
    prep.pin(&selected).unwrap();
    let original = SampleBuf::ready(44100, vec![0.3, 0.4]);
    let copy = prep.pin_buffer(&original).unwrap();
    assert!(Rc::ptr_eq(&copy, &prep.pin_buffer(&original).unwrap()));
    let mut closed = prep.close().unwrap();
    // A path and its bank entry are distinct existing resource keys: 3*32 +8.
    assert_eq!(closed.pcm_bytes(), 104);
    assert_eq!(closed.resource_count(), 4);
    original.fill_at(96000, vec![0.9; 128]);
    original.fail(FailCode::Type, "changed original");
    assert_eq!(closed.pcm_bytes(), 104);
    assert_eq!(
        closed
            .load(&SampleSrc::Buffer { id: copy.id })
            .unwrap()
            .frames
            .len(),
        2
    );
    assert_eq!(
        closed
            .load(&SampleSrc::Path(path("a")))
            .unwrap()
            .frames
            .len(),
        8
    );
    assert_eq!(
        closed
            .load(&SampleSrc::Bank { kw: bank, index: 1 })
            .unwrap()
            .frames
            .len(),
        8
    );
    assert_eq!(closed.pcm_bytes(), 104);
}

#[test]
fn dependency_work_reports_exact_existing_counts_without_expanding_repeats() {
    use vactr::song::{capture_part, part_repeat, RepeatSeedMode};
    use vactr::value::ratio::Ratio64;
    assert_eq!(
        song_asset_dependencies(&[], limits())
            .unwrap()
            .consumed_work(),
        0
    );
    assert_eq!(
        song_asset_dependencies(&[Value::Nil], limits())
            .unwrap()
            .consumed_work(),
        1
    );
    let shared = Value::list(vec![Value::Nil, Value::Int(7)]);
    assert_eq!(
        song_asset_dependencies(std::slice::from_ref(&shared), limits())
            .unwrap()
            .consumed_work(),
        3
    );
    assert_eq!(
        song_asset_dependencies(&[shared.clone(), shared], limits())
            .unwrap()
            .consumed_work(),
        4
    );
    let child = Rc::new(capture_part(BTreeMap::new(), Ratio64::ONE).unwrap());
    let work = |count| {
        let repeated = Rc::new(part_repeat(child.clone(), count, RepeatSeedMode::Same).unwrap());
        song_asset_dependencies(&[Value::Part(repeated)], limits())
            .unwrap()
            .consumed_work()
    };
    assert_eq!(work(1), 3);
    assert_eq!(work(1_000_000_000), work(1));
    let mut exhausted = limits();
    exhausted.max_walk_nodes = 2;
    let result = song_asset_dependencies(&[Value::list(vec![Value::Nil, Value::Nil])], exhausted);
    assert!(result.is_err());
    assert_eq!(result.err().unwrap().code, FailCode::FuelExhausted);
}

#[test]
fn selected_source_dependency_scan_never_runs_callable_or_expands_repeat() {
    use vactr::pattern::pat::{PParam, Pat, PatNode};
    use vactr::song::{capture_part, part_repeat, InstrumentSelector, RepeatSeedMode, SongSource};
    let track = intern_kw("drums");
    let callback = Value::Native(
        vactr::types::natives::NativeTable::global()
            .get("print")
            .unwrap()
            .0,
    );
    let pattern = Rc::new(Pat::new(
        PatNode::Sound {
            src: PParam::Fn(callback),
            kit: None,
        },
        None,
        false,
    ));
    let part = Rc::new(
        capture_part(
            BTreeMap::from([(track, pattern)]),
            vactr::value::Ratio64::ONE,
        )
        .unwrap(),
    );
    let repeated = Rc::new(part_repeat(part, 1_000_000_000, RepeatSeedMode::Same).unwrap());
    let source = Rc::new(
        SongSource::new(
            repeated,
            track,
            InstrumentSelector::new(vec![Sound::Builtin(intern_kw("analog"))]).unwrap(),
        )
        .unwrap(),
    );
    let selected = Value::Pattern(Rc::new(Pat::new(PatNode::SongSource(source), None, false)));
    let deps = song_asset_dependencies(&[selected], limits()).unwrap();
    assert_eq!(deps.native_functions.len(), 1);
    assert!(deps.consumed_work() < 20);
}
