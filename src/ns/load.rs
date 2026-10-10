//! Source loading (design 7.1.3 "Source loading", lang-reference section 4).
//!
//! The one host capability of this issue: `SourceLoader` reads the file a
//! path names and assigns its `FileId`. `NoopHost` (in `host/noop.rs`) reads
//! nothing (`host-unavailable`); the host tasks add the real loaders behind
//! the same trait. The `load` native runs the file through
//! read -> expand -> check -> compile -> run in a FRESH session-level scope
//! whose parent is the prelude (never the caller's session) and returns the
//! value of its last top-level form. The file's check diagnostics keep
//! their own spans, never fail the load, and are collected on the
//! `LoaderHost` for the evaluator to report (7.1.1, 7.1.3).

use std::rc::Rc;

use crate::compile::{compile, CompileCx};
use crate::expand::{expand, ExpandCx};
use crate::host::caps::AnalysisCx;
use crate::ns::namespace::{FormGen, Namespace, Prelude};
use crate::reader::node::Node;
use crate::reader::span::FileId;
use crate::reader::{read, AliasEnv};
use crate::types::check::check;
use crate::types::diag::{Diagnostic, Severity};
use crate::types::manifest::HostManifest;
use crate::value::intern::KwId;
use crate::value::value::{PathVal, Value};
use crate::vm::call::{kind_name, NativeCx};
use crate::vm::fail::{FailCode, Failure};
use crate::vm::vm::{EffectMode, Vm};

/// Reads source files for `load`.
pub trait SourceLoader {
    /// Configuration-only isolated song preparation capability.
    fn song_asset_factory(&self) -> Option<Rc<dyn crate::song::assets::SongAssetFactory>> {
        None
    }

    /// Resolves `path` (relative to `path.file`, 6.5.8) and returns the
    /// file's id and text.
    ///
    /// # Errors
    /// Any failure to read, `host-unavailable` when there is no host.
    fn read(&mut self, path: &PathVal) -> Result<(FileId, Rc<str>), Failure>;

    /// Reads raw bytes from `path`, bounded by `limit`. Loaders without
    /// byte-file support (including browser loaders) fail with
    /// `host-unavailable`.
    ///
    /// # Errors
    /// Any read failure, or `host-unavailable` when raw-byte loading is not
    /// supported.
    fn read_bytes(&mut self, _path: &PathVal, _limit: u64) -> Result<Vec<u8>, Failure> {
        Err(Failure::new(
            FailCode::HostUnavailable,
            "raw-byte file loading is not available",
        ))
    }

    /// The self-analysis context (14.5.9). The session's loader returns
    /// its `AnalysisCx` here, so a native reaches the capabilities and taps
    /// through the VM's `LoaderHost` (`host.0.analysis()`) without a new VM
    /// field. `None` (every other loader) makes the self-analysis natives
    /// fail `host-unavailable`.
    fn analysis(&mut self) -> Option<&mut AnalysisCx> {
        None
    }
}

/// The default loader: no host, every read fails `host-unavailable`. It
/// lives in `host::noop` with every other capability (design 12.8.2).
pub use crate::host::noop::NoopHost;

/// The VM host capability that carries the loader (`Vm::set_host`) and the
/// check diagnostics of the files loaded since the last `take_load_diags`.
pub struct LoaderHost(pub Box<dyn SourceLoader>, pub Vec<Diagnostic>);

/// Takes the check diagnostics of loaded files out of the VM's loader host.
pub fn take_load_diags(vm: &mut Vm) -> Vec<Diagnostic> {
    let Some(mut host) = vm.take_host() else {
        return Vec::new();
    };
    let out = host
        .downcast_mut::<LoaderHost>()
        .map(|h| std::mem::take(&mut h.1))
        .unwrap_or_default();
    vm.set_host(Some(host));
    out
}

