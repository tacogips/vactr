//! Session-side momentary override behavior (design 5.6).

use super::support::{offset, site_at, stales, Rig, DOC};
use crate::host::testing::AudioCall;
use crate::host::wire::CtlMsg;
use crate::ns::tweak::TweakId;
use crate::session::protocol::{
    ClientMsg, EvalBody, MomentaryBody, ServerMsg, StaleReason, WireNum, WireSpan, WireTier,
};

const SRC: &str = "s [:analog] > note [60] > lpf 800 > gain 0.2 > d1\n";

fn setup() -> (Rig, crate::session::protocol::EvalResultBody) {
    let mut rig = Rig::new();
    let (result, _) = rig.eval(SRC, 1, 1);
    rig.tick();
    (rig, result)
}

fn ramp_frames(rig: &Rig, ramp_ms: u32) -> u32 {
    let duration_ms = ramp_ms.max(5);
    (f64::from(duration_ms) * 0.001 * f64::from(rig.s.runtime().cfg.sample_rate)).round() as u32
}

fn send(
    rig: &mut Rig,
    site: &crate::session::protocol::WireSite,
    target: Option<WireNum>,
    ramp_ms: u32,
) -> Vec<ServerMsg> {
    rig.send(ClientMsg::Momentary(MomentaryBody {
        file: DOC.to_string(),
        id: site.id,
        form_gen: site.form_gen,
        edit_epoch: 1,
        target,
        ramp_ms,
    }))
}

fn eval_span(
    rig: &mut Rig,
    code: &str,
    revision: u64,
    start: u32,
    end: u32,
) -> crate::session::protocol::EvalResultBody {
    let out = rig.send(ClientMsg::Eval(EvalBody {
        file: DOC.to_string(),
        code: code.to_string(),
        span: Some(WireSpan::new(start, end)),
        doc_revision: revision,
        edit_epoch: 1,
    }));
    let Some(ServerMsg::EvalResult(result)) = out.first() else {
        panic!("span evaluation must return eval-result")
    };
    result.clone()
}

fn cell_ramps(rig: &Rig) -> Vec<(u32, u32, f32, u32, bool)> {
    rig.audio
        .calls()
        .into_iter()
        .filter_map(|(_, call)| match call {
            AudioCall::Post(CtlMsg::CellRamp {
                cell,
                epoch,
                target,
                frames,
                release,
                ..
            }) => Some((cell.get(), epoch, target, frames, release)),
            _ => None,
        })
        .collect()
}

fn encoded(rig: &Rig, cell: u32, value: i32) -> f32 {
    rig.s
        .runtime()
        .cells()
        .encode_for(
            crate::dsp::cells::CellId::new(cell),
            &crate::value::value::Value::Int(value),
        )
        .expect("fed control cell encodes integer value")
}

fn slot_for(
    rig: &Rig,
    site: &crate::session::protocol::WireSite,
) -> crate::ns::namespace::VarSlotRef {
    rig.s
        .evaluator()
        .ns()
        .tweaks()
        .borrow()
        .get(TweakId::new(site.id))
        .expect("site")
        .slot
        .clone()
}

#[test]
fn direct_drag_ramps_cells_without_writing_the_tweak_slot() {
    let (mut rig, result) = setup();
    let site = site_at(&result, SRC, "800").clone();
    let slot = slot_for(&rig, &site);
    let version = slot.version();
    let before = slot.base();
    let fed_cells = rig.s.runtime().cells().site_cells(slot.id());
    assert!(
        !fed_cells.is_empty(),
        "lpf site must feed at least one cell"
    );
    let ramp_count = cell_ramps(&rig).len();
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    let ramps = cell_ramps(&rig);
    let drag_ramps = &ramps[ramp_count..];
    assert!(!drag_ramps.is_empty(), "lpf feeds at least one cell");
    for (cell, epoch, _, _, _) in &fed_cells {
        assert!(drag_ramps
            .iter()
            .any(|(ramp_cell, ramp_epoch, value, frames, release)| {
                *ramp_cell == cell.get()
                    && ramp_epoch == epoch
                    && *value == encoded(&rig, cell.get(), 1200)
                    && *frames == ramp_frames(&rig, 30)
                    && !release
            }));
    }
    assert_eq!(slot.base().to_string(), before.to_string());
    assert_eq!(slot.version(), version);
    rig.clock.set(0.02);
    rig.tick();
    let expected = (800.0_f64 + (1200.0 - 800.0) * (0.02 / 0.03)).round() as i32;
    assert_eq!(slot.get().to_string(), expected.to_string());
    assert_eq!(slot.base().to_string(), "800");
}

