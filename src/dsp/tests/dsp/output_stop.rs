use crate::dsp::arena::{encode_bus, encode_inst, StoreKind};
use crate::dsp::graph::{Edge, EffectKind, InstId, UGenSpec};
use crate::dsp::ring::encode_graph_record;
use crate::dsp::tests::dsp::{bus_def, caps, chain, config, BrowserRig, NativeRig, BLOCK, SR};
use crate::host::wire::{Ctl, CtlMsg, HostMsg, OutputMode, OutputPhase, Release, SlotControl};
use crate::sched::slots::{CtlId, SlotId};

fn pad_event(time: f64, generation: u32) -> crate::host::wire::AudioEvent {
    let mut event =
        crate::host::wire::AudioEvent::new(time, SlotId::new(1), generation, InstId::new(1));
    for (id, value) in [
        (CtlId::new(0), 220.0),
        (CtlId::new(9), 0.001),
        (CtlId::new(10), 0.01),
        (CtlId::new(11), 0.8),
        (CtlId::new(12), 0.05),
        (CtlId::new(25), 2.0),
    ] {
        event
            .push_ctl(id, Ctl::Const(value))
            .expect("pad control fits");
    }
    event
}

const DELAY_SEND: CtlId = CtlId::new(38);
const DELAY_TIME: CtlId = CtlId::new(39);
const DELAY_FEEDBACK: CtlId = CtlId::new(40);

fn pad() -> crate::dsp::graph::InstDef {
    let mut def = chain(1, vec![UGenSpec::SinOsc, UGenSpec::EnvAdsr, UGenSpec::Mul]);
    def.edges = Box::new([
        Edge {
            from: 0,
            to: 2,
            port: 0,
            output: 0,
        },
        Edge {
            from: 1,
            to: 2,
            port: 1,
            output: 0,
        },
    ]);
    def
}

fn stopped_config(store: StoreKind) -> crate::dsp::ring::EngineConfig {
    let mut cfg = config(&caps(), store);
    cfg.tail_hold_seconds = 0.1;
    cfg.drain_cap_seconds = 2.0;
    cfg
}

fn phases(messages: Vec<HostMsg>) -> Vec<OutputPhase> {
    messages
        .into_iter()
        .filter_map(|message| match message {
            HostMsg::OutputState { phase, .. } => Some(phase),
            _ => None,
        })
        .collect()
}

fn install_pad_native(rig: &mut NativeRig) {
    rig.install(&pad());
    let _ = rig.step();
}

fn install_pad_browser(rig: &mut BrowserRig) {
    let mut bytes = Vec::new();
    encode_inst(&pad(), &mut bytes).expect("pad encodes");
    let mut record = Vec::new();
    encode_graph_record(1, 1, &bytes, &mut record);
    rig.push(&record);
    for _ in 0..8 {
        let _ = rig.step();
        if rig.engine.template(InstId::new(1)).is_some() {
            break;
        }
    }
    assert!(rig.engine.template(InstId::new(1)).is_some());
}

fn start_pad(rig: &mut impl RigOps) {
    rig.send_start();
}

trait RigOps {
    fn send_start(&mut self);
    fn send_gentle_natural(&mut self);
    fn post_cut_then_gentle(&mut self);
    fn step_owned(&mut self) -> Vec<f32>;
    fn take_phases(&mut self) -> Vec<OutputPhase>;
}

impl RigOps for NativeRig {
    fn send_start(&mut self) {
        self.send(crate::dsp::tests::dsp::event(
            1,
            self.engine.now(),
            &[
                (CtlId::new(0), 220.0),
                (CtlId::new(9), 0.001),
                (CtlId::new(10), 0.01),
                (CtlId::new(11), 0.8),
                (CtlId::new(12), 0.05),
                (CtlId::new(25), 2.0),
                (DELAY_SEND, 0.9),
                (DELAY_TIME, 0.05),
                (DELAY_FEEDBACK, 0.35),
            ],
        ));
    }

