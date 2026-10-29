//! **The nouns, once** (`Robot/implementation-plan.md` section 8,
//! [author, 2026-10-29]: one model across the three clients).
//!
//! Every client of this protocol — the kernel a light client runs on, the
//! FFI that carries it to a phone, the desktop, and the node where a noun
//! is shared — reads these and no copy of its own. What is here is the
//! read model: **full identities** (a keyhash is thirty-two bytes, never
//! eight characters of hex), **wire-level enums** (a contact's `How` is
//! the wire's number, never a sentence), and **no screen's name on a
//! type**. What a screen shows for one of these is the screen's pure
//! part's business, where a test can hold it.
//!
//! **Nothing here has behaviour.** A noun is a value; what computes one
//! lives in the kernel, what draws one lives above the FFI.
//!
//! Under the `ffi` feature each type derives its binding, so the Kotlin
//! records are generated from these definitions and the shell holds none.

#![forbid(unsafe_code)]

/// Thirty-two bytes: a keyhash or a transaction id. Across the FFI it is
/// the bytes themselves, never hex.
pub type Hash32 = [u8; 32];

/// A participant's identity, by its keyhash.
pub type Keyhash = Hash32;

/// A transaction's id; a subnet is named by one (design §13.8).
pub type Txid = Hash32;

#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();

#[cfg(feature = "ffi")]
uniffi::custom_type!(Hash32, Vec<u8>, {
    remote,
    lower: |h| h.to_vec(),
    try_lift: |v| {
        let n = v.len();
        v.try_into()
            .map_err(|_| uniffi::deps::anyhow::anyhow!("an id is 32 bytes; that is {n}"))
    },
});

/// **How a contact came to be held** (`light-client-requirements.md`
/// §4.3's capture list), as the wire numbers it. What each is called is
/// the screen's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
#[repr(u8)]
pub enum How {
    Counterparty = 0,
    Introduced = 1,
    Nominee = 2,
    Verifier = 3,
    Infra = 4,
    Declined = 5,
    Provisioned = 6,
}

impl How {
    /// The wire's number back as a `How`, or none where it is not one.
    #[must_use]
    pub fn of(n: u64) -> Option<How> {
        Some(match n {
            0 => How::Counterparty,
            1 => How::Introduced,
            2 => How::Nominee,
            3 => How::Verifier,
            4 => How::Infra,
            5 => How::Declined,
            6 => How::Provisioned,
            _ => return None,
        })
    }
}

/// **A party this client holds** (`light-client-requirements.md` §4.3):
/// who, what they published for the subnet their locator names, how they
/// came to be held, and what is known of reaching them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Contact {
    pub keyhash: Keyhash,
    /// The alias they published, or none: the keyhash stands in, and a
    /// name that has not been received is never shown.
    pub alias: Option<String>,
    pub how: How,
    /// Whether a locator of theirs is held.
    pub located: bool,
    /// Whether an address of theirs is held, for the direct path.
    pub addressed: bool,
    /// How many times they have been met or heard from.
    pub count: u64,
    /// When last, seconds since the epoch.
    pub last_at: u64,
}

/// **What this key is in a subnet** — its rank there, in the order the
/// crowns are drawn: root above operator above patron — each off a record
/// it holds [author, 2026-10-28]: the patron of the anchoring record, an
/// endpoint record on its line there, an adoption it granted there. *Rank*
/// and not *standing*, which the design uses for a party's score under a
/// policy [2026-10-29].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Rank {
    pub root: bool,
    pub operator: bool,
    /// The direct lines this key issued here and no deeper figure
    /// [author, 2026-10-28].
    pub subordinates: u32,
}

/// **Where a key sits in a subnet** (`wire-format.md` §2.1): the anchor,
/// the path below it, and how many nibbles of it are its own.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Position {
    pub anchor: Keyhash,
    pub path: Vec<u8>,
    pub nibbles: u64,
}

/// **A subnet this client holds a position in** (design §13.8): named by
/// its id, called by the name its root published, this client's own name
/// there, what this client is in it, and where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Subnet {
    pub id: Txid,
    /// The subnet's alias, where its profile is held.
    pub alias: Option<String>,
    /// This client's own alias there, where it published one.
    pub own_alias: Option<String>,
    pub rank: Rank,
    /// None for the root of a subnet whose own position is the self-anchor.
    pub position: Option<Position>,
}

