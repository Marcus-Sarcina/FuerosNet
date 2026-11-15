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

/// The port a provisioned node serves on, the image's (`crates/packaging`).
pub const WAN_QUIC: u16 = 7447;

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
    /// The running nodes, by name: the anchor and every infra member;
    /// none for a remote fabric.
    pub nodes: BTreeMap<&'static str, Arc<LiveNode>>,
    /// Where each infra node is reached: the live node's socket, or the
    /// address a provisioned one was signed for.
    pub addresses: BTreeMap<&'static str, SocketAddr>,
    /// Each party's path under the anchor, as child indices; the anchor's
    /// is empty.
    pub paths: BTreeMap<&'static str, Vec<u8>>,
    /// The records the subnet rests on, in signing order.
    pub records: Vec<Record>,
    /// The anchor's signed entry (`wire-format.md` §7.2): what an
    /// outsider knows this subnet by.
    pub entry: Vec<u8>,
    /// Each infra node's signed endpoint record (`wire-format.md` §7.6),
    /// by name: what a client takes to have somewhere to start a descent
    /// from (design §12.3 Case 1).
    pub endpoint_records: BTreeMap<&'static str, Vec<u8>>,
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

    /// Where `name`'s node is reached, live or provisioned.
    pub fn addr_of(&self, name: &str) -> SocketAddr {
        *self
            .addresses
            .get(name)
            .unwrap_or_else(|| panic!("{name} has no address under {}", self.anchor))
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
    /// The foreign anchor entries each node was provisioned with, as
    /// `(node, subnet index)`: what `introduce` did in-process, done at
    /// provisioning for a node this process does not hold.
    pub introduced: Vec<(String, usize)>,
}

/// **The plan the WAN fabric runs, and the in-process tests with it**
/// [author, 2026-11-15]: two subnets that share no node.  Home: `alice2`
/// at the root, `bob` an infra node under it, `alice` and `w1` light
/// clients `bob` serves.  Far: `c1` at the root, `c2` under it, `c3` under
/// `c2`, `c4` a light client of `c3` and `c5` a light client under `c4`,
/// so `c3` serves its whole light subtree.  One plan for every harness,
/// so a claim proved in-process is the claim rerun on rented machines.
pub fn wan_plans() -> Vec<Plan> {
    vec![
        Plan::new("alice2")
            .infra("bob", "alice2", 1)
            .light("alice", "bob", 0)
            .light("w1", "bob", 1),
        Plan::new("c1")
            .infra("c2", "c1", 4)
            .infra("c3", "c2", 2)
            .light("c4", "c3", 1)
            .light("c5", "c4", 3),
    ]
}

/// What `Fabric::provision` wrote, for the launcher and for
/// `Fabric::remote`.
pub struct Provisioned {
    /// The manifest's path.
    pub manifest: std::path::PathBuf,
    /// Each infra node's state directory, by name.
    pub dirs: BTreeMap<&'static str, std::path::PathBuf>,
}

