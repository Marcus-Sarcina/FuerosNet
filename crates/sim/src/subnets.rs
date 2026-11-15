//! Several subnets of live nodes, each pre-populated with the shape a
//! scenario describes, over loopback (design §12.6.1, §14.1.2;
//! `wire-format.md` §7.2, §7.6, §7.7).
//!
//! **What this is for.**  A client in one subnet reaching a party in
//! another holds nothing of the second but an anchor entry (§7.2): a
//! keyhash, an address and the material that binds it.  Everything past
//! that is the descent, run against real nodes answering from real
//! tables: the referral each infra node gives from the endpoint record it
//! holds of its child, and the serving answer the last one gives with the
//! residual that names the client.  The same fabric takes a relay
//! submission at that serving node and hands out a session for the client
//! it serves to collect on, so messaging between two subnets that share no
//! node is a scenario rather than a diagram.
//!
//! **Nothing here decides.**  The plan says who adopted whom at which
//! index and which of them run infrastructure; the records are signed as
//! the parties would sign them, each node's table is folded from those
//! records, and the nodes answer from what they hold.  A node's endpoint
//! record reaches its own store and its patron's, which is what §12.6.1
//! says a patron needs in order to refer; nothing else is pre-loaded.

use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::topology::Table;
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::resolution::{
    AnchorEntry, AnchorTable, Ingestion, Path, anchor_entry, endpoint_record,
};
use rhtn_node::runtime::LiveNode;
use rhtn_node::store::KIND_ENDPOINT_RECORD;
use rhtn_node::view::NodeView;
use rhtn_transport::session::*;
use rhtn_transport::tls::{self, Party, Pins};
use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// The instant the fabric's records start from; every node's clock is
/// pinned just after the last record, so nothing arrives from a node's
/// future.
pub const EPOCH: u64 = 1_800_000_000;

/// The witness every non-formation meeting is attested by: a party of the
/// fabric that sits in no subnet.
pub const WITNESS: &str = "witness";

/// One party of a subnet other than its anchor: whom it was adopted under,
/// at which child index, and whether it runs infrastructure.
#[derive(Debug, Clone)]
pub struct Member {
    /// Its name, which its test identity derives from.
    pub name: &'static str,
    /// The party that adopted it.
    pub patron: &'static str,
    /// The child index under that patron (`wire-format.md` §2.1).
    pub index: u8,
    /// Whether it serves sessions and answers resolutions.  A light member
    /// is served by its nearest infra ancestor and attaches there.
    pub infra: bool,
}

/// A subnet as a scenario describes it: its anchor, which is its root and
/// the party an outsider knows it by, and its members in adoption order.
#[derive(Debug, Clone)]
pub struct Plan {
    /// The root.  It runs infrastructure, since an anchor is reached.
    pub anchor: &'static str,
    /// Everyone else, each adopted after the one before.
    pub members: Vec<Member>,
}

impl Plan {
    /// A subnet rooted at `anchor` with nobody under it yet.
    pub fn new(anchor: &'static str) -> Plan {
        Plan {
            anchor,
            members: Vec::new(),
        }
    }

    /// An infra member under `patron` at `index`.  **Its patron must run
    /// infrastructure too**: referral is only ever between infra nodes
    /// (design §12.6.1), so an infra child of a light client is a shape
    /// the descent has no step for.
    pub fn infra(mut self, name: &'static str, patron: &'static str, index: u8) -> Plan {
        self.members.push(Member {
            name,
            patron,
            index,
            infra: true,
        });
        self
    }

    /// A light member under `patron` at `index`, served by its nearest
    /// infra ancestor.
    pub fn light(mut self, name: &'static str, patron: &'static str, index: u8) -> Plan {
        self.members.push(Member {
            name,
            patron,
            index,
            infra: false,
        });
        self
    }
}

/// No sessions at all, for loading a view's store by hand.
struct Quiet;

