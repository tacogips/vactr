//! Immutable finite symbolic music. Lazy `Pat` values remain unchanged.
//!
//! Construction runs on the control thread. No PCM, pattern queries, VM
//! callbacks or repeat expansion are performed by these value constructors.

pub mod assets;
#[path = "song.rs"]
mod descriptor;
pub mod edit;
pub mod identity;
pub mod limits;
pub mod part;
pub mod query;
pub mod routing;
pub mod snapshot;
pub mod source;
pub mod source_uses;

pub use descriptor::{Song, SongSettings};
pub use identity::{
    EventHandle, InstrumentRoute, InstrumentSelector, NoteCommitMode, OccurrencePath, PartRevision,
    PlacementPath, ResolvedNote, SeedIdentity, SnapshotEpoch, SongEvent, SongEventId,
};
pub use limits::{FrameEndpoints, SongLimits};
pub use part::{
    capture_part, checked_repeat_count, part_repeat, sequence, Part, PartEdit, PartNode,
    RepeatSeedMode,
};
pub use source::SongSource;

pub use edit::{
    delete_event, instrument_fx, overwrite_region, replace_track, transform_instrument,
    SongBuildCtx,
};
pub use query::{query_part, SongQueryCtx};

pub use snapshot::{
    prepare_song, PreparedSong, SongApplyAck, SongCandidate, SongPreparationState,
    SongResourceLease, SongSnapshot,
};

/// Native complete-song streaming export through the original finite owner.
#[cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
pub mod export;
#[cfg(all(feature = "host-native", not(target_arch = "wasm32")))]
mod wav;