/// **Where another node sits**, as this client can resolve it
/// (`light-client-requirements.md` §4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Placed {
    pub node: Keyhash,
    pub position: Position,
}

/// **A line of authority this key holds** (`light-client-requirements.md`
/// §6; an adoption on the wire): under whom, on which series, and where
/// that puts this key.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Line {
    pub patron: Keyhash,
    pub series: u32,
    pub anchor: Keyhash,
    pub nibbles: u64,
}

/// **An infra node this client can reach** (`light-client-requirements.md`
/// §10, §4): a key and where it answers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Instance {
    pub keyhash: Keyhash,
    pub addresses: Vec<String>,
}

/// **A resource as a catalog served it** (`wire-format.md` §6.4; design
/// §11): what it is, whose it is, which node served the entry, and
/// whether that node's portion was whole.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Resource {
    /// The node that served the entry, which is the node asked for it.
    pub node: Keyhash,
    pub resource: Keyhash,
    pub owner: Keyhash,
    pub service_type: String,
    pub instance: String,
    pub endpoint: Vec<u8>,
    pub data_practice: Option<u64>,
    /// The portion it came in was cut short.
    pub truncated: bool,
    /// The portion it came in is older than the node's hint.
    pub stale: bool,
}

/// **A profile** (`wire-format.md` §7.11): a subject's own for a subnet
/// (kind 0), or the subnet's (kind 1), as published.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Profile {
    pub subject: Keyhash,
    pub subnet: Txid,
    pub kind: u64,
    pub alias: String,
    pub image_format: Option<u64>,
    pub image: Option<Vec<u8>>,
    pub published_at: u64,
}

/// Why a resource goes with a departure (`light-client-requirements.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LossReason {
    /// The node serving it is on the line being ended.
    HostLeaves,
    /// Its owner is on the line being ended.
    OwnerLeaves,
}

/// A resource that goes with a departure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Loss {
    pub resource: Keyhash,
    pub node: Keyhash,
    pub owner: Keyhash,
    pub service_type: String,
    pub instance: String,
    pub reason: LossReason,
}

/// **Why a cost is a floor, not a total** (`light-client-requirements.md`
/// §6: a stale list identified as stale, never silently used).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Limit {
    /// A predicate no client holds, so a resource may be lost unseen.
    PredicateNotHeld,
    /// A node's portion is stale.
    Stale { node: Keyhash },
    /// A node's portion was cut short.
    Truncated { node: Keyhash },
    /// A node in the horizon never swept.
    Unswept { node: Keyhash },
    /// This key holds several lines; the cost is one of them.
    SeveralLines { lines: u32 },
    /// This key's own adoption has not propagated back to it.
    NotInTable,
}

/// **What leaving costs** (`light-client-requirements.md` §6): what goes,
/// what stays, who leaves the horizon, and every reason the list is a
/// floor.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Cost {
    pub losing: Vec<Loss>,
    pub keeping: u32,
    pub leaving: Vec<Keyhash>,
    pub limits: Vec<Limit>,
}

impl Cost {
    /// Whether anything at all is lost, which is what decides whether
    /// there is a warning to show.
    #[must_use]
    pub fn is_free(&self) -> bool {
        self.losing.is_empty() && self.leaving.is_empty()
    }
}

/// **The connection to the serving node** (`light-client-requirements.md`
/// §4; UX-012): detached, attached and to whom, reconnecting, lost and why.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Connection {
    Detached,
    Attached { serving: Keyhash, primary: bool },
    Reconnecting { from: Keyhash },
    Lost { from: Keyhash, reason: String },
}

/// **Which noun changed**, for a client keeping a copy in step with the
/// kernel [author, 2026-10-29: update logic that keeps them in sync].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Noun {
    Contacts,
    Subnets,
    Lines,
    Instances,
    Resources,
    Profiles,
    Connection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn how_round_trips_the_wires_numbers_and_refuses_the_rest() {
        for n in 0..7 {
            assert_eq!(How::of(n).map(|h| h as u64), Some(n));
        }
        assert_eq!(How::of(7), None);
    }
}
