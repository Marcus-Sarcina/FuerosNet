//! Decode what the wire carries and print it.
//!
//! `rhtn-codec` already parses every family strictly and names its reason
//! for refusing. This turns that into something a person reads: bytes in,
//! the object's shape, its verdict under the kind it claims to be, and the
//! values that are derived rather than carried.
//!
//! **It decodes with the parser a node uses and no other.** A second,
//! laxer decoder written for convenience would disagree with the first,
//! and the disagreement would be invisible: the point of looking at bytes
//! with this is to see what a node would see.

use rhtn_codec::cbor::{Error, Item, parse_all};
use rhtn_codec::cose;
use rhtn_codec::frame::{self, Stream};
use rhtn_codec::schema;
use rhtn_crypto::verify::{self, Lookup};

/// What the bytes were read as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum As<'a> {
    /// A framed message on a stream: the type tag decides the family.
    Frame(Stream),
    /// A standalone object of a named kind, as `wire-format.md` names it.
    Kind(&'a str),
    /// Nothing declared: the shape alone.
    Shape,
}

/// Read a blob as hex text where every byte of it is one, and as raw bytes
/// otherwise.  A corpus entry is hex and a capture is not, and telling
/// them apart by looking costs nothing.
pub fn read_blob(bytes: &[u8]) -> Vec<u8> {
    let text: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    if !text.is_empty()
        && text.len().is_multiple_of(2)
        && text.iter().all(|c| c.is_ascii_hexdigit())
    {
        return (0..text.len() / 2)
            .map(|i| {
                u8::from_str_radix(std::str::from_utf8(&text[i * 2..i * 2 + 2]).unwrap(), 16)
                    .unwrap()
            })
            .collect();
    }
    bytes.to_vec()
}

/// Decode `raw` and describe it.  The report is lines a person reads; a
/// refusal is one of them and not an error, because the reason the strict
/// decoder gives is the useful output.
pub fn describe<L: Lookup + ?Sized>(raw: &[u8], what: As, ids: &L) -> String {
    let mut out = String::new();
    out.push_str(&format!("bytes     {}\n", raw.len()));
    if let As::Frame(stream) = what {
        return match frame::parse_payload(stream, raw) {
            Err(Error(e)) => out + &format!("frame     refused: {e}\n"),
            Ok(f) => {
                out.push_str(&format!(
                    "frame     type {} on the {} stream\n",
                    f.frame_type,
                    stream_name(stream)
                ));
                out.push_str(&format!(
                    "family    {}\n",
                    f.family
                        .map(|x| format!("{x:?}"))
                        .unwrap_or_else(|| "unknown, and skipped".into())
                ));
                out.push_str(&format!("body      {} bytes\n", f.body.len()));
                out + &diagnostic(&f.body_item, &raw[f.body.clone()], f.body.start)
            }
        };
    }
    let item = match parse_all(raw) {
        Err(Error(e)) => return out + &format!("cbor      refused: {e}\n"),
        Ok(i) => i,
    };
    if let As::Kind(kind) = what {
        out.push_str(&format!("kind      {kind}\n"));
        match kind {
            "envelope" => out.push_str(&envelope_lines(raw, ids)),
            "presentation" => out.push_str(&format!(
                "verdict   {}\n",
                verdict(verify::presentation(ids, raw))
            )),
            k => {
                out.push_str(&format!(
                    "schema    {}\n",
                    said(schema::check_kind(raw, k, &item).map_err(|e| e.0.to_string()))
                ));
                if is_signed_kind(k) {
                    out.push_str(&format!(
                        "signature {}\n",
                        signature(verify::record(ids, k, raw))
                    ));
                } else if k == "Delegation" {
                    // hybrid, under the delegating identity (`wire-format.md` §8.2)
                    out.push_str(&format!(
                        "signature {}\n",
                        signature(verify::delegation(ids, raw).map(|_| ()))
                    ));
                }
            }
        }
    }
    out + &diagnostic(&item, raw, 0)
}

/// The kinds carrying a signature of their own that a holder checks
/// (`wire-format.md` §7): everything else is structure alone.
fn is_signed_kind(kind: &str) -> bool {
    matches!(
        kind,
        "CurrencyAttestation"
            | "CatalogEntry"
            | "AbuseReport"
            | "AnchorEntry"
            | "SubtreeAck"
            | "PrekeyBundle"
            | "EndpointRecord"
            | "SignedLocator"
    )
}

