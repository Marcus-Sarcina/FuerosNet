//! The presence ceremony as a client runs it (design §7.1, §7.5.2,
//! §8.1.2; `light-client-requirements.md` §1): each party's decisions,
//! taken against what that party holds and sees, and the messages the
//! ceremony's direct channel carries between them.  Those messages are
//! carried by no wire object (§8.1.2's rule), so here they are values; an
//! in-process [`Harness`] moves them and records every path they take.

use crate::device::{ChannelKind, ChannelOutcome, ChannelResult, Device, guided_capture};
use crate::horizon::Horizon;
use crate::keys::{capture_key, pre_commitment};
use crate::notice::{Notice, Role};
use crate::payload::{self, PayloadError, PayloadState};
use crate::query::{KeyGrant, QueryRequest, Response, VerificationQuery};
use crate::record::{self, DisclosureSet, Proposal, Refusal, disclosure_root, disclosures, participant_check, sort_responses, witness_check};
use crate::rotation::Rotation;
use crate::selection::{self, Acquaintance, SelectionBasis, required};
use crate::store::{Capture, ClientStore, OwnSeed, SealParams, SealedCapture, seal};
use crate::subject::{SubjectConfig, SubjectState};
use crate::verifier::{GrantOutcome, QueryOutcome, VerifierConfig, VerifierState, Verifying};
use rhtn_archive::prekey::{PrekeyReply, decode_batch_reply};
use crate::{Keyhash, Txid};
use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{Adoption, Evidence, Locator, Seqno, TYPE_ADOPTION, TYPE_DEPARTURE, TYPE_PRESENCE, Witness, adoption_body, departure_body, envelope, envelope_from_entries, recovery_block, recovery_response_with_consent};
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
    Consent { query_id: [u8; 32], consent: Vec<u8> },
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
    RecoveryProposal { block: Vec<u8>, presented_head: Option<Txid> },
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
    Payload { to: Keyhash, bytes: Vec<u8> },
    /// The same bytes handed to the serving node to relay to `to`.
    Relay { to: Keyhash, bytes: Vec<u8> },
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
            Msg::Grant(b) | Msg::Query(b) | Msg::Response(b) | Msg::ResponseCopy(b) | Msg::Record(b) | Msg::RecoveryResponse(b) | Msg::AdoptionBody(b) | Msg::Seal(b) | Msg::PublishBundle(b) | Msg::PrekeyRequest(b) | Msg::PrekeyReply(b) | Msg::Transport(b) => b.clone(),
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
    /// Boxed: a signing identity carries its expanded post-quantum key
    /// inline, and a client is moved around a harness by value.
    pub id: Box<SigningIdentity>,
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
}

