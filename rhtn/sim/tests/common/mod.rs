//! Loopback nodes and clients for the session, queue and replication
//! entries, with the datagram path between them where a test needs one.

#![allow(dead_code)]

use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::SigningIdentity;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

pub const NAMES: [&str; 9] = ["alice", "bob", "carol", "alice2", "w1", "w2", "c1", "c2", "witness"];

/// The tests that count seconds on a wall clock run one at a time, so a
/// loaded machine does not turn a bound into a flake.
static SERIAL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn serial() -> tokio::sync::MutexGuard<'static, ()> {
    SERIAL.lock().await
}

pub fn id(n: &str) -> SigningIdentity {
    test_identity(n)
}

pub fn kh(n: &str) -> [u8; 32] {
    test_identity(n).public.keyhash
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

/// The instant every scenario starts from: the signers' clock opens here,
/// and every live node's clock is pinned here (a running node reads its
/// configuration's clock), so a test that needs time to pass sets a view's
/// clock forward rather than waiting on the wall.
pub const SIM_EPOCH: u64 = 1_800_000_000;

pub fn node_cfg(name: &str, interval: u64) -> NodeConfig {
    let mut cfg = NodeConfig::defaults(Arc::new(id(name)), pins(), interval);
    cfg.clock = Arc::new(|| SIM_EPOCH);
    cfg.log = Log::recording();
    cfg
}

pub fn client_cfg(name: &str) -> ClientConfig {
    ClientConfig {
        identity: Arc::new(id(name)),
        pins: pins(),
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(Default::default())),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: std::time::Duration::from_millis(1500),
        on_reachability: None,
        log: Log::recording(),
    }
}

pub fn client_ep() -> quinn::Endpoint {
    tls::client_endpoint(loopback()).unwrap()
}

/// A `SiblingRef` for `name` at `addr`, carrying its key material so a
/// client that has never contacted it can authenticate it (§8.2).
pub fn sibling_ref(name: &str, addr: SocketAddr) -> SiblingRef {
    let std::net::IpAddr::V4(v4) = addr.ip() else { panic!("v4") };
    SiblingRef {
        keyhash: kh(name),
        endpoints: vec![NetworkPoint { ip: v4.octets(), asn: None, port: Some(addr.port() as u64) }],
        key_material: Some(id(name).public.key_material()),
    }
}

/// Note an address for a keyhash in the client's own address book.
pub fn know(cfg: &ClientConfig, name: &str, addr: SocketAddr) {
    cfg.addresses.lock().unwrap().entry(kh(name)).or_default().push(addr);
}

/// A filter that drops every heartbeat, so a node can fall silent on the
/// detector's path while still sending everything else.
pub fn drop_heartbeats() -> OutboundFilter {
    Arc::new(|frame_type, bytes| if frame_type == FRAME_HEARTBEAT { None } else { Some(bytes.to_vec()) })
}

/// Every delivery that arrives within `ms`.
pub async fn drain(s: &mut Session, ms: u64) -> Vec<Vec<u8>> {
    let mut got = Vec::new();
    while let Ok(Some(b)) = tokio::time::timeout(std::time::Duration::from_millis(ms), s.deliveries.recv()).await {
        got.push(b);
    }
    got
}

pub fn messages(k: usize, tag: &str) -> Vec<Vec<u8>> {
    (0..k).map(|i| format!("{tag}:{i}").into_bytes()).collect()
}

// ------------------------------------------------------------ live nodes

use rhtn_archive::record::Record;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_crypto::Identity;

pub fn ids() -> Vec<Identity> {
    NAMES.iter().map(|n| test_identity(n).public).collect()
}

/// Signed transactions for a live scenario, archives advancing as they go.
pub struct Signers {
    archives: BTreeMap<Keyhash, rhtn_archive::chain::Archive>,
    pub store: BTreeMap<Txid, Vec<u8>>,
    pub clock: u64,
}

impl Default for Signers {
    fn default() -> Self {
        Self::new()
    }
}

