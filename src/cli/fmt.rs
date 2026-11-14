//! `vactr fmt [--check] <PATH>...` (command.md): format files in place,
//! report changed paths in check mode, or format stdin to stdout.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::cli::args::FmtInput;
use crate::fmt::{self, Formatted, Outcome};
use crate::types::diag::{Diagnostic, Severity};

/// Locks process stdio and runs the formatter command.
pub(crate) fn main(check: bool, input: &FmtInput, cwd: &Path) -> i32 {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    run(
        check,
        input,
        cwd,
        &mut stdin.lock(),
        &mut stdout.lock(),
        &mut stderr.lock(),
    )
}

/// Runs `fmt` with injected streams so the command's behavior is testable.
pub(crate) fn run(
    check: bool,
    input: &FmtInput,
    cwd: &Path,
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let mut io_error = false;
    let mut refused = false;
    let mut changed = false;

    match input {
        FmtInput::Stdin => {
            let mut bytes = Vec::new();
            match stdin.read_to_end(&mut bytes) {
                Ok(_) => match std::str::from_utf8(&bytes) {
                    Ok(source) => {
                        let formatted = fmt::format(source);
                        let (was_refused, stderr_error) =
                            report_refusal("-", source, &formatted, stderr);
                        refused |= was_refused;
                        io_error |= stderr_error;
                        if !check && write_all(stdout, formatted.text.as_bytes()) {
                            io_error = true;
                        }
                        changed = formatted.outcome == Outcome::Changed;
                    }
                    Err(_) => {
                        io_error = true;
                        write_error(stderr, "-", "not UTF-8");
                    }
                },
                Err(error) => {
                    io_error = true;
                    let message = error.to_string();
                    write_error(stderr, "-", &message);
                }
            }
        }
        FmtInput::Paths(paths) => {
            for path in paths {
                let full_path = resolve_path(cwd, path);
                match std::fs::read(&full_path) {
                    Ok(bytes) => match std::str::from_utf8(&bytes) {
                        Ok(source) => {
                            let formatted = fmt::format(source);
                            let (was_refused, stderr_error) = report_refusal(
                                &path.display().to_string(),
                                source,
                                &formatted,
                                stderr,
                            );
                            refused |= was_refused;
                            io_error |= stderr_error;
                            match formatted.outcome {
                                Outcome::Changed if check => {
                                    changed = true;
                                    let displayed = path.display().to_string();
                                    if write_line(stdout, &displayed) {
                                        io_error = true;
                                    }
                                }
                                Outcome::Changed => {
                                    if let Err(error) = std::fs::write(&full_path, formatted.text) {
                                        io_error = true;
                                        let message = error.to_string();
                                        write_error(stderr, &path.display().to_string(), &message);
                                    }
                                }
                                Outcome::Unchanged | Outcome::Refused => {}
                            }
                        }
                        Err(_) => {
                            io_error = true;
                            let displayed = path.display().to_string();
                            write_error(stderr, &displayed, "not UTF-8");
                        }
                    },
                    Err(error) => {
                        io_error = true;
                        let displayed = path.display().to_string();
                        let message = error.to_string();
                        write_error(stderr, &displayed, &message);
                    }
                }
            }
        }
    }

    if io_error {
        1
    } else if refused {
        3
    } else if check && changed {
        1
    } else {
        0
    }
}

fn resolve_path(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn report_refusal(
    path: &str,
    source: &str,
    formatted: &Formatted,
    stderr: &mut dyn Write,
) -> (bool, bool) {
    if formatted.outcome != Outcome::Refused {
        return (false, false);
    }
    let mut io_error = false;
    for diagnostic in &formatted.diags {
        if diagnostic.severity == Severity::Error {
            let (line, col) = line_col(source, diagnostic);
            let message = format!(
                "vactr: {path}:{line}:{col}: {}[{}] {}\n",
                diagnostic.severity.as_str(),
                diagnostic.code,
                diagnostic.message
            );
            if stderr.write_all(message.as_bytes()).is_err() {
                io_error = true;
            }
        }
    }
    (true, io_error)
}

fn line_col(source: &str, diagnostic: &Diagnostic) -> (usize, usize) {
    let offset = usize::try_from(diagnostic.span.start).unwrap_or(usize::MAX);
    let mut line = 1;
    let mut col = 1;
    for (byte, ch) in source.char_indices() {
        if byte >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn write_all(writer: &mut dyn Write, bytes: &[u8]) -> bool {
    writer.write_all(bytes).is_err()
}

fn write_line(writer: &mut dyn Write, line: &str) -> bool {
    writer.write_all(line.as_bytes()).is_err() || writer.write_all(b"\n").is_err()
}

fn write_error(writer: &mut dyn Write, path: &str, message: &str) {
    let line = format!("vactr: {path}: {message}\n");
    let _ = writer.write_all(line.as_bytes());
}
