//! A ceremony between two participants on subnets that share no node
//! (design §6.4, §12.6.3, §14.1.1; `wire-format.md` §7.10.1; functional
//! row NET-022): each party is the kernel a phone runs, attached to its own
//! serving node over real sockets; the participants' legs cross the local
//! interface by hand, as the design makes them; each witness's legs travel
//! the payload path, and the far witness is reached across the other
//! subnet's infrastructure.

use rhtn_archive::tx::TYPE_ADOPTION;
use rhtn_ffi::client::Participant;
use rhtn_ffi::device::*;
use rhtn_ffi::types::*;
use rhtn_sim::subnets::{Fabric, kh, material, wan_plans};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A phone with nothing to measure, a clock that moves from the fabric's
/// instant, a holder who says yes, and app-private storage.
struct Shell {
    store: Mutex<BTreeMap<String, Vec<u8>>>,
    base_ms: u64,
    started: Instant,
    asked: Mutex<Vec<String>>,
    /// Every diagnostic line the kernel rendered (the field-test flavour).
    diag: Mutex<Vec<String>>,
}

impl Diagnostics for Shell {
    fn event(&self, line: String) {
        self.diag.lock().unwrap().push(line);
    }
}

impl Shell {
    fn new(base_s: u64) -> Arc<Shell> {
        Arc::new(Shell {
            store: Mutex::new(BTreeMap::new()),
            base_ms: base_s * 1000,
            started: Instant::now(),
            asked: Mutex::new(Vec::new()),
            diag: Mutex::new(Vec::new()),
        })
    }
}

impl Camera for Shell {
    fn capture(&self, _: Ask) -> Vec<u8> {
        vec![7; 64]
    }
    fn probe(&self) -> Vec<u8> {
        vec![7; 64]
    }
}
impl Clock for Shell {
    fn now_ms(&self) -> u64 {
        self.base_ms + self.started.elapsed().as_millis() as u64
    }
    fn wait_ms(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms.min(50)));
    }
}
impl Random for Shell {
    fn fill(&self, n: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(n as usize);
        while out.len() < n as usize {
            out.extend_from_slice(&rhtn_transport::tls::random_bytes::<64>());
        }
        out.truncate(n as usize);
        out
    }
}
impl Operator for Shell {
    fn ask(&self, q: String) -> bool {
        self.asked.lock().unwrap().push(q);
        true
    }
}
impl Notices for Shell {
    fn told(&self, _: Told) {}
}
impl Storage for Shell {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        self.store.lock().unwrap().get(&name).cloned()
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        self.store.lock().unwrap().insert(name, bytes);
        true
    }
}
impl Custody for Shell {
    fn key(&self) -> Option<Vec<u8>> {
        None
    }
    fn keep(&self, _: Vec<u8>) -> bool {
        false
    }
    fn unsealed(&self) -> bool {
        true
    }
}

fn platform_of(s: Arc<Shell>) -> Arc<Platform> {
    Arc::new(Platform {
        camera: s.clone(),
        clock: s.clone(),
        random: s.clone(),
        operator: s.clone(),
        notices: s.clone(),
        storage: s.clone(),
        custody: s.clone(),
        diagnostics: s,
        engine: None,
    })
}

/// The 64 seed bytes a kernel starts from, as every workspace test
/// derives them from a name.
fn seeds(name: &str) -> Vec<u8> {
    let mut b =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    b.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
    b
}

fn idv(name: &str) -> Vec<u8> {
    kh(name).to_vec()
}

/// What a scenario holds of one running kernel.
struct Phone {
    name: &'static str,
    p: Participant,
    shell: Arc<Shell>,
    /// Every event the kernel raised, as read off its stream.
    events: Mutex<Vec<String>>,
}

impl Phone {
    /// Read every event waiting on the kernel's stream.
    fn drain(&self) {
        while let Some(e) = self.p.next_event(0) {
            self.events
                .lock()
                .unwrap()
                .push(format!("{:?}", e).chars().take(400).collect());
        }
    }
}

