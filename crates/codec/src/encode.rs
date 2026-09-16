//! Deterministic emission (§1.2): shortest heads, definite lengths.

pub fn emit_head(out: &mut Vec<u8>, mt: u8, n: u64) {
    let m = mt << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n < 0x100 {
        out.push(m | 24);
        out.push(n as u8);
    } else if n < 0x10000 {
        out.push(m | 25);
        out.extend_from_slice(&(n as u16).to_be_bytes());
    } else if n < 0x1_0000_0000 {
        out.push(m | 26);
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&n.to_be_bytes());
    }
}

pub fn emit_uint(out: &mut Vec<u8>, n: u64) {
    emit_head(out, 0, n);
}

/// Negative integer `v` (v < 0) as major type 1.
pub fn emit_neg(out: &mut Vec<u8>, v: i64) {
    debug_assert!(v < 0);
    emit_head(out, 1, (-1 - v) as u64);
}

pub fn emit_bstr(out: &mut Vec<u8>, b: &[u8]) {
    emit_head(out, 2, b.len() as u64);
    out.extend_from_slice(b);
}

pub fn emit_tstr(out: &mut Vec<u8>, s: &str) {
    emit_head(out, 3, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

pub fn emit_array_head(out: &mut Vec<u8>, n: usize) {
    emit_head(out, 4, n as u64);
}

pub fn emit_map_head(out: &mut Vec<u8>, n: usize) {
    emit_head(out, 5, n as u64);
}

pub fn emit_bool(out: &mut Vec<u8>, b: bool) {
    out.push(if b { 0xf5 } else { 0xf4 });
}

pub fn emit_null(out: &mut Vec<u8>) {
    out.push(0xf6);
}
