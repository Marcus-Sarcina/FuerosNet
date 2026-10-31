//! **The host's outbound request on a package's behalf**
//! (`infra-client-requirements.md` §9.3's fetch binding): a package hands
//! one HTTP/1.1 request as bytes, the host sends it over TLS to the
//! destination the request's `host` names — one the manifest declared,
//! `https://host[:port]` exactly — and hands the response back as bytes.
//! The host is the package's egress and nothing more: it reads the request
//! with the one parser, sends it with its own `host` and `connection:
//! close`, bounds what it reads back, follows a redirect only to a declared
//! destination, and refuses, with the reason, anything else.  A declared
//! name that resolves to the node's own machine or network is refused at
//! connection, since the allow-list is of names and the hazard is of
//! addresses.
use rhtn_node::http::{Reject, parse, parse_response, serialise_response};
use rhtn_resources::Fetcher;
use std::io::{Read as _, Write as _};
use std::net::{IpAddr, TcpStream, ToSocketAddrs as _};
use std::sync::Arc;
use std::time::Duration;

/// The most of a response the host carries back, headers and body.
pub const RESPONSE_BYTES: usize = 1 << 20;
/// How long one exchange may take, connecting, writing and reading.
pub const PATIENCE: Duration = Duration::from_secs(10);
/// How many redirects are followed, each to a declared destination.
pub const REDIRECTS: usize = 3;

/// One declared destination: a host name and a port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    /// The host, as declared, lower-cased.
    pub host: String,
    /// The port, 443 where none was declared.
    pub port: u16,
}

impl Destination {
    /// `https://host[:port]`, already checked by
    /// `rhtn_node::resources::destination_refused`.
    fn parse(d: &str) -> Option<Destination> {
        let rest = d.strip_prefix("https://")?;
        let (host, port) = match rest.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (h, p.parse::<u16>().ok()?),
            _ => (rest, 443),
        };
        Some(Destination {
            host: host.trim_matches(['[', ']']).to_ascii_lowercase(),
            port,
        })
    }
}

/// The fetcher for one instance: its declared destinations and the TLS
/// client that trusts the operating system's roots.
pub struct Outbound {
    allowed: Vec<Destination>,
    tls: Arc<rustls::ClientConfig>,
}

impl Outbound {
    /// For the destinations a manifest declared.
    pub fn new(declared: &[String]) -> Outbound {
        let allowed = declared
            .iter()
            .filter_map(|d| Destination::parse(d))
            .collect();
        let mut roots = rustls::RootCertStore::empty();
        for cert in rustls_native_certs::load_native_certs().certs {
            let _ = roots.add(cert);
        }
        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("the provider's default versions")
            .with_root_certificates(roots)
            .with_no_client_auth();
        Outbound {
            allowed,
            tls: Arc::new(tls),
        }
    }

    /// The destinations, as declared.
    #[must_use]
    pub fn allowed(&self) -> &[Destination] {
        &self.allowed
    }

    fn declared(&self, host: &str, port: u16) -> Option<&Destination> {
        let h = host.to_ascii_lowercase();
        self.allowed.iter().find(|d| d.host == h && d.port == port)
    }

