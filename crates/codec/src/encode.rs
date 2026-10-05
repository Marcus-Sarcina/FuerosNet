//! Deterministic emission (§1.2): shortest heads, definite lengths.

/// A CBOR head: the major type in the top three bits, then the argument
/// `n` in the **shortest form that holds it** (§1.2).  Every other emitter
/// here goes through this one, so determinism is decided in a single
/// place and cannot be got wrong per type.
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

/// An unsigned integer (major type 0).
pub fn emit_uint(out: &mut Vec<u8>, n: u64) {
    emit_head(out, 0, n);
}

/// Negative integer `v` (v < 0) as major type 1.
pub fn emit_neg(out: &mut Vec<u8>, v: i64) {
    debug_assert!(v < 0);
    emit_head(out, 1, (-1 - v) as u64);
}

/// A byte string (major type 2), its length definite.
pub fn emit_bstr(out: &mut Vec<u8>, b: &[u8]) {
    emit_head(out, 2, b.len() as u64);
    out.extend_from_slice(b);
}

/// A text string (major type 3), its length definite and in bytes.
pub fn emit_tstr(out: &mut Vec<u8>, s: &str) {
    emit_head(out, 3, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

/// An array head of `n` items (major type 4).  **Definite length only**:
/// §1.2 admits no indefinite form, so the count is known before the items
/// are written and a caller that writes a different number is wrong.
pub fn emit_array_head(out: &mut Vec<u8>, n: usize) {
    emit_head(out, 4, n as u64);
}

/// A map head of `n` pairs (major type 5).  Definite length, as arrays;
/// the keys the caller then writes must ascend (§1.2).
pub fn emit_map_head(out: &mut Vec<u8>, n: usize) {
    emit_head(out, 5, n as u64);
}

/// A boolean: the simple values 20 and 21.
pub fn emit_bool(out: &mut Vec<u8>, b: bool) {
    out.push(if b { 0xf5 } else { 0xf4 });
}

/// Null: simple value 22.
pub fn emit_null(out: &mut Vec<u8>) {
    out.push(0xf6);
}
