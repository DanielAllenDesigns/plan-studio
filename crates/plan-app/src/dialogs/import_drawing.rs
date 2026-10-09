//! File > Import > Import Drawing (DWG/DXF)... and the Import Drawing
//! Assistant (reference manual pp. 1289-1297).
//!
//! The first window, **Import Drawing**, lists the picked files and the
//! options of the whole import (Show Import Assistant, Show For Each File,
//! Create CAD Blocks, Auto Position Blocks). With the assistant on, each
//! file goes through its pages: **Select File** (join lines into polylines
//! and boxes, hatches, paper space), **Select Layers** (which layers come in,
//! and which are converted to walls), **Layer Mapping** (one layer, the same
//! names with or without the layer attributes, or the **Advanced Layer
//! Mapping** table), **Duplicate CAD Blocks** (when the floor already has a
//! block of that name), **Drawing Unit** (units and scale, dimensions as
//! dimensions or blocks, move to the origin, insertion point) and **Import
//! Complete** (the counts). The import is one undo step per Import; the
//! imported objects are selected.
//!
//! DWG is not read: the window says how to save a DXF instead (DECISIONS).
//! The reading itself is `plan_import` ([`plan_import::parse_dxf_bytes`] then
//! [`plan_import::convert`]); this file is the window and the project edit.

mod pages;

use super::exchange::segments_of;
use crate::editor::{EditorContext, ObjectRef};
use crate::tools::cad::{apply_hatch, plan_hatch, HATCHES};
use eframe::egui;
use plan_core::geometry::Point;
use plan_core::units::parse_ft_in;
use plan_import::{
    add_objects, cad_to_walls, convert, default_units, drawing_bounds, layer_counts, make_blocks,
    parse_dxf_bytes, to_inches_factor, unused_blocks, BlockConflict, BlockMode, CadToWallsOptions,
    Converted, DimensionMode, DxfDrawing, DxfUnits, HatchSpec, ImportError,
    ImportOptions, LayerMapping, LayerTarget,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The pages of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// The Import Drawing dialog: files and the options of the whole import.
    Files,
    SelectFile,
    SelectLayers,
    LayerMapping,
    AdvancedMapping,
    DuplicateBlocks,
    AdvancedDuplicates,
    DrawingUnit,
    Complete,
}

/// The Layer Mapping page's three choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingMode {
    /// Every layer goes to one plan layer.
    Single,
    /// Plan layers of the same names (made when missing).
    SameName,
    /// Each layer individually.
    Advanced,
}

/// Where one DXF layer goes (Advanced Layer Mapping).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerChoice {
    /// A plan layer of the same name.
    Same,
    /// An existing plan layer.
    Plan(String),
    /// A layer of this name (typed).
    Named(String),
}

/// A DXF layer of the Select Layers table.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerRow {
    pub name: String,
    pub objects: usize,
    pub color: [u8; 3],
    pub on: bool,
    pub frozen: bool,
    pub linetype: String,
    /// Hundredths of a millimetre; 0 for the default.
    pub weight: u32,
    /// Imported (checked).
    pub include: bool,
    /// Its lines become walls (Convert To).
    pub walls: bool,
}

/// One file of the import.
#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub drawing: DxfDrawing,
}

