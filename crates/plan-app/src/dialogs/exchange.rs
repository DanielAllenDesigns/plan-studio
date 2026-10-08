//! File exchange, CAD to Walls and the framing windows:
//!
//! * File > Export > DXF... (`plan_core::write_dxf` for the active floor) and
//!   Elevation DXF... (`plan_elevation` drawings of the four sides).
//! * File > Import > Import Drawing (DXF)... (`plan_import::parse_dxf`, a
//!   units choice, then CAD objects on the active floor).
//! * CAD > CAD to Walls... (`plan_import::cad_to_walls` on the selected CAD
//!   lines or the lines of one layer, with a preview count).
//! * Build > Framing: Build Framing, Build All Framing, Delete Framing, and
//!   the lumber Takeoff window with CSV export (Tools > Schedules).
//!
//! Every command that changes the plan is one undo step. The windows live in
//! a thread-local like `build_tools`, so the shell calls [`dispatch`] for the
//! menu actions and [`show_all`] once a frame.

use crate::editor::framing_view;
use crate::editor::{EditorContext, ObjectRef};
use crate::toolbar::{FileCommand, FramingCommand};
use eframe::egui::{self, Align2};
use plan_core::cad::{CadItem, CadObject};
use plan_core::geometry::Point;
use plan_core::{Layer, WallKind};
use plan_elevation::{elevation_from_project, Options, ViewDir};
use plan_import::{
    cad_to_walls, parse_dxf, to_cad_objects, to_inches_factor, CadToWallsOptions, CadToWallsResult,
    DxfDrawing, DxfUnits,
};
use std::cell::RefCell;
use std::path::Path;

// ===================================================================
// Export
// ===================================================================

/// The active floor as an R12 ASCII DXF.
pub fn floor_dxf(cx: &mut EditorContext) -> String {
    cx.refresh();
    plan_core::write_dxf(&cx.project, cx.floor, &cx.rooms)
}

/// The four exterior elevations (Front, Back, Left, Right) as one DXF, side
/// by side with a caption under each. `None` when the plan has nothing to
/// draw.
pub fn elevations_dxf(project: &plan_core::Project) -> Option<String> {
    const GAP: f64 = 120.0;
    let opts = Options::default();
    let mut sheet = plan_core::Project::new("Elevations");
    let mut x = 0.0;
    let mut any = false;
    for (name, dir) in [
        ("Front Elevation", ViewDir::Front),
        ("Back Elevation", ViewDir::Back),
        ("Left Elevation", ViewDir::Left),
        ("Right Elevation", ViewDir::Right),
    ] {
        let drawing = elevation_from_project(project, dir, &opts);
        if drawing.lines.is_empty() {
            continue;
        }
        any = true;
        let (lo, hi) = drawing.bounds;
        let shift = Point::new(x - lo.x, -lo.y);
        for o in drawing.to_cad(name) {
            if let CadItem::Line { a, b } = o.item {
                sheet.add_cad(
                    0,
                    o.layer,
                    CadItem::Line {
                        a: a.add(shift),
                        b: b.add(shift),
                    },
                );
            }
        }
        sheet.add_cad(
            0,
            "Elevation Labels",
            CadItem::Text {
                pos: Point::new(x, -24.0),
                text: name.to_uppercase(),
                height: 6.0,
                angle: 0.0,
            },
        );
        x += (hi.x - lo.x) + GAP;
    }
    any.then(|| plan_core::write_dxf(&sheet, 0, &[]))
}

fn save_text(default_name: &str, ext: &str, text: &str) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_name)
        .add_filter(ext, &[ext])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    write_file(&path, text)
}

fn write_file(path: &Path, text: &str) -> String {
    match std::fs::write(path, text.as_bytes()) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

// ===================================================================
// Import
// ===================================================================

/// The Units choices of the import window.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UnitsChoice {
    /// What the file declares (inches when it declares none).
    FromFile,
    Units(DxfUnits),
}

impl UnitsChoice {
    pub const ALL: [UnitsChoice; 6] = [
        UnitsChoice::FromFile,
        UnitsChoice::Units(DxfUnits::Inches),
        UnitsChoice::Units(DxfUnits::Feet),
        UnitsChoice::Units(DxfUnits::Millimeters),
        UnitsChoice::Units(DxfUnits::Centimeters),
        UnitsChoice::Units(DxfUnits::Meters),
    ];

