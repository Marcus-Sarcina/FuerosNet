//! The boundary as a shell uses it (`light-client-requirements.md` §1.3):
//! the platform's objects handed over once, the client running on a thread
//! of its own behind them, and every answer a value.

use rhtn_ffi::client::Participant;
use rhtn_ffi::device::*;
use rhtn_ffi::net::{Event, Wake};
use rhtn_ffi::types::KIND_APPLICATION;
use rhtn_ffi::types::*;
use std::sync::{Arc, Mutex};

/// A kind of the shell's own, which the kernel hands up as payload
/// (`Event::Payload`'s kind is the application's to read): what crosses
/// the direct path with nothing around it but the connection.
const SHELL_KIND: u64 = 100;

/// A shell's hardware: what it reports and what it was asked.
#[derive(Default)]
struct Shell {
    asked: Mutex<Vec<Ask>>,
    told: Mutex<Vec<Told>>,
    questions: Mutex<Vec<String>>,
    /// Bytes the platform's randomness returns for each call, in order.
    short: bool,
    /// The system clock rather than a fixed instant: a delegation's window
    /// is checked by the serving node on its own clock, so a device that
    /// presents one must agree with it about the time.
    real_time: bool,
    /// What the kernel wrote through the storage seam, by name: the
    /// app-private storage a phone would give it, kept here so a second
    /// process can start from what the first wrote.
    store: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
    /// Every write, by name, in order: what a save rewrote.
    writes: Mutex<Vec<String>>,
    /// The storage key this platform keeps (`light-client-requirements.md`
    /// §9), beside the store for the same reason the store is here.
    custody: Mutex<Option<Vec<u8>>>,
    /// A platform that declares itself unsealed: the environment owns at
    /// rest, and the kernel writes what it writes.
    declared_unsealed: bool,
    /// A platform whose custody refuses to keep a key, which must refuse
    /// the start rather than fall to plaintext.
    keeps_nothing: bool,
}

impl Custody for Shell {
    fn key(&self) -> Option<Vec<u8>> {
        self.custody.lock().unwrap().clone()
    }
    fn keep(&self, key: Vec<u8>) -> bool {
        if self.keeps_nothing {
            return false;
        }
        *self.custody.lock().unwrap() = Some(key);
        true
    }
    fn unsealed(&self) -> bool {
        self.declared_unsealed
    }
}

impl Storage for Shell {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        self.store.lock().unwrap().get(&name).cloned()
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        self.writes.lock().unwrap().push(name.clone());
        self.store.lock().unwrap().insert(name, bytes);
        true
    }
}

impl Camera for Shell {
    fn capture(&self, ask: Ask) -> Vec<u8> {
        self.asked.lock().unwrap().push(ask);
        vec![7u8; 64]
    }

    fn probe(&self) -> Vec<u8> {
        vec![7u8; 64]
    }
}

impl Clock for Shell {
    fn now_ms(&self) -> u64 {
        if self.real_time {
            return std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
        }
        1_800_000_000_000
    }
    fn wait_ms(&self, _ms: u64) {}
}

impl Random for Shell {
    fn fill(&self, n: u32) -> Vec<u8> {
        // a platform that returns short measure is refused, not padded
        let want = if self.short {
            (n as usize).saturating_sub(1)
        } else {
            n as usize
        };
        // real randomness: two shells drawing the same constant would seal
        // under the same ephemeral keys and read as one party to each other
        let mut out = Vec::with_capacity(want);
        while out.len() < want {
            out.extend_from_slice(&rhtn_transport::tls::random_bytes::<64>());
        }
        out.truncate(want);
        out
    }
}

impl Operator for Shell {
    fn ask(&self, question: String) -> bool {
        self.questions.lock().unwrap().push(question);
        true
    }
}

impl Notices for Shell {
    fn told(&self, notice: Told) {
        self.told.lock().unwrap().push(notice);
    }
}

fn platform_of(shell: Arc<Shell>) -> Arc<Platform> {
    Arc::new(Platform {
        camera: shell.clone(),
        clock: shell.clone(),
        random: shell.clone(),
        operator: shell.clone(),
        notices: shell.clone(),
        storage: shell.clone(),
        custody: shell,
        diagnostics: Arc::new(Silent),
        engine: None,
    })
}

fn seeds(name: &str) -> Vec<u8> {
    let mut b =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    b.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
    b
}

fn material(name: &str) -> Vec<u8> {
    rhtn_crypto::identity::testkit::test_identity(name)
        .public
        .key_material()
}

fn id(name: &str) -> Vec<u8> {
    rhtn_crypto::identity::testkit::test_identity(name)
        .public
        .keyhash
        .to_vec()
}

// acceptance: DMN-10
#[test]
fn a_shell_drives_the_client_through_the_boundary_and_gets_values_back() {
    let shell = Arc::new(Shell {
        ..Default::default()
    });
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| material(n))
        .collect();
    let p = Participant::start(seeds("alice"), known, platform_of(shell.clone())).expect("starts");
    assert_eq!(
        p.me(),
        id("alice"),
        "the identity it was given, derived on its own thread"
    );

    // THE CEREMONY OPENS, AND WHAT CROSSES IS BYTES.  §14.3 fixes an
    // encoding for every object two present devices exchange, so the shell
    // moves opaque strings on whatever bearer it has and parses none of
    // them -- the second parser above the kernel being the hazard design
    // §11.2 names.
    p.begin(Some(id("bob")), vec![id("carol")], true)
        .expect("begins");
    let optical = p.optical_contribution().expect("the first QR");
    assert_eq!(
        optical.len(),
        2022,
        "array head 1, version 1, the full KeyMaterial 2003, contribution 1+16"
    );

    // a second participant through the same boundary
    let bob = Participant::start(
        seeds("bob"),
        ["alice", "bob", "carol"]
            .iter()
            .map(|n| material(n))
            .collect(),
        platform_of(Arc::new(Shell {
            ..Default::default()
        })),
    )
    .unwrap();
    bob.begin(Some(id("alice")), vec![id("carol")], false)
        .expect("begins");
    // each reads the other's QR off the screen, then the ceremony-id each
    // computed; agreement is what fixes it
    let theirs = bob.optical_contribution().unwrap();
    assert_eq!(
        p.take_optical(theirs).expect("alice reads bob's QR"),
        id("bob"),
        "the QR names who showed it"
    );
    bob.take_optical(optical).expect("and bob reads alice's");
    let tb = bob.transcript_confirm().unwrap();
    let ta = p.transcript_confirm().unwrap();
    let cid = p.take_transcript(tb).expect("the ids agree");
    assert_eq!(bob.take_transcript(ta).unwrap(), cid, "one ceremony");
    assert_eq!(p.ceremony(), Some(cid));
    // and the bearer carries the intent, which the kernel checks against
    // the contribution it read off the screen
    let carriage = bob.intent_carriage().expect("bob's carriage");
    assert_eq!(carriage.len(), 1, "an empty bundle fits in one");
    assert_eq!(
        p.take_intent_carriage(id("bob"), carriage).unwrap(),
        0,
        "no continuations followed"
    );

    // **and the candidates cross the bearer, so the two can speak with no
    // node at all** (design §6.4, §12.6.3): neither participant here is
    // attached to anything, each is the other's contact from the exchange,
    // and after the handover a payload goes direct
    let offer = p.candidate_offer().expect("alice's candidates, anchored");
    assert!(
        bob.take_candidate_offer(offer)
            .expect("anchored to this ceremony"),
        "bob dials alice's candidates and holds the path"
    );
    let back = bob.candidate_offer().expect("bob's candidates, anchored");
    assert!(
        p.take_candidate_offer(back).expect("anchored"),
        "alice holds it too"
    );
    // the path is the seal (`wire-format.md` §7.10.1): a shell's own kind
    // crosses it with no node asked for anything. A direct message, kind
    // 0, always passes the recipient's node (design §14.2.2), so with none
    // attached it is refused, not sent some other way
    p.send(id("bob"), SHELL_KIND, b"with no node".to_vec())
        .expect("direct, with no node attached");
    match bob.next_event(5000) {
        Some(Event::Payload { from, kind, bytes }) => {
            assert_eq!(from, id("alice"));
            assert_eq!(kind, SHELL_KIND);
            assert_eq!(bytes, b"with no node");
        }
        other => panic!("bob got {other:?}"),
    }
    assert!(
        p.send(id("bob"), 0, b"a direct message".to_vec()).is_err(),
        "a direct message with no node to seal it through is refused"
    );

    // a refusal is a value carrying its reason, not a failure the shell
    // has to guess at
    let e = p.take_optical(vec![0x00]).unwrap_err();
    assert!(e.reason().contains("Malformed"), "{e}");
    let e = p.begin(Some(vec![1, 2, 3]), vec![], true).unwrap_err();
    assert!(e.reason().contains("32 bytes"), "{e}");
    let Err(e) = Participant::start(vec![0; 10], vec![], platform_of(shell)) else {
        panic!("ten bytes is not an identity")
    };
    assert!(e.reason().contains("64 bytes of seed"), "{e}");
}

