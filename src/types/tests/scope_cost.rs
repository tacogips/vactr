//! Counter-based cost and behavior checks for scope name indexing.

use crate::types::diag::{DiagCode, Severity};

use super::{check_src, diag_lines};

#[test]
fn unique_session_lets_scale_with_scope_probes() {
    let source = |count: usize| {
        (0..count)
            .map(|index| format!("let v{index} {index}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let src4 = source(4_000);
    crate::types::scope::reset_name_probes();
    let result4 = check_src(&src4);
    let c4 = crate::types::scope::name_probes();
    assert!(result4.diags.is_empty(), "{:?}", result4.diags);

    let src8 = source(8_000);
    crate::types::scope::reset_name_probes();
    let result8 = check_src(&src8);
    let c8 = crate::types::scope::name_probes();
    assert!(result8.diags.is_empty(), "{:?}", result8.diags);

    println!("scope name probes: c4={c4}, c8={c8}");
    assert!(c4 > 0);
    assert!(c8 <= 32_000, "c8={c8}");
    assert!(c8 as f64 <= 2.2 * c4 as f64, "c4={c4}, c8={c8}");
}

#[test]
fn rebinding_diagnostic_still_points_to_first_slot_binding() {
    let src = "let a 1\nlet a 2\nlet a 3";
    let result = check_src(src);
    assert_eq!(diag_lines(src, &result), ["rebinding@2", "rebinding@3"]);
    assert_eq!(result.diags[0].code, DiagCode::Rebinding);
    assert_eq!(result.diags[1].code, DiagCode::Rebinding);
    assert_eq!(
        result.diags[1].message,
        "`a` is already bound in this scope (at byte 12); a name is bound once per scope"
    );
}

#[test]
fn session_binding_shadowed_by_function_parameter_stays_a_warning() {
    let src = "let x 1\nfn f x:\n\tx";
    let result = check_src(src);
    assert_eq!(diag_lines(src, &result), ["shadowing@2"]);
    assert_eq!(result.diags[0].code, DiagCode::Shadowing);
    assert_eq!(result.diags[0].severity, Severity::Warning);
}
