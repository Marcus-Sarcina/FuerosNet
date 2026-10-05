//! The device behind the client (`Robot/implementation-plan.md`: device I/O
//! behind an interface): proximity channels, the camera, the clock, the
//! random source, the person, and the biometric engine.  Each is a trait
//! so the same client runs on a harness, and the decisions this module
//! makes — which channel to report, how a capture is guided, what is
//! stripped — are the client's, not the hardware's.

use crate::Keyhash;
use crate::notice::Notifier;
use crate::query::Verdict;
use crate::store::Frame;
use crate::verifier::{ByteEquality, Matcher};
use rhtn_codec::cose::sha256;
use std::collections::BTreeMap;
use std::rc::Rc;

/// A proximity channel, ranked strongest first (design §7.6.3): the code
/// is `wire-format.md` §4.5's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChannelKind {
    /// Ultra-wideband ranging.
    Uwb = 1,
    /// A near-field tap.
    Nfc = 2,
    /// The optical exchange.
    Optical = 3,
    /// Round-trip latency, which is the weakest.
    Latency = 4,
}

impl ChannelKind {
    /// Strongest first.
    pub const RANKED: [ChannelKind; 4] = [
        ChannelKind::Uwb,
        ChannelKind::Nfc,
        ChannelKind::Optical,
        ChannelKind::Latency,
    ];

    /// The code `wire-format.md` §4.5 gives this channel.
    pub fn code(self) -> u64 {
        self as u64
    }

    /// The channel `c` names, or nothing where it names none.
    pub fn from_code(c: u64) -> Option<Self> {
        Self::RANKED.into_iter().find(|k| k.code() == c)
    }
}

/// What one channel came to (`wire-format.md` §4.5 `Channel` field 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelResult {
    /// The channel ran and passed.  What passing means is the channel's
    /// own; no document defines it further, and `strongest` is the
    /// highest-ranked channel with this result (`wire-format.md` §4.5).
    Pass = 0,
    /// It ran and did not pass.
    Fail = 1,
    /// It did not run.
    Unavailable = 2,
}

/// One channel run: what it was, how it went, and the resolution it
/// claims where it claims one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelOutcome {
    /// Which channel ran.
    pub kind: ChannelKind,
    /// How it went.
    pub result: ChannelResult,
    /// The resolution it claims, where it claims one.
    pub resolution_m: Option<u64>,
}

/// The proximity hardware: which channels it has, and running one with a
/// counterparty's device.
pub trait Proximity {
    /// The channels this hardware has.
    fn supported(&self) -> Vec<ChannelKind>;
    /// Run `kind` against `peer` and say how it went.
    fn run(&self, kind: ChannelKind, peer: &Keyhash) -> ChannelOutcome;
}

/// Run every channel the hardware supports, strongest first, and report
/// what was achieved: the outcomes as observed, and the strongest that
/// passed.  A channel the hardware lacks is not listed, and nothing is
/// promoted (`light-client-requirements.md` §1.3).
pub fn run_channels(
    p: &dyn Proximity,
    peer: &Keyhash,
) -> (Vec<ChannelOutcome>, Option<ChannelKind>) {
    let supported = p.supported();
    let mut outcomes = Vec::new();
    let mut strongest = None;
    for kind in ChannelKind::RANKED {
        if !supported.contains(&kind) {
            continue;
        }
        let o = p.run(kind, peer);
        assert_eq!(
            o.kind, kind,
            "the hardware reports the channel it was asked to run"
        );
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
    /// Turn to the left.
    TurnLeft,
    /// Turn to the right.
    TurnRight,
    /// Look up.
    LookUp,
    /// Look down.
    LookDown,
    /// Smile.
    Smile,
    /// A neutral expression.
    Neutral,
    /// Blink.
    Blink,
    /// Come closer to the camera.
    Closer,
}

impl Prompt {
    /// Every prompt, in the order a guided capture draws from.
    pub const ALL: [Prompt; 8] = [
        Prompt::TurnLeft,
        Prompt::TurnRight,
        Prompt::LookUp,
        Prompt::LookDown,
        Prompt::Smile,
        Prompt::Neutral,
        Prompt::Blink,
        Prompt::Closer,
    ];
}

/// A frame as the camera pipeline hands it over: the pixels, and whatever
/// the pipeline attached to them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    /// The pixels.
    pub pixels: Vec<u8>,
    /// Whatever the pipeline attached to them, which [`strip`] drops.
    pub metadata: BTreeMap<String, Vec<u8>>,
}