#[test]
fn release_glides_back_to_base_and_removes_the_entry() {
    let (mut rig, result) = setup();
    let site = site_at(&result, SRC, "800").clone();
    let slot = slot_for(&rig, &site);
    let fed_cells = rig.s.runtime().cells().site_cells(slot.id());
    assert!(
        !fed_cells.is_empty(),
        "lpf site must feed at least one cell"
    );
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    rig.clock.set(0.02);
    rig.tick();
    let ramp_count = cell_ramps(&rig).len();
    assert!(send(&mut rig, &site, None, 1000).is_empty());
    let ramps = cell_ramps(&rig);
    let release_ramps = &ramps[ramp_count..];
    assert!(
        !release_ramps.is_empty(),
        "release must target base for one second"
    );
    for (cell, epoch, _, _, _) in &fed_cells {
        assert!(release_ramps
            .iter()
            .any(|(ramp_cell, ramp_epoch, value, frames, release)| {
                *ramp_cell == cell.get()
                    && ramp_epoch == epoch
                    && *value == encoded(&rig, cell.get(), 800)
                    && *frames == ramp_frames(&rig, 1000)
                    && *release
            }));
    }
    rig.clock.set(1.03);
    rig.tick();
    assert_eq!(rig.s.momentary.len(), 0);
    assert_eq!(slot.get().to_string(), "800");
    let ramp_count_after_reap = cell_ramps(&rig).len();
    rig.tick();
    assert_eq!(cell_ramps(&rig).len(), ramp_count_after_reap);
}

#[test]
fn snap_release_uses_the_minimum_five_millisecond_ramp() {
    let (mut rig, result) = setup();
    let site = site_at(&result, SRC, "800").clone();
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    assert!(send(&mut rig, &site, None, 0).is_empty());
    assert!(cell_ramps(&rig)
        .iter()
        .any(|(_, _, _, frames, release)| *frames == ramp_frames(&rig, 0) && *release));
}

#[test]
fn partial_span_eval_preserves_a_held_entry_in_an_untouched_form() {
    let mut rig = Rig::new();
    let form_a = "s [:analog] > note [60] > lpf 800 > gain 0.2 > d1\n";
    let form_b = "s [:analog] > note [60] > lpf 900 > gain 0.2 > d1\n";
    let src = format!("{form_a}{form_b}");
    let (result, _) = rig.eval(&src, 1, 1);
    rig.tick();
    let site_b = site_at(&result, &src, "900").clone();
    let slot_b = slot_for(&rig, &site_b);
    let cells_b: Vec<_> = rig
        .s
        .runtime()
        .cells()
        .site_cells(slot_b.id())
        .into_iter()
        .map(|(cell, epoch, _, _, _)| (cell.get(), epoch))
        .collect();
    assert!(!cells_b.is_empty(), "form B site must feed cells");
    assert!(send(&mut rig, &site_b, Some(WireNum::Int(1300)), 30).is_empty());
    rig.clock.set(0.03);
    rig.tick();
    let ramp_count = cell_ramps(&rig).len();

    let selected = eval_span(
        &mut rig,
        &src,
        2,
        0,
        u32::try_from(form_a.len()).expect("form span fits u32"),
    );
    assert_eq!(selected.forms.len(), 1, "only form A is evaluated");
    rig.tick();
    assert_eq!(rig.s.momentary.len(), 1);
    assert_eq!(slot_b.get().to_string(), "1300");
    let ramps = cell_ramps(&rig);
    let after_eval = &ramps[ramp_count..];
    assert!(!after_eval.iter().any(|(cell, epoch, _, frames, release)| {
        cells_b.contains(&(*cell, *epoch)) && *frames == 0 && *release
    }));

    let release_start = ramps.len();
    assert!(send(&mut rig, &site_b, None, 1000).is_empty());
    let ramps = cell_ramps(&rig);
    let release_ramps = &ramps[release_start..];
    for (cell, epoch) in cells_b {
        assert!(release_ramps
            .iter()
            .any(|(ramp_cell, ramp_epoch, target, frames, release)| {
                *ramp_cell == cell
                    && *ramp_epoch == epoch
                    && *target == encoded(&rig, cell, 900)
                    && *frames == ramp_frames(&rig, 1000)
                    && *release
            }));
    }
}

