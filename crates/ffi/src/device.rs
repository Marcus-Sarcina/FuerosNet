//! The platform's hardware, passed inward.
//!
//! `rhtn-client` reaches the world through interfaces it declares: the
//! proximity channels, the camera, the clock, randomness, the person and
//! the notifier. On a phone each is the platform's, so each arrives across
//! the boundary as an object the shell supplies.
//!
//! **The two sides hold their objects differently, and that is the work
//! this module does.** A shell hands over something usable from any
//! thread; the client reaches its device through `Rc` and runs on a thread
//! of its own, never moving. Each shell object is therefore wrapped once,
//! inside that thread, in an adapter the client's interfaces accept.
//!
//! **Nothing is decided here.** A rule enforced at this boundary and not
//! below it would be absent from every other client, so the adapters
//! translate and never adjudicate.

use crate::types::{Ask, Channel, ChannelOutcome, Told};
use rhtn_archive::Keyhash;
use rhtn_client::device as dev;
use rhtn_client::notice::{Notice, Notifier};
use std::sync::Arc;

/// The proximity hardware a shell offers.
///
/// **A channel the hardware lacks is not listed**, and one that failed is
/// reported as failed: nothing is promoted, and the strongest that passed
/// is the client's to decide (`light-client-requirements.md` §1.3).
#[uniffi::export(with_foreign)]
pub trait Proximity: Send + Sync {
    fn supported(&self) -> Vec<Channel>;
    fn run(&self, channel: Channel, peer: Vec<u8>) -> ChannelOutcome;
    /// Metres, where the channel measured one.
    fn resolution_m(&self, channel: Channel) -> Option<u64>;
}

/// The camera, which returns the pixels of one frame.
///
/// **What crosses is pixels.** Whatever the platform's pipeline attached
/// to the frame, location among it, does not come inward
/// (`light-client-requirements.md` §1.3): the shell hands over the image
/// and nothing beside it.
#[uniffi::export(with_foreign)]
pub trait Camera: Send + Sync {
    fn capture(&self, ask: Ask) -> Vec<u8>;
}

/// The platform's clock.
///
/// **It is the clock.** A skew a shell corrected silently would move a
/// witness's tolerance check without saying so
/// (`light-client-requirements.md` §1.2), so what the platform reports is
/// what the ceremony reads.
#[uniffi::export(with_foreign)]
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
    fn wait_ms(&self, ms: u64);
}

/// The platform's randomness.
#[uniffi::export(with_foreign)]
pub trait Random: Send + Sync {
    fn fill(&self, n: u32) -> Vec<u8>;
}

/// The person operating the client.
///
/// **The one party a client may ask a question**, and only where the
/// documents say to ask rather than tell: witnessing, querying and
/// answering ask none (design Appendix A.3).
#[uniffi::export(with_foreign)]
pub trait Operator: Send + Sync {
    fn ask(&self, question: String) -> bool;
}

/// Where notices go.  They are raised as they occur and never polled for.
/// Where the client's own state lives between runs (design §23.3:
/// archives, sealed captures and caches go where the storage is).  The
/// shell owns the place, app-private storage on a phone; the kernel owns
/// what is written there, one opaque blob under a name, and reads it back
/// at the next start.  **The seed is never written through this**: it is
/// the platform's key storage's, and the shell supplies it at every start.
#[uniffi::export(with_foreign)]
pub trait Storage: Send + Sync {
    /// The bytes last written under `name`, or nothing.
    fn read(&self, name: String) -> Option<Vec<u8>>;
    /// Write `bytes` under `name`, replacing what was there; false where the
    /// write did not land, which the kernel reports rather than assumes.
    fn write(&self, name: String, bytes: Vec<u8>) -> bool;
}

#[uniffi::export(with_foreign)]
pub trait Notices: Send + Sync {
    fn told(&self, notice: Told);
}

/// Custody of the kernel's storage key (`light-client-requirements.md`
/// §9). The kernel mints the key and seals what it persists under it; where
/// the key lives between runs is the platform's, by whatever affordance the
/// platform has — a key store wrapping it, a passphrase deriving it, the
/// operating environment where the platform declares itself unsealed. The
/// kernel calls this named source and never chooses.
#[uniffi::export(with_foreign)]
pub trait Custody: Send + Sync {
    /// The kept storage key, where one is kept; nothing where none is yet.
    fn key(&self) -> Option<Vec<u8>>;
    /// Keep a key the kernel minted. False refuses the start: failed
    /// custody is never a quiet fall to plaintext.
    fn keep(&self, key: Vec<u8>) -> bool;
    /// Declared unsealed: at rest, protection is the operating
    /// environment's (`infra-client-requirements.md` §7). A configuration
    /// the platform states, never inferred from a missing key.
    fn unsealed(&self) -> bool;
}

/// Where the field-test build's diagnostic events go
/// (`Robot/field-test-diagnostics.md`): one JSON line per event, handed over
/// as it happens on the kernel's threads, the contract [`Notices`] has.
/// The shell appends it to its own file; nothing here keeps it.
///
/// **A releasable build calls this never.** Its hooks are compiled out, so
/// the object is handed over for the one `Platform` shape both flavours
/// share and hears nothing; [`Silent`] is the Rust-side no-op.
#[uniffi::export(with_foreign)]
pub trait Diagnostics: Send + Sync {
    fn event(&self, line: String);
}

