//! The field-test build's redaction, over a whole harness ceremony
//! (`Robot/field-test-diagnostics.md`, section 2): every event a ceremony,
//! a payload session and a recovery raise is rendered by the field build's
//! own renderer and collected; then every secret those runs produced is
//! searched for in the rendering, eight bytes at a time, as hex in either
//! case and as a `Debug` byte list.  Not one run may appear.  The timeline
//! is then checked for the step sequence the plan's section 3 names.
//!
//! Built and run only in the field-test flavour, which is the one that has
//! events: `cargo test -p rhtn-client --no-default-features --features
//! fieldtest --test diag`.  The releasable flavour compiles this file to
//! nothing, as it compiles the hooks to nothing.
#![cfg(feature = "fieldtest")]

mod common;

use common::harness::*;
use common::*;
use rhtn_archive::prekey::{PrekeyReply, PrekeyRequest};
use rhtn_client::ceremony::{Client, Msg};
use rhtn_client::device::ChannelKind;
use rhtn_client::diag::json::JsonLines;
use rhtn_client::keys::capture_key;
use rhtn_client::payload::KIND_APPLICATION;
use rhtn_client::query::{KeyGrant, QueryRequest};
use rhtn_client::rotation::Rotation;
use rhtn_codec::cose::sha256;
use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex};
use tracing_subscriber::layer::SubscriberExt;

const PARTIES: [&str; 6] = ["alice", "alice2", "bob", "carol", "w1", "w2"];

/// The two seeds `test_identity` derives every party from.
fn identity_seeds(name: &str) -> Vec<Vec<u8>> {
    vec![
        sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes()).to_vec(),
        sha256(format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes()).to_vec(),
    ]
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A payload session from `from` to `to` on reusable prekeys, the serving
/// node's part played by hand, and one message each way on it.
fn session(s: &mut Setup, from: &str, to: &str) {
    let now = s.clock.get() / 1000;
    let bundle_of = |c: &mut Client, who: &str| {
        let device = c.payload.device;
        c.payload.keys.bundle(&id(who), &device, now)
    };
    let theirs = bundle_of(s.client(to), to);
    let mine = bundle_of(s.client(from), from);
    assert!(s.client(from).take_binding(&theirs), "the peer's binding");
    assert!(s.client(to).take_binding(&mine), "and mine at the peer");
    let asked = s
        .client(from)
        .send_payload(kh(to), KIND_APPLICATION, b"over the session")
        .expect("queued for the session");
    let nonce = asked
        .iter()
        .find_map(|m| match m {
            Msg::PrekeyRequest(req) => match PrekeyRequest::decode(req).expect("a request") {
                PrekeyRequest::One { nonce, .. } => Some(nonce),
                PrekeyRequest::Batch { .. } => None,
            },
            _ => None,
        })
        .expect("a one-time key asked for");
    let reply = PrekeyReply {
        nonce,
        bundles: vec![theirs],
        one_time: None,
        code: None,
    }
    .encode();
    let out = s
        .client(from)
        .take_prekey_reply(&reply)
        .expect("the session opens on the reusable material");
    deliver(s, from, to, out);
    let back = s
        .client(to)
        .send_payload(kh(from), KIND_APPLICATION, b"and back")
        .expect("on the session held");
    deliver(s, to, from, back);
}

fn deliver(s: &mut Setup, from: &str, to: &str, msgs: Vec<Msg>) {
    for m in msgs {
        match m {
            Msg::Payload { bytes, .. } | Msg::Relay { bytes, .. } => {
                s.client(to)
                    .receive_payload(kh(from), &bytes)
                    .expect("decrypts at the recipient");
            }
            other => panic!("not payload: {}", rhtn_client::diag::msg(&other)),
        }
    }
}

/// Every eight-byte run the rendering could be showing, as lowercase hex:
/// each 16-character window of every run of hex digits sixteen or longer,
/// in either case, and each eight-byte window of every `Debug` byte list.
fn observed_runs(text: &str) -> HashSet<String> {
    let mut seen = HashSet::new();
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_hexdigit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_hexdigit() {
                i += 1;
            }
            let run = text[start..i].to_ascii_lowercase();
            if run.len() >= 16 {
                for w in 0..=run.len() - 16 {
                    seen.insert(run[w..w + 16].to_string());
                }
            }
        } else {
            i += 1;
        }
    }
    for (at, _) in text.match_indices('[') {
        let rest = &text[at + 1..];
        let Some(end) = rest.find(']') else { continue };
        let nums: Option<Vec<u8>> = rest[..end]
            .split(',')
            .map(|n| n.trim().parse::<u8>().ok())
            .collect();
        if let Some(bytes) = nums
            && bytes.len() >= 8
        {
            for w in bytes.windows(8) {
                seen.insert(hex(w));
            }
        }
    }
    seen
}

fn in_order(names: &[String], expected: &[&str]) {
    let mut from = 0;
    for e in expected {
        match names[from..].iter().position(|n| n == e) {
            Some(i) => from += i + 1,
            None => panic!("{e} is not in the timeline after position {from}"),
        }
    }
}

