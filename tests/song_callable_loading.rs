//! Actual verified package and source loading preserve declaration certificates.
use std::collections::BTreeMap;
use std::rc::Rc;
use vactr::ns::evaluator::Evaluator;
use vactr::ns::load::{NoopHost, SourceLoader};
use vactr::ns::namespace::Prelude;
use vactr::ns::pkg::{PackageId, PkgNs};
use vactr::ns::stage::RecordingSink;
use vactr::pattern::TimeSpan;
use vactr::pkg::{
    cache::CacheBackend,
    digest::sources_digest,
    lock::{LockEntry, LockFile},
    mem_cache::MemCache,
    semver::Version,
};
use vactr::reader::span::FileId;
use vactr::session::song::{evaluate_song_candidate, CandidateBuildCtx};
use vactr::song::assets::{DecodedSongAssetFactory, SongAssetLimits};
use vactr::song::{prepare_song, PreparedSong, ResolvedNote, SnapshotEpoch, SongLimits};
use vactr::value::value::PathVal;
use vactr::value::{Ratio64, Value};
use vactr::vm::{
    fail::{FailCode, Failure},
    query_vm::VmQuery,
};

const UTILS: &str = "github.com/example/vactr-util";
const MUSIC: &str = "github.com/example/vactr-music";
fn limits() -> SongAssetLimits {
    SongAssetLimits {
        max_resources: 16,
        max_pcm_bytes: 100_000,
        max_source_files: 32,
        max_source_bytes: 100_000,
        max_banks: 16,
        max_walk_nodes: 100_000,
        max_walk_depth: 256,
    }
}
fn cache_package(cache: &mut MemCache, path: &str, files: &[(&str, &str)]) -> LockEntry {
    let id = PackageId::new(path);
    let version = Version::parse_tag("v1.0.0").unwrap();
    let files: Vec<(Rc<str>, Rc<[u8]>)> = files
        .iter()
        .map(|(p, t)| (Rc::from(*p), Rc::from(t.as_bytes())))
        .collect();
    let sha256 = sources_digest(&files);
    let staging = cache.create_staging().unwrap();
    for (p, bytes) in &files {
        cache.write(staging, p, bytes).unwrap();
    }
    cache.publish(staging, &id, &version, sha256).unwrap();
    LockEntry {
        id,
        version,
        sha256,
    }
}
fn verified_library() -> (MemCache, LockFile) {
    let mut cache = MemCache::new();
    let util = cache_package(
        &mut cache,
        UTILS,
        &[("mod.vact", "fn identity p:\n\tgain p 1\n")],
    );
    let music = cache_package(&mut cache, MUSIC, &[
        ("a-helpers.vact", "import github.com/example/vactr-util as util\nfn helper p:\n\tutil.identity p\n"),
        ("b-music.vact", "fn make:\n\tpart [voice: {s :analog}] duration: 2\nfn wrap p pitch: float = 66.75:\n\thelper {s p} > note pitch\n"),
    ]);
    (cache, LockFile::new(vec![util, music]))
}
fn candidate(code: &str, cache: &MemCache, lock: &LockFile) -> Result<PreparedSong, Failure> {
    let factory = DecodedSongAssetFactory::new(BTreeMap::new(), BTreeMap::new(), BTreeMap::new());
    let cx = CandidateBuildCtx {
        assets: &factory,
        asset_limits: limits(),
        lock: Some(lock),
        cache: Some(cache),
    };
    prepare_song(evaluate_song_candidate(
        code,
        "score.vact",
        1,
        SnapshotEpoch(3),
        &cx,
    )?)
}
#[test]
fn verified_package_generators_and_transitive_callbacks_work_qualified_and_open() {
    let (cache, lock) = verified_library();
    for (import, make, wrap) in [
        (
            "import github.com/example/vactr-music as music",
            "music.make",
            "music.wrap",
        ),
        ("import github.com/example/vactr-music open", "make", "wrap"),
    ] {
        let code = format!("{import}\nlet base {{{make} & []}}\nlet edited {{transform-instrument base :voice :analog {wrap}}}\nsong edited > play-song");
        let mut song = candidate(&code, &cache, &lock).unwrap();
        assert_eq!(song.snapshot().duration(), Ratio64::from_int(2));
        let rows = song
            .query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
            .unwrap();
        assert!(!rows.is_empty());
        assert!(rows
            .iter()
            .all(|r| r.note == Some(ResolvedNote::Float32(66.75))));
        assert_eq!(song.snapshot().routing().sources.len(), 4);
    }
}
#[test]
fn package_keyword_override_and_invalid_arguments_preserve_full_scheme() {
    let (cache, lock) = verified_library();
    let code = "import github.com/example/vactr-music as music\nlet base {music.make & []}\nlet edited {transform-instrument base :voice :analog {p -> music.wrap p pitch: 69.25}}\nsong edited > play-song";
    let mut song = candidate(code, &cache, &lock).unwrap();
    assert_eq!(
        song.query(TimeSpan::cycle(0).unwrap(), &SongLimits::default())
            .unwrap()[0]
            .note,
        Some(ResolvedNote::Float32(69.25))
    );
    for callback in [
        "{p -> music.wrap p unknown: 2}",
        "{p -> music.wrap p pitch: \"bad\"}",
        "{p -> music.wrap p p}",
        "{p -> music.wrap}",
        "{p -> music.wrap 2}",
    ] {
        let code = format!("import github.com/example/vactr-music as music\nlet base {{music.make & []}}\nlet edited {{transform-instrument base :voice :analog {callback}}}\nsong edited > play-song");
        assert!(candidate(&code, &cache, &lock).is_err(), "{callback}");
    }
}
struct TextLoader(BTreeMap<String, Rc<str>>);
impl SourceLoader for TextLoader {
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        self.0
            .get(path.text.trim_start_matches("./"))
            .cloned()
            .map(|t| (FileId::new(10), t))
            .ok_or_else(|| Failure::new(FailCode::LoadFailed, "missing fixture"))
    }
}
fn evaluator(source: &str) -> Evaluator {
    Evaluator::new(
        Prelude::core(),
        Box::new(TextLoader(BTreeMap::from([(
            "loaded.vact".into(),
            source.into(),
        )]))),
        Box::new(RecordingSink::default()),
    )
}
fn clean(ev: &mut Evaluator, code: &str) {
    let rows = ev.eval_str(code, FileId::CONSOLE).unwrap();
    for row in rows {
        assert!(row.value.is_ok(), "{:?}", row.value);
        assert!(
            row.diags
                .iter()
                .all(|d| d.severity != vactr::types::Severity::Error),
            "{:?}",
            row.diags
        );
    }
}
#[test]
fn source_file_internal_generators_return_an_editable_part() {
    let mut ev = evaluator("fn helper p:\n\tgain p 1\nfn make:\n\tpart [voice: {s :analog}] duration: 2\nfn wrap p pitch: float = 66.75:\n\thelper {s p} > note pitch\nlet base {make & []}\nlet edited {transform-instrument base :voice :analog wrap}\nfirst [edited]");
    clean(
        &mut ev,
        "let loaded {load ./loaded.vact}\nlet longer {part-repeat loaded 2}",
    );
    let Value::Part(part) = ev.ns().session_value("longer").unwrap() else {
        panic!("Part");
    };
    assert_eq!(part.duration(), Ratio64::from_int(4));
    assert!(ev.ns().session_value("helper").is_none());
    let (vm, ns) = ev.vm_and_ns();
    let rows = vactr::song::query_part(
        &part,
        TimeSpan::cycle(0).unwrap(),
        &mut vactr::song::SongQueryCtx {
            vm: &mut VmQuery::new(vm, ns),
            cells: &vactr::pattern::eval::InputCells::default(),
            seed: 0,
            tempo: vactr::clock::tempo::Tempo::default(),
            limits: &SongLimits::default(),
        },
    )
    .unwrap();
    assert!(!rows.is_empty());
    assert_eq!(rows[0].tone, Some(ResolvedNote::Float32(66.75)));
}

