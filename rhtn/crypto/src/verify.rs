//! Signature checks over parsed structures.  Each function takes an
//! identity lookup so the caller decides what it has pinned.

use crate::Identity;
use rhtn_codec::cbor::*;
use rhtn_codec::cose::{self, aad};
use rhtn_codec::encode::*;
use rhtn_codec::envelope;

/// Why a verification did not succeed (`wire-format.md` §3.4): a signer
/// whose key this verifier does not hold, which is the third outcome and
/// names the identity to fetch, or an object that fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    MissingKey(Vec<u8>),
    Invalid(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::MissingKey(k) => write!(f, "missing key for signer {}", k.iter().map(|b| format!("{b:02x}")).collect::<String>()),
            Failure::Invalid(s) => f.write_str(s),
        }
    }
}

impl From<&str> for Failure {
    fn from(s: &str) -> Self {
        Failure::Invalid(s.into())
    }
}

impl From<String> for Failure {
    fn from(s: String) -> Self {
        Failure::Invalid(s)
    }
}

/// Resolve a keyhash to a pinned identity.
pub trait Lookup {
    fn identity(&self, keyhash: &[u8]) -> Option<&Identity>;
}

impl Lookup for [Identity] {
    fn identity(&self, keyhash: &[u8]) -> Option<&Identity> {
        self.iter().find(|i| i.keyhash == keyhash)
    }
}
impl Lookup for Vec<Identity> {
    fn identity(&self, keyhash: &[u8]) -> Option<&Identity> {
        self.as_slice().identity(keyhash)
    }
}

fn parts_sign1(slice: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let __a_item = parse_all(slice).ok()?;
    let Item::Array(a) = &__a_item else { return None };
    if a.len() != 4 {
        return None;
    }
    let p = |it: &Item| match it {
        Item::Bytes(r) => Some(slice[r.clone()].to_vec()),
        _ => None,
    };
    let prot = p(&a[0])?;
    // a classical COSE_Sign1 whose enclosing structure names the signer:
    // alg -8 and nothing else, an empty unprotected header, a nil payload
    if named_signer_alg(&prot, &a[1]) != Some(cose::ALG_EDDSA) || !matches!(a[2], Item::Null) {
        return None;
    }
    Some((prot, p(&a[3])?))
}

/// The header rule for a signature whose enclosing structure names the
/// signer (`wire-format.md` §3.5): the protected header carries `alg` and
/// nothing else — no `kid`, which would be a second copy that could
/// disagree with the first — and the unprotected header is empty.  The
/// algorithm declared is returned for the caller to hold to the one its
/// profile requires; `None` where the headers depart from the rule.
fn named_signer_alg(prot: &[u8], unprotected: &Item) -> Option<i64> {
    if !matches!(unprotected, Item::Map(u) if u.is_empty()) {
        return None;
    }
    let parsed = parse_all(prot).ok()?;
    match &parsed {
        Item::Map(pm) if pm.len() == 1 => match &pm[0] {
            (Item::Uint(1), Item::Neg(a)) => Some(*a),
            _ => None,
        },
        _ => None,
    }
}

/// The container of a `COSE_Sign` in this profile (`wire-format.md` §1,
/// §3.5): the outer protected header is empty, since the structure carries
/// no signature of its own; the unprotected header is empty; and the
/// payload is nil, every signature being detached.  A container that
/// departs from this is a second encoding of the object, and malformed.
fn detached_sign_container(cs: &[Item]) -> bool {
    matches!(&cs[0], Item::Bytes(r) if r.is_empty()) && matches!(&cs[1], Item::Map(u) if u.is_empty()) && matches!(cs[2], Item::Null)
}

