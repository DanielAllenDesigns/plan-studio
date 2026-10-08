//! Materials Defaults (Default Settings > Materials): the material each
//! object class gives its parts unless the object was painted.
//!
//! The defaults live in the plan (`Project::material_defaults`), one entry
//! per class and part, so they travel with the file and undo with the plan.
//! A class with no entry for a part uses the built-in material of
//! `plan_materials::default_assignments_for`. The 3D view resolves a mesh by
//! its own paint first, then these defaults ([`super::apply_overrides`]).
//! The Room class's floor and ceiling entries are also the floor's default
//! surface materials (`FloorSettings`), which new rooms start with.

use super::{library, swatch};
use crate::editor::EditorContext;
use eframe::egui::{self, Align2};
use plan_core::Project;
use plan_materials::default_assignments_for;

/// The object classes that have default materials.
pub const CLASSES: [&str; 6] = ["Wall", "Door", "Window", "Cabinet", "Roof", "Room"];

/// `(part, built-in material)` of a class, in the order of the dialogs'
/// Components tabs.
pub fn parts(class: &str) -> Vec<(String, String)> {
    default_assignments_for(class)
        .into_iter()
        .map(|a| (a.component, a.material))
        .collect()
}

/// The material `part` of `class` has now: the plan's default, else the
/// built-in one.
pub fn effective(project: &Project, class: &str, part: &str) -> Option<String> {
    project
        .class_material(class, part)
        .map(str::to_string)
        .or_else(|| {
            parts(class)
                .into_iter()
                .find(|(p, _)| p == part)
                .map(|(_, m)| m)
        })
}

/// Does `part` of `class` have a default of the plan's own?
pub fn is_customized(project: &Project, class: &str, part: &str) -> bool {
    project
        .material_defaults
        .iter()
        .any(|d| d.class.eq_ignore_ascii_case(class) && d.part == part)
}

/// Sets (`Some`) or clears (`None`) the default of `part` of `class` as one
/// undo step; true when the plan changed.
pub fn set(cx: &mut EditorContext, class: &str, part: &str, material: Option<&str>) -> bool {
    let changes = match material {
        Some(m) => {
            cx.project.class_material(class, part) != Some(m)
                || !is_customized(&cx.project, class, part)
        }
        None => is_customized(&cx.project, class, part),
    };
    if !changes {
        return false;
    }
    cx.begin_change("Materials Defaults");
    match material {
        Some(m) => {
            cx.project.set_class_material(class, part, m);
        }
        None => {
            cx.project.clear_class_material(class, Some(part));
        }
    }
    // A new room starts with the floor's default surfaces.
    if class.eq_ignore_ascii_case("Room") {
        let value = material.unwrap_or("").to_string();
        for f in &mut cx.project.floors {
            match part {
                "Floor Finish" => f.settings.floor_material = value.clone(),
                "Ceiling Finish" => f.settings.ceiling_material = value.clone(),
                _ => {}
            }
        }
    }
    cx.mark_dirty();
    cx.status = match material {
        Some(m) => format!("{class} {part}: {m}"),
        None => format!("{class} {part}: built-in material"),
    };
    true
}

/// Removes every default of the plan; true when there were any.
pub fn reset_all(cx: &mut EditorContext) -> bool {
    if cx.project.material_defaults.is_empty() {
        return false;
    }
    cx.begin_change("Materials Defaults");
    cx.project.material_defaults.clear();
    cx.mark_dirty();
    cx.status = "Materials Defaults reset to the built-in materials".into();
    true
}

