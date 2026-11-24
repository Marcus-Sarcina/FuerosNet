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
//! **As the author sketched it** [2026-11-18]: `3[0, 2, 5, n]` is the
//! subordinate in slot 3, with subordinates of their own in slots 0, 2 and
//! 5, who runs a node; `3,5,9[]` a great-grandsubordinate with nobody
//! beneath them and no node. The digits are filled slots and `n` is *node
//! actual*: the member is served from a node of their own. Here each of
//! those is a row keyed by its path, the `n` among them.
//!
//! **Filled from what the node already receives.** Within its horizon,
//! from the bindings its own table settled on, which are signed; deeper,
//! from the memos that pass through it in either direction, which are
//! hints (`wire-format.md` §10.2.3) and say both who fills a slot and who
//! runs a node [author, 2026-11-23]. Nothing is cut, refused or disavowed
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

/// **Node actual**: the row beneath a position that says its member is
/// served from a node of their own. Outside 0 to 9, slot 0 being a real
/// slot [author, 2026-11-18], and what `wire-format.md` §10.2 carries in a
/// memo's slot field for the same statement.
pub const NODE: u8 = 10;

/// One row as the table holds it: a slot, or a position's node actual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Entry {
    /// Whether the statement holds. One withdrawn stays as a row, so that
    /// an earlier memo arriving late reinstates nothing (`wire-format.md`
    /// §10.2.2).
    filled: bool,
    /// The timestamp of what last set it.
    stamp: u64,
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
    /// `stamp`; with [`NODE`] for the slot, say that the position's own
    /// member runs a node or has stopped. Ordering is `wire-format.md`
    /// §10.2.2's: a later timestamp replaces, an equal one keeps what
    /// arrived first. Emptying a slot removes every row beneath it, the
    /// subtree having left with its head. Says whether the table changed.
    pub fn set(&mut self, patron: &[u8], slot: u8, filled: bool, stamp: u64) -> bool {
        if !Self::names_a_row(patron, slot) {
            return false;
        }
        // a row beneath an emptied position is not added: its head left,
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
                self.slots.insert(at, Entry { filled, stamp });
                return false;
            }
            _ => {}
        }
        if !filled {
            self.remove_beneath(&at);
        }
        self.slots.insert(at, Entry { filled, stamp });
        // a node runs where somebody stands: one heard of before its
        // position was is a position all the same
        if filled && slot == NODE && !self.filled(patron) {
            self.slots.insert(
                patron.to_vec(),
                Entry {
                    filled: true,
                    stamp: 0,
                },
            );
        }
        true
    }

    /// Whether `slot` of `patron` is a row this table can hold: a slot 0
    /// to 9 or a node actual, of a position within the path bound. The
    /// node itself has no node-actual row, being one by definition.
    fn names_a_row(patron: &[u8], slot: u8) -> bool {
        let fits = match slot {
            0..=9 => patron.len() < MAX_REACH,
            NODE => !patron.is_empty() && patron.len() <= MAX_REACH,
            _ => false,
        };
        fits && patron.iter().all(|s| *s <= 9)
    }

    /// **The same slot on a signed record's word**: what this node's own
    /// table settled replaces what is held where the two differ, a memo's
    /// ordering having no say over it.  A slot already as stated is left
    /// as it stands, with whatever is held beneath it.
    pub fn settle(&mut self, patron: &[u8], slot: u8, filled: bool, stamp: u64) -> bool {
        if !Self::names_a_row(patron, slot) {
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
            },
        );
        true
    }

    /// Mark the position as running a node, or as not, on this node's own
    /// records. Nothing where nobody stands there. Says whether the table
    /// changed.
    pub fn mark_node(&mut self, at: &[u8], runs: bool) -> bool {
        !at.is_empty() && self.filled(at) && self.settle(at, NODE, runs, 0)
    }

    /// Whether anyone stands at the position. The node itself always does.
    #[must_use]
    pub fn filled(&self, at: &[u8]) -> bool {
        at.is_empty() || (at.last() != Some(&NODE) && self.slots.get(at).is_some_and(|e| e.filled))
    }

    /// Whether the position is marked as running a node. The node itself is.
    #[must_use]
    pub fn runs_node(&self, at: &[u8]) -> bool {
        if at.is_empty() {
            return true;
        }
        let mut row = at.to_vec();
        row.push(NODE);
        self.filled(at) && self.slots.get(&row).is_some_and(|e| e.filled)
    }

    /// Every filled position beneath `at`, with whether it runs a node.
    pub fn beneath<'a>(&'a self, at: &'a [u8]) -> impl Iterator<Item = (&'a [u8], bool)> + 'a {
        self.slots
            .range(at.to_vec()..)
            .take_while(move |(p, _)| p.starts_with(at))
            .filter(move |(p, e)| e.filled && p.len() > at.len() && p.last() != Some(&NODE))
            .map(move |(p, _)| (p.as_slice(), self.runs_node(p)))
    }

    /// How many filled positions the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.beneath(&[]).count()
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
    /// §15.2.2): how many levels down the deepest position marked node
    /// actual stands. A node says so of itself by memo, as a patron says
    /// who fills a slot [author, 2026-11-23], so the mark is there at any
    /// depth a memo has come up from.
    #[must_use]
    pub fn node_depth(&self, at: &[u8]) -> usize {
        self.beneath(at)
            .filter(|(_, node)| *node)
            .map(|(p, _)| p.len() - at.len())
            .max()
            .unwrap_or(0)
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

    /// The table as it goes to disk: one line a row, the path as its
    /// digits with `n` closing a node actual's, `f` where the row holds
    /// and `e` where it was withdrawn, and the timestamp that set it.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut out = String::new();
        for (p, e) in &self.slots {
            let path: String = p
                .iter()
                .map(|s| {
                    if *s == NODE {
                        'n'
                    } else {
                        char::from(b'0' + *s)
                    }
                })
                .collect();
            let what = if e.filled { 'f' } else { 'e' };
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
            if parts.next().is_some() || path.is_empty() || path.len() > MAX_REACH + 1 {
                return None;
            }
            // a node actual closes the path of the position it speaks of
            let (digits, node) = match path.strip_suffix('n') {
                Some(d) if !d.is_empty() => (d, true),
                Some(_) => return None,
                None => (path, false),
            };
            let mut at: Position = digits
                .bytes()
                .map(|b| b.checked_sub(b'0').filter(|d| *d <= 9))
                .collect::<Option<_>>()?;
            if node {
                at.push(NODE);
            }
            let filled = match what {
                "f" => true,
                "e" => false,
                _ => return None,
            };
            let stamp = stamp.parse().ok()?;
            slots.insert(at, Entry { filled, stamp });
        }
        Some(Self { slots })
    }

    /// Whether any position at or above `at` was emptied.
    fn under_emptied(&self, at: &[u8]) -> bool {
        (1..=at.len()).any(|n| self.slots.get(&at[..n]).is_some_and(|e| !e.filled))
    }

    /// Remove every row beneath `at`, its node actual among them, `at`
    /// itself excepted.
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
        assert!(d.set(&[3, 5], NODE, true, 12));
        assert!(d.set(&[], 4, true, 10));
        assert_eq!(d.len(), 4);
        assert_eq!(d.depth(&[]), 3);
        // the sub in slot 3 leaves, and everything beneath it with them
        assert!(d.set(&[], 3, false, 20));
        assert_eq!(d.len(), 1);
        assert!(!d.filled(&[3]));
        assert!(!d.filled(&[3, 5, 9]));
        assert!(!d.runs_node(&[3, 5]));
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
        // and late memos from the subtree that left with it
        assert!(!d.set(&[3], 5, true, 15));
        assert!(!d.set(&[3], NODE, true, 15));
        assert!(!d.filled(&[3, 5]) && !d.filled(&[3]));
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
    fn a_row_out_of_range_or_a_path_over_the_bound_changes_nothing() {
        let mut d = Downline::new();
        assert!(!d.set(&[], 11, true, 1));
        assert!(!d.set(&[12], 1, true, 1));
        assert!(!d.set(&[1; MAX_REACH], 1, true, 1));
        // the node itself is one by definition and holds no row saying so
        assert!(!d.set(&[], NODE, true, 1));
        // and a node actual is the end of a path, never a step in one
        assert!(!d.set(&[NODE], 1, true, 1));
        assert!(d.is_empty());
        assert!(d.runs_node(&[]));
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
        // and a node heard of before its position was stands there
        assert!(d.set(&[5, 5, 5], NODE, true, 10));
        assert!(d.filled(&[5, 5, 5]) && d.runs_node(&[5, 5, 5]));
        assert_eq!(d.node_depth(&[]), 3);
        // the adoption's own memo, arriving after, changes nothing
        assert!(!d.set(&[5, 5], 5, true, 9));
        assert!(d.runs_node(&[5, 5, 5]));
    }

    #[test]
    fn who_runs_a_node_is_what_the_node_actual_rows_say() {
        // light members only, to any depth the table shows: no node beneath
        assert_eq!(chain(2).node_depth(&[]), 0);
        assert_eq!(chain(7).node_depth(&[]), 0);
        // a node actual at each depth is counted where it stands
        for at in 1..=7 {
            let mut d = chain(7);
            assert!(d.set(&vec![3; at], NODE, true, 10));
            assert_eq!(d.node_depth(&[]), at);
            assert_eq!(d.node_depth(&[3]), at - 1);
        }
        // a node that closed is one no longer, and an earlier launch
        // arriving late does not reopen it
        let mut d = chain(4);
        d.set(&[3, 3, 3, 3], NODE, true, 10);
        assert!(d.set(&[3, 3, 3, 3], NODE, false, 20));
        assert_eq!(d.node_depth(&[]), 0);
        assert!(!d.set(&[3, 3, 3, 3], NODE, true, 15));
        assert!(d.filled(&[3, 3, 3, 3]), "the member stays; the node went");
    }

    #[test]
    fn everyone_may_post_two_up_and_a_node_running_downline_adds_its_depth() {
        assert_eq!(Downline::new().reach(&[]), 2);
        assert_eq!(chain(2).reach(&[]), 2);
        let mut d = chain(2);
        assert!(d.mark_node(&[3, 3], true));
        assert_eq!(d.reach(&[]), 4);
        assert_eq!(d.reach(&[3]), 3);
        assert_eq!(d.reach(&[3, 3]), 2);
        // a mark on nobody is no mark
        assert!(!d.mark_node(&[9], true));
        // and no reach passes the path bound
        let mut d = chain(MAX_REACH);
        d.mark_node(&[3; MAX_REACH], true);
        assert_eq!(d.reach(&[]), MAX_REACH);
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
        d.set(&[0], NODE, true, 7);
        d.set(&[], 3, true, 7);
        d.set(&[], 3, false, 8);
        let text = d.encode();
        // `0[9, n]` and an emptied slot 3, a row each
        assert_eq!(text, "0 f 5\n09 f 6\n0n f 7\n3 e 8\n");
        assert_eq!(Downline::decode(&text), Some(d));
        // what does not read is refused whole
        assert_eq!(Downline::decode("0 f 5\n0x f 6\n"), None);
        assert_eq!(Downline::decode("0 q 5\n"), None);
        assert_eq!(Downline::decode(" f 5\n"), None);
        assert_eq!(Downline::decode("n f 5\n"), None);
        assert_eq!(Downline::decode("0n3 f 5\n"), None);
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
            assert!(parts[0].len() <= MAX_REACH + 1);
            assert!(parts[0].bytes().all(|b| b.is_ascii_digit() || b == b'n'));
            assert!(matches!(parts[1], "f" | "e"));
            assert!(parts[2].parse::<u64>().is_ok());
        }
    }
}
