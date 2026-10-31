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

// Every public item carries its own documentation: the source is the
// fourth reading of the design, and a bare name is not one.
#![warn(missing_docs)]

use rhtn_node::resources::Backend;
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

/// The functions each binding of [`HOST_EXPORTS`] puts in the host
/// instance, in the manifest's vocabulary: `rhtn/1:block` is the six
/// functions of the storage block, the rest one function each.
const BINDING_FUNCS: [(&str, &[&str]); 5] = [
    ("rhtn/1:request", &["request"]),
    ("rhtn/1:response", &["response"]),
    (
        "rhtn/1:block",
        &[
            "block-read",
            "block-write",
            "block-sync",
            "block-size",
            "block-list",
            "block-remove",
        ],
    ),
    ("rhtn/1:log", &["log"]),
    ("rhtn/1:clock", &["clock"]),
];

/// The binding a host-instance function belongs to, in the manifest's
/// vocabulary.
fn binding_of(func: &str) -> Option<&'static str> {
    BINDING_FUNCS
        .iter()
        .find(|(_, fs)| fs.contains(&func))
        .map(|(b, _)| *b)
}

/// A name a package may keep a file under in its block: `[A-Za-z0-9._-]`,
/// one to 64 bytes, no path in it.
pub const BLOCK_NAME: &str = "[A-Za-z0-9._-]{1,64}";

fn block_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name != "."
        && name != ".."
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
}

/// **A package's storage block** (`infra-client-requirements.md` §9.3): a
/// directory of the host's, provisioned per instance, with a byte ceiling
/// the host enforces on every write and `sync` as the durability
/// discipline.  Files are flat and named ([`BLOCK_NAME`]); nothing in
/// here is read by the host or by anybody else.  The same block serves a
/// package in the sandbox, through the `rhtn/1:block` binding, and one
/// on a socket of its own, which is handed the path.
pub struct Block {
    dir: std::path::PathBuf,
    quota: u64,
}

impl Block {
    /// Provision the block at `dir`, creating it where it is not, with
    /// `quota` bytes as its ceiling.
    pub fn provision(dir: impl Into<std::path::PathBuf>, quota: u64) -> std::io::Result<Block> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Block { dir, quota })
    }

    /// Where the block is.
    #[must_use]
    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }

    /// The ceiling, in bytes.
    #[must_use]
    pub fn quota(&self) -> u64 {
        self.quota
    }

    fn path(&self, name: &str) -> Result<std::path::PathBuf, String> {
        if !block_name_ok(name) {
            return Err(format!(
                "`{name}` is not a block name: {BLOCK_NAME}, and no path"
            ));
        }
        Ok(self.dir.join(name))
    }

    /// How many bytes the block holds in all.
    #[must_use]
    pub fn used(&self) -> u64 {
        std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter_map(|e| e.metadata().ok())
                    .filter(|m| m.is_file())
                    .map(|m| m.len())
                    .sum()
            })
            .unwrap_or(0)
    }

    /// `len` bytes of `name` from `offset`, fewer where the file ends.
    pub fn read(&self, name: &str, offset: u64, len: usize) -> Result<Vec<u8>, String> {
        use std::io::{Read as _, Seek as _};
        let p = self.path(name)?;
        let mut f = match std::fs::File::open(&p) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.to_string()),
        };
        f.seek(std::io::SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        f.take(len as u64)
            .read_to_end(&mut out)
            .map_err(|e| e.to_string())?;
        Ok(out)
    }

    /// Write `bytes` into `name` at `offset`, extending the file where it
    /// ends before; the file's size after.  **Refused where the block
    /// would exceed its quota**, with the quota named, and nothing is
    /// written then.
    pub fn write(&self, name: &str, offset: u64, bytes: &[u8]) -> Result<u64, String> {
        use std::io::{Seek as _, Write as _};
        let p = self.path(name)?;
        let before = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        let after = before.max(offset.saturating_add(bytes.len() as u64));
        let used = self.used();
        if used - before + after > self.quota {
            return Err(format!(
                "the block's quota is {} bytes: {} held, and this write would make {}",
                self.quota,
                used,
                used - before + after
            ));
        }
        let mut f = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&p)
            .map_err(|e| e.to_string())?;
        f.seek(std::io::SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        f.write_all(bytes).map_err(|e| e.to_string())?;
        Ok(after)
    }

    /// `sync`: when this returns, what was written to `name` is on
    /// stable storage — the host's entire persistence contract.
    pub fn sync(&self, name: &str) -> Result<(), String> {
        let p = self.path(name)?;
        let f = std::fs::File::open(&p).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        if let Ok(d) = std::fs::File::open(&self.dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }

    /// The size of `name`, 0 where there is no such file.
    pub fn size(&self, name: &str) -> Result<u64, String> {
        let p = self.path(name)?;
        Ok(std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0))
    }

    /// The names held, sorted.
    #[must_use]
    pub fn list(&self) -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(&self.dir)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }

    /// Remove `name`; nothing where there is no such file.
    pub fn remove(&self, name: &str) -> Result<(), String> {
        let p = self.path(name)?;
        match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Destroy the block and everything in it: the lifecycle's end, an
    /// operator's act and never a reload's.
    pub fn destroy(self) -> std::io::Result<()> {
        std::fs::remove_dir_all(&self.dir)
    }
}

