//! Persistence boundaries of the newly implemented daemon lifecycle.
use rhtn_crypto::identity::testkit::test_identity;
use rhtn_codec::cose;
use rhtn_archive::{tx::*, prekey::*};
use rhtn_daemon::{config::Config, service::Service};
use rhtn_node::{resolution::Ingestion, store::Decision};
use std::path::PathBuf;

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
    let cfg=Config { resources:None, resource_limits:Default::default(), identity, listen:"127.0.0.1:0".parse().unwrap(), upstream:None, queue:dir.join("queue"), archive:dir.join("archive"), queue_cap:None, heartbeat_secs:30, reconcile_secs:900, ingestion:Ingestion::UnverifiedGossip, request_allowance:(120,60), prekeys:dir.join("prekeys"), topology:dir.join("topology") };
    Layout { dir,cfg,peers }
}

#[tokio::test]
async fn d01_daemon_restart_must_restore_the_accepted_child_slot() {
    let l=layout("topology");
    let (n,p,old)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let bn=[cose::sha256(&n.public.keyhash)]; let bp=[cose::sha256(&p.public.keyhash)];
    let raw=envelope(TYPE_ADOPTION,&adoption_body(&Adoption { node:n.public.keyhash,patron:p.public.keyhash,locator:Locator { anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno {series:1,counter:0} },timestamp:100,key_material:None,evidence:Evidence::Transfer { former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash) },presented_head:None,back:[&bn,&bp] }),&[&n,&p]);
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

#[tokio::test]
async fn d02_served_one_time_key_must_not_return_from_the_last_daemon_snapshot() {
    let l=layout("prekeys");
    let (subject,requester)=(test_identity("alice"),test_identity("carol"));
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    let bundle=PrekeyBundle::build(&subject,1,b"opaque subject material",100);
    let otk=b"one-time opaque key".to_vec();
    {
        let mut view=service.node.view.lock().unwrap();
        view.prekeys.publish(&vec![subject.public.clone()],&bundle).unwrap();
        assert!(view.prekeys.stock(subject.public.keyhash,vec![otk.clone()]));
    }
    service.persist().unwrap();
    let request=PrekeyRequest::One { subject:subject.public.keyhash,one_time:true,nonce:[7;16] }.encode();
    {
        let mut view=service.node.view.lock().unwrap();
        let reply=PrekeyReply::decode(&view.prekeys.answer(&requester.public.keyhash,&request,101).unwrap()).unwrap();
        assert_eq!(reply.one_time,Some(otk.clone()),"control: key served successfully");
        assert_eq!(view.prekeys.pool_size(&subject.public.keyhash),0,"control: consumed in memory");
    }
    // Model loss of memory before the next periodic/clean-shutdown save.
    // This invokes the real startup path against exactly the persisted bytes;
    // it does not claim to simulate a torn filesystem write or power failure.
    service.node.endpoint.close(0u32.into(),b"abrupt stop");
    drop(service);
    let restored=Service::start(&l.cfg,&l.peers).await.unwrap();
    let reply=PrekeyReply::decode(&restored.node.view.lock().unwrap().prekeys.answer(&requester.public.keyhash,&request,102).unwrap()).unwrap();
    restored.node.endpoint.close(0u32.into(),b"done");
    assert_eq!(reply.one_time,None,"wire section 7.8: a successfully served one-time key must remain consumed after restart");
}

#[tokio::test]
async fn d03_daemon_must_verify_its_own_signature_without_a_self_peer_entry() {
    let l=layout("self-key");
    let text=["alice","carol"].iter().map(|n|test_identity(n).public.key_material().iter().map(|b|format!("{b:02x}")).collect::<String>()).collect::<Vec<_>>().join("\n");
    std::fs::write(&l.peers,text).unwrap();
    let (n,p,old)=(test_identity("alice"),test_identity("bob"),test_identity("carol"));
    let bn=[cose::sha256(&n.public.keyhash)]; let bp=[cose::sha256(&p.public.keyhash)];
    let raw=envelope(TYPE_ADOPTION,&adoption_body(&Adoption { node:n.public.keyhash,patron:p.public.keyhash,locator:Locator { anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno {series:1,counter:0} },timestamp:100,key_material:None,evidence:Evidence::Transfer { former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash) },presented_head:None,back:[&bn,&bp] }),&[&n,&p]);
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
    let a=envelope(TYPE_ADOPTION,&adoption_body(&Adoption {node:n.public.keyhash,patron:p.public.keyhash,locator:Locator {anchor:p.public.keyhash,path:vec![0],nibbles:1,seqno:Seqno {series:1,counter:0}},timestamp:100,key_material:None,evidence:Evidence::Transfer {former:old.public.keyhash,block:transfer_block(&old,&n.public.keyhash,&p.public.keyhash)},presented_head:None,back:[&bn,&bp]}),&[&n,&p]);
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

#[tokio::test]
async fn s05_daemon_persist_must_succeed_after_one_time_key_issuance() {
    let l=layout("persist-issued");let subject=test_identity("alice");let requester=test_identity("carol");
    let service=Service::start(&l.cfg,&l.peers).await.unwrap();
    {
        let mut v=service.node.view.lock().unwrap();
        v.prekeys.publish(&vec![subject.public.clone()],&PrekeyBundle::build(&subject,1,b"opaque",100)).unwrap();
        assert!(v.prekeys.stock(subject.public.keyhash,vec![vec![7]]));
        let req=PrekeyRequest::One {subject:subject.public.keyhash,one_time:true,nonce:[1;16]}.encode();
        assert_eq!(PrekeyReply::decode(&v.prekeys.answer(&requester.public.keyhash,&req,101).unwrap()).unwrap().one_time,Some(vec![7]));
    }
    let saved=service.persist();service.node.endpoint.close(0u32.into(),b"done");
    assert!(saved.is_ok(),"infra section 4.3: routine prekey issuance must not prevent daemon state persistence; got {saved:?}");
}
