//! **What a resource may ask of the client, and what goes back**
//! (`light-client-requirements.md` §8.1; `resource-requirements.md` §3.4).
//!
//! The ask is read once by the kernel and refused rather than guessed
//! at; the answer is composed by the kernel, addressed to the resource
//! that asked, and signed — where the utility signs — under a key for
//! that one audience, which is never the identity's.

mod common;

use common::*;
use rhtn_client::ceremony::Client;
use rhtn_client::resource::{Shape, answer};
use rhtn_client::utility::*;

fn client(name: &str) -> Client {
    let clock = std::rc::Rc::new(std::cell::Cell::new(1_700_000_000_000u64));
    let (device, _) = common::harness::device(clock, 7, 0);
    Client::new(id(name), ids(), Default::default(), device)
}

fn ask_json(utility: &str, extra: &str) -> Vec<u8> {
    format!(
        "{{\"utility\":\"{utility}\",\"ask\":\"a photograph of the meter, for the reading\",\"retention\":\"ninety days, then deleted\",\"return\":\"/meter/reading\",\"token\":\"m-42\"{extra}}}"
    )
    .into_bytes()
}

fn http(media: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!(
        "HTTP/1.1 200 OK\r\ncontent-type: {media}\r\ncontent-length: {}\r\n\r\n",
        body.len()
    )
    .into_bytes();
    out.extend_from_slice(body);
    out
}

/// The parts of a multipart body, by name: the headers and the bytes.
fn parts(request: &[u8]) -> (String, Vec<(String, String, Vec<u8>)>) {
    let s = String::from_utf8_lossy(request).to_string();
    let (head, body) = s.split_once("\r\n\r\n").expect("a head");
    let boundary = head
        .split("boundary=")
        .nth(1)
        .expect("a boundary")
        .lines()
        .next()
        .unwrap()
        .to_string();
    let mut out = Vec::new();
    for piece in body.split(&format!("--{boundary}")) {
        let piece = piece.trim_start_matches("\r\n");
        if piece.is_empty() || piece.starts_with("--") {
            continue;
        }
        let (h, b) = piece.split_once("\r\n\r\n").expect("a part head");
        let name = h
            .split("name=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string();
        let b = b.strip_suffix("\r\n").unwrap_or(b);
        out.push((name, h.to_string(), b.as_bytes().to_vec()));
    }
    (head.to_string(), out)
}

fn part<'a>(parts: &'a [(String, String, Vec<u8>)], name: &str) -> &'a (String, String, Vec<u8>) {
    parts
        .iter()
        .find(|(n, _, _)| n == name)
        .unwrap_or_else(|| panic!("a part named {name}"))
}

// acceptance: RSC-66
#[test]
fn an_ask_is_read_as_the_declared_type_and_refused_rather_than_guessed_at() {
    // the media type makes it an ask; the same bytes under text/plain are text
    let a = answer(&http(
        MEDIA,
        &ask_json("capture", ",\"media\":[\"image/jpeg\"]"),
    ))
    .expect("parses");
    assert_eq!(a.shape, Shape::Ask);
    let t = answer(&http("text/plain", &ask_json("capture", ""))).expect("parses");
    assert_eq!(t.shape, Shape::Text, "nothing is sniffed");

    let ask = parse(&ask_json(
        "capture",
        ",\"media\":[\"image/JPEG\"],\"channel\":\"qr\"",
    ))
    .expect("an ask");
    assert_eq!(ask.utility, Utility::Capture);
    assert_eq!(ask.purpose, "a photograph of the meter, for the reading");
    assert_eq!(ask.retention, "ninety days, then deleted");
    assert_eq!(ask.return_path, "/meter/reading");
    assert_eq!(ask.token, "m-42");
    assert_eq!(ask.media, vec!["image/jpeg".to_string()], "lower-cased");
    assert_eq!(ask.channel.as_deref(), Some("qr"));

    // every refusal is by name
    assert_eq!(parse(b"[]"), Err(Malformed::NotAnObject));
    assert_eq!(
        parse(&ask_json("teleport", "")),
        Err(Malformed::Unknown("teleport".into()))
    );
    assert_eq!(
        parse(b"{\"utility\":\"tag\",\"ask\":\"x\",\"retention\":\"y\",\"return\":\"meter\",\"token\":\"t\"}"),
        Err(Malformed::Field("return")),
        "a return path is rooted"
    );
    assert_eq!(
        parse(b"{\"utility\":\"tag\",\"ask\":\"x\",\"retention\":\"y\",\"return\":\"/.rhtn/envelope\",\"token\":\"t\"}"),
        Err(Malformed::Field("return")),
        "and never the node's own namespace"
    );
    assert_eq!(
        parse(b"{\"utility\":\"tag\",\"ask\":\"x\",\"retention\":\"y\",\"return\":\"/t\",\"token\":\"a b\"}"),
        Err(Malformed::Field("token"))
    );
    assert_eq!(
        parse(b"{\"utility\":\"tag\",\"ask\":\"x\",\"return\":\"/t\",\"token\":\"t\"}"),
        Err(Malformed::Field("retention")),
        "retention is stated or the ask is not one"
    );
    assert_eq!(
        parse(&ask_json("sign", "")),
        Err(Malformed::Field("payload")),
        "a signature needs its payload"
    );
    assert_eq!(
        parse(&ask_json("location", "")),
        Err(Malformed::Field("precision"))
    );
    assert_eq!(
        parse(&ask_json("location", ",\"precision\":13")),
        Err(Malformed::Field("precision"))
    );
    let long = "x".repeat(WORDS_BYTES + 1);
    assert_eq!(
        parse(format!("{{\"utility\":\"tag\",\"ask\":\"{long}\",\"retention\":\"y\",\"return\":\"/t\",\"token\":\"t\"}}").as_bytes()),
        Err(Malformed::Field("ask")),
        "a purpose is shown whole, so it is bounded"
    );
    assert_eq!(parse(&vec![b' '; ASK_BYTES + 1]), Err(Malformed::TooLarge));
    for m in [
        Malformed::TooLarge,
        Malformed::NotAnObject,
        Malformed::Field("ask"),
        Malformed::Unknown("z".into()),
    ] {
        assert!(!m.say().is_empty());
    }
}

