# Standalone signed records (`wire-format.md` §7)

Generated against `wire-format.md` SHA-256 `41ec82f4d40c916cd0a42cd316b4cbc226e2d07c7b2ddf667c854bcf86347077` and `network-design.md` SHA-256 `67f5d245694bcd47ad6755a66494ab629035d9517ddca2a10a8ad1e71b98c96d` — the design wins on any disagreement, so a design-only semantic change also stales these vectors. Regenerate after any change to either.

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

## The equal-seqno conflict pair (§7.6, §7.7.3)

A second record by bob — **same seqno `[9, 2]`, different endpoint set**, its
signature equally valid (127 bytes):

```
a40158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab14db812911e2
ab4be5940281a20144c000020703191d0903820902048443a10127a0f65840a9
238ad378c9b0ca567442b7450330555adda8e0ace3b5cbe6f225142afb53e2a6
a7455b6ee697f6926af3e06f73ba25107bb6122ca8f2a143a846b4966bec0b
```

Each record is individually well-formed; **holding both is the malformed
condition** — an equal `seqno` carrying different contents is a disagreement,
never a tie to break, and a reader MUST NOT prefer either (negative suite,
V6). A subject advances its own counter, so the pair can only mean equivocation
or a key in two hands.

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
