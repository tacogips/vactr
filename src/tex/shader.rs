//! Shader compilation: a `TexNode` chain to GLSL ES 3.0 source, from a
//! fixed per-operator snippet library (design 9.2, the Hydra model).
//! TASK-006.

use std::collections::BTreeSet;

use crate::tex::texnode::{BlendOp, ModKind, TexKind, TexNode};
use crate::tex::uniforms::{UniformPlan, UniformSpec};
use crate::vm::fail::{FailCode, Failure};

/// Chain nesting (`Chain`, `Blend`, `Modulate` recursion) bound (design
/// 9.2's "depth 64" test).
const MAX_DEPTH: u32 = 64;
/// Generated-source size bound.
const MAX_SOURCE_LEN: usize = 1_048_576;

/// A render-safe shader descriptor: plain data only, no `Value`, no
/// `VParam`, no evaluator-owned `Rc` (design 9.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShaderDesc {
    pub source: String,
    pub uniform_names: Box<[String]>,
    pub assets: Box<[TextAsset]>,
}

/// A non-numeric source payload (`text "hello"`) the host rasterizes to a
/// texture (design 9.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextAsset {
    pub id: u32,
    pub text: String,
}

/// A fixed order for the emitted GLSL functions, independent of the order
/// operators are encountered in the chain (design 9.2 "in a fixed
/// deterministic order").
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum OpTag {
    Osc,
    Noise,
    Voronoi,
    Shape,
    Gradient,
    Solid,
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
    BlendAdd,
    BlendSub,
    BlendLayer,
    BlendBlend,
    BlendMult,
    BlendDiff,
    BlendMask,
    ModModulate,
    ModTile,
    ModKaleid,
    ModScroll,
    ModRotate,
    ModScale,
    ModPixelate,
}

/// Compile-time codegen state: uniforms in encounter order, text assets,
/// the samplers to declare, and the GLSL functions used so far.
#[derive(Default)]
struct Ctx {
    uniforms: Vec<UniformSpec>,
    uniform_names: Vec<String>,
    assets: Vec<TextAsset>,
    samplers: Vec<String>,
    used: BTreeSet<OpTag>,
}

/// Compiles a visual chain to GLSL ES 3.0 fragment-shader source and its
/// evaluator-owned uniform plan (design 9.2).
///
/// # Errors
/// `Type` when the chain does not start with a source, or a parameter is
/// not a finite number; `Arity` for too many parameters on one operator;
/// `DepthExceeded` for a chain nested over 64 deep; `Overflow` for
/// generated source over 1 MiB.
pub fn compile_tex(t: &TexNode) -> Result<(ShaderDesc, UniformPlan), Failure> {
    let mut ctx = Ctx::default();
    let color_expr = compile_chain(t, "st", &mut ctx, 0)?;

    let mut src = String::new();
    src.push_str("#version 300 es\n");
    src.push_str("precision highp float;\n");
    src.push_str("uniform float time;\n");
    src.push_str("uniform vec2 resolution;\n");
    for name in &ctx.uniform_names {
        src.push_str(&format!("uniform float {name};\n"));
    }
    for sampler in &ctx.samplers {
        src.push_str(&format!("uniform sampler2D {sampler};\n"));
    }
    src.push_str("out vec4 fragColor;\n");
    for tag in &ctx.used {
        src.push_str(glsl_def(*tag));
        src.push('\n');
    }
    src.push_str("void main() {\n");
    src.push_str("  vec2 st = gl_FragCoord.xy / resolution.xy;\n");
    src.push_str(&format!("  fragColor = {color_expr};\n"));
    src.push_str("}\n");

    if src.len() > MAX_SOURCE_LEN {
        return Err(Failure::new(
            FailCode::Overflow,
            "generated shader source too large",
        ));
    }

    let desc = ShaderDesc {
        source: src,
        uniform_names: ctx.uniform_names.into_boxed_slice(),
        assets: ctx.assets.into_boxed_slice(),
    };
    let plan = UniformPlan {
        specs: ctx.uniforms.into_boxed_slice(),
    };
    Ok((desc, plan))
}

