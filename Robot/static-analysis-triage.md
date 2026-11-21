# Static analysis: the first Opengrep triage

**Every finding of the first Opengrep run, one row each, with a verdict**
[author, 2026-11-20]. The project implements a novel network architecture
and knowingly accepts trade-offs a generic rule reads as a bug, so the
returns are examined by hand before the tool is allowed to stop anything.
`crates/check.sh` step 3e runs the same configuration on every gate and
prints the count with a pointer here; **it becomes halting once the author
has reviewed this triage**, and a finding this file does not account for
is then a failure.

Three verdicts:

- **real defect**: the rule found something that should change.
- **accepted design trade-off**: the behaviour is intended and a root
  document accepts it; the row cites the section.
- **noise/false positive**: the rule misread the code, or its concern does
  not reach this code (test-only, dev-only tooling, or already mitigated);
  the row says which.

No code was changed on the strength of this triage; the real defects are
the author's to schedule.

---

## What was run

| | |
|---|---|
| Tool | Opengrep 2.0.0 (`opengrep_linux_x86`, released 2026-11-13), LGPL-2.1. sha256 `37e93f4da5fd04a18c5c1febbdd12f7e98dfccd4d0c9cf2593bcb25eeb462da0`, equal to the release's published digest; its sigstore signature verified against the release certificate's key (`openssl dgst -verify`), certificate identity `https://github.com/opengrep/opengrep/.github/workflows/rolling-release.yml@refs/heads/main`. The certificate's chain to Fulcio and the Rekor entry were not checked: no cosign on this machine |
| Rules | `opengrep/opengrep-rules` at `f1d2b562b414783763fd02a6ed2736eaed622efa` (2025-01-26, the head of `main` on 2026-11-20), as the codeload tarball, sha256 `9a5f1cd5c625418cc1c776120123e2d4371df9bb66e099426b17c3488e13619d`, unpacked at `~/opt/opengrep/rules-f1d2b56`. Licence: LGPL-2.1 with the Commons Clause, which withholds only the right to *sell* the rules (a product or service whose value derives substantially from them); running them over this tree in its own gate is not that |
| Rule set | the rules for the languages the tree is written in: `rust/`, `kotlin/`, `python/` and `bash/` over the tree, `dockerfile/` over the two Containerfiles: 442 rules (10, 14, 375, 7 and 36), counted as `- id:` lines in those directories' YAML |
| Targets | the repository from its root, minus `crates/opengrep.ignore`: `test-vectors/`, build output, the generated Kotlin binding, and the two symlinks into `../FuerosNet-output`. 525 of the 528 tracked `.rs`, `.kt`, `.kts`, `.py` and `.sh` files are targets; the three that are not are `test-vectors/tools/*.py` |
| Two passes | Opengrep recognises a Dockerfile by its name and not a Containerfile, so the first run applied the Dockerfile rules to nothing and said nothing about it. The second pass names `crates/packaging/Containerfile` and `Containerfile.testapp` with `--scan-unknown-extensions` |
| Tests included | Opengrep's built-in ignore list, used where no ignore file is named, skipped 212 of the 522 tracked code files outside `test-vectors/`: the crates' `tests/` directories and the Android shell's `src/test/`. `crates/opengrep.ignore` replaces that list, so the integration tests and the Android unit tests are scanned |

The command, from the repository root, is in `crates/check.sh` step 3e.

## Totals

**130 findings: 2 real defect, 1 accepted design trade-off, 127
noise/false positive.**

