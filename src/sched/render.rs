//! Per-frame uniform resolution (design 9.2-9.3, ED-WIRE item 4 / G5).
//!
//! `Runtime::activate` (`runtime.rs`) keeps the evaluator-owned
//! `UniformPlan` `compile_tex` returns for each output, one per `o0`..`o3`.
//! `render_frame` resolves every stored plan at the current frame position
//! (9.3) and ships the resulting `Uniforms` through
//! `RenderHost::set_uniforms` — the render side never sees a closure, only
//! plain `f32` values. A failing uniform keeps the render host's previous
//! value and is reported as a failure; an output whose slot is no longer
//! bound (hush/stop already sent the empty program through
//! `sched/control.rs`, or the slot was removed) has its stored plan
//! cleared and receives no `set_uniforms` call this frame.

use crate::ns::evaluator::Evaluator;
use crate::pattern::eval::QueryCtx;
use crate::sched::runtime::{grid, Runtime};
use crate::tex::texnode::OutId;
use crate::tex::uniforms::{resolve_uniforms, Uniforms};
use crate::vm::fail::Failure;
use crate::vm::query_vm::VmQuery;

impl Runtime {
    /// Resolves every output's stored `UniformPlan` at `host_now` and sends
    /// the result to the render host (design 9.3).
    ///
    /// The frame position is derived from `host_now` the same way `tick`'s
    /// step (1) derives the scheduler position: `grid` of the clock's
    /// cycles at `host_now`, never moving behind the scheduler's own
    /// position. Each spec is resolved exactly once (`resolve_uniforms`);
    /// a failing plan is reported and its output keeps whatever the render
    /// host already has (no `set_uniforms` call for it this frame).
    pub fn render_frame(&mut self, ev: &mut Evaluator, host_now: f64) -> Vec<Failure> {
        let mut faults = Vec::new();
        let frame_time = grid(self.clock.to_cycles(host_now), false).max(self.pos);
        let (vm, ns) = ev.vm_and_ns();
        let mut h = VmQuery::new(vm, ns);
        let mut cx = QueryCtx::new(&mut h, &self.input, self.cfg.seed);
        cx.tempo = self.clock.tempo();
        for idx in 0..self.uniform_plans.len() {
            let Some(plan) = self.uniform_plans[idx].clone() else {
                continue;
            };
            let out = OutId::new(u32::try_from(idx).unwrap_or(0));
            let bound = self
                .slots
                .iter()
                .any(|s| s.bound.is_some() && s.out_id() == Some(out));
            if !bound {
                self.uniform_plans[idx] = None;
                continue;
            }
            match resolve_uniforms(&plan, frame_time, &mut cx) {
                Ok(resolved) => {
                    let uniforms = Uniforms::from_resolved(&resolved);
                    self.hosts.render.set_uniforms(out, &uniforms);
                }
                Err(f) => faults.push(f),
            }
        }
        faults
    }
}