// acceptance: RSC-66
#[test]
fn the_answer_goes_back_to_the_resource_that_asked_as_a_form_it_can_read() {
    let mut c = client("alice");
    let node = [1u8; 32];
    let resource = [2u8; 32];
    let (id, ask) = c
        .take_utility_ask(node, resource, &ask_json("location", ",\"precision\":5"))
        .expect("taken");
    assert_eq!(ask.precision, Some(5));
    assert_eq!(c.pending_asks().count(), 1);

    // a place at another precision is not an answer to this ask
    let wrong = c.answer_utility(id, Outcome::Text("u4pruyd".into()));
    assert!(wrong.is_err(), "{wrong:?}");
    // the ask was spent by the attempt; take it again
    let (id, _) = c
        .take_utility_ask(node, resource, &ask_json("location", ",\"precision\":5"))
        .unwrap();
    let msg = c
        .answer_utility(id, Outcome::Text("u4pru".into()))
        .expect("answered");
    let rhtn_client::ceremony::Msg::ResourceRequest { node: to, bytes } = msg else {
        panic!("a resource request");
    };
    assert_eq!(to, node, "through the node that served it");
    let req = rhtn_archive::catalog::ResourceRequest::decode(&bytes).expect("decodes");
    assert_eq!(req.resource, resource, "to the resource that asked");
    let (head, ps) = parts(&req.message);
    assert!(
        head.starts_with("POST /meter/reading HTTP/1.1\r\n"),
        "{head}"
    );
    assert!(head.contains("content-type: multipart/form-data; boundary=rhtn"));
    assert_eq!(part(&ps, "utility").2, b"location");
    assert_eq!(part(&ps, "token").2, b"m-42");
    assert_eq!(part(&ps, "outcome").2, b"given");
    assert_eq!(part(&ps, "result").2, b"u4pru");
    assert!(
        ps.iter().all(|(n, _, _)| n != "claim"),
        "a place carries no claim"
    );
    assert_eq!(c.pending_asks().count(), 0, "spent");
    assert!(
        c.answer_utility(id, Outcome::Approved).is_err(),
        "and not answerable twice"
    );

    // a refusal is an answer the application sees
    let (id, _) = c
        .take_utility_ask(node, resource, &ask_json("scan", ""))
        .unwrap();
    let msg = c
        .answer_utility(id, Outcome::Declined("no".into()))
        .expect("declined");
    let rhtn_client::ceremony::Msg::ResourceRequest { bytes, .. } = msg else {
        panic!()
    };
    let req = rhtn_archive::catalog::ResourceRequest::decode(&bytes).unwrap();
    let (_, ps) = parts(&req.message);
    assert_eq!(part(&ps, "outcome").2, b"declined");
    assert_eq!(part(&ps, "reason").2, b"no");
    assert!(ps.iter().all(|(n, _, _)| n != "result"));

    // the oldest ask goes when the bound is reached
    for _ in 0..PENDING + 2 {
        c.take_utility_ask(node, resource, &ask_json("tag", ""))
            .unwrap();
    }
    assert_eq!(c.pending_asks().count(), PENDING);
}

