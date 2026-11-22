//! Payload on the client's side (design §14.2, §14.2.4; `wire-format.md`
//! §7.8, §7.10; `light-client-requirements.md` §3): the split seal a direct
//! message is sealed with and opened from, the node keys a sender seals
//! to, what a recipient remembers so a replay opens once, and the kinds
//! that tell a protocol object from application payload.
//!
//! **Sealed to the person, never to a device** [author, 2026-11-19]: a
//! sender holds the recipient's encryption key from its card and fetches
//! the newest key the recipient's node holds for it, and nothing here
//! names a device of anybody's.

use crate::Keyhash;
use rhtn_archive::sealed::{Sealed, SealedBody};
use rhtn_archive::submission::Relayed;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use rhtn_crypto::seal::{self, EncryptionSecret};
use rhtn_crypto::verify::EncryptionKey;
use rhtn_crypto::xwing;
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;

/// Randomness, as the caller draws it.
pub type Fresh<'a> = &'a mut dyn FnMut(&mut [u8]);

/// Plaintext kinds (`wire-format.md` §7.10.1): a protocol object or
/// application payload.
pub const KIND_APPLICATION: u64 = 0;
/// A `KeyGrant` (`wire-format.md` §7.3).
pub const KIND_KEY_GRANT: u64 = 1;
/// A `LateResponse` (`wire-format.md` §7.4).
pub const KIND_LATE_RESPONSE: u64 = 2;
/// The direct path's candidates, exchanged with the peer alone over the
/// relayed channel (design §12.6.3, §14.1.1).
pub const KIND_CANDIDATES: u64 = 3;
/// The verifier's copy of its signed response, to the subject over the
/// association the grant established (`wire-format.md` §5.6).
pub const KIND_RESPONSE_COPY: u64 = 4;
/// An `ArchiveRequest` for the recipient's own archive, and its reply
/// (`wire-format.md` §7.9).
///
/// **Payload, not a service** (`light-client-requirements.md` §2): the
/// subject holds their archive, so a patron evaluating you fetches from
/// you and no infra node serves it on your behalf.
pub const KIND_ARCHIVE_REQUEST: u64 = 5;
/// The reply to that request.
pub const KIND_ARCHIVE_REPLY: u64 = 6;
/// A verification query to its verifier, and the signed response back
/// (`wire-format.md` §5.6): the same `[ VerificationQuery, COSE_Sign1,
/// uint ]` body request type 4 carries, on the end-to-end path between
/// querier and verifier — direct where the path is held, relayed
/// otherwise (design §12.6.3) — and the `VerifierResponse` the other way.
pub const KIND_QUERY: u64 = 7;
/// The signed `VerifierResponse` back.
pub const KIND_RESPONSE: u64 = 8;
/// The ceremony's conversation after the local exchanges
/// (`wire-format.md` §7.10.1, kinds 9 to 18, in the order a ceremony uses
/// them).  Each one a participant sends goes to its counterparty and to
/// every witness either nominated, so a witness observes the sequence it
/// attests; the consent request is the one exception, to the subject
/// alone, since the fuzzed profile it carries reaches nobody who holds no
/// capture to compare it against.
///
/// A `VerificationQuery` (§5.5) for the subject to consent to.
pub const KIND_CONSENT_REQUEST: u64 = 9;
/// The `ConsentReply` (§7.10.2) back: the query id and the consent over it.
pub const KIND_CONSENT_REPLY: u64 = 10;
/// A `FishingProposal` (§7.10.2), a bundle augmentation (design §8.1.2).
pub const KIND_FISHING_PROPOSAL: u64 = 11;
/// A `WitnessRequest` (§7.10.2) to a nominee, and its `WitnessAnswer`.
pub const KIND_WITNESS_REQUEST: u64 = 12;
/// The nominee's `WitnessAnswer` back.
pub const KIND_WITNESS_ANSWER: u64 = 13;
/// The `GatheredResponses` (§7.10.2) of the participant that did not
/// propose, to the proposer.
pub const KIND_GATHERED_RESPONSES: u64 = 14;
/// One signer's `BackPointers` (§7.10.2, §3.1), to the proposer.
pub const KIND_BACK_POINTERS: u64 = 15;
/// The `ProposedBody` (§7.10.2) every signer reviews, and each signer's
/// `SigningReply`: its two entries or its refusal.
pub const KIND_PROPOSED_BODY: u64 = 16;
/// One signer's `SigningReply` back.
pub const KIND_SIGNING_REPLY: u64 = 17;
/// The finalised `Envelope` (§3), proposer to every signer.
pub const KIND_RECORD: u64 = 18;
/// `AdoptionDirection` (`wire-format.md` §7.10.2): what only the other
/// side can state before a patron can build an adoption body — which way
/// it runs, which subnet, and the series the party being adopted can
/// enter.
pub const KIND_ADOPTION_DIRECTION: u64 = 19;
/// A `WitnessEngagement` (§7.10.2) to one of the sender's own nominees,
/// with the intent [author, 2026-10-19].
pub const KIND_WITNESS_ENGAGEMENT: u64 = 20;
/// An `ExchangeTrace` (§7.10.2): one event of the optical exchange's
/// shape, to the sender's own nominees as it happens.
pub const KIND_EXCHANGE_TRACE: u64 = 21;
/// A `WitnessSet` (§7.10.2): the sender's nominees by locator, between
/// the participants on the local interface, for the counterparty's prune.
pub const KIND_WITNESS_SET: u64 = 22;
/// A `WitnessKeep` (§7.10.2): of the counterparty's set, the kept.
pub const KIND_WITNESS_KEEP: u64 = 23;
/// A `WitnessStanddown` (§7.10.2): a nominee that did not survive the
/// prune.
pub const KIND_WITNESS_STANDDOWN: u64 = 24;
/// The sender's current locator, a peer's to hold (`wire-format.md` §4.4:
/// a peering's snapshot of an address is kept current by the peers
/// themselves; §7.10.1 kind 26).
pub const KIND_PEER_LOCATOR: u64 = 26;
/// The sender's backup blob, for a peer to hold (design §6.3; kind 27).
pub const KIND_BACKUP_DEPOSIT: u64 = 27;
/// Asks a peer for the blob it holds of the sender (kind 28).
pub const KIND_BACKUP_REQUEST: u64 = 28;
/// A peer's answer: the recipient's own blob, as held (kind 29).
pub const KIND_BACKUP_RETURN: u64 = 29;
/// The peering record itself, from a peer on every contact
/// (`wire-format.md` §7.10.3): what a peer that lost everything takes the
/// sender as a peer by, verified under its own material and the sender's.
pub const KIND_PEERING_RECORD: u64 = 30;
// kind 25 (`ProximityUpdate`) was RETIRED 2026-10-24: ranging is not used
// (design §7.6.3). The number is not reused and a decoder meeting it rejects.

