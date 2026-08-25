# Resource interaction protocol — extracted requirements

**Status: closed 2026-08-16 as a workstream.** This was the working document for what
was then the last blocker in the design's open register.

> **Section references in this file are as-of-filing and are not remapped.** They were
> written against the design's numbering of 2026-08-16 and the document has been
> renumbered since; chasing them would falsify the record of what was asked and
> answered. Use the change log to follow a section through its moves. *(This notice
> was added 2026-08-25, after a sweep found twenty-three references here resolving to
> nothing or to unrelated sections.)*

> **The "closed" status was assigned too early, and the design says so.** An
> implementation attempt on 2026-08-23 could authorise a user end to end and could not
> carry a request, because the frames were named and never defined. *Closed* was
> assigned when this document's **design** questions were settled, which is a
> different claim from the encoding being complete. See design §18.2.

**Every question in it with design content has been answered**, and the answers live
in the specification:

| Answered | Where |
|---|---|
| Credential contents, pairwise principal | design §8.0.2, `resource-requirements.md` R0.1 |
| No credential forwarding, audience binding | design §8.0.4 |
| Resources assert nothing into the trust graph | design §8.0.3 |
| Request framing, header-spoofing hazard | `resource-requirements.md` R0.2 |
| Session termination on role change | `infra-client-requirements.md` design §8.4 |
| Owner movement | design §8.2 |
| Revocation scope, hosted vs brokered | design §8.2 |

**Two items remain, neither blocking**: whether an abuse report relates to a live
session, and what a resource may log (design §8.8).

**Retained as a record of the brief**, not as an open workstream. The reasoning
behind each answer is in the design; this shows what had to be answered and why the
list was closed.

---

## A. What the protocol sits between

**The infra node is the front door; the resource sits behind it** (design §8).
The shape is a reverse proxy with an authenticating gateway. The protocol being
designed is **what crosses the gateway**, in both directions:

| Leg | Status |
|---|---|
| Client → infra node | **Specified.** QUIC session, `Attach`, capabilities (`wire-format.md` §6) |
| Infra node → resource | **This protocol.** Undefined |
| Resource → infra node | **This protocol.** Undefined — resources may initiate (design §8.5) |
| Resource → external service | Out of scope; the resource's own business |

---

## B. Hard constraints — these are settled and the protocol must not violate them

1. **The resource never reads network state** (design §8). The node evaluates
   access and presents the result; the resource sees an authenticated principal
   with roles. **A compromised resource must leak only its own data, not the
   owner's archive.**
2. **Membership in the owner's Dunbar Org gates everything** (design §8.2). No
   grant reaches outside it. The node enforces this before the resource is
   consulted.
3. **Roles are declared by the package, bound by the operator** (design §8.4). A
   package cannot invent a role after installation; an operator cannot grant one
   the package does not understand.
4. **Two reserved actions**: `discover` (appears in the catalog view) and
   `connect` (may open a session). Everything beyond is resource-defined and
   opaque to the network (design §8.4).
5. **Discovery authority and connection authority are independently enforceable**
   (design §8.5). Holding `discover` does not imply `connect`.
6. **The catalog page is the front door for locally-hosted resources** (design
   design §8.5) — personalised, session-time, naming the viewer's roles.
7. **A `CatalogEntry` is optional.** A locally-hosted resource may have none and be
   reached only through its host (design §8.5).
8. **Payload to a resource is end-to-end encrypted to the resource** as the
   addressed endpoint (design §9.10). The resource is online by definition, so the
   design §9.9.1 transport handshake covers it — no PQXDH machinery needed.
9. **Sandboxed** (`infra-client-requirements.md` design §9.2): the resource receives
   exactly the host bindings granted and nothing else.
10. **The node-resource interface is an impenetrable trust boundary.** [D —
    2026-08-16] **A resource never passes a user's credential through to another
    resource.** Credentials are audience-bound: the node mints an assertion naming
    one resource, and any other resource MUST reject it.

    **Resource-to-resource trust is configured, never delegated.** If resource A
    needs resource B, that is a relationship between A and B — established by
    their operators — and it carries A's own authority, not a borrowed user's.

    **This removes the confused-deputy problem by construction.** A resource cannot
    be induced to wield a user's authority against another resource, because it
    never holds any. The alternative — delegation with guards — is what OAuth is,
    and most of OAuth's complexity and vulnerability history is the cost of getting
    those guards right.

    **What this forecloses, accepted:** a user cannot authorise resource A to act
    on their behalf at resource B. Any such composition is **operator-configured**,
    so it happens at organisational rather than individual scale. Natural for team
    tools — *"our tracker may post to our chat"* — and simply unavailable for
    user-scoped delegation.

    **Federation is unaffected** (design §8.0.1): instance-to-instance links in a
    subnet-wide service are peer relationships between operators, not credential
    forwarding. Each patron's instance authenticates its own users.

