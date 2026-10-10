//! Cabinet Shelf Specification (reference manual p. 678; tutorial pp. 250,
//! 273-274): the shelves of a Door, Double Door, Opening or Rollout face
//! item. Automatic keeps the number Chief picks from the opening height;
//! Manual lists every shelf with its thickness, spacing, depth and roll-out.
//!
//! The page is a plain function on `plan_cabinets::ShelfSpec` so the Face
//! Item Specification can embed it and tests can drive the edits without a
//! GUI.

use super::{row, section, Fields};
use eframe::egui::{self, Ui};
use plan_cabinets::{Shelf, ShelfDepth, ShelfSpec};

/// What the page needs to know about the opening the shelves sit in.
#[derive(Clone, Copy)]
pub struct ShelfContext {
    /// Clear height of the opening, inches.
    pub opening_height: f64,
    /// Rollout only makes sense for standard cabinets (the manual's Rollout
    /// check is off for wall cabinets).
    pub allow_rollout: bool,
}

/// Switches to Automatic shelving (the stored manual shelves are dropped).
pub fn set_automatic(spec: &mut ShelfSpec) {
    *spec = ShelfSpec::default();
}

/// Switches to Manual shelving, starting from the automatic arrangement so
/// the shelves do not jump (`h` is the opening height).
pub fn set_manual(spec: &mut ShelfSpec, h: f64) {
    if spec.manual {
        return;
    }
    let n = spec.count(h);
    *spec = ShelfSpec::manual_of(n);
    spec.equalize(h);
}

/// Adds a shelf after `after` (at the end when `None`) and re-spaces the
/// shelves evenly. Returns the new shelf's index.
pub fn add_shelf(spec: &mut ShelfSpec, after: Option<usize>, h: f64) -> usize {
    spec.manual = true;
    let at = after.map_or(spec.shelves.len(), |i| (i + 1).min(spec.shelves.len()));
    spec.shelves.insert(at, Shelf::default());
    if spec.equal_spacing {
        spec.equalize(h);
    }
    at
}

/// Removes shelf `i`; returns the index to select next (`None` when none are
/// left).
pub fn delete_shelf(spec: &mut ShelfSpec, i: usize, h: f64) -> Option<usize> {
    if i >= spec.shelves.len() {
        return None;
    }
    spec.shelves.remove(i);
    if spec.equal_spacing {
        spec.equalize(h);
    }
    if spec.shelves.is_empty() {
        None
    } else {
        Some(i.min(spec.shelves.len() - 1))
    }
}

