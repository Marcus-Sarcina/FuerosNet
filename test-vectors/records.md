# Standalone signed records (`wire-format.md` §7)

Generated against `wire-format.md` SHA-256 `d7591e5f3a431ba73b2e8c543eef43e47ea16d1c3159cd3a4bc2516bf31b587b` and `network-design.md` SHA-256 `0ed4d17e2c3cab09230169ebcb7be14e56cd9d318bff77d0e63276eed2ff5ccd` — the design wins on any disagreement, so a design-only semantic change also stales these vectors. Regenerate after any change to either.

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). Each §7 object is a standalone `COSE_Sign1` under its
own domain-separation tag (§1.1) — this file grows toward one known-answer
vector per signing context. Payload reading throughout: the deterministic CBOR
of the map of exactly the named fields (§1's fields-X–Y rule, which governs
all eight signed objects).

## Node endpoint record (§7.6) — complete, classical-only

Bob publishes one endpoint; `external_aad = "rhtn/1:endpoints"`. Fields 1–3
(57 bytes):

```
a30158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab14db812911e2
ab4be5940281a301440a0000010219fbff03191d0803820902
```

Protected header `{1: -8}` → `a10127`; no `kid` — field 1 names
the signer (§3.5).

`Sig_structure`:

```
846a5369676e61747572653143a10127507268746e2f313a656e64706f696e74
735839a30158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab14db81
2911e2ab4be5940281a301440a0000010219fbff03191d0803820902
```

Ed25519 signature by bob:

```
a513df69375698611a8aaab4241d1dc7a8b1537d15f59a0c1c282242cac11c05
0bfd7fbb7377db937cb21c36157b57c4d363e9561a02b300796f3153cc8ba70a
```

Complete `EndpointRecord` (131 bytes):

```
a40158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab14db812911e2
ab4be5940281a301440a0000010219fbff03191d0803820902048443a10127a0
f65840a513df69375698611a8aaab4241d1dc7a8b1537d15f59a0c1c282242ca
c11c050bfd7fbb7377db937cb21c36157b57c4d363e9561a02b300796f3153cc
8ba70a
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
- **Unsigned message encodings — no signature exists to generate**: the capture
  key grant (§7.3, transient end-to-end payload), the late-response wrapper
  (§7.4 — its embedded `VerifierResponse` is already signed; the wrapper adds
  no signature), resolution messages (§7.7.3), archive fetch (§7.9), and the
  session messages of §8. These get **encoding** vectors, not signature
  vectors.