fn load_verified(code: &str) -> (Rc<PkgNs>, Vec<vactr::types::Diagnostic>) {
    let mut cache = MemCache::new();
    let entry = cache_package(&mut cache, MUSIC, &[("mod.vact", code)]);
    let lock = LockFile::new(vec![entry]);
    let source = vactr::pkg::locked_sources(&lock, &cache, &PackageId::new(MUSIC)).unwrap();
    let mut ev = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    );
    let prelude = ev.ns().prelude().clone();
    let (vm, _) = ev.vm_and_ns();
    PkgNs::load(
        PackageId::new(MUSIC),
        prelude,
        vm,
        &source,
        &vactr::types::manifest::HostManifest::spec_default(),
        &mut |_| FileId::new(20),
        &mut |_| None,
    )
}
#[test]
fn loaded_package_schemes_reject_stale_transitive_dependencies_and_replacement() {
    use vactr::value::intern::intern_sym;
    let (pkg, diags) = load_verified("fn helper p:\n\tgain p 1\nfn dependent p:\n\thelper p\nfn transitive p:\n\tdependent p\nfn unrelated p:\n\tgain p 0.5");
    assert!(diags.is_empty(), "{diags:?}");
    assert!(pkg
        .ns
        .check_env()
        .global_callables
        .contains_key("transitive"));
    let helper = pkg.ns.session_slot(intern_sym("helper")).unwrap();
    helper.set(helper.get());
    let env = pkg.ns.check_env();
    assert!(!env.global_callables.contains_key("helper"));
    assert!(!env.global_callables.contains_key("dependent"));
    assert!(!env.global_callables.contains_key("transitive"));
    assert!(env.global_callables.contains_key("unrelated"));
    let (new_pkg, diags) = load_verified("fn replacement p:\n\tgain p 0.7");
    assert!(diags.is_empty());
    assert!(!new_pkg
        .ns
        .check_env()
        .global_callables
        .contains_key("helper"));
    assert!(new_pkg
        .ns
        .check_env()
        .global_callables
        .contains_key("replacement"));
}
#[test]
fn invalid_and_failed_package_declarations_never_publish_false_proofs() {
    let (pkg, diags) = load_verified("fn good p:\n\tgain p 1\nfn bad p: float:\n\ts p\nlet failed {/ 1 0}\nfn later p:\n\tgain p 0.7");
    assert!(diags
        .iter()
        .any(|d| d.code == vactr::types::DiagCode::PackageLoadFailed));
    let env = pkg.ns.check_env();
    assert!(env.global_callables.contains_key("good"));
    assert!(!env.global_callables.contains_key("bad"));
    assert!(!env.global_callables.contains_key("failed"));
    assert!(env.global_callables.contains_key("later"));
    assert!(pkg.ns.session_value("failed").is_none());
    let (pkg, diags) = load_verified("fn helper p:\n\tgain p 1\nfn helper p: float:\n\ts p");
    assert!(!diags.is_empty());
    assert!(!pkg.ns.check_env().global_callables.contains_key("helper"));
}
#[test]
fn per_form_publication_does_not_duplicate_or_change_document_diagnostics() {
    let code = "fn bad p: float:\n\ts p\nfn good p:\n\tgain p 1\nfirst [good]";
    let mut ev = evaluator(code);
    let fresh = vactr::ns::namespace::Namespace::with_prelude(ev.ns().prelude().clone());
    let forms = vactr::ns::load::read_forms(code, FileId::new(10)).unwrap();
    let expected = vactr::types::check::check(
        &forms,
        &fresh.check_env(),
        &vactr::types::manifest::HostManifest::spec_default(),
    )
    .diags;
    let outcome = ev.eval_str("load ./loaded.vact", FileId::CONSOLE).unwrap();
    assert!(
        outcome[0].value.is_ok(),
        "diagnostics alone preserve legacy loading"
    );
    let actual: Vec<_> = outcome
        .into_iter()
        .flat_map(|r| r.diags)
        .filter(|d| d.span.file == FileId::new(10))
        .collect();
    assert_eq!(actual, expected);
    assert!(!actual.is_empty());
}
#[test]
fn returned_function_invocation_and_callback_transport_are_distinct() {
    let mut ev = evaluator("fn make:\n\tpart [voice: {s :analog}] duration: 1\nfirst [make]");
    clean(
        &mut ev,
        "let loaded {load ./loaded.vact}\nlet produced {loaded & []}",
    );
    assert!(matches!(
        ev.ns().session_value("produced"),
        Some(Value::Part(_))
    ));
    assert!(
        !ev.ns().check_env().global_callables.contains_key("loaded"),
        "Value transport has no declaration certificate"
    );
    let mut ev = evaluator("fn wrap p pitch: float = 66.75:\n\ts p > note pitch\nfirst [wrap]");
    clean(
        &mut ev,
        "let loaded {load ./loaded.vact}\nlet base {part [voice: {s :analog}] duration: 1}",
    );
    let result = ev
        .eval_str(
            "transform-instrument base :voice :analog loaded",
            FileId::CONSOLE,
        )
        .unwrap();
    assert!(
        result
            .iter()
            .flat_map(|r| &r.diags)
            .any(|d| d.severity == vactr::types::Severity::Error),
        "returned Fn callback is not falsely certified"
    );
}
#[test]
fn exported_shared_variables_do_not_become_independent_argument_schemes() {
    let (pkg, diags) = load_verified("fn same a b:\n\t= a b");
    assert!(diags.is_empty(), "{diags:?}");
    let env = pkg.ns.check_env();
    let manifest = vactr::types::manifest::HostManifest::spec_default();
    for (code, valid) in [
        ("same 1 2", true),
        ("same true false", true),
        ("same 1 true", false),
    ] {
        let forms = vactr::ns::load::read_forms(code, FileId::new(30)).unwrap();
        let checked = vactr::types::check::check(&forms, &env, &manifest);
        assert_eq!(
            checked
                .diags
                .iter()
                .all(|d| d.severity != vactr::types::Severity::Error),
            valid,
            "{code}: {:?}",
            checked.diags
        );
    }
}

