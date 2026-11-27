//! **Paired from the phone** (`wire-format.md` §14.3.3): a desktop has no
//! camera to read the phone's screen, so it shows one code and everything
//! after crosses a session that code keys.
//!
//! The code is a `DeviceOffer`: the transport key this desktop presents,
//! a secret drawn for this offer, and where on the local network the phone
//! reaches it.  The phone reads it, and then on the session, in order:
//!
//! 1. the phone's `DeviceStart`: the identity and the run's first window,
//!    which this desktop stands up on;
//! 2. this desktop's `DeviceIntroduction`: the key it presents;
//! 3. the phone's `DeviceCredential`: the whole run, opening with that
//!    first window byte for byte, and the identity's two encryption
//!    secrets, so this desktop reads what is sealed to the person;
//! 4. the dataset (design §23.3): this desktop's contribution, the
//!    phone's, and the hand-over sealed under the key the two derive, each
//!    holder saying yes on their own device — or an empty carriage where
//!    the phone's holder said no;
//! 5. **for each further identity the phone holds** [author, 2026-11-23:
//!    *"one pairing session with a fresh key per identity"*], its
//!    `DeviceNext`, this desktop's introduction of a key minted for that
//!    identity alone, the credential for it, and that identity's dataset as
//!    in 4 — until the phone closes the session.
//!
//! **The bearer is this shell's and the phone's, not the protocol's**
//! (§14.3): one TCP connection on the local network, each carriage as a
//! four-byte big-endian length and the sealed bytes.  It is bound to the
//! one local address the offer names and a peer from outside the private
//! ranges is refused before it is read, which is §14.1's MUST as far as
//! this side can keep it: a carriage that reached it another way is not
//! taken as having crossed the local interface.

use rhtn_client::local::{
    DeviceCredential, DeviceIntroduction, DeviceNext, DeviceOffer, DeviceStart,
};
use rhtn_ffi::client::Participant;
use rhtn_ffi::device::Platform;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use zeroize::Zeroizing;

/// **Where each identity a pairing brings keeps its state**: its kernel's
/// platform, made from its key material.
pub type PlatformFor<'a> = dyn Fn(&[u8]) -> Result<Arc<Platform>, String> + 'a;

/// What [`Offer::pair`] says when it was stopped before a phone came.
pub const STOPPED: &str = "the pairing was stopped before a phone came";

/// How long one step waits for the phone, its holder's yes included.
pub const STEP: Duration = Duration::from_secs(180);

/// The largest carriage taken: a dataset with its sealed captures is the
/// large one, and a photo store reaches hundreds of megabytes
/// (design §23.3).
pub const CARRIAGE_CEILING: usize = 1 << 30;

/// **One offer, shown until the phone reads it.**  Dropping it lets the
/// session's key go (§14.3.3: the offer's lifetime).
pub struct Offer {
    seed: Zeroizing<[u8; 32]>,
    offer: DeviceOffer,
    listener: TcpListener,
}

/// What a finished pairing leaves, one for each identity the phone brought:
/// the running participant, and what this desktop keeps to start it again.
pub struct Paired {
    /// The kernel, standing on the run.
    pub participant: Arc<Participant>,
    /// **The transport seed this identity's key is made from**, its own:
    /// no two identities on this desktop present one key, since a key two
    /// identities present links them (`light-client-requirements.md` §2).
    pub seed: Zeroizing<[u8; 32]>,
    /// The colour the phone shows this identity in, where it said.
    pub colour: Option<String>,
    /// The identity's `KeyMaterial`.
    pub material: Vec<u8>,
    /// The run, in window order as the phone sent it.
    pub run: Vec<Vec<u8>>,
    /// Whether the dataset was taken; `false` where either holder said no,
    /// which leaves the pairing standing without it.
    pub dataset: bool,
}

