//! File operations: safe saves, archives, autosave, crash recovery, recent
//! files, drag and drop, and the unsaved-changes prompts.
//!
//! * **Saving** writes a temporary file and renames it over the plan
//!   (`plan_core::io`), after copying the version being replaced into
//!   `<plan folder>/Archives/<plan name>/` under a UTC time stamp. The newest
//!   [`FileSettings::archive_keep`] copies stay.
//! * **Autosave** writes `Archives/<plan name>/autosave.psplan` every
//!   [`FileSettings::autosave_minutes`] while the plan has unsaved changes and
//!   never touches the real file. Opening a plan whose autosave is newer than
//!   the file offers Recover or Discard.
//! * **Crash recovery**: a panic on the main thread (and an exit that skips
//!   the unsaved-changes prompt, such as Cmd+Q) writes
//!   `~/.plan-studio/recovery/recovery-<time>.psplan`; the next launch offers
//!   it.
//! * **Dirty tracking** hashes the serialized plan after each burst of
//!   changes and compares it with the hash of the last save, so undoing back
//!   to the saved state is clean again.
//!
//! The pure helpers (stamps, archive rotation, zip, argument parsing, titles)
//! come first and have tests; [`FileState`] and the `impl PlanApp` block drive
//! them from the window.

use crate::dialogs::unsaved::{self, Outcome, Prompt, Recovery, RecoveryPrompt};
use crate::dialogs::{app_info, preferences};
use crate::editor::EditorContext;
use crate::PlanApp;
use eframe::egui;
use plan_core::io as plan_io;
use plan_core::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// File extension of a plan.
pub const EXT: &str = "psplan";

// ----- command ids (Action::Custom) -----

pub const SAVE_COPY: &str = "file.save_copy";
pub const CLOSE: &str = "file.close";
pub const REVERT: &str = "file.revert";
pub const BACKUP: &str = "file.backup";
pub const CLEAR_RECENT: &str = "file.clear_recent";
pub const ARCHIVES: &str = "file.archives";

/// Is `id` one of the File-menu commands run by [`PlanApp::file_command`]?
pub fn is_command(id: &str) -> bool {
    [SAVE_COPY, CLOSE, REVERT, BACKUP, CLEAR_RECENT, ARCHIVES].contains(&id)
}

// ----- settings -----

/// The `"files"` key of `~/.plan-studio/settings.json`.
pub const SETTINGS_KEY: &str = "files";

/// How plans are archived and autosaved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FileSettings {
    /// Write an autosave while there are unsaved changes.
    pub autosave: bool,
    /// Minutes between autosaves.
    pub autosave_minutes: u32,
    /// Archive copies kept per plan.
    pub archive_keep: usize,
}

impl Default for FileSettings {
    fn default() -> Self {
        Self {
            autosave: true,
            autosave_minutes: 5,
            archive_keep: 20,
        }
    }
}

impl FileSettings {
    /// Settings with out-of-range values pulled back.
    pub fn clamped(mut self) -> Self {
        self.autosave_minutes = self.autosave_minutes.clamp(1, 120);
        self.archive_keep = self.archive_keep.clamp(1, 500);
        self
    }
}

/// The `files` key of the settings file at `path`.
pub fn read_settings_at(path: &Path) -> Option<FileSettings> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    serde_json::from_value::<FileSettings>(v.get(SETTINGS_KEY)?.clone())
        .ok()
        .map(FileSettings::clamped)
}

/// Writes the `files` key of the settings file at `path`, keeping the others.
pub fn write_settings_at(path: &Path, s: &FileSettings) -> Result<(), String> {
    let mut v = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}));
    v[SETTINGS_KEY] = serde_json::to_value(s).map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    plan_io::write_atomic(path, text.as_bytes()).map_err(|e| e.to_string())
}

// ----- time -----

/// Year, month, day, hour, minute, second (UTC) of `t`.
pub fn civil(t: SystemTime) -> (i64, u32, u32, u32, u32, u32) {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400) as u32;
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, rem / 3600, rem % 3600 / 60, rem % 60)
}

/// `20261008-140305`: the time stamp in archive and recovery file names (UTC).
pub fn stamp(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// `2026-10-08 14:03 UTC`, for the prompts.
pub fn pretty_when(t: SystemTime) -> String {
    let (y, mo, d, h, mi, _) = civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02} UTC")
}

/// "Saved 2 min ago" for the status bar; `ago` is the time since the last
/// save or open (None: never).
pub fn saved_label(ago: Option<Duration>, dirty: bool) -> String {
    let Some(ago) = ago else {
        return if dirty {
            "Not saved".into()
        } else {
            String::new()
        };
    };
    let secs = ago.as_secs();
    let when = if secs < 45 {
        "just now".to_string()
    } else if secs < 90 {
        "1 min ago".to_string()
    } else if secs < 3600 {
        format!("{} min ago", (secs + 30) / 60)
    } else if secs < 86_400 {
        format!("{} hr ago", secs / 3600)
    } else {
        format!("{} days ago", secs / 86_400)
    };
    if dirty {
        format!("Saved {when}, edited since")
    } else {
        format!("Saved {when}")
    }
}

/// The window title: `Plan Studio \u{2014} name.psplan \u{2022}` (the dot marks unsaved changes).
pub fn window_title(path: Option<&Path>, dirty: bool) -> String {
    let name = path.and_then(|p| p.file_name()).map_or_else(
        || "Untitled".to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    format!(
        "Plan Studio \u{2014} {name}{}",
        if dirty { " \u{2022}" } else { "" }
    )
}

// ----- archives -----

/// `<plan folder>/Archives/<plan name>/`.
pub fn archives_dir(plan: &Path) -> PathBuf {
    let stem = plan
        .file_stem()
        .map_or_else(|| "plan".to_string(), |s| s.to_string_lossy().into_owned());
    plan.parent()
        .unwrap_or_else(|| Path::new(""))
        .join("Archives")
        .join(stem)
}

fn plan_stem(plan: &Path) -> String {
    plan.file_stem()
        .map_or_else(|| "plan".to_string(), |s| s.to_string_lossy().into_owned())
}

/// The timed archive copies of `plan`, newest first (the autosave is not one).
pub fn list_archives(plan: &Path) -> Vec<PathBuf> {
    let prefix = format!("{}-", plan_stem(plan));
    let Ok(rd) = std::fs::read_dir(archives_dir(plan)) else {
        return Vec::new();
    };
    let mut v: Vec<((String, u32), PathBuf)> = rd
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            let stem = p.file_stem()?.to_string_lossy().into_owned();
            let key = archive_key(stem.strip_prefix(&prefix)?)?;
            p.extension().is_some_and(|x| x == EXT).then_some((key, p))
        })
        .collect();
    v.sort_by_key(|(key, _)| std::cmp::Reverse(key.clone()));
    v.into_iter().map(|(_, p)| p).collect()
}

/// The order of an archive copy from the part of its name after
/// `<name>-`: the time stamp (fixed width), then the copy counter of that
/// stamp (`20261008-140305` is copy 1, `20261008-140305-3` copy 3), so ten
/// copies in one second still sort by number.
fn archive_key(rest: &str) -> Option<(String, u32)> {
    let stamp_len = stamp(SystemTime::UNIX_EPOCH).len();
    let stamp_part = rest.get(..stamp_len)?;
    match rest.get(stamp_len..)? {
        "" => Some((stamp_part.to_string(), 1)),
        tail => {
            let n = tail.strip_prefix('-')?.parse().ok()?;
            Some((stamp_part.to_string(), n))
        }
    }
}

/// Deletes the oldest archive copies of `plan` beyond `keep`; how many went.
pub fn rotate_archives(plan: &Path, keep: usize) -> io::Result<usize> {
    let all = list_archives(plan);
    let mut removed = 0;
    for old in all.iter().skip(keep.max(1)) {
        std::fs::remove_file(old)?;
        removed += 1;
    }
    Ok(removed)
}

/// Copies the plan file now on disk into its archive folder as
/// `<name>-<stamp>.psplan`, then rotates. `None` when there is no file yet.
pub fn archive_previous(plan: &Path, stamp: &str, keep: usize) -> io::Result<Option<PathBuf>> {
    if !plan.is_file() {
        return Ok(None);
    }
    let dir = archives_dir(plan);
    std::fs::create_dir_all(&dir)?;
    let stem = plan_stem(plan);
    // A later copy of the same second gets a higher counter than any copy
    // still there (older ones may have rotated away), so the newest copy
    // always sorts first and is never the one rotated out.
    let prefix = format!("{stem}-");
    let newest_counter = list_archives(plan)
        .iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().into_owned();
            archive_key(name.strip_prefix(&prefix)?)
        })
        .filter(|(s, _)| s == stamp)
        .map(|(_, n)| n)
        .max();
    let mut n = newest_counter.map_or(1, |n| n + 1);
    let name = |n: u32| {
        if n == 1 {
            format!("{stem}-{stamp}.{EXT}")
        } else {
            format!("{stem}-{stamp}-{n}.{EXT}")
        }
    };
    let mut dest = dir.join(name(n));
    while dest.exists() {
        n += 1;
        dest = dir.join(name(n));
    }
    plan_io::write_atomic(&dest, &std::fs::read(plan)?)?;
    rotate_archives(plan, keep)?;
    Ok(Some(dest))
}

