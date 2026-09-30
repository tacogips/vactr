use crate::complete::{
    complete, complete_bytes, Candidate, CandidateKind, Completion, ContextKind, Snapshot,
    STATUS_BAD_CURSOR, STATUS_NOT_UTF8, STATUS_OK,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};
fn sources() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let paths = Command::new("git")
        .args(["ls-files", "--", "*.vact"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut sources = paths
        .into_iter()
        .filter_map(|p| fs::read_to_string(root.join(p)).ok())
        .collect::<Vec<_>>();
    let fixtures = Command::new("git")
        .args(["ls-files", "--", "src/fmt/tests/fixtures/*.in"])
        .current_dir(root)
        .output();
    if let Ok(output) = fixtures {
        if output.status.success() {
            for path in String::from_utf8_lossy(&output.stdout).lines() {
                if let Ok(source) = fs::read_to_string(root.join(path)) {
                    sources.push(source);
                }
            }
        }
    }
    sources
}
#[test]
fn every_prefix_and_mutant_is_panic_free() {
    let all = sources();
    for src in &all {
        let mut boundaries = src.char_indices().map(|(i, _)| i).collect::<Vec<_>>();
        boundaries.push(src.len());
        for c in boundaries {
            let _ = complete(src, c, &Snapshot::builtin(), 100);
        }
    }
    let mut rng = 0x8f3d_9a71_42c6_b50du64;
    const TOKENS: [&str; 8] = ["\t", " ", ">", ":", "#@", "\n", "\"", "{"];
    for i in 0..2000 {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        let src = all
            .get(i % all.len().max(1))
            .map(String::as_str)
            .unwrap_or("");
        let mut boundaries = src.char_indices().map(|(i, _)| i).collect::<Vec<_>>();
        boundaries.push(src.len());
        let at = boundaries
            .get(
                usize::try_from(rng % u64::try_from(boundaries.len().max(1)).unwrap_or(u64::MAX))
                    .unwrap_or(0),
            )
            .copied()
            .unwrap_or(0);
        let mut s = src.to_owned();
        s.insert_str(at, TOKENS[usize::try_from(rng % 8).unwrap_or(0)]);
        let _ = complete(&s, s.len(), &Snapshot::builtin(), 100);
    }
}
#[test]
fn byte_status_json_and_cursor_normalization() {
    assert_eq!(complete_bytes(&[0xff], 0, 0).0, STATUS_NOT_UTF8);
    assert_eq!(complete_bytes(b"x", 2, 0).0, STATUS_BAD_CURSOR);
    let (status, json) = complete_bytes(b"", 0, 0);
    assert_eq!(status, STATUS_OK);
    let v: serde_json::Value = serde_json::from_slice(&json).unwrap_or_default();
    assert_eq!(v.get("v").and_then(|v| v.as_u64()), Some(1));
    assert_eq!(complete("abc", 99, &Snapshot::builtin(), 10).to, 3);
    assert_eq!(complete("日本語", 4, &Snapshot::builtin(), 10).to, 3);
}
#[test]
fn json_contract_has_exact_fields_and_wire_names() {
    let completion = Completion {
        context: ContextKind::PipeTarget,
        from: 1,
        to: 2,
        incomplete: false,
        items: vec![Candidate {
            label: "local".to_owned(),
            kind: CandidateKind::Local,
            detail: "fn".to_owned(),
            insert: "local".to_owned(),
        }],
    };
    let value: serde_json::Value = serde_json::from_str(&completion.to_json()).unwrap_or_default();
    let mut keys = value
        .as_object()
        .map(|o| o.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    keys.sort();
    assert_eq!(keys, ["context", "from", "incomplete", "items", "to", "v"]);
    assert_eq!(value.get("context").and_then(|v| v.as_str()), Some("pipe"));
    for (kind, wire) in [
        (ContextKind::None, "none"),
        (ContextKind::Keyword, "keyword"),
        (ContextKind::Qualified, "qualified"),
        (ContextKind::PipeTarget, "pipe"),
        (ContextKind::Head, "head"),
        (ContextKind::PairKey, "pair-key"),
        (ContextKind::Argument, "argument"),
    ] {
        let json = serde_json::to_value(kind).unwrap_or_default();
        assert_eq!(json.as_str(), Some(wire), "{kind:?}");
    }
    let item = value
        .get("items")
        .and_then(|v| v.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.as_object());
    let mut item_keys = item
        .map(|object| object.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    item_keys.sort();
    assert_eq!(item_keys, ["detail", "insert", "kind", "label"]);
    assert_eq!(
        item.and_then(|object| object.get("kind"))
            .and_then(|kind| kind.as_str()),
        Some("local")
    );
}
#[test]
fn budget_tripwire() {
    let text = (0..2000).map(|_| "foo bar baz\n").collect::<String>();
    let t = Instant::now();
    let _ = complete(&text, text.len(), &Snapshot::builtin(), 100);
    assert!(t.elapsed().as_millis() < 250);
}
#[test]
#[ignore]
fn budget() {
    let text = (0..2000).map(|_| "foo bar baz\n").collect::<String>();
    let mut ms = Vec::new();
    for _ in 0..20 {
        let t = Instant::now();
        let _ = complete(&text, text.len(), &Snapshot::builtin(), 100);
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    ms.sort_by(f64::total_cmp);
    let median = (ms[9] + ms[10]) / 2.0;
    println!("completion median: {median:.3} ms");
    assert!(median < 5.0);
}
