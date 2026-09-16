//! Ask a running node the questions that change nothing.
//!
//! Attach, then the read-only request types: resolve a locator
//! (`wire-format.md` §7.7), fetch an archive (§7.9), query a catalog
//! (§6.4). Those are the three §9.2 calls read-only, answering the same
//! way however often they are replayed, which is what makes them safe to
//! send from a tool whose purpose is to look.
//!
//! **Nothing else is sent from here.** A prekey fetch consumes a one-time
//! key and a resource request has an application effect, whatever the HTTP
//! method inside; neither belongs behind a command that only looks. The
//! omission is the feature.
//!
//! **It authenticates as a real identity and pins the node it dials.**
//! There is no unauthenticated mode to add, the transport having none
//! (`wire-format.md` §9.1).

use rhtn_archive::catalog::{CatalogQuery, CatalogReply, REQUEST_CATALOG_QUERY};
use rhtn_archive::chain::{ArchiveReply, ArchiveRequest, REQUEST_ARCHIVE};
use rhtn_archive::record::Record;
use rhtn_archive::Keyhash;
use rhtn_crypto::{Identity, SigningIdentity};
use rhtn_node::resolution::{REQUEST_RESOLVE, ResolveReply, ResolveRequest};
use rhtn_transport::session::{AttachOutcome, ClientConfig, Log, Session, attach};
use rhtn_transport::tls::{self, Pins};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What a probe asks.
#[derive(Debug, Clone)]
pub enum Ask {
    /// Where a subject is, from an anchor and a path (`wire-format.md` §7.7).
    Resolve { subject: Keyhash, anchor: Keyhash, path: Vec<u8>, nibbles: u64 },
    /// A subject's archive, newest first where no head is named (§7.9).
    Archive { subject: Keyhash, max_records: u64 },
    /// What a node's catalog answers this asker (§6.4).
    Catalog { service_type: Option<String> },
}

/// Attach to `target` at `addr` as `me`, pinning everyone in `known`.
pub async fn attached(me: SigningIdentity, known: &[Identity], target: Keyhash, addr: SocketAddr) -> Result<(Session, quinn::Endpoint), String> {
    let pins = Pins::new();
    pins.pin_identity(&me.public);
    for id in known {
        pins.pin_identity(id);
    }
    let cfg = ClientConfig {
        identity: Arc::new(me),
        pins,
        capabilities: BTreeMap::new(),
        attestation: None,
        filter: None,
        sibling_cache: Arc::new(Mutex::new(Vec::new())),
        addresses: Arc::new(Mutex::new(std::collections::HashMap::from([(target, vec![addr])]))),
        tls: Arc::new(Mutex::new(Default::default())),
        connect_timeout: Duration::from_secs(5),
        on_reachability: None,
        log: Log::default(),
    };
    let ep = tls::client_endpoint("0.0.0.0:0".parse().unwrap()).map_err(|e| e.to_string())?;
    match attach(&cfg, &ep, target, addr, false).await {
        AttachOutcome::Attached(s) => Ok((s, ep)),
        other => Err(format!("{other:?}")),
    }
}

/// Whether each record's back-pointers name the one that follows it in
/// the batch, which arrives newest first (`wire-format.md` §7.9).
///
/// The subject's own list is the one that must match: a record carries one
/// back-pointer list per signer, and only the subject's continues the
/// subject's chain.  A record whose predecessor is absent from the batch
/// is where the batch stops being a chain, and the caller is told which.
fn linked(records: &[Record], subject: &Keyhash) -> Result<(), String> {
    for (i, pair) in records.windows(2).enumerate() {
        let (newer, older) = (&pair[0], &pair[1]);
        let Some(back) = newer.back_pointers_of(subject) else {
            return Err(format!("record {i} carries no back-pointers for the subject, so the batch is not its chain"));
        };
        if !back.contains(&older.txid) {
            return Err(format!(
                "the batch is not a chain: record {i} ({}) does not name record {} ({}) that follows it",
                crate::inspect::hex(&newer.txid),
                i + 1,
                crate::inspect::hex(&older.txid)
            ));
        }
    }
    Ok(())
}

/// Send one ask and describe the answer.  A refusal is described, not
/// hidden: its code is what the caller came for.
pub async fn ask(session: &Session, ask: &Ask, nonce: [u8; 16]) -> Result<String, String> {
    match ask {
        Ask::Resolve { subject, anchor, path, nibbles } => {
            let req = ResolveRequest { subject: *subject, anchor: *anchor, path: path.clone(), nibbles: *nibbles, nonce };
            let bytes = session.request(REQUEST_RESOLVE, &req.encode()).await?;
            let reply = ResolveReply::decode(&bytes).map_err(|e| format!("the reply does not decode: {e}"))?;
            Ok(format!("{reply:#?}\n"))
        }
        Ask::Archive { subject, max_records } => {
            let req = ArchiveRequest { subject: *subject, head: None, max_records: *max_records, stop_before: None, nonce };
            let bytes = session.request(REQUEST_ARCHIVE, &req.encode()).await?;
            let reply = ArchiveReply::decode(&bytes).map_err(|e| format!("the reply does not decode: {e}"))?;
            let mut out = format!("records   {}\nmore      {}\n", reply.records.len(), reply.more);
            let parsed: Result<Vec<Record>, String> = reply.records.iter().map(|r| Record::parse(r)).collect();
            let parsed = parsed.map_err(|e| format!("a record in the batch does not parse: {e}"))?;
            for r in &parsed {
                out.push_str(&format!("  {}\n", crate::inspect::hex(&r.txid)));
            }
            // §7.9: the requester verifies the chain itself, and a holder
            // cannot be trusted to have walked correctly.  One hash
            // comparison per record, over records already parsed.
            linked(&parsed, subject)?;
            // and what no requester can check: with no head asked for
            // there is nothing to match the first record against, so its
            // newestness is the holder's claim and is said to be
            out.push_str("chain     links verified; newestness is the holder's claim, no head having been requested\n");
            Ok(out)
        }
        Ask::Catalog { service_type } => {
            let req = CatalogQuery { service_type: service_type.clone(), nonce };
            let bytes = session.request(REQUEST_CATALOG_QUERY, &req.encode()).await?;
            let reply = CatalogReply::decode(&bytes).map_err(|e| format!("the reply does not decode: {e}"))?;
            let mut out = format!("entries   {}\n", reply.entries.len());
            if let Some(c) = &reply.continuation {
                out.push_str(&format!("more of   {c}\n"));
            }
            for e in &reply.entries {
                out.push_str(&format!("  {} bytes\n", e.len()));
            }
            Ok(out)
        }
    }
}