// acceptance: RSC-66
#[test]
fn a_capture_and_a_signature_are_claimed_under_a_key_for_that_audience_alone() {
    let mut c = client("alice");
    let node = [1u8; 32];
    let meter = [2u8; 32];
    let other = [3u8; 32];
    assert!(c.application_secret.is_none(), "minted at first need");

    let (id, _) = c
        .take_utility_ask(node, meter, &ask_json("capture", ""))
        .unwrap();
    let photo = b"JFIF a jpeg".to_vec();
    let msg = c
        .answer_utility(
            id,
            Outcome::Bytes {
                media: "image/jpeg".into(),
                bytes: photo.clone(),
            },
        )
        .expect("answered");
    let secret = c.application_secret.clone().expect("minted");
    let rhtn_client::ceremony::Msg::ResourceRequest { bytes, .. } = msg else {
        panic!()
    };
    let req = rhtn_archive::catalog::ResourceRequest::decode(&bytes).unwrap();
    let (_, ps) = parts(&req.message);
    let result = part(&ps, "result");
    assert!(result.1.contains("filename=\"result\""), "{}", result.1);
    assert!(
        result.1.contains("content-type: image/jpeg"),
        "{}",
        result.1
    );
    assert_eq!(result.2, photo);
    let digest: [u8; 32] = unhex(std::str::from_utf8(&part(&ps, "digest").2).unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(digest, rhtn_codec::cose::sha256(&photo));
    let key: [u8; 32] = unhex(std::str::from_utf8(&part(&ps, "key").2).unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let signature: [u8; 64] = unhex(std::str::from_utf8(&part(&ps, "claim").2).unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let at: u64 = std::str::from_utf8(&part(&ps, "at").2)
        .unwrap()
        .parse()
        .unwrap();
    let claim = Claim { key, signature };
    // **what the resource does with it**: the same array, and it holds
    assert!(verify_claim(
        &claim,
        &meter,
        Utility::Capture,
        "m-42",
        &digest,
        at
    ));
    // shown to another resource it says nothing: the audience is signed
    assert!(!verify_claim(
        &claim,
        &other,
        Utility::Capture,
        "m-42",
        &digest,
        at
    ));
    assert!(!verify_claim(
        &claim,
        &meter,
        Utility::Capture,
        "m-43",
        &digest,
        at
    ));
    // the key is this audience's and not the identity's
    assert_ne!(key, *c.public.ed.as_bytes(), "never the network identity");
    let theirs = application_key(&secret, &other);
    let (their_key, _) = rhtn_crypto::identity::sign_ed_seed(&theirs, b"x");
    assert_ne!(their_key, key, "another audience sees another key");
    // and the same audience sees the same key next time
    let (id, _) = c
        .take_utility_ask(node, meter, &ask_json("capture", ""))
        .unwrap();
    let msg = c
        .answer_utility(
            id,
            Outcome::Bytes {
                media: "image/jpeg".into(),
                bytes: photo.clone(),
            },
        )
        .unwrap();
    let rhtn_client::ceremony::Msg::ResourceRequest { bytes, .. } = msg else {
        panic!()
    };
    let (_, ps) = parts(
        &rhtn_archive::catalog::ResourceRequest::decode(&bytes)
            .unwrap()
            .message,
    );
    assert_eq!(part(&ps, "key").2, hex(&key).as_bytes());

    // a signature is over the exact payload the ask carried, and nothing
    // of the payload travels back: the resource has it
    let (id, ask) = c
        .take_utility_ask(node, meter, &ask_json("sign", ",\"payload\":\"00ff10\""))
        .unwrap();
    assert_eq!(ask.payload.as_deref(), Some(&[0x00u8, 0xff, 0x10][..]));
    let msg = c.answer_utility(id, Outcome::Approved).expect("signed");
    let rhtn_client::ceremony::Msg::ResourceRequest { bytes, .. } = msg else {
        panic!()
    };
    let (_, ps) = parts(
        &rhtn_archive::catalog::ResourceRequest::decode(&bytes)
            .unwrap()
            .message,
    );
    assert!(ps.iter().all(|(n, _, _)| n != "result"));
    let digest: [u8; 32] = unhex(std::str::from_utf8(&part(&ps, "digest").2).unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    assert_eq!(digest, rhtn_codec::cose::sha256(&[0x00, 0xff, 0x10]));
    let signature: [u8; 64] = unhex(std::str::from_utf8(&part(&ps, "claim").2).unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    let at: u64 = std::str::from_utf8(&part(&ps, "at").2)
        .unwrap()
        .parse()
        .unwrap();
    assert!(verify_claim(
        &Claim { key, signature },
        &meter,
        Utility::Sign,
        "m-42",
        &digest,
        at
    ));
    // a declined signature carries no claim
    let (id, _) = c
        .take_utility_ask(node, meter, &ask_json("sign", ",\"payload\":\"00\""))
        .unwrap();
    let msg = c
        .answer_utility(id, Outcome::Declined("not this".into()))
        .unwrap();
    let rhtn_client::ceremony::Msg::ResourceRequest { bytes, .. } = msg else {
        panic!()
    };
    let (_, ps) = parts(
        &rhtn_archive::catalog::ResourceRequest::decode(&bytes)
            .unwrap()
            .message,
    );
    assert!(ps.iter().all(|(n, _, _)| n != "claim" && n != "key"));
}

// acceptance: RSC-66
#[test]
fn the_application_secret_is_the_devices_and_survives_its_restart_but_not_a_backup() {
    let mut c = client("alice");
    let (id, _) = c
        .take_utility_ask([1; 32], [2; 32], &ask_json("sign", ",\"payload\":\"aa\""))
        .unwrap();
    c.answer_utility(id, Outcome::Approved).unwrap();
    let secret = c.application_secret.clone().expect("minted");

    // the device's own state carries it
    let blob = c.durable();
    let mut again = client("alice");
    again.restore_durable(&blob).expect("restored");
    assert_eq!(again.application_secret, Some(secret.clone()));

    // the backup does not: a device restored from its phrase signs under
    // a new key, which is the recovery policy
    let backup = c.store.encode();
    let store = rhtn_client::store::ClientStore::decode(&backup).expect("a store");
    assert_eq!(store, c.store, "the store round-trips without it");
    let mut fresh = client("alice");
    fresh.store = store;
    assert!(fresh.application_secret.is_none());

    // a blob written before the secret was kept still opens
    let old = client("alice");
    let blob = old.durable();
    assert!(old.application_secret.is_none());
    let mut reader = client("alice");
    reader
        .restore_durable(&blob)
        .expect("the preceding shape is read");
    assert!(reader.application_secret.is_none());
}

#[test]
fn a_result_is_bounded_by_the_request_frame_and_a_boundary_inside_a_part_is_redrawn() {
    let ask = parse(&ask_json("scan", "")).unwrap();
    let big = Outcome::Bytes {
        media: "application/pdf".into(),
        bytes: vec![0; RESULT_BYTES + 1],
    };
    assert_eq!(fits(&ask, &big), Err(Mismatch::TooLarge));
    let fine = Outcome::Bytes {
        media: "application/pdf".into(),
        bytes: vec![0; RESULT_BYTES],
    };
    assert_eq!(fits(&ask, &fine), Ok(()));
    let req = return_request(&ask, &fine, 1, None, &boundary(&[9; 12])).expect("fits the frame");
    assert!(req.len() <= rhtn_codec::bounds::REQUEST_FRAME_BYTES);

    let b = boundary(&[9; 12]);
    let poisoned = Outcome::Bytes {
        media: "text/plain".into(),
        bytes: format!("x\r\n--{b}y").into_bytes(),
    };
    assert_eq!(
        return_request(&ask, &poisoned, 1, None, &b),
        Err(Compose::Boundary)
    );
    assert!(return_request(&ask, &poisoned, 1, None, &boundary(&[8; 12])).is_ok());

    // shapes that do not fit their utility
    assert_eq!(fits(&ask, &Outcome::Approved), Err(Mismatch::Shape));
    assert_eq!(fits(&ask, &Outcome::Text("x".into())), Err(Mismatch::Shape));
    let tag = parse(&ask_json("tag", "")).unwrap();
    assert_eq!(fits(&tag, &Outcome::Text("A1".into())), Ok(()));
    assert_eq!(
        fits(
            &tag,
            &Outcome::Bytes {
                media: "text/plain".into(),
                bytes: b"A1".to_vec()
            }
        ),
        Ok(())
    );
    let notify = parse(&ask_json("notify", "")).unwrap();
    assert_eq!(fits(&notify, &Outcome::Approved), Ok(()));
    assert_eq!(fits(&notify, &Outcome::Declined("later".into())), Ok(()));
    assert_eq!(
        fits(&notify, &Outcome::Declined("".into())),
        Err(Mismatch::Text)
    );
    for m in [
        Mismatch::Shape,
        Mismatch::TooLarge,
        Mismatch::Precision,
        Mismatch::Text,
    ] {
        assert!(!m.say().is_empty());
    }
    assert_eq!(Utility::ALL.iter().filter(|u| u.signs()).count(), 2);
    for u in Utility::ALL {
        assert_eq!(Utility::of(u.name()), Some(u));
    }
}
