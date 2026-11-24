//! **The subnet's board at a node** (design §14.4; `wire-format.md` §10.3):
//! who a copy of a post is for, where it goes on, and what the node keeps
//! of it.
//!
//! **A node reads a post's two counts and nothing of the post** [author,
//! 2026-11-23]. Everything here works from the counts, the session a copy
//! arrived on and the node's own downline table ([`crate::downline`]); the
//! post is bytes, known by their hash.
//!
//! **A post travels in two legs.** The copy going up reaches the writer's
//! ancestors and each one's direct subordinates; the copies going down stay
//! in the writer's subtree. So a copy's counts are read by where it came
//! from ([`Source`]), and what a node does with one is a [`Plan`]: the band
//! of positions it is for here, the count to send upstream, and the
//! downline nodes to send it to.

use crate::downline::{BASE_REACH, Downline, MAX_REACH, Position};
use crate::view::NodeView;
use crate::{Adjacency, Keyhash};
use rhtn_archive::board::{BoardPost, BoardReply, BoardRequest, FRAME_BOARD_POST};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

/// The largest post, in bytes (`wire-format.md` §1.3). Provisional
/// [author, 2026-11-23].
pub const POST_MAX: usize = 16_384;

/// What a node's queue holds as shipped, in bytes (design §21).
/// Provisional [author, 2026-11-18], and each node's own to set.
pub const QUEUE_BYTES: usize = 100 * 1024 * 1024;

/// The most posts one catch-up reply carries (`wire-format.md` §1.3).
pub const REPLY_POSTS: usize = 256;

/// What a held post costs the queue: the blocks it occupies, as the
/// mailbox counts (`infra-client-requirements.md` §2, §13).
const BLOCK: usize = 4096;

/// Where a copy arrived from, which the session it arrived on says
/// (`wire-format.md` §10.3). The counts are measured from a different
/// place in each.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A member this node's table places at or beneath it, the empty
    /// position being its own operator. The counts are the post as
    /// written, from the writer.
    Member(Position),
    /// A node beneath this one, on the upward leg. The counts are from
    /// that node's position.
    Below(Position),
    /// This node's upstream, on the downward leg. The counts are from this
    /// node's own position.
    Above,
    /// **A sibling node's member attached here in failover**, standing so
    /// many levels below its own node [author, 2026-11-24]. Its position
    /// is not beneath this node, and it is carried as far as what reaches
    /// this node allows.
    Fostered(usize),
}

/// Why a copy is discarded whole: nothing delivered, stored or forwarded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discard {
    /// A count over the path bound.
    Count,
    /// Counts that do not fit the session the copy arrived on: from below
    /// with nothing left to go up or a downward count that is not 1, from
    /// above with anything left to go up.
    Leg,
}

/// **The positions a copy is for at this node**, as paths beneath it. Kept
/// with the post, so that a member asking later for what it missed is
/// answered against where it stands when it asks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Band {
    /// A position and its subtree, to so many levels.
    pub beneath: Option<(Position, usize)>,
    /// Positions in the band with their direct subordinates: the
    /// writer's ancestors on the upward leg.
    pub around: Vec<Position>,
    /// Positions in the band alone.
    pub only: Vec<Position>,
    /// A sibling's members attached in failover, where they stand no more
    /// than this many levels below their own node.
    pub fostered: Option<usize>,
}

impl Band {
    /// Whether the position, beneath this node, is in the band.
    #[must_use]
    pub fn holds(&self, at: &[u8]) -> bool {
        let beneath = self
            .beneath
            .as_ref()
            .is_some_and(|(root, depth)| at.starts_with(root) && at.len() - root.len() <= *depth);
        beneath
            || self
                .around
                .iter()
                .any(|a| at == a || (at.len() == a.len() + 1 && at.starts_with(a)))
            || self.only.iter().any(|a| at == a)
    }

    /// Whether a sibling's member in failover, so many levels below its
    /// own node, is in the band.
    #[must_use]
    pub fn holds_fostered(&self, below: usize) -> bool {
        self.fostered.is_some_and(|depth| below <= depth)
    }

    /// Whether the band holds nobody here.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.beneath.is_none()
            && self.around.is_empty()
            && self.only.is_empty()
            && self.fostered.is_none()
    }

    /// The band as one line, for the file a held post is kept in: `b` a
    /// position and a depth, `a` and `o` positions, `f` a depth, a position
    /// being its digits and the node's own a dot.
    #[must_use]
    pub fn encode(&self) -> String {
        let path = |p: &[u8]| -> String {
            if p.is_empty() {
                ".".into()
            } else {
                p.iter().map(|s| char::from(b'0' + *s)).collect()
            }
        };
        let list = |ps: &[Position]| ps.iter().map(|p| path(p)).collect::<Vec<_>>().join(",");
        let mut parts = Vec::new();
        if let Some((root, depth)) = &self.beneath {
            parts.push(format!("b={}/{depth}", path(root)));
        }
        if !self.around.is_empty() {
            parts.push(format!("a={}", list(&self.around)));
        }
        if !self.only.is_empty() {
            parts.push(format!("o={}", list(&self.only)));
        }
        if let Some(depth) = self.fostered {
            parts.push(format!("f={depth}"));
        }
        parts.join(";")
    }

    /// A band from the line [`Band::encode`] wrote. A part it does not
    /// know is passed over; one it knows and cannot read refuses the line.
    #[must_use]
    pub fn decode(line: &str) -> Option<Self> {
        let path = |s: &str| -> Option<Position> {
            if s == "." {
                return Some(Vec::new());
            }
            (!s.is_empty() && s.len() <= MAX_REACH)
                .then(|| {
                    s.bytes()
                        .map(|b| b.checked_sub(b'0').filter(|d| *d <= 9))
                        .collect::<Option<Position>>()
                })
                .flatten()
        };
        let list = |s: &str| s.split(',').map(path).collect::<Option<Vec<Position>>>();
        let mut band = Band::default();
        for part in line.split(';').filter(|p| !p.is_empty()) {
            let (key, value) = part.split_once('=')?;
            match key {
                "b" => {
                    let (root, depth) = value.split_once('/')?;
                    band.beneath = Some((path(root)?, depth.parse().ok()?));
                }
                "a" => band.around = list(value)?,
                "o" => band.only = list(value)?,
                "f" => band.fostered = Some(value.parse().ok()?),
                _ => {}
            }
        }
        Some(band)
    }
}

