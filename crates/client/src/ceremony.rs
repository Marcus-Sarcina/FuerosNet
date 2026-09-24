//! The presence ceremony as a client runs it (design §7.1, §7.5.2,
//! §8.1.2; `light-client-requirements.md` §1): each party's decisions,
//! taken against what that party holds and sees, and the messages the
//! ceremony's direct channel carries between them.  Those messages are
//! carried by no wire object (§8.1.2's rule), so here they are values; an
//! in-process [`Harness`] moves them and records every path they take.

use crate::backup;
use crate::device::{ChannelKind, ChannelOutcome, ChannelResult, Device, guided_capture};
use crate::horizon::Horizon;
use crate::keys::{capture_key, pre_commitment};
use crate::notice::{Notice, Role};
use crate::payload::{self, PayloadError, PayloadState};
use crate::query::{KeyGrant, QueryRequest, Response, VerificationQuery};
use crate::record::{
    self, DisclosureSet, Proposal, Refusal, disclosure_root, disclosures, participant_check,
    sort_responses, witness_check,
};
use crate::rotation::Rotation;
use crate::selection::{self, Acquaintance, SelectionBasis, required};
use crate::store::{Capture, ClientStore, OwnSeed, SealParams, SealedCapture, seal};
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

/// A client's own numbers for the ceremony.
#[derive(Clone)]
pub struct Config {
    /// How far a claimed `started_at` may sit from this clock, in seconds
    /// (`light-client-requirements.md` §1.2: the witness's to set).
    pub clock_tolerance_s: u64,
    /// The retention this client declares, in years (design §7.5.1).
    pub retention_years: u64,
    pub template_version: u64,
    pub capture: crate::device::CaptureParams,
    pub seal: SealParams,
    pub subject: SubjectConfig,
    pub verifier: VerifierConfig,
    pub payload: payload::PayloadConfig,
    /// The trust policy this client computes standing with: the reference
    /// metric unless something substitutes one (design §16.1).  **Nothing
    /// the client stores or sends consults it** (design §16.4), so a
    /// client running a substitute produces the same wire traffic.
    pub policy: std::sync::Arc<dyn rhtn_policy::Policy<Keyhash>>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            clock_tolerance_s: 300,
            retention_years: 2,
            template_version: 1,
            capture: Default::default(),
            seal: SealParams::default(),
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
    /// The counterparty's claimed start is far from this clock.
    ClockFar,
    /// No proximity channel passed.
    NoProximity,
    /// The two devices disagree on what was achieved.
    ChannelDisagreement,
    /// The engine produced a template of the wrong length.
    TemplateLength,
    /// A party refused to sign.
    Refused(Refusal),
    /// The proposed root is not the root of the agreed disclosure set.
    RootMismatch,
    /// The body does not carry this signer's own back-pointers.
    BackPointers,
    /// Every nominee declined.
    NoWitness,
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
    pub records: usize,
    pub sessions: usize,
    pub horizon: crate::horizon::Woke,
}

/// What one party says to another on the direct channel, or to a verifier
/// on a request stream.  `Grant` and `Query` are the only two that travel
/// to a verifier; everything else stays between the participants and
/// their witnesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    Intent(Intent),
    Channels(Vec<ChannelOutcome>),
    CaptureKey([u8; 32]),
    ConsentRequest(VerificationQuery),
    Consent {
        query_id: [u8; 32],
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
    WitnessRequest(WitnessRequest),
    WitnessAnswer(Option<u64>),
    BackPointers(Vec<Txid>),
    Proposal(Box<Proposed>),
    /// A signer's envelope entries over the body, or its refusal.
    Signed(Result<Vec<u8>, Refusal>),
    Record(Vec<u8>),
    /// The recovering subject names the prior key whose history it claims.
    ClaimPrior(Keyhash),
    /// A recovery verifier's signed response, hybrid, to the subject.
    RecoveryResponse(Vec<u8>),
    /// The assembled `Recovery` block and the old archive's head, to the
    /// patron.
    RecoveryProposal {
        block: Vec<u8>,
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
    /// The reply, from the serving node.
    PrekeyReply(Vec<u8>),
    /// The serving node's word that the pool ran dry.
    PoolExhausted,
    /// Bytes on the end-to-end channel, on the direct path to `to`.
    /// `device`: the recipient's device, carried so a failed direct path
    /// can fall back to the relay (`wire-format.md` §7.10 field 4).
    Payload {
        to: Keyhash,
        bytes: Vec<u8>,
        device: [u8; 32],
    },
    /// The same bytes handed to the serving node to relay to `to`.
    /// `device` is the recipient's device the ciphertext is for
    /// (`wire-format.md` §7.10 field 4).
    Relay {
        to: Keyhash,
        bytes: Vec<u8>,
        device: [u8; 32],
    },
    /// Application traffic to the serving node itself: it rides the
    /// transport session and needs no construction.
    Transport(Vec<u8>),
}

/// What the proposer shows every signer: the proposal, the disclosure set
/// its root commits to, and the back-pointers each signer supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    pub proposal: Proposal,
    pub set: DisclosureSet,
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
            | Msg::PrekeyRequest(b)
            | Msg::PrekeyReply(b)
            | Msg::Transport(b) => b.clone(),
            Msg::Payload { bytes, .. } | Msg::Relay { bytes, .. } => bytes.clone(),
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub contribution: [u8; 16],
    pub nominees: Vec<Keyhash>,
    pub bundle: Vec<Vec<u8>>,
    pub started_at: u64,
    pub retention_years: u64,
    pub initiator: bool,
}

