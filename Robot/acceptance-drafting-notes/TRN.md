# TRN notes — transport profile as executed (gap "transport", milestone 2)

16 entries, TRN-01..TRN-16. Two carry a non-null interpretation (TRN-15, TRN-16).

## Coverage of the assigned functions

| Function | Entries |
|---|---|
| Only the named group offered; nothing weaker negotiable | TRN-01 (client offers only X25519MLKEM768), TRN-02 (server completes no handshake with weaker-only offer) |
| RFC 7250 raw-public-key mutual authentication: dialling party's pinned-classical-member check | TRN-03 |
| Server's binding of Attach field 1 to the transport-authenticated identity | TRN-04 |
| ALPN | TRN-05 |
| Refusal as a QUIC close code, no AttachNack | TRN-13 (node side), TRN-14 (client side) |
| Capability parameters: tolerance mandatory, greasing not | TRN-10 (unknown id tolerated), TRN-11 (nothing advertised, nothing greased), TRN-12 (reference greases) |
| Control-frame framing on stream 0 | TRN-06 (framing bytes), TRN-07 (unknown type skipped, at the bound, before the ack), TRN-08 (over-bound length ends session), TRN-09 (malformed known type discarded whole) |
| Connection migration | TRN-15 |
| 0-RTT early data deferred until that connection's handshake completes | TRN-16 |

Oracles: `model` for TRN-03, TRN-04 (executable counterparts of `tamarin/wire-only/attach`'s
`server_authentication`, `client_authentication`, `queued_data_reaches_only_the_authenticated_peer`)
and TRN-16 (`tamarin/compliant/attach`'s `a_conforming_server_binds_only_after_the_handshake`);
`fixture` where `test-vectors/messages.md` supplies the frame bytes or a session trace
(TR1, TR2, TR4, TR6, TR9, the Attach/AttachAck frames, the greased-capability must-accept);
`behaviour` otherwise.

## Functions with no specification basis found, or with a basis too thin to test

- **A positive test that the implementation itself presents the classical member of its own
  KeyMaterial as its raw public key** (both roles) is not a separate entry; it is exercised only
  implicitly, by the counterpart's check in TRN-03 and by the successful handshakes in TRN-04's and
  TRN-13's givens. Worth its own entry if the budget grows.
- **Refusing an X.509 certificate presented by a peer.** wire-format.md §9.1 says raw public keys
  "not X.509"; whether an implementation must *reject* a peer that presents X.509 (as opposed to
  never presenting one) is not stated. No entry.
- **ALPN mismatch handling.** The profile names the ALPN and nothing more; closing a connection
  on which no application protocol is negotiated is RFC 9001's rule, not the profile's. No
  negative entry for ALPN; TRN-05 is positive only.
- **The consequence of "protocol error" for a repeated Attach or AttachAck after the ack**
  (wire-format.md §8.2). §8.0's over-length rule says "protocol error, and it ends the session";
  the post-ack rule says only "protocol error", so the observable outcome cannot be quoted.
  Corpus trace TR5 says `fail_attach`. Left to the session area.
- **Endpoint iteration on any other close code** ("the client tries the remaining candidates in
  the order received", §8.2; light-client-requirements.md §4's retry floor) was drafted and cut
  for the 16-entry cap. It belongs with either this area or session lifecycle; flagged so it is
  not lost.
- **NAT traversal, STUN and TURN** (design §14.1.1, infra-client-requirements.md §7) are the
  payload path, not the control transport, and are not covered here; they belong with the payload
  row ("after 5").
- **Capability-id derivation** (`first 8 bytes of SHA-256("rhtn/cap:" || name)`, big-endian) is
  already exercised by the corpus (`test-vectors/tools/generate.py` computes `CAP_BATCH` that way and
  the Attach fixture carries a named parameter), so no entry restates it.

## Inconsistencies noticed between documents (not fixed)

1. **wire-format.md §8.1.1's heading reads "Greasing — required, not decorative", while its body
   says "Sending is not a MUST, because nothing could check it"** and design §14.1.1 says sending
   greased parameters is "self-interested rather than obligatory". Tolerance is the MUST;
   the heading overstates sending.
2. **§8.1.1 says "one greased parameter per session" in one sentence and "at least one
   randomly-chosen 64-bit id" two sentences later.** TRN-12 asserts "an entry", satisfying both;
   a test asserting exactly one would follow the first sentence only.
3. **wire-format.md §9.2's stream-0 line lists "session control (attach, heartbeat, sibling
   updates)"**, while §8.0's frame table also places `TopologyPush` (5) and `TopologyMemo` (6) on
   stream 0. The parenthetical is incomplete rather than wrong.
4. **`test-vectors/messages.md` traces TR9 and TR10 cite §9.2 for the refusal close-code rule; the
   rule is in wire-format.md §8.2.** A corpus-file citation, not a root-document one.
5. **Robot/implementation-plan.md §8.1 lists "session-message encodings" as established by
   `corpus.json`**, and `messages.md` carries session traces TR1–TR24 with required actions; the
   corpus README says the harness executes only the encoding layer, so the traces are stated
   expectations, not executed checks. That is consistent with the plan's §8.2 owing the transport
   row; noted so nobody reads the traces as already-run tests.

## Questions only the author can answer

1. **What does rejecting an Attach whose field 1 mismatches the authenticated identity look like on
   the wire?** §9.1 says "rejecting any mismatch"; §8.2 assigns close code 1 to refusal *by
   policy*. Is a mismatch closed with code 1, with some other code, or is the frame dropped with
   the connection left open? TRN-04 checks only that no AttachAck and no queued delivery follow.
2. **Which close code, if any, ends the session on an over-bound control-frame length** (§8.0)?
   TRN-08 does not check the code.
3. **For an Attach in 0-RTT early data, what does "reject it" look like to the client** — a close,
   or silence that the client must follow with a fresh Attach after the handshake? Nothing in
   light-client-requirements.md §4 tells a client what to do when its early-data Attach is not
   answered. TRN-16 accepts either reject or defer.
4. **Is "until handshake completion" (§8.2) the handshake of the connection that carried the early
   data?** The compliant Tamarin theory reads it so and falsifies the alternative; the wire text
   does not say it. TRN-16 carries this as its interpretation. One sentence in §8.2 would remove it.
5. **Does a serving node commit to permitting client-initiated QUIC connection migration** (that
   is, not disabling active migration)? §9.2 and design §14.1.3 say migration is relied upon;
   infra-client-requirements.md §7 places no such obligation on the operator. TRN-15 carries this
   as its interpretation.
6. **Must a dialling party offer exactly `rhtn/1` in ALPN, or merely include it?** TRN-05 asserts
   only that it is carried and negotiated.
7. **Is an implementation obliged to refuse a peer that presents an X.509 certificate** rather than
   a raw public key, or only never to present one itself? See the first list.