/// Registers the `load` native against its table entry, once.
pub fn register_load(p: &mut Prelude) {
    if p.slot(crate::value::intern::intern_sym("load")).is_none() {
        p.register("load", load);
    }
    if p.slot(crate::value::intern::intern_sym("fm6-sysex"))
        .is_none()
    {
        p.register("fm6-sysex", crate::ns::fm6_sysex::fm6_sysex);
    }
}

/// Reads and expands `text`: the expanded top-level forms, or the first
/// reader or expander error.
///
/// # Errors
/// The first error-severity diagnostic.
pub fn read_forms(text: &str, file: FileId) -> Result<Vec<Node>, Diagnostic> {
    let r = read(text, file, &AliasEnv::new());
    if let Some(d) = r
        .diags
        .iter()
        .find(|d| matches!(d.severity, Severity::Error))
    {
        return Err(d.clone());
    }
    let mut ecx = ExpandCx::new(r.next_node_id());
    r.nodes.iter().map(|n| expand(n, &mut ecx)).collect()
}

fn load_failed(path: &PathVal, cause: &str) -> Failure {
    Failure::new(
        FailCode::LoadFailed,
        format!("load `{}`: {cause}", path.text),
    )
}

/// `load path`: an effect (`effect-in-query` inside a query).
fn load(cx: &mut NativeCx<'_>, args: &[Value], _: &[(KwId, Value)]) -> Result<Value, Failure> {
    if cx.effect_mode() == EffectMode::Query {
        return Err(Failure::new(
            FailCode::EffectInQuery,
            "`load` is not allowed inside a query",
        ));
    }
    let path = match args.first() {
        Some(Value::Path(p)) => Rc::clone(p),
        other => {
            return Err(Failure::new(
                FailCode::Type,
                format!(
                    "`load` expects a path, got {}",
                    other.map_or("nothing", kind_name)
                ),
            ))
        }
    };
    let Some(mut host) = cx.vm.take_host() else {
        return Err(Failure::new(
            FailCode::HostUnavailable,
            "no source loader is installed",
        ));
    };
    let read = match host.downcast_mut::<LoaderHost>() {
        Some(LoaderHost(loader, _)) => loader.read(&path),
        None => Err(Failure::new(
            FailCode::HostUnavailable,
            "no source loader is installed",
        )),
    };
    cx.vm.set_host(Some(host));
    let (file, text) = read?;
    // The loaded file's reads are not the caller's eager reads.
    let observer = cx.vm.take_read_observer();
    let mut diags = Vec::new();
    let r = run_source(cx.vm, cx.ns.prelude(), &path, &text, file, &mut diags);
    cx.vm.set_read_observer(observer);
    if let Some(mut host) = cx.vm.take_host() {
        if let Some(h) = host.downcast_mut::<LoaderHost>() {
            h.1.extend(diags);
        }
        cx.vm.set_host(Some(host));
    }
    r
}

/// Runs a loaded file in a fresh scope over `prelude` and returns its last
/// top-level value (`nil` when empty). The forms are checked first as one
/// document; the diagnostics go to `diags`. Each form's run is a nested VM run,
/// one Rust-level re-entry (7.1.5), so a load cycle ends in
/// `depth-exceeded`, which is passed through unwrapped; every other reader,
/// expander, compile or run failure is `load-failed` carrying the cause.
fn run_source(
    vm: &mut Vm,
    prelude: &Rc<Prelude>,
    path: &PathVal,
    text: &str,
    file: FileId,
    diags: &mut Vec<Diagnostic>,
) -> Result<Value, Failure> {
    let ns = Namespace::with_prelude(Rc::clone(prelude));
    let forms = read_forms(text, file).map_err(|d| load_failed(path, &d.to_string()))?;
    diags.extend(check(&forms, &ns.check_env(), &HostManifest::spec_default()).diags);
    run_forms(vm, &ns, path, &forms)
}

