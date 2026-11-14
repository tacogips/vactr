//! Full event identity, separate from random seed identity and query clipping.
use crate::pattern::query::Event;
use crate::value::intern::KwId;
use crate::value::ratio::Ratio64;
use crate::value::value::{Sound, Value};
use crate::vm::fail::{FailCode, Failure};
use std::cell::Cell;

/// Root immutable edit revision. Never used as a random seed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PartRevision(pub u64);
/// Active frozen snapshot generation, stamped on host commands and playing IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotEpoch(pub u64);
/// Stable structural seed key. Equal draws do not imply equal event identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeedIdentity(pub u64);

thread_local! { static NEXT_REVISION: Cell<u64> = const { Cell::new(1) }; }
impl PartRevision {
    pub(crate) fn fresh() -> Result<Self, Failure> {
        NEXT_REVISION.with(|next| {
            let revision = next.get();
            let successor = revision.checked_add(1).ok_or_else(|| {
                Failure::new(FailCode::Overflow, "part revision capacity exhausted")
            })?;
            next.set(successor);
            Ok(Self(revision))
        })
    }
}
/// Complete source tree/step branch traversal ordinals; NOT compact NodeId
/// hashes, byte spans or positions in a filtered query result. Song-only
/// realization supplies each source leaf and structural branch explicitly.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OccurrencePath {
    pub producer_ordinals: Vec<u32>,
    pub cycle: i64,
    pub onset: Ratio64,
}
/// Full sequence-child and repeat-iteration ordinals, from root to leaf.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlacementPath(pub Vec<u32>);

/// Opaque certified realization handle. Public callers may retain/clone it,
/// but cannot manufacture a same-revision occurrence that never existed.
///
/// ```compile_fail
/// use vactr::song::EventHandle;
/// let forged = EventHandle { revision: todo!(), track: todo!(),
///     placement: todo!(), occurrence: todo!(), tone: 0 };
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventHandle {
    revision: PartRevision,
    track: KwId,
    placement: PlacementPath,
    occurrence: OccurrencePath,
    tone: u32,
}
impl EventHandle {
    // SONG-04 issues these only after successful canonical realization.
    #[allow(dead_code)]
    pub(crate) fn issue(
        revision: PartRevision,
        track: KwId,
        placement: PlacementPath,
        occurrence: OccurrencePath,
        tone: u32,
    ) -> Self {
        Self {
            revision,
            track,
            placement,
            occurrence,
            tone,
        }
    }
    /// Part revision on which the occurrence was enumerated.
    #[must_use]
    pub const fn revision(&self) -> PartRevision {
        self.revision
    }
    /// Source track identity.
    #[must_use]
    pub const fn track(&self) -> KwId {
        self.track
    }
    /// Full nested finite placement identity.
    #[must_use]
    pub fn placement(&self) -> &PlacementPath {
        &self.placement
    }
    /// Unclipped source occurrence identity.
    #[must_use]
    pub fn occurrence(&self) -> &OccurrencePath {
        &self.occurrence
    }
    /// Chord tone ordinal (identical pitches remain distinct).
    #[must_use]
    pub const fn tone(&self) -> u32 {
        self.tone
    }
}
/// Playing identity adds the snapshot epoch to the complete local handle.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SongEventId {
    pub epoch: SnapshotEpoch,
    pub handle: EventHandle,
}

/// Frozen whole sound family before bank index selection. Duplicate entries
/// are removed by full Sound equality; no hash substitutes for that equality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentSelector {
    family: Vec<Sound>,
}
impl InstrumentSelector {
    /// Builds a nonempty audio-only frozen family.
    /// # Errors
    /// Rejects an empty family or external MIDI/OSC output.
    pub fn new(family: Vec<Sound>) -> Result<Self, Failure> {
        let mut unique = Vec::new();
        for sound in family {
            if matches!(sound, Sound::MidiOut(_) | Sound::Osc(_)) {
                return Err(Failure::new(
                    FailCode::Type,
                    "song selectors require audio instruments",
                ));
            }
            if !unique.contains(&sound) {
                unique.push(sound);
            }
        }
        if unique.is_empty() {
            return Err(Failure::new(FailCode::Type, "instrument family is empty"));
        }
        Ok(Self { family: unique })
    }
    /// Complete source family, without exposing mutation.
    #[must_use]
    pub fn family(&self) -> &[Sound] {
        &self.family
    }
    /// Matches full pre-transformation sound identity.
    #[must_use]
    pub fn contains(&self, sound: &Sound) -> bool {
        self.family.contains(sound)
    }
}
/// Selected family and declared private effect-chain template. This is typed
/// route data, never a reserved user control or a shared master send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstrumentRoute {
    pub family: InstrumentSelector,
    pub template: KwId,
}