    pub fn label(self) -> &'static str {
        match self {
            UnitsChoice::FromFile => "As the file says",
            UnitsChoice::Units(DxfUnits::Unitless) => "Unitless (inches)",
            UnitsChoice::Units(DxfUnits::Inches) => "Inches",
            UnitsChoice::Units(DxfUnits::Feet) => "Feet",
            UnitsChoice::Units(DxfUnits::Millimeters) => "Millimeters",
            UnitsChoice::Units(DxfUnits::Centimeters) => "Centimeters",
            UnitsChoice::Units(DxfUnits::Meters) => "Meters",
        }
    }

    fn override_units(self) -> Option<DxfUnits> {
        match self {
            UnitsChoice::FromFile => None,
            UnitsChoice::Units(u) => Some(u),
        }
    }

    /// Plan inches per drawing unit.
    pub fn factor(self, drawing: &DxfDrawing) -> f64 {
        to_inches_factor(drawing.units, self.override_units())
    }
}

/// Adds the drawing to the active floor as one undo step; layers the plan
/// does not have yet are created (hidden when the DXF layer is off). Returns
/// `(objects, new layers)`.
pub fn import_drawing(
    cx: &mut EditorContext,
    drawing: &DxfDrawing,
    units: UnitsChoice,
    layer_prefix: &str,
) -> (usize, usize) {
    let objects = to_cad_objects(drawing, units.factor(drawing), layer_prefix);
    if objects.is_empty() {
        return (0, 0);
    }
    cx.begin_change("Import Drawing");
    let mut new_layers = 0;
    for o in &objects {
        if cx.project.layers.get(&o.layer).is_some() {
            continue;
        }
        let dxf_name = o.layer.strip_prefix(layer_prefix).unwrap_or(&o.layer);
        let visible = drawing
            .layers
            .iter()
            .find(|l| l.name == dxf_name)
            .is_none_or(|l| l.visible);
        let mut layer = Layer::new(o.layer.clone(), [60, 60, 60], 18);
        layer.display = visible;
        if cx.project.layers.add(layer) {
            new_layers += 1;
        }
    }
    let n = plan_import::apply_cad(&mut cx.project, cx.floor, &objects).len();
    cx.mark_dirty();
    (n, new_layers)
}

struct ImportWindow {
    file: String,
    drawing: DxfDrawing,
    units: UnitsChoice,
    prefix: String,
}

impl ImportWindow {
    /// Plan size of the drawing's declared extents, for the summary line.
    fn extent_text(&self, cx: &EditorContext) -> Option<String> {
        let (lo, hi) = self.drawing.extents?;
        let k = self.units.factor(&self.drawing);
        Some(format!(
            "{} x {}",
            cx.fmt_dim((hi.x - lo.x).abs() * k),
            cx.fmt_dim((hi.y - lo.y).abs() * k)
        ))
    }
}

fn start_import(cx: &mut EditorContext) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("DXF drawing", &["dxf"])
        .pick_file()
    else {
        return;
    };
    match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|t| parse_dxf(&t).map_err(|e| e.to_string()))
    {
        Ok(drawing) => {
            let file = path
                .file_name()
                .map_or_else(|| "drawing".into(), |n| n.to_string_lossy().into_owned());
            with_windows(|w| {
                w.import = Some(ImportWindow {
                    file,
                    drawing,
                    units: UnitsChoice::FromFile,
                    prefix: String::new(),
                })
            });
        }
        Err(e) => cx.status = format!("Import failed: {e}"),
    }
}

// ===================================================================
// CAD to Walls
// ===================================================================

/// Where CAD to Walls takes its lines from.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum LineSource {
    /// The CAD lines (and polylines) in the selection.
    Selected,
    /// Every line and polyline on this layer of the active floor.
    Layer(String),
}

/// The segments of a CAD item that can be wall faces: a line, or the
/// segments of a polyline.
pub fn segments_of(item: &CadItem) -> Vec<(Point, Point)> {
    match item {
        CadItem::Line { a, b } => vec![(*a, *b)],
        CadItem::Polyline { points, closed } => {
            let mut v: Vec<(Point, Point)> = points.windows(2).map(|w| (w[0], w[1])).collect();
            if *closed && points.len() > 2 {
                v.push((points[points.len() - 1], points[0]));
            }
            v
        }
        _ => Vec::new(),
    }
}

