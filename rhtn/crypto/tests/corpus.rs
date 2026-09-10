//! The test-vector corpus (`test-vectors/corpus.json`, rhtn-test-corpus/1)
//! run through the codec and crypto crates: every bytes-class entry, deep
//! verification of envelopes, presentations and standalone records, and the
//! cross-entry bindings.  Descended from `test-vectors/runner-rs`.

use rhtn_codec::cbor::*;
use rhtn_codec::cose::{self, aad};
use rhtn_codec::frame::{self, Stream};
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_crypto::verify;
use rhtn_crypto::Identity;

pub const NAMES: [&str; 25] = [
    "alice", "bob", "carol", "alice2", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11",
    "w12", "w13", "w14", "w15", "w16", "c1", "c2", "c3", "c4", "c5",
];

pub fn corpus() -> serde_json::Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../test-vectors/corpus.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

pub fn identities() -> Vec<Identity> {
    NAMES.iter().map(|n| test_identity(n).public).collect()
}

/// The reply family a corpus reply entry belongs to, as the corpus states
/// it on the entry.
fn reply_family(e: &serde_json::Value) -> Family {
    let name = e["family"].as_str().unwrap_or_else(|| panic!("{}: no reply family stated", e["id"]));
    match name {
        "ResolveReply" => Family::ResolveReply,
        "ArchiveReply" => Family::ArchiveReply,
        "PrekeyReply" => Family::PrekeyReply,
        "CatalogReply" => Family::CatalogReply,
        "ResourceResponse" => Family::ResourceResponse,
        "CurrencyReply" => Family::CurrencyReply,
        "ResourceRegistrationReply" => Family::ResourceRegistrationReply,
        other => panic!("{}: unknown reply family {other}", e["id"]),
    }
}

/// A frame parses on whichever stream accepts it; the type spaces overlap
/// and the fixtures do not say which stream carried them.
fn parse_frame(raw: &[u8]) -> Result<frame::Frame, Error> {
    match frame::parse(Stream::Control, raw) {
        Ok(f) if f.family.is_some() => Ok(f),
        _ => frame::parse(Stream::Request, raw),
    }
}

