//! Exterior Room Specification (R-106; manual pp. 449 and 450, 464): the
//! covering and surface material of the exterior walls of one structure on a
//! floor, and the default ceiling height and floor platform of the level
//! (the same values the Exterior Room's edge grips drag).
//!
//! Panels: General (the level's defaults and the Living Area the Exterior
//! Room's label reports), Wall Covering (the exterior face of every exterior
//! wall of the structure) and Materials (the Exterior Wall Surface).
//!
//! OK hands the specification back; `rooms_edit::apply_exterior_spec` writes
//! it to the walls and the floor as one undo step.

use super::{
    on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT, PV_INK,
    PV_WALL,
};
use crate::editor::rooms_edit::{self, ExteriorInit};
use crate::editor::EditorContext;
use eframe::egui::{self, Painter, Rect, Ui};
use plan_core::extras::MoldingKind;
use plan_core::living::ExteriorRoom;
use plan_core::rooms::{molding_def, molding_defs};
use plan_core::units::fmt_ft_in;

const TABS: &[Tab] = &[on("General"), on("Wall Covering"), on("Materials")];

pub struct ExteriorRoomDialog {
    frame: SpecDialog,
    form: ExteriorForm,
}

struct ExteriorForm {
    init: ExteriorInit,
    spec: ExteriorRoom,
    ceiling_height: f64,
    floor_thickness: f64,
    fields: Fields,
}

// The accessors are the dialog's model API (the tests drive it).
#[allow(dead_code)]
impl ExteriorRoomDialog {
    pub fn new(init: ExteriorInit) -> Self {
        let form = ExteriorForm {
            spec: init.spec.clone(),
            ceiling_height: init.ceiling_height,
            floor_thickness: init.floor_thickness,
            fields: Fields::default(),
            init,
        };
        Self {
            frame: SpecDialog::new("Exterior Room Specification", "exterior_room"),
            form,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn init(&self) -> &ExteriorInit {
        &self.form.init
    }

    pub fn spec(&self) -> &ExteriorRoom {
        &self.form.spec
    }

    pub fn spec_mut(&mut self) -> &mut ExteriorRoom {
        &mut self.form.spec
    }

    pub fn ceiling_height(&self) -> f64 {
        self.form.ceiling_height
    }

    pub fn set_ceiling_height(&mut self, h: f64) {
        self.form.ceiling_height = h;
    }

    pub fn floor_thickness(&self) -> f64 {
        self.form.floor_thickness
    }

    pub fn set_floor_thickness(&mut self, t: f64) {
        self.form.floor_thickness = t;
    }

    pub fn has_error(&self) -> bool {
        self.form.error().is_some()
    }
}

thread_local! {
    static OPEN: std::cell::RefCell<Option<ExteriorRoomDialog>> = const { std::cell::RefCell::new(None) };
}

/// Draws the Exterior Room Specification: opens it when the Select tool, the
/// Edit toolbar or Enter asked for it (`rooms_edit::request_exterior_dialog`)
/// and writes an accepted one as one undo step. Called once a frame with the
/// other build dialogs.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    if let Some(idx) = rooms_edit::take_exterior_dialog_request(cx) {
        if let Some(init) = rooms_edit::exterior_dialog_init(cx, idx) {
            OPEN.with(|o| *o.borrow_mut() = Some(ExteriorRoomDialog::new(init)));
        }
    }
    let Some(mut d) = OPEN.with(|o| o.borrow_mut().take()) else {
        return;
    };
    match d.show(ctx) {
        Outcome::Open => OPEN.with(|o| *o.borrow_mut() = Some(d)),
        Outcome::Cancel => {}
        Outcome::Ok => {
            rooms_edit::apply_exterior_spec(
                cx,
                d.init().anchor,
                d.spec(),
                d.ceiling_height(),
                d.floor_thickness(),
            );
        }
    }
}

/// A combo of material names with a "None" entry first.
fn material_combo(ui: &mut Ui, salt: &str, value: &mut String, options: &[&str]) {
    egui::ComboBox::from_id_salt(salt.to_string())
        .width(220.0)
        .selected_text(if value.is_empty() {
            "None".to_string()
        } else {
            value.clone()
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), "None");
            for o in options {
                ui.selectable_value(value, (*o).to_string(), *o);
            }
        });
}

