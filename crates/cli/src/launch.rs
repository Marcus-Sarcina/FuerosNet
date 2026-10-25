//! `node launch`, `node destroy`, `node offers`: **H1 from a desktop**
//! (`light-client-requirements.md` §10; `infra-client-requirements.md`
//! §8.3; OPS-020).  The composition and the provider adapters are
//! `rhtn-launch`'s; what this file adds is a desktop's HTTPS transport over
//! the operating system's own trust roots, the wait for the machine's
//! address and for its surface, and the hand-off to [`crate::enrol`],
//! which signs the run once the proof checks, exactly as a by-hand
//! enrolment does.
//!
//! **The credential comes from a file the operator names** and is never
//! printed: on a desktop the operator pastes it there from the provider's
//! console.  The kernel-held copy that the phone carries in its backup is
//! the client's; this instrument holds nothing between runs.

use crate::enrol;
use rhtn_crypto::SigningIdentity;
use rhtn_launch::{Image, Plan, Provider, Standing, Transport};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long one exchange with a provider may take.
const TIMEOUT: Duration = Duration::from_secs(30);
/// How long a machine gets to come up with an address.
const ADDRESS_WAIT: Duration = Duration::from_secs(5 * 60);
/// How long the instance gets to install, pull the image and open its
/// surface.
const SURFACE_WAIT: Duration = Duration::from_secs(8 * 60);

/// **HTTPS over the operating system's trust roots**, one request per
/// connection, HTTP/1.1 with `Connection: close` so the body ends with the
/// stream; a chunked body is reassembled.  No bundle ships with this
/// binary: a provider is reached the way any other server is, with what
/// the host trusts (`infra-client-requirements.md` §8.2).
pub struct TlsTransport {
    config: Arc<rustls::ClientConfig>,
}

impl TlsTransport {
    /// A transport over the roots this system trusts; refused where none
    /// can be read, since a provider cannot be authenticated without them.
    pub fn new() -> Result<TlsTransport, String> {
        let certs = rustls_native_certs::load_native_certs();
        if certs.certs.is_empty() {
            return Err(format!(
                "no trust roots could be read from this system ({} errors)",
                certs.errors.len()
            ));
        }
        let mut roots = rustls::RootCertStore::empty();
        roots.add_parsable_certificates(certs.certs);
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(TlsTransport {
            config: Arc::new(config),
        })
    }
}

impl Transport for TlsTransport {
    fn exchange(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<(u16, String), String> {
        let (host, port, path) = split_url(url)?;
        let server = rustls::pki_types::ServerName::try_from(host.clone())
            .map_err(|_| format!("{host} is not a host name"))?;
        let conn = rustls::ClientConnection::new(self.config.clone(), server)
            .map_err(|e| format!("tls: {e}"))?;
        let addr = format!("{host}:{port}");
        let tcp = TcpStream::connect(&addr).map_err(|e| format!("connect {addr}: {e}"))?;
        tcp.set_read_timeout(Some(TIMEOUT)).ok();
        tcp.set_write_timeout(Some(TIMEOUT)).ok();
        let mut stream = rustls::StreamOwned::new(conn, tcp);
        let body_bytes = body.unwrap_or("").as_bytes();
        let mut head = format!("{method} {path} HTTP/1.1\r\nhost: {host}\r\nconnection: close\r\n");
        for (k, v) in headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        head.push_str(&format!("content-length: {}\r\n\r\n", body_bytes.len()));
        stream
            .write_all(head.as_bytes())
            .and_then(|_| stream.write_all(body_bytes))
            .map_err(|e| format!("send to {host}: {e}"))?;
        let mut raw = Vec::new();
        if let Err(e) = stream.read_to_end(&mut raw) {
            // a close without close_notify is how most servers end a
            // connection: what was read stands
            if raw.is_empty() {
                return Err(format!("read from {host}: {e}"));
            }
        }
        parse_response(&raw)
    }
}

/// `https://host[:port]/path` into its three parts; `http://` is taken too,
/// for a test's server, and spoken plain.
fn split_url(url: &str) -> Result<(String, u16, String), String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| format!("{url} has no scheme"))?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => {
            (h.to_string(), p.parse::<u16>().map_err(|_| "a port")?)
        }
        _ => (
            authority.to_string(),
            if scheme == "http" { 80 } else { 443 },
        ),
    };
    if scheme != "https" {
        return Err(format!("{url}: a provider is spoken to over https"));
    }
    Ok((host, port, path.to_string()))
}

/// The status and the body of an HTTP/1.1 response, a chunked body
/// reassembled.
fn parse_response(raw: &[u8]) -> Result<(u16, String), String> {
    let text = String::from_utf8_lossy(raw);
    let (head, body) = text
        .split_once("\r\n\r\n")
        .ok_or("the answer had no header block")?;
    let mut lines = head.lines();
    let status_line = lines.next().unwrap_or("");
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("no status in {status_line:?}"))?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let body = if chunked {
        dechunk(body)
    } else {
        body.to_string()
    };
    Ok((status, body))
}

