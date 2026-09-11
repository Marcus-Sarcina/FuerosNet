//! The evidence an adoption carries, at the record level: what
//! `Record::parse` and `check_signatures` say about a transfer or recovery
//! whose embedded blocks are malformed or name the wrong parties.  The
//! byte-level cases are `rhtn-crypto`'s (DEC-19); these are the same
//! objects as a table would receive them.

mod common;

use common::World;
use rhtn_archive::record::SigStatus;
use rhtn_archive::tx::*;

// acceptance: DEC-19
#[test]
fn a_transfer_whose_former_patron_block_is_empty_is_not_verified() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    // carol vouches for alice's move to bob with no signature at all: the
    // node and patron sign the body, and the block is `[h'', {}, nil, []]`
    let empty = vec![0x84, 0x40, 0xa0, 0xf6, 0x80];
    let rec = w.adopt_with("alice", "bob", Evidence::Transfer { former: w.kh("carol"), block: empty }, 1, None);
    match rec.check_signatures(&w.lookup()) {
        SigStatus::Invalid(e) => assert!(e.contains("transfer statement"), "{e}"),
        other => panic!("an empty block verified: {other:?}"),
    }
    // the genuine statement is the control
    let block = transfer_block(w.id("carol"), &w.kh("alice"), &w.kh("bob"));
    let rec = w.adopt_with("alice", "bob", Evidence::Transfer { former: w.kh("carol"), block }, 2, None);
    assert_eq!(rec.check_signatures(&w.lookup()), SigStatus::Verified);
}

use rhtn_archive::genesis;
use rhtn_archive::record::Record;
use rhtn_codec::cbor::{map_without_key, value_slice};
use rhtn_codec::encode::*;
use rhtn_codec::schema::check_body;

/// An adoption body of `node` under `patron` on the world's back-pointers.
fn adoption_bytes(w: &World, node: &str, patron: &str, evidence: Evidence, series: u32) -> Vec<u8> {
    let (bn, bp) = (w.back(node), w.back(patron));
    adoption_body(&Adoption {
        node: w.kh(node),
        patron: w.kh(patron),
        locator: Locator { anchor: w.kh(patron), path: vec![0x10], nibbles: 2, seqno: Seqno { series, counter: 0 } },
        timestamp: w.clock + 1,
        key_material: None,
        evidence,
        presented_head: None,
        back: [&bn, &bp],
    })
}

/// What `Record::parse` says of `body` enveloped as `tx_type` by `signers`.
fn parsed(w: &World, tx_type: u64, body: &[u8], signers: &[&str]) -> Result<Record, String> {
    let sids: Vec<&rhtn_crypto::SigningIdentity> = signers.iter().map(|s| w.id(s)).collect();
    Record::parse(&envelope(tx_type, body, &sids))
}

/// `body` with the one-byte uint at `key` set to `v`.
fn with_uint(body: &[u8], key: u64, v: u8) -> Vec<u8> {
    let r = value_slice(body, key).unwrap();
    assert_eq!(r.len(), 1, "a one-byte uint at key {key}");
    let mut out = body.to_vec();
    out[r.start] = v;
    out
}

fn body_err(body: &[u8]) -> String {
    check_body(body, &rhtn_codec::cbor::parse_all(body).unwrap()).unwrap_err().0.to_string()
}

