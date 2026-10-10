//! FM1V-10 cross-check fixture adapted from the pinned msfa algorithm bus flags.

use crate::dsp::ugen::fm::algorithms::{algorithm, Algorithm, ALGORITHMS, RENDER_ORDER};

// Adapted from music-synthesizer-for-android (Apache-2.0),
// fm_core.cc, revision f67d41d313b7dc85f6fb99e79e515cc9d208cfff. Copyright Google Inc.
const MSFA_FLAGS: [[u8; 6]; 32] = [
    [0xc1, 0x11, 0x11, 0x14, 0x01, 0x14], // 1
    [0x01, 0x11, 0x11, 0x14, 0xc1, 0x14], // 2
    [0xc1, 0x11, 0x14, 0x01, 0x11, 0x14], // 3
    [0x41, 0x11, 0x94, 0x01, 0x11, 0x14], // 4
    [0xc1, 0x14, 0x01, 0x14, 0x01, 0x14], // 5
    [0x41, 0x94, 0x01, 0x14, 0x01, 0x14], // 6
    [0xc1, 0x11, 0x05, 0x14, 0x01, 0x14], // 7
    [0x01, 0x11, 0xc5, 0x14, 0x01, 0x14], // 8
    [0x01, 0x11, 0x05, 0x14, 0xc1, 0x14], // 9
    [0x01, 0x05, 0x14, 0xc1, 0x11, 0x14], // 10
    [0xc1, 0x05, 0x14, 0x01, 0x11, 0x14], // 11
    [0x01, 0x05, 0x05, 0x14, 0xc1, 0x14], // 12
    [0xc1, 0x05, 0x05, 0x14, 0x01, 0x14], // 13
    [0xc1, 0x05, 0x11, 0x14, 0x01, 0x14], // 14
    [0x01, 0x05, 0x11, 0x14, 0xc1, 0x14], // 15
    [0xc1, 0x11, 0x02, 0x25, 0x05, 0x14], // 16
    [0x01, 0x11, 0x02, 0x25, 0xc5, 0x14], // 17
    [0x01, 0x11, 0x11, 0xc5, 0x05, 0x14], // 18
    [0xc1, 0x14, 0x14, 0x01, 0x11, 0x14], // 19
    [0x01, 0x05, 0x14, 0xc1, 0x14, 0x14], // 20
    [0x01, 0x14, 0x14, 0xc1, 0x14, 0x14], // 21
    [0xc1, 0x14, 0x14, 0x14, 0x01, 0x14], // 22
    [0xc1, 0x14, 0x14, 0x01, 0x14, 0x04], // 23
    [0xc1, 0x14, 0x14, 0x14, 0x04, 0x04], // 24
    [0xc1, 0x14, 0x14, 0x04, 0x04, 0x04], // 25
    [0xc1, 0x05, 0x14, 0x01, 0x14, 0x04], // 26
    [0x01, 0x05, 0x14, 0xc1, 0x14, 0x04], // 27
    [0x04, 0xc1, 0x11, 0x14, 0x01, 0x14], // 28
    [0xc1, 0x14, 0x01, 0x14, 0x04, 0x04], // 29
    [0x04, 0xc1, 0x11, 0x14, 0x04, 0x04], // 30
    [0xc1, 0x14, 0x04, 0x04, 0x04, 0x04], // 31
    [0xc4, 0x04, 0x04, 0x04, 0x04, 0x04], // 32
];

const OUT_BUS_ONE: u8 = 1 << 0;
const OUT_BUS_TWO: u8 = 1 << 1;
const OUT_BUS_ADD: u8 = 1 << 2;
const IN_BUS_ONE: u8 = 1 << 4;
const IN_BUS_TWO: u8 = 1 << 5;
const FB_IN: u8 = 1 << 6;
const FB_OUT: u8 = 1 << 7;

