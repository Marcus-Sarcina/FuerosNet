//! **The two halves of the utility contract, held against each other**
//! (`light-client-requirements.md` §8.1; `resource-requirements.md` §3.4).
//!
//! The application's asks are parsed by **the kernel's own reader**, and
//! the answers are composed by **the kernel's own composer** and parsed
//! back here. So this walks the whole sequence the bench walks, without a
//! phone: what it cannot say is whether a camera, a radio or an unlock
//! works, and what it does say is that every packet either side sends is
//! one the other side reads.

use rhtn_client::utility::{self, Outcome};
use rhtn_testapp::{App, Claimed, STEPS, parse_answer};

/// The resource's keyhash, which every claim is addressed to.
const ME: [u8; 32] = [0x5a; 32];

/// A device's application secret, as the kernel mints one.
const SECRET: [u8; 32] = [0x11; 32];

fn get(app: &mut App, who: &str) -> (u16, String, Vec<u8>) {
    let request = format!(
        "GET / HTTP/1.1\r\nhost: testapp.internal\r\nrhtn-principal: {who}\r\nrhtn-roles: reader\r\n\r\n"
    );
    answered(app.handle(request.as_bytes()))
}

fn post(app: &mut App, who: &str, request: &[u8]) -> (u16, String, Vec<u8>) {
    // the kernel composes the request with `host: resource`; the node
    // inserts the credential headers, which is what this stands in for
    let text = String::from_utf8_lossy(request).to_string();
    let with = text.replacen(
        "host: resource\r\n",
        &format!("host: testapp.internal\r\nrhtn-principal: {who}\r\nrhtn-roles: reader\r\n"),
        1,
    );
    answered(app.handle(with.as_bytes()))
}

fn answered(out: Vec<u8>) -> (u16, String, Vec<u8>) {
    let end = out
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("an end of headers");
    let head = String::from_utf8_lossy(&out[..end]).to_string();
    let status: u16 = head
        .lines()
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let media = head
        .to_ascii_lowercase()
        .lines()
        .find_map(|l| {
            l.strip_prefix("content-type:")
                .map(|v| v.trim().to_string())
        })
        .unwrap_or_default();
    (status, media, out[end + 4..].to_vec())
}

/// What a phone would answer for each utility, in the order [`STEPS`] asks
/// them: the two the shell cannot give are declined with the words it
/// declines them with.
fn answer_for(ask: &utility::Ask) -> Outcome {
    match (ask.utility.name(), ask.media.first().map(String::as_str)) {
        ("notify", _) => Outcome::Approved,
        ("tag", _) => Outcome::Text("A1-7734".into()),
        ("check-in", _) => Outcome::Text("counter-3".into()),
        ("location", _) => Outcome::Text("u4pruyd".into()),
        ("capture", _) => Outcome::Bytes {
            media: "image/jpeg".into(),
            bytes: b"JFIF the meter at 41821".to_vec(),
        },
        ("scan", Some("image/jpeg")) => Outcome::Bytes {
            media: "image/jpeg".into(),
            bytes: b"JFIF the delivery note".to_vec(),
        },
        // a scan a resource will take only as a PDF: the shell can
        // photograph and not read, and says so
        ("scan", Some("application/pdf")) => Outcome::Declined(
            "this app can photograph the document but not read it into application/pdf".into(),
        ),
        ("sign", _) => Outcome::Approved,
        ("import", _) => Outcome::Bytes {
            media: "image/png".into(),
            bytes: b"\x89PNG the one they had".to_vec(),
        },
        ("live", _) => {
            Outcome::Declined("this app carries no live audio or video; it is not built yet".into())
        }
        ("export", _) => Outcome::Approved,
        (other, _) => panic!("the sequence asked for {other}, which this walk has no answer for"),
    }
}

/// The request the kernel composes for one answer, claim and all.
fn compose(ask: &utility::Ask, outcome: &Outcome, at: u64) -> Vec<u8> {
    utility::fits(ask, outcome).expect("the outcome answers the ask");
    let claim = if ask.utility.signs() && !matches!(outcome, Outcome::Declined(_)) {
        let digest = utility::claimed(ask, outcome).expect("something to claim");
        Some((
            digest,
            utility::claim(&SECRET, &ME, ask.utility, &ask.token, &digest, at),
        ))
    } else {
        None
    };
    utility::return_request(
        ask,
        outcome,
        at,
        claim.as_ref().map(|(d, c)| (d, c)),
        &utility::boundary(&[7; 12]),
    )
    .expect("one request")
}