    fn send_gentle_natural(&mut self) {
        self.post(CtlMsg::SlotControl(SlotControl {
            slot: SlotId::new(1),
            new_gen: 2,
            effective_time: self.engine.now(),
            release: Release::Natural,
        }));
        self.post(CtlMsg::OutputStop {
            mode: OutputMode::Gentle,
        });
    }

    fn post_cut_then_gentle(&mut self) {
        self.post(CtlMsg::OutputStop {
            mode: OutputMode::Cut,
        });
        self.post(CtlMsg::OutputStop {
            mode: OutputMode::Gentle,
        });
    }

    fn step_owned(&mut self) -> Vec<f32> {
        self.step().to_vec()
    }

    fn take_phases(&mut self) -> Vec<OutputPhase> {
        phases(self.acks())
    }
}

impl RigOps for BrowserRig {
    fn send_start(&mut self) {
        self.send(crate::dsp::tests::dsp::event(
            1,
            self.engine.now(),
            &[
                (CtlId::new(0), 220.0),
                (CtlId::new(9), 0.001),
                (CtlId::new(10), 0.01),
                (CtlId::new(11), 0.8),
                (CtlId::new(12), 0.05),
                (CtlId::new(25), 2.0),
                (DELAY_SEND, 0.9),
                (DELAY_TIME, 0.05),
                (DELAY_FEEDBACK, 0.35),
            ],
        ));
    }

    fn send_gentle_natural(&mut self) {
        self.post(CtlMsg::SlotControl(SlotControl {
            slot: SlotId::new(1),
            new_gen: 2,
            effective_time: self.engine.now(),
            release: Release::Natural,
        }));
        self.post(CtlMsg::OutputStop {
            mode: OutputMode::Gentle,
        });
    }

    fn post_cut_then_gentle(&mut self) {
        self.post(CtlMsg::OutputStop {
            mode: OutputMode::Cut,
        });
        self.post(CtlMsg::OutputStop {
            mode: OutputMode::Gentle,
        });
    }

    fn step_owned(&mut self) -> Vec<f32> {
        self.step().to_vec()
    }

    fn take_phases(&mut self) -> Vec<OutputPhase> {
        phases(self.acks())
    }
}

fn render_gentle_tail<R: RigOps>(mut rig: R) -> (Vec<f32>, Vec<OutputPhase>) {
    start_pad(&mut rig);
    let _ = rig.step_owned();
    for _ in 0..7 {
        let _ = rig.step_owned();
        let _ = rig.take_phases();
    }
    rig.send_gentle_natural();
    let mut output = Vec::new();
    let mut reports = Vec::new();
    for _ in 0..1_000 {
        output.extend(rig.step_owned());
        reports.extend(rig.take_phases());
        if reports.contains(&OutputPhase::Idle) {
            break;
        }
    }
    (output, reports)
}

#[test]
fn gentle_stop_keeps_delay_tail_and_native_browser_match() {
    let native = render_gentle_tail({
        let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
        install_pad_native(&mut rig);
        rig
    });
    let browser = render_gentle_tail({
        let mut rig = BrowserRig::browser_with(stopped_config(StoreKind::Arena { bytes: 1 << 20 }));
        install_pad_browser(&mut rig);
        rig
    });
    assert_eq!(native.1, [OutputPhase::Draining, OutputPhase::Idle]);
    assert_eq!(browser.1, [OutputPhase::Draining, OutputPhase::Idle]);
    assert_eq!(native.0.len(), browser.0.len());
    let max_delta = native
        .0
        .iter()
        .zip(&browser.0)
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f32, f32::max);
    assert!(max_delta <= 1.0e-6, "native/browser max delta={max_delta}");
    assert_eq!(native.1, browser.1, "native and browser reports match");
    let audible = (0.1 * SR) as usize * 2;
    assert!(native.0[..audible].iter().any(|sample| *sample != 0.0));
    assert!(native.0[audible..].iter().any(|sample| *sample != 0.0));
    let window = (0.05 * SR) as usize * 2;
    let peaks = native
        .0
        .chunks_exact(window)
        .map(|samples| {
            samples
                .iter()
                .map(|sample| sample.abs())
                .fold(0.0, f32::max)
        })
        .collect::<Vec<_>>();
    assert!(
        peaks.len() > 2,
        "tail spans multiple 50 ms windows: {peaks:?}"
    );
    for pair in peaks.windows(2) {
        assert!(
            pair[1] <= pair[0] + 1.0e-6,
            "tail peaks increased: {peaks:?}"
        );
    }
    assert!(native.0.iter().rev().take(256).all(|sample| *sample == 0.0));
}

