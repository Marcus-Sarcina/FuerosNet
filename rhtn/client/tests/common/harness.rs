//! The ceremony harness's devices: proximity hardware with a fixed set of
//! channels, a camera that photographs whoever the harness put in front of
//! it and attaches the metadata a pipeline would, a shared clock, seeded
//! randomness, an operator who says yes and remembers being asked, and a
//! notification hook that records every notice with the time.

#![allow(dead_code)]

use super::*;
use rhtn_client::ceremony::*;
use rhtn_client::device::*;
use rhtn_client::notice::{Notice, Notifier};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Proximity hardware with a fixed set of channels, all of which pass.
pub struct Channels(pub Vec<ChannelKind>);
impl Proximity for Channels {
    fn supported(&self) -> Vec<ChannelKind> {
        self.0.clone()
    }
    fn run(&self, kind: ChannelKind, _peer: &[u8; 32]) -> ChannelOutcome {
        ChannelOutcome { kind, result: ChannelResult::Pass, resolution_m: if kind == ChannelKind::Uwb { Some(1) } else { None } }
    }
}

/// A camera whose frames start with the face in front of it — whoever the
/// harness put there — and carry the metadata a pipeline attaches:
/// location, time, device.  It remembers every prompt it was given and
/// when.
pub struct Cam {
    pub facing: RefCell<Vec<u8>>,
    pub clock: Rc<Cell<u64>>,
    pub captures: RefCell<Vec<(u64, Prompt)>>,
}
impl Cam {
    pub fn prompts(&self) -> Vec<Prompt> {
        self.captures.borrow().iter().map(|(_, p)| *p).collect()
    }
    pub fn first_capture_after(&self, t: u64) -> Option<u64> {
        self.captures.borrow().iter().map(|(at, _)| *at).find(|at| *at >= t)
    }
}
impl Camera for Cam {
    fn capture(&self, prompt: Prompt) -> RawFrame {
        self.captures.borrow_mut().push((self.clock.get(), prompt));
        let mut pixels = self.facing.borrow().clone();
        pixels.extend_from_slice(format!(":{prompt:?}").as_bytes());
        let metadata = BTreeMap::from([
            ("GPSLatitude".to_string(), b"51.5074N".to_vec()),
            ("DateTimeOriginal".to_string(), b"2026:09:11 10:00:00".to_vec()),
            ("Model".to_string(), b"HarnessCam 1".to_vec()),
        ]);
        RawFrame { pixels, metadata }
    }
}

/// A clock every client on the harness shares, advanced by waiting.
pub struct SharedClock(pub Rc<Cell<u64>>);
impl Clock for SharedClock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
    fn wait_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}

/// A clock a fixed offset from the shared one.
pub struct SkewedClock(pub Rc<Cell<u64>>, pub i64);
impl Clock for SkewedClock {
    fn now_ms(&self) -> u64 {
        (self.0.get() as i64 + self.1) as u64
    }
    fn wait_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}

/// Seeded randomness, so two runs can be independent or identical.
pub struct Seeded(pub Cell<u64>);
impl Random for Seeded {
    fn fill(&self, out: &mut [u8]) {
        for b in out {
            let mut x = self.0.get();
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0.set(x);
            *b = (x >> 24) as u8;
        }
    }
}

/// An operator who says yes to everything and remembers being asked.
pub struct Person(pub RefCell<Vec<String>>);
impl Operator for Person {
    fn ask(&self, question: &str) -> bool {
        self.0.borrow_mut().push(question.to_string());
        true
    }
}

/// A notification hook that records every notice with the clock reading.
pub struct Hook {
    pub clock: Rc<Cell<u64>>,
    pub notices: RefCell<Vec<(u64, Notice)>>,
}
impl Notifier for Hook {
    fn notify(&self, n: Notice) {
        self.notices.borrow_mut().push((self.clock.get(), n));
    }
}

/// What the test keeps of one client's device.
pub struct Handles {
    pub cam: Rc<Cam>,
    pub person: Rc<Person>,
    pub hook: Rc<Hook>,
}

pub fn face(name: &str) -> Vec<u8> {
    format!("face:{name:<10}").into_bytes()
}

pub fn device(channels: Vec<ChannelKind>, clock: Rc<Cell<u64>>, seed: u64, skew_ms: i64) -> (Device, Handles) {
    let cam = Rc::new(Cam { facing: RefCell::new(vec![]), clock: clock.clone(), captures: RefCell::new(vec![]) });
    let person = Rc::new(Person(RefCell::new(vec![])));
    let hook = Rc::new(Hook { clock: clock.clone(), notices: RefCell::new(vec![]) });
    let clk: Rc<dyn Clock> = if skew_ms == 0 { Rc::new(SharedClock(clock)) } else { Rc::new(SkewedClock(clock, skew_ms)) };
    let d = Device { proximity: Rc::new(Channels(channels)), camera: cam.clone(), clock: clk, random: Rc::new(Seeded(Cell::new(seed))), operator: person.clone(), notifier: hook.clone(), engine: Rc::new(HashEngine::new(16)) };
    (d, Handles { cam, person, hook })
}

/// A harness of named clients, all with the same channels.
pub struct Setup {
    pub h: Harness,
    pub handles: BTreeMap<&'static str, Handles>,
    pub clock: Rc<Cell<u64>>,
    /// The clock reading when the last ceremony began.
    pub last_start: u64,
}

pub fn setup(names: &[&'static str], channels: &[ChannelKind]) -> Setup {
    setup_with(names, channels, &[])
}

pub fn setup_with(names: &[&'static str], channels: &[ChannelKind], skews: &[(&str, i64)]) -> Setup {
    let clock = Rc::new(Cell::new(1_790_000_000_000u64));
    let mut h = Harness::default();
    let mut handles = BTreeMap::new();
    for (i, n) in names.iter().enumerate() {
        let skew = skews.iter().find(|(s, _)| s == n).map(|(_, k)| *k).unwrap_or(0);
        let (d, hd) = device(channels.to_vec(), clock.clone(), 0x9e37_79b9_7f4a_7c15 ^ (i as u64 + 1), skew);
        h.add(Client::new(id(n), ids(), Config::default(), d));
        handles.insert(*n, hd);
    }
    Setup { h, handles, clock, last_start: 0 }
}

impl Setup {
    /// The clock ticks an hour and each device's camera faces the
    /// counterparty.
    pub fn face_off(&mut self, a: &str, b: &str) {
        self.clock.set(self.clock.get() + 3_600_000);
        self.last_start = self.clock.get();
        *self.handles[a].cam.facing.borrow_mut() = face(b);
        *self.handles[b].cam.facing.borrow_mut() = face(a);
    }
    pub fn run(&mut self, a: &str, b: &str, a_nom: &[&str], b_nom: &[&str]) -> Result<[u8; 32], Abort> {
        self.face_off(a, b);
        self.h.run(kh(a), kh(b), a_nom.iter().map(|n| kh(n)).collect(), b_nom.iter().map(|n| kh(n)).collect())
    }
    pub fn client(&mut self, n: &str) -> &mut Client {
        self.h.client(&kh(n))
    }
    pub fn notices(&self, n: &str) -> Vec<(u64, Notice)> {
        self.handles[n].hook.notices.borrow().clone()
    }
    pub fn prompts_asked(&self, n: &str) -> Vec<String> {
        self.handles[n].person.0.borrow().clone()
    }
}