/// The segments `source` stands for on the active floor.
pub fn collect_lines(cx: &EditorContext, source: &LineSource) -> Vec<(Point, Point)> {
    let selected: Vec<_> = cx
        .selection
        .items
        .iter()
        .filter_map(|o| match o {
            ObjectRef::Cad(id) => Some(*id),
            _ => None,
        })
        .collect();
    cx.floor()
        .cad
        .iter()
        .filter(|c| match source {
            LineSource::Selected => selected.contains(&c.id),
            LineSource::Layer(name) => c.layer == *name,
        })
        .flat_map(|c: &CadObject| segments_of(&c.item))
        .collect()
}

/// Layers of the active floor that hold lines or polylines, with their
/// segment counts.
pub fn line_layers(cx: &EditorContext) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = Vec::new();
    for c in &cx.floor().cad {
        let n = segments_of(&c.item).len();
        if n == 0 {
            continue;
        }
        match out.iter_mut().find(|(l, _)| *l == c.layer) {
            Some((_, k)) => *k += n,
            None => out.push((c.layer.clone(), n)),
        }
    }
    out
}

/// Adds the proposed walls at the default height of their kind as one undo
/// step; returns how many were added.
pub fn apply_proposals(cx: &mut EditorContext, result: &CadToWallsResult) -> usize {
    if result.walls.is_empty() {
        return 0;
    }
    cx.begin_change("CAD to Walls");
    let floor = cx.floor;
    for w in &result.walls {
        let height = cx.wall_height(w.kind);
        cx.project
            .add_wall(floor, w.start, w.end, w.thickness, height, w.kind);
    }
    cx.mark_dirty();
    result.walls.len()
}

struct CadWallsWindow {
    source: LineSource,
    layers: Vec<(String, usize)>,
    selected_lines: usize,
    opts: CadToWallsOptions,
    result: Option<CadToWallsResult>,
    lines: usize,
}

impl CadWallsWindow {
    fn new(cx: &EditorContext) -> Self {
        let layers = line_layers(cx);
        let selected_lines = collect_lines(cx, &LineSource::Selected).len();
        let source = if selected_lines > 0 {
            LineSource::Selected
        } else {
            LineSource::Layer(
                layers
                    .iter()
                    .max_by_key(|(_, n)| *n)
                    .map_or_else(String::new, |(l, _)| l.clone()),
            )
        };
        let mut w = Self {
            source,
            layers,
            selected_lines,
            opts: CadToWallsOptions::default(),
            result: None,
            lines: 0,
        };
        w.preview(cx);
        w
    }

    fn preview(&mut self, cx: &EditorContext) {
        let lines = collect_lines(cx, &self.source);
        self.lines = lines.len();
        self.result = Some(cad_to_walls(&lines, &self.opts));
    }
}

// ===================================================================
// Windows host
// ===================================================================

#[derive(Default)]
struct Windows {
    import: Option<ImportWindow>,
    cad_walls: Option<CadWallsWindow>,
    /// The Framing Takeoff window: `Some(all_floors)`.
    takeoff: Option<bool>,
}

thread_local! {
    static WINDOWS: RefCell<Windows> = RefCell::new(Windows::default());
}

fn with_windows<R>(f: impl FnOnce(&mut Windows) -> R) -> R {
    WINDOWS.with(|w| f(&mut w.borrow_mut()))
}

/// File > Export / Import and CAD > CAD to Walls.
pub fn dispatch_file(cx: &mut EditorContext, c: FileCommand) {
    match c {
        FileCommand::ExportDxf => {
            let text = floor_dxf(cx);
            let name = format!("{} - {}.dxf", cx.project.name, cx.floor().name);
            cx.status = save_text(&name, "dxf", &text);
        }
        FileCommand::ExportElevationsDxf => match elevations_dxf(&cx.project) {
            Some(text) => {
                let name = format!("{} Elevations.dxf", cx.project.name);
                cx.status = save_text(&name, "dxf", &text);
            }
            None => cx.status = "There is nothing to draw an elevation of".into(),
        },
        FileCommand::ImportDxf => start_import(cx),
        FileCommand::CadToWalls => {
            cx.refresh();
            let w = CadWallsWindow::new(cx);
            if w.layers.is_empty() && w.selected_lines == 0 {
                cx.status = "CAD to Walls: there are no CAD lines on this floor".into();
            } else {
                with_windows(|win| win.cad_walls = Some(w));
            }
        }
    }
}