/// A plaintext with its kind in front.
pub fn wrap(kind: u64, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    emit_array_head(&mut out, 2);
    emit_uint(&mut out, kind);
    emit_bstr(&mut out, bytes);
    out
}

/// The kind and the bytes back out of a wrapped plaintext.
pub fn unwrap(plaintext: &[u8]) -> Result<(u64, Vec<u8>), String> {
    let parts = array_item_ranges(plaintext, 0).ok_or("plaintext not an array")?;
    if parts.len() != 2 {
        return Err("plaintext is kind and bytes".into());
    }
    let p = Parser { b: plaintext };
    let (k, _) = p.item(parts[0].start).map_err(|e| e.0)?;
    let (v, _) = p.item(parts[1].start).map_err(|e| e.0)?;
    match (as_uint(&k), &v) {
        (Some(kind), Item::Bytes(r)) => Ok((kind, plaintext[r.clone()].to_vec())),
        _ => Err("plaintext shape".into()),
    }
}

/// Why a message could not be sealed or opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadError {
    /// No encryption key is held for the recipient: it is no contact, or
    /// one whose card carried none (`light-client-requirements.md` §3:
    /// reply only to a sender you hold as a contact).
    NoEncryptionKey,
    /// The node's key for the recipient is not held yet.
    NoNodeKey,
    /// This device holds no encryption secret of the identity's.
    NoSecret,
    /// It does not open: a wrong half, a changed byte, or not a seal.
    Unopened,
    /// It opens and its tag does not verify for the sender it names, so it
    /// is discarded and shown as nothing (`wire-format.md` §7.8.1).
    TagFailed,
    /// Opened before (`wire-format.md` §7.8.1): a replay opens once.
    Replayed,
    /// The bytes do not read as what they claim to be.
    Malformed(String),
}