fn molding_combo(ui: &mut Ui, salt: &str, kind: MoldingKind, value: &mut String) {
    egui::ComboBox::from_id_salt(salt.to_string())
        .width(220.0)
        .selected_text(if value.is_empty() {
            "None".to_string()
        } else {
            value.clone()
        })
        .show_ui(ui, |ui| {
            ui.selectable_value(value, String::new(), "None");
            for d in molding_defs(kind) {
                ui.selectable_value(value, d.name.to_string(), d.name);
            }
        });
    if let Some(d) = molding_def(value) {
        ui.weak(format!(
            "{} high, {} out",
            fmt_ft_in(d.height()),
            fmt_ft_in(d.projection())
        ));
    }
}

impl ExteriorForm {
    fn general(&mut self, ui: &mut Ui) {
        section(ui, "Exterior Room");
        let i = &self.init;
        egui::Grid::new("exterior_room_info")
            .striped(true)
            .show(ui, |ui| {
                for (k, v) in [
                    ("Floor", i.floor_name.clone()),
                    ("Rooms in the structure", i.rooms.to_string()),
                    ("Exterior walls", i.exterior_walls.to_string()),
                    ("Footprint", format!("{:.0} sq ft", i.footprint_sq_ft)),
                    (
                        "Living Area",
                        format!(
                            "{} sq ft",
                            plan_core::living::group_thousands(i.living_sq_ft as i64)
                        ),
                    ),
                    ("Floor Height", fmt_ft_in(i.floor_elevation)),
                ] {
                    ui.label(k);
                    ui.label(v);
                    ui.end_row();
                }
            });
        section(ui, "Default Heights of This Floor Level");
        self.fields.length_row(
            ui,
            "Default Ceiling Height",
            "ext_ceiling",
            &mut self.ceiling_height,
        );
        ui.add_enabled_ui(self.init.bottom_editable, |ui| {
            self.fields.length_row(
                ui,
                "Floor Platform Thickness",
                "ext_floor_t",
                &mut self.floor_thickness,
            );
        });
        ui.weak(if self.init.bottom_editable {
            "These are the values the Exterior Room's top and bottom edge grips drag; floors above move with them."
        } else {
            "The default floor height of this level cannot be changed. The ceiling height is the grip on the top edge."
        });
    }

    fn wall_covering(&mut self, ui: &mut Ui) {
        section(ui, "Exterior Face of the Walls");
        ui.weak("A covering put here goes on the outside face of every exterior wall of this structure.");
        let mats = self.init.wall_materials.clone();
        let c = &mut self.spec.covering;
        row(ui, "Wall Covering", |ui| {
            material_combo(ui, "ext_cov", &mut c.covering, &mats)
        });
        if !c.covering.is_empty() {
            self.fields.length_row(
                ui,
                "Covering Thickness",
                "ext_cov_t",
                &mut c.covering_thickness,
            );
        }
        row(ui, "Wainscot", |ui| {
            material_combo(ui, "ext_wain", &mut c.wainscot, &mats)
        });
        if !c.wainscot.is_empty() {
            self.fields
                .length_row(ui, "Wainscot Height", "ext_wain_h", &mut c.wainscot_height);
            self.fields.length_row(
                ui,
                "Wainscot Thickness",
                "ext_wain_t",
                &mut c.wainscot_thickness,
            );
        }
        row(ui, "Chair Rail", |ui| {
            molding_combo(ui, "ext_chair", MoldingKind::Chair, &mut c.chair_rail)
        });
        if !c.chair_rail.is_empty() {
            self.fields.length_row(
                ui,
                "Chair Rail Height",
                "ext_chair_h",
                &mut c.chair_rail_height,
            );
        }
        row(ui, "Base Molding", |ui| {
            molding_combo(ui, "ext_base", MoldingKind::Base, &mut c.base)
        });
        row(ui, "Crown Molding", |ui| {
            molding_combo(ui, "ext_crown", MoldingKind::Crown, &mut c.crown)
        });
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Exterior Wall Surface");
        let mats = self.init.wall_materials.clone();
        row(ui, "Material", |ui| {
            material_combo(ui, "ext_surface", &mut self.spec.surface_material, &mats)
        });
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut self.spec.surface_rgb);
        });
        ui.weak("Leave it on None to keep the material each wall has.");
    }
}

