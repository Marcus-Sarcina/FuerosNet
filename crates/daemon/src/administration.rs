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

use aws_lc_rs::hmac;
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

/// The domain separation over the proof: a tag of this project's own, so
/// the token cannot be made to authenticate anything else that happens to
/// hash the same bytes.
pub const PROOF_INFO: &[u8] = b"rhtn/1:enrolment-proof";

/// The most one request may carry. A run's delegations are a few
/// hundred bytes each; this is room for a generous batch and no more, so a
/// caller cannot spend the instance's memory before it serves anything.
pub const MAX_REQUEST: usize = 64 * 1024;

/// How long one caller may hold the listener. Enrolment is a fetch and a
/// push; a connection that takes longer than this is not enrolling.
pub const CALLER_TIMEOUT: Duration = Duration::from_secs(10);

/// The nonce a fetch must carry, in bytes. Sixteen: the client chooses it
/// per fetch, and the proof is worthless for any other.
pub const NONCE_BYTES: usize = 16;

/// The proof an instance returns beside its transport public key:
/// `HMAC-SHA256` under the enrolment token over [`PROOF_INFO`], the
/// caller's nonce, and the key.
///
/// **The nonce is what makes it unreplayable** and the key is what it is
/// about: a proof over one key does not carry to another, which is the
/// substitution the token is here to stop.
pub fn proof(token: &[u8; 32], nonce: &[u8], key: &[u8; 32]) -> [u8; 32] {
    let mut ctx = hmac::Context::with_key(&hmac::Key::new(hmac::HMAC_SHA256, token));
    ctx.update(PROOF_INFO);
    ctx.update(nonce);
    ctx.update(key);
    let mut out = [0u8; 32];
    out.copy_from_slice(ctx.sign().as_ref());
    out
}

/// The surface an operator reaches their instance over: what it needs to
/// answer a fetch, to take a run, and to hand the node a record.
pub struct Surface {
    /// Where it listens, from the configuration's `[administration]`.
    pub listen: SocketAddr,
    /// The one-time token, used as a MAC key and never sent.
    pub token: [u8; 32],
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
    /// **Where this node keeps who may reach each resource**
    /// ([`crate::grants`]), which is what a grant act rewrites. Absent,
    /// an act has nowhere to persist and refuses rather than changing a
    /// table that would not survive a restart.
    pub grants: Option<PathBuf>,
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
            // and the next one is waiting
            let _ = tokio::time::timeout(CALLER_TIMEOUT, self.exchange(stream, from)).await;
        }
        // the listener runs until it is dropped, which the service's own end
        // does to it
    }

    /// One connection: read a request, answer it, close.
    async fn exchange(&self, mut stream: TcpStream, from: SocketAddr) {
        let mut buf = Vec::new();
        let req = loop {
            let mut chunk = [0u8; 4096];
            match stream.read(&mut chunk).await {
                Ok(0) => return,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
                Err(_) => return,
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
        let (code, body) = self.answer_to(&req);
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
    pub fn answer_to(&self, req: &http::Request) -> (u16, String) {
        let (path, query) = match req.target.split_once('?') {
            Some((p, q)) => (p, Some(q)),
            None => (req.target.as_str(), None),
        };
        match (req.method.as_str(), path) {
            ("GET", "/") | ("GET", "/index.html") => self.page(query),
            ("GET", "/node") => self.fetch(query),
            ("POST", "/node/acknowledge") => self.acknowledge(&req.body),
            ("POST", "/node/grant") => self.grant_add(&req.body),
            ("POST", "/node/grant/narrow") => self.grant_narrow(&req.body),
            ("POST", "/node/grant/drop") => self.grant_drop(&req.body),
            ("POST", "/node/grant/template") => self.grant_template(&req.body),
            ("POST", "/node/reload") => self.reload(),
            ("POST", "/node/stop") => self.stop(),
            ("PUT", "/node/run") | ("POST", "/node/run") => self.take_run(&req.body),
            ("PUT", "/node/endpoint-record") | ("POST", "/node/endpoint-record") => {
                self.take_signed(Signed::Endpoint, &req.body)
            }
            ("PUT", "/node/anchor-entry") | ("POST", "/node/anchor-entry") => {
                self.take_signed(Signed::Anchor, &req.body)
            }
            ("GET", _) | ("PUT", _) | ("POST", _) => (
                404,
                "this surface answers GET /, GET /node, \
                 PUT /node/{run,endpoint-record,anchor-entry}, \
                 POST /node/{acknowledge,reload,stop} and \
                 POST /node/grant[/narrow|/drop|/template]\n"
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

    /// **A management act, and what authorises one** [author, 2026-10-08]:
    /// reaching this port. `infra-client-requirements.md` §8.2 puts
    /// administration "out of band, with everything else about the host",
    /// so where the surface binds is the access decision — and an act
    /// carries no key, which is what lets an operator administer from a
    /// desktop holding no seed (design §23.3).
    ///
    /// **None of these is what §8.1 and OPS-011 forbid**: no frame is
    /// composed, nothing is replayed, no signature is made and no packet
    /// is approved by hand. They are OPS-012's "explicit management acts".
    fn acting(&self) -> Result<Arc<LiveNode>, (u16, String)> {
        self.running()
            .map(|(n, _)| n)
            .ok_or((409, "this node is not serving yet\n".into()))
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
        match crate::hosting::apply(&mut view.resources, &path, limits, self.grants.as_deref()) {
            Ok(n) => {
                // §10.2: the operator has just configured roles
                view.expand_grants();
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
        (
            200,
            format!(
                "transport {}\nproof {}\nphase {}\ncredentials {}\nendpoint-record {}\nanchor-entry {}\n",
                hex(&key),
                hex(&proof(&self.token, &nonce, &key)),
                if self.running().is_some() {
                    "serving"
                } else {
                    "enrolling"
                },
                self.credential.issued().len(),
                held(&self.endpoint_record),
                held(&self.anchor_entry),
            ),
        )
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
        404 => "Not Found",
        405 => "Method Not Allowed",
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
            })
        })
        .collect()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
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
