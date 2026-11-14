//! Live-input voice release (design 11.7).
//!
//! A live NoteOn starts a voice tagged with its `VoiceTag`; `VoiceRelease
//! { tag }` releases exactly that voice while it still sounds. `TagMap` is
//! the audio side's preallocated `tag -> voice` map (capacity = voices), and
//! `Tombstones` remembers the last releases that matched no voice, so a
//! NoteOn arriving after its own release is dropped instead of starting an
//! unreleasable voice. Neither allocates after construction.

use crate::host::wire::VoiceTag;

/// The tombstone ring size (design 12.8.9).
pub const TOMBSTONES: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Open {
    tag: VoiceTag,
    voice: u32,
    /// Insertion order, for oldest-open stealing.
    order: u64,
}

/// The open live-input voices.
#[derive(Clone, Debug)]
pub struct TagMap {
    entries: Box<[Option<Open>]>,
    next_order: u64,
}

impl TagMap {
    /// A map for `capacity` voices.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: vec![None; capacity].into_boxed_slice(),
            next_order: 0,
        }
    }

    /// The fixed capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.entries.len()
    }

    /// The number of open voices.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    /// True when no voice is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(Option::is_none)
    }

    /// Records that `voice` plays `tag`. Returns `false` when the map is
    /// full; the caller steals `oldest()` first.
    pub fn insert(&mut self, tag: VoiceTag, voice: u32) -> bool {
        let Some(free) = self.entries.iter_mut().find(|e| e.is_none()) else {
            return false;
        };
        *free = Some(Open {
            tag,
            voice,
            order: self.next_order,
        });
        self.next_order += 1;
        true
    }

    /// The voice playing `tag`, if it is open.
    #[must_use]
    pub fn voice_of(&self, tag: VoiceTag) -> Option<u32> {
        self.entries
            .iter()
            .flatten()
            .find(|o| o.tag == tag)
            .map(|o| o.voice)
    }

    /// Closes `tag` and returns its voice, if it was open.
    pub fn release(&mut self, tag: VoiceTag) -> Option<u32> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.is_some_and(|o| o.tag == tag))?;
        entry.take().map(|o| o.voice)
    }

    /// Closes whatever tag `voice` plays (the voice ended or was stolen).
    pub fn remove_voice(&mut self, voice: u32) -> Option<VoiceTag> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.is_some_and(|o| o.voice == voice))?;
        entry.take().map(|o| o.tag)
    }

    /// The oldest open voice: the steal candidate on pool exhaustion.
    #[must_use]
    pub fn oldest(&self) -> Option<(VoiceTag, u32)> {
        self.entries
            .iter()
            .flatten()
            .min_by_key(|o| o.order)
            .map(|o| (o.tag, o.voice))
    }
}

/// A ring of the last `TOMBSTONES` releases that matched no voice.
#[derive(Clone, Debug)]
pub struct Tombstones {
    ring: [Option<VoiceTag>; TOMBSTONES],
    next: usize,
}

impl Default for Tombstones {
    fn default() -> Self {
        Self::new()
    }
}

impl Tombstones {
    /// An empty ring.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ring: [None; TOMBSTONES],
            next: 0,
        }
    }

    /// Records an unmatched release, overwriting the oldest when full.
    pub fn push(&mut self, tag: VoiceTag) {
        self.ring[self.next] = Some(tag);
        self.next = (self.next + 1) % TOMBSTONES;
    }

    /// True when `tag` was released before it started.
    #[must_use]
    pub fn contains(&self, tag: VoiceTag) -> bool {
        self.ring.iter().any(|t| *t == Some(tag))
    }

    /// Consumes the tombstone of `tag`; returns whether there was one (the
    /// NoteOn carrying it is then dropped).
    pub fn take(&mut self, tag: VoiceTag) -> bool {
        match self.ring.iter_mut().find(|t| **t == Some(tag)) {
            Some(t) => {
                *t = None;
                true
            }
            None => false,
        }
    }
}
