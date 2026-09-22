//! The four things a client hands the node serving it (`wire-format.md`
//! §7.10): its bundle, one-time keys for its pool, payload to carry, and
//! where to ring it.  Each is a request stream and each is answered with
//! the nonce it carried and a code, and nothing further
//! (`infra-client-requirements.md` §6.1).
//!
//! Every one of the four is about the client that sent it.  The subject
//! of a publication, the owner of a deposited pool, the sender of a
//! relayed message and the holder of an endpoint are all the
//! authenticated peer, so a client cannot reach past its own
//! relationship by asking.

use crate::Keyhash;
use crate::view::NodeView;
use crate::wake::Registered;
use rhtn_archive::prekey::PrekeyBundle;
use rhtn_archive::submission::*;
use rhtn_transport::queue::Refusal;
use rhtn_transport::session::Node;
use std::sync::Arc;

/// Take a publication from the client that sent it.  A bundle naming
/// another party is refused: publishing it would be choosing the material
/// that party's peers open sessions against
/// (`infra-client-requirements.md` §6.1).
pub fn publication(
    view: &mut NodeView,
    ids: &[rhtn_crypto::identity::Identity],
    peer: &Keyhash,
    body: &[u8],
) -> Option<Vec<u8>> {
    let p = PrekeyPublication::decode(body).ok()?;
    let subject = PrekeyBundle::parse(&p.bundle).ok().map(|b| b.subject);
    let code = if subject != Some(*peer) {
        SUBMISSION_REFUSED
    } else if view.prekeys.publish(ids, &p.bundle).is_ok() {
        SUBMISSION_ACCEPTED
    } else {
        SUBMISSION_REFUSED
    };
    Some(SubmissionReply::code(p.nonce, code).encode())
}

/// Take one-time keys into the sender's own pool, bounded.  Over the
/// bound the deposit is refused with the code rather than silently
/// trimmed, so a client knows what its pool holds.
pub fn deposit(view: &mut NodeView, peer: &Keyhash, body: &[u8]) -> Option<Vec<u8>> {
    let d = OneTimeDeposit::decode(body).ok()?;
    let cfg = view.prekeys.cfg.clone();
    let code = if d.keys.len() > cfg.per_deposit
        || view.prekeys.pool_size(peer) + d.keys.len() > cfg.pool
    {
        SUBMISSION_OVER_BOUND
    } else if view.prekeys.stock(*peer, d.keys) {
        SUBMISSION_ACCEPTED
    } else {
        // **accepted means the node took them.**  A deposit it could not
        // store is refused, not acknowledged: a client told its pool was
        // replenished stops replenishing it
        SUBMISSION_REFUSED
    };
    Some(SubmissionReply::code(d.nonce, code).encode())
}

/// Take payload to carry.  Answered on taking, not on delivering: taking
/// it is the promise the queue already makes (design §14.1.6), and a
/// sender made to wait for delivery would be waiting on a party who may
/// be away for days.  The sender is named in front of the ciphertext for
/// the recipient, the way everything else this node queues is.
pub fn relay(node: Option<&Arc<Node>>, peer: &Keyhash, body: &[u8]) -> Option<Vec<u8>> {
    let r = RelaySubmission::decode(body).ok()?;
    let code = match node {
        None => SUBMISSION_REFUSED,
        Some(n) => match n.enqueue(r.recipient, relayed(*peer, &r.ciphertext)) {
            Ok(()) => SUBMISSION_ACCEPTED,
            // at a cap is a bound; a recipient this node holds no record
            // of, and a credential it has verified superseded, are both
            // refusals about somebody else
            Err(Refusal::AtCap) => SUBMISSION_OVER_BOUND,
            Err(Refusal::NoRecord | Refusal::Superseded) => SUBMISSION_REFUSED,
        },
    };
    Some(SubmissionReply::code(r.nonce, code).encode())
}

/// Register, refresh or withdraw where to ring the sender.  One endpoint
/// per relationship: registering again replaces what is held, and a
/// registration with no endpoint withdraws it (design §14.1.5).
pub fn wake(view: &mut NodeView, peer: &Keyhash, body: &[u8]) -> Option<Vec<u8>> {
    let w = WakeRegistration::decode(body).ok()?;
    let code = match view.wake.register(*peer, w.endpoint, w.key, w.lapses_at) {
        Registered::Held | Registered::Withdrawn => SUBMISSION_ACCEPTED,
        Registered::Refused => SUBMISSION_OVER_BOUND,
    };
    Some(SubmissionReply::code(w.nonce, code).encode())
}
