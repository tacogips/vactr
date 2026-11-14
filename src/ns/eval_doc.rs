//! Document evaluation entry points for the session (design 14.5.4).
//!
//! `Evaluator::eval_form` checks every form against the spec-default host
//! manifest. The session checks against ITS manifest (the spec default plus
//! registered package banks), so `eval_form_in` swaps the check
//! diagnostics: `eval_form` puts its own check diagnostics first, and the
//! check is a pure function of the namespace state before the form runs, so
//! the leading ones are exactly the spec-default check of the same form.
//! Both manifest checks use the evaluator's bounded environment selection.
//!
//! The document revision of a form is NOT stamped here: `attempt` builds
//! its `CompileCx` privately (with revision 0). The session records the
//! revision of every form it evaluates and stamps it into the `SrcRef`s it
//! publishes (`session/publish.rs`).

use std::collections::BTreeSet;
use std::rc::Rc;

use crate::ns::evaluator::{DynamicManifestCacheEntry, Evaluator, FormOutcome};
use crate::reader::node::Node;
use crate::types::check::check;
use crate::types::manifest::HostManifest;

impl Evaluator {
    /// Installed pattern-control names, shared across compiled forms until the
    /// registry's declared names change.
    pub(super) fn dynamic_custom_controls(&self) -> Rc<BTreeSet<Rc<str>>> {
        self.insts().map_or_else(
            || Rc::new(BTreeSet::new()),
            |insts| insts.borrow().declared_names_rc(),
        )
    }

    /// Add installed instrument header names to the checker manifest.
    #[must_use]
    pub fn dynamic_manifest(&self, base: &HostManifest) -> Rc<HostManifest> {
        let Some(insts) = self.insts() else {
            return Rc::new(base.clone());
        };
        let controls = insts.borrow().declared_names_rc();
        if controls.is_empty() {
            return Rc::new(base.clone());
        }
        if let Some(manifest) = self
            .dynamic_manifest_cache
            .borrow()
            .iter()
            .find(|entry| entry.base == *base && Rc::ptr_eq(&entry.controls, &controls))
            .map(|entry| Rc::clone(&entry.manifest))
        {
            return manifest;
        }
        let manifest = Rc::new(base.with_controls(controls.iter()));
        let mut cache = self.dynamic_manifest_cache.borrow_mut();
        if cache.len() == 4 {
            cache.remove(0);
        }
        cache.push(DynamicManifestCacheEntry {
            base: base.clone(),
            controls,
            manifest: Rc::clone(&manifest),
        });
        manifest
    }

    /// `eval_form`, with the form checked against `manifest` instead of the
    /// spec default.
    pub fn eval_form_in(&mut self, form: &Node, manifest: &HostManifest) -> FormOutcome {
        let spec = self.dynamic_manifest(&HostManifest::spec_default());
        let manifest = self.dynamic_manifest(manifest);
        let env = self.check_env_for(form);
        let forms = std::slice::from_ref(form);
        let checked = check(forms, &env, &spec);
        if manifest == spec {
            return self.eval_form_checked(form, checked);
        }
        let default_len = checked.diags.len();
        let mut diags = check(forms, &env, &manifest).diags;
        let mut out = self.eval_form_checked(form, checked);
        let rest = out.diags.split_off(default_len.min(out.diags.len()));
        diags.extend(rest);
        out.diags = diags;
        out
    }
}
