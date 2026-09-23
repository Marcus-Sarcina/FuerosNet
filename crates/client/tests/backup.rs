//! Backup: envelope encryption over what a device loss takes away
//! (design §13.7.1), and what a restore refuses.

mod common;

use common::*;
use rhtn_archive::tx::{TYPE_PRESENCE, Witness};
use rhtn_client::backup::{self, Contents, Cost, Failure, Wrap};
use rhtn_client::record::*;
use rhtn_client::store::{Capture, ClientStore, Frame, OwnSeed, SealParams, seal};

/// Cheap enough to run in a test and still Argon2id: the cost is the
/// operator's number, and a test asserting a shape has no business
/// spending 64 MiB on it.
fn cheap() -> Cost {
    Cost {
        m_kib: 64,
        passes: 1,
        lanes: 1,
    }
}

fn full_set() -> DisclosureSet {
    let values = [
        capture_value(0, 4, 0, 2),
        empty_location_value(),
        integrity_value(false, 0),
        retention_value(2),
        integrity_value(false, 0),
        retention_value(2),
        proximity_value(&[(1, 0, Some(4))], 1),
    ];
    disclosures(values, std::array::from_fn(|i| [i as u8 + 1; 16]))
}

/// Everything a device loss takes away: the identity, the archive, and the
/// store beside it.
fn contents(w: &mut World) -> (Contents, [u8; 32]) {
    let set = full_set();
    let t = w.tick();
    let p = Proposal {
        started_at: t,
        finalized_at: t + 600,
        participants: [kh("alice"), kh("bob")],
        witnesses: vec![Witness {
            keyhash: kh("w1"),
            nominated_by: kh("alice"),
            flags: 7,
        }],
        responses: vec![],
        root: disclosure_root(&set),
    };
    let back = vec![w.back("alice"), w.back("bob"), w.back("w1")];
    let rec = w.commit(TYPE_PRESENCE, &p.body(&back), &["alice", "bob", "w1"]);

    let mut store = ClientStore::default();
    store.records.insert(rec.txid, rec.bytes.clone());
    store.disclosures.insert(rec.txid, set);
    store.seeds.insert(
        rec.txid,
        OwnSeed {
            seed: [9; 32],
            counterparty: kh("bob"),
            ceremony_id: [4; 32],
            finalized_at: t + 600,
        },
    );
    let capture = Capture {
        template: vec![3; 32],
        frames: vec![Frame {
            at_ms: 7,
            bytes: b"a frame".to_vec(),
        }],
        modality: 0,
        template_version: 1,
    };
    store.sealed.insert(
        rec.txid,
        seal(
            &SealParams::default(),
            &[1; 32],
            kh("bob"),
            kh("alice"),
            [4; 32],
            &capture,
        ),
    );
    (
        Contents {
            seeds: Some([[1; 32], [2; 32]]),
            records: vec![rec.bytes.clone()],
            store,
            provider: None,
        },
        rec.txid,
    )
}

// acceptance: ARC-23
#[test]
fn a_backup_is_enveloped_under_a_key_kept_apart_and_comes_back_whole() {
    let mut w = World::new();
    let (before, _) = contents(&mut w);
    let wrap = Wrap::passphrase(cheap());
    let blob = backup::export(&before, &wrap, b"a passphrase").expect("exports");

    // **the KEK is never stored with the ciphertext** (design §13.7.1):
    // neither the passphrase nor anything derived from it is in the blob,
    // and neither is the plaintext it protects
    assert!(
        !find(&blob, b"a passphrase"),
        "the passphrase is not in the backup"
    );
    let kek_probe = before.seeds.unwrap()[0];
    assert!(
        !find(&blob, &kek_probe),
        "nor is the identity seed in the clear"
    );
    assert!(!find(&blob, b"a frame"), "nor a captured frame");

    let after = backup::import(&blob, b"a passphrase").expect("imports");
    assert_eq!(
        after, before,
        "everything a device loss takes away comes back"
    );

    // the salt is fresh per backup, so two exports of the same contents
    // under the same passphrase are different bytes
    let again =
        backup::export(&before, &Wrap::passphrase(cheap()), b"a passphrase").expect("exports");
    assert_ne!(again, blob, "a fresh salt and fresh nonces each time");
    assert_eq!(
        backup::import(&again, b"a passphrase").expect("imports"),
        before
    );
}