// acceptance: RSC-66
#[test]
fn the_whole_sequence_walks_and_every_claim_verifies_at_the_resource() {
    let mut app = App::new(ME);
    let who = "aa".repeat(32);

    // the first request is answered with the first ask, under the media
    // type that makes it one
    let (status, media, body) = get(&mut app, &who);
    assert_eq!(status, 200);
    assert_eq!(media, rhtn_testapp::ASK_MEDIA);

    let mut body = body;
    let mut at = 1_700_000_000_000u64;
    for (i, step) in STEPS.iter().enumerate() {
        // **the kernel reads what the application wrote**
        let ask = utility::parse(&body).unwrap_or_else(|m| {
            panic!("the kernel refused the ask for {}: {}", step.token, m.say())
        });
        assert_eq!(ask.utility.name(), step.utility);
        assert_eq!(ask.token, step.token);
        assert_eq!(ask.return_path, rhtn_testapp::return_path(step.token));
        assert!(!ask.purpose.is_empty() && !ask.retention.is_empty());

        let outcome = answer_for(&ask);
        at += 1_000;
        let request = compose(&ask, &outcome, at);
        let (status, media, next) = post(&mut app, &who, &request);
        assert_eq!(status, 200, "step {} {}", i, step.token);
        body = next;
        if i + 1 < STEPS.len() {
            assert_eq!(media, rhtn_testapp::ASK_MEDIA, "the next ask follows");
        } else {
            // **the last answer is the file**, declared as what it is so a
            // client offers to save it rather than drawing it
            assert_eq!(media, "text/csv");
            let text = String::from_utf8_lossy(&body).to_string();
            assert!(
                text.starts_with("utility,outcome,claim,what,bytes\n"),
                "{text}"
            );
            assert!(text.contains("capture,given,verified"), "{text}");
            assert!(text.contains("sign,given,verified"), "{text}");
            assert!(text.contains("live,declined"), "{text}");
            assert!(
                text.contains("application/pdf"),
                "the PDF refusal is in it: {text}"
            );
            assert!(!text.contains("failed:"), "a claim did not verify: {text}");
        }
    }

    // and the walk's record stands for a later look
    let (status, media, page) = get(&mut app, &who);
    assert_eq!(status, 200);
    assert_eq!(media, "text/html");
    let page = String::from_utf8_lossy(&page).to_string();
    assert!(page.contains("verified"), "{page}");
    assert!(!page.contains("FAILED"), "{page}");
    assert_eq!(app.walks[&who].taken.len(), STEPS.len());

    // another principal walks its own sequence from the start
    let (_, media, body) = get(&mut app, &"bb".repeat(32));
    assert_eq!(media, rhtn_testapp::ASK_MEDIA);
    assert_eq!(utility::parse(&body).unwrap().token, STEPS[0].token);
}

