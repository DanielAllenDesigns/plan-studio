//! File exchange, CAD to Walls and the framing windows:
//!
//! * File > Export > DXF... (`plan_core::write_dxf` for the active floor) and
//!   Elevation DXF... (`plan_elevation` drawings of the four sides).
//! * File > Import > Import Drawing (DWG/DXF)... is `dialogs::import_drawing`
//!   (the Import Drawing Assistant); this file keeps the plain-options
//!   import ([`import_drawing_with`]) the scenarios use.
//! * CAD > CAD to Walls... (`plan_import::cad_to_walls` on the selected CAD
//!   lines or the lines of one layer, with a preview count).
//! * Build > Framing: Build Framing, Build All Framing, Delete Framing, and
//!   the lumber Takeoff window with CSV export (Tools > Schedules).
//! * File > Templates > Import Chief Template...: the decode summary of a
//!   Chief `.plan` / `.layout` with Import into My Defaults and Set as default
//!   plan / layout template. Also draws the Preferences > Templates page.
//!
//! Every command that changes the plan is one undo step. The windows live in
//! a thread-local like `build_tools`, so the shell calls [`dispatch`] for the
//! menu actions and [`show_all`] once a frame.

use crate::editor::framing_view;
use crate::editor::{EditorContext, ObjectRef};
use crate::templates;
use crate::toolbar::{FileCommand, FramingCommand};
use eframe::egui::{self, Align2};
use plan_core::cad::{CadItem, CadObject};
use plan_core::geometry::Point;
use plan_core::{Layer, WallKind};
use plan_elevation::{elevation, Options, ViewDir};
use plan_import::{
    cad_to_walls, to_cad_objects_with, to_inches_factor, CadToWallsOptions, CadToWallsResult,
    DxfDrawing, DxfUnits, ImportOptions,
};
#[cfg(test)]
use plan_import::{parse_dxf, LayerMapping, LayerTarget};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

// ===================================================================
// Export
// ===================================================================

/// The active floor as an R12 ASCII DXF, with the roof plane outlines on layer
/// "Roof Planes" and the manual framing on layer "Framing".
pub fn floor_dxf(cx: &mut EditorContext) -> String {
    cx.refresh();
    let set = &cx.defaults.dimensions;
    let annotation = plan_core::DxfAnnotation {
        inches_per_foot: cx.sheet.scale.inches_per_foot(),
        dim_text_style: set.text_style.clone(),
        dim_printed_size: set.printed_size,
        dim_format: Some(cx.dim_format()),
    };
    // The schedules placed on the floor go along as tables on their layer.
    let with_tables =
        project_with_schedules(&cx.project, cx.floor, &cx.rooms, annotation.inches_per_foot);
    // Cabinets, placed symbols and stairs (and the treads seen through a
    // stairwell, on "Stairs, Hidden") go along as lines and texts.
    let with_overlay = project_with_overlay(with_tables, cx.floor);
    framing_view::floor_dxf_with(&with_overlay, cx.floor, &cx.rooms, annotation)
}

/// `project` with the floor's cabinets, symbols and stairs as CAD on their
/// layers (see [`crate::editor::plan_overlay::dxf_items`]); the plan itself
/// is not touched.
pub fn project_with_overlay(mut project: plan_core::Project, floor: usize) -> plan_core::Project {
    for (layer, item) in crate::editor::plan_overlay::dxf_items(&project, floor) {
        project.add_cad(floor, layer, item);
    }
    project
}

/// Paper size of a DXF schedule table's row and text, inches (like the
/// tables of a layout page: 1/4" rows, 8 pt text).
const TABLE_ROW_IN: f64 = 0.25;
const TABLE_TEXT_IN: f64 = 8.0 / 72.0;

