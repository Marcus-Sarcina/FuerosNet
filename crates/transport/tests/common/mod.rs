//! The split seal as the transport's tests need it (`wire-format.md`
//! §7.8.1, §7.10): a recipient's encryption secret, a node's key for that
//! recipient, a message sealed to both, and the node's half handed out at
//! delivery. The node itself never opens anything; these helpers stand in
//! for the sender, the node-key service and the recipient's client.

#![allow(dead_code)]

use rhtn_archive::sealed::Sealed;
use rhtn_archive::submission::unrelayed;
use rhtn_crypto::seal::{self, EncryptionSecret};
use rhtn_crypto::xwing::{self, CIPHERTEXT_BYTES, ESEED_BYTES};
use rhtn_transport::queue::Queued;
use rhtn_transport::session::OpenHalf;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// One recipient's side of the seal, at one node: the recipient's own
/// encryption secret and the node keys the node holds for it, by number.
pub struct SealKeys {
    pub recipient: [u8; 32],
    pub node: [u8; 32],
    pub secret: EncryptionSecret,
    pub node_keys: Arc<Mutex<HashMap<u64, xwing::Secret>>>,
}

impl SealKeys {
    /// Keys for `recipient` at `node`, node key 1 held.
    pub fn new(recipient: [u8; 32], node: [u8; 32]) -> Self {
        let mut root = [0u8; 32];
        root[..16].copy_from_slice(&recipient[..16]);
        root[16..].copy_from_slice(&node[..16]);
        let k = SealKeys {
            recipient,
            node,
            secret: EncryptionSecret::from_identity_root(&root),
            node_keys: Arc::new(Mutex::new(HashMap::new())),
        };
        k.hold(1);
        k
    }

    /// The node generates key `n` for this recipient.
    pub fn hold(&self, n: u64) {
        let mut seed = [0u8; 32];
        seed[..8].copy_from_slice(&n.to_be_bytes());
        seed[8..].copy_from_slice(&self.recipient[..24]);
        self.node_keys
            .lock()
            .unwrap()
            .insert(n, xwing::Secret::from_seed(seed));
    }

    /// **Seal `body` to the recipient and node key `n`**, as a sender
    /// does; `salt` keeps two seals of one body apart.
    pub fn seal(&self, n: u64, body: &[u8], salt: u8) -> Vec<u8> {
        let node_public = *self.node_keys.lock().unwrap()[&n].public();
        let eseed = |tag: u8| {
            let mut e = [0u8; ESEED_BYTES];
            e[0] = tag;
            e[1] = salt;
            e[2..10].copy_from_slice(&n.to_be_bytes());
            e[10..42].copy_from_slice(&rhtn_codec::cose::sha256(body));
            e
        };
        let (ss_r, ct_r) = xwing::encapsulate(self.secret.kem_public(), &eseed(1)).unwrap();
        let (ss_n, ct_n) = xwing::encapsulate(&node_public, &eseed(2)).unwrap();
        let key = seal::message_key(&ss_r, &ss_n, &self.recipient, &self.node, n, &ct_r, &ct_n);
        let mut nonce = [0u8; seal::NONCE_BYTES];
        nonce[0] = salt;
        nonce[1..9].copy_from_slice(&n.to_be_bytes());
        Sealed {
            n,
            ct_r: Box::new(ct_r),
            ct_n: Box::new(ct_n),
            body: seal::seal(&key, nonce, body),
        }
        .encode()
    }

    /// **The node's half** at delivery: `ctN` decapsulated under the key
    /// the message was sealed to, nothing where that key is gone.
    pub fn open_half(&self) -> OpenHalf {
        let held = self.node_keys.clone();
        let mine = self.recipient;
        Arc::new(move |recipient: &[u8; 32], q: &Queued| {
            if *recipient != mine {
                return None;
            }
            let s = Sealed::decode(&q.sealed).ok()?;
            held.lock()
                .unwrap()
                .get(&q.n)
                .map(|k| k.decapsulate(&s.ct_n))
        })
    }

    /// **What the recipient's own decapsulation opens a delivery to**,
    /// with the half the delivery carries: the body, or nothing.
    pub fn open(&self, delivery: &[u8]) -> Option<Vec<u8>> {
        let r = unrelayed(delivery)?;
        self.open_with(&r.sealed, &r.ss_n)
    }

    /// Open `sealed` with `ss_n` as the node's half.
    pub fn open_with(&self, sealed: &[u8], ss_n: &[u8; 32]) -> Option<Vec<u8>> {
        let s = Sealed::decode(sealed).ok()?;
        let ss_r = self.secret.decapsulate(&s.ct_r);
        let key = seal::message_key(
            &ss_r,
            ss_n,
            &self.recipient,
            &self.node,
            s.n,
            &s.ct_r,
            &s.ct_n,
        );
        seal::open(&key, &s.body).map(|b| b.to_vec())
    }
}

/// A `Sealed` carrying `body` where its ciphertext goes, behind room for
/// a nonce and a GCM tag, sealed to nobody: what a test of the mailbox
/// alone needs, the node opening nothing.
pub fn opaque(body: &[u8]) -> Vec<u8> {
    let mut b = vec![0u8; seal::NONCE_BYTES + 16];
    b.extend_from_slice(body);
    Sealed {
        n: 1,
        ct_r: Box::new([1; CIPHERTEXT_BYTES]),
        ct_n: Box::new([2; CIPHERTEXT_BYTES]),
        body: b,
    }
    .encode()
}

/// The sealed message one delivery carries, and its position.
pub fn delivered(delivery: &[u8]) -> (u64, Vec<u8>) {
    let r = unrelayed(delivery).expect("a RelayedPayload");
    (r.position, r.sealed)
}
