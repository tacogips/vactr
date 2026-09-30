//! Argument parsing (design 14.5.10, `command.md`): every verb and flag,
//! and every usage-error path.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;

use crate::cli::args::{parse, Command, FmtInput, HostChoice, StoreSpec};
use crate::ns::pkg::PackageId;
use crate::pkg::semver::Version;

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(ToString::to_string).collect()
}

#[test]
fn no_arguments_is_help() {
    assert_eq!(parse(&args(&[])), Ok(Command::Help));
}

#[test]
fn help_and_version_forms() {
    assert_eq!(parse(&args(&["help"])), Ok(Command::Help));
    assert_eq!(parse(&args(&["--help"])), Ok(Command::Help));
    assert_eq!(parse(&args(&["version"])), Ok(Command::Version));
    assert_eq!(parse(&args(&["--version"])), Ok(Command::Version));
}

#[test]
fn unknown_verb_is_a_usage_error() {
    assert!(parse(&args(&["frobnicate"])).is_err());
}

#[test]
fn repl_parses_with_and_without_host() {
    assert_eq!(
        parse(&args(&["repl"])),
        Ok(Command::Repl {
            host: HostChoice::Native
        })
    );
    assert_eq!(
        parse(&args(&["repl", "--host", "noop"])),
        Ok(Command::Repl {
            host: HostChoice::Noop
        })
    );
    assert_eq!(
        parse(&args(&["repl", "--host=noop"])),
        Ok(Command::Repl {
            host: HostChoice::Noop
        })
    );
}

#[test]
fn audio_input_is_opt_in_and_requires_a_native_host() {
    assert_eq!(
        parse(&args(&["repl", "--audio-in"])),
        Ok(Command::Repl {
            host: HostChoice::NativeInput
        })
    );
    assert_eq!(
        parse(&args(&["run", "a.vact", "--audio-in"])),
        Ok(Command::Run {
            file: PathBuf::from("a.vact"),
            host: HostChoice::NativeInput,
            cycles: None
        })
    );
    assert_eq!(
        parse(&args(&["serve", "--audio-in"])),
        Ok(Command::Serve {
            file: None,
            host: HostChoice::NativeInput,
            port: 0,
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST)
        })
    );
    for verb in ["repl", "run", "serve"] {
        let mut invocation = vec![verb];
        if verb == "run" {
            invocation.push("a.vact");
        }
        invocation.extend(["--host", "noop", "--audio-in"]);
        assert!(parse(&args(&invocation)).is_err(), "{verb}");
    }
}

#[test]
fn repl_rejects_an_unknown_flag() {
    assert!(parse(&args(&["repl", "--bogus"])).is_err());
}

#[test]
fn run_parses_file_host_and_cycles() {
    assert_eq!(
        parse(&args(&["run", "a.vact"])),
        Ok(Command::Run {
            file: PathBuf::from("a.vact"),
            host: HostChoice::Native,
            cycles: None,
        })
    );
    assert_eq!(
        parse(&args(&["run", "a.vact", "--host", "noop", "--cycles", "2"])),
        Ok(Command::Run {
            file: PathBuf::from("a.vact"),
            host: HostChoice::Noop,
            cycles: Some(2),
        })
    );
    // Flags may precede the positional file.
    assert_eq!(
        parse(&args(&["run", "--cycles", "3", "a.vact"])),
        Ok(Command::Run {
            file: PathBuf::from("a.vact"),
            host: HostChoice::Native,
            cycles: Some(3),
        })
    );
}

#[test]
fn run_without_a_file_is_a_usage_error() {
    assert!(parse(&args(&["run"])).is_err());
    assert!(parse(&args(&["run", "--host", "noop"])).is_err());
}

#[test]
fn run_rejects_a_zero_or_non_numeric_cycles() {
    assert!(parse(&args(&["run", "a.vact", "--cycles", "0"])).is_err());
    assert!(parse(&args(&["run", "a.vact", "--cycles", "x"])).is_err());
}

