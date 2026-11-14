//! Checked finite song constructors: literal diagnostics complement runtime checks.
use std::collections::BTreeSet;

use super::{literal_number, pair_key, Args};
use crate::reader::node::{Atom, Node, NodeKind};
use crate::types::check::Checker;
use crate::types::diag::DiagCode;
use crate::types::natives::NativeSig;
use crate::types::ty::Ty;

const SONG_NAMES: [&str; 11] = [
    "part",
    "part-repeat",
    "sequence",
    "replace-track",
    "transform-instrument",
    "part-events",
    "delete-event",
    "overwrite-region",
    "instrument-fx",
    "song",
    "play-song",
];

pub(super) fn check(cx: &mut Checker<'_>, call: &Node, sig: &NativeSig, args: &Args<'_>) {
    if !SONG_NAMES.contains(&sig.name) {
        return;
    }
    let p = &args.positional;
    for pair in &args.named {
        let Some(value) = pair.children.get(1) else {
            continue;
        };
        let key = pair_key(pair).unwrap_or("");
        let expected = match key {
            "duration" | "bpm" | "cycle-beats" | "tail-seconds" => Some(Ty::Ratio),
            "seed" => Some(Ty::Int64),
            "meter" => Some(Ty::List(Box::new(Ty::Int))),
            "seed-mode" => Some(Ty::keyword()),
            _ => None,
        };
        cx.check_one(
            value,
            expected.as_ref(),
            false,
            &format!("`{key}:` of `{}`", sig.name),
        );
        match key {
            "duration" | "bpm" | "cycle-beats" => number_bound(cx, value, true, key),
            "tail-seconds" => number_bound(cx, value, false, key),
            "seed" => integer_bound(cx, value, 0.0, f64::INFINITY, "seed"),
            "seed-mode" => {
                if let NodeKind::Atom(Atom::Keyword(mode)) = &literal_node(value).kind {
                    if !matches!(&**mode, "same" | "vary") {
                        invalid(cx, value, "seed-mode must be :same or :vary");
                    }
                }
            }
            "meter" => meter(cx, value),
            _ => {}
        }
    }
    let mut names = BTreeSet::new();
    for pair in &args.named {
        if !names.insert(pair_key(pair)) {
            invalid(cx, pair, "duplicate song named argument");
        }
    }
    match sig.name {
        "part" => {
            if !args
                .named
                .iter()
                .any(|pair| pair_key(pair) == Some("duration"))
            {
                invalid(cx, call, "part requires an explicit positive duration:");
            }
            if let Some(tracks) = p.first() {
                track_dictionary(cx, tracks);
            }
        }
        "part-repeat" => {
            if let Some(count) = p.get(1) {
                integer_bound(cx, count, 0.0, f64::from(u32::MAX), "part-repeat count");
            }
        }
        "transform-instrument" => {
            if let Some(callback) = p.get(3) {
                transform_callback(cx, callback);
                if cx.contains_effect(callback, 0) {
                    invalid(cx, callback, "song transform callback must be pure");
                }
            }
        }
        "overwrite-region" | "part-events" => {
            if let (Some(begin), Some(end)) = (p.get(2), p.get(3)) {
                number_bound(cx, begin, false, "region begin");
                number_bound(cx, end, false, "region end");
                if let (Some(a), Some(b)) = (literal_exact(begin), literal_exact(end)) {
                    if a >= b {
                        invalid(cx, end, "region requires begin < end");
                    }
                    if let Some(duration) = p.first().and_then(|part| capture_duration(part)) {
                        if b > duration {
                            invalid(cx, end, "region extends past part duration");
                        }
                    }
                }
            }
        }
        _ => {}
    }
    // A directly visible capture supplies a closed track-key set. Dynamic
    // captures/edits are checked by the runtime against their immutable Part.
    if matches!(
        sig.name,
        "replace-track"
            | "transform-instrument"
            | "part-events"
            | "overwrite-region"
            | "instrument-fx"
    ) {
        if let (Some(source), Some(track)) = (p.first(), p.get(1)) {
            if let (Some(tracks), NodeKind::Atom(Atom::Keyword(key))) =
                (capture_tracks(source), &track.kind)
            {
                if !tracks
                    .children
                    .iter()
                    .any(|entry| pair_key(entry) == Some(key))
                {
                    invalid(cx, track, "unknown track key in captured part");
                }
            }
        }
    }
}