#[test]
fn partial_span_eval_preserves_a_releasing_entry_until_its_glide_end() {
    let mut rig = Rig::new();
    let form_a = "s [:analog] > note [60] > lpf 800 > gain 0.2 > d1\n";
    let form_b = "s [:analog] > note [60] > lpf 900 > gain 0.2 > d1\n";
    let src = format!("{form_a}{form_b}");
    let (result, _) = rig.eval(&src, 1, 1);
    rig.tick();
    let site_b = site_at(&result, &src, "900").clone();
    let slot_b = slot_for(&rig, &site_b);
    let cells_b: Vec<_> = rig
        .s
        .runtime()
        .cells()
        .site_cells(slot_b.id())
        .into_iter()
        .map(|(cell, epoch, _, _, _)| (cell.get(), epoch))
        .collect();
    assert!(!cells_b.is_empty(), "form B site must feed cells");
    assert!(send(&mut rig, &site_b, Some(WireNum::Int(1300)), 30).is_empty());
    rig.clock.set(0.03);
    rig.tick();
    assert!(send(&mut rig, &site_b, None, 1000).is_empty());
    rig.clock.set(0.2);
    rig.tick();
    let ramp_count = cell_ramps(&rig).len();

    let selected = eval_span(
        &mut rig,
        &src,
        2,
        0,
        u32::try_from(form_a.len()).expect("form span fits u32"),
    );
    assert_eq!(selected.forms.len(), 1, "only form A is evaluated");
    rig.tick();
    assert_eq!(rig.s.momentary.len(), 1);
    assert_ne!(slot_b.get().to_string(), "900", "glide remains in flight");
    let ramps = cell_ramps(&rig);
    let after_eval = &ramps[ramp_count..];
    assert!(!after_eval.iter().any(|(cell, epoch, _, frames, release)| {
        cells_b.contains(&(*cell, *epoch)) && *frames == 0 && *release
    }));

    rig.clock.set(1.02);
    rig.tick();
    assert_eq!(rig.s.momentary.len(), 1);
    assert_ne!(slot_b.get().to_string(), "900", "glide has not ended yet");

    rig.clock.set(1.04);
    rig.tick();
    assert_eq!(rig.s.momentary.len(), 0);
    assert_eq!(slot_b.get().to_string(), "900");
}

#[test]
fn stale_drags_drop_silently_but_releases_resolve_the_previous_id() {
    let mut rig = Rig::new();
    let src = "var cutoff 800\ns [:analog] > note [60] > lpf cutoff > gain 0.2 > d1\n";
    let (result, _) = rig.eval(src, 1, 1);
    rig.tick();
    let old = site_at(&result, src, "800").clone();
    assert_eq!(old.origin, crate::session::protocol::WireOrigin::Binding);
    assert!(send(&mut rig, &old, Some(WireNum::Int(1200)), 30).is_empty());
    rig.clock.set(0.03);
    rig.tick();
    let (next, _) = rig.eval(src, 2, 1);
    let new = site_at(&next, src, "800").clone();
    assert_ne!(old.id, new.id);
    assert_eq!(new.origin, crate::session::protocol::WireOrigin::Binding);
    let new_slot = slot_for(&rig, &new);
    assert_eq!(new_slot.get().to_string(), "1200");
    assert_eq!(new_slot.base().to_string(), "800");
    let current_cells: Vec<_> = rig
        .s
        .runtime()
        .cells()
        .site_cells(new_slot.id())
        .into_iter()
        .map(|(cell, epoch, _, _, _)| (cell.get(), epoch))
        .collect();
    assert!(!current_cells.is_empty(), "re-keyed site must feed cells");
    let before_ramps = cell_ramps(&rig).len();
    assert!(send(&mut rig, &old, Some(WireNum::Int(1500)), 30).is_empty());
    assert_eq!(new_slot.get().to_string(), "1200");
    assert_eq!(cell_ramps(&rig).len(), before_ramps);
    let release_start = cell_ramps(&rig).len();
    assert!(send(&mut rig, &old, None, 1000).is_empty());
    let ramps = cell_ramps(&rig);
    let release_ramps = &ramps[release_start..];
    assert!(
        !release_ramps.is_empty(),
        "old-id release must ramp current cells"
    );
    for (cell, epoch) in &current_cells {
        assert!(release_ramps
            .iter()
            .any(|(ramp_cell, ramp_epoch, value, frames, release)| {
                ramp_cell == cell
                    && ramp_epoch == epoch
                    && *value == encoded(&rig, *cell, 800)
                    && *frames == ramp_frames(&rig, 1000)
                    && *release
            }));
    }
    rig.clock.set(1.04);
    rig.tick();
    assert_eq!(rig.s.momentary.len(), 0);
    assert_eq!(new_slot.get().to_string(), "800");
}

