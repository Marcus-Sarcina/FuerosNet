//! **The downline table** (design §15.2.2; `wire-format.md` §10.2.5): which
//! positions beneath this node are filled, and which of them run a node.
//!
//! **Positions and nobody's identity** [author, 2026-11-18]. A position is
//! the path of slot indices from this node down to it, and who stands at
//! one is not held here, at any depth. That is what lets the table be
//! required where the memo table, which holds occupants, is optional
//! [author, 2026-11-23]: design §12.6.1 refuses requiring a map of who
//! lives beneath a node, and this holds where and not who.
//!
//! **Filled from what the node already receives.** Within its horizon,
//! from the bindings its own table settled on, which are signed; deeper,
//! from the memos that pass through it in either direction, which are
//! hints (`wire-format.md` §10.2.3). Nothing is cut, refused or disavowed
//! on this table's word. What reads it is how far a board post travels
//! (design §14.4), where a table reading shallow lowers a writer's reach
//! and a fabricated memo lengthens its fabricator's own.
//!
//! **It is the node's own state, and not the board's** [author,
//! 2026-11-23]: the board is the first thing to read it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The slot indices from this node down to a position beneath it. The
/// empty path is the node itself.
pub type Position = Vec<u8>;

/// What everyone may post upward (design §14.4) [author, 2026-11-18].
pub const BASE_REACH: usize = 2;

/// The path bound (`wire-format.md` §1.3), which no reach exceeds.
pub const MAX_REACH: usize = 24;

/// design §3.3's L: no position lies more than this many levels below a
/// node.
const LIGHT_LEVELS: usize = 2;

/// One slot as the table holds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    /// Whether anyone stands there. An emptied slot stays as a row, so
    /// that an earlier memo arriving late reinstates nothing
    /// (`wire-format.md` §10.2.2).
    filled: bool,
    /// The timestamp of the transaction that last set it.
    stamp: u64,
    /// Whether whoever stands there runs a node.
    node: bool,
}

/// The table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Downline {
    slots: BTreeMap<Position, Entry>,
}

impl Downline {
    /// An empty table: a node with nobody beneath it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Fill or empty the slot `slot` of the position `patron`, as of
    /// `stamp`. Ordering is `wire-format.md` §10.2.2's: a later timestamp
    /// replaces, an equal one keeps what arrived first. Emptying removes
    /// every position beneath the slot, the subtree having left with its
    /// head. Says whether the table changed.
    pub fn set(&mut self, patron: &[u8], slot: u8, filled: bool, stamp: u64) -> bool {
        if slot > 9 || patron.len() >= MAX_REACH || patron.iter().any(|s| *s > 9) {
            return false;
        }
        // a position beneath an emptied one is not added: its head left,
        // and it with it
        if filled && self.under_emptied(patron) {
            return false;
        }
        let mut at = patron.to_vec();
        at.push(slot);
        match self.slots.get(&at) {
            Some(held) if held.stamp >= stamp => return false,
            Some(held) if held.filled == filled => {
                // the same statement, later: the stamp moves and nothing else
                let node = held.node;
                self.slots.insert(
                    at,
                    Entry {
                        filled,
                        stamp,
                        node,
                    },
                );
                return false;
            }
            _ => {}
        }
        if !filled {
            self.remove_beneath(&at);
        }
        self.slots.insert(
            at,
            Entry {
                filled,
                stamp,
                node: false,
            },
        );
        true
    }

    /// **The same slot on a signed record's word**: what this node's own
    /// table settled replaces what is held where the two differ, a memo's
    /// ordering having no say over it.  A slot already as stated is left
    /// as it stands, with whatever is held beneath it.
    pub fn settle(&mut self, patron: &[u8], slot: u8, filled: bool, stamp: u64) -> bool {
        if slot > 9 || patron.len() >= MAX_REACH || patron.iter().any(|s| *s > 9) {
            return false;
        }
        let mut at = patron.to_vec();
        at.push(slot);
        let held = self.slots.get(&at).copied();
        if held.is_some_and(|e| e.filled == filled) {
            return false;
        }
        if !filled {
            self.remove_beneath(&at);
        }
        self.slots.insert(
            at,
            Entry {
                filled,
                stamp: stamp.max(held.map_or(0, |e| e.stamp)),
                node: false,
            },
        );
        true
    }

