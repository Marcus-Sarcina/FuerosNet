//! The prekey service at the serving node (`wire-format.md` §7.8;
//! `infra-client-requirements.md` §6): what it serves freely, what it
//! consumes, what it bounds, what it tells, and what it never keeps.

mod common;

use common::*;
use rhtn_node::prekeys::*;
use std::collections::BTreeSet;

fn fixture(id: &str) -> Vec<u8> {
    let c: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/corpus.json")).unwrap()).unwrap();
    let e = c["entries"].as_array().unwrap().iter().find(|e| e["id"] == id).unwrap();
    hex::decode(e["hex"].as_str().unwrap()).unwrap()
}

fn bundle_for(name: &str, blob: &[u8], at: u64) -> Vec<u8> {
    PrekeyBundle::build(&id(name), CONSTRUCTION_PQXDH, blob, at)
}

fn one_time_reply(svc: &mut PrekeyService, requester: &str, subject: &str, n: u8, now: u64) -> PrekeyReply {
    let req = PrekeyRequest::One { subject: kh(subject), one_time: true, nonce: [n; 16] }.encode();
    PrekeyReply::decode(&svc.answer(&kh(requester), &req, now).expect("a reply")).unwrap()
}

fn stocked(subject: &str, n: usize) -> PrekeyService {
    let mut svc = PrekeyService::new(PrekeyConfig { one_time_per_requester_per_subject: 4, window_s: 3600 });
    svc.publish(&ids(), &bundle_for(subject, b"reusable material", 1_800_000_000)).unwrap();
    svc.stock(kh(subject), (0..n).map(|i| format!("otk-{i}").into_bytes()).collect());
    svc
}

#[test]
fn the_corpus_prekey_objects_read_here() {
    let b = PrekeyBundle::parse(&fixture("P-prekey")).unwrap();
    assert_eq!((b.subject, b.construction), (kh("alice"), CONSTRUCTION_PQXDH));
    assert_eq!(b.verify(&ids()), Ok(true));
    let wrong = PrekeyBundle::parse(&fixture("N-wrong-signer-prekey")).unwrap();
    assert_ne!(wrong.verify(&ids()), Ok(true), "signed by a key the bundle does not name");
    let f11 = fixture("P-frame-11");
    let f12 = fixture("P-frame-12");
    let body = |f: &[u8]| {
        let p = &f[4..];
        let outer = rhtn_codec::cbor::array_item_ranges(p, 0).unwrap();
        p[outer[1].clone()].to_vec()
    };
    let one = PrekeyRequest::decode(&body(&f11)).unwrap();
    assert!(matches!(one, PrekeyRequest::One { one_time: true, .. }));
    let batch = PrekeyRequest::decode(&body(&f12)).unwrap();
    let PrekeyRequest::Batch { subjects, .. } = &batch else { panic!() };
    assert_eq!(subjects.len(), 2);
    assert!(subjects[0] < subjects[1]);
    assert_eq!(one.encode(), body(&f11));
    assert_eq!(batch.encode(), body(&f12));
    let r5 = PrekeyReply::decode(&fixture("P-reply-05")).unwrap();
    assert!(r5.bundle.is_some() && r5.one_time.is_some() && r5.code.is_none());
    assert_eq!(r5.encode(), fixture("P-reply-05"));
    let r6 = PrekeyReply::decode(&fixture("P-reply-06")).unwrap();
    assert_eq!((r6.bundle.is_none(), r6.code), (true, Some(FAIL_UNKNOWN_SUBJECT)));
    assert_eq!(r6.encode(), fixture("P-reply-06"));
}

