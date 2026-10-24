//! The platform, where the platform is a terminal.
//!
//! `rhtn-ffi` takes six objects from a shell (design §14.1.0's boundary as
//! the author restated it): the camera, the clock, randomness, the person,
//! the notifier and the storage.  A phone has all six.  A terminal has three
//! of them outright, one by asking, and the rest only because somebody says
//! what they are.
//!
//! **So this one is told rather than measured, and never pretends
//! otherwise.** A machine with no camera pointed at anybody captures
//! nothing real, so what the camera here returns is declared rather than
//! seen.  A declaration is what it is — evidence about a scenario, not about
//! hardware — and that is the whole difference between an instrument and a
//! client.

use rhtn_ffi::device::{Camera, Clock, Notices, Operator, Random, Storage};
use rhtn_ffi::types::{Ask, Told};
use std::sync::Mutex;

/// What the operator of the instrument has declared, and what the client
/// has said back.
#[derive(Default)]
pub struct Terminal {
    /// What is answered when the client asks the person a question.  A
    /// standing answer rather than a prompt, because standard input is the
    /// command channel: a question read from there would race the script
    /// driving it.  **Declining is the default**, since the question is
    /// always whether to release something.
    answer: Mutex<bool>,
    /// Raised as they occur, drained after each command, so a transcript
    /// stays in the order a reader expects.
    said: Mutex<Vec<String>>,
    /// The client's state for this process alone: an instrument keeps
    /// nothing between runs, and a script that wants continuity starts
    /// the same process again.
    kept: Mutex<std::collections::BTreeMap<String, Vec<u8>>>,
}

// **Unsealed by declaration** (`light-client-requirements.md` §9): this
// instrument is explicitly ephemeral, and on a terminal the store's
// protection is the operating environment's, as an operator's is
// (`infra-client-requirements.md` §7). Stated here, never fallen back to.
impl rhtn_ffi::device::Custody for Terminal {
    fn key(&self) -> Option<Vec<u8>> {
        None
    }
    fn keep(&self, _key: Vec<u8>) -> bool {
        false
    }
    fn unsealed(&self) -> bool {
        true
    }
}

impl Storage for Terminal {
    fn read(&self, name: String) -> Option<Vec<u8>> {
        self.kept.lock().unwrap().get(&name).cloned()
    }
    fn write(&self, name: String, bytes: Vec<u8>) -> bool {
        self.kept.lock().unwrap().insert(name, bytes);
        true
    }
}

impl Terminal {
    /// Set the standing answer to a question put to the person.
    pub fn answers(&self, yes: bool) {
        *self.answer.lock().unwrap() = yes;
    }

    /// The standing answer, as [`answers`](Self::answers) last set it.
    pub fn answering(&self) -> bool {
        *self.answer.lock().unwrap()
    }

    /// Everything said since the last drain.
    pub fn drain(&self) -> Vec<String> {
        std::mem::take(&mut *self.said.lock().unwrap())
    }

    fn say(&self, line: String) {
        self.said.lock().unwrap().push(line);
    }
}

impl Camera for Terminal {
    /// A frame that is the same frame every time for the same prompt.
    ///
    /// **Nothing is claimed for it.** design §22.2 leaves the biometric
    /// profile open and the reference engine compares hashes and
    /// recognises nobody, so a real camera in front of this engine would
    /// prove no more than this does. What the capture exercises is the
    /// sealing, the key release and the record's shape.
    fn capture(&self, ask: Ask) -> Vec<u8> {
        let tag = format!("rhtn-instrument:frame:{ask:?}");
        tag.into_bytes().repeat(16)
    }

    /// A probe's frame: the same frame, with nothing asked.
    fn probe(&self) -> Vec<u8> {
        b"rhtn-instrument:probe".repeat(16).to_vec()
    }
}

impl Clock for Terminal {
    /// **The platform's clock is the clock** (`light-client-requirements.md`
    /// §1.2): a skew corrected here would move a witness's tolerance check
    /// without saying so.
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn wait_ms(&self, ms: u64) {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

impl Random for Terminal {
    fn fill(&self, n: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(n as usize);
        while out.len() < n as usize {
            out.extend_from_slice(&rhtn_transport::tls::random_bytes::<32>());
        }
        out.truncate(n as usize);
        out
    }
}

impl Operator for Terminal {
    fn ask(&self, question: String) -> bool {
        let yes = self.answering();
        self.say(format!(
            "asked {} -> {}",
            question,
            if yes { "yes" } else { "no" }
        ));
        yes
    }
}

impl Notices for Terminal {
    fn told(&self, notice: Told) {
        self.say(format!("told {}", describe(&notice)));
    }
}

/// One line per notice, in the vocabulary `rhtn-ffi` closes over.
fn describe(t: &Told) -> String {
    match t {
        Told::RecordDisclosure { role } => format!("record-disclosure role={role}"),
        Told::QuerySurfaced { verifier } => format!("query-surfaced verifier={}", hex(verifier)),
        Told::ProbingRefused { requester } => {
            format!("probing-refused requester={}", hex(requester))
        }
        Told::NomineesOutnumbered { mine, theirs } => {
            format!("nominees-outnumbered mine={mine} theirs={theirs}")
        }
        Told::NoCandidateRecognised => "no-candidate-recognised".into(),
        Told::UnrecognisedDeclaration { resource, value } => format!(
            "unrecognised-declaration resource={} value={value}",
            hex(resource)
        ),
        Told::PayloadUnattributable { from } => {
            format!("payload-unattributable from={}", hex(from))
        }
        Told::Unknown { described } => format!("unknown {described}"),
    }
}

/// `b` as lower-case hex, which is how this driver writes every byte
/// string.
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// The bytes `s` spells in hex, or nothing where it does not spell any.
pub fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}