/// The schedules placed on `floor` as CAD: for each, its title, a header and
/// one line per row in a ruled grid hanging from the schedule's position, on
/// the schedule's layer. `inches_per_foot` is the sheet scale (paper inches
/// per plan foot) that turns the table's paper size into plan inches; `0`
/// uses 1/4" = 1'.
pub fn schedule_tables_cad(
    project: &plan_core::Project,
    floor: usize,
    rooms: &[plan_core::Room],
    inches_per_foot: f64,
) -> Vec<(String, CadItem)> {
    use plan_core::schedules::ScheduleLayer;
    let Some(f) = project.floors.get(floor) else {
        return Vec::new();
    };
    // Plan inches per paper inch.
    let k = 12.0
        / if inches_per_foot > 0.0 {
            inches_per_foot
        } else {
            0.25
        };
    let (row_h, text_h) = (TABLE_ROW_IN * k, TABLE_TEXT_IN * k);
    let mut out = Vec::new();
    for def in ScheduleLayer::load(f).schedules {
        let t = plan_docs::schedule_kinds::table(project, &def, floor, Some((floor, rooms)));
        let widths: Vec<f64> = (0..t.columns.len())
            .map(|c| {
                let longest = std::iter::once(plan_docs::PdfDoc::text_width(&t.columns[c], 8.0))
                    .chain(
                        t.rows
                            .iter()
                            .filter_map(|r| r.get(c))
                            .map(|v| plan_docs::PdfDoc::text_width(v, 8.0)),
                    )
                    .fold(0.0, f64::max);
                (longest / 72.0 + 0.14) * k
            })
            .collect();
        let total: f64 = widths.iter().sum();
        let at = def.position;
        let layer = def.layer.clone();
        let line = |a: Point, b: Point| (layer.clone(), CadItem::Line { a, b });
        let text = |x: f64, y: f64, s: &str| {
            (
                layer.clone(),
                CadItem::Text {
                    pos: Point::new(x, y),
                    text: s.to_string(),
                    height: text_h,
                    angle: 0.0,
                },
            )
        };
        // Title above the grid, then the header row and the data rows.
        out.push(text(at.x, at.y + 0.3 * row_h, &t.title));
        let n_rows = t.rows.len() + 1;
        for r in 0..=n_rows {
            let y = at.y - row_h * r as f64;
            out.push(line(Point::new(at.x, y), Point::new(at.x + total, y)));
        }
        let bottom = at.y - row_h * n_rows as f64;
        let mut x = at.x;
        for (c, w) in widths.iter().enumerate() {
            out.push(line(Point::new(x, at.y), Point::new(x, bottom)));
            out.push(text(x + 0.07 * k, at.y - 0.75 * row_h, &t.columns[c]));
            for (r, row) in t.rows.iter().enumerate() {
                if let Some(cell) = row.get(c) {
                    out.push(text(
                        x + 0.07 * k,
                        at.y - row_h * (r + 2) as f64 + 0.25 * row_h,
                        cell,
                    ));
                }
            }
            x += w;
        }
        out.push(line(Point::new(x, at.y), Point::new(x, bottom)));
    }
    out
}

/// A copy of `project` whose `floor` also holds its placed schedules as CAD
/// (see [`schedule_tables_cad`]); the plan itself is not touched.
fn project_with_schedules(
    project: &plan_core::Project,
    floor: usize,
    rooms: &[plan_core::Room],
    inches_per_foot: f64,
) -> plan_core::Project {
    let tables = schedule_tables_cad(project, floor, rooms, inches_per_foot);
    if tables.is_empty() {
        return project.clone();
    }
    let mut copy = project.clone();
    for (layer, item) in tables {
        copy.add_cad(floor, layer, item);
    }
    copy
}

/// The four exterior elevations (Front, Back, Left, Right) as one DXF, side
/// by side with a caption under each. `None` when the plan has nothing to
/// draw.
pub fn elevations_dxf(project: &plan_core::Project) -> Option<String> {
    const GAP: f64 = 120.0;
    let opts = Options::default();
    // The 3D view's own scene, so roofs, stairs, dormers and the like appear
    // (and the plan's casing and sills).
    let scene = crate::shell::layout_window::view_scene(project);
    let mut sheet = plan_core::Project::new("Elevations");
    let mut x = 0.0;
    let mut any = false;
    for (name, dir) in [
        ("Front Elevation", ViewDir::Front),
        ("Back Elevation", ViewDir::Back),
        ("Left Elevation", ViewDir::Left),
        ("Right Elevation", ViewDir::Right),
    ] {
        let drawing = elevation(&scene, dir, &opts);
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

/// The plan's layout as JSON (pretty), or `None` when it has none.
pub fn export_layout_json(cx: &EditorContext) -> Option<String> {
    let layout = crate::shell::layout_window::load(&cx.project)?;
    serde_json::to_string_pretty(&layout).ok()
}

/// Opens a saved layout: `text` is a layout's JSON (or a whole plan's JSON,
/// whose layout is taken). It replaces the plan's layout as one undo step.
/// Returns the status message.
pub fn import_layout_json(cx: &mut EditorContext, text: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    // A whole plan file keeps its layout in "layout".
    let layout_value = match value.get("pages") {
        Some(_) => value,
        None => value
            .get("layout")
            .cloned()
            .ok_or("this file holds no layout")?,
    };
    let layout: plan_layout::Layout =
        serde_json::from_value(layout_value).map_err(|e| format!("not a layout: {e}"))?;
    let pages = layout.pages.len();
    let name = layout.name.clone();
    cx.begin_change("Import Layout");
    crate::shell::layout_window::store(&mut cx.project, &layout);
    cx.mark_dirty();
    Ok(format!(
        "Opened layout {name} ({pages} page{})",
        if pages == 1 { "" } else { "s" }
    ))
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

/// Asks for a file name and writes `bytes` there (an Excel workbook).
fn save_bytes(default_name: &str, ext: &str, bytes: &[u8]) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_name)
        .add_filter(ext, &[ext])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    match std::fs::write(&path, bytes) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
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

/// The Units choices of the plain import ([`import_drawing_with`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum UnitsChoice {
    /// What the file declares (inches when it declares none).
    FromFile,
    Units(DxfUnits),
}

#[cfg_attr(not(test), allow(dead_code))]
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
            UnitsChoice::Units(u) => u.label(),
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

/// What an import did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct ImportReport {
    /// CAD objects added.
    pub objects: usize,
    /// Plan layers created for them.
    pub new_layers: usize,
    /// Walls made by "Convert to walls".
    pub walls: usize,
}

