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
use crate::net::{Attached, Event, Net, Status, Wake, pins_for};
use crate::types::{Answer, Id, Refused, id_of, keyhash};
use rhtn_adaptors::actor::Handle;
use rhtn_archive::Keyhash;
use rhtn_client::ceremony::Client;
use rhtn_client::device::DirectPath;
use rhtn_transport::tls::Party;
use std::net::SocketAddr;
use std::sync::Arc;

impl Drop for Participant {
    /// The seeds go with the participant [2026-09-30]. They are held for
    /// the backup alone (design §13.7.1) and only on the ceremony device;
    /// on the way out they are wiped, for the reason every holder of key
    /// material in the client wipes: freed memory keeps no keys (design
    /// §3).
    fn drop(&mut self) {
        use zeroize::Zeroize;
        if let Some(seeds) = &mut self.seeds {
            for s in seeds {
                s.zeroize();
            }
        }
    }
}

/// A participant client, running on a thread of its own.
///
/// Every method is blocking and short: the work happens on the client's
/// thread and the answer comes back. A shell calls these from wherever it
/// likes.
#[derive(uniffi::Object)]
pub struct Participant {
    handle: Handle,
    net: Net,
    /// Where the client's state goes between runs, under [`STATE`].
    storage: Arc<dyn crate::device::Storage>,
    /// **One save at a time.** The state is saved after every call that
    /// can change it and after every event that arrived, and those run
    /// on two of the shell's threads at once — a conversation step on one,
    /// the event loop on the other. Two writes of the same name in flight
    /// together is what a platform's storage cannot be asked to take: on
    /// the bench one of the pair landed and the other was refused, and
    /// the refusal stopped a ceremony that had done nothing wrong
    /// [2026-10-06]. The lock also orders the states, so what lands last
    /// is what was taken last.
    saving: Arc<std::sync::Mutex<()>>,
    /// Nouns the kernel marked changed while an arrival was being handed
    /// up; raised as one `Event::Changed` at the next quiet call.
    pending_changed: std::sync::Mutex<Vec<Vec<rhtn_model::Noun>>>,
    /// The seeds this device holds, for the backup: on the ceremony
    /// device alone.
    seeds: Option<[[u8; 32]; 2]>,
}

/// The name the client's durable state is written under.
pub const STATE: &str = "client";
/// The name the cached sibling list is written under
/// (`light-client-requirements.md` §4: persisted across restarts).
pub const SIBLINGS: &str = "siblings";

/// A submission nonce drawn from the platform's randomness, in full or
/// not at all: short measure is refused rather than padded, as the seed
/// path refuses it (`device.rs`), because a nonce completed with zeroes is
/// not random.
///
/// **It refuses by panicking, and the closure's type is why**: the nonce
/// source is a `Fn() -> [u8; 16]` the transport calls, with nowhere to
/// return a refusal to. The choice is a loud stop or a silent weak nonce,
/// and a platform whose randomness returns short measure is broken in a
/// way no submission should proceed under.
fn nonce_drawn(random: &dyn crate::device::Random) -> [u8; 16] {
    let drawn = random.fill(16);
    <[u8; 16]>::try_from(drawn.as_slice()).unwrap_or_else(|_| {
        panic!(
            "the platform's randomness returned {} bytes of 16",
            drawn.len()
        )
    })
}

/// The storage seam with the kernel's sealing on it
/// (`light-client-requirements.md` §9): what goes down is sealed under the
/// key the platform's custody returned, what comes up is opened, and a blob
/// written before the kernel sealed carries no header and passes through to
/// the readers that always read it.
struct SealedStore {
    under: Arc<dyn crate::device::Storage>,
    key: [u8; 32],
}

impl SealedStore {
    /// The storage a start runs on, per the platform's declared custody:
    /// the plain seam where the platform is unsealed by configuration, and
    /// the sealing seam otherwise — under the kept key, or under one minted
    /// here and kept there. **A source that cannot keep the key refuses the
    /// start**; falling quietly to plaintext is the one thing this must
    /// never do.
    fn over(platform: &Platform) -> Result<Arc<dyn crate::device::Storage>, Refused> {
        // the platform's storage with the diagnostic events on it: name,
        // size and outcome of each read and write, never the bytes
        let under: Arc<dyn crate::device::Storage> =
            Arc::new(crate::diag::TracedStorage(platform.storage.clone()));
        if platform.custody.unsealed() {
            tracing::info!(target: "platform", state = "unsealed", "platform.custody");
            return Ok(under);
        }
        let key: [u8; 32] = match platform.custody.key() {
            Some(k) => {
                // present or not, and never the key
                tracing::info!(target: "platform", state = "present", "platform.custody");
                k.try_into().map_err(|k: Vec<u8>| {
                    Refused::new(format!("a storage key is 32 bytes, not {}", k.len()))
                })?
            }
            None => {
                let mut drawn = platform.random.fill(32);
                let minted = <[u8; 32]>::try_from(drawn.as_slice());
                // the drawn copy is wiped whichever way this goes: it is
                // the storage key, and freed memory keeps no keys (§3)
                {
                    use zeroize::Zeroize;
                    let kept = minted.is_ok() && platform.custody.keep(drawn.clone());
                    drawn.zeroize();
                    tracing::info!(target: "platform", state = "created", kept, "platform.custody");
                    let Ok(k) = minted else {
                        return Err(Refused::new("the platform's random source came up short"));
                    };
                    if !kept {
                        return Err(Refused::new(
                            "the platform did not keep the storage key; \
                             unsealed is declared, never fallen back to",
                        ));
                    }
                    k
                }
            }
        };
        Ok(Arc::new(SealedStore { under, key }))
    }
}

impl crate::device::Storage for SealedStore {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        let bytes = self.under.read(name.clone())?;
        if rhtn_client::sealed::is_sealed(&bytes) {
            // an authentication failure hands back the bytes as written, so
            // the caller refuses the start over state that does not open
            // rather than beginning fresh as though nothing were held
            rhtn_client::sealed::open(&self.key, &name, &bytes).or(Some(bytes))
        } else {
            // written before the kernel sealed; read as it always was
            Some(bytes)
        }
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        let sealed = rhtn_client::sealed::seal(&self.key, &name, &bytes);
        self.under.write(name, sealed)
    }
}

/// The key is wiped when the seam goes (`light-client-requirements.md` §3).
impl Drop for SealedStore {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key.zeroize();
    }
}

/// **The maintenance contract.**  While the process runs and a session is
/// held, the kernel rotates a prekey whose interval has elapsed, replenishes
/// a pool that has fallen low and asks for bindings it wants, every this
/// often on its own clock; a shell calls [`Participant::maintain`] besides
/// when the platform grants it background time, since a suspended process
/// runs no clock.  Sixty seconds is a chosen value: the work is nothing
/// unless something is due, and a pool at its floor waits at most this long.
pub const MAINTAIN_EVERY: std::time::Duration = std::time::Duration::from_secs(10);

/// What a restore from a backup did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Restored {
    /// Records the import took.
    pub records: u64,
    /// Sealed captures the import scan discarded as past their retention
    /// (design §13.7.1), which the person is told rather than not.
    pub discarded: u64,
}

/// One turn of the capture's hold, as [`Participant::capture_tick`] reports
/// it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
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

/// A verifier the client selected, and why it was eligible.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Selected {
    /// The verifier.
    pub verifier: Id,
    /// 0 met, 1 in the horizon, 2 reachable, 3 discretionary
    /// (`wire-format.md` §5.5's `selection_basis`).
    pub basis: u32,
}

/// A response about the counterparty, as a screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Response {
    /// The verifier that answered.
    pub verifier: Id,
    /// Whom the answer is about.
    pub subject: Id,
    /// What it said.
    pub answer: Answer,
}

/// A witness's place on a record (`wire-format.md` §4.5 field 4).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Witnessing {
    /// The witness.
    pub witness: Id,
    /// Which participant nominated it.
    pub nominated_by: Id,
    /// The witness flags the record carries for it.
    pub flags: u64,
}

/// The nomination itself (`wire-format.md` §7.10.2, kind 20) [author,
/// 2026-10-19]: what a nominee is sent with the intent, before the
/// counterparty has accepted anything.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Engaging {
    /// The engagement handle, minted per ceremony attempt.
    pub handle: Id,
    /// The claimed start, the intent's time.
    pub started_at: u64,
    /// The counterparty, where the nominator has read its code.
    pub counterparty: Option<Id>,
}

impl Engaging {
    fn of(e: &rhtn_client::conversation::Engagement) -> Engaging {
        Engaging {
            handle: e.handle.to_vec(),
            started_at: e.started_at,
            counterparty: e.counterparty.map(|k| k.to_vec()),
        }
    }

    fn inward(&self) -> Result<rhtn_client::conversation::Engagement, Refused> {
        Ok(rhtn_client::conversation::Engagement {
            handle: self
                .handle
                .as_slice()
                .try_into()
                .map_err(|_| Refused::new("a handle is 32 bytes"))?,
            started_at: self.started_at,
            counterparty: match &self.counterparty {
                Some(k) => {
                    Some(keyhash(k).ok_or_else(|| Refused::new("a counterparty is 32 bytes"))?)
                }
                None => None,
            },
        })
    }
}

/// **How far the engagement has got** (`wire-format.md` §7.10.2, kind 20)
/// [author, 2026-10-19]: a shell shows the pair while the opening runs,
/// and a nominee still unreached when the ceremony-id fixes is one that
/// does not appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct Engaged {
    /// Nominees the engagement reached.
    pub engaged: u32,
    /// Nominees nominated.
    pub nominated: u32,
}

/// One event of the optical exchange's shape (`wire-format.md` §7.10.2,
/// kind 21), as the shell observes it [author, 2026-10-19].
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum TraceStep {
    /// Both block counts are known.
    Shape {
        /// Blocks this sender has to transmit.
        mine: u64,
        /// Blocks the counterparty declares.
        theirs: u64,
    },
    /// An inbound block landed, by index.
    Inbound {
        /// The block's index.
        index: u64,
    },
    /// An outbound block was acknowledged on the counterparty's display.
    Outbound {
        /// The block's index.
        index: u64,
    },
}

/// What a nominee is asked to witness.
///
/// Like the ceremony's own exchange, the network does not carry this: a
/// nominee is asked over
/// whatever the three devices have between them, and no document fixes an
/// encoding for the asking.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WitnessAsk {
    /// The ceremony being witnessed.
    pub ceremony: Id,
    /// The two parties meeting.
    pub participants: Vec<Id>,
    /// When the meeting began, in milliseconds since the epoch.
    pub started_at: u64,
    /// The engagement handle the request closes (`wire-format.md`
    /// §7.10.2 field 5).
    pub handle: Id,
}

/// The record two parties are proposing, before anybody has signed it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Proposed {
    /// When the meeting began, in milliseconds since the epoch.
    pub started_at: u64,
    /// When the proposer finalised the body.
    pub finalized_at: u64,
    /// The two parties, in the order the body carries them.
    pub participants: Vec<Id>,
    /// The witnesses and their places.
    pub witnesses: Vec<Witnessing>,
    /// Signed `VerifierResponse`s, in the order the body carries them.
    pub responses: Vec<Vec<u8>>,
    /// The disclosure root the body commits to.
    pub root: Vec<u8>,
    /// Subtype 1, a formation record: two identities at their genesis, no
    /// witness and no response (`wire-format.md` §3.2; design §13.2).
    pub formation: bool,
}

/// One of the seven values a record discloses, with the salt it is hashed
/// under (`wire-format.md` §4.5.1.1).  The seven arrive in label order and
/// the label is carried so a shell can show which is which.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Revealed {
    /// Which of the seven this is.
    pub label: String,
    /// The salt the value is hashed under.
    pub salt: Vec<u8>,
    /// The value itself.
    pub value: Vec<u8>,
}

/// A verifier's answer: the copy for the querier, and the copy for the
/// subject where one is owed.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Answered {
    /// The query answered.
    pub query: Id,
    /// The querier's copy, encoded.
    pub to_querier: Vec<u8>,
    /// The subject's copy, where one is owed.
    pub to_subject: Option<SubjectCopy>,
}

/// The subject's copy of an answer: whom it goes to and the bytes.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SubjectCopy {
    /// Whom the copy goes to.
    pub subject: Id,
    /// The copy, encoded.
    pub bytes: Vec<u8>,
}

/// What consenting yielded: the consent the querier's request carries,
/// and, where this subject holds a record with the query's verifier, the
/// grant that releases the capture key.  **The grant is owed to that
/// verifier directly and to nobody else** (design §7.5.2), and the client
/// cannot see what carries it, so honouring that is the caller's.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Consented {
    /// The consent the querier's request carries.
    pub consent: Vec<u8>,
    /// The grant owed to the query's verifier, where one is.
    pub grant: Option<Vec<u8>>,
}

/// What an intake code turned out to carry (`wire-format.md` §§2.3, 2.4,
/// 7.6).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Intake {
    /// What the code carried, in words a screen can show.
    pub kind: String,
    /// Whom it was about: the party introduced, or the node.
    pub who: Id,
}

/// How one peer stood after a contact round ([`Participant::contact_peers`]).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PeerContact {
    /// The peer.
    pub who: Id,
    /// Whether a direct path is held now.
    pub held: bool,
    /// What happened, in the kernel's words: `held`, `dialled`, `no hint`
    /// or `unreached`.
    pub how: String,
}

/// What a resource answered (`wire-format.md` §11.1).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResourceAnswer {
    /// 0 delivered, 1 refused, 2 unavailable, 3 malformed, 4 no subtree
    /// acknowledgement, 5 no matching role.
    pub status: u64,
    /// The application's answer, **read here and not above the
    /// boundary**, where one was delivered and could be read.
    pub page: Option<ResourcePage>,
    /// Why the delivered message could not be read, where it could not.
    /// A shell shows this and renders nothing.
    pub unreadable: Option<String>,
    /// **A typed ask of this client** (`light-client-requirements.md`
    /// §8.1), where the resource answered with one instead of a page:
    /// read here, held under its number, and put to the person by the
    /// shell as a question.  Answered with [`Participant::answer_utility`].
    pub ask: Option<UtilityAsk>,
}

/// **What a resource asked of this client**, read by the kernel and
/// shown whole: the utility, its purpose and retention in the resource's
/// own words, and what the utility needs.  The resource and node are the
/// ones the answer goes back to; a shell shows them and cannot change
/// them.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct UtilityAsk {
    /// The number the answer names.
    pub id: u64,
    /// The utility: `scan`, `capture`, `location`, `check-in`, `tag`,
    /// `sign`, `live`, `notify`, `export`, `import`.
    pub utility: String,
    /// What it is for, in the resource's words.
    pub purpose: String,
    /// How long the resource says it keeps the result.
    pub retention: String,
    /// The resource that asked.
    pub resource: Id,
    /// The node that served it.
    pub node: Id,
    /// `sign`: the exact bytes to be approved and signed.
    pub payload: Option<Vec<u8>>,
    /// `location`: the geohash length asked for.
    pub precision: Option<u8>,
    /// `scan`, `capture`, `import`: the media types the resource takes.
    pub media: Vec<String>,
    /// `check-in`, `tag`: the channel the resource minds.
    pub channel: Option<String>,
    /// `live`: the seconds asked for.
    pub seconds: Option<u64>,
}

/// **What the person did with an ask.**  The kernel checks it against the
/// ask, signs where the utility signs, composes the answer and sends it;
/// a shell supplies the capture and nothing else.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum UtilityOutcome {
    /// Declined, with why: the person said no, or this device has no
    /// such utility.  The application sees the refusal.
    Declined {
        /// Why, in words.
        reason: String,
    },
    /// Bytes under a media type: a scan, a capture, a file.
    Bytes {
        /// The declared type.
        media: String,
        /// The bytes.
        bytes: Vec<u8>,
    },
    /// Text: a geohash, what a tag said.
    Text {
        /// The text.
        text: String,
    },
    /// Approved and nothing to carry: a signature over the ask's own
    /// payload, a live session, a standing permission, an export taken.
    Approved,
}

impl UtilityOutcome {
    fn inward(self) -> rhtn_client::utility::Outcome {
        use rhtn_client::utility::Outcome;
        match self {
            UtilityOutcome::Declined { reason } => Outcome::Declined(reason),
            UtilityOutcome::Bytes { media, bytes } => Outcome::Bytes { media, bytes },
            UtilityOutcome::Text { text } => Outcome::Text(text),
            UtilityOutcome::Approved => Outcome::Approved,
        }
    }
}

/// **What a client may show of a resource's answer**
/// (`light-client-requirements.md` §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum PageShape {
    /// `text/html`: a page, for the frame.
    Page,
    /// `text/plain`: text, which is not a page and takes no frame.
    Text,
    /// Anything else, and anything undeclared: named to the person
    /// rather than rendered. **Nothing is sniffed** — a body inspected to
    /// find out what it is arrives as something nobody agreed to see.
    Other,
}

