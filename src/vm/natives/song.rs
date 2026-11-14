//! Explicit finite-song natives. All construction runs on the control thread.
//! Structural arguments are forced once with bounded work; prepared callbacks
//! borrow the current VM synchronously in Query mode, never a frozen snapshot.
use std::collections::BTreeMap;
use std::rc::Rc;

use crate::clock::tempo::Tempo;
use crate::ns::namespace::Prelude;
use crate::ns::stage::StagedEffect;
use crate::pattern::build::pattern_of;
use crate::pattern::{InputCells, TimeSpan};
use crate::song::{
    self, InstrumentSelector, Part, RepeatSeedMode, ResolvedNote, Song, SongLimits, SongSettings,
};
use crate::value::intern::{intern_kw, KwId};
use crate::value::key::Key;
use crate::value::ratio::Ratio64;
use crate::value::value::{ListVal, Sound, Value};
use crate::vm::call::NativeCx;
use crate::vm::fail::{FailCode, Failure};
use crate::vm::natives::pattern::named;
use crate::vm::natives::{arg, int_of, type_err};
use crate::vm::query_vm::VmQuery;
use crate::vm::vm::int_value;

type Named<'a> = &'a [(KwId, Value)];
type ResultValue = Result<Value, Failure>;

/// Registers the eleven explicit song APIs without replacing legacy repeat/cat.
pub fn register(p: &mut Prelude) {
    p.register("part", part);
    p.register("part-repeat", repeat);
    p.register("sequence", sequence);
    p.register("replace-track", replace);
    p.register("transform-instrument", transform);
    p.register("part-events", events);
    p.register("delete-event", delete);
    p.register("overwrite-region", overwrite);
    p.register("instrument-fx", fx);
    p.register("song", descriptor);
    p.register("play-song", play);
}

fn limits(cx: &NativeCx<'_>) -> SongLimits {
    cx.vm
        .song_work()
        .map_or_else(SongLimits::default, |work| work.borrow().limits)
}
fn charge(cx: &NativeCx<'_>, count: usize) -> Result<(), Failure> {
    if let Some(work) = cx.vm.song_work() {
        work.borrow_mut().charge(count as u64)?;
    }
    Ok(())
}

