//! Six-operator frequency-modulation algorithm topologies.
//!
//! The production table is authored from the published six-operator algorithm
//! chart in `design-fm1-voices.md`. Its rows are cross-checked against the
//! Apache-2.0 `music-synthesizer-for-android` bus-flag table at revision
//! `f67d41d313b7dc85f6fb99e79e515cc9d208cfff` by
//! `src/dsp/tests/dsp/fm_algorithms.rs`.
//!
//! Operators are numbered 1 through 6, following the algorithm chart. A bit
//! in `modulators[target - 1]` identifies a direct modulator; carrier bits use
//! the same operator-to-bit mapping. Feedback is one directed edge.

/// Direct modulation inputs, carriers and feedback for one six-operator graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Algorithm {
    /// `modulators[i]` has bit `j` set when operator `j + 1` modulates operator `i + 1`.
    pub modulators: [u8; 6],
    /// Bit `i` is set when operator `i + 1` is a carrier.
    pub carriers: u8,
    /// Feedback edge as `(source, destination)` operator numbers in `1..=6`.
    /// A self-loop has the same source and destination.
    pub feedback: (u8, u8),
}

/// The 32 six-operator topologies; array index zero is algorithm 1.
pub const ALGORITHMS: [Algorithm; 32] = [
    // 1: 2->1, 6->5, 5->4, 4->3; carriers 1, 3; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0, 0b001000, 0b010000, 0b100000, 0],
        carriers: 0b000101,
        feedback: (6, 6),
    },
    // 2: 2->1, 6->5, 5->4, 4->3; carriers 1, 3; feedback 2->2.
    Algorithm {
        modulators: [0b000010, 0, 0b001000, 0b010000, 0b100000, 0],
        carriers: 0b000101,
        feedback: (2, 2),
    },
    // 3: 3->2, 2->1, 6->5, 5->4; carriers 1, 4; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0b000100, 0, 0b010000, 0b100000, 0],
        carriers: 0b001001,
        feedback: (6, 6),
    },
    // 4: 3->2, 2->1, 6->5, 5->4; carriers 1, 4; feedback 4->6.
    Algorithm {
        modulators: [0b000010, 0b000100, 0, 0b010000, 0b100000, 0],
        carriers: 0b001001,
        feedback: (4, 6),
    },
    // 5: 2->1, 4->3, 6->5; carriers 1, 3, 5; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0, 0b001000, 0, 0b100000, 0],
        carriers: 0b010101,
        feedback: (6, 6),
    },
    // 6: 2->1, 4->3, 6->5; carriers 1, 3, 5; feedback 5->6.
    Algorithm {
        modulators: [0b000010, 0, 0b001000, 0, 0b100000, 0],
        carriers: 0b010101,
        feedback: (5, 6),
    },
    // 7: 2->1, 4->3, 5->3, 6->5; carriers 1, 3; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0, 0b011000, 0, 0b100000, 0],
        carriers: 0b000101,
        feedback: (6, 6),
    },
    // 8: 2->1, 4->3, 5->3, 6->5; carriers 1, 3; feedback 4->4.
    Algorithm {
        modulators: [0b000010, 0, 0b011000, 0, 0b100000, 0],
        carriers: 0b000101,
        feedback: (4, 4),
    },
    // 9: 2->1, 4->3, 5->3, 6->5; carriers 1, 3; feedback 2->2.
    Algorithm {
        modulators: [0b000010, 0, 0b011000, 0, 0b100000, 0],
        carriers: 0b000101,
        feedback: (2, 2),
    },
    // 10: 3->2, 2->1, 5->4, 6->4; carriers 1, 4; feedback 3->3.
    Algorithm {
        modulators: [0b000010, 0b000100, 0, 0b110000, 0, 0],
        carriers: 0b001001,
        feedback: (3, 3),
    },
    // 11: 3->2, 2->1, 5->4, 6->4; carriers 1, 4; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0b000100, 0, 0b110000, 0, 0],
        carriers: 0b001001,
        feedback: (6, 6),
    },
    // 12: 2->1, 4->3, 5->3, 6->3; carriers 1, 3; feedback 2->2.
    Algorithm {
        modulators: [0b000010, 0, 0b111000, 0, 0, 0],
        carriers: 0b000101,
        feedback: (2, 2),
    },
    // 13: 2->1, 4->3, 5->3, 6->3; carriers 1, 3; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0, 0b111000, 0, 0, 0],
        carriers: 0b000101,
        feedback: (6, 6),
    },
    // 14: 2->1, 4->3, 5->4, 6->4; carriers 1, 3; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0, 0b001000, 0b110000, 0, 0],
        carriers: 0b000101,
        feedback: (6, 6),
    },
    // 15: 2->1, 4->3, 5->4, 6->4; carriers 1, 3; feedback 2->2.
    Algorithm {
        modulators: [0b000010, 0, 0b001000, 0b110000, 0, 0],
        carriers: 0b000101,
        feedback: (2, 2),
    },
    // 16: 2->1, 3->1, 5->1, 4->3, 6->5; carrier 1; feedback 6->6.
    Algorithm {
        modulators: [0b010110, 0, 0b001000, 0, 0b100000, 0],
        carriers: 0b000001,
        feedback: (6, 6),
    },
    // 17: 2->1, 3->1, 5->1, 4->3, 6->5; carrier 1; feedback 2->2.
    Algorithm {
        modulators: [0b010110, 0, 0b001000, 0, 0b100000, 0],
        carriers: 0b000001,
        feedback: (2, 2),
    },
    // 18: 2->1, 3->1, 4->1, 5->4, 6->5; carrier 1; feedback 3->3.
    Algorithm {
        modulators: [0b001110, 0, 0, 0b010000, 0b100000, 0],
        carriers: 0b000001,
        feedback: (3, 3),
    },
    // 19: 3->2, 2->1, 6->4, 6->5; carriers 1, 4, 5; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0b000100, 0, 0b100000, 0b100000, 0],
        carriers: 0b011001,
        feedback: (6, 6),
    },
    // 20: 3->1, 3->2, 5->4, 6->4; carriers 1, 2, 4; feedback 3->3.
    Algorithm {
        modulators: [0b000100, 0b000100, 0, 0b110000, 0, 0],
        carriers: 0b001011,
        feedback: (3, 3),
    },
    // 21: 3->1, 3->2, 6->4, 6->5; carriers 1, 2, 4, 5; feedback 3->3.
    Algorithm {
        modulators: [0b000100, 0b000100, 0, 0b100000, 0b100000, 0],
        carriers: 0b011011,
        feedback: (3, 3),
    },
    // 22: 2->1, 6->3, 6->4, 6->5; carriers 1, 3, 4, 5; feedback 6->6.
    Algorithm {
        modulators: [0b000010, 0, 0b100000, 0b100000, 0b100000, 0],
        carriers: 0b011101,
        feedback: (6, 6),
    },
    // 23: 3->2, 6->4, 6->5; carriers 1, 2, 4, 5; feedback 6->6.
    Algorithm {
        modulators: [0, 0b000100, 0, 0b100000, 0b100000, 0],
        carriers: 0b011011,
        feedback: (6, 6),
    },
    // 24: 6->3, 6->4, 6->5; carriers 1, 2, 3, 4, 5; feedback 6->6.
    Algorithm {
        modulators: [0, 0, 0b100000, 0b100000, 0b100000, 0],
        carriers: 0b011111,
        feedback: (6, 6),
    },
    // 25: 6->4, 6->5; carriers 1, 2, 3, 4, 5; feedback 6->6.
    Algorithm {
        modulators: [0, 0, 0, 0b100000, 0b100000, 0],
        carriers: 0b011111,
        feedback: (6, 6),
    },
    // 26: 3->2, 5->4, 6->4; carriers 1, 2, 4; feedback 6->6.
    Algorithm {
        modulators: [0, 0b000100, 0, 0b110000, 0, 0],
        carriers: 0b001011,
        feedback: (6, 6),
    },
    // 27: 3->2, 5->4, 6->4; carriers 1, 2, 4; feedback 3->3.
    Algorithm {
        modulators: [0, 0b000100, 0, 0b110000, 0, 0],
        carriers: 0b001011,
        feedback: (3, 3),
    },
    // 28: 2->1, 5->4, 4->3; carriers 1, 3, 6; feedback 5->5.
    Algorithm {
        modulators: [0b000010, 0, 0b001000, 0b010000, 0, 0],
        carriers: 0b100101,
        feedback: (5, 5),
    },
    // 29: 4->3, 6->5; carriers 1, 2, 3, 5; feedback 6->6.
    Algorithm {
        modulators: [0, 0, 0b001000, 0, 0b100000, 0],
        carriers: 0b010111,
        feedback: (6, 6),
    },
    // 30: 5->4, 4->3; carriers 1, 2, 3, 6; feedback 5->5.
    Algorithm {
        modulators: [0, 0, 0b001000, 0b010000, 0, 0],
        carriers: 0b100111,
        feedback: (5, 5),
    },
    // 31: 6->5; carriers 1, 2, 3, 4, 5; feedback 6->6.
    Algorithm {
        modulators: [0, 0, 0, 0, 0b100000, 0],
        carriers: 0b011111,
        feedback: (6, 6),
    },
    // 32: no modulation edges; all operators are carriers; feedback 6->6.
    Algorithm {
        modulators: [0; 6],
        carriers: 0b111111,
        feedback: (6, 6),
    },
];

/// Operators are processed from 6 down to 1 so modulators precede targets.
pub const RENDER_ORDER: [u8; 6] = [6, 5, 4, 3, 2, 1];

/// Select an algorithm by its one-based number, clamping to `1..=32`.
pub fn algorithm(number: u8) -> &'static Algorithm {
    &ALGORITHMS[(number.clamp(1, 32) - 1) as usize]
}

/// Return the number of carrier operators in an algorithm.
pub const fn carrier_count(a: &Algorithm) -> u32 {
    a.carriers.count_ones()
}