/// Build > Framing commands and the takeoff window.
pub fn dispatch_framing(cx: &mut EditorContext, c: FramingCommand) {
    match c {
        FramingCommand::Build => {
            framing_view::build(cx, false);
        }
        FramingCommand::BuildAll => {
            framing_view::build(cx, true);
        }
        FramingCommand::Delete => {
            framing_view::clear(cx);
        }
        FramingCommand::Takeoff => with_windows(|w| w.takeoff = Some(false)),
    }
}

/// Draws the open windows; call once a frame.
pub fn show_all(ctx: &egui::Context, cx: &mut EditorContext) {
    let mut w = with_windows(std::mem::take);
    w.show(ctx, cx);
    with_windows(|slot| {
        // A command run while the windows were out may have opened new ones.
        let newer = std::mem::take(slot);
        *slot = w;
        if newer.import.is_some() {
            slot.import = newer.import;
        }
        if newer.cad_walls.is_some() {
            slot.cad_walls = newer.cad_walls;
        }
        if newer.takeoff.is_some() {
            slot.takeoff = newer.takeoff;
        }
    });
}

impl Windows {
    fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if let Some(mut win) = self.import.take() {
            if import_window(ctx, cx, &mut win) {
                self.import = Some(win);
            }
        }
        if let Some(mut win) = self.cad_walls.take() {
            if cad_walls_window(ctx, cx, &mut win) {
                self.cad_walls = Some(win);
            }
        }
        if let Some(all) = self.takeoff {
            self.takeoff = takeoff_window(ctx, cx, all);
        }
    }
}

fn import_window(ctx: &egui::Context, cx: &mut EditorContext, w: &mut ImportWindow) -> bool {
    let mut open = true;
    let mut import = false;
    egui::Window::new("Import Drawing (DXF)")
        .id(egui::Id::new("import_dxf"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.strong(&w.file);
            let d = &w.drawing;
            ui.label(format!(
                "{} entities on {} layers; the file's units: {}",
                d.entities.len(),
                d.layers.len(),
                UnitsChoice::Units(d.units).label()
            ));
            if !d.skipped.is_empty() {
                let skipped: Vec<String> =
                    d.skipped.iter().map(|(k, n)| format!("{n} {k}")).collect();
                ui.weak(format!("Not imported: {}", skipped.join(", ")));
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Units");
                egui::ComboBox::from_id_salt("import_units")
                    .selected_text(w.units.label())
                    .show_ui(ui, |ui| {
                        for u in UnitsChoice::ALL {
                            ui.selectable_value(&mut w.units, u, u.label());
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Layer name prefix");
                ui.add(egui::TextEdit::singleline(&mut w.prefix).desired_width(120.0));
            });
            if let Some(t) = w.extent_text(cx) {
                ui.label(format!("Size in the plan: {t}"));
            }
            ui.weak(format!("Goes onto {} as CAD objects.", cx.floor().name));
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    import = true;
                }
            });
        });
    if import {
        let (n, layers) = import_drawing(cx, &w.drawing, w.units, &w.prefix);
        cx.status = if n == 0 {
            "The drawing had nothing to import".into()
        } else {
            format!("Imported {n} objects ({layers} new layers)")
        };
        return false;
    }
    open
}

