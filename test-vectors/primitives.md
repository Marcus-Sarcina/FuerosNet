# Primitives

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md).

## Deterministic CBOR atoms (`wire-format.md` §1)

Shortest-form unsigned integers at every width boundary:

| Value | Encoding |
|---|---|
| 0 | `00` |
| 23 | `17` |
| 24 | `1818` |
| 255 | `18ff` |
| 256 | `190100` |
| 65535 | `19ffff` |
| 65536 | `1a00010000` |
| 4294967295 | `1affffffff` |
| 4294967296 | `1b0000000100000000` |

A 2026 timestamp encodes in five bytes, as §2 claims:
`1767225600` (2026-01-01T00:00:00Z) → `1a6955b900`.

Map keys sort ascending (§1): `{1: 0, 2: 0, 10: 0}` →
`a3010002000a00`.

## seqno (§2, §2.3)

`[series, counter]`, two shortest-form uints — never a single integer.

| seqno | Encoding |
|---|---|
| `[5, 42]` | `8205182a` |
| `[5, 4294967295]` (sealed: counter at u32 max) | `82051affffffff` |

## Path (§2.1)

Nibbles pack high-first; the byte string is exactly `ceil(n/2)` bytes; on an
odd count the final low nibble is zero.

| Path | Encoding |
|---|---|
| nibbles `3,1,4,1,5` (odd) | `a201433141500205` |
| nibbles `3,1,4,1` (even) | `a2014231410204` |

Reading the odd case: `a2` map(2) · `01 43 314150` packed bytes (`50`: nibble 5
then the mandatory zero) · `02 05` length **in nibbles**.

## Locator (§2.3)

`{1: anchor keyhash, 2: path, 3: seqno}` — anchor is bob, path is the odd
example, seqno `[5, 42]` (50 bytes):

```
a30158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab14db812911e2
ab4be59402a201433141500205038205182a
```

## SignedLocator (§2.3) — a complete classical signature, end to end

The one fully computable signature vector in this draft: classical-only by
profile, so no ML-DSA slot.

Payload — deterministic CBOR of fields 1–2 (INTERPRETATION 1, README):

```
a2015820435c987e0caa65d2c4edefb75db354b8678d3014fdce71596db0abb9
b730e9a902a30158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab14
db812911e2ab4be59402a201433141500205038205182a
```

Protected header `{1: -8}` → `a10127` (no `kid`: the surrounding
structure names the signer, wire §3.5).

`Sig_structure` (`["Signature1", protected, external_aad = "rhtn/1:locator",
payload]`):

```
846a5369676e61747572653143a101274e7268746e2f313a6c6f6361746f7258
57a2015820435c987e0caa65d2c4edefb75db354b8678d3014fdce71596db0ab
b9b730e9a902a30158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab
14db812911e2ab4be59402a201433141500205038205182a
```

Ed25519 signature by alice over those bytes:

```
e6efbe52c205493d4a7eef6b5ae5aeb046a7a9a4d51f72c8370fb4e5c4a658d7
ddeaeef30c57de86215c3ebf4a682418989a401459fe8daa8a7ebae3a6058d06
```

Complete `SignedLocator` (161 bytes) — field 3 is the untagged
`COSE_Sign1` `[protected, {}, null, signature]` with detached payload:

```
a3015820435c987e0caa65d2c4edefb75db354b8678d3014fdce71596db0abb9
b730e9a902a30158205693d22ed3d6dd0cc82926601127d63226bb9ec918ab14
db812911e2ab4be59402a201433141500205038205182a038443a10127a0f658
40e6efbe52c205493d4a7eef6b5ae5aeb046a7a9a4d51f72c8370fb4e5c4a658
d7ddeaeef30c57de86215c3ebf4a682418989a401459fe8daa8a7ebae3a6058d
06
```

## Genesis back-pointer (§3.1)

`SHA-256(the signer's keyhash)` — over the raw 32 bytes (INTERPRETATION 2,
README).

| Signer | Genesis value |
|---|---|
| alice | `ce9e488f637a11ad3951f4ec71b9f9ddcfe6b4a7be2d4784f720c1327b2b8a17` |
| bob | `0fd3776b51f0d1b599db54c1153e9f10b2157e85a0a2b7edfb90a588ad4df3e3` |
| carol | `ad90dacfd29bde5b50a4f8844dac0c596c95e25330e317e40b44c2dafb6b9e20` |