impl rhtn_node::Adjacency for Quiet {
    fn peers(&self) -> Vec<Keyhash> {
        Vec::new()
    }
    fn send(&self, _: &Keyhash, _: u64, _: &[u8]) {}
    fn request(&self, _: &Keyhash, _: u64, _: &[u8]) -> bool {
        false
    }
}

/// A subnet standing: its nodes, the records it rests on, where each party
/// sits, and the entry an outsider holds it by.
pub struct Subnet {
    /// Its anchor's name.
    pub anchor: &'static str,
    /// The subnet field of its locators (`wire-format.md` §2.3 field 4).
    pub id: [u8; 32],
    /// The plan it was built from.
    pub plan: Plan,
    /// The running nodes, by name: the anchor and every infra member.
    pub nodes: BTreeMap<&'static str, Arc<LiveNode>>,
    /// Each party's path under the anchor, as child indices; the anchor's
    /// is empty.
    pub paths: BTreeMap<&'static str, Vec<u8>>,
    /// The records the subnet rests on, in signing order.
    pub records: Vec<Record>,
    /// The anchor's signed entry (`wire-format.md` §7.2): what an
    /// outsider knows this subnet by.
    pub entry: Vec<u8>,
}

impl Subnet {
    /// The parties that run infrastructure here: the anchor and the infra
    /// members.
    pub fn infra(&self) -> Vec<&'static str> {
        std::iter::once(self.anchor)
            .chain(self.plan.members.iter().filter(|m| m.infra).map(|m| m.name))
            .collect()
    }

    /// Who adopted `name`; none for the anchor.
    pub fn patron_of(&self, name: &str) -> Option<&'static str> {
        self.plan
            .members
            .iter()
            .find(|m| m.name == name)
            .map(|m| m.patron)
    }

    /// The infra node that serves `name`: itself where it runs
    /// infrastructure, its nearest infra ancestor otherwise (design
    /// §14.1.2).
    pub fn serving_node_of(&self, name: &str) -> &'static str {
        let infra = self.infra();
        let mut cur = self
            .plan
            .members
            .iter()
            .map(|m| m.name)
            .chain(std::iter::once(self.anchor))
            .find(|n| *n == name)
            .unwrap_or_else(|| panic!("{name} is not in the subnet under {}", self.anchor));
        loop {
            if infra.contains(&cur) {
                return cur;
            }
            cur = self
                .patron_of(cur)
                .unwrap_or_else(|| panic!("{cur} has no infra ancestor"));
        }
    }

    /// The running node called `name`.
    pub fn node(&self, name: &str) -> &Arc<LiveNode> {
        self.nodes
            .get(name)
            .unwrap_or_else(|| panic!("{name} runs no node under {}", self.anchor))
    }

    /// `name`'s locator here (`wire-format.md` §2.3).
    pub fn locator(&self, name: &str) -> Locator {
        let (anchor, path, nibbles) = self.position(name);
        Locator {
            anchor,
            path,
            nibbles,
            seqno: Seqno {
                series: 1,
                counter: 0,
            },
            subnet: Some(self.id),
        }
    }

    /// `name`'s position as a resolution request carries it: the anchor,
    /// the packed path and its nibble count.
    pub fn position(&self, name: &str) -> (Keyhash, Vec<u8>, u64) {
        let ix = self
            .paths
            .get(name)
            .unwrap_or_else(|| panic!("{name} has no position under {}", self.anchor));
        let p = Path::from_indices(ix);
        (kh(self.anchor), p.bytes, p.nibbles)
    }

    /// The anchor entry, parsed.
    pub fn anchor_entry(&self) -> AnchorEntry {
        AnchorEntry::parse(&self.entry).expect("the entry this fabric signed")
    }

    /// What an outsider asks the anchor with: its keyhash, its endpoints
    /// as the entry encodes them, and the material that binds it on a
    /// dial (`wire-format.md` §9.1).
    pub fn askable(&self) -> (Keyhash, Vec<Vec<u8>>, Option<Vec<u8>>) {
        let e = self.anchor_entry();
        (
            e.anchor,
            e.endpoints.iter().map(|p| p.encode_bytes()).collect(),
            Some(test_identity(self.anchor).public.key_material()),
        )
    }
}

