//! Floor Defaults (R-56, R-58): the ceiling height, platform and finish
//! thicknesses, default room type and materials of a floor. Opened from the
//! Floor Defaults toolbar button and Build > Floor > Floor Defaults for the
//! active floor, and from Edit > Default Settings > Floors and Rooms > Floor
//! Defaults for the defaults every new floor starts with.

use super::{row, section, Fields, Outcome, ERROR_RED};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::floors::FloorSettings;
use plan_core::units::fmt_ft_in;

/// What the dialog edits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FloorDefaultsTarget {
    /// The active floor (its name is shown).
    ThisFloor(String),
    /// The defaults of floors built from now on.
    PlanDefaults,
}

pub struct FloorDefaultsDialog {
    target: FloorDefaultsTarget,
    ceiling_height: f64,
    settings: FloorSettings,
    /// Also make the values the plan defaults (this-floor target only).
    as_plan_default: bool,
    /// Names of the plan's room types, for the default room type list.
    types: Vec<String>,
    fields: Fields,
}

// The setters are the dialog's model API (the tests drive it); the UI edits
// the same fields directly.
#[allow(dead_code)]
impl FloorDefaultsDialog {
    pub fn new(
        target: FloorDefaultsTarget,
        ceiling_height: f64,
        settings: FloorSettings,
        types: Vec<String>,
    ) -> Self {
        Self {
            target,
            ceiling_height,
            settings,
            as_plan_default: false,
            types,
            fields: Fields::default(),
        }
    }

    pub fn target(&self) -> &FloorDefaultsTarget {
        &self.target
    }

    pub fn ceiling_height(&self) -> f64 {
        self.ceiling_height
    }

    pub fn settings(&self) -> &FloorSettings {
        &self.settings
    }

    pub fn as_plan_default(&self) -> bool {
        self.as_plan_default
    }

    pub fn set_ceiling_height(&mut self, v: f64) {
        self.ceiling_height = v;
    }

    pub fn settings_mut(&mut self) -> &mut FloorSettings {
        &mut self.settings
    }

    fn error(&self) -> Option<&'static str> {
        if self.fields.any_invalid() {
            Some("Fix the highlighted field")
        } else if self.ceiling_height <= 0.0 {
            Some("Ceiling height must be greater than zero")
        } else if self.settings.floor_structure_thickness < 0.0
            || self.settings.ceiling_structure_thickness < 0.0
        {
            Some("Structure thickness cannot be negative")
        } else {
            None
        }
    }

    /// Draws the dialog; Enter is OK and Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error();
        let title = match &self.target {
            FloorDefaultsTarget::ThisFloor(_) => "Floor Defaults",
            FloorDefaultsTarget::PlanDefaults => "Floor Defaults (new floors)",
        };
        egui::Window::new(title)
            .id(egui::Id::new("floor_defaults_dialog"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(420.0);
                if let FloorDefaultsTarget::ThisFloor(name) = &self.target {
                    ui.label(RichText::new(name.as_str()).strong());
                }
                section(ui, "Heights");
                self.fields.length_row(
                    ui,
                    "Ceiling Height",
                    "fd_ceiling",
                    &mut self.ceiling_height,
                );
                self.fields.length_row(
                    ui,
                    "Floor Structure",
                    "fd_floor_struct",
                    &mut self.settings.floor_structure_thickness,
                );
                self.fields.length_row(
                    ui,
                    "Ceiling Structure",
                    "fd_ceil_struct",
                    &mut self.settings.ceiling_structure_thickness,
                );
                row(ui, "Floor Height", |ui| {
                    ui.label(fmt_ft_in(self.settings.floor_height(self.ceiling_height)));
                });
                ui.weak("Floors above move with these; walls at the old ceiling height follow.");

                section(ui, "Finish");
                self.fields.length_row(
                    ui,
                    "Floor Finish",
                    "fd_floor_fin",
                    &mut self.settings.floor_finish_thickness,
                );
                self.fields.length_row(
                    ui,
                    "Ceiling Finish",
                    "fd_ceil_fin",
                    &mut self.settings.ceiling_finish_thickness,
                );

                section(ui, "New Rooms");
                row(ui, "Default Room Type", |ui| {
                    let shown = if self.settings.default_room_type.is_empty() {
                        "(first in the list)".to_string()
                    } else {
                        self.settings.default_room_type.clone()
                    };
                    egui::ComboBox::from_id_salt("fd_room_type")
                        .width(200.0)
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings.default_room_type,
                                String::new(),
                                "(first in the list)",
                            );
                            for t in &self.types {
                                ui.selectable_value(
                                    &mut self.settings.default_room_type,
                                    t.clone(),
                                    t,
                                );
                            }
                        });
                });
                row(ui, "Floor Material", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings.floor_material)
                            .desired_width(200.0),
                    );
                });
                row(ui, "Ceiling Material", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings.ceiling_material)
                            .desired_width(200.0),
                    );
                });
                row(ui, "Wall Material", |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.settings.wall_material)
                            .desired_width(200.0),
                    );
                });
                if matches!(self.target, FloorDefaultsTarget::ThisFloor(_)) {
                    ui.add_space(4.0);
                    ui.checkbox(
                        &mut self.as_plan_default,
                        "Use these for floors built from now on",
                    );
                }
                ui.add_space(8.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let ok = egui::Button::new(RichText::new("   OK   ").strong());
                    if ui.add_enabled(error.is_none(), ok).clicked() {
                        outcome = Outcome::Ok;
                    }
                    if ui.button("Cancel").clicked() {
                        outcome = Outcome::Cancel;
                    }
                    if let Some(e) = error {
                        ui.colored_label(ERROR_RED, e);
                    }
                });
            });
        if !open {
            outcome = Outcome::Cancel;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        } else if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_height_is_ceiling_plus_both_platforms() {
        let mut d = FloorDefaultsDialog::new(
            FloorDefaultsTarget::PlanDefaults,
            96.0,
            FloorSettings::default(),
            vec!["Bedroom".into()],
        );
        assert_eq!(d.settings().floor_height(d.ceiling_height()), 96.0 + 10.25);
        d.settings_mut().ceiling_structure_thickness = 1.0;
        assert_eq!(d.settings().floor_height(d.ceiling_height()), 96.0 + 11.25);
        assert!(d.error().is_none());
        d.set_ceiling_height(0.0);
        assert!(d.error().is_some());
    }
}