| Rule | Findings | Verdict |
|---|---:|---|
| `bash.lang.correctness.unquoted-command-substitution-in-command` | 2 | noise/false positive |
| `bash.lang.correctness.unquoted-variable-expansion-in-command` | 7 | noise/false positive |
| `dockerfile.audit.dockerfile-source-not-pinned` | 2 | real defect |
| `dockerfile.best-practice.missing-image-version` | 2 | noise/false positive |
| `dockerfile.best-practice.remove-package-cache` | 3 | noise/false positive |
| `kotlin.lang.security.gcm-detection` | 1 | noise/false positive |
| `kotlin.lang.security.unencrypted-socket` | 1 | accepted design trade-off |
| `kotlin.lang.security.unencrypted-socket` | 7 | noise/false positive |
| `python.lang.best-practice.arbitrary-sleep` | 2 | noise/false positive |
| `python.lang.best-practice.unspecified-open-encoding` | 13 | noise/false positive |
| `python.lang.correctness.common-mistakes.string-concat-in-list` | 1 | noise/false positive |
| `python.lang.maintainability.return-not-in-function` | 2 | noise/false positive |
| `python.lang.security.audit.dangerous-subprocess-use-audit` | 1 | noise/false positive |
| `python.lang.security.audit.dynamic-urllib-use-detected` | 1 | noise/false positive |
| `python.lang.security.audit.insecure-file-permissions` | 1 | noise/false positive |
| `python.lang.security.audit.regex_dos` | 4 | noise/false positive |
| `rust.lang.security.args` | 8 | noise/false positive |
| `rust.lang.security.temp-dir` | 68 | noise/false positive |
| `rust.lang.security.unsafe-usage` | 4 | noise/false positive |

## The real defects