// ----- autosave and recovery -----

/// The file name of an autosave.
pub const AUTOSAVE_FILE: &str = "autosave.psplan";
/// The autosave of a plan nobody has saved yet.
pub const UNTITLED_AUTOSAVE: &str = "untitled-autosave.psplan";

/// `~/.plan-studio/recovery/`.
pub fn recovery_dir() -> Option<PathBuf> {
    crate::paths::user_file("recovery")
}

/// Where the autosave of `plan` goes.
pub fn autosave_path(plan: &Path) -> PathBuf {
    // Preferences > Folders > Autosave puts every plan's autosave in one
    // folder; the name keeps the plan apart from same-named plans elsewhere.
    if let Some(dir) = crate::dialogs::preferences::pages::folder(
        crate::dialogs::preferences::pages::FolderKind::Autosave,
    ) {
        return dir.join(shared_autosave_name(plan));
    }
    archives_dir(plan).join(AUTOSAVE_FILE)
}

/// The file name of `plan`'s autosave in the shared autosave folder:
/// `<plan name>-<hash of its path>-autosave.psplan`.
fn shared_autosave_name(plan: &Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    plan.hash(&mut h);
    format!(
        "{}-{:08x}-{AUTOSAVE_FILE}",
        plan_stem(plan),
        h.finish() as u32
    )
}

/// Is it time to autosave? Only while there are unsaved changes.
pub fn autosave_due(dirty: bool, enabled: bool, since_last: Duration, minutes: u32) -> bool {
    dirty && enabled && since_last >= Duration::from_secs(u64::from(minutes.max(1)) * 60)
}

/// Is an autosave modified after the plan it belongs to (a missing plan
/// counts as older)?
pub fn is_newer(autosave: SystemTime, plan: Option<SystemTime>) -> bool {
    plan.is_none_or(|p| autosave > p)
}

/// The autosave of `plan` when it is newer than the plan file.
pub fn autosave_newer_than_plan(plan: &Path) -> Option<PathBuf> {
    let auto = autosave_path(plan);
    let at = std::fs::metadata(&auto).ok()?.modified().ok()?;
    let saved = std::fs::metadata(plan).ok().and_then(|m| m.modified().ok());
    is_newer(at, saved).then_some(auto)
}

/// Crash recovery files in `dir` (and the untitled autosave), newest first.
pub fn find_recovery_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<(SystemTime, PathBuf)> = rd
        .filter_map(Result::ok)
        .filter_map(|e| {
            let p = e.path();
            let name = p.file_name()?.to_string_lossy().into_owned();
            let ours = (name.starts_with("recovery-") && name.ends_with(".psplan"))
                || name == UNTITLED_AUTOSAVE;
            let at = e.metadata().ok()?.modified().ok()?;
            ours.then_some((at, p))
        })
        .collect();
    v.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    v.into_iter().map(|(_, p)| p).collect()
}

/// How many crash recovery files are kept.
pub const RECOVERY_KEEP: usize = 10;

/// Writes `json` as `recovery-<stamp>.psplan` in `dir` and prunes the older
/// ones beyond [`RECOVERY_KEEP`].
pub fn write_recovery_to(dir: &Path, stamp: &str, json: &str) -> io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let file = dir.join(format!("recovery-{stamp}.{EXT}"));
    plan_io::write_atomic(&file, json.as_bytes())?;
    let mut old: Vec<PathBuf> = find_recovery_files(dir)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("recovery-"))
        })
        .collect();
    for p in old.drain(RECOVERY_KEEP.min(old.len())..) {
        let _ = std::fs::remove_file(p);
    }
    Ok(file)
}

// ----- the panic snapshot -----

struct Snapshot {
    json: String,
    dirty: bool,
}

static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);

/// Remembers the plan as of the last change check for the panic hook.
fn publish_snapshot(json: String, dirty: bool) {
    if let Ok(mut g) = SNAPSHOT.lock() {
        *g = Some(Snapshot { json, dirty });
    }
}

/// Forgets unsaved work on purpose (the user chose Don't Save).
fn clear_snapshot_dirty() {
    if let Ok(mut g) = SNAPSHOT.lock() {
        if let Some(s) = g.as_mut() {
            s.dirty = false;
        }
    }
}

/// Writes the snapshot to the recovery folder when it holds unsaved work.
fn write_snapshot_recovery() -> Option<PathBuf> {
    let g = SNAPSHOT.try_lock().ok()?;
    let s = g.as_ref().filter(|s| s.dirty)?;
    write_recovery_to(&recovery_dir()?, &stamp(SystemTime::now()), &s.json).ok()
}

/// Installs the top-level panic hook: a panic on the main thread (the editor;
/// the 3D workers catch their own) saves the unsaved plan to the recovery
/// folder before the default message prints.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("main") {
            if let Some(f) = write_snapshot_recovery() {
                eprintln!("Plan Studio kept your unsaved plan in {}", f.display());
            }
        }
        previous(info);
    }));
}

// ----- opening from the command line, Finder and drops -----

/// `%20`-style escapes decoded.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Is `p` a plan file by its extension?
pub fn is_plan_file(p: &Path) -> bool {
    p.extension().is_some_and(|e| e.eq_ignore_ascii_case(EXT))
}

/// The plan named on the command line (`plan-studio house.psplan`, what a
/// double-click passes on Windows and Linux, or a `file://` URL). Options and
/// macOS's `-psn_...` argument are skipped.
pub fn parse_open_args<I, S>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    args.into_iter().find_map(|a| {
        let a: OsString = a.into();
        let text = a.to_string_lossy();
        if text.starts_with('-') {
            return None;
        }
        let path = match text.strip_prefix("file://") {
            Some(rest) => PathBuf::from(percent_decode(
                rest.strip_prefix("localhost").unwrap_or(rest),
            )),
            None => PathBuf::from(&a),
        };
        is_plan_file(&path).then_some(path)
    })
}

/// The first plan among dropped files.
pub fn first_plan(paths: &[PathBuf]) -> Option<PathBuf> {
    paths.iter().find(|p| is_plan_file(p)).cloned()
}

// ----- zip (stored, no compression) -----

const fn crc_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[n] = c;
        n += 1;
    }
    t
}

static CRC_TABLE: [u32; 256] = crc_table();

/// CRC-32 (zip, PNG) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = CRC_TABLE[((c ^ u32::from(b)) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

fn dos_time_date(t: SystemTime) -> (u16, u16) {
    let (y, mo, d, h, mi, s) = civil(t);
    let y = y.clamp(1980, 2107) as u32;
    let date = ((y - 1980) << 9) | (mo << 5) | d;
    let time = (h << 11) | (mi << 5) | (s / 2);
    (time as u16, date as u16)
}

/// Writes a zip archive of `entries` (name, bytes), stored without
/// compression. Plan files are text and the images are already compressed.
pub fn write_zip(
    w: &mut impl Write,
    entries: &[(String, Vec<u8>)],
    when: SystemTime,
) -> io::Result<()> {
    let too_big = || {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the backup is too large for a zip file (4 GB)",
        )
    };
    let (time, date) = dos_time_date(when);
    let mut offset: u64 = 0;
    let mut central = Vec::new();
    let put = |w: &mut dyn Write, v: &[u8], offset: &mut u64| -> io::Result<()> {
        w.write_all(v)?;
        *offset += v.len() as u64;
        Ok(())
    };
    for (name, data) in entries {
        let size = u32::try_from(data.len()).map_err(|_| too_big())?;
        let start = u32::try_from(offset).map_err(|_| too_big())?;
        let crc = crc32(data);
        let mut head = Vec::new();
        head.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        head.extend_from_slice(&20u16.to_le_bytes());
        head.extend_from_slice(&0x0800u16.to_le_bytes());
        head.extend_from_slice(&0u16.to_le_bytes());
        head.extend_from_slice(&time.to_le_bytes());
        head.extend_from_slice(&date.to_le_bytes());
        head.extend_from_slice(&crc.to_le_bytes());
        head.extend_from_slice(&size.to_le_bytes());
        head.extend_from_slice(&size.to_le_bytes());
        head.extend_from_slice(&(name.len() as u16).to_le_bytes());
        head.extend_from_slice(&0u16.to_le_bytes());
        head.extend_from_slice(name.as_bytes());
        put(w, &head, &mut offset)?;
        put(w, data, &mut offset)?;
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0x0800u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&time.to_le_bytes());
        central.extend_from_slice(&date.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name.len() as u16).to_le_bytes());
        central.extend_from_slice(&[0u8; 12]);
        central.extend_from_slice(&start.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let cd_start = u32::try_from(offset).map_err(|_| too_big())?;
    let cd_size = u32::try_from(central.len()).map_err(|_| too_big())?;
    let count = u16::try_from(entries.len()).map_err(|_| too_big())?;
    w.write_all(&central)?;
    let mut end = Vec::new();
    end.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    end.extend_from_slice(&[0u8; 4]);
    end.extend_from_slice(&count.to_le_bytes());
    end.extend_from_slice(&count.to_le_bytes());
    end.extend_from_slice(&cd_size.to_le_bytes());
    end.extend_from_slice(&cd_start.to_le_bytes());
    end.extend_from_slice(&0u16.to_le_bytes());
    w.write_all(&end)
}

