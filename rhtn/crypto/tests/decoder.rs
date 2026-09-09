//! Acceptance entries for the decoder (`rhtn/acceptance/acceptance.json`,
//! area `decoder`), over the corpus fixtures.  Each test names its entry.
//!
//! Budgets: the entries parameterise a per-input time budget T and a memory
//! budget M that no document sets.  T is enforced below.  M is bounded by
//! construction rather than measured: the parser allocates in proportion to
//! the bytes present and never to a declared length (DEC-03 shows the
//! declared-length case returning at once).

mod common;
use common::*;
use rhtn_codec::cbor::*;
use rhtn_codec::encode::*;
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(1);

fn accept_set() -> Vec<Fixture> {
    fixtures().into_iter().filter(|f| f.outcome == "accept").collect()
}

fn timed<R>(f: impl FnOnce() -> R) -> R {
    let t0 = Instant::now();
    let r = f();
    assert!(t0.elapsed() < T, "decode exceeded the time budget");
    r
}

// acceptance: DEC-01
#[test]
fn dec_01_every_proper_prefix_is_malformed() {
    let ids = identities();
    let mut prefixes = 0usize;
    for f in accept_set() {
        for n in 0..f.bytes.len() {
            let r = timed(|| decode(&ids, &f.id, &f.kind, &f.bytes[..n]));
            assert!(r.is_err(), "{}: prefix of {n} bytes accepted", f.id);
            prefixes += 1;
        }
    }
    assert!(prefixes > 100_000, "{prefixes} prefixes tried");
}

/// A seeded xorshift64 generator, so a run is reproducible from S.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % (n as u64).max(1)) as usize
    }
}

const SEED: u64 = 0x5eed_2026_0909;
const VARIANTS: usize = 48;

// acceptance: DEC-02
#[test]
fn dec_02_seeded_mutations_get_a_verdict_and_valid_ones_round_trip() {
    let ids = identities();
    let set = accept_set();
    let mut rng = Rng(SEED);
    let (mut valid, mut malformed) = (0usize, 0usize);
    for (i, f) in set.iter().enumerate() {
        for _ in 0..VARIANTS {
            let mut v = f.bytes.clone();
            match rng.below(4) {
                0 => {
                    let p = rng.below(v.len());
                    v[p] = rng.next() as u8;
                }
                1 => {
                    let p = rng.below(v.len() + 1);
                    v.insert(p, rng.next() as u8);
                }
                2 => {
                    let p = rng.below(v.len());
                    v.remove(p);
                }
                _ => {
                    let other = &set[(i + 1 + rng.below(set.len() - 1)) % set.len()].bytes;
                    let len = 1 + rng.below(32.min(other.len()));
                    let src = rng.below(other.len() - len + 1);
                    let dst = rng.below(v.len() + 1);
                    let run = other[src..src + len].to_vec();
                    v.splice(dst..dst, run);
                }
            }
            let verdict = timed(|| decode(&ids, &f.id, &f.kind, &v));
            match verdict {
                Ok(()) => {
                    let span = canonical_span(&f.kind, &v);
                    let item = parse_all(span).expect("valid means parsed");
                    assert_eq!(reencode(&item, span), span, "{}: a valid variant must round-trip", f.id);
                    valid += 1;
                }
                Err(_) => malformed += 1,
            }
        }
    }
    eprintln!("DEC-02: seed {SEED:#x}, {VARIANTS} variants per entry: {valid} valid, {malformed} malformed");
    assert_eq!(valid + malformed, set.len() * VARIANTS);
}

// acceptance: DEC-03
#[test]
fn dec_03_declared_lengths_beyond_the_input_are_rejected_at_once() {
    let ids = identities();
    let body = body_of(&fixture("P-adopt-min").bytes);
    // (a) field 1's keyhash with a 2^64-1 byte-string head
    let mut a_val = vec![0x5b];
    a_val.extend_from_slice(&u64::MAX.to_be_bytes());
    a_val.extend_from_slice(&[0u8; 32]);
    let a = replace_value(&body, 0, 1, &a_val);
    // (b) field 0's list with a 2^32-1 element array head and one element
    let r0 = value_slice(&body, 0).unwrap();
    let first = array_item_ranges(&body, r0.start).unwrap()[0].clone();
    let mut b_val = vec![0x9a, 0xff, 0xff, 0xff, 0xff];
    b_val.extend_from_slice(&body[first]);
    let b = replace_value(&body, 0, 0, &b_val);
    // (c) the body's map head declaring 2^32-1 entries over the original entries
    let (_, _, adv) = Parser { b: &body }.head(0).unwrap();
    let mut c = vec![0xba, 0xff, 0xff, 0xff, 0xff];
    c.extend_from_slice(&body[adv..]);
    for (name, input) in [("bstr", a), ("array", b), ("map", c)] {
        let r = timed(|| decode(&ids, "P-adopt-min", "body", &input));
        assert!(r.is_err(), "{name}: declared length beyond the input was accepted");
    }
}