/// Selected sources have sound-valued leaves and can also be passed directly
/// through existing control transforms. Keep this admission local to song edits.
fn transform_callback(cx: &mut Checker<'_>, node: &Node) {
    let actual = cx.u.shallow(&cx.ty_of(node));
    let Ty::Fn(parameters, result) = &actual else {
        let code = if matches!(actual, Ty::Any) {
            DiagCode::AnyNotNarrowed
        } else {
            DiagCode::TypeMismatch
        };
        cx.emit(
            code,
            node.span,
            "song transform requires a known one-argument callback",
        );
        return;
    };
    if parameters.len() != 1 {
        invalid(
            cx,
            node,
            "song transform callback takes exactly one source pattern",
        );
        return;
    }
    if matches!(cx.u.shallow(&parameters[0]), Ty::Any) || matches!(cx.u.shallow(result), Ty::Any) {
        cx.emit(
            DiagCode::AnyNotNarrowed,
            node.span,
            "song transform callback input and result must be narrowed source patterns",
        );
        return;
    }
    if !accept_transform_type(&mut cx.u, &actual) {
        invalid(
            cx,
            node,
            "song transform callback must map a sound/control pattern to a sound/control pattern",
        );
    }
}

fn accept_transform_type(u: &mut crate::types::unify::Unifier, actual: &Ty) -> bool {
    if matches!(u.shallow(actual), Ty::Fn(ref ps, _) if ps.len() == 1 && matches!(
        u.shallow(&ps[0]), Ty::Signal | Ty::Nil | Ty::Sound | Ty::KeywordOf(_)
        | Ty::Int | Ty::Int64 | Ty::Float | Ty::Float64 | Ty::Ratio | Ty::Bool | Ty::Str))
    {
        return false;
    }
    let views = [super::ctl(), Ty::Pattern(Box::new(Ty::Sound))];
    for input in &views {
        for output in &views {
            let expected = Ty::func(vec![input.clone()], output.clone());
            // A failed trial restores every binding. Checking the whole function
            // preserves relationships between its input and result variables.
            if u.try_unify(&expected, actual).is_ok() {
                return true;
            }
        }
    }
    false
}

fn invalid(cx: &mut Checker<'_>, node: &Node, message: &str) {
    cx.emit(DiagCode::TypeMismatch, node.span, message);
}

fn number_bound(cx: &mut Checker<'_>, node: &Node, positive: bool, name: &str) {
    let node = literal_node(node);
    if let Some(value) = literal_number(node) {
        if !value.is_finite() || value < 0.0 || (positive && value == 0.0) {
            invalid(
                cx,
                node,
                &format!(
                    "{name} must be {}",
                    if positive { "positive" } else { "nonnegative" }
                ),
            );
        }
        if matches!(node.kind, NodeKind::Atom(Atom::Float { .. })) {
            invalid(
                cx,
                node,
                &format!("{name} must use exact integer or ratio time"),
            );
        }
    }
}

fn integer_bound(cx: &mut Checker<'_>, node: &Node, min: f64, max: f64, name: &str) {
    let node = literal_node(node);
    if let Some(value) = literal_number(node) {
        if !value.is_finite()
            || value.fract() != 0.0
            || value < min
            || value > max
            || matches!(node.kind, NodeKind::Atom(Atom::Float { .. }))
            || matches!(node.kind, NodeKind::Atom(Atom::Ratio(r)) if r.den() != 1)
        {
            invalid(
                cx,
                node,
                &format!("{name} must be an integer in {min}..={max}"),
            );
        }
    }
}

fn meter(cx: &mut Checker<'_>, node: &Node) {
    let node = literal_node(node);
    if !matches!(node.kind, NodeKind::List) {
        return;
    }
    if node.children.len() != 2 {
        invalid(
            cx,
            node,
            "meter requires [positive numerator power-of-two denominator]",
        );
        return;
    }
    for item in &node.children {
        integer_bound(cx, item, 1.0, f64::from(u32::MAX), "meter component");
    }
    if let Some(value) = literal_exact(&node.children[1]) {
        if value.den() != 1 || u32::try_from(value.num()).map_or(true, |n| !n.is_power_of_two()) {
            invalid(
                cx,
                &node.children[1],
                "meter denominator must be a positive power of two",
            );
        }
    }
}

fn track_dictionary(cx: &mut Checker<'_>, node: &Node) {
    let node = literal_node(node);
    if !matches!(node.kind, NodeKind::List) {
        return;
    }
    let mut keys = BTreeSet::new();
    for entry in &node.children {
        let Some(key) = pair_key(entry) else {
            invalid(cx, entry, "part tracks require keyword dictionary keys");
            continue;
        };
        if !keys.insert(key) {
            invalid(cx, entry, "duplicate part track key");
        }
        if let Some(value) = entry.children.get(1) {
            cx.check_one(
                value,
                Some(&Ty::Pattern(Box::new(Ty::Any))),
                false,
                "part track pattern",
            );
        }
    }
}

