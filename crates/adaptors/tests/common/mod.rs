//! Loopback nodes, hosted clients and the records a scene rests on.

#![allow(dead_code)]

use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::courier::Courier;
use rhtn_adaptors::direct::{NoDirect, Reachable};
use rhtn_adaptors::serving::{Inboxes, LocalNode, Serving};
use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_client::ceremony::{Abort, Client, Config, Dispatched, Msg};
use rhtn_client::device::*;
use rhtn_client::notice::{Notice, Notifier, Silent};
use rhtn_client::sequence::Conversed;
use rhtn_codec::cose::sha256;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedReceiver;

/// The subnet the fixtures' positions are in (`wire-format.md` §2.3 field 4).
pub const SUBNET: [u8; 32] = [0xdb; 32];

// four witnesses, so a four-nominee set can be pruned to three
// [author, 2026-10-19]: a party absent from this cast is one the
// node never knew, whose bundle it refuses and whom nobody can reach
pub const NAMES: [&str; 8] = ["alice", "bob", "carol", "w1", "w2", "w3", "w4", "witness"];

pub fn id(n: &str) -> SigningIdentity {
    test_identity(n)
}

pub fn kh(n: &str) -> Keyhash {
    test_identity(n).public.keyhash
}

pub fn ids() -> Vec<Identity> {
    NAMES.iter().map(|n| test_identity(n).public).collect()
}

pub fn pins() -> Pins {
    let p = Pins::new();
    for n in NAMES {
        p.pin_identity(&test_identity(n).public);
    }
    p
}

pub fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

pub fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        me: tls::Party::of(Arc::new(id(name))),
        pins: pins(),
        bind: Default::default(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: Duration::from_millis(1500),
        on_reachability: None,
        log: Log::recording(),
    }
}

pub fn client_ep() -> quinn::Endpoint {
    tls::client_endpoint(loopback()).unwrap()
}

pub fn know(cfg: &ClientConfig, name: &str, addr: SocketAddr) {
    cfg.addresses
        .lock()
        .unwrap()
        .entry(kh(name))
        .or_default()
        .push(addr);
}

/// A live node for `name` at `path` below `anchor`, holding `table`.
pub fn live_node(name: &str, table: Table, anchor: &str, path: &[u8]) -> Arc<LiveNode> {
    let p = Path::from_indices(path);
    let mut view = NodeView::new(
        Arc::new(id(name)),
        Locator {
            anchor: kh(anchor),
            path: p.bytes,
            nibbles: p.nibbles,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
            subnet: Some(SUBNET),
        },
    );
    view.table = table;
    let mut cfg = NodeConfig::defaults(Arc::new(id(name)), pins(), 30);
    cfg.log = Log::recording();
    LiveNode::start(
        cfg,
        view,
        ids(),
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    )
}