// acceptance: RSC-66
#[test]
fn a_claim_for_another_audience_or_over_other_bytes_is_refused_here() {
    let mut app = App::new(ME);
    let who = "cc".repeat(32);
    // walk to the capture, which is the first step that signs
    let mut body = get(&mut app, &who).2;
    let mut at = 1_700_000_000_000u64;
    let mut capture = None;
    for step in STEPS.iter() {
        let ask = utility::parse(&body).expect("an ask");
        if step.utility == "capture" {
            capture = Some(ask);
            break;
        }
        at += 1_000;
        let outcome = answer_for(&ask);
        body = post(&mut app, &who, &compose(&ask, &outcome, at)).2;
    }
    let ask = capture.expect("the sequence asks for a capture");
    let outcome = Outcome::Bytes {
        media: "image/jpeg".into(),
        bytes: b"JFIF the meter".to_vec(),
    };
    let digest = utility::claimed(&ask, &outcome).expect("a digest");

    // **a claim made out to another resource says nothing here**
    let elsewhere = [0x77u8; 32];
    let theirs = utility::claim(&SECRET, &elsewhere, ask.utility, &ask.token, &digest, at);
    let request = utility::return_request(
        &ask,
        &outcome,
        at,
        Some((&digest, &theirs)),
        &utility::boundary(&[3; 12]),
    )
    .unwrap();
    let kind = "multipart/form-data; boundary=".to_string() + &utility::boundary(&[3; 12]);
    let got = parse_answer(&kind, body_of(&request)).expect("the answer parses");
    let step = STEPS.iter().find(|s| s.utility == "capture").unwrap();
    assert!(
        matches!(rhtn_testapp::judge(&ME, step, &got), Claimed::Failed(ref why) if why.contains("verify")),
        "a claim for another audience must not verify here"
    );

    // **and one over other bytes**: the digest is checked against what
    // came back, not taken on trust
    let mine = utility::claim(&SECRET, &ME, ask.utility, &ask.token, &[9u8; 32], at);
    let request = utility::return_request(
        &ask,
        &outcome,
        at,
        Some((&[9u8; 32], &mine)),
        &utility::boundary(&[4; 12]),
    )
    .unwrap();
    let kind = "multipart/form-data; boundary=".to_string() + &utility::boundary(&[4; 12]);
    let got = parse_answer(&kind, body_of(&request)).expect("the answer parses");
    assert!(
        matches!(rhtn_testapp::judge(&ME, step, &got), Claimed::Failed(ref why) if why.contains("digest")),
        "the digest must be over what came back"
    );

    // a claim on a utility that signs nothing is a defect, not a bonus
    let tag = STEPS.iter().find(|s| s.utility == "tag").unwrap();
    let mut odd = got.clone();
    odd.utility = "tag".into();
    odd.token = tag.token.into();
    assert!(matches!(
        rhtn_testapp::judge(&ME, tag, &odd),
        Claimed::Failed(_)
    ));
}

fn body_of(request: &[u8]) -> &[u8] {
    let end = request.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    &request[end + 4..]
}

#[test]
fn an_answer_that_crosses_the_walk_is_refused_and_the_walk_stands() {
    let mut app = App::new(ME);
    let who = "dd".repeat(32);
    let body = get(&mut app, &who).2;
    let ask = utility::parse(&body).expect("the first ask");
    assert_eq!(ask.token, "t1-notify");

    // an answer naming a step this walk is not on
    let crossed = String::from_utf8_lossy(&compose(&ask, &Outcome::Approved, 1))
        .replacen("/u/t1-notify", "/u/t7-sign", 1)
        .into_bytes();
    let (status, _, said) = post(&mut app, &who, &crossed);
    assert_eq!(status, 409);
    assert!(String::from_utf8_lossy(&said).contains("t1-notify"));

    // and the walk has not moved
    let again = utility::parse(&get(&mut app, &who).2).expect("the same ask");
    assert_eq!(again.token, "t1-notify");

    // a body that is not an answer at all
    let nonsense = b"POST /u/t1-notify HTTP/1.1\r\nhost: resource\r\ncontent-type: text/plain\r\ncontent-length: 2\r\n\r\nhi".to_vec();
    let (status, _, _) = post(&mut app, &who, &nonsense);
    assert_eq!(status, 400);

    // a method this resource does not take
    let (status, _, _) =
        answered(app.handle(b"PUT / HTTP/1.1\r\nhost: t\r\nrhtn-principal: x\r\n\r\n"));
    assert_eq!(status, 405);
}

#[test]
fn every_ask_in_the_sequence_is_one_the_kernel_reads() {
    // the sequence is the ten utilities, with the two the shell cannot
    // give among them on purpose
    let asked: Vec<&str> = STEPS.iter().map(|s| s.utility).collect();
    for utility in utility::Utility::ALL {
        assert!(
            asked.contains(&utility.name()),
            "the sequence never asks for {}",
            utility.name()
        );
    }
    for step in STEPS {
        let ask = utility::parse(rhtn_testapp::ask(step).as_bytes())
            .unwrap_or_else(|m| panic!("{}: {}", step.token, m.say()));
        assert_eq!(ask.utility.name(), step.utility);
        if step.utility == "sign" {
            assert_eq!(
                ask.payload.as_deref(),
                step.payload.map(|p| p.as_bytes()),
                "a signature's payload is the exact bytes"
            );
        }
    }
    // the tokens are distinct, which is what makes a crossed answer visible
    let mut tokens: Vec<&str> = STEPS.iter().map(|s| s.token).collect();
    tokens.sort_unstable();
    let before = tokens.len();
    tokens.dedup();
    assert_eq!(before, tokens.len(), "two steps share a token");
}
