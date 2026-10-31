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
//! **Between nodes: HTTPS, node to node, raw public keys pinned by the
//! operator** [author, 2026-10-26]. An envelope for a related instance
//! another node hosts is *pending* until the daemon carries it to that
//! node's federation listener over TLS, each side presenting its own
//! transport key and the sender checking the key the operator configured
//! for the far instance's node ([`Location`]); the far node offers it into
//! the instance's plane exactly as its own surface does. The applications
//! see nothing of this: instance to instance in meaning, node to node on
//! the wire, so §5's bookkeeping and the relationship check stay the
//! node's. What is given up is offline delivery — HTTPS reaches a node
//! that is up, and a pending envelope is retried each tick until one is.
//! Where no location is configured for the far instance the envelope
//! stays pending, readable from the operator's surface and deliverable by
//! hand at the other node's. Relationships are configured in the hosting
//! file; the "established by directive" path is not built. No body is
//! pulled on acceptance: an envelope carries its body, bounded. The
//! signature is carried, canonical bytes fixed, and not yet verified.

use crate::Keyhash;
use crate::http::base64url;
use crate::resources::Backend;
use rhtn_codec::cose::sha256;
use rhtn_codec::encode::{emit_array_head, emit_bstr, emit_tstr, emit_uint};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Every path under this prefix is the node's own: the gateway refuses
/// a member's request naming one.
pub const PATH_PREFIX: &str = "/.rhtn/";
/// Where the node delivers one envelope.
pub const PATH_ENVELOPE: &str = "/.rhtn/envelope";
/// Where the node asks for envelopes to send.
pub const PATH_OUTBOX: &str = "/.rhtn/outbox";
/// Where the node states the instance's relationships.
pub const PATH_RELATIONSHIPS: &str = "/.rhtn/relationships";

/// **The header that says a request is the host's own**, inserted on
/// every control exchange [2026-10-27].
///
/// The reserved path is not on its own a proof of where a request came
/// from, and saying it was overstated the case: the gateway strips every
/// `rhtn-*` header a member wrote (`resource-requirements.md` §3.1)
/// before inserting its own, so a header the node sets cannot be forged
/// *through the gateway* — which is conclusive for a package in the
/// sandbox, since nothing else can call it. A resource holding its own
/// port is reachable by other local processes, and for it §3.1's answer
/// stands: bind where only the node reaches you, or require the secret
/// §3.1 describes. A path alone settles neither case.
pub const CONTROL_HEADER: &str = "rhtn-control";
/// What that header carries.
pub const CONTROL_VALUE: &str = "node";

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
/// **The longest an emitted item's own name may be**, and the shape it
/// takes: `[A-Za-z0-9._-]`. The node puts these in the query of the
/// outbox poll it composes, so a name with a space or a newline in it
/// would be a name that breaks the request carrying it.
pub const MAX_ITEM: usize = 64;
/// The most items one outbox poll names as taken.
pub const ITEMS_REPORTED: usize = 64;

/// **Where a plane's custody is kept, and why it is written when it is**
/// [2026-10-27].
///
/// The planes were in memory, which meant a `2xx` to a sending node said
/// *I have it* when nothing had it: a restart lost what had not reached
/// the application, the sender had already dropped its copy on that
/// answer, and no surviving component knew the message had to be sent
/// again. Application idempotence cannot close that — it makes a
/// redelivery harmless and cannot recreate a message nobody knows to
/// resend.
///
/// **So the write happens where custody changes hands, not on a tick.**
/// The rest of this node's state is written back periodically and accepts
/// losing at most an interval (`crate::store::TopologyStore`); custody
/// cannot, because the whole worth of the answer is that the sender may
/// now forget. An envelope is on disk and flushed before the node says it
/// has it, and is removed once the resource has taken it.
///
/// **What is kept, per hosted instance:**
///
/// | File | Holds | Removed when |
/// |---|---|---|
/// | `held` | every record's watermark: revision, digest, state, stage | the record is retired, which nothing does yet |
/// | `in/<seq>` | an envelope admitted and not yet applied | the resource answers 2xx |
/// | `out/<seq>` | an emission admitted and not yet carried | the far side has it |
///
/// `held` is the replay protection, so it is as necessary as the queues:
/// without it a restart would read a tombstoned record's older live copy
/// as new. It is rewritten whole and renamed into place, a few hundred
/// kilobytes at the bound and the simplest thing that cannot tear.
///
/// **Where verification would go.** Admission is the one place where
/// durability and provenance both belong: check, admit, acknowledge.
/// Should instance signatures come to be verified before authoritative
/// state moves, this is the seam — nothing here is written or
/// acknowledged before [`Plane::offer`] has decided.
#[derive(Debug, Clone)]
pub struct Custody {
    dir: PathBuf,
}

/// What one plane's files said, read back at startup.
#[derive(Debug, Default)]
pub struct Kept {
    /// The watermark of each record.
    pub held: Vec<(Keyhash, String, Held)>,
    /// Envelopes admitted and not yet applied, with their attempts.
    pub inbox: Vec<(u64, Envelope, u32)>,
    /// Emissions admitted and not yet carried: the sequence, where it is
    /// addressed, the item the application named it by, and the envelope.
    pub outbox: Vec<(u64, Keyhash, String, Envelope)>,
}

impl Custody {
    /// Custody kept under `dir`, one directory per hosted instance.
    pub fn at(dir: impl Into<PathBuf>) -> Custody {
        Custody { dir: dir.into() }
    }

    /// Where this instance's own files are.
    fn plane(&self, resource: &Keyhash) -> PathBuf {
        self.dir.join(hex(resource))
    }

    /// Write and flush one file, then flush the directory that names it:
    /// a name that is not yet durable names nothing.
    fn put(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write as _;
        if let Some(d) = at.parent() {
            std::fs::create_dir_all(d)?;
        }
        let tmp = at.with_extension("writing");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, at)?;
        if let Some(d) = at.parent()
            && let Ok(dir) = std::fs::File::open(d)
        {
            let _ = dir.sync_all();
        }
        Ok(())
    }

    /// The watermark table, rewritten whole.
    fn write_held(
        &self,
        resource: &Keyhash,
        held: &BTreeMap<(Keyhash, String), Held>,
    ) -> std::io::Result<()> {
        let lines: Vec<String> = held
            .iter()
            .map(|((o, r), h)| {
                json!({
                    "origin": hex(o),
                    "record": r,
                    "revision": h.revision,
                    "digest": hex(&h.digest),
                    "state": h.state.word(),
                    "stage": h.stage.word(),
                })
                .to_string()
            })
            .collect();
        Custody::put(
            &self.plane(resource).join("held"),
            lines.join("\n").as_bytes(),
        )
    }

    /// An envelope taken into custody for a hosted instance.
    fn write_in(&self, resource: &Keyhash, d: &Delivery) -> std::io::Result<()> {
        let body = json!({"attempts": d.attempts, "envelope": d.envelope.to_json()}).to_string();
        Custody::put(
            &self.plane(resource).join("in").join(d.id.to_string()),
            body.as_bytes(),
        )
    }

    /// An emission taken into custody from a hosted instance.
    fn write_out(&self, resource: &Keyhash, p: &Pending) -> std::io::Result<()> {
        let body = json!({
            "to": hex(&p.to),
            "item": p.item,
            "envelope": p.envelope.to_json(),
        })
        .to_string();
        Custody::put(
            &self.plane(resource).join("out").join(p.id.to_string()),
            body.as_bytes(),
        )
    }

    /// Custody given up: the resource has it, or the far side has it.
    fn forget(&self, resource: &Keyhash, which: &str, id: u64) -> std::io::Result<()> {
        let at = self.plane(resource).join(which).join(id.to_string());
        match std::fs::remove_file(&at) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    /// **What this instance was holding**, read back at startup.
    pub fn read(&self, resource: &Keyhash) -> Kept {
        let dir = self.plane(resource);
        let mut kept = Kept::default();
        if let Ok(text) = std::fs::read_to_string(dir.join("held")) {
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let Ok(v) = serde_json::from_str::<Value>(line) else {
                    continue;
                };
                let (Some(o), Some(r), Some(rev), Some(d)) = (
                    v["origin"].as_str().and_then(unhex32),
                    v["record"].as_str(),
                    v["revision"].as_u64(),
                    v["digest"].as_str().and_then(unhex32),
                ) else {
                    continue;
                };
                kept.held.push((
                    o,
                    r.to_string(),
                    Held {
                        revision: rev,
                        digest: d,
                        state: State::of(v["state"].as_str().unwrap_or("live")),
                        stage: Stage::of(v["stage"].as_str().unwrap_or("waiting")),
                    },
                ));
            }
        }
        for which in ["in", "out"] {
            let Ok(rd) = std::fs::read_dir(dir.join(which)) else {
                continue;
            };
            let mut found: Vec<(u64, Value)> = rd
                .flatten()
                .filter_map(|e| {
                    let id = e.file_name().to_string_lossy().parse::<u64>().ok()?;
                    let text = std::fs::read_to_string(e.path()).ok()?;
                    Some((id, serde_json::from_str(&text).ok()?))
                })
                .collect();
            // the sequence is the order they were admitted in, which is
            // the order they are offered again
            found.sort_by_key(|(id, _)| *id);
            for (id, v) in found {
                let Ok(e) = Envelope::from_json(&v["envelope"]) else {
                    continue;
                };
                if which == "in" {
                    kept.inbox
                        .push((id, e, v["attempts"].as_u64().unwrap_or(0) as u32));
                } else if let Some(to) = v["to"].as_str().and_then(unhex32) {
                    kept.outbox.push((
                        id,
                        to,
                        v["item"].as_str().unwrap_or_default().to_string(),
                        e,
                    ));
                }
            }
        }
        kept
    }

    /// Every instance with files here, so a restart can tell a plane it
    /// still hosts from one it does not.
    pub fn instances(&self) -> Vec<Keyhash> {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        rd.flatten()
            .filter_map(|e| unhex32(&e.file_name().to_string_lossy()))
            .collect()
    }
}

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
    /// The word it is written as, on the wire and in custody.
    pub fn word(self) -> &'static str {
        match self {
            State::Live => "live",
            State::Withdrawn => "withdrawn",
        }
    }

    /// Read back from that word; anything else reads as live, which is
    /// the state a record is in before it is withdrawn.
    pub fn of(word: &str) -> State {
        match word {
            "withdrawn" => State::Withdrawn,
            _ => State::Live,
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
            // **a live envelope carries its body** [2026-10-27]: offers
            // then pull is not built, and a bodyless live envelope took
            // the record's revision — so the complete copy that followed
            // was suppressed as a duplicate and the body never arrived
            (None, State::Live) => {
                return Some("a live envelope carries its body: this carrier pulls none".into());
            }
            (None, State::Withdrawn) => {}
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

/// **How far a record has got with the resource it was handed to**
/// [2026-10-27]: admitted into the node's custody is not the same fact as
/// applied by the application, and telling them apart is what lets a
/// delivery that failed for good be asked for again without its origin
/// inventing a revision it does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Admitted and queued, or out for delivery.
    Waiting,
    /// The resource answered 2xx: it has taken responsibility.
    Applied,
    /// Carried no further: declined to exhaustion, or revoked. Offering
    /// the same envelope again queues it afresh.
    Stopped,
}

impl Stage {
    /// The word it is written as in custody.
    pub fn word(self) -> &'static str {
        match self {
            Stage::Waiting => "waiting",
            Stage::Applied => "applied",
            Stage::Stopped => "stopped",
        }
    }

    /// Read back from that word; anything else reads as waiting, which
    /// costs a redelivery and never a loss.
    pub fn of(word: &str) -> Stage {
        match word {
            "applied" => Stage::Applied,
            "stopped" => Stage::Stopped,
            _ => Stage::Waiting,
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
    /// How far it got with the resource.
    pub stage: Stage,
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
    /// A record carried no further — declined to exhaustion or revoked —
    /// and offered again identically: queued afresh, which is how an
    /// operator or a sender retries without a new revision.
    Requeued,
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
/// and how often the resource has declined it.
///
/// **The relation it will arrive under is not kept here** [2026-10-27]:
/// it is read from the relationships at the moment of delivery, because
/// an operator who has removed a relationship since the envelope arrived
/// has withdrawn the authority it would have arrived under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivery {
    /// The envelope.
    pub envelope: Envelope,
    /// Deliveries attempted so far.
    pub attempts: u32,
    /// Which queued delivery this is, so that planning an exchange can
    /// name it without removing it from the queue.
    id: u64,
}

/// **Where an instance another node hosts is reached**: that node's
/// federation listener, and the transport key it presents, which the
/// operator was given by the far operator and wrote beside the
/// relationship. The sender pins it, so a listener presenting another key
/// is not delivered to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// The far node's federation listener.
    pub address: std::net::SocketAddr,
    /// The raw public key that node presents: its transport key, as its
    /// own page and startup line print it.
    pub key: [u8; 32],
}

/// An envelope a hosted resource emitted, waiting for a carrier to the
/// instance it names: this node where it hosts that instance, the
/// daemon's transport where a [`Location`] is configured for it, nothing
/// yet where neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    /// Where it is to go.
    pub to: Keyhash,
    /// The envelope.
    pub envelope: Envelope,
    /// **What the application calls this item**, which is how it learns
    /// the node has custody of it: the next outbox poll names every item
    /// the node holds or has carried, and the application stops offering
    /// those (`resource-requirements.md` §3.3). An item re-offered while
    /// the node still holds it is the same item, not a second one.
    pub item: String,
    /// Which admission this is, so custody can be given up by name.
    id: u64,
}

