//! Rope Light Specification (manual pp. 705-706; E-22): open by
//! double-clicking a rope light with an Electrical Tool. The same General
//! fields make the Rope Light Defaults page of Edit > Default Settings
//! ([`general_fields`] is shared with `default_pages::electrical`).
//!
//! * General: the Elevation Reference and Height of the top of the rope
//!   profile, the Distance Between Lights and Center Lights, Show Lights with
//!   the Light Display Size, and Treat as One Object (the rope light is one
//!   line of the electrical schedule).
//! * Light Data: the light sources (not built yet; disabled).
//! * Polyline: the length of the path, its area and volume and the number of
//!   lines.
//! * Strip Profile: the width and height of the strip.
//! * Layer: the Electrical layer (disabled).
//! * Schedule: whether the rope light is listed in the electrical schedule
//!   (Treat as One Object).
//!
//! The dialog edits a [`RopeDraft`]; [`RopeDraft::apply`] copies it onto the
//! rope light.

use super::{
    dis_combo, off, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_INK,
};
use crate::editor::Camera;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::{Id, Point};
use plan_electrical::{RopeLightPath, RopeReference, RopeSpec};

const TABS: &[Tab] = &[
    on("General"),
    off("Light Data"),
    on("Polyline"),
    on("Strip Profile"),
    on("Layer"),
    on("Schedule"),
];

/// What the dialog edits of a rope light.
#[derive(Clone, Debug, PartialEq)]
pub struct RopeDraft {
    pub id: Id,
    pub spec: RopeSpec,
    pub label: String,
}

impl RopeDraft {
    /// The draft of `r`.
    pub fn from_rope(r: &RopeLightPath) -> Self {
        Self {
            id: r.id,
            spec: r.spec.clone(),
            label: r.label.clone(),
        }
    }

    /// Copies the edited values onto `r`.
    pub fn apply(&self, r: &mut RopeLightPath) {
        r.spec = self.spec.clone();
        r.label = self.label.clone();
    }
}

/// The General panel's fields, shared by the specification and the defaults:
/// Elevation, Light Spacing and Plan View Display.
pub fn general_fields(ui: &mut Ui, fields: &mut Fields, spec: &mut RopeSpec) {
    section(ui, "Elevation");
    row(ui, "Elevation Reference", |ui| {
        egui::ComboBox::from_id_salt("rope_reference")
            .selected_text(spec.reference.name())
            .show_ui(ui, |ui| {
                for r in RopeReference::ALL {
                    ui.selectable_value(&mut spec.reference, r, r.name());
                }
            });
    });
    let label = match spec.reference {
        RopeReference::Floor => "Height (from Floor)",
        RopeReference::Ceiling => "Height (from Ceiling)",
    };
    fields.length_row(ui, label, "rope_height", &mut spec.height);
    section(ui, "Light Spacing");
    fields.length_row(
        ui,
        "Distance Between Lights",
        "rope_spacing",
        &mut spec.spacing,
    );
    ui.checkbox(&mut spec.center_lights, "Center Lights");
    section(ui, "Plan View Display");
    ui.checkbox(&mut spec.show_lights, "Show Lights");
    fields.length_row(
        ui,
        "Light Display Size",
        "rope_light_size",
        &mut spec.light_size,
    );
    ui.add_space(4.0);
    ui.checkbox(&mut spec.treat_as_object, "Treat as One Object");
}

/// Is a spec valid to store? A reason when not.
pub fn spec_error(spec: &RopeSpec) -> Option<String> {
    if spec.spacing < 0.5 {
        return Some("The distance between lights must be at least 1/2\"".into());
    }
    if spec.height < 0.0 {
        return Some("Height cannot be negative".into());
    }
    if spec.light_size < 0.0 {
        return Some("The light display size cannot be negative".into());
    }
    if spec.profile_width <= 0.0 || spec.profile_height <= 0.0 {
        return Some("The strip profile needs a width and a height".into());
    }
    None
}

pub struct RopeLightDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: RopeDraft,
    /// The path, for the Polyline panel and the preview.
    points: Vec<Point>,
    closed: bool,
    fields: Fields,
}