/// A subnet's anchor entry and its infra nodes' endpoint records, by name.
type Entries = (Vec<u8>, BTreeMap<&'static str, Vec<u8>>);

/// The signing stage: records, paths and the store, before any node
/// exists to hold them.
struct Signed {
    built: Vec<Planned>,
    names: Vec<&'static str>,
    store: BTreeMap<Txid, Vec<u8>>,
    /// Each party's own signed history: what a daemon reads its own
    /// position from after a restart (`adopt_own_position`), since the
    /// topology store keeps the seen fact and not the act.
    archives: BTreeMap<Keyhash, Archive>,
    clock: u64,
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
    /// Sign the plans' records from `epoch`, subnet by subnet, each
    /// member after the one before.
    fn sign(plans: Vec<Plan>, epoch: u64) -> Signed {
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
            clock: epoch,
        };
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
        Signed {
            built,
            names,
            clock: signing.clock + 60,
            store: signing.store,
            archives: signing.archives,
        }
    }

    /// The subnet as a skeleton: the plan and its paths, no nodes yet,
    /// for the helpers that read positions off it.
    fn skeleton(p: &Planned) -> Subnet {
        Subnet {
            anchor: p.plan.anchor,
            id: p.subnet,
            plan: p.plan.clone(),
            nodes: BTreeMap::new(),
            addresses: BTreeMap::new(),
            paths: p.paths.clone(),
            records: Vec::new(),
            entry: Vec::new(),
            endpoint_records: BTreeMap::new(),
        }
    }

    /// The infra nodes of a planned subnet, the anchor first.
    fn infra_of(p: &Planned) -> Vec<&'static str> {
        std::iter::once(p.plan.anchor)
            .chain(p.plan.members.iter().filter(|m| m.infra).map(|m| m.name))
            .collect()
    }

    /// One infra node's view of its subnet: the table folded from the
    /// records, the slots its subordinates occupy, and the light clients
    /// it serves, which are its whole light subtree (design §12.6.1).
    fn view_for(
        p: &Planned,
        n: &str,
        ids: &[Identity],
        store: &BTreeMap<Txid, Vec<u8>>,
        clock: u64,
    ) -> NodeView {
        let tmp = Self::skeleton(p);
        let mut table = Table::with_me(kh(n));
        for i in Self::infra_of(p) {
            table.mark_infra(kh(i));
        }
        for r in &p.records {
            table
                .apply(r, ids, store, None)
                .unwrap_or_else(|e| panic!("{n} applies {}: {e:?}", hex8(&r.txid)));
        }
        let mut view = NodeView::new(Arc::new(id(n)), tmp.locator(n));
        view.table = table;
        view.set_now(clock);
        for m in p.plan.members.iter().filter(|m| m.patron == n) {
            view.set_slot(m.index as u64, Some(kh(m.name)), clock);
        }
        for m in p.plan.members.iter().filter(|m| !m.infra) {
            if tmp.serving_node_of(m.name) == n {
                view.attached.insert(kh(m.name));
            }
        }
        view
    }

    /// Every infra node's endpoint record at the address it is reached
    /// at, and the anchor's entry.
    fn records_at(
        p: &Planned,
        addrs: &BTreeMap<&'static str, SocketAddr>,
    ) -> (BTreeMap<&'static str, Vec<u8>>, Vec<u8>) {
        let mut endpoint_records = BTreeMap::new();
        for n in Self::infra_of(p) {
            let rec = endpoint_record(
                &id(n),
                &[point(addrs[n])],
                Seqno {
                    series: 1,
                    counter: 1,
                },
            );
            endpoint_records.insert(n, rec);
        }
        let entry = anchor_entry(
            &id(p.plan.anchor),
            &[point(addrs[p.plan.anchor])],
            p.plan.members.len() as u64 + 1,
            Seqno {
                series: 1,
                counter: 1,
            },
        );
        (endpoint_records, entry)
    }

    /// The endpoint records `holder` keeps: its own and its infra
    /// children's, which is what a referral is made from.
    fn take_records(
        view: &mut NodeView,
        holder: &str,
        p: &Planned,
        records: &BTreeMap<&'static str, Vec<u8>>,
        ids: &[Identity],
    ) {
        let tmp = Self::skeleton(p);
        for (n, rec) in records {
            if *n != holder && tmp.patron_of(n) != Some(holder) {
                continue;
            }
            let d = view.take_object(&Quiet, &kh(n), KIND_ENDPOINT_RECORD, rec, ids);
            assert!(
                matches!(
                    d,
                    rhtn_node::store::Decision::Stored | rhtn_node::store::Decision::Duplicate
                ),
                "{holder} takes {n}'s endpoint record: {d:?}"
            );
        }
    }

    /// The fabric in this process: every infra node a `LiveNode` on a
    /// loopback socket, as the automated tests run it.
    pub fn build(plans: Vec<Plan>, interval: u64) -> Fabric {
        let Signed {
            built,
            names,
            store,
            clock,
            archives: _,
        } = Self::sign(plans, EPOCH);
        let ids: Vec<Identity> = names.iter().map(|n| id(n).public).collect();
        let pins = Pins::new();
        for n in &names {
            pins.pin_identity(&id(n).public);
        }
        let mut subnets = Vec::new();
        for p in built {
            let infra = Self::infra_of(&p);
            let tmp = Self::skeleton(&p);
            let mut nodes = BTreeMap::new();
            for n in &infra {
                let view = Self::view_for(&p, n, &ids, &store, clock);
                let mut cfg = NodeConfig::defaults(Arc::new(id(n)), pins.clone(), interval);
                cfg.clock = Arc::new(move || clock);
                cfg.log = Log::recording();
                // **a node queues for the clients it serves and refuses the
                // rest** (`infra-client-requirements.md` §2): the light
                // members under it, and its own operator
                let served: BTreeSet<Keyhash> = p
                    .plan
                    .members
                    .iter()
                    .filter(|m| !m.infra && tmp.serving_node_of(m.name) == *n)
                    .map(|m| kh(m.name))
                    .chain(std::iter::once(kh(n)))
                    .collect();
                cfg.serves = Arc::new(move |k| served.contains(k));
                let node = LiveNode::start(
                    cfg,
                    view,
                    ids.clone(),
                    AnchorTable::new(0, Ingestion::UnverifiedGossip),
                );
                nodes.insert(*n, node);
            }
            let addresses: BTreeMap<&'static str, SocketAddr> =
                nodes.iter().map(|(n, node)| (*n, node.addr)).collect();
            let (endpoint_records, entry) = Self::records_at(&p, &addresses);
            for h in &infra {
                let mut view = nodes[h].view.lock().unwrap();
                Self::take_records(&mut view, h, &p, &endpoint_records, &ids);
            }
            let Planned {
                plan,
                subnet,
                paths,
                records,
            } = p;
            subnets.push(Subnet {
                anchor: plan.anchor,
                id: subnet,
                plan,
                nodes,
                addresses,
                paths,
                records,
                entry,
                endpoint_records,
            });
        }
        Fabric {
            subnets,
            names,
            store,
            clock,
            introduced: Vec::new(),
        }
    }

    /// **The fabric as state directories for `rhtnd`** [author,
    /// 2026-11-15]: the same plan signed against the addresses given, one
    /// directory per infra node under `out`, each holding what the daemon
    /// reads at start — its identity, the peers it knows, its topology
    /// store with the derived table and slots written beside it, its own
    /// endpoint record, the anchor's entry at the anchor, and the other
    /// subnet's entry in the `anchors` directory for every node
    /// `introduced` names.  Paths in each `rhtnd.conf` are rooted at
    /// `root(name)` (the mount point on a rented machine, the directory
    /// itself for a process here), and `listen(name)` is the bind.
    ///
    /// Records are signed from a day ago on the real clock, since the
    /// daemons run on it; a kernel driving the fabric runs on the clock
    /// the manifest states.  The manifest beside the directories carries
    /// the records, the entries and the addresses, so a test process that
    /// never signed them rebuilds the same fabric with [`Fabric::remote`].
    pub fn provision(
        plans: Vec<Plan>,
        addrs: &BTreeMap<&'static str, SocketAddr>,
        introduced: &[(&str, usize)],
        out: &std::path::Path,
        root: impl Fn(&str) -> String,
        listen: impl Fn(&str) -> String,
    ) -> Provisioned {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after 1970")
            .as_secs();
        let epoch = now - 86_400;
        let Signed {
            built,
            names,
            store,
            clock,
            mut archives,
        } = Self::sign(plans, epoch);
        let ids: Vec<Identity> = names.iter().map(|n| id(n).public).collect();
        let peers: String = names
            .iter()
            .map(|n| format!("{}\n", crate::daemons::hex(&material(n))))
            .collect();
        let entries: Vec<Entries> = built
            .iter()
            .map(|p| Self::records_at(p, addrs))
            .map(|(r, e)| (e, r))
            .collect();
        let mut dirs = BTreeMap::new();
        let mut manifest = serde_json::Map::new();
        manifest.insert("epoch".into(), clock.into());
        let mut subnets = Vec::new();
        for (i, p) in built.iter().enumerate() {
            let (entry, endpoint_records) = &entries[i];
            let tmp = Self::skeleton(p);
            for n in Self::infra_of(p) {
                let dir = out.join(n);
                let _ = std::fs::remove_dir_all(&dir);
                std::fs::create_dir_all(dir.join("anchors")).expect("the state directory");
                crate::daemons::write_identity(&dir.join("identity.key"), n);
                std::fs::write(dir.join("peers"), &peers).expect("the peers file");
                let mut view = Self::view_for(p, n, &ids, &store, clock);
                Self::take_records(&mut view, n, p, endpoint_records, &ids);
                // its own signed history: the adoption it reads its
                // position from at start, and its children's
                if let Some(a) = archives.remove(&kh(n)) {
                    view.archive = a;
                }
                view.archive
                    .save(&dir.join("archive"))
                    .expect("the archive");
                view.store
                    .save(&dir.join("topology"))
                    .expect("the topology store");
                std::fs::write(
                    dir.join("topology").join("derived"),
                    view.materialise().encode(),
                )
                .expect("the derived state");
                std::fs::write(dir.join("endpoint-record"), &endpoint_records[n])
                    .expect("the endpoint record");
                let is_anchor = n == p.plan.anchor;
                if is_anchor {
                    std::fs::write(dir.join("anchor-entry"), entry).expect("the anchor entry");
                }
                for (node, foreign) in introduced {
                    if *node == n {
                        let (e, _) = &entries[*foreign];
                        std::fs::write(dir.join("anchors").join(built[*foreign].plan.anchor), e)
                            .expect("a foreign anchor entry");
                    }
                }
                let upstream = tmp.patron_of(n).map(|patron| (kh(patron), addrs[patron]));
                let r = root(n);
                let mut conf = format!(
                    "identity = \"{r}/identity.key\"\nlisten = \"{}\"\nqueue = \"{r}/queue\"\nprekeys = \"{r}/prekeys\"\ntopology = \"{r}/topology\"\narchive = \"{r}/archive\"\nheartbeat = 30\ningestion = \"unverified-gossip\"\nendpoint-record = \"{r}/endpoint-record\"\nanchors = \"{r}/anchors\"\n",
                    listen(n)
                );
                if is_anchor {
                    conf.push_str(&format!("anchor-entry = \"{r}/anchor-entry\"\n"));
                }
                conf.push_str("\n[allowance]\nrequests = 120\nseconds = 60\n");
                if let Some((patron, addr)) = upstream {
                    conf.push_str(&format!(
                        "\n[upstream]\nnode = \"{}\"\naddresses = [\"{addr}\"]\n",
                        crate::daemons::hex(&patron)
                    ));
                }
                conf.push_str(&format!(
                    "\n[log]\npath = \"{r}/diag.jsonl\"\nlevel = \"debug\"\n"
                ));
                std::fs::write(dir.join("rhtnd.conf"), conf).expect("the configuration");
                dirs.insert(n, dir);
            }
            let mut sub = serde_json::Map::new();
            sub.insert("anchor".into(), p.plan.anchor.into());
            sub.insert(
                "records".into(),
                p.records
                    .iter()
                    .map(|r| serde_json::Value::from(crate::daemons::hex(&r.bytes)))
                    .collect::<Vec<_>>()
                    .into(),
            );
            sub.insert("entry".into(), crate::daemons::hex(entry).into());
            sub.insert(
                "endpoint_records".into(),
                endpoint_records
                    .iter()
                    .map(|(n, b)| {
                        (
                            (*n).to_string(),
                            serde_json::Value::from(crate::daemons::hex(b)),
                        )
                    })
                    .collect::<serde_json::Map<_, _>>()
                    .into(),
            );
            sub.insert(
                "addresses".into(),
                Self::infra_of(p)
                    .iter()
                    .map(|n| {
                        (
                            (*n).to_string(),
                            serde_json::Value::from(addrs[n].to_string()),
                        )
                    })
                    .collect::<serde_json::Map<_, _>>()
                    .into(),
            );
            subnets.push(serde_json::Value::Object(sub));
        }
        manifest.insert("subnets".into(), subnets.into());
        manifest.insert(
            "store".into(),
            store
                .iter()
                .map(|(t, b)| {
                    (
                        crate::daemons::hex(t),
                        serde_json::Value::from(crate::daemons::hex(b)),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
                .into(),
        );
        manifest.insert(
            "introduced".into(),
            introduced
                .iter()
                .map(|(n, i)| serde_json::json!([n, i]))
                .collect::<Vec<_>>()
                .into(),
        );
        let path = out.join("manifest.json");
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&serde_json::Value::Object(manifest)).expect("json"),
        )
        .expect("the manifest");
        Provisioned {
            manifest: path,
            dirs,
        }
    }

    /// **The fabric as a manifest describes it**: the plans this process
    /// knows, the records, entries and addresses that `provision` wrote.
    /// No node is held; every address is somewhere else.
    pub fn remote(plans: Vec<Plan>, manifest: &std::path::Path) -> Fabric {
        let text = std::fs::read_to_string(manifest)
            .unwrap_or_else(|e| panic!("{}: {e}", manifest.display()));
        let m: serde_json::Value = serde_json::from_str(&text).expect("a manifest");
        let unhex = |s: &str| -> Vec<u8> {
            (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
                .collect()
        };
        let mut names: Vec<&'static str> = vec![WITNESS];
        for p in &plans {
            names.push(p.anchor);
            names.extend(p.members.iter().map(|m| m.name));
        }
        let mut subnets = Vec::new();
        for (i, plan) in plans.into_iter().enumerate() {
            let sub = &m["subnets"][i];
            assert_eq!(
                sub["anchor"].as_str(),
                Some(plan.anchor),
                "the manifest's subnet {i} is not {}'s",
                plan.anchor
            );
            let subnet = rhtn_codec::cose::sha256(format!("subnet:{}", plan.anchor).as_bytes());
            let mut paths: BTreeMap<&'static str, Vec<u8>> = BTreeMap::new();
            paths.insert(plan.anchor, Vec::new());
            for mem in &plan.members {
                let mut path = paths[mem.patron].clone();
                path.push(mem.index);
                paths.insert(mem.name, path);
            }
            let records: Vec<Record> = sub["records"]
                .as_array()
                .expect("records")
                .iter()
                .map(|r| Record::parse(&unhex(r.as_str().expect("hex"))).expect("a record"))
                .collect();
            let lookup = |n: &str| -> &'static str {
                names
                    .iter()
                    .find(|k| **k == n)
                    .copied()
                    .unwrap_or_else(|| panic!("{n} is not planned"))
            };
            let endpoint_records = sub["endpoint_records"]
                .as_object()
                .expect("endpoint records")
                .iter()
                .map(|(n, b)| (lookup(n), unhex(b.as_str().expect("hex"))))
                .collect();
            let addresses = sub["addresses"]
                .as_object()
                .expect("addresses")
                .iter()
                .map(|(n, a)| {
                    (
                        lookup(n),
                        a.as_str().expect("addr").parse().expect("an address"),
                    )
                })
                .collect();
            subnets.push(Subnet {
                anchor: plan.anchor,
                id: subnet,
                plan,
                nodes: BTreeMap::new(),
                addresses,
                paths,
                records,
                entry: unhex(sub["entry"].as_str().expect("entry")),
                endpoint_records,
            });
        }
        let store = m["store"]
            .as_object()
            .expect("store")
            .iter()
            .map(|(t, b)| {
                let txid: Txid = unhex(t).try_into().expect("a txid");
                (txid, unhex(b.as_str().expect("hex")))
            })
            .collect();
        let introduced = m["introduced"]
            .as_array()
            .expect("introduced")
            .iter()
            .map(|e| {
                (
                    e[0].as_str().expect("a node").to_string(),
                    e[1].as_u64().expect("an index") as usize,
                )
            })
            .collect();
        Fabric {
            subnets,
            names,
            store,
            clock: m["epoch"].as_u64().expect("epoch"),
            introduced,
        }
    }

    /// Whether this fabric's nodes are somewhere else.
    pub fn is_remote(&self) -> bool {
        self.subnets.iter().all(|s| s.nodes.is_empty())
    }

    /// The fabric the tests run: in this process, or the one a manifest
    /// named by `RHTN_WAN_MANIFEST` describes, with the nodes named in
    /// `introduced` holding the other subnet's anchor entry either way.
    pub fn for_tests(plans: Vec<Plan>, interval: u64, introduced: &[(&str, usize)]) -> Fabric {
        match std::env::var_os("RHTN_WAN_MANIFEST") {
            Some(p) => {
                let f = Self::remote(plans, std::path::Path::new(&p));
                for (n, i) in introduced {
                    assert!(
                        f.introduce(n, *i),
                        "the manifest's fabric was not provisioned with {n} holding subnet {i}'s entry"
                    );
                }
                f
            }
            None => {
                let f = Self::build(plans, interval);
                for (n, i) in introduced {
                    assert!(f.introduce(n, *i), "{n}'s table takes subnet {i}'s entry");
                }
                f
            }
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
        // unspecified, so a remote fabric is reachable and a loopback one
        // still is
        tls::client_endpoint("0.0.0.0:0".parse().unwrap()).expect("a client endpoint")
    }

    /// Attach `name` to the node that serves it, as a light client does
    /// (design §14.1.2), and hand back the session it collects on.
    pub async fn attach(&self, name: &str) -> Session {
        let subnet = self.subnet_of(name);
        let serving = subnet.serving_node_of(name);
        let addr = subnet.addr_of(serving);
        let cfg = self.client_cfg(name);
        cfg.addresses
            .lock()
            .unwrap()
            .entry(kh(serving))
            .or_default()
            .push(addr);
        match attach(&cfg, &self.client_endpoint(), kh(serving), addr, false).await {
            AttachOutcome::Attached(s) => s,
            other => panic!("{name} attaches to {serving}: {other:?}"),
        }
    }

    /// Hand the anchor entry of subnet `foreign` to the node called
    /// `node`, as an introduction would (`wire-format.md` §7.2); whether
    /// its table took it.
    pub fn introduce(&self, node: &str, foreign: usize) -> bool {
        if self.is_remote() {
            return self
                .introduced
                .iter()
                .any(|(n, i)| n == node && *i == foreign);
        }
        let entry = self.subnets[foreign].anchor_entry();
        let n = self.subnet_of(node).node(node);
        n.anchors.lock().unwrap().offer(entry, &self.ids())
    }
}

fn hex8(k: &[u8; 32]) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}
