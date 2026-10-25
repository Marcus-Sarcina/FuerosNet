//! **The reference client's biometric engine** (design §22.2 [author,
//! 2026-09-29]): OpenCV's SFace with YuNet for detection and alignment —
//! 112×112 input, a 128-dimension template compared by cosine distance,
//! Apache-2.0 for code and weights alike — run here over `tract`, so one
//! implementation serves every platform the kernel builds for and the
//! profile one client produces is what another's matcher compares
//! (§7.4.4: the one part of the presence layer that cannot be pluggable).
//!
//! **Modality version 2** is this engine's: the template is 128 `f32`,
//! little-endian, 512 bytes; the profile a query carries is the template
//! quantised to `i8` with seeded noise, 128 bytes (§7.5's fuzzing target);
//! the verdict is categorical at two cosine thresholds with an
//! inconclusive band between (§7.4.4). The fuzz is the part §22.2 still
//! leaves to implementation and is [`FUZZ_SIGMA`] here, provisional.
//!
//! **The thresholds are this client's own policy and not a standard**
//! [author, 2026-10-13]. What has to be shared is everything up to the
//! comparison — the template's format and length, the fuzzing algorithm,
//! the matcher — because a profile one engine produces must be readable
//! by another's. Where the verdict is read is internal to the verifying
//! client and enforceable by nobody (design §1.1), and the wire carries
//! the verdict rather than the score, so another implementer may be
//! stricter or looser than this one is.
//!
//! **What this client chose, and on what.** [`Thresholds`] takes
//! `match_at` from the figure OpenCV's own SFace demo compares at, 0.363
//! cosine, in the repository these weights come from
//! (`github.com/opencv/opencv_zoo`, `face_recognition_sface_2021dec.onnx`,
//! the model of the paper "SFace: Sigmoid-Constrained Hypersphere Loss
//! for Robust Face Recognition"). It is the one number within reach that
//! was set against impostors, which is what a match threshold has to be
//! set against. The bench's genuine comparisons — 0.732, 0.750, 0.813,
//! 0.838 and 0.876, across four phones — bound nothing on that side:
//! they are one person, in one room, inside a few days, and a threshold
//! drawn to clear that margin would reject the ordinary case of the same
//! face a year later in other light [author, 2026-10-13]. `no_match_below`
//! sits well under the match point because the two errors are not alike:
//! a `NoMatch` is signed evidence against a person, carried in a record,
//! and one mismatch is not proof of fraud (functional row POL-016), while
//! an `Inconclusive` asserts nothing — so a weak capture of the right
//! person belongs in the band.
//!
//! The frames the kernel hands over are the camera's luminance plane at
//! a geometry the platform states when it builds the engine; the models
//! take three channels and are given the one, three times.

use rhtn_client::device::Engine;
use rhtn_client::query::Verdict;
use rhtn_client::store::Frame;
use rhtn_client::verifier::Matcher;
use std::io::Cursor;
use tract_onnx::prelude::*;

/// The modality version this engine produces and compares under.
pub const TEMPLATE_VERSION: u64 = 2;
/// The template's fixed length under version 2: 128 × f32, little-endian.
pub const TEMPLATE_LEN: usize = 128 * 4;
/// The profile's length: 128 × i8.
pub const PROFILE_LEN: usize = 128;
/// The embedding's dimension.
pub const DIM: usize = 128;
/// SFace's input side.
pub const SIDE: usize = 112;
/// The largest frame a camera may hand over, on a side.
const FRAME_SIDE: usize = 640;
/// YuNet's input side here: 320.  The graph was exported declaring
/// 640×640, and its convolutions take any size `tract` is told; a frame is
/// shrunk two-to-one where it needs to be, placed top-left on a black
/// canvas of this size, and the coordinates read back are scaled to the
/// frame's own.  **A quarter of the work of the 640 canvas the engine ran
/// at until 2026-10-25**, when the bench's low-end phones took four to
/// eight seconds a detection and sampled a 30 s window five times; a face
/// held at arm's length is a hundred pixels across at this size, far
/// above what the detector needs.
const CANVAS: usize = 320;
/// The orientation no face has been found at yet (see [`SFace::find`]).
const TURNS_UNKNOWN: u8 = u8::MAX;
/// The noise added to a profile, per unit-vector component (provisional).
pub const FUZZ_SIGMA: f32 = 0.03;

