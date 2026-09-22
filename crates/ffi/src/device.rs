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
pub trait Camera: Send + Sync {
    fn capture(&self, ask: Ask) -> Vec<u8>;
}

/// The platform's clock.
///
/// **It is the clock.** A skew a shell corrected silently would move a
/// witness's tolerance check without saying so
/// (`light-client-requirements.md` §1.2), so what the platform reports is
/// what the ceremony reads.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
    fn wait_ms(&self, ms: u64);
}

/// The platform's randomness.
pub trait Random: Send + Sync {
    fn fill(&self, n: u32) -> Vec<u8>;
}

/// The person operating the client.
///
/// **The one party a client may ask a question**, and only where the
/// documents say to ask rather than tell: witnessing, querying and
/// answering ask none (design Appendix A.3).
pub trait Operator: Send + Sync {
    fn ask(&self, question: String) -> bool;
}

/// Where notices go.  They are raised as they occur and never polled for.
pub trait Notices: Send + Sync {
    fn told(&self, notice: Told);
}

/// Everything a shell supplies, in one object it hands over once.
#[derive(Clone)]
pub struct Platform {
    pub proximity: Arc<dyn Proximity>,
    pub camera: Arc<dyn Camera>,
    pub clock: Arc<dyn Clock>,
    pub random: Arc<dyn Random>,
    pub operator: Arc<dyn Operator>,
    pub notices: Arc<dyn Notices>,
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
        let result = self.0.run(c, peer.to_vec()).result();
        // the channel asked for is the channel reported: a shell that
        // answered about another would silently move what was measured
        dev::ChannelOutcome {
            kind,
            result,
            resolution_m: self.0.resolution_m(c),
        }
    }
}

struct CameraIn(Arc<dyn Camera>);

impl dev::Camera for CameraIn {
    fn capture(&self, prompt: dev::Prompt) -> dev::RawFrame {
        // metadata is empty because none crosses: there is nothing to
        // strip that was not already left on the platform's side
        dev::RawFrame {
            pixels: self.0.capture(Ask::of(prompt)),
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
        self.0.ask(question.to_string())
    }
}

struct NoticesIn(Arc<dyn Notices>);

impl Notifier for NoticesIn {
    fn notify(&self, notice: Notice) {
        self.0.told(Told::of(&notice));
    }
}