// acceptance: PAY-02
#[test]
fn a_one_time_key_is_served_once_and_never_again() {
    let mut svc = stocked("alice", 3);
    let a = one_time_reply(&mut svc, "bob", "alice", 1, 0);
    let b = one_time_reply(&mut svc, "carol", "alice", 2, 0);
    let (ka, kb) = (a.one_time.unwrap(), b.one_time.unwrap());
    assert_ne!(ka, kb, "different keys");
    assert_eq!(svc.pool_size(&kh("alice")), 1, "the pool shrinks by two");
    // every later reply carries a key that is neither
    let mut served = BTreeSet::from([ka, kb]);
    let c = one_time_reply(&mut svc, "w1", "alice", 3, 0);
    assert!(served.insert(c.one_time.unwrap()), "never served again");
    assert_eq!(svc.pool_size(&kh("alice")), 0);
    let d = one_time_reply(&mut svc, "w2", "alice", 4, 0);
    assert!(d.one_time.is_none() && d.bundle.is_some(), "none remain: reusable material alone");
}

// acceptance: PAY-04
#[test]
fn reusable_material_is_served_freely_and_consumes_nothing() {
    let mut svc = stocked("alice", 3);
    for (who, n) in [("bob", 1u8), ("carol", 2)] {
        let req = PrekeyRequest::One { subject: kh("alice"), one_time: false, nonce: [n; 16] }.encode();
        let r = PrekeyReply::decode(&svc.answer(&kh(who), &req, 0).unwrap()).unwrap();
        assert_eq!(r.bundle.as_deref(), svc.bundle(&kh("alice")).map(|b| b.as_slice()));
        assert!(r.one_time.is_none(), "no field 3");
        assert_eq!(r.nonce, [n; 16]);
    }
    assert_eq!(svc.pool_size(&kh("alice")), 3, "the pool is unchanged");
    // and a sweep the same, for every subject named, in order
    svc.publish(&ids(), &bundle_for("carol", b"carol's", 1_800_000_000)).unwrap();
    let mut subjects = vec![kh("alice"), kh("carol"), kh("w9")];
    subjects.sort();
    let req = PrekeyRequest::Batch { subjects: subjects.clone(), nonce: [7; 16] }.encode();
    let replies = decode_batch_reply(&svc.answer(&kh("bob"), &req, 0).unwrap()).unwrap();
    assert_eq!(replies.len(), 3);
    for (r, s) in replies.iter().zip(&subjects) {
        assert_eq!(r.nonce, [7; 16]);
        assert!(r.one_time.is_none());
        if *s == kh("w9") {
            assert_eq!(r.code, Some(FAIL_UNKNOWN_SUBJECT));
        } else {
            assert_eq!(PrekeyBundle::parse(r.bundle.as_ref().unwrap()).unwrap().subject, *s);
        }
    }
    assert_eq!(svc.pool_size(&kh("alice")), 3);
}

// acceptance: PAY-05
#[test]
fn one_time_issuance_is_limited_per_requester_per_subject() {
    let mut svc = PrekeyService::new(PrekeyConfig { one_time_per_requester_per_subject: 2, window_s: 3600 });
    for s in ["alice", "carol"] {
        svc.publish(&ids(), &bundle_for(s, b"m", 1_800_000_000)).unwrap();
        svc.stock(kh(s), (0..5).map(|i| vec![i]).collect());
    }
    // L = 2: the first two carry keys, the third is served no one-time key
    assert!(one_time_reply(&mut svc, "bob", "alice", 1, 100).one_time.is_some());
    assert!(one_time_reply(&mut svc, "bob", "alice", 2, 101).one_time.is_some());
    let excess = one_time_reply(&mut svc, "bob", "alice", 3, 102);
    assert!(excess.one_time.is_none() && excess.bundle.is_some());
    assert_eq!(svc.pool_size(&kh("alice")), 3, "nothing spent on the excess");
    // another subject: its own allowance
    assert!(one_time_reply(&mut svc, "bob", "carol", 4, 103).one_time.is_some());
    // another requester: its own allowance for alice
    assert!(one_time_reply(&mut svc, "w1", "alice", 5, 104).one_time.is_some());
    // the window passes: the allowance is back
    assert!(one_time_reply(&mut svc, "bob", "alice", 6, 100 + 3600).one_time.is_some());
}