// acceptance: DEC-20
#[test]
fn an_adoption_carries_one_evidence_form_and_a_recovery_names_its_parties() {
    let mut w = World::new(&["alice", "bob", "carol", "alice2"]);
    let pop = w.meet("alice", "bob");
    // borrowed from the world: a signing identity carries its expanded
    // post-quantum key inline, and four of them by value outgrow a debug
    // build's test stack
    let (old, new, carol, bob) = (w.id("alice"), w.id("alice2"), w.id("carol"), w.id("bob"));
    let qid = |s: &str| rhtn_codec::cose::sha256(s.as_bytes());

    // (a) the presence form is the control; with field 8 removed nothing
    // stands as evidence
    let body = adoption_bytes(&w, "alice", "bob", Evidence::Presence(pop.txid), 1);
    assert!(parsed(&w, TYPE_ADOPTION, &body, &["alice", "bob"]).is_ok());
    let none = map_without_key(&body, 8).unwrap();
    assert!(parsed(&w, TYPE_ADOPTION, &none, &["alice", "bob"]).unwrap_err().contains("exactly one of fields 6, 8 and 9"), "no evidence");
    // (b) two forms at once: the presence record and a transfer statement
    let mut both = body.clone();
    both[0] += 1;
    emit_uint(&mut both, 9);
    emit_map_head(&mut both, 2);
    emit_uint(&mut both, 1);
    emit_bstr(&mut both, &carol.public.keyhash);
    emit_uint(&mut both, 2);
    both.extend_from_slice(&transfer_block(carol, &w.kh("alice"), &w.kh("bob")));
    assert!(parsed(&w, TYPE_ADOPTION, &both, &["alice", "bob"]).unwrap_err().contains("exactly one of fields 6, 8 and 9"), "two forms");

    // (c) the Recovery block: alice2 claims alice's history under bob, with
    // carol recognising her.  Every signature is genuine in every variant;
    // what each variant gets wrong is whom the evidence is about
    let good = recovery_response(carol, new, &qid("q1"), &old.public.keyhash);
    let recovery = |old: &rhtn_crypto::SigningIdentity, responses: Vec<Vec<u8>>| Evidence::Recovery(recovery_block(old, &new.public.keyhash, &bob.public.keyhash, responses));
    let control = adoption_bytes(&w, "alice2", "bob", recovery(old, vec![good.clone()]), 1);
    let rec = parsed(&w, TYPE_ADOPTION, &control, &["alice2", "bob"]).expect("the control parses");
    assert_eq!(rec.check_signatures(&w.lookup()), rhtn_archive::record::SigStatus::Verified);
    let refused = |ev: Evidence| parsed(&w, TYPE_ADOPTION, &adoption_bytes(&w, "alice2", "bob", ev, 1), &["alice2", "bob"]).unwrap_err();
    // a response attesting continuity to a third party
    let e = refused(recovery(old, vec![recovery_response(carol, bob, &qid("q2"), &old.public.keyhash)]));
    assert!(e.contains("subject other than the adopted node"), "{e}");
    // a response about another prior identity
    let e = refused(recovery(old, vec![recovery_response(carol, new, &qid("q3"), &bob.public.keyhash)]));
    assert!(e.contains("field 8 differs from the prior key"), "{e}");
    // the successor as its own verifier
    let e = refused(recovery(old, vec![recovery_response(new, new, &qid("q4"), &old.public.keyhash)]));
    assert!(e.contains("verifier is its subject"), "{e}");
    // no match among the responses
    let e = refused(recovery(old, vec![with_uint(&good, 4, 1)]));
    assert!(e.contains("at least one match"), "{e}");
    // a selection basis other than met
    let e = refused(recovery(old, vec![with_uint(&good, 10, 1)]));
    assert!(e.contains("selection basis is not met"), "{e}");
    // one verifier twice
    let e = refused(recovery(old, vec![good.clone(), recovery_response(carol, new, &qid("q5"), &old.public.keyhash)]));
    assert!(e.contains("unsorted or repeating a verifier"), "{e}");
    // no response at all
    let e = refused(recovery(old, vec![]));
    assert!(e.contains("at least one response"), "{e}");
    // the prior key is the new key: no rotation at all
    let e = refused(recovery(new, vec![recovery_response(carol, new, &qid("q6"), &new.public.keyhash)]));
    assert!(e.contains("prior key equals the new key"), "{e}");
}

// acceptance: DEC-21
#[test]
fn a_formation_is_genesis_rooted_and_a_normal_record_is_witnessed() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    let (ka, kb, kc) = (w.kh("alice"), w.kh("bob"), w.kh("carol"));
    let root = [7u8; 32];
    let t = 1_800_000_000u64;
    // the control: a formation on genesis pointers
    let f = formation_body([&[genesis(&ka)], &[genesis(&kb)]], [&ka, &kb], t, t + 600, &root);
    assert!(parsed(&w, TYPE_PRESENCE, &f, &["alice", "bob"]).is_ok());
    // (a) arbitrary hashes where the genesis value belongs
    let f_bad = formation_body([&[[1; 32]], &[[2; 32]]], [&ka, &kb], t, t + 600, &root);
    assert!(parsed(&w, TYPE_PRESENCE, &f_bad, &["alice", "bob"]).unwrap_err().contains("genesis"), "non-genesis formation");
    // (b) the world forms fresh keys once, and meets established ones on a
    // witnessed normal record; a formation for an established key fails
    let first = w.meet("alice", "bob");
    assert_eq!(first.field_uint(6), Some(1), "fresh keys form");
    let second = w.meet("alice", "carol");
    assert_eq!(second.field_uint(6), Some(0), "an established key meets on a normal record");
    assert_eq!(second.signers.len(), 3, "witnessed");
    let f2 = formation_body([&w.back("alice"), &w.back("carol")], [&ka, &kc], t, t + 600, &root);
    assert!(parsed(&w, TYPE_PRESENCE, &f2, &["alice", "carol"]).unwrap_err().contains("genesis"), "a second formation for alice");
    // (c) a normal record's subtype flipped to formation: it carries
    // witnesses, which a formation may not
    let normal = &second.bytes[second.body.clone()];
    assert!(body_err(&with_uint(normal, 6, 1)).contains("no witnesses"), "formation with witnesses");
    // (d) a formation's subtype flipped to normal: no witnesses, and a
    // normal record carries one to sixteen
    assert!(body_err(&with_uint(&f, 6, 0)).contains("carries witnesses"), "normal record without witnesses");
    // (e) the two participants one identity
    let same = formation_body([&[genesis(&ka)], &[genesis(&ka)]], [&ka, &ka], t, t + 600, &root);
    assert!(body_err(&same).contains("participant identities are one"), "equal participants");
}

