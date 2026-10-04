//! The ceremony's conversation on the end-to-end path (`wire-format.md`
//! §7.10.1 kinds 9 to 18; design §7.1; `light-client-requirements.md`
//! §1.1, §1.2): the sequence as a participant drives it and as a witness
//! observes it, over the sessions and the courier the query path uses.
//!
//! **Four steps are the participant's to take**, each returning what the
//! courier carries.  [`Client::converse_open`] asks every nominee to
//! witness and hands over this signer's back-pointers; it comes first, so
//! a witness is holding the ceremony before anything else of it arrives,
//! which is what lets it attribute the rest.  [`Client::converse_queries`]
//! puts the consent requests to the subject.  [`Client::converse_gathered`]
//! hands the proposer the responses the other side gathered, once its
//! queries are answered.  [`Client::converse_propose`] shows every signer
//! the body.  Everything else answers what arrived: [`Client::converse_in`]
//! takes a kind from the counterparty or a witness and puts what the step
//! owes in the outbox, which the courier empties after each delivery.  No
//! timer runs here: when to propose, and so which witness answered within
//! the ceremony, is the proposer's own call.
//!
//! **Who hears what** is §7.10.1's rule: each one a participant sends goes
//! to its counterparty and to every witness either nominated, the consent
//! request alone to the subject, the body and the record to every signer.
//! A witness answers the two participants it knows from the request; it
//! learns no other witness until the body names them.
//!
//! **The initiator proposes.** Both intents carry which side began
//! (`wire-format.md` §14.3.2), so the two agree on it without a message.
//! A witness answers every request it is sent and the proposer attributes
//! the answer to the side whose nominee list names the witness, since the
//! answer itself names nobody (§7.10.2).

use crate::ceremony::{
    Abort, Client, Msg, Proposed, WitnessRequest, abort, conversation_from, conversation_payload,
};
use crate::payload::{self, PayloadError};
use crate::query::{Response, VerificationQuery};
use crate::record::Refusal;
use crate::selection::SelectionBasis;
use crate::{Keyhash, Txid};
use rhtn_archive::record::Record;
use rhtn_archive::tx::{TYPE_PRESENCE, Witness, envelope_from_entries};
use std::collections::{BTreeMap, BTreeSet};

/// How many ceremonies a client holds as a witness at once: the holder's
/// own bound, as the other bounds are.
pub const PENDING_WITNESS_REQUESTS: usize = 8;

/// The participant's side of the conversation under way: what it put out
/// and what came back, by sender.
#[derive(Default)]
pub(crate) struct Conversation {
    /// Queries issued and awaiting the subject's consent, by id: the
    /// query and the basis it was selected on.
    pub(crate) awaiting_consent: BTreeMap<[u8; 32], (VerificationQuery, SelectionBasis)>,
    /// Witness answers received (kind 13), by witness.
    pub(crate) answers: BTreeMap<Keyhash, Option<u64>>,
    /// The counterparty's gathered responses (kind 14), once received.
    pub(crate) their_responses: Option<Vec<Vec<u8>>>,
    /// Back-pointers received (kind 15), by signer.
    pub(crate) back: BTreeMap<Keyhash, Vec<Txid>>,
    /// What this side proposed, or was shown (kind 16).
    pub(crate) proposed: Option<Proposed>,
    /// At the proposer: each signer's entries (kind 17).
    pub(crate) entries: BTreeMap<Keyhash, Vec<u8>>,
    /// At the proposer: the signers that refused (kind 17).
    pub(crate) refused: BTreeMap<Keyhash, Refusal>,
}

/// One ceremony this client was asked to witness and accepted.
pub(crate) struct Witnessing {
    pub(crate) request: WitnessRequest,
    /// The attestation bits answered with (Witness field 3).
    pub(crate) flags: u64,
    /// The conversation as it reached this witness: each sender and the
    /// kinds it sent.  A set, since what is attested is that a kind
    /// arrived from a party and not how often, and so bounded by the two
    /// participants and the ten kinds whatever a participant repeats.
    pub(crate) observed: BTreeSet<(Keyhash, u64)>,
}

