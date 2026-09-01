# Verifier selection — recomputation

**Draft. Spec-derived, unverified by an implementation.** See
[README.md](README.md). All inputs are raw byte concatenations hashed with
SHA-256 (`wire-format.md` §5) — no CBOR wrapping anywhere in this file
(INTERPRETATION 3, README).

## Nonce commitment (§5.1)

`commitment = SHA-256("rhtn/1:nonce-commit" || witness_keyhash || nonce)`,
nonce exactly 32 bytes. Test nonces are
`SHA-256("rhtn-test-vectors:nonce:<name>")`.

| Witness | Nonce | Commitment |
|---|---|---|
| w1 | `f271aecc154e4c40edb0f70c35cc00085552ccb052e9126b6917c41670185a2b` | `979536b6d2320ba25e1e20607d4cf07c76af6a84faeef471e8f5081c1650bc43` |
| w2 | `b6972f97b7363428c17878c5741a4f649e142c56c23b56511b101828cdb3dce8` | `c1a695e6772507de7b0aa4449c3a399609ca6535ff204dfabaf7a81ca16a4034` |
| w3 | `17cc9a63f97f221fdb50e1f5dd1450611cfa3525fb4c98f3be0a051999f7ec47` | `30ece26f6b719ac7fbff1b3e1d983a73f1f242554c9325ba9e8574641e350b81` |

## Seed (§5.3)

Ceremony: participants alice and carol, `started_at = 1767268800`, so
`window_ordinal = floor(1767268800 / 86400) = 20454`, encoded as 8
bytes big-endian: `0000000000004fe6`.

Participants canonicalise bytewise: min is
carol, max is
alice. Witnesses in ascending keyhash
order: w1, w3, w2.

Seed preimage (284 bytes = 20-byte tag + 32 + 32 + 8 + 3 × 64):

```
7268746e2f313a76657269666965722d736565642535e9a92cef24e674fe0b7d
b0cfd67762cf55953336bca8cc7db8ab05049ced435c987e0caa65d2c4edefb7
5db354b8678d3014fdce71596db0abb9b730e9a90000000000004fe669644d3e
698b7c54622f1b8045880d696bcd98d459981f696e8bc76c8d1c5d57f271aecc
154e4c40edb0f70c35cc00085552ccb052e9126b6917c41670185a2bb821af99
267d56d032bd36070b68336492a16797d3811752a8ee2edd8a40f37017cc9a63
f97f221fdb50e1f5dd1450611cfa3525fb4c98f3be0a051999f7ec47be9d9075
4088a297896519ccc1eed484d2fa0d255d47bc0d51fc9972c47b1b9cb6972f97
b7363428c17878c5741a4f649e142c56c23b56511b101828cdb3dce8
```

seed: `8ed92a138845b1d3a3d2ab80bf559f2a47c85cdd9766c617bfcc37751fd1d533`

## Threshold (§5.4)

`required(subject) = min(floor(n / 2), 10, |candidates|)`:

| n | candidates | required |
|---|---|---|
| 0 | 0 | 0 |
| 1 | 1 | 0 |
| 2 | 1 | 1 |
| 3 | 2 | 1 |
| 7 | 5 | 3 |
| 20 | 1 | 1 |
| 25 | 12 | 10 |

## Sampling (§5.4)

`rank(c) = SHA-256(seed || subject || c)`, subject alice, candidates c1–c5;
ascending rank, ties by ascending keyhash.

| Candidate | rank |
|---|---|
| c1 | `df80fc6e4357c3697a68f3ca363856f5e015054bf127651660ecfd4e6b811f59` |
| c2 | `f27dcf078f677b11debc3f1b7e0637a34a9bc3dc3227d37eb402cf75ced828c9` |
| c3 | `2a99b708bebabf3fc3ddfe154fb1311d0be0404ffe841b7eef01980f6f4c28dc` |
| c4 | `f221160e544a8730a558e0c2d249acbd3fa78cecf7b7e9c48a739f656e786f06` |
| c5 | `34e95887b0e7a44032114acd301f96764bc6e3f8fd9e41abbbe6c433570cf960` |

Rank order: c3 < c5 < c1 < c4 < c2.

With n = 7 and these five candidates, `required = 3`;
**selected: c3, c5, c1**. Exactly that many are queried (§5.4).

## Window boundaries (§5.3.1)

With `started_at = 1767268800`, the 730-day window is
`1767268800 − 63072000 = 1704196800 < finalized_at < 1767268800` —
open at the far end, closed at the near end:

| Prior record `finalized_at` | Counted? |
|---|---|
| 1704196800 (exactly 730 days) | no — boundary instant is out |
| 1704196801 | yes |
| 1767268799 | yes |
| 1767268800 | no — not before `started_at` |
