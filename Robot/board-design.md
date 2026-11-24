# The discussion board — design draft for review

Drafted 2026-11-23 for the author's review. Nothing here is in a root document
yet. Each rule says whose it is: **[ruled]** with the date where the author
already said it (most of it is recorded in `light-client-screens.md`, *Subnet
view*, from 2026-10-27 to 2026-11-18), **[proposed]** where it is mine and
waits on him. The questions at the end are the ones that block writing it into
the design.

## 1. What it is

- **One board per subnet, a standard node feature** [ruled 2026-11-23]: not a
  package, not a resource, not on any catalog page. Every node serving a subnet
  carries its board; a client shows it on the subnet's page beside the
  launchers [ruled 2026-10-27: *"the content area split between navigation
  buttons to that subnet's resources and a scrolling view of its chat"*].
- **A node serves one subnet** (design §3.1), so it holds one board
  [ruled 2026-11-18].
- **Threads with titles; a reply inherits its thread's scope** [ruled
  2026-10-27]: the reply screen states the scope rather than offering it.
- **A post is signed, not encrypted** [ruled 2026-11-18]. So a node holds
  board content in the clear, which design §18.1's seized node would expose:
  [proposed] said in design §18 as the cost of a board, and in the client's compose
  screen in no words at all (the author's no-warnings rule) — the scope
  count is what tells the writer who reads it.
- **Why it is in the protocol at all** [ruled 2026-11-23]: *"handling the
  propagation rules is a part of the protocol since it is necessary for
  interoperability"*. Two nodes of different implementations must agree on who
  a post reaches; what the board looks like is each client's.

## 2. Reach: who reads a post

All [ruled 2026-11-18] unless marked.

- **A post carries `+n/-m`**: `n` levels up and `m` down, along the subnet
  tree, relative to the writer. **Default `+2/-2`**; the selector moves either
  by one. The named scopes (*this subnet*, *and below*, *and above*) are gone
  [ruled 2026-11-18].
- **Up to `n`**: each ancestor up to the `n`th, together with that ancestor's
  direct subordinates, and nothing further down their branches. So `+1/-0`
  reaches the patron and the writer's siblings; `+0/-0` is a note to self;
  `+0/-1` the writer's direct subordinates.
- **Down to `m`**: the writer's own subtree to `m` levels.
- **Read from where the reader stands** [ruled 2026-10-27]: a `+3/-3` post is
  `+2/-4` to the writer's patron and `+4/-2` to the writer's subordinates. The
  band of positions reached never moves; only the coordinates do. A client
  renders the scope in its reader's coordinates.
- **Nothing ever comes from another subnet**: a post from four tiers up is the
  same subnet seen from further away [ruled 2026-10-27].
- **Wide distribution is a patron's act**: a higher patron sees a `+4/-3` post
  and may repost it, say as `-7`. Curating and setting priority is part of their
  authority [ruled 2026-11-18].

## 3. Transmission authority: how far up a member may post

[ruled 2026-11-18] *"This is to prevent upper levels of the heirarchy from being
spammed with messages from weakly attached, low prestige members."*

- **Everyone may post `+2`.** Each level of downline depth adds one: direct
  subordinates allow `+3`, grandsubordinates `+4`, and so on.
- **Enforced by the first node a post reaches**, from its own view of that
  member's downline depth.
- **A post over the cap is propagated with its cap lowered** to the sender's
  authority, not refused.
- [proposed] **The downward count is not capped**: the ruling caps upward reach
  only, and a member's own subtree is theirs to address at any depth.

## 4. The node's view of its downline

[ruled 2026-11-18]

- **A positional table, persisted**: one entry per downline member, keyed by
  its path of child indices below this node, each holding the occupied child
  indices under it and whether it operates a node. For example, `3[0,2,5,8]` is
  the sub in slot 3 with subs in 2, 5 and 8, running a node; `3,5,9[1]` is a
  great-grandsub with one sub and no node.
- **Built from the rootward memos alone** (`wire-format.md` §10.2). A fill memo
  adds a slot. An emptying memo (departure, disavowal) removes it **and every
  entry under it**. So depth is exact both ways. No identities are held to
  arbitrary depth, and no message is cached for it. *"I am open to other graph
  compaction schemes, but the important part is that you keep a current view of
  your downline in such a way that you do not have to cache every message or
  hold the ID of every subordinate to arbitrary depth."*
- **Node operation is a byte outside 0–9 in the entry's list** [ruled
  2026-11-18]. It is learned from the endpoint records of the infra nodes that
  attach to this node as their upstream, with no new wire field.
