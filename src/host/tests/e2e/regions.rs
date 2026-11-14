//! Sample-region operators end to end (design 10.1; TASK-008 criterion 3):
//! `begin`/`end`, `chop`, `slice`, `splice`, `loop-at`, `fit`, rendered
//! through the `E2e` rig with a deterministic RAMP sample so a region is
//! identifiable by its values (`ramp_at(frame) == (frame + 1) / N`, always
//! positive and strictly increasing), and the multiplicity/partition
//! invariants of design 11.3 layer 2 (TASK-007 criterion 1), proven here
//! through the real `NativeAudioHost` ring instead of `sched`'s recording
//! stub (`src/sched/tests/sched/merge.rs`).
//!
//! Serial repair R3 resolves `speed-fit` (`splice`/`loop-at`/`fit`'s
//! commit-time rate marker) into the `speed` control in
//! `src/sched/commit.rs`, which previously had no reader at all (`splice`
//! rendered identically to plain `slice`). Serial repair R5 fixes
//! `query_loop_at` (`src/pattern/combinators/region.rs`) writing the
//! Bool-domain `loop` control as `Value::Int(1)`, which failed every
//! `loop-at` event's commit with `type`; plain `loop-at` (no workaround)
//! is asserted below to both loop and stretch to the right speed.

use std::sync::Arc;

use super::E2e;
use crate::host::caps::{SampleData, SampleLoader, SampleSrc};
use crate::ns::stage::SlotKey;
use crate::pattern::query::TimeSpan;
use crate::sched::commit::SampleState;
use crate::value::intern::intern_kw;
use crate::value::ratio::Ratio64;
use crate::vm::fail::Failure;

/// The ramp sample's frame count (one second at 48 kHz).
const N: usize = 48_000;
const D1: SlotKey = SlotKey::D(1);

struct RampLoader;

impl SampleLoader for RampLoader {
    fn load(&mut self, _src: &SampleSrc) -> Result<Arc<SampleData>, Failure> {
        let frames: Vec<f32> = (0..N).map(ramp_at).collect();
        Ok(Arc::new(SampleData {
            rate: 48_000,
            channels: 1,
            frames: frames.into_boxed_slice(),
        }))
    }
}

/// The ramp's value at `frame` (identifies a region by value; always > 0).
fn ramp_at(frame: usize) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    {
        (frame.min(N - 1) + 1) as f32 / N as f32
    }
}

fn r(n: i64, d: i64) -> Ratio64 {
    Ratio64::new(n, d).expect("a ratio")
}

fn add(a: Ratio64, b: Ratio64) -> Ratio64 {
    a.checked_add(b).expect("ratio add")
}

fn span(begin: Ratio64, end: Ratio64) -> TimeSpan {
    TimeSpan { begin, end }
}

/// A rig with the ramp loader over `:bd`, requested (at zero gain, so it
/// never itself sounds) and installed before returning: a subsequent
/// single-shot render is then guaranteed to actually play, instead of
/// being dropped behind the sample's own (one-shot) async load gate. The
/// priming slot (`d9`) is stopped before returning so it never adds to
/// `E2e::committed` (an exact-multiplicity counter) once the sample is
/// installed, it never needs to request it again.
fn primed() -> E2e {
    let mut e = E2e::with_loader(Box::new(RampLoader));
    e.eval("s :bd > gain 0 > d9");
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
            return e;
        }
        e.run_for(0.1);
    }
    panic!("the ramp sample never installed");
}

/// The largest absolute sample value.
fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0_f32, |m, v| m.max(v.abs()))
}

/// The first and last indices whose absolute value exceeds `eps`.
fn nonzero_run(samples: &[f32], eps: f32) -> Option<(usize, usize)> {
    let first = samples.iter().position(|v| v.abs() > eps)?;
    let last = samples.iter().rposition(|v| v.abs() > eps)?;
    Some((first, last))
}

/// Rising-edge onsets: indices where the signal crosses above `eps` after
/// a run at or below it (one per audible voice start, given onsets spaced
/// well apart relative to the envelope's decay).
fn onsets(samples: &[f32], eps: f32) -> Vec<usize> {
    let mut out = Vec::new();
    let mut below = true;
    for (i, v) in samples.iter().enumerate() {
        let above = v.abs() > eps;
        if above && below {
            out.push(i);
        }
        below = !above;
    }
    out
}

