//! **The local map** (design §15.2.3): what this node's lookups show of
//! the rest of its subnet, beside the downline table and in its shape.
//!
//! **Passive** [author, 2026-11-27]. Nothing is asked for it: the replies
//! to the resolutions this node drives for the clients it serves
//! (`wire-format.md` §7.7.1) are read on their way back, and that is all
//! it is built from. A referral says a node runs where its next hop
//! stands; a serving answer says one runs at the serving node's position
//! and none between there and the subject, the serving node being the
//! subject's nearest infrastructure ancestor (design §12.6.1). A failure
//! places nothing, its code not saying at which index the path stopped.
//!
//! **Assumed to have blind spots and to hold what is no longer so**
//! [author, 2026-11-27]: it sees only the paths looked up through it, as
//! they stood then, on the word of unsigned replies. **And resolution does
//! not read it** [author, 2026-11-27]: nothing in [`crate::resolution`]
//! refers to it, and an answer this node gives is the same with or
//! without it.
//!
//! **Positions and nobody's identity**, as in the downline table, and only
//! in this node's own subnet: a position is the path of slot indices from
//! the subnet's anchor, and a lookup under any other anchor is not kept.
//! None of it lies beneath this node, whose downline a lookup never comes
//! back from: one through this node is answered from its own records and
//! followed onward by the client.

use crate::Keyhash;
use crate::downline::{Downline, NODE};
use crate::resolution::{ResolveReply, ResolveRequest};
use crate::view::NodeView;
use std::fmt::Write as _;

/// The map: the subnet it is of, and what has been seen there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalMap {
    /// The anchor the positions are read from. A node that comes to stand
    /// under another starts the map again, the old one being of somewhere
    /// it no longer is.
    anchor: Option<Keyhash>,
    /// The positions, from the anchor down: filled where one was seen
    /// filled, and node actual where one was seen running.
    seen: Downline,
}

impl LocalMap {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The anchor the map is read from, once anything is in it.
    #[must_use]
    pub fn anchor(&self) -> Option<&Keyhash> {
        self.anchor.as_ref()
    }

    /// What has been seen, as positions from the anchor: whether one was
    /// seen filled ([`Downline::filled`]) and seen running a node
    /// ([`Downline::runs_node`]).
    #[must_use]
    pub fn seen(&self) -> &Downline {
        &self.seen
    }

    /// **Read one reply to a lookup under `anchor` for `path`**, the hop
    /// that gave it standing `consumed` indices down, as of `now`. Says
    /// whether the map changed.
    pub fn note(
        &mut self,
        anchor: &Keyhash,
        path: &[u8],
        consumed: usize,
        reply: &ResolveReply,
        now: u64,
    ) -> bool {
        if self.anchor.as_ref() != Some(anchor) {
            *self = Self {
                anchor: Some(*anchor),
                seen: Downline::new(),
            };
        }
        match reply {
            ResolveReply::Referral { referral, .. } => {
                let to = usize::try_from(referral.advances)
                    .ok()
                    .and_then(|k| consumed.checked_add(k))
                    .filter(|to| *to >= 1 && *to <= path.len());
                let Some(to) = to else { return false };
                let mut changed = self.fill(&path[..to], now);
                changed |= self.seen.set(&path[..to], NODE, true, now);
                changed
            }
            ResolveReply::Serving { serving, .. } => {
                let Some(at) = path.len().checked_sub(serving.residual.len()) else {
                    return false;
                };
                let mut changed = self.fill(path, now);
                if at >= 1 {
                    changed |= self.seen.set(&path[..at], NODE, true, now);
                }
                // nobody between the serving node and the subject runs one
                for n in at + 1..=path.len() {
                    changed |= self.seen.set(&path[..n], NODE, false, now);
                }
                changed
            }
            ResolveReply::Failure { .. } => false,
        }
    }

    /// Every position from the anchor down to `to`, seen filled.
    fn fill(&mut self, to: &[u8], now: u64) -> bool {
        let mut changed = false;
        for n in 1..=to.len() {
            changed |= self.seen.set(&to[..n - 1], to[n - 1], true, now);
        }
        changed
    }

    /// The map as it goes to disk: the anchor's keyhash in hex on the first
    /// line, and the positions beneath it as [`Downline::encode`] writes
    /// them.
    #[must_use]
    pub fn encode(&self) -> String {
        let Some(anchor) = &self.anchor else {
            return String::new();
        };
        let mut out = String::new();
        for b in anchor {
            let _ = write!(out, "{b:02x}");
        }
        out.push('\n');
        out.push_str(&self.seen.encode());
        out
    }

    /// The map from what [`LocalMap::encode`] wrote. A file that does not
    /// read is refused whole, and the map starts again from the lookups to
    /// come.
    #[must_use]
    pub fn decode(s: &str) -> Option<Self> {
        let Some((first, rest)) = s.split_once('\n') else {
            return s.is_empty().then(Self::new);
        };
        if first.len() != 64 {
            return None;
        }
        let mut anchor = [0u8; 32];
        for (i, b) in anchor.iter_mut().enumerate() {
            *b = u8::from_str_radix(first.get(2 * i..2 * i + 2)?, 16).ok()?;
        }
        Some(Self {
            anchor: Some(anchor),
            seen: Downline::decode(rest)?,
        })
    }
}

impl NodeView {
    /// **What a lookup this node drove showed** (design §15.2.3): each reply
    /// with the indices consumed before it, applied to the local map where
    /// the lookup was in this node's own subnet.
    pub fn note_lookup(&mut self, req: &ResolveRequest, heard: &[(usize, ResolveReply)]) {
        if req.anchor != self.position.anchor {
            return;
        }
        let path = req.path().indices();
        let now = self.now();
        for (consumed, reply) in heard {
            self.local_map
                .note(&req.anchor, &path, *consumed, reply, now);
        }
    }
}
