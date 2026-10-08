//! Where Plan Studio's defaults come from: `~/.plan-studio/defaults.json` when
//! the user saved one ("Save Current Defaults as My Template"), else the
//! Chief X18 template embedded in the binary. Plus the small helpers that
//! turn [`PlanDefaults`] into the objects the editor places.

use plan_core::defaults::{WallLayer, WallTypeDef};
use plan_core::{Opening, PlanDefaults, WallKind};
use std::path::PathBuf;

/// Daniel's Chief X18 template (`assets/templates/chief-x18-daniel.json`).
pub const EMBEDDED_TEMPLATE: &str = include_str!("../assets/templates/chief-x18-daniel.json");

/// The embedded template; falls back to the in-code values if the file were
/// ever malformed (a unit test keeps the two identical).
pub fn embedded() -> PlanDefaults {
    PlanDefaults::from_json(EMBEDDED_TEMPLATE).unwrap_or_else(|_| PlanDefaults::chief_x18_daniel())
}

/// `~/.plan-studio/defaults.json`, or `None` when no home directory is known.
pub fn user_path() -> Option<PathBuf> {
    crate::paths::user_file("defaults.json")
}

/// The user's saved defaults if present, else the embedded template. The
/// second value is a note to show when the user's file could not be read.
pub fn load() -> (PlanDefaults, Option<String>) {
    let Some(path) = user_path().filter(|p| p.exists()) else {
        return (embedded(), None);
    };
    match PlanDefaults::load(&path) {
        Ok(d) => (d, None),
        Err(e) => (
            embedded(),
            Some(format!(
                "Could not read {} ({e}); using the Chief X18 template",
                path.display()
            )),
        ),
    }
}

/// Writes `d` as the user's template.
pub fn save_user(d: &PlanDefaults) -> Result<PathBuf, String> {
    let path = user_path().ok_or(crate::paths::NO_HOME)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = d.to_json().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Removes the user's template so the embedded one applies again.
pub fn clear_user() -> Result<(), String> {
    let Some(path) = user_path() else {
        return Ok(());
    };
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// The opening new doors are placed from (`exterior` for exterior walls).
pub fn door_template(d: &PlanDefaults, exterior: bool) -> Opening {
    let o = if exterior {
        &d.exterior_door
    } else {
        &d.interior_door
    };
    let mut t = Opening::default_door(0, 0, 0.0);
    t.width = o.width;
    t.height = o.height;
    t.sill_height = o.sill_height;
    t
}

/// The opening new windows are placed from.
pub fn window_template(d: &PlanDefaults) -> Opening {
    let w = &d.window;
    let mut t = Opening::default_window(0, 0, 0.0);
    t.width = w.width;
    t.height = w.height;
    t.sill_height = w.sill_height;
    t
}

/// Picks the wall type name for a wall whose thickness was edited: the
/// `picked` type if it still has that thickness, else the first type of that
/// kind with it, else a new single-purpose `Custom-<thickness>` type (a
/// drywall / framing / drywall stack) added to `d`.
pub fn resolve_wall_type(
    d: &mut PlanDefaults,
    kind: WallKind,
    picked: Option<&str>,
    thickness: f64,
) -> String {
    let fits = |t: &WallTypeDef| (t.thickness() - thickness).abs() < 1e-6;
    if let Some(t) = picked.and_then(|n| d.wall_type(n)).filter(|t| fits(t)) {
        return t.name.clone();
    }
    if let Some(t) = d.wall_types.iter().find(|t| t.kind == kind && fits(t)) {
        return t.name.clone();
    }
    let name = format!("Custom-{thickness}");
    if d.wall_type(&name).is_none() {
        let layer = |n: &str, t: f64, main: bool| WallLayer {
            name: n.into(),
            thickness: t,
            is_main: main,
            material: if main { "Fir Framing" } else { "Drywall" }.into(),
        };
        let layers = if thickness >= 1.5 {
            vec![
                layer("Drywall", 0.5, false),
                layer("Framing", thickness - 1.0, true),
                layer("Drywall", 0.5, false),
            ]
        } else {
            vec![layer("Drywall", thickness, true)]
        };
        d.wall_types.push(WallTypeDef {
            name: name.clone(),
            layers,
            kind,
        });
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::OpeningKind;

    #[test]
    fn embedded_template_matches_chief_x18_daniel() {
        let loaded = PlanDefaults::from_json(EMBEDDED_TEMPLATE).expect("template parses");
        assert_eq!(loaded, PlanDefaults::chief_x18_daniel());
        assert_eq!(embedded(), loaded);
    }

    #[test]
    fn templates_come_from_defaults() {
        let d = PlanDefaults::chief_x18_daniel();
        let door = door_template(&d, false);
        assert_eq!(
            (door.width, door.height, door.sill_height),
            (30.0, 96.0, 0.0)
        );
        let ext = door_template(&d, true);
        assert_eq!(ext.width, 36.0);
        let win = window_template(&d);
        assert_eq!(
            (win.width, win.height, win.sill_height, win.kind),
            (32.0, 72.0, 24.0, OpeningKind::Window)
        );
    }

    #[test]
    fn resolve_wall_type_reuses_or_adds() {
        let mut d = PlanDefaults::chief_x18_daniel();
        let n = d.wall_types.len();
        assert_eq!(
            resolve_wall_type(&mut d, WallKind::Exterior, Some("Siding-6"), 7.0),
            "Siding-6"
        );
        // The picked type no longer fits; the first exterior type with 7 5/8" wins.
        assert_eq!(
            resolve_wall_type(&mut d, WallKind::Exterior, Some("Siding-6"), 7.625),
            "Stucco-6"
        );
        assert_eq!(d.wall_types.len(), n);
        let name = resolve_wall_type(&mut d, WallKind::Interior, None, 5.0);
        assert_eq!(name, "Custom-5");
        assert_eq!(d.wall_type(&name).unwrap().thickness(), 5.0);
        assert_eq!(
            resolve_wall_type(&mut d, WallKind::Interior, None, 5.0),
            name
        );
        assert_eq!(d.wall_types.len(), n + 1);
    }
}
