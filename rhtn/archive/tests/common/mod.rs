//! A small world for the archive and topology entries: named test
//! identities, one archive per key, and a shared record store every party
//! can fetch from.  Transactions are signed "through the archive
//! component": back-pointers come from each signer's archive and the signed
//! record is appended to each.

#![allow(dead_code)]

use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::tx::*;
use rhtn_archive::walk::Fetch;
use rhtn_archive::{Keyhash, Txid};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use std::collections::BTreeMap;

pub struct World {
    pub ids: BTreeMap<String, SigningIdentity>,
    pub archives: BTreeMap<Keyhash, Archive>,
    pub store: BTreeMap<Txid, Vec<u8>>,
    pub clock: u64,
}

impl Fetch for World {
    fn fetch(&self, txid: &Txid) -> Option<Vec<u8>> {
        self.store.get(txid).cloned()
    }
}

impl World {
    pub fn new(names: &[&str]) -> Self {
        let mut w = World { ids: BTreeMap::new(), archives: BTreeMap::new(), store: BTreeMap::new(), clock: 1_800_000_000 };
        for n in names.iter().chain(["witness"].iter()) {
            let id = test_identity(n);
            w.archives.insert(id.public.keyhash, Archive::new(id.public.keyhash));
            w.ids.insert(n.to_string(), id);
        }
        w
    }

    pub fn id(&self, n: &str) -> &SigningIdentity {
        &self.ids[n]
    }

    pub fn kh(&self, n: &str) -> Keyhash {
        self.ids[n].public.keyhash
    }

    pub fn lookup(&self) -> Vec<Identity> {
        self.ids.values().map(|i| i.public.clone()).collect()
    }

    pub fn archive(&self, n: &str) -> &Archive {
        &self.archives[&self.kh(n)]
    }

    pub fn archive_mut(&mut self, n: &str) -> &mut Archive {
        let k = self.kh(n);
        self.archives.get_mut(&k).unwrap()
    }

    pub fn head(&self, n: &str) -> Txid {
        let h = self.archive(n).heads();
        assert_eq!(h.len(), 1, "{n} has {} heads", h.len());
        h[0]
    }

