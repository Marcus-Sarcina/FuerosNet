//! Explicit expansions of functional_tests.md that the current components support.
use rhtn_archive::{record::Record, tx::{envelope, formation_body, TYPE_PRESENCE}};
use rhtn_client::record::*;
use rhtn_crypto::identity::testkit::test_identity;

#[test]
fn cap_011_all_128_disclosure_subsets_preserve_identity_and_the_exact_audience_view() {
    // The requested subset is the oracle; this is not an independent hash vector.
    let labels = ["capture", "location", "p0.integrity", "p0.retention",
                  "p1.integrity", "p1.retention", "proximity"];
    let a = test_identity("alice");
    let b = test_identity("bob");
    let mut signers = [&a, &b];
    signers.sort_by_key(|s| s.public.keyhash);
    let ids = vec![a.public.clone(), b.public.clone()];
    let set = disclosures([
        capture_value(0, 4, 0, 2), empty_location_value(),
        integrity_value(false, 0), retention_value(2),
        integrity_value(false, 0), retention_value(2),
        proximity_value(&[(3, 0, None)], 3),
    ], std::array::from_fn(|i| [i as u8 + 1; 16]));
    let heads = signers.map(|s| [rhtn_archive::genesis(&s.public.keyhash)]);
    let body = formation_body([&heads[0], &heads[1]],
        [&signers[0].public.keyhash, &signers[1].public.keyhash],
        100, 101, &disclosure_root(&set));
    let raw = envelope(TYPE_PRESENCE, &body, &signers);
    let original = Record::parse(&raw).expect("valid formation control");
    for mask in 0u8..128 {
        let reveal: Vec<_> = labels.iter().enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0).map(|(_, l)| *l).collect();
        let hide: Vec<_> = labels.iter().enumerate()
            .filter(|(i, _)| mask & (1 << i) == 0).map(|(_, l)| *l).collect();
        let encoded = present(&raw, &set, &reveal);
        let p = read_presentation(&ids, &encoded).expect("every legal subset verifies");
        assert_eq!(p.record.bytes, original.bytes, "envelope changed at mask {mask}");
        assert_eq!(p.record.txid, original.txid);
        assert_eq!(p.revealed.keys().copied().collect::<Vec<_>>(), reveal);
        assert_eq!(p.withheld, hide);
        for (i, label) in labels.iter().enumerate() {
            if mask & (1 << i) != 0 {
                assert_eq!(p.revealed[label], set[i].value);
            }
        }
        // Every representation still binds its final digest/disclosure bytes.
        let mut corrupted = encoded;
        *corrupted.last_mut().unwrap() ^= 1;
        assert!(read_presentation(&ids, &corrupted).is_err(), "tamper accepted at mask {mask}");
    }
}
