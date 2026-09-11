//! The device behind the client (`Robot/implementation-plan.md`: device I/O
//! behind an interface): proximity channels, the camera, the clock, the
//! random source, the person, and the biometric engine.  Each is a trait
//! so the same client runs on a harness, and the decisions this module
//! makes — which channel to report, how a capture is guided, what is
//! stripped — are the client's, not the hardware's.

use crate::notice::Notifier;
use crate::query::Verdict;
use crate::store::Frame;
use crate::verifier::{ByteEquality, Matcher};
use crate::Keyhash;
use rhtn_codec::cose::sha256;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A proximity channel, ranked strongest first (design §7.6.3): the code
/// is `wire-format.md` §4.5's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelKind {
    Uwb = 1,
    Nfc = 2,
    Optical = 3,
    Latency = 4,
}

impl ChannelKind {
    /// Strongest first.
    pub const RANKED: [ChannelKind; 4] = [ChannelKind::Uwb, ChannelKind::Nfc, ChannelKind::Optical, ChannelKind::Latency];

    pub fn code(self) -> u64 {
        self as u64
    }

    pub fn from_code(c: u64) -> Option<Self> {
        Self::RANKED.into_iter().find(|k| k.code() == c)
    }
}

/// What one channel came to (`wire-format.md` §4.5 `Channel` field 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelResult {
    Pass = 0,
    Fail = 1,
    Unavailable = 2,
}

/// One channel run: what it was, how it went, and the resolution it
/// claims where it claims one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelOutcome {
    pub kind: ChannelKind,
    pub result: ChannelResult,
    pub resolution_m: Option<u64>,
}

/// The proximity hardware: which channels it has, and running one with a
/// counterparty's device.
pub trait Proximity {
    fn supported(&self) -> Vec<ChannelKind>;
    fn run(&self, kind: ChannelKind, peer: &Keyhash) -> ChannelOutcome;
}

/// Run every channel the hardware supports, strongest first, and report
/// what was achieved: the outcomes as observed, and the strongest that
/// passed.  A channel the hardware lacks is not listed, and nothing is
/// promoted (`light-client-requirements.md` §1.3).
pub fn run_channels(p: &dyn Proximity, peer: &Keyhash) -> (Vec<ChannelOutcome>, Option<ChannelKind>) {
    let supported = p.supported();
    let mut outcomes = Vec::new();
    let mut strongest = None;
    for kind in ChannelKind::RANKED {
        if !supported.contains(&kind) {
            continue;
        }
        let o = p.run(kind, peer);
        assert_eq!(o.kind, kind, "the hardware reports the channel it was asked to run");
        if o.result == ChannelResult::Pass && strongest.is_none() {
            strongest = Some(kind);
        }
        outcomes.push(o);
    }
    (outcomes, strongest)
}

/// A prompt the guided capture gives the counterparty (design §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompt {
    TurnLeft,
    TurnRight,
    LookUp,
    LookDown,
    Smile,
    Neutral,
    Blink,
    Closer,
}

impl Prompt {
    pub const ALL: [Prompt; 8] = [Prompt::TurnLeft, Prompt::TurnRight, Prompt::LookUp, Prompt::LookDown, Prompt::Smile, Prompt::Neutral, Prompt::Blink, Prompt::Closer];
}

/// A frame as the camera pipeline hands it over: the pixels, and whatever
/// the pipeline attached to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    pub pixels: Vec<u8>,
    pub metadata: BTreeMap<String, Vec<u8>>,
}

/// Strip a frame to its pixels (`light-client-requirements.md` §1.3):
/// nothing the pipeline attached survives, whatever the pipeline's default.
pub fn strip(raw: RawFrame) -> Vec<u8> {
    raw.pixels
}

pub trait Camera {
    fn capture(&self, prompt: Prompt) -> RawFrame;
}

pub trait Clock {
    fn now_ms(&self) -> u64;
    fn wait_ms(&self, ms: u64);
}

pub trait Random {
    fn fill(&self, out: &mut [u8]);
}

/// The person operating the client: the one party a client may ask a
/// question.  Witnessing, querying and answering ask none (design
/// Appendix A.3).
pub trait Operator {
    fn ask(&self, question: &str) -> bool;
}

