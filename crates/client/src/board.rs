//! **A subnet's board as a client keeps it**
//! (`light-client-requirements.md` §4.5; design §14.4): the posts it has
//! read or may read, in a pool bounded by storage, with what is unread
//! and where it stands with each node it has caught up from.
//!
//! **The node carries; the client keeps and counts** [author,
//! 2026-11-23]. A node never learns what was read, holds no thread and no
//! date; all of that is here, worked out from posts whose signatures this
//! client checked itself.

use crate::{Keyhash, Txid};
use rhtn_archive::board::Post;
use rhtn_archive::tx::Locator;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use std::collections::{BTreeMap, BTreeSet};

/// What a pool holds for one subnet as shipped, in bytes (design §21).
/// Provisional [author, 2026-11-18], and the person's to set.
pub const POOL_BYTES: usize = 50 * 1024 * 1024;

/// One post as the pool keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// The post, parsed and checked.
    pub post: Post,
    /// The hash it is known by.
    pub hash: [u8; 32],
    /// **Its date, read as no later than when it reached this client**:
    /// a post dated ahead would otherwise outlive every honest one.
    pub dated: u64,
    /// Whether the person has been shown it.
    pub read: bool,
}

/// What became of a post handed to the pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    /// New, checked and kept.
    New,
    /// Already held: one hash is one post, however many times it arrives.
    Held,
    /// It does not read as a post.
    Unread,
    /// Its signature does not make it the writer's it names.
    Unsigned,
    /// It names a position in another subnet: nothing on a board comes
    /// from one.
    Foreign,
}

/// One thread, as a list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Thread {
    /// The hash of the post that began it.
    pub id: [u8; 32],
    /// Its title, where the post that began it is held.
    pub title: Option<String>,
    /// Who began it, where that post is held.
    pub began_by: Option<Keyhash>,
    /// How many of its posts are held.
    pub posts: usize,
    /// How many of those the person has not been shown.
    pub unread: usize,
    /// The date of the newest.
    pub latest: u64,
}

/// The pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pool {
    cap: usize,
    bytes: usize,
    posts: BTreeMap<[u8; 32], Kept>,
    /// The last number each node gave this client: a node's own counter,
    /// which means nothing at another.
    cursors: BTreeMap<Keyhash, u64>,
    /// The nodes this client has caught up from on the session it holds
    /// now, where a delivery's number moves the cursor. Not kept across
    /// a run.
    current: BTreeSet<Keyhash>,
}

