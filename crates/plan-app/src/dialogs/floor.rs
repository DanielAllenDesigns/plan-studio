//! The small floor dialogs: Build New Floor and Insert New Floor, Build
//! Foundation and the Foundation Defaults, the Delete Current Floor
//! confirmation (R-59..R-63, R-119..R-121, R-129..R-134) and the Floor
//! Defaults and Reference Display dialogs (R-56, R-65), which have their own
//! modules.

use super::floor_defaults::{FloorDefaultsDialog, FloorDefaultsTarget};
use super::foundation::FoundationForm;
use super::reference_display::{self, ReferenceDisplayDialog};
use super::{Outcome, ERROR_RED};
use crate::editor::rooms_edit::{self, FoundationSpec, NewFloorSpec};
use crate::editor::EditorContext;
use crate::toolbar::ViewFlag;
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, RichText};
use plan_core::defaults::WallTypeDef;
use plan_core::floors::{ordinal_floor_name, DeriveFrom, FloorPlacement};
use plan_core::{FloorKind, PlanDefaults, Project};

/// Whether Move Highest Floor's Roof Up can be ticked, and why not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoofChoice {
    pub available: bool,
    pub reason: &'static str,
}

impl RoofChoice {
    /// Available only when roof planes are built on the highest floor and
    /// neither the Auto Rebuild Roofs preference nor that roof's own switch
    /// is on (manual p. 764).
    pub fn of(project: &Project) -> Self {
        let top = project
            .floors
            .iter()
            .rposition(|f| f.kind == FloorKind::Normal && !f.is_cad_detail());
        let Some(t) = top else {
            return Self {
                available: false,
                reason: "There is no floor to move a roof from",
            };
        };
        let set = crate::editor::roof_view::load(&project.floors[t]);
        if set.planes.is_empty() {
            return Self {
                available: false,
                reason: "No roof planes are built on the highest floor",
            };
        }
        let auto = super::preferences::pages::current()
            .architectural
            .auto_rebuild_roofs
            || set.settings.as_ref().is_some_and(|s| s.auto_rebuild);
        if auto {
            return Self {
                available: false,
                reason: "Not available while Auto Rebuild Roofs is on",
            };
        }
        Self {
            available: true,
            reason: "",
        }
    }
}