/// Several subnets standing at once, signed from one clock.
pub struct Fabric {
    /// The subnets, in the order their plans were given.
    pub subnets: Vec<Subnet>,
    /// Every party of the fabric, the witness included.
    pub names: Vec<&'static str>,
    /// Every record by txid, which is what a table fetches presence from.
    pub store: BTreeMap<Txid, Vec<u8>>,
    /// The clock after the last record: what every node reads.
    pub clock: u64,
}

/// A party's test identity.
pub fn id(n: &str) -> SigningIdentity {
    test_identity(n)
}

/// A party's keyhash.
pub fn kh(n: &str) -> Keyhash {
    test_identity(n).public.keyhash
}

/// A party's `KeyMaterial`, which a counterparty pins.
pub fn material(n: &str) -> Vec<u8> {
    test_identity(n).public.key_material()
}

/// The point a loopback address is published as.
pub fn point(addr: SocketAddr) -> NetworkPoint {
    NetworkPoint::from_socket(addr).expect("a v4 loopback address")
}

/// One subnet signed and not yet started: its plan, its id, where each
/// party sits and the records that put them there.
struct Planned {
    plan: Plan,
    subnet: [u8; 32],
    paths: BTreeMap<&'static str, Vec<u8>>,
    records: Vec<Record>,
}

/// Records as the parties would sign them, each signer's archive advancing.
struct Signing {
    archives: BTreeMap<Keyhash, Archive>,
    store: BTreeMap<Txid, Vec<u8>>,
    clock: u64,
}

impl Signing {
    fn back(&mut self, n: &str) -> Vec<Txid> {
        self.archives
            .entry(kh(n))
            .or_insert_with(|| Archive::new(kh(n)))
            .next_back_pointers()
    }

    fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }

    fn commit(&mut self, tx_type: u64, body: &[u8], signers: &[&str]) -> Record {
        let sids: Vec<SigningIdentity> = signers.iter().map(|s| id(s)).collect();
        let refs: Vec<&SigningIdentity> = sids.iter().collect();
        let rec = Record::parse(&envelope(tx_type, body, &refs)).expect("well-formed");
        for s in signers {
            self.archives
                .entry(kh(s))
                .or_insert_with(|| Archive::new(kh(s)))
                .append(rec.clone())
                .expect("appends");
        }
        self.store.insert(rec.txid, rec.bytes.clone());
        rec
    }

    /// Presence between `a` and `b`: a formation where both keys are
    /// fresh, a witnessed record otherwise (`wire-format.md` §3.2).
    fn meet(&mut self, a: &str, b: &str) -> Record {
        let fresh = |s: &mut Signing, n: &str| s.archives.get(&kh(n)).is_none_or(|a| a.is_empty());
        let t = self.tick();
        if fresh(self, a) && fresh(self, b) {
            let (ba, bb) = (self.back(a), self.back(b));
            let root = rhtn_codec::cose::sha256(format!("f:{a}:{b}:{t}").as_bytes());
            let body = formation_body([&ba, &bb], [&kh(a), &kh(b)], t, t + 600, &root);
            return self.commit(TYPE_PRESENCE, &body, &[a, b]);
        }
        let back = vec![self.back(a), self.back(b), self.back(WITNESS)];
        let root = rhtn_codec::cose::sha256(format!("m:{a}:{b}:{t}").as_bytes());
        let w = Witness {
            keyhash: kh(WITNESS),
            nominated_by: kh(a),
            flags: 3,
        };
        let body = presence_record_body(&back, [&kh(a), &kh(b)], &[w], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b, WITNESS])
    }

    /// `node` adopted under `patron` at `path` in the subnet `anchor`
    /// roots, on presence between them.
    fn adopt(
        &mut self,
        node: &str,
        patron: &str,
        anchor: &str,
        path: &[u8],
        subnet: [u8; 32],
    ) -> Record {
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
                subnet: Some(subnet),
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
}