// acceptance: DMN-10
#[test]
fn randomness_of_short_measure_is_refused_and_the_shell_is_told() {
    let shell = Arc::new(Shell {
        short: true,
        // a kept storage key: the mint would otherwise draw from the same
        // short source and refuse first, which is not this test's target
        custody: Mutex::new(Some(vec![5u8; 32])),
        ..Default::default()
    });
    let known: Vec<Vec<u8>> = ["alice"].iter().map(|n| material(n)).collect();
    // the kernel draws on the platform's randomness once as it starts, so
    // a platform giving short measure is caught there: bytes completed
    // with zeroes are not random, and padding them silently is worse than
    // refusing
    let Err(e) = Participant::start(seeds("alice"), known, platform_of(shell)) else {
        panic!("short measure was accepted")
    };
    // and it reaches the shell as a value, not as a process that vanished
    assert!(e.reason().contains("came up short"), "{e}");
}

// ------------------------------------- the kernel's own side of the wire

use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::{Log, NodeConfig};
use rhtn_transport::tls::Pins;

/// The subnet the fixtures' positions are in (`wire-format.md` §2.3 field 4).
const SUBNET: [u8; 32] = [0xdb; 32];

fn sid(name: &str) -> rhtn_crypto::SigningIdentity {
    rhtn_crypto::identity::testkit::test_identity(name)
}

/// A serving node on loopback that pins the parties this test uses, with
/// its configuration open to a test that needs a particular refusal.
fn serving_with(name: &str, tweak: impl FnOnce(&mut NodeConfig)) -> Arc<LiveNode> {
    let me = Arc::new(sid(name));
    let pins = Pins::new();
    let mut known = Vec::new();
    for n in ["alice", "bob", "carol"] {
        pins.pin_identity(&sid(n).public);
        known.push(sid(n).public);
    }
    let p = Path::from_indices(&[]);
    let view = NodeView::new(
        me.clone(),
        rhtn_archive::tx::Locator {
            anchor: me.public.keyhash,
            path: p.bytes,
            nibbles: p.nibbles,
            seqno: rhtn_archive::tx::Seqno {
                series: 1,
                counter: 0,
            },
            subnet: Some(SUBNET),
        },
    );
    // alice and carol are bob's own light clients: in its subtree, so
    // its sessions with them are primary (`wire-format.md` §8.2)
    let mut view = view;
    view.set_slot(0, Some(sid("alice").public.keyhash), 1_800_000_000);
    view.set_slot(1, Some(sid("carol").public.keyhash), 1_800_000_000);
    let mut cfg = NodeConfig::defaults(me, pins, 30);
    cfg.log = Log::recording();
    tweak(&mut cfg);
    LiveNode::start(
        cfg,
        view,
        known,
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    )
}

/// A serving node on loopback that pins the parties this test uses.
fn serving(name: &str) -> Arc<LiveNode> {
    let me = Arc::new(sid(name));
    let pins = Pins::new();
    let mut known = Vec::new();
    for n in ["alice", "bob", "carol"] {
        pins.pin_identity(&sid(n).public);
        known.push(sid(n).public);
    }
    let p = Path::from_indices(&[]);
    let view = NodeView::new(
        me.clone(),
        rhtn_archive::tx::Locator {
            anchor: me.public.keyhash,
            path: p.bytes,
            nibbles: p.nibbles,
            seqno: rhtn_archive::tx::Seqno {
                series: 1,
                counter: 0,
            },
            subnet: Some(SUBNET),
        },
    );
    // alice and carol are bob's own light clients: in its subtree, so
    // its sessions with them are primary (`wire-format.md` §8.2)
    let mut view = view;
    view.set_slot(0, Some(sid("alice").public.keyhash), 1_800_000_000);
    view.set_slot(1, Some(sid("carol").public.keyhash), 1_800_000_000);
    let mut cfg = NodeConfig::defaults(me, pins, 30);
    cfg.log = Log::recording();
    LiveNode::start(
        cfg,
        view,
        known,
        AnchorTable::new(0, Ingestion::UnverifiedGossip),
    )
}