impl std::fmt::Display for PayloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PayloadError::NoEncryptionKey => write!(f, "no encryption key held for the recipient"),
            PayloadError::NoNodeKey => write!(f, "no node key held for the recipient"),
            PayloadError::NoSecret => write!(f, "this device holds no encryption secret"),
            PayloadError::Unopened => write!(f, "the seal does not open"),
            PayloadError::TagFailed => write!(f, "the tag does not verify for the sender named"),
            PayloadError::Replayed => write!(f, "a message already opened"),
            PayloadError::Malformed(s) => write!(f, "malformed: {s}"),
        }
    }
}

/// **A node key as a sender holds it**: fetched from the recipient's node
/// when the sender connects to write and verified under that node, never
/// written to disk and never kept on the contact [author, 2026-11-20].
#[derive(Clone, PartialEq, Eq)]
pub struct NodeKeyHeld {
    /// The node holding its private half.
    pub node: Keyhash,
    /// Its number.
    pub n: u64,
    /// The X-Wing encapsulation key.
    pub public: Box<[u8; xwing::PUBLIC_BYTES]>,
}

impl std::fmt::Debug for NodeKeyHeld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeKeyHeld(n {})", self.n)
    }
}

/// What a sender puts in a seal besides the kind and the bytes.
pub struct Sender<'a> {
    /// The sender's identity.
    pub keyhash: &'a Keyhash,
    /// Its encryption secrets.
    pub secret: &'a EncryptionSecret,
    /// The alias shown with it, display only.
    pub alias: &'a str,
    /// Its own introduction, on a first message to this recipient.
    pub introduction: Option<&'a [u8]>,
}

/// **Seal `kind` and `bytes` to `to`** (`wire-format.md` §7.8.1): an X-Wing
/// encapsulation to the recipient's encryption key and one to its node's
/// key, the message key over both, the tag over both ciphertexts and the
/// body's fields, and the body under AES-256-GCM. The encoded `Sealed`.
pub fn seal_to(
    from: &Sender,
    to: &Keyhash,
    their: &EncryptionKey,
    node_key: &NodeKeyHeld,
    kind: u64,
    bytes: &[u8],
    fresh: Fresh,
) -> Result<Vec<u8>, PayloadError> {
    let mut eseed = Zeroizing::new([0u8; xwing::ESEED_BYTES]);
    fresh(&mut eseed[..]);
    let (ss_r, ct_r) =
        xwing::encapsulate(&their.kem, &eseed).map_err(|e| PayloadError::Malformed(e.into()))?;
    fresh(&mut eseed[..]);
    let (ss_n, ct_n) = xwing::encapsulate(&node_key.public, &eseed)
        .map_err(|e| PayloadError::Malformed(e.into()))?;
    let key = seal::message_key(&ss_r, &ss_n, to, &node_key.node, node_key.n, &ct_r, &ct_n);
    let fields = SealedBody::tagged(from.keyhash, from.alias, from.introduction, kind, bytes);
    let tag_key = from.secret.tag_key(&their.auth, from.keyhash, to);
    let tag = seal::tag(&tag_key, &ct_r, &ct_n, &fields);
    let body = SealedBody {
        sender: *from.keyhash,
        alias: from.alias.to_string(),
        introduction: from.introduction.map(<[u8]>::to_vec),
        kind,
        bytes: bytes.to_vec(),
        tag,
    };
    let mut nonce = [0u8; seal::NONCE_BYTES];
    fresh(&mut nonce);
    Ok(Sealed {
        n: node_key.n,
        ct_r: Box::new(ct_r),
        ct_n: Box::new(ct_n),
        body: seal::seal(&key, nonce, &Zeroizing::new(body.encode())),
    }
    .encode())
}

