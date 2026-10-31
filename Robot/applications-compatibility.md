# The core against the application suite's environment expectations

Assistant working file, 2026-10-30. What `../FuerosNet-applications` expects
of its environment — the host (node and daemon), the directive plane, and
the light client — set against what the core states and implements at
`f15489e`. The applications' documents are proposals and a consumer; where
they and a root document disagree on a protocol fact, the root document
wins, and nothing here changes that. What this file does is say, item by
item, whether the core **provides** what they build against,
**contradicts** it, or **has nothing** on it yet, and what follows.

The applications read the protocol at `0764307` (2026-10-27). Since then:
57 commits; the two seam sections changed by one paragraph
(2026-10-28, `resource-requirements.md` §3.3: a package's `rhtn-*` response
headers are dropped on the member path), and nothing else in that section or in
`infra-client-requirements.md` §9.2. Their citations still resolve.

## 1. The host surface — `rhtn/host@1` (`hosting-contract.md`)

The contract names five bindings a host provides a package, a version, a
manifest that declares tolerances and budgets, two runtimes and a key
scheme. The host is the node and its daemon, so every row below is the
core's to meet or to refuse.

| Expectation | Core today | State |
|---|---|---|
| **request/response**: the gateway's door and the `/.rhtn/` exchanges | the gateway; `rhtn/1:request` and `rhtn/1:response`, the sandbox's only two bindings (`resources/src/lib.rs`, `HOST_EXPORTS`); the three exchanges in `node/src/federation.rs` | **provided** |
| **storage block**: a per-instance durable region with `sync`, a byte quota, provision/destroy lifecycle; a WASM guest's block is a preopened directory | **built 2026-10-30** as the host's own binding `rhtn/1:block` (file-shaped: read, write, sync, size, list, remove under flat names), not a WASI preopen — a preopen cannot enforce a quota or say what `sync` means; provisioned per instance under the daemon's `state` directory at the manifest's `block_bytes`, enforced on every write; a socket-backed package is handed the path and the ceiling on the relationships exchange (`rhtn-block`, `rhtn-block-bytes`); destroy is an operator's act, not a reload's | **provided** (`infra-client-requirements.md` §9.3, PKG-017, RSC-63, DMN-35) |
| **jobs**: `schedule(at\|in, task, budget) → id`, `cancel`, `list`; durable across restart; under the instance's authority as of the configuration epoch; budgeted | **built 2026-10-31**: `rhtn/1:jobs` (schedule at a time with a budget, cancel, list), one file per job under `state/jobs/<hex>`, run on the daemon's tick through the package's `job` entry under the instance's authority, once, no retry, outcome in the log; bounded by `jobs` in flight and `job_seconds_per_day`; a socket-backed package has its own timers | **provided** (RSC-65) |
| **log**: a bounded, timestamped, per-instance operator-visible stream | **built**: `rhtn/1:log` in the sandbox, a per-instance file for a socket-backed package (`rhtn-log`, `rhtn-log-bytes`), host-stamped lines, bounded at `log_bytes`, oldest out first; on the administration surface as `GET /node/resource/<hex>/log`, with the host's budget and fetch refusals written into it | **provided** |
| **clock**: the host's time, advisory | **built**: `rhtn/1:clock`, milliseconds since the epoch | **provided** |
| **versioning**: `rhtn/host@N` offered by the host, tolerated by the manifest, refused at installation with both sides named; bindings only ever added | **built**: `HOST_VERSIONS = ["rhtn/host@1"]`; the manifest's `host` list; a package tolerating none is refused at installation naming both lists | **provided** |
| **quotas and budgets** in the manifest: block bytes, jobs in flight, log bytes retained, request rate, job compute per day; refusals named on the operator's surface | **built**: `[budget]` with all five; block bytes, log bytes, jobs in flight and job seconds a day enforced, refusals in the instance's log on the surface; requests a minute recorded and not enforced, deferred [author, 2026-10-31] on what a member is told | **provided, one budget deferred** |
| **runtime**: a socket-backed service first, as its own OS user; a WASM profile second; a parity fixture across both | both runtimes exist (`[[host]]` with `address` → a socket backend; with `component` → the sandbox); OS-user confinement is the operator's deployment; no parity fixture | **provided, the fixture absent** |
| **envelope key scheme**: Ed25519 in v1 under the node's 512-byte bound; instance public keys introduced in the manifest; proof of possession; rotation; per-instance custody | `MAX_SIGNATURE = 512` (consistent); **the instance's public key is written beside the resource in the hosting file** (`key`, 32 bytes hex, validated at installation) — per instance, which is where the contract's own custody rule puts it; the node carries the signature and does not verify it, by both documents' agreement; proof of possession and rotation wait on verification | **consistent; the key recorded** |
| **their section 9's absences** (no session-close notification, no cross-instance reads, no network access) | the same absences | **provided** |
| **their section 10's conformance**: golden byte shapes of the exchanges; block, quota, durability, atomicity, cancellation and parity fixtures | the exchanges are tested in `node/src/federation.rs` and `daemon/tests/federation.rs` (an admitted envelope survives a restart); the rest cannot exist before the bindings do | **partly** |

