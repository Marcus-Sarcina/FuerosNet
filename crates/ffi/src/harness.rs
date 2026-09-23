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
        for (slot, c) in clients.iter().enumerate() {
            let id = rhtn_crypto::identity::testkit::test_identity(c);
            pins.pin_identity(&id.public);
            view.set_slot(slot as u64, Some(id.public.keyhash), 1_800_000_000);
            known.push(id.public);
        }
        let mut cfg = NodeConfig::defaults(me, pins, 30);
        cfg.log = Log::default();
        let node = LiveNode::start(
            cfg,
            view,
            known,
            AnchorTable::new(0, Ingestion::UnverifiedGossip),
        );
        Arc::new(TestNode { node, _rt: rt })
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
