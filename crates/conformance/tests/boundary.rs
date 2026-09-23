//! The newly implemented application boundary against a real serving node.
use rhtn_ffi::{client::Participant,device::*,types::*};
use rhtn_crypto::identity::testkit::test_identity;
use std::sync::Arc;
struct Shell;
impl Proximity for Shell {
 fn supported(&self)->Vec<Channel> {vec![]}
 fn run(&self,_:Channel,_:Vec<u8>)->ChannelOutcome {ChannelOutcome::Unavailable}
 fn resolution_m(&self,_:Channel)->Option<u64> {None}
}
impl Camera for Shell {fn capture(&self,_:Ask)->Vec<u8>{vec![7;64]}}
impl Clock for Shell {fn now_ms(&self)->u64 {1_800_000_000_000} fn wait_ms(&self,_:u64){}}
impl Random for Shell {fn fill(&self,n:u32)->Vec<u8>{vec![19;n as usize]}}
impl Operator for Shell {fn ask(&self,_:String)->bool {true}}
impl Notices for Shell {fn told(&self,_:Told){}}
impl Storage for Shell {fn read(&self,_:String)->Option<Vec<u8>>{None} fn write(&self,_:String,_:Vec<u8>)->bool{true}}
fn platform()->Platform {let s=Arc::new(Shell);Platform {proximity:s.clone(),camera:s.clone(),clock:s.clone(),random:s.clone(),operator:s.clone(),notices:s.clone(),storage:s}}
fn seeds(n:&str)->Vec<u8> {let mut b=rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{n}:ed25519-seed").as_bytes()).to_vec();b.extend(rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{n}:ml-dsa-65-seed").as_bytes()));b}

#[tokio::test(flavor="multi_thread",worker_threads=4)]
async fn s03_ffi_send_must_report_a_refused_relay_submission() {
 use rhtn_node::{view::NodeView,runtime::LiveNode,resolution::{AnchorTable,Ingestion}};
 use rhtn_archive::tx::{Locator,Seqno};
 use rhtn_transport::{session::NodeConfig,tls::Pins};
 let (a,b,c)=(test_identity("alice"),Arc::new(test_identity("bob")),test_identity("carol"));
 let ids=vec![a.public.clone(),b.public.clone(),c.public.clone()];let pins=Pins::new();for id in &ids {pins.pin_identity(id);}
 let mut view=NodeView::new(b.clone(),Locator::root(b.public.keyhash,Seqno {series:1,counter:0}));
 let mut ck=rhtn_client::payload::PayloadKeys::generate(&mut |b|b.fill(31),1_800_000_000);
 view.prekeys.publish(&ids,&ck.bundle(&c,c.public.ed.as_bytes(),1_800_000_000)).unwrap();
 let mut cfg=NodeConfig::defaults(b.clone(),pins,30);cfg.queue_cap=Some(0);
 let node=LiveNode::start(cfg,view,ids.clone(),AnchorTable::new(0,Ingestion::UnverifiedGossip));
 assert!(node.node.serves(&c.public.keyhash),"control: Carol is known through her held bundle");
 let known=ids.iter().map(|i|i.key_material()).collect();
 let p=Arc::new(tokio::task::spawn_blocking(move ||Participant::start(seeds("alice"),known,platform()).unwrap()).await.unwrap());
 let (q,addr,bid,cid)=(p.clone(),node.addr.to_string(),b.public.keyhash.to_vec(),c.public.keyhash.to_vec());
 tokio::task::spawn_blocking(move ||q.attach(bid,vec![addr],vec![cid])).await.unwrap().unwrap();
 let recipient=c.public.keyhash;
 assert!(matches!(node.node.enqueue(recipient,vec![1]),Err(rhtn_transport::queue::Refusal::AtCap)),"control: configured queue bound refuses delivery for this recipient");
 let q=p.clone();let sent=tokio::task::spawn_blocking(move ||q.send(recipient.to_vec(),KIND_APPLICATION,b"message that cannot be taken".to_vec())).await.unwrap();
 assert_eq!(node.node.queued(&recipient),0,"control: no message was queued");
 node.endpoint.close(0u32.into(),b"done");
 assert!(sent.is_err(),"light-client section 9: the shell must receive the serving node's refusal instead of success; got {sent:?}");
}

#[test]
fn b01_named_ceremony_passthrough_preserves_consent_binding() {
    use rhtn_client::query::{VerificationQuery,consent_verifies};
    let ids=[test_identity("alice").public,test_identity("bob").public,test_identity("carol").public];
    let known=ids.iter().map(|i|i.key_material()).collect::<Vec<_>>();
    let alice=Participant::start(seeds("alice"),known.clone(),platform()).unwrap();
    let bob=Participant::start(seeds("bob"),known,platform()).unwrap();
    alice.begin(ids[1].keyhash.to_vec(),vec![ids[2].keyhash.to_vec()],true).unwrap();
    let theirs=bob.begin(ids[0].keyhash.to_vec(),vec![ids[2].keyhash.to_vec()],false).unwrap();
    let cid=alice.take_intent(ids[1].keyhash.to_vec(),theirs).unwrap();
    let query=VerificationQuery {subject:ids[0].keyhash,querier:ids[1].keyhash,ceremony_id:cid.try_into().unwrap(),profile:vec![1],template_version:1,verifier:ids[2].keyhash};
    let out=alice.consent(query.encode()).unwrap();
    let exported=out.as_ref().is_some_and(|b|consent_verifies(&ids[0],b,&query.query_id()));
    // The former blanket ban is superseded by D 14.1.0 / L 9 named opaque pass-through.
    // This control verifies binding only; it does not certify a shipping shell never interprets or routes routine wire data.
    assert!(exported,"the named ceremony exchange must preserve the consent/query binding");
}