    pub fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }

    /// Sign and take a record: envelope over `body` by `signers`, appended to
    /// each signer's archive and put in the store.
    pub fn commit(&mut self, tx_type: u64, body: &[u8], signers: &[&str]) -> Record {
        let sids: Vec<&SigningIdentity> = signers.iter().map(|s| &self.ids[*s]).collect();
        let env = envelope(tx_type, body, &sids);
        let rec = Record::parse(&env).expect("well-formed");
        for s in signers {
            let k = self.kh(s);
            self.archives.get_mut(&k).unwrap().append(rec.clone()).expect("appends");
        }
        self.store.insert(rec.txid, env);
        rec
    }

    pub fn back(&self, n: &str) -> Vec<Txid> {
        self.archive(n).next_back_pointers()
    }

    /// A formation record between two fresh keys (design §13.1).
    pub fn formation(&mut self, a: &str, b: &str) -> Record {
        let t = self.tick();
        let (ba, bb) = (self.back(a), self.back(b));
        let (ka, kb) = (self.kh(a), self.kh(b));
        let root = rhtn_codec::cose::sha256(b"formation: nothing disclosable");
        let body = formation_body([&ba, &bb], [&ka, &kb], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b])
    }

    /// Presence evidence between `a` and `b`: a formation record where both
    /// keys are fresh (design §13.1), and otherwise a normal record
    /// witnessed by the world's witness, since a key appears in at most
    /// one formation record, its first (`wire-format.md` §3.2).
    pub fn meet(&mut self, a: &str, b: &str) -> Record {
        if self.archive(a).is_empty() && self.archive(b).is_empty() {
            return self.formation(a, b);
        }
        let t = self.tick();
        let back = vec![self.back(a), self.back(b), self.back("witness")];
        let (ka, kb) = (self.kh(a), self.kh(b));
        let root = rhtn_codec::cose::sha256(format!("meeting:{a}:{b}:{t}").as_bytes());
        let w = Witness { keyhash: self.kh("witness"), nominated_by: ka, flags: 3 };
        let body = presence_record_body(&back, [&ka, &kb], &[w], t, t + 600, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b, "witness"])
    }

    /// An adoption of `node` under `patron` on presence evidence, opening
    /// `series` at counter 0.
    pub fn adopt(&mut self, node: &str, patron: &str, pop: Txid, series: u32) -> Record {
        self.adopt_with(node, patron, Evidence::Presence(pop), series, None)
    }

    pub fn adopt_with(&mut self, node: &str, patron: &str, evidence: Evidence, series: u32, presented_head: Option<Txid>) -> Record {
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let a = Adoption {
            node: self.kh(node),
            patron: self.kh(patron),
            locator: Locator { anchor: self.kh(patron), path: vec![0x10], nibbles: 2, seqno: Seqno { series, counter: 0 } },
            timestamp: t,
            key_material: None,
            evidence,
            presented_head,
            back: [&bn, &bp],
        };
        let body = adoption_body(&a);
        self.commit(TYPE_ADOPTION, &body, &[node, patron])
    }

    pub fn depart(&mut self, node: &str, patron: &str, seqno: Seqno) -> Record {
        let t = self.tick();
        let b = self.back(node);
        let body = departure_body(&b, &self.kh(node), &self.kh(patron), seqno, t, None);
        self.commit(TYPE_DEPARTURE, &body, &[node])
    }

    pub fn disavow(&mut self, patron: &str, node: &str, code: Option<u64>) -> Record {
        let t = self.tick();
        let b = self.back(patron);
        let body = disavowal_body(&b, &self.kh(patron), &self.kh(node), t, code);
        self.commit(TYPE_DISAVOWAL, &body, &[patron])
    }

    /// A disavowal stamped at `t` by the patron's own clock.
    pub fn disavow_at(&mut self, patron: &str, node: &str, code: Option<u64>, t: u64) -> Record {
        let b = self.back(patron);
        let body = disavowal_body(&b, &self.kh(patron), &self.kh(node), t, code);
        self.commit(TYPE_DISAVOWAL, &body, &[patron])
    }

    /// An adoption stamped at `t`, for slot-ordering tests.
    pub fn adopt_at(&mut self, node: &str, patron: &str, pop: Txid, series: u32, t: u64) -> Record {
        let (bn, bp) = (self.back(node), self.back(patron));
        let a = Adoption {
            node: self.kh(node),
            patron: self.kh(patron),
            locator: Locator { anchor: self.kh(patron), path: vec![0x10], nibbles: 2, seqno: Seqno { series, counter: 0 } },
            timestamp: t,
            key_material: None,
            evidence: Evidence::Presence(pop),
            presented_head: None,
            back: [&bn, &bp],
        };
        let body = adoption_body(&a);
        self.commit(TYPE_ADOPTION, &body, &[node, patron])
    }

    pub fn reissue(&mut self, node: &str, patron: &str, leaving: Seqno, new_series: u32) -> Record {
        let t = self.tick();
        let (bn, bp) = (self.back(node), self.back(patron));
        let body = reissue_body([&bn, &bp], &self.kh(node), &self.kh(patron), leaving, new_series, t);
        self.commit(TYPE_REISSUE, &body, &[node, patron])
    }

    /// A departure body signed and stored WITHOUT touching any archive, for
    /// records a test wants to hand to a verifier by itself.
    pub fn loose_departure(&mut self, node: &str, patron: &str, back: &[Txid], seqno: Seqno, t: u64) -> Record {
        let body = departure_body(back, &self.kh(node), &self.kh(patron), seqno, t, None);
        let env = envelope(TYPE_DEPARTURE, &body, &[&self.ids[node]]);
        let rec = Record::parse(&env).expect("well-formed");
        self.store.insert(rec.txid, env);
        rec
    }
}
