# Notes: RES (resolution) and REP (replication-peering) draft entries

Draft written 2026-09-09 against the working tree at cb48989 (plus uncommitted
changes outside these documents). Entries: 16 RES, 15 REP; 11 carry an
interpretation. Checked with a replica of `rhtn/acceptance/tools/check.py` (same
`catalogue.py` helpers, whole-catalogue gap and marker checks skipped): 0 flags.

## 1. Assigned functions with no specification basis found

1. **"Prefix" visibility in the peering transaction.** `wire-format.md` §4.4's
   `NetworkPoint` carries a 4-byte IPv4 address, an optional ASN and an optional
   port; there is no prefix field. Counting by routable prefix is deferred with
   IPv6 (design §4 "Explicitly deferred", §17.3). No entry covers prefix.
2. **The carrier for sibling replication.** Design §3.4 ("siblings replicate each
   other's traffic") and §14.1.2 ("this state replicates to siblings") name no
   frame; `wire-format.md` has none (its only sibling frame, `SiblingUpdate`, carries
   the sibling *list*). REP-01 observes the sibling's resulting state only and says
   so in its interpretation.
3. **Peer backup.** Design §6.3 has a peer "persisting the peer's data — as an
   encrypted backup" and `wire-format.md` §4.4 field 6 is a "replication commitment,
   bytes", but no document says what is replicated to a peer, when, or by what
   message; design §16.3 calls it "off-protocol". No entry.
4. **"Replication lag observable rather than resolved."** No document uses the
   phrase. The nearest basis is design §12.6.5.1 ("a sibling issues a fresh
   attestation reflecting whatever rotation reached the replicated state") together
   with `wire-format.md` §7.1 field 5 (issuer role 1, secondhand). REP-03 encodes
   that reading and carries an interpretation.
5. **The anchor table's "budget".** Design §12.2's 25 MB is a sizing estimate and
   §12.7.3 says "there is no budget to blow, because no node is obliged to cache
   anything". No bound is tested; RES-02 tests the per-node threshold instead,
   parameterised because design §21.1 lists it as unset.
6. **Replication depth beyond the floor** (design §3.4, unset in §21.1). No entry
   depends on it, so nothing was parameterised; recorded so the omission is visible.

## 2. Inconsistencies noticed (not fixed)

1. **Queued messages and siblings.** Design §3.4 floor: "every user's messages
   replicate on their nearest infra node and that node's siblings." Design §14.1.6:
   "Siblings do not hold queue state." `infra-client-requirements.md` §2: "Do not
   replicate queue state to siblings." If §3.4's "messages" means queued payload
   these conflict; if it means control metadata the wording misleads. REP-02
   follows §14.1.6 and the infra document.
2. **A locator does not carry endpoints.** `infra-client-requirements.md` §4.3:
   "Replace an endpoint set when you receive a locator with a strictly greater
   seqno". `wire-format.md` §2.3's `Locator` is `{anchor, path, seqno}` and design
   §12.6.1 says "SignedLocator is not the carrier"; the carrier is the
   `EndpointRecord` (§7.6, infra §4.4). RES-15 follows §7.6.
3. **Forwarding chains.** `infra-client-requirements.md` §4.3: "Collapse forwarding
   chains at the source: follow the chain and return the terminal record, not the
   next hop (design §12.3)." But infra §4.2 says "do not redirect on its behalf",
   `wire-format.md` §7.7.3 says "No party redirects a resolution to a different
   position on the subject's behalf ... removing forwarding removed the thing that
   needed the guard", and design §12.3 Case 2 says "Nothing redirects on Alice's
   behalf." The §4.3 bullet reads as a survivor of the withdrawn mechanism.
4. **Failure-code field number.** `wire-format.md` §7.7.3 says "Failure codes for
   field 5" while the `ResolveReply` schema puts the code in field 4 (field 5 is
   the `Referral`).
5. **Serving node versus patron in §14.1.2 item 3.** "the patron marks the client
   unreachable and begins queuing", while item 1 of the same section says the
   serving node "is not necessarily its patron". REP-01 reads it as the serving node.
6. Cosmetic: design §12.3's step numbering runs 6, 7, then 9 (no 8); design §12.7's
   opening sentence has an unclosed parenthesis; `wire-format.md` §7.7.3 spells the
   advances bound ">= 1" in the schema comment and "≥ 1" in prose.

## 3. Questions only the author can answer

1. REP-07 (metric ignores ASN concentration) needs the reference metric, which is
   milestone 5. Keep it in REP at milestone 4, move it to milestone 5, or move it to
   the MET area?
2. REP-01: is a wire carrier for the replicated unreachable marking intended, or is
   sibling replication of session state a deployment matter outside the protocol?
3. Is design §3.4's replication floor about queued messages or about control
   metadata? (Inconsistency 1 turns on this.)
4. RES-14: after a departed child is removed, is failure code 0 ("no such child at
   some index") or 1 ("not authoritative and cannot refer") intended?
5. REP-14: does a direct-path override require both users' choice, or does one
   side's suffice? The test sets both.
6. RES-06 and the anchor table: what does a holder do with an unranked
   different-series locator or anchor entry beyond not installing it — hold it
   beside the current one, or drop it? (`wire-format.md` §7.6 answers this for
   endpoint records, "holding one per proved series is the correct end state"; §2.3
   and §7.2 do not for locators and anchor entries.)
7. Should anchor-entry freshness (`wire-format.md` §7.2, the same §2.3 rule) get an
   entry of its own, or is coverage on the locator path (RES-04 to RES-06) enough?

## 4. Overlaps with other areas, for the merge

- REP-02 (no queue replication) touches QUE; REP-03 (secondhand issuance) touches
  CUR; REP-04 (move without archive presentation) touches TOP and ARC; REP-08 (no
  rootward memo for peering) touches PRP; REP-15 (sibling list composition) touches
  SES; REP-07 touches MET. Each was kept because its rule sentence is in the REP
  row's cited sections.
- RES-13's rule quote is taken from a schema comment in `wire-format.md` §7.7.3 and
  therefore carries the comment's line-leading semicolons ("... the list as ;
  alternatives ..."); that is what the section text contains after the checker's
  whitespace normalisation, and no prose sentence states the rule.

## 5. Candidates not written (RES is at the 16-entry ceiling)

Nonce reuse across endpoint retries (§7.7.3); the code-2 retry and code-0
no-retry dispositions (§7.7.3 table); replaying an unchanged endpoint record
without renumbering (§7.6, infra §4.4); one endpoint record per patron
relationship (§7.6); starting a later resolution from a cached intermediate with
the TTL parameterised (design §12.6.1, §21.1); "disclose nothing beyond the query
itself" to an unauthenticated anchor (§7.2, needs an interpretation of "nothing
beyond"); anchor-entry freshness (§7.2); an endpoint record generating no rootward
memo (§7.6, §10.2).
