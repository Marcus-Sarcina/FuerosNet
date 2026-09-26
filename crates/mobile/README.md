# The mobile shells

The light client application is a phone application. The ceremony's
proximity channels are UWB, NFC and an optical exchange, and its capture is
a guided camera session (`light-client-requirements.md` §1.3); none of them
exists in a Rust process, so the participant-facing application is Kotlin
on Android and Swift on iOS over `rhtn-ffi`.

**These are not Cargo workspace members and cannot be.** They build with
Gradle and Xcode, and `crates/check.sh` does not run them. Their tests run in
their own toolchains, on their own runners.

**They live here anyway, in the same tree as the library.** Eight of the
nine manual product entries in the acceptance catalogue are this
application's obligations, and an entry is marked implemented by a marker
in a file the catalogue's walk reaches. A shell in another repository could
never close one. The walk is extended to `.kt` and `.swift` when the first
marker is written; until then these directories hold no code.

**Licensing: Apache-2.0 per artifact, AGPL per build that carries
libsignal** [author, 2026-09-26]. Every file written here is Apache-2.0, as
the workspace beneath is. A shell that links libsignal — the audited
implementation of the payload session's cryptography, AGPL-3.0 — is conveyed
as a whole under AGPL-3.0, because linking it requires that; a build without
it is Apache-2.0 throughout. **No file changes licence either way**, and a
release says which of the two it is. Contributions here are granted under
Apache-2.0, as in the root repository, which is what lets the dependency be
swapped later without anyone's permission.

**The boundary is `rhtn-ffi` and nothing else.** A shell that reaches past
it into `rhtn-client` puts protocol behaviour above the boundary, where no
other client would have it.

| Directory | Shell |
|---|---|
| `android/` | Kotlin, Gradle |
| `ios/` | Swift, Xcode |
