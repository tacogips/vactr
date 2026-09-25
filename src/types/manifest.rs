//! The host manifest the checker reads (design 7): the builtin sound,
//! synth and control sets and the host capabilities. The prelude's
//! `default-sound-kit` is built from the sound and synth sets (7.1.4), so
//! `s :bd-haus` checks and `s :not-a-sample` is `unknown-keyword`.

use std::collections::BTreeSet;

use crate::reader::node::{Atom, Node, NodeKind};
use crate::types::check::Checker;
use crate::types::diag::DiagCode;
use crate::types::infer::ctl;
use crate::types::infer_call::{pair_key, split_args, STEP_ALL, STEP_FIRST};
use crate::types::natives::{HostCap, NativeSig};
use crate::types::natives_domain::CONTROLS;
use crate::types::scope::ScopeKind;
use crate::types::ty::{BindKind, KeySet, Ty};

/// What the host provides, as far as the checker needs it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HostManifest {
    /// Builtin sample and sound keywords.
    pub sounds: KeySet,
    /// Synth templates, playable as sounds (`s :analog`).
    pub synths: KeySet,
    /// Pattern control names.
    pub controls: KeySet,
    /// The capabilities this host has (`beyond-capability`).
    pub caps: BTreeSet<HostCap>,
}

/// Every builtin sound keyword used after `s` in the spec examples, with
/// the first document line that uses it. ME-INTEGRATE may add keys.
const SPEC_SOUNDS: [&str; 13] = [
    "bd-haus",  // design-music.md:34 `s [:bd-haus :sn-dub] > d1`
    "sn-dub",   // design-music.md:34
    "bd-tek",   // design-music.md:42 `s {alt :bd-haus :bd-tek} > d2`
    "crash",    // design-music.md:45 `s :crash > once`
    "pluck",    // design-music.md:65 `s :pluck > midi-notes channel: 1 > d1`
    "bd",       // design-music.md:102 `s :bd > n [0 3] > d1`
    "sd",       // design-music.md:119 `s [:bd :sd] kit: tr909 > d1`
    "break",    // design-music.md:130 `s :break > n 3 > d1`
    "piano",    // design-music.md:146 `s :piano > chord ...`
    "hh",       // design-music.md:186 `s [:bd :sd [:hh :hh] [:cp :cp] ...]`
    "cp",       // design-music.md:186
    "sawtooth", // design-music.md:278 `> s :sawtooth`
    "vocal",    // design-music.md:402 `s [:vocal] > bus :texture > d1`
];

/// The synthesis templates shipped in the prelude (design-music.md
/// section 4, "Templates shipped in the prelude").
const SPEC_SYNTHS: [&str; 7] = [
    "sampler",
    "analog",
    "fm",
    "pd",
    "additive",
    "wavetable",
    "granular",
];

impl HostManifest {
    /// The manifest the spec examples assume: every sound and synth keyword
    /// they use, the pattern controls (plus `n` and `note`), and every
    /// capability.
    #[must_use]
    pub fn spec_default() -> HostManifest {
        HostManifest {
            sounds: KeySet::of(SPEC_SOUNDS),
            synths: KeySet::of(SPEC_SYNTHS),
            controls: KeySet::of(CONTROLS.iter().copied().chain(["n", "note"])),
            caps: [
                HostCap::MidiIn,
                HostCap::MidiOut,
                HostCap::Analysis,
                HostCap::Render,
            ]
            .into_iter()
            .collect(),
        }
    }

    /// The keys of the prelude's `default-sound-kit`: the builtin sounds
    /// and the synth templates.
    #[must_use]
    pub fn sound_kit_keys(&self) -> KeySet {
        self.sounds.union(&self.synths)
    }

    /// True when the host has every capability in `needs`.
    #[must_use]
    pub fn has_caps(&self, needs: &[HostCap]) -> bool {
        needs.iter().all(|c| self.caps.contains(c))
    }

    /// The editor declaration of one builtin (design 13.5: `EditorDecl`/
    /// `ParamMeta` carried through `HostManifest`).
    #[must_use]
    pub fn editor_decl(&self, name: &str) -> Option<&'static crate::dsp::meta::EditorDecl> {
        crate::dsp::meta::decl_for(name)
    }

    /// Every editor declaration this host's catalog defines (design 13.5).
    #[must_use]
    pub fn editor_decls(&self) -> &'static [crate::dsp::meta::EditorDecl] {
        crate::dsp::meta::all()
    }

    /// The `inst` header parameter names of every synth template this host
    /// provides (design 12.8.6 M3, 13.5): each is a control-table row, read
    /// through the `EditorDecl`/`ParamMeta` carried here. `Open` synths
    /// have no enumerable template list, so it is empty.
    #[must_use]
    pub fn template_params(&self) -> Vec<&'static str> {
        let KeySet::Of(synths) = &self.synths else {
            return Vec::new();
        };
        synths
            .iter()
            .filter_map(|name| self.editor_decl(name))
            .flat_map(|decl| decl.params.iter().map(|p| p.name))
            .collect()
    }

    /// This manifest with `extra` added to the sound keywords: the session
    /// manifest is the spec default plus every registered package asset
    /// bank (design 14.5.7 "Assets", S3). An `Open` set stays open.
    #[must_use]
    pub fn with_sounds<I, S>(&self, extra: I) -> HostManifest
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        HostManifest {
            sounds: self.sounds.union(&KeySet::of(extra)),
            ..self.clone()
        }
    }
}