fn run_forms(
    vm: &mut Vm,
    ns: &Namespace,
    path: &PathVal,
    forms: &[Node],
) -> Result<Value, Failure> {
    let mut last = Value::Nil;
    for form in forms {
        let inputs = ns.checked_inputs();
        let checked = check(
            std::slice::from_ref(form),
            &ns.check_env(),
            &HostManifest::spec_default(),
        );
        let checked_ok = !checked.diags.iter().any(|d| d.severity == Severity::Error);
        let mut cx = CompileCx::new(ns, FormGen::new(0));
        cx.tweak_sites = false;
        let proto = compile(form, &mut cx).map_err(|d| load_failed(path, &d.to_string()))?;
        let compile_ok = !cx.diags.iter().any(|d| d.severity == Severity::Error);
        last = vm.run(proto, ns).map_err(|e| match e.code {
            FailCode::DepthExceeded => e,
            _ => {
                let mut f = load_failed(path, &e.to_string());
                f.origin = e.origin;
                f
            }
        })?;
        if checked_ok && compile_ok {
            if let Some(inputs) = inputs.as_ref() {
                ns.install_checked_callables(&checked.callables, inputs, FormGen::new(0));
            }
        }
    }
    Ok(last)
}

#[cfg(test)]
mod certificate_tests {
    use super::*;
    use crate::value::intern::intern_sym;
    fn execute(vm: &mut Vm, ns: &Namespace, text: &str) -> Result<Value, Failure> {
        let forms = read_forms(text, FileId::new(10)).unwrap();
        run_forms(
            vm,
            ns,
            &PathVal {
                text: "loaded.vact".into(),
                file: None,
            },
            &forms,
        )
    }
    #[test]
    fn same_owner_source_replacement_and_default_dependencies_invalidate_proofs() {
        let mut vm = Vm::new();
        let mut prelude = Prelude::core();
        crate::vm::natives::register_domain(&mut prelude);
        let ns = Namespace::with_prelude(Rc::new(prelude));
        execute(&mut vm, &ns, "var base-pitch 66.75\nfn helper p:\n\tgain p 1\nfn wrap p pitch: float = base-pitch:\n\thelper {s p} > note pitch").unwrap();
        assert!(ns.check_env().global_callables.contains_key("wrap"));
        assert_eq!(
            ns.session_slot(intern_sym("wrap")).unwrap().owner(),
            Some(FormGen::new(0))
        );
        let pitch = ns.session_slot(intern_sym("base-pitch")).unwrap();
        pitch.set(Value::Float(67.25));
        assert!(!ns.check_env().global_callables.contains_key("wrap"));
        assert!(ns.check_env().global_callables.contains_key("helper"));
        execute(&mut vm, &ns, "fn dependent p:\n\thelper p").unwrap();
        assert!(ns.check_env().global_callables.contains_key("dependent"));
        execute(&mut vm, &ns, "fn helper p:\n\tgain p 0.5").unwrap();
        assert!(!ns.check_env().global_callables.contains_key("dependent"));
        assert!(ns.check_env().global_callables.contains_key("helper"));
    }
    #[test]
    fn failed_source_definition_does_not_certify_or_destroy_prior_proof() {
        let mut vm = Vm::new();
        let mut prelude = Prelude::core();
        crate::vm::natives::register_domain(&mut prelude);
        let ns = Namespace::with_prelude(Rc::new(prelude));
        execute(&mut vm, &ns, "fn helper p:\n\tgain p 1").unwrap();
        let slot = ns.session_slot(intern_sym("helper")).unwrap();
        let version = slot.version();
        assert_eq!(
            execute(&mut vm, &ns, "let helper {/ 1 0}")
                .unwrap_err()
                .code,
            FailCode::LoadFailed
        );
        assert_eq!(slot.version(), version);
        assert!(ns.check_env().global_callables.contains_key("helper"));
        execute(&mut vm, &ns, "fn bad p: float:\n\ts p").unwrap();
        assert!(!ns.check_env().global_callables.contains_key("bad"));
    }
}
