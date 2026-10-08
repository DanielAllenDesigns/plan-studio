//! Specification of one terrain object (double-click it with a Terrain tool):
//! terrain feature (rectangular, kidney, spline, polyline, round) and hole,
//! break line, terrain wall or curb, garden bed, grass region, water feature,
//! stepping stones, plant run (with the Plant Chooser), sprinklers, road,
//! driveway, sidewalk and road marking, and the elevation point, line and
//! region and the terrain modifiers (Hill, Valley, Raised, Lowered and Flat
//! Region). Pages: General (sizes, heights, material, spacing), Line Style,
//! Fill Style (regions and walls) and Layer. The dialog edits a
//! [`TerrainObject`] draft the tool stores on OK.

use super::super::{
    fmt_short, on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_FAINT,
    PV_INK,
};
use crate::editor::site_view::TerrainObject;
use crate::tools::terrain::{
    apply_plant, plant_categories, plant_choices, plants_in, SPLINE_SAMPLES,
};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::Point;
use plan_terrain::{
    FeatureKind, FillStyle, Landscape, LandscapeKind, ModifierKind, ObjectStyle, PlantForm,
    RoadKind, WallKind,
};

const GENERAL_ONLY: &[Tab] = &[on("General")];
const ROAD_TABS: &[Tab] = &[on("General"), on("Layer")];
const PATHS: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];
const REGIONS: &[Tab] = &[
    on("General"),
    on("Line Style"),
    on("Fill Style"),
    on("Layer"),
];
const MATERIALS: [&str; 4] = ["Concrete", "Stone", "Brick", "Grass"];
const ROAD_MATERIALS: [&str; 5] = ["Asphalt", "Concrete", "Gravel", "Brick", "Stone"];
/// Plants listed at a time in the Plant Chooser.
const CHOOSER_HEIGHT: f32 = 150.0;
const BED_MATERIALS: [&str; 3] = ["Mulch", "Soil", "Stone"];
const MIN_SIZE: f64 = 1.0;

pub struct ObjectDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: TerrainObject,
    fields: Fields,
    chooser: Chooser,
}

/// State of the Plant Chooser list of the Plant Specification.
#[derive(Default)]
struct Chooser {
    open: bool,
    /// Category shown (empty = all).
    category: String,
    search: String,
}

impl ObjectDialog {
    pub fn new(obj: TerrainObject) -> Self {
        let title = obj.title();
        Self {
            frame: SpecDialog::new(title, ("terrain_object", title)),
            form: Form {
                draft: obj,
                fields: Fields::default(),
                chooser: Chooser::default(),
            },
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Outcome {
        self.frame.show(ctx, &mut self.form)
    }

    pub fn draft(&self) -> &TerrainObject {
        &self.form.draft
    }

    #[cfg(test)]
    pub fn draft_mut(&mut self) -> &mut TerrainObject {
        &mut self.form.draft
    }

    #[cfg(test)]
    pub fn error(&self) -> Option<String> {
        self.form.error()
    }

    #[cfg(test)]
    pub fn tab_names(&self) -> Vec<&'static str> {
        self.form.tabs().iter().map(|t| t.name).collect()
    }
}

fn material_combo(ui: &mut Ui, salt: &str, value: &mut String, options: &[&str]) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for o in options {
                ui.selectable_value(value, (*o).to_string(), *o);
            }
        });
}

impl Form {
    fn style_mut(&mut self) -> Option<&mut ObjectStyle> {
        match &mut self.draft {
            TerrainObject::Feature(f) if f.kind != FeatureKind::Hole => Some(&mut f.style),
            TerrainObject::Break(b) => Some(&mut b.style),
            TerrainObject::Wall(w) => Some(&mut w.style),
            TerrainObject::Landscape(l) => Some(&mut l.style),
            _ => None,
        }
    }

    fn default_layer(&self) -> String {
        match &self.draft {
            TerrainObject::Feature(_) => plan_terrain::landscape::LAYER_FEATURES.into(),
            TerrainObject::Break(_) => plan_terrain::landscape::LAYER_BREAKS.into(),
            TerrainObject::Wall(w) => w.default_layer().into(),
            TerrainObject::Landscape(l) => l.default_layer().into(),
            _ => "Terrain".into(),
        }
    }