fn derive(flags: &[u8; 6]) -> Algorithm {
    // msfa's core loops over array indices 0..5; dx7note initializes the
    // corresponding patch operators in that same order. The published chart
    // numbers those operators in reverse: index 0 is operator 6, index 5 is 1.
    let mut buses = [0_u8; 3];
    let mut modulators = [0_u8; 6];
    let mut carriers = 0_u8;
    let mut feedback_source = 0_u8;
    let mut feedback_destination = 0_u8;

    for (index, &flag) in flags.iter().enumerate() {
        let operator = 6 - index as u8;
        let input_bus: u8 = if flag & IN_BUS_ONE != 0 {
            1
        } else if flag & IN_BUS_TWO != 0 {
            2
        } else {
            0
        };
        let output_bus: u8 = if flag & OUT_BUS_ONE != 0 {
            1
        } else if flag & OUT_BUS_TWO != 0 {
            2
        } else {
            0
        };
        if input_bus != 0 {
            modulators[usize::from(operator - 1)] = buses[usize::from(input_bus)];
        }
        if output_bus == 0 {
            carriers |= 1 << (operator - 1);
        }
        if flag & FB_OUT != 0 {
            feedback_source = operator;
        }
        if flag & FB_IN != 0 {
            feedback_destination = operator;
        }
        if output_bus != 0 {
            let operator_bit = 1 << (operator - 1);
            if flag & OUT_BUS_ADD != 0 {
                buses[usize::from(output_bus)] |= operator_bit;
            } else {
                buses[usize::from(output_bus)] = operator_bit;
            }
        }
    }

    Algorithm {
        modulators,
        carriers,
        feedback: (feedback_source, feedback_destination),
    }
}

#[test]
fn fm_algo_table_matches_msfa_derivation() {
    for (index, flags) in MSFA_FLAGS.iter().enumerate() {
        assert_eq!(
            derive(flags),
            ALGORITHMS[index],
            "algorithm {} differs from msfa",
            index + 1,
        );
    }
}

#[test]
fn fm_algo_table_matches_design_examples() {
    let first = &ALGORITHMS[0];
    assert_eq!(first.carriers, (1 << 0) | (1 << 2));
    assert_eq!(first.modulators[0], 1 << 1);
    assert_eq!(first.feedback, (6, 6));
    assert_eq!(ALGORITHMS[1].feedback, (2, 2));
    assert_eq!(ALGORITHMS[4].carriers, (1 << 0) | (1 << 2) | (1 << 4));
    assert_eq!(ALGORITHMS[31].carriers, 0b11_1111);
    assert_eq!(ALGORITHMS[31].modulators, [0; 6]);
    assert_eq!(ALGORITHMS[31].feedback, (6, 6));
}

#[test]
fn fm_algo_every_modulator_precedes_target_in_render_order() {
    assert_eq!(RENDER_ORDER, [6, 5, 4, 3, 2, 1]);
    for algorithm in &ALGORITHMS {
        for (target_index, &modulators) in algorithm.modulators.iter().enumerate() {
            let target = target_index as u8 + 1;
            for source_index in 0..6 {
                if modulators & (1 << source_index) != 0 {
                    let source = source_index as u8 + 1;
                    assert!(source > target, "operator {source} must precede {target}");
                }
            }
        }
    }
}

#[test]
fn fm_algo_each_algorithm_has_a_carrier_and_one_feedback_edge() {
    for (index, algorithm) in ALGORITHMS.iter().enumerate() {
        assert_ne!(
            algorithm.carriers,
            0,
            "algorithm {} has no carrier",
            index + 1
        );
        let (source, destination) = algorithm.feedback;
        assert!(
            (1..=6).contains(&source),
            "algorithm {} has no feedback source",
            index + 1
        );
        assert!(
            (1..=6).contains(&destination),
            "algorithm {} has no feedback destination",
            index + 1
        );
    }
}

#[test]
fn fm_algo_number_clamps() {
    assert_eq!(algorithm(0), &ALGORITHMS[0]);
    assert_eq!(algorithm(40), &ALGORITHMS[31]);
}
