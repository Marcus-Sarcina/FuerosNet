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