#[test]
fn stale_form_generation_and_unknown_release_have_no_reply_or_ramp() {
    let (mut rig, result) = setup();
    let mut site = site_at(&result, SRC, "800").clone();
    site.form_gen += 1;
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    assert!(cell_ramps(&rig).is_empty());
    let unknown = crate::session::protocol::WireSite { id: 9999, ..site };
    assert!(send(&mut rig, &unknown, None, 1000).is_empty());
    assert_eq!(rig.s.momentary.len(), 0);
    assert!(cell_ramps(&rig).is_empty());
}

#[test]
fn edit_invalidated_site_still_accepts_safe_release_by_old_identity() {
    let (mut rig, result) = setup();
    let site = site_at(&result, SRC, "800").clone();
    let slot = slot_for(&rig, &site);
    let fed_cells = rig.s.runtime().cells().site_cells(slot.id());
    assert!(
        !fed_cells.is_empty(),
        "lpf site must feed at least one cell"
    );
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    let at = offset(SRC, "800", 0);
    rig.doc_changed(2, 1, &[(at, at + 3, 3)], &[(at, at + 3)], 2);
    let ramp_count = cell_ramps(&rig).len();
    assert!(send(&mut rig, &site, None, 0).is_empty());
    let ramps = cell_ramps(&rig);
    let release_ramps = &ramps[ramp_count..];
    for (cell, epoch, _, _, _) in fed_cells {
        assert!(release_ramps
            .iter()
            .any(|(ramp_cell, ramp_epoch, target, frames, release)| {
                *ramp_cell == cell.get()
                    && *ramp_epoch == epoch
                    && *target == encoded(&rig, cell.get(), 800)
                    && *frames == ramp_frames(&rig, 0)
                    && *release
            }));
    }
}

#[test]
fn ineligible_site_replies_without_posting_control() {
    let mut rig = Rig::new();
    let src = "s :analog > note [60] > gain {* 0.5 2} > d1\n";
    let (result, _) = rig.eval(src, 1, 1);
    let site = site_at(&result, src, "0.5");
    let out = send(&mut rig, site, Some(WireNum::Int(1200)), 30);
    assert_eq!(stales(&out)[0].reason, StaleReason::MomentaryIneligible);
    assert!(cell_ramps(&rig).is_empty());
    let src = "[{s :analog > note [60] > gain 0.5 > d1} {s :analog > once}]\n";
    let (manual, _) = rig.eval(src, 2, 1);
    let site = site_at(&manual, src, "0.5");
    assert_eq!(site.tier, WireTier::Manual);
    let out = send(&mut rig, site, Some(WireNum::Float(0.9)), 30);
    assert_eq!(stales(&out)[0].reason, StaleReason::MomentaryIneligible);
}

#[test]
fn capacity_rejects_the_seventeenth_direct_entry() {
    let mut rig = Rig::new();
    let mut src = String::from("s [:analog] > note [60]");
    for value in 800..817 {
        src.push_str(&format!(" > lpf {value}"));
    }
    src.push_str(" > d1\n");
    let (result, _) = rig.eval(&src, 1, 1);
    assert!(
        result.forms.iter().all(|form| form.failure.is_none()),
        "the synth form must evaluate: {:?}",
        result.forms
    );
    let sites: Vec<_> = (800..817)
        .map(|value| site_at(&result, &src, &value.to_string()))
        .collect();
    assert_eq!(sites.len(), 17, "fixture provides 17 direct LPF sites");
    assert!(sites.iter().all(|site| site.tier == WireTier::Direct));
    for site in sites.iter().take(16) {
        assert!(send(&mut rig, site, Some(WireNum::Float(900.0)), 30).is_empty());
    }
    let out = send(&mut rig, sites[16], Some(WireNum::Float(900.0)), 30);
    assert_eq!(stales(&out)[0].reason, StaleReason::MomentaryCapacity);
}

