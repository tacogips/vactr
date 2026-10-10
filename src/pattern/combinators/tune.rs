//! Pattern combinator for applying a tuning to event controls.

use std::rc::Rc;

use crate::pattern::combinators::control::query_mapped;
use crate::pattern::combinators::kw;
use crate::pattern::eval::QState;
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::query::{Event, TimeSpan};
use crate::pattern::tuning::{Mapping, Tuning, TUNING_CONTROL};
use crate::reader::span::Span;

/// Applies `tunings` per event. A structured tuning pattern can give an
/// otherwise unstructured subject its event structure.
#[must_use]
pub fn tune(tunings: Rc<Pat>, subject: Rc<Pat>, mapping: Mapping, span: Option<Span>) -> Pat {
    let structured = subject.structured || tunings.structured;
    Pat::new(
        PatNode::Tune {
            tunings,
            subject,
            mapping,
        },
        span,
        structured,
    )
}

pub(crate) fn query_tune(
    tunings: &Pat,
    subject: &Pat,
    mapping: Mapping,
    p: &Pat,
    span: TimeSpan,
    st: &mut QState<'_, '_>,
) -> Vec<Event> {
    query_mapped(kw(TUNING_CONTROL), tunings, subject, p, span, st, |value| {
        Ok(Tuning::from_spec(value)?
            .with_mapping(&mapping)?
            .to_control())
    })
}

#[cfg(test)]
mod tests;
