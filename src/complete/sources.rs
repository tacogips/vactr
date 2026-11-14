use super::{Candidate, CandidateKind, ContextKind, Snapshot};
use crate::dsp::{meta, ugen::catalog::TEMPLATE_NAMES};
use crate::reader::span::FileId;
use crate::reader::{import, layout, lexer::Tok, node::Trivia};
use crate::types::{
    natives::{NativeKind, NativeTable},
    ty::KeySet,
};

#[derive(Clone)]
pub(crate) struct Ranked {
    pub candidate: Candidate,
    pub tier: u8,
    pub depth: u8,
    pub order: usize,
}
fn add(
    out: &mut Vec<Ranked>,
    label: impl Into<String>,
    kind: CandidateKind,
    detail: impl Into<String>,
    insert: impl Into<String>,
    tier: u8,
    order: usize,
) {
    let label = label.into();
    out.push(Ranked {
        candidate: Candidate {
            insert: insert.into(),
            label,
            kind,
            detail: detail.into(),
        },
        tier,
        depth: 0,
        order,
    });
}
pub(crate) fn candidates(
    text: &str,
    cursor: usize,
    snap: &Snapshot,
    context: ContextKind,
    locals: &[Candidate],
) -> Vec<Ranked> {
    let mut out = Vec::new();
    let line_start = text
        .get(..cursor)
        .and_then(|s| s.rfind('\n'))
        .map_or(0, |n| n + 1);
    let current = text.get(line_start..cursor).unwrap_or("");
    let word_start = current
        .rfind(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | ':')))
        .map_or(0, |n| n + 1);
    let prefix = current.get(word_start..).unwrap_or("");
    if context == ContextKind::Keyword {
        let name = prefix.strip_prefix(':').unwrap_or(prefix);
        let sound_head =
            current.trim_start().starts_with("s ") || current.trim_start().starts_with("sound ");
        for (set, kind) in [
            (&snap.manifest.sounds, "sound"),
            (&snap.manifest.synths, "synth"),
            (&snap.manifest.controls, "control"),
        ] {
            if let KeySet::Of(keys) = set {
                for key in keys {
                    add(
                        &mut out,
                        format!(":{key}"),
                        CandidateKind::Keyword,
                        kind,
                        format!(":{key}"),
                        0,
                        if sound_head && kind == "control" {
                            1
                        } else {
                            0
                        },
                    );
                }
            }
        }
        for name in TEMPLATE_NAMES {
            add(
                &mut out,
                format!(":{name}"),
                CandidateKind::Keyword,
                "template",
                format!(":{name}"),
                0,
                0,
            );
        }
        for d in &snap.document {
            if d.label.starts_with("inst ") {
                let name = d.label.trim_start_matches("inst ");
                add(
                    &mut out,
                    format!(":{name}"),
                    CandidateKind::Keyword,
                    "document synth",
                    format!(":{name}"),
                    0,
                    0,
                );
            }
        }
        let _ = name;
        return out;
    }
    if context == ContextKind::Qualified {
        if let Some((prefix, name)) = prefix.split_once('.') {
            for pkg in &snap.packages {
                if pkg.prefix == prefix {
                    for n in &pkg.names {
                        add(
                            &mut out,
                            format!("{prefix}.{n}"),
                            CandidateKind::Qualified,
                            &pkg.path,
                            format!("{prefix}.{n}"),
                            0,
                            0,
                        );
                    }
                }
            }
            let _ = name;
        }
        return out;
    }
    if context == ContextKind::PairKey {
        if let Some(head) = current.split_ascii_whitespace().next() {
            if let Some(sig) = NativeTable::global().get(head) {
                for key in sig.1.keywords {
                    add(
                        &mut out,
                        format!("{key}:"),
                        CandidateKind::Key,
                        "native keyword",
                        format!("{key}: "),
                        0,
                        0,
                    );
                }
            }
            if let Some(decl) = meta::decl_for(head) {
                for param in &decl.params {
                    add(
                        &mut out,
                        format!("{}:", param.name),
                        CandidateKind::Key,
                        "control",
                        format!("{}: ", param.name),
                        0,
                        0,
                    );
                }
            }
            for key in document_header_keys(text, head) {
                add(
                    &mut out,
                    format!("{key}:"),
                    CandidateKind::Key,
                    "document parameter",
                    format!("{key}: "),
                    0,
                    0,
                );
            }
        }
    }
    if context == ContextKind::PipeTarget {
        if let Some((seed, _)) = seed_sound(text, cursor) {
            if let Some(decl) = meta::decl_for(&seed) {
                for p in &decl.params {
                    add(
                        &mut out,
                        p.name,
                        CandidateKind::Control,
                        &seed,
                        p.name,
                        0,
                        0,
                    );
                }
            } else {
                for key in document_header_keys(text, &seed) {
                    add(&mut out, &key, CandidateKind::Control, &seed, &key, 0, 0);
                }
            }
        }
    }
    for local in locals {
        add(
            &mut out,
            local.label.clone(),
            local.kind,
            local.detail.clone(),
            local.insert.clone(),
            1,
            0,
        );
    }
    let mut trivia = Trivia::default();
    let mut lines = layout::split_lines(text, FileId::new(1), &mut trivia);
    let stmts = layout::statements(&mut lines);
    for stmt in stmts {
        if let Some(t) = stmt.tokens.get(1) {
            if let Tok::Ident(name) = &t.kind {
                if matches!(stmt.tokens.first().map(|t|&t.kind),Some(Tok::Ident(k)) if matches!(&**k,"let"|"var"|"fn"|"inst"|"struct"|"enum"|"bus"|"look"))
                    && !snap.document.iter().any(|doc| doc.label == **name)
                {
                    add(
                        &mut out,
                        name.to_string(),
                        CandidateKind::Value,
                        "document",
                        name.to_string(),
                        2,
                        0,
                    );
                }
            }
        }
    }
    for d in &snap.document {
        add(
            &mut out,
            d.label.clone(),
            d.kind,
            d.detail.clone(),
            d.label.clone(),
            2,
            0,
        );
    }
    for (_, sig) in NativeTable::global().iter() {
        let kind = match sig.kind {
            NativeKind::Function => CandidateKind::Function,
            NativeKind::Value => CandidateKind::Value,
        };
        add(
            &mut out,
            sig.name,
            kind,
            sig.ty.first().copied().unwrap_or("prelude"),
            sig.name,
            3,
            0,
        );
    }
    for imp in import::prescan_imports(text) {
        add(
            &mut out,
            imp.prefix.to_string(),
            CandidateKind::Module,
            imp.path.to_string(),
            imp.prefix.to_string(),
            4,
            0,
        );
    }
    for pkg in &snap.packages {
        for name in &pkg.names {
            add(
                &mut out,
                format!("{}.{}", pkg.prefix, name),
                CandidateKind::Qualified,
                &pkg.path,
                format!("{}.{}", pkg.prefix, name),
                4,
                0,
            );
        }
    }
    let _ = (prefix, line_start);
    out
}