/// The cosine thresholds a verdict is read at (design §7.4.4; §22.2
/// leaves them to the implementation).
///
/// **Loosened for production** [author, 2026-10-13]: the bench's genuine
/// comparisons ran 0.732 to 0.876, and every one of them is the same
/// person, in one room, within a few days — a sample that says nothing
/// about the same face a year later, in other light, on another phone.
/// Thresholds chosen to clear that margin would reject the ordinary case.
///
/// `match_at` is **SFace's own published operating point**, which is the
/// only figure here that was measured against impostors; this bench has
/// one face and so bounds no false-accept rate at all, and nothing below
/// that point can be justified from what it shows. `no_match_below` sits
/// well under it because the two errors are not alike: a `NoMatch` is
/// signed evidence against a person, carried in a record — the design has
/// to warn that one mismatch is not proof of fraud — while an
/// `Inconclusive` asserts nothing. A weak capture of the right person
/// belongs in the band, not in an accusation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// At or above: `Match`.
    pub match_at: f32,
    /// Below: `NoMatch`; between the two: `Inconclusive`.
    pub no_match_below: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            match_at: 0.363,
            no_match_below: 0.20,
        }
    }
}

/// A face YuNet found: its box and five landmarks (right eye, left eye,
/// nose tip, right mouth corner, left mouth corner), in frame pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    /// The detector's confidence, √(class · objectness).
    pub score: f32,
    /// The box's left edge, in frame pixels.
    pub x: f32,
    /// The box's top edge, in frame pixels.
    pub y: f32,
    /// The box's width, in frame pixels.
    pub w: f32,
    /// The box's height, in frame pixels.
    pub h: f32,
    /// The five landmarks the alignment uses, in frame pixels and in the
    /// order this struct's own comment gives.
    pub landmarks: [(f32, f32); 5],
}

type Runnable = std::sync::Arc<TypedRunnableModel>;

/// The engine: both models loaded for one frame geometry.
pub struct SFace {
    yunet: Runnable,
    sface: Runnable,
    width: usize,
    height: usize,
    /// The detector's canvas on a side.
    canvas: usize,
    /// How many frame pixels one canvas pixel stands for: 2 where the
    /// frame is larger than the canvas on either side, 1 otherwise.
    scale: usize,
    /// The quarter turns a face was last found at ([`TURNS_UNKNOWN`]
    /// before any): tried first, since a camera does not turn between
    /// frames and three more detections of an empty frame are the cost of
    /// not remembering.
    turns: std::sync::atomic::AtomicU8,
    thresholds: Thresholds,
    matcher: Cosine,
}

impl std::fmt::Debug for SFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SFace")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("thresholds", &self.thresholds)
            .finish()
    }
}

/// YuNet's output heads, in the order the decoder reads them.
const HEADS: [&str; 12] = [
    "cls_8", "cls_16", "cls_32", "obj_8", "obj_16", "obj_32", "bbox_8", "bbox_16", "bbox_32",
    "kps_8", "kps_16", "kps_32",
];
const STRIDES: [usize; 3] = [8, 16, 32];
/// YuNet's score floor and the overlap at which two boxes are one.
const SCORE_FLOOR: f32 = 0.9;
const NMS_IOU: f32 = 0.3;

impl SFace {
    /// Load both models for frames of `width` × `height` luminance bytes,
    /// which must fit a 640×640 frame; larger than the detector's canvas
    /// on a side, they are shrunk two-to-one for detection.
    pub fn from_onnx(
        yunet: &[u8],
        sface: &[u8],
        width: usize,
        height: usize,
        thresholds: Thresholds,
    ) -> Result<SFace, String> {
        Self::from_onnx_at(yunet, sface, width, height, thresholds, CANVAS)
    }