fn capture_call(node: &Node) -> Option<&Node> {
    let node = literal_node(node);
    (matches!(node.kind, NodeKind::Call) && node.children.first()?.sym_name() == Some("part"))
        .then_some(node)
}
fn capture_tracks(node: &Node) -> Option<&Node> {
    let tracks = capture_call(node)?.children.get(1)?;
    matches!(tracks.kind, NodeKind::List).then_some(tracks)
}
fn capture_duration(node: &Node) -> Option<crate::value::ratio::Ratio64> {
    let pair = capture_call(node)?
        .children
        .iter()
        .find(|n| pair_key(n) == Some("duration"))?;
    literal_exact(pair.children.get(1)?)
}

fn literal_exact(node: &Node) -> Option<crate::value::ratio::Ratio64> {
    let node = literal_node(node);
    match node.kind {
        NodeKind::Atom(Atom::Int(value)) => Some(crate::value::ratio::Ratio64::from_int(value)),
        NodeKind::Atom(Atom::Ratio(value)) => Some(value),
        _ => None,
    }
}

/// Unwrap only statically pure single-expression groups. Multi-statement and
/// callable bodies remain dynamic, and the walk shares the checker depth bound.
fn literal_node(mut node: &Node) -> &Node {
    for _ in 0..256 {
        if matches!(node.kind, NodeKind::Block) && node.children.len() == 1 {
            node = &node.children[0];
        } else {
            break;
        }
    }
    node
}

/// Sound functions receive an exact cycle Ratio at point-query time. Keep the
/// input and sound-result trial together so a shared time/result variable
/// cannot be narrowed independently to a keyword source.
pub(super) fn lazy_sound_call(
    cx: &mut Checker<'_>,
    _call: &Node,
    sig: &'static NativeSig,
    args: &[Node],
    depth: u32,
) -> Option<Ty> {
    let args = super::split_args(args, true);
    if args.splat || args.positional.len() != 1 {
        return None;
    }
    let has_kit = args.named.iter().any(|pair| pair_key(pair) == Some("kit"));
    for pair in args.named {
        let key = pair_key(pair).unwrap_or("");
        if let Some(value) = pair.children.get(1) {
            let actual = cx.infer(value, depth);
            if key == "kit" {
                cx.check_arg(
                    &Ty::Dict(Box::new(Ty::keyword()), Box::new(Ty::Sound)),
                    &actual,
                    value,
                    "`kit:` of `s`",
                );
            }
        }
        if key != "kit" {
            cx.emit(
                DiagCode::TypeMismatch,
                pair.span,
                format!(
                    "`{}` takes only the named argument `kit:`, not `{key}:`",
                    sig.name
                ),
            );
        }
    }
    let check = !has_kit
        && cx.scopes.session_lookup("sound-kit").is_none()
        && cx.env.global("sound-kit").is_none()
        && cx.live.open_prefix("sound-kit").is_none();
    sound_source(cx, args.positional[0], check, depth, 0);
    Some(super::ctl())
}
// Mirror the ordinary one-source syntax walk, but infer each leaf only once.
// Fn leaves retain their actual time/result relation instead of being checked
// as a scalar sound. Literal and SOUND FIRST behavior stays unchanged.
fn sound_source(cx: &mut Checker<'_>, source: &Node, check: bool, depth: u32, nesting: u32) {
    if nesting > 64 {
        cx.infer(source, depth);
        return;
    }
    match &source.kind {
        NodeKind::Atom(Atom::Keyword(k)) => {
            let session_inst = cx
                .env
                .global(k)
                .is_some_and(|g| g.kind == crate::types::ty::BindKind::Inst);
            if check && !cx.kit_keys.contains(k) && !cx.inst_names.contains(&**k) && !session_inst {
                cx.emit(
                    DiagCode::UnknownKeyword,
                    source.span,
                    format!("`:{k}` is not a sound in the default sound kit"),
                );
            }
            cx.types.insert(source.id, Ty::Sound);
        }
        NodeKind::Atom(Atom::Nil) => {
            cx.types.insert(source.id, Ty::Nil);
        }
        NodeKind::List => {
            for child in &source.children {
                sound_source(cx, child, check, depth, nesting + 1);
            }
            cx.types.insert(source.id, Ty::Pattern(Box::new(Ty::Sound)));
        }
        NodeKind::Block if source.children.len() == 1 => {
            let mark = cx.scopes.depth();
            cx.scopes.push(crate::types::scope::ScopeKind::Block);
            sound_source(cx, &source.children[0], check, depth, nesting + 1);
            cx.scopes.truncate(mark);
            let ty = cx
                .types
                .get(&source.children[0].id)
                .cloned()
                .unwrap_or(Ty::Any);
            cx.types.insert(source.id, ty);
        }
        NodeKind::Call if sound_step(cx, source).is_some() => {
            let all = sound_step(cx, source) == Some(true);
            for (index, child) in source.children.iter().enumerate().skip(1) {
                match child.kind {
                    NodeKind::Pair => {
                        for v in child.children.iter().skip(1) {
                            cx.infer(v, depth);
                        }
                    }
                    _ if all || index == 1 => sound_source(cx, child, check, depth, nesting + 1),
                    _ => {
                        cx.infer(child, depth);
                    }
                }
            }
            cx.types.insert(source.id, Ty::Pattern(Box::new(Ty::Sound)));
        }
        _ => {
            let actual = cx.infer(source, depth);
            if let Ty::Fn(inputs, result) = cx.u.shallow(&actual) {
                let time_input = inputs.first().is_some_and(|input| {
                    matches!(
                        cx.u.shallow(input),
                        Ty::Var(_)
                            | Ty::Ratio
                            | Ty::Float
                            | Ty::Float64
                            | Ty::Int
                            | Ty::Int64
                            | Ty::NumLit(_)
                    )
                });
                let valid = (inputs.is_empty() || (inputs.len() == 1 && time_input))
                    && sound_result_view(cx, &result, 0).is_some_and(|result| {
                        let expected_inputs = if inputs.is_empty() {
                            vec![]
                        } else {
                            vec![Ty::Ratio]
                        };
                        cx.u.try_unify(&Ty::func(expected_inputs, result), &actual)
                            .is_ok()
                    });
                if !valid {
                    cx.emit(DiagCode::TypeMismatch,source.span,
                    "lazy sound callback must accept no arguments or Ratio time and return sound/keyword steps or rests");
                }
            } else {
                cx.check_arg(
                    &Ty::Pattern(Box::new(Ty::Sound)),
                    &actual,
                    source,
                    "the sound of `s`",
                );
            }
        }
    }
}
fn sound_step(cx: &Checker<'_>, source: &Node) -> Option<bool> {
    let sig = cx.prelude_head(source.children.first()?)?;
    if super::STEP_ALL.contains(&sig.name) {
        Some(true)
    } else if super::STEP_FIRST.contains(&sig.name) {
        Some(false)
    } else {
        None
    }
}