impl Offer {
    /// An offer for the transport key `seed` makes, listening on `at`.  A
    /// first pairing draws the seed ([`fresh_seed`]); a later hand-over
    /// shows the key this desktop already presents.
    pub fn new(seed: Zeroizing<[u8; 32]>, at: IpAddr) -> std::io::Result<Offer> {
        if !local(at) {
            return Err(std::io::Error::other(format!(
                "{at} is not a local-network address"
            )));
        }
        let listener = TcpListener::bind(SocketAddr::new(at, 0))?;
        let reach = listener.local_addr()?.to_string();
        let device = device_key(&seed);
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&rhtn_transport::tls::random_bytes::<32>());
        Ok(Offer {
            seed,
            offer: DeviceOffer {
                device,
                secret,
                reach,
            },
            listener,
        })
    }

    /// The code to show: the `DeviceOffer`'s bytes.
    pub fn code(&self) -> Vec<u8> {
        self.offer.encode()
    }

    /// The transport key this offer names.
    pub fn device(&self) -> [u8; 32] {
        self.offer.device
    }

    /// The transport seed, for the caller to keep once paired.
    pub fn seed(&self) -> &[u8; 32] {
        &self.seed
    }

    /// **Wait for the phone and run the whole pairing**: the credential and
    /// then the dataset.  A connection whose first carriage does not open
    /// under this offer is dropped and the next one waited for, so a
    /// stranger on the network costs a reconnect and nothing else.  While
    /// it waits, `stop` set ends the wait with nothing paired: the person
    /// chose the phrase instead.
    pub fn pair(
        &self,
        known: Vec<Vec<u8>>,
        platform: &PlatformFor<'_>,
        stop: &AtomicBool,
    ) -> Result<Vec<Paired>, String> {
        let session = self.offer.session();
        self.listener
            .set_nonblocking(true)
            .map_err(|e| e.to_string())?;
        let (mut stream, start) = loop {
            let (mut stream, peer) = match self.listener.accept() {
                Ok(a) => a,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if stop.load(Ordering::Relaxed) {
                        return Err(STOPPED.into());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
                Err(e) => return Err(e.to_string()),
            };
            if !local(peer.ip()) {
                continue;
            }
            stream.set_nonblocking(false).map_err(|e| e.to_string())?;
            stream
                .set_read_timeout(Some(STEP))
                .map_err(|e| e.to_string())?;
            let Ok(sealed) = read_frame(&mut stream) else {
                continue;
            };
            let Some(plain) = session.open(&sealed) else {
                continue;
            };
            break (stream, DeviceStart::decode(&plain)?);
        };

        let ids = |material: &[u8]| {
            let mut ids = known.clone();
            if !ids.iter().any(|k| k == material) {
                ids.push(material.to_vec());
            }
            ids
        };
        let participant = Arc::new(
            Participant::start_delegated(
                start.material.clone(),
                self.seed.to_vec(),
                vec![start.delegation.clone()],
                ids(&start.material),
                platform(&start.material)?,
            )
            .map_err(|e| e.to_string())?,
        );
        let intro = DeviceIntroduction {
            device: self.offer.device,
        };
        write_frame(&mut stream, &session.seal(&intro.encode()))?;
        let credential = self.credential(&session, &mut stream, &participant)?;
        if credential.run.first() != Some(&start.delegation) {
            return Err("the run does not open with the window this desktop stood up on".into());
        }
        let dataset = self.dataset(&session, &mut stream, &participant)?;
        let mut paired = vec![Paired {
            participant,
            seed: self.seed.clone(),
            colour: start.colour,
            material: start.material,
            run: credential.run,
            dataset,
        }];

        // **every further identity the phone holds, on this one session,
        // each under a key of its own** [author, 2026-11-23]; the phone
        // closing the session is the end of them
        while let Ok(sealed) = read_frame(&mut stream) {
            let plain = session
                .open(&sealed)
                .ok_or("the next identity does not open under this offer")?;
            let next = DeviceNext::decode(&plain)?;
            if paired.iter().any(|p| p.material == next.material) {
                return Err("the phone named an identity twice".into());
            }
            let seed = fresh_seed();
            let intro = DeviceIntroduction {
                device: device_key(&seed),
            };
            write_frame(&mut stream, &session.seal(&intro.encode()))?;
            let plain = session
                .open(&read_frame(&mut stream)?)
                .ok_or("the credential does not open under this offer")?;
            let credential = DeviceCredential::decode(&plain)?;
            let first = credential
                .run
                .first()
                .ok_or("the credential carries no run")?;
            let participant = Arc::new(
                Participant::start_delegated(
                    next.material.clone(),
                    seed.to_vec(),
                    vec![first.clone()],
                    ids(&next.material),
                    platform(&next.material)?,
                )
                .map_err(|e| e.to_string())?,
            );
            take(&participant, &credential)?;
            let dataset = self.dataset(&session, &mut stream, &participant)?;
            paired.push(Paired {
                participant,
                seed,
                colour: next.colour,
                material: next.material,
                run: credential.run,
                dataset,
            });
        }
        Ok(paired)
    }

    /// The credential for the key this desktop just introduced, taken: the
    /// run, and the identity's encryption secrets.
    fn credential(
        &self,
        session: &rhtn_client::local::DeviceSession,
        stream: &mut TcpStream,
        participant: &Participant,
    ) -> Result<DeviceCredential, String> {
        let plain = session
            .open(&read_frame(stream)?)
            .ok_or("the credential does not open under this offer")?;
        let credential = DeviceCredential::decode(&plain)?;
        take(participant, &credential)?;
        Ok(credential)
    }

    /// The dataset's three carriages: this desktop's contribution out, the
    /// phone's in, and the hand-over, which the phone sends only on its
    /// holder's yes and this desktop takes only on its own holder's.
    fn dataset(
        &self,
        session: &rhtn_client::local::DeviceSession,
        stream: &mut TcpStream,
        participant: &Participant,
    ) -> Result<bool, String> {
        let mine = participant
            .dataset_contribution()
            .map_err(|e| e.to_string())?;
        write_frame(stream, &session.seal(&mine))?;
        let theirs = session
            .open(&read_frame(stream)?)
            .ok_or("the phone's contribution does not open under this offer")?;
        participant
            .take_dataset_contribution(theirs.to_vec())
            .map_err(|e| e.to_string())?;
        // **an empty carriage where its holder said no**, so the session
        // goes on to the next identity; a phone that closed instead has
        // said no to this one and brings no more
        let Ok(sealed) = read_frame(stream) else {
            return Ok(false);
        };
        let carriage = session
            .open(&sealed)
            .ok_or("the hand-over does not open under this offer")?;
        if carriage.is_empty() {
            return Ok(false);
        }
        Ok(participant.take_dataset_carriage(carriage.to_vec()).is_ok())
    }
}