- **A re-adopted member's carried subtree sends no memos and reads shallow
  until it changes**: an undercount, and so a capped post [ruled].
- **Not built.** No code holds this table yet (`propagation.rs` has
  `downline_contains` over the topology table and nothing positional).

## 5. Forwarding

[ruled 2026-11-18]

- **The decision is the node's**, not the board client's.
- **Upward always goes on**: every node above, within `+n`, gets the post,
  because each node's owner is attached to their own node.
- **Downward is per node, from its downline**: which downline nodes must receive
  a post so that every user in scope is reached, users attaching to their
  nearest node.
- **Three lists, for `-1`, `-2` and `-3`**, computed once per change in horizon
  composition (adoption, departure, node launch, node closure), not per message.
  `-0` is trivial. Anything deeper uses `-3`'s list, with the real count carried
  on (`min(k, 3)`, downward leg only [ruled]).
- [proposed] **A node delivers a post to every member it serves whose position
  is in the post's band**, and forwards it to the nodes its lists name, each of
  which repeats the test from its own position. Nothing is forwarded to a node
  whose band would be empty.
- [proposed] **A post seen before is not forwarded again**, by its identifier
  (section 6), which is what stops an up-then-down path from echoing.

## 6. The post as nodes must agree on it

[proposed; see question A] Interoperability needs nodes to agree on a handful
of fields, whatever a client draws:

| Field | Why a node needs it |
|---|---|
| the subnet id | a node serves one subnet, and a post from another is not its board's |
| the author's keyhash and position | the band is computed from the writer's position, and the cap from their downline |
| `n` and `m` | the band |
| the thread it belongs to, or none for a new thread | a reply inherits its thread's scope, so the node checks it against the thread's own |
| a timestamp | the client's pool drops by it (section 7) |
| the body | the client's to read; bounded |
| the author's signature over all of it | a post is signed (section 1), and the first node checks it before it enforces the cap |
| an identifier: the hash of the signed post | duplicate suppression (section 5), and how a reply names its thread |

## 7. Storage and catch-up

[ruled 2026-11-18]

- **The node keeps a storage-limited FIFO queue** and drops first-in-first-out
  as packets arrive. There is no time component.
- **The light client keeps a storage-limited pool, one per subnet**, and drops
  the oldest first by the post's own timestamp, since it reads content rather
  than arrival order.
- Provisional figures [ruled]: 100 MB per node, 50 MB per subnet on the phone.
- **A returning client fetches what it missed as it fetches its mailbox**
  [ruled].
- [proposed] **And is told of a new post as of anything else routine**: over
  the session it keeps [ruled 2026-11-17], with the board's unseen count rolled
  into the subnet's orb beside its resources' [ruled 2026-11-23]. Unlike a
  resource's count, the board's is the client's own: the client holds the pool,
  so it knows what it has shown.

## 8. What this does not need

- **No catalog entry, no grant, no role**: membership in the subnet's positions
  is the gate, computed from the tree.
- **No archive record**: a post is not a transaction, which is why the board
  was never a protocol archive [ruled 2026-10-28].

## Answered 2026-11-23 [author]