/// A resource's answer, parsed out of the one HTTP/1.1 message §11
/// carries.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ResourcePage {
    /// The application's own HTTP status — **not** `ResourceAnswer`'s,
    /// which says whether anything was delivered at all.
    pub http_status: u32,
    /// The declared media type, lower-cased and without parameters, or
    /// none where the resource declared none.
    pub media: Option<String>,
    /// The declared charset, lower-cased, where there was one.
    pub charset: Option<String>,
    /// What may be shown of it.
    pub shape: PageShape,
    /// The body as it arrived.
    pub body: Vec<u8>,
}

/// One candidate provider, as a provisioning screen orders them
/// (`light-client-requirements.md` §6; PRD-11).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ProviderRow {
    /// The provider's ASN, as the caller named it.
    pub asn: u64,
    /// How many parties inside this client's own horizon are already seen
    /// peering from inside that ASN.
    pub nodes: u32,
    /// **The display tag `light-client-requirements.md` §6 asks for** —
    /// "mark the crowded ones rather than hiding them" — set where this
    /// operator already has presence inside that provider. **How it is
    /// styled, and whether a share of the horizon rather than any
    /// presence at all should set it, are screen-design decisions**
    /// [author, 2026-10-16]: the count is beside it either way, and
    /// nothing is hidden.
    pub crowded: bool,
}

/// The two nominee lists a ceremony holds: this side's and the other's.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Nominees {
    /// This side's nominees.
    pub mine: Vec<Id>,
    /// The counterparty's.
    pub theirs: Vec<Id>,
}

/// Where the ceremony's conversation stands on the end-to-end path
/// (`wire-format.md` §7.10.1), for the shell to decide its next step.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Progress {
    /// Whether this side proposes: the initiator does.
    pub proposer: bool,
    /// Whether this side has opened the conversation already: its
    /// nominees asked and its back-pointers handed over.
    pub opened: bool,
    /// Whether it has handed the proposer what it gathered already.
    pub handed: bool,
    /// **Whether proposing would now produce a body.** Ask this; do not
    /// propose to find out. A refusal used as a poll result had the
    /// proposer asking 28 times in 3 s on the bench [client review,
    /// 2026-10-11].
    pub proposable: bool,
    /// Whether a proposal is otherwise ready and no nominee has attested:
    /// past genesis that is a record which cannot finalize, and is the
    /// person's to wait out or stop.
    pub no_witness: bool,
    /// What a proposal waits on, in words to show a person — not to
    /// parse.
    pub waiting_on: Option<String>,
    /// Queries issued and not yet answered.
    pub queries_outstanding: u32,
    /// Surviving witnesses, of either side, whose answer has not arrived:
    /// the one count a shell paces its proposal by inside the window.
    pub witnesses_outstanding: u32,
    /// Nominees that will attest.
    pub attesting: Vec<Id>,
    /// Nominees that declined.
    pub declined: Vec<Id>,
    /// Signers whose back-pointers are held.
    pub back_from: Vec<Id>,
    /// Whether the counterparty's gathered responses are held.
    pub their_responses: bool,
    /// Whether the body is out, or has been shown.
    pub proposed: bool,
    /// Signers whose entries are held, at the proposer.
    pub signed: Vec<Id>,
    /// Signers that refused, at the proposer.
    pub refused: Vec<Id>,
}

impl Progress {
    fn of(p: &rhtn_client::sequence::Progress) -> Progress {
        let ids = |v: &[Keyhash]| v.iter().map(id_of).collect();
        Progress {
            proposer: p.proposer,
            opened: p.opened,
            handed: p.handed,
            proposable: p.proposable,
            no_witness: p.no_witness,
            waiting_on: p.waiting_on.clone(),
            queries_outstanding: p.queries_outstanding as u32,
            witnesses_outstanding: p.witnesses_outstanding as u32,
            attesting: p
                .answered
                .iter()
                .filter(|(_, f)| f.is_some())
                .map(|(k, _)| id_of(k))
                .collect(),
            declined: p
                .answered
                .iter()
                .filter(|(_, f)| f.is_none())
                .map(|(k, _)| id_of(k))
                .collect(),
            back_from: ids(&p.back_from),
            their_responses: p.their_responses,
            proposed: p.proposed,
            signed: ids(&p.signed),
            refused: ids(&p.refused),
        }
    }
}

/// What a proposer holds after proposing: the record as every signer is
/// shown it, and the seven disclosures it commits to.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Proposal {
    /// The record body as every signer is shown it.
    pub proposed: Proposed,
    /// The seven disclosures the body's root commits to.
    pub revealed: Vec<Revealed>,
}

/// One signer's entries on a record body.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SignedEntries {
    /// Whose entries these are.
    pub signer: Id,
    /// The entries, encoded.
    pub entries: Vec<u8>,
}

impl Witnessing {
    fn of(w: &rhtn_archive::tx::Witness) -> Witnessing {
        Witnessing {
            witness: id_of(&w.keyhash),
            nominated_by: id_of(&w.nominated_by),
            flags: w.flags,
        }
    }

    fn inward(&self) -> Result<rhtn_archive::tx::Witness, Refused> {
        Ok(rhtn_archive::tx::Witness {
            keyhash: keyhash(&self.witness).ok_or_else(|| Refused::new("a witness is 32 bytes"))?,
            nominated_by: keyhash(&self.nominated_by)
                .ok_or_else(|| Refused::new("a nominator is 32 bytes"))?,
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
            handle: r.handle.to_vec(),
        }
    }

    fn inward(&self) -> Result<rhtn_client::ceremony::WitnessRequest, Refused> {
        let ceremony: [u8; 32] = self
            .ceremony
            .as_slice()
            .try_into()
            .map_err(|_| Refused::new("a ceremony id is 32 bytes"))?;
        let p: Option<Vec<Keyhash>> = self.participants.iter().map(|k| keyhash(k)).collect();
        let p = p.ok_or_else(|| Refused::new("a participant is 32 bytes"))?;
        let participants: [Keyhash; 2] = p
            .try_into()
            .map_err(|_| Refused::new("a ceremony has two participants"))?;
        Ok(rhtn_client::ceremony::WitnessRequest {
            ceremony_id: ceremony,
            participants,
            started_at: self.started_at,
            handle: self
                .handle
                .as_slice()
                .try_into()
                .map_err(|_| Refused::new("a handle is 32 bytes"))?,
        })
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
            formation: p.formation,
        }
    }

    fn inward(&self) -> Result<rhtn_client::record::Proposal, Refused> {
        let p: Option<Vec<Keyhash>> = self.participants.iter().map(|k| keyhash(k)).collect();
        let p = p.ok_or_else(|| Refused::new("a participant is 32 bytes"))?;
        let participants: [Keyhash; 2] = p
            .try_into()
            .map_err(|_| Refused::new("a ceremony has two participants"))?;
        let witnesses: Result<Vec<_>, Refused> =
            self.witnesses.iter().map(|w| w.inward()).collect();
        Ok(rhtn_client::record::Proposal {
            started_at: self.started_at,
            finalized_at: self.finalized_at,
            participants,
            witnesses: witnesses?,
            responses: self.responses.clone(),
            root: self
                .root
                .as_slice()
                .try_into()
                .map_err(|_| Refused::new("a disclosure root is 32 bytes"))?,
            formation: self.formation,
        })
    }

    /// Everybody who signs this record: the two participants and the
    /// witnesses, which is what a shell needs to know who to carry it to.
    #[must_use]
    pub fn signers(&self) -> Vec<Id> {
        self.inward()
            .map(|p| p.signers().iter().map(id_of).collect())
            .unwrap_or_default()
    }
}

/// The seven disclosures, out and back.
///
/// **Label order is the record's, not the shell's** (`wire-format.md`
/// §4.5.1.1): the seven come back in the order they went out, and one
/// carrying a label that does not belong at its position is refused rather
/// than sorted into place.
fn revealed_of(set: &rhtn_client::record::DisclosureSet) -> Vec<Revealed> {
    set.iter()
        .map(|d| Revealed {
            label: d.label.to_string(),
            salt: d.salt.to_vec(),
            value: d.value.clone(),
        })
        .collect()
}

fn revealed_inward(set: &[Revealed]) -> Result<rhtn_client::record::DisclosureSet, Refused> {
    if set.len() != rhtn_client::record::LABELS.len() {
        return Err(Refused::new(format!(
            "a disclosure set is {} values, not {}",
            rhtn_client::record::LABELS.len(),
            set.len()
        )));
    }
    let mut out: Vec<rhtn_client::record::Disclosure> = Vec::with_capacity(set.len());
    for (i, r) in set.iter().enumerate() {
        let label = rhtn_client::record::LABELS[i];
        if r.label != label {
            return Err(Refused::new(format!(
                "disclosure {i} is `{}` where the record's order has `{label}`",
                r.label
            )));
        }
        out.push(rhtn_client::record::Disclosure {
            label,
            salt: r
                .salt
                .as_slice()
                .try_into()
                .map_err(|_| Refused::new("a salt is 16 bytes"))?,
            value: r.value.clone(),
        });
    }
    out.try_into()
        .map_err(|_| Refused::new("a disclosure set is six values"))
}

fn answered_of(a: &rhtn_client::verifier::Answer) -> Answered {
    Answered {
        query: a.query_id.to_vec(),
        to_querier: a.to_querier.clone(),
        to_subject: Some(SubjectCopy {
            subject: id_of(&a.to_subject.0),
            bytes: a.to_subject.1.clone(),
        }),
    }
}

fn back_inward(back: &[Vec<Vec<u8>>]) -> Result<Vec<Vec<rhtn_archive::Txid>>, Refused> {
    back.iter()
        .map(|one| {
            one.iter()
                .map(|t| {
                    t.as_slice()
                        .try_into()
                        .map_err(|_| Refused::new("a transaction id is 32 bytes"))
                })
                .collect()
        })
        .collect()
}

impl Participant {
    /// Restore what the last run wrote, if anything.  **A blob that does
    /// not open refuses the start**: a client begun fresh over state it
    /// could not read would publish new material and lose every message
    /// queued for the old, silently.
    fn open(&self) -> Result<(), Refused> {
        // the sibling list, whole or not at all: a partial list is no list
        if let Some(bytes) = self.storage.read(SIBLINGS.into()) {
            self.net.restore_siblings(&bytes);
        }
        let Some(bytes) = self.storage.read(STATE.into()) else {
            return Ok(());
        };
        self.handle
            .with_blocking(move |c| -> Result<(), String> {
                let r = c.restore_durable(&bytes)?;
                let held: Vec<String> = c
                    .payload
                    .sessions
                    .held()
                    .iter()
                    .map(|(k, d)| {
                        format!(
                            "{}/{}",
                            rhtn_client::diag::id8(k),
                            rhtn_client::diag::id8(d)
                        )
                    })
                    .collect();
                tracing::info!(target: "pay", sessions = r.sessions, ?held, "pay.restored");
                Ok(())
            })
            .map_err(|e| Refused::new(format!("the stored state does not open: {e}")))?;
        // the kernel re-learned its contacts from the state; the transport
        // binds a dial by them (`wire-format.md` §9.1), so it learns them too
        self.pin_known();
        Ok(())
    }

    /// **Every identity the kernel verifies by is pinned for the
    /// transport** (`wire-format.md` §9.1): the direct path binds the key
    /// a peer presents by the pin, in both directions, and the pin set is
    /// the transport's own — filled at open from what the shell handed
    /// over, from the sibling cache and at first optical contact, which
    /// left out every party a ceremony, an introduction or a cold call
    /// taught the kernel afterwards, and lost those at the next start.
    /// Refilled from the kernel's `known` at open, at an introduction
    /// taken and whenever the contacts change; pinning is idempotent
    /// [bench `9ea2c16-model-validation-1`, 2026-10-30: after a restart a
    /// peer's every dial was closed *unbound* and none was made].
    fn pin_known(&self) {
        let ids = self.handle.with_blocking(|c| c.known.clone());
        for id in &ids {
            self.net.pin_identity(id);
        }
    }
}

impl Participant {
    /// One step of the conversation (`wire-format.md` §7.10.1): taken on
    /// the client, carried by the courier, and the state written, since
    /// the ratchets advanced and a restart that lost them could not read
    /// the replies.
    fn converse<F>(&self, step: F) -> Result<(), Refused>
    where
        F: FnOnce(
                &mut rhtn_client::ceremony::Client,
            )
                -> Result<Vec<rhtn_client::ceremony::Msg>, rhtn_client::ceremony::Abort>
            + Send
            + 'static,
    {
        let msgs = self
            .handle
            .with_blocking(step)
            .map_err(|a| Refused::new(format!("{a:?}")))?;
        // **a leg a witness's node refused is a witness that will not
        // appear, not a stopped ceremony** (design §7.1) [bench,
        // 2026-10-24]: said, and the step stands
        let unreached = self.net.carry_noting(msgs)?;
        if !unreached.is_empty() {
            tracing::warn!(target: "cer", unreached = unreached.len(), "cer.leg.refused");
        }
        self.save()
    }
}

impl Participant {
    /// Start the kernel's own maintenance clock ([`MAINTAIN_EVERY`]).
    fn schedule(&self) {
        let (h, st, one) = (
            self.handle.clone(),
            self.storage.clone(),
            self.saving.clone(),
        );
        // the same write [`Participant::save`] makes, under the same lock:
        // run after a maintenance round that carried anything, and after
        // every inbound dispatch
        let after: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
            let _one = one.lock().unwrap_or_else(|e| e.into_inner());
            let bytes = h.with_blocking(|c| {
                let held = c.payload.sessions.held().len();
                tracing::debug!(target: "pay", sessions = held, "pay.saving");
                c.durable()
            });
            let _ = st.write(STATE.into(), bytes);
        });
        self.net.persist_with(after.clone());
        self.net
            .schedule_maintenance(self.handle.clone(), MAINTAIN_EVERY, after.clone());
        // and the peers, on their own clock (design §6.3)
        self.net
            .schedule_contact(self.handle.clone(), crate::net::PEER_CONTACT_EVERY, after);
    }
}

