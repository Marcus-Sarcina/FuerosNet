//! The boundary as a shell uses it (`light-client-requirements.md` §1.3):
//! the platform's objects handed over once, the client running on a thread
//! of its own behind them, and every answer a value.

use rhtn_ffi::client::{Intent, Participant};
use rhtn_ffi::device::*;
use rhtn_ffi::net::{Event, Wake};
use rhtn_ffi::types::KIND_APPLICATION;
use rhtn_ffi::types::*;
use std::sync::{Arc, Mutex};

/// A shell's hardware: what it reports and what it was asked.
#[derive(Default)]
struct Shell {
    has: Vec<Channel>,
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

impl Proximity for Shell {
    fn supported(&self) -> Vec<Channel> {
        self.has.clone()
    }
    fn run(&self, channel: Channel, _peer: Vec<u8>) -> ChannelOutcome {
        match channel {
            Channel::Nfc => ChannelOutcome::Pass,
            _ => ChannelOutcome::Fail,
        }
    }
    fn resolution_m(&self, channel: Channel) -> Option<u64> {
        match channel {
            Channel::Uwb => Some(2),
            _ => None,
        }
    }
}

impl Camera for Shell {
    fn capture(&self, ask: Ask) -> Vec<u8> {
        self.asked.lock().unwrap().push(ask);
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
        // real randomness: two shells drawing the same constant would
        // derive the same ratchet keys and read as one party to each other
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
        proximity: shell.clone(),
        camera: shell.clone(),
        clock: shell.clone(),
        random: shell.clone(),
        operator: shell.clone(),
        notices: shell.clone(),
        storage: shell,
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
        has: vec![Channel::Nfc, Channel::Uwb],
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

    // the ceremony opens: the intent crosses as fields, since no document
    // fixes an encoding for what two present devices tell each other
    let i: Intent = p.begin(id("bob"), vec![id("carol")], true).expect("begins");
    assert_eq!(i.contribution.len(), 16);
    assert_eq!(i.nominees, vec![id("carol")]);
    assert!(i.initiator);
    assert_eq!(
        i.started_at, 1_800_000_000,
        "the platform's clock, in seconds"
    );

    // the hardware is asked through the boundary, strongest first, and
    // nothing is promoted: the channel that failed is reported failed
    let bob = Participant::start(
        seeds("bob"),
        ["alice", "bob", "carol"]
            .iter()
            .map(|n| material(n))
            .collect(),
        platform_of(Arc::new(Shell {
            has: vec![Channel::Nfc, Channel::Uwb],
            ..Default::default()
        })),
    )
    .unwrap();
    let theirs = bob
        .begin(id("alice"), vec![id("carol")], false)
        .expect("begins");
    p.take_intent(id("bob"), theirs)
        .expect("takes the counterparty's intent");
    let achieved = p.proximity().expect("runs the channels");
    assert_eq!(
        achieved.iter().map(|a| a.channel).collect::<Vec<_>>(),
        vec![Channel::Uwb, Channel::Nfc],
        "strongest first"
    );
    assert_eq!(achieved[0].outcome, ChannelOutcome::Fail);
    assert_eq!(
        achieved[0].resolution_m,
        Some(2),
        "what the hardware measured"
    );
    assert_eq!(achieved[1].outcome, ChannelOutcome::Pass);

    // a refusal is a value carrying its reason, not a failure the shell
    // has to guess at
    let e = p.begin(vec![1, 2, 3], vec![], true).unwrap_err();
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
        has: vec![Channel::Nfc],
        short: true,
        ..Default::default()
    });
    let known: Vec<Vec<u8>> = ["alice"].iter().map(|n| material(n)).collect();
    // the client seeds its payload material as it starts, so a platform
    // giving short measure is caught there: a seed completed with zeroes
    // is not random, and padding one silently is worse than refusing
    let Err(e) = Participant::start(seeds("alice"), known, platform_of(shell)) else {
        panic!("short measure was accepted")
    };
    // and it reaches the shell as a value, not as a process that vanished
    assert!(e.reason().contains("could not be built"), "{e}");
}

// ------------------------------------- the kernel's own side of the wire