/// What a message of the conversation came to at the client that opened
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conversed {
    /// A consent request about me: consented, the reply and any grant
    /// sent; or not.
    Consent { query: [u8; 32], consented: bool },
    /// The subject consented to a query I issued: it went to its verifier.
    Consented { query: [u8; 32] },
    /// A nominee answered: the flags it will sign under, or a decline.
    WitnessAnswer {
        witness: Keyhash,
        flags: Option<u64>,
    },
    /// Asked to witness: the flags this client answered with, or a
    /// decline.
    Asked {
        ceremony: [u8; 32],
        flags: Option<u64>,
    },
    /// The counterparty's gathered responses, at the proposer.
    Gathered { responses: usize },
    /// A signer's back-pointers, at the proposer.
    BackPointers { signer: Keyhash },
    /// Shown the body: signed, or refused; the reply went either way.
    Reviewed { refused: Option<Refusal> },
    /// A signer replied, at the proposer: its entries, or its refusal.
    Signed {
        signer: Keyhash,
        refused: Option<Refusal>,
    },
    /// The record finalized and held.
    Finalized { txid: Txid },
    /// Observed and kept, as a witness or as the counterparty: nothing
    /// owed in answer.
    Observed { kind: u64 },
    /// Not taken: from no party to a ceremony this client is in or
    /// witnesses, a body that does not read, or a step this client cannot
    /// take.
    Refused { kind: u64, why: String },
}

impl std::fmt::Display for Conversed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Conversed::Consent {
                consented: true, ..
            } => write!(f, "consented to a query about me"),
            Conversed::Consent {
                consented: false, ..
            } => write!(f, "declined a query about me"),
            Conversed::Consented { .. } => {
                write!(f, "the subject consented; the query went to its verifier")
            }
            Conversed::WitnessAnswer {
                witness,
                flags: Some(_),
            } => write!(f, "witness {} will attest", hex8(witness)),
            Conversed::WitnessAnswer {
                witness,
                flags: None,
            } => write!(f, "witness {} declined", hex8(witness)),
            Conversed::Asked { flags: Some(_), .. } => write!(f, "asked to witness, and will"),
            Conversed::Asked { flags: None, .. } => write!(f, "asked to witness, and declined"),
            Conversed::Gathered { responses } => {
                write!(f, "the counterparty's {responses} responses arrived")
            }
            Conversed::BackPointers { signer } => {
                write!(f, "back-pointers from {}", hex8(signer))
            }
            Conversed::Reviewed { refused: None } => write!(f, "the body reviewed and signed"),
            Conversed::Reviewed { refused: Some(r) } => write!(f, "the body refused: {r:?}"),
            Conversed::Signed {
                signer,
                refused: None,
            } => write!(f, "signed by {}", hex8(signer)),
            Conversed::Signed {
                signer,
                refused: Some(r),
            } => write!(f, "{} refused: {r:?}", hex8(signer)),
            Conversed::Finalized { txid } => write!(f, "the record is finalised: {}", hex8(txid)),
            Conversed::Observed { kind } => write!(f, "kind {kind} observed"),
            Conversed::Refused { kind, why } => write!(f, "kind {kind} refused: {why}"),
        }
    }
}

/// The participant's view of where its conversation stands, for whoever
/// decides when to propose.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Progress {
    /// Whether this side proposes.
    pub proposer: bool,
    /// Queries issued and not yet answered.
    pub queries_outstanding: usize,
    /// Nominees that answered: the flags each will sign under, or a
    /// decline.
    pub answered: Vec<(Keyhash, Option<u64>)>,
    /// Signers whose back-pointers are held.
    pub back_from: Vec<Keyhash>,
    /// Whether the counterparty's gathered responses are held.
    pub their_responses: bool,
    /// Whether the body is out, or has been shown.
    pub proposed: bool,
    /// Signers whose entries are held, at the proposer.
    pub signed: Vec<Keyhash>,
    /// Signers that refused, at the proposer.
    pub refused: Vec<Keyhash>,
}

fn hex8(k: &[u8; 32]) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// Who a message of the conversation is from, seen from a participant.
enum From {
    Counterparty,
    Witness,
    /// Nobody in my ceremony: a participant of one I witness, or a
    /// stranger.
    Elsewhere,
}

