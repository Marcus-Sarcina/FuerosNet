//! **A transfer, three people's act** [author, 2026-11-28]
//! (`wire-format.md` §7.10.4; design §6.1.1, §6.2, §6.2.3): one patron's
//! person proposes it, the other patron's person agrees, and the node's
//! person signs the adoption offered under the new patron or does not.
//! Every ask waits at the serving node of the party it is for, and each
//! client takes the step an agreement already given owes when it next
//! attaches. **Strictly in-horizon**: every party inside each of the
//! others' horizons, by each one's own view.

mod common;

use common::harness::setup;
use common::net::*;
use common::*;
use rhtn_archive::tx::TYPE_ADOPTION;
use rhtn_client::ceremony::Abort;
use rhtn_client::payload::{KIND_TRANSFER_GRANT, KIND_TRANSFER_OFFER, KIND_TRANSFER_REQUEST};
use rhtn_client::store::{ClientStore, Stage};
use rhtn_codec::cose::aad;
use rhtn_codec::encode::{emit_bstr, emit_map_head, emit_uint};
use rhtn_crypto::verify;

/// A `TransferRequest` naming `node`, as a new patron sends it.
fn request(node: &str) -> Vec<u8> {
    let mut b = Vec::new();
    emit_map_head(&mut b, 1);
    emit_uint(&mut b, 1);
    emit_bstr(&mut b, &kh(node));
    b
}

/// `{1: former, 2: block}`, §4.1's `Transfer`.
fn transfer(former: &str, block: &[u8]) -> Vec<u8> {
    let mut b = Vec::new();
    emit_map_head(&mut b, 2);
    emit_uint(&mut b, 1);
    emit_bstr(&mut b, &kh(former));
    emit_uint(&mut b, 2);
    b.extend_from_slice(block);
    b
}

const TREE: [&str; 7] = ["c1", "alice", "carol", "bob", "c2", "c3", "c4"];

/// **One subnet, three levels deep**, every client's horizon taught the
/// topology as its serving node would bring it:
///
/// ```text
///               c1
///          alice    carol
///        bob   c2      c4
///              c3
/// ```
///
/// From bob, carol is his patron's sibling and c3 his nephew, both inside
/// his horizon and his patron's; c4 is his cousin, three edges away.
fn tree() -> Net {
    let mut names = TREE.to_vec();
    names.extend(["w1", "w2"]);
    let mut n = net_of(setup(&names), &["w1"], &TREE.map(|c| (c, "w1")));
    for a in TREE {
        for b in TREE {
            if a != b {
                n.introduce(a, b);
            }
        }
    }
    let mut recs = Vec::new();
    for (i, (node, patron)) in [
        ("alice", "c1"),
        ("carol", "c1"),
        ("bob", "alice"),
        ("c2", "alice"),
        ("c3", "c2"),
        ("c4", "carol"),
    ]
    .into_iter()
    .enumerate()
    {
        let t = n.s.adopted(node, patron, "w2", i as u32 + 1);
        recs.push(n.s.client(patron).store.records[&t].clone());
    }
    for c in TREE {
        for r in &recs {
            n.s.client(c).horizon.ingest(r, &ids()[..]);
        }
        n.attach(c);
    }
    n
}

/// The adoption of `node` under `patron` that `who`'s archive holds.
fn adoption_in(n: &mut Net, who: &str, node: &str, patron: &str) -> Option<Vec<u8>> {
    let c = n.s.client(who);
    c.archive
        .records()
        .find(|r| {
            r.tx_type == TYPE_ADOPTION
                && r.field_hash(1) == Some(kh(node))
                && r.field_hash(2) == Some(kh(patron))
        })
        .map(|r| r.bytes.clone())
}

/// What waits on `who`'s person, by what it asks.
fn awaiting(n: &mut Net, who: &str) -> Vec<Stage> {
    n.s.client(who)
        .transfers_awaiting()
        .into_iter()
        .map(|t| t.stage)
        .collect()
}

/// `who`'s person answers the one transfer waiting on them.
fn answer(n: &mut Net, who: &str, agree: bool) {
    let waiting = n.s.client(who).transfers_awaiting();
    assert_eq!(waiting.len(), 1, "{who}: {waiting:?}");
    let msgs =
        n.s.client(who)
            .answer_transfer(&waiting[0].id(), agree)
            .expect("an answer");
    n.carry(kh(who), msgs);
}

/// A transfer proposed by `proposer` and agreed to by everybody after.
fn agreed(n: &mut Net, proposer: &str, node: &str, other: &str, takers: &[&str]) {
    let msgs =
        n.s.client(proposer)
            .propose_transfer(kh(node), kh(other))
            .expect("inside every horizon");
    n.carry(kh(proposer), msgs);
    for who in takers {
        answer(n, who, true);
    }
}

/// Every party's held transfers.
fn held(n: &mut Net, who: &str) -> usize {
    n.s.client(who).store.transfers.len()
}

