//! Golden render and graph digests captured before MOD-004 changes.

use std::collections::BTreeMap;
use std::sync::Arc;

use super::super::E2e;
use crate::dsp::caps::CapabilitySet;
use crate::dsp::graph::InstDef;
use crate::dsp::ugen::{BuildEnv, Template};
use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::ns::insts::TEMPLATE_NAMES;
use crate::sched::commit::SampleState;
use crate::value::intern::{intern_kw, name_of_kw};
use crate::vm::fail::Failure;

const SAMPLE_FRAMES: usize = 48_000;
const RENDER_SECONDS: f64 = 0.5;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DigestRecord {
    kind: &'static str,
    name: String,
    variant: &'static str,
    digest: u64,
    frames: usize,
}

impl DigestRecord {
    fn line(&self) -> String {
        format!(
            "{} {} {} {:016x} {}",
            self.kind, self.name, self.variant, self.digest, self.frames
        )
    }
}

fn fnv_bytes(bytes: impl IntoIterator<Item = u8>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fnv64(words: impl Iterator<Item = u32>) -> u64 {
    fnv_bytes(words.flat_map(u32::to_le_bytes))
}

fn render_record(name: &str, variant: &'static str, pan: Option<f32>) -> DigestRecord {
    let mut e = E2e::new();
    let pan = pan.map_or_else(String::new, |value| format!(" > pan {value}"));
    e.eval(&format!("s :{name} > note [:a3]{pan} > once"));
    let (left, right) = e.run_stereo_for(RENDER_SECONDS);
    let frames = left.len();
    let digest = fnv64(left.iter().chain(&right).map(|sample| sample.to_bits()));
    DigestRecord {
        kind: "render",
        name: name.to_owned(),
        variant,
        digest,
        frames,
    }
}

fn graph_record(name: &str, def: &InstDef) -> DigestRecord {
    let mut bytes = format!("{:?}", def.nodes).into_bytes();
    for edge in &def.edges {
        bytes.extend_from_slice(format!("{},{},{};", edge.from, edge.to, edge.port).as_bytes());
    }
    DigestRecord {
        kind: "graph",
        name: name.to_owned(),
        variant: "baseline",
        digest: fnv_bytes(bytes),
        frames: 0,
    }
}

struct RampLoader {
    channels: u8,
}

impl SampleLoader for RampLoader {
    fn load(&mut self, _src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let channels = usize::from(self.channels);
        let mut frames = Vec::with_capacity(SAMPLE_FRAMES * channels);
        for frame in 0..SAMPLE_FRAMES {
            #[allow(clippy::cast_precision_loss)]
            let value = (frame + 1) as f32 / SAMPLE_FRAMES as f32;
            frames.push(value);
            if channels == 2 {
                frames.push(-0.5 * value);
            }
        }
        Ok(Arc::new(SampleData {
            rate: 48_000,
            channels: self.channels,
            frames: frames.into_boxed_slice(),
        }))
    }
}

fn sampler_with_resource(channels: u8, variant: &'static str) -> DigestRecord {
    let mut e = E2e::with_loader(Box::new(RampLoader { channels }));
    e.eval("s :sampler > bank :bd > gain 0 > d9");
    let src = SampleSrc::Bank {
        kw: intern_kw("bd"),
        index: 0,
    };
    for _ in 0..40 {
        if matches!(
            e.rt.samples().state(&src),
            Some((_, SampleState::Installed))
        ) {
            e.eval("stop :d9");
            e.eval("s :sampler > bank :bd > note [:a3] > once");
            let (left, right) = e.run_stereo_for(RENDER_SECONDS);
            let frames = left.len();
            return DigestRecord {
                kind: "render",
                name: "sampler".to_owned(),
                variant,
                digest: fnv64(left.iter().chain(&right).map(|sample| sample.to_bits())),
                frames,
            };
        }
        e.run_for(0.1);
    }
    panic!("sampler {variant}: ramp sample did not install");
}

fn actual_records() -> Vec<DigestRecord> {
    let e = E2e::new();
    let env = BuildEnv {
        sr: 48_000.0,
        caps: CapabilitySet::native(),
        voice_mem: 24_000,
    };
    let mut templates = BTreeMap::new();
    for entry in e.reg.borrow().entries() {
        let name = String::from(name_of_kw(entry.name).as_ref());
        if TEMPLATE_NAMES
            .iter()
            .any(|template_name| *template_name == name.as_str())
        {
            templates.insert(name, entry.def.clone());
        }
    }
    assert_eq!(
        templates.len(),
        TEMPLATE_NAMES.len(),
        "all prelude templates realized"
    );

    let mut records = Vec::new();
    for (name, def) in &templates {
        records.push(graph_record(name, def));
        let template = Template::from_inst(def, &env)
            .unwrap_or_else(|error| panic!("{name}: template build {error:?}"));
        records.push(render_record(name, "center", None));
        if !template.has_aux && !template.has_quad {
            records.push(render_record(name, "pan02", Some(0.2)));
        }
    }
    records.push(sampler_with_resource(1, "sampler-mono-res"));
    records.push(sampler_with_resource(2, "sampler-stereo-res"));
    records.sort_by_key(DigestRecord::line);
    records
}

fn fixture_records() -> Vec<String> {
    include_str!("golden_digests.txt")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

fn mismatch_report(expected: &[String], actual: &[String]) -> Vec<String> {
    let expected_by_key: BTreeMap<_, _> = expected
        .iter()
        .map(|line| {
            (
                line.split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" "),
                line,
            )
        })
        .collect();
    let actual_by_key: BTreeMap<_, _> = actual
        .iter()
        .map(|line| {
            (
                line.split_whitespace()
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" "),
                line,
            )
        })
        .collect();
    let keys: std::collections::BTreeSet<_> =
        expected_by_key.keys().chain(actual_by_key.keys()).collect();
    keys.into_iter()
        .filter_map(|key| {
            let want = expected_by_key.get(key);
            let got = actual_by_key.get(key);
            (want != got).then(|| {
                format!(
                    "{}: expected {}; recomputed {}",
                    key,
                    want.map_or("<missing>", |line| line.as_str()),
                    got.map_or("<missing>", |line| line.as_str())
                )
            })
        })
        .collect()
}

#[test]
fn golden_renders_match_pre_mod004_baseline() {
    let expected = fixture_records();
    let actual: Vec<_> = actual_records().iter().map(DigestRecord::line).collect();
    let mismatches = mismatch_report(&expected, &actual);
    assert!(
        mismatches.is_empty(),
        "golden baseline mismatches:\n{}",
        mismatches.join("\n")
    );
}

#[test]
fn golden_renders_are_deterministic() {
    for name in ["analog", "filter-voice", "sampler"] {
        let first = render_record(name, "center", None);
        let second = render_record(name, "center", None);
        assert_eq!(first, second, "{name}: repeated render digest");
    }
}

#[test]
#[ignore = "blesses the pre-MOD-004 fixture once"]
fn bless_golden_digests() {
    if std::env::var("VACTR_BLESS_GOLDEN").as_deref() != Ok("1") {
        return;
    }
    let contents = actual_records()
        .iter()
        .map(DigestRecord::line)
        .collect::<Vec<_>>()
        .join("\n");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/host/tests/e2e/templates/golden_digests.txt");
    std::fs::write(path, format!("{contents}\n")).expect("write golden digest fixture");
}
