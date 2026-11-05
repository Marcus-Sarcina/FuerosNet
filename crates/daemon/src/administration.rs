//! **The operator's out-of-band surface** (`infra-client-requirements.md`
//! §8): the one channel between an operator's client and the instance they
//! run, carrying the things only their signature can make.
//!
//! **Out of band, and not protocol.** §8.2 settles that administration is
//! reached "the way any other server is reached", and that no request type
//! carries a command and none is owed one. So this is a surface of its own
//! on a port of its own, speaking HTTP/1.1 through the strict parser the
//! gateway already uses ([`rhtn_node::http`]), and nothing here appears on
//! the wire the protocol defines.
//!
//! **It does not read or speak for the node** (§8.1). What crosses inward
//! is an object the *operator* signed, which the node then publishes on its
//! own authority; nothing here composes or signs anything, and the node's
//! own discipline decides whether what arrives is taken.
//!
//! ## Two phases, one surface
//!
//! **Enrolling**, before any credential is in force: the instance has
//! minted a transport keypair nobody gave it
//! ([`crate::service::transport_credential`]) and is waiting for the run
//! its operator's client signs over the public half. It hands that half
//! out and takes the run, and the two records §4.4 says it cannot sign for
//! itself.
//!
//! **Serving**, afterwards, over the same port and the same routes. **This
//! surface outlives enrolment** [ruled, author, 2026-10-08], and an earlier
//! draft of this module closed it on the argument that a surface outliving
//! what it was for is a standing way in for nothing. That argument was
//! wrong about what it was for: §4.4 has an endpoint record re-signed
//! whenever the address set changes and an anchor entry whenever the
//! subtree size does, §7 has the run renewed before it lapses, and §8.3
//! has the node serve its own administration — **none of which happens
//! once**. An instance whose address moves and has no way to be handed a
//! new record "publishes nothing until [its operator is reachable]", which
//! is the consequence §4.4 says to plan for, and this is the plan.
//!
//! **The run is the commit point, so everything else goes first.**
//! `Credential::add` puts a run in force as it verifies it, which ends the
//! wait in [`crate::service::Service::start`] — and `start` then reads the
//! two records from the paths the configuration names, refusing a path it
//! cannot read. So a page that pushes the run before the records has the
//! node look for a file it has not sent yet. The fetch names what is still
//! wanted for exactly this reason.
//!
//! ## The two legs, and why only one of them needs a secret
//!
//! **Handing a signed thing in needs no authority of its own.** A run is
//! signed by the operator, and [`Credential::add`] verifies that it is the
//! operator's, over *this* transport key, and a delegation. The two records
//! carry the operator's signature over this node's own keyhash, checked
//! here as [`crate::service::Service::start`] checks them. So bytes from
//! any caller are either the operator's or refused, and **no bearer secret
//! guards these routes** — which is what keeps a long-lived surface from
//! being a long-lived credential.
//!
//! **Handing the public half out does.** An on-path answer carrying an
//! attacker's key would have the client sign a run over it, and the
//! attacker would then serve as the operator's node. The only thing a
//! client can share with a host that does not exist yet is what it wrote
//! into the instance's configuration, so the surface carries a **one-time
//! token** there — and the token is used as a MAC key and never sent: the
//! client chooses a nonce, and the answer carries the key with
//! `HMAC-SHA256(token, info || nonce || key)` beside it.
//!
//! **The token is not the transport key and must not be mistaken for one.**
//! Its whole power is to answer a fetch; the transport private key never
//! leaves the instance, which is what design §23.3 is for and what keeps
//! the credential §8.2 warns "can destroy the instance and bill its owner"
//! clear of the instance's identity.
//!
//! **What ordering is not this module's.** Whether an arriving record is
//! newer than the one held is the node's question and it already answers
//! it: `AnchorTable::offer` refuses "a seqno no newer than the one held",
//! and an originated endpoint record meets the store's supersession
//! discipline. So a record is written to disk **only where the node took
//! it**, and a replayed one changes nothing — neither the node's state nor
//! the file a restart would read.
//!
//! **The cryptography is here rather than in `rhtn-crypto`** on purpose:
//! that crate carries what the wire fixes, and this is administration which
//! no document gives an encoding. A second implementation of the node owes
//! nothing to the shape below.

use rhtn_crypto::Identity;
use rhtn_node::grant::{Clause, Grant};
use rhtn_node::http::{self, Reject};
use rhtn_node::resources::Manifest;
use rhtn_node::runtime::LiveNode;
use rhtn_node::store::{Decision, KIND_ENDPOINT_RECORD};
use rhtn_transport::tls::Credential;
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// **The port this surface conventionally takes: 7449/TCP.**
///
/// A convention and not a default: the configuration names the address,
/// because `Config` takes no policy by omission. What this is for is that a
/// provisioning page, a firewall rule and this crate's documentation agree
/// on one number. It sits beside the node's own 7447/UDP so an operator
/// opening one thinks of the other, it is TCP where that is UDP, and
/// nothing else in this project claims it [author, 2026-10-08].
pub const DEFAULT_PORT: u16 = 7449;

/// **The token acts are proved under at this start**: the replacement the
/// operator's client set, where a file holds one, and the configuration's
/// otherwise.
///
/// **Why a replacement at all** [author, 2026-11-02]: the configuration's
/// token arrived in the machine's provisioning data, which anything on the
/// machine can read back from the provider's metadata service — a hosted
/// package among them. So the client rotates it once the surface has
/// proved itself, and the standing secret is one that was never written
/// anywhere but here.
pub fn token_in_force(configured: [u8; 32], file: Option<&std::path::Path>) -> [u8; 32] {
    let Some(path) = file else {
        return configured;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return configured;
    };
    match bytes_of(text.trim()).and_then(|b| <[u8; 32]>::try_from(b).ok()) {
        Some(token) => {
            crate::say!(
                "rhtnd: the administration token in force is the one in {}",
                path.display()
            );
            token
        }
        // a file that is not a token is not a reason to refuse to serve,
        // and saying so is what an operator can act on
        None => {
            crate::say!(
                "rhtnd: {} does not hold 64 hex digits; the configuration's token stands",
                path.display()
            );
            configured
        }
    }
}

/// **0600 on a secret this instance keeps**: the token in force is what
/// authorises an act, and a file anybody on the machine may read is not a
/// secret.  Said rather than failed where the mode cannot be set, since
/// the token is already written and refusing afterwards would leave the
/// operator locked out.
#[cfg(unix)]
fn restrict(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
        crate::say!("rhtnd: {}: could not set 0600: {e}", path.display());
    }
}

#[cfg(not(unix))]
fn restrict(_: &std::path::Path) {}

/// Bytes out of lower-case hex, or nothing where it is not hex of whole
/// bytes: what a nonce and a proof arrive as.
fn bytes_of(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(s.get(i * 2..i * 2 + 2)?, 16).ok())
        .collect()
}

/// **How many nonces an instance remembers refusing a second time**: a
/// proof is worth one act, and a captured one is worth nothing once its
/// nonce is spent.  Bounded because an instance must not grow a set an
/// unauthenticated caller fills; the oldest goes, and a caller whose
/// nonce has fallen out of the set simply chooses another.
pub const NONCES_REMEMBERED: usize = 1024;

/// The domain separation over the proof, and the nonce's width: the
/// instance's end of a MAC whose other end is the operator's client, so
/// both read one definition (`rhtn_crypto::enrolment`).
pub use rhtn_crypto::enrolment::{ACT_INFO, NONCE_BYTES, PROOF_INFO, act_checks, proof};

/// The most one request may carry. A run's delegations are a few
/// hundred bytes each; this is room for a generous batch and no more, so a
/// caller cannot spend the instance's memory before it serves anything.
pub const MAX_REQUEST: usize = 64 * 1024;

/// How long one caller may hold the listener. Enrolment is a fetch and a
/// push; a connection that takes longer than this is not enrolling.
pub const CALLER_TIMEOUT: Duration = Duration::from_secs(10);

/// **How long an act that reaches this instance's host may take**
/// [author, 2026-11-02]: an install asks the host to pull an image over
/// the machine's own network, which is minutes where it is anything, and
/// is not a stalled caller.
///
/// It bounds the whole exchange, so it is the outer budget; the read is
/// still held to [`CALLER_TIMEOUT`] inside it, because a caller that
/// sends nothing is a stalled caller whatever it was going to ask for.
pub const PACKAGE_TIMEOUT: Duration = Duration::from_secs(300);

