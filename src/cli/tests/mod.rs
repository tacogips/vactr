//! Unit tests for the CLI's pure pieces: argument parsing (`args.rs`) and
//! the session-socket handshake (`ws.rs`). The end-to-end binary
//! behavior (`repl`, `run`, `serve`, `get`) is `tests/cli.rs`.

mod args;
#[cfg(feature = "host-native")]
mod ws;
