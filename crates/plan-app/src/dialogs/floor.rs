//! The small floor dialogs: Build New Floor, Build Foundation, the Delete
//! Current Floor confirmation (R-59..R-63) and the Floor Defaults and
//! Reference Display dialogs (R-56, R-65), which have their own modules.

use super::floor_defaults::{FloorDefaultsDialog, FloorDefaultsTarget};
use super::reference_display::{self, ReferenceDisplayDialog};
use super::{Fields, Outcome, ERROR_RED};
use crate::editor::rooms_edit::{self, FoundationSpec, FoundationType, NewFloorSpec};
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::floors::{ordinal_floor_name, DeriveFrom, FloorPlacement};
use plan_core::{FloorKind, PlanDefaults, Project};

/// Which floor dialog is open and its draft.
pub enum FloorDialog {
    /// Build New Floor (R-59): what to derive from the current floor, where
    /// to put the new one, its heights, and a foundation if the plan has none.
    NewFloor {
        source: String,
        new: String,
        spec: NewFloorSpec,
        /// The plan has no foundation floor, so one can be built too.
        can_build_foundation: bool,
        build_foundation: bool,
        foundation: FoundationSpec,
        fields: FieldsBox,
    },
    /// Floor Defaults of the active floor or of the plan (R-56).
    Defaults(Box<FloorDefaultsDialog>),
    /// Reference Display options (R-65).
    Reference(Box<ReferenceDisplayDialog>),
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
    /// Build New Floor, deriving from the floor at `current`.
    pub fn new_floor(project: &Project, current: usize, defaults: &PlanDefaults) -> Self {
        let (top, new) = new_floor_names(project);
        let source = project
            .floors
            .get(current)
            .filter(|f| f.kind != FloorKind::Foundation)
            .map_or(top, |f| f.name.clone());
        FloorDialog::NewFloor {
            source,
            new,
            spec: NewFloorSpec::new(),
            can_build_foundation: !project
                .floors
                .first()
                .is_some_and(|f| f.kind == FloorKind::Foundation),
            build_foundation: false,
            foundation: FoundationSpec::from_defaults(defaults),
            fields: FieldsBox::default(),
        }
    }

    /// Floor Defaults of the active floor.
    pub fn defaults_for_floor(cx: &EditorContext) -> Self {
        let f = cx.floor();
        FloorDialog::Defaults(Box::new(FloorDefaultsDialog::new(
            FloorDefaultsTarget::ThisFloor(f.name.clone()),
            f.ceiling_height,
            f.settings.clone(),
            room_type_names(&cx.defaults),
        )))
    }

    /// Floor Defaults of floors built from now on (Default Settings).
    pub fn defaults_for_plan(cx: &EditorContext) -> Self {
        FloorDialog::Defaults(Box::new(FloorDefaultsDialog::new(
            FloorDefaultsTarget::PlanDefaults,
            cx.defaults.rooms.ceiling_height,
            cx.defaults.rooms.floor.clone(),
            room_type_names(&cx.defaults),
        )))
    }

    /// The Reference Display dialog on the choices in force.
    pub fn reference(cx: &EditorContext) -> Self {
        let layer_sets = cx
            .project
            .layer_sets
            .names()
            .into_iter()
            .map(String::from)
            .collect();
        FloorDialog::Reference(Box::new(ReferenceDisplayDialog::new(
            reference_display::settings(&cx.project),
            cx.view_flags.contains(&ViewFlag::ReferenceDisplay),
            cx.project.floors.iter().map(|f| f.name.clone()).collect(),
            cx.floor,
            layer_sets,
        )))
    }

    /// Applies an accepted dialog to the plan.
    pub fn apply(self, cx: &mut EditorContext) {
        match self {
            FloorDialog::NewFloor { .. } => {
                if let Some(spec) = self.new_floor_spec() {
                    rooms_edit::build_new_floor_with(cx, &spec);
                }
            }
            FloorDialog::BuildFoundation { spec, .. } => rooms_edit::build_foundation(cx, spec),
            FloorDialog::ConfirmDelete { .. } => {
                rooms_edit::delete_floor(cx);
            }
            FloorDialog::Defaults(d) => match d.target() {
                FloorDefaultsTarget::ThisFloor(_) => {
                    rooms_edit::apply_floor_defaults(
                        cx,
                        d.ceiling_height(),
                        d.settings().clone(),
                        d.as_plan_default(),
                    );
                }
                FloorDefaultsTarget::PlanDefaults => {
                    rooms_edit::set_plan_floor_defaults(
                        cx,
                        d.ceiling_height(),
                        d.settings().clone(),
                    );
                    cx.status = "Updated the defaults for new floors".into();
                }
            },
            FloorDialog::Reference(d) => apply_reference_display(cx, &d),
        }
    }

