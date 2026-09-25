//! The pure hash RNG (design 10.3). Every random draw is a hash of its key
//! (seed, node id, cycle[, sequence]); there is no mutable RNG state.

use crate::reader::span::NodeId;
use crate::value::ratio::Ratio64;

/// The splitmix64 finalizer.
#[must_use]
pub const fn mix64(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// An order-sensitive hash accumulator over `u64` words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Hasher(u64);

impl Hasher {
    /// A hasher with a domain tag.
    #[must_use]
    pub const fn new(tag: u64) -> Self {
        Self(mix64(tag))
    }

    /// Adds one word.
    #[must_use]
    pub const fn word(self, w: u64) -> Self {
        Self(mix64(self.0 ^ mix64(w)))
    }

    /// Adds a signed word.
    #[must_use]
    pub const fn int(self, w: i64) -> Self {
        self.word(w as u64)
    }

    /// Adds an exact ratio.
    #[must_use]
    pub const fn ratio(self, r: Ratio64) -> Self {
        self.int(r.num()).int(r.den())
    }

    /// Adds the bytes of a string.
    #[must_use]
    pub fn text(self, s: &str) -> Self {
        let mut h = self.word(s.len() as u64);
        for chunk in s.as_bytes().chunks(8) {
            let mut w = 0u64;
            for (i, b) in chunk.iter().enumerate() {
                w |= u64::from(*b) << (8 * i);
            }
            h = h.word(w);
        }
        h
    }

    /// The hash value.
    #[must_use]
    pub const fn finish(self) -> u64 {
        self.0
    }
}

/// The key of one draw.
#[must_use]
pub fn draw(seed: u64, node: NodeId, cycle: i64, seq: u64) -> u64 {
    Hasher::new(0x7261_6e64)
        .word(seed)
        .word(u64::from(node.get()))
        .int(cycle)
        .word(seq)
        .finish()
}

/// A uniform draw in `[0, 1)` for the key.
#[must_use]
pub fn unit(seed: u64, node: NodeId, cycle: i64, seq: u64) -> f64 {
    to_unit(draw(seed, node, cycle, seq))
}

/// A uniform draw in `[0, 1)` keyed by an exact time position (the
/// occurrence anchor), for per-event decisions.
#[must_use]
pub fn unit_at(seed: u64, node: NodeId, at: Ratio64) -> f64 {
    let seq = Hasher::new(0x6174).ratio(at).finish();
    unit(seed, node, at.floor(), seq)
}

/// Maps a hash to `[0, 1)` using its top 53 bits.
#[must_use]
pub fn to_unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// A draw in `0..n` (0 when `n` is 0).
#[must_use]
pub fn below(seed: u64, node: NodeId, cycle: i64, seq: u64, n: u64) -> u64 {
    if n == 0 {
        0
    } else {
        draw(seed, node, cycle, seq) % n
    }
}
