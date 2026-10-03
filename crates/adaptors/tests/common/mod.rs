//! Loopback nodes, hosted clients and the records a scene rests on.

#![allow(dead_code)]

use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::direct::Reachable;
use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_client::ceremony::{Client, Config};
use rhtn_client::device::*;
use rhtn_client::notice::Silent;
use rhtn_codec::cose::sha256;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Party, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const NAMES: [&str; 6] = ["alice", "bob", "carol", "w1", "w2", "witness"];

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
        me: Party::of(Arc::new(id(name))),
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
            },
            timestamp: t,
            key_material: None,
            evidence: Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&bn, &bp],
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

/// Proximity hardware with a fixed set of channels, all of which pass.
pub struct Passing(pub Vec<ChannelKind>);

impl Proximity for Passing {
    fn supported(&self) -> Vec<ChannelKind> {
        self.0.clone()
    }
    fn run(&self, kind: ChannelKind, _: &Keyhash) -> ChannelOutcome {
        ChannelOutcome {
            kind,
            result: ChannelResult::Pass,
            resolution_m: None,
        }
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
    device_with(direct, Vec::new(), 0)
}

/// A device whose proximity hardware has `channels`, all passing, and
/// whose clock is `skew_s` from the wall.
pub fn device_with(direct: Rc<dyn DirectPath>, channels: Vec<ChannelKind>, skew_s: i64) -> Device {
    let clock: Rc<dyn Clock> = if skew_s == 0 {
        Rc::new(SystemClock)
    } else {
        Rc::new(SkewedClock(skew_s))
    };
    Device {
        proximity: Rc::new(Passing(channels)),
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
    spawn_client_with(name, cfg, reachable, Vec::new(), 0)
}

/// The same, on a device with `channels` and a clock `skew_s` from the
/// wall.
pub fn spawn_client_with(
    name: &'static str,
    cfg: Config,
    reachable: Reachable,
    channels: Vec<ChannelKind>,
    skew_s: i64,
) -> Handle {
    Handle::spawn(move || {
        Client::new(
            id(name),
            ids(),
            cfg,
            device_with(Rc::new(reachable), channels, skew_s),
        )
    })
    .expect("the client builds")
}
