//! Deterministic CBOR (§1.2), validated on the received bytes.
//!
//! The parser keeps the bytes and returns a tree of ranges into them, so an
//! unknown key's value is an opaque slice preserved exactly as it arrived.
//! It is iterative: nesting depth is bounded by the input length and never by
//! the machine stack, so no input can end the process.  Every arithmetic step
//! on a declared length is checked, and no declared size is trusted before
//! the bytes behind it are present.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Uint(u64),
    Neg(i64),
    Bytes(Range<usize>),
    Text(Range<usize>),
    Array(Vec<Item>),
    /// Key items paired with value items, in received order (which the
    /// profile requires to be ascending bytewise).
    Map(Vec<(Item, Item)>),
    Bool(bool),
    Null,
}

/// Dropping is iterative for the same reason parsing is: the derived
/// destructor would recurse once per nesting level, and a 56 KB input of
/// nested one-element arrays overflowed the stack that way under the fuzzer.
/// Children are drained into a worklist first, so each item is shallow when
/// its own destructor runs.
impl Drop for Item {
    fn drop(&mut self) {
        let mut work: Vec<Item> = Vec::new();
        match self {
            Item::Array(items) => work.append(items),
            Item::Map(pairs) => {
                for (k, v) in pairs.drain(..) {
                    work.push(k);
                    work.push(v);
                }
            }
            _ => {}
        }
        while let Some(mut it) = work.pop() {
            match &mut it {
                Item::Array(items) => work.append(items),
                Item::Map(pairs) => {
                    for (k, v) in pairs.drain(..) {
                        work.push(k);
                        work.push(v);
                    }
                }
                _ => {}
            }
        }
    }
}

/// Why an input is malformed.  The message names the first rule broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(pub &'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}

pub struct Parser<'a> {
    pub b: &'a [u8],
}

enum Frame {
    Array { remaining: u64, items: Vec<Item> },
    Map {
        remaining: u64,
        pairs: Vec<(Item, Item)>,
        /// key parsed, value pending
        pending_key: Option<Item>,
        prev_key: Option<Range<usize>>,
        key_start: usize,
    },
}

impl<'a> Parser<'a> {
    /// (major type, argument, head length) with the shortest-form rule.
    pub fn head(&self, p: usize) -> Result<(u8, u64, usize), Error> {
        let ib = *self.b.get(p).ok_or(Error("eof"))?;
        let (mt, ai) = (ib >> 5, ib & 0x1f);
        let (n, adv): (u64, usize) = match ai {
            0..=23 => (ai as u64, 1),
            24 => {
                let v = *self.b.get(p + 1).ok_or(Error("eof"))? as u64;
                if v < 24 {
                    return Err(Error("non-shortest u8"));
                }
                (v, 2)
            }
            25 => {
                let s = self.b.get(p + 1..p + 3).ok_or(Error("eof"))?;
                let v = u16::from_be_bytes([s[0], s[1]]) as u64;
                if v < 0x100 {
                    return Err(Error("non-shortest u16"));
                }
                (v, 3)
            }
            26 => {
                let s = self.b.get(p + 1..p + 5).ok_or(Error("eof"))?;
                let v = u32::from_be_bytes([s[0], s[1], s[2], s[3]]) as u64;
                if v < 0x10000 {
                    return Err(Error("non-shortest u32"));
                }
                (v, 5)
            }
            27 => {
                let s = self.b.get(p + 1..p + 9).ok_or(Error("eof"))?;
                let v = u64::from_be_bytes(s.try_into().unwrap());
                if v < 0x1_0000_0000 {
                    return Err(Error("non-shortest u64"));
                }
                (v, 9)
            }
            _ => return Err(Error("indefinite or reserved length")),
        };
        Ok((mt, n, adv))
    }

