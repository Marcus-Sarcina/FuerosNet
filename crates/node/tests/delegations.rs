//! A node and its delegations: what it holds from the topology class,
//! what it pushes, what it signs under its delegated key and how a
//! neighbour verifies that (`wire-format.md` §8.2, §10.1, §7.5, §7.1;
//! design §23.3).

mod common;

use common::*;
use rhtn_archive::record::{Record, SigStatus};
use rhtn_archive::topology::{AckIssuer, AckTaken, Table};
use rhtn_archive::tx::{self, TYPE_DISAVOWAL};
use rhtn_codec::cbor::value_slice;
use rhtn_codec::cose::{self, aad};
use rhtn_codec::encode::*;
use rhtn_crypto::delegation::issue;
use rhtn_crypto::verify::{self, Failure};
use rhtn_node::Keyhash;
use rhtn_node::currency::{CurrencyState, ROLE_PATRON};
use rhtn_node::propagation::*;
use rhtn_node::store::{Decision, Horizon, KIND_DELEGATION, Known, TopologyStore};
use rhtn_node::view::NodeView;
use rhtn_transport::tls::{Clock, Credential};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const WINDOW: u64 = rhtn_codec::schema::DELEGATION_WINDOW_SECONDS;

/// Everyone is within the store horizon: what a test that is not about
/// the horizon wants.
struct Everywhere;
impl Horizon for Everywhere {
    fn within(&self, _: &Keyhash, _: usize) -> bool {
        true
    }
}

fn movable(t: u64) -> (Clock, Arc<AtomicU64>) {
    let cell = Arc::new(AtomicU64::new(t));
    let c = cell.clone();
    (Arc::new(move || c.load(Ordering::SeqCst)), cell)
}

/// `name` runs as an instance: a transport key from `seed` under `name`'s
/// own delegation opening at `t`, the clock a test moves.
fn instance(name: &str, seed: u8, t: u64, clock: Clock) -> Arc<Credential> {
    let c = Credential::from_seed(&[seed; 32], kh(name)).with_clock(clock);
    c.add(&ids(), &issue(&id(name), &c.public(), t)).unwrap();
    Arc::new(c)
}

/// P (alice) over N (bob) over S1 (carol) and S2 (w1); the view is N's,
/// with sessions to all four.
fn scene() -> (World, NodeView, Arc<Fabric>) {
    let mut w = World::new();
    let (a_n, _) = w.adopt("bob", "alice", 1);
    let (a_s1, _) = w.adopt("carol", "bob", 2);
    let (a_s2, _) = w.adopt("w1", "bob", 3);
    let table = table_with(
        kh("bob"),
        &w,
        &[&a_n, &a_s1, &a_s2],
        &["alice", "bob", "carol", "w1"],
    );
    let mut n = view("bob", table, "alice", &[0]);
    n.set_now(w.clock + 3600);
    n.set_slot(0, Some(kh("carol")), w.clock);
    n.set_slot(1, Some(kh("w1")), w.clock);
    let fab = Fabric::with(&[kh("alice"), kh("carol"), kh("w1")]);
    (w, n, fab)
}

// acceptance: PRP-26
#[test]
fn prp_26_a_pushed_delegation_is_stored_newest_per_keyhash_and_forwarded_once() {
    let (w, mut n, fab) = scene();
    let t = w.clock;
    let g = id("alice");
    let k1 = Credential::from_seed(&[1; 32], kh("alice")).public();
    let k2 = Credential::from_seed(&[2; 32], kh("alice")).public();
    let d1 = issue(&g, &k1, t);
    let d2 = issue(&g, &k2, t + WINDOW);
    // D1 arrives from carol (X) and is stored
    assert_eq!(
        n.receive_push(
            &*fab,
            &kh("carol"),
            &encode_push(KIND_DELEGATION, &d1),
            &ids()
        ),
        Decision::Stored
    );
    assert_eq!(n.store.delegation(&kh("alice")).unwrap().key, k1);
    fab.clear();
    // D2, later, from X: stored, D1 gone, forwarded to everyone but X
    assert_eq!(
        n.receive_push(
            &*fab,
            &kh("carol"),
            &encode_push(KIND_DELEGATION, &d2),
            &ids()
        ),
        Decision::Stored
    );
    let held = n.store.delegation(&kh("alice")).unwrap();
    assert_eq!(held.key, k2);
    assert_eq!(held.not_before, t + WINDOW);
    assert_eq!(n.store.delegations().len(), 1, "one per keyhash");
    assert_eq!(
        fab.recipients(FRAME_TOPOLOGY_PUSH),
        [kh("alice"), kh("w1")].into_iter().collect::<BTreeSet<_>>()
    );
    assert_eq!(
        fab.to(&kh("w1"), FRAME_TOPOLOGY_PUSH)[0],
        encode_push(KIND_DELEGATION, &d2)
    );
    // a second copy of D2 from w1: a duplicate, forwarded to nobody
    fab.clear();
    assert_eq!(
        n.receive_push(&*fab, &kh("w1"), &encode_push(KIND_DELEGATION, &d2), &ids()),
        Decision::Duplicate
    );
    assert_eq!(fab.count(FRAME_TOPOLOGY_PUSH), 0);
    // and D1 arriving again is older than what is held: a duplicate too
    assert_eq!(
        n.receive_push(&*fab, &kh("w1"), &encode_push(KIND_DELEGATION, &d1), &ids()),
        Decision::Duplicate
    );
}