fn cad_walls_window(ctx: &egui::Context, cx: &mut EditorContext, w: &mut CadWallsWindow) -> bool {
    let mut open = true;
    let mut apply = false;
    let mut changed = false;
    egui::Window::new("CAD to Walls")
        .id(egui::Id::new("cad_to_walls"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.label("Pairs of parallel lines become walls.");
            ui.separator();
            changed |= ui
                .radio_value(
                    &mut w.source,
                    LineSource::Selected,
                    format!("Selected CAD lines ({})", w.selected_lines),
                )
                .clicked();
            let on_layer = matches!(w.source, LineSource::Layer(_));
            ui.horizontal(|ui| {
                if ui.radio(on_layer, "All lines on layer").clicked() && !on_layer {
                    w.source = LineSource::Layer(
                        w.layers.first().map(|(l, _)| l.clone()).unwrap_or_default(),
                    );
                    changed = true;
                }
                let cur = match &w.source {
                    LineSource::Layer(l) => l.clone(),
                    LineSource::Selected => String::new(),
                };
                ui.add_enabled_ui(on_layer, |ui| {
                    egui::ComboBox::from_id_salt("cad_walls_layer")
                        .selected_text(cur.clone())
                        .show_ui(ui, |ui| {
                            for (l, n) in &w.layers {
                                if ui
                                    .selectable_label(cur == *l, format!("{l} ({n})"))
                                    .clicked()
                                {
                                    w.source = LineSource::Layer(l.clone());
                                    changed = true;
                                }
                            }
                        });
                });
            });
            ui.separator();
            let o = &mut w.opts;
            for (label, v) in [
                ("Thinnest wall", &mut o.min_thickness),
                ("Thickest wall", &mut o.max_thickness),
                ("Shortest wall", &mut o.min_length),
                ("Snap corners within", &mut o.snap_tolerance),
                ("Exterior from thickness", &mut o.exterior_threshold),
            ] {
                ui.horizontal(|ui| {
                    ui.label(label);
                    changed |= ui
                        .add(
                            egui::DragValue::new(v)
                                .speed(0.25)
                                .range(0.0..=240.0)
                                .suffix("\""),
                        )
                        .changed();
                });
            }
            ui.separator();
            if ui.button("Preview").clicked() {
                changed = true;
            }
            match &w.result {
                Some(r) => {
                    let ext = r.walls.iter().filter(|p| p.kind == WallKind::Exterior).count();
                    ui.label(format!(
                        "{} lines give {} walls ({} exterior, {} interior); {} line pieces left over.",
                        w.lines,
                        r.walls.len(),
                        ext,
                        r.walls.len() - ext,
                        r.unmatched.len()
                    ));
                }
                None => {
                    ui.weak("Press Preview.");
                }
            }
            ui.separator();
            let n = w.result.as_ref().map_or(0, |r| r.walls.len());
            if ui
                .add_enabled(n > 0, egui::Button::new(format!("Create {n} Walls")))
                .clicked()
            {
                apply = true;
            }
        });
    if changed {
        w.preview(cx);
    }
    if apply {
        if let Some(r) = w.result.take() {
            let n = apply_proposals(cx, &r);
            cx.status = format!("Created {n} walls from CAD lines");
        }
        return false;
    }
    open
}