#[uniffi::export]
impl Participant {
    /// Start a client on a thread of its own.
    ///
    /// The identity is the two seeds it is derived from, as every other
    /// reader of one takes them. The platform's objects are wrapped inside
    /// that thread, which is the only place the client's own device may be
    /// built.
    #[uniffi::constructor]
    pub fn start(
        seeds: Vec<u8>,
        known: Vec<Vec<u8>>,
        platform: Arc<Platform>,
    ) -> Result<Participant, Refused> {
        crate::diag::install(platform.diagnostics.clone());
        // said here, after the renderer exists: the platform built its
        // engine before anything could show it
        tracing::info!(target: "platform", present = platform.engine.is_some(), "platform.engine");
        crate::diag::call("start", || {
            if seeds.len() != 64 {
                return Err(Refused::new(format!(
                    "an identity is 64 bytes of seed, not {}",
                    seeds.len()
                )));
            }
            let (ed, pq): ([u8; 32], [u8; 32]) = (
                seeds[..32].try_into().unwrap(),
                seeds[32..].try_into().unwrap(),
            );
            // the crossing copy goes as soon as it is read [2026-09-30]: it is
            // the identity, and freed memory keeps no keys (design §3). What
            // the caller holds is the caller's; this side's copy is not left
            // in the heap for whatever allocates next
            {
                use zeroize::Zeroize;
                let mut seeds = seeds;
                seeds.zeroize();
            }
            let ids: Result<Vec<rhtn_crypto::Identity>, Refused> = known
                .iter()
                .map(|k| {
                    rhtn_crypto::Identity::from_key_material(k)
                        .ok_or_else(|| Refused::new("a known identity is not a KeyMaterial array"))
                })
                .collect();
            let ids = ids?;
            // this key twice, from the same seeds: the transport needs one and
            // the client never leaves the thread that holds the other
            let me = Arc::new(rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq));
            // **a participant knows its own key without being told.** The
            // delegated-device path above already adds its own identity to
            // `known` where the caller left it out; this one did not, so a
            // client was only able to verify a record it had itself signed
            // if whoever started it handed its own `KeyMaterial` back.
            //
            // Every field run did, because the harness built `known` as
            // node, peer and self — which hid this completely. A phone
            // provisioned knowing nobody, which is what meeting a stranger
            // actually looks like, reached `REVIEW` and refused the
            // counterparty's signature with *missing key for signer*,
            // naming **itself** [measured, 2026-10-07].
            let mut ids = ids;
            if !ids.iter().any(|i| i.keyhash == me.public.keyhash) {
                ids.push(me.public.clone());
            }
            // the nonces this client's submissions carry come from the
            // platform's random source, which is the only one there is
            let random = platform.random.clone();
            let nonce: Arc<dyn Fn() -> [u8; 16] + Send + Sync> =
                Arc::new(move || nonce_drawn(random.as_ref()));
            let net = Net::new(Party::of(me), pins_for(&ids), nonce)?;
            let known = ids.clone();
            // the storage this run persists through: the platform's, with the
            // kernel's sealing on it per the platform's declared custody
            let storage = SealedStore::over(&platform)?;
            let reachable = net.reachable();
            let handle = Handle::spawn(move || {
                let me = rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq);
                // the client's route reads the same set the direct socket fills
                let direct: std::rc::Rc<dyn DirectPath> = std::rc::Rc::new(reachable);
                Client::new(me, known, platform.config(), platform.device(direct))
            })
            .map_err(Refused::new)?;
            // the payload path with no node, from the start (design §6.4)
            net.stand_detached(&handle)?;
            let p = Participant {
                handle,
                net,
                storage,
                saving: Arc::new(std::sync::Mutex::new(())),
                pending_changed: std::sync::Mutex::new(Vec::new()),
                seeds: Some([ed, pq]),
            };
            p.open()?;
            p.schedule();
            Ok(p)
        })
    }

    /// **Start as a key succeeding another one** (design §9.0, §9.1):
    /// the third door of the first run, where the other two are
    /// [`Participant::start`] and a start followed by
    /// [`Participant::restore_backup`].
    ///
    /// **The prior key is required and is not optional**, because
    /// `wire-format.md` §4.1 admits no variant of the `Recovery` block:
    /// every claim of a predecessor's history carries **both** halves, the
    /// old key's own signature and a prior counterparty's recognition. A
    /// device holding no prior seed cannot make one, and what it can do
    /// instead is start afresh — which is the unlinked path, and abandons
    /// the standing rather than giving up the unlinkability.
    ///
    /// **The archive is optional, and its absence is ordinary**
    /// (`light-client-requirements.md` §6): *recovery restores standing
    /// and not history*. Where the person kept their own backup it is
    /// opened here under the prior identity and the old lines come back
    /// with it; where they did not, the recovery still carries their
    /// standing and the history is simply gone. **Neither is presented as
    /// the other**: what came back is `old_lines`, and a screen that found
    /// none says so.
    ///
    /// `seeds` is the new identity, minted on this device. `prior_seeds`
    /// is the one being succeeded, which the person kept apart from the
    /// backup as §2 has them do — the blob opens under it, and a backup
    /// naming another identity is refused however it opened.
    ///
    /// Both copies of a seed are wiped on this side as soon as they are
    /// read, as [`Participant::start`] wipes its own.
    #[uniffi::constructor]
    pub fn recovering(
        seeds: Vec<u8>,
        prior_seeds: Vec<u8>,
        backup: Option<Vec<u8>>,
        known: Vec<Vec<u8>>,
        platform: Arc<Platform>,
    ) -> Result<Participant, Refused> {
        if prior_seeds.len() != 64 {
            return Err(Refused::new(format!(
                "a prior identity is 64 bytes of seed, not {}",
                prior_seeds.len()
            )));
        }
        let (ed, pq): ([u8; 32], [u8; 32]) = (
            prior_seeds[..32].try_into().unwrap(),
            prior_seeds[32..].try_into().unwrap(),
        );
        {
            use zeroize::Zeroize;
            let mut prior_seeds = prior_seeds;
            prior_seeds.zeroize();
        }
        let prior = Box::new(rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq));
        let prior_public = prior.public.clone();

        // the old archive, where the person brought one: opened under the
        // key that wrote it, which is the key being succeeded
        let mut archive = rhtn_archive::chain::Archive::new(prior_public.keyhash);
        if let Some(blob) = backup {
            let contents = rhtn_client::backup::import(&blob, prior.identity_root())
                .map_err(|e| Refused::new(e.to_string()))?;
            match contents.owner {
                Some(o) if o == prior_public.keyhash => {}
                Some(_) => {
                    return Err(Refused::new(
                        "that backup belongs to another identity than the key being succeeded",
                    ));
                }
                None => return Err(Refused::new("that backup names no owner")),
            }
            let mut parsed: Vec<rhtn_archive::record::Record> = contents
                .records
                .iter()
                .map(|r| rhtn_archive::record::Record::parse(r))
                .collect::<Result<_, _>>()
                .map_err(|e| Refused::new(format!("record: {e}")))?;
            parsed.sort_by_key(|r| (r.effective, r.txid));
            for rec in parsed {
                archive
                    .append(rec)
                    .map_err(|e| Refused::new(format!("record: {e}")))?;
            }
        }

        // **the succeeded key is one this client knows**, so that what it
        // signed verifies here: the successor statement is checked by
        // everyone it reaches, and a client unable to check its own is of
        // no use to the person holding it
        let mut known = known;
        if !known.iter().any(|k| {
            rhtn_crypto::Identity::from_key_material(k)
                .is_some_and(|i| i.keyhash == prior_public.keyhash)
        }) {
            known.push(prior_public.key_material().to_vec());
        }

        let p = Participant::start(seeds, known, platform)?;
        p.handle.with_blocking(move |c| {
            c.rotation = Some(Box::new(rhtn_client::rotation::Rotation::begin(
                prior, archive,
            )));
        });
        p.save()?;
        Ok(p)
    }

    /// The connection as a screen shows it: detached, attached to the
    /// serving node or degraded on a sibling, reconnecting, or lost.
    /// Changes also arrive as [`Event::Connection`].
    #[must_use]
    pub fn status(&self) -> Status {
        crate::diag::call_plain("status", || self.net.status())
    }

    /// The parties this client is peered with (design §6.3), from the
    /// peering records it holds.
    pub fn peers(&self) -> Vec<Id> {
        crate::diag::call_plain("peers", || {
            self.handle
                .with_blocking(|c| c.peers().iter().map(id_of).collect())
        })
    }

    /// **Reach for every peer now** (design §6.3): each peer no direct
    /// path is held to is dialled at the candidates it last offered, and
    /// this side's own go back where that opens.  The kernel does this on
    /// its own clock besides; a shell calls it when the platform says the
    /// network came back, or a person asks.  How each peer stood.
    pub fn contact_peers(&self) -> Result<Vec<PeerContact>, Refused> {
        crate::diag::call("contact_peers", || {
            let round = self.net.contact_peers(&self.handle)?;
            // a round may have taken a deposit or reconciled: written
            self.save()?;
            Ok(round
                .into_iter()
                .map(|c| PeerContact {
                    who: id_of(&c.peer),
                    held: c.held,
                    how: c.how.to_string(),
                })
                .collect())
        })
    }

    /// **The device's network changed** (`light-client-requirements.md`
    /// §9: the hardware crosses inward, and connectivity is the
    /// platform's to know).  The direct socket is stood again where the
    /// device now routes out of another interface, and every peer is
    /// reached for, as [`Participant::contact_peers`] does.
    pub fn network_changed(&self) -> Result<Vec<PeerContact>, Refused> {
        crate::diag::call("network_changed", || {
            let restood = self.net.refresh_detached(&self.handle)?;
            tracing::info!(target: "pay", restood, "pay.network_changed");
            self.contact_peers()
        })
    }

    /// Whether a direct path to `peer` is held now: status a screen may
    /// show, never a choice — the path order is fixed (design §12.6.3).
    #[must_use]
    pub fn direct_to(&self, peer: Id) -> bool {
        crate::diag::call_plain("direct_to", || {
            keyhash(&peer).is_some_and(|k| self.net.direct_to(&k))
        })
    }

    /// Sweep the serving node's catalog (`light-client-requirements.md`
    /// §8) and keep what it served; [`Participant::catalog`] shows it.
    pub fn browse(&self) -> Result<(), Refused> {
        crate::diag::call("browse", || {
            self.net.browse(&self.handle)?;
            self.save()
        })
    }

    /// Every resource the catalog holds (`rhtn_model::Resource`;
    /// `wire-format.md` §6.4), attributed to the node that served its
    /// entry and marked where that node's portion was not whole.
    #[must_use]
    pub fn resources(&self) -> Vec<rhtn_model::Resource> {
        crate::diag::call_plain("resources", || {
            self.handle.with_blocking(|c| c.resources_held())
        })
    }

    /// Every meeting this client took part in, newest first
    /// (`rhtn_model::Meeting`): the record, the counterparty, the
    /// witnesses, and the adoption or peering that rested on it.
    #[must_use]
    pub fn meetings(&self) -> Vec<rhtn_model::Meeting> {
        crate::diag::call_plain("meetings", || {
            self.handle.with_blocking(|c| c.meetings_held())
        })
    }

    /// Every profile held: this client's own, by subnet, and the subnets'
    /// (`rhtn_model::Profile`; `wire-format.md` §7.11).
    #[must_use]
    pub fn profiles(&self) -> Vec<rhtn_model::Profile> {
        crate::diag::call_plain("profiles", || {
            self.handle.with_blocking(|c| c.profiles_held())
        })
    }

    /// Write the client's state to the platform's storage.  Called by the
    /// kernel after every step that changes what a restart would need,
    /// and by a shell at any point it is about to be suspended.
    pub fn save(&self) -> Result<(), Refused> {
        crate::diag::call("save", || {
            // a poisoned lock is a thread that panicked mid-save, and the
            // next save overwriting whatever it left is the recovery
            let _one = self.saving.lock().unwrap_or_else(|e| e.into_inner());
            let bytes = self.handle.with_blocking(|c| {
                let held = c.payload.sessions.held().len();
                tracing::debug!(target: "pay", sessions = held, "pay.saving");
                c.durable()
            });
            let took = self.storage.write(STATE.into(), bytes)
                && self.storage.write(SIBLINGS.into(), self.net.siblings());
            if took {
                Ok(())
            } else {
                Err(Refused::new(
                    "the platform's storage did not take the state",
                ))
            }
        })
    }

    /// Everything a device loss would take away, enveloped under the key
    /// the identity derives (design §13.7.1): the records, the store and
    /// the provider credential.  **Never the identity**, which is what
    /// opens it; a device holding no seed makes none.  The payload
    /// material and the sessions are this device's own and do not travel.
    pub fn export_backup(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("export_backup", || {
            self.handle
                .with_blocking(|c| c.export())
                .map_err(|e| Refused::new(e.to_string()))
        })
    }

    /// Open a backup under this identity, scan it, and install it into a
    /// client that holds nothing yet; then write the result to storage.
    /// Refused where this client already has an archive, where the backup
    /// does not open under this identity, or where it names another.
    pub fn restore_backup(&self, blob: Vec<u8>) -> Result<Restored, Refused> {
        crate::diag::call("restore_backup", || {
            let r = self
                .handle
                .with_blocking(move |c| {
                    let (contents, discarded) = c.import(&blob).map_err(|e| e.to_string())?;
                    let records = c.install(contents)?;
                    Ok::<_, String>(Restored {
                        records: records as u64,
                        discarded: (discarded.captures + discarded.own) as u64,
                    })
                })
                .map_err(Refused::new)?;
            self.save()?;
            Ok(r)
        })
    }

    /// Start as a device that holds no seed (design §23.3): a desktop or a
    /// terminal instrument.  `key_material` is the identity's own
    /// `KeyMaterial` array, `transport_seed` the 32-byte seed of the
    /// transport keypair this device minted, and `delegations` the run the
    /// ceremony device signed over its public half ([`Participant::delegate`]).
    ///
    /// The identity key is nowhere on this device, so what that key signs
    /// is refused here: consent, a verifier's answer, a body, a departure.
    /// The device's prekey bundle is signed on the ceremony device through
    /// [`Participant::bundle_to_sign`] and [`Participant::take_signed_bundle`].
    #[uniffi::constructor]
    pub fn start_delegated(
        key_material: Vec<u8>,
        transport_seed: Vec<u8>,
        delegations: Vec<Vec<u8>>,
        known: Vec<Vec<u8>>,
        platform: Arc<Platform>,
    ) -> Result<Participant, Refused> {
        crate::diag::install(platform.diagnostics.clone());
        crate::diag::call("start_delegated", || {
            let public = rhtn_crypto::Identity::from_key_material(&key_material)
                .ok_or_else(|| Refused::new("the identity is not a KeyMaterial array"))?;
            let seed: [u8; 32] = transport_seed
                .as_slice()
                .try_into()
                .map_err(|_| Refused::new("a transport key is 32 bytes of seed"))?;
            let ids: Result<Vec<rhtn_crypto::Identity>, Refused> = known
                .iter()
                .map(|k| {
                    rhtn_crypto::Identity::from_key_material(k)
                        .ok_or_else(|| Refused::new("a known identity is not a KeyMaterial array"))
                })
                .collect();
            let mut ids = ids?;
            if !ids.iter().any(|i| i.keyhash == public.keyhash) {
                ids.push(public.clone());
            }
            let clock = platform.clock.clone();
            let credential = rhtn_transport::tls::Credential::from_seed(&seed, public.keyhash)
                .with_clock(Arc::new(move || clock.now_ms() / 1000));
            if delegations.is_empty() {
                return Err(Refused::new(
                    "a delegated device starts with at least one delegation",
                ));
            }
            for d in &delegations {
                credential
                    .add(&ids, d)
                    .map_err(|e| Refused::new(format!("delegation refused: {e}")))?;
            }
            let credential = Arc::new(credential);
            let presented = credential.public();
            let random = platform.random.clone();
            let nonce: Arc<dyn Fn() -> [u8; 16] + Send + Sync> =
                Arc::new(move || nonce_drawn(random.as_ref()));
            let net = Net::new(Party::of(credential), pins_for(&ids), nonce)?;
            let known = ids.clone();
            let storage = SealedStore::over(&platform)?;
            let reachable = net.reachable();
            let handle = Handle::spawn(move || {
                let direct: std::rc::Rc<dyn DirectPath> = std::rc::Rc::new(reachable);
                Client::delegated(
                    public,
                    presented,
                    known,
                    platform.config(),
                    platform.device(direct),
                )
            })
            .map_err(Refused::new)?;
            // the payload path with no node, from the start (design §6.4)
            net.stand_detached(&handle)?;
            let p = Participant {
                handle,
                net,
                storage,
                saving: Arc::new(std::sync::Mutex::new(())),
                pending_changed: std::sync::Mutex::new(Vec::new()),
                seeds: None,
            };
            p.open()?;
            p.schedule();
            Ok(p)
        })
    }

    /// Whether this device holds the seed.
    #[must_use]
    pub fn holds_seed(&self) -> bool {
        crate::diag::call_plain("holds_seed", || {
            self.handle.with_blocking(|c| c.holds_seed())
        })
    }

    /// On the ceremony device: a run of `count` contiguous 48-hour
    /// delegations from `not_before` over the transport key another
    /// device of this identity minted (`wire-format.md` §8.2).
    pub fn delegate(
        &self,
        transport_key: Vec<u8>,
        not_before: u64,
        count: u32,
    ) -> Result<Vec<Vec<u8>>, Refused> {
        crate::diag::call("delegate", || {
            let key: [u8; 32] = transport_key
                .as_slice()
                .try_into()
                .map_err(|_| Refused::new("a transport key is 32 bytes"))?;
            if count == 0 {
                return Err(Refused::new("a run is at least one delegation"));
            }
            self.handle
                .with_blocking(move |c| c.delegate_run(&key, not_before, count as usize))
                .map_err(|e| Refused::new(format!("{e:?}")))
        })
    }

    /// The transport key this device presents: the public half the
    /// ceremony device delegates to.  On a device holding the seed it is
    /// the identity's classical member.
    #[must_use]
    pub fn presented_key(&self) -> Vec<u8> {
        crate::diag::call_plain("presented_key", || self.net.presented_key().to_vec())
    }

    /// On a device holding no seed: the unsigned bundle over its own
    /// payload material, for the ceremony device to sign.  Nothing where
    /// the material it holds is already signed over, or where this device
    /// holds the seed.
    #[must_use]
    pub fn bundle_to_sign(&self) -> Option<Vec<u8>> {
        crate::diag::call_plain("bundle_to_sign", || {
            self.handle.with_blocking(|c| c.bundle_to_sign())
        })
    }

    /// On the ceremony device: sign a bundle payload another device of
    /// this identity made.
    pub fn sign_device_bundle(&self, payload: Vec<u8>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("sign_device_bundle", || {
            self.handle
                .with_blocking(move |c| c.sign_device_bundle(&payload))
                .map_err(|e| Refused::new(format!("{e:?}")))
        })
    }

    /// On a device holding no seed: take the bundle the ceremony device
    /// signed, and publish it where a serving node is attached.
    pub fn take_signed_bundle(&self, signed: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("take_signed_bundle", || {
            let msg = self
                .handle
                .with_blocking(move |c| c.take_signed_bundle(&signed))
                .map_err(|e| Refused::new(format!("{e:?}")))?;
            if let Some(m) = msg {
                self.net.carry(vec![m])?;
            }
            self.save()
        })
    }

    /// This client's own identity.
    pub fn me(&self) -> Id {
        crate::diag::call_plain("me", || id_of(&self.handle.me()))
    }

    /// This client's own public identity as `KeyMaterial`
    /// (`wire-format.md` §2.2).
    ///
    /// The keyhash [`Self::me`] returns names an identity to somebody who
    /// already holds its keys; this carries the keys themselves, and is
    /// what a participant hands to whoever must pin it — the hash of what
    /// this returns being that keyhash (`wire-format.md` §3.4). Public in
    /// its entirety: it is the half of the identity meant to travel.
    pub fn material(&self) -> Vec<u8> {
        crate::diag::call_plain("material", || {
            self.handle.with_blocking(|c| c.public.key_material())
        })
    }

    /// Which payload construction this build runs, for the shell to show
    /// its user (`light-client-requirements.md` §3): a value, and the
    /// wording is the shell's, in its user's own language.
    #[must_use]
    pub fn payload_construction(&self) -> crate::types::Construction {
        crate::diag::call_plain("payload_construction", || {
            self.handle
                .with_blocking(|c| c.payload_construction())
                .into()
        })
    }

    /// Begin a ceremony with `counterparty`, nominating witnesses from
    /// their neighbourhood (`light-client-requirements.md` §1.1).
    ///
    /// The intent is for the shell to carry to the other device by
    /// whatever means the two have; the protocol does not say how, and
    /// neither does this.
    pub fn begin(
        &self,
        counterparty: Option<Id>,
        nominees: Vec<Id>,
        initiator: bool,
    ) -> Result<(), Refused> {
        crate::diag::call("begin", || {
            let cp = match counterparty {
                Some(c) => {
                    Some(keyhash(&c).ok_or_else(|| Refused::new("a counterparty is 32 bytes"))?)
                }
                None => None,
            };
            let noms: Option<Vec<Keyhash>> = nominees.iter().map(|n| keyhash(n)).collect();
            let noms = noms.ok_or_else(|| Refused::new("a nominee is 32 bytes"))?;
            self.handle
                .with_blocking(move |c| c.begin(cp, noms, initiator).map(|_| ()))
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// **The first QR this device shows** (`wire-format.md` §14.3.1): its
    /// identity and its contribution, as the bytes to put on the screen.
    ///
    /// Neither value is secret, and the section says so. What the optical
    /// channel buys is that each party read it off a screen it was looking
    /// at, which costs being there — the cost design §1 meters.
    pub fn optical_contribution(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("optical_contribution", || {
            self.handle
                .with_blocking(|c| c.optical_contribution())
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// The counterparty's first QR, as the camera read it. Who showed it
    /// comes back; a QR naming anybody but this ceremony's counterparty is
    /// refused.
    pub fn take_optical(&self, bytes: Vec<u8>) -> Result<Id, Refused> {
        crate::diag::call("take_optical", || {
            let taken = bytes.clone();
            let who = self.handle.with_blocking(move |c| {
                c.take_optical(&taken)
                    .map(|k| k.to_vec())
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })?;
            // the transport pins the same key the client did, so the
            // payload session to this party can open (`wire-format.md`
            // §14.3.1; design §12.3's first contact)
            if let Ok(oc) = rhtn_client::local::OpticalContribution::decode(&bytes)
                && let Some(id) = rhtn_crypto::Identity::from_key_material(&oc.material)
            {
                self.net.pin_identity(&id);
            }
            Ok(who)
        })
    }

    /// **The second QR**: the ceremony-id this device computed from the two
    /// contributions (§14.3.1, design §7.5.2).
    pub fn transcript_confirm(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("transcript_confirm", || {
            self.handle
                .with_blocking(|c| c.transcript_confirm())
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// The counterparty's second QR, checked against this device's own:
    /// *"a mismatch is where a man in the middle shows, and the ceremony
    /// stops"*. The agreed ceremony-id comes back, which is what both
    /// devices name the ceremony by from here on.
    pub fn take_transcript(&self, bytes: Vec<u8>) -> Result<Id, Refused> {
        crate::diag::call("take_transcript", || {
            self.handle.with_blocking(move |c| {
                c.take_transcript(&bytes)
                    .map(|id| id.to_vec())
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// **What the bearer carries** (§14.3.2): this device's intent as its
    /// carriage set — the exchange, then one continuation per 256 bundle
    /// entries beyond the first (§5.4).
    ///
    /// Each is an opaque string to move on whatever bearer the two devices
    /// have, in the order given. A shell reads none of them.
    pub fn intent_carriage(&self) -> Result<Vec<Vec<u8>>, Refused> {
        crate::diag::call("intent_carriage", || {
            self.handle
                .with_blocking(|c| c.intent_carriage())
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// The counterparty's carriage set, in the order it arrived. How many
    /// continuations were accepted comes back: fewer than were sent is a
    /// shorter bundle and a smaller *n*, never a failed ceremony (§5.4).
    pub fn take_intent_carriage(&self, from: Id, carriage: Vec<Vec<u8>>) -> Result<u32, Refused> {
        crate::diag::call("take_intent_carriage", || {
            let f = keyhash(&from).ok_or_else(|| Refused::new("a party is 32 bytes"))?;
            self.handle.with_blocking(move |c| {
                c.take_intent_carriage(f, &carriage)
                    .map(|n| n as u32)
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// **This side's candidates for the counterparty, anchored, for the
    /// bearer** (`wire-format.md` §14.3.2, design §12.6.3): gathered on the
    /// direct socket and sealed under the ceremony, so that two parties who
    /// met with no node leave with a route to each other.  Refused where no
    /// counterparty is pinned yet or nothing may be gathered.
    pub fn candidate_offer(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("candidate_offer", || {
            let peer = self
                .handle
                .with_blocking(|c| c.counterparty())
                .ok_or_else(|| Refused::new("no counterparty yet"))?;
            let cands = self.net.offer_bytes(peer)?;
            self.handle
                .with_blocking(move |c| c.candidate_carriage(cands))
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// **The counterparty's candidates off the bearer**: checked against the
    /// ceremony's anchor, then dialled.  Whether a direct path is now held.
    pub fn take_candidate_offer(&self, bytes: Vec<u8>) -> Result<bool, Refused> {
        crate::diag::call("take_candidate_offer", || {
            let peer = self
                .handle
                .with_blocking(|c| c.counterparty())
                .ok_or_else(|| Refused::new("no counterparty yet"))?;
            let cands = self
                .handle
                .with_blocking(move |c| c.take_candidate_carriage(&bytes))
                .map_err(|a| Refused::new(format!("{a:?}")))?;
            self.net.open_offered(peer, cands)
        })
    }

    /// Candidates for the direct path on the ceremony's local interfaces
    /// (§14.3.2, design §12.6.3), anchored.
    pub fn candidate_carriage(&self, candidates: Vec<u8>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("candidate_carriage", || {
            self.handle
                .with_blocking(move |c| c.candidate_carriage(candidates))
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// The counterparty's candidates, anchored or refused. An address to
    /// dial comes back, not evidence of anything.
    pub fn take_candidate_carriage(&self, bytes: Vec<u8>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("take_candidate_carriage", || {
            self.handle
                .with_blocking(move |c| c.take_candidate_carriage(&bytes))
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// This client's capture key as the anchored message the bearer
    /// carries at capture time (`wire-format.md` §14.3.2, design
    /// §7.5.2.6): [`Participant::capture_key`] under the ceremony's id.
    ///
    /// **The key crosses this boundary as bytes the shell owns**: what the
    /// kernel held of it is wiped as the call returns, and the shell wipes
    /// its copy once the bearer has carried it.
    pub fn capture_key_carriage(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("capture_key_carriage", || {
            self.handle
                .with_blocking(|c| c.capture_key_carriage().map(|k| k.to_vec()))
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// The counterparty's capture key, anchored to this ceremony or
    /// refused: the 32 bytes to hand [`Participant::capture`].  The
    /// carried bytes are wiped here once read; the shell wipes the key it
    /// is handed once the capture has taken it.
    pub fn take_capture_key_carriage(&self, bytes: Vec<u8>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("take_capture_key_carriage", || {
            let bytes = zeroize::Zeroizing::new(bytes);
            self.handle
                .with_blocking(move |c| c.take_capture_key_carriage(&bytes).map(|k| k.to_vec()))
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// The conversation's carriages for the co-present counterparty, in
    /// order, sealed under the local session (`wire-format.md` §14.3.2):
    /// what the shell's bearer carries, as it carried the intent, the
    /// outcomes and the capture key.
    ///
    /// **Drained, not read.** Each is returned once. The counterparty's
    /// leg of kinds 9 to 18 crosses the local interface (design §7.1)
    /// and no node carries it, so a shell that does not call this after
    /// every step of the conversation leaves the ceremony waiting on
    /// messages that are sitting here. A step produces one from a call
    /// on this side, from a carriage taken, or from a message that
    /// arrived over the network, so all three are moments to drain.
    #[must_use]
    pub fn carriages(&self) -> Vec<Vec<u8>> {
        crate::diag::call_plain("carriages", || self.handle.with_blocking(|c| c.carriages()))
    }

    /// A conversation carriage the bearer delivered from the counterparty:
    /// opened under the local session and taken as the step it carries,
    /// whose outcome reaches the shell as [`Event::Conversed`] exactly as
    /// a step that arrived over the network does.
    ///
    /// A carriage that will not open is dropped and nothing is said:
    /// there is no party to tell, since whoever sent it held no key
    /// (`wire-format.md` §14.3.2). What the step owes a witness goes out
    /// on the courier from here, where one is attached.
    pub fn take_carriage(&self, bytes: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("take_carriage", || {
            self.net.take_carriage(&self.handle, bytes)?;
            self.net.carry_outbox(&self.handle)?;
            let _ = self.save();
            Ok(())
        })
    }

    /// **This device's prekey handover for the co-present counterparty**
    /// (`wire-format.md` §14.3.4): its signed bundle and one one-time key,
    /// anchored to the ceremony and sealed under the local session, for
    /// the shell's bearer to carry like the capture key.
    ///
    /// **What it is for**: opening an end-to-end channel needs the peer's
    /// published bundle and fetching one needs a node, so a pair whose
    /// ceremony completed with no node reachable (design §6.4) would
    /// otherwise hold a record and be unable to say a word to each other.
    /// Handed over here, neither asks anybody.
    pub fn prekey_carriage(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("prekey_carriage", || {
            let out = self
                .handle
                .with_blocking(|c| c.prekey_carriage())
                .map_err(|a| Refused::new(format!("{a:?}")))?;
            let _ = self.save();
            Ok(out)
        })
    }

    /// The counterparty's handover, opened and kept: the bundle verified
    /// under the identity pinned at `wire-format.md` §14.3.1 and held as
    /// prefetched material, the one-time key kept for the first session
    /// opened with that device.
    ///
    /// A handover of another ceremony, or one whose bundle does not
    /// verify, is refused: the signature is what makes the material
    /// trustworthy and never the channel it arrived on.
    pub fn take_prekey_carriage(&self, bytes: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("take_prekey_carriage", || {
            self.handle
                .with_blocking(move |c| c.take_prekey_carriage(&bytes))
                .map_err(|a| Refused::new(format!("{a:?}")))?;
            let _ = self.save();
            Ok(())
        })
    }

    /// The verifiers this client selected of the counterparty's pool, and
    /// the basis each was eligible on (`wire-format.md` §5.5).
    pub fn select_verifiers(&self) -> Result<Vec<Selected>, Refused> {
        crate::diag::call("select_verifiers", || {
            self.handle
                .with_blocking(|c| c.select_verifiers())
                .map(|picked| {
                    picked
                        .into_iter()
                        .map(|(v, b)| Selected {
                            verifier: id_of(&v),
                            basis: b as u32,
                        })
                        .collect()
                })
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// The responses gathered about the counterparty so far, as a screen
    /// shows them.
    pub fn responses(&self) -> Vec<Response> {
        crate::diag::call_plain("responses", || {
            self.handle.with_blocking(|c| {
                c.responses()
                    .iter()
                    .filter_map(|r| rhtn_client::query::Response::read(r).ok())
                    .map(|r| Response {
                        verifier: id_of(&r.verifier),
                        subject: id_of(&r.subject),
                        answer: Answer::of(r.verdict),
                    })
                    .collect()
            })
        })
    }

    /// Consent to a query about this subject, or decline
    /// (`light-client-requirements.md` §1.4).
    ///
    /// **The person is asked, and this returns what they answered.** A
    /// shell that consented on their behalf would be signing for them.
    /// Consenting is also what mints the grant, where one is owed: the
    /// two are one act of the subject, so they leave together.
    pub fn consent(&self, query: Vec<u8>) -> Result<Option<Consented>, Refused> {
        crate::diag::call("consent", || {
            self.handle.with_blocking(move |c| {
                let q =
                    rhtn_client::query::VerificationQuery::decode(&query).map_err(Refused::new)?;
                Ok(c.consent(&q).map(|(consent, grant)| Consented {
                    consent,
                    // the grant carries the capture key; the client's copy
                    // is wiped with it and the crossing copy is the shell's
                    grant: grant.map(|g| g.encode().to_vec()),
                }))
            })
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
    pub fn attach(
        &self,
        serving: Id,
        addresses: Vec<String>,
        population: Vec<Id>,
    ) -> Result<Attached, Refused> {
        crate::diag::call("attach", || {
            let node =
                keyhash(&serving).ok_or_else(|| Refused::new("a serving node is 32 bytes"))?;
            let pop: Option<Vec<Keyhash>> = population.iter().map(|p| keyhash(p)).collect();
            let pop = pop.ok_or_else(|| Refused::new("a population member is 32 bytes"))?;
            let addrs: Result<Vec<SocketAddr>, Refused> = addresses
                .iter()
                .map(|a| {
                    a.parse::<SocketAddr>()
                        .map_err(|_| Refused::new(format!("{a} is not an address")))
                })
                .collect();
            let addrs = addrs?;
            if addrs.is_empty() {
                return Err(Refused::new("a serving node needs at least one address"));
            }
            let a = self.net.attach(&self.handle, node, &addrs, pop)?;
            self.save()?;
            Ok(a)
        })
    }

    /// Close the session cleanly and keep everything else: what a shell
    /// does when it is about to be suspended, so the serving node queues
    /// for this device from now rather than after three missed intervals.
    /// The state is written first.
    pub fn detach(&self) -> Result<(), Refused> {
        crate::diag::call("detach", || {
            self.save()?;
            self.net.detach();
            Ok(())
        })
    }

    /// Routine maintenance: rotate a prekey whose interval has elapsed,
    /// replenish a pool that has fallen low, and ask for a binding this
    /// client wants.  What that produces goes to the serving node from
    /// here.
    ///
    /// Refused where nothing is attached: maintenance is a conversation
    /// with a node, and there is no node.
    pub fn maintain(&self) -> Result<(), Refused> {
        crate::diag::call("maintain", || {
            self.net.maintain(&self.handle)?;
            self.save()
        })
    }

    /// Send `bytes` of `kind` to `to`, over the direct path where one is
    /// held and through the serving node otherwise (design §14.1.1).
    ///
    /// A shell's own traffic is [`crate::types::KIND_APPLICATION`]; the
    /// other kinds are the client's own and the adaptors answer them
    /// without a shell seeing them (design §14.2.4.6).
    pub fn send(&self, to: Id, kind: u64, bytes: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("send", || {
            let peer = keyhash(&to).ok_or_else(|| Refused::new("a recipient is 32 bytes"))?;
            self.net.send(peer, kind, bytes)?;
            // the ratchet advanced: a restart that lost this could not read
            // the reply
            self.save()
        })
    }

    /// Where this client asks to be rung when something is waiting
    /// (design §14.1.5), or `None` to withdraw whatever the node holds.
    ///
    /// **The endpoint is the user's choice** and this side never obtains
    /// one: the shell hands over what the person's own service gave it.
    pub fn wake(&self, endpoint: Option<Wake>) -> Result<(), Refused> {
        crate::diag::call("wake", || self.net.wake(endpoint))
    }

    /// The next thing that arrived, or nothing within `timeout_ms`.
    ///
    /// **What crosses outward is what to draw.** Payload has already been
    /// decrypted here and the ciphertext never leaves.
    pub fn next_event(&self, timeout_ms: u64) -> Option<Event> {
        crate::diag::call_plain("next_event", || {
            let e = self.net.next_event(timeout_ms);
            if e.is_some() {
                // what arrived advanced a ratchet; the event is the shell's
                // either way, and a storage that did not take the state is
                // reported at the next call that can carry a refusal
                let _ = self.save();
            }
            // **and what changed, after whatever arrived**: the nouns the
            // kernel marked since the last call, as one event, so a shell
            // keeping a copy refreshes exactly those. Arrivals first, since
            // an arrival is usually what changed them.
            match e {
                Some(e) => {
                    let nouns = self.handle.with_blocking(|c| c.take_changed());
                    if nouns.contains(&rhtn_model::Noun::Contacts) {
                        self.pin_known();
                    }
                    if !nouns.is_empty() {
                        self.pending_changed.lock().unwrap().push(nouns);
                    }
                    Some(e)
                }
                None => {
                    let mut pending = std::mem::take(&mut *self.pending_changed.lock().unwrap());
                    let fresh = self.handle.with_blocking(|c| c.take_changed());
                    if fresh.contains(&rhtn_model::Noun::Contacts) {
                        self.pin_known();
                    }
                    if !fresh.is_empty() {
                        pending.push(fresh);
                    }
                    let mut nouns: Vec<rhtn_model::Noun> = pending.into_iter().flatten().collect();
                    nouns.sort();
                    nouns.dedup();
                    (!nouns.is_empty()).then_some(Event::Changed { nouns })
                }
            }
        })
    }

    /// Where every node this client can resolve sits (`rhtn_model::Placed`;
    /// `light-client-requirements.md` §4.2), one row per position.
    #[must_use]
    pub fn places(&self) -> Vec<rhtn_model::Placed> {
        crate::diag::call_plain("places", || {
            self.handle.with_blocking(|c| {
                c.horizon
                    .resolvable()
                    .into_iter()
                    .flat_map(|n| {
                        c.horizon
                            .places_of(&n)
                            .into_iter()
                            .map(move |p| rhtn_model::Placed {
                                node: n,
                                position: p.clone(),
                            })
                            .collect::<Vec<_>>()
                    })
                    .collect()
            })
        })
    }

    /// The infra nodes whose endpoint records this client holds, each with
    /// the addresses it answers at (`rhtn_model::Instance`;
    /// `light-client-requirements.md` §4). Composed here rather than in
    /// the kernel because an address is a transport point.
    #[must_use]
    pub fn instances(&self) -> Vec<rhtn_model::Instance> {
        crate::diag::call_plain("instances", || {
            self.handle.with_blocking(|c| {
                c.horizon
                    .reachable_infra()
                    .into_iter()
                    .map(|(keyhash, points)| rhtn_model::Instance {
                        keyhash,
                        addresses: points
                            .iter()
                            .filter_map(|p| {
                                rhtn_transport::session::NetworkPoint::decode_bytes(p).ok()
                            })
                            .map(|p| p.socket().to_string())
                            .collect(),
                    })
                    .collect()
            })
        })
    }

    /// Everybody this client can place without asking anyone.
    #[must_use]
    pub fn resolvable(&self) -> Vec<Id> {
        crate::diag::call_plain("resolvable", || {
            self.handle
                .with_blocking(|c| c.horizon.resolvable().iter().map(id_of).collect())
        })
    }

    /// How many adoption or sibling edges away a party is, or nothing
    /// beyond the horizon.
    #[must_use]
    pub fn distance(&self, node: Id) -> Option<u32> {
        crate::diag::call_plain("distance", || {
            let k = keyhash(&node)?;
            self.handle
                .with_blocking(move |c| c.horizon.distance(&k).map(|d| d as u32))
        })
    }

    /// Whether this client holds the transaction `txid` names, in its own
    /// archive or in what its patron propagated.
    #[must_use]
    pub fn holds(&self, txid: Id) -> bool {
        crate::diag::call_plain("holds", || {
            let Some(t) = keyhash(&txid) else {
                return false;
            };
            self.handle
                .with_blocking(move |c| c.horizon.holds(&t) || c.archive.get(&t).is_some())
        })
    }

    /// **The witnesses a held presence record names**, each as the pair
    /// (witness, nominated by), read from the signed body: what a screen
    /// says about who attested comes from the record and not from the
    /// conversation's last snapshot, which on the bench was taken before
    /// the witness signed and said nobody had (run 8bc1291-b1-node-1,
    /// 2026-10-25).  Empty where the record is not held or names none.
    #[must_use]
    pub fn record_witnesses(&self, txid: Id) -> Vec<Vec<Id>> {
        crate::diag::call_plain("record_witnesses", || {
            let Some(t) = keyhash(&txid) else {
                return Vec::new();
            };
            self.handle.with_blocking(move |c| {
                c.store
                    .records
                    .get(&t)
                    .and_then(|b| rhtn_archive::record::Record::parse(b).ok())
                    .map(|r| {
                        r.witnesses()
                            .into_iter()
                            .map(|(w, by)| vec![id_of(&w), id_of(&by)])
                            .collect()
                    })
                    .unwrap_or_default()
            })
        })
    }

    /// How many records the copy is a fold over.
    #[must_use]
    pub fn records(&self) -> u64 {
        crate::diag::call_plain("records", || {
            self.handle.with_blocking(|c| c.horizon.records() as u64)
        })
    }

    /// Drop what has left the horizon, and say how many parties were
    /// forgotten (`light-client-requirements.md` §4.2).
    pub fn prune(&self) -> u64 {
        crate::diag::call_plain("prune", || {
            self.handle.with_blocking(|c| c.horizon.prune() as u64)
        })
    }

    /// Whether a session with a serving node is held right now.
    pub fn attached(&self) -> bool {
        crate::diag::call_plain("attached", || self.net.session().is_some())
    }

    /// Drop the ceremony under way without a record: the shell's Stop,
    /// told to the kernel so that nothing of the conversation is answered
    /// after it.  Nothing sealed is filed and the seed goes with it; the
    /// counterparty and the witnesses hear nothing, there being no message
    /// for it (`wire-format.md` §7.10.1), and read the silence.  Outside a
    /// ceremony it does nothing.
    pub fn abandon(&self) -> Result<(), Refused> {
        crate::diag::call("abandon", || {
            self.handle.with_blocking(|c| c.abandon());
            self.save()
        })
    }

    /// The retention this device declares at its next intent, in whole
    /// years (design §7.5.1).
    #[must_use]
    pub fn retention_years(&self) -> u64 {
        crate::diag::call_plain("retention_years", || {
            self.handle.with_blocking(|c| c.retention_years())
        })
    }

    /// Declare the retention for the ceremonies from here on, in whole
    /// years (design §7.5.1).  Refused at zero and while a ceremony is
    /// under way, its intent having stated the figure.
    pub fn set_retention_years(&self, years: u64) -> Result<(), Refused> {
        crate::diag::call("set_retention_years", || {
            self.handle
                .with_blocking(move |c| c.set_retention_years(years))
                .map_err(Refused::new)?;
            self.save()
        })
    }

    /// **The window this client will not sign inside, as a witness**, in
    /// seconds, measured from its own receipt of the request (design §7.1
    /// item 9; `light-client-requirements.md` §1.2).
    #[must_use]
    pub fn witness_window_s(&self) -> u64 {
        crate::diag::call_plain("witness_window_s", || {
            self.handle.with_blocking(|c| c.cfg.witness_window_s)
        })
    }

    /// Set it.  **Not a control a shipped client offers its person** — the
    /// figure is the design's and a client that shortened it would be
    /// weakening its own attestation — but the instrument that drives
    /// process tests and the bench needs it: a four-minute wait is
    /// untestable in a process whose clock it does not own, and a bench
    /// run wants the window short enough to repeat [author, 2026-10-21].
    /// Zero is the wait off.
    pub fn set_witness_window_s(&self, seconds: u64) -> Result<(), Refused> {
        crate::diag::call("set_witness_window_s", || {
            self.handle
                .with_blocking(move |c| c.cfg.witness_window_s = seconds);
            Ok(())
        })
    }

    /// The ceremony this client is in, once both intents have crossed.
    #[must_use]
    pub fn ceremony(&self) -> Option<Id> {
        crate::diag::call_plain("ceremony", || {
            self.handle
                .with_blocking(|c| c.ceremony_id().map(|i| i.to_vec()))
        })
    }

    /// The key this client's own captures will be sealed under, for the
    /// counterparty to capture with (design §7.5.2).  The kernel's copy is
    /// wiped as the call returns; the bytes handed across are the shell's
    /// to wipe.
    pub fn capture_key(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("capture_key", || {
            self.handle.with_blocking(|c| {
                c.capture_key()
                    .map(|k| k.to_vec())
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// **Begin the capture's hold** (design §7.1 item 5): planned now,
    /// turned by [`Participant::capture_tick`] from a thread of the shell's
    /// for the ceremony's window, and sealed by [`Participant::capture_seal`]
    /// when the window is out.  The three together are what
    /// [`Participant::capture`] does in one call for a shell with nothing
    /// else to do meanwhile; a phone has the verification to run.
    pub fn capture_begin(&self) -> Result<(), Refused> {
        crate::diag::call("capture_begin", || {
            self.handle
                .with_blocking(|c| c.capture_begin())
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// One turn of the hold: a frame taken on the camera, prompted where a
    /// prompt falls now.  Says when the next is due, how many frames are
    /// kept, whether a template is in hand for the queries, and whether the
    /// window has elapsed.  A lost hold is refused with its reason, and the
    /// ceremony is over.
    pub fn capture_tick(&self) -> Result<CaptureTurn, Refused> {
        crate::diag::call_plain("capture_tick", || {
            self.handle
                .with_blocking(|c| c.capture_tick())
                .map(|t| CaptureTurn {
                    done: t.done,
                    next_ms: t.next_ms,
                    frames: t.frames,
                    template_ready: t.template_ready,
                    face: t.face,
                    away_ms: t.away_ms,
                })
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// Seal the capture under the counterparty's key once the window has
    /// elapsed; refused with `Waiting` while it has not.  **The key is
    /// discarded once the capture is sealed**, every copy the kernel made
    /// of it zeroed by the time this returns; the bytes the shell handed in
    /// are the shell's to wipe.
    pub fn capture_seal(&self, their_key: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("capture_seal", || {
            let their_key = zeroize::Zeroizing::new(their_key);
            let k = their_key
                .as_slice()
                .try_into()
                .map(zeroize::Zeroizing::<[u8; 32]>::new)
                .map_err(|_| Refused::new("a capture key is 32 bytes"))?;
            self.handle.with_blocking(move |c| {
                c.capture_seal(k)
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// Run the guided capture of the counterparty, sealed under the key
    /// they supplied.  **The key is discarded once the capture is sealed**,
    /// and discarded means wiped: every copy the kernel made of it is
    /// zeroed by the time this returns, so this client holds no
    /// decryptable likeness of them.  The bytes the shell handed in are
    /// the shell's to wipe.
    pub fn capture(&self, their_key: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("capture", || {
            let their_key = zeroize::Zeroizing::new(their_key);
            let k = their_key
                .as_slice()
                .try_into()
                .map(zeroize::Zeroizing::<[u8; 32]>::new)
                .map_err(|_| Refused::new("a capture key is 32 bytes"))?;
            self.handle
                .with_blocking(move |c| c.capture(k).map_err(|a| Refused::new(format!("{a:?}"))))
        })
    }

    /// The query to put to one selected verifier (`wire-format.md` §5.5),
    /// encoded as the verifier will read it.
    pub fn query_for(&self, verifier: Id) -> Result<Vec<u8>, Refused> {
        crate::diag::call("query_for", || {
            let v = keyhash(&verifier).ok_or_else(|| Refused::new("a verifier is 32 bytes"))?;
            self.handle.with_blocking(move |c| {
                c.query_for(v)
                    .map(|q| q.encode())
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// The request that carries a query, the subject's consent and the
    /// basis the verifier was selected on.
    pub fn request(
        &self,
        query: Vec<u8>,
        consent: Vec<u8>,
        basis: u32,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("request", || {
            let b = basis_of(basis)?;
            self.handle.with_blocking(move |c| {
                let q =
                    rhtn_client::query::VerificationQuery::decode(&query).map_err(Refused::new)?;
                Ok(c.request(&q, consent, b))
            })
        })
    }

    /// Take a request put to this client as a verifier.
    ///
    /// Nothing where the grant it needs has not arrived — it waits, bounded
    /// — and a refusal where the input is not evidence, since **no signed
    /// response is fabricated** for one.
    pub fn take_query(&self, from: Id, bytes: Vec<u8>) -> Result<Option<Answered>, Refused> {
        crate::diag::call("take_query", || {
            let f = keyhash(&from).ok_or_else(|| Refused::new("a requester is 32 bytes"))?;
            self.handle
                .with_blocking(move |c| match c.take_query(f, &bytes) {
                    rhtn_client::verifier::QueryOutcome::Answered(a) => Ok(Some(answered_of(&a))),
                    rhtn_client::verifier::QueryOutcome::AwaitingGrant => Ok(None),
                    rhtn_client::verifier::QueryOutcome::Closed(why) => Err(Refused::new(why)),
                })
        })
    }

    /// Take a capture key released to this client as a verifier.
    pub fn take_grant(&self, from: Id, bytes: Vec<u8>) -> Result<Option<Answered>, Refused> {
        crate::diag::call("take_grant", || {
            let f = keyhash(&from).ok_or_else(|| Refused::new("a subject is 32 bytes"))?;
            self.handle
                .with_blocking(move |c| match c.take_grant(f, &bytes) {
                    rhtn_client::verifier::GrantOutcome::Answered(a) => Ok(Some(answered_of(&a))),
                    rhtn_client::verifier::GrantOutcome::Buffered
                    | rhtn_client::verifier::GrantOutcome::Ignored => Ok(None),
                    rhtn_client::verifier::GrantOutcome::Rejected(why) => Err(Refused::new(why)),
                })
        })
    }

    /// Take a verifier's response about the counterparty, handed in by
    /// the shell; one that arrives on the end-to-end path is taken by the
    /// kernel and surfaces as [`crate::net::Event::Answered`].
    pub fn take_response(&self, bytes: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("take_response", || {
            self.handle
                .with_blocking(move |c| c.take_response(&bytes).map(|_| ()).map_err(Refused::new))
        })
    }

    /// Put a consented query to its verifier (`wire-format.md` §5.6): the
    /// request is built and carried on the end-to-end path — direct where
    /// the path is held, through the serving nodes otherwise (design
    /// §12.6.3) — and the answer comes back as
    /// [`crate::net::Event::Answered`].
    pub fn put_query(&self, query: Vec<u8>, consent: Vec<u8>, basis: u32) -> Result<(), Refused> {
        crate::diag::call("put_query", || {
            let req = self.request(query, consent, basis)?;
            self.net.carry(vec![rhtn_client::ceremony::Msg::Query(req)])
        })
    }

    /// The responses gathered so far, as they will sit in the body.
    #[must_use]
    pub fn gathered(&self) -> Vec<Vec<u8>> {
        crate::diag::call_plain("gathered", || self.handle.with_blocking(|c| c.responses()))
    }

    /// Who each side nominated: this client's, then the counterparty's.
    #[must_use]
    pub fn nominees(&self) -> Nominees {
        crate::diag::call_plain("nominees", || {
            self.handle.with_blocking(|c| {
                let (mine, theirs) = c.nominees();
                Nominees {
                    mine: mine.iter().map(id_of).collect(),
                    theirs: theirs.iter().map(id_of).collect(),
                }
            })
        })
    }

    /// **Hold a party as a contact by its key material alone**: what a
    /// shell does with a key handed over without a locator. Such a contact
    /// cannot be dialled (there is no address to dial) but it is a party
    /// this client chose, so a direct path the party offers is opened
    /// (design §12.6.3). The bench uses it to stand in for the meeting a
    /// witness and its nominator would have had.
    pub fn hold_contact(&self, material: Vec<u8>) -> Result<Id, Refused> {
        crate::diag::call("hold_contact", || {
            let who = self.handle.with_blocking(move |c| {
                let identity = rhtn_crypto::Identity::from_key_material(&material)
                    .ok_or_else(|| Refused::new("not key material"))?;
                c.contact(&identity, rhtn_client::contacts::How::Introduced, true);
                Ok::<_, Refused>(identity.keyhash)
            })?;
            self.save()?;
            Ok(id_of(&who))
        })
    }

    /// **Forget a contact** (`light-client-requirements.md` §4.3: the
    /// contacts are this client's own, bounded and pruned by it).  Whether
    /// one was held.  The bench's way of taking something out of the
    /// dataset to watch a peer's copy bring it back (design §6.3).
    pub fn forget_contact(&self, who: Id) -> Result<bool, Refused> {
        crate::diag::call("forget_contact", || {
            let k = keyhash(&who).ok_or_else(|| Refused::new("a contact is 32 bytes"))?;
            let had = self
                .handle
                .with_blocking(move |c| c.store.contacts.remove(&k).is_some());
            self.save()?;
            Ok(had)
        })
    }

    /// **Forget every contact** (the bench's lever, design §6.3): the
    /// store's contacts go, how many is answered, and the next round's
    /// returned blob is what brings them back.
    pub fn forget_contacts(&self) -> Result<u32, Refused> {
        crate::diag::call("forget_contacts", || {
            let n = self.handle.with_blocking(|c| c.forget_contacts());
            self.save()?;
            Ok(n as u32)
        })
    }

    /// **The provider credentials this operator holds**, opaque to the
    /// kernel (`light-client-requirements.md` §2, §10): what a launch flow
    /// speaks to a hosting provider with, carried in the backup under the
    /// identity (design §13.7.1, ARC-28) and never in the archive.  The
    /// shell decides the bytes' shape; the kernel keeps and carries them.
    #[must_use]
    pub fn provider_credentials(&self) -> Option<Vec<u8>> {
        crate::diag::call_plain("provider_credentials", || {
            self.handle.with_blocking(|c| c.provider_credential.clone())
        })
    }

    /// Hold `bytes` as the provider credentials from now on, or none where
    /// empty; written at once, since what a device loss would take away
    /// is exactly this.
    pub fn hold_provider_credentials(&self, bytes: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("hold_provider_credentials", || {
            self.handle.with_blocking(move |c| {
                c.provider_credential = if bytes.is_empty() { None } else { Some(bytes) };
            });
            self.save()
        })
    }

    /// **Forget every sealed capture held of `subject`**: how many went.
    /// The bench's act, over adb, for the verifier that is asked about a
    /// party it holds nothing on (VER-008's unavailable answer, its own
    /// held evidence refuting the basis claimed for it), which no meeting
    /// produces by itself since a subject introduces only parties it has
    /// met [author, 2026-10-25: a tethered wipe, never a screen control].
    pub fn forget_captures(&self, subject: Id) -> Result<u64, Refused> {
        crate::diag::call("forget_captures", || {
            let k = keyhash(&subject).ok_or_else(|| Refused::new("a subject is 32 bytes"))?;
            let n = self.handle.with_blocking(move |c| {
                let before = c.store.sealed.len();
                c.store.sealed.retain(|_, s| s.subject != k);
                (before - c.store.sealed.len()) as u64
            });
            self.save()?;
            Ok(n)
        })
    }

    /// **This client's own introduction** (`wire-format.md` §2.3): its key
    /// material with its own signed locator, for another party to take with
    /// [`Participant::take_introduction`] and hold it by.
    pub fn own_introduction(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("own_introduction", || {
            self.handle
                .with_blocking(|c| c.own_introduction())
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// **Take an introduction** (`wire-format.md` §2.3): somebody's key
    /// material and a locator of theirs, scanned or pasted, which is the
    /// one way a person brings a party into this client without having
    /// met them (`light-client-requirements.md` §1.4). The party, or why
    /// not.
    ///
    /// The state is written here, so an intake survives a restart.
    pub fn take_introduction(&self, bytes: Vec<u8>) -> Result<Id, Refused> {
        crate::diag::call("take_introduction", || {
            let who = self
                .handle
                .with_blocking(move |c| c.take_introduction(&bytes))
                .map_err(Refused::new)?;
            self.pin_known();
            self.save()?;
            Ok(id_of(&who))
        })
    }

    /// **Take whatever an intake code carries**: an anchored introduction
    /// (`wire-format.md` §2.4), a bare introduction (§2.3), or a node's
    /// endpoint record (§7.6). The kernel decides which by parsing, so
    /// nobody has to be asked what kind of code they were handed.
    ///
    /// What it was, and whom it was about.
    pub fn take_intake(&self, bytes: Vec<u8>) -> Result<Intake, Refused> {
        crate::diag::call("take_intake", || {
            let took = self.handle.with_blocking(move |c| {
                if let Ok(who) = c.take_anchored_introduction(&bytes) {
                    return Ok(Intake {
                        kind: "an introduction, with where its anchor answers".into(),
                        who: id_of(&who),
                    });
                }
                if let Ok(who) = c.take_introduction(&bytes) {
                    return Ok(Intake {
                        kind: "an introduction".into(),
                        who: id_of(&who),
                    });
                }
                match c.take_endpoint(&bytes) {
                    Ok(node) => Ok(Intake {
                        kind: "where a node answers".into(),
                        who: id_of(&node),
                    }),
                    Err(why) => Err(why),
                }
            });
            let took = took.map_err(Refused::new)?;
            self.save()?;
            Ok(took)
        })
    }

    /// **This client's own code to hand over** (`wire-format.md` §2.4):
    /// its introduction with the anchor of its own locator and the record
    /// of where that anchor answers. Refused where it cannot say where it
    /// sits, or holds no record for its anchor — a card nobody could act
    /// on is not offered.
    pub fn own_card(&self) -> Result<Vec<u8>, Refused> {
        crate::diag::call("own_card", || {
            self.handle
                .with_blocking(|c| c.own_card())
                .map_err(|a| Refused::new(format!("{a:?}")))
        })
    }

    /// **Take a node's endpoint record** (`wire-format.md` §7.6): where
    /// **Take a node's endpoint record** (`wire-format.md` §7.6): where
    /// that node answers, signed by it. What gives an introduction an
    /// address to start from where this client holds no node of its own
    /// (design §12.3 Case 1). The node, or why not.
    pub fn take_endpoint(&self, bytes: Vec<u8>) -> Result<Id, Refused> {
        crate::diag::call("take_endpoint", || {
            let node = self
                .handle
                .with_blocking(move |c| c.take_endpoint(&bytes))
                .map_err(Refused::new)?;
            self.save()?;
            Ok(id_of(&node))
        })
    }

    /// Every contact held, most recently met first (`rhtn_model::Contact`;
    /// `light-client-requirements.md` §4.3): full keyhashes and the
    /// wire's `How`, nothing shortened or worded.
    #[must_use]
    pub fn contacts(&self) -> Vec<rhtn_model::Contact> {
        crate::diag::call_plain("contacts", || {
            self.handle.with_blocking(|c| c.contacts_held())
        })
    }

    /// Publish an alias and, if the person chooses, an image, for the
    /// subnet named — this client's own where `kind` is 0, the subnet's
    /// where 1 — signed now and handed to the serving node
    /// (`wire-format.md` §7.11; `light-client-requirements.md` §4.4).  The
    /// image is a format tag (1 PNG, 2 JPEG, 3 WebP) and its bytes, at most
    /// 64 KiB.  Published again at every attach.
    pub fn publish_profile(
        &self,
        subnet: Id,
        kind: u64,
        alias: String,
        image_format: Option<u64>,
        image: Option<Vec<u8>>,
    ) -> Result<(), Refused> {
        crate::diag::call("publish_profile", || {
            let s = keyhash(&subnet).ok_or_else(|| Refused::new("a subnet is 32 bytes"))?;
            let img = match (image_format, image) {
                (Some(f), Some(b)) => Some((f, b)),
                (None, None) => None,
                _ => return Err(Refused::new("an image is a format and its bytes together")),
            };
            self.converse(move |c| c.publish_profile(s, kind, &alias, img))
        })
    }

    /// The name this client publishes to every subnet it has not named
    /// itself differently in, or none.
    #[must_use]
    pub fn default_alias(&self) -> Option<String> {
        crate::diag::call_plain("default_alias", || {
            self.handle
                .with_blocking(|c| c.default_alias().map(str::to_string))
        })
    }

    /// Set that name [author, 2026-10-27], 1 to 64 bytes; it is published
    /// at once to the subnet this client acts in where it holds no name of
    /// its own there, and to each later one at attach.
    pub fn set_default_alias(&self, alias: String) -> Result<(), Refused> {
        crate::diag::call("set_default_alias", || {
            self.converse(move |c| c.set_default_alias(&alias))?;
            self.save()
        })
    }

    /// Fetch every contact's profile for the subnet its locator names,
    /// from where that locator leads (`wire-format.md` §7.11).  What comes
    /// back is kept beside the contact and shows in `contacts`.
    pub fn request_profiles(&self) -> Result<(), Refused> {
        crate::diag::call("request_profiles", || {
            self.converse(move |c| Ok(c.profile_requests()))
        })
    }

    /// The alias a subnet's profile carries, where this client holds one
    /// (`wire-format.md` §7.11 kind 1).
    #[must_use]
    pub fn subnet_alias(&self, subnet: Id) -> Option<String> {
        crate::diag::call_plain("subnet_alias", || {
            let s = keyhash(&subnet)?;
            self.handle.with_blocking(move |c| c.subnet_alias(&s))
        })
    }

    /// **Candidate providers ordered by concentration inside this
    /// client's own horizon, least first** (PRD-11): each ASN the caller
    /// names, with how many parties in the horizon already run inside it.
    /// Nothing is hidden and nothing is scored — the screen marks the
    /// crowded and says what the ordering does not cover.
    #[must_use]
    pub fn providers_by_concentration(&self, asns: Vec<u64>) -> Vec<ProviderRow> {
        crate::diag::call_plain("providers_by_concentration", || {
            self.handle.with_blocking(move |c| {
                c.providers_by_concentration(&asns)
                    .into_iter()
                    .map(|(asn, nodes)| ProviderRow {
                        asn,
                        nodes: u32::try_from(nodes).unwrap_or(u32::MAX),
                        crowded: nodes > 0,
                    })
                    .collect()
            })
        })
    }

    /// **Ask a resource for something, through the node hosting it**
    /// (`wire-format.md` §11.1): the message is the application's own
    /// HTTP/1.1 request and nothing here reads it. What comes back is the
    /// status §11.1 fixes — 0 delivered, 1 refused, 2 unavailable, 3
    /// malformed, 4 no subtree acknowledgement, 5 no matching role — and
    /// the application's answer where one was delivered.
    ///
    /// **Held nowhere.** A resource's answer is the application's
    /// business; this client keeps none of it — except an ask it made of
    /// this client, which is held until the person answers it.
    pub fn request_resource(
        &self,
        node: Id,
        resource: Id,
        message: Vec<u8>,
    ) -> Result<ResourceAnswer, Refused> {
        crate::diag::call("request_resource", || {
            let n = keyhash(&node).ok_or_else(|| Refused::new("a node is 32 bytes"))?;
            let r = keyhash(&resource).ok_or_else(|| Refused::new("a resource is 32 bytes"))?;
            let msg = self
                .handle
                .with_blocking(move |c| c.request_resource(n, r, message));
            self.carry_to_resource(n, r, msg)
        })
    }

    /// **Answer an ask a resource made** (`light-client-requirements.md`
    /// §8.1): the outcome is checked against the ask, the claim made
    /// where the utility signs, and the one request composed and sent to
    /// the resource that asked.  What comes back is that resource's next
    /// answer — a page, or another ask.
    pub fn answer_utility(
        &self,
        id: u64,
        outcome: UtilityOutcome,
    ) -> Result<ResourceAnswer, Refused> {
        crate::diag::call("answer_utility", || {
            let (n, r) = self
                .handle
                .with_blocking(move |c| c.asks.get(&id).map(|p| (p.node, p.resource)))
                .ok_or_else(|| Refused::new("no such ask is waiting"))?;
            let inward = outcome.inward();
            let msg = self
                .handle
                .with_blocking(move |c| c.answer_utility(id, inward))
                .map_err(Refused::new)?;
            let answered = self.carry_to_resource(n, r, msg)?;
            // the application secret may have been minted
            self.save()?;
            Ok(answered)
        })
    }

    /// How far the engagement has got: the nominees reached, of those
    /// nominated.
    #[must_use]
    pub fn nominees_engaged(&self) -> Engaged {
        crate::diag::call_plain("nominees_engaged", || {
            let (engaged, nominated) = self.handle.with_blocking(|c| c.nominees_engaged());
            Engaged {
                engaged: engaged as u32,
                nominated: nominated as u32,
            }
        })
    }

    /// The engagement this side sends its nominees (kind 20): the handle
    /// minted at `begin`, the claimed start, and the counterparty where a
    /// code has named one.
    pub fn engagement(&self) -> Result<Engaging, Refused> {
        crate::diag::call("engagement", || {
            self.handle.with_blocking(|c| {
                c.engagement()
                    .map(|e| Engaging::of(&e))
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// As nominee: take an engagement (kind 20), so the trace that follows
    /// has somewhere to land and the later request something to close.
    /// False where the claimed start is outside this clock's tolerance.
    pub fn take_engagement(&self, from: Id, engagement: Engaging) -> Result<bool, Refused> {
        crate::diag::call("take_engagement", || {
            let k = keyhash(&from).ok_or_else(|| Refused::new("a party is 32 bytes"))?;
            let e = engagement.inward()?;
            Ok(self.handle.with_blocking(move |c| c.take_engagement(k, &e)))
        })
    }

    /// What this client asks its nominees to witness.
    pub fn witness_ask(&self) -> Result<WitnessAsk, Refused> {
        crate::diag::call("witness_ask", || {
            self.handle.with_blocking(|c| {
                c.witness_request()
                    .map(|r| WitnessAsk::of(&r))
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// Answer an ask put to this client as a nominee: the flags it will
    /// sign under, or nothing where it declines.
    #[must_use]
    pub fn take_witness_ask(&self, ask: WitnessAsk) -> Option<u64> {
        crate::diag::call_plain("take_witness_ask", || {
            let Ok(r) = ask.inward() else { return None };
            self.handle
                .with_blocking(move |c| c.take_witness_request(&r))
        })
    }

    /// The back-pointers this client will put in its own signer entry
    /// (`wire-format.md` §3.1).
    #[must_use]
    pub fn back_pointers(&self) -> Vec<Vec<u8>> {
        crate::diag::call_plain("back_pointers", || {
            self.handle
                .with_blocking(|c| c.back_pointers().iter().map(|t| t.to_vec()).collect())
        })
    }

    /// Propose the record, given the counterparty's responses and the
    /// witnesses who accepted.  The disclosures come back with it: they are
    /// what the record commits to and what a holder may later reveal.
    pub fn propose(
        &self,
        theirs: Vec<Vec<u8>>,
        witnesses: Vec<Witnessing>,
    ) -> Result<Proposal, Refused> {
        crate::diag::call("propose", || {
            let w: Result<Vec<_>, Refused> = witnesses.iter().map(|x| x.inward()).collect();
            let w = w?;
            self.handle.with_blocking(move |c| {
                c.propose(theirs, w)
                    .map(|(p, set)| Proposal {
                        proposed: Proposed::of(&p),
                        revealed: revealed_of(&set),
                    })
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// The body every signer signs over, given each signer's back-pointers
    /// in signer order.
    pub fn body(&self, proposal: Proposed, back: Vec<Vec<Vec<u8>>>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("body", || {
            let p = proposal.inward()?;
            let b = back_inward(&back)?;
            Ok(p.body(&b))
        })
    }

    /// Review a proposal as a participant and sign it, or refuse.
    ///
    /// **The disclosures are reviewed, not taken on trust**: a participant
    /// signing a root it has not seen the values behind would be committing
    /// to a record it cannot read.
    pub fn review_and_sign(
        &self,
        proposal: Proposed,
        set: Vec<Revealed>,
        back: Vec<Vec<Vec<u8>>>,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("review_and_sign", || {
            let p = proposal.inward()?;
            let d = revealed_inward(&set)?;
            let b = back_inward(&back)?;
            self.handle.with_blocking(move |c| {
                c.review_and_sign(&p, &d, &b)
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// Sign a proposal as a witness, which sees no disclosures.
    pub fn witness_sign(
        &self,
        proposal: Proposed,
        back: Vec<Vec<Vec<u8>>>,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("witness_sign", || {
            let p = proposal.inward()?;
            let b = back_inward(&back)?;
            self.handle.with_blocking(move |c| {
                c.witness_sign(&p, &b)
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })
        })
    }

    /// Engage this side's nominees (kind 20) [author, 2026-10-19]: one
    /// message each over the payload path, with the intent, before the
    /// counterparty has accepted anything.  A nominee that cannot be
    /// reached was unavailable and is dropped from the set.
    pub fn converse_engage(&self) -> Result<(), Refused> {
        crate::diag::call("converse_engage", || {
            let msgs = self
                .handle
                .with_blocking(|c| c.converse_engage())
                .map_err(|a| Refused::new(format!("{a:?}")))?;
            // a nominee whose engagement the node refused was not reached,
            // and leaves the set the counterparty will be offered
            let unreached = self.net.carry_noting(msgs)?;
            if !unreached.is_empty() {
                self.handle
                    .with_blocking(move |c| c.engagement_unreached(&unreached));
            }
            self.save()
        })
    }

    /// One event of the optical exchange's shape to this side's nominees
    /// (kind 21), as it happens: the shape once both block counts are
    /// known, then each inbound block as it lands and each outbound as
    /// the counterparty's display acknowledges it.  The confirmation
    /// event is the kernel's own, sent where the ceremony-id fixes.
    pub fn optical_trace(&self, event: TraceStep) -> Result<(), Refused> {
        crate::diag::call("optical_trace", || {
            let ev = match event {
                TraceStep::Shape { mine, theirs } => {
                    rhtn_client::conversation::TraceEvent::Shape { mine, theirs }
                }
                TraceStep::Inbound { index } => {
                    rhtn_client::conversation::TraceEvent::Inbound(index)
                }
                TraceStep::Outbound { index } => {
                    rhtn_client::conversation::TraceEvent::Outbound(index)
                }
            };
            self.converse(move |c| c.trace(ev))
        })
    }

    /// Open the window (`wire-format.md` §7.10.1) [author, 2026-10-19]:
    /// the request, its field 5 closing each engagement, to this side's
    /// surviving nominees, and this signer's back-pointers to everyone.
    /// Refused, naming the prune, until the counterparty's keep has
    /// arrived.
    pub fn converse_open(&self) -> Result<(), Refused> {
        crate::diag::call("converse_open", || self.converse(|c| c.converse_open()))
    }

    /// Select the counterparty's verifiers and put each query to the
    /// counterparty for consent, kind 9 to the subject alone.  Consented,
    /// each goes to its verifier from here and the answer arrives as
    /// [`Event::Answered`].
    pub fn converse_queries(&self) -> Result<(), Refused> {
        crate::diag::call("converse_queries", || {
            self.converse(|c| c.converse_queries())
        })
    }

    /// Hand the proposer the responses gathered, kind 14: the side that
    /// did not begin calls this once its queries are answered, or as many
    /// as will be.  The proposer's call sends nothing.
    pub fn converse_gathered(&self) -> Result<(), Refused> {
        crate::diag::call("converse_gathered", || {
            self.converse(|c| c.converse_gathered())
        })
    }

    /// Propose the body from what arrived and show every signer, kind 16:
    /// the initiator's call, once the counterparty's responses and the
    /// witnesses' answers are in.  Refused, naming what is awaited, while
    /// they are not; the signatures come back and the record goes out to
    /// every signer from here.
    pub fn converse_propose(&self) -> Result<(), Refused> {
        crate::diag::call("converse_propose", || {
            self.converse(|c| c.converse_propose())
        })
    }

    /// Where the conversation stands, or nothing outside a ceremony.
    #[must_use]
    pub fn progress(&self) -> Option<Progress> {
        crate::diag::call_plain("progress", || {
            self.handle
                .with_blocking(|c| c.progress().as_ref().map(Progress::of))
        })
    }

    /// Where this client sits in the subnet `subnet` names (design
    /// §13.8), as a `Locator`, or nothing where it is in no such subnet.
    #[must_use]
    pub fn position_in(&self, subnet: Id) -> Option<Vec<u8>> {
        crate::diag::call_plain("position_in", || {
            let s = keyhash(&subnet)?;
            self.handle.with_blocking(move |c| {
                c.position_in(&s).map(|p| {
                    let mut out = Vec::new();
                    p.emit(&mut out);
                    out
                })
            })
        })
    }

    /// **Where this client acts**, as a `Locator`: its position in the
    /// first subnet it was adopted into, or its own self-anchored one
    /// where it has no patron (`wire-format.md` §2.1).
    #[must_use]
    pub fn position(&self) -> Vec<u8> {
        crate::diag::call_plain("position", || {
            self.handle.with_blocking(|c| {
                let mut out = Vec::new();
                c.acting_position().emit(&mut out);
                out
            })
        })
    }

    /// Every subnet this client holds a position in (`rhtn_model::Subnet`):
    /// what it is called, what this client is called there, what this
    /// client is in it, and where it sits.
    #[must_use]
    pub fn subnets(&self) -> Vec<rhtn_model::Subnet> {
        crate::diag::call_plain("subnets", || {
            self.handle.with_blocking(|c| c.subnets_held())
        })
    }

    /// The subnet this client acts in when nothing names one: the first
    /// it was adopted into, else the one under its own key, else none.
    #[must_use]
    pub fn subnet(&self) -> Option<Id> {
        crate::diag::call_plain("subnet", || {
            self.handle.with_blocking(|c| c.subnet().map(|s| id_of(&s)))
        })
    }

    /// As a patron: the adoption body putting `node` one hop below this
    /// client, in the subnet `subnet` names, or under its own key where
    /// none is named.
    ///
    /// **Which tree is the patron's to say**, because a patron in two
    /// subnets sits at a different path in each and the address it issues
    /// is its path in the one it is adopting into.
    pub fn propose_adoption(
        &self,
        subnet: Option<Id>,
        node: Id,
        presence: Id,
        series: u32,
        node_back: Vec<Vec<u8>>,
        carrying: Option<Id>,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("propose_adoption", || {
            let a = match subnet {
                Some(s) => Some(keyhash(&s).ok_or_else(|| Refused::new("a subnet is 32 bytes"))?),
                None => None,
            };
            let carrying = match carrying {
                Some(s) => Some(keyhash(&s).ok_or_else(|| Refused::new("a scope is 32 bytes"))?),
                None => None,
            };
            let n = keyhash(&node).ok_or_else(|| Refused::new("a subordinate is 32 bytes"))?;
            let p = keyhash(&presence)
                .ok_or_else(|| Refused::new("a presence record is named by a 32-byte txid"))?;
            let back = back_inward(&[node_back])?.remove(0);
            self.handle.with_blocking(move |c| {
                c.propose_adoption_in(
                    a,
                    n,
                    &back,
                    rhtn_client::ceremony::Adopting {
                        evidence: rhtn_archive::tx::Evidence::Presence(p),
                        series,
                        presented_head: None,
                        key_material: None,
                        carrying,
                    },
                )
                .map_err(|e| Refused::new(format!("{e:?}")))
            })
        })
    }

    /// The subnets this client roots and could bring under a patron.
    #[must_use]
    pub fn subnets_rooted(&self) -> Vec<rhtn_model::Subnet> {
        crate::diag::call_plain("subnets_rooted", || {
            self.handle.with_blocking(|c| c.subnets_rooted_held())
        })
    }

    /// **As the party being adopted: check the body, then sign it**
    /// (`light-client-requirements.md` §7). The patron built it and this
    /// device is shown it, so every field is checked against what this
    /// client holds — the node is me, the patron is somebody I met, the
    /// evidence is that meeting, the address is one hop below the
    /// patron's own, the series is one I have never occupied and the
    /// back-pointers are my archive's. A refusal carries its reason, for
    /// the person to read and the patron to be told.
    ///
    /// [`Client::sign_body`] is the unchecked one, for a body this device
    /// built itself.
    pub fn review_and_sign_adoption(&self, body: Vec<u8>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("review_and_sign_adoption", || {
            let entries = self
                .handle
                .with_blocking(move |c| c.review_and_sign_adoption(&body))
                .map_err(|e| Refused::new(rhtn_client::diag::abort(&e)))?;
            // the signature is this key's act and the store holds what it
            // was over: a restart between signing and the record arriving
            // must not lose the series this key has now entered
            self.save()?;
            Ok(entries)
        })
    }

    /// **A series this key has never occupied** (`light-client-requirements.md`
    /// §2), the lowest of `candidates` that is free, or nothing where
    /// every one of them is taken: what a party being adopted tells its
    /// patron to propose, the patron having no way to know.
    #[must_use]
    pub fn fresh_series(&self, candidates: Vec<u32>) -> Option<u32> {
        crate::diag::call_plain("fresh_series", || {
            self.handle
                .with_blocking(move |c| rhtn_client::rotation::fresh_series(&c.archive, candidates))
        })
    }

    /// Sign a body this client proposed or was shown, as its subject or
    /// its patron.
    pub fn sign_body(&self, body: Vec<u8>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("sign_body", || {
            self.handle
                .with_blocking(move |c| c.sign_body(&body))
                .map_err(|e| Refused::new(format!("{e:?}")))
        })
    }

    /// **An endpoint record for an instance of this operator's**
    /// (`infra-client-requirements.md` §4.4): the signature is the
    /// operator's, because the instance holds no seed to make its own, so
    /// a change of address is a thing this device signs and hands over.
    /// §8.3 has the client ship the pages that do it.
    /// **One record per patron relationship** (`wire-format.md` §7.6):
    /// the `subnet` names which line the instance serves, and the
    /// record carries that line's series. `None` is the subnet under
    /// this client's own key. Refused where it holds no position there.
    pub fn endpoint_record(
        &self,
        subnet: Option<Id>,
        points: Vec<Vec<u8>>,
        counter: u32,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("endpoint_record", || {
            let s = subnet_of(subnet)?;
            self.handle
                .with_blocking(move |c| c.endpoint_record_in(s, &points, counter))
                .map_err(|e| Refused::new(format!("{e:?}")))
        })
    }

    /// **The anchor entry enrolment always signs** [author, 2026-10-25]:
    /// for the same instance, on the same line, with the subtree a fresh
    /// instance has beneath it (`rhtn_client::FRESH_SUBTREE`).
    /// The size is the kernel's to state, not the operator's to type.
    pub fn anchor_entry_fresh(
        &self,
        subnet: Option<Id>,
        points: Vec<Vec<u8>>,
        counter: u32,
    ) -> Result<Vec<u8>, Refused> {
        self.anchor_entry(subnet, points, rhtn_client::FRESH_SUBTREE, counter)
    }

    /// **An anchor entry for the same instance**, claiming a subtree of
    /// `subtree_size`, signed by the same operator for the same reason,
    /// on the same line (`wire-format.md` §7.6).
    pub fn anchor_entry(
        &self,
        subnet: Option<Id>,
        points: Vec<Vec<u8>>,
        subtree_size: u64,
        counter: u32,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("anchor_entry", || {
            let s = subnet_of(subnet)?;
            self.handle
                .with_blocking(move |c| c.anchor_entry_in(s, &points, subtree_size, counter))
                .map_err(|e| Refused::new(format!("{e:?}")))
        })
    }

    /// **As either party: the peering body between this client and
    /// `other`** (`wire-format.md` §4.4; design §6.3). A light client
    /// peers by locator — its own position in its field, the position it
    /// holds for the other in theirs — and the peering rests on a meeting
    /// of the two, unconditionally. `other_back` is the other party's own
    /// back-pointers, which only they can supply.
    pub fn propose_peering(
        &self,
        other: Id,
        presence: Id,
        other_back: Vec<Vec<u8>>,
        commitment: Option<u64>,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("propose_peering", || {
            let o = keyhash(&other).ok_or_else(|| Refused::new("a peer is 32 bytes"))?;
            let p = keyhash(&presence)
                .ok_or_else(|| Refused::new("a presence record is named by a 32-byte txid"))?;
            let back = back_inward(&[other_back])?.remove(0);
            self.handle
                .with_blocking(move |c| c.propose_peering(o, p, &back, commitment))
                .map_err(|e| Refused::new(rhtn_client::diag::abort(&e)))
        })
    }

    /// **As the other party: check the peering body, then sign it.** The
    /// record is between this client and somebody else, the meeting it
    /// rests on is one of the pair held here, the position written for
    /// this key is the one it holds, its back-pointers are this archive's
    /// next and the timestamp is one this clock can hold.
    pub fn review_and_sign_peering(&self, body: Vec<u8>) -> Result<Vec<u8>, Refused> {
        crate::diag::call("review_and_sign_peering", || {
            let entries = self
                .handle
                .with_blocking(move |c| c.review_and_sign_peering(&body))
                .map_err(|e| Refused::new(rhtn_client::diag::abort(&e)))?;
            self.save()?;
            Ok(entries)
        })
    }

    /// Take a finalised peering, which this client holds and hands to its
    /// serving node to flood. A peering is no part of a position, so
    /// nothing about where this client sits changes.
    pub fn take_peering(&self, envelope: Vec<u8>) -> Result<Id, Refused> {
        crate::diag::call("take_peering", || {
            let t = self
                .handle
                .with_blocking(move |c| c.take_peering(&envelope))
                .map_err(|e| Refused::new(rhtn_client::diag::abort(&e)))?;
            self.save()?;
            Ok(id_of(&t))
        })
    }

    /// Take a finalized adoption this client signed.  Where it is an
    /// adoption of this client, it is also what tells it where it now
    /// sits.
    pub fn take_adoption(&self, envelope: Vec<u8>) -> Result<Id, Refused> {
        crate::diag::call("take_adoption", || {
            let t = self.handle.with_blocking(move |c| {
                c.take_adoption(&envelope)
                    .map(|t| t.to_vec())
                    .map_err(|e| Refused::new(format!("{e:?}")))
            })?;
            self.net.carry_outbox(&self.handle)?;
            self.save()?;
            Ok(t)
        })
    }

    /// Take the finished record.  A participant is given the disclosures
    /// with it; a witness is not, and holds the record without them.
    pub fn finalize(&self, envelope: Vec<u8>, set: Option<Vec<Revealed>>) -> Result<Id, Refused> {
        crate::diag::call("finalize", || {
            let d = match set {
                None => None,
                Some(s) => Some(revealed_inward(&s)?),
            };
            let t = self.handle.with_blocking(move |c| {
                c.finalize(&envelope, d.as_ref())
                    .map(|t| t.to_vec())
                    .map_err(|a| Refused::new(format!("{a:?}")))
            })?;
            // **the record goes up as soon as it exists.** A ceremony that
            // ended in a record nobody else will ever see did the work and
            // none of the good.
            self.net.carry_outbox(&self.handle)?;
            self.save()?;
            Ok(t)
        })
    }

    // ---- ending an authority (E3) --------------------------------------

    /// The lines of authority this key holds (`rhtn_model::Line`;
    /// `light-client-requirements.md` §6).
    #[must_use]
    pub fn lines(&self) -> Vec<rhtn_model::Line> {
        crate::diag::call_plain("lines", || self.handle.with_blocking(|c| c.lines_held()))
    }

    /// **What leaving `patron` would cost** (`light-client-requirements.md`
    /// §6): the resources this client can reach now and could not after,
    /// the parties that leave its horizon with the line, and the limits
    /// of the answer.
    ///
    /// **Shown before `depart`, and the shell is expected to show it.**
    /// §6 wants the person warned before a change that revokes access to
    /// what they use, and this is the computation that warning is made
    /// of: the difference between the horizon this client has and the one
    /// it would have, over the catalog entries it holds.
    ///
    /// **A floor, which is why `limits` is not optional.** Access is a
    /// resource's own predicate, evaluated by its host against the
    /// asker's reach, and no predicate is held here: a host still in view
    /// may refuse once the line is gone. `limits` also carries the
    /// portions that are stale or were truncated, so a partial list is
    /// shown as partial rather than read as the whole cost.
    ///
    /// Refused where this key holds no relationship with `patron` —
    /// the same refusal `depart` gives, so a cost is never quoted for a
    /// departure that would not happen.
    pub fn cost_of_leaving(&self, patron: Id) -> Result<rhtn_model::Cost, Refused> {
        crate::diag::call("cost_of_leaving", || {
            let p = keyhash(&patron).ok_or_else(|| Refused::new("a patron is 32 bytes"))?;
            self.handle.with_blocking(move |c| {
                c.cost_of_leaving(&p)
                    .map_err(|a| Refused::new(rhtn_client::diag::abort(&a)))
            })
        })
    }

    /// **Leave a patron** (`light-client-requirements.md` §4.2): minted,
    /// signed, appended and posted in one act, because there is nothing to
    /// propose and nobody to ask.
    ///
    /// **Exit is a right rather than a request** (design §6.2): the old
    /// patron does not sign, and no refusal can come back from them. What
    /// can come back is this client's own: no such relationship, or one
    /// already left, which a second departure must not advance a counter
    /// over.
    ///
    /// `reason` is the subject's own code where they give one, carried in
    /// the record and interpreted by nobody here.
    ///
    /// **What it costs is computed separately and before**:
    /// [`Participant::cost_of_leaving`] is §6's warning, and this call
    /// does not repeat it. Nothing here refuses a departure whose cost
    /// was never asked for — the person is being told, not gated.
    pub fn depart(&self, patron: Id, reason: Option<u64>) -> Result<Id, Refused> {
        crate::diag::call("depart", || {
            let p = keyhash(&patron).ok_or_else(|| Refused::new("a patron is 32 bytes"))?;
            let t = self.handle.with_blocking(move |c| {
                c.depart(p, reason)
                    .map(|t| t.to_vec())
                    .map_err(|a| Refused::new(rhtn_client::diag::abort(&a)))
            })?;
            // the departure is this key's own act and is already in the
            // outbox; a restart between signing and flooding must not lose
            // the counter it advanced
            self.net.carry_outbox(&self.handle)?;
            self.save()?;
            Ok(t)
        })
    }

    // ---- standing, computed here and kept nowhere (E2) -----------------

    /// **What this client's own policy makes of `subject`, now**
    /// (design §16.2, `light-client-requirements.md` §4.2).
    ///
    /// **Computed on demand and retained nowhere**, from this client's own
    /// copy of its horizon: the adoptions it holds, the meetings it was
    /// present at, and the determinations its patrons made. Nothing it
    /// sends depends on the answer (design §16.4), and no two clients owe
    /// each other the same one.
    #[must_use]
    pub fn standing(&self, subject: Id) -> f64 {
        crate::diag::call_plain("standing", || match keyhash(&subject) {
            None => 0.0,
            Some(k) => self.handle.with_blocking(move |c| c.standing(&k)),
        })
    }

    /// **One computation over a set** (design §16.2): each candidate's own
    /// standing, which of them the policy admits together, and what the
    /// admitted set can carry at once.
    ///
    /// A set is not the sum of its members — a conserving policy bounds
    /// what they can use jointly — so this is the call a screen makes when
    /// it is about to show several, and `standing` the one for a single
    /// party.
    #[must_use]
    pub fn evaluate(&self, candidates: Vec<Id>) -> Evaluation {
        crate::diag::call_plain("evaluate", || {
            let ks: Vec<Keyhash> = candidates.iter().filter_map(|c| keyhash(c)).collect();
            self.handle.with_blocking(move |c| {
                let e = c.evaluate(&ks);
                Evaluation {
                    individual: e
                        .individual
                        .iter()
                        .map(|(k, v)| Standing {
                            who: id_of(k),
                            score: *v,
                        })
                        .collect(),
                    admitted: e.admitted.iter().map(id_of).collect(),
                    joint: e.joint,
                }
            })
        })
    }

    // ---- recovery (A1's third door, E4) --------------------------------

    /// **The prior key this device is recovering from**, where it is one.
    ///
    /// Nothing else here answers until this does: every call below is
    /// about a key that is being succeeded, and a device holding no prior
    /// key is an ordinary device.
    #[must_use]
    pub fn prior_key(&self) -> Option<Id> {
        crate::diag::call_plain("prior_key", || {
            self.handle
                .with_blocking(|c| c.prior_key().map(|k| id_of(&k)))
        })
    }

    /// **As a recovery verifier, the query about the person in front of
    /// me** (design §9.1), under this meeting's pre-commitment and
    /// addressed to myself.
    ///
    /// **Only where my own records show I met the prior key claimed.** A
    /// verifier who never met them has nothing to recognise and is
    /// refused here rather than asked a question they cannot answer.
    pub fn recovery_query(&self, prior: Id) -> Result<Vec<u8>, Refused> {
        crate::diag::call("recovery_query", || {
            let k = keyhash(&prior).ok_or_else(|| Refused::new("a prior key is 32 bytes"))?;
            self.handle.with_blocking(move |c| {
                c.recovery_query(k)
                    .map(|q| q.encode())
                    .map_err(|a| Refused::new(rhtn_client::diag::abort(&a)))
            })
        })
    }

    /// **As a recovery verifier: the one question of the meeting**
    /// (design §9.1), and on yes the hybrid response naming the prior key.
    ///
    /// **The recognition is the person's**, not the device's: this asks
    /// through the platform's own operator and refuses on anything but
    /// yes. The response says *personal knowledge* and *met*, which is
    /// what a recovery verifier has and all they have.
    pub fn recognise(
        &self,
        query: Vec<u8>,
        consent: Vec<u8>,
        prior: Id,
    ) -> Result<Vec<u8>, Refused> {
        crate::diag::call("recognise", || {
            let k = keyhash(&prior).ok_or_else(|| Refused::new("a prior key is 32 bytes"))?;
            self.handle.with_blocking(move |c| {
                let q =
                    rhtn_client::query::VerificationQuery::decode(&query).map_err(Refused::new)?;
                c.recognise(&q, &consent, k)
                    .map_err(|a| Refused::new(rhtn_client::diag::abort(&a)))
            })
        })
    }

    /// **As the recovering subject: take a verifier's answer**, verified
    /// hybrid under that verifier and held.
    ///
    /// Refused where it is not about me, where it answers a query I did
    /// not consent to, or where it names another prior key: a recovery
    /// gathers recognitions of *one* predecessor, and a response about
    /// another is not evidence of this one.
    pub fn take_recovery_response(&self, bytes: Vec<u8>) -> Result<(), Refused> {
        crate::diag::call("take_recovery_response", || {
            self.handle
                .with_blocking(move |c| c.take_recovery_response(&bytes).map_err(Refused::new))
        })
    }

    /// **As the recovering subject: the block the adoption carries**
    /// (design §9.1) — the old key's successor statement over prior, new
    /// and patron, with the recognitions gathered — and the old archive's
    /// head where there is one.
    ///
    /// **Refused until somebody has recognised them.** A recovery with no
    /// match in it is a claim about a person that nobody made.
    pub fn recovery_block(&self, patron: Id) -> Result<RecoveryBlock, Refused> {
        crate::diag::call("recovery_block", || {
            let p = keyhash(&patron).ok_or_else(|| Refused::new("a patron is 32 bytes"))?;
            self.handle.with_blocking(move |c| {
                c.recovery_block(&p)
                    .map(|(block, head)| RecoveryBlock {
                        block,
                        old_head: head.map(|t| t.to_vec()),
                    })
                    .map_err(|a| Refused::new(rhtn_client::diag::abort(&a)))
            })
        })
    }

    /// **Seal every old line as the old key's last act** (design §9.0):
    /// one self-signed locator per relationship, at the counter the series
    /// reached.
    ///
    /// **After this the old key is gone from this client**, which is why
    /// it is the last thing it does. Empty where there is no prior key to
    /// seal with.
    #[must_use]
    pub fn seal_old_lines(&self) -> Vec<SealedLine> {
        crate::diag::call_plain("seal_old_lines", || {
            self.handle.with_blocking(|c| {
                c.seal_old_lines()
                    .into_iter()
                    .map(|(patron, seal)| SealedLine {
                        patron: id_of(&patron),
                        seal,
                    })
                    .collect()
            })
        })
    }

    /// **The chain proving my series under `patron`** (`wire-format.md`
    /// §4.6.1), presented when it is asked for and propagated to nobody.
    #[must_use]
    pub fn present_chain(&self, patron: Id) -> Vec<Vec<u8>> {
        crate::diag::call_plain("present_chain", || match keyhash(&patron) {
            None => Vec::new(),
            Some(p) => self.handle.with_blocking(move |c| c.present_chain(&p)),
        })
    }
}

/// One candidate's standing under this client's own policy.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Standing {
    /// Who it is about.
    pub who: Id,
    /// What the policy makes of them, on its own scale.
    pub score: f64,
}

/// What one computation over a set came to (design §16.2).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct Evaluation {
    /// Each candidate's own standing, in the order considered.
    pub individual: Vec<Standing>,
    /// The candidates the policy admits together, in its own order.
    pub admitted: Vec<Id>,
    /// What the admitted set can carry at once, which a conserving policy
    /// bounds below the sum of its members.
    pub joint: f64,
}

/// The block a recovery adoption carries, and the head of the archive the
/// prior key left behind.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct RecoveryBlock {
    /// The successor statement and the recognitions gathered.
    pub block: Vec<u8>,
    /// The old archive's head, where the prior key had one.
    pub old_head: Option<Id>,
}

/// One line sealed as the old key's last act.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SealedLine {
    /// The patron whose line this seals.
    pub patron: Id,
    /// The self-signed locator that closes it.
    pub seal: Vec<u8>,
}

/// The envelope a presence record travels in, from the body every signer
/// signed and each signer's entry (`wire-format.md` §3.2).  **A signer
/// that does not name a keyhash is refused, not dropped**: an envelope
/// quietly built without one would be refused by every archive it reached
/// as a record short a signature, with nothing to say which.
#[uniffi::export]
pub fn presence_envelope(body: Vec<u8>, entries: Vec<SignedEntries>) -> Result<Vec<u8>, Refused> {
    let e = signer_entries(entries)?;
    Ok(rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_PRESENCE,
        &body,
        &e,
    ))
}

/// The envelope a peering travels in, over its body and the two peers'
/// entries.
#[uniffi::export]
pub fn peering_envelope(body: Vec<u8>, entries: Vec<SignedEntries>) -> Result<Vec<u8>, Refused> {
    let e = signer_entries(entries)?;
    Ok(rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_PEERING,
        &body,
        &e,
    ))
}

/// The envelope an adoption travels in.
#[uniffi::export]
pub fn adoption_envelope(body: Vec<u8>, entries: Vec<SignedEntries>) -> Result<Vec<u8>, Refused> {
    let e = signer_entries(entries)?;
    Ok(rhtn_archive::tx::envelope_from_entries(
        rhtn_archive::tx::TYPE_ADOPTION,
        &body,
        &e,
    ))
}

/// A subnet id from the boundary, where one was named.
fn subnet_of(subnet: Option<Id>) -> Result<Option<Keyhash>, Refused> {
    match subnet {
        Some(s) => Ok(Some(
            keyhash(&s).ok_or_else(|| Refused::new("a subnet is 32 bytes"))?,
        )),
        None => Ok(None),
    }
}

fn signer_entries(entries: Vec<SignedEntries>) -> Result<Vec<(Keyhash, Vec<u8>)>, Refused> {
    entries
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            keyhash(&s.signer)
                .map(|k| (k, s.entries))
                .ok_or_else(|| Refused::new(format!("signer {i} is not a 32-byte keyhash")))
        })
        .collect()
}

fn basis_of(b: u32) -> Result<rhtn_client::selection::SelectionBasis, Refused> {
    match b {
        0 => Ok(rhtn_client::selection::SelectionBasis::Met),
        1 => Ok(rhtn_client::selection::SelectionBasis::InHorizon),
        2 => Ok(rhtn_client::selection::SelectionBasis::Reachable),
        3 => Ok(rhtn_client::selection::SelectionBasis::Discretionary),
        _ => Err(Refused::new(format!(
            "{b} is not a selection basis (`wire-format.md` §5.5 fixes 0 to 3)"
        ))),
    }
}

#[uniffi::export]
impl Platform {
    /// The platform's nine objects, handed over once — the ninth,
    /// diagnostics, hears nothing from a releasable build — and the
    /// engine's two models with the camera's frame geometry; both models
    /// empty is the reference stand-in (design §22.2).
    #[uniffi::constructor]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        camera: Arc<dyn crate::device::Camera>,
        clock: Arc<dyn crate::device::Clock>,
        random: Arc<dyn crate::device::Random>,
        operator: Arc<dyn crate::device::Operator>,
        notices: Arc<dyn crate::device::Notices>,
        storage: Arc<dyn crate::device::Storage>,
        custody: Arc<dyn crate::device::Custody>,
        diagnostics: Arc<dyn crate::device::Diagnostics>,
        yunet: Vec<u8>,
        sface: Vec<u8>,
        frame_width: u32,
        frame_height: u32,
    ) -> Platform {
        platform(
            camera,
            clock,
            random,
            operator,
            notices,
            storage,
            custody,
            diagnostics,
            yunet,
            sface,
            frame_width,
            frame_height,
        )
    }
}

/// So a shell need not hold `Arc` itself.
#[allow(clippy::too_many_arguments)]
pub fn platform(
    camera: Arc<dyn crate::device::Camera>,
    clock: Arc<dyn crate::device::Clock>,
    random: Arc<dyn crate::device::Random>,
    operator: Arc<dyn crate::device::Operator>,
    notices: Arc<dyn crate::device::Notices>,
    storage: Arc<dyn crate::device::Storage>,
    custody: Arc<dyn crate::device::Custody>,
    diagnostics: Arc<dyn crate::device::Diagnostics>,
    yunet: Vec<u8>,
    sface: Vec<u8>,
    frame_width: u32,
    frame_height: u32,
) -> Platform {
    // **the engine is the shell's to supply and the kernel's to run**:
    // the two models' bytes and the camera's frame geometry; both models
    // absent is the reference stand-in, and a model that does not load
    // is said and then the stand-in, never a half-engine
    let engine = if yunet.is_empty() && sface.is_empty() {
        None
    } else {
        match rhtn_engine::SFace::from_onnx(
            &yunet,
            &sface,
            frame_width as usize,
            frame_height as usize,
            rhtn_engine::Thresholds::default(),
        ) {
            Ok(e) => Some(Arc::new(e)),
            Err(why) => {
                tracing::warn!(target: "platform", %why, "platform.engine.refused");
                None
            }
        }
    };
    Platform {
        camera,
        clock,
        random,
        operator,
        notices,
        storage,
        custody,
        diagnostics,
        engine,
    }
}

/// **Whether an instance's proof is the one its token produces**
/// (`rhtn_crypto::enrolment`): the check a provisioning page runs before
/// anything is signed.
///
/// `infra-client-requirements.md` §8.2 has the operator reach their own
/// instance out of band, and the hazard is substitution — something else
/// answering at that address would offer *its* transport key, and a run
/// signed over it would delegate this identity to a stranger. The token is
/// known to the operator who wrote the configuration and to the instance
/// that read it, and is used as a MAC key and never sent.
///
/// **A page that cannot check this must not sign.** Nothing below this
/// line is reversible: a run is a credential, and the far side keeps it.
#[uniffi::export]
pub fn enrolment_proof_checks(
    token: Vec<u8>,
    nonce: Vec<u8>,
    transport_key: Vec<u8>,
    shown: Vec<u8>,
) -> Result<bool, Refused> {
    let token: [u8; 32] = token
        .as_slice()
        .try_into()
        .map_err(|_| Refused::new("an enrolment token is 32 bytes"))?;
    let key: [u8; 32] = transport_key
        .as_slice()
        .try_into()
        .map_err(|_| Refused::new("a transport key is 32 bytes"))?;
    if nonce.len() != rhtn_crypto::enrolment::NONCE_BYTES {
        return Err(Refused::new(format!(
            "a nonce is {} bytes",
            rhtn_crypto::enrolment::NONCE_BYTES
        )));
    }
    Ok(rhtn_crypto::enrolment::checks(&token, &nonce, &key, &shown))
}

/// **A `NetworkPoint` for an instance at this address** (`wire-format.md`
/// §4.4), encoded, for the two records below.
///
/// IPv4 only, said rather than guessed at: the point encoding carries four
/// octets, and a provisioning page handed a v6 address should say so to
/// its operator rather than quietly name a different host.
#[uniffi::export]
pub fn network_point(host: String, port: u16) -> Result<Vec<u8>, Refused> {
    let v4: std::net::Ipv4Addr = host
        .parse()
        .map_err(|_| Refused::new(format!("`{host}` is not an IPv4 address")))?;
    let point = rhtn_transport::session::NetworkPoint::new(v4.octets(), Some(u64::from(port)));
    let mut out = Vec::new();
    point.encode(&mut out);
    Ok(out)
}

impl Participant {
    /// Carry one request to a resource and read what came back, once.
    fn carry_to_resource(
        &self,
        node: rhtn_model::Keyhash,
        resource: rhtn_model::Keyhash,
        msg: rhtn_client::ceremony::Msg,
    ) -> Result<ResourceAnswer, Refused> {
        let answers = self.net.carry_for_answer(vec![msg])?;
        let reply = answers
            .into_iter()
            .next()
            .ok_or_else(|| Refused::new("the node hosting it answered nothing"))?;
        let (status, body) = self
            .handle
            .with_blocking(move |c| c.take_resource_response(&reply))
            .map_err(Refused::new)?;
        // **the HTTP message is read here.** `wire-format.md` §11.2
        // names the hazard a second parser creates on the node's side
        // of this exchange — two parties disagreeing where one
        // message ends — and a shell parsing what the kernel handed
        // it raw would be that second parser on this side.
        let mut answer = ResourceAnswer {
            status,
            page: None,
            unreadable: None,
            ask: None,
        };
        let Some(bytes) = body else {
            return Ok(answer);
        };
        match rhtn_client::resource::answer(&bytes) {
            Err(e) => answer.unreadable = Some(e.say()),
            // **an ask is read, not shown**: the body is the question,
            // taken by the kernel under a number
            Ok(a) if a.shape == rhtn_client::resource::Shape::Ask => {
                let body = a.body;
                match self
                    .handle
                    .with_blocking(move |c| c.take_utility_ask(node, resource, &body))
                {
                    Ok((id, ask)) => {
                        answer.ask = Some(UtilityAsk {
                            id,
                            utility: ask.utility.name().into(),
                            purpose: ask.purpose,
                            retention: ask.retention,
                            resource: resource.to_vec(),
                            node: node.to_vec(),
                            payload: ask.payload,
                            precision: ask.precision,
                            media: ask.media,
                            channel: ask.channel,
                            seconds: ask.seconds,
                        })
                    }
                    Err(m) => {
                        answer.unreadable =
                            Some(format!("the resource's ask could not be read: {}", m.say()))
                    }
                }
            }
            Ok(a) => {
                answer.page = Some(ResourcePage {
                    http_status: a.http_status as u32,
                    media: a.media,
                    charset: a.charset,
                    shape: match a.shape {
                        rhtn_client::resource::Shape::Page => PageShape::Page,
                        rhtn_client::resource::Shape::Text => PageShape::Text,
                        rhtn_client::resource::Shape::Other | rhtn_client::resource::Shape::Ask => {
                            PageShape::Other
                        }
                    },
                    body: a.body,
                })
            }
        }
        Ok(answer)
    }
}
