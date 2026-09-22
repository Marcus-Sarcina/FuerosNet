//! The reference policy's reading of a presented archive (design §16.1,
//! §16.2.1, §16.7): a prospective patron walks what it is shown, counts
//! only transactions whose other participant it already recognises, and
//! weights each by the flow its own graph can push to that participant.
//! Records naming unknown identities are not weighed less; they are not
//! weighed.  A reviewed archive informs the initial trust state and
//! installs no edge.

use crate::evidence::Evidence;
use crate::policy::Policy;
use rhtn_archive::record::Record;
use rhtn_archive::topology::initial_trust;
use rhtn_archive::{Keyhash, Txid};

/// What an evaluator derived from a presented archive.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ArchiveStanding {
    /// Each record with a recognised counterparty: the counterparty, the
    /// record, and the evaluator's own standing for that counterparty.
    pub weighed: Vec<(Keyhash, Txid, f64)>,
    /// Records whose every other participant is a stranger: ignored, not
    /// discounted.
    pub ignored: usize,
    /// The sum of the weights: the standing the archive informs.
    pub total: f64,
}

/// Evaluate `records`, presented by `subject`, from the observer's evidence
/// under `policy`.  Only the intersection with identities the evidence
/// names is weighed (design §16.1: "A patron following it weighs the
/// intersection and nothing else").
pub fn evaluate_archive(
    policy: &dyn Policy<Keyhash>,
    ev: &Evidence<Keyhash>,
    subject: &Keyhash,
    records: &[Record],
) -> ArchiveStanding {
    let mut known = ev.known();
    known.remove(subject);
    let trust = initial_trust(&known, subject, records);
    let mut out = ArchiveStanding::default();
    let mut counted = std::collections::BTreeSet::new();
    for (counterparty, txids) in &trust.by_counterparty {
        let weight = policy.score(ev, counterparty);
        for t in txids {
            out.weighed.push((*counterparty, *t, weight));
            out.total += weight;
            counted.insert(*t);
        }
    }
    out.ignored = records
        .iter()
        .filter(|r| !counted.contains(&r.txid))
        .count();
    out
}
