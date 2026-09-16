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

**The boundary is `rhtn-ffi` and nothing else.** A shell that reaches past
it into `rhtn-client` puts protocol behaviour above the boundary, where no
other client would have it.

| Directory | Shell |
|---|---|
| `android/` | Kotlin, Gradle |
| `ios/` | Swift, Xcode |