impl Client {
    /// The counterparty and every witness either side nominated, besides
    /// me: who hears each message of the conversation.
    fn conversation_parties(&self) -> Result<(Keyhash, Vec<Keyhash>), Abort> {
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        let me = self.keyhash();
        let witnesses: BTreeSet<Keyhash> = a
            .my_nominees
            .iter()
            .chain(a.their_nominees.iter())
            .copied()
            .filter(|k| *k != me && Some(*k) != a.counterparty)
            .collect();
        Ok((a.peer()?, witnesses.into_iter().collect()))
    }

    /// One message of the conversation to each of `to`, through the
    /// sessions held or opened: what the courier carries, and who could
    /// not be sent to.  A recipient no bundle is held for is skipped: a
    /// witness that cannot be reached is one that does not appear
    /// (design §7.1).
    fn converse_to(
        &mut self,
        msg: &Msg,
        to: &[Keyhash],
    ) -> (Vec<Msg>, Vec<(Keyhash, PayloadError)>) {
        let Some((kind, bytes)) = conversation_payload(msg) else {
            return (Vec::new(), Vec::new());
        };
        let mut out = Vec::new();
        let mut failed = Vec::new();
        for k in to {
            match self.send_payload(*k, kind, &bytes) {
                Ok(m) => out.extend(m),
                Err(e) => {
                    tracing::warn!(
                        target: "pay",
                        to = %crate::diag::id8(k),
                        kind = crate::diag::kind_name(kind),
                        error = %e,
                        "pay.send.failed"
                    );
                    failed.push((*k, e));
                }
            }
        }
        (out, failed)
    }

    /// The same, where the counterparty not hearing it stops the step.
    fn converse_or_abort(
        &mut self,
        msg: &Msg,
        to: &[Keyhash],
        counterparty: Keyhash,
    ) -> Result<Vec<Msg>, Abort> {
        let (out, failed) = self.converse_to(msg, to);
        if let Some((_, e)) = failed.iter().find(|(k, _)| *k == counterparty) {
            return Err(Abort::Payload(e.to_string()));
        }
        Ok(out)
    }

    /// The same, from inside a delivery: what goes out waits in the
    /// outbox for the courier.
    fn converse_into_outbox(&mut self, msg: &Msg, to: &[Keyhash]) {
        let (out, _) = self.converse_to(msg, to);
        self.outbox.extend(out);
    }

    /// Everyone a participant's message goes to: the counterparty and
    /// every witness either nominated.
    fn everyone(&self) -> Result<(Keyhash, Vec<Keyhash>), Abort> {
        let (counterparty, mut all) = self.conversation_parties()?;
        all.insert(0, counterparty);
        Ok((counterparty, all))
    }

    /// Step 6 on the path (design §7.1): select the counterparty's
    /// verifiers and put each query to the counterparty for consent, as
    /// kind 9 to the subject alone.  What comes back as kind 10 sends the
    /// consented query to its verifier.
    pub fn converse_queries(&mut self) -> Result<Vec<Msg>, Abort> {
        let (counterparty, _) = self.conversation_parties()?;
        let picked = self.select_verifiers()?;
        let mut out = Vec::new();
        for (v, basis) in picked {
            let q = self.query_for(v)?;
            let qid = q.query_id();
            out.extend(self.converse_or_abort(
                &Msg::ConsentRequest(q.clone()),
                &[counterparty],
                counterparty,
            )?);
            self.active()?
                .conversation
                .awaiting_consent
                .insert(qid, (q, basis));
        }
        Ok(out)
    }

    /// Open the conversation: ask every nominee to witness (kind 12) and
    /// hand over this signer's back-pointers (kind 15), both to the
    /// counterparty and every witness.  First, after the capture: a
    /// witness holds the ceremony from here and observes what follows
    /// (design §7.1 step 8).
    pub fn converse_open(&mut self) -> Result<Vec<Msg>, Abort> {
        let (counterparty, all) = self.everyone()?;
        let req = self.witness_request()?;
        let mut out = self.converse_or_abort(&Msg::WitnessRequest(req), &all, counterparty)?;
        let back = self.back_pointers();
        out.extend(self.converse_or_abort(&Msg::BackPointers(back), &all, counterparty)?);
        Ok(out)
    }

