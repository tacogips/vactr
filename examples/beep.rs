//! `cargo run --example beep [-- FILE.wav]` (design 12.8.10).
//!
//! Opens the native hosts, builds an `Evaluator` and a `Runtime` over one
//! instrument registry, evaluates `s :analog > note [:a4] > once` (or
//! `s {sample PATH} > once` for a WAV file argument), then ticks the
//! runtime on the 5 ms timer for two seconds and exits 0. Any setup
//! failure prints its diagnostic and exits 1.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use vactr::dsp::caps::CapabilitySet;
use vactr::host::caps::InstResolver;
use vactr::host::native::{NativeConfig, NativeHosts, TickSource, TICK_PERIOD};
use vactr::ns::evaluator::Evaluator;
use vactr::ns::insts::InstRegistry;
use vactr::ns::namespace::Prelude;
use vactr::reader::span::FileId;
use vactr::sched::cells::Tier;
use vactr::sched::runtime::{Runtime, RuntimeConfig};

const RUN_FOR: Duration = Duration::from_secs(2);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("beep: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// The program to play, and the extra sample root it needs.
fn program(arg: Option<String>) -> Result<(String, Option<PathBuf>), String> {
    let Some(arg) = arg else {
        return Ok(("s :analog > note [:a4] > once".into(), None));
    };
    let file = std::fs::canonicalize(&arg).map_err(|e| format!("cannot read `{arg}`: {e}"))?;
    let text = file.to_string_lossy().into_owned();
    let literal_ok = text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._~-/".contains(c));
    if !literal_ok || text.contains("//") {
        return Err(format!(
            "`{text}` cannot be written as a path literal (use [A-Za-z0-9._~-] only)"
        ));
    }
    let dir = file.parent().map(Path::to_path_buf);
    Ok((format!("s {{sample {text}}} > once"), dir))
}

fn run() -> Result<(), String> {
    let (source, extra_root) = program(std::env::args().nth(1))?;
    let mut cfg = NativeConfig::default();
    cfg.sample_roots.extend(extra_root);
    let native = NativeHosts::open(&cfg).map_err(|d| d.to_string())?;
    for d in &native.diags {
        eprintln!("beep: {d}");
    }
    let clock = native.clock.clone();
    let registry = InstRegistry::shared();
    let resolver: Rc<dyn InstResolver> = Rc::new(Rc::clone(&registry));
    let rcfg = RuntimeConfig {
        cell_pool: native.cells.capacity(),
        tier: Tier::Native(native.cells.clone()),
        ..RuntimeConfig::default()
    };
    let (mut rt, sink) = Runtime::new(native.hosts, resolver, CapabilitySet::native(), rcfg);
    let mut ev = Evaluator::with_insts(
        Prelude::core(),
        Box::new(native.loader.clone()),
        Box::new(sink),
        registry,
    );

    let outcomes = ev
        .eval_str(&source, FileId::CONSOLE)
        .map_err(|d| d.to_string())?;
    for o in &outcomes {
        for d in &o.diags {
            eprintln!("beep: {d}");
        }
        if let Err(f) = &o.value {
            return Err(format!("{}: {}", f.code, f.message));
        }
    }
    let rep = rt.drain(&mut ev);
    for d in &rep.diags {
        eprintln!("beep: {d}");
    }
    if let Some(f) = rep.faults.first() {
        return Err(format!("{}: {}", f.code, f.message));
    }

    let (_ticks, wakes) = TickSource::start(TICK_PERIOD);
    let start = Instant::now();
    let mut committed = 0;
    while start.elapsed() < RUN_FOR {
        if let Err(RecvTimeoutError::Disconnected) = wakes.recv_timeout(Duration::from_millis(100))
        {
            return Err("the tick thread stopped".into());
        }
        let t = rt.tick(&mut ev, clock.now());
        committed += t.committed;
        for d in &t.diags {
            eprintln!("beep: {d}");
        }
        for f in &t.faults {
            eprintln!("beep: {}: {}", f.code, f.message);
        }
        for line in &t.console {
            println!("{line}");
        }
    }
    println!(
        "beep: played `{source}`; {committed} event(s) committed over {:.2}s of audio",
        clock.now()
    );
    Ok(())
}