// acceptance: PRP-28
#[test]
fn prp_28_an_instance_pushes_each_credential_as_it_comes_into_force() {
    let (w, mut n, fab) = scene();
    let t = w.clock;
    let (clock, cell) = movable(t);
    let cred = instance("bob", 3, t, clock);
    cred.add(&ids(), &issue(&id("bob"), &cred.public(), t + WINDOW))
        .unwrap();
    n.credential = Some(cred.clone());
    // the neighbour: carol's view, bob within her horizon
    let mut w2 = World::new();
    let (a_n, _) = w2.adopt("bob", "alice", 1);
    let (a_s1, _) = w2.adopt("carol", "bob", 2);
    let table = table_with(kh("carol"), &w2, &[&a_n, &a_s1], &["alice", "bob", "carol"]);
    let mut c = view("carol", table, "alice", &[0, 0]);
    let fab_c = Fabric::with(&[kh("bob")]);

    assert_eq!(n.push_credential(&*fab, &ids()), Some(Decision::Stored));
    assert_eq!(
        fab.recipients(FRAME_TOPOLOGY_PUSH),
        [kh("alice"), kh("carol"), kh("w1")]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "to every adjacent node"
    );
    assert_eq!(
        n.push_credential(&*fab, &ids()),
        None,
        "once per credential"
    );
    let push = fab.to(&kh("carol"), FRAME_TOPOLOGY_PUSH)[0].clone();
    assert_eq!(
        c.receive_push(&*fab_c, &kh("bob"), &push, &ids()),
        Decision::Stored
    );
    assert_eq!(c.store.delegation(&kh("bob")).unwrap().not_before, t);

    // the boundary passes: the next credential is pushed, and the
    // neighbour drops the previous one for that keyhash and forwards
    cell.store(t + WINDOW + 1, Ordering::SeqCst);
    fab.clear();
    fab_c.clear();
    assert_eq!(n.push_credential(&*fab, &ids()), Some(Decision::Stored));
    let push = fab.to(&kh("carol"), FRAME_TOPOLOGY_PUSH)[0].clone();
    assert_eq!(
        c.receive_push(&*fab_c, &kh("bob"), &push, &ids()),
        Decision::Stored
    );
    assert_eq!(
        c.store.delegation(&kh("bob")).unwrap().not_before,
        t + WINDOW
    );
    assert_eq!(c.store.delegations().len(), 1);
    assert_eq!(
        n.store.delegations().len(),
        1,
        "the instance's own store holds the newest"
    );
}

/// G (alice) over P (bob) over N (carol): G's table with the adoption of
/// N applied under `issuer`, and the acknowledgements it emitted.
fn grandpatron_acks(issuer: &AckIssuer) -> (World, Vec<Vec<u8>>, Vec<Record>) {
    let mut w = World::new();
    let (a_p, _) = w.adopt("bob", "alice", 1);
    let (a_n, _) = w.adopt("carol", "bob", 2);
    let mut t = Table::with_me(kh("alice"));
    t.mark_infra(kh("alice"));
    t.mark_infra(kh("bob"));
    t.apply(&a_p, &ids(), &w.store, None).unwrap();
    let out = t.apply(&a_n, &ids(), &w.store, Some(issuer)).unwrap();
    (w, out.acks, vec![a_p, a_n])
}

/// A sibling S (w1) holding the same adoptions in its own table.
fn sibling_table(w: &World, records: &[Record]) -> Table {
    let mut t = Table::with_me(kh("w1"));
    for n in ["alice", "bob", "w1"] {
        t.mark_infra(kh(n));
    }
    for r in records {
        t.apply(r, &ids(), &w.store, None).unwrap();
    }
    t
}

