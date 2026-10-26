//! **The directive plane, as the node carries it** — a stub of the
//! hosting side of `resource-design/federation-model.md` §2, §5 and §6
//! and of "The directive surface" in
//! `resource-design/application-bundles-and-component-library.md`,
//! built so the application suite can be developed against it before
//! the hosting contract is settled [author, 2026-10-26].
//!
//! **What federation is here.** Instances exchange *envelopes*: records
//! going sideways and downwards, directives going down the patronage
//! hierarchy and compliance receipts coming back up. An instance's
//! relationships — its patron's instance, its subordinates' instances,
//! named kin — mirror its operator's patronage and are the operator's
//! configuration (§6: "topology follows patronage"). The node is the
//! carrier between the instances it hosts and the keeper of what each
//! has been handed; the policy — what a directive means, what to apply,
//! what to report — is the application's (component 2.32), and nothing
//! here reads a directive's body.
//!
//! **What the node offers a resource, and through which door.**
//! `infra-client-requirements.md` §9.2 and PKG-003 fix the package API at
//! request traffic and response: no host binding exposes anything else,
//! and this plane adds none. Everything crosses the one door the gateway
//! already opens, as HTTP exchanges on paths reserved for the node:
//!
//! | Exchange | Direction | Carries |
//! |---|---|---|
//! | `PUT /.rhtn/relationships` | node → resource | the instance's configured relationships, on binding and when they change |
//! | `PUT /.rhtn/envelope` | node → resource | one envelope from a related instance; the response may carry envelopes to send |
//! | `GET /.rhtn/outbox` | node → resource | asked each tick; the response carries envelopes to send |
//!
//! **A request under `/.rhtn/` came from the node.** The gateway refuses
//! a member's request naming such a path before it reaches any backend
//! ([`crate::resources::Gateway`]), and `rhtn-*` headers a member wrote
//! are already stripped (`resource-requirements.md` §3.1), so a resource
//! that sees one of these paths is seeing its host and nothing else.
//! The node marks each delivery with `rhtn-audience` (the resource),
//! `rhtn-origin` (the sending instance, hex) and `rhtn-relation`
//! (`patron`, `subordinate` or `kin:<name>`), the relation being the
//! node's own reading of the operator's configuration.
//!
//! **The envelope's semantics are §5's and are the node's to keep**, per
//! hosted resource: conflict detection before duplicate suppression, a
//! newer revision superseding, an older discarded, a tombstone that no
//! replayed older live copy undoes. A conflict — a different digest at a
//! revision already held — is kept and surfaced to the operator, never
//! resolved. The signature is the origin instance's over the immutable
//! fields; its bytes are carried and the canonical form is fixed here
//! ([`Envelope::signed_bytes`]) so both ends sign and check the same
//! thing, but **this stub does not verify it**: instance keys are the
//! applications' and no key material for them is held here yet.
//!
//! **What is stubbed.** Delivery between two instances *this node hosts*
//! is real. Delivery to an instance on another node is not: an envelope
//! for a related instance this node does not host waits in the outbox,
//! readable from the operator's surface (`GET /node/resource/<hex>/
//! federation`) and deliverable by hand at the other node's surface
//! (`PUT /node/resource/<hex>/envelope`). The transport that replaces
//! the hand is §6's open choice. Relationships are configured in the
//! hosting file; the "established by directive" path is not built. No
//! body is pulled on acceptance: an envelope carries its body, bounded.

use crate::Keyhash;
use crate::http::base64url;
use rhtn_codec::cose::sha256;
use rhtn_codec::encode::{emit_array_head, emit_bstr, emit_tstr, emit_uint};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Every path under this prefix is the node's own: the gateway refuses
/// a member's request naming one.
pub const PATH_PREFIX: &str = "/.rhtn/";
/// Where the node delivers one envelope.
pub const PATH_ENVELOPE: &str = "/.rhtn/envelope";
/// Where the node asks for envelopes to send.
pub const PATH_OUTBOX: &str = "/.rhtn/outbox";
/// Where the node states the instance's relationships.
pub const PATH_RELATIONSHIPS: &str = "/.rhtn/relationships";

/// The schema a directive travels under, as the first applications and
/// this node's tests name it; an application may define others.
pub const SCHEMA_DIRECTIVE: &str = "rhtn/directive@1";
/// The schema a compliance receipt travels under.
pub const SCHEMA_RECEIPT: &str = "rhtn/compliance-receipt@1";

/// The longest record identifier an envelope names.
pub const MAX_RECORD_ID: usize = 128;
/// The longest schema name.
pub const MAX_SCHEMA: usize = 64;
/// The most instances a `path` may list before the envelope is refused
/// as looping.
pub const MAX_PATH: usize = 16;
/// The largest body carried with an envelope.
pub const MAX_BODY: usize = 64 * 1024;
/// The longest signature carried.
pub const MAX_SIGNATURE: usize = 512;
/// The most (origin, record) identities one resource's plane remembers.
pub const MAX_HELD: usize = 4096;
/// The most envelopes waiting for one resource to take them.
pub const MAX_INBOX: usize = 256;
/// The most envelopes waiting to leave one resource.
pub const MAX_OUTBOX: usize = 256;
/// The most conflicts kept for the operator to see, per resource.
pub const MAX_CONFLICTS: usize = 64;
/// How many times a delivery the resource would not take is retried,
/// one per tick, before the envelope is dropped and the drop recorded.
pub const MAX_ATTEMPTS: u32 = 32;
/// The longest a kin group's name may be.
pub const MAX_KIN_NAME: usize = 32;

/// Whether a record is live or withdrawn (§5: a tombstone keeps digest
/// and revision and drops the body).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The record stands.
    Live,
    /// Withdrawn: a tombstone, which no older live copy revives.
    Withdrawn,
}