/// The state of the window.
pub struct Assistant {
    pub page: Page,
    pub files: Vec<FileEntry>,
    /// The file the assistant is on.
    pub cur: usize,
    // Import Drawing dialog.
    pub show_assistant: bool,
    pub show_each: bool,
    pub create_blocks: bool,
    pub auto_position: bool,
    // Select File.
    pub join_lines: bool,
    pub boxes: bool,
    pub import_hatch: bool,
    pub paper_space: bool,
    // Select Layers.
    pub rows: Vec<LayerRow>,
    /// The wall type "convert to walls" uses ("" is the default of its kind).
    pub wall_type: String,
    // Layer Mapping.
    pub mode: MappingMode,
    pub single_layer: String,
    pub layer_attrs: bool,
    pub advanced: Vec<(String, LayerChoice)>,
    // Duplicate CAD Blocks.
    pub dup_default: BlockConflict,
    pub dup_each: bool,
    pub dup_names: Vec<(String, BlockConflict)>,
    // Drawing Unit.
    /// `None`: what the file says.
    pub units: Option<DxfUnits>,
    pub scale: f64,
    pub dims: DimensionMode,
    pub to_origin: bool,
    pub rotation_deg: f64,
    pub insert_x: String,
    pub insert_y: String,
    /// The floor (or CAD detail) the drawing goes onto.
    pub target_floor: usize,
    // Import Complete.
    pub preview: Option<Preview>,
    pub message: String,
    /// The pages already visited, for Back.
    pub trail: Vec<Page>,
}

/// What an import would do, for the last page.
#[derive(Debug, Clone, PartialEq)]
pub struct Preview {
    pub summary: plan_import::Summary,
    pub notes: Vec<String>,
    pub size: Option<(f64, f64)>,
    pub walls: usize,
}

/// What an import did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Report {
    pub objects: usize,
    pub dimensions: usize,
    pub blocks: usize,
    pub new_layers: usize,
    pub walls: usize,
    pub hatches: usize,
}

