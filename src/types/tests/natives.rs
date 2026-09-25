//! The native signature table (design 7.1.3; ME-MASKS required tests).

use crate::types::masks::MaskEntry;
use crate::types::natives::{HostCap, NativeKind, NativeMask, NativeTable};
use crate::types::ty::{Scheme, Ty};

fn table() -> &'static NativeTable {
    NativeTable::global()
}

fn mask(name: &str) -> Vec<MaskEntry> {
    let (_, sig) = table()
        .get(name)
        .unwrap_or_else(|| panic!("{name} missing"));
    sig.forcing_mask().0.into_vec()
}

#[test]
fn names_are_unique_and_ids_dense() {
    let t = table();
    assert!(t.len() >= 200, "table has {} entries", t.len());
    assert_eq!(t.len(), t.name_count(), "a name appears twice");
    for (k, (id, sig)) in t.iter().enumerate() {
        assert_eq!(usize::try_from(id.get()).ok(), Some(k));
        assert_eq!(t.get(sig.name).map(|(i, _)| i), Some(id));
        assert_eq!(t.sig(id).map(|s| s.name), Some(sig.name));
    }
}

#[test]
fn arity_mask_and_schemes_agree() {
    for (_, sig) in table().iter() {
        let n = sig.name;
        let schemes: Vec<Scheme> = sig
            .ty
            .iter()
            .map(|t| Scheme::parse(t).unwrap_or_else(|| panic!("{n}: bad scheme {t:?}")))
            .collect();
        assert!(!schemes.is_empty(), "{n}: no scheme");
        assert_eq!(sig.schemes(), schemes, "{n}");
        match sig.kind {
            NativeKind::Value => {
                assert!(sig.mask.is_empty() && sig.max_args == Some(0), "{n}");
                assert!(schemes.iter().all(|s| !s.ty.is_fn()), "{n}: value typed fn");
            }
            NativeKind::Function => {
                match sig.max_args {
                    Some(max) => {
                        assert!(sig.min_args <= max, "{n}: min > max");
                        assert_eq!(sig.mask.len(), usize::from(max), "{n}: mask length");
                    }
                    None => assert!(!sig.mask.is_empty(), "{n}: variadic without a rest entry"),
                }
                for s in &schemes {
                    let Ty::Fn(params, _) = &s.ty else {
                        panic!("{n}: function typed {}", s.ty);
                    };
                    if sig.is_overloaded() {
                        // One scheme per subject type; each within the arity.
                        let within = usize::from(sig.min_args)..=sig.mask.len();
                        assert!(within.contains(&params.len()), "{n}: {}", s.ty);
                    } else {
                        assert_eq!(params.len(), sig.mask.len(), "{n}: {}", s.ty);
                    }
                }
            }
        }
    }
}

#[test]
fn only_scale_and_shape_are_overloaded() {
    let overloaded: Vec<&str> = table()
        .iter()
        .filter(|(_, s)| s.is_overloaded())
        .map(|(_, s)| s.name)
        .collect();
    // `scope`, `spectrum` and `render` are overloaded on a bus source versus a
    // sample value (design 12.3 self-analysis amendment, 2026-09-25).
    assert_eq!(
        overloaded,
        ["shape", "scale", "scope", "spectrum", "render"]
    );
    for name in ["scale", "shape"] {
        let (_, sig) = table().get(name).expect("present");
        assert_eq!(sig.schemes().len(), 2, "{name}");
    }
    // Subject-type order (7.1.4): shape = number then pattern, scale =
    // pattern then texture.
    let first_param = |name: &str, k: usize| {
        let (_, sig) = table().get(name).expect("present");
        match &sig.schemes()[k].ty {
            Ty::Fn(p, _) => p[0].clone(),
            t => panic!("{t}"),
        }
    };
    assert_eq!(first_param("shape", 0), Ty::Float);
    assert!(matches!(first_param("shape", 1), Ty::Pattern(_)));
    assert!(matches!(first_param("scale", 0), Ty::Pattern(_)));
    assert_eq!(first_param("scale", 1), Ty::Tex);
}

#[test]
fn sound_entries() {
    for name in ["s", "sound"] {
        let (_, sig) = table().get(name).expect("present");
        assert_eq!(sig.keywords, ["kit"], "{name}");
        assert_eq!((sig.min_args, sig.max_args), (1, Some(1)), "{name}");
    }
    let ty = |name: &str| {
        let (_, sig) = table().get(name).expect("present");
        sig.schemes()[0].ty.to_string()
    };
    assert_eq!(ty("sample"), "fn path -> sound");
    assert_eq!(ty("midi"), "fn int -> sound");
    assert_eq!(ty("load"), "fn path -> 't0");
    assert_eq!(ty("default-sound-kit"), "[keyword: sound]");
    assert_eq!(ty("sound-kit"), "[keyword: sound]");
    for name in ["default-sound-kit", "sound-kit"] {
        assert_eq!(
            table().get(name).map(|(_, s)| s.kind),
            Some(NativeKind::Value)
        );
    }
    let (_, load) = table().get("load").expect("load");
    assert!(load.effectful);
    let (_, midi) = table().get("midi").expect("midi");
    assert_eq!(midi.needs, [HostCap::MidiOut]);
    let (_, notes) = table().get("midi-notes").expect("midi-notes");
    assert_eq!(
        (notes.keywords, notes.needs),
        (&["channel"][..], &[HostCap::MidiIn][..])
    );
}