trait PostClearRig {
    fn now(&self) -> f64;
    fn send_event(&mut self, event: crate::host::wire::AudioEvent);
    fn post_message(&mut self, message: CtlMsg);
    fn render_block(&mut self) -> Vec<f32>;
    fn take_output_phases(&mut self) -> Vec<OutputPhase>;
}

impl PostClearRig for NativeRig {
    fn now(&self) -> f64 {
        self.engine.now()
    }

    fn send_event(&mut self, event: crate::host::wire::AudioEvent) {
        self.send(event);
    }

    fn post_message(&mut self, message: CtlMsg) {
        self.post(message);
    }

    fn render_block(&mut self) -> Vec<f32> {
        self.step().to_vec()
    }

    fn take_output_phases(&mut self) -> Vec<OutputPhase> {
        phases(self.acks())
    }
}

impl PostClearRig for BrowserRig {
    fn now(&self) -> f64 {
        self.engine.now()
    }

    fn send_event(&mut self, event: crate::host::wire::AudioEvent) {
        self.send(event);
    }

    fn post_message(&mut self, message: CtlMsg) {
        self.post(message);
    }

    fn render_block(&mut self) -> Vec<f32> {
        self.step().to_vec()
    }

    fn take_output_phases(&mut self) -> Vec<OutputPhase> {
        phases(self.acks())
    }
}

fn delay_bus() -> crate::dsp::graph::BusDef {
    let delay = crate::dsp::effects::catalog::spec(
        EffectKind::Delay,
        &[
            ("time", Ctl::Const(0.05)),
            ("feedback", Ctl::Const(0.2)),
            ("mix", Ctl::Const(0.8)),
        ],
    )
    .expect("delay effect spec");
    bus_def(3, vec![delay])
}

fn install_delay_native(rig: &mut NativeRig) {
    rig.install_bus(&delay_bus(), false);
    let _ = rig.step();
}

fn install_delay_browser(rig: &mut BrowserRig) {
    let mut bytes = Vec::new();
    encode_bus(&delay_bus(), false, &mut bytes).expect("delay bus encodes");
    let mut record = Vec::new();
    encode_graph_record(2, 1, &bytes, &mut record);
    rig.push(&record);
    let _ = rig.step();
}

fn routed_pad_event(time: f64, generation: u32) -> crate::host::wire::AudioEvent {
    let mut event =
        crate::host::wire::AudioEvent::new(time, SlotId::new(1), generation, InstId::new(1));
    for (id, value) in [
        (CtlId::new(0), 220.0),
        (CtlId::new(9), 0.001),
        (CtlId::new(10), 0.01),
        (CtlId::new(11), 0.8),
        (CtlId::new(12), 0.05),
        (CtlId::new(25), 2.0),
        (CtlId::new(47), 3.0),
    ] {
        event
            .push_ctl(id, Ctl::Const(value))
            .expect("event control fits");
    }
    event
}

fn natural_release(rig: &mut impl PostClearRig, new_gen: u32) {
    rig.post_message(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen,
        effective_time: rig.now(),
        release: Release::Natural,
    }));
}

