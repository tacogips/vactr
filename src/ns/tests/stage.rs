//! Staged effects (design 7.1.3): released only when the whole form
//! succeeded; a bind-then-fail form releases nothing.

use crate::ns::stage::{EffectBuffer, RecordingSink, SlotKey, StagedEffect};
use crate::value::value::Value;
use crate::vm::tests::Sess;

#[test]
fn effects_are_released_on_success() {
    let mut s = Sess::new();
    assert_eq!(s.show("d1 5"), "5");
    assert!(matches!(
        s.sink.effects.as_slice(),
        [StagedEffect::SlotBind {
            slot: SlotKey::D(1),
            value: Value::Int(5)
        }]
    ));
    assert!(s.vm.effects().is_empty());
}

#[test]
fn a_bind_then_fail_form_releases_nothing() {
    let mut s = Sess::new();
    s.eval("fn bad k:\n\td1 k\n\tprint \"x\"\n\t/ 1 0")
        .expect("define");
    assert_eq!(s.show("bad 5"), "fail: division-by-zero");
    assert!(s.sink.effects.is_empty(), "{:?}", s.sink.effects);
    assert!(s.vm.effects().is_empty());
    // The next form's effects are unaffected.
    s.eval("d2 1").expect("d2");
    assert_eq!(s.sink.effects.len(), 1);
}

#[test]
fn a_failed_run_drops_only_its_own_effects() {
    let mut s = Sess::new();
    s.eval("fn bad k:\n\td1 k\n\t/ 1 0").expect("define");
    // Stage without releasing, then a failing run: the earlier stays.
    let (ns, vm) = (&s.ns, &mut s.vm);
    let ok = crate::vm::tests::compile_src(ns, "d3 1");
    vm.run(ok, ns).expect("stages d3");
    let bad = crate::vm::tests::compile_src(ns, "bad 5");
    vm.run(bad, ns).expect_err("fails");
    assert_eq!(vm.effects().len(), 1);
}

#[test]
fn slot_effects_stage_their_kinds() {
    let mut s = Sess::new();
    s.eval("slot :drums 1\nstop :d2\nstop :drums\nhush\nonce 1 at: 4 gain: 0.5\nuse-bpm 120\nprint 1 2")
        .expect("effects");
    let kinds: Vec<String> = s
        .sink
        .effects
        .iter()
        .map(|e| match e {
            StagedEffect::SlotBind { slot, .. } => format!("bind {}", slot.name()),
            StagedEffect::Revoke(k) => format!("revoke {}", k.name()),
            StagedEffect::StopAll => "stop-all".into(),
            StagedEffect::Cut => "cut".into(),
            StagedEffect::OneShot { at, overrides, .. } => {
                format!("once {:?} {}", at.map(|r| r.to_string()), overrides.len())
            }
            StagedEffect::Tempo(_) => "tempo".into(),
            StagedEffect::Console(c) => format!("print {c}"),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        [
            "bind drums",
            "revoke d2",
            "revoke drums",
            "cut",
            "once Some(\"4\") 1",
            "tempo",
            "print 1 2"
        ]
    );
}

#[test]
fn a_buffer_releases_in_order_and_drops_on_request() {
    let mut b = EffectBuffer::default();
    b.push(StagedEffect::Console("a".into()));
    b.push(StagedEffect::Console("b".into()));
    let mut sink = RecordingSink::default();
    b.release(&mut sink);
    assert_eq!(sink.console(), ["a", "b"]);
    assert!(b.is_empty());
    b.push(StagedEffect::Console("c".into()));
    b.drop_all();
    assert!(b.is_empty());
}
