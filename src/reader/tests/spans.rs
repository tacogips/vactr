//! Byte-accurate span goldens (UTF-8, tabs, CRLF).

use crate::reader::node::{Node, TriviaKind};

fn spans(n: &Node) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    n.walk(&mut |m: &Node| {
        if m.children.is_empty() {
            out.push((m.span.start, m.span.end));
        }
    });
    out
}

#[test]
fn utf8_string_and_crlf() {
    let src = "let s \"h\u{e9}llo\" # c\r\nprint s\n";
    let r = super::rd(src);
    assert!(r.diags.is_empty());
    assert_eq!(spans(&r.nodes[0]), [(0, 3), (4, 5), (6, 14)]);
    assert_eq!(spans(&r.nodes[1]), [(20, 25), (26, 27)]);
    assert_eq!(r.trivia.items.len(), 1);
    assert_eq!(
        (r.trivia.items[0].span.start, r.trivia.items[0].span.end),
        (15, 18)
    );
    let cr = 18;
    for n in &r.nodes {
        n.walk(&mut |m: &Node| assert!(!(m.span.start..m.span.end).contains(&cr)));
    }
    assert!(r.trivia.items.iter().all(|t| t.span.end <= cr));
}

#[test]
fn tabs_are_one_byte() {
    let r = super::rd("fn f a:\n\t* a 12\n");
    assert!(r.diags.is_empty());
    assert_eq!(
        spans(&r.nodes[0]),
        [(0, 2), (3, 4), (5, 6), (9, 10), (11, 12), (13, 15)]
    );
}

#[test]
fn directive_span() {
    let r = super::rd("#@ panel: gain\nlet g 1\n");
    assert_eq!(r.trivia.items.len(), 1);
    assert_eq!(r.trivia.items[0].kind, TriviaKind::Directive);
    assert_eq!(
        (r.trivia.items[0].span.start, r.trivia.items[0].span.end),
        (0, 14)
    );
}

#[test]
fn compound_spans_cover_their_parts() {
    let src = "once {s :crash} gain: 0.5 > d1";
    let r = super::rd(src);
    let call = &r.nodes[0];
    assert_eq!((call.span.start, call.span.end), (0, src.len() as u32));
    let inner = &call.children[1];
    assert_eq!((inner.span.start, inner.span.end), (0, 25));
    let pair = &inner.children[2];
    assert_eq!(
        &src[pair.span.start as usize..pair.span.end as usize],
        "gain: 0.5"
    );
    let group = &inner.children[1];
    assert_eq!(
        &src[group.span.start as usize..group.span.end as usize],
        "{s :crash}"
    );
}

#[test]
fn node_ids_are_preorder_from_zero() {
    let r = super::rd("f {g 1} 2\nh 3\n");
    let mut ids = Vec::new();
    for n in &r.nodes {
        n.walk(&mut |m: &Node| ids.push(m.id.get()));
    }
    let expected: Vec<u32> = (0..ids.len() as u32).collect();
    assert_eq!(ids, expected);
    assert_eq!(r.next_node_id().get(), ids.len() as u32);
}