    /// [`SFace::from_onnx`] at a stated detector canvas: what a bench
    /// measurement compares sizes with.  The reference client uses
    /// [`SFace::from_onnx`].
    #[doc(hidden)]
    pub fn from_onnx_at(
        yunet: &[u8],
        sface: &[u8],
        width: usize,
        height: usize,
        thresholds: Thresholds,
        canvas: usize,
    ) -> Result<SFace, String> {
        if width == 0 || height == 0 || width > FRAME_SIDE || height > FRAME_SIDE {
            return Err(format!(
                "a frame must fit {FRAME_SIDE}x{FRAME_SIDE}, not {width}x{height}"
            ));
        }
        let scale = if width.max(height) > canvas { 2 } else { 1 };
        // the export declares every intermediate tensor's shape at the
        // 640 canvas; those are discarded so the graph is inferred at this
        // one, the convolutions taking any size they are given
        let mut y = tract_onnx::onnx()
            .with_ignore_value_info(true)
            .model_for_read(&mut Cursor::new(yunet))
            .map_err(|e| format!("YuNet: {e}"))?;
        y.set_input_fact(0, f32::fact([1, 3, canvas, canvas]).into())
            .map_err(|e| format!("YuNet input: {e}"))?;
        // the heads by the names the graph declares its outputs under
        // (the tensor labels, not node names); a graph declaring them in
        // another order is read in this one
        let declared: Vec<OutletId> = y.outputs.clone();
        let mut heads: Vec<OutletId> = Vec::with_capacity(HEADS.len());
        for name in HEADS {
            let found = declared
                .iter()
                .find(|o| y.outlet_label(**o).is_some_and(|l| l == name))
                .copied()
                .ok_or_else(|| format!("YuNet declares no output named {name}"))?;
            heads.push(found);
        }
        y.select_output_outlets(&heads)
            .map_err(|e| format!("YuNet outputs: {e}"))?;
        let yunet = y
            .into_optimized()
            .map_err(|e| format!("YuNet optimise: {e:?}"))?
            .into_runnable()
            .map_err(|e| format!("YuNet plan: {e}"))?;
        let mut s = tract_onnx::onnx()
            .model_for_read(&mut Cursor::new(sface))
            .map_err(|e| format!("SFace: {e}"))?;
        s.set_input_fact(0, f32::fact([1, 3, SIDE, SIDE]).into())
            .map_err(|e| format!("SFace input: {e}"))?;
        let sface = s
            .into_optimized()
            .map_err(|e| format!("SFace optimise: {e:?}"))?
            .into_runnable()
            .map_err(|e| format!("SFace plan: {e}"))?;
        Ok(SFace {
            yunet,
            sface,
            width,
            height,
            canvas,
            scale,
            turns: std::sync::atomic::AtomicU8::new(TURNS_UNKNOWN),
            thresholds,
            matcher: Cosine(thresholds),
        })
    }

    /// The most confident face in a luminance frame of the configured
    /// geometry, if any.
    pub fn detect(&self, gray: &[u8]) -> Option<Face> {
        self.detect_in(gray, self.width, self.height)
    }

    /// [`SFace::find`]'s result for a measurement: the face and the turns.
    #[doc(hidden)]
    pub fn find_scored(&self, gray: &[u8]) -> Option<(Face, u8)> {
        self.find(gray).map(|(_, _, _, f, t)| (f, t))
    }

    /// **A face in the frame, whichever way up the camera delivered it.**
    /// A phone's sensor is mounted landscape and its frames arrive
    /// rotated — 270° on the front cameras of this bench — and YuNet
    /// knows upright faces only. The frame is tried as delivered, then
    /// turned a quarter at a time; what comes back is the frame as it was
    /// found, its geometry, the face in that frame, and how many quarter
    /// turns it took, so the embedding is cut from the same pixels.
    pub fn find(&self, gray: &[u8]) -> Option<(Vec<u8>, usize, usize, Face, u8)> {
        // a frame the camera never delivered is no frame: the shell hands
        // an empty one where its wait ran out, and that is nobody, not a
        // reason to stop the client
        if gray.len() < self.width * self.height {
            return None;
        }
        // the orientation a face was last found at first, then the rest
        let last = self.turns.load(std::sync::atomic::Ordering::Relaxed);
        let mut order: Vec<u8> = Vec::with_capacity(4);
        if last != TURNS_UNKNOWN {
            order.push(last);
        }
        order.extend([0u8, 3, 1, 2].iter().filter(|t| **t != last));
        for turns in order {
            let (g, w, h) = rotate(gray, self.width, self.height, turns);
            if let Some(face) = self.detect_in(&g, w, h) {
                self.turns
                    .store(turns, std::sync::atomic::Ordering::Relaxed);
                return Some((g, w, h, face, turns));
            }
        }
        None
    }