impl Phone {
    /// Start `name`'s kernel knowing every party of the fabric, and attach
    /// it to the node that serves it, as a phone does.
    fn start(f: &Fabric, name: &'static str) -> Phone {
        let shell = Shell::new(f.clock);
        let known: Vec<Vec<u8>> = f.names.iter().map(|n| material(n)).collect();
        let p = Participant::start(seeds(name), known, platform_of(shell.clone()))
            .unwrap_or_else(|e| panic!("{name} starts: {}", e.reason()));
        let subnet = f.subnet_of(name);
        // **its own place first**, as a phone holds it: the meeting its
        // adoption rests on and the adoption itself, out of its own
        // archive, so its locator names its anchor and not itself
        let adoption = subnet
            .records
            .iter()
            .find(|r| r.tx_type == TYPE_ADOPTION && r.field_hash(1) == Some(kh(name)))
            .unwrap_or_else(|| panic!("{name} was adopted"));
        let pop = adoption
            .field_hash(8)
            .expect("an adoption rests on a meeting");
        p.finalize(f.store[&pop].clone(), None)
            .unwrap_or_else(|e| panic!("{name} holds its meeting: {}", e.reason()));
        p.take_adoption(adoption.bytes.clone())
            .unwrap_or_else(|e| panic!("{name} holds its adoption: {}", e.reason()));
        // the ceremony's window, which the capture is held across and the
        // proposer waits out: seconds here, where a phone takes minutes
        p.set_witness_window_s(5)
            .expect("the window is the shell's to set");
        let serving = subnet.serving_node_of(name);
        let addr = subnet.addr_of(serving);
        let attached = p
            .attach(idv(serving), vec![addr.to_string()], vec![])
            .unwrap_or_else(|e| panic!("{name} attaches to {serving}: {}", e.reason()));
        assert!(attached.primary, "{name} is in {serving}'s subtree");
        Phone {
            name,
            p,
            shell,
            events: Mutex::new(Vec::new()),
        }
    }
}

/// Move every leg each participant holds for the other across the local
/// interface, both ways, as a bearer would; how many moved.
fn pump(a: &Phone, b: &Phone) -> usize {
    let mut moved = 0;
    for _ in 0..8 {
        let mut any = false;
        for (from, to) in [(a, b), (b, a)] {
            for c in from.p.carriages() {
                to.p.take_carriage(c).unwrap_or_else(|e| {
                    panic!("{} takes {}'s carriage: {}", to.name, from.name, e.reason())
                });
                moved += 1;
                any = true;
            }
        }
        a.drain();
        b.drain();
        if !any {
            break;
        }
    }
    moved
}

/// Everything the scenario can see of where it stands, for a failure.
fn dump(f: &Fabric, phones: &[&Phone]) -> String {
    let mut out = String::new();
    for p in phones {
        p.drain();
        out.push_str(&format!(
            "\n== {}: {} | meetings {} | nominees {:?} | asked {:?}\n",
            p.name,
            progress_of(p),
            p.p.meetings().len(),
            p.p.nominees(),
            p.shell.asked.lock().unwrap()
        ));
        for e in p.events.lock().unwrap().iter() {
            out.push_str("   ");
            out.push_str(e);
            out.push('\n');
        }
        for l in p.shell.diag.lock().unwrap().iter() {
            if l.contains("WARN")
                || l.contains("declined")
                || l.contains("refused")
                || l.contains("pay.relay")
                || l.contains("node.resolve")
                || l.contains("unreached")
                || l.contains("pay.session")
                || l.contains("pay.error")
                || l.contains("pay.direct")
                || l.contains("pay.descent")
                || l.contains("pay.theirs")
                || l.contains("pay.prekey")
                || l.contains("pay.bundle")
            {
                out.push_str("   ~ ");
                out.push_str(&l.chars().take(300).collect::<String>());
                out.push('\n');
            }
        }
    }
    for s in &f.subnets {
        for (n, node) in &s.nodes {
            for who in ["alice", "c4", "w1", "c5"] {
                let q = node.node.queued(&kh(who));
                if q > 0 {
                    out.push_str(&format!("   {n} holds {q} for {who}\n"));
                }
            }
        }
    }
    out
}

