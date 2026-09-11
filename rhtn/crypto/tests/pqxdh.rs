//! PQXDH as instantiated here: both parties reach one secret, with and
//! without one-time keys; any key or ciphertext altered breaks agreement;
//! the construction is deterministic in its seeds.

use rhtn_crypto::pqxdh::*;

fn keys(tag: u8) -> (DhSecret, DhSecret, KemSecret, DhSecret, KemSecret) {
    (DhSecret::from_seed([tag; 32]), DhSecret::from_seed([tag + 1; 32]), KemSecret::from_seed([tag + 2; 64]), DhSecret::from_seed([tag + 3; 32]), KemSecret::from_seed([tag + 4; 64]))
}

#[test]
fn both_parties_reach_one_secret_with_and_without_one_time_keys() {
    let (ik_b, spk_b, pqspk_b, opk_b, pqopk_b) = keys(10);
    let ik_a = DhSecret::from_seed([1; 32]);
    let ek_a = DhSecret::from_seed([2; 32]);
    let (ik_b_pub, spk_b_pub, pqspk_b_pub, opk_b_pub, pqopk_b_pub) = (ik_b.public(), spk_b.public(), pqspk_b.public(), opk_b.public(), pqopk_b.public());
    assert_eq!(pqspk_b_pub.0.len(), KEM_PUBLIC_BYTES);
    // with one-time keys
    let their = TheirBundle { ik: &ik_b_pub, spk: &spk_b_pub, pqspk: &pqspk_b_pub, opk: Some(&opk_b_pub), pqopk: Some(&pqopk_b_pub) };
    let init = initiate(&ik_a, &ek_a, &their, [7; 32]).unwrap();
    assert_eq!(init.kem_ciphertext.len(), KEM_CIPHERTEXT_BYTES);
    let me = Responder { ik: &ik_b, spk: &spk_b, pqspk: &pqspk_b, opk: Some(&opk_b), pqopk: Some(&pqopk_b) };
    let sk_b = respond(&me, &ik_a.public(), &init.ek, &init.kem_ciphertext).unwrap();
    assert_eq!(init.sk, sk_b);
    // deterministic in its inputs
    assert_eq!(initiate(&ik_a, &ek_a, &their, [7; 32]).unwrap().sk, init.sk);
    // without: reusable material alone, a different secret
    let their0 = TheirBundle { ik: &ik_b_pub, spk: &spk_b_pub, pqspk: &pqspk_b_pub, opk: None, pqopk: None };
    let init0 = initiate(&ik_a, &ek_a, &their0, [7; 32]).unwrap();
    let me0 = Responder { ik: &ik_b, spk: &spk_b, pqspk: &pqspk_b, opk: None, pqopk: None };
    assert_eq!(respond(&me0, &ik_a.public(), &init0.ek, &init0.kem_ciphertext).unwrap(), init0.sk);
    assert_ne!(init0.sk, init.sk);
    // the responder using a one-time key the initiator did not: disagreement
    assert_ne!(respond(&me, &ik_a.public(), &init0.ek, &init0.kem_ciphertext).unwrap(), init0.sk);
    // a flipped ciphertext byte: decapsulation yields another secret (implicit rejection)
    let mut ct = init.kem_ciphertext.clone();
    ct[0] ^= 1;
    assert_ne!(respond(&me, &ik_a.public(), &init.ek, &ct).unwrap(), init.sk);
    // another initiator identity: disagreement
    let ik_x = DhSecret::from_seed([99; 32]);
    assert_ne!(respond(&me, &ik_x.public(), &init.ek, &init.kem_ciphertext).unwrap(), init.sk);
    // the associated data names both identities in order
    let ad = associated_data(&ik_a.public(), &ik_b_pub);
    assert_eq!((&ad[..32], &ad[32..]), (&ik_a.public().0[..], &ik_b_pub.0[..]));
}

#[test]
fn the_kdf_is_hkdf_over_f_and_the_key_material_under_the_info_string() {
    let km = [5u8; 96];
    let mut ikm = vec![0xFFu8; 32];
    ikm.extend_from_slice(&km);
    let mut expect = [0u8; 32];
    hkdf_sha256(&[0u8; 32], &ikm, INFO, &mut expect);
    assert_eq!(kdf(&km), expect);
    assert_eq!(INFO, b"rhtn/1_X25519_SHA-256_ML-KEM-768");
}