Rows 1 and 2, one defect in two files: **both Containerfiles build `FROM`
mutable tags**. `crates/packaging/Containerfile:19` (`FROM rust:${RUST}`,
`RUST=1.98-bookworm`) and `crates/packaging/Containerfile.testapp:23`
(the same), and in both the runtime stage's `FROM ${RUNTIME}`
(`debian:bookworm-slim`), which the rule cannot see through the `ARG` and
so reports only as rows 13 and 14's false "no version". One commit can
therefore build different image bytes on different days.
`infra-client-requirements.md` §9.1 takes digest-not-tag as the stance for
what a host installs; whether the base images follow it is open, and the
cost runs the other way too: a digest pin stops a rebuild picking up
Debian's security fixes until somebody bumps it. **Fixed 2026-11-20**: the
author ruled to pin ("front-load variability in the test and release
process"); both Containerfiles now name each base by tag and digest.

## The accepted trade-off

Row 3: the desktop-pairing bearer in `DesktopActivity.kt` is a plain TCP
socket on the local network. `wire-format.md` §14.3 makes the bearer the
shell's choice and puts integrity in the optical anchor rather than the
bearer; `wire-format.md` §14.3.3 has everything after the desktop's
code cross a session that code keys.

## One row for the author's eye

Row 20: `DiagStream.kt` streams diagnostics in plaintext to the bench over
the tester's LAN. Marked noise because it exists only in the fieldtest
flavour and the releasable flavour never constructs it, but no root
section accepts it: diagnostics are tooling outside the specification, so
it is not a design trade-off in this file's sense. **Ruled 2026-11-20**:
acceptable in the fieldtest build only; a finding of it reaching the
releasable flavour would be a real defect.

## What the run did not see

Nine files were not wholly parsed, so a rule can miss what is in the
unparsed spans. Opengrep's own report, by file:

| File | Unparsed |
|---|---|
| `crates/tools/field-run.sh` | the whole file (a syntax error at line 1) |
| `crates/tools/sweep.sh` | lines 51 to 193 |
| `crates/packaging/package-runner.sh` | lines 180 to 192, 201, 247 to 305, 311 to 348, 356, 381 to 384 |
| `crates/mobile/android/app/src/main/kotlin/com/comptus/fueros/BleBearer.kt` | lines 178 to 179 and 819 to 831 |
| `crates/mobile/android/gradlew` | lines 74 and 178 |
| `crates/tools/nodeless.sh` | lines 89 and 130 |
| `crates/sim/src/daemons.rs` | lines 424 and 426 |
| `crates/daemon/tests/lifecycle.rs` | lines 132 and 136 |
| `crates/packaging/Containerfile` | reported at line 22 by the second pass, whose results for the file are complete: the same four as a copy named `Dockerfile` gave |

Five of the nine are shell, where `case` patterns and long compound
commands are what the parser gives up on: 116 of
`package-runner.sh`'s 430 lines go unread, and that is the script a
node's runner key is bound to.

## Since the first run [2026-11-21]

**123 findings now, against 130, and 9 files not wholly parsed, the same
nine.** Every difference is a change to the code, none a new kind of
finding, and none is a defect:

| Change | Rule | Where | Verdict |
|---|---|---|---|
| −8 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs`, rows 109 to 116 | gone with the file: the split seal deleted it (`5c91ac52`) |
| −1 | `rust.lang.security.temp-dir` | `crates/conformance/tests/september15.rs`, row 73 | gone: the same commit took the temp-dir fixture out of the test |
| +1 | `rust.lang.security.temp-dir` | `crates/node/tests/node_keys.rs:23` | **noise/false positive**: an integration-test fixture directory, as rows 109 to 116 were |
| +1 | `rust.lang.security.temp-dir` | `crates/node/tests/profiles.rs:147` | **noise/false positive**: a second fixture directory beside row 117's |
| 0 | `dockerfile.audit.dockerfile-source-not-pinned` | rows 1 and 2, now `Containerfile:24` and `Containerfile.testapp:28` | **now noise/false positive**: both bases are pinned by digest since 2026-11-20, inside `ARG RUST`, which the rule cannot see through, as it cannot for rows 13 and 14 |

Four more appeared in passing and were fixed rather than triaged: two
`open()` calls without an encoding in `crates/brand/icon.py`, and the
gate's own `$OG_LIMITS` expanded unquoted, now an array.

**The count does not depend on the machine's load.** Two runs of the
gate's own command gave the same 116 and 8 over the tree and the gate
the same 7 and 1 over the Containerfiles; the earlier differences
between gate runs (123, 125, 127) were each a commit's worth of code. The
gate nonetheless gives a rule 300 s per file and skips no file for slow
rules (`--timeout 300 --timeout-threshold 0`), and names any timeout on
its summary line [author, 2026-11-21], so that a loaded machine cannot
make one silently.

## Every finding

Line numbers are as of the commit that added this file.

| # | Rule | Where | What it flags | Verdict | Why |
|---|---|---|---|---|---|
| 1 | `dockerfile.audit.dockerfile-source-not-pinned` | `crates/packaging/Containerfile:19` | `FROM` by tag, not by digest | **real defect** | the node image a launch pulls is built from mutable tags (`rust:1.98-bookworm` here, and the runtime stage's `debian:bookworm-slim` the rule cannot see through `${RUNTIME}`), so one commit can build different bytes. The design's own stance for what a host installs is digest, never tag (`infra-client-requirements.md` §9.1); whether the base images follow it, at the cost of bumping the pin to take Debian's fixes, is the author's call |
| 2 | `dockerfile.audit.dockerfile-source-not-pinned` | `crates/packaging/Containerfile.testapp:23` | `FROM` by tag, not by digest | **real defect** | the test package's image is built from mutable tags (`rust:1.98-bookworm` here, and the runtime stage's `debian:bookworm-slim` the rule cannot see through `${RUNTIME}`), so one commit can build different bytes. The design's own stance for what a host installs is digest, never tag (`infra-client-requirements.md` §9.1); whether the base images follow it, at the cost of bumping the pin to take Debian's fixes, is the author's call |
| 3 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/main/kotlin/com/comptus/fueros/DesktopActivity.kt:180` | plain `Socket`/`ServerSocket`, not TLS | **accepted design trade-off** | the desktop-pairing bearer. `wire-format.md` §14.3: the bearer is the shell's choice and its integrity comes from the optical anchor, not the bearer; `wire-format.md` §14.3.3: everything after the desktop's code crosses a session that code keys, sealed by the kernel's `DeviceLink` |
| 4 | `bash.lang.correctness.unquoted-command-substitution-in-command` | `crates/check.sh:186` | unquoted `$(...)` | **noise/false positive** | false positive: inside `$(( ))` arithmetic, where no word splitting happens; the file holds one integer |
| 5 | `bash.lang.correctness.unquoted-command-substitution-in-command` | `crates/check.sh:215` | unquoted `$(...)` | **noise/false positive** | false positive: inside `$(( ))` arithmetic, where no word splitting happens; the file holds one integer |
| 6 | `bash.lang.correctness.unquoted-variable-expansion-in-command` | `crates/mobile/android/tools/build-native.sh:41` | unquoted variable expansion | **noise/false positive** | intended split: `$FEATURES` is zero or more cargo flags the script's own `case` sets (SC2086 disabled there) |
| 7 | `bash.lang.correctness.unquoted-variable-expansion-in-command` | `crates/packaging/package-runner.sh:133` | unquoted variable expansion | **noise/false positive** | intended split of the client's request into words: `set -f` stops globbing, shell punctuation is refused before it and every argument is checked against `[A-Za-z0-9./:=,_@-]` after (SC2086 disabled there) |
| 8 | `bash.lang.correctness.unquoted-variable-expansion-in-command` | `crates/packaging/package-runner.sh:230` | unquoted variable expansion | **noise/false positive** | intended: `${BLOCK_VOLUME:+-v "$BLOCK_VOLUME"}` expands to nothing or to two words (SC2086 disabled there) |
| 9 | `bash.lang.correctness.unquoted-variable-expansion-in-command` | `crates/packaging/testapp.sh:112` | unquoted variable expansion | **noise/false positive** | intended split: `${RHTN_SSH:-}` is the operator's own ssh options |
| 10 | `bash.lang.correctness.unquoted-variable-expansion-in-command` | `models/run-all.sh:185` | unquoted variable expansion | **noise/false positive** | intended split: `$TAMARIN_RTS` is `+RTS -N8 -M64g -RTS`, four arguments |
| 11 | `bash.lang.correctness.unquoted-variable-expansion-in-command` | `models/run-all.sh:238` | unquoted variable expansion | **noise/false positive** | intended split: `$TAMARIN_RTS` is `+RTS -N8 -M64g -RTS`, four arguments |
| 12 | `bash.lang.correctness.unquoted-variable-expansion-in-command` | `models/run-all.sh:349` | unquoted variable expansion | **noise/false positive** | intended split: `$TAMARIN_RTS` is `+RTS -N8 -M64g -RTS`, four arguments |
| 13 | `dockerfile.best-practice.missing-image-version` | `crates/packaging/Containerfile:61` | `FROM` with no version | **noise/false positive** | false positive: `${RUNTIME}` is `ARG RUNTIME=debian:bookworm-slim`, a tag the rule cannot see through (it is not a digest: see the source-not-pinned row for the same file) |
| 14 | `dockerfile.best-practice.missing-image-version` | `crates/packaging/Containerfile.testapp:35` | `FROM` with no version | **noise/false positive** | false positive: `${RUNTIME}` is `ARG RUNTIME=debian:bookworm-slim`, a tag the rule cannot see through (it is not a digest: see the source-not-pinned row for the same file) |
| 15 | `dockerfile.best-practice.remove-package-cache` | `crates/packaging/Containerfile:22` | `apt-get update` without `apt-get clean` | **noise/false positive** | the same RUN removes `/var/lib/apt/lists/*`, and the Debian base images' docker-clean apt configuration already discards downloaded packages |
| 16 | `dockerfile.best-practice.remove-package-cache` | `crates/packaging/Containerfile:76` | `apt-get update` without `apt-get clean` | **noise/false positive** | the same RUN removes `/var/lib/apt/lists/*`, and the Debian base images' docker-clean apt configuration already discards downloaded packages |
| 17 | `dockerfile.best-practice.remove-package-cache` | `crates/packaging/Containerfile.testapp:25` | `apt-get update` without `apt-get clean` | **noise/false positive** | the same RUN removes `/var/lib/apt/lists/*`, and the Debian base images' docker-clean apt configuration already discards downloaded packages |
| 18 | `kotlin.lang.security.gcm-detection` | `crates/mobile/android/app/src/main/kotlin/com/comptus/fueros/AndroidShell.kt:275` | `GCMParameterSpec` built (nonce-reuse audit) | **noise/false positive** | the decrypt path, reading the IV stored with the ciphertext; the encrypt path (line 206) lets the Keystore provider generate a fresh random IV per seal and stores it, so no nonce is reused |
| 19 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/androidTest/kotlin/com/comptus/fueros/MediaEgressTest.kt:57` | plain `Socket`/`ServerSocket`, not TLS | **noise/false positive** | test-only: a server the test itself connects to, on loopback or an ephemeral port |
| 20 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/main/kotlin/com/comptus/fueros/DiagStream.kt:163` | plain `Socket`/`ServerSocket`, not TLS | **noise/false positive** | field-test flavour only: the live diagnostics stream to the bench on the tester's LAN, which the releasable flavour never constructs. No root section accepts it because diagnostics are tooling outside the specification, not a design trade-off; **flagged for the author's eye** for that reason |
| 21 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/test/kotlin/com/comptus/fueros/DiagStreamTest.kt:45` | plain `Socket`/`ServerSocket`, not TLS | **noise/false positive** | test-only: a server the test itself connects to, on loopback or an ephemeral port |
| 22 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/test/kotlin/com/comptus/fueros/DiagStreamTest.kt:63` | plain `Socket`/`ServerSocket`, not TLS | **noise/false positive** | test-only: a server the test itself connects to, on loopback or an ephemeral port |
| 23 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/test/kotlin/com/comptus/fueros/DiagStreamTest.kt:91` | plain `Socket`/`ServerSocket`, not TLS | **noise/false positive** | test-only: a server the test itself connects to, on loopback or an ephemeral port |
| 24 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/test/kotlin/com/comptus/fueros/LaunchTest.kt:20` | plain `Socket`/`ServerSocket`, not TLS | **noise/false positive** | test-only: a server the test itself connects to, on loopback or an ephemeral port |
| 25 | `kotlin.lang.security.unencrypted-socket` | `crates/mobile/android/app/src/test/kotlin/com/comptus/fueros/ProvisionTest.kt:24` | plain `Socket`/`ServerSocket`, not TLS | **noise/false positive** | test-only: a server the test itself connects to, on loopback or an ephemeral port |
| 26 | `python.lang.best-practice.arbitrary-sleep` | `crates/sim/tools/wan.py:244` | `time.sleep()` with a constant | **noise/false positive** | bounded polling of the cloud provider and of cloud-init in the WAN harness; the wait is the intent |
| 27 | `python.lang.best-practice.arbitrary-sleep` | `crates/sim/tools/wan.py:256` | `time.sleep()` with a constant | **noise/false positive** | bounded polling of the cloud provider and of cloud-init in the WAN harness; the wait is the intent |
| 28 | `python.lang.best-practice.unspecified-open-encoding` | `Robot/matrixcheck.py:7` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 29 | `python.lang.best-practice.unspecified-open-encoding` | `Robot/matrixcheck.py:9` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 30 | `python.lang.best-practice.unspecified-open-encoding` | `Robot/matrixcheck.py:30` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 31 | `python.lang.best-practice.unspecified-open-encoding` | `Robot/tools/docput.py:50` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 32 | `python.lang.best-practice.unspecified-open-encoding` | `Robot/tools/docput.py:72` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 33 | `python.lang.best-practice.unspecified-open-encoding` | `Robot/tools/lintreport.py:115` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 34 | `python.lang.best-practice.unspecified-open-encoding` | `crates/codec/fuzz/seed.py:8` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 35 | `python.lang.best-practice.unspecified-open-encoding` | `crates/sim/tools/wan.py:147` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 36 | `python.lang.best-practice.unspecified-open-encoding` | `crates/sim/tools/wan.py:178` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 37 | `python.lang.best-practice.unspecified-open-encoding` | `crates/sim/tools/wan.py:247` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 38 | `python.lang.best-practice.unspecified-open-encoding` | `crates/sim/tools/wan.py:354` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 39 | `python.lang.best-practice.unspecified-open-encoding` | `crates/tools/bench-host-key.py:44` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 40 | `python.lang.best-practice.unspecified-open-encoding` | `crates/tools/bench-host-key.py:56` | `open()` without `encoding=` | **noise/false positive** | a development tool run on Linux; under a UTF-8 locale the default is UTF-8, and under the CI image's C/POSIX locale Python 3.7+ switches to UTF-8 mode by itself (PEP 540) |
| 41 | `python.lang.correctness.common-mistakes.string-concat-in-list` | `Robot/stalecheck.py:66` | implicit string concatenation in a list (missing comma?) | **noise/false positive** | false positive: two literals deliberately joined and then `.split()` into words; not a list literal |
| 42 | `python.lang.maintainability.return-not-in-function` | `Robot/tools/docput.py:51` | `return` outside a function | **noise/false positive** | false positive: the matched code is a `lambda` used as a sort key |
| 43 | `python.lang.maintainability.return-not-in-function` | `crates/acceptance/tools/gen_stubs.py:16` | `return` outside a function | **noise/false positive** | false positive: the matched code is a `lambda` used as a sort key |
| 44 | `python.lang.security.audit.dangerous-subprocess-use-audit` | `crates/tools/bench-host-key.py:97` | `subprocess.run` (command-injection audit) | **noise/false positive** | false positive: an argument list with no shell, naming `cargo run -p rhtn-cli` and a path the tool made |
| 45 | `python.lang.security.audit.dynamic-urllib-use-detected` | `crates/sim/tools/wan.py:92` | `urlopen` with a non-literal URL | **noise/false positive** | false positive: the URL is the constant `https://api.digitalocean.com` plus a path the tool builds; no `file:` or other scheme is reachable |
| 46 | `python.lang.security.audit.insecure-file-permissions` | `crates/tools/bench-host-key.py:87` | `os.chmod` call | **noise/false positive** | false positive: 0o700 narrows a temporary directory to its owner |
| 47 | `python.lang.security.audit.regex_dos` | `Robot/refcheck.py:122` | regex with nested quantifiers (ReDoS audit) | **noise/false positive** | applied to the project's own documents and catalogue, never to untrusted input |
| 48 | `python.lang.security.audit.regex_dos` | `Robot/stalecheck.py:68` | regex with nested quantifiers (ReDoS audit) | **noise/false positive** | applied to the project's own documents and catalogue, never to untrusted input |
| 49 | `python.lang.security.audit.regex_dos` | `Robot/tools/kdoccheck.py:32` | regex with nested quantifiers (ReDoS audit) | **noise/false positive** | applied to the project's own documents and catalogue, never to untrusted input |
| 50 | `python.lang.security.audit.regex_dos` | `crates/acceptance/tools/catalogue.py:53` | regex with nested quantifiers (ReDoS audit) | **noise/false positive** | applied to the project's own documents and catalogue, never to untrusted input |
| 51 | `rust.lang.security.args` | `crates/cli/src/main.rs:107` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 52 | `rust.lang.security.args` | `crates/daemon/src/main.rs:19` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 53 | `rust.lang.security.args` | `crates/desktop/examples/render_pages.rs:13` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 54 | `rust.lang.security.args` | `crates/desktop/src/main.rs:71` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 55 | `rust.lang.security.args` | `crates/ffi/src/bin/payload-peer.rs:129` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 56 | `rust.lang.security.args` | `crates/participant/src/main.rs:25` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 57 | `rust.lang.security.args` | `crates/sim/examples/provision.rs:22` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 58 | `rust.lang.security.args` | `crates/testapp/src/main.rs:22` | `std::env::args()` read | **noise/false positive** | option parsing that skips argv[0]; the rule's concern is trusting argv[0] (the program's own path), which none of these reads |
| 59 | `rust.lang.security.temp-dir` | `crates/archive/tests/chain.rs:879` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 60 | `rust.lang.security.temp-dir` | `crates/cli/tests/diag.rs:165` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 61 | `rust.lang.security.temp-dir` | `crates/cli/tests/live.rs:319` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 62 | `rust.lang.security.temp-dir` | `crates/cli/tests/probe.rs:73` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 63 | `rust.lang.security.temp-dir` | `crates/cli/tests/probe.rs:280` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 64 | `rust.lang.security.temp-dir` | `crates/client/tests/position.rs:431` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 65 | `rust.lang.security.temp-dir` | `crates/client/tests/record.rs:911` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 66 | `rust.lang.security.temp-dir` | `crates/conformance/tests/daemon.rs:15` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 67 | `rust.lang.security.temp-dir` | `crates/conformance/tests/findings.rs:116` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 68 | `rust.lang.security.temp-dir` | `crates/conformance/tests/findings.rs:271` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 69 | `rust.lang.security.temp-dir` | `crates/conformance/tests/latest.rs:12` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 70 | `rust.lang.security.temp-dir` | `crates/conformance/tests/latest.rs:65` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 71 | `rust.lang.security.temp-dir` | `crates/conformance/tests/revision.rs:69` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 72 | `rust.lang.security.temp-dir` | `crates/conformance/tests/revision.rs:99` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 73 | `rust.lang.security.temp-dir` | `crates/conformance/tests/september15.rs:60` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 74 | `rust.lang.security.temp-dir` | `crates/conformance/tests/september26.rs:741` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 75 | `rust.lang.security.temp-dir` | `crates/conformance/tests/september26.rs:926` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 76 | `rust.lang.security.temp-dir` | `crates/conformance/tests/september26.rs:961` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 77 | `rust.lang.security.temp-dir` | `crates/conformance/tests/september26.rs:1126` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 78 | `rust.lang.security.temp-dir` | `crates/crypto/src/ssh.rs:487` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 79 | `rust.lang.security.temp-dir` | `crates/daemon/src/packages.rs:374` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 80 | `rust.lang.security.temp-dir` | `crates/daemon/src/runner.rs:271` | `std::env::temp_dir()` used for a path | **noise/false positive** | shipped code, and the case the rule exists for: a private key written for one act. Opened `create_new` (O_EXCL, which refuses an existing file or symlink) at 0600 under a pid-and-nanosecond name, overwritten and removed on drop; a planted path makes the write fail, never land elsewhere |
| 81 | `rust.lang.security.temp-dir` | `crates/daemon/tests/administration.rs:82` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 82 | `rust.lang.security.temp-dir` | `crates/daemon/tests/delegated.rs:66` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 83 | `rust.lang.security.temp-dir` | `crates/daemon/tests/delegated.rs:326` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 84 | `rust.lang.security.temp-dir` | `crates/daemon/tests/delegated.rs:391` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 85 | `rust.lang.security.temp-dir` | `crates/daemon/tests/delegated.rs:474` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 86 | `rust.lang.security.temp-dir` | `crates/daemon/tests/fabric.rs:121` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 87 | `rust.lang.security.temp-dir` | `crates/daemon/tests/fabric.rs:277` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 88 | `rust.lang.security.temp-dir` | `crates/daemon/tests/federation.rs:61` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 89 | `rust.lang.security.temp-dir` | `crates/daemon/tests/host.rs:19` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 90 | `rust.lang.security.temp-dir` | `crates/daemon/tests/hosting.rs:12` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 91 | `rust.lang.security.temp-dir` | `crates/daemon/tests/lifecycle.rs:51` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 92 | `rust.lang.security.temp-dir` | `crates/daemon/tests/runner_script.rs:43` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 93 | `rust.lang.security.temp-dir` | `crates/daemon/tests/runner_script.rs:236` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 94 | `rust.lang.security.temp-dir` | `crates/desktop/src/custody.rs:172` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 95 | `rust.lang.security.temp-dir` | `crates/desktop/src/lib.rs:562` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 96 | `rust.lang.security.temp-dir` | `crates/desktop/src/pairing.rs:320` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 97 | `rust.lang.security.temp-dir` | `crates/desktop/src/participant.rs:346` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 98 | `rust.lang.security.temp-dir` | `crates/desktop/src/platform.rs:171` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 99 | `rust.lang.security.temp-dir` | `crates/desktop/src/threads.rs:416` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 100 | `rust.lang.security.temp-dir` | `crates/node/src/federation.rs:2725` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 101 | `rust.lang.security.temp-dir` | `crates/node/src/horizon.rs:242` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 102 | `rust.lang.security.temp-dir` | `crates/node/src/horizon.rs:255` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 103 | `rust.lang.security.temp-dir` | `crates/node/src/horizon.rs:271` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 104 | `rust.lang.security.temp-dir` | `crates/node/src/horizon.rs:293` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 105 | `rust.lang.security.temp-dir` | `crates/node/src/horizon.rs:304` | `std::env::temp_dir()` used for a path | **noise/false positive** | inside the file's `#[cfg(test)] mod tests`; not compiled into any binary |
| 106 | `rust.lang.security.temp-dir` | `crates/node/tests/horizon.rs:56` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 107 | `rust.lang.security.temp-dir` | `crates/node/tests/persist.rs:56` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 108 | `rust.lang.security.temp-dir` | `crates/node/tests/persist.rs:131` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 109 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:323` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 110 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:405` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 111 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:487` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 112 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:527` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 113 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:609` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 114 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:702` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 115 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:778` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 116 | `rust.lang.security.temp-dir` | `crates/node/tests/prekeys.rs:809` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 117 | `rust.lang.security.temp-dir` | `crates/node/tests/profiles.rs:127` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 118 | `rust.lang.security.temp-dir` | `crates/node/tests/queue.rs:91` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 119 | `rust.lang.security.temp-dir` | `crates/node/tests/queue.rs:152` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 120 | `rust.lang.security.temp-dir` | `crates/node/tests/queue.rs:199` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 121 | `rust.lang.security.temp-dir` | `crates/node/tests/wake.rs:20` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 122 | `rust.lang.security.temp-dir` | `crates/resources/tests/sandbox.rs:329` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 123 | `rust.lang.security.temp-dir` | `crates/sim/src/daemons.rs:102` | `std::env::temp_dir()` used for a path | **noise/false positive** | `rhtn-sim` is the simulation harness, a dev-dependency of four crates and of nothing shipped |
| 124 | `rust.lang.security.temp-dir` | `crates/sim/src/participants.rs:95` | `std::env::temp_dir()` used for a path | **noise/false positive** | `rhtn-sim` is the simulation harness, a dev-dependency of four crates and of nothing shipped |
| 125 | `rust.lang.security.temp-dir` | `crates/sim/tests/restart.rs:81` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 126 | `rust.lang.security.temp-dir` | `crates/sim/tests/restart.rs:139` | `std::env::temp_dir()` used for a path | **noise/false positive** | integration-test fixture directory; test code, never shipped |
| 127 | `rust.lang.security.unsafe-usage` | `crates/client/tests/zeroize.rs:45` | `unsafe` block | **noise/false positive** | test-only: drops in place to prove the memory is zeroed, with its SAFETY comment |
| 128 | `rust.lang.security.unsafe-usage` | `crates/daemon/tests/lifecycle.rs:113` | `unsafe` block | **noise/false positive** | test-only: `kill(pid, SIGTERM)` through libc to stop a spawned daemon |
| 129 | `rust.lang.security.unsafe-usage` | `crates/daemon/tests/lifecycle.rs:135` | `unsafe` block | **noise/false positive** | test-only: `kill(pid, SIGTERM)` through libc to stop a spawned daemon |
| 130 | `rust.lang.security.unsafe-usage` | `crates/sim/src/daemons.rs:395` | `unsafe` block | **noise/false positive** | dev-only harness: `kill(pid, SIGTERM)` through libc to stop a spawned daemon |
