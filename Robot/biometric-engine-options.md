# Biometric engine options — a survey for design §22.2

**What this is.** design §22.2 left the biometric engine open, and the
reference `HashEngine` recognises nobody. The author asked (2026-09-29) for
a survey of permissively licensed, cross-platform, on-device face
verification options. This is that survey, researched 2026-09-29 via web
sources cited inline; licences and figures are as of that date and should be
re-verified at integration time.

**The decision is taken** [author, 2026-09-29]: the first choice below —
OpenCV SFace int8 with YuNet, through the `ort` Rust crate — is accepted,
and design §22.2 records the extractor and matcher as chosen. This file
stays as the survey that grounded it; nothing in the root cites it.

**What the engine must do** (from design §7.5, §22.2 and the client as
built): derive a fixed-length template from 3–5 prompted frames captured on
a phone; derive a fuzzed profile from a template; compare a profile against
a stored template to match / no-match / inconclusive. Entirely on-device —
a cloud API is disqualifying by construction. 1:1 verification, never 1:N
identification. Per the dependency licence policy: Apache-2.0/MIT or looser
for **both code and model weights**; AGPL only in segregated client apps,
which an engine linked into the kernel is not.

## The headline finding

Permissively licensed *code* is everywhere; permissively licensed *weights*
are rare, because nearly every strong face-embedding model is trained on
research-only datasets (MS1M variants, VGGFace2 CC BY-NC, CASIA-WebFace).
Whether weights inherit a training set's licence is unsettled law, but
Oxford VGG states that models trained on its data are non-commercial-only
(<https://www.robots.ox.ac.uk/~vgg/data/vgg_face/licence.txt>), so the safe
rule is: **accept only weights whose distributor grants a permissive licence
on the weight file itself.** Three candidates survive that filter.

## The three that survive

| Option | Provides | Code | Weights | Size | Accuracy | Path |
|---|---|---|---|---|---|---|
| **OpenCV SFace** (MobileFaceNet + SFace loss, opencv_zoo) | 128-d embedding + published match thresholds; pair with YuNet (MIT) for detect/align | Apache-2.0 | **Apache-2.0** — LICENSE in `models/face_recognition_sface` of opencv_zoo; provenance question open (opencv_zoo issue #313, unanswered) | 38.7 MB fp32 / **9.9 MB int8** | 99.40% LFW (int8 99.32%) | ONNX → `ort` or `tract` (both Rust, both permissive) |
| **AuraFace-v1** (fal.ai, ArcFace-loss ResNet100) | 512-d embedding; SCRFD detector in repo | Apache-2.0 | **Apache-2.0, explicitly commercially-usable training data** — the cleanest provenance in the field (huggingface.co/fal/AuraFace-v1) | **261 MB fp32**; ~65 MB int8 self-quantised | CFP-FP 95.18 / AgeDB 96.10; thin published set, no LFW/IJB-C | ONNX, same paths; no mobile artifact shipped |
| **dlib ResNet-29** | detect + landmarks + 128-d embedding | Boost | **Public domain** — dlib-models README says so outright | ~22 MB | 99.38% LFW (2017-era; no hard-set figures) | C++ core, trivial Rust FFI, no ONNX runtime needed; dlib 20.0.1 (2026-03) still maintained |

## Disqualified, named so nobody re-treads them

- **InsightFace** (buffalo_l, antelopev2): MIT code, **non-commercial
  weights** per its model_zoo README — the default in every tutorial and
  most wrapper crates, and the single most likely trap. It has already
  burned prominent projects (InstantID).
- **EdgeFace** (Idiap): technically the best mobile fit (1.77M params,
  IJB-C 94.85) — weights CC BY-NC-SA; commercial licence sold separately.
- **facenet-pytorch, GhostFaceNets, tutorial MobileFaceNet .tflite files**:
  permissive code over weights trained on NC datasets with no independent
  grant. The clean MobileFaceNet *is* OpenCV SFace.
- **MediaPipe Face** (Google, Apache-2.0 throughout): no embedding task —
  detection, landmarks and blendshapes only. Useful for alignment and for
  active-liveness cues under the prompted-movement capture, not matching.

## The advice

**First choice: OpenCV SFace int8 (9.9 MB) + YuNet, through the `ort` Rust
crate, with `tract` as a pure-Rust fallback.** The only sub-10 MB model with
an explicit permissive grant from an accountable distributor, published
verification thresholds (cosine 0.363 / L2 1.128, 112×112 in, 128-d out),
and the whole pipeline — detect, align, embed, compare — permissive end to
end. Fits the kernel's Rust-core-plus-FFI shape with one C dependency
(`ort`) or none (`tract`). Blemish: the training-data paper trail is
thinner than AuraFace's; the Apache grant itself stands.

**Upgrade path if field captures defeat SFace: AuraFace-v1, self-quantised.**
Cleanest provenance story anywhere, higher capacity, but 261 MB fp32 means
producing and validating our own mobile build, and its published benchmarks
are thin.

**Conservative fallback: dlib.** Public-domain weights, nine years in
production, no inference runtime at all — at 2017-grade accuracy.

**Liveness**: the ceremony's prompted movements are active liveness the
capture flow already owns; MediaPipe blendshapes can score them, and
Silent-Face-Anti-Spoofing / MiniFASNet (Apache-2.0, ~2 MB) adds a passive
print/replay check cheaply. Secondary either way.

**One caution the survey cannot remove**: design §7.4.4 already treats
false rejection as the common failure and matching quality as
policy-weighted, not validity-gating. Whatever engine is chosen, the
protocol's own posture — match / no-match / inconclusive as a weighed claim
— is what absorbs a mid-grade model; nothing here needs to be perfect to be
honest.