    fn detect_in(&self, gray: &[u8], fw: usize, fh: usize) -> Option<Face> {
        let s = self.scale;
        if gray.len() < fw * fh || fw > self.canvas * s || fh > self.canvas * s {
            return None;
        }
        let (w, h) = (self.canvas, self.canvas);
        // the frame shrunk `s` to one by block mean, placed top-left
        let (sw, sh) = (fw / s, fh / s);
        let input = tract_ndarray::Array4::<f32>::from_shape_fn((1, 3, h, w), |(_, _, y, x)| {
            if x < sw && y < sh {
                let mut sum = 0u32;
                for dy in 0..s {
                    for dx in 0..s {
                        sum += u32::from(gray[(y * s + dy) * fw + x * s + dx]);
                    }
                }
                sum as f32 / (s * s) as f32
            } else {
                0.0
            }
        });
        let st_scale = s as f32;
        let outputs = self.yunet.run(tvec!(Tensor::from(input).into())).ok()?;
        let view = |i: usize| outputs[i].to_plain_array_view::<f32>().ok();
        let mut faces: Vec<Face> = Vec::new();
        for (s, &stride) in STRIDES.iter().enumerate() {
            let (cls, obj, bbox, kps) = (view(s)?, view(3 + s)?, view(6 + s)?, view(9 + s)?);
            let cols = w / stride;
            let rows = h / stride;
            let n = cols * rows;
            let cls = cls.into_shape_with_order((n,)).ok()?;
            let obj = obj.into_shape_with_order((n,)).ok()?;
            let bbox = bbox.into_shape_with_order((n, 4)).ok()?;
            let kps = kps.into_shape_with_order((n, 10)).ok()?;
            for i in 0..n {
                let score = (cls[i].clamp(0.0, 1.0) * obj[i].clamp(0.0, 1.0)).sqrt();
                if score < SCORE_FLOOR {
                    continue;
                }
                let (r, c) = ((i / cols) as f32, (i % cols) as f32);
                // canvas pixels back to frame pixels
                let st = stride as f32 * st_scale;
                let cx = (c + bbox[(i, 0)]) * st;
                let cy = (r + bbox[(i, 1)]) * st;
                let bw = bbox[(i, 2)].exp() * st;
                let bh = bbox[(i, 3)].exp() * st;
                let mut landmarks = [(0.0f32, 0.0f32); 5];
                for (k, l) in landmarks.iter_mut().enumerate() {
                    *l = ((c + kps[(i, 2 * k)]) * st, (r + kps[(i, 2 * k + 1)]) * st);
                }
                faces.push(Face {
                    score,
                    x: cx - bw / 2.0,
                    y: cy - bh / 2.0,
                    w: bw,
                    h: bh,
                    landmarks,
                });
            }
        }
        nms(faces).into_iter().next()
    }

    /// The unit embedding of `face` in `gray`, a frame of the configured
    /// geometry: aligned to SFace's five reference points, run, and
    /// L2-normalised.
    pub fn embed(&self, gray: &[u8], face: &Face) -> Option<[f32; DIM]> {
        self.embed_in(gray, self.width, self.height, face)
    }

    fn embed_in(&self, gray: &[u8], fw: usize, fh: usize, face: &Face) -> Option<[f32; DIM]> {
        let crop = align(gray, fw, fh, &face.landmarks);
        let input =
            tract_ndarray::Array4::<f32>::from_shape_fn((1, 3, SIDE, SIDE), |(_, _, y, x)| {
                crop[y * SIDE + x]
            });
        let outputs = self.sface.run(tvec!(Tensor::from(input).into())).ok()?;
        let out = outputs[0].to_plain_array_view::<f32>().ok()?;
        let flat: Vec<f32> = out.iter().copied().collect();
        if flat.len() != DIM {
            return None;
        }
        let mut v = [0f32; DIM];
        v.copy_from_slice(&flat);
        Some(normalise(v))
    }
}

impl Engine for SFace {
    /// The template: the mean of the frames' unit embeddings, each frame
    /// with a face contributing one; empty where no frame had a face,
    /// which the kernel refuses as a capture that did not happen.
    fn template(&self, frames: &[Frame]) -> Vec<u8> {
        let mut sum = [0f32; DIM];
        let mut n = 0usize;
        for (i, f) in frames.iter().enumerate() {
            let t0 = std::time::Instant::now();
            let found = self.find(&f.bytes);
            match &found {
                Some((_, _, _, face, turns)) => tracing::info!(
                    target: "engine",
                    frame = i,
                    score = face.score,
                    turns,
                    took_ms = t0.elapsed().as_millis() as u64,
                    "engine.face"
                ),
                None => tracing::warn!(
                    target: "engine",
                    frame = i,
                    took_ms = t0.elapsed().as_millis() as u64,
                    "engine.no_face"
                ),
            }
            let Some((g, w, h, face, _)) = found else {
                continue;
            };
            let Some(e) = self.embed_in(&g, w, h, &face) else {
                continue;
            };
            for (s, x) in sum.iter_mut().zip(e.iter()) {
                *s += x;
            }
            n += 1;
        }
        tracing::info!(target: "engine", frames = frames.len(), with_face = n, "engine.template");
        if n == 0 {
            return Vec::new();
        }
        encode(&normalise(sum))
    }

