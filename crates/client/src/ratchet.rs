//! The Double Ratchet (design §14.2.4.3), as Signal specifies it: a root
//! chain advanced by Diffie-Hellman ratchet steps, a sending and a
//! receiving chain each advanced per message, message keys derived and
//! discarded, and a bounded set of keys skipped for messages that arrive
//! out of order.  The Triple Ratchet runs this beside the Sparse
//! Post-Quantum Ratchet with their outputs mixed; the post-quantum ratchet
//! is the part still to import (`Robot/implementation-plan.md` section 7),
//! and this is the half that stands until then.
//!
//! Keys and the AEAD: X25519 for the ratchet keys, HKDF-SHA-256 for the
//! root chain, HMAC-SHA-256 for the symmetric chains, and AES-256-GCM
//! under a key and nonce derived from each message key.

use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use aws_lc_rs::hmac;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_crypto::pqxdh::{DhPublic, DhSecret, hkdf_sha256};
use std::collections::BTreeMap;

/// How far ahead a receiving chain is advanced for a message that has not
/// arrived (the specification's MAX_SKIP).
pub const MAX_SKIP: u32 = 1000;
/// How many skipped message keys are kept.
pub const MAX_SKIPPED_KEYS: usize = 2000;
const ROOT_INFO: &[u8] = b"rhtn/1:ratchet-root";
const MESSAGE_INFO: &[u8] = b"rhtn/1:ratchet-message";

/// One party's ratchet state for one session.
#[derive(Clone)]
pub struct Ratchet {
    dhs: DhSecret,
    dhr: Option<DhPublic>,
    rk: [u8; 32],
    cks: Option<[u8; 32]>,
    ckr: Option<[u8; 32]>,
    ns: u32,
    nr: u32,
    pn: u32,
    skipped: BTreeMap<([u8; 32], u32), [u8; 32]>,
    /// The associated data every message binds: the two identity keys.
    ad: Vec<u8>,
}

impl std::fmt::Debug for Ratchet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Ratchet(ns {}, nr {}, pn {}, skipped {})",
            self.ns,
            self.nr,
            self.pn,
            self.skipped.len()
        )
    }
}

fn kdf_rk(rk: &[u8; 32], dh_out: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let mut out = [0u8; 64];
    hkdf_sha256(rk, dh_out, ROOT_INFO, &mut out);
    (out[..32].try_into().unwrap(), out[32..].try_into().unwrap())
}

fn kdf_ck(ck: &[u8; 32]) -> ([u8; 32], [u8; 32]) {
    let key = hmac::Key::new(hmac::HMAC_SHA256, ck);
    let next: [u8; 32] = hmac::sign(&key, &[0x02]).as_ref().try_into().unwrap();
    let mk: [u8; 32] = hmac::sign(&key, &[0x01]).as_ref().try_into().unwrap();
    (next, mk)
}

fn aead_key(mk: &[u8; 32]) -> ([u8; 32], [u8; 12]) {
    let mut out = [0u8; 44];
    hkdf_sha256(&[0u8; 32], mk, MESSAGE_INFO, &mut out);
    (out[..32].try_into().unwrap(), out[32..].try_into().unwrap())
}

fn header_bytes(dh: &DhPublic, pn: u32, n: u32) -> Vec<u8> {
    let mut h = Vec::new();
    emit_map_head(&mut h, 3);
    emit_uint(&mut h, 1);
    emit_bstr(&mut h, &dh.0);
    emit_uint(&mut h, 2);
    emit_uint(&mut h, pn as u64);
    emit_uint(&mut h, 3);
    emit_uint(&mut h, n as u64);
    h
}

fn parse_header(h: &[u8]) -> Result<(DhPublic, u32, u32), String> {
    let item = parse_all(h).map_err(|e| e.0)?;
    let Item::Map(m) = &item else {
        return Err("header not a map".into());
    };
    let dh = match map_get(m, 1) {
        Some(Item::Bytes(r)) if r.len() == 32 => {
            DhPublic(<[u8; 32]>::try_from(&h[r.clone()]).unwrap())
        }
        _ => return Err("header dh".into()),
    };
    let pn = map_get(m, 2).and_then(as_uint).ok_or("header pn")? as u32;
    let n = map_get(m, 3).and_then(as_uint).ok_or("header n")? as u32;
    Ok((dh, pn, n))
}