/// What a nominee is told when asked to witness (design §7.1 step 8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessRequest {
    pub ceremony_id: [u8; 32],
    pub participants: [Keyhash; 2],
    pub started_at: u64,
    pub channels: Vec<ChannelOutcome>,
}

/// What a patron is proposing, beside who and into which subnet.
///
/// Grouped because the four move together: the evidence the binding rests
/// on, the series of the relationship line it opens, the head a recovering
/// subject presented, and the key material an adoption of a party the
/// patron is introducing carries.
pub struct Adopting {
    pub evidence: Evidence,
    pub series: u32,
    pub presented_head: Option<Txid>,
    pub key_material: Option<Vec<u8>>,
}

/// The state of the one ceremony this client is a participant in.
struct Active {
    counterparty: Keyhash,
    initiator: bool,
    started_at: u64,
    contribution: [u8; 16],
    ceremony_id: Option<[u8; 32]>,
    my_nominees: BTreeSet<Keyhash>,
    their_nominees: Vec<Keyhash>,
    their_bundle: Vec<Vec<u8>>,
    their_retention: u64,
    seed: [u8; 32],
    channels: Vec<ChannelOutcome>,
    strongest: Option<ChannelKind>,
    sealed: Option<SealedCapture>,
    /// The template of the counterparty, in memory for the queries and
    /// gone with the ceremony.
    template: Option<Vec<u8>>,
    image_count: u64,
    issued: BTreeSet<[u8; 32]>,
    responses: Vec<Vec<u8>>,
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
    pub known: Vec<Identity>,
    pub archive: Archive,
    pub store: ClientStore,
    pub subject: SubjectState,
    pub verifier: VerifierState,
    pub acquaintance: Acquaintance,
    pub cfg: Config,
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
    positions: BTreeMap<Keyhash, Locator>,
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
    outbox: Vec<Msg>,
    /// As a recovering subject: the responses gathered at recovery
    /// meetings, verified, awaiting the block.
    pub recovery_responses: Vec<Vec<u8>>,
    /// Payload confidentiality: material, sessions and what waits.
    pub payload: PayloadState,
    /// What this client keeps of its own horizon (design §15.1.1): a copy
    /// of what its serving node propagated, kept so a patron that is not
    /// answering can be routed around.
    pub horizon: Horizon,
    active: Option<Active>,
    witnessing: Option<WitnessRequest>,
}

/// What arrived on the end-to-end channel, delivered where it belongs
/// (design §14.2.4.6).
#[derive(Debug)]
pub enum Dispatched {
    Application(Vec<u8>),
    Grant(GrantOutcome),
    Late(Result<Txid, String>),
    /// The peer's candidates for the direct path, for the transport to
    /// dial.
    Candidates(Vec<u8>),
    /// A verifier's copy of its response about me: the query it answered,
    /// or why the copy was refused.
    ResponseCopy(Result<[u8; 32], String>),
    /// An archive fetch this client answered from its own archive: how
    /// many records went back, and whether more remain.  The reply is in
    /// the outbox.
    Served {
        records: usize,
        more: bool,
    },
    /// A reply to a fetch this client made: the records it verified into
    /// its own store, or why it took none of them.
    Fetched(Result<usize, String>),
}

/// The refusal a device holding no seed gives to an act of the identity
/// key.
const NO_SEED: &str = "this device holds no seed";

