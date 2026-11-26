//! Retained and current assertions for implemented components.
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_node::http;
use std::sync::Arc;

fn fresh(out:&mut [u8]) { out.copy_from_slice(&vec![19;out.len()]); }

#[test]
fn r01_http_header_values_must_not_hide_trusted_headers() {
    let msg=b"GET / HTTP/1.1\r\nHost: backend\r\nX-Note: safe\nrhtn-roles: admin\r\n\r\n";
    assert!(http::parse(msg).is_err(),"wire §11.2: reject embedded line endings rather than forward caller rhtn-* bytes inside an unchecked value");
}

#[test]
fn r01_oversized_chunk_length_must_return_an_error_without_panicking() {
    let msg=format!("POST / HTTP/1.1\r\nHost: backend\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n",usize::MAX);
    let result=std::panic::catch_unwind(||http::parse(msg.as_bytes()));
    assert!(matches!(result,Ok(Err(_))),"wire §11: malformed HTTP must produce a refusal, not panic in the gateway");
}

#[test]
fn r02_a_sealed_message_must_match_the_named_senders_bound_identity() {
    // 2026-11-20, slice 2: the seal replaced the session this finding was
    // filed against; the claim stands, and the tag is what carries it
    use rhtn_client::payload::{Sender,NodeKeyHeld,seal_to,open_relayed,tag_verifies};
    use rhtn_crypto::{seal::EncryptionSecret,verify::EncryptionKey};
    let (sender,victim,recipient,node)=(test_identity("carol"),test_identity("alice"),test_identity("bob"),test_identity("w1"));
    let secret=|n:&str|EncryptionSecret::from_identity_root(&rhtn_codec::cose::sha256(n.as_bytes()));
    let key_of=|kh:[u8;32],s:&EncryptionSecret|EncryptionKey{keyhash:kh,kem:*s.kem_public(),auth:s.auth_public()};
    let (carol,alice,bob)=(secret("carol"),secret("alice"),secret("bob"));
    let node_secret=rhtn_crypto::xwing::Secret::from_seed([21;32]);
    let held=NodeKeyHeld{node:node.public.keyhash,n:1,public:Box::new(*node_secret.public())};
    // Carol seals naming Alice as its sender, under Carol's own secret
    let forged=seal_to(&Sender{keyhash:&victim.public.keyhash,secret:&carol,alias:"Alice",card:None},&recipient.public.keyhash,&key_of(recipient.public.keyhash,&bob),&held,0,b"message from Carol",&mut fresh).unwrap();
    let honest=seal_to(&Sender{keyhash:&sender.public.keyhash,secret:&carol,alias:"Carol",card:None},&recipient.public.keyhash,&key_of(recipient.public.keyhash,&bob),&held,0,b"message from Carol",&mut fresh).unwrap();
    let open=|sealed:&[u8]|{let s=rhtn_archive::sealed::Sealed::decode(sealed).unwrap();let half=node_secret.decapsulate(&s.ct_n);open_relayed(&bob,&recipient.public.keyhash,&node.public.keyhash,&rhtn_archive::submission::unrelayed(&rhtn_archive::submission::relayed(1,sealed,&half)).unwrap()).unwrap()};
    let (s,b)=open(&honest);
    assert!(tag_verifies(&bob,&recipient.public.keyhash,&key_of(sender.public.keyhash,&carol),&s,&b),"control: Carol's own message is attributed to Carol");
    let (s,b)=open(&forged);
    assert_eq!(b.sender,victim.public.keyhash,"control: the body names Alice");
    assert!(!tag_verifies(&bob,&recipient.public.keyhash,&key_of(victim.public.keyhash,&alice),&s,&b),"design §14.2: a relay must not turn Carol's message into one authenticated as Alice");
}

#[tokio::test]
async fn r04_direct_open_must_honor_the_local_privacy_gate() {
    use rhtn_adaptors::direct::{Direct,LightDirect};
    use rhtn_transport::{tls::Pins,traversal::{Candidate,CandidateKind}};
    let (a,b)=(Arc::new(test_identity("alice")),Arc::new(test_identity("bob")));
    let pins=Pins::new();pins.pin_identity(&a.public);pins.pin_identity(&b.public);
    let a_socket=LightDirect::bind(rhtn_transport::tls::Party::of(a.clone()),pins.clone(),Default::default(),rhtn_adaptors::direct::Reachable::default(),"127.0.0.1:0".parse().unwrap(),None,None,Arc::new(|_|false),Arc::new(|_,_|{}),rhtn_adaptors::direct::Control::none()).unwrap();
    let b_socket=LightDirect::bind(rhtn_transport::tls::Party::of(b.clone()),pins,Default::default(),rhtn_adaptors::direct::Reachable::default(),"127.0.0.1:0".parse().unwrap(),None,None,Arc::new(|_|true),Arc::new(|_,_|{}),rhtn_adaptors::direct::Control::none()).unwrap();
    assert!(a_socket.gather(b.public.keyhash).await.is_none(),"control: policy prohibits direct contact");
    let candidate=Candidate{kind:CandidateKind::Host,addr:b_socket.addr().unwrap()};
    assert!(!a_socket.open(b.public.keyhash,vec![candidate]).await,"design §12.6.3: receiving candidates cannot bypass the local direct-path policy");
}

