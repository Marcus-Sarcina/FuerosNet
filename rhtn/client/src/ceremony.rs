//! The presence ceremony as a client runs it (design §7.1, §7.5.2,
//! §8.1.2; `light-client-requirements.md` §1): each party's decisions,
//! taken against what that party holds and sees, and the messages the
//! ceremony's direct channel carries between them.  Those messages are
//! carried by no wire object (§8.1.2's rule), so here they are values; an
//! in-process [`Harness`] moves them and records every path they take.

use crate::device::{ChannelKind, ChannelOutcome, ChannelResult, Device, guided_capture};
use crate::keys::{capture_key, pre_commitment};
use crate::notice::{Notice, Role};
use crate::query::{KeyGrant, QueryRequest, Response, VerificationQuery};
use crate::record::{self, DisclosureSet, Proposal, Refusal, disclosure_root, disclosures, participant_check, sort_responses, witness_check};
use crate::selection::{self, Acquaintance, SelectionBasis, required};
use crate::store::{Capture, ClientStore, OwnSeed, SealParams, SealedCapture, seal};
use crate::subject::{SubjectConfig, SubjectState};
use crate::verifier::{GrantOutcome, QueryOutcome, VerifierConfig, VerifierState, Verifying};
use crate::{Keyhash, Txid};
use rhtn_archive::chain::Archive;
use rhtn_archive::record::Record;
use rhtn_archive::tx::{TYPE_PRESENCE, Witness, envelope_from_entries};
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
}

impl Default for Config {
    fn default() -> Self {
        Config { clock_tolerance_s: 300, retention_years: 2, template_version: 1, capture: Default::default(), seal: SealParams::default(), subject: SubjectConfig::default(), verifier: VerifierConfig::default() }
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
            Msg::Grant(b) | Msg::Query(b) | Msg::Response(b) | Msg::ResponseCopy(b) | Msg::Record(b) => b.clone(),
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
    pub id: SigningIdentity,
    pub known: Vec<Identity>,
    pub archive: Archive,
    pub store: ClientStore,
    pub subject: SubjectState,
    pub verifier: VerifierState,
    pub acquaintance: Acquaintance,
    pub cfg: Config,
    pub device: Device,
    active: Option<Active>,
    witnessing: Option<WitnessRequest>,
}

fn hex8(k: &Keyhash) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

impl Client {
    pub fn new(id: SigningIdentity, known: Vec<Identity>, cfg: Config, device: Device) -> Self {
        let kh = id.public.keyhash;
        Client { id, known, archive: Archive::new(kh), store: ClientStore::default(), subject: SubjectState::new(cfg.subject.clone()), verifier: VerifierState::new(cfg.verifier.clone()), acquaintance: Acquaintance::default(), cfg, device, active: None, witnessing: None }
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
        Ok(txid)
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
