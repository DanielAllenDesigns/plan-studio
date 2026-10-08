//! Terrain Specification (Terrain > Terrain Specification; CB-51) and its two
//! single-purpose forms, the Terrain Cut and Fill Report and Import Terrain Data.
//!
//! Specification pages: General (subfloor height above the terrain, building
//! pad elevation, level the terrain under the building, contour interval,
//! grid spacing and subdivision, smoothing, north angle, auto-rebuild),
//! Contours (interval, major-every, label spacing, and the line style of the
//! primary and secondary contours: color, weight, dashed, labels), Building
//! Pad (the pad's margin and slope, with the cut/fill table), Materials
//! (ground and bare-ground materials), Layer. The dialog edits a
//! [`TerrainRecord`] draft that the tool stores on OK.
//!
//! The Cut and Fill Report lists every graded pad in cubic yards, with the
//! totals and the soil to haul away or bring in, and copies itself as CSV. It
//! only reports: OK and Cancel store nothing. Import Terrain Data reads
//! survey points from DXF, GPX or XYZ text (a file or pasted text,
//! `plan_terrain::import_points`) into the elevation data of the draft; OK
//! stores them as one undo step. [`ObjectDialog`] is the specification of one
//! terrain object (wall, bed, plant run, ...).

use super::{
    fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_FAINT,
    PV_INK,
};
use crate::editor::site_view::TerrainRecord;
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::Point;
use plan_terrain::{
    cut_fill_report, import_points, ContourStyle, CutFillReport, ImportUnit, ImportedPoints,
    PRIMARY_CONTOUR_WEIGHT, SECONDARY_CONTOUR_WEIGHT,
};

mod object;
pub use object::ObjectDialog;

const TABS: &[Tab] = &[
    on("General"),
    on("Contours"),
    on("Building Pad"),
    on("Materials"),
    on("Layer"),
];
const REPORT_TABS: &[Tab] = &[on("Cut and Fill")];
const IMPORT_TABS: &[Tab] = &[on("Import")];
const MAX_SUBDIVISION: u32 = 8;
const MAX_MAJOR_EVERY: u32 = 50;
/// Smallest contour interval and grid spacing the build accepts, inches.
const MIN_INTERVAL: f64 = 1.0;
const MIN_GRID: f64 = 12.0;
const MAX_SMOOTHING: u32 = 20;
/// Materials the ground surface can be built of.
pub const GROUND_MATERIALS: [&str; 6] = ["Grass", "Dirt", "Gravel", "Stone", "Mulch", "Concrete"];
/// Materials of the bare ground (cut slopes, strips under walls).
pub const DIRT_MATERIALS: [&str; 4] = ["Dirt", "Gravel", "Mulch", "Stone"];

/// Which form the dialog is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Specification,
    CutFill,
    Import,
}

pub struct TerrainDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    mode: Mode,
    draft: TerrainRecord,
    fields: Fields,
    /// Cut and fill of the pads as the record was opened.
    report: CutFillReport,
    import: ImportState,
    /// Elevation points the record had when the dialog opened.
    opened_points: usize,
}

/// The Import Terrain Data form.
struct ImportState {
    text: String,
    unit: ImportUnit,
    /// Center the points on the terrain perimeter.
    center: bool,
    /// Lower the points so the lowest is at elevation 0.
    zero_lowest: bool,
    /// What the last Add did: a summary, or why it failed.
    message: Option<Result<String, String>>,
}

impl TerrainDialog {
    fn build(record: &TerrainRecord, mode: Mode, title: &str, key: &str) -> Self {
        Self {
            frame: SpecDialog::new(title, key),
            form: Form {
                mode,
                draft: record.clone(),
                fields: Fields::default(),
                report: cut_fill_report(&record.terrain),
                opened_points: record.terrain.elevation_points.len(),
                import: ImportState {
                    text: String::new(),
                    unit: ImportUnit::Auto,
                    center: record.has_perimeter(),
                    zero_lowest: false,
                    message: None,
                },
            },
        }
    }