/// **Open what a node delivered** (`wire-format.md` §7.8.1): the
/// recipient's half under `secret`, the node's half as it came, the
/// message key over both, and the body. Attribution is the caller's, which
/// holds the contacts the tag is checked against.
pub fn open_relayed(
    secret: &EncryptionSecret,
    me: &Keyhash,
    node: &Keyhash,
    relayed: &Relayed,
) -> Result<(Sealed, SealedBody), PayloadError> {
    let sealed =
        Sealed::decode(&relayed.sealed).map_err(|e| PayloadError::Malformed(e.to_string()))?;
    let ss_r = secret.decapsulate(&sealed.ct_r);
    let key = seal::message_key(
        &ss_r,
        &relayed.ss_n,
        me,
        node,
        sealed.n,
        &sealed.ct_r,
        &sealed.ct_n,
    );
    let plain = seal::open(&key, &sealed.body).ok_or(PayloadError::Unopened)?;
    let body = SealedBody::decode(&plain).map_err(|e| PayloadError::Malformed(e.to_string()))?;
    Ok((sealed, body))
}

/// Whether `body`'s tag verifies for its sender under `their`, the
/// sender's encryption key.
pub fn tag_verifies(
    secret: &EncryptionSecret,
    me: &Keyhash,
    their: &EncryptionKey,
    sealed: &Sealed,
    body: &SealedBody,
) -> bool {
    let tag_key = secret.tag_key(&their.auth, &body.sender, me);
    seal::tag_verifies(
        &tag_key,
        &sealed.ct_r,
        &sealed.ct_n,
        &body.fields(),
        &body.tag,
    )
}

/// How many of a node's newest keys a recipient remembers opened messages
/// under (`wire-format.md` §7.8.1): an older one cannot be submitted again.
pub const OPENED_KEYS: usize = 2;

/// Everything the client keeps for payload: where it is attached and
/// served, what waits for a node key, what it has opened, how far it has
/// collected from each node, and the node keys fetched to seal with.
#[derive(Default)]
pub struct PayloadState {
    /// **Every node this client is attached to** — step 5 of the browser
    /// model [author, 2026-11-11]: a tab per subnet, and an attachment made
    /// before any position (a bench provision, a genesis) belongs to no
    /// subnet yet. There is no "the serving node" beside this set
    /// [restructure step 1, 2026-11-14]: what a client sweeps or asks goes
    /// to a node named by the address it serves.
    pub attachments: BTreeSet<Keyhash>,
    /// **The node serving each of this client's addresses**, by the anchor
    /// of the subnet: *"Each ID:Subnet is effectively a unique FN
    /// address"* [author, 2026-11-11]. Written where an address is gained
    /// through a node ([`crate::ceremony::Client::serve_at`]).
    pub serving_at: BTreeMap<Keyhash, Keyhash>,
    /// **The subnet the last message from each peer came by**, as its
    /// anchor [author, 2026-11-11: *"Outbound messages attempt to reach the
    /// recipient through the last subnet from which a message by that user
    /// was received"*]. Recorded at arrival from the attachment it arrived
    /// on ([`crate::ceremony::Client::record_arrival`]).
    pub last_from: BTreeMap<Keyhash, Keyhash>,
    /// **The nodes this identity agreed to serve from, with the addresses
    /// the agreement named** [review, 2026-11-13: the one store of it — the
    /// shell kept a copy of its own and read that at bring-up]. An
    /// agreement is an invitation taken, an instance enrolled, or a node
    /// launched; an attach the bench's provision drove is not one.
    pub agreed: BTreeMap<Keyhash, Vec<String>>,
    /// **A move said of a node agreed to, while it stands**
    /// ([`crate::relocation`]): one per node, the newest invitation's.
    pub relocations: BTreeMap<Keyhash, crate::relocation::Held>,
    /// **The operator's key material for a node an adoption's meeting
    /// carried from above its patron** [author, 2026-11-22], kept with the
    /// invitation it came with and with the agreement that invitation
    /// becomes: the operator is not a contact, so nothing else would teach
    /// a restarted client the key that proves the node and binds its dial.
    /// Gone with the last of the two.
    pub carried: BTreeMap<Keyhash, Vec<u8>>,
    /// **The identity's encryption secrets on this device**: derived from
    /// the root where the seed is held, handed over in a pairing where it
    /// is not (`wire-format.md` §14.3.3). Written only on a device that
    /// holds no seed, which could not derive them again.
    pub secret: Option<EncryptionSecret>,
    /// Whether [`PayloadState::secret`] came in a pairing, and so is
    /// written.
    pub secret_handed: bool,
    /// **This device's own introduction**, carried in the seal on a first
    /// message to a recipient (`wire-format.md` §7.8.1): refreshed at
    /// attach and whenever the positions move, and never written.
    pub card: Option<Vec<u8>>,
    /// **Node keys fetched to seal with**, by recipient: never written and
    /// never on the contact [author, 2026-11-20]. A refusal for a stale
    /// key drops the one held and the next send fetches the newest.
    pub node_keys: BTreeMap<Keyhash, NodeKeyHeld>,
    /// Plaintexts waiting for a node key, by recipient, each `[kind,
    /// bytes]` as [`wrap`] made it.
    pub pending: BTreeMap<Keyhash, Vec<Vec<u8>>>,
    /// **The parties a message has been attributed from**: a sender's
    /// introduction rides its messages to a recipient until it has had a
    /// reply from it (`wire-format.md` §7.10.1).
    pub heard_from: BTreeSet<Keyhash>,
    /// **What has been opened**, by the node that delivered it and the
    /// node key's number, the [`OPENED_KEYS`] newest per node: a replay
    /// opens once, across a restart too (`wire-format.md` §7.8.1).
    pub opened: BTreeMap<Keyhash, BTreeMap<u64, BTreeSet<[u8; 32]>>>,
    /// **The newest queue position collected from each node**
    /// (`wire-format.md` §7.10): what an attach there carries as field 5.
    pub cursors: BTreeMap<Keyhash, u64>,
}