/// Flattens `Chain` nodes into a linear stage list, left to right.
fn flatten<'a>(node: &'a TexNode, out: &mut Vec<&'a TexNode>, depth: u32) -> Result<(), Failure> {
    if depth > MAX_DEPTH {
        return Err(depth_exceeded());
    }
    match &node.kind {
        TexKind::Chain(l, r) => {
            flatten(l, out, depth + 1)?;
            flatten(r, out, depth + 1)?;
        }
        _ => out.push(node),
    }
    Ok(())
}

/// Compiles one chain (top-level or a `Blend`/`Modulate` "other" chain) to
/// a GLSL color expression, given the coordinate expression it starts
/// from. Geometry and `Modulate` stages fold into the coordinate BEFORE
/// the source is evaluated (Hydra's model: `osc > rotate` compiles to
/// `osc(rotate(st, ..), ..)`); color and `Blend` stages fold into the
/// color AFTER, in chain order.
fn compile_chain(
    node: &TexNode,
    uv_in: &str,
    ctx: &mut Ctx,
    depth: u32,
) -> Result<String, Failure> {
    if depth > MAX_DEPTH {
        return Err(depth_exceeded());
    }
    let mut stages = Vec::new();
    flatten(node, &mut stages, depth)?;
    let Some((first, rest)) = stages.split_first() else {
        return Err(Failure::new(FailCode::Type, "an empty visual chain"));
    };
    if !is_source(&first.kind) {
        return Err(Failure::new(
            FailCode::Type,
            "a visual chain starts with a source",
        ));
    }

    let mut uv = uv_in.to_string();
    for stage in rest {
        match &stage.kind {
            TexKind::Modulate(kind, other) => {
                let other_color = compile_chain(other, &uv, ctx, depth + 1)?;
                uv = emit_modulate(*kind, &uv, &other_color, stage, ctx)?;
            }
            k if is_geometry(k) => uv = emit_wrap(stage, &uv, ctx)?,
            _ => {}
        }
    }

    let mut color = emit_source(first, &uv, ctx)?;
    for stage in rest {
        match &stage.kind {
            TexKind::Blend(op, other) => {
                let other_color = compile_chain(other, &uv, ctx, depth + 1)?;
                color = emit_blend(*op, &color, &other_color, stage, ctx)?;
            }
            k if is_color(k) => color = emit_wrap(stage, &color, ctx)?,
            _ => {}
        }
    }
    Ok(color)
}

fn depth_exceeded() -> Failure {
    Failure::new(FailCode::DepthExceeded, "visual chain nested too deeply")
}

fn arity_err() -> Failure {
    Failure::new(FailCode::Arity, "too many visual-chain parameters")
}

fn is_source(k: &TexKind) -> bool {
    matches!(
        k,
        TexKind::Osc
            | TexKind::Noise
            | TexKind::Voronoi
            | TexKind::Shape
            | TexKind::Gradient
            | TexKind::Solid
            | TexKind::Text(_)
            | TexKind::SrcOut(_)
    )
}

fn is_geometry(k: &TexKind) -> bool {
    matches!(
        k,
        TexKind::Rotate
            | TexKind::Scale
            | TexKind::Pixelate
            | TexKind::Tile
            | TexKind::TileX
            | TexKind::TileY
            | TexKind::Kaleid
            | TexKind::Scroll
    )
}

fn is_color(k: &TexKind) -> bool {
    matches!(
        k,
        TexKind::Posterize
            | TexKind::Shift
            | TexKind::Invert
            | TexKind::Contrast
            | TexKind::Brightness
            | TexKind::Luma
            | TexKind::Thresh
            | TexKind::Color
            | TexKind::Saturate
            | TexKind::Hue
            | TexKind::Colorama
    )
}

