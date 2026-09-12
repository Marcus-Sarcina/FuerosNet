//! The operator's configuration: what a node cannot derive and must be told.
//!
//! Every field is something the crates beneath need and no document
//! supplies — where the identity lives, what the node serves on, and the
//! bounds that are the operator's to choose (`infra-client-requirements.md`
//! §1).  Nothing here is protocol: a field that could be derived from the
//! topology would be a second source of truth for it.
//!
//! The file format and its parser arrive with the lifecycle.

use rhtn_archive::Keyhash;
use rhtn_node::resolution::Ingestion;
use std::net::SocketAddr;
use std::path::PathBuf;

/// A node's configuration as its operator writes it.
///
/// There is no `Default`: a default listen address or queue cap would be a
/// policy choice made by omission, and the operator's obligations are
/// stated as choices (`infra-client-requirements.md` §1).
#[derive(Debug, Clone)]
pub struct Config {
    /// Where the node's signing identity is read from.  It is never
    /// generated on start: a node that mints a key when the file is
    /// missing serves under an identity nobody has adopted, and its
    /// operator would not know.
    pub identity: PathBuf,
    /// The address the node serves QUIC on and answers STUN Binding
    /// requests at (`infra-client-requirements.md` §7, design §14.1.1).
    pub listen: SocketAddr,
    /// The patron this node attaches to, and where to reach it.  Absent at
    /// a root, which attaches to nobody.
    pub upstream: Option<(Keyhash, Vec<SocketAddr>)>,
    /// Where the queue's directory store lives, so what was accepted
    /// survives a restart (`infra-client-requirements.md` §2).
    pub queue: PathBuf,
    /// The cap on one recipient's queue, above which the newest is refused
    /// and its sender told (`infra-client-requirements.md` §2, design
    /// §14.1.6).  Absent is uncapped.
    pub queue_cap: Option<usize>,
    /// The heartbeat interval this node advertises, in seconds
    /// (`wire-format.md` §8.2 fixes the range at 1 to 3600).
    pub heartbeat_secs: u64,
    /// How this node takes anchor entries it did not verify itself
    /// (`infra-client-requirements.md` §4.1).
    pub ingestion: Ingestion,
    /// How many requests one peer may make in a window, and how long the
    /// window is in seconds.  The values are the operator's.
    pub request_allowance: (u32, u64),
}
