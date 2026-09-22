//! The objects a verification query is made of (`wire-format.md` §4.5,
//! §5.6, §7.3, §7.4; design §7.3, §7.4): the query and its id, the
//! subject's consent over that id, the request that carries both with the
//! selector's claim, the verifier's signed response, the capture key grant
//! and the late response.  Encodings only; what a party does with them is
//! [`crate::verifier`]'s and [`crate::subject`]'s.

use crate::{Keyhash, Txid};
use rhtn_codec::cbor::*;
use rhtn_codec::cose::{self, aad};
use rhtn_codec::encode::*;
use rhtn_codec::schema::{self, Family};
use rhtn_crypto::{Identity, SigningIdentity};

fn bytes32(b: &[u8], m: &[(Item, Item)], key: u64) -> Option<[u8; 32]> {
    match map_get(m, key) {
        Some(Item::Bytes(r)) if r.len() == 32 => b[r.clone()].try_into().ok(),
        _ => None,
    }
}

fn bytes(b: &[u8], m: &[(Item, Item)], key: u64) -> Option<Vec<u8>> {
    match map_get(m, key) {
        Some(Item::Bytes(r)) => Some(b[r.clone()].to_vec()),
        _ => None,
    }
}

fn uint(m: &[(Item, Item)], key: u64) -> Option<u64> {
    map_get(m, key).and_then(as_uint)
}

/// A `VerificationQuery` (`wire-format.md` §4.5): the subject, the querier,
/// the ceremony pre-commitment, the fuzzed profile, the template version,
/// and the one verifier it is addressed to.  Its id is the hash of the map
/// without the id (§5.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationQuery {
    pub subject: Keyhash,
    pub querier: Keyhash,
    pub ceremony_id: [u8; 32],
    pub profile: Vec<u8>,
    pub template_version: u64,
    pub verifier: Keyhash,
}

impl VerificationQuery {
    fn emit_without_id(&self, out: &mut Vec<u8>) {
        emit_map_head(out, 6);
        emit_uint(out, 1);
        emit_bstr(out, &self.subject);
        emit_uint(out, 2);
        emit_bstr(out, &self.querier);
        emit_uint(out, 3);
        emit_bstr(out, &self.ceremony_id);
        emit_uint(out, 4);
        emit_bstr(out, &self.profile);
        emit_uint(out, 5);
        emit_uint(out, self.template_version);
        emit_uint(out, 7);
        emit_bstr(out, &self.verifier);
    }

    /// `query_id`: SHA-256 of the canonical CBOR of fields 1-5 and 7 (§5.6).
    pub fn query_id(&self) -> [u8; 32] {
        let mut b = Vec::new();
        self.emit_without_id(&mut b);
        cose::sha256(&b)
    }

    /// The map with its id in field 6.
    pub fn encode(&self) -> Vec<u8> {
        let qid = self.query_id();
        let mut out = Vec::new();
        emit_map_head(&mut out, 7);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.subject);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.querier);
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.ceremony_id);
        emit_uint(&mut out, 4);
        emit_bstr(&mut out, &self.profile);
        emit_uint(&mut out, 5);
        emit_uint(&mut out, self.template_version);
        emit_uint(&mut out, 6);
        emit_bstr(&mut out, &qid);
        emit_uint(&mut out, 7);
        emit_bstr(&mut out, &self.verifier);
        out
    }

    /// Decode and check: the schema's rules, the id recomputing, field 7
    /// present.
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "VerificationQuery", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("query not a map".into());
        };
        let q = VerificationQuery {
            subject: bytes32(b, m, 1).ok_or("subject")?,
            querier: bytes32(b, m, 2).ok_or("querier")?,
            ceremony_id: bytes32(b, m, 3).ok_or("ceremony pre-commitment")?,
            profile: bytes(b, m, 4).ok_or("profile")?,
            template_version: uint(m, 5).ok_or("template version")?,
            verifier: bytes32(b, m, 7).ok_or("addressed verifier")?,
        };
        if bytes32(b, m, 6) != Some(q.query_id()) {
            return Err("query_id does not recompute".into());
        }
        Ok(q)
    }
}

/// The subject's consent (`wire-format.md` §5.6, design §7.4.2): a classical
/// `COSE_Sign1` over the raw 32 bytes of the query id, under
/// `rhtn/1:consent`, with no `kid`.
pub fn consent(subject: &SigningIdentity, query_id: &[u8; 32]) -> Vec<u8> {
    subject.sign1_ed_unnamed(aad::CONSENT, query_id)
}