// acceptance: DMN-13
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_kernel_holds_the_session_and_no_wire_byte_crosses_outward() {
    let node = serving("bob");
    let addr = node.addr.to_string();
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| material(n))
        .collect();

    // a shell calls from its own thread, which is what a shell is
    let p = Arc::new(
        tokio::task::spawn_blocking({
            let known = known.clone();
            move || {
                Participant::start(
                    seeds("alice"),
                    known,
                    platform_of(Arc::new(Shell {
                        ..Default::default()
                    })),
                )
                .expect("starts")
            }
        })
        .await
        .unwrap(),
    );
    assert!(!p.attached(), "nothing is attached before an attach");

    // the attach happens here, over this side's own session: what comes
    // back is what a screen shows
    let a = {
        let (q, addr) = (p.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![id("carol")]))
            .await
            .unwrap()
            .expect("attaches")
    };
    assert_eq!(a.serving, id("bob"));
    assert!(a.primary, "the patron itself, not a sibling");
    assert_eq!(a.queued, 0);
    assert!(p.attached());

    // and the node holds the session the kernel opened, which no shell
    // carried for it — and nothing published with it: a node makes its
    // keys for a client when a sender asks (`wire-format.md` §7.8), and an
    // attach hands over no key material at all
    assert!(
        node.node.sessions().contains(&sid("alice").public.keyhash),
        "the session went over the wire from the kernel"
    );
    assert!(
        node.view.lock().unwrap().node_keys.recipients().is_empty(),
        "and no key was made for anyone by the attach"
    );

    // where to be rung is the user's choice, handed over and withdrawn
    // through the same seam
    let carol = sid("carol").public.keyhash;
    let alice = sid("alice").public.keyhash;
    {
        let q = p.clone();
        tokio::task::spawn_blocking(move || {
            q.wake(Some(Wake {
                url: "https://push.example/rhtn/a3f9".into(),
                key: vec![7; 32],
                signing_key: vec![3; 32],
                lapses_at: None,
            }))
        })
        .await
        .unwrap()
        .expect("registers");
    }
    assert_eq!(
        node.view
            .lock()
            .unwrap()
            .wake
            .get(&alice)
            .map(|e| e.url.clone()),
        Some("https://push.example/rhtn/a3f9".to_string())
    );
    {
        let q = p.clone();
        tokio::task::spawn_blocking(move || q.wake(None))
            .await
            .unwrap()
            .expect("withdraws");
    }
    assert!(
        node.view.lock().unwrap().wake.get(&alice).is_none(),
        "a withdrawal is as sayable as a registration"
    );

    // payload reaches another client of the same node, through the kernel
    // on both sides: alice holds carol's card, which carries the key a
    // message to her is sealed to, carol attaches, and what alice sends
    // comes out of carol's event queue already opened and attributed
    let carol_side = Arc::new(
        tokio::task::spawn_blocking({
            let known = known.clone();
            move || {
                Participant::start(
                    seeds("carol"),
                    known,
                    platform_of(Arc::new(Shell::default())),
                )
                .expect("starts")
            }
        })
        .await
        .unwrap(),
    );
    {
        let (q, intro) = (
            p.clone(),
            carol_side.own_introduction().expect("carol's card"),
        );
        tokio::task::spawn_blocking(move || q.take_introduction(intro))
            .await
            .unwrap()
            .expect("alice holds carol's card");
    }
    {
        let (q, addr) = (carol_side.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![]))
            .await
            .unwrap()
            .expect("carol attaches");
    }
    {
        let q = p.clone();
        tokio::task::spawn_blocking(move || {
            q.send(id("carol"), KIND_APPLICATION, b"for carol".to_vec())
        })
        .await
        .unwrap()
        .expect("sent");
    }
    let got = {
        let q = carol_side.clone();
        tokio::task::spawn_blocking(move || q.next_event(5000))
            .await
            .unwrap()
    };
    assert_eq!(
        got,
        Some(Event::Payload {
            from: id("alice"),
            kind: 0,
            bytes: b"for carol".to_vec()
        }),
        "decrypted here, and the ciphertext never left"
    );
    let _ = carol;

    // maintenance is a conversation with a node, and refused where there
    // is none
    {
        let q = p.clone();
        tokio::task::spawn_blocking(move || q.maintain())
            .await
            .unwrap()
            .expect("maintains");
    }
    let bare = tokio::task::spawn_blocking({
        let known = known.clone();
        move || {
            Participant::start(seeds("bob"), known, platform_of(Arc::new(Shell::default())))
                .expect("starts")
        }
    })
    .await
    .unwrap();
    let e = tokio::task::spawn_blocking(move || bare.maintain())
        .await
        .unwrap()
        .unwrap_err();
    assert!(e.reason().contains("no serving node is attached"), "{e}");

    // an address that is not one is a refusal carrying its reason, and an
    // attach with none is refused before anything is dialled
    let q = p.clone();
    let e = tokio::task::spawn_blocking(move || {
        q.attach(id("bob"), vec!["not an address".into()], vec![])
    })
    .await
    .unwrap()
    .unwrap_err();
    assert!(e.reason().contains("is not an address"), "{e}");
    let q = p.clone();
    let e = tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![], vec![]))
        .await
        .unwrap()
        .unwrap_err();
    assert!(e.reason().contains("at least one address"), "{e}");

    // nothing arrived for this client, and asking says so rather than
    // blocking forever. **The kernel's own notice of what changed is not
    // an arrival**: the attach marked the nouns it touched, and that
    // comes up the same stream once, as the model's names and no byte of
    // the wire
    let q = p.clone();
    let quiet = tokio::task::spawn_blocking(move || q.next_event(200))
        .await
        .unwrap();
    assert!(
        matches!(quiet, None | Some(rhtn_ffi::net::Event::Changed { .. })),
        "nothing but the kernel's own notice: {quiet:?}"
    );
    let q = p.clone();
    assert!(
        tokio::task::spawn_blocking(move || q.next_event(200))
            .await
            .unwrap()
            .is_none(),
        "and then nothing at all"
    );
}

// functional: PAY-009
// acceptance: DMN-15
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_message_the_serving_node_refused_reaches_the_application_as_unsent() {
    // the node takes nothing for anyone: a queue cap of zero refuses the
    // newest and tells the sender (design §14.1.6)
    let node = serving_with("bob", |c| c.queue_cap = Some(0));
    let addr = node.addr.to_string();
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| material(n))
        .collect();

    let carol = Arc::new(
        tokio::task::spawn_blocking({
            let known = known.clone();
            move || {
                Participant::start(
                    seeds("carol"),
                    known,
                    platform_of(Arc::new(Shell::default())),
                )
                .expect("starts")
            }
        })
        .await
        .unwrap(),
    );
    let alice = Arc::new(
        tokio::task::spawn_blocking({
            let known = known.clone();
            move || {
                Participant::start(
                    seeds("alice"),
                    known,
                    platform_of(Arc::new(Shell {
                        ..Default::default()
                    })),
                )
                .expect("starts")
            }
        })
        .await
        .unwrap(),
    );
    // alice holds carol's card, and both attach: the node serves carol
    // and will queue nothing for her
    {
        let (q, intro) = (
            alice.clone(),
            carol.own_introduction().expect("carol's card"),
        );
        tokio::task::spawn_blocking(move || q.take_introduction(intro))
            .await
            .unwrap()
            .expect("alice holds carol's card");
    }
    {
        let (q, a) = (carol.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![a], vec![]))
            .await
            .unwrap()
            .expect("carol attaches");
    }
    {
        let (q, a) = (alice.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![a], vec![id("carol")]))
            .await
            .unwrap()
            .expect("alice attaches");
    }
    assert!(
        node.node.serves(&sid("carol").public.keyhash),
        "control: the node serves the recipient"
    );

    let before = node.node.queued(&sid("carol").public.keyhash);
    let q = alice.clone();
    let e = tokio::task::spawn_blocking(move || {
        q.send(id("carol"), KIND_APPLICATION, b"for carol".to_vec())
    })
    .await
    .unwrap()
    .expect_err("the node refused it, and the application is told");
    assert!(
        e.reason().contains("unsent"),
        "the reason says the work was not done: {e}"
    );
    assert_eq!(
        node.node.queued(&sid("carol").public.keyhash),
        before,
        "and nothing was queued"
    );
}

