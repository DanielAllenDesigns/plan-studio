//! The Import Terrain Assistant, the Import GPS Data Assistant and the Grow
//! Plants dialog (manual pp. 1341-1347, 1358). They are pages of
//! [`super::TerrainDialog`]: each assistant step is a tab, and the last step
//! holds the button that adds the result to the draft.

use super::super::{fmt_short, on, row, section, Fields, Tab};
use crate::editor::site_view::TerrainRecord;
use eframe::egui::{self, Color32, Ui};
use plan_core::Point;
use plan_terrain::{
    filter_points, grow_plants, import_gps, import_points, import_terrain_text, ranges_of,
    read_columns, scale_points, ColumnOrder, DataRanges, Delimiter, GpsImportAs, GpsResult,
    GpsTransform, ImportFormat, ImportUnit, ImportedPoints, RangeFilter, RawPoint, ScaleOptions,
    TerrainImport, TextLayout, MANY_POINTS,
};

pub(super) const IMPORT_TABS: &[Tab] = &[on("Select File"), on("Filter Data"), on("Scale Data")];
pub(super) const GPS_TABS: &[Tab] = &[
    on("Select File"),
    on("Import As"),
    on("Transform Coordinates"),
];
pub(super) const GROW_TABS: &[Tab] = &[on("Grow Plants")];

/// What the pasted or loaded text is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Source {
    Empty,
    Dxf,
    Gpx,
    Text,
}

pub(super) fn detect(text: &str) -> Source {
    let head: String = text.chars().take(4096).collect::<String>().to_lowercase();
    if text.trim().is_empty() {
        Source::Empty
    } else if head.contains("<gpx") {
        Source::Gpx
    } else if head.lines().map(str::trim).filter(|l| !l.is_empty()).take(2).collect::<Vec<_>>()
        == ["0", "section"]
    {
        Source::Dxf
    } else {
        Source::Text
    }
}

fn red(ui: &mut Ui, text: &str) {
    ui.colored_label(Color32::from_rgb(0xB0, 0x30, 0x30), text);
}

/// A min/max limit with an on/off box.
#[derive(Clone, Copy, Default)]
struct Limit {
    on: bool,
    lo: f64,
    hi: f64,
}

impl Limit {
    fn range(&self) -> Option<(f64, f64)> {
        self.on.then_some((self.lo, self.hi))
    }
}

fn limit_row(ui: &mut Ui, label: &str, limit: &mut Limit, data: (f64, f64)) {
    ui.horizontal(|ui| {
        if ui.checkbox(&mut limit.on, label).changed() && limit.on && limit.lo == limit.hi {
            (limit.lo, limit.hi) = data;
        }
        ui.add_enabled_ui(limit.on, |ui| {
            ui.add(egui::DragValue::new(&mut limit.lo).speed(1.0).prefix("from "));
            ui.add(egui::DragValue::new(&mut limit.hi).speed(1.0).prefix("to "));
        });
    });
    ui.weak(format!("The file runs {:.2} to {:.2}.", data.0, data.1));
}

// ===================================================================
// Import Terrain Assistant
// ===================================================================

/// State of the Import Terrain Assistant.
pub(super) struct ImportState {
    pub text: String,
    pub layout: TextLayout,
    limit_x: Limit,
    limit_y: Limit,
    limit_z: Limit,
    thin: bool,
    max_points: usize,
    pub scale: ScaleOptions,
    map: bool,
    map_x: f64,
    map_y: f64,
    /// Create a perimeter around the data when the terrain has none.
    pub create_perimeter: bool,
    /// Center the points on the terrain perimeter.
    pub center: bool,
    /// Lower the points so the lowest is at elevation 0.
    pub zero_lowest: bool,
    /// What the last Add did: a summary, or why it failed.
    pub message: Option<Result<String, String>>,
}

impl ImportState {
    pub fn new(has_perimeter: bool) -> Self {
        ImportState {
            text: String::new(),
            layout: TextLayout::default(),
            limit_x: Limit::default(),
            limit_y: Limit::default(),
            limit_z: Limit::default(),
            thin: false,
            max_points: 1000,
            scale: ScaleOptions::uniform(ImportUnit::Auto),
            map: false,
            map_x: 0.0,
            map_y: 0.0,
            create_perimeter: true,
            center: has_perimeter,
            zero_lowest: false,
            message: None,
        }
    }