// acceptance: ARC-24
#[test]
fn a_backup_refuses_a_wrong_secret_a_tampered_blob_and_a_truncated_one_by_name() {
    let mut w = World::new();
    let (before, _) = contents(&mut w);
    let blob =
        backup::export(&before, &Wrap::passphrase(cheap()), b"a passphrase").expect("exports");

    // **the secret is wrong, which is a different thing to tell a person
    // than a tampered payload**, and both are authentication failures
    assert_eq!(
        backup::import(&blob, b"the wrong one"),
        Err(Failure::Secret)
    );
    assert_eq!(backup::import(&blob, b""), Err(Failure::Secret));

    // altering the header makes the data key refuse to unwrap: the header
    // is the wrap's aad, so a changed cost or salt is not a different
    // derivation but a failure
    let mut headered = blob.clone();
    let at = headered
        .iter()
        .position(|b| *b == 64)
        .expect("the memory cost is in there");
    headered[at] = 65;
    assert!(
        matches!(
            backup::import(&headered, b"a passphrase"),
            Err(Failure::Secret) | Err(Failure::Malformed(_))
        ),
        "an altered header does not open"
    );

    // altering the payload authenticates the key and fails the body: the
    // two steps are distinguishable, which is what lets a person be told
    // which one went wrong
    let mut tampered = blob.clone();
    let n = tampered.len();
    tampered[n - 20] ^= 1;
    assert_eq!(
        backup::import(&tampered, b"a passphrase"),
        Err(Failure::Payload)
    );

    // **nothing is returned from a backup that did not authenticate
    // whole**: a truncated blob is refused rather than half-imported
    for cut in [blob.len() - 1, blob.len() / 2, 8, 0] {
        let short = &blob[..cut];
        assert!(
            backup::import(short, b"a passphrase").is_err(),
            "a backup cut at {cut} opens nothing"
        );
    }
}

// acceptance: ARC-25
#[test]
fn the_wrapping_is_replaceable_without_touching_what_it_wraps() {
    let mut w = World::new();
    let (before, _) = contents(&mut w);

    // **the cost is the operator's and travels in the header**, so a
    // reader uses what the writer used rather than what it would have
    // chosen: two backups at different costs both open
    for cost in [
        cheap(),
        Cost {
            m_kib: 128,
            passes: 2,
            lanes: 1,
        },
    ] {
        let blob =
            backup::export(&before, &Wrap::passphrase(cost), b"a passphrase").expect("exports");
        assert_eq!(
            backup::import(&blob, b"a passphrase").expect("imports"),
            before,
            "at {cost:?}"
        );
    }

    // **a wrap method this reader does not implement is named, not
    // guessed** (design §13.7.1: passphrase in v1, hardware token later,
    // split shares later still — none of which changes the format)
    let blob =
        backup::export(&before, &Wrap::passphrase(cheap()), b"a passphrase").expect("exports");
    let future = with_wrap_method(&blob, 7);
    assert_eq!(
        backup::import(&future, b"a passphrase"),
        Err(Failure::UnsupportedWrap(7))
    );

    // and a later format version likewise
    let later = with_version(&blob, 2);
    assert_eq!(
        backup::import(&later, b"a passphrase"),
        Err(Failure::Version(2))
    );
}

