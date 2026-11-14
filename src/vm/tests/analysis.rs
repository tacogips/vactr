//! SS-ANALYSIS: the self-analysis natives (design 14.5.9), run through an
//! `Evaluator` whose source loader carries an `AnalysisCx` over the
//! recording host's synthetic taps.

use std::cell::RefCell;
use std::rc::Rc;

use crate::dsp::caps::CapabilitySet;
use crate::dsp::offline::OFFLINE_RATE;
use crate::host::caps::{AnalysisCx, TapReader, TapSrc};
use crate::host::noop::NoopHost;
use crate::host::testing::{MockClock, SynthTaps, SYNTH_AMP};
use crate::ns::evaluator::Evaluator;
use crate::ns::load::SourceLoader;
use crate::ns::namespace::Prelude;
use crate::ns::stage::{EffectSink, StagedEffect};
use crate::reader::span::FileId;
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;
use crate::value::sample::SampleBuf;
use crate::value::value::{PathVal, Sound, Value};
use crate::vm::fail::{FailCode, Failure};

const FILE: FileId = FileId::new(1);

/// A loader that reads nothing and carries the analysis context.
struct AnalysisLoader(AnalysisCx);

impl SourceLoader for AnalysisLoader {
    fn read(&mut self, _: &PathVal) -> Result<(FileId, Rc<str>), Failure> {
        Err(Failure::new(FailCode::HostUnavailable, "no files"))
    }

    fn analysis(&mut self) -> Option<&mut AnalysisCx> {
        Some(&mut self.0)
    }
}

/// A sink the test can read after boxing.
#[derive(Clone, Default)]
struct Staged(Rc<RefCell<Vec<StagedEffect>>>);

impl EffectSink for Staged {
    fn apply(&mut self, e: StagedEffect) {
        self.0.borrow_mut().push(e);
    }
}

struct Rig {
    ev: Evaluator,
    staged: Staged,
}

impl Rig {
    fn with(caps: CapabilitySet, taps: Option<Box<dyn TapReader>>) -> Rig {
        let staged = Staged::default();
        let loader = AnalysisLoader(AnalysisCx { caps, taps });
        let ev = Evaluator::new(Prelude::core(), Box::new(loader), Box::new(staged.clone()));
        Rig { ev, staged }
    }

    /// Native caps and synthetic taps at one second of mock time.
    fn native() -> Rig {
        let taps = SynthTaps {
            clock: MockClock::new(1.0),
        };
        Rig::with(CapabilitySet::native(), Some(Box::new(taps)))
    }

    fn eval(&mut self, src: &str) -> Result<Value, Failure> {
        let out = self.ev.eval_str(src, FILE).expect("reads");
        out.into_iter().last().expect("a form").value
    }

    fn fail(&mut self, src: &str) -> Failure {
        self.eval(src).expect_err(src)
    }

    fn floats(&mut self, src: &str) -> Vec<f32> {
        match self.eval(src).unwrap_or_else(|f| panic!("{src:?}: {f}")) {
            Value::List(l) => l
                .items
                .iter()
                .map(|v| match v {
                    Value::Float(x) => *x,
                    other => panic!("{src:?}: {other:?}"),
                })
                .collect(),
            other => panic!("{src:?}: {other:?}"),
        }
    }

    fn float(&mut self, src: &str) -> f32 {
        match self.eval(src).unwrap_or_else(|f| panic!("{src:?}: {f}")) {
            Value::Float(x) => x,
            other => panic!("{src:?}: {other:?}"),
        }
    }

    /// The buffer of the last staged `capture`/`render`.
    fn last_buffer(&self) -> Rc<SampleBuf> {
        let staged = self.staged.0.borrow();
        staged
            .iter()
            .rev()
            .find_map(|e| match e {
                StagedEffect::Capture { buf, .. } | StagedEffect::Render { buf, .. } => {
                    Some(Rc::clone(buf))
                }
                _ => None,
            })
            .expect("a staged buffer")
    }
}

#[test]
fn scope_of_the_master_tap_returns_the_last_frames() {
    let mut r = Rig::native();
    let xs = r.floats("scope :master 512");
    assert_eq!(xs.len(), 512);
    let peak = xs.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    #[allow(clippy::cast_possible_truncation)]
    let amp = SYNTH_AMP as f32;
    assert!((peak - amp).abs() < 0.01, "{peak}");
    assert_eq!(r.fail("scope :master 0").code, FailCode::Type);
    assert_eq!(r.fail("scope :master 8193").code, FailCode::Type);
}

#[test]
fn spectrum_of_the_master_tap_peaks_at_the_synthetic_frequency() {
    let mut r = Rig::native();
    let mags = r.floats("spectrum :master bins: 64");
    assert_eq!(mags.len(), 64);
    // 3000 Hz is bin 8 of a 128-point FFT at 48 kHz.
    let top = mags
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(k, _)| k);
    assert_eq!(top, Some(8), "{mags:?}");
    assert_eq!(r.floats("spectrum :master").len(), 64);
}

