//! The extra panels of the terrain object specifications (manual pp.
//! 1327-1341, 1353-1356, 1362-1366): Polyline, Label, Object Information,
//! Schedule, Display (elevation points), Flare and Curb (roads), Blades and
//! Appearance (grass), Distributed Plant (garden beds) and Plant Image.

use super::super::{fmt_short, row, section, Fields};
use crate::editor::site_view::TerrainObject;
use crate::tools::terrain::plants_in;
use eframe::egui::{self, Ui};
use plan_core::geometry::polygon_area;
use plan_core::Point;
use plan_terrain::{
    default_seasons, is_conifer, Distribution, FeatureKind, GrassBlades, Landscape, LandscapeKind,
    ObjectExtras, PlantImage, RoadKind, RoadStrip, ScheduleCategory, Season, DEFAULT_FLARE,
    DEFAULT_MARKER_RADIUS,
};

/// Cubic inches in a cubic foot.
const CUBIC_INCHES_PER_FOOT: f64 = 1728.0;
const SQUARE_INCHES_PER_FOOT: f64 = 144.0;

/// The outline an object has, whether it is closed, and its thickness.
pub fn outline_of(obj: &TerrainObject) -> (Vec<Point>, bool, f64) {
    match obj {
        TerrainObject::Feature(f) => (f.polygon.clone(), true, f.height.abs().max(f.thickness)),
        TerrainObject::Break(b) => (b.points.clone(), false, 0.0),
        TerrainObject::Wall(w) => (w.points.clone(), false, w.height + w.depth),
        TerrainObject::Landscape(l) => (l.points.clone(), l.is_region(), l.height),
        TerrainObject::Road(r) => {
            if r.outline.len() >= 3 || r.kind.is_outline_kind() {
                (plan_terrain::road_polygon(r), true, r.thickness)
            } else {
                (r.centerline.clone(), false, r.thickness)
            }
        }
        TerrainObject::Line(l) => (l.points.clone(), false, 0.0),
        TerrainObject::Point(e, _) => (vec![e.pos], false, 0.0),
        TerrainObject::Region(r, _) => (r.polygon.clone(), true, 0.0),
        TerrainObject::Modifier(m, _) => (m.polygon.clone(), true, m.height),
    }
}

/// Polyline panel: the perimeter, area, volume and number of lines (read only).
pub fn polyline_panel(ui: &mut Ui, obj: &TerrainObject) {
    section(ui, "Polyline");
    let (pts, closed, thickness) = outline_of(obj);
    if pts.len() < 2 {
        ui.weak("This object has no outline.");
        return;
    }
    let mut length: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    if closed {
        length += pts[pts.len() - 1].dist(pts[0]);
    }
    let lines = if closed { pts.len() } else { pts.len() - 1 };
    let mut width_strip = false;
    let area = if closed {
        polygon_area(&pts).abs()
    } else if let TerrainObject::Road(r) = obj {
        width_strip = true;
        length * r.width.max(0.0)
    } else if let TerrainObject::Wall(w) = obj {
        width_strip = true;
        length * w.thickness
    } else {
        0.0
    };
    row(ui, if closed { "Perimeter" } else { "Length" }, |ui| {
        ui.label(fmt_short(length))
    });
    if closed || width_strip {
        row(ui, "Area", |ui| {
            ui.label(format!("{:.1} sq ft", area / SQUARE_INCHES_PER_FOOT))
        });
        row(ui, "Volume", |ui| {
            ui.label(format!(
                "{:.1} cu ft",
                area * thickness.abs() / CUBIC_INCHES_PER_FOOT
            ))
        });
    }
    row(ui, "Number of Lines", |ui| ui.label(lines.to_string()));
}

/// Label panel: show a label on the "Terrain Labels" layer, with custom text.
pub fn label_panel(ui: &mut Ui, fields: &mut Fields, ex: &mut ObjectExtras, automatic: &str) {
    section(ui, "Label");
    ui.checkbox(&mut ex.label.shown, "Show a label in plan");
    row(ui, "Label text", |ui| {
        ui.add(egui::TextEdit::singleline(&mut ex.label.text).hint_text(automatic))
    });
    ui.weak("Empty uses the automatic label.");
    fields.length_row(ui, "Offset X", "label_dx", &mut ex.label.offset.x);
    fields.length_row(ui, "Offset Y", "label_dy", &mut ex.label.offset.y);
    ui.weak(format!(
        "On the \"{}\" layer.",
        plan_terrain::LAYER_TERRAIN_LABELS
    ));
}