    /// Hand the proposer the responses gathered (kind 14), to the
    /// counterparty and every witness: the side that did not begin calls
    /// this once its queries are answered, or as many as will be.  The
    /// proposer holds its own and sends nothing.
    pub fn converse_gathered(&mut self) -> Result<Vec<Msg>, Abort> {
        let (counterparty, all) = self.everyone()?;
        if self.active()?.initiator {
            return Ok(Vec::new());
        }
        let gathered = self.responses();
        self.converse_or_abort(&Msg::Responses(gathered), &all, counterparty)
    }

    /// Step 7 on the path, the proposer's: the body from the responses
    /// both sides gathered and the witnesses that accepted, signed here
    /// and shown to every other signer (kind 16).  A witness is in it
    /// where its answer and its back-pointers have both arrived; one
    /// that has not answered by now does not appear (design §7.1).
    /// Waits, and says so, on the counterparty's responses.
    pub fn converse_propose(&mut self) -> Result<Vec<Msg>, Abort> {
        let me = self.keyhash();
        let (counterparty, _) = self.conversation_parties()?;
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        if !a.initiator {
            return Err(abort("converse_propose", Abort::NotProposer));
        }
        if a.conversation.proposed.is_some() {
            return Err(abort(
                "converse_propose",
                Abort::Waiting("the body is out and its signatures are awaited"),
            ));
        }
        let theirs = a.conversation.their_responses.clone().ok_or_else(|| {
            abort(
                "converse_propose",
                Abort::Waiting("the counterparty's responses"),
            )
        })?;
        let mut witnesses = Vec::new();
        for (w, flags) in &a.conversation.answers {
            let Some(flags) = flags else { continue };
            if !a.conversation.back.contains_key(w) {
                continue;
            }
            let nominated_by = if a.my_nominees.contains(w) {
                me
            } else if a.their_nominees.contains(w) {
                counterparty
            } else {
                continue;
            };
            witnesses.push(Witness {
                keyhash: *w,
                nominated_by,
                flags: *flags,
            });
        }
        if witnesses.is_empty() {
            return Err(abort("converse_propose", Abort::NoWitness));
        }
        let a = self.active.as_ref().ok_or(Abort::NotActive)?;
        if !a.conversation.back.contains_key(&counterparty) {
            return Err(abort(
                "converse_propose",
                Abort::Waiting("the counterparty's back-pointers"),
            ));
        }
        let (proposal, set) = self.propose(theirs, witnesses)?;
        let signers = proposal.signers();
        let mut back = Vec::with_capacity(signers.len());
        for s in &signers {
            if *s == me {
                back.push(self.back_pointers());
                continue;
            }
            let a = self.active.as_ref().ok_or(Abort::NotActive)?;
            back.push(a.conversation.back.get(s).cloned().ok_or_else(|| {
                abort(
                    "converse_propose",
                    Abort::Waiting("a signer's back-pointers"),
                )
            })?);
        }
        let entries = self.review_and_sign(&proposal, &set, &back)?;
        let proposed = Proposed {
            proposal,
            set,
            back,
        };
        let others: Vec<Keyhash> = signers.into_iter().filter(|s| *s != me).collect();
        let out = self.converse_or_abort(
            &Msg::Proposal(Box::new(proposed.clone())),
            &others,
            counterparty,
        )?;
        let a = self.active()?;
        a.conversation.proposed = Some(proposed);
        a.conversation.entries.insert(me, entries);
        Ok(out)
    }

    /// Where the conversation stands, for whoever decides the next step;
    /// nothing where no ceremony is under way.
    pub fn progress(&self) -> Option<Progress> {
        let a = self.active.as_ref()?;
        let answered: BTreeSet<[u8; 32]> = a
            .responses
            .iter()
            .filter_map(|r| Response::read(r).ok().map(|r| r.query_id))
            .collect();
        let c = &a.conversation;
        Some(Progress {
            proposer: a.initiator,
            queries_outstanding: a.issued.iter().filter(|q| !answered.contains(*q)).count(),
            answered: c.answers.iter().map(|(k, f)| (*k, *f)).collect(),
            back_from: c.back.keys().copied().collect(),
            their_responses: c.their_responses.is_some(),
            proposed: c.proposed.is_some(),
            signed: c.entries.keys().copied().collect(),
            refused: c.refused.keys().copied().collect(),
        })
    }

