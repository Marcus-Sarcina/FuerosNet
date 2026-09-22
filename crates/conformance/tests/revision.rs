//! New checks of the revised horizon and submission implementations.
use rhtn_archive::{tx::*,record::Record};
use rhtn_crypto::{identity::testkit::test_identity,SigningIdentity};
use rhtn_node::{view::NodeView,store::{Decision,KIND_TRANSACTION}};
use rhtn_client::horizon::{Horizon,Took,Woke};
use std::sync::Arc;

struct Quiet;
impl rhtn_node::Adjacency for Quiet {
    fn peers(&self)->Vec<[u8;32]> {vec![]}
    fn send(&self,_:&[u8;32],_:u64,_:&[u8]) {}
    fn request(&self,_:&[u8;32],_:u64,_:&[u8])->bool {false}
}
fn adoption(n:&SigningIdentity,p:&SigningIdentity,f:&SigningIdentity,slot:u8,time:u64)->Record {
    let bn=[rhtn_archive::genesis(&n.public.keyhash)];let bp=[rhtn_archive::genesis(&p.public.keyhash)];
    Record::parse(&envelope(TYPE_ADOPTION,&adoption_body(&Adoption {node:n.public.keyhash,patron:p.public.keyhash,locator:Locator {anchor:p.public.keyhash,path:vec![slot<<4],nibbles:1,seqno:Seqno {series:1,counter:0}},timestamp:time,key_material:None,evidence:Evidence::Transfer {former:f.public.keyhash,block:transfer_block(f,&n.public.keyhash,&p.public.keyhash)},presented_head:None,back:[&bn,&bp]}),&[n,p])).unwrap()
}
fn view(id:Arc<SigningIdentity>)->NodeView { NodeView::new(id.clone(),Locator::root(id.public.keyhash,Seqno {series:1,counter:0})) }

#[test]
fn h01_same_count_and_high_water_must_not_validate_a_different_record_set() {
    let (a,b,c)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let ids=vec![a.public.clone(),b.public.clone(),c.public.clone()];
    let old=adoption(&a,&b,&c,0,100);let replacement=adoption(&a,&b,&c,1,100);
    let last=envelope(TYPE_DISAVOWAL,&disavowal_body(&[rhtn_archive::genesis(&b.public.keyhash)],&b.public.keyhash,&c.public.keyhash,200,None),&[&b]);
    let mut original=Horizon::new(b.public.keyhash);
    for raw in [&old.bytes,&last] {assert_eq!(original.ingest(raw,&ids),Took::Applied);}
    let snapshot=original.materialise();
    let mut recovered=Horizon::new(b.public.keyhash);
    for raw in [&replacement.bytes,&last] { assert!(recovered.restore_record(raw.clone())); }
    let woke=recovered.wake(Some(&snapshot),&ids);
    assert!(matches!(woke,Woke::Replayed{..}),"design section 15.1.1: a different record set cannot reuse a stale fold just because its count and last record match; got {woke:?}");
    assert_eq!(recovered.locator_in(&a.public.keyhash,&b.public.keyhash).unwrap().path,vec![0x10]);
}

#[test]
fn h01_node_must_reject_a_derived_snapshot_from_another_identity() {
    let (a,b,c)=(Arc::new(test_identity("alice")),Arc::new(test_identity("bob")),test_identity("carol"));
    let ids=vec![a.public.clone(),b.public.clone(),c.public.clone()];let r=adoption(&a,&b,&c,0,100);
    let mut first=view(a.clone());let mut correct=view(b.clone());
    for v in [&mut first,&mut correct] {assert_eq!(v.take_object(&Quiet,&b.public.keyhash,KIND_TRANSACTION,&r.bytes,&ids),Decision::Stored);}
    assert_eq!(correct.slot_of(&a.public.keyhash),Some(0));
    let snapshot=first.materialise();
    let mut recovered=view(b);recovered.store=correct.store;
    let outcome=recovered.restore_materialised(Some(&snapshot),&ids);
    assert_eq!(recovered.slot_of(&a.public.keyhash),Some(0),"infra section 4.3: another node's derived slots must be discarded and reconstructed; got {outcome:?}");
}

#[test]
fn h02_replay_must_not_restore_a_pruned_departed_locator() {
    let (a,b,c)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let ids=vec![a.public.clone(),b.public.clone(),c.public.clone()];let r=adoption(&a,&b,&c,0,100);
    let d=envelope(TYPE_DEPARTURE,&departure_body(&[r.txid],&a.public.keyhash,&b.public.keyhash,Seqno {series:1,counter:1},101,None),&[&a]);
    let mut h=Horizon::new(b.public.keyhash);
    for raw in [&r.bytes,&d] {assert_eq!(h.ingest(raw,&ids),Took::Applied);}
    assert_eq!(h.distance(&a.public.keyhash),None,"control: the relationship ended");
    h.prune();assert!(h.places_of(&a.public.keyhash).is_empty(),"control: bounded view removed the departed party");
    assert!(matches!(h.wake(None,&ids),Woke::Replayed{..}));
    assert!(h.places_of(&a.public.keyhash).is_empty(),"light-client section 4.2: rebuilding from retained records must not resurrect a place outside the current horizon");
}