// ----- Backup Entire Plan -----

/// Picture and PDF paths the plan refers to (underlays, placed images,
/// library items), found by walking its JSON; only files that exist.
pub fn referenced_files(project: &Project) -> Vec<PathBuf> {
    let Ok(v) = serde_json::to_value(project) else {
        return Vec::new();
    };
    let mut found = BTreeSet::new();
    collect_paths(&v, &mut found, 0);
    found
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .collect()
}

fn looks_like_asset(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    [
        ".png", ".jpg", ".jpeg", ".pdf", ".gif", ".bmp", ".tif", ".tiff",
    ]
    .iter()
    .any(|e| l.ends_with(e))
}

fn collect_paths(v: &serde_json::Value, out: &mut BTreeSet<String>, depth: u32) {
    match v {
        serde_json::Value::Object(m) => {
            for (k, val) in m {
                if k == "path" {
                    if let Some(s) = val.as_str().filter(|s| looks_like_asset(s)) {
                        out.insert(s.to_string());
                    }
                }
                collect_paths(val, out, depth);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_paths(x, out, depth)),
        serde_json::Value::String(s) => {
            if let Some(p) = s.strip_prefix(plan_core::images::IMAGE_TAG_PREFIX) {
                if looks_like_asset(p) {
                    out.insert(p.to_string());
                }
            }
            // Some records hold their own JSON as text.
            if depth < 3 {
                if let Some(i) = s.find('{') {
                    if let Ok(inner) = serde_json::from_str::<serde_json::Value>(&s[i..]) {
                        collect_paths(&inner, out, depth + 1);
                    }
                }
            }
        }
        _ => {}
    }
}

/// A file-system-safe version of a plan name.
pub fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | ' ' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim().trim_matches('.').to_string();
    if s.is_empty() {
        "plan".into()
    } else {
        s
    }
}

/// What a backup made.
#[derive(Debug, PartialEq, Eq)]
pub struct BackupReport {
    pub file: PathBuf,
    pub assets: usize,
    pub missing: Vec<PathBuf>,
}

/// Zips the plan (`plan_json`) and the files it refers to into
/// `<dest>/<name>-backup-<stamp>.zip`.
pub fn backup_plan(
    name: &str,
    plan_json: &str,
    assets: &[PathBuf],
    dest: &Path,
    now: SystemTime,
) -> io::Result<BackupReport> {
    let base = safe_name(name);
    let mut entries = vec![(format!("{base}.{EXT}"), plan_json.as_bytes().to_vec())];
    let mut readme = format!(
        "Plan Studio backup of \"{name}\" made {}.\n\nThe plan refers to pictures by their original paths. \
         Copies of the files that were found are in assets/ (number-prefixed):\n\n",
        pretty_when(now)
    );
    let mut missing = Vec::new();
    let mut included = 0;
    for (i, a) in assets.iter().enumerate() {
        match std::fs::read(a) {
            Ok(bytes) => {
                let file = a
                    .file_name()
                    .map_or_else(|| "file".to_string(), |n| n.to_string_lossy().into_owned());
                let entry = format!("assets/{:02}-{}", i + 1, file);
                readme.push_str(&format!("{entry}  <-  {}\n", a.display()));
                entries.push((entry, bytes));
                included += 1;
            }
            Err(_) => missing.push(a.clone()),
        }
    }
    for m in &missing {
        readme.push_str(&format!("(not found, not included)  {}\n", m.display()));
    }
    entries.push(("README.txt".into(), readme.into_bytes()));
    std::fs::create_dir_all(dest)?;
    let file = dest.join(format!("{base}-backup-{}.zip", stamp(now)));
    plan_io::write_atomic_with(&file, |f| write_zip(f, &entries, now))?;
    Ok(BackupReport {
        file,
        assets: included,
        missing,
    })
}

// ----- state -----

/// What the user asked for that may need the unsaved-changes prompt first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    New,
    /// Open a plan: the path, or None to show the file dialog.
    Open(Option<PathBuf>),
    /// Close the plan (back to an empty one).
    Close,
    Quit,
    Revert,
    /// File > Import > Chief Plan...: the import replaces the open plan.
    ImportChief,
}

impl Pending {
    fn verb(&self) -> &'static str {
        match self {
            Pending::New => "start a new plan",
            Pending::Open(_) => "open another plan",
            Pending::Close => "close it",
            Pending::Quit => "quit",
            Pending::Revert => "revert",
            Pending::ImportChief => "import a Chief plan",
        }
    }
}

/// A recovered copy offered to the user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryOffer {
    pub file: PathBuf,
    /// The plan the work belongs to (None: an untitled plan).
    pub plan: Option<PathBuf>,
    pub crash: bool,
}