---

## C. Requirements stated but not yet given a mechanism

| # | Requirement | Source |
|---|---|---|
| ~~C1~~ | ~~How a request is framed~~ **RESOLVED** — `resource-requirements.md` R0.2: HTTP/3 over the existing QUIC session, credential in headers, with the header-spoofing hazard stated on both sides | design §8.0.3 |
| ~~C2~~ | ~~What the credential contains~~ **RESOLVED** — `resource-requirements.md` R0.1: pairwise principal, roles, audience, session validity | design §8.0.2, design §8.0.4 |
| ~~C3~~ | ~~How a resource initiates contact~~ **RESOLVED** — as a payload endpoint (design §9.10), addressable via `CatalogEntry`. Nothing new needed | design §8.0.3 |
| C4 | What happens when an owner moves — **partly resolved** — design §8.2: old upline loses access, down-line unaffected, **new upline gains it silently** (client warns). Hosting is the larger effect: a light-client owner's resource may have to migrate | design §8.2 |
| C5 | **Abuse-report delivery to the owner** is specified as a transaction (`wire-format.md` type 7) but its relationship to a live session is not | design §8.6 |
| C6 | **Resource access logging** — whether the protocol says anything at all about what a resource may record | design §12.5, P20 |

---

## D. Properties the protocol must preserve, derived rather than stated

These follow from decisions elsewhere and would be easy to break.

1. **A compromised node can already forge access** (design §12.4). The protocol
   should not widen that: it should not let a *resource* forge access, nor let a
   node's assertion for one subnet be presented in another.
2. ~~Notifying a hosted package that roles changed.~~ **RESOLVED — no mechanism
   needed.** The node **terminates** hosted sessions on role change
   (`infra-client-requirements.md` design §8.4); the principal reconnects and is
   re-evaluated. Termination avoids a notification protocol, an acknowledgement, a
   latency budget, and any definition of what happens to an in-flight request.
3. **The resource must not be able to enumerate the org.** It sees principals who
   connect; a protocol that let it ask *"who else could connect?"* would hand a
   package the neighbourhood map the network works to keep horizon-limited.
4. **Log data is the resource's, not the network's.** P20 identifies resource
   access logs as a privacy finding with no mitigation. Whatever the protocol
   carries becomes what a log can contain.

---

## E. Open questions the design has not asked

1. ~~Is the resource in the trust path or only behind it?~~ **RESOLVED — behind
   it, always.** A resource reports to its host and may message nodes peer-to-peer
   where configured; it asserts nothing the trust metric consumes (design
   design §8.0.3). The alternative would require global message types for arbitrary
   service needs, so the protocol would grow with the ecosystem. **This is why C1
   is plumbing:** the request path carries no trust.
2. ~~Does a hosted resource have a session with a user, or with the node?~~
   **RESOLVED for revocation purposes:** the node terminates hosted sessions on
   role change, so it need not reach into a live session. What remains is narrower
   — **whether the resource sees distinct principals or only the node** — which is
   a question about the credential's contents (C2) rather than about session
   ownership.
3. **What does a resource do when the node is unavailable?** A serving node with
   siblings fails over for *clients* (design §9.9.2); resources are hosted on one node and
   have no equivalent. A resource is as available as its host.
4. **Can a resource be moved between hosts** without changing identity? Related to
   C4 and to design §8.2's separation of ownership from hosting, and unaddressed.

---

## F. Non-requirements — settled as out of scope

- **What a credential means to an external consumer.** Policy belongs to them;
  which issuers they recognise is the IdP plugin developer's problem, not the
  network's (design §16, `resource-requirements.md` R9).
- **The resource's own internal protocol.** Beyond the gateway, a resource is a
  black box (design §8).
- **Group semantics.** Group operations are pairwise fanout in a client library
  (design §9.11); a resource wanting group behaviour builds it.

---

## Suggested order of decision

**C5 and C6 remain, and neither blocks anything.** C5 — whether an abuse report
relates to a live session — is answerable either way; it is a transaction
(`wire-format.md` type 7) and works standalone. C6 — what a resource may log — is
the resource's business, already registered as privacy finding P20, and the
protocol says nothing because it has nothing to say.

**Everything with design content in it is closed.**

Then C2, C1, C3, C4. C5 and C6 are small once the session model exists.
