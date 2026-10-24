//! The device behind the client (`Robot/implementation-plan.md`: device I/O
//! behind an interface): the camera, the clock, the random source, the
//! person, and the biometric engine.  Each is a trait
//! so the same client runs on a harness, and the decisions this module
//! makes — how a capture is guided, what is stripped — are the client's,
//! not the hardware's.

use crate::Keyhash;
use crate::notice::Notifier;
use crate::query::Verdict;
use crate::store::Frame;
use crate::verifier::{ByteEquality, Matcher};
use rhtn_codec::cose::sha256;
use std::collections::BTreeMap;
use std::rc::Rc;

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

/// **The hold, as a machine the caller turns** (design §7.5, §7.1 item 5):
/// the capture runs for the ceremony's window *concurrently* with the
/// verification, so it cannot be one call that holds the client's thread
/// for the window.  [`Hold::begin`] plans the prompts, each [`Hold::tick`]
/// takes one frame — prompted where a prompt falls now, a bare probe
/// otherwise — and says when the next is due, and [`Hold::finish`] judges
/// what the window saw.  [`hold_continuity`] is the same machine turned to
/// completion against the device's clock, for a caller with nothing else
/// to do meanwhile.
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
pub struct Hold {
    at: Vec<u64>,
    next: usize,
    window: u64,
    enforced: bool,
    start: u64,
    probe_ms: u64,
    max_gap_ms: u64,
    presence_pct: u64,
    version: u64,
    frames: Vec<Frame>,
    prompts: Vec<Prompt>,
    held: Option<Vec<u8>>,
    away_since: Option<u64>,
    away_total: u64,
    done: bool,
    seen_now: bool,
}

/// What one turn of the hold came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// The window is still open.
    Continue {
        /// When the next frame is due, in milliseconds.
        next_ms: u64,
    },
    /// The window has elapsed: [`Hold::finish`] is what remains.
    Done,
    /// Abandoned, and why.  **The meeting ends**: a party that could not
    /// hold its counterparty has nothing to attest.
    Lost(Lost),
}

impl Hold {
    /// Plan the hold: the prompt count and their instants, drawn through
    /// the window — the floor where one is set, `window_ms` where the
    /// caller names the ceremony's own, and the capture's drawn span
    /// otherwise.  Nothing is taken until the first [`Hold::tick`].
    pub fn begin(
        clock: &dyn Clock,
        rng: &dyn Random,
        version: u64,
        p: &CaptureParams,
        c: &Continuity,
        window_ms: Option<u64>,
    ) -> Hold {
        let n = draw(rng, p.min_frames, p.max_frames);
        let window = if c.duration_ms != 0 {
            c.duration_ms
        } else {
            window_ms
                .filter(|w| *w != 0)
                .unwrap_or_else(|| draw(rng, p.min_span_ms, p.max_span_ms))
        };
        let mut at: Vec<u64> = (0..n).map(|_| draw(rng, 0, window)).collect();
        at.sort_unstable();
        Hold {
            at,
            next: 0,
            window,
            // **nothing is abandoned where no window was set**: see
            // [`Continuity::duration_ms`] for why a short window measures
            // no ratio
            enforced: c.duration_ms != 0,
            start: clock.now_ms(),
            probe_ms: c.probe_ms,
            max_gap_ms: c.max_gap_ms,
            presence_pct: c.presence_pct,
            version,
            frames: Vec::new(),
            prompts: Vec::new(),
            held: None,
            away_since: None,
            away_total: 0,
            done: false,
            seen_now: false,
        }
    }

    /// The frames retained so far, at the prompts that have fallen: what
    /// yields a template before the window is out (design §7.1 item 6).
    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Whether the last tick's frame held a face.
    pub fn seen_now(&self) -> bool {
        self.seen_now
    }

    /// How long a face has been absent in all, in milliseconds, so far.
    pub fn away_ms(&self, clock: &dyn Clock) -> u64 {
        let now = clock.now_ms().saturating_sub(self.start);
        self.away_total + self.away_since.map_or(0, |s| now.saturating_sub(s))
    }

    /// How long the hold has run, in milliseconds of the device's clock.
    pub fn elapsed_ms(&self, clock: &dyn Clock) -> u64 {
        clock.now_ms().saturating_sub(self.start)
    }

