//! Room Specification (docs/chief-x18-dialogs.md, "Room Types and Room
//! Specification"; R-19..R-36, R-45).
//!
//! The dialog edits a draft [`RoomName`] plus the [`RoomExtras`] view of the
//! rest of the specification. OK hands both back to the app, which writes
//! them with `rooms_edit::apply_room_spec` as one undo step; conditioned, the
//! stem wall, moldings, fill and label options are stored in the room's
//! `RoomName`, the other extras are kept for the session.

use super::{
    dis_check, off, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab,
    PV_ACCENT, PV_FAINT, PV_INK, PV_WALL,
};
use crate::editor::rooms_edit::{FillPattern, RoomExtras};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::defaults::RoomTypeDef;
use plan_core::extras::AreaKind;
use plan_core::geometry::Point;
use plan_core::units::fmt_ft_in;
use plan_core::RoomName;

const ROOM_TABS: &[Tab] = &[
    on("General"),
    on("Structure"),
    off("Deck"),
    on("Moldings"),
    on("Wall Covering"),
    on("Fill Style"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Object Information"),
    on("Schedule"),
];

/// Everything the dialog needs to know about the room it edits.
#[derive(Clone, Debug)]
pub struct RoomInit {
    pub room_index: usize,
    pub name: RoomName,
    pub extras: RoomExtras,
    pub types: Vec<RoomTypeDef>,
    /// The outline drawn in the preview (interior surfaces when known).
    pub polygon: Vec<Point>,
    pub interior_dims: String,
    pub interior_area_sq_ft: f64,
    pub standard_area_sq_ft: f64,
    pub perimeter_in: f64,
    pub floor_elevation: f64,
    pub floor_ceiling_height: f64,
    /// The generated name ("Room 1") a room has before it is named.
    pub default_name: String,
    pub total_living_sq_ft: f64,
    pub floor_name: String,
}

pub struct RoomDialog {
    frame: SpecDialog,
    form: RoomForm,
}

struct RoomForm {
    init: RoomInit,
    name: RoomName,
    extras: RoomExtras,
    fields: Fields,
}

// The setters below are the dialog's model API (the tests drive it); the UI
// edits the same fields directly.
#[allow(dead_code)]
impl RoomDialog {
    pub fn new(init: RoomInit) -> Self {
        let form = RoomForm {
            name: init.name.clone(),
            extras: init.extras.clone(),
            fields: Fields::default(),
            init,
        };
        Self {
            frame: SpecDialog::new("Room Specification", "room"),
            form,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn room_index(&self) -> usize {
        self.form.init.room_index
    }

    pub fn room_name(&self) -> &RoomName {
        &self.form.name
    }

    pub fn extras(&self) -> &RoomExtras {
        &self.form.extras
    }

    pub fn extras_mut(&mut self) -> &mut RoomExtras {
        &mut self.form.extras
    }

    pub fn set_name(&mut self, name: &str) {
        self.form.name.name = name.to_string();
    }

    /// Changing the Room Type renames a room that still has its type's name
    /// (or its generated one); a hand-typed name stays (R-21).
    pub fn set_room_type(&mut self, room_type: &str) {
        self.form.set_room_type(room_type);
    }

    pub fn set_living(&mut self, include: Option<bool>) {
        self.form.name.include_in_living_area = include;
    }

    pub fn has_error(&self) -> bool {
        self.form.error().is_some()
    }
}

impl RoomForm {
    fn type_def(&self) -> Option<&RoomTypeDef> {
        self.init
            .types
            .iter()
            .find(|t| t.name == self.name.room_type)
    }

    fn set_room_type(&mut self, room_type: &str) {
        let old = self.name.room_type.clone();
        let follows = {
            let n = self.name.name.trim();
            n.is_empty() || n == old || n == self.init.default_name
        };
        self.name.room_type = room_type.to_string();
        if follows {
            self.name.name = room_type.to_string();
        }
    }

    /// "Use Default (Included)" text for the living-area radio.
    fn living_default(&self) -> &'static str {
        match self.type_def() {
            Some(t) if !t.include_in_living_area => "Use Default (Excluded)",
            _ => "Use Default (Included)",
        }
    }

    fn conditioned_default(&self) -> &'static str {
        match self.type_def() {
            Some(t) if !t.conditioned => "Use Default (Unconditioned)",
            _ => "Use Default (Conditioned)",
        }
    }

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Room Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.name.name).desired_width(200.0));
        });
        row(ui, "Room Type", |ui| {
            let mut picked: Option<String> = None;
            egui::ComboBox::from_id_salt("room_type")
                .width(200.0)
                .selected_text(self.name.room_type.clone())
                .show_ui(ui, |ui| {
                    for t in &self.init.types {
                        let on = t.name == self.name.room_type;
                        if ui.selectable_label(on, &t.name).clicked() {
                            picked = Some(t.name.clone());
                        }
                    }
                });
            if let Some(t) = picked {
                self.set_room_type(&t);
            }
        });
        row(ui, "Function", |ui| {
            let f = self.type_def().map_or("Standard", |t| t.function.as_str());
            ui.label(f);
        });

        section(ui, "Living Area");
        let default_text = self.living_default();
        ui.radio_value(
            &mut self.name.include_in_living_area,
            Some(true),
            "Include in Total Living Area Calculation",
        );
        ui.radio_value(
            &mut self.name.include_in_living_area,
            Some(false),
            "Exclude from Total Living Area Calculation",
        );
        ui.radio_value(&mut self.name.include_in_living_area, None, default_text);

        section(ui, "Conditioned Room");
        let default_text = self.conditioned_default();
        ui.radio_value(&mut self.extras.conditioned, Some(true), "Conditioned");
        ui.radio_value(&mut self.extras.conditioned, Some(false), "Unconditioned");
        ui.radio_value(&mut self.extras.conditioned, None, default_text);
    }

    fn structure(&mut self, ui: &mut Ui) {
        let elev = self.init.floor_elevation;
        let ceil_default = self.init.floor_ceiling_height;

        section(ui, "Floor Height");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.extras.floor_height_absolute, true, "Absolute");
            ui.radio_value(&mut self.extras.floor_height_absolute, false, "Relative");
        });
        let base = if self.extras.floor_height_absolute {
            elev
        } else {
            0.0
        };
        let mut v = base + self.name.floor_height_offset;
        if self
            .fields
            .length_row(ui, "Floor Height", "floor_h", &mut v)
        {
            self.name.floor_height_offset = v - base;
        }

        section(ui, "Ceiling Height");
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.extras.ceiling_height_absolute, true, "Absolute");
            ui.radio_value(&mut self.extras.ceiling_height_absolute, false, "Relative");
        });
        let cbase = if self.extras.ceiling_height_absolute {
            elev + self.name.floor_height_offset
        } else {
            0.0
        };
        let mut c = cbase + self.name.ceiling_height.unwrap_or(ceil_default);
        if self
            .fields
            .length_row(ui, "Ceiling Height", "ceil_h", &mut c)
        {
            self.name.ceiling_height = Some(c - cbase);
        }
        if self.name.ceiling_height.is_some()
            && ui.small_button("Use the floor's ceiling height").clicked()
        {
            self.name.ceiling_height = None;
        }

        let mut rough = self.name.rough_ceiling.is_some();
        if ui.checkbox(&mut rough, "Rough Ceiling").changed() {
            self.name.rough_ceiling = rough.then_some(ceil_default);
        }
        if let Some(r) = self.name.rough_ceiling.as_mut() {
            self.fields
                .length_row(ui, "Rough Ceiling Height", "rough_h", r);
        }

        section(ui, "Finish");
        self.fields.length_row(
            ui,
            "Floor Finish Thickness",
            "floor_fin",
            &mut self.extras.floor_finish_thickness,
        );
        self.fields.length_row(
            ui,
            "Ceiling Finish Thickness",
            "ceil_fin",
            &mut self.extras.ceiling_finish_thickness,
        );

        section(ui, "Platforms");
        ui.checkbox(&mut self.name.has_floor, "Floor Under This Room");
        ui.checkbox(&mut self.name.has_ceiling, "Ceiling Over This Room");
        ui.checkbox(&mut self.extras.roof_over, "Roof Over This Room")
            .on_hover_text(super::SESSION_NOTE);

        section(ui, "Stem Wall");
        ui.checkbox(&mut self.extras.stem_wall, "Stem Wall");
        if self.extras.stem_wall {
            self.fields.length_row(
                ui,
                "Stem Wall Height",
                "stem_h",
                &mut self.extras.stem_wall_height,
            );
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add_enabled(false, egui::Button::new("Floor Structure Define\u{2026}"));
            ui.add_enabled(false, egui::Button::new("Ceiling Structure Define\u{2026}"));
        });
    }

    fn moldings(&mut self, ui: &mut Ui) {
        section(ui, "Profiles");
        ui.weak("No molding profiles yet. Base and crown names are set on the Materials tab.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for b in ["Add New\u{2026}", "Edit\u{2026}", "Delete"] {
                ui.add_enabled(false, egui::Button::new(b));
            }
        });
    }

    fn wall_covering(&mut self, ui: &mut Ui) {
        section(ui, "Wall Covering");
        row(ui, "Interior Wall Covering", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.extras.wall_covering).desired_width(200.0))
                .on_hover_text(super::SESSION_NOTE);
        });
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        section(ui, "Fill Style");
        row(ui, "Pattern", |ui| {
            egui::ComboBox::from_id_salt("room_fill")
                .selected_text(self.extras.fill.pattern.name())
                .show_ui(ui, |ui| {
                    for p in FillPattern::ALL {
                        ui.selectable_value(&mut self.extras.fill.pattern, p, p.name());
                    }
                });
        });
        row(ui, "Color", |ui| {
            ui.color_edit_button_srgb(&mut self.extras.fill.color);
        });
        row(ui, "Opacity", |ui| {
            ui.add(egui::Slider::new(&mut self.extras.fill.alpha, 0.1..=1.0));
        });
        ui.weak("Drawn in the plan view only.");
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Finishes");
        let mut floor = self.name.floor_finish.clone().unwrap_or_default();
        row(ui, "Floor Finish", |ui| {
            ui.add(egui::TextEdit::singleline(&mut floor).desired_width(200.0));
        });
        self.name.floor_finish = (!floor.trim().is_empty()).then_some(floor);
        let mut ceil = self.name.ceiling_finish.clone().unwrap_or_default();
        row(ui, "Ceiling Finish", |ui| {
            ui.add(egui::TextEdit::singleline(&mut ceil).desired_width(200.0));
        });
        self.name.ceiling_finish = (!ceil.trim().is_empty()).then_some(ceil);
        section(ui, "Moldings");
        row(ui, "Base", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.extras.base_molding).desired_width(200.0));
        });
        row(ui, "Crown", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.extras.crown_molding).desired_width(200.0));
        });
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Display in All Views");
        let l = &mut self.extras.label;
        ui.checkbox(&mut l.show_name, "Room Name");
        ui.checkbox(&mut l.show_dimensions, "Interior Dimensions");
        ui.checkbox(&mut l.show_area, "Area");
        ui.add_enabled_ui(l.show_area, |ui| {
            ui.radio_value(&mut l.area_kind, AreaKind::Interior, "Interior Area");
            ui.radio_value(&mut l.area_kind, AreaKind::Standard, "Standard Area");
            ui.radio_value(&mut l.area_kind, AreaKind::Centerline, "Centerline Area");
        });
        ui.weak("With everything unchecked the room shows no label.");
        section(ui, "Appearance");
        row(ui, "Text Style", |ui| {
            ui.add_enabled(false, egui::Button::new("Use Layer Text Style"));
        });
    }

    fn components(&mut self, ui: &mut Ui) {
        section(ui, "Floor and Ceiling Finish Layers");
        let none = "(none)";
        let rows = [
            (
                "Floor finish",
                self.name
                    .floor_finish
                    .clone()
                    .unwrap_or_else(|| none.into()),
                self.extras.floor_finish_thickness,
            ),
            (
                "Ceiling finish",
                self.name
                    .ceiling_finish
                    .clone()
                    .unwrap_or_else(|| none.into()),
                self.extras.ceiling_finish_thickness,
            ),
        ];
        egui::Grid::new("room_components")
            .striped(true)
            .show(ui, |ui| {
                ui.strong("Component");
                ui.strong("Material");
                ui.strong("Thickness");
                ui.end_row();
                for (c, m, t) in rows {
                    ui.label(c);
                    ui.label(m);
                    ui.label(fmt_ft_in(t));
                    ui.end_row();
                }
                for (c, m) in [
                    ("Base molding", &self.extras.base_molding),
                    ("Crown molding", &self.extras.crown_molding),
                ] {
                    ui.label(c);
                    ui.label(if m.is_empty() { none } else { m.as_str() });
                    ui.label("");
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        ui.weak(format!(
            "Values are calculated based on the room displayed in the preview (interior area = {} sq ft).",
            self.init.interior_area_sq_ft.round()
        ));
    }

    fn object_information(&mut self, ui: &mut Ui) {
        section(ui, "Room");
        let i = &self.init;
        let rows = [
            ("Floor", i.floor_name.clone()),
            ("Interior dimensions", i.interior_dims.clone()),
            (
                "Interior area",
                format!("{:.1} sq ft", i.interior_area_sq_ft),
            ),
            (
                "Standard area",
                format!("{:.1} sq ft", i.standard_area_sq_ft),
            ),
            ("Perimeter", fmt_ft_in(i.perimeter_in)),
            (
                "Total living area (all floors)",
                format!("{:.0} sq ft", i.total_living_sq_ft),
            ),
        ];
        egui::Grid::new("room_info").striped(true).show(ui, |ui| {
            for (k, v) in rows {
                ui.label(k);
                ui.label(v);
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        dis_check(ui, "Locked", false);
    }

    fn schedule(&mut self, ui: &mut Ui) {
        section(ui, "Room Finish Schedule Row");
        let ceil = self
            .name
            .ceiling_height
            .unwrap_or(self.init.floor_ceiling_height);
        egui::Grid::new("room_schedule_row")
            .striped(true)
            .show(ui, |ui| {
                for h in ["Name", "Type", "Area", "Ceiling", "Floor", "Ceiling finish"] {
                    ui.strong(h);
                }
                ui.end_row();
                ui.label(&self.name.name);
                ui.label(&self.name.room_type);
                ui.label(format!("{:.0} sq ft", self.init.interior_area_sq_ft));
                ui.label(fmt_ft_in(ceil));
                ui.label(self.name.floor_finish.clone().unwrap_or_default());
                ui.label(self.name.ceiling_finish.clone().unwrap_or_default());
                ui.end_row();
            });
    }
}

impl SpecPages for RoomForm {
    fn tabs(&self) -> &'static [Tab] {
        ROOM_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.name.name.trim().is_empty() {
            return Some("The room needs a name".into());
        }
        if self.name.ceiling_height.is_some_and(|c| c <= 0.0) {
            return Some("Ceiling height must be greater than zero".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match ROOM_TABS[tab].name {
            "General" => self.general(ui),
            "Structure" => self.structure(ui),
            "Moldings" => self.moldings(ui),
            "Wall Covering" => self.wall_covering(ui),
            "Fill Style" => self.fill_style(ui),
            "Materials" => self.materials(ui),
            "Label" => self.label(ui),
            "Components" => self.components(ui),
            "Object Information" => self.object_information(ui),
            "Schedule" => self.schedule(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            rect.min + egui::vec2(0.0, 6.0),
            Align2::LEFT_CENTER,
            "Plan view",
            11.0,
        );
        let area = Rect::from_min_max(
            rect.min + egui::vec2(0.0, 16.0),
            rect.max - egui::vec2(0.0, 28.0),
        );
        let poly = &self.init.polygon;
        if poly.len() >= 3 {
            let (mut lo, mut hi) = (poly[0], poly[0]);
            for q in poly {
                lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
                hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
            }
            let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
            let s = ((area.width() as f64 - 8.0) / w).min((area.height() as f64 - 8.0) / h) as f32;
            let c = area.center();
            let (mx, my) = ((lo.x + hi.x) as f32 * 0.5, (lo.y + hi.y) as f32 * 0.5);
            let pts: Vec<Pos2> = poly
                .iter()
                .map(|q| Pos2::new(c.x + (q.x as f32 - mx) * s, c.y - (q.y as f32 - my) * s))
                .collect();
            let [r, g, b] = self.extras.fill.color;
            let fill = if self.extras.fill.pattern == FillPattern::None {
                PV_WALL
            } else {
                Color32::from_rgb(r, g, b)
            };
            p.add(Shape::convex_polygon(
                pts.clone(),
                fill,
                Stroke::new(1.5_f32, PV_INK),
            ));
            p.add(Shape::closed_line(pts, Stroke::new(1.5_f32, PV_INK)));
        }
        p.rect_stroke(
            area,
            0.0,
            Stroke::new(0.8_f32, PV_FAINT),
            egui::StrokeKind::Inside,
        );
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.max.y - 18.0),
            Align2::CENTER_CENTER,
            self.name.name.clone(),
            12.0,
        );
        p.text(
            Pos2::new(rect.center().x, rect.max.y - 5.0),
            Align2::CENTER_CENTER,
            format!(
                "{}  {:.0} sq ft",
                self.init.interior_dims, self.init.interior_area_sq_ft
            ),
            egui::FontId::proportional(10.0),
            PV_ACCENT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::PlanDefaults;

    fn dialog() -> RoomDialog {
        let d = PlanDefaults::chief_x18_daniel();
        RoomDialog::new(RoomInit {
            room_index: 0,
            name: RoomName::new(Point::new(10.0, 10.0), "Room 1", "Standard"),
            extras: RoomExtras::from_defaults(&d),
            types: d.rooms.room_types.clone(),
            polygon: vec![
                Point::new(0.0, 0.0),
                Point::new(100.0, 0.0),
                Point::new(100.0, 100.0),
                Point::new(0.0, 100.0),
            ],
            interior_dims: "8'-4\" x 8'-4\"".into(),
            interior_area_sq_ft: 69.4,
            standard_area_sq_ft: 80.0,
            perimeter_in: 400.0,
            floor_elevation: 0.0,
            floor_ceiling_height: 109.125,
            default_name: "Room 1".into(),
            total_living_sq_ft: 0.0,
            floor_name: "1st Floor".into(),
        })
    }

    #[test]
    fn changing_type_renames_a_default_named_room() {
        let mut d = dialog();
        d.set_room_type("Bedroom");
        assert_eq!(d.room_name().name, "Bedroom");
        assert_eq!(d.room_name().room_type, "Bedroom");
        // A hand-typed name survives a type change.
        d.set_name("Guest Suite");
        d.set_room_type("Bath");
        assert_eq!(d.room_name().name, "Guest Suite");
        assert_eq!(d.room_name().room_type, "Bath");
    }

    #[test]
    fn empty_name_blocks_ok() {
        let mut d = dialog();
        assert!(!d.has_error());
        d.set_name("  ");
        assert!(d.has_error());
    }

    #[test]
    fn living_area_default_text_follows_the_type() {
        let mut d = dialog();
        assert_eq!(d.form.living_default(), "Use Default (Included)");
        d.set_room_type("Garage");
        assert_eq!(d.form.living_default(), "Use Default (Excluded)");
    }
}
