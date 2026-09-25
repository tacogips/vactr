//! Golden shader-source tests (design 9.2/9.4, TASK-006).

use std::rc::Rc;

use crate::tex::shader::{compile_tex, TextAsset};
use crate::tex::texnode::{pipe, TexKind, TexNode, VParam};

/// `osc 20 > rotate 0.5` (design 9.4's planned golden). Both parameters
/// are constants, so the compiled shader has no uniforms.
const OSC_ROTATE_GOLDEN: &str = r#"#version 300 es
precision highp float;
uniform float time;
uniform vec2 resolution;
out vec4 fragColor;
vec4 tex_osc(vec2 uv, float freq, float sync, float offset) {
  float r = sin((uv.x + offset) * freq + time * sync) * 0.5 + 0.5;
  float g = sin((uv.x + offset) * freq + time * sync + 2.094) * 0.5 + 0.5;
  float b = sin((uv.x + offset) * freq + time * sync + 4.189) * 0.5 + 0.5;
  return vec4(r, g, b, 1.0);
}

vec2 tex_rotate(vec2 uv, float angle, float speed) {
  float a = radians(angle) + time * speed;
  vec2 c = uv - 0.5;
  float s = sin(a);
  float cs = cos(a);
  return vec2(c.x * cs - c.y * s, c.x * s + c.y * cs) + 0.5;
}

void main() {
  vec2 st = gl_FragCoord.xy / resolution.xy;
  fragColor = tex_osc(tex_rotate(st, 0.5, 0.0), 20.0, 0.1, 0.0);
}
"#;

#[test]
fn osc_rotate_golden_source_is_stable() {
    let osc_node = TexNode::new(TexKind::Osc, vec![VParam::Const(20.0)], None);
    let rotate_node = TexNode::new(TexKind::Rotate, vec![VParam::Const(0.5)], None);
    let chain = pipe(Rc::new(osc_node), rotate_node, None);

    let (desc, plan) = compile_tex(&chain).unwrap();
    assert_eq!(desc.source, OSC_ROTATE_GOLDEN);
    assert!(plan.specs.is_empty());
    assert!(desc.uniform_names.is_empty());
    assert!(desc.assets.is_empty());

    // Compiling the same chain twice gives byte-identical output.
    let osc_node2 = TexNode::new(TexKind::Osc, vec![VParam::Const(20.0)], None);
    let rotate_node2 = TexNode::new(TexKind::Rotate, vec![VParam::Const(0.5)], None);
    let chain2 = pipe(Rc::new(osc_node2), rotate_node2, None);
    let (desc2, plan2) = compile_tex(&chain2).unwrap();
    assert_eq!(desc.source, desc2.source);
    assert!(plan2.specs.is_empty());
}

/// `text "hello" > scale 0.5`: the text payload becomes a `TextAsset` and
/// a `u_text0` sampler; both parameters are constants (`scale`'s only
/// given argument is `0.5`), so there are no numeric uniforms.
#[test]
fn text_scale_chain_produces_a_text_asset() {
    let text_node = TexNode::new(TexKind::Text(Rc::from("hello")), Vec::new(), None);
    let scale_node = TexNode::new(TexKind::Scale, vec![VParam::Const(0.5)], None);
    let chain = pipe(Rc::new(text_node), scale_node, None);

    let (desc, _plan) = compile_tex(&chain).unwrap();
    assert_eq!(
        &*desc.assets,
        &[TextAsset {
            id: 0,
            text: "hello".to_string(),
        }][..]
    );
    assert!(desc.source.contains("u_text0"));
    assert!(desc.source.contains("uniform sampler2D u_text0;"));
}
