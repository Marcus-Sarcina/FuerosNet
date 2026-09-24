# Android shell

Kotlin over `rhtn-ffi`. What is here is the skeleton: a Gradle project
whose one screen starts the kernel over the binding and shows what it
presents. `tools/build-native.sh` cross-compiles `librhtn_ffi.so` for
`arm64-v8a` and `x86_64` and generates the Kotlin binding from a host
build of the same crate, both into directories the tree does not carry;
`./gradlew assembleDebug` then builds the APK. The binding is generated
without the `harness` feature, which is never in a shell's build, so the
load-time checksum probe wants exactly the symbols the packaged library
has. The seeds the skeleton mints on first launch stand in until the
ceremony exists; the application id `com.comptus.fueros` is provisional.

**What this shell owes**, beyond rendering the client's states:

- The proximity channels the device has, and no promotion of one that
  failed (`light-client-requirements.md` §1.3). Android exposes UWB and
  NFC to an application; a device lacking one reports it as unavailable
  rather than absent from the list.
- A guided camera capture whose frames cross the boundary as pixels, with
  whatever the platform's pipeline attached stripped at the boundary (§1.3).
- The privacy choices §5 requires the user be able to make, and the
  warnings §6 requires before anything irreversible.
- Encrypted backup under a key kept apart from the backup, and a plain
  statement of what losing the device costs (design §13.7.1).

The catalogue carries these as PRD-01 to PRD-05 and PRD-07 to PRD-09.