    fn filter(&self) -> RangeFilter {
        RangeFilter {
            x: self.limit_x.range(),
            y: self.limit_y.range(),
            z: self.limit_z.range(),
            max_points: self.thin.then_some(self.max_points.max(1)),
        }
    }

    fn scale_options(&self) -> ScaleOptions {
        ScaleOptions {
            map_to_origin: self.map.then_some((self.map_x, self.map_y)),
            ..self.scale
        }
    }

    /// The points of the text as the layout reads them (text files only).
    fn raw(&self) -> (Vec<RawPoint>, usize) {
        read_columns(&self.text, &self.layout)
    }

    pub fn select_file(&mut self, ui: &mut Ui) {
        section(ui, "Select File");
        ui.weak(
            "Survey points from a DXF drawing, a GPX file (way points) or text: .txt, .csv, \
             .prn, .xyz, .nez, .auf.",
        );
        ui.add_space(4.0);
        if ui.button("Choose File\u{2026}").clicked() {
            self.choose_file();
        }
        ui.label("Or paste the text here:");
        ui.add(
            egui::TextEdit::multiline(&mut self.text)
                .desired_rows(8)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
        let source = detect(&self.text);
        match source {
            Source::Empty => {
                ui.weak("Nothing read yet.");
            }
            Source::Dxf => {
                ui.label("A DXF drawing: points, 3D faces and polyline vertices are read.");
            }
            Source::Gpx => {
                ui.label("A GPX file: way points carry elevation; route points are ignored.");
            }
            Source::Text => {
                row(ui, "Data organization", |ui| {
                    egui::ComboBox::from_id_salt("import_order")
                        .selected_text(self.layout.order.name())
                        .show_ui(ui, |ui| {
                            for o in ColumnOrder::ALL {
                                ui.selectable_value(&mut self.layout.order, o, o.name());
                            }
                        })
                });
                row(ui, "Delimiter", |ui| {
                    egui::ComboBox::from_id_salt("import_delim")
                        .selected_text(self.layout.delimiter.name())
                        .show_ui(ui, |ui| {
                            for d in Delimiter::ALL {
                                ui.selectable_value(&mut self.layout.delimiter, d, d.name());
                            }
                        })
                });
                row(ui, "Header lines to skip", |ui| {
                    ui.add(egui::DragValue::new(&mut self.layout.skip_lines).range(0..=50))
                });
                let (pts, skipped) = self.raw();
                ui.weak(format!(
                    "{} points read{}",
                    pts.len(),
                    if skipped > 0 {
                        format!("; {skipped} lines skipped")
                    } else {
                        String::new()
                    }
                ));
                if let Some(first) = pts.first() {
                    ui.weak(format!(
                        "First point: X {} Y {} Z {}",
                        first.x, first.y, first.z
                    ));
                }
            }
        }
    }

    /// Reads a file into the text box.
    fn choose_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Survey points",
                &["dxf", "gpx", "txt", "csv", "prn", "xyz", "auf", "nez", "pts"],
            )
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                self.text = text;
                self.message = None;
            }
            Err(e) => {
                self.message = Some(Err(format!("Could not read {}: {e}", path.display())));
            }
        }
    }

    pub fn filter_data(&mut self, ui: &mut Ui) {
        section(ui, "Filter Data");
        if detect(&self.text) != Source::Text {
            ui.weak("Range limits and thinning apply to text files; DXF and GPX are read whole.");
            return;
        }
        let (pts, _) = self.raw();
        let Some(r) = ranges_of(&pts) else {
            ui.weak("No points to filter yet: pick a file and its data organization first.");
            return;
        };
        ui.label(format!("{} points in the file", r.count));
        limit_row(ui, "Limit X", &mut self.limit_x, r.x);
        limit_row(ui, "Limit Y", &mut self.limit_y, r.y);
        limit_row(ui, "Limit Z", &mut self.limit_z, r.z);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.thin, "Reduce to");
            ui.add_enabled(
                self.thin,
                egui::DragValue::new(&mut self.max_points)
                    .range(10..=100_000)
                    .suffix(" evenly spread points"),
            );
        });
        let kept = filter_points(&pts, &self.filter()).len();
        ui.label(format!("{kept} points will be imported"));
        if kept > MANY_POINTS {
            red(
                ui,
                "More than 2000 points make the terrain slow to build: reduce the count.",
            );
        }
    }

    pub fn scale_data(&mut self, ui: &mut Ui, fields: &mut Fields, draft: &mut TerrainRecord) {
        section(ui, "Scale Data");
        let axis = |ui: &mut Ui, label: &str, salt: &str, unit: &mut ImportUnit| {
            row(ui, label, |ui| {
                egui::ComboBox::from_id_salt(salt)
                    .selected_text(unit.name())
                    .show_ui(ui, |ui| {
                        for u in ImportUnit::ALL {
                            ui.selectable_value(unit, u, u.name());
                        }
                    })
            });
        };
        axis(ui, "X units", "import_ux", &mut self.scale.unit_x);
        axis(ui, "Y units", "import_uy", &mut self.scale.unit_y);
        axis(ui, "Z units", "import_uz", &mut self.scale.unit_z);
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.map, "Map this file point to the plan origin");
            ui.add_enabled_ui(self.map, |ui| {
                ui.add(egui::DragValue::new(&mut self.map_x).speed(1.0).prefix("X "));
                ui.add(egui::DragValue::new(&mut self.map_y).speed(1.0).prefix("Y "));
            });
        });
        row(ui, "Relief scale", |ui| {
            ui.add(
                egui::DragValue::new(&mut self.scale.relief_scale)
                    .range(0.01..=100.0)
                    .speed(0.01),
            )
        });
        fields.degrees_row(ui, "Rotate north counterclockwise", "deg_import_rot", &mut self.scale.rotate_ccw);
        ui.add_space(4.0);
        ui.add_enabled(
            draft.has_perimeter(),
            egui::Checkbox::new(&mut self.center, "Center the points on the terrain perimeter"),
        );
        ui.checkbox(&mut self.zero_lowest, "Make the lowest point elevation 0");
        ui.checkbox(
            &mut self.create_perimeter,
            "Create a terrain perimeter around the data when there is none",
        );
        ui.add_space(4.0);
        if ui
            .add_enabled(
                !self.text.trim().is_empty(),
                egui::Button::new("Add to terrain"),
            )
            .clicked()
        {
            self.add(draft);
        }
        match &self.message {
            Some(Ok(m)) => {
                ui.label(m);
            }
            Some(Err(e)) => red(ui, e),
            None => {}
        }
        ui.weak(format!(
            "{} elevation points in the terrain",
            draft.terrain.elevation_points.len()
        ));
    }

    /// Parses the text and adds its points to the draft's elevation data.
    pub fn add(&mut self, draft: &mut TerrainRecord) {
        let result = self.read().map(|mut got| {
            if self.center && draft.has_perimeter() {
                got.center_on(perimeter_center(&draft.terrain.perimeter));
            }
            if self.zero_lowest {
                got.zero_lowest();
            }
            got
        });
        self.message = Some(result.map(|got: ImportedPoints| {
            let before = draft.terrain.perimeter.len();
            let added = draft
                .terrain
                .import_elevation_points(&got.points, self.create_perimeter);
            let mut s = format!("{}; {added} added", got.summary());
            if before < 3 && draft.terrain.perimeter.len() >= 3 {
                s.push_str("; a perimeter was made around the data");
            }
            s
        }));
    }

    /// The points the three steps make from the text.
    fn read(&self) -> Result<ImportedPoints, String> {
        match detect(&self.text) {
            Source::Empty => Err("Nothing to import: pick a file or paste the data".into()),
            Source::Text => import_terrain_text(
                &self.text,
                &TerrainImport {
                    layout: self.layout,
                    filter: self.filter(),
                    scale: self.scale_options(),
                },
            )
            .map(|mut got| {
                got.format = ImportFormat::Xyz;
                got
            }),
            Source::Dxf | Source::Gpx => {
                // Their layout is fixed: read, then rotate, scale the relief
                // and map the origin (in inches).
                let mut got = import_points(&self.text, self.scale.unit_x)?;
                let raw: Vec<RawPoint> = got
                    .points
                    .iter()
                    .map(|p| RawPoint {
                        x: p.pos.x,
                        y: p.pos.y,
                        z: p.z,
                        number: String::new(),
                        description: String::new(),
                    })
                    .collect();
                let mut opts = ScaleOptions::uniform(ImportUnit::Inches);
                opts.relief_scale = self.scale.relief_scale;
                opts.rotate_ccw = self.scale.rotate_ccw;
                if self.map {
                    opts.map_to_origin =
                        Some((self.map_x * self.scale.unit_x.inches(), self.map_y * self.scale.unit_y.inches()));
                }
                got.points = scale_points(&raw, &opts);
                Ok(got)
            }
        }
    }
}

