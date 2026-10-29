//! The presence ceremony as a client runs it (design §7.1, §7.5.2,
//! §8.1.2; `light-client-requirements.md` §1): each party's decisions,
//! taken against what that party holds and sees, and the messages the
//! ceremony's direct channel carries between them.  Those messages are
//! carried by no wire object (§8.1.2's rule), so here they are values; an
//! in-process [`Harness`] moves them and records every path they take.

use crate::backup;
use crate::device::{Device, Held, Hold, Tick};
use crate::diag::id8;
use crate::horizon::{Askable, Horizon, Reach, Upstream};
use crate::keys::{capture_key, pre_commitment};
use crate::notice::{Notice, Role};
use crate::payload::{self, PayloadError, PayloadState};
use crate::query::{KeyGrant, QueryRequest, Response, Verdict, VerificationQuery};
use crate::record::{
    self, DisclosureSet, Proposal, Refusal, disclosure_root, disclosures, participant_check,
    sort_responses, witness_check,
};
use crate::rotation::Rotation;
use crate::selection::{self, Acquaintance, SelectionBasis, required};
use crate::store::{Capture, ClientStore, OwnCapture, SealParams, SealedCapture, seal};
use crate::subject::{SubjectConfig, SubjectState};
use crate::verifier::{GrantOutcome, QueryOutcome, VerifierConfig, VerifierState, Verifying};
use crate::{Keyhash, Txid};
use rhtn_archive::chain::Archive;
use rhtn_archive::prekey::{PrekeyReply, decode_batch_reply};
use rhtn_archive::record::Record;
use rhtn_archive::tx::{
    Adoption, Evidence, Locator, Seqno, TYPE_ADOPTION, TYPE_DEPARTURE, TYPE_PRESENCE, Witness,
    adoption_body, departure_body, envelope, envelope_from_entries, recovery_block,
    recovery_response_with_consent,
};
use rhtn_codec::cose::aad;
use rhtn_crypto::verify;
use rhtn_crypto::{Identity, SigningIdentity};
use std::collections::{BTreeMap, BTreeSet};
use zeroize::Zeroizing;

/// A client's own numbers for the ceremony.
#[derive(Clone)]
pub struct Config {
    /// How far a claimed `started_at` may sit from this clock, in seconds
    /// (`light-client-requirements.md` §1.2: the witness's to set).
    pub clock_tolerance_s: u64,
    /// **The window a witness will not sign inside**, in seconds, measured
    /// from its own receipt of the request (design §7.1 item 9;
    /// `light-client-requirements.md` §1.2).  Four minutes is the design's
    /// figure and the default a releasable build carries; **the field-test
    /// flavour relaxes it to thirty seconds** [author, 2026-10-21], which
    /// is what CER-026 anticipates when it says a build that relaxes this
    /// is a build whose runs say nothing about that row.
    ///
    /// **Zero turns the wait off**, the idiom [`crate::device::Continuity`]
    /// already uses for its own window: the in-process harness drives whole
    /// ceremonies against a real clock and would otherwise wait out every
    /// one of them.
    pub witness_window_s: u64,
    /// The retention this client declares, in years (design §7.5.1).
    pub retention_years: u64,
    /// The template version this client captures and compares under.
    pub template_version: u64,
    /// The guided capture's figures.
    pub capture: crate::device::CaptureParams,
    /// How what it keeps at rest is sealed.
    pub seal: SealParams,
    /// The window a capture is held across (design §7.5).  The floor is
    /// inactive by default [author, 2026-10-18].
    pub continuity: crate::device::Continuity,
    /// Its numbers as a subject.
    pub subject: SubjectConfig,
    /// Its numbers as a verifier.
    pub verifier: VerifierConfig,
    /// Its numbers for the payload path.
    pub payload: payload::PayloadConfig,
    /// The trust policy this client computes standing with: the reference
    /// metric unless something substitutes one (design §16.1).  **Nothing
    /// the client stores or sends consults it** (design §16.4), so a
    /// client running a substitute produces the same wire traffic.
    pub policy: std::sync::Arc<dyn rhtn_policy::Policy<Keyhash>>,
    /// How many contacts this client keeps before pruning toward what
    /// still resolves (`light-client-requirements.md` §4.3).
    pub contacts_limit: usize,
    /// How long a query or a grant waits for its verifier's answer
    /// before the person is asked whether to go on without it.  At half
    /// of this with nothing back, it goes again on a fresh session: a
    /// verifier that lost its sessions (a process that ended before it
    /// wrote) refuses everything sent on the old one and has no session
    /// to say so on, and only the sender can open another.
    pub query_patience_s: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            contacts_limit: 4096,
            query_patience_s: 60,
            clock_tolerance_s: 300,
            // the design's four minutes, relaxed for a bench that runs
            // ceremonies back to back [author, 2026-10-21]
            witness_window_s: if cfg!(feature = "fieldtest") { 30 } else { 240 },
            retention_years: 2,
            template_version: 1,
            capture: Default::default(),
            seal: SealParams::default(),
            continuity: crate::device::Continuity::default(),
            subject: SubjectConfig::default(),
            verifier: VerifierConfig::default(),
            payload: payload::PayloadConfig::default(),
            policy: std::sync::Arc::new(rhtn_policy::ReferenceMetric::default()),
        }
    }
}

/// Why a ceremony stopped short of a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Abort {
    /// The person declined to start.
    Declined,
    /// No ceremony is under way with this counterparty.
    NotActive,
    /// A ceremony is under way but its id is not fixed yet: the
    /// counterparty's contribution has not been taken (§14.3.1), so
    /// nothing can be anchored to it or derived from it.
    NoCeremonyId,
    /// A carriage that would not open under the local session's key
    /// (`wire-format.md` §14.3.2).  Says only that the bytes came from
    /// somebody without the key: a peer whose shell does not encrypt, a
    /// tampered byte, or another ceremony's traffic.  **Not an anchor
    /// failure** — §14.3.1's checks run on the plaintext and are what say
    /// which ceremony a carriage claimed.
    Unsealable,
    /// The counterparty's claimed start is far from this clock.
    ClockFar,
    /// A bearer-carried message echoes a contribution this device did not
    /// read off the counterparty's screen (`wire-format.md` §14.3.1): the
    /// bearer disagrees with the screen, and the ceremony stops.
    ContributionMismatch,
    /// The counterparty's ceremony-id is not the one this device computed
    /// (§14.3.1): where a man in the middle shows.
    CeremonyIdMismatch,
    /// A §14.3 object did not decode, or decoded as the wrong kind.
    Malformed(String),
    /// The engine produced a template of the wrong length.
    TemplateLength,
    /// This party could not hold its counterparty across the window
    /// (design §7.5): the meeting ends, since a party that lost the face
    /// it was holding has nothing to attest.
    LostHold(crate::device::Lost),
    /// A party refused to sign.
    Refused(Refusal),
    /// The proposed root is not the root of the agreed disclosure set.
    RootMismatch,
    /// The body does not carry this signer's own back-pointers.
    BackPointers,
    /// The record did not parse or append.
    Record(String),
    /// The recovering subject holds no prior key to rotate from.
    NoPriorKey,
    /// This device holds no seed: the act belongs to the ceremony device
    /// (design §23.3).
    NoSeed,
    /// The bundle payload names another subject, or the signed bundle is
    /// not over this device's current material.
    NotMine(&'static str),
    /// This client's archive shows no relationship with that patron, so
    /// there is nothing for it to leave.
    NoRelationship(Keyhash),
    /// The relationship's counter is at its maximum, so a departure
    /// cannot advance it within this series (`wire-format.md` §2.3).
    CounterExhausted,
    /// The verifier never met the prior key, or does not recognise the
    /// person; or the subject gathered no recognition.
    NotRecognised,
    /// The patron's check of the evidence against the adoption's own
    /// fields failed, or it holds no position to adopt under.
    PatronRefused(String),
    /// **The party being adopted refused the body it was shown**, with
    /// the reason: an adoption of somebody else, evidence that is not a
    /// meeting of these two, a position that is not the patron's to
    /// issue, a series this key has occupied, or back-pointers that are
    /// not this archive's.  Not a protocol failure and not a fault of the
    /// patron's necessarily — the reason is what the person is shown.
    NodeRefused(String),
    /// No code has been read yet, so the counterparty is not known: the
    /// step needs the first QR (`wire-format.md` §14.3.1).
    NoCounterparty,
    /// The counterparty's intent claims the same side as this device's
    /// own, both having begun or neither (`wire-format.md` §14.3.2): with
    /// two proposers or none the conversation cannot run, so the intent is
    /// refused where it is read.
    InitiatorClaim,
    /// The conversation's next step waits on something that has not
    /// arrived (`wire-format.md` §7.10.1): what, in words.
    Waiting(&'static str),
    /// Proposing is the initiator's; this side is shown the body.
    NotProposer,
    /// A message of the conversation could not be sent to the
    /// counterparty: the payload path's reason.
    Payload(String),
}

/// One archive fetch under way with a party: the nonce the next reply must
/// echo, the frontier it was asked for, the chronology owed from earlier
/// pages, and the page size.
#[derive(Debug, Clone)]
struct Fetch {
    nonce: [u8; 16],
    frontier: Vec<Txid>,
    bounds: BTreeMap<Txid, u64>,
    max: u64,
}

/// What a restore from the device's own storage found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restored {
    /// How many records came back.
    pub records: usize,
    /// How many payload sessions did.
    pub sessions: usize,
    /// What the horizon did with the snapshot it found.
    pub horizon: crate::horizon::Woke,
}

/// What one party says to another on the direct channel, or to a verifier
/// on a request stream.  `Grant` and `Query` are the only two that travel
/// to a verifier; everything else stays between the participants and
/// their witnesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// What each side commits to as the meeting opens.
    Intent(Intent),
    /// The capture key, wiped wherever a copy is dropped.
    CaptureKey(Zeroizing<[u8; 32]>),
    /// A query for the subject to consent to.
    ConsentRequest(VerificationQuery),
    /// The subject's consent to one.
    Consent {
        /// The query consented to.
        query_id: [u8; 32],
        /// The signed consent over it.
        consent: Vec<u8>,
    },
    /// A `KeyGrant`, subject to verifier.
    Grant(Vec<u8>),
    /// A type-4 request body, querier to verifier.
    Query(Vec<u8>),
    /// A signed response, verifier to querier.
    Response(Vec<u8>),
    /// The same bytes, verifier to subject.
    ResponseCopy(Vec<u8>),
    /// The responses one party gathered, handed to the proposer.
    Responses(Vec<Vec<u8>>),
    /// What a nominee is asked to witness.
    WitnessRequest(WitnessRequest),
    /// Its answer: the flags it will sign under, or a decline.
    WitnessAnswer(Option<u64>),
    /// One signer's back-pointers for the body.
    BackPointers(Vec<Txid>),
    /// The body every signer reviews, with its disclosure set.
    Proposal(Box<Proposed>),
    /// A signer's envelope entries over the body, or its refusal.
    Signed(Result<Vec<u8>, Refusal>),
    /// The finalised envelope, proposer to every signer.
    Record(Vec<u8>),
    /// A nomination to one of this side's own contacts, with the intent
    /// (`wire-format.md` §7.10.2, kind 20) [author, 2026-10-19].
    Engagement(crate::conversation::Engagement),
    /// One event of the optical exchange's shape, to this side's own
    /// nominees as it happens (kind 21).
    Trace(crate::conversation::Trace),
    /// This side's nominees by locator, for the counterparty's prune
    /// (kind 22).
    WitnessSet(Vec<Vec<u8>>),
    /// Of the counterparty's set, the kept (kind 23).
    WitnessKeep(Vec<Keyhash>),
    /// A nominee of this side's that did not survive the prune (kind 24).
    Standdown([u8; 32]),
    /// The recovering subject names the prior key whose history it claims.
    ClaimPrior(Keyhash),
    /// A recovery verifier's signed response, hybrid, to the subject.
    RecoveryResponse(Vec<u8>),
    /// The assembled `Recovery` block and the old archive's head, to the
    /// patron.
    RecoveryProposal {
        /// The assembled `Recovery` block.
        block: Vec<u8>,
        /// The old archive's head, where the subject presents one.
        presented_head: Option<Txid>,
    },
    /// The adoption body the patron proposes.
    AdoptionBody(Vec<u8>),
    /// A sealed line, to whoever holds a locator for it.
    Seal(Vec<u8>),
    /// A signed prekey bundle, to the serving node.
    PublishBundle(Vec<u8>),
    /// One-time keys, to the serving node.
    StockOneTime(Vec<Vec<u8>>),
    /// A `PrekeyRequest` or `PrekeyBatchRequest`, to the serving node.
    PrekeyRequest(Vec<u8>),
    /// A `CatalogQuery` (`wire-format.md` §6.4), to the serving node.
    CatalogQuery(Vec<u8>),
    /// A `ResourceRequest` (`wire-format.md` §11.1), to the node hosting
    /// the resource it names.
    ResourceRequest {
        /// The node asked, which hosts the resource.
        node: Keyhash,
        /// The request's bytes.
        bytes: Vec<u8>,
    },
    /// This client's own signed `Profile` (`wire-format.md` §7.11), to
    /// the serving node.
    PublishProfile(Vec<u8>),
    /// A `ProfileRequest` (`wire-format.md` §7.11), to where the
    /// subject's locator leads.
    ProfileRequest(Vec<u8>),
    /// The reply, from the serving node.
    PrekeyReply(Vec<u8>),
    /// The serving node's word that the pool ran dry.
    PoolExhausted,
    /// Bytes on the end-to-end channel, on the direct path to `to`.
    /// `device`: the recipient's device, carried so a failed direct path
    /// can fall back to the relay (`wire-format.md` §7.10 field 4).
    Payload {
        /// The recipient.
        to: Keyhash,
        /// The ciphertext.
        bytes: Vec<u8>,
        /// The recipient's device it is for.
        device: [u8; 32],
    },
    /// The same bytes handed to the serving node to relay to `to`.
    /// `device` is the recipient's device the ciphertext is for
    /// (`wire-format.md` §7.10 field 4).
    Relay {
        /// The recipient.
        to: Keyhash,
        /// The ciphertext.
        bytes: Vec<u8>,
        /// The recipient's device it is for.
        device: [u8; 32],
    },
    /// Application traffic to the serving node itself: it rides the
    /// transport session and needs no construction.
    Transport(Vec<u8>),
    /// **A conversation step for the co-present counterparty**, sealed
    /// under the local session (`wire-format.md` §14.3.2) and carried by
    /// the bearer (design §7.1).
    ///
    /// No node takes part in this and none is asked to: the counterparty
    /// is co-present by definition, so its leg of kinds 9 to 18 crosses
    /// the local interface while a witness's leg goes over the end-to-end
    /// path [author, 2026-10-05]. **This is what makes a formation
    /// ceremony work with no node reachable**, which design §6.4's escape
    /// from a false social universe depends on, by the formation path of
    /// design §13.2.
    Carriage(Vec<u8>),
}

/// One turn of the capture's hold, as [`Client::capture_tick`] reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureTurn {
    /// Whether the window has elapsed and the seal may be taken.
    pub done: bool,
    /// When the next turn is due, in milliseconds; zero once done.
    pub next_ms: u64,
    /// The frames retained so far.
    pub frames: u64,
    /// Whether a template is in hand, so the queries can go.
    pub template_ready: bool,
    /// Whether this turn's frame held a face.
    pub face: bool,
    /// How long a face has been absent in all, in milliseconds.
    pub away_ms: u64,
}

/// What the proposer shows every signer: the proposal, the disclosure set
/// its root commits to, and the back-pointers each signer supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    /// The body as every signer will sign it.
    pub proposal: Proposal,
    /// The disclosure set its root commits to.
    pub set: DisclosureSet,
    /// The back-pointers each signer supplied, in signer order.
    pub back: Vec<Vec<Txid>>,
}

impl Msg {
    /// The bytes a message carries, for a path check.
    pub fn payload(&self) -> Vec<u8> {
        match self {
            Msg::CaptureKey(k) => k.to_vec(),
            Msg::Grant(b)
            | Msg::Query(b)
            | Msg::Response(b)
            | Msg::ResponseCopy(b)
            | Msg::Record(b)
            | Msg::RecoveryResponse(b)
            | Msg::AdoptionBody(b)
            | Msg::Seal(b)
            | Msg::PublishBundle(b)
            | Msg::PublishProfile(b)
            | Msg::ProfileRequest(b)
            | Msg::PrekeyRequest(b)
            | Msg::PrekeyReply(b)
            | Msg::Transport(b) => b.clone(),
            Msg::Payload { bytes, .. }
            | Msg::Relay { bytes, .. }
            | Msg::ResourceRequest { bytes, .. } => bytes.clone(),
            Msg::StockOneTime(v) => v.concat(),
            Msg::RecoveryProposal { block, .. } => block.clone(),
            Msg::Consent { consent, .. } => consent.clone(),
            Msg::Responses(v) => v.concat(),
            Msg::Signed(Ok(e)) => e.clone(),
            _ => format!("{self:?}").into_bytes(),
        }
    }
}

/// What a participant announces (design §7.1 step 1): its contribution to
/// the pre-commitment, the witnesses it nominates from the counterparty's
/// neighbourhood, the bundle of its prior records for the counterparty's
/// selection, its clock, and its declared retention.
/// **`Debug` redacts the contribution** (the impl below): since 2026-10-05
/// the two contributions key the local session (`wire-format.md` §14.3.2),
/// so one of them in a log line is half of a key that a party who read the
/// other screen completes [reviewer, 2026-10-06].
#[derive(Clone, PartialEq, Eq)]
pub struct Intent {
    /// This party's contribution to the pre-commitment.
    pub contribution: [u8; 16],
    /// Its bundle: introductions to the prior counterparties it names,
    /// each with the most recent locator it holds for them
    /// (`wire-format.md` §2.3, §5.4), for the counterparty's selection.
    pub bundle: Vec<Vec<u8>>,
    /// Its clock, in milliseconds since the epoch.
    pub started_at: u64,
    /// The retention it declares, in years.
    pub retention_years: u64,
    /// Whether this side initiated.
    pub initiator: bool,
    /// Its own `SignedLocator`, the most recent it has signed.
    pub locator: Vec<u8>,
}

impl std::fmt::Debug for Intent {
    /// Everything but the contribution, which is key material: the field is
    /// named and its value is not, so a log line still says an intent was
    /// here [reviewer, 2026-10-06].
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Intent")
            .field("contribution", &"<redacted>")
            .field("bundle", &self.bundle.len())
            .field("started_at", &self.started_at)
            .field("retention_years", &self.retention_years)
            .field("initiator", &self.initiator)
            .finish()
    }
}

/// What a nominee is told when asked to witness (design §7.1 step 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessRequest {
    /// The ceremony being witnessed.
    pub ceremony_id: [u8; 32],
    /// The two parties, in body order.
    pub participants: [Keyhash; 2],
    /// The claimed start, which a witness checks its own clock against.
    pub started_at: u64,
    /// The engagement handle this request closes (`wire-format.md`
    /// §7.10.2 field 5) [author, 2026-10-19]: minted by the nominating
    /// side at `begin`, one per ceremony attempt, so what the witness
    /// watched under it binds to the record it may later sign.
    pub handle: [u8; 32],
}

/// What a patron is proposing, beside who and into which subnet.
///
/// Grouped because the four move together: the evidence the binding rests
/// on, the series of the relationship line it opens, the head a recovering
/// subject presented, and the key material an adoption of a party the
/// patron is introducing carries.
pub struct Adopting {
    /// The evidence the binding rests on.
    pub evidence: Evidence,
    /// The series of the relationship line it opens.
    pub series: u32,
    /// The head a recovering subject presented, where one did.
    pub presented_head: Option<Txid>,
    /// The key material of a party the patron is introducing, where the
    /// adoption carries it.
    pub key_material: Option<Vec<u8>>,
    /// The scope the node brings under the patron, where it brings one
    /// (`wire-format.md` §4.1 field 10): the node's own claim, copied.
    pub carrying: Option<Txid>,
}

/// The state of the one ceremony this client is a participant in.
pub(crate) struct Active {
    /// Who is in front of me: fixed by the first QR read, which carries
    /// their key material (`wire-format.md` §14.3.1; design §12.3's first
    /// contact), or given at `begin` where a prior code already named them.
    pub(crate) counterparty: Option<Keyhash>,
    pub(crate) initiator: bool,
    pub(crate) started_at: u64,
    /// The device's clock when `begin` ran, for the durations the
    /// diagnostic events carry.
    began_ms: u64,
    contribution: [u8; 16],
    /// The counterparty's, as this device read it **off their screen**
    /// (`wire-format.md` §14.3.1).  Everything the bearer then carries is
    /// checked against it, which is the whole of what the anchor buys.
    their_contribution: Option<[u8; 16]>,
    ceremony_id: Option<[u8; 32]>,
    /// The engagement handle (`wire-format.md` §7.10.2) this side minted
    /// at `begin`: one per ceremony attempt, carried by every message of
    /// the opening and closed by the witness request's field 5.
    pub(crate) handle: [u8; 32],
    /// The nominees this side's engagement (kind 20) has reached.  A
    /// nominee whose binding is not held yet stays pending and is
    /// engaged by the next call, availability being discovered after
    /// selecting (`light-client-requirements.md` §1.1); one still
    /// unreached when the set crosses was unavailable, and is not
    /// offered to the prune [author, 2026-10-19].
    pub(crate) engaged_to: BTreeSet<Keyhash>,
    /// Whether this kernel has tried to engage at all.  Where a caller
    /// carries the engagements itself — the terminal participant hands
    /// them over as lines — the kernel has discovered nothing about
    /// availability and the caller's nominee list stands [author,
    /// 2026-10-19].
    pub(crate) engage_attempted: bool,
    /// Whether this side's set (kind 22) has crossed to the counterparty:
    /// once, when the ceremony-id fixed.
    pub(crate) set_sent: bool,
    /// The counterparty's keep of this side's set (kind 23): this side's
    /// survivors, once the prune has answered.  `None` until it does.
    pub(crate) my_survivors: Option<BTreeSet<Keyhash>>,
    /// What this side kept of the counterparty's set (kind 23): their
    /// survivors.  `None` until their set arrived and was pruned.
    pub(crate) their_survivors: Option<Vec<Keyhash>>,
    pub(crate) my_nominees: BTreeSet<Keyhash>,
    pub(crate) their_nominees: Vec<Keyhash>,
    their_bundle: Vec<Vec<u8>>,
    their_retention: u64,
    sealed: Option<SealedCapture>,
    /// The hold the capture runs as, from [`Client::capture_begin`] until
    /// [`Client::capture_seal`] takes it (design §7.1 item 5).
    hold: Option<Hold>,
    /// Whether the hold's window has elapsed: what the seal waits on.
    hold_done: bool,
    /// The template of the counterparty, in memory for the queries and
    /// gone with the ceremony.
    template: Option<Vec<u8>>,
    image_count: u64,
    pub(crate) issued: BTreeSet<[u8; 32]>,
    pub(crate) responses: Vec<Vec<u8>>,
    /// The conversation on the end-to-end path (`crate::sequence`): what
    /// this side put out and what came back.
    pub(crate) conversation: crate::sequence::Conversation,
}

impl Drop for Active {
    /// **The ceremony's key material goes with the ceremony**
    /// [reviewer, 2026-10-06]. The two contributions key the local session
    /// (`wire-format.md` §14.3.2) and the template is a likeness; neither
    /// has a use once this ceremony is over, and §14.3.2 gives the session
    /// key exactly the ceremony's lifetime. Dropping `Active` is every way
    /// one ends: finished, stopped, abandoned, or replaced by the next.
    ///
    /// **No capture seed is here to wipe** [author, 2026-10-17]: the
    /// capture key derives from the identity root (design §7.5.2.6), so
    /// what this ceremony held was never a secret of its own.
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.contribution.zeroize();
        if let Some(theirs) = self.their_contribution.as_mut() {
            theirs.zeroize();
        }
        if let Some(t) = self.template.as_mut() {
            t.zeroize();
        }
    }
}

impl Active {
    /// The counterparty, once a code has named them.
    pub(crate) fn peer(&self) -> Result<Keyhash, Abort> {
        self.counterparty.ok_or(Abort::NoCounterparty)
    }
}

/// **What this key is in one subnet**, as a crown is drawn from
/// (`Robot/light-client-screens.md`, the crowns).
///
/// **Three facts, each off a local record** [author, 2026-10-28]. Which
/// of them a crown shows is the shell's precedence, not this type's:
/// they overlap, and only the highest is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubnetStanding {
    /// The subnet (design §13.8).
    pub subnet: Txid,
    /// Whether this key is the patron in the subnet's anchoring record,
    /// which is what makes it the root.
    pub root: bool,
    /// Whether this key published an endpoint record on the line it
    /// holds here, which is what makes it an infra node in this subnet.
    pub operator: bool,
    /// How many subordinates it still holds here.
    pub subordinates: usize,
}

/// A participant client: its identity, archive and store, its two query
/// roles, whom it recognises, and the device it runs on.
pub struct Client {
    /// Whom this client is: the identity it speaks as on every device.
    pub public: Identity,
    /// The seed, on the device that performs ceremonies and nowhere else
    /// (design §23.3).  A delegated device holds none, and every act the
    /// signing table gives the identity key is refused on it with
    /// [`Abort::NoSeed`].  Boxed: a signing identity carries its expanded
    /// post-quantum key inline, and a client is moved around a harness by
    /// value.
    signer: Option<Box<SigningIdentity>>,
    /// An operator's provider credential, opaque here
    /// (`light-client-requirements.md` §2): held for the backup and never
    /// written to the archive.
    pub provider_credential: Option<Vec<u8>>,
    /// The identities whose keys this client holds, for checking
    /// signatures.
    pub known: Vec<Identity>,
    /// Its own archive.
    pub archive: Archive,
    /// Its persisted state: sealed captures, own-capture rows, records,
    /// disclosures.
    pub store: ClientStore,
    /// A digest of the last backup each peer was handed, so a round that
    /// changed nothing deposits nothing.  This process's memory alone: a
    /// restart deposits once more, which is the cheaper error.
    deposited: BTreeMap<Keyhash, [u8; 32]>,
    /// Its state as a subject.
    pub subject: SubjectState,
    /// Its state as a verifier.
    pub verifier: VerifierState,
    /// Whom it recognises, by tier, which is what selection draws on.
    pub acquaintance: Acquaintance,
    /// Its own numbers.
    pub cfg: Config,
    /// The device it runs on.
    pub device: Device,
    /// Where this client sits, in each subnet it is in.
    ///
    /// **Absent an adoption it self-anchors**: `wire-format.md` §2.1 names
    /// the empty path the self-anchor case, and design Appendix A has a
    /// root name itself. That is a position like any other, which is what
    /// makes adopting somebody as a newly minted root ordinary rather than
    /// a case needing anything special [author, 2026-09-14].
    ///
    /// **One per subnet**, because a party adopted under two patrons is in
    /// two of them (`wire-format.md` §2.3's one series per patron
    /// relationship, §7.6's one record per line) and a subordinate's
    /// locator is its patron's path *in that subnet* with a nibble added.
    /// A single position could not say which tree an adoption was into.
    positions: BTreeMap<Txid, Locator>,
    /// The subnet under this client's own key where it is a root: its
    /// scope id there, the txid of its first adoption as a lone root
    /// (design §13.8); none while it has adopted nobody.
    root_subnet: Option<Txid>,
    /// This client's scope id in each subnet it has adopted somebody in,
    /// by subnet (design §13.8): the txid of its first adoption there.
    /// Latent while it sits under a patron, and the name its subtree goes
    /// by once it has departed.
    scopes: BTreeMap<Txid, Txid>,
    /// Whether the scope under this client's own key is carried under a
    /// patron now (`wire-format.md` §4.1 field 10): while it is, it is no
    /// subnet of its own.
    root_carried: bool,
    /// The catalog this client has browsed, and the sweep of its serving
    /// node that produced it (`wire-format.md` §6.4).
    ///
    /// **One node at a time**, because that is what a light client can
    /// ask: it holds one session, and the answer it gets is that node's
    /// portion — its own entries and those of the clients it serves
    /// (design §11.5), not a directory of anything wider.
    pub catalog: crate::catalog::View,
    browsing: Option<(Keyhash, crate::catalog::Sweep)>,
    /// Archive fetches outstanding, by subject: the nonce sent and the
    /// head asked for.  One at a time per subject, which is what lets a
    /// reply be tied to a request (design §15).
    fetching: BTreeMap<Keyhash, Fetch>,
    /// As a recovering subject: the rotation from the prior key, holding
    /// that key until the lines are sealed.
    pub rotation: Option<Box<Rotation>>,
    /// Records this client made and has not yet handed to its serving
    /// node.
    ///
    /// **A client cannot flood and does not try.** What it can do is give
    /// the one node it is attached to a transaction it signed; §10.1's
    /// forwarding rule does the rest. Kept here rather than returned from
    /// the call that made the record, because the call that made it is the
    /// ceremony's and the carrying is the adaptors', and the two happen on
    /// different sides of the boundary.
    pub(crate) outbox: Vec<Msg>,
    /// As a recovering subject: the responses gathered at recovery
    /// meetings, verified, awaiting the block.
    pub recovery_responses: Vec<Vec<u8>>,
    /// Payload confidentiality: material, sessions and what waits.
    pub payload: PayloadState,
    /// What this client keeps of its own horizon (design §15.1.1): a copy
    /// of what its serving node propagated, kept so a patron that is not
    /// answering can be routed around.
    pub horizon: Horizon,
    pub(crate) active: Option<Active>,
    /// The ceremonies this client was asked to witness and accepted, by
    /// ceremony-id, at most [`crate::sequence::PENDING_WITNESS_REQUESTS`]
    /// of them.
    pub(crate) witnessing: BTreeMap<[u8; 32], crate::sequence::Witnessing>,
    /// The engagements this client holds as a nominee (kind 20), by
    /// handle: the opening it is watching before any request arrives
    /// [author, 2026-10-19].  Bounded as `witnessing` is.
    pub(crate) engaged: BTreeMap<[u8; 32], crate::sequence::Engaged>,
}