fn output_after_restart(rig: &mut impl PostClearRig) -> Vec<f32> {
    rig.send_event(routed_pad_event(rig.now(), 3));
    for _ in 0..8 {
        let _ = rig.render_block();
        let _ = rig.take_output_phases();
    }
    natural_release(rig, 4);
    let mut output = Vec::new();
    for _ in 0..48 {
        output.extend(rig.render_block());
        let _ = rig.take_output_phases();
    }
    output
}

fn stop_to_idle(rig: &mut impl PostClearRig, mode: OutputMode) {
    rig.send_event(routed_pad_event(rig.now(), 1));
    for _ in 0..8 {
        let _ = rig.render_block();
        let _ = rig.take_output_phases();
    }
    rig.post_message(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: rig.now(),
        release: if mode == OutputMode::Gentle {
            Release::Natural
        } else {
            Release::Panic
        },
    }));
    rig.post_message(CtlMsg::OutputStop { mode });
    let mut idle = false;
    for _ in 0..1_000 {
        let _ = rig.render_block();
        idle |= rig.take_output_phases().contains(&OutputPhase::Idle);
        if idle {
            break;
        }
    }
    assert!(idle, "{mode:?} stop reaches Idle after bounded clearing");
}

fn assert_post_clear_restart_matches_fresh(
    mode: OutputMode,
    mut cleared: impl PostClearRig,
    mut fresh: impl PostClearRig,
) {
    stop_to_idle(&mut cleared, mode);
    let resumed = output_after_restart(&mut cleared);
    let baseline = output_after_restart(&mut fresh);
    assert_eq!(resumed.len(), baseline.len());
    let max_delta = resumed
        .iter()
        .zip(&baseline)
        .map(|(left, right)| (left - right).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_delta <= 1.0e-6,
        "{mode:?} restart max delta={max_delta}"
    );
    let tail_start = resumed.len() / 2;
    assert!(
        resumed[tail_start..]
            .iter()
            .any(|sample| sample.abs() > 1.0e-6),
        "{mode:?} restarted delay produces a wet tail"
    );
}

#[test]
fn delay_layout_survives_cut_and_gentle_idle_restart_on_both_hosts() {
    for mode in [OutputMode::Cut, OutputMode::Gentle] {
        assert_post_clear_restart_matches_fresh(
            mode,
            {
                let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
                install_pad_native(&mut rig);
                install_delay_native(&mut rig);
                rig
            },
            {
                let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
                install_pad_native(&mut rig);
                install_delay_native(&mut rig);
                rig
            },
        );
        assert_post_clear_restart_matches_fresh(
            mode,
            {
                let mut rig =
                    BrowserRig::browser_with(stopped_config(StoreKind::Arena { bytes: 1 << 20 }));
                install_pad_browser(&mut rig);
                install_delay_browser(&mut rig);
                rig
            },
            {
                let mut rig =
                    BrowserRig::browser_with(stopped_config(StoreKind::Arena { bytes: 1 << 20 }));
                install_pad_browser(&mut rig);
                install_delay_browser(&mut rig);
                rig
            },
        );
    }
}

#[test]
#[allow(clippy::cast_precision_loss)]
fn cut_gate_is_bounded_by_linear_five_millisecond_fade() {
    let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    let input = vec![0.25; BLOCK * 2];
    let before = rig.step_with_input(&input).to_vec();
    let prior_peak = before
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0_f32, f32::max);
    assert!((prior_peak - 0.25).abs() <= 1.0e-6);
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Cut,
    });
    let fade_frames = (rig.engine.config().cut_fade_seconds * SR).round() as usize;
    let mut output = Vec::new();
    for _ in 0..3 {
        output.extend_from_slice(rig.step_with_input(&input));
    }
    for (frame, stereo) in output.chunks_exact(2).enumerate() {
        let gain = (1.0 - frame as f32 / fade_frames as f32).max(0.0);
        let bound = prior_peak * gain + 1.0e-6;
        assert!(stereo.iter().all(|sample| sample.abs() <= bound));
        if frame >= fade_frames {
            assert_eq!(stereo, [0.0, 0.0]);
        }
    }
}