    /// A message of the conversation from `from`, opened on its session:
    /// into the ceremony this client is a participant in where `from` is a
    /// party to it, into the witness's side otherwise.  A party is the
    /// counterparty or a witness either side nominated; from a witness
    /// only the kinds a witness sends are taken.
    pub(crate) fn converse_in(&mut self, from: Keyhash, kind: u64, bytes: &[u8]) -> Conversed {
        let msg = match conversation_from(kind, bytes) {
            Ok(m) => m,
            Err(e) => {
                return Conversed::Refused {
                    kind,
                    why: format!("{e:?}"),
                };
            }
        };
        let party = match self.active.as_ref() {
            Some(a) if a.counterparty == Some(from) => From::Counterparty,
            Some(a) if a.my_nominees.contains(&from) || a.their_nominees.contains(&from) => {
                From::Witness
            }
            _ => From::Elsewhere,
        };
        let out = match party {
            From::Counterparty => self.counterparty_said(from, kind, msg),
            From::Witness => self.witness_said(from, kind, msg),
            From::Elsewhere => self.as_witness(from, kind, msg),
        };
        // a message from no party to a ceremony this client is in or
        // witnesses, or one it could not take: dropped, and said so
        if let Conversed::Refused { why, .. } = &out {
            tracing::warn!(
                target: "pay",
                from = %crate::diag::id8(&from),
                kind = crate::diag::kind_name(kind),
                why = %why,
                "pay.stranger"
            );
        }
        out
    }

    fn counterparty_said(&mut self, from: Keyhash, kind: u64, msg: Msg) -> Conversed {
        let refused = |why: String| Conversed::Refused { kind, why };
        let Ok((_, all)) = self.everyone() else {
            return refused("no ceremony".into());
        };
        let initiator = self.active.as_ref().is_some_and(|a| a.initiator);
        match msg {
            // step 6, as subject: consent or not; the grant to the
            // verifier directly, the consent to the querier and every
            // witness
            Msg::ConsentRequest(q) => {
                let qid = q.query_id();
                let Some((consent, grant)) = self.consent(&q) else {
                    return Conversed::Consent {
                        query: qid,
                        consented: false,
                    };
                };
                if let Some(g) = grant
                    && let Ok(m) =
                        self.send_payload(q.verifier, payload::KIND_KEY_GRANT, &g.encode())
                {
                    self.outbox.extend(m);
                }
                self.converse_into_outbox(
                    &Msg::Consent {
                        query_id: qid,
                        consent,
                    },
                    &all,
                );
                Conversed::Consent {
                    query: qid,
                    consented: true,
                }
            }
            // step 6, as selector: the consented query to its verifier,
            // on the query path (kind 7)
            Msg::Consent { query_id, consent } => {
                let Some((q, basis)) = self
                    .active
                    .as_mut()
                    .and_then(|a| a.conversation.awaiting_consent.remove(&query_id))
                else {
                    return refused("consent to a query I did not issue".into());
                };
                let req = self.request(&q, consent, basis);
                self.outbox.push(Msg::Query(req));
                Conversed::Consented { query: query_id }
            }
            // the counterparty asking its own nominees
            Msg::WitnessRequest(_) => Conversed::Observed { kind },
            Msg::WitnessAnswer(_) => refused("a participant does not answer as a witness".into()),
            Msg::Responses(r) => {
                if !initiator {
                    return Conversed::Observed { kind };
                }
                let n = r.len();
                if let Some(a) = self.active.as_mut() {
                    a.conversation.their_responses = Some(r);
                }
                Conversed::Gathered { responses: n }
            }
            Msg::BackPointers(t) => {
                if let Some(a) = self.active.as_mut() {
                    a.conversation.back.insert(from, t);
                }
                Conversed::BackPointers { signer: from }
            }
            // step 7, shown the body: reviewed and signed, or refused,
            // and the reply to the proposer and every witness
            Msg::Proposal(p) => {
                if initiator {
                    return refused("the counterparty does not propose".into());
                }
                let signed = self.review_and_sign(&p.proposal, &p.set, &p.back);
                if signed.is_ok()
                    && let Some(a) = self.active.as_mut()
                {
                    a.conversation.proposed = Some(*p);
                }
                self.reply_signing(kind, signed, &all)
            }
            Msg::Signed(reply) => self.signer_replied(from, kind, reply),
            // the record, over the body this side reviewed
            Msg::Record(envelope) => {
                if initiator {
                    return refused("the proposer finalises its own record".into());
                }
                let Some(p) = self
                    .active
                    .as_ref()
                    .and_then(|a| a.conversation.proposed.clone())
                else {
                    return refused("a record before its body".into());
                };
                match Record::parse(&envelope) {
                    Ok(rec) if rec.bytes[rec.body.clone()] == p.proposal.body(&p.back)[..] => {}
                    Ok(_) => {
                        return refused("a record over a body this client did not review".into());
                    }
                    Err(e) => return refused(e),
                }
                match self.finalize(&envelope, Some(&p.set)) {
                    Ok(txid) => Conversed::Finalized { txid },
                    Err(e) => refused(format!("{e:?}")),
                }
            }
            _ => refused("not a message of the conversation".into()),
        }
    }

