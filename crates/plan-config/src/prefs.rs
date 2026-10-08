//! Chief preferences: a minimal INI reader and the drawing-related values
//! Plan Studio cares about.

use crate::normalize_newlines;
use serde::{Deserialize, Serialize};

/// Object-snap switches and bumping, as in Chief's Preferences > Snaps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SnapPrefs {
    pub endpoints: bool,
    pub midpoints: bool,
    pub center: bool,
    pub quadrant: bool,
    pub intersections: bool,
    pub tangents: bool,
    pub perpendicular: bool,
    pub on_object: bool,
    /// Orthogonal/extension snapping (INI key `Cad Snap to Orthogonal`).
    pub extension: bool,
    pub bumping: bool,
    pub bumping_distance: f64,
}

/// The preference values Plan Studio imports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChiefPreferences {
    pub color_theme: String,
    pub background_rgb: [u8; 3],
    pub layout_background_rgb: [u8; 3],
    pub crosshair_rgb: [u8; 3],
    pub snaps: SnapPrefs,
    pub crosshair_on: bool,
    pub autosave_on: bool,
    pub default_plan_template: String,
    pub default_layout_template: String,
    /// `(setting name, weight)` pairs for every `... Line Weight` key.
    pub line_weights: Vec<(String, u32)>,
}

/// Daniel's X18 values, from `docs/daniel-chief-setup.md` section 5
/// (read from `Chief Architect Premier X18.ini` on 2026-10-07).
///
/// `on_object` and `extension` are not named in that inventory; both are left
/// on, which is Chief's own default.
pub fn daniel_x18() -> ChiefPreferences {
    ChiefPreferences {
        color_theme: "Smoke 2 -Daniel Allen Design".into(),
        background_rgb: [245, 241, 239],
        layout_background_rgb: [249, 248, 244],
        crosshair_rgb: [0, 0, 255],
        snaps: SnapPrefs {
            endpoints: true,
            midpoints: true,
            center: true,
            quadrant: true,
            intersections: true,
            tangents: true,
            perpendicular: false,
            on_object: true,
            extension: true,
            bumping: true,
            bumping_distance: 5.0,
        },
        crosshair_on: false,
        autosave_on: true,
        default_plan_template: "x17 Working Template 2025-08-20.plan".into(),
        default_layout_template: "18x24 PRESENTATION LAYOUT TEMPLATE.layout".into(),
        line_weights: vec![
            ("Minimum Display Line Weight".into(), 1),
            ("Layout Edge Line Weight".into(), 18),
            ("Layout Pattern Line Weight".into(), 10),
            ("Pattern Tile Line Weight".into(), 10),
        ],
    }
}

// ---------------------------------------------------------------------------
// INI
// ---------------------------------------------------------------------------

/// One `[section]` and its `key=value` lines in file order.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct IniSection {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// A parsed INI file.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct IniFile {
    pub sections: Vec<IniSection>,
}

fn norm_key(k: &str) -> String {
    k.trim().replace("%20", " ").to_lowercase()
}

impl IniFile {
    pub fn section(&self, name: &str) -> Option<&IniSection> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Value of `key` in `section`; keys compare case-insensitively.
    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let want = norm_key(key);
        self.section(section)?
            .entries
            .iter()
            .find(|(k, _)| norm_key(k) == want)
            .map(|(_, v)| v.as_str())
    }

    /// First value of `key` in any section (the `%General` section first).
    pub fn find(&self, key: &str) -> Option<&str> {
        let want = norm_key(key);
        let mut order: Vec<&IniSection> = self.sections.iter().collect();
        order.sort_by_key(|s| s.name != "%General");
        order
            .into_iter()
            .flat_map(|s| s.entries.iter())
            .find(|(k, _)| norm_key(k) == want)
            .map(|(_, v)| v.as_str())
    }

    /// First value found for any of `keys`.
    pub fn find_any(&self, keys: &[&str]) -> Option<&str> {
        keys.iter().find_map(|k| self.find(k))
    }

    /// Every `(key, value)` whose key ends with `suffix` (case-insensitive).
    pub fn entries_ending_with(&self, suffix: &str) -> Vec<(&str, &str)> {
        let suffix = suffix.to_lowercase();
        self.sections
            .iter()
            .flat_map(|s| s.entries.iter())
            .filter(|(k, _)| norm_key(k).ends_with(&suffix))
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
    }
}