/// The middle of the perimeter's bounding box.
pub(super) fn perimeter_center(perimeter: &[Point]) -> Point {
    let (mut lo, mut hi) = (perimeter[0], perimeter[0]);
    for q in perimeter {
        lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
        hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
    }
    Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0)
}

// ===================================================================
// Import GPS Data Assistant
// ===================================================================

/// State of the Import GPS Data Assistant.
pub(super) struct GpsState {
    pub text: String,
    pub waypoints_as: GpsImportAs,
    pub tracks_as: GpsImportAs,
    pub transform: GpsTransform,
    set_origin: bool,
    pub message: Option<Result<String, String>>,
    /// What the last Import made (applied to the plan on OK).
    pub result: Option<GpsResult>,
}

impl GpsState {
    pub fn new() -> Self {
        GpsState {
            text: String::new(),
            waypoints_as: GpsImportAs::ElevationData,
            tracks_as: GpsImportAs::Polyline,
            transform: GpsTransform::default(),
            set_origin: false,
            message: None,
            result: None,
        }
    }

    pub fn select_file(&mut self, ui: &mut Ui) {
        section(ui, "Select File");
        ui.weak("A GPS Exchange file (.gpx, version 1.1).");
        if ui.button("Choose File\u{2026}").clicked() {
            if let Some(path) = rfd::FileDialog::new().add_filter("GPX", &["gpx"]).pick_file() {
                match std::fs::read_to_string(&path) {
                    Ok(t) => {
                        self.text = t;
                        self.message = None;
                    }
                    Err(e) => {
                        self.message =
                            Some(Err(format!("Could not read {}: {e}", path.display())));
                    }
                }
            }
        }
        ui.label("Or paste the text here:");
        ui.add(
            egui::TextEdit::multiline(&mut self.text)
                .desired_rows(8)
                .desired_width(f32::INFINITY)
                .font(egui::TextStyle::Monospace),
        );
        if let Ok(pts) = plan_terrain::parse_gpx_points(&self.text) {
            use plan_terrain::GpsKind::{Route, Track, Way};
            let n = |k| pts.iter().filter(|p| p.kind == k).count();
            ui.weak(format!(
                "{} way points, {} track points, {} route points (ignored)",
                n(Way),
                n(Track),
                n(Route)
            ));
        } else if !self.text.trim().is_empty() {
            red(ui, "This is not a GPX file.");
        }
    }