/// A desktop of alice's that holds no seed (design §23.3): started from
/// her key material, a transport seed of its own and the run her phone
/// delegated over it, and handed her two encryption secrets; it attaches
/// under the delegation, what was sealed to alice while it was away opens
/// there, attributed to its sender, and what her key signs is refused on
/// it.
// acceptance: DMN-27
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_device_holding_no_seed_attaches_under_its_delegation_and_opens_what_is_sealed_to_the_person()
 {
    let node = serving("bob");
    let addr = node.addr.to_string();
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| material(n))
        .collect();
    let shell = || {
        platform_of(Arc::new(Shell {
            real_time: true,
            ..Default::default()
        }))
    };

    // the phone holds the seed; the desktop mints a transport key and the
    // phone delegates over its public half, as a provisioning channel
    // would carry it
    let phone = Arc::new({
        let known = known.clone();
        tokio::task::spawn_blocking(move || Participant::start(seeds("alice"), known, shell()))
            .await
            .unwrap()
            .expect("the phone starts")
    });
    assert!(phone.holds_seed());
    let transport_seed = vec![0x51u8; 32];
    let presented = rhtn_transport::tls::Credential::from_seed(
        transport_seed.as_slice().try_into().unwrap(),
        sid("alice").public.keyhash,
    )
    .public();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let run = {
        let (q, key) = (phone.clone(), presented.to_vec());
        tokio::task::spawn_blocking(move || q.delegate(key, now - 60, 2))
            .await
            .unwrap()
            .expect("a run of two")
    };
    assert_eq!(run.len(), 2);
    assert!(
        phone.delegate(vec![1; 31], now, 1).is_err(),
        "a transport key is 32 bytes"
    );

    // no delegation, no device: the constructor refuses rather than
    // starting something that could bind to nothing
    assert!(
        Participant::start_delegated(
            material("alice"),
            transport_seed.clone(),
            vec![],
            known.clone(),
            shell()
        )
        .is_err()
    );
    // a delegation by bob over this key is not alice's device's to hold
    let bobs = {
        let known = known.clone();
        let bob =
            tokio::task::spawn_blocking(move || Participant::start(seeds("bob"), known, shell()))
                .await
                .unwrap()
                .unwrap();
        bob.delegate(presented.to_vec(), now - 60, 1).unwrap()
    };
    assert!(
        Participant::start_delegated(
            material("alice"),
            transport_seed.clone(),
            bobs,
            known.clone(),
            shell()
        )
        .is_err()
    );

    let desktop = Arc::new({
        let (known, run, seed) = (known.clone(), run.clone(), transport_seed.clone());
        tokio::task::spawn_blocking(move || {
            Participant::start_delegated(material("alice"), seed, run, known, shell())
        })
        .await
        .unwrap()
        .expect("the desktop starts")
    });
    assert!(!desktop.holds_seed());
    assert_eq!(desktop.presented_key(), presented.to_vec());
    assert_eq!(
        phone.presented_key(),
        sid("alice").public.ed.as_bytes().to_vec(),
        "the phone presents the identity's classical member"
    );
    assert!(
        desktop.sign_body(b"a body".to_vec()).is_err(),
        "the identity key is not here"
    );

    // **a message sealed to alice waits at the node**: carol holds her
    // card and sends while no device of alice's is attached
    let carol = Arc::new({
        let known = known.clone();
        tokio::task::spawn_blocking(move || Participant::start(seeds("carol"), known, shell()))
            .await
            .unwrap()
            .expect("carol starts")
    });
    {
        let (q, intro) = (
            carol.clone(),
            phone.own_introduction().expect("alice's card"),
        );
        tokio::task::spawn_blocking(move || q.take_introduction(intro))
            .await
            .unwrap()
            .expect("carol holds alice's card");
    }
    {
        let (q, addr) = (carol.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![id("alice")]))
            .await
            .unwrap()
            .expect("carol attaches");
    }
    {
        let q = carol.clone();
        tokio::task::spawn_blocking(move || {
            q.send(id("alice"), KIND_APPLICATION, b"for alice".to_vec())
        })
        .await
        .unwrap()
        .expect("sealed to alice and taken");
    }
    assert!(
        node.node.queued(&sid("alice").public.keyhash) >= 1,
        "waiting, with carol's offer of the direct path behind it"
    );

    // the phone's two encryption secrets, as the pairing's credential
    // carries them (`wire-format.md` §14.3.3): what alice's phone derives
    // from her root, and nothing the desktop could derive itself
    let secret =
        rhtn_crypto::seal::EncryptionSecret::from_identity_root(sid("alice").identity_root());
    let (kem_seed, auth) = (secret.kem_seed().to_vec(), secret.auth_secret().to_vec());
    assert!(
        desktop
            .take_encryption_secrets(vec![0; 31], auth.clone())
            .is_err(),
        "an X-Wing seed is 32 bytes"
    );
    desktop
        .take_encryption_secrets(kem_seed, auth)
        .expect("the desktop holds alice's secrets");

    // the attach binds under the delegation: the node pinned alice, and
    // the key presented is the one the delegation names
    let a = {
        let (q, addr) = (desktop.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![]))
            .await
            .unwrap()
            .expect("attaches under the delegation")
    };
    assert_eq!(a.serving, id("bob"));
    assert!(a.queued >= 1, "the node says it is waiting");
    assert!(desktop.attached());
    assert!(
        node.node.sessions().contains(&sid("alice").public.keyhash),
        "the session is alice's"
    );
    // and the queued message is handed over, opened, and carol's
    let got = {
        let q = desktop.clone();
        tokio::task::spawn_blocking(move || {
            loop {
                match q.next_event(5000) {
                    Some(Event::Changed { .. }) | Some(Event::Connection(_)) => continue,
                    other => break other,
                }
            }
        })
        .await
        .unwrap()
    };
    assert_eq!(
        got,
        Some(Event::Payload {
            from: id("carol"),
            kind: KIND_APPLICATION,
            bytes: b"for alice".to_vec()
        }),
        "opened on the desktop under alice's secrets, attributed to carol"
    );
}