#[test]
fn s01_failed_prekey_deposit_must_not_be_acknowledged_as_accepted() {
    use rhtn_archive::{submission::*,prekey::PrekeyBundle};
    use rhtn_node::prekeys::{PrekeyService,PrekeyConfig};
    let dir=std::env::temp_dir().join(format!("rhtn-review-deposit-{}",std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let a=test_identity("alice");let b=Arc::new(test_identity("bob"));let mut v=view(b);
    v.prekeys=PrekeyService::at(&dir,PrekeyConfig::default()).unwrap();
    v.prekeys.publish(&vec![a.public.clone()],&PrekeyBundle::build(&a,1,b"opaque",100, &[0u8; 32])).unwrap();
    std::fs::rename(dir.join("prekeys"),dir.join("saved-prekeys")).unwrap();
    std::fs::write(dir.join("prekeys"),b"directory unavailable").unwrap();
    let req=OneTimeDeposit {keys:vec![vec![7]],nonce:[1;16]};
    let reply=SubmissionReply::decode(&rhtn_node::submissions::deposit(&mut v,&a.public.keyhash,&[0;32],&req.encode()).unwrap()).unwrap();
    let held=v.prekeys.pool_size(&a.public.keyhash);std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(held,0,"control: storage failure prevented stock");
    assert_ne!(reply.code,SUBMISSION_ACCEPTED,"wire section 7.10: accepted must mean the node actually took the submitted keys");
}

#[test]
fn s02_departure_must_forget_the_clients_registered_wake_endpoint() {
    let (a,b,c)=(test_identity("alice"),Arc::new(test_identity("bob")),test_identity("carol"));
    let ids=vec![a.public.clone(),b.public.clone(),c.public.clone()];let r=adoption(&a,&b,&c,0,100);let mut v=view(b.clone());
    assert_eq!(v.take_object(&Quiet,&b.public.keyhash,KIND_TRANSACTION,&r.bytes,&ids),Decision::Stored);
    assert_eq!(v.wake.register(a.public.keyhash,[0;32],Some("https://wake.example/client".into()),Some(vec![1;32]),None),rhtn_node::wake::Registered::Held);
    let d=envelope(TYPE_DEPARTURE,&departure_body(&[r.txid],&a.public.keyhash,&b.public.keyhash,Seqno {series:1,counter:1},101,None),&[&a]);
    assert_eq!(v.take_object(&Quiet,&b.public.keyhash,KIND_TRANSACTION,&d,&ids),Decision::Stored);
    assert!(!v.table.patrons(&a.public.keyhash).contains(&b.public.keyhash),"control: relationship ended");
    assert!(v.wake.get(&a.public.keyhash).is_none(),"infra section 6.1: the routable wake identifier must be forgotten when its relationship ends");
}

#[test]
fn s04_prekey_issuance_must_not_persist_requester_subject_metadata() {
    use rhtn_archive::prekey::*;
    use rhtn_node::prekeys::{PrekeyService,PrekeyConfig};
    let dir=std::env::temp_dir().join(format!("rhtn-review-issuance-{}",std::process::id()));
    let (s,r)=(test_identity("alice"),test_identity("carol"));
    let mut pool=PrekeyService::at(&dir,PrekeyConfig::default()).unwrap();
    pool.publish(&vec![s.public.clone()],&PrekeyBundle::build(&s,1,b"opaque",100, &[0u8; 32])).unwrap();
    assert!(pool.stock(s.public.keyhash,vec![vec![7]]));
    let req=PrekeyRequest::One {subject:s.public.keyhash,one_time:true,nonce:[1;16],device: Some([0u8; 32])}.encode();
    assert_eq!(PrekeyReply::decode(&pool.answer(&r.public.keyhash,&req,101).unwrap()).unwrap().one_time,Some(vec![7]));
    pool.expire(4000);
    let text=std::fs::read_to_string(dir.join("prekeys/issued")).unwrap_or_default();
    std::fs::remove_dir_all(&dir).unwrap();
    let hex=|k:&[u8]|k.iter().map(|b|format!("{b:02x}")).collect::<String>();
    let prefix=format!("{} {} ",hex(&r.public.keyhash),hex(&s.public.keyhash));
    assert!(!text.lines().any(|l|l.starts_with(&prefix)),"infra section 6: keep no record of who asked for whose bundle, including after the rate window expires");
}
