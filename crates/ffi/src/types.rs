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

/// A keyhash as it crosses the boundary.
pub fn id_of(k: &Keyhash) -> Id {
    k.to_vec()
}

/// A keyhash from a shell, which is 32 bytes or nothing.
pub fn keyhash(id: &[u8]) -> Option<Keyhash> {
    id.try_into().ok()
}

/// A proximity channel, as the shell knows it (`light-client-requirements.md` §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Channel {
    /// UWB secure ranging, the strongest channel considered (design §7.6.3).
    Uwb,
    /// An NFC tap.
    Nfc,
    /// The screen-to-camera pass, which the anchor exchange itself makes.
    Optical,
    /// A route-latency plausibility check, weak evidence.
    Latency,
}

impl Channel {
    /// The channel the client's own kind names.
    pub fn of(k: ChannelKind) -> Channel {
        match k {
            ChannelKind::Uwb => Channel::Uwb,
            ChannelKind::Nfc => Channel::Nfc,
            ChannelKind::Optical => Channel::Optical,
            ChannelKind::Latency => Channel::Latency,
        }
    }

    /// The client's kind this channel names.
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
/// The payload session's construction, as the client states it to its own
/// user (`light-client-requirements.md` §3). A value, not a sentence: the
/// wording is each shell's, in its user's own language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Construction {
    /// The classical construction.
    DoubleRatchet,
    /// The construction with the post-quantum ratchet beside it.
    TripleRatchet,
}

impl From<rhtn_client::ratchet::Construction> for Construction {
    fn from(c: rhtn_client::ratchet::Construction) -> Self {
        match c {
            rhtn_client::ratchet::Construction::DoubleRatchet => Construction::DoubleRatchet,
            rhtn_client::ratchet::Construction::TripleRatchet => Construction::TripleRatchet,
        }
    }
}

/// What running one channel came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum ChannelOutcome {
    /// The channel ran and the parties were close enough.
    Pass,
    /// The channel ran and they were not.
    Fail,
    /// The hardware does not have it. **Not a failure**, and never promoted
    /// to one.
    Unavailable,
}

impl ChannelOutcome {
    /// The client's own result this outcome names.
    pub fn result(self) -> ChannelResult {
        match self {
            ChannelOutcome::Pass => ChannelResult::Pass,
            ChannelOutcome::Fail => ChannelResult::Fail,
            ChannelOutcome::Unavailable => ChannelResult::Unavailable,
        }
    }
}

/// What the capture asks the counterparty to do (design §7.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Ask {
    /// Turn your head left.
    TurnLeft,
    /// Turn your head right.
    TurnRight,
    /// Look up.
    LookUp,
    /// Look down.
    LookDown,
    /// Smile.
    Smile,
    /// Rest your face.
    Neutral,
    /// Blink.
    Blink,
    /// Come closer to the camera.
    Closer,
}

impl Ask {
    /// The ask the client's own prompt names.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Answer {
    /// The verifier recognises the subject.
    Match,
    /// It does not.
    NoMatch,
    /// Comparison was attempted and settled nothing.
    Inconclusive,
    /// It holds nothing to compare against. **Silence counts for neither
    /// side** (design §7.4.3).
    Unavailable,
}

impl Answer {
    /// The answer the client's own verdict names.
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
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum Told {
    /// What the record will contain and who can read it, at capture.
    RecordDisclosure {
        /// Which role the reader holds in the ceremony.
        role: String,
    },
    /// A query about this subject arrived.
    QuerySurfaced {
        /// The verifier that was asked.
        verifier: Id,
    },
    /// A requester exceeded its allowance and gets no grant.
    ProbingRefused {
        /// Who asked too often.
        requester: Id,
    },
    /// This party's nominees are absent from or outnumbered in a proposal.
    NomineesOutnumbered {
        /// How many witnesses this party nominated.
        mine: u32,
        /// How many the counterparty did.
        theirs: u32,
    },
    /// The selector recognises nobody in the counterparty's pool.
    NoCandidateRecognised,
    /// A catalog entry declares a practice this client does not know.
    UnrecognisedDeclaration {
        /// The resource whose entry declares it.
        resource: Id,
        /// The value this client does not know.
        value: u64,
    },
    /// Payload arrived that could not be attributed to the sender named.
    PayloadUnattributable {
        /// Who it claimed to be from.
        from: Id,
    },
    /// A notice this build of the shell does not know, described.
    Unknown {
        /// The notice in words, for a shell to show as it is.
        described: String,
    },
}

impl Told {
    /// The notice as a shell reads it; one this build does not know becomes
    /// [`Told::Unknown`] with its text.
    pub fn of(n: &Notice) -> Told {
        match n {
            Notice::RecordDisclosure { role } => Told::RecordDisclosure {
                role: match role {
                    Role::Participant => "participant".into(),
                },
            },
            Notice::QuerySurfaced { verifier } => Told::QuerySurfaced {
                verifier: id_of(verifier),
            },
            Notice::ProbingRefused { requester } => Told::ProbingRefused {
                requester: id_of(requester),
            },
            Notice::NomineesOutnumbered { mine, theirs } => Told::NomineesOutnumbered {
                mine: *mine as u32,
                theirs: *theirs as u32,
            },
            Notice::NoCandidateRecognised => Told::NoCandidateRecognised,
            Notice::UnrecognisedDeclaration { resource, value } => Told::UnrecognisedDeclaration {
                resource: id_of(resource),
                value: *value,
            },
            Notice::PayloadUnattributable { from } => {
                Told::PayloadUnattributable { from: id_of(from) }
            }
        }
    }
}

/// Why something could not be done, as a value.
///
/// **Every one of these is something the person is owed an explanation
/// for**, so none of them crosses as a bare failure: the reason is carried
/// and the shell decides how to say it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum Refused {
    /// The one shape a refusal takes: its reason, in words a screen can
    /// show.  An enum because the binding generator throws enums, and a
    /// variant named apart from the enum because the generated Kotlin
    /// nests one in the other.
    Reason {
        /// The reason, in words a screen can show.
        reason: String,
    },
}

impl Refused {
    /// A refusal carrying `reason`.
    pub fn new(reason: impl Into<String>) -> Refused {
        Refused::Reason {
            reason: reason.into(),
        }
    }

    /// The reason, in words a screen can show.
    pub fn reason(&self) -> &str {
        match self {
            Refused::Reason { reason } => reason,
        }
    }
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason())
    }
}

impl std::error::Error for Refused {}

// --------------------------------------------------- the payload's kinds

/// What a message on the end-to-end channel is (design §14.2.4.6).  A
/// shell sends and reads the first of these; the rest are the client's own
/// traffic and the adaptors answer them without the shell seeing them.
pub const KIND_APPLICATION: u64 = rhtn_client::payload::KIND_APPLICATION;
/// A subject releasing a capture key, which the adaptors answer without
/// the shell seeing it.
pub const KIND_KEY_GRANT: u64 = rhtn_client::payload::KIND_KEY_GRANT;
/// A verifier's response arriving after finalization, likewise.
pub const KIND_LATE_RESPONSE: u64 = rhtn_client::payload::KIND_LATE_RESPONSE;

/// The payload kind a shell's own traffic carries, for a binding that
/// cannot read a constant.
#[uniffi::export]
pub fn kind_application() -> u64 {
    KIND_APPLICATION
}