fn hex8(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

impl Client {
    pub fn new(id: SigningIdentity, known: Vec<Identity>, cfg: Config, device: Device) -> Self {
        let kh = id.public.keyhash;
        let now = device.clock.now_ms() / 1000;
        let random = device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let payload = PayloadState::new(cfg.payload.clone(), &mut fresh, now);
        Client { id: Box::new(id), known, archive: Archive::new(kh), store: ClientStore::default(), payload, subject: SubjectState::new(cfg.subject.clone()), verifier: VerifierState::new(cfg.verifier.clone()), acquaintance: Acquaintance::default(), horizon: Horizon::new(kh), cfg, device, positions: BTreeMap::new(), outbox: Vec::new(), catalog: crate::catalog::View::default(), browsing: None, rotation: None, recovery_responses: Vec::new(), active: None, witnessing: None }
    }

    pub fn keyhash(&self) -> Keyhash {
        self.id.public.keyhash
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
    pub fn begin(&mut self, counterparty: Keyhash, nominees: Vec<Keyhash>, initiator: bool) -> Result<Intent, Abort> {
        if !self.device.operator.ask(&format!("Start a presence ceremony with {}?", hex8(&counterparty))) {
            return Err(Abort::Declined);
        }
        let contribution = self.random::<16>();
        let seed = self.random::<32>();
        let started_at = self.now_s();
        self.active = Some(Active { counterparty, initiator, started_at, contribution, ceremony_id: None, my_nominees: nominees.iter().copied().collect(), their_nominees: vec![], their_bundle: vec![], their_retention: 0, seed, channels: vec![], strongest: None, sealed: None, template: None, image_count: 0, issued: BTreeSet::new(), responses: vec![] });
        Ok(Intent { contribution, nominees, bundle: self.store.records.values().cloned().collect(), started_at, retention_years: self.cfg.retention_years, initiator })
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
        let (outcomes, strongest) = crate::device::run_channels(self.device.proximity.as_ref(), &peer);
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
        let strongest = |v: &[ChannelOutcome]| v.iter().find(|o| o.result == ChannelResult::Pass).map(|o| o.kind);
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
        self.device.notifier.notify(Notice::RecordDisclosure { role: Role::Participant });
        let (frames, _prompts) = guided_capture(self.device.camera.as_ref(), self.device.clock.as_ref(), self.device.random.as_ref(), &self.cfg.capture);
        let template = self.device.engine.template(&frames);
        if template.len() != self.cfg.seal.template_len {
            return Err(Abort::TemplateLength);
        }
        let capture = Capture { modality: 0, template_version: self.cfg.template_version, template: template.clone(), frames };
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
        let pool = selection::pool(&self.known, &a.their_bundle, &a.counterparty, &me, a.started_at);
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
            (a.counterparty, a.ceremony_id.ok_or(Abort::NotActive)?, a.template.clone().ok_or(Abort::NotActive)?)
        };
        let profile = self.device.engine.profile(&template);
        let q = VerificationQuery { subject, querier: me, ceremony_id: cid, profile, template_version: version, verifier };
        self.active()?.issued.insert(q.query_id());
        Ok(q)
    }

    /// The request that carries a consented query to its verifier.
    pub fn request(&self, q: &VerificationQuery, consent: Vec<u8>, basis: SelectionBasis) -> Vec<u8> {
        QueryRequest { query: q.clone(), consent, selection_basis: basis as u64 }.encode()
    }

    /// Step 6, as subject: consent to a query about me, or not; and where I
    /// consent, the grant for its verifier, which goes to that verifier
    /// directly and to nobody else.
    pub fn consent(&mut self, q: &VerificationQuery) -> Option<(Vec<u8>, Option<KeyGrant>)> {
        let consent = self.subject.consent_to(&self.id, q, self.device.notifier.as_ref())?;
        let grant = self.subject.grant_for(&self.id, &self.store, &q.verifier, q.query_id(), self.now_s());
        Some((consent, grant))
    }

    /// As verifier: a grant from `from`.
    pub fn take_grant(&mut self, from: Keyhash, bytes: &[u8]) -> GrantOutcome {
        let cx = Verifying { me: &self.id, ids: &self.known, store: &self.store, matcher: self.device.engine.matcher() };
        self.verifier.take_grant(&cx, from, bytes, self.device.clock.now_ms())
    }

    /// As verifier: a query from `from`.  The person is told, at this
    /// moment, that answering records them in someone else's evidence
    /// (design §19.6); they are not asked.
    pub fn take_query(&mut self, from: Keyhash, bytes: &[u8]) -> QueryOutcome {
        self.device.notifier.notify(Notice::RecordDisclosure { role: Role::Verifier });
        let cx = Verifying { me: &self.id, ids: &self.known, store: &self.store, matcher: self.device.engine.matcher() };
        self.verifier.take_query(&cx, from, bytes, self.device.clock.now_ms())
    }

    /// As verifier: let the bounds pass by this clock.  A query whose grant
    /// never came within the buffer is answered `unavailable`; a grant
    /// whose query never came is dropped unopened.
    pub fn expire(&mut self) -> Vec<crate::verifier::Answer> {
        let cx = Verifying { me: &self.id, ids: &self.known, store: &self.store, matcher: self.device.engine.matcher() };
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
        self.subject.take_response_copy(&self.id, &self.known, bytes)
    }

    /// The responses I gathered about the counterparty.
    pub fn responses(&self) -> Vec<Vec<u8>> {
        self.active.as_ref().map(|a| a.responses.clone()).unwrap_or_default()
    }

    pub fn nominees(&self) -> (Vec<Keyhash>, Vec<Keyhash>) {
        self.active.as_ref().map(|a| (a.my_nominees.iter().copied().collect(), a.their_nominees.clone())).unwrap_or_default()
    }

    /// What a nominee is asked to witness.
    pub fn witness_request(&self) -> Result<WitnessRequest, Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        Ok(WitnessRequest { ceremony_id: a.ceremony_id.ok_or(Abort::NotActive)?, participants: participants(self.keyhash(), a.counterparty), started_at: a.started_at, channels: a.channels.clone() })
    }

    /// As nominee: witness, or decline.  The claimed start must be within
    /// tolerance of this clock (`light-client-requirements.md` §1.2); the
    /// person is told, not asked (design Appendix A.3).  The flags attest
    /// what was observed: the protocol ran, both were responsive, and the
    /// latency bound where a latency channel passed.
    pub fn take_witness_request(&mut self, req: &WitnessRequest) -> Option<u64> {
        witness_check(req.started_at, self.now_s(), self.cfg.clock_tolerance_s).ok()?;
        self.device.notifier.notify(Notice::RecordDisclosure { role: Role::Witness });
        let latency = req.channels.iter().any(|c| c.kind == ChannelKind::Latency && c.result == ChannelResult::Pass);
        self.witnessing = Some(req.clone());
        Some(3 | if latency { 4 } else { 0 })
    }

    pub fn back_pointers(&self) -> Vec<Txid> {
        self.archive.next_back_pointers()
    }

    /// Step 7, as proposer: the body everyone will sign.  Responses from
    /// both sides, sorted as the body requires; the disclosure set with
    /// fresh salts; the witnesses that accepted.
    pub fn propose(&mut self, their_responses: Vec<Vec<u8>>, witnesses: Vec<Witness>) -> Result<(Proposal, DisclosureSet), Abort> {
        let me = self.keyhash();
        let now = self.now_s();
        let salts: [[u8; 16]; 7] = std::array::from_fn(|_| self.random::<16>());
        let (retention, their_retention) = (self.cfg.retention_years, self.active()?.their_retention);
        let a = self.active()?;
        let parts = participants(me, a.counterparty);
        let (r0, r1) = if parts[0] == me { (retention, their_retention) } else { (their_retention, retention) };
        let channels: Vec<(u64, u64, Option<u64>)> = a.channels.iter().map(|c| (c.kind.code(), c.result as u64, c.resolution_m)).collect();
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
        let proposal = Proposal { started_at: a.started_at, finalized_at: now, participants: parts, witnesses, responses, root: disclosure_root(&set) };
        Ok((proposal, set))
    }

    /// The checks every signer makes on a proposal: the root is the root
    /// of the set shown, and the body carries my own back-pointers at my
    /// position.  Then the participant's own (`light-client-requirements.md`
    /// §1.1, §1.4), and the person is told where their nominees are absent
    /// or outnumbered.  Signing is the last thing that happens.
    pub fn review_and_sign(&mut self, proposal: &Proposal, set: &DisclosureSet, back: &[Vec<Txid>]) -> Result<Vec<u8>, Abort> {
        let me = self.keyhash();
        if proposal.root != disclosure_root(set) {
            return Err(Abort::RootMismatch);
        }
        self.check_back(proposal, back)?;
        let held = self.subject.responses.clone();
        let mine = self.active.as_ref().map(|a| a.my_nominees.clone()).unwrap_or_default();
        participant_check(&me, proposal, &mine, &held, self.device.notifier.as_ref()).map_err(Abort::Refused)?;
        Ok(self.id.sign_entries(aad::ENVELOPE, &proposal.body(back)))
    }

    /// As witness: sign the ceremony I accepted, and only that one.
    pub fn witness_sign(&mut self, proposal: &Proposal, back: &[Vec<Txid>]) -> Result<Vec<u8>, Abort> {
        let w = self.witnessing.as_ref().ok_or(Abort::NotActive)?;
        if w.started_at != proposal.started_at || w.participants != proposal.participants {
            return Err(Abort::NotActive);
        }
        self.check_back(proposal, back)?;
        Ok(self.id.sign_entries(aad::ENVELOPE, &proposal.body(back)))
    }

    fn check_back(&self, proposal: &Proposal, back: &[Vec<Txid>]) -> Result<(), Abort> {
        let me = self.keyhash();
        let pos = proposal.signers().iter().position(|s| *s == me).ok_or(Abort::NotActive)?;
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
    pub fn finalize(&mut self, envelope: &[u8], set: Option<&DisclosureSet>) -> Result<Txid, Abort> {
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
            self.store.seeds.insert(txid, OwnSeed { seed: a.seed, counterparty: a.counterparty, ceremony_id: a.ceremony_id.unwrap_or([0; 32]), finalized_at });
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
        self.rotation.as_ref().and_then(|r| r.old_key()).map(|k| k.public.keyhash)
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
    pub fn recognise(&mut self, q: &VerificationQuery, consent: &[u8], prior: Keyhash) -> Result<Vec<u8>, Abort> {
        let subject = q.subject;
        if !self.device.operator.ask(&format!("Do you recognise the person in front of you as the holder of {}?", hex8(&prior))) {
            return Err(Abort::NotRecognised);
        }
        self.device.notifier.notify(Notice::RecordDisclosure { role: Role::Verifier });
        Ok(recovery_response_with_consent(&self.id, &subject, &q.query_id(), consent, &prior))
    }

    /// As the recovering subject: a response to the query I consented to,
    /// verified hybrid under its verifier, naming my prior key.
    pub fn take_recovery_response(&mut self, bytes: &[u8]) -> Result<(), String> {
        let r = Response::read(bytes)?;
        let prior = self.prior_key().ok_or("no prior key")?;
        if r.subject != self.keyhash() || r.selection_basis != 0 {
            return Err("not about me as a recovery".into());
        }
        if !self.subject.consented(&self.ceremony_id().ok_or("no ceremony")?).contains(&r.query_id) {
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
        if !self.recovery_responses.iter().any(|r| Response::read(r).is_ok_and(|x| x.verdict == crate::query::Verdict::Match)) {
            return Err(Abort::NotRecognised);
        }
        Ok((recovery_block(old, &self.keyhash(), patron, self.recovery_responses.clone()), rot.old_head()))
    }

    /// As a patron: the adoption body for `node` under my position, the
    /// evidence checked against the body's own fields before anything is
    /// signed (`wire-format.md` §4.1).
    pub fn propose_adoption(&self, node: Keyhash, node_back: &[Txid], what: Adopting) -> Result<Vec<u8>, Abort> {
        self.propose_adoption_in(self.anchor(), node, node_back, what)
    }

    /// The same, naming the subnet to adopt into.
    ///
    /// **Which tree matters and is the patron's to say.** A patron in two
    /// subnets sits at a different path in each, and the locator it issues
    /// is its path in the one it is adopting into; adopting "somewhere"
    /// would put the subordinate at an address the other subnet cannot
    /// read.
    pub fn propose_adoption_in(&self, anchor: Keyhash, node: Keyhash, node_back: &[Txid], what: Adopting) -> Result<Vec<u8>, Abort> {
        let Adopting { evidence, series, presented_head, key_material } = what;
        let pos = self.position_in(&anchor).ok_or_else(|| Abort::PatronRefused(format!("no position under {}", hex8(&anchor))))?;
        let index = self.free_index(&anchor)?;
        let mut path = pos.path.clone();
        let nibbles = pos.nibbles + 1;
        if pos.nibbles.is_multiple_of(2) {
            path.push(index << 4);
        } else {
            let last = path.len() - 1;
            path[last] |= index;
        }
        let locator = Locator { anchor: pos.anchor, path, nibbles, seqno: Seqno { series, counter: 0 } };
        let back = self.archive.next_back_pointers();
        let a = Adoption { node, patron: self.keyhash(), locator, timestamp: self.now_s(), key_material, evidence, presented_head, back: [node_back, &back] };
        let body = adoption_body(&a);
        verify::adoption_evidence(&self.known, &body).map_err(|e| Abort::PatronRefused(e.to_string()))?;
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
        let ended: BTreeSet<Keyhash> =
            self.horizon.table.bindings().iter().filter(|b| b.patron == me && b.end.is_some()).map(|b| b.node).collect();
        let mut taken: BTreeSet<u8> = BTreeSet::new();
        for rec in self.archive.records().filter(|r| r.tx_type == TYPE_ADOPTION) {
            let (Some(patron), Some(node), Some(loc)) = (rec.field_hash(2), rec.field_hash(1), rec.locator()) else { continue };
            if patron != me || loc.anchor != *anchor || ended.contains(&node) {
                continue;
            }
            if let Some(i) = loc.slot() {
                taken.insert(i);
            }
        }
        (0..10).find(|i| !taken.contains(i)).ok_or_else(|| Abort::PatronRefused("all ten subordinate slots are taken (design §3.1: at most f = 10 subordinates)".into()))
    }

    /// Sign a body I proposed or was shown, as its subject or its patron.
    pub fn sign_body(&self, body: &[u8]) -> Vec<u8> {
        self.id.sign_entries(aad::ENVELOPE, body)
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
        let held = self.positions.get(&rel.position.anchor).ok_or(Abort::NoRelationship(patron))?;
        let counter = held.seqno.counter.checked_add(1).ok_or(Abort::CounterExhausted)?;
        let back = self.archive.next_back_pointers();
        let body = departure_body(&back, &me, &patron, Seqno { series: rel.series(), counter }, self.now_s(), reason);
        let env = envelope(TYPE_DEPARTURE, &body, &[&self.id]);
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
        (*anchor == self.keyhash()).then(|| Locator::root(self.keyhash(), Seqno { series: 1, counter: 0 }))
    }

    /// Every subnet this client has a position in, its own first.
    #[must_use]
    pub fn anchors(&self) -> Vec<Keyhash> {
        let mut out = vec![self.keyhash()];
        out.extend(self.positions.keys().copied().filter(|a| *a != self.keyhash()));
        out
    }

    /// The subnet this client acts in when nothing names one: the first it
    /// was adopted into, or its own while it has no patron.
    #[must_use]
    pub fn anchor(&self) -> Keyhash {
        self.positions.keys().copied().find(|a| *a != self.keyhash()).unwrap_or_else(|| self.keyhash())
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
        self.archive.chain_for(patron).map(|c| c.bytes()).unwrap_or_default()
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
        let bundle = self.payload.keys.bundle(&self.id, now);
        let n = self.payload.cfg.pool_target;
        let keys = self.payload.keys.one_time_keys(n, &mut fresh);
        self.payload.pool_reported = n;
        let mut out = vec![Msg::PublishBundle(bundle), Msg::StockOneTime(keys)];
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
                vec![Msg::PrekeyRequest(rhtn_archive::prekey::PrekeyRequest::One { subject: others[0], one_time: false, nonce }.encode())]
            }
            _ => vec![Msg::PrekeyRequest(payload::batch_request(&others, self.nonce()))],
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
        let Some(serving) = self.payload.serving else { return Vec::new() };
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
    pub fn take_catalog_reply(&mut self, bytes: &[u8]) -> Result<(crate::catalog::Step, Vec<Msg>), String> {
        let reply = rhtn_archive::catalog::CatalogReply::decode(bytes)?;
        let Some((node, mut sweep)) = self.browsing.take() else { return Err("no sweep is under way".into()) };
        let step = sweep.take(&self.known, self.catalog.portion(node), &reply, rhtn_codec::bounds::CATALOG_REPLY_ENTRIES);
        let out = match &step {
            crate::catalog::Step::Again(t) => {
                let q = sweep.query(Some(t.clone()), self.nonce());
                vec![Msg::CatalogQuery(q.encode())]
            }
            _ => Vec::new(),
        };
        // a sweep that asked again is still under way; one that is done,
        // truncated, or answered under a nonce it did not send is not
        if matches!(step, crate::catalog::Step::Again(_) | crate::catalog::Step::WrongNonce) {
            self.browsing = Some((node, sweep));
        }
        Ok((step, out))
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
            out.push(Msg::PublishBundle(self.payload.keys.bundle(&self.id, now)));
        }
        if self.payload.pool_reported < self.payload.cfg.replenish_below {
            out.extend(self.restock());
        }
        // bindings wanted for peers who wrote before this client held
        // theirs: reusable material only, one request each
        for subject in std::mem::take(&mut self.payload.wanted) {
            let nonce = self.nonce();
            out.push(Msg::PrekeyRequest(rhtn_archive::prekey::PrekeyRequest::One { subject, one_time: false, nonce }.encode()));
        }
        out
    }

    fn restock(&mut self) -> Vec<Msg> {
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let n = self.payload.cfg.pool_target.saturating_sub(self.payload.pool_reported);
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
        if remaining < self.payload.cfg.replenish_below { self.restock() } else { Vec::new() }
    }

    /// Send `bytes` of `kind` to `to` (design §14.2.4.1): to the serving
    /// node it rides the transport session; to a leaf it goes on the
    /// session held, or waits for the one-time key requested now, and
    /// takes the direct path where one exists and the relay otherwise.
    pub fn send_payload(&mut self, to: Keyhash, kind: u64, bytes: &[u8]) -> Result<Vec<Msg>, PayloadError> {
        if Some(to) == self.payload.serving {
            return Ok(vec![Msg::Transport(bytes.to_vec())]);
        }
        let plaintext = payload::wrap(kind, bytes);
        if self.payload.sessions.has_session(&to) {
            let m = self.payload.sessions.send(&to, &plaintext)?;
            return Ok(vec![self.route(to, m)]);
        }
        self.payload.pending.entry(to).or_default().push(plaintext);
        if self.payload.outstanding.values().any(|t| *t == to) {
            return Ok(Vec::new());
        }
        let nonce = self.nonce();
        self.payload.outstanding.insert(nonce, to);
        Ok(vec![Msg::PrekeyRequest(payload::one_time_request(to, nonce))])
    }

    fn route(&self, to: Keyhash, bytes: Vec<u8>) -> Msg {
        if self.device.direct.reachable(&to) { Msg::Payload { to, bytes } } else { Msg::Relay { to, bytes } }
    }

    /// A reply from the serving node: a sweep's bundles are kept; a
    /// one-time reply opens the session it was asked for, with the key
    /// where one came and on reusable material alone where none did, and
    /// sends what was waiting.
    pub fn take_prekey_reply(&mut self, bytes: &[u8]) -> Result<Vec<Msg>, String> {
        if let Ok(replies) = decode_batch_reply(bytes) {
            for r in replies {
                if let Some(b) = r.bundle
                    && let Ok(p) = payload::read_bundle(&self.known, &b) {
                        self.payload.sessions.prefetched.insert(p.subject, p);
                    }
            }
            return Ok(Vec::new());
        }
        let r = PrekeyReply::decode(bytes)?;
        let Some(to) = self.payload.outstanding.remove(&r.nonce) else {
            // a reusable-only reply: the bundle is kept, and nothing opens
            if let Some(b) = r.bundle
                && let Ok(p) = payload::read_bundle(&self.known, &b) {
                    self.payload.sessions.prefetched.insert(p.subject, p);
                }
            return Ok(Vec::new());
        };
        let their = match r.bundle {
            Some(b) => {
                let p = payload::read_bundle(&self.known, &b)?;
                if p.subject != to {
                    return Err("bundle for another subject".into());
                }
                self.payload.sessions.prefetched.insert(to, p.clone());
                p
            }
            None => self.payload.sessions.prefetched.get(&to).cloned().ok_or("no bundle for the peer")?,
        };
        let one_time = match r.one_time {
            Some(k) => Some(payload::OneTimeKey::decode(&k)?),
            None => None,
        };
        let mut waiting = self.payload.pending.remove(&to).unwrap_or_default();
        if waiting.is_empty() {
            waiting.push(payload::wrap(payload::KIND_APPLICATION, b""));
        }
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let first = self.payload.sessions.open(&self.payload.keys, to, &their, one_time.as_ref(), &mut fresh, &waiting[0]).map_err(|e| e.to_string())?;
        let mut out = vec![self.route(to, first)];
        for p in &waiting[1..] {
            let m = self.payload.sessions.send(&to, p).map_err(|e| e.to_string())?;
            out.push(self.route(to, m));
        }
        Ok(out)
    }

    /// What arrived on the end-to-end channel from `from`: decrypted on the
    /// session, or a session opened on my prekeys, and delivered by kind —
    /// a key grant to the grant handler, a late response beside its record,
    /// application payload to the application.
    pub fn receive_payload(&mut self, from: Keyhash, bytes: &[u8]) -> Result<Dispatched, String> {
        let random = self.device.random.clone();
        let mut fresh = |out: &mut [u8]| random.fill(out);
        let plaintext = match self.payload.sessions.receive(&mut self.payload.keys, from, bytes, &mut fresh) {
            Ok(p) => p,
            Err(e) => {
                // Nothing is opened and nothing dispatched under a name
                // this client cannot give the message.  Where the binding
                // is merely absent, it is asked for: the peer's next
                // attempt is attributable, and this one is not recovered.
                if matches!(e, payload::PayloadError::NoBundle) {
                    self.payload.wanted.insert(from);
                }
                if matches!(e, payload::PayloadError::NotTheSender | payload::PayloadError::NoBundle) {
                    self.device.notifier.notify(Notice::PayloadUnattributable { from });
                }
                return Err(e.to_string());
            }
        };
        let (kind, inner) = payload::unwrap(&plaintext)?;
        Ok(match kind {
            payload::KIND_KEY_GRANT => Dispatched::Grant(self.take_grant(from, &inner)),
            payload::KIND_LATE_RESPONSE => {
                let consented = self.subject.consented_by_ceremony().clone();
                Dispatched::Late(record::take_late_response(&mut self.store, &self.known, &inner, &consented))
            }
            payload::KIND_CANDIDATES => Dispatched::Candidates(inner),
            payload::KIND_RESPONSE_COPY => Dispatched::ResponseCopy(self.take_response_copy(&inner)),
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
        self.log.push(Sent { from, to, msg: msg.clone() });
        msg
    }

    /// The messages that travelled to `to`.
    pub fn to(&self, to: &Keyhash) -> Vec<&Sent> {
        self.log.iter().filter(|s| s.to == *to).collect()
    }

    /// Run one ceremony between `a` (the initiator) and `b`, each
    /// nominating from the other's neighbourhood, to a record every signer
    /// holds.
    pub fn run(&mut self, a: Keyhash, b: Keyhash, a_nominees: Vec<Keyhash>, b_nominees: Vec<Keyhash>) -> Result<Txid, Abort> {
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
        let Msg::Channels(ch) = &m else { unreachable!() };
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
                let Msg::WitnessRequest(r) = &m else { unreachable!() };
                let answer = self.client(w).take_witness_request(r);
                self.send(*w, nominator, Msg::WitnessAnswer(answer));
                if let Some(flags) = answer {
                    witnesses.push(Witness { keyhash: *w, nominated_by: nominator, flags });
                }
            }
        }
        if witnesses.is_empty() {
            return Err(Abort::NoWitness);
        }
        // 7. proposal, review, signatures
        let theirs = self.client(&b).responses();
        let m = self.send(b, a, Msg::Responses(theirs));
        let Msg::Responses(theirs) = m else { unreachable!() };
        let (proposal, set) = self.client(&a).propose(theirs, witnesses)?;
        let signers = proposal.signers();
        let mut back = Vec::new();
        for s in &signers {
            let bp = self.client(s).back_pointers();
            let m = self.send(*s, a, Msg::BackPointers(bp));
            let Msg::BackPointers(bp) = m else { unreachable!() };
            back.push(bp);
        }
        let body = proposal.body(&back);
        let mut entries: Vec<(Keyhash, Vec<u8>)> = Vec::new();
        for s in &signers {
            let signed = if *s == a {
                self.client(&a).review_and_sign(&proposal, &set, &back)
            } else {
                self.send(a, *s, Msg::Proposal(Box::new(Proposed { proposal: proposal.clone(), set: set.clone(), back: back.clone() })));
                if *s == b { self.client(&b).review_and_sign(&proposal, &set, &back) } else { self.client(s).witness_sign(&proposal, &back) }
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
    pub fn run_adoption(&mut self, node: Keyhash, patron: Keyhash, node_nominees: Vec<Keyhash>, patron_nominees: Vec<Keyhash>, series: u32) -> Result<Txid, Abort> {
        let pop = self.run(node, patron, node_nominees, patron_nominees)?;
        let node_back = self.client(&node).back_pointers();
        let body = self.client(&patron).propose_adoption(node, &node_back, Adopting { evidence: Evidence::Presence(pop), series, presented_head: None, key_material: None })?;
        let m = self.send(patron, node, Msg::AdoptionBody(body));
        let Msg::AdoptionBody(body) = m else { unreachable!() };
        let n_entries = self.client(&node).sign_body(&body);
        self.send(node, patron, Msg::Signed(Ok(n_entries.clone())));
        let p_entries = self.client(&patron).sign_body(&body);
        let envelope = envelope_from_entries(TYPE_ADOPTION, &body, &[(node, n_entries), (patron, p_entries)]);
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
    pub fn run_recovery_meeting(&mut self, subject: Keyhash, verifier: Keyhash) -> Result<Vec<u8>, Abort> {
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
        let Msg::Channels(ch) = &m else { unreachable!() };
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
        let Msg::ConsentRequest(q) = m else { unreachable!() };
        let (consent, _) = self.client(&subject).consent(&q).ok_or(Abort::NotRecognised)?;
        let m = self.send(subject, verifier, Msg::Consent { query_id: q.query_id(), consent });
        let Msg::Consent { consent, .. } = m else { unreachable!() };
        let resp = self.client(&verifier).recognise(&q, &consent, prior)?;
        let m = self.send(verifier, subject, Msg::RecoveryResponse(resp));
        let Msg::RecoveryResponse(resp) = m else { unreachable!() };
        self.client(&subject).take_recovery_response(&resp).map_err(Abort::Record)?;
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
    pub fn run_recovery_adoption(&mut self, subject: Keyhash, patron: Keyhash, series: u32) -> Result<Txid, Abort> {
        let (block, head) = self.client(&subject).recovery_block(&patron)?;
        let m = self.send(subject, patron, Msg::RecoveryProposal { block, presented_head: head });
        let Msg::RecoveryProposal { block, presented_head } = m else { unreachable!() };
        let node_back = self.client(&subject).back_pointers();
        let km = self.client(&subject).id.public.key_material();
        let body = self.client(&patron).propose_adoption(subject, &node_back, Adopting { evidence: Evidence::Recovery(block), series, presented_head, key_material: Some(km) })?;
        let m = self.send(patron, subject, Msg::AdoptionBody(body));
        let Msg::AdoptionBody(body) = m else { unreachable!() };
        let s_entries = self.client(&subject).sign_body(&body);
        self.send(subject, patron, Msg::Signed(Ok(s_entries.clone())));
        let p_entries = self.client(&patron).sign_body(&body);
        let envelope = envelope_from_entries(TYPE_ADOPTION, &body, &[(subject, s_entries), (patron, p_entries)]);
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
            let Msg::ConsentRequest(q) = m else { unreachable!() };
            let Some((consent, grant)) = self.client(&subject).consent(&q) else { continue };
            if let Some(g) = grant {
                let m = self.send(subject, v, Msg::Grant(g.encode()));
                let Msg::Grant(bytes) = m else { unreachable!() };
                self.client(&v).take_grant(subject, &bytes);
            }
            let m = self.send(subject, selector, Msg::Consent { query_id: q.query_id(), consent });
            let Msg::Consent { consent, .. } = m else { unreachable!() };
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
                let Msg::Response(bytes) = m else { unreachable!() };
                let _ = self.client(&selector).take_response(&bytes);
                let m = self.send(v, subject, Msg::ResponseCopy(ans.to_subject.1.clone()));
                let Msg::ResponseCopy(bytes) = m else { unreachable!() };
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
        let cx = Verifying { me: &self.id, ids: &self.known, store: &self.store, matcher: self.device.engine.matcher() };
        let later = self.device.clock.now_ms() + self.cfg.verifier.grant_buffer_ms;
        self.verifier.expire(&cx, later)
    }
}