    /// Send one request to `to`, read the whole response within the
    /// bounds, and hand it back parsed.
    fn exchange(
        &self,
        to: &Destination,
        method: &str,
        target: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> Result<rhtn_node::http::Response, String> {
        let addrs: Vec<_> = (to.host.as_str(), to.port)
            .to_socket_addrs()
            .map_err(|e| format!("`{}` does not resolve: {e}", to.host))?
            .collect();
        if addrs.is_empty() {
            return Err(format!("`{}` resolves to nothing", to.host));
        }
        if let Some(a) = addrs.iter().find(|a| own_network(&a.ip())) {
            return Err(format!(
                "`{}` resolves to {}, which is the node's own machine or network",
                to.host,
                a.ip()
            ));
        }
        let tcp = addrs
            .iter()
            .find_map(|a| TcpStream::connect_timeout(a, PATIENCE).ok())
            .ok_or_else(|| format!("`{}:{}` could not be reached", to.host, to.port))?;
        tcp.set_read_timeout(Some(PATIENCE))
            .map_err(|e| e.to_string())?;
        tcp.set_write_timeout(Some(PATIENCE))
            .map_err(|e| e.to_string())?;
        let name = rustls::pki_types::ServerName::try_from(to.host.clone())
            .map_err(|_| format!("`{}` is not a name TLS can check", to.host))?;
        let conn = rustls::ClientConnection::new(self.tls.clone(), name)
            .map_err(|e| format!("TLS to `{}`: {e}", to.host))?;
        let mut tls = rustls::StreamOwned::new(conn, tcp);
        let mut out = format!(
            "{method} {target} HTTP/1.1\r\nhost: {}\r\nconnection: close\r\n",
            host_header(to)
        );
        for (n, v) in headers {
            let k = n.to_ascii_lowercase();
            if matches!(
                k.as_str(),
                "host" | "connection" | "content-length" | "transfer-encoding" | "expect"
            ) {
                continue;
            }
            out.push_str(&format!("{n}: {v}\r\n"));
        }
        out.push_str(&format!("content-length: {}\r\n\r\n", body.len()));
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(body);
        tls.write_all(&bytes)
            .map_err(|e| format!("sending to `{}`: {e}", to.host))?;
        let mut reply = Vec::new();
        let mut buf = [0u8; 16 << 10];
        loop {
            match tls.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    reply.extend_from_slice(&buf[..n]);
                    if reply.len() > RESPONSE_BYTES {
                        return Err(format!(
                            "the response from `{}` is over {RESPONSE_BYTES} bytes",
                            to.host
                        ));
                    }
                }
                // a close without close_notify is how most servers end a
                // `connection: close` exchange; what was read stands
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(format!("reading from `{}`: {e}", to.host)),
            }
        }
        parse_response(&reply)
            .map_err(|r| format!("the response from `{}` does not read: {r:?}", to.host))
    }
}

fn host_header(d: &Destination) -> String {
    if d.port == 443 {
        d.host.clone()
    } else {
        format!("{}:{}", d.host, d.port)
    }
}

/// Whether an address is the node's own machine or network: loopback,
/// private, link-local, unspecified, or a unique-local or link-local v6.
fn own_network(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            v.is_loopback()
                || v.is_private()
                || v.is_link_local()
                || v.is_unspecified()
                || v.is_broadcast()
        }
        IpAddr::V6(v) => {
            v.is_loopback()
                || v.is_unspecified()
                || (v.segments()[0] & 0xfe00) == 0xfc00
                || (v.segments()[0] & 0xffc0) == 0xfe80
                || v.to_ipv4_mapped()
                    .is_some_and(|m| own_network(&IpAddr::V4(m)))
        }
    }
}

