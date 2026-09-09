//! Frames (§8.0, §9.2): `u32-be length || deterministic CBOR of [type, body]`,
//! on stream 0 (control) or a bidirectional stream (request), and the unsigned
//! message families they carry, which reject unknown map keys (§1.2).

use crate::bounds;
use crate::cbor::*;
use crate::schema::{self, Family};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Control,
    Request,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub stream: Stream,
    pub frame_type: u64,
    /// The message family the type names on this stream, if known.
    pub family: Option<Family>,
    /// The body's bytes within the payload (after the length prefix).
    pub body: std::ops::Range<usize>,
    pub body_item: Item,
}

/// Split the length prefix and check it against the input and the stream's
/// ceiling.  Returns the payload.
pub fn payload(stream: Stream, raw: &[u8]) -> Result<&[u8], Error> {
    if raw.len() < 4 {
        return Err(Error("frame too short"));
    }
    let n = u32::from_be_bytes(raw[..4].try_into().unwrap()) as usize;
    let ceiling = match stream {
        Stream::Control => bounds::CONTROL_FRAME_BYTES,
        Stream::Request => bounds::REQUEST_FRAME_BYTES,
    };
    if n > ceiling {
        return Err(Error("frame over the stream bound"));
    }
    if n != raw.len() - 4 {
        return Err(Error("length prefix mismatch"));
    }
    Ok(&raw[4..])
}

/// Which family a type names on each stream (§8.0's table; §9.2's).
pub fn family_of(stream: Stream, frame_type: u64) -> Option<Family> {
    use Family::*;
    Some(match (stream, frame_type) {
        (Stream::Control, 1) => Attach,
        (Stream::Control, 2) => AttachAck,
        (Stream::Control, 3) => Heartbeat,
        (Stream::Control, 4) => SiblingUpdate,
        (Stream::Control, 5) => TopologyPush,
        (Stream::Control, 6) => TopologyMemo,
        (Stream::Request, 1) => ResolveRequest,
        (Stream::Request, 2) => ArchiveRequest,
        (Stream::Request, 3) => PrekeyRequestOrBatch,
        (Stream::Request, 4) => VerifierQuery,
        (Stream::Request, 5) => CatalogQuery,
        (Stream::Request, 6) => ResourceRequest,
        (Stream::Request, 7) => ResourceRegistration,
        (Stream::Request, 8) => CurrencyRequest,
        _ => return None,
    })
}

/// Parse a frame's payload (the bytes after the length prefix) on `stream`,
/// validating the body against its family's schema where the type is known.
/// An unknown control type is skipped by the session layer (§8.0) and is
/// reported here as `family: None` with the body left unvalidated; an unknown
/// request type is the stream's failure (§9.2).
pub fn parse_payload(stream: Stream, p: &[u8]) -> Result<Frame, Error> {
    let mut item = parse_all(p)?;
    // move the body out rather than cloning it: a clone recurses per nesting level
    let (frame_type, body_item) = match &mut item {
        Item::Array(a) if a.len() == 2 => {
            let frame_type = as_uint(&a[0]).ok_or(Error("frame type not uint"))?;
            (frame_type, a.pop().unwrap())
        }
        Item::Array(_) => return Err(Error("frame arity")),
        _ => return Err(Error("frame not array")),
    };
    let ranges = array_item_ranges(p, 0).ok_or(Error("frame walk"))?;
    let body = ranges[1].clone();
    let family = family_of(stream, frame_type);
    match family {
        None if stream == Stream::Request => return Err(Error("unknown request type")),
        None => {}
        Some(f) => schema::check_unsigned(f, p, body.start)?,
    }
    Ok(Frame { stream, frame_type, family, body, body_item })
}

/// Parse a complete frame including its length prefix.
pub fn parse(stream: Stream, raw: &[u8]) -> Result<Frame, Error> {
    let p = payload(stream, raw)?;
    parse_payload(stream, p)
}