**A. What a node reads.** Already settled, and I had lost it: a post rides
payload messaging with distant nodes as relays [2026-10-28: *"shell-native over
payload messaging with distant nodes acting as relays for unlimited fanout in a
single subdomain. It may be beneficial to add a new message type to the
protocol"*], and **the `+n/-m` count is a visibility counter each node adjusts as
it forwards, which "doubles as a routing signal to downstream nodes"**
[2026-11-18]. Today: *"the node should just deliver dumb opaque blobs and the app
should retain responsibility for presentation, including unreads."* So:
- **A node reads the counter and nothing else of the post.** It learns the
  sender from the session the post arrived on, which is what the first node's
  transmission-authority cap needs; the signature, thread, timestamp and body are
  inside the blob, which the apps verify and present. Section 6's table shrinks
  to two fields on the wire: the counter, and the blob.
- **The carrier is one new message type** carrying `[up, down, blob]`, the
  counter in the forwarding node's own coordinates.

**B. The unread count is the app's**, which holds the pool: the node never sees a
post's timestamp, which is why the two keep different retention.

**C. Only upward reach is throttled, as a matter of principle.** *"Since the rule
is that you can reach upward 2 layers plus the depth of your infra-operating
downline, the reach boundary would always equal or exceed the downline depth even
if the rule was symmetrical."* Note the wording: **the depth of the
infra-operating downline**, which is what the node-operation flag in the table
measures.

**D. The downline table is built with the board but is application-wide state**,
for other purposes later, not encapsulated by the board.

## The mechanism, derived from the rulings above [proposed, 2026-11-23]

Reach has two legs, and the counter has to say which one a copy is on. Your reach
rule: upward, each ancestor to `n` *and that ancestor's direct subordinates, and
nothing further down their branches*; downward, the writer's own subtree to `m`.
Re-expressing the counter alone (`+3/-3` becomes `+2/-4` at the patron) would
send the patron's whole subtree to depth 4, which reaches into the siblings'
branches. So the copy that travels up and the copies that travel down are
different copies:

- **The carrier is one new request type**, `BoardPost`, carrying `{subnet, up,
  down, blob}`. `up` and `down` are the counter in the receiving node's own
  coordinates.
- **At the writer's node**, the first node, `up` is capped at the writer's
  transmission authority. That is 2 plus the depth of the writer's
  infra-operating downline, from the downline table. The node then:
  - delivers to its own attached members in band;
  - sends one copy up, `(up - 1, 1)`, to its patron's node, if `up > 0`;
  - sends copies down to the nodes its `-min(m, 3)` list names, each carrying
    `(0, m - d)`, where `d` is that node's depth below this one.
- **On the upward leg** (`down` is 1), a node delivers to its owner and its
  direct subordinates, and its `-1` list carries the copy to the subordinates
  that run nodes, which deliver to their owners and go no further. While
  `up > 0`, it sends `(up - 1, 1)` on up.
- **On the downward leg** (`up` is 0), a node delivers to its attached members
  within `down` levels and repeats the list step.
- **No node forwards a blob it has already forwarded**, keyed by the blob's hash.
- **A client posts by the same request to its own node**, and reads what it
  missed from that node's queue on attaching, as it reads its mailbox.
- **What a node keeps** is the blob, the counter and the subnet, in its 100 MB
  FIFO. It holds no author, no thread and no timestamp, which are inside the
  blob.

This is mine, not yours. Its one judgement is that the counter's `down` on the
upward leg is fixed at 1, which encodes "and that ancestor's direct
subordinates".

**Approved 2026-11-23** [author: *"Yes, that is the correct behavior"*]:
two legs, with the upward copy's `down` fixed at 1.

## Gaps in that mechanism, found writing it into the design (asked 2026-11-23)

1. **Not every level runs a node.** A member attaches to its nearest
   infrastructure ancestor (design section 12.6.1), so the first node may
   sit several levels above the writer, and a node's upstream may sit
   several levels above its own owner. A pair `(up, down)` cannot say
   "these ancestors along this branch, and their direct subordinates" at a
   node serving several levels.
   - *Proposed*: the receiving node reads the branch from the session the
     copy arrived on (its downline table gives the path to the sender) and
     applies the band to each position it serves.
     - On the upward leg: each of the writer's ancestors within `up`, and
       those ancestors' direct subordinates.
     - On the downward leg: every position within `down` below the
       writer's line.
2. **Catch-up needs a cursor.** A node's FIFO has many readers, so nothing
   is deleted on a read, and it holds no timestamps.
   - *Proposed*: the node numbers what it stores, and a returning client
     asks for everything after the last number it holds. A number fallen
     out of the FIFO answers from the oldest held.
3. **The downline table is required state** (transmission authority reads
   its depth). Design section 15.2.1 calls the memo table optional, and
   section 12.6.1 says a node must never be required to hold a map of the
   subnet.
   - Positions without identities is the author's own compaction
     [2026-11-18]; whether it satisfies section 12.6.1 or amends it is his
     call.

**Answered 2026-11-23** [author]: yes to 1 and 2. On 3: *"positions-only
satisfies 12.6.1; that is what we designed it for."* So section 12.6.1 stands
as written, and the design says why the downline table meets it: it holds no
identities, and section 12.6.1 forbids requiring a map of who is where.

## Three more, asked and answered 2026-11-23 [author]

Asked after reading design section 12.6.1 again; two of the three were
already settled and I had lost them.

1. **How a node learns its downline's shape.** *"You populate the subnet map
   by a combination of adoptions seen within the trust horizon and
   annotations based on the rootward and leafward propagating address
   change/cycle detection messages."* So: the topology the node stores within
   its own horizon (adoptions, endings, endpoint records, which flood both
   ways) gives the near part exactly, and the rootward memos give positions
   at any depth. No new wire field.
   - *The leafward one* is the downward memo of wire-format section 10.2.4,
     the same frame sent down the other branch; I had lost that it exists.
     A memo passing through in either direction updates the table.
   - *My reading of "address change messages"*: the endpoint records, the
     topology class's address objects, which flood the horizon both ways.
     Unconfirmed.
   - *What that leaves*: beyond the horizon a memo says a slot is filled and
     not that its occupant runs a node. **Assumed, asked**: the topology rule
     of design section 3.3 fills it in. No position lies more than two levels
     below a node, so a filled position at depth k of at least 3 below a
     writer means a node at depth k-2 or deeper in that writer's downline.
     The count is the larger of the deepest node the table marks and that
     floor, and never over-counts.
2. **Deep forwarding.** There is no such edge: *"there can be no node more
   than two steps below the nearest infra node by the basic topology
   rules."* Design section 3.3 (L = 2): a node's nearest downline nodes stand
   one, two or three levels down and never further, which is why three lists
   are all of them [2026-11-18: *"propagation for all magnitudes beyond -3
   will be the same"*].
3. **Blob bound** 16 KiB, node queue 100 MB (about 6,400 full-size posts):
   *"These assumptions seem good to start with."*

## The wire shape, as written into the documents 2026-11-23

- **One control frame, type 10, `BoardPost`**: `{up, down, blob, number?}`,
  no reply, in three directions. Who the counts are measured from is known
  from the session it arrived on:
  - from a member: from the writer, as written;
  - node to its upstream (the upward leg): from the *sending* node's
    position, `down` fixed at 1. Sender coordinates, because the node above
    may serve light ancestors that are in the band while its own position is
    not, which a count from the receiver could only say with a negative
    number;
  - node to a downstream node (the downward leg): from the *receiving* node's
    position, `up` 0;
  - node to a member it serves: both 0, with the node's number.
- **Catch-up is request type 17**, `{nonce, after?}`, answered with numbered
  posts in the asker's band, oldest first, and whether more are held.
- **Raised with the author, not built**: every node on the way up could apply
  the cap to the position a copy arrived from, which would make the cap hold
  against a node that does not apply it for its own members. The ruling is
  the first node only.

## Built 2026-11-23: the downline table

`crates/node/src/downline.rs`, wired into `NodeView` (`downline`), kept in a
file `downline` beside the derived state and read back at start.

- **Sources**: the node's own slots (`set_slot`), the bindings its table
  holds beneath them (`sync_downline`, after every stored transaction and
  endpoint record, and after a rebuild or restore), and every memo passing
  through (`note_downline`, in `route_memo` before any routing decision).
- **A memo is read as a hint**: not applied to one of the node's own slots,
  not applied to a slot the table holds a signed binding for, and from below
  only about the branch it came up (`path_to` the sender must prefix it).
- **Node marks** come from `table.is_infra`, which endpoint records set.
- **Not built**: nothing reads it yet. The board (frame 10, request 17, the
  queue, the client pool, both UIs) is next.

**Found on the way, not fixed**: no code originates a memo for a slot of a
light-client patron. `originate_memo` fires for the node's own slots only,
and the client crate sends none. Design section 15.2 has a memo for every
membership change, sent through the nearest infrastructure ancestor
(functional test TOP-023). The effect here is that an upstream node does not
see the light tail beneath a downstream node, so depth reads up to two levels
short and never long.
