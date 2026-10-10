//! Terrain Specification (Terrain > Terrain Specification; CB-51) and its
//! single-purpose forms: the Terrain Cut and Fill Report, the Import Terrain
//! Assistant, the Import GPS Data Assistant and Grow Plants.
//!
//! Specification pages: General (Absolute Elevation: Automatic or retain the
//! surface at the Reference Point or at Contour 0, with the distance, the
//! Floor 1 subfloor elevation and the reference point; building pad
//! elevation, Flatten Pad, Hide Terrain Intersected by Building, the Skirt,
//! Terrain Surface Smoothing, Triangle Count with its readout, contour
//! interval, grid spacing, north angle, season, auto-rebuild), Contours
//! (interval, offset, major-every, label spacing and units, highlight
//! negative elevations, 2D smoothing, and the line style of the primary and
//! secondary contours: color, weight, dashed, labels), Polyline (the
//! perimeter's length, area and lines), Building Pad (the pad's margin and
//! slope, with the cut/fill table), Materials (ground, bare-ground and skirt
//! materials), Label, Object Information, Schedule (the perimeter's own),
//! Layer. The dialog edits a [`TerrainRecord`] draft that the tool stores on
//! OK.
//!
//! The Cut and Fill Report lists every graded pad in cubic yards, with the
//! totals and the soil to haul away or bring in, and copies itself as CSV. It
//! only reports: OK and Cancel store nothing. The Import Terrain Assistant
//! (Select File, Filter Data, Scale Data) and the Import GPS Data Assistant
//! (Select File, Import As, Transform Coordinates) read survey points from
//! DXF, GPX or column text (`plan_terrain::import_assistant`); OK stores
//! them as one undo step. [`ObjectDialog`] is the specification of one
//! terrain object (wall, bed, plant run, ...).