#[test]
fn cut_clears_orbit_and_legacy_bus_memory_before_idle() {
    use crate::dsp::effects::catalog::spec;
    use crate::dsp::graph::EffectKind;

    let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    install_pad_native(&mut rig);
    rig.install_bus(
        &bus_def(
            3,
            vec![spec(EffectKind::Delay, &[]).expect("delay effect spec")],
        ),
        false,
    );
    let _ = rig.step();
    let event = crate::dsp::tests::dsp::event(
        1,
        rig.engine.now(),
        &[
            (CtlId::new(0), 220.0),
            (CtlId::new(9), 0.001),
            (CtlId::new(10), 0.01),
            (CtlId::new(11), 0.8),
            (CtlId::new(12), 0.05),
            (CtlId::new(25), 2.0),
            (DELAY_SEND, 0.9),
            (DELAY_TIME, 0.05),
            (DELAY_FEEDBACK, 0.35),
            (CtlId::new(47), 3.0),
        ],
    );
    rig.send(event);
    for _ in 0..8 {
        let _ = rig.step();
    }
    assert!(
        !rig.engine.output_memory_is_clear(),
        "delay memory was exercised"
    );
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Cut,
    });
    let mut idle = false;
    for _ in 0..200 {
        let _ = rig.step();
        if phases(rig.acks()).contains(&OutputPhase::Idle) {
            idle = true;
            break;
        }
    }
    assert!(idle, "bounded clear reaches Idle");
    assert!(
        rig.engine.output_memory_is_clear(),
        "all output memory is zero: {:?}",
        rig.engine.output_memory_status()
    );
}

fn render_cut<R: RigOps>(mut rig: R) -> (Vec<f32>, Vec<OutputPhase>) {
    start_pad(&mut rig);
    for _ in 0..8 {
        let _ = rig.step_owned();
        let _ = rig.take_phases();
    }
    rig.post_cut_then_gentle();
    let mut output = Vec::new();
    let mut reports = Vec::new();
    for _ in 0..80 {
        output.extend(rig.step_owned());
        reports.extend(rig.take_phases());
        if reports.contains(&OutputPhase::Idle) {
            break;
        }
    }
    (output, reports)
}

#[test]
fn cut_is_not_downgraded_by_gentle_and_matches_browser_host() {
    let native = render_cut({
        let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
        install_pad_native(&mut rig);
        rig
    });
    let browser = render_cut({
        let mut rig = BrowserRig::browser_with(stopped_config(StoreKind::Arena { bytes: 1 << 20 }));
        install_pad_browser(&mut rig);
        rig
    });
    assert_eq!(native.0, browser.0);
    assert_eq!(
        native.1,
        [
            OutputPhase::Cutting,
            OutputPhase::Cutting,
            OutputPhase::Idle
        ]
    );
    assert_eq!(native.1, browser.1);
    let deduplicated = native
        .1
        .iter()
        .copied()
        .fold(Vec::new(), |mut phases, phase| {
            if phases.last() != Some(&phase) {
                phases.push(phase);
            }
            phases
        });
    assert_eq!(deduplicated, [OutputPhase::Cutting, OutputPhase::Idle]);
    assert!(native.0.iter().any(|sample| sample.abs() > 1.0e-5));
    assert!(native.0.iter().rev().take(256).all(|sample| *sample == 0.0));
}

