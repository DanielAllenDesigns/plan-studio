//! Where Chief keeps its default plan and layout templates.
//!
//! Chief stores them in its main preferences file
//! (`~/.config/Chief Architect Inc/Chief Architect Premier X18.ini`, section
//! `[%General]`) under the keys `Default Plan Template` and
//! `Default Layout Template` (written as `Default%20Plan%20Template`). X18
//! writes the bare file name, which lives in
//! `~/Documents/Chief Architect Premier X18 Data/Templates/`; X17 wrote the
//! full path. When the preference is missing or names a file that is not
//! there, [`detect_chief_templates`] looks for the stock names Daniel's setup
//! uses.

use crate::prefs::parse_ini;
use std::path::{Path, PathBuf};

/// The INI key naming the default plan template.
pub const PLAN_TEMPLATE_KEY: &str = "Default Plan Template";
/// The INI key naming the default layout template.
pub const LAYOUT_TEMPLATE_KEY: &str = "Default Layout Template";
/// Daniel's default plan template when no preference names one.
pub const FALLBACK_PLAN_TEMPLATE: &str = "x17 Working Template 2025-08-20.plan";
/// Daniel's default layout template when no preference names one.
pub const FALLBACK_LAYOUT_TEMPLATE: &str = "18x24 PRESENTATION LAYOUT TEMPLATE.layout";
/// Chief versions tried, newest first.
const VERSIONS: [&str; 2] = ["X18", "X17"];

/// The default templates found on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChiefTemplates {
    pub plan: Option<PathBuf>,
    pub layout: Option<PathBuf>,
    /// How each was found: `"Chief Architect Premier X18.ini"` or `"scan"`.
    pub plan_source: Option<String>,
    pub layout_source: Option<String>,
}

/// The Templates folder of one Chief version.
pub fn templates_folder(home: &Path, version: &str) -> PathBuf {
    home.join("Documents")
        .join(format!("Chief Architect Premier {version} Data"))
        .join("Templates")
}

/// The preference file of one Chief version.
pub fn preferences_file(home: &Path, version: &str) -> PathBuf {
    home.join(".config")
        .join("Chief Architect Inc")
        .join(format!("Chief Architect Premier {version}.ini"))
}