/// What arrived on the end-to-end channel, delivered where it belongs
/// (design §14.2.4.6).
#[derive(Debug)]
pub enum Dispatched {
    /// Application payload, for whatever is above this client, with the
    /// kind it arrived under: 0 is application payload proper, and **any
    /// kind this client does not know is application payload to it**
    /// (`wire-format.md` §7.10.1), which the application can only place
    /// if it is told which [author, 2026-10-14].
    Application {
        /// The kind on the wire.
        kind: u64,
        /// The plaintext.
        bytes: Vec<u8>,
    },
    /// A `KeyGrant`, and what it came to at this verifier.
    Grant(GrantOutcome),
    /// A `LateResponse`: the record it attached to, or why it did not.
    Late(Result<Txid, String>),
    /// The peer's candidates for the direct path, for the transport to
    /// dial.
    Candidates(Vec<u8>),
    /// A verifier's copy of its response about me: the query it answered,
    /// or why the copy was refused.
    ResponseCopy(Result<[u8; 32], String>),
    /// A query put to this client as a verifier on the end-to-end path:
    /// its id, where the body decoded, and what it came to.  An answer
    /// waiting on a grant is answered when the grant lands, or expires.
    Query {
        /// The query's id, where the body decoded.
        query: Option<[u8; 32]>,
        /// What it came to.
        outcome: QueryOutcome,
    },
    /// A verifier's response to a query I issued: the query and the
    /// verdict, or why the response was refused.
    Response(Result<([u8; 32], Verdict), String>),
    /// An archive fetch this client answered from its own archive: how
    /// many records went back, and whether more remain.  The reply is in
    /// the outbox.
    Served {
        /// How many records went back.
        records: usize,
        /// Whether more remain to be fetched.
        more: bool,
    },
    /// A reply to a fetch this client made: the records it verified into
    /// its own store, or why it took none of them.
    Fetched(Result<usize, String>),
    /// A message of the ceremony's conversation (`wire-format.md` §7.10.1,
    /// kinds 9 to 18): what it came to, as a participant or a witness.
    /// What the step owes in answer is in the outbox.
    Conversation(crate::sequence::Conversed),
    /// A message between peers (design §6.3; kinds 26 to 29): what it
    /// came to, in words a screen can show.  What it owes in answer is in
    /// the outbox.
    Peer(String),
}

/// What [`Client::reconcile_from_peer`] took.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reconciled {
    /// Records appended to the archive.
    pub records: usize,
    /// Contacts inserted.
    pub contacts: usize,
    /// Everything else inserted: own-capture rows, late responses,
    /// disclosure sets, profiles, direct hints.
    pub other: usize,
    /// Records that did not verify or did not chain.
    pub refused: usize,
}

/// Insert what `theirs` has and `mine` lacks; how many.
fn insert_absent<K: Ord, V>(mine: &mut BTreeMap<K, V>, theirs: BTreeMap<K, V>) -> usize {
    let mut n = 0;
    for (k, v) in theirs {
        if let std::collections::btree_map::Entry::Vacant(e) = mine.entry(k) {
            e.insert(v);
            n += 1;
        }
    }
    n
}

/// The refusal a device holding no seed gives to an act of the identity
/// key.
const NO_SEED: &str = "this device holds no seed";

