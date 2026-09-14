//! The packages this node hosts, and who may reach them.
//!
//! Two obligations meet here. `infra-client-requirements.md` §9 has the
//! operator decide what to host and confine what they admit, and §10 has
//! them hold a role table the gateway consults by lookup. Both are
//! configuration rather than protocol: nothing a peer sees is decided in
//! this file, and a node that hosts nothing is conforming.
//!
//! The file is a line at a time, in the shape of the peers file, because
//! the number of packages and the number of grants are both open and the
//! configuration refuses a repeated key:
//!
//! ```text
//! host  <resource keyhash> <owner keyhash> <authority> <manifest path>
//! grant <resource keyhash> <member keyhash> connect,reader,writer
//! ```
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
use rhtn_node::resources::{Binding, Gateway, MAX_ROLES, Manifest, RESERVED_ROLES, Row, instantiate};
use rhtn_resources::{Hosted, Limits, Sandbox};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

/// Why a hosting file was refused: the line it was on, and what was wrong
/// with it.  Line 0 means the file as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub line: usize,
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
    Refused { line, what: what.into() }
}

fn keyhash(s: &str) -> Option<Keyhash> {
    if s.len() != 64 || !s.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, b) in out.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// Read `path` and bind what it names into `gateway`.
///
/// **Every package is admitted before any is bound.** A file naming one
/// package the sandbox refuses leaves the node hosting none of them rather
/// than some of them: a half-applied configuration is one the operator did
/// not write, and `service.rs` reports rather than repairs.
pub fn apply(gateway: &mut Gateway, path: &Path, limits: Limits) -> Result<usize, Refused> {
    let text = std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    let mut hosts: Vec<(Keyhash, Binding)> = Vec::new();
    let mut grants: Vec<(Keyhash, Keyhash, Row, usize)> = Vec::new();

    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut f = line.split_whitespace();
        match f.next() {
            Some("host") => {
                let (Some(res), Some(owner), Some(authority), Some(package)) = (f.next(), f.next(), f.next(), f.next()) else {
                    return Err(at(n, "`host` is a resource keyhash, an owner keyhash, an authority and a manifest path"));
                };
                if f.next().is_some() {
                    return Err(at(n, "`host` takes four fields"));
                }
                let res = keyhash(res).ok_or_else(|| at(n, "the resource keyhash is not 64 lower-case hex digits"))?;
                let owner = keyhash(owner).ok_or_else(|| at(n, "the owner keyhash is not 64 lower-case hex digits"))?;
                if hosts.iter().any(|(r, _)| *r == res) {
                    return Err(at(n, "that resource is already hosted on an earlier line"));
                }
                let (manifest, component) = read_manifest(Path::new(package)).map_err(|e| at(n, format!("{package}: {e}")))?;
                let package_ = instantiate(&manifest).map_err(|e| at(n, format!("{package}: {e}")))?;
                let bytes = std::fs::read(&component).map_err(|e| at(n, format!("{}: {e}", component.display())))?;
                let sandbox = Sandbox::admit(&bytes, limits).map_err(|e| at(n, format!("{}: {e}", component.display())))?;
                let mut declared: Vec<String> = manifest.imports.clone();
                declared.sort();
                declared.dedup();
                if sandbox.reaches() != declared {
                    return Err(at(
                        n,
                        format!("{package}: the manifest declares {declared:?} and the component reaches {:?}", sandbox.reaches()),
                    ));
                }
                hosts.push((
                    res,
                    Binding {
                        owner,
                        authority: authority.to_string(),
                        backend: Some(Arc::new(Hosted::new(sandbox))),
                        declared_roles: package_.roles,
                    },
                ));
            }
            Some("grant") => {
                let (Some(res), Some(member), Some(roles)) = (f.next(), f.next(), f.next()) else {
                    return Err(at(n, "`grant` is a resource keyhash, a member keyhash and a comma-separated role list"));
                };
                if f.next().is_some() {
                    return Err(at(n, "`grant` takes three fields, and a role list carries no spaces"));
                }
                let res = keyhash(res).ok_or_else(|| at(n, "the resource keyhash is not 64 lower-case hex digits"))?;
                let member = keyhash(member).ok_or_else(|| at(n, "the member keyhash is not 64 lower-case hex digits"))?;
                let named: Vec<&str> = roles.split(',').map(str::trim).filter(|r| !r.is_empty()).collect();
                let connect = named.contains(&"connect");
                let mut application = BTreeSet::new();
                for r in named {
                    if r == "connect" {
                        continue;
                    }
                    if RESERVED_ROLES.contains(&r) {
                        return Err(at(n, format!("`{r}` is reserved for the node's own evaluation and is not an application role")));
                    }
                    application.insert(r.to_string());
                }
                grants.push((res, member, Row { roles: application, connect }, n));
            }
            Some(other) => return Err(at(n, format!("`{other}` is not `host` or `grant`"))),
            None => continue,
        }
    }

    // every grant is checked against the package it names before any of
    // them is applied, for the same reason the packages are all admitted
    // first: a file the operator did not write is worse than none
    for (res, _, row, n) in &grants {
        let Some((_, binding)) = hosts.iter().find(|(r, _)| r == res) else {
            // a grant for a resource nothing hosts is a typo with no
            // effect, and keeping it silently would leave the operator
            // believing they granted something
            return Err(at(*n, "no `host` line names that resource"));
        };
        if row.roles.len() > MAX_ROLES {
            return Err(at(*n, format!("{} roles is wider than a credential header carries", row.roles.len())));
        }
        if let Some(r) = row.roles.iter().find(|r| !binding.declared_roles.contains(*r)) {
            return Err(at(*n, format!("`{r}` is not a role that package declared")));
        }
    }

    let bound = hosts.len();
    for (res, binding) in hosts {
        gateway.bind(res, binding);
    }
    for (res, member, row, n) in grants {
        gateway.set_row(res, member, row).map_err(|e| at(n, format!("{e:?}")))?;
    }
    Ok(bound)
}

/// Read a package's manifest, and where its component sits.
///
/// Strict for the reason the configuration is: an unknown key, a repeated
/// key or a missing one is an error naming its line, because a manifest
/// the host quietly repairs declares something the package did not.
fn read_manifest(path: &Path) -> Result<(Manifest, std::path::PathBuf), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
    let mut seen: Vec<(String, String, usize)> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (k, v) = line.split_once('=').ok_or_else(|| format!("manifest line {}: not `key = value`", i + 1))?;
        let (k, v) = (k.trim().to_string(), v.trim().to_string());
        if !["roles", "imports", "component"].contains(&k.as_str()) {
            return Err(format!("manifest line {}: `{k}` is not a manifest key", i + 1));
        }
        if seen.iter().any(|(x, _, _)| *x == k) {
            return Err(format!("manifest line {}: `{k}` was already set", i + 1));
        }
        seen.push((k, v, i + 1));
    }
    let take = |key: &str| seen.iter().find(|(k, _, _)| k == key).map(|(_, v, _)| v.clone());
    let list = |key: &str| take(key).unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect::<Vec<_>>();
    let component = take("component").ok_or("manifest: `component` is not set")?;
    let manifest = Manifest { roles: list("roles").into_iter().collect(), imports: list("imports") };
    // relative to the manifest, so a package is a directory an operator
    // can move without rewriting what is inside it
    let dir = path.parent().unwrap_or(Path::new("."));
    Ok((manifest, dir.join(component)))
}
