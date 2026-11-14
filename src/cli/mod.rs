//! The `vactr` CLI (design 14.5.10, `command.md`): argument parsing
//! (`args`), the verbs (`repl`, `run`, `serve`, `get`) and the native
//! session socket (`ws`). Besides `pkg::native` and `lsp`, this is the
//! only part of the crate that uses `std::process`, `std::net` and
//! `std::thread` (invariant V5).
//!
//! Every verb shares one `Session` built by [`build_session`] and one
//! host timebase, [`HostClock`]: a real audio clock under `--host native`,
//! or a virtual clock advanced one `TICK_PERIOD` per step under `--host
//! noop` (or when the `host-native` feature is not compiled in), so
//! `--cycles` runs end deterministically with no sleeping (14.5.10, S6).

pub mod args;
mod get;
mod repl;
mod run;
#[cfg(feature = "host-native")]
mod serve;
#[cfg(feature = "host-native")]
pub(crate) mod ws;

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::dsp::caps::CapabilitySet;
use crate::host::caps::Hosts;
#[cfg(feature = "host-native")]
use crate::ns::load::SourceLoader;
use crate::pkg::cache::CacheBackend;
use crate::pkg::lock::LockFile;
use crate::pkg::native::{pkg_cache_root, FsCache};
use crate::pkg::store::LOCK_FILE;
#[cfg(feature = "host-native")]
use crate::sched::runtime::RuntimeConfig;
use crate::session::console::{format_diag, format_failure};
use crate::session::{EvalOutcome, Session, SessionConfig};

use args::{parse, Command, HostChoice, UsageError};

/// The evaluator tick period every verb drives its host on. This mirrors
/// `host::native::TICK_PERIOD` (5 ms, design 12.8.10); the CLI keeps its
/// own copy so it builds without the `host-native` feature.
pub(crate) const TICK_PERIOD: std::time::Duration = std::time::Duration::from_millis(5);

/// The host timebase driving a session (14.5.10).
pub(crate) enum HostClock {
    /// The real audio clock (`--host native`).
    #[cfg(feature = "host-native")]
    Native(crate::host::native::FrameClock),
    /// A virtual clock advanced by [`HostClock::advance`] only.
    Virtual(Cell<f64>),
}

impl HostClock {
    /// Host seconds.
    pub(crate) fn now(&self) -> f64 {
        match self {
            #[cfg(feature = "host-native")]
            HostClock::Native(c) => c.now(),
            HostClock::Virtual(v) => v.get(),
        }
    }

    /// Advances the virtual clock by `dt` seconds; a no-op on a real
    /// clock, which advances itself.
    pub(crate) fn advance(&self, dt: f64) {
        match self {
            #[cfg(feature = "host-native")]
            HostClock::Native(_) => {}
            HostClock::Virtual(v) => v.set(v.get() + dt),
        }
    }
}

/// Runs `main` over the process arguments and exits with its code (the
/// binary's entry point; `std::process` stays inside `cli`).
pub fn run() -> ! {
    std::process::exit(main(std::env::args_os().collect()))
}

