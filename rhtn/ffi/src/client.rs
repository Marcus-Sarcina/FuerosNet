//! What a shell asks of the client.
//!
//! `rhtn-adaptors` already binds a client to a node and a socket and runs
//! it on a thread of its own. This is that handle with the Rust taken off:
//! no generics, no borrows across the boundary, and every answer a value.
//!
//! **The shell keeps no protocol state.** A shell that kept its own copy
//! of a ceremony's progress would disagree with the client about what was
//! signed, and the client is the one whose bytes are on the wire. What
//! crosses outward is what to draw; what crosses inward is what the person
//! or the hardware did.

use crate::device::Platform;
use crate::net::{Attached, Event, Net, Wake, pins_for};
use crate::types::{Answer, Channel, ChannelOutcome, Id, Refused, id_of, keyhash};
use rhtn_adaptors::actor::Handle;
use rhtn_archive::Keyhash;
use rhtn_client::ceremony::{Client, Config};
use rhtn_client::device::DirectPath;
use std::net::SocketAddr;
use std::sync::Arc;

/// A participant client, running on a thread of its own.
///
/// Every method is blocking and short: the work happens on the client's
/// thread and the answer comes back. A shell calls these from wherever it
/// likes.
pub struct Participant {
    handle: Handle,
    net: Net,
}

/// What a proximity run achieved, as the shell is shown it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Achieved {
    pub channel: Channel,
    pub outcome: ChannelOutcome,
    pub resolution_m: Option<u64>,
}

/// What two devices tell each other when a ceremony opens (design §7.1).
///
/// **The wire does not carry this.** The ceremony's own conversation
/// happens between two devices in each other's presence, over whatever
/// channel they have, and no document fixes an encoding for it. It crosses
/// as fields so the shell can carry it however the two devices manage, and
/// reconstruct it on the other side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    pub contribution: Vec<u8>,
    pub nominees: Vec<Id>,
    pub bundle: Vec<Vec<u8>>,
    pub started_at: u64,
    pub retention_years: u64,
    pub initiator: bool,
}

impl Intent {
    fn of(i: &rhtn_client::ceremony::Intent) -> Intent {
        Intent {
            contribution: i.contribution.to_vec(),
            nominees: i.nominees.iter().map(id_of).collect(),
            bundle: i.bundle.clone(),
            started_at: i.started_at,
            retention_years: i.retention_years,
            initiator: i.initiator,
        }
    }

    fn inward(&self) -> Result<rhtn_client::ceremony::Intent, Refused> {
        let contribution: [u8; 16] = self.contribution.as_slice().try_into().map_err(|_| Refused::new("a contribution is 16 bytes"))?;
        let nominees: Option<Vec<Keyhash>> = self.nominees.iter().map(|n| keyhash(n)).collect();
        Ok(rhtn_client::ceremony::Intent {
            contribution,
            nominees: nominees.ok_or_else(|| Refused::new("a nominee is 32 bytes"))?,
            bundle: self.bundle.clone(),
            started_at: self.started_at,
            retention_years: self.retention_years,
            initiator: self.initiator,
        })
    }
}

/// A verifier the client selected, and why it was eligible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    pub verifier: Id,
    /// 0 met, 1 in the horizon, 2 reachable, 3 discretionary
    /// (`wire-format.md` §5.5's `selection_basis`).
    pub basis: u32,
}

/// A response about the counterparty, as a screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub verifier: Id,
    pub subject: Id,
    pub answer: Answer,
}


/// A witness's place on a record (`wire-format.md` §4.5 field 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Witnessing {
    pub witness: Id,
    pub nominated_by: Id,
    pub flags: u64,
}

/// What a nominee is asked to witness.
///
/// Like [`Intent`], the wire does not carry this: a nominee is asked over
/// whatever the three devices have between them, and no document fixes an
/// encoding for the asking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessAsk {
    pub ceremony: Id,
    pub participants: Vec<Id>,
    pub started_at: u64,
    pub channels: Vec<Achieved>,
}

/// The record two parties are proposing, before anybody has signed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposed {
    pub started_at: u64,
    pub finalized_at: u64,
    pub participants: Vec<Id>,
    pub witnesses: Vec<Witnessing>,
    /// Signed `VerifierResponse`s, in the order the body carries them.
    pub responses: Vec<Vec<u8>>,
    pub root: Vec<u8>,
}