fn hex8(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// An abort on its way out of a step: the step and the variant are the
/// event, and the abort is returned as it was, so `return Err(abort(..))`
/// reads as the `return Err(..)` it replaces.  Compiled out of a
/// releasable build with every other hook (`crate::diag`).
pub(crate) fn abort(step: &'static str, e: Abort) -> Abort {
    tracing::warn!(target: "cer", step, abort = %crate::diag::abort(&e), "cer.abort");
    e
}

/// An anchored carriage refused: which message, and why
/// (`ContributionMismatch`, `CeremonyIdMismatch` or `ClockFar`).
fn anchor_refused(which: &'static str, e: Abort) -> Abort {
    tracing::warn!(target: "cer", which, why = %crate::diag::abort(&e), "cer.anchor.refused");
    e
}

impl Client {
    /// A ceremony device holding `id`: it has the seed, so every act the
    /// signing table gives the identity key is open to it.
    pub fn new(id: SigningIdentity, known: Vec<Identity>, cfg: Config, device: Device) -> Self {
        let now = device.clock.now_ms() / 1000;
        let random = device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let mut payload = PayloadState::new(cfg.payload.clone(), &mut fresh, now);
        // this device holds the seed, so it is named by the identity's
        // classical member (`wire-format.md` §7.8) until told otherwise
        payload.device = *id.public.ed.as_bytes();
        Self::with_signer(
            id.public.clone(),
            Some(Box::new(id)),
            payload,
            known,
            cfg,
            device,
        )
    }

    /// A client on a device that holds no seed: a desktop or a terminal
    /// instrument carrying a delegated transport credential (design
    /// §23.3), named by the key that credential presents.  It is a payload
    /// endpoint of its own and a holder of what the network pushes it;
    /// what the identity key signs is done on the ceremony device and
    /// refused here.
    pub fn delegated(
        public: Identity,
        presented: [u8; 32],
        known: Vec<Identity>,
        cfg: Config,
        device: Device,
    ) -> Self {
        let now = device.clock.now_ms() / 1000;
        let random = device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let mut payload = PayloadState::new(cfg.payload.clone(), &mut fresh, now);
        payload.device = presented;
        Self::with_signer(public, None, payload, known, cfg, device)
    }

    fn with_signer(
        public: Identity,
        signer: Option<Box<SigningIdentity>>,
        payload: PayloadState,
        known: Vec<Identity>,
        cfg: Config,
        device: Device,
    ) -> Self {
        let kh = public.keyhash;
        Client {
            public,
            signer,
            provider_credential: None,
            known,
            archive: Archive::new(kh),
            store: ClientStore::default(),
            payload,
            subject: SubjectState::new(cfg.subject.clone()),
            verifier: VerifierState::new(cfg.verifier.clone()),
            acquaintance: Acquaintance::default(),
            deposited: BTreeMap::new(),
            horizon: Horizon::new(kh),
            cfg,
            device,
            positions: BTreeMap::new(),
            root_subnet: None,
            scopes: BTreeMap::new(),
            root_carried: false,
            outbox: Vec::new(),
            catalog: crate::catalog::View::default(),
            browsing: None,
            fetching: BTreeMap::new(),
            rotation: None,
            recovery_responses: Vec::new(),
            active: None,
            witnessing: BTreeMap::new(),
            engaged: BTreeMap::new(),
        }
    }

    /// This client's own keyhash.
    pub fn keyhash(&self) -> Keyhash {
        self.public.keyhash
    }

    /// Whether this device holds the seed, and so performs ceremonies.
    pub fn holds_seed(&self) -> bool {
        self.signer.is_some()
    }

    /// The identity key, where this device holds it.
    pub fn signer(&self) -> Result<&SigningIdentity, Abort> {
        self.signer.as_deref().ok_or(Abort::NoSeed)
    }

    fn now_s(&self) -> u64 {
        self.device.clock.now_ms() / 1000
    }

    /// Milliseconds since `begin` by the device's clock, for the events;
    /// zero outside a ceremony.
    fn since_begin_ms(&self) -> u64 {
        self.active
            .as_ref()
            .map(|a| self.device.clock.now_ms().saturating_sub(a.began_ms))
            .unwrap_or(0)
    }

    pub(crate) fn active(&mut self) -> Result<&mut Active, Abort> {
        self.active.as_mut().ok_or(Abort::NotActive)
    }

    pub(crate) fn random<const N: usize>(&self) -> [u8; N] {
        let mut b = [0u8; N];
        self.device.random.fill(&mut b);
        b
    }

    /// Whom this client has met: the participants of its own records.
    pub fn refresh_acquaintance(&mut self) {
        let me = self.keyhash();
        for r in self.store.records.values() {
            if let Ok(rec) = Record::parse(r) {
                for p in rec.participants() {
                    if p != me {
                        self.acquaintance.met.insert(p);
                    }
                }
            }
        }
    }

    /// Step 1: open a ceremony, nominating `nominees` from the
    /// counterparty's neighbourhood.  The counterparty may be named where
    /// a prior code already did, or left for the first QR read to fix
    /// (`wire-format.md` §14.3.1): the one who shows the first invite
    /// knows nobody yet.  The one question a ceremony asks the person is
    /// whether to start it.
    pub fn begin(
        &mut self,
        counterparty: Option<Keyhash>,
        nominees: Vec<Keyhash>,
        initiator: bool,
    ) -> Result<Intent, Abort> {
        let question = match &counterparty {
            Some(k) => format!("Start a presence ceremony with {}?", hex8(k)),
            None => "Start a presence ceremony with the person in front of you?".to_string(),
        };
        if !self.device.operator.ask(&question) {
            return Err(abort("begin", Abort::Declined));
        }
        // **Witnesses are required where available** [ruled, author,
        // 2026-10-10]: a caller that names nobody is handed the parties
        // this client has met, which is the candidate pool §8.1 already
        // defines, rather than a ceremony with no witness to ask. On the
        // bench two phones that had each met somebody began with no
        // nominee at all, and the body then waited on a witness that was
        // never asked. The counterparty, where known, is never a nominee
        // of its own meeting; where it is not yet known it is dropped at
        // the pin ([`Client::take_optical`]).
        let nominees = if nominees.is_empty() {
            self.met(counterparty.as_ref())
        } else {
            nominees
        };
        let contribution = self.random::<16>();
        let handle = self.random::<32>();
        let started_at = self.now_s();
        let began_ms = self.device.clock.now_ms();
        tracing::info!(
            target: "cer",
            counterparty = %counterparty.as_ref().map(|k| id8(k).to_string()).unwrap_or_else(|| "-".into()),
            initiator,
            started_at,
            nominees = nominees.len(),
            bundle = self.store.contacts.values().filter(|c| c.locator.is_some()).count(),
            retention = self.cfg.retention_years,
            "cer.begin"
        );
        self.active = Some(Active {
            counterparty,
            initiator,
            started_at,
            began_ms,
            contribution,
            their_contribution: None,
            ceremony_id: None,
            handle,
            engaged_to: BTreeSet::new(),
            engage_attempted: false,
            set_sent: false,
            my_survivors: None,
            their_survivors: None,
            my_nominees: nominees.iter().copied().collect(),
            their_nominees: vec![],
            their_bundle: vec![],
            their_retention: 0,
            sealed: None,
            hold: None,
            hold_done: false,
            template: None,
            image_count: 0,
            issued: BTreeSet::new(),
            responses: vec![],
            conversation: Default::default(),
        });
        self.intent()
    }

    /// This device's intent over the active ceremony: the same fields
    /// [`Client::begin`] returns, built from the ceremony rather than from
    /// the call, so the carriage and the call cannot disagree about what
    /// this device said.
    ///
    /// **The bundle is every party this client holds a locator for**, each
    /// introduced by its key material and the most recent locator held
    /// (`wire-format.md` §2.3, §5.4) — curating it is the subject's act
    /// through the interface above (`light-client-requirements.md` §1.4),
    /// and truncation is visible as a smaller *n*.  A party whose material
    /// this client no longer holds cannot be introduced and is left out.
    pub fn intent(&self) -> Result<Intent, Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        Ok(Intent {
            contribution: a.contribution,
            bundle: self.introductions(),
            started_at: a.started_at,
            retention_years: self.cfg.retention_years,
            initiator: a.initiator,
            locator: self.own_locator()?,
        })
    }

    /// **Take an introduction** (`wire-format.md` §2.3): a party's key
    /// material and a `SignedLocator` of theirs, as a holder hands one on
    /// and as the first of the four things a person must be able to do
    /// from the installed app (`light-client-requirements.md` §1.4's
    /// bundle, and the intake the client review of 2026-10-11 found had
    /// no call at all).
    ///
    /// **This is Case 0 of the design's resolution sequence** (§12.3):
    /// the locator travels with the key, out of band — an optical code at
    /// a meeting, a referral from a mutual contact — and no lookup occurs.
    /// What it leaves behind is an identity this client verifies by and a
    /// contact holding the most recent locator for it, which is the local
    /// table §12.3 Case 1 looks an anchor up in and which
    /// [`Client::reach_for`] reads.
    ///
    /// The introduction is verified before anything is kept: the codec
    /// binds the material to the locator's subject and the signature is
    /// checked under that material alone. Whose it is, or why not.
    pub fn take_introduction(&mut self, bytes: &[u8]) -> Result<Keyhash, String> {
        let intro = rhtn_archive::locator::Introduction::parse(bytes)?;
        let who = intro.identity.keyhash;
        if who == self.keyhash() {
            return Err("an introduction to myself".into());
        }
        self.contact(&intro.identity, crate::contacts::How::Introduced, true);
        // the §2.3 rule decides whether this replaces a locator already
        // held: the same series with a greater counter, never outright
        self.hold_locator(&who, &intro.locator.bytes, false)
            .map_err(|e| format!("{e:?}"))?;
        tracing::info!(
            target: "cer",
            who = %id8(&who),
            anchor = %id8(&intro.locator.locator.anchor),
            "cer.introduction.taken"
        );
        Ok(who)
    }

    /// **Take an anchored introduction** (`wire-format.md` §2.4): an
    /// introduction together with the key material and published records
    /// of the anchor its locator names.
    ///
    /// **One code carries both** [author, 2026-10-13]. An introduction on
    /// its own leaves a holder with a position and nobody to dial — the
    /// anchor is named by keyhash, its material is elsewhere, and its
    /// address is in neither — so a code that carried only the
    /// introduction could not be acted on by a client with no node of its
    /// own, which is the case the intake exists for.
    ///
    /// The anchor is taken first, because the introduction is only useful
    /// once there is somewhere to start the walk from.
    pub fn take_anchored_introduction(&mut self, bytes: &[u8]) -> Result<Keyhash, String> {
        let card = rhtn_archive::locator::AnchoredIntroduction::parse(bytes)?;
        // the anchor's identity, so its records check and a dial to it
        // binds (`wire-format.md` §9.1)
        if !self.known.iter().any(|i| i.keyhash == card.anchor.keyhash) {
            self.known.push(card.anchor.clone());
        }
        self.take_endpoint(&card.endpoint.bytes)?;
        // the entry says what subtree the anchor claims, which a caching
        // policy reads (design §12.5); nothing in this client reads one
        // yet, so it is kept beside the address it came with
        if let (Some(entry), Some(c)) = (
            &card.entry,
            self.store.contacts.get_mut(&card.anchor.keyhash),
        ) {
            c.anchor_entry = Some(entry.clone());
        }
        let introduction = rhtn_archive::locator::introduction(
            &card.introduction.material,
            &card.introduction.locator.bytes,
        );
        let who = self.take_introduction(&introduction)?;
        tracing::info!(
            target: "cer",
            who = %id8(&who),
            anchor = %id8(&card.anchor.keyhash),
            "cer.card.taken"
        );
        Ok(who)
    }

    /// **This client's own code to hand over** (`wire-format.md` §2.4): its
    /// introduction, and the anchor of its own locator with the records it
    /// holds of where that anchor answers.
    ///
    /// Refused where this client cannot say where it sits, or holds no
    /// endpoint record for its own anchor — a card that could not be acted
    /// on is not offered.
    pub fn own_card(&self) -> Result<Vec<u8>, Abort> {
        let locator = self.own_locator()?;
        let anchor = self.acting_position().anchor;
        let material = self
            .known
            .iter()
            .find(|i| i.keyhash == anchor)
            .map(|i| i.key_material())
            .ok_or(Abort::NoRelationship(anchor))?;
        let record = self
            .store
            .contacts
            .get(&anchor)
            .and_then(|c| c.endpoint_record.clone())
            .ok_or(Abort::NoRelationship(anchor))?;
        let intro = rhtn_archive::locator::introduction(&self.public.key_material(), &locator);
        Ok(rhtn_archive::locator::anchored_introduction(
            &intro, &material, &record, None,
        ))
    }

    /// **Note a node the resolution walk went through** (§7.7's descent;
    /// `light-client-requirements.md` §4.3 already asks for "the
    /// infrastructure you went through to reach any of them").
    ///
    /// **Why the walk's hops are worth keeping** [author, 2026-10-14]:
    /// they make the pruning effective later. §4.3 drops a contact that
    /// stays discoverable from an upstream it holds, and the nodes between
    /// an anchor and a serving node are exactly those upstreams — a store
    /// that never saw them can only prune by age.
    ///
    /// **Material that hashes to the keyhash is that node's**, the same
    /// binding a dial rests on (`wire-format.md` §9.1), so a hop whose
    /// referral carried its material is filed; one that did not is
    /// counted and dropped, a contact being filed under the material it
    /// holds. §7.7.3 makes that field optional on purpose.
    ///
    /// The addresses are the referrer's assertion, and are kept as that.
    pub fn note_hop(&mut self, node: Keyhash, material: Option<Vec<u8>>, endpoints: Vec<Vec<u8>>) {
        let named = material
            .as_deref()
            .and_then(rhtn_crypto::Identity::from_key_material)
            .filter(|i| i.keyhash == node);
        match named {
            Some(identity) => self.contact(&identity, crate::contacts::How::Infra, true),
            // **kept by keyhash alone** [author, 2026-10-14]. §7.7.3
            // leaves the material optional and the session does not
            // supply it: a dial presents the classical key, where
            // `KeyMaterial` is the classical and post-quantum pair the
            // keyhash is taken over (§2.2). So this is a name and a place
            // to try — never verified by, never introduced, and dialled
            // unbound as §7.7.3 allows a referral's hop to be.
            None => {
                let now = self.now_s();
                self.store
                    .contacts
                    .entry(node)
                    .and_modify(|c| c.touch(now))
                    .or_insert_with(|| {
                        crate::contacts::Contact::named_only(crate::contacts::How::Infra, now)
                    });
                tracing::debug!(target: "cer", node = %id8(&node), "cer.hop.unnamed");
            }
        }
        if let Some(c) = self.store.contacts.get_mut(&node) {
            for e in endpoints {
                if !c.heard.contains(&e) {
                    c.heard.push(e);
                }
            }
            c.heard.truncate(crate::contacts::HEARD_PER_CONTACT);
        }
        tracing::debug!(target: "cer", node = %id8(&node), "cer.hop.noted");
    }

    /// **Take a node's endpoint record** (`wire-format.md` §7.6): where a
    /// node answers, signed by that node.
    ///
    /// **A participant holds these for the same reason a node does**
    /// (`light-client-requirements.md` §4.2): routing around a node that
    /// is not answering is a thing a client must do without asking
    /// anyone. It is also what makes an introduction actionable with no
    /// node of one's own — a locator names an anchor and a path, never an
    /// address, so §12.3 Case 1's "address to start from" has to arrive
    /// some other way, and this is the object the design already has for
    /// it. Nothing here was reachable from the client until now: the
    /// horizon could ingest one and no call did [2026-10-13].
    pub fn take_endpoint(&mut self, bytes: &[u8]) -> Result<Keyhash, String> {
        let er = rhtn_archive::endpoint::EndpointRecord::parse(bytes)?;
        let node = er.node;
        // **signed by the node it names, or it is nothing**: the material
        // has to be held to check that, so a node whose key this client
        // does not hold cannot be taken on trust (`wire-format.md` §9.1
        // binds a dial by the same material).
        match er.signature_checks(&self.known) {
            Some(true) => {}
            Some(false) => return Err("the record is not that node's".into()),
            None => return Err("no key material held for that node".into()),
        }
        // the horizon takes it where the node is in this client's own
        // neighbourhood, and refuses anything further off: that bound is
        // the horizon's purpose, not a defect
        let took = self.horizon.ingest_endpoint(bytes, &self.known);
        // **and the contact holds it either way**, which is the local
        // table of §12.3 Case 1 — the one an anchor outside the
        // neighbourhood is looked up in
        let held = self
            .store
            .contacts
            .get(&node)
            .and_then(|c| c.endpoint_record.as_deref())
            .and_then(|b| rhtn_archive::endpoint::EndpointRecord::parse(b).ok())
            .map(|r| r.seqno);
        // §10.1.2's ordering: the same series with a greater counter
        // supersedes, and series do not rank against each other
        let fresh = match held {
            None => true,
            Some(s) => s.series == er.seqno.series && er.seqno.counter > s.counter,
        };
        if fresh {
            let identity = self
                .known
                .iter()
                .find(|i| i.keyhash == node)
                .cloned()
                .ok_or("no identity held for that node")?;
            self.contact(&identity, crate::contacts::How::Infra, true);
            if let Some(c) = self.store.contacts.get_mut(&node) {
                c.endpoint_record = Some(bytes.to_vec());
            }
        }
        tracing::info!(
            target: "cer",
            node = %id8(&node),
            took = ?took,
            points = er.endpoints.len(),
            kept = fresh,
            "cer.endpoint.taken"
        );
        Ok(node)
    }

    /// The introductions this client can make: **its prior counterparties**
    /// (`wire-format.md` §5.4) — the parties it has met, as its records
    /// say — for whom it holds a locator and the material; the counterparty
    /// of the meeting at hand included, which counts toward the subject's
    /// claim and is never selected (§5.3). A contact learned any other way
    /// is not introduced: a party this client never met holds no capture
    /// of it, cannot verify a query about it, and offered as a verifier
    /// could only close the query unanswered (MRK, 2026-10-11).
    #[must_use]
    pub fn introductions(&self) -> Vec<Vec<u8>> {
        self.store
            .contacts
            .iter()
            .filter(|(who, _)| self.acquaintance.met.contains(*who))
            .filter_map(|(_, c)| {
                if !c.named() {
                    return None;
                }
                let sl = c.locator.as_deref()?;
                Some(rhtn_archive::locator::introduction(&c.material, sl))
            })
            .collect()
    }

    /// **A contact learned, or dealt with again** (`light-client-requirements.md`
    /// §4.3): filed under its material's hash with how it came to be held,
    /// its identity added to what this client verifies by, and the store
    /// pruned toward what still resolves where it has grown past its bound.
    pub fn contact(&mut self, identity: &Identity, how: crate::contacts::How, touch: bool) {
        let now = self.now_s();
        let who = identity.keyhash;
        if !self.known.iter().any(|k| k.keyhash == who) {
            self.known.push(identity.clone());
        }
        match self.store.contacts.get_mut(&who) {
            Some(c) => {
                if touch {
                    c.touch(now);
                }
            }
            None => {
                self.store.contacts.insert(
                    who,
                    crate::contacts::Contact::new(identity.key_material(), how, now),
                );
            }
        }
        self.prune_contacts();
    }

    /// One more dealing with `who`, where they are a contact.
    pub fn touch_contact(&mut self, who: &Keyhash) {
        let now = self.now_s();
        if let Some(c) = self.store.contacts.get_mut(who) {
            c.touch(now);
        }
    }

    /// A party dealt with as `how` — a nominee or verifier that answered —
    /// touched where a contact, made one where its material is held and it
    /// is not yet: a party answered is a party captured (§4.3).
    pub fn note_contact(&mut self, who: &Keyhash, how: crate::contacts::How) {
        if self.store.contacts.contains_key(who) {
            self.touch_contact(who);
        } else if let Some(id) = self.known.iter().find(|k| k.keyhash == *who).cloned() {
            self.contact(&id, how, true);
        }
    }

    /// The store pruned to its bound, saying how many went.
    pub fn prune_contacts(&mut self) -> usize {
        let keep = self.cfg.contacts_limit;
        let gone = crate::contacts::prune(&mut self.store.contacts, keep);
        if gone > 0 {
            tracing::info!(target: "cer", gone, keep, "cer.contacts.pruned");
        }
        gone
    }

    /// Every contact's identity among what this client verifies by: what
    /// a ceremony taught is what a restarted client still knows.
    fn sync_known_from_contacts(&mut self) {
        let have: std::collections::BTreeSet<Keyhash> =
            self.known.iter().map(|k| k.keyhash).collect();
        let learned: Vec<Identity> = self
            .store
            .contacts
            .iter()
            .filter(|(k, _)| !have.contains(*k))
            .filter_map(|(_, c)| Identity::from_key_material(&c.material))
            .collect();
        self.known.extend(learned);
    }

    /// This client's own `SignedLocator` for the subnet it acts in: its
    /// position there, or the self-anchor while it has no patron
    /// (`wire-format.md` §2.1, §2.3).
    pub fn own_locator(&self) -> Result<Vec<u8>, Abort> {
        let position = self.acting_position();
        Ok(rhtn_archive::locator::signed_locator(
            self.signer()?,
            &position,
        ))
    }

    /// **This client's own introduction** (`wire-format.md` §2.3): its key
    /// material and its own `SignedLocator`, as a party hands one on so
    /// that a holder can address it and offer it by locator — which is how
    /// a nominee travels to the counterparty's prune ([`Client::my_witness_set`]
    /// in `sequence.rs`).
    pub fn own_introduction(&self) -> Result<Vec<u8>, Abort> {
        let me = self.signer()?;
        let sl = rhtn_archive::locator::signed_locator(me, &self.acting_position());
        Ok(rhtn_archive::locator::introduction(
            &me.public.key_material(),
            &sl,
        ))
    }

    /// Keep `sl` as the locator held for `who` where it is theirs and
    /// newer than what is held (`wire-format.md` §2.3): verified under the
    /// key already pinned for them, replaced by a greater counter in the
    /// same series.  `outright` is the subject's own statement in its
    /// intent, which replaces whatever is held, series aside.
    fn hold_locator(&mut self, who: &Keyhash, sl: &[u8], outright: bool) -> Result<(), Abort> {
        let read = rhtn_archive::locator::SignedLocator::parse(sl).map_err(Abort::Malformed)?;
        if read.subject != *who {
            return Err(Abort::Malformed("a locator for another party".into()));
        }
        read.verify(&self.known).map_err(Abort::Malformed)?;
        let held = self
            .store
            .contacts
            .get(who)
            .and_then(|c| c.locator.as_deref());
        let newer = match held {
            None => true,
            Some(have) => {
                outright
                    || rhtn_archive::locator::SignedLocator::parse(have).map_or(true, |h| {
                        let (a, b) = (&read.locator.seqno, &h.locator.seqno);
                        a.series == b.series && a.counter > b.counter
                    })
            }
        };
        if newer {
            // a locator is held for a contact: one whose material this
            // client has, which `verify` above has just established
            if let Some(c) = self.store.contacts.get_mut(who) {
                c.locator = Some(sl.to_vec());
            } else if let Some(id) = self.known.iter().find(|k| k.keyhash == *who).cloned() {
                // the party's own statement of where it sits is the party
                // met; a third party's is an introduction
                let how = if outright {
                    crate::contacts::How::Counterparty
                } else {
                    crate::contacts::How::Introduced
                };
                let mut c = crate::contacts::Contact::new(id.key_material(), how, self.now_s());
                c.locator = Some(sl.to_vec());
                self.store.contacts.insert(*who, c);
                self.prune_contacts();
            }
        }
        Ok(())
    }

    /// **Step 1 of the optical exchange** (`wire-format.md` §14.3.1): the
    /// first QR this device shows, its identity and its contribution.
    ///
    /// Public, both of them, and the section says so: what the optical
    /// channel buys is not secrecy but that each party read the value off a
    /// screen it was looking at, which costs being there.
    pub fn optical_contribution(&self) -> Result<Vec<u8>, Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        let bytes = crate::local::OpticalContribution {
            material: self.public.key_material(),
            device: self.keyhash(),
            contribution: a.contribution,
        }
        .encode();
        tracing::info!(target: "cer", which = "contribution", bytes = bytes.len(), "cer.optical.shown");
        Ok(bytes)
    }

    /// The counterparty's first QR, read off their screen.  Their
    /// contribution is held from here on, and every bearer-carried message
    /// is checked against it.
    pub fn take_optical(&mut self, bytes: &[u8]) -> Result<Keyhash, Abort> {
        let read = crate::local::OpticalContribution::decode(bytes).map_err(Abort::Malformed)?;
        let identity = rhtn_crypto::Identity::from_key_material(&read.material)
            .ok_or_else(|| Abort::Malformed("the QR's key material does not read".into()))?;
        let a = self.active()?;
        // the first code fixes who is in front of me; a later one naming
        // anyone else is a party who is not the one in front of me
        match a.counterparty {
            None => {
                a.counterparty = Some(read.device);
                // a party cannot witness its own meeting
                a.my_nominees.remove(&read.device);
            }
            Some(k) if k == read.device => {}
            Some(_) => return Err(abort("take_optical", Abort::NotActive)),
        }
        a.their_contribution = Some(read.contribution);
        // first contact pins the full key (design §12.3); met before, the
        // same material is already held and nothing changes
        let pinned = !self.known.iter().any(|k| k.keyhash == read.device);
        self.contact(&identity, crate::contacts::How::Counterparty, true);
        tracing::info!(
            target: "cer",
            which = "contribution",
            bytes = bytes.len(),
            from = %id8(&read.device),
            pinned,
            "cer.optical.read"
        );
        Ok(read.device)
    }

    /// **Step 2**: the ceremony-id this device computed, as the second QR
    /// (§14.3.1, design §7.5.2).  Both contributions must be in.
    pub fn transcript_confirm(&mut self) -> Result<Vec<u8>, Abort> {
        let cid = self.my_transcript()?;
        let bytes = crate::local::TranscriptConfirm { ceremony_id: cid }.encode();
        tracing::info!(target: "cer", which = "confirm", bytes = bytes.len(), "cer.optical.shown");
        Ok(bytes)
    }

    /// The ceremony-id this device computes from the two contributions,
    /// once both are in.
    fn my_transcript(&mut self) -> Result<[u8; 32], Abort> {
        let me = self.keyhash();
        let a = self.active()?;
        let theirs = a.their_contribution.ok_or(Abort::NotActive)?;
        Ok(pre_commitment(
            (&me, &a.contribution),
            (&a.peer()?, &theirs),
        ))
    }

    /// The counterparty's second QR, checked against this device's own:
    /// *"a mismatch is where a man in the middle shows, and the ceremony
    /// stops"* (§14.3.1).  Agreeing fixes the ceremony-id and opens the
    /// subject's window.
    pub fn take_transcript(&mut self, bytes: &[u8]) -> Result<[u8; 32], Abort> {
        let read = crate::local::TranscriptConfirm::decode(bytes).map_err(Abort::Malformed)?;
        let mine = self.my_transcript()?;
        if read.ceremony_id != mine {
            return Err(anchor_refused("confirm", Abort::CeremonyIdMismatch));
        }
        tracing::info!(target: "cer", which = "confirm", bytes = bytes.len(), "cer.optical.read");
        self.active()?.ceremony_id = Some(mine);
        tracing::info!(
            target: "cer",
            ceremony = %id8(&mine),
            took_ms = self.since_begin_ms(),
            "cer.id_fixed"
        );
        self.subject.open_window(mine);
        // the opening moves [author, 2026-10-19]: the confirmation to each
        // nominee against its engagement, and this side's set to the
        // counterparty for its prune
        self.opening_id_fixed(mine);
        Ok(mine)
    }

    /// **What the bearer carries first** (§14.3.2): this device's intent as
    /// its carriage set — the `IntentExchange`, then one
    /// `BundleContinuation` per 256 entries beyond the first (§5.4).
    ///
    /// The shell moves these as opaque strings on whatever bearer the two
    /// devices have and reads none of them.
    pub fn intent_carriage(&mut self) -> Result<Vec<Vec<u8>>, Abort> {
        let intent = self.intent()?;
        let cid = self.anchored_id()?;
        let entries = intent.bundle.len();
        let out = crate::local::intent_carriage(
            &crate::local::IntentExchange {
                contribution: intent.contribution,
                bundle: intent.bundle,
                started_at: intent.started_at,
                retention_years: intent.retention_years,
                initiator: intent.initiator,
                continuations: 0,
                locator: intent.locator,
            },
            &cid,
        );
        tracing::info!(
            target: "cer",
            entries,
            continuations = out.len().saturating_sub(1),
            retention = intent.retention_years,
            initiator = intent.initiator,
            "cer.intent.sent"
        );
        out.iter().map(|p| self.seal(p)).collect()
    }

    /// The counterparty's carriage set.  The echoed contribution is checked
    /// against the one read optically — *"a bearer that does not agree with
    /// the screen, and the ceremony does not continue over it"* (§14.3.2) —
    /// and the bundle is what the accepted continuations carried.
    ///
    /// **The check is not authentication** (§14.3.1): the contribution is
    /// public, so this catches a bearer contradicting the screen and never
    /// one quoting it.
    pub fn take_intent_carriage(
        &mut self,
        from: Keyhash,
        carriage: &[Vec<u8>],
    ) -> Result<usize, Abort> {
        let cid = self.anchored_id()?;
        let theirs = self
            .active
            .as_ref()
            .and_then(|a| a.their_contribution)
            .ok_or(Abort::NotActive)?;
        let opened: Vec<Zeroizing<Vec<u8>>> = carriage
            .iter()
            .map(|c| self.open(c))
            .collect::<Result<_, _>>()?;
        let plain: Vec<Vec<u8>> = opened.iter().map(|p| p.to_vec()).collect();
        let (read, taken) = crate::local::read_intent(&plain, &cid).map_err(Abort::Malformed)?;
        if read.contribution != theirs {
            return Err(anchor_refused("intent", Abort::ContributionMismatch));
        }
        self.take_intent(
            from,
            &Intent {
                contribution: read.contribution,
                bundle: read.bundle,
                started_at: read.started_at,
                retention_years: read.retention_years,
                initiator: read.initiator,
                locator: read.locator,
            },
        )?;
        Ok(taken)
    }

    /// Candidates for the direct path, on the ceremony's own channels
    /// (§14.3.2, design §12.6.3): the bare array the payload path carries,
    /// with the anchor this carriage adds.
    pub fn candidate_carriage(&self, candidates: Vec<u8>) -> Result<Vec<u8>, Abort> {
        let cid = self.anchored_id()?;
        self.seal(
            &crate::local::CandidateHandover {
                ceremony_id: cid,
                candidates,
            }
            .encode(),
        )
    }

    /// The counterparty's candidates, anchored or refused.  What comes back
    /// is the bare array for the transport to dial: an address, not
    /// evidence of anything.
    pub fn take_candidate_carriage(&mut self, bytes: &[u8]) -> Result<Vec<u8>, Abort> {
        let plain = self.open(bytes)?;
        let read = crate::local::CandidateHandover::decode(&plain).map_err(Abort::Malformed)?;
        self.anchored("candidates", read.ceremony_id)?;
        // kept as a payload-path offer is (`ClientStore::direct_hints`):
        // the counterparty is a contact from here on, and this is where
        // it answers
        if let Some(them) = self.counterparty() {
            self.store
                .direct_hints
                .insert(them, read.candidates.clone());
        }
        Ok(read.candidates)
    }

    /// The key the counterparty seals its captures of me under, as the
    /// anchored message the bearer carries at capture time (§14.3.2,
    /// design §7.5.2.6): [`Client::capture_key`] under this ceremony's id.
    /// The bytes are wiped when dropped, as the key in them is.
    pub fn capture_key_carriage(&self) -> Result<Zeroizing<Vec<u8>>, Abort> {
        let cid = self.anchored_id()?;
        let out = crate::local::CaptureKeyHandover {
            ceremony_id: cid,
            key: self.capture_key()?,
        }
        .encode();
        let sealed = Zeroizing::new(self.seal(&out)?);
        tracing::info!(target: "cer", direction = "sent", "cer.capture_key");
        Ok(sealed)
    }

    /// The counterparty's capture key, anchored to this ceremony or
    /// refused.  What comes back is the key [`Client::capture`] seals my
    /// captures of them beneath, wiped when dropped, and nothing is held
    /// once that is done.
    pub fn take_capture_key_carriage(
        &mut self,
        bytes: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, Abort> {
        let plain = self.open(bytes)?;
        let read = crate::local::CaptureKeyHandover::decode(&plain).map_err(Abort::Malformed)?;
        self.anchored("capture_key", read.ceremony_id)?;
        tracing::info!(target: "cer", direction = "received", "cer.capture_key");
        Ok(read.key)
    }

    /// An anchored message's ceremony-id against this device's own;
    /// `which` names the message for the event a refusal raises.
    fn anchored(&mut self, which: &'static str, theirs: [u8; 32]) -> Result<(), Abort> {
        let mine = self.anchored_id()?;
        if theirs != mine {
            return Err(anchor_refused(which, Abort::CeremonyIdMismatch));
        }
        Ok(())
    }

    /// The local session's key for this ceremony (`wire-format.md`
    /// §14.3.2): SHA-256 of the session tag and both 16-byte
    /// contributions, under a label distinct from the pre-commitment's.
    ///
    /// **Available exactly when a carriage can move.** Both contributions
    /// are needed, and the second arrives optically before the first
    /// carriage does, so there is no ordering in which a sender holds half
    /// a key. Before that this is `NoCeremonyId`, the same answer
    /// [`Client::anchored_id`] gives for the same reason.
    pub(crate) fn session_key(&self) -> Result<Zeroizing<[u8; 32]>, Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        let theirs = a.their_contribution.ok_or(Abort::NoCeremonyId)?;
        Ok(crate::keys::session_key(
            (&self.public.keyhash, &a.contribution),
            (&a.peer()?, &theirs),
        ))
    }

    /// Seal one carriage for this ceremony's session.
    pub(crate) fn seal(&self, plaintext: &[u8]) -> Result<Vec<u8>, Abort> {
        let (k, cid) = (self.session_key()?, self.anchored_id()?);
        Ok(crate::local::seal_carriage(&k, &cid, plaintext))
    }

    /// Open one carriage, or refuse it as [`Abort::Unsealable`].
    pub(crate) fn open(&self, sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>, Abort> {
        let (k, cid) = (self.session_key()?, self.anchored_id()?);
        crate::local::open_carriage(&k, &cid, sealed)
            .ok_or_else(|| abort("open_carriage", Abort::Unsealable))
    }

    /// Seal and open a carriage **as a co-present adversary would**.
    ///
    /// `wire-format.md` §14.3.1 concedes exactly this: a party that read
    /// both screens holds the session key, so it can wrap any payload in a
    /// carriage that opens and re-anchor one ceremony's message into
    /// another. §14.3.2's encryption moves who can do that, not whether it
    /// can be done, and `models/tamarin/local/exchange.spthy` carries both
    /// as lemmas that verify.
    ///
    /// **These exist so a test can be that adversary**, which is the only
    /// way to exercise the anchor checks now that the key refuses
    /// everybody else before they are reached. A shell has no use for them
    /// and must not acquire one: `light-client-requirements.md` §9 keeps
    /// cryptography in the kernel, and neither is reachable through the
    /// FFI.
    #[doc(hidden)]
    pub fn seal_as_co_present(&self, plaintext: &[u8]) -> Result<Vec<u8>, Abort> {
        self.seal(plaintext)
    }

    /// The counterpart of [`Client::seal_as_co_present`].
    #[doc(hidden)]
    pub fn open_as_co_present(&self, sealed: &[u8]) -> Result<Vec<u8>, Abort> {
        self.open(sealed).map(|p| p.to_vec())
    }

    /// This ceremony's id, or why there is none: no ceremony under way,
    /// or one whose id the counterparty's contribution has not fixed yet.
    pub(crate) fn anchored_id(&self) -> Result<[u8; 32], Abort> {
        self.active
            .as_ref()
            .ok_or(Abort::NotActive)?
            .ceremony_id
            .ok_or(Abort::NoCeremonyId)
    }

    /// Take the counterparty's intent: its contribution fixes the
    /// pre-commitment with mine, and the window opens.  The responder
    /// adopts the initiator's start if it is within tolerance of its own.
    pub fn take_intent(&mut self, from: Keyhash, intent: &Intent) -> Result<[u8; 32], Abort> {
        let me = self.keyhash();
        let tolerance = self.cfg.clock_tolerance_s;
        let a = self.active()?;
        if a.counterparty != Some(from) {
            return Err(abort("take_intent", Abort::NotActive));
        }
        if intent.started_at.abs_diff(a.started_at) > tolerance {
            return Err(anchor_refused("intent", Abort::ClockFar));
        }
        // the flag is self-reported; the one check that converts two
        // silent deadlocks, both proposing and neither, into a refusal
        if intent.initiator == a.initiator {
            return Err(anchor_refused("intent", Abort::InitiatorClaim));
        }
        if intent.initiator && !a.initiator {
            a.started_at = intent.started_at;
        }
        let cid = pre_commitment((&me, &a.contribution), (&from, &intent.contribution));
        a.ceremony_id = Some(cid);
        // **The contribution is kept, not just consumed.**  Both paths that
        // fix a ceremony-id see this value and only the optical one used to
        // record it, so a ceremony begun through the value path held an id
        // whose inputs it could no longer name.  Nothing needed them until
        // the local session's key did (`wire-format.md` §14.3.2), and in
        // the optical flow this is the value `take_intent_carriage` has
        // already checked the echo against, so writing it again changes
        // nothing there [2026-10-05].
        a.their_contribution = Some(intent.contribution);
        // **the nominees no longer ride the intent** [author, 2026-10-19]:
        // the sets travel by locator once the id has fixed (kind 22), and
        // the counterparty's survivors are what this side keeps of them
        a.their_bundle = intent.bundle.clone();
        a.their_retention = intent.retention_years;
        // the sender's own position, kept as the locator held for them:
        // its own statement replaces whatever a third party said of it
        self.hold_locator(&from, &intent.locator, true)?;
        self.touch_contact(&from);
        self.opening_id_fixed(cid);
        tracing::info!(
            target: "cer",
            from = %id8(&from),
            entries = intent.bundle.len(),
            retention = intent.retention_years,
            initiator = intent.initiator,
            "cer.intent.received"
        );
        tracing::info!(
            target: "cer",
            ceremony = %id8(&cid),
            took_ms = self.since_begin_ms(),
            "cer.id_fixed"
        );
        self.subject.open_window(cid);
        Ok(cid)
    }

    /// The active ceremony's id, once both contributions have fixed it.
    pub fn ceremony_id(&self) -> Option<[u8; 32]> {
        self.active.as_ref().and_then(|a| a.ceremony_id)
    }

    /// The key the counterparty seals its captures of me under (design
    /// §7.5.2.6), derived from my identity root and this ceremony's id.
    ///
    /// **Nothing is drawn for it and nothing is kept** [author,
    /// 2026-10-17]: the same three inputs derive it again whenever this
    /// subject wants to release, on this device or another of the same
    /// identity.
    pub fn capture_key(&self) -> Result<Zeroizing<[u8; 32]>, Abort> {
        let cid = self.anchored_id()?;
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        Ok(capture_key(
            self.signer()?.identity_root(),
            &self.keyhash(),
            &a.peer()?,
            &cid,
        ))
    }

    /// Step 5: tell the person what the record will contain and who can
    /// read it, run the guided capture of the counterparty, derive the
    /// template, seal both under the key the counterparty supplied, and
    /// let that key go: it is wiped here, and this side holds no copy.
    pub fn capture(&mut self, their_key: Zeroizing<[u8; 32]>) -> Result<(), Abort> {
        self.capture_begin()?;
        loop {
            let turn = self.capture_tick()?;
            if turn.done {
                break;
            }
            self.device.clock.wait_ms(turn.next_ms);
        }
        self.capture_seal(their_key)
    }

    /// **Begin the hold** (design §7.1 item 5): the capture runs for the
    /// ceremony's window — `witness_window_s`, or the capture's own drawn
    /// span where that is zero, the harness's idiom — concurrently with
    /// everything else inside it, so it is a machine the shell turns with
    /// [`Client::capture_tick`] rather than a call that holds this thread.
    /// Begun once; a second call is a no-op.
    pub fn capture_begin(&mut self) -> Result<(), Abort> {
        let window_ms = self.cfg.witness_window_s * 1_000;
        let a = self.active.as_mut().ok_or(Abort::NotActive)?;
        a.peer()?;
        a.ceremony_id.ok_or(Abort::NoCeremonyId)?;
        if a.hold.is_some() {
            return Ok(());
        }
        self.device.notifier.notify(Notice::RecordDisclosure {
            role: Role::Participant,
        });
        a.hold = Some(Hold::begin(
            self.device.clock.as_ref(),
            self.device.random.as_ref(),
            self.cfg.template_version,
            &self.cfg.capture,
            &self.cfg.continuity,
            Some(window_ms),
        ));
        a.hold_done = false;
        Ok(())
    }

    /// **One turn of the hold**: a prompt where one falls now, a probe
    /// otherwise.  **The first retained frame yields the template** the
    /// queries carry (design §7.1 item 6: "the first query fires as soon as
    /// a captured frame yields a template"), so the verification starts
    /// while the capture continues.  A lost hold ends the ceremony here,
    /// as it did in the one-call form.
    pub fn capture_tick(&mut self) -> Result<CaptureTurn, Abort> {
        let template_len = self.cfg.seal.template_len;
        let a = self.active.as_mut().ok_or(Abort::NotActive)?;
        let hold = a.hold.as_mut().ok_or(Abort::NotActive)?;
        let turn = hold.tick(
            self.device.camera.as_ref(),
            self.device.clock.as_ref(),
            self.device.random.as_ref(),
            self.device.engine.as_ref(),
        );
        if a.template.is_none() && !hold.frames().is_empty() {
            let t = self.device.engine.template(hold.frames());
            if t.len() == template_len {
                a.template = Some(t);
            }
        }
        let frames = hold.frames().len() as u64;
        let template_ready = a.template.is_some();
        let face = hold.seen_now();
        let away_ms = hold.away_ms(self.device.clock.as_ref());
        match turn {
            Tick::Lost(why) => {
                tracing::info!(target: "cer", ?why, "cer.capture.lost");
                Err(abort("capture", Abort::LostHold(why)))
            }
            Tick::Done => {
                a.hold_done = true;
                Ok(CaptureTurn {
                    done: true,
                    next_ms: 0,
                    frames,
                    template_ready,
                    face,
                    away_ms,
                })
            }
            Tick::Continue { next_ms } => Ok(CaptureTurn {
                done: false,
                next_ms,
                frames,
                template_ready,
                face,
                away_ms,
            }),
        }
    }

    /// **Seal the capture under the counterparty's key** once the window
    /// has elapsed (design §7.5.2.6): the frames retained at the prompts,
    /// the template they yield, sealed and held for the record.  Refused
    /// with `Waiting` while the hold is still running — the key may land
    /// before the window is out, and the shell asks again when it is.
    /// **The key is discarded once the capture is sealed.**
    pub fn capture_seal(&mut self, their_key: Zeroizing<[u8; 32]>) -> Result<(), Abort> {
        let me = self.keyhash();
        let (peer, cid) = {
            let a = self.active()?;
            (a.peer()?, a.ceremony_id.ok_or(Abort::NoCeremonyId)?)
        };
        let (hold, done) = {
            let a = self.active.as_mut().ok_or(Abort::NotActive)?;
            if a.hold.is_none() {
                return Err(Abort::NotActive);
            }
            if !a.hold_done {
                return Err(Abort::Waiting("the capture's window"));
            }
            (a.hold.take().expect("checked"), a.hold_done)
        };
        debug_assert!(done);
        let ms = hold.elapsed_ms(self.device.clock.as_ref());
        let frames = match hold.finish() {
            Held::Kept(frames, _prompts) => frames,
            Held::Lost(why) => {
                tracing::info!(target: "cer", ?why, "cer.capture.lost");
                return Err(abort("capture", Abort::LostHold(why)));
            }
        };
        let template = self.device.engine.template(&frames);
        // the count and the version, never a frame or the template
        tracing::info!(
            target: "cer",
            image_count = frames.len(),
            template_version = self.cfg.template_version,
            ms,
            ok = template.len() == self.cfg.seal.template_len,
            "cer.capture"
        );
        if template.len() != self.cfg.seal.template_len {
            return Err(abort("capture", Abort::TemplateLength));
        }
        let capture = Capture {
            modality: 0,
            template_version: self.cfg.template_version,
            template: template.clone(),
            frames,
        };
        let sealed = seal(&self.cfg.seal, &their_key, peer, me, cid, &capture);
        drop(their_key);
        let a = self.active()?;
        a.image_count = capture.frames.len() as u64;
        a.sealed = Some(sealed);
        a.template = Some(template);
        Ok(())
    }

    /// Step 6, as selector: read the counterparty's bundle as my own claim
    /// about it and pick its verifiers by what I recognise.
    pub fn select_verifiers(&mut self) -> Result<Vec<(Keyhash, SelectionBasis)>, Abort> {
        let me = self.keyhash();
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        let pool = selection::pool(&a.their_bundle, &a.peer()?, &me);
        // **a verified introduction pins its subject**: the key to verify
        // a verifier's answer by and the position to reach it at came
        // with the bundle, and nothing else need supply them
        for i in &pool.introduced {
            self.contact(
                &i.introduction.identity,
                crate::contacts::How::Introduced,
                false,
            );
            let _ = self.hold_locator(&i.subject, &i.introduction.locator.bytes, false);
        }
        if !pool.candidates.is_empty() && !self.acquaintance.recognises_any(&pool) {
            tracing::warn!(target: "cer", pool = pool.candidates.len(), "cer.select.none_recognised");
            self.device.notifier.notify(Notice::NoCandidateRecognised);
        }
        let need = required(pool.n, pool.candidates.len());
        let picked = selection::select(&pool, &self.acquaintance, need, true);
        tracing::info!(
            target: "cer",
            n = pool.n,
            pool = pool.candidates.len(),
            required = need,
            selected = picked.len(),
            bases = ?picked.iter().map(|(_, b)| *b as u64).collect::<Vec<_>>(),
            "cer.select"
        );
        Ok(picked)
    }

    /// The query to `verifier` about the counterparty: the fuzzed profile of
    /// the person in front of me, under this ceremony.
    pub fn query_for(&mut self, verifier: Keyhash) -> Result<VerificationQuery, Abort> {
        let me = self.keyhash();
        let version = self.cfg.template_version;
        let (subject, cid, template) = {
            let a = self.active.as_ref().ok_or(Abort::NotActive)?;
            (
                a.peer()?,
                a.ceremony_id.ok_or(Abort::NoCeremonyId)?,
                // **the capture has not yielded a template yet**: the
                // queries carry the counterparty's fuzzed profile, which
                // comes from this side's frames of them, and the hold runs
                // beside the conversation on its own clock. Said as a wait,
                // not as no ceremony: a caller that put the queries the
                // moment the keep arrived met NotActive and abandoned a live
                // ceremony with a verifier selected (run 8bc1291-b1-node-1,
                // GXV, 2026-10-25)
                a.template.clone().ok_or(Abort::Waiting(
                    "a template of the counterparty from the capture",
                ))?,
            )
        };
        let profile = self.device.engine.profile(&template);
        let q = VerificationQuery {
            subject,
            querier: me,
            ceremony_id: cid,
            profile,
            template_version: version,
            verifier,
        };
        let qid = q.query_id();
        self.active()?.issued.insert(qid);
        tracing::info!(
            target: "cer",
            query = %id8(&qid),
            verifier = %id8(&verifier),
            subject = %id8(&subject),
            "cer.query.issued"
        );
        Ok(q)
    }

    /// The request that carries a consented query to its verifier.
    pub fn request(
        &mut self,
        q: &VerificationQuery,
        consent: Vec<u8>,
        basis: SelectionBasis,
    ) -> Vec<u8> {
        let bytes = QueryRequest {
            query: q.clone(),
            consent,
            selection_basis: basis as u64,
        }
        .encode();
        self.awaiting_answer(q.query_id(), q.verifier, payload::KIND_QUERY, &bytes);
        bytes
    }

    /// Note what went to a verifier, for the answer owed to it.
    fn awaiting_answer(&mut self, query: [u8; 32], to: Keyhash, kind: u64, bytes: &[u8]) {
        let now = self.now_s();
        if let Some(a) = self.active.as_mut() {
            a.conversation.outgoing.insert(
                query,
                crate::sequence::Outgoing {
                    to,
                    kind,
                    bytes: bytes.to_vec(),
                    at_s: now,
                    resent: false,
                },
            );
        }
    }

    /// **A query or grant unanswered at half the patience goes again on
    /// a fresh session**, once.  Every session with that verifier is set
    /// aside and the same bytes sent, which asks for a one-time key and
    /// opens anew when it comes; the verifier's crossing rule takes the
    /// open (`wire-format.md` §7.10).  What the verifier still sends on
    /// the old session opens here on the one set aside.
    ///
    /// Run by `maintain`, on the kernel's own clock: nothing of this is
    /// the shell's to drive.
    pub fn resend_unanswered(&mut self, now: u64) -> Vec<Msg> {
        let half = self.cfg.query_patience_s / 2;
        let due: Vec<([u8; 32], Keyhash, u64, Vec<u8>)> = match self.active.as_mut() {
            Some(a) => a
                .conversation
                .outgoing
                .iter_mut()
                .filter(|(_, o)| !o.resent && now >= o.at_s.saturating_add(half))
                .map(|(q, o)| {
                    o.resent = true;
                    (*q, o.to, o.kind, o.bytes.clone())
                })
                .collect(),
            None => return Vec::new(),
        };
        let mut out = Vec::new();
        for (query, to, kind, bytes) in due {
            let displaced = self.payload.sessions.displace(&to);
            tracing::warn!(
                target: "pay",
                query = %id8(&query),
                kind = crate::diag::kind_name(kind),
                to = %id8(&to),
                displaced,
                "pay.resend"
            );
            match self.send_payload(to, kind, &bytes) {
                Ok(m) => out.extend(m),
                Err(why) => tracing::warn!(
                    target: "pay",
                    query = %id8(&query),
                    to = %id8(&to),
                    %why,
                    "pay.resend.refused"
                ),
            }
        }
        out
    }

    /// Step 6, as subject: consent to a query about me, or not; and where I
    /// consent, the grant for its verifier, which goes to that verifier
    /// directly and to nobody else.
    ///
    /// Consent is the subject's act under the identity key, so a device
    /// holding no seed gives none: the query is answered on the ceremony
    /// device or not at all.
    pub fn consent(&mut self, q: &VerificationQuery) -> Option<(Vec<u8>, Option<KeyGrant>)> {
        let me = self.signer.as_deref()?;
        let consent = self
            .subject
            .consent_to(me, q, self.device.notifier.as_ref())?;
        let grant =
            self.subject
                .grant_for(me, &self.store, &q.verifier, q.query_id(), self.now_s());
        if let Some(g) = &grant {
            self.awaiting_answer(
                q.query_id(),
                q.verifier,
                payload::KIND_KEY_GRANT,
                &g.encode(),
            );
        }
        // whether a key was released, never the key
        tracing::info!(
            target: "sub",
            query = %id8(&q.query_id()),
            verifier = %id8(&q.verifier),
            granted = grant.is_some(),
            "sub.grant"
        );
        Some((consent, grant))
    }

    /// As verifier: a grant from `from`.
    pub fn take_grant(&mut self, from: Keyhash, bytes: &[u8]) -> GrantOutcome {
        let Some(me) = self.signer.as_deref() else {
            return GrantOutcome::Rejected(NO_SEED);
        };
        let cx = Verifying {
            seal: &self.cfg.seal,
            me,
            ids: &self.known,
            store: &self.store,
            matcher: self.device.engine.matcher(),
        };
        self.verifier
            .take_grant(&cx, from, bytes, self.device.clock.now_ms())
    }

    /// As verifier: a query from `from`.  The person is neither asked nor
    /// told: answering is the client's background task, and design §19.6
    /// owes a verifier no warning.
    pub fn take_query(&mut self, from: Keyhash, bytes: &[u8]) -> QueryOutcome {
        let Some(me) = self.signer.as_deref() else {
            return QueryOutcome::Closed(NO_SEED);
        };
        let cx = Verifying {
            seal: &self.cfg.seal,
            me,
            ids: &self.known,
            store: &self.store,
            matcher: self.device.engine.matcher(),
        };
        self.verifier
            .take_query(&cx, from, bytes, self.device.clock.now_ms())
    }

    /// As verifier: let the bounds pass by this clock.  A query whose grant
    /// never came within the buffer is answered `unavailable`; a grant
    /// whose query never came is dropped unopened.
    pub fn expire(&mut self) -> Vec<crate::verifier::Answer> {
        let Some(me) = self.signer.as_deref() else {
            return Vec::new();
        };
        let cx = Verifying {
            seal: &self.cfg.seal,
            me,
            ids: &self.known,
            store: &self.store,
            matcher: self.device.engine.matcher(),
        };
        self.verifier.expire(&cx, self.device.clock.now_ms())
    }

    /// As querier: a response to a query I issued, verified under its
    /// verifier.  The query it answers and the verdict.
    pub fn take_response(&mut self, bytes: &[u8]) -> Result<([u8; 32], Verdict), String> {
        let out = self.take_response_in(bytes);
        if out.is_ok()
            && let Ok(r) = Response::read(bytes)
        {
            self.note_contact(&r.verifier, crate::contacts::How::Verifier);
        }
        if tracing::event_enabled!(target: "cer", tracing::Level::INFO) {
            match &out {
                Ok((q, v)) => {
                    let verifier = Response::read(bytes)
                        .map(|r| id8(&r.verifier))
                        .unwrap_or_default();
                    tracing::info!(target: "cer", query = %id8(q), %verifier, verdict = ?v, "cer.response");
                }
                Err(why) => tracing::warn!(target: "cer", %why, "cer.response.refused"),
            }
        }
        out
    }

    fn take_response_in(&mut self, bytes: &[u8]) -> Result<([u8; 32], Verdict), String> {
        let r = Response::read(bytes)?;
        verify::response(&self.known, bytes, false).map_err(|e| e.to_string())?;
        let a = self.active.as_mut().ok_or("no ceremony")?;
        if !a.issued.contains(&r.query_id) {
            return Err("not a query I issued".into());
        }
        a.responses.push(bytes.to_vec());
        a.conversation.outgoing.remove(&r.query_id);
        Ok((r.query_id, r.verdict))
    }

    /// As subject: the copy of a response about me.
    pub fn take_response_copy(&mut self, bytes: &[u8]) -> Result<[u8; 32], String> {
        let me = self.signer.as_deref().ok_or(NO_SEED)?;
        let out = self.subject.take_response_copy(me, &self.known, bytes);
        if let (Ok(q), Some(a)) = (&out, self.active.as_mut()) {
            a.conversation.outgoing.remove(q);
        }
        match &out {
            Ok(q) => tracing::info!(target: "sub", query = %id8(q), "sub.response_copy"),
            Err(why) => tracing::warn!(target: "sub", %why, "sub.response_copy.refused"),
        }
        out
    }

    /// The responses I gathered about the counterparty.
    pub fn responses(&self) -> Vec<Vec<u8>> {
        self.active
            .as_ref()
            .map(|a| a.responses.clone())
            .unwrap_or_default()
    }

    /// The parties this client is peered with (design §6.3), from the
    /// peering records it holds.
    pub fn peers(&self) -> Vec<Keyhash> {
        self.store.peers(&self.public.keyhash)
    }

    /// Who is in front of this device in the active ceremony, where one is
    /// under way and the first code has named them.
    pub fn counterparty(&self) -> Option<Keyhash> {
        self.active.as_ref().and_then(|a| a.counterparty)
    }

    /// **The parties this client has met**, as a nominee list: the
    /// acquaintance it keeps from its own records, less itself and less
    /// `except` (the counterparty of the meeting at hand, who cannot
    /// witness it), at most ten [ruled, author, 2026-10-10: witnesses are
    /// required where available].
    pub fn met(&self, except: Option<&Keyhash>) -> Vec<Keyhash> {
        let me = self.keyhash();
        let mut out: Vec<Keyhash> = self
            .acquaintance
            .met
            .iter()
            .filter(|p| **p != me && Some(*p) != except)
            .copied()
            .collect();
        out.truncate(10);
        out
    }

    /// The two nominee lists of the active ceremony: mine, then theirs.
    pub fn nominees(&self) -> (Vec<Keyhash>, Vec<Keyhash>) {
        self.active
            .as_ref()
            .map(|a| {
                (
                    a.my_nominees.iter().copied().collect(),
                    a.their_nominees.clone(),
                )
            })
            .unwrap_or_default()
    }

    /// **How far the engagement has got**: the nominees reached, of those
    /// nominated (kind 20) [author, 2026-10-19].  A shell shows the pair
    /// while the opening runs; a nominee still unreached when the set
    /// crosses was unavailable and is not offered to the prune.
    #[must_use]
    pub fn nominees_engaged(&self) -> (usize, usize) {
        self.active
            .as_ref()
            .map(|a| (a.engaged_to.len(), a.my_nominees.len()))
            .unwrap_or_default()
    }

    /// What a nominee is asked to witness.
    pub fn witness_request(&self) -> Result<WitnessRequest, Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        let req = WitnessRequest {
            ceremony_id: a.ceremony_id.ok_or(Abort::NoCeremonyId)?,
            participants: participants(self.keyhash(), a.peer()?),
            started_at: a.started_at,
            handle: a.handle,
        };
        tracing::info!(
            target: "cer",
            ceremony = %id8(&req.ceremony_id),
            nominees = a.my_nominees.len(),
            "cer.witness.asked"
        );
        Ok(req)
    }

    /// The engagement this side sends its nominees (kind 20): the handle
    /// minted at `begin`, the claimed start, and the counterparty where a
    /// code has named one.
    pub fn engagement(&self) -> Result<crate::conversation::Engagement, Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        Ok(crate::conversation::Engagement {
            handle: a.handle,
            started_at: a.started_at,
            counterparty: a.counterparty,
        })
    }

    /// As nominee: the engagement itself (kind 20) [author, 2026-10-19],
    /// held so the trace that follows has somewhere to land and the later
    /// request something to close.  The claimed start is checked against
    /// this clock as a request's is; held ceremonies are bounded as
    /// `witnessing` is, the oldest claimed start going.
    pub fn take_engagement(&mut self, from: Keyhash, e: &crate::conversation::Engagement) -> bool {
        if self.engaged.contains_key(&e.handle) {
            return true;
        }
        if witness_check(e.started_at, self.now_s(), self.cfg.clock_tolerance_s).is_err() {
            tracing::warn!(
                target: "cer",
                from = %id8(&from),
                why = "the claimed start is outside this clock's tolerance",
                "cer.witness.declined"
            );
            return false;
        }
        if self.engaged.len() >= crate::sequence::PENDING_WITNESS_REQUESTS {
            let oldest = self
                .engaged
                .iter()
                .min_by_key(|(_, e)| e.started_at)
                .map(|(k, _)| *k);
            if let Some(k) = oldest {
                self.engaged.remove(&k);
            }
        }
        let held_ms = self.device.clock.now_ms();
        tracing::info!(
            target: "cer",
            handle = %id8(&e.handle),
            from = %id8(&from),
            held = self.engaged.len() + 1,
            "cer.witness.engaged_by"
        );
        self.engaged.insert(
            e.handle,
            crate::sequence::Engaged {
                from,
                started_at: e.started_at,
                counterparty: e.counterparty,
                ceremony_id: None,
                trace: Vec::new(),
                held_ms,
            },
        );
        true
    }

    /// **How many ceremonies this client holds as a witness**: the
    /// requests it accepted and has not closed (`light-client-requirements.md`
    /// §1.2's bound).
    #[must_use]
    pub fn witnessing_count(&self) -> usize {
        self.witnessing.len()
    }

    /// **What this client watched under `handle`**: each trace event as
    /// `(event, value, arrival)`, the arrival by this device's clock in
    /// milliseconds [author, 2026-10-19].  The pattern is what a witness
    /// attests from (design §7.1 item 9), so it is the witness's to read.
    #[must_use]
    pub fn observed_trace(&self, handle: &[u8; 32]) -> Vec<(u64, u64, u64)> {
        self.engaged
            .get(handle)
            .map(|e| e.trace.clone())
            .unwrap_or_default()
    }

    /// As nominee: one event of the trace (kind 21), from the party that
    /// engaged this client and nobody else.  The confirmation fixes the
    /// ceremony-id the later request must repeat (CER-70); every event's
    /// arrival is kept on this clock, the pattern being the evidence
    /// (design §7.1 item 9).
    pub fn take_trace(&mut self, from: Keyhash, t: &crate::conversation::Trace) -> bool {
        let now = self.device.clock.now_ms();
        let Some(e) = self.engaged.get_mut(&t.handle) else {
            return false;
        };
        if e.from != from {
            return false;
        }
        use crate::conversation::TraceEvent::*;
        let (code, value) = match t.event {
            Shape { mine, theirs } => (0, mine.max(theirs)),
            Inbound(i) => (1, i),
            Outbound(i) => (2, i),
            Confirmed(cid) => {
                e.ceremony_id = Some(cid);
                (3, 0)
            }
        };
        e.trace.push((code, value, now));
        // **the arrival, on this clock, in the stream** [author,
        // 2026-10-19]: what a witness attests from is the pattern these
        // offsets make, so a run is read by whether they spread across the
        // exchange or arrive together at its end (design §7.1 items 3, 9)
        let since_ms = now.saturating_sub(e.held_ms);
        let seen = e.trace.len();
        tracing::info!(
            target: "cer",
            handle = %id8(&t.handle),
            from = %id8(&from),
            event = match code {
                0 => "shape",
                1 => "inbound",
                2 => "outbound",
                _ => "confirmed",
            },
            value,
            since_ms,
            seen,
            "cer.witness.watched"
        );
        true
    }

    /// As nominee: witness, or decline.  The claimed start must be within
    /// tolerance of this clock (`light-client-requirements.md` §1.2); the
    /// person is told, not asked (design Appendix A.3).  The flags attest
    /// what was observed: the protocol ran, both were responsive, and the
    /// latency bound where a latency channel passed.
    ///
    /// Asked again for a ceremony already held, by the other participant
    /// or as a repeat, the answer given stands.  At most
    /// [`crate::sequence::PENDING_WITNESS_REQUESTS`] ceremonies are held
    /// at once, the holder's own bound as the other bounds are: past it
    /// the one with the oldest claimed start goes.
    pub fn take_witness_request(&mut self, req: &WitnessRequest) -> Option<u64> {
        if let Some(w) = self.witnessing.get(&req.ceremony_id) {
            return Some(w.flags);
        }
        // **The request closes an engagement** [author, 2026-10-19]: a
        // request whose handle this client never watched binds nothing it
        // observed, and a ceremony-id disagreeing with the trace's
        // confirmation means the sequence it watched is not the ceremony
        // it is asked to sign.  Both decline (CER-70).
        match self.engaged.get(&req.handle) {
            None => {
                tracing::warn!(
                    target: "cer",
                    ceremony = %id8(&req.ceremony_id),
                    why = "a request whose engagement this client never watched",
                    "cer.witness.declined"
                );
                return None;
            }
            Some(e) => {
                if e.ceremony_id.is_some_and(|cid| cid != req.ceremony_id) {
                    tracing::warn!(
                        target: "cer",
                        ceremony = %id8(&req.ceremony_id),
                        why = "the ceremony-id disagrees with the trace's confirmation",
                        "cer.witness.declined"
                    );
                    return None;
                }
                if let Some(cp) = e.counterparty
                    && !req.participants.contains(&cp)
                {
                    tracing::warn!(
                        target: "cer",
                        ceremony = %id8(&req.ceremony_id),
                        why = "the request names a meeting the engagement did not",
                        "cer.witness.declined"
                    );
                    return None;
                }
            }
        }
        if witness_check(req.started_at, self.now_s(), self.cfg.clock_tolerance_s).is_err() {
            tracing::warn!(
                target: "cer",
                ceremony = %id8(&req.ceremony_id),
                why = "the claimed start is outside this clock's tolerance",
                "cer.witness.declined"
            );
            return None;
        }
        let flags = 3;
        let mut evicted = false;
        if self.witnessing.len() >= crate::sequence::PENDING_WITNESS_REQUESTS {
            let oldest = self
                .witnessing
                .iter()
                .min_by_key(|(_, w)| w.request.started_at)
                .map(|(k, _)| *k);
            if let Some(k) = oldest {
                self.witnessing.remove(&k);
                evicted = true;
            }
        }
        tracing::info!(
            target: "cer",
            ceremony = %id8(&req.ceremony_id),
            flags,
            evicted,
            held = self.witnessing.len() + 1,
            "cer.witness.answered"
        );
        self.witnessing.insert(
            req.ceremony_id,
            crate::sequence::Witnessing {
                request: req.clone(),
                flags,
                held_ms: self.device.clock.now_ms(),
                pending: None,
                observed: BTreeSet::new(),
            },
        );
        Some(flags)
    }

    /// The back-pointers this client would supply for a proposed body.
    pub fn back_pointers(&self) -> Vec<Txid> {
        let back = self.archive.next_back_pointers();
        tracing::debug!(target: "cer", count = back.len(), "cer.back_pointers");
        back
    }

    /// Step 7, as proposer: the body everyone will sign.  Responses from
    /// both sides, sorted as the body requires; the disclosure set with
    /// fresh salts; the witnesses that accepted.
    pub fn propose(
        &mut self,
        their_responses: Vec<Vec<u8>>,
        witnesses: Vec<Witness>,
    ) -> Result<(Proposal, DisclosureSet), Abort> {
        let me = self.keyhash();
        let now = self.now_s();
        let salts: [[u8; 16]; 6] = std::array::from_fn(|_| self.random::<16>());
        let (retention, their_retention) =
            (self.cfg.retention_years, self.active()?.their_retention);
        let mine_back = self.archive.next_back_pointers();
        let a = self.active()?;
        let parts = participants(me, a.peer()?);
        let (r0, r1) = if parts[0] == me {
            (retention, their_retention)
        } else {
            (their_retention, retention)
        };
        let values = [
            record::capture_value(0, a.image_count, 0, 1),
            record::empty_location_value(),
            record::integrity_value(false, 0),
            record::retention_value(r0),
            record::integrity_value(false, 0),
            record::retention_value(r1),
        ];
        let set = disclosures(values, salts);
        let mut responses = a.responses.clone();
        responses.extend(their_responses);
        sort_responses(&mut responses);
        // **No witness proposes anyway** [author, 2026-10-19]: there is
        // no floor (`wire-format.md` §3.2), the absent field stays
        // readable in the signed body forever, and the readers it matters
        // to — the participants and the patrons who later review their
        // archives — weigh it there (design §7.1.1, §13.2).  The
        // formation subtype remains the genesis pair's shape alone.
        let formation = witnesses.is_empty() && {
            let counterparty = a.peer()?;
            // the counterparty's back-pointers where the conversation has
            // carried them; a driver that hands them to review instead
            // has the codec's genesis rule catch a pair that is not fresh
            let theirs = a.conversation.back.get(&counterparty);
            responses.is_empty()
                && mine_back == vec![rhtn_archive::genesis(&me)]
                && theirs.is_none_or(|t| *t == vec![rhtn_archive::genesis(&counterparty)])
        };
        // **a party signs once.** A nominee both participants named answers
        // both and is handed here twice; the record has one place for it,
        // and the body must be the same whichever side proposes, so the
        // place is attributed to the participant that sorts first.
        let witnesses = {
            let rank = |n: &Keyhash| parts.iter().position(|p| p == n);
            let mut kept: Vec<Witness> = Vec::with_capacity(witnesses.len());
            for w in witnesses {
                match kept.iter_mut().find(|k| k.keyhash == w.keyhash) {
                    Some(k) => {
                        if rank(&w.nominated_by) < rank(&k.nominated_by) {
                            *k = w;
                        }
                    }
                    None => kept.push(w),
                }
            }
            kept
        };
        let proposal = Proposal {
            started_at: a.started_at,
            finalized_at: now,
            participants: parts,
            witnesses,
            responses,
            root: disclosure_root(&set),
            formation,
        };
        tracing::info!(
            target: "cer",
            witnesses = proposal.witnesses.len(),
            responses = proposal.responses.len(),
            formation,
            took_ms = self.since_begin_ms(),
            "cer.propose"
        );
        Ok((proposal, set))
    }

    /// The checks every signer makes on a proposal: the root is the root
    /// of the set shown, and the body carries my own back-pointers at my
    /// position.  Then the participant's own (`light-client-requirements.md`
    /// §1.1, §1.4), and the person is told where their nominees are absent
    /// or outnumbered.  Signing is the last thing that happens.
    pub fn review_and_sign(
        &mut self,
        proposal: &Proposal,
        set: &DisclosureSet,
        back: &[Vec<Txid>],
    ) -> Result<Vec<u8>, Abort> {
        let me = self.keyhash();
        if proposal.root != disclosure_root(set) {
            return Err(abort("review_and_sign", Abort::RootMismatch));
        }
        self.check_back(proposal, back)?;
        let held = self.subject.responses.clone();
        let mine = self
            .active
            .as_ref()
            .map(|a| a.my_nominees.clone())
            .unwrap_or_default();
        participant_check(&me, proposal, &mine, &held, self.device.notifier.as_ref())
            .map_err(|r| abort("review_and_sign", Abort::Refused(r)))?;
        let entries = self
            .signer()?
            .sign_entries(aad::ENVELOPE, &proposal.body(back));
        tracing::info!(
            target: "cer",
            role = "participant",
            signers = proposal.signers().len(),
            "cer.review"
        );
        Ok(entries)
    }

    /// As witness: sign the ceremony I accepted, and only that one.
    pub fn witness_sign(
        &mut self,
        proposal: &Proposal,
        back: &[Vec<Txid>],
    ) -> Result<Vec<u8>, Abort> {
        let held = self.witnessing.values().find(|w| {
            w.request.started_at == proposal.started_at
                && w.request.participants == proposal.participants
        });
        let Some(w) = held else {
            return Err(abort("witness_sign", Abort::NotActive));
        };
        // **The window, on this clock, from this client's own receipt of
        // the request** (design §7.1 item 9;
        // `light-client-requirements.md` §1.2) [author, 2026-10-21]. The
        // participants will have been at it longer by now — the engagement
        // and the codes precede the request, and the channels, the capture
        // and the verification run inside the interval being timed — and
        // **a participant's claim about elapsed time neither shortens nor
        // lengthens it**, which is why `started_at` is not consulted here.
        // Withheld rather than refused: the answer is a signature or
        // nothing (`wire-format.md` §7.10.2).
        let waited_ms = self.device.clock.now_ms().saturating_sub(w.held_ms);
        let window_ms = self.cfg.witness_window_s * 1_000;
        if self.cfg.witness_window_s != 0 && waited_ms < window_ms {
            tracing::info!(
                target: "cer",
                ceremony = %id8(&w.request.ceremony_id),
                waited_ms,
                window_s = self.cfg.witness_window_s,
                "cer.witness.withheld"
            );
            return Err(Abort::Waiting("the window has not matured on this clock"));
        }
        self.check_back(proposal, back)?;
        let entries = self
            .signer()?
            .sign_entries(aad::ENVELOPE, &proposal.body(back));
        tracing::info!(
            target: "cer",
            role = "witness",
            signers = proposal.signers().len(),
            "cer.review"
        );
        Ok(entries)
    }

    fn check_back(&self, proposal: &Proposal, back: &[Vec<Txid>]) -> Result<(), Abort> {
        let me = self.keyhash();
        let pos = proposal
            .signers()
            .iter()
            .position(|s| *s == me)
            .ok_or(Abort::NotActive)?;
        if back.get(pos) != Some(&self.archive.next_back_pointers()) {
            return Err(abort("check_back", Abort::BackPointers));
        }
        Ok(())
    }

    /// The retention this client declares at its next intent, in whole
    /// years (design §7.5.1).
    pub fn retention_years(&self) -> u64 {
        self.cfg.retention_years
    }

    /// Declare a retention for the ceremonies from here on, in whole years
    /// (design §7.5.1; `light-client-requirements.md` §1.3: the person's
    /// own horizon).  Refused at zero, and while a ceremony is under way, whose
    /// intent already carries the figure the body will state.
    pub fn set_retention_years(&mut self, years: u64) -> Result<(), &'static str> {
        if years == 0 {
            return Err("a retention is at least one year");
        }
        if self.active.is_some() {
            return Err("a ceremony is under way and has declared its retention");
        }
        tracing::info!(
            target: "cer",
            from = self.cfg.retention_years,
            to = years,
            "cer.retention"
        );
        self.cfg.retention_years = years;
        Ok(())
    }

    /// Drop the ceremony under way without a record: nothing sealed is
    /// filed, and the contributions and the template go with it.
    pub fn abandon(&mut self) {
        tracing::info!(
            target: "cer",
            active = self.active.is_some(),
            took_ms = self.since_begin_ms(),
            "cer.abandon"
        );
        self.active = None;
        self.subject.responses.clear();
        self.drop_legs();
    }

    /// The legs still queued for peers this ceremony never reached go with
    /// it (see [`crate::payload::PayloadState::drop_legs`]).
    fn drop_legs(&mut self) {
        let dropped = self.payload.drop_legs();
        if dropped > 0 {
            tracing::info!(target: "cer", dropped, "cer.pending.dropped");
        }
    }

    /// The record, finalized: appended to my archive and kept; as a
    /// participant, the sealed capture of the counterparty and the row for
    /// the capture they hold of me are filed under it, the disclosure set
    /// is kept as record state, and
    /// the ceremony's window closes with everything it counted.
    pub fn finalize(
        &mut self,
        envelope: &[u8],
        set: Option<&DisclosureSet>,
    ) -> Result<Txid, Abort> {
        let rec = Record::parse(envelope).map_err(|e| abort("finalize", Abort::Record(e)))?;
        verify::envelope(&self.known, envelope)
            .map_err(|e| abort("finalize", Abort::Record(e.to_string())))?;
        let txid = rec.txid;
        let finalized_at = rec.effective;
        let parts = rec.participants();
        let signers = rec.signers.len();
        self.archive
            .append(rec.clone())
            .map_err(|e| abort("finalize", Abort::Record(e)))?;
        self.store.records.insert(txid, envelope.to_vec());
        // the ceremony closes only where this record is its own: a witness
        // in a ceremony of its own at the same time keeps that one
        let mine = parts.contains(&self.keyhash());
        // **every signer of a record of mine is a party dealt with**
        // (`light-client-requirements.md` §4.3): the counterparty, and each
        // witness that attested — captured here, the one place both the
        // conversation and a driver that gathers signatures itself arrive
        if mine {
            let me = self.keyhash();
            for s in rec.signers.clone() {
                if s == me {
                    continue;
                }
                let how = if parts.contains(&s) {
                    crate::contacts::How::Counterparty
                } else {
                    crate::contacts::How::Nominee
                };
                self.note_contact(&s, how);
            }
        }
        tracing::info!(
            target: "cer",
            txid = %id8(&txid),
            signers,
            mine,
            took_ms = self.since_begin_ms(),
            "cer.finalize"
        );
        // taken, not moved out of: `Active`'s `Drop` wipes the ceremony's
        // key material, and a type with a destructor cannot be dismantled
        // field by field.  Nothing secret is copied on the way past: what
        // `store.own` keeps is the counterparty and the ceremony's id, the
        // capture key being derived from the identity root when wanted
        // (design §7.5.2.6)
        if mine {
            self.drop_legs();
        }
        if let Some(mut a) = self.active.take_if(|_| mine) {
            let peer = a.peer()?;
            if let Some(sealed) = a.sealed.take() {
                self.store.sealed.insert(txid, sealed);
            }
            self.store.own.insert(
                txid,
                OwnCapture {
                    counterparty: peer,
                    ceremony_id: a.ceremony_id.unwrap_or([0; 32]),
                    finalized_at,
                },
            );
            if let Some(set) = set {
                self.store.disclosures.insert(txid, set.clone());
            }
            self.subject.close_window();
            self.subject.responses.clear();
            self.refresh_acquaintance();
        }
        // witnessed and done: the ceremony this record closes is no
        // longer pending
        self.witnessing
            .retain(|_, w| !w.request.participants.iter().all(|p| parts.contains(p)));
        // the record goes up to be flooded, whether this client was a
        // participant or a witness: a witness holds it and its signature
        // is in it, so it is as much the witness's to propagate
        self.outbox.push(Msg::Record(envelope.to_vec()));
        Ok(txid)
    }
}

