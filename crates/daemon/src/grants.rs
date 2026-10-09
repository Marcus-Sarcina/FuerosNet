//! **Where a resource's grants are kept, and why not in the hosting
//! file.**
//!
//! `infra-client-requirements.md` §10.2 has the role table re-evaluated
//! when "an operator is configuring roles", and OPS-012 has that
//! configuring be an explicit management act. An act that did not survive
//! a restart would not be one: a revocation that came back on the next
//! start is worse than one that never happened, because the operator
//! watched it work.
//!
//! **So grants persist, and that decides where they live** [ruled, author,
//! 2026-10-09]: "it does rewrite configuration with respect to the grants
//! table and the individual resource configs … but grants by itself is
//! just a table update". The node's own configuration is a different
//! thing, and an act still does not touch it — a listen address, a queue
//! cap or the acknowledgement policy is a file its operator owns, and
//! changing one needs a restart or a re-read. A grant is a table update.
//!
//! **Two owners, two files.** The hosting file says what this node hosts;
//! the operator writes it and the node never rewrites it. This directory
//! says who may reach each of them; the node owns it and rewrites a file
//! whenever an act changes that resource's grants. One file per resource,
//! named by its keyhash, so an act touches the one resource it is about
//! and nothing else's data is rewritten to save it.

use crate::hosting::{Refused, at, grant_of, grant_toml, parse_grants};
use rhtn_archive::Keyhash;
use rhtn_node::grant::Grant;
use std::path::{Path, PathBuf};

/// Where this resource's grants are kept inside the directory.
pub fn path_for(dir: &Path, resource: &Keyhash) -> PathBuf {
    let hex: String = resource.iter().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("{hex}.toml"))
}

/// **Read a resource's grants**, or none where the file is absent.
///
/// An absent file is not a fault: §7 has a package's roles start bound to
/// nobody, so *"you installed this and have not decided who may use it"*
/// is the ordinary state of a resource just hosted.
pub fn read_for(dir: &Path, resource: &Keyhash) -> Result<Vec<Grant>, Refused> {
    let path = path_for(dir, resource);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(at(0, format!("{}: {e}", path.display()))),
    };
    let entries = parse_grants(&text).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    let mut out = Vec::new();
    for g in &entries {
        out.push(grant_of(g).map_err(|e| at(0, format!("{}: {e}", path.display())))?);
    }
    Ok(out)
}

/// **Write a resource's grants**, replacing whatever the file said.
///
/// The file is the node's to rewrite, so it is written whole rather than
/// edited: there is no operator's formatting in it to preserve, and a
/// partial write is the one outcome worse than either. It goes down beside
/// itself and is renamed over, so a start that interrupts an act reads the
/// old table rather than half of the new one.
pub fn write_for(dir: &Path, resource: &Keyhash, grants: &[Grant]) -> Result<(), Refused> {
    std::fs::create_dir_all(dir).map_err(|e| at(0, format!("{}: {e}", dir.display())))?;
    let path = path_for(dir, resource);
    let mut text = String::from(
        "# Who may reach this resource.  **Written by the node**: an act on\n\
         # its operator's surface rewrites this file whole, so nothing here\n\
         # is kept but the grants themselves.  The hosting file is the\n\
         # operator's and is never rewritten.\n",
    );
    for g in grants {
        text.push_str(&grant_toml(g));
    }
    let beside = path.with_extension("toml.new");
    std::fs::write(&beside, &text).map_err(|e| at(0, format!("{}: {e}", beside.display())))?;
    std::fs::rename(&beside, &path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    Ok(())
}
