//! Immutable selective edits; callbacks run once during pure construction.
use super::{EventHandle, InstrumentSelector, Part, PartEdit, SongLimits, SongSource};
use crate::pattern::eval::QueryVm;
use crate::pattern::pat::{Pat, PatNode};
use crate::pattern::query::TimeSpan;
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};
use std::rc::Rc;

/// A synchronous effect-restricted construction borrow, not a transport snapshot.
/// The QueryVm implementation must reject effects; emitted output is rejected
/// here and the caller's preexisting captured output is restored on every path.
pub struct SongBuildCtx<'a> {
    pub vm: &'a mut dyn QueryVm,
    pub limits: &'a SongLimits,
}
/// Replace an existing root track, retaining duration and source randomness.
/// # Errors
/// Unknown track or exceeded construction limits.
pub fn replace_track(part: &Part, track: KwId, replacement: Rc<Pat>) -> Result<Part, Failure> {
    Rc::new(part.clone()).edit(PartEdit::ReplaceTrack {
        track,
        pattern: replacement,
    })
}
/// Invoke a transform once on the selected symbolic stream and store its result.
/// # Errors
/// Invalid selection, callback failure, emitted output or invalid result pattern.
pub fn transform_instrument(
    part: &Part,
    track: KwId,
    selector: InstrumentSelector,
    transform: Value,
    cx: &mut SongBuildCtx<'_>,
) -> Result<Part, Failure> {
    part.validate_limits(cx.limits)?;
    let source = SongSource::new(Rc::new(part.clone()), track, selector.clone())?;
    let selected = Rc::new(Pat::new(PatNode::SongSource(Rc::new(source)), None, true));
    let saved = cx.vm.take_output();
    let called = cx.vm.call(&transform, &[Value::Pattern(selected)]);
    let produced = cx.vm.take_output();
    cx.vm.put_output(saved);
    let value = called?;
    if !produced.is_empty() {
        return Err(Failure::new(
            FailCode::EffectInQuery,
            "song transform emitted output",
        ));
    }
    let pattern = crate::pattern::build::pattern_of(&value, None)?;
    Rc::new(part.clone()).edit_with_limits(
        PartEdit::TransformInstrument {
            track,
            selector,
            pattern,
        },
        cx.limits,
    )
}
/// Associate a private FX template with a selected original instrument family.
/// # Errors
/// Unknown track or exceeded construction limits.
pub fn instrument_fx(
    part: &Part,
    track: KwId,
    selector: InstrumentSelector,
    template: KwId,
) -> Result<Part, Failure> {
    Rc::new(part.clone()).edit(PartEdit::InstrumentFx {
        track,
        selector,
        template,
    })
}
/// Delete one certified tone from the exact immutable source revision.
/// # Errors
/// Stale revision or unknown root track.
pub fn delete_event(part: &Part, handle: &EventHandle) -> Result<Part, Failure> {
    Rc::new(part.clone()).edit(PartEdit::DeleteEvent(handle.clone()))
}
/// Replace complete events whose onsets are in [begin,end), inserting at begin.
/// # Errors
/// Unknown track, inverted/out-of-Part region or construction limits.
pub fn overwrite_region(
    part: &Part,
    track: KwId,
    region: TimeSpan,
    replacement: Rc<Pat>,
) -> Result<Part, Failure> {
    Rc::new(part.clone()).edit(PartEdit::OverwriteRegion {
        track,
        region,
        pattern: replacement,
    })
}