impl Client {
    /// As the recovering subject: the prior key this rotation claims.
    pub fn prior_key(&self) -> Option<Keyhash> {
        self.rotation
            .as_ref()
            .and_then(|r| r.old_key())
            .map(|k| k.public.keyhash)
    }

    /// As a recovery verifier, its own querier (design §9.1): the query
    /// about the person in front of me, under this meeting's
    /// pre-commitment, addressed to myself — only where my own records show
    /// I met the prior key claimed.
    pub fn recovery_query(&mut self, prior: Keyhash) -> Result<VerificationQuery, Abort> {
        if !self.store.has_met(&prior) {
            return Err(Abort::NotRecognised);
        }
        let me = self.keyhash();
        self.query_for(me)
    }

    /// As a recovery verifier: the recognition is the person's (design
    /// §9.1), so the one question of the meeting is asked; on yes, the
    /// hybrid response naming the prior key, `personal_knowledge`, `met`.
    pub fn recognise(
        &mut self,
        q: &VerificationQuery,
        consent: &[u8],
        prior: Keyhash,
    ) -> Result<Vec<u8>, Abort> {
        let subject = q.subject;
        if !self.device.operator.ask(&format!(
            "Do you recognise the person in front of you as the holder of {}?",
            hex8(&prior)
        )) {
            return Err(Abort::NotRecognised);
        }
        Ok(recovery_response_with_consent(
            self.signer()?,
            &subject,
            &q.query_id(),
            consent,
            &prior,
        ))
    }

    /// As the recovering subject: a response to the query I consented to,
    /// verified hybrid under its verifier, naming my prior key.
    pub fn take_recovery_response(&mut self, bytes: &[u8]) -> Result<(), String> {
        let r = Response::read(bytes)?;
        let prior = self.prior_key().ok_or("no prior key")?;
        if r.subject != self.keyhash() || r.selection_basis != 0 {
            return Err("not about me as a recovery".into());
        }
        if !self
            .subject
            .consented(&self.ceremony_id().ok_or("no ceremony")?)
            .contains(&r.query_id)
        {
            return Err("a query I did not consent to".into());
        }
        verify::response(&self.known, bytes, true).map_err(|e| e.to_string())?;
        if crate::query::prior_key_of(bytes) != Some(prior) {
            return Err("names another prior key".into());
        }
        self.recovery_responses.push(bytes.to_vec());
        Ok(())
    }

    /// As the recovering subject: the `Recovery` block for adoption under
    /// `patron` — the old key's successor statement over prior, new and
    /// patron, and the responses gathered — and the old archive's head.
    pub fn recovery_block(&self, patron: &Keyhash) -> Result<(Vec<u8>, Option<Txid>), Abort> {
        let rot = self.rotation.as_ref().ok_or(Abort::NoPriorKey)?;
        let old = rot.old_key().ok_or(Abort::NoPriorKey)?;
        if !self
            .recovery_responses
            .iter()
            .any(|r| Response::read(r).is_ok_and(|x| x.verdict == crate::query::Verdict::Match))
        {
            return Err(Abort::NotRecognised);
        }
        Ok((
            recovery_block(
                old,
                &self.keyhash(),
                patron,
                self.recovery_responses.clone(),
            ),
            rot.old_head(),
        ))
    }

    /// As a patron: the adoption body for `node` under my position, the
    /// evidence checked against the body's own fields before anything is
    /// signed (`wire-format.md` §4.1).
    pub fn propose_adoption(
        &self,
        node: Keyhash,
        node_back: &[Txid],
        what: Adopting,
    ) -> Result<Vec<u8>, Abort> {
        self.propose_adoption_in(self.subnet(), node, node_back, what)
    }

    /// The same, naming the subnet to adopt into: one this client holds a
    /// position in, or none for the one under its own key.
    ///
    /// **Which tree matters and is the patron's to say.** A patron in two
    /// subnets sits at a different path in each, and the locator it issues
    /// is its path in the one it is adopting into; adopting "somewhere"
    /// would put the subordinate at an address the other subnet cannot
    /// read.  The locator names the subnet (`wire-format.md` §2.3): the
    /// patron's own; a lone root's first adoption names none, its own
    /// txid being the id from then on (design §13.8).
    pub fn propose_adoption_in(
        &self,
        subnet: Option<Txid>,
        node: Keyhash,
        node_back: &[Txid],
        what: Adopting,
    ) -> Result<Vec<u8>, Abort> {
        let Adopting {
            evidence,
            series,
            presented_head,
            key_material,
            carrying,
        } = what;
        let pos = match subnet {
            Some(s) => self
                .position_in(&s)
                .ok_or_else(|| Abort::PatronRefused(format!("no position in {}", hex8(&s))))?,
            None => self.own_position(),
        };
        let index = self.free_index(&pos)?;
        let mut path = pos.path.clone();
        let nibbles = pos.nibbles + 1;
        if pos.nibbles.is_multiple_of(2) {
            path.push(index << 4);
        } else {
            let last = path.len() - 1;
            path[last] |= index;
        }
        let locator = Locator {
            anchor: pos.anchor,
            path,
            nibbles,
            seqno: Seqno { series, counter: 0 },
            subnet: pos.subnet,
        };
        let back = self.archive.next_back_pointers();
        let a = Adoption {
            node,
            patron: self.keyhash(),
            locator,
            timestamp: self.now_s(),
            key_material,
            evidence,
            presented_head,
            back: [node_back, &back],
            carrying,
        };
        let body = adoption_body(&a);
        verify::adoption_evidence(&self.known, &body)
            .map_err(|e| Abort::PatronRefused(e.to_string()))?;
        Ok(body)
    }

    /// The lowest subordinate index this client has not already issued in
    /// the subnet `anchor` names.
    ///
    /// **Ten slots and no eleventh.** design §3.1 gives every node at most
    /// `f = 10` subordinates and `wire-format.md` §2.1 gives a path nibble
    /// the values 0 to 9, which is the same ten counted twice: the index
    /// *is* the slot. Two subordinates at one index would be two parties at
    /// one address, and the routing slot beneath it holds one occupant.
    ///
    /// **Read from this client's own archive**, which is the only complete
    /// record of what it has issued. A holder elsewhere may not have the
    /// other adoptions and so cannot check this; the patron always can,
    /// which is design §1.1's test answered in the patron's favour.
    fn free_index(&self, pos: &Locator) -> Result<u8, Abort> {
        let me = self.keyhash();
        // a relationship this client can see has ended frees the index it
        // held.  **Where it can see none, every index it issued stays
        // taken**: a patron that cannot observe a departure has no grounds
        // to reissue the slot, and reissuing one it was wrong about would
        // put two parties at one address.
        let ended: BTreeSet<Keyhash> = self
            .horizon
            .table
            .bindings()
            .iter()
            .filter(|b| b.patron == me && b.end.is_some())
            .map(|b| b.node)
            .collect();
        let mut taken: BTreeSet<u8> = BTreeSet::new();
        for rec in self
            .archive
            .records()
            .filter(|r| r.tx_type == TYPE_ADOPTION)
        {
            let (Some(patron), Some(node), Some(loc)) =
                (rec.field_hash(2), rec.field_hash(1), rec.locator())
            else {
                continue;
            };
            // a subordinate of this position: one hop below it, in its
            // subnet — or the first adoption under a lone root, which
            // names the subnet by its own txid (design §13.8)
            let below = loc.anchor == pos.anchor
                && loc.nibbles == pos.nibbles + 1
                && (loc.subnet == pos.subnet || Some(rec.txid) == pos.subnet);
            if patron != me || !below || ended.contains(&node) {
                continue;
            }
            if let Some(i) = loc.slot() {
                taken.insert(i);
            }
        }
        (0..10).find(|i| !taken.contains(i)).ok_or_else(|| {
            Abort::PatronRefused(
                "all ten subordinate slots are taken (design §3.1: at most f = 10 subordinates)"
                    .into(),
            )
        })
    }

