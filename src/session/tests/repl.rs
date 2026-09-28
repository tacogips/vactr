//! The REPL (design 14.2, 14.5.10; TASK-009 criteria 8 and 9): console
//! registers count completed forms only, the session survives every
//! failure class, and the AUDIBLE-GATE PROXY: a pattern bound from
//! `run_repl` reaches the recording audio host as committed events.

use super::support::Rig;
use crate::session::protocol::ServerMsg;
use crate::session::repl::{run_repl, LineBuffer};
use crate::value::value::Value;

/// Runs `input` through `run_repl` with a clock that advances 10 ms per
/// read; returns the transcript.
fn repl(rig: &mut Rig, input: &[u8]) -> String {
    let clock = rig.clock.clone();
    let mut tick = || {
        clock.advance(0.01);
        clock.now()
    };
    let mut out = Vec::new();
    run_repl(&mut rig.s, input, &mut out, &mut tick).expect("repl");
    String::from_utf8(out).expect("utf8 transcript")
}

#[test]
fn a_failed_expression_binds_no_register() {
    let mut rig = Rig::new();
    let t = repl(&mut rig, b"/ 1 0\n+ 1 2\n* _1 10\n");
    assert!(t.contains("error[division-by-zero]"), "{t}");
    assert!(t.contains("_1 = 3"), "{t}");
    assert!(t.contains("_2 = 30"), "{t}");
    assert!(!t.contains("_3"), "{t}");
    assert_eq!(rig.s.registers(), 2);
    let ns = rig.s.evaluator().ns();
    assert!(matches!(ns.console_register(1), Some(Value::Int(3))));
    assert!(ns.console_register(3).is_none());
}

#[test]
fn continuation_lines_form_one_entry() {
    let mut b = LineBuffer::default();
    assert!(b.feed("let a 1").len() == 1);
    assert!(b.feed("fn f x:").is_empty());
    assert!(b.is_open());
    assert!(b.feed("\t+ x 1").is_empty());
    assert!(b.feed("> id").is_empty());
    let done = b.feed("");
    assert_eq!(done, vec!["fn f x:\n\t+ x 1\n> id".to_string()]);
    assert!(b.feed("fn g x:").is_empty());
    assert!(b.feed("  x").is_empty());
    // A new unindented statement submits the block, then itself.
    assert_eq!(
        b.feed("g 2"),
        vec!["fn g x:\n  x".to_string(), "g 2".to_string()]
    );
    assert!(b.feed("fn h:").is_empty());
    assert_eq!(b.finish().as_deref(), Some("fn h:"));
    let mut rig = Rig::new();
    let t = repl(&mut rig, b"fn f x:\n\t+ x 1\n\nf 41\n");
    assert!(t.contains("= 42"), "{t}");
}

#[test]
fn the_session_survives_every_failure_class() {
    let mut rig = Rig::new();
    let input: &[u8] = b"let x [1 2\n\
+ 1 \"a\"\n\
/ 1 0\n\
s :no-such-sound > d1\n\
import github.com/nobody/vactr-nothing\n\
\xff\xfe garbage \x00\n\
{{{{\n\
+ 2 2\n";
    let t = repl(&mut rig, input);
    assert!(t.contains("error["), "{t}");
    assert!(t.contains("package-not-locked"), "{t}");
    let four = t.lines().any(|l| l.ends_with("= 4"));
    assert!(four, "the session still evaluates: {t}");
    // Protocol garbage and malformed input through the codec.
    for junk in [
        "\u{0}",
        "{\"v\":1",
        "[[[[[[[[",
        "{\"v\":1,\"seq\":1,\"kind\":\"eval\",\"body\":{\"file\":1}}",
    ] {
        let out = rig.s.apply_text(0, junk);
        assert!(matches!(out[0].env.body, ServerMsg::ProtocolError(_)));
    }
    let t = repl(&mut rig, b"+ 3 4\n");
    assert!(t.lines().any(|l| l.ends_with("= 7")), "{t}");
}

#[test]
fn audible_gate_proxy_a_repl_bound_pattern_reaches_the_audio_host() {
    let mut rig = Rig::new();
    let t = repl(&mut rig, b"s :analog > note [:a4] > d1\n");
    assert!(!t.contains("error["), "{t}");
    // Keep ticking as the CLI's evaluator thread would.
    rig.run_to(4.5);
    let sent = rig.sent();
    assert!(sent.len() >= 2, "committed events: {}", sent.len());
    let times: Vec<f64> = sent.iter().map(|e| e.time).collect();
    assert!(times.windows(2).all(|w| w[0] < w[1]), "{times:?}");
}
