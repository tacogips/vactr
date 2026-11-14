//! Indentation levels for code and comment-only lines (design sections 3.4–3.5).

use std::cmp::Ordering;

use crate::fmt::lines::{Line, LineKind};

#[derive(Clone, Copy)]
struct Statement {
    original: u32,
    formatted: u32,
}

struct Frame {
    original_body: u32,
    formatted_body: u32,
}

/// Assigns formatted levels to code lines with the reader's block-frame model.
///
/// `targets` is the sorted list of `#@` attach-target start offsets; the
/// comment-only chain is built from those targets so that directive placement
/// is preserved (guarantee G6). Merged `elif`/`else` lines are not targets.
pub(crate) fn assign_code_levels(lines: &mut [Line], targets: &[usize]) {
    let mut frames = vec![Frame {
        original_body: 0,
        formatted_body: 0,
    }];
    let mut stack: Vec<Statement> = Vec::new();
    let mut chains: Vec<Option<Vec<Statement>>> = vec![None; lines.len()];

    for (index, line) in lines.iter_mut().enumerate() {
        let LineKind::Code { opens_block, .. } = line.kind else {
            continue;
        };
        let original = line.original_level;
        while frames.len() > 1
            && frames
                .last()
                .is_some_and(|frame| frame.original_body > original)
        {
            frames.pop();
        }

        // The root frame is always present. If an impossible malformed
        // indentation reaches this helper, it is treated as a statement.
        let Some(top) = frames.last() else {
            continue;
        };
        let formatted = match original.cmp(&top.original_body) {
            Ordering::Equal | Ordering::Less => top.formatted_body,
            Ordering::Greater => top.formatted_body.saturating_add(1),
        };
        line.formatted_level = Some(formatted);
        if opens_block {
            frames.push(Frame {
                original_body: original.saturating_add(1),
                formatted_body: formatted.saturating_add(1),
            });
        }
        if targets.binary_search(&line.indent_end).is_ok() {
            while stack.last().is_some_and(|item| item.original >= original) {
                stack.pop();
            }
            stack.push(Statement {
                original,
                formatted,
            });
        }
        chains[index] = Some(stack.clone());
    }

    let mut previous_chain: Option<Vec<Statement>> = None;
    let mut index = 0usize;
    while index < lines.len() {
        if lines[index].is_code() {
            previous_chain = chains.get(index).cloned().flatten();
            index = index.saturating_add(1);
            continue;
        }
        if !matches!(lines[index].kind, LineKind::Comment { .. }) {
            index = index.saturating_add(1);
            continue;
        }

        let run_start = index;
        while index < lines.len() && matches!(lines[index].kind, LineKind::Comment { .. }) {
            index = index.saturating_add(1);
        }
        let next_level = lines
            .iter()
            .skip(index)
            .find(|line| line.is_code())
            .and_then(|line| line.formatted_level)
            .unwrap_or(0);
        for comment in lines.iter_mut().take(index).skip(run_start) {
            comment.formatted_level = Some(comment_level(
                previous_chain.as_deref(),
                comment.indent_width,
                next_level,
            ));
        }
    }
}

/// Applies the 3.5 chain clamp for one comment-only line.
fn comment_level(chain: Option<&[Statement]>, width: u32, next_level: u32) -> u32 {
    let Some(chain) = chain.filter(|items| !items.is_empty()) else {
        return 0;
    };
    let Some(index) = chain.iter().rposition(|item| item.original <= width) else {
        return 0;
    };
    let statement = chain.get(index).copied();
    let Some(statement) = statement else {
        return 0;
    };
    let upper = chain
        .get(index.saturating_add(1))
        .map_or(statement.formatted.max(next_level), |next| {
            next.formatted.saturating_sub(1)
        });
    statement
        .formatted
        .saturating_add(width.saturating_sub(statement.original))
        .min(upper)
}
