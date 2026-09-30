//! Deterministic mutation coverage for arbitrary formatter inputs.

use super::corpus::committed_sources;
use super::{check_guarantees, FIXTURES};
use crate::fmt::{format, Outcome};
use crate::reader::import::AliasEnv;
use crate::reader::read;
use crate::reader::span::FileId;
use crate::types::diag::Severity;

const MUTATIONS: [&str; 8] = ["\t", " ", ">", ":", "#@", "\n", "\"", "{"];

#[test]
fn two_thousand_deterministic_mutants_obey_the_guarantees() {
    let mut sources: Vec<String> = FIXTURES
        .iter()
        .map(|(_, source)| (*source).to_owned())
        .collect();
    sources.extend(committed_sources().into_iter().map(|(_, source)| source));
    assert!(!sources.is_empty());

    let mut random = XorShift64(0x8f3d_9a71_42c6_b50d);
    for iteration in 0..2_000usize {
        let source_index = if iteration < sources.len() {
            iteration
        } else {
            random.index(sources.len())
        };
        let source = sources.get(source_index).map(String::as_str).unwrap_or("");
        let token = MUTATIONS[random.index(MUTATIONS.len())];
        let mutant = mutate(source, token, random.next());
        let formatted = format(&mutant);
        let parsed = read(&mutant, FileId::new(1), &AliasEnv::new());
        let refused = parsed
            .diags
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error);
        if refused {
            match formatted.outcome {
                Outcome::Refused => assert_eq!(formatted.text, mutant, "iteration {iteration}"),
                Outcome::Changed => {
                    let repaired = read(&formatted.text, FileId::new(1), &AliasEnv::new());
                    assert!(
                        repaired
                            .diags
                            .iter()
                            .all(|diagnostic| diagnostic.severity != Severity::Error),
                        "changed mutant has reader errors at iteration {iteration}"
                    );
                    for line in crate::fmt::lines::split(&formatted.text, FileId::new(1)) {
                        if line.is_code() {
                            assert!(
                                !formatted.text[line.start..line.indent_end].contains(' '),
                                "changed mutant has spaces in code indentation at iteration {iteration}"
                            );
                        }
                    }
                }
                Outcome::Unchanged => {
                    panic!("reader-error mutant became unchanged at iteration {iteration}")
                }
            }
        } else {
            check_guarantees(&mutant);
        }
    }
}

fn mutate(source: &str, token: &str, selector: u64) -> String {
    if selector & 1 == 0 {
        let boundaries: Vec<usize> = source
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(source.len()))
            .collect();
        let at = boundaries
            .get(index_from(selector >> 1, boundaries.len()))
            .copied()
            .unwrap_or(0);
        let mut result = String::with_capacity(source.len().saturating_add(token.len()));
        result.push_str(&source[..at]);
        result.push_str(token);
        result.push_str(&source[at..]);
        result
    } else {
        let occurrences: Vec<usize> = source
            .match_indices(token)
            .map(|(index, _)| index)
            .collect();
        if occurrences.is_empty() {
            format!("{source}{token}")
        } else {
            let at = occurrences
                .get(index_from(selector >> 1, occurrences.len()))
                .copied()
                .unwrap_or(0);
            let end = at.saturating_add(token.len());
            format!("{}{}", &source[..at], &source[end..])
        }
    }
}

fn index_from(value: u64, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        usize::try_from(value % u64::try_from(len).unwrap_or(u64::MAX)).unwrap_or(0)
    }
}

struct XorShift64(u64);

impl XorShift64 {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn index(&mut self, len: usize) -> usize {
        index_from(self.next(), len)
    }
}
