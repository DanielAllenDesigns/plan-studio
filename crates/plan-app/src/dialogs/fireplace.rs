//! Fireplace Specification (CB-87): General, Hearth, Mantel, Chimney,
//! Materials, Label and Layer for a fireplace or chimney placed with the
//! Fireplace tools.
//!
//! The dialog edits clones of the placed symbol (size, angle, layer) and of
//! its [`Fireplace`] record; OK hands both back and
//! `editor::fireplace_view::apply` writes them as one undo step. Values that
//! do not fit together (a firebox wider than the body, a chimney larger than
//! the body) block OK with a message instead of being clamped under the
//! cursor; the apply step clamps whatever is left.

use super::{
    on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT, PV_FAINT,
    PV_INK, PV_WALL,
};
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Shape, Stroke, Ui};
use plan_core::fireplace::{
    CapKind, ChimneyTop, Fireplace, FireplaceKind, Frame, Fuel, FIREPLACE_LAYER,
};
use plan_core::units::fmt_ft_in;
use plan_core::PlacedSymbol;

const TABS: &[Tab] = &[
    on("General"),
    on("Hearth"),
    on("Mantel"),
    on("Chimney"),
    on("Materials"),
    on("Label"),
    on("Layer"),
];

/// Suggestions for the material boxes.
const SURFACES: [&str; 9] = [
    "Brick",
    "Stone",
    "Concrete",
    "Stucco",
    "Siding",
    "Wood",
    "Painted Trim",
    "Metal",
    "Drywall",
];

pub struct FireplaceDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    sym: PlacedSymbol,
    fp: Fireplace,
    layers: Vec<String>,
    fields: Fields,
    /// The Chimney tab's top height while it is a fixed one.
    fixed_top: f64,
}