// acceptance: PAY-06
#[test]
fn the_subject_is_told_when_its_pool_is_exhausted() {
    let mut svc = stocked("alice", 2);
    assert!(svc.take_exhausted().is_empty());
    one_time_reply(&mut svc, "bob", "alice", 1, 0);
    assert!(svc.take_exhausted().is_empty(), "one left");
    one_time_reply(&mut svc, "carol", "alice", 2, 0);
    assert_eq!(svc.take_exhausted(), vec![kh("alice")], "the last key served: the subject is told");
    assert!(svc.take_exhausted().is_empty(), "told once");
    // the subject replenishes, and the next drain tells again
    svc.stock(kh("alice"), vec![b"fresh".to_vec()]);
    one_time_reply(&mut svc, "w1", "alice", 3, 0);
    assert_eq!(svc.take_exhausted(), vec![kh("alice")]);
}

// acceptance: PAY-09
#[test]
fn nothing_persisted_says_who_asked_for_whose_bundle() {
    let mut svc = stocked("alice", 3);
    svc.publish(&ids(), &bundle_for("carol", b"c", 1_800_000_000)).unwrap();
    one_time_reply(&mut svc, "bob", "alice", 1, 0);
    one_time_reply(&mut svc, "w1", "carol", 2, 0);
    let mut subjects = vec![kh("alice"), kh("carol")];
    subjects.sort();
    svc.answer(&kh("w2"), &PrekeyRequest::Batch { subjects, nonce: [3; 16] }.encode(), 0);
    let dir = std::env::temp_dir().join(format!("rhtn-prekeys-{}", std::process::id()));
    svc.save(&dir).unwrap();
    // every byte on disk: no requester keyhash, in hex or raw
    let mut all = Vec::new();
    for e in walkdir::walk(&dir) {
        all.extend(e.to_string_lossy().as_bytes());
        all.push(0);
        if e.is_file() {
            all.extend(std::fs::read(&e).unwrap());
        }
    }
    for who in ["bob", "w1", "w2"] {
        let raw = kh(who);
        let hex: String = raw.iter().map(|b| format!("{b:02x}")).collect();
        assert!(!all.windows(32).any(|w| w == raw), "{who} named raw");
        assert!(!all.windows(hex.len()).any(|w| w == hex.as_bytes()), "{who} named in hex");
    }
    // and a reload serves the same bundles and the keys not yet served
    let back = PrekeyService::load(&dir, PrekeyConfig::default()).unwrap();
    assert_eq!(back.bundle(&kh("alice")), svc.bundle(&kh("alice")));
    assert_eq!(back.pool_size(&kh("alice")), 2);
    assert_eq!(back.pool_size(&kh("carol")), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

mod walkdir {
    pub fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut out = vec![dir.to_path_buf()];
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                if e.path().is_dir() {
                    out.extend(walk(&e.path()));
                } else {
                    out.push(e.path());
                }
            }
        }
        out
    }
}

// acceptance: PAY-10
#[test]
fn a_bundle_is_stored_and_served_without_its_blob_being_read() {
    let mut svc = PrekeyService::default();
    let arbitrary: Vec<u8> = (0..200u32).map(|i| (i * 7 % 251) as u8).collect();
    let bundle = bundle_for("alice", &arbitrary, 1_800_000_000);
    svc.publish(&ids(), &bundle).expect("stored whatever the blob is");
    let req = PrekeyRequest::One { subject: kh("alice"), one_time: false, nonce: [1; 16] }.encode();
    let r = PrekeyReply::decode(&svc.answer(&kh("bob"), &req, 0).unwrap()).unwrap();
    assert_eq!(r.bundle.as_deref(), Some(bundle.as_slice()), "served unchanged");
    assert_eq!(PrekeyBundle::parse(r.bundle.as_ref().unwrap()).unwrap().blob, arbitrary);
    // what is checked is the signature, not the contents: a bundle signed by another key is not held
    let forged = PrekeyBundle::build(&id("carol"), CONSTRUCTION_PQXDH, b"x", 1);
    let mut renamed = forged.clone();
    let pos = renamed.windows(32).position(|w| w == kh("carol")).unwrap();
    renamed[pos..pos + 32].copy_from_slice(&kh("alice"));
    assert!(svc.publish(&ids(), &renamed).is_err());
    assert_eq!(svc.bundle(&kh("alice")), Some(&bundle));
}
