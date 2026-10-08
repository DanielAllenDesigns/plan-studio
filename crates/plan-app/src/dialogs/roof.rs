//! The Build Roof dialog (RF-1, RF-2) and the Roof Plane Specification
//! (RF-36), on the shared dialog frame.
//!
//! Both edit a cloned draft; OK hands it back through `draft()`.

#![allow(dead_code)]

use super::{
    dis_check, fmt_short, on, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_ACCENT,
    PV_INK,
};
use crate::editor::roof_view::{pitch_label, RoofPlaneRecord, RoofSettings, ROOF_MATERIALS};
use eframe::egui::{self, Align2, FontId, Painter, Pos2, Rect, Stroke, Ui};

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
            1 => {
                section(ui, "Eaves and Ridge");
                ui.checkbox(&mut self.draft.ridge_caps, "Include Ridge Caps")
                    .on_hover_text("Stored; ridge caps are not modeled yet");
                ui.checkbox(&mut self.draft.gutters, "Include Gutter")
                    .on_hover_text("Stored; gutters are not modeled yet");
            }
            2 => {
                section(ui, "Roofing");
                row(ui, "Material", |ui| {
                    material_combo(ui, "plane_material", &mut self.draft.material);
                });
            }
            3 => {
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
        assert_eq!(PLANE_TABS.len(), 5);
    }
}