/// Object Information panel.
pub fn info_panel(ui: &mut Ui, ex: &mut ObjectExtras) {
    section(ui, "Object Information");
    for (label, field) in [
        ("Manufacturer", &mut ex.info.manufacturer),
        ("Supplier", &mut ex.info.supplier),
        ("Code", &mut ex.info.code),
        ("Comment", &mut ex.info.comment),
        ("URL", &mut ex.info.url),
    ] {
        row(ui, label, |ui| ui.text_edit_singleline(field));
    }
}

/// Components panel: the names of the components that make the object.
pub fn components_panel(ui: &mut Ui, ex: &mut ObjectExtras) {
    section(ui, "Components");
    let mut remove = None;
    for (i, c) in ex.components.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(c).desired_width(200.0));
            if ui.small_button("\u{2212}").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        ex.components.remove(i);
    }
    if ui.button("Add Component").clicked() {
        ex.components.push(String::new());
    }
    if ex.components.is_empty() {
        ui.weak("No components. They are listed in the Materials List.");
    }
}

/// The schedule category an object belongs to until its Schedule panel says otherwise.
pub fn default_category(obj: &TerrainObject) -> Option<ScheduleCategory> {
    Some(match obj {
        TerrainObject::Feature(f) if f.kind == FeatureKind::Hole => return None,
        TerrainObject::Feature(_) | TerrainObject::Modifier(..) => {
            ScheduleCategory::TerrainFeatures
        }
        TerrainObject::Wall(_) => ScheduleCategory::TerrainPaths,
        TerrainObject::Landscape(l) => match l.kind {
            LandscapeKind::Plants | LandscapeKind::Sprinklers | LandscapeKind::SprinklerLine => {
                return None
            }
            _ => ScheduleCategory::TerrainFeatures,
        },
        TerrainObject::Road(r) => match r.kind {
            RoadKind::Road | RoadKind::CulDeSac => ScheduleCategory::Roads,
            RoadKind::Driveway => ScheduleCategory::Driveways,
            RoadKind::Sidewalk => ScheduleCategory::TerrainPaths,
            RoadKind::Marking => ScheduleCategory::RoadMarkings,
            RoadKind::Median => ScheduleCategory::Medians,
        },
        _ => return None,
    })
}

/// Schedule panel: the category the object is listed under.
pub fn schedule_panel(ui: &mut Ui, ex: &mut ObjectExtras, default: Option<ScheduleCategory>) {
    section(ui, "Schedule");
    let auto = default.map_or("Not scheduled".to_string(), |c| {
        format!("Automatic ({})", c.name())
    });
    let shown = if ex.schedule_category.trim().is_empty() {
        auto.clone()
    } else {
        ex.schedule_category.clone()
    };
    row(ui, "Category", |ui| {
        egui::ComboBox::from_id_salt("terrain_schedule_category")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut ex.schedule_category, String::new(), auto.as_str());
                for c in ScheduleCategory::ALL {
                    ui.selectable_value(&mut ex.schedule_category, c.name().to_string(), c.name());
                }
            })
    });
    ui.weak("Any object can be moved to another category of the terrain schedule.");
}

/// Display panel of an Elevation Point.
pub fn display_panel(ui: &mut Ui, fields: &mut Fields, ex: &mut ObjectExtras) {
    section(ui, "Display");
    row(ui, "Note", |ui| ui.text_edit_singleline(&mut ex.note));
    if ui.button("Insert Macro: elevation").clicked() {
        ex.note.push_str("%elevation%");
    }
    ui.weak("The note is drawn beside the point; %elevation% shows the point's elevation.");
    let mut r = if ex.marker_radius > 0.0 {
        ex.marker_radius
    } else {
        DEFAULT_MARKER_RADIUS
    };
    if fields.length_row(ui, "Marker radius", "marker_r", &mut r) {
        ex.marker_radius = r.max(0.0);
    }
}

/// Road Flare panel.
pub fn flare_panel(ui: &mut Ui, fields: &mut Fields, r: &mut RoadStrip) {
    section(ui, "Flare");
    for (label, key, slot) in [
        ("Start", "flare_start", &mut r.flare_start),
        ("End", "flare_end", &mut r.flare_end),
    ] {
        let mut on = slot.is_some();
        if ui
            .checkbox(&mut on, format!("Flare at the {label}"))
            .changed()
        {
            *slot = on.then_some(DEFAULT_FLARE);
        }
        if let Some(radius) = slot.as_mut() {
            fields.length_row(ui, "Radius", key, radius);
        }
    }
    ui.weak("The strip widens into a quarter-circle fillet where it meets another road.");
}

/// Road Curb panel.
pub fn curb_panel(ui: &mut Ui, fields: &mut Fields, r: &mut RoadStrip) {
    section(ui, "Curb");
    ui.checkbox(&mut r.curb, "Has curb");
    if r.curb {
        fields.length_row(ui, "Curb height", "curb_h", &mut r.curb_height);
        fields.length_row(ui, "Curb width", "curb_w", &mut r.curb_width);
        ui.checkbox(&mut r.cut_curb, "Cut the curb for driveways and sidewalks");
    }
}