/// Verify an embedded `COSE_Sign` block signed by one known party: the
/// enclosing structure names the signer, so no entry carries a `kid` and
/// no header carries anything but `alg`.  One logical signer contributes
/// exactly two entries, classical then post-quantum (`wire-format.md`
/// §3.5), so an empty block, a lone entry, a duplicated algorithm or a
/// reversed pair is malformed before any signature is checked; both
/// entries must verify.
fn verify_sign_block(signer: &Identity, block: &[u8], aad_tag: &[u8], payload: &[u8]) -> Result<(), String> {
    let __cs_item = parse_all(block).map_err(|e| format!("cose: {e}"))?;
    let Item::Array(cs) = &__cs_item else {
        return Err("cose not array".into());
    };
    if cs.len() != 4 {
        return Err("cose arity".into());
    }
    if !detached_sign_container(cs) {
        return Err("cose container departs from the profile".into());
    }
    let Item::Array(entries) = &cs[3] else { return Err("entries not array".into()) };
    if entries.len() != 2 {
        return Err("a hybrid signer contributes exactly two entries".into());
    }
    let mut got = [false, false];
    for (i, e) in entries.iter().enumerate() {
        let Item::Array(ea) = e else { return Err("entry not array".into()) };
        if ea.len() != 3 {
            return Err("entry arity".into());
        }
        let (Some(Item::Bytes(pr)), Some(Item::Bytes(sr))) = (ea.first(), ea.get(2)) else {
            return Err("entry shape".into());
        };
        let prot = &block[pr.clone()];
        let sig = &block[sr.clone()];
        let Some(alg) = named_signer_alg(prot, &ea[1]) else {
            return Err("an embedded signature carries a header beyond alg".into());
        };
        if alg != [cose::ALG_EDDSA, cose::ALG_ML_DSA_65][i] {
            return Err("entries out of canonical order".into());
        }
        let tbs = cose::sig_structure_sign(prot, aad_tag, payload);
        match alg {
            cose::ALG_EDDSA => { if !signer.verify_ed(sig, &tbs) { return Err("ed25519 fails".into()); } got[0] = true; }
            _ => { if !signer.verify_pq(sig, &tbs) { return Err("ml-dsa fails".into()); } got[1] = true; }
        }
    }
    if got != [true, true] {
        return Err("signer/alg coverage incomplete".into());
    }
    Ok(())
}

/// A verifier response (§4.5): subject consent over the raw `query_id`
/// (classical), and the verifier's signature over the map minus field 9,
/// classical in a presence record and hybrid inside a `Recovery` block.
pub fn response<L: Lookup + ?Sized>(ids: &L, resp: &[u8], hybrid: bool) -> Result<(), Failure> {
    let __rm_item = parse_all(resp).map_err(|_| "response cbor")?;
    let Item::Map(rm) = &__rm_item else {
        return Err("response not map".into());
    };
    let get_b = |k: u64| match map_get(rm, k) { Some(Item::Bytes(r)) => Some(resp[r.clone()].to_vec()), _ => None };
    let subject = get_b(2).ok_or("subject")?;
    let verifier = get_b(1).ok_or("verifier")?;
    let qid = get_b(3).ok_or("query_id")?;
    let c_range = value_slice(resp, 7).ok_or("consent")?;
    let (cp, csig) = parts_sign1(&resp[c_range]).ok_or("consent shape")?;
    let sid = ids.identity(&subject).ok_or_else(|| Failure::MissingKey(subject.clone()))?;
    if !sid.verify_ed(&csig, &cose::sig_structure_sign1(&cp, aad::CONSENT, &qid)) {
        return Err("consent fails".into());
    }
    let payload = map_without_key(resp, 9).ok_or("payload")?;
    let v_range = value_slice(resp, 9).ok_or("field 9")?;
    let vslice = &resp[v_range];
    if hybrid {
        // the block must be by the named verifier alone
        let vid = ids.identity(&verifier).ok_or_else(|| Failure::MissingKey(verifier.clone()))?;
        verify_sign_block(vid, vslice, aad::VERIFIER, &payload).map_err(Failure::Invalid)
    } else {
        let vid = ids.identity(&verifier).ok_or_else(|| Failure::MissingKey(verifier.clone()))?;
        let (vp, vsig) = parts_sign1(vslice).ok_or("field 9 shape")?;
        if !vid.verify_ed(&vsig, &cose::sig_structure_sign1(&vp, aad::VERIFIER, &payload)) {
            return Err("verifier signature fails".into());
        }
        Ok(())
    }
}

