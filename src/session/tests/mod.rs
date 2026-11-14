//! SS-SESSION tests (design 14.5; vactr-session-core.md "Required
//! Tests"): the codec, the eval pipeline, reactive publication (every
//! TASK-005 trace), write authority, tiers, packages through the session,
//! directives, the REPL and the self-analysis surfaces.

mod analysis;
mod authority;
mod codec;
mod directives;
mod editor_wire;
mod eval;
mod live_stop;
mod momentary;
mod packages;
mod publish;
mod repl;
mod support;
mod tiers;
