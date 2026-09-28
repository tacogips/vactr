//! Hand-written argument parsing (design 14.5.10, `command.md`): the verbs
//! and flags of `vactrol`, with no external CLI-parser crate.
//!
//! Every parse error becomes a `UsageError` (exit 2). Flags accept both
//! `--flag value` and `--flag=value`.

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;

use crate::ns::pkg::PackageId;
use crate::pkg::manifest::valid_package_path;
use crate::pkg::semver::Version;

/// The audio/MIDI host a verb runs against.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HostChoice {
    Native,
    NativeInput,
    Noop,
}

/// Where `get` fetches packages from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StoreSpec {
    /// The default: `https://` over git (`command.md`).
    Git,
    /// `--store dir:<root>`.
    Dir(PathBuf),
}

/// A parsed invocation.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Command {
    Repl {
        host: HostChoice,
    },
    Run {
        file: PathBuf,
        host: HostChoice,
        cycles: Option<u64>,
    },
    Serve {
        file: Option<PathBuf>,
        host: HostChoice,
        port: u16,
        bind: IpAddr,
    },
    Get {
        target: Option<(PackageId, Option<Version>)>,
        store: StoreSpec,
    },
    Lsp {
        session: Option<String>,
    },
    Version,
    Help,
}

/// A usage error (exit 2): an unknown verb or flag, a missing or invalid
/// argument.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct UsageError(pub String);

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UsageError {}

fn usage(message: impl Into<String>) -> UsageError {
    UsageError(message.into())
}

/// The usage text printed on `help`/`--help`/no arguments (exit 0) and
/// alongside a usage error on stderr (exit 2).
pub const USAGE: &str = "\
usage: vactrol <verb> [options]

verbs:
  repl [--host native|noop] [--audio-in]
  run <file.vact> [--host native|noop] [--audio-in] [--cycles N]
  serve [<file.vact>] [--host native|noop] [--audio-in] [--port P] [--bind ADDR]
  get [<path>[@version]] [--store dir:<root>]
  lsp [--session <ws-url>]
  version | --version
  help | --help";

/// True when `arg` is `flag` or `flag=...`.
fn flag_matches(arg: &str, flag: &str) -> bool {
    arg == flag
        || arg
            .strip_prefix(flag)
            .is_some_and(|rest| rest.starts_with('='))
}

/// The value of the flag at `args[*i]` (inline after `=`, else the next
/// token), advancing `*i` past whatever it consumed.
fn take_value(args: &[String], i: &mut usize, flag: &str) -> Result<String, UsageError> {
    if let Some((_, v)) = args[*i].split_once('=') {
        return Ok(v.to_string());
    }
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| usage(format!("`{flag}` needs a value")))
}

fn parse_host(v: &str) -> Result<HostChoice, UsageError> {
    match v {
        "native" => Ok(HostChoice::Native),
        "noop" => Ok(HostChoice::Noop),
        other => Err(usage(format!(
            "`--host` must be `native` or `noop`, got `{other}`"
        ))),
    }
}

fn with_audio_in(host: HostChoice, audio_in: bool) -> Result<HostChoice, UsageError> {
    match (host, audio_in) {
        (HostChoice::Noop, true) => Err(usage("`--audio-in` requires `--host native`")),
        (_, true) => Ok(HostChoice::NativeInput),
        (host, false) => Ok(host),
    }
}

fn parse_store(v: &str) -> Result<StoreSpec, UsageError> {
    match v.strip_prefix("dir:") {
        Some(root) if !root.is_empty() => Ok(StoreSpec::Dir(PathBuf::from(root))),
        _ => Err(usage(format!("`--store` must be `dir:<root>`, got `{v}`"))),
    }
}

/// `<path>[@<version>]`.
fn parse_target(s: &str) -> Result<(PackageId, Option<Version>), UsageError> {
    let (path, version) = match s.split_once('@') {
        Some((p, v)) => (p, Some(v)),
        None => (s, None),
    };
    if !valid_package_path(path) {
        return Err(usage(format!(
            "`{path}` is not a lowercase `github.com/<owner>/<name>` path"
        )));
    }
    let version = match version {
        Some(v) => Some(
            Version::parse_tag(v)
                .ok_or_else(|| usage(format!("`{v}` is not a `vMAJOR.MINOR.PATCH` version")))?,
        ),
        None => None,
    };
    Ok((PackageId::new(path), version))
}

fn parse_repl(args: &[String]) -> Result<Command, UsageError> {
    let mut host = HostChoice::Native;
    let mut audio_in = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if flag_matches(a, "--host") {
            host = parse_host(&take_value(args, &mut i, "--host")?)?;
        } else if a == "--audio-in" {
            audio_in = true;
        } else {
            return Err(usage(format!("unknown flag `{a}` for `repl`")));
        }
        i += 1;
    }
    Ok(Command::Repl {
        host: with_audio_in(host, audio_in)?,
    })
}