impl std::fmt::Debug for PayloadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PayloadState")
            .field("attachments", &self.attachments.len())
            .field("secret", &self.secret.is_some())
            .field("pending", &self.pending.len())
            .field("cursors", &self.cursors)
            .finish()
    }
}

/// A relocation's standing as the state writes it.
fn standing_code(s: rhtn_model::Standing) -> u64 {
    use rhtn_model::Standing::*;
    match s {
        Unproved => 0,
        Proved => 1,
        Declined => 2,
        Failed => 3,
    }
}

/// The standing a written code names; none for a code no writer uses.
fn standing_of(n: u64) -> Option<rhtn_model::Standing> {
    use rhtn_model::Standing::*;
    Some(match n {
        0 => Unproved,
        1 => Proved,
        2 => Declined,
        3 => Failed,
        _ => return None,
    })
}

impl PayloadState {
    /// Whether `hash` was opened before under `node`'s key `n`, and
    /// remember it if not: the node's [`OPENED_KEYS`] newest keys are kept
    /// and older ones forgotten, since a message sealed to one of those is
    /// refused at submission (`wire-format.md` §7.8).
    pub fn first_opening(&mut self, node: &Keyhash, n: u64, hash: [u8; 32]) -> bool {
        let keys = self.opened.entry(*node).or_default();
        if keys.len() >= OPENED_KEYS && keys.keys().next().is_some_and(|oldest| n < *oldest) {
            // older than every key remembered: it cannot have been
            // submitted lately, so it is a replay
            return false;
        }
        let fresh = keys.entry(n).or_default().insert(hash);
        while keys.len() > OPENED_KEYS {
            let oldest = *keys.keys().next().expect("non-empty");
            keys.remove(&oldest);
        }
        fresh
    }

