//! The profiles a node holds and serves (`wire-format.md` §7.11;
//! `infra-client-requirements.md` §6.2): one per (subject, subnet, kind),
//! the latest by publication time, taken only from the subject it names
//! and served to anyone who asks.  Opaque to what the node does with it:
//! bytes a subject signed, never rendered, matched or indexed by alias.

use crate::{Keyhash, Txid};
use rhtn_archive::profile::{FAIL_UNKNOWN, Profile, ProfileReply, ProfileRequest};
use rhtn_crypto::verify::Lookup;
use std::collections::BTreeMap;

/// What one profile is filed under.
pub type Triple = (Keyhash, Txid, u64);

/// The store, kept on disk where it was opened at a directory.
#[derive(Debug, Default)]
pub struct ProfileStore {
    held: BTreeMap<Triple, Vec<u8>>,
    dir: Option<std::path::PathBuf>,
}

impl ProfileStore {
    /// Hold a profile a subject publishes: it must verify under the
    /// subject it names, and it replaces what is held for its triple only
    /// where it was published later.  The subject it names is the
    /// caller's to check against the session (`submissions`).
    pub fn publish<L: Lookup + ?Sized>(&mut self, ids: &L, bytes: &[u8]) -> Result<Triple, String> {
        let p = Profile::parse(bytes)?;
        if p.verify(ids).is_err() {
            return Err("profile signature fails".into());
        }
        let triple = (p.subject, p.subnet, p.kind);
        if let Some(held) = self.held.get(&triple)
            && let Ok(h) = Profile::parse(held)
            && h.published_at >= p.published_at
        {
            return Err("not later than the profile held".into());
        }
        if let Some(dir) = &self.dir {
            let d = subject_dir(dir, &p.subject);
            std::fs::create_dir_all(&d)
                .and_then(|_| std::fs::write(d.join(file_name(&p.subnet, p.kind)), bytes))
                .map_err(|e| e.to_string())?;
        }
        self.held.insert(triple, bytes.to_vec());
        Ok(triple)
    }

    /// The profile held for a triple.
    pub fn get(&self, subject: &Keyhash, subnet: &Txid, kind: u64) -> Option<&Vec<u8>> {
        self.held.get(&(*subject, *subnet, kind))
    }

    /// Answer a fetch: the reply bytes, or nothing where the body is not
    /// a request.  Who asked is not recorded
    /// (`infra-client-requirements.md` §6.2).
    pub fn answer(&self, body: &[u8]) -> Option<Vec<u8>> {
        let r = ProfileRequest::decode(body).ok()?;
        let profile = self.get(&r.subject, &r.subnet, r.kind).cloned();
        Some(
            ProfileReply {
                nonce: r.nonce,
                code: profile.is_none().then_some(FAIL_UNKNOWN),
                profile,
            }
            .encode(),
        )
    }

    /// How many are held.
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// Whether none are.
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Write every profile held under `dir`.
    pub fn save(&self, dir: &std::path::Path) -> std::io::Result<()> {
        for ((subject, subnet, kind), bytes) in &self.held {
            let d = subject_dir(dir, subject);
            std::fs::create_dir_all(&d)?;
            std::fs::write(d.join(file_name(subnet, *kind)), bytes)?;
        }
        Ok(())
    }

    /// The store kept at `dir`: what is there is loaded, and from here on
    /// a profile is written as it is taken.
    pub fn at(dir: &std::path::Path) -> std::io::Result<ProfileStore> {
        let mut s = ProfileStore::load(dir)?;
        s.dir = Some(dir.to_path_buf());
        Ok(s)
    }

    /// A store restored from `dir`.  What no longer parses is skipped.
    pub fn load(dir: &std::path::Path) -> std::io::Result<ProfileStore> {
        let mut s = ProfileStore::default();
        let root = dir.join("profiles");
        let Ok(subjects) = std::fs::read_dir(&root) else {
            return Ok(s);
        };
        for sd in subjects.flatten() {
            let Ok(files) = std::fs::read_dir(sd.path()) else {
                continue;
            };
            for f in files.flatten() {
                let Ok(bytes) = std::fs::read(f.path()) else {
                    continue;
                };
                if let Ok(p) = Profile::parse(&bytes) {
                    s.held.insert((p.subject, p.subnet, p.kind), bytes);
                }
            }
        }
        Ok(s)
    }
}

fn subject_dir(dir: &std::path::Path, subject: &Keyhash) -> std::path::PathBuf {
    dir.join("profiles").join(hex(subject))
}

fn file_name(subnet: &Txid, kind: u64) -> String {
    format!("{}-{kind}", hex(subnet))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