    pub fn import_as(&mut self, ui: &mut Ui) {
        section(ui, "Import As");
        let combo = |ui: &mut Ui, label: &str, salt: &str, value: &mut GpsImportAs, elevation: bool| {
            row(ui, label, |ui| {
                egui::ComboBox::from_id_salt(salt)
                    .selected_text(value.name())
                    .show_ui(ui, |ui| {
                        for a in GpsImportAs::ALL {
                            if a == GpsImportAs::ElevationData && !elevation {
                                continue;
                            }
                            ui.selectable_value(value, a, a.name());
                        }
                    })
            });
        };
        combo(ui, "Way points", "gps_way", &mut self.waypoints_as, true);
        combo(ui, "Track points", "gps_track", &mut self.tracks_as, false);
        ui.weak(
            "Way points carry elevation. Track points do not: they become markers, a polyline \
             or the terrain perimeter. Route points are ignored.",
        );
    }

    pub fn transform(&mut self, ui: &mut Ui, fields: &mut Fields, draft: &mut TerrainRecord) {
        section(ui, "Transform Coordinates");
        fields.length_row(ui, "Lower elevation data by", "gps_lower", &mut self.transform.lower_by);
        fields.degrees_row(ui, "Rotate north counterclockwise", "deg_gps_rot", &mut self.transform.rotate_ccw);
        if ui
            .checkbox(&mut self.set_origin, "Map a latitude and longitude to the plan origin")
            .changed()
        {
            self.transform.origin = self.set_origin.then_some((0.0, 0.0));
        }
        if let Some((lat, lon)) = self.transform.origin.as_mut() {
            row(ui, "Latitude", |ui| {
                ui.add(egui::DragValue::new(lat).range(-90.0..=90.0).speed(0.0001).max_decimals(6))
            });
            row(ui, "Longitude", |ui| {
                ui.add(egui::DragValue::new(lon).range(-180.0..=180.0).speed(0.0001).max_decimals(6))
            });
        } else {
            ui.weak("The first point is the origin.");
        }
        ui.add_space(4.0);
        if ui
            .add_enabled(!self.text.trim().is_empty(), egui::Button::new("Import"))
            .clicked()
        {
            self.run(draft);
        }
        match &self.message {
            Some(Ok(m)) => {
                ui.label(m);
            }
            Some(Err(e)) => red(ui, e),
            None => {}
        }
    }