impl Fabric {
    /// Stand the subnets up: the records signed, each infra node's table
    /// folded from its subnet's records, the nodes started, every infra
    /// node's endpoint record held by itself and its patron, and each
    /// anchor's entry signed.  `interval` is the nodes' heartbeat interval
    /// in seconds.
    pub fn build(plans: Vec<Plan>, interval: u64) -> Fabric {
        let mut names: Vec<&'static str> = vec![WITNESS];
        for p in &plans {
            names.push(p.anchor);
            names.extend(p.members.iter().map(|m| m.name));
        }
        let mut seen = BTreeSet::new();
        for n in &names {
            assert!(seen.insert(*n), "{n} appears twice in the fabric");
        }
        let mut signing = Signing {
            archives: BTreeMap::new(),
            store: BTreeMap::new(),
            clock: EPOCH,
        };

        // the records, subnet by subnet, each member after the one before
        let mut built: Vec<Planned> = Vec::new();
        for plan in plans {
            let subnet = rhtn_codec::cose::sha256(format!("subnet:{}", plan.anchor).as_bytes());
            let mut paths: BTreeMap<&'static str, Vec<u8>> = BTreeMap::new();
            paths.insert(plan.anchor, Vec::new());
            let mut records = Vec::new();
            let infra: BTreeSet<&str> = std::iter::once(plan.anchor)
                .chain(plan.members.iter().filter(|m| m.infra).map(|m| m.name))
                .collect();
            for m in &plan.members {
                let mut path = paths
                    .get(m.patron)
                    .unwrap_or_else(|| panic!("{} adopted before its patron {}", m.name, m.patron))
                    .clone();
                path.push(m.index);
                if m.infra {
                    assert!(
                        infra.contains(m.patron),
                        "{} runs infrastructure under {}, which does not",
                        m.name,
                        m.patron
                    );
                }
                paths.insert(m.name, path.clone());
                records.push(signing.adopt(m.name, m.patron, plan.anchor, &path, subnet));
            }
            built.push(Planned {
                plan,
                subnet,
                paths,
                records,
            });
        }
        let clock = signing.clock + 60;
        let store = signing.store;
        let ids: Vec<Identity> = names.iter().map(|n| id(n).public).collect();
        let pins = Pins::new();
        for n in &names {
            pins.pin_identity(&id(n).public);
        }

        // the nodes, each from its own table
        let mut subnets = Vec::new();
        for Planned {
            plan,
            subnet,
            paths,
            records,
        } in built
        {
            let infra: Vec<&'static str> = std::iter::once(plan.anchor)
                .chain(plan.members.iter().filter(|m| m.infra).map(|m| m.name))
                .collect();
            let mut nodes = BTreeMap::new();
            let tmp = Subnet {
                anchor: plan.anchor,
                id: subnet,
                plan: plan.clone(),
                nodes: BTreeMap::new(),
                paths: paths.clone(),
                records: Vec::new(),
                entry: Vec::new(),
            };
            for n in &infra {
                let mut table = Table::with_me(kh(n));
                for i in &infra {
                    table.mark_infra(kh(i));
                }
                for r in &records {
                    table
                        .apply(r, &ids, &store, None)
                        .unwrap_or_else(|e| panic!("{n} applies {}: {e:?}", hex8(&r.txid)));
                }
                let mut view = NodeView::new(Arc::new(id(n)), tmp.locator(n));
                view.table = table;
                view.set_now(clock);
                for m in plan.members.iter().filter(|m| m.patron == *n) {
                    view.set_slot(m.index as u64, Some(kh(m.name)), clock);
                }
                // the serving node holds its whole light-client subtree
                // (design §12.6.1), not its children alone
                for m in plan.members.iter().filter(|m| !m.infra) {
                    if tmp.serving_node_of(m.name) == *n {
                        view.attached.insert(kh(m.name));
                    }
                }
                let mut cfg = NodeConfig::defaults(Arc::new(id(n)), pins.clone(), interval);
                cfg.clock = Arc::new(move || clock);
                cfg.log = Log::recording();
                let node = LiveNode::start(
                    cfg,
                    view,
                    ids.clone(),
                    AnchorTable::new(0, Ingestion::UnverifiedGossip),
                );
                nodes.insert(*n, node);
            }
            // every infra node's endpoint record, held by itself and by
            // its patron, which is what a referral is made from
            for n in &infra {
                let node = &nodes[n];
                let rec = endpoint_record(
                    &id(n),
                    &[point(node.addr)],
                    Seqno {
                        series: 1,
                        counter: 1,
                    },
                );
                let mut holders = vec![*n];
                if let Some(p) = tmp.patron_of(n) {
                    holders.push(p);
                }
                for h in holders {
                    let d = nodes[h].view.lock().unwrap().take_object(
                        &Quiet,
                        &kh(n),
                        KIND_ENDPOINT_RECORD,
                        &rec,
                        &ids,
                    );
                    assert!(
                        matches!(
                            d,
                            rhtn_node::store::Decision::Stored
                                | rhtn_node::store::Decision::Duplicate
                        ),
                        "{h} takes {n}'s endpoint record: {d:?}"
                    );
                }
            }
            let entry = anchor_entry(
                &id(plan.anchor),
                &[point(nodes[&plan.anchor].addr)],
                plan.members.len() as u64 + 1,
                Seqno {
                    series: 1,
                    counter: 1,
                },
            );
            subnets.push(Subnet {
                anchor: plan.anchor,
                id: subnet,
                plan,
                nodes,
                paths,
                records,
                entry,
            });
        }
        Fabric {
            subnets,
            names,
            store,
            clock,
        }
    }

