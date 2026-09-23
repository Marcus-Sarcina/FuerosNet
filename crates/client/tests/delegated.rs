//! A client on a device that holds no seed (design §23.3): what the
//! identity key signs is refused there, and the ceremony device is where
//! delegations are issued.

mod common;

use common::harness::*;
use common::*;
use rhtn_client::ceremony::*;
use rhtn_client::device::ChannelKind;
use rhtn_client::query::VerificationQuery;
use rhtn_client::verifier::{GrantOutcome, QueryOutcome};
use rhtn_transport::tls::Credential;

fn desktop_of(name: &str, presented: [u8; 32]) -> Client {
    let s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    s.clock.set(1_790_000_000_000);
    let (dev, _h) = common::harness::device(vec![ChannelKind::Nfc], s.clock.clone(), 77, 0);
    Client::delegated(id(name).public, presented, ids(), Config::default(), dev)
}

/// Every act the signing table gives the identity key is refused on a
/// device that holds none, by name, and nothing is fabricated in its
/// place.
#[test]
fn a_device_holding_no_seed_refuses_what_the_identity_key_signs() {
    let mut d = desktop_of("bob", [0x42; 32]);
    assert_eq!(d.sign_body(b"a body").unwrap_err(), Abort::NoSeed);
    assert_eq!(
        d.delegate(&[7; 32], 1_790_000_000).unwrap_err(),
        Abort::NoSeed
    );
    assert_eq!(d.sign_device_bundle(b"\xa0").unwrap_err(), Abort::NoSeed);
    assert!(matches!(d.signer(), Err(Abort::NoSeed)));

    // as subject: no consent, so no grant and no response copy taken
    let q = VerificationQuery {
        subject: kh("bob"),
        querier: kh("alice"),
        ceremony_id: [1; 32],
        profile: vec![],
        template_version: 1,
        verifier: kh("alice"),
    };
    assert!(d.consent(&q).is_none());
    assert!(d.take_response_copy(b"anything").is_err());
    // as verifier: a query is closed and a grant rejected, both saying why
    assert_eq!(
        d.take_query(kh("alice"), &q.encode()),
        QueryOutcome::Closed("this device holds no seed")
    );
    assert_eq!(
        d.take_grant(kh("alice"), b"grant"),
        GrantOutcome::Rejected("this device holds no seed")
    );
    assert!(d.expire().is_empty());
}

/// The ceremony device delegates to a key another device minted: a
/// hybrid delegation the network verifies under the identity, naming that
/// key, for one window; a run is contiguous.
#[test]
fn the_ceremony_device_delegates_to_a_transport_key_another_device_minted() {
    let s = setup(&["alice", "bob"], &[ChannelKind::Nfc]);
    s.clock.set(1_790_000_000_000);
    let (dev, _h) = common::harness::device(vec![ChannelKind::Nfc], s.clock.clone(), 77, 0);
    let phone = Client::new(id("bob"), ids(), Config::default(), dev);
    assert!(phone.holds_seed());

    let cred = Credential::from_seed(&[5; 32], kh("bob"))
        .with_clock(std::sync::Arc::new(|| 1_790_000_100));
    let key = cred.public();
    let raw = phone.delegate(&key, 1_790_000_000).unwrap();
    let d = rhtn_crypto::verify::delegation(&ids(), &raw).expect("verifies hybrid under bob");
    assert_eq!(d.key, key);
    assert_eq!(d.keyhash, kh("bob"));
    assert_eq!(d.not_after - d.not_before, 172_800);
    // the credential the desktop holds takes it and is in force
    cred.add(&ids(), &raw).unwrap();
    assert!(cred.current().is_some());
    // one by alice for the same key is not bob's to hold
    let alices = {
        let (dev, _h) = common::harness::device(vec![ChannelKind::Nfc], s.clock.clone(), 78, 0);
        Client::new(id("alice"), ids(), Config::default(), dev)
            .delegate(&key, 1_790_000_000)
            .unwrap()
    };
    assert!(cred.add(&ids(), &alices).is_err());

    let run = phone.delegate_run(&key, 1_790_000_000, 3).unwrap();
    let windows: Vec<(u64, u64)> = run
        .iter()
        .map(|r| {
            let d = rhtn_crypto::verify::delegation(&ids(), r).unwrap();
            (d.not_before, d.not_after)
        })
        .collect();
    assert_eq!(
        windows,
        vec![
            (1_790_000_000, 1_790_172_800),
            (1_790_172_800, 1_790_345_600),
            (1_790_345_600, 1_790_518_400)
        ],
        "each not_before is the previous not_after"
    );
}
