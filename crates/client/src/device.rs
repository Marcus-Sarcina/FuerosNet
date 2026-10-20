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
    /// **One frame with nothing asked of the person** (design §7.5): what
    /// [`hold_continuity`] probes with between prompts.  A probe is not a
    /// prompt — the person is shown nothing and asked for nothing — so a
    /// camera states it separately rather than having a prompt stand in
    /// for it [2026-10-18].
    fn probe(&self) -> RawFrame;
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
    /// **Whoever is in front of the lens right now**, as the template one
    /// frame yields, or `None` where no face is there (design §7.5).  It
    /// is what [`hold_continuity`] probes with: deciding that a face is
    /// present rather than a blank wall is a capability the capture needs
    /// anyway.
    fn sample(&self, frame: &Frame) -> Option<Vec<u8>> {
        let t = self.template(std::slice::from_ref(frame));
        (!t.is_empty()).then_some(t)
    }
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
    // `sample` is the default: the stand-in finds a face in anything,
    // which is what lets a ceremony run end to end on a synthetic frame
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

/// **The window a capture is held across** (design §7.5): the figures a
/// party uses to satisfy itself that it met one person continuously.
///
/// **This is each party enforcing its own interest and no part of the
/// record**: nothing about it enters a transaction, no counterparty or
/// witness can tell that it ran, and a client that skips it weakens only
/// the standing of the user who ran it — which is why it is a commitment
/// in `light-client-requirements.md` §1.3 and not a rule anybody issues.
#[derive(Debug, Clone)]
pub struct Continuity {
    /// The window to hold the counterparty across.  **Zero turns the
    /// continuity check off** [author, 2026-10-18], which is the setting
    /// to use while the bench is run often: the capture runs, the loop
    /// still probes and says what it saw, and **nothing is abandoned**.
    /// The reference client's own figure is the four minutes design §7.1
    /// prices a meeting in.
    ///
    /// **Why off rather than re-scaled onto the capture's own span**
    /// [bench, 2026-10-18]: the ratio is design §7.5's figure for four
    /// minutes, where a lapse is a small share of a long window.  Over
    /// ten seconds the same ninety per cent is a different rule — a probe
    /// costs the detector about 430 ms on an S21, so a cycle is near a
    /// second and two probes without a face is already a fifth of the
    /// window.  Two bad probes ending a meeting is not what the figure
    /// means, so a window too short to measure a ratio over measures
    /// none.
    pub duration_ms: u64,
    /// The longest a face may be absent before the meeting is abandoned.
    pub max_gap_ms: u64,
    /// The least share of the window a face must be present for, in
    /// hundredths: the reference figure is ninety per cent.
    pub presence_pct: u64,
    /// How often the loop probes for a face.
    pub probe_ms: u64,
}

impl Default for Continuity {
    /// **The check is off** [author, 2026-10-18]; the ratio and the gap
    /// are design §7.5's reference figures for when it is on, and the
    /// probe interval is this client's own.
    fn default() -> Self {
        Continuity {
            duration_ms: 0,
            max_gap_ms: 20_000,
            presence_pct: 90,
            probe_ms: 500,
        }
    }
}

/// Why a party gave up holding its counterparty (design §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lost {
    /// A face was absent for longer than the maximum gap.
    Gap,
    /// A different face appeared while the one being held was away.
    Another,
    /// The gaps summed past the allowed share of the window.
    Ratio,
    /// No face was ever there.
    None_,
}

/// What a held capture came to.
#[derive(Debug)]
pub enum Held {
    /// The frames retained at the prompts, and the prompts given.
    Kept(Vec<Frame>, Vec<Prompt>),
    /// Abandoned, and why.  **The meeting ends**: a party that could not
    /// hold its counterparty has nothing to attest.
    Lost(Lost),
}