/// The biometric engine (design §22.2, undecided): a template from frames,
/// fixed in length per modality version; the fuzzed profile a query
/// carries; and the comparison.
pub trait Engine {
    fn template(&self, frames: &[Frame]) -> Vec<u8>;
    fn profile(&self, template: &[u8]) -> Vec<u8>;
    fn matcher(&self) -> &dyn Matcher;
}

/// The reference stand-in for an engine: the template is the hash of the
/// first frame's leading bytes, the profile is the template, and a
/// comparison is byte equality.  It exists so the ceremony runs end to
/// end; it recognises nobody.
pub struct HashEngine {
    pub face_bytes: usize,
    matcher: ByteEquality,
}

impl HashEngine {
    pub fn new(face_bytes: usize) -> Self {
        HashEngine { face_bytes, matcher: ByteEquality }
    }
}

impl Engine for HashEngine {
    fn template(&self, frames: &[Frame]) -> Vec<u8> {
        let first = frames.first().map(|f| f.bytes.as_slice()).unwrap_or(&[]);
        sha256(&first[..first.len().min(self.face_bytes)]).to_vec()
    }

    fn profile(&self, template: &[u8]) -> Vec<u8> {
        template.to_vec()
    }

    fn matcher(&self) -> &dyn Matcher {
        &self.matcher
    }
}

/// The guided capture's figures (design §7.5, §21: chosen values).
#[derive(Debug, Clone)]
pub struct CaptureParams {
    pub min_frames: u64,
    pub max_frames: u64,
    pub min_span_ms: u64,
    pub max_span_ms: u64,
}

impl Default for CaptureParams {
    fn default() -> Self {
        CaptureParams { min_frames: 3, max_frames: 5, min_span_ms: 10_000, max_span_ms: 15_000 }
    }
}

fn draw(rng: &dyn Random, lo: u64, hi: u64) -> u64 {
    let mut b = [0u8; 8];
    rng.fill(&mut b);
    lo + u64::from_be_bytes(b) % (hi - lo + 1)
}

/// The guided capture (design §7.5; `light-client-requirements.md` §1.3):
/// between three and five frames over ten to fifteen seconds, each under a
/// prompt drawn at random, each stripped to its pixels before anything
/// else touches it.  Returns the frames and the prompts given.
pub fn guided_capture(cam: &dyn Camera, clock: &dyn Clock, rng: &dyn Random, p: &CaptureParams) -> (Vec<Frame>, Vec<Prompt>) {
    let n = draw(rng, p.min_frames, p.max_frames);
    let span = draw(rng, p.min_span_ms, p.max_span_ms);
    let start = clock.now_ms();
    let mut frames = Vec::new();
    let mut prompts = Vec::new();
    for i in 0..n {
        let prompt = Prompt::ALL[draw(rng, 0, Prompt::ALL.len() as u64 - 1) as usize];
        let raw = cam.capture(prompt);
        frames.push(Frame { at_ms: clock.now_ms() - start, bytes: strip(raw) });
        prompts.push(prompt);
        if i + 1 < n {
            clock.wait_ms(span / (n - 1));
        }
    }
    (frames, prompts)
}

/// The direct payload path (design §14.1.1, §12.6.3): whether a peer can
/// be reached without the relay.  Traversal is the transport's; what the
/// client decides is only which route a message takes.
pub trait DirectPath {
    fn reachable(&self, peer: &Keyhash) -> bool;
}

/// No direct path to anyone: every payload is relayed.
pub struct NoDirectPath;

impl DirectPath for NoDirectPath {
    fn reachable(&self, _: &Keyhash) -> bool {
        false
    }
}

/// Everything a client reaches the world through.
pub struct Device {
    pub proximity: Rc<dyn Proximity>,
    pub camera: Rc<dyn Camera>,
    pub clock: Rc<dyn Clock>,
    pub random: Rc<dyn Random>,
    pub operator: Rc<dyn Operator>,
    pub notifier: Rc<dyn Notifier>,
    pub engine: Rc<dyn Engine>,
    pub direct: Rc<dyn DirectPath>,
}

/// A verdict is what an engine returns; re-exported so a harness engine
/// need not reach into the query module.
pub type EngineVerdict = Verdict;