// functional: TX-032
#[test]
fn a_former_patron_proposes_the_new_patron_agrees_and_the_node_signs_each_when_next_on() {
    let mut n = tree();
    for c in ["alice", "bob", "carol"] {
        n.s.handles[c].reach.0.set(false);
    }
    n.away.insert(kh("bob"));
    n.away.insert(kh("carol"));

    // **1. alice's person releases bob to carol**: her statement, signed
    // by her act, waits at carol's node
    let msgs =
        n.s.client("alice")
            .propose_transfer(kh("bob"), kh("carol"))
            .expect("bob is alice's, and carol inside every horizon");
    n.carry(kh("alice"), msgs);
    assert_eq!(n.queued("carol").len(), 1);
    assert_eq!(held(&mut n, "alice"), 0, "the release is sent, not held");

    // **2. carol comes back and is asked**; nothing reaches bob until
    // she agrees
    n.away.remove(&kh("carol"));
    let got = n.collect("carol");
    assert!(got[0].starts_with("Transfer("), "{got:?}");
    assert_eq!(awaiting(&mut n, "carol"), [Stage::TakeOffered]);
    assert!(n.queued("bob").is_empty());
    let st = n.s.client("carol").store.clone();
    let back = ClientStore::decode(&st.encode()).expect("the store reads back");
    assert_eq!(back.transfers, st.transfers, "kept across a restart");
    answer(&mut n, "carol", true);
    assert!(awaiting(&mut n, "carol").is_empty());
    assert_eq!(n.queued("bob").len(), 1, "the offer waits for bob");

    // **3. bob comes back, is offered the adoption, and signs**; the rest
    // runs between the clients
    n.away.remove(&kh("bob"));
    n.collect("bob");
    assert_eq!(awaiting(&mut n, "bob"), [Stage::MoveOffered]);
    assert!(adoption_in(&mut n, "bob", "bob", "carol").is_none());
    answer(&mut n, "bob", true);

    let env = adoption_in(&mut n, "bob", "bob", "carol").expect("bob is adopted under carol");
    assert_eq!(
        adoption_in(&mut n, "carol", "bob", "carol").as_ref(),
        Some(&env),
        "and carol holds the same record"
    );
    verify::envelope(&ids()[..], &env).expect("both signatures");
    let body = body_of(&env);
    verify::adoption_evidence(&ids()[..], &body).expect("field 9 is alice's statement");
    assert!(
        rhtn_codec::cbor::value_slice(&body, 8).is_none(),
        "and no meeting: no verifiers, no witnesses"
    );
    for c in ["alice", "bob", "carol"] {
        assert_eq!(held(&mut n, c), 0, "{c} holds nothing of it after");
    }

    // the move is to carol's position, one hop below it
    let rec = rhtn_archive::record::Record::parse(&env).unwrap();
    let loc = rec.locator().unwrap();
    let hers =
        n.s.client("bob")
            .horizon
            .locator_in(&kh("carol"), &loc.anchor)
            .cloned()
            .unwrap();
    assert_eq!(loc.nibbles, hers.nibbles + 1);
}

// functional: TX-032
#[test]
fn a_new_patron_proposes_the_former_agrees_and_the_node_may_refuse() {
    let mut n = tree();

    // **carol's person asks alice for bob**, and alice's is asked
    let msgs =
        n.s.client("carol")
            .propose_transfer(kh("bob"), kh("alice"))
            .expect("bob is under alice, inside every horizon");
    n.carry(kh("carol"), msgs);
    assert_eq!(awaiting(&mut n, "alice"), [Stage::ReleaseAsked]);
    assert!(
        awaiting(&mut n, "carol").is_empty(),
        "carol asked; she is not asked again"
    );

    // alice agrees: her statement to carol, and carol, having asked,
    // offers bob the adoption with nobody asked twice
    answer(&mut n, "alice", true);
    assert!(awaiting(&mut n, "carol").is_empty());
    assert_eq!(awaiting(&mut n, "bob"), [Stage::MoveOffered]);

    // **bob's person declines**: nothing is signed, nobody is told
    let before = n.delivered.len();
    answer(&mut n, "bob", false);
    assert_eq!(n.delivered.len(), before, "nothing went anywhere");
    assert!(adoption_in(&mut n, "bob", "bob", "carol").is_none());
    assert_eq!(held(&mut n, "bob"), 0);
}