/// Poll `done` until it holds or `ms` elapse.
pub async fn until(ms: u64, mut done: impl FnMut() -> bool) -> bool {
    let end = tokio::time::Instant::now() + Duration::from_millis(ms);
    while tokio::time::Instant::now() < end {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    done()
}

// ---------------------------------------------------------------- records

/// The records a scene rests on, signed as the parties would sign them.
pub struct Scene {
    archives: BTreeMap<Keyhash, Archive>,
    pub store: BTreeMap<Txid, Vec<u8>>,
    pub records: Vec<Record>,
    clock: u64,
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl Scene {
    pub fn new() -> Scene {
        Self::starting(1_800_000_000)
    }

    /// A scene whose first record is an hour after `clock`: what a bundle
    /// handed to a selector needs, since only a record finalized before
    /// the ceremony qualifies (`wire-format.md` §5.3).
    pub fn starting(clock: u64) -> Scene {
        Scene {
            archives: NAMES.iter().map(|n| (kh(n), Archive::new(kh(n)))).collect(),
            store: BTreeMap::new(),
            records: Vec::new(),
            clock,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }

    fn back(&self, n: &str) -> Vec<Txid> {
        self.archives[&kh(n)].next_back_pointers()
    }

    fn commit(&mut self, tx_type: u64, body: &[u8], signers: &[&str]) -> Record {
        let sids: Vec<SigningIdentity> = signers.iter().map(|s| id(s)).collect();
        let refs: Vec<&SigningIdentity> = sids.iter().collect();
        let rec = Record::parse(&envelope(tx_type, body, &refs)).expect("well-formed");
        for s in signers {
            self.archives
                .get_mut(&kh(s))
                .unwrap()
                .append(rec.clone())
                .expect("appends");
        }
        self.store.insert(rec.txid, rec.bytes.clone());
        self.records.push(rec.clone());
        rec
    }

    /// Presence between `a` and `b`: a formation record where both keys
    /// are fresh, a witnessed record otherwise (`wire-format.md` §3.2).
    pub fn meet(&mut self, a: &str, b: &str) -> Record {
        let t = self.tick();
        if self.archives[&kh(a)].is_empty() && self.archives[&kh(b)].is_empty() {
            let (ba, bb) = (self.back(a), self.back(b));
            let root = sha256(format!("f:{a}:{b}:{t}").as_bytes());
            let body = formation_body([&ba, &bb], [&kh(a), &kh(b)], t, t + 600, &root);
            return self.commit(TYPE_PRESENCE, &body, &[a, b]);
        }
        let back = vec![self.back(a), self.back(b), self.back("witness")];
        let root = sha256(format!("m:{a}:{b}:{t}").as_bytes());
        let w = Witness {
            keyhash: kh("witness"),
            nominated_by: kh(a),
            flags: 3,
        };
        let body = presence_record_body(&back, [&kh(a), &kh(b)], &[w], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b, "witness"])
    }

    /// `node` adopted under `patron` at `path` below `anchor`, on presence
    /// between them.
    pub fn adopt(&mut self, node: &str, patron: &str, anchor: &str, path: &[u8]) -> Record {
        let pop = self.meet(patron, node);
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let p = Path::from_indices(path);
        let a = Adoption {
            node: kh(node),
            patron: kh(patron),
            locator: Locator {
                anchor: kh(anchor),
                path: p.bytes,
                nibbles: p.nibbles,
                seqno: Seqno {
                    series: 1,
                    counter: 0,
                },
                subnet: Some(SUBNET),
            },
            timestamp: t,
            key_material: None,
            evidence: Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&bn, &bp],
            carrying: None,
        };
        self.commit(TYPE_ADOPTION, &adoption_body(&a), &[node, patron])
    }

    /// A table for `me` holding every record so far, `infra` marked.
    pub fn table(&self, me: &str, infra: &[&str]) -> Table {
        let mut t = Table::with_me(kh(me));
        for n in infra {
            t.mark_infra(kh(n));
        }
        for r in &self.records {
            t.apply(r, &ids(), &self.store, None)
                .unwrap_or_else(|e| panic!("apply: {e:?}"));
        }
        t
    }
}

// ----------------------------------------------------------------- device

struct Cam;

impl Camera for Cam {
    fn capture(&self, _: Prompt) -> RawFrame {
        RawFrame {
            pixels: vec![0; 64],
            metadata: BTreeMap::new(),
        }
    }

    fn probe(&self) -> RawFrame {
        RawFrame {
            pixels: vec![0; 64],
            metadata: BTreeMap::new(),
        }
    }
}

/// The wall clock, since the adaptors' bounds elapse in real time.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
    fn wait_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

struct OsRandom;

impl Random for OsRandom {
    fn fill(&self, out: &mut [u8]) {
        for chunk in out.chunks_mut(32) {
            let r = tls::random_bytes::<32>();
            chunk.copy_from_slice(&r[..chunk.len()]);
        }
    }
}

struct Yes;

impl Operator for Yes {
    fn ask(&self, _: &str) -> bool {
        true
    }
}

/// The wall clock a fixed offset away: a witness whose clock is far.
pub struct SkewedClock(pub i64);

impl Clock for SkewedClock {
    fn now_ms(&self) -> u64 {
        (SystemClock.now_ms() as i64 + self.0 * 1000) as u64
    }
    fn wait_ms(&self, ms: u64) {
        SystemClock.wait_ms(ms);
    }
}

/// A device with nothing to run a ceremony on, a wall clock, and the
/// direct path the adaptors report.
pub fn device(direct: Rc<dyn DirectPath>) -> Device {
    device_with(direct, 0)
}

/// A device whose clock is `skew_s` from the wall.
pub fn device_with(direct: Rc<dyn DirectPath>, skew_s: i64) -> Device {
    let clock: Rc<dyn Clock> = if skew_s == 0 {
        Rc::new(SystemClock)
    } else {
        Rc::new(SkewedClock(skew_s))
    };
    Device {
        camera: Rc::new(Cam),
        clock,
        random: Rc::new(OsRandom),
        operator: Rc::new(Yes),
        notifier: Rc::new(Silent),
        engine: Rc::new(HashEngine::new(16)),
        direct,
    }
}

/// A client for `name` on a thread of its own, its route choice reading
/// `reachable`.
pub fn spawn_client(name: &'static str, cfg: Config, reachable: Reachable) -> Handle {
    spawn_client_with(name, cfg, reachable, 0)
}

/// The same, on a device with a clock `skew_s` from the wall.
pub fn spawn_client_with(
    name: &'static str,
    cfg: Config,
    reachable: Reachable,
    skew_s: i64,
) -> Handle {
    Handle::spawn(move || {
        Client::new(
            id(name),
            ids(),
            cfg,
            device_with(Rc::new(reachable), skew_s),
        )
    })
    .expect("the client builds")
}

// ------------------------------------------------ devices that remember

/// A camera whose frames start with the face in front of it — whoever the
/// test put there — and carry the metadata a pipeline attaches: location,
/// time, device.  It remembers every prompt it was given and when.
pub struct Watching {
    pub facing: Mutex<Vec<u8>>,
    pub captures: Mutex<Vec<(u64, Prompt)>>,
}

impl Watching {
    pub fn new() -> Arc<Watching> {
        Arc::new(Watching {
            facing: Mutex::new(face("nobody")),
            captures: Mutex::new(Vec::new()),
        })
    }
    pub fn prompts(&self) -> Vec<Prompt> {
        self.captures
            .lock()
            .unwrap()
            .iter()
            .map(|(_, p)| *p)
            .collect()
    }
    pub fn first_capture_after(&self, t: u64) -> Option<u64> {
        self.captures
            .lock()
            .unwrap()
            .iter()
            .map(|(at, _)| *at)
            .find(|at| *at >= t)
    }
}

impl Camera for Watching {
    fn capture(&self, prompt: Prompt) -> RawFrame {
        self.captures
            .lock()
            .unwrap()
            .push((SystemClock.now_ms(), prompt));
        let mut pixels = self.facing.lock().unwrap().clone();
        pixels.extend_from_slice(format!(":{prompt:?}").as_bytes());
        let metadata = BTreeMap::from([
            ("GPSLatitude".to_string(), b"51.5074N".to_vec()),
            (
                "DateTimeOriginal".to_string(),
                b"2026:09:11 10:00:00".to_vec(),
            ),
            ("Model".to_string(), b"HarnessCam 1".to_vec()),
        ]);
        RawFrame { pixels, metadata }
    }

    fn probe(&self) -> RawFrame {
        RawFrame {
            pixels: vec![0; 64],
            metadata: BTreeMap::new(),
        }
    }
}

/// The face a camera is looking at.
pub fn face(name: &str) -> Vec<u8> {
    format!("face:{name:<10}").into_bytes()
}

/// An operator who says yes to everything and remembers being asked.
pub struct Asked(pub Mutex<Vec<String>>);

impl Operator for Asked {
    fn ask(&self, question: &str) -> bool {
        self.0.lock().unwrap().push(question.to_string());
        true
    }
}

/// A notification hook that records every notice with the clock reading.
pub struct Told(pub Mutex<Vec<(u64, Notice)>>);

impl Notifier for Told {
    fn notify(&self, n: Notice) {
        self.0.lock().unwrap().push((SystemClock.now_ms(), n));
    }
}

/// What a test keeps of one client's device: the parts that remember.
#[derive(Clone)]
pub struct Kept {
    pub cam: Arc<Watching>,
    pub person: Arc<Asked>,
    pub hook: Arc<Told>,
}

impl Kept {
    pub fn new() -> Kept {
        Kept {
            cam: Watching::new(),
            person: Arc::new(Asked(Mutex::new(Vec::new()))),
            hook: Arc::new(Told(Mutex::new(Vec::new()))),
        }
    }
    /// Every question this device's person was asked.
    pub fn asked(&self) -> Vec<String> {
        self.person.0.lock().unwrap().clone()
    }
    /// Every notice raised here, with the reading when it was.
    pub fn notices(&self) -> Vec<(u64, Notice)> {
        self.hook.0.lock().unwrap().clone()
    }
}

/// A device whose camera, person and notifier are `kept`, with
/// `channels` passing and a clock `skew_s` from the wall.
pub fn device_keeping(direct: Rc<dyn DirectPath>, skew_s: i64, kept: Kept) -> Device {
    let mut d = device_with(direct, skew_s);
    d.camera = Rc::new(KeptCam(kept.cam.clone()));
    d.operator = Rc::new(KeptAsk(kept.person.clone()));
    d.notifier = Rc::new(KeptTell(kept.hook.clone()));
    d
}

/// The device's parts are `Rc`, the test's handle on them is `Arc`: these
/// three carry one across.
struct KeptCam(Arc<Watching>);
impl Camera for KeptCam {
    fn capture(&self, p: Prompt) -> RawFrame {
        self.0.capture(p)
    }
    fn probe(&self) -> RawFrame {
        self.0.probe()
    }
}
struct KeptAsk(Arc<Asked>);
impl Operator for KeptAsk {
    fn ask(&self, q: &str) -> bool {
        self.0.ask(q)
    }
}
struct KeptTell(Arc<Told>);
impl Notifier for KeptTell {
    fn notify(&self, n: Notice) {
        self.0.notify(n)
    }
}

/// A client for `name` on a thread of its own, on a device whose camera,
/// person and notifier the caller keeps.
pub fn spawn_client_keeping(
    name: &'static str,
    cfg: Config,
    reachable: Reachable,
    skew_s: i64,
    kept: Kept,
) -> Handle {
    Handle::spawn(move || {
        Client::new(
            id(name),
            ids(),
            cfg,
            device_keeping(Rc::new(reachable), skew_s, kept),
        )
    })
    .expect("the client builds")
}

// ------------------------------------------------------- hosted parties

/// One light client beside the node, with what arrived at it.
pub struct Party {
    pub handle: Handle,
    pub courier: Arc<Courier>,
    pub app: UnboundedReceiver<(Keyhash, Dispatched)>,
    /// Everything that arrived, in order, once drained.
    pub seen: Vec<(Keyhash, Dispatched)>,
    /// The parts of its device that remember.
    pub kept: Kept,
}

impl Party {
    pub async fn with<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Client) -> R + Send + 'static,
    ) -> R {
        self.handle.with(f).await
    }

    /// Take a step on the client and carry what it returns.
    pub async fn step(
        &self,
        f: impl FnOnce(&mut Client) -> Result<Vec<Msg>, Abort> + Send + 'static,
    ) -> Result<(), Abort> {
        let msgs = self.handle.with(f).await?;
        let carried = self.courier.carry(msgs).await;
        assert!(carried.complete(), "{carried:?}");
        Ok(())
    }

    /// Poll `f` on the client until it holds or `ms` elapse.
    pub async fn holds(
        &self,
        ms: u64,
        f: impl Fn(&mut Client) -> bool + Send + Sync + 'static,
    ) -> bool {
        let f = Arc::new(f);
        let end = Instant::now() + Duration::from_millis(ms);
        loop {
            let g = f.clone();
            if self.handle.with(move |c| g(c)).await {
                return true;
            }
            if Instant::now() >= end {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Take whatever has arrived, without waiting.
    pub fn drain(&mut self) {
        while let Ok(e) = self.app.try_recv() {
            self.seen.push(e);
        }
    }

    /// What the conversation came to here, by sender, in arrival order.
    pub fn conversed(&self) -> Vec<(Keyhash, Conversed)> {
        self.seen
            .iter()
            .filter_map(|(k, d)| match d {
                Dispatched::Conversation(c) => Some((*k, c.clone())),
                _ => None,
            })
            .collect()
    }
}

/// A light client beside the node with no direct path, on a device with
/// `channels`, a clock `skew_s` from the wall, and parts that remember.
pub fn host(
    name: &'static str,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
    skew_s: i64,
) -> Party {
    host_with(name, serving, inboxes, skew_s, ceremony_cfg())
}

/// The same, with the client's own configuration.
pub fn host_with(
    name: &'static str,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
    skew_s: i64,
    cfg: Config,
) -> Party {
    let reachable = Reachable::default();
    let kept = Kept::new();
    let handle = spawn_client_keeping(name, cfg, reachable.clone(), skew_s, kept.clone());
    let (courier, app) = Courier::new(
        handle.clone(),
        serving.clone(),
        Arc::new(NoDirect(reachable)),
    );
    inboxes.host(kh(name), courier.inbound());
    Party {
        handle,
        courier,
        app,
        seen: Vec::new(),
        kept,
    }
}

/// The client's numbers for a quick run: a short grant buffer, and a
/// capture over a fraction of a second.  The capture's own figures are
/// CER-03's to test, which asks for the defaults.
pub fn ceremony_cfg() -> Config {
    let mut c = Config::default();
    c.verifier.grant_buffer_ms = 300;
    // **the witness's window off**, the idiom the continuity window uses
    // [author, 2026-10-21]: these runs drive whole ceremonies against a
    // real clock and would otherwise wait out the window in every one.
    // What the window does is asserted where a clock can be moved —
    // `client/tests/ceremony.rs`, CER-52
    c.witness_window_s = 0;
    c.capture = CaptureParams {
        min_frames: 3,
        max_frames: 3,
        min_span_ms: 20,
        max_span_ms: 20,
    };
    c
}

/// **Run the bearer for the life of a meeting**, which is what a shell's
/// radio does and what no node can.
///
/// The counterparty's leg of kinds 9 to 18 is sealed under the local
/// session and carried locally (design §7.1), so the kernel hands it out
/// through `Client::carriages` and something outside the kernel has to
/// move it. A step taken directly and a step answered inside a delivery
/// both land in that drain, at moments a test cannot enumerate, so this
/// runs continuously rather than at chosen points.
///
/// It also drains the node-bound half, as `carry_outbox` does in the FFI
/// and for the same reason: a delivery queues what it owes and somebody
/// has to carry it.
pub fn bearing(all: Vec<(Keyhash, Handle, Arc<Courier>)>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // **whoever this phone is facing, for as long as the flow lasts**:
        // the kernel says who that is while a ceremony is open and says
        // nobody once it closes, and the last leg of all — the record —
        // is drained after the close. A shell holds the counterparty for
        // the life of the meeting (`Meet.counterparty`), and so does this.
        let mut facing: BTreeMap<Keyhash, Keyhash> = BTreeMap::new();
        loop {
            for (me, from, mine) in &all {
                if let Some(k) = from.with(|c| c.counterparty()).await {
                    facing.insert(*me, k);
                }
                let theirs = facing.get(me).and_then(|k| {
                    all.iter()
                        .find(|(who, _, _)| who == k)
                        .map(|(_, _, courier)| courier.clone())
                });
                if let Some(theirs) = theirs {
                    for bytes in from.with(|c| c.carriages()).await {
                        // through the recipient's courier, so the outcome
                        // reaches the application exactly as a network-borne
                        // step's does
                        theirs.take_carriage(bytes).await;
                    }
                }
                let msgs = from.with(|c| c.outbox()).await;
                if !msgs.is_empty() {
                    let out = mine.carry(msgs).await;
                    // **a refused conversation leg is a lost message**, and
                    // the FFI's `carry_outbox` reports one; a test bearer
                    // that swallowed it would hide exactly the loss this
                    // test exists to catch [reviewer, 2026-10-08]. A
                    // refused *record* is the node declining to store a
                    // subject outside its reach (`OutOfStore`), which a
                    // scene's table may make of a participant and which
                    // the signers' own archives are the assertion for
                    let lost: Vec<&Msg> = out
                        .refused
                        .iter()
                        .filter(|m| !matches!(m, Msg::Record(_)))
                        .collect();
                    assert!(lost.is_empty(), "the courier refused {lost:?}");
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
}

// ------------------------------------------------------------ a meeting

/// **Everyone beside one node, and the meetings they make.**
///
/// The local exchanges are made by hand here, which is where the design
/// makes them (`wire-format.md` §14.3): a code shown, a phone held up, a
/// camera run, over an interface no node takes part in. Everything after
/// them is the conversation, and nothing here sequences it —
/// [`Meeting::attend`] is the shell's own loop (`Meet.converse` and
/// `Meet.poll`), calling the kernel's entry points when `progress()`
/// says each is due, while the bearer and the node carry what they
/// produce [author, 2026-10-14: "the harness should only drive actions as
/// user or network inputs might and gather results"].
pub struct Meeting {
    pub node: Arc<LiveNode>,
    pub serving: Arc<dyn Serving>,
    pub inboxes: Inboxes,
    pub parties: BTreeMap<&'static str, Party>,
    bearer: Option<tokio::task::JoinHandle<()>>,
    /// When the last meeting's local exchanges began.
    pub last_start: u64,
    /// How many nominees the last meeting named, both sides together.
    nominated: usize,
    /// What the proposer's archive held before the last meeting: the
    /// record of it is whatever appears that is not in here.
    before: std::collections::BTreeSet<Txid>,
}

/// One party to host: its name, and how far its clock is from the wall.
pub struct Who(pub &'static str, pub i64);

impl Who {
    /// A party whose clock is right.
    pub fn with(name: &'static str) -> Who {
        Who(name, 0)
    }
    /// A party with no proximity hardware and a clock `skew_s` out.
    pub fn skewed(name: &'static str, skew_s: i64) -> Who {
        Who(name, skew_s)
    }
    /// A party with nothing to run a ceremony on: a witness or a verifier.
    pub fn watching(name: &'static str) -> Who {
        Who(name, 0)
    }
}

/// A party holds `name` as a contact by material and locator: what a
/// nominator accumulates in use, seeded by hand — the set travels by
/// locator (kind 22), and a nominee the sender cannot introduce is not
/// offered to the prune.
pub async fn knows(p: &Party, name: &'static str) {
    let loc = rhtn_archive::tx::Locator::root(
        kh(name),
        rhtn_archive::tx::Seqno {
            series: 1,
            counter: 0,
        },
    );
    let sl = rhtn_archive::locator::signed_locator(&id(name), &loc);
    let material = id(name).public.key_material();
    let k = kh(name);
    p.with(move |c| {
        let contact = c.store.contacts.entry(k).or_insert_with(|| {
            rhtn_client::contacts::Contact::new(material, rhtn_client::contacts::How::Nominee, 1)
        });
        contact.locator = Some(sl);
    })
    .await;
}

/// **Engage this side's nominees until each is reached** [author,
/// 2026-10-19]: the first attempt on a nominee whose binding is not held
/// asks for it, so engaging is retried while the courier fetches — as a
/// device does over the seconds the optical exchange takes.  A nominee
/// still unreached at the deadline was unavailable.
pub async fn engage(p: &Party) -> Result<(), Abort> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        p.step(|c| c.converse_engage()).await?;
        let (engaged, nominated) = p.with(|c| c.nominees_engaged()).await;
        if engaged == nominated || Instant::now() >= deadline {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// **The patient proposer** [author, 2026-10-19]: a witness asked and not
/// yet answered is given the window, as a shell gives it.  There is no
/// floor and nothing gates — `converse_propose` would build a witnessless
/// body the moment it could — so a proposer that fires at the first
/// opportunity leaves every witness out of its own record.
pub async fn await_witnesses(p: &Party, patience: Duration) -> usize {
    let deadline = Instant::now() + patience;
    loop {
        let outstanding = p
            .with(|c| c.progress().map(|pr| pr.witnesses_outstanding))
            .await
            .unwrap_or_default();
        if outstanding == 0 || Instant::now() >= deadline {
            return outstanding;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Open the window for both sides [author, 2026-10-19]: engage each
/// side's nominees, let the prune's legs cross on the bearer, and retry
/// the open while it waits on the counterparty's keep.
pub async fn open_both(a: &Party, b: &Party) -> Result<(), Abort> {
    for p in [a, b] {
        engage(p).await?;
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut open = [false, false];
    loop {
        for (i, p) in [a, b].into_iter().enumerate() {
            if open[i] {
                continue;
            }
            match p.step(|c| c.converse_open()).await {
                Ok(()) => open[i] = true,
                Err(Abort::Waiting(_)) if Instant::now() < deadline => {}
                Err(e) => return Err(e),
            }
        }
        if open.iter().all(|o| *o) {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

impl Meeting {
    /// `who` hosted beside the node `node`, which holds `table` and
    /// serves them all; everyone attached and swept, so each holds the
    /// others' prekeys.
    pub async fn open(who: Vec<Who>, node: &'static str, table: Table) -> Meeting {
        Self::open_with(who, node, table, ceremony_cfg()).await
    }

    /// The same, with the clients' own configuration.
    pub async fn open_with(
        who: Vec<Who>,
        node: &'static str,
        table: Table,
        cfg: Config,
    ) -> Meeting {
        let live = live_node(node, table, node, &[]);
        let inboxes = Inboxes::default();
        let serving: Arc<dyn Serving> = LocalNode::new(live.clone(), inboxes.clone());
        let mut parties = BTreeMap::new();
        for Who(name, skew) in who {
            parties.insert(name, host_with(name, &serving, &inboxes, skew, cfg.clone()));
        }
        let population: Vec<Keyhash> = parties.keys().map(|n| kh(n)).collect();
        for p in parties.values() {
            p.courier.attach(population.clone()).await;
        }
        // **swept again once everyone has published**: the first to attach
        // swept before the last had a bundle, and a recipient can only
        // attribute an initial message from a peer whose binding it holds
        for p in parties.values() {
            p.courier.sweep(population.clone()).await;
        }
        Meeting {
            node: live,
            serving,
            inboxes,
            parties,
            bearer: None,
            last_start: 0,
            nominated: 0,
            before: Default::default(),
        }
    }

    pub fn party(&self, name: &str) -> &Party {
        self.parties.get(name).expect("a party in this meeting")
    }

    pub fn at(&mut self, name: &str) -> &mut Party {
        self.parties.get_mut(name).expect("a party in this meeting")
    }

    /// What `who` holds of `scene`, as a start from storage reads it: the
    /// records, the acquaintance they fold to, and a contact with a
    /// locator for each party `introduce` names.
    pub async fn holds(&self, who: &str, scene: &Scene, introduce: &[&'static str]) {
        let bundle: Vec<(Txid, Vec<u8>)> = scene
            .records
            .iter()
            .map(|r| (r.txid, r.bytes.clone()))
            .collect();
        let introduced: Vec<(Keyhash, &'static str)> =
            introduce.iter().map(|n| (kh(n), *n)).collect();
        self.party(who)
            .with(move |c| {
                for (t, b) in bundle {
                    c.store.records.insert(t, b);
                }
                c.refresh_acquaintance();
                for (k, n) in introduced {
                    let loc = Locator::root(
                        k,
                        Seqno {
                            series: 1,
                            counter: 0,
                        },
                    );
                    let material = id(n).public.key_material();
                    let mut contact = rhtn_client::contacts::Contact::new(
                        material,
                        rhtn_client::contacts::How::Counterparty,
                        1,
                    );
                    contact.locator = Some(rhtn_archive::locator::signed_locator(&id(n), &loc));
                    c.store.contacts.insert(k, contact);
                }
            })
            .await;
    }

    /// `whom` in `who`'s horizon, which is what a selection among a
    /// subject's introduced parties reads.
    pub async fn in_horizon(&self, who: &str, whom: &[&'static str]) {
        let ks: Vec<Keyhash> = whom.iter().map(|n| kh(n)).collect();
        self.party(who)
            .with(move |c| {
                for k in ks {
                    c.acquaintance.horizon.insert(k);
                }
            })
            .await;
    }

    /// **The local exchanges of one meeting** (`wire-format.md` §14.3;
    /// design §7.1 steps 1 to 5): the two intents, the channels, and each
    /// party capturing the other, with the cameras facing.
    pub async fn exchanges(
        &mut self,
        a: &'static str,
        b: &'static str,
        a_nom: &[&'static str],
        b_nom: &[&'static str],
    ) -> Result<(), Abort> {
        self.last_start = SystemClock.now_ms();
        self.nominated = a_nom.len() + b_nom.len();
        self.before = self
            .party(a)
            .with(|c| c.archive.records().map(|r| r.txid).collect())
            .await;
        *self.party(a).kept.cam.facing.lock().unwrap() = face(b);
        *self.party(b).kept.cam.facing.lock().unwrap() = face(a);
        let (ka, kb) = (kh(a), kh(b));
        let (na, nb): (Vec<Keyhash>, Vec<Keyhash>) = (
            a_nom.iter().map(|n| kh(n)).collect(),
            b_nom.iter().map(|n| kh(n)).collect(),
        );
        let ia = self
            .party(a)
            .with(move |c| c.begin(Some(kb), na, true))
            .await?;
        let ib = self
            .party(b)
            .with(move |c| c.begin(Some(ka), nb, false))
            .await?;
        // **a nominee is a contact the nominator can reach** (design
        // §7.1.1): the pool is the party's own contacts, held by material
        // and locator, which is also what the set travels by (kind 22).
        // The harness seeds what a live client accumulates
        for (who, noms) in [(a, a_nom), (b, b_nom)] {
            for n in noms {
                let sl = self.party(n).with(|c| c.own_locator()).await?;
                let material = id(n).public.key_material();
                let k = kh(n);
                self.party(who)
                    .with(move |c| {
                        let contact = c.store.contacts.entry(k).or_insert_with(|| {
                            rhtn_client::contacts::Contact::new(
                                material,
                                rhtn_client::contacts::How::Nominee,
                                1,
                            )
                        });
                        contact.locator = Some(sl);
                    })
                    .await;
            }
        }
        // the engagements go with the intent [author, 2026-10-19]: each
        // side to its own nominees, over the payload path, before the
        // counterparty has accepted anything — and retried while the
        // bindings land, the set closing when the id fixes
        self.bear();
        engage(self.party(a)).await?;
        engage(self.party(b)).await?;
        self.party(a).with(move |c| c.take_intent(kb, &ib)).await?;
        self.party(b).with(move |c| c.take_intent(ka, &ia)).await?;
        let kca = self.party(a).with(|c| c.capture_key()).await?;
        let kcb = self.party(b).with(|c| c.capture_key()).await?;
        let (ra, rb) = tokio::join!(
            self.party(a).with(move |c| c.capture(kcb)),
            self.party(b).with(move |c| c.capture(kca))
        );
        ra?;
        rb?;
        Ok(())
    }

    /// The bearers, one per hosted party, running until this meeting is
    /// dropped: each carries to whoever its own kernel is facing, so a
    /// scene where the same party meets two others in turn needs no
    /// rewiring between them.
    fn bear(&mut self) {
        if self.bearer.is_some() {
            return;
        }
        let all = self
            .parties
            .iter()
            .map(|(n, p)| (kh(n), p.handle.clone(), p.courier.clone()))
            .collect();
        self.bearer = Some(bearing(all));
    }

    /// **The conversation, attended as the shell attends it**: each side
    /// opens and puts its queries once (`Meet.converse`), then each does
    /// what `progress()` says is due (`Meet.poll`) — the responder hands
    /// over what it gathered, the proposer proposes — until a record is
    /// held or the ceremony cannot reach one.
    ///
    /// Nothing is moved by hand: every message rides the bearer or the
    /// sessions the courier opens through the node.
    pub async fn attend(&mut self, a: &'static str, b: &'static str) -> Result<Txid, Abort> {
        self.bear();
        // the sets and keeps cross on the bearer once it runs, and the
        // window opens with the request [author, 2026-10-19]: the open is
        // retried while it waits on the counterparty's keep
        let mut opened = std::collections::BTreeSet::new();
        let queries_by = Instant::now() + Duration::from_secs(5);
        let deadline = Instant::now() + Duration::from_secs(60);
        // **the patient proposer**, paced off the run's own deadline
        // rather than a constant of its own: a fixed window is a race on
        // a loaded machine, and the one thing this must not do is propose
        // before the witnesses it asked have answered
        let witnesses_by = deadline - Duration::from_secs(20);
        loop {
            for who in [a, b] {
                if !opened.contains(who) {
                    match self.party(who).step(|c| c.converse_open()).await {
                        Ok(()) => {
                            opened.insert(who);
                            self.party(who).step(|c| c.converse_queries()).await?;
                        }
                        Err(Abort::Waiting(_)) => continue,
                        Err(e) => return Err(e),
                    }
                }
                let Some(p) = self.party(who).with(|c| c.progress()).await else {
                    continue;
                };
                // the person's patience, which is the flow's and not the
                // kernel's: a verifier that never answers does not appear
                if p.queries_outstanding > 0 && Instant::now() < queries_by {
                    continue;
                }
                if !p.proposer {
                    if !p.handed {
                        self.party(who).step(|c| c.converse_gathered()).await?;
                    }
                    continue;
                }
                // **the patient person**: an asked witness that has not
                // answered is given the window, as the shell gives it —
                // the floor is gone and nothing gates, but a proposer
                // that fires the moment it can leaves every witness out
                if p.witnesses_outstanding > 0 && Instant::now() < witnesses_by {
                    continue;
                }
                if p.proposable {
                    match self.party(who).step(|c| c.converse_propose()).await {
                        Ok(()) | Err(Abort::Waiting(_)) => {}
                        Err(e) => return Err(e),
                    }
                }
                // a missing witness gates nothing [author, 2026-10-19]:
                // `proposable` no longer waits on one, and the record
                // finalises saying nobody attested
            }
            if let Some(t) = self.held(a).await {
                // **the record is the proposer's to send** (kind 18), and
                // every signer holds it: a test that read the proposer's
                // archive the moment it appeared would be reading before
                // the sending
                assert!(
                    self.party(b)
                        .holds(5_000, move |c| c.store.records.contains_key(&t))
                        .await,
                    "the record reached the counterparty"
                );
                return Ok(t);
            }
            if Instant::now() >= deadline {
                for who in [a, b] {
                    let p = self.party(who).with(|c| c.progress()).await;
                    eprintln!("{who}: {p:?}");
                }
                return Err(Abort::Waiting("the record never came"));
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// The record this meeting reached, if `who`'s archive has grown:
    /// not by time, which two meetings in one second share, and not by
    /// the archive's own order, which is by txid and so is a hash.
    async fn held(&self, who: &str) -> Option<Txid> {
        let before = self.before.clone();
        self.party(who)
            .with(move |c| {
                c.archive
                    .records()
                    .map(|r| r.txid)
                    .find(|t| !before.contains(t))
            })
            .await
    }

    /// One meeting: the local exchanges, then the conversation.
    pub async fn meet(
        &mut self,
        a: &'static str,
        b: &'static str,
        a_nom: &[&'static str],
        b_nom: &[&'static str],
    ) -> Result<Txid, Abort> {
        self.exchanges(a, b, a_nom, b_nom).await?;
        self.attend(a, b).await
    }

    /// The envelope `who` holds for `t`.
    pub async fn record(&self, who: &str, t: Txid) -> Vec<u8> {
        self.party(who)
            .with(move |c| c.store.records.get(&t).cloned())
            .await
            .expect("the record is held here")
    }
}

impl Drop for Meeting {
    fn drop(&mut self) {
        if let Some(b) = self.bearer.take() {
            b.abort();
        }
    }
}