fn seal(mk: &[u8; 32], ad: &[u8], header: &[u8], plaintext: &[u8]) -> Vec<u8> {
    let (k, nonce) = aead_key(mk);
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &k).expect("32-byte key"));
    let mut aad = ad.to_vec();
    aad.extend_from_slice(header);
    let mut buf = plaintext.to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(&aad),
        &mut buf,
    )
    .expect("sealing cannot fail");
    buf
}

fn open(mk: &[u8; 32], ad: &[u8], header: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    let (k, nonce) = aead_key(mk);
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &k).expect("32-byte key"));
    let mut aad = ad.to_vec();
    aad.extend_from_slice(header);
    let mut buf = ciphertext.to_vec();
    let n = key
        .open_in_place(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(&aad),
            &mut buf,
        )
        .map_err(|_| "message does not authenticate")?
        .len();
    buf.truncate(n);
    Ok(buf)
}

impl Ratchet {
    /// The initiator, holding the shared secret and the responder's signed
    /// prekey as its first ratchet key, with a fresh ratchet key of its own.
    pub fn initiator(sk: [u8; 32], their_spk: DhPublic, dhs: DhSecret, ad: Vec<u8>) -> Self {
        let (rk, cks) = kdf_rk(&sk, &dhs.agree(&their_spk));
        Ratchet {
            dhs,
            dhr: Some(their_spk),
            rk,
            cks: Some(cks),
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            skipped: BTreeMap::new(),
            ad,
        }
    }

    /// The responder, whose signed prekey is its first ratchet key.
    pub fn responder(sk: [u8; 32], spk: DhSecret, ad: Vec<u8>) -> Self {
        Ratchet {
            dhs: spk,
            dhr: None,
            rk: sk,
            cks: None,
            ckr: None,
            ns: 0,
            nr: 0,
            pn: 0,
            skipped: BTreeMap::new(),
            ad,
        }
    }