fn dechunk(body: &str) -> String {
    let mut out = String::new();
    let mut rest = body;
    while let Some((size_line, after)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size_line.trim().split(';').next().unwrap_or("0"), 16)
            .unwrap_or(0);
        if size == 0 {
            break;
        }
        let Some(chunk) = after.get(..size) else {
            break;
        };
        out.push_str(chunk);
        rest = after.get(size + 2..).unwrap_or("");
    }
    out
}

/// What `node launch` was asked.
pub struct Ask {
    /// The provider's name, as `rhtn_launch::provider` knows it.
    pub provider: String,
    /// The credential the operator holds there.
    pub credential: String,
    /// Where, how big, which images.
    pub plan: Plan,
    /// The machine's name at the provider; one from the token otherwise.
    pub name: Option<String>,
    /// How many 48-hour credentials the run carries.
    pub credentials: usize,
    /// The subtree size to claim in an anchor entry, where one is wanted.
    pub subtree: Option<u64>,
}

/// **Launch, wait, enrol**: the machine created with the composed script,
/// polled for its address, its surface polled until it answers, then the
/// enrolment with the token this instrument minted.  Each step is a line;
/// a step that fails stops the rest and says what is left running.
pub fn launch(me: &SigningIdentity, ask: &Ask) -> Result<String, String> {
    let provider = rhtn_launch::provider(&ask.provider)
        .ok_or_else(|| format!("{} is not a provider this build drives", ask.provider))?;
    let t = TlsTransport::new()?;
    let mut random = [0u8; 32];
    rustls::crypto::aws_lc_rs::default_provider()
        .secure_random
        .fill(&mut random)
        .map_err(|e| format!("no randomness: {e:?}"))?;
    let token_hex = rhtn_launch::token_hex(&random);
    let token = enrol::hex32(&token_hex).ok_or("the token did not read back")?;
    let material = hex(&me.public.key_material());
    let script = rhtn_launch::user_data(
        &material,
        &token_hex,
        &ask.plan.image,
        rhtn_launch::QUIC_PORT,
        rhtn_launch::ADMIN_PORT,
    );
    let name = ask
        .name
        .clone()
        .unwrap_or_else(|| format!("rhtn-{}", &token_hex[..8]));
    let mut said = Vec::new();
    let launched = provider.launch(&t, &ask.credential, &ask.plan, &name, &script)?;
    said.push(format!(
        "{} created machine {} in {} as {name}; waiting for its address",
        provider.name(),
        launched.id,
        ask.plan.region
    ));
    let ip = wait_for_address(provider.as_ref(), &t, &ask.credential, &launched.id)
        .ok_or_else(|| {
            format!(
                "{}\nno address within five minutes; machine {} is running at {} and can be destroyed with `node destroy`",
                said.join("\n"),
                launched.id,
                provider.name()
            )
        })?;
    let surface = format!("{ip}:{}", rhtn_launch::ADMIN_PORT);
    said.push(format!(
        "address {ip}; waiting for the instance to install, pull the image and open its surface at {surface}"
    ));
    let enrol_ask = enrol::Ask {
        at: surface.clone(),
        token,
        credentials: ask.credentials,
        endpoint: Some(
            format!("{ip}:{}", rhtn_launch::QUIC_PORT)
                .parse()
                .map_err(|_| "the address did not parse")?,
        ),
        subtree: ask.subtree,
    };
    let started = Instant::now();
    let mut last = String::new();
    while started.elapsed() < SURFACE_WAIT {
        match enrol::enrol(me, &enrol_ask) {
            Ok(lines) => {
                said.push("its surface answers; enrolled:".into());
                said.push(lines);
                said.push(format!(
                    "token {token_hex}\nkeep it: `node renew {surface} --token <it>` renews the run"
                ));
                return Ok(said.join("\n"));
            }
            Err(e) => {
                // a surface not yet open is a connect failure; anything
                // else is the enrolment's own refusal and ends this
                if !e.contains("connect") {
                    return Err(format!("{}\n{e}", said.join("\n")));
                }
                last = e;
                std::thread::sleep(Duration::from_secs(10));
            }
        }
    }
    Err(format!(
        "{}\nthe surface did not answer within eight minutes ({last}); the machine is running: enrol it by hand with the token {token_hex}, or `node destroy` it",
        said.join("\n")
    ))
}

fn wait_for_address(
    provider: &dyn Provider,
    t: &dyn Transport,
    credential: &str,
    id: &str,
) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < ADDRESS_WAIT {
        std::thread::sleep(Duration::from_secs(5));
        if let Ok(Standing {
            active: true,
            ipv4: Some(ip),
            ..
        }) = provider.standing(t, credential, id)
        {
            return Some(ip);
        }
    }
    None
}