    /// Every party's identity.
    pub fn ids(&self) -> Vec<Identity> {
        self.names.iter().map(|n| id(n).public).collect()
    }

    /// Every party pinned.
    pub fn pins(&self) -> Pins {
        let p = Pins::new();
        for n in &self.names {
            p.pin_identity(&id(n).public);
        }
        p
    }

    /// The subnet `name` sits in.
    pub fn subnet_of(&self, name: &str) -> &Subnet {
        self.subnets
            .iter()
            .find(|s| s.paths.contains_key(name))
            .unwrap_or_else(|| panic!("{name} sits in no subnet of the fabric"))
    }

    /// A client configuration for `name`, pinning every party and
    /// recording its sessions.
    pub fn client_cfg(&self, name: &str) -> ClientConfig {
        ClientConfig {
            me: Party::of(Arc::new(id(name))),
            pins: self.pins(),
            bind: Default::default(),
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

    /// A QUIC endpoint on loopback for a client to dial from.
    pub fn client_endpoint(&self) -> quinn::Endpoint {
        tls::client_endpoint("127.0.0.1:0".parse().unwrap()).expect("a loopback endpoint")
    }

    /// Attach `name` to the node that serves it, as a light client does
    /// (design §14.1.2), and hand back the session it collects on.
    pub async fn attach(&self, name: &str) -> Session {
        let subnet = self.subnet_of(name);
        let serving = subnet.serving_node_of(name);
        let node = subnet.node(serving);
        let cfg = self.client_cfg(name);
        cfg.addresses
            .lock()
            .unwrap()
            .entry(node.me())
            .or_default()
            .push(node.addr);
        match attach(&cfg, &self.client_endpoint(), node.me(), node.addr, false).await {
            AttachOutcome::Attached(s) => s,
            other => panic!("{name} attaches to {serving}: {other:?}"),
        }
    }

    /// Hand the anchor entry of subnet `foreign` to the node called
    /// `node`, as an introduction would (`wire-format.md` §7.2); whether
    /// its table took it.
    pub fn introduce(&self, node: &str, foreign: usize) -> bool {
        let entry = self.subnets[foreign].anchor_entry();
        let n = self.subnet_of(node).node(node);
        n.anchors.lock().unwrap().offer(entry, &self.ids())
    }
}

fn hex8(k: &[u8; 32]) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}