    fn general(&mut self, ui: &mut Ui) {
        let fields = &mut self.fields;
        match &mut self.draft {
            TerrainObject::Feature(f) if f.kind == FeatureKind::Hole => {
                section(ui, "General");
                ui.label("The terrain surface is cut away inside the outline.");
                ui.weak("Terrain Holes have no other settings; delete the hole to fill it.");
            }
            TerrainObject::Feature(f) => {
                section(ui, "General");
                row(ui, "Shape", |ui| {
                    ui.label(format!("{} outline", f.kind_name()))
                });
                if f.kind == FeatureKind::Round {
                    let before = f.radius;
                    fields.length_row(ui, "Radius", "feature_radius", &mut f.radius);
                    if (f.radius - before).abs() > f64::EPSILON && f.radius > 0.0 {
                        f.reflatten();
                    }
                    ui.add_space(2.0);
                }
                row(ui, "Material", |ui| {
                    material_combo(ui, "tf_material", &mut f.material, &MATERIALS)
                });
                ui.checkbox(&mut f.pad, "Grade the terrain (cut and fill pad)");
                if f.pad {
                    fields.length_row(
                        ui,
                        "Pad above (+) or below (-) ground",
                        "feature_h",
                        &mut f.height,
                    );
                    row(ui, "Side slope 1 rise to", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut f.slope_ratio)
                                .range(0.25..=12.0)
                                .speed(0.05)
                                .suffix(" run"),
                        )
                    });
                    ui.weak(
                        "The ground under the outline is levelled to a flat pad at this height \
                         over the mean ground; its sides slope back to the ground.",
                    );
                } else {
                    fields.length_row(ui, "Height above ground", "feature_h", &mut f.height);
                    ui.weak("A flat slab at this height over the mean ground under the outline.");
                }
                if !f.control.is_empty() {
                    ui.weak(format!(
                        "{} control points: drag them with the Select tool.",
                        f.control.len()
                    ));
                }
            }
            TerrainObject::Break(b) => {
                section(ui, "General");
                fields.length_row(ui, "Elevation", "break_z", &mut b.z);
                ui.weak("The surface is held at this elevation along the line.");
            }
            TerrainObject::Wall(w) => {
                section(ui, "General");
                row(ui, "Type", |ui| {
                    ui.radio_value(&mut w.kind, WallKind::Wall, "Wall");
                    ui.radio_value(&mut w.kind, WallKind::Curb, "Curb");
                });
                row(ui, "Material", |ui| {
                    material_combo(ui, "tw_material", &mut w.material, &MATERIALS[..3])
                });
                fields.length_row(ui, "Top above terrain", "wall_h", &mut w.height);
                fields.length_row(ui, "Bottom below terrain", "wall_d", &mut w.depth);
                fields.length_row(ui, "Thickness", "wall_t", &mut w.thickness);
                ui.checkbox(&mut w.stepped, "Stepped top (the top drops in courses)");
                if w.stepped {
                    fields.length_row(ui, "Course height", "wall_step", &mut w.step);
                    ui.weak(
                        "The top holds level and steps down by whole courses as the ground \
                         falls, instead of following every bump of the ground.",
                    );
                }
                ui.checkbox(&mut w.cut, "Cut the terrain along the wall");
                if w.cut {
                    fields.length_row(
                        ui,
                        "Grade step across the wall",
                        "wall_retain",
                        &mut w.retain,
                    );
                    ui.weak(
                        "The ground on the right of the wall (drawing direction) is this much \
                         lower than on its left; contours stop at the wall.",
                    );
                }
            }
            TerrainObject::Landscape(l) => landscape_general(ui, fields, l, &mut self.chooser),
            TerrainObject::Road(r) => {
                section(ui, "General");
                row(ui, "Type", |ui| {
                    ui.radio_value(&mut r.kind, RoadKind::Road, "Road");
                    ui.radio_value(&mut r.kind, RoadKind::Driveway, "Driveway");
                    ui.radio_value(&mut r.kind, RoadKind::Sidewalk, "Sidewalk");
                    ui.radio_value(&mut r.kind, RoadKind::Marking, "Marking");
                });
                if r.kind == RoadKind::Marking {
                    fields.length_row(ui, "Line width", "road_w", &mut r.width);
                    ui.checkbox(&mut r.dashed, "Dashed");
                    let mut own = r.color.is_some();
                    if ui.checkbox(&mut own, "Use a color of its own").changed() {
                        r.color = own.then_some([0xC9, 0x9A, 0x12]);
                    }
                    if let Some(c) = &mut r.color {
                        row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
                    }
                    ui.weak(
                        "Lies on the ground, or on the crown of any road under it; \
                         the plan draws it as a line this wide.",
                    );
                } else {
                    row(ui, "Material", |ui| {
                        let mut name = r.material_name().to_string();
                        material_combo(ui, "road_material", &mut name, &ROAD_MATERIALS);
                        r.material = name;
                    });
                    fields.length_row(ui, "Width", "road_w", &mut r.width);
                    fields.length_row(ui, "Crown", "road_crown", &mut r.crown);
                    ui.checkbox(&mut r.curb, "Curbs");
                    if r.curb {
                        fields.length_row(ui, "Curb height", "road_curb_h", &mut r.curb_height);
                    }
                }
            }
            TerrainObject::Point(e) => {
                section(ui, "General");
                fields.length_row(ui, "Elevation", "point_z", &mut e.z);
                fields.length_row(ui, "Position X", "point_x", &mut e.pos.x);
                fields.length_row(ui, "Position Y", "point_y", &mut e.pos.y);
                ui.weak("The surface passes through this spot height.");
            }
            TerrainObject::Region(r) => {
                section(ui, "General");
                fields.length_row(ui, "Elevation", "region_z", &mut r.z);
                ui.weak(format!(
                    "The surface is held at this elevation inside the {} corner outline.",
                    r.polygon.len()
                ));
            }
            TerrainObject::Modifier(m) => {
                section(ui, "General");
                match m.kind {
                    ModifierKind::FlatRegion => {
                        ui.label("Levels the ground inside the outline to its mean elevation.");
                        ui.weak("Flat Regions (Cut/Fill) have no height to set.");
                    }
                    kind => {
                        let label = match kind {
                            ModifierKind::Hill => "Hill height",
                            ModifierKind::Valley => "Valley depth",
                            ModifierKind::RaisedRegion => "Raise by",
                            _ => "Lower by",
                        };
                        fields.length_row(ui, label, "modifier_h", &mut m.height);
                    }
                }
            }
            TerrainObject::Line(l) => {
                section(ui, "General");
                fields.length_row(ui, "Elevation", "line_z", &mut l.z);
                if l.control.len() >= 3 {
                    let before = l.tension;
                    row(ui, "Spline tension", |ui| {
                        ui.add(
                            egui::DragValue::new(&mut l.tension)
                                .range(0.0..=1.0)
                                .speed(0.01)
                                .fixed_decimals(2),
                        )
                    });
                    if (l.tension - before).abs() > f64::EPSILON {
                        l.reflatten(SPLINE_SAMPLES);
                    }
                    ui.weak("0 is straight segments, 0.5 a smooth curve, 1 a loose one.");
                }
            }
        }
    }

    fn line_style(&mut self, ui: &mut Ui) {
        let Some(st) = self.style_mut() else { return };
        section(ui, "Line Style");
        let mut own = st.line_color.is_some();
        if ui.checkbox(&mut own, "Use a color of its own").changed() {
            st.line_color = own.then_some([0, 0, 0]);
        }
        if let Some(c) = &mut st.line_color {
            row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
        }
        let mut weight = st.line_weight;
        row(ui, "Line weight", |ui| {
            ui.add(
                egui::DragValue::new(&mut weight)
                    .range(0.0..=8.0)
                    .speed(0.05)
                    .suffix(" pt"),
            )
        });
        st.line_weight = weight;
        ui.weak("0 takes the object's own weight.");
        ui.checkbox(&mut st.dashed, "Dashed");
    }

    fn fill_style(&mut self, ui: &mut Ui) {
        let Some(st) = self.style_mut() else { return };
        section(ui, "Fill Style");
        ui.radio_value(&mut st.fill, FillStyle::Default, "Default for this object");
        ui.radio_value(&mut st.fill, FillStyle::None, "No Fill");
        ui.radio_value(&mut st.fill, FillStyle::Solid, "Solid");
        ui.radio_value(&mut st.fill, FillStyle::Hatch, "Hatch");
        ui.radio_value(&mut st.fill, FillStyle::Ripple, "Ripple (water)");
        let mut own = st.fill_color.is_some();
        if ui
            .checkbox(&mut own, "Use a fill color of its own")
            .changed()
        {
            st.fill_color = own.then_some([128, 128, 128]);
        }
        if let Some(c) = &mut st.fill_color {
            row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
        }
    }

    fn layer(&mut self, ui: &mut Ui) {
        if let TerrainObject::Road(r) = &mut self.draft {
            section(ui, "Layer");
            row(ui, "Layer", |ui| ui.text_edit_singleline(&mut r.layer));
            ui.weak("Empty draws it on the terrain's own layer.");
            return;
        }
        let default = self.default_layer();
        let Some(st) = self.style_mut() else { return };
        section(ui, "Layer");
        row(ui, "Layer", |ui| ui.text_edit_singleline(&mut st.layer));
        ui.weak(format!("Empty puts it on \"{default}\"."));
    }
}