/// A single resolved note preserving the original numeric precision/type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResolvedNote {
    Int(i64),
    Ratio(Ratio64),
    Float32(f32),
    Float64(f64),
}
impl TryFrom<&Value> for ResolvedNote {
    type Error = Failure;
    fn try_from(value: &Value) -> Result<Self, Failure> {
        let note = match value {
            Value::Int(v) => Self::Int(i64::from(*v)),
            Value::Int64(v) => Self::Int(*v),
            Value::Ratio(v) => Self::Ratio(*v),
            Value::Float(v) => Self::Float32(*v),
            Value::Float64(v) => Self::Float64(*v),
            _ => {
                return Err(Failure::new(
                    FailCode::Type,
                    "song note must be a single number",
                ))
            }
        };
        note.validate()?;
        Ok(note)
    }
}
impl ResolvedNote {
    /// Rejects nonfinite numeric notes.
    pub fn validate(self) -> Result<(), Failure> {
        if matches!(self, Self::Float32(v) if !v.is_finite())
            || matches!(self, Self::Float64(v) if !v.is_finite())
        {
            return Err(Failure::new(FailCode::Type, "song note must be finite"));
        }
        Ok(())
    }
    /// Numeric note for existing host commit, only at that boundary.
    #[must_use]
    pub fn to_f64(self) -> f64 {
        match self {
            Self::Int(v) => v as f64,
            Self::Ratio(v) => v.to_f64(),
            Self::Float32(v) => f64::from(v),
            Self::Float64(v) => v,
        }
    }
}
/// Explicit commit contract: realization already split all chord tones.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteCommitMode {
    Mono,
}
/// A canonical individually addressable event; `event.part` may later be
/// clipped, but clipping never changes its handle or mono commit contract.
#[derive(Clone, Debug)]
pub struct SongEvent {
    pub handle: EventHandle,
    pub track: KwId,
    pub instrument: Sound,
    pub event: Event,
    pub placement: PlacementPath,
    pub tone: Option<ResolvedNote>,
    pub commit_mode: NoteCommitMode,
    /// Selected private branch routing; unaffected events retain `None`.
    pub route: Option<InstrumentRoute>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::pat::{Pat, PatNode};
    use crate::pattern::step::Step;
    use crate::song::{capture_part, PartEdit};
    use crate::value::intern::intern_kw;
    use std::collections::BTreeMap;
    use std::rc::Rc;
    fn base() -> Rc<crate::song::Part> {
        let pat = Rc::new(Pat::new(PatNode::Pure(Step::bare(Value::Nil)), None, false));
        Rc::new(capture_part(BTreeMap::from([(intern_kw("drums"), pat)]), Ratio64::ONE).unwrap())
    }
    fn handle(part: &crate::song::Part) -> EventHandle {
        EventHandle::issue(
            part.revision(),
            intern_kw("drums"),
            PlacementPath(Vec::new()),
            OccurrencePath {
                producer_ordinals: vec![0, 3, 1],
                cycle: 0,
                onset: Ratio64::ZERO,
            },
            0,
        )
    }
    #[test]
    fn full_handles_and_epochs_distinguish_all_identity_components() {
        let base = base();
        let first = handle(&base);
        let mut twins = Vec::new();
        let mut h = first.clone();
        h.track = intern_kw("hats");
        twins.push(h);
        let mut h = first.clone();
        h.placement.0.push(0);
        twins.push(h);
        let mut h = first.clone();
        h.occurrence.producer_ordinals.push(0);
        twins.push(h);
        let mut h = first.clone();
        h.occurrence.cycle = 1;
        twins.push(h);
        let mut h = first.clone();
        h.occurrence.onset = Ratio64::ONE;
        twins.push(h);
        let mut h = first.clone();
        h.tone = 1;
        twins.push(h);
        assert!(twins.iter().all(|h| h != &first));
        assert_ne!(
            SongEventId {
                epoch: SnapshotEpoch(1),
                handle: first.clone()
            },
            SongEventId {
                epoch: SnapshotEpoch(2),
                handle: first
            }
        );
    }
    #[test]
    fn typed_route_metadata_preserves_original_event_and_controls() {
        let part = base();
        let sound = Sound::Builtin(intern_kw("bd"));
        let whole = crate::pattern::query::TimeSpan::cycle(0).unwrap();
        let mut event = Event::new(
            Some(whole),
            whole,
            Value::Sound(Rc::new(sound.clone())),
            None,
        );
        event.controls.insert(intern_kw("gain"), Value::Float(0.5));
        let original = SongEvent {
            handle: handle(&part),
            track: intern_kw("drums"),
            instrument: sound.clone(),
            event,
            placement: PlacementPath(Vec::new()),
            tone: Some(ResolvedNote::Float64(60.25)),
            commit_mode: NoteCommitMode::Mono,
            route: None,
        };
        let mut routed = original.clone();
        routed.route = Some(InstrumentRoute {
            family: InstrumentSelector::new(vec![sound]).unwrap(),
            template: intern_kw("dark"),
        });
        assert!(original.route.is_none());
        assert_eq!(routed.handle, original.handle);
        assert_eq!(routed.tone, original.tone);
        assert_eq!(routed.commit_mode, original.commit_mode);
        assert_eq!(routed.event.controls.len(), original.event.controls.len());
        assert!(crate::value::deep_eq(
            &routed.event.controls[&intern_kw("gain")],
            &original.event.controls[&intern_kw("gain")]
        )
        .unwrap());
        assert_eq!(routed.event.whole, original.event.whole);
    }

    #[test]
    fn edit_revision_changes_and_old_handles_are_rejected() {
        let base = base();
        let h = handle(&base);
        let edited = Rc::clone(&base)
            .edit(PartEdit::DeleteEvent(h.clone()))
            .unwrap();
        assert_ne!(base.revision(), edited.revision());
        assert_eq!(base.seed_identity(), edited.seed_identity());
        assert!(Rc::new(edited).edit(PartEdit::DeleteEvent(h)).is_err());
    }
}
