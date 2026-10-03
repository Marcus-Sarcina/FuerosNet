//! Diagnostic field constructors for the node's events
//! (`Robot/field-test-diagnostics.md`, section 3.5 is the table, section 2
//! the redaction rule).
//!
//! **Constructors, not a framework.** The events are `tracing` events raised
//! where the table says; this module is where an identity, a count or a
//! variant is turned into a field one way everywhere, so that a secret has
//! no constructor at all. An event names variants, counts, sizes, reason
//! strings the code already produces, and the first eight hex characters of
//! an identifier. It never formats an object's bytes, a ciphertext, a seed,
//! a signature or a signed record whole: `Decision`, `MemoOutcome`,
//! `Replicated` and `Delivery` are rendered by the functions here rather
//! than by `Debug`, because their derived `Debug` prints whole keyhashes
//! and, for `Replicated`, the object itself.
//!
//! The same small set the client's `diag` module carries, copied rather
//! than depended on: a node has no reason to link the client.
//!
//! **Compiled out of a releasable build.** The leaf binary's `releasable`
//! feature sets `tracing/max_level_off`, under which every `event!` in this
//! crate is a comparison against a constant and the optimiser removes the
//! body, the field expressions and the callsite string.

use crate::Keyhash;
use std::fmt;
use std::sync::OnceLock;
use std::time::Instant;

/// The first eight hex characters of a 32-byte identifier: enough to follow
/// one party through one bundle, and not the identifier.
#[must_use]
pub fn id8(k: &[u8; 32]) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// A count, as an event field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count(pub usize);

impl fmt::Display for Count {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A duration in milliseconds, as an event field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ms(pub u64);

impl fmt::Display for Ms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<std::time::Duration> for Ms {
    fn from(d: std::time::Duration) -> Ms {
        Ms(d.as_millis() as u64)
    }
}

/// A size in bytes, as an event field: how much went by, never what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bytes(pub usize);

impl fmt::Display for Bytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

static STARTED: OnceLock<Instant> = OnceLock::new();

/// Milliseconds since this process first asked, which a field-test
/// installer does at start. The renderer's `ms` field; the one wall-clock
/// anchor is the `diag.anchor` event the installer raises.
#[must_use]
pub fn since_start() -> Ms {
    Ms(STARTED.get_or_init(Instant::now).elapsed().as_millis() as u64)
}

/// The name of a topology object kind (`store::KIND_*`).
#[must_use]
pub fn kind_name(kind: u64) -> &'static str {
    use crate::store::*;
    match kind {
        KIND_TRANSACTION => "transaction",
        KIND_ENDPOINT_RECORD => "endpoint_record",
        KIND_DELEGATION => "delegation",
        KIND_SUBTREE_ACK => "subtree_ack",
        _ => "unknown",
    }
}

/// A store [`Decision`](crate::store::Decision): the variant, and the
/// reason string where it carries one. `Held` names the kind held and not
/// the bytes; `Conflict` names its subject by `id8`.
#[must_use]
pub fn decision(d: &crate::store::Decision) -> String {
    use crate::store::Decision::*;
    match d {
        Stored => "Stored".into(),
        Duplicate => "Duplicate".into(),
        OutOfStore => "OutOfStore".into(),
        Held(p) => format!("Held({})", kind_name(p.kind)),
        Conflict { subject, seqno } => format!(
            "Conflict({}, {}/{})",
            id8(subject),
            seqno.series,
            seqno.counter
        ),
        Refused(s) => format!("Refused({s})"),
        Malformed(s) => format!("Malformed({s})"),
    }
}

/// A [`MemoOutcome`](crate::propagation::MemoOutcome): the variant and the
/// parties it names, truncated.
#[must_use]
pub fn memo(m: &crate::propagation::MemoOutcome) -> String {
    use crate::propagation::MemoOutcome::*;
    match m {
        ForeignSubnet => "ForeignSubnet".into(),
        Forwarded { to } => format!("Forwarded({})", id8(to)),
        ForwardedAndDescended { to, down } => {
            format!("ForwardedAndDescended({}, {})", id8(to), id8(down))
        }
        StoppedAtRoot => "StoppedAtRoot".into(),
        StoppedAtRootAndDescended { down } => format!("StoppedAtRootAndDescended({})", id8(down)),
        AlreadyPassed => "AlreadyPassed".into(),
        CycleConfirmed { removed } => format!(
            "CycleConfirmed({})",
            removed.map(|k| id8(&k)).unwrap_or_default()
        ),
        Unconfirmed => "Unconfirmed".into(),
        ForAttachedClient { client } => format!("ForAttachedClient({})", id8(client)),
        HeldElsewhere { patron, slot, .. } => {
            format!("HeldElsewhere({}, slot {slot})", id8(patron))
        }
        Descended { to } => format!("Descended({})", id8(to)),
        DescentEnded => "DescentEnded".into(),
        Unroutable => "Unroutable".into(),
        Malformed(s) => format!("Malformed({s})"),
    }
}

/// A resolution [`Step`](crate::resolution::Step): the variant, the next
/// hop or the failure code, and never the endpoints.
#[must_use]
pub fn step(s: &crate::resolution::Step) -> String {
    use crate::resolution::Step::*;
    match s {
        Arrived(si) => format!("Arrived({})", id8(&si.node)),
        Continue(r) => format!("Continue({}, +{})", id8(&r.next), r.advances),
        Malformed(why) => format!("Malformed({why})"),
        Failed { code, disposition } => format!("Failed({code}, {disposition:?})"),
        WrongNonce => "WrongNonce".into(),
    }
}

/// A [`Replicated`](crate::peering::Replicated) item: the variant and a
/// size, never the object.
#[must_use]
pub fn replicated(r: &crate::peering::Replicated) -> String {
    use crate::peering::Replicated::*;
    match r {
        Topology { kind, object } => {
            format!("Topology({}, {} bytes)", kind_name(*kind), object.len())
        }
        TrustBearing { object } => format!("TrustBearing({} bytes)", object.len()),
        Reachability { client, reachable } => {
            format!("Reachability({}, {reachable})", id8(client))
        }
    }
}

/// A payload [`Delivery`](crate::peering::Delivery): who carried it, to
/// whom, truncated.
#[must_use]
pub fn delivery(d: &crate::peering::Delivery) -> String {
    use crate::peering::Delivery::*;
    match d {
        Direct { to } => format!("Direct({})", id8(to)),
        Relayed { via, to } => format!("Relayed(via {}, to {})", id8(via), id8(to)),
        Queued { at, to } => format!("Queued(at {}, to {})", id8(at), id8(to)),
    }
}

/// An [`AckTaken`](rhtn_archive::topology::AckTaken): the variant, with
/// the grandpatron a deferral waits on truncated.
#[must_use]
pub fn ack_taken(a: &rhtn_archive::topology::AckTaken) -> String {
    use rhtn_archive::topology::AckTaken::*;
    match a {
        Taken => "Taken".into(),
        NoOpenBinding => "NoOpenBinding".into(),
        Deferred(k) => format!("Deferred({})", id8(k)),
    }
}

/// A node's own position in an event: the one party there is to name.
#[must_use]
pub fn who(k: &Keyhash) -> String {
    id8(k)
}
