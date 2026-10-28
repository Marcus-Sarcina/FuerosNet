# Capability matrix: what a shell does, and what the kernel gives it

Working document, 2026-09-22. What a working note of 2026-09-21, since
pruned, asked for: each intended screen or action of the light client,
the kernel operation it calls (`crates/ffi/src/client.rs`, `Participant`),
the adaptor beneath it, what comes back, and the acceptance entry that
closes it. **Owed** marks a row the kernel does not yet carry. The row is
the contract: a shell that needs something not on it is asking for a
kernel change, not a bypass.

Every result is a value or a `Refused` carrying its reason; nothing above
the boundary parses a wire byte or keeps protocol state. Notices reach the
shell through the platform's `Notices` object as `Told`; events through
`next_event`.

## 1. Identity and lifecycle

| Screen or action | Kernel operation | Beneath it | Comes back | Entry |
|---|---|---|---|---|
| First start on the ceremony device | `Participant::start(seeds, known, platform)` | `Client::new`; state restored from `Storage` under `STATE`, refused if it does not open | a participant, or why not | DMN-10, DMN-13 |
| First start on a desktop or terminal | `start_delegated(key_material, transport_seed, delegations, known, platform)` | `Client::delegated`; the credential on the platform's clock | the same | DMN-13 |
| Phone provisions a second device | `presented_key` on the device; `delegate(key, not_before, count)` on the phone | `Client::delegate_run` | the run, carried by the shell's own channel | (no entry: the author's to add) |
| Second device's bundle | `bundle_to_sign` on the device; `sign_device_bundle` on the phone; `take_signed_bundle` on the device | `Client::bundle_to_sign`, `sign_device_bundle`, `take_signed_bundle`; published where attached | ok, or why not | PAY-20 (harness) |
| Suspend or background | `detach` then release; `save` at any point | state written; the session closed with its frame on the wire | ok | DMN-13 |
| Backup | `export_backup(secret)` | `Client::export` with the seeds this device holds | the envelope | ARC-23, ARC-28 |
| Restore onto a fresh install | `restore_backup(blob, secret)` | `Client::import` (scanned) then `install`; refused into a client with an archive | records, discarded | ARC-24, ARC-28 |
| Who am I | `me`, `holds_seed`, `signers` | | ids | |
| **Settings: retention** | `retention_years`, `set_retention_years` (whole years, refused during a ceremony) [2026-10-03] | | | |
| **Settings: verifier limits, seal cost** | **Owed.** `Config::default()` is fixed at start | | | |
| **Biometric engine** | **Owed.** `HashEngine` is fixed in `device.rs`; a platform `Matcher` object is the seam | | | CER-37, CER-38 |
| **Platform key storage for the seed** | **Owed by design decision**: the shell supplies the seed at every start; the seal stays in memory (CER-39 deferred) | | | CER-39 |

## 2. Connection

| Screen or action | Kernel operation | Beneath it | Comes back | Entry |
|---|---|---|---|---|
| Attach to the serving node | `attach(serving, addresses, population)` | serving node first, cached siblings where it is dark; bundle published, pool stocked, population swept | `Attached { serving, primary, queued }` | DMN-13, SES-07 (kernel test) |
| **The screen for it** [2026-10-27] | the same call, from `AttachActivity` behind Home's connection line | the nodes whose endpoint records the horizon holds (`reachable`), or a key and an address typed; the people held are the population swept | the status line, and the refusal's words | |
| Connection status | `status`; `Event::Connection` | the watcher fails over after three missed intervals | `Status` | SES-08 (kernel test) |
| Degraded banner | `Attached.primary == false`, `Status::Attached { primary: false }` | the sibling's ack mode | | `light-client-requirements.md` §4 |
| Maintenance | kernel's own clock every `MAINTAIN_EVERY`; `maintain` when granted background time | prekey rotation, pool refill, wanted bindings | ok, or why not | PAY-11 |
| Wake endpoint | `wake(Some(..))`, `wake(None)` | registration on the session | ok | DMN-13 |

## 3. Payload

| Screen or action | Kernel operation | Beneath it | Comes back | Entry |
|---|---|---|---|---|
| Send | `send(to, kind, bytes)` | a session per device of the recipient; the direct path where held, the relay otherwise | ok; `unsent` where nothing carried it | DMN-13, DMN-15, PAY-20 |
| Receive | `next_event` | decrypted on the client's thread | `Event::Payload` | DMN-13 |
| Direct-path status | `direct_to(peer)` | the courier's held paths; **there is no path setting** [author, 2026-09-27]: direct preferred, relay the fallback, nobody's choice | held or not | (PRD-01 withdrawn) |
| Late response, response copy | `next_event` | | `Event::Late`, `Event::ResponseCopy` | |

