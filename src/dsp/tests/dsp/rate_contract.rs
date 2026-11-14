//! Host rate/quantum validation and install-time voice-state budgeting.

use super::{caps, chain, config, ctl, event, rms, BrowserRig, NativeRig};
use crate::dsp::arena::{encode_inst, StoreKind};
use crate::dsp::engine::{Engine, EngineConfig};
use crate::dsp::graph::UGenSpec;
use crate::dsp::ring::{encode_graph_record, ConfigError};
use crate::dsp::ugen::{BuildError, Template};

#[test]
fn invalid_host_configuration_fails_before_allocation() {
    let base = config(&caps(), StoreKind::NativeArc);
    for bad_rate in [0.0, f32::NAN, 7_999.0, 192_001.0] {
        let mut cfg = base;
        cfg.sample_rate = bad_rate;
        assert!(matches!(
            Engine::try_with_config(cfg),
            Err(ConfigError::SampleRate)
        ));
    }
    for bad_block in [0, EngineConfig::MAX_BLOCK + 1] {
        let mut cfg = base;
        cfg.max_block = bad_block;
        assert!(matches!(
            Engine::try_with_config(cfg),
            Err(ConfigError::BlockSize)
        ));
    }
    let mut cfg = base;
    cfg.voice_seconds = f32::INFINITY;
    assert!(matches!(
        Engine::try_with_config(cfg),
        Err(ConfigError::StateBudget)
    ));
}

#[test]
fn fixed_ported_voice_state_must_fit_preallocated_budget() {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.sample_rate = 96_000.0;
    cfg.voice_seconds = 0.001;
    let engine = Engine::try_with_config(cfg).unwrap();
    let def = chain(1, vec![UGenSpec::FeedbackDrum]);
    assert_eq!(
        Template::from_inst(&def, &engine.build_env()).unwrap_err(),
        BuildError::MemExceeded
    );
    cfg.voice_seconds = 0.02;
    let engine = Engine::try_with_config(cfg).unwrap();
    let template = Template::from_inst(&def, &engine.build_env()).unwrap();
    assert_eq!(template.mem_total, 1001);
}

#[test]
fn native_and_browser_drum_render_across_rates_and_blocks() {
    let def = chain(1, vec![UGenSpec::FmDrum]);
    for rate in [44_100.0, 48_000.0, 96_000.0] {
        for block in [64, 256] {
            let mut native_cfg = config(&caps(), StoreKind::NativeArc);
            native_cfg.sample_rate = rate;
            native_cfg.max_block = block;
            let mut native = NativeRig::native_with(native_cfg);
            native.install(&def);
            let _ = native.step();
            native.send(event(
                1,
                native.engine.now(),
                &[(ctl::FREQ, 120.0), (ctl::LEGATO, 0.25)],
            ));
            let (left, right) = native.run(8);
            assert!(left.iter().chain(&right).all(|x| x.is_finite()));
            assert!(rms(&left) > 1.0e-5, "native {rate}/{block}");

            let mut browser_cfg = config(&caps(), StoreKind::Arena { bytes: 4 << 20 });
            browser_cfg.sample_rate = rate;
            browser_cfg.max_block = block;
            let mut browser = BrowserRig::browser_with(browser_cfg);
            let mut graph = Vec::new();
            encode_inst(&def, &mut graph).unwrap();
            let mut record = Vec::new();
            encode_graph_record(1, 1, &graph, &mut record);
            browser.push(&record);
            let _ = browser.run(6);
            browser.send(event(
                1,
                browser.engine.now(),
                &[(ctl::FREQ, 120.0), (ctl::LEGATO, 0.25)],
            ));
            let (left, right) = browser.run(8);
            assert!(left.iter().chain(&right).all(|x| x.is_finite()));
            assert!(rms(&left) > 1.0e-5, "browser {rate}/{block}");
        }
    }
}