#[tokio::test]
async fn r05_malformed_resource_body_must_receive_status_three() {
    use rhtn_node::{runtime::LiveNode,view::NodeView,resolution::{AnchorTable,Ingestion}};
    use rhtn_archive::{tx::{Locator,Seqno},catalog::ResourceResponse};
    use rhtn_transport::{session::*,tls::{self,Pins}};
    use tokio::time::{timeout,Duration};
    timeout(Duration::from_secs(5),async {
        let (a,b)=(Arc::new(test_identity("alice")),Arc::new(test_identity("bob")));
        let pins=Pins::new();pins.pin_identity(&a.public);pins.pin_identity(&b.public);
        let view=NodeView::new(b.clone(),Locator::root(b.public.keyhash,Seqno{series:1,counter:0}));
        let live=LiveNode::start(NodeConfig::defaults(b.clone(),pins.clone(),30),view,vec![a.public.clone(),b.public.clone()],AnchorTable::new(0,Ingestion::UnverifiedGossip));
        let cfg=ClientConfig{me:rhtn_transport::tls::Party::of(a),pins,bind:Default::default(),capabilities:Default::default(),attestation:None,filter:None,sibling_cache:Default::default(),addresses:Default::default(),cursors:Default::default(),tls:Default::default(),connect_timeout:Duration::from_secs(2),on_reachability:None,log:Log::default()};
        let ep=tls::client_endpoint("127.0.0.1:0".parse().unwrap()).unwrap();
        let AttachOutcome::Attached(session)=attach(&cfg,&ep,b.public.keyhash,live.addr,false).await else {panic!("attach")};
        let reply=session.request(6,&[0xa0]).await; // valid outer framing, empty body map
        session.conn.close(0u32.into(),b"done");
        assert!(reply.as_ref().ok().and_then(|r|ResourceResponse::decode(r).ok()).is_some_and(|r|r.status==3),"wire §11 step 0 requires status 3 for a decodable frame with malformed ResourceRequest; got {reply:?}");
    }).await.unwrap();
}

#[test]
fn r08_reserved_node_roles_must_not_reach_the_backend() {
    use rhtn_node::resources::*;
    use rhtn_archive::{catalog::ResourceRequest,topology::Table};
    use std::collections::BTreeSet;
    struct Echo;
    impl Backend for Echo {
        fn running(&self)->bool { true }
        fn handle(&self,b:&[u8])->Result<Vec<u8>,String> { Ok(b.to_vec()) }
    }
    let owner=test_identity("alice").public.keyhash;
    let resource=[9;32];
    let manifest=Manifest{roles:BTreeSet::from(["connect".into(),"discover".into(),"read".into()]),imports:vec![],admin:vec![],templates:vec![],..Default::default()};
    // Rejecting the reserved declarations at install time is also conforming.
    let Ok(package)=instantiate(&manifest) else { return };
    let mut gateway=Gateway::default();
    gateway.bind(resource,Binding{owner,authority:"backend".into(),backend:Some(Arc::new(Echo)),declared:manifest.clone()});
    // Refusing the reserved role assignment before serving is also conforming.
    if gateway.set_row(resource,owner,Row{connect:true,roles:package.roles}).is_err() { return }
    let request=ResourceRequest{resource,message:b"GET / HTTP/1.1\r\nHost: backend\r\n\r\n".to_vec()}.encode();
    let reply=gateway.serve(&owner,&Table::with_me(owner),&owner,&request,0);
    assert_eq!(reply.status,0,"control: admitted owner reaches the backend");
    let headers=http::headers_of(&reply.body.unwrap());
    let roles=&headers.iter().find(|(n,_)|n=="rhtn-roles").unwrap().1;
    assert!(!roles.split(',').any(|r|matches!(r.trim(),"connect"|"discover")),"resource §3: reserved node roles must never appear in the application credential; got {roles}");
}

#[tokio::test]
async fn r04_node_direct_open_must_honor_the_horizon_gate() {
    use rhtn_adaptors::direct::{Direct,NodeDirect};
    use rhtn_node::{runtime::LiveNode,view::NodeView,resolution::{AnchorTable,Ingestion}};
    use rhtn_archive::tx::{Locator,Seqno};
    use rhtn_transport::{session::NodeConfig,tls::Pins,traversal::{Candidate,CandidateKind}};
    let (a,b)=(Arc::new(test_identity("alice")),Arc::new(test_identity("bob")));
    let pins=Pins::new();pins.pin_identity(&a.public);pins.pin_identity(&b.public);
    let ids=vec![a.public.clone(),b.public.clone()];
    let start=|me:Arc<rhtn_crypto::SigningIdentity>|LiveNode::start(NodeConfig::defaults(me.clone(),pins.clone(),30),NodeView::new(me.clone(),Locator::root(me.public.keyhash,Seqno{series:1,counter:0})),ids.clone(),AnchorTable::new(0,Ingestion::UnverifiedGossip));
    let left=start(a);let right=start(b.clone());
    let direct=NodeDirect::new(left,pins,Arc::new(|_,_|{}));
    assert!(direct.gather(b.public.keyhash).await.is_none(),"control: unrelated roots are outside each other's horizon");
    assert!(!direct.open(b.public.keyhash,vec![Candidate{kind:CandidateKind::Host,addr:right.addr}]).await,"design §12.6.3: NodeDirect must enforce the same local boundary as LightDirect");
}
