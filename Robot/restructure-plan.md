# The restructure: a formal data model, and functions over it

Planned 2026-11-13 from [the review](code-review-2026-11-13.md) §7, at the
author's direction: *"a functional structure over a formal data model
rather than an object structure with private state and assumptions around
cardinality."* Assistant's document; nothing in the root cites it. Every
decision that is the author's is marked **[owed]** and listed at the end.

## 1. What the model must carry

The author's list, as cardinalities the model states and never assumes:

| Relation | Cardinality | Today |
|---|---|---|
| a person holds devices | one to many | nothing models the person; a device is where the shell runs |
| a device holds identities | many to many — an identity on many devices, a device holding many | the shell's index file; the kernel sees one identity per `Participant` |
| an identity holds addresses | one to many — one per subnet | `positions`, and `subnet()` meaning "the first" |
| an address is served by a node | one to one, at a time | `serving: Option`, `serving_at`, and the shell's `serving` file |
| an identity on a device holds sessions | one per address it attaches | `Net.live` singular, `tabs` beside it |
| an identity knows parties | one to many, per identity | `contacts.rs`, read through the focused participant |
| an identity has a conversation with a party | one per identity × party, in parallel across identities | the shell's `Front.convos` and `threads` file; nothing in the kernel |
| a light client and a node interoperate by the wire alone | — | holds for the wire; the state has no schema |

## 2. The model

Written once, as data, with its persistence and its rule beside each
relation. Keys are keyhashes (32 bytes) unless said; `identity` is the
identity the row belongs to, which **every row carries** so that no reader
ever asks "whose".

```
Identity   (keyhash, material)                              -- held on this device
Holding    (identity, device_key)                           -- this device's; the index
Address    (identity, subnet) -> anchor, locator, series    -- derived from the records this identity holds
Agreed     (identity, node) -> [host:port]                  -- the nodes agreed to, with the addresses the agreement named
Serving    (identity, subnet) -> node                       -- derived: Agreed ∩ the node serving that subnet
Attachment (identity, node) -> state, since, last_seen      -- live, not persisted
Party      (identity, other) -> how, alias, number          -- what one identity knows of another
Profile    (identity, subnet, kind) -> alias, image         -- what it publishes, per address
LastFrom   (identity, other) -> subnet                      -- written at every arrival, on every path
Thread     (identity, other)                                -- one per identity, in parallel
Message    (identity, other, seq) -> mine, text, delivery, reason, at
Record     (identity, txid) -> bytes                        -- the archive
Capture    (identity, other, ceremony) -> sealed, window    -- the one retention window (design §7.5.1)
Node       (keyhash) -> endpoints, operator                 -- what the horizon knows of a node
```

And the rules as functions over it, each pure, each with a test that
constructs rows and asserts on the answer:

```
addresses(identity)                 = Address where identity
node_for(identity, other)           = Serving[identity, LastFrom[identity, other]]
                                      else { Serving[identity, s] for s in addresses(other) }      -- the flood
attach_all(identity, device)        = for (node, addrs) in Agreed[identity]: attach(node, addrs)
admits(node, who)                   = a Record the node holds introduces who                         -- the node's side
offer_to_write(device)              = for identity in Holding[device]:
                                        for other in Party[identity] where how = met, no Thread[identity, other]
publish_profiles(identity)          = for subnet in addresses(identity): Profile[identity, subnet, *]
```

What the shell holds of its own after this: the window geometry, the
colours, the diagnostics. Everything a screen shows is a query keyed by
identity; everything a person does is a function call with the identity
named.

## 3. Persistence

Not a database engine: the wire format's discipline applied at rest. One
file per relation per identity scope, CBOR, a version word first, decoded
whole or refused whole, written when that relation changes — so a message
sent does not rewrite the archive, and a bad write loses one relation, not
the identity. The schema is a document and a pin: each relation's fields,
their meaning, which document's rule sets its retention and its
visibility, and a vector per relation in `test-vectors/`. **[owed]** where
the schema lives: a section of `light-client-requirements.md` §9, or a
root document of its own.