impl State {
    fn word(self) -> &'static str {
        match self {
            State::Live => "live",
            State::Withdrawn => "withdrawn",
        }
    }
    fn code(self) -> u64 {
        match self {
            State::Live => 0,
            State::Withdrawn => 1,
        }
    }
}

/// **One envelope** (`federation-model.md` §5): the identity of a copy
/// is (`origin`, `record`); the origin signs the immutable fields and
/// never the route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// The originating instance: its resource keyhash.
    pub origin: Keyhash,
    /// The origin's local record identifier.
    pub record: String,
    /// The record type and its version.
    pub schema: String,
    /// A strictly ordered per-record counter from the origin.
    pub revision: u64,
    /// The content hash of the body.
    pub digest: [u8; 32],
    /// Live or withdrawn.
    pub state: State,
    /// The instances this copy has crossed: transport metadata outside
    /// the signature, which a forwarder appends to.
    pub path: Vec<Keyhash>,
    /// The origin instance's signature over [`Envelope::signed_bytes`];
    /// carried, not checked here.
    pub signature: Vec<u8>,
    /// The body, where it travels with the envelope; a tombstone has
    /// none. Where present its hash is `digest`, which the node checks.
    pub body: Option<Vec<u8>>,
}

impl Envelope {
    /// The bytes the origin signs: a CBOR array of the six immutable
    /// fields, `[origin, record, schema, revision, digest, state]`, with
    /// `state` 0 for live and 1 for withdrawn. Fixed here so an origin
    /// and a verifier agree without a second specification.
    pub fn signed_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 6);
        emit_bstr(&mut out, &self.origin);
        emit_tstr(&mut out, &self.record);
        emit_tstr(&mut out, &self.schema);
        emit_uint(&mut out, self.revision);
        emit_bstr(&mut out, &self.digest);
        emit_uint(&mut out, self.state.code());
        out
    }

    /// Why this envelope is not one the node will hold, where it is not:
    /// a bound exceeded, a body whose hash is not its digest, a tombstone
    /// carrying a body.
    pub fn refused(&self) -> Option<String> {
        if self.record.is_empty() || self.record.len() > MAX_RECORD_ID {
            return Some(format!("a record id is 1 to {MAX_RECORD_ID} bytes"));
        }
        if self.schema.is_empty() || self.schema.len() > MAX_SCHEMA {
            return Some(format!("a schema is 1 to {MAX_SCHEMA} bytes"));
        }
        if self.path.len() > MAX_PATH {
            return Some(format!(
                "the path lists {} instances, more than {MAX_PATH}: a copy that has crossed that \
                 many is looping",
                self.path.len()
            ));
        }
        if self.signature.len() > MAX_SIGNATURE {
            return Some(format!("a signature is at most {MAX_SIGNATURE} bytes"));
        }
        match (&self.body, self.state) {
            (Some(b), State::Live) => {
                if b.len() > MAX_BODY {
                    return Some(format!("the body is {} bytes, over {MAX_BODY}", b.len()));
                }
                if sha256(b) != self.digest {
                    return Some("the body does not hash to the digest".into());
                }
            }
            (Some(_), State::Withdrawn) => {
                return Some("a tombstone carries no body".into());
            }
            (None, _) => {}
        }
        None
    }

    /// The envelope as JSON, the form it crosses HTTP in: keyhashes and
    /// the digest as 64 hex digits, the signature and the body base64url
    /// (RFC 4648 §5, no padding), the state a word.
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "origin": hex(&self.origin),
            "record": self.record,
            "schema": self.schema,
            "revision": self.revision,
            "digest": hex(&self.digest),
            "state": self.state.word(),
            "path": self.path.iter().map(|k| hex(k)).collect::<Vec<_>>(),
            "signature": base64url(&self.signature),
        });
        if let Some(b) = &self.body {
            v["body"] = Value::String(base64url(b));
        }
        v
    }

    /// An envelope read back from its JSON, or why it is not one.
    pub fn from_json(v: &Value) -> Result<Envelope, String> {
        let s = |k: &str| -> Result<&str, String> {
            v.get(k)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("`{k}` is missing or not a string"))
        };
        let origin = unhex32(s("origin")?).ok_or("`origin` is not 64 hex digits")?;
        let digest = unhex32(s("digest")?).ok_or("`digest` is not 64 hex digits")?;
        let revision = v
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or("`revision` is missing or not a whole number")?;
        let state = match s("state")? {
            "live" => State::Live,
            "withdrawn" => State::Withdrawn,
            other => return Err(format!("`state` is `live` or `withdrawn`, not `{other}`")),
        };
        let path = match v.get("path") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(a)) => a
                .iter()
                .map(|p| {
                    p.as_str()
                        .and_then(unhex32)
                        .ok_or("`path` holds something that is not 64 hex digits".to_string())
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => return Err("`path` is not a list".into()),
        };
        let signature = match v.get("signature") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::String(b)) => unbase64url(b).ok_or("`signature` is not base64url")?,
            Some(_) => return Err("`signature` is not a string".into()),
        };
        let body = match v.get("body") {
            None | Some(Value::Null) => None,
            Some(Value::String(b)) => Some(unbase64url(b).ok_or("`body` is not base64url")?),
            Some(_) => return Err("`body` is not a string".into()),
        };
        let e = Envelope {
            origin,
            record: s("record")?.to_string(),
            schema: s("schema")?.to_string(),
            revision,
            digest,
            state,
            path,
            signature,
            body,
        };
        if let Some(why) = e.refused() {
            return Err(why);
        }
        Ok(e)
    }

    /// A live envelope over `body` with its digest computed, unsigned:
    /// what a test or an application's first draft builds before signing.
    pub fn live(origin: Keyhash, record: &str, schema: &str, revision: u64, body: &[u8]) -> Self {
        Envelope {
            origin,
            record: record.into(),
            schema: schema.into(),
            revision,
            digest: sha256(body),
            state: State::Live,
            path: Vec::new(),
            signature: Vec::new(),
            body: Some(body.to_vec()),
        }
    }

    /// The tombstone for a record: the digest and revision kept, the
    /// body dropped (§5).
    pub fn withdrawn(
        origin: Keyhash,
        record: &str,
        schema: &str,
        revision: u64,
        digest: [u8; 32],
    ) -> Self {
        Envelope {
            origin,
            record: record.into(),
            schema: schema.into(),
            revision,
            digest,
            state: State::Withdrawn,
            path: Vec::new(),
            signature: Vec::new(),
            body: None,
        }
    }
}