// acceptance: DEC-04
#[test]
fn dec_04_self_describe_tag_is_rejected() {
    let ids = identities();
    let env = fixture("P-adopt-min").bytes;
    let tag = [0xd9, 0xd9, 0xf7];
    let tagged = |b: &[u8]| { let mut v = tag.to_vec(); v.extend_from_slice(b); v };
    assert!(decode(&ids, "P-adopt-min", "envelope", &tagged(&env)).is_err());
    assert!(decode(&ids, "P-adopt-min", "envelope", &env).is_ok());
    let body = body_of(&env);
    assert!(decode(&ids, "P-adopt-min", "body", &tagged(&body)).is_err());
    assert!(decode(&ids, "P-adopt-min", "body", &body).is_ok());
    let frame = fixture("P-frame-01").bytes;
    assert!(decode(&ids, "P-frame-01", "frame", &framed(&tagged(&frame[4..]))).is_err());
    assert!(decode(&ids, "P-frame-01", "frame", &frame).is_ok());
}

// acceptance: DEC-07
#[test]
fn dec_07_catalog_reply_with_112_entries_is_rejected() {
    let ids = identities();
    let reply = fixture("P-catalog-reply-truncated").bytes;
    assert!(decode(&ids, "", "CatalogReply", &reply).is_ok());
    let r2 = value_slice(&reply, 2).unwrap();
    let entries = array_item_ranges(&reply, r2.start).unwrap();
    assert_eq!(entries.len(), 111);
    let last = reply[entries[111 - 1].clone()].to_vec();
    let mut arr = Vec::new();
    emit_array_head(&mut arr, 112);
    arr.extend_from_slice(&reply[entries[0].start..entries[110].end]);
    arr.extend_from_slice(&last);
    let with_continuation = replace_value(&reply, 0, 2, &arr);
    assert!(decode(&ids, "", "CatalogReply", &with_continuation).is_err());
    let without = remove_key(&with_continuation, 0, 3);
    assert!(decode(&ids, "", "CatalogReply", &without).is_err());
}

// acceptance: DEC-08
#[test]
fn dec_08_unknown_key_on_every_unsigned_message_is_rejected() {
    let ids = identities();
    let extra = ext_entry(99, &[0xc0, 0xff, 0xee]);
    let mut tried = 0usize;
    for f in accept_set() {
        let unsigned = f.kind == "frame" || f.kind == "reply" || f.kind == "e2e-payload" || unsigned_family(&f.kind).is_some();
        if !unsigned {
            continue;
        }
        let span = canonical_span(&f.kind, &f.bytes).to_vec();
        // the top-level map: a frame's is the body inside [type, body]
        let map_at = if f.kind == "frame" {
            let parts = array_item_ranges(&span, 0).unwrap();
            parts[1].start
        } else {
            0
        };
        if (Parser { b: &span }).head(map_at).unwrap().0 != 5 {
            continue; // the verifier-query body is an array
        }
        let modified = append_entries(&span, map_at, &extra, 1);
        let modified = if f.kind == "frame" { framed(&modified) } else { modified };
        assert!(decode(&ids, &f.id, &f.kind, &modified).is_err(), "{}: unknown key accepted", f.id);
        assert!(decode(&ids, &f.id, &f.kind, &f.bytes).is_ok(), "{}: original rejected", f.id);
        tried += 1;
    }
    assert!(tried >= 30, "{tried} unsigned vectors exercised");
}

// acceptance: DEC-09
#[test]
fn dec_09_unknown_key_in_the_envelope_map_is_rejected() {
    let env = fixture("P-adopt-min").bytes;
    let modified = append_entries(&env, 0, &ext_entry(5, &[0xc0, 0xff, 0xee]), 1);
    assert_eq!(modified[0], 0xa5);
    assert!(decode_envelope_structure(&modified).is_err());
    assert!(decode_envelope_structure(&env).is_ok());
    // the body and its signatures are untouched, so the rejection is the envelope map's alone
    assert_eq!(body_of(&modified), body_of(&env));
}

fn extensions_body() -> Vec<u8> {
    body_of(&fixture("P-adopt-extensions").bytes)
}