    /// **As the party being adopted: check the body before signing it**
    /// (`light-client-requirements.md` §7; design §6.1.1, where every
    /// adoption carries both signatures).
    ///
    /// [`Client::sign_body`] signs what it is handed, which is right for
    /// a caller that built the body itself. **A body that arrived is a
    /// different object**: a signature on it puts this key under a
    /// patron, in a series, at an address this device did not choose, so
    /// a device that signed whatever reached it would be adopted by
    /// anybody able to reach it. Every field is checked against what this
    /// client holds, and a body that fails is refused with its reason —
    /// which the person is shown and the proposer is told, since a
    /// refusal here is an ordinary answer and not a fault.
    ///
    /// **What is checked, and why each is checkable here.** The node is
    /// me, the patron is somebody I have met, and the evidence is the
    /// presence record of *that* meeting, which my own archive holds. The
    /// position extends the patron's own by one nibble in the patron's
    /// anchor — their locator came with their intent (§14.3.2) and is
    /// signed, so the claim is theirs and not the body's. The series is
    /// one this key has never occupied (`light-client-requirements.md`
    /// §2), the counter opens at zero (`wire-format.md` §2.3), and the
    /// back-pointers the body carries for me are my archive's own, since
    /// a signature over somebody else's is a commitment to a position in
    /// the DAG I am not at.
    pub fn review_and_sign_adoption(&mut self, body: &[u8]) -> Result<Vec<u8>, Abort> {
        use rhtn_codec::cbor::{Item, map_get, parse_all};
        let me = self.keyhash();
        let item = parse_all(body).map_err(|e| Abort::Malformed(format!("adoption body: {e}")))?;
        rhtn_codec::schema::check_body_of_type(body, &item, TYPE_ADOPTION)
            .map_err(|e| Abort::Malformed(format!("adoption body: {}", e.0)))?;
        let Item::Map(m) = &item else {
            return Err(Abort::Malformed("adoption body is not a map".into()));
        };
        let bytes_at = |k: u64| match map_get(m, k) {
            Some(Item::Bytes(r)) => Some(body[r.clone()].to_vec()),
            _ => None,
        };
        let key_at = |k: u64| -> Option<Keyhash> { bytes_at(k)?.try_into().ok() };
        let refused = |why: String| Abort::NodeRefused(why);

        // 1. an adoption of me, by somebody else
        let node = key_at(1).ok_or_else(|| Abort::Malformed("field 1".into()))?;
        if node != me {
            return Err(refused("an adoption of another party".into()));
        }
        let patron = key_at(2).ok_or_else(|| Abort::Malformed("field 2".into()))?;
        if patron == me {
            return Err(refused("a patron is somebody else".into()));
        }

        // 2. **the evidence is a meeting of these two that I hold.** A
        // recovery or a transfer is the subject's own to assemble
        // (design §9.0, §6.2), so a body carrying one did not come from
        // this path and is not signed on it
        if rhtn_codec::cbor::value_slice(body, 6).is_some() {
            return Err(refused(
                "a recovery adoption is assembled by its subject, not proposed to it".into(),
            ));
        }
        if rhtn_codec::cbor::value_slice(body, 9).is_some() {
            return Err(refused(
                "a transfer carries the former patron's statement; this path adopts on a meeting"
                    .into(),
            ));
        }
        let pop: Txid = bytes_at(8)
            .ok_or_else(|| refused("no presence evidence (field 8)".into()))?
            .try_into()
            .map_err(|_| Abort::Malformed("field 8 is not a txid".into()))?;
        let held = self
            .store
            .records
            .get(&pop)
            .ok_or_else(|| refused(format!("no record {} is held here", hex8(&pop))))?;
        let meeting = Record::parse(held).map_err(Abort::Record)?;
        if meeting.tx_type != TYPE_PRESENCE {
            return Err(refused("the evidence is not a presence record".into()));
        }
        // **as a set, not a sequence**: field 3 has no canonical order
        // and carries the proposer's (`wire-format.md` §4.5), so which of
        // the two is first says nothing about who they are
        let mut met = meeting.participants();
        met.sort();
        if met != participants(me, patron).to_vec() {
            return Err(refused("the evidence is a meeting of other parties".into()));
        }

        // 3. the position is the patron's to issue: one nibble below
        // their own, in their own anchor
        let r3 = rhtn_codec::cbor::value_slice(body, 3)
            .ok_or_else(|| Abort::Malformed("field 3".into()))?;
        let loc = Locator::decode(&body[r3]).map_err(Abort::Malformed)?;
        let theirs = self
            .store
            .contacts
            .get(&patron)
            .and_then(|c| c.locator.as_deref())
            .ok_or_else(|| {
                refused("no position of the patron's own is held, so nothing says this address is theirs to issue".into())
            })?;
        let theirs = rhtn_archive::locator::SignedLocator::parse(theirs)
            .map_err(Abort::Malformed)?
            .locator;
        // **or one hop below the patron's own key** (`wire-format.md` §2.1,
        // design §13.8): a patron that holds a position in somebody else's
        // subnet may still root one under its own key — "which tree is the
        // patron's to say" — and the locator this side holds for it is its
        // position elsewhere, which says nothing against that. The subnet
        // such an address names is the patron's own, which nobody but the
        // patron can vouch for (design §1.1). On the bench the patron MRK,
        // adopted under GXV an hour before, chose a new subnet under its own
        // key and GXV refused the body against the position it held for MRK
        // in GXV's own subnet (run 9613069-a2-nodeless-reverse-1,
        // 2026-10-24).
        let under_own_key = loc.anchor == patron && loc.nibbles == 1 && loc.path.len() == 1;
        if !under_own_key && (loc.anchor != theirs.anchor || loc.nibbles != theirs.nibbles + 1) {
            return Err(refused(format!(
                "the address is not one hop below the patron's own position in {}",
                hex8(&theirs.anchor)
            )));
        }
        // and in the patron's subnet: the one its own position names, or
        // none where the patron is a lone root adopting for the first
        // time, this record's own txid being the id (`wire-format.md`
        // §2.3, §4.1; design §13.8)
        if !under_own_key && loc.subnet != theirs.subnet {
            return Err(refused(match theirs.subnet {
                Some(s) => format!(
                    "the address is in a subnet other than the patron's, {}",
                    hex8(&s)
                ),
                None => "the patron is a lone root, whose first adoption names no subnet".into(),
            }));
        }
        // and the hops above the last are the patron's own, nibble by
        // nibble (`wire-format.md` §2.1: packed four bits, high first)
        let nibble = |path: &[u8], i: u64| -> Option<u8> {
            let byte = *path.get(usize::try_from(i / 2).ok()?)?;
            Some(if i.is_multiple_of(2) {
                byte >> 4
            } else {
                byte & 0x0f
            })
        };
        for i in 0..(if under_own_key { 0 } else { theirs.nibbles }) {
            if nibble(&loc.path, i) != nibble(&theirs.path, i) {
                return Err(refused(
                    "the address does not extend the patron's own path".into(),
                ));
            }
        }

        // 3b. the scope the body says I bring is one I root
        // (`wire-format.md` §4.1 field 10): my own claim, copied by the
        // patron, and not a scope of anybody else's
        if let Some(carried) = key_at(10)
            && !self.scopes_rooted().contains(&carried)
        {
            return Err(refused(format!(
                "the body says this key brings scope {}, which it does not root",
                hex8(&carried)
            )));
        }

        // 4. a series this key has never occupied, opening at zero
        if loc.seqno.counter != 0 {
            return Err(refused("an adoption opens a series at counter zero".into()));
        }
        if self.archive.series_occupied().contains(&loc.seqno.series) {
            return Err(refused(format!(
                "series {} is one this key has occupied",
                loc.seqno.series
            )));
        }

        // 5. my own back-pointers, and a timestamp my clock can hold
        let mine = self.archive.next_back_pointers();
        // **the first list is the node's** (`wire-format.md` §3.1: an
        // adoption's signer order is node, then patron), and this body
        // adopts me
        let Some(Item::Array(lists)) = map_get(m, 0) else {
            return Err(Abort::Malformed("key 0".into()));
        };
        let Some(Item::Array(ours)) = lists.first() else {
            return Err(Abort::Malformed(
                "key 0 carries no list for the node".into(),
            ));
        };
        let carried: Vec<Txid> = ours
            .iter()
            .filter_map(|it| match it {
                Item::Bytes(r) => <Txid>::try_from(&body[r.clone()]).ok(),
                _ => None,
            })
            .collect();
        if carried != mine {
            return Err(refused(
                "the body carries back-pointers that are not this archive's".into(),
            ));
        }
        let when = match map_get(m, 4) {
            Some(it) => {
                rhtn_codec::cbor::as_uint(it).ok_or_else(|| Abort::Malformed("field 4".into()))?
            }
            None => return Err(Abort::Malformed("field 4".into())),
        };
        if when < meeting.effective {
            return Err(refused(
                "the adoption is timestamped before the meeting it rests on".into(),
            ));
        }
        let now = self.now_s();
        if when > now.saturating_add(self.cfg.clock_tolerance_s) {
            return Err(refused(format!(
                "the adoption is timestamped {} s ahead of this clock",
                when - now
            )));
        }
        verify::adoption_evidence(&self.known, body)
            .map_err(|e| refused(format!("the evidence does not verify: {e}")))?;

        let entries = self.signer()?.sign_entries(aad::ENVELOPE, body);
        tracing::info!(
            target: "cer",
            role = "node",
            patron = %id8(&patron),
            series = loc.seqno.series,
            "cer.adoption.signed"
        );
        Ok(entries)
    }

    /// Sign a body I proposed or was shown, as its subject or its patron.
    pub fn sign_body(&self, body: &[u8]) -> Result<Vec<u8>, Abort> {
        Ok(self.signer()?.sign_entries(aad::ENVELOPE, body))
    }

    /// Take a finalized adoption I signed: appended, kept, and — where it
    /// is an adoption *of me* — folded into where I sit.
    pub fn take_adoption(&mut self, envelope: &[u8]) -> Result<Txid, Abort> {
        let rec = Record::parse(envelope).map_err(Abort::Record)?;
        verify::envelope(&self.known, envelope).map_err(|e| Abort::Record(e.to_string()))?;
        let t = rec.txid;
        self.archive.append(rec).map_err(Abort::Record)?;
        self.store.records.insert(t, envelope.to_vec());
        self.adopt_own_positions();
        self.outbox.push(Msg::Record(envelope.to_vec()));
        Ok(t)
    }

    /// **As either party: the peering body between this client and
    /// `other`** (`wire-format.md` §4.4; design §6.3).
    ///
    /// **A light client peers by locator.** Field 3 and field 4 take "a
    /// network point where that party runs an instance, its locator
    /// otherwise" [author, 2026-09-29], and a phone runs none: this
    /// client's own position goes in its field and the position it holds
    /// for `other` in theirs, each a snapshot at signing that the record
    /// is never reissued for.
    ///
    /// **The presence record is required unconditionally** (design §6.3):
    /// peers share no prior relationship to draw on, so a meeting is the
    /// whole of what a peering rests on. `other_back` is the other
    /// party's own back-pointers, which only they can supply.
    ///
    /// A peering carries none of the subnet's authority (design §15.1),
    /// so nothing here consults a patron and nothing asks one.
    pub fn propose_peering(
        &self,
        other: Keyhash,
        pop: Txid,
        other_back: &[Txid],
        commitment: Option<u64>,
    ) -> Result<Vec<u8>, Abort> {
        let me = self.keyhash();
        if other == me {
            return Err(Abort::Malformed("a peering is with somebody else".into()));
        }
        // the meeting it rests on, held here and of these two
        self.presence_of(&me, &other, &pop)?;
        let mine = self.acting_position();
        let theirs = self.position_held_for(&other)?;
        let back = self.archive.next_back_pointers();
        let (a, b, a_at, b_at, back_a, back_b) = if me <= other {
            (me, other, mine, theirs, back.clone(), other_back.to_vec())
        } else {
            (other, me, theirs, mine, other_back.to_vec(), back.clone())
        };
        let at = |l: &Locator| {
            let mut out = Vec::new();
            l.emit(&mut out);
            rhtn_archive::tx::PeerAt::Position(out)
        };
        Ok(rhtn_archive::tx::peering_body(&rhtn_archive::tx::Peering {
            back: [&back_a, &back_b],
            a,
            b,
            a_at: at(&a_at),
            b_at: at(&b_at),
            timestamp: self.now_s(),
            commitment,
            pop,
        }))
    }

    /// **As the other party: check the peering body, then sign it.**
    ///
    /// What this client can check is what it holds: that the record is
    /// between it and somebody else, that the meeting it rests on is one
    /// of this pair held here, that the address written for *this* party
    /// is this party's own position — nobody else writes where I am — and
    /// that my back-pointers are my archive's next and the timestamp is
    /// one my clock can hold. **The other party's address is theirs to
    /// state** (§4.4's snapshot), so a locator of theirs older than the
    /// one held here is not a fault.
    pub fn review_and_sign_peering(&mut self, body: &[u8]) -> Result<Vec<u8>, Abort> {
        use rhtn_codec::cbor::{Item, map_get, parse_all};
        let me = self.keyhash();
        let item = parse_all(body).map_err(|e| Abort::Malformed(format!("peering body: {e}")))?;
        rhtn_codec::schema::check_body_of_type(body, &item, rhtn_archive::tx::TYPE_PEERING)
            .map_err(|e| Abort::Malformed(format!("peering body: {}", e.0)))?;
        let Item::Map(m) = &item else {
            return Err(Abort::Malformed("peering body is not a map".into()));
        };
        let key_at = |k: u64| -> Option<Keyhash> {
            match map_get(m, k) {
                Some(Item::Bytes(r)) => body[r.clone()].to_vec().try_into().ok(),
                _ => None,
            }
        };
        let refused = |why: String| Abort::NodeRefused(why);

        // 1. a peering of mine, with somebody else
        let a = key_at(1).ok_or_else(|| Abort::Malformed("field 1".into()))?;
        let b = key_at(2).ok_or_else(|| Abort::Malformed("field 2".into()))?;
        let (mine_at, other) = if a == me {
            (3u64, b)
        } else if b == me {
            (4u64, a)
        } else {
            return Err(refused("a peering between other parties".into()));
        };

        // 2. the meeting it rests on, held here and of these two
        let pop: Txid =
            key_at(8).ok_or_else(|| refused("no presence evidence (field 8)".into()))?;
        self.presence_of(&me, &other, &pop)?;

        // 3. where it says I am is where I am: my own position, which is
        //    mine to state and nobody else's
        let r = rhtn_codec::cbor::value_slice(body, mine_at)
            .ok_or_else(|| Abort::Malformed(format!("field {mine_at}")))?;
        let stated = Locator::decode(&body[r]).map_err(|_| {
            refused("the address written for this key is not a locator of its own".into())
        })?;
        let held = self.acting_position();
        if (
            stated.anchor,
            stated.path.as_slice(),
            stated.nibbles,
            stated.subnet,
        ) != (held.anchor, held.path.as_slice(), held.nibbles, held.subnet)
        {
            return Err(refused(
                "the position written for this key is not the one it holds".into(),
            ));
        }

        // 4. my back-pointers, and a timestamp my clock can hold
        let mine = self.archive.next_back_pointers();
        // **key 0 holds one list per signer, in signer order**
        // (`wire-format.md` §3.1), which for a peering is A then B — so
        // the list that is mine is the one at my own side
        let Some(Item::Array(lists)) = map_get(m, 0) else {
            return Err(Abort::Malformed("key 0".into()));
        };
        let whose = usize::from(mine_at == 4);
        let Some(Item::Array(ours)) = lists.get(whose) else {
            return Err(Abort::Malformed(
                "key 0 carries no list for this key".into(),
            ));
        };
        let carried: Vec<Txid> = ours
            .iter()
            .filter_map(|it| match it {
                Item::Bytes(r) => <Txid>::try_from(&body[r.clone()]).ok(),
                _ => None,
            })
            .collect();
        if carried != mine {
            return Err(refused(
                "the back-pointers for this key are not this archive's next".into(),
            ));
        }
        let when = map_get(m, 5)
            .and_then(rhtn_codec::cbor::as_uint)
            .ok_or_else(|| Abort::Malformed("field 5".into()))?;
        let now = self.now_s();
        if when > now.saturating_add(self.cfg.clock_tolerance_s) {
            return Err(refused(format!(
                "the peering is timed {} seconds ahead of this clock",
                when - now
            )));
        }
        self.sign_body(body)
    }

    /// Take a finalised peering: held, and handed to the serving node to
    /// flood (`wire-format.md` §10.1). A peering is no part of this
    /// client's position, so nothing refolds.
    pub fn take_peering(&mut self, envelope: &[u8]) -> Result<Txid, Abort> {
        let rec = Record::parse(envelope).map_err(Abort::Record)?;
        if rec.tx_type != rhtn_archive::tx::TYPE_PEERING {
            return Err(Abort::Malformed("not a peering".into()));
        }
        verify::envelope(&self.known, envelope).map_err(|e| Abort::Record(e.to_string()))?;
        let t = rec.txid;
        self.archive.append(rec).map_err(Abort::Record)?;
        self.store.records.insert(t, envelope.to_vec());
        self.outbox.push(Msg::Record(envelope.to_vec()));
        Ok(t)
    }

    /// The presence record `pop` as this client holds it, refused where it
    /// is not held here or is not a meeting of `one` and `two`.
    fn presence_of(&self, one: &Keyhash, two: &Keyhash, pop: &Txid) -> Result<(), Abort> {
        let held =
            self.store.records.get(pop).ok_or_else(|| {
                Abort::NodeRefused(format!("no record {} is held here", hex8(pop)))
            })?;
        let rec = Record::parse(held).map_err(Abort::Record)?;
        if rec.tx_type != TYPE_PRESENCE {
            return Err(Abort::NodeRefused(
                "the evidence is not a presence record".into(),
            ));
        }
        let mut met = rec.participants();
        met.sort();
        if met != participants(*one, *two).to_vec() {
            return Err(Abort::NodeRefused(
                "the evidence is a meeting of other parties".into(),
            ));
        }
        Ok(())
    }

    /// The position this client holds for `who`, from the locator it was
    /// handed (§2.3): what a peering writes in that party's field.
    fn position_held_for(&self, who: &Keyhash) -> Result<Locator, Abort> {
        let sl = self
            .store
            .contacts
            .get(who)
            .and_then(|c| c.locator.as_deref())
            .ok_or_else(|| {
                Abort::NodeRefused(format!(
                    "no position is held for {}, so nothing says where to reach them",
                    hex8(who)
                ))
            })?;
        Ok(rhtn_archive::locator::SignedLocator::parse(sl)
            .map_err(Abort::Malformed)?
            .locator)
    }

    /// **Candidate providers ordered by how concentrated each already is
    /// inside this client's own horizon, least first**
    /// (`light-client-requirements.md` §6; PRD-11): each candidate's ASN
    /// with how many parties in the horizon are already seen peering from
    /// inside it. Ties keep the caller's order, so a list arrives back in
    /// a stable shape.
    ///
    /// **Nothing is hidden and nothing is scored.** Every candidate comes
    /// back, with its count beside it, because the screen marks the
    /// crowded rather than withholding them — and because what counts as
    /// crowded is a judgement this client has no figure for. The ordering
    /// speaks to one operator's own redundancy and not to the network's:
    /// visibility does not compose (design §16.2.1, §16.3.1).
    #[must_use]
    pub fn providers_by_concentration(&self, candidates: &[u64]) -> Vec<(u64, usize)> {
        let seen = self.horizon.asns_seen();
        let mut out: Vec<(usize, u64, usize)> = candidates
            .iter()
            .enumerate()
            .map(|(i, asn)| (i, *asn, seen.get(asn).map_or(0, |s| s.len())))
            .collect();
        out.sort_by_key(|(i, _, n)| (*n, *i));
        out.into_iter().map(|(_, asn, n)| (asn, n)).collect()
    }

