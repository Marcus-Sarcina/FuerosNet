//! Scripted multi-node scenarios over loopback.

use rhtn_crypto::SigningIdentity;
use rhtn_transport::session::{Node, NodeConfig};
use rhtn_transport::tls;
use rhtn_transport::traversal::{self, TraversalSocket};
use std::net::SocketAddr;
use std::sync::Arc;

/// One serving node, running.
pub struct Running {
    pub node: Arc<Node>,
    pub addr: SocketAddr,
    pub endpoint: quinn::Endpoint,
    /// The socket it serves on: QUIC, and STUN for the clients it serves
    /// (design §14.1.1).
    pub traversal: Arc<TraversalSocket>,
}

impl Running {
    /// Start a node on loopback, answering STUN at the address it serves
    /// QUIC on.
    pub fn start(cfg: NodeConfig) -> Running {
        let socket = TraversalSocket::bind("127.0.0.1:0".parse().unwrap(), cfg.nat).unwrap();
        let addr = socket.addr().unwrap();
        let crypto =
            quinn::crypto::rustls::QuicServerConfig::try_from(tls::server_config(&cfg.identity))
                .expect("quinn accepts the profile");
        let mut qcfg = quinn::ServerConfig::with_crypto(Arc::new(crypto));
        qcfg.transport_config(Arc::new(tls::transport_config()));
        let endpoint = traversal::endpoint(socket.clone(), Some(qcfg)).unwrap();
        let node = Node::new(cfg);
        tokio::spawn(node.clone().serve(endpoint.clone()));
        Running {
            node,
            addr,
            endpoint,
            traversal: socket,
        }
    }

    /// Stop answering, as a node going dark does.
    pub fn go_dark(&self) {
        self.endpoint.close(0u32.into(), b"dark");
    }

    pub fn identity(&self) -> Arc<SigningIdentity> {
        self.node.cfg.identity.clone()
    }
}
