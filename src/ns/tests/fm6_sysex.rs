//! Evaluator behavior for the native six-operator SysEx file loader.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use super::reactive_basic::{Harness, MapLoader};
use crate::dsp::ugen::fm::patch::{global, Fm6Patch, VOICE_PARAMS};
use crate::dsp::ugen::fm::sysex::{encode_bulk, encode_single};
use crate::host::native::NativeSampleLoader;
use crate::ns::load::SourceLoader;
use crate::reader::span::FileId;
use crate::types::natives::NativeTable;
use crate::value::intern::intern_sym;
use crate::value::value::{PathVal, Value};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::tests::probe_prelude;
use crate::vm::vm::EffectMode;

#[derive(Default)]
struct BytesLoader {
    files: BTreeMap<String, Vec<u8>>,
}

impl BytesLoader {
    fn with_file(name: &str, bytes: Vec<u8>) -> Self {
        Self {
            files: BTreeMap::from([(name.to_owned(), bytes)]),
        }
    }
}

impl SourceLoader for BytesLoader {
    fn read(&mut self, _path: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        Err(Failure::new(
            FailCode::HostUnavailable,
            "text source loading is unavailable in this fixture",
        ))
    }

    fn read_bytes(&mut self, path: &PathVal, limit: u64) -> Result<Vec<u8>, Failure> {
        let name = path.text.trim_start_matches("./");
        let bytes = self.files.get(name).ok_or_else(|| {
            Failure::new(FailCode::HostUnavailable, format!("no byte file `{name}`"))
        })?;
        if bytes.len() as u64 > limit {
            return Err(Failure::new(
                FailCode::HostUnavailable,
                format!("`{name}` exceeds the byte limit"),
            ));
        }
        Ok(bytes.clone())
    }
}

fn synthetic_patch(seed: u8) -> Fm6Patch {
    let mut patch = Fm6Patch::EMPTY;
    patch.params[0] = seed;
    patch.params[global::ALGORITHM] = seed % 32;
    patch
}

fn with_bytes(name: &str, bytes: Vec<u8>) -> Harness {
    Harness::with(
        probe_prelude(),
        Box::new(BytesLoader::with_file(name, bytes)),
    )
}

fn assert_patch_value(value: &Value, patch: &Fm6Patch) {
    let Value::List(params) = value else {
        panic!("expected parameter list, got {value:?}");
    };
    assert_eq!(params.items.len(), VOICE_PARAMS);
    for (actual, expected) in params.items.iter().zip(patch.params) {
        assert!(matches!(actual, Value::Int(value) if *value == i32::from(expected)));
    }
}

#[test]
fn fm6_sysex_native_returns_bulk_voices() {
    let patches = std::array::from_fn(|index| synthetic_patch(index as u8));
    let mut h = with_bytes("bank.syx", encode_bulk(&patches));
    let Value::List(voices) = h.ok("fm6-sysex ./bank.syx") else {
        panic!("expected voice list");
    };
    assert_eq!(voices.items.len(), 32);
    assert_patch_value(&voices.items[0], &patches[0]);
    assert_eq!(h.show("len {fm6-sysex ./bank.syx}"), "32");
    assert_eq!(h.show("fm6-sysex ./bank.syx > first > len"), "155");
}

#[test]
fn fm6_sysex_native_returns_single_voice() {
    let patch = synthetic_patch(23);
    let mut h = with_bytes("single.syx", encode_single(&patch));
    let Value::List(voices) = h.ok("fm6-sysex ./single.syx") else {
        panic!("expected voice list");
    };
    assert_eq!(voices.items.len(), 1);
    assert_patch_value(&voices.items[0], &patch);
}

#[test]
fn fm6_sysex_native_reports_checksum_error() {
    let mut bytes = encode_single(&synthetic_patch(4));
    let checksum = bytes.len() - 2;
    bytes[checksum] ^= 1;
    let mut h = with_bytes("bad.syx", bytes);
    let error = h
        .run("fm6-sysex ./bad.syx")
        .pop()
        .and_then(|outcome| outcome.value.err())
        .expect("checksum failure");
    assert_eq!(error.code, FailCode::LoadFailed);
    assert!(error.message.contains("fm6-sysex"), "{error}");
    assert!(error.message.contains("checksum"), "{error}");
}

#[test]
fn fm6_sysex_native_without_bytes_support_is_host_unavailable() {
    let mut h = Harness::with(probe_prelude(), Box::new(MapLoader::default()));
    let error = h
        .run("fm6-sysex ./bank.syx")
        .pop()
        .and_then(|outcome| outcome.value.err())
        .expect("unsupported byte read");
    assert_eq!(error.code, FailCode::HostUnavailable);
}

#[test]
fn fm6_sysex_native_rejects_query_mode() {
    let mut h = with_bytes("bank.syx", encode_single(&synthetic_patch(1)));
    h.ok("fn fetch k:\n\tfm6-sysex ./bank.syx");
    let function = h.ev.ns().session_value("fetch").expect("fetch function");
    let (vm, ns) = h.ev.vm_and_ns();
    let result = vm.with_effect_mode(EffectMode::Query, |vm| {
        vm.call_value(ns, &function, vec![Value::Int(0)], Vec::new())
    });
    assert_eq!(
        result.map_err(|error| error.code).err(),
        Some(FailCode::EffectInQuery)
    );
}

#[test]
fn fm6_sysex_native_rejects_non_path_arguments() {
    let mut h = with_bytes("bank.syx", encode_single(&synthetic_patch(1)));
    let error = h
        .run("fm6-sysex \"bank.syx\"")
        .pop()
        .and_then(|outcome| outcome.value.err())
        .expect("path type failure");
    assert_eq!(error.code, FailCode::Type);
}

#[test]
fn fm6_sysex_native_type_signature() {
    let (_, signature) = NativeTable::global()
        .iter()
        .find(|(_, signature)| signature.name == "fm6-sysex")
        .expect("native table entry");
    assert!(signature.effectful);
    assert_eq!(signature.ty, &["fn path -> [[int]]"]);
    let prelude = probe_prelude();
    let evaluator = crate::ns::evaluator::Evaluator::new(
        prelude,
        Box::new(MapLoader::default()),
        Box::new(crate::ns::stage::RecordingSink::default()),
    );
    assert!(evaluator
        .ns()
        .prelude()
        .slot(intern_sym("fm6-sysex"))
        .is_some());
}

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let path =
            std::env::temp_dir().join(format!("vactr-fm6-sysex-{}-{nonce}", std::process::id()));
        fs::create_dir(&path).expect("create unique temporary root");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fm6_sysex_native_loader_enforces_limit() {
    let root = TempRoot::new();
    let oversized_path = root.path().join("oversized.syx");
    let valid_path = root.path().join("valid.syx");
    fs::File::create(&oversized_path)
        .expect("create oversized fixture")
        .write_all(&vec![0; 70 * 1024])
        .expect("write oversized fixture");
    let expected = vec![0x43; 4104];
    fs::File::create(&valid_path)
        .expect("create valid fixture")
        .write_all(&expected)
        .expect("write valid fixture");

    let mut loader = NativeSampleLoader::new(&[root.path().to_path_buf()], root.path());
    let path_value = |path: &Path| PathVal {
        text: Rc::from(path.to_string_lossy().as_ref()),
        file: None,
    };
    assert!(loader
        .read_bytes(&path_value(&oversized_path), 64 * 1024)
        .is_err());
    let actual = loader
        .read_bytes(&path_value(&valid_path), 64 * 1024)
        .expect("read within-limit fixture");
    assert_eq!(actual, expected);
}
