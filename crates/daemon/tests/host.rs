//! **The host surface a socket-backed instance reaches its storage over**
//! (`infra-client-requirements.md` §9.3, `resource-requirements.md` §3.3).
//!
//! Driven over the socket rather than against the decision function, so
//! what these prove is what an instance would actually meet: the request
//! as parsed, the secret as presented, the ceiling as the node answers it.

use rhtn_daemon::host::{HostSurface, hex_secret};
use rhtn_resources::{Block, LogFile};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A directory that goes when the test does.
struct Dir(std::path::PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let p = std::env::temp_dir().join(format!("rhtn-host-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("made");
        Dir(p)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One request, as an instance makes it: the code and the body back.
async fn ask(
    at: std::net::SocketAddr,
    method: &str,
    target: &str,
    secret: Option<&str>,
    body: &[u8],
) -> (u16, Vec<u8>) {
    let mut s = TcpStream::connect(at).await.expect("connected");
    let mut head = format!(
        "{method} {target} HTTP/1.1\r\ncontent-length: {}\r\n",
        body.len()
    );
    if let Some(sec) = secret {
        head.push_str(&format!("rhtn-secret: {sec}\r\n"));
    }
    head.push_str("\r\n");
    s.write_all(head.as_bytes()).await.expect("sent");
    s.write_all(body).await.expect("sent");
    s.flush().await.expect("flushed");
    let mut got = Vec::new();
    s.read_to_end(&mut got).await.expect("answered");
    let text = String::from_utf8_lossy(&got).to_string();
    let code = text
        .split(' ')
        .nth(1)
        .and_then(|c| c.trim().parse::<u16>().ok())
        .unwrap_or(0);
    let at = got
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(got.len(), |i| i + 4);
    (code, got[at..].to_vec())
}

fn kh(b: u8) -> [u8; 32] {
    [b; 32]
}

// acceptance: DMN-42
#[tokio::test]
async fn an_instance_keeps_its_block_over_the_surface_and_is_told_its_ceiling_on_a_write_past_it() {
    let d = Dir::new("round-trip");
    let surface = HostSurface::bind().await.expect("bound");
    let at = surface.bound();
    let block = Arc::new(Block::provision(d.0.join("blocks").join("one"), 64).expect("made"));
    let log = Arc::new(LogFile::at(d.0.join("one.log"), 512).expect("made"));
    let secret = hex_secret(&surface.register(kh(1), Some(block.clone()), Some(log.clone())));
    tokio::spawn(surface.clone().serve());

    // **a round trip**: written, sized, read back, listed, synced, removed
    let (code, body) = ask(at, "PUT", "/block/notes", Some(&secret), b"kept here").await;
    assert_eq!(code, 200, "{}", String::from_utf8_lossy(&body));
    assert_eq!(String::from_utf8_lossy(&body).trim(), "9");
    let (code, body) = ask(at, "GET", "/block/notes", Some(&secret), b"").await;
    assert_eq!(code, 200);
    assert_eq!(body, b"kept here");
    let (code, body) = ask(at, "GET", "/block/notes/size", Some(&secret), b"").await;
    assert_eq!(code, 200);
    assert_eq!(String::from_utf8_lossy(&body).trim(), "9");
    let (code, body) = ask(at, "GET", "/block", Some(&secret), b"").await;
    assert_eq!(code, 200);
    assert_eq!(String::from_utf8_lossy(&body).trim(), "notes");
    // `sync` returning is the whole of the durability discipline (§9.3)
    let (code, _) = ask(at, "POST", "/block/notes/sync", Some(&secret), b"").await;
    assert_eq!(code, 204);
    // an offset read, as a package reading part of a file does
    let (code, body) = ask(at, "GET", "/block/notes?offset=5&len=4", Some(&secret), b"").await;
    assert_eq!(code, 200);
    assert_eq!(body, b"here");

    // **a write past the ceiling is refused with the ceiling named**, and
    // nothing of it lands (§9.3): 64 bytes is the quota, 9 are held
    let (code, body) = ask(at, "PUT", "/block/big", Some(&secret), &[b'x'; 80]).await;
    let said = String::from_utf8_lossy(&body).to_string();
    assert_eq!(code, 413, "{said}");
    assert!(said.contains("64"), "the ceiling is named: {said}");
    // **absence is empty here, as it is through the binding**
    // (`Block::read` answers a name it does not hold with no bytes), and
    // one implementation behind both kinds of package is the point of
    // reaching this over a surface at all (§9.3)
    let (code, body) = ask(at, "GET", "/block/big", Some(&secret), b"").await;
    assert_eq!(code, 200);
    assert!(body.is_empty(), "nothing of the refused write landed");
    assert_eq!(
        block.list(),
        vec!["notes".to_string()],
        "and no file either"
    );

    // removed, and gone from the listing
    let (code, _) = ask(at, "DELETE", "/block/notes", Some(&secret), b"").await;
    assert_eq!(code, 204);
    let (_, body) = ask(at, "GET", "/block", Some(&secret), b"").await;
    assert_eq!(String::from_utf8_lossy(&body).trim(), "");
}

// acceptance: DMN-42
#[tokio::test]
async fn the_log_is_written_here_and_not_read_back_and_the_host_stamps_the_line() {
    let d = Dir::new("log");
    let surface = HostSurface::bind().await.expect("bound");
    let at = surface.bound();
    let log = Arc::new(LogFile::at(d.0.join("two.log"), 512).expect("made"));
    let secret = hex_secret(&surface.register(kh(2), None, Some(log.clone())));
    tokio::spawn(surface.clone().serve());

    let (code, _) = ask(at, "POST", "/log", Some(&secret), b"started\n").await;
    assert_eq!(code, 204);
    let lines = log.lines();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].ends_with("started"), "{:?}", lines[0]);
    assert!(
        lines[0].len() > "started".len(),
        "the host stamped it: {:?}",
        lines[0]
    );

    // **outward only** (§9.3): the package writes and the operator reads,
    // so there is nothing here for the package to read back
    let (code, body) = ask(at, "GET", "/log", Some(&secret), b"").await;
    assert_eq!(code, 405, "{}", String::from_utf8_lossy(&body));
    assert!(String::from_utf8_lossy(&body).contains("operator"));

    // and an instance with no block is told it has none rather than
    // reaching somebody's
    let (code, _) = ask(at, "GET", "/block", Some(&secret), b"").await;
    assert_eq!(code, 404);
}

// acceptance: DMN-43
#[tokio::test]
async fn the_secret_says_which_instance_is_calling_and_reaches_no_other_instance_s_block() {
    let d = Dir::new("secrets");
    let surface = HostSurface::bind().await.expect("bound");
    let at = surface.bound();
    let mine = Arc::new(Block::provision(d.0.join("blocks").join("mine"), 4096).expect("made"));
    let yours = Arc::new(Block::provision(d.0.join("blocks").join("yours"), 4096).expect("made"));
    let my_secret = hex_secret(&surface.register(kh(3), Some(mine.clone()), None));
    let your_secret = hex_secret(&surface.register(kh(4), Some(yours.clone()), None));
    tokio::spawn(surface.clone().serve());

    assert_eq!(
        ask(at, "PUT", "/block/x", Some(&my_secret), b"mine")
            .await
            .0,
        200
    );
    // **each reaches its own and only its own**, and there is no instance
    // named in the request to get wrong: the secret resolves it (§9.3)
    let (code, body) = ask(at, "GET", "/block/x", Some(&your_secret), b"").await;
    assert_eq!(code, 200);
    assert!(body.is_empty(), "the other instance's block holds no `x`");
    assert_eq!(mine.list(), vec!["x".to_string()]);
    assert!(yours.list().is_empty(), "nothing was written to the other");

    // no secret at all, and a secret that is nobody's, are the same answer
    assert_eq!(ask(at, "GET", "/block", None, b"").await.0, 401);
    assert_eq!(
        ask(at, "GET", "/block", Some(&"ab".repeat(32)), b"")
            .await
            .0,
        401
    );

    // **an instance the operator stopped hosting keeps nothing here**: its
    // secret stops with the binding rather than after it
    surface.retain(&[kh(3)]);
    assert_eq!(
        ask(at, "GET", "/block", Some(&your_secret), b"").await.0,
        401
    );
    assert_eq!(ask(at, "GET", "/block", Some(&my_secret), b"").await.0, 200);

    // **a target with more than one reading is refused outright**, the
    // rule §3.3 states for the leg running the other way
    assert_eq!(
        ask(at, "GET", "/block/%2e%2e%2fother", Some(&my_secret), b"")
            .await
            .0,
        400
    );
    assert_eq!(
        ask(at, "GET", "/block/../other", Some(&my_secret), b"")
            .await
            .0,
        400
    );
}
