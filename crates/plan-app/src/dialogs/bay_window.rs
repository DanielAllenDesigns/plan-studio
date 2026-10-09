//! The Bay/Box and Bow Window Specification panels (DW-48, DW-155, DW-167;
//! manual pp. 639 to 642): Sections, Size, Wall Type, Ceiling and Floor on the
//! General panel; Roof, Dimensions and Components on the Options panel.
//!
//! These are panels of the [`OpeningDialog`](super::OpeningDialog): a bay, box
//! or bow window is an opening whose style draws a wall-section unit, so its
//! Specification is the Window Specification with these in place of the
//! window-type rows. The module is a child of `dialogs/opening.rs`
//! (`#[path]`), so it reaches the form's fields.

use super::{row, section, OpeningForm, OpeningStyle, Ui};
use eframe::egui;
use plan_core::openings::bay::{
    bay_shape, BayRecess, LoweredCeiling, RaisedFloor, BOW_MAX, BOW_MIN,
};
use plan_core::openings::mull::MulledLabel;

impl OpeningForm {
    /// General panel of a bay, box or bow window: Sections, Size, Wall Type,
    /// Ceiling and Floor.
    pub(super) fn bay_general(&mut self, ui: &mut Ui) {
        let style = self.draft.style;
        section(ui, "Sections");
        match style {
            OpeningStyle::BowWindow => {
                row(ui, "Quantity", |ui| {
                    let mut n = self.draft.extras.spec.bay.segments as i32;
                    if ui
                        .add(egui::DragValue::new(&mut n).range(BOW_MIN as i32..=BOW_MAX as i32))
                        .on_hover_text("The number of component windows of the bow: two to twenty")
                        .changed()
                    {
                        self.draft.extras.spec.bay.segments = n.clamp(2, 20) as u32;
                    }
                });
            }
            OpeningStyle::BoxWindow => {
                row(ui, "Bay Angle", |ui| {
                    ui.label("90\u{B0} (a box window)");
                });
            }
            _ => {
                let mut angle = self.draft.extras.spec.bay.angle_deg;
                if self
                    .fields
                    .degrees_row(ui, "Bay Angle", "deg_bay_angle", &mut angle)
                {
                    self.draft.extras.spec.bay.angle_deg = angle.clamp(10.0, 90.0);
                }
            }
        }
        section(ui, "Size");
        let default_depth = self.draft.extras.spec.bay.depth_for(style);
        let mut depth = default_depth;
        if self.fields.length_row(ui, "Depth", "bay_depth", &mut depth) {
            self.draft.extras.spec.bay.depth = depth.clamp(6.0, 96.0);
        }
        let shape = bay_shape(style, self.draft.width, &self.draft.extras.spec.bay);
        ui.weak(format!(
            "{} sections; the front is {} across",
            shape.sections.len(),
            super::fmt_short(shape.front_width)
        ));
        section(ui, "Wall Type");
        row(ui, "Wall Type", |ui| {
            let mut text = self
                .draft
                .extras
                .spec
                .bay
                .wall_type
                .clone()
                .unwrap_or_default();
            let hint = "Use Main Wall Type";
            if ui
                .add(egui::TextEdit::singleline(&mut text).hint_text(hint))
                .on_hover_text("Leave empty to use the wall type of the wall the unit is placed in")
                .changed()
            {
                self.draft.extras.spec.bay.wall_type = (!text.trim().is_empty()).then_some(text);
            }
        });
        section(ui, "Ceiling");
        let mut lowered = self.draft.extras.spec.bay.lowered_ceiling.is_some();
        if ui
            .checkbox(&mut lowered, "Has Lowered Ceiling")
            .on_hover_text("Lowers the ceiling inside the unit, and the tops of its walls")
            .changed()
        {
            self.draft.extras.spec.bay.lowered_ceiling = lowered.then(LoweredCeiling::default);
        }
        if let Some(mut c) = self.draft.extras.spec.bay.lowered_ceiling {
            let mut changed = false;
            changed |= self
                .fields
                .length_row(ui, "Height Lowered", "bay_c_height", &mut c.height);
            changed |=
                self.fields
                    .length_row(ui, "Finish Thickness", "bay_c_finish", &mut c.finish);
            changed |=
                self.fields
                    .length_row(ui, "Structure Thickness", "bay_c_struct", &mut c.structure);
            if changed {
                self.draft.extras.spec.bay.lowered_ceiling = Some(c);
            }
        }
        section(ui, "Floor");
        let mut raised = self.draft.extras.spec.bay.raised_floor.is_some();
        if ui
            .checkbox(&mut raised, "Has Raised Floor")
            .on_hover_text("A bench seat or a garden window; no foundation is built under it")
            .changed()
        {
            self.draft.extras.spec.bay.raised_floor = raised.then(RaisedFloor::default);
        }
        if let Some(mut f) = self.draft.extras.spec.bay.raised_floor {
            let mut changed = false;
            changed |= self
                .fields
                .length_row(ui, "Height Raised", "bay_f_height", &mut f.height);
            let before = f.use_floor_finish;
            ui.checkbox(&mut f.use_floor_finish, "Use Floor Finish")
                .on_hover_text("Measure the Height Raised from the floor finish, not the subfloor");
            changed |= before != f.use_floor_finish;
            changed |=
                self.fields
                    .length_row(ui, "Finish Thickness", "bay_f_finish", &mut f.finish);
            changed |=
                self.fields
                    .length_row(ui, "Structure Thickness", "bay_f_struct", &mut f.structure);
            if changed {
                self.draft.extras.spec.bay.raised_floor = Some(f);
            }
        }
    }