#[test]
fn binding_origin_sites_ramp_the_var_fed_cell() {
    let mut rig = Rig::new();
    let src = "var cutoff 800\ns [:analog] > note [60] > lpf cutoff > gain 0.2 > d1\n";
    let (result, _) = rig.eval(src, 1, 1);
    rig.tick();
    let site = site_at(&result, src, "800").clone();
    assert_eq!(site.origin, crate::session::protocol::WireOrigin::Binding);
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    assert!(
        !cell_ramps(&rig).is_empty(),
        "the var's fed lpf cell receives the ramp"
    );
}

#[test]
fn integer_inst_control_cells_use_frames_zero_ramps() {
    let mut rig = Rig::new();
    let src = "inst typed-cutoff cutoff: int = 100:\n\tsin-osc {* freq cutoff}\ns :typed-cutoff > cutoff 100 > once\n";
    let (result, _) = rig.eval(src, 1, 1);
    assert!(
        result.forms.iter().all(|form| form.failure.is_none()),
        "integer control fixture must evaluate: {:?}",
        result.forms
    );
    rig.tick();
    let site = site_at(&result, src, "100").clone();
    assert_eq!(
        site.origin,
        crate::session::protocol::WireOrigin::InstDefault
    );
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    let ramps = cell_ramps(&rig);
    assert!(
        !ramps.is_empty(),
        "integer default must feed an external cell"
    );
    assert!(
        ramps.iter().all(|(_, _, target, frames, release)| {
            *frames == 0 && target.fract() == 0.0 && !release
        }),
        "integer control cells must step to integral values: {ramps:?}"
    );
}

#[test]
fn stepped_glide_release_ends_with_a_drop_at_base() {
    let mut rig = Rig::new();
    let src = "inst typed-cutoff cutoff: int = 100:\n\tsin-osc {* freq cutoff}\ns :typed-cutoff > cutoff 100 > once\n";
    let (result, _) = rig.eval(src, 1, 1);
    assert!(
        result.forms.iter().all(|form| form.failure.is_none()),
        "integer control fixture must evaluate: {:?}",
        result.forms
    );
    rig.tick();
    let site = site_at(&result, src, "100").clone();
    assert_eq!(
        site.origin,
        crate::session::protocol::WireOrigin::InstDefault
    );

    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    let held = cell_ramps(&rig);
    let mut cells = Vec::new();
    for (cell, epoch, _, frames, release) in held {
        assert_eq!(frames, 0, "integer control ramps must be stepped");
        assert!(!release, "the initial tweak is a held value");
        if !cells.contains(&(cell, epoch)) {
            cells.push((cell, epoch));
        }
    }
    assert!(
        !cells.is_empty(),
        "integer default must feed external cells"
    );

    rig.clock.set(0.02);
    rig.tick();
    let release_start = cell_ramps(&rig).len();
    assert!(send(&mut rig, &site, None, 1000).is_empty());
    assert_eq!(
        cell_ramps(&rig).len(),
        release_start,
        "a glide release must not drop stepped cells at its start"
    );

    for step in 1..=70 {
        rig.clock.set(0.02 + f64::from(step) * 0.016);
        rig.tick();
    }
    assert_eq!(rig.s.momentary.len(), 0, "release entry must be reaped");

    let release_ramps = cell_ramps(&rig);
    let release_ramps = &release_ramps[release_start..];
    for (cell, epoch) in cells {
        let base_target = rig
            .s
            .evaluator()
            .insts()
            .expect("instrument registry is installed")
            .borrow()
            .entries()
            .find_map(|inst| inst.default_cell_value(crate::dsp::cells::CellId::new(cell)))
            .expect("inst-default cell encodes its base value");
        let posts: Vec<_> = release_ramps
            .iter()
            .filter(|(posted_cell, posted_epoch, _, _, _)| {
                *posted_cell == cell && *posted_epoch == epoch
            })
            .collect();
        assert!(!posts.is_empty(), "release must step cell {cell}:{epoch}");
        assert!(
            posts.iter().all(|(_, _, _, frames, _)| *frames == 0),
            "stepped release posts must use frames=0: {posts:?}"
        );
        let last = posts.last().expect("release posts are nonempty");
        assert_eq!(last.2, base_target, "cell {cell}:{epoch} must end at base");
        assert!(last.4, "the final post for cell {cell}:{epoch} must drop");
        let first_drop = posts
            .iter()
            .position(|(_, _, target, frames, release)| {
                *target == base_target && *frames == 0 && *release
            })
            .expect("release must drop at the encoded base");
        assert!(
            posts[first_drop + 1..]
                .iter()
                .all(|(_, _, _, _, release)| *release),
            "no held post may follow the base drop: {posts:?}"
        );
    }
}

