//! Where Plan Studio's defaults come from: `~/.plan-studio/defaults.json` when
//! the user saved one ("Save Current Defaults as My Template"), else the
//! Chief X18 template embedded in the binary with the decoded values of the
//! user's own Chief plan template laid over it (`crate::templates`, when the
//! template exists and seeding is on). Plus the small helpers that
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

/// What a new plan starts from when the user has not saved their own
/// defaults: the embedded template, seeded from the Chief plan template named
/// in `~/.plan-studio/settings.json` when there is one. Without a template
/// (or with seeding off) this is exactly [`embedded`]. The second value is a
/// note for the status bar.
pub fn template_base() -> (PlanDefaults, Option<String>) {
    crate::templates::seeded_defaults(&crate::templates::load_settings(), embedded())
}

/// The user's saved defaults if present, else [`template_base`]. The second
/// value is a note to show (a fresh template decode, or a file that could not
/// be read).
pub fn load() -> (PlanDefaults, Option<String>) {
    let Some(path) = user_path().filter(|p| p.exists()) else {
        return template_base();
    };
    match PlanDefaults::load(&path) {
        Ok(d) => (d, None),
        Err(e) => (
            template_base().0,
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
            spec: Default::default(),
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
            props: Default::default(),
            name: name.clone(),
            layers,
            kind,
        });
    }
    name
}

// ===================================================================
// Dynamic defaults and Set as Default (manual pp. 103 to 104)
// ===================================================================

/// Edit-toolbar command id: Set as Default.
pub const SET_AS_DEFAULT: &str = "defaults.set_as_default";

thread_local! {
    /// The defaults as the last frame saw them (see [`track`]).
    static LAST: std::cell::RefCell<Option<(u64, PlanDefaults)>> = const { std::cell::RefCell::new(None) };
}

/// Objects that use the default follow a changed default: after the plan
/// defaults changed from `old` to the ones in `cx`, every wall, door, window
/// and cabinet that is still on the default takes the new value (walls:
/// `Project::follow_wall_defaults`; openings: their Use Default flags,
/// `Project::follow_type_defaults`; cabinets: `apply_dynamic_defaults`). One
/// undo step. Returns how many objects changed.
pub fn follow_changed_defaults(cx: &mut crate::editor::EditorContext, old: &PlanDefaults) -> usize {
    let mut total = 0;
    cx.as_one_step("Default Settings", |cx| {
        cx.begin_change("Default Settings");
        let mut n = cx.project.follow_wall_defaults(old, &cx.defaults);
        n += cx
            .project
            .follow_type_defaults(&cx.defaults.opening_variants);
        if n == 0 {
            cx.cancel_change();
        } else {
            cx.mark_dirty();
        }
        n += crate::tools::cabinet::apply_dynamic_defaults(cx, &old.cabinets);
        total = n;
    });
    total
}

/// Watches the plan defaults once a frame: when they changed since the last
/// frame (a defaults dialog's OK, Set as Default, a template import) the
/// objects that use the default follow ([`follow_changed_defaults`]). The
/// first call only remembers the defaults. Returns how many objects changed.
pub fn track(cx: &mut crate::editor::EditorContext) -> usize {
    let uid = cx.cache_key().0;
    let old = LAST.with(|l| {
        let mut slot = l.borrow_mut();
        match slot.as_ref() {
            Some((u, prev)) if *u == uid && *prev == cx.defaults => None,
            // Another context (a new plan window, a test): start over.
            Some((u, _)) if *u != uid => {
                *slot = Some((uid, cx.defaults.clone()));
                None
            }
            Some(_) => slot.replace((uid, cx.defaults.clone())).map(|(_, d)| d),
            None => {
                *slot = Some((uid, cx.defaults.clone()));
                None
            }
        }
    });
    match old {
        Some(old) => follow_changed_defaults(cx, &old),
        None => 0,
    }
}

/// Is there one object selected that the Edit toolbar's Set as Default reads?
/// (Cabinets, construction lines and electrical devices have their own Set
/// as Default buttons.)
pub fn can_set_as_default(cx: &crate::editor::EditorContext) -> bool {
    use crate::editor::ObjectRef;
    match cx.selection.single() {
        Some(ObjectRef::Wall(_) | ObjectRef::Opening(_) | ObjectRef::Dimension(_)) => true,
        Some(ObjectRef::Cad(id) | ObjectRef::Text(id)) => cx.floor().annot_of(id).is_some(),
        _ => false,
    }
}