    pub fn new(record: &TerrainRecord) -> Self {
        Self::build(
            record,
            Mode::Specification,
            "Terrain Specification",
            "terrain_spec",
        )
    }

    /// The Terrain Cut and Fill Report.
    pub fn cut_fill(record: &TerrainRecord) -> Self {
        Self::build(
            record,
            Mode::CutFill,
            "Terrain Cut and Fill Report",
            "terrain_cut_fill",
        )
    }

    /// Import Terrain Data.
    pub fn import(record: &TerrainRecord) -> Self {
        Self::build(
            record,
            Mode::Import,
            "Import Terrain Data",
            "terrain_import",
        )
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &TerrainRecord {
        &self.form.draft
    }

    /// Does OK store the draft? The cut and fill report only reports, and
    /// Import Terrain Data stores only when it added points.
    pub fn stores(&self) -> bool {
        match self.form.mode {
            Mode::Specification => true,
            Mode::CutFill => false,
            Mode::Import => {
                self.form.draft.terrain.elevation_points.len() > self.form.opened_points
            }
        }
    }

    #[cfg(test)]
    /// Test hook: the draft to edit before storing.
    pub fn draft_mut(&mut self) -> &mut TerrainRecord {
        &mut self.form.draft
    }

    #[cfg(test)]
    pub fn tab_names(&self) -> Vec<&'static str> {
        self.form.tabs().iter().map(|t| t.name).collect()
    }

    #[cfg(test)]
    /// Test hook: types survey text into the Import form.
    pub fn set_import_text(&mut self, text: &str) {
        self.form.import.text = text.into();
    }

    #[cfg(test)]
    /// Test hook: presses "Add to terrain" and returns the message.
    pub fn press_add(&mut self) -> Option<Result<String, String>> {
        self.form.add_import();
        self.form.import.message.clone()
    }

    #[cfg(test)]
    pub fn report(&self) -> &CutFillReport {
        &self.form.report
    }
}

/// A line color picker with "use a color of its own".
fn color_row(ui: &mut Ui, value: &mut Option<[u8; 3]>, default: [u8; 3]) {
    let mut own = value.is_some();
    if ui.checkbox(&mut own, "Use a color of its own").changed() {
        *value = own.then_some(default);
    }
    if let Some(c) = value {
        row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
    }
}

/// The weight, color and dash controls of one family of contour lines, with a
/// sample of the line.
fn contour_style_rows(
    ui: &mut Ui,
    style: &mut ContourStyle,
    default: [u8; 3],
    default_weight: f64,
) {
    color_row(ui, &mut style.color, default);
    row(ui, "Line weight", |ui| {
        ui.add(
            egui::DragValue::new(&mut style.weight)
                .range(0.0..=8.0)
                .speed(0.05)
                .suffix(" pt"),
        )
    });
    ui.weak(format!("0 takes the default of {default_weight} pt."));
    ui.checkbox(&mut style.dashed, "Dashed");
    let (rect, _) = ui.allocate_exact_size(egui::vec2(200.0, 14.0), egui::Sense::hover());
    let c = style.color.unwrap_or(default);
    let stroke = Stroke::new(
        style.weight_or(default_weight) as f32 * 1.6,
        Color32::from_rgb(c[0], c[1], c[2]),
    );
    let y = rect.center().y;
    if style.dashed {
        ui.painter().extend(egui::Shape::dashed_line(
            &[Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            stroke,
            8.0,
            5.0,
        ));
    } else {
        ui.painter()
            .line_segment([Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)], stroke);
    }
}

/// The plan's default colors of the two contour families (match `site_view`).
const PRIMARY_COLOR: [u8; 3] = [0x7A, 0x55, 0x2B];
const SECONDARY_COLOR: [u8; 3] = [0xA8, 0x8B, 0x63];