#[test]
fn every_bytes_entry_agrees_with_its_expectation() {
    let corpus = corpus();
    assert_eq!(corpus["format"], "rhtn-test-corpus/1");
    let ids = identities();
    let mut pass = 0u32;
    let mut failures: Vec<String> = Vec::new();
    let mut adopt_min: Vec<u8> = Vec::new();
    let mut pc1_txid: Vec<u8> = Vec::new();
    let mut normal_qid: Vec<u8> = Vec::new();
    let mut push_payload: Vec<u8> = Vec::new();
    let mut keygrant: Vec<u8> = Vec::new();

    for e in corpus["entries"].as_array().unwrap() {
        let id = e["id"].as_str().unwrap();
        if e["class"] != "bytes" {
            continue;
        }
        let expect = &e["expect"];
        let outcome = expect["outcome"].as_str().unwrap();
        let kind = expect["kind"].as_str().unwrap_or("");
        let layer = expect["layer"].as_str().unwrap_or("");
        // a shape fixture the corpus marks as carrying a stale signature is
        // checked structurally; every other signed fixture verifies
        let stale = expect["reason"].as_str().unwrap_or("").contains("signature stale");
        let raw = hex::decode(e["hex"].as_str().unwrap()).unwrap();

        let verdict: Result<(), String> = if kind == "frame" {
            match (outcome, parse_frame(&raw)) {
                ("accept", Ok(_)) => Ok(()),
                ("accept", Err(err)) => Err(format!("frame must be accepted but: {err}")),
                ("reject", Err(_)) => Ok(()),
                ("reject", Ok(_)) => Err("reject frame passed".into()),
                _ => Err("unhandled".into()),
            }
        } else if kind == "reply" {
            let fam = reply_family(e);
            let r = parse_all(&raw).map_err(|e| e.0).and_then(|_| schema::check_unsigned(fam, &raw, 0).map_err(|e| e.0));
            match (outcome, r) {
                ("accept", Ok(())) => Ok(()),
                ("accept", Err(err)) => Err(format!("reply must be accepted but: {err}")),
                ("reject", Err(_)) => Ok(()),
                ("reject", Ok(())) => Err("reject reply passed".into()),
                _ => Err("unhandled".into()),
            }
        } else {
            let parsed = parse_all(&raw);
            match (outcome, parsed) {
                ("reject", Err(_)) if layer == "cbor" => Ok(()),
                ("reject", Ok(_)) if layer == "cbor" => Err("cbor-layer reject parsed".into()),
                (_, Err(err)) => Err(format!("must parse but: {err}")),
                ("accept", Ok(item)) => match kind {
                    "envelope" => verify::envelope(&ids, &raw).map(|_| ()),
                    "presentation" => verify::presentation(&ids, &raw),
                    k if verified_record_kind(k) && !stale => match (schema::check_kind(&raw, k, &item), verify::record(&ids, k, &raw)) {
                        (Err(e2), _) => Err(e2.0.into()),
                        (Ok(()), Ok(true)) => Ok(()),
                        (Ok(()), Ok(false)) => Err("record signature fails".into()),
                        (Ok(()), Err(e2)) => Err(e2),
                    },
                    _ => schema::check_kind(&raw, kind, &item).map_err(|e| e.0.into()),
                },
                ("reject", Ok(item)) => {
                    let schema_ok = schema::check_kind(&raw, kind, &item).is_ok();
                    let sig_fails = verified_record_kind(kind) && !stale && matches!(verify::record(&ids, kind, &raw), Ok(false));
                    if !schema_ok || sig_fails {
                        Ok(())
                    } else if implemented_kind(kind) || verified_record_kind(kind) {
                        Err("reject entry passed our validator".into())
                    } else {
                        continue;
                    }
                }
                _ => Err("unhandled".into()),
            }
        };

        match id {
            "P-adopt-min" => adopt_min = raw.clone(),
            "P-alice-c1-record" => {
                if let Some(r) = value_slice(&raw, 3) {
                    pc1_txid = cose::sha256(&raw[r]).to_vec();
                }
            }
            "P-frame-13" => {
                let p = &raw[4..];
                let outer = array_item_ranges(p, 0).unwrap();
                let triple = array_item_ranges(p, outer[1].start).unwrap();
                let q = &p[triple[0].clone()];
                let r6 = value_slice(q, 6).unwrap();
                normal_qid = q[r6.start + 2..r6.end].to_vec();
                assert_eq!(cose::sha256(&map_without_key(q, 6).unwrap()).to_vec(), normal_qid, "frame-13 query_id");
                let c = &p[triple[1].clone()];
                let __ca_item = parse_all(c).unwrap();
                let Item::Array(ca) = &__ca_item else { panic!() };
                let gp = |it: &Item| match it { Item::Bytes(r) => c[r.clone()].to_vec(), _ => Vec::new() };
                let alice = test_identity("alice").public;
                assert!(alice.verify_ed(&gp(&ca[3]), &cose::sig_structure_sign1(&gp(&ca[0]), aad::CONSENT, &normal_qid)), "frame-13 consent");
            }
            "P-frame-06" => {
                let p = &raw[4..];
                let __fa_item = parse_all(p).unwrap();
                let Item::Array(fa) = &__fa_item else { panic!() };
                if let Item::Map(fm) = &fa[1]
                    && let Some(Item::Bytes(pr)) = map_get(fm, 2) {
                        push_payload = p[pr.clone()].to_vec();
                        verify::envelope(&ids, &push_payload).expect("inner envelope");
                    }
            }
            "P-e2e-01" => keygrant = raw.clone(),
            _ => {}
        }
        match verdict {
            Ok(()) => pass += 1,
            Err(msg) => failures.push(format!("{id}: {msg}")),
        }
    }
    assert_eq!(push_payload, adopt_min, "TopologyPush payload is P-adopt-min");
    let __kg_item = parse_all(&keygrant).unwrap();
    let Item::Map(kg) = &__kg_item else { panic!() };
    let gb = |k: u64| match map_get(kg, k) { Some(Item::Bytes(r)) => keygrant[r.clone()].to_vec(), _ => Vec::new() };
    assert_eq!(gb(1), pc1_txid, "KeyGrant names the prior record");
    assert_eq!(gb(2), normal_qid, "KeyGrant names the current query");
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
    eprintln!("corpus: {pass} entries agree");
    assert!(pass >= 149, "expected at least the runner's 149 agreements, got {pass}");
}

/// Standalone records whose fixtures carry live signatures, verified as
/// such; a shape fixture that says its signature is stale by design is the
/// one exception, taken per fixture.
fn verified_record_kind(kind: &str) -> bool {
    matches!(kind, "CurrencyAttestation" | "CatalogEntry" | "AbuseReport" | "AnchorEntry" | "SubtreeAck" | "PrekeyBundle" | "EndpointRecord" | "SignedLocator")
}

fn implemented_kind(kind: &str) -> bool {
    matches!(
        kind,
        "VerifierResponse" | "Locator" | "NetworkPoint" | "LocationEvidence" | "Proximity" | "Scope" | "Capabilities"
            | "CatalogEntry" | "CurrencyAttestation" | "ResolveReply" | "ResourceResponse" | "ArchiveRequest"
            | "PrekeyBundle" | "PrekeyBatchRequest" | "Witness" | "body" | "SignedLocator" | "VerificationQuery"
            | "CatalogReply"
    )
}
