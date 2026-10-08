//! Reading and writing plan files (`.psplan`, pretty-printed JSON) safely.
//!
//! A save never edits the real file in place: the bytes go to a temporary
//! file in the same folder, are flushed to disk, and the temporary file is
//! then renamed over the real one. A crash, a full disk or a failed write
//! leaves the old file untouched and removes the temporary file.

use crate::Project;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// The temporary file a save of `path` writes first: `.<name>.tmp` beside it.
pub fn temp_path_for(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "plan".into());
    path.with_file_name(format!(".{name}.tmp"))
}

/// Writes `path` atomically: `write` fills a temporary file beside it, which
/// replaces `path` only when `write` and the flush both succeed. On any error
/// the temporary file is removed and `path` is left exactly as it was.
pub fn write_atomic_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let tmp = temp_path_for(path);
    let result = (|| {
        let mut f = File::create(&tmp)?;
        write(&mut f)?;
        f.flush()?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// [`write_atomic_with`] for bytes already in memory.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_atomic_with(path, |f| f.write_all(bytes))
}

/// Saves `project` to `path` atomically.
pub fn save_project(project: &Project, path: &Path) -> Result<(), String> {
    let json = project.to_json().map_err(|e| e.to_string())?;
    write_atomic(path, json.as_bytes()).map_err(|e| e.to_string())
}

/// Loads the plan at `path`.
pub fn load_project(path: &Path) -> Result<Project, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Project::from_json(&text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("plan-core-io-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn an_atomic_write_replaces_the_file_and_leaves_no_temporary() {
        let d = dir("ok");
        let f = d.join("a.psplan");
        write_atomic(&f, b"old").unwrap();
        write_atomic(&f, b"new").unwrap();
        assert_eq!(fs::read(&f).unwrap(), b"new");
        assert!(!temp_path_for(&f).exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_failed_write_keeps_the_old_file_and_removes_the_partial_one() {
        let d = dir("fail");
        let f = d.join("a.psplan");
        write_atomic(&f, b"old contents").unwrap();
        let err = write_atomic_with(&f, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("disk full"))
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "disk full");
        assert_eq!(fs::read(&f).unwrap(), b"old contents");
        assert!(!temp_path_for(&f).exists());
        // Nothing but the real file is left in the folder.
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_failed_first_save_creates_no_file_at_all() {
        let d = dir("first");
        let f = d.join("new.psplan");
        let r = write_atomic_with(&f, |_| Err(io::Error::other("nope")));
        assert!(r.is_err());
        assert_eq!(fs::read_dir(&d).unwrap().count(), 0);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn projects_save_and_load_back() {
        let d = dir("project");
        let f = d.join("p.psplan");
        let p = Project::new("Round trip");
        save_project(&p, &f).unwrap();
        assert_eq!(load_project(&f).unwrap().name, "Round trip");
        assert!(load_project(&d.join("missing.psplan")).is_err());
        let _ = fs::remove_dir_all(&d);
    }
}