    /// Options panel of a bay, box or bow window: Roof, Dimensions,
    /// Components and the label.
    pub(super) fn bay_options(&mut self, ui: &mut Ui) {
        section(ui, "Roof");
        {
            let r = &mut self.draft.extras.spec.bay.roof;
            ui.checkbox(&mut r.use_existing, "Use Existing Roof")
                .on_hover_text("The standard roof ignores the unit, which tucks under the eave");
            ui.checkbox(&mut r.extend_existing, "Extend Existing Roof Over")
                .on_hover_text("The main roof comes down over the unit and follows its shape");
            ui.checkbox(&mut r.rectangular, "Rectangular Roof Over")
                .on_hover_text(
                    "A roof square across the end instead of following the profile of the unit",
                );
        }
        ui.weak("Rebuild the roofs to see a change of the roof options.");
        let own_roof = !self.draft.extras.spec.bay.roof.use_existing;
        ui.add_enabled_ui(own_roof, |ui| self.bay_roof(ui));
        section(ui, "Dimensions");
        {
            let b = &mut self.draft.extras.spec.bay;
            ui.checkbox(&mut b.standard_dimension, "Display Standard Dimension");
            ui.add_enabled_ui(self.draft.style == OpeningStyle::BowWindow, |ui| {
                ui.checkbox(
                    &mut self.draft.extras.spec.bay.center_dimension,
                    "Display Dimensions to Center",
                );
            });
        }
        section(ui, "Components");
        ui.checkbox(
            &mut self.draft.extras.spec.bay.has_components,
            "Has Component Windows",
        );
        let has_components = self.draft.extras.spec.bay.has_components;
        let bow = self.draft.style == OpeningStyle::BowWindow;
        let bay = &mut self.draft.extras.spec.bay;
        ui.add_enabled_ui(has_components, |ui| {
            ui.checkbox(&mut bay.no_trimmers, "No Trimmers for Components");
            ui.checkbox(&mut bay.no_framing_between, "No Framing Between Components");
            ui.add_enabled_ui(bow, |ui| {
                ui.checkbox(&mut bay.connect_outer_casing, "Connect Outer Casing");
            });
            let mut recessed = bay.recess.is_some();
            if ui.checkbox(&mut recessed, "Components Recessed").changed() {
                bay.recess = recessed.then_some(BayRecess::Main);
            }
            if let Some(r) = bay.recess.as_mut() {
                row(ui, "Recessed", |ui| {
                    ui.radio_value(r, BayRecess::Main, "To Main Layer");
                    ui.radio_value(r, BayRecess::Sheathing, "To Sheathing Layer");
                });
            }
        });
        section(ui, "Label");
        row(ui, "Label", |ui| {
            let l = &mut self.draft.extras.spec.bay.label;
            egui::ComboBox::from_id_salt("bay_label")
                .selected_text(l.name())
                .show_ui(ui, |ui| {
                    for m in MulledLabel::ALL {
                        ui.selectable_value(l, m, m.name());
                    }
                });
        });
        section(ui, "Foundation");
        ui.checkbox(
            &mut self.draft.extras.spec.bay.foundation,
            "Build a foundation under the unit (first floor)",
        )
        .on_hover_text("Not built when the unit's floor is raised from the floor of the room");
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use plan_core::geometry::Point;

    fn host() -> Wall {
        Wall::new(
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        )
    }

    fn bay_dialog(style: OpeningStyle) -> OpeningDialog {
        let mut o = Opening::default_window(9, 1, 120.0);
        o.style = style;
        o.width = 50.0;
        o.extras.spec.bay = plan_core::openings::bay::BayUnit::for_style(style);
        OpeningDialog::for_opening(o, &host(), Vec::new(), OpeningExtras::default())
    }

    #[test]
    fn the_general_and_options_panels_draw_for_each_unit() {
        for style in [
            OpeningStyle::BayWindow,
            OpeningStyle::BoxWindow,
            OpeningStyle::BowWindow,
        ] {
            let mut d = bay_dialog(style);
            let ctx = egui::Context::default();
            assert!(d.draw_tab_for_test(&ctx, "General"), "{style:?}");
            assert!(d.draw_tab_for_test(&ctx, "Options"), "{style:?}");
        }
    }

    #[test]
    fn the_bay_values_are_edited_on_the_draft() {
        let mut d = bay_dialog(OpeningStyle::BowWindow);
        {
            let b = &mut d.draft_mut().extras.spec.bay;
            b.segments = 7;
            b.depth = 14.0;
            b.roof.rectangular = true;
            b.lowered_ceiling = Some(LoweredCeiling::default());
        }
        d.sync_stored_for_test();
        let b = &d.draft().extras.spec.bay;
        assert_eq!((b.segments, b.depth), (7, 14.0));
        assert!(b.roof.rectangular && b.lowered_ceiling.is_some());
    }

    use plan_core::openings::bay::LoweredCeiling;
}