// acceptance: DEC-22
#[test]
fn a_body_is_checked_against_the_type_its_envelope_names() {
    let mut w = World::new(&["alice", "bob"]);
    let pop = w.meet("alice", "bob");
    w.adopt("bob", "alice", pop.txid, 1);
    let (ka, kb) = (w.kh("alice"), w.kh("bob"));
    let t = w.clock + 1;
    // (a) a departure-shaped body: a departure as type 2, and malformed as
    // type 1, where shape alone would have let it through
    let dep = departure_body(&w.back("bob"), &kb, &ka, Seqno { series: 1, counter: 1 }, t, None);
    assert!(parsed(&w, TYPE_DEPARTURE, &dep, &["bob"]).is_ok());
    assert!(parsed(&w, TYPE_ADOPTION, &dep, &["bob", "alice"]).unwrap_err().contains("locator"), "a departure body in an adoption envelope");
    // (b) every two-party type refuses one identity in both roles
    let one = body_err(&departure_body(&w.back("bob"), &kb, &kb, Seqno { series: 1, counter: 1 }, t, None));
    assert!(one.contains("one identity"), "departure: {one}");
    let one = body_err(&disavowal_body(&w.back("alice"), &ka, &ka, t, None));
    assert!(one.contains("one identity"), "disavowal: {one}");
    let one = body_err(&reissue_body([&w.back("bob"), &w.back("bob")], &kb, &kb, Seqno { series: 1, counter: 3 }, 2, t));
    assert!(one.contains("one identity"), "reissue: {one}");
    let one = body_err(&adoption_bytes(&w, "bob", "bob", Evidence::Presence(pop.txid), 2));
    assert!(one.contains("one identity"), "adoption: {one}");
    // (c) a reissue whose new series does not open at counter 0
    let re = reissue_body([&w.back("bob"), &w.back("alice")], &kb, &ka, Seqno { series: 1, counter: 3 }, 2, t);
    assert!(parsed(&w, TYPE_REISSUE, &re, &["bob", "alice"]).is_ok());
    let r4 = value_slice(&re, 4).unwrap();
    let mut re1 = re[..r4.start].to_vec();
    re1.extend_from_slice(&[0x82, 0x02, 0x01]);
    re1.extend_from_slice(&re[r4.end..]);
    assert!(body_err(&re1).contains("counter not 0"), "reissue counter");
    // (d) a peering without the presence record between the peers
    let peering = |with_pop: bool| {
        let mut b = Vec::new();
        emit_map_head(&mut b, if with_pop { 7 } else { 6 });
        emit_back_pointers(&mut b, &[w.back("alice"), w.back("bob")]);
        emit_uint(&mut b, 1);
        emit_bstr(&mut b, &ka);
        emit_uint(&mut b, 2);
        emit_bstr(&mut b, &kb);
        for k in [3u64, 4] {
            emit_uint(&mut b, k);
            emit_map_head(&mut b, 1);
            emit_uint(&mut b, 1);
            emit_bstr(&mut b, &[127, 0, 0, k as u8]);
        }
        emit_uint(&mut b, 5);
        emit_uint(&mut b, t);
        if with_pop {
            emit_uint(&mut b, 8);
            emit_bstr(&mut b, &pop.txid);
        }
        b
    };
    assert!(parsed(&w, TYPE_PEERING, &peering(true), &["alice", "bob"]).is_ok());
    assert!(body_err(&peering(false)).contains("field 8"), "peering without evidence");
}