/// **Hold the counterparty across the window, and retain the frames taken
/// at the prompts** (design §7.5).
///
/// One mechanism rather than two: the prompts fall at random through the
/// window and the retained images are the ones taken at them, so the
/// randomness that spreads the archive and the prompts that make the
/// capture a liveness check are the same thing.  Between prompts the loop
/// probes for a face and compares what it finds against the face it is
/// holding — **the same comparison a verifier makes** (§7.3), at its
/// easiest point, so nothing new is pinned for it.
///
/// What is not retained does not survive: a probe's sample is compared and
/// dropped, never stored, which is what keeps a compliant client holding
/// no decryptable likeness it was not given a key for (§7.5.2).
pub fn hold_continuity(
    cam: &dyn Camera,
    clock: &dyn Clock,
    rng: &dyn Random,
    engine: &dyn Engine,
    version: u64,
    p: &CaptureParams,
    c: &Continuity,
) -> Held {
    let n = draw(rng, p.min_frames, p.max_frames);
    // the window: the floor where one is set, and the capture's own span
    // where it is not
    let window = if c.duration_ms == 0 {
        draw(rng, p.min_span_ms, p.max_span_ms)
    } else {
        c.duration_ms
    };
    // the prompts' instants, drawn through the window and taken in order
    let mut at: Vec<u64> = (0..n).map(|_| draw(rng, 0, window)).collect();
    at.sort_unstable();
    // **nothing is abandoned where no window was set**: see
    // [`Continuity::duration_ms`] for why a short window measures no ratio
    let enforced = c.duration_ms != 0;
    let start = clock.now_ms();
    let (mut frames, mut prompts) = (Vec::new(), Vec::new());
    let (mut held, mut next) = (None::<Vec<u8>>, 0usize);
    let (mut away_since, mut away_total) = (None::<u64>, 0u64);
    loop {
        let now = clock.now_ms().saturating_sub(start);
        // a prompt falls here: show it, keep the frame, and let the frame
        // itself serve as this tick's probe
        let prompt = (next < at.len() && now >= at[next]).then(|| {
            next += 1;
            Prompt::ALL[draw(rng, 0, Prompt::ALL.len() as u64 - 1) as usize]
        });
        // prompted where a prompt falls here, and a bare probe otherwise:
        // the person is asked nothing between prompts
        let raw = match prompt {
            Some(p) => cam.capture(p),
            None => cam.probe(),
        };
        let frame = Frame {
            at_ms: now,
            bytes: strip(raw),
        };
        match engine.sample(&frame) {
            Some(seen) => {
                if let Some(h) = &held {
                    // a different face while the one held was away is the
                    // substitution the loop exists to catch
                    if enforced
                        && away_since.is_some()
                        && engine.matcher().compare(h, &engine.profile(&seen), version)
                            == crate::query::Verdict::NoMatch
                    {
                        return Held::Lost(Lost::Another);
                    }
                } else {
                    held = Some(seen);
                }
                if let Some(since) = away_since.take() {
                    away_total += now.saturating_sub(since);
                }
            }
            None => {
                let since = *away_since.get_or_insert(now);
                if enforced && now.saturating_sub(since) > c.max_gap_ms {
                    return Held::Lost(Lost::Gap);
                }
            }
        }
        if let Some(prompt) = prompt {
            prompts.push(prompt);
            frames.push(frame);
        }
        if now >= window {
            break;
        }
        clock.wait_ms(c.probe_ms);
    }
    if let Some(since) = away_since {
        away_total += window.saturating_sub(since);
    }
    if enforced && held.is_none() {
        return Held::Lost(Lost::None_);
    }
    // the gaps, summed, against the share the window allows
    let short = away_total * 100 > window * (100 - c.presence_pct.min(100));
    if enforced && short {
        return Held::Lost(Lost::Ratio);
    }
    // **off, and still said**: a run wants to know what the loop saw even
    // where nothing turns on it, the check being the party's own (§7.5)
    tracing::info!(
        target: "cer",
        enforced,
        window_ms = window,
        away_ms = away_total,
        held = held.is_some(),
        would_abandon = short,
        "cer.continuity"
    );
    Held::Kept(frames, prompts)
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
