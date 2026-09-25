//! INTEGRATE: native-table completeness (design 7.1.3). Every `NativeTable`
//! entry has exactly one registered implementation (a prelude value for a
//! value entry) with the same arity, keywords and mask, and every
//! registered native and prelude name has an entry.

use crate::ns::evaluator::Evaluator;
use crate::ns::load::NoopHost;
use crate::ns::namespace::{Prelude, SlotKind};
use crate::ns::stage::RecordingSink;
use crate::types::natives::{NativeKind, NativeTable};
use crate::value::intern::{intern_sym, name_of_sym};
use crate::value::value::Value;
use crate::vm::natives::full_prelude;

/// The prelude an `Evaluator` runs with.
fn evaluator_prelude() -> Evaluator {
    Evaluator::new(
        Prelude::core(),
        Box::new(NoopHost),
        Box::new(RecordingSink::default()),
    )
}

#[test]
fn every_table_entry_has_one_matching_registration() {
    let ev = evaluator_prelude();
    let p = ev.ns().prelude();
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let table = NativeTable::global();
    assert_eq!(table.len(), table.name_count(), "names are unique");
    for (id, sig) in table.iter() {
        let slot = p
            .slot(intern_sym(sig.name))
            .unwrap_or_else(|| panic!("`{}` is not in the prelude", sig.name));
        assert_eq!(slot.kind(), SlotKind::Prelude);
        match sig.kind {
            NativeKind::Function => {
                let entry = p
                    .native(id)
                    .unwrap_or_else(|| panic!("`{}` has no implementation", sig.name));
                assert_eq!(entry.sig, *sig, "{}", sig.name);
                assert_eq!(entry.sig.min_args, sig.min_args, "{}", sig.name);
                assert_eq!(entry.sig.max_args, sig.max_args, "{}", sig.name);
                assert_eq!(entry.sig.keywords, sig.keywords, "{}", sig.name);
                assert_eq!(entry.sig.forcing_mask(), sig.forcing_mask(), "{}", sig.name);
                assert!(
                    matches!(slot.get(), Value::Native(n) if n == id),
                    "`{}` binds its own native",
                    sig.name
                );
            }
            NativeKind::Value => {
                assert!(p.native(id).is_none(), "`{}` is a value", sig.name);
                assert!(
                    !matches!(slot.get(), Value::Native(_) | Value::Nil),
                    "`{}` has a prelude value",
                    sig.name
                );
            }
        }
    }
}

#[test]
fn every_registration_has_a_table_entry() {
    let ev = evaluator_prelude();
    let p = ev.ns().prelude();
    let table = NativeTable::global();
    let mut natives = 0;
    for (id, entry) in p.natives() {
        let sig = table
            .sig(id)
            .unwrap_or_else(|| panic!("`{}` has no table entry", entry.sig.name));
        assert_eq!(sig.name, entry.sig.name);
        natives += 1;
    }
    let functions = table
        .iter()
        .filter(|(_, s)| s.kind == NativeKind::Function)
        .count();
    assert_eq!(natives, functions);
    for name in p.names() {
        let name = name_of_sym(name);
        assert!(table.get(&name).is_some(), "`{name}` has no table entry");
    }
    assert_eq!(p.names().count(), table.len());
}

#[test]
fn the_full_prelude_is_the_table_minus_load() {
    let p = full_prelude();
    assert!(p.errors().is_empty(), "{:?}", p.errors());
    let table = NativeTable::global();
    assert_eq!(p.names().count(), table.len() - 1);
    assert!(p.slot(intern_sym("load")).is_none());
    // Registering the domain again is a no-op, not a duplicate.
    let mut again = full_prelude();
    crate::vm::natives::register_domain(&mut again);
    assert!(again.errors().is_empty(), "{:?}", again.errors());
}

#[test]
fn the_prelude_sound_kits_are_one_builtin_sound_per_manifest_key() {
    let ev = evaluator_prelude();
    let p = ev.ns().prelude();
    let default = p.slot(intern_sym("default-sound-kit")).expect("kit").get();
    let current = p.slot(intern_sym("sound-kit")).expect("kit").get();
    assert!(crate::value::eq::deep_eq(&default, &current).unwrap_or(false));
    let Value::Dict(d) = default else {
        panic!("a dict");
    };
    let keys = crate::types::manifest::HostManifest::spec_default().sound_kit_keys();
    let crate::types::ty::KeySet::Of(keys) = keys else {
        panic!("closed");
    };
    assert_eq!(d.len(), keys.len());
    for (k, v) in d.iter() {
        let crate::value::key::Key::Kw(kw) = k else {
            panic!("keyword keys");
        };
        assert!(keys.contains(&*crate::value::intern::name_of_kw(*kw)));
        assert!(matches!(
            v,
            Value::Sound(s) if **s == crate::value::value::Sound::Builtin(*kw)
        ));
    }
}
