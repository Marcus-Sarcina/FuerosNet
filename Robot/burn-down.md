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
| D-03 | Plain-HTTP limits: no redirects, cookies, websockets or streaming | **Decided 2026-11-24: stated as the contract, large and continuous content going as parts** (resource-requirements section 3), **and live and streaming built now**. **A live session goes to a media port the package holds on the node's own host** [author, 2026-11-24], signalled through the ordinary path; the reason is his flat-fee rule (one machine, a bigger VM before a second account) **Ruled the same day**: any real-time connection goes through `live`, whether or not the member sends capture; a package declares one UDP port above 1023 in its manifest and a clash is a refused install, the node assigning none | — |
| D-04 | A package answer's size | **Decided 2026-11-24: the node is raised to 16 MiB**, the whole message held to the wire's bound (PKG-022) | — |
| D-05 | Whether a node on the developer's own machine is a supported path | **Decided 2026-11-24: supported.** `local.sh` gains what the install act needs and goes into the gate (B-06, B-07) | — |

### Protocol and node

| # | Decision | Options | Meanwhile |
|---|---|---|---|
| D-06 | Queue cap on a launched node | **Decided 2026-11-25: 256 MB**, written into the configuration a launch makes ("Leave the 256MB cap") | — |
| D-07 | Whom a node places at attach | **Decided 2026-11-25: extended to members served from above**, the operator's own case not taken up. **And the retention it needed, ruled the same day**: "The node keeps every record it sees that impacts the composition of it's trust horizon (Adoptions, departures, disavowals, rotations out to the two-hop boundary)", **narrowed by him the same day** once the source of r09 was traced to his statements of 09-21 and 09-23: "Option A, adoptions in force within two hops. Also rotations in force if they are more recent for that userID than the last adoption. Departures/disavowals replace both adoptions and rotations for the ID" and "The tables are still the reference though, these records are just for proof of identity purposes". Built as: per relationship the table holds, the store keeps the record in force under `tx/` (the adoption, with the series reissue that moved it; or the departure or disavowal that ended it) and removes what it superseded; the kept records are never folded, so a discarded table is repaired as before 2026-11-25 (peerings and presence evidence are held while running, as before); a node hands a member whose patron holds a slot there and runs no node the adoption those two signed; the device takes it only from a node answering in mode 0, holds it unkept until the patron's material arrives in the node's own adoption of that patron, and drops it at that node's silence in another mode. Stated in infra section 4.3, wire format section 8.2, design sections 15.1.1 and 23.3, light-client sections 2 and 4.2, TOP-035, APP-019. **Put to him with it**: whether peerings are kept too (not on his list, so not); that the device takes the patron's material from a record carrying it | — |
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
| B-12 | `live` on the phone: **built 2026-11-25** (see B-55); video capture, PDF scan and `uwb` are still declined there | — |
| B-13 | Install and offering screens on the desktop (phone only today) | — |
| B-14 | The install act's seal warning says less than light-client section 10 requires | — |
| B-15 | `crates/packaging/README.md`: the "carries no control yet" passage predates the acts | with B-01 |

### Protocol and node

