//! `Ty`, `KeySet`, `Scheme::parse` and `CheckEnv` basics.

use crate::types::ty::{BindKind, CheckEnv, GlobalInfo, KeySet, Scheme, Ty, TyVar};

#[test]
fn ty_equality() {
    assert_eq!(Ty::List(Box::new(Ty::Int)), Ty::List(Box::new(Ty::Int)));
    assert_ne!(Ty::List(Box::new(Ty::Int)), Ty::List(Box::new(Ty::Float)));
    assert_ne!(Ty::Path, Ty::Url);
    assert_ne!(Ty::Sound, Ty::Str);
    assert_eq!(
        Ty::func(vec![Ty::Path], Ty::Sound),
        Ty::Fn(Box::new([Ty::Path]), Box::new(Ty::Sound))
    );
    assert_eq!(Ty::keyword(), Ty::KeywordOf(KeySet::Open));
    assert!(Ty::func(vec![], Ty::Nil).is_fn());
}

#[test]
fn keyset_membership() {
    let kit = KeySet::of(["bd", "sd"]);
    assert!(kit.contains("bd"));
    assert!(!kit.contains("hh"));
    assert!(!kit.is_open());
    assert!(KeySet::Open.contains("anything"));
    let both = kit.union(&KeySet::of(["hh"]));
    assert!(both.contains("hh") && both.contains("sd"));
    assert!(kit.union(&KeySet::Open).is_open());
    assert_eq!(KeySet::of(["b", "a"]), KeySet::of(["a", "b"]));
}

#[test]
fn scheme_parse_notation() {
    let s = Scheme::parse("fn ['a] (fn 'a -> 'b) -> ['b]").expect("parses");
    assert_eq!(s.vars.len(), 2);
    let a = Ty::Var(TyVar::new(0));
    let b = Ty::Var(TyVar::new(1));
    assert_eq!(
        s.ty,
        Ty::func(
            vec![Ty::List(Box::new(a.clone())), Ty::func(vec![a], b.clone()),],
            Ty::List(Box::new(b)),
        )
    );
    let kit = Scheme::parse("[keyword: sound]").expect("parses");
    assert_eq!(
        kit.ty,
        Ty::Dict(Box::new(Ty::keyword()), Box::new(Ty::Sound))
    );
    assert!(kit.vars.is_empty());
    let opt = Scheme::parse("fn ['a] -> ?'a").expect("parses");
    assert!(matches!(opt.ty, Ty::Fn(_, ref r) if matches!(**r, Ty::Opt(_))));
    let ctl = Scheme::parse("ctl").expect("parses");
    assert_eq!(
        ctl.ty,
        Ty::Pattern(Box::new(Ty::Dict(
            Box::new(Ty::keyword()),
            Box::new(Ty::Any)
        )))
    );
    let thunk = Scheme::parse("fn 'a (fn -> 'b) -> nil").expect("parses");
    assert_eq!(thunk.ty.to_string(), "fn 't0 (fn -> 't1) -> nil");
}

#[test]
fn scheme_parse_rejects_bad_notation() {
    for bad in [
        "", "fn int", "[int", "[int: ]", "pattern", "wat", "int int", "'", "(int",
    ] {
        assert_eq!(Scheme::parse(bad), None, "{bad:?}");
    }
    let deep = format!("{}int{}", "[".repeat(200), "]".repeat(200));
    assert_eq!(Scheme::parse(&deep), None, "nesting past the guard");
}

#[test]
fn ty_display() {
    let t = Ty::func(
        vec![Ty::Pattern(Box::new(Ty::Sound)), Ty::Opt(Box::new(Ty::Int))],
        Ty::Dict(Box::new(Ty::keyword()), Box::new(Ty::Path)),
    );
    assert_eq!(t.to_string(), "fn (pattern sound) ?int -> [keyword: path]");
    assert_eq!(
        Ty::KeywordOf(KeySet::of(["a", "b"])).to_string(),
        "keyword{:a :b}"
    );
}

#[test]
fn check_env_lookups() {
    let mut env = CheckEnv::empty();
    assert!(env.global("x").is_none());
    let info = GlobalInfo {
        kind: BindKind::Var,
        scheme: Some(Scheme::mono(Ty::Int)),
        mask: None,
        span: None,
    };
    env.globals.insert("x".into(), info.clone());
    env.qualified
        .entry("pads".into())
        .or_default()
        .insert("warm".into(), info.clone());
    env.opens
        .push(("pads".into(), ["warm".into()].into_iter().collect()));
    assert_eq!(env.global("x"), Some(&info));
    assert_eq!(env.qualified("pads", "warm"), Some(&info));
    assert_eq!(env.qualified("pads", "cold"), None);
    assert_eq!(env.open_prefix("warm").map(|p| &**p), Some("pads"));
    assert_eq!(env.open_prefix("x"), None);
}