impl Signers {
    pub fn new() -> Signers {
        Signers { archives: NAMES.iter().map(|n| (kh(n), rhtn_archive::chain::Archive::new(kh(n)))).collect(), store: BTreeMap::new(), clock: SIM_EPOCH }
    }
    fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }
    fn back(&self, n: &str) -> Vec<Txid> {
        self.archives[&kh(n)].next_back_pointers()
    }
    fn commit(&mut self, tx_type: u64, body: &[u8], signers: &[&str]) -> Record {
        let sids: Vec<SigningIdentity> = signers.iter().map(|s| test_identity(s)).collect();
        let refs: Vec<&SigningIdentity> = sids.iter().collect();
        let rec = Record::parse(&envelope(tx_type, body, &refs)).expect("well-formed");
        for s in signers {
            self.archives.get_mut(&kh(s)).unwrap().append(rec.clone()).expect("appends");
        }
        self.store.insert(rec.txid, rec.bytes.clone());
        rec
    }
    pub fn formation(&mut self, a: &str, b: &str) -> Record {
        let t = self.tick();
        let (ba, bb) = (self.back(a), self.back(b));
        let root = rhtn_codec::cose::sha256(format!("f:{a}:{b}:{t}").as_bytes());
        let body = formation_body([&ba, &bb], [&kh(a), &kh(b)], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b])
    }
    /// Presence evidence between `a` and `b`: a formation record where both
    /// keys are fresh, and otherwise a normal record witnessed by the
    /// harness's witness, since a key appears in at most one formation
    /// record, its first (`wire-format.md` §3.2).
    pub fn meet(&mut self, a: &str, b: &str) -> Record {
        if self.archives[&kh(a)].is_empty() && self.archives[&kh(b)].is_empty() {
            return self.formation(a, b);
        }
        let t = self.tick();
        let back = vec![self.back(a), self.back(b), self.back("witness")];
        let root = rhtn_codec::cose::sha256(format!("meeting:{a}:{b}:{t}").as_bytes());
        let w = Witness { keyhash: kh("witness"), nominated_by: kh(a), flags: 3 };
        let body = presence_record_body(&back, [&kh(a), &kh(b)], &[w], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b, "witness"])
    }

    /// A recovery adoption (`wire-format.md` §4.1): `new` claims `old`'s
    /// history under `patron`, placed at `(anchor, path, series)`, with
    /// `verifier`, a prior counterparty, recognising the holder.
    pub fn recover(&mut self, old: &str, new: &str, patron: &str, verifier: &str, place: (&str, &[u8], u32)) -> Record {
        let (anchor, path, series) = place;
        let t = self.tick();
        let qid = rhtn_codec::cose::sha256(format!("recover:{old}:{new}:{t}").as_bytes());
        let resp = recovery_response(&id(verifier), &id(new), &qid, &kh(old));
        let block = recovery_block(&id(old), &kh(new), &kh(patron), vec![resp]);
        let (bn, bp) = (self.back(new), self.back(patron));
        let p = rhtn_node::resolution::Path::from_indices(path);
        let a = Adoption {
            node: kh(new),
            patron: kh(patron),
            locator: Locator { anchor: kh(anchor), path: p.bytes, nibbles: p.nibbles, seqno: Seqno { series, counter: 0 } },
            timestamp: t,
            key_material: None,
            evidence: Evidence::Recovery(block),
            presented_head: None,
            back: [&bn, &bp],
        };
        self.commit(TYPE_ADOPTION, &adoption_body(&a), &[new, patron])
    }

    /// An adoption of `node` under `patron` at `path` in the subnet
    /// `anchor` names, on a fresh presence record.
    pub fn adopt(&mut self, node: &str, patron: &str, anchor: &str, path: &[u8], series: u32) -> Record {
        let pop = self.meet(patron, node);
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let p = rhtn_node::resolution::Path::from_indices(path);
        let a = Adoption {
            node: kh(node),
            patron: kh(patron),
            locator: Locator { anchor: kh(anchor), path: p.bytes, nibbles: p.nibbles, seqno: Seqno { series, counter: 0 } },
            timestamp: t,
            key_material: None,
            evidence: Evidence::Presence(pop.txid),
            presented_head: None,
            back: [&bn, &bp],
        };
        self.commit(TYPE_ADOPTION, &adoption_body(&a), &[node, patron])
    }
}

/// A table for `me` holding every record, evidence and all.
pub fn table_of(me: &str, s: &Signers, records: &[&Record], infra: &[&str]) -> Table {
    let mut t = Table::with_me(kh(me));
    for n in infra {
        t.mark_infra(kh(n));
    }
    for r in records {
        t.apply(r, &ids(), &s.store, None).unwrap_or_else(|e| panic!("apply: {e:?}"));
    }
    t
}

/// A node view for `me` at `path` under `anchor`.
pub fn view_of(me: &str, table: Table, anchor: &str, path: &[u8], now: u64) -> rhtn_node::view::NodeView {
    let p = rhtn_node::resolution::Path::from_indices(path);
    let mut v = rhtn_node::view::NodeView::new(Arc::new(id(me)), Locator { anchor: kh(anchor), path: p.bytes, nibbles: p.nibbles, seqno: Seqno { series: 1, counter: 0 } });
    v.table = table;
    v.set_now(now);
    v
}

/// Poll `done` until it holds or `ms` elapse.
pub async fn until(ms: u64, mut done: impl FnMut() -> bool) -> bool {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(ms);
    while tokio::time::Instant::now() < deadline {
        if done() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    done()
}
