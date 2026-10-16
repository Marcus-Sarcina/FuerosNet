# The field-test sequence: plan of record

Scoped with the author, 2026-10-10, after the first cold ceremony on
low-spec hardware (`82ffd1b`). The author's list, with the assistant's
additions and reorderings taken; two of the assistant's first readings were
wrong and are corrected below. **Robot/ is the assistant's workspace**: this
file is operational and is pruned or rewritten as runs land.

**Legend.** *proven* — passed in-process (a named test); *hardware* — passed
on phones or containers; *build* — code that does not exist yet and must
before the test can.

**The bench.** Four phones: two SM-G996U1 (S21: Android 15, API 35, NFC and
UWB) and two TCL 30 Z (4188C: Android 12, API 31, 2.7 GB, BLE only), all in
`crates/tools/phones.conf`. The cold bench is `nodeless.sh --phone A
--phone B <label>`; the node bench is `field-run.sh`, which provisions a
daemon, two instrument witnesses, and names a nominee for phones at
genesis. **The cold bench admits phones at genesis only** (design §6.4's
witnessless shape): a phone that has met anyone is refused `NoWitness`, so
a nodeless run wipes its two phones first (`field-setup.sh --conf` on a
file naming only those two keeps the others' identities).

## Two axes, not one sequence

The ceremony's **variants** (plain, adoption either way, peering,
witnessed, verified) and its **setting** (nodeless, or with a node on the
bench) are independent: adoption and peering are participant acts that
work with or without an infra node — the design has light-client patrons
as ordinary ("a leaf node's patron may be a light client"; resolution walks
"up past any light-client patrons"), a root is self-anchored, and design §6 prices
a peering edge like a proof-of-presence edge because it "joins parties who
by construction share no position". The assistant first ordered a node
before adoption and read peering as node-to-node; both wrong.

## A. Ceremony variants — nodeless first, then with a node