#[test]
fn serve_parses_file_host_port_and_bind() {
    assert_eq!(
        parse(&args(&["serve"])),
        Ok(Command::Serve {
            file: None,
            host: HostChoice::Native,
            port: 0,
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
        })
    );
    assert_eq!(
        parse(&args(&[
            "serve",
            "a.vact",
            "--host",
            "noop",
            "--port",
            "9000",
            "--bind",
            "127.0.0.1"
        ])),
        Ok(Command::Serve {
            file: Some(PathBuf::from("a.vact")),
            host: HostChoice::Noop,
            port: 9000,
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
        })
    );
}

#[test]
fn serve_rejects_a_non_loopback_bind() {
    assert!(parse(&args(&["serve", "--bind", "0.0.0.0"])).is_err());
    assert!(parse(&args(&["serve", "--bind", "10.0.0.1"])).is_err());
}

#[test]
fn serve_accepts_ipv6_loopback() {
    assert_eq!(
        parse(&args(&["serve", "--bind", "::1"])),
        Ok(Command::Serve {
            file: None,
            host: HostChoice::Native,
            port: 0,
            bind: IpAddr::V6(std::net::Ipv6Addr::LOCALHOST),
        })
    );
}

#[test]
fn get_parses_a_bare_path_a_versioned_path_and_the_store_flag() {
    assert_eq!(
        parse(&args(&["get"])),
        Ok(Command::Get {
            target: None,
            store: StoreSpec::Git,
        })
    );
    assert_eq!(
        parse(&args(&["get", "github.com/test/vactr-pads"])),
        Ok(Command::Get {
            target: Some((PackageId::new("github.com/test/vactr-pads"), None)),
            store: StoreSpec::Git,
        })
    );
    assert_eq!(
        parse(&args(&[
            "get",
            "github.com/test/vactr-pads@v1.0.0",
            "--store",
            "dir:/tmp/store",
        ])),
        Ok(Command::Get {
            target: Some((
                PackageId::new("github.com/test/vactr-pads"),
                Some(Version::new(1, 0, 0)),
            )),
            store: StoreSpec::Dir(PathBuf::from("/tmp/store")),
        })
    );
}

#[test]
fn get_rejects_an_invalid_path_version_or_store() {
    assert!(parse(&args(&["get", "not-a-path"])).is_err());
    assert!(parse(&args(&["get", "github.com/test/vactr-pads@bogus"])).is_err());
    assert!(parse(&args(&["get", "--store", "http:nope"])).is_err());
    assert!(parse(&args(&["get", "--store", "dir:"])).is_err());
}

#[test]
fn lsp_parses_with_and_without_session() {
    assert_eq!(parse(&args(&["lsp"])), Ok(Command::Lsp { session: None }));
    assert_eq!(
        parse(&args(&[
            "lsp",
            "--session",
            "ws://127.0.0.1:1/session?token=x"
        ])),
        Ok(Command::Lsp {
            session: Some("ws://127.0.0.1:1/session?token=x".to_string())
        })
    );
}

#[test]
fn fmt_parses_paths_and_check_in_any_position() {
    assert_eq!(
        parse(&args(&["fmt", "a.vact"])),
        Ok(Command::Fmt {
            check: false,
            input: FmtInput::Paths(vec![PathBuf::from("a.vact")]),
        })
    );
    for invocation in [
        args(&["fmt", "--check", "a", "b"]),
        args(&["fmt", "a", "--check", "b"]),
    ] {
        assert_eq!(
            parse(&invocation),
            Ok(Command::Fmt {
                check: true,
                input: FmtInput::Paths(vec![PathBuf::from("a"), PathBuf::from("b")]),
            })
        );
    }
}

#[test]
fn fmt_parses_stdin_and_rejects_invalid_inputs() {
    assert_eq!(
        parse(&args(&["fmt", "-"])),
        Ok(Command::Fmt {
            check: false,
            input: FmtInput::Stdin,
        })
    );
    assert_eq!(
        parse(&args(&["fmt", "--check", "-"])),
        Ok(Command::Fmt {
            check: true,
            input: FmtInput::Stdin,
        })
    );
    for invocation in [
        args(&["fmt"]),
        args(&["fmt", "-", "a"]),
        args(&["fmt", "-", "-"]),
        args(&["fmt", "--bogus", "a"]),
        args(&["fmt", "--check", "--check", "a"]),
    ] {
        assert!(parse(&invocation).is_err(), "{invocation:?}");
    }
}
