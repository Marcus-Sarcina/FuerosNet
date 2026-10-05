//! What a client tells the person, and when (`light-client-requirements.md`
//! §1.1, §1.4, §1.5; design §7.4.1, §19.6).  A witness and a verifier are
//! told nothing: neither role is a person acting, and design §19.6 owes
//! them no warning.  The form of a notice is not
//! specified; what is specified is that it is raised, and at which moment,
//! so the client raises each through one hook a test can watch.

use crate::Keyhash;

/// A notice to the person operating this client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// What the record will contain and who will be able to read it, at
    /// the moment of capture (`light-client-requirements.md` §1.5).
    RecordDisclosure {
        /// The capacity the person is told in.
        role: Role,
    },
    /// A query about this subject arrived (design §7.4.1: surfaced, not
    /// logged).
    QuerySurfaced {
        /// The verifier that asked.
        verifier: Keyhash,
    },
    /// A requester exceeded its allowance and gets no grant.
    ProbingRefused {
        /// Who asked past their allowance.
        requester: Keyhash,
    },
    /// This party's nominees are absent from or outnumbered in a proposed
    /// witness set (`light-client-requirements.md` §1.1).
    NomineesOutnumbered {
        /// How many of this party's nominees the set holds.
        mine: usize,
        /// How many of the counterparty's.
        theirs: usize,
    },
    /// The selector recognises nobody in the counterparty's candidate pool
    /// (`light-client-requirements.md` §1.4).
    NoCandidateRecognised,
    /// A catalog entry declares a data practice this client does not
    /// recognise (`wire-format.md` §6.1): a declaration exists.
    UnrecognisedDeclaration {
        /// The resource whose entry declares it.
        resource: Keyhash,
        /// The declared value this client does not recognise.
        value: u64,
    },
    /// An initial payload message could not be attributed to the sender it
    /// named, so no session was opened and nothing was dispatched
    /// (design §14.2.4.2).
    PayloadUnattributable {
        /// Whom the message named as its sender.
        from: Keyhash,
    },
}

/// The capacity a party is told in.  Only a participant is told (design
/// §19.6): the variant set is the obligation's, and a witness or verifier
/// role would be a notice the design withdrew.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A party to the ceremony, which is the one capacity told.
    Participant,
}

/// Where notices go.  A running client shows them to its operator; a
/// harness records them.
pub trait Notifier {
    /// Raise `notice`, at the moment the documents say to raise it.
    fn notify(&self, notice: Notice);
}

/// Nowhere.
pub struct Silent;

impl Notifier for Silent {
    fn notify(&self, _: Notice) {}
}