/// A diagnostics sink that drops every line: what a test or a tool that
/// wants no diagnostics hands over.
pub struct Silent;

impl Diagnostics for Silent {
    fn event(&self, _: String) {}
}

/// Everything a shell supplies, in one object it hands over once.
#[derive(Clone, uniffi::Object)]
pub struct Platform {
    pub proximity: Arc<dyn Proximity>,
    pub camera: Arc<dyn Camera>,
    pub clock: Arc<dyn Clock>,
    pub random: Arc<dyn Random>,
    pub operator: Arc<dyn Operator>,
    pub notices: Arc<dyn Notices>,
    pub storage: Arc<dyn Storage>,
    pub custody: Arc<dyn Custody>,
    pub diagnostics: Arc<dyn Diagnostics>,
}

impl Platform {
    /// The client's own device, built from this platform.
    ///
    /// Called on the thread the client runs on, which is where the `Rc`
    /// the client holds must be made.  The biometric engine is not the
    /// shell's: design §22.2 leaves the real one open, and the reference
    /// compares hashes and recognises nobody.
    pub fn device(&self, direct: std::rc::Rc<dyn dev::DirectPath>) -> dev::Device {
        dev::Device {
            proximity: std::rc::Rc::new(ProximityIn(self.proximity.clone())),
            camera: std::rc::Rc::new(CameraIn(self.camera.clone())),
            clock: std::rc::Rc::new(ClockIn(self.clock.clone())),
            random: std::rc::Rc::new(RandomIn(self.random.clone())),
            operator: std::rc::Rc::new(OperatorIn(self.operator.clone())),
            notifier: std::rc::Rc::new(NoticesIn(self.notices.clone())),
            engine: std::rc::Rc::new(dev::HashEngine::new(16)),
            direct,
        }
    }
}

struct ProximityIn(Arc<dyn Proximity>);

impl dev::Proximity for ProximityIn {
    fn supported(&self) -> Vec<dev::ChannelKind> {
        self.0.supported().into_iter().map(|c| c.kind()).collect()
    }

    fn run(&self, kind: dev::ChannelKind, peer: &Keyhash) -> dev::ChannelOutcome {
        let c = Channel::of(kind);
        let started = std::time::Instant::now();
        let result = self.0.run(c, peer.to_vec()).result();
        // the channel asked for is the channel reported: a shell that
        // answered about another would silently move what was measured
        let out = dev::ChannelOutcome {
            kind,
            result,
            resolution_m: self.0.resolution_m(c),
        };
        tracing::info!(
            target: "platform",
            channel = ?kind,
            outcome = ?result,
            resolution_m = out.resolution_m,
            ms = started.elapsed().as_millis() as u64,
            "platform.proximity"
        );
        out
    }
}

struct CameraIn(Arc<dyn Camera>);

impl dev::Camera for CameraIn {
    fn capture(&self, prompt: dev::Prompt) -> dev::RawFrame {
        let started = std::time::Instant::now();
        let pixels = self.0.capture(Ask::of(prompt));
        // the frame's size and the prompt it answered, never a pixel
        tracing::info!(
            target: "platform",
            prompt = ?prompt,
            bytes = pixels.len(),
            ms = started.elapsed().as_millis() as u64,
            "platform.camera"
        );
        // metadata is empty because none crosses: there is nothing to
        // strip that was not already left on the platform's side
        dev::RawFrame {
            pixels,
            metadata: Default::default(),
        }
    }
}

struct ClockIn(Arc<dyn Clock>);

impl dev::Clock for ClockIn {
    fn now_ms(&self) -> u64 {
        self.0.now_ms()
    }
    fn wait_ms(&self, ms: u64) {
        self.0.wait_ms(ms);
    }
}

struct RandomIn(Arc<dyn Random>);

impl dev::Random for RandomIn {
    fn fill(&self, out: &mut [u8]) {
        // asked for exactly what is wanted, and short measure is refused
        // rather than padded: a seed completed with zeroes is not random
        let got = self.0.fill(out.len() as u32);
        assert_eq!(
            got.len(),
            out.len(),
            "the platform's randomness returned {} bytes of {}",
            got.len(),
            out.len()
        );
        out.copy_from_slice(&got);
    }
}

struct OperatorIn(Arc<dyn Operator>);

impl dev::Operator for OperatorIn {
    fn ask(&self, question: &str) -> bool {
        let started = std::time::Instant::now();
        let answer = self.0.ask(question.to_string());
        // the kernel's questions name a party by its first eight hex
        // characters and nothing else, so the text is the question's id
        tracing::info!(
            target: "platform",
            question,
            answer,
            ms = started.elapsed().as_millis() as u64,
            "platform.operator"
        );
        answer
    }
}

struct NoticesIn(Arc<dyn Notices>);

impl Notifier for NoticesIn {
    fn notify(&self, notice: Notice) {
        // the variant alone: the identities some notices carry stay out
        tracing::info!(
            target: "platform",
            notice = rhtn_client::diag::notice(&notice),
            "platform.notice"
        );
        self.0.told(Told::of(&notice));
    }
}
