//! The Build Roof dialog (RF-1, RF-2), the Roof Plane Specification (RF-36,
//! with its holes list and Build Roof edge fields) and the Dormer
//! Specification (RF-48), on the shared dialog frame.
//!
//! Both edit a cloned draft; OK hands it back through `draft()`.

#![allow(dead_code)]

use super::{
    dis_check, fmt_short, on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT,
    PV_INK,
};
use crate::editor::roof_view::{pitch_label, RoofPlaneRecord, RoofSettings, ROOF_MATERIALS};
use eframe::egui::{self, Align2, FontId, Painter, Pos2, Rect, Stroke, Ui};
use plan_roof::{DormerKind, DormerSpec};

const MIN_PITCH: f64 = 0.5;
const MAX_PITCH: f64 = 24.0;

fn pitch_row(ui: &mut Ui, label: &str, pitch: &mut f64) {
    row(ui, label, |ui| {
        ui.add(
            egui::DragValue::new(pitch)
                .range(MIN_PITCH..=MAX_PITCH)
                .speed(0.1)
                .max_decimals(2)
                .suffix(" : 12"),
        );
    });
}

fn material_combo(ui: &mut Ui, salt: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for m in ROOF_MATERIALS {
                ui.selectable_value(value, m.to_string(), m);
            }
        });
}

// ===================================================================
// Build Roof
// ===================================================================

const BUILD_TABS: &[Tab] = &[on("Roof"), on("Options"), on("Materials")];

struct BuildPages {
    s: RoofSettings,
    fields: Fields,
    /// Which floor the roof goes on, for the Roof page.
    note: String,
}

