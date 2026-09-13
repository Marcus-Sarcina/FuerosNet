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

    /// Whether a session with a serving node is held right now.
    pub fn attached(&self) -> bool {
        self.net.session().is_some()
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