/// Take a credential into `participant`: the run, and the identity's
/// encryption secrets, so this desktop reads what is sealed to the person
/// (`wire-format.md` §14.3.3).
fn take(participant: &Participant, credential: &DeviceCredential) -> Result<(), String> {
    participant
        .take_run(credential.run.clone())
        .map_err(|e| e.to_string())?;
    participant
        .take_encryption_secrets(credential.kem_seed.to_vec(), credential.auth.to_vec())
        .map_err(|e| e.to_string())
}

/// A transport seed drawn fresh, for a first pairing.
pub fn fresh_seed() -> Zeroizing<[u8; 32]> {
    Zeroizing::new(rhtn_transport::tls::random_bytes::<32>())
}

/// The public transport key `seed` makes.  The keyhash a credential is
/// built with plays no part in its key, so none is named here.
pub fn device_key(seed: &[u8; 32]) -> [u8; 32] {
    rhtn_transport::tls::Credential::from_seed(seed, [0u8; 32]).public()
}

/// **Whether an address is on a local network**: the private, shared and
/// link-local ranges, and loopback.  What the offer binds and what it
/// accepts from.
pub fn local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            v.is_private()
                || v.is_link_local()
                || v.is_loopback()
                // the shared range carriers put behind their NAT
                || (o[0] == 100 && (o[1] & 0xc0) == 64)
        }
        IpAddr::V6(v) => {
            let s = v.segments()[0];
            v.is_loopback() || (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
        }
    }
}