    /// Mark the position as running a node, or as not. Nothing where
    /// nobody stands there. Says whether the table changed.
    pub fn mark_node(&mut self, at: &[u8], runs: bool) -> bool {
        match self.slots.get_mut(at) {
            Some(e) if e.filled && e.node != runs => {
                e.node = runs;
                true
            }
            _ => false,
        }
    }

    /// Whether anyone stands at the position. The node itself always does.
    #[must_use]
    pub fn filled(&self, at: &[u8]) -> bool {
        at.is_empty() || self.slots.get(at).is_some_and(|e| e.filled)
    }

    /// Whether the position is marked as running a node. The node itself is.
    #[must_use]
    pub fn runs_node(&self, at: &[u8]) -> bool {
        at.is_empty() || self.slots.get(at).is_some_and(|e| e.filled && e.node)
    }

    /// Every filled position beneath `at`, with whether it runs a node.
    pub fn beneath<'a>(&'a self, at: &'a [u8]) -> impl Iterator<Item = (&'a [u8], bool)> + 'a {
        self.slots
            .range(at.to_vec()..)
            .take_while(move |(p, _)| p.starts_with(at))
            .filter(move |(p, e)| e.filled && p.len() > at.len())
            .map(|(p, e)| (p.as_slice(), e.node))
    }

    /// How many filled positions the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.values().filter(|e| e.filled).count()
    }

    /// Whether nobody stands beneath this node.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// How many levels the deepest filled position stands beneath `at`.
    #[must_use]
    pub fn depth(&self, at: &[u8]) -> usize {
        self.beneath(at)
            .map(|(p, _)| p.len() - at.len())
            .max()
            .unwrap_or(0)
    }

    /// **The depth of the node-running downline beneath `at`** (design
    /// §15.2.2): the deepest position marked as running a node, and beyond
    /// what is marked, what design §3.3 implies. No position lies more
    /// than two levels below a node, so a position three or more levels
    /// down stands for a node no more than two levels above it. A floor,
    /// never over the true figure where §3.3 holds.
    #[must_use]
    pub fn node_depth(&self, at: &[u8]) -> usize {
        let marked = self
            .beneath(at)
            .filter(|(_, node)| *node)
            .map(|(p, _)| p.len() - at.len())
            .max()
            .unwrap_or(0);
        let deepest = self.depth(at);
        let implied = deepest.saturating_sub(LIGHT_LEVELS);
        marked.max(implied)
    }

    /// **How far up whoever stands at `at` may post** (design §14.4)
    /// [author, 2026-11-18, 2026-11-23]: two, plus the depth of their
    /// node-running downline.
    #[must_use]
    pub fn reach(&self, at: &[u8]) -> usize {
        (BASE_REACH + self.node_depth(at)).min(MAX_REACH)
    }

    /// Whether this node serves the position (design §12.6.1): its own,
    /// and every filled position beneath it with no node at or above it
    /// short of this one.
    #[must_use]
    pub fn serves(&self, at: &[u8]) -> bool {
        self.filled(at) && (1..=at.len()).all(|n| !self.runs_node(&at[..n]))
    }

    /// **The nearest downline nodes**: each position running a node with
    /// none between it and this one. They stand one, two or three levels
    /// down (design §3.3).
    #[must_use]
    pub fn nearest_nodes(&self) -> Vec<Position> {
        self.beneath(&[])
            .filter(|(p, node)| *node && (1..p.len()).all(|n| !self.runs_node(&p[..n])))
            .map(|(p, _)| p.to_vec())
            .collect()
    }

    /// **The three lists a post is forwarded by** (design §14.4) [author,
    /// 2026-11-18]: the nearest downline nodes within one level, within
    /// two and within three, the third serving every count above it.
    /// Worked out when the downline changes, not per post.
    #[must_use]
    pub fn lists(&self) -> [Vec<Position>; 3] {
        let all = self.nearest_nodes();
        let within =
            |k: usize| -> Vec<Position> { all.iter().filter(|p| p.len() <= k).cloned().collect() };
        let (one, two) = (within(1), within(2));
        [one, two, all]
    }

    /// The table as it goes to disk: one line a slot, the path as its
    /// digits, `f` filled, `n` filled and running a node, `e` emptied, and
    /// the timestamp that set it.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        for (p, e) in &self.slots {
            let path: String = p.iter().map(|s| char::from(b'0' + *s)).collect();
            let what = match (e.filled, e.node) {
                (true, true) => 'n',
                (true, false) => 'f',
                (false, _) => 'e',
            };
            let _ = writeln!(out, "{path} {what} {}", e.stamp);
        }
        out
    }

    /// The table from what [`Downline::encode`] wrote. A file that does
    /// not read is refused whole, and the table is rebuilt from the store
    /// and the memos to come: it reads shallow until then, which is the
    /// direction it is allowed to be wrong in.
    #[must_use]
    pub fn decode(s: &str) -> Option<Self> {
        let mut slots = BTreeMap::new();
        for line in s.lines() {
            let mut parts = line.split(' ');
            let (path, what, stamp) = (parts.next()?, parts.next()?, parts.next()?);
            if parts.next().is_some() || path.is_empty() || path.len() > MAX_REACH {
                return None;
            }
            let at: Position = path
                .bytes()
                .map(|b| b.checked_sub(b'0').filter(|d| *d <= 9))
                .collect::<Option<_>>()?;
            let (filled, node) = match what {
                "n" => (true, true),
                "f" => (true, false),
                "e" => (false, false),
                _ => return None,
            };
            let stamp = stamp.parse().ok()?;
            slots.insert(
                at,
                Entry {
                    filled,
                    stamp,
                    node,
                },
            );
        }
        Some(Self { slots })
    }

    /// Whether any position at or above `at` was emptied.
    fn under_emptied(&self, at: &[u8]) -> bool {
        (1..=at.len()).any(|n| self.slots.get(&at[..n]).is_some_and(|e| !e.filled))
    }

    /// Remove every row beneath `at`, `at` itself excepted.
    fn remove_beneath(&mut self, at: &[u8]) {
        let gone: Vec<Position> = self
            .slots
            .range(at.to_vec()..)
            .take_while(|(p, _)| p.starts_with(at))
            .filter(|(p, _)| p.len() > at.len())
            .map(|(p, _)| p.clone())
            .collect();
        for p in gone {
            self.slots.remove(&p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chain of filled positions down slot 3, `depth` levels.
    fn chain(depth: usize) -> Downline {
        let mut d = Downline::new();
        for n in 0..depth {
            assert!(d.set(&vec![3; n], 3, true, 10));
        }
        d
    }

    #[test]
    fn a_filling_memo_adds_the_position_and_an_emptying_one_removes_its_subtree() {
        let mut d = Downline::new();
        assert!(d.set(&[], 3, true, 10));
        assert!(d.set(&[3], 5, true, 11));
        assert!(d.set(&[3, 5], 9, true, 12));
        assert!(d.set(&[], 4, true, 10));
        assert_eq!(d.len(), 4);
        assert_eq!(d.depth(&[]), 3);
        // the sub in slot 3 leaves, and everything beneath it with them
        assert!(d.set(&[], 3, false, 20));
        assert_eq!(d.len(), 1);
        assert!(!d.filled(&[3]));
        assert!(!d.filled(&[3, 5, 9]));
        assert!(d.filled(&[4]));
        assert_eq!(d.depth(&[]), 1);
    }

    #[test]
    fn an_earlier_memo_arriving_late_reinstates_nothing() {
        let mut d = Downline::new();
        assert!(d.set(&[], 3, true, 10));
        assert!(d.set(&[], 3, false, 20));
        // the fill it was emptied after, arriving late
        assert!(!d.set(&[], 3, true, 10));
        assert!(!d.filled(&[3]));
        // and a late memo from the subtree that left with it
        assert!(!d.set(&[3], 5, true, 15));
        assert!(!d.filled(&[3, 5]));
        // an equal timestamp keeps what arrived first
        assert!(!d.set(&[], 3, true, 20));
        assert!(!d.filled(&[3]));
        // a later adoption into the slot fills it again
        assert!(d.set(&[], 3, true, 30));
        assert!(d.filled(&[3]));
        assert!(d.set(&[3], 5, true, 31));
    }

    #[test]
    fn a_signed_record_settles_a_slot_whatever_a_memo_said_of_it() {
        let mut d = Downline::new();
        d.set(&[], 3, true, 10);
        d.set(&[3], 5, true, 11);
        // already as stated: nothing moves, and what is beneath stays
        assert!(!d.settle(&[], 3, true, 99));
        assert!(d.filled(&[3, 5]));
        // a hint dated ahead emptied it; the record says it is held
        d.set(&[], 3, false, 500);
        assert!(d.settle(&[], 3, true, 12));
        assert!(d.filled(&[3]));
        // and the record's ending takes the subtree, as a memo's does
        d.set(&[3], 5, true, 501);
        assert!(d.settle(&[], 3, false, 20));
        assert!(!d.filled(&[3]) && !d.filled(&[3, 5]));
    }

    #[test]
    fn a_slot_out_of_range_or_a_path_over_the_bound_changes_nothing() {
        let mut d = Downline::new();
        assert!(!d.set(&[], 10, true, 1));
        assert!(!d.set(&[12], 1, true, 1));
        assert!(!d.set(&[1; MAX_REACH], 1, true, 1));
        assert!(d.is_empty());
    }

    #[test]
    fn a_position_reached_through_ones_never_heard_of_still_counts() {
        // a subtree carried in by a re-adoption sends no memo until it
        // changes, and the first memo names a slot four levels down
        let mut d = Downline::new();
        assert!(d.set(&[2, 7, 1], 4, true, 10));
        assert_eq!(d.depth(&[]), 4);
        assert_eq!(d.depth(&[2]), 3);
        assert_eq!(d.depth(&[8]), 0);
    }

    #[test]
    fn who_runs_a_node_is_exact_near_and_a_floor_beyond() {
        // light members only, one and two levels down: no node beneath
        assert_eq!(chain(1).node_depth(&[]), 0);
        assert_eq!(chain(2).node_depth(&[]), 0);
        // a marked node at each of one, two and three levels
        for at in 1..=3 {
            let mut d = chain(at);
            assert!(d.mark_node(&vec![3; at], true));
            assert_eq!(d.node_depth(&[]), at);
        }
        // memos alone, nothing marked: design §3.3 puts a node no more
        // than two levels above the deepest position
        assert_eq!(chain(3).node_depth(&[]), 1);
        assert_eq!(chain(4).node_depth(&[]), 2);
        assert_eq!(chain(7).node_depth(&[]), 5);
        // and the same from a member's own position
        assert_eq!(chain(7).node_depth(&[3, 3]), 3);
        // a mark deeper than the floor wins
        let mut d = chain(4);
        assert!(d.mark_node(&[3, 3, 3, 3], true));
        assert_eq!(d.node_depth(&[]), 4);
    }

    #[test]
    fn everyone_may_post_two_up_and_a_node_running_downline_adds_its_depth() {
        assert_eq!(Downline::new().reach(&[]), 2);
        assert_eq!(chain(2).reach(&[]), 2);
        let mut d = chain(2);
        d.mark_node(&[3, 3], true);
        assert_eq!(d.reach(&[]), 4);
        assert_eq!(d.reach(&[3]), 3);
        assert_eq!(d.reach(&[3, 3]), 2);
        // a mark on nobody is no mark
        assert!(!d.mark_node(&[9], true));
        // and no reach passes the path bound
        assert_eq!(chain(MAX_REACH).reach(&[]), MAX_REACH);
    }

    #[test]
    fn a_node_serves_its_own_position_and_the_light_ones_beneath_it() {
        let mut d = Downline::new();
        d.set(&[], 1, true, 1);
        d.set(&[1], 2, true, 1);
        d.set(&[], 4, true, 1);
        d.set(&[4], 5, true, 1);
        d.mark_node(&[4], true);
        assert!(d.serves(&[]));
        assert!(d.serves(&[1]));
        assert!(d.serves(&[1, 2]));
        // a subordinate running a node serves itself and its own
        assert!(!d.serves(&[4]));
        assert!(!d.serves(&[4, 5]));
        // and nobody is served where nobody stands
        assert!(!d.serves(&[7]));
    }

    #[test]
    fn the_three_lists_are_the_nearest_nodes_within_one_two_and_three_levels() {
        let mut d = Downline::new();
        // a node one level down, with a node beneath it that is not nearest
        d.set(&[], 0, true, 1);
        d.set(&[0], 0, true, 1);
        d.mark_node(&[0], true);
        d.mark_node(&[0, 0], true);
        // a node two levels down, under a light member
        d.set(&[], 1, true, 1);
        d.set(&[1], 1, true, 1);
        d.mark_node(&[1, 1], true);
        // a node three levels down, under two light members
        d.set(&[], 2, true, 1);
        d.set(&[2], 2, true, 1);
        d.set(&[2, 2], 2, true, 1);
        d.mark_node(&[2, 2, 2], true);
        let [one, two, three] = d.lists();
        assert_eq!(one, vec![vec![0]]);
        assert_eq!(two, vec![vec![0], vec![1, 1]]);
        assert_eq!(three, vec![vec![0], vec![1, 1], vec![2, 2, 2]]);
        // slot 0 is a real slot, and a path of zeros a real position
        assert!(d.runs_node(&[0, 0]));
        // a node closed leaves the lists at the next working out
        assert!(d.mark_node(&[1, 1], false));
        assert_eq!(d.lists()[2], vec![vec![0], vec![2, 2, 2]]);
        // and one that left takes its mark with it
        d.set(&[], 0, false, 2);
        assert_eq!(d.lists()[2], vec![vec![2, 2, 2]]);
    }

    #[test]
    fn the_table_comes_back_from_disk_as_it_went() {
        let mut d = Downline::new();
        d.set(&[], 0, true, 5);
        d.set(&[0], 9, true, 6);
        d.mark_node(&[0], true);
        d.set(&[], 3, true, 7);
        d.set(&[], 3, false, 8);
        let text = d.encode();
        assert_eq!(text, "0 n 5\n09 f 6\n3 e 8\n");
        assert_eq!(Downline::decode(&text), Some(d));
        // what does not read is refused whole
        assert_eq!(Downline::decode("0 n 5\n0x f 6\n"), None);
        assert_eq!(Downline::decode("0 q 5\n"), None);
        assert_eq!(Downline::decode(" f 5\n"), None);
        assert_eq!(Downline::decode("0 f 5 6\n"), None);
        assert_eq!(Downline::decode(""), Some(Downline::new()));
    }

    #[test]
    fn nothing_in_the_table_is_an_identity() {
        // what goes to disk is paths, a letter and a time: no field could
        // hold a keyhash (design §15.2.2)
        let mut d = chain(5);
        d.mark_node(&[3, 3, 3], true);
        for line in d.encode().lines() {
            let parts: Vec<&str> = line.split(' ').collect();
            assert_eq!(parts.len(), 3);
            assert!(parts[0].len() <= MAX_REACH && parts[0].bytes().all(|b| b.is_ascii_digit()));
            assert!(matches!(parts[1], "f" | "n" | "e"));
            assert!(parts[2].parse::<u64>().is_ok());
        }
    }
}
