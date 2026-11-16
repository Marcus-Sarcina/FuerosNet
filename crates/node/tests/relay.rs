//! **The socket relay presents the instance's secret** (`wire-format.md`
//! §11.2; `resource-requirements.md` §3.1; GAT-042): inserted among the
//! node's own headers on the wire, once, and absent where none is held.

use rhtn_node::relay::Relay;
use rhtn_node::resources::Backend;
use std::io::{Read, Write};

// functional: GAT-042
#[test]
fn the_secret_goes_on_the_wire_among_the_nodes_headers_and_only_where_held() {
    let message = b"GET /walk HTTP/1.1\r\nhost: shop.internal\r\nrhtn-principal: p\r\nrhtn-roles: reader\r\n\r\n";
    let bare = Relay::to("127.0.0.1:1".parse().unwrap(), 1 << 20);
    assert_eq!(
        bare.on_the_wire(message),
        message.to_vec(),
        "no secret, nothing added"
    );
    let secret = [0xabu8; 32];
    let relay = Relay::to("127.0.0.1:1".parse().unwrap(), 1 << 20).presenting(secret);
    let wired = relay.on_the_wire(message);
    let text = String::from_utf8(wired.clone()).unwrap();
    let want = format!("rhtn-secret: {}\r\n", "ab".repeat(32));
    assert_eq!(text.matches("rhtn-secret:").count(), 1);
    assert!(
        text.ends_with(&format!("rhtn-roles: reader\r\n{want}\r\n")),
        "{text:?}"
    );
    assert_eq!(wired.len(), message.len() + want.len());

    // and over a socket, the package receives exactly that
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    let served = std::thread::spawn(move || {
        let (mut c, _) = l.accept().unwrap();
        let mut buf = [0u8; 4096];
        let n = c.read(&mut buf).unwrap();
        c.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok")
            .unwrap();
        buf[..n].to_vec()
    });
    let relay = Relay::to(addr, 1 << 20).presenting(secret);
    let reply = relay.handle(message).expect("answered");
    assert!(reply.ends_with(b"ok"));
    let got = served.join().unwrap();
    assert_eq!(
        got, wired,
        "what the socket received is the hop with the secret"
    );
}
