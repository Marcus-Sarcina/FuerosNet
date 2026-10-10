//! What a delegated credential takes into its run, and what it refuses.

use rhtn_crypto::delegation::issue_run;
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_transport::tls::Credential;

fn kh(n: &str) -> [u8; 32] {
    test_identity(n).public.keyhash
}

/// **A credential overlapping one already held is refused**, which is what
/// stops an operator who enrols twice from holding the same hours several
/// times over. `wire-format.md` §8.2 has a run contiguous — each
/// `not_before` the previous `not_after` — so the next of a run starts
/// where it ends, and that one is taken.
#[test]
fn an_overlapping_credential_is_refused_and_a_contiguous_one_is_taken() {
    let bob = test_identity("bob");
    let ids = std::slice::from_ref(&bob.public);
    let cred = Credential::from_seed(&[7u8; 32], kh("bob"));
    let public = cred.public();
    let start = 1_700_000_000;

    // a run of three, taken whole
    let run = issue_run(&bob, &public, start, 3);
    for d in &run {
        cred.add(ids, d).expect("the run is taken");
    }
    assert_eq!(3, cred.issued().len());
    let ends = cred.issued().iter().map(|i| i.not_after).max().unwrap();
    assert_eq!(
        start + 3 * 48 * 3600,
        ends,
        "three contiguous 48-hour windows"
    );

    // **the same bytes again are taken once, silently**: a credential
    // re-sent is not an error and not a duplicate
    cred.add(ids, &run[1])
        .expect("identical bytes are idempotent");
    assert_eq!(3, cred.issued().len());

    // **the same windows signed again from a later clock are refused**,
    // naming what is held and where the run ends
    let again = issue_run(&bob, &public, start + 5, 3);
    let e = cred.add(ids, &again[0]).expect_err("an overlap is refused");
    assert!(e.contains("overlaps one already held"), "{e}");
    assert!(
        e.contains(&format!("{start}..{}", start + 48 * 3600)),
        "names the held window: {e}"
    );
    assert!(
        e.contains(&format!("the run ends at {ends}")),
        "and where the run ends: {e}"
    );
    assert_eq!(3, cred.issued().len(), "nothing was taken");

    // a renewal starts where the run ends, and the half-open windows make
    // that contiguous rather than overlapping
    let next = issue_run(&bob, &public, ends, 2);
    for d in &next {
        cred.add(ids, d).expect("the next run is contiguous");
    }
    assert_eq!(5, cred.issued().len());

    // and one entirely before the run is an overlap of nothing: taken, and
    // kept in window order
    let earlier = issue_run(&bob, &public, start - 48 * 3600, 1);
    cred.add(ids, &earlier[0])
        .expect("a window before the run touches none");
    let order: Vec<u64> = cred.issued().iter().map(|i| i.not_before).collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "held in window order");
}