#[test]
fn required_names_present_and_out_of_scope_absent() {
    for name in [
        "load",
        "sample",
        "midi",
        "default-sound-kit",
        "sound-kit",
        "grid",
        "range",
        "gain",
        // TASK-008 vocabulary (BE-INST, design 12.8.6).
        "bus",
        "master",
        "granular",
        "granulate",
        "sin-osc",
        "env-perc",
        "compressor",
        "vco",
    ] {
        assert!(table().get(name).is_some(), "{name} missing");
    }
    // `struct` is never a pattern function (Q3); `inst` is a special form and
    // templates such as `sampler` are sounds, not natives.
    for name in ["struct", "inst", "look", "sampler"] {
        assert!(table().get(name).is_none(), "{name} must be absent");
    }
    // `range` is subject first (M2): `range sine 200 2000`.
    let (_, range) = table().get("range").expect("range");
    assert_eq!(
        range.mask,
        [NativeMask::Value, NativeMask::Late, NativeMask::Late]
    );
}

#[test]
fn deferred_domain_parameters_are_late() {
    use MaskEntry::{Fn as F, Late as L, Value as V};
    // design-implementation.md 5.5 (line ~380): `osc {* 20 {sin time}}`
    // keeps its thunk alive per frame.
    let osc = mask("osc");
    assert_eq!(osc.len(), 3);
    assert!(osc.iter().all(|m| *m == L), "osc is all Late: {osc:?}");
    for name in ["noise", "voronoi", "gradient", "solid"] {
        assert!(mask(name).iter().all(|m| *m == L), "{name}");
    }
    assert_eq!(mask("rotate"), [V, L, L]);
    assert_eq!(mask("fast"), [V, L]);
    assert_eq!(mask("maybe"), [V, L]);
    assert_eq!(mask("degrade-by"), [V, L]);
    assert_eq!(mask("range"), [V, L, L]);
    assert_eq!(mask("every"), [V, L, F]);
    assert_eq!(mask("at"), [V, F]);
    assert_eq!(mask("map"), [V, F]);
    assert_eq!(mask("+"), [V, V]);
    // Every visual transform's numeric parameters are Late.
    for name in [
        "kaleid", "pixelate", "tile", "scroll", "color", "hue", "modulate", "scale",
    ] {
        let m = mask(name);
        assert!(m[1..].iter().any(|e| *e == L), "{name}: {m:?}");
    }
}

/// The in-scope names of the spec vocabulary tables, copied verbatim.
#[test]
fn listed_vocabulary_is_present() {
    // lang-reference.md section 5, lines 859-862 (`bus` and `master` are
    // TASK-008's and excluded).
    let core = "put join len first last tail reverse sort map filter reduce find any all take \
        take-while drop enumerate repeat dict is-nil is-list int int64 float round neg mod sin \
        cos min max abs print use-bpm use-cycle use-clock midi-clock-out once at stop hush \
        d1 d2 d3 d4 d5 d6 d7 d8 d9 slot";
    // design-music.md section 7, line 415 (patterns) and line 416 (signals);
    // from line 417 (sound) only `midi` is in scope (the rest is TASK-008,
    // and `osc` is the visual source).
    let patterns = "s sound n note gain pan speed lpf hpf room size delay fast slow rev every \
        whenmod sometimes rarely often alt maybe euclid hold repeat choose stack cat fastcat \
        superimpose off jux iter chop striate slice splice begin end loop-at fit cut ply chunk \
        hurry segment range scale chord voicing arp grid";
    let signals = "sine saw tri square rand irand perlin time beat phase cycle fft amp cc \
        midi-notes lag map-range midi";
    // design-music.md lines 150-153: the controls on any pattern.
    let controls = "gain pan speed lpf hpf resonance room size delay delaytime delayfeedback \
        crush shape vowel legato attack release sustain begin end cut orbit velocity";
    // design-visual.md section 2, line 78; the `modulate` family and
    // `tile-x`/`tile-y` from section 1 (lines 25-29), outputs `o0`..`o3`.
    let visuals = "osc noise voronoi shape gradient solid src text rotate scale pixelate tile \
        kaleid scroll posterize shift invert contrast brightness luma thresh color saturate hue \
        colorama add sub layer blend mult diff mask modulate modulate-tile modulate-kaleid \
        modulate-scroll modulate-rotate modulate-scale modulate-pixelate out render use-fps \
        use-canvas tile-x tile-y o0 o1 o2 o3";
    // The expander's `Builtin` heads (6.5.5) and the operators.
    let builtins = "or neg concat map + - * / = < > <= >= ..";
    let all = [core, patterns, signals, controls, visuals, builtins];
    let missing: Vec<&str> = all
        .iter()
        .flat_map(|list| list.split_whitespace())
        .filter(|name| table().get(name).is_none())
        .collect();
    assert!(missing.is_empty(), "missing: {missing:?}");
}

#[test]
fn effects_and_capabilities() {
    for name in [
        "print", "d1", "d9", "slot", "once", "at", "hush", "stop", "use-bpm", "out",
    ] {
        assert!(
            table().get(name).is_some_and(|(_, s)| s.effectful),
            "{name}"
        );
    }
    for name in ["s", "fast", "osc", "sample", "gain", "+"] {
        assert!(
            table().get(name).is_some_and(|(_, s)| !s.effectful),
            "{name}"
        );
    }
    let (_, fft) = table().get("fft").expect("fft");
    assert_eq!(fft.needs, [HostCap::Analysis]);
    let (_, once) = table().get("once").expect("once");
    assert!(once.keywords.contains(&"at") && once.keywords.contains(&"gain"));
}

#[test]
fn entry_at_rest_convention() {
    let (_, plus) = table().get("+").expect("+");
    assert_eq!(plus.entry_at(7), Some(NativeMask::Value));
    let (_, at) = table().get("at").expect("at");
    assert_eq!(at.entry_at(1), Some(NativeMask::Fn));
    assert_eq!(at.entry_at(2), None);
}
