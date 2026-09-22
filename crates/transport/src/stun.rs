//! STUN Binding (RFC 8489 §5, §14.2), the one STUN transaction the direct
//! payload path needs (design §14.1.1; `wire-format.md` §9.2): a client
//! asks its serving node what address the node sees it at, and the node
//! answers with the source address of the request, XOR-mapped.  Nothing
//! else of STUN is used: no authentication, since the reflexive address is
//! what the network already knows, and no other method.
//!
//! What is here is the encoding to the byte, so a datagram can be told
//! from QUIC on the same socket (RFC 9443 §3: STUN's first two bits are
//! zero, QUIC's first byte is never below 64).

use std::net::{IpAddr, SocketAddr};

pub const MAGIC_COOKIE: u32 = 0x2112_A442;
pub const BINDING_REQUEST: u16 = 0x0001;
pub const BINDING_RESPONSE: u16 = 0x0101;
pub const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
pub const ATTR_FINGERPRINT: u16 = 0x8028;
const FINGERPRINT_XOR: u32 = 0x5354_554e;
pub const HEADER_BYTES: usize = 20;

/// Whether a datagram is a STUN message: the two high bits zero, the magic
/// cookie in place, and the length field consistent with the datagram.
pub fn is_stun(b: &[u8]) -> bool {
    b.len() >= HEADER_BYTES
        && b[0] & 0xC0 == 0
        && u32::from_be_bytes([b[4], b[5], b[6], b[7]]) == MAGIC_COOKIE
        && u16::from_be_bytes([b[2], b[3]]) as usize + HEADER_BYTES == b.len()
}

/// A Binding message's kind, from its message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Request,
    Response,
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn header(msg_type: u16, length: u16, txid: &[u8; 12]) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_BYTES + length as usize);
    out.extend_from_slice(&msg_type.to_be_bytes());
    out.extend_from_slice(&length.to_be_bytes());
    out.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
    out.extend_from_slice(txid);
    out
}

/// Append the FINGERPRINT attribute (§14.7): CRC-32 of the message so far
/// XOR 0x5354554e, the length field already counting it.
fn fingerprint(mut msg: Vec<u8>) -> Vec<u8> {
    let len = (msg.len() - HEADER_BYTES + 8) as u16;
    msg[2..4].copy_from_slice(&len.to_be_bytes());
    let fp = crc32(&msg) ^ FINGERPRINT_XOR;
    msg.extend_from_slice(&ATTR_FINGERPRINT.to_be_bytes());
    msg.extend_from_slice(&4u16.to_be_bytes());
    msg.extend_from_slice(&fp.to_be_bytes());
    msg
}

/// A Binding Request under `txid`, with a FINGERPRINT.
pub fn binding_request(txid: &[u8; 12]) -> Vec<u8> {
    fingerprint(header(BINDING_REQUEST, 0, txid))
}

/// The XOR-MAPPED-ADDRESS attribute for `addr` under `txid` (§14.2): the
/// port XOR the cookie's high half, the address XOR the cookie and, for
/// IPv6, the transaction id.
fn xor_mapped_address(addr: SocketAddr, txid: &[u8; 12]) -> Vec<u8> {
    let mut v = Vec::new();
    let port = addr.port() ^ (MAGIC_COOKIE >> 16) as u16;
    match addr.ip() {
        IpAddr::V4(ip) => {
            v.extend_from_slice(&[0, 1]);
            v.extend_from_slice(&port.to_be_bytes());
            let x = u32::from_be_bytes(ip.octets()) ^ MAGIC_COOKIE;
            v.extend_from_slice(&x.to_be_bytes());
        }
        IpAddr::V6(ip) => {
            v.extend_from_slice(&[0, 2]);
            v.extend_from_slice(&port.to_be_bytes());
            let mut key = MAGIC_COOKIE.to_be_bytes().to_vec();
            key.extend_from_slice(txid);
            for (o, k) in ip.octets().iter().zip(key.iter()) {
                v.push(o ^ k);
            }
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&ATTR_XOR_MAPPED_ADDRESS.to_be_bytes());
    out.extend_from_slice(&(v.len() as u16).to_be_bytes());
    out.extend_from_slice(&v);
    out
}

/// A Binding Response to the request under `txid`, naming `seen`, the
/// source address the request arrived from.
pub fn binding_response(txid: &[u8; 12], seen: SocketAddr) -> Vec<u8> {
    let attr = xor_mapped_address(seen, txid);
    let mut msg = header(BINDING_RESPONSE, attr.len() as u16, txid);
    msg.extend_from_slice(&attr);
    fingerprint(msg)
}

/// A parsed Binding message: its kind, its transaction id, and the mapped
/// address a response carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub kind: Binding,
    pub txid: [u8; 12],
    pub mapped: Option<SocketAddr>,
}

/// Parse a STUN datagram as a Binding message; anything else, or a
/// FINGERPRINT that does not check, is not one.
pub fn parse(b: &[u8]) -> Option<Parsed> {
    if !is_stun(b) {
        return None;
    }
    let kind = match u16::from_be_bytes([b[0], b[1]]) {
        BINDING_REQUEST => Binding::Request,
        BINDING_RESPONSE => Binding::Response,
        _ => return None,
    };
    let txid: [u8; 12] = b[8..20].try_into().ok()?;
    let mut at = HEADER_BYTES;
    let mut mapped = None;
    while at + 4 <= b.len() {
        let t = u16::from_be_bytes([b[at], b[at + 1]]);
        let l = u16::from_be_bytes([b[at + 2], b[at + 3]]) as usize;
        let v = b.get(at + 4..at + 4 + l)?;
        match t {
            ATTR_XOR_MAPPED_ADDRESS => {
                if v.len() < 4 {
                    return None;
                }
                let port = u16::from_be_bytes([v[2], v[3]]) ^ (MAGIC_COOKIE >> 16) as u16;
                let ip = match v[1] {
                    1 if v.len() == 8 => IpAddr::V4(
                        (u32::from_be_bytes([v[4], v[5], v[6], v[7]]) ^ MAGIC_COOKIE).into(),
                    ),
                    2 if v.len() == 20 => {
                        let mut key = MAGIC_COOKIE.to_be_bytes().to_vec();
                        key.extend_from_slice(&txid);
                        let mut o = [0u8; 16];
                        for (i, (x, k)) in v[4..20].iter().zip(key.iter()).enumerate() {
                            o[i] = x ^ k;
                        }
                        IpAddr::V6(o.into())
                    }
                    _ => return None,
                };
                mapped = Some(SocketAddr::new(ip, port));
            }
            ATTR_FINGERPRINT => {
                if v.len() != 4 {
                    return None;
                }
                let want = crc32(&b[..at]) ^ FINGERPRINT_XOR;
                if u32::from_be_bytes([v[0], v[1], v[2], v[3]]) != want {
                    return None;
                }
            }
            _ => {}
        }
        at += 4 + l.div_ceil(4) * 4;
    }
    Some(Parsed { kind, txid, mapped })
}