// functional: PAY-015
/// A process restart with a conversation under way and a message queued
/// meanwhile: the kernel starts from what it wrote through the storage
/// seam, the queued message opens on the restored state, attributed to
/// its sender, and the reply goes as before.  State that does not open
/// refuses the start; a backup made here restores into an empty kernel.
// acceptance: DMN-13
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_kernel_restarts_from_the_storage_seam_with_its_session_and_its_queue_intact() {
    let node = serving("bob");
    let addr = node.addr.to_string();
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| material(n))
        .collect();
    let carol_shell = Arc::new(Shell::default());
    let start = |name: &'static str, shell: Arc<Shell>| {
        let known = known.clone();
        tokio::task::spawn_blocking(move || {
            Participant::start(seeds(name), known, platform_of(shell)).expect("starts")
        })
    };
    let alice = Arc::new(start("alice", Arc::new(Shell::default())).await.unwrap());
    let carol = Arc::new(start("carol", carol_shell.clone()).await.unwrap());
    assert!(
        carol_shell.read("payload".into()).is_none(),
        "nothing written before anything happened"
    );
    let attach = |p: Arc<Participant>, pop: Vec<Vec<u8>>| {
        let addr = addr.clone();
        tokio::task::spawn_blocking(move || p.attach(id("bob"), vec![addr], pop))
    };
    // each holds the other's card, as two who have met do
    for (p, q) in [(&alice, &carol), (&carol, &alice)] {
        let (p, intro) = (p.clone(), q.own_introduction().expect("a card"));
        tokio::task::spawn_blocking(move || p.take_introduction(intro))
            .await
            .unwrap()
            .expect("taken");
    }
    attach(carol.clone(), vec![])
        .await
        .unwrap()
        .expect("carol attaches");
    attach(alice.clone(), vec![id("carol")])
        .await
        .unwrap()
        .expect("alice attaches");
    attach(carol.clone(), vec![id("alice")])
        .await
        .unwrap()
        .expect("carol sweeps");
    let on_disk = carol_shell
        .read("payload".into())
        .expect("an attach writes the state");
    assert!(
        rhtn_client::sealed::is_sealed(&on_disk),
        "what reaches the platform's storage is sealed, not the kernel's bytes"
    );

    // a message each way
    let send = |p: Arc<Participant>, to: Vec<u8>, what: &'static [u8]| {
        tokio::task::spawn_blocking(move || p.send(to, KIND_APPLICATION, what.to_vec()))
    };
    let next = |p: Arc<Participant>| tokio::task::spawn_blocking(move || p.next_event(5000));
    send(alice.clone(), id("carol"), b"first")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(carol.clone()).await.unwrap(),
        Some(Event::Payload {
            from: id("alice"),
            kind: 0,
            bytes: b"first".to_vec()
        })
    );
    send(carol.clone(), id("alice"), b"second")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Payload {
            from: id("carol"),
            kind: 0,
            bytes: b"second".to_vec()
        })
    );
    // carol's process ends: detached cleanly, then gone
    {
        let q = carol.clone();
        tokio::task::spawn_blocking(move || q.detach())
            .await
            .unwrap()
            .expect("detaches");
    }
    assert!(!carol.attached());
    drop(carol);
    // what the process left behind, exactly: a seal carries a fresh nonce
    // per write, so byte-equality is asserted across the restart alone
    let written = carol_shell.read("payload".into()).unwrap();
    // meanwhile alice sends: the node queues it for carol's device
    let before = node.node.queued(&sid("carol").public.keyhash);
    send(alice.clone(), id("carol"), b"third")
        .await
        .unwrap()
        .expect("queued at the node");
    assert_eq!(
        node.node.queued(&sid("carol").public.keyhash),
        before + 1,
        "queued for the person, who is away"
    );

    // a new process from the same storage: the queued message opens on
    // what it read back, and the reply goes
    let carol2 = Arc::new(start("carol", carol_shell.clone()).await.unwrap());
    assert_eq!(
        carol_shell.read("payload".into()).unwrap(),
        written,
        "starting rewrote nothing"
    );
    let a = attach(carol2.clone(), vec![])
        .await
        .unwrap()
        .expect("carol attaches again");
    assert_eq!(a.queued, 1, "the node says one is waiting");
    assert_eq!(
        next(carol2.clone()).await.unwrap(),
        Some(Event::Payload {
            from: id("alice"),
            kind: 0,
            bytes: b"third".to_vec()
        }),
        "opened under the state the first process wrote"
    );
    send(carol2.clone(), id("alice"), b"fourth")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Payload {
            from: id("carol"),
            kind: 0,
            bytes: b"fourth".to_vec()
        }),
        "and the conversation continues"
    );

    // state that does not open refuses the start rather than starting
    // fresh over it
    let broken = Arc::new(Shell::default());
    broken.write("client".into(), b"not a state".to_vec());
    let e = {
        let known = known.clone();
        tokio::task::spawn_blocking(move || {
            Participant::start(seeds("carol"), known, platform_of(broken)).err()
        })
        .await
        .unwrap()
        .expect("refused")
    };
    assert!(e.reason().contains("does not open"), "{e}");

    // a backup made by alice restores into an empty kernel and is written
    // to its storage; into one that has records it would be refused, which
    // the client's own tests show
    let blob = {
        let q = alice.clone();
        tokio::task::spawn_blocking(move || q.export_backup())
            .await
            .unwrap()
            .expect("exports")
    };
    let empty_shell = Arc::new(Shell::default());
    let alice2 = Arc::new(start("alice", empty_shell.clone()).await.unwrap());
    let r = {
        let (q, blob) = (alice2.clone(), blob.clone());
        tokio::task::spawn_blocking(move || q.restore_backup(blob))
            .await
            .unwrap()
            .expect("restores")
    };
    assert_eq!(
        r,
        rhtn_ffi::client::Restored {
            records: 0,
            discarded: 0
        }
    );
    assert!(empty_shell.read("payload".into()).is_some());
    // and another identity's kernel holds another key, so the blob does
    // not open there: the identity is the key (design §13.7.1)
    let bob_shell = Arc::new(Shell::default());
    let bob2 = Arc::new(start("bob", bob_shell).await.unwrap());
    let e = {
        let q = bob2.clone();
        tokio::task::spawn_blocking(move || q.restore_backup(blob))
            .await
            .unwrap()
            .expect_err("another identity's key opens nothing")
    };
    assert!(e.reason().contains("did not unwrap"), "{e}");
    // **and nothing in the run wrote the one blob**: a save and the
    // after-arrivals hook both write relations, so the old name is never
    // written on a store that never held it
    let blob_writes = carol_shell
        .writes
        .lock()
        .unwrap()
        .iter()
        .filter(|w| *w == "client")
        .count();
    assert_eq!(blob_writes, 0, "the one blob is not written by any path");
}

/// **A party the kernel learned is pinned for the transport, and still
/// after a restart** (`wire-format.md` §9.1: a dial binds by the pin, in
/// both directions).  Carol starts knowing nobody but the node and takes
/// alice's introduction; alice's dial on her holds.  Her process is
/// replaced by one started from what it wrote, still knowing nobody but
/// the node: the kernel re-learns alice from the contacts, the transport
/// with it, and alice's dial holds again.  Where it did not, a peer's
/// every dial after a restart was closed *unbound* and none was made
/// back [bench `9ea2c16-model-validation-1`, 2026-10-30].
// acceptance: TRN-28
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_party_the_kernel_learned_binds_a_direct_dial_across_a_restart() {
    let node = serving("bob");
    let addr = node.addr.to_string();
    let start = |name: &'static str, known: Vec<Vec<u8>>, shell: Arc<Shell>| {
        tokio::task::spawn_blocking(move || {
            Participant::start(seeds(name), known, platform_of(shell)).expect("starts")
        })
    };
    // alice knows carol from the start; carol knows only the node
    let alice = Arc::new(
        start(
            "alice",
            vec![material("alice"), material("bob"), material("carol")],
            Arc::new(Shell::default()),
        )
        .await
        .unwrap(),
    );
    let carol_shell = Arc::new(Shell::default());
    let carol = Arc::new(
        start("carol", vec![material("bob")], carol_shell.clone())
            .await
            .unwrap(),
    );
    let attach = |p: Arc<Participant>, pop: Vec<Vec<u8>>| {
        let addr = addr.clone();
        tokio::task::spawn_blocking(move || p.attach(id("bob"), vec![addr], pop))
    };
    attach(alice.clone(), vec![])
        .await
        .unwrap()
        .expect("alice attaches");
    attach(carol.clone(), vec![])
        .await
        .unwrap()
        .expect("carol attaches");
    // each takes the other's introduction: a contact both ways, which is
    // what the direct path is gated on, and for carol the first she hears
    // of alice's material
    let intro = |p: Arc<Participant>| {
        tokio::task::spawn_blocking(move || p.own_introduction().expect("attached, so a locator"))
    };
    let take = |p: Arc<Participant>, i: Vec<u8>| {
        tokio::task::spawn_blocking(move || p.take_introduction(i).expect("taken"))
    };
    let from_alice = intro(alice.clone()).await.unwrap();
    let from_carol = intro(carol.clone()).await.unwrap();
    assert_eq!(take(carol.clone(), from_alice).await.unwrap(), id("alice"));
    assert_eq!(take(alice.clone(), from_carol).await.unwrap(), id("carol"));
    // carol's first message offers her candidates over the relay; alice
    // dials them, and carol binds the key alice presents by the pin she
    // took a moment ago, not at start
    let send = |p: Arc<Participant>, to: Vec<u8>, what: &'static [u8]| {
        tokio::task::spawn_blocking(move || p.send(to, KIND_APPLICATION, what.to_vec()))
    };
    let next = |p: Arc<Participant>| tokio::task::spawn_blocking(move || p.next_event(5000));
    async fn held(p: Arc<Participant>, peer: Vec<u8>) -> bool {
        for _ in 0..50 {
            if p.direct_to(peer.clone()) {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        false
    }
    send(carol.clone(), id("alice"), b"first")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Payload {
            from: id("carol"),
            kind: 0,
            bytes: b"first".to_vec()
        })
    );
    assert!(
        held(carol.clone(), id("alice")).await,
        "carol holds the path alice opened: alice's key bound by the pin"
    );
    assert!(held(alice.clone(), id("carol")).await, "and alice holds it");
    // carol's process ends; another starts from what it wrote, handed the
    // node's material and nobody else's
    {
        let q = carol.clone();
        tokio::task::spawn_blocking(move || q.detach())
            .await
            .unwrap()
            .expect("detaches");
    }
    drop(carol);
    let carol2 = Arc::new(
        start("carol", vec![material("bob")], carol_shell.clone())
            .await
            .unwrap(),
    );
    attach(carol2.clone(), vec![])
        .await
        .unwrap()
        .expect("carol attaches again");
    send(carol2.clone(), id("alice"), b"second")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Payload {
            from: id("carol"),
            kind: 0,
            bytes: b"second".to_vec()
        })
    );
    assert!(
        held(carol2.clone(), id("alice")).await,
        "the new process binds alice's dial by the pin it refilled from its contacts"
    );
    assert!(
        held(alice.clone(), id("carol")).await,
        "and alice holds the new path"
    );
}