/// Grass Region > Blades.
pub fn blades_panel(ui: &mut Ui, b: &mut GrassBlades) {
    section(ui, "Blades");
    row(ui, "Density", |ui| {
        ui.add(
            egui::DragValue::new(&mut b.density)
                .range(0.0..=400.0)
                .suffix(" per sq ft"),
        )
    });
    for (label, lo, hi, max, speed) in [
        ("Height", 0usize, 1usize, 24.0, 0.1),
        ("Width", 2, 3, 2.0, 0.01),
        ("Curve", 4, 5, 1.0, 0.01),
    ] {
        let mut vals = [
            b.min_height,
            b.max_height,
            b.min_width,
            b.max_width,
            b.min_curve,
            b.max_curve,
        ];
        row(ui, &format!("Minimum {label}"), |ui| {
            ui.add(
                egui::DragValue::new(&mut vals[lo])
                    .range(0.0..=max)
                    .speed(speed),
            )
        });
        row(ui, &format!("Maximum {label}"), |ui| {
            ui.add(
                egui::DragValue::new(&mut vals[hi])
                    .range(0.0..=max)
                    .speed(speed),
            )
        });
        [
            b.min_height,
            b.max_height,
            b.min_width,
            b.max_width,
            b.min_curve,
            b.max_curve,
        ] = vals;
    }
    ui.weak("Heights and widths in inches; a curve of 0 is a straight blade.");
}

/// Grass Region > Appearance.
pub fn appearance_panel(ui: &mut Ui, l: &mut Landscape) {
    section(ui, "Appearance");
    let look = &mut l.grass_look;
    let mut remove = None;
    for (i, c) in look.colors.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.label(format!("Color {}", i + 1));
            ui.color_edit_button_srgb(c);
            if ui.small_button("Remove").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove.filter(|_| look.colors.len() > 1) {
        look.colors.remove(i);
    }
    if look.colors.len() < 6 && ui.button("Add a color").clicked() {
        look.colors.push([0x5A, 0x9A, 0x3A]);
    }
    row(ui, "Noise frequency", |ui| {
        ui.add(
            egui::DragValue::new(&mut look.noise_frequency)
                .range(0.0..=1.0)
                .speed(0.01),
        )
    });
    row(ui, "Roughness", |ui| {
        ui.add(
            egui::DragValue::new(&mut look.roughness)
                .range(0.0..=1.0)
                .speed(0.01),
        )
    });
    ui.checkbox(&mut look.mow.enabled, "Mow");
    if look.mow.enabled {
        row(ui, "Cut height", |ui| {
            ui.add(
                egui::DragValue::new(&mut look.mow.cut_height)
                    .range(0.0..=12.0)
                    .speed(0.1)
                    .suffix(" in"),
            )
        });
        row(ui, "Mow line intensity", |ui| {
            ui.add(
                egui::DragValue::new(&mut look.mow.line_intensity)
                    .range(0.0..=1.0)
                    .speed(0.01),
            )
        });
        row(ui, "Mow line width", |ui| {
            ui.add(
                egui::DragValue::new(&mut look.mow.line_width)
                    .range(1.0..=240.0)
                    .suffix(" in"),
            )
        });
        row(ui, "Mow angle", |ui| {
            ui.add(
                egui::DragValue::new(&mut look.mow.angle)
                    .range(-180.0..=180.0)
                    .suffix("\u{b0}"),
            )
        });
    }
    // A live sample of the grass.
    let (rect, _) = ui.allocate_exact_size(egui::vec2(220.0, 46.0), egui::Sense::hover());
    let p = ui.painter();
    let avg = look.average_color();
    p.rect_filled(rect, 3.0, egui::Color32::from_rgb(avg[0], avg[1], avg[2]));
    if look.mow.enabled {
        let w = (look.mow.line_width as f32 / 6.0).clamp(4.0, 40.0);
        let mut x = rect.min.x;
        let mut odd = false;
        while x < rect.max.x {
            if odd {
                let a = (look.mow.line_intensity * 120.0) as u8;
                p.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x, rect.min.y),
                        egui::pos2((x + w).min(rect.max.x), rect.max.y),
                    ),
                    0.0,
                    egui::Color32::from_black_alpha(a),
                );
            }
            odd = !odd;
            x += w;
        }
    }
}

