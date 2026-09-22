//! Current assertions derived from functional_tests.md and the September 15 specifications.
use rhtn_archive::{tx::*,record::Record};
use rhtn_crypto::{SigningIdentity,identity::testkit::test_identity};
use rhtn_client::horizon::{Horizon,Took,Woke};
use rhtn_codec::{encode::*,cose};
use std::sync::{Arc,Mutex};
use rhtn_node::{view::NodeView,store::{Decision,KIND_TRANSACTION}};
fn adoption(n:&SigningIdentity,p:&SigningIdentity,f:&SigningIdentity,slot:u8,time:u64)->Record {
 let bn=[rhtn_archive::genesis(&n.public.keyhash)];let bp=[rhtn_archive::genesis(&p.public.keyhash)];
 Record::parse(&envelope(TYPE_ADOPTION,&adoption_body(&Adoption{node:n.public.keyhash,patron:p.public.keyhash,locator:Locator{anchor:p.public.keyhash,path:vec![slot<<4],nibbles:1,seqno:Seqno{series:1,counter:0}},timestamp:time,key_material:None,evidence:Evidence::Transfer{former:f.public.keyhash,block:transfer_block(f,&n.public.keyhash,&p.public.keyhash)},presented_head:None,back:[&bn,&bp]}),&[n,p])).unwrap()
}
fn endpoint(n:&SigningIdentity,series:u32,counter:u32,port:u16)->Vec<u8>{
 let mut p=vec![];rhtn_transport::session::NetworkPoint::new([127,0,0,1],Some(port as u64)).encode(&mut p);
 let mut payload=vec![];emit_map_head(&mut payload,3);emit_uint(&mut payload,1);emit_bstr(&mut payload,&n.public.keyhash);emit_uint(&mut payload,2);emit_array_head(&mut payload,1);payload.extend(p);emit_uint(&mut payload,3);Seqno{series,counter}.emit(&mut payload);
 let sig=n.sign1_ed_unnamed(cose::aad::ENDPOINTS,&payload);let mut out=vec![];emit_map_head(&mut out,4);out.extend_from_slice(&payload[1..]);emit_uint(&mut out,4);out.extend(sig);out
}
fn horizon()->(Horizon,SigningIdentity,SigningIdentity,Vec<rhtn_crypto::Identity>,Record){
 let (a,b,c)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));let ids=vec![a.public.clone(),b.public.clone(),c.public.clone()];let r=adoption(&a,&b,&c,0,100);let mut h=Horizon::new(b.public.keyhash);assert_eq!(h.ingest(&r.bytes,&ids),Took::Applied);(h,a,b,ids,r)
}
#[test]
fn e01_client_equal_sequence_conflict_must_leave_neither_endpoint_current(){
 let (mut h,a,_,ids,_)=horizon();let one=endpoint(&a,1,1,5001);let two=endpoint(&a,1,1,5002);
 assert_eq!(h.ingest_endpoint(&one,&ids),Took::Applied);assert_eq!(h.ingest_endpoint(&one,&ids),Took::Duplicate);
 assert_eq!(h.endpoints_of(&a.public.keyhash).len(),1);
 let got=h.ingest_endpoint(&two,&ids);
 assert!(h.endpoints_of(&a.public.keyhash).is_empty(),"W 10.1.2: neither conflicting endpoint is current; result {got:?}");
}
#[test]
fn e02_client_must_not_admit_an_unproved_second_endpoint_series(){
 let (mut h,a,_,ids,_)=horizon();assert_eq!(h.ingest_endpoint(&endpoint(&a,1,1,5001),&ids),Took::Applied);
 let other=endpoint(&a,99,1,5002);let got=h.ingest_endpoint(&other,&ids);
 assert_ne!(got,Took::Applied,"W 10.1.2: a second series without its reissue proof must not become current");
 assert_eq!(h.endpoints_of(&a.public.keyhash).len(),1);
}
#[test]
fn e03_horizon_restore_must_preserve_accepted_endpoint_reachability(){
 let (mut h,a,b,ids,r)=horizon();assert_eq!(h.ingest_endpoint(&endpoint(&a,1,1,5001),&ids),Took::Applied);
 let before=h.endpoints_of(&a.public.keyhash);assert_eq!(before.len(),1);let snap=h.materialise();
 let mut restored=Horizon::new(b.public.keyhash);assert!(restored.restore_record(r.bytes));assert_eq!(restored.wake(Some(&snap),&ids),Woke::Current);
 assert_eq!(restored.endpoints_of(&a.public.keyhash),before,"L 4.2: a current restored horizon must retain the endpoints needed to route around a dark patron");
}
#[derive(Default)]struct Adj{peers:Vec<[u8;32]>,sent:Mutex<Vec<Vec<u8>>>}
impl rhtn_node::Adjacency for Adj{fn peers(&self)->Vec<[u8;32]>{self.peers.clone()}fn send(&self,_:&[u8;32],_:u64,b:&[u8]){self.sent.lock().unwrap().push(b.to_vec());}fn request(&self,_:&[u8;32],_:u64,_:&[u8])->bool{false}}
#[test]
fn t01_node_must_not_store_or_forward_a_second_occupant_of_a_filled_slot(){
 let (a,b,c,d)=(test_identity("alice"),Arc::new(test_identity("bob")),test_identity("carol"),test_identity("w1"));let ids=vec![a.public.clone(),b.public.clone(),c.public.clone(),d.public.clone()];let first=adoption(&a,&b,&d,0,100);let other=adoption(&c,&b,&d,0,101);
 let mut v=NodeView::new(b.clone(),Locator::root(b.public.keyhash,Seqno{series:1,counter:0}));v.attached.insert(d.public.keyhash);let adj=Adj{peers:vec![d.public.keyhash],..Default::default()};
 assert_eq!(v.take_object(&adj,&b.public.keyhash,KIND_TRANSACTION,&first.bytes,&ids),Decision::Stored);assert_eq!(v.slot_of(&a.public.keyhash),Some(0));adj.sent.lock().unwrap().clear();
 let result=v.take_object(&adj,&b.public.keyhash,KIND_TRANSACTION,&other.bytes,&ids);
 assert_eq!(v.slot_of(&a.public.keyhash),Some(0),"incumbent remains in the table");
 let held=v.store.transactions().any(|r|r.txid==other.txid);let sent=!adj.sent.lock().unwrap().is_empty();
 assert!(!held&&!sent,"D 3.1: filled-slot adoption is refused at storage, not stored and flooded before table refusal; decision={result:?}, stored={held}, sent={sent}");
}
#[test]
fn s04_upgrade_must_remove_the_previously_written_requester_subject_log(){
 use rhtn_node::prekeys::{PrekeyService,PrekeyConfig};
 let dir=std::env::temp_dir().join(format!("rhtn-review-legacy-{}",std::process::id()));std::fs::create_dir_all(dir.join("prekeys")).unwrap();let path=dir.join("prekeys/issued");
 std::fs::write(&path,format!("{} {} 100 1\n","aa".repeat(32),"bb".repeat(32))).unwrap();
 let mut p=PrekeyService::at(&dir,PrekeyConfig::default()).unwrap();p.expire(10000);p.save(&dir).unwrap();let retained=std::fs::read_to_string(path).unwrap_or_default();std::fs::remove_dir_all(dir).unwrap();
 assert!(retained.is_empty(),"I 6: upgrading/expiring must not leave the old durable requester-subject log indefinitely");
}
#[test]
fn s02_recovery_must_forget_the_superseded_clients_wake_endpoint(){
 let (old,p,former,new)=(test_identity("alice"),Arc::new(test_identity("bob")),test_identity("carol"),test_identity("alice2"));let ids=vec![old.public.clone(),p.public.clone(),former.public.clone(),new.public.clone()];let a=adoption(&old,&p,&former,0,100);
 let mut v=NodeView::new(p.clone(),Locator::root(p.public.keyhash,Seqno{series:1,counter:0}));let adj=Adj::default();assert_eq!(v.take_object(&adj,&p.public.keyhash,KIND_TRANSACTION,&a.bytes,&ids),Decision::Stored);
 assert_eq!(v.wake.register(old.public.keyhash,Some("https://wake.example/old".into()),Some(vec![8;32]),None),rhtn_node::wake::Registered::Held);
 let response=recovery_response(&p,&new,&[7;32],&old.public.keyhash);let mut statement=vec![];emit_array_head(&mut statement,3);for k in [&old.public.keyhash,&new.public.keyhash,&p.public.keyhash]{emit_bstr(&mut statement,k);}
 let mut block=vec![];emit_map_head(&mut block,3);emit_uint(&mut block,1);emit_bstr(&mut block,&old.public.keyhash);emit_uint(&mut block,2);emit_array_head(&mut block,1);block.extend(response);emit_uint(&mut block,3);block.extend(sign_block(&old,cose::aad::SUCCESSOR,&statement));
 let bn=[rhtn_archive::genesis(&new.public.keyhash)];let bp=[a.txid];let body=adoption_body(&Adoption{node:new.public.keyhash,patron:p.public.keyhash,locator:Locator{anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno{series:2,counter:0}},timestamp:101,key_material:None,evidence:Evidence::Recovery(block),presented_head:None,back:[&bn,&bp]});let raw=envelope(TYPE_ADOPTION,&body,&[&new,&p]);
 assert_eq!(v.take_object(&adj,&p.public.keyhash,KIND_TRANSACTION,&raw,&ids),Decision::Stored);assert!(v.is_superseded(&old.public.keyhash),"control: recovery was effective");assert_eq!(v.slot_of(&new.public.keyhash),Some(0));
 assert!(v.wake.get(&old.public.keyhash).is_none(),"I 6.1: recovery ended the old serving relationship but retained its wake URL/key");
}
#[test]
fn g01_changed_standing_grant_must_rewrite_derived_rows(){
 use rhtn_node::resources::{Gateway,Binding,Row};use rhtn_archive::topology::Table;use std::collections::BTreeSet;
 let (owner,res)=([41;32],[42;32]);let table=Table::with_me(owner);let mut g=Gateway::default();
 g.bind(res,Binding{owner,authority:"test.internal".into(),backend:None,declared_roles:BTreeSet::from(["reader".into()])});
 let allow=Row{roles:BTreeSet::from(["reader".into()]),connect:true};let deny=Row{roles:BTreeSet::new(),connect:false};
 g.stand(res,allow.clone()).unwrap();g.refresh(&table);assert_eq!(g.row(&res,&owner),Some(&allow));
 g.stand(res,deny.clone()).unwrap();g.refresh(&table);
 assert_eq!(g.row(&res,&owner),Some(&deny),"I 10.2: changing standing policy must update rows it generated, not preserve them as if individually assigned");
}
#[test]
fn p01_a_set_of_only_disavowed_candidates_has_no_usable_joint_standing(){
 use rhtn_policy::{Evidence,Policy,ReferenceMetric};
 let mut ev=Evidence::new(0u8);ev.adopt(0,1);ev.disavow(0,1);
 let result=ReferenceMetric::default().evaluate(&ev,&[1]);
 assert_eq!(result.individual,vec![(1,0.0)]);assert!(result.admitted.is_empty());
 assert_eq!(result.joint,0.0,"D 18.5 and Evaluation's usable-set contract: denied candidates cannot retain joint admitted capacity");
}
