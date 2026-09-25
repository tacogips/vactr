//! The effect catalog (12.8.8): every `EffectKind` renders finite, bounded
//! output on noise and on silence with zero allocation; `mix 0` is
//! bit-identical to the input for every kind that has a mix; and
//! `meta::decl_for` has exactly one declaration per effect, ugen and
//! template name.

use std::collections::BTreeSet;

use super::{caps, noise, SR};
use crate::dsp::alloc_probe::armed;
use crate::dsp::arena::{SampleStore, StoreKind};
use crate::dsp::cells::AtomicCells;
use crate::dsp::effects::{self, mix_index, param_ctl, FxCtx, FxStats, FxUnit, MAX_FX_PARAMS};
use crate::dsp::fft::{Fft, FFT_SIZE};
use crate::dsp::graph::EffectKind;
use crate::dsp::meta;
use crate::dsp::ugen::catalog::{TEMPLATE_NAMES, UGEN_NAMES};
use crate::host::wire::Ctl;

const BLOCK: usize = 128;

/// Runs one unit over `input` (stereo, the same signal on both sides) in
/// blocks; returns `(left, right)`. Every block is probed for allocation.
fn render(
    kind: EffectKind,
    given: &[(crate::sched::slots::CtlId, Ctl)],
    input: &[f32],
) -> (Vec<f32>, Vec<f32>) {
    let caps = caps();
    let store = SampleStore::new(StoreKind::NativeArc);
    let fft = Fft::new(FFT_SIZE);
    let cells = AtomicCells::new(4);
    let mut mem = vec![0.0; effects::mem_len(kind, SR, &caps)];
    let mut unit = FxUnit::empty();
    unit.configure(kind, given, &cells, &mut mem, SR, &caps);
    let mut scratch = vec![0.0; 4 * FFT_SIZE];
    let mut analysis = vec![0.0; 1024];
    let mut dry = vec![0.0; 2 * BLOCK];
    let mut stats = FxStats::default();
    let (mut outl, mut outr) = (Vec::new(), Vec::new());
    let (mut l, mut r) = (vec![0.0; BLOCK], vec![0.0; BLOCK]);
    for chunk in input.chunks(BLOCK) {
        let n = chunk.len();
        l[..n].copy_from_slice(chunk);
        r[..n].copy_from_slice(chunk);
        let mut ctx = FxCtx {
            sr: SR,
            store: &store,
            fft: &fft,
            caps: &caps,
            scratch: &mut scratch,
            analysis: &mut analysis,
            stats: &mut stats,
        };
        let ((), allocs) = armed(|| {
            unit.update(&cells);
            unit.run(&mut mem, &mut l[..n], &mut r[..n], &mut dry, &mut ctx);
        });
        assert_eq!(allocs, 0, "{} allocated", kind.name());
        outl.extend_from_slice(&l[..n]);
        outr.extend_from_slice(&r[..n]);
    }
    (outl, outr)
}

#[test]
fn every_kind_is_finite_and_bounded_on_noise_and_silence() {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let two_seconds = (2.0 * SR) as usize;
    let input = noise(two_seconds, 0.5, 17);
    let silence = vec![0.0; BLOCK * 64];
    for &kind in EffectKind::ALL {
        let (l, r) = render(kind, &[], &input);
        let peak = l.iter().chain(&r).fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            l.iter().chain(&r).all(|v| v.is_finite()),
            "{} finite",
            kind.name()
        );
        assert!(peak <= 16.0, "{} bounded ({peak})", kind.name());
        let (l, r) = render(kind, &[], &silence);
        assert!(
            l.iter().chain(&r).all(|v| v.is_finite()),
            "{} finite on silence",
            kind.name()
        );
    }
}

#[test]
fn mix_zero_is_bit_identical_to_the_input() {
    let input = noise(BLOCK * 32, 0.5, 3);
    let mut with_mix = 0;
    for &kind in EffectKind::ALL {
        let Some(i) = mix_index(kind) else {
            continue;
        };
        with_mix += 1;
        let id =
            crate::sched::slots::CtlId::new(effects::EFFECT_PARAM_BASE + u16::try_from(i).unwrap());
        let (l, r) = render(kind, &[(id, Ctl::Const(0.0))], &input);
        assert_eq!(l, input, "{} mix 0 left", kind.name());
        assert_eq!(r, input, "{} mix 0 right", kind.name());
    }
    assert!(with_mix > 40, "most kinds have a mix ({with_mix})");
}

#[test]
fn every_kind_has_well_formed_parameters() {
    for &kind in EffectKind::ALL {
        let ps = effects::params(kind);
        assert!(ps.len() <= MAX_FX_PARAMS, "{}", kind.name());
        let names: BTreeSet<&str> = ps.iter().map(|p| p.name).collect();
        assert_eq!(names.len(), ps.len(), "{} has duplicate names", kind.name());
        for p in ps {
            assert!(
                p.min <= p.default && p.default <= p.max,
                "{}.{}",
                kind.name(),
                p.name
            );
            assert!(p
                .name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
            assert!(param_ctl(kind, p.name).is_some());
        }
    }
}

#[test]
fn meta_has_one_declaration_per_catalog_name() {
    let mut want: BTreeSet<&str> = EffectKind::ALL.iter().map(|k| k.name()).collect();
    want.extend(UGEN_NAMES.iter().copied());
    want.extend(TEMPLATE_NAMES.iter().copied());
    let all = meta::all();
    let names: Vec<&str> = all.iter().map(|d| d.name).collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "no duplicate declaration");
    assert_eq!(unique, want, "exactly the effect, ugen and template names");
    for name in &want {
        assert!(meta::decl_for(name).is_some(), "{name}");
    }
    let comp = meta::decl_for("compressor").unwrap();
    assert!(comp.params.iter().any(|p| p.name == "threshold"));
    assert_eq!(
        meta::decl_for("peq").unwrap().kind,
        meta::EditorKind::EqCurve
    );
    assert_eq!(
        meta::decl_for("sample-play").unwrap().kind,
        meta::EditorKind::SamplerWave
    );
    assert!(meta::decl_for("no-such-builtin").is_none());
}