    /// Encrypt one message: the sending chain advances, the message key is
    /// used once, and the header carries this party's ratchet key and
    /// counters.  `[header, ciphertext]`.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let cks = self.cks.ok_or("no sending chain yet")?;
        let (ck, mk) = kdf_ck(&cks);
        self.cks = Some(ck);
        let header = header_bytes(&self.dhs.public(), self.pn, self.ns);
        self.ns += 1;
        let ct = seal(&mk, &self.ad, &header, plaintext);
        let mut out = Vec::new();
        emit_array_head(&mut out, 2);
        emit_bstr(&mut out, &header);
        emit_bstr(&mut out, &ct);
        Ok(out)
    }

    /// Decrypt one message.  A new ratchet key in the header steps the
    /// Diffie-Hellman ratchet, with `fresh` supplying the seed of this
    /// party's next key; messages skipped are kept for later, bounded.  On
    /// any failure the state is as it was.
    pub fn decrypt(
        &mut self,
        msg: &[u8],
        fresh: &mut dyn FnMut() -> [u8; 32],
    ) -> Result<Vec<u8>, String> {
        let parts = array_item_ranges(msg, 0).ok_or("message not an array")?;
        if parts.len() != 2 {
            return Err("message is header and ciphertext".into());
        }
        let bs = |r: &std::ops::Range<usize>| -> Result<Vec<u8>, String> {
            let (it, _) = Parser { b: msg }.item(r.start).map_err(|e| e.0)?;
            match &it {
                Item::Bytes(br) => Ok(msg[br.clone()].to_vec()),
                _ => Err("not bytes".into()),
            }
        };
        let (header, ct) = (bs(&parts[0])?, bs(&parts[1])?);
        let (dh, pn, n) = parse_header(&header)?;
        let mut trial = self.clone();
        let plaintext = trial.decrypt_in(&header, &ct, dh, pn, n, fresh)?;
        *self = trial;
        Ok(plaintext)
    }

    fn decrypt_in(
        &mut self,
        header: &[u8],
        ct: &[u8],
        dh: DhPublic,
        pn: u32,
        n: u32,
        fresh: &mut dyn FnMut() -> [u8; 32],
    ) -> Result<Vec<u8>, String> {
        if let Some(mk) = self.skipped.remove(&(dh.0, n)) {
            return open(&mk, &self.ad, header, ct);
        }
        if self.dhr != Some(dh) {
            self.skip(pn)?;
            self.dh_ratchet(dh, fresh);
        }
        self.skip(n)?;
        let ckr = self.ckr.ok_or("no receiving chain")?;
        let (ck, mk) = kdf_ck(&ckr);
        self.ckr = Some(ck);
        self.nr += 1;
        open(&mk, &self.ad, header, ct)
    }

    fn skip(&mut self, until: u32) -> Result<(), String> {
        if self.nr.saturating_add(MAX_SKIP) < until {
            return Err("too many messages skipped".into());
        }
        if let (Some(mut ckr), Some(dhr)) = (self.ckr, self.dhr) {
            while self.nr < until {
                let (ck, mk) = kdf_ck(&ckr);
                ckr = ck;
                self.skipped.insert((dhr.0, self.nr), mk);
                self.nr += 1;
            }
            self.ckr = Some(ckr);
            while self.skipped.len() > MAX_SKIPPED_KEYS {
                let oldest = *self.skipped.keys().next().unwrap();
                self.skipped.remove(&oldest);
            }
        }
        Ok(())
    }

    fn dh_ratchet(&mut self, dh: DhPublic, fresh: &mut dyn FnMut() -> [u8; 32]) {
        self.pn = self.ns;
        self.ns = 0;
        self.nr = 0;
        self.dhr = Some(dh);
        let (rk, ckr) = kdf_rk(&self.rk, &self.dhs.agree(&dh));
        self.rk = rk;
        self.ckr = Some(ckr);
        self.dhs = DhSecret::from_seed(fresh());
        let (rk, cks) = kdf_rk(&self.rk, &self.dhs.agree(&dh));
        self.rk = rk;
        self.cks = Some(cks);
    }

    /// The state as it goes to the device's own storage (`crate::durable`).
    pub fn encode(&self) -> Vec<u8> {
        use crate::durable::*;
        let mut out = Vec::new();
        emit_array_head(&mut out, 10);
        emit_bstr(&mut out, &self.dhs.to_bytes());
        emit_opt_bstr(&mut out, self.dhr.as_ref().map(|p| &p.0[..]));
        emit_bstr(&mut out, &self.rk);
        emit_opt_bstr(&mut out, self.cks.as_ref().map(|k| &k[..]));
        emit_opt_bstr(&mut out, self.ckr.as_ref().map(|k| &k[..]));
        emit_uint(&mut out, self.ns as u64);
        emit_uint(&mut out, self.nr as u64);
        emit_uint(&mut out, self.pn as u64);
        emit_array_head(&mut out, self.skipped.len());
        for ((pk, n), mk) in &self.skipped {
            emit_array_head(&mut out, 3);
            emit_bstr(&mut out, pk);
            emit_uint(&mut out, *n as u64);
            emit_bstr(&mut out, mk);
        }
        emit_bstr(&mut out, &self.ad);
        out
    }

    /// One back, whole or not at all.
    pub fn decode(b: &[u8]) -> Option<Ratchet> {
        use crate::durable::*;
        let (_, f) = parse_array(b, 10)?;
        let mut skipped = BTreeMap::new();
        for s in array(&f[8])? {
            let [pk, n, mk] = array(s)?.as_slice() else {
                return None;
            };
            skipped.insert((fixed::<32>(b, pk)?, uint(n)? as u32), fixed::<32>(b, mk)?);
        }
        if skipped.len() > MAX_SKIPPED_KEYS {
            return None;
        }
        Some(Ratchet {
            dhs: DhSecret::from_seed(fixed::<32>(b, &f[0])?),
            dhr: optional(&f[1], |it| fixed::<32>(b, it).map(DhPublic))?,
            rk: fixed::<32>(b, &f[2])?,
            cks: optional(&f[3], |it| fixed::<32>(b, it))?,
            ckr: optional(&f[4], |it| fixed::<32>(b, it))?,
            ns: uint(&f[5])? as u32,
            nr: uint(&f[6])? as u32,
            pn: uint(&f[7])? as u32,
            skipped,
            ad: bytes(b, &f[9])?,
        })
    }

    /// Whether this party can send yet: the responder cannot until it has
    /// received the initiator's first message.
    pub fn can_send(&self) -> bool {
        self.cks.is_some()
    }
}