| | Proves | Needs | Stands on |
|---|---|---|---|
| **A1 Mixed S21/TCL, plain** — **done, `82ffd1b-a1-mixed-1`**; range test still not run | the asymmetric case: a good camera on a dim screen and the reverse; the strongest-channel rule across unequal hardware (the TCL pair passed on Optical alone); **the range test**, still an S21 figure; first mixed-throughput data for the presentation-rate idea | nothing new | hardware: both symmetric pairs |
| **A2 Adoption, both directions** — **the scope-carrying pass done, `3cd8165-a2-carry-1-1` (2026-10-15)**: two meetings on one run. First GXV adopted ZU8 (record `89745eb3`, adoption `29b0c905`), so GXV rooted a scope; then GXV initiated as the client, was asked *submit as which scope?* and chose to bring it, 4VC scanned and was asked *adopt into which scope?* and chose the subnet it had adopted MRK into (`a9b13bcd…`); record `4a9d181b` with 4 signers (two witnesses this time), GXV offered series 2 "bringing scope 29b0c905…", 4VC proposed with field 10 (13 ms), GXV's kernel checked it as a scope it roots and signed (21 ms), both hold `b1232720`; the daemon stored it at 7240 bytes against 7170 for an adoption carrying nothing — the locator's subnet and field 10. **Reverse direction done, `3cd8165-a2-reverse-1-1` (2026-10-15)**: MRK initiated as the client, 4VC scanned and, holding no subnet, was answered *adopt into which scope?* unasked (a new subnet under its own key); record `c5692877` (3 signers, 90 s), the adoption `a9b13bcd` proposed by 4VC (25 ms), signed by MRK's kernel (14 ms) and held by both; MRK offered series 1 *as an individual*, rooting nothing. One defect, fixed: the direction frame's label was composed in the patron's frame ("under your own key"), now in the counterparty's ("a new subnet under c9b44cb4"). Still to run: a pass where the subordinate brings a scope. **Rerun after the rework, `1807b03-1807b03-a2-adoption-1-1` (2026-10-15)**: GXV patron as a lone root, ZU8 adopted; record `2a6bde34` with 3 signers (one laptop witness, the other side's nominee was the same one), the adoption `2b7a56fd` held by both and stored by the daemon (the presence record `OutOfStore` there, as in the earlier run); the direction frame carried the subnet's label ("into … a new subnet", worded better since), the body named no subnet as a lone root's first adoption must, and ZU8's kernel signed it (`cer.adoption.signed`, series 1). Both S21s logged two `sendmsg … I/O error` warnings at attach with tethering off, and attached in 1.3 s regardless. **First pass with a node, `722519a-a2-adoption-7`**: GXV patron, ZU8 adopted, record `d3ca1939` with 4 signers and both verifiers answering Match, then the adoption `de2b5961` held by both — the node offered series 1, the patron proposed, the node's kernel checked the body and signed it (`cer.adoption.signed`). **One defect, fixed**: the responder's brief said nothing about the patronage, reading this device's own D1a choice instead of the settled direction. Still to run: the reverse direction, the simultaneous-opposite case, and the nodeless pass, which needs two phones at genesis. Whether the legs carry registered kinds is still the author's to rule, and the answer does not change what the bench runs | `THEM_UNDER_ME` and `ME_UNDER_THEM` chosen at D1; a TCL under an S21 that is its own root; then D7's simultaneous-opposite case; then with the patron attached to a node | nothing new nodeless; a node for the second pass | proven: `adoption_under_slot` |
| **A3 Peering** — **the kernel act is built (2026-10-15), the shell is not**: `Client::propose_peering` / `review_and_sign_peering` / `take_peering` and the three over the ffi, a light client peering **by locator** as `wire-format.md` §4.4 field 3 admits, on a meeting of the two; `rhtn-archive` is the one emitter of the body now and the node's own wrapper calls it. Owed: a shell screen, which the Meet flow's `DONE` step is the place for, as D7's adoption is | a peering edge between two phones, nodeless; then with a node | a kernel act, then the shell | proven: peering priced and propagated in-process |
| **A4 Witnessed** — **done, `82ffd1b-a4-node-3`**, both pairs (S21s `41e88dba`, TCLs `1a2556fb`), 4 signers each, both bench witnesses attesting; the TCLs' first attempt found the bearer's server side ungated (fixed: `notifyDone`) | the far-node path, with-node only: the nominee must be reachable, and a nominee met nodeless has no route (`reach_for`); nomination from met parties is built (2026-10-11: `Client::met`, a witness both name signs once, the shell stops waiting where no witness can come) | a node on the bench; nomination from met parties | proven: the witness path in-process; `field-run.sh` names nominees |

## B. Verification

Every test here needs a **prior ceremony**: the verifier holds a sealed
capture of the subject from an earlier meeting. Four phones make this
possible; it is also why phones are **not wiped between runs** once B
begins, except the two a nodeless run requires at genesis.

With A having met B earlier, B meets C and A is selected:

- the same-face test the author ruled on 2026-10-06: A asked about B → **match**; A asked about a party it never met → **unavailable**, its own held evidence refuting the basis (VER-008)
- a mismatched face → **no-match**; an unusable or unreleased capture → **inconclusive** or **unavailable** with no basis (VER-012); an older modality against its legacy engine (VER-013)
- a wrong query hash or missing consent refused without a fabricated response (VER-004); the grant delivered straight to the verifier, bypassing querier and witnesses (VER-010); a grant before its query buffered briefly (VER-011)
- the response reaching both querier and subject (VER-015); a late reply kept private or as the optional late object (VER-016)
- **with a node** — a party met nodeless has no route afterwards (`reach_for` routes by position through infra; a ceremony's direct-path candidates are not kept), and between pairs sharing no party the pool is empty by construction (`wire-format.md` §5.3's "verifies alone"; `unverifiable` is never resolved). B1 on the cold bench showed both; **B1 with-node (`82ffd1b-a4-node-3`, `4c9d99b7`) confirmed the pool: `n:1 pool:0` on both sides, the handed record unverifiable for the co-signer never held**. **The ruling that gated this was already made** [author, 2026-10-11, confirmed 2026-10-15]: `wire-format.md` §5.4's bundle is introductions, each carrying the candidate's key material and the most recent locator the subject holds, so the key to verify a candidate's answer arrives with the candidate and `selection::pool` reads it. The `pool: 0` on `82ffd1b-a4-node-3` was who had met whom — each bundle named only the counterparty — and not a missing decision. **What B needs is a pairing**: two parties whose bundles name a third each has met, which the bench now has after three adoption runs

## C. Infrastructure

**C1 Launch a node.** The proven half: phone-to-container enrolment
(hardware, 2026-10-10). The **build** half: H1 — creating the instance from
the client; on the bench, the client launching a container on this machine
(`local.sh` is the stand-in); the cloud flow later and larger. Also owed:
**renewal** — a run is 7 × 48 h and nothing renews it.

**C2 Node-to-node peering** — two containers; proven in-process
(`a_new_root_peers_floods_and_is_reached_over_real_sessions`).

**C3 The integrated chain**, as runs:
- patron attached to the node — proven (`attach_to`); hardware is the `field-run.sh` shape
- patron accesses a resource; a sub accesses one under a grant — the node side proven (hosted package served, grants by predicate, a resource on its own port); **build**: the client's Catalog screen is a placeholder, so the phone side is a build before it is a test
- the node's address held as anchor by an unconnected user; that user reaches the sub through the patron's node — anchor entry and payload relay proven; on hardware, the "payload demo" peer with a third phone that knows only the anchor
- the node caches for an absent sub — `material_queues_for_a_client_marked_unreachable` and redelivery across a restart, proven; on hardware, airplane mode on the sub; the **failover sibling** case beside it, which has its own proven test

**C4 Two infra tiers, a light tier, traversal** — three containers and four
phones; `a_resolution_is_proxied_through_a_real_referral` is the proven
core; the `sim` crate already runs several daemons.

## D. Beyond

Recovery, rotation and departure on hardware — all with in-process tests,
none run on phones. The iOS client, which has no position until it exists.

## Order

A1 is done; a second plain pair was met deliberately (`82ffd1b-a2-adoption-1`, mislabelled) for series B. B1 nodeless hung at witness selection and showed that nothing met nodeless is reachable afterwards and no cross-pair verifier can qualify — so **B and A4 are with-node**, and the node bench (C1's proven half plus `field-run.sh`) is now the gate for nearly everything left. **[ruled, author, 2026-10-10]** witnesses are required where available and a witnessless meeting is not genesis-only; proceeding witnessless past genesis needs design §13.2 / `wire-format.md` §3.2 amended, which gates any nodeless meeting between non-genesis phones. **The node bench is up (2026-10-11, `82ffd1b-a4-node-1`), all four phones re-provisioned at genesis on it; the two met pairs are to be re-met there for B.** A4, B's with-node
half and all of C wait on a node being routine on the bench, which is C1's
proven half plus `field-run.sh`. B's nodeless half needs only a prior run's
state kept. Within C, the Catalog screen is the one build that gates a test
in the middle of a chain.
