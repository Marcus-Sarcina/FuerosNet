//! The packages this node hosts, and who may reach them.
//!
//! Two obligations meet here. `infra-client-requirements.md` §9 has the
//! operator decide what to host and confine what they admit, and §10 has
//! them hold a role table the gateway consults by lookup. Both are
//! configuration rather than protocol: nothing a peer sees is decided in
//! this file, and a node that hosts nothing is conforming.
//!
//! The file is TOML, and stays a file of its own now that it need not be:
//! what a node hosts and who may reach it is what changes most often, and
//! an operator regenerating it should not be rewriting the identity, the
//! listen address and the paths beside it.
//!
//! ```toml
//! [[host]]
//! resource  = "<keyhash>"
//! owner     = "<keyhash>"
//! authority = "shop.internal"
//! manifest  = "packages/shop/manifest"
//!
//! [[host.grant]]
//! member = "<keyhash>"
//! roles  = ["connect", "reader", "writer"]
//! ```
//!
//! **A grant sits inside the package it grants on**, which is what the
//! nesting buys: a grant naming a resource nothing hosts was an error the
//! line format could express and this one cannot.
//!
//! **A `host` line names a manifest, not a component.** What a package
//! declares is the package's, and §9.1 puts the declaration in the
//! manifest that ships with it; an operator writing role names into their
//! own configuration would be declaring them on the package's behalf. The
//! manifest sits beside its component and is `key = value` a line at a
//! time:
//!
//! ```text
//! roles = reader,writer
//! imports = rhtn/1:request,rhtn/1:response
//! component = echo.wasm
//! ```
//!
//! **What it declares is checked against what the component does**, in
//! both directions. A manifest importing a binding this host does not have
//! is refused by `rhtn_node::resources::instantiate`; a component reaching
//! for one its manifest did not declare is refused here. Neither is a
//! supply chain (§9.1) — that is signing and provenance and is not this
//! file — but a declaration nothing compares against declares nothing.
//!
//! `connect` in a grant is the gate `resource-requirements.md` §3 reserves
//! for the node's own evaluation, which is why it appears here and never
//! reaches the package: the roles a package is told about are the rest of
//! the list. A grant without it is a row that cannot connect, which is a
//! thing an operator may want to write down.

use rhtn_archive::Keyhash;
use rhtn_node::grant::{Clause, Grant, Template};
use rhtn_node::relay::Relay;
use rhtn_node::resources::{Binding, Gateway, Kind, Manifest, Operation, Parameter, instantiate};
use rhtn_resources::{Hosted, Limits, Sandbox};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

/// Why a hosting file was refused: the line it was on, and what was wrong
/// with it.  Line 0 means the file as a whole, which is where everything
/// but a shape error lands — a package that will not run is about the
/// package and not about a line in this file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// The line it was on; 0 means the file as a whole.
    pub line: usize,
    /// What was wrong with it.
    pub what: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            0 => write!(f, "{}", self.what),
            n => write!(f, "line {n}: {}", self.what),
        }
    }
}

impl std::error::Error for Refused {}

fn at(line: usize, what: impl Into<String>) -> Refused {
    Refused {
        line,
        what: what.into(),
    }
}

fn keyhash(s: &str) -> Option<Keyhash> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The file's own shape, before any package is read.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    host: Vec<HostEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HostEntry {
    resource: String,
    owner: String,
    authority: String,
    manifest: std::path::PathBuf,
    /// Where the resource listens, where it holds its own port rather
    /// than running inside this node's sandbox
    /// (`resource-requirements.md` §3's second leg,
    /// `rhtn_node::relay::Relay`).
    ///
    /// **Which of the two a resource is follows from this**, which is
    /// §10.6's "it follows from where the resource runs, so nothing needs
    /// declaring": an address and the node proxies, a component and the
    /// node runs it, neither and the node brokers.
    address: Option<String>,
    /// What the operator granted, in the predicate language
    /// (`infra-client-requirements.md` §10.2, §10.3).
    ///
    /// **One vocabulary, not two.** A grant over everyone, a grant over a
    /// tier and a grant to one named party used to be separate keys; they
    /// are one list of predicates now, because §10.3 has named
    /// individuals *inside* the membership gate rather than beside it
    /// (`resource-requirements.md` §7.1.2) and a second shape for the
    /// same act is a second place for it to disagree.
    #[serde(default)]
    grant: Vec<GrantEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantEntry {
    roles: Vec<String>,
    /// The clauses a member must satisfy, all of them. **Absent grants to
    /// every member of the owner's horizon** — §10.1's outer gate and
    /// nothing further.
    #[serde(default, rename = "where")]
    clauses: Vec<ClauseEntry>,
}

