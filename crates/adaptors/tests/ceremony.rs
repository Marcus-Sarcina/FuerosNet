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
use rhtn_archive::record::Record;
use rhtn_client::device::ChannelKind;
use rhtn_client::record::{LABELS, read_presentation};
use rhtn_client::store::open;
use rhtn_codec::cbor::*;

/// The proximity field of a record's disclosure set, as (kind, result).
async fn proximity_of(m: &Meeting, who: &str, t: rhtn_archive::Txid) -> (Vec<(u64, u64)>, u64) {
    let v = m
        .party(who)
        .with(move |c| c.store.disclosures[&t][6].value.clone())
        .await;
    let item = parse_all(&v).unwrap();
    let Item::Map(map) = &item else {
        panic!("proximity map")
    };
    let strongest = as_uint(map_get(map, 2).unwrap()).unwrap();
    let Some(Item::Array(ch)) = map_get(map, 1) else {
        panic!("channels")
    };
    let kinds = ch
        .iter()
        .map(|c| {
            let Item::Map(cm) = c else { panic!() };
            (
                as_uint(map_get(cm, 1).unwrap()).unwrap(),
                as_uint(map_get(cm, 2).unwrap()).unwrap(),
            )
        })
        .collect();
    (kinds, strongest)
}

/// Two parties at their genesis beside a node: their first meeting is the
/// formation case, which needs no witness and no verifier, so the
/// conversation is the two of them and the record is what it reaches.
async fn pair(channels: &[ChannelKind]) -> Meeting {
    Meeting::open(
        vec![
            Who::with("alice", channels),
            Who::with("bob", channels),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
    )
    .await
}

// acceptance: CER-01
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_strongest_channel_the_hardware_has_is_what_the_record_says_was_achieved() {
    let mut m = pair(&[ChannelKind::Nfc, ChannelKind::Optical, ChannelKind::Latency]).await;
    let t = m.meet("alice", "bob", &[], &[]).await.expect("a record");
    for n in ["alice", "bob"] {
        assert_eq!(
            m.party(n).with(|c| c.archive.len()).await,
            1,
            "{n} holds the record"
        );
    }
    let (kinds, strongest) = proximity_of(&m, "alice", t).await;
    assert_eq!(
        strongest,
        ChannelKind::Nfc.code(),
        "NFC is the strongest achieved"
    );
    assert!(
        kinds.iter().all(|(k, _)| *k != ChannelKind::Uwb.code()),
        "no UWB entry"
    );
    assert_eq!(kinds, vec![(2, 0), (3, 0), (4, 0)]);
    // both participants hold the same set, and the presentation reads
    let mine = m
        .party("alice")
        .with(move |c| c.store.disclosures[&t].clone())
        .await;
    let theirs = m
        .party("bob")
        .with(move |c| c.store.disclosures[&t].clone())
        .await;
    assert_eq!(mine, theirs, "one set, held by both");
    let env = m.record("alice", t).await;
    let p = read_presentation(
        &ids(),
        &rhtn_client::record::present(&env, &mine, &["proximity"]),
    )
    .unwrap();
    assert_eq!(p.revealed.len(), 1);
}

// acceptance: CER-02
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_weaker_channel_is_never_presented_as_a_stronger_one() {
    let mut m = pair(&[ChannelKind::Optical]).await;
    let t = m.meet("alice", "bob", &[], &[]).await.expect("a record");
    let (kinds, strongest) = proximity_of(&m, "alice", t).await;
    assert_eq!(strongest, ChannelKind::Optical.code());
    assert_eq!(kinds, vec![(3, 0)], "optical and nothing ranked above it");
}

/// Two new joiners, nobody to nominate and nobody to ask: their first
/// meeting is a formation record, the one witnessless shape (design
/// §13.2; `wire-format.md` §3.2), and their second, no longer at genesis,
/// waits for a witness.  The first field run stopped here, on a
/// `NoWitness` that no horizon could ever answer [2026-10-04].
// acceptance: CER-50
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_new_joiners_form_a_record_with_no_witness_and_only_once() {
    let mut m = pair(&[ChannelKind::Nfc]).await;
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
    // the same two again: no longer at their genesis, no witness is a wait
    assert_eq!(
        m.meet("alice", "bob", &[], &[]).await.err(),
        Some(rhtn_client::ceremony::Abort::NoWitness),
        "a second witnessless meeting is not a formation"
    );
}

// acceptance: CER-06
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_client_holds_no_decryptable_likeness_and_no_sealing_key() {
    let mut m = pair(&[ChannelKind::Nfc]).await;
    let t = m.meet("alice", "bob", &[], &[]).await.expect("a record");
    let a_seed = m
        .party("alice")
        .with(move |c| c.store.seeds[&t].clone())
        .await;
    let k_capture =
        rhtn_client::keys::capture_key(&a_seed.seed, &kh("alice"), &kh("bob"), &a_seed.ceremony_id);
    // what bob holds: the sealed capture, the record, and bob's own seed
    let (sealed, b_seed) = m
        .party("bob")
        .with(move |c| {
            (
                c.store.sealed.get(&t).cloned(),
                c.store.seeds.get(&t).cloned(),
            )
        })
        .await;
    let sealed = sealed.expect("bob's sealed capture of alice");
    let b_seed = b_seed.expect("bob's seed for alice's captures of bob");
    assert_ne!(b_seed.seed, a_seed.seed);
    // and not: any frame, the template, alice's seed, or the key alice
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
            a_seed.seed.to_vec(),
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
    // opening without a grant: bob's own seed derives nothing that opens it
    let wrong =
        rhtn_client::keys::capture_key(&b_seed.seed, &kh("alice"), &kh("bob"), &a_seed.ceremony_id);
    assert!(open(&Default::default(), &wrong, &sealed).is_err());
    assert!(open(&Default::default(), &[0; 32], &sealed).is_err());
}

// acceptance: CER-23
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn captured_frames_are_stripped_of_metadata_before_sealing() {
    let mut m = pair(&[ChannelKind::Nfc]).await;
    let t = m.meet("alice", "bob", &[], &[]).await.expect("a record");
    let seed = m
        .party("alice")
        .with(move |c| c.store.seeds[&t].clone())
        .await;
    let key =
        rhtn_client::keys::capture_key(&seed.seed, &kh("alice"), &kh("bob"), &seed.ceremony_id);
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
            Who::with("alice", &[ChannelKind::Nfc]),
            Who::with("bob", &[ChannelKind::Nfc]),
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
    let mut m = pair(&[ChannelKind::Nfc]).await;
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
    assert_eq!(p.withheld.len(), 7);
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
            Who::with("alice", &[ChannelKind::Nfc]),
            Who::with("bob", &[ChannelKind::Nfc]),
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
            Who::with("alice", &[ChannelKind::Nfc]),
            Who::with("bob", &[ChannelKind::Nfc]),
            Who::watching("w1"),
            Who::watching("witness"),
        ],
        "witness",
        Scene::new().table("witness", &["witness"]),
        rhtn_client::ceremony::Config::default(),
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
    assert_ne!(
        first, second,
        "independent randomness, different prompt sequences"
    );
    // each of bob's sealed captures of alice opens under alice's key and
    // holds 3–5 frames spanning 10–15 s
    for t in [t1, t2] {
        let seed = m
            .party("alice")
            .with(move |c| c.store.seeds[&t].clone())
            .await;
        let key =
            rhtn_client::keys::capture_key(&seed.seed, &kh("alice"), &kh("bob"), &seed.ceremony_id);
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
        let span = cap.frames.last().unwrap().at_ms - cap.frames[0].at_ms;
        assert!((10_000..=15_000).contains(&span), "span {span} ms");
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
            Who::with("alice", &[ChannelKind::Nfc]),
            Who::with("bob", &[ChannelKind::Nfc]),
            Who::with("carol", &[ChannelKind::Nfc]),
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
    assert_eq!(copies.len(), 1, "one answer about alice: {copies:?}");
    let seed = m
        .party("alice")
        .with(move |c| c.store.seeds[&ab].clone())
        .await;
    let released =
        rhtn_client::keys::capture_key(&seed.seed, &kh("alice"), &kh("bob"), &seed.ceremony_id);
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
                .seeds
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
            Who::with("alice", &[ChannelKind::Nfc]),
            Who::with("bob", &[ChannelKind::Nfc]),
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