The migration: at the first start on the new build each identity's blob is
read once by the old decoder and written out as relations; the blob is
kept until the next start reads the relations back cleanly, then forgotten.

## 4. The order, and what each step proves

Each step is gated and installed; each removes one class of the veneer.

| # | Step | What it removes | Proof |
|---|---|---|---|
| 0 | ~~The seam scenario~~ **Done 2026-11-14**: `crates/participant/tests/seam.rs` (PRT-11), over the scripted participant `rhtnp` (now with `agree`/`agreed`/`disagree`/`attachments`/`detach-from`/`invitation`/`save` and a `--store` so a restart holds what it held) and two spawned `rhtnd`, each speaking as its operator; 68 s; found two defects on its first runs (the archive loader's order, ARC-019; the driver must save as the shell does) | the bench as the only test of composition | it runs in minutes and fails for each of this week's defects when reverted |
| 1 | ~~`Agreed`, `Serving`, `LastFrom` as relations in the kernel~~ **Done 2026-11-14**: `Agreed` and `LastFrom` were correction 3's; this step removed the last singleton, `PayloadState.serving` (its storage slot written empty so older blobs parse), made the catalog sweep name the node it reads (`Client::browse_at(node)`, `browsing_at()`, the courier sending over the session held with that node, the FFI `browse(node)`, the shell sweeping each node an identity is attached to), published a signed bundle wherever a session is held, and deleted the shell's `serving` file format and its one-time import (`Serving.kt`, `ServingTest.kt`). `Serving (identity, subnet) -> node` stays `serving_at` for now; deriving it from `Agreed` is step 5's, with the relations split | the shell's `serving` file; `serving: Option` | the scenario, plus relation tests |
| 2 | **Restated 2026-11-14 [author]**: the kernel offers the shell a sealing service (encrypt/decrypt bytes, keys never leaving the kernel); the thread store stays the shell's, keyed by identity, as an append-only file of per-message ciphertext through that service; `Front.convos`/`Threads.kt` are rewritten onto it rather than moved into the kernel | conversations in the clear at rest; the history resealed whole on every change | the scenario sends, restarts, reads back; the stored file holds no plaintext; the (N+1)th message appends |
| 3 | `Net` as a map over `Attachment`; one `watch` per attachment; status per attachment | `live`/`current`/`status` singulars; `primary: bool` | failover and re-attach per attachment in the scenario |
| 4 | The shell model keyed by identity everywhere: `Model` rows carry `by`; the focused-identity accessors go; `Kernel` split into `Identities`, `Attachments`, `Ceremony`, `Levers` | two notions of "whose"; the god object | the pins become behavioural tests of the split objects |
| 5 | The client blob split per relation with versions, with the migration | the 2.6 MB whole-state write; versioning by array length | a vector per relation, pinned |
| 6 | The FFI: `Refused` with a kind; the ~50 callerless exports pruned; `local-interfaces.md` carries the data model | string-matched errors; surface nobody owns | binding regenerated, shell compiles against the smaller surface |
| 7 | The registers linked: families cited by acceptance entries, `check.py` reports families with no test | "is it tested" by reading | the report |

Estimates: 0 — 3 days; 1 — 2; 2 — 3; 3 — 2; 4 — 4; 5 — 3; 6 — 2; 7 — 1.
About four weeks at the pace of the last two, with the bench run at steps
0, 2, 3 and 4.

## 4a. Step 2 as designed, 2026-11-14

**The service** (`light-client-requirements.md` §9, APP-015). The kernel's
storage seam already seals every blob the kernel writes under the key the
platform's custody keeps (`SealedStore`, `rhtn_client::sealed::seal(key,
name, bytes)` with the name bound as associated data). The service is
that primitive offered across the FFI under a key *derived* from the
storage key — `sha256(key ‖ "application")`, so no blob of the
application's can be read as one of the kernel's or the reverse — as
`Participant::seal(label, bytes) -> bytes` and `Participant::open(label,
bytes) -> bytes | refused`, the label bound as associated data so a blob
cannot be moved from one place to another. Where the platform declares
itself unsealed (the terminal instrument) both pass bytes through, which
is the one declared exception §9 already has. The key never crosses the
seam; a shell holds only ciphertext and what it has opened.

**The thread store** (the shell's, keyed by identity). One directory per
identity scope, one file per thread: `threads/<peer hex>` holding
records `[len][sealed][len]`, the trailing length so the file is walked
backwards; each record is one message or one delivery change as JSON,
sealed under the label `<by>/<peer>/<seq>`; a sealed `threads/index`
naming each thread's peer, name, direct flag and count, rewritten when a
thread is added or renamed and never on a message. **Lazy** [author]: a
thread's file is opened when its conversation is opened and read
backwards for the newest page (50 records), an earlier page read when
the person scrolls to the top; the list screen reads the index alone, so
nothing of a thread not opened is decrypted at start, and `Front` holds
only the pages read and the count of what it has not. Append is the only
write a message causes; the sent/unsent change appends too and is folded
in on read.

**Migration**: the one JSON file `threads` of the 12th is read once, its
threads written out as files, and forgotten.

**Tests**: `ThreadsTest.kt` rewritten for the record framing and the
backward walk with a fake sealer; the Rust side's `seal`/`open` round
trip, the label binding (a blob opened under another label is refused),
and the derived key differing from the storage key; the seam scenario
unchanged, since the instrument keeps no threads.

## 5. Rules the restructure must not move

- The wire stays as it is: the local model changes nothing on the wire,
  and `wire-format.md`'s singular session is the micro-scale description
  the author ruled it to be [2026-11-11].
- No shared state: every relation is one identity's, on one device.
- Nothing but a sealed capture has a retention window (design §7.5.1).
- The kernel seals what it persists; the shell keeps the key
  (`light-client-requirements.md` §9).
- An identity without an agreed node is detached; the bench's provision is
  a start-time convenience [author, 2026-11-12].

## 6. Decisions owed to the author

1. ~~Where the schema lives~~ **Ruled 2026-11-14 [author]: in
   `light-client-requirements.md`** — *"these are internal Light Client
   schemas, not wire objects, so they belong in the Light Client design."*
2. ~~Whether the model carries a `Person`~~ **Ruled 2026-11-14 [author]:
   no.** *"The whole of a light client is presumed to be a person (the
   device owner), but is not manipulated as an object anywhere in the
   system."* Outside formal models: "a Light Client (presumably a single
   user)" or "a User (all identities active on a light client)", the
   alternative as a parenthetical where confusion is possible; "the user"
   is fine for UI behaviour and inputs. Recorded in
   `Robot/authoring-conventions.md`.
3. ~~Conversations: sealed with the rest, or a stated exception~~ **Ruled
   2026-11-14 [author]: neither — a general sealing service** (light-client
   §9, APP-015; step 2 restated above). Was:
   **Open until then** — asked again with the context: the kernel
   seals everything it persists under the Keystore key; the shell keeps
   conversations in plaintext files beside it. Either the thread store
   goes through the kernel (sealed, which step 2 does anyway) or
   light-client §9 states what the shell may keep in the clear.
4. ~~Whether verifier, nominee and introduced parties are offered as
   contacts to write to~~ **Ruled 2026-11-14 [author]**: not listed on the
   main DM screen unless a thread is active; they stay in contacts; a
   search by name to start a DM may come later.
5. ~~Whether step 0 may use the fieldtest build's levers as the driver~~
   **Restated 2026-11-14**, the author did not recognise the question:
   step 0 is an in-process test of T5–T7's shape (two daemons, two
   participants, adoption, enrol, invite, tab, message both ways,
   restart). Something has to play the shell — press the buttons the
   author pressed on the bench. The choice is between a small Rust driver
   written for the test that calls the kernel the way the shell does, and
   reusing the fieldtest levers (the `am start --es` hooks) by running the
   real shell under the Android unit-test JVM. Assumed meanwhile: the
   Rust driver, since the levers need the Android runtime and the test is
   meant to run in the gate in minutes. **Ruled 2026-11-14 [author]: the
   Rust driver, "unless and until we start running into significant
   impedance between the test cycle as it runs in the live tests and its
   behavior in the automated tests."**
