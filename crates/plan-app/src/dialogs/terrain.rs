//! Terrain Specification (Terrain > Terrain Specification; CB-51).
//!
//! General: subfloor height above the terrain (the terrain-to-first-floor
//! distance), building pad elevation, grid spacing and subdivision, smoothing,
//! the plan's north angle and the auto-rebuild switch. Contours: interval,
//! major-every, label spacing. Building Pad: level the terrain under the house
//! and the cut/fill volumes of every graded pad. Materials holds the
//! grass/dirt controls the model has no fields for yet (disabled). Layer is
//! the visible layer the terrain is drawn on. The dialog edits a
//! [`TerrainRecord`] draft that the tool stores on OK. [`ObjectDialog`] is the
//! specification of one terrain object (wall, bed, plant run, ...).

use super::{
    dis_combo, fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab,
    PV_FAINT, PV_INK,
};
use crate::editor::site_view::TerrainRecord;
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::Point;
use plan_terrain::{cut_fill_report, CutFillReport};

mod object;
pub use object::ObjectDialog;

const TABS: &[Tab] = &[
    on("General"),
    on("Contours"),
    on("Building Pad"),
    on("Materials"),
    on("Layer"),
];
const MAX_SUBDIVISION: u32 = 8;
const MAX_MAJOR_EVERY: u32 = 50;
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
    /// Cut and fill of the pads as the record was opened.
    report: CutFillReport,
}

impl TerrainDialog {
    pub fn new(record: &TerrainRecord) -> Self {
        Self {
            frame: SpecDialog::new("Terrain Specification", "terrain_spec"),
            form: Form {
                draft: record.clone(),
                fields: Fields::default(),
                report: cut_fill_report(&record.terrain),
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
            "Terrain to first floor",
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
            "Grid spacing",
            "grid",
            &mut self.draft.terrain.grid_spacing,
        );
        row(ui, "Subdivision", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.draft.terrain.subdivision)
                    .range(1..=MAX_SUBDIVISION)
                    .suffix(" x"),
            )
        });
        row(ui, "Smoothing passes", |ui| {
            ui.add(egui::DragValue::new(&mut self.draft.terrain.smoothing).range(0..=MAX_SMOOTHING))
        });
        self.fields.degrees_row(
            ui,
            "North angle",
            "deg_north",
            &mut self.draft.terrain.north_angle,
        );
        ui.weak("Degrees clockwise from the top of the plan to true north (North Pointer).");
        ui.add_space(4.0);
        ui.checkbox(
            &mut self.draft.auto_rebuild,
            "Rebuild the terrain after every edit",
        );
        if !self.draft.auto_rebuild {
            ui.weak("The surface stays as built until Build Terrain runs again.");
        }
        ui.add_space(6.0);
        ui.weak(format!(
            "Finished floor elevation: {}",
            fmt_short(self.draft.terrain.finished_floor_elevation())
        ));
    }

    fn contours(&mut self, ui: &mut Ui) {
        section(ui, "Contours");
        self.fields.length_row(
            ui,
            "Contour interval",
            "interval",
            &mut self.draft.contour_interval,
        );
        row(ui, "Major contour every", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.draft.terrain.contour_major_every)
                    .range(1..=MAX_MAJOR_EVERY)
                    .suffix(" contours"),
            )
        });
        self.fields.length_row(
            ui,
            "Label spacing",
            "label_spacing",
            &mut self.draft.terrain.contour_label_spacing,
        );
        ui.weak("Elevation text along each labeled contour; 0 puts one label on each line.");
        ui.checkbox(
            &mut self.draft.terrain.contour_label_major_only,
            "Label the major contours only",
        );
    }

    fn building_pad(&mut self, ui: &mut Ui) {
        let Form {
            draft,
            fields,
            report,
        } = self;
        section(ui, "Building Pad");
        ui.checkbox(
            &mut draft.terrain.flatten_pad,
            "Level the terrain under the building",
        );
        let finished_floor = draft.terrain.finished_floor_elevation();
        match draft.terrain.building_pad.as_mut() {
            Some(pad) => {
                fields.length_row(
                    ui,
                    "Margin around the building",
                    "pad_margin",
                    &mut pad.margin,
                );
                row(ui, "Side slope 1 rise to", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut pad.slope_ratio)
                            .range(0.25..=12.0)
                            .speed(0.05)
                            .suffix(" run"),
                    )
                });
                let mut at_floor = pad.first_floor.is_some();
                if ui
                    .checkbox(
                        &mut at_floor,
                        "Level at the first floor less the terrain-to-first-floor distance",
                    )
                    .changed()
                {
                    pad.first_floor = at_floor.then_some(finished_floor);
                }
                if let Some(ff) = pad.first_floor.as_mut() {
                    fields.length_row(ui, "First floor elevation", "pad_first_floor", ff);
                } else {
                    ui.weak("Without a first floor the pad is level with the mean ground.");
                }
            }
            None => {
                ui.weak("Terrain > Building Pad creates the pad from the building walls.");
            }
        }
        ui.add_space(8.0);
        section(ui, "Cut and Fill");
        if report.is_empty() {
            ui.weak("No graded pads yet: a feature with \"Grade the terrain\" set, or the building pad.");
            return;
        }
        egui::Grid::new("terrain_cut_fill")
            .num_columns(3)
            .spacing([14.0, 3.0])
            .show(ui, |ui| {
                ui.strong("Pad");
                ui.strong("Cut cu yd");
                ui.strong("Fill cu yd");
                ui.end_row();
                for item in &report.items {
                    ui.label(&item.name);
                    ui.label(format!("{:.1}", item.cut_cy()));
                    ui.label(format!("{:.1}", item.fill_cy()));
                    ui.end_row();
                }
                ui.strong("Total");
                ui.strong(format!("{:.1}", report.cut_cy()));
                ui.strong(format!("{:.1}", report.fill_cy()));
                ui.end_row();
            });
        let net = report.net_cy();
        ui.weak(if net >= 0.0 {
            format!("{net:.1} cu yd of soil to haul away")
        } else {
            format!("{:.1} cu yd of soil to bring in", -net)
        });
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
        if self.draft.terrain.contour_label_spacing < 0.0 {
            return Some("Label spacing cannot be negative".into());
        }
        if let Some(pad) = &self.draft.terrain.building_pad {
            if pad.margin < 0.0 {
                return Some("The pad margin cannot be negative".into());
            }
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
            "Contours" => self.contours(ui),
            "Building Pad" => self.building_pad(ui),
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
    fn the_pages_and_cut_fill_table_draw_for_a_graded_terrain() {
        let mut rec = TerrainRecord::new();
        rec.terrain.perimeter = plan_terrain::Terrain::default().perimeter;
        rec.terrain.features.push(plan_terrain::Feature {
            polygon: vec![
                Point::new(400.0, 400.0),
                Point::new(640.0, 400.0),
                Point::new(640.0, 640.0),
                Point::new(400.0, 640.0),
            ],
            height: -12.0,
            pad: true,
            ..plan_terrain::Feature::default()
        });
        rec.terrain.building_pad = Some(plan_terrain::BuildingPad::default());
        let mut dlg = TerrainDialog::new(&rec);
        assert_eq!(dlg.form.report.items.len(), 1);
        let ctx = egui::Context::default();
        for tab in 0..TABS.len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
            });
        }
        dlg.draft_mut().terrain.contour_label_spacing = -1.0;
        assert!(dlg.form.error().unwrap().contains("Label"));
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
