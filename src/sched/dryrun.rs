//! The bind-time dry run (design 11.5, 12.8.1).
//!
//! One full cycle of the new binding is queried in Query effect mode (the
//! caller passes a `VmQuery`, 10.4) against the current read view: global
//! writes, scheduling and host sends fail inside it instead of escaping,
//! and the pure hash RNG leaves no state behind, so neither a successful
//! nor a failed dry run changes the namespace, the slots, staging, pending
//! queues or any host. Captured `print` output stays in the returned
//! report. A texture binding is checked by compiling its chain.

use crate::pattern::eval::{InputCells, QueryCtx, QueryVm};
use crate::pattern::query::{query, QueryResult, TimeSpan};
use crate::sched::slots::Binding;
use crate::tex::shader::compile_tex;

/// Queries cycle 0 of `b` (a pattern) or compiles it (a texture).
pub fn dry_run(b: &Binding, vm: &mut dyn QueryVm, cells: &InputCells, seed: u64) -> QueryResult {
    match b {
        Binding::Pattern(p) => {
            let mut cx = QueryCtx::new(vm, cells, seed);
            let Ok(span) = TimeSpan::cycle(0) else {
                return QueryResult::default();
            };
            query(p, span, &mut cx)
        }
        Binding::Texture(t) => {
            let mut r = QueryResult::default();
            if let Err(f) = compile_tex(t) {
                r.faults.push(f);
            }
            r
        }
    }
}
