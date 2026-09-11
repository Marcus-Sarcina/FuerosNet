//! The two constructions against the vectors' known answers
//! (`test-vectors/records.md`): the ceremony pre-commitment and the capture
//! key, each to the byte.

use rhtn_client::keys::{capture_key, pre_commitment};
use rhtn_crypto::identity::testkit::test_identity;

fn h32(s: &str) -> [u8; 32] {
    hex::decode(s).unwrap().try_into().unwrap()
}

// acceptance: CER-05
#[test]
fn the_pre_commitment_is_contributory_in_ascending_keyhash_order() {
    let (alice, bob) = (test_identity("alice").public.keyhash, test_identity("bob").public.keyhash);
    let ca: [u8; 16] = hex::decode("57c54784788abe36a304be6d2eff43d4").unwrap().try_into().unwrap();
    let cb: [u8; 16] = hex::decode("d43a0b07379cf934c8b7e4b54629f95c").unwrap().try_into().unwrap();
    let want = h32("cb8ea88ad0a089017394c291f918217c4dc8d754a4639eb024f23662e5ca2b18");
    // the known answer orders bob's contribution first: his keyhash is the lower
    assert!(bob < alice, "the vector's order is by keyhash, bob then alice");
    assert_eq!(pre_commitment((&alice, &ca), (&bob, &cb)), want, "both compute the same bytes");
    assert_eq!(pre_commitment((&bob, &cb), (&alice, &ca)), want, "whichever side computes it");
    // changing either contribution changes the value
    let mut ca2 = ca;
    ca2[0] ^= 1;
    assert_ne!(pre_commitment((&alice, &ca2), (&bob, &cb)), want);
    let mut cb2 = cb;
    cb2[15] ^= 1;
    assert_ne!(pre_commitment((&alice, &ca), (&bob, &cb2)), want);
}

// acceptance: CER-04
#[test]
fn the_capture_key_is_hkdf_sha256_over_the_stated_info() {
    let seed = h32("298a1bd33f525f98915dd373e76cfe00badeb753751b7a11622284adc63e466d");
    let info = hex::decode(concat!(
        "7268746e2f313a636170747572658410def778a5de3a25991aba399716bc8ecc",
        "fda9ad57d4ea8a0c8dcfc852aa6a96664caec817f958a439b6d326c45f5ab7bb",
        "b3671ebb447eca25a411b975e73aab9c3a6457a92a1ab0738104359b0712fad9",
        "e4df0f53f5a29829855d4d1d58f5"
    ))
    .unwrap();
    assert_eq!(&info[..14], b"rhtn/1:capture");
    let subject: [u8; 32] = info[14..46].try_into().unwrap();
    let holder: [u8; 32] = info[46..78].try_into().unwrap();
    let ceremony: [u8; 32] = info[78..110].try_into().unwrap();
    assert_eq!(subject, test_identity("alice").public.keyhash, "subject alice");
    assert_eq!(holder, test_identity("c1").public.keyhash, "holder c1");
    let want = h32("6157379db20e9b35da24fbab9ab4c8bc8676dc8f8c22c89d2b1fe2fea7b2985c");
    assert_eq!(capture_key(&seed, &subject, &holder, &ceremony), want);
    // a different holder or ceremony yields a different key
    assert_ne!(capture_key(&seed, &subject, &test_identity("bob").public.keyhash, &ceremony), want);
    let mut other = ceremony;
    other[31] ^= 1;
    assert_ne!(capture_key(&seed, &subject, &holder, &other), want);
    assert_ne!(capture_key(&seed, &holder, &subject, &ceremony), want, "subject and holder are not interchangeable");
}