thread_local! {
    static ASSISTANT: RefCell<Option<Assistant>> = const { RefCell::new(None) };
    /// A message the window shows (a DWG file, an unreadable file).
    static NOTICE: RefCell<Option<String>> = const { RefCell::new(None) };
    /// The folder the last file came from.
    static LAST_DIR: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn is_open() -> bool {
    ASSISTANT.with(|a| a.borrow().is_some())
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn close() {
    ASSISTANT.with(|a| *a.borrow_mut() = None);
    NOTICE.with(|n| *n.borrow_mut() = None);
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn notice() -> Option<String> {
    NOTICE.with(|n| n.borrow().clone())
}

/// Runs `f` on the open assistant (tests drive it this way).
#[cfg_attr(not(test), allow(dead_code))]
pub fn with_assistant<R>(f: impl FnOnce(&mut Assistant) -> R) -> Option<R> {
    ASSISTANT.with(|a| a.borrow_mut().as_mut().map(f))
}

/// File > Import > Import Drawing (DWG/DXF)...: the file picker, then the
/// window.
pub fn start(cx: &mut EditorContext) {
    let mut dlg = rfd::FileDialog::new().add_filter("Drawing (DXF, DWG)", &["dxf", "dwg"]);
    if let Some(dir) = LAST_DIR.with(|d| d.borrow().clone()) {
        dlg = dlg.set_directory(dir);
    }
    let Some(paths) = dlg.pick_files() else {
        return;
    };
    open_paths(cx, &paths);
}

/// Opens the window on the files at `paths` (a picked or dropped file).
pub fn open_paths(cx: &mut EditorContext, paths: &[PathBuf]) {
    if let Some(dir) = paths.first().and_then(|p| p.parent()) {
        LAST_DIR.with(|d| *d.borrow_mut() = Some(dir.to_path_buf()));
    }
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for p in paths {
        match std::fs::read(p) {
            Ok(bytes) => files.push((file_name(p), bytes)),
            Err(e) => {
                NOTICE.with(|n| *n.borrow_mut() = Some(format!("Could not read {}: {e}", p.display())));
            }
        }
    }
    open_files(cx, files);
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map_or_else(|| "drawing".into(), |n| n.to_string_lossy().into_owned())
}

/// Opens the window on file contents.
pub fn open_files(cx: &mut EditorContext, files: Vec<(String, Vec<u8>)>) {
    let mut ok: Vec<FileEntry> = Vec::new();
    let mut problems: Vec<String> = Vec::new();
    for (name, bytes) in files {
        match parse_dxf_bytes(&bytes) {
            Ok(drawing) => ok.push(FileEntry { name, drawing }),
            Err(ImportError::Dwg(v)) => {
                problems.push(format!("{name}: {}", plan_import::dxf::dwg_guidance(&v)));
            }
            Err(e) => problems.push(format!("{name}: {e}")),
        }
    }
    if !problems.is_empty() {
        let msg = problems.join("\n\n");
        cx.status = format!("Import failed: {}", msg.lines().next().unwrap_or_default());
        NOTICE.with(|n| *n.borrow_mut() = Some(msg));
    }
    if ok.is_empty() {
        return;
    }
    let a = Assistant::new(cx, ok);
    ASSISTANT.with(|slot| *slot.borrow_mut() = Some(a));
}

impl Assistant {
    pub fn new(cx: &EditorContext, files: Vec<FileEntry>) -> Self {
        let mut a = Self {
            page: Page::Files,
            files,
            cur: 0,
            show_assistant: true,
            show_each: false,
            create_blocks: false,
            auto_position: false,
            join_lines: false,
            boxes: false,
            import_hatch: true,
            paper_space: false,
            rows: Vec::new(),
            wall_type: String::new(),
            mode: MappingMode::SameName,
            single_layer: String::new(),
            layer_attrs: true,
            advanced: Vec::new(),
            dup_default: BlockConflict::AutoName,
            dup_each: false,
            dup_names: Vec::new(),
            units: None,
            scale: 1.0,
            dims: DimensionMode::Objects,
            to_origin: true,
            rotation_deg: 0.0,
            insert_x: "0".into(),
            insert_y: "0".into(),
            target_floor: cx.floor,
            preview: None,
            message: String::new(),
            trail: Vec::new(),
        };
        a.single_layer = plan_layers(cx).first().cloned().unwrap_or_default();
        a.load_file(cx);
        a
    }

    pub fn drawing(&self) -> &DxfDrawing {
        &self.files[self.cur.min(self.files.len() - 1)].drawing
    }

    pub fn file_name(&self) -> &str {
        &self.files[self.cur.min(self.files.len() - 1)].name
    }

    /// Fills the layer tables from the file the assistant is on. The layers
    /// the original program showed come in checked; frozen ones do not.
    pub fn load_file(&mut self, cx: &EditorContext) {
        let rows = rows_for(self.drawing(), self.paper_space);
        // The layer a DXF layer names in the plan is the default of the
        // advanced table: the same name.
        let plan = plan_layers(cx);
        self.advanced = rows
            .iter()
            .map(|r| {
                let c = plan
                    .iter()
                    .find(|p| p.eq_ignore_ascii_case(&r.name))
                    .map_or(LayerChoice::Same, |p| LayerChoice::Plan(p.clone()));
                (r.name.clone(), c)
            })
            .collect();
        self.rows = rows;
        self.preview = None;
    }

    /// The factor from the units choice.
    pub fn factor(&self) -> f64 {
        let d = self.drawing();
        match self.units {
            None => to_inches_factor(default_units(d), None),
            Some(u) => to_inches_factor(u, Some(u)),
        }
    }

    pub fn units_label(&self) -> String {
        let d = self.drawing();
        match self.units {
            None => format!("As the file says ({})", default_units(d).label()),
            Some(u) => u.label().to_string(),
        }
    }

    /// The import options the pages describe, for `drawing` placed with its
    /// own lower-left corner `shift` inches right of the insertion point.
    pub fn options(&self, cx: &EditorContext, drawing: &DxfDrawing, shift: f64) -> ImportOptions {
        let mut o = ImportOptions::new(self.factor(), "");
        o.scale = self.scale.max(1e-9);
        o.rotation_deg = self.rotation_deg;
        o.base = if self.to_origin {
            drawing_bounds(drawing, self.paper_space).map_or(Point::ZERO, |(lo, _)| lo)
        } else {
            Point::ZERO
        };
        o.insertion = Point::new(
            parse_ft_in(&self.insert_x).unwrap_or(0.0) + shift,
            parse_ft_in(&self.insert_y).unwrap_or(0.0),
        );
        o.dimensions = self.dims;
        o.blocks = if self.create_blocks {
            BlockMode::WholeDrawing
        } else {
            BlockMode::PerInsert
        };
        o.drawing_name = self.file_name().trim_end_matches(".dxf").to_string();
        o.include_paper_space = self.paper_space;
        o.import_hatch = self.import_hatch;
        o.join_lines = self.join_lines;
        o.boxes = self.boxes;
        o.text_styles = cx
            .project
            .text_styles
            .styles
            .iter()
            .map(|s| s.name.clone())
            .collect();
        // Layers: skipped ones, and where the others go.
        for r in &self.rows {
            if !r.include {
                o.layers.push(LayerMapping {
                    source: r.name.clone(),
                    target: LayerTarget::Skip,
                });
            }
        }
        match self.mode {
            MappingMode::Single => {
                o.single_layer = Some(self.single_layer.clone());
                o.object_attrs = true;
            }
            MappingMode::SameName => {
                o.layer_attrs = self.layer_attrs;
                o.object_attrs = !self.layer_attrs;
            }
            MappingMode::Advanced => {
                o.layer_attrs = self.layer_attrs;
                o.object_attrs = !self.layer_attrs;
                for (src, choice) in &self.advanced {
                    if self.rows.iter().any(|r| r.name == *src && !r.include) {
                        continue;
                    }
                    let target = match choice {
                        LayerChoice::Same => LayerTarget::Keep,
                        LayerChoice::Plan(n) | LayerChoice::Named(n) => LayerTarget::Rename(n.clone()),
                    };
                    o.layers.push(LayerMapping {
                        source: src.clone(),
                        target,
                    });
                }
            }
        }
        o
    }

    /// The block conflict policies chosen on the duplicate pages.
    fn conflicts(&self) -> (BlockConflict, BTreeMap<String, BlockConflict>) {
        let map = if self.dup_each {
            self.dup_names
                .iter()
                .map(|(n, c)| (n.to_uppercase(), *c))
                .collect()
        } else {
            BTreeMap::new()
        };
        (self.dup_default, map)
    }

    /// Names of the blocks this import would make that the floor has already.
    pub fn duplicate_blocks(&self, cx: &EditorContext) -> Vec<String> {
        let conv = convert(self.drawing(), &self.options(cx, self.drawing(), 0.0));
        let have: Vec<String> = cx
            .project
            .floors
            .get(self.target_floor)
            .map(|f| f.cad_blocks())
            .unwrap_or_default()
            .into_iter()
            .map(|b| b.name.to_uppercase())
            .collect();
        let mut out: Vec<String> = Vec::new();
        for b in &conv.blocks {
            if have.contains(&b.name.to_uppercase()) && !out.contains(&b.name) {
                out.push(b.name.clone());
            }
        }
        out
    }

    /// The names of the layers whose lines become walls (after the mapping).
    fn wall_layers(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.include && r.walls)
            .map(|r| r.name.clone())
            .collect()
    }

    /// Runs the conversion for the last page.
    pub fn refresh_preview(&mut self, cx: &EditorContext) {
        let d = self.drawing();
        let opts = self.options(cx, d, 0.0);
        let conv = convert(d, &opts);
        let size = conv.bounds().map(|(lo, hi)| (hi.x - lo.x, hi.y - lo.y));
        let walls = if self.wall_layers().is_empty() {
            0
        } else {
            let lines = wall_lines(&conv, &self.wall_layers());
            cad_to_walls(&lines, &CadToWallsOptions::default()).walls.len()
        };
        let mut notes = conv.notes.clone();
        let unused = unused_blocks(d);
        if !unused.is_empty() {
            notes.push(format!(
                "{} block definition{} no INSERT uses {} not placed (Plan Studio keeps no block library to hold them)",
                unused.len(),
                if unused.len() == 1 { "" } else { "s" },
                if unused.len() == 1 { "is" } else { "are" }
            ));
        }
        if !d.skipped.is_empty() {
            let parts: Vec<String> = d.skipped.iter().map(|(k, n)| format!("{n} {k}")).collect();
            notes.push(format!("Not imported: {}", parts.join(", ")));
        }
        self.preview = Some(Preview {
            summary: conv.summary(),
            notes,
            size,
            walls,
        });
    }

    /// Imports every file with the pages' settings as one undo step.
    pub fn import_all(&self, cx: &mut EditorContext) -> Report {
        let mut total = Report::default();
        let mut began = false;
        let mut shift = 0.0;
        // The drawing goes onto the floor chosen on the Drawing Unit page.
        let active = cx.floor;
        let target = self.target_floor.min(cx.project.floors.len().saturating_sub(1));
        cx.floor = target;
        let files: Vec<&FileEntry> = if self.show_each {
            vec![&self.files[self.cur.min(self.files.len() - 1)]]
        } else {
            self.files.iter().collect()
        };
        for (i, f) in files.iter().enumerate() {
            let mut a = self.shared_for(f);
            a.cur = a.files.iter().position(|x| x.name == f.name).unwrap_or(0);
            let opts = a.options(cx, &f.drawing, shift);
            let conv = convert(&f.drawing, &opts);
            if conv.objects.is_empty() && conv.dimensions.is_empty() {
                continue;
            }
            if !began {
                cx.begin_change("Import Drawing");
                began = true;
            }
            let r = a.apply(cx, &conv);
            total.objects += r.objects;
            total.dimensions += r.dimensions;
            total.blocks += r.blocks;
            total.new_layers += r.new_layers;
            total.walls += r.walls;
            total.hatches += r.hatches;
            if self.auto_position {
                if let Some((lo, hi)) = conv.bounds() {
                    shift += (hi.x - lo.x) + 48.0;
                    let _ = (lo, i);
                }
            }
        }
        cx.floor = active;
        if target != active {
            // The imported objects are on another floor: nothing to select here.
            cx.selection.items.clear();
        }
        if began {
            cx.mark_dirty();
        }
        total
    }

    /// A copy of the settings for another file of a shared import: the
    /// layer table is rebuilt for that file, keeping the choices for layers
    /// of the same names.
    fn shared_for(&self, f: &FileEntry) -> Assistant {
        let mut a = Assistant {
            page: self.page,
            files: vec![f.clone()],
            cur: 0,
            show_assistant: self.show_assistant,
            show_each: self.show_each,
            create_blocks: self.create_blocks,
            auto_position: self.auto_position,
            join_lines: self.join_lines,
            boxes: self.boxes,
            import_hatch: self.import_hatch,
            paper_space: self.paper_space,
            rows: self.rows.clone(),
            wall_type: self.wall_type.clone(),
            mode: self.mode,
            single_layer: self.single_layer.clone(),
            layer_attrs: self.layer_attrs,
            advanced: self.advanced.clone(),
            dup_default: self.dup_default,
            dup_each: self.dup_each,
            dup_names: self.dup_names.clone(),
            units: self.units,
            scale: self.scale,
            dims: self.dims,
            to_origin: self.to_origin,
            rotation_deg: self.rotation_deg,
            insert_x: self.insert_x.clone(),
            insert_y: self.insert_y.clone(),
            target_floor: self.target_floor,
            preview: None,
            message: String::new(),
            trail: Vec::new(),
        };
        if self.files.len() > 1 && !self.show_each && f.name != self.file_name() {
            // Another file of a shared import: its own layers, the choices
            // of the layers the files share.
            let old_rows = std::mem::take(&mut a.rows);
            let old_adv = std::mem::take(&mut a.advanced);
            a.rows = rows_for(&f.drawing, a.paper_space)
                .into_iter()
                .map(|mut r| {
                    if let Some(o) = old_rows.iter().find(|o| o.name == r.name) {
                        r.include = o.include;
                        r.walls = o.walls;
                    }
                    r
                })
                .collect();
            a.advanced = a
                .rows
                .iter()
                .map(|r| {
                    let c = old_adv
                        .iter()
                        .find(|(n, _)| *n == r.name)
                        .map_or(LayerChoice::Same, |(_, c)| c.clone());
                    (r.name.clone(), c)
                })
                .collect();
        }
        a
    }

    /// Adds `conv` to the active floor inside the undo step the caller began.
    fn apply(&self, cx: &mut EditorContext, conv: &Converted) -> Report {
        let floor = cx.floor;
        let (default, per) = self.conflicts();
        let mut rep = add_objects(&mut cx.project, floor, conv, default, &per);
        let mut report = Report {
            objects: rep.ids.iter().filter(|i| **i != 0).count(),
            dimensions: rep.dimension_ids.len(),
            new_layers: rep.new_layers,
            ..Report::default()
        };
        // Pattern hatches are drawn lines, like the CAD Hatch tool's.
        for (i, o) in conv.objects.iter().enumerate() {
            if let (Some(h), id) = (&o.hatch, rep.ids[i]) {
                if id != 0 && draw_hatch(cx, id, h) {
                    report.hatches += 1;
                }
            }
        }
        report.blocks = make_blocks(&mut cx.project, floor, conv, &mut rep);
        // Convert to walls.
        let wall_layers = self.wall_layers();
        if !wall_layers.is_empty() {
            let lines = wall_lines(conv, &wall_layers);
            let result = cad_to_walls(&lines, &CadToWallsOptions::default());
            report.walls = add_walls(cx, &result, &self.wall_type);
        }
        // The drawing's components are selected, as Chief does.
        cx.selection.items = rep
            .ids
            .iter()
            .filter(|i| **i != 0)
            .map(|i| ObjectRef::Cad(*i))
            .collect();
        report
    }
}

/// The layer table of a drawing, as the Select Layers page lists it.
fn rows_for(d: &DxfDrawing, paper: bool) -> Vec<LayerRow> {
    let counts: BTreeMap<String, usize> = layer_counts(d, paper).into_iter().collect();
    let mut rows: Vec<LayerRow> = d
        .layers
        .iter()
        .map(|l| {
            let n = counts.get(&l.name).copied().unwrap_or(0);
            LayerRow {
                name: l.name.clone(),
                objects: n,
                color: l.rgb(),
                on: l.visible,
                frozen: l.frozen,
                linetype: l.linetype.clone(),
                weight: u32::try_from(l.weight).unwrap_or(0),
                include: l.visible && n > 0,
                walls: false,
            }
        })
        .collect();
    for (name, n) in &counts {
        if !rows.iter().any(|r| r.name.eq_ignore_ascii_case(name)) {
            rows.push(LayerRow {
                name: name.clone(),
                objects: *n,
                color: [0, 0, 0],
                on: true,
                frozen: false,
                linetype: "CONTINUOUS".into(),
                weight: 0,
                include: true,
                walls: false,
            });
        }
    }
    rows
}

fn plan_layers(cx: &EditorContext) -> Vec<String> {
    cx.project.layers.layers.iter().map(|l| l.name.clone()).collect()
}

/// The line segments, in the plan, of the objects that came from the DXF
/// layers in `layers`.
fn wall_lines(conv: &Converted, layers: &[String]) -> Vec<(Point, Point)> {
    conv.objects
        .iter()
        .filter(|o| layers.iter().any(|l| *l == o.source_layer))
        .flat_map(|o| segments_of(&o.item))
        .collect()
}

/// Draws the hatch lines of `h` on the closed outline `id`; false when the
/// pattern cannot be drawn.
fn draw_hatch(cx: &mut EditorContext, id: plan_core::Id, h: &HatchSpec) -> bool {
    let choice = HATCHES
        .iter()
        .position(|(n, _)| *n == h.style)
        .unwrap_or(1);
    let mut spacing = h.spacing;
    for _ in 0..8 {
        if let Ok(job) = plan_hatch(cx, id, choice, spacing) {
            apply_hatch(cx, job);
            let floor = cx.floor;
            cx.project.edit_cad_attrs(floor, id, |a| {
                if let Some(f) = a.fill.as_mut() {
                    f.color = h.color;
                    f.angle_deg = h.angle_deg;
                }
            });
            return true;
        }
        spacing *= 2.0;
    }
    false
}

/// Adds the proposed walls at the default height of their kind; with a wall
/// type named, the walls take that type's thickness and kind. Returns how
/// many were added.
fn add_walls(cx: &mut EditorContext, result: &plan_import::CadToWallsResult, ty: &str) -> usize {
    let floor = cx.floor;
    let def = (!ty.is_empty())
        .then(|| {
            cx.wall_types()
                .iter()
                .find(|t| t.name == ty)
                .cloned()
                .or_else(|| cx.defaults.wall_type(ty).cloned())
        })
        .flatten();
    if let Some(d) = &def {
        if cx.project.wall_type_def(&d.name).is_none() {
            cx.project.register_wall_type(d.clone());
        }
    }
    for w in &result.walls {
        let (thickness, kind) = def.as_ref().map_or((w.thickness, w.kind), |d| (d.thickness(), d.kind));
        let height = cx.wall_height(kind);
        let id = cx.project.add_wall(floor, w.start, w.end, thickness, height, kind);
        if let (Some(d), Some(wall)) = (&def, cx.project.floors[floor].wall_mut(id)) {
            wall.wall_type = Some(d.name.clone());
        }
    }
    result.walls.len()
}

/// A short line for the status bar.
pub fn status_line(r: &Report) -> String {
    if r.objects == 0 && r.dimensions == 0 {
        return "The drawing had nothing to import".into();
    }
    let mut s = format!("Imported {} objects", r.objects);
    if r.dimensions > 0 {
        s.push_str(&format!(", {} dimensions", r.dimensions));
    }
    if r.blocks > 0 {
        s.push_str(&format!(", {} CAD blocks", r.blocks));
    }
    s.push_str(&format!(" ({} new layers)", r.new_layers));
    if r.walls > 0 {
        s.push_str(&format!(" and made {} walls", r.walls));
    }
    s
}

/// The colour of a layer row's swatch.
pub fn swatch(c: [u8; 3]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// Draws the window; the shell calls it once a frame.
pub fn show(ctx: &egui::Context, cx: &mut EditorContext) {
    // A .dxf or .dwg dropped on the window starts the import.
    let dropped: Vec<PathBuf> = ctx.input(|i| {
        i.raw
            .dropped_files
            .iter()
            .filter_map(|f| f.path.clone())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("dxf") || e.eq_ignore_ascii_case("dwg"))
            })
            .collect()
    });
    if !dropped.is_empty() {
        open_paths(cx, &dropped);
    }
    pages::show_notice(ctx);
    if let Some(mut a) = ASSISTANT.with(|s| s.borrow_mut().take()) {
        let keep = pages::show(&mut a, ctx, cx);
        if keep {
            ASSISTANT.with(|slot| {
                if slot.borrow().is_none() {
                    *slot.borrow_mut() = Some(a);
                }
            });
        }
    }
}