/// What a node does with one copy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Who it is for among those attached here.
    pub band: Band,
    /// The count to send upstream as `(up, 1)`, where the post goes on up.
    pub up: Option<usize>,
    /// The downline nodes to send it to, each with the count it is sent
    /// as `(0, down)`.
    pub down: Vec<(Position, usize)>,
    /// **The upward count this node carries the post with, from the
    /// writer**, where this is the first node the post reached: what was
    /// asked, lowered to what the writer has earned. Every copy carries
    /// it on unchanged, so a reader anywhere in the band reads the band
    /// the post was carried in and not the one its writer asked for.
    pub carried: Option<usize>,
}

/// **Work out what to do with a copy** (`wire-format.md` §10.3) from the
/// node's downline table and the three lists worked out from it
/// ([`Downline::lists`]), which are passed in because they are redone
/// when the downline changes and not per post [author, 2026-11-18].
pub fn plan(
    downline: &Downline,
    lists: &[Vec<Position>; 3],
    source: &Source,
    up: usize,
    down: usize,
) -> Result<Plan, Discard> {
    if up > MAX_REACH || down > MAX_REACH {
        return Err(Discard::Count);
    }
    // the nearest downline nodes within `depth` levels of this node
    let within = |depth: usize| -> &[Position] {
        match depth {
            0 => &[],
            d => &lists[d.min(3) - 1],
        }
    };
    let mut out = Plan::default();
    // an ancestor on the line and its direct subordinates are in the band,
    // and each of those subordinates that runs a node is sent `(0, 0)`:
    // its own operator, and no further
    let around = |out: &mut Plan, line: &[u8], levels: usize, skip: Option<&[u8]>| {
        for i in 1..=levels.min(line.len()) {
            let a = &line[..line.len() - i];
            out.band.around.push(a.to_vec());
            for p in within(a.len() + 1) {
                let direct = p.len() == a.len() + 1 && p.starts_with(a);
                if direct && skip != Some(p.as_slice()) && !out.down.iter().any(|(q, _)| q == p) {
                    out.down.push((p.clone(), 0));
                }
            }
        }
    };
    match source {
        Source::Member(w) => {
            // the first node a post reaches lowers its upward count to
            // what the writer has earned, and does not refuse it (design
            // §14.4) [author, 2026-11-18]
            let up = up.min(downline.reach(w));
            out.carried = Some(up);
            out.band.beneath = Some((w.clone(), down));
            for p in within(w.len() + down) {
                if p.len() > w.len() && p.starts_with(w) && p.len() - w.len() <= down {
                    out.down.push((p.clone(), down - (p.len() - w.len())));
                }
            }
            around(&mut out, w, up, None);
            out.up = (up > w.len()).then(|| up - w.len());
        }
        Source::Below(d) => {
            if up == 0 || down != 1 || d.is_empty() {
                return Err(Discard::Leg);
            }
            around(&mut out, d, up, Some(d.as_slice()));
            out.up = (up > d.len()).then(|| up - d.len());
        }
        Source::Above => {
            if up != 0 {
                return Err(Discard::Leg);
            }
            out.band.beneath = Some((Vec::new(), down));
            // what this node is sent is what a sibling node would have
            // been, the two standing side by side under one patron
            out.band.fostered = Some(down);
            for p in within(down) {
                if p.len() <= down {
                    out.down.push((p.clone(), down - p.len()));
                }
            }
        }
        Source::Fostered(below) => {
            // this node's table shows nothing of that member's downline,
            // so their reach is what every member has
            let up = up.min(BASE_REACH);
            out.carried = Some(up);
            if up > *below {
                // this node's own operator is a subordinate of the
                // writer's ancestor, and is in the band; nobody beneath
                // them is
                out.band.only.push(Vec::new());
                out.up = Some(up - below);
            }
        }
    }
    Ok(out)
}

/// The hash a post is known by (`wire-format.md` §10.3).
#[must_use]
pub fn hash(post: &[u8]) -> [u8; 32] {
    Sha256::digest(post).into()
}

