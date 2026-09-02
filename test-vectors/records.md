# Standalone signed records (`wire-format.md` §7)

Generated against `wire-format.md` `91cfc3ba6e031573…`, `network-design.md` `c3e5713f440d7116…` and `light-client-requirements.md` `bc0ebf1d1794b602…` (full hashes, producer and output hashes in `tools/spec-pins.json`). The design wins on any disagreement; a change to any pinned document stales these vectors.

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). Each **signed** §7 object is a standalone `COSE_Sign1`
under its own domain-separation tag (§1.1) — this file grows toward one
known-answer vector per signing context, and the unsigned §7 encodings are
listed apart at the end so nobody generates signatures the specification does
not define. Payload reading throughout: the deterministic CBOR
of the map of exactly the named fields (§1's fields-X–Y rule, which governs
all eight signed objects).

## Node endpoint record (§7.6) — complete, classical-only

Bob publishes one endpoint; `external_aad = "rhtn/1:endpoints"`. Fields 1–3
(57 bytes):

```
a30158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac
9c32c3750281a301440a0000010219fbff03191d0803820902
```

Protected header `{1: -8}` → `a10127`; no `kid` — field 1 names
the signer (§3.5).

`Sig_structure`:

```
846a5369676e61747572653143a10127507268746e2f313a656e64706f696e74
735839a30158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b
0794ac9c32c3750281a301440a0000010219fbff03191d0803820902
```

Ed25519 signature by bob:

```
90fdfcc4f7d8654621fe69dcf8bcc52441528908a5eed31c4a082d317a3568fa
91c6d7af763f4f0d9314795a3357ad15dfced75eef79191e6d4a70fb0bddb10c
```

Complete `EndpointRecord` (131 bytes):

```
a40158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac
9c32c3750281a301440a0000010219fbff03191d0803820902048443a10127a0
f6584090fdfcc4f7d8654621fe69dcf8bcc52441528908a5eed31c4a082d317a
3568fa91c6d7af763f4f0d9314795a3357ad15dfced75eef79191e6d4a70fb0b
ddb10c
```

## The equal-seqno conflict pair (§7.6, §7.7.3)

A second record by bob — **same seqno `[9, 2]`, different endpoint set**, its
signature equally valid (127 bytes):

```
a40158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac
9c32c3750281a20144c000020703191d0903820902048443a10127a0f65840eb
592e1d16a335c3366203311bef91333edd763cb7c6b124ea1a0023176eb38218
e9baccba9ef48d05987fc90936141ef7d60f7638a940a64d5aede63e52c408
```

Each record is individually well-formed; **holding both is the malformed
condition** — an equal `seqno` carrying different contents is a disagreement,
never a tie to break, and a reader MUST NOT prefer either (negative suite,
V6). A subject advances its own counter, so the pair can only mean equivocation
or a key in two hands.

## An `EndpointRecord` carrying an unknown extension — MUST ACCEPT (D8)

§1's coverage rule is global: the signed payload is the map of the named fields
**plus any unknown extension keys**. This record carries `99: h'c0ffee'` inside
the signed payload of the standalone `COSE_Sign1` path — the same property
D2/E10 prove for the envelope path. Mutate the extension value and the
signature fails (E13). Complete record (137 bytes):

```
a50158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac
9c32c3750281a301440a0000010219fbff03191d0803820902048443a10127a0
f65840828aa398c9725465853470ec0d908fdb7da2ecab61b9d84f1a03fe582c
810c0b86dc53260f6b906e1a35ebd1784612102ef0f6d624185a593d4e56ce80
2c0804186343c0ffee
```

## The §7 object model — signed contexts versus unsigned encodings

**Not every §7 object is signed, and the target list must not imply otherwise**
(third review). One known-answer signature per *signing context* remains the
goal — but only for objects that have one:

- **Signed, queued**: currency attestation (§7.1), anchor table entry (§7.2),
  subtree acknowledgement (§7.5), prekey bundle (§7.8) — plus, outside §7,
  the catalog entry (§6.1), the abuse report's embedded signature (§6.3), the
  successor statement (§4.1), and the verifier response and consent contexts
  (§4.5, §5.6).
- **Unsigned message encodings — no signature exists to generate**: the
  currency request and reply (§7.1), the capture key grant (§7.3, transient
  end-to-end payload), the late-response wrapper (§7.4 — its embedded
  `VerifierResponse` is already signed; the wrapper adds no signature),
  resolution messages (§7.7.3), archive fetch (§7.9), resource registration
  and its reply (§6.2), the catalog query and reply (§6.4), resource
  request/response (§11), and the session messages of §8. These get
  **encoding** vectors, not signature vectors. *This inventory is maintained
  by hand until the canonical corpus enumerates it mechanically from the
  wire-format schemas (eighth review) — a hand list can itself omit a family,
  and did: the currency messages were missing from it until then.*