fn envelope_lines<L: Lookup + ?Sized>(raw: &[u8], ids: &L) -> String {
    let mut out = String::new();
    match rhtn_codec::envelope::parse(raw) {
        Err(e) => return format!("envelope  refused: {e}\n"),
        Ok(env) => {
            // the txid is over the body map and derived, never carried (§1)
            out.push_str(&format!(
                "txid      {}\n",
                hex(&cose::txid(&raw[env.body.clone()]))
            ));
            out.push_str(&format!(
                "type      {} ({})\n",
                env.tx_type,
                tx_name(env.tx_type)
            ));
            out.push_str(&format!("signers   {}\n", env.signers.len()));
            for s in &env.signers {
                out.push_str(&format!("  {}\n", hex(s)));
            }
        }
    }
    out.push_str(&format!(
        "verdict   {}\n",
        verdict(verify::envelope(ids, raw).map(|_| ()))
    ));
    out
}

/// A verdict that keeps §3.4's distinction: a key this holder lacks makes
/// an object unverifiable here, which is not the same as one that fails.
fn verdict(r: Result<(), verify::Failure>) -> String {
    match r {
        Ok(()) => "verified".into(),
        Err(verify::Failure::MissingKey(k)) => format!("unverifiable: no key held for {}", hex(&k)),
        Err(e) => format!("refused: {e}"),
    }
}

fn tx_name(t: u64) -> &'static str {
    match t {
        1 => "adoption",
        2 => "departure",
        3 => "disavowal",
        4 => "peering",
        5 => "presence",
        7 => "series reissue",
        _ => "not a transaction type this version knows",
    }
}

fn stream_name(s: Stream) -> &'static str {
    match s {
        Stream::Control => "control",
        Stream::Request => "request",
    }
}

fn said(r: Result<(), String>) -> String {
    match r {
        Ok(()) => "accepted".into(),
        Err(e) => format!("refused: {e}"),
    }
}

fn signature(r: Result<(), verify::Failure>) -> String {
    match r {
        Ok(()) => "verified".into(),
        // a key this holder lacks is not a failure: the object is
        // unverifiable here and may verify elsewhere (`wire-format.md` §3.4)
        Err(verify::Failure::MissingKey(k)) => format!("unverifiable: no key held for {}", hex(&k)),
        Err(e) => format!("refused: {e}"),
    }
}

/// CBOR as a person reads it, close to RFC 8949's diagnostic notation.
/// Byte strings show their length and their first bytes, since a 1952-byte
/// key printed in full is not something anyone reads.
pub fn diagnostic(item: &Item, raw: &[u8], base: usize) -> String {
    let mut s = String::new();
    write_item(&mut s, item, raw, base, 0);
    s.push('\n');
    s
}

fn write_item(s: &mut String, item: &Item, raw: &[u8], base: usize, depth: usize) {
    let pad = "  ".repeat(depth);
    match item {
        Item::Uint(u) => s.push_str(&u.to_string()),
        Item::Neg(n) => s.push_str(&n.to_string()),
        Item::Bool(b) => s.push_str(if *b { "true" } else { "false" }),
        Item::Null => s.push_str("null"),
        Item::Text(r) => s.push_str(&format!(
            "{:?}",
            String::from_utf8_lossy(slice(raw, base, r))
        )),
        Item::Bytes(r) => {
            let b = slice(raw, base, r);
            match b.len() {
                0 => s.push_str("h''"),
                n if n <= 32 => s.push_str(&format!("h'{}'", hex(b))),
                n => s.push_str(&format!("h'{}...' ({n} bytes)", hex(&b[..16]))),
            }
        }
        Item::Array(items) => {
            if items.is_empty() {
                s.push_str("[]");
                return;
            }
            s.push_str("[\n");
            for it in items {
                s.push_str(&"  ".repeat(depth + 1));
                write_item(s, it, raw, base, depth + 1);
                s.push_str(",\n");
            }
            s.push_str(&pad);
            s.push(']');
        }
        Item::Map(kv) => {
            if kv.is_empty() {
                s.push_str("{}");
                return;
            }
            s.push_str("{\n");
            for (k, v) in kv {
                s.push_str(&"  ".repeat(depth + 1));
                write_item(s, k, raw, base, depth + 1);
                s.push_str(": ");
                write_item(s, v, raw, base, depth + 1);
                s.push_str(",\n");
            }
            s.push_str(&pad);
            s.push('}');
        }
    }
}

/// A range is over the whole message; a frame's body is a window into it,
/// so its items index from the body's start.
fn slice<'a>(raw: &'a [u8], base: usize, r: &std::ops::Range<usize>) -> &'a [u8] {
    let (a, b) = (r.start.saturating_sub(base), r.end.saturating_sub(base));
    raw.get(a..b).unwrap_or(&[])
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