/// A complete transaction envelope: structure, every entry under both
/// algorithms, and the embedded evidence (§4.1's Recovery block, §4.5's
/// responses).
pub fn envelope<L: Lookup + ?Sized>(ids: &L, b: &[u8]) -> Result<envelope::Envelope, Failure> {
    let env = envelope::parse(b).map_err(|e| format!("structure: {e}"))?;
    let body = &b[env.body.clone()];
    // the body's structural rules by the type the envelope names, before
    // any signature: verify means structurally valid (§3.4), and this entry
    // point refuses what the record parser refuses
    let body_item = parse_all(body).map_err(|e| format!("body: {e}"))?;
    rhtn_codec::schema::check_body_of_type(body, &body_item, env.tx_type).map_err(|e| format!("body: {e}"))?;
    let mut seen = std::collections::BTreeMap::<Vec<u8>, [bool; 2]>::new();
    for e in &env.entries {
        let id = ids.identity(&e.kid).ok_or_else(|| Failure::MissingKey(e.kid.clone()))?;
        let prot = &b[e.protected.clone()];
        let sig = &b[e.signature.clone()];
        let tbs = cose::sig_structure_sign(prot, aad::ENVELOPE, body);
        let slot = seen.entry(e.kid.clone()).or_insert([false, false]);
        match e.alg {
            cose::ALG_EDDSA => { if !id.verify_ed(sig, &tbs) { return Err("ed25519 fails".into()); } slot[0] = true; }
            cose::ALG_ML_DSA_65 => { if !id.verify_pq(sig, &tbs) { return Err("ml-dsa fails".into()); } slot[1] = true; }
            _ => return Err("unexpected alg".into()),
        }
    }
    let distinct: std::collections::BTreeSet<_> = env.signers.iter().collect();
    if seen.len() != distinct.len() || seen.values().any(|v| !v[0] || !v[1]) {
        return Err("signer/alg coverage incomplete".into());
    }
    if env.tx_type == 5
        && let Some(r5) = value_slice_at(b, env.body.start, 5) {
            for rr in array_item_ranges(b, r5.start).ok_or("responses walk")? {
                response(ids, &b[rr], false)?;
            }
        }
    if env.tx_type == 1 {
        adoption_evidence(ids, body)?;
    }
    Ok(env)
}

/// The evidence an adoption carries, checked against the adoption's own
/// fields (§4.1's consistency rules): a `Recovery` block's responses, each
/// hybrid, and its successor statement over the prior key, field 1 and
/// field 2 under the prior key; a `Transfer` block's statement over field
/// 1, the former patron it names and field 2 under that former patron.  A
/// patron runs this before its countersignature goes on the line.
pub fn adoption_evidence<L: Lookup + ?Sized>(ids: &L, body: &[u8]) -> Result<(), Failure> {
    let item = parse_all(body).map_err(|e| format!("body: {e}"))?;
    let Item::Map(bm) = &item else { return Err("body not map".into()) };
    let bs = |it: &Item| match it { Item::Bytes(r) => body[r.clone()].to_vec(), _ => Vec::new() };
    if let Some(r6) = value_slice(body, 6) {
        let prior = value_slice_at(body, r6.start, 1).map(|r| body[r.start + 2..r.end].to_vec()).ok_or("prior")?;
        let r2 = value_slice_at(body, r6.start, 2).ok_or("recovery responses")?;
        for rr in array_item_ranges(body, r2.start).ok_or("recovery walk")? {
            response(ids, &body[rr], true)?;
        }
        let newk = bs(map_get(bm, 1).ok_or("new key")?);
        let patron = bs(map_get(bm, 2).ok_or("patron")?);
        let mut stmt = Vec::new();
        emit_array_head(&mut stmt, 3);
        emit_bstr(&mut stmt, &prior);
        emit_bstr(&mut stmt, &newk);
        emit_bstr(&mut stmt, &patron);
        let r3 = value_slice_at(body, r6.start, 3).ok_or("successor")?;
        let pid = ids.identity(&prior).ok_or_else(|| Failure::MissingKey(prior.clone()))?;
        verify_sign_block(pid, &body[r3], aad::SUCCESSOR, &stmt).map_err(|e| Failure::Invalid(format!("successor proof: {e}")))?;
    }
    // field 9: the former patron's transfer statement over
    // [node, former, new patron] (§4.1), by the party field 9.1 names
    if let Some(r9) = value_slice(body, 9) {
        let former = value_slice_at(body, r9.start, 1).map(|r| body[r.start + 2..r.end].to_vec()).ok_or("former patron")?;
        let node = bs(map_get(bm, 1).ok_or("node")?);
        let patron = bs(map_get(bm, 2).ok_or("patron")?);
        let mut stmt = Vec::new();
        emit_array_head(&mut stmt, 3);
        emit_bstr(&mut stmt, &node);
        emit_bstr(&mut stmt, &former);
        emit_bstr(&mut stmt, &patron);
        let r2 = value_slice_at(body, r9.start, 2).ok_or("transfer block")?;
        let fid = ids.identity(&former).ok_or_else(|| Failure::MissingKey(former.clone()))?;
        verify_sign_block(fid, &body[r2], aad::TRANSFER, &stmt).map_err(|e| Failure::Invalid(format!("transfer statement: {e}")))?;
    }
    Ok(())
}

