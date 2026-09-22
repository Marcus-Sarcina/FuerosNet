//! Shared helpers for the client's tests: the corpus fixtures and the test
//! identities the vectors were generated with, and a small world of
//! archives for signing presence records.

#![allow(dead_code)]

pub mod harness;

use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::tx::*;
use rhtn_archive::{Keyhash, Txid};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::{Identity, SigningIdentity};
use std::collections::BTreeMap;

pub const NAMES: [&str; 14] = [
    "alice", "bob", "carol", "alice2", "w1", "w2", "w3", "w4", "c1", "c2", "c3", "c4", "c5",
    "witness",
];

pub fn corpus() -> serde_json::Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../test-vectors/corpus.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub fn fixture(id: &str) -> Vec<u8> {
    let c = corpus();
    let e = c["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == id)
        .unwrap_or_else(|| panic!("no fixture {id}"));
    hex::decode(e["hex"].as_str().unwrap()).unwrap()
}

pub fn ids() -> Vec<Identity> {
    NAMES.iter().map(|n| test_identity(n).public).collect()
}

pub fn id(n: &str) -> SigningIdentity {
    test_identity(n)
}

pub fn kh(n: &str) -> Keyhash {
    test_identity(n).public.keyhash
}

/// The body an envelope carries.
pub fn body_of(env: &[u8]) -> Vec<u8> {
    let r = rhtn_codec::cbor::value_slice(env, 3).unwrap();
    env[r].to_vec()
}

/// Archives advancing as records are signed, the way every harness in the
/// workspace signs: back-pointers from each signer's archive, the record
/// appended to each.
pub struct World {
    pub archives: BTreeMap<Keyhash, Archive>,
    pub store: BTreeMap<Txid, Vec<u8>>,
    pub clock: u64,
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    pub fn new() -> Self {
        World {
            archives: NAMES.iter().map(|n| (kh(n), Archive::new(kh(n)))).collect(),
            store: BTreeMap::new(),
            clock: 1_790_000_000,
        }
    }

    pub fn tick(&mut self) -> u64 {
        self.clock += 3600;
        self.clock
    }

    pub fn back(&self, n: &str) -> Vec<Txid> {
        self.archives[&kh(n)].next_back_pointers()
    }

    pub fn commit(&mut self, tx_type: u64, body: &[u8], signers: &[&str]) -> Record {
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
        rec
    }

    /// Signed and returned without being parsed, for the tests whose
    /// subject is what a decoder refuses.  Nothing is appended: a record
    /// that does not parse has no place in anybody's archive.
    pub fn loose(&self, tx_type: u64, body: &[u8], signers: &[&str]) -> Vec<u8> {
        let sids: Vec<SigningIdentity> = signers.iter().map(|s| id(s)).collect();
        let refs: Vec<&SigningIdentity> = sids.iter().collect();
        envelope(tx_type, body, &refs)
    }

    /// A formation between two fresh keys, finalizing at `finalized`.
    pub fn formation_at(&mut self, a: &str, b: &str, started: u64, finalized: u64) -> Record {
        let (ba, bb) = (self.back(a), self.back(b));
        let root = rhtn_codec::cose::sha256(format!("f:{a}:{b}:{started}").as_bytes());
        let body = formation_body([&ba, &bb], [&kh(a), &kh(b)], started, finalized, &root);
        self.commit(TYPE_PRESENCE, &body, &[a, b])
    }

    /// A normal record between `a` and `b` witnessed by `w`, finalizing at
    /// `finalized`.
    pub fn normal_at(&mut self, a: &str, b: &str, w: &str, started: u64, finalized: u64) -> Record {
        let back = vec![self.back(a), self.back(b), self.back(w)];
        let root = rhtn_codec::cose::sha256(format!("n:{a}:{b}:{w}:{started}").as_bytes());
        let witness = Witness {
            keyhash: kh(w),
            nominated_by: kh(a),
            flags: 3,
        };
        let body = presence_record_body(
            &back,
            [&kh(a), &kh(b)],
            &[witness],
            started,
            finalized,
            &root,
        );
        self.commit(TYPE_PRESENCE, &body, &[a, b, w])
    }

    /// Presence evidence between `a` and `b` at the world's clock: a
    /// formation for fresh keys, otherwise a witnessed normal record.
    pub fn meet(&mut self, a: &str, b: &str) -> Record {
        let t = self.tick();
        if self.archives[&kh(a)].is_empty() && self.archives[&kh(b)].is_empty() {
            self.formation_at(a, b, t, t + 600)
        } else {
            self.normal_at(a, b, "witness", t, t + 600)
        }
    }
}