fn material_combo(ui: &mut Ui, salt: &str, value: &mut String, options: &[&str]) {
    let shown = if value.trim().is_empty() {
        options[0].to_string()
    } else {
        value.clone()
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for o in options {
                ui.selectable_value(value, (*o).to_string(), *o);
            }
        });
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
        ui.checkbox(
            &mut self.draft.terrain.flatten_pad,
            "Level the terrain under the building automatically",
        );
        self.fields.length_row(
            ui,
            "Contour interval",
            "interval_general",
            &mut self.draft.contour_interval,
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
        ui.add_space(6.0);
        section(ui, "Primary lines (the major contours)");
        contour_style_rows(
            ui,
            &mut self.draft.terrain.contour_primary,
            PRIMARY_COLOR,
            PRIMARY_CONTOUR_WEIGHT,
        );
        ui.add_enabled(
            false,
            egui::Checkbox::new(&mut true, "Labeled with their elevation"),
        );
        ui.add_space(6.0);
        section(ui, "Secondary lines (between the majors)");
        contour_style_rows(
            ui,
            &mut self.draft.terrain.contour_secondary,
            SECONDARY_COLOR,
            SECONDARY_CONTOUR_WEIGHT,
        );
        let mut labeled = !self.draft.terrain.contour_label_major_only;
        if ui
            .checkbox(&mut labeled, "Labeled with their elevation")
            .changed()
        {
            self.draft.terrain.contour_label_major_only = !labeled;
        }
    }

    fn building_pad(&mut self, ui: &mut Ui) {
        let Form {
            draft,
            fields,
            report,
            ..
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
        cut_fill_table(ui, report);
    }

    fn materials(&mut self, ui: &mut Ui) {
        section(ui, "Materials");
        row(ui, "Ground", |ui| {
            material_combo(
                ui,
                "terrain_ground",
                &mut self.draft.terrain.ground_material,
                &GROUND_MATERIALS,
            )
        });
        row(ui, "Bare ground", |ui| {
            material_combo(
                ui,
                "terrain_dirt",
                &mut self.draft.terrain.dirt_material,
                &DIRT_MATERIALS,
            )
        });
        ui.weak(
            "The ground material colors the terrain surface in 3D; the bare ground shows on \
             the cut slopes of graded pads.",
        );
    }

    fn layer(&mut self, ui: &mut Ui) {
        section(ui, "Layer");
        row(ui, "Layer", |ui| {
            ui.text_edit_singleline(&mut self.draft.layer)
        });
    }

    fn cut_fill(&mut self, ui: &mut Ui) {
        section(ui, "Cut and Fill");
        cut_fill_table(ui, &self.report);
        if self.report.is_empty() {
            return;
        }
        ui.add_space(6.0);
        if ui.button("Copy as CSV").clicked() {
            ui.ctx().copy_text(self.report.to_csv());
        }
    }

    fn import_page(&mut self, ui: &mut Ui) {
        section(ui, "Import Terrain Data");
        ui.weak("Survey points from a DXF drawing, a GPX file or XYZ text (x y z per line).");
        ui.add_space(4.0);
        if ui.button("Choose File\u{2026}").clicked() {
            self.choose_file();
        }
        ui.label("Or paste the text here:");
        ui.add(
            egui::TextEdit::multiline(&mut self.import.text)
                .desired_rows(8)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
        row(ui, "Coordinates in", |ui| {
            egui::ComboBox::from_id_salt("import_unit")
                .selected_text(self.import.unit.name())
                .show_ui(ui, |ui| {
                    for u in ImportUnit::ALL {
                        ui.selectable_value(&mut self.import.unit, u, u.name());
                    }
                })
        });
        ui.add_enabled(
            self.draft.has_perimeter(),
            egui::Checkbox::new(
                &mut self.import.center,
                "Center the points on the terrain perimeter",
            ),
        );
        ui.checkbox(
            &mut self.import.zero_lowest,
            "Make the lowest point elevation 0",
        );
        ui.add_space(4.0);
        if ui
            .add_enabled(
                !self.import.text.trim().is_empty(),
                egui::Button::new("Add to terrain"),
            )
            .clicked()
        {
            self.add_import();
        }
        match &self.import.message {
            Some(Ok(m)) => {
                ui.label(m);
            }
            Some(Err(e)) => {
                ui.colored_label(Color32::from_rgb(0xB0, 0x30, 0x30), e);
            }
            None => {}
        }
        ui.weak(format!(
            "{} elevation points in the terrain",
            self.draft.terrain.elevation_points.len()
        ));
    }

    /// Reads a file into the text box.
    fn choose_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Survey points", &["dxf", "gpx", "txt", "csv", "xyz", "pts"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.import.text = text;
                self.import.message = None;
            }
            Err(e) => {
                self.import.message = Some(Err(format!("Could not read {}: {e}", path.display())));
            }
        }
    }

    /// Parses the text and adds its points to the draft's elevation data.
    fn add_import(&mut self) {
        let result = import_points(&self.import.text, self.import.unit).map(|mut got| {
            if self.import.center && self.draft.has_perimeter() {
                got.center_on(perimeter_center(&self.draft.terrain.perimeter));
            }
            if self.import.zero_lowest {
                got.zero_lowest();
            }
            got
        });
        self.import.message = Some(result.map(|got: ImportedPoints| {
            let added = self.draft.terrain.add_elevation_points(&got.points);
            format!("{}; {added} added", got.summary())
        }));
    }
}

