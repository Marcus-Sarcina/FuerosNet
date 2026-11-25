//! **The socket under [`rhtn_testapp::App`]**, and nothing else: a
//! resource the node reaches over a local port
//! (`resource-requirements.md` §3's second leg, `infra-client-requirements.md`
//! §10.6).
//!
//! ```text
//! rhtn-testapp [--stream] --listen 127.0.0.1:7460 --resource <hex> [--log <path>]
//! rhtn-testapp keyhash <material-hex>
//! ```
//!
//! `--resource` is the keyhash the node hosts it under, which is the
//! audience every claim is addressed to; without it the claims cannot be
//! checked and the application says so rather than passing them.
//! `keyhash` is the one convenience it carries for the bench: the keyhash
//! of a phone's key material, which the hosting file needs as the owner.
//! `--stream`, or `RHTN_TESTAPP_FACE=stream` in the environment, serves the
//! other face, [`rhtn_testapp::stream`]: content in parts, in place of the
//! walk through the utilities.

use rhtn_testapp::{App, hex, unhex, wanted};
use std::io::{Read, Write};
use std::net::TcpListener;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(material) = args.strip_two("keyhash") {
        let Some(bytes) = unhex(&material) else {
            eprintln!("rhtn-testapp: the material is not hex");
            return std::process::ExitCode::from(2);
        };
        match rhtn_crypto::Identity::from_key_material(&bytes) {
            Some(id) => {
                println!("{}", hex(&id.keyhash));
                return std::process::ExitCode::SUCCESS;
            }
            None => {
                eprintln!("rhtn-testapp: those bytes are not one identity's key material");
                return std::process::ExitCode::from(2);
            }
        }
    }
    let mut listen = "127.0.0.1:7460".to_string();
    let mut resource = String::new();
    let mut log: Option<String> = None;
    let mut secret: Option<[u8; 32]> = None;
    // **which face an image serves is the image's own fact**: a host
    // starts every package with the same arguments
    // (`infra-client-requirements.md` §9.1), so an image built to serve
    // the other face says so in its environment
    let mut streaming: Option<rhtn_testapp::stream::Stream> =
        (std::env::var("RHTN_TESTAPP_FACE").as_deref() == Ok("stream"))
            .then(rhtn_testapp::stream::Stream::new);
    // **a bench probe, where an image names one**: listen on a port of
    // this host's for a packet that came by no request, and tell the page
    // where to aim a peer connection's first one. Nothing is answered.
    if let Ok(at) = std::env::var("RHTN_TESTAPP_PROBE")
        && !at.is_empty()
        && let Some(face) = streaming.take()
    {
        if let Some(port) = at.rsplit(':').next().and_then(|p| p.parse::<u16>().ok()) {
            std::thread::spawn(move || listen_for_datagrams(port));
            // one port for each way a page might open a stream: its own
            // socket, a window's it made, a worker's, a preconnect, a
            // fetch or a beacon, and a relay a peer connection asks
            for beside in (1..=6u16).filter_map(|n| port.checked_add(n)) {
                std::thread::spawn(move || listen_for_connections(beside));
            }
        }
        streaming = Some(face.probing(&at));
    }
    // **and a live session asked for before the page**, where an image
    // names its seconds
    if let Some(seconds) = std::env::var("RHTN_TESTAPP_LIVE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        && let Some(face) = streaming.take()
    {
        streaming = Some(face.living(seconds));
    }
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--listen" => {
                listen = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--resource" => {
                resource = args.get(i + 1).cloned().unwrap_or_default();
                i += 2;
            }
            "--log" => {
                log = args.get(i + 1).cloned();
                i += 2;
            }
            "--stream" => {
                streaming = Some(rhtn_testapp::stream::Stream::new());
                i += 1;
            }
            "--secret" => {
                let Some(s) = args
                    .get(i + 1)
                    .and_then(|h| unhex(h))
                    .and_then(|b| <[u8; 32]>::try_from(b).ok())
                else {
                    eprintln!(
                        "rhtn-testapp: --secret is the 64 hex digits the host started this instance with"
                    );
                    return std::process::ExitCode::from(2);
                };
                secret = Some(s);
                i += 2;
            }
            other => {
                eprintln!("rhtn-testapp: {other} is not an option of this program");
                return std::process::ExitCode::from(2);
            }
        }
    }
    let me: [u8; 32] = match unhex(&resource).and_then(|b| b.try_into().ok()) {
        Some(k) => k,
        None => {
            eprintln!("rhtn-testapp: --resource is the 64 hex digits of this resource's keyhash");
            return std::process::ExitCode::from(2);
        }
    };
    let socket = match TcpListener::bind(&listen) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("rhtn-testapp: {listen}: {e}");
            return std::process::ExitCode::from(1);
        }
    };
    let mut app = App::new(me);
    if let Some(s) = secret {
        app = app.with_secret(s);
    } else {
        eprintln!("rhtn-testapp: started without --secret: every request is answered as anonymous");
    }
    println!(
        "rhtn-testapp: {listen}, resource {}…",
        &resource[..16.min(resource.len())]
    );
    let _ = std::io::stdout().flush();

    // **one connection at a time**, which is what the node opens: a
    // request, an answer, and the connection closed from this end
    for incoming in socket.incoming() {
        let mut stream = match incoming {
            Ok(s) => s,
            Err(e) => {
                eprintln!("rhtn-testapp: accept: {e}");
                continue;
            }
        };
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(20)));
        let mut buf = Vec::new();
        let mut chunk = [0u8; 65536];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    // **the whole message before it is answered**: a
                    // request larger than one frame reaches the node as a
                    // sequence (`wire-format.md` §11) and arrives here as
                    // one body, which can be several reads long
                    if let Some(want) = wanted(&buf)
                        && buf.len() >= want
                    {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("rhtn-testapp: read: {e}");
                    break;
                }
            }
        }
        if buf.is_empty() {
            continue;
        }
        // **the other face**: content in parts, which reads nothing of the
        // node's and owes no notice
        if let Some(face) = streaming.as_mut() {
            let before = face.said.len();
            let out = face.handle(&buf);
            let _ = stream.write_all(&out);
            let _ = stream.flush();
            drop(stream);
            for line in &face.said[before..] {
                println!("{line}");
            }
            let _ = std::io::stdout().flush();
            continue;
        }
        let before = app.said.len();
        let out = app.handle(&buf);
        let _ = stream.write_all(&out);
        let _ = stream.flush();
        drop(stream);
        // **what waits for a member, told to the host once answered**
        // (`resource-requirements.md` §3.3); kept for the next request
        // where the host has not said where, or did not take it
        if let (Some(url), Some(s)) = (app.notices.clone(), secret) {
            let owed = app.take_owed();
            for n in owed {
                match tell(&url, &s, &n) {
                    Ok(204) => println!(
                        "{{\"what\":\"noticed\",\"who\":\"{}\",\"count\":{}}}",
                        n.principal.chars().take(16).collect::<String>(),
                        n.count
                    ),
                    Ok(code) => {
                        eprintln!("rhtn-testapp: the host answered a notice with {code}");
                    }
                    Err(why) => {
                        eprintln!("rhtn-testapp: a notice did not reach the host: {why}");
                        app.owed.push(n);
                    }
                }
            }
        }
        for line in &app.said[before..] {
            println!("{line}");
            if let Some(path) = &log
                && let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
            {
                let _ = writeln!(f, "{line}");
            }
        }
        let _ = std::io::stdout().flush();
    }
    std::process::ExitCode::SUCCESS
}

