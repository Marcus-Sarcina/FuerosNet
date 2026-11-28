# The mobile shells

The light client application is a phone application. The ceremony's
proximity channels are NFC and an optical exchange, and its capture is
a guided camera session (`light-client-requirements.md` §1.3); none of them
exists in a Rust process, so the participant-facing application is Kotlin
on Android and Swift on iOS over `rhtn-ffi`.

**These are not Cargo workspace members and cannot be.** They build with
Gradle and Xcode, in their own toolchains. `crates/check.sh` runs the
Android shell's unit tests on the JVM where an SDK and a JDK are present
(its step 4c) and builds no APK.

**They live here anyway, in the same tree as the library.** Eight of the
nine manual product entries in the acceptance catalogue are this
application's obligations, and an entry is marked implemented by a marker
in a file the catalogue's walk reaches. A shell in another repository could
never close one. **The walk reaches `.kt` and `.swift` already**
(`crates/acceptance/tools/catalogue.py`), so a marker written here counts
the day it is written.

**`android/` holds the shell; `ios/` holds a README.** The Android one is
the participant's application as it stands — the ceremony end to end, the
conversations, the contacts, the catalog and the operator's screens — and
the Swift one is not started. Two directories, one of them a plan.

**Licensing: Apache-2.0, per artifact and per build.** Every file written
here is Apache-2.0, as the workspace beneath is, and so is every build of the
shell: the payload's cryptography is the kernel's own (the split seal), and
no AGPL code is linked. Contributions here are granted under Apache-2.0, as
in the root repository.

**The engine a resource's page is framed in is Mozilla's GeckoView, under
MPL-2.0** [author, 2026-11-25; MPL-2.0 in a client, 2026-09-16]. It is
shipped as Mozilla released it, with no file of it changed, so its terms
reach nothing written here; its source is at
https://hg.mozilla.org/releases/mozilla-release. The system's own engine
could not be told to offer a page no peer connection, which is why this one
is carried (`app/src/main/kotlin/com/comptus/fueros/Engine.kt`).

**The boundary is `rhtn-ffi` and nothing else.** A shell that reaches past
it into `rhtn-client` puts protocol behaviour above the boundary, where no
other client would have it.

| Directory | Shell |
|---|---|
| `android/` | Kotlin, Gradle |
| `ios/` | Swift, Xcode |