/// Runs the parsed command line; the process exit code (`command.md`).
#[must_use]
pub fn main(args: Vec<OsString>) -> i32 {
    let strings: Vec<String> = args
        .iter()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match parse(&strings) {
        Ok(Command::Version) => {
            println!("vactr {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Ok(Command::Help) => {
            println!("{}", args::USAGE);
            0
        }
        Ok(Command::Repl { host }) => repl::main(host, &cwd),
        Ok(Command::Run { file, host, cycles }) => run::main(&file, host, cycles, &cwd),
        Ok(Command::Serve {
            file,
            host,
            port,
            bind,
        }) => serve_dispatch(file, host, port, bind, &cwd),
        Ok(Command::Get { target, store }) => get::main(target, store, &cwd),
        Ok(Command::Lsp { session }) => lsp_verb(session),
        Err(UsageError(message)) => {
            eprintln!("vactr: {message}");
            eprintln!("{}", args::USAGE);
            2
        }
    }
}

#[cfg(feature = "host-native")]
fn serve_dispatch(
    file: Option<PathBuf>,
    host: HostChoice,
    port: u16,
    bind: std::net::IpAddr,
    cwd: &Path,
) -> i32 {
    serve::main(file, host, port, bind, cwd)
}

#[cfg(not(feature = "host-native"))]
fn serve_dispatch(
    _file: Option<PathBuf>,
    _host: HostChoice,
    _port: u16,
    _bind: std::net::IpAddr,
    _cwd: &Path,
) -> i32 {
    eprintln!("vactr: `serve` needs the `host-native` feature");
    1
}

#[cfg(feature = "lsp")]
fn lsp_verb(session: Option<String>) -> i32 {
    crate::lsp::run_stdio(session.as_deref())
}

#[cfg(not(feature = "lsp"))]
fn lsp_verb(_session: Option<String>) -> i32 {
    eprintln!("vactr was built without the lsp feature");
    1
}

/// `./vactr.lock`, when present; a parse error is printed and the
/// session runs unlocked (packages then fail `package-not-locked`).
fn read_lock(cwd: &Path) -> Option<LockFile> {
    let path = cwd.join(LOCK_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    match LockFile::parse(&text) {
        Ok(lock) => Some(lock),
        Err(e) => {
            eprintln!("vactr: {}: {e}", path.display());
            None
        }
    }
}

/// The verified package cache at `$VACTR_HOME/pkg`; a failure to open
/// it is a warning, and the session runs without a cache (S6).
fn open_cache() -> Option<Box<dyn CacheBackend>> {
    match FsCache::new(&pkg_cache_root()) {
        Ok(cache) => Some(Box::new(cache)),
        Err(e) => {
            eprintln!("vactr: package cache unavailable: {e}");
            None
        }
    }
}

/// Builds the session every verb (`repl`, `run`, `serve`) evaluates
/// against, and the host timebase it ticks on.
fn build_session(host: HostChoice, cwd: &Path) -> Result<(Session, HostClock), String> {
    let lock = read_lock(cwd);
    let cache = open_cache();
    #[cfg(feature = "host-native")]
    {
        build_session_native(host, cwd, lock, cache)
    }
    #[cfg(not(feature = "host-native"))]
    {
        if host == HostChoice::NativeInput {
            return Err("`--audio-in` requires the `host-native` build feature".into());
        }
        if host == HostChoice::Native {
            eprintln!("vactr: built without the `host-native` feature; using `--host noop`");
        }
        let mut cfg = SessionConfig::new(CapabilitySet::native());
        cfg.lock = lock;
        cfg.cache = cache;
        Ok((
            Session::new(cfg, Hosts::noop()),
            HostClock::Virtual(Cell::new(0.0)),
        ))
    }
}

#[cfg(feature = "host-native")]
fn build_session_native(
    host: HostChoice,
    cwd: &Path,
    lock: Option<LockFile>,
    cache: Option<Box<dyn CacheBackend>>,
) -> Result<(Session, HostClock), String> {
    use std::rc::Rc;

    use crate::host::native::{NativeConfig, NativeHosts, NativeSampleLoader};
    use crate::ns::insts::InstRegistry;
    use crate::sched::cells::Tier;

    let native_cfg = NativeConfig {
        audio_in: host == HostChoice::NativeInput,
        base: cwd.to_path_buf(),
        sample_roots: vec![cwd.to_path_buf()],
        ..NativeConfig::default()
    };
    // Even under `--host noop`, `load` reads through the real sample-root
    // loader; only the audio/MIDI/render hosts are inert.
    let loader: Box<dyn SourceLoader> = Box::new(NativeSampleLoader::new(
        &native_cfg.sample_roots,
        &native_cfg.base,
    ));
    // Shared with the session below, so the audio host's bus taps resolve
    // named buses (`scope`, `spectrum`, `capture`) through the same
    // instrument registry the evaluator installs into (14.5.9).
    let reg = InstRegistry::shared();
    let (hosts, runtime, clock) = match host {
        HostChoice::Native | HostChoice::NativeInput => {
            match NativeHosts::open_with_bus_names(&native_cfg, Rc::new(Rc::clone(&reg))) {
                Ok(native) => {
                    for d in &native.diags {
                        eprintln!("vactr: {d}");
                    }
                    let runtime = RuntimeConfig {
                        cell_pool: native.cells.capacity(),
                        tier: Tier::Native(native.cells.clone()),
                        sample_rate: native.clock.sample_rate(),
                        ..RuntimeConfig::default()
                    };
                    (native.hosts, runtime, HostClock::Native(native.clock))
                }
                Err(d) => {
                    if host == HostChoice::NativeInput {
                        return Err(format!("requested audio input is unavailable: {d}"));
                    }
                    eprintln!("vactr: {d}; falling back to `--host noop`");
                    (
                        Hosts::noop(),
                        RuntimeConfig::default(),
                        HostClock::Virtual(Cell::new(0.0)),
                    )
                }
            }
        }
        HostChoice::Noop => (
            Hosts::noop(),
            RuntimeConfig::default(),
            HostClock::Virtual(Cell::new(0.0)),
        ),
    };
    let mut cfg = SessionConfig::new(CapabilitySet::native());
    cfg.runtime = runtime;
    cfg.lock = lock;
    cfg.cache = cache;
    cfg.loader = loader;
    cfg.insts = Some(reg);
    Ok((Session::new(cfg, hosts), clock))
}

/// Prints one `eval`'s diagnostics, drained faults, console output and
/// per-form failures, in that order (`run`, `serve`'s optional file).
pub(crate) fn print_eval_outcome(session: &Session, outcome: &EvalOutcome, out: &mut dyn Write) {
    for d in &outcome.diagnostics {
        let _ = writeln!(out, "{}", format_diag(&session.file_name(d.span.file), d));
    }
    for f in &outcome.faults {
        let _ = writeln!(out, "{}", format_failure(f));
    }
    for line in &outcome.console {
        let _ = writeln!(out, "{line}");
    }
    for form in &outcome.forms {
        if let Some(f) = &form.failure {
            let _ = writeln!(out, "{}", format_failure(f));
        }
    }
}