/// Whether `consent` is the subject's signature over `query_id`, in the
/// profile's shape: alg -8 and nothing else protected, nothing unprotected,
/// a nil payload.
pub fn consent_verifies(subject: &Identity, consent: &[u8], query_id: &[u8; 32]) -> bool {
    let Ok(item) = parse_all(consent) else {
        return false;
    };
    let Item::Array(a) = &item else { return false };
    if a.len() != 4 || !matches!(a[2], Item::Null) || !matches!(&a[1], Item::Map(u) if u.is_empty())
    {
        return false;
    }
    let (Item::Bytes(pr), Item::Bytes(sr)) = (&a[0], &a[3]) else {
        return false;
    };
    let prot = &consent[pr.clone()];
    if prot != cose::protected_alg(cose::ALG_EDDSA).as_slice() {
        return false;
    }
    subject.verify_ed(
        &consent[sr.clone()],
        &cose::sig_structure_sign1(prot, aad::CONSENT, query_id),
    )
}

/// What a type-4 request stream carries (`wire-format.md` §5.6, §9.2):
/// the query, the subject's consent beside it, and the selector's claim of
/// the selection basis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryRequest {
    pub query: VerificationQuery,
    pub consent: Vec<u8>,
    pub selection_basis: u64,
}

/// The request type a query travels under (`wire-format.md` §9.2).
pub const REQUEST_VERIFIER_QUERY: u64 = 4;

impl QueryRequest {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 3);
        out.extend_from_slice(&self.query.encode());
        out.extend_from_slice(&self.consent);
        emit_uint(&mut out, self.selection_basis);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::VerifierQuery, b, 0).map_err(|e| e.0)?;
        let parts = array_item_ranges(b, 0).ok_or("request-4 body")?;
        let query = VerificationQuery::decode(&b[parts[0].clone()])?;
        let consent = b[parts[1].clone()].to_vec();
        let (sb, _) = Parser { b }.item(parts[2].start).map_err(|e| e.0)?;
        Ok(QueryRequest {
            query,
            consent,
            selection_basis: as_uint(&sb).ok_or("selection basis")?,
        })
    }
}

/// A response's result (`wire-format.md` §4.5 field 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Match = 0,
    NoMatch = 1,
    Inconclusive = 2,
    Unavailable = 3,
}

impl Verdict {
    pub fn from_u64(v: u64) -> Option<Self> {
        Some(match v {
            0 => Verdict::Match,
            1 => Verdict::NoMatch,
            2 => Verdict::Inconclusive,
            3 => Verdict::Unavailable,
            _ => return None,
        })
    }
}

/// A response's basis (`wire-format.md` §4.5 field 5): required for a
/// result that evaluated, absent for `unavailable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    PhotoMatch = 0,
    PersonalKnowledge = 1,
    Both = 2,
}

impl Basis {
    pub fn from_u64(v: u64) -> Option<Self> {
        Some(match v {
            0 => Basis::PhotoMatch,
            1 => Basis::PersonalKnowledge,
            2 => Basis::Both,
            _ => return None,
        })
    }
    /// Whether a template version accompanies this basis (§4.5 field 6:
    /// present for a photo basis, absent for personal knowledge).
    pub fn carries_template_version(self) -> bool {
        matches!(self, Basis::PhotoMatch | Basis::Both)
    }
}

/// A `VerifierResponse` as a verifier issues it in a presence record's
/// form (`wire-format.md` §4.5): classical, no prior key, the subject's
/// consent carried in field 7 and the selector's claim echoed in field 10.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub verifier: Keyhash,
    pub subject: Keyhash,
    pub query_id: [u8; 32],
    pub verdict: Verdict,
    pub basis: Option<Basis>,
    pub template_version: Option<u64>,
    pub consent: Vec<u8>,
    pub selection_basis: u64,
}

impl Response {
    fn emit(&self, out: &mut Vec<u8>, signature: Option<&[u8]>) {
        let n = 6
            + self.basis.is_some() as usize
            + self.template_version.is_some() as usize
            + signature.is_some() as usize;
        emit_map_head(out, n);
        emit_uint(out, 1);
        emit_bstr(out, &self.verifier);
        emit_uint(out, 2);
        emit_bstr(out, &self.subject);
        emit_uint(out, 3);
        emit_bstr(out, &self.query_id);
        emit_uint(out, 4);
        emit_uint(out, self.verdict as u64);
        if let Some(basis) = self.basis {
            emit_uint(out, 5);
            emit_uint(out, basis as u64);
        }
        if let Some(v) = self.template_version {
            emit_uint(out, 6);
            emit_uint(out, v);
        }
        emit_uint(out, 7);
        out.extend_from_slice(&self.consent);
        if let Some(sig) = signature {
            emit_uint(out, 9);
            out.extend_from_slice(sig);
        }
        emit_uint(out, 10);
        emit_uint(out, self.selection_basis);
    }