// functional: APP-011
#[test]
fn state_written_before_the_kernel_sealed_is_read_and_resealed() {
    // a device from before the seam: the platform declared unsealed, so the
    // kernel's bytes reached the disk as written
    let before = Arc::new(Shell {
        declared_unsealed: true,
        ..Default::default()
    });
    let known = vec![material("alice")];
    let p = Participant::start(seeds("alice"), known.clone(), platform_of(before.clone()))
        .expect("starts unsealed, by declaration");
    p.save().expect("saves");
    let plain = before.read("payload".into()).expect("state written");
    assert!(
        !rhtn_client::sealed::is_sealed(&plain),
        "a declared-unsealed platform holds the kernel's bytes as written"
    );
    drop(p);

    // the same files under a platform that now keeps a key: the older shape
    // carries no header, is read as it always was, and the next write seals
    let after = Arc::new(Shell {
        store: Mutex::new(before.store.lock().unwrap().clone()),
        ..Default::default()
    });
    let p = Participant::start(seeds("alice"), known, platform_of(after.clone()))
        .expect("pre-seam state opens");
    assert!(
        after.custody.lock().unwrap().is_some(),
        "a key was minted and kept"
    );
    p.save().expect("saves");
    let now = after.read("payload".into()).expect("state written");
    assert!(
        rhtn_client::sealed::is_sealed(&now),
        "the first write after the seam seals"
    );
}

// functional: APP-011
#[test]
fn failed_custody_refuses_the_start_rather_than_writing_plain() {
    let shell = Arc::new(Shell {
        keeps_nothing: true,
        ..Default::default()
    });
    let refused = Participant::start(
        seeds("alice"),
        vec![material("alice")],
        platform_of(shell.clone()),
    );
    assert!(
        refused.is_err(),
        "unsealed is declared, never fallen back to"
    );
    assert!(
        shell.store.lock().unwrap().is_empty(),
        "and nothing reached the disk on the way down"
    );
}

// functional: APP-011
#[test]
fn anothers_key_does_not_open_the_state_and_the_start_is_refused() {
    let theirs = Arc::new(Shell::default());
    let known = vec![material("alice")];
    let p = Participant::start(seeds("alice"), known.clone(), platform_of(theirs.clone()))
        .expect("starts");
    p.save().expect("saves");
    drop(p);

    let wrong = Arc::new(Shell {
        store: Mutex::new(theirs.store.lock().unwrap().clone()),
        custody: Mutex::new(Some(vec![9u8; 32])),
        ..Default::default()
    });
    let refused = Participant::start(seeds("alice"), known, platform_of(wrong));
    assert!(
        refused.is_err(),
        "sealed state that does not open refuses the start rather than beginning fresh over it"
    );
}

/// **The authority calls a shell needs, over the boundary**: what this key
/// holds, what ending one does, what its own policy makes of a party, and
/// the rotation surface before there is anything to rotate.
///
/// The positive departure and the positive standing are proved where the
/// evidence can exist, across four `rhtnp` processes (`rhtn-participant`'s
/// live tests). What is here is the half a device can be in without a
/// ceremony behind it, and every answer of it is a value rather than a
/// panic: a fresh client is asked all twelve and refuses by name.
#[test]
fn authority_recovery_and_standing_answer_before_there_is_anything_to_answer_about() {
    let shell = Arc::new(Shell::default());
    let known: Vec<Vec<u8>> = ["alice", "bob"].iter().map(|n| material(n)).collect();
    let p = Participant::start(seeds("alice"), known, platform_of(shell.clone())).expect("starts");

    // ADOPTED BY NOBODY, so there is no line and nothing to leave.
    assert!(p.lines().is_empty(), "no patron, no relationship");
    let no_line = p.depart(id("bob"), None).expect_err("nothing to leave");
    assert!(
        no_line.reason().contains("NoRelationship"),
        "the refusal names what it could not find: {}",
        no_line.reason()
    );

    // **STANDING IS COMPUTED, NEVER STORED** (design §16.2): a party this
    // client has no evidence about still gets an answer, and the answer is
    // a number rather than an absence.
    let s = p.standing(id("bob"));
    assert!(s.is_finite(), "a score, not a refusal: {s}");
    assert_eq!(
        p.standing(vec![1, 2, 3]),
        0.0,
        "and a thing that is not a keyhash is nobody"
    );

    // one computation over a set, which answers for what it was given
    let e = p.evaluate(vec![id("bob"), id("alice")]);
    assert_eq!(e.individual.len(), 2, "one score per candidate");
    assert!(e.joint.is_finite(), "and a joint figure for the set");
    assert!(
        e.individual
            .iter()
            .all(|i| i.who == id("bob") || i.who == id("alice")),
        "about the two it was asked about and nobody else"
    );
    assert!(
        p.evaluate(vec![vec![9, 9]]).individual.is_empty(),
        "a candidate that is not a keyhash is not scored"
    );

    // **THE RECOVERY SURFACE, WITH NO PRIOR KEY.** A device that is not
    // a rotating device is an ordinary device, and says so on every call rather
    // than half-answering.
    assert_eq!(p.prior_key(), None, "nothing is being succeeded");
    assert!(p.seal_old_lines().is_empty(), "no old key, no last act");
    assert!(p.present_chain(id("bob")).is_empty(), "no line, no chain");
    let no_prior = p.recovery_block(id("bob")).expect_err("nothing to recover");
    assert!(
        no_prior.reason().contains("NoPriorKey"),
        "and it says which: {}",
        no_prior.reason()
    );

    // a verifier who never met the key claimed has nothing to recognise
    let unmet = p.recovery_query(id("bob")).expect_err("never met them");
    assert!(
        unmet.reason().contains("NotRecognised"),
        "refused for the reason it is refused: {}",
        unmet.reason()
    );

    // and the two that take bytes refuse what is not theirs
    assert!(p.recognise(vec![0, 1, 2], vec![], id("bob")).is_err());
    assert!(p.take_recovery_response(vec![0, 1, 2]).is_err());

    // **WHAT LEAVING COSTS, ASKED BEFORE LEAVING.** The same refusal the
    // departure gives, because a cost quoted for a departure that would
    // not happen is worse than no cost at all.
    let no_cost = p
        .cost_of_leaving(id("bob"))
        .expect_err("nothing to leave, nothing to cost");
    assert!(
        no_cost.reason().contains("NoRelationship"),
        "refused by the same name: {}",
        no_cost.reason()
    );

    // every call that takes a keyhash says so when it is handed something
    // else, rather than treating the first 32 bytes of whatever arrived
    for r in [
        p.depart(vec![7], None).err(),
        p.cost_of_leaving(vec![7]).err(),
        p.recovery_query(vec![7]).err(),
        p.recovery_block(vec![7]).err(),
        p.recognise(vec![0], vec![], vec![7]).err(),
    ] {
        let r = r.expect("a short id is refused");
        assert!(r.reason().contains("32 bytes"), "{}", r.reason());
    }
}

