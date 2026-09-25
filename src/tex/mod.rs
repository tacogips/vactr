//! Visual chains (design section 9). TASK-006.

pub mod shader;
pub mod texnode;
pub mod uniforms;

pub use shader::{compile_tex, ShaderDesc, TextAsset};
pub use texnode::{BlendOp, ModKind, OutId, TexKind, TexNode, VParam};
pub use uniforms::{resolve_uniforms, UniformPlan, UniformSpec, Uniforms};

#[cfg(test)]
mod tests;