impl Pool {
    /// An empty pool holding at most `cap` bytes of posts.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Pool {
            cap,
            bytes: 0,
            posts: BTreeMap::new(),
            cursors: BTreeMap::new(),
            current: BTreeSet::new(),
        }
    }

    /// **Take a post that reached this client for `subnet`'s board at
    /// `arrived`.** It is kept where it parses, its signature makes it
    /// the writer's it names, and it names no other subnet. A post this
    /// client wrote is read already. At the bound the oldest go, by their
    /// own dates.
    pub fn take(&mut self, subnet: &Txid, me: &Keyhash, bytes: &[u8], arrived: u64) -> Took {
        let hash = Post::hash(bytes);
        if self.posts.contains_key(&hash) {
            return Took::Held;
        }
        let Ok(post) = Post::parse(bytes) else {
            return Took::Unread;
        };
        if post.verify().is_err() {
            return Took::Unsigned;
        }
        if post.position.subnet.is_some_and(|s| s != *subnet) {
            return Took::Foreign;
        }
        self.bytes += bytes.len();
        self.posts.insert(
            hash,
            Kept {
                read: post.writer == *me,
                dated: post.written_at.min(arrived),
                hash,
                post,
            },
        );
        while self.bytes > self.cap {
            let Some(oldest) = self
                .posts
                .values()
                .min_by_key(|k| (k.dated, k.hash))
                .map(|k| k.hash)
            else {
                break;
            };
            if let Some(gone) = self.posts.remove(&oldest) {
                self.bytes -= gone.post.bytes.len();
            }
        }
        Took::New
    }

    /// How many posts are held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.posts.len()
    }

    /// Drop one post, by its hash.
    pub fn forget(&mut self, hash: &[u8; 32]) {
        if let Some(gone) = self.posts.remove(hash) {
            self.bytes -= gone.post.bytes.len();
        }
    }

    /// Whether nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.posts.is_empty()
    }

    /// One post, by its hash.
    #[must_use]
    pub fn get(&self, hash: &[u8; 32]) -> Option<&Kept> {
        self.posts.get(hash)
    }

    /// The threads held, the one with the newest post first.
    #[must_use]
    pub fn threads(&self) -> Vec<Thread> {
        let mut by: BTreeMap<[u8; 32], Thread> = BTreeMap::new();
        for k in self.posts.values() {
            let id = k.post.thread_id();
            let t = by.entry(id).or_insert(Thread {
                id,
                title: None,
                began_by: None,
                posts: 0,
                unread: 0,
                latest: 0,
            });
            t.posts += 1;
            t.unread += usize::from(!k.read);
            t.latest = t.latest.max(k.dated);
            if k.post.thread.is_none() {
                t.title.clone_from(&k.post.title);
                t.began_by = Some(k.post.writer);
            }
        }
        let mut out: Vec<Thread> = by.into_values().collect();
        out.sort_by(|a, b| b.latest.cmp(&a.latest).then(a.id.cmp(&b.id)));
        out
    }

    /// One thread's posts, oldest first.
    #[must_use]
    pub fn thread(&self, id: &[u8; 32]) -> Vec<&Kept> {
        let mut out: Vec<&Kept> = self
            .posts
            .values()
            .filter(|k| k.post.thread_id() == *id)
            .collect();
        out.sort_by_key(|k| (k.dated, k.hash));
        out
    }

    /// The person has been shown a thread: how many posts that newly reads.
    pub fn mark_read(&mut self, id: &[u8; 32]) -> usize {
        let mut newly = 0;
        for k in self.posts.values_mut() {
            if !k.read && k.post.thread_id() == *id {
                k.read = true;
                newly += 1;
            }
        }
        newly
    }

    /// How many posts the person has not been shown: the client's own
    /// count, which no node is told.
    #[must_use]
    pub fn unread(&self) -> usize {
        self.posts.values().filter(|k| !k.read).count()
    }

    /// The last number `node` gave this client, to ask from.
    #[must_use]
    pub fn cursor(&self, node: &Keyhash) -> Option<u64> {
        self.cursors.get(node).copied()
    }

    /// A catch-up reply from `node` read through `through`: the cursor
    /// moves there, and where the node holds no more this client is
    /// current with it for the session.
    pub fn caught_up(&mut self, node: &Keyhash, through: u64, more: bool) {
        self.cursors.insert(*node, through);
        if more {
            self.current.remove(node);
        } else {
            self.current.insert(*node);
        }
    }

    /// A post arrived on the session from `node` under `number`: it moves
    /// the cursor once this client has caught up, and before that the
    /// asking in progress is what moves it.
    pub fn delivered(&mut self, node: &Keyhash, number: u64) {
        if self.current.contains(node) && self.cursor(node).is_none_or(|c| number > c) {
            self.cursors.insert(*node, number);
        }
    }

    /// The session with `node` ended: the next one asks again.
    pub fn detached(&mut self, node: &Keyhash) {
        self.current.remove(node);
    }

    /// The pool as it is kept between runs: each post with when it was
    /// dated and whether it was read, and the cursors.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 2);
        emit_array_head(&mut out, self.posts.len());
        for k in self.posts.values() {
            emit_array_head(&mut out, 3);
            emit_bstr(&mut out, &k.post.bytes);
            emit_uint(&mut out, k.dated);
            emit_bool(&mut out, k.read);
        }
        emit_map_head(&mut out, self.cursors.len());
        for (node, n) in &self.cursors {
            emit_bstr(&mut out, node);
            emit_uint(&mut out, *n);
        }
        out
    }

    /// A pool from what [`Pool::encode`] wrote, held to `cap`. Bytes that
    /// do not read are no pool, and the client starts an empty one and
    /// asks its node again.
    #[must_use]
    pub fn decode(cap: usize, b: &[u8]) -> Option<Self> {
        let item = parse_all(b).ok()?;
        let Item::Array(parts) = &item else {
            return None;
        };
        let [Item::Array(posts), Item::Map(cursors)] = parts.as_slice() else {
            return None;
        };
        let mut pool = Pool::new(cap);
        for p in posts {
            let Item::Array(f) = p else { return None };
            let [Item::Bytes(r), Item::Uint(dated), Item::Bool(read)] = f.as_slice() else {
                return None;
            };
            let post = Post::parse(&b[r.clone()]).ok()?;
            let hash = Post::hash(&post.bytes);
            pool.bytes += post.bytes.len();
            pool.posts.insert(
                hash,
                Kept {
                    post,
                    hash,
                    dated: *dated,
                    read: *read,
                },
            );
        }
        for (k, v) in cursors {
            let (Item::Bytes(r), Item::Uint(n)) = (k, v) else {
                return None;
            };
            pool.cursors
                .insert(<[u8; 32]>::try_from(&b[r.clone()]).ok()?, *n);
        }
        Some(pool)
    }
}