use super::{
    fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_FAINT,
    PV_INK,
};
use crate::editor::site_view::TerrainRecord;
use eframe::egui::{self, Align2, Color32, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::Point;
use plan_terrain::{
    cut_fill_report, AbsoluteElevation, ContourStyle, CutFillReport, GpsResult, LabelUnits,
    ObjectKey, Season, SkirtMode, SmoothingLevel, TriangleDetail, PRIMARY_CONTOUR_WEIGHT,
    SECONDARY_CONTOUR_WEIGHT,
};

mod assistants;
mod object;
mod panels;
pub use object::ObjectDialog;

const TABS: &[Tab] = &[
    on("General"),
    on("Contours"),
    on("Polyline"),
    on("Line Style"),
    on("Fill Style"),
    on("Building Pad"),
    on("Materials"),
    on("Label"),
    on("Components"),
    on("Object Information"),
    on("Schedule"),
    on("Layer"),
];
const REPORT_TABS: &[Tab] = &[on("Cut and Fill")];
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
pub enum Mode {
    Specification,
    CutFill,
    Import,
    ImportGps,
    Grow,
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
    import: assistants::ImportState,
    gps: assistants::GpsState,
    grow: assistants::GrowState,
    /// Elevation points the record had when the dialog opened.
    opened_points: usize,
    /// Plants of the record when it opened (Grow Plants stores only if they changed).
    opened_landscape: Vec<plan_terrain::Landscape>,
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
                import: assistants::ImportState::new(record.has_perimeter()),
                gps: assistants::GpsState::new(),
                grow: assistants::GrowState::new(record),
                opened_landscape: record.terrain.landscape.clone(),
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

    /// The Import Terrain Assistant (File > Import > Terrain Data).
    pub fn import(record: &TerrainRecord) -> Self {
        Self::build(
            record,
            Mode::Import,
            "Import Terrain Assistant",
            "terrain_import",
        )
    }

    /// The Import GPS Data Assistant (File > Import > GPS Data).
    pub fn import_gps(record: &TerrainRecord) -> Self {
        Self::build(
            record,
            Mode::ImportGps,
            "Import GPS Data Assistant",
            "terrain_import_gps",
        )
    }

    /// Terrain > Plant > Grow All Plants.
    pub fn grow(record: &TerrainRecord) -> Self {
        Self::build(record, Mode::Grow, "Grow Plants", "terrain_grow")
    }

    /// Which form this is.
    pub fn mode(&self) -> Mode {
        self.form.mode
    }

    /// What the GPS assistant made (markers and polylines go on the plan).
    pub fn gps_result(&self) -> Option<&GpsResult> {
        self.form.gps.result.as_ref()
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
            Mode::ImportGps => self.form.gps.result.is_some(),
            Mode::Grow => self.form.draft.terrain.landscape != self.form.opened_landscape,
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
    /// Test hook: the Import Terrain Assistant's state.
    fn import_mut(&mut self) -> &mut assistants::ImportState {
        &mut self.form.import
    }

    #[cfg(test)]
    /// Test hook: presses "Add to terrain" and returns the message.
    pub fn press_add(&mut self) -> Option<Result<String, String>> {
        self.form.import.add(&mut self.form.draft);
        self.form.import.message.clone()
    }

    #[cfg(test)]
    /// Test hook: types GPX text into the GPS form.
    pub fn set_gps_text(&mut self, text: &str) {
        self.form.gps.text = text.into();
    }

    #[cfg(test)]
    /// Test hook: presses the GPS assistant's Import button.
    pub fn press_gps_import(&mut self) -> Option<Result<String, String>> {
        self.form.gps.run(&mut self.form.draft);
        self.form.gps.message.clone()
    }

    #[cfg(test)]
    /// Test hook: moves the Grow Plants slider.
    pub fn set_grow_years(&mut self, years: f64) {
        self.form.grow.years = years;
        self.form.grow.changed =
            plan_terrain::grow_plants(&mut self.form.draft.terrain.landscape, years);
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

/// The plan's default color of the terrain perimeter (matches `site_view`).
const PERIMETER_COLOR: [u8; 3] = [0x4F, 0x7F, 0x3A];
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
        let auto = self.draft.terrain.absolute_elevation == AbsoluteElevation::Automatic;
        let mut automatic = auto;
        if ui
            .checkbox(&mut automatic, "Absolute elevation: automatic")
            .changed()
        {
            self.draft.terrain.absolute_elevation = if automatic {
                AbsoluteElevation::Automatic
            } else {
                AbsoluteElevation::ReferencePoint
            };
        }
        if automatic {
            self.fields.length_row(
                ui,
                "Terrain to first floor",
                "subfloor",
                &mut self.draft.terrain.subfloor_height_above_terrain,
            );
            ui.weak(
                "The distance between Floor 1 and the terrain is set for you (6\"); \
                 type another to override it.",
            );
        } else {
            row(ui, "Retain surface elevation at", |ui| {
                ui.radio_value(
                    &mut self.draft.terrain.absolute_elevation,
                    AbsoluteElevation::ReferencePoint,
                    "Reference Point",
                );
                ui.radio_value(
                    &mut self.draft.terrain.absolute_elevation,
                    AbsoluteElevation::ContourZero,
                    "Contour 0",
                );
            });
            let at_reference =
                self.draft.terrain.absolute_elevation == AbsoluteElevation::ReferencePoint;
            self.fields.length_row(
                ui,
                if at_reference {
                    "Surface at Reference Point"
                } else {
                    "Surface at Contour 0"
                },
                "surface_offset",
                &mut self.draft.terrain.surface_offset,
            );
            ui.weak("The vertical distance from the Floor 1 subfloor to the surface (normally negative).");
            self.fields.length_row(
                ui,
                "Floor 1 subfloor elevation",
                "floor_one",
                &mut self.draft.terrain.floor_one_elevation,
            );
            if at_reference {
                let mut rp = self.draft.terrain.effective_reference_point();
                if let Some(p) = rp.as_mut() {
                    let (mut x, mut y) = (p.x, p.y);
                    let cx = self
                        .fields
                        .length_row(ui, "Reference Point X", "ref_x", &mut x);
                    let cy = self
                        .fields
                        .length_row(ui, "Reference Point Y", "ref_y", &mut y);
                    if cx || cy {
                        self.draft.terrain.reference_point = Some(Point::new(x, y));
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Place at the middle of the perimeter").clicked() {
                        self.draft.terrain.reference_point = None;
                        self.draft.terrain.reference_point =
                            self.draft.terrain.effective_reference_point();
                    }
                    if ui
                        .add_enabled(
                            self.draft.terrain.reference_point.is_some(),
                            egui::Button::new("Remove"),
                        )
                        .clicked()
                    {
                        self.draft.terrain.reference_point = None;
                    }
                });
            }
        }
        self.fields.length_row(
            ui,
            "Building pad elevation",
            "pad",
            &mut self.draft.terrain.building_pad_elevation,
        );
        ui.checkbox(
            &mut self.draft.terrain.flatten_pad,
            "Flatten pad: level the terrain under the building",
        );
        ui.checkbox(
            &mut self.draft.terrain.hide_under_building,
            "Hide terrain intersected by building",
        );
        ui.add_space(4.0);
        section(ui, "Skirt");
        ui.checkbox(
            &mut self.draft.terrain.skirt.enabled,
            "Skirt around the terrain edge",
        );
        if self.draft.terrain.skirt.enabled {
            self.fields.length_row(
                ui,
                "Thickness",
                "skirt_t",
                &mut self.draft.terrain.skirt.thickness,
            );
            row(ui, "Bottom", |ui| {
                for m in [SkirtMode::FlatBase, SkirtMode::FollowTerrain] {
                    ui.radio_value(&mut self.draft.terrain.skirt.mode, m, m.name());
                }
            });
        }
        ui.add_space(4.0);
        section(ui, "Surface");
        row(ui, "Terrain surface smoothing", |ui| {
            egui::ComboBox::from_id_salt("terrain_smoothing")
                .selected_text(self.draft.terrain.smoothing_level.name())
                .show_ui(ui, |ui| {
                    for l in SmoothingLevel::ALL {
                        ui.selectable_value(&mut self.draft.terrain.smoothing_level, l, l.name());
                    }
                })
        });
        if self.draft.terrain.smoothing_level == SmoothingLevel::Passes {
            row(ui, "Smoothing passes", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.draft.terrain.smoothing)
                        .range(0..=MAX_SMOOTHING),
                )
            });
        }
        row(ui, "Triangle count", |ui| {
            egui::ComboBox::from_id_salt("terrain_triangles")
                .selected_text(self.draft.terrain.triangle_detail.name())
                .show_ui(ui, |ui| {
                    for d in TriangleDetail::ALL {
                        ui.selectable_value(&mut self.draft.terrain.triangle_detail, d, d.name());
                    }
                })
        });
        match self.draft.terrain.triangle_detail {
            TriangleDetail::Grid => {
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
                self.fields.length_row(
                    ui,
                    "Maximum triangle size",
                    "max_tri",
                    &mut self.draft.terrain.max_triangle_size,
                );
                ui.weak("0 uses the grid spacing.");
            }
            TriangleDetail::Custom => {
                row(ui, "Number of triangles", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut self.draft.terrain.custom_triangles)
                            .range(50..=60_000),
                    )
                });
            }
            _ => {}
        }
        let built = self
            .draft
            .terrain
            .last_build
            .map_or("not built yet".to_string(), |b| {
                format!("{} in the last build", b.triangles)
            });
        ui.weak(format!(
            "About {} triangles ({built}). Low 1000, Medium 2000 and High 4000 suit about 20,000 sq ft.",
            self.draft.terrain.estimated_triangles()
        ));
        self.fields.length_row(
            ui,
            "Contour interval",
            "interval_general",
            &mut self.draft.contour_interval,
        );
        self.fields.degrees_row(
            ui,
            "North angle",
            "deg_north",
            &mut self.draft.terrain.north_angle,
        );
        ui.weak("Degrees clockwise from the top of the plan to true north (North Pointer).");
        row(ui, "Season", |ui| {
            egui::ComboBox::from_id_salt("terrain_season")
                .selected_text(self.draft.terrain.season.name())
                .show_ui(ui, |ui| {
                    for s in Season::ALL {
                        ui.selectable_value(&mut self.draft.terrain.season, s, s.name());
                    }
                })
        });
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
        self.fields.length_row(
            ui,
            "Offset",
            "contour_offset",
            &mut self.draft.terrain.contour_offset,
        );
        ui.weak(
            "Shifts which elevation gets a contour: lines fall at the offset plus whole intervals.",
        );
        ui.weak(format!(
            "Secondary contours every {}, primary contours every {}.",
            fmt_short(self.draft.contour_interval),
            fmt_short(
                self.draft.contour_interval * f64::from(self.draft.terrain.contour_major_every)
            )
        ));
        row(ui, "Major contour every", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.draft.terrain.contour_major_every)
                    .range(1..=MAX_MAJOR_EVERY)
                    .suffix(" contours"),
            )
        });
        ui.checkbox(
            &mut self.draft.terrain.contour_smoothing,
            "Smooth the contour lines",
        );
        if self.draft.terrain.contour_smoothing {
            row(ui, "Smoothing passes", |ui| {
                ui.add(
                    egui::DragValue::new(&mut self.draft.terrain.contour_smooth_passes)
                        .range(1..=6),
                )
            });
        }
        self.fields.length_row(
            ui,
            "Label spacing",
            "label_spacing",
            &mut self.draft.terrain.contour_label_spacing,
        );
        ui.weak("Elevation text along each labeled contour; 0 puts one label on each line.");
        row(ui, "Label units", |ui| {
            egui::ComboBox::from_id_salt("contour_label_units")
                .selected_text(self.draft.terrain.contour_label_units.name())
                .show_ui(ui, |ui| {
                    for u in LabelUnits::ALL {
                        ui.selectable_value(
                            &mut self.draft.terrain.contour_label_units,
                            u,
                            u.name(),
                        );
                    }
                })
        });
        ui.checkbox(
            &mut self.draft.terrain.highlight_negative,
            "Highlight negative elevations (labels below 0 in red)",
        );
        ui.add_space(6.0);
        section(ui, "Primary lines (the major contours)");
        contour_style_rows(
            ui,
            &mut self.draft.terrain.contour_primary,
            PRIMARY_COLOR,
            PRIMARY_CONTOUR_WEIGHT,
        );
        ui.checkbox(
            &mut self.draft.terrain.label_primary,
            "Label primary contours with their elevation",
        );
        ui.weak("Drawn on the \"Terrain, Primary Contours\" layer.");
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
            .checkbox(
                &mut labeled,
                "Label secondary contours with their elevation",
            )
            .changed()
        {
            self.draft.terrain.contour_label_major_only = !labeled;
        }
        ui.weak("Drawn on the \"Terrain, Secondary Contours\" layer.");
    }

    /// Polyline panel of the perimeter: length, area and lines.
    fn polyline(&mut self, ui: &mut Ui) {
        section(ui, "Polyline");
        let pts = &self.draft.terrain.perimeter;
        if pts.len() < 3 {
            ui.weak("Draw the Terrain Perimeter first.");
            return;
        }
        let length: f64 = (0..pts.len())
            .map(|i| pts[i].dist(pts[(i + 1) % pts.len()]))
            .sum();
        row(ui, "Perimeter", |ui| ui.label(fmt_short(length)));
        row(ui, "Area", |ui| {
            ui.label(format!(
                "{:.0} sq ft",
                plan_core::geometry::polygon_area(pts).abs() / 144.0
            ))
        });
        row(ui, "Number of Lines", |ui| ui.label(pts.len().to_string()));
        let holes: f64 = self
            .draft
            .terrain
            .features
            .iter()
            .filter(|f| f.kind == plan_terrain::FeatureKind::Hole)
            .map(|f| plan_core::geometry::polygon_area(&f.polygon).abs() / 144.0)
            .sum();
        if holes > 0.0 {
            ui.weak(format!(
                "Terrain holes take {holes:.0} sq ft out of the area."
            ));
        }
    }

    /// Line Style page of the perimeter: its own color, weight and dashes.
    fn line_style(&mut self, ui: &mut Ui) {
        section(ui, "Line Style");
        let st = &mut self.draft.terrain.perimeter_extras.style;
        color_row(ui, &mut st.line_color, PERIMETER_COLOR);
        row(ui, "Line weight", |ui| {
            ui.add(
                egui::DragValue::new(&mut st.line_weight)
                    .range(0.0..=8.0)
                    .speed(0.05)
                    .suffix(" pt"),
            )
        });
        ui.weak("0 takes the perimeter's own weight (1.5 pt).");
        ui.checkbox(&mut st.dashed, "Dashed");
    }

    /// Fill Style page of the perimeter: the ground inside it in plan.
    fn fill_style(&mut self, ui: &mut Ui) {
        use plan_terrain::FillStyle;
        section(ui, "Fill Style");
        let st = &mut self.draft.terrain.perimeter_extras.style;
        ui.radio_value(&mut st.fill, FillStyle::Default, "No fill (outline only)");
        ui.radio_value(&mut st.fill, FillStyle::Solid, "Solid");
        ui.radio_value(&mut st.fill, FillStyle::Hatch, "Hatch");
        let mut own = st.fill_color.is_some();
        if ui
            .checkbox(&mut own, "Use a fill color of its own")
            .changed()
        {
            st.fill_color = own.then_some([79, 143, 58]);
        }
        if let Some(c) = &mut st.fill_color {
            row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
        }
    }

    fn label(&mut self, ui: &mut Ui) {
        let auto = plan_terrain::auto_label(&self.draft.terrain, ObjectKey::Perimeter);
        let t = &mut self.draft.terrain;
        panels::label_panel(ui, &mut self.fields, &mut t.perimeter_extras, &auto);
    }

    fn info(&mut self, ui: &mut Ui) {
        panels::info_panel(ui, &mut self.draft.terrain.perimeter_extras);
    }

    fn schedule(&mut self, ui: &mut Ui) {
        panels::schedule_panel(
            ui,
            &mut self.draft.terrain.perimeter_extras,
            Some(plan_terrain::ScheduleCategory::TerrainPerimeter),
        );
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
        row(ui, "Skirt", |ui| {
            material_combo(
                ui,
                "terrain_skirt",
                &mut self.draft.terrain.skirt.material,
                &DIRT_MATERIALS,
            )
        });
        ui.weak(
            "The ground material colors the terrain surface in 3D; the bare ground shows on \
             the cut slopes of graded pads; the skirt hangs from the terrain edge. These \
             materials are not counted in the Materials List.",
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
            Mode::Import => assistants::IMPORT_TABS,
            Mode::ImportGps => assistants::GPS_TABS,
            Mode::Grow => assistants::GROW_TABS,
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
            "Polyline" => self.polyline(ui),
            "Line Style" => self.line_style(ui),
            "Fill Style" => self.fill_style(ui),
            "Building Pad" => self.building_pad(ui),
            "Materials" => self.materials(ui),
            "Label" => self.label(ui),
            "Components" => panels::components_panel(ui, &mut self.draft.terrain.perimeter_extras),
            "Object Information" => self.info(ui),
            "Schedule" => self.schedule(ui),
            "Layer" => self.layer(ui),
            "Cut and Fill" => self.cut_fill(ui),
            "Select File" if self.mode == Mode::Import => self.import.select_file(ui),
            "Select File" => self.gps.select_file(ui),
            "Filter Data" => self.import.filter_data(ui),
            "Scale Data" => self
                .import
                .scale_data(ui, &mut self.fields, &mut self.draft),
            "Import As" => self.gps.import_as(ui),
            "Transform Coordinates" => self.gps.transform(ui, &mut self.fields, &mut self.draft),
            "Grow Plants" => self.grow.page(ui, &mut self.draft),
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
            [
                "General",
                "Contours",
                "Polyline",
                "Line Style",
                "Fill Style",
                "Building Pad",
                "Materials",
                "Label",
                "Components",
                "Object Information",
                "Schedule",
                "Layer"
            ]
        );
        assert!(spec.stores());
        let report = TerrainDialog::cut_fill(&rec);
        assert_eq!(report.tab_names(), ["Cut and Fill"]);
        assert!(!report.stores(), "the report stores nothing");
        assert_eq!(report.report().items.len(), 1);
        let import = TerrainDialog::import(&rec);
        assert_eq!(
            import.tab_names(),
            ["Select File", "Filter Data", "Scale Data"]
        );
        assert!(
            !import.stores(),
            "an import that added nothing stores nothing"
        );
        let gps = TerrainDialog::import_gps(&rec);
        assert_eq!(
            gps.tab_names(),
            ["Select File", "Import As", "Transform Coordinates"]
        );
        assert!(!gps.stores());
        let grow = TerrainDialog::grow(&rec);
        assert_eq!(grow.tab_names(), ["Grow Plants"]);
        assert!(!grow.stores());
        for mut d in [spec, report, import, gps, grow] {
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
    fn the_import_assistant_reads_the_chosen_columns_filters_scales_and_makes_a_perimeter() {
        let mut rec = TerrainRecord::new();
        assert!(!rec.has_perimeter());
        let mut dlg = TerrainDialog::import(&rec);
        // A survey with a header, point numbers and Y before X, in meters.
        dlg.set_import_text("ID,N,E,Z\n1,10,20,3\n2,30,40,5\n3,50,60,7\n4,70,80,900\n");
        {
            let st = dlg.import_mut();
            st.layout.order = plan_terrain::ColumnOrder::NYxz;
            st.layout.skip_lines = 1;
            st.scale = plan_terrain::ScaleOptions::uniform(plan_terrain::ImportUnit::Meters);
        }
        // Draw every page of the assistant with its filter step live.
        draw_all_pages(&mut dlg);
        let msg = dlg.press_add().unwrap().unwrap();
        assert!(msg.contains("4 points read"), "{msg}");
        assert!(
            msg.contains("a perimeter was made around the data"),
            "{msg}"
        );
        let t = &dlg.draft().terrain;
        assert_eq!(t.elevation_points.len(), 4);
        assert!(t.perimeter.len() >= 3);
        // YXZ: the first column is Y, so the first point is at x = 20 m, y = 10 m.
        let first = &t.elevation_points[0];
        assert!(
            (first.pos.x - 20.0 / 0.0254).abs() < 1e-6
                && (first.pos.y - 10.0 / 0.0254).abs() < 1e-6
        );
        assert!(dlg.stores());
        // OK stores the new perimeter along with the points.
        rec.apply_spec(dlg.draft());
        assert!(rec.has_perimeter());
        assert_eq!(rec.terrain.elevation_points.len(), 4);
    }

    #[test]
    fn the_gps_assistant_adds_way_points_and_a_perimeter_and_keeps_its_markers() {
        let rec = graded_record();
        let mut dlg = TerrainDialog::import_gps(&rec);
        assert!(dlg.press_gps_import().is_none() || dlg.gps_result().is_none());
        dlg.set_gps_text(
            r#"<gpx version="1.1"><wpt lat="40.0" lon="-75.0"><ele>100</ele><name>A</name></wpt>
            <wpt lat="40.0005" lon="-75.0"><ele>102</ele></wpt>
            <trk><trkseg><trkpt lat="40.0" lon="-75.0"/><trkpt lat="40.0005" lon="-75.0"/>
            <trkpt lat="40.0005" lon="-74.9995"/></trkseg></trk></gpx>"#,
        );
        draw_all_pages(&mut dlg);
        // Import As: way points as elevation data, the track as the perimeter.
        dlg.form.gps.tracks_as = plan_terrain::GpsImportAs::Perimeter;
        let msg = dlg.press_gps_import().unwrap().unwrap();
        assert!(msg.contains("2 elevation points"), "{msg}");
        let result = dlg.gps_result().expect("a result");
        assert_eq!(result.perimeter.len(), 3);
        assert_eq!(dlg.draft().terrain.elevation_points.len(), 2);
        assert_eq!(dlg.draft().terrain.perimeter.len(), 3);
        assert!(dlg.stores());
    }

    #[test]
    fn grow_plants_scales_the_draft_and_stores_only_when_changed() {
        let mut rec = graded_record();
        let mut run = plan_terrain::Landscape::new(
            plan_terrain::LandscapeKind::Plants,
            plan_terrain::ShapeKind::Polyline,
            vec![Point::new(0.0, 0.0), Point::new(200.0, 0.0)],
        );
        run.height = 240.0;
        run.size = 120.0;
        run.mature_height = 240.0;
        run.mature_width = 120.0;
        run.maturity_months = 240.0;
        rec.terrain.landscape.push(run);
        let mut dlg = TerrainDialog::grow(&rec);
        assert!(!dlg.stores());
        draw_all_pages(&mut dlg);
        dlg.set_grow_years(0.0);
        assert!(dlg.stores());
        assert!(dlg.draft().terrain.landscape[0].height < 240.0);
        dlg.set_grow_years(20.0);
        assert_eq!(dlg.draft().terrain.landscape[0].height, 240.0);
        assert!(!dlg.stores(), "back at its mature size nothing changed");
    }

    #[test]
    fn the_general_page_edits_absolute_elevation_skirt_smoothing_and_triangles() {
        let mut dlg = TerrainDialog::new(&graded_record());
        {
            let t = &mut dlg.draft_mut().terrain;
            t.absolute_elevation = AbsoluteElevation::ReferencePoint;
            t.reference_point = Some(Point::new(10.0, 20.0));
            t.surface_offset = -9.0;
            t.skirt.enabled = true;
            t.skirt.mode = SkirtMode::FollowTerrain;
            t.smoothing_level = SmoothingLevel::Medium;
            t.triangle_detail = TriangleDetail::Custom;
            t.custom_triangles = 800;
            t.hide_under_building = true;
            t.season = Season::Winter;
            t.contour_label_units = LabelUnits::DecimalFeet;
            t.perimeter_extras.label.shown = true;
        }
        draw_all_pages(&mut dlg);
        let mut stored = graded_record();
        stored.apply_spec(dlg.draft());
        let t = &stored.terrain;
        assert_eq!(t.absolute_elevation, AbsoluteElevation::ReferencePoint);
        assert_eq!(t.reference_point, Some(Point::new(10.0, 20.0)));
        assert_eq!(
            (t.surface_offset, t.skirt.enabled, t.skirt.mode),
            (-9.0, true, SkirtMode::FollowTerrain)
        );
        assert_eq!(
            (t.smoothing_level, t.triangle_detail, t.custom_triangles),
            (SmoothingLevel::Medium, TriangleDetail::Custom, 800)
        );
        assert!(t.hide_under_building && t.season == Season::Winter);
        assert_eq!(t.contour_label_units, LabelUnits::DecimalFeet);
        assert!(t.perimeter_extras.label.shown);
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