// The accessors are the dialog's model API (the tests drive it); the UI
// edits the same fields directly.
#[allow(dead_code)]
impl FireplaceDialog {
    /// A dialog for `sym` and its specification `fp`; `layers` are the
    /// plan's layer names.
    pub fn new(sym: PlacedSymbol, fp: Fireplace, layers: Vec<String>) -> Self {
        let title = if fp.kind == FireplaceKind::ChimneyOnly {
            "Chimney Specification"
        } else {
            "Fireplace Specification"
        };
        let fixed_top = match fp.chimney.top {
            ChimneyTop::Height(h) => h,
            ChimneyTop::Auto => 168.0,
        };
        let mut layers = layers;
        if !layers.iter().any(|l| *l == sym.layer) {
            layers.push(sym.layer.clone());
        }
        if !layers.iter().any(|l| l == FIREPLACE_LAYER) {
            layers.push(FIREPLACE_LAYER.to_string());
        }
        Self {
            frame: SpecDialog::new(title, "fireplace"),
            form: Form {
                sym,
                fp,
                layers,
                fields: Fields::default(),
                fixed_top,
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn symbol(&self) -> &PlacedSymbol {
        &self.form.sym
    }

    pub fn symbol_mut(&mut self) -> &mut PlacedSymbol {
        &mut self.form.sym
    }

    pub fn fireplace(&self) -> &Fireplace {
        &self.form.fp
    }

    pub fn fireplace_mut(&mut self) -> &mut Fireplace {
        &mut self.form.fp
    }

    /// Changes the kind of fireplace: a chimney on its own takes the size
    /// of its chimney, a firebox kind takes the default size and chimney of
    /// that kind.
    pub fn set_kind(&mut self, kind: FireplaceKind) {
        self.form.set_kind(kind);
    }

    pub fn has_error(&self) -> bool {
        self.form.problem().is_some()
    }

    pub fn error_text(&self) -> Option<String> {
        self.form.problem()
    }

    /// The fixed top the Chimney tab remembers.
    pub fn set_fixed_top(&mut self, height: f64) {
        self.form.fixed_top = height;
        self.form.fp.chimney.top = ChimneyTop::Height(height);
    }

    pub fn use_auto_top(&mut self) {
        self.form.fp.chimney.top = ChimneyTop::Auto;
    }
}

impl Form {
    fn set_kind(&mut self, kind: FireplaceKind) {
        if kind == self.fp.kind {
            return;
        }
        let was_chimney = self.fp.kind == FireplaceKind::ChimneyOnly;
        let keep = (self.fp.id, self.fp.name.clone());
        let mut fresh = Fireplace::new(keep.0, kind);
        // A name the user typed stays; the kind's own name follows the kind.
        let old_default = Fireplace::new(0, self.fp.kind).name;
        if keep.1 != old_default {
            fresh.name = keep.1;
        }
        fresh.in_wall = self.fp.in_wall;
        fresh.elevation = self.fp.elevation;
        self.fp = fresh;
        if kind == FireplaceKind::ChimneyOnly {
            self.sym.catalog_id = plan_core::fireplace::CHIMNEY_CATALOG_ID.to_string();
            self.sym.width = self.fp.chimney.width;
            self.sym.depth = self.fp.chimney.depth;
            self.fp.in_wall = false;
        } else {
            self.sym.catalog_id = plan_core::fireplace::FIREPLACE_CATALOG_ID.to_string();
            if was_chimney {
                let (w, d) = Fireplace::default_size(kind);
                self.sym.width = w;
                self.sym.depth = d;
            }
        }
    }

    fn problem(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.fp.name.trim().is_empty() {
            return Some("The fireplace needs a name".into());
        }
        let (w, d) = (self.sym.width, self.sym.depth);
        if w < 12.0 || d < 6.0 {
            return Some("The body is smaller than 12 in by 6 in".into());
        }
        let fp = &self.fp;
        if fp.kind.has_firebox() {
            if fp.firebox.width < 6.0 || fp.firebox.height < 6.0 || fp.firebox.depth < 2.0 {
                return Some("The firebox opening is too small".into());
            }
            if fp.firebox.width > w - 8.0 {
                return Some("The firebox is wider than the body less two 4 in jambs".into());
            }
            if fp.firebox.depth > d {
                return Some("The firebox is deeper than the body".into());
            }
            if fp.chimney.enabled && (fp.chimney.width > w || fp.chimney.depth > d) {
                return Some("The chimney is larger than the body".into());
            }
        }
        if fp.chimney.enabled && fp.chimney.flue_width + 4.0 > fp.chimney.width.min(w) {
            return Some("The flue does not fit in the chimney".into());
        }
        if let ChimneyTop::Height(h) = fp.chimney.top {
            if h <= 12.0 {
                return Some("The chimney must end above the body".into());
            }
        }
        None
    }

    // ----- tabs -----

    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        row(ui, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut self.fp.name).desired_width(200.0));
        });
        row(ui, "Type", |ui| {
            let mut picked = None;
            egui::ComboBox::from_id_salt("fp_kind")
                .width(200.0)
                .selected_text(self.fp.kind.name())
                .show_ui(ui, |ui| {
                    for k in FireplaceKind::ALL {
                        if ui.selectable_label(k == self.fp.kind, k.name()).clicked() {
                            picked = Some(k);
                        }
                    }
                });
            if let Some(k) = picked {
                self.set_kind(k);
            }
        });
        if self.fp.kind.has_firebox() {
            row(ui, "Fuel", |ui| {
                egui::ComboBox::from_id_salt("fp_fuel")
                    .width(200.0)
                    .selected_text(self.fp.fuel.name())
                    .show_ui(ui, |ui| {
                        for f in Fuel::ALL {
                            ui.selectable_value(&mut self.fp.fuel, f, f.name());
                        }
                    });
            });
        }
        if self.fp.kind != FireplaceKind::ChimneyOnly {
            ui.checkbox(&mut self.fp.in_wall, "Built into the wall")
                .on_hover_text(
                    "The body replaces the stretch of wall it stands in, and the wall is cut there in 3D",
                );
        }

        section(ui, "Size");
        let f = &mut self.fields;
        f.length_row(ui, "Width", "fp_w", &mut self.sym.width);
        f.length_row(ui, "Depth", "fp_d", &mut self.sym.depth);
        f.length_row(ui, "Bottom Above Floor", "fp_elev", &mut self.fp.elevation);
        let mut to_ceiling = self.fp.height.is_none();
        if ui
            .checkbox(&mut to_ceiling, "Body Reaches the Ceiling")
            .changed()
        {
            self.fp.height = (!to_ceiling).then_some(96.0);
        }
        if let Some(h) = self.fp.height.as_mut() {
            f.length_row(ui, "Body Height", "fp_h", h);
        }

