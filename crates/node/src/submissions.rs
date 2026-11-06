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
    device: &[u8; 32],
    body: &[u8],
) -> Option<Vec<u8>> {
    let p = PrekeyPublication::decode(body).ok()?;
    let parsed = PrekeyBundle::parse(&p.bundle).ok();
    // the bundle is the sending device's: one naming another device is
    // refused, since the session it arrived on is what names the device
    // (`wire-format.md` §7.10, §8.2)
    let code = if parsed.as_ref().map(|b| (b.subject, b.device)) != Some((*peer, *device)) {
        SUBMISSION_REFUSED
    } else if view.prekeys.publish(ids, &p.bundle).is_ok() {
        SUBMISSION_ACCEPTED
    } else {
        SUBMISSION_REFUSED
    };
    Some(SubmissionReply::code(p.nonce, code).encode())
}

/// Take a profile from the client that sent it: one naming another
/// party is refused, and so is one not later than what is held
/// (`wire-format.md` §7.10, §7.11; `infra-client-requirements.md` §6.2).
/// A subnet's profile is taken from whoever signed it: that the signer is
/// the root is a convention the node does not check (design §12.8).
pub fn profile(
    view: &mut NodeView,
    ids: &[rhtn_crypto::identity::Identity],
    peer: &Keyhash,
    body: &[u8],
) -> Option<Vec<u8>> {
    let p = ProfilePublication::decode(body).ok()?;
    let names_peer = rhtn_archive::profile::Profile::parse(&p.profile)
        .is_ok_and(|parsed| parsed.subject == *peer);
    let code = if names_peer && view.profiles.publish(ids, &p.profile).is_ok() {
        SUBMISSION_ACCEPTED
    } else {
        SUBMISSION_REFUSED
    };
    Some(SubmissionReply::code(p.nonce, code).encode())
}

/// **Take one row of the horizon an attached client asserts**
/// (`wire-format.md` §7.12).
///
/// **The one thing a client hands its node that names another party.** The
/// four of §7.10 are each about the sender — its bundle, its pool, its
/// message, its endpoint — so the authenticated requester is the subject
/// and nothing has to be bounded. This is not: a row is about a member,
/// and what keeps it honest is where the sender sits.
///
/// **So authority follows the topology** [author, 2026-11-04]: a row whose
/// patron is this sender or sits beneath it, and nothing else, which is
/// [`rhtn_archive::topology::Table::may_assert`]. The sender is the
/// keyhash the connection bound, never a field in the body.
///
/// **The material is pinned with the row or the row is pointless**: a node
/// resolves a connection by comparing the presented key against material
/// it holds, so a row whose material this node cannot pin names a member
/// it will never recognise. The schema has already bound that material to
/// the row's own keyhash (§1.1), so nothing here compares them.
pub fn horizon(
    view: &mut NodeView,
    pins: Option<&rhtn_transport::tls::Pins>,
    ids: &mut Vec<rhtn_crypto::identity::Identity>,
    peer: &Keyhash,
    body: &[u8],
) -> Option<Vec<u8>> {
    let a = HorizonAssertion::decode(body).ok()?;
    let series = u32::try_from(a.series).ok();
    let row = series.map(|series| rhtn_archive::topology::Asserted {
        node: a.node,
        patron: a.patron,
        series,
        adoption: a.adoption.unwrap_or([0; 32]),
        from: a.from.unwrap_or(0),
        anchor: a.anchor,
        slot: u8::try_from(a.slot.unwrap_or(0)).ok(),
        revision: a.revision,
    });
    let code = match row {
        None => SUBMISSION_REFUSED,
        Some(row) if !view.table.may_assert(peer, &row) => {
            // **a sender speaking past its own downline is refused, not
            // held**: it is not a row waiting for something, it is a row
            // this sender was never entitled to assert
            SUBMISSION_REFUSED
        }
        // **the disk before the table, as the operator's route does**
        // [2026-11-04]. The first cut of this wrote neither, so a row an
        // attached client asserted bound in memory and was gone at the
        // next start — and this node is restarted routinely. A node with
        // nowhere to keep one refuses rather than taking it into a table
        // that would forget it: the same discipline a grant act holds to,
        // since an act that did not survive a restart would not be one.
        Some(_) if view.horizon_dir.is_none() => SUBMISSION_REFUSED,
        Some(row) if a.retracted => {
            let dir = view.horizon_dir.clone().expect("checked above");
            if crate::horizon::write(
                &dir,
                &crate::horizon::Row {
                    row: row.clone(),
                    material: None,
                    retracted: true,
                },
            )
            .is_err()
            {
                SUBMISSION_REFUSED
            } else {
                match view
                    .table
                    .retract(&row.node, &row.patron, row.series, row.revision)
                {
                    rhtn_archive::topology::Asserting::Stale { .. } => SUBMISSION_REFUSED,
                    _ => SUBMISSION_ACCEPTED,
                }
            }
        }
        Some(row) => {
            // material first: a row bound without it names a member this
            // node could not recognise, and would report success
            // **a node not yet serving cannot pin**, and a row it took
            // without pinning would bind a member it will never recognise
            let pinned = match (pins, a.material.as_ref()) {
                (Some(p), Some(m)) => p.pin(row.node, m).is_ok(),
                _ => false,
            };
            let dir = view.horizon_dir.clone().expect("checked above");
            let kept = pinned
                && crate::horizon::write(
                    &dir,
                    &crate::horizon::Row {
                        row: row.clone(),
                        material: a.material.clone(),
                        retracted: false,
                    },
                )
                .is_ok();
            // **known as well as pinned**: `ids` is what lets this node
            // verify what the member signs, and a member bound without it
            // has its bundle and its profile refused the moment it
            // attaches, which detaches it [2026-11-04].
            if kept
                && let Some(id) = a
                    .material
                    .as_deref()
                    .and_then(rhtn_crypto::Identity::from_key_material)
                && !ids.iter().any(|k| k.keyhash == id.keyhash)
            {
                ids.push(id);
            }
            if !kept {
                SUBMISSION_REFUSED
            } else {
                match view.table.assert(&row) {
                    rhtn_archive::topology::Asserting::Stale { .. } => SUBMISSION_REFUSED,
                    _ => SUBMISSION_ACCEPTED,
                }
            }
        }
    };
    Some(SubmissionReply::code(a.nonce, code).encode())
}