/// Strip a frame to its pixels (`light-client-requirements.md` §1.3):
/// nothing the pipeline attached survives, whatever the pipeline's default.
pub fn strip(raw: RawFrame) -> Vec<u8> {
    raw.pixels
}

/// The camera, which returns one frame per prompt.
pub trait Camera {
    /// One frame, taken while `prompt` was being shown.
    fn capture(&self, prompt: Prompt) -> RawFrame;
}

/// The clock.  What it reports is what the ceremony reads.
pub trait Clock {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;
    /// Sleep for `ms` milliseconds.
    fn wait_ms(&self, ms: u64);
}

/// The random source.
pub trait Random {
    /// Fill `out` with random bytes.
    fn fill(&self, out: &mut [u8]);
}

/// The person operating the client: the one party a client may ask a
/// question.  Witnessing, querying and answering ask none (design
/// Appendix A.3).
pub trait Operator {
    /// Put `question` to the person; their yes or no.
    fn ask(&self, question: &str) -> bool;
}

/// The biometric engine (design §22.2, undecided): a template from frames,
/// fixed in length per modality version; the fuzzed profile a query
/// carries; and the comparison.
pub trait Engine {
    /// The template `frames` yield, of this modality version's fixed
    /// length.
    fn template(&self, frames: &[Frame]) -> Vec<u8>;
    /// The fuzzed profile a query carries, derived from `template`.
    fn profile(&self, template: &[u8]) -> Vec<u8>;
    /// The comparison this engine does.
    fn matcher(&self) -> &dyn Matcher;
}

/// The reference stand-in for an engine: the template is the hash of the
/// first frame's leading bytes, the profile is the template, and a
/// comparison is byte equality.  It exists so the ceremony runs end to
/// end; it recognises nobody.
pub struct HashEngine {
    /// How many leading bytes of the first frame the template hashes.
    pub face_bytes: usize,
    matcher: ByteEquality,
}

impl HashEngine {
    /// An engine hashing `face_bytes` leading bytes of the first frame.
    pub fn new(face_bytes: usize) -> Self {
        HashEngine {
            face_bytes,
            matcher: ByteEquality,
        }
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
    /// The fewest frames a capture takes.
    pub min_frames: u64,
    /// The most.
    pub max_frames: u64,
    /// The shortest span the frames are taken over.
    pub min_span_ms: u64,
    /// The longest.
    pub max_span_ms: u64,
}

impl Default for CaptureParams {
    fn default() -> Self {
        CaptureParams {
            min_frames: 3,
            max_frames: 5,
            min_span_ms: 10_000,
            max_span_ms: 15_000,
        }
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
pub fn guided_capture(
    cam: &dyn Camera,
    clock: &dyn Clock,
    rng: &dyn Random,
    p: &CaptureParams,
) -> (Vec<Frame>, Vec<Prompt>) {
    let n = draw(rng, p.min_frames, p.max_frames);
    let span = draw(rng, p.min_span_ms, p.max_span_ms);
    let start = clock.now_ms();
    let mut frames = Vec::new();
    let mut prompts = Vec::new();
    for i in 0..n {
        let prompt = Prompt::ALL[draw(rng, 0, Prompt::ALL.len() as u64 - 1) as usize];
        let raw = cam.capture(prompt);
        frames.push(Frame {
            at_ms: clock.now_ms() - start,
            bytes: strip(raw),
        });
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
    /// Whether `peer` can be reached without the relay.
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
    /// The proximity hardware.
    pub proximity: Rc<dyn Proximity>,
    /// The camera.
    pub camera: Rc<dyn Camera>,
    /// The clock.
    pub clock: Rc<dyn Clock>,
    /// The random source.
    pub random: Rc<dyn Random>,
    /// The person.
    pub operator: Rc<dyn Operator>,
    /// Where notices go.
    pub notifier: Rc<dyn Notifier>,
    /// The biometric engine.
    pub engine: Rc<dyn Engine>,
    /// Whether a peer is directly reachable.
    pub direct: Rc<dyn DirectPath>,
}

/// A verdict is what an engine returns; re-exported so a harness engine
/// need not reach into the query module.
pub type EngineVerdict = Verdict;
