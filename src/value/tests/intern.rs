use crate::value::{intern_kw, intern_sym, name_of_kw, name_of_sym, Interner, KwId};

#[test]
fn round_trip_and_one_id_per_text() {
    let mut i = Interner::new();
    let a = i.intern("kick");
    let b = i.intern("snare");
    assert_ne!(a, b);
    assert_eq!(i.intern("kick"), a);
    assert_eq!(i.resolve(a).as_deref(), Some("kick"));
    assert_eq!(i.resolve(b).as_deref(), Some("snare"));
    assert_eq!(i.resolve(999), None);
}

#[test]
fn thread_local_round_trip() {
    let k = intern_kw("hat");
    assert_eq!(intern_kw("hat"), k);
    assert_eq!(&*name_of_kw(k), "hat");
    let s = intern_sym("voice");
    assert_eq!(intern_sym("voice"), s);
    assert_eq!(&*name_of_sym(s), "voice");
}

#[test]
fn unknown_id_is_empty_not_panic() {
    assert_eq!(&*name_of_kw(KwId::new(u32::MAX)), "");
}