use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::{Log, NodeConfig};
use rhtn_transport::tls::Pins;

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
                        has: vec![Channel::Nfc],
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

    // and the node holds what the attach published and stocked, which no
    // shell carried for it
    {
        let view = node.view.lock().unwrap();
        assert!(
            view.prekeys.bundle(&sid("alice").public.keyhash).is_some(),
            "the bundle went over the wire from the kernel"
        );
        assert!(
            view.prekeys.pool_size(&sid("alice").public.keyhash) > 0,
            "and the one-time pool with it"
        );
    }

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
    // on both sides: carol attaches so its bundle is published, alice
    // sweeps it, and what alice sends comes out of carol's event queue
    // already decrypted
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
        let (q, addr) = (carol_side.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![]))
            .await
            .unwrap()
            .expect("carol attaches");
    }
    // alice sweeps again, since the first sweep ran before carol published
    {
        let (q, addr) = (p.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![id("carol")]))
            .await
            .unwrap()
            .expect("alice attaches again");
    }
    // and carol sweeps alice, since carol attached before alice's second
    // publication and a recipient attributes an initial message only under
    // the binding it holds
    {
        let (q, addr) = (carol_side.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![id("alice")]))
            .await
            .unwrap()
            .expect("carol sweeps");
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
    // blocking forever
    let q = p.clone();
    assert!(
        tokio::task::spawn_blocking(move || q.next_event(200))
            .await
            .unwrap()
            .is_none()
    );
}

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
                        has: vec![Channel::Nfc],
                        ..Default::default()
                    })),
                )
                .expect("starts")
            }
        })
        .await
        .unwrap(),
    );
    // carol publishes so alice can address it, and alice sweeps carol
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
        node.view
            .lock()
            .unwrap()
            .prekeys
            .bundle(&sid("carol").public.keyhash)
            .is_some(),
        "control: the recipient's material is known"
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
/// delegated over it; it attaches under the delegation, its bundle is
/// signed on the phone and published under its own device, and what her
/// key signs is refused on it.
// acceptance: DMN-27
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_device_holding_no_seed_attaches_under_its_delegation_and_publishes_what_the_phone_signed()
 {
    let node = serving("bob");
    let addr = node.addr.to_string();
    let known: Vec<Vec<u8>> = ["alice", "bob", "carol"]
        .iter()
        .map(|n| material(n))
        .collect();
    let shell = || {
        platform_of(Arc::new(Shell {
            has: vec![Channel::Nfc],
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
    assert!(
        phone.bundle_to_sign().is_none(),
        "the phone signs its own bundle"
    );

    // the attach binds under the delegation: the node pinned alice, and
    // the key presented is the one the delegation names
    let a = {
        let (q, addr) = (desktop.clone(), addr.clone());
        tokio::task::spawn_blocking(move || q.attach(id("bob"), vec![addr], vec![id("carol")]))
            .await
            .unwrap()
            .expect("attaches under the delegation")
    };
    assert_eq!(a.serving, id("bob"));
    assert!(desktop.attached());
    {
        let view = node.view.lock().unwrap();
        assert!(
            view.prekeys
                .bundle_for(&sid("alice").public.keyhash, &presented)
                .is_none(),
            "no bundle yet: nothing signed it"
        );
        assert!(
            view.prekeys
                .pool_size_for(&sid("alice").public.keyhash, &presented)
                > 0,
            "the pool is the device's own and is stocked"
        );
    }

    // the unsigned bundle crosses to the phone and the signed one back,
    // and the node then holds it under the desktop's device
    let payload = desktop
        .bundle_to_sign()
        .expect("material awaiting a signature");
    let signed = phone
        .sign_device_bundle(payload)
        .expect("the phone signs its own device's");
    {
        let (q, signed) = (desktop.clone(), signed.clone());
        tokio::task::spawn_blocking(move || q.take_signed_bundle(signed))
            .await
            .unwrap()
            .expect("published");
    }
    assert!(desktop.bundle_to_sign().is_none());
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    {
        let view = node.view.lock().unwrap();
        let b = view
            .prekeys
            .bundle_for(&sid("alice").public.keyhash, &presented)
            .expect("the node holds the desktop's bundle");
        assert_eq!(b, &signed);
        let parsed = rhtn_archive::prekey::PrekeyBundle::parse(b).unwrap();
        assert_eq!(parsed.device, presented);
        assert_eq!(parsed.subject, sid("alice").public.keyhash);
    }
}

/// A process restart with an established payload session and messages
/// queued meanwhile: the kernel starts from what it wrote through the
/// storage seam, the queued message opens on the restored ratchet, and the
/// reply continues the same session.  State that does not open refuses
/// the start; a backup made here restores into an empty kernel.
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
        carol_shell.read("client".into()).is_none(),
        "nothing written before anything happened"
    );
    let attach = |p: Arc<Participant>, pop: Vec<Vec<u8>>| {
        let addr = addr.clone();
        tokio::task::spawn_blocking(move || p.attach(id("bob"), vec![addr], pop))
    };
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
    assert!(
        carol_shell.read("client".into()).is_some(),
        "an attach writes the state"
    );

    // a session both ways
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
            bytes: b"second".to_vec()
        })
    );
    let written = carol_shell.read("client".into()).unwrap();

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
    // meanwhile alice sends: the node queues it for carol's device
    let before = node.node.queued(&sid("carol").public.keyhash);
    send(alice.clone(), id("carol"), b"third")
        .await
        .unwrap()
        .expect("queued at the node");
    assert_eq!(
        node.node.queued(&sid("carol").public.keyhash),
        before + 1,
        "queued for the device that is away"
    );

    // a new process from the same storage: the session is there, the
    // queued message opens on it, and the reply continues it
    let carol2 = Arc::new(start("carol", carol_shell.clone()).await.unwrap());
    assert_eq!(
        carol_shell.read("client".into()).unwrap(),
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
            bytes: b"third".to_vec()
        }),
        "decrypted on the ratchet the first process wrote"
    );
    send(carol2.clone(), id("alice"), b"fourth")
        .await
        .unwrap()
        .expect("sent");
    assert_eq!(
        next(alice.clone()).await.unwrap(),
        Some(Event::Payload {
            from: id("carol"),
            bytes: b"fourth".to_vec()
        }),
        "the same session, continued"
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
        tokio::task::spawn_blocking(move || q.export_backup(b"pw".to_vec()))
            .await
            .unwrap()
            .expect("exports")
    };
    let empty_shell = Arc::new(Shell::default());
    let alice2 = Arc::new(start("alice", empty_shell.clone()).await.unwrap());
    let r = {
        let (q, blob) = (alice2.clone(), blob.clone());
        tokio::task::spawn_blocking(move || q.restore_backup(blob, b"pw".to_vec()))
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
    assert!(empty_shell.read("client".into()).is_some());
    let e = {
        let q = alice2.clone();
        tokio::task::spawn_blocking(move || q.restore_backup(blob, b"wrong".to_vec()))
            .await
            .unwrap()
            .expect_err("a wrong passphrase opens nothing")
    };
    assert!(e.reason().contains("Secret"), "{e}");
}