/// **The third door of the first run** (A1): a key succeeding another one.
///
/// The prior key is required, because `wire-format.md` §4.1 admits no
/// variant of the `Recovery` block — both halves or nothing — and the
/// archive is not, because `light-client-requirements.md` §6 has a rotation
/// restoring standing and not history. So this proves the two cases apart:
/// a succession opens with the person's own backup and equally without
/// one, and a backup that is not the succeeded key's is refused however it
/// opened.
///
/// **What it does not prove is that records load**, because a key that has
/// met nobody backs up an archive with nothing in it, and a ceremony is
/// not reachable from here. That path is `install`'s own, tested where it
/// lives (ARC-24); what is new here and is tested is the owner check
/// against the *prior* key rather than against this client's own.
#[test]
fn a_key_succeeds_another_with_its_backup_and_without_one() {
    let known: Vec<Vec<u8>> = ["alice", "bob"].iter().map(|n| material(n)).collect();

    // ALICE, BEFORE ANY OF THIS, writes her own backup
    let first = Arc::new(Shell::default());
    let alice =
        Participant::start(seeds("alice"), known.clone(), platform_of(first)).expect("starts");
    let blob = alice.export_backup().expect("a backup of her own");
    drop(alice);

    // A NEW KEY SUCCEEDS HERS, with that backup in hand
    let shell = Arc::new(Shell::default());
    let heir = Participant::rotating(
        seeds("bob"),
        seeds("alice"),
        Some(blob.clone()),
        known.clone(),
        platform_of(shell),
    )
    .expect("succeeds alice");
    assert_eq!(
        heir.prior_key(),
        Some(id("alice")),
        "the key being succeeded, which the Recovery block names"
    );
    assert_eq!(
        heir.me(),
        id("bob"),
        "and this device's own key is the new one"
    );
    // nothing has recognised her yet, so there is no block to carry
    let no_match = match heir.recovery_block(id("bob")) {
        Err(e) => e,
        Ok(_) => panic!("a recovery with no recognition in it must not build"),
    };
    assert!(
        no_match.reason().contains("NotRecognised"),
        "a recovery with no match in it is a claim nobody made"
    );

    // AND WITHOUT A BACKUP the succession still stands: standing is what
    // comes back, history is what does not
    let bare = Participant::rotating(
        seeds("bob"),
        seeds("alice"),
        None,
        known.clone(),
        platform_of(Arc::new(Shell::default())),
    )
    .expect("succeeds her with nothing of hers");
    assert_eq!(
        bare.prior_key(),
        Some(id("alice")),
        "the same key succeeded"
    );
    assert!(
        bare.seal_old_lines().is_empty(),
        "and no line to seal, there being no archive to find one in"
    );
    assert!(
        heir.seal_old_lines().is_empty(),
        "nor in hers, she having been adopted by nobody before she backed up"
    );

    // A BACKUP THAT IS NOT THE SUCCEEDED KEY'S is refused however it
    // opened: opening says which key the reader holds, not which wrote it
    let other = Participant::start(
        seeds("bob"),
        known.clone(),
        platform_of(Arc::new(Shell::default())),
    )
    .expect("starts");
    let bobs = other.export_backup().expect("bob's own");
    drop(other);
    let wrong = Participant::rotating(
        seeds("carol"),
        seeds("alice"),
        Some(bobs),
        known.clone(),
        platform_of(Arc::new(Shell::default())),
    );
    let wrong = match wrong {
        Err(e) => e,
        Ok(_) => panic!("a backup that is not the succeeded key's must not install"),
    };
    assert!(
        !wrong.reason().is_empty(),
        "and the refusal says something: {}",
        wrong.reason()
    );

    // a prior seed of the wrong measure is refused by name, as the new
    // identity's own is at `start`
    let short = Participant::rotating(
        seeds("bob"),
        vec![1, 2, 3],
        None,
        known,
        platform_of(Arc::new(Shell::default())),
    );
    let short = match short {
        Err(e) => e,
        Ok(_) => panic!("a prior seed of the wrong measure must be refused"),
    };
    assert!(short.reason().contains("64 bytes"), "{}", short.reason());
}

/// **The kernel seals the application's own files** (APP-015): bytes
/// sealed under a label open under that label and no other, never under
/// the storage key the custody keeps, and a platform that declared itself
/// unsealed gets its bytes back as they were.
// functional: APP-015
#[test]
fn the_application_seals_its_own_files_through_the_kernel_under_a_key_it_never_sees() {
    let shell = Arc::new(Shell {
        ..Default::default()
    });
    let known: Vec<Vec<u8>> = ["alice", "bob"].iter().map(|n| material(n)).collect();
    let p = Participant::start(seeds("alice"), known.clone(), platform_of(shell.clone()))
        .expect("starts");
    let plain = b"the first line of a thread".to_vec();
    let sealed = p.seal("threads/bob".into(), plain.clone());
    assert_ne!(sealed, plain);
    assert!(
        rhtn_client::sealed::is_sealed(&sealed),
        "the kernel's own construction"
    );
    assert_eq!(
        p.unseal("threads/bob".into(), sealed.clone())
            .expect("opens"),
        plain
    );
    assert!(
        p.unseal("threads/carol".into(), sealed.clone()).is_err(),
        "another label refuses: a blob cannot be moved"
    );
    let mut bent = sealed.clone();
    let last = bent.len() - 1;
    bent[last] ^= 1;
    assert!(
        p.unseal("threads/bob".into(), bent).is_err(),
        "a byte changed refuses"
    );
    let storage_key: [u8; 32] = shell
        .custody
        .lock()
        .unwrap()
        .clone()
        .expect("the custody keeps the storage key")
        .try_into()
        .unwrap();
    assert!(
        rhtn_client::sealed::open(&storage_key, "threads/bob", &sealed).is_none(),
        "the storage key does not open the application's blob: the key is derived, not shared"
    );
    drop(p);

    let open = Arc::new(Shell {
        declared_unsealed: true,
        ..Default::default()
    });
    let q = Participant::start(seeds("alice"), known, platform_of(open)).expect("starts");
    assert_eq!(
        q.seal("threads/bob".into(), plain.clone()),
        plain,
        "unsealed by declaration: through"
    );
    assert_eq!(
        q.unseal("threads/bob".into(), plain.clone())
            .expect("through"),
        plain
    );
    assert!(
        q.unseal("threads/bob".into(), sealed).is_err(),
        "a sealed blob on an unsealed platform is refused, never read as plaintext"
    );
}