fn find(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// Rewrite the header's wrap method, leaving everything else alone.
fn with_wrap_method(blob: &[u8], method: u64) -> Vec<u8> {
    rewrite_header_field(blob, 1, method)
}

fn with_version(blob: &[u8], version: u64) -> Vec<u8> {
    rewrite_header_field(blob, 0, version)
}

/// The header is `[version, method, salt, m, t, p, nonces]`; both fields
/// this rewrites are small uints, so the length does not move.
fn rewrite_header_field(blob: &[u8], index: usize, value: u64) -> Vec<u8> {
    use rhtn_codec::cbor::{Item, parse_all};
    let item = parse_all(blob).expect("a backup");
    let Item::Array(parts) = &item else {
        panic!("three fields")
    };
    let Item::Bytes(hr) = &parts[0] else {
        panic!("a header")
    };
    let header = &blob[hr.clone()];
    let hitem = parse_all(header).expect("a header");
    let Item::Array(f) = &hitem else {
        panic!("an array")
    };
    let Item::Uint(_) = &f[index] else {
        panic!("a uint")
    };
    let mut fresh = header.to_vec();
    // a uint under 24 is one byte at a known offset: the array head, then
    // one byte per preceding small uint
    let at = 1 + index;
    assert!(
        value < 24 && fresh[at] < 24,
        "only small values move no bytes"
    );
    fresh[at] = value as u8;
    let mut out = Vec::new();
    rhtn_codec::encode::emit_array_head(&mut out, 3);
    rhtn_codec::encode::emit_bstr(&mut out, &fresh);
    for p in &parts[1..] {
        let Item::Bytes(r) = p else { panic!("bytes") };
        rhtn_codec::encode::emit_bstr(&mut out, &blob[r.clone()]);
    }
    out
}

/// **Import is where the leak occurs** (design §13.7.1): age-based
/// flushing acts on live data, and a restored backup reintroduces files
/// that aged while offline.  So the scan runs on the way in, and it covers
/// device migration and manual copies as well as restore.
// acceptance: ARC-26
#[test]
fn a_scan_on_import_discards_what_is_past_its_window_and_keeps_the_history() {
    let mut w = World::new();
    let (before, txid) = contents(&mut w);
    let finalized = before.store.seeds[&txid].finalized_at;
    let two_years = 2 * 365 * 86_400;

    // inside the window, everything lands
    let mut fresh = before.clone();
    assert_eq!(
        fresh.scan(finalized + 60, two_years),
        backup::Discarded::default(),
        "nothing is past anything yet"
    );
    assert_eq!(fresh, before);

    // a day past it, the likeness goes and the history stays
    let mut aged = before.clone();
    let out = aged.scan(finalized + two_years + 86_400, two_years);
    assert_eq!(
        out,
        backup::Discarded {
            captures: 1,
            seeds: 1
        },
        "the capture and the seed"
    );
    assert!(
        aged.store.sealed.is_empty(),
        "no likeness past the window it was committed under"
    );
    assert!(
        aged.store.seeds.is_empty(),
        "nor the seed that would release one"
    );

    // **history is not likeness and is not touched** — §13.7.1's concern
    // is photographs outliving the commitment made about them, and L §2
    // separately forbids deleting presence records with a chain prune
    assert_eq!(aged.store.records, before.store.records, "the records stay");
    assert_eq!(
        aged.store.disclosures, before.store.disclosures,
        "and what they committed to"
    );
    assert_eq!(aged.records, before.records, "and the archive");
    assert_eq!(
        aged.seeds, before.seeds,
        "and the identity, which is not a retention question"
    );
}

/// The scan runs through the client, on its own clock and its own window,
/// and says what it threw away rather than swallowing it.
// acceptance: ARC-27
#[test]
fn a_client_scans_what_it_imports_and_reports_what_it_dropped() {
    let mut w = World::new();
    let (before, txid) = contents(&mut w);
    let finalized = before.store.seeds[&txid].finalized_at;
    let blob =
        backup::export(&before, &Wrap::passphrase(cheap()), b"a passphrase").expect("exports");

    // a client whose clock is inside the window imports the lot
    let mut c = fresh("alice", (finalized + 60) * 1000);
    let (kept, dropped) = c.import(&blob, b"a passphrase").expect("imports");
    assert_eq!(dropped, backup::Discarded::default());
    assert_eq!(kept.store.sealed.len(), 1);

    // one whose clock is past it gets the history and not the likeness,
    // and is told so
    c = fresh("alice", (finalized + 2 * 365 * 86_400 + 1) * 1000);
    let (kept, dropped) = c.import(&blob, b"a passphrase").expect("imports");
    assert_eq!(
        dropped,
        backup::Discarded {
            captures: 1,
            seeds: 1
        },
        "reported, not swallowed"
    );
    assert!(kept.store.sealed.is_empty());
    assert_eq!(kept.store.records, before.store.records);

    // and a backup that does not open discards nothing, because nothing
    // was opened
    assert!(c.import(&blob, b"the wrong one").is_err());

    // the round trip through the client's own export
    let mut source = fresh("alice", (finalized + 60) * 1000);
    source.store = before.store.clone();
    let own = source
        .export(before.seeds, cheap(), b"another passphrase")
        .expect("exports");
    let (back, _) = source.import(&own, b"another passphrase").expect("imports");
    assert_eq!(
        back.store, before.store,
        "what this client wrote, this client reads"
    );
    assert_eq!(back.seeds, before.seeds);
}

/// A client at a stated instant, for the scan's clock.
fn fresh(name: &str, at_ms: u64) -> rhtn_client::ceremony::Client {
    let clock = std::rc::Rc::new(std::cell::Cell::new(at_ms));
    let (device, _) = common::harness::device(vec![], clock, 7, 0);
    rhtn_client::ceremony::Client::new(id(name), ids(), Default::default(), device)
}

// acceptance: ARC-28
#[test]
fn an_operators_provider_credential_rides_in_the_envelope_and_never_in_the_archive() {
    let mut w = World::new();
    let (mut c, _t) = contents(&mut w);
    let credential = b"provider-account:token-7f3a9c:region-eu".to_vec();
    c.provider = Some(credential.clone());
    let wrap = Wrap::passphrase(cheap());
    let blob = rhtn_client::backup::export(&c, &wrap, b"correct horse").unwrap();

    // under the passphrase it comes back whole; under another it does not
    // come back at all
    let back = rhtn_client::backup::import(&blob, b"correct horse").unwrap();
    assert_eq!(back.provider.as_deref(), Some(credential.as_slice()));
    assert_eq!(back, c);
    assert_eq!(
        rhtn_client::backup::import(&blob, b"wrong").unwrap_err(),
        Failure::Secret,
        "the credential opens only under the passphrase"
    );
    assert!(
        !find(&blob, &credential),
        "no byte of it is in the clear in the envelope"
    );

    // the archive a sibling replicates is the records, and they hold no
    // byte of it: the credential is the operator's business, not the
    // network's
    for r in &c.records {
        assert!(!find(r, &credential), "a record carries none of it");
    }
    let store = c.store.encode();
    assert!(
        !find(&store, &credential),
        "nor does the store the client writes"
    );

    // a client holding one exports it, and one holding none exports the
    // slot empty rather than absent
    let mut client = fresh("alice", 1_790_000_000_000);
    client.provider_credential = Some(credential.clone());
    let blob = client.export(None, cheap(), b"pw").unwrap();
    let (got, _) = client.import(&blob, b"pw").unwrap();
    assert_eq!(got.provider, Some(credential));
    client.provider_credential = None;
    let blob = client.export(None, cheap(), b"pw").unwrap();
    assert_eq!(client.import(&blob, b"pw").unwrap().0.provider, None);
}