/// The framing takeoff table. Returns the state to keep (`None` once closed).
fn takeoff_window(ctx: &egui::Context, cx: &mut EditorContext, mut all: bool) -> Option<bool> {
    let mut open = true;
    let mut export = false;
    let members = framing_view::members_for(&cx.project, cx.floor, all);
    egui::Window::new("Framing Takeoff")
        .id(egui::Id::new("framing_takeoff"))
        .open(&mut open)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut all, false, format!("{} only", cx.floor().name));
                ui.radio_value(&mut all, true, "All floors");
            });
            ui.separator();
            if members.is_empty() {
                ui.weak("No framing yet. Use Build > Framing > Build Framing.");
            } else {
                let (cols, rows) = framing_view::takeoff_table(&members);
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        egui::Grid::new("framing_takeoff_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                for c in &cols {
                                    ui.strong(c);
                                }
                                ui.end_row();
                                for r in &rows {
                                    for c in r {
                                        ui.label(c);
                                    }
                                    ui.end_row();
                                }
                            });
                    });
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(format!("{} members", members.len()));
                export = ui
                    .add_enabled(!members.is_empty(), egui::Button::new("Export CSV\u{2026}"))
                    .clicked();
            });
        });
    if export {
        cx.status = save_text(
            "framing_takeoff.csv",
            "csv",
            &framing_view::takeoff_csv(&members),
        );
    }
    open.then_some(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn box_house(cx: &mut EditorContext) {
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 6.5, 109.125, WallKind::Exterior);
        }
        cx.refresh();
    }

    #[test]
    fn exported_dxf_round_trips_through_the_importer() {
        let mut cx = cx();
        box_house(&mut cx);
        let text = floor_dxf(&mut cx);
        let drawing = parse_dxf(&text).unwrap();
        assert!(!drawing.entities.is_empty());
        let (n, new_layers) = import_drawing(&mut cx, &drawing, UnitsChoice::FromFile, "DXF: ");
        assert!(n >= 4, "{n}");
        assert!(new_layers >= 1);
        assert!(cx.floor().cad.iter().all(|c| c.layer.starts_with("DXF: ")));
        // One undo step takes the import (objects and layers) away.
        assert_eq!(cx.undo_label(), Some("Import Drawing"));
        let layers_after = cx.project.layers.layers.len();
        cx.undo();
        assert!(cx.floor().cad.is_empty());
        assert!(cx.project.layers.layers.len() < layers_after);
    }

    #[test]
    fn import_units_scale_the_drawing() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nA\n10\n0\n20\n0\n11\n1000\n21\n0\n0\nENDSEC\n0\nEOF\n";
        let d = parse_dxf(dxf).unwrap();
        let mut cx = cx();
        import_drawing(&mut cx, &d, UnitsChoice::Units(DxfUnits::Millimeters), "");
        let CadItem::Line { b, .. } = &cx.floor().cad[0].item else {
            panic!("expected a line");
        };
        assert!((b.x - 1000.0 / 25.4).abs() < 1e-6, "{}", b.x);
        assert_eq!(UnitsChoice::FromFile.factor(&d), 1.0);
    }

    #[test]
    fn cad_to_walls_from_a_layer_or_the_selection() {
        let mut cx = cx();
        // Two parallel 120" lines 6" apart make one wall.
        let a = cx.project.add_cad(
            0,
            "Plan",
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(120.0, 0.0),
            },
        );
        cx.project.add_cad(
            0,
            "Plan",
            CadItem::Line {
                a: Point::new(0.0, 6.0),
                b: Point::new(120.0, 6.0),
            },
        );
        cx.project.add_cad(
            0,
            "Other",
            CadItem::Polyline {
                points: vec![Point::new(0.0, 500.0), Point::new(10.0, 500.0)],
                closed: false,
            },
        );
        assert_eq!(
            line_layers(&cx),
            vec![("Plan".to_string(), 2), ("Other".to_string(), 1)]
        );

        let lines = collect_lines(&cx, &LineSource::Layer("Plan".into()));
        assert_eq!(lines.len(), 2);
        let result = cad_to_walls(&lines, &CadToWallsOptions::default());
        assert_eq!(result.walls.len(), 1);
        assert!(collect_lines(&cx, &LineSource::Selected).is_empty());
        cx.selection.set(ObjectRef::Cad(a));
        assert_eq!(collect_lines(&cx, &LineSource::Selected).len(), 1);

        assert_eq!(apply_proposals(&mut cx, &result), 1);
        assert_eq!(cx.floor().walls.len(), 1);
        assert!((cx.floor().walls[0].thickness - 6.0).abs() < 1e-6);
        assert_eq!(cx.undo_label(), Some("CAD to Walls"));
        cx.undo();
        assert!(cx.floor().walls.is_empty());
        cx.selection.clear();
        // The window opens on the busiest layer and previews its count.
        let w = CadWallsWindow::new(&cx);
        assert_eq!(w.source, LineSource::Layer("Plan".into()));
        assert_eq!(w.result.as_ref().unwrap().walls.len(), 1);
    }

    #[test]
    fn closed_polylines_give_all_their_sides() {
        let sq = CadItem::Polyline {
            points: vec![
                Point::new(0.0, 0.0),
                Point::new(10.0, 0.0),
                Point::new(10.0, 10.0),
                Point::new(0.0, 10.0),
            ],
            closed: true,
        };
        assert_eq!(segments_of(&sq).len(), 4);
        assert!(segments_of(&CadItem::Circle {
            center: Point::ZERO,
            radius: 1.0
        })
        .is_empty());
    }

    #[test]
    fn elevations_export_needs_a_building() {
        let mut cx = cx();
        assert!(elevations_dxf(&cx.project).is_none());
        box_house(&mut cx);
        let text = elevations_dxf(&cx.project).expect("a box has elevations");
        let d = parse_dxf(&text).unwrap();
        assert!(!d.entities.is_empty());
        for name in ["Front Elevation", "Right Elevation"] {
            assert!(d.layers.iter().any(|l| l.name.starts_with(name)), "{name}");
        }
    }

    #[test]
    fn windows_draw_without_panicking() {
        let ctx = egui::Context::default();
        let mut cx = cx();
        box_house(&mut cx);
        crate::editor::framing_view::build(&mut cx, false);
        cx.project.add_cad(
            0,
            "Plan",
            CadItem::Line {
                a: Point::new(0.0, 0.0),
                b: Point::new(120.0, 0.0),
            },
        );
        dispatch_file(&mut cx, FileCommand::CadToWalls);
        dispatch_framing(&mut cx, FramingCommand::Takeoff);
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| show_all(ctx, &mut cx));
        }
        with_windows(|w| {
            assert!(w.cad_walls.is_some());
            assert_eq!(w.takeoff, Some(false));
        });
        with_windows(|w| *w = Windows::default());
    }
}