/// The surface an operator reaches their instance over: what it needs to
/// answer a fetch, to take a run, and to hand the node a record.
pub struct Surface {
    /// Where it listens, from the configuration's `[administration]`.
    pub listen: SocketAddr,
    /// **The token, used as a MAC key and never sent**: the instance
    /// proves it holds it when a client fetches, and a client proves it
    /// holds it on every act that does not arrive on the loopback.
    ///
    /// **It is replaced once the surface is trusted** [author,
    /// 2026-11-02]: the one in the configuration travelled in the
    /// machine's own provisioning data, which anything on the machine can
    /// read back, so the operator's client sets a fresh one over the
    /// surface at enrolment and that is the one acts are proved under.
    pub token: Arc<Mutex<[u8; 32]>>,
    /// **Where a replacement token is kept**, so a rotation survives a
    /// restart.  Absent, a rotation has nowhere to persist and refuses
    /// rather than holding a token the next start would forget — the same
    /// answer a grant act gives without a grants directory.
    pub token_file: Option<PathBuf>,
    /// The nonces this instance has taken an act under, newest last: a
    /// proof is worth one act, and this is what makes a second use of one
    /// worth nothing ([`NONCES_REMEMBERED`]).
    pub nonces: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
    /// The transport credential, for its public half and to verify a run
    /// into.
    pub credential: Arc<Credential>,
    /// The operator whose signature a run must carry.
    pub operator: Identity,
    /// Where a taken run is written, so it survives a restart: the same
    /// directory `Service::start`'s loop reads.
    pub delegations: PathBuf,
    /// Where this instance's operator-signed endpoint record goes, when the
    /// configuration names a path for one (`infra-client-requirements.md`
    /// §4.4).
    pub endpoint_record: Option<PathBuf>,
    /// Where its operator-signed anchor entry goes, likewise.
    pub anchor_entry: Option<PathBuf>,
    /// **The running node, once there is one.** `None` while enrolling,
    /// which is how the two phases tell themselves apart: with a node, a
    /// record taken is published at once; without one, it is kept for
    /// [`crate::service::Service::start`] to read, which it does after the
    /// loop that waits for the run.
    pub node: Arc<Mutex<Option<Running>>>,
    /// The hosting file and its limits, for a reload.
    pub hosting: Option<(PathBuf, rhtn_resources::Limits)>,
    /// **The surface its hosted instances reach their storage over**
    /// ([`crate::host`]): a package bound here is told where to reach its
    /// block and its log, and the secret that says which instance it is,
    /// exactly as one bound at start is.
    pub packages: std::sync::Arc<crate::host::HostSurface>,
    /// **Where this node keeps who may reach each resource**
    /// ([`crate::grants`]), which is what a grant act rewrites. Absent,
    /// an act has nowhere to persist and refuses rather than changing a
    /// table that would not survive a restart.
    pub grants: Option<PathBuf>,
    /// **Where this node keeps the rows its operator asserts about its
    /// horizon** ([`crate::horizon`]). Absent, an assertion has nowhere to
    /// persist and refuses rather than binding a member who would be gone
    /// at the next restart.
    pub horizon: Option<PathBuf>,
    /// Where this node keeps its packages' blocks and logs
    /// (`infra-client-requirements.md` §9.3), for a reload to provision
    /// against.
    pub state: Option<PathBuf>,
    /// **Where this instance's own host answers a shell**, where its
    /// operator gave it one: what an install asks to pull and start a
    /// package ([`crate::runner`]).  Absent, this node installs nothing
    /// and says so.
    pub host_shell: Option<SocketAddr>,
    /// **Where this surface actually bound**, which is not always what
    /// [`Surface::listen`] says: an operator naming port 0 has the system
    /// pick one, and the node's own line reports the one it got.  Kept so
    /// an act can refuse a package the port this surface is answering on.
    pub bound: Arc<Mutex<Option<SocketAddr>>>,
    /// Told when an act asks this node to stop; the service's own loop is
    /// waiting on it, so the act triggers the shutdown that exists rather
    /// than inventing one.
    pub stopping: Arc<tokio::sync::Notify>,
}

/// **The node once it is up, and the one fact about it the node itself
/// cannot be asked for**: whether this configuration bound a package, which
/// is one of the four things §8's exposure disclosure states.
///
/// The two travel together because they become true together, and a page
/// that had one without the other would be describing a node that is
/// serving with an exposure it cannot state.
pub struct Running {
    /// The node.
    pub node: Arc<LiveNode>,
    /// Whether the hosting file bound at least one package.
    pub hosts_resources: bool,
}

/// One of the two records an instance cannot sign for itself
/// (`infra-client-requirements.md` §4.4): the signature on each is its
/// operator's, so a change of either is something their client signs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signed {
    /// The endpoint record: where this node answers.
    Endpoint,
    /// The anchor entry: how large the subtree beneath it is.
    Anchor,
}

impl Signed {
    /// The configuration key naming where it is kept, so a refusal tells an
    /// operator which line is missing.
    fn key(self) -> &'static str {
        match self {
            Signed::Endpoint => "endpoint-record",
            Signed::Anchor => "anchor-entry",
        }
    }
}

