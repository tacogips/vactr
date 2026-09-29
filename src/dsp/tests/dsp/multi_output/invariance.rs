use super::{
    bits, browser_rig, default_ctls, fm_graph, load_browser, load_native, mono_sample_graph,
    native_rig, render_windowed, stereo_frames, stereo_graph, va_filter_graph,
};
use crate::dsp::graph::InstDef;
use crate::dsp::tests::dsp::rms;

const RATES: [f32; 3] = [44_100.0, 48_000.0, 96_000.0];
const BLOCKS: [usize; 3] = [64, 256, 97];

type Render = (Vec<f32>, Vec<f32>);

fn render_native(def: &InstDef, frames: Option<&[f32]>, rate: f32, block: usize) -> Render {
    let mut rig = native_rig(rate, block);
    load_native(&mut rig, def, frames);
    render_windowed(&mut rig, &default_ctls(), rate, block)
}

fn render_browser(def: &InstDef, frames: Option<&[f32]>, rate: f32, block: usize) -> Render {
    let mut rig = browser_rig(rate, block);
    load_browser(&mut rig, def, frames);
    render_windowed(&mut rig, &default_ctls(), rate, block)
}

fn render(browser: bool, def: &InstDef, frames: Option<&[f32]>, rate: f32, block: usize) -> Render {
    if browser {
        render_browser(def, frames, rate, block)
    } else {
        render_native(def, frames, rate, block)
    }
}

fn assert_signal(render: &Render, label: &str) {
    assert!(
        render
            .0
            .iter()
            .chain(&render.1)
            .all(|sample| sample.is_finite()),
        "{label}: finite"
    );
    assert!(rms(&render.0) > 1.0e-5, "{label}: left is audible");
    assert!(rms(&render.1) > 1.0e-5, "{label}: right is audible");
}

fn assert_render_bits_equal(a: &Render, b: &Render, label: &str) {
    assert_eq!(bits(&a.0), bits(&b.0), "{label}: left");
    assert_eq!(bits(&a.1), bits(&b.1), "{label}: right");
}

fn delta_bits(a: &[f32], b: &[f32]) -> Vec<u32> {
    a.iter()
        .zip(b)
        .map(|(left, right)| (left - right).to_bits())
        .collect()
}

fn assert_cross_block_contract(
    pair: &[(usize, Render)],
    fallback: &[(usize, Render)],
    rate: f32,
    tier: &str,
    name: &str,
    exact_delta: bool,
) {
    assert_eq!(pair.len(), fallback.len());
    let mut found_fallback = false;
    for first in 0..pair.len() {
        for second in (first + 1)..pair.len() {
            let (block_a, pair_a) = &pair[first];
            let (block_b, pair_b) = &pair[second];
            let pair_left_equal = bits(&pair_a.0) == bits(&pair_b.0);
            let pair_right_equal = bits(&pair_a.1) == bits(&pair_b.1);
            if pair_left_equal && pair_right_equal {
                continue;
            }
            let fallback_a = &fallback[first].1;
            let fallback_b = &fallback[second].1;
            if !pair_left_equal {
                assert_ne!(bits(&fallback_a.0), bits(&fallback_b.0),
                    "{name} left varies only in pair at {rate} Hz, {tier}, blocks {block_a}/{block_b}");
                if exact_delta {
                    assert_eq!(
                        delta_bits(&pair_a.0, &pair_b.0),
                        delta_bits(&fallback_a.0, &fallback_b.0),
                        "{name} left cross-block delta differs from fallback at {rate} Hz, {tier}"
                    );
                } else {
                    let pair_first = bits(&pair_a.0)
                        .iter()
                        .zip(bits(&pair_b.0))
                        .position(|(a, b)| a != &b);
                    let fallback_first = bits(&fallback_a.0)
                        .iter()
                        .zip(bits(&fallback_b.0))
                        .position(|(a, b)| a != &b);
                    assert_eq!(
                        pair_first, fallback_first,
                        "{name} left first differing frame differs from single-output reference at {rate} Hz, {tier}"
                    );
                }
            }
            if !pair_right_equal {
                assert_ne!(bits(&fallback_a.1), bits(&fallback_b.1),
                    "{name} right varies only in pair at {rate} Hz, {tier}, blocks {block_a}/{block_b}");
                if exact_delta {
                    assert_eq!(
                        delta_bits(&pair_a.1, &pair_b.1),
                        delta_bits(&fallback_a.1, &fallback_b.1),
                        "{name} right cross-block delta differs from fallback at {rate} Hz, {tier}"
                    );
                } else {
                    let pair_first = bits(&pair_a.1)
                        .iter()
                        .zip(bits(&pair_b.1))
                        .position(|(a, b)| a != &b);
                    let fallback_first = bits(&fallback_a.1)
                        .iter()
                        .zip(bits(&fallback_b.1))
                        .position(|(a, b)| a != &b);
                    assert_eq!(
                        pair_first, fallback_first,
                        "{name} right first differing frame differs from single-output reference at {rate} Hz, {tier}"
                    );
                }
            }
            let first_frame = pair_a
                .0
                .iter()
                .zip(&pair_b.0)
                .position(|(a, b)| a.to_bits() != b.to_bits())
                .or_else(|| {
                    pair_a
                        .1
                        .iter()
                        .zip(&pair_b.1)
                        .position(|(a, b)| a.to_bits() != b.to_bits())
                })
                .unwrap();
            let evidence = if exact_delta {
                "legacy and pair deltas match"
            } else {
                "single-output reference has the same first differing frame"
            };
            eprintln!(
                "FINDING: {name} pre-MOD-004 cross-block dependence at {rate} Hz, {tier}, blocks {block_a}/{block_b}, first differing frame {} ({evidence})",
                2048 + first_frame,
            );
            found_fallback = true;
        }
    }
    if !found_fallback {
        for first in 0..pair.len() {
            for second in (first + 1)..pair.len() {
                assert_render_bits_equal(
                    &pair[first].1,
                    &pair[second].1,
                    &format!(
                        "{name} cross-block {rate} Hz {tier} blocks {}/{}",
                        pair[first].0, pair[second].0
                    ),
                );
            }
        }
    }
}