fn hash_text(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// How soon after a change the plan is re-serialized to check it.
const CHECK_GAP: Duration = Duration::from_millis(400);

/// Everything the window remembers about the file being edited.
pub struct FileState {
    pub settings: FileSettings,
    /// Hash of the plan as last saved or opened.
    saved_hash: u64,
    dirty: bool,
    /// Recovered work that has not been saved yet.
    force_dirty: bool,
    checked: Option<(u64, u64)>,
    checked_at: Option<Instant>,
    saved_at: Option<Instant>,
    last_autosave: Instant,
    /// The autosave file this session wrote and not yet removed.
    autosave_file: Option<PathBuf>,
    /// Files to delete after the next successful save (recovered copies).
    cleanup: Vec<PathBuf>,
    prompt: Option<(Prompt, Pending)>,
    ready: Option<Pending>,
    recovery: Option<RecoveryOffer>,
    allow_close: bool,
    discard_on_exit: bool,
    /// Only the real window scans the home folder (tests leave this off).
    startup_scan: bool,
    started: bool,
    startup_path: Option<PathBuf>,
    untitled_autosave: Option<PathBuf>,
    last_title: String,
    pub show_archives: bool,
}

impl Default for FileState {
    fn default() -> Self {
        Self {
            settings: FileSettings::default(),
            saved_hash: 0,
            dirty: false,
            force_dirty: false,
            checked: None,
            checked_at: None,
            saved_at: None,
            last_autosave: Instant::now(),
            autosave_file: None,
            cleanup: Vec::new(),
            prompt: None,
            ready: None,
            recovery: None,
            allow_close: false,
            discard_on_exit: false,
            startup_scan: false,
            started: false,
            startup_path: None,
            untitled_autosave: None,
            last_title: String::new(),
            show_archives: false,
        }
    }
}

impl FileState {
    /// Called once by `main`: loads the settings, notes the file named on the
    /// command line and turns on the startup recovery scan.
    pub fn begin_startup(&mut self, args: impl IntoIterator<Item = OsString>) {
        if let Some(s) = preferences::settings_path().and_then(|p| read_settings_at(&p)) {
            self.settings = s;
        }
        self.startup_path = parse_open_args(args);
        self.untitled_autosave = recovery_dir().map(|d| d.join(UNTITLED_AUTOSAVE));
        self.startup_scan = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// A prompt is open: the rest of the window should not take keys.
    pub fn modal_open(&self) -> bool {
        self.prompt.is_some() || self.recovery.is_some()
    }

    /// The plan in `cx` is now the saved one (opened, saved, new).
    pub fn rebaseline(&mut self, cx: &EditorContext) {
        let json = cx.project.to_json().unwrap_or_default();
        self.saved_hash = hash_text(&json);
        self.dirty = false;
        self.force_dirty = false;
        self.checked = Some(cx.cache_key());
        self.checked_at = Some(Instant::now());
        self.saved_at = Some(Instant::now());
        self.last_autosave = Instant::now();
        publish_snapshot(json, false);
    }

    /// Re-checks whether the plan differs from the saved one. Returns true
    /// while a check is still waiting (a drag in progress, or a change too
    /// recent), so the caller can ask for another frame.
    pub fn update_dirty(
        &mut self,
        cx: &EditorContext,
        has_path: bool,
        pointer_down: bool,
        now: Instant,
    ) -> bool {
        let key = cx.cache_key();
        if self.checked == Some(key) {
            return false;
        }
        if pointer_down
            || self
                .checked_at
                .is_some_and(|t| now.duration_since(t) < CHECK_GAP)
        {
            return true;
        }
        let json = cx.project.to_json().unwrap_or_default();
        let hash = hash_text(&json);
        if !has_path && !cx.can_undo() && !self.force_dirty {
            // An untouched new plan stays clean (a template scan may reseed it).
            self.saved_hash = hash;
        }
        self.dirty = self.force_dirty || hash != self.saved_hash;
        self.checked = Some(key);
        self.checked_at = Some(now);
        publish_snapshot(json, self.dirty);
        false
    }

    /// Checks the plan against the saved one now, ignoring the usual delay
    /// after a change (a request that may lose work needs the true answer).
    pub fn settle(&mut self, cx: &EditorContext, has_path: bool) {
        self.checked_at = None;
        self.update_dirty(cx, has_path, false, Instant::now());
    }

    /// Nothing is unsaved any more.
    fn mark_saved(&mut self, cx: &EditorContext) {
        self.rebaseline(cx);
    }

    /// Is `p` cleared to run (no unsaved changes stood in its way)?
    #[cfg(test)]
    pub(crate) fn is_ready_for(&self, p: &Pending) -> bool {
        self.ready.as_ref() == Some(p)
    }

    /// Whether any request is waiting on the user.
    pub fn busy(&self) -> bool {
        self.prompt.is_some() || self.recovery.is_some()
    }

    /// Exit without the prompt (Cmd+Q on macOS, a killed window): keep
    /// unsaved work in the recovery folder.
    pub fn on_exit(&mut self, cx: &EditorContext) {
        if self.discard_on_exit {
            return;
        }
        let json = cx.project.to_json().unwrap_or_default();
        let changed = self.force_dirty || hash_text(&json) != self.saved_hash;
        let untouched_new = self.untitled_autosave.is_some() && !cx.can_undo() && !self.force_dirty;
        if changed && !untouched_new {
            publish_snapshot(json, true);
            let _ = write_snapshot_recovery();
        }
    }
}

// ----- the window side -----

impl PlanApp {
    /// The plan's name for prompts and titles.
    fn plan_label(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map_or_else(
            || "Untitled".to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    /// Asks for `p`; when the plan has unsaved changes the Save / Don't Save /
    /// Cancel prompt comes first.
    pub(crate) fn request_file_action(&mut self, p: Pending) {
        if self.files.busy() {
            return;
        }
        self.files.settle(&self.cx, self.path.is_some());
        if p == Pending::Revert && self.path.is_none() {
            self.cx.status =
                "This plan has not been saved yet; there is nothing to revert to".into();
            return;
        }
        let skip = p == Pending::Revert
            && crate::dialogs::preferences::pages::dont_ask(unsaved::REVERT_KEY);
        if self.files.dirty && !skip {
            let prompt = if p == Pending::Revert {
                Prompt::revert(&self.plan_label())
            } else {
                Prompt::unsaved(&self.plan_label(), p.verb())
            };
            self.files.prompt = Some((prompt, p));
        } else {
            self.files.ready = Some(p);
        }
    }

    /// Runs the File-menu commands of this module.
    pub(crate) fn file_command(&mut self, id: &str) {
        match id {
            SAVE_COPY => self.save_a_copy(),
            CLOSE => self.request_file_action(Pending::Close),
            REVERT => self.request_file_action(Pending::Revert),
            BACKUP => self.backup_entire_plan(),
            CLEAR_RECENT => {
                app_info::clear_recent();
                self.cx.status = "Cleared the recent plans list".into();
            }
            ARCHIVES => self.files.show_archives = !self.files.show_archives,
            _ => {}
        }
    }

    /// Opens the plan file at `path` (File > Open and Open Recent). Offers
    /// the autosave when it is newer than the file.
    pub(crate) fn open_path(&mut self, path: PathBuf) {
        match plan_io::load_project(&path) {
            Ok(p) if !p.floors.is_empty() => {
                self.cx.set_project(p);
                // The saved snap, Edit Type and Replicate defaults go over the plan's.
                crate::dialogs::preferences::pages::apply_editing(&mut self.cx.defaults.editing);
                self.cx.status = format!("Opened {}", path.display());
                app_info::push_recent(&path);
                self.path = Some(path.clone());
                self.tools.restart(&mut self.cx);
                self.files.rebaseline(&self.cx);
                if let Some(auto) = autosave_newer_than_plan(&path) {
                    self.files.recovery = Some(RecoveryOffer {
                        file: auto,
                        plan: Some(path),
                        crash: false,
                    });
                }
            }
            Ok(_) => self.cx.status = "File contains no floors".into(),
            Err(e) => self.cx.status = format!("Open failed: {e}"),
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &[EXT])
            .pick_file()
        {
            self.open_path(path);
        }
    }

    pub(crate) fn save_project(&mut self) {
        self.try_save();
    }

    pub(crate) fn save_project_as(&mut self) {
        self.try_save_as();
    }

    /// Saves to the plan's file (Save As when it has none). True when saved.
    pub(crate) fn try_save(&mut self) -> bool {
        match self.path.clone() {
            Some(p) => self.write_to(p),
            None => self.try_save_as(),
        }
    }

    pub(crate) fn try_save_as(&mut self) -> bool {
        let suggested = format!("{}.{EXT}", safe_name(&self.cx.project.name));
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &[EXT])
            .set_file_name(suggested)
            .save_file()
        else {
            return false;
        };
        if path.extension().is_none() {
            path.set_extension(EXT);
        }
        self.write_to(path)
    }

    /// The safe save: archive the file being replaced, write a temporary file
    /// and rename it over the plan.
    pub(crate) fn write_to(&mut self, path: PathBuf) -> bool {
        let json = match self.cx.project.to_json() {
            Ok(j) => j,
            Err(e) => {
                self.cx.status = format!("Save failed: {e}");
                return false;
            }
        };
        let now = SystemTime::now();
        let archive_note =
            match archive_previous(&path, &stamp(now), self.files.settings.archive_keep) {
                Ok(_) => String::new(),
                Err(e) => format!(" (could not archive the old version: {e})"),
            };
        if let Err(e) = plan_io::write_atomic(&path, json.as_bytes()) {
            self.cx.status = format!("Save failed: {e}; the file on disk is unchanged");
            return false;
        }
        self.remove_autosaves(Some(&path));
        for f in std::mem::take(&mut self.files.cleanup) {
            let _ = std::fs::remove_file(f);
        }
        app_info::push_recent(&path);
        self.path = Some(path.clone());
        self.files.mark_saved(&self.cx);
        self.cx.status = format!("Saved {}{archive_note}", path.display());
        true
    }

    /// File > Save a Copy: writes the plan elsewhere and keeps working on the
    /// original.
    fn save_a_copy(&mut self) {
        let suggested = format!("{} copy.{EXT}", safe_name(&self.cx.project.name));
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &[EXT])
            .set_file_name(suggested)
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension(EXT);
        }
        self.cx.status = match plan_io::save_project(&self.cx.project, &path) {
            Ok(()) => format!("Saved a copy to {}", path.display()),
            Err(e) => format!("Save a copy failed: {e}"),
        };
    }

    /// File > Backup Entire Plan...: a zip of the plan and the pictures it
    /// uses, in a folder the user picks.
    fn backup_entire_plan(&mut self) {
        let Some(dest) = rfd::FileDialog::new()
            .set_title("Choose a folder for the backup")
            .pick_folder()
        else {
            return;
        };
        self.backup_to(&dest);
    }

    pub(crate) fn backup_to(&mut self, dest: &Path) {
        let json = match self.cx.project.to_json() {
            Ok(j) => j,
            Err(e) => {
                self.cx.status = format!("Backup failed: {e}");
                return;
            }
        };
        let assets = referenced_files(&self.cx.project);
        let name = self
            .path
            .as_ref()
            .map_or_else(|| self.cx.project.name.clone(), |p| plan_stem(p));
        self.cx.status = match backup_plan(&name, &json, &assets, dest, SystemTime::now()) {
            Ok(r) => {
                let missing = if r.missing.is_empty() {
                    String::new()
                } else {
                    format!(", {} picture(s) not found", r.missing.len())
                };
                format!(
                    "Backed up to {} ({} picture(s){missing})",
                    r.file.display(),
                    r.assets
                )
            }
            Err(e) => format!("Backup failed: {e}"),
        };
    }

    /// Deletes the autosave files of the current document and of `also`.
    fn remove_autosaves(&mut self, also: Option<&Path>) {
        let mut files: Vec<PathBuf> = Vec::new();
        files.extend(self.files.autosave_file.take());
        if let Some(p) = &self.path {
            files.push(autosave_path(p));
        }
        if let Some(p) = also {
            files.push(autosave_path(p));
        }
        files.extend(self.files.untitled_autosave.clone());
        for f in files {
            let _ = std::fs::remove_file(f);
        }
    }

    /// Runs a request once it is allowed.
    fn perform(&mut self, ctx: &egui::Context, p: Pending) {
        match p {
            Pending::New => {
                self.new_project();
            }
            Pending::Close => {
                self.new_project();
                self.cx.status = "Closed the plan".into();
            }
            Pending::Open(Some(path)) => self.open_path(path),
            Pending::Open(None) => self.open_dialog(),
            Pending::ImportChief => self.import_chief_plan(),
            Pending::Revert => {
                if let Some(path) = self.path.clone() {
                    self.open_path(path);
                    self.cx.status = format!("Reverted to the saved {}", self.plan_label());
                }
            }
            Pending::Quit => {
                self.files.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// The prompt answered: save first when asked, then carry on.
    fn answer_prompt(&mut self, answer: Outcome, p: Pending) {
        match answer {
            Outcome::Cancel => {}
            Outcome::Save => {
                if self.try_save() {
                    self.files.ready = Some(p);
                }
            }
            Outcome::DontSave => {
                self.remove_autosaves(None);
                if p == Pending::Quit {
                    self.files.discard_on_exit = true;
                    clear_snapshot_dirty();
                }
                self.files.ready = Some(p);
            }
        }
    }

    /// Looks for recovery files once at startup; opens the file named on the
    /// command line.
    fn start_files(&mut self) {
        if !self.files.startup_scan {
            return;
        }
        if let Some(path) = self.files.startup_path.take() {
            self.open_path(path);
            return;
        }
        let Some(dir) = recovery_dir() else {
            return;
        };
        if let Some(file) = find_recovery_files(&dir).into_iter().next() {
            self.files.recovery = Some(RecoveryOffer {
                file,
                plan: None,
                crash: true,
            });
        }
    }

    fn recovery_prompt(&self, offer: &RecoveryOffer) -> RecoveryPrompt {
        let when = std::fs::metadata(&offer.file)
            .and_then(|m| m.modified())
            .map_or_else(|_| "an earlier session".to_string(), pretty_when);
        let what = offer.plan.as_ref().and_then(|p| p.file_name()).map_or_else(
            || "an untitled plan".to_string(),
            |n| format!("\u{201C}{}\u{201D}", n.to_string_lossy()),
        );
        RecoveryPrompt {
            what,
            when,
            crash: offer.crash,
        }
    }

    fn answer_recovery(&mut self, offer: RecoveryOffer, answer: Recovery) {
        match answer {
            Recovery::Recover => match plan_io::load_project(&offer.file) {
                Ok(p) if !p.floors.is_empty() => {
                    self.cx.set_project(p);
                    self.path = offer.plan.clone();
                    self.tools.restart(&mut self.cx);
                    self.files.rebaseline(&self.cx);
                    self.files.force_dirty = true;
                    self.files.dirty = true;
                    self.files.cleanup.push(offer.file.clone());
                    self.cx.status = "Recovered the unsaved work; save it to keep it".into();
                }
                Ok(_) => self.cx.status = "The recovery file contains no floors".into(),
                Err(e) => self.cx.status = format!("Could not recover: {e}"),
            },
            Recovery::Discard => {
                let _ = std::fs::remove_file(&offer.file);
                if offer.crash {
                    // Older crash copies go with it; the next launch is quiet.
                    if let Some(dir) = recovery_dir() {
                        for f in find_recovery_files(&dir) {
                            let _ = std::fs::remove_file(f);
                        }
                    }
                }
            }
        }
    }

    /// Writes the autosave now (File > Manage Auto Archives and the timer).
    /// The real plan file is not touched.
    pub(crate) fn autosave_now(&mut self) -> Result<PathBuf, String> {
        let target = match (&self.path, &self.files.untitled_autosave) {
            (Some(p), _) => autosave_path(p),
            (None, Some(u)) => u.clone(),
            (None, None) => return Err("no place to keep an autosave of an untitled plan".into()),
        };
        let json = self.cx.project.to_json().map_err(|e| e.to_string())?;
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        plan_io::write_atomic(&target, json.as_bytes()).map_err(|e| e.to_string())?;
        self.files.autosave_file = Some(target.clone());
        Ok(target)
    }

    /// The autosave timer: writes when the plan has unsaved changes and the
    /// interval has passed. True when it wrote.
    pub(crate) fn tick_autosave(&mut self, now: Instant) -> bool {
        let s = &self.files.settings;
        if !autosave_due(
            self.files.dirty,
            s.autosave,
            now.duration_since(self.files.last_autosave),
            s.autosave_minutes,
        ) {
            return false;
        }
        self.files.last_autosave = now;
        match self.autosave_now() {
            Ok(_) => true,
            Err(e) => {
                self.cx.status = format!("Autosave failed: {e}");
                false
            }
        }
    }

    /// The status bar's "Saved 2 min ago".
    pub(crate) fn saved_status(&self, now: Instant) -> String {
        saved_label(
            self.files.saved_at.map(|t| now.duration_since(t)),
            self.files.dirty,
        )
    }

    /// Everything file-related that happens once a frame: startup, the close
    /// button, dropped files, dirty tracking, prompts, autosave, the title.
    pub(crate) fn drive_files(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        #[cfg(target_os = "macos")]
        {
            // Finder's open-document events (see `mac_open`): before the first
            // frame's startup they stand in for a command-line file.
            crate::mac_open::set_repaint(ctx);
            if let Some(p) = first_plan(&crate::mac_open::take_requests()) {
                if self.files.started {
                    self.request_file_action(Pending::Open(Some(p)));
                } else {
                    self.files.startup_path = Some(p);
                }
            }
        }
        if !self.files.started {
            self.files.started = true;
            self.start_files();
        }
        if ctx.input(|i| i.viewport().close_requested()) && !self.files.allow_close {
            self.files.settle(&self.cx, self.path.is_some());
            if self.files.dirty || self.files.busy() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.request_file_action(Pending::Quit);
            } else {
                // Nothing to lose: let the window close.
                self.files.allow_close = true;
            }
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        // Watch ~/Downloads for new material-package zips (when asked to).
        crate::tools::materials::package::tick(ctx);
        if !dropped.is_empty() {
            match first_plan(&dropped) {
                Some(p) => self.request_file_action(Pending::Open(Some(p))),
                // A dropped .zip is a material package (Lightbeans).
                None if crate::tools::materials::package::handle_dropped(
                    &mut self.cx,
                    &dropped,
                ) > 0 => {}
                None => {
                    self.cx.status =
                        "Drop a .psplan file to open it, or a material package .zip".into()
                }
            }
        }
        let down = ctx.input(|i| i.pointer.any_down());
        let has_path = self.path.is_some();
        if self.files.update_dirty(&self.cx, has_path, down, now) {
            ctx.request_repaint_after(CHECK_GAP);
        }
        if !self.files.dirty {
            // Back to the saved state: an older autosave is no longer news.
            if let Some(f) = self.files.autosave_file.take() {
                let _ = std::fs::remove_file(f);
            }
        }
        if let Some((prompt, pending)) = self.files.prompt.clone() {
            if let Some(answer) = unsaved::show(ctx, &prompt) {
                self.files.prompt = None;
                self.answer_prompt(answer, pending);
            }
        } else if let Some(offer) = self.files.recovery.clone() {
            let prompt = self.recovery_prompt(&offer);
            if let Some(answer) = unsaved::show_recovery(ctx, &prompt) {
                self.files.recovery = None;
                self.answer_recovery(offer, answer);
            }
        }
        if let Some(p) = self.files.ready.take() {
            self.perform(ctx, p);
        }
        self.tick_autosave(now);
        self.archives_window(ctx);
        let title = window_title(self.path.as_deref(), self.files.is_dirty());
        if title != self.files.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.files.last_title = title;
        }
        ctx.request_repaint_after(Duration::from_secs(30));
    }

    /// File > Manage Auto Archives...: autosave and archive settings, and the
    /// archive copies of this plan.
    fn archives_window(&mut self, ctx: &egui::Context) {
        if !self.files.show_archives {
            return;
        }
        let mut open = true;
        let before = self.files.settings.clone();
        let mut settings = before.clone();
        let mut open_archive: Option<PathBuf> = None;
        let mut backup = false;
        let archives = self.path.as_deref().map(list_archives).unwrap_or_default();
        let folder = self.path.as_deref().map(archives_dir);
        egui::Window::new("Manage Auto Archives")
            .id(egui::Id::new("manage_auto_archives"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.checkbox(
                    &mut settings.autosave,
                    "Autosave while there are unsaved changes",
                );
                ui.horizontal(|ui| {
                    ui.label("Every");
                    ui.add(egui::DragValue::new(&mut settings.autosave_minutes).range(1..=120));
                    ui.label("minutes");
                });
                ui.horizontal(|ui| {
                    ui.label("Keep the last");
                    ui.add(egui::DragValue::new(&mut settings.archive_keep).range(1..=500));
                    ui.label("archive copies of each plan");
                });
                ui.separator();
                match &folder {
                    Some(f) => {
                        ui.label(format!("Archives of this plan ({})", f.display()));
                        if archives.is_empty() {
                            ui.weak(
                                "None yet; a copy is kept each time you save over a saved plan.",
                            );
                        }
                        for a in archives.iter().take(10) {
                            ui.horizontal(|ui| {
                                ui.label(
                                    a.file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_default(),
                                );
                                if ui.small_button("Open").clicked() {
                                    open_archive = Some(a.clone());
                                }
                            });
                        }
                        if ui.button("Show Archives Folder").clicked() {
                            let _ = app_info::open_external(&f.to_string_lossy());
                        }
                    }
                    None => {
                        ui.weak("Save the plan to start its archive.");
                    }
                }
                if ui.button("Back Up Entire Plan\u{2026}").clicked() {
                    backup = true;
                }
            });
        let settings = settings.clamped();
        if settings != before {
            if let Some(p) = preferences::settings_path() {
                let _ = write_settings_at(&p, &settings);
            }
            self.files.settings = settings;
        }
        self.files.show_archives = open;
        if backup {
            self.backup_entire_plan();
        }
        if let Some(a) = open_archive {
            self.request_file_action(Pending::Open(Some(a)));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use crate::theme::AppSettings;
    use std::fs;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("plan-files-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn app() -> PlanApp {
        PlanApp::new(AppSettings::default(), plan_defaults::embedded(), None)
    }

    /// Makes an edit the dirty check sees.
    fn edit(a: &mut PlanApp) {
        a.cx.begin_change("Rename");
        a.cx.project.name = format!("{} edited", a.cx.project.name);
        a.cx.mark_dirty();
        a.files.settle(&a.cx, a.path.is_some());
        assert!(a.files.is_dirty());
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn stamps_are_utc_and_sortable() {
        assert_eq!(stamp(at(0)), "19700101-000000");
        // 2026-10-08 14:03:05 UTC
        assert_eq!(stamp(at(1_791_468_185)), "20261008-140305");
        assert_eq!(pretty_when(at(1_791_468_185)), "2026-10-08 14:03 UTC");
        // A leap day.
        assert_eq!(stamp(at(1_709_208_000)), "20240229-120000");
    }

    #[test]
    fn the_title_carries_the_name_and_the_dirty_marker() {
        let p = Path::new("/plans/house.psplan");
        assert_eq!(
            window_title(Some(p), false),
            "Plan Studio \u{2014} house.psplan"
        );
        assert_eq!(
            window_title(Some(p), true),
            "Plan Studio \u{2014} house.psplan \u{2022}"
        );
        assert_eq!(
            window_title(None, true),
            "Plan Studio \u{2014} Untitled \u{2022}"
        );
    }

    #[test]
    fn saved_labels_read_like_a_person() {
        let s = |n| Some(Duration::from_secs(n));
        assert_eq!(saved_label(s(5), false), "Saved just now");
        assert_eq!(saved_label(s(60), false), "Saved 1 min ago");
        assert_eq!(saved_label(s(125), false), "Saved 2 min ago");
        assert_eq!(saved_label(s(7300), false), "Saved 2 hr ago");
        assert_eq!(saved_label(s(125), true), "Saved 2 min ago, edited since");
        assert_eq!(saved_label(None, true), "Not saved");
        assert_eq!(saved_label(None, false), "");
    }

    #[test]
    fn rotation_keeps_only_the_newest_n_archives() {
        let d = dir("rotate");
        let plan = d.join("house.psplan");
        for i in 0..7u64 {
            fs::write(&plan, format!("version {i}")).unwrap();
            archive_previous(&plan, &stamp(at(1_700_000_000 + i * 10)), 3).unwrap();
        }
        let kept = list_archives(&plan);
        assert_eq!(kept.len(), 3);
        // Newest first; each copy holds the version it replaced.
        let text = |p: &PathBuf| fs::read_to_string(p).unwrap();
        assert_eq!(text(&kept[0]), "version 6");
        assert_eq!(text(&kept[2]), "version 4");
        assert!(kept[0].starts_with(d.join("Archives").join("house")));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn two_saves_in_one_second_keep_both_copies() {
        let d = dir("same-second");
        let plan = d.join("a.psplan");
        fs::write(&plan, "1").unwrap();
        let a = archive_previous(&plan, "20260101-000000", 20)
            .unwrap()
            .unwrap();
        fs::write(&plan, "2").unwrap();
        let b = archive_previous(&plan, "20260101-000000", 20)
            .unwrap()
            .unwrap();
        assert_ne!(a, b);
        assert_eq!(list_archives(&plan).len(), 2);
        // The newer copy sorts first.
        assert_eq!(list_archives(&plan)[0], b);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn a_first_save_has_nothing_to_archive_and_autosaves_are_not_archives() {
        let d = dir("first");
        let plan = d.join("new.psplan");
        assert_eq!(
            archive_previous(&plan, "20260101-000000", 20).unwrap(),
            None
        );
        fs::create_dir_all(archives_dir(&plan)).unwrap();
        fs::write(autosave_path(&plan), "x").unwrap();
        assert!(list_archives(&plan).is_empty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn autosave_is_due_only_when_dirty_enabled_and_the_interval_passed() {
        let five = Duration::from_secs(300);
        assert!(autosave_due(true, true, five, 5));
        assert!(
            !autosave_due(false, true, five * 3, 5),
            "a clean plan is never autosaved"
        );
        assert!(!autosave_due(true, false, five * 3, 5));
        assert!(!autosave_due(true, true, five - Duration::from_secs(1), 5));
    }

    #[test]
    fn the_autosave_folder_preference_moves_the_autosave() {
        use crate::dialogs::preferences::pages::{self, FolderKind};
        let plan = Path::new("/projects/a/house.psplan");
        let other = Path::new("/projects/b/house.psplan");
        let beside = autosave_path(plan);
        assert!(beside.starts_with("/projects/a/Archives/house"));
        pages::update(|p| p.folders.set(FolderKind::Autosave, "/tmp/ps-autosaves"));
        let shared = autosave_path(plan);
        assert!(shared.starts_with("/tmp/ps-autosaves"));
        let name = shared.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("house-") && name.ends_with("-autosave.psplan"));
        assert_ne!(shared, autosave_path(other), "same-named plans stay apart");
        pages::update(|p| p.folders.set(FolderKind::Autosave, ""));
        assert_eq!(autosave_path(plan), beside);
    }

    #[test]
    fn autosave_writes_only_when_dirty_and_never_touches_the_real_file() {
        let d = dir("autosave");
        let plan = d.join("house.psplan");
        let mut a = app();
        assert!(a.write_to(plan.clone()));
        let saved = fs::read(&plan).unwrap();
        let later = Instant::now() + Duration::from_secs(3600);
        // Clean: nothing is written however much time passes.
        assert!(!a.tick_autosave(later));
        assert!(!autosave_path(&plan).exists());
        // Dirty: the timer writes the autosave beside the archives.
        a.cx.begin_change("Rename");
        a.cx.project.name = "Edited".into();
        a.cx.mark_dirty();
        a.files.checked_at = None;
        assert!(!a.files.update_dirty(&a.cx, true, false, Instant::now()));
        assert!(a.files.is_dirty());
        assert!(a.tick_autosave(later));
        let auto = autosave_path(&plan);
        assert!(fs::read_to_string(&auto).unwrap().contains("Edited"));
        assert_eq!(
            fs::read(&plan).unwrap(),
            saved,
            "the real file is untouched"
        );
        // Saving clears the autosave.
        assert!(a.write_to(plan.clone()));
        assert!(!auto.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn recovery_is_offered_only_when_the_autosave_is_newer_than_the_plan() {
        assert!(is_newer(at(200), Some(at(100))));
        assert!(!is_newer(at(100), Some(at(200))));
        assert!(!is_newer(at(100), Some(at(100))));
        assert!(is_newer(at(100), None));

        let d = dir("recover");
        let plan = d.join("house.psplan");
        fs::write(&plan, "{}").unwrap();
        let auto = autosave_path(&plan);
        fs::create_dir_all(auto.parent().unwrap()).unwrap();
        fs::write(&auto, "{}").unwrap();
        let set = |p: &Path, t: SystemTime| {
            fs::File::options()
                .write(true)
                .open(p)
                .unwrap()
                .set_modified(t)
                .unwrap()
        };
        set(&plan, at(2_000_000_000));
        set(&auto, at(1_900_000_000));
        assert_eq!(
            autosave_newer_than_plan(&plan),
            None,
            "an older autosave is ignored"
        );
        set(&auto, at(2_100_000_000));
        assert_eq!(autosave_newer_than_plan(&plan), Some(auto));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn opening_a_plan_with_a_newer_autosave_offers_recovery_and_recover_loads_it() {
        let d = dir("open-recover");
        let plan = d.join("house.psplan");
        let mut a = app();
        assert!(a.write_to(plan.clone()));
        // A later session's autosave of an edited plan.
        let mut edited = a.cx.project.clone();
        edited.name = "From the autosave".into();
        let auto = autosave_path(&plan);
        fs::create_dir_all(auto.parent().unwrap()).unwrap();
        fs::write(&auto, edited.to_json().unwrap()).unwrap();
        let t = fs::metadata(&plan).unwrap().modified().unwrap() + Duration::from_secs(60);
        fs::File::options()
            .write(true)
            .open(&auto)
            .unwrap()
            .set_modified(t)
            .unwrap();

        let mut b = app();
        b.open_path(plan.clone());
        let offer = b.files.recovery.clone().expect("an offer");
        assert_eq!(offer.file, auto);
        b.files.recovery = None;
        b.answer_recovery(offer, Recovery::Recover);
        assert_eq!(b.cx.project.name, "From the autosave");
        assert!(b.files.is_dirty(), "recovered work counts as unsaved");
        assert_eq!(b.path.as_deref(), Some(plan.as_path()));
        // Saving removes the recovered copy.
        assert!(b.write_to(plan.clone()));
        assert!(!auto.exists());
        assert!(!b.files.is_dirty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn discarding_a_recovery_offer_deletes_the_file() {
        let d = dir("discard");
        let f = d.join("autosave.psplan");
        fs::write(&f, "{}").unwrap();
        let mut a = app();
        a.answer_recovery(
            RecoveryOffer {
                file: f.clone(),
                plan: None,
                crash: false,
            },
            Recovery::Discard,
        );
        assert!(!f.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn crash_recovery_files_are_written_found_newest_first_and_pruned() {
        let d = dir("crash");
        for i in 0..13u64 {
            write_recovery_to(&d, &stamp(at(1_700_000_000 + i)), &format!("{{\"n\":{i}}}"))
                .unwrap();
            // Distinct modification times so "newest" is well defined.
            let f = d.join(format!("recovery-{}.psplan", stamp(at(1_700_000_000 + i))));
            fs::File::options()
                .write(true)
                .open(&f)
                .unwrap()
                .set_modified(at(1_700_000_000 + i))
                .unwrap();
        }
        let found = find_recovery_files(&d);
        assert_eq!(found.len(), RECOVERY_KEEP);
        assert!(fs::read_to_string(&found[0]).unwrap().contains("\"n\":12"));
        fs::write(d.join(UNTITLED_AUTOSAVE), "{}").unwrap();
        assert_eq!(find_recovery_files(&d).len(), RECOVERY_KEEP + 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn save_archives_the_old_version_and_a_failed_save_changes_nothing() {
        let d = dir("save");
        let plan = d.join("house.psplan");
        let mut a = app();
        a.files.settings.archive_keep = 2;
        for i in 0..4 {
            a.cx.project.name = format!("v{i}");
            assert!(a.write_to(plan.clone()));
        }
        assert_eq!(
            list_archives(&plan).len(),
            2,
            "only the newest two copies stay"
        );
        assert!(fs::read_to_string(&plan).unwrap().contains("v3"));
        // The folder must not hold a leftover temporary file.
        let names: Vec<String> = fs::read_dir(&d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
        // A save into a folder that does not exist fails and says so.
        let bad = d.join("missing").join("x.psplan");
        assert!(!a.write_to(bad.clone()));
        assert!(a.cx.status.starts_with("Save failed"));
        assert!(!bad.exists());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn dirty_tracking_follows_edits_undo_and_saves() {
        let d = dir("dirty");
        let plan = d.join("p.psplan");
        let mut a = app();
        assert!(a.write_to(plan.clone()));
        let tick = |a: &mut PlanApp| {
            a.files.checked_at = None;
            a.files.update_dirty(&a.cx, true, false, Instant::now());
        };
        tick(&mut a);
        assert!(!a.files.is_dirty());
        a.cx.begin_change("Rename");
        a.cx.project.name = "Changed".into();
        a.cx.mark_dirty();
        tick(&mut a);
        assert!(a.files.is_dirty());
        // A drag in progress defers the check.
        a.cx.mark_dirty();
        assert!(a.files.update_dirty(&a.cx, true, true, Instant::now()));
        a.cx.undo();
        tick(&mut a);
        assert!(
            !a.files.is_dirty(),
            "undoing back to the saved plan is clean"
        );
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn an_untouched_new_plan_is_clean_even_after_a_template_reseed() {
        let mut a = app();
        a.files.rebaseline(&a.cx);
        let p = a.cx.project.clone();
        let mut other = p.clone();
        other.name = "Reseeded".into();
        a.cx.set_project(other);
        a.files.checked_at = None;
        a.files.update_dirty(&a.cx, false, false, Instant::now());
        assert!(!a.files.is_dirty());
        // Once the user does something it counts.
        a.cx.begin_change("Rename");
        a.cx.project.name = "Mine".into();
        a.cx.mark_dirty();
        a.files.checked_at = None;
        a.files.update_dirty(&a.cx, false, false, Instant::now());
        assert!(a.files.is_dirty());
    }

    #[test]
    fn unsaved_plans_prompt_and_clean_plans_go_straight_through() {
        let mut a = app();
        a.request_file_action(Pending::New);
        assert_eq!(a.files.ready.take(), Some(Pending::New));
        edit(&mut a);
        a.request_file_action(Pending::Quit);
        assert!(a.files.ready.is_none());
        let (prompt, pending) = a.files.prompt.clone().expect("the prompt");
        assert_eq!(pending, Pending::Quit);
        assert!(prompt.message().contains("quit"));
        // Cancel keeps the plan and the window.
        a.files.prompt = None;
        a.answer_prompt(Outcome::Cancel, Pending::Quit);
        assert!(a.files.ready.is_none());
        // Don't Save carries on.
        a.answer_prompt(Outcome::DontSave, Pending::New);
        assert_eq!(a.files.ready.take(), Some(Pending::New));
        // A second request while a prompt is open is ignored.
        a.request_file_action(Pending::Close);
        a.request_file_action(Pending::New);
        assert_eq!(
            a.files.prompt.as_ref().map(|(_, p)| p.clone()),
            Some(Pending::Close)
        );
        // Revert needs a saved plan.
        a.files.prompt = None;
        a.request_file_action(Pending::Revert);
        assert!(a.cx.status.contains("nothing to revert"));
    }

    #[test]
    fn save_then_continue_runs_the_request() {
        let d = dir("save-continue");
        let plan = d.join("p.psplan");
        let mut a = app();
        a.path = Some(plan.clone());
        edit(&mut a);
        a.answer_prompt(Outcome::Save, Pending::New);
        assert!(plan.exists());
        assert_eq!(a.files.ready.take(), Some(Pending::New));
        assert!(!a.files.is_dirty());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn revert_reloads_the_saved_file() {
        let d = dir("revert");
        let plan = d.join("p.psplan");
        let mut a = app();
        a.cx.project.name = "Saved name".into();
        assert!(a.write_to(plan.clone()));
        a.cx.project.name = "Scribbles".into();
        a.perform(&egui::Context::default(), Pending::Revert);
        assert_eq!(a.cx.project.name, "Saved name");
        assert!(a.cx.status.starts_with("Reverted"));
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn opening_a_plan_lays_the_saved_editing_defaults_over_it() {
        use crate::dialogs::preferences::pages;
        let d = dir("open-editing");
        let plan = d.join("p.psplan");
        let mut a = app();
        assert!(a.write_to(plan.clone()));
        let mut mine = a.cx.defaults.editing.clone();
        mine.snap_distance_px = 11.5;
        assert_ne!(a.cx.defaults.editing, mine);
        pages::remember_editing(&mine);
        a.open_path(plan);
        assert_eq!(a.cx.defaults.editing, mine);
        pages::update(|p| p.editing_saved = false);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn revert_asks_until_dont_ask_again_is_set() {
        use crate::dialogs::preferences::pages;
        let d = dir("revert-dont-ask");
        let plan = d.join("p.psplan");
        let mut a = app();
        assert!(a.write_to(plan));
        edit(&mut a);
        a.request_file_action(Pending::Revert);
        assert!(a.files.prompt.is_some(), "the confirmation shows first");
        assert!(a.files.ready.is_none());
        a.files.prompt = None;
        pages::set_dont_ask(unsaved::REVERT_KEY);
        a.request_file_action(Pending::Revert);
        assert!(a.files.prompt.is_none());
        assert_eq!(a.files.ready.take(), Some(Pending::Revert));
        // Reset Options shows the question again.
        pages::reset_dont_ask();
        a.request_file_action(Pending::Revert);
        assert!(a.files.prompt.is_some());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn recents_dedupe_and_cap() {
        let mut list: Vec<PathBuf> = Vec::new();
        for i in 0..14 {
            list = app_info::with_recent(&list, Path::new(&format!("/p/{i}.psplan")));
        }
        assert_eq!(list.len(), app_info::MAX_RECENT);
        list = app_info::with_recent(&list, Path::new("/p/9.psplan"));
        assert_eq!(list[0], Path::new("/p/9.psplan"));
        assert_eq!(list.iter().filter(|p| p.ends_with("9.psplan")).count(), 1);
        assert_eq!(list.len(), app_info::MAX_RECENT);
        // Clearing the list empties the menu.
        app_info::push_recent(Path::new("/definitely/missing.psplan"));
        app_info::clear_recent();
        assert!(app_info::recent_files().is_empty());
    }

    #[test]
    fn the_file_command_ids_are_recognised() {
        for id in [SAVE_COPY, CLOSE, REVERT, BACKUP, CLEAR_RECENT, ARCHIVES] {
            assert!(is_command(id));
        }
        assert!(!is_command("edit.undo"));
    }

    #[test]
    fn a_plan_named_on_the_command_line_is_found() {
        let p = |args: &[&str]| parse_open_args(args.iter().map(OsString::from));
        assert_eq!(
            p(&["/plans/house.psplan"]),
            Some(PathBuf::from("/plans/house.psplan"))
        );
        assert_eq!(
            p(&["-psn_0_12345", "/plans/House.PSPLAN"]),
            Some(PathBuf::from("/plans/House.PSPLAN"))
        );
        assert_eq!(p(&["--verbose", "notes.txt"]), None);
        assert_eq!(p(&[]), None);
        assert_eq!(
            p(&["file:///Users/dan/My%20Plans/a.psplan"]),
            Some(PathBuf::from("/Users/dan/My Plans/a.psplan"))
        );
        assert_eq!(
            first_plan(&[PathBuf::from("a.png"), PathBuf::from("b.psplan")]),
            Some(PathBuf::from("b.psplan"))
        );
        assert_eq!(first_plan(&[PathBuf::from("a.png")]), None);
    }

    #[test]
    fn settings_round_trip_keep_other_keys_and_clamp() {
        let d = dir("settings");
        let f = d.join("settings.json");
        fs::write(
            &f,
            r#"{"theme":"dark","files":{"archive_keep":0,"autosave_minutes":999}}"#,
        )
        .unwrap();
        let s = read_settings_at(&f).unwrap();
        assert_eq!(
            (s.archive_keep, s.autosave_minutes, s.autosave),
            (1, 120, true)
        );
        write_settings_at(&f, &FileSettings::default()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        assert_eq!(v["files"]["archive_keep"], 20);
        assert_eq!(read_settings_at(&d.join("none.json")), None);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    /// Reads back the entries of a stored zip: (name, bytes, stored crc).
    fn read_zip(z: &[u8]) -> Vec<(String, Vec<u8>, u32)> {
        let u16at = |o: usize| u16::from_le_bytes([z[o], z[o + 1]]) as usize;
        let u32at = |o: usize| u32::from_le_bytes([z[o], z[o + 1], z[o + 2], z[o + 3]]);
        let eocd = z.len() - 22;
        assert_eq!(u32at(eocd), 0x0605_4b50);
        let count = u16at(eocd + 10);
        let mut at = u32at(eocd + 16) as usize;
        let mut out = Vec::new();
        for _ in 0..count {
            assert_eq!(u32at(at), 0x0201_4b50);
            let crc = u32at(at + 16);
            let size = u32at(at + 24) as usize;
            let nlen = u16at(at + 28);
            let local = u32at(at + 42) as usize;
            let name = String::from_utf8(z[at + 46..at + 46 + nlen].to_vec()).unwrap();
            assert_eq!(u32at(local), 0x0403_4b50);
            let data_at = local + 30 + u16at(local + 26) + u16at(local + 28);
            out.push((name, z[data_at..data_at + size].to_vec(), crc));
            at += 46 + nlen;
        }
        out
    }

    #[test]
    fn the_zip_writer_stores_entries_with_good_checksums() {
        let mut buf = Vec::new();
        let entries = vec![
            ("a.txt".to_string(), b"hello".to_vec()),
            ("sub/b.bin".to_string(), vec![1, 2, 3, 4]),
        ];
        write_zip(&mut buf, &entries, at(1_791_468_185)).unwrap();
        let back = read_zip(&buf);
        assert_eq!(back.len(), 2);
        for ((n, d), (bn, bd, crc)) in entries.iter().zip(&back) {
            assert_eq!((n, d), (bn, bd));
            assert_eq!(*crc, crc32(d));
        }
    }

    #[test]
    fn a_backup_zips_the_plan_and_the_pictures_it_uses() {
        let d = dir("backup");
        let pic = d.join("site photo.png");
        fs::write(&pic, [0x89, b'P', b'N', b'G']).unwrap();
        let missing = d.join("gone.png");
        let mut project = Project::new("Back Me Up");
        project.floors[0]
            .underlays
            .push(plan_core::underlay::Underlay::new(
                "Site",
                pic.to_string_lossy().into_owned(),
                100,
                100,
                plan_core::Point::ZERO,
            ));
        let found = referenced_files(&project);
        assert_eq!(found, vec![pic.clone()]);
        let json = project.to_json().unwrap();
        let out = d.join("out");
        let report = backup_plan(
            "Back Me Up",
            &json,
            &[pic.clone(), missing.clone()],
            &out,
            at(1_791_468_185),
        )
        .unwrap();
        assert_eq!(report.assets, 1);
        assert_eq!(report.missing, vec![missing]);
        assert_eq!(
            report.file.file_name().unwrap(),
            "Back Me Up-backup-20261008-140305.zip"
        );
        let entries = read_zip(&fs::read(&report.file).unwrap());
        let names: Vec<&str> = entries.iter().map(|e| e.0.as_str()).collect();
        assert_eq!(
            names,
            [
                "Back Me Up.psplan",
                "assets/01-site photo.png",
                "README.txt"
            ]
        );
        assert_eq!(entries[0].1, json.as_bytes());
        assert_eq!(entries[1].1, vec![0x89, b'P', b'N', b'G']);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn backup_works_from_the_app_into_a_chosen_folder() {
        let d = dir("backup-app");
        let mut a = app();
        a.backup_to(&d);
        assert!(a.cx.status.starts_with("Backed up to"), "{}", a.cx.status);
        assert_eq!(fs::read_dir(&d).unwrap().count(), 1);
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn safe_names_drop_path_characters() {
        assert_eq!(safe_name("A/B: C?"), "A_B_ C_");
        assert_eq!(safe_name("  "), "plan");
        assert_eq!(safe_name("..."), "plan");
    }

    #[test]
    fn a_dropped_plan_opens_and_other_drops_are_refused() {
        let d = dir("drop");
        let plan = d.join("dropped.psplan");
        let mut other = Project::new("Dropped");
        other.floors = app().cx.project.floors.clone();
        plan_io::save_project(&other, &plan).unwrap();
        let dropped = |path: &Path| egui::RawInput {
            dropped_files: vec![egui::DroppedFile {
                path: Some(path.to_path_buf()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut a = app();
        let ctx = egui::Context::default();
        let _ = ctx.run(dropped(&plan), |ctx| a.drive_files(ctx));
        assert_eq!(a.cx.project.name, "Dropped");
        assert_eq!(a.path.as_deref(), Some(plan.as_path()));
        assert!(!a.files.is_dirty());
        let _ = ctx.run(dropped(&d.join("picture.png")), |ctx| a.drive_files(ctx));
        assert!(a.cx.status.contains("Drop a .psplan"));
        assert_eq!(a.cx.project.name, "Dropped");
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn the_close_button_asks_first_when_there_are_unsaved_changes() {
        let close = || {
            let mut raw = egui::RawInput::default();
            raw.viewports.insert(
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    events: vec![egui::ViewportEvent::Close],
                    ..Default::default()
                },
            );
            raw
        };
        let ctx = egui::Context::default();
        // Clean: the window may close (no prompt, no cancel).
        let mut a = app();
        let _ = ctx.run(close(), |ctx| a.drive_files(ctx));
        assert!(!a.files.modal_open());
        assert!(a.files.allow_close, "nothing to lose, so the window closes");
        // Dirty: the prompt comes first.
        let mut b = app();
        edit(&mut b);
        let _ = ctx.run(close(), |ctx| b.drive_files(ctx));
        assert!(b.files.modal_open());
        assert!(!b.files.allow_close);
    }

    #[test]
    fn the_archives_window_and_prompts_draw_headlessly() {
        let mut a = app();
        a.files.show_archives = true;
        a.files.prompt = Some((Prompt::unsaved("x.psplan", "quit"), Pending::Quit));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| a.drive_files(ctx));
        assert!(a.files.show_archives);
        assert!(a.files.modal_open());
    }
}