/// One post as the queue holds it: the bytes, who they were for here, and
/// the number it was stored under. Nothing of who wrote it, when, or in
/// what thread, which are inside the bytes and not this node's to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The node's own number for it.
    pub number: u64,
    /// The post.
    pub post: Vec<u8>,
    /// Who it was for at this node.
    pub band: Band,
    /// The upward count it was carried with, from its writer.
    pub carried: usize,
}

/// One answer to a member asking for what it missed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<'a> {
    /// Number, post and the upward count it was carried with, ascending.
    pub posts: Vec<(u64, &'a [u8], usize)>,
    /// The number this page read through: what to ask from next.
    pub through: u64,
    /// Whether posts numbered above that are held.
    pub more: bool,
}

/// **The node's queue** (design §14.4) [author, 2026-11-18]: bounded by
/// storage and not by time, the oldest out first, each post numbered as it
/// is stored from a counter that is never reused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Queue {
    cap: usize,
    cost: usize,
    next: u64,
    held: VecDeque<Held>,
    hashes: BTreeSet<[u8; 32]>,
    /// The numbers that fell out since last asked: what to remove from
    /// wherever the queue is also kept.
    gone: Vec<u64>,
}

impl Queue {
    /// An empty queue holding at most `cap` bytes, counted in blocks.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self::resumed(cap, 1)
    }

    /// An empty queue whose next number is `next`: a queue that lost what
    /// it held, and not the numbers it had given out.
    #[must_use]
    pub fn resumed(cap: usize, next: u64) -> Self {
        Queue {
            cap,
            cost: 0,
            next: next.max(1),
            held: VecDeque::new(),
            hashes: BTreeSet::new(),
            gone: Vec::new(),
        }
    }

    /// A queue from the posts a node kept on disk, in the order of their
    /// numbers, counting on from the newest. The oldest go where the
    /// bound has since been lowered.
    #[must_use]
    pub fn restored(cap: usize, mut held: Vec<Held>) -> Self {
        held.sort_by_key(|h| h.number);
        let mut q = Self::resumed(cap, held.last().map_or(1, |h| h.number + 1));
        for h in held {
            let cost = Self::cost_of(&h.post);
            if h.post.is_empty() || h.post.len() > POST_MAX || cost > cap {
                q.gone.push(h.number);
                continue;
            }
            while q.cost + cost > q.cap {
                let Some(old) = q.held.pop_front() else { break };
                q.cost -= Self::cost_of(&old.post);
                q.hashes.remove(&hash(&old.post));
                q.gone.push(old.number);
            }
            q.cost += cost;
            q.hashes.insert(hash(&h.post));
            q.held.push_back(h);
        }
        q
    }

    /// The numbers that fell out since this was last asked.
    pub fn take_gone(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.gone)
    }

    /// The number the next post stored will be given.
    #[must_use]
    pub fn next(&self) -> u64 {
        self.next
    }

    /// How many posts are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Whether a post of this hash is held: a second copy of one is
    /// discarded, neither delivered nor forwarded again.
    #[must_use]
    pub fn holds(&self, hash: &[u8; 32]) -> bool {
        self.hashes.contains(hash)
    }

    /// The posts held, oldest first.
    pub fn held(&self) -> impl Iterator<Item = &Held> {
        self.held.iter()
    }

    /// Store a post and say what number it was given. Nothing for one
    /// already held, for an empty one, or for one over the bound. The
    /// oldest go to make room, and a post too large for the whole queue
    /// is not kept.
    pub fn keep(&mut self, post: Vec<u8>, band: Band, carried: usize) -> Option<u64> {
        let h = hash(&post);
        let cost = Self::cost_of(&post);
        if post.is_empty() || post.len() > POST_MAX || cost > self.cap || self.holds(&h) {
            return None;
        }
        while self.cost + cost > self.cap {
            let gone = self.held.pop_front()?;
            self.cost -= Self::cost_of(&gone.post);
            self.hashes.remove(&hash(&gone.post));
            self.gone.push(gone.number);
        }
        let number = self.next;
        self.next += 1;
        self.cost += cost;
        self.hashes.insert(h);
        self.held.push_back(Held {
            number,
            post,
            band,
            carried,
        });
        Some(number)
    }

    /// **What a member missed** (`wire-format.md` §10.3): the posts
    /// numbered above `after` that `mine` says are for the asker, oldest
    /// first, as many as fit in `room` bytes and no more than
    /// [`REPLY_POSTS`]. A number this queue no longer holds, or never gave
    /// out, is answered as its absence is: from the oldest post held.
    pub fn after(&self, after: Option<u64>, mine: impl Fn(&Band) -> bool, room: usize) -> Page<'_> {
        let from = after.filter(|n| *n < self.next).unwrap_or(0);
        let mut page = Page {
            posts: Vec::new(),
            through: from.max(self.held.front().map_or(self.next - 1, |h| h.number - 1)),
            more: false,
        };
        let mut used = 0;
        for h in self.held.iter().filter(|h| h.number > from) {
            if mine(&h.band) {
                // a number, a byte-string head, the post and a count
                let size = h.post.len() + 18;
                if page.posts.len() == REPLY_POSTS || used + size > room {
                    page.more = true;
                    break;
                }
                used += size;
                page.posts.push((h.number, h.post.as_slice(), h.carried));
            }
            page.through = h.number;
        }
        page
    }

    fn cost_of(post: &[u8]) -> usize {
        post.len().div_ceil(BLOCK) * BLOCK
    }
}