fn parse_cycles(v: &str) -> Result<u64, UsageError> {
    let n: u64 = v
        .parse()
        .map_err(|_| usage(format!("`--cycles` must be a positive integer, got `{v}`")))?;
    if n == 0 {
        return Err(usage("`--cycles` must be a positive integer, got `0`"));
    }
    Ok(n)
}

fn parse_run(args: &[String]) -> Result<Command, UsageError> {
    let mut host = HostChoice::Native;
    let mut audio_in = false;
    let mut cycles = None;
    let mut file: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if flag_matches(a, "--host") {
            host = parse_host(&take_value(args, &mut i, "--host")?)?;
        } else if a == "--audio-in" {
            audio_in = true;
        } else if flag_matches(a, "--cycles") {
            cycles = Some(parse_cycles(&take_value(args, &mut i, "--cycles")?)?);
        } else if let Some(stripped) = a.strip_prefix("--") {
            return Err(usage(format!("unknown flag `--{stripped}` for `run`")));
        } else if file.is_none() {
            file = Some(PathBuf::from(a));
        } else {
            return Err(usage(format!("unexpected argument `{a}`")));
        }
        i += 1;
    }
    let file = file.ok_or_else(|| usage("`run` needs a file argument"))?;
    Ok(Command::Run {
        file,
        host: with_audio_in(host, audio_in)?,
        cycles,
    })
}

fn parse_serve(args: &[String]) -> Result<Command, UsageError> {
    let mut host = HostChoice::Native;
    let mut audio_in = false;
    let mut port: u16 = 0;
    let mut bind: IpAddr = IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    let mut file: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if flag_matches(a, "--host") {
            host = parse_host(&take_value(args, &mut i, "--host")?)?;
        } else if a == "--audio-in" {
            audio_in = true;
        } else if flag_matches(a, "--port") {
            let v = take_value(args, &mut i, "--port")?;
            port = v
                .parse()
                .map_err(|_| usage(format!("`--port` must be 0-65535, got `{v}`")))?;
        } else if flag_matches(a, "--bind") {
            let v = take_value(args, &mut i, "--bind")?;
            let addr: IpAddr = v
                .parse()
                .map_err(|_| usage(format!("`--bind` must be an IP address, got `{v}`")))?;
            if !addr.is_loopback() {
                return Err(usage(format!("`--bind {v}` must be a loopback address")));
            }
            bind = addr;
        } else if let Some(stripped) = a.strip_prefix("--") {
            return Err(usage(format!("unknown flag `--{stripped}` for `serve`")));
        } else if file.is_none() {
            file = Some(PathBuf::from(a));
        } else {
            return Err(usage(format!("unexpected argument `{a}`")));
        }
        i += 1;
    }
    Ok(Command::Serve {
        file,
        host: with_audio_in(host, audio_in)?,
        port,
        bind,
    })
}

fn parse_get(args: &[String]) -> Result<Command, UsageError> {
    let mut store = StoreSpec::Git;
    let mut target = None;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if flag_matches(a, "--store") {
            store = parse_store(&take_value(args, &mut i, "--store")?)?;
        } else if let Some(stripped) = a.strip_prefix("--") {
            return Err(usage(format!("unknown flag `--{stripped}` for `get`")));
        } else if target.is_none() {
            target = Some(parse_target(a)?);
        } else {
            return Err(usage(format!("unexpected argument `{a}`")));
        }
        i += 1;
    }
    Ok(Command::Get { target, store })
}

fn parse_lsp(args: &[String]) -> Result<Command, UsageError> {
    let mut session = None;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if flag_matches(a, "--session") {
            session = Some(take_value(args, &mut i, "--session")?);
        } else {
            return Err(usage(format!("unknown flag `{a}` for `lsp`")));
        }
        i += 1;
    }
    Ok(Command::Lsp { session })
}

/// Parses the arguments after the program name (`argv[1..]`, already
/// lossily converted to UTF-8).
///
/// # Errors
/// `UsageError` for an unknown verb or flag, or a missing or invalid
/// argument.
pub fn parse(args: &[String]) -> Result<Command, UsageError> {
    let Some(verb) = args.first() else {
        return Ok(Command::Help);
    };
    match verb.as_str() {
        "--version" | "version" => Ok(Command::Version),
        "--help" | "help" => Ok(Command::Help),
        "repl" => parse_repl(&args[1..]),
        "run" => parse_run(&args[1..]),
        "serve" => parse_serve(&args[1..]),
        "get" => parse_get(&args[1..]),
        "lsp" => parse_lsp(&args[1..]),
        other => Err(usage(format!("unknown verb `{other}`"))),
    }
}
