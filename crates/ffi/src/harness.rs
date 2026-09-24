//! What a binding's round trip needs and a shell never does: a serving
//! node on loopback and the test identities, behind the `harness` feature.
//!
//! **Not the facade.** A shell builds without this feature and sees none
//! of it; it exists so a generated binding can be driven end to end from
//! its own language, against a real node, with no Rust test in the loop.

use crate::types::Id;
use rhtn_node::resolution::{AnchorTable, Ingestion, Path};
use rhtn_node::runtime::LiveNode;
use rhtn_node::view::NodeView;
use rhtn_transport::session::{Log, NodeConfig};
use rhtn_transport::tls::Pins;
use std::sync::Arc;

/// A test identity by name, as every workspace test derives it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TestIdentity {
    /// The 64 bytes of seed [`crate::client::Participant::start`] takes.
    pub seeds: Vec<u8>,
    /// The `KeyMaterial` array others pin.
    pub material: Vec<u8>,
    pub id: Id,
}

#[uniffi::export]
pub fn test_identity(name: String) -> TestIdentity {
    let sid = rhtn_crypto::identity::testkit::test_identity(&name);
    let mut seeds =
        rhtn_codec::cose::sha256(format!("rhtn-test-vectors:{name}:ed25519-seed").as_bytes())
            .to_vec();
    seeds.extend_from_slice(&rhtn_codec::cose::sha256(
        format!("rhtn-test-vectors:{name}:ml-dsa-65-seed").as_bytes(),
    ));
    TestIdentity {
        seeds,
        material: sid.public.key_material(),
        id: sid.public.keyhash.to_vec(),
    }
}

/// A serving node on loopback, run by a test identity, pinning the named
/// clients and holding them in its slots so their sessions are primary.
#[derive(uniffi::Object)]
pub struct TestNode {
    node: Arc<LiveNode>,
    _rt: tokio::runtime::Runtime,
}

#[uniffi::export]
impl TestNode {
    #[uniffi::constructor]
    pub fn start(name: String, clients: Vec<String>) -> Arc<TestNode> {
        Self::started(name, clients, vec![], None)
    }

    /// `start`, serving where it is told and admitting identities minted
    /// elsewhere.
    ///
    /// `admitted` carries the **public** key material of clients this node
    /// has no seeds for, which is what lets a device that minted its own
    /// identity attach without anybody moving a seed to reach it. The
    /// listen address is given because a node on loopback is unreachable
    /// from an emulator or a second machine.
    #[uniffi::constructor]
    pub fn start_on(
        name: String,
        clients: Vec<String>,
        admitted: Vec<Vec<u8>>,
        listen: String,
    ) -> Arc<TestNode> {
        Self::started(name, clients, admitted, listen.parse().ok())
    }

    /// Where it serves.
    pub fn address(&self) -> String {
        self.node.addr.to_string()
    }

    /// What waits for `client` in its mailbox.
    pub fn queued(&self, client: Id) -> u64 {
        crate::types::keyhash(&client).map_or(0, |k| self.node.node.queued(&k) as u64)
    }
}

impl TestNode {
    fn started(
        name: String,
        clients: Vec<String>,
        admitted: Vec<Vec<u8>>,
        listen: Option<std::net::SocketAddr>,
    ) -> Arc<TestNode> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        let _guard = rt.enter();
        let me = Arc::new(rhtn_crypto::identity::testkit::test_identity(&name));
        let pins = Pins::new();
        let mut known = vec![me.public.clone()];
        pins.pin_identity(&me.public);
        let p = Path::from_indices(&[]);
        let mut view = NodeView::new(
            me.clone(),
            rhtn_archive::tx::Locator {
                anchor: me.public.keyhash,
                path: p.bytes,
                nibbles: p.nibbles,
                seqno: rhtn_archive::tx::Seqno {
                    series: 1,
                    counter: 0,
                },
            },
        );
        let mut slot = 0u64;
        for c in clients.iter() {
            let id = rhtn_crypto::identity::testkit::test_identity(c);
            pins.pin_identity(&id.public);
            view.set_slot(slot, Some(id.public.keyhash), 1_800_000_000);
            known.push(id.public);
            slot += 1;
        }
        // an identity this node holds no seeds for: pinned and slotted from
        // its public half alone, exactly as the named ones are
        for km in admitted.iter() {
            let Some(id) = rhtn_crypto::Identity::from_key_material(km) else {
                continue;
            };
            pins.pin_identity(&id);
            view.set_slot(slot, Some(id.keyhash), 1_800_000_000);
            known.push(id);
            slot += 1;
        }
        let mut cfg = NodeConfig::defaults(me, pins, 30);
        cfg.log = Log::default();
        cfg.listen = listen;
        let node = LiveNode::start(
            cfg,
            view,
            known,
            AnchorTable::new(0, Ingestion::UnverifiedGossip),
        );
        Arc::new(TestNode { node, _rt: rt })
    }
}