#[test]
fn actual_compile_failure_does_not_publish_and_later_package_forms_continue() {
    // Flat legal syntax avoids changing reader depth or thread-stack policy.
    let code = format!(
        "fn too-large:\n\tfirst [{}]\nfn later p:\n\tgain p 1",
        "0 ".repeat(70_000)
    );
    let forms = vactr::ns::load::read_forms(&code, FileId::new(20)).unwrap();
    let ns = vactr::ns::namespace::Namespace::new(Prelude::core());
    let mut cx = vactr::compile::CompileCx::new(&ns, vactr::ns::namespace::FormGen::new(1));
    let error = vactr::compile::compile(&forms[0], &mut cx).unwrap_err();
    assert_eq!(error.code, vactr::types::DiagCode::NestingTooDeep);
    assert!(error.message.contains("too large"));
    let (pkg, diags) = load_verified(&code);
    assert!(diags.iter().any(|d| d.message.contains("too large")));
    assert!(!pkg
        .ns
        .check_env()
        .global_callables
        .contains_key("too-large"));
    assert!(pkg.ns.session_value("too-large").is_none());
    assert!(pkg.ns.check_env().global_callables.contains_key("later"));
}

#[test]
fn isolated_candidate_loads_source_generated_part_without_active_aliases() {
    let source = "fn helper p:\n\tgain p 1\nfn make:\n\tpart [voice: {s :analog}] duration: 1\nfn wrap p pitch: float = 66.75:\n\thelper {s p} > note pitch\nlet base {make & []}\nlet edited {transform-instrument base :voice :analog wrap}\nfirst [edited]";
    let factory = DecodedSongAssetFactory::new(
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::from([("./loaded.vact".into(), (FileId::new(10), Rc::from(source)))]),
    );
    let cx = CandidateBuildCtx {
        assets: &factory,
        asset_limits: limits(),
        lock: None,
        cache: None,
    };
    let candidate = evaluate_song_candidate(
        "let loaded {load ./loaded.vact}\nsong {part-repeat loaded 2} > play-song",
        "score.vact",
        1,
        SnapshotEpoch(3),
        &cx,
    )
    .unwrap();
    let mut song = prepare_song(candidate).unwrap();
    assert_eq!(song.snapshot().duration(), Ratio64::from_int(2));
    let events = song
        .query(TimeSpan::cycle(1).unwrap(), &SongLimits::default())
        .unwrap();
    assert!(!events.is_empty());
    assert_eq!(events[0].note, Some(ResolvedNote::Float32(66.75)));
    assert!(song
        .snapshot()
        .routing()
        .sources
        .iter()
        .any(|file| file.text.contains("fn make")));
}
