//! Additional checks of newly implemented or changed paths in this review.
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_archive::tx::*;
use std::sync::Arc;

/// The subnet the fixtures' positions are in (`wire-format.md` §2.3 field 4).
const SUBNET: [u8; 32] = [0xdb; 32];

// 2026-11-20, slice 2: the one-time pool this was filed against is gone;
// a node key's private half is what a node deletes now, and it is unlinked
// as it goes rather than at a save, so a reload is the whole claim
#[test]
fn r11_a_reload_must_not_resurrect_a_deleted_node_key() {
    use rhtn_node::node_keys::{NodeKeyService,NodeKeyConfig};
    let dir=std::env::temp_dir().join(format!("rhtn-review-snapshot-{}",std::process::id()));
    let _=std::fs::remove_dir_all(&dir);
    let (n,r)=(test_identity("bob"),test_identity("alice"));
    let cfg=||NodeKeyConfig {rotate_secs:0,grace_secs:0};
    let mut keys=NodeKeyService::at(&dir,cfg()).unwrap();
    keys.newest(&n,&r.public.keyhash,100);
    keys.newest(&n,&r.public.keyhash,100);
    assert_eq!(keys.held(&r.public.keyhash),vec![1,2],"control: rotated, the retired key held until pruned");
    assert_eq!(keys.prune(&r.public.keyhash,&|_|false,100),1,"control: the key nothing is sealed to is deleted");
    let reloaded=NodeKeyService::at(&dir,cfg()).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(reloaded.held(&r.public.keyhash),vec![2],"wire section 7.8: a reload must not resurrect a deleted node key");
}

#[tokio::test]
async fn c01_archive_probe_must_reject_a_disconnected_batch() {
    use rhtn_cli::probe::{self,Ask};
    use rhtn_archive::{chain::{ArchiveRequest,ArchiveReply},record::{Record,SigStatus}};
    use rhtn_transport::{session::{NodeConfig,Node},tls::{self,Pins}};
    let (a,b)=(test_identity("alice"),Arc::new(test_identity("bob")));
    let ids=vec![a.public.clone(),b.public.clone()];
    let genesis=[rhtn_archive::genesis(&b.public.keyhash)];
    let first=envelope(TYPE_DISAVOWAL,&disavowal_body(&genesis,&b.public.keyhash,&a.public.keyhash,100,None),&[&b]);
    let second=envelope(TYPE_DISAVOWAL,&disavowal_body(&genesis,&b.public.keyhash,&a.public.keyhash,101,None),&[&b]);
    for raw in [&first,&second] { assert_eq!(Record::parse(raw).unwrap().check_signatures(&ids),SigStatus::Verified); }
    // Both records point at genesis; second does not point at first.
    // Return an actual good chain first, then this disconnected one.
    let fid=Record::parse(&first).unwrap().txid;
    let good=envelope(TYPE_DISAVOWAL,&disavowal_body(&[fid],&b.public.keyhash,&a.public.keyhash,101,None),&[&b]);
    let pins=Pins::new();for id in &ids { pins.pin_identity(id); }
    let mut cfg=NodeConfig::defaults(b.clone(),pins,30);
    cfg.on_request=Some(Arc::new(move |_,_,_,body| {
        let req=ArchiveRequest::decode(&body).unwrap();
        let records=if req.nonce==[1;16] {vec![good.clone(),first.clone()]} else {vec![second.clone(),first.clone()]};
        Box::pin(async move {Some(ArchiveReply {nonce:req.nonce,records,more:false,frontier:Vec::new()}.encode())})
    }));
    let ep=tls::server_endpoint(cfg.presenter(),"127.0.0.1:0".parse().unwrap()).unwrap();let addr=ep.local_addr().unwrap();
    let task=tokio::spawn(Node::new(cfg).serve(ep.clone()));
    let (session,_client)=probe::attached(a,&ids,b.public.keyhash,addr).await.unwrap();
    let ask=Ask::Archive {subject:b.public.keyhash,max_records:2};
    assert!(probe::ask(&session,&ask,[1;16]).await.is_ok(),"control: a valid batch can be inspected");
    let bad=probe::ask(&session,&ask,[2;16]).await;
    session.conn.close(0u32.into(),b"done");ep.close(0u32.into(),b"done");task.abort();
    assert!(bad.is_err(),"wire section 7.9: requester must verify internal chain links, even when no head was requested; got {bad:?}");
}

// functional: OPS-019
// functional: TOOL-002
#[test]
fn cli_minted_identity_is_readable_by_daemon_and_survives_a_refused_overwrite() {
    let dir=std::env::temp_dir().join(format!("rhtn-review-mint-{}",std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();let path=dir.join("identity");
    let id=rhtn_cli::keys::mint(&path,[[31;32],[32;32]]).unwrap();
    let bytes=std::fs::read(&path).unwrap();
    assert_eq!(rhtn_daemon::service::read_identity(&path).unwrap().public.keyhash,id.keyhash,"CLI-generated identities meet the daemon's length and ownership requirements");
    assert!(rhtn_cli::keys::mint(&path,[[41;32],[42;32]]).is_err(),"an existing identity must not be replaced");
    assert_eq!(std::fs::read(&path).unwrap(),bytes,"refused mint leaves the original identity intact");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn r12_departure_in_reissued_series_must_end_the_same_relationship() {
    use rhtn_archive::{record::Record,topology::{Table,Evaluation},series::SeriesChain};
    struct Missing;
    impl rhtn_archive::walk::Fetch for Missing { fn fetch(&self,_:&[u8;32])->Option<Vec<u8>> {None} }
    let (n,p,f)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let bn=[rhtn_archive::genesis(&n.public.keyhash)];let bp=[rhtn_archive::genesis(&p.public.keyhash)];
    let a=envelope(TYPE_ADOPTION,&adoption_body(&Adoption {node:n.public.keyhash,patron:p.public.keyhash,locator:Locator {anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno {series:1,counter:0}, subnet: Some(SUBNET) },timestamp:100,key_material:None,evidence:Evidence::Transfer {former:f.public.keyhash,block:transfer_block(&f,&n.public.keyhash,&p.public.keyhash)},presented_head:None,back:[&bn,&bp], carrying: None }),&[&n,&p]);
    let a=Record::parse(&a).unwrap();let backs=[a.txid];
    let r=envelope(TYPE_REISSUE,&reissue_body([&backs,&backs],&n.public.keyhash,&p.public.keyhash,Seqno {series:1,counter:0},2,101),&[&n,&p]);
    let r=Record::parse(&r).unwrap();
    let ids=vec![n.public.clone(),p.public.clone(),f.public.clone()];
    assert!(SeriesChain::from_records(&[a.bytes.clone(),r.bytes.clone()],&ids).is_ok(),"control: complete countersigned proof of series 2");
    let d=envelope(TYPE_DEPARTURE,&departure_body(&[r.txid],&n.public.keyhash,&p.public.keyhash,Seqno {series:2,counter:1},102,None),&[&n]);
    let d=Record::parse(&d).unwrap();
    let mut table=Table::with_me(p.public.keyhash);
    for rec in [&a,&r] { table.apply_with(rec,&ids,&Missing,None,Evaluation::Deferred).unwrap(); }
    assert!(table.patrons(&n.public.keyhash).contains(&p.public.keyhash),"control: reissue does not end the relationship");
    table.apply_with(&d,&ids,&Missing,None,Evaluation::Deferred).unwrap();
    assert!(!table.patrons(&n.public.keyhash).contains(&p.public.keyhash),"wire sections 4.2 and 4.6: departure under the proven reissued series must sever the relationship");
}