    /// Runs the assistant and adds the elevation data and perimeter to the draft.
    pub fn run(&mut self, draft: &mut TerrainRecord) {
        match import_gps(&self.text, self.waypoints_as, self.tracks_as, &self.transform) {
            Ok(got) => {
                let added = draft.terrain.add_elevation_points(&got.elevation_points);
                if got.perimeter.len() >= 3 {
                    draft.terrain.perimeter = got.perimeter.clone();
                }
                self.message = Some(Ok(format!("{}; {added} elevation points added", got.summary())));
                self.result = Some(got);
            }
            Err(e) => {
                self.message = Some(Err(e));
                self.result = None;
            }
        }
    }
}

// ===================================================================
// Grow Plants
// ===================================================================

/// State of the Grow Plants dialog.
pub(super) struct GrowState {
    pub years: f64,
    pub changed: usize,
    pub plants: usize,
}

impl GrowState {
    pub fn new(draft: &TerrainRecord) -> Self {
        GrowState {
            years: 0.0,
            changed: 0,
            plants: draft
                .terrain
                .landscape
                .iter()
                .filter(|l| l.kind == plan_terrain::LandscapeKind::Plants && l.mature_height > 0.0)
                .count(),
        }
    }

    pub fn page(&mut self, ui: &mut Ui, draft: &mut TerrainRecord) {
        section(ui, "Grow Plants");
        ui.label("Age of the plants since they were planted");
        let resp = ui.add(
            egui::Slider::new(&mut self.years, 0.0..=20.0)
                .step_by(0.5)
                .suffix(" years"),
        );
        if resp.changed() {
            self.changed = grow_plants(&mut draft.terrain.landscape, self.years);
        }
        ui.weak(format!(
            "{} plant runs have a mature size and grow with the slider; the others keep their size.",
            self.plants
        ));
        if let Some(l) = draft
            .terrain
            .landscape
            .iter()
            .find(|l| l.kind == plan_terrain::LandscapeKind::Plants && l.mature_height > 0.0)
        {
            ui.weak(format!(
                "A sample run is now {} tall and {} wide (mature: {} by {}).",
                fmt_short(l.height),
                fmt_short(l.size),
                fmt_short(l.mature_height),
                fmt_short(l.mature_width)
            ));
        }
    }
}

/// Counts for the Filter Data step, for tests.
#[allow(dead_code)]
pub(super) fn ranges(text: &str, layout: &TextLayout) -> Option<DataRanges> {
    ranges_of(&read_columns(text, layout).0)
}