fn landscape_general(ui: &mut Ui, fields: &mut Fields, l: &mut Landscape, chooser: &mut Chooser) {
    section(ui, "General");
    if !l.control.is_empty() {
        ui.weak(format!(
            "{} control points: drag them with the Select tool.",
            l.control.len()
        ));
    }
    match l.kind {
        LandscapeKind::GardenBed => {
            row(ui, "Material", |ui| {
                material_combo(ui, "bed_material", &mut l.material, &BED_MATERIALS)
            });
            fields.length_row(ui, "Mulch depth", "bed_depth", &mut l.height);
            ui.checkbox(&mut l.edging, "Edging");
            if l.edging {
                fields.length_row(ui, "Edging height", "bed_edge", &mut l.size);
            }
        }
        LandscapeKind::GrassRegion => {
            fields.length_row(ui, "Height above ground", "grass_h", &mut l.height);
        }
        LandscapeKind::WaterFeature => {
            fields.length_row(ui, "Water level below grade", "water_drop", &mut l.height);
            fields.length_row(ui, "Depth of water", "water_depth", &mut l.depth);
            ui.checkbox(&mut l.edging, "Stone edge");
            if l.edging {
                fields.length_row(ui, "Edge width", "water_edge", &mut l.size);
            }
        }
        LandscapeKind::SteppingStones => {
            fields.length_row(ui, "Stone size", "stone_size", &mut l.size);
            fields.length_row(ui, "Spacing", "stone_space", &mut l.spacing);
            fields.length_row(ui, "Thickness", "stone_thick", &mut l.height);
            ui.weak(format!("{} stones along the path", l.stones().len()));
        }
        LandscapeKind::Plants => {
            let current = plant_choices()
                .iter()
                .find(|p| p.id == l.plant)
                .map_or_else(|| "Custom".to_string(), |p| p.name.clone());
            row(ui, "Plant", |ui| {
                ui.label(current);
                let label = if chooser.open {
                    "Close Plant Chooser"
                } else {
                    "Plant Chooser\u{2026}"
                };
                if ui.button(label).clicked() {
                    chooser.open = !chooser.open;
                }
            });
            if chooser.open {
                plant_chooser(ui, l, chooser);
            }
            fields.length_row(ui, "Canopy width", "plant_w", &mut l.size);
            fields.length_row(ui, "Height", "plant_h", &mut l.height);
            fields.length_row(ui, "Spacing", "plant_space", &mut l.spacing);
            row(ui, "3D form", |ui| {
                egui::ComboBox::from_id_salt("plant_form")
                    .selected_text(l.form.name())
                    .show_ui(ui, |ui| {
                        for f in PlantForm::ALL {
                            ui.selectable_value(&mut l.form, f, f.name());
                        }
                    });
            });
            ui.weak(format!(
                "{} plants along the path, built as {}",
                l.plant_positions().len(),
                l.plant_form().name().to_lowercase()
            ));
        }
        LandscapeKind::Sprinklers => {
            fields.length_row(ui, "Spray radius", "spr_radius", &mut l.size);
            fields.length_row(ui, "Head spacing", "spr_space", &mut l.spacing);
            fields.degrees_row(ui, "Spray angle", "deg_spray", &mut l.arc);
            fields.length_row(ui, "Riser height", "spr_riser", &mut l.height);
            ui.weak(format!("{} heads along the path", l.heads().len()));
        }
    }
}