/// The tag, GLSL function name and parameter spec of a source, geometry or
/// color kind (every `TexKind` except `Text`, `SrcOut`, `Blend`,
/// `Modulate` and `Chain`).
fn simple_op(k: &TexKind) -> (OpTag, &'static str, &'static [f32]) {
    match k {
        TexKind::Osc => (OpTag::Osc, "tex_osc", &[60.0, 0.1, 0.0]),
        TexKind::Noise => (OpTag::Noise, "tex_noise", &[10.0, 0.1]),
        TexKind::Voronoi => (OpTag::Voronoi, "tex_voronoi", &[5.0, 0.3, 0.3]),
        TexKind::Shape => (OpTag::Shape, "tex_shape", &[3.0, 0.3, 0.01]),
        TexKind::Gradient => (OpTag::Gradient, "tex_gradient", &[0.0]),
        TexKind::Solid => (OpTag::Solid, "tex_solid", &[0.0, 0.0, 0.0, 1.0]),
        TexKind::Rotate => (OpTag::Rotate, "tex_rotate", &[10.0, 0.0]),
        TexKind::Scale => (OpTag::Scale, "tex_scale", &[1.5, 1.0, 1.0]),
        TexKind::Pixelate => (OpTag::Pixelate, "tex_pixelate", &[20.0, 20.0]),
        TexKind::Tile => (OpTag::Tile, "tex_tile", &[3.0, 3.0]),
        TexKind::TileX => (OpTag::TileX, "tex_tile_x", &[3.0]),
        TexKind::TileY => (OpTag::TileY, "tex_tile_y", &[3.0]),
        TexKind::Kaleid => (OpTag::Kaleid, "tex_kaleid", &[4.0]),
        TexKind::Scroll => (OpTag::Scroll, "tex_scroll", &[0.5, 0.5, 0.0, 0.0]),
        TexKind::Posterize => (OpTag::Posterize, "tex_posterize", &[3.0, 0.6]),
        TexKind::Shift => (OpTag::Shift, "tex_shift", &[0.5, 0.0, 0.0, 0.0]),
        TexKind::Invert => (OpTag::Invert, "tex_invert", &[1.0]),
        TexKind::Contrast => (OpTag::Contrast, "tex_contrast", &[1.6]),
        TexKind::Brightness => (OpTag::Brightness, "tex_brightness", &[0.4]),
        TexKind::Luma => (OpTag::Luma, "tex_luma", &[0.5, 0.1]),
        TexKind::Thresh => (OpTag::Thresh, "tex_thresh", &[0.5, 0.04]),
        TexKind::Color => (OpTag::Color, "tex_color", &[1.0, 1.0, 1.0, 1.0]),
        TexKind::Saturate => (OpTag::Saturate, "tex_saturate", &[2.0]),
        TexKind::Hue => (OpTag::Hue, "tex_hue", &[0.4]),
        TexKind::Colorama => (OpTag::Colorama, "tex_colorama", &[0.005]),
        // Text, SrcOut, Blend, Modulate and Chain are handled by their own
        // callers and never reach here.
        _ => (OpTag::Solid, "tex_solid", &[]),
    }
}

fn blend_op(op: BlendOp) -> (OpTag, &'static str, &'static [f32]) {
    match op {
        BlendOp::Add => (OpTag::BlendAdd, "tex_add", &[1.0]),
        BlendOp::Sub => (OpTag::BlendSub, "tex_sub", &[1.0]),
        BlendOp::Layer => (OpTag::BlendLayer, "tex_layer", &[]),
        BlendOp::Blend => (OpTag::BlendBlend, "tex_blend", &[0.5]),
        BlendOp::Mult => (OpTag::BlendMult, "tex_mult", &[1.0]),
        BlendOp::Diff => (OpTag::BlendDiff, "tex_diff", &[]),
        BlendOp::Mask => (OpTag::BlendMask, "tex_mask", &[0.5]),
    }
}