impl Surface {
    /// The running node, where [`crate::service::Service::start`] has put
    /// one there.
    fn running(&self) -> Option<(Arc<LiveNode>, bool)> {
        self.node
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| (r.node.clone(), r.hosts_resources))
    }

    /// Serve until the caller drops this future.
    ///
    /// **One caller at a time.** Each exchange is short and an operator is
    /// one person, so there is nothing to gain from concurrency and a bound
    /// on it is one fewer thing for a stranger to spend.
    pub async fn serve(self) {
        let listener = match TcpListener::bind(self.listen).await {
            Ok(l) => l,
            Err(e) => {
                crate::say!("rhtnd: the operator's surface: {}: {e}", self.listen);
                return;
            }
        };
        // **the address it actually bound**, which an operator needs when
        // the configuration named port 0, as the node's own line does
        let bound = listener.local_addr().unwrap_or(self.listen);
        *self.bound.lock().unwrap() = Some(bound);
        tracing::info!(
            target: "daemon",
            step = "surface_open",
            listen = %bound,
            "daemon.lifecycle"
        );
        crate::say!(
            "rhtnd: the operator's surface on {bound}: fetch /node?nonce=<32 hex> for the \
             transport key, PUT the signed run to /node/run, and the operator-signed records \
             to /node/endpoint-record and /node/anchor-entry"
        );
        loop {
            let Ok((stream, from)) = listener.accept().await else {
                continue;
            };
            // a caller that stalls holds nothing: the exchange is bounded
            // and the next one is waiting.  **The outer bound is the
            // longer one** ([`PACKAGE_TIMEOUT`]), because an install
            // waits on the host's engine; the read inside it keeps the
            // short one, which is what a stalled caller meets.
            let _ = tokio::time::timeout(PACKAGE_TIMEOUT, self.exchange(stream, from)).await;
        }
        // the listener runs until it is dropped, which the service's own end
        // does to it
    }

    /// One connection: read a request, answer it, close.
    async fn exchange(&self, mut stream: TcpStream, from: SocketAddr) {
        let mut buf = Vec::new();
        let began = std::time::Instant::now();
        let req = loop {
            let mut chunk = [0u8; 4096];
            // **the message itself is held to the short budget**: the
            // long one above is for what answering costs, not for how
            // long a caller may take to ask
            let left = match CALLER_TIMEOUT.checked_sub(began.elapsed()) {
                Some(d) => d,
                None => {
                    let _ = answer(&mut stream, 408, "the request did not all arrive\n").await;
                    return;
                }
            };
            match tokio::time::timeout(left, stream.read(&mut chunk)).await {
                Err(_) => {
                    let _ = answer(&mut stream, 408, "the request did not all arrive\n").await;
                    return;
                }
                Ok(Ok(0)) => return,
                Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
                Ok(Err(_)) => return,
            }
            if buf.len() > MAX_REQUEST {
                let _ = answer(
                    &mut stream,
                    413,
                    "the request is larger than this surface takes\n",
                )
                .await;
                return;
            }
            match http::parse(&buf) {
                Ok(r) => break r,
                // the message has not all arrived: these two are the
                // parser saying so, and everything else is a refusal
                Err(Reject::Malformed("no end of headers"))
                | Err(Reject::Malformed("body short")) => {
                    continue;
                }
                Err(e) => {
                    tracing::warn!(
                        target: "daemon",
                        step = "surface_refused",
                        from = %from,
                        why = ?e,
                        "daemon.lifecycle"
                    );
                    let _ = answer(&mut stream, 400, "the request does not read\n").await;
                    return;
                }
            }
        };
        let (code, body) = self.answer_to(&req, Some(from));
        tracing::info!(
            target: "daemon",
            step = "surface_request",
            from = %from,
            method = %req.method,
            target = %req.target,
            code,
            "daemon.lifecycle"
        );
        let _ = answer(&mut stream, code, &body).await;
    }

    /// What a request comes to: the status and the body, both decided
    /// without touching the socket so a test can hold this alone.
    ///
    /// `from` is where the request arrived from, which is half of what
    /// authorises it ([`Surface::authorised`]); `None` is a caller in this
    /// process, which is a test or the service itself.
    pub fn answer_to(&self, req: &http::Request, from: Option<SocketAddr>) -> (u16, String) {
        let (path, query) = match req.target.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (req.target.as_str(), None),
        };
        if let Err(refusal) = self.authorised(req, path, query, from) {
            return refusal;
        }
        match (req.method.as_str(), path) {
            ("GET", "/") | ("GET", "/index.html") => self.page(query),
            ("GET", "/node") => self.fetch(query),
            ("POST", "/node/acknowledge") => self.acknowledge(&req.body),
            // **two routes, kept apart from any peer's** [author,
            // 2026-11-04: "separate routes"]: what arrives here is the
            // operator's own, authorised by the act proof this surface
            // demands, and needs no downline test. A row asserted by
            // another node is a different authority and will have its own
            // route, gated by `Table::may_assert`.
            ("POST", "/node/horizon") => self.horizon_assert(&req.body),
            ("POST", "/node/horizon/drop") => self.horizon_retract(&req.body),
            ("POST", "/node/grant") => self.grant_add(&req.body),
            ("POST", "/node/grant/narrow") => self.grant_narrow(&req.body),
            ("POST", "/node/grant/drop") => self.grant_drop(&req.body),
            ("POST", "/node/grant/template") => self.grant_template(&req.body),
            ("POST", "/node/token") => self.rotate(&req.body),
            ("GET", "/node/acts") => self.declared(),
            ("POST", p) if p.starts_with("/node/act/") => {
                self.invoke(p.trim_start_matches("/node/act/"), &req.body)
            }
            ("GET", "/node/packages") => self.package_list(),
            ("POST", "/node/package/install") => self.package_install(&req.body),
            ("POST", "/node/package/remove") => self.package_remove(&req.body),
            ("POST", "/node/package/running") => self.package_said(&req.body, false),
            ("POST", "/node/package/logs") => self.package_said(&req.body, true),
            ("POST", "/node/reload") => self.reload(),
            ("POST", "/node/stop") => self.stop(),
            ("PUT", "/node/run") | ("POST", "/node/run") => self.take_run(&req.body),
            ("PUT", "/node/endpoint-record") | ("POST", "/node/endpoint-record") => {
                self.take_signed(Signed::Endpoint, &req.body)
            }
            (m, p) if p.starts_with("/node/resource/") => self.resource_act(m, p, &req.body),
            ("PUT", "/node/anchor-entry") | ("POST", "/node/anchor-entry") => {
                self.take_signed(Signed::Anchor, &req.body)
            }
            ("GET", _) | ("PUT", _) | ("POST", _) => (
                404,
                "this surface answers GET /, GET /node, GET /node/acts, \
                 POST /node/act/<declared act>, GET /node/packages, \
                 PUT /node/{run,endpoint-record,anchor-entry}, \
                 POST /node/{acknowledge,reload,stop,token}, \
                 POST /node/grant[/narrow|/drop|/template], \
                 POST /node/package/{install,remove,running,logs}, and for a hosted instance \
                 GET /node/resource/<hex>/federation, GET /node/resource/<hex>/log, \
                 PUT /node/resource/<hex>/envelope and POST /node/resource/<hex>/federate\n"
                    .into(),
            ),
            _ => (405, "this surface answers GET and PUT\n".into()),
        }
    }

    /// **The administration page this node serves its own operator**
    /// (`infra-client-requirements.md` §8.3), rendered by
    /// [`crate::operator::page`].
    ///
    /// **Before the node is up there is no state to show**, so what the
    /// page says then is what the surface is waiting for. An operator who
    /// opens this on a fresh instance should read why it is not serving
    /// rather than an empty table.
    fn page(&self, query: Option<&str>) -> (u16, String) {
        let Some((node, hosts_resources)) = self.running() else {
            let held = |p: &Option<PathBuf>| match p {
                None => "not configured",
                Some(p) if p.exists() => "held",
                Some(_) => "WANTED",
            };
            return (
                200,
                crate::operator::waiting_page(
                    &hex(&self.credential.public()),
                    self.credential.issued().len(),
                    held(&self.endpoint_record),
                    held(&self.anchor_entry),
                ),
            );
        };
        let status = crate::operator::StatusView::of(
            &node,
            Some(&self.credential),
            self.endpoint_record.as_deref(),
            self.anchor_entry.as_deref(),
        );
        let exposure = crate::operator::ExposureView::new(
            status.subordinates,
            true,
            hosts_resources,
            status.patron.is_some(),
        );
        let resources = resource_views(&node);
        let showing = query
            .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("resource=")))
            .and_then(unhex)
            .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok());
        (
            200,
            crate::operator::page(&status, &exposure, &resources, showing.as_ref()),
        )
    }

    /// **What authorises an act** [author, 2026-10-08, narrowed
    /// 2026-11-02]: arriving on the loopback, or a proof under this
    /// instance's token.
    ///
    /// §8.2 puts administration "out of band, with everything else about
    /// the host", and where the surface binds was the whole access
    /// decision. **That holds on the loopback and nowhere else**: a
    /// rented instance binds this surface on every interface it has, so
    /// reaching the port there is no evidence of anything — anyone could
    /// stop the node or grant themselves a role on a hosted resource.
    ///
    /// So an act that did not arrive on the loopback carries `nonce` and
    /// `proof` in its query, and the proof is
    /// `HMAC-SHA256` under the token over the method, the target without
    /// the proof itself, and the body
    /// (`rhtn_crypto::enrolment::act_proof`). **The token is still never
    /// sent**, an act still carries no signature, and a desktop holding
    /// no seed can still administer — which is what the original rule was
    /// for.
    ///
    /// **The fetch is the one exception**: it is the bootstrap, where the
    /// client has not yet been told anything by this instance and the
    /// instance proves itself to the client. It discloses the transport
    /// key it minted, which is public, and the proof over it.
    fn authorised(
        &self,
        req: &http::Request,
        path: &str,
        query: Option<&str>,
        from: Option<SocketAddr>,
    ) -> Result<(), (u16, String)> {
        // the bootstrap, and a caller inside this process
        if path == "/node" && req.method == "GET" {
            return Ok(());
        }
        // **the three routes that carry a signature authorise
        // themselves**, which this module's own heading has settled since
        // 2026-10-08 and the narrowing above briefly forgot [2026-11-02].
        //
        // A run is verified to be the operator's over *this* transport
        // key; each record is verified to be the operator's over this
        // node's own keyhash. Bytes from any caller are therefore either
        // the operator's or refused, so **a proof would add nothing** —
        // and requiring one broke the enrolment it was meant to protect:
        // these three routes *are* the exchange that first puts a
        // credential on a launched instance, and a client that cannot
        // complete it has no instance to administer at all.
        if matches!(req.method.as_str(), "PUT" | "POST")
            && matches!(
                path,
                "/node/run" | "/node/endpoint-record" | "/node/anchor-entry"
            )
        {
            return Ok(());
        }
        match from {
            None => return Ok(()),
            Some(a) if a.ip().is_loopback() => return Ok(()),
            Some(_) => {}
        }
        let refused = |why: &str| {
            Err((
                401,
                format!(
                    "this act is not authorised: {why}. An act that does not arrive on the \
                     loopback carries nonce and proof in its query, the proof being \
                     HMAC-SHA256 under this instance's token over the method, the target \
                     without the proof, and the body\n"
                ),
            ))
        };
        let q = query.unwrap_or("");
        let field = |name: &str| -> Option<String> {
            q.split('&')
                .find_map(|p| p.strip_prefix(&format!("{name}=")))
                .map(str::to_string)
        };
        let (Some(nonce_hex), Some(proof_hex)) = (field("nonce"), field("proof")) else {
            return refused("it carries no nonce and proof");
        };
        let Some(nonce) = bytes_of(&nonce_hex) else {
            return refused("the nonce is not hex");
        };
        if nonce.len() != NONCE_BYTES {
            return refused("the nonce is not 16 bytes");
        }
        let Some(shown) = bytes_of(&proof_hex) else {
            return refused("the proof is not hex");
        };
        // **the target as it was, less the proof**: everything else a
        // caller could change is inside what the proof covers
        let signed: String = {
            let kept: Vec<&str> = q.split('&').filter(|p| !p.starts_with("proof=")).collect();
            if kept.is_empty() {
                path.to_string()
            } else {
                format!("{path}?{}", kept.join("&"))
            }
        };
        if !act_checks(
            &self.token.lock().unwrap(),
            &nonce,
            &req.method,
            &signed,
            &req.body,
            &shown,
        ) {
            return refused("the proof is not this token's over this act");
        }
        // **a nonce is spent by the act it authorised**, so a captured
        // proof is worth nothing afterwards
        let mut seen = self.nonces.lock().unwrap();
        if seen.contains(&nonce) {
            return refused("that nonce has been used");
        }
        if seen.len() >= NONCES_REMEMBERED {
            seen.pop_front();
        }
        seen.push_back(nonce);
        Ok(())
    }

    /// **A management act carries no key**, which is what lets an operator
    /// administer from a desktop holding no seed (design §23.3).
    ///
    /// **None of these is what §8.1 and OPS-011 forbid**: no frame is
    /// composed, nothing is replayed, no signature is made and no packet
    /// is approved by hand. They are OPS-012's "explicit management acts".
    fn acting(&self) -> Result<Arc<LiveNode>, (u16, String)> {
        self.running()
            .map(|(n, _)| n)
            .ok_or((409, "this node is not serving yet\n".into()))
    }

    /// **Replace the token acts are proved under** [author, 2026-11-02],
    /// which the operator's client does once at enrolment: the one the
    /// configuration carried travelled in the machine's provisioning
    /// data, and anything on the machine can read that back from the
    /// provider's metadata service — a hosted package among them. The
    /// replacement was never written anywhere but the file below.
    ///
    /// **Authorised as every act is**: on the loopback by arriving there,
    /// and otherwise by a proof under the token in force, which is the
    /// one being replaced. So a caller who cannot prove the old token
    /// cannot set a new one.
    ///
    /// **Refused where there is nowhere to keep it**, as a grant act is
    /// without a grants directory: a token held only in memory would be
    /// forgotten at the next start, and an operator would be locked out
    /// of their own instance with no way to tell why.
    fn rotate(&self, body: &[u8]) -> (u16, String) {
        let Some(path) = self.token_file.clone() else {
            return (
                409,
                "this instance names no `[administration] token-file`, so a replacement token \
                 has nowhere to survive a restart\n"
                    .into(),
            );
        };
        let f = form(body);
        let given = field(&f, "token");
        let Some(fresh) = bytes_of(given).and_then(|b| <[u8; 32]>::try_from(b).ok()) else {
            return (400, "a token is 64 hex digits\n".into());
        };
        if fresh == *self.token.lock().unwrap() {
            return (400, "that is the token already in force\n".into());
        }
        // **the file first, then the memory**: a token in force that the
        // next start would not find is the one state worth refusing to
        // hold
        if let Some(dir) = path.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            return (500, format!("{}: {e}\n", dir.display()));
        }
        if let Err(e) = std::fs::write(&path, format!("{}\n", hex(&fresh))) {
            return (500, format!("{}: {e}\n", path.display()));
        }
        restrict(&path);
        *self.token.lock().unwrap() = fresh;
        // every proof made under the old token is worth nothing now, and
        // the nonces it spent are of no further interest
        self.nonces.lock().unwrap().clear();
        tracing::info!(target: "daemon", step = "token_rotated", "daemon.lifecycle");
        (
            200,
            format!(
                "the token in force is the one you set, kept in {}\n",
                path.display()
            ),
        )
    }

    /// **What this instance declares it can be asked for**
    /// ([`crate::acts`]), which is what a client draws its own screens
    /// from.
    ///
    /// **It describes this instant.** The acknowledgement policy decides
    /// a label, the grants that stand decide which grant may be named,
    /// and a package's declared operations travel beside the node's own —
    /// so a client that asks again after an act draws the page as it is
    /// rather than as it was built.
    fn declared(&self) -> (u16, String) {
        let Some((node, _)) = self.running() else {
            return (
                409,
                "this node is not serving yet, so it declares nothing it could be asked for\n"
                    .into(),
            );
        };
        let installed: Vec<[u8; 32]> = self
            .hosting
            .as_ref()
            .and_then(|(p, _)| crate::packages::read(p).ok())
            .map(|ps| ps.iter().map(|p| p.resource).collect())
            .unwrap_or_default();
        let acknowledging = node.view.lock().unwrap().ack_policy.is_some();
        let views: Vec<(crate::operator::ResourceView, bool)> = resource_views(&node)
            .into_iter()
            .map(|v| {
                let mine = installed.contains(&v.resource);
                (v, mine)
            })
            .collect();
        (
            200,
            format!("{}\n", crate::acts::declaration(acknowledging, &views)),
        )
    }

    /// **Invoke one declared act by name**, which is the one route a
    /// client needs beyond the declaration itself.
    ///
    /// **The declared parameter names are the field names the act
    /// reads**, so an invocation body is the body the implementing route
    /// already takes and nothing is translated here. What this adds is
    /// the name: a client that knows `/node/act/<name>` and the
    /// declaration knows the whole surface, and the routes beneath are
    /// the node's own business — which is what §8.3 means by a surface
    /// that *"matches its software"*.
    ///
    /// **An unknown name is a 404 that names the declaration**, because a
    /// client meeting one is a client built against another version and
    /// the thing it needs is to go and read.
    fn invoke(&self, name: &str, body: &[u8]) -> (u16, String) {
        match name {
            "acknowledge" => self.acknowledge(body),
            "reload" => self.reload(),
            "stop" => self.stop(),
            "grant" => self.grant_add(body),
            "grant-narrow" => self.grant_narrow(body),
            "grant-drop" => self.grant_drop(body),
            "grant-template" => self.grant_template(body),
            "install" => self.package_install(body),
            "remove" => self.package_remove(body),
            "running" => self.package_said(body, false),
            "logs" => self.package_said(body, true),
            other => (
                404,
                format!(
                    "`{other}` is not an act this instance declares. GET /node/acts is the \
                     list, and `{SURFACE}` is the vocabulary it is written in\n",
                    SURFACE = crate::acts::SURFACE
                ),
            ),
        }
    }

    /// **The host's shell, where this instance's operator gave it one.**
    fn shell(&self) -> Result<crate::runner::Shell, (u16, String)> {
        let Some(at) = self.host_shell else {
            return Err((
                409,
                "this instance names no `[administration] host-shell`, so it cannot ask its own \
                 host to start anything: it installs nothing, and says so rather than failing \
                 an install for a reason it invented\n"
                    .into(),
            ));
        };
        crate::runner::Shell::of(at).map_err(|why| (409, format!("{why}\n")))
    }

    /// **Install a package on this instance's own host** [author,
    /// 2026-11-02], which is the act an operator with no shell makes
    /// instead of having one.
    ///
    /// ## What it is, and what each party contributes
    ///
    /// Installing a package is five things
    /// (`infra-client-requirements.md` §9, §9.4): the image on the
    /// machine, the package's manifest where the node reads it, an entry
    /// in the hosting file, the package running on a port of its own, and
    /// the node having re-read both. **Nothing of it is a protocol act
    /// and nothing of it touches the operator's seed.**
    ///
    /// - **The client brings the key**, the restricted one derived from
    ///   its operator's signing seed (`rhtn_crypto::ssh::host_runner_key`),
    ///   for this act and no other: the node is handed it, uses it, and
    ///   keeps nothing ([`crate::runner`]).
    /// - **The client brings the package's manifest**, which is the
    ///   package's own declaration relayed rather than the operator's
    ///   statement about it — an operator writing role names into their
    ///   node's configuration would be declaring them on the package's
    ///   behalf (§9.1, [`crate::hosting`]).
    /// - **The node brings the record**, so the entry it generates
    ///   survives a restart and a removal names what an install made
    ///   ([`crate::packages`]).
    /// - **The host brings the engine**, and decides what it will be
    ///   asked for: the runner's vocabulary and nothing else.
    ///
    /// ## Both or neither
    ///
    /// The image is pulled before anything is written, so a reference the
    /// machine cannot fetch changes nothing. The node re-reads its
    /// configuration before the package is started, so a manifest it
    /// refuses leaves nothing running. **And either failing puts the
    /// record and the hosting file back as they were and re-reads them**,
    /// which is the discipline a grant act already follows: what is in
    /// force is what is on disk.
    ///
    /// ## Who may reach it afterwards: nobody, yet
    ///
    /// No grant is written. §7 has a package's roles start bound to
    /// nobody, and that is the state a package just installed should be
    /// in — the operator decides who may reach it as a second, explicit
    /// act. The answer says so rather than leaving them to discover it.
    fn package_install(&self, body: &[u8]) -> (u16, String) {
        if self.acting().is_err() {
            return (409, "this node is not serving yet\n".into());
        }
        let Some((hosting, _)) = self.hosting.clone() else {
            return (
                409,
                "the configuration names no `resources` file, so there is nowhere to write down \
                 what this node hosts\n"
                    .into(),
            );
        };
        let shell = match self.shell() {
            Ok(s) => s,
            Err(e) => return e,
        };
        let f = form(body);
        let key = match runner_key(&f) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let want = match asked(&f) {
            Ok(w) => w,
            Err(why) => return (400, format!("{why}\n")),
        };
        let manifest = field(&f, "manifest");
        if manifest.trim().is_empty() {
            return (
                400,
                "an install carries the package's own manifest as `manifest`: what a package \
                 declares is the package's, and this node writes it where it reads it rather \
                 than composing one on the package's behalf\n"
                    .into(),
            );
        }
        // **what this node already installed**, which is what the entry
        // is generated from and what a failure is put back to
        let was = match crate::packages::read(&hosting) {
            Ok(w) => w,
            Err(why) => return (500, format!("{why}\n")),
        };
        if let Some(other) = was
            .iter()
            .find(|p| p.port == want.port && p.resource != want.resource)
        {
            return (
                400,
                format!(
                    "`{}` already listens on 127.0.0.1:{}: two resources at one address is one \
                     resource answering for both\n",
                    hex(&other.resource),
                    want.port
                ),
            );
        }
        let mine = self
            .bound
            .lock()
            .unwrap()
            .map_or(self.listen.port(), |a| a.port());
        if want.port == mine {
            return (
                400,
                format!("{mine} is where this instance's own administration surface answers\n"),
            );
        }

        // **the image first**: a reference the machine cannot fetch
        // changes nothing on it
        let pulled = match shell.ask(&key, &["pull", &want.image]) {
            Ok(said) => said,
            Err(why) => return (502, format!("the image was not pulled: {why}\n")),
        };

        let manifest_at = crate::packages::manifest_at(&hosting, &want.resource);
        let held_manifest = std::fs::read(&manifest_at).ok();
        let mut now: Vec<crate::packages::Package> = was
            .iter()
            .filter(|p| p.resource != want.resource)
            .cloned()
            .collect();
        now.push(want.clone());
        let name = crate::packages::Package::name(&want.resource);

        // what putting it back means, wherever this goes wrong after here
        let undo = || {
            match &held_manifest {
                Some(bytes) => {
                    let _ = std::fs::write(&manifest_at, bytes);
                }
                None => {
                    let _ = std::fs::remove_file(&manifest_at);
                }
            }
            let _ = crate::packages::write(&hosting, &was);
            let _ = self.reload();
        };

        if let Some(dir) = manifest_at.parent()
            && let Err(e) = std::fs::create_dir_all(dir)
        {
            return (500, format!("{}: {e}\n", dir.display()));
        }
        if let Err(e) = std::fs::write(&manifest_at, manifest) {
            return (500, format!("{}: {e}\n", manifest_at.display()));
        }
        if let Err(why) = crate::packages::write(&hosting, &now) {
            undo();
            return (500, format!("{why}; nothing changed\n"));
        }
        // **the node re-reads before the package is started**: a manifest
        // this host will not admit leaves nothing running
        let (code, said) = self.reload();
        if code != 200 {
            undo();
            return (
                400,
                format!("this node did not take the package: {said}\nnothing changed\n"),
            );
        }
        // and now the package itself, under a name derived from the
        // resource so a second install replaces it
        let mut request: Vec<String> = vec!["start".into(), name.clone(), want.image.clone()];
        request.extend(want.arguments());
        let words: Vec<&str> = request.iter().map(String::as_str).collect();
        let started = match shell.ask(&key, &words) {
            Ok(s) => s,
            Err(why) => {
                undo();
                return (
                    502,
                    format!("the host did not start it: {why}\nnothing changed\n"),
                );
            }
        };
        tracing::info!(
            target: "daemon",
            step = "surface_act",
            act = "install",
            resource = %hex(&want.resource),
            image = %want.image,
            port = want.port,
            "daemon.lifecycle"
        );
        (
            200,
            format!(
                "installed {}\nname rhtn-pkg-{name}\nimage {}\naddress 127.0.0.1:{}\nauthority \
                 {}\nowner {}\ngrants none\nnext nobody may reach it until a role is granted on \
                 it\npulled {}\nstarted {}\n",
                hex(&want.resource),
                want.image,
                want.port,
                want.authority,
                hex(&want.owner),
                pulled.trim(),
                started.trim(),
            ),
        )
    }

    /// **Remove a package this node installed**: the container stopped,
    /// the entry dropped, the configuration re-read.
    ///
    /// **A container already gone is not a failure.** An operator
    /// removing a package whose process died, or whose machine was
    /// rebuilt, is removing it from the node's configuration, which is
    /// the part this act is about; what the host said is reported rather
    /// than refused.
    ///
    /// **What is kept**: the grants, the storage block, the log and the
    /// jobs. A removal is how an operator upgrades — it is a removal and
    /// an install of the same resource — and an upgrade that silently
    /// dropped who may reach the package, or its data, would be the worse
    /// surprise. The answer names what was left behind.
    fn package_remove(&self, body: &[u8]) -> (u16, String) {
        if self.acting().is_err() {
            return (409, "this node is not serving yet\n".into());
        }
        let Some((hosting, _)) = self.hosting.clone() else {
            return (409, "the configuration names no `resources` file\n".into());
        };
        let shell = match self.shell() {
            Ok(s) => s,
            Err(e) => return e,
        };
        let f = form(body);
        let key = match runner_key(&f) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let Some(resource) = unhex(field(&f, "resource"))
            .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
            .map(|k: [u8; 32]| k)
        else {
            return (400, "a resource is 64 lower-case hex digits\n".into());
        };
        let was = match crate::packages::read(&hosting) {
            Ok(w) => w,
            Err(why) => return (500, format!("{why}\n")),
        };
        if !was.iter().any(|p| p.resource == resource) {
            return (
                404,
                "this node installed no such package through its surface. A resource its \
                 operator wrote into the hosting file by hand is theirs to remove there\n"
                    .into(),
            );
        }
        let name = crate::packages::Package::name(&resource);
        let stopped = match shell.ask(&key, &["stop", &name]) {
            Ok(said) => format!("stopped {}", said.trim()),
            Err(why) => format!("the host said: {why}"),
        };
        let now: Vec<crate::packages::Package> = was
            .iter()
            .filter(|p| p.resource != resource)
            .cloned()
            .collect();
        if let Err(why) = crate::packages::write(&hosting, &now) {
            return (500, format!("{why}; nothing changed\n"));
        }
        let (code, said) = self.reload();
        if code != 200 {
            let _ = crate::packages::write(&hosting, &was);
            let _ = self.reload();
            return (
                400,
                format!("this node did not re-read its configuration: {said}\nnothing changed\n"),
            );
        }
        let _ = std::fs::remove_file(crate::packages::manifest_at(&hosting, &resource));
        tracing::info!(
            target: "daemon",
            step = "surface_act",
            act = "remove",
            resource = %hex(&resource),
            "daemon.lifecycle"
        );
        (
            200,
            format!(
                "removed {}\n{stopped}\nkept the grants, the storage block, the log and the \
                 jobs: a removal is also how a package is upgraded\n",
                hex(&resource)
            ),
        )
    }

    /// **What this node installed through its own surface**, as its record
    /// holds it: what the operator's client reads to draw them.
    ///
    /// It reads the record and does not ask the host, so it needs no key.
    /// What is *running* is the host's answer to a `list`, which the act
    /// below carries a key for.
    fn package_list(&self) -> (u16, String) {
        let Some((hosting, _)) = self.hosting.clone() else {
            return (409, "the configuration names no `resources` file\n".into());
        };
        match crate::packages::read(&hosting) {
            Err(why) => (500, format!("{why}\n")),
            Ok(ps) => {
                let mut out = format!("packages {}\n", ps.len());
                for p in &ps {
                    out.push_str(&format!(
                        "package {} rhtn-pkg-{} {} 127.0.0.1:{} {}\n",
                        hex(&p.resource),
                        crate::packages::Package::name(&p.resource),
                        p.image,
                        p.port,
                        p.authority
                    ));
                }
                (200, out)
            }
        }
    }

    /// **What the host says about the packages it is running**, and what
    /// one of them wrote: the runner's `list` and `logs`, carried through
    /// so an operator with no shell can read them.
    ///
    /// This is the package's *process* output, as the engine kept it. The
    /// instance's own log — what it wrote through the host binding §9.3
    /// gives it — is `GET /node/resource/<hex>/log`, which is a different
    /// thing and needs no key.
    fn package_said(&self, body: &[u8], logs: bool) -> (u16, String) {
        let shell = match self.shell() {
            Ok(s) => s,
            Err(e) => return e,
        };
        let f = form(body);
        let key = match runner_key(&f) {
            Ok(k) => k,
            Err(e) => return e,
        };
        let request: Vec<String> = if logs {
            let Some(resource) = unhex(field(&f, "resource"))
                .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
                .map(|k: [u8; 32]| k)
            else {
                return (400, "a resource is 64 lower-case hex digits\n".into());
            };
            let lines = match field(&f, "lines") {
                "" => "40".to_string(),
                n => match n.parse::<usize>() {
                    Ok(n) if n <= 500 => n.to_string(),
                    _ => return (400, "a line count is a number, at most 500\n".into()),
                },
            };
            vec![
                "logs".into(),
                crate::packages::Package::name(&resource),
                lines,
            ]
        } else {
            vec!["list".into()]
        };
        let words: Vec<&str> = request.iter().map(String::as_str).collect();
        match shell.ask(&key, &words) {
            Ok(said) => (200, said),
            Err(why) => (502, format!("{why}\n")),
        }
    }

    /// The standing acknowledgement policy (design §11.2.1): whether every
    /// adoption beneath this node is countersigned as it is stored.
    fn acknowledge(&self, body: &[u8]) -> (u16, String) {
        let Ok(node) = self.acting() else {
            return (409, "this node is not serving yet\n".into());
        };
        let want = String::from_utf8_lossy(body);
        let on = match want.split('=').nth(1).unwrap_or("").trim() {
            "on" | "true" | "yes" => true,
            "off" | "false" | "no" => false,
            _ => return (400, "acknowledge is on or off\n".into()),
        };
        // the same policy `Service::start` installs from the
        // configuration's `acknowledge`: every adoption beneath this node
        node.view.lock().unwrap().ack_policy =
            on.then(|| -> rhtn_archive::topology::AckPolicy { Arc::new(|_, _| true) });
        tracing::info!(
            target: "daemon",
            step = "surface_act",
            act = "acknowledge",
            on,
            "daemon.lifecycle"
        );
        // **the act is not written back to the configuration**, which is a
        // file its operator owns: a daemon that rewrote it would be
        // deciding what their next start says. The page states what is in
        // force; the configuration states what a restart will choose.
        seen(format!(
            "acknowledgement is now {}; the configuration on disk is unchanged, so a restart \
             takes what it says\n",
            if on { "on" } else { "off" }
        ))
    }

    /// **Change one resource's grants**, persist them, and expand.
    ///
    /// OPS-012 has an operator's predicates and standing policies kept as
    /// **explicit management acts**, and this is one. Unlike the
    /// acknowledgement policy above it is written back: a grant is a table
    /// update [ruled, author, 2026-10-09], and an act that did not survive
    /// a restart would not be an act — a revocation that came back on the
    /// next start is worse than one that never happened, because the
    /// operator watched it work.
    ///
    /// **Both or neither.** The table is changed first, because it is the
    /// authority on what a grant may say; if the file cannot be written
    /// the table is put back and the act fails, so the two never disagree
    /// about who may reach this resource.
    fn amend(
        &self,
        resource: &str,
        change: impl FnOnce(&mut Vec<Grant>, &Manifest) -> Result<String, String>,
    ) -> (u16, String) {
        let Ok(node) = self.acting() else {
            return (409, "this node is not serving yet\n".into());
        };
        let Some(dir) = self.grants.clone() else {
            return (
                409,
                "the configuration names no `grants` directory, so a grant could not survive a \
                 restart and is refused rather than held until one\n"
                    .into(),
            );
        };
        let Some(res) = unhex(resource)
            .and_then(|v| <[u8; 32]>::try_from(v).ok())
            .map(|k: [u8; 32]| k)
        else {
            return (400, "a resource is 64 lower-case hex digits\n".into());
        };
        let mut view = node.view.lock().unwrap();
        let Some(declared) = view.resources.binding(&res).map(|b| b.declared.clone()) else {
            return (
                404,
                "no resource is bound here under that identity\n".into(),
            );
        };
        let was: Vec<Grant> = view.resources.grants_for(&res).to_vec();
        let mut now = was.clone();
        let said = match change(&mut now, &declared) {
            Ok(s) => s,
            Err(e) => return (400, format!("{e}\n")),
        };
        if let Err(e) = view.resources.set_grants(res, now.clone()) {
            return (400, format!("{e}\n"));
        }
        if let Err(e) = crate::grants::write_for(&dir, &res, &now) {
            // put the table back, so what is in force is what is on disk
            let _ = view.resources.set_grants(res, was);
            view.expand_grants();
            return (500, format!("{e}; nothing changed\n"));
        }
        // §10.2's first moment: the operator has just configured roles
        let (granted, dropped) = view.expand_grants();
        drop(view);
        tracing::info!(
            target: "daemon",
            step = "surface_act",
            act = "grant",
            granted,
            dropped,
            "daemon.lifecycle"
        );
        seen(format!(
            "{said}. {granted} row(s) written and {dropped} dropped, and the node's own grants \
             table is written, so a restart takes it."
        ))
    }

    /// **Assert one row of this node's horizon** — the insert and the
    /// update of the operator's own CRUD interface onto the table
    /// (`Robot/deferred-decisions.md`, the horizon as a table its operator
    /// maintains).
    ///
    /// **It carries no reason and none is asked for** [author,
    /// 2026-11-04]: this node runs under its operator's authority and is
    /// not being persuaded of anything. What authorises the act is the
    /// channel — the act proof every route here demands, or arrival on the
    /// loopback — and nothing in the body.
    ///
    /// **The material is not optional.** A keyhash is taken over the
    /// classical and post-quantum keys together and the transport presents
    /// only the classical one, so `Pins::keyhash_for_key` resolves a
    /// connection by comparing against material. A row without it names a
    /// member this node could never recognise, which is the
    /// `Refusal::Unbound` the bench met on 2026-11-03.
    fn horizon_assert(&self, body: &[u8]) -> (u16, String) {
        let f = form(body);
        let Ok(node) = self.acting() else {
            return (409, "this node is not serving yet\n".into());
        };
        let Some(dir) = self.horizon.clone() else {
            return (
                409,
                "the configuration names no `horizon` directory, so a row could not survive a \
                 restart and is refused rather than binding a member who would be gone at the \
                 next start\n"
                    .into(),
            );
        };
        let (Some(subject), Some(patron)) = (key32(field(&f, "node")), key32(field(&f, "patron")))
        else {
            return (
                400,
                "`node` and `patron` are 64 lower-case hex digits\n".into(),
            );
        };
        let Ok(series) = field(&f, "series").parse::<u32>() else {
            return (400, "`series` is a number\n".into());
        };
        let Ok(revision) = field(&f, "revision").parse::<u64>() else {
            return (
                400,
                "`revision` is a number, and is this client's own counter for the row: a second \
                 device posting an older one changes nothing\n"
                    .into(),
            );
        };
        let Some(material) = unhex(field(&f, "material")) else {
            return (
                400,
                "`material` is the member's key material, in hex\n".into(),
            );
        };
        if rhtn_crypto::Identity::from_key_material(&material)
            .is_none_or(|id| id.keyhash != subject)
        {
            return (
                400,
                "that material is not a KeyMaterial array whose keyhash is `node`\n".into(),
            );
        }
        let row = rhtn_archive::topology::Asserted {
            node: subject,
            patron,
            series,
            adoption: key32(field(&f, "adoption")).unwrap_or([0; 32]),
            from: field(&f, "from").parse::<u64>().unwrap_or(0),
            anchor: key32(field(&f, "anchor")),
            slot: field(&f, "slot").parse::<u8>().ok(),
            revision,
        };
        // **the disk first, then the table.** A row bound in memory and not
        // written is a member who attaches until the next restart, which is
        // the failure `grants` refuses for the same reason.
        if let Err(e) = rhtn_node::horizon::write(
            &dir,
            &rhtn_node::horizon::Row {
                row: row.clone(),
                material: Some(material.clone()),
                retracted: false,
            },
        ) {
            return (500, format!("{e}; nothing changed\n"));
        }
        let outcome = {
            let mut view = node.view.lock().unwrap();
            view.table.assert(&row)
        };
        // **pinned, and known.** The pin is what lets the transport resolve
        // the key this member presents; `ids` is what lets the node verify
        // what that member signs. A row that gave only one of the two binds
        // a member whose records this node could not check.
        let pinned = node.node.cfg.pins.pin(subject, &material).is_ok();
        if let Some(id) = rhtn_crypto::Identity::from_key_material(&material) {
            let mut ids = node.ids.lock().unwrap();
            if !ids.iter().any(|k| k.keyhash == subject) {
                ids.push(id);
            }
        }
        tracing::info!(
            target: "daemon",
            step = "surface_act",
            act = "horizon-assert",
            node8 = %rhtn_node::diag::id8(&subject),
            outcome = ?outcome,
            pinned,
            "daemon.lifecycle"
        );
        seen(format!(
            "{outcome:?}: {} under {} at series {series}, revision {revision}. The row is written, \
             so a restart takes it{}.",
            rhtn_node::diag::id8(&subject),
            rhtn_node::diag::id8(&patron),
            if pinned {
                ""
            } else {
                ", but its material could not be pinned, so it cannot attach until that is fixed"
            }
        ))
    }

    /// **Retract one row** — the delete of the same interface.
    ///
    /// **Any row, not only one this node was told.** A binding a
    /// transaction made is ordinarily ended by one, but [author,
    /// 2026-11-04] *"there is no guarantee that the inverse transaction
    /// will reach the node, since transactions change topology and
    /// networks have outages"* — so an operator whose node never received
    /// a departure can still say the member has gone.
    ///
    /// **It takes the row's identity and its revision, and nothing else**,
    /// so it is the one act here that a node of any version can honour:
    /// a retraction that depended on the row's own format would stop
    /// working the moment the two ends ran different builds.
    fn horizon_retract(&self, body: &[u8]) -> (u16, String) {
        let f = form(body);
        let Ok(node) = self.acting() else {
            return (409, "this node is not serving yet\n".into());
        };
        let Some(dir) = self.horizon.clone() else {
            return (
                409,
                "the configuration names no `horizon` directory, so a retraction could not \
                 survive a restart and is refused rather than letting the row come back\n"
                    .into(),
            );
        };
        let (Some(subject), Some(patron)) = (key32(field(&f, "node")), key32(field(&f, "patron")))
        else {
            return (
                400,
                "`node` and `patron` are 64 lower-case hex digits\n".into(),
            );
        };
        let Ok(series) = field(&f, "series").parse::<u32>() else {
            return (400, "`series` is a number\n".into());
        };
        let Ok(revision) = field(&f, "revision").parse::<u64>() else {
            return (400, "`revision` is a number\n".into());
        };
        // **the tombstone first.** If the row left the table and the
        // tombstone were not written, the next start would read the row
        // back and bind the member again.
        if let Err(e) = rhtn_node::horizon::write(
            &dir,
            &rhtn_node::horizon::Row {
                row: rhtn_archive::topology::Asserted {
                    node: subject,
                    patron,
                    series,
                    adoption: [0; 32],
                    from: 0,
                    anchor: None,
                    slot: None,
                    revision,
                },
                material: None,
                retracted: true,
            },
        ) {
            return (500, format!("{e}; nothing changed\n"));
        }
        let outcome = {
            let mut view = node.view.lock().unwrap();
            view.table.retract(&subject, &patron, series, revision)
        };
        tracing::info!(
            target: "daemon",
            step = "surface_act",
            act = "horizon-retract",
            node8 = %rhtn_node::diag::id8(&subject),
            outcome = ?outcome,
            "daemon.lifecycle"
        );
        seen(format!(
            "{outcome:?}: {} under {} at series {series}. The tombstone is written at revision \
             {revision}, so neither a restart nor another device's older insert brings it back.",
            rhtn_node::diag::id8(&subject),
            rhtn_node::diag::id8(&patron),
        ))
    }

    /// Add a grant: roles, and one clause of §10.3's vocabulary or none at
    /// all, which grants over every member of the owner's horizon.
    fn grant_add(&self, body: &[u8]) -> (u16, String) {
        let f = form(body);
        self.amend(field(&f, "resource"), |grants, _| {
            // a checkbox per declared role, so several arrive under one
            // name; a comma-separated value is read the same way, since
            // an operator reaching this route by hand will write one
            let roles: BTreeSet<String> = fields(&f, "roles")
                .iter()
                .flat_map(|v| v.split(','))
                .map(str::trim)
                .filter(|r| !r.is_empty())
                .map(str::to_string)
                .collect();
            let of = field(&f, "of");
            let clauses = if of.is_empty() || of == "everyone" {
                Vec::new()
            } else {
                vec![Clause::read(of, Some(field(&f, "with")))?]
            };
            let g = Grant { roles, clauses };
            let said = format!("granted {g}");
            grants.push(g);
            Ok(said)
        })
    }

    /// Narrow a grant by one clause, which is the conjunction
    /// `resource-requirements.md` §7.1 asks for: "every node at [relative
    /// tier] with [trust above threshold]".
    fn grant_narrow(&self, body: &[u8]) -> (u16, String) {
        let f = form(body);
        self.amend(field(&f, "resource"), |grants, _| {
            let at: usize = field(&f, "at")
                .parse()
                .map_err(|_| "which grant is a number".to_string())?;
            let g = grants.get_mut(at).ok_or("no such grant")?;
            g.clauses
                .push(Clause::read(field(&f, "of"), Some(field(&f, "with")))?);
            Ok(format!("narrowed to {g}"))
        })
    }

    /// Withdraw a grant. §10.5 ends the sessions the rows it wrote were
    /// carrying, which happens because the rows go rather than in spite of
    /// it.
    fn grant_drop(&self, body: &[u8]) -> (u16, String) {
        let f = form(body);
        self.amend(field(&f, "resource"), |grants, _| {
            let at: usize = field(&f, "at")
                .parse()
                .map_err(|_| "which grant is a number".to_string())?;
            if at >= grants.len() {
                return Err("no such grant".into());
            }
            Ok(format!("withdrew {}", grants.remove(at)))
        })
    }

    /// Take a grant the package shipped (§10.4), which is the one-click
    /// choice — and it is the operator's click, not the author's.
    fn grant_template(&self, body: &[u8]) -> (u16, String) {
        let f = form(body);
        self.amend(field(&f, "resource"), |grants, declared| {
            let name = field(&f, "name");
            let t = declared
                .templates
                .iter()
                .find(|t| t.name == name)
                .ok_or_else(|| format!("`{name}` is not a grant this package offers"))?;
            grants.push(t.grant.clone());
            Ok(format!("took `{name}`, which grants {}", t.grant))
        })
    }

    /// Re-read the hosting file, so what this node hosts and who may reach
    /// it can change without a restart (OPS-012).
    fn reload(&self) -> (u16, String) {
        let Ok(node) = self.acting() else {
            return (409, "this node is not serving yet\n".into());
        };
        let Some((path, limits)) = self.hosting.clone() else {
            return (409, "the configuration names no `resources` file\n".into());
        };
        let mut view = node.view.lock().unwrap();
        match crate::hosting::apply(
            &mut view.resources,
            &path,
            limits,
            self.grants.as_deref(),
            self.state.as_deref(),
        ) {
            Ok(mut applied) => {
                let n = applied.bound;
                // the same facts a start would have told it (§9.3)
                applied.over(&self.packages);
                for (res, facts) in applied.facts {
                    view.federation.set_host(res, facts);
                }
                // §10.2: the operator has just configured roles
                view.expand_grants();
                // and the instances' relationships, from the same file
                match crate::hosting::relationships(&path) {
                    Ok(rs) => {
                        for (res, r) in rs {
                            if let Err(e) = view.federation.configure(res, r) {
                                return (400, format!("{}: {e}\n", path.display()));
                            }
                        }
                    }
                    Err(e) => return (400, format!("{}: {e}\n", path.display())),
                }
                match crate::hosting::locations(&path) {
                    Ok(ls) => {
                        view.federation.unlocate_all();
                        for (instance, at) in ls {
                            if let Err(e) = view.federation.locate(instance, at) {
                                return (400, format!("{}: {e}\n", path.display()));
                            }
                        }
                    }
                    Err(e) => return (400, format!("{}: {e}\n", path.display())),
                }
                // a resource that is no longer hosted has no plane, and a
                // backend that has just been bound has been told nothing
                let bound = view.resources.bound();
                view.federation.hosted_now(&bound);
                tracing::info!(
                    target: "daemon",
                    step = "surface_act",
                    act = "reload",
                    bound = n,
                    "daemon.lifecycle"
                );
                seen(format!(
                    "re-read {}: {n} package(s) bound\n",
                    path.display()
                ))
            }
            Err(e) => (400, format!("{}: {e}\n", path.display())),
        }
    }

    /// **The directive plane, from the operator's surface**
    /// (`rhtn_node::federation`): `PUT …/envelope` offers an envelope to
    /// a hosted instance as if from the related instance that signed it,
    /// standing in for the transport between nodes that does not exist
    /// yet; `GET …/federation` reads the plane — relationships, what is
    /// held, what waits, what is pending for an instance elsewhere, and
    /// any conflict — which is also how an operator carries a pending
    /// envelope to the other node by hand; `POST …/federate` runs one
    /// pass now rather than at the next tick.
    fn resource_act(&self, method: &str, path: &str, body: &[u8]) -> (u16, String) {
        use rhtn_node::federation::{Envelope, Outcome};
        let rest = path.trim_start_matches("/node/resource/");
        let Some((res_hex, act)) = rest.split_once('/') else {
            return (
                404,
                "GET /node/resource/<hex>/federation, GET …/log, PUT …/envelope, POST …/federate\n"
                    .into(),
            );
        };
        let Some(resource) = unhex(res_hex).and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
        else {
            return (400, format!("`{res_hex}` is not 64 hex digits\n"));
        };
        let Ok(node) = self.acting() else {
            return (409, "this node is not serving yet\n".into());
        };
        match (method, act) {
            ("GET", "federation") => {
                let view = node.view.lock().unwrap();
                let Some(plane) = view.federation.plane(&resource) else {
                    return (404, "this node hosts no such instance\n".into());
                };
                let hex32 = |k: &[u8]| hex(k);
                let held: Vec<serde_json::Value> = plane
                    .held()
                    .iter()
                    .map(|(o, r, h)| {
                        serde_json::json!({
                            "origin": hex32(o), "record": r, "revision": h.revision,
                            "digest": hex32(&h.digest),
                            "state": if h.state == rhtn_node::federation::State::Live { "live" } else { "withdrawn" },
                            "stage": match h.stage {
                                rhtn_node::federation::Stage::Waiting => "waiting",
                                rhtn_node::federation::Stage::Applied => "applied",
                                rhtn_node::federation::Stage::Stopped => "stopped",
                            },
                        })
                    })
                    .collect();
                let body = serde_json::json!({
                    "resource": hex32(&resource),
                    "relationships": plane.relationships.to_json(),
                    "relationships_told": plane.relationships_told,
                    "held": held,
                    "waiting": plane.waiting(),
                    "pending": plane.pending.iter().map(|p| serde_json::json!({
                        "to": hex32(&p.to), "envelope": p.envelope.to_json()
                    })).collect::<Vec<_>>(),
                    "conflicts": plane.conflicts.iter().map(|(e, h)| serde_json::json!({
                        "offered": e.to_json(), "held_revision": h.revision, "held_digest": hex32(&h.digest)
                    })).collect::<Vec<_>>(),
                    "stopped": plane.stopped.iter().map(|(e, why)| serde_json::json!({
                        "envelope": e.to_json(), "why": why
                    })).collect::<Vec<_>>(),
                    "epoch": plane.epoch,
                    "custody": if view.federation.keeping() { "kept" } else { "in memory" },
                });
                (200, format!("{body}\n"))
            }
            ("PUT", "envelope") | ("POST", "envelope") => {
                let v: serde_json::Value = match serde_json::from_slice(body) {
                    Ok(v) => v,
                    Err(e) => return (400, format!("the body is not JSON: {e}\n")),
                };
                let e = match Envelope::from_json(&v) {
                    Ok(e) => e,
                    Err(why) => return (400, format!("not an envelope: {why}\n")),
                };
                let outcome = {
                    let mut view = node.view.lock().unwrap();
                    match view.federation.deliver(&resource, e) {
                        Ok(o) => o,
                        Err(why) => return (409, format!("{why}\n")),
                    }
                };
                tracing::info!(
                    target: "daemon",
                    step = "surface_act",
                    act = "envelope",
                    outcome = ?outcome,
                    "daemon.lifecycle"
                );
                // **taken: handed on now rather than at the next tick**,
                // and by the service's own rounds, which call no backend
                // under the node's lock ([`crate::service::Service::federate`])
                if matches!(
                    outcome,
                    Outcome::New | Outcome::Superseded | Outcome::Requeued
                ) {
                    crate::federation::rounds(&node);
                }
                let status = match &outcome {
                    Outcome::New
                    | Outcome::Superseded
                    | Outcome::Requeued
                    | Outcome::Duplicate
                    | Outcome::Older
                    | Outcome::Tombstoned => 200,
                    Outcome::Conflict => 409,
                    Outcome::Refused(_) => 400,
                };
                (status, format!("{outcome:?}\n"))
            }
            // the local rounds; what is pending for another node goes on
            // the tick, which is where the transport lives
            ("POST", "federate") => (200, format!("{:?}\n", crate::federation::rounds(&node))),
            // **the instance's own log, for its operator**
            // (`infra-client-requirements.md` §9.3): what the package
            // wrote and what the host refused it, the newest at the end,
            // the last 64 KiB where it is longer
            ("GET", "log") => {
                let Some(root) = &self.state else {
                    return (
                        404,
                        "this node keeps no state directory for its packages\n".into(),
                    );
                };
                let file = root.join("logs").join(format!("{res_hex}.log"));
                match std::fs::read(&file) {
                    Ok(bytes) => {
                        let cut = bytes.len().saturating_sub(64 << 10);
                        let from = if cut == 0 {
                            0
                        } else {
                            bytes[cut..]
                                .iter()
                                .position(|b| *b == b'\n')
                                .map_or(cut, |i| cut + i + 1)
                        };
                        (200, String::from_utf8_lossy(&bytes[from..]).into_owned())
                    }
                    Err(_) => (404, "this instance has no log\n".into()),
                }
            }
            _ => (
                404,
                "GET /node/resource/<hex>/federation, GET …/log, PUT …/envelope, POST …/federate\n"
                    .into(),
            ),
        }
    }

    /// Stop, down the path a SIGTERM takes, so what is held is written
    /// back and nothing in flight is lost.
    fn stop(&self) -> (u16, String) {
        if self.acting().is_err() {
            return (409, "this node is not serving yet\n".into());
        }
        tracing::info!(target: "daemon", step = "surface_act", act = "stop", "daemon.lifecycle");
        // **notify_one and not notify_waiters**: the service's loop builds
        // its `notified()` future afresh on each pass of the select, so a
        // wake that only reaches waiters registered at that instant is lost
        // in the gap between passes. `notify_one` leaves a permit, which
        // the next pass takes [2026-10-08].
        self.stopping.notify_one();
        seen("stopping: what is held is written back on the way out\n".into())
    }

    /// The public half, with the proof that this instance holds the token.
    fn fetch(&self, query: Option<&str>) -> (u16, String) {
        let nonce = query
            .and_then(|q| {
                q.split('&')
                    .find_map(|kv| kv.strip_prefix("nonce="))
                    .map(str::to_string)
            })
            .and_then(|h| unhex(&h));
        let Some(nonce) = nonce.filter(|n| n.len() == NONCE_BYTES) else {
            return (
                400,
                format!("a fetch carries `nonce=` of {NONCE_BYTES} bytes in hex\n"),
            );
        };
        let key = self.credential.public();
        // **what it still wants, so one fetch drives the exchange** rather
        // than a list of steps written down somewhere else: each of the
        // three is named with whether this instance is holding it
        let held = |p: &Option<PathBuf>| match p {
            None => "unconfigured",
            Some(p) if p.exists() => "held",
            Some(_) => "wanted",
        };
        // **when the run ends, so the operator can renew before it
        // lapses** (`infra-client-requirements.md` §7: "telling your
        // operator before the last one is the obligation"). A renewal
        // starts where the run ends, and the fetch is where this instance
        // says where that is.
        let issued = self.credential.issued();
        let ends = issued.iter().map(|i| i.not_after).max().unwrap_or(0);
        // **the subtree this instance counts, and what its entry claims**
        // (`infra-client-requirements.md` §4.4): the size is a fact about
        // the node, so the node states it and the client re-signs where
        // the two differ. The points come back too, so a re-signing names
        // the same addresses without the client having to remember them.
        let running = self.running();
        let subtree = running
            .as_ref()
            .map(|(node, _)| node.view.lock().unwrap().subtree_seen())
            .unwrap_or(0);
        let entry = self
            .anchor_entry
            .as_deref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| rhtn_node::resolution::AnchorEntry::parse(&b).ok());
        let mut said = format!(
            "transport {}\nproof {}\nphase {}\ncredentials {}\nrun-ends {}\nendpoint-record {}\nanchor-entry {}\nsubtree {subtree}\n",
            hex(&key),
            hex(&proof(&self.token.lock().unwrap(), &nonce, &key)),
            if running.is_some() {
                "serving"
            } else {
                "enrolling"
            },
            issued.len(),
            ends,
            held(&self.endpoint_record),
            held(&self.anchor_entry),
        );
        if let Some(e) = &entry {
            said.push_str(&format!(
                "anchor-claims {}\nanchor-seqno {}\nanchor-series {}\nanchor-points {}\n",
                e.subtree_size,
                e.seqno.counter,
                e.seqno.series,
                e.endpoints
                    .iter()
                    .map(|p| hex(&p.encode_bytes()))
                    .collect::<Vec<_>>()
                    .join(",")
            ));
        }
        (200, said)
    }

    /// One of the two records only this instance's operator can sign:
    /// checked as [`crate::service::Service::start`] will check it, then
    /// kept where the configuration says that check will read it.
    ///
    /// **The order is what makes this work without a restart.** `start`
    /// reads both of these *after* the loop that waits for the run, so a
    /// record delivered during enrolment is on disk before anything looks
    /// for it.
    ///
    /// **And it is checked here as well as there.** `start` refuses a
    /// record that is not this node's under its operator's signature; so
    /// does this, because the operator is standing in front of this
    /// refusal, where there they would meet it as a node that would not
    /// come up.
    fn take_signed(&self, which: Signed, body: &[u8]) -> (u16, String) {
        let path = match which {
            Signed::Endpoint => &self.endpoint_record,
            Signed::Anchor => &self.anchor_entry,
        };
        let Some(path) = path else {
            return (
                409,
                format!(
                    "the configuration names no `{}`, so there is nowhere one would be read from\n",
                    which.key()
                ),
            );
        };
        if body.is_empty() {
            return (400, format!("a `{}` was empty\n", which.key()));
        }
        let ids = std::slice::from_ref(&self.operator);
        let me = self.operator.keyhash;
        let what = match which {
            Signed::Endpoint => match rhtn_node::store::EndpointRecord::parse(body) {
                Err(e) => return (400, format!("not an endpoint record: {e}\n")),
                Ok(r) if r.node != me => {
                    return (400, "an endpoint record for another node\n".into());
                }
                Ok(r) if r.signature_checks(ids) != Some(true) => {
                    return (
                        400,
                        "an endpoint record this instance's operator did not sign\n".into(),
                    );
                }
                Ok(r) => format!("endpoints {} seqno {}", r.endpoints.len(), r.seqno.counter),
            },
            Signed::Anchor => match rhtn_node::resolution::AnchorEntry::parse(body) {
                Err(e) => return (400, format!("not an anchor entry: {e}\n")),
                Ok(e) if e.anchor != me => {
                    return (400, "an anchor entry for another subnet\n".into());
                }
                Ok(e) if e.signature_checks(ids) != Some(true) => {
                    return (
                        400,
                        "an anchor entry this instance's operator did not sign\n".into(),
                    );
                }
                Ok(e) => format!("subtree {} seqno {}", e.subtree_size, e.seqno.counter),
            },
        };
        // **an identical re-send is a no-op and says so.** §2.3 has an
        // unchanged list republished by replaying the record already held,
        // so a client retrying a push it is unsure landed must not meet a
        // refusal for having succeeded.
        if std::fs::read(path).is_ok_and(|held| held == body) {
            return (200, format!("already held {} {what}\n", which.key()));
        }
        // **a serving node decides, and the file follows its decision.**
        // Ordering is the node's question and it already answers it: an
        // originated endpoint record meets the store's supersession
        // discipline, and `AnchorTable::offer` refuses a seqno no newer
        // than the one held. So a replay changes neither the node's state
        // nor the bytes a restart would read.
        if let Some((node, _)) = self.running() {
            let (took, said) = match which {
                Signed::Endpoint => match node.originate(KIND_ENDPOINT_RECORD, body) {
                    Decision::Stored => (true, "published".to_string()),
                    other => (false, format!("{other:?}")),
                },
                // parsed again rather than threaded out of the check above:
                // `offer` consumes an entry, and a parse that has already
                // succeeded cannot fail here
                Signed::Anchor => {
                    let ids = node.ids.lock().unwrap().clone();
                    match rhtn_node::resolution::AnchorEntry::parse(body) {
                        Ok(entry) => {
                            let took = node.anchors.lock().unwrap().offer(entry, ids.as_slice());
                            if took {
                                (true, "offered".to_string())
                            } else {
                                (false, "no newer than the entry held".to_string())
                            }
                        }
                        Err(e) => (false, e),
                    }
                }
            };
            if !took {
                tracing::info!(
                    target: "daemon",
                    step = "surface_record",
                    which = which.key(),
                    took = false,
                    said = %said,
                    "daemon.lifecycle"
                );
                return (409, format!("the node did not take it: {said}\n"));
            }
        }
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            return (500, format!("{}: {e}\n", parent.display()));
        }
        if let Err(e) = std::fs::write(path, body) {
            return (500, format!("{}: {e}\n", path.display()));
        }
        tracing::info!(
            target: "daemon",
            step = "surface_record",
            which = which.key(),
            took = true,
            what = %what,
            "daemon.lifecycle"
        );
        (200, format!("taken {} {what}\n", which.key()))
    }

    /// A run, verified and kept.
    ///
    /// **Verified at the door rather than on the next read.** `read_run`
    /// already reports and skips what it cannot take, so writing first
    /// would work — but then a caller could fill the instance's disk with
    /// bytes nobody signed, and the operator would learn of it from a log
    /// rather than from the refusal they are standing in front of.
    fn take_run(&self, body: &[u8]) -> (u16, String) {
        if body.is_empty() {
            return (
                400,
                "a run is the delegation's bytes, and this was empty\n".into(),
            );
        }
        let d = match self
            .credential
            .add(std::slice::from_ref(&self.operator), body)
        {
            Ok(d) => d,
            Err(e) => return (400, format!("not taken into the run: {e}\n")),
        };
        if let Err(e) = std::fs::create_dir_all(&self.delegations) {
            return (500, format!("{}: {e}\n", self.delegations.display()));
        }
        // named by the window it opens, so a run arriving in pieces lands
        // as its own files and a credential re-sent overwrites itself
        let at = self.delegations.join(format!("run-{}", d.not_before));
        if let Err(e) = std::fs::write(&at, body) {
            return (500, format!("{}: {e}\n", at.display()));
        }
        let held = self.credential.issued().len();
        tracing::info!(
            target: "daemon",
            step = "surface_run",
            not_before = d.not_before,
            not_after = d.not_after,
            held,
            "daemon.lifecycle"
        );
        (
            200,
            format!(
                "taken {}..{}\ncredentials {held}\n",
                d.not_before, d.not_after
            ),
        )
    }
}