    /// Write this client's own state where it can be read back: its
    /// archive, and the store beside it.
    ///
    /// **The identity is not written here.** It is minted by `rhtn keys`
    /// and read at start; a client that wrote its own signing key each
    /// time it saved would put the key wherever the store went, which is
    /// the aggregation design §13.7.1 spends its length avoiding.
    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        self.archive.save(dir)?;
        self.store.save(dir)?;
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join("durable"), self.durable())
    }

    /// Start from what is at `dir` under the identity given, or from
    /// nothing where there is nothing there.
    ///
    /// **The positions are re-derived, never loaded.** Where this client
    /// sits is a fold over its own archive ([`Client::adopt_own_positions`]),
    /// so a restored archive reaches the same answer and a stored position
    /// could disagree with the records that produced it.
    pub fn at(
        dir: &std::path::Path,
        id: SigningIdentity,
        known: Vec<Identity>,
        cfg: Config,
        device: Device,
    ) -> std::io::Result<Client> {
        let kh = id.public.keyhash;
        let mut c = Client::new(id, known, cfg, device);
        // the durable blob carries what the two directories do and the
        // payload and horizon besides; where it exists it is the state,
        // and a blob that does not open is an error and not a fresh start
        match std::fs::read(dir.join("durable")) {
            Ok(b) => {
                c.restore_durable(&b).map_err(std::io::Error::other)?;
                return Ok(c);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        c.archive = Archive::load(dir, kh)?;
        c.store = ClientStore::load(dir)?;
        c.adopt_own_positions();
        c.sync_known_from_contacts();
        // **derived state is derived at every start** [author, 2026-10-11:
        // a client's behaviour must not depend on what this session did
        // first]: whom this client has met is a fold over its records,
        // and until now it was folded only at a finalize — so a restarted
        // client had met nobody until its next ceremony ended, nominated
        // nobody, and recognised none of a bundle it had every right to
        c.refresh_acquaintance();
        Ok(c)
    }

    /// Everything this device keeps for its next start (`crate::durable`):
    /// the archive's records, the store, the payload state, the horizon's
    /// records, snapshot and delegations, and the provider credential.
    /// The seed is not here: it is the platform's key storage's, and the
    /// shell supplies it at every start.
    pub fn durable(&self) -> Vec<u8> {
        use rhtn_codec::encode::*;
        let mut out = Vec::new();
        emit_array_head(&mut out, 7);
        let records: Vec<&Vec<u8>> = self.archive.records().map(|r| &r.bytes).collect();
        emit_array_head(&mut out, records.len());
        for r in records {
            emit_bstr(&mut out, r);
        }
        emit_bstr(&mut out, &self.store.encode());
        emit_bstr(&mut out, &self.payload.encode());
        // **the seen fact, not the act** (`infra-client-requirements.md`
        // §4.3, design §15.1.1 [author, 2026-09-23]): a client is no party
        // to another's adoption, so what it keeps of one is that it took it
        // and the shape it produced.  Written as `[txid, effective]`; a
        // blob written before this carried the bodies, and is read.
        let seen = self.horizon.seen();
        emit_array_head(&mut out, seen.len());
        for (txid, effective) in seen {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, &txid);
            emit_uint(&mut out, effective);
        }
        emit_bstr(&mut out, &self.horizon.materialise().encode());
        emit_bstr(&mut out, &self.horizon.delegations_held());
        // the holder: whose state this is, so a blob is refused by any other
        // client, a device's material and sessions being its own; and what
        // it holds for its provider
        emit_array_head(&mut out, 2);
        emit_bstr(&mut out, &self.public.keyhash);
        crate::durable::emit_opt_bstr(&mut out, self.provider_credential.as_deref());
        out
    }

    /// Start from what [`Client::durable`] wrote, whole or not at all: a
    /// blob that does not open leaves this client as it was and says so,
    /// because a client that restored half its state would hold sessions
    /// it cannot advance.  Positions are re-derived from the archive, and
    /// the horizon wakes from its snapshot, replaying where the snapshot
    /// cannot account for the records held.
    pub fn restore_durable(&mut self, b: &[u8]) -> Result<Restored, String> {
        use crate::durable::*;
        let (_, f) = parse_array(b, 7).ok_or("not a durable state")?;
        // bound to an identity and a device before anything is read: a
        // client restoring another's state, or another device's, would
        // hold sessions and material that are not its own
        // **the preceding format is read, not refused** (OPS-009): field 6
        // carried the provider credential alone before the holder was named
        // beside it, and a client that refused its own last state would lose
        // the archive, the sessions and the obligations it wrote.  Where the
        // holder is named it is checked; where it is not, the device and the
        // archive's own signer rule are what bind the state to this client.
        let holder: Option<[u8; 32]> = match &f[6] {
            rhtn_codec::cbor::Item::Array(a) if a.len() == 2 => {
                let parts = array(&f[6]).ok_or("holder")?;
                Some(fixed::<32>(b, &parts[0]).ok_or("owner")?)
            }
            _ => None,
        };
        if let Some(owner) = holder
            && owner != self.public.keyhash
        {
            return Err("this state belongs to another identity".into());
        }
        let recs: Vec<Vec<u8>> = array(&f[0])
            .ok_or("records")?
            .iter()
            .map(|x| bytes(b, x))
            .collect::<Option<_>>()
            .ok_or("records")?;
        let store = ClientStore::decode(&bytes(b, &f[1]).ok_or("store")?).ok_or("store")?;
        let payload =
            PayloadState::decode(self.cfg.payload.clone(), &bytes(b, &f[2]).ok_or("payload")?)
                .ok_or("payload")?;
        // each entry is a seen fact, or a whole record where the blob was
        // written before the fact replaced the act
        enum Held {
            Fact(Txid, u64),
            Act(Vec<u8>),
        }
        let held: Vec<Held> = array(&f[3])
            .ok_or("horizon")?
            .iter()
            .map(|x| match x {
                rhtn_codec::cbor::Item::Array(_) => {
                    let pair = array(x)?;
                    if pair.len() != 2 {
                        return None;
                    }
                    Some(Held::Fact(fixed::<32>(b, &pair[0])?, uint(&pair[1])?))
                }
                _ => bytes(b, x).map(Held::Act),
            })
            .collect::<Option<_>>()
            .ok_or("horizon")?;
        let snap = rhtn_archive::topology::Snapshot::decode(&bytes(b, &f[4]).ok_or("snapshot")?)
            .ok_or("snapshot")?;
        let delegations = bytes(b, &f[5]).ok_or("delegations")?;
        let provider = match &f[6] {
            rhtn_codec::cbor::Item::Array(a) if a.len() == 2 => {
                let parts = array(&f[6]).ok_or("holder")?;
                optional(&parts[1], |it| bytes(b, it)).ok_or("provider")?
            }
            // the preceding shapes: a byte string, or nothing
            rhtn_codec::cbor::Item::Bytes(_) => Some(bytes(b, &f[6]).ok_or("provider")?),
            _ => None,
        };
        if payload.device != self.payload.device {
            return Err("this state belongs to another device".into());
        }
        // everything is built into temporaries before any of it lands:
        // the last fallible step is behind the first replacement, so a
        // refusal leaves this client exactly as it was
        let kh = self.public.keyhash;
        let mut archive = Archive::new(kh);
        let mut parsed: Vec<Record> = recs
            .iter()
            .map(|r| Record::parse(r))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("record: {e}"))?;
        parsed.sort_by_key(|r| (r.effective, r.txid));
        let mut records = 0;
        for rec in parsed {
            archive.append(rec).map_err(|e| format!("record: {e}"))?;
            records += 1;
        }
        let mut horizon = Horizon::new(kh);
        for h in held {
            match h {
                Held::Fact(txid, effective) => horizon.restore_seen(txid, effective),
                Held::Act(bytes) => {
                    horizon.restore_record(bytes);
                }
            }
        }
        let woke = horizon.wake(Some(&snap), &self.known);
        horizon
            .restore_delegations(&delegations)
            .ok_or("delegations")?;
        self.archive = archive;
        self.store = store;
        self.payload = payload;
        self.horizon = horizon;
        self.provider_credential = provider;
        self.positions.clear();
        self.root_subnet = None;
        self.scopes.clear();
        self.root_carried = false;
        self.adopt_own_positions();
        // derived state is derived at every start (see `at`)
        self.sync_known_from_contacts();
        self.refresh_acquaintance();
        Ok(Restored {
            records,
            sessions: self.payload.sessions.ratchets.len(),
            horizon: woke,
        })
    }

    /// Take a backup's contents into a client that holds nothing yet: the
    /// records appended in order, the store and the provider credential
    /// installed, positions re-derived.  **Refused on a client with an
    /// archive**: what a client does with contents that would replace an
    /// intact identity is a decision, not a side effect of opening a file.
    pub fn install(&mut self, mut contents: backup::Contents) -> Result<usize, String> {
        if !self.archive.is_empty() {
            return Err("this client already holds an archive".into());
        }
        // **decryption establishes which key this reader holds, not which
        // identity wrote the blob.**  Every backup names its subject in its
        // own right, and one that names another is refused however it
        // opened.
        match contents.owner {
            Some(owner) if owner == self.public.keyhash => {}
            Some(_) => return Err("this backup belongs to another identity".into()),
            None => return Err("this backup names no owner".into()),
        }
        let mut parsed: Vec<Record> = contents
            .records
            .iter()
            .map(|r| Record::parse(r))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("record: {e}"))?;
        parsed.sort_by_key(|r| (r.effective, r.txid));
        let mut archive = Archive::new(self.public.keyhash);
        for rec in parsed {
            archive.append(rec).map_err(|e| format!("record: {e}"))?;
        }
        let n = archive.len();
        self.archive = archive;
        // taken out rather than moved out: the contents wipe their seeds
        // on drop, and a type that drops cannot be moved out of piecemeal
        self.store = std::mem::take(&mut contents.store);
        self.provider_credential = contents.provider.take();
        self.positions.clear();
        self.root_subnet = None;
        self.scopes.clear();
        self.root_carried = false;
        self.adopt_own_positions();
        Ok(n)
    }

    /// Everything a device loss would take away, enveloped under the key
    /// the identity derives (design §13.7.1).
    ///
    /// **The identity stays out.** The store writes no key and neither
    /// does this: what opens the blob is what a holder puts on a device by
    /// hand (`light-client-requirements.md` §2), and a device holding no
    /// seed can write no backup — [`backup::Failure::NoKey`].
    pub fn export(&self) -> Result<Vec<u8>, backup::Failure> {
        let root = self
            .signer()
            .map_err(|_| backup::Failure::NoKey)?
            .identity_root();
        let contents = backup::Contents {
            owner: Some(self.public.keyhash),
            records: self.archive.records().map(|r| r.bytes.clone()).collect(),
            store: self.store.clone(),
            provider: self.provider_credential.clone(),
        };
        backup::export(&contents, &backup::Wrap::identity(), root)
    }

    /// **What a peer holds for this client** (design §6.3): the records
    /// and the view kept beside them — contacts, held locators, profiles,
    /// the own-capture rows — and **not the sealed captures of others, not
    /// the provider credential, never the identity**.  Under the same key
    /// as [`Client::export`], so a peer holds ciphertext it cannot read.
    pub fn export_for_peer(&self) -> Result<Vec<u8>, backup::Failure> {
        let root = self
            .signer()
            .map_err(|_| backup::Failure::NoKey)?
            .identity_root();
        let contents = backup::Contents {
            owner: Some(self.public.keyhash),
            records: self.archive.records().map(|r| r.bytes.clone()).collect(),
            store: self.store_for_peer(),
            provider: None,
        };
        backup::export(&contents, &backup::Wrap::identity(), root)
    }

    /// The store as a peer blob carries it: without the sealed captures
    /// of others and without what this client holds for its own peers.
    fn store_for_peer(&self) -> ClientStore {
        let mut store = self.store.clone();
        store.sealed.clear();
        store.held_for.clear();
        store
    }

    /// The plaintext a peer blob would carry now, digested: what decides
    /// whether a round has anything new to deposit.
    fn peer_blob_digest(&self) -> [u8; 32] {
        let mut pre = Vec::new();
        for r in self.archive.records() {
            pre.extend_from_slice(&r.txid);
        }
        pre.extend_from_slice(&self.store_for_peer().encode());
        rhtn_codec::cose::sha256(&pre)
    }

    /// **Bring this client up to what a peer holds for it** (design §6.3,
    /// and §23.3's rule for a device that is behind): the blob opens under
    /// this identity or it is nothing; it names this identity or it is
    /// refused; then what it holds and this client does not is taken —
    /// records appended where they verify, contacts and the rest of the
    /// view inserted where absent.  **Nothing held here is replaced**: a
    /// peer's copy is older than the device that made it, and what is
    /// here is what this device knows now.  How much was taken.
    pub fn reconcile_from_peer(&mut self, blob: &[u8]) -> Result<Reconciled, String> {
        let root = self
            .signer()
            .map_err(|_| backup::Failure::NoKey.to_string())?
            .identity_root();
        let mut contents = backup::import(blob, root).map_err(|e| e.to_string())?;
        match contents.owner {
            Some(owner) if owner == self.public.keyhash => {}
            Some(_) => return Err("the blob a peer returned belongs to another identity".into()),
            None => return Err("the blob a peer returned names no owner".into()),
        }
        contents.scan(self.now_s(), self.cfg.subject.retention_seconds);
        let mut out = Reconciled::default();
        // records: the ones not held, oldest first, each verified under the
        // keys this client knows before it is appended
        let mut parsed: Vec<Record> = contents
            .records
            .iter()
            .filter_map(|r| Record::parse(r).ok())
            .filter(|r| self.archive.get(&r.txid).is_none())
            .collect();
        parsed.sort_by_key(|r| (r.effective, r.txid));
        for rec in parsed {
            if verify::envelope(&self.known, &rec.bytes).is_err() {
                out.refused += 1;
                continue;
            }
            let (t, bytes) = (rec.txid, rec.bytes.clone());
            if self.archive.append(rec).is_ok() {
                self.store.records.entry(t).or_insert(bytes);
                out.records += 1;
            } else {
                out.refused += 1;
            }
        }
        let theirs = std::mem::take(&mut contents.store);
        out.contacts += insert_absent(&mut self.store.contacts, theirs.contacts);
        out.other += insert_absent(&mut self.store.records, theirs.records);
        out.other += insert_absent(&mut self.store.own, theirs.own);
        out.other += insert_absent(&mut self.store.late, theirs.late);
        out.other += insert_absent(&mut self.store.disclosures, theirs.disclosures);
        out.other += insert_absent(&mut self.store.profiles, theirs.profiles);
        out.other += insert_absent(&mut self.store.subnet_profiles, theirs.subnet_profiles);
        out.other += insert_absent(&mut self.store.direct_hints, theirs.direct_hints);
        if out.records > 0 {
            self.positions.clear();
            self.root_subnet = None;
            self.scopes.clear();
            self.root_carried = false;
            self.adopt_own_positions();
        }
        tracing::info!(
            target: "peer",
            records = out.records,
            contacts = out.contacts,
            other = out.other,
            refused = out.refused,
            "peer.reconciled"
        );
        Ok(out)
    }

    /// **What goes to a peer on contact** (design §6.3): this client's
    /// current locator, so the peer's copy of where it stands is the
    /// present one; a request for the blob the peer holds of it; and a
    /// fresh deposit where the dataset changed since the last.  Queued in
    /// the outbox; `None` where `peer` is not one.
    pub fn greet_peer(&mut self, peer: Keyhash) -> Option<()> {
        if !self.peers().contains(&peer) {
            return None;
        }
        if let Ok(sl) = self.own_locator() {
            self.queue_to_peer(peer, payload::KIND_PEER_LOCATOR, &sl);
        }
        self.queue_to_peer(peer, payload::KIND_BACKUP_REQUEST, &[]);
        let digest = self.peer_blob_digest();
        if self.deposited.get(&peer) != Some(&digest) {
            match self.export_for_peer() {
                Ok(blob) => {
                    tracing::info!(target: "peer", peer = %id8(&peer), bytes = blob.len(), "peer.deposit");
                    self.queue_to_peer(peer, payload::KIND_BACKUP_DEPOSIT, &blob);
                    self.deposited.insert(peer, digest);
                }
                Err(e) => {
                    tracing::info!(target: "peer", peer = %id8(&peer), why = %e, "peer.deposit.none");
                }
            }
        }
        Some(())
    }

    fn queue_to_peer(&mut self, peer: Keyhash, kind: u64, bytes: &[u8]) {
        match self.send_payload(peer, kind, bytes) {
            Ok(msgs) => self.outbox.extend(msgs),
            Err(e) => {
                tracing::info!(target: "peer", peer = %id8(&peer), kind = crate::diag::kind_name(kind), why = ?e, "peer.send.refused");
            }
        }
    }

    /// Open a backup and **scan it before any of it lands**
    /// (design §13.7.1: import is where over-retention leaks). What the
    /// scan discarded is returned beside the contents rather than
    /// swallowed: a person restoring is entitled to know their backup held
    /// material their own commitment had run out on.
    ///
    /// **Nothing is merged here.** This opens and scans; what a client
    /// does with contents that would replace an intact identity is its
    /// own decision and not this function's to make quietly.
    pub fn import(
        &self,
        blob: &[u8],
    ) -> Result<(backup::Contents, backup::Discarded), backup::Failure> {
        let root = self
            .signer()
            .map_err(|_| backup::Failure::NoKey)?
            .identity_root();
        let mut contents = backup::import(blob, root)?;
        let discarded = contents.scan(self.now_s(), self.cfg.subject.retention_seconds);
        Ok((contents, discarded))
    }

    /// Leave `patron` (`wire-format.md` §4.2).
    ///
    /// **Nothing to propose and nobody to ask.** §4.2 has the old patron
    /// not sign, and design §6.2 forbids a party with authority over
    /// another from gating an action whose sole effect is to end that
    /// authority — so this mints, signs, appends and posts in one step.
    /// There is no countersignature to wait for and no refusal that can
    /// come back; that is what makes exit a right rather than a request.
    ///
    /// The sequence is the relationship's current series at the next
    /// counter, which §4.2 requires and §2.3 scopes to that series alone.
    /// Afterwards this client holds no position in the subnet it left, and
    /// the envelope is in the outbox for its serving node to flood.
    pub fn depart(&mut self, patron: Keyhash, reason: Option<u64>) -> Result<Txid, Abort> {
        let me = self.keyhash();
        let rel = crate::rotation::relationships(&self.archive)
            .into_iter()
            .find(|r| r.patron == patron)
            .ok_or(Abort::NoRelationship(patron))?;
        // **the archive shows every relationship this key ever had**, and
        // one it has already left is not one it can leave again — a second
        // departure would advance the counter over a binding that closed.
        // The position is where that is known: one address, one patron,
        // so the address the adoption gave answers it — by address and
        // not by subnet, because a root's first adoption names none
        // (design §13.8) and the position held fills it in
        let held = self
            .positions
            .values()
            .find(|p| {
                p.anchor == rel.position.anchor
                    && p.path == rel.position.path
                    && p.nibbles == rel.position.nibbles
            })
            .ok_or(Abort::NoRelationship(patron))?;
        let counter = held
            .seqno
            .counter
            .checked_add(1)
            .ok_or(Abort::CounterExhausted)?;
        let back = self.archive.next_back_pointers();
        let body = departure_body(
            &back,
            &me,
            &patron,
            Seqno {
                series: rel.series(),
                counter,
            },
            self.now_s(),
            reason,
        );
        let env = envelope(TYPE_DEPARTURE, &body, &[self.signer()?]);
        let rec = Record::parse(&env).map_err(Abort::Record)?;
        let t = rec.txid;
        self.archive.append(rec).map_err(Abort::Record)?;
        self.store.records.insert(t, env.clone());
        self.adopt_own_positions();
        self.outbox.push(Msg::Record(env));
        Ok(t)
    }

    /// What this client has made and not yet handed up, taken away.
    /// What the courier is owed: everything the outbox holds **except**
    /// the counterparty's carriages, which no node carries.
    pub fn outbox(&mut self) -> Vec<Msg> {
        let (carriages, rest): (Vec<Msg>, Vec<Msg>) = std::mem::take(&mut self.outbox)
            .into_iter()
            .partition(|m| matches!(m, Msg::Carriage(_)));
        self.outbox = carriages;
        rest
    }

    /// What the bearer is owed: the counterparty's carriages, drained.
    ///
    /// **Separate from [`Client::outbox`] because the two go different
    /// ways**, and a carriage handed to a courier would be refused by a
    /// node that knows no such thing. A caller that drains one and not the
    /// other leaves the ceremony waiting, so every path that can produce a
    /// carriage drains this.
    pub fn carriages(&mut self) -> Vec<Vec<u8>> {
        let (carriages, rest): (Vec<Msg>, Vec<Msg>) = std::mem::take(&mut self.outbox)
            .into_iter()
            .partition(|m| matches!(m, Msg::Carriage(_)));
        self.outbox = rest;
        carriages
            .into_iter()
            .filter_map(|m| match m {
                Msg::Carriage(b) => Some(b),
                _ => None,
            })
            .collect()
    }

    /// Take a conversation step off the bearer: open it, and hand the
    /// kind and bytes to the same path a payload-borne step takes.
    ///
    /// **The anchor is the key.** A carriage that does not open under this
    /// ceremony's session key is refused as [`Abort::Unsealable`], which
    /// is all a receiver can say about bytes from a party without the key;
    /// everything the step itself claims is checked by `converse_in` on
    /// the plaintext, exactly as for a step that arrived over the network.
    pub fn take_conversation_carriage(
        &mut self,
        bytes: &[u8],
    ) -> Result<crate::sequence::Conversed, Abort> {
        let from = self.active.as_ref().ok_or(Abort::NotActive)?.peer()?;
        let plain = self.open(bytes)?;
        let (kind, inner) = crate::payload::unwrap(&plain).map_err(Abort::Malformed)?;
        Ok(self.converse_in(from, kind, &inner))
    }

    /// Where this client sits in the subnet `subnet` names (design §13.8):
    /// the position it was adopted into there, or its own self-anchored
    /// one where that is the subnet under its own key.
    #[must_use]
    pub fn position_in(&self, subnet: &Txid) -> Option<Locator> {
        if let Some(p) = self.positions.get(subnet) {
            return Some(p.clone());
        }
        // the subnet under its own key, while that scope is not carried
        // under a patron (`wire-format.md` §4.1 field 10)
        (self.root_subnet == Some(*subnet) && !self.root_carried).then(|| self.own_position())
    }

    /// This client's position under its own key.
    ///
    /// **A party with no ancestor names itself** (`wire-format.md` §2.1):
    /// the empty path, and the subnet its first adoption as a lone root
    /// named by its txid, or none while it has adopted nobody.
    #[must_use]
    pub fn own_position(&self) -> Locator {
        Locator::root_in(
            self.keyhash(),
            Seqno {
                series: 1,
                counter: 0,
            },
            self.root_subnet,
        )
    }

    /// Where this client acts when nothing names a subnet: its position
    /// in the first it was adopted into, or its own while it has no
    /// patron.
    #[must_use]
    pub fn acting_position(&self) -> Locator {
        self.positions
            .values()
            .next()
            .cloned()
            .unwrap_or_else(|| self.own_position())
    }

    /// The subnet this client acts in when nothing names one: the first it
    /// was adopted into, else the one under its own key, else none.
    #[must_use]
    pub fn subnet(&self) -> Option<Txid> {
        self.positions
            .keys()
            .next()
            .copied()
            .or(self.root_subnet.filter(|_| !self.root_carried))
    }

    /// Every subnet this client has a position in: the one under its own
    /// key first where it names one, then each it was adopted into, and
    /// each scope it now roots after departing the subnet it was in.
    #[must_use]
    pub fn subnets(&self) -> Vec<Txid> {
        let mut out: Vec<Txid> = self
            .root_subnet
            .filter(|_| !self.root_carried)
            .into_iter()
            .collect();
        out.extend(self.positions.keys().copied());
        out
    }

    /// **What this key is in one subnet** (`Robot/light-client-screens.md`,
    /// the crowns): whether it is the root there, and how many
    /// subordinates it still holds there.
    ///
    /// **All three are local records** [author, 2026-10-28], and each is
    /// read off the one that settles it:
    ///
    /// - **The root is the patron named in the subnet's own anchoring
    ///   record.** A subnet's id *is* the txid of the adoption that first
    ///   gave its root a subordinate (design §13.8), so the question is
    ///   whether this key is the patron in the record that txid names —
    ///   which a root holds, having signed it.
    /// - **An infra node is one this key published an endpoint record
    ///   for, on the line it holds in that subnet.** The record names no
    ///   subnet (`wire-format.md` §7.6) and is signed on that line's
    ///   series ([`Client::endpoint_record_in`]), which is what ties it
    ///   to one. **The instance is a device of this identity, not a key
    ///   of its own** — it runs under a delegation — so the node's
    ///   endpoint record is this key's own.
    /// - **A subordinate is an adoption this key granted** into that
    ///   subnet that nothing has since ended.
    ///
    /// The three overlap and only the highest is drawn; **that
    /// precedence is the shell's**, in one place, rather than decided
    /// again here.
    pub fn subnet_standing(&self) -> Vec<SubnetStanding> {
        let me = self.keyhash();
        // the adoptions this key granted, by the subnet each placed
        // somebody in.  **This key's own archive**, because an adoption
        // it signed is one it holds whether or not anything propagated
        // it back; the table below says which of them have since ended.
        let mut granted: BTreeMap<Txid, Vec<Txid>> = BTreeMap::new();
        for rec in self.archive.records() {
            if rec.tx_type == TYPE_ADOPTION
                && rec.field_hash(2) == Some(me)
                && rec.field_hash(1) != Some(me)
                && let Some(l) = rec.locator()
            {
                // a root's first adoption names no subnet and is the
                // subnet (design §13.8)
                let subnet = l.subnet.unwrap_or(rec.txid);
                granted.entry(subnet).or_default().push(rec.txid);
            }
        }
        // **ended rather than open**, which is the safe way round for a
        // mark on a screen: a binding the propagated copy has not been
        // told about is one this key granted and nothing has ended, so
        // it still counts. Asking for `open` instead would blank the
        // crown of a patron whose own adoptions have not come back.
        let ended: std::collections::BTreeSet<Txid> = self
            .horizon
            .table
            .bindings()
            .iter()
            .filter(|b| !b.open())
            .map(|b| b.adoption)
            .collect();
        self.subnets()
            .into_iter()
            .map(|subnet| SubnetStanding {
                subnet,
                // the anchoring record itself, rather than the position
                // this key derived from it: one record, one reading
                root: self
                    .archive
                    .get(&subnet)
                    .is_some_and(|r| r.tx_type == TYPE_ADOPTION && r.field_hash(2) == Some(me)),
                operator: self
                    .line_in(Some(subnet))
                    .ok()
                    .is_some_and(|series| self.horizon.infra_on(&me, series)),
                subordinates: granted
                    .get(&subnet)
                    .map(|v| v.iter().filter(|t| !ended.contains(*t)).count())
                    .unwrap_or(0),
            })
            .collect()
    }

    /// The scopes this client roots and could bring under a patron
    /// (`wire-format.md` §4.1 field 10): the one under its own key where
    /// its first adoption named it and it is not carried already, and
    /// each it carried out of a subnet it departed.
    #[must_use]
    pub fn scopes_rooted(&self) -> Vec<Txid> {
        let mut out: Vec<Txid> = self
            .root_subnet
            .filter(|_| !self.root_carried)
            .into_iter()
            .collect();
        out.extend(
            self.positions
                .iter()
                .filter(|(_, l)| l.anchor == self.keyhash() && l.nibbles == 0)
                .map(|(s, _)| *s)
                .filter(|s| Some(*s) != self.root_subnet),
        );
        out
    }

    /// This client's scope id in `subnet` (design §13.8): the txid of its
    /// first adoption there, or none where it has adopted nobody there.
    /// Latent while it sits under a patron, and the subnet's own id once
    /// it has departed.
    #[must_use]
    pub fn scope_in(&self, subnet: &Txid) -> Option<Txid> {
        self.scopes.get(subnet).copied()
    }

    /// Fold this client's own adoptions and departures into where it
    /// sits, one position per subnet.  Derived from the archive rather
    /// than kept beside it, so a restored archive reaches the same answer;
    /// a later adoption under one patron replaces the earlier, which is
    /// what a reissue is.
    ///
    /// **Read in order, because both directions occur.** A departure this
    /// client signed takes the position away — a party that left is not
    /// still at the address it left — and a re-adoption afterwards puts
    /// one back. Filtering rather than replaying would make the two
    /// cancel in whichever order they were written.
    fn adopt_own_positions(&mut self) {
        let me = self.keyhash();
        // **the relationship's own order decides, not the archive's**
        // [found 2026-10-14]: this folded `records()`, which is by txid —
        // a hash — so a departure that happened to sort before the
        // adoption it ends removed nothing and the adoption put the
        // position back. `wire-format.md` §2.3 gives one series per
        // patron relationship and every record this key signs in it the
        // next counter, so (series, counter) is the order in which the
        // relationship advances, and the last word in it is what is held.
        let mut last: BTreeMap<Keyhash, (u32, u32, Option<Locator>)> = BTreeMap::new();
        // the scope each open relationship carried under its patron
        // (`wire-format.md` §4.1 field 10), by patron
        let mut carried: BTreeMap<Keyhash, Txid> = BTreeMap::new();
        // **a scope is named by the first adoption that gave this key a
        // subordinate in a subnet** (design §13.8): the earliest by the
        // record's own time, the txid deciding a tie, per subnet — and
        // the lone root's own, whose first adoption names no subnet
        let mut first: BTreeMap<Option<Txid>, (u64, Txid)> = BTreeMap::new();
        // the subnet each relationship this key was adopted into is in,
        // for the scope a departure leaves this key the root of
        let mut subnet_under: BTreeMap<Keyhash, Txid> = BTreeMap::new();
        for rec in self.archive.records() {
            if rec.tx_type == TYPE_ADOPTION
                && rec.field_hash(2) == Some(me)
                && let Some(loc) = rec.locator()
            {
                let at = (rec.effective, rec.txid);
                let e = first.entry(loc.subnet).or_insert(at);
                if at < *e {
                    *e = at;
                }
            }
            if rec.field_hash(1) != Some(me) {
                continue;
            }
            let said = match rec.tx_type {
                TYPE_ADOPTION => Some(rec.locator().map(|l| {
                    // the root's first adoption names its subnet by its
                    // own txid; this key's position says so from here on
                    let subnet = l.subnet.unwrap_or(rec.txid);
                    if let Some(p) = rec.field_hash(2) {
                        subnet_under.insert(p, subnet);
                        match rec.carried_scope() {
                            Some(s) => {
                                carried.insert(p, s);
                            }
                            None => {
                                carried.remove(&p);
                            }
                        }
                    }
                    Locator {
                        subnet: Some(subnet),
                        ..l
                    }
                })),
                TYPE_DEPARTURE => Some(None),
                _ => None,
            };
            let (Some(said), Some(patron), Some(seq)) = (said, rec.field_hash(2), rec.seqno())
            else {
                continue;
            };
            let at = (seq.series, seq.counter);
            match last.get(&patron) {
                Some((s, c, _)) if (*s, *c) >= at => {}
                _ => {
                    last.insert(patron, (at.0, at.1, said));
                }
            }
        }
        self.root_subnet = first.get(&None).map(|(_, t)| *t);
        self.scopes = first
            .iter()
            .filter_map(|(s, (_, t))| Some(((*s)?, *t)))
            .collect();
        self.positions = last
            .iter()
            .filter_map(|(_, (_, _, loc))| loc.clone())
            .filter_map(|l| Some((l.subnet?, l)))
            .collect();
        // **a scope carried under a patron is no subnet of its own while
        // the relationship is open** (design §13.8): every position in it
        // shows the patron's subnet, and this key lists it no more
        let open_carried: BTreeSet<Txid> = last
            .iter()
            .filter(|(_, (_, _, held))| held.is_some())
            .filter_map(|(p, _)| carried.get(p).copied())
            .collect();
        self.root_carried = self.root_subnet.is_some_and(|s| open_carried.contains(&s));
        for s in &open_carried {
            self.positions.remove(s);
        }
        // **a departure roots the scope this key carried** (design §13.8,
        // §12.7.7): where it had subordinates in the subnet it left, it is
        // now the root of a subnet named by its scope id there, at the
        // self-anchor
        for (patron, (_, _, held)) in &last {
            if held.is_some() {
                continue;
            }
            // the scope it had there, and the one it brought there, both
            // stand alone again; the one under its own key is its own
            // position already
            let had = subnet_under
                .get(patron)
                .and_then(|s| self.scopes.get(s))
                .copied();
            let brought = carried.get(patron).copied();
            for scope in had.into_iter().chain(brought) {
                if Some(scope) == self.root_subnet {
                    continue;
                }
                let root = Locator::root_in(
                    me,
                    Seqno {
                        series: 1,
                        counter: 0,
                    },
                    Some(scope),
                );
                self.positions.entry(scope).or_insert(root);
            }
        }
    }

    /// The chain proving my series under `patron`, presented when asked
    /// (`wire-format.md` §4.6.1) and propagated to nobody.
    pub fn present_chain(&self, patron: &Keyhash) -> Vec<Vec<u8>> {
        self.archive
            .chain_for(patron)
            .map(|c| c.bytes())
            .unwrap_or_default()
    }

    /// As the recovering subject: seal every old line as the old key's
    /// last act (design §9.0).  After this the old key is gone.
    pub fn seal_old_lines(&mut self) -> Vec<(Keyhash, Vec<u8>)> {
        match self.rotation.as_mut() {
            Some(r) => r.seal().to_vec(),
            None => Vec::new(),
        }
    }
}

impl Client {
    fn nonce(&self) -> [u8; 16] {
        let mut n = [0u8; 16];
        self.device.random.fill(&mut n);
        n
    }

    /// Attach to `serving` (design §14.1.2): the bundle is published, the
    /// one-time pool stocked, and the reusable material of the whole
    /// population prefetched as one sweep (`light-client-requirements.md`
    /// §3).  Everything returned goes to the serving node.
    pub fn attach(&mut self, serving: Keyhash, population: &[Keyhash]) -> Vec<Msg> {
        self.payload.serving = Some(serving);
        if let Some(id) = self.known.iter().find(|k| k.keyhash == serving).cloned() {
            self.contact(&id, crate::contacts::How::Infra, true);
        }
        let now = self.now_s();
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let n = self.payload.cfg.pool_target;
        let keys = self.payload.keys.one_time_keys(n, &mut fresh);
        self.payload.pool_reported = n;
        let mut out: Vec<Msg> = self.bundle_to_publish(now).into_iter().collect();
        out.push(Msg::StockOneTime(keys));
        // the profiles this client holds for the subnet it acts in, as
        // its bundle is: at attach and whenever they change
        // (`light-client-requirements.md` §4.4)
        out.extend(self.profiles_to_publish());
        out.extend(self.sweep(population));
        out
    }

    /// This client's own profile for the subnet it acts in, and the
    /// subnet's where it set one, as publications.
    fn profiles_to_publish(&self) -> Vec<Msg> {
        let Some(subnet) = self.subnet() else {
            return Vec::new();
        };
        self.store
            .profiles
            .get(&subnet)
            .into_iter()
            .chain(self.store.subnet_profiles.get(&subnet).filter(|b| {
                rhtn_archive::profile::Profile::parse(b).is_ok_and(|p| p.subject == self.keyhash())
            }))
            .map(|b| Msg::PublishProfile(b.clone()))
            .collect()
    }

    /// Publish an alias and, if the person chooses, an image, for
    /// `subnet` — this client's own (kind 0) or the subnet's (kind 1,
    /// the root's by convention, design §12.8) — signed now and handed to
    /// the serving node (`wire-format.md` §7.11;
    /// `light-client-requirements.md` §4.4).  **Into no transaction**: the
    /// profile is held beside the records, never among them.
    pub fn publish_profile(
        &mut self,
        subnet: Txid,
        kind: u64,
        alias: &str,
        image: Option<(u64, Vec<u8>)>,
    ) -> Result<Vec<Msg>, Abort> {
        if alias.is_empty() || alias.len() > rhtn_codec::bounds::PROFILE_ALIAS_BYTES {
            return Err(Abort::Malformed("an alias is 1 to 64 bytes".into()));
        }
        if image.as_ref().is_some_and(|(f, b)| {
            !(1..=3).contains(f)
                || b.is_empty()
                || b.len() > rhtn_codec::bounds::PROFILE_IMAGE_BYTES
        }) {
            return Err(Abort::Malformed(
                "an image is a format the wire names and at most 64 KiB".into(),
            ));
        }
        if kind > 1 {
            return Err(Abort::Malformed(
                "a profile is the subject's own or the subnet's".into(),
            ));
        }
        let now = self.now_s();
        // later than whatever this client published before, so a node
        // holding the earlier one replaces it
        let last = match kind {
            0 => self.store.profiles.get(&subnet),
            _ => self.store.subnet_profiles.get(&subnet),
        }
        .and_then(|b| rhtn_archive::profile::Profile::parse(b).ok())
        .map(|p| p.published_at)
        .unwrap_or(0);
        let at = now.max(last + 1);
        let bytes = rhtn_archive::profile::Profile::build(
            self.signer()?,
            &subnet,
            kind,
            alias,
            image.as_ref().map(|(f, b)| (*f, b.as_slice())),
            at,
        );
        match kind {
            0 => self.store.profiles.insert(subnet, bytes.clone()),
            _ => self.store.subnet_profiles.insert(subnet, bytes.clone()),
        };
        Ok(vec![Msg::PublishProfile(bytes)])
    }

    /// Fetch the profile `subject` published for `subnet`, or the
    /// subnet's own where `kind` is 1 (`wire-format.md` §7.11).
    pub fn request_profile(&self, subject: Keyhash, subnet: Txid, kind: u64) -> Msg {
        Msg::ProfileRequest(
            rhtn_archive::profile::ProfileRequest {
                subject,
                subnet,
                kind,
                nonce: self.nonce(),
            }
            .encode(),
        )
    }

    /// The fetches for every contact's own profile in the subnet its
    /// locator names: what a client asks after it takes a locator
    /// (`light-client-requirements.md` §4.3).
    pub fn profile_requests(&self) -> Vec<Msg> {
        self.store
            .contacts
            .iter()
            .filter_map(|(k, c)| {
                let l = rhtn_archive::locator::SignedLocator::parse(c.locator.as_deref()?).ok()?;
                let subnet = l.locator.subnet?;
                Some(self.request_profile(*k, subnet, rhtn_archive::profile::KIND_OWN))
            })
            .collect()
    }

    /// Take a `ProfileReply`: a profile is verified under the subject it
    /// names, which must be a party this client knows, and kept beside the
    /// contact (its own) or by subnet (the subnet's) where it is later
    /// than what is held.  **Never shown unverified**, and the keyhash
    /// stands in where nothing came (`light-client-requirements.md` §4.3).
    pub fn take_profile_reply(&mut self, bytes: &[u8]) -> Result<bool, String> {
        let reply = rhtn_archive::profile::ProfileReply::decode(bytes)?;
        let Some(p) = reply.profile else {
            return Ok(false);
        };
        let profile = rhtn_archive::profile::Profile::parse(&p)?;
        profile.verify(&self.known)?;
        let taken = match profile.kind {
            rhtn_archive::profile::KIND_OWN => {
                match self.store.contacts.get_mut(&profile.subject) {
                    Some(c) => c.take_profile(&profile),
                    None => return Err("a profile of a party that is not a contact".into()),
                }
            }
            _ => {
                let later = self
                    .store
                    .subnet_profiles
                    .get(&profile.subnet)
                    .and_then(|b| rhtn_archive::profile::Profile::parse(b).ok())
                    .is_none_or(|held| held.published_at < profile.published_at);
                if later {
                    self.store.subnet_profiles.insert(profile.subnet, p);
                }
                later
            }
        };
        if taken {
            tracing::info!(target: "cer", subject = %hex8(&profile.subject), kind = profile.kind, "cer.profile");
        }
        Ok(taken)
    }

    /// What a contact is shown as: the alias it published for the subnet
    /// its locator is in, or its keyhash where none has reached this
    /// client (`light-client-requirements.md` §4.3).
    #[must_use]
    pub fn contact_label(&self, who: &Keyhash) -> String {
        self.store
            .contacts
            .get(who)
            .and_then(|c| c.alias.clone())
            .unwrap_or_else(|| hex8(who))
    }

    /// The alias this client published for itself in `subnet`, where it
    /// has: what an invitation carries as this device's claim
    /// (`light-client-requirements.md` §7).
    #[must_use]
    pub fn own_alias(&self, subnet: &Txid) -> Option<String> {
        self.store
            .profiles
            .get(subnet)
            .and_then(|b| rhtn_archive::profile::Profile::parse(b).ok())
            .map(|p| p.alias)
    }

    /// What a subnet is shown as: its alias where its profile is held.
    #[must_use]
    pub fn subnet_alias(&self, subnet: &Txid) -> Option<String> {
        self.store
            .subnet_profiles
            .get(subnet)
            .and_then(|b| rhtn_archive::profile::Profile::parse(b).ok())
            .map(|p| p.alias)
    }

    /// Sweep the population's reusable material (`light-client-requirements.md`
    /// §3), which `attach` does once.  A sweep is a snapshot of what was
    /// published when it ran, so a client that must attribute an initial
    /// message from a peer who published later sweeps again.
    /// The population is the caller's: a client that names its serving
    /// node wants that binding, since a node can be a participant and send
    /// payload of its own.  Only this client is dropped.
    pub fn sweep(&mut self, population: &[Keyhash]) -> Vec<Msg> {
        let me = self.keyhash();
        let others: Vec<Keyhash> = population.iter().copied().filter(|k| *k != me).collect();
        match others.len() {
            0 => Vec::new(),
            // a sweep names at least two (`wire-format.md` §7.8); one other
            // is asked for singly, reusable material only
            1 => {
                let nonce = self.nonce();
                vec![Msg::PrekeyRequest(
                    rhtn_archive::prekey::PrekeyRequest::One {
                        subject: others[0],
                        one_time: false,
                        nonce,
                        device: None,
                    }
                    .encode(),
                )]
            }
            _ => vec![Msg::PrekeyRequest(payload::batch_request(
                &others,
                self.nonce(),
            ))],
        }
    }

    /// **Ask a resource for something** (`wire-format.md` §11.1): the
    /// request goes to the node hosting it, which parses and re-serialises
    /// the application message and evaluates the asker's reach before the
    /// resource ever sees it (§11.2). The message is the application's
    /// own — an HTTP/1.1 request — and nothing here reads it.
    ///
    /// **The node asked is the one that served the entry.** A catalog
    /// entry names its resource and the node that answered for it
    /// (§6.1), and a resource is reached through its host and not
    /// directly.
    pub fn request_resource(&self, node: Keyhash, resource: Keyhash, message: Vec<u8>) -> Msg {
        Msg::ResourceRequest {
            node,
            bytes: rhtn_archive::catalog::ResourceRequest { resource, message }.encode(),
        }
    }

    /// Take a `ResourceResponse`: the status and the application's answer
    /// where one was delivered. **Held nowhere** — a resource's answer is
    /// the application's business and no part of this client's state.
    pub fn take_resource_response(&self, bytes: &[u8]) -> Result<(u64, Option<Vec<u8>>), String> {
        let r = rhtn_archive::catalog::ResourceResponse::decode(bytes)?;
        tracing::info!(target: "cer", status = r.status, "cer.resource");
        Ok((r.status, r.body))
    }