/// What a node did with an arriving `BoardPost`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Taken {
    /// It does not read, or carries a number, which is a node's to give a
    /// member and nobody's to give a node, or carries the count a post
    /// was carried with from a member, or lacks it from a node.
    Malformed,
    /// From a party that is neither a member here, a node beneath, nor
    /// the upstream.
    Stranger,
    /// A post of this hash is already held: not delivered, stored or
    /// forwarded again.
    Duplicate,
    /// Its counts do not fit the session it arrived on.
    Discarded(Discard),
    /// It is for nobody here and goes nowhere on.
    Nowhere,
    /// The queue could not take it.
    Unkept,
    /// Stored under `number`, delivered to so many members, sent upstream
    /// or not, and sent to so many nodes beneath.
    Carried {
        /// The number it was stored under.
        number: u64,
        /// Members it was delivered to.
        delivered: usize,
        /// Whether it went on upstream.
        up: bool,
        /// Nodes beneath it was sent to.
        down: usize,
    },
}

/// Where a party attached here stands, for what is delivered to it.
enum Standing {
    /// At a position this node's own records place beneath it.
    At(Position),
    /// A sibling's member in failover, so many levels below its own node.
    Fostered(usize),
}

/// The room a catch-up reply's posts may take: the request frame bound
/// (`wire-format.md` §9.2) less the reply's own fields.
const REPLY_ROOM: usize = 262_144 - 64;

impl NodeView {
    /// **Receive a `BoardPost`** on the session from `from`
    /// (`wire-format.md` §10.3): work out who it is for here from the
    /// counts and where it came from, keep it, deliver it to every member
    /// attached here in its band, and send it on.  The post's own bytes
    /// are not read.
    pub fn receive_board(&mut self, adj: &dyn Adjacency, from: &Keyhash, body: &[u8]) -> Taken {
        let taken = self.carry_board(adj, from, body);
        // the counts and the outcome, and nothing of the post
        tracing::debug!(
            target: "node",
            from8 = %crate::diag::id8(from),
            taken = ?taken,
            "node.board"
        );
        taken
    }

    fn carry_board(&mut self, adj: &dyn Adjacency, from: &Keyhash, body: &[u8]) -> Taken {
        let Ok(frame) = BoardPost::decode(body) else {
            return Taken::Malformed;
        };
        if frame.number.is_some() {
            return Taken::Malformed;
        }
        let Some(source) = self.board_source(from) else {
            return Taken::Stranger;
        };
        if self.board.holds(&hash(&frame.post)) {
            return Taken::Duplicate;
        }
        let (up, down) = (
            usize::try_from(frame.up).unwrap_or(usize::MAX),
            usize::try_from(frame.down).unwrap_or(usize::MAX),
        );
        let plan = match plan(&self.downline, self.downline.lists(), &source, up, down) {
            Ok(p) => p,
            Err(why) => return Taken::Discarded(why),
        };
        // **the first node says what the post is carried with, and every
        // node after it carries that on**: a member says nothing of it,
        // and a node always does
        let said = frame.carried.and_then(|n| usize::try_from(n).ok());
        let carried = match (plan.carried, said) {
            (Some(first), None) => first,
            (None, Some(passed)) if passed <= MAX_REACH => passed,
            _ => return Taken::Malformed,
        };
        if plan.band.is_empty() && plan.up.is_none() && plan.down.is_empty() {
            return Taken::Nowhere;
        }
        let Some(number) = self
            .board
            .keep(frame.post.clone(), plan.band.clone(), carried)
        else {
            return Taken::Unkept;
        };
        self.file_board(number, &plan.band, carried, &frame.post);
        // **to every session of every member in the band**, the writer's
        // own among them: a control frame goes to each session a key
        // holds, and the post coming back numbered is how a writer's
        // device learns this node took it
        let me = self.me();
        let delivery = BoardPost {
            up: 0,
            down: 0,
            post: frame.post.clone(),
            number: Some(number),
            carried: Some(carried as u64),
        }
        .encode();
        let mut members: BTreeSet<Keyhash> = self.attached.clone();
        members.insert(me);
        let mut delivered = 0;
        for k in members {
            if self.in_band(&k, &plan.band) && adj.has_session(&k) {
                adj.send(&k, FRAME_BOARD_POST, &delivery);
                delivered += 1;
            }
        }
        let copy = |up: usize, down: usize| {
            BoardPost {
                up: up as u64,
                down: down as u64,
                post: frame.post.clone(),
                number: None,
                carried: Some(carried as u64),
            }
            .encode()
        };
        // upward always goes on while the count allows, to this node's
        // patron where a session exists and otherwise to the nearest node
        // on the patron chain, as a memo does
        let upstream = self
            .patron_in(&self.anchor())
            .filter(|p| adj.has_session(p))
            .or(self.serving_node);
        let mut went_up = false;
        if let (Some(up), Some(to)) = (plan.up, upstream) {
            adj.send(&to, FRAME_BOARD_POST, &copy(up, 1));
            went_up = true;
        }
        let mut sent_down = 0;
        for (at, down) in &plan.down {
            if let Some(k) = self.key_at(at)
                && adj.has_session(&k)
            {
                adj.send(&k, FRAME_BOARD_POST, &copy(0, *down));
                sent_down += 1;
            }
        }
        Taken::Carried {
            number,
            delivered,
            up: went_up,
            down: sent_down,
        }
    }