/// `node destroy`: the machine at the provider, by its id there.
pub fn destroy(provider: &str, credential: &str, id: &str) -> Result<String, String> {
    let p = rhtn_launch::provider(provider)
        .ok_or_else(|| format!("{provider} is not a provider this build drives"))?;
    let t = TlsTransport::new()?;
    p.destroy(&t, credential, id)?;
    Ok(format!("destroyed machine {id} at {}", p.name()))
}

/// `node offers`: the regions and sizes a provider offers, for choosing.
pub fn offers(provider: &str, credential: &str) -> Result<String, String> {
    let p = rhtn_launch::provider(provider)
        .ok_or_else(|| format!("{provider} is not a provider this build drives"))?;
    let t = TlsTransport::new()?;
    let mut out = vec![format!("{} regions:", p.name())];
    for r in p.regions(&t, credential)? {
        if r.available {
            out.push(format!("  {:<10} {}", r.slug, r.name));
        }
    }
    out.push("sizes, cheapest first:".into());
    let mut sizes = p.sizes(&t, credential)?;
    sizes.sort_by(|a, b| {
        a.monthly
            .partial_cmp(&b.monthly)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for s in sizes.iter().take(12) {
        out.push(format!(
            "  {:<16} {:>5} MB  {} vcpu  ${:.2}/month",
            s.slug, s.memory_mb, s.vcpus, s.monthly
        ));
    }
    Ok(out.join("\n"))
}

/// A plan from the options, with the image where one was named.
pub fn plan(region: &str, size: &str, registry: Option<&str>, tag: Option<&str>) -> Plan {
    let mut p = Plan::new(region, size);
    p.image = Image {
        registry: registry
            .unwrap_or(rhtn_launch::REGISTRY_DEFAULT)
            .to_string(),
        tag: tag.unwrap_or(rhtn_launch::TAG_DEFAULT).to_string(),
    };
    p
}

/// The token a credential file holds: the operator's own, pasted at
/// `node credential` or written there by hand.
pub fn credential_from(path: &str) -> Result<String, String> {
    let s = std::fs::read_to_string(path).map_err(|e| format!("read {path}: {e}"))?;
    let t = s.trim();
    if t.is_empty() {
        return Err(format!("{path} is empty"));
    }
    Ok(t.to_string())
}

/// **`node credential`: the provider's token page, the steps there, and
/// the token pasted** (`light-client-requirements.md` §10).  No
/// application is registered with any provider and nothing of the
/// project's stands between the operator and the provider: the operator
/// makes a token of their own at the provider's console, scoped and
/// expiring as they choose, and pastes it.  The page is opened in the
/// person's browser and printed as well, the steps are printed beside it,
/// the token is read from `input`, tried at the provider, and written to
/// `out` readable by its owner alone.
pub fn credential(
    provider: &str,
    out: &str,
    input: &mut dyn std::io::BufRead,
) -> Result<String, String> {
    let p = rhtn_launch::provider(provider)
        .ok_or_else(|| format!("{provider} is not a provider this build drives"))?;
    let page = p.token_page();
    eprintln!("make a token at {}'s own page:\n  {page}", p.name());
    for (n, step) in p.token_walkthrough().iter().enumerate() {
        eprintln!("  {}. {step}", n + 1);
    }
    // the person's browser, where one answers; the URL above is the
    // fallback and the record either way
    let _ = std::process::Command::new("xdg-open")
        .arg(page)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    eprint!("paste the token here: ");
    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(|e| format!("read the token: {e}"))?;
    let token = line.trim();
    if token.is_empty() {
        return Err("no token was pasted".into());
    }
    let t = TlsTransport::new()?;
    let regions = p.regions(&t, token)?;
    write_private(out, token)?;
    Ok(format!(
        "the token answers at {}: {} regions offered; kept in {out}",
        p.name(),
        regions.len()
    ))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Write a credential document readable by its owner alone, as `keys mint`
/// writes an identity: a bearer token is a key.
fn write_private(path: &str, text: &str) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| format!("write {path}: {e}"))?;
        f.write_all(text.as_bytes())
            .map_err(|e| format!("write {path}: {e}"))?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("write {path}: {e}"))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, text).map_err(|e| format!("write {path}: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunked_answer_is_reassembled_and_a_plain_one_read_as_is() {
        let raw = b"HTTP/1.1 200 OK\r\ntransfer-encoding: chunked\r\n\r\n5\r\nhello\r\n6\r\n world\r\n0\r\n\r\n";
        assert_eq!(
            parse_response(raw).unwrap(),
            (200, "hello world".to_string())
        );
        let raw = b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 2\r\n\r\n{}";
        assert_eq!(parse_response(raw).unwrap(), (401, "{}".to_string()));
    }

    #[test]
    fn a_url_splits_and_only_https_is_spoken() {
        assert_eq!(
            split_url("https://api.digitalocean.com/v2/droplets?x=1").unwrap(),
            (
                "api.digitalocean.com".to_string(),
                443,
                "/v2/droplets?x=1".to_string()
            )
        );
        assert_eq!(split_url("https://h:8443").unwrap().1, 8443);
        assert!(split_url("http://plain.example/").is_err());
    }
}