/// Explicit work stack prevents nested literals from consuming the Rust stack.
/// Pattern/function internals stay lazy; only structural containers are forced.
fn structural(cx: &mut NativeCx<'_>, value: &Value) -> ResultValue {
    enum Work {
        Visit(Value, u32),
        List(usize, Option<Rc<crate::value::value::ListProv>>),
        Dict(Vec<Key>),
    }
    let limits = limits(cx);
    let initial_depth = cx.vm.song_query_depth()?;
    charge(cx, 1)?;
    let mut work = vec![Work::Visit(value.clone(), initial_depth)];
    let mut done = Vec::new();
    let mut visited = 0_u32;
    let mut scheduled = 1_u32;
    let admission = limits
        .max_nodes
        .min(u32::try_from(cx.vm.fuel()).unwrap_or(u32::MAX));
    while let Some(item) = work.pop() {
        match item {
            Work::Visit(value, depth) => {
                cx.tick()?;
                visited += 1;
                if depth > limits.max_depth {
                    return Err(Failure::new(
                        FailCode::DepthExceeded,
                        "song argument nesting exceeded",
                    ));
                }
                if visited > admission {
                    return Err(Failure::new(
                        FailCode::FuelExhausted,
                        "song argument work exceeded",
                    ));
                }
                match value {
                    Value::VarRef(slot) => {
                        let value = cx.vm.read_slot(&slot)?;
                        scheduled = admit_children(scheduled, 1, admission)?;
                        charge(cx, 1)?;
                        work.push(Work::Visit(value, depth + 1));
                    }
                    Value::Thunk(_) => {
                        let value = pure_thunk(cx, &value)?;
                        scheduled = admit_children(scheduled, 1, admission)?;
                        charge(cx, 1)?;
                        work.push(Work::Visit(value, depth + 1));
                    }
                    Value::List(list) => {
                        scheduled = admit_children(scheduled, list.items.len(), admission)?;
                        if list.items.len() > limits.max_nodes as usize {
                            return Err(Failure::new(
                                FailCode::FuelExhausted,
                                "song argument list exceeded",
                            ));
                        }
                        charge(
                            cx,
                            list.items
                                .len()
                                .checked_add(1)
                                .ok_or_else(|| type_err("song list cost overflow"))?,
                        )?;
                        work.push(Work::List(list.items.len(), list.prov.clone()));
                        work.extend(
                            list.items
                                .iter()
                                .rev()
                                .cloned()
                                .map(|v| Work::Visit(v, depth + 1)),
                        );
                    }
                    Value::Dict(dict) => {
                        scheduled = admit_children(scheduled, dict.len(), admission)?;
                        if dict.len() > limits.max_nodes as usize {
                            return Err(Failure::new(
                                FailCode::FuelExhausted,
                                "song argument dictionary exceeded",
                            ));
                        }
                        charge(
                            cx,
                            dict.len()
                                .checked_mul(2)
                                .and_then(|n| n.checked_add(1))
                                .ok_or_else(|| type_err("song dictionary cost overflow"))?,
                        )?;
                        work.push(Work::Dict(dict.keys().cloned().collect()));
                        work.extend(
                            dict.values()
                                .rev()
                                .cloned()
                                .map(|v| Work::Visit(v, depth + 1)),
                        );
                    }
                    other => {
                        charge(cx, 1)?;
                        done.push(other);
                    }
                }
            }
            Work::List(size, prov) => {
                charge(
                    cx,
                    size.checked_add(1)
                        .ok_or_else(|| type_err("song list cost overflow"))?,
                )?;
                let items = done.split_off(done.len() - size).into_boxed_slice();
                done.push(Value::List(Rc::new(ListVal { items, prov })));
            }
            Work::Dict(keys) => {
                charge(
                    cx,
                    keys.len()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(|| type_err("song dictionary cost overflow"))?,
                )?;
                let values = done.split_off(done.len() - keys.len());
                done.push(Value::dict(keys.into_iter().zip(values).collect()));
            }
        }
    }
    done.pop().ok_or_else(|| type_err("missing song argument"))
}
fn admit_children(scheduled: u32, children: usize, limit: u32) -> Result<u32, Failure> {
    let total = u32::try_from(children)
        .ok()
        .and_then(|n| scheduled.checked_add(n))
        .filter(|n| *n <= limit)
        .ok_or_else(|| {
            Failure::new(
                FailCode::FuelExhausted,
                "song argument queued work exceeded",
            )
        })?;
    Ok(total)
}
fn pure_thunk(cx: &mut NativeCx<'_>, value: &Value) -> ResultValue {
    use crate::pattern::eval::QueryVm;
    let mut vm = VmQuery::new(cx.vm, cx.ns);
    let saved = vm.take_output();
    let result = vm.call(value, &[]);
    let output = vm.take_output();
    vm.put_output(saved);
    let result = result?;
    if !output.is_empty() {
        return Err(Failure::new(
            FailCode::EffectInQuery,
            "song structural argument emitted output",
        ));
    }
    Ok(result)
}
/// Force only the outer kit wrapper, without evaluating unrelated members.
fn outer(cx: &mut NativeCx<'_>, mut value: Value) -> ResultValue {
    for _ in 0..limits(cx).max_depth {
        cx.tick()?;
        value = match value {
            Value::VarRef(slot) => cx.vm.read_slot(&slot)?,
            Value::Thunk(_) => pure_thunk(cx, &value)?,
            other => return Ok(other),
        };
    }
    Err(Failure::new(
        FailCode::DepthExceeded,
        "song outer argument nesting exceeded",
    ))
}
fn value(cx: &mut NativeCx<'_>, a: &[Value], i: usize) -> ResultValue {
    structural(cx, &arg(a, i))
}
fn part_arg(cx: &mut NativeCx<'_>, a: &[Value]) -> Result<Rc<Part>, Failure> {
    match value(cx, a, 0)? {
        Value::Part(p) => Ok(p),
        _ => Err(type_err("expected a finite Part")),
    }
}
fn keyword(value: &Value) -> Result<KwId, Failure> {
    match value {
        Value::Keyword(k) => Ok(*k),
        _ => Err(type_err("expected a keyword")),
    }
}
fn ratio(value: &Value) -> Result<Ratio64, Failure> {
    match value {
        Value::Int(i) => Ok(Ratio64::from_int(i64::from(*i))),
        Value::Int64(i) => Ok(Ratio64::from_int(*i)),
        Value::Ratio(r) => Ok(*r),
        _ => Err(type_err(
            "song time/settings require an exact integer or ratio",
        )),
    }
}
fn part_value(part: Result<Part, Failure>) -> ResultValue {
    part.map(|p| Value::Part(Rc::new(p)))
}
fn part(cx: &mut NativeCx<'_>, a: &[Value], kw: Named<'_>) -> ResultValue {
    let tracks = value(cx, a, 0)?;
    let Value::Dict(tracks) = tracks else {
        return Err(type_err("part tracks must be a dictionary"));
    };
    let duration = named(kw, "duration").ok_or_else(|| type_err("part requires duration:"))?;
    let duration = ratio(&structural(cx, duration)?)?;
    let mut patterns = BTreeMap::new();
    for (key, value) in tracks.iter() {
        let Key::Kw(key) = key else {
            return Err(type_err("part track keys must be keywords"));
        };
        charge(cx, 2)?;
        patterns.insert(*key, pattern_of(value, cx.span)?);
    }
    charge(cx, 1)?;
    part_value(Part::capture(patterns, duration, &limits(cx)))
}
fn repeat(cx: &mut NativeCx<'_>, a: &[Value], kw: Named<'_>) -> ResultValue {
    let part = part_arg(cx, a)?;
    let count = song::checked_repeat_count(ratio(&value(cx, a, 1)?)?)?;
    let mode = match named(kw, "seed-mode") {
        None => RepeatSeedMode::Same,
        Some(v) => match keyword(&structural(cx, v)?)? {
            k if k == intern_kw("same") => RepeatSeedMode::Same,
            k if k == intern_kw("vary") => RepeatSeedMode::Vary,
            _ => return Err(type_err("seed-mode must be :same or :vary")),
        },
    };
    charge(cx, 1)?;
    part_value(part.repeat(count, mode, &limits(cx)))
}
fn sequence(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let Value::List(list) = value(cx, a, 0)? else {
        return Err(type_err("sequence requires a list of Parts"));
    };
    charge(
        cx,
        list.items
            .len()
            .checked_add(1)
            .ok_or_else(|| type_err("song sequence cost overflow"))?,
    )?;
    let parts = list
        .items
        .iter()
        .map(|v| match v {
            Value::Part(p) => Ok(Rc::clone(p)),
            _ => Err(type_err("sequence elements must be finite Parts")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    part_value(Part::sequence(parts, &limits(cx)))
}
fn replace(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let p = part_arg(cx, a)?;
    let track = keyword(&value(cx, a, 1)?)?;
    let pattern = pattern_of(&value(cx, a, 2)?, cx.span)?;
    part_value(song::replace_track(&p, track, pattern))
}
fn selector(cx: &mut NativeCx<'_>, v: Value) -> Result<InstrumentSelector, Failure> {
    let resolved = if let Value::Keyword(k) = v {
        use crate::pattern::eval::QueryVm;
        let kit = VmQuery::new(cx.vm, cx.ns).sound_kit()?;
        let Value::Dict(kit) = outer(cx, kit)? else {
            return Err(type_err("sound-kit must be a dictionary"));
        };
        match kit.get(&Key::Kw(k)) {
            Some(v) => v.clone(),
            None => VmQuery::new(cx.vm, cx.ns).inst_sound(k).ok_or_else(|| {
                Failure::new(FailCode::UnknownSound, "unknown song instrument selector")
            })?,
        }
    } else {
        v
    };
    let resolved = structural(cx, &resolved)?;
    let sound = |v: &Value| match v {
        Value::Sound(s) => Ok((**s).clone()),
        Value::Inst(i) => Ok(Sound::Inst(*i)),
        _ => Err(type_err(
            "selector must resolve to a sound or complete sound bank",
        )),
    };
    let family = match resolved {
        Value::List(l) => l.items.iter().map(sound).collect::<Result<Vec<_>, _>>()?,
        v => vec![sound(&v)?],
    };
    InstrumentSelector::new(family)
}
fn transform(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let p = part_arg(cx, a)?;
    let track = keyword(&value(cx, a, 1)?)?;
    let selected = value(cx, a, 2)?;
    let selected = selector(cx, selected)?;
    // Fn forcing mask retains callable input; resolving references never calls it.
    let mut callback = arg(a, 3);
    let mut reads = 0;
    while let Value::VarRef(slot) = callback {
        cx.tick()?;
        reads += 1;
        if reads > limits(cx).max_depth {
            return Err(Failure::new(
                FailCode::DepthExceeded,
                "song callback reference nesting exceeded",
            ));
        }
        callback = cx.vm.read_slot(&slot)?;
    }
    if !matches!(callback, Value::Fn(_) | Value::Thunk(_) | Value::Native(_)) {
        return Err(type_err(
            "song transform requires a callable pattern transform",
        ));
    }
    let limits = limits(cx);
    let mut vm = VmQuery::new(cx.vm, cx.ns);
    part_value(song::transform_instrument(
        &p,
        track,
        selected,
        callback,
        &mut song::SongBuildCtx {
            vm: &mut vm,
            limits: &limits,
        },
    ))
}
fn events(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let p = part_arg(cx, a)?;
    let track = keyword(&value(cx, a, 1)?)?;
    if !p.tracks().contains(&track) {
        return Err(type_err("unknown part track"));
    }
    let span = TimeSpan::new(ratio(&value(cx, a, 2)?)?, ratio(&value(cx, a, 3)?)?)?;
    if span.begin >= span.end || span.begin < Ratio64::ZERO || span.end > p.duration() {
        return Err(type_err("part-events range is outside the Part"));
    }
    let cells = InputCells::new();
    let limits = limits(cx);
    let work = cx.vm.song_work();
    let depth = cx.vm.song_query_depth()?;
    let ns = cx.ns;
    let rows = cx.vm.with_song_query_frames(|raw_vm| {
        let mut vm = VmQuery::new(raw_vm, ns);
        let mut context = song::SongQueryCtx {
            vm: &mut vm,
            cells: &cells,
            seed: 0,
            tempo: Tempo::default(),
            limits: &limits,
        };
        match work {
            Some(work) => song::query::query_part_metered(&p, span, &mut context, work, depth),
            None => song::query_part(&p, span, &mut context),
        }
    })?;
    // Scan and allocate the exact five-field row, whole list, handle and sound
    // before constructing them; handles are moved from the authentic query.
    for row in &rows {
        charge(cx, 1)?;
        if row.track == track {
            charge(
                cx,
                12_usize
                    .checked_add(row.handle.occurrence().producer_ordinals.len())
                    .ok_or_else(|| type_err("song row cost overflow"))?,
            )?;
        }
    }
    charge(cx, 1)?;
    Ok(Value::list(
        rows.into_iter()
            .filter(|row| row.track == track)
            .map(|row| {
                let whole = row.event.whole.unwrap_or(row.event.part);
                let note = match row.tone {
                    None => Value::Nil,
                    Some(ResolvedNote::Int(i)) => int_value(i),
                    Some(ResolvedNote::Ratio(r)) => Value::Ratio(r),
                    Some(ResolvedNote::Float32(x)) => Value::Float(x),
                    Some(ResolvedNote::Float64(x)) => Value::Float64(x),
                };
                Value::dict(BTreeMap::from([
                    (
                        Key::Kw(intern_kw("handle")),
                        Value::EventHandle(Rc::new(row.handle)),
                    ),
                    (Key::Kw(intern_kw("track")), Value::Keyword(row.track)),
                    (
                        Key::Kw(intern_kw("whole")),
                        Value::list(vec![Value::Ratio(whole.begin), Value::Ratio(whole.end)]),
                    ),
                    (
                        Key::Kw(intern_kw("instrument")),
                        Value::Sound(Rc::new(row.instrument)),
                    ),
                    (Key::Kw(intern_kw("note")), note),
                ]))
            })
            .collect(),
    ))
}
fn delete(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let p = part_arg(cx, a)?;
    let Value::EventHandle(handle) = value(cx, a, 1)? else {
        return Err(type_err("delete-event requires an opaque event handle"));
    };
    part_value(song::delete_event(&p, &handle))
}
fn overwrite(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let p = part_arg(cx, a)?;
    let track = keyword(&value(cx, a, 1)?)?;
    let region = TimeSpan::new(ratio(&value(cx, a, 2)?)?, ratio(&value(cx, a, 3)?)?)?;
    let pattern = pattern_of(&value(cx, a, 4)?, cx.span)?;
    part_value(song::overwrite_region(&p, track, region, pattern))
}
fn fx(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let p = part_arg(cx, a)?;
    let track = keyword(&value(cx, a, 1)?)?;
    let selected = value(cx, a, 2)?;
    let selected = selector(cx, selected)?;
    let template = keyword(&value(cx, a, 3)?)?;
    let registry =
        cx.vm.dsp.registry.as_ref().ok_or_else(|| {
            Failure::new(FailCode::HostUnavailable, "song FX registry unavailable")
        })?;
    if registry.borrow().bus(template).is_none() {
        return Err(type_err(
            "song instrument-fx requires a declared bus-chain template",
        ));
    }
    part_value(song::instrument_fx(&p, track, selected, template))
}
fn descriptor(cx: &mut NativeCx<'_>, a: &[Value], kw: Named<'_>) -> ResultValue {
    let p = part_arg(cx, a)?;
    let mut settings = SongSettings::default();
    for (k, v) in kw {
        let v = structural(cx, v)?;
        match &*crate::value::intern::name_of_kw(*k) {
            "bpm" => settings.bpm = ratio(&v)?,
            "cycle-beats" => settings.cycle_beats = ratio(&v)?,
            "tail-seconds" => settings.tail_seconds = ratio(&v)?,
            "seed" => {
                settings.seed = u64::try_from(int_of(&v, "song seed")?)
                    .map_err(|_| type_err("song seed must be nonnegative"))?
            }
            "meter" => {
                let Value::List(l) = v else {
                    return Err(type_err("song meter requires [numerator denominator]"));
                };
                if l.items.len() != 2 {
                    return Err(type_err("song meter requires two integers"));
                }
                let convert = |v: &Value| {
                    u32::try_from(int_of(v, "song meter")?)
                        .map_err(|_| type_err("song meter integer is out of range"))
                };
                settings.meter = [convert(&l.items[0])?, convert(&l.items[1])?];
            }
            _ => return Err(type_err("unknown song setting")),
        }
    }
    Song::new(p, settings).map(|s| Value::Song(Rc::new(s)))
}
fn play(cx: &mut NativeCx<'_>, a: &[Value], _: Named<'_>) -> ResultValue {
    let Value::Song(song) = value(cx, a, 0)? else {
        return Err(type_err("play-song requires a finite Song"));
    };
    cx.stage(StagedEffect::PlaySong(Rc::clone(&song)))?;
    Ok(Value::Song(song))
}