/// The Next button of the open window (what a click does). False when the
/// window finished and closed.
#[cfg_attr(not(test), allow(dead_code))]
pub fn next(cx: &mut EditorContext) -> bool {
    let Some(mut a) = ASSISTANT.with(|s| s.borrow_mut().take()) else {
        return false;
    };
    let keep = pages::go_next(&mut a, cx);
    if keep {
        ASSISTANT.with(|slot| *slot.borrow_mut() = Some(a));
    }
    keep
}

/// The page the open window is on.
#[cfg_attr(not(test), allow(dead_code))]
pub fn page() -> Option<Page> {
    with_assistant(|a| a.page)
}

pub(crate) fn set_notice(msg: Option<String>) {
    NOTICE.with(|n| *n.borrow_mut() = msg);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_import::parse_dxf;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn assistant(cx: &EditorContext) -> Assistant {
        let dxf = "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n\
0\nLAYER\n2\ndoors\n70\n0\n62\n3\n\
0\nLAYER\n2\nA-WALL\n70\n0\n62\n1\n\
0\nLAYER\n2\nOLD\n70\n1\n62\n5\n0\nENDTAB\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\ndoors\n10\n0\n20\n0\n11\n5\n21\n0\n\
0\nLINE\n8\nA-WALL\n10\n0\n20\n0\n11\n5\n21\n0\n\
0\nLINE\n8\nOLD\n10\n0\n20\n0\n11\n5\n21\n0\n0\nENDSEC\n0\nEOF\n";
        Assistant::new(
            cx,
            vec![FileEntry {
                name: "x.dxf".into(),
                drawing: parse_dxf(dxf).unwrap(),
            }],
        )
    }

    #[test]
    fn the_layer_tables_start_from_what_the_file_and_the_plan_have() {
        let cx = cx();
        let a = assistant(&cx);
        let row = |n: &str| a.rows.iter().find(|r| r.name == n).unwrap().clone();
        // Visible layers are checked, the frozen one is not.
        assert!(row("A-WALL").include && row("doors").include);
        assert!(!row("OLD").include && row("OLD").frozen);
        assert_eq!(row("A-WALL").color, [255, 0, 0]);
        // A DXF layer named like a plan layer lands on it (any case); the rest keep their names.
        let to = |n: &str| a.advanced.iter().find(|(m, _)| m == n).unwrap().1.clone();
        assert_eq!(to("doors"), LayerChoice::Plan("Doors".into()));
        assert_eq!(to("A-WALL"), LayerChoice::Same);
        assert_eq!(a.page, Page::Files);
    }

    #[test]
    fn the_pages_build_the_import_options() {
        let cx = cx();
        let mut a = assistant(&cx);
        a.scale = 2.0;
        a.rotation_deg = 90.0;
        a.insert_x = "10'".into();
        a.to_origin = false;
        a.mode = MappingMode::Advanced;
        let o = a.options(&cx, &a.files[0].drawing.clone(), 0.0);
        assert_eq!(o.scale, 2.0);
        assert_eq!(o.insertion, Point::new(120.0, 0.0));
        // The frozen layer is skipped; the plan's Doors layer takes "doors".
        assert_eq!(o.target_layer("OLD"), None);
        assert_eq!(o.target_layer("doors").as_deref(), Some("Doors"));
        assert_eq!(o.target_layer("A-WALL").as_deref(), Some("A-WALL"));
        // One layer for everything keeps the objects' own looks.
        a.mode = MappingMode::Single;
        a.single_layer = "All".into();
        let o = a.options(&cx, &a.files[0].drawing.clone(), 0.0);
        assert_eq!(o.target_layer("A-WALL").as_deref(), Some("All"));
        assert!(o.object_attrs);
        // The file has no units: inches.
        assert_eq!(a.factor(), 1.0);
        a.units = Some(DxfUnits::Millimeters);
        assert!((a.factor() - 1.0 / 25.4).abs() < 1e-12);
    }

    #[test]
    fn with_no_layer_checked_the_assistant_stops_on_the_layer_page() {
        let mut cx = cx();
        let mut a = assistant(&cx);
        a.page = Page::SelectLayers;
        a.rows.iter_mut().for_each(|r| r.include = false);
        assert!(pages::go_next(&mut a, &mut cx));
        assert_eq!(a.page, Page::SelectLayers);
        assert!(a.message.contains("Check at least one layer"));
    }

    #[test]
    fn the_status_line_counts_what_came_in() {
        let r = Report {
            objects: 12,
            dimensions: 1,
            blocks: 2,
            new_layers: 3,
            walls: 4,
            hatches: 1,
        };
        assert_eq!(
            status_line(&r),
            "Imported 12 objects, 1 dimensions, 2 CAD blocks (3 new layers) and made 4 walls"
        );
        assert_eq!(status_line(&Report::default()), "The drawing had nothing to import");
    }
}
