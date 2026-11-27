//! Persistence boundaries of the newly implemented daemon lifecycle.
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_codec::cose;
use rhtn_archive::{tx::*, node_key::*};
use rhtn_daemon::{config::Config, service::Service};
use rhtn_node::{resolution::Ingestion, store::Decision};
use std::path::PathBuf;

/// The subnet the fixtures' positions are in (`wire-format.md` §2.3 field 4).
const SUBNET: [u8; 32] = [0xdb; 32];

struct Layout { dir: PathBuf, cfg: Config, peers: PathBuf }
impl Drop for Layout { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.dir); } }
fn layout(tag: &str) -> Layout {
    let dir=std::env::temp_dir().join(format!("rhtn-review-{tag}-{}",std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let identity=dir.join("identity");
    let mut seeds=cose::sha256(b"rhtn-test-vectors:bob:ed25519-seed").to_vec();
    seeds.extend(cose::sha256(b"rhtn-test-vectors:bob:ml-dsa-65-seed"));
    std::fs::write(&identity,seeds).unwrap();
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; std::fs::set_permissions(&identity,std::fs::Permissions::from_mode(0o600)).unwrap(); }
    let peers=dir.join("peers");
    let text=["alice","bob","carol"].iter().map(|n|test_identity(n).public.key_material().iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>().join("\n");
    std::fs::write(&peers,text).unwrap();
    let cfg=Config { log:None, resources:None, grants:None, horizon:None, state:None, resource_limits:Default::default(), identity:Some(identity), operator:None, transport_key:None, delegations:None, endpoint_record:None, anchor_entry:None, anchors:None, acknowledge:false, administration:None, federation:None, listen:"127.0.0.1:0".parse().unwrap(), upstream:None, queue:dir.join("queue"), archive:dir.join("archive"), queue_cap:None, delivered_retention_secs:rhtn_transport::session::RETENTION_SECS, node_keys:Default::default(), board_queue:rhtn_node::board::QUEUE_BYTES, heartbeat_secs:30, reconcile_secs:900, ingestion:Ingestion::UnverifiedGossip, request_allowance:(120,60), keys:dir.join("keys"), topology:dir.join("topology") };
    Layout { dir,cfg,peers }
}

#[tokio::test]
async fn d01_daemon_restart_must_restore_the_accepted_child_slot() {
    let l=layout("topology");
    let (n,p,old)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let bn=[cose::sha256(&n.public.keyhash)]; let bp=[cose::sha256(&p.public.keyhash)];
    let raw=envelope(TYPE_ADOPTION,&adoption_body(&Adoption { node:n.public.keyhash,patron:p.public.keyhash,locator:Locator { anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno {series:1,counter:0}, subnet: Some(SUBNET) },timestamp:100,key_material:None,evidence:Evidence::Transfer { former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash) },presented_head:None,back:[&bn,&bp], carrying: None }),&[&n,&p]);
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    assert_eq!(service.node.originate_transaction(&raw),Decision::Stored);
    assert_eq!(service.node.view.lock().unwrap().slot_of(&n.public.keyhash),Some(0),"control: the running daemon accepted the child");
    service.persist().unwrap();
    service.node.endpoint.close(0u32.into(),b"restart");
    drop(service);
    let restored=Service::start(&l.cfg,&l.peers).await.unwrap();
    assert_eq!(restored.node.originate_transaction(&raw),Decision::Duplicate,"control: saved seen-set recognizes the held adoption");
    let slot=restored.node.view.lock().unwrap().slot_of(&n.public.keyhash);
    restored.node.endpoint.close(0u32.into(),b"done");
    assert_eq!(slot,Some(0),"infra sections 4.1-4.3: restoring accepted topology must restore its current child slot, not just suppress its replay");
}

// 2026-11-20, slice 2: the one-time key this was filed against is gone;
// what a node now deletes is a node key's private half, and the claim is
// the same: a deletion must not come back from the last snapshot
#[tokio::test]
async fn d02_a_deleted_node_key_must_not_return_from_the_last_daemon_snapshot() {
    let l=layout("node-keys");
    let subject=test_identity("alice");
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    service.persist().unwrap();
    let (first,second)={
        let mut view=service.node.view.lock().unwrap();
        // the rotation and the grace as test parameters: every fetch rotates, and nothing is held past it
        view.node_keys.cfg.rotate_secs=0;view.node_keys.cfg.grace_secs=0;
        let signer=view.ack_signer().unwrap();let now=view.now();
        let first=NodeKey::parse(&view.node_keys.newest(&*signer,&subject.public.keyhash,now)).unwrap().fields.n;
        let second=NodeKey::parse(&view.node_keys.newest(&*signer,&subject.public.keyhash,now)).unwrap().fields.n;
        (first,second)
    };
    assert_ne!(first,second,"control: rotated");
    let (_,pruned)=service.maintain();
    assert_eq!(pruned,1,"control: the retired key, sealed to by nothing, is deleted in memory");
    assert_eq!(service.node.view.lock().unwrap().node_keys.held(&subject.public.keyhash),vec![second]);
    // Model loss of memory before the next periodic/clean-shutdown save.
    // This invokes the real startup path against exactly the persisted bytes;
    // it does not claim to simulate a torn filesystem write or power failure.
    service.node.endpoint.close(0u32.into(),b"abrupt stop");
    drop(service);
    let restored=Service::start(&l.cfg,&l.peers).await.unwrap();
    let held=restored.node.view.lock().unwrap().node_keys.held(&subject.public.keyhash);
    restored.node.endpoint.close(0u32.into(),b"done");
    assert_eq!(held,vec![second],"wire section 7.8: a node key deleted with the last message sealed to it must remain deleted after restart");
}

#[tokio::test]
async fn d03_daemon_must_verify_its_own_signature_without_a_self_peer_entry() {
    let l=layout("self-key");
    let text=["alice","carol"].iter().map(|n|test_identity(n).public.key_material().iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>().join("\n");
    std::fs::write(&l.peers,text).unwrap();
    let (n,p,old)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let bn=[cose::sha256(&n.public.keyhash)]; let bp=[cose::sha256(&p.public.keyhash)];
    let raw=envelope(TYPE_ADOPTION,&adoption_body(&Adoption { node:n.public.keyhash,patron:p.public.keyhash,locator:Locator { anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno {series:1,counter:0}, subnet: Some(SUBNET) },timestamp:100,key_material:None,evidence:Evidence::Transfer { former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash) },presented_head:None,back:[&bn,&bp], carrying: None }),&[&n,&p]);
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    let decision=service.node.originate_transaction(&raw);
    service.node.endpoint.close(0u32.into(),b"done");
    if let Decision::Held(pending)=&decision { assert_eq!(pending.missing_key,Some(p.public.keyhash),"control: the missing key is the daemon itself"); }
    assert!(matches!(decision,Decision::Stored),"wire section 3.4: this node already has its own public key; a valid own transaction must not await it from a peer");
}

#[tokio::test]
async fn d01_restart_must_restore_own_archive_back_pointers() {
    let l=layout("own-archive");
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    let child=test_identity("alice").public.keyhash;
    let rec=service.node.view.lock().unwrap().disavow(&child,None).unwrap();
    assert_eq!(service.node.originate_transaction(&rec.bytes),Decision::Stored);
    assert_eq!(service.node.view.lock().unwrap().archive.next_back_pointers(),vec![rec.txid]);
    service.persist().unwrap();
    service.node.endpoint.close(0u32.into(),b"restart");
    drop(service);
    let restored=Service::start(&l.cfg,&l.peers).await.unwrap();
    assert_eq!(restored.node.originate_transaction(&rec.bytes),Decision::Duplicate,"control: the signed own record survived");
    let back=restored.node.view.lock().unwrap().archive.next_back_pointers();
    restored.node.endpoint.close(0u32.into(),b"done");
    assert_eq!(back,vec![rec.txid],"wire section 3.1: the next own record must extend the retained archive instead of restarting at genesis");
}

#[tokio::test]
async fn d01_restart_must_restore_the_reissued_own_locator_series() {
    let l=layout("own-series");
    let (n,p,old)=(test_identity("bob"),test_identity("alice"),test_identity("carol"));
    let bn=[cose::sha256(&n.public.keyhash)];let bp=[cose::sha256(&p.public.keyhash)];
    let a=envelope(TYPE_ADOPTION,&adoption_body(&Adoption {node:n.public.keyhash,patron:p.public.keyhash,locator:Locator {anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno {series:1,counter:0}, subnet: Some(SUBNET) },timestamp:100,key_material:None,evidence:Evidence::Transfer {former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash)},presented_head:None,back:[&bn,&bp], carrying: None }),&[&n,&p]);
    let aid=rhtn_archive::record::Record::parse(&a).unwrap().txid;
    let backs=[aid];
    let reissued=envelope(TYPE_REISSUE,&reissue_body([&backs,&backs],&n.public.keyhash,&p.public.keyhash,Seqno {series:1,counter:0},2,101),&[&n,&p]);
    let ids=vec![n.public.clone(),p.public.clone(),old.public.clone()];
    assert!(rhtn_archive::series::SeriesChain::from_records(&[a.clone(),reissued.clone()],&ids).is_ok(),"control: a valid reissue chain");
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    for raw in [&a,&reissued] { assert_eq!(service.node.originate_transaction(raw),Decision::Stored); }
    service.persist().unwrap();service.node.endpoint.close(0u32.into(),b"restart");drop(service);
    let restored=Service::start(&l.cfg,&l.peers).await.unwrap();
    assert_eq!(restored.node.originate_transaction(&reissued),Decision::Duplicate,"control: the reissue was retained");
    let position=restored.node.view.lock().unwrap().position.clone();
    restored.node.endpoint.close(0u32.into(),b"done");
    assert_eq!(position.anchor,p.public.keyhash,"control: adoption position restored");
    assert_eq!(position.seqno.series,2,"wire sections 2.3 and 4.6.1: a stored countersigned reissue replaces the old locator series");
}

// 2026-11-20, slice 2: one-time issuance is gone; serving a node key is
// what a node now answers, and it must not stop the daemon persisting
#[tokio::test]
async fn s05_daemon_persist_must_succeed_after_serving_node_keys() {
    let l=layout("persist-served");let subject=test_identity("alice");
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    {
        let mut v=service.node.view.lock().unwrap();
        let signer=v.ack_signer().unwrap();let now=v.now();
        let req=NodeKeyRequest::One {recipient:subject.public.keyhash,nonce:[1;16]}.encode();
        let reply=NodeKeyReply::decode(&v.node_keys.answer(&*signer,&|_|true,&req,now).unwrap()).unwrap();
        assert!(reply.outcome.is_ok(),"control: a node key was served");
    }
    let saved=service.persist();service.node.endpoint.close(0u32.into(),b"done");
    assert!(saved.is_ok(),"infra section 4.3: routine node-key service must not prevent daemon state persistence; got {saved:?}");
}
