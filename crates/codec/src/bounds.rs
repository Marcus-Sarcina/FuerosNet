//! The global structural bounds of `wire-format.md` §1.3.  Exceeding one is
//! malformed, not merely unusual; none is derived from a capacity study.

pub const ARCHIVE_SUBSET_REFS: usize = 256;
pub const VERIFIER_RESPONSES_PER_RECOVERY: usize = 32;
pub const WITNESSES_PER_RECORD: usize = 16;
pub const PATH_NIBBLES: u64 = 24;
pub const PREKEY_BUNDLE_BLOB: usize = 4096;
pub const MERGE_BACK_POINTERS_PER_SIGNER: usize = 8;
pub const VERIFIER_RESPONSES_PER_RECORD: usize = 32;
pub const ASSERTED_LOCATIONS_PER_RECORD: usize = 4;
pub const CORROBORATIONS_PER_RECORD: usize = 16;
pub const PROXIMITY_CHANNELS_PER_RECORD: usize = 8;
pub const EXPLICIT_SCOPE_KEYHASHES: usize = 256;
pub const NETWORK_POINTS_PER_RECORD: usize = 8;
pub const CATALOG_ENTRY_BYTES: usize = 2048;
pub const CATALOG_REPLY_ENTRIES: usize = 111;
pub const UNKNOWN_KEYS_PER_MAP: usize = 16;
pub const UNKNOWN_VALUE_BYTES: usize = 1024;
pub const CAPABILITIES_ENTRIES: usize = 64;
pub const CAPABILITIES_VALUE_BYTES: usize = 1024;
pub const SIBLING_REFS: usize = 9;
pub const PEERING_AUDIT_HISTORY: usize = 8;
/// Stream-0 control frames (§8.0): the length prefix's ceiling.
pub const CONTROL_FRAME_BYTES: usize = 65_536;
/// Bidirectional request frames (§9.2).
pub const REQUEST_FRAME_BYTES: usize = 262_144;

/// Envelope `COSE_Signature` entries for a transaction type: twice the
/// logical-signer ceiling, which is the sum of the type's per-role bounds
/// (§1.3, derived and never asserted independently).
pub fn envelope_entry_ceiling(tx_type: u64) -> Option<usize> {
    let signers = match tx_type {
        1 | 4 | 7 => 2,
        2 | 3 => 1,
        5 => 2 + WITNESSES_PER_RECORD,
        _ => return None,
    };
    Some(signers * 2)
}