/// **A reach read from where the reader stands** [author, 2026-10-27]:
/// the writer's `+3/-3` is `+2/-4` one level above them and `+4/-2` one
/// level below. The band never moves; the count does. Where the two
/// positions are given under different anchors there is nothing to
/// measure by, and the reach is shown as written.
#[must_use]
pub fn reach_from(up: u64, down: u64, writer: &Locator, reader: &Locator) -> (u64, u64) {
    if writer.anchor != reader.anchor {
        return (up, down);
    }
    // how many levels the reader stands above the writer; below is negative
    let above = i128::from(writer.nibbles) - i128::from(reader.nibbles);
    let clamp = |n: i128| u64::try_from(n.max(0)).unwrap_or(u64::MAX);
    (
        clamp(i128::from(up) - above),
        clamp(i128::from(down) + above),
    )
}

// ------------------------------------------------- the client's own boards

use crate::ceremony::{Client, Msg};
use rhtn_archive::board::{BoardPost, BoardReply, BoardRequest, Draft, Writer};
use rhtn_crypto::signer::Sign1;

/// The reach a client offers before the writer changes it (design §21)
/// [author, 2026-10-27].
pub const DEFAULT_REACH: (u64, u64) = (2, 2);

/// The version word the board relation carries first.
const BOARD_VERSION: u64 = 1;

/// The name the board relation is kept under: the pools, a subnet each.
pub const RELATION_BOARD: &str = "board";

/// **The boards a client keeps**, a pool a subnet, and the askings it has
/// in flight.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Boards {
    pools: BTreeMap<Txid, Pool>,
    /// The nonce of the asking each node has not yet answered.
    asked: BTreeMap<Keyhash, [u8; 16]>,
}

impl Boards {
    fn pool(&mut self, subnet: &Txid) -> &mut Pool {
        self.pools
            .entry(*subnet)
            .or_insert_with(|| Pool::new(POOL_BYTES))
    }
}

/// A key a device holding no seed signs a post under, with its
/// delegation: what the transport holds and the kernel does not.
pub type Delegated<'a> = (&'a dyn Sign1, &'a [u8]);

impl Client {
    /// **The node that carries this client's board in `subnet`**: the one
    /// serving it there, or its own where it is the root. Nothing where
    /// this client holds no attachment to it.
    #[must_use]
    pub fn board_node(&self, subnet: &Txid) -> Option<Keyhash> {
        let node = self.board_node_of(subnet);
        self.payload.attachments.contains(&node).then_some(node)
    }

    fn board_node_of(&self, subnet: &Txid) -> Keyhash {
        self.serving_in(subnet).unwrap_or(self.public.keyhash)
    }

    /// The subnet whose board `node` carries for this client.
    fn board_subnet(&self, node: &Keyhash) -> Option<Txid> {
        self.subnets()
            .into_iter()
            .find(|s| self.board_node_of(s) == *node)
    }

    /// **Write a post and hand it to the node serving `subnet`**
    /// (`light-client-requirements.md` §4.5). A post beginning a thread
    /// takes a title and the reach given, `+2/-2` where none is; one
    /// continuing a thread carries that thread's reach, read from where
    /// this client stands, and is offered none. Signed by the identity
    /// where this device holds its seed, and otherwise under the key it
    /// was delegated.
    pub fn board_post(
        &mut self,
        delegated: Option<Delegated<'_>>,
        subnet: &Txid,
        reach: Option<(u64, u64)>,
        thread: Option<[u8; 32]>,
        title: Option<&str>,
        text: &str,
    ) -> Result<Vec<Msg>, String> {
        let position = self
            .position_in(subnet)
            .ok_or("this identity holds no position in that subnet")?;
        let node = self
            .board_node(subnet)
            .ok_or("not attached to the node serving that subnet")?;
        let (up, down) = match thread {
            None => reach.unwrap_or(DEFAULT_REACH),
            Some(id) => {
                // every post of a thread carries its reach, each from
                // where its writer stood: any one of them says the band
                let pool = self.boards.pool(subnet);
                let held = pool.thread(&id);
                let any = held
                    .first()
                    .ok_or("no post of that thread is held to take its reach from")?;
                reach_from(any.post.up, any.post.down, &any.post.position, &position)
            }
        };
        let material = self.public.key_material();
        let alias = self
            .own_alias(subnet)
            .or_else(|| self.default_alias().map(str::to_string));
        let draft = Draft {
            position: &position,
            up,
            down,
            written_at: self.now_s(),
            thread,
            title,
            text,
            alias: alias.as_deref(),
        };
        let bytes = match (self.signer(), delegated) {
            (Ok(identity), _) => Post::build(&Writer::own(identity, &material), &draft),
            (Err(_), Some((signer, delegation))) => Post::build(
                &Writer {
                    signer,
                    material: &material,
                    delegation: Some(delegation),
                },
                &draft,
            ),
            (Err(_), None) => return Err("this device holds no key to sign a post with".into()),
        }
        .ok_or("the post is out of bounds: a title or words too long, or none")?;
        let (me, now) = (self.public.keyhash, self.now_s());
        self.boards.pool(subnet).take(subnet, &me, &bytes, now);
        self.changed.insert(rhtn_model::Noun::Board);
        Ok(vec![Msg::BoardPost {
            node,
            bytes: BoardPost {
                up,
                down,
                post: bytes,
                number: None,
            }
            .encode(),
        }])
    }