// acceptance: DEC-10
#[test]
fn dec_10_duplicated_unknown_key_is_rejected() {
    let ids = identities();
    let body = extensions_body();
    assert!(decode(&ids, "", "body", &body).is_ok());
    let r99 = value_slice(&body, 99).unwrap();
    let entry_start = r99.start - 2; // key 99 encodes as 18 63
    let original_entry = body[entry_start..r99.end].to_vec();
    let mut second = original_entry.clone();
    *second.last_mut().unwrap() = 0xef;
    for dup in [original_entry.clone(), second] {
        let mut v = body[..r99.end].to_vec();
        v.extend_from_slice(&dup);
        v.extend_from_slice(&body[r99.end..]);
        v[0] += 1; // a7 -> a8
        assert!(decode(&ids, "", "body", &v).is_err());
    }
}

// acceptance: DEC-11
#[test]
fn dec_11_non_deterministic_unknown_value_is_rejected() {
    let ids = identities();
    let body = extensions_body();
    for bad in [vec![0x9f, 0x05, 0xff], vec![0x18, 0x0a], vec![0x58, 0x03, 0xc0, 0xff, 0xee]] {
        let v = replace_value(&body, 0, 99, &bad);
        assert!(decode(&ids, "", "body", &v).is_err(), "{bad:02x?} accepted");
    }
}

// acceptance: DEC-12
#[test]
fn dec_12_extension_value_nested_1024_deep_is_accepted_and_round_trips() {
    let ids = identities();
    let body = extensions_body();
    let mut deep = vec![0x81u8; 1023];
    deep.push(0x80);
    let v = replace_value(&body, 0, 99, &deep);
    timed(|| decode(&ids, "", "body", &v)).expect("1024-deep extension value");
    let item = parse_all(&v).unwrap();
    assert_eq!(reencode(&item, &v), v);
}

// acceptance: DEC-13
#[test]
fn dec_13_sixteen_unknown_keys_in_each_of_two_maps_are_accepted() {
    let ids = identities();
    let body = remove_key(&extensions_body(), 0, 99);
    let r3 = value_slice(&body, 3).unwrap();
    let locator = remove_key(&body[r3.clone()], 0, 99);
    let mut entries = Vec::new();
    for k in 99..=114u64 {
        entries.extend_from_slice(&ext_entry(k, &[0xc0, 0xff, 0xee]));
    }
    let locator16 = append_entries(&locator, 0, &entries, 16);
    let body16 = append_entries(&replace_value(&body, 0, 3, &locator16), 0, &entries, 16);
    decode(&ids, "", "body", &body16).expect("16 per map");
    let item = parse_all(&body16).unwrap();
    assert_eq!(reencode(&item, &body16), body16);
    // and one more in either map is over the bound
    let seventeen = ext_entry(115, &[0xc0, 0xff, 0xee]);
    assert!(decode(&ids, "", "body", &append_entries(&body16, 0, &seventeen, 1)).is_err());
    let r3 = value_slice(&body16, 3).unwrap();
    let loc17 = append_entries(&body16[r3.clone()], 0, &seventeen, 1);
    assert!(decode(&ids, "", "body", &replace_value(&body16, 0, 3, &loc17)).is_err());
}

// acceptance: DEC-14
#[test]
fn dec_14_attach_with_empty_capabilities_is_accepted() {
    let ids = identities();
    let payload = fixture("P-frame-01").bytes[4..].to_vec();
    let parts = array_item_ranges(&payload, 0).unwrap();
    let modified = replace_value(&payload, parts[1].start, 3, &[0xa0]);
    decode(&ids, "P-frame-01", "frame", &framed(&modified)).expect("empty Capabilities");
}

// acceptance: DEC-15
#[test]
fn dec_15_attach_without_capabilities_is_rejected() {
    let ids = identities();
    let payload = fixture("P-frame-01").bytes[4..].to_vec();
    let parts = array_item_ranges(&payload, 0).unwrap();
    let modified = remove_key(&payload, parts[1].start, 3);
    assert!(decode(&ids, "P-frame-01", "frame", &framed(&modified)).is_err());
}

// acceptance: DEC-16
#[test]
fn dec_16_every_accepted_object_round_trips_byte_for_byte() {
    let mut n = 0usize;
    for f in accept_set() {
        let span = canonical_span(&f.kind, &f.bytes);
        let item = parse_all(span).unwrap_or_else(|e| panic!("{}: {e}", f.id));
        assert_eq!(reencode(&item, span), span, "{}: re-encoding differs", f.id);
        n += 1;
    }
    assert!(n >= 90, "{n} accepted objects round-tripped");
}
