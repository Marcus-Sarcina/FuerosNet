//! The ceremony end to end on the harness: three clients meet in turn,
//! witnesses sign, a verifier is asked about a subject it holds, and the
//! record every signer keeps is checked against what the requirements say
//! the client does on the way.

mod common;

use common::*;
use rhtn_client::ceremony::*;
use rhtn_client::device::*;
use rhtn_client::notice::{Notice, Notifier, Role};
use rhtn_client::query::{Response, Verdict};
use rhtn_client::record::{LABELS, read_presentation};
use rhtn_client::store::{Frame, open};
use rhtn_codec::cbor::*;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Proximity hardware with a fixed set of channels, all of which pass.
struct Channels(Vec<ChannelKind>);
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
struct Cam {
    facing: RefCell<Vec<u8>>,
    clock: Rc<Cell<u64>>,
    captures: RefCell<Vec<(u64, Prompt)>>,
}
impl Cam {
    fn prompts(&self) -> Vec<Prompt> {
        self.captures.borrow().iter().map(|(_, p)| *p).collect()
    }
    fn first_capture_after(&self, t: u64) -> Option<u64> {
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
struct SharedClock(Rc<Cell<u64>>);
impl Clock for SharedClock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
    fn wait_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}

/// A clock a fixed offset from the shared one.
struct SkewedClock(Rc<Cell<u64>>, i64);
impl Clock for SkewedClock {
    fn now_ms(&self) -> u64 {
        (self.0.get() as i64 + self.1) as u64
    }
    fn wait_ms(&self, ms: u64) {
        self.0.set(self.0.get() + ms);
    }
}

/// Seeded randomness, so two runs can be independent or identical.
struct Seeded(Cell<u64>);
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
struct Person(RefCell<Vec<String>>);
impl Operator for Person {
    fn ask(&self, question: &str) -> bool {
        self.0.borrow_mut().push(question.to_string());
        true
    }
}

/// A notification hook that records every notice with the clock reading.
struct Hook {
    clock: Rc<Cell<u64>>,
    notices: RefCell<Vec<(u64, Notice)>>,
}
impl Notifier for Hook {
    fn notify(&self, n: Notice) {
        self.notices.borrow_mut().push((self.clock.get(), n));
    }
}

/// What the test keeps of one client's device.
struct Handles {
    cam: Rc<Cam>,
    person: Rc<Person>,
    hook: Rc<Hook>,
}

fn face(name: &str) -> Vec<u8> {
    format!("face:{name:<10}").into_bytes()
}

fn device(channels: Vec<ChannelKind>, clock: Rc<Cell<u64>>, seed: u64, skew_ms: i64) -> (Device, Handles) {
    let cam = Rc::new(Cam { facing: RefCell::new(vec![]), clock: clock.clone(), captures: RefCell::new(vec![]) });
    let person = Rc::new(Person(RefCell::new(vec![])));
    let hook = Rc::new(Hook { clock: clock.clone(), notices: RefCell::new(vec![]) });
    let clk: Rc<dyn Clock> = if skew_ms == 0 { Rc::new(SharedClock(clock)) } else { Rc::new(SkewedClock(clock, skew_ms)) };
    let d = Device { proximity: Rc::new(Channels(channels)), camera: cam.clone(), clock: clk, random: Rc::new(Seeded(Cell::new(seed))), operator: person.clone(), notifier: hook.clone(), engine: Rc::new(HashEngine::new(16)) };
    (d, Handles { cam, person, hook })
}

/// A harness of named clients, all with the same channels.
struct Setup {
    h: Harness,
    handles: BTreeMap<&'static str, Handles>,
    clock: Rc<Cell<u64>>,
    /// The clock reading when the last ceremony began.
    last_start: u64,
}

fn setup(names: &[&'static str], channels: &[ChannelKind]) -> Setup {
    setup_with(names, channels, &[])
}

fn setup_with(names: &[&'static str], channels: &[ChannelKind], skews: &[(&str, i64)]) -> Setup {
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
    fn run(&mut self, a: &str, b: &str, a_nom: &[&str], b_nom: &[&str]) -> Result<[u8; 32], Abort> {
        self.clock.set(self.clock.get() + 3_600_000);
        self.last_start = self.clock.get();
        // each device's camera faces the counterparty
        *self.handles[a].cam.facing.borrow_mut() = face(b);
        *self.handles[b].cam.facing.borrow_mut() = face(a);
        self.h.run(kh(a), kh(b), a_nom.iter().map(|n| kh(n)).collect(), b_nom.iter().map(|n| kh(n)).collect())
    }
    fn client(&mut self, n: &str) -> &mut Client {
        self.h.client(&kh(n))
    }
    fn notices(&self, n: &str) -> Vec<(u64, Notice)> {
        self.handles[n].hook.notices.borrow().clone()
    }
    fn prompts_asked(&self, n: &str) -> Vec<String> {
        self.handles[n].person.0.borrow().clone()
    }
}

/// The proximity field of a record's disclosure set, as (kind, result).
fn proximity_of(c: &Client, txid: &[u8; 32]) -> (Vec<(u64, u64)>, u64) {
    let set = &c.store.disclosures[txid];
    let v = &set[6].value;
    let item = parse_all(v).unwrap();
    let Item::Map(m) = &item else { panic!("proximity map") };
    let strongest = as_uint(map_get(m, 2).unwrap()).unwrap();
    let Some(Item::Array(ch)) = map_get(m, 1) else { panic!("channels") };
    let kinds = ch
        .iter()
        .map(|c| {
            let Item::Map(cm) = c else { panic!() };
            (as_uint(map_get(cm, 1).unwrap()).unwrap(), as_uint(map_get(cm, 2).unwrap()).unwrap())
        })
        .collect();
    (kinds, strongest)
}

fn all(names: &[&'static str]) -> Vec<&'static str> {
    names.to_vec()
}

const EVERYONE: [&str; 6] = ["alice", "bob", "carol", "w1", "w2", "w3"];

// acceptance: CER-01
#[test]
fn the_strongest_channel_the_hardware_has_is_what_the_record_says_was_achieved() {
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc, ChannelKind::Optical, ChannelKind::Latency]);
    let txid = s.run("alice", "bob", &["w1"], &["w2"]).expect("a record");
    for n in ["alice", "bob", "w1", "w2"] {
        assert!(s.client(n).store.records.contains_key(&txid), "{n} holds the record");
        assert_eq!(s.client(n).archive.len(), 1);
    }
    let (kinds, strongest) = proximity_of(s.client("alice"), &txid);
    assert_eq!(strongest, ChannelKind::Nfc.code(), "NFC is the strongest achieved");
    assert!(kinds.iter().all(|(k, _)| *k != ChannelKind::Uwb.code()), "no UWB entry");
    assert_eq!(kinds, vec![(2, 0), (3, 0), (4, 0)]);
    // both participants hold the same set, and the presentation reads
    let set = s.client("bob").store.disclosures[&txid].clone();
    assert_eq!(set, s.client("alice").store.disclosures[&txid]);
    let rec = s.client("alice").store.records[&txid].clone();
    let p = read_presentation(&ids(), &rhtn_client::record::present(&rec, &set, &["proximity"])).unwrap();
    assert_eq!(p.revealed.len(), 1);
}

// acceptance: CER-02
#[test]
fn a_weaker_channel_is_never_presented_as_a_stronger_one() {
    let mut s = setup(&EVERYONE, &[ChannelKind::Optical]);
    let txid = s.run("alice", "bob", &["w1"], &["w2"]).expect("a record");
    let (kinds, strongest) = proximity_of(s.client("alice"), &txid);
    assert_eq!(strongest, ChannelKind::Optical.code());
    assert_eq!(kinds, vec![(3, 0)], "optical and nothing ranked above it");
    // no channel at all: no record
    let mut none = setup(&EVERYONE, &[]);
    assert_eq!(none.run("alice", "bob", &["w1"], &["w2"]), Err(Abort::NoProximity));
}

// acceptance: CER-03
#[test]
fn the_guided_capture_takes_three_to_five_frames_over_ten_to_fifteen_seconds_under_prompts_that_vary() {
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    let t1 = s.run("alice", "bob", &["w1"], &["w2"]).unwrap();
    let first: Vec<Prompt> = s.handles["bob"].cam.prompts();
    let t2 = s.run("alice", "bob", &["w1"], &["w2"]).unwrap();
    let second: Vec<Prompt> = s.handles["bob"].cam.prompts()[first.len()..].to_vec();
    assert_ne!(first, second, "independent randomness, different prompt sequences");
    // each of bob's sealed captures of alice opens under alice's key and holds 3–5 frames spanning 10–15 s
    for t in [t1, t2] {
        let seed = s.client("alice").store.seeds[&t].clone();
        let key = rhtn_client::keys::capture_key(&seed.seed, &kh("alice"), &kh("bob"), &seed.ceremony_id);
        let sealed = s.client("bob").store.sealed[&t].clone();
        let cap = open(&Default::default(), &key, &sealed).expect("opens under the subject's key");
        assert!((3..=5).contains(&cap.frames.len()), "{} frames", cap.frames.len());
        let span = cap.frames.last().unwrap().at_ms - cap.frames[0].at_ms;
        assert!((10_000..=15_000).contains(&span), "span {span} ms");
    }
}

// acceptance: CER-06
#[test]
fn a_client_holds_no_decryptable_likeness_and_no_sealing_key() {
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    let t = s.run("alice", "bob", &["w1"], &["w2"]).unwrap();
    let a_seed = s.client("alice").store.seeds[&t].clone();
    let k_capture = rhtn_client::keys::capture_key(&a_seed.seed, &kh("alice"), &kh("bob"), &a_seed.ceremony_id);
    let bob = s.client("bob");
    // what bob holds: the sealed capture, the record, and bob's own seed
    assert!(bob.store.sealed.contains_key(&t));
    assert!(bob.store.seeds.contains_key(&t), "bob's seed for alice's captures of bob");
    assert_ne!(bob.store.seeds[&t].seed, a_seed.seed);
    // and not: any frame, the template, alice's seed, or the key alice supplied
    let cap = open(&Default::default(), &k_capture, &bob.store.sealed[&t]).unwrap();
    for f in &cap.frames {
        assert!(!bob.store.holds_bytes(&f.bytes), "no plaintext frame");
        assert!(!bob.store.holds_bytes(&f.bytes[..12]), "not even the face");
    }
    assert!(!bob.store.holds_bytes(&cap.template), "no plaintext template");
    assert!(!bob.store.holds_bytes(&k_capture), "no k_capture for alice's capture");
    assert!(!bob.store.holds_bytes(&a_seed.seed), "no seed of alice's");
    // opening without a grant: bob's own seed derives nothing that opens it
    let wrong = rhtn_client::keys::capture_key(&bob.store.seeds[&t].seed, &kh("alice"), &kh("bob"), &a_seed.ceremony_id);
    assert!(open(&Default::default(), &wrong, &bob.store.sealed[&t]).is_err());
    assert!(open(&Default::default(), &[0; 32], &bob.store.sealed[&t]).is_err());
}

// acceptance: CER-23
#[test]
fn captured_frames_are_stripped_of_metadata_before_sealing() {
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    let t = s.run("alice", "bob", &["w1"], &["w2"]).unwrap();
    let seed = s.client("alice").store.seeds[&t].clone();
    let key = rhtn_client::keys::capture_key(&seed.seed, &kh("alice"), &kh("bob"), &seed.ceremony_id);
    let cap = open(&Default::default(), &key, &s.client("bob").store.sealed[&t]).unwrap();
    assert!(!cap.frames.is_empty());
    for Frame { bytes, .. } in &cap.frames {
        assert!(bytes.starts_with(b"face:alice"), "the pixels");
        for needle in [&b"51.5074N"[..], b"2026:09:11", b"HarnessCam", b"GPSLatitude", b"DateTimeOriginal", b"Model"] {
            assert!(!bytes.windows(needle.len()).any(|w| w == needle), "no metadata survives");
        }
    }
}

/// Four meetings: carol meets bob, alice meets bob twice, then alice meets
/// carol, where carol, who recognises bob, selects him as alice's verifier
/// (alice's bundle of two records requires one); carol's own bundle of one
/// requires none.  The log is left holding the last ceremony only.
fn three_meetings(channels: &[ChannelKind]) -> (Setup, [u8; 32], [u8; 32], [u8; 32]) {
    let mut s = setup(&EVERYONE, channels);
    let cb = s.run("carol", "bob", &["w1"], &["w2"]).unwrap();
    let _ab1 = s.run("alice", "bob", &["w1"], &["w2"]).unwrap();
    let ab = s.run("alice", "bob", &["w1"], &["w2"]).unwrap();
    let before = s.h.log.len();
    let ac = s.run("alice", "carol", &["w2"], &["w3"]).unwrap();
    s.h.log.drain(..before);
    (s, cb, ab, ac)
}

// acceptance: CER-24
#[test]
fn the_capture_key_goes_directly_to_the_selected_verifier_and_nowhere_else() {
    let (s, _, ab, ac) = three_meetings(&[ChannelKind::Nfc]);
    // carol, running the ceremony with alice, selected bob for alice: the
    // record carries bob's response about alice, and alice's grant to bob
    // named the alice-bob record
    let grants: Vec<&Sent> = s.h.log.iter().filter(|m| matches!(m.msg, Msg::Grant(_))).collect();
    assert!(!grants.is_empty(), "keys were released");
    let alice_to_bob: Vec<&&Sent> = grants.iter().filter(|m| m.from == kh("alice")).collect();
    assert_eq!(alice_to_bob.len(), 1, "one grant from alice, for the one verifier selected for her");
    assert_eq!(alice_to_bob[0].to, kh("bob"));
    let Msg::Grant(bytes) = &alice_to_bob[0].msg else { unreachable!() };
    let g = rhtn_client::query::KeyGrant::decode(bytes).unwrap();
    assert_eq!(g.record, ab, "against the most recent capture bob holds of alice");
    // no grant bytes on any path to carol or to a witness
    for m in &s.h.log {
        if m.to == kh("carol") || m.to == kh("w2") || m.to == kh("w3") {
            let p = m.msg.payload();
            assert!(!p.windows(32).any(|w| w == g.key), "the key reached {} on a {:?}", if m.to == kh("carol") { "carol" } else { "a witness" }, std::mem::discriminant(&m.msg));
            assert!(!matches!(m.msg, Msg::Grant(_)));
        }
    }
    // and the record carries bob's match about alice
    let rec = s.h.clients[&kh("alice")].store.records[&ac].clone();
    let body = body_of(&rec);
    let r5 = value_slice(&body, 5).expect("responses");
    let responses: Vec<Response> = array_item_ranges(&body, r5.start).unwrap().into_iter().map(|r| Response::read(&body[r]).unwrap()).collect();
    let about_alice: Vec<&Response> = responses.iter().filter(|r| r.subject == kh("alice")).collect();
    assert_eq!(about_alice.len(), 1);
    assert_eq!((about_alice[0].verifier, about_alice[0].verdict), (kh("bob"), Verdict::Match));
}

// acceptance: CER-17
#[test]
fn querying_witnessing_and_answering_ask_nobody() {
    let (s, _, _, _) = three_meetings(&[ChannelKind::Nfc]);
    assert!(s.h.log.iter().any(|m| matches!(m.msg, Msg::Query(_))), "queries were issued");
    assert!(s.h.log.iter().any(|m| matches!(m.msg, Msg::Response(_))), "and answered");
    // the only question anyone was asked was whether to start, and only the participants
    for n in EVERYONE {
        let asked = s.prompts_asked(n);
        assert!(asked.iter().all(|q| q.starts_with("Start a presence ceremony")), "{n} was asked: {asked:?}");
        if n == "bob" || n.starts_with('w') {
            // bob verified in the last ceremony and did not start it; witnesses only witnessed
            let last_started = asked.len();
            let _ = last_started;
        }
    }
    assert!(s.prompts_asked("w1").is_empty() && s.prompts_asked("w2").is_empty() && s.prompts_asked("w3").is_empty(), "no witness was asked anything");
    // the verifier's operator learns nothing of the ceremony: bob's notices
    // in the last ceremony name no party and carry nothing but the
    // standing disclosure
    let bob = s.notices("bob");
    let during: Vec<&Notice> = bob.iter().filter(|(t, _)| *t >= s.last_start).map(|(_, n)| n).collect();
    assert!(!during.is_empty(), "bob was queried");
    assert!(during.iter().all(|n| matches!(n, Notice::RecordDisclosure { role: Role::Verifier })), "bob's operator was told only the disclosure, not the query: {during:?}");
}

// acceptance: CER-26
#[test]
fn what_the_record_will_contain_is_disclosed_at_capture_and_at_the_moment_a_witness_or_verifier_is_asked() {
    let (s, _, _, _) = three_meetings(&[ChannelKind::Nfc]);
    let start = s.last_start;
    // participants: a disclosure before capture, which is before the first frame's time
    for n in ["alice", "carol"] {
        let ns = s.notices(n);
        let disclosed = ns.iter().find(|(t, x)| *t >= start && matches!(x, Notice::RecordDisclosure { role: Role::Participant })).map(|(t, _)| *t).expect("a participant disclosure");
        let first_frame = s.handles[n].cam.first_capture_after(start).expect("{n} captured");
        assert!(disclosed <= first_frame, "{n} was told at {} ms, before its first frame at {} ms", disclosed - start, first_frame - start);
    }
    // witnesses: at the moment each was asked
    for w in ["w2", "w3"] {
        let ns = s.notices(w);
        assert!(ns.iter().any(|(t, x)| *t >= start && matches!(x, Notice::RecordDisclosure { role: Role::Witness })), "{w} was told");
        let asked = s.h.log.iter().position(|m| m.to == kh(w) && matches!(m.msg, Msg::WitnessRequest(_))).expect("asked");
        let answered = s.h.log.iter().position(|m| m.from == kh(w) && matches!(m.msg, Msg::WitnessAnswer(_))).expect("answered");
        assert!(asked < answered);
    }
    // the verifier: at the moment it was asked
    let bob = s.notices("bob");
    assert!(bob.iter().any(|(t, x)| *t >= start && matches!(x, Notice::RecordDisclosure { role: Role::Verifier })), "bob was told when queried");
}

#[test]
fn a_witness_far_from_the_claimed_start_declines_and_the_others_carry_the_record() {
    let mut s = setup_with(&EVERYONE, &[ChannelKind::Nfc], &[("w1", 3_600_000)]);
    let t = s.run("alice", "bob", &["w1"], &["w2"]).expect("w2 suffices");
    let rec = rhtn_archive::record::Record::parse(&s.client("alice").store.records[&t]).unwrap();
    assert_eq!(rec.signers.len(), 3, "alice, bob and w2");
    assert!(!rec.signers.contains(&kh("w1")));
    assert!(s.client("w1").store.records.is_empty());
    assert!(s.notices("alice").iter().any(|(_, n)| matches!(n, Notice::NomineesOutnumbered { mine: 0, theirs: 1 })), "alice is told her nominee is absent");
    // every nominee declining: no record
    let mut none = setup_with(&EVERYONE, &[ChannelKind::Nfc], &[("w1", 3_600_000), ("w2", -3_600_000)]);
    assert_eq!(none.run("alice", "bob", &["w1"], &["w2"]), Err(Abort::NoWitness));
}

#[test]
fn the_records_disclosable_fields_are_all_present_and_withheld_by_default() {
    let mut s = setup(&EVERYONE, &[ChannelKind::Nfc]);
    let t = s.run("alice", "bob", &["w1"], &["w2"]).unwrap();
    let alice = s.client("alice");
    let set = alice.store.disclosures[&t].clone();
    assert_eq!(set.iter().map(|d| d.label).collect::<Vec<_>>(), LABELS.to_vec());
    let rec = alice.store.records[&t].clone();
    let p = read_presentation(&ids(), &rhtn_client::record::present(&rec, &set, &[])).unwrap();
    assert_eq!(p.withheld.len(), 7);
    let _ = all(&EVERYONE);
}