    /// **Answer a member asking for what it missed** (`wire-format.md`
    /// §10.3, request type 17): the posts numbered above the one it holds
    /// whose band holds where it stands now.  Nothing, which closes the
    /// stream, for an asker holding no session attached here.
    #[must_use]
    pub fn answer_board(&self, asker: &Keyhash, body: &[u8]) -> Option<Vec<u8>> {
        let req = BoardRequest::decode(body).ok()?;
        if *asker != self.me() && !self.attached.contains(asker) {
            return None;
        }
        let standing = self.board_standing(asker)?;
        let page = self.board.after(
            req.after,
            |band| match &standing {
                Standing::At(at) => band.holds(at),
                Standing::Fostered(below) => band.holds_fostered(*below),
            },
            REPLY_ROOM,
        );
        Some(
            BoardReply {
                nonce: req.nonce,
                posts: page
                    .posts
                    .iter()
                    .map(|(n, post, carried)| (*n, post.to_vec(), *carried as u64))
                    .collect(),
                through: page.through,
                more: page.more,
            }
            .encode(),
        )
    }

    /// **Keep the queue in `dir`**, a file a post named by its number, and
    /// take up what is already there: the numbers go on from the newest
    /// held, so none is given twice across a restart.  A file that does
    /// not read is left where it is and not held.
    pub fn open_board(&mut self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let mut held = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            let number = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.parse::<u64>().ok());
            let (Some(number), Ok(bytes)) = (number, std::fs::read(&path)) else {
                continue;
            };
            let Some(cut) = bytes.iter().position(|b| *b == b'\n') else {
                continue;
            };
            // the count the post was carried with, a space, and the band
            let line = std::str::from_utf8(&bytes[..cut])
                .ok()
                .and_then(|l| l.split_once(' '))
                .and_then(|(carried, band)| {
                    let carried = carried.parse::<usize>().ok().filter(|n| *n <= MAX_REACH)?;
                    Some((carried, Band::decode(band)?))
                });
            if let Some((carried, band)) = line {
                held.push(Held {
                    number,
                    post: bytes[cut + 1..].to_vec(),
                    band,
                    carried,
                });
            }
        }
        self.board = Queue::restored(QUEUE_BYTES, held);
        self.board_dir = Some(dir.to_path_buf());
        self.clear_gone();
        Ok(())
    }

    /// Write a post just kept to its file, and remove the files of those
    /// that fell out to make room, leaving nothing of them.
    fn file_board(&mut self, number: u64, band: &Band, carried: usize, post: &[u8]) {
        if let Some(dir) = &self.board_dir {
            let mut bytes = format!("{carried} {}", band.encode()).into_bytes();
            bytes.push(b'\n');
            bytes.extend_from_slice(post);
            if let Err(e) = std::fs::write(Self::board_file(dir, number), bytes) {
                tracing::warn!(target: "node", error = %e, "node.board.unwritten");
            }
        }
        self.clear_gone();
    }

    fn clear_gone(&mut self) {
        let gone = self.board.take_gone();
        if let Some(dir) = &self.board_dir {
            for n in gone {
                let _ = std::fs::remove_file(Self::board_file(dir, n));
            }
        }
    }

    fn board_file(dir: &Path, number: u64) -> PathBuf {
        dir.join(format!("{number:020}"))
    }

    /// Where a copy came from, by the session it arrived on.
    fn board_source(&self, from: &Keyhash) -> Option<Source> {
        if *from == self.me() {
            return Some(Source::Member(Vec::new()));
        }
        if let Some(at) = self.path_to(from) {
            // a subordinate that runs a node sends copies; one that does
            // not is a member writing
            return Some(if self.table.is_infra(from) {
                Source::Below(at)
            } else {
                Source::Member(at)
            });
        }
        if self.patron_in(&self.anchor()) == Some(*from) || self.serving_node == Some(*from) {
            return Some(Source::Above);
        }
        if self.attached.contains(from) {
            return self.levels_below_own_node(from).map(Source::Fostered);
        }
        None
    }

    /// Where a party attached here stands, for delivery and for catch-up.
    /// Nothing for a node beneath, which is sent copies and delivered
    /// nothing, and for a party this node cannot place.
    fn board_standing(&self, k: &Keyhash) -> Option<Standing> {
        if *k == self.me() {
            return Some(Standing::At(Vec::new()));
        }
        if let Some(at) = self.path_to(k) {
            return (!self.table.is_infra(k)).then_some(Standing::At(at));
        }
        self.levels_below_own_node(k).map(Standing::Fostered)
    }

    fn in_band(&self, k: &Keyhash, band: &Band) -> bool {
        match self.board_standing(k) {
            Some(Standing::At(at)) => band.holds(&at),
            Some(Standing::Fostered(below)) => band.holds_fostered(below),
            None => false,
        }
    }

    /// How many levels a sibling's member stands below its own node, by
    /// the bindings this node holds: nothing where it holds too few to
    /// say, and that member is not carried.
    fn levels_below_own_node(&self, k: &Keyhash) -> Option<usize> {
        let mut at = *k;
        for below in 0..=2 {
            if self.table.is_infra(&at) {
                return Some(below);
            }
            at = self.table.patrons(&at).into_iter().next()?;
        }
        None
    }

    /// Who stands at a position beneath this node, by its own slots and
    /// the bindings it holds.
    fn key_at(&self, at: &[u8]) -> Option<Keyhash> {
        let (first, rest) = at.split_first()?;
        let mut k = self.slots.get(&u64::from(*first))?.occupant?;
        for slot in rest {
            k = self.slot_states(&k).get(slot)?.0?;
        }
        Some(k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A downline with a node at every level down slot 0, three deep, and
    /// light members beside them:
    ///
    /// ```text
    /// me ─ 0n ─ 00n ─ 000n
    ///    │    └ 01
    ///    ├ 1 ─ 10
    ///    │   └ 11n
    ///    └ 2 ─ 20 ─ 200n
    /// ```
    fn tree() -> (Downline, [Vec<Position>; 3]) {
        let mut d = Downline::new();
        let rows: [(&[u8], u8); 10] = [
            (&[], 0),
            (&[0], 0),
            (&[0, 0], 0),
            (&[0], 1),
            (&[], 1),
            (&[1], 0),
            (&[1], 1),
            (&[], 2),
            (&[2], 0),
            (&[2, 0], 0),
        ];
        for (patron, slot) in rows {
            assert!(d.set(patron, slot, true, 1));
        }
        for node in [&[0][..], &[0, 0], &[0, 0, 0], &[1, 1], &[2, 0, 0]] {
            assert!(d.mark_node(node, true));
        }
        let lists = d.lists().clone();
        (d, lists)
    }

    fn sent(p: &Plan) -> Vec<(Position, usize)> {
        let mut v = p.down.clone();
        v.sort();
        v
    }

    #[test]
    fn a_note_to_self_reaches_the_writers_own_position_and_goes_nowhere() {
        let (d, lists) = tree();
        let p = plan(&d, &lists, &Source::Member(vec![1]), 0, 0).unwrap();
        assert!(p.band.holds(&[1]));
        assert!(!p.band.holds(&[]) && !p.band.holds(&[1, 0]) && !p.band.holds(&[2]));
        assert_eq!((p.up, p.down.len()), (None, 0));
    }

    #[test]
    fn down_reaches_the_writers_subtree_through_the_nearest_nodes() {
        let (d, lists) = tree();
        // the operator posts `+0/-1`: the direct subordinates, the one of
        // them that runs a node being sent a copy for its operator alone
        let p = plan(&d, &lists, &Source::Member(vec![]), 0, 1).unwrap();
        assert!(p.band.holds(&[]) && p.band.holds(&[1]) && p.band.holds(&[2]));
        assert!(!p.band.holds(&[1, 0]));
        assert_eq!(sent(&p), vec![(vec![0], 0)]);
        assert_eq!(p.up, None);
        // `-2`: the light grandsubordinates, and the nodes within two
        assert_eq!(
            sent(&plan(&d, &lists, &Source::Member(vec![]), 0, 2).unwrap()),
            vec![(vec![0], 1), (vec![1, 1], 0)]
        );
        // `-5` uses the third list and carries its real count on
        assert_eq!(
            sent(&plan(&d, &lists, &Source::Member(vec![]), 0, 5).unwrap()),
            vec![(vec![0], 4), (vec![1, 1], 3), (vec![2, 0, 0], 2)]
        );
        // a light member's `-1` stays in their own subtree: the node
        // beneath them, and nobody in another branch
        let p = plan(&d, &lists, &Source::Member(vec![1]), 0, 1).unwrap();
        assert!(p.band.holds(&[1, 0]) && !p.band.holds(&[2]) && !p.band.holds(&[]));
        assert_eq!(sent(&p), vec![(vec![1, 1], 0)]);
    }

    #[test]
    fn up_reaches_each_ancestor_and_that_ancestors_direct_subordinates() {
        let (d, lists) = tree();
        // the light member at 10 posts `+1/-0`: the patron at 1 and the
        // sibling at 11, who runs a node; nobody else
        let p = plan(&d, &lists, &Source::Member(vec![1, 0]), 1, 0).unwrap();
        assert!(p.band.holds(&[1]) && p.band.holds(&[1, 0]) && p.band.holds(&[1, 1]));
        assert!(!p.band.holds(&[]) && !p.band.holds(&[2]) && !p.band.holds(&[0]));
        assert_eq!(sent(&p), vec![(vec![1, 1], 0)]);
        assert_eq!(p.up, None, "the node's own position is not in reach");
        // `+2/-0` adds the operator and the operator's direct
        // subordinates, and no nephew
        let p = plan(&d, &lists, &Source::Member(vec![1, 0]), 2, 0).unwrap();
        assert!(p.band.holds(&[]) && p.band.holds(&[2]) && p.band.holds(&[0]));
        assert!(!p.band.holds(&[2, 0]) && !p.band.holds(&[0, 1]));
        assert_eq!(sent(&p), vec![(vec![0], 0), (vec![1, 1], 0)]);
        assert_eq!(p.up, None, "and it stops at this node");
    }

    #[test]
    fn upward_reach_is_lowered_to_what_the_writer_has_earned_and_not_refused() {
        let (d, lists) = tree();
        // 10 has nobody beneath them: `+9` is carried as `+2`
        let p = plan(&d, &lists, &Source::Member(vec![1, 0]), 9, 0).unwrap();
        assert_eq!(p.up, None);
        // 1 has a node one level down: `+9` is carried as `+3`, two of it
        // past this node
        assert_eq!(d.reach(&[1]), 3);
        let p = plan(&d, &lists, &Source::Member(vec![1]), 9, 0).unwrap();
        assert_eq!(p.up, Some(2));
        // the operator's downline runs nodes three deep: `+9` is `+5`
        assert_eq!(
            plan(&d, &lists, &Source::Member(vec![]), 9, 0).unwrap().up,
            Some(5)
        );
        // the downward count is never lowered
        let p = plan(&d, &lists, &Source::Member(vec![1, 0]), 0, 24).unwrap();
        assert_eq!(p.band.beneath, Some((vec![1, 0], 24)));
        // and a count over the path bound is no post at all
        assert_eq!(
            plan(&d, &lists, &Source::Member(vec![]), 25, 0),
            Err(Discard::Count)
        );
        assert_eq!(plan(&d, &lists, &Source::Above, 0, 25), Err(Discard::Count));
    }

    #[test]
    fn an_upward_copy_counts_from_the_node_that_sent_it() {
        let (d, lists) = tree();
        // from the node at 200, three levels down under two light
        // members, with one level left: its patron at 20 and 20's direct
        // subordinates, and not this node's own operator
        let p = plan(&d, &lists, &Source::Below(vec![2, 0, 0]), 1, 1).unwrap();
        assert!(p.band.holds(&[2, 0]) && p.band.holds(&[2, 0, 0]));
        assert!(!p.band.holds(&[2]) && !p.band.holds(&[]));
        assert_eq!((p.up, p.down.len()), (None, 0), "not back to its sender");
        // with four left it passes this node by one
        let p = plan(&d, &lists, &Source::Below(vec![2, 0, 0]), 4, 1).unwrap();
        assert!(p.band.holds(&[]) && p.band.holds(&[1]) && p.band.holds(&[2]));
        assert!(!p.band.holds(&[1, 0]), "no nephew");
        assert_eq!(p.up, Some(1));
        // the operator's direct subordinate that runs a node is sent it
        assert_eq!(sent(&p), vec![(vec![0], 0)]);
        // from the node at 0, one level: this node's operator and the
        // operator's direct subordinates, the sender not among those sent
        let p = plan(&d, &lists, &Source::Below(vec![0]), 1, 1).unwrap();
        assert!(p.band.holds(&[]) && p.band.holds(&[1]) && p.band.holds(&[2]));
        assert_eq!((p.up, p.down.len()), (None, 0));
        // the upward leg is not capped again on the way
        assert_eq!(
            plan(&d, &lists, &Source::Below(vec![0]), 20, 1).unwrap().up,
            Some(19)
        );
    }

    #[test]
    fn a_downward_copy_counts_from_the_node_that_receives_it() {
        let (d, lists) = tree();
        // `(0, 0)`: this node's operator and no further
        let p = plan(&d, &lists, &Source::Above, 0, 0).unwrap();
        assert!(p.band.holds(&[]) && !p.band.holds(&[1]));
        assert_eq!((p.up, p.down.len()), (None, 0));
        // `(0, 2)`: two levels, and the nodes within them
        let p = plan(&d, &lists, &Source::Above, 0, 2).unwrap();
        assert!(p.band.holds(&[1, 0]) && !p.band.holds(&[2, 0, 0]));
        assert_eq!(sent(&p), vec![(vec![0], 1), (vec![1, 1], 0)]);
    }

    #[test]
    fn a_copy_that_does_not_fit_its_session_is_discarded() {
        let (d, lists) = tree();
        let below = Source::Below(vec![0]);
        assert_eq!(plan(&d, &lists, &below, 0, 1), Err(Discard::Leg));
        assert_eq!(plan(&d, &lists, &below, 1, 0), Err(Discard::Leg));
        assert_eq!(plan(&d, &lists, &below, 1, 2), Err(Discard::Leg));
        assert_eq!(plan(&d, &lists, &Source::Above, 1, 0), Err(Discard::Leg));
        assert_eq!(
            plan(&d, &lists, &Source::Below(vec![]), 1, 1),
            Err(Discard::Leg),
            "a node is not beneath itself"
        );
    }

    #[test]
    fn a_siblings_member_in_failover_is_carried_as_far_as_this_node_can() {
        let (d, lists) = tree();
        // what comes from above is what their own node would have been
        // sent: theirs where they stand within its count
        let p = plan(&d, &lists, &Source::Above, 0, 1).unwrap();
        assert!(p.band.holds_fostered(0) && p.band.holds_fostered(1));
        assert!(!p.band.holds_fostered(2));
        // and nothing that started beneath this node is theirs
        let p = plan(&d, &lists, &Source::Member(vec![1]), 2, 2).unwrap();
        assert!(!p.band.holds_fostered(0));
        // their own post goes upstream, counted from this node, at what
        // every member may post; this node's operator is in its band
        let p = plan(&d, &lists, &Source::Fostered(1), 9, 3).unwrap();
        assert_eq!(p.up, Some(1));
        assert!(p.band.holds(&[]) && !p.band.holds(&[1]));
        assert!(
            p.down.is_empty(),
            "their own subtree is not reached from here"
        );
        // one that does not reach past their own node goes nowhere
        let p = plan(&d, &lists, &Source::Fostered(2), 2, 0).unwrap();
        assert!(p.band.is_empty() && p.up.is_none());
    }

    fn band_for(at: &[u8]) -> Band {
        Band {
            only: vec![at.to_vec()],
            ..Band::default()
        }
    }

    #[test]
    fn the_queue_numbers_what_it_stores_and_holds_one_copy_of_a_post() {
        let mut q = Queue::new(QUEUE_BYTES);
        assert_eq!(q.keep(b"one".to_vec(), Band::default(), 2), Some(1));
        assert_eq!(q.keep(b"two".to_vec(), Band::default(), 2), Some(2));
        // a second copy is known by its hash and not stored again
        assert!(q.holds(&hash(b"one")));
        assert_eq!(q.keep(b"one".to_vec(), Band::default(), 2), None);
        // nor an empty post, nor one over the bound
        assert_eq!(q.keep(Vec::new(), Band::default(), 2), None);
        assert_eq!(q.keep(vec![7; POST_MAX + 1], Band::default(), 2), None);
        assert_eq!(q.keep(vec![7; POST_MAX], Band::default(), 2), Some(3));
        assert_eq!((q.len(), q.next()), (3, 4));
    }

    #[test]
    fn the_oldest_go_first_at_the_bound_and_no_number_is_given_twice() {
        // room for three posts of a block each
        let mut q = Queue::new(3 * BLOCK);
        for n in 0..5u8 {
            assert_eq!(
                q.keep(vec![n; 10], Band::default(), 2),
                Some(u64::from(n) + 1)
            );
        }
        assert_eq!(q.held().map(|h| h.number).collect::<Vec<_>>(), [3, 4, 5]);
        // what fell out is forgotten, hash and all: sent again it is a
        // new arrival with a new number
        assert!(!q.holds(&hash(&[0; 10])));
        assert_eq!(q.keep(vec![0; 10], Band::default(), 2), Some(6));
        // a post of four blocks cannot be held at all
        assert_eq!(q.keep(vec![9; 3 * BLOCK + 1], Band::default(), 2), None);
        // a queue that lost what it held keeps counting from where it was
        let q = Queue::resumed(3 * BLOCK, 7);
        assert_eq!((q.len(), q.next()), (0, 7));
    }

    #[test]
    fn catch_up_returns_the_askers_band_after_the_number_it_holds() {
        let mut q = Queue::new(QUEUE_BYTES);
        q.keep(b"a".to_vec(), band_for(&[1]), 2);
        q.keep(b"b".to_vec(), band_for(&[2]), 2);
        q.keep(b"c".to_vec(), band_for(&[1]), 2);
        q.keep(b"d".to_vec(), band_for(&[2]), 2);
        let one = |b: &Band| b.holds(&[1]);
        let two = |b: &Band| b.holds(&[2]);
        // two members asking after the same number are answered apart
        let p = q.after(None, one, usize::MAX);
        assert_eq!(p.posts, vec![(1, &b"a"[..], 2), (3, &b"c"[..], 2)]);
        assert_eq!(
            (p.through, p.more),
            (4, false),
            "read through a post not theirs"
        );
        let p = q.after(Some(2), two, usize::MAX);
        assert_eq!(p.posts, vec![(4, &b"d"[..], 2)]);
        // nothing new is an empty page at the same number
        let p = q.after(Some(4), one, usize::MAX);
        assert_eq!((p.posts.len(), p.through, p.more), (0, 4, false));
        // a number never given out is answered from the oldest held
        assert_eq!(q.after(Some(900), one, usize::MAX).posts.len(), 2);
    }

    #[test]
    fn a_page_stops_at_the_frame_and_says_more_is_held() {
        let mut q = Queue::new(QUEUE_BYTES);
        for n in 0..20u8 {
            q.keep(vec![n; POST_MAX], Band::default(), 2);
        }
        // the largest posts, fifteen to a reply of 262,144 bytes
        let p = q.after(None, |_| true, 262_144 - 64);
        assert_eq!((p.posts.len(), p.through, p.more), (15, 15, true));
        let p = q.after(Some(p.through), |_| true, 262_144 - 64);
        assert_eq!((p.posts.len(), p.through, p.more), (5, 20, false));
        // and never more than the bound on posts, however small
        let mut q = Queue::new(QUEUE_BYTES);
        for n in 0..300u16 {
            q.keep(n.to_be_bytes().to_vec(), Band::default(), 2);
        }
        let p = q.after(None, |_| true, usize::MAX);
        assert_eq!((p.posts.len(), p.through, p.more), (REPLY_POSTS, 256, true));
        // a number fallen out of the queue is answered from what is left
        let mut q = Queue::new(2 * BLOCK);
        for n in 0..4u8 {
            q.keep(vec![n], Band::default(), 2);
        }
        let p = q.after(Some(1), |_| true, usize::MAX);
        assert_eq!(
            p.posts.iter().map(|(n, _, _)| *n).collect::<Vec<_>>(),
            [3, 4]
        );
    }
}
