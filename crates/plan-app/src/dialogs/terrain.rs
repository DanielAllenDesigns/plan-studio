//! Terrain Specification (Terrain > Terrain Specification; CB-51).
//!
//! General: subfloor height above the terrain, building pad elevation,
//! contour interval, smoothing and grid spacing. Materials holds the
//! grass/dirt controls the model has no fields for yet (disabled). Layer is
//! the visible layer the terrain is drawn on. The dialog edits a
//! [`TerrainRecord`] draft that the tool stores on OK.

use super::{
    dis_combo, fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab,
    PV_FAINT, PV_INK,
};
use crate::editor::site_view::TerrainRecord;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::Point;

const TABS: &[Tab] = &[on("General"), on("Materials"), on("Layer")];
/// Smallest contour interval and grid spacing the build accepts, inches.
const MIN_INTERVAL: f64 = 1.0;
const MIN_GRID: f64 = 12.0;
const MAX_SMOOTHING: u32 = 20;

pub struct TerrainDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: TerrainRecord,
    fields: Fields,
}

impl TerrainDialog {
    pub fn new(record: &TerrainRecord) -> Self {
        Self {
            frame: SpecDialog::new("Terrain Specification", "terrain_spec"),
            form: Form {
                draft: record.clone(),
                fields: Fields::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &TerrainRecord {
        &self.form.draft
    }

    #[cfg(test)]
    /// Test hook: the draft to edit before storing.
    pub fn draft_mut(&mut self) -> &mut TerrainRecord {
        &mut self.form.draft
    }
}

impl Form {
    fn general(&mut self, ui: &mut Ui) {
        section(ui, "General");
        self.fields.length_row(
            ui,
            "Subfloor height above terrain",
            "subfloor",
            &mut self.draft.terrain.subfloor_height_above_terrain,
        );
        self.fields.length_row(
            ui,
            "Building pad elevation",
            "pad",
            &mut self.draft.terrain.building_pad_elevation,
        );
        self.fields.length_row(
            ui,
            "Contour interval",
            "interval",
            &mut self.draft.contour_interval,
        );
        self.fields.length_row(
            ui,
            "Grid spacing",
            "grid",
            &mut self.draft.terrain.grid_spacing,
        );
        row(ui, "Smoothing passes", |ui| {
            ui.add(egui::DragValue::new(&mut self.draft.terrain.smoothing).range(0..=MAX_SMOOTHING))
        });
        ui.add_space(6.0);
        ui.weak(format!(
            "Finished floor elevation: {}",
            fmt_short(self.draft.terrain.finished_floor_elevation())
        ));
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        row(ui, "Ground", |ui| dis_combo(ui, "terrain_ground", "Grass"));
        row(ui, "Bare ground", |ui| {
            dis_combo(ui, "terrain_dirt", "Dirt")
        });
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            ui.text_edit_singleline(&mut self.draft.layer)
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
        if self.draft.contour_interval < MIN_INTERVAL {
            return Some("Contour interval must be at least 1\"".into());
        }
        if self.draft.terrain.grid_spacing < MIN_GRID {
            return Some("Grid spacing must be at least 1'".into());
        }
        if self.draft.layer.trim().is_empty() {
            return Some("Pick a layer".into());
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match TABS[tab].name {
            "General" => self.general(ui),
            "Materials" => self.materials(ui),
            "Layer" => self.layer(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let t = &self.draft.terrain;
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            "Terrain",
            11.0,
        );
        let area = Rect::from_min_max(Pos2::new(rect.min.x, rect.min.y + 18.0), rect.max);
        if t.perimeter.len() >= 3 {
            let (mut lo, mut hi) = (t.perimeter[0], t.perimeter[0]);
            for q in &t.perimeter {
                lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
                hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
            }
            let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
            let scale = (f64::from(area.width()) / w).min(f64::from(area.height() - 24.0) / h);
            let at = |q: &Point| {
                Pos2::new(
                    area.min.x + ((q.x - lo.x) * scale) as f32,
                    area.min.y + ((hi.y - q.y) * scale) as f32,
                )
            };
            let pts: Vec<Pos2> = t.perimeter.iter().map(at).collect();
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.5_f32, PV_INK)));
            for e in &t.elevation_points {
                p.circle_filled(at(&e.pos), 2.0, PV_INK);
            }
        } else {
            pv_text(
                p,
                area.center(),
                Align2::CENTER_CENTER,
                "No perimeter yet",
                11.0,
            );
        }
        p.hline(
            rect.x_range(),
            rect.max.y - 18.0,
            Stroke::new(0.8_f32, PV_FAINT),
        );
        pv_text(
            p,
            Pos2::new(rect.min.x, rect.max.y - 8.0),
            Align2::LEFT_CENTER,
            format!(
                "{} points, {} lines, {} regions",
                t.elevation_points.len(),
                t.elevation_lines.len(),
                t.elevation_regions.len() + t.modifiers.len()
            ),
            10.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_edits_stay_in_the_draft_until_stored() {
        let rec = TerrainRecord::new();
        let mut dlg = TerrainDialog::new(&rec);
        dlg.draft_mut().contour_interval = 24.0;
        dlg.draft_mut().terrain.subfloor_height_above_terrain = 18.0;
        assert_eq!(dlg.draft().contour_interval, 24.0);
        assert_eq!(rec.contour_interval, 12.0);
        assert!(dlg.form.error().is_none());
        dlg.draft_mut().contour_interval = 0.0;
        assert!(dlg.form.error().is_some());
    }

    #[test]
    fn dialog_draws_headlessly() {
        let mut rec = TerrainRecord::new();
        rec.terrain.perimeter = plan_terrain::Terrain::default().perimeter;
        let mut dlg = TerrainDialog::new(&rec);
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                assert_eq!(dlg.show(ctx), Outcome::Open);
            });
        }
    }
}
