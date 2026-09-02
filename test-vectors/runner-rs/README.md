# rhtn-conformance — the Rust corpus runner

An independent executable validator for `../corpus.json`
(format `rhtn-test-corpus/1`). Run:

```
cargo run --release            # expects ../corpus.json
cargo run --release -- <dir>   # a directory holding corpus.json
```

**Independence posture.** No code is shared with the Python generator or
harness. The crate carries its own byte-level deterministic-CBOR parser
(definite lengths, shortest integers, bytewise-sorted duplicate-free map
keys — validated on the received bytes, as the profile demands), re-derives
all 25 test identities from the seed recipe the fixtures state, and verifies
signatures through the Rust ecosystem's own implementations: Ed25519 via
`ed25519-dalek`, ML-DSA-65 via RustCrypto's `ml-dsa` (0.1.1,
`SigningKey::from_seed` = FIPS 204 KeyGen_internal). Every agreement is
therefore cross-language, and for the post-quantum half **cross-
implementation**: the fixtures' signatures were produced by dilithium-py and
verify here under RustCrypto.

**What it executes.** Every `bytes`-class corpus entry: accept entries must
parse canonically and pass the runner's own schema/semantic validators;
`cbor`-layer rejects must fail the parser; higher-layer rejects must parse
and then fail a validator wherever the runner implements that kind's rules.
Deeply verified: all twelve transaction envelopes (signer sets derived from
the body per type, both algorithms per signer, 36 entries on the normal
record), the embedded evidence layer (subject consents over raw query-ids,
classical verifier responses over the map-minus-field-9, the recovery
block's hybrid responses and `[prior, new, patron]` successor proof), all
four presentations (roots recomputed from exact received slices), and the
standalone signed records under their named signers with the wrong-signer
analogues required to fail. Trace/context/unit entries are structured, not
bytes, and are skipped by design.

**Findings log.** 2026-09-02: caught `B-ext-value-1024/1025` measuring
payload bytes where §1's ceiling bounds the **encoded slice** — a fixture
defect the Python harness did not check; both fixtures corrected and the
bound taught to both harnesses.

Honesty note: this runner shares an author with the suite, so it is
cross-language and cross-crypto-implementation validation, not the
independent-party reproduction that promotion ultimately wants. It is,
however, exactly the artifact such a party can start from.