    /// **A `BoardPost` arrived on the session with `node`**
    /// (`wire-format.md` §10.3): one for this member carries the node's
    /// number and is taken into the pool of the subnet that node serves.
    /// A copy between nodes, which carries none, is not a client's.
    pub fn board_arrived(&mut self, node: Keyhash, body: &[u8]) -> Option<Took> {
        let frame = BoardPost::decode(body).ok()?;
        let number = frame.number?;
        let subnet = self.board_subnet(&node)?;
        let (me, now) = (self.public.keyhash, self.now_s());
        let pool = self.boards.pool(&subnet);
        let took = pool.take(&subnet, &me, &frame.post, now);
        pool.delivered(&node, number);
        if took == Took::New {
            self.changed.insert(rhtn_model::Noun::Board);
        }
        Some(took)
    }

    /// **Ask `node` for what this client missed** (`wire-format.md` §10.3
    /// request type 17), from the last number it gave: on attaching, and
    /// again while it says more is held. Nothing where that node carries
    /// no board of this client's.
    pub fn board_ask(&mut self, node: Keyhash) -> Vec<Msg> {
        let Some(subnet) = self.board_subnet(&node) else {
            return Vec::new();
        };
        let nonce = self.nonce();
        let after = self.boards.pool(&subnet).cursor(&node);
        self.boards.asked.insert(node, nonce);
        vec![Msg::BoardAsk {
            node,
            bytes: BoardRequest { nonce, after }.encode(),
        }]
    }

    /// **Take `node`'s answer**: each post into the pool, the cursor to
    /// where the reply read through, and the next asking where more is
    /// held. A reply this client did not ask that node for is not taken.
    pub fn take_board_reply(&mut self, node: Keyhash, bytes: &[u8]) -> Result<Vec<Msg>, String> {
        let reply = BoardReply::decode(bytes)?;
        if self.boards.asked.get(&node) != Some(&reply.nonce) {
            return Err("a reply this client did not ask that node for".into());
        }
        self.boards.asked.remove(&node);
        let subnet = self
            .board_subnet(&node)
            .ok_or("that node carries no board of this client's")?;
        let (me, now) = (self.public.keyhash, self.now_s());
        let pool = self.boards.pool(&subnet);
        let mut new = false;
        for (_, post) in &reply.posts {
            new |= pool.take(&subnet, &me, post, now) == Took::New;
        }
        pool.caught_up(&node, reply.through, reply.more);
        if new {
            self.changed.insert(rhtn_model::Noun::Board);
        }
        Ok(if reply.more {
            self.board_ask(node)
        } else {
            Vec::new()
        })
    }

    /// The session with `node` ended: the next one asks again.
    pub fn board_detached(&mut self, node: &Keyhash) {
        self.boards.asked.remove(node);
        for pool in self.boards.pools.values_mut() {
            pool.detached(node);
        }
    }

    /// The threads of `subnet`'s board, the one with the newest post first.
    #[must_use]
    pub fn board_threads(&self, subnet: &Txid) -> Vec<Thread> {
        self.boards
            .pools
            .get(subnet)
            .map(Pool::threads)
            .unwrap_or_default()
    }