## 4. Catalog and resources

| Screen or action | Kernel operation | Beneath it | Comes back | Entry |
|---|---|---|---|---|
| Browse | `browse` then `catalog` | `Serving::catalog`, request type 5, each reply taken until done or truncated | `CatalogItem` with node, truncated, stale | CAT-* (client-level) |
| Refresh and staleness | `browse` again; `CatalogItem.stale`, `truncated` | | | |
| **Register a resource** | **Owed.** `CatalogService::register` exists on the node; no kernel call | | | |
| Access a resource | `request_resource`, built 2026-10-16; `CatalogActivity` asks one through the node that served its entry | request type 6 on the serving node's stream | `ResourceAnswer` with `wire-format.md` §11.1's six statuses | RSC-* |
| **Roles** | **Owed.** The role view exists on the node; no kernel call | | | |
| Unrecognised declaration | `Told::UnrecognisedDeclaration` | `surface_declaration` | | |

## 5. Ceremony

| Screen or action | Kernel operation | Beneath it | Comes back | Entry |
|---|---|---|---|---|
| Open | `begin`, `take_intent` | the ceremony's own conversation, carried by the shell | ceremony id | DMN-10, CER-* |
| Proximity | `proximity`, `take_channels` | the platform's channels | what was achieved | CER-* |
| Capture | `capture_key`, `capture` | the platform's camera; sealed under the counterparty's key | | CER-* |
| Verifiers | `select_verifiers`, `query_for`, `request`, `responses`, `gathered` | queries on request streams through the serving node | | VER-* |
| Consent, verifier role | `consent`, `take_query`, `take_grant`, `take_response` | refused by name on a device holding no seed | | VER-*, SUB-* |
| Witnesses | `nominees`, `witness_ask`, `take_witness_ask`, `witness_sign` | | | CER-* |
| Sign and finish | `back_pointers`, `propose`, `body`, `review_and_sign`, `sign_body`, `finalize` | the record carried up at once | txid | REC-*, DMN-10 |
| Adoption | `propose_adoption`, `take_adoption`, `position_in`, `anchors` | | | TOP-* |
| Neighbourhood | `places`, `reachable`, `resolvable`, `distance`, `holds`, `records`, `prune` | the horizon this client keeps | | TOP-43, HOR-* |
| Recovery, the verifier's side | `recovery_query(prior)`, `recognise(query, consent, prior)` [2026-10-27] | refused where this device's own records do not show it met the prior key | the query, and the hybrid response naming the prior key | REC-* |
| Recovery, the subject's side | `prior_key`, `take_recovery_response`, `recovery_block(patron)`, `seal_old_lines`, `present_chain(patron)` [2026-10-27] | | the block the adoption carries, the old head, the seals | REC-* |
| Entering a recovery | `Participant::recovering(seeds, prior_seeds, backup, known, platform)` [2026-10-27] | the prior identity derived here, its backup opened under it where the person brought one, and the rotation begun on the new key | a participant already succeeding another key | REC-* |
| | **The prior seed is required**, `wire-format.md` §4.1 admitting no variant of the `Recovery` block: both halves or nothing. **The backup is not**, `light-client-requirements.md` §6 having recovery restore standing and not history | | | |
| Departure | `relationships()` for what could be left; `depart(patron, reason)` [2026-10-27] | minted, signed, appended and posted in one act: no countersignature, no refusal from the patron | the departure's txid, or why not | |
| **What leaving costs** | **Owed.** `light-client-requirements.md` §6 wants the resources an ending revokes listed first, and that computation exists on neither side | | | PRD-02, UX-006 |
| Standing and evaluation | `standing(subject)`, `evaluate(candidates)` [2026-10-27] | the client's own horizon: its adoptions, the meetings it was present at, its patrons' determinations | a score, or one computation over a set with its joint figure | |

## 6. The two client modes

Before an instance is attached and after (`Robot/app-requirements-notes.md`
section 1): the operator's frame, delegation issuance, the provider
credential in the envelope. Issuance and the credential slot are on the
kernel (`delegate`, `export_backup`); **the mode itself is not read
anywhere**: a kernel query *does this identity run an instance* is owed,
and the operator's frame waits on the administration-channel decision
which is the author's.

## 7. What crosses inward

The platform's seven objects (`crates/ffi/src/device.rs`): proximity,
camera, clock, randomness, the person (`Operator`), notices, storage. A
biometric matcher is the eighth once a real engine exists.
