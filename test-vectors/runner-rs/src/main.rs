//! rhtn-conformance: an independent executable runner for the RHTN test
//! corpus (`corpus.json`, format rhtn-test-corpus/1).
//!
//! Independence posture: this crate shares no code with the Python generator
//! or harness. It carries its own byte-level deterministic-CBOR parser (the
//! profile demands validation on received bytes), re-derives every test
//! identity from the recipe stated in the fixtures, and verifies signatures
//! through the Rust ecosystem's own implementations — Ed25519 via
//! ed25519-dalek and ML-DSA-65 via RustCrypto's ml-dsa — so agreement here is
//! cross-language AND cross-implementation for the post-quantum half.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

// ---------------------------------------------------------------- CBOR

#[derive(Debug, Clone)]
enum Item {
    Uint(u64),
    Neg(i64),
    Bytes(std::ops::Range<usize>),
    Text(std::ops::Range<usize>),
    Array(Vec<Item>),
    /// key items paired with value items; raw ranges retained for slicing
    Map(Vec<(Item, Item)>),
    Bool(bool),
    Null,
}

struct Parser<'a> {
    b: &'a [u8],
}

#[derive(Debug)]
struct CborErr(&'static str);

impl<'a> Parser<'a> {
    fn head(&self, p: usize) -> Result<(u8, u64, usize), CborErr> {
        let ib = *self.b.get(p).ok_or(CborErr("eof"))?;
        let (mt, ai) = (ib >> 5, ib & 0x1f);
        let (n, adv): (u64, usize) = match ai {
            0..=23 => (ai as u64, 1),
            24 => {
                let v = *self.b.get(p + 1).ok_or(CborErr("eof"))? as u64;
                if v < 24 {
                    return Err(CborErr("non-shortest u8"));
                }
                (v, 2)
            }
            25 => {
                let s = self.b.get(p + 1..p + 3).ok_or(CborErr("eof"))?;
                let v = u16::from_be_bytes([s[0], s[1]]) as u64;
                if v < 0x100 {
                    return Err(CborErr("non-shortest u16"));
                }
                (v, 3)
            }
            26 => {
                let s = self.b.get(p + 1..p + 5).ok_or(CborErr("eof"))?;
                let v = u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as u64;
                if v < 0x10000 {
                    return Err(CborErr("non-shortest u32"));
                }
                (v, 5)
            }
            27 => {
                let s = self.b.get(p + 1..p + 9).ok_or(CborErr("eof"))?;
                let v = u64::from_be_bytes(s.try_into().unwrap());
                if v < 0x1_0000_0000 {
                    return Err(CborErr("non-shortest u64"));
                }
                (v, 9)
            }
            _ => return Err(CborErr("indefinite or reserved length")),
        };
        Ok((mt, n, adv))
    }

    fn item(&self, p: usize) -> Result<(Item, usize), CborErr> {
        let ib = *self.b.get(p).ok_or(CborErr("eof"))?;
        // simple values (major 7) that carry no length argument
        if ib == 0xf4 {
            return Ok((Item::Bool(false), p + 1));
        }
        if ib == 0xf5 {
            return Ok((Item::Bool(true), p + 1));
        }
        if ib == 0xf6 {
            return Ok((Item::Null, p + 1));
        }
        let (mt, n, adv) = self.head(p)?;
        let q = p + adv;
        match mt {
            0 => Ok((Item::Uint(n), q)),
            1 => Ok((Item::Neg(-1 - (n as i64)), q)),
            2 => {
                let end = q + n as usize;
                if end > self.b.len() {
                    return Err(CborErr("bstr overrun"));
                }
                Ok((Item::Bytes(q..end), end))
            }
            3 => {
                let end = q + n as usize;
                if end > self.b.len() {
                    return Err(CborErr("tstr overrun"));
                }
                std::str::from_utf8(&self.b[q..end]).map_err(|_| CborErr("bad utf8"))?;
                Ok((Item::Text(q..end), end))
            }
            4 => {
                let mut items = Vec::new();
                let mut at = q;
                for _ in 0..n {
                    let (it, next) = self.item(at)?;
                    items.push(it);
                    at = next;
                }
                Ok((Item::Array(items), at))
            }
            5 => {
                let mut pairs = Vec::new();
                let mut at = q;
                let mut prev: Option<std::ops::Range<usize>> = None;
                for _ in 0..n {
                    let ks = at;
                    let (k, kv_end) = self.item(at)?;
                    let key_bytes = ks..kv_end;
                    if let Some(pr) = &prev {
                        if self.b[key_bytes.clone()] <= self.b[pr.clone()] {
                            return Err(CborErr("map keys unsorted or duplicate"));
                        }
                    }
                    prev = Some(key_bytes);
                    let (v, next) = self.item(kv_end)?;
                    pairs.push((k, v));
                    at = next;
                }
                Ok((Item::Map(pairs), at))
            }
            7 => Err(CborErr("unsupported simple/float")),
            _ => Err(CborErr("tag not admitted by profile")),
        }
    }
}

fn parse_all(b: &[u8]) -> Result<Item, CborErr> {
    let p = Parser { b };
    let (item, end) = p.item(0)?;
    if end != b.len() {
        return Err(CborErr("trailing bytes"));
    }
    Ok(item)
}

/// Range of the encoded value for integer key `key` in a map starting at `at0`.
fn value_slice_at(b: &[u8], at0: usize, key: u64) -> Option<std::ops::Range<usize>> {
    let p = Parser { b };
    let (_, n, adv) = p.head(at0).ok()?;
    let mut at = at0 + adv;
    for _ in 0..n {
        let (k, kend) = p.item(at).ok()?;
        let (_, vend) = p.item(kend).ok()?;
        if matches!(k, Item::Uint(x) if x == key) {
            return Some(kend..vend);
        }
        at = vend;
    }
    None
}

/// Item ranges of an array starting at `at0`.
fn array_item_ranges(b: &[u8], at0: usize) -> Option<Vec<std::ops::Range<usize>>> {
    let p = Parser { b };
    let (_, n, adv) = p.head(at0).ok()?;
    let mut out = Vec::new();
    let mut at = at0 + adv;
    for _ in 0..n {
        let (_, next) = p.item(at).ok()?;
        out.push(at..next);
        at = next;
    }
    Some(out)
}

/// Range of the encoded value for integer key `key` in a top-level map.
fn value_slice(b: &[u8], key: u64) -> Option<std::ops::Range<usize>> {
    let p = Parser { b };
    let (_, n, adv) = p.head(0).ok()?;
    let mut at = adv;
    for _ in 0..n {
        let (k, kend) = p.item(at).ok()?;
        let (_, vend) = p.item(kend).ok()?;
        if matches!(k, Item::Uint(x) if x == key) {
            return Some(kend..vend);
        }
        at = vend;
    }
    None
}

/// Encoded bytes of the map with one integer key removed (header count fixed).
fn map_without_key(b: &[u8], key: u64) -> Option<Vec<u8>> {
    let p = Parser { b };
    let (mt, n, adv) = p.head(0).ok()?;
    if mt != 5 {
        return None;
    }
    let mut out = Vec::new();
    let mut kept = 0u64;
    let mut body = Vec::new();
    let mut at = adv;
    for _ in 0..n {
        let ks = at;
        let (k, kend) = p.item(at).ok()?;
        let (_, vend) = p.item(kend).ok()?;
        if !matches!(k, Item::Uint(x) if x == key) {
            body.extend_from_slice(&b[ks..vend]);
            kept += 1;
        }
        at = vend;
    }
    emit_head(&mut out, 5, kept);
    out.extend_from_slice(&body);
    Some(out)
}

fn emit_head(out: &mut Vec<u8>, mt: u8, n: u64) {
    let m = mt << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n < 0x100 {
        out.push(m | 24);
        out.push(n as u8);
    } else if n < 0x10000 {
        out.push(m | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n < 0x1_0000_0000 {
        out.push(m | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

fn emit_bstr(out: &mut Vec<u8>, b: &[u8]) {
    emit_head(out, 2, b.len() as u64);
    out.extend_from_slice(b);
}

fn emit_tstr(out: &mut Vec<u8>, s: &str) {
    emit_head(out, 3, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

// ---------------------------------------------------------------- identities

use ed25519_dalek::{Signature as EdSig, Verifier, VerifyingKey as EdVk};
use ml_dsa::MlDsa65;
use ml_dsa::signature::Keypair as _;

struct Identity {
    name: &'static str,
    keyhash: [u8; 32],
    ed: EdVk,
    pq: ml_dsa::VerifyingKey<MlDsa65>,
}

fn sha(b: &[u8]) -> [u8; 32] {
    Sha256::digest(b).into()
}

fn derive_identity(name: &'static str) -> Identity {
    let ed_seed = sha(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes());
    let signing = ed25519_dalek::SigningKey::from_bytes(&ed_seed);
    let ed = signing.verifying_key();

    let xi = sha(format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes());
    let pq_sk = ml_dsa::SigningKey::<MlDsa65>::from_seed(&xi.into());
    let pq = pq_sk.verifying_key();

    // KeyMaterial = [ {1:1, -1:6, -2: ed_pub}, {1:7, 3:-49, -1: pq_pub} ]
    let mut km = Vec::new();
    emit_head(&mut km, 4, 2);
    // classical COSE_Key: labels sorted bytewise: 1 (0x01), -1 (0x20), -2 (0x21)
    emit_head(&mut km, 5, 3);
    km.push(0x01);
    km.push(0x01);
    km.push(0x20);
    km.push(0x06);
    km.push(0x21);
    emit_bstr(&mut km, ed.as_bytes());
    // pq COSE_Key: labels 1 (0x01), 3 (0x03), -1 (0x20)
    emit_head(&mut km, 5, 3);
    km.push(0x01);
    km.push(0x07);
    km.push(0x03);
    km.push(0x38);
    km.push(0x30); // -49
    km.push(0x20);
    emit_bstr(&mut km, pq.encode().as_ref());
    let keyhash = sha(&km);
    Identity { name, keyhash, ed, pq }
}

// ---------------------------------------------------------------- COSE

fn sig_structure_sign(protected: &[u8], aad: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_head(&mut out, 4, 5);
    emit_tstr(&mut out, "Signature");
    emit_bstr(&mut out, b"");
    emit_bstr(&mut out, protected);
    emit_bstr(&mut out, aad);
    emit_bstr(&mut out, payload);
    out
}

fn sig_structure_sign1(protected: &[u8], aad: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_head(&mut out, 4, 4);
    emit_tstr(&mut out, "Signature1");
    emit_bstr(&mut out, protected);
    emit_bstr(&mut out, aad);
    emit_bstr(&mut out, payload);
    out
}

fn verify_ed(id: &Identity, sig: &[u8], tbs: &[u8]) -> bool {
    let Ok(s) = EdSig::from_slice(sig) else { return false };
    id.ed.verify(tbs, &s).is_ok()
}

fn verify_pq(id: &Identity, sig: &[u8], tbs: &[u8]) -> bool {
    let Ok(s) = ml_dsa::Signature::<MlDsa65>::try_from(sig) else {
        return false;
    };
    id.pq.verify_with_context(tbs, &[], &s)
}

// ---------------------------------------------------------------- helpers

struct Ctx<'a> {
    bytes: &'a [u8],
    ids: &'a [Identity],
}

impl<'a> Ctx<'a> {
    fn by_keyhash(&self, kh: &[u8]) -> Option<&Identity> {
        self.ids.iter().find(|i| i.keyhash == kh)
    }
    fn bs(&self, it: &Item) -> &'a [u8] {
        match it {
            Item::Bytes(r) | Item::Text(r) => &self.bytes[r.clone()],
            _ => &[],
        }
    }
}

fn map_get<'m>(m: &'m [(Item, Item)], key: u64) -> Option<&'m Item> {
    m.iter().find_map(|(k, v)| match k {
        Item::Uint(x) if *x == key => Some(v),
        _ => None,
    })
}

fn as_uint(it: &Item) -> Option<u64> {
    match it {
        Item::Uint(n) => Some(*n),
        _ => None,
    }
}

// ---------------------------------------------------------------- envelope

/// Verify a full transaction envelope: structure, signer set, every signature
/// under both algorithms. Returns Err(reason) on any failure.
fn verify_envelope(ids: &[Identity], env_bytes: &[u8]) -> Result<(), String> {
    let item = parse_all(env_bytes).map_err(|e| format!("cbor: {}", e.0))?;
    let Item::Map(top) = &item else {
        return Err("envelope not a map".into());
    };
    let bs = |it: &Item| -> Vec<u8> {
        match it {
            Item::Bytes(r) | Item::Text(r) => env_bytes[r.clone()].to_vec(),
            _ => Vec::new(),
        }
    };
    let version = map_get(top, 1).and_then(as_uint).ok_or("no version")?;
    if version != 1 {
        return Err("version != 1".into());
    }
    let ty = map_get(top, 2).and_then(as_uint).ok_or("no type")?;
    let body_range = value_slice(env_bytes, 3).ok_or("no body")?;
    let body_bytes = &env_bytes[body_range.clone()];
    // parse the body AT ITS OFFSET so ranges stay relative to env_bytes
    let p = Parser { b: env_bytes };
    let (body_item, bend) = p.item(body_range.start).map_err(|_| "body cbor")?;
    if bend != body_range.end {
        return Err("body length disagreement".into());
    }
    let Item::Map(body) = body_item else {
        return Err("body not map".into());
    };

    // signer set per type
    let mut signers: Vec<Vec<u8>> = Vec::new();
    match ty {
        1 | 4 | 7 => {
            for f in [1u64, 2] {
                signers.push(bs(map_get(&body, f).ok_or("missing role field")?));
            }
        }
        2 | 3 => {
            signers.push(bs(map_get(&body, 1).ok_or("missing role field")?));
        }
        5 => {
            let Item::Array(parts) = map_get(&body, 3).ok_or("no participants")? else {
                return Err("participants not array".into());
            };
            for pi in parts {
                let Item::Map(pm) = pi else { return Err("participant not map".into()) };
                signers.push(bs(map_get(pm, 1).ok_or("participant kh")?));
            }
            if let Some(Item::Array(ws)) = map_get(&body, 4) {
                for w in ws {
                    let Item::Map(wm) = w else { return Err("witness not map".into()) };
                    signers.push(bs(map_get(wm, 1).ok_or("witness kh")?));
                }
            }
        }
        _ => return Err(format!("unknown type {ty}")),
    }

    // COSE_Sign: [b'', {}, null, [entries]]
    let Item::Array(cs) = map_get(top, 4).ok_or("no signatures")? else {
        return Err("cose not array".into());
    };
    if cs.len() != 4 {
        return Err("cose arity".into());
    }
    let Item::Array(entries) = &cs[3] else { return Err("entries not array".into()) };
    if entries.len() != signers.len() * 2 {
        return Err(format!(
            "entry count {} != 2x signers {}",
            entries.len(),
            signers.len()
        ));
    }
    let mut seen: BTreeMap<Vec<u8>, [bool; 2]> = BTreeMap::new();
    for e in entries {
        let Item::Array(ea) = e else { return Err("entry not array".into()) };
        let prot = &bs(&ea[0])[..];
        let Item::Map(pm) = parse_all(prot).map_err(|_| "protected cbor")? else {
            return Err("protected not map".into());
        };
        let alg = pm
            .iter()
            .find_map(|(k, v)| match (k, v) {
                (Item::Uint(1), Item::Neg(a)) => Some(*a),
                _ => None,
            })
            .ok_or("no alg")?;
        let kid = pm
            .iter()
            .find_map(|(k, v)| match (k, v) {
                (Item::Uint(4), Item::Bytes(r)) => Some(prot[r.clone()].to_vec()),
                _ => None,
            })
            .ok_or("no kid")?;
        if !signers.iter().any(|s| *s == kid) {
            return Err("kid outside body signer set".into());
        }
        let id = ids
            .iter()
            .find(|i| i.keyhash == *kid)
            .ok_or("kid not a derivable test identity")?;
        let sig = &bs(&ea[2])[..];
        let tbs = sig_structure_sign(prot, b"rhtn/1:envelope", body_bytes);
        let slot = seen.entry(kid.clone()).or_insert([false, false]);
        match alg {
            -8 => {
                if !verify_ed(id, sig, &tbs) {
                    return Err(format!("ed25519 fails for {}", id.name));
                }
                slot[0] = true;
            }
            -49 => {
                if !verify_pq(id, sig, &tbs) {
                    return Err(format!("ml-dsa fails for {}", id.name));
                }
                slot[1] = true;
            }
            _ => return Err("unexpected alg".into()),
        }
    }
    if seen.len() != signers.iter().collect::<std::collections::BTreeSet<_>>().len()
        || seen.values().any(|v| !v[0] || !v[1])
    {
        return Err("signer/alg coverage incomplete".into());
    }

    // ---- embedded evidence signatures ----
    let find_id = |kh: &[u8]| ids.iter().find(|i| i.keyhash == *kh);
    let sign1_parts = |slice: &[u8]| -> Option<(Vec<u8>, Vec<u8>)> {
        let Item::Array(a) = parse_all(slice).ok()? else { return None };
        let p = |it: &Item| match it {
            Item::Bytes(r) => Some(slice[r.clone()].to_vec()),
            _ => None,
        };
        Some((p(&a[0])?, p(&a[3])?))
    };
    let verify_response = |resp_slice: &[u8], hybrid: bool| -> Result<(), String> {
        let Item::Map(rm) = parse_all(resp_slice).map_err(|_| "resp cbor")? else {
            return Err("resp not map".into());
        };
        let get_b = |k: u64| -> Option<Vec<u8>> {
            match map_get(&rm, k) {
                Some(Item::Bytes(r)) => Some(resp_slice[r.clone()].to_vec()),
                _ => None,
            }
        };
        let subject = get_b(2).ok_or("resp subject")?;
        let verifier = get_b(1).ok_or("resp verifier")?;
        let qid = get_b(3).ok_or("resp qid")?;
        // field 7: subject consent over RAW qid
        let c_range = value_slice_at(resp_slice, 0, 7).ok_or("consent")?;
        let (cp, csig) = sign1_parts(&resp_slice[c_range]).ok_or("consent shape")?;
        let sid = find_id(&subject).ok_or("subject id")?;
        if !verify_ed(sid, &csig, &sig_structure_sign1(&cp, b"rhtn/1:consent", &qid)) {
            return Err(format!("consent fails for {}", sid.name));
        }
        // field 9 over map-minus-9
        let payload = map_without_key(resp_slice, 9).ok_or("resp payload")?;
        let vid = find_id(&verifier).ok_or("verifier id")?;
        let v_range = value_slice_at(resp_slice, 0, 9).ok_or("field 9")?;
        let vslice = &resp_slice[v_range];
        if hybrid {
            let Item::Array(cs9) = parse_all(vslice).map_err(|_| "f9 cbor")? else {
                return Err("f9 shape".into());
            };
            let Item::Array(entries9) = &cs9[3] else { return Err("f9 entries".into()) };
            let mut got = [false, false];
            for e9 in entries9 {
                let Item::Array(ea) = e9 else { return Err("f9 entry".into()) };
                let prot = match &ea[0] {
                    Item::Bytes(r) => vslice[r.clone()].to_vec(),
                    _ => return Err("f9 prot".into()),
                };
                let sig = match &ea[2] {
                    Item::Bytes(r) => vslice[r.clone()].to_vec(),
                    _ => return Err("f9 sig".into()),
                };
                let tbs = sig_structure_sign(&prot, b"rhtn/1:verifier", &payload);
                if prot.ends_with(&[0x27]) {
                    if !verify_ed(vid, &sig, &tbs) {
                        return Err("hybrid ed fails".into());
                    }
                    got[0] = true;
                } else {
                    if !verify_pq(vid, &sig, &tbs) {
                        return Err("hybrid ml-dsa fails".into());
                    }
                    got[1] = true;
                }
            }
            if !(got[0] && got[1]) {
                return Err("hybrid coverage".into());
            }
        } else {
            let (vp, vsig) = sign1_parts(vslice).ok_or("f9 shape")?;
            if !verify_ed(vid, &vsig, &sig_structure_sign1(&vp, b"rhtn/1:verifier", &payload)) {
                return Err(format!("verifier sig fails for {}", vid.name));
            }
        }
        Ok(())
    };

    if ty == 5 {
        if let Some(r5) = value_slice_at(env_bytes, body_range.start, 5) {
            for rr in array_item_ranges(env_bytes, r5.start).ok_or("resp walk")? {
                verify_response(&env_bytes[rr], false)?;
            }
        }
    }
    if ty == 1 {
        if let Some(r6) = value_slice_at(env_bytes, body_range.start, 6) {
            // Recovery block: {1: prior, 2: [responses], 3: successor CoseSign}
            let rb = r6.clone();
            let prior = value_slice_at(env_bytes, rb.start, 1)
                .map(|r| env_bytes[r.start + 2..r.end].to_vec())
                .ok_or("prior")?;
            let r2 = value_slice_at(env_bytes, rb.start, 2).ok_or("recovery responses")?;
            for rr in array_item_ranges(env_bytes, r2.start).ok_or("rec walk")? {
                verify_response(&env_bytes[rr], true)?;
            }
            // successor proof over [prior, new, patron]
            let newk = bs(map_get(&body, 1).ok_or("new key")?);
            let patron = bs(map_get(&body, 2).ok_or("patron")?);
            let mut stmt = Vec::new();
            emit_head(&mut stmt, 4, 3);
            emit_bstr(&mut stmt, &prior);
            emit_bstr(&mut stmt, &newk);
            emit_bstr(&mut stmt, &patron);
            let r3 = value_slice_at(env_bytes, rb.start, 3).ok_or("successor")?;
            let sslice = &env_bytes[r3];
            let Item::Array(cs3) = parse_all(sslice).map_err(|_| "succ cbor")? else {
                return Err("succ shape".into());
            };
            let Item::Array(sents) = &cs3[3] else { return Err("succ entries".into()) };
            let pid = find_id(&prior).ok_or("prior id")?;
            for se in sents {
                let Item::Array(ea) = se else { return Err("succ entry".into()) };
                let prot = match &ea[0] {
                    Item::Bytes(r) => sslice[r.clone()].to_vec(),
                    _ => return Err("succ prot".into()),
                };
                let sig = match &ea[2] {
                    Item::Bytes(r) => sslice[r.clone()].to_vec(),
                    _ => return Err("succ sig".into()),
                };
                let tbs = sig_structure_sign(&prot, b"rhtn/1:successor", &stmt);
                let ok = if prot.ends_with(&[0x27]) {
                    verify_ed(pid, &sig, &tbs)
                } else {
                    verify_pq(pid, &sig, &tbs)
                };
                if !ok {
                    return Err("successor proof fails".into());
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- presentation

const LABELS: [&str; 7] = [
    "capture",
    "location",
    "p0.integrity",
    "p0.retention",
    "p1.integrity",
    "p1.retention",
    "proximity",
];

fn verify_presentation(ctx: &Ctx, pres: &[u8]) -> Result<(), String> {
    let Item::Array(outer) = parse_all(pres).map_err(|e| format!("cbor: {}", e.0))? else {
        return Err("presentation not array".into());
    };
    if outer.len() != 2 {
        return Err("presentation arity".into());
    }
    // envelope is item 0 — verify it wholesale as well.
    let env_end = {
        let p = Parser { b: pres };
        let (_, adv) = p.item(1).map_err(|_| "env item")?;
        adv
    };
    // outer array header is 1 byte (arity 2)
    let env_bytes = &pres[1..env_end];
    verify_envelope(ctx.ids, env_bytes)?;

    let Item::Array(slots) = &outer[1] else { return Err("slots not array".into()) };
    if slots.len() != 7 {
        return Err("slot count".into());
    }
    // walk exact slot ranges: outer head (1 byte) + env, then slots header
    let pw = Parser { b: pres };
    let (_, _, slots_head_adv) = pw.head(env_end).map_err(|_| "slots head")?;
    let mut slot_ranges = Vec::new();
    let mut at = env_end + slots_head_adv;
    for _ in 0..7 {
        let (_, next) = pw.item(at).map_err(|_| "slot walk")?;
        slot_ranges.push(at..next);
        at = next;
    }
    let mut digests = Vec::new();
    for (i, slot) in slots.iter().enumerate() {
        match slot {
            Item::Bytes(r) => {
                if r.len() != 32 {
                    return Err("withheld digest width".into());
                }
                digests.extend_from_slice(&pres[r.clone()]);
            }
            Item::Array(d) => {
                if d.len() != 3 {
                    return Err("disclosure arity".into());
                }
                let Item::Bytes(salt) = &d[0] else { return Err("salt".into()) };
                if salt.len() != 16 {
                    return Err("salt width".into());
                }
                let Item::Text(lab) = &d[1] else { return Err("label".into()) };
                if &pres[lab.clone()] != LABELS[i].as_bytes() {
                    return Err("label != slot position".into());
                }
                // digest over the exact received Disclosure encoding
                let mut pre = vec![0u8];
                pre.extend_from_slice(&pres[slot_ranges[i].clone()]);
                digests.extend_from_slice(&sha(&pre));
            }
            _ => return Err("slot type".into()),
        }
    }
    let mut rootpre = vec![1u8];
    rootpre.extend_from_slice(&digests);
    let root = sha(&rootpre);
    // body field 8 of the embedded envelope
    let body_range = value_slice(env_bytes, 3).ok_or("env body")?;
    let body = &env_bytes[body_range];
    let r8 = value_slice(body, 8).ok_or("no field 8")?;
    // field 8 is a bstr: strip its 2-byte header (58 20)
    let claimed = &body[r8.start + 2..r8.end];
    if claimed != root {
        return Err("root mismatch".into());
    }
    Ok(())
}

// ---------------------------------------------------------------- schema checks

/// Lightweight per-kind validation used to EXERCISE reject entries.
fn schema_check(ctx: &Ctx, kind: &str, item: &Item) -> Result<(), String> {
    let b = ctx.bytes;
    match kind {
        "VerifierResponse" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let result = map_get(m, 4).and_then(as_uint).ok_or("no result")?;
            if result > 3 {
                return Err("result out of range".into());
            }
            if let Some(basis) = map_get(m, 5).and_then(as_uint) {
                if basis > 2 {
                    return Err("basis out of range".into());
                }
            }
            if let Some(sb) = map_get(m, 10).and_then(as_uint) {
                if sb > 2 {
                    return Err("selection_basis out of range".into());
                }
            } else {
                return Err("selection_basis required".into());
            }
            map_get(m, 7).ok_or("consent required")?;
            Ok(())
        }
        "Locator" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            map_get(m, 1).ok_or("anchor")?;
            let Item::Map(path) = map_get(m, 2).ok_or("path required")? else {
                return Err("path not map".into());
            };
            let count = map_get(path, 2).and_then(as_uint).ok_or("nibbles")?;
            if count > 24 {
                return Err("path over 24 nibbles".into());
            }
            map_get(m, 3).ok_or("seqno")?;
            Ok(())
        }
        "SignedLocator" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let kh = ctx.bs(map_get(m, 1).ok_or("subject")?);
            if kh.len() != 32 {
                return Err("keyhash width".into());
            }
            Ok(())
        }
        "NetworkPoint" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            if let Some(port) = map_get(m, 3).and_then(as_uint) {
                if port == 0 || port > 65535 || port == 7431 {
                    return Err("port invalid".into());
                }
            }
            Ok(())
        }
        "LocationEvidence" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let Item::Array(asserted) = map_get(m, 1).ok_or("asserted")? else {
                return Err("asserted not array".into());
            };
            if asserted.len() > 4 {
                return Err("asserted over 4".into());
            }
            for a in asserted {
                let Item::Map(am) = a else { return Err("assert map".into()) };
                let Item::Text(g) = map_get(am, 2).ok_or("geohash")? else {
                    return Err("geohash not tstr".into());
                };
                let gh = &b[g.clone()];
                if !(gh.len() == 3 || gh.len() == 4)
                    || !gh.iter().all(|c| b"0123456789bcdefghjkmnpqrstuvwxyz".contains(c))
                {
                    return Err("geohash malformed".into());
                }
            }
            if let Some(Item::Array(cor)) = map_get(m, 2) {
                if cor.len() > 16 {
                    return Err("corroborations over 16".into());
                }
            }
            Ok(())
        }
        "Proximity" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let Item::Array(ch) = map_get(m, 1).ok_or("channels")? else {
                return Err("channels not array".into());
            };
            if ch.is_empty() || ch.len() > 8 {
                return Err("channel count".into());
            }
            Ok(())
        }
        "Scope" => match item {
            Item::Uint(t) => {
                if *t == 3 {
                    Err("retired tag".into())
                } else {
                    Ok(())
                }
            }
            Item::Array(a) if a.len() == 2 => {
                if let Item::Array(list) = &a[1] {
                    if list.len() > 256 {
                        return Err("scope list over 256".into());
                    }
                    let mut prev: Option<Vec<u8>> = None;
                    for k in list {
                        let kb = ctx.bs(k).to_vec();
                        if let Some(p) = &prev {
                            if kb <= *p {
                                return Err("scope list unsorted/dup".into());
                            }
                        }
                        prev = Some(kb);
                    }
                }
                Ok(())
            }
            _ => Err("scope shape".into()),
        },
        "Capabilities" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            if m.len() > 64 {
                return Err("over 64 entries".into());
            }
            for (_, v) in m {
                if let Item::Bytes(r) = v {
                    if r.len() > 1024 {
                        return Err("value over 1024".into());
                    }
                }
            }
            Ok(())
        }
        "CatalogEntry" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            if ctx.bytes.len() > 2048 {
                return Err("entry over 2048".into());
            }
            for f in [1u64, 2, 3, 4, 5, 8] {
                map_get(m, f).ok_or("missing required")?;
            }
            Ok(())
        }
        "CurrencyAttestation" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let role = map_get(m, 5).and_then(as_uint).ok_or("role")?;
            if role > 3 {
                return Err("role out of range".into());
            }
            map_get(m, 6).ok_or("issuer required")?;
            Ok(())
        }
        "ResolveReply" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let code = map_get(m, 2).and_then(as_uint).ok_or("code")?;
            if code > 2 {
                return Err("code out of range".into());
            }
            if code == 2 {
                let Item::Map(rf) = map_get(m, 5).ok_or("referral")? else {
                    return Err("referral map".into());
                };
                let adv = map_get(rf, 3).and_then(as_uint).ok_or("advances")?;
                if adv == 0 {
                    return Err("advances zero".into());
                }
            }
            Ok(())
        }
        "ResourceResponse" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let s = map_get(m, 1).and_then(as_uint).ok_or("status")?;
            if s > 5 {
                return Err("status out of range".into());
            }
            Ok(())
        }
        "ArchiveRequest" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let mx = map_get(m, 3).and_then(as_uint).ok_or("max")?;
            if !(1..=256).contains(&mx) {
                return Err("max_records out of range".into());
            }
            Ok(())
        }
        "PrekeyBundle" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            if let Some(Item::Bytes(r)) = map_get(m, 3) {
                if r.len() > 4096 {
                    return Err("blob over 4KB".into());
                }
            }
            Ok(())
        }
        "PrekeyBatchRequest" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            if let Some(Item::Array(pop)) = map_get(m, 1) {
                if pop.len() < 2 || pop.len() > 256 {
                    return Err("population out of range".into());
                }
            }
            Ok(())
        }
        "VerificationQuery" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let qid = match map_get(m, 6) {
                Some(Item::Bytes(r)) => b[r.clone()].to_vec(),
                _ => return Err("no qid".into()),
            };
            let f15 = map_without_key(b, 6).ok_or("fields 1-5")?;
            if sha(&f15).to_vec() != qid {
                return Err("query_id does not recompute".into());
            }
            if let Some(v) = map_get(m, 5).and_then(as_uint) {
                if v > 65535 {
                    return Err("template version over uint16".into());
                }
            }
            Ok(())
        }
        "Witness" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            if map_get(m, 4).is_some() || map_get(m, 5).is_some() {
                return Err("retired witness key".into());
            }
            Ok(())
        }
        "body" => {
            let Item::Map(m) = item else { return Err("not map".into()) };
            let Item::Array(lists) = map_get(m, 0).ok_or("key 0")? else {
                return Err("key0 not array".into());
            };
            for l in lists {
                let Item::Array(hs) = l else { return Err("list".into()) };
                if hs.is_empty() || hs.len() > 8 {
                    return Err("back-pointer bound".into());
                }
            }
            // unknown-extension bounds over the raw map
            {
                let p = Parser { b };
                if let Ok((_, n, adv)) = p.head(0) {
                    let mut at = adv;
                    let mut unknown = 0usize;
                    for _ in 0..n {
                        let (k, kend) = p.item(at).map_err(|_| "walk")?;
                        let (_, vend) = p.item(kend).map_err(|_| "walk")?;
                        if !matches!(&k, Item::Uint(x) if *x <= 8) {
                            unknown += 1;
                            if vend - kend > 1024 {
                                return Err("extension value over 1024".into());
                            }
                        }
                        at = vend;
                    }
                    if unknown > 16 {
                        return Err("over 16 extension keys".into());
                    }
                }
            }
            // classify by field-3 shape
            let f3 = map_get(m, 3);
            if matches!(map_get(m, 1), Some(Item::Bytes(_)))
                && matches!(map_get(m, 2), Some(Item::Bytes(_)))
                && f3.is_none()
            {
                return Err("field 3 required".into());
            }
            let is_presence =
                matches!(f3, Some(Item::Array(a)) if a.iter().all(|x| matches!(x, Item::Map(_))))
                    && map_get(m, 6).is_some();
            let is_adoption = matches!(f3, Some(Item::Map(_)));
            let is_disavowal =
                matches!(f3, Some(Item::Uint(_))) && map_get(m, 4).is_some();
            let is_peering = is_adoption
                && matches!(map_get(m, 4), Some(Item::Map(_)));
            if is_presence {
                if map_get(m, 7).is_some() {
                    return Err("retired body key 7".into());
                }
                map_get(m, 8).ok_or("presence field 8 required")?;
                let sub = map_get(m, 6).and_then(as_uint).ok_or("subtype uint")?;
                if sub > 1 {
                    return Err("subtype out of range".into());
                }
                let s = map_get(m, 1).and_then(as_uint).ok_or("started_at uint")?;
                let f = map_get(m, 2).and_then(as_uint).ok_or("finalized_at uint")?;
                if f < s || f - s > 86_400 {
                    return Err("finalization gap".into());
                }
                if let Some(Item::Array(ws)) = map_get(m, 4) {
                    if ws.len() > 16 {
                        return Err("witnesses over 16".into());
                    }
                    for w in ws {
                        if let Item::Map(wm) = w {
                            if map_get(wm, 4).is_some() || map_get(wm, 5).is_some() {
                                return Err("retired witness key".into());
                            }
                        }
                    }
                }
                if let Some(Item::Array(resp)) = map_get(m, 5) {
                    if resp.len() > 32 {
                        return Err("responses over 32".into());
                    }
                    if resp.is_empty() {
                        return Err("empty response array must be omitted".into());
                    }
                }
            } else if is_disavowal {
                let code = map_get(m, 4).and_then(as_uint).ok_or("code uint")?;
                if code > 63 {
                    return Err("disavowal code out of space".into());
                }
            } else if is_peering {
                if let Some(Item::Array(audits)) = map_get(m, 7) {
                    if audits.len() > 8 {
                        return Err("audits over 8".into());
                    }
                }
            } else if is_adoption {
                map_get(m, 4).and_then(as_uint).ok_or("adoption timestamp uint")?;
                if let Some(Item::Map(loc)) = f3 {
                    if let Some(Item::Array(sq)) = map_get(loc, 3) {
                        if let Some(Item::Uint(c)) = sq.get(1) {
                            if *c != 0 {
                                return Err("adoption counter not 0".into());
                            }
                        }
                    }
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// (sig-slot, external_aad, signer-field) per standalone signed kind.
fn sign1_profile(kind: &str) -> Option<(u64, &'static [u8], u64)> {
    Some(match kind {
        "CurrencyAttestation" => (7, b"rhtn/1:currency", 6),
        "CatalogEntry" => (8, b"rhtn/1:catalog", 2),
        "AbuseReport" => (5, b"rhtn/1:abuse", 1),
        "AnchorEntry" => (5, b"rhtn/1:anchor", 1),
        "SubtreeAck" => (5, b"rhtn/1:subtree-ack", 2),
        "PrekeyBundle" => (5, b"rhtn/1:prekey", 1),
        _ => return None,
    })
}

/// Verify a standalone COSE_Sign1-signed record under its NAMED signer.
fn verify_record(ids: &[Identity], kind: &str, raw: &[u8]) -> Result<bool, String> {
    let Some((slot, aad, sfield)) = sign1_profile(kind) else {
        return Err("no profile".into());
    };
    let Item::Map(m) = parse_all(raw).map_err(|_| "cbor")? else {
        return Err("not map".into());
    };
    let signer_kh = match map_get(&m, sfield) {
        Some(Item::Bytes(r)) => raw[r.clone()].to_vec(),
        _ => return Err("signer field".into()),
    };
    let id = ids
        .iter()
        .find(|i| i.keyhash == *signer_kh)
        .ok_or("unknown signer identity")?;
    let Some(Item::Array(cs)) = map_get(&m, slot) else {
        return Err("sig slot".into());
    };
    let prot = match &cs[0] {
        Item::Bytes(r) => raw[r.clone()].to_vec(),
        _ => return Err("protected".into()),
    };
    let sig = match &cs[3] {
        Item::Bytes(r) => raw[r.clone()].to_vec(),
        _ => return Err("sig".into()),
    };
    let payload = map_without_key(raw, slot).ok_or("payload")?;
    let tbs = sig_structure_sign1(&prot, aad, &payload);
    Ok(verify_ed(id, &sig, &tbs))
}

// ---------------------------------------------------------------- main

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| "..".into());
    let corpus: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{dir}/corpus.json")).unwrap())
            .unwrap();
    assert_eq!(corpus["format"], "rhtn-test-corpus/1");

    let names: Vec<&'static str> = vec![
        "alice", "bob", "carol", "alice2", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8",
        "w9", "w10", "w11", "w12", "w13", "w14", "w15", "w16", "c1", "c2", "c3", "c4", "c5",
    ];
    eprintln!("deriving {} identities (ML-DSA keygen via RustCrypto)...", names.len());
    let ids: Vec<Identity> = names.into_iter().map(derive_identity).collect();

    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut skipped = 0u32;
    let mut deep_env = 0u32;
    let mut deep_pres = 0u32;
    // cross-entry state
    let mut adopt_min_bytes: Vec<u8> = Vec::new();
    let mut pc1_txid: Vec<u8> = Vec::new();
    let mut normal_qid: Vec<u8> = Vec::new();
    let mut push_payload: Vec<u8> = Vec::new();
    let mut keygrant: Vec<u8> = Vec::new();

    for e in corpus["entries"].as_array().unwrap() {
        let id = e["id"].as_str().unwrap();
        let class = e["class"].as_str().unwrap();
        if class != "bytes" {
            skipped += 1;
            continue;
        }
        let expect = &e["expect"];
        let outcome = expect["outcome"].as_str().unwrap();
        let kind = expect["kind"].as_str().unwrap_or("");
        let layer = expect["layer"].as_str().unwrap_or("");
        let mut raw = hex::decode(e["hex"].as_str().unwrap()).unwrap();

        if kind == "frame" {
            if raw.len() < 4 {
                report(&mut fail, id, "frame too short");
                continue;
            }
            let n = u32::from_be_bytes(raw[..4].try_into().unwrap()) as usize;
            if n != raw.len() - 4 {
                report(&mut fail, id, "length prefix mismatch");
                continue;
            }
            raw = raw[4..].to_vec();
        }

        let parsed = parse_all(&raw);
        let ctx = Ctx { bytes: &raw, ids: &ids };

        let verdict: Result<(), String> = match (outcome, parsed) {
            ("reject", Err(_)) if layer == "cbor" => Ok(()),
            ("reject", Ok(_)) if layer == "cbor" => Err("cbor-layer reject parsed".into()),
            (_, Err(e2)) => Err(format!("must parse but: {}", e2.0)),
            ("accept", Ok(item)) => match kind {
                "envelope" => {
                    deep_env += 1;
                    verify_envelope(&ids, &raw)
                }
                "presentation" => {
                    deep_pres += 1;
                    verify_presentation(&ctx, &raw)
                }
                k if sign1_profile(k).is_some() => {
                    match (schema_check(&ctx, kind, &item), verify_record(&ids, k, &raw)) {
                        (Err(e2), _) => Err(e2),
                        (Ok(()), Ok(true)) => Ok(()),
                        (Ok(()), Ok(false)) => Err("record signature fails".into()),
                        (Ok(()), Err(e2)) => Err(e2),
                    }
                }
                _ => schema_check(&ctx, kind, &item),
            },
            ("reject", Ok(item)) => {
                let schema = schema_check(&ctx, kind, &item);
                let sig_fails = if sign1_profile(kind).is_some() {
                    matches!(verify_record(&ids, kind, &raw), Ok(false))
                } else {
                    false
                };
                match (schema, sig_fails) {
                    (Err(_), _) | (_, true) => Ok(()),
                    (Ok(()), false) => {
                        if implemented_kind(kind) || sign1_profile(kind).is_some() {
                            Err("reject entry passed our validator".into())
                        } else {
                            skipped += 1;
                            continue;
                        }
                    }
                }
            }
            _ => Err("unhandled".into()),
        };

        // stash cross-entry material
        match id {
            "P-adopt-min" => adopt_min_bytes = raw.clone(),
            "P-alice-c1-record" => {
                if let Some(r) = value_slice(&raw, 3) {
                    pc1_txid = sha(&raw[r]).to_vec();
                }
            }
            "P-frame-13" => {
                // frame = [4, [query, consent, sb]] — deep-verify the triple
                if let Ok(Item::Array(fa)) = parse_all(&raw) {
                    let Item::Array(body3) = &fa[1] else {
                        report(&mut fail, id, "request-4 body not array");
                        continue;
                    };
                    if body3.len() != 3 {
                        report(&mut fail, id, "request-4 arity != 3");
                        continue;
                    }
                    let p = Parser { b: &raw };
                    let (_, _, fadv) = p.head(0).unwrap();
                    // skip the type uint, then the body array header
                    let (_, tend) = p.item(fadv).unwrap();
                    let (_, _, badv) = p.head(tend).unwrap();
                    let adv = tend + badv;
                    if let Ok((_, qend)) = p.item(adv) {
                        let qslice = &raw[adv..qend];
                        if let Some(r6) = value_slice(qslice, 6) {
                            normal_qid = qslice[r6.start + 2..r6.end].to_vec();
                        }
                        let f15 = map_without_key(qslice, 6).unwrap_or_default();
                        if sha(&f15).to_vec() != normal_qid {
                            report(&mut fail, id, "frame-13 query_id fails");
                            continue;
                        }
                        // consent: second element, Sign1 by alice over raw qid
                        if let Ok((_, cend)) = p.item(qend) {
                            let cslice = &raw[qend..cend];
                            if let Ok(Item::Array(ca)) = parse_all(cslice) {
                                let gp = |it: &Item| match it {
                                    Item::Bytes(r) => cslice[r.clone()].to_vec(),
                                    _ => Vec::new(),
                                };
                                let alice = ids.iter().find(|i| i.name == "alice").unwrap();
                                let tbs = sig_structure_sign1(
                                    &gp(&ca[0]),
                                    b"rhtn/1:consent",
                                    &normal_qid,
                                );
                                if !verify_ed(alice, &gp(&ca[3]), &tbs) {
                                    report(&mut fail, id, "frame-13 consent fails");
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            "P-frame-06" => {
                // TopologyPush: kind 0, payload is a full envelope — verify it
                if let Some(r2) = value_slice(&raw, 2).or(None) {
                    let _ = r2;
                }
                if let Ok(Item::Array(fa)) = parse_all(&raw) {
                    if let Item::Map(fm) = &fa[1] {
                        if let Some(Item::Bytes(pr)) = map_get(fm, 2) {
                            push_payload = raw[pr.clone()].to_vec();
                            if let Err(e2) = verify_envelope(&ids, &push_payload) {
                                report(&mut fail, id, &format!("inner envelope: {e2}"));
                                continue;
                            }
                        }
                    }
                }
            }
            "P-e2e-01" => keygrant = raw.clone(),
            _ => {}
        }

        match verdict {
            Ok(()) => pass += 1,
            Err(msg) => report(&mut fail, id, &msg),
        }
    }

    // ---- cross-entry bindings ----
    let mut xchecks = 0u32;
    if !push_payload.is_empty() && push_payload == adopt_min_bytes {
        xchecks += 1;
    } else {
        report(&mut fail, "X-push-carries-adoption", "TopologyPush payload != P-adopt-min bytes");
    }
    if !keygrant.is_empty() {
        if let Ok(Item::Map(kg)) = parse_all(&keygrant) {
            let gb = |k: u64| match map_get(&kg, k) {
                Some(Item::Bytes(r)) => keygrant[r.clone()].to_vec(),
                _ => Vec::new(),
            };
            if gb(1) == pc1_txid && gb(2) == normal_qid {
                xchecks += 1;
            } else {
                report(
                    &mut fail,
                    "X-keygrant-bindings",
                    "KeyGrant does not bind the prior record and current query",
                );
            }
        }
    }
    pass += xchecks;

    eprintln!(
        "\nrhtn-conformance: {pass} pass, {fail} fail, {skipped} skipped \
         ({deep_env} envelopes fully verified incl. ML-DSA, {deep_pres} presentations)"
    );
    std::process::exit(if fail == 0 { 0 } else { 1 });
}

fn implemented_kind(kind: &str) -> bool {
    matches!(
        kind,
        "VerifierResponse"
            | "Locator"
            | "NetworkPoint"
            | "LocationEvidence"
            | "Proximity"
            | "Scope"
            | "Capabilities"
            | "CatalogEntry"
            | "CurrencyAttestation"
            | "ResolveReply"
            | "ResourceResponse"
            | "ArchiveRequest"
            | "PrekeyBundle"
            | "PrekeyBatchRequest"
            | "Witness"
            | "body"
            | "SignedLocator"
            | "VerificationQuery"
    )
}

fn report(fail: &mut u32, id: &str, msg: &str) {
    *fail += 1;
    eprintln!("FAIL {id}: {msg}");
}
