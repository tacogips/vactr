//! `bus`/`master` chains and routing end to end (design 12.5, 12.8.6;
//! TASK-008 criterion 4), through the real `NativeAudioHost` ring.
//!
//! Routing (which bus a voice's output sums into) is serial repair R3:
//! `src/host/caps.rs`'s `InstResolver::bus`, `src/ns/insts.rs`'s
//! implementation of it over `InstRegistry`'s declared buses, and
//! `src/sched/commit.rs::audio_events` resolving the `bus` control (a
//! `CtlRoute::Scheduler` row, otherwise skipped) into the installed bus
//! id. A chain unit's parameters actually taking effect (`gain`,
//! `threshold`, ...) is serial repair R6: `src/dsp/build.rs` now keys
//! every effect parameter (bus/master chain units and inst-body effects
//! alike) with `effects::param_ctl` (the effect-local `EFFECT_PARAM_BASE +
//! index` space `effects::param_index`/`FxUnit::configure` read), never
//! the control-table/`EXTRA_CTL_BASE` ids `param_id` gives out — those
//! silently failed `param_index`'s lookup, so no chain parameter ever
//! left its built-in default, confirmed before R6 by a routed and an
//! unrouted voice rendering bit-identical peaks.

use super::E2e;

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0_f32, |m, v| m.max(v.abs()))
}

/// `10^(db / 20)`: the linear amplitude ratio of a dB gain.
fn db_ratio(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[test]
fn a_bus_definition_compiles_and_installs() {
    let mut e = E2e::new();
    // `eval` already asserts no drain fault by itself (it panics on one);
    // `e.faults` is filled only by ticking (`run_for`/`render_committed`),
    // so it is checked below, once a voice actually renders through the
    // bus, to catch a real runtime fault too, not just a compile one.
    e.eval("bus :drums:\n\tgain -18");
    let id = e
        .reg
        .borrow()
        .bus(crate::value::intern::intern_kw("drums"))
        .map(|b| b.id);
    assert!(id.is_some(), "registered under its name");

    e.eval("s :analog > note [:c4] > bus :drums > once");
    e.run_for(1.0);
    assert!(
        e.faults.is_empty(),
        "no faults once routed and rendered: {:?}",
        e.faults
    );
}

#[test]
fn a_routed_voice_is_measurably_quieter_than_unrouted() {
    let mut unrouted = E2e::new();
    unrouted.eval("s :analog > note [:c4] > once");
    let unrouted_peak = peak(&unrouted.run_for(1.0));

    let mut routed = E2e::new();
    routed.eval("bus :drums:\n\tgain -18");
    routed.eval("s :analog > note [:c4] > bus :drums > once");
    let routed_peak = peak(&routed.run_for(1.0));

    let ratio = routed_peak / unrouted_peak;
    let want = db_ratio(-18.0); // ~0.126
    assert!(
        (ratio - want).abs() < want * 0.3,
        "routed/unrouted peak ratio {ratio} vs -18 dB ({want})"
    );
}

#[test]
fn master_lowers_both_a_routed_and_an_unrouted_slot() {
    let mut baseline = E2e::new();
    baseline.eval("s :analog > note [:c4] > once");
    let base_peak = peak(&baseline.run_for(1.0));

    let mut unrouted = E2e::new();
    unrouted.eval("master:\n\tgain -12");
    unrouted.eval("s :analog > note [:c4] > once");
    let unrouted_peak = peak(&unrouted.run_for(1.0));

    let mut routed = E2e::new();
    routed.eval("master:\n\tgain -12");
    routed.eval("bus :drums:\n\tgain -6");
    routed.eval("s :analog > note [:c4] > bus :drums > once");
    let routed_peak = peak(&routed.run_for(1.0));

    let want_unrouted = db_ratio(-12.0); // ~0.251, master only
    let got_unrouted = unrouted_peak / base_peak;
    assert!(
        (got_unrouted - want_unrouted).abs() < want_unrouted * 0.3,
        "master -12 dB, unrouted: {got_unrouted} vs {want_unrouted}"
    );

    let want_routed = db_ratio(-12.0) * db_ratio(-6.0); // ~0.126, bus + master
    let got_routed = routed_peak / base_peak;
    assert!(
        (got_routed - want_routed).abs() < want_routed * 0.3,
        "bus -6 dB + master -12 dB, routed: {got_routed} vs {want_routed}"
    );
}

#[test]
fn a_bus_redefinition_swaps_to_the_new_gain_with_zero_allocation_and_no_dropout() {
    let mut e = E2e::new();
    e.eval("bus :drums:\n\tgain -18");
    // `legato 1` holds the ADSR gate open for the whole step, and a long
    // release keeps the tail well past the next retrigger: the routed
    // voice stays continuously audible, so a gap can only be the swap.
    e.eval("s :analog > note [:c4] > legato 1 > release 5 > bus :drums > d1");
    let before = e.run_for(1.0);
    let before_peak = peak(&before);
    assert!(before_peak > 1.0e-3, "sounds before the swap");

    // Redefines the chain while the slot keeps playing: the generation +
    // refcount lifecycle (design 12.5) retires the old chain once nothing
    // routes to it, without a gap in the routed voice's output. `run_for`
    // already asserts zero allocation on every render.
    e.eval("bus :drums:\n\tgain -6");
    let during = e.run_for(2.0);
    assert!(
        during.iter().all(|v| v.is_finite()),
        "finite across the swap"
    );
    // No silent gap: every 5 ms block (240 frames) has some signal.
    for block in during.chunks(240) {
        assert!(peak(block) > 0.0, "no dropout: a silent block");
    }
    assert_eq!(
        e.faults.len(),
        0,
        "no faults across the swap: {:?}",
        e.faults
    );

    // -18 dB -> -6 dB is a real level change: about 4x louder (12 dB).
    let after_peak = peak(&e.run_for(1.0));
    let ratio = after_peak / before_peak;
    assert!(
        ratio > db_ratio(12.0) * 0.7,
        "the new -6 dB chain is louder than the old -18 dB one: ratio {ratio}"
    );
}

#[test]
fn room_maps_onto_the_bus_unit_parameter() {
    let mut zero = E2e::new();
    zero.eval("s :analog > note [:c4] > room 0 > once");
    let zero_out = zero.run_for(1.0);

    let mut plain = E2e::new();
    plain.eval("s :analog > note [:c4] > once");
    let plain_out = plain.run_for(1.0);

    // `room 0` is transparent: close to the same signal as no `room` at
    // all (both default to 0 anyway, but this exercises the control path).
    let zero_peak = peak(&zero_out);
    let plain_peak = peak(&plain_out);
    assert!(
        (zero_peak - plain_peak).abs() < plain_peak * 0.2,
        "room 0 is transparent: {zero_peak} vs {plain_peak}"
    );

    let mut wet = E2e::new();
    wet.eval("s :analog > note [:c4] > room 0.3 > once");
    let wet_out = wet.run_for(1.0);
    assert!(wet_out.iter().all(|v| v.is_finite()));

    // `room 0.3` audibly changes the signal (the reverb tail extends the
    // energy well past where the dry voice's own envelope has decayed).
    let tail_energy = |s: &[f32]| -> f32 { s[s.len() - 4800..].iter().map(|v| v * v).sum() };
    assert!(
        tail_energy(&wet_out) > tail_energy(&zero_out) * 2.0,
        "room 0.3 leaves an audible tail: {} vs room 0's {}",
        tail_energy(&wet_out),
        tail_energy(&zero_out)
    );
}
