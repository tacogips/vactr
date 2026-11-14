use super::sources::Ranked;
use super::{Candidate, ContextKind};
fn class(label: &str, needle: &str) -> Option<u8> {
    let l = label.to_ascii_lowercase();
    let n = needle.to_ascii_lowercase();
    if n.is_empty() || l.starts_with(&n) {
        return Some(0);
    }
    let boundaries: Vec<usize> = l
        .char_indices()
        .filter_map(|(i, _c)| {
            if i == 0 || matches!(l.as_bytes().get(i.wrapping_sub(1)), Some(b'-' | b'.')) {
                Some(i)
            } else {
                None
            }
        })
        .collect();
    if boundaries
        .iter()
        .any(|i| l.get(*i..).is_some_and(|s| s.starts_with(&n)))
    {
        return Some(1);
    }
    let mut it = l.char_indices();
    let mut start = true;
    let mut first = true;
    for c in n.chars() {
        let found = it.find(|(_, x)| *x == c);
        let (i, _) = found?;
        if first && !boundaries.contains(&i) {
            return None;
        }
        if start {
            start = false
        }
        first = false;
    }
    Some(2)
}
pub(crate) fn rank(
    items: &mut Vec<Ranked>,
    prefix: &str,
    context: ContextKind,
    limit: usize,
) -> (Vec<Candidate>, bool) {
    let needle = if context == ContextKind::Keyword {
        prefix.strip_prefix(':').unwrap_or(prefix)
    } else if context == ContextKind::Qualified {
        prefix.rsplit('.').next().unwrap_or(prefix)
    } else {
        prefix
    };
    let mut best = std::collections::BTreeMap::<String, (Ranked, u8)>::new();
    for item in items.drain(..) {
        let match_label = if context == ContextKind::Keyword {
            item.candidate
                .label
                .strip_prefix(':')
                .unwrap_or(&item.candidate.label)
        } else if context == ContextKind::Qualified {
            item.candidate
                .label
                .rsplit('.')
                .next()
                .unwrap_or(&item.candidate.label)
        } else {
            &item.candidate.label
        };
        let Some(cls) = class(match_label, needle) else {
            continue;
        };
        let key = item.candidate.label.to_ascii_lowercase();
        match best.get(&key) {
            Some((old, _)) if (old.tier, old.depth) <= (item.tier, item.depth) => {}
            _ => {
                best.insert(key, (item, cls));
            }
        }
    }
    let mut v: Vec<_> = best.into_values().collect();
    v.sort_by(|(a, ca), (b, cb)| {
        (*ca, a.tier, a.depth, a.order, a.candidate.label.as_bytes()).cmp(&(
            *cb,
            b.tier,
            b.depth,
            b.order,
            b.candidate.label.as_bytes(),
        ))
    });
    let incomplete = v.len() > limit;
    (
        v.into_iter()
            .take(limit)
            .map(|(r, _)| r.candidate)
            .collect(),
        incomplete,
    )
}