impl SpecPages for ExteriorForm {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.ceiling_height < 12.0 {
            return Some("The default ceiling height must be at least 1'".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS[tab].name {
            "General" => self.general(ui),
            "Wall Covering" => self.wall_covering(ui),
            "Materials" => self.materials(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        // A section of the wall: the surface material over the wall body.
        let r = rect.shrink(18.0);
        let wall = Rect::from_min_max(
            egui::Pos2::new(r.center().x - 14.0, r.min.y),
            egui::Pos2::new(r.center().x + 14.0, r.max.y),
        );
        p.rect_filled(wall, 0.0, PV_WALL);
        if !self.spec.surface_material.is_empty() || !self.spec.covering.covering.is_empty() {
            let [cr, cg, cb] = self.spec.surface_rgb;
            let skin = Rect::from_min_max(
                egui::Pos2::new(wall.max.x, wall.min.y),
                egui::Pos2::new(wall.max.x + 8.0, wall.max.y),
            );
            p.rect_filled(skin, 0.0, egui::Color32::from_rgb(cr, cg, cb));
        }
        p.rect_stroke(
            wall,
            0.0,
            egui::Stroke::new(1.0_f32, PV_INK),
            egui::StrokeKind::Inside,
        );
        p.text(
            egui::Pos2::new(r.center().x, r.min.y - 6.0),
            egui::Align2::CENTER_CENTER,
            "Exterior wall",
            egui::FontId::proportional(11.0),
            PV_INK,
        );
        p.text(
            egui::Pos2::new(wall.max.x + 14.0, r.center().y),
            egui::Align2::LEFT_CENTER,
            if self.spec.covering.covering.is_empty() {
                "outside".to_string()
            } else {
                self.spec.covering.covering.clone()
            },
            egui::FontId::proportional(10.0),
            PV_ACCENT,
        );
        p.text(
            egui::Pos2::new(wall.min.x - 6.0, r.center().y),
            egui::Align2::RIGHT_CENTER,
            "inside",
            egui::FontId::proportional(10.0),
            PV_FAINT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;

    fn init() -> ExteriorInit {
        ExteriorInit {
            floor: 0,
            floor_name: "1st Floor".into(),
            anchor: Point::new(10.0, 10.0),
            spec: ExteriorRoom::default(),
            rooms: 3,
            exterior_walls: 6,
            living_sq_ft: 1820.0,
            footprint_sq_ft: 2100.0,
            ceiling_height: 109.125,
            floor_elevation: 0.0,
            floor_thickness: 10.25,
            bottom_editable: false,
            wall_materials: plan_core::rooms::WALL_SURFACES.to_vec(),
        }
    }

    #[test]
    fn every_panel_draws_and_the_choices_are_kept() {
        let ctx = egui::Context::default();
        let mut d = ExteriorRoomDialog::new(init());
        d.spec_mut().covering.covering = plan_core::rooms::WALL_SURFACES[0].to_string();
        d.spec_mut().covering.wainscot = plan_core::rooms::WALL_SURFACES[0].to_string();
        d.spec_mut().covering.chair_rail = "Chair Rail - Colonial 3".into();
        d.spec_mut().surface_material = plan_core::rooms::WALL_SURFACES[0].to_string();
        for tab in 0..TABS.len() {
            let f = &mut d.form;
            let _ = ctx.run(egui::RawInput::default(), |c| {
                egui::CentralPanel::default().show(c, |ui| SpecPages::page(f, ui, tab));
            });
        }
        assert!(!d.has_error());
        assert!(!d.spec().is_blank());
        d.set_ceiling_height(6.0);
        assert!(d.has_error(), "a ceiling lower than a foot is refused");
    }
}