#[test]
fn a_bad_bins_value_is_a_type_failure() {
    let mut r = Rig::native();
    for src in [
        "spectrum :master bins: 48",
        "spectrum :master bins: 4",
        "spectrum :master bins: 4096",
    ] {
        assert_eq!(r.fail(src).code, FailCode::Type, "{src}");
    }
}

#[test]
fn slot_and_input_sources_are_beyond_capability() {
    let mut r = Rig::native();
    for src in [
        "scope :d1 64",
        "spectrum :d9",
        "scope :in 64",
        "capture :in 1",
    ] {
        let f = r.fail(src);
        assert_eq!(f.code, FailCode::BeyondCapability, "{src}");
        assert!(f.message.contains("not available on this host"), "{f}");
    }
    assert_eq!(r.fail("scope :nowhere 64").code, FailCode::Type);
}

#[test]
fn without_an_analysis_context_every_native_is_host_unavailable() {
    let mut ev = Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(Staged::default()),
    );
    for src in [
        "scope :master 64",
        "spectrum :master bins: 64",
        "capture :master 1",
        "render 2",
    ] {
        let out = ev.eval_str(src, FILE).expect("reads");
        let f = out[0].value.clone().expect_err(src);
        assert_eq!(f.code, FailCode::HostUnavailable, "{src}");
    }
    // With a context but no taps, a live tap is host-unavailable too.
    let mut r = Rig::with(CapabilitySet::native(), None);
    assert_eq!(r.fail("scope :master 64").code, FailCode::HostUnavailable);
}

#[test]
fn render_on_browser_caps_is_beyond_capability_at_the_call() {
    let mut r = Rig::with(CapabilitySet::browser(), None);
    let f = r.fail("let x 1\nrender 2");
    assert_eq!(f.code, FailCode::BeyondCapability);
    assert!(f.message.contains("not available on this host"), "{f}");
    let span = f.origin.span.expect("an origin");
    assert_eq!(span.file, FILE);
    let renders = r
        .staged
        .0
        .borrow()
        .iter()
        .filter(|e| matches!(e, StagedEffect::Render { .. }))
        .count();
    assert_eq!(renders, 0, "no render is staged");
    // The visual setting is unaffected, with or without an output.
    assert!(r.eval("render :o0").is_ok());
    assert!(r.eval("render").is_ok());
}

#[test]
fn render_and_capture_stage_pending_buffers() {
    let mut r = Rig::native();
    let v = r.eval("let out render 2\nout").expect("render");
    let Value::Sound(s) = v else { panic!("{v:?}") };
    let Sound::Buffer(b) = &*s else {
        panic!("{s:?}")
    };
    assert_eq!(b.frames(), None, "pending");
    assert_eq!(b.rate(), OFFLINE_RATE);
    let staged = r.staged.0.borrow().clone();
    let cycles = staged.iter().find_map(|e| match e {
        StagedEffect::Render { cycles, .. } => Some(*cycles),
        _ => None,
    });
    assert_eq!(cycles, Some(Ratio64::from_int(2)));
    r.eval("bus :drums:\n\tlevel").expect("a bus");
    r.eval("let hit capture :drums 1").expect("capture");
    let staged = r.staged.0.borrow().clone();
    let src = staged.iter().rev().find_map(|e| match e {
        StagedEffect::Capture { src, .. } => Some(*src),
        _ => None,
    });
    assert_eq!(src, Some(TapSrc::Bus(intern_kw("drums"))));
    assert_eq!(r.fail("capture :master 0").code, FailCode::Type);
}

#[test]
fn rms_and_peak_of_a_ready_buffer_match_the_closed_form() {
    let mut r = Rig::native();
    r.eval("let out render 1").expect("render");
    // A full-scale square wave in the left channel, silence in the right:
    // rms over both channels = sqrt(1/2), peak = 1.
    let frames: Vec<f32> = (0..1000)
        .flat_map(|k| [if k % 2 == 0 { 1.0 } else { -1.0 }, 0.0])
        .collect();
    r.last_buffer().fill(frames);
    let rms = r.float("rms out");
    assert!(
        (rms - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5,
        "{rms}"
    );
    assert!((r.float("peak out") - 1.0).abs() < 1e-6);
    // `scope buf n`: the first n mono frames.
    assert_eq!(r.floats("scope out 3"), vec![0.5, -0.5, 0.5]);
    assert_eq!(r.floats("spectrum out bins: 8").len(), 8);
}

#[test]
fn reading_a_pending_buffer_is_capture_pending() {
    let mut r = Rig::native();
    r.eval("let hit capture :master 1").expect("capture");
    for src in ["rms hit", "peak hit", "scope hit 8", "spectrum hit"] {
        assert_eq!(r.fail(src).code, FailCode::CapturePending, "{src}");
    }
    r.last_buffer().fail(FailCode::HostUnavailable, "gone");
    assert_eq!(r.fail("rms hit").code, FailCode::HostUnavailable);
}
