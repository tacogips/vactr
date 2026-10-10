//! Frozen mono events encoded against the original private graph authority.
use crate::dsp::controls::{self, CtlDomain, CtlRoute};
use crate::dsp::graph::InstId;
use crate::host::caps::{SongPhysicalBranch, SongReadyBundle};
use crate::host::wire::{AudioEvent, Ctl};
use crate::pattern::tuning::Tuning;
use crate::sched::slots::{CtlId, SlotId};
use crate::song::routing::{SongAudioEvent, SongBranchRoute};
use crate::song::snapshot::{FrozenControl, FrozenSongEvent};
use crate::song::ResolvedNote;
use crate::value::intern::name_of_kw;
use crate::value::ratio::Ratio64;
use crate::value::value::Value;
use crate::vm::fail::{FailCode, Failure};

fn scalar(value: &FrozenControl) -> Result<Value, Failure> {
    Ok(match value {
        FrozenControl::Nil => Value::Nil,
        FrozenControl::Bool(v) => Value::Bool(*v),
        FrozenControl::Keyword(v) => Value::Keyword(*v),
        FrozenControl::String(v) => Value::Str(v.clone()),
        FrozenControl::Number(ResolvedNote::Int(v)) => Value::Int64(*v),
        FrozenControl::Number(ResolvedNote::Ratio(v)) => Value::Ratio(*v),
        FrozenControl::Number(ResolvedNote::Float32(v)) => Value::Float(*v),
        FrozenControl::Number(ResolvedNote::Float64(v)) => Value::Float64(*v),
        FrozenControl::List(_) => {
            return Err(Failure::new(
                FailCode::Type,
                "non-scalar frozen event control",
            ))
        }
    })
}
fn tuning_value(value: &FrozenControl) -> Result<Value, Failure> {
    match value {
        FrozenControl::List(values) => Ok(Value::list(
            values
                .iter()
                .map(tuning_value)
                .collect::<Result<Vec<_>, _>>()?,
        )),
        value => scalar(value),
    }
}
fn push(event: &mut AudioEvent, id: CtlId, value: f32) -> Result<(), Failure> {
    if !value.is_finite() {
        return Err(Failure::new(
            FailCode::Type,
            "nonfinite frozen audio control",
        ));
    }
    if let Some((_, ctl)) = event.ctl[..usize::from(event.n_ctl)]
        .iter_mut()
        .find(|(key, _)| *key == id)
    {
        *ctl = Ctl::Const(value);
        Ok(())
    } else {
        event.push_ctl(id, Ctl::Const(value))
    }
}
pub(super) fn encode(
    ready: &SongReadyBundle,
    row: &FrozenSongEvent,
    branch: &SongBranchRoute,
    pool: SongPhysicalBranch,
    generation: u32,
    frame: u64,
    seconds_per_cycle: Ratio64,
) -> Result<SongAudioEvent, Failure> {
    let inventory = ready.prepared().snapshot().routing();
    let instrument = inventory
        .instruments
        .iter()
        .find(|i| i.graph.id == branch.resolved_instrument)
        .ok_or_else(|| Failure::new(FailCode::Type, "closed song instrument missing"))?;
    let tuning = row
        .controls
        .iter()
        .find(|(key, _)| name_of_kw(*key).as_ref() == "tuning")
        .map(|(_, value)| tuning_value(value))
        .transpose()?
        .and_then(|value| Tuning::from_control(&value))
        .transpose()?;
    let config = pool.initial.config;
    let mut event = AudioEvent::new(
        frame as f64 / f64::from(ready.clock().sample_rate),
        SlotId::new(config.branch.0),
        generation,
        InstId::new(config.instrument.id),
    );
    if let Some(sample) = &branch.sample {
        let key = ready
            .sample_resource(sample)
            .ok_or_else(|| Failure::new(FailCode::Type, "private sample lease missing"))?;
        let value = key.resource.id as f32;
        if f64::from(value) != f64::from(key.resource.id) {
            return Err(Failure::new(
                FailCode::Overflow,
                "sample control id is not exact",
            ));
        }
        let bank = controls::row("bank")
            .ok_or_else(|| Failure::new(FailCode::Type, "sample bank control missing"))?;
        push(&mut event, bank.ctl, value)?;
    }
    for (key, frozen) in &row.controls {
        let name = name_of_kw(*key);
        if name.as_ref() == "tuning" && tuning.is_some() {
            continue;
        }
        if matches!(name.as_ref(), "note" | "n") {
            continue;
        }
        if let Some(param) = instrument.parameters.iter().find(|p| p.name == *key) {
            push(&mut event, param.ctl, param.encode(&scalar(frozen)?)?)?;
            continue;
        }
        if name.as_ref() == "speed-fit" {
            let FrozenControl::List(values) = frozen else {
                return Err(Failure::new(
                    FailCode::Type,
                    "invalid frozen speed-fit marker",
                ));
            };
            if !(2..=3).contains(&values.len()) {
                return Err(Failure::new(
                    FailCode::Type,
                    "invalid frozen speed-fit arity",
                ));
            }
            let value = Value::list(values.iter().map(scalar).collect::<Result<Vec<_>, _>>()?);
            let fit = crate::pattern::combinators::region::SpeedFit::from_value(&value)
                .ok_or_else(|| Failure::new(FailCode::Type, "invalid frozen speed-fit recipe"))?;
            let data = branch
                .sample
                .as_ref()
                .and_then(|source| ready.sample_data(source))
                .ok_or_else(|| {
                    Failure::new(FailCode::Type, "speed-fit sample authority missing")
                })?;
            if data.channels == 0 || data.rate == 0 {
                return Err(Failure::new(
                    FailCode::Type,
                    "invalid retained sample geometry",
                ));
            }
            let seconds =
                data.frames.len() as f64 / f64::from(data.channels) / f64::from(data.rate);
            let speed = controls::row("speed")
                .ok_or_else(|| Failure::new(FailCode::Type, "speed control missing"))?;
            push(
                &mut event,
                speed.ctl,
                fit.resolve(seconds, seconds_per_cycle.to_f64()) as f32,
            )?;
            continue;
        }
        let control = controls::row(&name).ok_or_else(|| {
            Failure::new(
                FailCode::Type,
                format!("unknown frozen instrument control `{name}`"),
            )
        })?;
        if control.domain == CtlDomain::Resource {
            if name.as_ref() == "bank" && branch.sample.is_some() {
                continue;
            }
            return Err(Failure::new(
                FailCode::BeyondCapability,
                "event resource control lacks private captured binding",
            ));
        }
        if control.route == CtlRoute::Scheduler {
            match name.as_ref() {
                "cut" | "orbit" => {
                    let v = match frozen {
                        FrozenControl::Number(n) => n.to_f64(),
                        _ => return Err(Failure::new(FailCode::Type, "invalid event voice hint")),
                    };
                    if !v.is_finite() {
                        return Err(Failure::new(FailCode::Type, "invalid event voice hint"));
                    }
                    let shift = if name.as_ref() == "cut" { 8 } else { 0 };
                    event.voice_hint |= (v.clamp(0.0, 255.0) as u32) << shift;
                }
                "speed-fit" => {
                    return Err(Failure::new(
                        FailCode::BeyondCapability,
                        "speed-fit requires captured sample duration authority",
                    ))
                }
                _ => {}
            }
            continue;
        }
        push(
            &mut event,
            control.ctl,
            controls::encode(control, &scalar(frozen)?)?,
        )?;
    }
    if instrument.graph.nodes.iter().any(|node| {
        matches!(
            node,
            crate::dsp::graph::UGenSpec::DigitalDrumCore
                | crate::dsp::graph::UGenSpec::DigitalSnareCore
                | crate::dsp::graph::UGenSpec::DigitalMetalCore
                | crate::dsp::graph::UGenSpec::DigitalHatCore
                | crate::dsp::graph::UGenSpec::BassCore
        )
    }) {
        for (name, value) in [
            ("cps", 1.0 / seconds_per_cycle.to_f64()),
            ("onset-time", event.time.rem_euclid(3600.0)),
        ] {
            if !row
                .controls
                .iter()
                .any(|(key, _)| name_of_kw(*key).as_ref() == name)
            {
                let control = controls::row(name)
                    .ok_or_else(|| Failure::new(FailCode::Type, "implicit drum control missing"))?;
                push(&mut event, control.ctl, value as f32)?;
            }
        }
    }
    if let Some(note) = row.note {
        note.validate()?;
        let control = controls::row("freq")
            .ok_or_else(|| Failure::new(FailCode::Type, "frequency control missing"))?;
        let frequency = if let Some(tuning) = &tuning {
            tuning.freq(note.to_f64())?
        } else {
            Some(crate::sched::commit::note_to_freq(note.to_f64()))
        };
        if let Some(frequency) = frequency {
            push(&mut event, control.ctl, frequency as f32)?;
        }
    }
    if !row
        .controls
        .iter()
        .any(|(key, _)| name_of_kw(*key).as_ref() == "dur")
    {
        let whole = row.whole.unwrap_or(row.part);
        if let Some(control) = controls::row("dur") {
            let duration = whole
                .end
                .checked_sub(whole.begin)?
                .checked_mul(seconds_per_cycle)?;
            push(&mut event, control.ctl, duration.to_f64() as f32)?;
        }
    }
    Ok(SongAudioEvent {
        epoch: config.epoch,
        branch: config.branch,
        generation,
        frame,
        event,
    })
}
