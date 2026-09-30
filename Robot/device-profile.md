# Minimum Android device for the light client — a profile from the tree (2026-09-30)

A working estimate for buying test phones, measured where the tree can be
measured and stated as an estimate where it cannot. Nothing here is a
requirement the documents make; `light-client-requirements.md` §1.3 asks for
the strongest channel *the hardware supports* and reports a missing radio as
unavailable, and that is the floor's shape: **a camera and a 64-bit SoC on
Android 12 runs a ceremony; NFC and UWB raise the channel, not the bar.**

## What the tree fixes today

| Item | Value | Where |
|---|---|---|
| `minSdk` | **31 (Android 12)** — chosen because the platform UWB API arrived there | `crates/mobile/android/app/build.gradle.kts` |
| ABIs | **arm64-v8a** (and x86_64 for the emulator); 32-bit ARM is not built | same |
| NDK platform | cargo-ndk's default (21); the native library itself does not need 31 | `tools/build-native.sh` |
| Native library, release, arm64 | **11.4 MB** | `jniLibs/arm64-v8a/librhtn_ffi.so` |
| Debug APK (both ABIs, JNA, unshrunk) | 39.1 MB | `app/build/outputs/apk/debug` |
| Dependency closure of the FFI | **156 crates**; the heavy ones are aws-lc-rs (TLS, hashing, X25519/Ed25519), quinn + rustls (QUIC, X25519MLKEM768), tokio, ml-kem, ml-dsa, argon2, uniffi. **No wasmtime** (the resource engine is the node's, not the client's) and **no `ort` yet** | `cargo tree -p rhtn-ffi` |
| Manifest | `INTERNET` only; no `uses-feature`, so nothing filters installation by hardware | `AndroidManifest.xml` |
| Backup KDF | Argon2id, **64 MiB**, 3 passes, 1 lane, transient at backup and restore | `client/src/backup.rs` |

## Measured: peak resident memory, release, host x86_64

`/usr/bin/time -v` over each client and FFI test binary, single-threaded.

| Binary | Peak RSS |
|---|---|
| ceremony (10 tests) | 13.6 MB |
| payload — sessions and ratchets (16) | 13.5 MB |
| recovery (4) | 16.8 MB |
| query (3), position (5) | 12.2 MB, 10.1 MB |
| horizon (13), verifier (10), record (11) | 5.2 MB, 5.3 MB, 4.8 MB |
| **FFI boundary — the kernel with tokio, QUIC and a serving node in-process (9)** | **87.3 MB** |

The client's own working set is tens of megabytes; the 87 MB figure carries a
whole serving node beside it, which a phone never does. Not measured, and
added by estimate below: the biometric engine, the camera pipeline and the
Android runtime itself.

## The estimate

| Axis | Floor | Comfortable | Why |
|---|---|---|---|
| **SoC** | any **64-bit ARMv8** (Cortex-A53 class, 2016+) | Cortex-A7x class | ML-DSA-65 verification and ML-KEM-768 are single-digit ms on A53; Argon2 at 64 MiB × 3 passes is ~1–3 s on A53, once per backup; SFace int8 is ~50–150 ms per face on A53 CPU, ~10–30 ms on A7x. No NPU or GPU is used or needed |
| **RAM** | **2 GB** | **3–4 GB** | Kernel ≤ ~30 MB; Argon2 64 MiB transient; `ort` + SFace int8 + YuNet ~50–100 MB transient at capture; the camera pipeline and the Android runtime dominate the rest. 1 GB Android Go devices would run it but capture is where they would fall over, and Go builds of 12+ are rare |
| **Storage** | **16 GB device**, ~200 MB for us | 32 GB | APK release arm64-only ~15–18 MB, +~25 MB once `ort` and the 9.9 MB SFace model ship; records ~35 KB each, ~7 MB for 200 meetings (design §7.2); captures under 100 MB over 730 days (design §7.5); anchor index budget 25 MB (design §12.2). Under 200 MB all in |
| **Android** | **12 (API 31)** as built | 13–14 | The only thing 31 buys is the platform UWB API. Everything else in the tree is fine from API 26 (JNA 5.17 supports 21+, the library is built for 21). A build with the UWB path guarded by API level could drop `minSdk` to 26–28 and admit phones from 2017–2019; that is a build decision to make deliberately, not a fact about the code |
| **Cameras** | front **and** rear, any resolution ≥ 2 MP, fixed focus acceptable | autofocus rear | The bootstrap QR is read rear-camera; the intent QR (a 32-byte ceremony-id plus a 16-byte contribution — a small QR) is read by the **selfie** camera at arm's length (`Robot/light-client-screens.md`; `wire-format.md` §14.3.1 says it resolves on a modest one). The guided capture is 3–5 frames at face-crop resolution; nothing needs a good camera |
| **NFC** | not required; **required for the NFC channel** | — | Design §7.6.3's channel 2. Many budget phones ship *variants* without NFC (Redmi/Realme regional SKUs); check the exact SKU |
| **UWB** | not required; **required for the strongest channel** | — | Design §7.6.3's channel 1. Handset UWB is flagship-only: Pixel 6 Pro / 7 Pro / 8 Pro / 9 Pro, Galaxy S21+/Ultra onward, Note20 Ultra, Z Fold 2+, Xiaomi Mix 4, Motorola Edge+ (2022/23). Used Pixel 6 Pro or S21+ are the cheap end |
| **Keystore** | any | hardware-backed / StrongBox | `light-client-requirements.md` §9: the wrapping key is hardware-backed *where the device has one* |
| **Radios for the bearer** | Wi-Fi; Bluetooth for a local bearer once built | — | `wire-format.md` §14.3.1 leaves the bulk bearer to the shell; a FuerosNet fetch is the last fallback |

## A fleet that proves the floor and exercises the top

- **2 × floor phones**: 64-bit, Android 12+, 3–4 GB, NFC-equipped SKU, front and rear camera. Moto G-series with NFC, Samsung Galaxy A1x/A2x (NFC variants), or a used Pixel 4a/5a. These prove the least-restrictive claim and run the NFC and optical channels between them.
- **2 × UWB phones**, used: Pixel 6 Pro or Galaxy S21+/Note20 Ultra. The only way to run channel 1 device-to-device.
- **1 × verifier**: any of the above, or the desktop client, since a verifier does not range.
- The x86_64 emulator covers the kernel and screens but no radio and no real camera.

## What would move these numbers

`ort` landing (adds ~15–25 MB of onnxruntime per ABI and the transient
inference memory); a decision to drop `minSdk` below 31; the physical run,
which will replace the CPU estimates with timings.