// acceptance: DEC-24
#[test]
fn nested_signed_structures_are_checked_inside_a_body() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    let pop = w.meet("alice", "bob");
    let (ka, kb, kc) = (w.kh("alice"), w.kh("bob"), w.kh("carol"));
    let t = w.clock + 1;
    // (a) an adoption whose locator claims one nibble over no bytes: the
    // packed-path invariant holds inside a signed body too
    let (bn, bp) = (w.back("alice"), w.back("bob"));
    let body = adoption_body(&Adoption {
        node: ka,
        patron: kb,
        locator: Locator { anchor: kb, path: Vec::new(), nibbles: 1, seqno: Seqno { series: 1, counter: 0 } },
        timestamp: t,
        key_material: None,
        evidence: Evidence::Presence(pop.txid),
        presented_head: None,
        back: [&bn, &bp],
    });
    let e = parsed(&w, TYPE_ADOPTION, &body, &["alice", "bob"]).unwrap_err();
    assert!(e.contains("byte length"), "{e}");
    // (b) a witness nominated by nobody who was there
    let root = [5u8; 32];
    let backs = vec![w.back("alice"), w.back("bob"), w.back("carol")];
    let stray = presence_record_body(&backs, [&ka, &kb], &[Witness { keyhash: kc, nominated_by: kc, flags: 3 }], t, t + 600, &root);
    let e = parsed(&w, TYPE_PRESENCE, &stray, &["alice", "bob", "carol"]).unwrap_err();
    assert!(e.contains("nominator"), "{e}");
    let nominated = presence_record_body(&backs, [&ka, &kb], &[Witness { keyhash: kc, nominated_by: kb, flags: 3 }], t, t + 600, &root);
    assert!(parsed(&w, TYPE_PRESENCE, &nominated, &["alice", "bob", "carol"]).is_ok(), "nominated by a participant");
    // (c) a peering whose network point carries a five-byte address
    let peering = |addr: &[u8]| {
        let mut b = Vec::new();
        emit_map_head(&mut b, 7);
        emit_back_pointers(&mut b, &[w.back("alice"), w.back("bob")]);
        emit_uint(&mut b, 1);
        emit_bstr(&mut b, &ka);
        emit_uint(&mut b, 2);
        emit_bstr(&mut b, &kb);
        emit_uint(&mut b, 3);
        emit_map_head(&mut b, 1);
        emit_uint(&mut b, 1);
        emit_bstr(&mut b, addr);
        emit_uint(&mut b, 4);
        emit_map_head(&mut b, 1);
        emit_uint(&mut b, 1);
        emit_bstr(&mut b, &[127, 0, 0, 2]);
        emit_uint(&mut b, 5);
        emit_uint(&mut b, t);
        emit_uint(&mut b, 8);
        emit_bstr(&mut b, &pop.txid);
        b
    };
    assert!(check_body(&peering(&[127, 0, 0, 1]), &rhtn_codec::cbor::parse_all(&peering(&[127, 0, 0, 1])).unwrap()).is_ok());
    assert!(body_err(&peering(&[127, 0, 0, 1, 9])).contains("address width"));
}

// acceptance: DEC-26
#[test]
fn a_missing_embedded_signers_key_is_the_third_outcome_and_names_the_key() {
    let mut w = World::new(&["alice", "bob", "carol", "alice2"]);
    let pop = w.meet("alice", "bob");
    let block = transfer_block(w.id("carol"), &w.kh("alice"), &w.kh("bob"));
    let xfer = w.adopt_with("alice", "bob", Evidence::Transfer { former: w.kh("carol"), block }, 1, None);
    let without = |name: &str| -> Vec<rhtn_crypto::Identity> { w.lookup().into_iter().filter(|i| i.keyhash != w.kh(name)).collect() };
    // the former patron's key absent: neither verified nor rejected, and
    // the identity to fetch is named
    assert_eq!(xfer.check_signatures(&without("carol")), SigStatus::Unverifiable { missing: w.kh("carol") });
    assert_eq!(xfer.check_signatures(&w.lookup()), SigStatus::Verified);
    // a recovery: the verifier's key, then the prior key, each named
    let (old, new, carol, bob) = (w.id("alice"), w.id("alice2"), w.id("carol"), w.id("bob"));
    let qid = rhtn_codec::cose::sha256(b"dec-26");
    let resp = recovery_response(carol, new, &qid, &old.public.keyhash);
    let rec = Evidence::Recovery(recovery_block(old, &new.public.keyhash, &bob.public.keyhash, vec![resp]));
    let body = adoption_bytes(&w, "alice2", "bob", rec, 2);
    let rec = parsed(&w, TYPE_ADOPTION, &body, &["alice2", "bob"]).unwrap();
    assert_eq!(rec.check_signatures(&without("carol")), SigStatus::Unverifiable { missing: w.kh("carol") });
    assert_eq!(rec.check_signatures(&without("alice")), SigStatus::Unverifiable { missing: w.kh("alice") });
    assert_eq!(rec.check_signatures(&w.lookup()), SigStatus::Verified);
    // a failing embedded signature under a held key is still a failure
    let mut bad = xfer.bytes.clone();
    let r9 = value_slice(&bad[xfer.body.clone()], 9).unwrap();
    let at = xfer.body.start + r9.end - 1;
    bad[at] ^= 1;
    let bad = Record::parse(&bad).unwrap();
    assert!(matches!(bad.check_signatures(&w.lookup()), SigStatus::Invalid(_)));
    let _ = pop;
}