impl SpecPages for BuildPages {
    fn tabs(&self) -> &'static [Tab] {
        BUILD_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.s.pitch < MIN_PITCH || self.s.pitch > MAX_PITCH {
            Some("Pitch must be between 0.5 and 24 in 12".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => {
                section(ui, "Roof");
                ui.checkbox(&mut self.s.build_planes, "Build Roof Planes");
                ui.checkbox(&mut self.s.auto_rebuild, "Auto Rebuild Roofs")
                    .on_hover_text("Rebuild the automatic planes when the walls change");
                ui.checkbox(&mut self.s.ignore_top_floor, "Ignore Top Floor")
                    .on_hover_text("Build the roof over the floor below the top one");
                ui.checkbox(&mut self.s.build_ceiling_planes, "Build Ceiling Planes")
                    .on_hover_text("Stored; vaulted ceiling planes are not generated yet");
                section(ui, "Defaults for walls without their own roof settings");
                pitch_row(ui, "Pitch", &mut self.s.pitch);
                self.fields
                    .length_row(ui, "Overhang", "overhang", &mut self.s.overhang);
                self.fields.length_row(
                    ui,
                    "Raise Roof Off Plate",
                    "raise",
                    &mut self.s.raise_off_plate,
                );
                ui.add_space(6.0);
                ui.weak(&self.note);
            }
            1 => {
                section(ui, "Framing");
                let mut v = self.s.build_framing;
                if ui
                    .checkbox(&mut v, "Build Framing")
                    .on_hover_text("Stored; roof framing is not generated yet")
                    .changed()
                {
                    self.s.build_framing = v;
                }
                dis_check(ui, "Rafters", true);
                dis_check(ui, "Trusses", false);
                ui.weak("Roof framing is a placeholder until plan-framing exists.");
            }
            _ => {
                section(ui, "Roofing");
                row(ui, "Material", |ui| {
                    material_combo(ui, "build_roof_material", &mut self.s.material);
                });
            }
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // A hip roof from above: outline, ridge and four hips.
        let r = Rect::from_center_size(area.center(), area.size() * egui::vec2(0.8, 0.5));
        let ink = Stroke::new(1.5_f32, PV_INK);
        p.rect_stroke(r, 0.0, ink, egui::StrokeKind::Inside);
        let inset = r.height() * 0.5;
        let (l, rr) = (
            Pos2::new(r.min.x + inset, r.center().y),
            Pos2::new(r.max.x - inset, r.center().y),
        );
        p.line_segment([l, rr], ink);
        for (c, e) in [
            (l, r.left_top()),
            (l, r.left_bottom()),
            (rr, r.right_top()),
            (rr, r.right_bottom()),
        ] {
            p.line_segment([c, e], Stroke::new(1.0_f32, PV_ACCENT));
        }
        p.text(
            Pos2::new(area.center().x, r.max.y + 18.0),
            Align2::CENTER_CENTER,
            pitch_label(self.s.pitch),
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

/// Build Roof dialog (one click on the tool opens it).
pub struct BuildRoofDialog {
    frame: SpecDialog,
    pages: BuildPages,
}

impl BuildRoofDialog {
    /// `note` tells which floor the roof is built over.
    pub fn new(settings: RoofSettings, note: impl Into<String>) -> Self {
        Self {
            frame: SpecDialog::new("Build Roof", "build_roof"),
            pages: BuildPages {
                s: settings,
                fields: Fields::default(),
                note: note.into(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    pub fn settings(&self) -> &RoofSettings {
        &self.pages.s
    }
}

// ===================================================================
// Roof Plane Specification
// ===================================================================

const PLANE_TABS: &[Tab] = &[
    on("General"),
    on("Holes"),
    on("Build Roof Edge"),
    on("Options"),
    on("Materials"),
    on("Layer"),
    on("Label"),
];

struct PlanePages {
    draft: RoofPlaneRecord,
    layers: Vec<String>,
    fields: Fields,
}

impl PlanePages {
    /// The holes and skylights of the plane: sizes, skylight construction,
    /// and a Delete button each.
    fn holes_page(&mut self, ui: &mut Ui) {
        section(ui, "Holes and Skylights");
        if self.draft.holes.is_empty() {
            ui.weak("This plane has no holes. Use the Roof Hole and Skylight tools.");
            return;
        }
        let mut remove = None;
        for (i, h) in self.draft.holes.iter_mut().enumerate() {
            let (w, l) = h.size();
            ui.horizontal(|ui| {
                ui.strong(if h.is_skylight() { "Skylight" } else { "Hole" });
                ui.label(format!("{} x {}", fmt_short(w), fmt_short(l)));
                if ui.button("Delete").clicked() {
                    remove = Some(i);
                }
            });
            if let Some(spec) = &mut h.skylight {
                for (label, value, max) in [
                    ("Curb Height", &mut spec.curb_height, 48.0),
                    ("Glass Thickness", &mut spec.glass_thickness, 6.0),
                    ("Frame Width", &mut spec.frame_width, 12.0),
                ] {
                    row(ui, label, |ui| {
                        ui.add(
                            egui::DragValue::new(value)
                                .range(0.25..=max)
                                .speed(0.1)
                                .suffix(" in"),
                        );
                    });
                }
            }
            ui.add_space(4.0);
        }
        if let Some(i) = remove {
            self.draft.holes.remove(i);
        }
    }

    /// The Build Roof overrides of the edge this plane rises from.
    fn edge_page(&mut self, ui: &mut Ui) {
        section(ui, "Edge (used by Build Roof)");
        if self.draft.source.is_none() {
            ui.weak("Only planes made by Build Roof rise from a wall edge.");
            return;
        }
        let mut pitch_on = self.draft.edge.pitch.is_some();
        let mut pitch = self.draft.edge.pitch.unwrap_or(self.draft.pitch);
        ui.horizontal(|ui| {
            ui.checkbox(&mut pitch_on, "Pitch");
            pitch_row(ui, "", &mut pitch);
        });
        self.draft.edge.pitch = pitch_on.then_some(pitch);
        let mut over_on = self.draft.edge.overhang.is_some();
        let mut over = self.draft.edge.overhang.unwrap_or(self.draft.overhang);
        ui.checkbox(&mut over_on, "Overhang");
        if over_on {
            self.fields
                .length_row(ui, "Overhang from wall face", "edge_overhang", &mut over);
        }
        self.draft.edge.overhang = over_on.then_some(over);
        ui.checkbox(
            &mut self.draft.edge.gable,
            "Gable end (no plane rises from this edge)",
        )
        .on_hover_text("Rebuilds the roof; the plane disappears");
        ui.add_space(6.0);
        ui.weak("OK rebuilds the automatic roof with these edge settings.");
    }
}

impl SpecPages for PlanePages {
    fn tabs(&self) -> &'static [Tab] {
        PLANE_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.draft.pitch < MIN_PITCH || self.draft.pitch > MAX_PITCH {
            Some("Pitch must be between 0.5 and 24 in 12".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => {
                section(ui, "General");
                let mut pitch = self.draft.pitch;
                pitch_row(ui, "Pitch", &mut pitch);
                if (pitch - self.draft.pitch).abs() > 1e-9 {
                    // Heights follow the new pitch right away.
                    self.draft.set_pitch(pitch);
                }
                let mut h = self.draft.baseline_height();
                if self
                    .fields
                    .length_row(ui, "Baseline Height", "baseline", &mut h)
                {
                    self.draft.set_baseline_height(h);
                }
                row(ui, "Overhang", |ui| {
                    ui.add_enabled(false, egui::Label::new(fmt_short(self.draft.overhang)));
                });
                row(ui, "Surface Area", |ui| {
                    ui.label(format!("{:.1} sq ft", self.draft.area() / 144.0));
                });
                row(ui, "Origin", |ui| {
                    ui.label(if self.draft.auto {
                        "Automatic (rebuilt with the walls)"
                    } else {
                        "Manual (kept when the roof is rebuilt)"
                    });
                });
                section(ui, "Vertices");
                egui::Grid::new("roof_vertices")
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("#");
                        ui.strong("X");
                        ui.strong("Y");
                        ui.strong("Elevation");
                        ui.end_row();
                        for (i, v) in self.draft.polygon3d.iter().enumerate() {
                            ui.label(format!("{}", i + 1));
                            ui.label(fmt_short(v[0]));
                            ui.label(fmt_short(-v[2]));
                            ui.label(fmt_short(v[1]));
                            ui.end_row();
                        }
                    });
            }
            1 => self.holes_page(ui),
            2 => self.edge_page(ui),
            3 => {
                section(ui, "Eaves and Ridge");
                ui.checkbox(&mut self.draft.ridge_caps, "Include Ridge Caps")
                    .on_hover_text("Stored; ridge caps are not modeled yet");
                ui.checkbox(&mut self.draft.gutters, "Include Gutter")
                    .on_hover_text("Stored; gutters are not modeled yet");
            }
            4 => {
                section(ui, "Roofing");
                row(ui, "Material", |ui| {
                    material_combo(ui, "plane_material", &mut self.draft.material);
                });
            }
            5 => {
                section(ui, "Layer");
                row(ui, "Layer", |ui| {
                    egui::ComboBox::from_id_salt("plane_layer")
                        .selected_text(self.draft.layer.clone())
                        .show_ui(ui, |ui| {
                            for l in &self.layers {
                                ui.selectable_value(&mut self.draft.layer, l.clone(), l);
                            }
                        });
                });
            }
            _ => {
                section(ui, "Label");
                row(ui, "Label", |ui| {
                    ui.text_edit_singleline(&mut self.draft.label);
                });
                ui.weak("The pitch is always shown next to the label.");
            }
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        let poly = self.draft.plan_polygon();
        if poly.len() < 3 {
            return;
        }
        let (mut lo, mut hi) = (poly[0], poly[0]);
        for q in &poly {
            lo.x = lo.x.min(q.x);
            lo.y = lo.y.min(q.y);
            hi.x = hi.x.max(q.x);
            hi.y = hi.y.max(q.y);
        }
        let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        let s = ((area.width() as f64 / w).min(area.height() as f64 * 0.8 / h)).max(1e-6);
        let c = area.center();
        let to = |q: plan_core::Point| {
            Pos2::new(
                c.x + ((q.x - (lo.x + hi.x) * 0.5) * s) as f32,
                c.y - ((q.y - (lo.y + hi.y) * 0.5) * s) as f32,
            )
        };
        let pts: Vec<Pos2> = poly.iter().map(|q| to(*q)).collect();
        p.add(egui::Shape::closed_line(
            pts.clone(),
            Stroke::new(1.5_f32, PV_INK),
        ));
        p.line_segment([pts[0], pts[1]], Stroke::new(3.0_f32, PV_ACCENT));
        p.text(
            Pos2::new(c.x, area.max.y - 8.0),
            Align2::CENTER_CENTER,
            pitch_label(self.draft.pitch),
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

/// Roof Plane Specification (RF-36).
pub struct RoofPlaneDialog {
    frame: SpecDialog,
    pages: PlanePages,
}

impl RoofPlaneDialog {
    pub fn new(record: RoofPlaneRecord, layers: Vec<String>) -> Self {
        let key = record.id;
        Self {
            frame: SpecDialog::new("Roof Plane Specification", ("roof_plane", key)),
            pages: PlanePages {
                draft: record,
                layers,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    /// The edited copy.
    pub fn draft(&self) -> &RoofPlaneRecord {
        &self.pages.draft
    }
}

// ===================================================================
// Dormer Specification
// ===================================================================

const DORMER_TABS: &[Tab] = &[on("General"), on("Roof"), on("Window")];

struct DormerPages {
    spec: DormerSpec,
    fields: Fields,
    window_on: bool,
    window: (f64, f64),
}

fn kind_name(k: DormerKind) -> &'static str {
    match k {
        DormerKind::Gable => "Gable",
        DormerKind::Shed => "Shed",
        DormerKind::Hip => "Hip",
    }
}

impl SpecPages for DormerPages {
    fn tabs(&self) -> &'static [Tab] {
        DORMER_TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            Some("Enter a valid length".into())
        } else if self.spec.width < 6.0 {
            Some("The dormer must be at least 6 inches wide".into())
        } else if self.spec.wall_height < 6.0 {
            Some("The dormer walls must be at least 6 inches tall".into())
        } else if self.spec.pitch < MIN_PITCH || self.spec.pitch > MAX_PITCH {
            Some("Pitch must be between 0.5 and 24 in 12".into())
        } else {
            None
        }
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match tab {
            0 => {
                section(ui, "Dormer");
                row(ui, "Type", |ui| {
                    egui::ComboBox::from_id_salt("dormer_kind")
                        .selected_text(kind_name(self.spec.kind))
                        .show_ui(ui, |ui| {
                            for k in [DormerKind::Gable, DormerKind::Shed, DormerKind::Hip] {
                                ui.selectable_value(&mut self.spec.kind, k, kind_name(k));
                            }
                        });
                });
                self.fields
                    .length_row(ui, "Width", "dormer_width", &mut self.spec.width);
                self.fields.length_row(
                    ui,
                    "Wall Height",
                    "dormer_wall",
                    &mut self.spec.wall_height,
                );
                section(ui, "Position on the roof plane");
                self.fields.length_row(
                    ui,
                    "Along the Eave",
                    "dormer_along",
                    &mut self.spec.position_along_eave,
                );
                self.fields.length_row(
                    ui,
                    "Setback from Eave",
                    "dormer_setback",
                    &mut self.spec.setback_from_eave,
                );
            }
            1 => {
                section(ui, "Dormer Roof");
                pitch_row(ui, "Pitch", &mut self.spec.pitch);
                self.fields.length_row(
                    ui,
                    "Height to Ridge",
                    "dormer_ridge",
                    &mut self.spec.height_to_ridge,
                );
                ui.weak("A ridge higher than the walls sets the pitch; 0 uses the pitch above.");
            }
            _ => {
                section(ui, "Window");
                ui.checkbox(&mut self.window_on, "Window in the front wall");
                if self.window_on {
                    self.fields
                        .length_row(ui, "Width", "dormer_win_w", &mut self.window.0);
                    self.fields
                        .length_row(ui, "Height", "dormer_win_h", &mut self.window.1);
                }
                self.spec.window = self.window_on.then_some(self.window);
            }
        }
    }

    fn preview(&self, p: &Painter, area: Rect) {
        // Front elevation: walls and the roof shape.
        let w = area.width() * 0.7;
        let h = area.height() * 0.3;
        let base = Pos2::new(area.center().x, area.center().y + h * 0.5);
        let left = Pos2::new(base.x - w * 0.5, base.y);
        let right = Pos2::new(base.x + w * 0.5, base.y);
        let ink = Stroke::new(1.5_f32, PV_INK);
        p.rect_stroke(
            Rect::from_two_pos(left, Pos2::new(right.x, base.y - h)),
            0.0,
            ink,
            egui::StrokeKind::Inside,
        );
        let eave_l = Pos2::new(left.x, base.y - h);
        let eave_r = Pos2::new(right.x, base.y - h);
        match self.spec.kind {
            DormerKind::Gable | DormerKind::Hip => {
                let ridge = Pos2::new(base.x, base.y - h - area.height() * 0.2);
                p.line_segment([eave_l, ridge], ink);
                p.line_segment([ridge, eave_r], ink);
            }
            DormerKind::Shed => {
                p.line_segment(
                    [eave_l, Pos2::new(eave_r.x, eave_r.y - area.height() * 0.1)],
                    ink,
                );
            }
        }
        if self.window_on {
            let c = Pos2::new(base.x, base.y - h * 0.5);
            p.rect_stroke(
                Rect::from_center_size(c, egui::vec2(w * 0.3, h * 0.5)),
                0.0,
                Stroke::new(1.0_f32, PV_ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        p.text(
            Pos2::new(area.center().x, area.max.y - 8.0),
            Align2::CENTER_CENTER,
            format!("{} dormer", kind_name(self.spec.kind)),
            FontId::proportional(13.0),
            PV_INK,
        );
    }
}

/// Dormer Specification (RF-48): the dormer's dimensions. Used after an Auto
/// Dormer click and to edit a placed dormer.
pub struct DormerDialog {
    frame: SpecDialog,
    pages: DormerPages,
}

impl DormerDialog {
    pub fn new(spec: DormerSpec) -> Self {
        Self {
            frame: SpecDialog::new("Dormer Specification", "dormer"),
            pages: DormerPages {
                window_on: spec.window.is_some(),
                window: spec.window.unwrap_or((24.0, 36.0)),
                spec,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.pages)
    }

    /// The edited dimensions.
    pub fn spec(&self) -> DormerSpec {
        self.pages.spec
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::Point;

    #[test]
    fn build_dialog_starts_from_the_settings() {
        let s = RoofSettings::fallback();
        let d = BuildRoofDialog::new(s.clone(), "Over 1st Floor");
        assert_eq!(d.settings(), &s);
        assert!(d.pages.error().is_none());
        assert_eq!(BUILD_TABS.len(), 3);
    }

    #[test]
    fn dormer_dialog_round_trips_its_spec_and_validates() {
        let spec = DormerSpec {
            window: Some((30.0, 40.0)),
            ..DormerSpec::default()
        };
        let mut d = DormerDialog::new(spec);
        assert_eq!(d.spec(), spec);
        assert!(d.pages.error().is_none());
        d.pages.spec.width = 2.0;
        assert!(d.pages.error().is_some());
        assert_eq!(DORMER_TABS.len(), 3);
    }

    #[test]
    fn plane_dialog_rejects_bad_pitch() {
        let r = RoofPlaneRecord::new(
            1,
            vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 5.0, -10.0]],
            8.0,
            (Point::new(0.0, 0.0), Point::new(10.0, 0.0)),
        );
        let mut d = RoofPlaneDialog::new(r, vec!["Roof Planes".into()]);
        assert!(d.pages.error().is_none());
        d.pages.draft.pitch = 40.0;
        assert!(d.pages.error().is_some());
        assert_eq!(PLANE_TABS.len(), 7);
    }
}