impl RopeLightDialog {
    pub fn for_rope(r: &RopeLightPath) -> Self {
        Self {
            frame: SpecDialog::new("Rope Light Specification", "rope_light"),
            form: Form {
                draft: RopeDraft::from_rope(r),
                points: r.points.clone(),
                closed: r.closed,
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &RopeDraft {
        &self.form.draft
    }

    #[cfg(test)]
    /// Test hook: the draft to edit before applying.
    pub fn draft_mut(&mut self) -> &mut RopeDraft {
        &mut self.form.draft
    }
}

impl Form {
    fn path(&self) -> RopeLightPath {
        let mut r = RopeLightPath::new(self.points.clone(), self.draft.spec.clone());
        r.closed = self.closed;
        r
    }

    fn polyline(&self, ui: &mut Ui) {
        section(ui, "Polyline");
        let r = self.path();
        let len = r.length();
        let s = &self.draft.spec;
        let profile_area = s.profile_width * s.profile_height;
        row(ui, "Perimeter", |ui| {
            ui.label(plan_core::units::fmt_ft_in(len))
        });
        row(ui, "Area", |ui| ui.label("0 sq in"));
        row(ui, "Volume", |ui| {
            ui.label(format!("{:.1} cu in", profile_area * len))
        });
        row(ui, "Number of Lines", |ui| {
            ui.label(r.segments().len().to_string())
        });
        row(ui, "Light Sources", |ui| {
            ui.label(r.lights().len().to_string())
        });
    }

    fn strip(&mut self, ui: &mut Ui) {
        section(ui, "Strip Profile");
        self.fields.length_row(
            ui,
            "Width",
            "rope_profile_width",
            &mut self.draft.spec.profile_width,
        );
        self.fields.length_row(
            ui,
            "Height",
            "rope_profile_height",
            &mut self.draft.spec.profile_height,
        );
    }

    fn schedule(&mut self, ui: &mut Ui) {
        section(ui, "Schedule");
        ui.checkbox(
            &mut self.draft.spec.treat_as_object,
            "Include in the Electrical Schedule (Treat as One Object)",
        );
        row(ui, "Label", |ui| {
            ui.text_edit_singleline(&mut self.draft.label)
        });
    }
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        TABS
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        spec_error(&self.draft.spec)
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS[tab].name {
            "General" => general_fields(ui, &mut self.fields, &mut self.draft.spec),
            "Polyline" => self.polyline(ui),
            "Strip Profile" => self.strip(ui),
            "Layer" => {
                section(ui, "Layer");
                row(ui, "Layer", |ui| dis_combo(ui, "rope_layer", "Electrical"));
            }
            "Schedule" => self.schedule(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            "Rope Light",
            11.0,
        );
        let area = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + 16.0), rect.max);
        let r = self.path();
        let (mut lo, mut hi) = (
            Point::new(f64::INFINITY, f64::INFINITY),
            Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY),
        );
        for q in &r.points {
            lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
            hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
        }
        if !lo.x.is_finite() {
            return;
        }
        let span = (hi.x - lo.x).max(hi.y - lo.y).max(12.0);
        let cam = Camera {
            center: Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5),
            px_per_in: f64::from(area.width().min(area.height())) * 0.85 / span,
            rect: area,
            rotation: 0.0,
        };
        let mut pts: Vec<Pos2> = r.points.iter().map(|q| cam.world_to_screen(*q)).collect();
        if r.closed && pts.len() > 2 {
            pts.push(pts[0]);
        }
        p.add(egui::Shape::line(pts, Stroke::new(1.5_f32, PV_INK)));
        if self.draft.spec.show_lights {
            let dot = ((self.draft.spec.light_size * 0.5 * cam.px_per_in) as f32).clamp(1.5, 5.0);
            for l in r.lights() {
                p.circle_filled(cam.world_to_screen(l), dot, PV_INK);
            }
        }
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.max.y - 6.0),
            Align2::CENTER_CENTER,
            format!("Length {}", super::fmt_short(r.length())),
            11.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rope() -> RopeLightPath {
        let mut r = RopeLightPath::new(
            vec![Point::new(0.0, 0.0), Point::new(100.0, 0.0)],
            RopeSpec::default(),
        );
        r.id = 4;
        r
    }

    #[test]
    fn the_draft_round_trips_onto_the_rope_light() {
        let mut r = rope();
        let mut dlg = RopeLightDialog::for_rope(&r);
        {
            let d = dlg.draft_mut();
            d.spec.spacing = 10.0;
            d.spec.reference = RopeReference::Ceiling;
            d.spec.height = 6.0;
            d.spec.treat_as_object = false;
            d.label = "Cove".into();
        }
        dlg.draft().apply(&mut r);
        assert_eq!(r.spec.spacing, 10.0);
        assert_eq!(r.spec.reference, RopeReference::Ceiling);
        assert!(!r.spec.treat_as_object);
        assert_eq!(r.label, "Cove");
        assert_eq!(r.points.len(), 2, "the path is not edited here");
    }

    #[test]
    fn a_bad_spec_blocks_ok() {
        let mut s = RopeSpec::default();
        assert!(spec_error(&s).is_none());
        s.spacing = 0.0;
        assert!(spec_error(&s).is_some());
        s.spacing = 6.0;
        s.profile_height = 0.0;
        assert!(spec_error(&s).is_some());
    }

    #[test]
    fn the_dialog_draws_headlessly() {
        let r = rope();
        let mut dlg = RopeLightDialog::for_rope(&r);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(dlg.show(ctx), Outcome::Open);
            });
        }
    }
}