    fn profile(&self, template: &[u8]) -> Vec<u8> {
        fuzz(template)
    }

    fn matcher(&self) -> &dyn Matcher {
        &self.matcher
    }
}

/// The comparison under version 2: cosine between the template and the
/// profile dequantised, read at the thresholds; any other version is
/// not this matcher's to judge.
#[derive(Debug, Clone, Copy)]
pub struct Cosine(pub Thresholds);

impl Matcher for Cosine {
    fn compare(&self, template: &[u8], profile: &[u8], template_version: u64) -> Verdict {
        if template_version != TEMPLATE_VERSION {
            return Verdict::Unavailable;
        }
        let (Some(t), Some(p)) = (decode(template), dequantise(profile)) else {
            // neither is of the length this version fixes: a third road to
            // `inconclusive`, and one a run should not have to guess at
            tracing::warn!(
                target: "engine",
                template = template.len(),
                profile = profile.len(),
                "engine.compare.unreadable"
            );
            return Verdict::Inconclusive;
        };
        let c = cosine(&t, &p);
        let verdict = if c >= self.0.match_at {
            Verdict::Match
        } else if c < self.0.no_match_below {
            Verdict::NoMatch
        } else {
            Verdict::Inconclusive
        };
        // **the number, where a run can read it.** The thresholds of
        // §22.2 are provisional and the author's to set, and a verdict
        // alone says nothing about how far from one it fell: on the bench
        // two version-2 captures of one person answered inconclusive
        // [2026-10-12].  A field build's own records; no subscriber is
        // installed in a releasable one.
        tracing::info!(
            target: "engine",
            score = f64::from(c),
            match_at = f64::from(self.0.match_at),
            no_match_below = f64::from(self.0.no_match_below),
            verdict = ?verdict,
            "engine.compare"
        );
        verdict
    }
}

// ---------------------------------------------------------------- the maths

/// `gray` turned `turns` quarter turns clockwise, with its new geometry.
pub fn rotate(gray: &[u8], w: usize, h: usize, turns: u8) -> (Vec<u8>, usize, usize) {
    if gray.len() < w * h {
        return (Vec::new(), 0, 0);
    }
    let mut out = vec![0u8; w * h];
    match turns % 4 {
        0 => return (gray[..w * h].to_vec(), w, h),
        1 => {
            // 90° clockwise: the new column is the old row, read upward
            for y in 0..h {
                for x in 0..w {
                    out[x * h + (h - 1 - y)] = gray[y * w + x];
                }
            }
            return (out, h, w);
        }
        2 => {
            for y in 0..h {
                for x in 0..w {
                    out[(h - 1 - y) * w + (w - 1 - x)] = gray[y * w + x];
                }
            }
        }
        _ => {
            // 270° clockwise, which is 90° anticlockwise
            for y in 0..h {
                for x in 0..w {
                    out[(w - 1 - x) * h + y] = gray[y * w + x];
                }
            }
            return (out, h, w);
        }
    }
    (out, w, h)
}

/// Boxes that overlap a more confident one past [`NMS_IOU`] are the same
/// face; what remains is sorted most confident first.
fn nms(mut faces: Vec<Face>) -> Vec<Face> {
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Face> = Vec::new();
    for f in faces {
        if kept.iter().all(|k| iou(k, &f) <= NMS_IOU) {
            kept.push(f);
        }
    }
    kept
}

fn iou(a: &Face, b: &Face) -> f32 {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.w).min(b.x + b.w);
    let y2 = (a.y + a.h).min(b.y + b.h);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a.w * a.h + b.w * b.h - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// SFace's reference landmarks on its 112×112 input (the ArcFace
/// alignment OpenCV's `alignCrop` uses).
const REFERENCE: [(f32, f32); 5] = [
    (38.2946, 51.6963),
    (73.5318, 51.5014),
    (56.0252, 71.7366),
    (41.5493, 92.3655),
    (70.7299, 92.2041),
];