    /// Everything of this state that a restart would otherwise lose.
    /// Node keys are not written, and the secrets only where they were
    /// handed over.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 12);
        emit_uint(&mut out, 2);
        emit_array_head(&mut out, self.attachments.len());
        for n in &self.attachments {
            emit_bstr(&mut out, n);
        }
        emit_array_head(&mut out, self.serving_at.len());
        for (anchor, node) in &self.serving_at {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, anchor);
            emit_bstr(&mut out, node);
        }
        emit_array_head(&mut out, self.last_from.len());
        for (peer, anchor) in &self.last_from {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, peer);
            emit_bstr(&mut out, anchor);
        }
        emit_array_head(&mut out, self.agreed.len());
        for (node, addrs) in &self.agreed {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, node);
            emit_array_head(&mut out, addrs.len());
            for a in addrs {
                emit_tstr(&mut out, a);
            }
        }
        emit_array_head(&mut out, self.relocations.len());
        for (node, h) in &self.relocations {
            emit_array_head(&mut out, 6);
            emit_bstr(&mut out, node);
            emit_tstr(&mut out, &h.to);
            emit_uint(&mut out, standing_code(h.standing));
            match &h.why {
                Some(w) => emit_tstr(&mut out, w),
                None => emit_null(&mut out),
            }
            match &h.label {
                Some(l) => emit_tstr(&mut out, l),
                None => emit_null(&mut out),
            }
            match &h.subnet {
                Some(s) => emit_bstr(&mut out, s),
                None => emit_null(&mut out),
            }
        }
        match (&self.secret, self.secret_handed) {
            (Some(s), true) => {
                emit_array_head(&mut out, 2);
                emit_bstr(&mut out, s.kem_seed());
                emit_bstr(&mut out, &s.auth_secret()[..]);
            }
            _ => emit_null(&mut out),
        }
        emit_array_head(&mut out, self.pending.len());
        for (to, plains) in &self.pending {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, to);
            emit_array_head(&mut out, plains.len());
            for p in plains {
                emit_bstr(&mut out, p);
            }
        }
        emit_array_head(&mut out, self.heard_from.len());
        for k in &self.heard_from {
            emit_bstr(&mut out, k);
        }
        emit_array_head(&mut out, self.opened.len());
        for (node, keys) in &self.opened {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, node);
            emit_array_head(&mut out, keys.len());
            for (n, hashes) in keys {
                emit_array_head(&mut out, 2);
                emit_uint(&mut out, *n);
                emit_array_head(&mut out, hashes.len());
                for h in hashes {
                    emit_bstr(&mut out, h);
                }
            }
        }
        emit_array_head(&mut out, self.cursors.len());
        for (node, c) in &self.cursors {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, node);
            emit_uint(&mut out, *c);
        }
        emit_array_head(&mut out, self.carried.len());
        for (node, material) in &self.carried {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, node);
            emit_bstr(&mut out, material);
        }
        out
    }

    /// The state back, whole or not at all.
    pub fn decode(b: &[u8]) -> Option<PayloadState> {
        use crate::durable::*;
        let (_, f) = parse_array(b, 12)?;
        if uint(&f[0])? != 2 {
            return None;
        }
        let keyhashes = |it: &Item| -> Option<Vec<Keyhash>> {
            array(it)?.iter().map(|x| fixed::<32>(b, x)).collect()
        };
        let pairs = |it: &Item| -> Option<Vec<(Keyhash, Keyhash)>> {
            array(it)?
                .iter()
                .map(|p| {
                    let [a, c] = array(p)?.as_slice() else {
                        return None;
                    };
                    Some((fixed::<32>(b, a)?, fixed::<32>(b, c)?))
                })
                .collect()
        };
        let text = |it: &Item| match it {
            Item::Text(r) => std::str::from_utf8(&b[r.clone()]).ok().map(str::to_string),
            _ => None,
        };
        let opt_text = |it: &Item| match it {
            Item::Null => Some(None),
            it => text(it).map(Some),
        };
        let agreed: BTreeMap<Keyhash, Vec<String>> = array(&f[4])?
            .iter()
            .map(|p| {
                let [node, addrs] = array(p)?.as_slice() else {
                    return None;
                };
                let addrs: Vec<String> = array(addrs)?.iter().map(text).collect::<Option<_>>()?;
                Some((fixed::<32>(b, node)?, addrs))
            })
            .collect::<Option<_>>()?;
        let relocations: BTreeMap<Keyhash, crate::relocation::Held> = array(&f[5])?
            .iter()
            .map(|p| {
                let [node, to, standing, why, label, subnet] = array(p)?.as_slice() else {
                    return None;
                };
                let subnet = match subnet {
                    Item::Null => None,
                    it => Some(fixed::<32>(b, it)?),
                };
                Some((
                    fixed::<32>(b, node)?,
                    crate::relocation::Held {
                        to: text(to)?,
                        standing: standing_of(uint(standing)?)?,
                        why: opt_text(why)?,
                        label: opt_text(label)?,
                        subnet,
                    },
                ))
            })
            .collect::<Option<_>>()?;
        let secret = match &f[6] {
            Item::Null => None,
            it => {
                let [k, a] = array(it)?.as_slice() else {
                    return None;
                };
                Some(EncryptionSecret::from_secrets(
                    &fixed::<32>(b, k)?,
                    &fixed::<32>(b, a)?,
                ))
            }
        };
        let pending: BTreeMap<Keyhash, Vec<Vec<u8>>> = array(&f[7])?
            .iter()
            .map(|p| {
                let [to, plains] = array(p)?.as_slice() else {
                    return None;
                };
                let plains = array(plains)?
                    .iter()
                    .map(|x| bytes(b, x))
                    .collect::<Option<_>>()?;
                Some((fixed::<32>(b, to)?, plains))
            })
            .collect::<Option<_>>()?;
        let opened = array(&f[9])?
            .iter()
            .map(|p| {
                let [node, keys] = array(p)?.as_slice() else {
                    return None;
                };
                let keys = array(keys)?
                    .iter()
                    .map(|k| {
                        let [n, hashes] = array(k)?.as_slice() else {
                            return None;
                        };
                        let hashes = array(hashes)?
                            .iter()
                            .map(|h| fixed::<32>(b, h))
                            .collect::<Option<_>>()?;
                        Some((uint(n)?, hashes))
                    })
                    .collect::<Option<_>>()?;
                Some((fixed::<32>(b, node)?, keys))
            })
            .collect::<Option<_>>()?;
        let cursors = array(&f[10])?
            .iter()
            .map(|p| {
                let [node, c] = array(p)?.as_slice() else {
                    return None;
                };
                Some((fixed::<32>(b, node)?, uint(c)?))
            })
            .collect::<Option<_>>()?;
        let carried = array(&f[11])?
            .iter()
            .map(|p| {
                let [node, material] = array(p)?.as_slice() else {
                    return None;
                };
                Some((fixed::<32>(b, node)?, bytes(b, material)?))
            })
            .collect::<Option<_>>()?;
        Some(PayloadState {
            attachments: keyhashes(&f[1])?.into_iter().collect(),
            serving_at: pairs(&f[2])?.into_iter().collect(),
            last_from: pairs(&f[3])?.into_iter().collect(),
            agreed,
            relocations,
            carried,
            secret_handed: secret.is_some(),
            secret,
            card: None,
            node_keys: BTreeMap::new(),
            pending,
            heard_from: keyhashes(&f[8])?.into_iter().collect(),
            opened,
            cursors,
        })
    }

    /// **A ceremony's legs do not outlive the ceremony** [bench, 2026-10-24].
    /// A step to a party with no node key yet waits for one; a
    /// conversation leg (kinds 9 to 24) so waiting for a ceremony that has
    /// since ended is a message to nobody. Drops them; what waits for the
    /// application and the rest stays. How many went.
    pub fn drop_legs(&mut self) -> usize {
        let mut dropped = 0;
        for waiting in self.pending.values_mut() {
            let before = waiting.len();
            waiting.retain(|plain| match unwrap(plain) {
                Ok((kind, _)) => !(KIND_CONSENT_REQUEST..=KIND_WITNESS_STANDDOWN).contains(&kind),
                Err(_) => true,
            });
            dropped += before - waiting.len();
        }
        self.pending.retain(|_, v| !v.is_empty());
        dropped
    }
}