// acceptance: none; a test facility's own test (Robot/field-test-diagnostics.md, section 6)
#[test]
fn no_secret_of_a_whole_ceremony_appears_in_any_event_and_the_timeline_is_complete() {
    let lines: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = lines.clone();
    let subscriber = tracing_subscriber::registry().with(JsonLines::new(move |line| {
        sink.lock().unwrap().push(line);
    }));
    // two ceremonies, an adoption, a payload session, two recovery
    // meetings and the recovery adoption, all rendered
    let s = tracing::subscriber::with_default(subscriber, || {
        let mut s = setup(&PARTIES, &[ChannelKind::Nfc]);
        s.run("alice", "bob", &["w2"], &[])
            .expect("alice meets bob");
        s.run("alice", "carol", &["w2"], &[])
            .expect("alice meets carol");
        s.face_off("alice", "w1");
        s.h.run_adoption(kh("alice"), kh("w1"), vec![kh("w2")], vec![], 3)
            .expect("alice adopted under w1");
        session(&mut s, "alice", "bob");
        let old_archive = s.client("alice").archive.clone();
        s.client("alice2").rotation = Some(Box::new(Rotation::begin(
            Box::new(id("alice")),
            old_archive,
        )));
        s.face_off("alice2", "bob");
        s.h.run_recovery_meeting(kh("alice2"), kh("bob"))
            .expect("bob recognises");
        s.face_off("alice2", "carol");
        s.h.run_recovery_meeting(kh("alice2"), kh("carol"))
            .expect("carol recognises");
        s.h.run_recovery_adoption(kh("alice2"), kh("w1"), 7)
            .expect("adopted");
        s
    });
    let lines = lines.lock().unwrap().clone();
    assert!(
        lines.len() > 200,
        "{} events is not a timeline",
        lines.len()
    );
    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|e| panic!("not one JSON object: {e}: {l}"))
        })
        .collect();
    for v in &parsed {
        assert!(v.get("ms").is_some_and(|m| m.is_u64()), "ms: {v}");
        assert!(v.get("layer").is_some_and(|l| l.is_string()), "layer: {v}");
        assert!(v.get("event").is_some_and(|e| e.is_string()), "event: {v}");
    }

    // everything secret the runs produced: identity seeds, the ceremony
    // seeds and the capture keys they derive, the payload material and the
    // sessions as they go to storage, and what crossed on the harness: the
    // keys in flight, the keys granted, the consents and the profiles
    let mut secrets: Vec<(&'static str, Vec<u8>)> = Vec::new();
    for n in PARTIES {
        for seed in identity_seeds(n) {
            secrets.push(("identity seed", seed));
        }
    }
    for (k, c) in &s.h.clients {
        for seed in c.store.seeds.values() {
            secrets.push(("ceremony seed", seed.seed.to_vec()));
            secrets.push((
                "capture key",
                capture_key(&seed.seed, k, &seed.counterparty, &seed.ceremony_id).to_vec(),
            ));
        }
        secrets.push(("payload material", c.payload.keys.encode()));
        secrets.push(("sessions", c.payload.sessions.encode()));
    }
    for sent in &s.h.log {
        match &sent.msg {
            Msg::CaptureKey(k) => secrets.push(("capture key in flight", k.to_vec())),
            Msg::Grant(b) => secrets.push((
                "granted key",
                KeyGrant::decode(b).expect("a grant").key.to_vec(),
            )),
            Msg::Consent { consent, .. } => secrets.push(("consent", consent.clone())),
            Msg::ConsentRequest(q) => secrets.push(("profile", q.profile.clone())),
            Msg::Query(b) => {
                let r = QueryRequest::decode(b).expect("a request");
                secrets.push(("profile", r.query.profile.clone()));
                secrets.push(("consent", r.consent.clone()));
            }
            _ => {}
        }
    }
    assert!(
        secrets.iter().filter(|(w, _)| *w == "granted key").count() >= 1,
        "the runs released at least one key"
    );
    assert!(
        secrets.iter().filter(|(w, _)| *w == "sessions").count() == PARTIES.len(),
        "every party's sessions"
    );
    let text = lines.join("\n");
    let seen = observed_runs(&text);
    let mut hits = Vec::new();
    for (what, bytes) in &secrets {
        for w in bytes.windows(8) {
            let h = hex(w);
            if seen.contains(&h) {
                hits.push(format!("{what}: {h}"));
            }
        }
    }
    hits.sort();
    hits.dedup();
    assert!(hits.is_empty(), "secret runs rendered: {hits:?}");

    // the timeline: the step sequence of a ceremony, then the session, the
    // recovery meeting's close and the seal on its way
    let names: Vec<String> = parsed
        .iter()
        .map(|v| v["event"].as_str().unwrap_or("").to_string())
        .collect();
    in_order(
        &names,
        &[
            "cer.begin",
            "cer.intent.received",
            "cer.id_fixed",
            "cer.proximity",
            "cer.proximity.done",
            "cer.capture",
            "cer.select",
            "cer.query.issued",
            "sub.consented",
            "sub.grant",
            "ver.grant",
            "ver.query.answered",
            "cer.response",
            "sub.response_copy",
            "cer.witness.asked",
            "cer.witness.answered",
            "cer.propose",
            "cer.review",
            "cer.finalize",
            "pay.session",
            "pay.receive",
            "cer.abandon",
        ],
    );
    assert!(
        lines.iter().any(|l| l.contains("\"msg\":\"Seal\"")),
        "the recovery's seal travelled on the harness"
    );
    assert!(
        !names.iter().any(|n| n == "cer.abort" || n == "pay.error"),
        "a clean run aborts nothing and loses no payload"
    );
    assert!(
        lines.iter().any(|l| l.contains("\"state\":\"opened\"")),
        "the session's opening was seen"
    );

    // the histogram, for `--nocapture`: the hooks that fired, by name
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for n in &names {
        *counts.entry(n.as_str()).or_default() += 1;
    }
    eprintln!("{} events over {} names", names.len(), counts.len());
    for (n, c) in &counts {
        eprintln!("  {c:>5}  {n}");
    }
}
