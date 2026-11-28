//! **A `NetworkPoint`'s address, read and replaced** (`wire-format.md`
//! §4.4) for re-signing a moved node's records: the address changes, and
//! what the point was signed with besides (the ASN, the zone, the port)
//! does not. Below the boundary, so a shell keeps no parser of its own.

use rhtn_ffi::client::{network_point_at, network_point_ip};

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

// functional: OPS-030
#[test]
fn the_address_is_read_and_replaced_and_nothing_else_moves() {
    // {1: h'9df5f2b6', 2: 14061, 4: "nyc1"}: 157.245.242.182, AS14061, nyc1
    let held = unhex("a301449df5f2b6021936ed04646e796331");
    assert_eq!(
        network_point_ip(held.clone()).as_deref(),
        Some("157.245.242.182")
    );
    let moved = network_point_at(held.clone(), "165.22.9.217".into()).unwrap();
    assert_eq!(moved, unhex("a30144a51609d9021936ed04646e796331"));
    assert_eq!(network_point_ip(moved).as_deref(), Some("165.22.9.217"));
    assert!(network_point_at(held, "300.1.1.1".into()).is_err());
    assert_eq!(network_point_ip(unhex("a10264")), None);
}