/// **A package's operator-visible log** (`infra-client-requirements.md`
/// §9.3): lines the package writes, each stamped by the host, kept in one
/// file bounded at `retain` bytes — the oldest lines go first.  For
/// operators, never members.
pub struct LogFile {
    path: std::path::PathBuf,
    retain: u64,
    lock: Mutex<()>,
}

impl LogFile {
    /// A log at `path`, retaining `retain` bytes.
    pub fn at(path: impl Into<std::path::PathBuf>, retain: u64) -> std::io::Result<LogFile> {
        let path = path.into();
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d)?;
        }
        Ok(LogFile {
            path,
            retain,
            lock: Mutex::new(()),
        })
    }

    /// Where the log is.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Append one line, stamped with the host's clock in milliseconds;
    /// the file is cut back to the retained bytes, whole lines, after.
    pub fn write(&self, line: &str) {
        use std::io::Write as _;
        let _held = self.lock.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let one = format!("{now} {}\n", line.replace(['\r', '\n'], " "));
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
        {
            let _ = f.write_all(one.as_bytes());
        }
        if let Ok(m) = std::fs::metadata(&self.path)
            && m.len() > self.retain
            && let Ok(bytes) = std::fs::read(&self.path)
        {
            let cut = bytes.len().saturating_sub(self.retain as usize);
            let from = bytes[cut..]
                .iter()
                .position(|b| *b == b'\n')
                .map_or(cut, |i| cut + i + 1);
            let _ = std::fs::write(&self.path, &bytes[from..]);
        }
    }

    /// The lines retained, oldest first.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        std::fs::read_to_string(&self.path)
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }
}