/// The Plant Chooser: the Plants catalog by category with a search field; a
/// click on a plant sets the run's plant, sizes and 3D form.
fn plant_chooser(ui: &mut Ui, l: &mut Landscape, chooser: &mut Chooser) {
    ui.group(|ui| {
        row(ui, "Category", |ui| {
            let shown = if chooser.category.is_empty() {
                "All plants".to_string()
            } else {
                chooser.category.clone()
            };
            egui::ComboBox::from_id_salt("plant_category")
                .selected_text(shown)
                .width(200.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut chooser.category, String::new(), "All plants");
                    for c in plant_categories() {
                        ui.selectable_value(&mut chooser.category, c.clone(), c);
                    }
                });
        });
        row(ui, "Search", |ui| {
            ui.text_edit_singleline(&mut chooser.search)
        });
        let list = plants_in(&chooser.category, &chooser.search);
        egui::ScrollArea::vertical()
            .id_salt("plant_chooser_list")
            .max_height(CHOOSER_HEIGHT)
            .show(ui, |ui| {
                for p in &list {
                    let text = format!(
                        "{}   {} wide, {} tall",
                        p.name,
                        fmt_short(p.width),
                        fmt_short(p.height)
                    );
                    if ui.selectable_label(l.plant == p.id, text).clicked() {
                        apply_plant(l, p);
                    }
                }
            });
        ui.weak(format!("{} plants", list.len()));
    });
}

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        match &self.draft {
            TerrainObject::Road(_) => ROAD_TABS,
            TerrainObject::Line(_)
            | TerrainObject::Point(_)
            | TerrainObject::Region(_)
            | TerrainObject::Modifier(_) => GENERAL_ONLY,
            TerrainObject::Feature(f) if f.kind == FeatureKind::Hole => GENERAL_ONLY,
            TerrainObject::Break(_) => PATHS,
            TerrainObject::Landscape(l) if !l.is_region() => PATHS,
            _ => REGIONS,
        }
    }

    fn error(&self) -> Option<String> {
        if self.fields.any_invalid() {
            return Some("Fix the highlighted field".into());
        }
        match &self.draft {
            TerrainObject::Wall(w) => {
                if w.thickness < MIN_SIZE {
                    return Some("The thickness must be at least 1\"".into());
                }
                if w.height + w.depth <= 0.0 {
                    return Some("The wall needs a height".into());
                }
                if !w.retain.is_finite() {
                    return Some("The grade step is not a length".into());
                }
                if w.stepped && w.step < MIN_SIZE {
                    return Some("A course is at least 1\" high".into());
                }
            }
            TerrainObject::Road(r) if r.width <= 0.0 => {
                return Some("The width must be greater than zero".into());
            }
            TerrainObject::Road(r) if r.kind == RoadKind::Marking && r.width > 48.0 => {
                return Some("A marking is at most 4' wide".into());
            }
            TerrainObject::Feature(f) if f.kind == FeatureKind::Round && f.radius < MIN_SIZE => {
                return Some("The radius must be at least 1\"".into());
            }
            TerrainObject::Modifier(m) if m.height < 0.0 => {
                return Some("The height cannot be negative".into());
            }
            TerrainObject::Landscape(l) => {
                let needs_spacing = matches!(
                    l.kind,
                    LandscapeKind::SteppingStones
                        | LandscapeKind::Plants
                        | LandscapeKind::Sprinklers
                );
                if needs_spacing && l.spacing < MIN_SIZE {
                    return Some("The spacing must be at least 1\"".into());
                }
                if matches!(
                    l.kind,
                    LandscapeKind::SteppingStones | LandscapeKind::Plants
                ) && l.size < MIN_SIZE
                {
                    return Some("The size must be at least 1\"".into());
                }
                if l.kind == LandscapeKind::Sprinklers && !(1.0..=360.0).contains(&l.arc) {
                    return Some("The spray angle is between 1 and 360 degrees".into());
                }
                if l.height < 0.0 || l.depth < 0.0 || l.size < 0.0 {
                    return Some("Sizes cannot be negative".into());
                }
            }
            TerrainObject::Feature(f) if f.height < 0.0 && !f.pad => {
                return Some("The height cannot be negative".into());
            }
            TerrainObject::Feature(f) if f.pad && !(0.25..=12.0).contains(&f.slope_ratio) => {
                return Some("The side slope is between 0.25 and 12".into());
            }
            TerrainObject::Line(l) if !(0.0..=1.0).contains(&l.tension) => {
                return Some("The spline tension is between 0 and 1".into());
            }
            _ => {}
        }
        None
    }

    fn page(&mut self, ui: &mut Ui, tab: usize) {
        match self.tabs()[tab].name {
            "General" => self.general(ui),
            "Line Style" => self.line_style(ui),
            "Fill Style" => self.fill_style(ui),
            "Layer" => self.layer(ui),
            _ => {}
        }
    }

    fn preview(&self, p: &Painter, rect: Rect) {
        let (pts, closed) = match &self.draft {
            TerrainObject::Feature(f) => (f.polygon.clone(), true),
            TerrainObject::Break(b) => (b.points.clone(), false),
            TerrainObject::Wall(w) => (w.points.clone(), false),
            TerrainObject::Landscape(l) => (l.points.clone(), l.is_region()),
            TerrainObject::Road(r) => (r.centerline.clone(), false),
            TerrainObject::Line(l) => (l.points.clone(), false),
            TerrainObject::Point(e) => (vec![e.pos], false),
            TerrainObject::Region(r) => (r.polygon.clone(), true),
            TerrainObject::Modifier(m) => (m.polygon.clone(), true),
        };
        pv_text(
            p,
            Pos2::new(rect.center().x, rect.min.y + 6.0),
            Align2::CENTER_CENTER,
            self.draft.title().trim_end_matches(" Specification"),
            11.0,
        );
        let area = Rect::from_min_max(
            Pos2::new(rect.min.x, rect.min.y + 18.0),
            Pos2::new(rect.max.x, rect.max.y - 22.0),
        );
        if pts.len() >= 2 {
            let (mut lo, mut hi) = (pts[0], pts[0]);
            for q in &pts {
                lo = Point::new(lo.x.min(q.x), lo.y.min(q.y));
                hi = Point::new(hi.x.max(q.x), hi.y.max(q.y));
            }
            let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
            let scale = (f64::from(area.width()) / w).min(f64::from(area.height()) / h);
            let at = |q: &Point| {
                Pos2::new(
                    area.min.x + ((q.x - lo.x) * scale) as f32,
                    area.min.y + ((hi.y - q.y) * scale) as f32,
                )
            };
            let line: Vec<Pos2> = pts.iter().map(at).collect();
            let stroke = Stroke::new(1.5_f32, PV_INK);
            if closed {
                p.add(egui::Shape::closed_line(line, stroke));
            } else {
                p.add(egui::Shape::line(line, stroke));
            }
            if let TerrainObject::Landscape(l) = &self.draft {
                let marks: Vec<Point> = match l.kind {
                    LandscapeKind::SteppingStones => l.stones().into_iter().map(|s| s.0).collect(),
                    LandscapeKind::Plants => l.plant_positions(),
                    LandscapeKind::Sprinklers => l.heads().into_iter().map(|s| s.0).collect(),
                    _ => Vec::new(),
                };
                for m in &marks {
                    p.circle_filled(at(m), 2.5, PV_INK);
                }
            }
        } else if matches!(self.draft, TerrainObject::Point(_)) {
            // A spot height: a cross.
            let c = area.center();
            let stroke = Stroke::new(1.5_f32, PV_INK);
            p.line_segment([c - egui::vec2(8.0, 0.0), c + egui::vec2(8.0, 0.0)], stroke);
            p.line_segment([c - egui::vec2(0.0, 8.0), c + egui::vec2(0.0, 8.0)], stroke);
        } else {
            pv_text(p, area.center(), Align2::CENTER_CENTER, "No outline", 11.0);
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
            format!("{} points", pts.len()),
            10.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_terrain::{Feature, LandscapeKind, ShapeKind, TerrainBreak, TerrainWall};

    fn pts() -> Vec<Point> {
        vec![
            Point::new(0.0, 0.0),
            Point::new(300.0, 0.0),
            Point::new(300.0, 200.0),
            Point::new(0.0, 200.0),
        ]
    }

    fn objects() -> Vec<TerrainObject> {
        let mut v = vec![
            TerrainObject::Feature(Feature {
                polygon: pts(),
                ..Feature::default()
            }),
            TerrainObject::Break(TerrainBreak {
                points: pts(),
                ..TerrainBreak::default()
            }),
            TerrainObject::Wall(TerrainWall::new(WallKind::Wall, pts(), false)),
            TerrainObject::Road(plan_terrain::RoadStrip {
                kind: RoadKind::Road,
                centerline: pts(),
                width: 120.0,
                curb: true,
                ..plan_terrain::RoadStrip::default()
            }),
            TerrainObject::Line(plan_terrain::ElevationLine::spline(pts(), 12.0, 0.5, 8)),
            TerrainObject::Feature(Feature::round(Point::new(100.0, 100.0), 60.0)),
            TerrainObject::Feature(Feature {
                kind: FeatureKind::Hole,
                polygon: pts(),
                ..Feature::default()
            }),
            TerrainObject::Road(plan_terrain::RoadStrip::marking(pts(), 4.0, true)),
            TerrainObject::Point(plan_terrain::ElevationPoint {
                pos: Point::new(10.0, 20.0),
                z: 36.0,
            }),
            TerrainObject::Region(plan_terrain::ElevationRegion {
                polygon: pts(),
                z: 12.0,
            }),
        ];
        for kind in [
            ModifierKind::Hill,
            ModifierKind::Valley,
            ModifierKind::RaisedRegion,
            ModifierKind::LoweredRegion,
            ModifierKind::FlatRegion,
        ] {
            v.push(TerrainObject::Modifier(plan_terrain::Modifier {
                kind,
                polygon: pts(),
                height: 48.0,
            }));
        }
        for kind in [
            LandscapeKind::GardenBed,
            LandscapeKind::GrassRegion,
            LandscapeKind::WaterFeature,
            LandscapeKind::SteppingStones,
            LandscapeKind::Plants,
            LandscapeKind::Sprinklers,
        ] {
            v.push(TerrainObject::Landscape(Landscape::new(
                kind,
                ShapeKind::Polyline,
                pts(),
            )));
        }
        v
    }

    #[test]
    fn tabs_follow_the_kind_of_object() {
        for obj in objects() {
            let d = ObjectDialog::new(obj.clone());
            let tabs = d.tab_names();
            assert_eq!(tabs[0], "General");
            match &obj {
                TerrainObject::Road(_) => assert_eq!(tabs, ["General", "Layer"]),
                TerrainObject::Line(_)
                | TerrainObject::Point(_)
                | TerrainObject::Region(_)
                | TerrainObject::Modifier(_) => assert_eq!(tabs.len(), 1),
                TerrainObject::Feature(f) if f.kind == FeatureKind::Hole => {
                    assert_eq!(tabs, ["General"]);
                }
                TerrainObject::Landscape(l) if !l.is_region() => {
                    assert_eq!(tabs, ["General", "Line Style", "Layer"]);
                }
                TerrainObject::Break(_) => assert_eq!(tabs, ["General", "Line Style", "Layer"]),
                _ => assert_eq!(tabs, ["General", "Line Style", "Fill Style", "Layer"]),
            }
            assert!(d.error().is_none(), "{}", obj.title());
        }
    }

    #[test]
    fn bad_sizes_block_ok() {
        let mut d = ObjectDialog::new(TerrainObject::Wall(TerrainWall::new(
            WallKind::Wall,
            pts(),
            false,
        )));
        let TerrainObject::Wall(w) = d.draft_mut() else {
            unreachable!()
        };
        w.thickness = 0.0;
        assert!(d.error().is_some());
        let mut d = ObjectDialog::new(TerrainObject::Landscape(Landscape::new(
            LandscapeKind::SteppingStones,
            ShapeKind::Polyline,
            pts(),
        )));
        let set_spacing = |d: &mut ObjectDialog, v: f64| {
            let TerrainObject::Landscape(l) = d.draft_mut() else {
                unreachable!()
            };
            l.spacing = v;
        };
        set_spacing(&mut d, 0.0);
        assert!(d.error().unwrap().contains("spacing"));
        set_spacing(&mut d, 30.0);
        assert!(d.error().is_none());
        let mut d = ObjectDialog::new(TerrainObject::Landscape(Landscape::new(
            LandscapeKind::Sprinklers,
            ShapeKind::Polyline,
            pts(),
        )));
        let TerrainObject::Landscape(l) = d.draft_mut() else {
            unreachable!()
        };
        l.arc = 400.0;
        assert!(d.error().is_some());
    }

    #[test]
    fn every_dialog_draws_every_page_headlessly() {
        let ctx = egui::Context::default();
        for obj in objects() {
            let mut d = ObjectDialog::new(obj);
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    assert_eq!(d.show(ctx), Outcome::Open);
                });
            }
            // Every page and the preview, directly.
            for tab in 0..d.form.tabs().len() {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        d.form.page(ui, tab);
                        let (_, painter) = ui
                            .allocate_painter(egui::Vec2::new(200.0, 200.0), egui::Sense::hover());
                        d.form.preview(&painter, painter.clip_rect());
                    });
                });
            }
        }
    }

    #[test]
    fn the_new_objects_keep_their_edits_and_refuse_bad_values() {
        // A round feature resizes its outline with the radius.
        let mut d = ObjectDialog::new(TerrainObject::Feature(Feature::round(
            Point::new(0.0, 0.0),
            60.0,
        )));
        let TerrainObject::Feature(f) = d.draft_mut() else {
            unreachable!()
        };
        f.radius = 0.0;
        assert!(d.error().unwrap().contains("radius"));
        // A marking is at most 4' wide.
        let mut d = ObjectDialog::new(TerrainObject::Road(plan_terrain::RoadStrip::marking(
            pts(),
            4.0,
            false,
        )));
        assert_eq!(d.draft().title(), "Road Marking Specification");
        let TerrainObject::Road(r) = d.draft_mut() else {
            unreachable!()
        };
        r.width = 60.0;
        assert!(d.error().unwrap().contains("4'"));
        // A stepped wall needs a course height.
        let mut d = ObjectDialog::new(TerrainObject::Wall(TerrainWall::new(
            WallKind::Wall,
            pts(),
            false,
        )));
        let TerrainObject::Wall(w) = d.draft_mut() else {
            unreachable!()
        };
        w.stepped = true;
        w.step = 0.0;
        assert!(d.error().unwrap().contains("course"));
        // A modifier's height is a magnitude.
        let mut d = ObjectDialog::new(TerrainObject::Modifier(plan_terrain::Modifier {
            kind: ModifierKind::Hill,
            polygon: pts(),
            height: 72.0,
        }));
        assert_eq!(d.draft().title(), "Hill Specification");
        let TerrainObject::Modifier(m) = d.draft_mut() else {
            unreachable!()
        };
        m.height = -1.0;
        assert!(d.error().is_some());
    }

    #[test]
    fn the_plant_chooser_sets_the_plant_its_size_and_form() {
        let mut run = Landscape::new(LandscapeKind::Plants, ShapeKind::Polyline, pts());
        let list = crate::tools::terrain::plant_choices();
        let spruce = list.iter().find(|p| p.conifer).expect("a conifer");
        apply_plant(&mut run, spruce);
        let mut d = ObjectDialog::new(TerrainObject::Landscape(run));
        d.form.chooser.open = true;
        d.form.chooser.category = spruce.category.clone();
        // Draw the page with the chooser open.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| d.form.page(ui, 0));
        });
        let TerrainObject::Landscape(l) = d.draft() else {
            unreachable!()
        };
        assert_eq!(l.form, PlantForm::Cone);
        assert_eq!(l.plant, spruce.id);
    }

    #[test]
    fn edits_stay_in_the_draft() {
        let original = objects()
            .into_iter()
            .find(|o| matches!(o, TerrainObject::Landscape(_)))
            .unwrap();
        let mut d = ObjectDialog::new(original.clone());
        let TerrainObject::Landscape(l) = d.draft_mut() else {
            unreachable!()
        };
        l.spacing = 60.0;
        l.style.layer = "Mine".into();
        assert_ne!(*d.draft(), original);
    }
}
