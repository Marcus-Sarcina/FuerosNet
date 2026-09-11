//! The evidence an adoption carries, at the record level: what
//! `Record::parse` and `check_signatures` say about a transfer or recovery
//! whose embedded blocks are malformed or name the wrong parties.  The
//! byte-level cases are `rhtn-crypto`'s (DEC-19); these are the same
//! objects as a table would receive them.

mod common;

use common::World;
use rhtn_archive::record::SigStatus;
use rhtn_archive::tx::*;

// acceptance: DEC-19
#[test]
fn a_transfer_whose_former_patron_block_is_empty_is_not_verified() {
    let mut w = World::new(&["alice", "bob", "carol"]);
    // carol vouches for alice's move to bob with no signature at all: the
    // node and patron sign the body, and the block is `[h'', {}, nil, []]`
    let empty = vec![0x84, 0x40, 0xa0, 0xf6, 0x80];
    let rec = w.adopt_with("alice", "bob", Evidence::Transfer { former: w.kh("carol"), block: empty }, 1, None);
    match rec.check_signatures(&w.lookup()) {
        SigStatus::Invalid(e) => assert!(e.contains("transfer statement"), "{e}"),
        other => panic!("an empty block verified: {other:?}"),
    }
    // the genuine statement is the control
    let block = transfer_block(w.id("carol"), &w.kh("alice"), &w.kh("bob"));
    let rec = w.adopt_with("alice", "bob", Evidence::Transfer { former: w.kh("carol"), block }, 2, None);
    assert_eq!(rec.check_signatures(&w.lookup()), SigStatus::Verified);
}
