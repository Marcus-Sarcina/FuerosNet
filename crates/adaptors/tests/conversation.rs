//! The ceremony's conversation over the end-to-end path (`wire-format.md`
//! §7.10.1 kinds 9 to 18; design §7.1; functional row NET-022): two
//! participants and their witnesses beside one node, every message after
//! the local exchanges carried by the courier through the sessions the
//! query path uses, none moved by hand.  The local exchanges themselves
//! (§14.3) are made by hand here, which is where the design makes them.

mod common;

use common::*;
use rhtn_adaptors::actor::Handle;
use rhtn_adaptors::courier::Courier;
use rhtn_adaptors::direct::{NoDirect, Reachable};
use rhtn_adaptors::serving::{Inboxes, LocalNode, Serving};
use rhtn_archive::record::Record;
use rhtn_archive::tx::Witness;
use rhtn_archive::{Keyhash, Txid};
use rhtn_client::ceremony::{
    Abort, Client, Config, Dispatched, Msg, Proposed, conversation_payload,
};
use rhtn_client::conversation::{BackPointers, proposal_from_body};
use rhtn_client::device::{CaptureParams, ChannelKind};
use rhtn_client::payload::KIND_BACK_POINTERS;
use rhtn_client::query::{Response, Verdict};
use rhtn_client::record::Refusal;
use rhtn_client::sequence::Conversed;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedReceiver;

struct Party {
    handle: Handle,
    courier: Arc<Courier>,
    app: UnboundedReceiver<(Keyhash, Dispatched)>,
    /// Everything that arrived, in order, once drained.
    seen: Vec<(Keyhash, Dispatched)>,
}