/// `s`/`sound` and the sound-kit keyword check (7.1.4).
impl Checker<'_> {
    /// `s`/`sound` (7.1.4): one positional `pattern sound` argument and an
    /// optional `kit: [keyword: sound]`; two or more positional arguments
    /// are `sound-not-first`.
    pub(crate) fn sound_call(
        &mut self,
        n: &Node,
        sig: &'static NativeSig,
        args: &[Node],
        d: u32,
    ) -> Ty {
        let a = split_args(args, true);
        let mut has_kit = false;
        for p in &a.named {
            let key = pair_key(p).unwrap_or("");
            let value = p.children.get(1);
            let vt = value.map(|v| self.infer(v, d));
            if key == "kit" {
                has_kit = true;
                if let (Some(v), Some(vt)) = (value, vt) {
                    let kit = Ty::Dict(Box::new(Ty::keyword()), Box::new(Ty::Sound));
                    self.check_arg(&kit, &vt, v, "`kit:` of `s`");
                }
            } else {
                self.emit(
                    DiagCode::TypeMismatch,
                    p.span,
                    format!(
                        "`{}` takes only the named argument `kit:`, not `{key}:`",
                        sig.name
                    ),
                );
            }
        }
        match a.positional.len() {
            0 if !a.splat => self.emit(
                DiagCode::TypeMismatch,
                n.span,
                format!("`{}` takes one sound", sig.name),
            ),
            0 => {}
            1 => {
                let check = !has_kit && self.sound_kit_is_prelude();
                self.sound_arg(a.positional[0], check, d, 0);
            }
            _ => {
                self.emit(
                    DiagCode::SoundNotFirst,
                    n.span,
                    format!(
                        "`{}` starts the chain and takes one sound, not a pattern; write `s :bd > n [0 3]`",
                        sig.name
                    ),
                );
                self.mute_types += 1;
                for p in &a.positional {
                    self.infer(p, d);
                }
                self.mute_types -= 1;
            }
        }
        ctl()
    }

    /// True when `sound-kit` at this call site is the prelude's: no session
    /// binding (in the document or before it) or open import binds it
    /// (7.1.4).
    fn sound_kit_is_prelude(&self) -> bool {
        // A local `sound-kit` inside a `fn` does not affect `s` (7.1.4).
        self.scopes.session_lookup("sound-kit").is_none()
            && self.env.global("sound-kit").is_none()
            && self.live.open_prefix("sound-kit").is_none()
    }

    /// A `sound` position: a keyword names a sound in the kit (checked
    /// when `check`), a list or step constructor holds more sound
    /// positions, anything else is a sound value.
    fn sound_arg(&mut self, n: &Node, check: bool, d: u32, depth: u32) {
        if depth > 64 {
            self.infer(n, d);
            return;
        }
        match &n.kind {
            NodeKind::Atom(Atom::Keyword(k)) => {
                let session_inst = self.env.global(k).is_some_and(|g| g.kind == BindKind::Inst);
                if check
                    && !self.kit_keys.contains(k)
                    && !self.inst_names.contains(&**k)
                    && !session_inst
                {
                    self.emit(
                        DiagCode::UnknownKeyword,
                        n.span,
                        format!("`:{k}` is not a sound in the default sound kit"),
                    );
                }
                self.types.insert(n.id, Ty::Sound);
            }
            NodeKind::Atom(Atom::Nil) => {
                self.types.insert(n.id, Ty::Nil);
            }
            NodeKind::List => {
                for c in n.children.iter() {
                    self.sound_arg(c, check, d, depth + 1);
                }
                self.types.insert(n.id, Ty::Pattern(Box::new(Ty::Sound)));
            }
            NodeKind::Block if n.children.len() == 1 => {
                let mark = self.scopes.depth();
                self.scopes.push(ScopeKind::Block);
                self.sound_arg(&n.children[0], check, d, depth + 1);
                self.scopes.truncate(mark);
                let t = self
                    .types
                    .get(&n.children[0].id)
                    .cloned()
                    .unwrap_or(Ty::Any);
                self.types.insert(n.id, t);
            }
            NodeKind::Call if self.step_ctor(n).is_some() => {
                let all = self.step_ctor(n) == Some(true);
                for (k, c) in n.children.iter().enumerate().skip(1) {
                    match c.kind {
                        NodeKind::Pair => {
                            for v in c.children.iter().skip(1) {
                                self.infer(v, d);
                            }
                        }
                        _ if all || k == 1 => self.sound_arg(c, check, d, depth + 1),
                        _ => {
                            self.infer(c, d);
                        }
                    }
                }
                self.types.insert(n.id, Ty::Pattern(Box::new(Ty::Sound)));
            }
            _ => {
                let t = self.infer(n, d);
                let want = Ty::Pattern(Box::new(Ty::Sound));
                self.check_arg(&want, &t, n, "the sound of `s`");
            }
        }
    }

    /// `Some(true)` for `alt`/`choose`, `Some(false)` for a step
    /// constructor whose first argument is the step, `None` otherwise.
    fn step_ctor(&self, n: &Node) -> Option<bool> {
        let sig = self.prelude_head(n.children.first()?)?;
        if STEP_ALL.contains(&sig.name) {
            Some(true)
        } else if STEP_FIRST.contains(&sig.name) {
            Some(false)
        } else {
            None
        }
    }
}
