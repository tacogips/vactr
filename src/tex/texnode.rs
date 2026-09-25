//! Visual chain nodes (design 9.1). TASK-006.
//!
//! A `TexNode` mirrors `Pat`: Hydra operator chains build this tree on the
//! evaluator thread, and `VParam` mirrors `PParam` so late-bound vars,
//! tweak slots, signals, number patterns and functions of time all stay
//! live per frame (design 9.1).

use std::rc::Rc;

use crate::ns::namespace::VarSlotRef;
use crate::pattern::build::pattern_of;
use crate::pattern::eval::num_f64;
use crate::pattern::pat::Pat;
use crate::pattern::signal::Sig;
use crate::reader::span::Span;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

/// A visual chain node (design 9.1).
#[derive(Clone, Debug)]
pub struct TexNode {
    pub kind: TexKind,
    pub params: Box<[VParam]>,
    pub span: Option<Span>,
}

/// The visual-chain node kinds (design 9.1, `design-visual.md` vocabulary).
#[derive(Clone, Debug)]
pub enum TexKind {
    Osc,
    Noise,
    Voronoi,
    Shape,
    Gradient,
    Solid,
    /// `text "hello"`: the payload rasterizes to a texture at compile time
    /// (9.2).
    Text(Rc<str>),
    /// `src oN`: the previous frame of output `N`.
    SrcOut(OutId),
    Rotate,
    Scale,
    Pixelate,
    Tile,
    TileX,
    TileY,
    Kaleid,
    Scroll,
    Posterize,
    Shift,
    Invert,
    Contrast,
    Brightness,
    Luma,
    Thresh,
    Color,
    Saturate,
    Hue,
    Colorama,
    /// `add sub layer blend mult diff mask`: combines with another chain's
    /// color at the same coordinate.
    Blend(BlendOp, Rc<TexNode>),
    /// The `modulate` family: perturbs the coordinate using another
    /// chain's color.
    Modulate(ModKind, Rc<TexNode>),
    /// Left-to-right pipe (Hydra's `>`).
    Chain(Rc<TexNode>, Rc<TexNode>),
}

/// A blend combinator (design 9.1, `design-visual.md` "blend:").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BlendOp {
    Add,
    Sub,
    Layer,
    Blend,
    Mult,
    Diff,
    Mask,
}

/// A modulate combinator (design 9.1, `design-visual.md` "modulate:").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModKind {
    Modulate,
    Tile,
    Kaleid,
    Scroll,
    Rotate,
    Scale,
    Pixelate,
}

/// A visual-chain parameter: mirrors `PParam` for numbers (design 9.1).
#[derive(Clone, Debug)]
pub enum VParam {
    Const(f32),
    Late(VarSlotRef),
    Sig(Rc<Sig>),
    Pat(Rc<Pat>),
    Fn(Value),
}

impl VParam {
    /// The parameter for a value in a visual-chain parameter position: a
    /// number is `Const`, a var ref is `Late`, a callable is `Fn`, a signal
    /// is `Sig`, a pattern or list is `Pat`.
    ///
    /// # Errors
    /// `Type` for a value that is none of the above.
    pub fn from_value(v: &Value) -> Result<VParam, Failure> {
        if let Some(x) = num_f64(v) {
            return Ok(VParam::Const(x as f32));
        }
        match v {
            Value::VarRef(r) => Ok(VParam::Late(r.clone())),
            Value::Fn(_) | Value::Native(_) | Value::Thunk(_) => Ok(VParam::Fn(v.clone())),
            Value::Signal(s) => Ok(VParam::Sig(Rc::clone(s))),
            Value::Pattern(p) => Ok(VParam::Pat(Rc::clone(p))),
            Value::List(_) => Ok(VParam::Pat(pattern_of(v, None)?)),
            _ => Err(Failure::new(
                FailCode::Type,
                "a visual-chain parameter must be a number, signal, pattern or function of time",
            )),
        }
    }
}

impl TexNode {
    /// Builds a node from its kind, parameters and source span.
    #[must_use]
    pub fn new(kind: TexKind, params: Vec<VParam>, span: Option<Span>) -> Self {
        Self {
            kind,
            params: params.into_boxed_slice(),
            span,
        }
    }
}

/// Pipes `src` into `op` (Hydra's `>`, left-to-right): builds
/// `Chain(src, op)`.
#[must_use]
pub fn pipe(src: Rc<TexNode>, op: TexNode, span: Option<Span>) -> TexNode {
    TexNode::new(TexKind::Chain(src, Rc::new(op)), Vec::new(), span)
}

impl TexKind {
    /// The non-payload kind named by a Hydra/vactrol operator name
    /// (`design-visual.md`): sources, geometry and color operators. `text`,
    /// `src`, the blend family, the modulate family and the chain pipe are
    /// built directly since they carry extra data.
    #[must_use]
    pub fn from_name(name: &str) -> Option<TexKind> {
        Some(match name {
            "osc" => TexKind::Osc,
            "noise" => TexKind::Noise,
            "voronoi" => TexKind::Voronoi,
            "shape" => TexKind::Shape,
            "gradient" => TexKind::Gradient,
            "solid" => TexKind::Solid,
            "rotate" => TexKind::Rotate,
            "scale" => TexKind::Scale,
            "pixelate" => TexKind::Pixelate,
            "tile" => TexKind::Tile,
            "tile-x" => TexKind::TileX,
            "tile-y" => TexKind::TileY,
            "kaleid" => TexKind::Kaleid,
            "scroll" => TexKind::Scroll,
            "posterize" => TexKind::Posterize,
            "shift" => TexKind::Shift,
            "invert" => TexKind::Invert,
            "contrast" => TexKind::Contrast,
            "brightness" => TexKind::Brightness,
            "luma" => TexKind::Luma,
            "thresh" => TexKind::Thresh,
            "color" => TexKind::Color,
            "saturate" => TexKind::Saturate,
            "hue" => TexKind::Hue,
            "colorama" => TexKind::Colorama,
            _ => return None,
        })
    }
}

/// `subject > scale amount ..` (design 7.1.4's M1 subject overload): pipes
/// `subject` into a `Scale` geometry node.
#[must_use]
pub fn scale_texture(subject: Rc<TexNode>, params: Vec<VParam>, span: Option<Span>) -> TexNode {
    pipe(subject, TexNode::new(TexKind::Scale, params, span), span)
}

/// `shape sides ..` as a bare source node (no subject to pipe from).
#[must_use]
pub fn shape_source(params: Vec<VParam>, span: Option<Span>) -> TexNode {
    TexNode::new(TexKind::Shape, params, span)
}

id_newtype!(
    /// A visual output.
    OutId(u32)
);
