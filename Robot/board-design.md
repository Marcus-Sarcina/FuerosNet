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

## Questions that block writing this into the design

**A. "Does not require a wire definition."** Nodes of different implementations
must still exchange posts, and the fields in section 6 are what they would agree on. I
read your words as one of these:
1. **Posts ride an existing carrier unchanged.** [proposed] The relay a message
   already takes between nodes (`wire-format.md` §7.10), under a reserved
   payload kind, signed rather than sealed. Section 6 is then written in the design's
   propagation section as the post's contents, not as a new frame.
2. **The post is the board application's own format**, and nodes forward bytes
   they do not parse. But then no node can check the cap, the band or the
   signature, which section 3's first-node enforcement needs.

Which did you mean, or something else?

**B. The board's unseen count**: is it the client's own, since the client holds
the pool and knows what it showed (my section 7 proposal), or should the node count it
as a resource's count is kept?

**C. Downward reach uncapped**: confirm that section 3's authority limits only `+n`.

**D. The downline table is unbuilt** (section 4). Build it as part of the board, or
first as its own piece?
