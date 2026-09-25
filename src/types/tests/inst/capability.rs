//! Capability checks at realization (12.8.8): constant grain limits are
//! checked against the registry's tier with `CapabilitySet::require`; the
//! definition still installs (the audio side clamps, 12.6).

use crate::dsp::caps::{Cap, CapabilitySet};
use crate::ns::insts::InstRegistry;
use crate::types::diag::DiagCode;
use crate::value::intern::intern_kw;

use super::Session;

#[test]
fn grain_limits_beyond_the_tier_are_diagnostics() {
    let reg = InstRegistry::shared();
    reg.borrow_mut().caps = CapabilitySet::browser();
    let mut s = Session::with(reg);
    let out = s.eval("inst g src: keyword = :pad:\n\tgranular src size: 0.9 density: 500");
    let o = out.last().expect("a form");
    assert!(o.value.is_ok(), "{:?}", o.value);
    let over: Vec<_> = o
        .diags
        .iter()
        .filter(|d| d.code == DiagCode::BeyondCapability)
        .collect();
    assert_eq!(over.len(), 2, "{:?}", o.diags);
    assert!(s.reg.borrow().id_of(intern_kw("g")).is_some());
    // Within the native tier there is nothing to report.
    let mut s = Session::new();
    let out = s.eval("inst g src: keyword = :pad:\n\tgranular src size: 0.9 density: 500");
    assert!(out.last().expect("a form").diags.is_empty());
}

#[test]
fn offline_render_is_not_available_in_the_browser() {
    let d = CapabilitySet::browser()
        .require(Cap::OfflineRender, None)
        .expect_err("refused");
    assert_eq!(d.code, DiagCode::BeyondCapability);
    assert!(d.message.contains("not available"), "{}", d.message);
}
