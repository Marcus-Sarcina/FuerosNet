//! **Holding the counterparty across the window** (design §7.5): the
//! prompts fall through it, the frames taken at them are what is
//! retained, and the meeting is abandoned where a face was absent too
//! long, where another face appeared while the one held was away, or
//! where the gaps summed past the share the window allows.
//!
//! The four-minute floor is **inactive** by default [author, 2026-10-18]
//! while the bench is run often, so the window here is the capture's own
//! span; a party setting a floor gets the same loop over a longer one.

use rhtn_client::device::{
    Camera, CaptureParams, Clock, Continuity, Engine, Held, Lost, Prompt, Random, RawFrame,
    hold_continuity,
};
use rhtn_client::query::Verdict;
use rhtn_client::store::Frame;
use rhtn_client::verifier::Matcher;
use std::cell::Cell;

/// A clock that only moves when the loop waits, so a test states the
/// window in probes rather than in wall time.
struct Ticks(Cell<u64>);
impl Clock for Ticks {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
    fn wait_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}

/// A camera whose frame says who is in front of it: one byte, which the
/// engine below reads as an identity.  `0` is nobody.
struct Who(Vec<u8>, Cell<usize>);
impl Who {
    fn next(&self) -> RawFrame {
        let i = self.1.get();
        self.1.set(i + 1);
        RawFrame {
            pixels: vec![*self.0.get(i).unwrap_or(self.0.last().unwrap())],
            metadata: Default::default(),
        }
    }
}
impl Camera for Who {
    fn capture(&self, _: Prompt) -> RawFrame {
        self.next()
    }
    fn probe(&self) -> RawFrame {
        self.next()
    }
}

/// An engine that reads the frame's one byte as the face: no face where
/// it is zero, and two samples match when the bytes agree.
struct ByByte;
impl Matcher for ByByte {
    fn compare(&self, template: &[u8], profile: &[u8], _: u64) -> Verdict {
        if template == profile {
            Verdict::Match
        } else {
            Verdict::NoMatch
        }
    }
}
impl Engine for ByByte {
    fn template(&self, frames: &[Frame]) -> Vec<u8> {
        frames.first().map(|f| f.bytes.clone()).unwrap_or_default()
    }
    fn sample(&self, frame: &Frame) -> Option<Vec<u8>> {
        (frame.bytes.first().copied().unwrap_or(0) != 0).then(|| frame.bytes.clone())
    }
    fn profile(&self, template: &[u8]) -> Vec<u8> {
        template.to_vec()
    }
    fn matcher(&self) -> &dyn Matcher {
        self
    }
}

/// A source fixed at one byte, so a test's draws are reproducible.
/// **Every draw from it is the same value**, which is what a test wants
/// when it cares about the face and not the instants.
struct Fixed(u8);
impl Random for Fixed {
    fn fill(&self, out: &mut [u8]) {
        out.fill(self.0);
    }
}

/// A reproducible source that actually varies: the hash of a counter,
/// which spreads where a counter alone would march. A test about where
/// the prompts fall needs draws that differ within one run.
struct Spread(Cell<u64>, u64);
impl Random for Spread {
    fn fill(&self, out: &mut [u8]) {
        let n = self.0.get();
        self.0.set(n + 1);
        let h = rhtn_codec::cose::sha256(&[n.to_be_bytes(), self.1.to_be_bytes()].concat());
        for (i, b) in out.iter_mut().enumerate() {
            *b = h[i % 32];
        }
    }
}

fn params() -> CaptureParams {
    CaptureParams {
        min_frames: 3,
        max_frames: 5,
        min_span_ms: 10_000,
        max_span_ms: 10_000,
    }
}

/// A window long enough for the rules to be measured over: zero turns the
/// check off, which is the bench's setting and not what these assert.
fn enforcing() -> Continuity {
    Continuity {
        duration_ms: 10_000,
        ..Continuity::default()
    }
}

fn run(faces: Vec<u8>, c: &Continuity) -> Held {
    hold_continuity(
        &Who(faces, Cell::new(0)),
        &Ticks(Cell::new(0)),
        &Fixed(1),
        &ByByte,
        1,
        &params(),
        c,
    )
}

// acceptance: CER-66
#[test]
fn one_face_across_the_window_is_kept_and_a_long_gap_is_not() {
    let c = enforcing();
    // present throughout: the frames taken at the prompts are retained
    match run(vec![7], &c) {
        Held::Kept(frames, prompts) => {
            assert_eq!(frames.len(), prompts.len(), "one frame per prompt");
            assert!((3..=5).contains(&frames.len()), "{} frames", frames.len());
            assert!(
                frames.windows(2).all(|w| w[0].at_ms <= w[1].at_ms),
                "in the order they were taken"
            );
            assert!(frames.iter().all(|f| f.bytes == vec![7]));
        }
        Held::Lost(why) => panic!("held nobody: {why:?}"),
    }
    // nobody at all
    assert!(matches!(run(vec![0], &c), Held::Lost(Lost::None_)));
    // **with the floor inactive the gap rule is vacuous**, and that is
    // worth stating rather than discovering: the window is then the
    // capture's own ten to fifteen seconds and the reference maximum gap
    // is twenty, so no gap inside it can exceed the maximum — what bites
    // on the bench is the ratio below
    assert!(c.max_gap_ms > c.duration_ms);
    let mut faces = vec![7];
    faces.extend(std::iter::repeat_n(0u8, 60));
    assert!(
        matches!(run(faces.clone(), &c), Held::Lost(Lost::Ratio)),
        "the ratio catches it where the gap cannot"
    );
    // a gap shorter than the window does refuse: the loop's rule is
    // sound, and a party setting the floor gets it
    let tight = Continuity {
        max_gap_ms: 2_000,
        ..enforcing()
    };
    assert!(matches!(run(faces, &tight), Held::Lost(Lost::Gap)));
}

