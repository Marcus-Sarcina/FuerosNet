//! **Where the rows an operator asserts about its horizon are kept.**
//!
//! A node learns its horizon two ways. The one that was always here is the
//! replay: transactions arrive, each is judged against this node's own
//! reach, and what passes is stored. The one this file serves is the
//! author's direction of 2026-11-04 — *"I want the trust horizon to
//! function more like a database table on both the node and the light
//! client"* — where the operator's client posts a row per member and
//! later updates or deletes it, **carrying no reason**, because the node
//! runs under that operator's authority and is not being persuaded.
//!
//! **Not the topology store, and not derived from it.** The store is this
//! node's seen-set, which a node that forgets replays a forwarding wave
//! over (`infra-client-requirements.md` §4.3). These are assertions with
//! no transaction behind them: they bind a member so it can attach and are
//! marked [`EvidenceStatus::Unevaluated`] — the design's own words, *"a
//! position, not yet a weighed edge"*. The same distinction the archive
//! already has from the store [author, 2026-09-13].
//!
//! **So they persist, for the reason grants do.** An act that did not
//! survive a restart would not be an act: a member who could attach until
//! the node was restarted is worse than one who never could, because the
//! operator watched it work. One file per row, named by the row's own
//! identity, so an act rewrites the one row it is about.
//!
//! **A retraction leaves a tombstone rather than removing the file**, and
//! that is not tidiness. The revision is what makes two of the operator's
//! devices converge, and a retraction whose revision was forgotten at the
//! next start could be undone by an insert that was in flight when it
//! happened. The row goes; the revision stays.
//!
//! **The format is lines of `key = value` and is meant to be read by a
//! later version than wrote it** [author, 2026-11-04: a retraction "needs
//! to be universal and forward compatible because it is not guaranteed
//! that two users' nodes will be running the same version of the node
//! software"]. An unknown key is ignored rather than refused, so a row
//! written by a newer node still binds here, and a tombstone written by
//! one still retracts.

use crate::hosting::{Refused, at};
use rhtn_archive::Keyhash;
use rhtn_archive::topology::Asserted;
use std::path::{Path, PathBuf};

/// A row as it is held here: the binding, and the key material that lets
/// the member be authenticated when it turns up.
///
/// **The material is the part a keyhash cannot replace.** A keyhash is
/// taken over the classical and post-quantum keys together, and the
/// transport presents only the classical one, so `Pins::keyhash_for_key`
/// resolves a connection by comparing the presented key against stored
/// material. Without it a row names a member the node could never
/// recognise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The binding asserted.
    pub row: Asserted,
    /// The member's key material, or none where this is a tombstone.
    pub material: Option<Vec<u8>>,
    /// Whether this row was retracted: the revision stands, the binding
    /// does not.
    pub retracted: bool,
}

/// The file a row lives in: its three-part identity, lower-case hex and
/// the series in decimal, so an operator reading the directory can see
/// which row is which.
#[must_use]
pub fn path_for(dir: &Path, node: &Keyhash, patron: &Keyhash, series: u32) -> PathBuf {
    dir.join(format!("{}.{}.{series}.row", hex(node), hex(patron)))
}

fn hex(k: &[u8; 32]) -> String {
    k.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok())
        .collect()
}

fn key(s: &str) -> Option<Keyhash> {
    unhex(s).and_then(|v| <[u8; 32]>::try_from(v).ok())
}

/// Write a row, or the tombstone a retraction leaves.
///
/// Beside-then-rename, so a reader never sees half a row and a crash
/// between the two leaves the previous one intact.
pub fn write(dir: &Path, r: &Row) -> Result<(), Refused> {
    std::fs::create_dir_all(dir).map_err(|e| at(0, format!("{}: {e}", dir.display())))?;
    let path = path_for(dir, &r.row.node, &r.row.patron, r.row.series);
    let mut text = String::from(
        "# A row this node's operator asserted about its horizon.  **Written\n\
         # by the node**: an act on the operator's surface rewrites this file\n\
         # whole.  An unknown key here is ignored, so a row a later version\n\
         # wrote still binds.\n",
    );
    text.push_str(&format!("node = {}\n", hex(&r.row.node)));
    text.push_str(&format!("patron = {}\n", hex(&r.row.patron)));
    text.push_str(&format!("series = {}\n", r.row.series));
    text.push_str(&format!("revision = {}\n", r.row.revision));
    text.push_str(&format!("retracted = {}\n", r.retracted));
    if !r.retracted {
        text.push_str(&format!("adoption = {}\n", hex(&r.row.adoption)));
        text.push_str(&format!("from = {}\n", r.row.from));
        if let Some(a) = r.row.anchor {
            text.push_str(&format!("anchor = {}\n", hex(&a)));
        }
        if let Some(s) = r.row.slot {
            text.push_str(&format!("slot = {s}\n"));
        }
        if let Some(m) = &r.material {
            text.push_str(&format!(
                "material = {}\n",
                m.iter().fold(String::new(), |mut s, b| {
                    s.push_str(&format!("{b:02x}"));
                    s
                })
            ));
        }
    }
    let beside = path.with_extension("row.new");
    std::fs::write(&beside, &text).map_err(|e| at(0, format!("{}: {e}", beside.display())))?;
    std::fs::rename(&beside, &path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    Ok(())
}

/// Read one row's file.
///
/// **An unknown key is ignored and a missing one refuses**, which is the
/// forward-compatible half: a row carrying fields this build has no name
/// for still binds, and one missing its identity is not a row at all.
pub fn read(path: &Path) -> Result<Row, Refused> {
    let text =
        std::fs::read_to_string(path).map_err(|e| at(0, format!("{}: {e}", path.display())))?;
    let mut node = None;
    let mut patron = None;
    let mut series = None;
    let mut revision = None;
    let mut retracted = false;
    let mut adoption = [0u8; 32];
    let mut from = 0u64;
    let mut anchor = None;
    let mut slot = None;
    let mut material = None;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(at(n + 1, "a line is `key = value`"));
        };
        let (k, v) = (k.trim(), v.trim());
        match k {
            "node" => node = key(v),
            "patron" => patron = key(v),
            "series" => series = v.parse::<u32>().ok(),
            "revision" => revision = v.parse::<u64>().ok(),
            "retracted" => retracted = v == "true",
            "adoption" => {
                adoption = key(v).ok_or_else(|| at(n + 1, "an adoption is 64 hex digits"))?
            }
            "from" => from = v.parse::<u64>().unwrap_or(0),
            "anchor" => anchor = key(v),
            "slot" => slot = v.parse::<u8>().ok(),
            "material" => material = unhex(v),
            // **ignored, on purpose**: a later version's field
            _ => {}
        }
    }
    let (Some(node), Some(patron), Some(series), Some(revision)) = (node, patron, series, revision)
    else {
        return Err(at(
            0,
            "a row needs `node`, `patron`, `series` and `revision`",
        ));
    };
    Ok(Row {
        row: Asserted {
            node,
            patron,
            series,
            adoption,
            from,
            anchor,
            slot,
            revision,
        },
        material,
        retracted,
    })
}

