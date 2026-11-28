//! **A secret on the disk, written whole or not at all.**
//!
//! A key or a token is written beside its final name, owner-only from the
//! moment the file exists, flushed, and renamed over the name it is for,
//! and then the directory that names it is flushed. So a crash leaves
//! either the earlier file or the new one, never a truncated one read back
//! as nothing, and no other user is ever able to read it, not even between
//! a write and a `chmod`.

use std::io::Write as _;
use std::path::Path;

/// Write `bytes` to `path`, owner-only and atomically, creating the
/// directory where it is missing.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
    if let Some(d) = dir {
        std::fs::create_dir_all(d)?;
    }
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("a path with no file name"))?;
    // beside the final name, and hidden from a reader that lists the
    // directory for what it holds
    let tmp = path.with_file_name(format!(".{}.writing", name.to_string_lossy()));
    let _ = std::fs::remove_file(&tmp);
    let mut open = std::fs::OpenOptions::new();
    open.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        open.mode(0o600);
    }
    let written = open.open(&tmp).and_then(|mut f| {
        f.write_all(bytes)?;
        f.sync_all()
    });
    if let Err(e) = written.and_then(|()| std::fs::rename(&tmp, path)) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Some(d) = dir
        && let Ok(d) = std::fs::File::open(d)
    {
        let _ = d.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_is_written_whole_and_owner_only() {
        let d = std::env::temp_dir().join(format!("rhtn-durable-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let p = d.join("deep").join("token");
        write_private(&p, b"first").unwrap();
        write_private(&p, b"second").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let left: Vec<_> = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left, vec!["token"], "nothing is left beside it");
        let _ = std::fs::remove_dir_all(&d);
    }
}
