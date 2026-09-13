//! What crosses the boundary.
//!
//! A generated binding carries records, enums and byte strings; it does not
//! carry Rust's lifetimes, traits or errors. The types here are the
//! translation, and the rule for each is that it loses nothing a shell
//! needs to render and adds nothing the library did not say.
//!
//! Every one of them is plain data with no borrow and no generic, because
//! that is what a generator can read whichever generator is chosen.

use rhtn_archive::Keyhash;
use rhtn_client::device::{ChannelKind, ChannelResult, Prompt};
use rhtn_client::notice::{Notice, Role};
use rhtn_client::query::Verdict;

/// A keyhash as it crosses: its 32 bytes.
///
/// **No display form is fixed for one** by any document, so how a person
/// is shown a keyhash is the shell's, and the two shells must agree: the
/// same identity rendered two ways reads as two identities to the person
/// comparing them aloud.
pub type Id = Vec<u8>;

pub fn id_of(k: &Keyhash) -> Id {
    k.to_vec()
}

/// A keyhash from a shell, which is 32 bytes or nothing.
pub fn keyhash(id: &[u8]) -> Option<Keyhash> {
    id.try_into().ok()
}

/// A proximity channel, as the shell knows it (`light-client-requirements.md` §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Uwb,
    Nfc,
    Optical,
    Latency,
}

impl Channel {
    pub fn of(k: ChannelKind) -> Channel {
        match k {
            ChannelKind::Uwb => Channel::Uwb,
            ChannelKind::Nfc => Channel::Nfc,
            ChannelKind::Optical => Channel::Optical,
            ChannelKind::Latency => Channel::Latency,
        }
    }

    pub fn kind(self) -> ChannelKind {
        match self {
            Channel::Uwb => ChannelKind::Uwb,
            Channel::Nfc => ChannelKind::Nfc,
            Channel::Optical => ChannelKind::Optical,
            Channel::Latency => ChannelKind::Latency,
        }
    }
}

/// What running a channel came to.  **A channel the hardware lacks is
/// unavailable and never a failure**, and nothing is promoted: the
/// distinction is the shell's to report and the client's to weigh
/// (`light-client-requirements.md` §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelOutcome {
    Pass,
    Fail,
    Unavailable,
}

impl ChannelOutcome {
    pub fn result(self) -> ChannelResult {
        match self {
            ChannelOutcome::Pass => ChannelResult::Pass,
            ChannelOutcome::Fail => ChannelResult::Fail,
            ChannelOutcome::Unavailable => ChannelResult::Unavailable,
        }
    }
}

/// What the capture asks the counterparty to do (design §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    TurnLeft,
    TurnRight,
    LookUp,
    LookDown,
    Smile,
    Neutral,
    Blink,
    Closer,
}

impl Ask {
    pub fn of(p: Prompt) -> Ask {
        match p {
            Prompt::TurnLeft => Ask::TurnLeft,
            Prompt::TurnRight => Ask::TurnRight,
            Prompt::LookUp => Ask::LookUp,
            Prompt::LookDown => Ask::LookDown,
            Prompt::Smile => Ask::Smile,
            Prompt::Neutral => Ask::Neutral,
            Prompt::Blink => Ask::Blink,
            Prompt::Closer => Ask::Closer,
        }
    }
}

/// A verifier's answer about a subject (`wire-format.md` §5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Match,
    NoMatch,
    Inconclusive,
    Unavailable,
}

impl Answer {
    pub fn of(v: Verdict) -> Answer {
        match v {
            Verdict::Match => Answer::Match,
            Verdict::NoMatch => Answer::NoMatch,
            Verdict::Inconclusive => Answer::Inconclusive,
            Verdict::Unavailable => Answer::Unavailable,
        }
    }
}

/// What the person is told, as a closed set.
///
/// **A shell that meets a notice it does not know must still show
/// something**, so the set is versioned rather than open: a new one is a
/// new variant here and a shell that has not been rebuilt renders
/// [`Told::Unknown`] with its text rather than nothing (design §19.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Told {
    /// What the record will contain and who can read it, at capture.
    RecordDisclosure { role: String },
    /// A query about this subject arrived.
    QuerySurfaced { verifier: Id },
    /// A requester exceeded its allowance and gets no grant.
    ProbingRefused { requester: Id },
    /// This party's nominees are absent from or outnumbered in a proposal.
    NomineesOutnumbered { mine: u32, theirs: u32 },
    /// The selector recognises nobody in the counterparty's pool.
    NoCandidateRecognised,
    /// A catalog entry declares a practice this client does not know.
    UnrecognisedDeclaration { resource: Id, value: u64 },
    /// Payload arrived that could not be attributed to the sender named.
    PayloadUnattributable { from: Id },
    /// A notice this build of the shell does not know, described.
    Unknown { described: String },
}

impl Told {
    pub fn of(n: &Notice) -> Told {
        match n {
            Notice::RecordDisclosure { role } => Told::RecordDisclosure {
                role: match role {
                    Role::Participant => "participant".into(),
                    Role::Witness => "witness".into(),
                    Role::Verifier => "verifier".into(),
                },
            },
            Notice::QuerySurfaced { verifier } => Told::QuerySurfaced { verifier: id_of(verifier) },
            Notice::ProbingRefused { requester } => Told::ProbingRefused { requester: id_of(requester) },
            Notice::NomineesOutnumbered { mine, theirs } => Told::NomineesOutnumbered { mine: *mine as u32, theirs: *theirs as u32 },
            Notice::NoCandidateRecognised => Told::NoCandidateRecognised,
            Notice::UnrecognisedDeclaration { resource, value } => Told::UnrecognisedDeclaration { resource: id_of(resource), value: *value },
            Notice::PayloadUnattributable { from } => Told::PayloadUnattributable { from: id_of(from) },
        }
    }
}

/// Why something could not be done, as a value.
///
/// **Every one of these is something the person is owed an explanation
/// for**, so none of them crosses as a bare failure: the reason is carried
/// and the shell decides how to say it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub reason: String,
}

impl Refused {
    pub fn new(reason: impl Into<String>) -> Refused {
        Refused { reason: reason.into() }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

impl std::error::Error for Refused {}
