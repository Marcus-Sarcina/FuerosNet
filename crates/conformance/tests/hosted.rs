//! Review of request isolation in the implemented local verifier adaptor.
use rhtn_adaptors::{actor::Handle,courier::Courier,direct::{NoDirect,Reachable},serving::{Serving,Answer},verifier::Verifiers};
use rhtn_archive::Keyhash;
use rhtn_client::{ceremony::{Client,Config},device::*,notice::Silent,query::*};
use rhtn_crypto::identity::testkit::test_identity;
use std::{rc::Rc,sync::Arc,time::Duration};

fn boxed_identity(name: &str)->Box<rhtn_crypto::SigningIdentity> { Box::new(test_identity(name)) }

struct DeviceStub;
impl Proximity for DeviceStub {
    fn supported(&self)->Vec<ChannelKind> { vec![] }
    fn run(&self,kind:ChannelKind,_:&Keyhash)->ChannelOutcome { ChannelOutcome{kind,result:ChannelResult::Unavailable,resolution_m:None} }
}
impl Camera for DeviceStub {
    fn capture(&self,_:Prompt)->RawFrame { RawFrame{pixels:vec![],metadata:Default::default()} }
}
impl Clock for DeviceStub {
    fn now_ms(&self)->u64 { 0 }
    fn wait_ms(&self,_:u64) {}
}
impl Random for DeviceStub { fn fill(&self,b:&mut[u8]) { b.fill(17); } }
impl Operator for DeviceStub { fn ask(&self,_:&str)->bool { true } }
struct ServingStub;
impl Serving for ServingStub {
    fn node_device(&self) -> [u8; 32] {
        [0; 32]
    }
    fn propagate<'a>(&'a self,_:Vec<u8>)->Answer<'a,bool> { Box::pin(async {false}) }
    fn me(&self)->Keyhash { [1;32] }
    fn holds(&self,_:&Keyhash)->bool { false }
    fn serves(&self,_:&Keyhash)->bool { false }
    fn publish<'a>(&'a self,_:&'a [u8])->Answer<'a,bool> { Box::pin(async {false}) }
    fn stock<'a>(&'a self,_:Keyhash,_:Vec<Vec<u8>>)->Answer<'a,bool> { Box::pin(async {false}) }
    fn prekey<'a>(&'a self,_:Keyhash,_:&'a [u8])->Answer<'a,Option<Vec<u8>>> { Box::pin(async {None}) }
    fn relay<'a>(&'a self,_:Keyhash,_:Keyhash,_:Vec<u8>,_:[u8;32])->Answer<'a,bool> { Box::pin(async {false}) }
    fn wake<'a>(&'a self,_:Keyhash,_:Option<rhtn_archive::submission::WakeEndpoint>)->Answer<'a,bool> { Box::pin(async {false}) }
}

#[tokio::test]
async fn r10_wrong_requester_must_not_cancel_another_requesters_pending_query() {
    tokio::time::timeout(Duration::from_secs(3),async {
        let subject=boxed_identity("alice");
        let verifier_key=test_identity("bob").public.keyhash;
        let requester=test_identity("carol").public;
        let stranger=test_identity("w1").public;
        let ids=vec![subject.public.clone(),test_identity("bob").public,requester.clone(),stranger.clone()];
        let handle=Handle::spawn(move || {
            let mut cfg=Config::default();cfg.verifier.grant_buffer_ms=5000;
            let device=Device{proximity:Rc::new(DeviceStub),camera:Rc::new(DeviceStub),clock:Rc::new(DeviceStub),random:Rc::new(DeviceStub),operator:Rc::new(DeviceStub),notifier:Rc::new(Silent),engine:Rc::new(HashEngine::new(16)),direct:Rc::new(Reachable::default())};
            Client::new(test_identity("bob"),ids,cfg,device)
        }).unwrap();
        let (courier,_app)=Courier::new(handle.clone(),Arc::new(ServingStub),Arc::new(NoDirect(Reachable::default())));
        let service=Verifiers::new();service.host(handle.clone(),courier);
        let q=VerificationQuery{subject:subject.public.keyhash,querier:requester.keyhash,ceremony_id:[9;32],profile:vec![1],template_version:1,verifier:verifier_key};
        let qid=q.query_id();
        let body=QueryRequest{query:q,consent:consent(&subject,&qid),selection_basis:1}.encode();
        let original=tokio::spawn(service.clone().answer(requester.keyhash,body.clone()));
        while handle.with(|c|c.verifier.awaiting()).await!=vec![qid] {
            tokio::task::yield_now().await;
        }
        // The same signed query on another authenticated identity's stream is invalid.
        assert!(service.clone().answer(stranger.keyhash,body).await.is_none());
        // Its rejection must leave the legitimate request waiting for its grant.
        let mut original=original;
        let premature=tokio::time::timeout(Duration::from_millis(100),&mut original).await;
        original.abort();
        assert!(premature.is_err(),"wire §5.6: wrong-peer rejection must not replace the legitimate query's reply channel; got {premature:?}");
    }).await.unwrap();
}