fn mod_op(kind: ModKind) -> (OpTag, &'static str, &'static [f32]) {
    match kind {
        ModKind::Modulate => (OpTag::ModModulate, "tex_modulate", &[0.1]),
        ModKind::Tile => (OpTag::ModTile, "tex_mod_tile", &[3.0, 3.0]),
        ModKind::Kaleid => (OpTag::ModKaleid, "tex_mod_kaleid", &[4.0]),
        ModKind::Scroll => (OpTag::ModScroll, "tex_mod_scroll", &[0.5, 0.5, 0.0, 0.0]),
        ModKind::Rotate => (OpTag::ModRotate, "tex_mod_rotate", &[1.0, 0.0]),
        ModKind::Scale => (OpTag::ModScale, "tex_mod_scale", &[1.0, 1.0]),
        ModKind::Pixelate => (OpTag::ModPixelate, "tex_mod_pixelate", &[10.0, 3.0]),
    }
}

/// The GLSL argument list for `stage`'s parameters against `specs`: a
/// `Const` parameter is inlined as a literal; anything else becomes a
/// fresh uniform in encounter order; a missing trailing parameter uses the
/// spec default.
fn build_args(stage: &TexNode, specs: &[f32], ctx: &mut Ctx) -> Result<String, Failure> {
    if stage.params.len() > specs.len() {
        return Err(arity_err());
    }
    let mut parts = Vec::with_capacity(specs.len());
    for (i, spec) in specs.iter().enumerate() {
        let text = match stage.params.get(i) {
            Some(v) => param_expr(ctx, v)?,
            None => fmt_f32(*spec),
        };
        parts.push(text);
    }
    Ok(parts.join(", "))
}

fn param_expr(ctx: &mut Ctx, v: &crate::tex::texnode::VParam) -> Result<String, Failure> {
    use crate::tex::texnode::VParam;
    match v {
        VParam::Const(x) => {
            if !x.is_finite() {
                return Err(Failure::new(
                    FailCode::Type,
                    "a visual-chain parameter must be finite",
                ));
            }
            Ok(fmt_f32(*x))
        }
        other => {
            let name = format!("u{}", ctx.uniforms.len());
            ctx.uniforms.push(UniformSpec {
                name: name.clone(),
                src: other.clone(),
            });
            ctx.uniform_names.push(name.clone());
            Ok(name)
        }
    }
}