/// **The local address the phone would reach this desktop at**: the one
/// the system routes outward from, found by aiming a datagram socket and
/// sending nothing.  Nothing where that address is not a local one.
pub fn lan_address() -> Option<IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // a documentation address: no datagram is sent by a connect
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    local(ip).then_some(ip)
}

/// One carriage off the stream: its length, then its bytes.
pub fn read_frame(s: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut n = [0u8; 4];
    s.read_exact(&mut n).map_err(|e| e.to_string())?;
    let n = u32::from_be_bytes(n) as usize;
    if n > CARRIAGE_CEILING {
        return Err(format!("a carriage of {n} bytes is over the ceiling"));
    }
    let mut b = vec![0u8; n];
    s.read_exact(&mut b).map_err(|e| e.to_string())?;
    Ok(b)
}

/// One carriage onto the stream.
pub fn write_frame(s: &mut impl Write, b: &[u8]) -> Result<(), String> {
    let n = u32::try_from(b.len()).map_err(|_| "a carriage over four gigabytes")?;
    s.write_all(&n.to_be_bytes()).map_err(|e| e.to_string())?;
    s.write_all(b).map_err(|e| e.to_string())?;
    s.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::Desk;
    use rhtn_ffi::client::DeviceLink;
    use rhtn_ffi::device::Custody;
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// A storage key kept in memory: custody for a test.
    #[derive(Default)]
    struct Held(Mutex<Option<Vec<u8>>>);
    impl Custody for Held {
        fn key(&self) -> Option<Vec<u8>> {
            self.0.lock().unwrap().clone()
        }
        fn keep(&self, key: Vec<u8>) -> bool {
            *self.0.lock().unwrap() = Some(key);
            true
        }
        fn unsealed(&self) -> bool {
            false
        }
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rhtn-pair-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn platform(dir: &std::path::Path) -> Arc<Platform> {
        Desk::new(dir, Arc::new(|_| true), Arc::new(|_| {}))
            .unwrap()
            .platform(Arc::new(Held::default()))
    }

    /// The test vectors' seeds for `name`, as the boundary test makes them.
    fn seeds(name: &str) -> Vec<u8> {
        let h = |s: String| rhtn_codec::cose::sha256(s.as_bytes()).to_vec();
        let mut b = h(format!("rhtn-test-vectors:{name}:ed25519-seed"));
        b.extend(h(format!("rhtn-test-vectors:{name}:ml-dsa-65-seed")));
        b
    }

    // functional: APP-018
    #[test]
    fn a_restored_desktop_takes_its_identitys_backup_and_no_other() {
        // the archive reaches a desktop restored from the phrase from a
        // backup alone (`light-client-requirements.md` §2): the phone
        // writes one, the desktop on the same seeds opens it, and a
        // desktop of another identity is refused it
        let (pd, dd, od) = (scratch("bk-phone"), scratch("bk-desk"), scratch("bk-other"));
        let phone = Participant::start(seeds("alice"), vec![], platform(&pd)).unwrap();
        let backup = phone.export_backup().unwrap();
        let desk = Participant::start(seeds("alice"), vec![], platform(&dd)).unwrap();
        let other = Participant::start(seeds("bob"), vec![], platform(&od)).unwrap();
        assert!(
            other.restore_backup(backup.clone()).is_err(),
            "another identity's"
        );
        let took = desk.restore_backup(backup).unwrap();
        assert_eq!(took.discarded, 0);
        assert_eq!(desk.subnets(), phone.subnets());
        assert_eq!(desk.contacts(), phone.contacts());
        for d in [pd, dd, od] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// The phone's side, as its shell runs it: read the code, connect, and
    /// answer each carriage through the kernel — for each identity it
    /// holds, the first by `DeviceStart` and the rest by `DeviceNext`, each
    /// with whether its holder hands the dataset over.
    fn phone(
        identities: Vec<(Arc<Participant>, bool)>,
        code: Vec<u8>,
    ) -> Result<Vec<DeviceCredential>, String> {
        let link = DeviceLink::read(code).map_err(|e| e.to_string())?;
        let mut s = TcpStream::connect(link.reach()).map_err(|e| e.to_string())?;
        let mut crossed = Vec::new();
        for (i, (phone, hand)) in identities.into_iter().enumerate() {
            let first = if i == 0 {
                phone.device_start(link.clone(), now() - 60, 3, Some("Teal".into()))
            } else {
                phone.device_next(link.clone(), None)
            };
            write_frame(&mut s, &first.map_err(|e| e.to_string())?)?;
            let intro = read_frame(&mut s)?;
            let credential = phone
                .device_credential(link.clone(), intro, now() - 60, 3)
                .map_err(|e| e.to_string())?;
            // what crossed, as the desktop opens it, kept for the test
            crossed.push(DeviceCredential::decode(
                &link.open(credential.clone()).map_err(|e| e.to_string())?,
            )?);
            write_frame(&mut s, &credential)?;
            // shown before the other's is taken
            let mine = phone.dataset_contribution().map_err(|e| e.to_string())?;
            let theirs = link.open(read_frame(&mut s)?).map_err(|e| e.to_string())?;
            phone
                .take_dataset_contribution(theirs)
                .map_err(|e| e.to_string())?;
            write_frame(&mut s, &link.seal(mine))?;
            // **an empty carriage is the holder's no**, and the session
            // goes on to the next identity
            let carriage = if hand {
                phone.dataset_carriage().map_err(|e| e.to_string())?
            } else {
                Vec::new()
            };
            write_frame(&mut s, &link.seal(carriage))?;
        }
        Ok(crossed)
    }

    // functional: NET-035
    #[test]
    fn a_desktop_is_paired_from_the_code_it_shows() {
        let (pd, dd) = (scratch("phone"), scratch("desk"));
        let ph = Arc::new(Participant::start(seeds("alice"), vec![], platform(&pd)).unwrap());
        let offer = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
        let code = offer.code();
        let p = ph.clone();
        let side = std::thread::spawn(move || phone(vec![(p, true)], code));
        let mut paired = offer
            .pair(vec![], &|_| Ok(platform(&dd)), &AtomicBool::new(false))
            .expect("paired");
        let crossed = side
            .join()
            .unwrap()
            .expect("the phone's side runs")
            .remove(0);
        assert_eq!(paired.len(), 1, "one identity, one pairing");
        let paired = paired.remove(0);
        assert_eq!(
            paired.colour.as_deref(),
            Some("Teal"),
            "the phone's colour for it"
        );

        assert_eq!(paired.material, ph.material());
        assert_eq!(paired.run.len(), 3);
        assert!(paired.dataset, "both holders said yes");
        let desk = &paired.participant;
        assert!(!desk.holds_seed());
        assert_eq!(desk.presented_key(), offer.device().to_vec());
        // **the credential carries the run and alice's two encryption
        // secrets** (`wire-format.md` §14.3.3), equal to what her phone
        // derives from her root: what lets the desktop open what is sealed
        // to the person and seal as her
        assert_eq!(crossed.run, paired.run);
        let s = seeds("alice");
        let root = rhtn_crypto::SigningIdentity::from_seeds(
            s[..32].try_into().unwrap(),
            s[32..].try_into().unwrap(),
        );
        let derived = rhtn_crypto::seal::EncryptionSecret::from_identity_root(root.identity_root());
        assert_eq!(*crossed.kem_seed, *derived.kem_seed(), "her X-Wing seed");
        assert_eq!(
            *crossed.auth,
            *derived.auth_secret(),
            "her authentication key"
        );
        let _ = std::fs::remove_dir_all(&pd);
        let _ = std::fs::remove_dir_all(&dd);
    }

    /// The kernel's sealing service, as the window keeps the threads under it.
    struct KernelSealer(Arc<Participant>);

    impl crate::threads::Sealer for KernelSealer {
        fn seal(&self, label: &str, bytes: &[u8]) -> Vec<u8> {
            self.0.seal(label.to_string(), bytes.to_vec())
        }
        fn unseal(&self, label: &str, bytes: &[u8]) -> Option<Vec<u8>> {
            self.0.unseal(label.to_string(), bytes.to_vec()).ok()
        }
    }

    /// **A run is renewed by pairing again, and what the desktop holds
    /// carries over** [author, 2026-11-27]: the second pairing stands on
    /// the state and the custody the first left, so the conversation kept
    /// under the first run reads under the second, which opens under a new
    /// key; and the run held says when the next start pairs again.
    // functional: APP-021
    #[test]
    fn a_desktop_paired_again_on_what_it_holds_keeps_its_conversations() {
        use crate::participant::{PairedIdentity, hex, run_ended, run_ends};
        use crate::threads::{Delivery, Record, Threads};
        let (pd, dd) = (scratch("again-phone"), scratch("again-desk"));
        let ph = Arc::new(Participant::start(seeds("alice"), vec![], platform(&pd)).unwrap());
        let custody = Arc::new(Held::default());
        let pair = || {
            let offer = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
            let code = offer.code();
            let p = ph.clone();
            let side = std::thread::spawn(move || phone(vec![(p, false)], code));
            let on_desk = |_: &[u8]| {
                Ok(Desk::new(&dd, Arc::new(|_| true), Arc::new(|_| {}))
                    .unwrap()
                    .platform(custody.clone()))
            };
            let mut paired = offer
                .pair(vec![], &on_desk, &AtomicBool::new(false))
                .expect("paired");
            side.join().unwrap().expect("the phone's side runs");
            paired.remove(0)
        };
        let (me, peer) = (hex(&ph.me()), "ab".repeat(32));
        let threads =
            |p: &Arc<Participant>| Threads::new(&dd, &me, Box::new(KernelSealer(p.clone())));

        let first = pair();
        threads(&first.participant)
            .append(
                &peer,
                &Record::Said {
                    mine: false,
                    who: "them".into(),
                    text: "kept".into(),
                    delivery: Delivery::Sent,
                    reason: None,
                },
            )
            .unwrap();
        let held = [PairedIdentity {
            material: first.material.clone(),
            run: first.run.clone(),
        }];
        let ends = run_ends(&first.run).expect("the run reads");
        assert!(!run_ended(&held, ends - 1) && run_ended(&held, ends));
        let before = first.participant.presented_key();
        drop(first);

        let second = pair();
        assert_ne!(second.participant.presented_key(), before, "a new key");
        assert!(run_ends(&second.run).is_some_and(|end| end > now()));
        let page = threads(&second.participant).newest(&peer, 0);
        assert!(!page.broken, "every record opens under the second run");
        assert_eq!(
            page.messages
                .iter()
                .map(|m| m.text.as_str())
                .collect::<Vec<_>>(),
            ["kept"]
        );
        let _ = std::fs::remove_dir_all(&pd);
        let _ = std::fs::remove_dir_all(&dd);
    }

    /// **Every identity the phone holds crosses on one session, each under a
    /// key of its own** [author, 2026-11-23], and a no to one identity's
    /// dataset leaves the next one's standing.
    // functional: APP-020
    #[test]
    fn every_identity_the_phone_holds_is_paired_on_one_session_under_a_key_of_its_own() {
        let (pa, pb, dd) = (scratch("two-a"), scratch("two-b"), scratch("two-desk"));
        let alice = Arc::new(Participant::start(seeds("alice"), vec![], platform(&pa)).unwrap());
        let bob = Arc::new(Participant::start(seeds("bob"), vec![], platform(&pb)).unwrap());
        let offer = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
        let code = offer.code();
        let (a, b) = (alice.clone(), bob.clone());
        let side = std::thread::spawn(move || phone(vec![(a, false), (b, true)], code));
        let made = Mutex::new(0);
        let paired = offer
            .pair(
                vec![],
                &|_| {
                    let mut n = made.lock().unwrap();
                    *n += 1;
                    Ok(platform(&dd.join(n.to_string())))
                },
                &AtomicBool::new(false),
            )
            .expect("paired");
        let crossed = side.join().unwrap().expect("the phone's side runs");
        assert_eq!(paired.len(), 2);
        assert_eq!(paired[0].material, alice.material());
        assert_eq!(paired[1].material, bob.material());
        assert!(
            !paired[0].dataset && paired[1].dataset,
            "a no to alice's, a yes to bob's"
        );
        assert_eq!(paired[1].colour, None);
        // **a key each, and the first the offer's**
        let (ka, kb) = (
            paired[0].participant.presented_key(),
            paired[1].participant.presented_key(),
        );
        assert_eq!(ka, offer.device().to_vec());
        assert_ne!(ka, kb, "no key presented by two identities");
        assert_eq!(kb, device_key(&paired[1].seed).to_vec());
        assert_eq!(crossed[1].run, paired[1].run);
        // and the phone refuses a second identity's introduction naming the
        // first one's key: two identities on one key are linked by it
        let link = DeviceLink::read(offer.code()).unwrap();
        let _ = bob.device_next(link.clone(), None).unwrap();
        let reused = link.seal(
            DeviceIntroduction {
                device: offer.device(),
            }
            .encode(),
        );
        assert!(bob.device_credential(link, reused, now() - 60, 3).is_err());
        for d in [pa, pb, dd] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    // functional: NET-035
    #[test]
    fn a_carriage_under_another_offer_opens_on_neither_side() {
        let a = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
        let b = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
        let (sa, sb) = (a.offer.session(), b.offer.session());
        let sealed = sa.seal(b"a start");
        assert!(sb.open(&sealed).is_none(), "another offer's secret");
        assert_eq!(
            sa.open(&sealed).as_deref().map(|v| v.as_slice()),
            Some(&b"a start"[..])
        );
        let link = DeviceLink::read(b.code()).unwrap();
        assert!(
            link.open(sealed).is_err(),
            "the phone's link refuses it too"
        );
    }

    #[test]
    fn a_pairing_stopped_while_it_waits_pairs_nothing() {
        let d = scratch("stopped");
        let offer = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
        let stop = AtomicBool::new(true);
        let e = offer
            .pair(vec![], &|_| Ok(platform(&d)), &stop)
            .err()
            .unwrap();
        assert_eq!(e, STOPPED);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn an_offer_binds_only_a_local_address_and_reads_back() {
        assert!(Offer::new(fresh_seed(), "8.8.8.8".parse().unwrap()).is_err());
        for ip in [
            "10.1.2.3",
            "172.16.0.1",
            "192.168.1.9",
            "100.64.0.1",
            "169.254.1.1",
            "fd00::1",
            "fe80::1",
        ] {
            assert!(local(ip.parse().unwrap()), "{ip}");
        }
        for ip in ["8.8.8.8", "100.128.0.1", "2001:db8::1"] {
            assert!(!local(ip.parse().unwrap()), "{ip}");
        }
        let o = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
        let read = DeviceOffer::decode(&o.code()).unwrap();
        assert_eq!(read.device, o.device());
        assert_eq!(read.device, device_key(o.seed()));
        assert!(read.reach.starts_with("127.0.0.1:"));
    }
}