/// The middle of the perimeter's bounding box.
fn perimeter_center(perimeter: &[Point]) -> Point {
    let (mut lo, mut hi) = (perimeter[0], perimeter[0]);
    for q in perimeter {
        lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
        hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
    }
    Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0)
}

/// The cut and fill table of the pads, with the soil to move.
fn cut_fill_table(ui: &mut Ui, report: &CutFillReport) {
    if report.is_empty() {
        ui.weak(
            "No graded pads yet: a feature with \"Grade the terrain\" set, or the building pad.",
        );
        return;
    }
    egui::Grid::new("terrain_cut_fill")
        .num_columns(5)
        .spacing([14.0, 3.0])
        .show(ui, |ui| {
            ui.strong("Pad");
            ui.strong("Top");
            ui.strong("Area sq ft");
            ui.strong("Cut cu yd");
            ui.strong("Fill cu yd");
            ui.end_row();
            for item in &report.items {
                ui.label(&item.name);
                ui.label(fmt_short(item.top));
                ui.label(format!("{:.0}", item.area_sq_ft));
                ui.label(format!("{:.1}", item.cut_cy()));
                ui.label(format!("{:.1}", item.fill_cy()));
                ui.end_row();
            }
            ui.strong("Total");
            ui.label("");
            ui.label("");
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

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        match self.mode {
            Mode::Specification => TABS,
            Mode::CutFill => REPORT_TABS,
            Mode::Import => IMPORT_TABS,
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        if self.mode != Mode::Specification {
            return None;
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
        match self.tabs()[tab].name {
            "General" => self.general(ui),
            "Contours" => self.contours(ui),
            "Building Pad" => self.building_pad(ui),
            "Materials" => self.materials(ui),
            "Layer" => self.layer(ui),
            "Cut and Fill" => self.cut_fill(ui),
            "Import" => self.import_page(ui),
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

    fn graded_record() -> TerrainRecord {
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
        rec
    }

    fn draw_all_pages(dlg: &mut TerrainDialog) {
        let ctx = egui::Context::default();
        for tab in 0..dlg.form.tabs().len() {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| dlg.form.page(ui, tab));
            });
        }
    }

    #[test]
    fn the_three_forms_have_their_own_tabs_and_titles() {
        let rec = graded_record();
        let spec = TerrainDialog::new(&rec);
        assert_eq!(
            spec.tab_names(),
            ["General", "Contours", "Building Pad", "Materials", "Layer"]
        );
        assert!(spec.stores());
        let report = TerrainDialog::cut_fill(&rec);
        assert_eq!(report.tab_names(), ["Cut and Fill"]);
        assert!(!report.stores(), "the report stores nothing");
        assert_eq!(report.report().items.len(), 1);
        let import = TerrainDialog::import(&rec);
        assert_eq!(import.tab_names(), ["Import"]);
        assert!(
            !import.stores(),
            "an import that added nothing stores nothing"
        );
        for mut d in [spec, report, import] {
            draw_all_pages(&mut d);
            let ctx = egui::Context::default();
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    assert_eq!(d.show(ctx), Outcome::Open);
                });
            }
        }
    }

    #[test]
    fn the_materials_and_contour_styles_are_live_in_the_draft() {
        let mut dlg = TerrainDialog::new(&graded_record());
        {
            let t = &mut dlg.draft_mut().terrain;
            t.ground_material = "Gravel".into();
            t.contour_primary.weight = 2.0;
            t.contour_secondary.dashed = true;
            t.contour_secondary.color = Some([10, 20, 30]);
            t.contour_label_major_only = false;
        }
        draw_all_pages(&mut dlg);
        let mut rec = graded_record();
        rec.apply_spec(dlg.draft());
        assert_eq!(rec.terrain.ground_material, "Gravel");
        assert_eq!(rec.terrain.contour_primary.weight, 2.0);
        assert!(rec.terrain.contour_secondary.dashed);
        assert_eq!(rec.terrain.contour_secondary.color, Some([10, 20, 30]));
        assert!(!rec.terrain.contour_label_major_only);
        // The default record is Grass and Dirt with the plan's own line styles.
        let base = TerrainRecord::new().terrain;
        assert_eq!(base.ground_material, "Grass");
        assert_eq!(base.dirt_material, "Dirt");
        assert_eq!(base.contour_primary, ContourStyle::default());
    }

    #[test]
    fn the_import_form_adds_survey_points_to_the_draft_and_apply_stores_them() {
        let rec = graded_record();
        let mut dlg = TerrainDialog::import(&rec);
        assert_eq!(
            dlg.press_add().map(|r| r.is_err()),
            Some(true),
            "nothing typed"
        );
        dlg.set_import_text("0 0 100\n10 0 102\n10 10 104\n0 10 103\n");
        let msg = dlg.press_add().unwrap().unwrap();
        assert!(msg.contains("4 points read from XYZ text"), "{msg}");
        assert!(msg.ends_with("4 added"), "{msg}");
        assert!(dlg.stores());
        assert_eq!(dlg.draft().terrain.elevation_points.len(), 4);
        // The survey was centered on the lot (600, 480).
        let ys: Vec<f64> = dlg
            .draft()
            .terrain
            .elevation_points
            .iter()
            .map(|e| e.pos.y)
            .collect();
        assert!((ys.iter().sum::<f64>() / 4.0 - 480.0).abs() < 1e-6);
        // Adding the same text again adds nothing.
        let msg = dlg.press_add().unwrap().unwrap();
        assert!(msg.ends_with("0 added"), "{msg}");
        draw_all_pages(&mut dlg);
        // OK stores the points through apply_spec.
        let mut stored = rec.clone();
        stored.apply_spec(dlg.draft());
        assert_eq!(stored.terrain.elevation_points.len(), 4);
        // A draft without more points never wipes the stored ones.
        let mut again = stored.clone();
        again.apply_spec(&TerrainRecord::new());
        assert_eq!(again.terrain.elevation_points.len(), 4);
    }

    #[test]
    fn the_cut_fill_report_copies_as_csv() {
        let dlg = TerrainDialog::cut_fill(&graded_record());
        let csv = dlg.report().to_csv();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "Pad,Top elevation (in),Area (sq ft),Cut (cu yd),Fill (cu yd)"
        );
        assert_eq!(lines.len(), 3, "{csv}");
        assert!(lines[1].starts_with("Rectangular Feature 1,"), "{csv}");
        assert!(lines[2].starts_with("Total,,,"), "{csv}");
    }
}