// acceptance: CER-66
#[test]
fn another_face_while_the_one_held_was_away_ends_the_meeting() {
    let c = enforcing();
    // held 7, away, then 9 appears: a substitution, which is the case the
    // loop exists to catch
    let mut faces = vec![7, 7];
    faces.extend(std::iter::repeat_n(0u8, 4));
    faces.push(9);
    faces.extend(std::iter::repeat_n(9u8, 40));
    assert!(matches!(run(faces, &c), Held::Lost(Lost::Another)));
}

// acceptance: CER-66
#[test]
fn the_gaps_summing_past_the_allowed_share_ends_it_too() {
    // a tighter share than the reference, so short gaps are enough to
    // exceed it without any single one passing the maximum
    let c = Continuity {
        presence_pct: 95,
        ..enforcing()
    };
    let mut faces = Vec::new();
    for _ in 0..12 {
        faces.push(7u8);
        faces.extend([0u8, 0]);
    }
    faces.extend(std::iter::repeat_n(7u8, 40));
    assert!(matches!(run(faces, &c), Held::Lost(Lost::Ratio)));
}

/// **A floor lengthens the window and changes nothing else** — which is
/// what leaving the four minutes inactive means: the same loop, over the
/// span the capture takes.
#[test]
fn a_set_floor_holds_the_same_face_over_a_longer_window() {
    let c = Continuity {
        duration_ms: 240_000,
        ..Continuity::default()
    };
    match run(vec![7], &c) {
        Held::Kept(frames, _) => {
            assert!((3..=5).contains(&frames.len()));
            assert!(
                frames.iter().any(|f| f.at_ms > 15_000),
                "the prompts fall through the four minutes, not the first fifteen seconds"
            );
        }
        Held::Lost(why) => panic!("held nobody over the floor: {why:?}"),
    }
}

/// **The prompts fall through the window rather than clustering** (design
/// §7.5): held over many draws, because one capture's three to five
/// instants are a sample and a single run proves nothing about spread.
/// What a burst would look like is all of them in one corner.
#[test]
fn the_prompts_spread_through_the_window_over_many_draws() {
    let c = Continuity::default();
    let p = params();
    let (mut total, mut runs, mut last_fifth) = (0f64, 0u32, 0u32);
    for b in 1u8..=200u8 {
        let held = hold_continuity(
            &Who(vec![7], Cell::new(0)),
            &Ticks(Cell::new(0)),
            &Spread(Cell::new(0), u64::from(b)),
            &ByByte,
            1,
            &p,
            &c,
        );
        let Held::Kept(frames, _) = held else {
            panic!("held nobody at source {b}")
        };
        let window = p.max_span_ms as f64;
        for f in &frames {
            total += f.at_ms as f64 / window;
            runs += 1;
        }
        if frames.iter().all(|f| f.at_ms as f64 > window * 0.8) {
            last_fifth += 1;
        }
    }
    let mean = total / f64::from(runs);
    assert!(
        (0.25..=0.75).contains(&mean),
        "the instants sit at {mean:.2} of the window on average, which is a corner"
    );
    assert!(
        last_fifth * 5 < 200,
        "{last_fifth} of 200 captures came wholly from the window's last fifth"
    );
}

/// **With no window set the check is off, and the capture still happens**
/// [author, 2026-10-18]. This is the bench's setting: a run that loses the
/// face for most of a short capture gets its frames anyway, because the
/// ratio design §7.5 states is a figure for four minutes and a ten-second
/// window measures nothing like it. Two probes without a face is a fifth
/// of such a window, and two probes is what a real detector costs.
// acceptance: CER-66
#[test]
fn a_window_of_zero_abandons_nothing() {
    let off = Continuity::default();
    assert_eq!(off.duration_ms, 0, "the floor is inactive by default");
    // away for most of it, a second face, and nobody at all: each would
    // have ended the meeting under an enforced window
    let mostly_away = {
        let mut v = vec![7u8];
        v.extend(std::iter::repeat_n(0u8, 60));
        v
    };
    for faces in [mostly_away, vec![7, 0, 0, 9, 9, 9, 9, 9], vec![0]] {
        match run(faces.clone(), &off) {
            Held::Kept(frames, prompts) => {
                assert_eq!(frames.len(), prompts.len());
                assert!((3..=5).contains(&frames.len()), "{} frames", frames.len());
            }
            Held::Lost(why) => panic!("abandoned with the check off: {why:?} on {faces:?}"),
        }
    }
    // and the same inputs under a window do end it, so the rules are
    // gated and not deleted
    assert!(matches!(
        run(vec![0], &enforcing()),
        Held::Lost(Lost::None_)
    ));
}
