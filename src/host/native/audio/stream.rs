use super::*;
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{SampleFormat, SupportedStreamConfig};

pub(super) fn open_with_outputs(
    cfg: &NativeConfig,
    output_channels: u8,
) -> Result<NativeAudioHost, Diagnostic> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| {
        unavailable("audio output is not available on this host (no output device)")
    })?;
    let supported = f32_config(&device)?;
    let channels = usize::from(supported.channels());
    if output_channels == 4 && channels != 4 {
        return Err(unavailable(
            "quad output needs a four-channel default device configuration",
        ));
    }
    let rate = supported.sample_rate().0;
    let config = supported.config();
    #[allow(clippy::cast_precision_loss)]
    let engine = crate::host::song_profile::song_engine_config(
        rate as f32,
        MAX_BLOCK,
        CapabilitySet::native(),
        StoreKind::NativeArc,
        output_channels,
    )
    .map_err(|_| unavailable("unsupported song audio device configuration"))?;
    let (mut this, mut side) = NativeAudioHost::pair(engine, AtomicCells::new(cfg.cells));
    let capture = if cfg.audio_in {
        let input_device = host.default_input_device().ok_or_else(|| {
            unavailable("audio input was requested but no default input device is available")
        })?;
        let input_config = input_f32_config(&input_device, rate)?;
        let input_channels = usize::from(input_config.channels());
        let (mut producer, consumer, counters) = capture::pair();
        let input_errors = Arc::clone(&this.stream_errors);
        let input_stream = input_device
            .build_input_stream(
                &input_config.config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    producer.push_interleaved(data, input_channels);
                },
                move |_| {
                    input_errors.fetch_add(1, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|e| {
                unavailable(format!(
                    "audio input was requested but its stream cannot open ({e})"
                ))
            })?;
        Some((input_stream, consumer, counters))
    } else {
        None
    };
    let errors = Arc::clone(&this.stream_errors);
    let (input_stream, mut input_consumer, counters) = match capture {
        Some((stream, consumer, counters)) => (Some(stream), Some(consumer), Some(counters)),
        None => (None, None, None),
    };
    let mut input_scratch = [0.0; 2 * MAX_BLOCK];
    let output_clock = this.output_clock.clone();
    let callback_clock = this.clock.clone();
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                output_clock.record(
                    info,
                    callback_clock.frames(),
                    u32::try_from(data.len() / channels).unwrap_or(u32::MAX),
                );
                if let Some(consumer) = &mut input_consumer {
                    render_captured(&mut side, consumer, &mut input_scratch, data, channels);
                } else {
                    side.render(data, channels);
                }
            },
            move |_| {
                errors.fetch_add(1, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| unavailable(format!("audio output is not available on this host ({e})")))?;
    if let Some(input) = &input_stream {
        input.play().map_err(|e| {
            unavailable(format!(
                "audio input was requested but its stream cannot start ({e})"
            ))
        })?;
    }
    stream
        .play()
        .map_err(|e| unavailable(format!("audio output is not available on this host ({e})")))?;
    this.output_clock.set_running(true);
    *this.stream.borrow_mut() = Some(stream);
    this.input_stream = input_stream;
    this.capture_counters = counters;
    Ok(this)
}

/// The device's default configuration when it is f32, else an f32 range
/// at the default rate (or the range's highest rate).
fn f32_config(device: &cpal::Device) -> Result<SupportedStreamConfig, Diagnostic> {
    let default = device
        .default_output_config()
        .map_err(|e| unavailable(format!("audio output is not available on this host ({e})")))?;
    if default.sample_format() == SampleFormat::F32 {
        return Ok(default);
    }
    let rate = default.sample_rate();
    let ranges = device
        .supported_output_configs()
        .map_err(|e| unavailable(format!("audio output is not available on this host ({e})")))?;
    let mut fallback = None;
    for r in ranges.filter(|r| r.sample_format() == SampleFormat::F32) {
        if r.min_sample_rate() <= rate && rate <= r.max_sample_rate() {
            return Ok(r.with_sample_rate(rate));
        }
        fallback.get_or_insert(r.with_max_sample_rate());
    }
    fallback.ok_or_else(|| {
        unavailable("audio output is not available on this host (no f32 output format)")
    })
}

/// A f32 input configuration at the already chosen output rate. Prefer
/// stereo, then mono; wider devices use their first two channels.
fn input_f32_config(device: &cpal::Device, rate: u32) -> Result<SupportedStreamConfig, Diagnostic> {
    let ranges = device.supported_input_configs().map_err(|e| {
        unavailable(format!(
            "audio input was requested but its formats are unavailable ({e})"
        ))
    })?;
    ranges
        .filter(|r| {
            r.sample_format() == SampleFormat::F32
                && super::capture_channel_rank(r.channels()).is_some()
                && r.min_sample_rate().0 <= rate
                && rate <= r.max_sample_rate().0
        })
        .min_by_key(|r| super::capture_channel_rank(r.channels()))
        .map(|r| r.with_sample_rate(cpal::SampleRate(rate)))
        .ok_or_else(|| {
            unavailable(format!(
                "audio input was requested but no f32 input supports the output rate {rate} Hz"
            ))
        })
}