/// What one resource's plane remembers of a record it has been handed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The newest revision taken.
    pub revision: u64,
    /// Its digest.
    pub digest: [u8; 32],
    /// Whether it stands or is withdrawn.
    pub state: State,
}

/// What became of an offered envelope (§5's semantics, in the order §5
/// states them).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A record not held before: taken.
    New,
    /// A newer revision of a held record: taken, superseding.
    Superseded,
    /// The same revision, the same digest: a re-delivery, and a no-op.
    Duplicate,
    /// An older revision than the one held: discarded.
    Older,
    /// A different digest at a revision already held: the origin's
    /// history forked. Kept for the operator, applied to nothing.
    Conflict,
    /// A live copy offered where a tombstone stands: discarded, whatever
    /// its revision says (§5: revision order is checked before state,
    /// and a tombstone's revision is the newest the origin ever issued).
    Tombstoned,
    /// Not an envelope the node will hold, and why.
    Refused(String),
}

/// How one instance is related to another, as the operator configured it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Relation {
    /// The other is this instance's patron's instance.
    Patron,
    /// The other is a subordinate's instance.
    Subordinate,
    /// The other is kin, in the named group.
    Kin(String),
}

impl Relation {
    /// The word the `rhtn-relation` header carries.
    pub fn word(&self) -> String {
        match self {
            Relation::Patron => "patron".into(),
            Relation::Subordinate => "subordinate".into(),
            Relation::Kin(n) => format!("kin:{n}"),
        }
    }
}

/// An instance's configured relationships (§6).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relationships {
    /// Its patron's instance, where its operator has a patron whose
    /// instance it federates with.
    pub patron: Option<Keyhash>,
    /// Its subordinates' instances.
    pub subordinates: BTreeSet<Keyhash>,
    /// Named kin groups: peers with no command relationship.
    pub kin: BTreeMap<String, BTreeSet<Keyhash>>,
}

impl Relationships {
    /// How `other` stands to this instance, or none where it is not a
    /// configured relationship. A patron is answered before a kin group
    /// that also names it; subordinates before kin.
    pub fn relation_of(&self, other: &Keyhash) -> Option<Relation> {
        if self.patron.as_ref() == Some(other) {
            return Some(Relation::Patron);
        }
        if self.subordinates.contains(other) {
            return Some(Relation::Subordinate);
        }
        self.kin
            .iter()
            .find(|(_, members)| members.contains(other))
            .map(|(name, _)| Relation::Kin(name.clone()))
    }

    /// Every instance this one is related to.
    pub fn all(&self) -> BTreeSet<Keyhash> {
        let mut out: BTreeSet<Keyhash> = self.subordinates.clone();
        out.extend(self.patron);
        for m in self.kin.values() {
            out.extend(m.iter().copied());
        }
        out
    }

    /// Why these relationships cannot be configured, where they cannot.
    pub fn refused(&self) -> Option<String> {
        for name in self.kin.keys() {
            if !crate::resources::namelike(name) || name.len() > MAX_KIN_NAME {
                return Some(format!(
                    "`{name}` is not a kin group name: [a-z0-9_-], 1 to {MAX_KIN_NAME} bytes"
                ));
            }
        }
        None
    }

    /// The relationships as JSON, the form `PUT /.rhtn/relationships`
    /// carries.
    pub fn to_json(&self) -> Value {
        json!({
            "patron": self.patron.as_ref().map(|k| hex(k)),
            "subordinates": self.subordinates.iter().map(|k| hex(k)).collect::<Vec<_>>(),
            "kin": self.kin.iter().map(|(n, m)| (n.clone(), m.iter().map(|k| hex(k)).collect::<Vec<_>>())).collect::<BTreeMap<_, _>>(),
        })
    }
}

/// Where an emitted envelope is to go, as the resource names it in its
/// outbox: a relation, or one instance by keyhash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// The patron's instance.
    Patron,
    /// Every subordinate's instance.
    Subordinates,
    /// Every instance in the named kin group.
    Kin(String),
    /// One instance, which must be a configured relationship.
    Instance(Keyhash),
}

impl Destination {
    /// Read from the outbox item's `to`: `patron`, `subordinates`,
    /// `kin:<name>`, or 64 hex digits.
    pub fn parse(s: &str) -> Option<Destination> {
        match s {
            "patron" => Some(Destination::Patron),
            "subordinates" => Some(Destination::Subordinates),
            _ => {
                if let Some(n) = s.strip_prefix("kin:") {
                    return Some(Destination::Kin(n.to_string()));
                }
                unhex32(s).map(Destination::Instance)
            }
        }
    }

    /// The instances this names under `r`, or none where it names
    /// nothing configured.
    pub fn resolve(&self, r: &Relationships) -> BTreeSet<Keyhash> {
        match self {
            Destination::Patron => r.patron.into_iter().collect(),
            Destination::Subordinates => r.subordinates.clone(),
            Destination::Kin(n) => r.kin.get(n).cloned().unwrap_or_default(),
            Destination::Instance(k) => {
                if r.relation_of(k).is_some() {
                    BTreeSet::from([*k])
                } else {
                    BTreeSet::new()
                }
            }
        }
    }