#[test]
fn stop_ack_and_transition_reports_stay_frame_ordered_under_ack_backpressure() {
    let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    install_pad_native(&mut rig);
    install_delay_native(&mut rig);
    rig.send(routed_pad_event(rig.engine.now(), 1));
    for _ in 0..8 {
        let _ = rig.step();
    }
    let _ = rig.acks();
    assert!(!rig.engine.output_memory_is_clear());

    let stops_per_block = crate::dsp::ring::CHANNEL_CAPACITY;
    let ack_capacity = 4096;
    for _ in 0..ack_capacity / stops_per_block {
        for _ in 0..stops_per_block {
            rig.post(CtlMsg::SlotControl(SlotControl {
                slot: SlotId::new(1),
                new_gen: 2,
                effective_time: rig.engine.now(),
                release: Release::None,
            }));
        }
        let _ = rig.step();
    }
    assert_eq!(rig.acks_rx.len(), ack_capacity);

    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Cut,
    });
    let mut clear_complete = false;
    for _ in 0..200 {
        let _ = rig.step();
        assert_eq!(rig.acks_rx.len(), ack_capacity);
        if rig.engine.output_memory_is_clear() {
            clear_complete = true;
            break;
        }
    }
    assert!(
        clear_complete,
        "cut clear completes while reports are blocked"
    );
    for _ in 0..2 {
        let _ = rig.step();
        assert_eq!(rig.acks_rx.len(), ack_capacity);
    }

    let _ = rig.acks();
    let _ = rig.step();
    let reports = rig
        .acks()
        .into_iter()
        .filter_map(|message| match message {
            HostMsg::OutputState { phase, frame } => Some((phase, frame)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(reports.windows(2).all(|pair| pair[0].1 <= pair[1].1));
    assert!(reports
        .iter()
        .any(|(phase, _)| *phase == OutputPhase::Cutting));
    assert_eq!(
        reports.last().map(|(phase, _)| *phase),
        Some(OutputPhase::Idle)
    );
}

#[test]
fn stop_ack_fifo_backpressures_controls_and_retries_when_ack_ring_is_full() {
    let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    let stop = || CtlMsg::OutputStop {
        mode: OutputMode::Cut,
    };
    let stops_per_block = crate::dsp::ring::CHANNEL_CAPACITY;
    let ack_capacity = 4096;

    for _ in 0..ack_capacity / stops_per_block {
        for _ in 0..stops_per_block {
            rig.post(stop());
        }
        let _ = rig.step();
    }
    assert_eq!(rig.acks_rx.len(), ack_capacity);

    for _ in 0..stops_per_block {
        rig.post(stop());
    }
    let _ = rig.step();
    assert_eq!(rig.acks_rx.len(), ack_capacity);

    rig.post(stop());
    let _ = rig.step();
    assert_eq!(rig.acks_rx.len(), ack_capacity);

    let full_reports = phases(rig.acks());
    assert_eq!(full_reports.len(), ack_capacity);
    assert!(full_reports
        .iter()
        .all(|phase| *phase == OutputPhase::Cutting));
    let _ = rig.step();
    assert_eq!(phases(rig.acks()).len(), stops_per_block);

    let _ = rig.step();
    assert_eq!(phases(rig.acks()), [OutputPhase::Cutting]);
}

#[test]
fn gentle_stop_holds_silence_threshold_before_idle() {
    let mut cfg = stopped_config(StoreKind::NativeArc);
    cfg.tail_hold_seconds = 0.1;
    let mut rig = NativeRig::native_with(cfg);
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Gentle,
    });
    let mut idle_block = None;
    for block_index in 0..80 {
        let block = rig.step().to_vec();
        if phases(rig.acks()).contains(&OutputPhase::Idle) {
            assert!(block.iter().all(|sample| *sample == 0.0));
            idle_block = Some(block_index + 1);
            break;
        }
    }
    let minimum_blocks = (0.1 * SR / BLOCK as f32).ceil() as usize;
    assert!(idle_block.expect("idle after held silence") >= minimum_blocks);
}

