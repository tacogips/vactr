//! Public-domain tuning and scale preset tables from design section 4.8.

use crate::value::ratio::Ratio64;
use crate::value::value::Value;

use super::{edo_spec, ratios_spec};

/// A scale's native EDO size, period and ascending degree positions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalePreset {
    pub name: &'static str,
    pub size: i64,
    pub period: (i64, i64),
    pub steps: &'static [i64],
}

type RatioPreset = (&'static str, i64, i64);
type TuningPreset = (&'static str, &'static [RatioPreset]);

const TUNING_PRESETS: &[TuningPreset] = &[
    (
        "ji-5",
        &[
            ("16/15", 16, 15),
            ("9/8", 9, 8),
            ("6/5", 6, 5),
            ("5/4", 5, 4),
            ("4/3", 4, 3),
            ("45/32", 45, 32),
            ("3/2", 3, 2),
            ("8/5", 8, 5),
            ("5/3", 5, 3),
            ("9/5", 9, 5),
            ("15/8", 15, 8),
            ("2", 2, 1),
        ],
    ),
    (
        "ji-7",
        &[
            ("15/14", 15, 14),
            ("8/7", 8, 7),
            ("6/5", 6, 5),
            ("5/4", 5, 4),
            ("4/3", 4, 3),
            ("7/5", 7, 5),
            ("3/2", 3, 2),
            ("8/5", 8, 5),
            ("5/3", 5, 3),
            ("7/4", 7, 4),
            ("15/8", 15, 8),
            ("2", 2, 1),
        ],
    ),
    (
        "partch-43",
        &[
            ("81/80", 81, 80),
            ("33/32", 33, 32),
            ("21/20", 21, 20),
            ("16/15", 16, 15),
            ("12/11", 12, 11),
            ("11/10", 11, 10),
            ("10/9", 10, 9),
            ("9/8", 9, 8),
            ("8/7", 8, 7),
            ("7/6", 7, 6),
            ("32/27", 32, 27),
            ("6/5", 6, 5),
            ("11/9", 11, 9),
            ("5/4", 5, 4),
            ("14/11", 14, 11),
            ("9/7", 9, 7),
            ("21/16", 21, 16),
            ("4/3", 4, 3),
            ("27/20", 27, 20),
            ("11/8", 11, 8),
            ("7/5", 7, 5),
            ("10/7", 10, 7),
            ("16/11", 16, 11),
            ("40/27", 40, 27),
            ("3/2", 3, 2),
            ("32/21", 32, 21),
            ("14/9", 14, 9),
            ("11/7", 11, 7),
            ("8/5", 8, 5),
            ("18/11", 18, 11),
            ("5/3", 5, 3),
            ("27/16", 27, 16),
            ("12/7", 12, 7),
            ("7/4", 7, 4),
            ("16/9", 16, 9),
            ("9/5", 9, 5),
            ("20/11", 20, 11),
            ("11/6", 11, 6),
            ("15/8", 15, 8),
            ("40/21", 40, 21),
            ("64/33", 64, 33),
            ("160/81", 160, 81),
            ("2", 2, 1),
        ],
    ),
];

const SCALE_PRESETS: &[ScalePreset] = &[
    ScalePreset {
        name: "edo19-major",
        size: 19,
        period: (2, 1),
        steps: &[0, 3, 6, 8, 11, 14, 17],
    },
    ScalePreset {
        name: "edo19-minor",
        size: 19,
        period: (2, 1),
        steps: &[0, 3, 5, 8, 11, 13, 16],
    },
    ScalePreset {
        name: "edo31-major",
        size: 31,
        period: (2, 1),
        steps: &[0, 5, 10, 13, 18, 23, 28],
    },
    ScalePreset {
        name: "edo53-major",
        size: 53,
        period: (2, 1),
        steps: &[0, 9, 18, 22, 31, 40, 49],
    },
    ScalePreset {
        name: "maqam-rast",
        size: 24,
        period: (2, 1),
        steps: &[0, 4, 7, 10, 14, 18, 21],
    },
    ScalePreset {
        name: "maqam-bayati",
        size: 24,
        period: (2, 1),
        steps: &[0, 3, 6, 10, 14, 16, 20],
    },
    ScalePreset {
        name: "maqam-saba",
        size: 24,
        period: (2, 1),
        steps: &[0, 3, 6, 8, 14, 16, 20],
    },
    ScalePreset {
        name: "maqam-hijaz",
        size: 24,
        period: (2, 1),
        steps: &[0, 2, 8, 10, 14, 16, 20],
    },
    ScalePreset {
        name: "slendro",
        size: 5,
        period: (2, 1),
        steps: &[0, 1, 2, 3, 4],
    },
    ScalePreset {
        name: "pelog",
        size: 9,
        period: (2, 1),
        steps: &[0, 1, 2, 4, 5, 6, 7],
    },
    ScalePreset {
        name: "bp-lambda",
        size: 13,
        period: (3, 1),
        steps: &[0, 2, 3, 4, 6, 7, 9, 10, 12],
    },
];

/// Returns a canonical spec dictionary for a named tuning preset.
#[must_use]
pub fn tuning_preset(name: &str) -> Option<Value> {
    if name == "bohlen-pierce" {
        return edo_spec(&Value::Int(13), Some(&Value::Int(3))).ok();
    }
    let (_, ratios) = TUNING_PRESETS.iter().find(|(preset, _)| *preset == name)?;
    let values = ratios
        .iter()
        .map(|(_, n, d)| {
            Some(if *d == 1 {
                Value::Int(*n as i32)
            } else {
                Value::Ratio(Ratio64::new(*n, *d).ok()?)
            })
        })
        .collect::<Option<Vec<_>>>()?;
    ratios_spec(&Value::list(values)).ok()
}

/// Looks up one of the eleven microtonal scale presets.
#[must_use]
pub fn scale_preset(name: &str) -> Option<&'static ScalePreset> {
    SCALE_PRESETS.iter().find(|preset| preset.name == name)
}