/// Parses INI text. Comment lines start with `;` or `#`; a leading BOM and
/// `\r\n` line endings are tolerated; surrounding double quotes on a value are
/// removed. Keys before the first `[section]` go into a section named `""`.
pub fn parse_ini(text: &str) -> IniFile {
    let text = normalize_newlines(text);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    let mut ini = IniFile::default();
    let mut current: Option<usize> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            ini.sections.push(IniSection {
                name: name.to_string(),
                entries: Vec::new(),
            });
            current = Some(ini.sections.len() - 1);
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let idx = *current.get_or_insert_with(|| {
            ini.sections.push(IniSection::default());
            ini.sections.len() - 1
        });
        let v = v.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|x| x.strip_suffix('"'))
            .unwrap_or(v);
        ini.sections[idx]
            .entries
            .push((k.trim().to_string(), v.to_string()));
    }
    ini
}

// ---------------------------------------------------------------------------
// INI -> preferences
// ---------------------------------------------------------------------------

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Reads `(R, G, B)`, `R,G,B,A` or `R G B`; extra components (alpha) are ignored.
fn parse_rgb(v: &str) -> Option<[u8; 3]> {
    let nums: Vec<u32> = v
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse().ok())
        .collect();
    if nums.len() < 3 || nums[..3].iter().any(|&n| n > 255) {
        return None;
    }
    Some([nums[0] as u8, nums[1] as u8, nums[2] as u8])
}