/// Which floor dialog is open and its draft.
#[allow(clippy::large_enum_variant)]
pub enum FloorDialog {
    /// Build New Floor or Insert New Floor (R-59, R-121): what to derive
    /// from the current floor, where to put the new one, its heights, the
    /// roof and elevation options, and a foundation if the plan has none.
    NewFloor {
        source: String,
        new: String,
        spec: NewFloorSpec,
        /// Insert New Floor: the floor goes below the current one.
        insert: bool,
        roof: RoofChoice,
        /// The plan has no foundation floor, so one can be built too.
        can_build_foundation: bool,
        build_foundation: bool,
        foundation: Box<FoundationForm>,
    },
    /// Floor Defaults of the active floor or of the plan (R-56).
    Defaults(Box<FloorDefaultsDialog>),
    /// Reference Display options (R-65).
    Reference(Box<ReferenceDisplayDialog>),
    BuildFoundation {
        form: Box<FoundationForm>,
    },
    /// Edit > Default Settings > Foundation > Edit: the same panels, and OK
    /// changes no floor (manual p. 738).
    FoundationDefaults {
        form: Box<FoundationForm>,
    },
    ConfirmDelete {
        floor: String,
    },
}

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
            insert: false,
            roof: RoofChoice::of(project),
            can_build_foundation: !project
                .floors
                .first()
                .is_some_and(|f| f.kind == FloorKind::Foundation),
            build_foundation: false,
            foundation: Box::new(foundation_form(
                FoundationSpec::from_defaults(defaults),
                defaults,
            )),
        }
    }

    /// Insert New Floor (manual p. 764): a floor below the floor at
    /// `current`, derived from its walls.
    pub fn insert_floor(project: &Project, current: usize, defaults: &PlanDefaults) -> Self {
        let mut d = Self::new_floor(project, current, defaults);
        if let FloorDialog::NewFloor {
            spec,
            insert,
            can_build_foundation,
            ..
        } = &mut d
        {
            spec.place = FloorPlacement::Below;
            *insert = true;
            // The foundation goes in with Build New Floor, not here.
            *can_build_foundation = false;
        }
        d
    }

    /// Floor Defaults of the active floor.
    pub fn defaults_for_floor(cx: &EditorContext) -> Self {
        let f = cx.floor();
        FloorDialog::Defaults(Box::new(
            FloorDefaultsDialog::new(
                FloorDefaultsTarget::ThisFloor(f.name.clone()),
                f.ceiling_height,
                f.settings.clone(),
                room_type_names(&cx.defaults),
            )
            .with_library(cx.project.assemblies.clone())
            .with_moldings(crate::tools::molding::floor_molding_table(cx)),
        ))
    }

    /// Floor Defaults of floors built from now on (Default Settings).
    pub fn defaults_for_plan(cx: &EditorContext) -> Self {
        FloorDialog::Defaults(Box::new(
            FloorDefaultsDialog::new(
                FloorDefaultsTarget::PlanDefaults,
                cx.defaults.rooms.ceiling_height,
                cx.defaults.rooms.floor.clone(),
                room_type_names(&cx.defaults),
            )
            .with_library(cx.project.assemblies.clone()),
        ))
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
        let self_spec = self.new_floor_spec();
        match self {
            FloorDialog::NewFloor { mut foundation, .. } => {
                store_wall_types(cx, foundation.take_edited_types());
                if let Some(spec) = self_spec {
                    rooms_edit::build_new_floor_with(cx, &spec);
                }
            }
            FloorDialog::BuildFoundation { mut form } => {
                store_wall_types(cx, form.take_edited_types());
                rooms_edit::build_foundation(cx, form.spec)
            }
            FloorDialog::FoundationDefaults { mut form } => {
                store_wall_types(cx, form.take_edited_types());
                let o = form.spec.to_options();
                cx.defaults.foundation = o.settings;
                cx.defaults.foundation_wall.height = form.spec.stem_height;
                cx.mark_dirty();
                cx.status = "Updated the Foundation Defaults".into();
            }
            FloorDialog::ConfirmDelete { .. } => {
                rooms_edit::delete_floor(cx);
            }
            FloorDialog::Defaults(mut d) => match d.target() {
                FloorDefaultsTarget::ThisFloor(_) => {
                    let saved = d.take_saved();
                    let moldings = d.changed_moldings().cloned();
                    // The structure and the moldings are one undo step.
                    cx.undo_group(|cx| {
                        rooms_edit::apply_floor_defaults(
                            cx,
                            d.ceiling_height(),
                            d.settings().clone(),
                            d.as_plan_default(),
                        );
                        if let Some(table) = moldings {
                            crate::tools::molding::set_floor_molding_table(cx, table);
                        }
                    });
                    store_saved_assemblies(cx, saved);
                }
                FloorDefaultsTarget::PlanDefaults => {
                    let saved = d.take_saved();
                    rooms_edit::set_plan_floor_defaults(
                        cx,
                        d.ceiling_height(),
                        d.settings().clone(),
                    );
                    store_saved_assemblies(cx, saved);
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
                roof,
                ..
            } => {
                let mut spec = spec.clone();
                spec.foundation =
                    (*build_foundation && *can_build_foundation).then_some(foundation.spec);
                spec.move_roof_up &= roof.available;
                Some(spec)
            }
            _ => None,
        }
    }

    pub fn foundation(spec: FoundationSpec) -> Self {
        FloorDialog::BuildFoundation {
            form: Box::new(FoundationForm::new(spec)),
        }
    }

    /// Build Foundation on the Foundation Defaults and the plan's wall
    /// types; a foundation already built opens on the choices it was built
    /// with, so building again rebuilds it in place.
    pub fn build_foundation(cx: &EditorContext) -> Self {
        let mut spec = FoundationSpec::from_defaults(&cx.defaults);
        if let Some(o) = cx
            .project
            .floors
            .first()
            .filter(|f| f.kind == FloorKind::Foundation)
            .and_then(|f| f.settings.foundation_options)
        {
            spec.restore(&o);
        }
        spec.platform = cx
            .project
            .floors
            .iter()
            .find(|f| f.kind != FloorKind::Foundation)
            .map_or(spec.platform, |f| f.settings.floor_structure_thickness);
        FloorDialog::BuildFoundation {
            form: Box::new(foundation_form(spec, &cx.defaults).with_wall_types(
                cx.wall_types().to_vec(),
                &cx.defaults.foundation_wall.wall_type,
            )),
        }
    }

    /// The Foundation Defaults dialog (Edit > Default Settings > Foundation).
    pub fn foundation_defaults(cx: &EditorContext) -> Self {
        FloorDialog::FoundationDefaults {
            form: Box::new(
                foundation_form(FoundationSpec::from_defaults(&cx.defaults), &cx.defaults)
                    .with_wall_types(
                        cx.wall_types().to_vec(),
                        &cx.defaults.foundation_wall.wall_type,
                    ),
            ),
        }
    }

    pub fn confirm_delete(floor_name: &str) -> Self {
        FloorDialog::ConfirmDelete {
            floor: floor_name.to_string(),
        }
    }

    fn title(&self) -> &'static str {
        match self {
            FloorDialog::NewFloor { insert: true, .. } => "Insert New Floor",
            FloorDialog::NewFloor { .. } => "Build New Floor",
            FloorDialog::BuildFoundation { .. } => "Build Foundation",
            FloorDialog::FoundationDefaults { .. } => "Foundation Defaults",
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
                ..
            } => foundation.error(),
            FloorDialog::BuildFoundation { form } | FloorDialog::FoundationDefaults { form } => {
                form.error()
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
                ui.set_min_width(420.0);
                match self {
                    FloorDialog::NewFloor {
                        source,
                        new,
                        spec,
                        insert,
                        roof,
                        can_build_foundation,
                        build_foundation,
                        foundation,
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
                        ui.add_enabled(
                            roof.available,
                            egui::Checkbox::new(
                                &mut spec.move_roof_up,
                                "Move Highest Floor's Roof Up",
                            ),
                        )
                        .on_disabled_hover_text(roof.reason)
                        .on_hover_text("Roof planes on the highest floor rise with the new floor.");
                        ui.checkbox(
                            &mut spec.step_elevations,
                            if *insert {
                                "Step floor/ceiling elevations to match existing floors"
                            } else {
                                "Step floor/ceiling elevations to match existing floor"
                            },
                        )
                        .on_hover_text(
                            "Keep the ceiling heights of the existing floor by stepping the \
                             new floor to match.",
                        );
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
                                foundation.show(ui);
                            }
                        }
                        ui.add_space(4.0);
                        ui.checkbox(&mut spec.attic, "Also build an attic floor");
                    }
                    FloorDialog::Defaults(_) | FloorDialog::Reference(_) => {}
                    FloorDialog::BuildFoundation { form }
                    | FloorDialog::FoundationDefaults { form } => {
                        form.show(ui);
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
        // The Wall Type Definitions window an Edit button opened.
        let editing = match self {
            FloorDialog::BuildFoundation { form } | FloorDialog::FoundationDefaults { form } => {
                form.child(ctx);
                form.editing()
            }
            FloorDialog::NewFloor { foundation, .. } => {
                foundation.child(ctx);
                foundation.editing()
            }
            _ => false,
        };
        if !open {
            outcome = Outcome::Cancel;
        }
        if editing {
            return Outcome::Open;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            outcome = Outcome::Cancel;
        } else if error.is_none() && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter)) {
            outcome = Outcome::Ok;
        }
        outcome
    }
}

