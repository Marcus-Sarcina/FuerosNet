//! **The operator's side of the administration surface**
//! (`rhtn_cli::enrol`; `infra-client-requirements.md` §4.4, §7): a
//! renewal against a scripted surface, which signs the next run and
//! **re-signs the anchor entry where the instance's count has moved under
//! it** [author, 2026-10-26].

use rhtn_crypto::identity::testkit::test_identity;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, Mutex};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).expect("hex"))
        .collect()
}

/// What the scripted surface was asked, each request's line and body.
type Asked = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// A surface that answers `n` requests: the fetch with the fields given,
/// and every PUT with `200` and one word. The transport key is the
/// instance's, and the proof is computed over the nonce the caller chose,
/// so `enrol`'s own check passes exactly as it would against a daemon.
fn surface(token: [u8; 32], key: [u8; 32], fields: String, n: usize) -> (SocketAddr, Asked) {
    let l = TcpListener::bind("127.0.0.1:0").expect("a port");
    let addr = l.local_addr().unwrap();
    let asked: Asked = Arc::new(Mutex::new(Vec::new()));
    let keep = asked.clone();
    std::thread::spawn(move || {
        for _ in 0..n {
            let Ok((mut c, _)) = l.accept() else { continue };
            let mut buf = Vec::new();
            let mut chunk = [0u8; 8192];
            // the caller writes a head with a length and then the body,
            // and closes nothing until it has read: read to the length
            let head_end = loop {
                match c.read(&mut chunk) {
                    Ok(0) => break buf.len(),
                    Ok(k) => {
                        buf.extend_from_slice(&chunk[..k]);
                        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                    }
                    Err(_) => break buf.len(),
                }
            };
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let want: usize = head
                .lines()
                .find_map(|x| x.strip_prefix("content-length: "))
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0);
            while buf.len() < head_end + want {
                match c.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(k) => buf.extend_from_slice(&chunk[..k]),
                }
            }
            let line = head.lines().next().unwrap_or("").to_string();
            keep.lock()
                .unwrap()
                .push((line.clone(), buf[head_end..].to_vec()));
            let body = if line.starts_with("GET /node?nonce=") {
                let nonce = unhex(
                    line.trim_start_matches("GET /node?nonce=")
                        .split(' ')
                        .next()
                        .unwrap_or(""),
                );
                format!(
                    "transport {}\nproof {}\n{fields}",
                    hex(&key),
                    hex(&rhtn_daemon::administration::proof(&token, &nonce, &key)),
                )
            } else {
                "taken\n".to_string()
            };
            let _ = c.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: text/plain\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        }
    });
    (addr, asked)
}

#[test]
fn a_renewal_signs_the_next_run_and_re_signs_the_entry_where_the_subtree_moved() {
    let me = test_identity("bob");
    let token = [7u8; 32];
    let key = [9u8; 32];
    // the point the standing entry names, which a re-signing must keep
    let point = rhtn_node::resolution::NetworkPoint::new([203, 0, 113, 9], Some(7431));
    let fields = format!(
        "phase serving\ncredentials 1\nrun-ends 1800000000\nendpoint-record held\n\
         anchor-entry held\nsubtree 4\nanchor-claims 1\nanchor-seqno 3\nanchor-points {}\n",
        hex(&point.encode_bytes())
    );
    // the fetch, one run credential, and the re-signed entry
    let (at, asked) = surface(token, key, fields, 3);
    let said = rhtn_cli::enrol::renew(
        &me,
        &rhtn_cli::enrol::Ask {
            at: at.to_string(),
            token,
            credentials: 1,
            endpoint: None,
        },
    )
    .expect("the renewal goes through");

    assert!(said.contains("the run ends at 1800000000"), "{said}");
    assert!(
        said.contains("anchor entry re-signed at seqno 4: a subtree of 4 where it claimed 1"),
        "{said}"
    );

    let asked = asked.lock().unwrap().clone();
    assert!(
        asked[0].0.starts_with("GET /node?nonce="),
        "{:?}",
        asked[0].0
    );
    assert_eq!(asked[1].0, "PUT /node/run HTTP/1.1");
    assert_eq!(asked[2].0, "PUT /node/anchor-entry HTTP/1.1");

    let entry = rhtn_node::resolution::AnchorEntry::parse(&asked[2].1).expect("an anchor entry");
    assert_eq!(entry.anchor, me.public.keyhash, "signed for this node");
    assert_eq!(entry.subtree_size, 4, "the size the instance counted");
    assert_eq!(entry.seqno.counter, 4, "the next counter, so it replaces");
    assert_eq!(
        entry.endpoints,
        vec![point],
        "the addresses the standing entry named, unchanged"
    );
}

#[test]
fn a_renewal_leaves_the_entry_alone_where_the_count_is_what_it_claims() {
    let me = test_identity("bob");
    let token = [7u8; 32];
    let key = [9u8; 32];
    let fields = "phase serving\ncredentials 1\nrun-ends 1800000000\nendpoint-record held\n\
                  anchor-entry held\nsubtree 2\nanchor-claims 2\nanchor-seqno 3\n"
        .to_string();
    let (at, asked) = surface(token, key, fields, 2);
    let said = rhtn_cli::enrol::renew(
        &me,
        &rhtn_cli::enrol::Ask {
            at: at.to_string(),
            token,
            credentials: 1,
            endpoint: None,
        },
    )
    .expect("the renewal goes through");
    assert!(
        said.contains("it claims a subtree of 2 and counts 2, so it stands"),
        "{said}"
    );
    let asked = asked.lock().unwrap().clone();
    assert_eq!(asked.len(), 2, "nothing was put to the entry: {asked:?}");
    assert!(asked.iter().all(|(l, _)| !l.contains("anchor-entry")));
}