/// **One hosted resource's side of the plane**: what it has been handed,
/// what waits for it, what it has emitted, and the relationships that
/// decide who may hand it anything.
#[derive(Debug, Default)]
pub struct Plane {
    /// The operator's configuration for this instance.
    pub relationships: Relationships,
    /// What the instance is told of its host beside its relationships
    /// (`infra-client-requirements.md` §9.3): the surface version, where
    /// its block and its log are, as headers on the relationships
    /// exchange.
    pub host: Vec<(String, String)>,
    /// Whether the resource has been told its relationships since they
    /// were last set.
    pub relationships_told: bool,
    held: BTreeMap<(Keyhash, String), Held>,
    inbox: VecDeque<Delivery>,
    /// Emitted and not yet carried.
    pub pending: Vec<Pending>,
    /// Conflicts seen, newest last, for the operator.
    pub conflicts: Vec<(Envelope, Held)>,
    /// **Work this plane stopped carrying, and why**, newest last, for
    /// the operator [2026-10-27]: declined to exhaustion, or revoked
    /// because the relationship it travelled under is gone. Cancellation
    /// that nobody can see is cancellation nobody can answer for.
    pub stopped: Vec<(Envelope, String)>,
    /// **Which configuration this plane is carrying under**: bumped
    /// whenever the relationships change, so a result composed under an
    /// older one is not applied to a newer one.
    pub epoch: u64,
    /// The next admission's identifier, for a queued delivery or a
    /// pending emission alike.
    next_id: u64,
    /// Where this plane's custody is kept, and which instance it is:
    /// `None` keeps it in memory, which promises nothing across a
    /// restart and which the node says plainly.
    keeping: Option<(Custody, Keyhash)>,
    /// The items carried since the last outbox poll, so the application
    /// hears about them once more even if a poll's answer was lost.
    carried_items: VecDeque<String>,
    /// The queued deliveries an exchange has been composed for and not
    /// yet committed.
    out: BTreeSet<u64>,
}

impl Plane {
    /// **Install a relationship configuration, and revoke the work the
    /// old one authorised** [2026-10-27].
    ///
    /// A relationship is the authority an envelope travels under, so
    /// removing one withdraws that authority from work already queued:
    /// an envelope waiting to be delivered from an origin that is no
    /// longer a relationship is stopped, and so is an emission addressed
    /// to an instance that is no longer one. Both are recorded in
    /// [`Plane::stopped`] rather than dropped quietly. The epoch advances
    /// so that an exchange already out under the old configuration is not
    /// committed against this one, and the resource is told again before
    /// anything else reaches it.
    pub fn configure(&mut self, r: Relationships) {
        if self.relationships == r {
            return;
        }
        self.relationships = r;
        self.relationships_told = false;
        self.epoch = self.epoch.wrapping_add(1);
        let queued: Vec<Delivery> = self.inbox.drain(..).collect();
        for d in queued {
            if self.relationships.relation_of(&d.envelope.origin).is_some() {
                self.inbox.push_back(d);
            } else {
                self.out.remove(&d.id);
                self.stop(
                    "in",
                    d.id,
                    d.envelope,
                    "its origin is no longer a configured relationship",
                );
            }
        }
        let waiting = std::mem::take(&mut self.pending);
        for p in waiting {
            if self.relationships.relation_of(&p.to).is_some() {
                self.pending.push(p);
            } else {
                self.stop(
                    "out",
                    p.id,
                    p.envelope,
                    "the instance it was addressed to is no longer a configured relationship",
                );
            }
        }
    }

    /// **Read back what this plane was holding** across a restart: the
    /// watermarks, the envelopes not yet applied, the emissions not yet
    /// carried. Nothing is written here — this is what the files already
    /// say — and the sequence resumes above the highest it finds, so a
    /// new admission cannot be given a name that is still in use.
    fn restore(&mut self, kept: Kept) {
        for (origin, record, held) in kept.held {
            self.held.insert((origin, record), held);
        }
        for (id, envelope, attempts) in kept.inbox {
            self.next_id = self.next_id.max(id);
            self.inbox.push_back(Delivery {
                envelope,
                attempts,
                id,
            });
        }
        for (id, to, item, envelope) in kept.outbox {
            self.next_id = self.next_id.max(id);
            self.pending.push(Pending {
                to,
                envelope,
                item,
                id,
            });
        }
    }

    /// Record work this plane carries no further, with the reason, and
    /// give up custody of it.
    ///
    /// **A delivery stopped leaves its record stopped**, whatever stopped
    /// it [2026-10-27]: exhaustion, a configuration that moved under the
    /// answer, a relationship withdrawn. The watermark says so because
    /// that is what lets the origin offer the identical envelope again
    /// and have it queued afresh (`resource-requirements.md` §3.3) — a
    /// record left *waiting* with nothing queued takes the next offer of
    /// it for a duplicate and suppresses it, so neither side could retry
    /// without inventing a revision.
    fn stop(&mut self, which: &str, id: u64, e: Envelope, why: &str) {
        self.give_up(which, id);
        if which == "in"
            && let Some(h) = self.held.get_mut(&(e.origin, e.record.clone()))
        {
            h.stage = Stage::Stopped;
            self.keep_held();
        }
        if self.stopped.len() >= MAX_CONFLICTS {
            self.stopped.remove(0);
        }
        self.stopped.push((e, why.to_string()));
    }

    /// **Carried, so custody ends here** — and the item is remembered,
    /// so the next outbox poll tells the application once more that the
    /// node has it even if the poll that said so first was lost.
    fn gave(&mut self, p: &Pending) {
        self.give_up("out", p.id);
        if !p.item.is_empty() {
            if self.carried_items.len() >= MAX_OUTBOX {
                self.carried_items.pop_front();
            }
            self.carried_items.push_back(p.item.clone());
        }
    }

    /// Hold an admitted envelope, and the watermark it moved, where a
    /// restart will find them. The error is the caller's to answer with:
    /// a sender told *I have it* by a node that has not written it down
    /// is a sender that may forget it for nothing.
    fn keep_in(&self, d: &Delivery) -> std::io::Result<()> {
        if let Some((c, res)) = &self.keeping {
            c.write_in(res, d)?;
            c.write_held(res, &self.held)?;
        }
        Ok(())
    }

    /// The watermarks alone, which move without an envelope arriving: a
    /// stage reached, a record stopped.
    fn keep_held(&self) {
        if let Some((c, res)) = &self.keeping
            && let Err(e) = c.write_held(res, &self.held)
        {
            tracing::warn!(target: "node", why = %e, "node.federation.custody");
        }
    }

    /// Custody given up, which is never a reason to stop: the worst a
    /// failed removal costs is a redelivery, and an application is
    /// required to be idempotent for exactly that reason.
    fn give_up(&self, which: &str, id: u64) {
        if let Some((c, res)) = &self.keeping
            && let Err(e) = c.forget(res, which, id)
        {
            tracing::warn!(target: "node", why = %e, "node.federation.custody");
        }
    }