    fn witness_said(&mut self, from: Keyhash, kind: u64, msg: Msg) -> Conversed {
        let initiator = self.active.as_ref().is_some_and(|a| a.initiator);
        match msg {
            Msg::WitnessAnswer(bits) => {
                if let Some(a) = self.active.as_mut() {
                    a.conversation.answers.insert(from, bits);
                }
                Conversed::WitnessAnswer {
                    witness: from,
                    flags: bits,
                }
            }
            Msg::BackPointers(t) => {
                if let Some(a) = self.active.as_mut() {
                    a.conversation.back.insert(from, t);
                }
                Conversed::BackPointers { signer: from }
            }
            Msg::Signed(reply) if initiator => self.signer_replied(from, kind, reply),
            Msg::Signed(_) => Conversed::Observed { kind },
            _ => Conversed::Refused {
                kind,
                why: "not a kind a witness sends".into(),
            },
        }
    }

    /// A signer's answer to the body, into the outbox for `to`: its
    /// entries; the party's own refusal as it stands; or, where the body
    /// does not verify against what this signer holds, its root, its
    /// back-pointers, or a ceremony it is not holding, refusal 4
    /// (`wire-format.md` §7.10.2), so the proposer is told rather than
    /// left waiting on a signature that will not come.  Anything else is
    /// this client's own failure, and nothing goes.
    fn reply_signing(
        &mut self,
        kind: u64,
        signed: Result<Vec<u8>, Abort>,
        to: &[Keyhash],
    ) -> Conversed {
        let reply = match signed {
            Ok(entries) => Ok(entries),
            Err(Abort::Refused(r)) => Err(r),
            Err(Abort::RootMismatch | Abort::BackPointers | Abort::NotActive) => {
                Err(Refusal::NotVerified)
            }
            Err(e) => {
                return Conversed::Refused {
                    kind,
                    why: format!("{e:?}"),
                };
            }
        };
        let refused = reply.as_ref().err().cloned();
        self.converse_into_outbox(&Msg::Signed(reply), to);
        Conversed::Reviewed { refused }
    }

    /// At the proposer: a signer's reply.  With every signer's entries in,
    /// the envelope is built, finalized here and sent to every other
    /// signer (kind 18).
    fn signer_replied(
        &mut self,
        from: Keyhash,
        kind: u64,
        reply: Result<Vec<u8>, Refusal>,
    ) -> Conversed {
        let me = self.keyhash();
        let refused = |why: &str| Conversed::Refused {
            kind,
            why: why.into(),
        };
        let a = match self.active.as_mut() {
            Some(a) if a.initiator => a,
            _ => return Conversed::Observed { kind },
        };
        let Some(p) = a.conversation.proposed.as_ref() else {
            return refused("a signature before the body went out");
        };
        let signers = p.proposal.signers();
        if !signers.contains(&from) {
            return refused("not a signer of the body");
        }
        match reply {
            Err(r) => {
                a.conversation.refused.insert(from, r.clone());
                return Conversed::Signed {
                    signer: from,
                    refused: Some(r),
                };
            }
            Ok(e) => {
                a.conversation.entries.insert(from, e);
            }
        }
        if signers
            .iter()
            .any(|s| !a.conversation.entries.contains_key(s))
        {
            return Conversed::Signed {
                signer: from,
                refused: None,
            };
        }
        let body = p.proposal.body(&p.back);
        let set = p.set.clone();
        let entries: Vec<(Keyhash, Vec<u8>)> = signers
            .iter()
            .map(|s| (*s, a.conversation.entries[s].clone()))
            .collect();
        let envelope = envelope_from_entries(TYPE_PRESENCE, &body, &entries);
        let others: Vec<Keyhash> = signers.into_iter().filter(|s| *s != me).collect();
        match self.finalize(&envelope, Some(&set)) {
            Ok(txid) => {
                self.converse_into_outbox(&Msg::Record(envelope), &others);
                Conversed::Finalized { txid }
            }
            Err(e) => refused(&format!("{e:?}")),
        }
    }