/// **What the host puts behind the bindings for one instance**: its
/// block, its log and the host's clock.  Absent where the package did not
/// declare the binding; a package reaching one it did not declare was
/// refused at admission.
#[derive(Clone)]
pub struct Env {
    /// The instance's storage block.
    pub block: Option<Arc<Block>>,
    /// The instance's operator-visible log.
    pub log: Option<Arc<LogFile>>,
    /// The host's clock, milliseconds since the epoch: advisory, and
    /// nothing in the protocol treats it as proof of anything.
    pub clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl Default for Env {
    fn default() -> Self {
        Env {
            block: None,
            log: None,
            clock: Arc::new(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0)
            }),
        }
    }
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
        Self {
            memory: 64 << 20,
            fuel: 200_000_000,
            table_elements: 10_000,
            response: 8 << 20,
        }
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
    /// The request ran into a budget the package declared, named.
    Budget(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAComponent(e) => write!(f, "not a component the host can compile: {e}"),
            Self::UnknownImport(i) => write!(f, "no such host binding: {i}"),
            Self::NoEntry => write!(f, "no `{GUEST_ENTRY}` export"),
            Self::Exhausted => write!(f, "the request spent what it was given"),
            Self::Trapped(e) => write!(f, "trapped: {e}"),
            Self::Oversized(n) => {
                write!(f, "the response is {n} bytes, over what the host carries")
            }
            Self::Budget(why) => write!(f, "over budget: {why}"),
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
    /// A budget the request ran into, named: the block's quota.
    budget: Option<String>,
    env: Env,
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
        let component =
            Component::new(&engine, bytes).map_err(|e| Refusal::NotAComponent(e.to_string()))?;

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
                    let Some(b) = binding_of(f) else {
                        return Err(Refusal::UnknownImport(format!("{name}#{f}")));
                    };
                    imports.push(b.to_string());
                }
            }
        }
        if !component
            .component_type()
            .exports(&engine)
            .any(|(n, _)| n == GUEST_ENTRY)
        {
            return Err(Refusal::NoEntry);
        }

        let mut linker: Linker<Call> = Linker::new(&engine);
        if !imports.is_empty() {
            // the instance is defined whole where any of it was asked for:
            // a component importing one of the two still links against an
            // instance carrying both, and asking for one is not a promise
            // not to be handed the other's name
            let mut inst = linker
                .instance(HOST_INSTANCE)
                .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
            inst.func_new(
                "request",
                |store: wasmtime::StoreContextMut<'_, Call>,
                 _: wasmtime::component::types::ComponentFunc,
                 _: &[Val],
                 out: &mut [Val]| {
                    out[0] = Val::List(store.data().request.iter().map(|b| Val::U8(*b)).collect());
                    Ok(())
                },
            )
            .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
            inst.func_new(
                "response",
                |mut store: wasmtime::StoreContextMut<'_, Call>,
                 _: wasmtime::component::types::ComponentFunc,
                 args: &[Val],
                 _: &mut [Val]| {
                    let Some(Val::List(body)) = args.first() else {
                        return Err(wasmtime::Error::msg("response takes one list<u8>"));
                    };
                    let bytes: Vec<u8> = body
                        .iter()
                        .map(|v| if let Val::U8(b) = v { *b } else { 0 })
                        .collect();
                    if bytes.len() > store.data().cap {
                        let n = bytes.len();
                        store.data_mut().over = Some(n);
                        return Err(wasmtime::Error::msg(format!("response is {n} bytes")));
                    }
                    store.data_mut().response = Some(bytes);
                    Ok(())
                },
            )
            .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
            link_block(&mut inst)?;
            inst.func_new(
                "log",
                |store: wasmtime::StoreContextMut<'_, Call>,
                 _: wasmtime::component::types::ComponentFunc,
                 args: &[Val],
                 _: &mut [Val]| {
                    let Some(Val::String(line)) = args.first() else {
                        return Err(wasmtime::Error::msg("log takes one string"));
                    };
                    match &store.data().env.log {
                        Some(l) => l.write(line),
                        None => return Err(wasmtime::Error::msg("no log was granted")),
                    }
                    Ok(())
                },
            )
            .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
            inst.func_new(
                "clock",
                |store: wasmtime::StoreContextMut<'_, Call>,
                 _: wasmtime::component::types::ComponentFunc,
                 _: &[Val],
                 out: &mut [Val]| {
                    out[0] = Val::U64((store.data().env.clock)());
                    Ok(())
                },
            )
            .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
        }

        Ok(Self {
            engine,
            component,
            linker,
            limits,
            imports,
        })
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
    pub fn serve(&self, message: &[u8], env: &Env) -> Result<Vec<u8>, Refusal> {
        let mut store = Store::new(
            &self.engine,
            Call {
                request: message.to_vec(),
                response: None,
                over: None,
                budget: None,
                env: env.clone(),
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
        store
            .set_fuel(self.limits.fuel)
            .map_err(|e| Refusal::Trapped(e.to_string()))?;

        let instance = self
            .linker
            .instantiate(&mut store, &self.component)
            .map_err(|e| classify(&e, &store))?;
        let entry = instance
            .get_func(&mut store, GUEST_ENTRY)
            .ok_or(Refusal::NoEntry)?;
        if let Err(e) = entry.call(&mut store, &[], &mut []) {
            if let Some(n) = store.data().over {
                return Err(Refusal::Oversized(n));
            }
            if let Some(why) = store.data().budget.clone() {
                return Err(Refusal::Budget(why));
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
    env: Env,
    running: Arc<Mutex<bool>>,
}

impl Hosted {
    /// A host serving out of `sandbox`, running from the start.
    #[must_use]
    pub fn new(sandbox: Sandbox) -> Self {
        Self::with(sandbox, Env::default())
    }

    /// A host serving out of `sandbox` with `env` behind the bindings.
    #[must_use]
    pub fn with(sandbox: Sandbox, env: Env) -> Self {
        Self {
            sandbox,
            env,
            running: Arc::new(Mutex::new(true)),
        }
    }

    /// Stop serving without unbinding: `resource-requirements.md` §8 has a
    /// host that cannot meet a package's requirements remain conforming,
    /// and a stopped package is that state made visible rather than an
    /// error at every request.
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    /// The sandbox this host serves out of.
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
        self.sandbox
            .serve(message, &self.env)
            .map_err(|r| r.to_string())
    }

    fn running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

/// What the block binding's functions take and give, each a thin call
/// into [`Block`] that traps where the block refuses: a trap ends the
/// request, and the budget it ran into is named to the caller.
fn link_block(inst: &mut wasmtime::component::LinkerInstance<'_, Call>) -> Result<(), Refusal> {
    fn string(v: Option<&Val>, what: &str) -> Result<String, wasmtime::Error> {
        match v {
            Some(Val::String(s)) => Ok(s.to_string()),
            _ => Err(wasmtime::Error::msg(format!("{what} is a string"))),
        }
    }
    fn u64_of(v: Option<&Val>, what: &str) -> Result<u64, wasmtime::Error> {
        match v {
            Some(Val::U64(n)) => Ok(*n),
            _ => Err(wasmtime::Error::msg(format!("{what} is a u64"))),
        }
    }
    fn block(store: &wasmtime::StoreContextMut<'_, Call>) -> Result<Arc<Block>, wasmtime::Error> {
        store
            .data()
            .env
            .block
            .clone()
            .ok_or_else(|| wasmtime::Error::msg("no block was granted"))
    }
    inst.func_new(
        "block-read",
        |store: wasmtime::StoreContextMut<'_, Call>,
         _: wasmtime::component::types::ComponentFunc,
         args: &[Val],
         out: &mut [Val]| {
            let b = block(&store)?;
            let name = string(args.first(), "name")?;
            let offset = u64_of(args.get(1), "offset")?;
            let len = match args.get(2) {
                Some(Val::U32(n)) => *n as usize,
                _ => return Err(wasmtime::Error::msg("len is a u32")),
            };
            let bytes = b.read(&name, offset, len).map_err(wasmtime::Error::msg)?;
            out[0] = Val::List(bytes.into_iter().map(Val::U8).collect());
            Ok(())
        },
    )
    .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
    inst.func_new(
        "block-write",
        |mut store: wasmtime::StoreContextMut<'_, Call>,
         _: wasmtime::component::types::ComponentFunc,
         args: &[Val],
         out: &mut [Val]| {
            let b = block(&store)?;
            let name = string(args.first(), "name")?;
            let offset = u64_of(args.get(1), "offset")?;
            let bytes: Vec<u8> = match args.get(2) {
                Some(Val::List(l)) => l
                    .iter()
                    .map(|v| if let Val::U8(b) = v { *b } else { 0 })
                    .collect(),
                _ => return Err(wasmtime::Error::msg("bytes is a list<u8>")),
            };
            match b.write(&name, offset, &bytes) {
                Ok(size) => {
                    out[0] = Val::U64(size);
                    Ok(())
                }
                Err(why) => {
                    store.data_mut().budget = Some(why.clone());
                    Err(wasmtime::Error::msg(why))
                }
            }
        },
    )
    .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
    inst.func_new(
        "block-sync",
        |store: wasmtime::StoreContextMut<'_, Call>,
         _: wasmtime::component::types::ComponentFunc,
         args: &[Val],
         _: &mut [Val]| {
            let b = block(&store)?;
            b.sync(&string(args.first(), "name")?)
                .map_err(wasmtime::Error::msg)
        },
    )
    .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
    inst.func_new(
        "block-size",
        |store: wasmtime::StoreContextMut<'_, Call>,
         _: wasmtime::component::types::ComponentFunc,
         args: &[Val],
         out: &mut [Val]| {
            let b = block(&store)?;
            out[0] = Val::U64(
                b.size(&string(args.first(), "name")?)
                    .map_err(wasmtime::Error::msg)?,
            );
            Ok(())
        },
    )
    .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
    inst.func_new(
        "block-list",
        |store: wasmtime::StoreContextMut<'_, Call>,
         _: wasmtime::component::types::ComponentFunc,
         _: &[Val],
         out: &mut [Val]| {
            let b = block(&store)?;
            out[0] = Val::List(b.list().into_iter().map(Val::String).collect());
            Ok(())
        },
    )
    .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
    inst.func_new(
        "block-remove",
        |store: wasmtime::StoreContextMut<'_, Call>,
         _: wasmtime::component::types::ComponentFunc,
         args: &[Val],
         _: &mut [Val]| {
            let b = block(&store)?;
            b.remove(&string(args.first(), "name")?)
                .map_err(wasmtime::Error::msg)
        },
    )
    .map_err(|e| Refusal::NotAComponent(e.to_string()))?;
    Ok(())
}