/// One of the seven values a record discloses, with the salt it is hashed
/// under (`wire-format.md` §4.5.1.1).  The seven arrive in label order and
/// the label is carried so a shell can show which is which.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Revealed {
    pub label: String,
    pub salt: Vec<u8>,
    pub value: Vec<u8>,
}

/// A verifier's answer: the copy for the querier, and the copy for the
/// subject where one is owed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answered {
    pub query: Id,
    pub to_querier: Vec<u8>,
    pub to_subject: Option<(Id, Vec<u8>)>,
}

impl Witnessing {
    fn of(w: &rhtn_archive::tx::Witness) -> Witnessing {
        Witnessing { witness: id_of(&w.keyhash), nominated_by: id_of(&w.nominated_by), flags: w.flags }
    }

    fn inward(&self) -> Result<rhtn_archive::tx::Witness, Refused> {
        Ok(rhtn_archive::tx::Witness {
            keyhash: keyhash(&self.witness).ok_or_else(|| Refused::new("a witness is 32 bytes"))?,
            nominated_by: keyhash(&self.nominated_by).ok_or_else(|| Refused::new("a nominator is 32 bytes"))?,
            flags: self.flags,
        })
    }
}

impl WitnessAsk {
    fn of(r: &rhtn_client::ceremony::WitnessRequest) -> WitnessAsk {
        WitnessAsk {
            ceremony: r.ceremony_id.to_vec(),
            participants: r.participants.iter().map(id_of).collect(),
            started_at: r.started_at,
            channels: r.channels.iter().map(|c| Achieved { channel: Channel::of(c.kind), outcome: outcome_of(c.result), resolution_m: c.resolution_m }).collect(),
        }
    }

    fn inward(&self) -> Result<rhtn_client::ceremony::WitnessRequest, Refused> {
        let ceremony: [u8; 32] = self.ceremony.as_slice().try_into().map_err(|_| Refused::new("a ceremony id is 32 bytes"))?;
        let p: Option<Vec<Keyhash>> = self.participants.iter().map(|k| keyhash(k)).collect();
        let p = p.ok_or_else(|| Refused::new("a participant is 32 bytes"))?;
        let participants: [Keyhash; 2] = p.try_into().map_err(|_| Refused::new("a ceremony has two participants"))?;
        Ok(rhtn_client::ceremony::WitnessRequest { ceremony_id: ceremony, participants, started_at: self.started_at, channels: self.channels.iter().map(inward_channel).collect() })
    }
}

impl Proposed {
    fn of(p: &rhtn_client::record::Proposal) -> Proposed {
        Proposed {
            started_at: p.started_at,
            finalized_at: p.finalized_at,
            participants: p.participants.iter().map(id_of).collect(),
            witnesses: p.witnesses.iter().map(Witnessing::of).collect(),
            responses: p.responses.clone(),
            root: p.root.to_vec(),
        }
    }

    fn inward(&self) -> Result<rhtn_client::record::Proposal, Refused> {
        let p: Option<Vec<Keyhash>> = self.participants.iter().map(|k| keyhash(k)).collect();
        let p = p.ok_or_else(|| Refused::new("a participant is 32 bytes"))?;
        let participants: [Keyhash; 2] = p.try_into().map_err(|_| Refused::new("a ceremony has two participants"))?;
        let witnesses: Result<Vec<_>, Refused> = self.witnesses.iter().map(|w| w.inward()).collect();
        Ok(rhtn_client::record::Proposal {
            started_at: self.started_at,
            finalized_at: self.finalized_at,
            participants,
            witnesses: witnesses?,
            responses: self.responses.clone(),
            root: self.root.as_slice().try_into().map_err(|_| Refused::new("a disclosure root is 32 bytes"))?,
        })
    }

    /// Everybody who signs this record: the two participants and the
    /// witnesses, which is what a shell needs to know who to carry it to.
    #[must_use]
    pub fn signers(&self) -> Vec<Id> {
        self.inward().map(|p| p.signers().iter().map(id_of).collect()).unwrap_or_default()
    }
}

/// The seven disclosures, out and back.
///
/// **Label order is the record's, not the shell's** (`wire-format.md`
/// §4.5.1.1): the seven come back in the order they went out, and one
/// carrying a label that does not belong at its position is refused rather
/// than sorted into place.
fn revealed_of(set: &rhtn_client::record::DisclosureSet) -> Vec<Revealed> {
    set.iter().map(|d| Revealed { label: d.label.to_string(), salt: d.salt.to_vec(), value: d.value.clone() }).collect()
}