    /// Browse the catalog of the node this client is attached to
    /// (`wire-format.md` §6.4): the first, unfiltered query of a sweep.
    ///
    /// **A sweep, not a request**, because one reply is bounded at 111
    /// entries and names the type to ask for next. Each reply is taken by
    /// [`Client::take_catalog_reply`], which posts the continuation until
    /// the node's portion is complete or the hint repeats.
    pub fn browse(&mut self) -> Vec<Msg> {
        let Some(serving) = self.payload.serving else {
            return Vec::new();
        };
        let mut sweep = crate::catalog::Sweep::default();
        let q = sweep.query(None, self.nonce());
        self.browsing = Some((serving, sweep));
        vec![Msg::CatalogQuery(q.encode())]
    }

    /// Take a `CatalogReply` from the serving node: entries are verified
    /// under their owners and kept in this client's portion for that node,
    /// and the continuation is followed once per type.
    ///
    /// **A reply this sweep did not ask for is not taken** — §6.4's echoed
    /// nonce is what ties the two, and an entry out of a reply that cannot
    /// be attributed is an entry from nowhere.
    pub fn take_catalog_reply(
        &mut self,
        bytes: &[u8],
    ) -> Result<(crate::catalog::Step, Vec<Msg>), String> {
        let reply = rhtn_archive::catalog::CatalogReply::decode(bytes)?;
        let Some((node, mut sweep)) = self.browsing.take() else {
            return Err("no sweep is under way".into());
        };
        let step = sweep.take(
            &self.known,
            self.catalog.portion(node),
            &reply,
            rhtn_codec::bounds::CATALOG_REPLY_ENTRIES,
        );
        let out = match &step {
            crate::catalog::Step::Again(t) => {
                let q = sweep.query(Some(t.clone()), self.nonce());
                vec![Msg::CatalogQuery(q.encode())]
            }
            _ => Vec::new(),
        };
        // a sweep that asked again is still under way; one that is done,
        // truncated, or answered under a nonce it did not send is not
        if matches!(
            step,
            crate::catalog::Step::Again(_) | crate::catalog::Step::WrongNonce
        ) {
            self.browsing = Some((node, sweep));
        }
        Ok((step, out))
    }

    /// Ask `subject` for its own archive from `head`, at most `max`
    /// records (`wire-format.md` §7.9).
    ///
    /// **The subject is asked, not its infrastructure**
    /// (`light-client-requirements.md` §2): the subject holds their
    /// archive, so a patron evaluating you fetches from you, and this is
    /// peer-to-peer payload rather than something a node serves on your
    /// behalf.  One fetch outstanding per subject; the nonce is what ties
    /// the answer to it (design §15: an attestation delivery carries the
    /// nonce the evaluator generated).
    pub fn fetch_archive(
        &mut self,
        subject: Keyhash,
        head: Option<Txid>,
        max: u64,
    ) -> Result<Vec<Msg>, PayloadError> {
        let nonce = self.nonce();
        let req = rhtn_archive::chain::ArchiveRequest {
            subject,
            frontier: head.into_iter().collect(),
            max_records: max,
            stop_before: None,
            nonce,
        };
        self.fetching.insert(
            subject,
            Fetch {
                nonce,
                frontier: head.into_iter().collect(),
                bounds: BTreeMap::new(),
                max,
            },
        );
        self.send_payload(subject, payload::KIND_ARCHIVE_REQUEST, &req.encode())
    }

    /// Answer a fetch from my own archive, and nobody else's.
    ///
    /// **A subject that cannot be asked cannot be evaluated**, which is
    /// what §2's obligation is for: the reply goes back on the same
    /// peer-to-peer channel the request came in on.  `Archive::serve`
    /// refuses any subject but this client's own key, so a request naming
    /// somebody else answers empty — which says nothing about that
    /// archive.
    fn serve_archive(&mut self, from: Keyhash, bytes: &[u8]) -> Dispatched {
        let Ok(req) = rhtn_archive::chain::ArchiveRequest::decode(bytes) else {
            return Dispatched::Served {
                records: 0,
                more: false,
            };
        };
        let mut reply = self.archive.serve(&req);
        // a presence record arrives in the presented form (`wire-format.md`
        // §7.9): the envelope and its six slots, nothing revealed unless
        // the holder chooses to, which is what this path is for.  One whose
        // disclosure set this holder does not have goes as the envelope,
        // there being nothing to present it with
        for raw in reply.records.iter_mut() {
            if let Ok(rec) = Record::parse(raw)
                && rec.tx_type == rhtn_archive::tx::TYPE_PRESENCE
                && let Some(set) = self.store.disclosures.get(&rec.txid)
            {
                *raw = crate::record::present(raw, set, &[]);
            }
        }
        let (records, more) = (reply.records.len(), reply.more);
        if let Ok(msgs) = self.send_payload(from, payload::KIND_ARCHIVE_REPLY, &reply.encode()) {
            self.outbox.extend(msgs);
        }
        Dispatched::Served { records, more }
    }

    /// Take a fetch's reply: the nonce ties it to the request, and the
    /// chain is verified here rather than trusted.
    ///
    /// **A holder cannot be trusted to have walked correctly**
    /// (`light-client-requirements.md` §2): each record's back-pointers
    /// must reach the record after it and the first must be the head that
    /// was asked for.  What verifies is kept — presence records into the
    /// evidence store, where an adoption naming one can be evaluated
    /// against it (design §15: attestation is fetched on demand).
    fn take_archive_reply(&mut self, from: Keyhash, bytes: &[u8]) -> Result<usize, String> {
        use rhtn_archive::walk::{BatchEnd, verify_batch_bounded};
        let reply = rhtn_archive::chain::ArchiveReply::decode(bytes)?;
        let Some(fetch) = self.fetching.get(&from).cloned() else {
            return Err("no fetch outstanding with this party".into());
        };
        if reply.nonce != fetch.nonce {
            return Err("reply does not carry the nonce of the fetch".into());
        }
        // **the batch is verified as the DAG it is** (`wire-format.md`
        // §7.9): every record reachable from the frontier, on every branch
        // of a merge, with the chronology an earlier page established
        // carried into this one.  A head this client did not name
        // constrains the first record not at all, the holder's-newest case
        let v = verify_batch_bounded(&from, &fetch.frontier, &fetch.bounds, &reply, &self.known);
        // **nothing lands until the whole batch stands.**  A reply whose
        // verified prefix is followed by a record the frontier does not
        // name is a refused reply, and a refused reply leaves no durable
        // state behind it (`wire-format.md` §7.9)
        for (rec, (_, status)) in v.verified.iter().zip(v.signatures.iter()) {
            if !matches!(status, rhtn_archive::record::SigStatus::Verified) {
                self.fetching.remove(&from);
                return Err(format!("record {} does not verify", hex8(&rec.txid)));
            }
        }
        if let BatchEnd::Mismatch { why, .. } = &v.end {
            self.fetching.remove(&from);
            return Err(format!("the batch does not verify: {why}"));
        }
        let mut kept = 0;
        for rec in &v.verified {
            self.store.records.insert(rec.txid, rec.bytes.clone());
            kept += 1;
        }
        // more remain: the reply's frontier is what this page named and did
        // not return, and the next request asks for exactly that, under a
        // fresh nonce and the bounds this page owes
        match (&v.end, reply.more) {
            (BatchEnd::Unfetched { continue_from, .. }, true) => {
                let frontier = if reply.frontier.is_empty() {
                    vec![*continue_from]
                } else {
                    reply.frontier.clone()
                };
                let nonce = self.nonce();
                let req = rhtn_archive::chain::ArchiveRequest {
                    subject: from,
                    frontier: frontier.clone(),
                    max_records: fetch.max,
                    stop_before: None,
                    nonce,
                };
                self.fetching.insert(
                    from,
                    Fetch {
                        nonce,
                        frontier,
                        bounds: v.bounds.clone(),
                        max: fetch.max,
                    },
                );
                if let Ok(msgs) =
                    self.send_payload(from, payload::KIND_ARCHIVE_REQUEST, &req.encode())
                {
                    self.outbox.extend(msgs);
                }
            }
            _ => {
                self.fetching.remove(&from);
            }
        }
        Ok(kept)
    }

    /// Routine maintenance: rotate the signed prekey when its interval has
    /// elapsed, and replenish the pool when it has fallen low.
    pub fn maintain(&mut self) -> Vec<Msg> {
        let now = self.now_s();
        // **a witness's window maturing is an event with nothing to
        // announce it** [author, 2026-10-21]: the body was shown and held,
        // the proposer has sent what it had, and only this clock moves.
        // Signed before the outbox is taken, so the leg goes up with it
        self.sign_matured();
        // anything made since the last carry goes up with the routine
        // traffic, so a record whose own carry was missed is not stranded
        let mut out = self.outbox();
        out.extend(self.resend_unanswered(now));
        if self.payload.keys.due_for_rotation(now, &self.payload.cfg) {
            let random = self.device.random.clone();
            let mut fresh = |out: &mut [u8]| random.fill(out);
            self.payload.keys.rotate_signed_prekey(now, &mut fresh);
            out.extend(self.bundle_to_publish(now));
        }
        if self.payload.pool_reported < self.payload.cfg.replenish_below {
            out.extend(self.restock());
        }
        // bindings wanted for peers who wrote before this client held
        // theirs: reusable material only, one request each
        for subject in std::mem::take(&mut self.payload.wanted) {
            let nonce = self.nonce();
            out.push(Msg::PrekeyRequest(
                rhtn_archive::prekey::PrekeyRequest::One {
                    subject,
                    one_time: false,
                    nonce,
                    device: None,
                }
                .encode(),
            ));
        }
        out
    }

    fn restock(&mut self) -> Vec<Msg> {
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let n = self
            .payload
            .cfg
            .pool_target
            .saturating_sub(self.payload.pool_reported);
        if n == 0 {
            return Vec::new();
        }
        let keys = self.payload.keys.one_time_keys(n, &mut fresh);
        self.payload.pool_reported += n;
        tracing::info!(target: "pay", minted = n, "pay.bundle.restock");
        vec![Msg::StockOneTime(keys)]
    }

    /// The serving node reports how many one-time keys remain, or that
    /// none do: below the threshold the pool is replenished at once.
    pub fn on_pool_report(&mut self, remaining: usize) -> Vec<Msg> {
        if remaining == 0 {
            tracing::warn!(target: "pay", "pay.bundle.pool_exhausted");
        } else {
            tracing::debug!(target: "pay", remaining, "pay.bundle.pool");
        }
        self.payload.pool_reported = remaining;
        if remaining < self.payload.cfg.replenish_below {
            self.restock()
        } else {
            Vec::new()
        }
    }

    /// **This device's prekey handover for the co-present counterparty**
    /// (`wire-format.md` §14.3.4): its signed bundle and one one-time key,
    /// anchored to this ceremony and sealed under the local session like
    /// every other carriage.
    ///
    /// **Why it crosses the local interface at all.** Opening an
    /// end-to-end channel needs the peer's published bundle, and fetching
    /// one needs a node (§7.8). Two devices that have just met have no
    /// reason to ask anybody: each already holds what the other needs, and
    /// a pair whose ceremony completed with no node reachable (design §6.4)
    /// would otherwise end holding a record and unable to say a word to
    /// each other.
    ///
    /// Nothing is spent that a pool misses: the one-time key is one of this
    /// device's own, generated here and kept with the rest, and a device
    /// whose material is unsigned — one holding no seed (design §23.3) —
    /// has nothing to hand over and says so.
    pub fn prekey_carriage(&mut self) -> Result<Vec<u8>, Abort> {
        let cid = self.anchored_id()?;
        let now = self.now_s();
        let bundle = match self.bundle_to_publish(now) {
            Some(Msg::PublishBundle(b)) => b,
            _ => {
                // a device holding no seed has nothing signed to hand
                // over (design §23.3)
                return Err(abort(
                    "prekey_carriage",
                    Abort::Malformed("this device has no signed bundle to hand over".into()),
                ));
            }
        };
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let one_time = self.payload.keys.one_time_keys(1, &mut fresh).pop();
        let handover = crate::local::PrekeyHandover {
            ceremony_id: cid,
            device: self.payload.device,
            bundle,
            one_time,
        };
        tracing::info!(
            target: "cer",
            ceremony = %id8(&cid),
            one_time = handover.one_time.is_some(),
            "cer.prekey.handed"
        );
        self.seal(&handover.encode())
    }

    /// **The counterparty's handover, opened and kept** (`wire-format.md`
    /// §14.3.4): the bundle verified under the identity pinned at §14.3.1
    /// and held as prefetched material, and the one-time key kept for the
    /// first session opened with that device.
    ///
    /// **The signature is what makes it trustworthy and not the channel.**
    /// A bundle that does not verify is refused however it arrived, which
    /// is the same rule the node path applies; the ceremony-id is checked
    /// so a handover of another ceremony cannot be replayed into this one.
    pub fn take_prekey_carriage(&mut self, sealed: &[u8]) -> Result<(), Abort> {
        let cid = self.anchored_id()?;
        let peer = self.active.as_ref().ok_or(Abort::NotActive)?.peer()?;
        let plain = self.open(sealed)?;
        let h = crate::local::PrekeyHandover::decode(&plain).map_err(Abort::Malformed)?;
        if h.ceremony_id != cid {
            return Err(abort("take_prekey_carriage", Abort::CeremonyIdMismatch));
        }
        let p = payload::read_bundle(&self.known, &h.bundle).map_err(Abort::Malformed)?;
        if p.subject != peer || p.device != h.device {
            return Err(abort(
                "take_prekey_carriage",
                Abort::Malformed("a bundle for another party or another device".into()),
            ));
        }
        tracing::info!(
            target: "cer",
            ceremony = %id8(&cid),
            device = %id8(&h.device),
            one_time = h.one_time.is_some(),
            "cer.prekey.taken"
        );
        if let Some(k) = h.one_time {
            // **refused here, not downgraded later** [reviewer, 2026-10-08]:
            // a handed key that does not decode used to be found out at
            // the first send and the session opened reusable-only — §7.8's
            // declared forward-secrecy reduction, taken silently on what
            // was simply bad input. A handover that cannot be used is a
            // handover refused, while the counterparty is still in front
            // of this device to notice
            payload::OneTimeKey::decode(&k).map_err(|e| {
                abort(
                    "take_prekey_carriage",
                    Abort::Malformed(format!("the handed one-time key does not decode: {e}")),
                )
            })?;
            self.payload.handed.insert((peer, h.device), k);
        }
        self.payload.sessions.prefetch(p);
        Ok(())
    }

    /// Open a session from material already held, asking nobody.
    ///
    /// **This is what a handover buys** (`wire-format.md` §14.3.4): the
    /// peer's bundle and one of its one-time keys are both in hand, so
    /// there is nothing left for a node to be asked. Called only where a
    /// handed key is held — prefetched reusable material alone does not
    /// open a session, because §7.8 has the one-time key requested when a
    /// session opens and reusable-only is the pool-ran-dry case.
    fn open_from_held(
        &mut self,
        to: Keyhash,
        device: [u8; 32],
        plaintext: &[u8],
    ) -> Option<Vec<Msg>> {
        let their = self.payload.sessions.prefetched.get(&(to, device))?.clone();
        let handed = self.payload.handed.remove(&(to, device));
        // the key was checked when it was handed over, so this cannot
        // fail; if it ever does, nothing opens — a session on reusable
        // material alone is a forward-secrecy reduction this side does
        // not take by accident
        let one_time = match handed {
            Some(k) => Some(payload::OneTimeKey::decode(&k).ok()?),
            None => None,
        };
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let me = self.payload.device;
        let first = self
            .payload
            .sessions
            .open(
                &self.payload.keys,
                &me,
                to,
                &their,
                one_time.as_ref(),
                &mut fresh,
                plaintext,
            )
            .ok()?;
        tracing::debug!(
            target: "pay",
            to = %id8(&to),
            device = %id8(&device),
            one_time = one_time.is_some(),
            "pay.open.held"
        );
        Some(vec![self.route(to, first, device)])
    }

    /// Everything addressing `to` needs that this client can answer from
    /// its own state (`wire-format.md` §7.7.2): who serves it, and — only
    /// where that is not held — the infrastructure a resolution could be
    /// asked of.
    ///
    /// **The identities are added here**, because the horizon holds
    /// positions and addresses and this holds the key material that binds
    /// a dial (`wire-format.md` §9.1).  A node whose identity this client
    /// does not hold is still named: a resolution reply may carry its
    /// material, which is what §7.7.3's optional field is for.
    pub fn reach_for(&self, to: &Keyhash) -> Reach {
        let binds = |node: &Keyhash| {
            self.known
                .iter()
                .find(|i| i.keyhash == *node)
                .map(|i| i.key_material())
        };
        let upstream = match self.horizon.upstream_of(to) {
            Upstream::Known {
                node,
                endpoints,
                key_material: _,
            } => {
                let key_material = binds(&node);
                Upstream::Known {
                    node,
                    endpoints,
                    key_material,
                }
            }
            // **a contact's locator is a position to route by**
            // (`light-client-requirements.md` §4.3; design §12.3 Case 1):
            // where the horizon holds nothing of the party, what they or
            // an introducer last said of where they sit is what a
            // resolution is made of
            Upstream::Unknown => match self
                .store
                .contacts
                .get(to)
                .and_then(|c| c.locator.as_deref())
                .and_then(|sl| rhtn_archive::locator::SignedLocator::parse(sl).ok())
            {
                Some(sl) => Upstream::Position {
                    anchor: sl.locator.anchor,
                    path: sl.locator.path,
                    nibbles: sl.locator.nibbles,
                },
                None => Upstream::Unknown,
            },
            other => other,
        };
        // only a bare position needs somebody asked, and nothing is
        // prepared for the other two cases
        let ask: Vec<Askable> = match &upstream {
            Upstream::Position { anchor, .. } => {
                let mut out: Vec<Askable> = self
                    .horizon
                    .reachable_infra()
                    .into_iter()
                    .map(|(node, endpoints)| Askable {
                        key_material: binds(&node),
                        node,
                        endpoints,
                    })
                    .collect();
                // **the anchor's own address, where the contacts hold one**
                // (design §12.3 Case 1 step 2: "Bob looks up `anchor` in
                // his local table for an address to start from — his
                // contacts"). The horizon keeps endpoints for its own
                // neighbourhood alone, so an anchor further off is here or
                // nowhere, and a client with no node of its own has only
                // this [2026-10-13]. First, because it is the step the
                // design starts the walk at.
                if let Some(c) = self.store.contacts.get(anchor)
                    && !out.iter().any(|a| a.node == *anchor)
                {
                    // what it published first, then what a referral said
                    // of it: signed before asserted
                    let published = c
                        .endpoint_record
                        .as_deref()
                        .and_then(|b| rhtn_archive::endpoint::EndpointRecord::parse(b).ok())
                        .map(|r| r.endpoints)
                        .unwrap_or_default();
                    let endpoints: Vec<Vec<u8>> = published
                        .into_iter()
                        .chain(c.heard.iter().cloned())
                        .collect();
                    if !endpoints.is_empty() {
                        out.insert(
                            0,
                            Askable {
                                node: *anchor,
                                endpoints,
                                key_material: binds(anchor),
                            },
                        );
                    }
                }
                out
            }
            _ => Vec::new(),
        };
        Reach { upstream, ask }
    }

    /// What was queued for `to` while a prekey reply was awaited, now
    /// that a session with the device exists: in the order it was queued.
    ///
    /// **A send with no session queues its plaintext and asks for a
    /// one-time key** (`wire-format.md` §7.8), and the queue is drained
    /// when the reply opens the session.  Where the ask is answered with
    /// no bundle — a subject that had not published one yet — nothing
    /// opens and the queue stays; and a session established the other way
    /// round, by the peer writing first, never looked at it.  **The
    /// queued steps were then stranded for as long as that session
    /// lived**, which cost a witness the opening of a conversation and
    /// cost the sender no error [2026-10-06]. This is what the gate
    /// caught.
    ///
    /// An encryption that fails leaves the rest of that device's queue
    /// undelivered, as the reply path's does: a ratchet that will not
    /// encrypt is not a condition a retry mends.
    fn flush_pending(&mut self, to: Keyhash) -> Vec<Msg> {
        let devices: Vec<[u8; 32]> = self
            .payload
            .pending
            .keys()
            .filter(|(p, _)| *p == to)
            .map(|(_, d)| *d)
            .filter(|d| self.payload.sessions.has_session_with(&to, d))
            .collect();
        let me = self.payload.device;
        let mut out = Vec::new();
        for device in devices {
            let waiting = self
                .payload
                .pending
                .remove(&(to, device))
                .unwrap_or_default();
            for plaintext in &waiting {
                let Some(r) = self.payload.sessions.ratchets.get_mut(&(to, device)) else {
                    break;
                };
                let Ok(m) = r.encrypt(plaintext) else { break };
                out.push(self.route(to, payload::channel_message(&me, &m), device));
            }
        }
        out
    }

    /// Send `bytes` of `kind` to `to` (design §14.2.4.1): to the serving
    /// node it rides the transport session; to a leaf it goes on the
    /// session held, or waits for the one-time key requested now, and
    /// takes the direct path where one exists and the relay otherwise.
    pub fn send_payload(
        &mut self,
        to: Keyhash,
        kind: u64,
        bytes: &[u8],
    ) -> Result<Vec<Msg>, PayloadError> {
        if Some(to) == self.payload.serving {
            tracing::debug!(
                target: "pay",
                kind = crate::diag::kind_name(kind),
                to = %id8(&to),
                path = "transport",
                bytes = bytes.len(),
                "pay.send"
            );
            return Ok(vec![Msg::Transport(bytes.to_vec())]);
        }
        let plaintext = payload::wrap(kind, bytes);
        // a session is with a device (design §14.2.4): every device of the
        // recipient with a session open gets its own ciphertext, and every
        // device without one gets a session opened, on a one-time key
        // asked for that device (`wire-format.md` §7.8 field 4)
        let me = self.payload.device;
        // anything queued for this peer earlier goes first, so the order it
        // was queued in is the order it travels
        let mut out = self.flush_pending(to);
        if self.payload.sessions.has_session(&to) {
            for (device, m) in self.payload.sessions.send(&me, &to, &plaintext)? {
                out.push(self.route(to, m, device));
            }
        }
        let devices = self.devices_of(&to);
        if devices.is_empty() {
            return Err(PayloadError::NoBundle);
        }
        for device in devices {
            if self.payload.sessions.has_session_with(&to, &device) {
                continue;
            }
            // **a one-time key handed over in person opens the session
            // here**, with no request and no node: what a prekey handover
            // across the local interface is for (`wire-format.md`
            // §14.3.4), and what lets a pair whose ceremony needed no node
            // say something to each other afterwards.
            //
            // **Only a handed key, never prefetched material alone.** §7.8
            // has a one-time key requested *when* a session is opened and
            // never prefetched, and a session opened on reusable material
            // alone is a declared reduction in forward secrecy for its
            // first message — the case where a pool ran dry, not a
            // shortcut to take whenever a sweep happens to have fetched a
            // bundle. Four tests said so when this first fired on any
            // prefetched material [2026-10-06].
            if self.payload.handed.contains_key(&(to, device))
                && let Some(msgs) = self.open_from_held(to, device, &plaintext)
            {
                out.extend(msgs);
                continue;
            }
            self.payload
                .pending
                .entry((to, device))
                .or_default()
                .push(plaintext.clone());
            if self
                .payload
                .outstanding
                .values()
                .any(|t| *t == (to, device))
            {
                continue;
            }
            let nonce = self.nonce();
            self.payload.outstanding.insert(nonce, (to, device));
            out.push(Msg::PrekeyRequest(payload::one_time_request(
                to, device, nonce,
            )));
        }
        if tracing::event_enabled!(target: "pay", tracing::Level::DEBUG) {
            let requested = out
                .iter()
                .filter(|m| matches!(m, Msg::PrekeyRequest(_)))
                .count();
            tracing::debug!(
                target: "pay",
                kind = crate::diag::kind_name(kind),
                to = %id8(&to),
                bytes = bytes.len(),
                sent = out.len() - requested,
                requested,
                "pay.send"
            );
        }
        Ok(out)
    }

    /// The recipient's devices a session with `to` is with: the devices
    /// its prefetched bundles name, else the classical key of the identity
    /// held for it, which names the seed-holding device.
    fn devices_of(&self, to: &Keyhash) -> Vec<[u8; 32]> {
        let known = self.payload.sessions.devices_of(to);
        if !known.is_empty() {
            return known;
        }
        self.known
            .iter()
            .find(|i| i.keyhash == *to)
            .map(|i| vec![*i.ed.as_bytes()])
            .unwrap_or_default()
    }

    fn route(&self, to: Keyhash, bytes: Vec<u8>, device: [u8; 32]) -> Msg {
        let direct = self.device.direct.reachable(&to);
        tracing::debug!(
            target: "pay",
            to = %id8(&to),
            device = %id8(&device),
            path = if direct { "direct" } else { "relay" },
            bytes = bytes.len(),
            "pay.route"
        );
        if direct {
            Msg::Payload { to, bytes, device }
        } else {
            Msg::Relay { to, bytes, device }
        }
    }

    /// Which payload construction this build runs, for the client to put
    /// where its user can reach it (`light-client-requirements.md` §3): no
    /// peer can tell from the wire, so the only party who can say is the
    /// client itself. **A value, read off the sessions' own type** — a
    /// restated answer would quietly go wrong the day the type moved —
    /// and the wording is each shell's, in its user's own language.
    pub fn payload_construction(&self) -> crate::ratchet::Construction {
        payload::construction()
    }

    /// A binding that arrived beside a delivery (`wire-format.md` §7.10).
    ///
    /// Kept where it verifies against material this client already holds,
    /// and **discarded where it does not**: the node carrying it is not
    /// trusted for it, and a delivery is not the place to learn that a
    /// party's published material is bad. Kept only where no binding for
    /// that subject's device is held already, which is §7.10's rule and
    /// not an optimisation. Answering whether it was kept is for the
    /// caller's own accounting — false is *not held from here*, whether
    /// because one was held already or because this one did not verify —
    /// and the message that came with it is processed either way.
    pub fn take_binding(&mut self, bytes: &[u8]) -> bool {
        // **A recipient that already holds the binding ignores what
        // arrived** (`wire-format.md` §7.10).  A delivery is carriage and
        // not a refresh: honouring a carried bundle over a held one would
        // let an old-but-valid bundle displace a newer binding and send
        // session establishment toward retired material, and checking
        // held-ness first also spares verifying a signature per relayed
        // message.  Freshness comes from publication and the sweep.
        let Ok(b) = rhtn_archive::prekey::PrekeyBundle::parse(bytes) else {
            return false;
        };
        if self.payload.sessions.holds_binding(&b.subject, &b.device) {
            return false;
        }
        match payload::read_bundle(&self.known, bytes) {
            Ok(p) => {
                self.payload.sessions.prefetch(p);
                true
            }
            Err(_) => false,
        }
    }

    /// A reply from the serving node: a sweep's bundles are kept; a
    /// one-time reply opens the session it was asked for, with the key
    /// where one came and on reusable material alone where none did, and
    /// sends what was waiting.
    pub fn take_prekey_reply(&mut self, bytes: &[u8]) -> Result<Vec<Msg>, String> {
        if let Ok(replies) = decode_batch_reply(bytes) {
            let (mut offered, mut kept) = (0usize, 0usize);
            for r in replies {
                // one bundle per device (`wire-format.md` §7.8), each kept
                // under the device it names
                for b in &r.bundles {
                    offered += 1;
                    if let Ok(p) = payload::read_bundle(&self.known, b) {
                        self.payload.sessions.prefetch(p);
                        kept += 1;
                    }
                }
            }
            tracing::info!(target: "pay", reply = "batch", offered, kept, "pay.bundle");
            return Ok(Vec::new());
        }
        let r = PrekeyReply::decode(bytes)?;
        let Some((to, device)) = self.payload.outstanding.remove(&r.nonce) else {
            // a reusable-only reply: the bundles are kept, and nothing opens
            let mut kept = 0usize;
            for b in &r.bundles {
                if let Ok(p) = payload::read_bundle(&self.known, b) {
                    self.payload.sessions.prefetch(p);
                    kept += 1;
                }
            }
            tracing::info!(
                target: "pay",
                reply = "reusable",
                offered = r.bundles.len(),
                kept,
                "pay.bundle"
            );
            return Ok(Vec::new());
        };
        tracing::info!(
            target: "pay",
            reply = "one_time",
            to = %id8(&to),
            device = %id8(&device),
            bundle = !r.bundles.is_empty(),
            one_time = r.one_time.is_some(),
            "pay.bundle"
        );
        let their = match r.bundles.first() {
            Some(b) => {
                let p = payload::read_bundle(&self.known, b)?;
                if p.subject != to {
                    return Err("bundle for another subject".into());
                }
                if p.device != device {
                    return Err("bundle for another device".into());
                }
                self.payload.sessions.prefetch(p.clone());
                p
            }
            None => self
                .payload
                .sessions
                .prefetched
                .get(&(to, device))
                .cloned()
                .ok_or("no bundle for the peer")?,
        };
        let one_time = match r.one_time {
            Some(k) => Some(payload::OneTimeKey::decode(&k)?),
            None => None,
        };
        let mut waiting = self
            .payload
            .pending
            .remove(&(to, device))
            .unwrap_or_default();
        if waiting.is_empty() {
            waiting.push(payload::wrap(payload::KIND_APPLICATION, b""));
        }
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let me = self.payload.device;
        let first = self
            .payload
            .sessions
            .open(
                &self.payload.keys,
                &me,
                to,
                &their,
                one_time.as_ref(),
                &mut fresh,
                &waiting[0],
            )
            .map_err(|e| e.to_string())?;
        let mut out = vec![self.route(to, first, device)];
        for p in &waiting[1..] {
            let r = self
                .payload
                .sessions
                .ratchets
                .get_mut(&(to, device))
                .ok_or("no session")?;
            let m = r.encrypt(p)?;
            out.push(self.route(to, payload::channel_message(&me, &m), device));
        }
        Ok(out)
    }

    /// Run as another of this identity's devices: what a desktop holding a
    /// delegation is (design §14.2.4).  Its bundle names `key`, the
    /// transport key it presents, and its messages say they are from it.
    pub fn as_device(&mut self, key: [u8; 32]) {
        self.payload.device = key;
    }