/// Builds preferences from a parsed INI. Keys the file does not have keep
/// [`daniel_x18`]'s values, so a partial or older INI still yields a complete
/// result.
///
/// Keys read (all case-insensitive, any section, `%General` first):
/// `selected color theme`, `Background Color`, `Layout Background Color`,
/// `Cross Hair Color`, `Cross Hair On`, `Autosave`, `Default Plan Template`,
/// `Default Layout Template`, `Cad Snap to End Points / Mid Points / Center /
/// Quadrant / Intersections / Tangents / Perpendicular / On Object /
/// Orthogonal`, `Bumping On`, `Bump Distance`, and every key ending in
/// `Line Weight`.
pub fn preferences_from_ini(ini: &IniFile) -> ChiefPreferences {
    let mut p = daniel_x18();
    let boolean = |keys: &[&str], slot: &mut bool| {
        if let Some(v) = ini.find_any(keys).and_then(parse_bool) {
            *slot = v;
        }
    };
    let color = |key: &str, slot: &mut [u8; 3]| {
        if let Some(v) = ini.find(key).and_then(parse_rgb) {
            *slot = v;
        }
    };
    let text = |keys: &[&str], slot: &mut String| {
        if let Some(v) = ini.find_any(keys).filter(|v| !v.is_empty()) {
            *slot = v.to_string();
        }
    };

    text(
        &["selected color theme", "Selected Color Theme"],
        &mut p.color_theme,
    );
    color("Background Color", &mut p.background_rgb);
    color("Layout Background Color", &mut p.layout_background_rgb);
    color("Cross Hair Color", &mut p.crosshair_rgb);
    boolean(&["Cross Hair On"], &mut p.crosshair_on);
    boolean(&["Autosave"], &mut p.autosave_on);
    text(&["Default Plan Template"], &mut p.default_plan_template);
    text(&["Default Layout Template"], &mut p.default_layout_template);

    let s = &mut p.snaps;
    boolean(&["Cad Snap to End Points"], &mut s.endpoints);
    boolean(&["Cad Snap to Mid Points"], &mut s.midpoints);
    boolean(&["Cad Snap to Center"], &mut s.center);
    boolean(&["Cad Snap to Quadrant"], &mut s.quadrant);
    boolean(&["Cad Snap to Intersections"], &mut s.intersections);
    boolean(&["Cad Snap to Tangents"], &mut s.tangents);
    boolean(&["Cad Snap to Perpendicular"], &mut s.perpendicular);
    boolean(
        &["Cad Snap to On Object", "Cad Snap to Object"],
        &mut s.on_object,
    );
    boolean(&["Cad Snap to Orthogonal"], &mut s.extension);
    boolean(&["Bumping On"], &mut s.bumping);
    if let Some(d) = ini
        .find("Bump Distance")
        .and_then(|v| v.trim().parse::<f64>().ok())
    {
        s.bumping_distance = d;
    }

    let weights: Vec<(String, u32)> = ini
        .entries_ending_with("line weight")
        .into_iter()
        .filter_map(|(k, v)| v.trim().parse().ok().map(|w| (k.trim().to_string(), w)))
        .collect();
    if !weights.is_empty() {
        p.line_weights = weights;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}; comment\r\n[%General]\r\nselected color theme=Night Owl\r\nBackground Color=(10, 20, 30)\r\nLayout Background Color=1,2,3,255\r\nCross Hair Color=@Variant(0 0 255)\r\nCross Hair On=true\r\nAutosave=0\r\nDefault Plan Template=\"My Plan.plan\"\r\nCad Snap to Perpendicular=true\r\nCad Snap to Mid Points=false\r\nBumping On=false\r\nBump Distance=7.5\r\nLayout Edge Line Weight=22\r\nPattern Tile Line Weight=4\r\n[Other]\r\nfoo=bar\r\n";

    #[test]
    fn parses_sections_and_values() {
        let ini = parse_ini(SAMPLE);
        assert_eq!(ini.sections.len(), 2);
        assert_eq!(ini.get("%General", "autosave"), Some("0"));
        assert_eq!(
            ini.get("%General", "Default Plan Template"),
            Some("My Plan.plan")
        );
        assert_eq!(ini.get("Other", "foo"), Some("bar"));
        assert_eq!(ini.find("FOO"), Some("bar"));
        assert_eq!(ini.get("%General", "missing"), None);
    }

    #[test]
    fn maps_known_keys_and_keeps_defaults_for_the_rest() {
        let p = preferences_from_ini(&parse_ini(SAMPLE));
        assert_eq!(p.color_theme, "Night Owl");
        assert_eq!(p.background_rgb, [10, 20, 30]);
        assert_eq!(p.layout_background_rgb, [1, 2, 3]);
        assert_eq!(p.crosshair_rgb, [0, 0, 255]);
        assert!(p.crosshair_on);
        assert!(!p.autosave_on);
        assert_eq!(p.default_plan_template, "My Plan.plan");
        assert_eq!(
            p.default_layout_template,
            daniel_x18().default_layout_template
        );
        assert!(p.snaps.perpendicular);
        assert!(!p.snaps.midpoints);
        assert!(p.snaps.endpoints);
        assert!(!p.snaps.bumping);
        assert!((p.snaps.bumping_distance - 7.5).abs() < 1e-9);
        assert_eq!(
            p.line_weights,
            vec![
                ("Layout Edge Line Weight".to_string(), 22),
                ("Pattern Tile Line Weight".to_string(), 4)
            ]
        );
    }

    #[test]
    fn empty_ini_gives_daniels_values() {
        assert_eq!(preferences_from_ini(&parse_ini("")), daniel_x18());
    }

    #[test]
    fn daniel_values_match_the_inventory() {
        let d = daniel_x18();
        assert_eq!(d.background_rgb, [245, 241, 239]);
        assert!(!d.snaps.perpendicular && d.snaps.bumping);
        assert!(!d.crosshair_on && d.autosave_on);
    }
}