fn revealed_inward(set: &[Revealed]) -> Result<rhtn_client::record::DisclosureSet, Refused> {
    if set.len() != rhtn_client::record::LABELS.len() {
        return Err(Refused::new(format!("a disclosure set is {} values, not {}", rhtn_client::record::LABELS.len(), set.len())));
    }
    let mut out: Vec<rhtn_client::record::Disclosure> = Vec::with_capacity(set.len());
    for (i, r) in set.iter().enumerate() {
        let label = rhtn_client::record::LABELS[i];
        if r.label != label {
            return Err(Refused::new(format!("disclosure {i} is `{}` where the record's order has `{label}`", r.label)));
        }
        out.push(rhtn_client::record::Disclosure {
            label,
            salt: r.salt.as_slice().try_into().map_err(|_| Refused::new("a salt is 16 bytes"))?,
            value: r.value.clone(),
        });
    }
    out.try_into().map_err(|_| Refused::new("a disclosure set is seven values"))
}

fn inward_channel(a: &Achieved) -> rhtn_client::device::ChannelOutcome {
    rhtn_client::device::ChannelOutcome { kind: a.channel.kind(), result: a.outcome.result(), resolution_m: a.resolution_m }
}

fn answered_of(a: &rhtn_client::verifier::Answer) -> Answered {
    Answered { query: a.query_id.to_vec(), to_querier: a.to_querier.clone(), to_subject: Some((id_of(&a.to_subject.0), a.to_subject.1.clone())) }
}

fn back_inward(back: &[Vec<Vec<u8>>]) -> Result<Vec<Vec<rhtn_archive::Txid>>, Refused> {
    back.iter()
        .map(|one| one.iter().map(|t| t.as_slice().try_into().map_err(|_| Refused::new("a transaction id is 32 bytes"))).collect())
        .collect()
}

