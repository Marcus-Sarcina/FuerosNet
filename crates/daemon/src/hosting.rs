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
use rhtn_node::resources::{
    Binding, Gateway, MAX_ROLES, Manifest, RESERVED_ROLES, Row, instantiate,
};
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
    /// A grant standing over every member of the owner's trust horizon
    /// (`infra-client-requirements.md` §10.1, §10.2), rather than one
    /// party at a time.
    #[serde(default)]
    standing: Vec<String>,
    #[serde(default)]
    grant: Vec<GrantEntry>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantEntry {
    member: String,
    roles: Vec<String>,
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
    let mut grants: Vec<(Keyhash, Keyhash, Row)> = Vec::new();
    let mut standing: Vec<(Keyhash, Row)> = Vec::new();

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
        let package = instantiate(&manifest).map_err(|e| at(0, named(&e)))?;
        let bytes = std::fs::read(&component)
            .map_err(|e| at(0, format!("{}: {e}", component.display())))?;
        let sandbox = Sandbox::admit(&bytes, limits)
            .map_err(|e| at(0, format!("{}: {e}", component.display())))?;
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

        for g in &h.grant {
            let member = keyhash(&g.member)
                .ok_or_else(|| at(0, format!("`{}` is not 64 lower-case hex digits", g.member)))?;
            grants.push((res, member, row_of(&g.roles, &package.roles)?));
        }

        if !h.standing.is_empty() {
            let row = row_of(&h.standing, &package.roles)?;
            standing.push((res, row));
        }
        hosts.push((
            res,
            Binding {
                owner,
                authority: h.authority.clone(),
                backend: Some(Arc::new(Hosted::new(sandbox))),
                declared_roles: package.roles,
            },
        ));
    }

    let bound = hosts.len();
    for (res, binding) in hosts {
        gateway.bind(res, binding);
    }
    for (res, member, row) in grants {
        gateway
            .set_row(res, member, row)
            .map_err(|e| at(0, format!("{e:?}")))?;
    }
    for (res, row) in standing {
        gateway
            .stand(res, row)
            .map_err(|e| at(0, format!("{e:?}")))?;
    }
    Ok(bound)
}

/// A role list from the operator, checked against what the package
/// declared.
///
/// `connect` is the gate `resource-requirements.md` §3 reserves for the
/// node's own evaluation, which is why it appears here and never reaches
/// the package: the roles a package is told about are the rest of the
/// list.
fn row_of(named: &[String], declared: &BTreeSet<String>) -> Result<Row, Refused> {
    let connect = named.iter().any(|r| r == "connect");
    let mut application = BTreeSet::new();
    for r in named {
        if r == "connect" {
            continue;
        }
        if RESERVED_ROLES.contains(&r.as_str()) {
            return Err(at(
                0,
                format!(
                    "`{r}` is reserved for the node's own evaluation and is not an application role"
                ),
            ));
        }
        if !declared.contains(r) {
            return Err(at(0, format!("`{r}` is not a role that package declared")));
        }
        application.insert(r.clone());
    }
    if application.len() > MAX_ROLES {
        return Err(at(
            0,
            format!(
                "{} roles is wider than a credential header carries",
                application.len()
            ),
        ));
    }
    Ok(Row {
        roles: application,
        connect,
    })
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
        let (k, v) = line
            .split_once('=')
            .ok_or_else(|| format!("manifest line {}: not `key = value`", i + 1))?;
        let (k, v) = (k.trim().to_string(), v.trim().to_string());
        if !["roles", "imports", "component"].contains(&k.as_str()) {
            return Err(format!(
                "manifest line {}: `{k}` is not a manifest key",
                i + 1
            ));
        }
        if seen.iter().any(|(x, _, _)| *x == k) {
            return Err(format!("manifest line {}: `{k}` was already set", i + 1));
        }
        seen.push((k, v, i + 1));
    }
    let take = |key: &str| {
        seen.iter()
            .find(|(k, _, _)| k == key)
            .map(|(_, v, _)| v.clone())
    };
    let list = |key: &str| {
        take(key)
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    let component = take("component").ok_or("manifest: `component` is not set")?;
    let manifest = Manifest {
        roles: list("roles").into_iter().collect(),
        imports: list("imports"),
    };
    // relative to the manifest, so a package is a directory an operator
    // can move without rewriting what is inside it
    let dir = path.parent().unwrap_or(Path::new("."));
    Ok((manifest, dir.join(component)))
}