/// One clause of §10.3's vocabulary as it is written down.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ClauseEntry {
    of: String,
    edges: Option<usize>,
    n: Option<usize>,
    percent: Option<u8>,
    when: Option<u64>,
    who: Option<String>,
}

/// Read one clause, refusing a key that means nothing where it is written.
///
/// **The same rule the administrative parameters follow**: `of` decides
/// which other keys have a meaning, and a package or operator who wrote
/// one that does not believes something the host will not do.
fn clause_of(c: &ClauseEntry) -> Result<Clause, String> {
    let only = |keys: [bool; 5], names: &str| -> Result<(), String> {
        let given = [
            c.edges.is_some(),
            c.n.is_some(),
            c.percent.is_some(),
            c.when.is_some(),
            c.who.is_some(),
        ];
        for (i, g) in given.iter().enumerate() {
            if *g && !keys[i] {
                return Err(format!("`{}` takes {names}", c.of));
            }
        }
        Ok(())
    };
    let need =
        |what: Option<usize>, key: &str| what.ok_or_else(|| format!("`{}` names `{key}`", c.of));
    match c.of.as_str() {
        "clients" => {
            only([false; 5], "no other key")?;
            Ok(Clause::Clients)
        }
        "grandclients" => {
            only([false; 5], "no other key")?;
            Ok(Clause::Grandclients)
        }
        "distance" => {
            only([true, false, false, false, false], "`edges`")?;
            Ok(Clause::AtDistance {
                edges: need(c.edges, "edges")?,
            })
        }
        "most-trusted" => {
            only([false, true, false, false, false], "`n`")?;
            Ok(Clause::MostTrusted { n: need(c.n, "n")? })
        }
        "top-fraction" => {
            only([false, false, true, false, false], "`percent`")?;
            Ok(Clause::TopFraction {
                percent: c
                    .percent
                    .ok_or_else(|| format!("`{}` names `percent`", c.of))?,
            })
        }
        "joined-before" => {
            only([false, false, false, true, false], "`when`")?;
            Ok(Clause::JoinedBefore {
                when: c.when.ok_or_else(|| format!("`{}` names `when`", c.of))?,
            })
        }
        "named" => {
            only([false, false, false, false, true], "`who`")?;
            let who = c.who.as_deref().unwrap_or_default();
            Ok(Clause::Named {
                who: keyhash(who)
                    .ok_or_else(|| format!("`{who}` is not 64 lower-case hex digits"))?,
            })
        }
        other => Err(format!(
            "`{other}` is not a clause; they are clients, grandclients, \
             distance, most-trusted, top-fraction, joined-before and named"
        )),
    }
}

/// The grant an entry describes.
fn grant_of(g: &GrantEntry) -> Result<Grant, String> {
    let mut roles = BTreeSet::new();
    for r in &g.roles {
        roles.insert(r.clone());
    }
    let mut clauses = Vec::new();
    for c in &g.clauses {
        clauses.push(clause_of(c)?);
    }
    Ok(Grant { roles, clauses })
}