    /// One thread's posts, oldest first, each with its reach as this
    /// client reads it from where it stands in the subnet.
    #[must_use]
    pub fn board_thread(&self, subnet: &Txid, id: &[u8; 32]) -> Vec<(Kept, (u64, u64))> {
        let Some(pool) = self.boards.pools.get(subnet) else {
            return Vec::new();
        };
        let here = self.position_in(subnet);
        pool.thread(id)
            .into_iter()
            .map(|k| {
                let reach = match &here {
                    Some(h) => reach_from(k.post.up, k.post.down, &k.post.position, h),
                    None => (k.post.up, k.post.down),
                };
                (k.clone(), reach)
            })
            .collect()
    }

    /// The person has been shown a thread.
    pub fn board_read(&mut self, subnet: &Txid, id: &[u8; 32]) {
        if self.boards.pool(subnet).mark_read(id) > 0 {
            self.changed.insert(rhtn_model::Noun::Board);
        }
    }

    /// How many posts of `subnet`'s board the person has not been shown.
    #[must_use]
    pub fn board_unread(&self, subnet: &Txid) -> usize {
        self.boards.pools.get(subnet).map_or(0, Pool::unread)
    }

    /// **The board relation as it is kept between runs**: every pool,
    /// under its subnet. Kept apart from the relations a start needs, so
    /// a state written before there was a board still opens.
    #[must_use]
    pub fn board_part(&self) -> Vec<u8> {
        let mut out = Vec::new();
        emit_array_head(&mut out, 2);
        emit_uint(&mut out, BOARD_VERSION);
        emit_array_head(&mut out, self.boards.pools.len());
        for (subnet, pool) in &self.boards.pools {
            emit_array_head(&mut out, 2);
            emit_bstr(&mut out, subnet);
            emit_bstr(&mut out, &pool.encode());
        }
        out
    }

    /// Take the board relation back. Bytes that do not read leave the
    /// boards empty, and the nodes are asked again.
    pub fn restore_board(&mut self, b: &[u8]) -> Option<usize> {
        let item = parse_all(b).ok()?;
        let Item::Array(parts) = &item else {
            return None;
        };
        let [Item::Uint(BOARD_VERSION), Item::Array(pools)] = parts.as_slice() else {
            return None;
        };
        let mut read = BTreeMap::new();
        for p in pools {
            let Item::Array(f) = p else { return None };
            let [Item::Bytes(subnet), Item::Bytes(pool)] = f.as_slice() else {
                return None;
            };
            read.insert(
                <[u8; 32]>::try_from(&b[subnet.clone()]).ok()?,
                Pool::decode(POOL_BYTES, &b[pool.clone()])?,
            );
        }
        self.boards.pools = read;
        Some(self.boards.pools.len())
    }
}

// ------------------------------------------------------- the model's nouns

impl Client {
    /// The threads of `subnet`'s board as the model's noun
    /// (`rhtn_model::BoardThread`), the one with the newest post first.
    #[must_use]
    pub fn board_threads_held(&self, subnet: &Txid) -> Vec<rhtn_model::BoardThread> {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        self.board_threads(subnet)
            .into_iter()
            .map(|t| rhtn_model::BoardThread {
                id: t.id,
                title: t.title,
                began_by: t.began_by,
                posts: count(t.posts),
                unread: count(t.unread),
                latest: t.latest,
            })
            .collect()
    }

    /// One thread's posts as the model's noun (`rhtn_model::BoardPost`),
    /// oldest first, each reach read from where this client stands.
    #[must_use]
    pub fn board_thread_held(&self, subnet: &Txid, id: &[u8; 32]) -> Vec<rhtn_model::BoardPost> {
        let count = |n: u64| u32::try_from(n).unwrap_or(u32::MAX);
        let me = self.public.keyhash;
        self.board_thread(subnet, id)
            .into_iter()
            .map(|(k, (up, down))| rhtn_model::BoardPost {
                hash: k.hash,
                thread: k.post.thread_id(),
                writer: k.post.writer,
                alias: k.post.alias.clone(),
                mine: k.post.writer == me,
                text: k.post.text.clone(),
                written_at: k.dated,
                up: count(up),
                down: count(down),
                read: k.read,
            })
            .collect()
    }

    /// **Forget a post this client wrote and could not hand over**: the
    /// node did not take the frame, so nobody else holds it and the pool
    /// does not show it as posted.
    pub fn board_forget(&mut self, subnet: &Txid, post: &[u8]) {
        if let Some(pool) = self.boards.pools.get_mut(subnet) {
            pool.forget(&Post::hash(post));
            self.changed.insert(rhtn_model::Noun::Board);
        }
    }
}