/// Renders `seconds` through `commit_only` only (no new queries): the
/// caller stages spans itself with `Runtime::stage_span`.
fn render_committed(e: &mut E2e, seconds: f64) -> Vec<f32> {
    const BLOCK: usize = 256;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let frames_total = (seconds * f64::from(super::SR)).round() as usize;
    let mut left = Vec::with_capacity(frames_total);
    let mut rendered = 0usize;
    while rendered < frames_total {
        let now = e.clock.now();
        let rep = e.rt.commit_only(now);
        e.committed += rep.committed;
        e.faults.extend(rep.faults);
        let mut buf = vec![0.0f32; BLOCK * 2];
        let (_, allocs) = crate::dsp::alloc_probe::armed(|| e.side.render(&mut buf, 2));
        assert_eq!(allocs, 0, "the audio callback allocated");
        left.extend(buf.chunks_exact(2).map(|f| f[0]));
        rendered += BLOCK;
    }
    left
}

/// `render_committed`, but starting the returned window at cycle `from`
/// (a lane's binding boundary, rarely "now" exactly): renders the lead-in
/// too (so it is genuinely committed and staged content plays out right on
/// time), then drops it from the result.
fn render_from(e: &mut E2e, from: Ratio64, seconds: f64) -> Vec<f32> {
    let now = e.clock.now();
    let target = e.rt.clock().to_host(from);
    let lead = (target - now).max(0.0);
    let total = render_committed(e, lead + seconds);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let skip = (lead * f64::from(super::SR)).round() as usize;
    total[skip.min(total.len())..].to_vec()
}

/// A release much longer than every render window here: the envelope
/// stays close to 1 throughout, so a region's audible duration reflects
/// `sample-play`'s own region/speed math, not envelope decay.
const LONG: &str = "attack 0 > release 10";

/// The contiguous runs of samples whose absolute value exceeds `eps`
/// (one per voice, given onsets spaced apart by more than a block).
fn nonzero_runs(samples: &[f32], eps: f32) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, v) in samples.iter().enumerate() {
        match (start, v.abs() > eps) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                out.push((s, i - 1));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, samples.len() - 1));
    }
    out
}

#[test]
fn begin_end_renders_exactly_its_region() {
    // A full [0, 1) playthrough as the reference: `sample-play`'s envelope
    // and amp/pan scaling are identical at the same offset from onset, so
    // comparing samples (not raw ramp values) cancels them out exactly.
    let mut full = primed();
    full.eval(&format!("s :bd > {LONG} > once"));
    let full_out = full.run_for(1.5);
    let (ff, _) = nonzero_run(&full_out, 1.0e-6).expect("the reference sounds");

    let mut e = primed();
    e.eval(&format!("s :bd > begin 0.25 > end 0.5 > {LONG} > once"));
    let left = e.run_for(1.5);
    let (first, last) = nonzero_run(&left, 1.0e-6).expect("a voice rendered");
    let n = last - first + 1;
    // 0.25 s of the 48 kHz ramp at normal speed.
    assert!((n as i64 - 12_000).abs() <= 4, "region length: {n}");
    for k in (0..n).step_by(997) {
        let want = full_out[ff + 12_000 + k];
        let got = left[first + k];
        assert!(
            (got - want).abs() < want.abs() * 0.2 + 1.0e-5,
            "sample {k} of [0.25, 0.5): got {got}, the [0, 1) reference has {want}"
        );
    }
}

#[test]
fn chop_renders_each_half_s_own_region() {
    let mut full = primed();
    full.eval(&format!("s :bd > {LONG} > once"));
    let full_out = full.run_for(1.5);
    let (ff, _) = nonzero_run(&full_out, 1.0e-6).expect("the reference sounds");

    let mut e = primed();
    e.eval(&format!("s :bd > chop 2 > {LONG} > once"));
    let left = e.run_for(2.5);
    let runs = nonzero_runs(&left, 1.0e-6);
    assert_eq!(runs.len(), 2, "two chop pieces: {runs:?}");
    // Piece 0 is the ramp's [0, 1/2), piece 1 its [1/2, 1), each at normal
    // speed (chop sets no `speed-fit`): half the reference's samples.
    for (piece, (s, en)) in runs.iter().copied().enumerate() {
        let n = en - s + 1;
        assert!((n as i64 - 24_000).abs() <= 4, "piece {piece} length: {n}");
        let offset = piece * (N / 2);
        for k in (0..n).step_by(1997) {
            let want = full_out[ff + offset + k];
            let got = left[s + k];
            assert!(
                (got - want).abs() < want.abs() * 0.2 + 1.0e-5,
                "piece {piece} sample {k}: got {got}, want {want}"
            );
        }
    }
}

