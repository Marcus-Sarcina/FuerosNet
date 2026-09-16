//! Archive fetch over a live session (`wire-format.md` §7.9;
//! `light-client-requirements.md` §2, "serve archive requests for your own
//! archive"): a node answers request type 2 for its own archive, head
//! first, and with an empty batch for a subject it is not.

mod common;

use common::*;
use rhtn_archive::chain::{ArchiveReply, ArchiveRequest, REQUEST_ARCHIVE};
use rhtn_archive::record::Record;
use rhtn_node::resolution::{AnchorTable, Ingestion};
use rhtn_node::runtime::LiveNode;
use rhtn_transport::session::*;

const I: u64 = 30;

#[tokio::test]
async fn a_node_serves_its_own_archive_on_request_and_an_empty_batch_for_another_subject() {
    let mut s = Signers::new();
    let a_n = s.adopt("bob", "alice", "alice", &[0], 1);
    let records = [&a_n];
    let mut n_view = view_of("bob", table_of("bob", &s, &records, &["alice", "bob"]), "alice", &[0], s.clock);
    // bob's archive: the presence the adoption rests on, then the adoption
    n_view.archive = s.archive_of("bob");
    assert_eq!(n_view.archive.len(), 2);
    let n = LiveNode::start(node_cfg("bob", I), n_view, ids(), AnchorTable::new(0, Ingestion::UnverifiedGossip));
    let ccfg = client_cfg("carol");
    know(&ccfg, "bob", n.addr);
    let AttachOutcome::Attached(c) = attach(&ccfg, &client_ep(), kh("bob"), n.addr, false).await else { panic!("C attaches to N") };
    // head absent: the holder's newest record, the recovery case
    let req = ArchiveRequest { subject: kh("bob"), head: None, max_records: 16, stop_before: None, nonce: [1; 16] };
    let reply = ArchiveReply::decode(&c.request(REQUEST_ARCHIVE, &req.encode()).await.expect("answered")).unwrap();
    assert_eq!(reply.nonce, [1; 16]);
    assert_eq!(reply.records.len(), 2);
    assert_eq!(Record::parse(&reply.records[0]).unwrap().txid, a_n.txid, "head first");
    assert!(!reply.more);
    // a subject this node is not: an empty batch, which says nothing about that archive
    let other = ArchiveRequest { subject: kh("alice"), head: None, max_records: 16, stop_before: None, nonce: [2; 16] };
    let reply = ArchiveReply::decode(&c.request(REQUEST_ARCHIVE, &other.encode()).await.expect("answered")).unwrap();
    assert_eq!(reply.nonce, [2; 16]);
    assert!(reply.records.is_empty() && !reply.more);
}
