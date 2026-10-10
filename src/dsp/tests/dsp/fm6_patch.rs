//! FM6 patch parameter contract test owned by FM1V-00.

use crate::dsp::ugen::fm::patch::{global, Fm6Patch, VOICE_PARAMS};

#[test]
fn fm6_patch_public_contract() {
    let mut values = [0.0; VOICE_PARAMS];
    values[global::ALGORITHM] = 31.0;
    values[global::FEEDBACK] = 7.0;
    let patch = Fm6Patch::from_flat(&values).expect("valid patch parameters");
    assert_eq!(patch.algorithm(), 31);
    assert_eq!(patch.validate(), Ok(()));
    assert_eq!(
        Fm6Patch::from_flat(&[0.0; VOICE_PARAMS - 1]).unwrap_err(),
        format!(
            "patch: expected {VOICE_PARAMS} values, got {}",
            VOICE_PARAMS - 1
        )
    );
}