    /// The choices of a Build New Floor dialog, with the foundation to build
    /// when one was asked for.
    pub fn new_floor_spec(&self) -> Option<NewFloorSpec> {
        match self {
            FloorDialog::NewFloor {
                spec,
                build_foundation,
                foundation,
                can_build_foundation,
                ..
            } => {
                let mut spec = spec.clone();
                spec.foundation =
                    (*build_foundation && *can_build_foundation).then_some(*foundation);
                Some(spec)
            }
            _ => None,
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
            FloorDialog::Defaults(_) => "Floor Defaults",
            FloorDialog::Reference(_) => "Reference Display",
        }
    }

    fn error(&self) -> Option<&'static str> {
        match self {
            FloorDialog::NewFloor {
                build_foundation: true,
                can_build_foundation: true,
                foundation,
                fields,
                ..
            } => {
                if fields.0.any_invalid() {
                    Some("Fix the highlighted field")
                } else if foundation.kind == FoundationType::WallsWithFootings
                    && foundation.stem_height <= 0.0
                {
                    Some("Stem wall height must be greater than zero")
                } else {
                    None
                }
            }
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
        match self {
            FloorDialog::Defaults(d) => return d.show(ctx),
            FloorDialog::Reference(d) => return d.show(ctx),
            _ => {}
        }
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
                        spec,
                        can_build_foundation,
                        build_foundation,
                        foundation,
                        fields,
                    } => {
                        ui.label(RichText::new("Plan").strong());
                        ui.radio_value(
                            &mut spec.derive,
                            DeriveFrom::ExteriorWalls,
                            format!("Derive new {new} plan from the {source} exterior walls"),
                        );
                        ui.radio_value(
                            &mut spec.derive,
                            DeriveFrom::AllWalls,
                            format!("Derive new {new} plan from all the {source} walls"),
                        );
                        ui.radio_value(
                            &mut spec.derive,
                            DeriveFrom::Blank,
                            format!("Make new blank plan for the {new}"),
                        );
                        ui.add_enabled_ui(spec.derive != DeriveFrom::Blank, |ui| {
                            ui.checkbox(&mut spec.copy_rooms, "Copy room names and types");
                        });
                        ui.checkbox(&mut spec.copy_foundation, "Copy slab, pad and pier data");
                        ui.add_space(4.0);
                        ui.label(RichText::new("Place").strong());
                        ui.horizontal(|ui| {
                            ui.radio_value(
                                &mut spec.place,
                                FloorPlacement::Above,
                                format!("Above the {source}"),
                            );
                            ui.radio_value(
                                &mut spec.place,
                                FloorPlacement::Below,
                                format!("Below the {source}"),
                            );
                        });
                        ui.add_space(4.0);
                        ui.label(RichText::new("Heights").strong());
                        ui.radio_value(
                            &mut spec.heights_from_defaults,
                            true,
                            "From the Floor Defaults",
                        );
                        ui.radio_value(
                            &mut spec.heights_from_defaults,
                            false,
                            format!("Same as the {source}"),
                        );
                        if *can_build_foundation {
                            ui.add_space(4.0);
                            ui.checkbox(build_foundation, "Also build a foundation");
                            if *build_foundation {
                                foundation_fields(ui, foundation, fields);
                            }
                        }
                    }
                    FloorDialog::Defaults(_) | FloorDialog::Reference(_) => {}
                    FloorDialog::BuildFoundation { spec, fields } => {
                        foundation_fields(ui, spec, fields);
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

fn room_type_names(d: &PlanDefaults) -> Vec<String> {
    d.rooms.room_types.iter().map(|t| t.name.clone()).collect()
}

/// Reference Display OK: keeps the choices, writes the floor into the active
/// plan view and turns the display on or off.
fn apply_reference_display(cx: &mut EditorContext, d: &ReferenceDisplayDialog) {
    let settings = d.settings().clone();
    cx.begin_change("Reference Display");
    let offset = reference_display::relative_offset(&settings, cx.floor);
    let active = cx.project.active_plan_view.clone();
    if let Some(v) = cx.project.plan_views.iter_mut().find(|v| v.name == active) {
        v.reference_floor = Some(offset);
        v.reference_display = d.show_display();
    }
    reference_display::set_settings(settings);
    if d.show_display() {
        cx.view_flags.insert(ViewFlag::ReferenceDisplay);
    } else {
        cx.view_flags.remove(&ViewFlag::ReferenceDisplay);
    }
    cx.mark_dirty();
    cx.status = "Updated the reference display".into();
}

/// The foundation type radios and the stem wall heights, shared by Build
/// Foundation and the foundation option of Build New Floor.
fn foundation_fields(ui: &mut egui::Ui, spec: &mut FoundationSpec, fields: &mut FieldsBox) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn cx_with_house() -> EditorContext {
        use plan_core::{Point, WallKind};
        let mut cx = EditorContext::new(crate::plan_defaults::embedded());
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 180.0),
            Point::new(0.0, 180.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.125, WallKind::Exterior);
        }
        cx.mark_dirty();
        cx.refresh();
        cx
    }

    #[test]
    fn the_floor_defaults_dialog_edits_the_floor_and_the_plan_defaults() {
        let mut cx = cx_with_house();
        let FloorDialog::Defaults(mut d) = FloorDialog::defaults_for_floor(&cx) else {
            panic!("floor defaults");
        };
        d.set_ceiling_height(100.0);
        d.settings_mut().floor_finish_thickness = 0.5;
        // Make it the plan default too.
        let mut dlg = FloorDialog::Defaults(d);
        if let FloorDialog::Defaults(d) = &mut dlg {
            d.settings_mut().default_room_type = "Bedroom".into();
        }
        dlg.apply(&mut cx);
        assert_eq!(cx.floor().ceiling_height, 100.0);
        assert_eq!(cx.floor().settings.floor_finish_thickness, 0.5);
        assert_eq!(cx.floor().settings.default_room_type, "Bedroom");
        // Not the plan default unless asked.
        assert_ne!(cx.defaults.rooms.ceiling_height, 100.0);
        // The Default Settings entry sets what the next floor starts with.
        let FloorDialog::Defaults(mut d) = FloorDialog::defaults_for_plan(&cx) else {
            panic!("plan defaults");
        };
        assert_eq!(*d.target(), FloorDefaultsTarget::PlanDefaults);
        d.set_ceiling_height(96.0);
        FloorDialog::Defaults(d).apply(&mut cx);
        assert_eq!(cx.defaults.rooms.ceiling_height, 96.0);
        let up = rooms_edit::build_new_floor_with(&mut cx, &NewFloorSpec::new()).unwrap();
        assert_eq!(cx.project.floors[up].ceiling_height, 96.0);
    }

    #[test]
    fn the_new_floor_dialog_hands_its_choices_to_the_command() {
        let mut cx = cx_with_house();
        let mut d = FloorDialog::new_floor(&cx.project, cx.floor, &cx.defaults);
        let FloorDialog::NewFloor {
            spec,
            build_foundation,
            ..
        } = &mut d
        else {
            panic!("new floor");
        };
        spec.derive = DeriveFrom::Blank;
        spec.place = FloorPlacement::Below;
        *build_foundation = true;
        let spec = d.new_floor_spec().unwrap();
        assert_eq!(spec.derive, DeriveFrom::Blank);
        assert!(
            spec.foundation.is_some(),
            "no foundation yet, so one is built"
        );
        d.apply(&mut cx);
        assert_eq!(
            cx.project.floors.len(),
            3,
            "foundation, the new floor, the old floor"
        );
        assert_eq!(cx.project.floors[0].kind, FloorKind::Foundation);
        assert!(cx.floor().walls.is_empty());
    }

    #[test]
    fn the_reference_display_dialog_turns_the_display_on_for_its_floor() {
        use super::reference_display::{reset_settings, ReferenceFloor};
        reset_settings();
        let mut cx = cx_with_house();
        cx.project.build_new_floor(false);
        cx.floor = 0;
        assert!(!cx.view_flags.contains(&ViewFlag::ReferenceDisplay));
        let FloorDialog::Reference(mut d) = FloorDialog::reference(&cx) else {
            panic!("reference");
        };
        d.set_show_display(true);
        d.settings_mut().floor = ReferenceFloor::Above;
        d.settings_mut().color = [200, 40, 40];
        FloorDialog::Reference(d).apply(&mut cx);
        assert!(cx.view_flags.contains(&ViewFlag::ReferenceDisplay));
        assert_eq!(
            crate::editor::render::reference_polygons(&cx).len(),
            0,
            "the floor above is blank"
        );
        let s = reference_display::settings(&cx.project);
        assert_eq!(s.floor, ReferenceFloor::Above);
        assert_eq!(s.color, [200, 40, 40]);
        let view = cx.project.current_plan_view().unwrap();
        assert_eq!(view.reference_floor, Some(1));
        assert!(view.reference_display);
        reset_settings();
    }

    /// Every floor dialog draws a frame without panicking and stays open.
    #[test]
    fn the_floor_dialogs_draw_a_frame() {
        let mut cx = cx_with_house();
        cx.project.build_new_floor(false);
        let ctx = egui::Context::default();
        let mut all = vec![
            FloorDialog::new_floor(&cx.project, cx.floor, &cx.defaults),
            FloorDialog::defaults_for_floor(&cx),
            FloorDialog::defaults_for_plan(&cx),
            FloorDialog::reference(&cx),
            FloorDialog::confirm_delete("2nd Floor"),
            FloorDialog::foundation(FoundationSpec::from_defaults(&cx.defaults)),
        ];
        if let FloorDialog::NewFloor {
            can_build_foundation,
            build_foundation,
            ..
        } = &mut all[0]
        {
            assert!(*can_build_foundation);
            *build_foundation = true;
        }
        for d in &mut all {
            for _ in 0..2 {
                let mut out = Outcome::Cancel;
                let _ = ctx.run(egui::RawInput::default(), |ctx| out = d.show(ctx));
                assert_eq!(out, Outcome::Open);
            }
        }
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
