//! **The socket under [`rhtn_testapp::App`]**, and nothing else: a
//! resource the node reaches over a local port
//! (`resource-requirements.md` §3's second leg, `infra-client-requirements.md`
//! §10.6).
//!
//! ```text
//! rhtn-testapp --listen 127.0.0.1:7460 --resource <hex> [--log <path>]
//! rhtn-testapp keyhash <material-hex>
//! ```
//!
//! `--resource` is the keyhash the node hosts it under, which is the
//! audience every claim is addressed to; without it the claims cannot be
//! checked and the application says so rather than passing them.
//! `keyhash` is the one convenience it carries for the bench: the keyhash
//! of a phone's key material, which the hosting file needs as the owner.

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
        let before = app.said.len();
        let out = app.handle(&buf);
        let _ = stream.write_all(&out);
        let _ = stream.flush();
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

/// `keyhash <hex>` and nothing else takes a positional argument.
trait Two {
    fn strip_two(&self, verb: &str) -> Option<String>;
}

impl Two for Vec<String> {
    fn strip_two(&self, verb: &str) -> Option<String> {
        (self.len() == 2 && self[0] == verb).then(|| self[1].clone())
    }
}