| # | Work | Waits on |
|---|---|---|
| B-16 | Node key age: replace on a clock and not only on a fetch; stop taking a key past its day; the client ages its cached copy | — |
| B-17 | Settings for a node's retention and board queue, and the person's board pool, which the documents call settable | — |
| B-18 | The batch node-key prefetch the documents describe; every fetch is single today | — |
| B-19 | Queue cap in a launched node's configuration. **Done 2026-11-25**: `rhtn_launch::QUEUE_CAP_BYTES`, `queue-cap = 268435456`. The bench droplet was launched before it and stays uncapped until it is launched again | — |
| B-20 | Placement beyond the direct patron | **Built 2026-11-25** (D-07): `View::placement_of`, `Client::placed_by` and `take_propagated`; node and client tests under TOP-56. Not yet measured on the bench: needs a member under a phone-only patron restored from the phrase |
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
| B-50 | **A resource's page reaches outside the frame** (found 2026-11-24, GXV, Android WebView 157, measured with the streaming package's probe page against listeners on the droplet). **Before**: with its code allowed, a page reached an outside address seven ways, none by a request the kernel carried: a socket from its own window, from a window it made, and from a worker (2 TLS connections each); a preconnect link (2); a datagram transport (6 datagrams); a peer connection from its own window and from one it made (10 STUN packets). A fetch and a beacon reached nothing. `webrtc 'block'` in the policy stopped nothing. **Built**: a proxy override sending everything the engine would fetch for itself to a dead port of the device (`Frames.wall`, opened for a node's own origin while its page is in front), and the peer-connection constructors removed at document start (`Frames.NO_PEERS`), both through `androidx.webkit` 1.14.0, a dependency he allowed. **After**: 0 connections on all five ports, 0 transport datagrams, no peer connection in the page's own window; **a window the page makes for itself still has one, and 5 STUN packets arrived**. That last route cannot be shut by script in this engine. **Owed to him**: accept it and correct the wording on the frame and in light-client section 8; or a deeper build (a walled process with a syscall filter, or another engine). The desktop's engine is still to be checked the same way. **GeckoView trial, 2026-11-24, at his word** (`Robot/gecko-trial/`; GeckoView 161.0.20261118180411, GXV; the same probe page, served by the bench package on the laptop and carried to the engine by a relay on the phone's loopback standing where the kernel would; listeners on the laptop). Engine as shipped: 3 socket connections (window, inner window, worker), 20 from a fetch and a beacon, 14 transport datagrams, 12 STUN packets from both windows. With a dead proxy for everything but the relay's own port, peer connections, the datagram transport and speculative connections switched off by preference, set from a file the embedder names: **0 connections and 0 datagrams on every listener, no peer connection in either window, and the page whole** (32 of 32 parts, clip decoded, 10 segments). The same in a build that is not debuggable. Aimed at the phone's own loopback: 24 connections open, 0 walled. Not measured: the engine's own background requests; the preference flipped for a `live` session; storage per resource; a release build's size (the library is 155 MB unpacked for one processor type; the trial app is 214 MB). The library asks to be compiled against platform 37.2 and this machine has 37.0. **Peer connections left on, 2026-11-25, at his question whether the loopback can catch and permit them.** (1) Held to the loopback interface (`media.peerconnection.ice.force_interface` "lo", `ice.loopback`): no effect, 12 STUN packets reached the laptop and those sent to the phone's own listener came from its WiFi address. (2) Told to reach peers only through the proxy (`media.peerconnection.ice.proxy_only`), the proxies being the trial app's own and logging: the page has a peer connection and **0 datagrams left, to the laptop or to the phone's own listener**; every stream the page tried was asked of the app and refused there (sockets from the window, the inner window and the worker by SOCKS; the fetch and the beacon by CONNECT; and the engine's own request to `safebrowsing.googleapis.com:443`); **but the peer connection's relay (`turn:` over a stream) was never asked of either proxy, permitted or not, and the engine reported the relay broken**. So a stream is caught and can be permitted by destination; a peer connection is shut, and letting one through the loopback is not shown |
| B-51 | **The media port, what is built and what is not** (2026-11-24). Built: the manifest's `media_port`, its refusals at install, `rhtn-media` told on the relationships exchange and again when the endpoint record moves, the three documents and PKG-023. Not built: the phone and the desktop giving a frame a peer connection inside an approved `live` session, for the seconds agreed, with an indicator; the bench package's face that terminates one (`str0m` intended); the bench run. **On the droplet** (node `b46039a2-bench`): an install declaring 7431 was refused as the node's own port and one declaring 443 as below 1024, each leaving the running package untouched; declaring 7462 it was installed and logged `stream.media` at `137.184.207.63:7462`. The probe cycle is `Robot/frame-probe/` (deploy, open, read) |
| B-52 | The node's own page runs its three top links together as one ("The nodetestapp.internalstream.internal"), seen on GXV 2026-11-24 |
| B-53 | **The desktop frame's engine opened a connection of its own for a preconnect link** (found 2026-11-25 on his run of the probe page: 1 connection to the droplet's listener per page load; peer connections, sockets, fetches and the datagram transport were all shut there). Reproduced in `examples/frame_check.rs` with a listener of its own, for a page whose code was refused too: 2 connections. **Fixed**: the frame's engine is sent to a dead proxy (`window::NOWHERE`), after which the check counts 0 with its other rows unchanged. **Confirmed on his second run**: 3 reports from the desktop and 0 arrivals at the droplet's listeners |
| B-54 | On the desktop the probe page's whole clip is refused (media error 4) while the same clip as segments plays; not chased |
| B-55 | **GeckoView adopted for the phone's resource frames** [author, 2026-11-25], with leave to download the library and platform 37.2 (installed). **His rule for streaming** [author, 2026-11-25]: the loopback foils a page-within-a-page on any page; streaming is allowed through channels a package defines, which its operator can read before deciding to trust it, and such a stream need not go through the loopback; his examples of a condition are a list of trusted endpoints, and endpoints verified to belong to known authorised users of the resource. **Built 2026-11-25**: `Engine.kt` (GeckoView 161, told one proxy and no peer connection, read back before any frame), `Carrier.kt` (that proxy: this app's own, on the loopback, answering a framed resource's origin through the kernel to the holder of a per-process secret and refusing everything else), `rhtn.request` published by a built-in extension, a resource's origin now `http://<keyhash in two labels>.localhost`, the screen on a `GeckoView`; the node's page stays on the system engine behind its wall. **Measured on GXV through the droplet**: 0 arrivals at any listener, no peer connection in either window, 32 MiB in 32 parts in 14.28 s (2.24 MiB/s), clip decoded, 10 segments appended, `rhtn.request` present, secure context. CAT-031. **Ruled 2026-11-25**: channels are declared in the manifest; a condition is the package's promise to its operator, which the phone does not check; members' devices streaming to each other is intended, being a media layer on the address disclosure direct messaging already makes; **and what this secures against is content riding in an otherwise trustworthy application, an advanced tracking pixel, not a malicious package**, the core's packages being trusted and third parties' left to the community to vet. Built on that: `media_endpoints` and `media_members` beside `media_port` (PKG-024), and the three documents. **The `live` session on the phone, built 2026-11-25** (`Giving.Way.LIVE`, `Engine.peers`, `Engine.only`, the screen's countdown and its ending, camera and microphone through the engine's permission delegate, `RECORD_AUDIO` declared; CAT-032). **Measured on GXV through the droplet** with a bench image that asks for 45 seconds before its page: in the session the page reported a peer connection in both windows and 12 STUN packets reached the package's media port, 32 of 32 parts were still carried and no socket, fetch or link reached a listener; after 45 seconds the page was loaded again and reported none; leaving the page at 41 seconds left ended it at once. **Left**: an exchange of real audio or video, which needs the bench package to terminate one on its port (`str0m`) and the camera and microphone path exercised; `live` on the desktop; a page cannot start a session from a button, since an ask is read only where a screen opens a path or in answer to an ask, and one answered to a page's own request is dropped (B-56); playback waits for a tap (the engine's default); the device run `FrameIsolationTest` now covers the node's frame only and `MediaEgressTest` is gone, the resource frame's device evidence being the probe, and the two acceptance entries that cite those runs describe the earlier frame; the frame's answer is composed in Kotlin beside the desktop's Rust, one definition owed; the app is 357 MB in a debug build, up from 187; the library adds three permissions to the merged manifest (network state, wake lock, audio settings), which he said to keep [author, 2026-11-25]; only arm64 carries the engine |
| B-56 | **An ask answered to a page's own request is dropped** (seen while building `live`, 2026-11-25; so before it): the phone reads an ask where its screen opens a path and in answer to an ask, and a request a framed page makes that is answered with one gets "the resource answered with nothing". The documents say an ask is any answer so declared. So a page cannot start a live session, or any ask, from a button. The carrier now sees every such answer and could hand it to the screen. **Fixed 2026-11-25**: `Carrier.Serving.asked` hands it to the screen that framed the page, which puts the question in the page's place; the page's request is answered 204 with nothing. The documents said any request already; a sentence in each now says a page's own is among them. The bench page has a button for it (`POST /go-live`). **On GXV through the droplet**: the button drew the ask for 30 seconds, approving it framed the page with a peer connection, 12 STUN packets reached the media port, and at the end the page was loaded again with none |
| B-57 | `DiagFileTest.queued_lines_reach_the_file_in_order` failed once in a local unit-test run on 2026-11-25 and passed on the next with nothing of it changed; not chased |
| B-58 | **The bench package's media face** [author, 2026-11-25: `str0m` and its dependencies downloaded]: `crates/testapp/src/media.rs` holds the declared port, answers an offer posted to `/offer` as the lite end, echoes a channel and counts audio and video. Loopback test in the gate. **Measured on GXV through the droplet 2026-11-25**: ICE completed, 46 lines echoed, 651 video frames and 1,066,993 bytes in 23 s, audio 0 because the app does not hold the microphone on that phone and the platform's prompt is the person's to answer. Open: audio once the microphone is granted; the desktop's `live` |
| B-59 | **A client opens no channel and runs no code for a resource the person has not opened at least once, the notification count excepted** [author, 2026-11-25]. Audited the same day: pages, scripts, asks and live sessions already wait for an open; **the icon does not** (`icons_at`, `GET /favicon.ico` to every resource a sweep finds, at every start, his ruling of 2026-11-23). Not written into a root document until he says which gives way; no "opened once" record exists yet |
| B-60 | `r09_topology_persistence_must_not_keep_foreign_transaction_history` (the reviewer's) is not run since 2026-11-25. Under the narrowed ruling its adoption half holds (the superseded adoption is gone from disk) and its departure half fails (the departure is the record in force and is kept); run ignored-only it fails on that. Its assertion is untouched; withdrawal is the author's and was asked. **Also open with it**: how long an ended relationship's record is kept (built: as long as the table holds the binding, which is without limit); whether "rotation" meant the key rotation, the series reissue or both (built: both). **Stale beside it, not changed**: infra section 4.3's last two bullets still call the derived view a cache of a pure function answered by a replay, where design section 15.1.1 since 09-23 has repair; pinned by DMN-12 and DMN-16 |
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
