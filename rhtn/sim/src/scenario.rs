//! Scripted multi-node scenarios over loopback.

use rhtn_transport::session::{Node, NodeConfig};
use rhtn_transport::tls;
use rhtn_crypto::SigningIdentity;
use std::net::SocketAddr;
use std::sync::Arc;

/// One serving node, running.
pub struct Running {
    pub node: Arc<Node>,
    pub addr: SocketAddr,
    pub endpoint: quinn::Endpoint,
}

impl Running {
    /// Start a node on loopback.
    pub fn start(cfg: NodeConfig) -> Running {
        let endpoint = tls::server_endpoint(&cfg.identity, "127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = endpoint.local_addr().unwrap();
        let node = Node::new(cfg);
        tokio::spawn(node.clone().serve(endpoint.clone()));
        Running { node, addr, endpoint }
    }

    /// Stop answering, as a node going dark does.
    pub fn go_dark(&self) {
        self.endpoint.close(0u32.into(), b"dark");
    }

    pub fn identity(&self) -> Arc<SigningIdentity> {
        self.node.cfg.identity.clone()
    }
}
