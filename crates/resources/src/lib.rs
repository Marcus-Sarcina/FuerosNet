//! The sandbox a hosted package runs in.
//!
//! `infra-client-requirements.md` §9.2 states the rule this crate enforces:
//! **no host binding exposes network transaction primitives to a package**,
//! and it is not a matter of granting narrow scopes carefully — *the hooks
//! do not exist*.  A package receives its credential and its own request
//! traffic and nothing else: no topology, no liveness state, no queue
//! contents, no prekey requests, no role-evaluation inputs.  The credential
//! reaches it inside that traffic, as the headers `resource-requirements.md`
//! §3 lists, so both halves of what it may see arrive through one door.
//!
//! The component model is the mechanism because it is capability-based: a
//! component reaches what it is handed and has no ambient authority to
//! anything else.  Whether that holds against a hostile module is an open
//! engineering question, as §9.2 says plainly, and nothing here claims to
//! settle it.

use rhtn_node::resources::{Backend, HOST_EXPORTS};
use std::sync::{Arc, Mutex};
use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};

/// The component-model instance the host's two bindings live in.  The
/// manifest vocabulary is `rhtn/1:request` and `rhtn/1:response`
/// (`rhtn_node::resources::HOST_EXPORTS`); this is the same pair written
/// the way a component binary names an import.
pub const HOST_INSTANCE: &str = "rhtn:host/bindings@1.0.0";

/// The one function a package exports, and the whole of what the host calls.
pub const GUEST_ENTRY: &str = "handle";

/// The manifest's binding names with the instance prefix taken off, which
/// is how a component names a function inside an imported instance.
/// Derived rather than written out, so the two lists cannot drift apart.
fn offered() -> Vec<&'static str> {
    HOST_EXPORTS.iter().map(|n| n.rsplit_once(':').map_or(*n, |(_, f)| f)).collect()
}

/// What a package may spend on one request.
///
/// A package is code the operator installed, running beside privileged
/// state (§9.2), so none of these is optional: an unmetered guest denies
/// the node's own service by looping, and an unbounded memory denies it by
/// growing.  The defaults are a starting point an operator overrides, not
/// a number any document fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Linear memory ceiling, in bytes.
    pub memory: usize,
    /// Instructions one request may retire before it is stopped.
    pub fuel: u64,
    /// Elements across all of the guest's tables.
    pub table_elements: usize,
    /// Bytes of response the host will carry back.
    pub response: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self { memory: 64 << 20, fuel: 200_000_000, table_elements: 10_000, response: 8 << 20 }
    }
}

/// Why a package was not admitted, or why a request did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// The bytes are not a component the host can compile.
    NotAComponent(String),
    /// It imports something the host does not offer.  §9.2's rule is that
    /// the binding does not exist, so this is the whole of the check: the
    /// name is reported so an operator can tell a package apart from a
    /// package built against a different host.
    UnknownImport(String),
    /// It does not export the entry the host calls.
    NoEntry,
    /// It ran out of what one request may spend.
    Exhausted,
    /// It trapped, or the host binding it called was misused.
    Trapped(String),
    /// It returned more than the host will carry.
    Oversized(usize),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAComponent(e) => write!(f, "not a component the host can compile: {e}"),
            Self::UnknownImport(i) => write!(f, "no such host binding: {i}"),
            Self::NoEntry => write!(f, "no `{GUEST_ENTRY}` export"),
            Self::Exhausted => write!(f, "the request spent what it was given"),
            Self::Trapped(e) => write!(f, "trapped: {e}"),
            Self::Oversized(n) => write!(f, "the response is {n} bytes, over what the host carries"),
        }
    }
}

impl std::error::Error for Refusal {}

/// What one request has, and what it produced.  This is the entire store
/// state, which is the point: there is nothing else in it for a binding to
/// reach even if one were added by mistake.
struct Call {
    request: Vec<u8>,
    response: Option<Vec<u8>>,
    over: Option<usize>,
    limits: StoreLimits,
    cap: usize,
}

/// A package admitted to run here.
pub struct Sandbox {
    engine: Engine,
    component: Component,
    linker: Linker<Call>,
    limits: Limits,
    imports: Vec<String>,
}