/// Draws the page. `sel` is the selected manual shelf (kept by the caller so
/// it survives frames). Returns true when `spec` changed.
pub fn shelf_spec_ui(
    ui: &mut Ui,
    fields: &mut Fields,
    spec: &mut ShelfSpec,
    ctx: ShelfContext,
    sel: &mut usize,
) -> bool {
    let before = spec.clone();
    section(ui, "Cabinet Shelf Specification");
    let h = ctx.opening_height;
    ui.horizontal(|ui| {
        if ui.radio(!spec.manual, "Automatic").clicked() && spec.manual {
            set_automatic(spec);
        }
        if ui.radio(spec.manual, "Manual").clicked() && !spec.manual {
            set_manual(spec, h);
            *sel = 0;
        }
    });
    if !spec.manual {
        row(ui, "Number of Shelves", |ui| {
            ui.label(format!("{} (from the opening height)", spec.count(h)));
        });
        return *spec != before;
    }
    if spec.shelves.is_empty() {
        ui.weak("(no shelves)");
    }
    *sel = (*sel).min(spec.shelves.len().saturating_sub(1));
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        for i in 0..spec.shelves.len() {
            let s = &spec.shelves[i];
            let text = format!(
                "{}  Shelf  {}{}",
                i + 1,
                super::fmt_short(s.thickness),
                if s.rollout { "  (rollout)" } else { "" }
            );
            if ui.selectable_label(*sel == i, text).clicked() {
                *sel = i;
            }
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Add").clicked() {
            *sel = add_shelf(spec, (!spec.shelves.is_empty()).then_some(*sel), h);
        }
        let has = !spec.shelves.is_empty();
        if ui.add_enabled(has, egui::Button::new("Delete")).clicked() {
            *sel = delete_shelf(spec, *sel, h).unwrap_or(0);
        }
        if ui
            .add_enabled(spec.shelves.len() > 1, egui::Button::new("Equalize"))
            .clicked()
        {
            spec.equalize(h);
        }
    });
    ui.checkbox(&mut spec.equal_spacing, "Equal Spacing");
    if let Some(s) = spec.shelves.get_mut(*sel) {
        section(ui, "Selected Shelf");
        fields.length_row(ui, "Thickness", "shelf_thick", &mut s.thickness);
        if !spec.equal_spacing {
            fields.length_row(ui, "Spacing", "shelf_space", &mut s.spacing);
        }
        row(ui, "Depth", |ui| {
            let mut kind = s.depth.name();
            egui::ComboBox::from_id_salt("shelf_depth")
                .selected_text(kind)
                .show_ui(ui, |ui| {
                    for (name, value) in [
                        ("Full", ShelfDepth::Full),
                        ("Half", ShelfDepth::Half),
                        ("Specify", ShelfDepth::Specify(12.0)),
                    ] {
                        let here = kind == name;
                        if ui.selectable_label(here, name).clicked() && !here {
                            s.depth = value;
                            kind = name;
                        }
                    }
                });
        });
        if let ShelfDepth::Specify(d) = &mut s.depth {
            fields.length_row(ui, "Specified Depth", "shelf_depth_in", d);
        }
        if ctx.allow_rollout {
            ui.checkbox(&mut s.rollout, "Rollout");
            if s.rollout {
                fields.length_row(ui, "Rollout Amount", "shelf_roll", &mut s.rollout_amount);
            }
        }
        row(ui, "Library Object", |ui| {
            ui.text_edit_singleline(&mut s.library);
        });
    }
    *spec != before
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn automatic_to_manual_keeps_the_count_and_spreads_the_shelves() {
        let mut spec = ShelfSpec::default();
        let auto_count = spec.count(30.0);
        assert!(auto_count > 0);
        set_manual(&mut spec, 30.0);
        assert!(spec.manual);
        assert_eq!(spec.shelves.len(), auto_count);
        let placed = spec.place(30.0);
        assert_eq!(placed.len(), auto_count);
        set_automatic(&mut spec);
        assert!(!spec.manual && spec.shelves.is_empty());
    }

    #[test]
    fn add_and_delete_respace_when_spacing_is_equal() {
        let mut spec = ShelfSpec::manual_of(1);
        let i = add_shelf(&mut spec, Some(0), 30.0);
        assert_eq!((i, spec.shelves.len()), (1, 2));
        let gap = spec.shelves[0].spacing;
        assert!((gap - spec.shelves[1].spacing).abs() < 1e-9);
        assert_eq!(delete_shelf(&mut spec, 1, 30.0), Some(0));
        assert_eq!(delete_shelf(&mut spec, 0, 30.0), None);
        assert!(delete_shelf(&mut spec, 5, 30.0).is_none());
    }

    #[test]
    fn the_page_draws_in_both_modes() {
        let ctx = egui::Context::default();
        let mut fields = Fields::default();
        let mut sel = 0;
        let mut spec = ShelfSpec::default();
        for manual in [false, true] {
            if manual {
                set_manual(&mut spec, 30.0);
            }
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    shelf_spec_ui(
                        ui,
                        &mut fields,
                        &mut spec,
                        ShelfContext {
                            opening_height: 30.0,
                            allow_rollout: true,
                        },
                        &mut sel,
                    );
                });
            });
        }
    }
}