    /// The pending ceremony between `participants` that `from` is a
    /// participant of.
    fn witnessing_for(&self, participants: &[Keyhash], from: Keyhash) -> Option<[u8; 32]> {
        if !participants.contains(&from) {
            return None;
        }
        self.witnessing
            .iter()
            .find(|(_, w)| {
                w.request.participants.len() == participants.len()
                    && w.request
                        .participants
                        .iter()
                        .all(|p| participants.contains(p))
            })
            .map(|(cid, _)| *cid)
    }

    /// Any pending ceremony `from` is a participant of.
    fn witnessing_with(&self, from: Keyhash) -> Option<[u8; 32]> {
        self.witnessing
            .iter()
            .find(|(_, w)| w.request.participants.contains(&from))
            .map(|(cid, _)| *cid)
    }

    fn observe(&mut self, cid: [u8; 32], from: Keyhash, kind: u64) {
        if let Some(w) = self.witnessing.get_mut(&cid) {
            w.observed.insert((from, kind));
        }
    }

    /// The witness's side (`light-client-requirements.md` §1.2): a request
    /// is answered to both participants, with this signer's back-pointers
    /// where it accepts; the body is signed for the ceremony accepted and
    /// the reply goes to the proposer; the record is held; everything
    /// else from a participant of a pending ceremony is observed.
    fn as_witness(&mut self, from: Keyhash, kind: u64, msg: Msg) -> Conversed {
        let refused = |why: &str| Conversed::Refused {
            kind,
            why: why.into(),
        };
        match msg {
            Msg::WitnessRequest(req) => {
                if !req.participants.contains(&from) {
                    return refused("a request from no participant of the ceremony it names");
                }
                let flags = self.take_witness_request(&req);
                self.observe(req.ceremony_id, from, kind);
                let parts = req.participants.to_vec();
                self.converse_into_outbox(&Msg::WitnessAnswer(flags), &parts);
                if flags.is_some() {
                    let back = self.back_pointers();
                    self.converse_into_outbox(&Msg::BackPointers(back), &parts);
                }
                Conversed::Asked {
                    ceremony: req.ceremony_id,
                    flags,
                }
            }
            Msg::Proposal(p) => {
                if !p.proposal.participants.contains(&from) {
                    return refused("a body from no participant of the ceremony it names");
                }
                // from a participant: signed for the ceremony this client
                // holds with it, and refused as not verified for one it is
                // not holding, so the proposer is answered either way
                let signed = match self.witnessing_for(&p.proposal.participants, from) {
                    Some(cid) => {
                        self.observe(cid, from, kind);
                        self.witness_sign(&p.proposal, &p.back)
                    }
                    None => Err(Abort::NotActive),
                };
                self.reply_signing(kind, signed, &[from])
            }
            Msg::Record(envelope) => {
                let rec = match Record::parse(&envelope) {
                    Ok(r) => r,
                    Err(e) => return refused(&e),
                };
                let Some(cid) = self.witnessing_for(&rec.participants(), from) else {
                    return refused(
                        "a record from no participant of a ceremony this client witnesses",
                    );
                };
                self.observe(cid, from, kind);
                match self.finalize(&envelope, None) {
                    Ok(txid) => Conversed::Finalized { txid },
                    Err(e) => refused(&format!("{e:?}")),
                }
            }
            Msg::WitnessAnswer(_) => refused("a witness is answered by nobody"),
            // the rest of what a participant says, the other side's
            // signature among it: observed, which is what is attested
            _ => match self.witnessing_with(from) {
                Some(cid) => {
                    self.observe(cid, from, kind);
                    Conversed::Observed { kind }
                }
                None => refused("from no party to a ceremony this client is in or witnesses"),
            },
        }
    }
}
