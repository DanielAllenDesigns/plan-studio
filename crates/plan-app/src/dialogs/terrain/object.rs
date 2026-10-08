//! Specification of one terrain object (double-click it with a Terrain tool):
//! terrain feature, break line, terrain wall or curb, garden bed, grass
//! region, water feature, stepping stones, plant run, sprinklers, and the
//! road and elevation line (General only). Pages: General (sizes, heights,
//! material, spacing), Line Style, Fill Style (regions and walls) and Layer.
//! The dialog edits a [`TerrainObject`] draft the tool stores on OK.

use super::super::{
    on, pv_text, row, section, Fields, Outcome, SpecDialog, SpecPages, Tab, PV_FAINT, PV_INK,
};
use crate::editor::site_view::TerrainObject;
use crate::tools::terrain::{apply_plant, plant_choices};
use eframe::egui::{self, Align2, Painter, Pos2, Rect, Stroke, Ui};
use plan_core::Point;
use plan_terrain::{FillStyle, Landscape, LandscapeKind, ObjectStyle, RoadKind, WallKind};

const GENERAL_ONLY: &[Tab] = &[on("General")];
const PATHS: &[Tab] = &[on("General"), on("Line Style"), on("Layer")];
const REGIONS: &[Tab] = &[
    on("General"),
    on("Line Style"),
    on("Fill Style"),
    on("Layer"),
];
const MATERIALS: [&str; 4] = ["Concrete", "Stone", "Brick", "Grass"];
const BED_MATERIALS: [&str; 3] = ["Mulch", "Soil", "Stone"];
const MIN_SIZE: f64 = 1.0;

pub struct ObjectDialog {
    frame: SpecDialog,
    form: Form,
}

struct Form {
    draft: TerrainObject,
    fields: Fields,
}

impl ObjectDialog {
    pub fn new(obj: TerrainObject) -> Self {
        let title = obj.title();
        Self {
            frame: SpecDialog::new(title, ("terrain_object", title)),
            form: Form {
                draft: obj,
                fields: Fields::default(),
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
            TerrainObject::Feature(f) => Some(&mut f.style),
            TerrainObject::Break(b) => Some(&mut b.style),
            TerrainObject::Wall(w) => Some(&mut w.style),
            TerrainObject::Landscape(l) => Some(&mut l.style),
            TerrainObject::Road(_) | TerrainObject::Line(_) => None,
        }
    }

    fn default_layer(&self) -> String {
        match &self.draft {
            TerrainObject::Feature(_) => plan_terrain::landscape::LAYER_FEATURES.into(),
            TerrainObject::Break(_) => plan_terrain::landscape::LAYER_BREAKS.into(),
            TerrainObject::Wall(w) => w.default_layer().into(),
            TerrainObject::Landscape(l) => l.default_layer().into(),
            TerrainObject::Road(_) | TerrainObject::Line(_) => "Terrain".into(),
        }
    }

    fn general(&mut self, ui: &mut Ui) {
        let fields = &mut self.fields;
        match &mut self.draft {
            TerrainObject::Feature(f) => {
                section(ui, "General");
                row(ui, "Material", |ui| {
                    material_combo(ui, "tf_material", &mut f.material, &MATERIALS)
                });
                fields.length_row(ui, "Height above ground", "feature_h", &mut f.height);
                ui.weak("A flat top at this height over the mean ground under the outline.");
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
            }
            TerrainObject::Landscape(l) => landscape_general(ui, fields, l),
            TerrainObject::Road(r) => {
                section(ui, "General");
                row(ui, "Type", |ui| {
                    ui.radio_value(&mut r.kind, RoadKind::Road, "Road");
                    ui.radio_value(&mut r.kind, RoadKind::Driveway, "Driveway");
                    ui.radio_value(&mut r.kind, RoadKind::Sidewalk, "Sidewalk");
                });
                fields.length_row(ui, "Width", "road_w", &mut r.width);
                ui.checkbox(&mut r.curb, "Curbs");
            }
            TerrainObject::Line(l) => {
                section(ui, "General");
                fields.length_row(ui, "Elevation", "line_z", &mut l.z);
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
        let default = self.default_layer();
        let Some(st) = self.style_mut() else { return };
        section(ui, "Layer");
        row(ui, "Layer", |ui| ui.text_edit_singleline(&mut st.layer));
        ui.weak(format!("Empty puts it on \"{default}\"."));
    }
}

fn landscape_general(ui: &mut Ui, fields: &mut Fields, l: &mut Landscape) {
    section(ui, "General");
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
            let plants = plant_choices();
            let current = plants
                .iter()
                .find(|p| p.id == l.plant)
                .map_or_else(|| "Custom".to_string(), |p| p.name.clone());
            row(ui, "Plant", |ui| {
                egui::ComboBox::from_id_salt("plant_choice")
                    .selected_text(current)
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for p in plants {
                            if ui.selectable_label(l.plant == p.id, &p.name).clicked() {
                                apply_plant(l, p);
                            }
                        }
                    });
            });
            fields.length_row(ui, "Canopy width", "plant_w", &mut l.size);
            fields.length_row(ui, "Height", "plant_h", &mut l.height);
            fields.length_row(ui, "Spacing", "plant_space", &mut l.spacing);
            ui.weak(format!(
                "{} plants along the path",
                l.plant_positions().len()
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

impl SpecPages for Form {
    fn tabs(&self) -> &'static [Tab] {
        match &self.draft {
            TerrainObject::Road(_) | TerrainObject::Line(_) => GENERAL_ONLY,
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
            }
            TerrainObject::Road(r) if r.width <= 0.0 => {
                return Some("The width must be greater than zero".into());
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
            TerrainObject::Feature(f) if f.height < 0.0 => {
                return Some("The height cannot be negative".into());
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
            }),
            TerrainObject::Line(plan_terrain::ElevationLine {
                points: pts(),
                z: 12.0,
            }),
        ];
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
                TerrainObject::Road(_) | TerrainObject::Line(_) => assert_eq!(tabs.len(), 1),
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
    fn edits_stay_in_the_draft() {
        let original = objects().remove(7);
        let mut d = ObjectDialog::new(original.clone());
        let TerrainObject::Landscape(l) = d.draft_mut() else {
            unreachable!()
        };
        l.spacing = 60.0;
        l.style.layer = "Mine".into();
        assert_ne!(*d.draft(), original);
    }
}