// acceptance: TOP-41
#[test]
fn top_41_an_instance_signs_its_acknowledgement_under_its_delegated_key() {
    let t = 1_800_000_000;
    let (clock, _) = movable(t);
    let g = instance("alice", 4, t, clock);
    let d = g.current().unwrap().raw;
    let issuer = AckIssuer {
        signer: Arc::new(g.signer()),
        policy: Arc::new(|_, _| true),
        now: t + 1,
    };
    let (w, acks, records) = grandpatron_acks(&issuer);
    assert_eq!(acks.len(), 1);
    let ack = &acks[0];
    // field 5 verifies under k and under no key of G's identity
    assert!(matches!(
        verify::record(&ids(), "SubtreeAck", ack),
        Err(Failure::MissingDelegation(k)) if k == kh("alice").to_vec()
    ));
    let mut store = TopologyStore::new();
    assert_eq!(
        store.accept(KIND_DELEGATION, &d, &kh("alice"), &ids(), &Everywhere),
        Decision::Stored
    );
    let known = Known {
        ids: &ids(),
        store: &store,
    };
    assert_eq!(verify::record(&known, "SubtreeAck", ack), Ok(()));
    // S accepts it, having matched field 2 to D's field 2 and the signing
    // key to D's field 1
    let mut s = sibling_table(&w, &records);
    assert_eq!(s.take_ack(&known, ack), Ok(AckTaken::Taken));
    assert_eq!(s.acks().len(), 1);
    assert_eq!(s.acks()[0].grandpatron, kh("alice"));
    // a delegation naming another key does not stand for it
    let other = TopologyStore::new();
    let mut other_store = other;
    let k_other = Credential::from_seed(&[9; 32], kh("alice")).public();
    other_store.accept(
        KIND_DELEGATION,
        &issue(&id("alice"), &k_other, t + WINDOW),
        &kh("alice"),
        &ids(),
        &Everywhere,
    );
    assert!(
        verify::record(
            &Known {
                ids: &ids(),
                store: &other_store
            },
            "SubtreeAck",
            ack
        )
        .is_err()
    );
}

// acceptance: TOP-42
#[test]
fn top_42_an_acknowledgement_waits_for_the_delegation_it_was_signed_under() {
    let t = 1_800_000_000;
    let (clock, _) = movable(t);
    let g = instance("alice", 5, t, clock);
    let issuer = AckIssuer {
        signer: Arc::new(g.signer()),
        policy: Arc::new(|_, _| true),
        now: t + 1,
    };
    let (w, acks, records) = grandpatron_acks(&issuer);
    let mut s = sibling_table(&w, &records);
    let mut store = TopologyStore::new();
    let known = Known {
        ids: &ids(),
        store: &store,
    };
    // neither accepted nor rejected: held aside
    assert_eq!(
        s.take_ack(&known, &acks[0]),
        Ok(AckTaken::Deferred(kh("alice")))
    );
    assert_eq!(s.acks().len(), 0);
    assert_eq!(s.deferred_acks(), 1);
    // it enters when G's delegation naming k arrives and verifies
    assert_eq!(
        store.accept(
            KIND_DELEGATION,
            &g.current().unwrap().raw,
            &kh("alice"),
            &ids(),
            &Everywhere
        ),
        Decision::Stored
    );
    let known = Known {
        ids: &ids(),
        store: &store,
    };
    assert_eq!(s.release_deferred_acks(&known), vec![AckTaken::Taken]);
    assert_eq!(s.acks().len(), 1);
    assert_eq!(s.deferred_acks(), 0);
}

// acceptance: CUR-21
#[test]
fn cur_21_an_instance_staples_its_delegation_to_the_attestation_it_signs_under_it() {
    // P (alice) has adopted S (bob); P runs as an instance
    let mut w = World::new();
    let (a, _) = w.adopt("bob", "alice", 1);
    let table = table_with(kh("alice"), &w, &[&a], &["alice"]);
    let mut p = view("alice", table, "alice", &[]);
    p.set_now(w.clock);
    let (clock, _) = movable(w.clock);
    let cred = instance("alice", 6, w.clock - 60, clock);
    let d = cred.current().unwrap().raw;
    p.credential = Some(cred.clone());
    let att = p
        .issue_currency(&CurrencyState::default(), &kh("bob"))
        .unwrap();
    // field 8 is present and equals D; field 7 verifies under k
    let r8 = value_slice(&att, 8).expect("field 8");
    assert_eq!(&att[r8], d.as_slice());
    // the relying party holds P's KeyMaterial alone, no delegation, and
    // accepts the attestation as P's
    let only_material: Vec<_> = ids();
    assert_eq!(
        verify::record(&only_material, "CurrencyAttestation", &att),
        Ok(())
    );
    let parsed = rhtn_archive::currency::parse_attestation(&only_material, &att).unwrap();
    assert_eq!(parsed.issuer, kh("alice"));
    assert_eq!(parsed.role, ROLE_PATRON);
    assert_eq!(parsed.delegation.as_deref(), Some(d.as_slice()));
    // under the identity's own key, an attestation carries no field 8
    p.credential = None;
    let plain = p
        .issue_currency(&CurrencyState::default(), &kh("bob"))
        .unwrap();
    assert!(value_slice(&plain, 8).is_none());
}

