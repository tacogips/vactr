//! VM tests (ME-VM required tests) and the session harness the compiler and
//! namespace tests share: read -> expand -> compile -> run.

use crate::compile::{compile, CompileCx};
use crate::expand::{expand, ExpandCx};
use crate::ns::namespace::{FormGen, Namespace, Prelude};
use crate::ns::stage::RecordingSink;
use crate::reader::span::FileId;
use crate::reader::{read, AliasEnv};
use crate::types::diag::Diagnostic;
use crate::types::natives::{NativeMask, NativeSig};
use crate::value::intern::KwId;
use crate::value::value::Value;
use crate::vm::call::NativeCx;
use crate::vm::fail::Failure;
use crate::vm::vm::Vm;

mod failures;
mod forcing;
mod inst;
mod integrate_pattern;
mod integrate_query;
mod integrate_sound;
mod integrate_tex;
mod native_table;
mod natives;
mod query_mode;
mod tweak;

/// An ordinary file (not the console).
pub(crate) const FILE: FileId = FileId::new(1);

/// A session: namespace, VM and a recording sink.
pub(crate) struct Sess {
    pub ns: Namespace,
    pub vm: Vm,
    pub sink: RecordingSink,
    pub gen: u64,
    pub diags: Vec<Diagnostic>,
    /// Import prefixes the reader accepts in qualified names.
    pub aliases: AliasEnv,
}

impl Sess {
    /// A session over the core prelude.
    pub(crate) fn new() -> Sess {
        Sess::with_prelude(Prelude::core())
    }

    pub(crate) fn with_prelude(p: Prelude) -> Sess {
        Sess {
            ns: Namespace::new(p),
            vm: Vm::new(),
            sink: RecordingSink::default(),
            gen: 0,
            diags: Vec::new(),
            aliases: AliasEnv::new(),
        }
    }

    /// Evaluates every top-level form of `src` in order and returns the
    /// last result. A form's effects reach the sink only when it succeeds.
    /// The source must read and expand clean.
    pub(crate) fn eval(&mut self, src: &str) -> Result<Value, Failure> {
        let r = read(src, FILE, &self.aliases);
        let codes: Vec<&str> = r.diags.iter().map(|d| d.code.as_str()).collect();
        assert!(codes.is_empty(), "{src:?}: reader diagnostics {codes:?}");
        let mut ecx = ExpandCx::new(r.next_node_id());
        let mut last = Ok(Value::Nil);
        for n in &r.nodes {
            let k = expand(n, &mut ecx).unwrap_or_else(|d| panic!("{src:?}: {d}"));
            self.gen += 1;
            let mut cx = CompileCx::new(&self.ns, FormGen::new(self.gen));
            let proto = compile(&k, &mut cx).unwrap_or_else(|d| panic!("{src:?}: {d}"));
            self.diags.extend(cx.diags);
            last = self.vm.run(proto, &self.ns);
            match &last {
                Ok(_) => self.vm.effects_mut().release(&mut self.sink),
                Err(_) => self.vm.effects_mut().drop_all(),
            }
        }
        last
    }

    /// The printed result, or `fail: CODE`.
    pub(crate) fn show(&mut self, src: &str) -> String {
        match self.eval(src) {
            Ok(v) => v.to_string(),
            Err(e) => format!("fail: {}", e.code),
        }
    }
}

/// Evaluates `src` in a fresh session and prints the last result.
pub(crate) fn show(src: &str) -> String {
    Sess::new().show(src)
}

fn first_arg(_: &mut NativeCx<'_>, args: &[Value], _: &[(KwId, Value)]) -> Result<Value, Failure> {
    Ok(args.first().cloned().unwrap_or(Value::Nil))
}

fn second_in_list(
    _: &mut NativeCx<'_>,
    args: &[Value],
    _: &[(KwId, Value)],
) -> Result<Value, Failure> {
    Ok(Value::list(args.get(1).cloned().into_iter().collect()))
}

fn nil_native(_: &mut NativeCx<'_>, _: &[Value], _: &[(KwId, Value)]) -> Result<Value, Failure> {
    Ok(Value::Nil)
}

const ANY1: &[&str] = &["fn any -> any"];
const ANY2: &[&str] = &["fn any any -> any"];

/// The core prelude plus test natives with chosen masks:
/// `late-probe` (`Late`, returns its argument untouched, standing in for a
/// domain constructor such as `osc`), `ctl` (`Value`, `Late`: a control
/// step that returns `[arg]` so the late argument survives a `let`), and
/// `value-at`, `fn-at`, `late-at` (an `at` whose body parameter is `Value`,
/// `Fn` or `Late`).
pub(crate) fn probe_prelude() -> Prelude {
    let mut p = Prelude::core();
    p.register_custom(
        NativeSig::func("late-probe", 1, 1, ANY1, &[NativeMask::Late]),
        first_arg,
    );
    p.register_custom(
        NativeSig::func("ctl", 2, 2, ANY2, &[NativeMask::Value, NativeMask::Late]),
        second_in_list,
    );
    p.register_custom(
        NativeSig::func(
            "value-at",
            2,
            2,
            ANY2,
            &[NativeMask::Value, NativeMask::Value],
        ),
        nil_native,
    );
    p.register_custom(
        NativeSig::func("fn-at", 2, 2, ANY2, &[NativeMask::Value, NativeMask::Fn]),
        nil_native,
    );
    p.register_custom(
        NativeSig::func(
            "late-at",
            2,
            2,
            ANY2,
            &[NativeMask::Value, NativeMask::Late],
        ),
        nil_native,
    );
    p
}

impl Sess {
    /// A session over `probe_prelude`.
    pub(crate) fn probe() -> Sess {
        Sess::with_prelude(probe_prelude())
    }

    /// The current value of the session binding `name`.
    pub(crate) fn get(&self, name: &str) -> Value {
        self.ns
            .session_value(name)
            .unwrap_or_else(|| panic!("`{name}` is not bound"))
    }

    /// The printed value of the session binding `name`.
    pub(crate) fn get_str(&self, name: &str) -> String {
        self.get(name).to_string()
    }
}

/// Reads, expands and compiles the single form `src` against `ns`.
pub(crate) fn compile_src(
    ns: &Namespace,
    src: &str,
) -> std::rc::Rc<crate::compile::proto::FnProto> {
    let r = read(src, FILE, &AliasEnv::new());
    let k = expand(&r.nodes[0], &mut ExpandCx::new(r.next_node_id())).expect("expand");
    compile(&k, &mut CompileCx::new(ns, FormGen::new(1000))).expect("compile")
}
