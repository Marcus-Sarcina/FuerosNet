# FuerosNet

FuerosNet is a reconfigurable-heirarchic trust and networking protocol built to leverage personal relationships and in-person interactions.

While FuerosNet adopts some patterns familiar from cryptocurrency and/or privacy network projects, it’s core social structure is self-organizing hierarchies of users who are known to one another through in-person interactions.


## Key Features

- The Proof-of-Presence ceremony generates a cryptographically attested record of a substantial in-person meeting between users. PoP replaces proof-of-work and similar mechanisms used to secure distributed systems by imposing it’s cost on the non-parallelizable resource of human presence and attention rather than on an economic resource such as electricity or financial assets.

- Trust and visibility in FuerosNet are user-centric, so the system does not require a global ledger or namespace. Users of FuerosNet are free to distrust or completely disregard distant portions of the network, providing a natural resistance to Sybil and similar attacks.

- Because FuerosNet does not require a token or other global ledger, it does not lend itself to financial attacks or pump-and-dump schemes. Instead of promising to make you rich, FuerosNet exists to facilitate communication and collaboration with people you know and trust.

- Message routing and access control flow along a self-organizing user heirarchy with social benefits described in [Section 3.3](/network-design.md#3.3-the-infrastructure-tier).

- Subnets within FuerosNet are able to smoothly merge or partition with the infrastructure and resources following their individual owners. Information and visibility resist concentration and therefor surveillance by untrusted parties.

- Infrastructure is user-owned with the increased status of a Patron node as the incentive to contribute. This means FuerosNet requires no central operator.

- Infrastructure nodes are able to host a wide variety of applications or other resources for use by Subs and directly adjacent nodes or function as a service gateway to outside systems, all of which are administered using a simple shared permission structure.

- FuerosNet is an open platform under Apache-2.0 licensing. Outside developers are encouraged to create applications, adapters, or alternate client software interoperable with this network.


## Overview

The system consists of three main components:

1) This repository contains the network protocol itself, establishing it’s high level design, trust model, transaction types, and wire format and implements the core libraries required to operate the protocol without any platform-specific implementation or user interface.

2) The Infrastructure-Node repo will be a reference implementation of the FuerosNet infra node along with a set of base resources and the resource interface for expansion with third-party applications.

3) Several Light-Client-Node repositories will contain reference implementations of the user-facing app for various desktop and mobile targets. (In FN jargon, this is called the “light client” in recognition that many users will not contribute infrastructure and will only operate as this type of node). Launching and administering infrastructure nodes will also primarily be done through an interface in the light client.


### Building from source

The Rust workspace is `crates/`; the root of the repository holds the design
documents it implements.

**Core libraries and tools.** Install [rustup](https://rustup.rs) (stable
toolchain, 1.98 or later) with the `rustfmt` and `clippy` components,
Python 3, and the two cargo tools the gate requires:

```
cargo install cargo-deny cargo-sweep --locked
```

Then run the code gate, which is the build:

```
cd crates && ./check.sh
```

It checks the specification pins, the acceptance catalogue and its
generated test stubs, formatting, lints with warnings denied, licences and
advisories, and runs every test in the workspace. A plain
`cargo build --workspace` or `cargo test --workspace` from `crates/` works
too. `test-vectors/tools/verify.py` checks the draft canonical vectors
against their pinned inputs.

**Optional gate steps.** With a JDK (17 or later), the Kotlin compiler
2.4.20 and `jna-5.17.0.jar` present, the gate also generates the Kotlin
binding and drives it against a real node; the tools are found on `PATH`
(or under `~/opt`, see `crates/tools/kotlin-roundtrip.sh`, with the jar
named by `RHTN_JNA_JAR`). With a nightly toolchain and `cargo-fuzz`, it
runs five short fuzz passes. Either set absent, those steps report
themselves skipped, never passed.

**The Android shell** (`crates/mobile/android/`) additionally needs the
Android SDK (platform 36, build-tools), NDK r30, `cmake`, and the Rust
cross-targets:

```
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk --locked
```

Point `ANDROID_HOME` (or `local.properties`) at the SDK, then from
`crates/mobile/android/`:

```
./tools/build-native.sh && ./gradlew assembleDebug
```

The first command cross-compiles `librhtn_ffi.so` and generates the Kotlin
binding; the second builds the APK. Continuous integration runs the gate
from `crates/.gitlab-ci.yml` on every push.

### Authors

A. Marcus Zuech is a small business owner and former PHB who wanted to see if he could actually vibe-code something genuinely ambitious.


## Feedback and Contributions

The [github repo](https://github.com/Marcus-Sarcina/FuerosNet) is a mirror of the official master hosted at [gitgud.io](https://gitgud.io/MZuech/FuerosNet) for visibility purposes(gitgud requires registration to browse public repos). Your comments are welcome in either location, but pull requests and contributor applications should be made to the gitgud.io instance.

You are also welcome to email marcus@sarcina.co with your comments, questions, unhinged verbal abuse, etc.