// functional: TX-033
#[test]
fn a_move_under_a_nephew_or_the_grandpatron_is_inside_every_horizon_and_one_under_a_cousin_is_not()
{
    let mut n = tree();

    // **c3 is bob's nephew**: two edges from bob and two from alice; c1
    // his grandpatron, alice's own patron. Each patron agrees, then bob
    for to in ["c3", "c1"] {
        agreed(&mut n, "alice", "bob", to, &[to, "bob"]);
        assert!(
            adoption_in(&mut n, "bob", "bob", to).is_some(),
            "{to}: {:?}",
            n.delivered
        );
    }

    // **c4 is bob's cousin**: two edges from alice and three from bob.
    // Neither patron's client will propose it
    assert!(matches!(
        n.s.client("alice").propose_transfer(kh("bob"), kh("c4")),
        Err(Abort::PatronRefused(_))
    ));
    assert!(
        n.s.client("c4")
            .propose_transfer(kh("bob"), kh("alice"))
            .is_err()
    );

    // asked anyway, alice holds nothing for her person
    let msgs =
        n.s.client("c4")
            .send_payload(kh("alice"), KIND_TRANSFER_REQUEST, &request("bob"))
            .unwrap();
    n.carry(kh("c4"), msgs);
    assert!(awaiting(&mut n, "alice").is_empty());

    // c4, shown a statement alice did sign, holds nothing for its person
    let block = rhtn_archive::tx::transfer_block(&id("alice"), &kh("bob"), &kh("c4"));
    let mut grant = Vec::new();
    emit_map_head(&mut grant, 2);
    emit_uint(&mut grant, 1);
    emit_bstr(&mut grant, &kh("bob"));
    emit_uint(&mut grant, 2);
    grant.extend_from_slice(&transfer("alice", &block));
    let msgs =
        n.s.client("alice")
            .send_payload(kh("c4"), KIND_TRANSFER_GRANT, &grant)
            .unwrap();
    n.carry(kh("alice"), msgs);
    assert!(awaiting(&mut n, "c4").is_empty());

    // and bob, offered that adoption by c4, is asked nothing
    let mut offer = Vec::new();
    emit_map_head(&mut offer, 1);
    emit_uint(&mut offer, 1);
    offer.extend_from_slice(&transfer("alice", &block));
    let msgs =
        n.s.client("c4")
            .send_payload(kh("bob"), KIND_TRANSFER_OFFER, &offer)
            .unwrap();
    n.carry(kh("c4"), msgs);
    assert!(awaiting(&mut n, "bob").is_empty());
    assert!(adoption_in(&mut n, "bob", "bob", "c4").is_none());
}

// functional: TX-032
#[test]
fn a_patron_releases_only_a_node_it_adopted_and_has_not_disavowed() {
    let mut n = tree();

    // alice never adopted carol, and c2 is not carol's patron
    assert!(matches!(
        n.s.client("alice").propose_transfer(kh("carol"), kh("c2")),
        Err(Abort::NoRelationship(_))
    ));
    // asked for carol anyway, alice holds nothing for her person
    let msgs =
        n.s.client("c2")
            .send_payload(kh("alice"), KIND_TRANSFER_REQUEST, &request("carol"))
            .unwrap();
    n.carry(kh("c2"), msgs);
    assert!(
        n.delivered
            .iter()
            .any(|(f, t, w)| *f == kh("c2") && *t == kh("alice") && w.contains("never adopted")),
        "{:?}",
        n.delivered
    );
    assert!(awaiting(&mut n, "alice").is_empty());

    // three parties, distinct
    for (node, other) in [("bob", "alice"), ("alice", "carol"), ("bob", "bob")] {
        assert!(matches!(
            n.s.client("alice").propose_transfer(kh(node), kh(other)),
            Err(Abort::PatronRefused(_))
        ));
    }

    // **once alice has disavowed bob, she releases no move of his**: the
    // disavowal reaching her view of the relationship as the topology
    // brings it, minted where her seed is (design §23.3)
    let t = n.now();
    let bp = n.s.client("alice").archive.next_back_pointers();
    let body = rhtn_archive::tx::disavowal_body(&bp, &kh("alice"), &kh("bob"), t, Some(1));
    let entries = id("alice").sign_entries(aad::ENVELOPE, &body);
    let env = rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_DISAVOWAL,
        &body,
        &[(kh("alice"), entries)],
    );
    let took = n.s.client("alice").horizon.ingest(&env, &ids()[..]);
    assert_eq!(took, rhtn_client::horizon::Took::Applied);
    assert!(matches!(
        n.s.client("alice").propose_transfer(kh("bob"), kh("carol")),
        Err(Abort::PatronRefused(_))
    ));
    let msgs =
        n.s.client("carol")
            .send_payload(kh("alice"), KIND_TRANSFER_REQUEST, &request("bob"))
            .unwrap();
    n.carry(kh("carol"), msgs);
    assert!(awaiting(&mut n, "alice").is_empty());
}

/// **A transfer body for a move nobody accepted is refused where an
/// adoption body is reviewed**: a statement the former patron did sign,
/// carried by a patron the node's person never signed the offer of.
// acceptance: CER-62
#[test]
fn a_body_carrying_a_statement_this_person_did_not_accept_is_refused() {
    let mut n = tree();
    let block = rhtn_archive::tx::transfer_block(&id("alice"), &kh("bob"), &kh("carol"));
    let back = n.s.client("bob").back_pointers();
    let body =
        n.s.client("carol")
            .propose_adoption(
                kh("bob"),
                &back,
                rhtn_client::ceremony::Adopting {
                    evidence: rhtn_archive::tx::Evidence::Transfer {
                        former: kh("alice"),
                        block,
                    },
                    series: 9,
                    presented_head: None,
                    key_material: None,
                    carrying: None,
                },
            )
            .expect("carol builds it");
    let refused = n.s.client("bob").review_and_sign_adoption(&body);
    assert!(
        matches!(&refused, Err(Abort::NodeRefused(why)) if why.contains("no move under")),
        "{refused:?}"
    );
}
