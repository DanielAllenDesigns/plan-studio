//! The small floor dialogs: Build New Floor, Build Foundation and the Delete
//! Current Floor confirmation (R-59..R-63).

use super::{Fields, Outcome, ERROR_RED};
use crate::editor::rooms_edit::{FoundationSpec, FoundationType};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::floors::ordinal_floor_name;
use plan_core::{FloorKind, Project};

/// Which floor dialog is open and its draft.
pub enum FloorDialog {
    /// Build New Floor: derive from the floor below or start blank.
    NewFloor {
        source: String,
        new: String,
        derive: bool,
    },
    BuildFoundation {
        spec: FoundationSpec,
        fields: FieldsBox,
    },
    ConfirmDelete {
        floor: String,
    },
}

/// The text buffers of the foundation dialog's length fields.
#[derive(Default)]
pub struct FieldsBox(Fields);

/// The names Build New Floor offers: the floor it derives from and the one
/// it creates ("1st Floor" and "2nd Floor").
pub fn new_floor_names(project: &Project) -> (String, String) {
    let normal = project
        .floors
        .iter()
        .filter(|f| f.kind == FloorKind::Normal)
        .count()
        .max(1);
    (ordinal_floor_name(normal), ordinal_floor_name(normal + 1))
}

impl FloorDialog {
    pub fn new_floor(project: &Project) -> Self {
        let (source, new) = new_floor_names(project);
        FloorDialog::NewFloor {
            source,
            new,
            derive: true,
        }
    }

    pub fn foundation(spec: FoundationSpec) -> Self {
        FloorDialog::BuildFoundation {
            spec,
            fields: FieldsBox::default(),
        }
    }

    pub fn confirm_delete(floor_name: &str) -> Self {
        FloorDialog::ConfirmDelete {
            floor: floor_name.to_string(),
        }
    }

    fn title(&self) -> &'static str {
        match self {
            FloorDialog::NewFloor { .. } => "Build New Floor",
            FloorDialog::BuildFoundation { .. } => "Build Foundation",
            FloorDialog::ConfirmDelete { .. } => "Delete Current Floor",
        }
    }

    fn error(&self) -> Option<&'static str> {
        match self {
            FloorDialog::BuildFoundation { spec, fields } => {
                if fields.0.any_invalid() {
                    Some("Fix the highlighted field")
                } else if spec.kind == FoundationType::WallsWithFootings && spec.stem_height <= 0.0
                {
                    Some("Stem wall height must be greater than zero")
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Draws the dialog; Enter is OK and Esc is Cancel.
    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        let mut outcome = Outcome::Open;
        let mut open = true;
        let error = self.error();
        let title = self.title();
        egui::Window::new(title)
            .id(egui::Id::new(("floor_dialog", title)))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                match self {
                    FloorDialog::NewFloor {
                        source,
                        new,
                        derive,
                    } => {
                        ui.radio_value(
                            derive,
                            true,
                            format!("Derive new {new} plan from the {source} plan"),
                        );
                        ui.radio_value(derive, false, format!("Make new blank plan for the {new}"));
                        ui.add_space(4.0);
                        ui.weak("Deriving copies the exterior walls with their doors and windows.");
                    }
                    FloorDialog::BuildFoundation { spec, fields } => {
                        ui.label(RichText::new("Foundation Type").strong());
                        for t in FoundationType::ALL {
                            ui.radio_value(&mut spec.kind, t, t.name());
                        }
                        ui.add_space(6.0);
                        let walls = spec.kind == FoundationType::WallsWithFootings;
                        ui.add_enabled_ui(walls, |ui| {
                            super::row(ui, "Stem Wall Height", |ui| {
                                fields.0.length(ui, "stem_h", &mut spec.stem_height)
                            });
                            super::row(ui, "Minimum Stem Wall", |ui| {
                                fields.0.length(ui, "stem_min", &mut spec.min_stem_height)
                            });
                        });
                        ui.checkbox(&mut spec.garage_floor, "Build Garage Floor\u{2026}")
                            .on_hover_text("Not stored by the model yet");
                    }
                    FloorDialog::ConfirmDelete { floor } => {
                        ui.label(format!(
                            "Delete {floor} and everything on it? Undo brings it back."
                        ));
                    }
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
    use plan_core::PlanDefaults;

    #[test]
    fn names_follow_the_floor_stack() {
        let mut p = Project::new("t");
        assert_eq!(
            new_floor_names(&p),
            ("1st Floor".to_string(), "2nd Floor".to_string())
        );
        p.build_new_floor(false);
        assert_eq!(new_floor_names(&p).1, "3rd Floor");
    }

    #[test]
    fn foundation_dialog_validates_the_stem_height() {
        let mut spec = FoundationSpec::from_defaults(&PlanDefaults::chief_x18_daniel());
        spec.stem_height = 0.0;
        let d = FloorDialog::foundation(spec);
        assert!(d.error().is_some());
        spec.kind = FoundationType::MonolithicSlab;
        assert!(FloorDialog::foundation(spec).error().is_none());
    }
}