fn seed_sound(text: &str, cursor: usize) -> Option<(String, String)> {
    let before = text.get(..cursor)?;
    let lines = before.lines().collect::<Vec<_>>();
    for line in lines.iter().rev() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('>') || trimmed.starts_with('#') {
            continue;
        }
        let mut words = trimmed.split_ascii_whitespace();
        let head = words.next()?;
        if !matches!(head, "s" | "sound") {
            return None;
        }
        let keyword = words.next()?.strip_prefix(':')?;
        return Some((keyword.to_owned(), head.to_owned()));
    }
    None
}

fn document_header_keys(text: &str, name: &str) -> Vec<String> {
    let mut trivia = Trivia::default();
    let mut lines = layout::split_lines(text, FileId::new(1), &mut trivia);
    let statements = layout::statements(&mut lines);
    statements.iter().find_map(|stmt| {
        let declaration = matches!(stmt.tokens.first().map(|t| &t.kind), Some(Tok::Ident(k)) if matches!(&**k, "fn" | "inst"));
        let matching = matches!(stmt.tokens.get(1).map(|t| &t.kind), Some(Tok::Ident(n)) if &**n == name);
        if !declaration || !matching { return None; }
        let mut keys = Vec::new();
        for (i, token) in stmt.tokens.iter().enumerate().skip(2) {
            if let Tok::Ident(key) = &token.kind {
                if stmt.tokens.get(i + 1).is_some_and(|next| matches!(next.kind, Tok::PairColon)) {
                    keys.push(key.to_string());
                }
            }
        }
        Some(keys)
    }).unwrap_or_default()
}