    /// One turn: a prompt where one falls now, a probe otherwise, judged
    /// against the face being held.  After [`Tick::Done`] it is a no-op
    /// answering `Done`.
    pub fn tick(
        &mut self,
        cam: &dyn Camera,
        clock: &dyn Clock,
        rng: &dyn Random,
        engine: &dyn Engine,
    ) -> Tick {
        if self.done {
            return Tick::Done;
        }
        let now = clock.now_ms().saturating_sub(self.start);
        // a prompt falls here: show it, keep the frame, and let the frame
        // itself serve as this tick's probe
        let prompt = (self.next < self.at.len() && now >= self.at[self.next]).then(|| {
            self.next += 1;
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
        let sample = engine.sample(&frame);
        // **every tick is said** [bench, 2026-10-24]: a window that sealed
        // nothing could not say whether the camera gave frames, whether a
        // face was in them, or when the first prompt fell
        tracing::info!(
            target: "cer",
            at_ms = now,
            prompted = prompt.is_some(),
            bytes = frame.bytes.len(),
            seen = sample.is_some(),
            "cer.hold.tick"
        );
        self.seen_now = sample.is_some();
        match sample {
            Some(seen) => {
                if let Some(h) = &self.held {
                    // a different face while the one held was away is the
                    // substitution the loop exists to catch
                    if self.enforced
                        && self.away_since.is_some()
                        && engine
                            .matcher()
                            .compare(h, &engine.profile(&seen), self.version)
                            == crate::query::Verdict::NoMatch
                    {
                        self.done = true;
                        return Tick::Lost(Lost::Another);
                    }
                } else {
                    self.held = Some(seen);
                }
                if let Some(since) = self.away_since.take() {
                    self.away_total += now.saturating_sub(since);
                }
            }
            None => {
                let since = *self.away_since.get_or_insert(now);
                if self.enforced && now.saturating_sub(since) > self.max_gap_ms {
                    self.done = true;
                    return Tick::Lost(Lost::Gap);
                }
            }
        }
        if let Some(prompt) = prompt {
            self.prompts.push(prompt);
            self.frames.push(frame);
        }
        if now >= self.window {
            self.done = true;
            return Tick::Done;
        }
        // on the probe's cadence: a prompt that falls between two probes is
        // taken at the next, as the one-call form always took it
        Tick::Continue {
            next_ms: self.probe_ms,
        }
    }

    /// Judge the window once it has elapsed: the gaps summed against the
    /// share the window allows, and the frames kept at the prompts.
    pub fn finish(mut self) -> Held {
        if let Some(since) = self.away_since.take() {
            self.away_total += self.window.saturating_sub(since);
        }
        if self.enforced && self.held.is_none() {
            return Held::Lost(Lost::None_);
        }
        // the gaps, summed, against the share the window allows
        let short = self.away_total * 100 > self.window * (100 - self.presence_pct.min(100));
        if self.enforced && short {
            return Held::Lost(Lost::Ratio);
        }
        // **off, and still said**: a run wants to know what the loop saw
        // even where nothing turns on it, the check being the party's own
        // (§7.5)
        tracing::info!(
            target: "cer",
            enforced = self.enforced,
            window_ms = self.window,
            away_ms = self.away_total,
            held = self.held.is_some(),
            would_abandon = short,
            "cer.continuity"
        );
        Held::Kept(self.frames, self.prompts)
    }
}

/// **Hold the counterparty across the window, and retain the frames taken
/// at the prompts** (design §7.5): the [`Hold`] turned to completion on
/// this thread, waiting on the device's clock between turns.
pub fn hold_continuity(
    cam: &dyn Camera,
    clock: &dyn Clock,
    rng: &dyn Random,
    engine: &dyn Engine,
    version: u64,
    p: &CaptureParams,
    c: &Continuity,
) -> Held {
    let mut hold = Hold::begin(clock, rng, version, p, c, None);
    loop {
        match hold.tick(cam, clock, rng, engine) {
            Tick::Done => return hold.finish(),
            Tick::Lost(why) => return Held::Lost(why),
            Tick::Continue { next_ms } => clock.wait_ms(next_ms),
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
