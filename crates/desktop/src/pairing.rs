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
//! 2. this desktop's `DeviceIntroduction`: its key and the bundle over the
//!    payload material it minted on standing up;
//! 3. the phone's `DeviceCredential`: the whole run, opening with that
//!    first window byte for byte, and the bundle signed;
//! 4. the dataset (design §23.3): this desktop's contribution, the
//!    phone's, and the hand-over sealed under the key the two derive, each
//!    holder saying yes on their own device.
//!
//! **The bearer is this shell's and the phone's, not the protocol's**
//! (§14.3): one TCP connection on the local network, each carriage as a
//! four-byte big-endian length and the sealed bytes.  It is bound to the
//! one local address the offer names and a peer from outside the private
//! ranges is refused before it is read, which is §14.1's MUST as far as
//! this side can keep it: a carriage that reached it another way is not
//! taken as having crossed the local interface.

use rhtn_client::local::{DeviceCredential, DeviceIntroduction, DeviceOffer, DeviceStart};
use rhtn_ffi::client::Participant;
use rhtn_ffi::device::Platform;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use zeroize::Zeroizing;

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

/// What a finished pairing leaves: the running participant, and what this
/// desktop keeps to start it again.
pub struct Paired {
    /// The kernel, standing on the run.
    pub participant: Arc<Participant>,
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
        platform: Arc<Platform>,
        stop: &AtomicBool,
    ) -> Result<Paired, String> {
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

        let mut ids = known;
        if !ids.contains(&start.material) {
            ids.push(start.material.clone());
        }
        let participant = Arc::new(
            Participant::start_delegated(
                start.material.clone(),
                self.seed.to_vec(),
                vec![start.delegation.clone()],
                ids,
                platform,
            )
            .map_err(|e| e.to_string())?,
        );
        let bundle = participant
            .bundle_to_sign()
            .ok_or("no bundle to sign: this desktop's material is signed over already")?;
        let intro = DeviceIntroduction {
            device: self.offer.device,
            bundle,
        };
        write_frame(&mut stream, &session.seal(&intro.encode()))?;

        let plain = session
            .open(&read_frame(&mut stream)?)
            .ok_or("the credential does not open under this offer")?;
        let credential = DeviceCredential::decode(&plain)?;
        if credential.run.first() != Some(&start.delegation) {
            return Err("the run does not open with the window this desktop stood up on".into());
        }
        participant
            .take_run(credential.run.clone())
            .map_err(|e| e.to_string())?;
        participant
            .take_signed_bundle(credential.bundle)
            .map_err(|e| e.to_string())?;

        let dataset = self.dataset(&session, &mut stream, &participant)?;
        Ok(Paired {
            participant,
            material: start.material,
            run: credential.run,
            dataset,
        })
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
        // the phone closes rather than sends where its holder said no
        let Ok(sealed) = read_frame(stream) else {
            return Ok(false);
        };
        let carriage = session
            .open(&sealed)
            .ok_or("the hand-over does not open under this offer")?;
        Ok(participant.take_dataset_carriage(carriage.to_vec()).is_ok())
    }
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
    /// answer each carriage through the kernel.
    fn phone(phone: Arc<Participant>, code: Vec<u8>, hand: bool) -> Result<(), String> {
        let link = DeviceLink::read(code).map_err(|e| e.to_string())?;
        let mut s = TcpStream::connect(link.reach()).map_err(|e| e.to_string())?;
        let start = phone
            .device_start(link.clone(), now() - 60, 3)
            .map_err(|e| e.to_string())?;
        write_frame(&mut s, &start)?;
        let intro = read_frame(&mut s)?;
        let credential = phone
            .device_credential(link.clone(), intro)
            .map_err(|e| e.to_string())?;
        write_frame(&mut s, &credential)?;
        // shown before the other's is taken: showing ends the pairing before
        let mine = phone.dataset_contribution().map_err(|e| e.to_string())?;
        let theirs = link.open(read_frame(&mut s)?).map_err(|e| e.to_string())?;
        phone
            .take_dataset_contribution(theirs)
            .map_err(|e| e.to_string())?;
        write_frame(&mut s, &link.seal(mine))?;
        if hand {
            let carriage = phone.dataset_carriage().map_err(|e| e.to_string())?;
            write_frame(&mut s, &link.seal(carriage))?;
        }
        Ok(())
    }

    // functional: NET-035
    #[test]
    fn a_desktop_is_paired_from_the_code_it_shows() {
        let (pd, dd) = (scratch("phone"), scratch("desk"));
        let ph = Arc::new(Participant::start(seeds("alice"), vec![], platform(&pd)).unwrap());
        let offer = Offer::new(fresh_seed(), "127.0.0.1".parse().unwrap()).unwrap();
        let code = offer.code();
        let p = ph.clone();
        let side = std::thread::spawn(move || phone(p, code, true));
        let paired = offer
            .pair(vec![], platform(&dd), &AtomicBool::new(false))
            .expect("paired");
        side.join().unwrap().expect("the phone's side runs");

        assert_eq!(paired.material, ph.material());
        assert_eq!(paired.run.len(), 3);
        assert!(paired.dataset, "both holders said yes");
        let desk = &paired.participant;
        assert!(!desk.holds_seed());
        assert_eq!(desk.presented_key(), offer.device().to_vec());
        assert!(desk.bundle_to_sign().is_none(), "the bundle is signed over");
        let _ = std::fs::remove_dir_all(&pd);
        let _ = std::fs::remove_dir_all(&dd);
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
        let e = offer.pair(vec![], platform(&d), &stop).err().unwrap();
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