/// Garden Bed > Distributed Plant.
pub fn distributed_panel(ui: &mut Ui, fields: &mut Fields, l: &mut Landscape, search: &mut String) {
    section(ui, "Distributed Plant");
    let mut on = l.distribution.is_some();
    if ui
        .checkbox(&mut on, "Spread a plant over the bed")
        .changed()
    {
        l.distribution = on.then(Distribution::default);
    }
    let count = l.distributed_positions().len();
    let Some(d) = l.distribution.as_mut() else {
        ui.weak("Copies of a plant image are scattered inside the bed at the spacing you choose.");
        return;
    };
    row(ui, "Plant", |ui| {
        let name = crate::tools::terrain::plant_choices()
            .iter()
            .find(|p| p.id == d.plant)
            .map_or_else(|| "None chosen".to_string(), |p| p.name.clone());
        ui.label(name)
    });
    row(ui, "Search", |ui| ui.text_edit_singleline(search));
    egui::ScrollArea::vertical()
        .id_salt("distributed_plants")
        .max_height(120.0)
        .show(ui, |ui| {
            for p in plants_in("", search).iter().take(40) {
                if ui.selectable_label(d.plant == p.id, &p.name).clicked() {
                    d.plant = p.id.clone();
                    d.size = p.width;
                    d.height = p.height;
                }
            }
        });
    fields.length_row(ui, "Spacing", "dist_space", &mut d.spacing);
    fields.length_row(ui, "Plant width", "dist_size", &mut d.size);
    fields.length_row(ui, "Plant height", "dist_height", &mut d.height);
    fields.length_row(ui, "Edge margin", "dist_margin", &mut d.margin);
    ui.checkbox(&mut d.stagger, "Stagger alternate rows");
    ui.weak(format!("{count} plants in the bed"));
}

/// Plant Specification > Plant Image.
pub fn plant_image_panel(ui: &mut Ui, fields: &mut Fields, l: &mut Landscape) {
    section(ui, "Plant Image");
    let mut on = l.image.is_some();
    if ui.checkbox(&mut on, "Draw the plants as images").changed() {
        l.image = on.then(|| {
            let mut img =
                PlantImage::sized("", l.size.max(1.0), l.height.max(1.0), is_conifer(&l.plant));
            img.seasons = default_seasons(is_conifer(&l.plant));
            img
        });
    }
    let (size, height) = (l.size, l.height);
    let Some(img) = l.image.as_mut() else {
        ui.weak("A plant image stands in 3D as a billboard that changes with the season.");
        return;
    };
    row(ui, "Image file", |ui| {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(&mut img.file);
            if ui.button("Browse\u{2026}").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Images", &["png", "jpg", "jpeg", "gif", "bmp"])
                    .pick_file()
                {
                    img.file = path.display().to_string();
                }
            }
        })
    });
    row(ui, "2D plant symbol", |ui| {
        ui.text_edit_singleline(&mut img.symbol_2d)
    });
    let mut width = img.width;
    if fields.length_row(ui, "Width", "img_w", &mut width) && width > 0.0 {
        img.set_width(width);
    }
    let mut h = img.height;
    if fields.length_row(ui, "Height", "img_h", &mut h) && h > 0.0 {
        img.set_height(h);
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut img.retain_aspect, "Retain aspect ratio");
        if ui.button("Reset original aspect ratio").clicked() {
            img.reset_aspect();
        }
    });
    row(ui, "Elevation reference", |ui| {
        ui.radio_value(&mut img.elevation_to_top, true, "to Top");
        ui.radio_value(&mut img.elevation_to_top, false, "to Bottom");
    });
    fields.length_row(ui, "Elevation", "img_z", &mut img.elevation);
    fields.length_row(ui, "Center point X", "img_cx", &mut img.center.x);
    fields.length_row(ui, "Center point Y", "img_cy", &mut img.center.y);
    ui.checkbox(&mut img.reverse, "Reverse image");
    ui.checkbox(&mut img.faces_camera, "Image always faces the camera");
    row(ui, "Copyright", |ui| {
        ui.text_edit_singleline(&mut img.copyright)
    });
    let mut transparent = img.transparent_color.is_some();
    if ui.checkbox(&mut transparent, "Transparent color").changed() {
        img.transparent_color = transparent.then_some([255, 255, 255]);
    }
    if let Some(c) = img.transparent_color.as_mut() {
        row(ui, "Color", |ui| ui.color_edit_button_srgb(c));
    }
    ui.add_space(4.0);
    ui.strong("Seasons");
    for (i, season) in Season::ALL.into_iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(season.name());
            ui.color_edit_button_srgb(&mut img.seasons[i].tint);
            ui.add(
                egui::DragValue::new(&mut img.seasons[i].foliage)
                    .range(0.0..=1.0)
                    .speed(0.01)
                    .prefix("foliage "),
            );
        });
    }
    if (img.width - size).abs() > 1e-9 || (img.height - height).abs() > 1e-9 {
        ui.weak("The image size sets the plant's canopy width and height.");
    }
    // The run's own size follows the image.
    l.size = img.width;
    l.height = img.height;
}