    fn word(&self) -> String {
        match self {
            Destination::Patron => "patron".into(),
            Destination::Subordinates => "subordinates".into(),
            Destination::Kin(n) => format!("kin:{n}"),
            Destination::Instance(k) => hex(k),
        }
    }
}

/// An envelope waiting to be delivered to a resource this node hosts,
/// with the relation the sender stands in and how often the resource
/// has declined it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// The envelope.
    pub envelope: Envelope,
    /// How the origin stands to the receiving instance.
    pub relation: Relation,
    /// Deliveries attempted so far.
    pub attempts: u32,
}

/// An envelope a hosted resource emitted, waiting for a carrier to the
/// instance it names: this node where it hosts that instance, nothing
/// yet where it does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// Where it is to go.
    pub to: Keyhash,
    /// The envelope.
    pub envelope: Envelope,
}

/// **One hosted resource's side of the plane**: what it has been handed,
/// what waits for it, what it has emitted, and the relationships that
/// decide who may hand it anything.
#[derive(Debug, Default)]
pub struct Plane {
    /// The operator's configuration for this instance.
    pub relationships: Relationships,
    /// Whether the resource has been told its relationships since they
    /// were last set.
    pub relationships_told: bool,
    held: BTreeMap<(Keyhash, String), Held>,
    inbox: VecDeque<Delivery>,
    /// Emitted and not yet carried.
    pub pending: Vec<Pending>,
    /// Conflicts seen, newest last, for the operator.
    pub conflicts: Vec<(Envelope, Held)>,
    /// Deliveries dropped after [`MAX_ATTEMPTS`], for the operator.
    pub dropped: Vec<Envelope>,
}

impl Plane {
    /// Set the relationships; the resource is told on the next pass.
    pub fn configure(&mut self, r: Relationships) {
        if self.relationships != r {
            self.relationships = r;
            self.relationships_told = false;
        }
    }

    /// **Offer an envelope to this plane** (§5). Taken envelopes join the
    /// inbox for delivery; what became of it is returned either way.
    /// `relation` is how the origin stands to this instance, which the
    /// caller established from the relationships before offering.
    pub fn offer(&mut self, e: Envelope, relation: Relation) -> Outcome {
        if let Some(why) = e.refused() {
            return Outcome::Refused(why);
        }
        let key = (e.origin, e.record.clone());
        let outcome = match self.held.get(&key) {
            None => {
                if self.held.len() >= MAX_HELD {
                    return Outcome::Refused(format!(
                        "this instance holds {MAX_HELD} records already"
                    ));
                }
                Outcome::New
            }
            Some(h) => {
                // §5: conflict detection precedes duplicate suppression,
                // and revision order is checked before state
                if h.revision == e.revision {
                    if h.digest != e.digest {
                        Outcome::Conflict
                    } else if h.state == e.state {
                        Outcome::Duplicate
                    } else if e.state == State::Withdrawn {
                        // the same revision withdrawn: the tombstone
                        // of what is held, which is an ordinary update
                        Outcome::Superseded
                    } else {
                        Outcome::Tombstoned
                    }
                } else if h.revision > e.revision {
                    if h.state == State::Withdrawn {
                        Outcome::Tombstoned
                    } else {
                        Outcome::Older
                    }
                } else if h.state == State::Withdrawn && e.state == State::Live {
                    // a live copy newer than the tombstone: the origin
                    // revived the record after withdrawing it, which
                    // the tombstone rule does not forbid — a tombstone
                    // beats an *older* live copy, and this one is newer
                    Outcome::Superseded
                } else {
                    Outcome::Superseded
                }
            }
        };
        match outcome {
            Outcome::New | Outcome::Superseded => {
                if self.inbox.len() >= MAX_INBOX {
                    return Outcome::Refused(format!(
                        "{MAX_INBOX} envelopes already wait for this instance"
                    ));
                }
                self.held.insert(
                    key,
                    Held {
                        revision: e.revision,
                        digest: e.digest,
                        state: e.state,
                    },
                );
                self.inbox.push_back(Delivery {
                    envelope: e,
                    relation,
                    attempts: 0,
                });
            }
            Outcome::Conflict => {
                let h = self.held.get(&key).cloned().expect("held");
                if self.conflicts.len() >= MAX_CONFLICTS {
                    self.conflicts.remove(0);
                }
                self.conflicts.push((e, h));
            }
            _ => {}
        }
        outcome
    }

    /// What the plane remembers, for the operator's page.
    pub fn held(&self) -> Vec<(Keyhash, String, Held)> {
        self.held
            .iter()
            .map(|((o, r), h)| (*o, r.clone(), h.clone()))
            .collect()
    }

    /// How many envelopes wait for the resource.
    pub fn waiting(&self) -> usize {
        self.inbox.len()
    }

    /// The next delivery to attempt, where one waits.
    fn next_delivery(&mut self) -> Option<Delivery> {
        self.inbox.pop_front()
    }

    /// Put a declined delivery back, or drop it after [`MAX_ATTEMPTS`].
    fn declined(&mut self, mut d: Delivery) {
        d.attempts += 1;
        if d.attempts >= MAX_ATTEMPTS {
            if self.dropped.len() >= MAX_CONFLICTS {
                self.dropped.remove(0);
            }
            self.dropped.push(d.envelope);
        } else {
            self.inbox.push_back(d);
        }
    }