/// A GLSL float literal, always with a decimal point.
fn fmt_f32(x: f32) -> String {
    let s = format!("{x:?}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

fn emit_wrap(stage: &TexNode, input: &str, ctx: &mut Ctx) -> Result<String, Failure> {
    let (tag, name, specs) = simple_op(&stage.kind);
    ctx.used.insert(tag);
    let args = build_args(stage, specs, ctx)?;
    Ok(format!("{name}({input}, {args})"))
}

fn emit_source(stage: &TexNode, uv: &str, ctx: &mut Ctx) -> Result<String, Failure> {
    match &stage.kind {
        TexKind::Text(text) => {
            if !stage.params.is_empty() {
                return Err(arity_err());
            }
            let id = u32::try_from(ctx.assets.len()).unwrap_or(u32::MAX);
            let sampler = format!("u_text{id}");
            ctx.assets.push(TextAsset {
                id,
                text: text.to_string(),
            });
            ctx.samplers.push(sampler.clone());
            Ok(format!("texture({sampler}, {uv})"))
        }
        TexKind::SrcOut(out_id) => {
            if !stage.params.is_empty() {
                return Err(arity_err());
            }
            let sampler = format!("u_out{}", out_id.get());
            if !ctx.samplers.contains(&sampler) {
                ctx.samplers.push(sampler.clone());
            }
            Ok(format!("texture({sampler}, {uv})"))
        }
        _ => emit_wrap(stage, uv, ctx),
    }
}

fn emit_blend(
    op: BlendOp,
    color: &str,
    other: &str,
    stage: &TexNode,
    ctx: &mut Ctx,
) -> Result<String, Failure> {
    let (tag, name, specs) = blend_op(op);
    ctx.used.insert(tag);
    let args = build_args(stage, specs, ctx)?;
    let sep = if args.is_empty() {
        String::new()
    } else {
        format!(", {args}")
    };
    Ok(format!("{name}({color}, {other}{sep})"))
}

fn emit_modulate(
    kind: ModKind,
    uv: &str,
    other: &str,
    stage: &TexNode,
    ctx: &mut Ctx,
) -> Result<String, Failure> {
    let (tag, name, specs) = mod_op(kind);
    ctx.used.insert(tag);
    let args = build_args(stage, specs, ctx)?;
    let sep = if args.is_empty() {
        String::new()
    } else {
        format!(", {args}")
    };
    Ok(format!("{name}({uv}, {other}{sep})"))
}

/// A GLSL function definition for a used operator tag (fixed library,
/// design 9.2's "hydra-synth glsl-functions" model). Self-contained: no
/// function calls another, so each can be emitted independently.
fn glsl_def(tag: OpTag) -> &'static str {
    match tag {
        OpTag::Osc => {
            "vec4 tex_osc(vec2 uv, float freq, float sync, float offset) {\n  float r = sin((uv.x + offset) * freq + time * sync) * 0.5 + 0.5;\n  float g = sin((uv.x + offset) * freq + time * sync + 2.094) * 0.5 + 0.5;\n  float b = sin((uv.x + offset) * freq + time * sync + 4.189) * 0.5 + 0.5;\n  return vec4(r, g, b, 1.0);\n}\n"
        }
        OpTag::Noise => {
            "vec4 tex_noise(vec2 uv, float scale, float offset) {\n  vec2 p = uv * scale + time * offset;\n  float n = fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);\n  return vec4(vec3(n), 1.0);\n}\n"
        }
        OpTag::Voronoi => {
            "vec4 tex_voronoi(vec2 uv, float scale, float speed, float blending) {\n  vec2 p = uv * scale;\n  vec2 ip = floor(p);\n  vec2 fp = fract(p);\n  float d = 1.0;\n  for (int y = -1; y <= 1; y++) {\n    for (int x = -1; x <= 1; x++) {\n      vec2 g = vec2(float(x), float(y));\n      vec2 o = fract(sin((ip + g) * 78.233 + time * speed) * 43758.5453);\n      float dist = length(g + o - fp);\n      d = mix(d, min(d, dist), blending);\n    }\n  }\n  return vec4(vec3(d), 1.0);\n}\n"
        }
        OpTag::Shape => {
            "vec4 tex_shape(vec2 uv, float sides, float radius, float smoothing) {\n  vec2 c = uv * 2.0 - 1.0;\n  float a = atan(c.x, c.y) + 3.14159265;\n  float r = 6.28318530 / sides;\n  float d = cos(floor(0.5 + a / r) * r - a) * length(c);\n  float shape = smoothstep(radius, radius - smoothing, d);\n  return vec4(vec3(shape), 1.0);\n}\n"
        }
        OpTag::Gradient => {
            "vec4 tex_gradient(vec2 uv, float speed) {\n  return vec4(uv.x + time * speed, uv.y, sin(time * speed), 1.0);\n}\n"
        }
        OpTag::Solid => {
            "vec4 tex_solid(vec2 uv, float r, float g, float b, float a) {\n  return vec4(r, g, b, a);\n}\n"
        }
        OpTag::Rotate => {
            "vec2 tex_rotate(vec2 uv, float angle, float speed) {\n  float a = radians(angle) + time * speed;\n  vec2 c = uv - 0.5;\n  float s = sin(a);\n  float cs = cos(a);\n  return vec2(c.x * cs - c.y * s, c.x * s + c.y * cs) + 0.5;\n}\n"
        }
        OpTag::Scale => {
            "vec2 tex_scale(vec2 uv, float amount, float x_mult, float y_mult) {\n  vec2 c = uv - 0.5;\n  return vec2(c.x / (amount * x_mult), c.y / (amount * y_mult)) + 0.5;\n}\n"
        }
        OpTag::Pixelate => {
            "vec2 tex_pixelate(vec2 uv, float pixel_x, float pixel_y) {\n  vec2 grid = vec2(pixel_x, pixel_y);\n  return floor(uv * grid) / grid;\n}\n"
        }
        OpTag::Tile => {
            "vec2 tex_tile(vec2 uv, float nx, float ny) {\n  return fract(uv * vec2(nx, ny));\n}\n"
        }
        OpTag::TileX => {
            "vec2 tex_tile_x(vec2 uv, float nx) {\n  return vec2(fract(uv.x * nx), uv.y);\n}\n"
        }
        OpTag::TileY => {
            "vec2 tex_tile_y(vec2 uv, float ny) {\n  return vec2(uv.x, fract(uv.y * ny));\n}\n"
        }
        OpTag::Kaleid => {
            "vec2 tex_kaleid(vec2 uv, float sides) {\n  vec2 c = uv - 0.5;\n  float a = atan(c.y, c.x);\n  float r = length(c);\n  float seg = 6.28318530 / sides;\n  a = abs(mod(a, seg) - seg * 0.5);\n  return vec2(cos(a), sin(a)) * r + 0.5;\n}\n"
        }
        OpTag::Scroll => {
            "vec2 tex_scroll(vec2 uv, float scroll_x, float scroll_y, float speed_x, float speed_y) {\n  return fract(uv + vec2(scroll_x + time * speed_x, scroll_y + time * speed_y));\n}\n"
        }
        OpTag::Posterize => {
            "vec4 tex_posterize(vec4 c, float bins, float gamma) {\n  vec3 p = pow(clamp(c.rgb, 0.0, 1.0), vec3(gamma));\n  p = floor(p * bins) / bins;\n  return vec4(p, c.a);\n}\n"
        }
        OpTag::Shift => {
            "vec4 tex_shift(vec4 c, float r, float g, float b, float a) {\n  return fract(c + vec4(r, g, b, a));\n}\n"
        }
        OpTag::Invert => {
            "vec4 tex_invert(vec4 c, float amount) {\n  return mix(c, vec4(1.0 - c.rgb, c.a), amount);\n}\n"
        }
        OpTag::Contrast => {
            "vec4 tex_contrast(vec4 c, float amount) {\n  return vec4((c.rgb - 0.5) * amount + 0.5, c.a);\n}\n"
        }
        OpTag::Brightness => {
            "vec4 tex_brightness(vec4 c, float amount) {\n  return vec4(c.rgb + amount, c.a);\n}\n"
        }
        OpTag::Luma => {
            "vec4 tex_luma(vec4 c, float threshold, float tolerance) {\n  float l = dot(c.rgb, vec3(0.299, 0.587, 0.114));\n  float a = smoothstep(threshold - tolerance, threshold + tolerance, l);\n  return vec4(c.rgb, a);\n}\n"
        }
        OpTag::Thresh => {
            "vec4 tex_thresh(vec4 c, float threshold, float tolerance) {\n  float l = dot(c.rgb, vec3(0.299, 0.587, 0.114));\n  float v = smoothstep(threshold - tolerance, threshold + tolerance, l);\n  return vec4(vec3(v), c.a);\n}\n"
        }
        OpTag::Color => {
            "vec4 tex_color(vec4 c, float r, float g, float b, float a) {\n  return vec4(c.r * r, c.g * g, c.b * b, c.a * a);\n}\n"
        }
        OpTag::Saturate => {
            "vec4 tex_saturate(vec4 c, float amount) {\n  float l = dot(c.rgb, vec3(0.299, 0.587, 0.114));\n  return vec4(mix(vec3(l), c.rgb, amount), c.a);\n}\n"
        }
        OpTag::Hue => {
            "vec4 tex_hue(vec4 c, float amount) {\n  float a = fract(amount);\n  vec3 s = vec3(mix(c.r, c.g, a), mix(c.g, c.b, a), mix(c.b, c.r, a));\n  return vec4(s, c.a);\n}\n"
        }
        OpTag::Colorama => {
            "vec4 tex_colorama(vec4 c, float amount) {\n  return vec4(fract(c.rgb + amount), c.a);\n}\n"
        }
        OpTag::BlendAdd => {
            "vec4 tex_add(vec4 a, vec4 b, float amount) {\n  return a + b * amount;\n}\n"
        }
        OpTag::BlendSub => {
            "vec4 tex_sub(vec4 a, vec4 b, float amount) {\n  return a - b * amount;\n}\n"
        }
        OpTag::BlendLayer => {
            "vec4 tex_layer(vec4 a, vec4 b) {\n  return vec4(mix(a.rgb, b.rgb, b.a), max(a.a, b.a));\n}\n"
        }
        OpTag::BlendBlend => {
            "vec4 tex_blend(vec4 a, vec4 b, float amount) {\n  return mix(a, b, amount);\n}\n"
        }
        OpTag::BlendMult => {
            "vec4 tex_mult(vec4 a, vec4 b, float amount) {\n  return mix(a, a * b, amount);\n}\n"
        }
        OpTag::BlendDiff => {
            "vec4 tex_diff(vec4 a, vec4 b) {\n  return vec4(abs(a.rgb - b.rgb), max(a.a, b.a));\n}\n"
        }
        OpTag::BlendMask => {
            "vec4 tex_mask(vec4 a, vec4 b, float cutoff) {\n  float l = dot(b.rgb, vec3(0.299, 0.587, 0.114));\n  return vec4(a.rgb, a.a * step(cutoff, l));\n}\n"
        }
        OpTag::ModModulate => {
            "vec2 tex_modulate(vec2 uv, vec4 c, float amount) {\n  return uv + c.xy * amount;\n}\n"
        }
        OpTag::ModTile => {
            "vec2 tex_mod_tile(vec2 uv, vec4 c, float nx, float ny) {\n  return fract(uv * vec2(nx, ny) + c.xy);\n}\n"
        }
        OpTag::ModKaleid => {
            "vec2 tex_mod_kaleid(vec2 uv, vec4 c, float sides) {\n  vec2 p = uv + c.xy * 0.1 - 0.5;\n  float a = atan(p.y, p.x);\n  float r = length(p);\n  float seg = 6.28318530 / sides;\n  a = abs(mod(a, seg) - seg * 0.5);\n  return vec2(cos(a), sin(a)) * r + 0.5;\n}\n"
        }
        OpTag::ModScroll => {
            "vec2 tex_mod_scroll(vec2 uv, vec4 c, float scroll_x, float scroll_y, float speed_x, float speed_y) {\n  return fract(uv + c.xy * 0.1 + vec2(scroll_x + time * speed_x, scroll_y + time * speed_y));\n}\n"
        }
        OpTag::ModRotate => {
            "vec2 tex_mod_rotate(vec2 uv, vec4 c, float multiple, float offset) {\n  float a = (c.x * multiple + offset) * 6.28318530;\n  vec2 p = uv - 0.5;\n  return vec2(p.x * cos(a) - p.y * sin(a), p.x * sin(a) + p.y * cos(a)) + 0.5;\n}\n"
        }
        OpTag::ModScale => {
            "vec2 tex_mod_scale(vec2 uv, vec4 c, float multiple, float offset) {\n  float amt = max(c.x * multiple + offset, 0.0001);\n  vec2 p = uv - 0.5;\n  return p / amt + 0.5;\n}\n"
        }
        OpTag::ModPixelate => {
            "vec2 tex_mod_pixelate(vec2 uv, vec4 c, float multiple, float offset) {\n  vec2 grid = vec2(max(c.x * multiple + offset, 1.0));\n  return floor(uv * grid) / grid;\n}\n"
        }
    }
}