**The finding in one sentence, as of the audit**: the directive plane was
built; the host surface beneath the applications was not — four of five
bindings absent, and the manifest the contract describes refused by the
daemon. **As of the next morning**: the manifest is taken and all six
bindings are provided — the block, the log and the clock on both
runtimes, fetch and jobs on the sandbox — so `rhtn/host@1` is met in
full on the sandbox; a socket-backed package has its own network and
timers.

## 2. The directive plane (`directive-surface.md`)

Their page describes the plane as built at `c7c4e08`/`6d67035` and marks
what remains. Checked against the code:

| Item | State |
|---|---|
| the three exchanges, `rhtn-control: node`, the normalised-target guard, node-key ingress | provided |
| their section 5's record rules, custody per instance, `?taken=` acknowledgement, epochs, revocation, bounded passes | provided |
| the bounds table: record id 128, schema 64, body 64 KiB, signature 512, path 16, 4,096 records, 256 waiting, 256 pending, 32 attempts, kin name 32 | **matches the constants exactly** (`MAX_RECORD_ID … MAX_KIN_NAME`, plus `MAX_CONFLICTS = 64` and `MAX_ITEM = 64` they do not list) |
| outstanding per their page: application-side signature verification | the application's, by both documents |
| outstanding per their page: the crash-matrix fixtures (sender and receiver restart, lost acknowledgement) | one of three exists (`an_envelope_admitted_and_not_applied_survives_a_restart`); sender-restart and lost-acknowledgement fixtures do not |
| not on their page: a package's `rhtn-*` response headers are stripped on the member path [2026-10-28] | their gateway shim (1.1) should know; **tell them** |

## 3. What they expect of the light client (`client-utilities.md`)