    /// **Take what a resource emitted**: each item's destination resolved
    /// against this instance's relationships, each envelope checked, and
    /// each (instance, envelope) queued as pending. Items naming nothing
    /// configured, or an origin other than this instance, are refused
    /// with their reasons.
    pub fn emit(&mut self, me: &Keyhash, items: &[(Destination, Envelope)]) -> Vec<String> {
        let mut refused = Vec::new();
        for (to, e) in items {
            if e.origin != *me {
                refused.push(format!(
                    "`{}` rev {}: the origin is not this instance",
                    e.record, e.revision
                ));
                continue;
            }
            if let Some(why) = e.refused() {
                refused.push(format!("`{}` rev {}: {why}", e.record, e.revision));
                continue;
            }
            let targets = to.resolve(&self.relationships);
            if targets.is_empty() {
                refused.push(format!(
                    "`{}` rev {}: `{}` names no configured relationship",
                    e.record,
                    e.revision,
                    to.word()
                ));
                continue;
            }
            for t in targets {
                if self.pending.len() >= MAX_OUTBOX {
                    refused.push(format!(
                        "`{}` rev {}: {MAX_OUTBOX} envelopes already wait to leave",
                        e.record, e.revision
                    ));
                    break;
                }
                self.pending.push(Pending {
                    to: t,
                    envelope: e.clone(),
                });
            }
        }
        refused
    }
}

/// **The planes of every resource this node hosts**, keyed by resource,
/// and the pass that moves envelopes between them and their resources.
#[derive(Debug, Default)]
pub struct Federation {
    planes: BTreeMap<Keyhash, Plane>,
}

/// What one pass did, for a log line and a test.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Passed {
    /// Envelopes a resource took.
    pub delivered: usize,
    /// Envelopes a resource declined or could not be reached for.
    pub declined: usize,
    /// Envelopes resources emitted.
    pub emitted: usize,
    /// Envelopes carried to another resource this node hosts.
    pub carried: usize,
    /// Reasons the node refused what a resource emitted.
    pub refused: Vec<String>,
}

/// What the pass needs of the gateway: a way to hand a resource one
/// message and read its answer. The gateway's backends are exactly that.
pub trait Carrier {
    /// Hand `message` to `resource`, where this node runs or relays to
    /// it; `None` where it is brokered or not bound, `Some(Err)` where
    /// the backend did not answer.
    fn hand(&self, resource: &Keyhash, message: &[u8]) -> Option<Result<Vec<u8>, String>>;
    /// The authority a resource's backend is addressed by.
    fn authority(&self, resource: &Keyhash) -> Option<String>;
}

impl Federation {
    /// Configure (or reconfigure) a resource's relationships, creating
    /// its plane.
    pub fn configure(&mut self, resource: Keyhash, r: Relationships) -> Result<(), String> {
        if let Some(why) = r.refused() {
            return Err(why);
        }
        self.planes.entry(resource).or_default().configure(r);
        Ok(())
    }

    /// Forget a resource that is no longer hosted.
    pub fn forget(&mut self, resource: &Keyhash) {
        self.planes.remove(resource);
    }

    /// The plane of a resource, where one is configured.
    pub fn plane(&self, resource: &Keyhash) -> Option<&Plane> {
        self.planes.get(resource)
    }

    /// Every resource with a plane.
    pub fn resources(&self) -> Vec<Keyhash> {
        self.planes.keys().copied().collect()
    }

    /// **Offer an envelope to a hosted resource from outside this node**:
    /// the operator's surface, standing in for the transport that does
    /// not exist yet. The origin must be a configured relationship of the
    /// receiving instance, or nothing is taken.
    pub fn deliver(&mut self, resource: &Keyhash, e: Envelope) -> Result<Outcome, String> {
        let plane = self
            .planes
            .get_mut(resource)
            .ok_or("this node hosts no such instance, or it has no relationships configured")?;
        let relation = plane.relationships.relation_of(&e.origin).ok_or_else(|| {
            format!(
                "{} is not a configured relationship of this instance",
                hex(&e.origin)
            )
        })?;
        Ok(plane.offer(e, relation))
    }

    /// **One pass over every plane**: tell each resource its
    /// relationships where it has not been told, deliver what waits,
    /// take what each emits, and carry what is addressed to an instance
    /// this node hosts into that instance's plane. What is addressed
    /// elsewhere stays pending.
    pub fn pass(&mut self, carrier: &dyn Carrier) -> Passed {
        let mut out = Passed::default();
        let resources = self.resources();
        for res in &resources {
            let Some(authority) = carrier.authority(res) else {
                continue;
            };
            // relationships first, so a resource reads a delivery
            // knowing who the origin is to it
            let told = self.planes.get(res).is_some_and(|p| p.relationships_told);
            if !told {
                let body = self.planes[res].relationships.to_json().to_string();
                let msg = message(
                    "PUT",
                    PATH_RELATIONSHIPS,
                    &authority,
                    res,
                    &[],
                    body.as_bytes(),
                );
                if let Some(Ok(resp)) = carrier.hand(res, &msg)
                    && (200..300).contains(&response_of(&resp).0)
                {
                    self.planes.get_mut(res).unwrap().relationships_told = true;
                }
            }
            // deliveries: each once per pass, declined ones back on the
            // queue for the next
            let waiting = self.planes[res].waiting();
            for _ in 0..waiting {
                let Some(d) = self.planes.get_mut(res).unwrap().next_delivery() else {
                    break;
                };
                let body = d.envelope.to_json().to_string();
                let extra = [
                    ("rhtn-origin", hex(&d.envelope.origin)),
                    ("rhtn-relation", d.relation.word()),
                ];
                let msg = message(
                    "PUT",
                    PATH_ENVELOPE,
                    &authority,
                    res,
                    &extra,
                    body.as_bytes(),
                );
                match carrier.hand(res, &msg) {
                    Some(Ok(resp)) => {
                        let (status, answer) = response_of(&resp);
                        if (200..300).contains(&status) {
                            out.delivered += 1;
                            self.take_emitted(res, &answer, &mut out);
                        } else {
                            out.declined += 1;
                            self.planes.get_mut(res).unwrap().declined(d);
                        }
                    }
                    _ => {
                        out.declined += 1;
                        self.planes.get_mut(res).unwrap().declined(d);
                    }
                }
            }
            // the outbox, asked for
            let msg = message("GET", PATH_OUTBOX, &authority, res, &[], b"");
            if let Some(Ok(resp)) = carrier.hand(res, &msg) {
                let (status, answer) = response_of(&resp);
                if (200..300).contains(&status) {
                    self.take_emitted(res, &answer, &mut out);
                }
            }
        }
        // carry what is pending to the instances this node hosts
        for res in &resources {
            let pending = std::mem::take(&mut self.planes.get_mut(res).unwrap().pending);
            let mut keep = Vec::new();
            for p in pending {
                let Some(target) = self.planes.get_mut(&p.to) else {
                    keep.push(p);
                    continue;
                };
                let Some(relation) = target.relationships.relation_of(res) else {
                    // the sender names a relationship the receiver's
                    // operator did not configure: §2's rule is that an
                    // obligation binds only within its own relationship,
                    // so this is kept, visible, and not forced through
                    keep.push(p);
                    continue;
                };
                let mut e = p.envelope;
                e.path.push(*res);
                match target.offer(e, relation) {
                    Outcome::Refused(why) => out.refused.push(why),
                    _ => out.carried += 1,
                }
            }
            self.planes.get_mut(res).unwrap().pending = keep;
        }
        out
    }