/// Take one-time keys into the sender's own pool, bounded.  Over the
/// bound the deposit is refused with the code rather than silently
/// trimmed, so a client knows what its pool holds.
pub fn deposit(
    view: &mut NodeView,
    peer: &Keyhash,
    device: &[u8; 32],
    body: &[u8],
) -> Option<Vec<u8>> {
    let d = OneTimeDeposit::decode(body).ok()?;
    let cfg = view.prekeys.cfg.clone();
    let code = if d.keys.len() > cfg.per_deposit
        || view.prekeys.pool_size_for(peer, device) + d.keys.len() > cfg.pool
    {
        SUBMISSION_OVER_BOUND
    } else if view.prekeys.stock_for(*peer, *device, d.keys) {
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
///
/// **The submitter's binding goes with it** (`wire-format.md` §7.10,
/// design §14.2.2): the bundle of the device on this session, which this
/// node serves anyway, because the recipient cannot have asked for the
/// bundle of somebody who had not written to it yet.  Where this node
/// holds none the two-element form goes, as it always did.
pub fn relay(
    view: &NodeView,
    node: Option<&Arc<Node>>,
    peer: &Keyhash,
    device: &[u8; 32],
    body: &[u8],
) -> Option<Vec<u8>> {
    let r = RelaySubmission::decode(body).ok()?;
    let binding = view.prekeys.bundle_for(peer, device).cloned();
    let code = match node {
        None => SUBMISSION_REFUSED,
        Some(n) => match n.enqueue_for(
            r.recipient,
            r.device,
            relayed(*peer, &r.ciphertext, binding.as_deref()),
        ) {
            Ok(()) => SUBMISSION_ACCEPTED,
            // at a cap is a bound; a recipient this node holds no record
            // of, and a credential it has verified superseded, are both
            // refusals about somebody else
            Err(Refusal::AtCap) => SUBMISSION_OVER_BOUND,
            // a store that would not take it is this node's own trouble,
            // and the bare refusal is the honest answer: over-bound would
            // tell the sender the recipient's mailbox is full, which is a
            // claim about somebody else and may be false (§7.10)
            Err(Refusal::NoRecord | Refusal::Superseded | Refusal::NotStored) => SUBMISSION_REFUSED,
        },
    };
    Some(SubmissionReply::code(r.nonce, code).encode())
}

/// Register, refresh or withdraw where to ring the sender.  One endpoint
/// per relationship: registering again replaces what is held, and a
/// registration with no endpoint withdraws it (design §14.1.5).
pub fn wake(
    view: &mut NodeView,
    peer: &Keyhash,
    device: &[u8; 32],
    body: &[u8],
) -> Option<Vec<u8>> {
    let mut w = WakeRegistration::decode(body).ok()?;
    // **the signing key is decoded, wiped and not kept, and that is
    // deliberate rather than an oversight** [2026-11-06].
    // `wire-format.md` §7.10 field 5 carries the client's own key so this
    // node can sign the post a push service will accept (design §14.1.5)
    // — and nothing here posts a doorbell yet (SUB-14, milestone
    // after-5). Keeping it would mean writing a secret the client lent us
    // into this register's own files, which is a decision about those
    // files' contents and permissions that the author has not made. So it
    // is wiped where it can be seen to be wiped, and the registration is
    // still taken: the endpoint, its body key and its lapse are what a
    // node without the posting path can act on.
    if let Some(k) = w.signing_key.as_mut() {
        use zeroize::Zeroize;
        k.zeroize();
    }
    w.signing_key = None;
    let code = match view
        .wake
        .register(*peer, *device, w.endpoint, w.key, w.lapses_at)
    {
        Registered::Held | Registered::Withdrawn => SUBMISSION_ACCEPTED,
        Registered::Refused => SUBMISSION_OVER_BOUND,
    };
    Some(SubmissionReply::code(w.nonce, code).encode())
}