/// The similarity transform (scale, rotation, translation) taking `from`
/// to `to` in the least-squares sense (Umeyama), as `[a, b, tx, c, d, ty]`
/// with `x' = a x + b y + tx`, `y' = c x + d y + ty`.
pub fn similarity(from: &[(f32, f32); 5], to: &[(f32, f32); 5]) -> [f32; 6] {
    let n = 5.0f32;
    let (mut mx, mut my, mut ux, mut uy) = (0f32, 0f32, 0f32, 0f32);
    for k in 0..5 {
        mx += from[k].0;
        my += from[k].1;
        ux += to[k].0;
        uy += to[k].1;
    }
    let (mx, my, ux, uy) = (mx / n, my / n, ux / n, uy / n);
    // covariance of the centred points and the source variance
    let (mut sxx, mut sxy, mut syx, mut syy, mut var) = (0f32, 0f32, 0f32, 0f32, 0f32);
    for k in 0..5 {
        let (x, y) = (from[k].0 - mx, from[k].1 - my);
        let (u, v) = (to[k].0 - ux, to[k].1 - uy);
        sxx += u * x;
        sxy += u * y;
        syx += v * x;
        syy += v * y;
        var += x * x + y * y;
    }
    // the rotation that best aligns: for a similarity, the 2×2 is
    // s·[cos −sin; sin cos], solved from the covariance's symmetric and
    // antisymmetric parts
    let cos_part = sxx + syy;
    let sin_part = syx - sxy;
    let norm = (cos_part * cos_part + sin_part * sin_part).sqrt();
    if norm == 0.0 || var == 0.0 {
        return [1.0, 0.0, ux - mx, 0.0, 1.0, uy - my];
    }
    let (cos, sin) = (cos_part / norm, sin_part / norm);
    let scale = norm / var;
    let (a, b, c, d) = (scale * cos, -scale * sin, scale * sin, scale * cos);
    [a, b, ux - (a * mx + b * my), c, d, uy - (c * mx + d * my)]
}

/// The 112×112 crop of `gray` with `landmarks` brought to the reference:
/// the transform is inverted per output pixel and the source sampled
/// bilinearly, outside the frame reading as black.
pub fn align(gray: &[u8], width: usize, height: usize, landmarks: &[(f32, f32); 5]) -> Vec<f32> {
    let m = similarity(landmarks, &REFERENCE);
    // invert the affine [a b tx; c d ty]
    let det = m[0] * m[4] - m[1] * m[3];
    let inv = if det.abs() < 1e-9 {
        [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
    } else {
        let (ia, ib, ic, id) = (m[4] / det, -m[1] / det, -m[3] / det, m[0] / det);
        [
            ia,
            ib,
            -(ia * m[2] + ib * m[5]),
            ic,
            id,
            -(ic * m[2] + id * m[5]),
        ]
    };
    let mut out = vec![0f32; SIDE * SIDE];
    for y in 0..SIDE {
        for x in 0..SIDE {
            let (xf, yf) = (x as f32, y as f32);
            let sx = inv[0] * xf + inv[1] * yf + inv[2];
            let sy = inv[3] * xf + inv[4] * yf + inv[5];
            out[y * SIDE + x] = sample(gray, width, height, sx, sy);
        }
    }
    out
}

fn sample(gray: &[u8], width: usize, height: usize, x: f32, y: f32) -> f32 {
    if x < 0.0 || y < 0.0 || x >= (width - 1) as f32 || y >= (height - 1) as f32 {
        return 0.0;
    }
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |xx: usize, yy: usize| f32::from(gray[yy * width + xx]);
    let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1, y0) * fx;
    let bottom = at(x0, y0 + 1) * (1.0 - fx) + at(x0 + 1, y0 + 1) * fx;
    top * (1.0 - fy) + bottom * fy
}