    /// Read `{"emit": [{"to": ..., "envelope": {...}}, ...]}` from a
    /// resource's answer and queue it; an answer that is not that shape
    /// emits nothing, which is the common case for a delivery's `200`.
    fn take_emitted(&mut self, res: &Keyhash, answer: &[u8], out: &mut Passed) {
        let Ok(v) = serde_json::from_slice::<Value>(answer) else {
            return;
        };
        let Some(items) = v.get("emit").and_then(Value::as_array) else {
            return;
        };
        let mut parsed = Vec::new();
        for item in items {
            let to = item
                .get("to")
                .and_then(Value::as_str)
                .and_then(Destination::parse);
            let env = item.get("envelope").map(Envelope::from_json);
            match (to, env) {
                (Some(to), Some(Ok(e))) => parsed.push((to, e)),
                (None, _) => out
                    .refused
                    .push("an emitted item names no destination".into()),
                (_, Some(Err(why))) => out.refused.push(why),
                (_, None) => out.refused.push("an emitted item has no envelope".into()),
            }
        }
        out.emitted += parsed.len();
        let refused = self.planes.get_mut(res).unwrap().emit(res, &parsed);
        out.refused.extend(refused);
    }
}

/// One HTTP/1.1 request the node composes for a resource on a reserved
/// path: the authority the operator bound as `host`, `rhtn-audience`, the
/// headers given, a JSON body.
pub fn message(
    method: &str,
    path: &str,
    authority: &str,
    resource: &Keyhash,
    extra: &[(&str, String)],
    body: &[u8],
) -> Vec<u8> {
    let mut out = format!("{method} {path} HTTP/1.1\r\nhost: {authority}\r\n").into_bytes();
    out.extend_from_slice(format!("rhtn-audience: {}\r\n", base64url(resource)).as_bytes());
    for (n, v) in extra {
        out.extend_from_slice(format!("{n}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(b"content-type: application/json\r\n");
    out.extend_from_slice(format!("content-length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend_from_slice(body);
    out
}

/// The status and body of a resource's HTTP answer, read leniently: a
/// backend's framing is the relay's or the sandbox's to have checked.
pub fn response_of(bytes: &[u8]) -> (u16, Vec<u8>) {
    let head_end = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|p| p + 4)
        .unwrap_or(bytes.len());
    let head = String::from_utf8_lossy(&bytes[..head_end]);
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split(' ').nth(1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    (status, bytes[head_end..].to_vec())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex32(s: &str) -> Option<Keyhash> {
    if s.len() != 64 || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// RFC 4648 §5 without padding, decoded; `None` for anything else.
pub fn unbase64url(s: &str) -> Option<Vec<u8>> {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    if s.is_empty() {
        return Some(Vec::new());
    }
    if s.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0;
    for c in s.bytes() {
        let v = A.iter().position(|a| *a == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(n: u8) -> Keyhash {
        [n; 32]
    }

    #[test]
    fn the_envelope_crosses_json_whole_and_signs_the_six_immutable_fields_only() {
        let mut e = Envelope::live(k(1), "notice-7", SCHEMA_DIRECTIVE, 3, b"{\"retain\":90}");
        e.path = vec![k(9)];
        e.signature = vec![1, 2, 3];
        let back = Envelope::from_json(&e.to_json()).unwrap();
        assert_eq!(back, e);
        let mut routed = e.clone();
        routed.path.push(k(8));
        routed.signature = vec![];
        assert_eq!(
            routed.signed_bytes(),
            e.signed_bytes(),
            "the route and the signature are outside what is signed"
        );
        let mut other = e.clone();
        other.revision = 4;
        assert_ne!(other.signed_bytes(), e.signed_bytes());
    }

    #[test]
    fn a_body_that_is_not_its_digest_and_a_tombstone_with_a_body_are_refused() {
        let mut e = Envelope::live(k(1), "r", "s@1", 1, b"x");
        e.digest = [0; 32];
        assert!(e.refused().unwrap().contains("digest"));
        let mut t = Envelope::withdrawn(k(1), "r", "s@1", 2, [0; 32]);
        t.body = Some(b"x".to_vec());
        assert!(t.refused().unwrap().contains("tombstone"));
        assert!(Envelope::from_json(&e.to_json()).is_err());
    }

    #[test]
    fn section_five_semantics_in_order() {
        let mut p = Plane::default();
        let r = Relation::Patron;
        let v1 = Envelope::live(k(1), "r", "s@1", 1, b"one");
        let v2 = Envelope::live(k(1), "r", "s@1", 2, b"two");
        assert_eq!(p.offer(v1.clone(), r.clone()), Outcome::New);
        assert_eq!(p.offer(v1.clone(), r.clone()), Outcome::Duplicate);
        assert_eq!(p.offer(v2.clone(), r.clone()), Outcome::Superseded);
        assert_eq!(p.offer(v1.clone(), r.clone()), Outcome::Older);
        // a different digest at a held revision: a conflict, kept
        let forked = Envelope::live(k(1), "r", "s@1", 2, b"TWO");
        assert_eq!(p.offer(forked.clone(), r.clone()), Outcome::Conflict);
        assert_eq!(p.conflicts.len(), 1);
        assert_eq!(
            p.held()[0].2.digest,
            v2.digest,
            "the conflict changed nothing"
        );
        // withdrawal, and a replayed older live copy stays withdrawn
        let gone = Envelope::withdrawn(k(1), "r", "s@1", 3, v2.digest);
        assert_eq!(p.offer(gone, r.clone()), Outcome::Superseded);
        assert_eq!(p.offer(v2.clone(), r.clone()), Outcome::Tombstoned);
        assert_eq!(p.offer(v1, r.clone()), Outcome::Tombstoned);
        assert_eq!(p.held()[0].2.state, State::Withdrawn);
        // three taken, so three wait for the resource
        assert_eq!(p.waiting(), 3);
    }

    #[test]
    fn relationships_decide_who_may_deliver_and_where_an_emission_goes() {
        let r = Relationships {
            patron: Some(k(1)),
            subordinates: BTreeSet::from([k(2)]),
            kin: BTreeMap::from([("guild".to_string(), BTreeSet::from([k(3)]))]),
        };
        assert_eq!(r.relation_of(&k(1)), Some(Relation::Patron));
        assert_eq!(r.relation_of(&k(2)), Some(Relation::Subordinate));
        assert_eq!(r.relation_of(&k(3)), Some(Relation::Kin("guild".into())));
        assert_eq!(r.relation_of(&k(4)), None);
        assert_eq!(
            Destination::parse("patron").unwrap().resolve(&r),
            BTreeSet::from([k(1)])
        );
        assert_eq!(
            Destination::parse("kin:guild").unwrap().resolve(&r),
            BTreeSet::from([k(3)])
        );
        assert!(
            Destination::parse(&hex(&k(4)))
                .unwrap()
                .resolve(&r)
                .is_empty()
        );

        let mut f = Federation::default();
        f.configure(k(5), r).unwrap();
        let from_stranger = Envelope::live(k(4), "r", "s@1", 1, b"x");
        assert!(f.deliver(&k(5), from_stranger).is_err());
        let from_patron = Envelope::live(k(1), "r", "s@1", 1, b"x");
        assert_eq!(f.deliver(&k(5), from_patron).unwrap(), Outcome::New);
        let mut bad_kin = Relationships::default();
        bad_kin.kin.insert("Not A Name".into(), BTreeSet::new());
        assert!(f.configure(k(6), bad_kin).is_err());
    }

    /// Two instances on one node, each an HTTP origin that records what
    /// it was handed: the patron's outbox emits a directive to its
    /// subordinates, the pass carries it, the subordinate's answer to the
    /// delivery emits a receipt to its patron, and the next pass carries
    /// that back.
    #[test]
    fn a_directive_goes_down_and_its_receipt_comes_back_on_one_node() {
        use std::cell::RefCell;
        struct Fake {
            handed: RefCell<Vec<(Keyhash, String)>>,
            patron: Keyhash,
            sub: Keyhash,
        }
        impl Carrier for Fake {
            fn authority(&self, r: &Keyhash) -> Option<String> {
                (*r == self.patron || *r == self.sub).then(|| format!("{}.internal", &hex(r)[..4]))
            }
            fn hand(&self, r: &Keyhash, message: &[u8]) -> Option<Result<Vec<u8>, String>> {
                let text = String::from_utf8_lossy(message).to_string();
                self.handed.borrow_mut().push((*r, text.clone()));
                let line = text.lines().next().unwrap_or("").to_string();
                let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                let answer = |status: u16, body: &str| {
                    Some(Ok(format!(
                        "HTTP/1.1 {status} X\r\ncontent-length: {}\r\n\r\n{body}",
                        body.len()
                    )
                    .into_bytes()))
                };
                if *r == self.patron && line.starts_with("GET /.rhtn/outbox") {
                    // emit once: the second poll finds nothing
                    if self
                        .handed
                        .borrow()
                        .iter()
                        .filter(|(x, m)| *x == self.patron && m.starts_with("GET /.rhtn/outbox"))
                        .count()
                        > 1
                    {
                        return answer(200, "{\"emit\":[]}");
                    }
                    let d = Envelope::live(
                        self.patron,
                        "policy-1",
                        SCHEMA_DIRECTIVE,
                        1,
                        b"{\"retention_floor_days\":90}",
                    );
                    return answer(
                        200,
                        &json!({"emit": [{"to": "subordinates", "envelope": d.to_json()}]})
                            .to_string(),
                    );
                }
                if *r == self.sub && line.starts_with("PUT /.rhtn/envelope") {
                    let got: Value = serde_json::from_str(&body).unwrap();
                    let e = Envelope::from_json(&got).unwrap();
                    assert_eq!(e.schema, SCHEMA_DIRECTIVE);
                    assert!(text.contains("rhtn-relation: patron"), "{text}");
                    assert!(text.contains(&format!("rhtn-origin: {}", hex(&self.patron))));
                    let receipt = Envelope::live(
                        self.sub,
                        "receipt-policy-1-rev1",
                        SCHEMA_RECEIPT,
                        1,
                        b"{\"applied\":true}",
                    );
                    return answer(
                        200,
                        &json!({"emit": [{"to": "patron", "envelope": receipt.to_json()}]})
                            .to_string(),
                    );
                }
                answer(200, "")
            }
        }
        let (patron, sub) = (k(1), k(2));
        let fake = Fake {
            handed: RefCell::new(Vec::new()),
            patron,
            sub,
        };
        let mut f = Federation::default();
        let rp = Relationships {
            subordinates: BTreeSet::from([sub]),
            ..Default::default()
        };
        let rs = Relationships {
            patron: Some(patron),
            ..Default::default()
        };
        f.configure(patron, rp).unwrap();
        f.configure(sub, rs).unwrap();

        let first = f.pass(&fake);
        assert_eq!(first.emitted, 1, "{first:?}");
        assert_eq!(
            first.carried, 1,
            "the directive crossed to the subordinate's plane: {first:?}"
        );
        assert!(first.refused.is_empty(), "{first:?}");
        assert!(f.plane(&sub).unwrap().relationships_told);
        assert_eq!(f.plane(&sub).unwrap().waiting(), 1);

        let second = f.pass(&fake);
        assert_eq!(second.delivered, 1, "{second:?}");
        assert_eq!(
            second.emitted, 1,
            "the receipt rode back on the delivery's answer: {second:?}"
        );
        assert_eq!(
            second.carried, 1,
            "and crossed to the patron's plane: {second:?}"
        );
        assert_eq!(f.plane(&patron).unwrap().waiting(), 1);
        let held = f.plane(&patron).unwrap().held();
        assert_eq!(held[0].1, "receipt-policy-1-rev1");

        let third = f.pass(&fake);
        assert_eq!(third.delivered, 1, "the patron took the receipt: {third:?}");
        let delivered_to_patron = fake
            .handed
            .borrow()
            .iter()
            .filter(|(r, m)| *r == patron && m.starts_with("PUT /.rhtn/envelope"))
            .count();
        assert_eq!(delivered_to_patron, 1);
        // the receipt's path says who carried it
        let text = fake
            .handed
            .borrow()
            .iter()
            .rev()
            .find(|(r, m)| *r == patron && m.starts_with("PUT /.rhtn/envelope"))
            .unwrap()
            .1
            .clone();
        assert!(
            text.contains(&hex(&sub)),
            "the path names the subordinate: {text}"
        );
        assert!(text.contains("rhtn-relation: subordinate"));
    }

    #[test]
    fn an_envelope_for_an_instance_elsewhere_waits_and_a_declined_delivery_is_retried_then_dropped()
    {
        struct Declining;
        impl Carrier for Declining {
            fn authority(&self, _: &Keyhash) -> Option<String> {
                Some("x.internal".into())
            }
            fn hand(&self, _: &Keyhash, message: &[u8]) -> Option<Result<Vec<u8>, String>> {
                let text = String::from_utf8_lossy(message);
                if text.starts_with("PUT /.rhtn/envelope") {
                    return Some(Ok(b"HTTP/1.1 503 X\r\ncontent-length: 0\r\n\r\n".to_vec()));
                }
                Some(Ok(b"HTTP/1.1 200 X\r\ncontent-length: 0\r\n\r\n".to_vec()))
            }
        }
        let mut f = Federation::default();
        let r = Relationships {
            patron: Some(k(1)),
            ..Default::default()
        };
        f.configure(k(5), r).unwrap();
        let e = Envelope::live(k(1), "r", "s@1", 1, b"x");
        f.deliver(&k(5), e).unwrap();
        for _ in 0..MAX_ATTEMPTS {
            let p = f.pass(&Declining);
            assert_eq!(p.declined, 1);
        }
        assert_eq!(f.plane(&k(5)).unwrap().waiting(), 0);
        assert_eq!(f.plane(&k(5)).unwrap().dropped.len(), 1);

        // emitted to a patron this node does not host: pending, kept
        let mut p = Plane::default();
        p.configure(Relationships {
            patron: Some(k(9)),
            ..Default::default()
        });
        let refused = p.emit(
            &k(5),
            &[(
                Destination::Patron,
                Envelope::live(k(5), "up", "s@1", 1, b"y"),
            )],
        );
        assert!(refused.is_empty());
        assert_eq!(p.pending.len(), 1);
        let refused = p.emit(
            &k(5),
            &[(
                Destination::Patron,
                Envelope::live(k(6), "up", "s@1", 1, b"y"),
            )],
        );
        assert!(refused[0].contains("origin"), "{refused:?}");
    }

    #[test]
    fn base64url_round_trips_and_a_response_is_read_leniently() {
        for n in 0..40u8 {
            let bytes: Vec<u8> = (0..n).collect();
            assert_eq!(unbase64url(&base64url(&bytes)).unwrap(), bytes);
        }
        assert_eq!(unbase64url("a"), None);
        assert_eq!(unbase64url("a=b"), None);
        let (s, b) = response_of(b"HTTP/1.1 204 No Content\r\n\r\n");
        assert_eq!((s, b.len()), (204, 0));
        let (s, b) = response_of(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\n{}");
        assert_eq!((s, b.as_slice()), (200, &b"{}"[..]));
        assert_eq!(response_of(b"junk").0, 0);
    }
}
