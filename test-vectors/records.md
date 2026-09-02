# Standalone signed records (`wire-format.md` §7)

Generated against `wire-format.md` `9d7a11a8a173dc35…`, `network-design.md` `cff13d0301ad5121…` and `light-client-requirements.md` `8cc2ee2788f4f81d…` (full hashes, producer and output hashes in `tools/spec-pins.json`). The design wins on any disagreement; a change to any pinned document stales these vectors.

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

- **Signed — complete 2026-09-02.** Every domain-separation context now has a
  known-answer signature: locator and endpoints (below), currency, catalog,
  abuse, anchor, subtree-ack and prekey (the bar-8 section at the end of this
  file, each with its wrong-signer analogue), and successor, verifier and
  consent (the recovery adoption and normal record in `transactions.md`).
  The envelope tag is every transaction's.
- **Unsigned message encodings — landed 2026-09-02 in `messages.md`**
  (canonical bar 9): every family in this inventory has a positive
  known-answer encoding there — session frames, topology push and memo,
  resolution, archive, prekey, query-plus-consent, catalog, resource
  request/response, registration and reply, currency, the capture key grant
  and the late-response wrapper — plus the session-trace table. *The
  inventory remains hand-maintained until the corpus format enumerates it
  mechanically from the schemas (bar 6).*

## The remaining signed contexts (canonical bar 8)