/// A vector brought to unit length; a zero vector stays zero.
pub fn normalise(mut v: [f32; DIM]) -> [f32; DIM] {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > 0.0 {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
    v
}

/// The cosine of two unit vectors.
pub fn cosine(a: &[f32; DIM], b: &[f32; DIM]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// A template as the capture stores it: 128 × f32, little-endian.
pub fn encode(v: &[f32; DIM]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// A template read back; none unless it is exactly one.
pub fn decode(b: &[u8]) -> Option<[f32; DIM]> {
    if b.len() != TEMPLATE_LEN {
        return None;
    }
    let mut v = [0f32; DIM];
    for (i, c) in b.as_chunks::<4>().0.iter().enumerate() {
        v[i] = f32::from_le_bytes(*c);
    }
    Some(v)
}

/// **The fuzzed profile** (design §7.5): the template quantised to `i8`
/// after seeded noise of [`FUZZ_SIGMA`] per component — specific enough
/// to discriminate, lossy enough that the template is not handed over.
/// The seed is the template's own hash, so one capture yields one
/// profile. An input that is not a template yields nothing.
pub fn fuzz(template: &[u8]) -> Vec<u8> {
    let Some(t) = decode(template) else {
        return Vec::new();
    };
    let seed = rhtn_codec::cose::sha256(template);
    let mut out = Vec::with_capacity(PROFILE_LEN);
    for (i, x) in t.iter().enumerate() {
        // a cheap deterministic gaussian-ish draw: the mean of four
        // uniform bytes of the seed stream, centred and scaled
        let h = rhtn_codec::cose::sha256(&[seed.as_slice(), &(i as u32).to_le_bytes()].concat());
        let u: f32 = h[..4].iter().map(|b| f32::from(*b) / 255.0).sum::<f32>() / 4.0 - 0.5;
        let noisy = x + u * FUZZ_SIGMA * 3.4641; // the mean of four uniforms has sd 1/√48
        out.push((noisy * 127.0).round().clamp(-127.0, 127.0) as i8 as u8);
    }
    out
}

/// A profile back to a unit vector; none unless it is one.
pub fn dequantise(p: &[u8]) -> Option<[f32; DIM]> {
    if p.len() != PROFILE_LEN {
        return None;
    }
    let mut v = [0f32; DIM];
    for (i, b) in p.iter().enumerate() {
        v[i] = f32::from(*b as i8) / 127.0;
    }
    Some(normalise(v))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(seed: u8) -> [f32; DIM] {
        let mut v = [0f32; DIM];
        for (i, x) in v.iter_mut().enumerate() {
            *x = ((i as f32 * 0.37 + f32::from(seed)) * 1.7).sin();
        }
        normalise(v)
    }

    #[test]
    fn a_profile_of_a_template_matches_it_and_another_template_does_not() {
        let t = unit(1);
        let p = fuzz(&encode(&t));
        assert_eq!(p.len(), PROFILE_LEN);
        let m = Cosine(Thresholds::default());
        assert_eq!(m.compare(&encode(&t), &p, TEMPLATE_VERSION), Verdict::Match);
        assert!(
            cosine(&t, &dequantise(&p).unwrap()) > 0.95,
            "lossy, not lost"
        );
        // another template, made orthogonal to this one
        let mut o = unit(9);
        let c = cosine(&t, &o);
        for (x, y) in o.iter_mut().zip(t.iter()) {
            *x -= c * y;
        }
        assert_eq!(
            m.compare(&encode(&normalise(o)), &p, TEMPLATE_VERSION),
            Verdict::NoMatch
        );
        assert_eq!(
            m.compare(&encode(&t), &p, 1),
            Verdict::Unavailable,
            "not this version's"
        );
        assert_eq!(
            m.compare(&encode(&t), &p[..100], TEMPLATE_VERSION),
            Verdict::Inconclusive
        );
        assert_eq!(fuzz(&p), Vec::<u8>::new(), "a profile is not a template");
    }

    /// **The three regions, at the numbers production ships with.** The
    /// pair is a decision [author, 2026-10-13] and not an accident, so a
    /// score either side of each line is held here: a comparison at the
    /// model's published operating point is a match, one below it carries
    /// its doubt as `Inconclusive`, and only a clearly different face
    /// earns the signed negative.
    #[test]
    fn the_chosen_thresholds_read_a_match_a_doubt_and_a_negative() {
        let t = unit(3);
        // a profile at a chosen cosine to t: t mixed with an orthogonal
        // direction, quantised as a profile is
        let at = |c: f32| {
            let mut o = unit(7);
            let d = cosine(&t, &o);
            for (x, y) in o.iter_mut().zip(t.iter()) {
                *x -= d * y;
            }
            let o = normalise(o);
            let mut mix = [0f32; DIM];
            for i in 0..DIM {
                mix[i] = c * t[i] + (1.0 - c * c).sqrt() * o[i];
            }
            normalise(mix)
                .iter()
                .map(|x| (x * 127.0).round() as i8 as u8)
                .collect::<Vec<u8>>()
        };
        let m = Cosine(Thresholds::default());
        let template = encode(&t);
        for (c, want) in [
            (0.42, Verdict::Match),
            (0.37, Verdict::Match),
            (0.30, Verdict::Inconclusive),
            (0.22, Verdict::Inconclusive),
            (0.10, Verdict::NoMatch),
        ] {
            assert_eq!(
                m.compare(&template, &at(c), TEMPLATE_VERSION),
                want,
                "a comparison at {c}"
            );
        }
    }

    #[test]
    fn the_band_between_the_thresholds_is_inconclusive() {
        let m = Cosine(Thresholds {
            match_at: 0.9,
            no_match_below: 0.1,
        });
        let t = unit(2);
        // a vector at a known angle to t: mix with an orthogonal direction
        let mut o = unit(5);
        let c = cosine(&t, &o);
        for (x, y) in o.iter_mut().zip(t.iter()) {
            *x -= c * y;
        }
        let o = normalise(o);
        let mut mid = [0f32; DIM];
        for i in 0..DIM {
            mid[i] = 0.5 * t[i] + 0.866 * o[i]; // cosine ≈ 0.5
        }
        let p: Vec<u8> = normalise(mid)
            .iter()
            .map(|x| (x * 127.0).round() as i8 as u8)
            .collect();
        assert_eq!(
            m.compare(&encode(&t), &p, TEMPLATE_VERSION),
            Verdict::Inconclusive
        );
    }

    #[test]
    fn the_similarity_transform_maps_the_reference_onto_itself_and_scales_a_shifted_copy() {
        let m = similarity(&REFERENCE, &REFERENCE);
        assert!((m[0] - 1.0).abs() < 1e-4 && m[1].abs() < 1e-4 && m[2].abs() < 1e-3);
        let from: [(f32, f32); 5] =
            std::array::from_fn(|k| (REFERENCE[k].0 * 2.0 + 100.0, REFERENCE[k].1 * 2.0 + 50.0));
        let m = similarity(&from, &REFERENCE);
        for k in 0..5 {
            let x = m[0] * from[k].0 + m[1] * from[k].1 + m[2];
            let y = m[3] * from[k].0 + m[4] * from[k].1 + m[5];
            assert!((x - REFERENCE[k].0).abs() < 1e-2 && (y - REFERENCE[k].1).abs() < 1e-2);
        }
    }

    #[test]
    fn an_aligned_crop_reads_the_source_where_the_landmarks_say() {
        // a frame bright at one landmark and dark elsewhere comes out bright
        // at that landmark's reference point
        let (w, h) = (640usize, 480usize);
        let from: [(f32, f32); 5] =
            std::array::from_fn(|k| (REFERENCE[k].0 * 2.0 + 200.0, REFERENCE[k].1 * 2.0 + 100.0));
        let mut gray = vec![0u8; w * h];
        let (nx, ny) = (from[2].0.round() as usize, from[2].1.round() as usize);
        for dy in 0..4 {
            for dx in 0..4 {
                gray[(ny + dy - 2) * w + nx + dx - 2] = 255;
            }
        }
        let crop = align(&gray, w, h, &from);
        let (rx, ry) = (
            REFERENCE[2].0.round() as usize,
            REFERENCE[2].1.round() as usize,
        );
        assert!(
            crop[ry * SIDE + rx] > 100.0,
            "the nose landmark lands on the nose reference"
        );
        assert!(crop[5 * SIDE + 5] < 1.0);
    }

    #[test]
    fn a_quarter_turn_moves_a_pixel_where_it_should_and_four_are_none() {
        let (w, h) = (4usize, 2usize);
        let g: Vec<u8> = (0..8).collect();
        let (r, rw, rh) = rotate(&g, w, h, 1);
        assert_eq!((rw, rh), (2, 4));
        // the top-left pixel of the original is the top-right after 90° cw
        assert_eq!(r[rw - 1], g[0]);
        let (back, bw, bh) = rotate(&r, rw, rh, 3);
        assert_eq!((bw, bh), (w, h));
        assert_eq!(back, g);
        let (once, _, _) = rotate(&g, w, h, 2);
        let (twice, _, _) = rotate(&once, w, h, 2);
        assert_eq!(twice, g);
    }

    #[test]
    fn a_frame_too_short_to_be_one_is_nobody_and_no_panic() {
        assert_eq!(rotate(&[], 640, 480, 1), (Vec::new(), 0, 0));
        assert_eq!(rotate(&[0u8; 10], 640, 480, 0), (Vec::new(), 0, 0));
    }

    #[test]
    fn nms_keeps_the_best_of_overlapping_boxes() {
        let f = |s: f32, x: f32| Face {
            score: s,
            x,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            landmarks: [(0.0, 0.0); 5],
        };
        let kept = nms(vec![f(0.91, 0.0), f(0.99, 1.0), f(0.95, 50.0)]);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].score, 0.99);
        assert_eq!(kept[1].x, 50.0);
    }
}
