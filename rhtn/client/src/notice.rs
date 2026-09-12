//! What a client tells the person, and when (`light-client-requirements.md`
//! §1.1, §1.4, §1.5; design §7.4.1, §19.6).  The form of a notice is not
//! specified; what is specified is that it is raised, and at which moment,
//! so the client raises each through one hook a test can watch.

use crate::Keyhash;

/// A notice to the person operating this client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// What the record will contain and who will be able to read it, at
    /// the moment of capture (`light-client-requirements.md` §1.5).
    RecordDisclosure { role: Role },
    /// A query about this subject arrived (design §7.4.1: surfaced, not
    /// logged).
    QuerySurfaced { verifier: Keyhash },
    /// A requester exceeded its allowance and gets no grant.
    ProbingRefused { requester: Keyhash },
    /// This party's nominees are absent from or outnumbered in a proposed
    /// witness set (`light-client-requirements.md` §1.1).
    NomineesOutnumbered { mine: usize, theirs: usize },
    /// The selector recognises nobody in the counterparty's candidate pool
    /// (`light-client-requirements.md` §1.4).
    NoCandidateRecognised,
    /// A catalog entry declares a data practice this client does not
    /// recognise (`wire-format.md` §6.1): a declaration exists.
    UnrecognisedDeclaration { resource: Keyhash, value: u64 },
    /// An initial payload message could not be attributed to the sender it
    /// named, so no session was opened and nothing was dispatched
    /// (design §14.2.4.2).
    PayloadUnattributable { from: Keyhash },
}

/// The capacity a party is told in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Participant,
    Witness,
    Verifier,
}

/// Where notices go.  A running client shows them to its operator; a
/// harness records them.
pub trait Notifier {
    fn notify(&self, notice: Notice);
}

/// Nowhere.
pub struct Silent;

impl Notifier for Silent {
    fn notify(&self, _: Notice) {}
}