/// Read `path` and bind what it names into `gateway`.
///
/// **Every package is admitted before any is bound**, and every grant is
/// checked before that. A file naming one package the sandbox refuses, or
/// one role the package never declared, leaves the node hosting none of
/// them rather than some of them: a half-applied configuration is one the
/// operator did not write, and `service.rs` reports rather than repairs.
pub fn apply(gateway: &mut Gateway, path: &Path, limits: Limits) -> Result<usize, Refused> {
    let text =
        std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    let file: File = toml::from_str(&text).map_err(|e| {
        at(
            e.span().map_or(0, |s| {
                text[..s.start.min(text.len())]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    + 1
            }),
            e.to_string(),
        )
    })?;

    let mut hosts: Vec<(Keyhash, Binding)> = Vec::new();
    let mut grants: Vec<(Keyhash, Vec<Grant>)> = Vec::new();
    // **two resources at one address is a collision, not a choice.**
    // Nothing partitions ports across package authors and nothing could,
    // so where the operator names the address the file is the only place
    // the clash can be seen.  Where the node comes to assign addresses
    // itself — §9's image, once it launches what it hosts — this is the
    // table it will assign into.
    let mut addresses: Vec<(std::net::SocketAddr, String)> = Vec::new();

    for h in &file.host {
        let named = |what: &str| format!("{}: {what}", h.manifest.display());
        let res = keyhash(&h.resource).ok_or_else(|| {
            at(
                0,
                format!("`{}` is not 64 lower-case hex digits", h.resource),
            )
        })?;
        let owner = keyhash(&h.owner)
            .ok_or_else(|| at(0, format!("`{}` is not 64 lower-case hex digits", h.owner)))?;
        if hosts.iter().any(|(r, _)| *r == res) {
            return Err(at(0, format!("`{}` is hosted twice", h.resource)));
        }
        let (manifest, component) = read_manifest(&h.manifest).map_err(|e| at(0, named(&e)))?;
        instantiate(&manifest).map_err(|e| at(0, named(&e)))?;
        // **a resource is reached one way, and the file says which**
        // (§10.6): an address and this node proxies to it, a component
        // and this node runs it.  Both is a contradiction rather than a
        // choice, and the operator is told so rather than having one of
        // them silently win
        let backend: Option<Arc<dyn rhtn_node::resources::Backend>> = match (&h.address, &component)
        {
            (Some(_), Some(c)) => {
                return Err(at(
                    0,
                    named(&format!(
                        "an address and a component at {}: a resource this node proxies to \
                             is not one it runs",
                        c.display()
                    )),
                ));
            }
            (Some(a), None) => {
                let addr: std::net::SocketAddr = a
                    .parse()
                    .map_err(|e| at(0, format!("`{a}` is not an address: {e}")))?;
                // **the relay speaks plain HTTP, so it may only speak it
                // to a local socket** (`resource-requirements.md` §3:
                // "HTTPS, required, where it crosses a network", and
                // "plain HTTP is permitted only on a local socket to a
                // package the node hosts itself").  The node already read
                // the request, so what the far leg protects is everyone
                // else: an operator relaying to a resource elsewhere must
                // not put an authenticated principal and an application
                // body on the open network.  Refused rather than carried,
                // until that leg can be given TLS
                if !addr.ip().is_loopback() {
                    return Err(at(
                        0,
                        format!(
                            "`{a}` is not a local address: this leg carries plain HTTP, and                              `resource-requirements.md` §3 requires HTTPS wherever it crosses                              a network"
                        ),
                    ));
                }
                if let Some((_, first)) = addresses.iter().find(|(x, _)| *x == addr) {
                    return Err(at(
                        0,
                        format!(
                            "`{a}` is where `{first}` already listens: two resources at one                              address is one resource answering for both"
                        ),
                    ));
                }
                addresses.push((addr, h.resource.clone()));
                Some(Arc::new(Relay::to(addr, limits.response)))
            }
            (None, Some(c)) => {
                let bytes = std::fs::read(c).map_err(|e| at(0, format!("{}: {e}", c.display())))?;
                let sandbox = Sandbox::admit(&bytes, limits)
                    .map_err(|e| at(0, format!("{}: {e}", c.display())))?;
                let mut declared: Vec<String> = manifest.imports.clone();
                declared.sort();
                declared.dedup();
                if sandbox.reaches() != declared {
                    return Err(at(
                        0,
                        named(&format!(
                            "the manifest declares {declared:?} and the component reaches {:?}",
                            sandbox.reaches()
                        )),
                    ));
                }
                Some(Arc::new(Hosted::new(sandbox)))
            }
            // **brokered**: the node authenticates and the caller's
            // traffic never passes through it (design §11.7, §10.6)
            (None, None) => None,
        };

        let mut written = Vec::new();
        for g in &h.grant {
            let grant = grant_of(g).map_err(|e| at(0, e))?;
            // **checked against the package before anything is bound**:
            // the gateway refuses it too, but by then this file has
            // already bound its earlier entries, and a half-applied
            // configuration is one the operator did not write
            if let Some(why) = grant.refused(&manifest.roles) {
                return Err(at(0, why.to_string()));
            }
            written.push(grant);
        }
        grants.push((res, written));
        hosts.push((
            res,
            Binding {
                owner,
                authority: h.authority.clone(),
                backend,
                declared: manifest,
            },
        ));
    }

    let bound = hosts.len();
    for (res, binding) in hosts {
        gateway.bind(res, binding);
    }
    for (res, written) in grants {
        gateway
            .set_grants(res, written)
            .map_err(|e| at(0, e.to_string()))?;
    }
    Ok(bound)
}