/// Set as Default (Edit toolbar): the settings of the selected object become
/// the defaults for its kind, and the objects that use the default follow.
/// Not available for the terrain's paths (sidewalks, streams, terrain walls,
/// curbs) or for kinds without a defaults dialog. Returns whether a default
/// changed; the status line says what happened.
pub fn set_as_default(cx: &mut crate::editor::EditorContext) -> bool {
    use crate::editor::ObjectRef;
    let Some(sel) = cx.selection.single() else {
        cx.status = "Select one object to set the defaults from".into();
        return false;
    };
    let old = cx.defaults.clone();
    let changed = match sel {
        ObjectRef::Terrain | ObjectRef::TerrainObject(_) => {
            cx.status = "Set as Default is not available for terrain paths".into();
            return false;
        }
        ObjectRef::Wall(id) => {
            let mut done = false;
            cx.as_one_step("Set as Default", |cx| {
                cx.begin_change("Set as Default");
                let mut d = cx.defaults.clone();
                let kind = cx.project.set_wall_as_default(&mut d, id);
                cx.defaults = d;
                if let Some(k) = kind {
                    cx.status = format!("{} Defaults have been updated", k.label());
                    cx.mark_dirty();
                    done = true;
                } else {
                    cx.cancel_change();
                    cx.status = "Set as Default is only available for standard walls".into();
                }
            });
            done
        }
        ObjectRef::Opening(id) => {
            let found = cx
                .floor()
                .openings
                .iter()
                .find(|o| o.id == id)
                .cloned()
                .map(|o| {
                    let kind = cx
                        .floor()
                        .wall(o.wall_id)
                        .map_or(WallKind::Interior, |w| w.kind);
                    (o, kind)
                });
            match found {
                Some((o, wk)) => {
                    let key = cx.defaults.opening_variants.set_as_default(&o, wk);
                    cx.status = format!("Defaults for {key:?} have been updated");
                    true
                }
                None => false,
            }
        }
        ObjectRef::Cabinet(_) => crate::tools::cabinet::set_as_default(cx),
        ObjectRef::Device(id) => crate::tools::electrical::set_device_as_default(cx, id),
        ObjectRef::Cad(id) | ObjectRef::Text(id) => set_annotation_as_default(cx, id),
        ObjectRef::Dimension(id) => set_dimension_as_default(cx, id),
        _ => {
            cx.status = "Set as Default is not available for this kind of object".into();
            false
        }
    };
    if changed && cx.defaults != old {
        follow_changed_defaults(cx, &old);
        // The watcher would see the same change next frame; it has been
        // handled now.
        LAST.with(|l| *l.borrow_mut() = Some((cx.cache_key().0, cx.defaults.clone())));
    }
    changed
}

/// Set as Default for a construction line or a callout, marker or note: the
/// annotation's settings become the Saved Default in use.
fn set_annotation_as_default(cx: &mut crate::editor::EditorContext, id: plan_core::Id) -> bool {
    use plan_core::callout::AnnotRef;
    let Some(r) = cx.floor().annot_of(id) else {
        // A construction line has its own command.
        return crate::dialogs::construction_line::run_command(
            cx,
            crate::dialogs::construction_line::SET_AS_DEFAULT,
        );
    };
    let (label, name) = match r {
        AnnotRef::Callout(i) => {
            let mut c = cx.floor().annots.callouts[i].clone();
            c.items.clear();
            c.pose = None;
            c.pose_idx = 0;
            cx.project.annot_defaults.callout = c;
            ("Callout", cx.project.annot_defaults.saved_name.clone())
        }
        AnnotRef::Marker(i) => {
            let mut m = cx.floor().annots.markers[i].clone();
            m.items.clear();
            cx.project.annot_defaults.marker = m;
            ("Marker", cx.project.annot_defaults.saved_name.clone())
        }
        AnnotRef::Note(i) => {
            let mut n = cx.floor().annots.notes[i].clone();
            n.items.clear();
            cx.project.annot_defaults.note = n;
            ("Note", cx.project.annot_defaults.saved_name.clone())
        }
    };
    cx.mark_dirty();
    cx.status = format!("{label} Defaults - {name} have been updated");
    true
}

/// Set as Default for a dimension line: the settings it sets itself (number
/// format, arrow, extension lines, text style) become the active Dimension
/// Defaults.
fn set_dimension_as_default(cx: &mut crate::editor::EditorContext, id: plan_core::Id) -> bool {
    let Some(dim) = cx.floor().dimensions.iter().find(|d| d.id == id).cloned() else {
        return false;
    };
    let name = cx.defaults.active_dimension_set.clone();
    let mut auto = cx.defaults.dimensions.clone();
    let look = &dim.look;
    if let Some(f) = look.fraction {
        auto.smallest_fraction = f;
    }
    if let Some(v) = look.decimals {
        auto.decimals = v;
    }
    if let Some(v) = look.unit_indicators {
        auto.unit_indicators = v;
    }
    if let Some(v) = look.trailing_zeroes {
        auto.trailing_zeroes = v;
    }
    if let Some(v) = look.arrow_size {
        auto.arrow_size = v;
    }
    if let Some(v) = look.ext_gap {
        auto.extension_gap = v;
    }
    if let Some(v) = look.ext_past {
        auto.extension_past = v;
    }
    if let Some(v) = &dim.text_style {
        auto.text_style = v.clone();
    }
    if auto == cx.defaults.dimensions {
        cx.status = "The Dimension Defaults already match this dimension".into();
        return false;
    }
    let set = plan_core::defaults::DimensionDefaultSet::new(name.clone(), auto);
    match cx
        .defaults
        .dimension_sets
        .iter_mut()
        .find(|s| s.name == name)
    {
        Some(slot) => *slot = set,
        None => cx.defaults.dimension_sets.push(set),
    }
    cx.defaults.set_active_dimension_set(&name);
    cx.status = format!("The Dimension Defaults \"{name}\" have been updated");
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::OpeningKind;

    /// Maintenance: `cargo test -p plan-app regenerate_embedded_template -- --ignored`
    /// rewrites the embedded template from the code defaults.
    #[test]
    #[ignore = "maintenance: regenerates assets/templates/chief-x18-daniel.json"]
    fn regenerate_embedded_template() {
        let text = PlanDefaults::chief_x18_daniel().to_json().expect("to_json");
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/templates/chief-x18-daniel.json"
        );
        std::fs::write(path, text).expect("write template");
    }

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
