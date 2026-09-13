//! The command line's entry point.

use rhtn_cli::{inspect, keys, probe};
use rhtn_codec::frame::Stream;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
usage: rhtn <command> [...]

  inspect [--kind <kind> | --frame control|request] [<file>]
        Decode bytes and print them. Hex or raw, from <file> or standard
        input. <kind> is the object's name in wire-format.md.

  keys mint <file>
        Write a new identity, readable by its owner alone. Refuses to
        replace one.
  keys show <file>
        Print an identity file's keyhash and key material.
  keys test <name>
        Print the test identity test-vectors/keys.md derives for <name>.
        These are not secret.

  probe <identity> <node-keyhash> <address> <ask> [...]
        Attach and ask one read-only question. <ask> is one of:
          resolve <subject> <anchor> [<path-nibbles>]
          archive <subject> [<max>]
          catalog [<service-type>]
        --peer <keyhash>:<material> pins another identity; repeatable.

Nothing here sends a request that changes state or spends a budget.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let r = match args.split_first() {
        Some((&"inspect", rest)) => do_inspect(rest),
        Some((&"keys", rest)) => do_keys(rest),
        Some((&"probe", rest)) => do_probe(rest),
        _ => {
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("rhtn: {e}");
            ExitCode::FAILURE
        }
    }
}

/// One flag and its value, taken out of the arguments.
fn flag<'a>(args: &mut Vec<&'a str>, name: &str) -> Option<&'a str> {
    let i = args.iter().position(|a| *a == name)?;
    if i + 1 >= args.len() {
        return None;
    }
    args.remove(i);
    Some(args.remove(i))
}

fn do_inspect(rest: &[&str]) -> Result<String, String> {
    let mut args: Vec<&str> = rest.to_vec();
    let kind = flag(&mut args, "--kind");
    let frame = flag(&mut args, "--frame");
    let raw = match args.first() {
        Some(path) => std::fs::read(path).map_err(|e| format!("{path}: {e}"))?,
        None => {
            use std::io::Read;
            let mut buf = Vec::new();
            std::io::stdin().read_to_end(&mut buf).map_err(|e| e.to_string())?;
            buf
        }
    };
    let bytes = inspect::read_blob(&raw);
    let what = match (kind, frame) {
        (Some(_), Some(_)) => return Err("--kind and --frame say two different things about one blob".into()),
        (Some(k), None) => inspect::As::Kind(k),
        (None, Some("control")) => inspect::As::Frame(Stream::Control),
        (None, Some("request")) => inspect::As::Frame(Stream::Request),
        (None, Some(other)) => return Err(format!("--frame is control or request, not {other}")),
        (None, None) => inspect::As::Shape,
    };
    Ok(inspect::describe(&bytes, what, &known_identities()))
}

fn do_keys(rest: &[&str]) -> Result<String, String> {
    match rest {
        ["mint", path] => {
            let mut seeds = [[0u8; 32]; 2];
            for s in &mut seeds {
                *s = rhtn_transport::tls::random_bytes::<32>();
            }
            let id = keys::mint(&PathBuf::from(path), seeds)?;
            Ok(format!("written   {path}\n{}", keys::public_lines(&id)))
        }
        ["show", path] => keys::describe(&PathBuf::from(path)),
        ["test", name] => {
            let [ed, pq] = keys::test_seeds(name);
            let id = rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq).public;
            Ok(format!("{}seeds     {} {}\n", keys::public_lines(&id), inspect::hex(&ed), inspect::hex(&pq)))
        }
        _ => Err("keys mint <file> | keys show <file> | keys test <name>".into()),
    }
}

fn do_probe(rest: &[&str]) -> Result<String, String> {
    let mut args: Vec<&str> = rest.to_vec();
    let mut peers: Vec<rhtn_crypto::Identity> = Vec::new();
    while let Some(p) = flag(&mut args, "--peer") {
        let (_, material) = p.split_once(':').ok_or("--peer is <keyhash>:<material>")?;
        let bytes = hex_bytes(material).ok_or("--peer material is not hex")?;
        peers.push(rhtn_crypto::Identity::from_key_material(&bytes).ok_or("--peer material is not a KeyMaterial array")?);
    }
    let [identity, target, addr, rest @ ..] = args.as_slice() else {
        return Err("probe <identity> <node-keyhash> <address> <ask> [...]".into());
    };
    let ask = match rest {
        ["resolve", subject, anchor, path @ ..] => {
            let nibbles: Vec<u8> = path.first().map(|p| p.bytes().map(|c| c - b'0').collect()).unwrap_or_default();
            let p = rhtn_node::resolution::Path::from_indices(&nibbles);
            probe::Ask::Resolve { subject: keyhash(subject)?, anchor: keyhash(anchor)?, path: p.bytes, nibbles: p.nibbles }
        }
        ["archive", subject, max @ ..] => probe::Ask::Archive {
            subject: keyhash(subject)?,
            max_records: max.first().and_then(|m| m.parse().ok()).unwrap_or(16),
        },
        ["catalog", service @ ..] => probe::Ask::Catalog { service_type: service.first().map(|s| s.to_string()) },
        _ => return Err("the ask is resolve, archive or catalog".into()),
    };
    let target = keyhash(target)?;
    let addr: std::net::SocketAddr = addr.parse().map_err(|_| format!("{addr} is not an address and port"))?;
    let me = read_identity(identity)?;
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?;
    rt.block_on(async move {
        let (session, _ep) = probe::attached(me, &peers, target, addr).await?;
        probe::ask(&session, &ask, rhtn_transport::tls::random_bytes::<16>()).await
    })
}

fn read_identity(path: &str) -> Result<rhtn_crypto::SigningIdentity, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if bytes.len() != keys::IDENTITY_BYTES {
        return Err(format!("{path} is {} bytes, not {}", bytes.len(), keys::IDENTITY_BYTES));
    }
    let (ed, pq): ([u8; 32], [u8; 32]) = (bytes[..32].try_into().unwrap(), bytes[32..].try_into().unwrap());
    Ok(rhtn_crypto::SigningIdentity::from_seeds(&ed, &pq))
}

fn keyhash(s: &str) -> Result<[u8; 32], String> {
    hex_bytes(s).and_then(|b| b.try_into().ok()).ok_or_else(|| format!("{s} is not a 64-digit hex keyhash"))
}

fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()).collect()
}

/// The identities this tool verifies against: those given with `--peer`
/// and no others.  It holds no key store, and says "unverifiable" rather
/// than guessing (`wire-format.md` §3.4).
fn known_identities() -> Vec<rhtn_crypto::Identity> {
    Vec::new()
}