impl Party {
    async fn with<R: Send + 'static>(
        &self,
        f: impl FnOnce(&mut Client) -> R + Send + 'static,
    ) -> R {
        self.handle.with(f).await
    }

    /// Take a step on the client and carry what it returns.
    async fn step(
        &self,
        f: impl FnOnce(&mut Client) -> Result<Vec<rhtn_client::ceremony::Msg>, Abort> + Send + 'static,
    ) -> Result<(), Abort> {
        let msgs = self.handle.with(f).await?;
        let carried = self.courier.carry(msgs).await;
        assert!(carried.complete(), "{carried:?}");
        Ok(())
    }

    /// Poll `f` on the client until it holds or `ms` elapse.
    async fn holds(
        &self,
        ms: u64,
        f: impl Fn(&mut Client) -> bool + Send + Sync + 'static,
    ) -> bool {
        let f = Arc::new(f);
        let end = Instant::now() + Duration::from_millis(ms);
        loop {
            let g = f.clone();
            if self.handle.with(move |c| g(c)).await {
                return true;
            }
            if Instant::now() >= end {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// Take whatever has arrived, without waiting.
    fn drain(&mut self) {
        while let Ok(e) = self.app.try_recv() {
            self.seen.push(e);
        }
    }

    /// What the conversation came to here, by sender, in arrival order.
    fn conversed(&self) -> Vec<(Keyhash, Conversed)> {
        self.seen
            .iter()
            .filter_map(|(k, d)| match d {
                Dispatched::Conversation(c) => Some((*k, c.clone())),
                _ => None,
            })
            .collect()
    }
}

/// A light client beside the node with no direct path, on a device with
/// `channels` and a clock `skew_s` from the wall.
fn host(
    name: &'static str,
    serving: &Arc<dyn Serving>,
    inboxes: &Inboxes,
    channels: Vec<ChannelKind>,
    skew_s: i64,
) -> Party {
    let reachable = Reachable::default();
    let handle = spawn_client_with(name, cfg(), reachable.clone(), channels, skew_s);
    let (courier, app) = Courier::new(
        handle.clone(),
        serving.clone(),
        Arc::new(NoDirect(reachable)),
    );
    inboxes.host(kh(name), courier.inbound());
    Party {
        handle,
        courier,
        app,
        seen: Vec::new(),
    }
}

/// The client's numbers for a quick run: a short grant buffer, and a
/// capture over a fraction of a second.  The capture's own figures are
/// CER-03's to test.
fn cfg() -> Config {
    let mut c = Config::default();
    c.verifier.grant_buffer_ms = 300;
    c.capture = CaptureParams {
        min_frames: 3,
        max_frames: 3,
        min_span_ms: 20,
        max_span_ms: 20,
    };
    c
}

fn kinds_from(seen: &[(Keyhash, Conversed)], from: &str) -> Vec<u64> {
    seen.iter()
        .filter(|(k, _)| *k == kh(from))
        .filter_map(|(_, c)| match c {
            Conversed::Observed { kind } => Some(*kind),
            _ => None,
        })
        .collect()
}

// acceptance: CER-22
// acceptance: CER-14
// acceptance: CER-17
// acceptance: CER-45
// acceptance: CER-46
// acceptance: CER-47
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_conversation_runs_over_the_payload_path_to_a_record_every_signer_holds() {
    // alice met w1 twice before, so bob's selection of alice's verifiers
    // from her bundle has n = 2 and one candidate: w1 is asked about her
    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut scene = Scene::starting(now_s - 30 * 86_400);
    scene.meet("alice", "w1");
    scene.meet("alice", "w1");
    // the node, "witness", serving everyone beside it; alice and bob the
    // participants; w1 alice's nominee, w2 bob's, its clock an hour out,
    // so it declines (`light-client-requirements.md` §1.2); carol nobody's
    let node = live_node(
        "witness",
        scene.table("witness", &["witness"]),
        "witness",
        &[],
    );
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    let optical = vec![ChannelKind::Optical];
    let mut alice = host("alice", &serving, &inboxes, optical.clone(), 0);
    let mut bob = host("bob", &serving, &inboxes, optical, 0);
    let mut w1 = host("w1", &serving, &inboxes, vec![], 0);
    let mut w2 = host("w2", &serving, &inboxes, vec![], 3600);
    let carol = host("carol", &serving, &inboxes, vec![], 0);
    let population: Vec<Keyhash> = ["alice", "bob", "w1", "w2", "carol"]
        .iter()
        .map(|n| kh(n))
        .collect();
    for p in [&alice, &bob, &w1, &w2, &carol] {
        p.courier.attach(population.clone()).await;
    }
    for p in [&alice, &bob, &w1, &w2, &carol] {
        p.courier.sweep(population.clone()).await;
    }
    let bundle: Vec<(Txid, Vec<u8>)> = scene
        .records
        .iter()
        .map(|r| (r.txid, r.bytes.clone()))
        .collect();
    alice
        .with(move |c| {
            for (t, b) in bundle {
                c.store.records.insert(t, b);
            }
        })
        .await;

    // the local exchanges, by hand (§14.3): intent, proximity, capture
    let ia = alice
        .with(|c| c.begin(kh("bob"), vec![kh("w1")], true))
        .await
        .expect("alice begins");
    let ib = bob
        .with(|c| c.begin(kh("alice"), vec![kh("w2")], false))
        .await
        .expect("bob begins");
    let cid = alice
        .with(move |c| c.take_intent(kh("bob"), &ib))
        .await
        .expect("alice takes bob's intent");
    let cid_b = bob
        .with(move |c| c.take_intent(kh("alice"), &ia))
        .await
        .expect("bob takes alice's intent");
    assert_eq!(cid, cid_b, "one ceremony");
    let ch = alice.with(|c| c.proximity()).await.expect("channels");
    bob.with(move |c| c.take_channels(&ch))
        .await
        .expect("bob agrees");
    let ka = alice.with(|c| c.capture_key()).await.expect("alice's key");
    let kb = bob.with(|c| c.capture_key()).await.expect("bob's key");
    let (ra, rb) = tokio::join!(
        alice.with(move |c| c.capture(kb)),
        bob.with(move |c| c.capture(ka))
    );
    ra.expect("alice captures bob");
    rb.expect("bob captures alice");

    // from here nothing is moved by hand.  The conversation opens: each
    // asks its nominee and hands over its back-pointers, to the
    // counterparty and both witnesses (kinds 12 and 15)
    for p in [&alice, &bob] {
        p.step(|c| c.converse_open()).await.expect("opens");
    }
    // step 6: each side's consent requests, kind 9 to the subject alone;
    // bob's names w1 about alice, alice has nobody to ask about bob
    for p in [&alice, &bob] {
        p.step(|c| c.converse_queries()).await.expect("queries");
    }
    assert_eq!(
        bob.with(|c| c.progress().unwrap().queries_outstanding)
            .await,
        1
    );
    assert_eq!(
        alice
            .with(|c| c.progress().unwrap().queries_outstanding)
            .await,
        0
    );
    // alice consents (kind 10, to bob and both witnesses), bob's query
    // rides to w1 as kind 7, and w1's answer rides back as kind 8
    if !bob
        .holds(5000, |c| {
            c.progress().is_some_and(|p| p.queries_outstanding == 0)
        })
        .await
    {
        for p in [&mut alice, &mut bob, &mut w1] {
            p.drain();
        }
        panic!(
            "bob's query is answered\nalice: {:?}\nbob: {:?}\nw1: {:?}",
            alice.seen, bob.seen, w1.seen
        );
    }
    // bob hands alice what he gathered (kind 14)
    bob.step(|c| c.converse_gathered()).await.expect("gathered");
    // a stranger's back-pointers to alice: carol is neither counterparty
    // nor witness, and nothing of hers enters the ceremony
    let stray = carol
        .with(|c| {
            c.send_payload(
                kh("alice"),
                KIND_BACK_POINTERS,
                &BackPointers {
                    txids: vec![[7; 32]],
                }
                .encode(),
            )
        })
        .await
        .expect("carol sends");
    carol.courier.carry(stray).await;
    // step 7: alice proposes once w1's answer and back-pointers and bob's
    // responses are in, and says what she waits on until then
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match alice.step(|c| c.converse_propose()).await {
            Ok(()) => break,
            Err(Abort::Waiting(_)) | Err(Abort::NoWitness) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await
            }
            Err(e) => panic!("propose: {e:?}"),
        }
    }
    // the body goes to bob and w1 (kind 16), each signs (kind 17), alice
    // finalizes and sends the record (kind 18): every signer holds it
    for (p, who) in [(&alice, "alice"), (&bob, "bob"), (&w1, "w1")] {
        assert!(
            p.holds(10_000, |c| c.archive.len() == 1).await,
            "{who} holds the record"
        );
    }
    let env = alice
        .with(|c| c.archive.records().next().unwrap().bytes.clone())
        .await;
    for p in [&bob, &w1] {
        let theirs = p
            .with(|c| c.archive.records().next().unwrap().bytes.clone())
            .await;
        assert_eq!(theirs, env, "the same record");
    }
    assert_eq!(
        w2.with(|c| c.archive.len()).await,
        0,
        "a witness that declined holds nothing"
    );
    // the record: the two participants and w1, nominated by alice; w2
    // absent; one response, w1 about alice, unavailable since alice
    // released no key for records she never sealed a capture under
    let rec = Record::parse(&env).unwrap();
    let (p0, p1) = if kh("alice") < kh("bob") {
        (kh("alice"), kh("bob"))
    } else {
        (kh("bob"), kh("alice"))
    };
    assert_eq!(rec.signers, vec![p0, p1, kh("w1")]);
    let (proposal, _) = proposal_from_body(&env[rec.body.clone()]).unwrap();
    assert_eq!(
        proposal.witnesses,
        vec![Witness {
            keyhash: kh("w1"),
            nominated_by: kh("alice"),
            flags: 3,
        }]
    );
    assert_eq!(proposal.responses.len(), 1);
    let r = Response::read(&proposal.responses[0]).unwrap();
    assert_eq!(
        (r.verifier, r.subject, r.verdict),
        (kh("w1"), kh("alice"), Verdict::Unavailable)
    );
    // the participants and both participants' disclosures agree
    let set_a = alice
        .with(move |c| c.store.disclosures.get(&rec.txid).cloned())
        .await;
    let txid = rec.txid;
    let set_b = bob
        .with(move |c| c.store.disclosures.get(&txid).cloned())
        .await;
    assert!(set_a.is_some() && set_a == set_b, "both hold the set");

    // what each party saw of the conversation
    for p in [&mut alice, &mut bob, &mut w1, &mut w2] {
        p.drain();
    }
    let a = alice.conversed();
    assert!(
        a.iter().any(|(k, c)| *k == kh("carol")
            && matches!(c, Conversed::Refused { kind, .. } if *kind == KIND_BACK_POINTERS)),
        "carol's back-pointers were refused: {a:?}"
    );
    assert!(
        a.iter()
            .any(|(k, c)| *k == kh("w2")
                && matches!(c, Conversed::WitnessAnswer { flags: None, .. })),
        "w2 declined"
    );
    assert!(
        a.iter()
            .any(|(k, c)| *k == kh("w1")
                && matches!(c, Conversed::WitnessAnswer { flags: Some(3), .. })),
        "w1 will attest"
    );
    assert!(
        a.iter().any(|(k, c)| *k == kh("bob")
            && matches!(
                c,
                Conversed::Consent {
                    consented: true,
                    ..
                }
            )),
        "alice consented to bob's query"
    );
    assert!(
        a.iter()
            .any(|(_, c)| matches!(c, Conversed::Finalized { txid: t } if *t == txid)),
        "alice finalized on the last signature"
    );
    // the verifier's copy of its response came to alice, the subject
    assert!(
        alice
            .seen
            .iter()
            .any(|(k, d)| *k == kh("w1") && matches!(d, Dispatched::ResponseCopy(Ok(_)))),
        "the copy reached the subject"
    );
    let b = bob.conversed();
    assert!(
        b.iter()
            .any(|(k, c)| *k == kh("alice") && matches!(c, Conversed::Consented { .. })),
        "bob's query went to its verifier on alice's consent"
    );
    assert!(
        b.iter()
            .any(|(k, c)| *k == kh("alice") && matches!(c, Conversed::Reviewed { refused: None })),
        "bob reviewed and signed"
    );
    assert!(
        b.iter()
            .any(|(_, c)| matches!(c, Conversed::Finalized { txid: t } if *t == txid)),
        "bob holds the record"
    );
    // w1 observed the sequence from what reached it: asked by alice,
    // then alice's consent and bob's request, responses, back-pointers
    // and signature as they were sent to every witness, then the body
    // and the record
    let w = w1.conversed();
    assert!(
        w.iter()
            .any(|(k, c)| *k == kh("alice") && matches!(c, Conversed::Asked { flags: Some(3), .. })),
        "w1 was asked by alice"
    );
    let from_alice = kinds_from(&w, "alice");
    let from_bob = kinds_from(&w, "bob");
    for k in [10, 15] {
        assert!(from_alice.contains(&k), "w1 saw kind {k} from alice: {w:?}");
    }
    for k in [14, 15, 17] {
        assert!(from_bob.contains(&k), "w1 saw kind {k} from bob: {w:?}");
    }
    assert!(
        w.iter()
            .any(|(k, c)| *k == kh("alice") && matches!(c, Conversed::Reviewed { refused: None })),
        "w1 signed the body"
    );
    assert!(
        w.iter()
            .any(|(_, c)| matches!(c, Conversed::Finalized { txid: t } if *t == txid)),
        "w1 holds the record"
    );
    // w2 declined and holds nothing of the ceremony: everything after the
    // request is refused as from no ceremony it witnesses
    let w = w2.conversed();
    assert!(
        w.iter()
            .any(|(_, c)| matches!(c, Conversed::Asked { flags: None, .. })),
        "w2 declined"
    );
    assert!(
        !w.iter()
            .any(|(_, c)| matches!(c, Conversed::Observed { .. } | Conversed::Reviewed { .. })),
        "w2 observed nothing and signed nothing: {w:?}"
    );
}

