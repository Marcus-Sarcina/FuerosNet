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
use zeroize::Zeroize;

/// How far ahead a receiving chain is advanced for a message that has not
/// arrived (the specification's MAX_SKIP).
pub const MAX_SKIP: u32 = 1000;
/// How many skipped message keys are kept.
pub const MAX_SKIPPED_KEYS: usize = 2000;
const ROOT_INFO: &[u8] = b"rhtn/1:ratchet-root";
const MESSAGE_INFO: &[u8] = b"rhtn/1:ratchet-message";

/// The post-quantum half of the Triple Ratchet (design §14.2.4.3).
///
/// **The Double Ratchet above is the conformance floor** [author,
/// 2026-09-26]; a Triple Ratchet is that ratchet with this one beside it
/// and their outputs mixed. This interface is the seam between them, and
/// it is **written from design §14.2.4.3 and the published Sparse
/// Post-Quantum Ratchet specification, not from any implementation of
/// one**: an implementation of this trait may carry whatever licence it
/// likes without reaching the crates that define it.
///
/// **Provisional until something implements it.** No post-quantum ratchet
/// exists here to exercise the shape, so what this promises is that the
/// mixing point and the carriage are where §14.2.4.3 puts them, not that
/// the method set is final.
pub trait PostQuantumRatchet {
    /// What to mix into the root chain at this advance, where this ratchet
    /// has something ready.
    ///
    /// **`None` is the ordinary answer.** A sparse ratchet spends most
    /// steps moving erasure-coded key material across successive headers
    /// and has nothing to contribute until a whole one lands; the step then
    /// proceeds on the Diffie-Hellman output alone, exactly as the floor
    /// does.
    fn contribution(&mut self) -> Option<[u8; 32]>;

    /// What this ratchet wants carried in the next header. Empty carries
    /// nothing, and a header with nothing to carry is the floor's header
    /// byte for byte.
    fn outgoing(&mut self) -> Vec<u8>;

    /// Material from the peer's header. An error is the message's, not the
    /// session's: the caller leaves its state as it was.
    fn incoming(&mut self, chunk: &[u8]) -> Result<(), String>;

    /// State to persist beside the ratchet's own (`crate::durable`).
    fn encode(&self) -> Vec<u8>;

    /// The reverse, into a default-constructed ratchet.
    fn restore(&mut self, b: &[u8]) -> Result<(), String>;
}

/// No post-quantum ratchet: the Double Ratchet alone, which is the floor.
///
/// Contributes nothing, carries nothing and persists nothing, so a session
/// running this derives exactly the keys and writes exactly the bytes it
/// did before the seam existed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoPostQuantum;

impl PostQuantumRatchet for NoPostQuantum {
    fn contribution(&mut self) -> Option<[u8; 32]> {
        None
    }
    fn outgoing(&mut self) -> Vec<u8> {
        Vec::new()
    }
    fn incoming(&mut self, _chunk: &[u8]) -> Result<(), String> {
        Err("no post-quantum ratchet holds this session".into())
    }
    fn encode(&self) -> Vec<u8> {
        Vec::new()
    }
    fn restore(&mut self, _b: &[u8]) -> Result<(), String> {
        Ok(())
    }
}

/// One party's ratchet state for one session.
#[derive(Clone)]
pub struct Ratchet<P: PostQuantumRatchet + Clone + Default = NoPostQuantum> {
    dhs: DhSecret,
    dhr: Option<DhPublic>,
    rk: [u8; 32],
    cks: Option<[u8; 32]>,
    ckr: Option<[u8; 32]>,
    ns: u32,
    nr: u32,
    pn: u32,
    skipped: BTreeMap<([u8; 32], u32), [u8; 32]>,
    /// The post-quantum half, or [`NoPostQuantum`] at the floor.
    pq: P,
    /// The associated data every message binds: the two identity keys.
    ad: Vec<u8>,
}

/// **Everything this holds is wiped when it goes.** The root key, both
/// chain keys and every skipped message key are ours rather than a dalek or
/// `aws-lc-rs` type's, so nothing else wipes them; `dhs` wipes its own.
///
/// This also covers the trial copy [`Ratchet::decrypt`] makes of the whole
/// state on every message: the copy is dropped on the failing path and the
/// original on the succeeding one, and either way what is dropped is wiped
/// rather than left in freed memory.
impl<P: PostQuantumRatchet + Clone + Default> Drop for Ratchet<P> {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.rk.zeroize();
        if let Some(k) = self.cks.as_mut() {
            k.zeroize();
        }
        if let Some(k) = self.ckr.as_mut() {
            k.zeroize();
        }
        for mk in self.skipped.values_mut() {
            mk.zeroize();
        }
    }
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