/// **A save writes the relations that changed and no other** (APP-016),
/// and **a client that kept the one blob opens it, writes the relations,
/// and reduces the blob to a mark at the next clean start**.
// functional: APP-016
#[test]
fn a_save_writes_only_the_relations_that_changed_and_the_one_blob_migrates_once() {
    let shell = Arc::new(Shell {
        ..Default::default()
    });
    let known: Vec<Vec<u8>> = ["alice", "bob"].iter().map(|n| material(n)).collect();
    let p = Participant::start(seeds("alice"), known.clone(), platform_of(shell.clone()))
        .expect("starts");
    p.save().expect("saved");
    let first: Vec<String> = std::mem::take(&mut *shell.writes.lock().unwrap());
    for r in rhtn_client::ceremony::RELATIONS {
        assert!(
            first.iter().any(|w| w == r),
            "the first save writes {r}: {first:?}"
        );
    }
    p.save().expect("saved again");
    let second: Vec<String> = std::mem::take(&mut *shell.writes.lock().unwrap());
    assert_eq!(
        second,
        vec!["siblings".to_string()],
        "nothing changed: nothing rewritten but the sibling list"
    );
    // a contact held changes the store and nothing else
    p.hold_contact(material("bob")).expect("held");
    p.save().expect("saved");
    let third: Vec<String> = std::mem::take(&mut *shell.writes.lock().unwrap());
    assert!(
        third.contains(&"store".to_string()) && !third.contains(&"archive".to_string()),
        "{third:?}"
    );
    drop(p);

    // the migration: a shell holding the one blob and no relations, which
    // is what a phone from before the split holds
    let old = Arc::new(Shell {
        ..Default::default()
    });
    {
        let r = Participant::start(seeds("alice"), known.clone(), platform_of(old.clone()))
            .expect("starts");
        r.hold_contact(material("bob")).expect("held");
        let whole = r.durable_blob();
        drop(r);
        let mut st = old.store.lock().unwrap();
        st.clear();
        st.insert("client".into(), whole);
    }
    old.writes.lock().unwrap().clear();
    let r = Participant::start(seeds("alice"), known.clone(), platform_of(old.clone()))
        .expect("opens the one blob");
    assert_eq!(r.contacts().len(), 1, "what the blob held is held");
    let wrote: Vec<String> = std::mem::take(&mut *old.writes.lock().unwrap());
    assert!(
        rhtn_client::ceremony::RELATIONS
            .iter()
            .all(|n| wrote.iter().any(|w| w == n)),
        "the relations written at the migration: {wrote:?}"
    );
    assert!(
        old.store
            .lock()
            .unwrap()
            .get("client")
            .map(|b| b.len() > 1)
            .unwrap_or(false),
        "the blob is kept at the migration"
    );
    drop(r);
    let r = Participant::start(seeds("alice"), known.clone(), platform_of(old.clone()))
        .expect("opens the relations");
    assert_eq!(r.contacts().len(), 1);
    let wrote: Vec<String> = std::mem::take(&mut *old.writes.lock().unwrap());
    assert!(
        wrote.contains(&"client".to_string())
            && !wrote
                .iter()
                .any(|w| rhtn_client::ceremony::RELATIONS.contains(&w.as_str())),
        "the blob is reduced to a mark once the relations read back cleanly, and nothing is migrated again: {wrote:?}"
    );
    drop(r);
    // and a third start reads the relations alone: no mark, no migration
    let r = Participant::start(seeds("alice"), known, platform_of(old.clone()))
        .expect("opens the relations");
    assert_eq!(r.contacts().len(), 1);
    let wrote: Vec<String> = std::mem::take(&mut *old.writes.lock().unwrap());
    assert!(
        wrote.iter().all(|w| w == "siblings"),
        "nothing but the sibling list: {wrote:?}"
    );
    drop(r);
}

/// The direct messages a node holds for `to`, opened as `to` opens them:
/// under its own secrets and the node's half. Each body as it was sealed,
/// in queue order — what tells a first message from a later one.
fn sealed_bodies(node: &Arc<LiveNode>, to: &str) -> Vec<rhtn_archive::sealed::SealedBody> {
    let k = sid(to).public.keyhash;
    let secret = rhtn_crypto::seal::EncryptionSecret::from_identity_root(sid(to).identity_root());
    node.node
        .queue_records(&k)
        .into_iter()
        .filter_map(|q| {
            let sealed = rhtn_archive::sealed::Sealed::decode(&q.sealed).ok()?;
            let ss_n = node
                .view
                .lock()
                .unwrap()
                .node_keys
                .open_half(&k, q.n, &sealed.ct_n)?;
            let r = rhtn_archive::submission::Relayed {
                position: q.position,
                sealed: q.sealed.clone(),
                ss_n,
            };
            rhtn_client::payload::open_relayed(&secret, &k, &node.me(), &r)
                .ok()
                .map(|(_, body)| body)
        })
        .filter(|b| b.kind == KIND_APPLICATION)
        .collect()
}

/// **Nobody the node can name** (`light-client-requirements.md` §3):
/// alice is attached to the node that also serves carol, holds carol's
/// card and has never had a message from her. Every fetch and every
/// submission goes on a connection of its own under a key minted for it —
/// the node binds each to nobody — and none rides the session alice holds
/// there, which names her. Her introduction rides inside the seal until
/// carol has written back, and not after.
// acceptance: PAY-31
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sender_attached_to_the_recipients_node_fetches_and_submits_as_nobody() {
    use rhtn_transport::bind::Bound;
    use rhtn_transport::session::Event as Wire;
    let node = serving("bob");
    let addr = node.addr.to_string();
    let start = |name: &'static str, known: Vec<Vec<u8>>| {
        tokio::task::spawn_blocking(move || {
            Participant::start(seeds(name), known, platform_of(Arc::new(Shell::default())))
                .expect("starts")
        })
    };
    let alice = Arc::new(
        start(
            "alice",
            vec![material("alice"), material("bob"), material("carol")],
        )
        .await
        .unwrap(),
    );
    // carol knows nobody but the node: alice reaches her as a stranger
    let carol = Arc::new(start("carol", vec![material("bob")]).await.unwrap());
    {
        let (q, intro) = (
            alice.clone(),
            carol.own_introduction().expect("carol's card"),
        );
        tokio::task::spawn_blocking(move || q.take_introduction(intro))
            .await
            .unwrap()
            .expect("alice holds carol's card");
    }
    for p in [&carol, &alice] {
        let (q, addr) = (p.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![]))
            .await
            .unwrap()
            .expect("attaches");
    }
    let alice_k = sid("alice").public.keyhash;
    assert!(
        node.node.sessions().contains(&alice_k),
        "alice holds a session with carol's node, which names her"
    );
    let minted = || {
        node.node
            .log
            .count(|e| matches!(e, Wire::Bound { how: Bound::Minted }))
    };
    let settle = || tokio::time::sleep(std::time::Duration::from_millis(400));
    let send = |what: &'static [u8]| {
        let q = alice.clone();
        tokio::task::spawn_blocking(move || q.send(id("carol"), KIND_APPLICATION, what.to_vec()))
    };
    let next_payload = |p: Arc<Participant>| {
        tokio::task::spawn_blocking(move || {
            loop {
                match p.next_event(5000) {
                    Some(Event::Payload { from, bytes, .. }) => break Some((from, bytes)),
                    Some(_) => continue,
                    None => break None,
                }
            }
        })
    };

    // the first two, back to back: a fetch, the two messages and the
    // offer of a direct path after the first, each on a minted connection
    // of its own. Anything of carol's — her own offer back is a message
    // too — counts as having written back, so the two go before she can
    let (m0, r0) = (minted(), node.node.relayed());
    send(b"one").await.unwrap().expect("taken");
    send(b"two").await.unwrap().expect("taken");
    settle().await;
    let (m2, r2) = (minted(), node.node.relayed());
    assert!(r2 - r0 >= 3, "the node took both, and the offer");
    assert!(
        m2 - m0 > r2 - r0,
        "a minted connection for each fetch and one for each submission: {} for {}",
        m2 - m0,
        r2 - r0
    );
    assert_eq!(
        next_payload(carol.clone()).await.unwrap().map(|(f, _)| f),
        Some(id("alice")),
        "carol attributes it to alice by the introduction inside"
    );

    // carol writes back, which is what stops alice's introduction riding
    {
        let q = carol.clone();
        tokio::task::spawn_blocking(move || {
            q.send(id("alice"), KIND_APPLICATION, b"back".to_vec())
        })
        .await
        .unwrap()
        .expect("carol answers a party she now holds");
    }
    assert_eq!(
        next_payload(alice.clone()).await.unwrap(),
        Some((id("carol"), b"back".to_vec()))
    );
    settle().await;
    let (m3, r3) = (minted(), node.node.relayed());
    send(b"three").await.unwrap().expect("taken");
    settle().await;
    assert_eq!(
        (minted() - m3, node.node.relayed() - r3),
        (1, 1),
        "the third as nobody too"
    );

    // what carol's queue holds from alice, opened as carol opens it
    let bodies: Vec<(Vec<u8>, bool)> = sealed_bodies(&node, "carol")
        .into_iter()
        .filter(|b| b.sender == alice_k)
        .map(|b| (b.bytes, b.introduction.is_some()))
        .collect();
    assert_eq!(
        bodies,
        vec![
            (b"one".to_vec(), true),
            (b"two".to_vec(), true),
            (b"three".to_vec(), false)
        ],
        "her introduction rides until carol has written back, and not after"
    );
}