/// A signer that cannot verify the body answers, so the proposer is told
/// rather than left waiting (`wire-format.md` §7.10.2, refusal 4).  Alice
/// proposes, and what reaches bob is her body rebuilt with a root that is
/// not the root of the set shown: his root check fails, he refuses as not
/// verified, alice records it among the refusals, and no record is made.
// acceptance: CER-22
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_signer_whose_root_check_fails_answers_and_the_proposer_records_it() {
    let now_s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let scene = Scene::starting(now_s - 30 * 86_400);
    let node = live_node(
        "witness",
        scene.table("witness", &["witness"]),
        "witness",
        &[],
    );
    let inboxes = Inboxes::default();
    let serving: Arc<dyn Serving> = LocalNode::new(node.clone(), inboxes.clone());
    let optical = vec![ChannelKind::Optical];
    let mut alice = host("alice", &serving, &inboxes, optical.clone(), 0);
    let mut bob = host("bob", &serving, &inboxes, optical, 0);
    let w1 = host("w1", &serving, &inboxes, vec![], 0);
    let population: Vec<Keyhash> = ["alice", "bob", "w1"].iter().map(|n| kh(n)).collect();
    for p in [&alice, &bob, &w1] {
        p.courier.attach(population.clone()).await;
    }
    for p in [&alice, &bob, &w1] {
        p.courier.sweep(population.clone()).await;
    }

    // the local exchanges, by hand (§14.3); w1 alice's nominee, bob
    // nominating nobody
    let ia = alice
        .with(|c| c.begin(kh("bob"), vec![kh("w1")], true))
        .await
        .expect("alice begins");
    let ib = bob
        .with(|c| c.begin(kh("alice"), vec![], false))
        .await
        .expect("bob begins");
    let cid = alice
        .with(move |c| c.take_intent(kh("bob"), &ib))
        .await
        .expect("alice takes bob's intent");
    let cid_b = bob
        .with(move |c| c.take_intent(kh("alice"), &ia))
        .await
        .expect("bob takes alice's intent");
    assert_eq!(cid, cid_b, "one ceremony");
    let ch = alice.with(|c| c.proximity()).await.expect("channels");
    bob.with(move |c| c.take_channels(&ch))
        .await
        .expect("bob agrees");
    let ka = alice.with(|c| c.capture_key()).await.expect("alice's key");
    let kb = bob.with(|c| c.capture_key()).await.expect("bob's key");
    let (ra, rb) = tokio::join!(
        alice.with(move |c| c.capture(kb)),
        bob.with(move |c| c.capture(ka))
    );
    ra.expect("alice captures bob");
    rb.expect("bob captures alice");

    // the conversation opens and nobody has a verifier to ask, so bob
    // hands over nothing gathered
    for p in [&alice, &bob] {
        p.step(|c| c.converse_open()).await.expect("opens");
    }
    for p in [&alice, &bob] {
        p.step(|c| c.converse_queries()).await.expect("queries");
    }
    bob.step(|c| c.converse_gathered()).await.expect("gathered");
    assert!(
        alice
            .holds(10_000, |c| {
                c.progress().is_some_and(|p| {
                    p.their_responses
                        && p.back_from.contains(&kh("bob"))
                        && p.back_from.contains(&kh("w1"))
                        && p.answered
                            .iter()
                            .any(|(k, f)| *k == kh("w1") && f.is_some())
                })
            })
            .await,
        "alice has what she proposes from"
    );

    // what bob is sent is alice's body rebuilt with its root flipped,
    // sealed before she proposes so the session's order is kept; her
    // own proposal then goes out and is kept back, so bob and w1 never
    // see a body that verifies
    let sent = alice
        .with(|c| {
            let flags = c
                .progress()
                .and_then(|p| p.answered.into_iter().find(|(k, _)| *k == kh("w1")))
                .and_then(|(_, f)| f)
                .expect("w1 attests");
            let witnesses = vec![Witness {
                keyhash: kh("w1"),
                nominated_by: kh("alice"),
                flags,
            }];
            let (mut proposal, set) = c.propose(vec![], witnesses).expect("a body");
            proposal.root[0] ^= 1;
            let back = vec![vec![[7u8; 32]]; proposal.signers().len()];
            let bad = Proposed {
                proposal,
                set,
                back,
            };
            let (k, bytes) = conversation_payload(&Msg::Proposal(Box::new(bad))).unwrap();
            let sent = c.send_payload(kh("bob"), k, &bytes).expect("alice sends");
            let kept_back = c.converse_propose().expect("alice proposes");
            assert!(!kept_back.is_empty(), "her own body went out");
            sent
        })
        .await;
    let carried = alice.courier.carry(sent).await;
    assert!(carried.complete(), "{carried:?}");

    // bob answers refusal 4 and alice records it rather than waiting
    assert!(
        alice
            .holds(10_000, |c| {
                c.progress().is_some_and(|p| p.refused == vec![kh("bob")])
            })
            .await,
        "alice holds bob's refusal"
    );
    for p in [&mut alice, &mut bob] {
        p.drain();
    }
    let b = bob.conversed();
    assert!(
        b.iter().any(|(k, c)| *k == kh("alice")
            && matches!(
                c,
                Conversed::Reviewed {
                    refused: Some(Refusal::NotVerified)
                }
            )),
        "bob refused the body as not verified: {b:?}"
    );
    let a = alice.conversed();
    assert!(
        a.iter().any(|(k, c)| *k == kh("bob")
            && matches!(
                c,
                Conversed::Signed {
                    signer,
                    refused: Some(Refusal::NotVerified)
                } if *signer == kh("bob")
            )),
        "alice saw the refusal: {a:?}"
    );
    assert!(
        !a.iter()
            .any(|(_, c)| matches!(c, Conversed::Finalized { .. })),
        "nothing was finalized: {a:?}"
    );
    for (p, who) in [(&alice, "alice"), (&bob, "bob"), (&w1, "w1")] {
        assert_eq!(
            p.with(|c| c.archive.len()).await,
            0,
            "{who} holds no record"
        );
    }
}
