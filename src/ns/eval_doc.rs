//! Document evaluation entry points for the session (design 14.5.4).
//!
//! `Evaluator::eval_form` checks every form against the spec-default host
//! manifest. The session checks against ITS manifest (the spec default plus
//! registered package banks), so `eval_form_in` swaps the check
//! diagnostics: `eval_form` puts its own check diagnostics first, and the
//! check is a pure function of the namespace state before the form runs, so
//! the leading ones are exactly the spec-default check of the same form.
//! `evaluator.rs` is not edited (it is at its line budget).
//!
//! The document revision of a form is NOT stamped here: `attempt` builds
//! its `CompileCx` privately (with revision 0). The session records the
//! revision of every form it evaluates and stamps it into the `SrcRef`s it
//! publishes (`session/publish.rs`).

use crate::ns::evaluator::{Evaluator, FormOutcome};
use crate::reader::node::Node;
use crate::types::check::check;
use crate::types::manifest::HostManifest;

impl Evaluator {
    /// Add installed instrument header names to the checker manifest.
    #[must_use]
    pub fn dynamic_manifest(&self, base: &HostManifest) -> HostManifest {
        self.insts().map_or_else(
            || base.clone(),
            |insts| base.with_controls(insts.borrow().declared_names()),
        )
    }

    /// `eval_form`, with the form checked against `manifest` instead of the
    /// spec default.
    pub fn eval_form_in(&mut self, form: &Node, manifest: &HostManifest) -> FormOutcome {
        let spec = self.dynamic_manifest(&HostManifest::spec_default());
        let manifest = self.dynamic_manifest(manifest);
        if manifest == spec {
            return self.eval_form(form);
        }
        let env = self.ns().check_env();
        let forms = std::slice::from_ref(form);
        let default_len = check(forms, &env, &spec).diags.len();
        let mut diags = check(forms, &env, &manifest).diags;
        let mut out = self.eval_form(form);
        let rest = out.diags.split_off(default_len.min(out.diags.len()));
        diags.extend(rest);
        out.diags = diags;
        out
    }
}