#[test]
fn default_stop_timer_config_remains_valid_at_maximum_host_rate() {
    let mut cfg = config(&caps(), StoreKind::NativeArc);
    cfg.sample_rate = crate::dsp::ring::EngineConfig::MAX_SAMPLE_RATE;
    assert_eq!(cfg.validate(), Ok(()));
}

#[test]
fn drain_cap_enters_cut_and_cut_gate_reaches_exact_silence() {
    let mut cfg = stopped_config(StoreKind::NativeArc);
    cfg.drain_cap_seconds = 0.02;
    let mut rig = NativeRig::native_with(cfg);
    install_pad_native(&mut rig);
    rig.send(crate::dsp::tests::dsp::event(
        1,
        rig.engine.now(),
        &[(CtlId::new(25), 2.0)],
    ));
    let _ = rig.step();
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Gentle,
    });
    let mut reports = Vec::new();
    for _ in 0..80 {
        let _ = rig.step();
        reports.extend(phases(rig.acks()));
        if reports.contains(&OutputPhase::Cutting) {
            break;
        }
    }
    assert_eq!(reports, [OutputPhase::Draining, OutputPhase::Cutting]);
    let mut cut_reported = false;
    for _ in 0..80 {
        let block = rig.step().to_vec();
        let current = phases(rig.acks());
        cut_reported |= current.contains(&OutputPhase::Idle);
        if cut_reported {
            assert!(block.iter().all(|sample| *sample == 0.0));
            break;
        }
    }
    assert!(cut_reported, "cut gate and clear reach idle");
}

#[test]
fn gentle_stop_while_idle_is_acknowledged_without_phase_change() {
    let mut cfg = stopped_config(StoreKind::NativeArc);
    cfg.tail_hold_seconds = BLOCK as f32 / SR;
    let mut rig = NativeRig::native_with(cfg);
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Gentle,
    });
    let mut output = Vec::new();
    for _ in 0..160 {
        let _ = rig.step();
        output.extend(phases(rig.acks()));
        if output.contains(&OutputPhase::Idle) {
            break;
        }
    }
    assert_eq!(output, [OutputPhase::Draining, OutputPhase::Idle]);
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Gentle,
    });
    let _ = rig.step();
    assert_eq!(phases(rig.acks()), [OutputPhase::Idle]);
}

#[test]
fn restart_during_draining_returns_to_running() {
    let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    let mut baseline = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    install_pad_native(&mut rig);
    install_pad_native(&mut baseline);
    rig.send(pad_event(rig.engine.now(), 1));
    baseline.send(pad_event(baseline.engine.now(), 1));
    let _ = rig.step();
    let _ = baseline.step();
    rig.post(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: rig.engine.now(),
        release: Release::Natural,
    }));
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Gentle,
    });
    baseline.post(CtlMsg::SlotControl(SlotControl {
        slot: SlotId::new(1),
        new_gen: 2,
        effective_time: baseline.engine.now(),
        release: Release::Natural,
    }));
    baseline.post(CtlMsg::OutputStop {
        mode: OutputMode::Gentle,
    });
    assert_eq!(rig.step(), baseline.step());
    let _ = rig.acks();
    let _ = baseline.acks();
    for _ in 0..3 {
        assert_eq!(rig.step(), baseline.step(), "pre-admission output prefix");
        let _ = rig.acks();
        let _ = baseline.acks();
    }
    rig.send(pad_event(rig.engine.now(), 2));
    let _ = rig.step();
    assert_eq!(phases(rig.acks()), [OutputPhase::Running]);
}

