# Test identities

Generated against `wire-format.md` `bf7f219ff183618d…`, `network-design.md` `5fd849a259c8df45…` and `light-client-requirements.md` `69c5ac5837304937…` (full hashes, producer and output hashes in `tools/spec-pins.json`). The design wins on any disagreement; a change to any pinned document stales these vectors.

**Draft. Spec-derived, unverified by an implementation.** Derivation rules and
status are in [README.md](README.md); regenerate with `tools/generate.py`.

Every identity is synthetic, deterministic, and **real for both components**:

- **Ed25519**: `seed = SHA-256("rhtn-test-vectors:<name>:ed25519-seed")`,
  public key per RFC 8032.
- **ML-DSA-65**: `xi = SHA-256("rhtn-test-vectors:<name>:ml-dsa-65-seed")`,
  keypair = **FIPS 204 `ML-DSA-65.KeyGen_internal(xi)`**. Signing is the
  deterministic variant with empty context (wire §2.2). The recipe is
  implementation-independent — at generation time a second, independent
  ML-DSA implementation reproduced the same public keys from the same seeds
  and verified the deterministic signatures.

Per `wire-format.md` §2.2, `KeyMaterial = [COSE_Key, COSE_Key]` in fixed order
classical-then-post-quantum, each key carrying exactly three labels, and
`keyhash = SHA-256(deterministic CBOR of KeyMaterial)` (§2).

The classical `COSE_Key` is `{1: 1, -1: 6, -2: x}`; deterministic map order
sorts by the bytewise order of the **encoded** keys (`0x01` < `0x20` < `0x21`),
so the entries appear as 1, −1, −2. The post-quantum `COSE_Key` is
`{1: 7, 3: -49, -1: pub}`, appearing as 1, 3, −1.

| Identity | Role in the vectors | Ed25519 public key | keyhash |
|---|---|---|---|
| alice | node / subject | `fafb7967ea0e2bc7d3b5023bdb9e0bc3ccc2366470048255c85f0b879313ac2a` | `8410def778a5de3a25991aba399716bc8eccfda9ad57d4ea8a0c8dcfc852aa6a` |
| bob | patron | `e10996fba9fb4d1158766179de2837c77be29531347d302a4413829e8bab3750` | `6bcf8a3e8899fc206bc603744414d58b01db857986d82f611b0794ac9c32c375` |
| carol | counterparty | `2d952f29a29261715f9b1aefce27620f0705a4a848ac772ca2bdeca416064cf0` | `71591ef14497c49bd95fc61e243ccc9e2d041f54d210e89ad6654f61554a2cf2` |
| w1 | witness | `a0855987914434fb5942870ba72be8d90d9aca2ff8dac412f8a90f54b149d5c0` | `efe7876868352e6f8cabae9ba591a3e5ca61cf64950d19eb40ab82120738587f` |
| w2 | witness | `642d6afb3e1a3bfa7d22a808b2f55398a86d2ae814dc213034eb863fa69420d3` | `5c5f9a52ddbd9c7cfd08d87c82183a5b730379cc55df76ab85a6e06b54bf5ab3` |
| w3 | witness | `8e5d5a88df88cf1a11ddcffd10fc02a6f04e096820ea950a4ba44d64a340a2a6` | `f7415dbb4ce281fcd37b40f25b258537f7fcccd439520b77d86077c76f9e39df` |
| c1 | verifier candidate | `2b00306803c7f23192ae6272729a088ef1d8d26748a1f9ecce506fe8a7b97c8e` | `96664caec817f958a439b6d326c45f5ab7bbb3671ebb447eca25a411b975e73a` |
| c2 | verifier candidate | `bcf539e57801c7ab8c6bd6d27580c7fadedf90455106bc733f01e7bf292325b4` | `a51010f95aeedaa925c96080e1a429dc407aea4b75d628c072509e85a50647a0` |
| c3 | verifier candidate | `5d59e2dbc3871d0b3f36b9615c168cf2f7b470ea5cdb6b2c15fdd59ed9b7c473` | `a4ef070f484a0facc0fbea53a7119d0b0aaf52071c1e892093f6bd71cb53ac8b` |
| c4 | verifier candidate | `3c38a9069d16508e52a661762537488502894f931844fdf186eee0df7cfeb600` | `4384a2cc10f2d505660a16118457e9aeddee94389abd9c4d0dffb610ef0342a2` |
| c5 | verifier candidate | `5b801d2f689d09a5ed820155bb97e827c6869ea589842dce0678e185ca7cb7ac` | `f2d688318409d89353636efb04014ae130f282e207d4d7f9146a3316125790ea` |

