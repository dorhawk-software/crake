//! craked's own log file, where the system keeps nothing for a service:
//! Windows (craked.log beside its config) and macOS (craked.log in its
//! state folder; launchd keeps stderr only in a file every user can read,
//! never rotated, and the system log is every admin's). Linux has the
//! journal. The file sits in a folder that's root's (SYSTEM's) alone, like
//! the log of decisions, and starts again past `KEEP` bytes, the older
//! one kept as craked.log.1.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// How big the file grows before it starts again (checked as craked
/// starts: after an upgrade, a restart).
const KEEP: u64 = 8 * 1024 * 1024;

/// Where craked keeps it as a service, or none (Linux: the journal).
pub fn path() -> Option<PathBuf> {
    if cfg!(any(windows, target_os = "macos")) {
        Some(Path::new(crake_proto::LOG_PATH).with_file_name("craked.log"))
    } else {
        None
    }
}

/// Its folder closed to others (and to be made, if it isn't there).
pub fn lock_folder(path: &Path) -> std::io::Result<()> {
    path.parent().map_or(Ok(()), crate::sys::create_private_dir)
}

/// The file to append to, root's alone, started again if it has grown
/// past `KEEP`.
pub fn open(path: &Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > KEEP) {
        let mut older = path.as_os_str().to_owned();
        older.push(".1");
        std::fs::rename(path, older)?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, crate::sys::OWNER_ONLY);
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grown_log_starts_again_and_keeps_the_older_one() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("crake").join("craked.log");
        lock_folder(&log).unwrap();
        std::io::Write::write_all(&mut open(&log).unwrap(), b"first\n").unwrap();
        std::io::Write::write_all(&mut open(&log).unwrap(), b"second\n").unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "first\nsecond\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&log).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "root's alone");
        }
        let big = vec![b'x'; usize::try_from(KEEP).unwrap() + 1];
        std::fs::write(&log, &big).unwrap();
        std::io::Write::write_all(&mut open(&log).unwrap(), b"after\n").unwrap();
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "after\n");
        let older = dir.path().join("crake").join("craked.log.1");
        assert_eq!(std::fs::metadata(older).unwrap().len(), KEEP + 1);
    }

    #[test]
    fn kept_where_the_system_keeps_nothing() {
        let path = path();
        if cfg!(target_os = "linux") {
            assert_eq!(path, None, "the journal has it");
        } else {
            let path = path.expect("a file");
            assert_eq!(path.parent(), Path::new(crake_proto::LOG_PATH).parent());
        }
    }
}