    /// The signed response: the verifier's classical signature over the map
    /// without field 9, under `rhtn/1:verifier`.  Field 5 is present
    /// exactly when the result evaluated, and field 6 exactly when the
    /// basis is a photo one.
    pub fn sign(&self, verifier: &SigningIdentity) -> Vec<u8> {
        assert_eq!(
            self.basis.is_some(),
            self.verdict != Verdict::Unavailable,
            "a basis accompanies every evaluated result and no unavailable one"
        );
        assert_eq!(
            self.template_version.is_some(),
            self.basis.is_some_and(Basis::carries_template_version),
            "a template version accompanies a photo basis and nothing else"
        );
        let mut payload = Vec::new();
        self.emit(&mut payload, None);
        let sig = verifier.sign1_ed_unnamed(aad::VERIFIER, &payload);
        let mut out = Vec::new();
        self.emit(&mut out, Some(&sig));
        out
    }

    /// Read a response's fields.  Verification is
    /// `rhtn_crypto::verify::response`'s.
    pub fn read(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_kind(b, "VerifierResponse", &item).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("response not a map".into());
        };
        let verdict =
            Verdict::from_u64(uint(m, 4).ok_or("result")?).ok_or("result out of range")?;
        let basis = match uint(m, 5) {
            Some(v) => Some(Basis::from_u64(v).ok_or("basis out of range")?),
            None => None,
        };
        if basis.is_some() == (verdict == Verdict::Unavailable) {
            return Err("basis present exactly when the result evaluated".into());
        }
        let template_version = uint(m, 6);
        if template_version.is_some() != basis.is_some_and(Basis::carries_template_version) {
            return Err("template version present exactly with a photo basis".into());
        }
        Ok(Response {
            verifier: bytes32(b, m, 1).ok_or("verifier")?,
            subject: bytes32(b, m, 2).ok_or("subject")?,
            query_id: bytes32(b, m, 3).ok_or("query_id")?,
            verdict,
            basis,
            template_version,
            consent: bytes_of_value(b, 7).ok_or("consent")?,
            selection_basis: uint(m, 10).ok_or("selection basis")?,
        })
    }
}

/// The prior key a recovery response names in field 8, if any.
pub fn prior_key_of(b: &[u8]) -> Option<Keyhash> {
    let r = value_slice(b, 8)?;
    let (it, _) = Parser { b }.item(r.start).ok()?;
    match &it {
        Item::Bytes(br) if br.len() == 32 => b[br.clone()].try_into().ok(),
        _ => None,
    }
}

fn bytes_of_value(b: &[u8], key: u64) -> Option<Vec<u8>> {
    value_slice(b, key).map(|r| b[r].to_vec())
}

/// A `KeyGrant` (`wire-format.md` §7.3): the record whose capture is
/// unsealed, the query it answers, and the capture key.  Payload only,
/// never a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyGrant {
    pub record: Txid,
    pub query_id: [u8; 32],
    pub key: [u8; 32],
}

impl KeyGrant {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.record);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.query_id);
        emit_uint(&mut out, 3);
        emit_bstr(&mut out, &self.key);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::KeyGrant, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("grant not a map".into());
        };
        Ok(KeyGrant {
            record: bytes32(b, m, 1).ok_or("record")?,
            query_id: bytes32(b, m, 2).ok_or("query_id")?,
            key: bytes32(b, m, 3).ok_or("key")?,
        })
    }
}

/// A `LateResponse` (`wire-format.md` §7.4): a response that missed the
/// ceremony, carried beside the record it supplements and never inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateResponse {
    pub record: Txid,
    pub subject: Keyhash,
    /// The signed `VerifierResponse`, as issued.
    pub response: Vec<u8>,
}

impl LateResponse {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_map_head(&mut out, 3);
        emit_uint(&mut out, 1);
        emit_bstr(&mut out, &self.record);
        emit_uint(&mut out, 2);
        emit_bstr(&mut out, &self.subject);
        emit_uint(&mut out, 3);
        out.extend_from_slice(&self.response);
        out
    }

    pub fn decode(b: &[u8]) -> Result<Self, String> {
        let item = parse_all(b).map_err(|e| e.0)?;
        schema::check_unsigned(Family::LateResponse, b, 0).map_err(|e| e.0)?;
        let Item::Map(m) = &item else {
            return Err("late response not a map".into());
        };
        Ok(LateResponse {
            record: bytes32(b, m, 1).ok_or("record")?,
            subject: bytes32(b, m, 2).ok_or("subject")?,
            response: bytes_of_value(b, 3).ok_or("response")?,
        })
    }
}