/// A standalone `COSE_Sign1` record under its named signer: the signature
/// slot, role tag and signer field per kind (§7).
pub fn record<L: Lookup + ?Sized>(ids: &L, kind: &str, raw: &[u8]) -> Result<bool, Failure> {
    let (slot, tag, sfield) = rhtn_codec::schema::sign1_profile(kind).ok_or("no profile")?;
    let __m_item = parse_all(raw).map_err(|_| "cbor")?;
    let Item::Map(m) = &__m_item else { return Err("not map".into()) };
    let signer = match map_get(m, sfield) { Some(Item::Bytes(r)) => raw[r.clone()].to_vec(), _ => return Err("signer field".into()) };
    let id = ids.identity(&signer).ok_or_else(|| Failure::MissingKey(signer.clone()))?;
    let Some(Item::Array(cs)) = map_get(m, slot) else { return Err("sig slot".into()) };
    if cs.len() != 4 {
        return Err("sign1 arity".into());
    }
    if !matches!(cs[2], Item::Null) {
        return Err("sign1 payload not detached".into());
    }
    let prot = match &cs[0] { Item::Bytes(r) => raw[r.clone()].to_vec(), _ => return Err("protected".into()) };
    let sig = match &cs[3] { Item::Bytes(r) => raw[r.clone()].to_vec(), _ => return Err("sig".into()) };
    // classical only (§7): the declared algorithm is -8, or the object is
    // not one this profile signs
    if named_signer_alg(&prot, &cs[1]) != Some(cose::ALG_EDDSA) {
        return Err("a standalone signature carries a header beyond alg, or an algorithm other than the profile's".into());
    }
    let payload = map_without_key(raw, slot).ok_or("payload")?;
    Ok(id.verify_ed(&sig, &cose::sig_structure_sign1(&prot, tag, &payload)))
}

const LABELS: [&str; 7] = ["capture", "location", "p0.integrity", "p0.retention", "p1.integrity", "p1.retention", "proximity"];

/// A partial presentation (§4.5.1): the embedded envelope verified wholesale,
/// then the seven slots' digests recomputed over the exact received
/// encodings and checked against the body's committed root.
pub fn presentation<L: Lookup + ?Sized>(ids: &L, pres: &[u8]) -> Result<(), String> {
    let __outer_item = parse_all(pres).map_err(|e| format!("cbor: {e}"))?;
    let Item::Array(outer) = &__outer_item else {
        return Err("presentation not array".into());
    };
    if outer.len() != 2 {
        return Err("presentation arity".into());
    }
    let ranges = array_item_ranges(pres, 0).ok_or("walk")?;
    let env_bytes = &pres[ranges[0].clone()];
    envelope(ids, env_bytes).map_err(|e| e.to_string())?;
    let Item::Array(slots) = &outer[1] else { return Err("slots not array".into()) };
    if slots.len() != 7 {
        return Err("slot count".into());
    }
    let slot_ranges = array_item_ranges(pres, ranges[1].start).ok_or("slots walk")?;
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
                let mut pre = vec![0u8];
                pre.extend_from_slice(&pres[slot_ranges[i].clone()]);
                digests.extend_from_slice(&cose::sha256(&pre));
            }
            _ => return Err("slot type".into()),
        }
    }
    let mut rootpre = vec![1u8];
    rootpre.extend_from_slice(&digests);
    let root = cose::sha256(&rootpre);
    let body_range = value_slice(env_bytes, 3).ok_or("env body")?;
    let body = &env_bytes[body_range];
    let r8 = value_slice(body, 8).ok_or("no field 8")?;
    if body[r8.start + 2..r8.end] != root {
        return Err("root mismatch".into());
    }
    Ok(())
}
