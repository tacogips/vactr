//! Session layer (design 14, 14.5).
//!
//! - `session`: the `Session` (one `Evaluator` + one `Runtime`), its config,
//!   document state and routing.
//! - `eval`: the document pipeline (14.5.4) and LSP-only `analyze`.
//! - `publish`: the ONE `bindings` batch per completed pass (14.5.5), the
//!   wire forms and the tick's telemetry.
//! - `authority`: edit epochs, invalidation and write validation (14.5.6).
//! - `protocol`, `codec`: the v1 messages and their JSON codec.
//! - `console`, `repl`: console registers and the REPL line loop (14.5.10).
//! - `changes`: editor change sets (SS-CONTRACTS).
//! - `editors`: the `manifest.editors` table (TASK-010 G2).
//! - `frontend`: additive `Session` methods for the editor: `render_frame`
//!   (G5), `drive_packages` and `check` (G6).

pub mod authority;
pub mod changes;
pub mod codec;
pub mod console;
pub mod editors;
pub mod eval;
mod frontend;
pub mod protocol;
pub mod publish;
pub mod repl;
#[allow(clippy::module_inception)]
pub mod session;
pub mod song;

pub use eval::{alias_env_for, analyze, Analysis, EvalOutcome, FormResult, PackageView};
pub use protocol::{ClientMsg, Envelope, ServerMsg};
pub use repl::run_repl;
pub use session::{Dest, Outgoing, PersistenceMode, Session, SessionConfig};

#[cfg(test)]
mod tests;
