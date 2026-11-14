use crate::complete::{complete, ContextKind, Snapshot};
fn at(src: &str) -> (String, usize) {
    let p = src.find('|').unwrap_or(src.len());
    (src.replace('|', ""), p)
}
fn context(src: &str) -> crate::complete::Completion {
    let (t, p) = at(src);
    complete(&t, p, &Snapshot::builtin(), 100)
}
#[test]
fn contexts_cover_head_argument_pipe_keyword_and_qualified() {
    assert_eq!(context("|").context, ContextKind::Head);
    assert_eq!(context("{m|").context, ContextKind::Head);
    assert_eq!(context("x -> pr|").context, ContextKind::Head);
    assert_eq!(context("s :bd > ga|").context, ContextKind::PipeTarget);
    assert_eq!(context("x\n\t> ga|").context, ContextKind::PipeTarget);
    assert_eq!(context("{> a|").context, ContextKind::Argument);
    assert_eq!(context("foo 1 ba|").context, ContextKind::Argument);
    assert_eq!(context("s :a|").context, ContextKind::Keyword);
    let keyword = context("s :a|");
    assert_eq!((keyword.from, keyword.to), (2, 4));
    let head = context("{m|");
    assert_eq!((head.from, head.to), (1, 2));
    assert_eq!(context("foo 1 ba|").from, 6);
    let mut snap = Snapshot::builtin();
    snap.packages.push(crate::complete::PackageNames {
        prefix: "drums".into(),
        path: "github.com/x/drums".into(),
        names: vec!["kick".into()],
    });
    let qualified = complete("drums.k", 7, &snap, 100);
    assert_eq!(qualified.context, ContextKind::Qualified);
    assert_eq!((qualified.from, qualified.to), (0, 7));
}
#[test]
fn strings_comments_definitions_and_pair_values_are_suppressed() {
    for src in ["\"ab|c\"", "# co|", "12|", "let na|", "fn f pa|", "gain:|"] {
        assert_eq!(context(src).context, ContextKind::None, "{src}");
    }
    assert_ne!(context("\"{fo|}\"").context, ContextKind::None);
    assert_eq!(context("gain: |").context, ContextKind::Argument);
}
#[test]
fn pair_key_after_native_arity() {
    let found = crate::types::natives::NativeTable::global()
        .iter()
        .find(|(_, s)| !s.keywords.is_empty() && s.max_args.is_some_and(|k| k >= 1));
    assert!(
        found.is_some(),
        "native table must provide keyed fixed arity"
    );
    let Some((_, sig)) = found else { return };
    let keyword = sig.keywords.first().copied().unwrap_or("");
    let prefix = keyword.chars().next().unwrap_or('x');
    let mut source = sig.name.to_string();
    for _ in 0..sig.max_args.unwrap_or(0) {
        source.push_str(" 1");
    }
    source.push(' ');
    source.push(prefix);
    source.push('|');
    let completed = context(&source);
    assert_eq!(completed.context, ContextKind::PairKey);
    assert!(completed
        .items
        .iter()
        .any(|item| item.label == format!("{keyword}:")));

    let fewer = format!("{} {prefix}|", sig.name);
    assert_eq!(context(&fewer).context, ContextKind::Argument);
}

#[test]
fn metadata_pair_key_is_prioritized_over_a_local() {
    let declaration = crate::dsp::meta::all()
        .iter()
        .find(|decl| decl.params.len() >= 2)
        .expect("metadata must contain a declaration with two parameters");
    let head = declaration.name;
    let first = declaration
        .params
        .first()
        .map(|p| p.name)
        .unwrap_or("first");
    let second = declaration
        .params
        .get(1)
        .map(|p| p.name)
        .unwrap_or("second");
    let prefix = second.chars().next().unwrap_or('x');
    let source = format!("fn f {second}:\n\t{head} 1 {first}: 2 {prefix}|");
    let (text, cursor) = at(&source);
    let result = complete(&text, cursor, &Snapshot::builtin(), 100);
    assert_eq!(result.context, ContextKind::PairKey);
    let key_position = result
        .items
        .iter()
        .position(|item| item.label == format!("{second}:"));
    let local_position = result.items.iter().position(|item| item.label == second);
    assert!(key_position.is_some());
    assert!(local_position.is_some());
    assert!(key_position < local_position);
}