#[test]
fn splice_rate_fits_its_step_unlike_plain_slice() {
    let mut slice_e = primed();
    slice_e.eval(&format!("s :bd > slice 2 0 > {LONG} > once"));
    let slice_out = slice_e.run_for(1.5);
    let (sf, sl) = nonzero_run(&slice_out, 1.0e-6).expect("slice sounds");
    let slice_len = sl - sf + 1;
    // Plain `slice` plays its half-sample region at normal speed: 0.5 s.
    assert!(
        (slice_len as i64 - 24_000).abs() <= 4,
        "slice length: {slice_len}"
    );

    let mut splice_e = primed();
    splice_e.eval(&format!("s :bd > splice 2 0 > {LONG} > once"));
    let splice_out = splice_e.run_for(3.0);
    let (pf, pl) = nonzero_run(&splice_out, 1.0e-6).expect("splice sounds");
    let splice_len = pl - pf + 1;
    // `splice` fits the same half-sample region to the event's own step
    // (one cycle, 2 s): `speed = (1/2 * 1 s) / (1 * 2 s) = 1/4`, so the
    // region takes 4x as long, ~2 s (96 000 frames).
    assert!(
        splice_len > slice_len * 3,
        "splice ({splice_len}) is rate-fitted, much slower than plain slice ({slice_len})"
    );
    assert!(
        (splice_len as i64 - 96_000).abs() < 4_800,
        "splice length ~2 s: {splice_len}"
    );
}

#[test]
fn loop_at_plays_with_loop_enabled_past_one_pass() {
    // R5: plain `loop-at` (no workaround) plays and loops.
    let mut e = primed();
    e.eval(&format!("s :bd > loop-at 2 > {LONG} > once"));
    let left = e.run_for(5.0);
    let (first, last) = nonzero_run(&left, 1.0e-6).expect("a voice rendered");
    // `speed-fit` stretches the 1 s sample over 2 cycles (4 s); a
    // non-looping voice would reach the region's end and stop well inside
    // a 5 s window, so a voice still sounding for most of it demonstrates
    // the wraparound.
    let seconds = f64::from((last - first) as u32) / f64::from(super::SR);
    assert!(
        seconds > 4.5,
        "loop enabled keeps the voice sounding: {seconds}s"
    );
    assert!(left[first..=last].iter().all(|v| v.is_finite()));

    // The stretch itself: speed = 1 s / (2 cycles * 2 s) = 0.25, so one
    // full pass (start back to start) takes 4 s — the sharpest drop in the
    // (finite, decaying-envelope-aside) monotonic-per-pass ramp signal.
    let voice = &left[first..=last];
    let (drop_at, _) = voice
        .windows(2)
        .enumerate()
        .min_by(|(_, a), (_, b)| (a[1] - a[0]).total_cmp(&(b[1] - b[0])))
        .expect("a wraparound");
    let pass_seconds = f64::from(u32::try_from(drop_at).unwrap_or(0)) / f64::from(super::SR);
    assert!(
        (pass_seconds - 4.0).abs() < 0.3,
        "one loop pass is ~4 s (speed 0.25): {pass_seconds}s"
    );
}

#[test]
fn fit_matches_the_event_length() {
    let mut e = primed();
    e.eval(&format!("s :bd > fit > {LONG} > once"));
    let left = e.run_for(3.0);
    let (first, last) = nonzero_run(&left, 1.0e-6).expect("a voice rendered");
    // `fit` stretches the whole 1 s sample over the event's own span (one
    // cycle, 2 s by default): far longer than the sample's own duration.
    let seconds = f64::from((last - first) as u32) / f64::from(super::SR);
    assert!(
        (seconds - 2.0).abs() < 0.2,
        "fit matches the event length (~2 s): got {seconds}s"
    );
}