/// **One notice, put to the host's surface** at `url` (`http://host:port/
/// notices`): the member's principal in the path, the count in the query,
/// the line as the body, the instance's secret presented. The status back.
fn tell(url: &str, secret: &[u8; 32], n: &rhtn_testapp::Notice) -> Result<u16, String> {
    let rest = url
        .strip_prefix("http://")
        .ok_or("the surface is not plain http")?;
    let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
    let body = n.line.clone().unwrap_or_default();
    let request = format!(
        "PUT /{path}/{}?count={} HTTP/1.1\r\nhost: {authority}\r\nrhtn-secret: {}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        n.principal,
        n.count,
        hex(secret),
        body.len()
    );
    let mut s = std::net::TcpStream::connect(authority).map_err(|e| e.to_string())?;
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(10)));
    s.write_all(request.as_bytes()).map_err(|e| e.to_string())?;
    let mut back = String::new();
    let _ = s.read_to_string(&mut back);
    back.split(' ')
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| "no status came back".to_string())
}

/// **Say what arrives on `port` by no request, and answer none of it**: the
/// sender's address, the size, and whether it opens as a STUN binding
/// request does. The first thirty-two, so a flood fills no log.
fn listen_for_datagrams(port: u16) {
    let socket = match std::net::UdpSocket::bind(("0.0.0.0", port)) {
        Ok(s) => s,
        Err(e) => {
            println!(
                "{{\"what\":\"stream.probe\",\"port\":{port},\"bound\":false,\"why\":\"{e}\"}}"
            );
            return;
        }
    };
    println!("{{\"what\":\"stream.probe\",\"port\":{port},\"bound\":true}}");
    let _ = std::io::stdout().flush();
    let mut buf = [0u8; 2048];
    for _ in 0..32 {
        let Ok((n, from)) = socket.recv_from(&mut buf) else {
            return;
        };
        // RFC 8489: a binding request is type 0x0001 under the magic cookie
        let stun = n >= 20 && buf[..2] == [0, 1] && buf[4..8] == [0x21, 0x12, 0xa4, 0x42];
        println!(
            "{{\"what\":\"stream.datagram\",\"from\":\"{from}\",\"bytes\":{n},\"stun\":{stun}}}"
        );
        let _ = std::io::stdout().flush();
    }
}

/// **Say what connects to `port`, and answer none of it**: the caller's
/// address and the first byte it sent, which is 22 where it opens as TLS
/// does. The first thirty-two.
fn listen_for_connections(port: u16) {
    let listener = match TcpListener::bind(("0.0.0.0", port)) {
        Ok(l) => l,
        Err(e) => {
            println!(
                "{{\"what\":\"stream.probe\",\"port\":{port},\"bound\":false,\"why\":\"{e}\"}}"
            );
            return;
        }
    };
    println!("{{\"what\":\"stream.probe\",\"port\":{port},\"bound\":true}}");
    let _ = std::io::stdout().flush();
    for stream in listener.incoming().take(32) {
        let Ok(mut stream) = stream else { continue };
        let from = stream
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();
        let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
        let mut first = [0u8; 1];
        let first = match stream.read(&mut first) {
            Ok(1) => i32::from(first[0]),
            _ => -1,
        };
        println!(
            "{{\"what\":\"stream.connect\",\"port\":{port},\"from\":\"{from}\",\"first\":{first}}}"
        );
        let _ = std::io::stdout().flush();
    }
}

/// `keyhash <hex>` and nothing else takes a positional argument.
trait Two {
    fn strip_two(&self, verb: &str) -> Option<String>;
}

impl Two for Vec<String> {
    fn strip_two(&self, verb: &str) -> Option<String> {
        (self.len() == 2 && self[0] == verb).then(|| self[1].clone())
    }
}