/// One response and no keep-alive: one exchange a connection, which is
/// also what the parser admits (`Reject::MoreThanOne`).
async fn answer(stream: &mut TcpStream, code: u16, body: &str) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        413 => "Content Too Large",
        _ => "Internal Server Error",
    };
    // a page is html and everything else is a line of text: the one bit of
    // content negotiation this surface does
    let kind = if body.starts_with("<!DOCTYPE html>") {
        "text/html"
    } else {
        "text/plain"
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         content-type: {kind}; charset=utf-8\r\n\
         content-length: {}\r\n\
         connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(body.as_bytes()).await?;
    stream.flush().await
}

/// **What an operator sees after an act.** A line, and a way back to the
/// page: a form that posted and got a paragraph has left them nowhere.
fn seen(said: String) -> (u16, String) {
    (
        200,
        format!(
            "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
             <title>done · rhtnd</title></head><body>\n\
             <p>{said}</p>\n<p><a href=\"/\">Back</a></p>\n</body></html>\n"
        ),
    )
}

/// **Every resource bound here, as its tab shows it**
/// (`infra-client-requirements.md` §10.6, §10.7): the binding, who holds a
/// row, and how many sessions are open. One lock, so the counts describe
/// one instant.
fn resource_views(node: &LiveNode) -> Vec<crate::operator::ResourceView> {
    let view = node.view.lock().unwrap();
    let g = &view.resources;
    // **a template's population, as of this instant** (§10.4): the
    // ranking behind a rank clause is computed here only if some template
    // or grant reads one, and is kept nowhere (design §16.2)
    let table = view.table.clone_for(view.me());
    let standing = if g.wants_standing() {
        rhtn_node::grant::Standing::of(&view.evaluate(&g.candidates(&table)).individual)
    } else {
        rhtn_node::grant::Standing::unknown()
    };
    let roles = |r: &rhtn_node::resources::Row| {
        let mut out: Vec<String> = r.roles.iter().cloned().collect();
        if r.connect {
            out.insert(0, "connect".into());
        }
        out
    };
    g.bound()
        .into_iter()
        .filter_map(|resource| {
            let b = g.binding(&resource)?;
            let rows = g
                .rows()
                .into_iter()
                .filter(|((res, _), _)| *res == resource)
                .map(|((_, member), row)| (member, roles(&row), g.is_derived(&resource, &member)))
                .collect();
            Some(crate::operator::ResourceView {
                resource,
                owner: b.owner,
                authority: b.authority.clone(),
                hosted: b.backend.is_some(),
                roles: g.role_bindings(&resource),
                grants: g
                    .grants_for(&resource)
                    .iter()
                    .map(|gr| gr.to_string())
                    .collect(),
                templates: b
                    .declared
                    .templates
                    .iter()
                    .map(|t| {
                        (
                            t.name.clone(),
                            t.label.clone(),
                            t.grant.to_string(),
                            g.population(&resource, &t.grant, &table, &standing),
                        )
                    })
                    .collect(),
                rows,
                sessions: g.hosted_sessions(),
                admin: b.declared.admin.clone(),
                federation: view.federation.plane(&resource).map(|p| {
                    crate::operator::FederationView {
                        patron: p.relationships.patron,
                        subordinates: p.relationships.subordinates.iter().copied().collect(),
                        kin: p
                            .relationships
                            .kin
                            .iter()
                            .map(|(n, m)| (n.clone(), m.iter().copied().collect()))
                            .collect(),
                        held: p
                            .held()
                            .into_iter()
                            .map(|(o, r, h)| {
                                (
                                    o,
                                    r,
                                    h.revision,
                                    h.state == rhtn_node::federation::State::Live,
                                )
                            })
                            .collect(),
                        waiting: p.waiting(),
                        pending: p.pending.len(),
                        conflicts: p.conflicts.len(),
                        stopped: p.stopped.len(),
                        custody: view.federation.keeping(),
                    }
                }),
            })
        })
        .collect()
}