/// Call a conversation step until it stops answering that it waits on
/// something, pumping the bearer between tries.
fn until_not_waiting(
    who: &Phone,
    other: &Phone,
    what: &str,
    mut step: impl FnMut(&Participant) -> Result<(), Refused>,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match step(&who.p) {
            Ok(()) => return,
            Err(e) if e.reason().contains("Waiting") && Instant::now() < deadline => {
                pump(who, other);
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => panic!("{} {what}: {}", who.name, e.reason()),
        }
    }
}

/// Pump the bearer until `done` holds or `secs` elapse.
fn settle(a: &Phone, b: &Phone, secs: u64, mut done: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < deadline {
        pump(a, b);
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    done()
}

fn progress_of(p: &Phone) -> String {
    match p.p.progress() {
        Some(pr) => format!(
            "opened {} proposable {} queries {} witnesses {} attesting {} declined {} back_from {} signed {} waiting_on {:?}",
            pr.opened,
            pr.proposable,
            pr.queries_outstanding,
            pr.witnesses_outstanding,
            pr.attesting.len(),
            pr.declined.len(),
            pr.back_from.len(),
            pr.signed.len(),
            pr.waiting_on
        ),
        None => "no ceremony".into(),
    }
}

/// The two subnets (`wan_plans`), each serving node holding the other's
/// anchor entry as a node's anchor table does (`wire-format.md` §7.2) and
/// nothing else of it: in this process, or the rented machines a manifest
/// in `RHTN_WAN_MANIFEST` names.
fn two_subnets() -> Fabric {
    Fabric::for_tests(wan_plans(), 30, &[("bob", 1), ("c3", 0)])
}

// acceptance: CER-73
// functional: NET-022
// functional: CER-033
// functional: PAY-021
/// **A ceremony opens across two subnets that share no node**: alice, a
/// light client of the home subnet served by bob, meets c4, a light
/// client of the far subnet served by c3 three infra nodes deep.  alice
/// nominates w1, served by bob; c4 nominates c5, served by c3.  Each
/// nominee's set crosses to the counterparty by locator, and every leg to
/// a witness travels the payload path: alice's request to c5 and c4's to
/// w1 cross subnets, each kernel putting the resolution to its own
/// serving node, whose anchor table holds the other subnet's entry, and
/// both witnesses answer that they will attest.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_ceremony_opens_across_two_subnets_and_both_witnesses_attest() {
    let f = Arc::new(two_subnets());
    let fab = f.clone();
    tokio::task::spawn_blocking(move || drive(&fab, false))
        .await
        .expect("the driver did not panic");
    drop(f);
}

/// **The same ceremony to its record**: every signer holds it, each
/// witness beside the meeting it attested.  What it took, read off the
/// kernels' own diagnostics once each had a sink of its own
/// [2026-11-15]: a far witness started knowing a participant's material
/// dropped the participant's opener whole, locator included, so it could
/// place nobody and ask nobody for the bundle, and the participant's
/// messages failed under a session the witness then opened itself; a
/// one-time-key reply landing after the peer had written first opened a
/// second session over the one that stood; and an open replaced the
/// ratchet the peer was still sending on and kept nothing of it.  The
/// construction's crossing rule (design §22.2) was never the defect.
// functional: PAY-021
// functional: PAY-022
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_ceremony_runs_between_two_subnets_that_share_no_node() {
    let f = Arc::new(two_subnets());
    let fab = f.clone();
    tokio::task::spawn_blocking(move || drive(&fab, true))
        .await
        .expect("the driver did not panic");
    drop(f);
}