| Expectation | Core today | State |
|---|---|---|
| **the utility request contract** (their section 6): a resource presents a typed utility request over its ordinary response channel; the kernel mediates; the user consents; the result returns as application data | **built 2026-10-31**: `light-client-requirements.md` §8.1 and `resource-requirements.md` §3.4 — one typed ask under `application/vnd.rhtn.utility+json`, read by the kernel, put to the person, answered by the kernel as one `POST` of multipart form data to the ask's return path; ten utilities; a per-device, per-audience application key, never the identity, with the claim array stated for their side to verify; the shell puts the ask to the person and gives a tag by the camera today, declining the rest as unsupported | **provided** (CAT-026, PKG-018, RSC-66); their section 6 should take the media type, the fields and the claim array as the core states them |
| **delivery** (their section 4): foreground polling of instances on the client's own sessions; a wake path that carries a hint and never content; nothing assumes the protocol's liveness signals carry resource notifications | hosted sessions with opaque IDs exist; the wake endpoint (design §14.1.5) is the node's mail doorbell and nothing says it carries a resource's hint | **consistent; nothing to build until a template needs it** |
| a per-device **application key** distinct from the network identity (their section 6's signing rows) | **built 2026-10-31** with the ask: expanded per audience from a device secret, never in the backup, so a restore mints a new one | **provided** |
| optical code exchange | exists on Android | provided |
| clock discipline as displayed uncertainty | nothing | theirs to build on the kernel's clock |

## 4. External services (`external-services.md`)

Their catalogue has the *node* fetching feeds (weather, CAP alerts, RSS,
social sources) on a package's behalf — "node fetch, cached per region".
The hosting contract's binding list is closed and has no outbound fetch;
the sandbox links no WASI sockets or `wasi:http`. **Ruled** [author,
2026-10-30]: a `fetch` binding, destinations declared in the manifest
(`https://host[:port]` exactly, no wildcard, never the node's own machine
or network), delivered with the package and replaced whole by an update.
**Built 2026-10-30**: `rhtn/1:fetch` — one HTTP/1.1 request in, one
response out, the one parser on both ends; the host sends only to a
declared destination over TLS under the operating system's roots,
bounds the response at a mebibyte, follows at most three redirects within
the list, and refuses a declared name that resolves to its own machine
or network (RSC-64). Their contract should gain the binding, and their
adapters' SSRF discipline is now the host's.

## 5. The bundles and federation documents

- **Tier 0** ("storage and compute are declared in the manifest and met by
  the hosting contract") — the declaration exists in words
  (`resource-requirements.md` §8, PKG-001); the meeting of it is section 1 above.
- **Execution targets**: "application storage/runtime service — durable
  database, blobs, timers, jobs — needs the hosting contract, not a current
  sandbox import". Correct about the core today.
- **Federation plumbing**, their section 6: transport is the node's HTTPS leg (built);
  "bodies pulled on acceptance" is deferred by their own surface page (v1
  inline). Consistent.
- Licence note: PostgreSQL's licence is not in `deny.toml`'s allow-list;
  `wasmtime-wasi` (Apache-2.0 WITH LLVM-exception) is, and is not yet a
  dependency.

## 6. What follows, in order

Done 2026-10-30/31 (`20e5fc8`, `eb0eb1f` and the commit after): the
manifest's fields and the version refusal; the block as a binding on both
runtimes; the log, on the administration surface with the host's
refusals in it; the clock; fetch to declared destinations; jobs; the
utility request contract and the application key;
`infra-client-requirements.md` §9.3 and the manifest text; PKG-017, RSC-63, RSC-64, RSC-65, DMN-35. Owed, in
order:

1. **The utility request's device half**: **built 2026-10-31** for nine
   of the ten — a photograph taken by the shell's own camera (`Still`)
   for `capture` and `scan`, a place as a geohash at the precision asked,
   a tag by the near-field radio or the camera for `tag` and `check-in`,
   the platform's unlock for `sign`, the document picker both ways for
   `import` and `export`, and the notification permission for `notify`.
   What remains: **`live`**, which wants a media service and an adapter
   the shell has none of; and **`scan`'s own shape**, since the shell
   photographs a document rather than deskewing, reading and assembling a
   PDF — a resource that will take only a PDF is told so. Their
   client-utilities page should know both. (A result is bounded at
   `wire-format.md` §11's 16 MiB per message since 2026-10-31, the
   request travelling as a sequence; the frame no longer bounds it.)
2. **Budgets not yet enforced**: requests a minute, and a bound on fetches
   a minute, which `infra-client-requirements.md` §9.3 does not yet name.
   **Both wait on one question** — what a member is told when the rate is
   spent — deferred by the author 2026-10-31 and written up in
   `Robot/deferred-decisions.md`.
3. **Jobs across a reload**: a reload re-provisions the table (the files
   stand); destroying a block or a table on an instance's removal is an
   operator's act not yet on the surface.
4. **Fixtures**: durability of a block across a kill; the sender-restart
   and lost-acknowledgement halves of the crash matrix; the parity
   fixture once a package runs on both runtimes; a live fetch against a
   real destination, which the gate has none of.
5. **For the applications' side**: the `resource-requirements.md` §3.3 paragraph of 2026-10-28
   (response `rhtn-*` headers dropped on the member path); the `fetch`
   binding added to their contract's list, in the request-in,
   response-out shape; the WASM profile's block as a binding rather than
   a preopen; `GET …/log` on the operator's surface.