        if self.fp.kind.has_firebox() {
            section(ui, "Firebox");
            let b = &mut self.fp.firebox;
            f.length_row(ui, "Opening Width", "fp_fb_w", &mut b.width);
            f.length_row(ui, "Opening Height", "fp_fb_h", &mut b.height);
            f.length_row(ui, "Depth", "fp_fb_d", &mut b.depth);
            f.length_row(ui, "Opening Above Hearth", "fp_fb_r", &mut b.raise);
        }

        section(ui, "Position");
        f.degrees_row(ui, "Angle", "deg_fp_angle", &mut self.sym.angle);
        row(ui, "Back Center", |ui| {
            ui.label(format!(
                "{}, {}",
                fmt_ft_in(self.sym.position.x),
                fmt_ft_in(self.sym.position.y)
            ));
        });
    }

    fn hearth(&mut self, ui: &mut Ui) {
        if !self.fp.kind.has_firebox() {
            ui.weak("A chimney has no hearth.");
            return;
        }
        let area = self.fp.firebox.width * self.fp.firebox.height / 144.0;
        let h = &mut self.fp.hearth;
        section(ui, "Hearth");
        ui.checkbox(&mut h.enabled, "Build a Hearth");
        ui.add_enabled_ui(h.enabled, |ui| {
            let f = &mut self.fields;
            f.length_row(ui, "Projection in Front", "fp_h_proj", &mut h.projection);
            f.length_row(
                ui,
                "Past Each Side of Opening",
                "fp_h_side",
                &mut h.side_extension,
            );
            f.length_row(ui, "Height of Top Above Floor", "fp_h_top", &mut h.height);
            f.length_row(ui, "Thickness", "fp_h_thk", &mut h.thickness);
        });
        // IRC R1001.10: 16 in front and 8 in beside an opening under 6 sq ft;
        // 20 in and 12 in for a larger one.
        let (front, side) = if area < 6.0 {
            (16.0, 8.0)
        } else {
            (20.0, 12.0)
        };
        ui.add_space(4.0);
        if h.enabled && (h.projection + 0.01 < front || h.side_extension + 0.01 < side) {
            ui.colored_label(
                super::ERROR_RED,
                format!(
                    "The code asks for {} in front and {} beside an opening of {:.1} sq ft (IRC R1001.10)",
                    front, side, area
                ),
            );
        } else {
            ui.weak(format!(
                "{front} in in front and {side} in beside the opening meet IRC R1001.10 for {area:.1} sq ft."
            ));
        }
    }

    fn mantel(&mut self, ui: &mut Ui) {
        if !self.fp.kind.has_firebox() {
            ui.weak("A chimney has no mantel.");
            return;
        }
        let opening_top = self.fp.elevation
            + self.fp.hearth.height
            + self.fp.firebox.raise
            + self.fp.firebox.height;
        let m = &mut self.fp.mantel;
        section(ui, "Mantel");
        ui.checkbox(&mut m.enabled, "Build a Mantel");
        ui.add_enabled_ui(m.enabled, |ui| {
            let f = &mut self.fields;
            f.length_row(ui, "Top of Shelf Above Floor", "fp_m_h", &mut m.height);
            f.length_row(
                ui,
                "Shelf Width (0 = Opening + 2 ft)",
                "fp_m_w",
                &mut m.width,
            );
            f.length_row(ui, "Shelf Depth", "fp_m_d", &mut m.depth);
            f.length_row(ui, "Shelf Thickness", "fp_m_t", &mut m.thickness);
            f.length_row(ui, "Leg Width (0 = None)", "fp_m_leg", &mut m.leg_width);
        });
        ui.add_space(4.0);
        if m.enabled && m.height - m.thickness < opening_top + 12.0 {
            ui.colored_label(
                super::ERROR_RED,
                "A combustible mantel needs 12 in of clearance over the opening",
            );
        }
    }

    fn chimney(&mut self, ui: &mut Ui) {
        let chimney_only = self.fp.kind == FireplaceKind::ChimneyOnly;
        let c = &mut self.fp.chimney;
        section(ui, "Chimney");
        if chimney_only {
            c.enabled = true;
            ui.weak("A chimney on its own takes the width and depth of its body.");
        } else {
            ui.checkbox(&mut c.enabled, "Build a Chimney");
        }
        ui.add_enabled_ui(c.enabled, |ui| {
            let f = &mut self.fields;
            if !chimney_only {
                f.length_row(ui, "Width", "fp_c_w", &mut c.width);
                f.length_row(ui, "Depth", "fp_c_d", &mut c.depth);
            }
            section(ui, "Height");
            let mut auto = c.top == ChimneyTop::Auto;
            ui.horizontal(|ui| {
                if ui.radio_value(&mut auto, true, "3-2-10 Rule").changed() && auto {
                    c.top = ChimneyTop::Auto;
                }
                if ui.radio_value(&mut auto, false, "Fixed Height").changed() && !auto {
                    c.top = ChimneyTop::Height(self.fixed_top);
                }
            });
            match c.top {
                ChimneyTop::Auto => {
                    f.length_row(ui, "Above the Roof it Passes Through", "fp_c_roof", &mut c.above_roof);
                    f.length_row(ui, "Above Anything Within 10 ft", "fp_c_near", &mut c.above_nearby);
                }
                ChimneyTop::Height(ref mut h) => {
                    if f.length_row(ui, "Top Above the Floor", "fp_c_top", h) {
                        self.fixed_top = *h;
                    }
                }
            }
            section(ui, "Cap and Flue");
            row(ui, "Cap", |ui| {
                egui::ComboBox::from_id_salt("fp_cap")
                    .width(200.0)
                    .selected_text(c.cap.name())
                    .show_ui(ui, |ui| {
                        for k in CapKind::ALL {
                            ui.selectable_value(&mut c.cap, k, k.name());
                        }
                    });
            });
            ui.add_enabled_ui(c.cap != CapKind::None, |ui| {
                f.length_row(ui, "Cap Overhang", "fp_c_ov", &mut c.cap_overhang);
                f.length_row(ui, "Cap Thickness", "fp_c_ct", &mut c.cap_thickness);
            });
            f.length_row(ui, "Flue Width", "fp_c_fw", &mut c.flue_width);
            f.length_row(ui, "Flue Depth", "fp_c_fd", &mut c.flue_depth);
            f.length_row(ui, "Flue Above the Cap", "fp_c_fr", &mut c.flue_rise);
            section(ui, "Roof and Floors");
            ui.checkbox(&mut c.flashing, "Flashing at the Roof")
                .on_hover_text("Metal apron flashing where the chimney meets the roof surface");
            ui.checkbox(&mut c.chase_through_floors, "Chase Continues Up Through the Floors")
                .on_hover_text(
                    "The chimney shows on every floor above as a chase, cutting those floors' platforms",
                );
            ui.weak("The roof is cut around the chimney automatically.");
        });
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Surfaces");
        let kind = self.fp.kind;
        let m = &mut self.fp.materials;
        let mut rows: Vec<(&str, &str, &mut String)> = vec![("Body", "fp_mat_body", &mut m.body)];
        if kind.has_firebox() {
            rows.push(("Firebox Lining", "fp_mat_fb", &mut m.firebox));
            rows.push(("Hearth", "fp_mat_hearth", &mut m.hearth));
            rows.push(("Mantel", "fp_mat_mantel", &mut m.mantel));
        }
        rows.push(("Chimney", "fp_mat_chimney", &mut m.chimney));
        rows.push(("Cap", "fp_mat_cap", &mut m.cap));
        for (label, salt, value) in rows {
            row(ui, label, |ui| {
                ui.add(egui::TextEdit::singleline(value).desired_width(110.0));
                egui::ComboBox::from_id_salt(salt)
                    .width(110.0)
                    .selected_text("Choose")
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(value.is_empty(), "Default").clicked() {
                            value.clear();
                        }
                        for s in SURFACES {
                            if ui.selectable_label(value.as_str() == s, s).clicked() {
                                *value = s.to_string();
                            }
                        }
                    });
            });
        }
        ui.add_space(4.0);
        ui.weak("Empty uses the kind's own material: brick for masonry, drywall and siding for a prefab chase. The Material Painter can still recolor any part.");
    }

    fn label(&mut self, ui: &mut Ui) {
        section(ui, "Label");
        let hint = self.fp.name.clone();
        ui.checkbox(&mut self.fp.label.show, "Show the Label in the Plan");
        ui.add_enabled_ui(self.fp.label.show, |ui| {
            row(ui, "Label Text", |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.fp.label.text)
                        .desired_width(200.0)
                        .hint_text(hint),
                );
            });
        });
        ui.weak("An empty label shows the name.");
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            egui::ComboBox::from_id_salt("fp_layer")
                .width(220.0)
                .selected_text(self.sym.layer.clone())
                .show_ui(ui, |ui| {
                    for l in &self.layers {
                        ui.selectable_value(&mut self.sym.layer, l.clone(), l);
                    }
                });
        });
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        self.problem()
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS[tab].name {
            "General" => self.general(ui),
            "Hearth" => self.hearth(ui),
            "Mantel" => self.mantel(ui),
            "Chimney" => self.chimney(ui),
            "Materials" => self.materials(ui),
            "Label" => self.label(ui),
            "Layer" => self.layer(ui),
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
        // The fireplace in its own frame, facing down the page.
        let mut sym = self.sym.clone();
        sym.position = plan_core::Point::ZERO;
        sym.angle = 0.0;
        let fp = &self.fp;
        let frame = Frame::of(&sym);
        let polys = [
            plan_core::fireplace::body_poly(&sym),
            plan_core::fireplace::hearth_poly(fp, &sym),
            plan_core::fireplace::mantel_poly(fp, &sym),
            plan_core::fireplace::chimney_poly(fp, &sym),
            plan_core::fireplace::firebox_poly(fp, &sym),
        ];
        let (mut lo, mut hi) = (
            plan_core::Point::new(f64::MAX, f64::MAX),
            plan_core::Point::new(f64::MIN, f64::MIN),
        );
        for q in polys.iter().flatten() {
            lo = plan_core::Point::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = plan_core::Point::new(hi.x.max(q.x), hi.y.max(q.y));
        }
        if hi.x > lo.x && hi.y > lo.y {
            let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
            let s = ((f64::from(area.width()) - 8.0) / w).min((f64::from(area.height()) - 8.0) / h)
                as f32;
            let c = area.center();
            let (mx, my) = ((lo.x + hi.x) as f32 * 0.5, (lo.y + hi.y) as f32 * 0.5);
            let at = |q: &plan_core::Point| {
                Pos2::new(c.x + (q.x as f32 - mx) * s, c.y - (q.y as f32 - my) * s)
            };
            let ink = Stroke::new(1.3_f32, PV_INK);
            let body: Vec<Pos2> = polys[0].iter().map(at).collect();
            p.add(Shape::convex_polygon(body, PV_WALL, ink));
            if fp.kind.has_firebox() {
                let fb: Vec<Pos2> = polys[4].iter().map(at).collect();
                p.add(Shape::convex_polygon(
                    fb,
                    Color32::from_rgba_unmultiplied(40, 30, 25, 90),
                    ink,
                ));
                for poly in [&polys[1], &polys[2]] {
                    if poly.len() >= 3 {
                        let mut pts: Vec<Pos2> = poly.iter().map(at).collect();
                        pts.push(pts[0]);
                        p.extend(Shape::dashed_line(
                            &pts,
                            Stroke::new(1.0_f32, PV_ACCENT),
                            4.0,
                            3.0,
                        ));
                    }
                }
            }
            if fp.chimney.enabled {
                let ch: Vec<Pos2> = polys[3].iter().map(at).collect();
                let faint = Stroke::new(0.9_f32, PV_FAINT);
                p.add(Shape::closed_line(ch.clone(), faint));
                p.line_segment([ch[0], ch[2]], faint);
                p.line_segment([ch[1], ch[3]], faint);
            }
            // The front arrow.
            let front = at(&frame.at(0.0, sym.depth));
            p.line_segment([front, front + egui::vec2(0.0, 10.0)], ink);
        }
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.max.y - 18.0),
            Align2::CENTER_CENTER,
            self.fp.name.clone(),
            12.0,
        );
        p.text(
            Pos2::new(rect.center().x, rect.max.y - 5.0),
            Align2::CENTER_CENTER,
            format!(
                "{} x {}",
                fmt_ft_in(self.sym.width),
                fmt_ft_in(self.sym.depth)
            ),
            egui::FontId::proportional(10.0),
            PV_ACCENT,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Project;

    fn dialog(kind: FireplaceKind) -> FireplaceDialog {
        let mut p = Project::new("t");
        let id = p.add_fireplace(0, kind, plan_core::Point::new(100.0, 50.0), 0.0);
        let f = &p.floors[0];
        let sym = f.symbol(id).unwrap().clone();
        let fp = f.fireplace_of(&sym);
        FireplaceDialog::new(sym, fp, vec!["Walls, Normal".into()])
    }

    fn draw_tab(d: &mut FireplaceDialog, tab: usize) {
        let ctx = egui::Context::default();
        let form = &mut d.form;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                SpecPages::page(form, ui, tab);
            });
        });
    }

    #[test]
    fn every_tab_draws_for_every_kind() {
        for kind in FireplaceKind::ALL {
            let mut d = dialog(kind);
            d.fireplace_mut().chimney.top = ChimneyTop::Height(180.0);
            d.fireplace_mut().materials.body = "Stone".into();
            for tab in 0..TABS.len() {
                draw_tab(&mut d, tab);
            }
        }
        // The preview draws too.
        let ctx = egui::Context::default();
        let d = dialog(FireplaceKind::Masonry);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(220.0, 300.0), egui::Sense::hover());
                let p = ui.painter_at(rect);
                SpecPages::preview(&d.form, &p, rect);
            });
        });
    }

    #[test]
    fn the_tabs_are_chiefs_seven() {
        let names: Vec<&str> = TABS.iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            [
                "General",
                "Hearth",
                "Mantel",
                "Chimney",
                "Materials",
                "Label",
                "Layer"
            ]
        );
        assert!(TABS.iter().all(|t| t.enabled));
    }

    #[test]
    fn a_fresh_dialog_has_no_error_and_a_firebox_that_is_too_wide_blocks_ok() {
        let mut d = dialog(FireplaceKind::Masonry);
        assert!(!d.has_error());
        d.fireplace_mut().firebox.width = 70.0;
        assert!(d.error_text().unwrap().contains("wider than the body"));
        d.fireplace_mut().firebox.width = 36.0;
        d.symbol_mut().width = 8.0;
        assert!(d.has_error());
        d.symbol_mut().width = 72.0;
        d.fireplace_mut().chimney.width = 90.0;
        assert!(d.error_text().unwrap().contains("chimney is larger"));
        d.fireplace_mut().chimney.width = 48.0;
        d.fireplace_mut().name = "  ".into();
        assert!(d.has_error());
    }

    #[test]
    fn a_fixed_top_must_clear_the_body_and_switching_back_to_auto_works() {
        let mut d = dialog(FireplaceKind::Masonry);
        d.set_fixed_top(10.0);
        assert!(d.has_error());
        d.set_fixed_top(200.0);
        assert!(!d.has_error());
        assert_eq!(d.fireplace().chimney.top, ChimneyTop::Height(200.0));
        d.use_auto_top();
        assert_eq!(d.fireplace().chimney.top, ChimneyTop::Auto);
    }

    #[test]
    fn changing_the_kind_to_a_chimney_takes_the_chimney_size() {
        let mut d = dialog(FireplaceKind::Masonry);
        d.set_kind(FireplaceKind::ChimneyOnly);
        assert_eq!(d.fireplace().kind, FireplaceKind::ChimneyOnly);
        assert_eq!(
            d.symbol().catalog_id,
            plan_core::fireplace::CHIMNEY_CATALOG_ID
        );
        assert_eq!((d.symbol().width, d.symbol().depth), (48.0, 24.0));
        assert_eq!(d.fireplace().name, "Chimney");
        assert!(!d.fireplace().in_wall);
        d.set_kind(FireplaceKind::Prefab);
        assert_eq!(
            d.symbol().catalog_id,
            plan_core::fireplace::FIREPLACE_CATALOG_ID
        );
        assert_eq!((d.symbol().width, d.symbol().depth), (54.0, 30.0));
        assert_eq!(d.fireplace().name, "Prefab Fireplace");
        // A typed name survives a kind change.
        d.fireplace_mut().name = "Library Fireplace".into();
        d.set_kind(FireplaceKind::Masonry);
        assert_eq!(d.fireplace().name, "Library Fireplace");
    }

    #[test]
    fn the_layer_list_always_holds_the_symbols_layer() {
        let d = dialog(FireplaceKind::Masonry);
        assert!(d.form.layers.iter().any(|l| l == FIREPLACE_LAYER));
        assert!(d.form.layers.iter().any(|l| l == "Walls, Normal"));
    }

    #[test]
    fn the_title_follows_the_kind() {
        assert_eq!(
            dialog(FireplaceKind::Masonry).frame.geometry_key(),
            "Fireplace Specification"
        );
        assert_eq!(
            dialog(FireplaceKind::ChimneyOnly).frame.geometry_key(),
            "Chimney Specification"
        );
    }
}
