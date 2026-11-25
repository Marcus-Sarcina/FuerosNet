# Burn-down: owed decisions and unbuilt work

Drawn 2026-11-24 from the audit of 2026-11-19 to 2026-11-24 (53 commits, the
author's messages, five read-only reviews). An item leaves this file when it
is decided, built or explicitly deferred; a deferral moves to
`deferred-decisions.md`. Numbers are not reused.

**D** items are the author's and are asked in every output until answered.
**B** items are work; one marked "after D-n" waits on that decision.

---

## Decisions owed

### Blocking the applications project's use of the node and clients

| # | Decision | Options | Meanwhile |
|---|---|---|---|
| D-01 | Utility asks on the desktop | **Decided 2026-11-24**: `export` a file download, `import` a file upload; `notify` a desktop notification where there is one; `scan` a connected scanner, an uploaded image as the fallback; `capture` a connected camera; `live` a connected webcam; `sign` on a desktop holding the seed; `location` and whatever a desktop cannot do are offered to the signing device. `check-in` and `tag` go to the signing device. The picture he gave: the desktop is where heavy authoring, administration and assessments happen, the phone a peripheral for certain tasks as it is for a 2FA challenge, and a laptop's own webcam the preferred route for a liveness check. **`sign` stays under the per-device application key** (the 2026-10-31 rule stands) and, as policy, a restored desktop signs and a paired one declines [author, 2026-11-24]. **`capture` is the webcam or declined, never an upload** [author, 2026-11-24], the claim being that this device took it. **No ask is forwarded and there is no new frame** [author, 2026-11-24]: the desktop declines what it cannot do, as it does now, and the resource orchestrates with the two things it already has, a notice to the member (resource-requirements section 3.3) and a page on the desktop saying to do the step on the phone | all declined on the desktop until built |
| D-02 | Request headers on `rhtn.request` | **Decided 2026-11-24: a page sets none.** Stated in resource-requirements section 3.4, light-client section 8 and CAT-027; Academy's `idempotency-key` header is the applications repository's to move | — |
| D-03 | Plain-HTTP limits: no redirects, cookies, websockets or streaming | **Decided 2026-11-24: stated as the contract, large and continuous content going as parts** (resource-requirements section 3), **and live and streaming built now**. **A live session goes to a media port the package holds on the node's own host** [author, 2026-11-24], signalled through the ordinary path; the reason is his flat-fee rule (one machine, a bigger VM before a second account) | — |
| D-04 | A package answer's size | **Decided 2026-11-24: the node is raised to 16 MiB**, the whole message held to the wire's bound (PKG-022) | — |
| D-05 | Whether a node on the developer's own machine is a supported path | **Decided 2026-11-24: supported.** `local.sh` gains what the install act needs and goes into the gate (B-06, B-07) | — |

### Protocol and node

| # | Decision | Options | Meanwhile |
|---|---|---|---|
| D-06 | Queue cap on a launched node | (a) ship 256 MB, which the sizing screen assumes (recommended); (b) leave uncapped | uncapped |
| D-07 | Whom a node places at attach | (a) as built: only an identity its operator adopted; (b) also its own operator, and a member it serves from two levels up | (a); a restored root operator learns nothing |
| D-08 | What rides inside the seal for a sender | (a) the introduction, as the wire format and code have it; (b) the full card, as the design says | (a) |
| D-09 | The grace for a just-replaced node key | 30 seconds, or another figure | 30 s, tagged as the author's without his having said it |
| D-10 | Which alias rides a message when the sender holds several subnet profiles | (a) first in subnet-id order, as built; (b) the one for the subnet written through | (a) |
| D-11 | A stranger whose message carries its introduction becomes a contact and can be answered | (a) intended; (b) a reply needs a contact made some other way | (a), so "reply only to contacts" bites only on mail with no introduction |
| D-12 | Notification while the person is away | **Withdrawn 2026-11-24, already settled**: a phone keeps its session in a pocket [author, 2026-11-14], a resource's count rides that session and the doorbell stays the opt-in of a client refused that standing [author, 2026-11-17, 2026-11-23], and the push discussion is deferred until he reopens it [2026-11-17]. The board post's part is D-21's | — |
| D-13 | Wire format section 10.3.1, the post's contents | (a) keep it as the wire format's; (b) move it out as the reference client's own, per "does not require a wire definition" | (a) |
| D-14 | Desktop delegation renewal at the end of its 90-day run | (a) build it, with a wire carriage; (b) defer, re-pairing being the way | nothing renews |
| D-15 | Light-client section 8's display duties (a resource's owner, the roles held, what was stale or truncated) | (a) withdraw them, launchers showing none; (b) show them somewhere | documented, not shown |