#[test]
fn multi_output_rate_block_pair_tier_and_partition_contract() {
    let frames = stereo_frames(false, false);
    let stereo_samples = Some(frames.as_slice());
    for rate in RATES {
        let va_pair = va_filter_graph(true, 1.0);
        let va_legacy = va_filter_graph(false, 1.0);
        let fm_pair = fm_graph(true);
        let fm_legacy = fm_graph(false);
        let stereo = stereo_graph();
        let stereo_reference = mono_sample_graph();
        for browser in [false, true] {
            let tier = if browser { "browser" } else { "native" };
            let mut va_pairs = Vec::new();
            let mut va_old = Vec::new();
            let mut fm_pairs = Vec::new();
            let mut fm_old = Vec::new();
            let mut stereo_pair = Vec::new();
            let mut stereo_old = Vec::new();
            for block in BLOCKS {
                let va_new = render(browser, &va_pair, None, rate, block);
                let va_legacy_render = render(browser, &va_legacy, None, rate, block);
                let fm_new = render(browser, &fm_pair, None, rate, block);
                let fm_legacy_render = render(browser, &fm_legacy, None, rate, block);
                let stereo_new = render(browser, &stereo, stereo_samples, rate, block);
                let mono_render = render(browser, &stereo_reference, stereo_samples, rate, block);
                for (name, output) in [
                    ("VA pair", &va_new),
                    ("VA legacy", &va_legacy_render),
                    ("FM pair", &fm_new),
                    ("FM legacy", &fm_legacy_render),
                    ("stereo", &stereo_new),
                    ("stereo single-output reference", &mono_render),
                ] {
                    assert_signal(output, &format!("{name}, {rate} Hz, {block}, {tier}"));
                }
                assert_render_bits_equal(
                    &va_new,
                    &va_legacy_render,
                    &format!("VA pair/legacy {rate} Hz {block} {tier}"),
                );
                assert_render_bits_equal(
                    &fm_new,
                    &fm_legacy_render,
                    &format!("FM pair/legacy {rate} Hz {block} {tier}"),
                );
                va_pairs.push((block, va_new));
                va_old.push((block, va_legacy_render));
                fm_pairs.push((block, fm_new));
                fm_old.push((block, fm_legacy_render));
                stereo_pair.push((block, stereo_new));
                stereo_old.push((block, mono_render));
            }
            assert_cross_block_contract(&va_pairs, &va_old, rate, tier, "VA pair", true);
            assert_cross_block_contract(&fm_pairs, &fm_old, rate, tier, "FM pair", true);
            assert_cross_block_contract(
                &stereo_pair,
                &stereo_old,
                rate,
                tier,
                "stereo sample",
                false,
            );
        }

        for block in BLOCKS {
            let native_va = render_native(&va_pair, None, rate, block);
            let browser_va = render_browser(&va_pair, None, rate, block);
            let native_fm = render_native(&fm_pair, None, rate, block);
            let browser_fm = render_browser(&fm_pair, None, rate, block);
            let native_stereo = render_native(&stereo, stereo_samples, rate, block);
            let browser_stereo = render_browser(&stereo, stereo_samples, rate, block);
            assert_render_bits_equal(
                &native_va,
                &browser_va,
                &format!("VA tier {rate} Hz {block}"),
            );
            assert_render_bits_equal(
                &native_fm,
                &browser_fm,
                &format!("FM tier {rate} Hz {block}"),
            );
            assert_render_bits_equal(
                &native_stereo,
                &browser_stereo,
                &format!("stereo tier {rate} Hz {block}"),
            );
        }
    }
}