/// The body of the Materials page (also the body of the window).
pub fn page(ui: &mut egui::Ui, cx: &mut EditorContext) {
    let lib = library();
    ui.weak("The material each kind of object gives its parts unless it was painted.");
    let mut pick: Option<(&'static str, String, Option<String>)> = None;
    egui::ScrollArea::vertical()
        .id_salt("materials_defaults_scroll")
        .max_height(380.0)
        .show(ui, |ui| {
            for class in CLASSES {
                ui.strong(class);
                egui::Grid::new(("materials_defaults_grid", class))
                    .num_columns(2)
                    .spacing([12.0, 4.0])
                    .show(ui, |ui| {
                        for (part, built_in) in parts(class) {
                            ui.label(&part);
                            let now = effective(&cx.project, class, &part);
                            let custom = is_customized(&cx.project, class, &part);
                            ui.horizontal(|ui| {
                                if let Some(d) = now.as_deref().and_then(|n| lib.find(n)) {
                                    swatch(ui, d.color);
                                }
                                egui::ComboBox::from_id_salt(("md_combo", class, &part))
                                    .selected_text(match (&now, custom) {
                                        (Some(n), true) => n.clone(),
                                        (Some(n), false) => format!("{n} (built-in)"),
                                        (None, _) => "none".to_string(),
                                    })
                                    .width(230.0)
                                    .show_ui(ui, |ui| {
                                        if ui
                                            .selectable_label(
                                                !custom,
                                                format!("Built-in ({built_in})"),
                                            )
                                            .clicked()
                                        {
                                            pick = Some((class, part.clone(), None));
                                        }
                                        for m in &lib.materials {
                                            if ui
                                                .selectable_label(
                                                    custom && now.as_deref() == Some(&m.name),
                                                    &m.name,
                                                )
                                                .clicked()
                                            {
                                                pick = Some((
                                                    class,
                                                    part.clone(),
                                                    Some(m.name.clone()),
                                                ));
                                            }
                                        }
                                    });
                            });
                            ui.end_row();
                        }
                    });
                ui.add_space(4.0);
            }
        });
    if let Some((class, part, material)) = pick {
        set(cx, class, &part, material.as_deref());
    }
    ui.separator();
    if ui
        .add_enabled(
            !cx.project.material_defaults.is_empty(),
            egui::Button::new("Reset All to Built-in"),
        )
        .clicked()
    {
        reset_all(cx);
    }
}

/// The Materials Defaults window.
pub fn window(ctx: &egui::Context, cx: &mut EditorContext) {
    let mut open = true;
    egui::Window::new("Default Settings: Materials")
        .id(egui::Id::new("materials_defaults"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| page(ui, cx));
    if !open {
        super::state(|s| s.defaults_open = false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    #[test]
    fn every_class_lists_its_parts_and_the_builtin_materials_exist() {
        let lib = library();
        for class in CLASSES {
            let p = parts(class);
            assert!(!p.is_empty(), "{class}");
            for (part, m) in p {
                assert!(lib.find(&m).is_some(), "{class} {part}: {m}");
            }
        }
    }

    #[test]
    fn a_default_is_one_undo_step_and_falls_back_to_the_builtin() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        assert_eq!(
            effective(&cx.project, "Wall", "Interior Wall Surface").as_deref(),
            Some("Drywall")
        );
        assert!(set(
            &mut cx,
            "Wall",
            "Interior Wall Surface",
            Some("Color – Bone")
        ));
        assert_eq!(cx.undo_label(), Some("Materials Defaults"));
        assert_eq!(
            effective(&cx.project, "Wall", "Interior Wall Surface").as_deref(),
            Some("Color – Bone")
        );
        assert!(is_customized(&cx.project, "Wall", "Interior Wall Surface"));
        assert!(!set(
            &mut cx,
            "Wall",
            "Interior Wall Surface",
            Some("Color – Bone")
        ));
        // Choosing the built-in again removes the entry.
        assert!(set(&mut cx, "Wall", "Interior Wall Surface", None));
        assert!(cx.project.material_defaults.is_empty());
        assert!(!set(&mut cx, "Wall", "Interior Wall Surface", None));
        cx.undo();
        assert_eq!(
            effective(&cx.project, "Wall", "Interior Wall Surface").as_deref(),
            Some("Color – Bone")
        );
        assert!(reset_all(&mut cx));
        assert!(!reset_all(&mut cx));
    }

    #[test]
    fn the_room_class_feeds_the_floors_default_surfaces() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        assert!(set(&mut cx, "Room", "Floor Finish", Some("Oak Flooring")));
        assert_eq!(cx.project.floors[0].settings.floor_material, "Oak Flooring");
        assert!(set(
            &mut cx,
            "Room",
            "Ceiling Finish",
            Some("Color – White")
        ));
        assert_eq!(
            cx.project.floors[0].settings.ceiling_material,
            "Color – White"
        );
        assert!(set(&mut cx, "Room", "Floor Finish", None));
        assert_eq!(cx.project.floors[0].settings.floor_material, "");
    }

    #[test]
    fn the_page_draws_headlessly() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| window(ctx, &mut cx));
    }
}