One known-answer `COSE_Sign1` per domain-separation tag. Every payload is the
object's canonical map **without its signature slot**; protected header
`{1: -8}`, no kid (the object names its signer), empty unprotected, detached
payload. Each is followed by its **wrong-signer analogue** (canonical bar 10):
byte-identical fields, the signature cryptographically valid under a key the
object does **not** name — the binding, not the mathematics, is the defect
(S23's rule). And each signature is bound to its tag: verified under any other
context's `external_aad`, it MUST fail — the cross-context substitution family
(S24).

**Currency attestation** — subject alice, issuer bob (role 0, patron), ~10 h
expiry (194 bytes; wrong-signer: carol):

```
a70158208410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a0c8dcf
c852aa6a0258208410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a
0c8dcfc852aa6a031a6a431510041a6a43a1b005000658206bcf8a3e8899fc20
6bc603744414d58b01db857986d82f611b0794ac9c32c375078443a10127a0f6
58409a7cc8fe0a7d316bf1fc6723cfddc5634c58a0acb5925e87de1cdddc8d7e
193629de72340d330f430be315a0818066577703f5e27d901b158721ad17c4cd
0805
```

```
a70158208410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a0c8dcf
c852aa6a0258208410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a
0c8dcfc852aa6a031a6a431510041a6a43a1b005000658206bcf8a3e8899fc20
6bc603744414d58b01db857986d82f611b0794ac9c32c375078443a10127a0f6
58405dcb3b26b8c637d2de08b6433d3dc2cf42689286f15a005d99ab1d93f2c2
7e4cd59d3037829964ce275fd3f295324a8514044a4dd08d15543b09c694f6f4
b609
```

**Catalog entry** — resource c5, owner bob, `connect_scope` absent (no
prediction offered), `data_practice` 1; the endpoint is the SRV analogue
(209 bytes; wrong-signer: carol):

```
a8015820f2d688318409d89353636efb04014ae130f282e207d4d7f9146a3316
125790ea0258206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b
0794ac9c32c375036a7268746e2d666f72756d04705468652052656164696e67
20526f6f6d055818717569633a2f2f3139382e35312e3130302e373a34343333
0743763d31088443a10127a0f658409ada6d6bdd717805798ab6196b547556d6
0ed50537b5d9838cd41587c102972ad8f49f945ade2c77affea411768be0beda
6680eca05c6ce6c7d1de49639c1d050901
```

```
a8015820f2d688318409d89353636efb04014ae130f282e207d4d7f9146a3316
125790ea0258206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b
0794ac9c32c375036a7268746e2d666f72756d04705468652052656164696e67
20526f6f6d055818717569633a2f2f3139382e35312e3130302e373a34343333
0743763d31088443a10127a0f658409a439f4990d90a9a2bfabf78f5b02a48da
68d8a6f155d843a17bbe20323c9b1b2508ddde29b3d5ec010869e9dd4be3487c
084f8def7e484160572b491bbba6000901
```

**Abuse report** — the resource c5 reports excessive load to its own owner;
field 1 is both the resource and the signer (145 bytes;
wrong-signer: carol, violating the field-1-equals-signer binding):

```
a5015820f2d688318409d89353636efb04014ae130f282e207d4d7f9146a3316
125790ea021a6a43232003020458186275727374206f6620396b207265717565
7374732f6d696e058443a10127a0f658409183c86b515264c1f827b41fe3ed21
142bfdb73ec8200684cb25adb9741c635619c3fec7c9c3c681b6dd7e718e846b
5f168380f4b2cb1234ba55d5d0a0d63608
```

```
a5015820f2d688318409d89353636efb04014ae130f282e207d4d7f9146a3316
125790ea021a6a43232003020458186275727374206f6620396b207265717565
7374732f6d696e058443a10127a0f65840b8dd73e3819c805d6d0284d34658dc
af9e2ecb14d65fdd9a57ef510e23b0f3e534fc345c1f1390df81cef5018c783d
c377ddb944acc0e6d907d484661f884c01
```

**Anchor table entry** — bob, one `NetworkPoint` with the port **absent**
(default 7431; writing it out is malformed, §1's default-omission rule),
subtree size 111 (130 bytes; wrong-signer: carol):

```
a50158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac
9c32c3750281a20144c63364070219fbf403186f04820107058443a10127a0f6
5840b3c1c58de2a0b2544f9152718b2d7c40f2c773cb973f33a7d788b2e98fa3
238fd6debb2236acefa97ba3d1e57479843d805646f1c0e3d9fb764ce8b73f86
4701
```

```
a50158206bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac
9c32c3750281a20144c63364070219fbf403186f04820107058443a10127a0f6
5840b3f985f3f2ce2f1bc03cf8006dadd1b4b162ba70a7e03ad529fd78438638
38f90aa08d8a42c60bed2ac519d86a70265048399652466f05cb63eb04b5d594
e106
```

**Subtree acknowledgement** — carol as grandpatron acknowledges alice's
adoption by bob (the tree above bob is asserted for the fixture, not built)
(186 bytes; wrong-signer: bob, who is the patron and exactly the party
that must not substitute for the grandpatron):

```
a501582097e88a435e646363020b799ad58ba79d0d92f5f938114db98ca4ce2b
7424e55f02582071591ef14497c49bd95fc61e243ccc9e2d041f54d210e89ad6
654f61554a2cf20358208410def778a5de3a25991aba399716bc8eccfda9ad57
d4ea8a0c8dcfc852aa6a041a69d99f68058443a10127a0f6584086ca366b3360
81472517c42096af103431f3f14c0a099d78a4ae789a3e84e1d4a4e4de79cedf
1f1e8bc20170d4271cbac90633e4f78e5b8d4fc4b407253bda04
```

```
a501582097e88a435e646363020b799ad58ba79d0d92f5f938114db98ca4ce2b
7424e55f02582071591ef14497c49bd95fc61e243ccc9e2d041f54d210e89ad6
654f61554a2cf20358208410def778a5de3a25991aba399716bc8eccfda9ad57
d4ea8a0c8dcfc852aa6a041a69d99f68058443a10127a0f65840e82b52f6f79f
8085d528a57d417610a838c3e4d2860d615c8d16399b43b883e3236819a87a82
d614ec4aa67011dd08b2eb01a9b23bf26d85e9638ea78c60da06
```

**Prekey bundle** — subject alice, construction 1 (PQXDH), 64 bytes of opaque
reusable material (185 bytes; wrong-signer: carol):

```
a50158208410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a0c8dcf
c852aa6a0201035840c5b654905fcee6436b8d3d1ba96ba61a02948f1cf18e89
a9b45fa174cb970800fb66e27d796ea27e4642b735e251341f9bf8e7de7fc8e4
7b35cb59e3a8c31ad9041a6a431510058443a10127a0f65840b41656e418fd1c
b558477031bfc44dbf9d427cd613bae2e8c4ba49db883b67629cfc4bb54e744a
4a76084fcf7c0010a7ae7550069a3fc3760151ad6c58e91102
```

```
a50158208410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a0c8dcf
c852aa6a0201035840c5b654905fcee6436b8d3d1ba96ba61a02948f1cf18e89
a9b45fa174cb970800fb66e27d796ea27e4642b735e251341f9bf8e7de7fc8e4
7b35cb59e3a8c31ad9041a6a431510058443a10127a0f6584003861cefc0c6c0
ddff2b622dbb4f10ce4c2c9d97ef1cd65c97b57428c051fd402743c8576cf277
4f94ae4c10a6f8de4919a53f6255388dbe4863d4443b8d5006
```

## Ceremony pre-commitment construction (design §7.5.2) — known answer

Contributory: SHA-256 of the ASCII tag `rhtn/1:ceremony` followed by each
participant's 16 random bytes, in ascending participant-keyhash order
(here bob then alice).

contributions (alice, bob):

```
57c54784788abe36a304be6d2eff43d4
d43a0b07379cf934c8b7e4b54629f95c
```

pre-commitment:

```
cb8ea88ad0a089017394c291f918217c4dc8d754a4639eb024f23662e5ca2b18
```

*(The presence-record fixtures predate this construction and carry arbitrary
32-byte pre-commitments; construction is unobservable from a record, so they
remain valid inputs.)*

## Capture-key derivation (design §7.5.2) — known answer

HKDF-SHA-256, salt empty, IKM the seed, info the ASCII tag `rhtn/1:capture`
followed by the raw subject keyhash, holder keyhash and ceremony
pre-commitment, output 32 bytes. Subject alice, holder c1, ceremony the
**prior alice–c1 meeting's** contributory pre-commitment (`transactions.md`) —
the ceremony that sealed the capture, never the one under assembly.

seed:

```
298a1bd33f525f98915dd373e76cfe00badeb753751b7a11622284adc63e466d
```

info (14-byte tag + 3 × 32 bytes):

```
7268746e2f313a636170747572658410def778a5de3a25991aba399716bc8ecc
fda9ad57d4ea8a0c8dcfc852aa6a96664caec817f958a439b6d326c45f5ab7bb
b3671ebb447eca25a411b975e73aab9c3a6457a92a1ab0738104359b0712fad9
e4df0f53f5a29829855d4d1d58f5
```

k_capture:

```
6157379db20e9b35da24fbab9ab4c8bc8676dc8f8c22c89d2b1fe2fea7b2985c
```

The `KeyGrant` in `messages.md` carries exactly this key, bound to the normal
record's txid and its first worked query.

## Pairwise principal (design §11.0.2) — known answer

`SHA-256("rhtn/1:pairwise" || resource_keyhash || user_keyhash)` — computed by
whichever node currently hosts the resource, so two node implementations MUST
agree byte-for-byte or a provider migration renames every user the resource
knows. Resource c1, user alice (keyhashes in `keys.md`):

principal_id:

```
0cb4d9074d2c56b823785bbfef3e8134717f52abbd1aa5fe22178a0b0d6959b0
```

