//! Namespaces, late-bound vars, tweak slots, packages and staged effects
//! (design 5.6, 5.7, 7.1.3, 13). TASK-005.

pub mod depgraph;
pub mod eval_doc;
pub mod evaluator;
pub mod fm6_sysex;
pub mod insts;
pub mod journal;
pub mod load;
pub mod namespace;
pub mod pkg;
pub mod stage;
pub mod tweak;

#[cfg(test)]
mod tests;
