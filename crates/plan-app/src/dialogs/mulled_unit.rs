//! The Mulled Unit Specification and Defaults panels (DW-160; manual pp. 609
//! to 611): Treat as Door, Single Wall Hole, Mullion Depth inside and outside,
//! and the label mode.
//!
//! A mulled unit's components are separate openings that share a group, so
//! the unit's panel is shown on the Options panel of whichever component is
//! opened (Select Next Object picks the component, Open Object edits it), and
//! it edits the settings of the whole unit: every component carries the same
//! [`MulledSpec`], and the one edited last (its `revision`) is the unit's.
//! The Window Defaults hold the Mulled Unit Defaults a new unit starts with.
//! The module is a child of `dialogs/opening.rs` (`#[path]`).

use super::{row, section, OpeningForm, Ui};
use eframe::egui;
use plan_core::openings::mull::{MulledLabel, MulledSpec};

impl OpeningForm {
    /// The Mulled Unit panel of an opening that is a component of a unit, or
    /// of the Window Defaults (the Mulled Unit Defaults).
    pub(super) fn mulled_unit_section(&mut self, ui: &mut Ui) {
        let defaults = matches!(self.target, super::OpeningTarget::DefaultWindow);
        let in_unit = self.draft.mull_group.is_some();
        if !in_unit && !defaults {
            return;
        }
        section(
            ui,
            if defaults {
                "Mulled Unit Defaults"
            } else {
                "Mulled Unit"
            },
        );
        if in_unit {
            ui.weak("This opening is a component of a mulled unit; these settings are the unit's.");
        }
        let mut spec: MulledSpec = if defaults {
            self.extras.mulled.clone()
        } else {
            self.draft.extras.spec.mulled.clone().unwrap_or_default()
        };
        let before = spec.clone();
        ui.checkbox(&mut spec.treat_as_door, "Treat as Door")
            .on_hover_text(
                "Drawn on the Doors layer, counted with the doors and listed as a Mulled Door Unit",
            );
        ui.checkbox(&mut spec.single_hole, "Single Wall Hole")
            .on_hover_text(
                "One hole in the wall around the whole unit instead of one per component",
            );
        let mut inside = spec.mullion_inside;
        if self
            .fields
            .length_row(ui, "Mullion Depth, Inside", "mu_inside", &mut inside)
        {
            spec.mullion_inside = inside.max(0.0);
        }
        let mut outside = spec.mullion_outside;
        if self
            .fields
            .length_row(ui, "Mullion Depth, Outside", "mu_outside", &mut outside)
        {
            spec.mullion_outside = outside.max(0.0);
        }
        row(ui, "Labels", |ui| {
            egui::ComboBox::from_id_salt("mulled_label")
                .selected_text(spec.label.name())
                .show_ui(ui, |ui| {
                    for m in MulledLabel::ALL {
                        ui.selectable_value(&mut spec.label, m, m.name());
                    }
                });
        });
        if spec != before {
            if defaults {
                self.extras.mulled = spec;
            } else {
                // The newest edit wins among the components.
                spec.revision = spec.revision.wrapping_add(1).max(self.unit_revision + 1);
                self.unit_revision = spec.revision;
                self.draft.extras.spec.mulled = Some(spec);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use plan_core::geometry::Point;

    #[test]
    fn a_component_edits_the_unit_and_the_newest_edit_has_the_higher_revision() {
        let wall = Wall::new(
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        let mut o = Opening::default_window(9, 1, 100.0);
        o.mull_group = Some(9);
        o.extras.spec.mulled = Some(MulledSpec::default());
        let mut d = OpeningDialog::for_opening(o, &wall, Vec::new(), OpeningExtras::default());
        let ctx = egui::Context::default();
        assert!(d.draw_tab_for_test(&ctx, "Options"));
        // Edit as the checkboxes would.
        {
            let m = d.draft_mut().extras.spec.mulled.as_mut().unwrap();
            m.single_hole = true;
            m.revision = 3;
        }
        d.sync_stored_for_test();
        let m = d.draft().extras.spec.mulled.as_ref().unwrap();
        assert!(m.single_hole && m.revision == 3);
    }

    use plan_core::openings::mull::MulledSpec;
}