### Confirmations of choices made for him

| # | Choice standing | Where |
|---|---|---|
| D-16 | "Unassigned" means the node publishes no endpoint record | infra section 4.4, under his tag |
| D-17 | A sealed package: the choice is fixed for the storage's life; a host that cannot seal refuses the install; a move carries it open; it never uses a mounted block | infra section 9.4, untagged |
| D-18 | A post carries its writer's full key material (about 2 KB), capping the words at 8,192 bytes | wire format section 10.3.1 |
| D-19 | The numbered echo to the writer is the only acknowledgement; one frame, no reply | wire format section 10.3 |
| D-20 | Frame number 10 and SHA-256 de-duplication carry his tag though he ruled neither | wire format section 10.3 |
| D-21 | The app shows a post's age and not its date; a post raises no notification; a pool is stored as one blob | the reference client |
| D-22 | The board cap follows the 11-23 wording (node-running downline), superseding the 11-18 note that direct subordinates allow +3 | design section 14.4 |
| D-23 | A patron whose own adoption was a recovery or a transfer carries no node to the member | light-client section 1 |
| D-24 | A card's count includes a resource's step twice: its own count and the unanswered ask | light-client section 8 |
| D-25 | The desktop stops answering a resource after 32 asks in a row | `ASKS_RUNNING` |
| D-26 | The adoption's eleventh field, with a null tenth | wire format section 14.3.2 |

### Wording and housekeeping

| # | Decision | Options |
|---|---|---|
| D-27 | "Resource" on consent, leaving and notification screens; the phone's "Serving node" row; the desktop's "Connect to a node" form | remove each, or outside the ruling |
| D-28 | `change-log.md`, silent since 2026-11-06 | backfill from the commits, or retire |

---

## Unbuilt work

### Hosting, for the applications project

| # | Work | Waits on |
|---|---|---|
| B-01 | Write the developer procedure (build, push by digest, mint, install, grant, open, logs) and the container contract (`--listen`, `--resource`, `--secret`, user, read-only root, `/rhtn-block`), with the block and log verbs | — |
| B-02 | **Done 2026-11-24** (53dc2563, PKG-021): a request is decided under the lock and its package asked holding nothing; a request the node answers itself waited 3.38 s behind a 2 s package before, under 1 s after | — |
| B-03 | Utility asks on the desktop | D-01 |
| B-04 | Closed with D-02: nothing to build | — |
| B-05 | **Done 2026-11-24**: the HTTP limits stated (resource-requirements section 3) and the answer bound raised (PKG-022) | — |
| B-06 | `local.sh`: `resources`, `state`, host shell | D-05 |
| B-07 | Run `smoke.sh` in the gate or CI, so a renamed config key fails a build and not a developer | — |
| B-08 | Command-line verbs to narrow or drop a grant and to read an instance's log | — |
| B-09 | An act that sets an installed package's federation relationships | — |
| B-10 | A client that composes a resource's registration, so a launcher has a name other than its authority | — |
| B-11 | Phone: form posts, a file picker, binary bodies; check a 3xx answer does not crash the frame | — |
| B-12 | `live` on the phone; video capture, PDF scan and `uwb` are declined there | — |
| B-13 | Install and offering screens on the desktop (phone only today) | — |
| B-14 | The install act's seal warning says less than light-client section 10 requires | — |
| B-15 | `crates/packaging/README.md`: the "carries no control yet" passage predates the acts | with B-01 |

### Protocol and node