/// The form of a Foundation dialog with the plan's wall types.
fn foundation_form(spec: FoundationSpec, defaults: &PlanDefaults) -> FoundationForm {
    FoundationForm::new(spec).with_wall_types(
        defaults.wall_types.clone(),
        &defaults.foundation_wall.wall_type,
    )
}

/// Puts the wall types edited through a Foundation dialog's Edit buttons in
/// the plan's defaults and registry.
fn store_wall_types(cx: &mut EditorContext, edited: Vec<WallTypeDef>) {
    for t in edited {
        match cx.defaults.wall_types.iter_mut().find(|x| x.name == t.name) {
            Some(slot) => *slot = t.clone(),
            None => cx.defaults.wall_types.push(t.clone()),
        }
        if !cx.project.wall_types.is_empty() {
            cx.project.register_wall_type(t);
        }
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

/// Puts the definitions saved from a Material Layers window into the plan's
/// library.
fn store_saved_assemblies(
    cx: &mut EditorContext,
    saved: Vec<plan_core::assemblies::NamedAssembly>,
) {
    if saved.is_empty() {
        return;
    }
    for n in saved {
        cx.project
            .assemblies
            .save_named(n.kind, &n.name, n.assembly);
    }
    cx.mark_dirty();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::rooms_edit::FoundationType;

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

    /// Each Foundation Type draws its own options, and the room choices.
    #[test]
    fn every_foundation_type_draws_its_options() {
        let ctx = egui::Context::default();
        for kind in FoundationType::ALL {
            let mut spec = FoundationSpec::from_defaults(&PlanDefaults::chief_x18_daniel());
            spec.kind = kind;
            let mut d = FloorDialog::foundation(spec);
            for _ in 0..2 {
                let mut out = Outcome::Cancel;
                let _ = ctx.run(egui::RawInput::default(), |ctx| out = d.show(ctx));
                assert_eq!(out, Outcome::Open, "{kind:?}");
            }
            assert!(d.error().is_none(), "{kind:?}");
        }
        // The new-floor dialog draws the foundation options and the attic
        // check box too.
        let mut cx = cx_with_house();
        let mut d = FloorDialog::new_floor(&cx.project, cx.floor, &cx.defaults);
        if let FloorDialog::NewFloor {
            build_foundation,
            spec,
            ..
        } = &mut d
        {
            *build_foundation = true;
            spec.attic = true;
        }
        let mut out = Outcome::Cancel;
        let _ = ctx.run(egui::RawInput::default(), |ctx| out = d.show(ctx));
        assert_eq!(out, Outcome::Open);
        d.apply(&mut cx);
        assert!(cx.project.floors.iter().any(|f| f.kind == FloorKind::Attic));
        assert_eq!(cx.project.floors[0].kind, FloorKind::Foundation);
    }

    #[test]
    fn the_foundation_dialog_builds_what_it_shows() {
        let mut cx = cx_with_house();
        let mut spec = FoundationSpec::from_defaults(&cx.defaults);
        spec.kind = FoundationType::Piers;
        spec.pier_spacing = 60.0;
        FloorDialog::foundation(spec).apply(&mut cx);
        let layer = plan_core::foundation::FoundationLayer::load(&cx.project.floors[0]);
        assert!(layer.piers.len() >= 6, "{}", layer.piers.len());
        let mut spec = FoundationSpec::from_defaults(&cx.defaults);
        spec.stem_height = 100.0;
        FloorDialog::foundation(spec).apply(&mut cx);
        assert_eq!(cx.project.floors[0].room_names[0].room_type, "Basement");
        assert_eq!(cx.project.floors.len(), 2);
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

    #[test]
    fn move_roof_up_needs_roof_planes_and_no_auto_rebuild() {
        let mut cx = cx_with_house();
        // The test plan's Preferences start with Auto Rebuild Roofs on; this
        // test turns it off where it checks the roof's own switch.
        crate::dialogs::preferences::pages::update(|p| p.architectural.auto_rebuild_roofs = false);
        // No roof built: not available.
        let none = RoofChoice::of(&cx.project);
        assert!(!none.available && none.reason.contains("No roof"));
        // A plane on the top floor, Auto Rebuild Roofs off in the roof's own
        // settings and in the preferences: available.
        let plane = serde_json::json!({
            "kind": "plane",
            "id": 900,
            "polygon3d": [[0.0, 100.0, 0.0], [240.0, 100.0, 0.0], [120.0, 140.0, -60.0]],
            "pitch": 8.0,
            "baseline": [[0.0, 0.0], [240.0, 0.0]],
            "auto": false,
        });
        cx.floor_mut().roofs.push(plane);
        cx.floor_mut().roofs.push(serde_json::json!({
            "kind": "settings",
            "auto_rebuild": true,
        }));
        let on = RoofChoice::of(&cx.project);
        assert!(
            !on.available && on.reason.contains("Auto Rebuild Roofs"),
            "{on:?}"
        );
        cx.floor_mut().roofs.pop();
        cx.floor_mut().roofs.push(serde_json::json!({
            "kind": "settings",
            "auto_rebuild": false,
        }));
        assert!(RoofChoice::of(&cx.project).available);
        // The dialog only passes the option on when it is available.
        let mut d = FloorDialog::new_floor(&cx.project, cx.floor, &cx.defaults);
        if let FloorDialog::NewFloor { spec, .. } = &mut d {
            spec.move_roof_up = true;
        }
        assert!(d.new_floor_spec().unwrap().move_roof_up);
        let mut q = cx_with_house();
        let mut d = FloorDialog::new_floor(&q.project, q.floor, &q.defaults);
        if let FloorDialog::NewFloor { spec, .. } = &mut d {
            spec.move_roof_up = true;
        }
        assert!(
            !d.new_floor_spec().unwrap().move_roof_up,
            "no planes, no move"
        );
        q.refresh();
    }

    #[test]
    fn insert_new_floor_presets_below_and_names_itself() {
        let cx = cx_with_house();
        let d = FloorDialog::insert_floor(&cx.project, cx.floor, &cx.defaults);
        assert_eq!(d.title(), "Insert New Floor");
        let spec = d.new_floor_spec().unwrap();
        assert_eq!(spec.place, FloorPlacement::Below);
        assert!(
            spec.foundation.is_none(),
            "the foundation is built with Build New Floor"
        );
        let b = FloorDialog::new_floor(&cx.project, cx.floor, &cx.defaults);
        assert_eq!(b.title(), "Build New Floor");
    }

    #[test]
    fn the_foundation_defaults_dialog_changes_the_defaults_and_not_the_plan() {
        let mut cx = cx_with_house();
        let mut d = FloorDialog::foundation_defaults(&cx);
        assert_eq!(d.title(), "Foundation Defaults");
        if let FloorDialog::FoundationDefaults { form } = &mut d {
            form.spec.settings.auto_rebuild = true;
            form.spec.settings.foam_seal = true;
            form.spec.min_stem_height = 18.0;
            form.spec.stem_height = 60.0;
        }
        let floors = cx.project.floors.len();
        d.apply(&mut cx);
        assert_eq!(cx.project.floors.len(), floors, "no change to the model");
        assert!(cx.defaults.foundation.auto_rebuild && cx.defaults.foundation.foam_seal);
        assert_eq!(cx.defaults.foundation.min_height, 18.0);
        assert_eq!(cx.defaults.foundation_wall.height, 60.0);
        // Build Foundation starts from them.
        let spec = FoundationSpec::from_defaults(&cx.defaults);
        assert!(spec.settings.auto_rebuild);
        assert_eq!(spec.min_stem_height, 18.0);
    }

    #[test]
    fn build_foundation_makes_its_choices_the_defaults_and_reopens_on_them() {
        let mut cx = cx_with_house();
        let mut spec = FoundationSpec::from_defaults(&cx.defaults);
        spec.stem_height = 80.0;
        spec.settings.hang_platform = true;
        spec.settings.rebar.use_mesh = false;
        FloorDialog::foundation(spec).apply(&mut cx);
        assert!(cx.defaults.foundation.hang_platform);
        // The dialog on a plan with a foundation starts from what was built.
        cx.defaults.foundation = plan_core::foundation::FoundationSettings::default();
        let FloorDialog::BuildFoundation { form } = FloorDialog::build_foundation(&cx) else {
            panic!("the Build Foundation dialog");
        };
        assert_eq!(form.spec.stem_height, 80.0);
        assert!(form.spec.settings.hang_platform && !form.spec.settings.rebar.use_mesh);
        assert_eq!(form.spec.kind, FoundationType::WallsWithFootings);
    }

    #[test]
    fn both_foundation_panels_draw_for_every_type() {
        let cx = cx_with_house();
        let ctx = egui::Context::default();
        for kind in FoundationType::ALL {
            for panel in [
                super::super::foundation::FoundationPanel::Foundation,
                super::super::foundation::FoundationPanel::Options,
            ] {
                let mut d = FloorDialog::build_foundation(&cx);
                if let FloorDialog::BuildFoundation { form } = &mut d {
                    form.spec.kind = kind;
                    form.panel = panel;
                }
                for _ in 0..2 {
                    let mut out = Outcome::Cancel;
                    let _ = ctx.run(egui::RawInput::default(), |ctx| out = d.show(ctx));
                    assert_eq!(out, Outcome::Open, "{kind:?} {panel:?}");
                }
            }
        }
        // The Edit buttons open the Wall Type Definitions.
        let FloorDialog::BuildFoundation { mut form } = FloorDialog::build_foundation(&cx) else {
            panic!("the Build Foundation dialog");
        };
        assert!(form.open_edit());
        assert!(form.editing());
        let _ = ctx.run(egui::RawInput::default(), |ctx| form.child(ctx));
    }

    #[test]
    fn deleting_floor_zero_is_refused_in_the_dialog_command_while_auto_rebuild_is_on() {
        let mut cx = cx_with_house();
        let mut spec = FoundationSpec::from_defaults(&cx.defaults);
        spec.settings.auto_rebuild = true;
        FloorDialog::foundation(spec).apply(&mut cx);
        cx.floor = 0;
        assert!(!rooms_edit::delete_floor(&mut cx));
        assert!(
            cx.status.contains("Auto Rebuild Foundation"),
            "{}",
            cx.status
        );
        assert_eq!(cx.project.floors.len(), 2);
    }

    #[test]
    fn floor_defaults_keeps_the_moldings_and_the_fill_style_in_one_undo_step() {
        let mut cx = cx_with_house();
        let FloorDialog::Defaults(mut d) = FloorDialog::defaults_for_floor(&cx) else {
            panic!("floor defaults");
        };
        d.moldings_mut()
            .expect("this floor's moldings")
            .add_new(plan_core::moldings::square_profile());
        d.settings_mut().room_fill = Some(plan_core::extras::RoomFill {
            color: [1, 2, 3],
            pattern: "Hatch".into(),
            alpha: 1.0,
        });
        let before = cx.undo_label().map(str::to_string);
        FloorDialog::Defaults(d).apply(&mut cx);
        assert!(!crate::tools::molding::floor_molding_table(&cx).is_empty());
        assert_eq!(
            cx.floor().settings.room_fill.as_ref().unwrap().pattern,
            "Hatch"
        );
        // Rooms without a fill of their own draw with the floor's.
        cx.refresh();
        let room = cx.rooms[0].clone();
        let fill = rooms_edit::fill_style(&cx, &room);
        assert_eq!(fill.color, [1, 2, 3]);
        // One step: undo takes both back.
        cx.undo();
        assert_eq!(cx.undo_label().map(str::to_string), before);
        assert!(crate::tools::molding::floor_molding_table(&cx).is_empty());
        assert!(cx.floor().settings.room_fill.is_none());
    }
}