fn drive(f: &Fabric, to_the_record: bool) {
    let t0 = Instant::now();
    let alice = Phone::start(f, "alice");
    let c4 = Phone::start(f, "c4");
    let w1 = Phone::start(f, "w1");
    let c5 = Phone::start(f, "c5");
    eprintln!("[{:?}] four kernels attached", t0.elapsed());

    // each participant holds its own nominee by introduction (§2.3), as a
    // party it has dealt with; the counterparty's nominee reaches it by
    // locator in the prune (kind 22)
    alice
        .p
        .take_introduction(w1.p.own_introduction().unwrap())
        .expect("alice holds w1");
    c4.p.take_introduction(c5.p.own_introduction().unwrap())
        .expect("c4 holds c5");
    // neither witness holds the far participant: the request carries both
    // participants' introductions (`wire-format.md` §7.10.2 field 6)
    // [author, 2026-11-15], and that is how each reaches the other side

    // step 1: both begin, alice initiating; each engages its nominee
    alice
        .p
        .begin(Some(idv("c4")), vec![idv("w1")], true)
        .expect("alice begins");
    c4.p.begin(Some(idv("alice")), vec![idv("c5")], false)
        .expect("c4 begins");
    eprintln!("[{:?}] begun", t0.elapsed());
    alice.p.converse_engage().expect("alice engages w1");
    c4.p.converse_engage().expect("c4 engages c5");
    eprintln!("[{:?}] engaged", t0.elapsed());

    // the optical exchange: contributions, then the ceremony-id each computed
    let (oa, oc) = (
        alice.p.optical_contribution().unwrap(),
        c4.p.optical_contribution().unwrap(),
    );
    assert_eq!(alice.p.take_optical(oc).unwrap(), idv("c4"));
    assert_eq!(c4.p.take_optical(oa).unwrap(), idv("alice"));
    let (ta, tc) = (
        alice.p.transcript_confirm().unwrap(),
        c4.p.transcript_confirm().unwrap(),
    );
    let cid = alice.p.take_transcript(tc).expect("the ids agree");
    assert_eq!(c4.p.take_transcript(ta).unwrap(), cid, "one ceremony");

    // the intents cross the bearer, each checked against the code read
    let (ia, ic) = (
        alice.p.intent_carriage().unwrap(),
        c4.p.intent_carriage().unwrap(),
    );
    c4.p.take_intent_carriage(idv("alice"), ia)
        .expect("c4 takes alice's intent");
    alice
        .p
        .take_intent_carriage(idv("c4"), ic)
        .expect("alice takes c4's intent");

    // the captures, each under the other's key, and at the same time, as
    // two devices across a table hold each other
    let (ka, kc) = (alice.p.capture_key().unwrap(), c4.p.capture_key().unwrap());
    std::thread::scope(|sc| {
        let a = sc.spawn(|| alice.p.capture(kc).expect("alice captures c4"));
        let c = sc.spawn(|| c4.p.capture(ka).expect("c4 captures alice"));
        a.join().unwrap();
        c.join().unwrap();
    });

    eprintln!("[{:?}] captured", t0.elapsed());
    // the prune: each side's set to the other by locator, the keep back
    pump(&alice, &c4);
    eprintln!("[{:?}] pruned", t0.elapsed());
    until_not_waiting(&alice, &c4, "opens", |p| p.converse_open());
    until_not_waiting(&c4, &alice, "opens", |p| p.converse_open());
    pump(&alice, &c4);

    eprintln!(
        "[{:?}] opened: alice {} / c4 {}",
        t0.elapsed(),
        progress_of(&alice),
        progress_of(&c4)
    );
    // step 6: the queries; neither has met anybody, so neither has a
    // verifier to ask about the other, and nothing is outstanding
    alice.p.converse_queries().expect("alice's queries");
    c4.p.converse_queries().expect("c4's queries");
    assert!(
        settle(&alice, &c4, 10, || {
            [&alice, &c4]
                .iter()
                .all(|p| p.p.progress().is_some_and(|pr| pr.queries_outstanding == 0))
        }),
        "queries settle: {}",
        dump(f, &[&alice, &c4, &w1, &c5])
    );
    eprintln!("[{:?}] queries settled", t0.elapsed());
    c4.p.converse_gathered()
        .expect("c4 hands over what it gathered");

    // the witnesses answer over the payload path: each nominee to the
    // side that nominated it within its own subnet, and to the other side
    // across them, so the far answer is waited for and not expected at
    // the instant the home one lands (on rented machines it is a
    // resolution and a handshake behind)
    assert!(
        settle(&alice, &c4, 30, || {
            alice.p.progress().is_some_and(|pr| pr.proposable)
                && [&alice, &c4]
                    .iter()
                    .all(|p| p.p.progress().is_some_and(|pr| pr.attesting.len() == 2))
        }),
        "alice can propose and both hold both answers: {}",
        dump(f, &[&alice, &c4, &w1, &c5])
    );
    eprintln!("[{:?}] proposable", t0.elapsed());
    // **both witnesses will attest, one reached across the subnets by each
    // side**: the far request went by the serving node's resolution and
    // the answer came back the same way
    for (p, mine) in [(&alice, "w1"), (&c4, "c5")] {
        let pr = p.p.progress().expect("in a ceremony");
        assert_eq!(
            pr.declined.len(),
            0,
            "{}: {}",
            p.name,
            dump(f, &[&alice, &c4, &w1, &c5])
        );
        assert!(
            pr.attesting.contains(&kh(mine).to_vec()),
            "{}'s own nominee {mine} attests",
            p.name
        );
    }
    assert_eq!(
        alice.p.progress().unwrap().attesting.len(),
        2,
        "the proposer holds both answers: {}",
        dump(f, &[&alice, &c4, &w1, &c5])
    );
    // a node's own counts, where this process holds the node
    if !f.is_remote() {
        let (home, far) = (f.subnet_of("alice"), f.subnet_of("c4"));
        assert_eq!(
            home.node("bob").node.queued(&kh("c5")),
            0,
            "bob carried nothing for c5"
        );
        assert_eq!(
            far.node("c3").node.queued(&kh("w1")),
            0,
            "c3 carried nothing for w1"
        );
    }
    if !to_the_record {
        return;
    }
    until_not_waiting(&alice, &c4, "proposes", |p| p.converse_propose());
    eprintln!("[{:?}] proposed", t0.elapsed());

    // the body goes to c4, w1 and c5; each signs; alice finalizes and sends
    // the record: every signer holds it, the witnesses beside the meeting
    // each restored at start
    let phones = [&alice, &c4, &w1, &c5];
    let meeting = |p: &Phone, with: &str| {
        p.p.meetings()
            .into_iter()
            .find(|m| m.counterparty == kh(with))
    };
    assert!(
        settle(&alice, &c4, 30, || meeting(&alice, "c4").is_some()),
        "alice finalized: {}",
        dump(f, &phones)
    );
    let record = meeting(&alice, "c4").unwrap().record;
    // each witness with who nominated it; the witness is the first of the pair
    let holds = |p: &Phone| -> Vec<[u8; 32]> {
        p.p.record_witnesses(record.to_vec())
            .into_iter()
            .filter_map(|pair| pair.into_iter().next())
            .filter_map(|w| w.try_into().ok())
            .collect()
    };
    assert!(
        settle(&alice, &c4, 30, || phones
            .iter()
            .all(|p| !holds(p).is_empty())),
        "every signer holds the record: {}",
        dump(f, &phones)
    );
    eprintln!("[{:?}] finalized everywhere", t0.elapsed());
    for p in phones {
        let w = holds(p);
        assert!(
            w.contains(&kh("w1")) && w.contains(&kh("c5")) && w.len() == 2,
            "{}'s copy carries both witnesses: {:?}",
            p.name,
            w
        );
    }
    assert_eq!(meeting(&c4, "alice").map(|m| m.record), Some(record));
    assert_eq!(meeting(&alice, "c4").unwrap().witnesses.len(), 2);

    assert_eq!(
        alice.shell.asked.lock().unwrap().len(),
        1,
        "{:?}",
        alice.shell.asked
    );
    assert_eq!(c4.shell.asked.lock().unwrap().len(), 1);
}