    /// Parse one item starting at `p`; returns it and the offset after it.
    pub fn item(&self, p: usize) -> Result<(Item, usize), Error> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut at = p;
        loop {
            // parse one leaf or open one container at `at`
            let ib = *self.b.get(at).ok_or(Error("eof"))?;
            let (mut done, next): (Option<Item>, usize) = match ib {
                0xf4 => (Some(Item::Bool(false)), at + 1),
                0xf5 => (Some(Item::Bool(true)), at + 1),
                0xf6 => (Some(Item::Null), at + 1),
                _ => {
                    let (mt, n, adv) = self.head(at)?;
                    let q = at + adv;
                    match mt {
                        0 => (Some(Item::Uint(n)), q),
                        1 => {
                            if n > i64::MAX as u64 {
                                return Err(Error("negative integer out of range"));
                            }
                            (Some(Item::Neg(-1 - n as i64)), q)
                        }
                        2 | 3 => {
                            let len = usize::try_from(n).map_err(|_| Error("length overrun"))?;
                            let end = q.checked_add(len).ok_or(Error("length overrun"))?;
                            if end > self.b.len() {
                                return Err(Error(if mt == 2 { "bstr overrun" } else { "tstr overrun" }));
                            }
                            if mt == 3 {
                                std::str::from_utf8(&self.b[q..end]).map_err(|_| Error("bad utf8"))?;
                                (Some(Item::Text(q..end)), end)
                            } else {
                                (Some(Item::Bytes(q..end)), end)
                            }
                        }
                        4 => {
                            if n == 0 {
                                (Some(Item::Array(Vec::new())), q)
                            } else {
                                stack.push(Frame::Array { remaining: n, items: Vec::new() });
                                (None, q)
                            }
                        }
                        5 => {
                            if n == 0 {
                                (Some(Item::Map(Vec::new())), q)
                            } else {
                                stack.push(Frame::Map {
                                    remaining: n,
                                    pairs: Vec::new(),
                                    pending_key: None,
                                    prev_key: None,
                                    key_start: q,
                                });
                                (None, q)
                            }
                        }
                        7 => return Err(Error("unsupported simple/float")),
                        _ => return Err(Error("tag not admitted by profile")),
                    }
                }
            };
            at = next;
            // deliver completed items upward until something is still open
            while let Some(item) = done.take() {
                match stack.last_mut() {
                    None => return Ok((item, at)),
                    Some(Frame::Array { remaining, items }) => {
                        items.push(item);
                        *remaining -= 1;
                        if *remaining == 0 {
                            let Some(Frame::Array { items, .. }) = stack.pop() else { unreachable!() };
                            done = Some(Item::Array(items));
                        }
                    }
                    Some(Frame::Map { remaining, pairs, pending_key, prev_key, key_start }) => {
                        if pending_key.is_none() {
                            // `item` is a key; its bytes ran key_start..at
                            let key_bytes = *key_start..at;
                            if let Some(pr) = prev_key {
                                if self.b[key_bytes.clone()] <= self.b[pr.clone()] {
                                    return Err(Error("map keys unsorted or duplicate"));
                                }
                            }
                            *prev_key = Some(key_bytes);
                            *pending_key = Some(item);
                        } else {
                            let k = pending_key.take().unwrap();
                            pairs.push((k, item));
                            *remaining -= 1;
                            *key_start = at;
                            if *remaining == 0 {
                                let Some(Frame::Map { pairs, .. }) = stack.pop() else { unreachable!() };
                                done = Some(Item::Map(pairs));
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Parse a complete input: one item and nothing after it.
pub fn parse_all(b: &[u8]) -> Result<Item, Error> {
    let p = Parser { b };
    let (item, end) = p.item(0)?;
    if end != b.len() {
        return Err(Error("trailing bytes"));
    }
    Ok(item)
}

/// Range of the encoded value for integer key `key` in the map at `at0`.
pub fn value_slice_at(b: &[u8], at0: usize, key: u64) -> Option<Range<usize>> {
    let p = Parser { b };
    let (mt, n, adv) = p.head(at0).ok()?;
    if mt != 5 {
        return None;
    }
    let mut at = at0 + adv;
    for _ in 0..n {
        let (k, kend) = p.item(at).ok()?;
        let (_, vend) = p.item(kend).ok()?;
        if matches!(k, Item::Uint(x) if x == key) {
            return Some(kend..vend);
        }
        at = vend;
    }
    None
}

/// Range of the encoded value for integer key `key` in a top-level map.
pub fn value_slice(b: &[u8], key: u64) -> Option<Range<usize>> {
    value_slice_at(b, 0, key)
}

/// (key range, value range) of every entry of the map at `at0`, in order.
pub fn map_entry_ranges(b: &[u8], at0: usize) -> Option<Vec<(Range<usize>, Range<usize>)>> {
    let p = Parser { b };
    let (mt, n, adv) = p.head(at0).ok()?;
    if mt != 5 {
        return None;
    }
    let mut out = Vec::new();
    let mut at = at0 + adv;
    for _ in 0..n {
        let (_, kend) = p.item(at).ok()?;
        let (_, vend) = p.item(kend).ok()?;
        out.push((at..kend, kend..vend));
        at = vend;
    }
    Some(out)
}

/// Item ranges of the array at `at0`.
pub fn array_item_ranges(b: &[u8], at0: usize) -> Option<Vec<Range<usize>>> {
    let p = Parser { b };
    let (mt, n, adv) = p.head(at0).ok()?;
    if mt != 4 {
        return None;
    }
    let mut out = Vec::new();
    let mut at = at0 + adv;
    for _ in 0..n {
        let (_, next) = p.item(at).ok()?;
        out.push(at..next);
        at = next;
    }
    Some(out)
}

/// The map at `b` with one integer key removed, head count fixed.  Every
/// other entry is copied as received, unknown keys included (§1.2).
pub fn map_without_key(b: &[u8], key: u64) -> Option<Vec<u8>> {
    let p = Parser { b };
    let (mt, n, adv) = p.head(0).ok()?;
    if mt != 5 {
        return None;
    }
    let mut body = Vec::new();
    let mut kept = 0u64;
    let mut at = adv;
    for _ in 0..n {
        let ks = at;
        let (k, kend) = p.item(at).ok()?;
        let (_, vend) = p.item(kend).ok()?;
        if !matches!(k, Item::Uint(x) if x == key) {
            body.extend_from_slice(&b[ks..vend]);
            kept += 1;
        }
        at = vend;
    }
    let mut out = Vec::new();
    crate::encode::emit_head(&mut out, 5, kept);
    out.extend_from_slice(&body);
    Some(out)
}

pub fn map_get<'m>(m: &'m [(Item, Item)], key: u64) -> Option<&'m Item> {
    m.iter().find_map(|(k, v)| match k {
        Item::Uint(x) if *x == key => Some(v),
        _ => None,
    })
}

pub fn as_uint(it: &Item) -> Option<u64> {
    match it {
        Item::Uint(n) => Some(*n),
        _ => None,
    }
}

/// Re-emit a parsed item deterministically, copying string bodies from
/// `src`.  For an input the parser accepted this reproduces it byte for byte,
/// which is the round-trip property §1.2's preservation rule needs.  Iterative
/// for the same reason the parser is.
pub fn reencode(item: &Item, src: &[u8]) -> Vec<u8> {
    use crate::encode::*;
    let mut out = Vec::new();
    // pre-order walk: a container emits its head, then its children in order
    enum Node<'i> {
        One(&'i Item),
        Pair(&'i Item, &'i Item),
    }
    let mut stack: Vec<Node> = vec![Node::One(item)];
    while let Some(n) = stack.pop() {
        let it = match n {
            Node::One(it) => it,
            Node::Pair(k, v) => {
                stack.push(Node::One(v));
                k
            }
        };
        match it {
            Item::Uint(n) => emit_uint(&mut out, *n),
            Item::Neg(v) => emit_neg(&mut out, *v),
            Item::Bytes(r) => emit_bstr(&mut out, &src[r.clone()]),
            Item::Text(r) => {
                emit_head(&mut out, 3, r.len() as u64);
                out.extend_from_slice(&src[r.clone()]);
            }
            Item::Bool(b) => emit_bool(&mut out, *b),
            Item::Null => emit_null(&mut out),
            Item::Array(items) => {
                emit_array_head(&mut out, items.len());
                for child in items.iter().rev() {
                    stack.push(Node::One(child));
                }
            }
            Item::Map(pairs) => {
                emit_map_head(&mut out, pairs.len());
                for (k, v) in pairs.iter().rev() {
                    stack.push(Node::Pair(k, v));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod deep {
    use super::*;

    /// A hundred thousand nested one-element arrays: parsed, re-encoded and
    /// dropped without recursion.  The fuzzer found a 56 KB input of this
    /// shape overflowing the stack in the derived destructor.
    #[test]
    fn deep_nesting_parses_round_trips_and_drops_iteratively() {
        let mut deep = vec![0x81u8; 100_000];
        deep.push(0x80);
        let item = parse_all(&deep).expect("parses");
        assert_eq!(reencode(&item, &deep), deep);
        drop(item);
        // and through the frame layer, which moves the body rather than cloning it
        let mut payload = vec![0x82, 0x18, 0x63];
        payload.extend_from_slice(&deep);
        let f = crate::frame::parse_payload(crate::frame::Stream::Control, &payload).expect("unknown type, deep body");
        assert_eq!(f.frame_type, 99);
        drop(f);
    }
}
