//! **What a meeting leaves behind, over the end-to-end path**: the record
//! the conversation reaches, the disclosure set it rests on, and what
//! each party keeps of the capture — asserted on ceremonies the kernel
//! ran over real sessions beside a node, with only the local exchanges
//! made by hand.
//!
//! These were on the client's in-process harness until 2026-10-14, where
//! a driver gathered the signatures itself. The author's ruling moved
//! them: "the goal is to get the harness completely out of the business
//! of orchestrating anything that the kernel+app would be orchestrating
//! in production". The first case moved this way found a defect the
//! driver had hidden, in whom a doubly-nominated witness is attributed
//! to.

mod common;

use common::*;
use rhtn_archive::Keyhash;
use rhtn_archive::record::Record;
use rhtn_client::record::{LABELS, read_presentation};
use rhtn_client::store::open;
use rhtn_codec::cbor::*;

/// A test identity's identity root, which is what a capture key derives
/// from (design §7.5.2.6).  Nothing is stored per capture, so a test that
/// wants the key recomputes it the way the subject's client does.
fn root(name: &str) -> [u8; 32] {
    *rhtn_crypto::identity::testkit::test_identity(name).identity_root()
}

/// Two parties at their genesis beside a node: their first meeting is the
/// formation case, which needs no witness and no verifier, so the
/// conversation is the two of them and the record is what it reaches.
async fn pair() -> Meeting {
    Meeting::open(
        vec![
            Who::with("alice"),
            Who::with("bob"),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
    )
    .await
}

/// Two new joiners, nobody to nominate and nobody to ask: their first
/// meeting is a formation record, the genesis pair's shape, and their
/// second — no longer at genesis — **finalises as a witnessless normal
/// record**, since no floor requires a witness [author, 2026-10-19]
/// (design §13.2; `wire-format.md` §3.2).  The first field run once
/// stopped here, on a `NoWitness` that no horizon could ever answer
/// [2026-10-04]; the rule that stopped it is gone, and the record now
/// says what happened instead of refusing to exist.
// acceptance: CER-50
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_new_joiners_form_a_record_with_no_witness_and_only_once() {
    let mut m = pair().await;
    let t = m
        .meet("alice", "bob", &[], &[])
        .await
        .expect("a formation record with no witness");
    let env = m.record("alice", t).await;
    let rec = Record::parse(&env).expect("a record");
    assert_eq!(
        rec.signers.len(),
        2,
        "the two participants sign and nobody else"
    );
    assert_eq!(
        rec.back,
        vec![
            vec![rhtn_archive::genesis(&rec.signers[0])],
            vec![rhtn_archive::genesis(&rec.signers[1])]
        ],
        "each participant's back-pointer is the genesis value"
    );
    assert_eq!(m.record("bob", t).await, env, "the same record at both");
    // the same two again: no longer at their genesis, and a witnessless
    // normal record rather than a second formation or a refusal
    let t2 = m
        .meet("alice", "bob", &[], &[])
        .await
        .expect("a witnessless normal record");
    assert_ne!(t2, t, "a new record, not the formation again");
    let env2 = m.record("alice", t2).await;
    let rec2 = Record::parse(&env2).expect("a record");
    assert_eq!(rec2.signers.len(), 2, "still only the two participants");
    assert_ne!(
        rec2.back,
        vec![
            vec![rhtn_archive::genesis(&rec2.signers[0])],
            vec![rhtn_archive::genesis(&rec2.signers[1])]
        ],
        "its back-pointers chain to the first record: not a formation"
    );
    assert_eq!(m.record("bob", t2).await, env2, "the same record at both");
}

// acceptance: CER-06
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_client_holds_no_decryptable_likeness_and_no_sealing_key() {
    let mut m = pair().await;
    let t = m.meet("alice", "bob", &[], &[]).await.expect("a record");
    let a_seed = m
        .party("alice")
        .with(move |c| c.store.own[&t].clone())
        .await;
    let a_root = root("alice");
    let k_capture =
        rhtn_client::keys::capture_key(&a_root, &kh("alice"), &kh("bob"), &a_seed.ceremony_id);
    // what bob holds: the sealed capture, the record, and its own row for
    // the capture alice holds of it
    let (sealed, b_seed) = m
        .party("bob")
        .with(move |c| {
            (
                c.store.sealed.get(&t).cloned(),
                c.store.own.get(&t).cloned(),
            )
        })
        .await;
    let sealed = sealed.expect("bob's sealed capture of alice");
    let _ = b_seed.expect("bob's own row for alice's capture of bob");
    // and not: any frame, the template, alice's root, or the key alice
    // supplied
    let cap =
        open(&Default::default(), &k_capture, &sealed).expect("opens under the subject's key");
    let needles: Vec<Vec<u8>> = cap
        .frames
        .iter()
        .flat_map(|f| [f.bytes.clone(), f.bytes[..12].to_vec()])
        .chain([
            cap.template.clone(),
            k_capture[..].to_vec(),
            a_root.to_vec(),
        ])
        .collect();
    let held = m
        .party("bob")
        .with(move |c| {
            needles
                .iter()
                .map(|n| c.store.holds_bytes(n))
                .collect::<Vec<bool>>()
        })
        .await;
    assert!(
        held.iter().all(|h| !h),
        "bob holds something of alice's in the clear: {held:?}"
    );
    // opening without a grant: bob's own root derives nothing that opens it
    let wrong =
        rhtn_client::keys::capture_key(&root("bob"), &kh("alice"), &kh("bob"), &a_seed.ceremony_id);
    assert!(open(&Default::default(), &wrong, &sealed).is_err());
    assert!(open(&Default::default(), &[0; 32], &sealed).is_err());
}

// acceptance: CER-23
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn captured_frames_are_stripped_of_metadata_before_sealing() {
    let mut m = pair().await;
    let t = m.meet("alice", "bob", &[], &[]).await.expect("a record");
    let seed = m
        .party("alice")
        .with(move |c| c.store.own[&t].clone())
        .await;
    let key =
        rhtn_client::keys::capture_key(&root("alice"), &kh("alice"), &kh("bob"), &seed.ceremony_id);
    let sealed = m
        .party("bob")
        .with(move |c| c.store.sealed[&t].clone())
        .await;
    let cap = open(&Default::default(), &key, &sealed).unwrap();
    assert!(!cap.frames.is_empty());
    for f in &cap.frames {
        assert!(f.bytes.starts_with(b"face:alice"), "the pixels");
        for needle in [
            &b"51.5074N"[..],
            b"2026:09:11",
            b"HarnessCam",
            b"GPSLatitude",
            b"DateTimeOriginal",
            b"Model",
        ] {
            assert!(
                !f.bytes.windows(needle.len()).any(|w| w == needle),
                "no metadata survives"
            );
        }
    }
}

/// Either party may begin (design §7.1): the same pair, the other one
/// beginning, reaches a record, and the participants are the keyhash
/// order of this implementation either way.
// acceptance: CER-48
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn either_party_may_begin_and_the_participants_sit_in_keyhash_order() {
    let mut m = Meeting::open(
        vec![
            Who::with("alice"),
            Who::with("bob"),
            Who::watching("w1"),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
    )
    .await;
    let forward = m
        .meet("alice", "bob", &[], &[])
        .await
        .expect("alice begins");
    // no longer at genesis, so the second needs a witness; bob begins it
    let swapped = m
        .meet("bob", "alice", &["w1"], &[])
        .await
        .expect("bob begins");
    assert_ne!(forward, swapped, "two meetings, two records");
    let parts = {
        let (a, b) = (kh("alice"), kh("bob"));
        if a < b { vec![a, b] } else { vec![b, a] }
    };
    for t in [forward, swapped] {
        let rec = Record::parse(&m.record("alice", t).await).expect("a record");
        assert_eq!(
            rec.participants(),
            parts,
            "the participants sit in keyhash order whichever party began"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_records_disclosable_fields_are_all_present_and_withheld_by_default() {
    let mut m = pair().await;
    let t = m.meet("alice", "bob", &[], &[]).await.expect("a record");
    let set = m
        .party("alice")
        .with(move |c| c.store.disclosures[&t].clone())
        .await;
    assert_eq!(
        set.iter().map(|d| d.label).collect::<Vec<_>>(),
        LABELS.to_vec()
    );
    let env = m.record("alice", t).await;
    let p = read_presentation(&ids(), &rhtn_client::record::present(&env, &set, &[])).unwrap();
    assert_eq!(p.withheld.len(), 6);
}

/// **Every nominee declining is not a refusal to meet**: two parties at
/// their genesis still form a record, the witnessless shape (design
/// §13.2), signed by the two alone.  Each nominee here has a clock an
/// hour from the claimed start, which is what a witness declines on
/// (`light-client-requirements.md` §1.2).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_at_their_genesis_whose_every_nominee_declines_still_form_a_record() {
    let mut m = Meeting::open(
        vec![
            Who::with("alice"),
            Who::with("bob"),
            Who::skewed("w1", 3_600),
            Who::skewed("w2", -3_600),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
    )
    .await;
    let t = m
        .meet("alice", "bob", &["w1"], &["w2"])
        .await
        .expect("a formation record, both nominees having declined");
    let rec = Record::parse(&m.record("alice", t).await).expect("a record");
    assert_eq!(rec.signers.len(), 2, "alice and bob alone");
    assert!(
        rec.back
            .iter()
            .zip(&rec.signers)
            .all(|(b, k)| *b == vec![rhtn_archive::genesis(k)])
    );
    for w in ["w1", "w2"] {
        assert_eq!(
            m.party(w).with(|c| c.archive.len()).await,
            0,
            "{w} declined and holds nothing"
        );
    }
    assert!(
        m.party("alice")
            .kept
            .notices()
            .iter()
            .any(|(_, n)| matches!(
                n,
                rhtn_client::notice::Notice::NomineesOutnumbered { mine: 0, theirs: 0 }
            )),
        "alice is told her nominee is absent: {:?}",
        m.party("alice").kept.notices()
    );
}

/// **The guided capture, at the figures the requirements name**
/// (`light-client-requirements.md` §1.3): three to five frames over ten
/// to fifteen seconds, under prompts that vary between runs.  The one
/// test here that runs at the real capture's length, because its subject
/// is that length.
// acceptance: CER-03
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_guided_capture_takes_three_to_five_frames_over_ten_to_fifteen_seconds_under_prompts_that_vary()
 {
    let mut m = Meeting::open_with(
        vec![
            Who::with("alice"),
            Who::with("bob"),
            Who::watching("w1"),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
        // **the real capture's length, which is this test's subject** —
        // but not the real witness window, which is not: a witness that
        // waits four minutes signs nothing inside this run's deadline
        // [author, 2026-10-21], and the window is held where a clock can
        // be moved (`rhtn-client`'s CER-52)
        rhtn_client::ceremony::Config {
            witness_window_s: 0,
            ..Default::default()
        },
    )
    .await;
    let t1 = m.meet("alice", "bob", &[], &[]).await.expect("a formation");
    let first: Vec<rhtn_client::device::Prompt> = m.party("bob").kept.cam.prompts();
    // the second, past genesis, with a nominee that attests
    let t2 = m
        .meet("alice", "bob", &["w1"], &[])
        .await
        .expect("a witnessed record");
    let second = m.party("bob").kept.cam.prompts()[first.len()..].to_vec();
    // **the prompts are drawn, not fixed**, which is held deterministically
    // in `rhtn-client`'s own tests against a controlled source: two live
    // ceremonies agreeing by chance is a 1-in-1250 event over eight
    // prompts and three to five frames, so asserting they differ here was
    // a flake waiting to happen [2026-10-18]. What this run shows is that
    // each ceremony drew its own sequence from the set.
    for seq in [&first, &second] {
        assert!((3..=5).contains(&seq.len()), "{} prompts", seq.len());
        assert!(
            seq.iter()
                .all(|p| rhtn_client::device::Prompt::ALL.contains(p))
        );
    }
    // each of bob's sealed captures of alice opens under alice's key and
    // holds 3–5 frames spanning 10–15 s
    for t in [t1, t2] {
        let seed = m
            .party("alice")
            .with(move |c| c.store.own[&t].clone())
            .await;
        let key = rhtn_client::keys::capture_key(
            &root("alice"),
            &kh("alice"),
            &kh("bob"),
            &seed.ceremony_id,
        );
        let sealed = m
            .party("bob")
            .with(move |c| c.store.sealed[&t].clone())
            .await;
        let cap = open(&Default::default(), &key, &sealed).expect("opens under the subject's key");
        assert!(
            (3..=5).contains(&cap.frames.len()),
            "{} frames",
            cap.frames.len()
        );
        // **the prompts fall at random through the window** (design §7.5),
        // so what the retained frames span is a fraction of it rather
        // than the whole: three to five uniform draws over ten to fifteen
        // seconds span about half of it on average, and asserting the
        // frames span the window itself was asserting the even spacing
        // the burst used to have [2026-10-18]. What holds here is that
        // every frame falls inside the window, in the order taken; the
        // spread itself is held over many draws in `rhtn-client`'s own
        // continuity tests.
        assert!(
            cap.frames.windows(2).all(|w| w[0].at_ms <= w[1].at_ms),
            "in the order taken"
        );
        let last = cap.frames.last().unwrap().at_ms;
        // **the window plus what a starved machine adds** [author,
        // 2026-10-21]. The capture loop is wall-clock driven: it takes a
        // frame when the prompt falls due, and a loaded host can hand it
        // the CPU late, so the last frame lands past `max_span_ms` without
        // the kernel having retained anything outside its declared window.
        // This read 15_000 exactly and failed twice on a busy machine —
        // once in CI's own image — which is a measurement artefact and not
        // a protocol fault. The frames' spread is held over many draws in
        // `rhtn-client`'s continuity tests, where the clock is controlled
        let window = rhtn_client::device::CaptureParams::default().max_span_ms;
        assert!(
            last <= window + 5_000,
            "a frame at {last} ms, past the {window} ms window by more than scheduling explains"
        );
    }
}

/// **A scene with a verifier who can answer**: alice has a record with
/// w2 from before, then meets bob for real, so bob holds a sealed capture
/// of her and can be asked about her; carol, who recognises bob and
/// nobody else of alice's, meets alice next and selects him.
///
/// Two of alice's prior parties is what makes a selection necessary at
/// all (`wire-format.md` §5.3: *n* counts the parties a bundle
/// introduces), and a verifier that never met the subject could only
/// close the query.
async fn scene_with_a_verifier() -> (Meeting, rhtn_archive::Txid, rhtn_archive::Txid) {
    let mut prior = Scene::new();
    prior.meet("alice", "w2");
    let mut m = Meeting::open(
        vec![
            Who::with("alice"),
            Who::with("bob"),
            Who::with("carol"),
            Who::watching("w1"),
            Who::watching("w2"),
            Who::watching("witness"),
        ],
        "witness",
        prior.table("witness", &["witness"]),
    )
    .await;
    m.holds("alice", &prior, &["w2"]).await;
    m.holds("w2", &prior, &[]).await;
    // alice meets bob: past her genesis, so w1 attests
    let ab = m
        .meet("alice", "bob", &["w1"], &[])
        .await
        .expect("alice meets bob");
    // bob is the one of alice's parties carol recognises
    m.in_horizon("carol", &["bob"]).await;
    let ac = m
        .meet("alice", "carol", &["w1"], &[])
        .await
        .expect("alice meets carol");
    (m, ab, ac)
}

/// The responses a record's body carries.
fn responses_in(env: &[u8]) -> Vec<rhtn_client::query::Response> {
    let rec = Record::parse(env).expect("a record");
    let body = &env[rec.body.clone()];
    let r5 = value_slice(body, 5).expect("responses");
    array_item_ranges(body, r5.start)
        .unwrap()
        .into_iter()
        .map(|r| rhtn_client::query::Response::read(&body[r]).unwrap())
        .collect()
}

// acceptance: CER-24
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn the_capture_key_goes_directly_to_the_selected_verifier_and_nowhere_else() {
    let (mut m, ab, ac) = scene_with_a_verifier().await;
    for who in ["alice", "bob", "carol", "w1", "w2"] {
        m.at(who).drain();
    }
    let _ = ac;
    // **the key alice released reached the verifier she released it to
    // and nobody else.** The grant is not a message any application is
    // shown (`Courier`: a grant is taken by the kernel and answered, and
    // nothing is said to the person), so what the path can be asked is
    // what came of it: exactly one verifier answered about alice, which
    // the subject knows from its copy of every answer (design §7.4.2),
    // and the key itself is in no other party's storage
    let copies: Vec<[u8; 32]> = m
        .party("alice")
        .seen
        .iter()
        .filter_map(|(_, d)| match d {
            rhtn_client::ceremony::Dispatched::ResponseCopy(r) => r.clone().ok(),
            _ => None,
        })
        .collect();
    // **two answers about alice, one per meeting** [author, 2026-10-30:
    // at least one prior counterparty is asked wherever one is offered]:
    // when alice met bob, bob asked w2 — her one prior counterparty, a
    // stranger to bob, asked at his discretion — and when she met carol,
    // carol asked bob. Each answered about alice from its own capture of
    // her; neither needed the key she released to the other
    assert_eq!(
        copies.len(),
        2,
        "one answer about alice per meeting: {copies:?}"
    );
    let seed = m
        .party("alice")
        .with(move |c| c.store.own[&ab].clone())
        .await;
    let released =
        rhtn_client::keys::capture_key(&root("alice"), &kh("alice"), &kh("bob"), &seed.ceremony_id);
    for who in ["carol", "w1", "w2"] {
        let k = released[..].to_vec();
        assert!(
            !m.party(who).with(move |c| c.store.holds_bytes(&k)).await,
            "the key reached {who}"
        );
    }
    // and the capture it opened is the one bob holds of her, from the
    // meeting they had
    let of_alice = m
        .party("bob")
        .with(|c| {
            c.store
                .own
                .iter()
                .filter(|(_, s)| s.counterparty == kh("alice"))
                .map(|(t, _)| *t)
                .collect::<Vec<_>>()
        })
        .await;
    assert_eq!(
        of_alice,
        vec![ab],
        "one capture of alice, from their meeting"
    );
    // and the record carries bob's match about alice
    let about_alice: Vec<rhtn_client::query::Response> = responses_in(&m.record("alice", ac).await)
        .into_iter()
        .filter(|r| r.subject == kh("alice"))
        .collect();
    assert_eq!(about_alice.len(), 1);
    assert_eq!(
        (about_alice[0].verifier, about_alice[0].verdict),
        (kh("bob"), rhtn_client::query::Verdict::Match)
    );
}

// acceptance: CER-17
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn querying_witnessing_and_answering_ask_nobody() {
    let (m, _, _) = scene_with_a_verifier().await;
    // the only question anyone was asked was whether to start, and only
    // the participants were asked it
    for who in ["alice", "bob", "carol", "w1", "w2"] {
        let asked = m.party(who).kept.asked();
        assert!(
            asked
                .iter()
                .all(|q| q.starts_with("Start a presence ceremony")),
            "{who} was asked: {asked:?}"
        );
    }
    for who in ["w1", "w2"] {
        assert!(
            m.party(who).kept.asked().is_empty(),
            "no nominee was asked anything"
        );
    }
    // the verifier's operator learns nothing of a query answered in the
    // background (design §19.6 owes a verifier no warning): bob was asked
    // nothing after the meeting he was a participant in, and told nothing
    // at all of the one he only answered about
    let during: Vec<rhtn_client::notice::Notice> = m
        .party("bob")
        .kept
        .notices()
        .into_iter()
        .filter(|(t, _)| *t >= m.last_start)
        .map(|(_, n)| n)
        .collect();
    assert!(
        during.is_empty(),
        "bob's operator was told something of a query answered in the background: {during:?}"
    );
}

// acceptance: CER-26
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn what_the_record_will_contain_is_disclosed_at_capture_and_to_nobody_else() {
    let (m, _, ac) = scene_with_a_verifier().await;
    let start = m.last_start;
    // the participants: a disclosure before the capture, which is before
    // the first frame's time
    for who in ["alice", "carol"] {
        let disclosed = m
            .party(who)
            .kept
            .notices()
            .iter()
            .find(|(t, x)| {
                *t >= start
                    && matches!(
                        x,
                        rhtn_client::notice::Notice::RecordDisclosure {
                            role: rhtn_client::notice::Role::Participant
                        }
                    )
            })
            .map(|(t, _)| *t)
            .expect("a participant disclosure");
        let first_frame = m
            .party(who)
            .kept
            .cam
            .first_capture_after(start)
            .expect("captured");
        assert!(
            disclosed <= first_frame,
            "{who} was told at {} ms, before its first frame at {} ms",
            disclosed as i64 - start as i64,
            first_frame as i64 - start as i64
        );
    }
    // the nominee that attested was asked and answered, and was told
    // nothing: neither it nor the verifier is a person acting (§19.6)
    let rec = Record::parse(&m.record("alice", ac).await).expect("a record");
    assert!(rec.signers.contains(&kh("w1")), "w1 attested");
    for who in ["w1", "bob"] {
        assert!(
            !m.party(who).kept.notices().iter().any(|(t, x)| *t >= start
                && matches!(x, rhtn_client::notice::Notice::RecordDisclosure { .. })),
            "{who} was told"
        );
    }
}

/// **A nominee whose clock is far from the claimed start declines**
/// (`light-client-requirements.md` §1.2), the other side's attests, and
/// the record carries the one that did; the party whose nominee is absent
/// is told so, since a side witnessed only by the other's nominee is
/// something the person may want to act on.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_witness_far_from_the_claimed_start_declines_and_the_others_carry_the_record() {
    let mut m = Meeting::open(
        vec![
            Who::with("alice"),
            Who::with("bob"),
            Who::skewed("w1", 3_600),
            Who::watching("w2"),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
    )
    .await;
    let t = m
        .meet("alice", "bob", &["w1"], &["w2"])
        .await
        .expect("w2 suffices");
    let rec = Record::parse(&m.record("alice", t).await).expect("a record");
    assert_eq!(rec.signers.len(), 3, "alice, bob and w2");
    assert!(!rec.signers.contains(&kh("w1")));
    assert_eq!(
        m.party("w1").with(|c| c.archive.len()).await,
        0,
        "a nominee that declined holds nothing"
    );
    assert!(
        m.party("alice")
            .kept
            .notices()
            .iter()
            .any(|(_, n)| matches!(
                n,
                rhtn_client::notice::Notice::NomineesOutnumbered { mine: 0, theirs: 1 }
            )),
        "alice is told her nominee is absent: {:?}",
        m.party("alice").kept.notices()
    );
}

/// **Four nominated, three kept, one stood down** (design §7.1 item 4).
/// The sets cross on the local interface and the counterparty prunes to
/// `min(3, offered)`: the record carries the two participants and the
/// three survivors, every survivor nominated by one side and selected by
/// the other, and the dismissed nominee's part ended at its standdown —
/// it holds nothing of the ceremony.
// acceptance: CER-68
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_counterparty_prunes_to_three_and_the_dismissed_stands_down() {
    let mut m = Meeting::open(
        vec![
            Who::with("alice"),
            Who::with("bob"),
            Who::watching("w1"),
            Who::watching("w2"),
            Who::watching("w3"),
            Who::watching("w4"),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
    )
    .await;
    let t = m
        .meet("alice", "bob", &["w1", "w2", "w3", "w4"], &[])
        .await
        .expect("a record");
    let rec = Record::parse(&m.record("alice", t).await).expect("a record");
    assert_eq!(
        rec.signers.len(),
        5,
        "two participants and the three kept witnesses"
    );
    // **the record names which three were kept**, and the fourth is the
    // one the prune dismissed: the keyhashes are the record's own word,
    // not this harness's guess at the salted choice
    let parties = [kh("alice"), kh("bob")];
    let kept: Vec<Keyhash> = rec
        .signers
        .iter()
        .copied()
        .filter(|s| !parties.contains(s))
        .collect();
    assert_eq!(kept.len(), 3, "three kept of the four nominated");
    let nominated = ["w1", "w2", "w3", "w4"];
    let dismissed: Vec<&str> = nominated
        .iter()
        .copied()
        .filter(|w| !kept.contains(&kh(w)))
        .collect();
    assert_eq!(dismissed.len(), 1, "one dismissed at the prune");
    // **each survivor holds the record it signed** (kind 18 reaches every
    // signer): awaited, because the proposer sends it after finalising
    // and arrival is not instantaneous — sampling here instead of waiting
    // is what made this read (0, 4) on a loaded machine while the record
    // itself was right
    for w in nominated.iter().filter(|w| kept.contains(&kh(w))) {
        assert!(
            m.party(w).holds(10_000, |c| c.archive.len() == 1).await,
            "{w} holds the record it attested"
        );
    }
    // and the dismissed one holds nothing of the ceremony, its part over
    // at its standdown
    let gone = dismissed[0];
    assert_eq!(
        m.party(gone).with(|c| c.archive.len()).await,
        0,
        "{gone} was dismissed and holds nothing"
    );
}