fn sound_result_view(cx: &Checker<'_>, actual: &Ty, depth: u32) -> Option<Ty> {
    if depth >= 64 {
        return None;
    }
    Some(match cx.u.shallow(actual) {
        Ty::Opt(t) => Ty::Opt(Box::new(sound_result_view(cx, &t, depth + 1)?)),
        Ty::List(t) => Ty::List(Box::new(sound_result_view(cx, &t, depth + 1)?)),
        Ty::Pattern(t) => Ty::Pattern(Box::new(sound_result_view(cx, &t, depth + 1)?)),
        Ty::Sound | Ty::KeywordOf(_) | Ty::Var(_) => Ty::Sound,
        Ty::Path => Ty::Path,
        Ty::Nil => Ty::Nil,
        _ => return None,
    })
}

#[cfg(test)]
mod transform_contract_tests {
    use super::*;
    use crate::types::unify::Unifier;

    #[test]
    fn failed_first_function_trial_restores_input_before_second_succeeds() {
        let mut u = Unifier::new();
        let input = u.fresh();
        let sound = Ty::Pattern(Box::new(Ty::Sound));
        let actual = Ty::func(vec![input.clone()], sound.clone());
        let first = Ty::func(vec![super::super::ctl()], super::super::ctl());
        assert!(u.try_unify(&first, &actual).is_err());
        assert_eq!(
            u.shallow(&input),
            input,
            "failed return must undo input binding"
        );
        let second = Ty::func(vec![super::super::ctl()], sound);
        assert!(u.try_unify(&second, &actual).is_ok());
        assert_eq!(u.shallow(&input), super::super::ctl());
        assert!(accept_transform_type(&mut u, &actual));
    }

    #[test]
    fn complete_callback_trials_preserve_shared_input_result_variables() {
        let mut u = Unifier::new();
        let same = u.fresh();
        let identity = Ty::func(vec![same.clone()], same.clone());
        assert!(accept_transform_type(&mut u, &identity));
        assert_eq!(u.shallow(&same), super::super::ctl());
        let incompatible = Ty::func(vec![Ty::Float], Ty::Pattern(Box::new(Ty::Sound)));
        assert!(!accept_transform_type(&mut u, &incompatible));
    }
}