#[test]
fn region_bounds_never_exceed_the_ramp_and_never_produce_nan() {
    for (b, en) in [(0.0, 1.0), (0.0, 0.0), (1.0, 1.0), (0.4, 0.4), (0.9, 1.0)] {
        let mut e = primed();
        e.eval(&format!("s :bd > begin {b} > end {en} > {LONG} > once"));
        let left = e.run_for(1.5);
        assert!(left.iter().all(|v| v.is_finite()), "{b}..{en}: finite");
        let peak = left.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
        assert!(peak <= 2.0, "{b}..{en}: bounded (peak {peak})");
    }
}

/// The peak of `pattern` bound to `d1` and staged over exactly one clean,
/// non-overlapping cycle `[from, from + 1)`: a same-setup amplitude
/// reference for the multiplicity tests below (their own `stage_span`
/// windows only ever overlap or reorder the identical occurrences this
/// covers once each, so a doubled/duplicated voice would sum to ~2x this).
fn clean_cycle_peak(pattern: &str, seconds: f64) -> f32 {
    let mut e = primed();
    e.eval(pattern);
    let from = lane_from(&e, D1);
    e.rt.stage_span(&mut e.ev, D1, span(from, add(from, r(1, 1))));
    peak(&render_from(&mut e, from, seconds))
}

#[test]
fn overlapped_staging_gives_each_voice_start_exactly_once() {
    let mut e = primed();
    e.eval("s [:bd :bd :bd :bd] > d1");
    // A fresh binding starts at the next whole-cycle boundary (11.2), not
    // necessarily cycle 0.
    let from = lane_from(&e, D1);
    let before = e.committed;
    e.rt.stage_span(&mut e.ev, D1, span(from, add(from, r(3, 4))));
    e.rt.stage_span(&mut e.ev, D1, span(add(from, r(1, 4)), add(from, r(1, 1))));
    let left = render_from(&mut e, from, 2.0);
    let found = onsets(&left, 1.0e-4);
    assert_eq!(
        found.len(),
        4,
        "each of the 4 onsets exactly once: {found:?}"
    );
    // A rising-edge count alone cannot tell 1 emission from 2 at the same
    // frame (they merge into one edge): `committed` is `TickReport`'s sum
    // of `Runtime::send`'s return, the exact count of `AudioEvent`s handed
    // to `hosts.audio.send` — the closest observable point to the ring.
    assert_eq!(e.committed - before, 4, "exactly 4 AudioEvents committed");
    let got = peak(&left);
    let want = clean_cycle_peak("s [:bd :bd :bd :bd] > d1", 2.0);
    assert!(
        got <= want * 1.2,
        "no doubled amplitude: {got} vs one clean pass's {want}"
    );
}

#[test]
fn stack_twins_render_both_voices() {
    let mut e = primed();
    let before = e.committed;
    e.eval(&format!("stack [{{s :bd}} {{s :bd}}] > {LONG} > once"));
    let left = e.run_for(1.5);
    assert_eq!(e.committed - before, 2, "exactly 2 AudioEvents committed");
    let stack_peak = peak(&left);

    let mut single = primed();
    single.eval(&format!("s :bd > {LONG} > once"));
    let single_peak = peak(&single.run_for(1.5));

    // Two identical, simultaneous voices sum constructively, to ~2x, not
    // more (a third voice, say) and not merely "louder than one".
    assert!(
        stack_peak > single_peak * 1.5,
        "stack (~2x): {stack_peak} vs one voice {single_peak}"
    );
    assert!(
        stack_peak < single_peak * 2.5,
        "stack stays near 2x: {stack_peak} vs {single_peak}"
    );
}