impl Participant {
    /// Start a client on a thread of its own.
    ///
    /// The identity is the two seeds it is derived from, as every other
    /// reader of one takes them. The platform's objects are wrapped inside
    /// that thread, which is the only place the client's own device may be
    /// built.
    pub fn start(seeds: Vec<u8>, known: Vec<Vec<u8>>, platform: Platform) -> Result<Participant, Refused> {
        if seeds.len() != 64 {
            return Err(Refused::new(format!("an identity is 64 bytes of seed, not {}", seeds.len())));
        }
        let (ed, pq): ([u8; 32], [u8; 32]) = (seeds[..32].try_into().unwrap(), seeds[32..].try_into().unwrap());
        let ids: Result<Vec<rhtn_crypto::Identity>, Refused> = known
            .iter()
            .map(|k| rhtn_crypto::Identity::from_key_material(k).ok_or_else(|| Refused::new("a known identity is not a KeyMaterial array")))
            .collect();
        let ids = ids?;
        // this key twice, from the same seeds: the transport needs one and
        // the client never leaves the thread that holds the other
        let me = Arc::new(rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq));
        // the nonces this client's submissions carry come from the
        // platform's random source, which is the only one there is
        let random = platform.random.clone();
        let nonce: Arc<dyn Fn() -> [u8; 16] + Send + Sync> = Arc::new(move || {
            let mut n = [0u8; 16];
            let drawn = random.fill(16);
            let take = drawn.len().min(16);
            n[..take].copy_from_slice(&drawn[..take]);
            n
        });
        let net = Net::new(me, pins_for(&ids), nonce)?;
        let known = ids.clone();
        let handle = Handle::spawn(move || {
            let me = rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq);
            let direct: std::rc::Rc<dyn DirectPath> = std::rc::Rc::new(rhtn_client::device::NoDirectPath);
            Client::new(me, known, Config::default(), platform.device(direct))
        })
        .map_err(Refused::new)?;
        Ok(Participant { handle, net })
    }

    /// This client's own identity.
    pub fn me(&self) -> Id {
        id_of(&self.handle.me())
    }

    /// Begin a ceremony with `counterparty`, nominating witnesses from
    /// their neighbourhood (`light-client-requirements.md` §1.1).
    ///
    /// The intent is for the shell to carry to the other device by
    /// whatever means the two have; the protocol does not say how, and
    /// neither does this.
    pub fn begin(&self, counterparty: Id, nominees: Vec<Id>, initiator: bool) -> Result<Intent, Refused> {
        let cp = keyhash(&counterparty).ok_or_else(|| Refused::new("a counterparty is 32 bytes"))?;
        let noms: Option<Vec<Keyhash>> = nominees.iter().map(|n| keyhash(n)).collect();
        let noms = noms.ok_or_else(|| Refused::new("a nominee is 32 bytes"))?;
        self.handle
            .with_blocking(move |c| c.begin(cp, noms, initiator).map(|i| Intent::of(&i)))
            .map_err(|a| Refused::new(format!("{a:?}")))
    }

    /// Take the counterparty's intent.  The ceremony's id comes back,
    /// which is what both devices name it by from here on.
    pub fn take_intent(&self, from: Id, intent: Intent) -> Result<Id, Refused> {
        let f = keyhash(&from).ok_or_else(|| Refused::new("a party is 32 bytes"))?;
        let i = intent.inward()?;
        self.handle.with_blocking(move |c| c.take_intent(f, &i).map(|id| id.to_vec()).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// Run every channel the hardware has, strongest first, and report
    /// what was achieved.  Nothing is promoted
    /// (`light-client-requirements.md` §1.3).
    pub fn proximity(&self) -> Result<Vec<Achieved>, Refused> {
        self.handle
            .with_blocking(|c| c.proximity())
            .map(|outs| {
                outs.into_iter()
                    .map(|o| Achieved { channel: Channel::of(o.kind), outcome: outcome_of(o.result), resolution_m: o.resolution_m })
                    .collect()
            })
            .map_err(|a| Refused::new(format!("{a:?}")))
    }

    /// The verifiers this client selected of the counterparty's pool, and
    /// the basis each was eligible on (`wire-format.md` §5.5).
    pub fn select_verifiers(&self) -> Result<Vec<Selected>, Refused> {
        self.handle
            .with_blocking(|c| c.select_verifiers())
            .map(|picked| picked.into_iter().map(|(v, b)| Selected { verifier: id_of(&v), basis: b as u32 }).collect())
            .map_err(|a| Refused::new(format!("{a:?}")))
    }

    /// The responses gathered about the counterparty so far, as a screen
    /// shows them.
    pub fn responses(&self) -> Vec<Response> {
        self.handle.with_blocking(|c| {
            c.responses()
                .iter()
                .filter_map(|r| rhtn_client::query::Response::read(r).ok())
                .map(|r| Response { verifier: id_of(&r.verifier), subject: id_of(&r.subject), answer: Answer::of(r.verdict) })
                .collect()
        })
    }

    /// Consent to a query about this subject, or decline
    /// (`light-client-requirements.md` §1.4).
    ///
    /// **The person is asked, and this returns what they answered.** A
    /// shell that consented on their behalf would be signing for them.
    pub fn consent(&self, query: Vec<u8>) -> Result<Option<Vec<u8>>, Refused> {
        self.handle.with_blocking(move |c| {
            let q = rhtn_client::query::VerificationQuery::decode(&query).map_err(Refused::new)?;
            Ok(c.consent(&q).map(|(consent, _)| consent))
        })
    }

    /// Attach to a serving node and sweep the population's prekey material
    /// (`light-client-requirements.md` §3).
    ///
    /// **The session is this side's**, not the shell's (design §14.1.0).
    /// The bundle is published, the pool stocked and the sweep run over the
    /// wire from here; what comes back is what a screen shows, and no byte
    /// of the protocol crosses outward.
    ///
    /// A second attach replaces the first, which is how a client returns to
    /// its patron after a spell on a sibling.
    pub fn attach(&self, serving: Id, addresses: Vec<String>, population: Vec<Id>) -> Result<Attached, Refused> {
        let node = keyhash(&serving).ok_or_else(|| Refused::new("a serving node is 32 bytes"))?;
        let pop: Option<Vec<Keyhash>> = population.iter().map(|p| keyhash(p)).collect();
        let pop = pop.ok_or_else(|| Refused::new("a population member is 32 bytes"))?;
        let addrs: Result<Vec<SocketAddr>, Refused> = addresses.iter().map(|a| a.parse::<SocketAddr>().map_err(|_| Refused::new(format!("{a} is not an address")))).collect();
        let addrs = addrs?;
        if addrs.is_empty() {
            return Err(Refused::new("a serving node needs at least one address"));
        }
        self.net.attach(&self.handle, node, &addrs, pop)
    }

    /// Routine maintenance: rotate a prekey whose interval has elapsed,
    /// replenish a pool that has fallen low, and ask for a binding this
    /// client wants.  What that produces goes to the serving node from
    /// here.
    ///
    /// Refused where nothing is attached: maintenance is a conversation
    /// with a node, and there is no node.
    pub fn maintain(&self) -> Result<(), Refused> {
        self.net.maintain(&self.handle)
    }

    /// Send `bytes` of `kind` to `to`, over the direct path where one is
    /// held and through the serving node otherwise (design §14.1.1).
    ///
    /// A shell's own traffic is [`crate::types::KIND_APPLICATION`]; the
    /// other kinds are the client's own and the adaptors answer them
    /// without a shell seeing them (design §14.2.4.6).
    pub fn send(&self, to: Id, kind: u64, bytes: Vec<u8>) -> Result<(), Refused> {
        let peer = keyhash(&to).ok_or_else(|| Refused::new("a recipient is 32 bytes"))?;
        self.net.send(peer, kind, bytes)
    }

    /// Where this client asks to be rung when something is waiting
    /// (design §14.1.5), or `None` to withdraw whatever the node holds.
    ///
    /// **The endpoint is the user's choice** and this side never obtains
    /// one: the shell hands over what the person's own service gave it.
    pub fn wake(&self, endpoint: Option<Wake>) -> Result<(), Refused> {
        self.net.wake(endpoint)
    }

    /// The next thing that arrived, or nothing within `timeout_ms`.
    ///
    /// **What crosses outward is what to draw.** Payload has already been
    /// decrypted here and the ciphertext never leaves.
    pub fn next_event(&self, timeout_ms: u64) -> Option<Event> {
        self.net.next_event(timeout_ms)
    }

    /// What this client holds of its own neighbourhood, and what it can
    /// do with it without asking anyone (design §15.1.1).
    ///
    /// **This is the whole point of keeping a copy.** A participant that
    /// had to ask its patron where somebody is cannot route around a
    /// patron that is not answering, and cannot weigh trust distance
    /// either (`light-client-requirements.md` §4.2).
    #[must_use]
    pub fn places(&self) -> Vec<Placed> {
        self.handle.with_blocking(|c| {
            c.horizon
                .resolvable()
                .into_iter()
                .flat_map(|n| {
                    c.horizon.places_of(&n).into_iter().map(move |p| Placed { node: id_of(&n), anchor: id_of(&p.anchor), path: p.path.clone(), nibbles: p.nibbles }).collect::<Vec<_>>()
                })
                .collect()
        })
    }

    /// Everybody this client can place without asking anyone.
    #[must_use]
    pub fn resolvable(&self) -> Vec<Id> {
        self.handle.with_blocking(|c| c.horizon.resolvable().iter().map(id_of).collect())
    }

    /// How many adoption or sibling edges away a party is, or nothing
    /// beyond the horizon.
    #[must_use]
    pub fn distance(&self, node: Id) -> Option<u32> {
        let k = keyhash(&node)?;
        self.handle.with_blocking(move |c| c.horizon.distance(&k).map(|d| d as u32))
    }

    /// Whether this client holds the transaction `txid` names, in its own
    /// archive or in what its patron propagated.
    #[must_use]
    pub fn holds(&self, txid: Id) -> bool {
        let Some(t) = keyhash(&txid) else { return false };
        self.handle.with_blocking(move |c| c.horizon.holds(&t) || c.archive.get(&t).is_some())
    }

    /// How many records the copy is a fold over.
    #[must_use]
    pub fn records(&self) -> u64 {
        self.handle.with_blocking(|c| c.horizon.records() as u64)
    }

    /// Drop what has left the horizon, and say how many parties were
    /// forgotten (`light-client-requirements.md` §4.2).
    pub fn prune(&self) -> u64 {
        self.handle.with_blocking(|c| c.horizon.prune() as u64)
    }

    /// Whether a session with a serving node is held right now.
    pub fn attached(&self) -> bool {
        self.net.session().is_some()
    }

    /// The ceremony this client is in, once both intents have crossed.
    #[must_use]
    pub fn ceremony(&self) -> Option<Id> {
        self.handle.with_blocking(|c| c.ceremony_id().map(|i| i.to_vec()))
    }

    /// Take what the counterparty's hardware achieved.
    ///
    /// **Both sides weigh the same pair of lists**, which is why this
    /// crosses rather than each side trusting its own: a channel one
    /// device passed and the other did not is a disagreement the record
    /// has to settle (`light-client-requirements.md` §1.3).
    pub fn take_channels(&self, theirs: Vec<Achieved>) -> Result<(), Refused> {
        let ch: Vec<_> = theirs.iter().map(inward_channel).collect();
        self.handle.with_blocking(move |c| c.take_channels(&ch).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// The key this client's own captures will be sealed under, for the
    /// counterparty to capture with (design §7.5.2).
    pub fn capture_key(&self) -> Result<Vec<u8>, Refused> {
        self.handle.with_blocking(|c| c.capture_key().map(|k| k.to_vec()).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// Run the guided capture of the counterparty, sealed under the key
    /// they supplied.  **The key is discarded once the capture is sealed**,
    /// so this client holds no decryptable likeness of them.
    pub fn capture(&self, their_key: Vec<u8>) -> Result<(), Refused> {
        let k: [u8; 32] = their_key.as_slice().try_into().map_err(|_| Refused::new("a capture key is 32 bytes"))?;
        self.handle.with_blocking(move |c| c.capture(k).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// The query to put to one selected verifier (`wire-format.md` §5.5),
    /// encoded as the verifier will read it.
    pub fn query_for(&self, verifier: Id) -> Result<Vec<u8>, Refused> {
        let v = keyhash(&verifier).ok_or_else(|| Refused::new("a verifier is 32 bytes"))?;
        self.handle.with_blocking(move |c| c.query_for(v).map(|q| q.encode()).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// The request that carries a query, the subject's consent and the
    /// basis the verifier was selected on.
    pub fn request(&self, query: Vec<u8>, consent: Vec<u8>, basis: u32) -> Result<Vec<u8>, Refused> {
        let b = basis_of(basis)?;
        self.handle.with_blocking(move |c| {
            let q = rhtn_client::query::VerificationQuery::decode(&query).map_err(Refused::new)?;
            Ok(c.request(&q, consent, b))
        })
    }

    /// Take a request put to this client as a verifier.
    ///
    /// Nothing where the grant it needs has not arrived — it waits, bounded
    /// — and a refusal where the input is not evidence, since **no signed
    /// response is fabricated** for one.
    pub fn take_query(&self, from: Id, bytes: Vec<u8>) -> Result<Option<Answered>, Refused> {
        let f = keyhash(&from).ok_or_else(|| Refused::new("a requester is 32 bytes"))?;
        self.handle.with_blocking(move |c| match c.take_query(f, &bytes) {
            rhtn_client::verifier::QueryOutcome::Answered(a) => Ok(Some(answered_of(&a))),
            rhtn_client::verifier::QueryOutcome::AwaitingGrant => Ok(None),
            rhtn_client::verifier::QueryOutcome::Closed(why) => Err(Refused::new(why)),
        })
    }

    /// Take a capture key released to this client as a verifier.
    pub fn take_grant(&self, from: Id, bytes: Vec<u8>) -> Result<Option<Answered>, Refused> {
        let f = keyhash(&from).ok_or_else(|| Refused::new("a subject is 32 bytes"))?;
        self.handle.with_blocking(move |c| match c.take_grant(f, &bytes) {
            rhtn_client::verifier::GrantOutcome::Answered(a) => Ok(Some(answered_of(&a))),
            rhtn_client::verifier::GrantOutcome::Buffered | rhtn_client::verifier::GrantOutcome::Ignored => Ok(None),
            rhtn_client::verifier::GrantOutcome::Rejected(why) => Err(Refused::new(why)),
        })
    }

    /// Take a verifier's response about the counterparty.
    pub fn take_response(&self, bytes: Vec<u8>) -> Result<(), Refused> {
        self.handle.with_blocking(move |c| c.take_response(&bytes).map_err(Refused::new))
    }

    /// The responses gathered so far, as they will sit in the body.
    #[must_use]
    pub fn gathered(&self) -> Vec<Vec<u8>> {
        self.handle.with_blocking(|c| c.responses())
    }

    /// Who each side nominated: this client's, then the counterparty's.
    #[must_use]
    pub fn nominees(&self) -> (Vec<Id>, Vec<Id>) {
        self.handle.with_blocking(|c| {
            let (mine, theirs) = c.nominees();
            (mine.iter().map(id_of).collect(), theirs.iter().map(id_of).collect())
        })
    }

    /// What this client asks its nominees to witness.
    pub fn witness_ask(&self) -> Result<WitnessAsk, Refused> {
        self.handle.with_blocking(|c| c.witness_request().map(|r| WitnessAsk::of(&r)).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// Answer an ask put to this client as a nominee: the flags it will
    /// sign under, or nothing where it declines.
    #[must_use]
    pub fn take_witness_ask(&self, ask: WitnessAsk) -> Option<u64> {
        let Ok(r) = ask.inward() else { return None };
        self.handle.with_blocking(move |c| c.take_witness_request(&r))
    }

    /// The back-pointers this client will put in its own signer entry
    /// (`wire-format.md` §3.1).
    #[must_use]
    pub fn back_pointers(&self) -> Vec<Vec<u8>> {
        self.handle.with_blocking(|c| c.back_pointers().iter().map(|t| t.to_vec()).collect())
    }

    /// Propose the record, given the counterparty's responses and the
    /// witnesses who accepted.  The disclosures come back with it: they are
    /// what the record commits to and what a holder may later reveal.
    pub fn propose(&self, theirs: Vec<Vec<u8>>, witnesses: Vec<Witnessing>) -> Result<(Proposed, Vec<Revealed>), Refused> {
        let w: Result<Vec<_>, Refused> = witnesses.iter().map(|x| x.inward()).collect();
        let w = w?;
        self.handle
            .with_blocking(move |c| c.propose(theirs, w).map(|(p, set)| (Proposed::of(&p), revealed_of(&set))).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// The body every signer signs over, given each signer's back-pointers
    /// in signer order.
    pub fn body(&self, proposal: Proposed, back: Vec<Vec<Vec<u8>>>) -> Result<Vec<u8>, Refused> {
        let p = proposal.inward()?;
        let b = back_inward(&back)?;
        Ok(p.body(&b))
    }

    /// Review a proposal as a participant and sign it, or refuse.
    ///
    /// **The disclosures are reviewed, not taken on trust**: a participant
    /// signing a root it has not seen the values behind would be committing
    /// to a record it cannot read.
    pub fn review_and_sign(&self, proposal: Proposed, set: Vec<Revealed>, back: Vec<Vec<Vec<u8>>>) -> Result<Vec<u8>, Refused> {
        let p = proposal.inward()?;
        let d = revealed_inward(&set)?;
        let b = back_inward(&back)?;
        self.handle.with_blocking(move |c| c.review_and_sign(&p, &d, &b).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// Sign a proposal as a witness, which sees no disclosures.
    pub fn witness_sign(&self, proposal: Proposed, back: Vec<Vec<Vec<u8>>>) -> Result<Vec<u8>, Refused> {
        let p = proposal.inward()?;
        let b = back_inward(&back)?;
        self.handle.with_blocking(move |c| c.witness_sign(&p, &b).map_err(|a| Refused::new(format!("{a:?}"))))
    }

    /// Where this client sits in the subnet `anchor` names, or nothing
    /// where it is in no such subnet.  **Asking under its own key always
    /// answers**: a party with no ancestor names itself
    /// (`wire-format.md` §2.1).
    #[must_use]
    pub fn position_in(&self, anchor: Id) -> Option<Vec<u8>> {
        let a = keyhash(&anchor)?;
        self.handle.with_blocking(move |c| {
            c.position_in(&a).map(|p| {
                let mut out = Vec::new();
                p.emit(&mut out);
                out
            })
        })
    }

    /// Every subnet this client has a position in, its own first.
    #[must_use]
    pub fn anchors(&self) -> Vec<Id> {
        self.handle.with_blocking(|c| c.anchors().iter().map(id_of).collect())
    }

    /// As a patron: the adoption body putting `node` one hop below this
    /// client, in the subnet `anchor` names.
    ///
    /// **Which tree is the patron's to say**, because a patron in two
    /// subnets sits at a different path in each and the address it issues
    /// is its path in the one it is adopting into.
    pub fn propose_adoption(&self, anchor: Id, node: Id, presence: Id, series: u32, node_back: Vec<Vec<u8>>) -> Result<Vec<u8>, Refused> {
        let a = keyhash(&anchor).ok_or_else(|| Refused::new("an anchor is 32 bytes"))?;
        let n = keyhash(&node).ok_or_else(|| Refused::new("a subordinate is 32 bytes"))?;
        let p = keyhash(&presence).ok_or_else(|| Refused::new("a presence record is named by a 32-byte txid"))?;
        let back = back_inward(&[node_back])?.remove(0);
        self.handle.with_blocking(move |c| {
            c.propose_adoption_in(
                a,
                n,
                &back,
                rhtn_client::ceremony::Adopting { evidence: rhtn_archive::tx::Evidence::Presence(p), series, presented_head: None, key_material: None },
            )
                .map_err(|e| Refused::new(format!("{e:?}")))
        })
    }

    /// Sign a body this client proposed or was shown, as its subject or
    /// its patron.
    #[must_use]
    pub fn sign_body(&self, body: Vec<u8>) -> Vec<u8> {
        self.handle.with_blocking(move |c| c.sign_body(&body))
    }

    /// Take a finalized adoption this client signed.  Where it is an
    /// adoption of this client, it is also what tells it where it now
    /// sits.
    pub fn take_adoption(&self, envelope: Vec<u8>) -> Result<Id, Refused> {
        let t = self.handle.with_blocking(move |c| c.take_adoption(&envelope).map(|t| t.to_vec()).map_err(|e| Refused::new(format!("{e:?}"))))?;
        self.net.carry_outbox(&self.handle)?;
        Ok(t)
    }

    /// Take the finished record.  A participant is given the disclosures
    /// with it; a witness is not, and holds the record without them.
    pub fn finalize(&self, envelope: Vec<u8>, set: Option<Vec<Revealed>>) -> Result<Id, Refused> {
        let d = match set {
            None => None,
            Some(s) => Some(revealed_inward(&s)?),
        };
        let t = self.handle.with_blocking(move |c| c.finalize(&envelope, d.as_ref()).map(|t| t.to_vec()).map_err(|a| Refused::new(format!("{a:?}"))))?;
        // **the record goes up as soon as it exists.** A ceremony that
        // ended in a record nobody else will ever see did the work and
        // none of the good.
        self.net.carry_outbox(&self.handle)?;
        Ok(t)
    }
}

/// Where a party sits, as a shell is shown it: one of these per subnet
/// they are in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub node: Id,
    pub anchor: Id,
    pub path: Vec<u8>,
    pub nibbles: u64,
}

/// The envelope a presence record travels in, from the body every signer
/// signed and each signer's entry (`wire-format.md` §3.2).
#[must_use]
pub fn presence_envelope(body: Vec<u8>, entries: Vec<(Id, Vec<u8>)>) -> Vec<u8> {
    let e: Vec<(Keyhash, Vec<u8>)> = entries.into_iter().filter_map(|(k, v)| keyhash(&k).map(|k| (k, v))).collect();
    rhtn_archive::tx::envelope_from_entries(rhtn_archive::tx::TYPE_PRESENCE, &body, &e)
}

/// The envelope an adoption travels in.
#[must_use]
pub fn adoption_envelope(body: Vec<u8>, entries: Vec<(Id, Vec<u8>)>) -> Vec<u8> {
    let e: Vec<(Keyhash, Vec<u8>)> = entries.into_iter().filter_map(|(k, v)| keyhash(&k).map(|k| (k, v))).collect();
    rhtn_archive::tx::envelope_from_entries(rhtn_archive::tx::TYPE_ADOPTION, &body, &e)
}

fn basis_of(b: u32) -> Result<rhtn_client::selection::SelectionBasis, Refused> {
    match b {
        0 => Ok(rhtn_client::selection::SelectionBasis::Met),
        1 => Ok(rhtn_client::selection::SelectionBasis::InHorizon),
        2 => Ok(rhtn_client::selection::SelectionBasis::Reachable),
        3 => Ok(rhtn_client::selection::SelectionBasis::Discretionary),
        _ => Err(Refused::new(format!("{b} is not a selection basis (`wire-format.md` §5.5 fixes 0 to 3)"))),
    }
}

fn outcome_of(r: rhtn_client::device::ChannelResult) -> ChannelOutcome {
    match r {
        rhtn_client::device::ChannelResult::Pass => ChannelOutcome::Pass,
        rhtn_client::device::ChannelResult::Fail => ChannelOutcome::Fail,
        rhtn_client::device::ChannelResult::Unavailable => ChannelOutcome::Unavailable,
    }
}

/// So a shell need not hold `Arc` itself.
pub fn platform(
    proximity: Arc<dyn crate::device::Proximity>,
    camera: Arc<dyn crate::device::Camera>,
    clock: Arc<dyn crate::device::Clock>,
    random: Arc<dyn crate::device::Random>,
    operator: Arc<dyn crate::device::Operator>,
    notices: Arc<dyn crate::device::Notices>,
) -> Platform {
    Platform { proximity, camera, clock, random, operator, notices }
}