/// Advance the root chain.
///
/// **`pq` is where the two ratchets' outputs are mixed** (design
/// §14.2.4.3): it extends the key material rather than replacing it, so a
/// step with nothing post-quantum ready derives precisely what the Double
/// Ratchet alone derives, and the floor is bit-identical to what this
/// function computed before the seam existed.
fn kdf_rk(rk: &[u8; 32], dh_out: &[u8; 32], pq: Option<[u8; 32]>) -> ([u8; 32], [u8; 32]) {
    let mut out = [0u8; 64];
    match pq {
        None => hkdf_sha256(rk, dh_out, ROOT_INFO, &mut out),
        Some(q) => {
            let mut ikm = [0u8; 64];
            ikm[..32].copy_from_slice(dh_out);
            ikm[32..].copy_from_slice(&q);
            hkdf_sha256(rk, &ikm, ROOT_INFO, &mut out);
            ikm.zeroize();
        }
    }
    let split = (out[..32].try_into().unwrap(), out[32..].try_into().unwrap());
    // the buffer the two halves were copied out of, and the mixed key
    // material beside it, are wiped rather than left on the stack
    out.zeroize();
    split
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

/// The header.
///
/// **Key 4 is the post-quantum ratchet's carriage and appears only when it
/// has something to carry**, so the floor emits the same three entries it
/// always did. The header is the AEAD's associated data, so a party that
/// adds the key and one that does not are not speaking the same session —
/// which is the conformance question design §14.2.4.3 settles, not
/// something either end can negotiate here.
fn header_bytes(dh: &DhPublic, pn: u32, n: u32, pq: &[u8]) -> Vec<u8> {
    let mut h = Vec::new();
    emit_map_head(&mut h, if pq.is_empty() { 3 } else { 4 });
    emit_uint(&mut h, 1);
    emit_bstr(&mut h, &dh.0);
    emit_uint(&mut h, 2);
    emit_uint(&mut h, pn as u64);
    emit_uint(&mut h, 3);
    emit_uint(&mut h, n as u64);
    if !pq.is_empty() {
        emit_uint(&mut h, 4);
        emit_bstr(&mut h, pq);
    }
    h
}

fn parse_header(h: &[u8]) -> Result<(DhPublic, u32, u32, Vec<u8>), String> {
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
    // key 4 where the sender's post-quantum ratchet had something to carry
    let pq = match map_get(m, 4) {
        Some(Item::Bytes(r)) => h[r.clone()].to_vec(),
        None => Vec::new(),
        _ => return Err("header pq".into()),
    };
    Ok((dh, pn, n, pq))
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

impl<P: PostQuantumRatchet + Clone + Default> Ratchet<P> {
    /// The initiator, holding the shared secret and the responder's signed
    /// prekey as its first ratchet key, with a fresh ratchet key of its own.
    pub fn initiator(sk: [u8; 32], their_spk: DhPublic, dhs: DhSecret, ad: Vec<u8>) -> Self {
        let mut pq = P::default();
        let (rk, cks) = kdf_rk(&sk, &dhs.agree(&their_spk), pq.contribution());
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
            pq,
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
            pq: P::default(),
            ad,
        }
    }

    /// Encrypt one message: the sending chain advances, the message key is
    /// used once, and the header carries this party's ratchet key and
    /// counters.  `[header, ciphertext]`.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, String> {
        let cks = self.cks.ok_or("no sending chain yet")?;
        let (ck, mut mk) = kdf_ck(&cks);
        self.cks = Some(ck);
        let carried = self.pq.outgoing();
        let header = header_bytes(&self.dhs.public(), self.pn, self.ns, &carried);
        self.ns += 1;
        let ct = seal(&mk, &self.ad, &header, plaintext);
        mk.zeroize();
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
        let (dh, pn, n, carried) = parse_header(&header)?;
        // on a trial copy, so a header this session cannot use leaves the
        // post-quantum half as it was along with everything else
        let mut trial = self.clone();
        if !carried.is_empty() {
            trial.pq.incoming(&carried)?;
        }
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
        if let Some(mut mk) = self.skipped.remove(&(dh.0, n)) {
            let opened = open(&mk, &self.ad, header, ct);
            // spent either way: it opened the message it was kept for, or
            // it did not and is not wanted again
            mk.zeroize();
            return opened;
        }
        if self.dhr != Some(dh) {
            self.skip(pn)?;
            self.dh_ratchet(dh, fresh);
        }
        self.skip(n)?;
        let ckr = self.ckr.ok_or("no receiving chain")?;
        let (ck, mut mk) = kdf_ck(&ckr);
        self.ckr = Some(ck);
        self.nr += 1;
        let opened = open(&mk, &self.ad, header, ct);
        mk.zeroize();
        opened
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
        let (rk, ckr) = kdf_rk(&self.rk, &self.dhs.agree(&dh), self.pq.contribution());
        self.rk = rk;
        self.ckr = Some(ckr);
        self.dhs = DhSecret::from_seed(fresh());
        let (rk, cks) = kdf_rk(&self.rk, &self.dhs.agree(&dh), self.pq.contribution());
        self.rk = rk;
        self.cks = Some(cks);
    }

    /// The state as it goes to the device's own storage (`crate::durable`).
    pub fn encode(&self) -> Vec<u8> {
        use crate::durable::*;
        let mut out = Vec::new();
        emit_array_head(&mut out, 11);
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
        // the post-quantum half's own state; empty at the floor
        emit_bstr(&mut out, &self.pq.encode());
        out
    }

    /// One back, whole or not at all.
    ///
    /// **Both shapes are read**: a state written before the seam existed
    /// carries ten elements and no post-quantum half, which is what the
    /// floor writes anyway.
    pub fn decode(b: &[u8]) -> Option<Ratchet<P>> {
        use crate::durable::*;
        let (_, f) = parse_array(b, 11).or_else(|| parse_array(b, 10))?;
        let mut pq = P::default();
        if let Some(it) = f.get(10) {
            pq.restore(&bytes(b, it)?).ok()?;
        }
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
            pq,
            ad: bytes(b, &f[9])?,
        })
    }

    /// Whether this party can send yet: the responder cannot until it has
    /// received the initiator's first message.
    pub fn can_send(&self) -> bool {
        self.cks.is_some()
    }
}