    /// The session moved to `serving`, a sibling after a failover: what
    /// this client publishes, sweeps and asks now goes there, and what it
    /// records of the answers names that node (`light-client-requirements.md`
    /// §4).
    pub fn reattached(&mut self, serving: Keyhash) {
        self.payload.serving = Some(serving);
    }

    /// The bundle to publish now, if this device can: signed here where
    /// the seed is here, or the one the ceremony device signed over the
    /// material currently held.  A delegated device whose material has no
    /// signature yet publishes nothing and offers [`Client::bundle_to_sign`].
    fn bundle_to_publish(&mut self, now: u64) -> Option<Msg> {
        if let Some(me) = self.signer.as_deref() {
            return Some(Msg::PublishBundle(self.payload.keys.bundle(
                me,
                &self.payload.device,
                now,
            )));
        }
        let blob = self.payload.keys.blob().encode();
        match &self.payload.signed_bundle {
            Some((over, signed)) if *over == blob => Some(Msg::PublishBundle(signed.clone())),
            _ => None,
        }
    }

    /// On a device holding no seed: the unsigned bundle over this device's
    /// current material, for the ceremony device to sign
    /// (`wire-format.md` §7.8, design §23.3).  Nothing where this device
    /// holds the seed, or where the material held is already signed over.
    pub fn bundle_to_sign(&self) -> Option<Vec<u8>> {
        if self.signer.is_some() {
            return None;
        }
        let blob = self.payload.keys.blob().encode();
        if matches!(&self.payload.signed_bundle, Some((over, _)) if *over == blob) {
            return None;
        }
        let now = self.now_s();
        Some(
            self.payload
                .keys
                .bundle_payload(&self.public.keyhash, &self.payload.device, now),
        )
    }

    /// On the ceremony device: sign a bundle payload another device of
    /// mine made over its own material.  Refused where the payload names
    /// another subject; the device it names is that device's to say.
    pub fn sign_device_bundle(&self, payload: &[u8]) -> Result<Vec<u8>, Abort> {
        let me = self.signer()?;
        let (subject, _device) = rhtn_archive::prekey::PrekeyBundle::payload_names(payload)
            .map_err(|_| Abort::NotMine("not a payload"))?;
        if subject != self.public.keyhash {
            return Err(Abort::NotMine("another subject"));
        }
        Ok(rhtn_archive::prekey::PrekeyBundle::sign(me, payload))
    }

    /// On a device holding no seed: take the bundle the ceremony device
    /// signed.  It must verify under my identity, name this device, and be
    /// over the material this device holds now; what comes back is the
    /// publication for the serving node, where one is attached.
    pub fn take_signed_bundle(&mut self, signed: &[u8]) -> Result<Option<Msg>, Abort> {
        let b = rhtn_archive::prekey::PrekeyBundle::parse(signed)
            .map_err(|_| Abort::NotMine("does not parse"))?;
        b.verify(std::slice::from_ref(&self.public))
            .map_err(|_| Abort::NotMine("does not verify under me"))?;
        if b.subject != self.public.keyhash {
            return Err(Abort::NotMine("another subject"));
        }
        if b.device != self.payload.device {
            return Err(Abort::NotMine("another device"));
        }
        let blob = self.payload.keys.blob().encode();
        if b.blob != blob {
            return Err(Abort::NotMine("not this device's current material"));
        }
        self.payload.signed_bundle = Some((blob, signed.to_vec()));
        self.payload.keys.published_at = Some(b.published_at);
        Ok(self
            .payload
            .serving
            .map(|_| Msg::PublishBundle(signed.to_vec())))
    }

    /// On the ceremony device: delegate to a transport key another device
    /// of mine minted, for one 48-hour window from `not_before`
    /// (`wire-format.md` §8.2).
    pub fn delegate(&self, key: &[u8; 32], not_before: u64) -> Result<Vec<u8>, Abort> {
        Ok(rhtn_crypto::delegation::issue(
            self.signer()?,
            key,
            not_before,
        ))
    }

    /// A run of `count` contiguous delegations from `start`: what an
    /// instance or a desktop is provisioned with
    /// (`infra-client-requirements.md` §7).
    pub fn delegate_run(
        &self,
        key: &[u8; 32],
        start: u64,
        count: usize,
    ) -> Result<Vec<Vec<u8>>, Abort> {
        Ok(rhtn_crypto::delegation::issue_run(
            self.signer()?,
            key,
            start,
            count,
        ))
    }

    /// **An endpoint record for an instance of this operator's**
    /// (`infra-client-requirements.md` §4.4): the signature is the
    /// operator's and the instance holds no seed to make its own, so this
    /// is a thing the client signs and hands over. `wire-format.md` §7.6
    /// publishes one for infrastructure only, which is what makes it the
    /// statement that its subject is infrastructure.
    /// The points arrive **already encoded**, which is what keeps this
    /// crate clear of the transport: what a point is belongs there, and
    /// signing a record over one does not need to know.
    pub fn endpoint_record(
        &self,
        points: &[Vec<u8>],
        seqno: rhtn_archive::tx::Seqno,
    ) -> Result<Vec<u8>, Abort> {
        Ok(rhtn_archive::endpoint::endpoint_record(
            self.signer()?,
            points,
            seqno,
        ))
    }

    /// **The series an endpoint record or anchor entry must carry for an
    /// instance serving `subnet`** (`wire-format.md` §7.6): the
    /// relationship line's own, since there is **one record per patron
    /// relationship** and a record's series "is unprovable in the other
    /// relationship's subnet". `None` names the subnet under this
    /// client's own key.
    ///
    /// Refused where this client holds no position there: an operator
    /// cannot claim an address in a subnet it is not in.
    pub fn line_in(&self, subnet: Option<Txid>) -> Result<u32, Abort> {
        let pos = match subnet {
            Some(s) => self.position_in(&s).ok_or(Abort::NoRelationship(s))?,
            None => self.own_position(),
        };
        Ok(pos.seqno.series)
    }

    /// An endpoint record for an instance serving `subnet`, on that
    /// line's series (`wire-format.md` §7.6).
    pub fn endpoint_record_in(
        &self,
        subnet: Option<Txid>,
        points: &[Vec<u8>],
        counter: u32,
    ) -> Result<Vec<u8>, Abort> {
        let seqno = rhtn_archive::tx::Seqno {
            series: self.line_in(subnet)?,
            counter,
        };
        self.endpoint_record(points, seqno)
    }

    /// An anchor entry for the same instance, on the same line.
    pub fn anchor_entry_in(
        &self,
        subnet: Option<Txid>,
        points: &[Vec<u8>],
        subtree_size: u64,
        counter: u32,
    ) -> Result<Vec<u8>, Abort> {
        let seqno = rhtn_archive::tx::Seqno {
            series: self.line_in(subnet)?,
            counter,
        };
        self.anchor_entry(points, subtree_size, seqno)
    }

    /// **An anchor entry for the same instance**, claiming a subtree of
    /// `subtree_size`, and signed by the same operator for the same
    /// reason.
    pub fn anchor_entry(
        &self,
        points: &[Vec<u8>],
        subtree_size: u64,
        seqno: rhtn_archive::tx::Seqno,
    ) -> Result<Vec<u8>, Abort> {
        Ok(rhtn_archive::endpoint::anchor_entry(
            self.signer()?,
            points,
            subtree_size,
            seqno,
        ))
    }

    /// What arrived on the end-to-end channel from `from`: decrypted on the
    /// session, or a session opened on my prekeys, and delivered by kind —
    /// a key grant to the grant handler, a late response beside its record,
    /// application payload to the application.
    pub fn receive_payload(&mut self, from: Keyhash, bytes: &[u8]) -> Result<Dispatched, String> {
        let out = self.receive_payload_in(from, bytes);
        // **a session opened by the peer writing first drains what was
        // waiting for one** ([`Client::flush_pending`]): into the outbox,
        // which the courier carries after every delivery, because a
        // delivery hands back a `Dispatched` and has nowhere else to put a
        // message of its own.
        if out.is_ok() {
            let waiting = self.flush_pending(from);
            self.outbox.extend(waiting);
        }
        match &out {
            Ok(d) => tracing::info!(
                target: "pay",
                from = %id8(&from),
                bytes = bytes.len(),
                dispatched = %crate::diag::dispatched(d),
                "pay.receive"
            ),
            Err(why) => tracing::warn!(
                target: "pay",
                from = %id8(&from),
                bytes = bytes.len(),
                %why,
                "pay.error"
            ),
        }
        out
    }

    fn receive_payload_in(&mut self, from: Keyhash, bytes: &[u8]) -> Result<Dispatched, String> {
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let me = self.keyhash();
        let plaintext = match self.payload.sessions.receive(
            &mut self.payload.keys,
            &me,
            from,
            bytes,
            &mut fresh,
        ) {
            Ok(p) => p,
            Err(e) => {
                // Nothing is opened and nothing dispatched under a name
                // this client cannot give the message.  Where the binding
                // is merely absent, it is asked for: the peer's next
                // attempt is attributable, and this one is not recovered.
                if matches!(e, payload::PayloadError::NoBundle) {
                    self.payload.wanted.insert(from);
                }
                if matches!(
                    e,
                    payload::PayloadError::NotTheSender | payload::PayloadError::NoBundle
                ) {
                    self.device
                        .notifier
                        .notify(Notice::PayloadUnattributable { from });
                }
                return Err(e.to_string());
            }
        };
        let (kind, inner) = payload::unwrap(&plaintext)?;
        Ok(match kind {
            payload::KIND_KEY_GRANT => Dispatched::Grant(self.take_grant(from, &inner)),
            payload::KIND_LATE_RESPONSE => {
                let consented = self.subject.consented_by_ceremony().clone();
                Dispatched::Late(record::take_late_response(
                    &mut self.store,
                    &self.known,
                    &inner,
                    &consented,
                ))
            }
            payload::KIND_CANDIDATES => {
                // the last address this party offered, kept for a dial with
                // no node to ask (`ClientStore::direct_hints`)
                self.store.direct_hints.insert(from, inner.clone());
                Dispatched::Candidates(inner)
            }
            // **between peers** (design §6.3): each is taken from a peer
            // and from nobody else, since what a peer holds is the one
            // thing an acquaintance does not get
            payload::KIND_PEER_LOCATOR
            | payload::KIND_BACKUP_DEPOSIT
            | payload::KIND_BACKUP_REQUEST
            | payload::KIND_BACKUP_RETURN
                if !self.peers().contains(&from) =>
            {
                tracing::info!(target: "peer", from = %id8(&from), kind = crate::diag::kind_name(kind), "peer.refused.not_a_peer");
                Dispatched::Peer(format!(
                    "{} sent a peer's message and is not a peer",
                    hex8(&from)
                ))
            }
            payload::KIND_PEER_LOCATOR => match self.hold_locator(&from, &inner, false) {
                Ok(()) => Dispatched::Peer(format!("{}'s current locator is held", hex8(&from))),
                Err(e) => Dispatched::Peer(format!("{}'s locator was refused: {e:?}", hex8(&from))),
            },
            payload::KIND_BACKUP_DEPOSIT => {
                let bytes = inner.len();
                self.store.held_for.insert(from, inner);
                tracing::info!(target: "peer", from = %id8(&from), bytes, "peer.held");
                Dispatched::Peer(format!(
                    "holding {bytes} bytes of backup for {}",
                    hex8(&from)
                ))
            }
            payload::KIND_BACKUP_REQUEST => match self.store.held_for.get(&from).cloned() {
                Some(blob) => {
                    let bytes = blob.len();
                    self.queue_to_peer(from, payload::KIND_BACKUP_RETURN, &blob);
                    tracing::info!(target: "peer", from = %id8(&from), bytes, "peer.returned");
                    Dispatched::Peer(format!(
                        "returned {bytes} bytes of backup to {}",
                        hex8(&from)
                    ))
                }
                None => Dispatched::Peer(format!(
                    "{} asked for a backup and none is held",
                    hex8(&from)
                )),
            },
            payload::KIND_BACKUP_RETURN => match self.reconcile_from_peer(&inner) {
                Ok(r) => Dispatched::Peer(format!(
                    "reconciled with {}: {} record(s), {} contact(s), {} other item(s) taken, {} refused",
                    hex8(&from),
                    r.records,
                    r.contacts,
                    r.other,
                    r.refused
                )),
                Err(e) => Dispatched::Peer(format!(
                    "the backup {} returned was not taken: {e}",
                    hex8(&from)
                )),
            },
            payload::KIND_RESPONSE_COPY => {
                Dispatched::ResponseCopy(self.take_response_copy(&inner))
            }
            payload::KIND_QUERY => Dispatched::Query {
                query: QueryRequest::decode(&inner)
                    .ok()
                    .map(|r| r.query.query_id()),
                outcome: self.take_query(from, &inner),
            },
            payload::KIND_RESPONSE => Dispatched::Response(self.take_response(&inner)),
            payload::KIND_ARCHIVE_REQUEST => self.serve_archive(from, &inner),
            payload::KIND_ARCHIVE_REPLY => {
                Dispatched::Fetched(self.take_archive_reply(from, &inner))
            }
            // the ceremony's conversation and its opening (`wire-format.md`
            // §7.10.1, kinds 9 to 18 and 20 to 25): into the ceremony's own
            // state, as a participant or a witness, with what the step owes
            // in answer put in the outbox.  Kind 19 arranges an adoption
            // and has its own row above
            k @ payload::KIND_CONSENT_REQUEST..=payload::KIND_RECORD
            | k @ payload::KIND_WITNESS_ENGAGEMENT..=payload::KIND_WITNESS_STANDDOWN => {
                Dispatched::Conversation(self.converse_in(from, k, &inner))
            }
            k => Dispatched::Application {
                kind: k,
                bytes: inner,
            },
        })
    }
}

/// The two participants in body order: ascending keyhash.
pub fn participants(a: Keyhash, b: Keyhash) -> [Keyhash; 2] {
    if a < b { [a, b] } else { [b, a] }
}

/// A conversation message as the end-to-end channel carries it
/// (`wire-format.md` §7.10.1): its kind and the bytes of its §7.10.2
/// structure.  `None` for a message that is not the conversation's, which
/// is every one the local exchanges (§14.3), the verifier path, the
/// serving node or the transport carry instead.
///
/// A proposal whose back-pointer lists do not match its signers is not one
/// this client built, and is `None` rather than a body with a signer's
/// list missing.
pub fn conversation_payload(msg: &Msg) -> Option<(u64, Vec<u8>)> {
    use crate::conversation::*;
    Some(match msg {
        Msg::ConsentRequest(q) => (payload::KIND_CONSENT_REQUEST, q.encode()),
        Msg::Consent { query_id, consent } => (
            payload::KIND_CONSENT_REPLY,
            ConsentReply {
                query_id: *query_id,
                consent: consent.clone(),
            }
            .encode(),
        ),
        Msg::WitnessRequest(r) => (payload::KIND_WITNESS_REQUEST, r.encode()),
        Msg::WitnessAnswer(bits) => (
            payload::KIND_WITNESS_ANSWER,
            WitnessAnswer { bits: *bits }.encode(),
        ),
        Msg::Responses(responses) => (
            payload::KIND_GATHERED_RESPONSES,
            GatheredResponses {
                responses: responses.clone(),
            }
            .encode(),
        ),
        Msg::BackPointers(txids) => (
            payload::KIND_BACK_POINTERS,
            BackPointers {
                txids: txids.clone(),
            }
            .encode(),
        ),
        Msg::Proposal(p) => {
            if p.back.len() != p.proposal.signers().len() {
                return None;
            }
            (
                payload::KIND_PROPOSED_BODY,
                ProposedBody {
                    body: p.proposal.body(&p.back),
                    // a presence record's proposal carries its set; the
                    // same kind arranging an adoption or a peering carries
                    // none (`wire-format.md` §7.10.2)
                    set: Some(p.set.clone()),
                }
                .encode(),
            )
        }
        Msg::Signed(reply) => (
            payload::KIND_SIGNING_REPLY,
            SigningReply {
                reply: reply.clone(),
            }
            .encode(),
        ),
        Msg::Record(envelope) => (payload::KIND_RECORD, envelope.clone()),
        Msg::Engagement(e) => (payload::KIND_WITNESS_ENGAGEMENT, e.encode()),
        Msg::Trace(t) => (payload::KIND_EXCHANGE_TRACE, t.encode()),
        Msg::WitnessSet(nominees) => (
            payload::KIND_WITNESS_SET,
            WitnessSet {
                nominees: nominees.clone(),
            }
            .encode(),
        ),
        Msg::WitnessKeep(kept) => (
            payload::KIND_WITNESS_KEEP,
            WitnessKeep { kept: kept.clone() }.encode(),
        ),
        Msg::Standdown(handle) => (
            payload::KIND_WITNESS_STANDDOWN,
            Standdown { handle: *handle }.encode(),
        ),
        _ => return None,
    })
}

/// The reverse: a kind and its bytes, read back as the message, or why
/// they do not read.  A body that does not decode is dropped by the
/// recipient that opened it (§7.10.1), and `Malformed` is that verdict.
///
/// Kind 11, the fishing proposal, decodes through
/// [`crate::conversation::FishingProposal`] and has no `Msg` yet: the
/// ceremony does not carry a bundle augmentation in process today, so it
/// falls through here until it does.
pub fn conversation_from(kind: u64, bytes: &[u8]) -> Result<Msg, Abort> {
    use crate::conversation::*;
    let malformed = Abort::Malformed;
    Ok(match kind {
        payload::KIND_CONSENT_REQUEST => {
            Msg::ConsentRequest(VerificationQuery::decode(bytes).map_err(malformed)?)
        }
        payload::KIND_CONSENT_REPLY => {
            let r = ConsentReply::decode(bytes).map_err(malformed)?;
            Msg::Consent {
                query_id: r.query_id,
                consent: r.consent,
            }
        }
        payload::KIND_WITNESS_REQUEST => {
            Msg::WitnessRequest(WitnessRequest::decode(bytes).map_err(malformed)?)
        }
        payload::KIND_WITNESS_ANSWER => {
            Msg::WitnessAnswer(WitnessAnswer::decode(bytes).map_err(malformed)?.bits)
        }
        payload::KIND_GATHERED_RESPONSES => Msg::Responses(
            GatheredResponses::decode(bytes)
                .map_err(malformed)?
                .responses,
        ),
        payload::KIND_BACK_POINTERS => {
            Msg::BackPointers(BackPointers::decode(bytes).map_err(malformed)?.txids)
        }
        payload::KIND_PROPOSED_BODY => {
            let pb = ProposedBody::decode(bytes).map_err(malformed)?;
            let (proposal, back) = proposal_from_body(&pb.body).map_err(malformed)?;
            let set = pb.set.ok_or_else(|| {
                malformed(String::from(
                    "a presence proposal carries its disclosure set",
                ))
            })?;
            Msg::Proposal(Box::new(Proposed {
                proposal,
                set,
                back,
            }))
        }
        payload::KIND_SIGNING_REPLY => {
            Msg::Signed(SigningReply::decode(bytes).map_err(malformed)?.reply)
        }
        payload::KIND_RECORD => {
            Record::parse(bytes).map_err(malformed)?;
            Msg::Record(bytes.to_vec())
        }
        payload::KIND_WITNESS_ENGAGEMENT => {
            Msg::Engagement(Engagement::decode(bytes).map_err(malformed)?)
        }
        payload::KIND_EXCHANGE_TRACE => Msg::Trace(Trace::decode(bytes).map_err(malformed)?),
        payload::KIND_WITNESS_SET => {
            Msg::WitnessSet(WitnessSet::decode(bytes).map_err(malformed)?.nominees)
        }
        payload::KIND_WITNESS_KEEP => {
            Msg::WitnessKeep(WitnessKeep::decode(bytes).map_err(malformed)?.kept)
        }
        payload::KIND_WITNESS_STANDDOWN => {
            Msg::Standdown(Standdown::decode(bytes).map_err(malformed)?.handle)
        }
        _ => {
            return Err(malformed(format!(
                "kind {kind} is not one the conversation reads"
            )));
        }
    })
}

/// One message on one path.
#[derive(Debug, Clone)]
pub struct Sent {
    /// Who sent it.
    pub from: Keyhash,
    /// Who it went to.
    pub to: Keyhash,
    /// What was sent.
    pub msg: Msg,
}

/// **An in-process carrier for what crosses a local interface**: every
/// client in one place, every message moved by hand and logged with its
/// path, so a test can say what travelled where.
///
/// **What it may drive** [author, 2026-10-14]: the local exchanges of a
/// meeting ([`Harness::exchanges`]), the adoption on a record already
/// held ([`Harness::adopt`]) and a recovery meeting, which is two people
/// face to face with no node in it. It drove the ceremony's conversation
/// until 2026-10-14, gathering the responses and the signatures itself —
/// which is the kernel's work in production, from arrivals — and the
/// first case moved to the end-to-end path exposed a defect that driver
/// had hidden. The conversation now runs where it runs: beside a node,
/// over the sessions, in `adaptors/tests`.
#[derive(Default)]
pub struct Harness {
    /// Every client on the harness, by keyhash.
    pub clients: BTreeMap<Keyhash, Client>,
    /// Every message moved, in order, with its path.
    pub log: Vec<Sent>,
}

impl Harness {
    /// Put `c` on the harness; its keyhash, which is how it is reached.
    pub fn add(&mut self, c: Client) -> Keyhash {
        let k = c.keyhash();
        self.clients.insert(k, c);
        k
    }

    /// The client `k`.  Panics where the harness holds none, which in a
    /// test is the test's own error.
    pub fn client(&mut self, k: &Keyhash) -> &mut Client {
        self.clients.get_mut(k).expect("a client on the harness")
    }

    fn send(&mut self, from: Keyhash, to: Keyhash, msg: Msg) -> Msg {
        // the event names what moved and how much of it, never the bytes.
        // The log beneath keeps the message itself for the tests' path
        // assertions (which bytes reached whom): harness state, in process,
        // and rendered by nothing.
        tracing::debug!(
            target: "harness",
            from = %id8(&from),
            to = %id8(&to),
            msg = crate::diag::msg(&msg),
            bytes = msg.payload().len(),
            "harness.send"
        );
        self.log.push(Sent {
            from,
            to,
            msg: msg.clone(),
        });
        msg
    }

    /// The messages that travelled to `to`.
    pub fn to(&self, to: &Keyhash) -> Vec<&Sent> {
        self.log.iter().filter(|s| s.to == *to).collect()
    }

    /// **The local exchanges of one ceremony** (`wire-format.md` §14.3;
    /// design §7.1 steps 1 to 5): the two intents across the optical
    /// channel, the capture keys crossed
    /// and each party capturing the other.
    ///
    /// **This is what a harness may drive** [author, 2026-10-14]: every
    /// step here is a user's act or a device's — a code shown, a phone
    /// held up, a camera run — and the bytes cross a local interface that
    /// no node takes part in. Everything after it is the conversation,
    /// which the kernel drives from arrivals and which the adaptors'
    /// tests exercise over real sessions.
    pub fn exchanges(
        &mut self,
        a: Keyhash,
        b: Keyhash,
        a_nominees: Vec<Keyhash>,
        b_nominees: Vec<Keyhash>,
    ) -> Result<(), Abort> {
        let ia = self.client(&a).begin(Some(b), a_nominees, true)?;
        let ib = self.client(&b).begin(Some(a), b_nominees, false)?;
        self.cross(a, b, ia, ib)
    }

    /// The intents, channels and captures, once both sides have begun.
    fn cross(&mut self, a: Keyhash, b: Keyhash, ia: Intent, ib: Intent) -> Result<(), Abort> {
        let m = self.send(a, b, Msg::Intent(ia));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&b).take_intent(a, i)?;
        let m = self.send(b, a, Msg::Intent(ib));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&a).take_intent(b, i)?;
        // 3. capture keys cross, then each captures the other
        let ka = self.client(&a).capture_key()?;
        let kb = self.client(&b).capture_key()?;
        self.send(a, b, Msg::CaptureKey(ka.clone()));
        self.send(b, a, Msg::CaptureKey(kb.clone()));
        self.client(&b).capture(ka)?;
        self.client(&a).capture(kb)?;
        Ok(())
    }

    /// **The adoption alone, on a presence record already held**
    /// (design §8.2): the patron proposes, both sign, and both take it.
    ///
    /// `pop` is the meeting the adoption rests on, which the caller
    /// brings — a test needs the *evidence* of a ceremony here, and the
    /// ceremony itself is the kernel's to run over the payload path
    /// [author, 2026-10-14].
    pub fn adopt(
        &mut self,
        node: Keyhash,
        patron: Keyhash,
        pop: Txid,
        series: u32,
    ) -> Result<Txid, Abort> {
        let node_back = self.client(&node).back_pointers();
        let body = self.client(&patron).propose_adoption(
            node,
            &node_back,
            Adopting {
                evidence: Evidence::Presence(pop),
                series,
                presented_head: None,
                key_material: None,
                carrying: None,
            },
        )?;
        let m = self.send(patron, node, Msg::AdoptionBody(body));
        let Msg::AdoptionBody(body) = m else {
            unreachable!()
        };
        // **the node checks what it signs**, as it does on a phone: the
        // body arrived from the patron and nothing else vouches for it
        let n_entries = self.client(&node).review_and_sign_adoption(&body)?;
        self.send(node, patron, Msg::Signed(Ok(n_entries.clone())));
        let p_entries = self.client(&patron).sign_body(&body).unwrap();
        let envelope = envelope_from_entries(
            TYPE_ADOPTION,
            &body,
            &[(node, n_entries), (patron, p_entries)],
        );
        let t = self.client(&patron).take_adoption(&envelope)?;
        self.send(patron, node, Msg::Record(envelope.clone()));
        self.client(&node).take_adoption(&envelope)?;
        Ok(t)
    }

    /// A recovery meeting (design §9.1): `subject`, holding its prior key,
    /// meets `verifier`, a prior counterparty of that key, with its new
    /// key.  The meeting opens as a ceremony through capture; then the
    /// verifier is its own querier, the subject's new key consents, the
    /// verifier's person recognises, and the hybrid response goes to the
    /// subject.  No record is produced; the response is what the meeting
    /// yields.
    pub fn run_recovery_meeting(
        &mut self,
        subject: Keyhash,
        verifier: Keyhash,
    ) -> Result<Vec<u8>, Abort> {
        let prior = self.client(&subject).prior_key().ok_or(Abort::NoPriorKey)?;
        let ia = self.client(&subject).begin(Some(verifier), vec![], true)?;
        let ib = self.client(&verifier).begin(Some(subject), vec![], false)?;
        let m = self.send(subject, verifier, Msg::Intent(ia));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&verifier).take_intent(subject, i)?;
        let m = self.send(verifier, subject, Msg::Intent(ib));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&subject).take_intent(verifier, i)?;
        let ks = self.client(&subject).capture_key()?;
        let kv = self.client(&verifier).capture_key()?;
        self.send(subject, verifier, Msg::CaptureKey(ks.clone()));
        self.send(verifier, subject, Msg::CaptureKey(kv.clone()));
        self.client(&verifier).capture(ks)?;
        self.client(&subject).capture(kv)?;
        // the claim, the query, the consent, the recognition
        self.send(subject, verifier, Msg::ClaimPrior(prior));
        let q = self.client(&verifier).recovery_query(prior)?;
        let m = self.send(verifier, subject, Msg::ConsentRequest(q.clone()));
        let Msg::ConsentRequest(q) = m else {
            unreachable!()
        };
        let (consent, _) = self
            .client(&subject)
            .consent(&q)
            .ok_or(Abort::NotRecognised)?;
        let m = self.send(
            subject,
            verifier,
            Msg::Consent {
                query_id: q.query_id(),
                consent,
            },
        );
        let Msg::Consent { consent, .. } = m else {
            unreachable!()
        };
        let resp = self.client(&verifier).recognise(&q, &consent, prior)?;
        let m = self.send(verifier, subject, Msg::RecoveryResponse(resp));
        let Msg::RecoveryResponse(resp) = m else {
            unreachable!()
        };
        self.client(&subject)
            .take_recovery_response(&resp)
            .map_err(Abort::Record)?;
        // the meeting closes without a record
        self.client(&subject).subject.close_window();
        self.client(&subject).abandon();
        self.client(&verifier).subject.close_window();
        self.client(&verifier).abandon();
        Ok(resp)
    }

    /// The recovery adoption (design §9.0; `wire-format.md` §4.1): the
    /// subject assembles the block from what its meetings yielded and its
    /// old key's statement, the patron proposes the adoption and checks
    /// the block against it, both sign, and the old lines are sealed as
    /// the old key's last act before the adoption goes anywhere.
    pub fn run_recovery_adoption(
        &mut self,
        subject: Keyhash,
        patron: Keyhash,
        series: u32,
    ) -> Result<Txid, Abort> {
        let (block, head) = self.client(&subject).recovery_block(&patron)?;
        let m = self.send(
            subject,
            patron,
            Msg::RecoveryProposal {
                block,
                presented_head: head,
            },
        );
        let Msg::RecoveryProposal {
            block,
            presented_head,
        } = m
        else {
            unreachable!()
        };
        let node_back = self.client(&subject).back_pointers();
        let km = self.client(&subject).public.key_material();
        let body = self.client(&patron).propose_adoption(
            subject,
            &node_back,
            Adopting {
                evidence: Evidence::Recovery(block),
                series,
                presented_head,
                key_material: Some(km),
                carrying: None,
            },
        )?;
        let m = self.send(patron, subject, Msg::AdoptionBody(body));
        let Msg::AdoptionBody(body) = m else {
            unreachable!()
        };
        let s_entries = self.client(&subject).sign_body(&body).unwrap();
        self.send(subject, patron, Msg::Signed(Ok(s_entries.clone())));
        let p_entries = self.client(&patron).sign_body(&body).unwrap();
        let envelope = envelope_from_entries(
            TYPE_ADOPTION,
            &body,
            &[(subject, s_entries), (patron, p_entries)],
        );
        // the old key's last act, before the adoption is pushed
        let seals = self.client(&subject).seal_old_lines();
        for (holder, seal) in seals {
            self.send(subject, holder, Msg::Seal(seal));
        }
        let t = self.client(&patron).take_adoption(&envelope)?;
        self.send(patron, subject, Msg::Record(envelope.clone()));
        self.client(&subject).take_adoption(&envelope)?;
        Ok(t)
    }
}