fn hex8(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

impl Client {
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
            horizon: Horizon::new(kh),
            cfg,
            device,
            positions: BTreeMap::new(),
            outbox: Vec::new(),
            catalog: crate::catalog::View::default(),
            browsing: None,
            fetching: BTreeMap::new(),
            rotation: None,
            recovery_responses: Vec::new(),
            active: None,
            witnessing: None,
        }
    }

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

    fn active(&mut self) -> Result<&mut Active, Abort> {
        self.active.as_mut().ok_or(Abort::NotActive)
    }

    fn random<const N: usize>(&self) -> [u8; N] {
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

    /// Step 1: announce intent to `counterparty`, nominating `nominees`
    /// from its neighbourhood.  The one question a ceremony asks the
    /// person is whether to start it.
    pub fn begin(
        &mut self,
        counterparty: Keyhash,
        nominees: Vec<Keyhash>,
        initiator: bool,
    ) -> Result<Intent, Abort> {
        if !self.device.operator.ask(&format!(
            "Start a presence ceremony with {}?",
            hex8(&counterparty)
        )) {
            return Err(Abort::Declined);
        }
        let contribution = self.random::<16>();
        let seed = self.random::<32>();
        let started_at = self.now_s();
        self.active = Some(Active {
            counterparty,
            initiator,
            started_at,
            contribution,
            ceremony_id: None,
            my_nominees: nominees.iter().copied().collect(),
            their_nominees: vec![],
            their_bundle: vec![],
            their_retention: 0,
            seed,
            channels: vec![],
            strongest: None,
            sealed: None,
            template: None,
            image_count: 0,
            issued: BTreeSet::new(),
            responses: vec![],
        });
        Ok(Intent {
            contribution,
            nominees,
            bundle: self.store.records.values().cloned().collect(),
            started_at,
            retention_years: self.cfg.retention_years,
            initiator,
        })
    }

    /// Take the counterparty's intent: its contribution fixes the
    /// pre-commitment with mine, and the window opens.  The responder
    /// adopts the initiator's start if it is within tolerance of its own.
    pub fn take_intent(&mut self, from: Keyhash, intent: &Intent) -> Result<[u8; 32], Abort> {
        let me = self.keyhash();
        let tolerance = self.cfg.clock_tolerance_s;
        let a = self.active()?;
        if a.counterparty != from {
            return Err(Abort::NotActive);
        }
        if intent.started_at.abs_diff(a.started_at) > tolerance {
            return Err(Abort::ClockFar);
        }
        if intent.initiator && !a.initiator {
            a.started_at = intent.started_at;
        }
        let cid = pre_commitment((&me, &a.contribution), (&from, &intent.contribution));
        a.ceremony_id = Some(cid);
        a.their_nominees = intent.nominees.clone();
        a.their_bundle = intent.bundle.clone();
        a.their_retention = intent.retention_years;
        self.subject.open_window(cid);
        Ok(cid)
    }

    pub fn ceremony_id(&self) -> Option<[u8; 32]> {
        self.active.as_ref().and_then(|a| a.ceremony_id)
    }

    /// Steps 3–4: run the proximity channels, strongest first, and keep
    /// what was achieved.
    pub fn proximity(&mut self) -> Result<Vec<ChannelOutcome>, Abort> {
        let peer = self.active()?.counterparty;
        let (outcomes, strongest) =
            crate::device::run_channels(self.device.proximity.as_ref(), &peer);
        let a = self.active()?;
        a.channels = outcomes.clone();
        a.strongest = strongest;
        if strongest.is_none() {
            return Err(Abort::NoProximity);
        }
        Ok(outcomes)
    }

    /// The counterparty's view of the channels: this device ran the same
    /// ones and must have seen the same strongest pass.
    pub fn take_channels(&mut self, theirs: &[ChannelOutcome]) -> Result<(), Abort> {
        let mine = self.proximity()?;
        let strongest = |v: &[ChannelOutcome]| {
            v.iter()
                .find(|o| o.result == ChannelResult::Pass)
                .map(|o| o.kind)
        };
        if strongest(&mine) != strongest(theirs) {
            return Err(Abort::ChannelDisagreement);
        }
        self.active()?.channels = theirs.to_vec();
        Ok(())
    }

    /// The key the counterparty seals its captures of me under (design
    /// §7.5.2.6), derived from my seed for this ceremony.
    pub fn capture_key(&self) -> Result<[u8; 32], Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        let cid = a.ceremony_id.ok_or(Abort::NotActive)?;
        Ok(capture_key(&a.seed, &self.keyhash(), &a.counterparty, &cid))
    }

    /// Step 5: tell the person what the record will contain and who can
    /// read it, run the guided capture of the counterparty, derive the
    /// template, seal both under the key the counterparty supplied, and
    /// let that key go.
    pub fn capture(&mut self, their_key: [u8; 32]) -> Result<(), Abort> {
        let me = self.keyhash();
        let (peer, cid) = {
            let a = self.active()?;
            (a.counterparty, a.ceremony_id.ok_or(Abort::NotActive)?)
        };
        self.device.notifier.notify(Notice::RecordDisclosure {
            role: Role::Participant,
        });
        let (frames, _prompts) = guided_capture(
            self.device.camera.as_ref(),
            self.device.clock.as_ref(),
            self.device.random.as_ref(),
            &self.cfg.capture,
        );
        let template = self.device.engine.template(&frames);
        if template.len() != self.cfg.seal.template_len {
            return Err(Abort::TemplateLength);
        }
        let capture = Capture {
            modality: 0,
            template_version: self.cfg.template_version,
            template: template.clone(),
            frames,
        };
        let sealed = seal(&self.cfg.seal, &their_key, peer, me, cid, &capture);
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
        let pool = selection::pool(
            &self.known,
            &a.their_bundle,
            &a.counterparty,
            &me,
            a.started_at,
        );
        if !pool.candidates.is_empty() && !self.acquaintance.recognises_any(&pool) {
            self.device.notifier.notify(Notice::NoCandidateRecognised);
        }
        let need = required(pool.n, pool.candidates.len());
        Ok(selection::select(&pool, &self.acquaintance, need, true))
    }

    /// The query to `verifier` about the counterparty: the fuzzed profile of
    /// the person in front of me, under this ceremony.
    pub fn query_for(&mut self, verifier: Keyhash) -> Result<VerificationQuery, Abort> {
        let me = self.keyhash();
        let version = self.cfg.template_version;
        let (subject, cid, template) = {
            let a = self.active.as_ref().ok_or(Abort::NotActive)?;
            (
                a.counterparty,
                a.ceremony_id.ok_or(Abort::NotActive)?,
                a.template.clone().ok_or(Abort::NotActive)?,
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
        self.active()?.issued.insert(q.query_id());
        Ok(q)
    }

    /// The request that carries a consented query to its verifier.
    pub fn request(
        &self,
        q: &VerificationQuery,
        consent: Vec<u8>,
        basis: SelectionBasis,
    ) -> Vec<u8> {
        QueryRequest {
            query: q.clone(),
            consent,
            selection_basis: basis as u64,
        }
        .encode()
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
        Some((consent, grant))
    }

    /// As verifier: a grant from `from`.
    pub fn take_grant(&mut self, from: Keyhash, bytes: &[u8]) -> GrantOutcome {
        let Some(me) = self.signer.as_deref() else {
            return GrantOutcome::Rejected(NO_SEED);
        };
        let cx = Verifying {
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
            me,
            ids: &self.known,
            store: &self.store,
            matcher: self.device.engine.matcher(),
        };
        self.verifier.expire(&cx, self.device.clock.now_ms())
    }

    /// As querier: a response to a query I issued, verified under its
    /// verifier.
    pub fn take_response(&mut self, bytes: &[u8]) -> Result<(), String> {
        let r = Response::read(bytes)?;
        verify::response(&self.known, bytes, false).map_err(|e| e.to_string())?;
        let a = self.active.as_mut().ok_or("no ceremony")?;
        if !a.issued.contains(&r.query_id) {
            return Err("not a query I issued".into());
        }
        a.responses.push(bytes.to_vec());
        Ok(())
    }

    /// As subject: the copy of a response about me.
    pub fn take_response_copy(&mut self, bytes: &[u8]) -> Result<[u8; 32], String> {
        let me = self.signer.as_deref().ok_or(NO_SEED)?;
        self.subject.take_response_copy(me, &self.known, bytes)
    }

    /// The responses I gathered about the counterparty.
    pub fn responses(&self) -> Vec<Vec<u8>> {
        self.active
            .as_ref()
            .map(|a| a.responses.clone())
            .unwrap_or_default()
    }

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

    /// What a nominee is asked to witness.
    pub fn witness_request(&self) -> Result<WitnessRequest, Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        Ok(WitnessRequest {
            ceremony_id: a.ceremony_id.ok_or(Abort::NotActive)?,
            participants: participants(self.keyhash(), a.counterparty),
            started_at: a.started_at,
            channels: a.channels.clone(),
        })
    }

    /// As nominee: witness, or decline.  The claimed start must be within
    /// tolerance of this clock (`light-client-requirements.md` §1.2); the
    /// person is told, not asked (design Appendix A.3).  The flags attest
    /// what was observed: the protocol ran, both were responsive, and the
    /// latency bound where a latency channel passed.
    pub fn take_witness_request(&mut self, req: &WitnessRequest) -> Option<u64> {
        witness_check(req.started_at, self.now_s(), self.cfg.clock_tolerance_s).ok()?;
        let latency = req
            .channels
            .iter()
            .any(|c| c.kind == ChannelKind::Latency && c.result == ChannelResult::Pass);
        self.witnessing = Some(req.clone());
        Some(3 | if latency { 4 } else { 0 })
    }

    pub fn back_pointers(&self) -> Vec<Txid> {
        self.archive.next_back_pointers()
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
        let salts: [[u8; 16]; 7] = std::array::from_fn(|_| self.random::<16>());
        let (retention, their_retention) =
            (self.cfg.retention_years, self.active()?.their_retention);
        let a = self.active()?;
        let parts = participants(me, a.counterparty);
        let (r0, r1) = if parts[0] == me {
            (retention, their_retention)
        } else {
            (their_retention, retention)
        };
        let channels: Vec<(u64, u64, Option<u64>)> = a
            .channels
            .iter()
            .map(|c| (c.kind.code(), c.result as u64, c.resolution_m))
            .collect();
        let values = [
            record::capture_value(0, a.image_count, 0, 1),
            record::empty_location_value(),
            record::integrity_value(false, 0),
            record::retention_value(r0),
            record::integrity_value(false, 0),
            record::retention_value(r1),
            record::proximity_value(&channels, a.strongest.ok_or(Abort::NoProximity)?.code()),
        ];
        let set = disclosures(values, salts);
        let mut responses = a.responses.clone();
        responses.extend(their_responses);
        sort_responses(&mut responses);
        let proposal = Proposal {
            started_at: a.started_at,
            finalized_at: now,
            participants: parts,
            witnesses,
            responses,
            root: disclosure_root(&set),
        };
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
            return Err(Abort::RootMismatch);
        }
        self.check_back(proposal, back)?;
        let held = self.subject.responses.clone();
        let mine = self
            .active
            .as_ref()
            .map(|a| a.my_nominees.clone())
            .unwrap_or_default();
        participant_check(&me, proposal, &mine, &held, self.device.notifier.as_ref())
            .map_err(Abort::Refused)?;
        Ok(self
            .signer()?
            .sign_entries(aad::ENVELOPE, &proposal.body(back)))
    }

    /// As witness: sign the ceremony I accepted, and only that one.
    pub fn witness_sign(
        &mut self,
        proposal: &Proposal,
        back: &[Vec<Txid>],
    ) -> Result<Vec<u8>, Abort> {
        let w = self.witnessing.as_ref().ok_or(Abort::NotActive)?;
        if w.started_at != proposal.started_at || w.participants != proposal.participants {
            return Err(Abort::NotActive);
        }
        self.check_back(proposal, back)?;
        Ok(self
            .signer()?
            .sign_entries(aad::ENVELOPE, &proposal.body(back)))
    }

    fn check_back(&self, proposal: &Proposal, back: &[Vec<Txid>]) -> Result<(), Abort> {
        let me = self.keyhash();
        let pos = proposal
            .signers()
            .iter()
            .position(|s| *s == me)
            .ok_or(Abort::NotActive)?;
        if back.get(pos) != Some(&self.archive.next_back_pointers()) {
            return Err(Abort::BackPointers);
        }
        Ok(())
    }

    /// Drop the ceremony under way without a record: nothing sealed is
    /// filed, the seed is gone with it.
    pub fn abandon(&mut self) {
        self.active = None;
        self.subject.responses.clear();
    }

    /// The record, finalized: appended to my archive and kept; as a
    /// participant, the sealed capture of the counterparty and my own seed
    /// are filed under it, the disclosure set is kept as record state, and
    /// the ceremony's window closes with everything it counted.
    pub fn finalize(
        &mut self,
        envelope: &[u8],
        set: Option<&DisclosureSet>,
    ) -> Result<Txid, Abort> {
        let rec = Record::parse(envelope).map_err(Abort::Record)?;
        verify::envelope(&self.known, envelope).map_err(|e| Abort::Record(e.to_string()))?;
        let txid = rec.txid;
        let finalized_at = rec.effective;
        self.archive.append(rec.clone()).map_err(Abort::Record)?;
        self.store.records.insert(txid, envelope.to_vec());
        if let Some(a) = self.active.take() {
            if let Some(sealed) = a.sealed {
                self.store.sealed.insert(txid, sealed);
            }
            self.store.seeds.insert(
                txid,
                OwnSeed {
                    seed: a.seed,
                    counterparty: a.counterparty,
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
        self.witnessing = None;
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
        self.propose_adoption_in(self.anchor(), node, node_back, what)
    }

    /// The same, naming the subnet to adopt into.
    ///
    /// **Which tree matters and is the patron's to say.** A patron in two
    /// subnets sits at a different path in each, and the locator it issues
    /// is its path in the one it is adopting into; adopting "somewhere"
    /// would put the subordinate at an address the other subnet cannot
    /// read.
    pub fn propose_adoption_in(
        &self,
        anchor: Keyhash,
        node: Keyhash,
        node_back: &[Txid],
        what: Adopting,
    ) -> Result<Vec<u8>, Abort> {
        let Adopting {
            evidence,
            series,
            presented_head,
            key_material,
        } = what;
        let pos = self
            .position_in(&anchor)
            .ok_or_else(|| Abort::PatronRefused(format!("no position under {}", hex8(&anchor))))?;
        let index = self.free_index(&anchor)?;
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
    fn free_index(&self, anchor: &Keyhash) -> Result<u8, Abort> {
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
            if patron != me || loc.anchor != *anchor || ended.contains(&node) {
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
        self.adopt_own_positions();
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
    pub fn install(&mut self, contents: backup::Contents) -> Result<usize, String> {
        if !self.archive.is_empty() {
            return Err("this client already holds an archive".into());
        }
        // **decryption establishes that the passphrase opened it, not that
        // this identity owns it.**  Every backup names its subject, and a
        // device holding no seed makes one too: a provider credential and
        // an evidence store are identity state whether or not the seeds
        // travel with them, so the owner is carried in its own right and
        // not inferred from what happens to be present.
        let named = contents.owner.or_else(|| {
            contents.seeds.map(|[ed, pq]| {
                rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq)
                    .public
                    .keyhash
            })
        });
        match named {
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
        self.store = contents.store;
        self.provider_credential = contents.provider;
        self.positions.clear();
        self.adopt_own_positions();
        Ok(n)
    }

    /// Everything a device loss would take away, enveloped under a key
    /// derived from `secret` (design §13.7.1).
    ///
    /// **The identity goes in.** The store writes no key; a backup is
    /// where the archive and the key are deliberately together, which is
    /// the aggregation §13.7.1 names and exactly why the blob is
    /// encrypted. Seeds absent where this client cannot supply them.
    pub fn export(
        &self,
        seeds: Option<[[u8; 32]; 2]>,
        cost: backup::Cost,
        secret: &[u8],
    ) -> Result<Vec<u8>, backup::Failure> {
        let contents = backup::Contents {
            owner: Some(self.public.keyhash),
            seeds,
            records: self.archive.records().map(|r| r.bytes.clone()).collect(),
            store: self.store.clone(),
            provider: self.provider_credential.clone(),
        };
        backup::export(&contents, &backup::Wrap::passphrase(cost), secret)
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
        secret: &[u8],
    ) -> Result<(backup::Contents, backup::Discarded), backup::Failure> {
        let mut contents = backup::import(blob, secret)?;
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
        // The position is where that is known: one subnet, one patron, so
        // the anchor answers it.
        let held = self
            .positions
            .get(&rel.position.anchor)
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
    pub fn outbox(&mut self) -> Vec<Msg> {
        std::mem::take(&mut self.outbox)
    }

    /// Where this client sits in the subnet `anchor` names.
    ///
    /// **A party with no ancestor names itself**, so asking for a position
    /// under this client's own key always answers: that is the
    /// self-anchor, not a missing value (`wire-format.md` §2.1).
    #[must_use]
    pub fn position_in(&self, anchor: &Keyhash) -> Option<Locator> {
        if let Some(p) = self.positions.get(anchor) {
            return Some(p.clone());
        }
        (*anchor == self.keyhash()).then(|| {
            Locator::root(
                self.keyhash(),
                Seqno {
                    series: 1,
                    counter: 0,
                },
            )
        })
    }

    /// Every subnet this client has a position in, its own first.
    #[must_use]
    pub fn anchors(&self) -> Vec<Keyhash> {
        let mut out = vec![self.keyhash()];
        out.extend(
            self.positions
                .keys()
                .copied()
                .filter(|a| *a != self.keyhash()),
        );
        out
    }

    /// The subnet this client acts in when nothing names one: the first it
    /// was adopted into, or its own while it has no patron.
    #[must_use]
    pub fn anchor(&self) -> Keyhash {
        self.positions
            .keys()
            .copied()
            .find(|a| *a != self.keyhash())
            .unwrap_or_else(|| self.keyhash())
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
        let mut held: BTreeMap<Keyhash, Locator> = BTreeMap::new();
        for rec in self.archive.records() {
            if rec.field_hash(1) != Some(me) {
                continue;
            }
            match rec.tx_type {
                TYPE_ADOPTION => {
                    if let (Some(patron), Some(loc)) = (rec.field_hash(2), rec.locator()) {
                        held.insert(patron, loc);
                    }
                }
                TYPE_DEPARTURE => {
                    if let Some(patron) = rec.field_hash(2) {
                        held.remove(&patron);
                    }
                }
                _ => {}
            }
        }
        self.positions = held.into_values().map(|l| (l.anchor, l)).collect();
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
        let now = self.now_s();
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let n = self.payload.cfg.pool_target;
        let keys = self.payload.keys.one_time_keys(n, &mut fresh);
        self.payload.pool_reported = n;
        let mut out: Vec<Msg> = self.bundle_to_publish(now).into_iter().collect();
        out.push(Msg::StockOneTime(keys));
        out.extend(self.sweep(population));
        out
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
        // §7.9): the envelope and its seven slots, nothing revealed unless
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
        // anything made since the last carry goes up with the routine
        // traffic, so a record whose own carry was missed is not stranded
        let mut out = self.outbox();
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
        vec![Msg::StockOneTime(keys)]
    }

    /// The serving node reports how many one-time keys remain, or that
    /// none do: below the threshold the pool is replenished at once.
    pub fn on_pool_report(&mut self, remaining: usize) -> Vec<Msg> {
        self.payload.pool_reported = remaining;
        if remaining < self.payload.cfg.replenish_below {
            self.restock()
        } else {
            Vec::new()
        }
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
            return Ok(vec![Msg::Transport(bytes.to_vec())]);
        }
        let plaintext = payload::wrap(kind, bytes);
        // a session is with a device (design §14.2.4): every device of the
        // recipient with a session open gets its own ciphertext, and every
        // device without one gets a session opened, on a one-time key
        // asked for that device (`wire-format.md` §7.8 field 4)
        let me = self.payload.device;
        let mut out = Vec::new();
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
        if self.device.direct.reachable(&to) {
            Msg::Payload { to, bytes, device }
        } else {
            Msg::Relay { to, bytes, device }
        }
    }

    /// A reply from the serving node: a sweep's bundles are kept; a
    /// one-time reply opens the session it was asked for, with the key
    /// where one came and on reusable material alone where none did, and
    /// sends what was waiting.
    pub fn take_prekey_reply(&mut self, bytes: &[u8]) -> Result<Vec<Msg>, String> {
        if let Ok(replies) = decode_batch_reply(bytes) {
            for r in replies {
                // one bundle per device (`wire-format.md` §7.8), each kept
                // under the device it names
                for b in &r.bundles {
                    if let Ok(p) = payload::read_bundle(&self.known, b) {
                        self.payload.sessions.prefetch(p);
                    }
                }
            }
            return Ok(Vec::new());
        }
        let r = PrekeyReply::decode(bytes)?;
        let Some((to, device)) = self.payload.outstanding.remove(&r.nonce) else {
            // a reusable-only reply: the bundles are kept, and nothing opens
            for b in &r.bundles {
                if let Ok(p) = payload::read_bundle(&self.known, b) {
                    self.payload.sessions.prefetch(p);
                }
            }
            return Ok(Vec::new());
        };
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

    /// What arrived on the end-to-end channel from `from`: decrypted on the
    /// session, or a session opened on my prekeys, and delivered by kind —
    /// a key grant to the grant handler, a late response beside its record,
    /// application payload to the application.
    pub fn receive_payload(&mut self, from: Keyhash, bytes: &[u8]) -> Result<Dispatched, String> {
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let plaintext =
            match self
                .payload
                .sessions
                .receive(&mut self.payload.keys, from, bytes, &mut fresh)
            {
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
            payload::KIND_CANDIDATES => Dispatched::Candidates(inner),
            payload::KIND_RESPONSE_COPY => {
                Dispatched::ResponseCopy(self.take_response_copy(&inner))
            }
            payload::KIND_ARCHIVE_REQUEST => self.serve_archive(from, &inner),
            payload::KIND_ARCHIVE_REPLY => {
                Dispatched::Fetched(self.take_archive_reply(from, &inner))
            }
            _ => Dispatched::Application(inner),
        })
    }
}

/// The two participants in body order: ascending keyhash.
pub fn participants(a: Keyhash, b: Keyhash) -> [Keyhash; 2] {
    if a < b { [a, b] } else { [b, a] }
}

/// One message on one path.
#[derive(Debug, Clone)]
pub struct Sent {
    pub from: Keyhash,
    pub to: Keyhash,
    pub msg: Msg,
}

/// An in-process carrier for the ceremony's conversations: every client
/// in one place, every message moved by hand and logged with its path, so
/// a test can say what travelled where.
#[derive(Default)]
pub struct Harness {
    pub clients: BTreeMap<Keyhash, Client>,
    pub log: Vec<Sent>,
}

impl Harness {
    pub fn add(&mut self, c: Client) -> Keyhash {
        let k = c.keyhash();
        self.clients.insert(k, c);
        k
    }

    pub fn client(&mut self, k: &Keyhash) -> &mut Client {
        self.clients.get_mut(k).expect("a client on the harness")
    }

    fn send(&mut self, from: Keyhash, to: Keyhash, msg: Msg) -> Msg {
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

    /// Run one ceremony between `a` (the initiator) and `b`, each
    /// nominating from the other's neighbourhood, to a record every signer
    /// holds.
    pub fn run(
        &mut self,
        a: Keyhash,
        b: Keyhash,
        a_nominees: Vec<Keyhash>,
        b_nominees: Vec<Keyhash>,
    ) -> Result<Txid, Abort> {
        // 1. intent
        let ia = self.client(&a).begin(b, a_nominees.clone(), true)?;
        let ib = self.client(&b).begin(a, b_nominees.clone(), false)?;
        let m = self.send(a, b, Msg::Intent(ia));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&b).take_intent(a, i)?;
        let m = self.send(b, a, Msg::Intent(ib));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&a).take_intent(b, i)?;
        // 3–4. proximity
        let ch = self.client(&a).proximity()?;
        let m = self.send(a, b, Msg::Channels(ch));
        let Msg::Channels(ch) = &m else {
            unreachable!()
        };
        self.client(&b).take_channels(ch)?;
        // 5. capture keys cross, then each captures the other
        let ka = self.client(&a).capture_key()?;
        let kb = self.client(&b).capture_key()?;
        self.send(a, b, Msg::CaptureKey(ka));
        self.send(b, a, Msg::CaptureKey(kb));
        self.client(&b).capture(ka)?;
        self.client(&a).capture(kb)?;
        // 6. each selects the other's verifiers and queries them
        self.queries(a, b)?;
        self.queries(b, a)?;
        // 8. witnesses
        let mut witnesses = Vec::new();
        for (nominator, nominees) in [(a, &a_nominees), (b, &b_nominees)] {
            let req = self.client(&nominator).witness_request()?;
            for w in nominees {
                let m = self.send(nominator, *w, Msg::WitnessRequest(req.clone()));
                let Msg::WitnessRequest(r) = &m else {
                    unreachable!()
                };
                let answer = self.client(w).take_witness_request(r);
                self.send(*w, nominator, Msg::WitnessAnswer(answer));
                if let Some(flags) = answer {
                    witnesses.push(Witness {
                        keyhash: *w,
                        nominated_by: nominator,
                        flags,
                    });
                }
            }
        }
        if witnesses.is_empty() {
            return Err(Abort::NoWitness);
        }
        // 7. proposal, review, signatures
        let theirs = self.client(&b).responses();
        let m = self.send(b, a, Msg::Responses(theirs));
        let Msg::Responses(theirs) = m else {
            unreachable!()
        };
        let (proposal, set) = self.client(&a).propose(theirs, witnesses)?;
        let signers = proposal.signers();
        let mut back = Vec::new();
        for s in &signers {
            let bp = self.client(s).back_pointers();
            let m = self.send(*s, a, Msg::BackPointers(bp));
            let Msg::BackPointers(bp) = m else {
                unreachable!()
            };
            back.push(bp);
        }
        let body = proposal.body(&back);
        let mut entries: Vec<(Keyhash, Vec<u8>)> = Vec::new();
        for s in &signers {
            let signed = if *s == a {
                self.client(&a).review_and_sign(&proposal, &set, &back)
            } else {
                self.send(
                    a,
                    *s,
                    Msg::Proposal(Box::new(Proposed {
                        proposal: proposal.clone(),
                        set: set.clone(),
                        back: back.clone(),
                    })),
                );
                if *s == b {
                    self.client(&b).review_and_sign(&proposal, &set, &back)
                } else {
                    self.client(s).witness_sign(&proposal, &back)
                }
            };
            let signed = match signed {
                Ok(e) => e,
                Err(Abort::Refused(r)) => {
                    self.send(*s, a, Msg::Signed(Err(r.clone())));
                    return Err(Abort::Refused(r));
                }
                Err(e) => return Err(e),
            };
            if *s != a {
                self.send(*s, a, Msg::Signed(Ok(signed.clone())));
            }
            entries.push((*s, signed));
        }
        let envelope = envelope_from_entries(TYPE_PRESENCE, &body, &entries);
        // finalized: every signer holds it
        let txid = self.client(&a).finalize(&envelope, Some(&set))?;
        for s in signers.iter().filter(|s| **s != a) {
            self.send(a, *s, Msg::Record(envelope.clone()));
            let set = if *s == b { Some(&set) } else { None };
            self.client(s).finalize(&envelope, set)?;
        }
        Ok(txid)
    }

    /// An ordinary adoption on the harness: `node` and `patron` meet in a
    /// ceremony, each nominating as given, and the patron adopts on that
    /// record.
    pub fn run_adoption(
        &mut self,
        node: Keyhash,
        patron: Keyhash,
        node_nominees: Vec<Keyhash>,
        patron_nominees: Vec<Keyhash>,
        series: u32,
    ) -> Result<Txid, Abort> {
        let pop = self.run(node, patron, node_nominees, patron_nominees)?;
        let node_back = self.client(&node).back_pointers();
        let body = self.client(&patron).propose_adoption(
            node,
            &node_back,
            Adopting {
                evidence: Evidence::Presence(pop),
                series,
                presented_head: None,
                key_material: None,
            },
        )?;
        let m = self.send(patron, node, Msg::AdoptionBody(body));
        let Msg::AdoptionBody(body) = m else {
            unreachable!()
        };
        let n_entries = self.client(&node).sign_body(&body).unwrap();
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
        let ia = self.client(&subject).begin(verifier, vec![], true)?;
        let ib = self.client(&verifier).begin(subject, vec![], false)?;
        let m = self.send(subject, verifier, Msg::Intent(ia));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&verifier).take_intent(subject, i)?;
        let m = self.send(verifier, subject, Msg::Intent(ib));
        let Msg::Intent(i) = &m else { unreachable!() };
        self.client(&subject).take_intent(verifier, i)?;
        let ch = self.client(&subject).proximity()?;
        let m = self.send(subject, verifier, Msg::Channels(ch));
        let Msg::Channels(ch) = &m else {
            unreachable!()
        };
        self.client(&verifier).take_channels(ch)?;
        let ks = self.client(&subject).capture_key()?;
        let kv = self.client(&verifier).capture_key()?;
        self.send(subject, verifier, Msg::CaptureKey(ks));
        self.send(verifier, subject, Msg::CaptureKey(kv));
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

    /// `selector` picks `subject`'s verifiers and queries each: the query
    /// goes to the subject for consent, the subject's grant goes to the
    /// verifier directly, the consented query goes to the verifier, and
    /// the verifier's answer goes to the selector with a copy to the
    /// subject.
    fn queries(&mut self, selector: Keyhash, subject: Keyhash) -> Result<(), Abort> {
        let picked = self.client(&selector).select_verifiers()?;
        for (v, basis) in picked {
            let q = self.client(&selector).query_for(v)?;
            let m = self.send(selector, subject, Msg::ConsentRequest(q.clone()));
            let Msg::ConsentRequest(q) = m else {
                unreachable!()
            };
            let Some((consent, grant)) = self.client(&subject).consent(&q) else {
                continue;
            };
            if let Some(g) = grant {
                let m = self.send(subject, v, Msg::Grant(g.encode()));
                let Msg::Grant(bytes) = m else { unreachable!() };
                self.client(&v).take_grant(subject, &bytes);
            }
            let m = self.send(
                subject,
                selector,
                Msg::Consent {
                    query_id: q.query_id(),
                    consent,
                },
            );
            let Msg::Consent { consent, .. } = m else {
                unreachable!()
            };
            let req = self.client(&selector).request(&q, consent, basis);
            let m = self.send(selector, v, Msg::Query(req));
            let Msg::Query(req) = m else { unreachable!() };
            let outcome = self.client(&v).take_query(selector, &req);
            let answers = match outcome {
                QueryOutcome::Answered(a) => vec![a],
                QueryOutcome::AwaitingGrant => self.client(&v).expire_now(),
                QueryOutcome::Closed(_) => vec![],
            };
            for ans in answers {
                let m = self.send(v, selector, Msg::Response(ans.to_querier.clone()));
                let Msg::Response(bytes) = m else {
                    unreachable!()
                };
                let _ = self.client(&selector).take_response(&bytes);
                let m = self.send(v, subject, Msg::ResponseCopy(ans.to_subject.1.clone()));
                let Msg::ResponseCopy(bytes) = m else {
                    unreachable!()
                };
                let _ = self.client(&subject).take_response_copy(&bytes);
            }
        }
        Ok(())
    }
}

impl Client {
    /// Let a query that will get no grant in this ceremony be answered
    /// now, `unavailable`: the harness has no later.
    fn expire_now(&mut self) -> Vec<crate::verifier::Answer> {
        let Some(me) = self.signer.as_deref() else {
            return Vec::new();
        };
        let cx = Verifying {
            me,
            ids: &self.known,
            store: &self.store,
            matcher: self.device.engine.matcher(),
        };
        let later = self.device.clock.now_ms() + self.cfg.verifier.grant_buffer_ms;
        self.verifier.expire(&cx, later)
    }
}