#[test]
fn restart_during_cut_clear_waits_for_clear_then_fades_in() {
    let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    install_pad_native(&mut rig);
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Cut,
    });
    let mut last_cut_block = Vec::new();
    for _ in 0..2 {
        last_cut_block = rig.step().to_vec();
    }
    assert!(last_cut_block.iter().all(|sample| *sample == 0.0));
    let _ = rig.acks();
    rig.send(pad_event(rig.engine.now(), 1));
    let mut prior_blocks = 0;
    let mut resumed = false;
    for _ in 0..8 {
        let block = rig.step().to_vec();
        let reports = phases(rig.acks());
        if reports.contains(&OutputPhase::Running) {
            resumed = true;
            assert!(block.iter().all(|sample| sample.is_finite()));
            break;
        }
        prior_blocks += 1;
        assert!(block.iter().all(|sample| *sample == 0.0));
    }
    assert!(resumed, "clear completion admits the pending restart");
    assert!(
        prior_blocks > 0,
        "restart waits for at least one clear step"
    );
    let mut audible = false;
    for _ in 0..4 {
        audible |= rig.step().iter().any(|sample| sample.abs() > 0.0);
    }
    assert!(audible, "the resumed output fades in after clearing");
}

#[test]
fn restart_after_idle_matches_fresh_engine_without_old_tail() {
    let mut resumed = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    install_pad_native(&mut resumed);
    resumed.post(CtlMsg::OutputStop {
        mode: OutputMode::Cut,
    });
    let mut idle = false;
    for _ in 0..16 {
        let _ = resumed.step();
        if phases(resumed.acks()).contains(&OutputPhase::Idle) {
            idle = true;
            break;
        }
    }
    assert!(idle, "cut clear completes");

    let mut fresh = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    install_pad_native(&mut fresh);
    resumed.send(pad_event(resumed.engine.now(), 1));
    fresh.send(pad_event(fresh.engine.now(), 1));
    let mut resumed_samples: Vec<f32> = Vec::new();
    let mut fresh_samples: Vec<f32> = Vec::new();
    for _ in 0..20 {
        resumed_samples.extend_from_slice(resumed.step());
        fresh_samples.extend_from_slice(fresh.step());
    }
    assert_eq!(resumed_samples, fresh_samples);
}

#[test]
fn retiring_bus_is_held_through_drain_then_collected() {
    use crate::dsp::bus::SlotState;

    let mut rig = NativeRig::native_with(stopped_config(StoreKind::NativeArc));
    rig.install_bus(&bus_def(1, Vec::new()), false);
    let _ = rig.step();
    rig.post(CtlMsg::GraphRetire { id: 1 });
    rig.post(CtlMsg::OutputStop {
        mode: OutputMode::Gentle,
    });
    let _ = rig.step();
    assert!(rig
        .engine
        .buses()
        .slots
        .iter()
        .any(|slot| slot.state == SlotState::Retiring));
    let mut idle = false;
    for _ in 0..80 {
        let _ = rig.step();
        if phases(rig.acks()).contains(&OutputPhase::Idle) {
            idle = true;
            break;
        }
    }
    assert!(idle, "tail hold finishes");
    let _ = rig.step();
    assert!(rig
        .engine
        .buses()
        .slots
        .iter()
        .all(|slot| slot.state != SlotState::Retiring));
}

#[test]
fn frames_zero_non_release_cell_ramp_holds_target_and_release_drops() {
    use crate::dsp::cells::{AtomicCells, CellId, CellRead};
    use crate::dsp::ramp::{CellRamps, RampedCells};

    let cell = CellId::new(3);
    let cells = AtomicCells::new(64);
    assert!(cells.set(cell, 0.25));
    let mut ramps = CellRamps::new(64);
    assert!(ramps.apply(cell, 1, 1, 0.75, 0, false, 0, 0.25));
    assert_eq!(RampedCells::new(&ramps, &cells, 0).get(cell), 0.75);
    assert_eq!(RampedCells::new(&ramps, &cells, 100).get(cell), 0.75);
    assert!(ramps.apply(cell, 1, 2, 0.0, 0, true, 100, 0.75));
    assert_eq!(RampedCells::new(&ramps, &cells, 100).get(cell), 0.25);
    assert_eq!(ramps.metadata(cell), None);
}