/// TOML's own message with the line it is about, as `config.rs` renders
/// one: `Display` adds a caret diagram that does not belong in a
/// single-line refusal, and `message()` is the sentence without it.
fn manifest_error(text: &str, e: &toml::de::Error) -> String {
    let line = e
        .span()
        .map(|s| {
            text[..s.start.min(text.len())]
                .bytes()
                .filter(|b| *b == b'\n')
                .count()
                + 1
        })
        .unwrap_or(0);
    let m = e.message();
    let said = match m.strip_prefix("missing field ") {
        Some(rest) => format!("{rest} is not set, and has no default"),
        None => m.to_string(),
    };
    if line > 0 {
        format!("manifest line {line}: {said}")
    } else {
        format!("manifest: {said}")
    }
}

/// Read a package's manifest, and where its component sits where it has
/// one.
///
/// **TOML, for the reason the configuration is** (`config.rs`): the
/// line-oriented format this replaced could not repeat a key, and an
/// administrative operation is a repeated table with named fields and
/// parameters of its own — the same shape that moved the operator's
/// configuration off that format. Strict for the reason that one is too:
/// an unknown key or a missing one is an error, because a manifest the
/// host quietly repairs declares something the package did not.
fn read_manifest(path: &Path) -> Result<(Manifest, Option<std::path::PathBuf>), String> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct File {
        /// The component, where the node runs the resource itself.
        /// **Absent for a resource that holds its own port**, which the
        /// node reaches over a socket instead and never compiles.
        component: Option<String>,
        #[serde(default)]
        roles: Vec<String>,
        #[serde(default)]
        imports: Vec<String>,
        #[serde(default)]
        admin: Vec<Op>,
        #[serde(default)]
        template: Vec<Tpl>,
    }
    /// A grant the package ships ready-made
    /// (`infra-client-requirements.md` §10.4), in the same clause
    /// vocabulary an operator's own grant is written in — which is the
    /// requirement, not a convenience: a template an operator cannot read
    /// back is the opaque configuration §7.3 refuses.
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Tpl {
        name: String,
        label: String,
        roles: Vec<String>,
        #[serde(default, rename = "where")]
        clauses: Vec<ClauseEntry>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Op {
        name: String,
        label: String,
        #[serde(default)]
        help: String,
        #[serde(default)]
        parameter: Vec<Param>,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Param {
        name: String,
        label: String,
        #[serde(rename = "type")]
        kind: String,
        low: Option<i64>,
        high: Option<i64>,
        max: Option<usize>,
        #[serde(default)]
        of: Vec<String>,
    }

    let text = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    let f: File = toml::from_str(&text).map_err(|e| manifest_error(&text, &e))?;

    let mut admin = Vec::new();
    for op in f.admin {
        let mut parameters = Vec::new();
        for p in op.parameter {
            // **the type decides which other keys mean anything**, and a
            // key that means nothing where it is written is a package
            // author believing something the host does not do
            let kind = match p.kind.as_str() {
                "flag" => Kind::Flag,
                "number" => Kind::Number {
                    low: p
                        .low
                        .ok_or_else(|| format!("`{}`: a number names `low`", p.name))?,
                    high: p
                        .high
                        .ok_or_else(|| format!("`{}`: a number names `high`", p.name))?,
                },
                "text" => Kind::Text {
                    max: p
                        .max
                        .ok_or_else(|| format!("`{}`: text names `max`, its length", p.name))?,
                },
                "choice" => Kind::Choice { of: p.of.clone() },
                "keyhash" => Kind::Keyhash,
                other => {
                    return Err(format!(
                        "`{}`: `{other}` is not a parameter type; they are \
                         flag, number, text, choice and keyhash",
                        p.name
                    ));
                }
            };
            parameters.push(Parameter {
                name: p.name,
                label: p.label,
                kind,
            });
        }
        admin.push(Operation {
            name: op.name,
            label: op.label,
            help: op.help,
            parameters,
        });
    }
    let mut templates = Vec::new();
    for t in f.template {
        templates.push(Template {
            name: t.name,
            label: t.label,
            grant: grant_of(&GrantEntry {
                roles: t.roles,
                clauses: t.clauses,
            })?,
        });
    }
    let manifest = Manifest {
        roles: f.roles.into_iter().collect(),
        imports: f.imports,
        admin,
        templates,
    };
    // relative to the manifest, so a package is a directory an operator
    // can move without rewriting what is inside it
    let dir = path.parent().unwrap_or(Path::new("."));
    Ok((manifest, f.component.map(|c| dir.join(c))))
}