| # | Work | Waits on |
|---|---|---|
| B-16 | Node key age: replace on a clock and not only on a fetch; stop taking a key past its day; the client ages its cached copy | — |
| B-17 | Settings for a node's retention and board queue, and the person's board pool, which the documents call settable | — |
| B-18 | The batch node-key prefetch the documents describe; every fetch is single today | — |
| B-19 | Queue cap in a launched node's configuration | D-06 |
| B-20 | Placement beyond the direct patron | D-07 |
| B-21 | Desktop shows unverified mail, as the phone does | — |
| B-22 | Desktop renewal | D-14 |
| B-23 | Withdrawn with D-12 | — |

### Desktop and phone

| # | Work |
|---|---|
| B-24 | Desktop: paging of earlier messages |
| B-25 | Desktop: .deb and AppImage packaging |
| B-26 | Desktop: the conversation card's last line; a resource's one-line notification |
| B-27 | Android Web Push; failover on iOS (deferred until iOS is built) |

### Documents

| # | Work | Waits on |
|---|---|---|
| B-28 | State the two residual exposures accepted with the minted key: the sender's address reaches the recipient's node, and the sender's own node sees where it relays | — |
| B-29 | Put the seal key's derivation label in a document; it is in code only | — |
| B-30 | The design is silent on unassigned nodes, the adoption-carried node and sealed storage, which live in the other three documents | — |
| B-31 | Say that a paired desktop reopens no sealed package, holding no seed; that a kernel fix waits for a restart; the board's two failover edges | — |
| B-32 | Design section 15: "a node's launch and closing" travel rootward, though nothing sends a closing | — |
| B-33 | The grace among design section 21's tunables | D-09 |
| B-34 | 44 functional rows are held by no test and no acceptance entry | — |
| B-35 | The change log | D-28 |

### Tests missing

| # | Test |
|---|---|
| B-36 | A sender holding only a card, end to end |
| B-37 | A form against a real package; an answer over the node's bound; a federated package. **Done 2026-11-24**: a slow package (PKG-021's end-to-end test), and a script page against a real package on GXV (the streaming face, `stream.internal`: 32 MiB in 32 parts at 1.76 MiB/s with the check matching, ten segments appended in 0.95 s and played to the end). The desktop's run of the same page is owed |

### Bench runs never made

| # | Run |
|---|---|
| B-38 | A sealed package across a droplet reboot, reopened by GXV's attach |
| B-39 | A paired desktop posting to the board |
| B-40 | Board catch-up after a phone was away |
| B-41 | A post relayed between two nodes (the bench has one) |
| B-42 | A restored desktop taking its place from its node |
| B-43 | ZU8's adoption with a restart before accepting; the MRK–4VC meeting; GXV adopting 4VC |
| B-44 | Back navigation on a phone; backup import on the desktop; the desktop's pinned conversation layout and board screens in the real window |
| B-45 | The DigitalOcean token flow, on hold |

### Faults seen and not chased

| # | Fault |
|---|---|
| B-50 | **A resource's page reaches outside the frame by a peer connection** (found 2026-11-24, GXV, Android WebView 157): with its code allowed, the page's `RTCPeerConnection` sent five STUN packets to an address and port of its choosing, by no request the kernel carried, and a `webrtc 'block'` policy did not stop them. Against light-client section 8. Fix: remove the constructors at document start in every window (`androidx.webkit`), verified by the streaming page's `reach` and `reach_inner` rows and the package's `stream.datagram` log; the desktop's engine to be checked the same way |
| B-46 | MRK's node-key request went unanswered and its profile requests were refused |
| B-47 | After accepting a node two levels up, a member's records still name its patron as the node serving it |
| B-48 | GXV's card for MRK shows 1 unread beside "no messages" |
| B-49 | The phone's "weird failure" of 2026-11-23, never described |

---

## The applications repository's own side

Not this repository's work; listed because the first deployment meets them.

- Academy serves nothing at `/` and no icon.
- It answers 404 to `PUT /.rhtn/relationships`, so it is never told its block or notices paths.
- Its outbox answers a bare array and reads the acknowledgement without `taken=`.
- Its documents still describe envelope signatures and instance keys.
- Its three protocol pins disagree, and the newest is 28 commits behind.