## Worked example: alice

Ed25519 seed (private key bytes):

```
10cca6af5565ee2be87cad39cdc59229063246c3d7880c11162471e16d4cf36f
```

ML-DSA-65 seed `xi`:

```
061d5a9dc34611a307e9d580b2955bfffde2e69575b315ff29fca466fbbef798
```

Classical `COSE_Key`, deterministic CBOR (40 bytes):

```
a301012006215820fafb7967ea0e2bc7d3b5023bdb9e0bc3ccc2366470048255
c85f0b879313ac2a
```

Byte-level reading: `a3` map(3) · `01 01` kty: OKP · `20 06` crv: Ed25519 ·
`21 58 20 …` x: 32-byte public key.

Post-quantum `COSE_Key` (1962 bytes): `a3` map(3) · `01 07`
kty: AKP · `03 38 30` alg: −49 · `20 59 07 a0 …` pub: the real 1,952-byte
ML-DSA-65 public key derived from `xi` above, printed in the appendix.

`KeyMaterial` is the two-element array `82` followed by both keys
(2003 bytes); its SHA-256 is the keyhash in the table.

## Appendix: alice's ML-DSA-65 `pub`

```
aa9bb5635ad554c4648aae966b844c1dde2305b9c520c3d540c5995934e54c6e
2d36901eaec79dac2b4eabd27a98076f63c2e9d22e090b8132b0c2095425cfde
82e37f476ca319223702e0080ede909f01b054fa1df90653727bd71f5536506e
383a970a3e7f96da21e8f46013a985699d65fd20c7b3c79b80d8eaa7c2d46c4e
388cc878ab15687040324b282bd34c5f210a94ee7cceb661366f249bf574b2f8
cb67cd04d84ba1ebee9b9785b564e7bc111971243c9f1509e0cff476b57953e6
9db223ef3ee4007ed57d19c99a3533b2f43e489ecf1e2695265d22457edceaa8
4f8b10dbdf59d9c525a5d8f199fb175cf6d827798803c23194b605ce79ec3a22
40f800abd0fd3bc2e352c436e10829b72cc9e3e004390b054fa99c8ad87cee4c
753afdc2a3d02bea9ace38216957ad470af7ff1de9d8f8b033a79502b3538634
97b8da75bab1fed365d494515d11285a350c4554a4246ab24bbb5dee22377a65
d3d2ef16b9e6ecd3ef6a8d5503aef5d70e07d85bdeb63bbcd5fd75ea841bd64c
4d2b8f7e37a3f68d6b9557ccbb53aa799fc9917b89e68da0b145c9887b528124
60a661d23d3e26101b885fa61b4d20503d12980f7024b8afaccdc606183153d0
db33f53610dd8d6ff3929fa353d6ae107c2ec6406dc891d8aa8a8d8e785d3ec5
76040c988a543bc69df0483681cf4154a9fb112c4748859b8ee23c52432c7f33
0b63a1e4f2fbc92cad3c02153d5164a1a4f578bab99a7d532fe24c7731379214
1d6137a7b6c05194abe306620af1b19b3d768ca8bf1ed551b136019fb7c1c6bd
08ea80880db95913dc2bc3e9863c640cf8cdb18df0e459edaa5ae5b152a0786c
c59df5b229ed0008406ddd43119135c66981e703fb79f74b70666d008451b888
3135e6504be58408fd56c5a1bac653c993d1853e07f98d1d551667fdda69df5a
14c8d1525b90a091bd805c58ee2befda79a7d3f277e05ef4d466ea486e7359ee
77ec12945f4f4871f16f472774d9f5978caebf330e2403cf69b60d2cb283bcfa
a0c4ae99ac9654e986e04f17eb43010d7237514db0df52e91d716ff4053cb690
416813ca456df871336c9cf781afd6e8ff19171eebf207c8952dabe8a97b86d6
3fce2e2a8231f99215a1b2eaa80b41343eb8858a829257da38e5b594345ca208
10116fb67e814de0ed561f8ef63b74a8286da98fe1d97458c55019ef6108a372
1fc7db246dfbe5e82c645f3e984bb90f9a1ec06379a1640c6eff71479c223651
f30332a372b03cb6483a32185a3c68766987ec1c5c10d24ac956c4490f9f1c0d
f9ea10c20cf5cf418759258089a4fa803e535b057cdc9cdd2f6f7b3fb82ea6c5
e003137ef62c6836579a010b1e5ceb8dd7755bffded52fe77c8ca87df80d3793
c83a45c39631c8b8d53dc7651f51bc227351fa8415aa4206584ea429b448ec5c
b24662fa0f82fc6db17b2e6d302dc9c2317faf73a9480e0ad7fd6453f8304c32
46b2d9346383b969a8669ef643a44b13137e1d2f0a7fa6d917488f306df733b2
0f059971fbda43e18bc07ac85395c487212fb090b61019f742bea170a67765c7
4356cc218c69dfc4bd95ffb98cae7a5cf531514d41b108e4c7fe1fb9b0d52d03
eaaad8f3dfa0bc052bae6c8c373163200f2bfd30f8b08696ebe0184ce5c1dfe3
41013f7c307762be9057fa16b4310da053118c7671c71324c7a0c2fde0f6d26b
a648b57799adbcba736bcc1699cffd76e674c2ccb5b1560c2bd8b36803451c18
a351ce1a31faf377bed390fb2f8f5bd32708e5f36cdb3b1db5515ff6923fe3e7
49cdeb9a7fd98d30e6a7ee42baa3ab496f1664fa76c542ccbdfb778f38230883
3618577dab1025fe9dbd37aa9f6b0b57d79e1a108c163fd44f4504ab85c555fd
8230aa8394399043786ec2c49d4c2747480c576e2047beb8f2dbc774f8c66f2c
55531b55469e66cbc318344ff439d9f2518361c6e83dc1ae63c5c7570f82bb53
aabf9f837707724b9adada74f24ad2f032621bd5d1441753796f6ace6645f1b8
098cc2291c0557d870df41d8b8d3f1274be277479b28d99a295bccb452e32986
d25cc98790ebd45658cf848cc0d764804219fe1dc032244d4e2c15f8e5ee50ab
da7084a9f7a432807a472996570d3a1ca8130a46bc96858d0135b9732ade0f90
958a8c23b96524e711039b27145b448fddd6212688ca4dfcabfb3977d301af9c
0cdad5f2a06c2f21afd1a572fa14e41e9e84d7c76704cf36347bdcda47b2adca
dbbe1925233afcd106b281d7093aebf6e60584a3a47830a87b8e2e2d3b1f27e7
5b01a0956d27a70a1bfd4b4fe21e49672d16471264a370dd0f52cc0dc313bc91
d7531fedaa2180cee8dd6d0afdaf6ea018b41c19275eb1be21c936785a9d9b04
5913fcb902af2c33d739a51c7108be95a5eda39fced4ad68a02bd0ad45c34a27
29b2fc9440ed6b5244873331c4b970a641caaa5b39bdd41e1be078bb28458341
7415284f6f82fd7ca1bd6276d1711f7d208c10ec6b5e2fe5032da922f213216d
a92bdfe38f86c718400aa228826032cc5971d95b2ad07c31d58221019cd41d9c
38dc2615662ae824471506d232b4bb8b489a19adae14feafcecae3e0bf3a2405
1e276133c6e7cb8a11a9181523532ab17610ae3002d62a3799173b17e17a7ee4
17b2b8366fcc9dfa07e747acd1cac06d6679eccd1469206405601e1d67d2b41c
a9dd97c529df897c5e8c751d8228d7bb74e88c3fbba662e531991fd95c135038
```

Other identities' keys follow the same derivation and are regenerable from the
script — or from any FIPS 204 implementation exposing seed-based keygen.