/// Where a redirect points, as a destination and a target, where it is
/// an absolute `https` URL.
fn redirect_target(location: &str) -> Option<(Destination, String)> {
    let rest = location.strip_prefix("https://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let d = Destination::parse(&format!("https://{authority}"))?;
    Some((d, path.to_string()))
}

impl Fetcher for Outbound {
    fn fetch(&self, request: &[u8]) -> Result<Vec<u8>, String> {
        let req =
            parse(request).map_err(|r: Reject| format!("the request does not read: {r:?}"))?;
        if !matches!(
            req.method.as_str(),
            "GET" | "HEAD" | "POST" | "PUT" | "DELETE"
        ) {
            return Err(format!("`{}` is not a method the host sends", req.method));
        }
        if !req.target.starts_with('/') {
            return Err("the target is a path; the destination is the `host` header".into());
        }
        // the parser checks that one `host` is there and keeps it to
        // itself (the gateway writes its own); the destination is read
        // off the head the parser just accepted
        let head = request
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .map_or(request, |i| &request[..i]);
        let host = String::from_utf8_lossy(head)
            .lines()
            .skip(1)
            .find_map(|l| {
                let (n, v) = l.split_once(':')?;
                n.trim()
                    .eq_ignore_ascii_case("host")
                    .then(|| v.trim().to_string())
            })
            .ok_or("no `host` header: the destination is the `host` header")?;
        let (h, port) = match host.rsplit_once(':') {
            Some((h, p)) if !h.contains(':') => (
                h.to_string(),
                p.parse::<u16>()
                    .map_err(|_| format!("`{p}` is not a port"))?,
            ),
            _ => (host.clone(), 443),
        };
        let mut to = self
            .declared(&h, port)
            .cloned()
            .ok_or_else(|| format!("`{host}` is not a destination the package declared"))?;
        let mut target = req.target.clone();
        let mut hops = 0;
        loop {
            let resp = self.exchange(&to, &req.method, &target, &req.headers, &req.body)?;
            if matches!(resp.status, 301 | 302 | 303 | 307 | 308)
                && let Some((_, loc)) = resp
                    .headers
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case("location"))
            {
                hops += 1;
                if hops > REDIRECTS {
                    return Err(format!("more than {REDIRECTS} redirects"));
                }
                let (next, path) = match redirect_target(loc) {
                    Some(x) => x,
                    None if loc.starts_with('/') => (to.clone(), loc.clone()),
                    None => return Err(format!("a redirect to `{loc}`, which is not https")),
                };
                let Some(d) = self.declared(&next.host, next.port) else {
                    return Err(format!(
                        "a redirect to `{}:{}`, which the package did not declare",
                        next.host, next.port
                    ));
                };
                to = d.clone();
                target = path;
                continue;
            }
            return Ok(serialise_response(&resp));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out() -> Outbound {
        Outbound::new(&[
            "https://feeds.example".into(),
            "https://api.example:8443".into(),
        ])
    }

    #[test]
    fn a_request_names_its_destination_and_only_a_declared_one_is_sent() {
        let o = out();
        assert_eq!(o.allowed().len(), 2);
        assert_eq!(o.allowed()[1].port, 8443);
        let why = o
            .fetch(b"GET /rss HTTP/1.1\r\nhost: other.example\r\n\r\n")
            .unwrap_err();
        assert!(why.contains("not a destination"), "{why}");
        let why = o
            .fetch(b"GET /rss HTTP/1.1\r\nhost: api.example\r\n\r\n")
            .unwrap_err();
        assert!(
            why.contains("not a destination"),
            "{why}: the port is part of it"
        );
        let why = o.fetch(b"GET /rss HTTP/1.1\r\n\r\n").unwrap_err();
        assert!(
            why.contains("no host") || why.contains("no `host`"),
            "{why}"
        );
        // the parser refuses a method it does not carry before this
        // fetcher sees it; one it carries and this fetcher does not send
        // is refused here
        let why = o
            .fetch(b"CONNECT feeds.example:443 HTTP/1.1\r\nhost: feeds.example\r\n\r\n")
            .unwrap_err();
        assert!(
            why.contains("does not read") || why.contains("not a method"),
            "{why}"
        );
        let why = o.fetch(b"not http at all").unwrap_err();
        assert!(why.contains("does not read"), "{why}");
    }

    #[test]
    fn the_nodes_own_machine_and_network_are_never_reached() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.0.8",
            "172.16.5.5",
            "169.254.1.1",
            "0.0.0.0",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(own_network(&ip.parse().unwrap()), "{ip}");
        }
        for ip in ["93.184.216.34", "2606:2800:220:1:248:1893:25c8:1946"] {
            assert!(!own_network(&ip.parse().unwrap()), "{ip}");
        }
        assert_eq!(
            redirect_target("https://feeds.example/x/y?z"),
            Some((
                Destination::parse("https://feeds.example").unwrap(),
                "/x/y?z".into()
            ))
        );
        assert_eq!(redirect_target("http://feeds.example/"), None);
    }
}