/// Turns a preference value into an existing file. A full path is used as
/// is when it exists (else its file name is tried in `dirs`); a bare name is
/// tried in each of `dirs` in order.
pub fn resolve_template(value: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let path = Path::new(value);
    if path.is_absolute() && path.is_file() {
        return Some(path.to_path_buf());
    }
    let name = path.file_name()?;
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// Folders that may hold templates: each version's Templates folder, newest
/// first.
fn template_dirs(home: &Path) -> Vec<PathBuf> {
    VERSIONS.iter().map(|v| templates_folder(home, v)).collect()
}

/// Finds a file called `name`: in the Templates folders, directly under
/// `~/Documents`, and in `Templates`/top level of each folder under it.
fn scan_for(home: &Path, name: &str) -> Option<PathBuf> {
    let docs = home.join("Documents");
    let direct = |dir: &Path| {
        let p = dir.join(name);
        p.is_file().then_some(p)
    };
    template_dirs(home)
        .iter()
        .find_map(|d| direct(d))
        .or_else(|| direct(&docs))
        .or_else(|| {
            let mut dirs: Vec<PathBuf> = std::fs::read_dir(&docs)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            dirs.sort();
            dirs.iter()
                .find_map(|d| direct(&d.join("Templates")).or_else(|| direct(d)))
        })
}

/// Reads Chief's preferences (X18, then X17) for the default templates and
/// falls back to scanning for the stock file names. Never fails; fields stay
/// `None` when nothing is found.
pub fn detect_chief_templates(home: &Path) -> ChiefTemplates {
    let mut out = ChiefTemplates::default();
    let dirs = template_dirs(home);
    for v in VERSIONS {
        let file = preferences_file(home, v);
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        let ini = parse_ini(&String::from_utf8_lossy(&bytes));
        let source = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // This version's own folder first.
        let own = templates_folder(home, v);
        let mut order = vec![own.clone()];
        order.extend(dirs.iter().filter(|d| **d != own).cloned());
        if out.plan.is_none() {
            if let Some(p) = ini
                .find(PLAN_TEMPLATE_KEY)
                .and_then(|val| resolve_template(val, &order))
            {
                out.plan = Some(p);
                out.plan_source = Some(source.clone());
            }
        }
        if out.layout.is_none() {
            if let Some(p) = ini
                .find(LAYOUT_TEMPLATE_KEY)
                .and_then(|val| resolve_template(val, &order))
            {
                out.layout = Some(p);
                out.layout_source = Some(source.clone());
            }
        }
    }
    if out.plan.is_none() {
        if let Some(p) = scan_for(home, FALLBACK_PLAN_TEMPLATE) {
            out.plan = Some(p);
            out.plan_source = Some("scan".into());
        }
    }
    if out.layout.is_none() {
        if let Some(p) = scan_for(home, FALLBACK_LAYOUT_TEMPLATE) {
            out.layout = Some(p);
            out.layout_source = Some("scan".into());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(tag: &str) -> PathBuf {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let p =
            std::env::temp_dir().join(format!("plan-config-tpl-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn touch(p: &Path) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn reads_bare_names_from_the_x18_preferences() {
        let home = temp_home("bare");
        let tpl = templates_folder(&home, "X18");
        touch(&tpl.join("My Plan.plan"));
        touch(&tpl.join("My Layout.layout"));
        let ini = preferences_file(&home, "X18");
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(
            &ini,
            "[%General]\r\nDefault%20Plan%20Template=My Plan.plan\r\nDefault%20Layout%20Template=My Layout.layout\r\n",
        )
        .unwrap();
        let t = detect_chief_templates(&home);
        assert_eq!(t.plan, Some(tpl.join("My Plan.plan")));
        assert_eq!(t.layout, Some(tpl.join("My Layout.layout")));
        assert_eq!(
            t.plan_source.as_deref(),
            Some("Chief Architect Premier X18.ini")
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn x17_full_paths_and_missing_files() {
        let home = temp_home("x17");
        let tpl = templates_folder(&home, "X17");
        touch(&tpl.join("Old.plan"));
        let ini = preferences_file(&home, "X17");
        std::fs::create_dir_all(ini.parent().unwrap()).unwrap();
        std::fs::write(
            &ini,
            format!(
                "[%General]\nDefault%20Plan%20Template={}\nDefault%20Layout%20Template=/nowhere/Gone.layout\n",
                tpl.join("Old.plan").display()
            ),
        )
        .unwrap();
        let t = detect_chief_templates(&home);
        assert_eq!(t.plan, Some(tpl.join("Old.plan")));
        assert_eq!(t.layout, None);
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn falls_back_to_scanning_for_the_stock_names() {
        let home = temp_home("scan");
        let other = home.join("Documents").join("Somewhere").join("Templates");
        touch(&other.join(FALLBACK_PLAN_TEMPLATE));
        touch(&home.join("Documents").join(FALLBACK_LAYOUT_TEMPLATE));
        let t = detect_chief_templates(&home);
        assert_eq!(t.plan, Some(other.join(FALLBACK_PLAN_TEMPLATE)));
        assert_eq!(t.plan_source.as_deref(), Some("scan"));
        assert_eq!(
            t.layout,
            Some(home.join("Documents").join(FALLBACK_LAYOUT_TEMPLATE))
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn nothing_found_is_not_an_error() {
        let home = temp_home("none");
        assert_eq!(detect_chief_templates(&home), ChiefTemplates::default());
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn resolve_rejects_empty_and_missing() {
        assert_eq!(resolve_template("  ", &[]), None);
        assert_eq!(resolve_template("nope.plan", &[std::env::temp_dir()]), None);
    }
}