/// Every row the directory holds, tombstones included, in no order the
/// caller should rely on. A file that cannot be read is skipped and said,
/// rather than stopping a node from starting: one unreadable row is a
/// member who cannot attach, and refusing to start is every member.
pub fn read_all(dir: &Path) -> (Vec<Row>, Vec<Refused>) {
    let mut rows = Vec::new();
    let mut refused = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return (rows, refused);
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "row") {
            continue;
        }
        match read(&p) {
            Ok(r) => rows.push(r),
            Err(e) => refused.push(e),
        }
    }
    (rows, refused)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(revision: u64, retracted: bool) -> Row {
        Row {
            row: Asserted {
                node: [0x11; 32],
                patron: [0x22; 32],
                series: 3,
                adoption: [0x33; 32],
                from: 1700,
                anchor: Some([0x44; 32]),
                slot: Some(7),
                revision,
            },
            material: (!retracted).then(|| vec![0xab; 2003]),
            retracted,
        }
    }

    #[test]
    fn a_row_survives_the_round_trip_whole() {
        let d = std::env::temp_dir().join(format!("rhtn-horizon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let r = row(5, false);
        write(&d, &r).expect("written");
        let back = read(&path_for(&d, &r.row.node, &r.row.patron, r.row.series)).expect("read");
        assert_eq!(back, r, "every field, the material included");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_tombstone_keeps_the_revision_and_drops_the_binding() {
        // **why it is a tombstone and not a deletion**: the revision is what
        // stops a second device's in-flight insert resurrecting the member
        let d = std::env::temp_dir().join(format!("rhtn-horizon-t-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        write(&d, &row(5, false)).expect("the row");
        write(&d, &row(6, true)).expect("the tombstone");
        let back = read(&path_for(&d, &[0x11; 32], &[0x22; 32], 3)).expect("read");
        assert!(back.retracted);
        assert_eq!(back.row.revision, 6, "the revision stands");
        assert!(back.material.is_none(), "and nothing of the member is kept");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_field_this_build_has_no_name_for_is_ignored_rather_than_refused() {
        // **the author's forward-compatibility requirement** [2026-11-04]: two
        // operators' nodes will not run the same build, and a retraction in
        // particular "needs to be universal and forward compatible"
        let d = std::env::temp_dir().join(format!("rhtn-horizon-f-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("x.row");
        std::fs::write(
            &p,
            "node = 1111111111111111111111111111111111111111111111111111111111111111\n\
             patron = 2222222222222222222222222222222222222222222222222222222222222222\n\
             series = 3\n\
             revision = 9\n\
             retracted = true\n\
             standing_hint = whatever-a-later-version-means-by-this\n",
        )
        .unwrap();
        let back = read(&p).expect("an unknown key must not refuse the row");
        assert_eq!(back.row.revision, 9);
        assert!(back.retracted, "the retraction is honoured regardless");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_row_without_its_identity_is_not_a_row() {
        let d = std::env::temp_dir().join(format!("rhtn-horizon-b-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("y.row");
        std::fs::write(&p, "series = 3\nrevision = 1\n").unwrap();
        assert!(read(&p).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn one_unreadable_row_does_not_cost_a_node_its_other_members() {
        let d = std::env::temp_dir().join(format!("rhtn-horizon-a-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        write(&d, &row(1, false)).expect("a good row");
        std::fs::write(d.join("broken.row"), "this is not a row at all\n").unwrap();
        let (rows, refused) = read_all(&d);
        assert_eq!(rows.len(), 1, "the good row still loads");
        assert_eq!(refused.len(), 1, "and the bad one is said");
        let _ = std::fs::remove_dir_all(&d);
    }
}