#[test]
fn releasing_entry_rebases_to_a_new_literal_base() {
    let mut rig = Rig::new();
    let src = "var cutoff 800\ns [:analog] > note [60] > lpf cutoff > gain 0.2 > d1\n";
    let (result, _) = rig.eval(src, 1, 1);
    rig.tick();
    let old = site_at(&result, src, "800").clone();
    let old_slot = slot_for(&rig, &old);
    let fed_cells = rig.s.runtime().cells().site_cells(old_slot.id());
    assert!(!fed_cells.is_empty(), "binding-origin site must feed cells");
    assert!(send(&mut rig, &old, Some(WireNum::Int(1200)), 30).is_empty());
    assert!(send(&mut rig, &old, None, 1000).is_empty());
    rig.clock.set(0.2);
    rig.tick();
    let ramp_count = cell_ramps(&rig).len();
    let changed = src.replace("800", "600");
    let (next, _) = rig.eval(&changed, 2, 1);
    rig.tick();
    let new = site_at(&next, &changed, "600").clone();
    let new_slot = slot_for(&rig, &new);
    let current_cells = rig.s.runtime().cells().site_cells(new_slot.id());
    assert!(
        !current_cells.is_empty(),
        "rebased site must feed current cells"
    );
    let ramps = cell_ramps(&rig);
    let rebased_ramps = &ramps[ramp_count..];
    assert!(
        !rebased_ramps.is_empty(),
        "rebase posts remaining release ramps"
    );
    let remaining_frames = (0.8_f64 * f64::from(rig.s.runtime().cfg.sample_rate)).round() as u32;
    for (cell, epoch, _, _, _) in current_cells {
        assert!(rebased_ramps
            .iter()
            .any(|(ramp_cell, ramp_epoch, target, frames, release)| {
                *ramp_cell == cell.get()
                    && *ramp_epoch == epoch
                    && *target == encoded(&rig, cell.get(), 600)
                    && *frames == remaining_frames
                    && *release
            }));
    }
    assert_eq!(rig.s.momentary.len(), 1);
    assert_eq!(new_slot.base().to_string(), "600");
}

#[test]
fn document_value_and_version_survive_hush() {
    let (mut rig, result) = setup();
    let site = site_at(&result, SRC, "800").clone();
    let slot = slot_for(&rig, &site);
    let version = slot.version();
    let fed_cells = rig.s.runtime().cells().site_cells(slot.id());
    assert!(
        !fed_cells.is_empty(),
        "lpf site must feed at least one cell"
    );
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    let ramp_count = cell_ramps(&rig).len();
    rig.send(ClientMsg::Hush(crate::session::protocol::Empty {}));
    let ramps = cell_ramps(&rig);
    let drop_ramps = &ramps[ramp_count..];
    for (cell, epoch, _, _, _) in fed_cells {
        assert!(drop_ramps
            .iter()
            .any(|(ramp_cell, ramp_epoch, _, frames, release)| {
                *ramp_cell == cell.get() && *ramp_epoch == epoch && *frames == 0 && *release
            }));
    }
    assert_eq!(slot.get().to_string(), "800");
    assert_eq!(slot.base().to_string(), "800");
    assert_eq!(slot.version(), version);
    assert_eq!(rig.s.momentary.len(), 0);
}

#[test]
fn hush_evaluated_as_code_clears_the_momentary_table_on_tick() {
    let (mut rig, result) = setup();
    let site = site_at(&result, SRC, "800").clone();
    assert!(send(&mut rig, &site, Some(WireNum::Int(1200)), 30).is_empty());
    let (hush, _) = rig.eval("hush", 2, 1);
    assert!(hush.forms.iter().all(|form| form.failure.is_none()));
    rig.tick();
    assert_eq!(rig.s.momentary.len(), 0);
}
