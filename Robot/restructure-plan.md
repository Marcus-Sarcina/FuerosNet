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
| 0 | **The seam scenario**: two in-process daemons, two `Participant`s, a shell-shaped driver; T5–T7's shape as a test — adoption, enrol, invite, attach as a tab, message both ways, restart | the bench as the only test of composition | it runs in minutes and fails for each of this week's defects when reverted |
| 1 | `Agreed`, `Serving`, `LastFrom` as relations in the kernel (correction 3 is the first of these) | the shell's `serving` file; `serving: Option` | the scenario, plus relation tests |
| 2 | `Thread`/`Message` in the kernel; the FFI exposes queries by identity; `Front.convos`/`Threads.kt` go | conversations outside the model, in the clear | the scenario sends, restarts, reads back |
| 3 | `Net` as a map over `Attachment`; one `watch` per attachment; status per attachment | `live`/`current`/`status` singulars; `primary: bool` | failover and re-attach per attachment in the scenario |
| 4 | The shell model keyed by identity everywhere: `Model` rows carry `by`; the focused-identity accessors go; `Kernel` split into `Identities`, `Attachments`, `Ceremony`, `Levers` | two notions of "whose"; the god object | the pins become behavioural tests of the split objects |
| 5 | The client blob split per relation with versions, with the migration | the 2.6 MB whole-state write; versioning by array length | a vector per relation, pinned |
| 6 | The FFI: `Refused` with a kind; the ~50 callerless exports pruned; `local-interfaces.md` carries the data model | string-matched errors; surface nobody owns | binding regenerated, shell compiles against the smaller surface |
| 7 | The registers linked: families cited by acceptance entries, `check.py` reports families with no test | "is it tested" by reading | the report |

Estimates: 0 — 3 days; 1 — 2; 2 — 3; 3 — 2; 4 — 4; 5 — 3; 6 — 2; 7 — 1.
About four weeks at the pace of the last two, with the bench run at steps
0, 2, 3 and 4.

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

1. Where the schema lives (a §9 section, or a root document).
2. Whether the model carries a `Person` at all — the design's party is the
   identity; "a user can have many devices" is a fact about a person the
   protocol never sees. Assumed: no `Person`; `Holding` carries it.
3. Conversations: sealed with the rest, or a stated exception (review §4).
4. Whether verifier, nominee and introduced parties are offered as
   contacts to write to (review §7.2); assumed met-in-person only.
5. Whether step 0 may use the fieldtest build's levers as the driver, or a
   shell-shaped driver is written for it.