/// Adds the drawing to the active floor as one undo step; layers the plan
/// does not have yet are created (hidden when the DXF layer is off). Returns
/// `(objects, new layers)`. (The window uses [`import_drawing_with`].)
#[cfg(test)]
pub fn import_drawing(
    cx: &mut EditorContext,
    drawing: &DxfDrawing,
    units: UnitsChoice,
    layer_prefix: &str,
) -> (usize, usize) {
    let opts = ImportOptions::new(units.factor(drawing), layer_prefix);
    let r = import_drawing_with(cx, drawing, &opts, None);
    (r.objects, r.new_layers)
}

/// [`import_drawing`] with the full options (layer mapping, scale, rotation,
/// insertion point) and, when `walls_from` names a plan layer, "Convert to
/// walls": the lines imported onto that layer go through the CAD to Walls
/// matcher with its default options and the walls join the same undo step.
#[cfg_attr(not(test), allow(dead_code))]
pub fn import_drawing_with(
    cx: &mut EditorContext,
    drawing: &DxfDrawing,
    opts: &ImportOptions,
    walls_from: Option<&str>,
) -> ImportReport {
    let objects = to_cad_objects_with(drawing, opts);
    if objects.is_empty() {
        return ImportReport::default();
    }
    cx.begin_change("Import Drawing");
    let mut new_layers = 0;
    // A layer the drawing switched off comes in hidden.
    let mut add_layer = |cx: &mut EditorContext, name: &str, visible: bool| {
        if cx.project.layers.get(name).is_some() {
            return;
        }
        let mut layer = Layer::new(name.to_string(), [60, 60, 60], 18);
        layer.display = visible;
        if cx.project.layers.add(layer) {
            new_layers += 1;
        }
    };
    for l in &drawing.layers {
        if let Some(target) = opts.target_layer(&l.name) {
            if objects.iter().any(|o| o.layer == target) {
                add_layer(cx, &target, l.visible);
            }
        }
    }
    for o in &objects {
        add_layer(cx, &o.layer, true);
    }
    let ids = plan_import::apply_cad(&mut cx.project, cx.floor, &objects);
    let mut walls = 0;
    if let Some(layer) = walls_from {
        let lines: Vec<(Point, Point)> = cx
            .floor()
            .cad
            .iter()
            .filter(|c| ids.contains(&c.id) && c.layer == layer)
            .flat_map(|c| segments_of(&c.item))
            .collect();
        let result = cad_to_walls(&lines, &CadToWallsOptions::default());
        walls = add_wall_proposals(cx, &result);
    }
    cx.mark_dirty();
    ImportReport {
        objects: ids.len(),
        new_layers,
        walls,
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
    add_wall_proposals(cx, result)
}

/// Adds the proposed walls inside the undo step the caller has begun.
fn add_wall_proposals(cx: &mut EditorContext, result: &CadToWallsResult) -> usize {
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
    chief_template: Option<ChiefTemplateWindow>,
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
        // The options window (units, layer names, weights, text, floors, 2D/3D)
        // comes first; its Export button writes the file.
        FileCommand::ExportDxf => super::dxf_options::open(cx),
        FileCommand::ExportElevationsDxf => match elevations_dxf(&cx.project) {
            Some(text) => {
                let name = format!("{} Elevations.dxf", cx.project.name);
                cx.status = save_text(&name, "dxf", &text);
            }
            None => cx.status = "There is nothing to draw an elevation of".into(),
        },
        FileCommand::ImportDxf => super::import_drawing::start(cx),
        FileCommand::ImportLayout => {
            let Some(path) = rfd::FileDialog::new()
                .add_filter("Layout (JSON)", &["json", "layout"])
                .pick_file()
            else {
                return;
            };
            cx.status = match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| import_layout_json(cx, &t))
            {
                Ok(msg) => msg,
                Err(e) => format!("Import failed: {e}"),
            };
        }
        FileCommand::ExportLayout => match export_layout_json(cx) {
            Some(text) => {
                let name = format!("{} Layout.json", cx.project.name);
                cx.status = save_text(&name, "json", &text);
            }
            None => cx.status = "The plan has no layout to export".into(),
        },
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
    super::export_picture::show(ctx, cx);
    super::dxf_options::show(ctx, cx);
    super::import_drawing::show(ctx, cx);
    super::symbol::show_import(ctx, cx);
    super::defaults::show_templates_page(ctx, cx);
    with_windows(|slot| {
        // A command run while the windows were out may have opened new ones.
        let newer = std::mem::take(slot);
        *slot = w;
        if newer.chief_template.is_some() {
            slot.chief_template = newer.chief_template;
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
        if let Some(mut win) = self.chief_template.take() {
            if chief_template_window(ctx, cx, &mut win) {
                self.chief_template = Some(win);
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

// ===================================================================
// Import Chief Template
// ===================================================================

struct ChiefTemplateWindow {
    path: PathBuf,
    preview: templates::Preview,
}

/// Decodes `path` (a Chief `.plan` / `.tpl` / `.layout`) and opens the
/// Import Chief Template window on it.
pub fn open_chief_template(cx: &mut EditorContext, path: &Path) {
    match templates::preview(path) {
        Ok(preview) => with_windows(|w| {
            w.chief_template = Some(ChiefTemplateWindow {
                path: path.to_path_buf(),
                preview,
            })
        }),
        Err(e) => cx.status = format!("Import failed: {e}"),
    }
}

/// Preferences > Templates: opens the page (drawn from [`show_all`]).
pub fn open_templates_page() {
    super::defaults::open_templates_page();
}

/// Seeds the defaults from the window's template and keeps them as the
/// user's template.
fn import_into_defaults(cx: &mut EditorContext, w: &ChiefTemplateWindow) {
    let seed = plan_chiefplan::seed_defaults(&w.preview.inventory, cx.defaults.clone());
    cx.defaults = seed.defaults.clone();
    let saved = crate::plan_defaults::save_user(&cx.defaults);
    cx.status = format!(
        "Imported {}: {} wall types, {} layers added{}",
        w.path.display(),
        seed.added_wall_types.len(),
        seed.added_layers.len(),
        match saved {
            Ok(_) => String::new(),
            Err(e) => format!(" (could not save your template: {e})"),
        }
    );
}

/// Makes the window's template the default plan (or layout) template.
fn set_as_default_template(cx: &mut EditorContext, w: &ChiefTemplateWindow) {
    let mut settings = templates::load_settings();
    if w.preview.is_layout {
        settings.layout = Some(w.path.clone());
    } else {
        settings.plan = Some(w.path.clone());
    }
    let (_, status) = super::defaults::apply_template_settings(cx, &settings, true);
    super::defaults::reload_templates_page();
    cx.status = status;
}

fn chief_template_window(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    w: &mut ChiefTemplateWindow,
) -> bool {
    let mut open = true;
    let mut import = false;
    let mut set_default = false;
    let kind = if w.preview.is_layout {
        "layout"
    } else {
        "plan"
    };
    egui::Window::new("Import Chief Template")
        .id(egui::Id::new("import_chief_template"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(420.0);
            ui.strong(
                w.path
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
            );
            ui.weak(w.path.display().to_string());
            ui.separator();
            for line in &w.preview.lines {
                ui.label(line);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Import into My Defaults").clicked() {
                    import = true;
                }
                if ui
                    .button(format!("Set as default {kind} template"))
                    .clicked()
                {
                    set_default = true;
                }
            });
        });
    if import {
        import_into_defaults(cx, w);
        return false;
    }
    if set_default {
        set_as_default_template(cx, w);
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
    let mut export_xlsx = false;
    let mut export_list = false;
    let data = framing_view::takeoff_data(&cx.project, cx.floor, all);
    // By member type with cut lengths (the framing schedule), or the lumber list.
    let by_member_id = egui::Id::new("framing_takeoff_by_member");
    let mut by_member = ctx
        .data(|d| d.get_temp::<bool>(by_member_id))
        .unwrap_or(true);
    egui::Window::new("Framing Takeoff")
        .id(egui::Id::new("framing_takeoff"))
        .open(&mut open)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.radio_value(&mut all, false, format!("{} only", cx.floor().name));
                ui.radio_value(&mut all, true, "All floors");
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut by_member, true, "By member type, with cut lengths");
                ui.radio_value(&mut by_member, false, "Lumber list");
            });
            ui.separator();
            if data.members == 0 {
                ui.weak("No framing yet. Use Build > Framing > Build Framing.");
            } else {
                let (cols, rows) = if by_member {
                    (&data.cut_columns, &data.cut_rows)
                } else {
                    (&data.columns, &data.rows)
                };
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        egui::Grid::new("framing_takeoff_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                for c in cols {
                                    ui.strong(c);
                                }
                                ui.end_row();
                                for r in rows {
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
                ui.label(format!("{} members", data.members));
                export = ui
                    .add_enabled(data.members > 0, egui::Button::new("Export CSV\u{2026}"))
                    .clicked();
                export_xlsx = ui
                    .add_enabled(data.members > 0, egui::Button::new("Export Excel\u{2026}"))
                    .clicked();
                export_list = ui
                    .add_enabled(
                        data.members > 0,
                        egui::Button::new("Export Material List\u{2026}"),
                    )
                    .clicked();
            });
        });
    ctx.data_mut(|d| d.insert_temp(by_member_id, by_member));
    if export {
        let csv = if by_member { &data.cut_csv } else { &data.csv };
        cx.status = save_text("framing_takeoff.csv", "csv", csv);
    }
    if export_xlsx {
        let (title, cols, rows) = if by_member {
            ("Framing Takeoff", &data.cut_columns, &data.cut_rows)
        } else {
            ("Lumber List", &data.columns, &data.rows)
        };
        cx.status = save_bytes(
            "framing_takeoff.xlsx",
            "xlsx",
            &plan_docs::xlsx::to_xlsx(title, cols, rows),
        );
    }
    if export_list {
        cx.status = save_text("framing_material_list.csv", "csv", &data.material_csv);
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
    fn a_layout_exports_and_imports_as_json_in_one_undo_step() {
        let mut cx = cx();
        assert!(export_layout_json(&cx).is_none());
        let mut layout = plan_layout::Layout::new("Smith Set", plan_docs::SheetSize::ArchC);
        layout.add_page(1, "Plan");
        layout.add_page(2, "Elevations");
        layout.portrait = true;
        crate::shell::layout_window::store(&mut cx.project, &layout);
        let text = export_layout_json(&cx).expect("a layout");

        let mut other = EditorContext::new(crate::plan_defaults::embedded());
        let msg = import_layout_json(&mut other, &text).unwrap();
        assert_eq!(msg, "Opened layout Smith Set (2 pages)");
        let back = crate::shell::layout_window::load(&other.project).unwrap();
        assert_eq!(back, layout);
        assert_eq!(other.undo_label(), Some("Import Layout"));
        other.undo();
        assert!(crate::shell::layout_window::load(&other.project).is_none());

        // A whole plan file works too: its layout is taken.
        let plan_json = cx.project.to_json().unwrap();
        let mut third = EditorContext::new(crate::plan_defaults::embedded());
        assert!(import_layout_json(&mut third, &plan_json).is_ok());
        assert!(crate::shell::layout_window::load(&third.project).is_some());
        // Garbage and plans without a layout are refused without a step.
        let mut fourth = EditorContext::new(crate::plan_defaults::embedded());
        assert!(import_layout_json(&mut fourth, "not json").is_err());
        assert!(import_layout_json(&mut fourth, "{\"name\":\"x\"}").is_err());
        assert!(import_layout_json(&mut fourth, "{\"pages\": 5}").is_err());
        assert!(!fourth.can_undo());
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
    fn import_applies_the_layer_mapping_placement_and_converts_walls() {
        let dxf = "0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nA-WALL\n10\n0\n20\n0\n11\n120\n21\n0\n\
0\nLINE\n8\nA-WALL\n10\n0\n20\n6\n11\n120\n21\n6\n\
0\nLINE\n8\nNOTES\n10\n0\n20\n0\n11\n5\n21\n0\n\
0\nENDSEC\n0\nEOF\n";
        let d = parse_dxf(dxf).unwrap();
        let mut cx = cx();
        let mut opts = ImportOptions::new(1.0, "DXF: ");
        opts.layers = vec![
            LayerMapping {
                source: "A-WALL".into(),
                target: LayerTarget::Rename("Walls, Normal".into()),
            },
            LayerMapping {
                source: "NOTES".into(),
                target: LayerTarget::Skip,
            },
        ];
        opts.insertion = Point::new(100.0, 100.0);
        let r = import_drawing_with(&mut cx, &d, &opts, Some("Walls, Normal"));
        assert_eq!((r.objects, r.walls), (2, 1));
        // Skipped layers import nothing; the mapped ones land on the plan
        // layer, which existed already, so no layer is created.
        assert_eq!(r.new_layers, 0);
        assert!(cx.floor().cad.iter().all(|c| c.layer == "Walls, Normal"));
        let CadItem::Line { a, .. } = &cx.floor().cad[0].item else {
            panic!("expected a line");
        };
        assert!(a.dist(Point::new(100.0, 100.0)) < 1e-9);
        assert_eq!(cx.floor().walls.len(), 1);
        // One undo step takes the objects and the walls away together.
        assert_eq!(cx.undo_label(), Some("Import Drawing"));
        cx.undo();
        assert!(cx.floor().cad.is_empty() && cx.floor().walls.is_empty());
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
    fn a_floor_dxf_carries_the_placed_schedules_as_tables() {
        use plan_core::schedules::{Schedule, ScheduleKind, ScheduleLayer};
        let mut cx = cx();
        box_house(&mut cx);
        cx.project
            .add_opening(
                0,
                cx.floor().walls[0].id,
                100.0,
                plan_core::OpeningKind::Door,
            )
            .unwrap();
        cx.refresh();
        let plain = floor_dxf(&mut cx);
        assert!(!plain.contains("Door Schedule"));
        let mut layer = ScheduleLayer::default();
        let mut def = Schedule::new(ScheduleKind::Door, Point::new(0.0, -100.0));
        def.layer = "Schedules".into();
        layer.add(def);
        layer.store(&mut cx.project.floors[0]);
        let text = floor_dxf(&mut cx);
        assert!(text.contains("Door Schedule"), "the title is in the DXF");
        // The table is lines and text on the schedule's layer; the plan itself
        // is not touched.
        assert!(text.len() > plain.len());
        assert!(cx.floor().cad.is_empty(), "the plan gained no CAD");
        let d = parse_dxf(&text).unwrap();
        assert!(d.layers.iter().any(|l| l.name == "Schedules"));
        let cad = schedule_tables_cad(&cx.project, 0, &cx.rooms, 0.25);
        assert!(cad.iter().all(|(layer, _)| layer == "Schedules"));
        let lines = cad
            .iter()
            .filter(|(_, i)| matches!(i, CadItem::Line { .. }))
            .count();
        // Rows + 1 horizontal rules and one rule per column edge.
        assert!(lines >= 4, "{lines}");
        let texts: Vec<String> = cad
            .iter()
            .filter_map(|(_, i)| match i {
                CadItem::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "Door Schedule"));
        assert!(texts.len() > 3, "headings and cells");
        // The same table at 1/8" is twice as tall in plan inches.
        let at = |ipf: f64| {
            schedule_tables_cad(&cx.project, 0, &cx.rooms, ipf)
                .iter()
                .filter_map(|(_, i)| match i {
                    CadItem::Text { height, .. } => Some(*height),
                    _ => None,
                })
                .fold(0.0, f64::max)
        };
        assert!((at(0.125) / at(0.25) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_elevation_dxf_draws_the_3d_views_scene() {
        let mut cx = cx();
        box_house(&mut cx);
        let walls_only = elevations_dxf(&cx.project).expect("elevations");
        let stair = plan_stairs::Stair::new(
            cx.project.alloc_id(),
            Point::new(60.0, 60.0),
            0.0,
            plan_stairs::StairParams::default(),
        );
        cx.project.floors[0].set_stairs(&[stair]).unwrap();
        let with_stair = elevations_dxf(&cx.project).expect("elevations");
        assert_ne!(walls_only, with_stair, "the stair is in the drawing");
        assert!(with_stair.len() > walls_only.len());
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
