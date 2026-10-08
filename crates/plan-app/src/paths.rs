//! Where Plan Studio keeps its per-user files (`~/.plan-studio/`).
//!
//! No `dirs` crate: the home directory comes from `HOME` (macOS, Linux), else
//! `USERPROFILE` (Windows), else `HOMEDRIVE` + `HOMEPATH`.

use std::ffi::OsString;
use std::path::PathBuf;

/// The folder name under the home directory.
pub const SETTINGS_DIR: &str = ".plan-studio";

/// The home directory according to `get` (an environment lookup), skipping
/// empty values.
pub fn home_from(get: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let nonempty = |name: &str| get(name).filter(|v| !v.is_empty());
    if let Some(h) = nonempty("HOME").or_else(|| nonempty("USERPROFILE")) {
        return Some(PathBuf::from(h));
    }
    let drive = nonempty("HOMEDRIVE")?;
    let path = nonempty("HOMEPATH")?;
    let mut full = drive;
    full.push(path);
    Some(PathBuf::from(full))
}

/// The current user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    home_from(|k| std::env::var_os(k))
}

/// `~/.plan-studio/<file>`, or `None` when no home directory is known.
pub fn user_file(file: &str) -> Option<PathBuf> {
    home_dir().map(|h| h.join(SETTINGS_DIR).join(file))
}

/// The wording of the "cannot save" errors.
pub const NO_HOME: &str = "no home directory (HOME / USERPROFILE is not set)";

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let m: HashMap<String, OsString> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), OsString::from(v)))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn home_prefers_home_then_userprofile_then_drive_and_path() {
        assert_eq!(
            home_from(env(&[("HOME", "/h"), ("USERPROFILE", "C:\\u")])),
            Some(PathBuf::from("/h"))
        );
        assert_eq!(
            home_from(env(&[("USERPROFILE", "C:\\Users\\dan")])),
            Some(PathBuf::from("C:\\Users\\dan"))
        );
        assert_eq!(
            home_from(env(&[("HOME", ""), ("USERPROFILE", "C:\\Users\\dan")])),
            Some(PathBuf::from("C:\\Users\\dan"))
        );
        assert_eq!(
            home_from(env(&[("HOMEDRIVE", "C:"), ("HOMEPATH", "\\Users\\dan")])),
            Some(PathBuf::from("C:\\Users\\dan"))
        );
        assert_eq!(home_from(env(&[])), None);
        assert_eq!(home_from(env(&[("HOMEDRIVE", "C:")])), None);
    }
}
