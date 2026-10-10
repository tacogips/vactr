//! Native loading of user-supplied six-operator SysEx voice files.

use std::rc::Rc;

use crate::dsp::ugen::fm::sysex::parse;
use crate::ns::load::LoaderHost;
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::vm::EffectMode;

/// Maximum SysEx file size accepted by the native (64 KiB).
pub(crate) const MAX_SYSEX_BYTES: u64 = 64 * 1024;

/// Reads and parses a single-voice or 32-voice six-operator SysEx file.
///
/// Parsing runs in the evaluator, so the resulting parameter lists are
/// ordinary language values and never cross onto the audio thread.
pub(crate) fn fm6_sysex(
    cx: &mut NativeCx<'_>,
    args: &[Value],
    _: &[(KwId, Value)],
) -> Result<Value, Failure> {
    if cx.effect_mode() == EffectMode::Query {
        return Err(Failure::new(
            FailCode::EffectInQuery,
            "`fm6-sysex` is not allowed inside a query",
        ));
    }
    let path = match args.first() {
        Some(Value::Path(path)) => Rc::clone(path),
        other => {
            return Err(Failure::new(
                FailCode::Type,
                format!(
                    "`fm6-sysex` expects a path, got {}",
                    other.map_or("nothing", kind_name)
                ),
            ));
        }
    };
    let Some(mut host) = cx.vm.take_host() else {
        return Err(no_loader());
    };
    let read = match host.downcast_mut::<LoaderHost>() {
        Some(LoaderHost(loader, _)) => loader.read_bytes(&path, MAX_SYSEX_BYTES),
        None => Err(no_loader()),
    };
    cx.vm.set_host(Some(host));
    let bytes = read?;
    let voices = parse(&bytes).map_err(|error| {
        Failure::new(
            FailCode::LoadFailed,
            format!("fm6-sysex `{}`: {error}", path.text),
        )
    })?;
    Ok(Value::list(
        voices
            .into_iter()
            .map(|voice| {
                Value::list(
                    voice
                        .params
                        .into_iter()
                        .map(|value| Value::Int(i32::from(value)))
                        .collect(),
                )
            })
            .collect(),
    ))
}

fn no_loader() -> Failure {
    Failure::new(FailCode::HostUnavailable, "no source loader is installed")
}