/// **The restricted key an install carries**, as the operator's client
/// sent it: the private half of the key the host's `authorized_keys`
/// binds to the runner (`rhtn_crypto::ssh::host_runner_key`).
///
/// **Held no longer than the act.** It is zeroized when this value is
/// dropped, and the file [`crate::runner`] writes it to is overwritten
/// and removed when the call returns. The node keeps no copy, which is
/// what makes an instance seized between two acts hold nothing that would
/// let it ask its host for anything.
fn runner_key(f: &[(String, String)]) -> Result<zeroize::Zeroizing<String>, (u16, String)> {
    let key = field(f, "key");
    if key.trim().is_empty() {
        return Err((
            400,
            "this act carries the restricted ssh key as `key`: an instance has no key of its \
             own for its host, and is handed one for the act it is making. Take it from your \
             client — Operator, the node's runner key — or from `rhtn keys ssh <identity> \
             --runner --private <file>`\n"
                .into(),
        ));
    }
    if !key.contains("OPENSSH PRIVATE KEY") {
        return Err((
            400,
            "`key` is not an OpenSSH private key: the private half is wanted here, which is \
             what opens the session, and the public half is what the host already holds\n"
                .into(),
        ));
    }
    Ok(zeroize::Zeroizing::new(key.to_string()))
}