#[test]
fn future_span_chop_renders_each_onset_once_in_both_query_orders() {
    let want = clean_cycle_peak("s :bd > chop 2 > d1", 1.2);
    for reversed in [false, true] {
        let mut e = primed();
        e.eval("s :bd > chop 2 > d1");
        let from = lane_from(&e, D1);
        // The whole [from, from + 1) splits into [from, from + 1/2) and
        // [from + 1/2, from + 1); two overlapping windows, each straddling
        // the split, in both orders: [from, from + 3/4) and
        // [from + 1/4, from + 1).
        let mut order = [
            span(from, add(from, r(3, 4))),
            span(add(from, r(1, 4)), add(from, r(1, 1))),
        ];
        if reversed {
            order.reverse();
        }
        let before = e.committed;
        for w in order {
            e.rt.stage_span(&mut e.ev, D1, w);
        }
        let left = render_from(&mut e, from, 1.2);
        let found = onsets(&left, 1.0e-4);
        assert_eq!(found.len(), 2, "reversed={reversed}: {found:?}");
        assert_eq!(
            e.committed - before,
            2,
            "reversed={reversed}: exactly 2 AudioEvents committed"
        );
        let got = peak(&left);
        assert!(
            got <= want * 1.2,
            "reversed={reversed}: no doubled amplitude: {got} vs {want}"
        );
    }
}

#[test]
fn slice_index_refresh_renders_only_the_new_region() {
    let mut full = primed();
    full.eval(&format!("s :bd > {LONG} > once"));
    let full_out = full.run_for(1.5);
    let (ff, _) = nonzero_run(&full_out, 1.0e-6).expect("the reference sounds");

    let mut e = primed();
    e.eval("var i 0");
    e.eval(&format!("s :bd > slice 2 i > {LONG} > d1"));
    let from = lane_from(&e, D1);
    let cycle = span(from, add(from, r(1, 1)));
    e.rt.stage_span(&mut e.ev, D1, cycle);
    e.ev.queue_upd("i", crate::value::value::Value::Int(1))
        .expect("upd");
    e.ev.run_pass();
    e.rt.invalidate(D1, cycle);
    e.rt.requery_dirty(&mut e.ev, D1);
    let left = render_from(&mut e, from, 2.0);

    // Only the refreshed occurrence plays: one voice, from [1/2, 1) of the
    // ramp (never the stale [0, 1/2) the index used to select).
    let runs = nonzero_runs(&left, 1.0e-6);
    assert_eq!(runs.len(), 1, "{runs:?}");
    let (s, en) = runs[0];
    let n = en - s + 1;
    assert!((n as i64 - 24_000).abs() <= 4, "half-sample region: {n}");
    for k in (0..n).step_by(1997) {
        let want = full_out[ff + N / 2 + k];
        let got = left[s + k];
        assert!(
            (got - want).abs() < want.abs() * 0.2 + 1.0e-5,
            "sample {k}: got {got}, want {want} (from [1/2, 1))"
        );
    }
}

#[test]
fn partition_invariance_incremental_staging_matches_one_shot() {
    let mut incremental = primed();
    incremental.eval(&format!("s :bd > chop 4 > {LONG} > d1"));
    let from = lane_from(&incremental, D1);
    let lead = (incremental.rt.clock().to_host(from) - incremental.clock.now()).max(0.0);
    let inc_full = incremental.run_for(lead + 4.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let skip = (lead * f64::from(super::SR)).round() as usize;
    let inc_out = &inc_full[skip.min(inc_full.len())..];
    let inc_energy: f32 = inc_out.iter().map(|v| v * v).sum();

    let mut staged = primed();
    staged.eval(&format!("s :bd > chop 4 > {LONG} > d1"));
    let from = lane_from(&staged, D1);
    staged
        .rt
        .stage_span(&mut staged.ev, D1, span(from, add(from, r(2, 1))));
    let staged_out = render_from(&mut staged, from, 4.0);
    let staged_energy: f32 = staged_out.iter().map(|v| v * v).sum();

    assert_eq!(
        onsets(inc_out, 1.0e-4).len(),
        onsets(&staged_out, 1.0e-4).len(),
        "the same number of voice starts either way"
    );
    assert!(
        (inc_energy - staged_energy).abs() < inc_energy.max(staged_energy) * 0.1,
        "incremental {inc_energy} vs one-shot-staged {staged_energy}"
    );
}

/// The cycle a slot's (freshly bound) lane starts at: binding always
/// lands on the next whole-cycle boundary at or after the bind time
/// (11.2), which by the time `primed()` has driven a render round trip is
/// rarely cycle 0.
fn lane_from(e: &E2e, slot: SlotKey) -> Ratio64 {
    e.rt.slots()
        .get(slot)
        .and_then(|s| s.lanes().first())
        .map_or(Ratio64::ZERO, |l| l.from)
}