// acceptance: CUR-22
#[test]
fn cur_22_an_attestation_whose_staple_names_another_key_is_malformed() {
    let t = 1_800_000_000;
    let (clock, _) = movable(t);
    let signing = instance("alice", 7, t, clock.clone());
    let other = instance("alice", 8, t + WINDOW, clock);
    let att = tx::currency_attestation_stapled(
        &signing.signer(),
        &kh("bob"),
        &kh("bob"),
        t,
        t + 3600,
        ROLE_PATRON,
        &other.current().unwrap().raw,
    );
    assert!(verify::record(&ids(), "CurrencyAttestation", &att).is_err());
    assert!(
        rhtn_archive::currency::parse_attestation(&ids(), &att).is_err(),
        "not accepted and not treated as absent: nothing is taken from it"
    );
    // holding the signing key's own delegation changes nothing: the staple
    // is checked against field 7, not the holder's view
    let mut store = TopologyStore::new();
    store.accept(
        KIND_DELEGATION,
        &signing.current().unwrap().raw,
        &kh("alice"),
        &ids(),
        &Everywhere,
    );
    assert!(
        verify::record(
            &Known {
                ids: &ids(),
                store: &store
            },
            "CurrencyAttestation",
            &att
        )
        .is_err()
    );
}

// acceptance: DEC-35
#[test]
fn dec_35_an_envelope_signed_under_a_delegated_key_fails_verification() {
    let mut w = World::new();
    let (a, _) = w.adopt("carol", "bob", 1);
    let dis = w.disavow("bob", "carol", Some(1));
    let body = &dis.bytes[value_slice(&dis.bytes, 3).unwrap()];
    // the instance's delegation is held and valid
    let t = w.clock;
    let (clock, _) = movable(t);
    let cred = instance("bob", 10, t - 60, clock);
    let mut store = TopologyStore::new();
    assert_eq!(
        store.accept(
            KIND_DELEGATION,
            &cred.current().unwrap().raw,
            &kh("bob"),
            &ids(),
            &Everywhere
        ),
        Decision::Stored
    );
    // a type-3 envelope whose single signature is by that transport key,
    // named as bob
    let prot = cose::protected_header(cose::ALG_EDDSA, &kh("bob"));
    let tbs = cose::sig_structure_sign(&prot, aad::ENVELOPE, body);
    let sig = {
        use ed25519_dalek::Signer as _;
        ed25519_dalek::SigningKey::from_bytes(&[10; 32])
            .sign(&tbs)
            .to_bytes()
            .to_vec()
    };
    let mut env = Vec::new();
    emit_map_head(&mut env, 4);
    emit_uint(&mut env, 1);
    emit_uint(&mut env, 1);
    emit_uint(&mut env, 2);
    emit_uint(&mut env, TYPE_DISAVOWAL);
    emit_uint(&mut env, 3);
    env.extend_from_slice(body);
    emit_uint(&mut env, 4);
    emit_array_head(&mut env, 4);
    emit_bstr(&mut env, b"");
    emit_map_head(&mut env, 0);
    emit_null(&mut env);
    emit_array_head(&mut env, 1);
    emit_array_head(&mut env, 3);
    emit_bstr(&mut env, &prot);
    emit_map_head(&mut env, 0);
    emit_bstr(&mut env, &sig);
    let known = Known {
        ids: &ids(),
        store: &store,
    };
    let verdict = match Record::parse(&env) {
        Ok(rec) => rec.check_signatures(&known),
        Err(e) => SigStatus::Invalid(e),
    };
    assert!(
        matches!(verdict, SigStatus::Invalid(_)),
        "a held delegation does not stand in for the identity's key material: {verdict:?}"
    );
    // and the store refuses it as malformed: not stored, not forwarded
    let mut n = view("w1", Table::with_me(kh("w1")), "alice", &[]);
    let fab = Fabric::with(&[kh("bob")]);
    let _ = a;
    assert!(matches!(
        n.receive_push(
            &*fab,
            &kh("bob"),
            &encode_push(rhtn_node::store::KIND_TRANSACTION, &env),
            &known
        ),
        Decision::Malformed(_) | Decision::OutOfStore | Decision::Refused(_)
    ));
    assert_eq!(fab.count(FRAME_TOPOLOGY_PUSH), 0);
}