/// What an install asked for, checked.
fn asked(f: &[(String, String)]) -> Result<crate::packages::Package, String> {
    let keyhash = |name: &str| -> Result<[u8; 32], String> {
        let given = field(f, name);
        if given.len() != 64 || given.bytes().any(|b| b.is_ascii_uppercase()) {
            return Err(format!("`{name}` is 64 lower-case hex digits"));
        }
        unhex(given)
            .and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())
            .ok_or_else(|| format!("`{name}` is 64 lower-case hex digits"))
    };
    let port: u16 = field(f, "port")
        .parse()
        .map_err(|_| "`port` is a number between 1 and 65535".to_string())?;
    if port == 0 {
        return Err("`port` is a number between 1 and 65535".into());
    }
    Ok(crate::packages::Package {
        resource: keyhash("resource")?,
        owner: keyhash("owner")?,
        authority: crate::packages::authority(field(f, "authority"))?,
        image: crate::packages::image(field(f, "image"))?,
        port,
    })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A keyhash from a form field: 64 hex digits, or nothing.
fn key32(s: &str) -> Option<[u8; 32]> {
    unhex(s).and_then(|v| <[u8; 32]>::try_from(v).ok())
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || s.is_empty() {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

/// **What a control on the page posted**: the pairs of an
/// `application/x-www-form-urlencoded` body.
///
/// A form is what a page without script has, and the page has no script
/// (`infra-client-requirements.md` §8.3 has the client provide the frame;
/// nothing the node serves may need more of it than a browser).
fn form(body: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(body)
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (unescape(k), unescape(v)),
            None => (unescape(p), String::new()),
        })
        .collect()
}

/// One field, or the empty string: a control the page did not draw is one
/// the act will refuse for what it says rather than for being absent.
fn field<'a>(f: &'a [(String, String)], name: &str) -> &'a str {
    f.iter()
        .find(|(k, _)| k == name)
        .map_or("", |(_, v)| v.as_str())
}

/// Every value under one name: a checkbox group posts one field per box
/// ticked.
fn fields<'a>(f: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    f.iter()
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
        .collect()
}

/// Percent-decoding, and `+` for a space.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(v) => {
                    out.push(v);
                    i += 3;
                }
                Err(_) => {
                    out.push(b'%');
                    i += 1;
                }
            },
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}