    /// **Offer an envelope to this plane** (§5). Taken envelopes join the
    /// inbox for delivery; what became of it is returned either way.
    /// `relation` is how the origin stands to this instance, which the
    /// caller established from the relationships before offering.
    pub fn offer(&mut self, e: Envelope, relation: Relation) -> Outcome {
        let _ = relation;
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
                        // **a record carried no further is asked for
                        // again by offering it again** [2026-10-27]: the
                        // same revision is a duplicate of something
                        // applied, and a retry of something stopped
                        if h.stage == Stage::Stopped {
                            Outcome::Requeued
                        } else {
                            Outcome::Duplicate
                        }
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
            Outcome::New | Outcome::Superseded | Outcome::Requeued => {
                if self.inbox.len() >= MAX_INBOX {
                    return Outcome::Refused(format!(
                        "{MAX_INBOX} envelopes already wait for this instance"
                    ));
                }
                // **a newer revision supersedes the queued older
                // snapshot** [2026-10-27]: left in the queue, the older
                // one would be delivered after the newer had been
                // applied, and a stale live copy could follow a
                // withdrawal
                for q in self
                    .inbox
                    .iter()
                    .filter(|q| q.envelope.origin == e.origin && q.envelope.record == e.record)
                {
                    self.out.remove(&q.id);
                }
                self.inbox
                    .retain(|q| q.envelope.origin != e.origin || q.envelope.record != e.record);
                self.next_id += 1;
                let d = Delivery {
                    envelope: e,
                    attempts: 0,
                    id: self.next_id,
                };
                let was = self.held.insert(
                    key.clone(),
                    Held {
                        revision: d.envelope.revision,
                        digest: d.envelope.digest,
                        state: d.envelope.state,
                        stage: Stage::Waiting,
                    },
                );
                // **written down before it is admitted** [2026-10-27]:
                // the answer that follows tells a sender it may forget
                // its copy, so a store that could not take it is a
                // refusal and not a note in a log
                if let Err(why) = self.keep_in(&d) {
                    match was {
                        Some(h) => {
                            self.held.insert(key, h);
                        }
                        None => {
                            self.held.remove(&key);
                        }
                    }
                    return Outcome::Refused(format!("it could not be taken into custody: {why}"));
                }
                self.inbox.push_back(d);
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

    /// **The next delivery to compose an exchange for, left in the
    /// queue** [2026-10-27], with the relation it would arrive under read
    /// from the relationships as they are *now*: an operator who removed
    /// a relationship has withdrawn the authority the envelope would
    /// arrive under, so one whose origin has ceased to be a relationship
    /// is stopped rather than handed on.
    ///
    /// **Nothing leaves the queue until the exchange is committed**,
    /// because an exchange that is composed and then dropped — a caller
    /// that gives up, a round that is abandoned — must not be the way an
    /// envelope disappears.
    fn pick_delivery(&mut self) -> Option<(Delivery, Relation)> {
        loop {
            let (id, origin) = self
                .inbox
                .iter()
                .find(|d| !self.out.contains(&d.id))
                .map(|d| (d.id, d.envelope.origin))?;
            match self.relationships.relation_of(&origin) {
                Some(r) => {
                    self.out.insert(id);
                    let d = self
                        .inbox
                        .iter()
                        .find(|d| d.id == id)
                        .expect("the delivery just found")
                        .clone();
                    return Some((d, r));
                }
                None => {
                    let d = self.take(id).expect("the delivery just found");
                    self.stop(
                        "in",
                        d.id,
                        d.envelope,
                        "its origin is no longer a configured relationship",
                    );
                }
            }
        }
    }

    /// Remove a queued delivery, wherever it sits.
    fn take(&mut self, id: u64) -> Option<Delivery> {
        self.out.remove(&id);
        let at = self.inbox.iter().position(|d| d.id == id)?;
        self.inbox.remove(at)
    }

    /// The resource took it: out of the queue, and the record is its
    /// responsibility now.
    fn applied(&mut self, d: &Delivery) {
        self.take(d.id);
        if let Some(h) = self
            .held
            .get_mut(&(d.envelope.origin, d.envelope.record.clone()))
        {
            h.stage = Stage::Applied;
        }
        // **the resource has it, so the node need not**: the watermark
        // stays, which is the replay protection, and the envelope goes
        self.keep_held();
        self.give_up("in", d.id);
    }

    /// The resource did not take it. Queue it again at the back, so one
    /// failing record does not starve the rest — unless a newer revision
    /// has been taken since, in which case the snapshot it carries is
    /// superseded and is not to arrive after its successor, or it has
    /// been declined to exhaustion.
    fn declined(&mut self, d: &Delivery) {
        let Some(mut d) = self.take(d.id) else {
            return;
        };
        let key = (d.envelope.origin, d.envelope.record.clone());
        if let Some(h) = self.held.get(&key)
            && h.revision > d.envelope.revision
        {
            self.give_up("in", d.id);
            return;
        }
        d.attempts += 1;
        if d.attempts >= MAX_ATTEMPTS {
            let why = format!("the resource declined it {MAX_ATTEMPTS} times");
            self.stop("in", d.id, d.envelope, &why);
        } else {
            // the attempt count is custody's too, or a record that fails
            // for ever would start again at nought after every restart
            let _ = self.keep_in(&d);
            self.inbox.push_back(d);
        }
    }

    /// **Take what a resource emitted**: each item's destination resolved
    /// against this instance's relationships, each envelope checked, and
    /// each (instance, envelope) queued as pending. Items naming nothing
    /// configured, or an origin other than this instance, are refused
    /// with their reasons.
    pub fn emit(&mut self, me: &Keyhash, items: &[(String, Destination, Envelope)]) -> Vec<String> {
        let mut refused = Vec::new();
        for (item, to, e) in items {
            // **an item the node already holds is the same item**
            // [2026-10-27]: an application that offered one and did not
            // hear that it was taken offers it again, and that is the
            // protocol working rather than a second emission
            if !item.is_empty() && self.pending.iter().any(|p| p.item == *item) {
                continue;
            }
            if !item.is_empty() && !itemlike(item) {
                refused.push(format!(
                    "`{item}` is not an item's name: [A-Za-z0-9._-], 1 to {MAX_ITEM} bytes"
                ));
                continue;
            }
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
                self.next_id += 1;
                let p = Pending {
                    to: t,
                    envelope: e.clone(),
                    item: item.clone(),
                    id: self.next_id,
                };
                // **into custody before it is the node's to carry**: an
                // emission the node reported as taken and then lost on a
                // restart is the loss this store exists to close
                if let Some((c, res)) = &self.keeping
                    && let Err(why) = c.write_out(res, &p)
                {
                    refused.push(format!(
                        "`{}` rev {}: it could not be taken into custody: {why}",
                        e.record, e.revision
                    ));
                    continue;
                }
                self.pending.push(p);
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
    /// Where instances other nodes host are reached, by instance.
    locations: BTreeMap<Keyhash, Location>,
    /// Where custody is kept, where it is kept at all.
    custody: Option<Custody>,
}

/// One envelope to carry to another node: which of this node's instances
/// emitted it, which instance it is for, where that instance's node is,
/// and the envelope itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Abroad {
    /// The emitting instance this node hosts.
    pub from: Keyhash,
    /// The instance it is for.
    pub to: Keyhash,
    /// Where to carry it.
    pub at: Location,
    /// The envelope.
    pub envelope: Envelope,
}

/// What one pass did, for a log line and a test.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Passed {
    /// Instances that took the relationships they were told.
    pub told: usize,
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

/// **Where a hosted resource is reached**, as the gateway knows it: the
/// authority its backend is addressed by, and the backend itself.
///
/// **The backend comes out by `Arc`** [2026-10-27] so that a caller can
/// hand it a message while holding no lock: the gateway sits behind the
/// node's own, and a resource called under that lock stops everything
/// else the node is doing for as long as it takes to answer.
pub trait Addresses {
    /// The authority and the backend, or none where the resource is
    /// brokered or not bound here.
    fn reachable(&self, resource: &Keyhash) -> Option<(String, Arc<dyn Backend>)>;
}

/// **One thing to ask one instance**: composed by [`Federation::work`],
/// performed by the caller, committed by [`Federation::done`].
pub struct Exchange {
    /// The instance being asked.
    pub resource: Keyhash,
    /// Its backend, to hand the message to.
    pub backend: Arc<dyn Backend>,
    /// The message, an HTTP/1.1 request on one of the node's own paths.
    pub message: Vec<u8>,
    /// What is being asked.
    pub about: About,
    /// The relationship configuration it was composed under.
    epoch: u64,
}

impl std::fmt::Debug for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Exchange")
            .field("resource", &hex(&self.resource))
            .field("about", &self.about)
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

/// What an [`Exchange`] is asking for.
#[derive(Debug)]
pub enum About {
    /// The relationships this instance federates within, which it must
    /// take before anything is delivered to it.
    Relationships,
    /// One envelope, which the resource takes or declines.
    Envelope(Box<Delivery>),
    /// A poll for what it has to send, carrying the items the node has
    /// taken custody of since the last one so the application can stop
    /// offering them.
    Outbox {
        /// The items named, cleared from the plane once the application
        /// has heard them.
        reported: Vec<String>,
    },
}

/// An [`Exchange`] and what came back: the status and body, or the words
/// of whatever stopped it.
#[derive(Debug)]
pub struct Answer {
    /// The exchange this answers.
    pub exchange: Exchange,
    /// What the backend said.
    pub answer: Result<(u16, Vec<u8>), String>,
}

impl Federation {
    /// **Where custody is kept**, installed before any plane is
    /// configured: a plane created afterwards keeps what it is handed,
    /// and one created before this was called does not.
    pub fn keep_in(&mut self, custody: Custody) {
        self.custody = Some(custody);
    }

    /// Whether this node is keeping custody at all. A node federating
    /// without it promises nothing across a restart, which its page says
    /// and `infra-client-requirements.md` §9.2 tells it not to do.
    pub fn keeping(&self) -> bool {
        self.custody.is_some()
    }

    /// Configure (or reconfigure) a resource's relationships, creating
    /// its plane.
    ///
    /// **A plane created here reads back what it was holding**: the
    /// watermark of every record, the envelopes admitted and not yet
    /// applied, and the emissions admitted and not yet carried. Order is
    /// the order they were admitted in, which is the order they are
    /// offered again.
    pub fn configure(&mut self, resource: Keyhash, r: Relationships) -> Result<(), String> {
        if let Some(why) = r.refused() {
            return Err(why);
        }
        let fresh = !self.planes.contains_key(&resource);
        let plane = self.planes.entry(resource).or_default();
        if fresh && let Some(c) = &self.custody {
            plane.keeping = Some((c.clone(), resource));
            plane.restore(c.read(&resource));
        }
        plane.configure(r);
        Ok(())
    }

    /// **Tell an instance of its host** (`infra-client-requirements.md`
    /// §9.3): the headers that ride the relationships exchange. A change
    /// is told again, since what it says moved.
    pub fn set_host(&mut self, resource: Keyhash, host: Vec<(String, String)>) {
        let plane = self.planes.entry(resource).or_default();
        if plane.host != host {
            plane.host = host;
            plane.relationships_told = false;
        }
    }

    /// Forget a resource that is no longer hosted.
    pub fn forget(&mut self, resource: &Keyhash) {
        self.planes.remove(resource);
    }

    /// **Keep a plane only for what is still hosted, and tell every
    /// instance its relationships again** [2026-10-27].
    ///
    /// What a reload changes is not only the relationships: a resource
    /// may have stopped being hosted here, in which case its plane is
    /// nobody's; and a resource may have been rebound to a new backend
    /// under relationships that did not change, in which case the plane
    /// thought it had told an instance that has never been told anything.
    /// Both were missed by configuring the planes alone, so a reload says
    /// what is hosted and the plane reconciles itself to it.
    pub fn hosted_now(&mut self, bound: &[Keyhash]) {
        self.planes.retain(|res, _| bound.contains(res));
        for plane in self.planes.values_mut() {
            plane.relationships_told = false;
        }
    }

    /// Set where an instance another node hosts is reached, replacing
    /// what was known. An instance this node hosts needs no location and
    /// is refused one, since a carrier would then send to itself.
    pub fn locate(&mut self, instance: Keyhash, at: Location) -> Result<(), String> {
        if self.planes.contains_key(&instance) {
            return Err(format!(
                "{} is hosted here and needs no location",
                hex(&instance)
            ));
        }
        self.locations.insert(instance, at);
        Ok(())
    }

    /// Forget every location, before a reload writes them again.
    pub fn unlocate_all(&mut self) {
        self.locations.clear();
    }

    /// Where `instance` is reached, where the operator said.
    pub fn location_of(&self, instance: &Keyhash) -> Option<Location> {
        self.locations.get(instance).copied()
    }

    /// **What waits to be carried to another node**: every pending
    /// envelope whose instance has a location. The daemon carries each
    /// and calls [`Federation::carried`] for those the far node took;
    /// the rest stay pending for the next tick.
    pub fn abroad(&self) -> Vec<Abroad> {
        let mut out = Vec::new();
        for (from, plane) in &self.planes {
            for p in &plane.pending {
                if let Some(at) = self.locations.get(&p.to) {
                    out.push(Abroad {
                        from: *from,
                        to: p.to,
                        at: *at,
                        envelope: p.envelope.clone(),
                    });
                }
            }
        }
        out
    }

    /// A pending envelope the far node took: no longer pending.
    pub fn carried(&mut self, from: &Keyhash, to: &Keyhash, envelope: &Envelope) {
        if let Some(plane) = self.planes.get_mut(from)
            && let Some(i) = plane
                .pending
                .iter()
                .position(|p| p.to == *to && p.envelope == *envelope)
        {
            plane.pending.remove(i);
        }
    }

    /// The plane of a resource, where one is configured.
    pub fn plane(&self, resource: &Keyhash) -> Option<&Plane> {
        self.planes.get(resource)
    }

    /// The same, to write to: this module's own, because what a plane
    /// accepts is decided by the rules in here and not by a caller.
    #[cfg(test)]
    fn plane_mut(&mut self, resource: &Keyhash) -> Option<&mut Plane> {
        self.planes.get_mut(resource)
    }

    /// Every resource with a plane.
    pub fn resources(&self) -> Vec<Keyhash> {
        self.planes.keys().copied().collect()
    }

    /// **What the planes are holding**: envelopes admitted and not yet
    /// delivered, and emissions admitted and not yet carried. Said at
    /// startup, because an operator restarting a node wants to know what
    /// it picked up where it left off.
    pub fn holding(&self) -> (usize, usize) {
        self.planes.values().fold((0, 0), |(w, p), plane| {
            (w + plane.waiting(), p + plane.pending.len())
        })
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

    /// **What to ask each hosted instance, composed here and performed
    /// by the caller** [2026-10-27].
    ///
    /// A pass used to make the backend calls itself, which meant a
    /// resource — a socket service or a confined package, either of which
    /// may be slow — was called while whatever lock the planes sit behind
    /// was held, and every other piece of the node's work waited on it.
    /// So the pass is in two halves: this one decides, under that lock,
    /// what each instance is to be asked and hands back the messages with
    /// the backends to hand them to; the caller performs them holding
    /// nothing; [`Federation::done`] commits the answers.
    ///
    /// **One delivery an instance a round**, and the outbox asked after
    /// it: a caller loops until there is no work left, so an instance
    /// with a hundred envelopes waiting does not starve the others, and
    /// one that is slow costs its own round rather than everyone's.
    ///
    /// **Nothing is delivered to an instance that has not taken its
    /// relationships**: an application acting on a directive is acting
    /// under a snapshot of who may send it one, and it has not accepted
    /// that snapshot until it answers.
    pub fn work(&mut self, at: &dyn Addresses) -> Vec<Exchange> {
        let mut out = Vec::new();
        for res in self.resources() {
            let Some((authority, backend)) = at.reachable(&res) else {
                continue;
            };
            let Some(plane) = self.planes.get_mut(&res) else {
                continue;
            };
            let epoch = plane.epoch;
            if !plane.relationships_told {
                let body = plane.relationships.to_json().to_string();
                let extra: Vec<(&str, String)> = plane
                    .host
                    .iter()
                    .map(|(n, v)| (n.as_str(), v.clone()))
                    .collect();
                out.push(Exchange {
                    resource: res,
                    backend,
                    message: message(
                        "PUT",
                        PATH_RELATIONSHIPS,
                        &authority,
                        &res,
                        &extra,
                        body.as_bytes(),
                    ),
                    about: About::Relationships,
                    epoch,
                });
                continue;
            }
            if let Some((d, relation)) = plane.pick_delivery() {
                let extra = [
                    ("rhtn-origin", hex(&d.envelope.origin)),
                    ("rhtn-relation", relation.word()),
                ];
                let body = d.envelope.to_json().to_string();
                out.push(Exchange {
                    resource: res,
                    backend: backend.clone(),
                    message: message(
                        "PUT",
                        PATH_ENVELOPE,
                        &authority,
                        &res,
                        &extra,
                        body.as_bytes(),
                    ),
                    about: About::Envelope(Box::new(d)),
                    epoch,
                });
            }
            // **what the node has taken, said on the way to asking for
            // more** [2026-10-27]: an application holding its own
            // durable outbox needs to know when an item has stopped
            // being its responsibility, and this is the only exchange
            // that can tell it. Named once more if it was named before
            // and the answer went missing; an item re-offered while the
            // node still holds it is the same item.
            let reported: Vec<String> = plane
                .pending
                .iter()
                .map(|p| p.item.clone())
                .chain(plane.carried_items.iter().cloned())
                .filter(|i| !i.is_empty())
                .take(ITEMS_REPORTED)
                .collect();
            let path = if reported.is_empty() {
                PATH_OUTBOX.to_string()
            } else {
                format!("{PATH_OUTBOX}?taken={}", reported.join(","))
            };
            out.push(Exchange {
                resource: res,
                backend,
                message: message("GET", &path, &authority, &res, &[], b""),
                about: About::Outbox { reported },
                epoch,
            });
        }
        out
    }

    /// **Commit what the exchanges answered**, and carry what they
    /// produced to the instances this node hosts.
    ///
    /// **An answer composed under a configuration that has since changed
    /// is not applied** [2026-10-27]: the epoch is checked first, and a
    /// delivery whose epoch has moved is stopped visibly rather than
    /// credited to a snapshot nobody accepted.
    pub fn done(&mut self, answers: Vec<Answer>) -> Passed {
        let mut out = Passed::default();
        for a in answers {
            let Some(plane) = self.planes.get_mut(&a.exchange.resource) else {
                continue;
            };
            if plane.epoch != a.exchange.epoch {
                // **stopped once, with one reason**: the change that
                // moved the epoch may have stopped this delivery itself
                // (its origin is no longer a relationship), and an
                // operator reading two reasons for one envelope would be
                // reading about two things
                if let About::Envelope(d) = a.exchange.about
                    && plane.take(d.id).is_some()
                {
                    plane.stop(
                        "in",
                        d.id,
                        d.envelope,
                        "the relationships changed while it was being delivered",
                    );
                }
                continue;
            }
            let answered = match &a.answer {
                Ok((status, body)) if (200..300).contains(status) => Some(body.clone()),
                _ => None,
            };
            match a.exchange.about {
                About::Relationships => {
                    if answered.is_some() {
                        plane.relationships_told = true;
                        out.told += 1;
                    }
                }
                About::Envelope(d) => {
                    if answered.is_some() {
                        out.delivered += 1;
                        plane.applied(&d);
                    } else {
                        out.declined += 1;
                        plane.declined(&d);
                    }
                }
                About::Outbox { reported } => {
                    // the application has heard which items the node
                    // holds, so they need not be named again
                    if answered.is_some() {
                        plane.carried_items.retain(|i| !reported.contains(i));
                    }
                }
            }
            // an answer to a delivery or a poll alike may carry what the
            // instance has to send
            if let Some(body) = answered {
                let (n, refused) = emitted(plane, &a.exchange.resource, &body);
                out.emitted += n;
                out.refused.extend(refused);
            }
        }
        let (carried, refused) = self.carry();
        out.carried += carried;
        out.refused.extend(refused);
        out
    }

    /// **Carry what is pending to the instances this node hosts**, which
    /// needs no exchange with anyone: an envelope one instance emitted
    /// for another hosted here crosses into that instance's plane.
    ///
    /// What stays pending: an instance this node does not host, which the
    /// daemon's transport carries ([`Federation::abroad`]); a receiver
    /// whose operator has not configured the sender as a relationship, so
    /// §2's rule that an obligation binds only within its own
    /// relationship is kept rather than forced; and a receiver that
    /// refused it for capacity, which is a transient refusal and not a
    /// reason to lose the work.
    fn carry(&mut self) -> (usize, Vec<String>) {
        let (mut carried, mut refused) = (0, Vec::new());
        for res in self.resources() {
            let pending = std::mem::take(&mut self.planes.get_mut(&res).expect("a plane").pending);
            let mut keep = Vec::new();
            for p in pending {
                let Some(target) = self.planes.get_mut(&p.to) else {
                    keep.push(p);
                    continue;
                };
                let Some(relation) = target.relationships.relation_of(&res) else {
                    keep.push(p);
                    continue;
                };
                let mut e = p.envelope.clone();
                e.path.push(res);
                match target.offer(e, relation) {
                    // **capacity is transient, so the work is kept**
                    // [2026-10-27]: dropped here, an emission the
                    // receiver had no room for was lost with only a log
                    // line to say so
                    Outcome::Refused(why) => {
                        refused.push(format!("`{}`: {why}", p.envelope.record));
                        keep.push(p);
                    }
                    // **the receiver's custody begins before the
                    // sender's ends**: the target wrote it down inside
                    // `offer`, so giving it up here cannot lose it
                    _ => {
                        carried += 1;
                        let sender = self.planes.get_mut(&res).expect("a plane");
                        sender.gave(&p);
                    }
                }
            }
            self.planes.get_mut(&res).expect("a plane").pending = keep;
        }
        (carried, refused)
    }
}

/// Read `{"emit": [{"to": ..., "envelope": {...}}, ...]}` from a
/// resource's answer and queue it; an answer that is not that shape emits
/// nothing, which is the common case for a delivery's `200`.
fn emitted(plane: &mut Plane, me: &Keyhash, answer: &[u8]) -> (usize, Vec<String>) {
    let Ok(v) = serde_json::from_slice::<Value>(answer) else {
        return (0, Vec::new());
    };
    let Some(items) = v.get("emit").and_then(Value::as_array) else {
        return (0, Vec::new());
    };
    let (mut parsed, mut refused) = (Vec::new(), Vec::new());
    for item in items {
        let to = item
            .get("to")
            .and_then(Value::as_str)
            .and_then(Destination::parse);
        let env = item.get("envelope").map(Envelope::from_json);
        // **the item's own name, where it gave one**: what the node
        // reports back as taken, and what tells a re-offer from a second
        // emission. An application that names none gets neither.
        let id = item
            .get("item")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        match (to, env) {
            (Some(to), Some(Ok(e))) => parsed.push((id, to, e)),
            (None, _) => refused.push("an emitted item names no destination".into()),
            (_, Some(Err(why))) => refused.push(why),
            (_, None) => refused.push("an emitted item has no envelope".into()),
        }
    }
    let n = parsed.len();
    refused.extend(plane.emit(me, &parsed));
    (n, refused)
}

/// Whether `s` is the shape an emitted item's own name takes.
pub fn itemlike(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_ITEM
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
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
    out.extend_from_slice(format!("{CONTROL_HEADER}: {CONTROL_VALUE}\r\n").as_bytes());
    out.extend_from_slice(format!("rhtn-audience: {}\r\n", base64url(resource)).as_bytes());
    for (n, v) in extra {
        out.extend_from_slice(format!("{n}: {v}\r\n").as_bytes());
    }
    out.extend_from_slice(b"content-type: application/json\r\n");
    out.extend_from_slice(format!("content-length: {}\r\n\r\n", body.len()).as_bytes());
    out.extend_from_slice(body);
    out
}

/// The status and body of a resource's HTTP answer, **read by the one
/// parser** ([`crate::http::parse_response`]).
///
/// It used to read the head itself and leniently, on the reasoning that
/// *a backend's framing is the relay's or the sandbox's to have checked*.
/// Neither did: the sandbox bounds a package's answer by size alone, and
/// the relay now hands up what it read without re-framing it — so a
/// chunked answer on this path would have reached a caller here with its
/// chunk sizes still in the body. One parser, so the three places that
/// read a response cannot disagree about where it ends
/// (`wire-format.md` §11.2).
///
/// **Status 0 for an answer that does not read**, as before for bytes
/// that were not a response at all: nothing on this path treats 0 as
/// taken, so an unreadable answer is one the node has not had — which is
/// the state it was already in before the package replied.
pub fn response_of(bytes: &[u8]) -> (u16, Vec<u8>) {
    match crate::http::parse_response(bytes) {
        Ok(r) => (r.status, r.body),
        Err(why) => {
            tracing::debug!(target: "node", why = ?why, "node.federation.unreadable");
            (0, Vec::new())
        }
    }
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
    use std::collections::VecDeque;

    fn k(n: u8) -> Keyhash {
        [n; 32]
    }

    /// **The instances a test hosts**, each an HTTP origin answering by
    /// *what it was asked* rather than by call order, and each
    /// remembering what it was handed.
    ///
    /// This stands where the gateway stands: it says where a resource is
    /// reached and hands over a backend, and [`round`] performs the
    /// exchanges exactly as the daemon does — plan under the lock, call
    /// outside it, commit.
    struct Backends(BTreeMap<Keyhash, Arc<Instance>>);

    struct Instance {
        /// Answers queued per path, taken in turn.
        answers: Mutex<BTreeMap<String, VecDeque<(u16, String)>>>,
        /// What to answer on a path once its queue is empty.
        standing: Mutex<BTreeMap<String, u16>>,
        handed: Mutex<Vec<String>>,
    }

    use std::sync::Mutex;

    impl Instance {
        /// An instance that takes everything and says nothing.
        fn new() -> Arc<Instance> {
            Arc::new(Instance {
                answers: Mutex::new(BTreeMap::new()),
                standing: Mutex::new(BTreeMap::new()),
                handed: Mutex::new(Vec::new()),
            })
        }
        /// Answer `path` with this, once.
        fn says(self: &Arc<Self>, path: &str, status: u16, body: &str) -> Arc<Self> {
            self.answers
                .lock()
                .unwrap()
                .entry(path.to_string())
                .or_default()
                .push_back((status, body.to_string()));
            self.clone()
        }
        /// Answer `path` with this status from here on.
        fn always(self: &Arc<Self>, path: &str, status: u16) -> Arc<Self> {
            self.standing
                .lock()
                .unwrap()
                .insert(path.to_string(), status);
            self.clone()
        }
        /// Stop refusing `path`.
        fn recovers(self: &Arc<Self>, path: &str) {
            self.standing.lock().unwrap().remove(path);
        }
        fn saw(&self) -> Vec<String> {
            self.handed.lock().unwrap().clone()
        }
        /// The envelopes it was handed, in the order it was handed them.
        fn records(&self) -> Vec<(String, u64)> {
            self.saw()
                .iter()
                .filter(|m| m.starts_with("PUT /.rhtn/envelope"))
                .filter_map(|m| {
                    let body = m.split("\r\n\r\n").nth(1)?;
                    let v: Value = serde_json::from_str(body).ok()?;
                    Some((v["record"].as_str()?.to_string(), v["revision"].as_u64()?))
                })
                .collect()
        }
    }

    impl Backend for Instance {
        fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String> {
            let text = String::from_utf8_lossy(message).to_string();
            let path = text.split(' ').nth(1).unwrap_or_default().to_string();
            self.handed.lock().unwrap().push(text);
            let queued = self
                .answers
                .lock()
                .unwrap()
                .get_mut(&path)
                .and_then(VecDeque::pop_front);
            let (status, body) = queued.unwrap_or_else(|| {
                let s = self
                    .standing
                    .lock()
                    .unwrap()
                    .get(&path)
                    .copied()
                    .unwrap_or(200);
                (s, String::new())
            });
            Ok(format!(
                "HTTP/1.1 {status} X\r\ncontent-length: {}\r\n\r\n{body}",
                body.len()
            )
            .into_bytes())
        }
        fn running(&self) -> bool {
            true
        }
    }

    impl Addresses for Backends {
        fn reachable(&self, resource: &Keyhash) -> Option<(String, Arc<dyn Backend>)> {
            let i = self.0.get(resource)?;
            Some((format!("{}.internal", &hex(resource)[..4]), i.clone()))
        }
    }

    /// One round of the daemon's own loop: plan, perform, commit.
    fn round(f: &mut Federation, at: &Backends) -> Passed {
        let work = f.work(at);
        let answers: Vec<Answer> = work
            .into_iter()
            .map(|e| {
                let answer = e
                    .backend
                    .handle(&e.message)
                    .map(|bytes| response_of(&bytes));
                Answer {
                    exchange: e,
                    answer,
                }
            })
            .collect();
        f.done(answers)
    }

    /// Several rounds, as the daemon's own loop runs them. Fixed in
    /// number: the outbox is asked every round, so there is no round at
    /// which a plane has nothing to say.
    fn settle(f: &mut Federation, at: &Backends) -> Passed {
        let mut all = Passed::default();
        for _ in 0..6 {
            let p = round(f, at);
            all.told += p.told;
            all.delivered += p.delivered;
            all.declined += p.declined;
            all.emitted += p.emitted;
            all.carried += p.carried;
            all.refused.extend(p.refused);
        }
        all
    }

    fn patron_of(k: Keyhash) -> Relationships {
        Relationships {
            patron: Some(k),
            ..Default::default()
        }
    }

    // acceptance: RSC-44
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
        // the array is the six fields in order, and the state is the
        // integer §3.3 fixes: 0 live, 1 withdrawn, and nothing else moves
        let live = Envelope::live(k(1), "r", "s@1", 1, b"x");
        let gone = Envelope::withdrawn(k(1), "r", "s@1", 1, live.digest);
        let (lb, gb) = (live.signed_bytes(), gone.signed_bytes());
        assert_eq!(lb[0], 0x86, "a definite array of six");
        assert_eq!(*lb.last().unwrap(), 0, "live is 0");
        assert_eq!(*gb.last().unwrap(), 1, "withdrawn is 1");
        assert_eq!(lb[..lb.len() - 1], gb[..gb.len() - 1]);
    }

    // acceptance: RSC-45
    #[test]
    fn a_body_that_is_not_its_digest_a_tombstone_with_one_and_a_live_one_without_are_refused() {
        let mut e = Envelope::live(k(1), "r", "s@1", 1, b"x");
        e.digest = [0; 32];
        assert!(e.refused().unwrap().contains("digest"));
        let mut t = Envelope::withdrawn(k(1), "r", "s@1", 2, [0; 32]);
        t.body = Some(b"x".to_vec());
        assert!(t.refused().unwrap().contains("tombstone"));
        assert!(Envelope::from_json(&e.to_json()).is_err());

        // **a live envelope carries its body**: offers then pull is not
        // built, and a bodyless live envelope took the revision and
        // suppressed the complete copy that followed as a duplicate
        let mut hollow = Envelope::live(k(1), "r", "s@1", 1, b"x");
        hollow.body = None;
        assert!(hollow.refused().unwrap().contains("carries its body"));
        let mut p = Plane::default();
        p.configure(patron_of(k(1)));
        assert!(matches!(
            p.offer(hollow, Relation::Patron),
            Outcome::Refused(_)
        ));
        assert_eq!(
            p.offer(Envelope::live(k(1), "r", "s@1", 1, b"x"), Relation::Patron),
            Outcome::New,
            "so the complete copy is the first the instance holds"
        );
    }

    // acceptance: RSC-46
    #[test]
    fn section_five_semantics_in_order() {
        let mut p = Plane::default();
        p.configure(patron_of(k(1)));
        let r = Relation::Patron;
        let v1 = Envelope::live(k(1), "r", "s@1", 1, b"one");
        let v2 = Envelope::live(k(1), "r", "s@1", 2, b"two");
        assert_eq!(p.offer(v1.clone(), r.clone()), Outcome::New);
        assert_eq!(p.offer(v1.clone(), r.clone()), Outcome::Duplicate);
        assert_eq!(p.offer(v2.clone(), r.clone()), Outcome::Superseded);
        assert_eq!(p.offer(v1.clone(), r.clone()), Outcome::Older);
        let forked = Envelope::live(k(1), "r", "s@1", 2, b"TWO");
        assert_eq!(p.offer(forked, r.clone()), Outcome::Conflict);
        assert_eq!(p.conflicts.len(), 1);
        assert_eq!(
            p.held()[0].2.digest,
            v2.digest,
            "the conflict changed nothing"
        );
        let gone = Envelope::withdrawn(k(1), "r", "s@1", 3, v2.digest);
        assert_eq!(p.offer(gone, r.clone()), Outcome::Superseded);
        assert_eq!(p.offer(v2, r.clone()), Outcome::Tombstoned);
        assert_eq!(p.offer(v1, r), Outcome::Tombstoned);
        assert_eq!(p.held()[0].2.state, State::Withdrawn);
        // **one queued snapshot a record**: the older revisions were
        // superseded in the queue rather than left to arrive after their
        // successors
        assert_eq!(p.waiting(), 1);
    }

    // acceptance: RSC-51
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

        // an emission naming an instance that is no relationship of the
        // sender is refused with its reason, and nothing is queued
        let stranger = hex(&k(4));
        let refused = f.plane_mut(&k(5)).expect("a plane").emit(
            &k(5),
            &[(
                "to-nobody".to_string(),
                Destination::parse(&stranger).unwrap(),
                Envelope::live(k(5), "up", "s@1", 1, b"y"),
            )],
        );
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert!(f.plane(&k(5)).expect("a plane").pending.is_empty());

        // **and one the receiver's own operator has not configured is
        // held, not forced through**: the sender's relationships say it
        // may go, the receiver's say nobody may send it, and the
        // receiver's is the one that decides what reaches the receiver
        let (from, to) = (k(7), k(8));
        let i = Instance::new();
        let at = Backends(BTreeMap::from([(from, i.clone()), (to, Instance::new())]));
        let mut g = Federation::default();
        g.configure(
            from,
            Relationships {
                subordinates: BTreeSet::from([to]),
                ..Default::default()
            },
        )
        .unwrap();
        g.configure(to, Relationships::default()).unwrap();
        round(&mut g, &at);
        g.plane_mut(&from).expect("a plane").emit(
            &from,
            &[(
                "down-1".to_string(),
                Destination::Subordinates,
                Envelope::live(from, "r", "s@1", 1, b"x"),
            )],
        );
        settle(&mut g, &at);
        let held = &g.plane(&from).expect("a plane").pending;
        assert_eq!(held.len(), 1, "held");
        assert_eq!(held[0].to, to, "and readable as pending for that instance");
        assert_eq!(
            g.plane(&to).expect("a plane").waiting(),
            0,
            "not forced through"
        );
        assert!(
            !i.saw()
                .iter()
                .any(|m| m.starts_with(&format!("PUT {PATH_ENVELOPE}"))),
            "and nothing was handed to anyone on its behalf: {:?}",
            i.saw()
        );
    }

    /// Two instances on one node: the patron's outbox emits a directive
    /// to its subordinates, the pass carries it, the subordinate's answer
    /// to the delivery emits a receipt to its patron, and that is carried
    /// back. Every exchange carries the header that says it is the host's.
    #[test]
    fn a_directive_goes_down_and_its_receipt_comes_back_on_one_node() {
        let (patron, sub) = (k(1), k(2));
        let directive = Envelope::live(patron, "policy-1", SCHEMA_DIRECTIVE, 1, b"{\"retain\":90}");
        let receipt = Envelope::live(sub, "receipt-policy-1", SCHEMA_RECEIPT, 1, b"{\"ok\":true}");
        let up = Instance::new().says(
            PATH_OUTBOX,
            200,
            &json!({"emit": [{"to": "subordinates", "envelope": directive.to_json()}]}).to_string(),
        );
        // the subordinate answers the directive's delivery with its
        // receipt, which is how a compliance receipt rides back on the
        // directive that asked for it
        let down = Instance::new().says(
            PATH_ENVELOPE,
            200,
            &json!({"emit": [{"to": "patron", "envelope": receipt.to_json()}]}).to_string(),
        );
        let at = Backends(BTreeMap::from([(patron, up.clone()), (sub, down.clone())]));
        let mut f = Federation::default();
        f.configure(
            patron,
            Relationships {
                subordinates: BTreeSet::from([sub]),
                ..Default::default()
            },
        )
        .unwrap();
        f.configure(sub, patron_of(patron)).unwrap();

        let all = settle(&mut f, &at);
        assert!(all.refused.is_empty(), "{all:?}");
        assert_eq!(all.emitted, 2, "a directive and its receipt: {all:?}");
        assert_eq!(all.carried, 2, "each crossed to the other's plane");
        assert_eq!(all.delivered, 2);

        // what the subordinate was handed, and under what
        let delivery = down
            .saw()
            .into_iter()
            .find(|m| m.starts_with("PUT /.rhtn/envelope"))
            .expect("the directive");
        assert!(delivery.contains(&format!("{CONTROL_HEADER}: {CONTROL_VALUE}")));
        assert!(delivery.contains("rhtn-relation: patron"));
        assert!(delivery.contains(&format!("rhtn-origin: {}", hex(&patron))));
        assert!(delivery.contains("host: 0202.internal"), "{delivery}");
        let got = Envelope::from_json(
            &serde_json::from_str(delivery.split("\r\n\r\n").nth(1).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(got.record, "policy-1");
        assert_eq!(got.path, vec![patron], "the path names what carried it");
        // and the relationships came first, to both
        for i in [&up, &down] {
            assert!(
                i.saw()[0].starts_with("PUT /.rhtn/relationships"),
                "{:?}",
                i.saw()[0]
            );
        }
        assert_eq!(up.records(), vec![("receipt-policy-1".to_string(), 1)]);
    }

    /// **An instance is told of its host on the exchange that tells it its
    /// relationships** (`infra-client-requirements.md` §9.3): the surface
    /// version and where its block and its log are, as headers; a change
    /// to them is told again, and nothing else of the plane changes.
    // acceptance: DMN-35
    #[test]
    fn an_instance_is_told_of_its_host_beside_its_relationships() {
        let res = k(5);
        let i = Instance::new();
        let at = Backends(BTreeMap::from([(res, i.clone())]));
        let mut f = Federation::default();
        f.configure(res, patron_of(k(1))).unwrap();
        f.set_host(
            res,
            vec![
                ("rhtn-host".into(), "rhtn/host@1".into()),
                ("rhtn-block".into(), "/state/blocks/05".into()),
                ("rhtn-block-bytes".into(), "4096".into()),
            ],
        );
        settle(&mut f, &at);
        let told = i.saw().into_iter().next().expect("told");
        assert!(told.starts_with("PUT /.rhtn/relationships"), "{told}");
        assert!(told.contains("rhtn-host: rhtn/host@1\r\n"), "{told}");
        assert!(told.contains("rhtn-block: /state/blocks/05\r\n"), "{told}");
        assert!(told.contains("rhtn-block-bytes: 4096\r\n"), "{told}");
        assert!(f.plane(&res).unwrap().relationships_told);
        // the same facts again change nothing; moved facts are told again
        f.set_host(
            res,
            vec![
                ("rhtn-host".into(), "rhtn/host@1".into()),
                ("rhtn-block".into(), "/state/blocks/05".into()),
                ("rhtn-block-bytes".into(), "4096".into()),
            ],
        );
        assert!(f.plane(&res).unwrap().relationships_told);
        f.set_host(res, vec![("rhtn-host".into(), "rhtn/host@1".into())]);
        assert!(!f.plane(&res).unwrap().relationships_told);
        settle(&mut f, &at);
        let again = i.saw().into_iter().nth(1).expect("told again");
        assert!(!again.contains("rhtn-block:"), "{again}");
    }

    /// **Nothing is delivered to an instance that has not taken its
    /// relationships.** An application acting on a directive acts under a
    /// snapshot of who may send it one; until it answers, it has accepted
    /// no snapshot.
    // acceptance: RSC-47
    #[test]
    fn an_instance_that_refuses_its_relationships_is_handed_nothing_else() {
        let res = k(5);
        let i = Instance::new().always(PATH_RELATIONSHIPS, 500);
        let at = Backends(BTreeMap::from([(res, i.clone())]));
        let mut f = Federation::default();
        f.configure(res, patron_of(k(1))).unwrap();
        f.deliver(&res, Envelope::live(k(1), "r", "s@1", 1, b"x"))
            .unwrap();
        settle(&mut f, &at);
        assert!(
            i.saw()
                .iter()
                .all(|m| m.starts_with("PUT /.rhtn/relationships")),
            "only the relationships were offered: {:?}",
            i.saw()
        );
        assert_eq!(
            f.plane(&res).unwrap().waiting(),
            1,
            "the envelope still waits"
        );
        assert!(!f.plane(&res).unwrap().relationships_told);
    }

    /// **A newer revision is not followed by an older one.** The resource
    /// declines revision 1, takes revision 2, and never sees 1 again: the
    /// queued older snapshot is superseded, and one still out when a
    /// newer is taken is not requeued behind it.
    // acceptance: RSC-48
    #[test]
    fn a_declined_revision_is_not_delivered_after_a_newer_one_was_taken() {
        let res = k(5);
        // the first envelope is declined, everything after is taken
        let i = Instance::new().says(PATH_ENVELOPE, 503, "");
        let at = Backends(BTreeMap::from([(res, i.clone())]));
        let mut f = Federation::default();
        f.configure(res, patron_of(k(1))).unwrap();
        f.deliver(&res, Envelope::live(k(1), "r", "s@1", 1, b"one"))
            .unwrap();
        round(&mut f, &at); // relationships
        round(&mut f, &at); // revision 1, declined
        f.deliver(&res, Envelope::live(k(1), "r", "s@1", 2, b"two"))
            .unwrap();
        settle(&mut f, &at);
        assert_eq!(
            i.records(),
            vec![("r".to_string(), 1), ("r".to_string(), 2)],
            "1 then 2, and never 1 again"
        );
        assert_eq!(f.plane(&res).unwrap().waiting(), 0);
    }

    /// **A delivery that failed for good can be asked for again** without
    /// its origin inventing a revision it does not have: offering the
    /// identical envelope requeues it.
    // acceptance: RSC-49
    #[test]
    fn a_record_carried_no_further_is_requeued_by_offering_it_again() {
        let res = k(5);
        let i = Instance::new().always(PATH_ENVELOPE, 503);
        let at = Backends(BTreeMap::from([(res, i.clone())]));
        let mut f = Federation::default();
        f.configure(res, patron_of(k(1))).unwrap();
        let e = Envelope::live(k(1), "r", "s@1", 1, b"x");
        f.deliver(&res, e.clone()).unwrap();
        for _ in 0..=MAX_ATTEMPTS {
            round(&mut f, &at);
        }
        let plane = f.plane(&res).unwrap();
        assert_eq!(plane.waiting(), 0, "it was declined to exhaustion");
        assert_eq!(plane.stopped.len(), 1);
        assert!(
            plane.stopped[0].1.contains("declined"),
            "{:?}",
            plane.stopped[0].1
        );
        assert_eq!(plane.held()[0].2.stage, Stage::Stopped);
        // the resource recovers, and the same envelope is offered again:
        // queued afresh rather than suppressed as a duplicate
        i.recovers(PATH_ENVELOPE);
        assert_eq!(f.deliver(&res, e.clone()).unwrap(), Outcome::Requeued);
        assert_eq!(f.plane(&res).unwrap().waiting(), 1);
        settle(&mut f, &at);
        assert_eq!(f.plane(&res).unwrap().held()[0].2.stage, Stage::Applied);
        assert_eq!(i.records().last().unwrap(), &("r".to_string(), 1));
    }

    /// **Removing a relationship revokes the work it authorised**, on
    /// both sides, and says so where an operator can see it.
    // acceptance: RSC-52
    #[test]
    fn removing_a_relationship_revokes_queued_and_pending_work() {
        let res = k(5);
        let i = Instance::new();
        let at = Backends(BTreeMap::from([(res, i.clone())]));
        let mut f = Federation::default();
        f.configure(res, patron_of(k(1))).unwrap();
        // one envelope in from the patron, one emission out to it
        f.deliver(&res, Envelope::live(k(1), "r", "s@1", 1, b"x"))
            .unwrap();
        round(&mut f, &at); // relationships taken
        let plane = f.plane_mut(&res).expect("a plane");
        plane.emit(
            &res,
            &[(
                "up-1".to_string(),
                Destination::Patron,
                Envelope::live(res, "up", "s@1", 1, b"y"),
            )],
        );
        assert_eq!(f.plane(&res).unwrap().waiting(), 1);
        assert_eq!(f.plane(&res).unwrap().pending.len(), 1);
        let epoch = f.plane(&res).unwrap().epoch;

        // the patron is removed
        f.configure(res, Relationships::default()).unwrap();
        let plane = f.plane(&res).unwrap();
        assert_eq!(plane.waiting(), 0, "the delivery is revoked");
        assert!(plane.pending.is_empty(), "and so is the emission");
        assert_eq!(plane.stopped.len(), 2, "both visible: {:?}", plane.stopped);
        assert!(
            plane
                .stopped
                .iter()
                .all(|(_, why)| why.contains("no longer"))
        );
        assert!(plane.epoch > epoch, "the configuration moved");
        assert!(!plane.relationships_told, "and is announced again");
        settle(&mut f, &at);
        assert!(
            i.saw()
                .iter()
                .all(|m| !m.starts_with("PUT /.rhtn/envelope")),
            "nothing was handed over under the old relationship: {:?}",
            i.saw()
        );

        // **the relation is read when the envelope is handed over**, not
        // when it arrived: an instance is never told a relation its
        // operator has since changed
        let other = k(9);
        let j = Instance::new();
        let at = Backends(BTreeMap::from([(other, j.clone())]));
        let mut g = Federation::default();
        g.configure(other, patron_of(k(1))).unwrap();
        g.deliver(&other, Envelope::live(k(1), "r", "s@1", 1, b"x"))
            .unwrap();
        g.configure(
            other,
            Relationships {
                kin: BTreeMap::from([("audit".to_string(), BTreeSet::from([k(1)]))]),
                ..Default::default()
            },
        )
        .unwrap();
        settle(&mut g, &at);
        let handed: Vec<String> = j
            .saw()
            .into_iter()
            .filter(|m| m.starts_with(&format!("PUT {PATH_ENVELOPE}")))
            .collect();
        assert_eq!(handed.len(), 1, "{handed:?}");
        assert!(
            handed[0].contains("rhtn-relation: kin:audit"),
            "the relation it arrived under is gone: {}",
            handed[0].lines().next().unwrap_or_default()
        );
    }

    /// **An emission a receiver had no room for is kept**, not lost: the
    /// refusal is transient and the sender's copy is the only one.
    #[test]
    fn an_emission_the_receiver_cannot_hold_stays_pending() {
        let (from, to) = (k(5), k(6));
        let _at = Backends(BTreeMap::from([
            (from, Instance::new()),
            (to, Instance::new()),
        ]));
        let mut f = Federation::default();
        f.configure(
            from,
            Relationships {
                subordinates: BTreeSet::from([to]),
                ..Default::default()
            },
        )
        .unwrap();
        f.configure(to, patron_of(from)).unwrap();
        // fill the receiver's inbox
        for n in 0..MAX_INBOX {
            let e = Envelope::live(from, &format!("r{n}"), "s@1", 1, b"x");
            assert_eq!(f.deliver(&to, e).unwrap(), Outcome::New);
        }
        let over = Envelope::live(from, "over", "s@1", 1, b"x");
        f.plane_mut(&from).expect("a plane").emit(
            &from,
            &[(
                "over-1".to_string(),
                Destination::Subordinates,
                over.clone(),
            )],
        );
        let p = f.done(Vec::new());
        assert_eq!(p.carried, 0);
        assert!(
            p.refused.iter().any(|w| w.contains("already wait")),
            "the refusal is said: {:?}",
            p.refused
        );
        let still = &f.plane(&from).unwrap().pending;
        assert_eq!(still.len(), 1, "the work is still the sender's to carry");
        assert_eq!((still[0].to, &still[0].envelope), (to, &over));
    }

    // acceptance: DMN-30
    #[test]
    fn a_pending_envelope_for_a_located_instance_is_abroad_until_carried() {
        let mut f = Federation::default();
        f.configure(k(5), patron_of(k(9))).unwrap();
        let at = Location {
            address: "203.0.113.9:7452".parse().unwrap(),
            key: [7; 32],
        };
        assert!(
            f.locate(k(5), at).is_err(),
            "a hosted instance has no location"
        );
        f.locate(k(9), at).unwrap();
        let e = Envelope::live(k(5), "up", "s@1", 1, b"y");
        f.plane_mut(&k(5)).expect("a plane").emit(
            &k(5),
            &[("up-1".to_string(), Destination::Patron, e.clone())],
        );
        let abroad = f.abroad();
        assert_eq!(abroad.len(), 1);
        assert_eq!(
            (abroad[0].from, abroad[0].to, abroad[0].at),
            (k(5), k(9), at)
        );
        f.carried(&k(5), &k(9), &abroad[0].envelope);
        assert!(f.abroad().is_empty());
        assert!(f.plane(&k(5)).unwrap().pending.is_empty());

        // **a destination that is no longer a relationship is not carried
        // anywhere**, location or not
        f.plane_mut(&k(5))
            .expect("a plane")
            .emit(&k(5), &[("up-2".to_string(), Destination::Patron, e)]);
        assert_eq!(f.abroad().len(), 1);
        f.configure(k(5), Relationships::default()).unwrap();
        assert!(
            f.abroad().is_empty(),
            "the authority it would travel under is gone"
        );
        f.unlocate_all();
        assert_eq!(f.location_of(&k(9)), None);

        // **and where no location is written it stays pending**, not
        // carried and not lost: readable from the surface, and
        // deliverable at the other node's surface by hand
        f.configure(k(5), patron_of(k(9))).unwrap();
        f.plane_mut(&k(5)).expect("a plane").emit(
            &k(5),
            &[(
                "up-4".to_string(),
                Destination::Patron,
                Envelope::live(k(5), "up", "s@1", 2, b"z"),
            )],
        );
        assert!(f.abroad().is_empty(), "nowhere to carry it to");
        let pending = &f.plane(&k(5)).expect("a plane").pending;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].to, k(9), "and it says which instance owes it");
    }

    /// **What a restart finds** (`Custody`): the envelopes it had taken
    /// and not delivered, the emissions it had taken and not carried, and
    /// the watermark of every record — which is the replay protection, so
    /// a tombstoned record's older live copy is still refused on the
    /// other side of a restart.
    ///
    /// This is the finding the review put first among the ones it called
    /// unfixed: a 2xx said *I have it* when nothing had it, and
    /// application idempotence cannot recreate a message no surviving
    /// component knows to resend.
    // acceptance: DMN-31
    #[test]
    fn a_restart_finds_what_was_admitted_and_what_it_already_knew() {
        let dir = std::env::temp_dir().join(format!("rhtn-custody-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (res, patron) = (k(5), k(1));

        // one envelope admitted, one emission admitted, one record
        // withdrawn and its tombstone applied
        {
            let mut f = Federation::default();
            f.keep_in(Custody::at(&dir));
            f.configure(res, patron_of(patron)).unwrap();
            assert!(f.keeping());
            let live = Envelope::live(patron, "policy-1", SCHEMA_DIRECTIVE, 1, b"{\"a\":1}");
            assert_eq!(f.deliver(&res, live.clone()).unwrap(), Outcome::New);
            let gone = Envelope::withdrawn(patron, "policy-0", SCHEMA_DIRECTIVE, 9, [3; 32]);
            assert_eq!(f.deliver(&res, gone).unwrap(), Outcome::New);
            f.plane_mut(&res).expect("a plane").emit(
                &res,
                &[(
                    "receipt-1".to_string(),
                    Destination::Patron,
                    Envelope::live(res, "receipt-1", SCHEMA_RECEIPT, 1, b"{\"ok\":true}"),
                )],
            );
            assert_eq!(f.plane(&res).unwrap().waiting(), 2);
            assert_eq!(f.plane(&res).unwrap().pending.len(), 1);
        }

        // the node stops. Nothing was applied and nothing was carried,
        // so everything is still the node's to deliver
        {
            let mut f = Federation::default();
            f.keep_in(Custody::at(&dir));
            f.configure(res, patron_of(patron)).unwrap();
            let plane = f.plane(&res).expect("a plane");
            assert_eq!(plane.waiting(), 2, "both envelopes are still to deliver");
            assert_eq!(plane.pending.len(), 1, "and the emission is still to carry");
            assert_eq!(plane.pending[0].item, "receipt-1");
            assert_eq!(plane.held().len(), 2, "and it knows what it holds");

            // **the watermark is the replay protection**: the withdrawn
            // record's older live copy is still refused, which an
            // in-memory plane would have taken as new
            let replayed = Envelope::live(patron, "policy-0", SCHEMA_DIRECTIVE, 8, b"{\"old\":1}");
            assert_eq!(f.deliver(&res, replayed).unwrap(), Outcome::Tombstoned);
            // and the duplicate of what is waiting is still a duplicate
            let again = Envelope::live(patron, "policy-1", SCHEMA_DIRECTIVE, 1, b"{\"a\":1}");
            assert_eq!(f.deliver(&res, again).unwrap(), Outcome::Duplicate);
        }

        // what is applied and what is carried are given up, and a
        // restart after that finds neither
        {
            let i = Instance::new();
            let at = Backends(BTreeMap::from([(res, i.clone())]));
            let mut f = Federation::default();
            f.keep_in(Custody::at(&dir));
            f.configure(res, patron_of(patron)).unwrap();
            settle(&mut f, &at);
            assert_eq!(f.plane(&res).unwrap().waiting(), 0, "both were taken");
            // the poll said which item the node holds
            let polls: Vec<String> = i
                .saw()
                .into_iter()
                .filter(|m| m.starts_with("GET /.rhtn/outbox"))
                .collect();
            assert!(
                polls.iter().any(|m| m.contains("taken=receipt-1")),
                "the application is told what the node has: {polls:?}"
            );
        }
        {
            let mut f = Federation::default();
            f.keep_in(Custody::at(&dir));
            f.configure(res, patron_of(patron)).unwrap();
            let plane = f.plane(&res).expect("a plane");
            assert_eq!(plane.waiting(), 0, "nothing is owed to the resource");
            assert_eq!(
                plane.held().len(),
                2,
                "and the watermarks stay, being the replay protection"
            );
            assert!(
                plane
                    .held()
                    .iter()
                    .all(|(_, _, h)| h.stage == Stage::Applied),
                "each one applied: {:?}",
                plane.held()
            );
            // the emission is still pending: nothing carried it, the
            // patron being at no node this test configured
            assert_eq!(plane.pending.len(), 1);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// An item's name is the application's, and is held to a shape a
    /// request can carry: the node puts it in the query of the poll it
    /// composes.
    // acceptance: RSC-50
    #[test]
    fn an_items_name_is_bounded_and_a_re_offer_is_the_same_item() {
        let mut p = Plane::default();
        p.configure(patron_of(k(1)));
        let e = Envelope::live(k(5), "up", "s@1", 1, b"y");
        let refused = p.emit(
            &k(5),
            &[("a b".to_string(), Destination::Patron, e.clone())],
        );
        assert!(refused[0].contains("is not an item's name"), "{refused:?}");
        assert!(p.pending.is_empty());
        let good = p.emit(
            &k(5),
            &[("up-1".to_string(), Destination::Patron, e.clone())],
        );
        assert!(good.is_empty());
        assert_eq!(p.pending.len(), 1);
        // offered again while the node still holds it: the same item
        let again = p.emit(&k(5), &[("up-1".to_string(), Destination::Patron, e)]);
        assert!(again.is_empty());
        assert_eq!(p.pending.len(), 1, "not a second emission");

        // **the poll is where the application hears it**: the item the
        // node holds is named in `?taken=`, and a carried one is named
        // once more in case the answer that said so first went missing.
        // Serving a poll is not itself an acknowledgement of anything.
        let (res, patron) = (k(5), k(1));
        let (mine, theirs) = (Instance::new(), Instance::new());
        let at = Backends(BTreeMap::from([
            (res, mine.clone()),
            (patron, theirs.clone()),
        ]));
        let mut f = Federation::default();
        f.configure(res, patron_of(patron)).unwrap();
        f.configure(
            patron,
            Relationships {
                subordinates: BTreeSet::from([res]),
                ..Default::default()
            },
        )
        .unwrap();
        round(&mut f, &at); // relationships, both of them
        f.plane_mut(&res).expect("a plane").emit(
            &res,
            &[(
                "up-7".to_string(),
                Destination::Patron,
                Envelope::live(res, "up", SCHEMA_RECEIPT, 1, b"y"),
            )],
        );
        let polls = || -> Vec<String> {
            mine.saw()
                .into_iter()
                .filter(|m| m.starts_with(&format!("GET {PATH_OUTBOX}")))
                .collect()
        };
        let before = polls().len();
        round(&mut f, &at);
        assert!(
            polls()[before..].iter().any(|m| m.contains("taken=up-7")),
            "the item the node holds is named: {:?}",
            polls()
        );
        assert!(
            f.plane(&res).expect("a plane").pending.is_empty(),
            "and it was carried"
        );
        let after = polls().len();
        round(&mut f, &at);
        assert!(
            polls()[after..].iter().any(|m| m.contains("taken=up-7")),
            "named once more, the first answer having been able to go missing: {:?}",
            polls()
        );
        let heard = polls().len();
        round(&mut f, &at);
        assert!(
            polls()[heard..].iter().all(|m| !m.contains("up-7")),
            "and not again once the application has heard it: {:?}",
            polls()
        );
    }

    /// **The instance is called holding nothing, and an answer the
    /// configuration moved under is not applied**
    /// (`infra-client-requirements.md` §9.2).
    ///
    /// The adversarial shape: the instance, *while the node is calling
    /// it*, reaches back into the node and reconfigures its own plane. A
    /// pass that called a resource under the lock guarding this node's
    /// state would hold that lock for as long as the resource took, so
    /// the re-entry is the test — the plan is data, the call is the
    /// caller's, and here the caller is holding the lock the instance
    /// takes. Its answer then concerns a configuration nobody accepted,
    /// so it is stopped where an operator sees it rather than credited;
    /// offering the identical envelope again queues it afresh.
    // acceptance: DMN-33
    #[test]
    fn an_instance_called_mid_round_can_reconfigure_its_plane_and_its_answer_is_not_applied() {
        use std::sync::OnceLock;

        /// An instance that reconfigures its own node from inside its
        /// answer, and says whether it was able to.
        struct Reenters {
            node: OnceLock<Arc<Mutex<Federation>>>,
            res: Keyhash,
            /// What it was asked, in order.
            saw: Mutex<Vec<String>>,
            /// Whether the lock was free while it was answering.
            free: Mutex<Option<bool>>,
        }

        impl Backend for Reenters {
            fn handle(&self, message: &[u8]) -> Result<Vec<u8>, String> {
                let text = String::from_utf8_lossy(message).to_string();
                let path = text.split(' ').nth(1).unwrap_or_default().to_string();
                self.saw.lock().unwrap().push(path.clone());
                if path == PATH_ENVELOPE && self.free.lock().unwrap().is_none() {
                    // `try_lock` rather than `lock`: a pass holding it
                    // would deadlock here, and a hanging test says less
                    // than a failing one
                    let node = self.node.get().expect("the node");
                    let free = match node.try_lock() {
                        Ok(mut f) => {
                            // the patron stays a relationship, so what is
                            // queued is not revoked: the epoch alone moves
                            f.configure(
                                self.res,
                                Relationships {
                                    patron: Some(k(1)),
                                    kin: BTreeMap::from([(
                                        "audit".to_string(),
                                        BTreeSet::from([k(3)]),
                                    )]),
                                    ..Default::default()
                                },
                            )
                            .unwrap();
                            true
                        }
                        Err(_) => false,
                    };
                    *self.free.lock().unwrap() = Some(free);
                }
                Ok(b"HTTP/1.1 200 X\r\ncontent-length: 0\r\n\r\n".to_vec())
            }
            fn running(&self) -> bool {
                true
            }
        }

        /// One instance, reachable.
        struct One(Keyhash, Arc<dyn Backend>);
        impl Addresses for One {
            fn reachable(&self, resource: &Keyhash) -> Option<(String, Arc<dyn Backend>)> {
                (*resource == self.0).then(|| ("one.internal".to_string(), self.1.clone()))
            }
        }

        /// The daemon's own round, with the node's state behind the lock
        /// it really is behind: plan under it, call holding nothing,
        /// commit under it again.
        fn round_locked(f: &Arc<Mutex<Federation>>, at: &dyn Addresses) -> Passed {
            let work = f.lock().unwrap().work(at);
            let answers: Vec<Answer> = work
                .into_iter()
                .map(|e| {
                    let answer = e.backend.handle(&e.message).map(|b| response_of(&b));
                    Answer {
                        exchange: e,
                        answer,
                    }
                })
                .collect();
            f.lock().unwrap().done(answers)
        }

        let res = k(5);
        let it = Arc::new(Reenters {
            node: OnceLock::new(),
            res,
            saw: Mutex::new(Vec::new()),
            free: Mutex::new(None),
        });
        let node = Arc::new(Mutex::new(Federation::default()));
        it.node.set(node.clone()).ok();
        let at = One(res, it.clone());
        let envelope = Envelope::live(k(1), "r", SCHEMA_DIRECTIVE, 1, b"x");
        {
            let mut f = node.lock().unwrap();
            f.configure(res, patron_of(k(1))).unwrap();
            f.deliver(&res, envelope.clone()).unwrap();
        }
        round_locked(&node, &at); // the relationships, taken
        round_locked(&node, &at); // the envelope, delivered mid-reconfiguration

        assert_eq!(
            *it.free.lock().unwrap(),
            Some(true),
            "the node called the instance while holding its own lock"
        );
        let f = node.lock().unwrap();
        let plane = f.plane(&res).expect("a plane");
        assert_eq!(plane.held().len(), 1, "the record's watermark is its own");
        assert_eq!(
            plane.held()[0].2.stage,
            Stage::Stopped,
            "stopped, not applied, and stopped is what lets it be offered again"
        );
        assert_eq!(
            plane.stopped.len(),
            1,
            "stopped visibly: {:?}",
            plane.stopped
        );
        assert!(
            plane.stopped[0]
                .1
                .contains("changed while it was being delivered")
        );
        drop(f);

        // the identical envelope, offered again, queues afresh and this
        // time is applied: nothing was lost by the refusal
        node.lock().unwrap().deliver(&res, envelope).unwrap();
        round_locked(&node, &at); // the relationships again, after the change
        round_locked(&node, &at);
        let f = node.lock().unwrap();
        let plane = f.plane(&res).expect("a plane");
        assert_eq!(plane.held()[0].2.stage, Stage::Applied);
        assert_eq!(
            it.saw
                .lock()
                .unwrap()
                .iter()
                .filter(|p| *p == PATH_ENVELOPE)
                .count(),
            2,
            "the second offer was delivered, not deduplicated"
        );
    }

    /// **A rebound backend is told its relationships again, and the plane
    /// of a resource this node no longer hosts is forgotten**
    /// (`infra-client-requirements.md` §9.2). Configuring the planes alone
    /// missed both: a new backend has been told nothing while the plane
    /// thought it had told the instance, and a plane nobody hosts is
    /// nobody's.
    // acceptance: DMN-34
    #[test]
    fn a_rebound_backend_is_told_again_and_an_unhosted_plane_is_forgotten() {
        let (a, b) = (k(5), k(6));
        let (ia, ib) = (Instance::new(), Instance::new());
        let at = Backends(BTreeMap::from([(a, ia.clone()), (b, ib.clone())]));
        let mut f = Federation::default();
        f.configure(a, patron_of(k(1))).unwrap();
        f.configure(b, patron_of(k(1))).unwrap();
        settle(&mut f, &at);
        let told = |i: &Arc<Instance>| {
            i.saw()
                .iter()
                .filter(|m| m.starts_with(&format!("PUT {PATH_RELATIONSHIPS}")))
                .count()
        };
        assert_eq!((told(&ia), told(&ib)), (1, 1), "each told once");

        // a reload: `a` is rebound to another backend under relationships
        // that did not change, and `b` is no longer hosted here
        f.hosted_now(&[a]);
        assert!(f.plane(&b).is_none(), "its plane is nobody's");
        f.configure(a, patron_of(k(1))).unwrap();
        settle(&mut f, &at);
        assert_eq!(told(&ia), 2, "the new backend had been told nothing");
        assert_eq!(told(&ib), 1, "and nothing is pushed to what is not hosted");
        assert_eq!(f.resources(), vec![a]);
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