impl Sandbox {
    /// Compile a package and decide whether it may run.
    ///
    /// The import check is made before instantiation rather than left to
    /// it.  Instantiation would fail anyway — an unsatisfied import is not
    /// linkable — but it fails naming a symbol, and an operator declining
    /// a package needs to be told which binding it wanted.
    pub fn admit(bytes: &[u8], limits: Limits) -> Result<Self, Refusal> {
        let mut cfg = Config::new();
        cfg.wasm_component_model(true);
        cfg.consume_fuel(true);
        let engine = Engine::new(&cfg).map_err(|e| Refusal::NotAComponent(e.to_string()))?;
        let component = Component::new(&engine, bytes).map_err(|e| Refusal::NotAComponent(e.to_string()))?;

        let mut imports = Vec::new();
        let ty = component.component_type();
        for (name, item) in ty.imports(&engine) {
            if name != HOST_INSTANCE {
                return Err(Refusal::UnknownImport(name.to_string()));
            }
            // naming the right instance is not the check.  A package that
            // asks this instance for `topology` has asked for a hook that
            // does not exist, and the refusal has to say which one.
            if let wasmtime::component::types::ComponentItem::ComponentInstance(inst) = item.ty {
                for (f, _) in inst.exports(&engine) {
                    let Some(k) = offered().iter().position(|o| *o == f) else {
                        return Err(Refusal::UnknownImport(format!("{name}#{f}")));
                    };
                    imports.push(HOST_EXPORTS[k].to_string());
                }
            }
        }
        if !component.component_type().exports(&engine).any(|(n, _)| n == GUEST_ENTRY) {
            return Err(Refusal::NoEntry);
        }

        let mut linker: Linker<Call> = Linker::new(&engine);
        if !imports.is_empty() {
            // the instance is defined whole where any of it was asked for:
            // a component importing one of the two still links against an
            // instance carrying both, and asking for one is not a promise
            // not to be handed the other's name
            let mut inst = linker.instance(HOST_INSTANCE).map_err(|e| Refusal::NotAComponent(e.to_string()))?;
            inst.func_new("request", |store: wasmtime::StoreContextMut<'_, Call>, _: wasmtime::component::types::ComponentFunc, _: &[Val], out: &mut [Val]| {
                out[0] = Val::List(store.data().request.iter().map(|b| Val::U8(*b)).collect());
                Ok(())
            })
            .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
            inst.func_new("response", |mut store: wasmtime::StoreContextMut<'_, Call>, _: wasmtime::component::types::ComponentFunc, args: &[Val], _: &mut [Val]| {
                let Some(Val::List(body)) = args.first() else {
                    return Err(wasmtime::Error::msg("response takes one list<u8>"));
                };
                let bytes: Vec<u8> = body.iter().map(|v| if let Val::U8(b) = v { *b } else { 0 }).collect();
                if bytes.len() > store.data().cap {
                    let n = bytes.len();
                    store.data_mut().over = Some(n);
                    return Err(wasmtime::Error::msg(format!("response is {n} bytes")));
                }
                store.data_mut().response = Some(bytes);
                Ok(())
            })
            .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
        }

        Ok(Self { engine, component, linker, limits, imports })
    }

    /// The host bindings this package asked for, in the manifest's own
    /// vocabulary, so a caller can compare them against a declaration
    /// without knowing how a component names an import.  Sorted, because
    /// what a package reaches is a set and a manifest is compared against
    /// it rather than against an order.
    #[must_use]
    pub fn reaches(&self) -> Vec<String> {
        let mut out = self.imports.clone();
        out.sort();
        out.dedup();
        out
    }

    /// Run one request.
    ///
    /// Each request gets its own store, so its fuel, its memory and its
    /// linear memory's contents are its own.  A package holds nothing
    /// across requests because there is no binding through which it could:
    /// with `request` and `response` the whole of what it is handed, the
    /// only durable state a resource has is on the far side of its own
    /// door.  A trap therefore costs the caller its answer and costs the
    /// next caller nothing.
    pub fn serve(&self, message: &[u8]) -> Result<Vec<u8>, Refusal> {
        let mut store = Store::new(
            &self.engine,
            Call {
                request: message.to_vec(),
                response: None,
                over: None,
                limits: StoreLimitsBuilder::new()
                    .memory_size(self.limits.memory)
                    .table_elements(self.limits.table_elements)
                    // structural ceilings rather than budgets an operator
                    // tunes: a component compiles to several core
                    // instances, and what actually bounds a package is
                    // the memory and the fuel above.
                    .instances(64)
                    .memories(4)
                    .tables(16)
                    .build(),
                cap: self.limits.response,
            },
        );
        store.limiter(|c| &mut c.limits);
        store.set_fuel(self.limits.fuel).map_err(|e| Refusal::Trapped(e.to_string()))?;

        let instance = self
            .linker
            .instantiate(&mut store, &self.component)
            .map_err(|e| classify(&e, &store))?;
        let entry = instance.get_func(&mut store, GUEST_ENTRY).ok_or(Refusal::NoEntry)?;
        if let Err(e) = entry.call(&mut store, &[], &mut []) {
            if let Some(n) = store.data().over {
                return Err(Refusal::Oversized(n));
            }
            return Err(classify(&e, &store));
        }

        let out = store.data_mut().response.take().unwrap_or_default();
        if out.len() > self.limits.response {
            return Err(Refusal::Oversized(out.len()));
        }
        Ok(out)
    }
}

/// Distinguish a package that spent its budget from one that broke.  The
/// two are different facts about the same package and an operator acts on
/// them differently: the first is capacity, which
/// `infra-client-requirements.md` §9 says is not a conformance failure on
/// either side, and the second is a defect.
fn classify(e: &wasmtime::Error, store: &Store<Call>) -> Refusal {
    if store.get_fuel().unwrap_or(1) == 0 {
        return Refusal::Exhausted;
    }
    if let Some(t) = e.downcast_ref::<wasmtime::Trap>()
        && *t == wasmtime::Trap::OutOfFuel
    {
        return Refusal::Exhausted;
    }
    Refusal::Trapped(format!("{e:#}"))
}

/// A `Sandbox` as the gateway sees it.
///
/// `rhtn_node::resources::Gateway` reaches a backend through `&self` from
/// whatever thread carries the request, so the store a call needs is built
/// per call rather than held.  The mutex is over the running flag alone.
pub struct Hosted {
    sandbox: Sandbox,
    running: Arc<Mutex<bool>>,
}

impl Hosted {
    #[must_use]
    pub fn new(sandbox: Sandbox) -> Self {
        Self { sandbox, running: Arc::new(Mutex::new(true)) }
    }

    /// Stop serving without unbinding: `resource-requirements.md` §8 has a
    /// host that cannot meet a package's requirements remain conforming,
    /// and a stopped package is that state made visible rather than an
    /// error at every request.
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    #[must_use]
    pub fn sandbox(&self) -> &Sandbox {
        &self.sandbox
    }
}

impl Backend for Hosted {
    fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String> {
        if !self.running() {
            return Err("the package is not running".into());
        }
        self.sandbox.serve(message).map_err(|r| r.to_string())
    }

    fn running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}
