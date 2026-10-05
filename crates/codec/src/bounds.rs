//! The global structural bounds of `wire-format.md` §1.3.  Exceeding one is
//! malformed, not merely unusual; none is derived from a capacity study.
//!
//! **Each is a ceiling on a decoder's exposure to a hostile peer**, set well
//! above any use anyone has articulated, so a value past one of them means
//! the input is malformed and not that the system is overloaded.  §1.3 says
//! what follows from that: the numbers carry no evidence that they are
//! correct, only that they are safe, and a legitimate use that ever
//! approaches one means the ceiling is wrong and should move.
//!
//! Where §1.3 gives a reason for a number, it is repeated below.  Where it
//! gives none, there is none to give: the row is a chosen ceiling.

/// Archive subset references (§1.3).
pub const ARCHIVE_SUBSET_REFS: usize = 256;
/// Bundles per `PrekeyReply`, one per device (`wire-format.md` §7.8): a chosen
/// ceiling beside design §23.3's count of about three.
pub const PREKEY_BUNDLES_PER_REPLY: usize = 8;
/// Verifier responses per recovery (§1.3).
pub const VERIFIER_RESPONSES_PER_RECOVERY: usize = 32;
/// Witnesses per presence record (§1.3).  With the two participants this
/// is the 18 logical signers the envelope bound derives from.
pub const WITNESSES_PER_RECORD: usize = 16;
/// Path length in nibbles (§1.3): depth 24 at a fanout of 10 exceeds any
/// plausible network.
pub const PATH_NIBBLES: u64 = 24;
/// A prekey bundle's opaque blob (§1.3).  A PQXDH bundle is an ML-KEM-768
/// encapsulation key of 1,184 bytes plus signed prekeys and signatures,
/// roughly 1.5 to 2 KB, so 4 KB is a denial-of-service ceiling with
/// headroom rather than a capacity figure.
pub const PREKEY_BUNDLE_BLOB: usize = 4096;
/// Back-pointers per signer in a merge (§1.3).  Longer than one means the
/// transaction reunites that many branch heads (§3.1).
pub const MERGE_BACK_POINTERS_PER_SIGNER: usize = 8;
/// Verifier responses per presence record (§1.3): two subjects times a
/// per-subject threshold capped at ten, with headroom.  **Sixteen does not
/// fit** — two well-connected participants require ten each.
pub const VERIFIER_RESPONSES_PER_RECORD: usize = 32;
/// Asserted locations per record (§1.3).
pub const ASSERTED_LOCATIONS_PER_RECORD: usize = 4;
/// Corroborations per record (§1.3): one per witness.
pub const CORROBORATIONS_PER_RECORD: usize = 16;
/// Proximity channels per record (§1.3).
pub const PROXIMITY_CHANNELS_PER_RECORD: usize = 8;
/// Keyhashes in an explicit scope list (§1.3).
pub const EXPLICIT_SCOPE_KEYHASHES: usize = 256;
/// `NetworkPoint` entries per anchor entry or endpoint record (§1.3, §7.2,
/// §7.6).  **This bound is those two structures' alone**: peering carries
/// exactly one `NetworkPoint` or `Locator` per endpoint (§4.4).
pub const NETWORK_POINTS_PER_RECORD: usize = 8;
/// A `CatalogEntry`'s total encoded bytes (§1.3).
pub const CATALOG_ENTRY_BYTES: usize = 2048;
/// `CatalogReply` entries (§1.3): an answering node answers for itself plus
/// the ≤110 users it serves (§6.4, design §11.5).  **Not the trust
/// horizon's population**, which is larger (design §15.1) and irrelevant
/// here, the bound being per answering node.  The frame bound caps it at
/// 127 regardless.
pub const CATALOG_REPLY_ENTRIES: usize = 111;
/// Unknown extension keys per map (§1.3).
pub const UNKNOWN_KEYS_PER_MAP: usize = 16;
/// An unknown extension value, in **bytes of encoded CBOR** (§1.3): the
/// complete encoded slice, which is measurable for every value type and is
/// what bounds a parser's work.  Not the aggregate of the byte or text
/// content inside it.
pub const UNKNOWN_VALUE_BYTES: usize = 1024;
/// `Capabilities` map entries (§1.3).
pub const CAPABILITIES_ENTRIES: usize = 64;
/// A `Capabilities` value, in bytes (§1.3).
pub const CAPABILITIES_VALUE_BYTES: usize = 1024;
/// `SiblingRef` entries in an `AttachAck` or a `SiblingUpdate` (§1.3):
/// f − 1, the update replacing the same logical list.
pub const SIBLING_REFS: usize = 9;
/// Peering audit history entries (§1.3).
pub const PEERING_AUDIT_HISTORY: usize = 8;
/// The local device-to-device interfaces (§14.3).
pub const INTENT_NOMINEES: usize = 64;
/// `ArchiveEntry` entries per `IntentExchange` or `BundleContinuation`
/// (§1.3, §14.3.2): a bound on the **carriage**; the bundle itself has none
/// (§5.4).
pub const INTENT_BUNDLE_ENTRIES: usize = 256;
/// `Candidate` entries per `CandidateHandover`, and per candidate exchange
/// on the payload path (§1.3, §14.3.2).
pub const CANDIDATES_PER_EXCHANGE: usize = 8;
/// Delegations per `DeviceCredential` (§1.3, §14.3.3): the run an instance
/// is provisioned with, 90 days end to end (design §12.6.5).
pub const DELEGATIONS_PER_CREDENTIAL: usize = 45;
/// The ceremony's conversation on the end-to-end channel (§7.10.2).
/// `ArchiveEntry` entries per `FishingProposal`: §5.4's carriage bound.
pub const FISHING_PROPOSAL_ENTRIES: usize = 256;
/// `Channel` entries per `WitnessRequest`: the proximity-channel ceiling.
pub const WITNESS_REQUEST_CHANNELS: usize = 8;
/// `VerifierResponse` entries per `GatheredResponses`: the per-record bound.
pub const GATHERED_RESPONSES: usize = 32;
/// `COSE_Signature` entries per `SigningReply`: one logical signer (§3.5).
pub const SIGNING_REPLY_ENTRIES: usize = 2;
/// A `DeviceCredential` delegation entry: a hybrid signature and its
/// fields, bounded with room (§14.3.3).
pub const DELEGATION_ENTRY_BYTES: usize = 4096;
/// A carried `PrekeyBundle`, signed or its unsigned payload: §1.3's 4 KB
/// blob bound plus the bundle's own fields (§14.3.3).
pub const DEVICE_BUNDLE_BYTES: usize = 5120;
/// Stream-0 control frames (§8.0): the length prefix's ceiling.
pub const CONTROL_FRAME_BYTES: usize = 65_536;
/// Bidirectional request frames (§9.2).
pub const REQUEST_FRAME_BYTES: usize = 262_144;

/// Envelope `COSE_Signature` entries for a transaction type: twice the
/// logical-signer ceiling, which is the sum of the type's per-role bounds
/// (§1.3, derived and never asserted independently).
///
/// **§1.3 requires the derivation and forbids the assertion.** An
/// independently stated envelope bound can contradict the per-role bounds,
/// so that a record one permits the other rejects; deriving it makes that
/// contradiction structurally impossible rather than something a reader has
/// to notice.  The doubling is §3.5's: each logical signer contributes one
/// classical entry and one post-quantum entry.
///
/// `None` for a transaction type this profile does not define.
pub fn envelope_entry_ceiling(tx_type: u64) -> Option<usize> {
    let signers = match tx_type {
        1 | 4 | 7 => 2,
        2 | 3 => 1,
        5 => 2 + WITNESSES_PER_RECORD,
        _ => return None,
    };
    Some(signers * 2)
}
