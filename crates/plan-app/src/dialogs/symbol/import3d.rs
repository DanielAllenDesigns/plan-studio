//! File > Import > 3D Symbol (STL, 3DS, COLLADA .dae, OBJ, glTF): Chief's
//! Import 3D Symbol dialog. The file goes into the User Catalog as a library
//! symbol (a 3D model plus the plan symbol drawn from above), ready to place
//! with the Library tool.
//!
//! The dialog collects:
//!
//! * **Name** and **Category** (the User Catalog folder);
//! * **Units** the file was modelled in (Inches, Feet, Millimeters,
//!   Centimeters, Meters, or the unit a COLLADA file declares) and **Up
//!   axis**, both pre-set from the file ([`plan_import::suggest`]: the file's
//!   own values, else the format's axis and a size-based unit guess);
//! * **Symbol faces direction**: which way the model's front points (the
//!   file's +Z/-Z/+X/-X for a Y-up file, -Y/+Y/+X/-X for Z up) so it ends up
//!   facing the symbol's front;
//! * **Size**: the 3D bounding box (width, depth, height) with optional new
//!   values, proportional or per side;
//! * **Placement** (the symbol's origin follows it: wall mounted symbols are
//!   drawn from their back-center, the others from their center), default
//!   **Elevation**, and the layer;
//! * **Materials**: textures and colors map to `plan-materials` entries (a
//!   texture file called `oak_floor.jpg` finds the Oak material and takes its
//!   color), listed per part.
//!
//! SketchUp (`.skp`) files cannot be read without SketchUp's own SDK; the
//! dialog says so and how to export instead ([`plan_import::SKP_MESSAGE`]).
//! Everything is one undo step in the sense that importing changes only the
//! library, not the plan.

use crate::editor::{EditorContext, EditorRequest};
use crate::tools::library::user::{self as store, ModelImport};
use crate::tools::ToolId;
use eframe::egui::{self, Align2, Color32};
use plan_import::formats::Suggestion;
use plan_import::model::UNITS;
use plan_import::{shape, Facing, ImportedModel, ModelOptions, UpAxis};
use plan_library::manage::USER_ROOT;
use plan_library::Placement;
use plan_materials::MaterialLibrary;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Where each part's look came from, for the Materials list.
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialMatch {
    /// Part name from the file.
    pub part: String,
    /// The file's material name.
    pub file_material: Option<String>,
    /// The texture file the material uses.
    pub texture: Option<String>,
    /// The `plan-materials` entry found for it.
    pub plan_material: Option<String>,
    /// The color the part ends up with.
    pub color: Option<[u8; 3]>,
    /// True when the plan material's color replaced the file's.
    pub applied: bool,
}

/// Words in a texture file name that say nothing about the material.
const NOISE: [&str; 26] = [
    "tex", "texture", "textures", "map", "diffuse", "color", "colour", "base", "albedo", "jpg",
    "jpeg", "png", "img", "image", "material", "mat", "bump", "normal", "top", "side", "front",
    "back", "left", "right", "bottom", "surface",
];

/// The material of `lib` a texture file name points at: the entry whose name
/// holds the most of the file name's words (`oak_floor_01.jpg` finds an
/// `Oak Hardwood` material), the first word counting a little more. `None`
/// when no word matches.
pub fn match_texture(lib: &MaterialLibrary, texture: &str) -> Option<(String, [u8; 3])> {
    let stem = texture
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(texture)
        .rsplit_once('.')
        .map_or(texture, |(s, _)| s)
        .to_ascii_lowercase();
    let words: Vec<String> = stem
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| w.len() >= 3 && !NOISE.contains(w))
        .map(str::to_string)
        .collect();
    let mut best: Option<(usize, &plan_materials::MaterialDef)> = None;
    for def in lib.search("") {
        let name = def.name.to_ascii_lowercase();
        let score: usize = words
            .iter()
            .enumerate()
            .filter(|(_, w)| name.contains(w.as_str()))
            .map(|(i, w)| w.len() * 10 + if i == 0 { 5 } else { 0 })
            .sum();
        if score > 0 && best.is_none_or(|(s, _)| score > s) {
            best = Some((score, def));
        }
    }
    best.map(|(_, d)| (d.name.clone(), d.color))
}

/// The library material nearest to `rgb` by color.
fn nearest_material(lib: &MaterialLibrary, rgb: [u8; 3]) -> Option<String> {
    let d = |c: [u8; 3]| -> i32 {
        (0..3)
            .map(|i| (i32::from(c[i]) - i32::from(rgb[i])).pow(2))
            .sum()
    };
    lib.search("")
        .into_iter()
        .min_by_key(|m| d(m.color))
        .map(|m| m.name.clone())
}

/// Maps the parts' textures and colors to `lib`: a part whose texture finds a
/// material takes that material's color; the rest keep the file's color. One
/// entry per part, for the dialog's list.
pub fn map_materials(
    model: &mut ImportedModel,
    lib: &MaterialLibrary,
    apply: bool,
) -> Vec<MaterialMatch> {
    let mut out = Vec::new();
    for part in &mut model.parts {
        let found = part
            .texture
            .as_deref()
            .and_then(|t| match_texture(lib, t));
        let mut m = MaterialMatch {
            part: part.name.clone(),
            file_material: part.material.clone(),
            texture: part.texture.clone(),
            plan_material: found.as_ref().map(|(n, _)| n.clone()),
            color: part.color,
            applied: false,
        };
        match found {
            Some((_, color)) if apply => {
                part.color = Some(color);
                m.color = Some(color);
                m.applied = true;
            }
            Some(_) => {}
            None => {
                m.plan_material = part.color.and_then(|c| nearest_material(lib, c));
            }
        }
        out.push(m);
    }
    out
}

/// The dialog's state.
pub struct ImportSymbol {
    path: PathBuf,
    ext: String,
    suggestion: Suggestion,
    /// Index into [`UNITS`]; `None` is the file's declared unit
    /// (`options.unit_scale` as suggested).
    unit_index: Option<usize>,
    declared_scale: f64,
    pub options: ModelOptions,
    pub facing: Facing,
    pub name: String,
    pub folder: String,
    pub placement: Option<Placement>,
    pub elevation: f64,
    pub layer: String,
    /// New size: width (x), depth (z), height (y); `None` keeps the file's.
    pub resize: bool,
    pub keep_proportions: bool,
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    pub map_materials: bool,
    /// The file read with the units and axis: `Err` is the message to show.
    parsed: Result<ImportedModel, String>,
    /// `parsed` turned and resized, materials mapped; what gets imported.
    shaped: Option<ImportedModel>,
    matches: Vec<MaterialMatch>,
    error: String,
    /// Preview texture and the key it was made for.
    preview: Option<(usize, egui::TextureHandle)>,
    dirty: bool,
}

fn folder_text(path: &[String]) -> String {
    path.join(" > ")
}

fn parse_folder(text: &str) -> Vec<String> {
    let mut out: Vec<String> = text
        .split('>')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if out.first().map(String::as_str) != Some(USER_ROOT) {
        out.insert(0, USER_ROOT.to_string());
    }
    out
}

impl ImportSymbol {
    /// A dialog for the file at `path`, units and axis taken from the file.
    pub fn open(path: &Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let bytes = std::fs::read(path).unwrap_or_default();
        let suggestion = plan_import::suggest(&ext, &bytes);
        let o = suggestion.options;
        let unit_index = UNITS.iter().position(|(_, k)| (*k - o.unit_scale).abs() < 1e-6 * k);
        let defaults = store::import_defaults(path);
        let mut s = ImportSymbol {
            path: path.to_path_buf(),
            ext,
            suggestion,
            unit_index,
            declared_scale: o.unit_scale,
            options: o,
            facing: Facing::Front,
            name: defaults.name,
            folder: folder_text(&defaults.folder),
            placement: None,
            elevation: 0.0,
            layer: String::new(),
            resize: false,
            keep_proportions: true,
            width: 0.0,
            depth: 0.0,
            height: 0.0,
            map_materials: true,
            parsed: Err(String::new()),
            shaped: None,
            matches: Vec::new(),
            error: String::new(),
            preview: None,
            dirty: true,
        };
        s.reparse();
        s
    }

    /// The file's name.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    }

    /// What the unit and axis were suggested from.
    pub fn suggestion(&self) -> &Suggestion {
        &self.suggestion
    }

    /// Reads the file again with the chosen unit and axis.
    pub fn reparse(&mut self) {
        if let Some(i) = self.unit_index {
            self.options.unit_scale = UNITS[i].1;
        } else {
            self.options.unit_scale = self.declared_scale;
        }
        self.parsed = if self.ext == "skp" {
            Err(plan_import::SKP_MESSAGE.to_string())
        } else {
            store::parse_model_file(&self.path, &self.options)
        };
        if let Ok(m) = &self.parsed {
            if let Some(e) = shape::size_of(&shape::faced(m, self.facing)) {
                // Start the size fields at the model's own size.
                (self.width, self.height, self.depth) = (e[0], e[1], e[2]);
            }
        }
        self.dirty = true;
    }

    /// Picks the unit by index into [`UNITS`] and reads the file again.
    pub fn set_unit(&mut self, index: usize) {
        self.unit_index = Some(index.min(UNITS.len() - 1));
        self.reparse();
    }

    /// Chooses the up axis and reads the file again.
    pub fn set_up_axis(&mut self, up: UpAxis) {
        self.options.up_axis = up;
        self.reparse();
    }

    /// Chooses which way the model's front points.
    pub fn set_facing(&mut self, facing: Facing) {
        self.facing = facing;
        self.reparse();
    }

    /// The file's model with the units and axis, before turning and sizing.
    pub fn parsed(&self) -> Result<&ImportedModel, &str> {
        self.parsed.as_ref().map_err(String::as_str)
    }

    /// Size of the turned model before any resize: width, depth, height.
    pub fn natural_size(&self) -> Option<[f64; 3]> {
        let m = self.parsed.as_ref().ok()?;
        let e = shape::size_of(&shape::faced(m, self.facing))?;
        Some([e[0], e[2], e[1]])
    }

    /// The model as it will be imported (turned, resized, materials mapped).
    pub fn shaped(&mut self) -> Option<&ImportedModel> {
        self.refresh();
        self.shaped.as_ref()
    }

    /// The Materials list.
    pub fn material_matches(&mut self) -> &[MaterialMatch] {
        self.refresh();
        &self.matches
    }

    fn refresh(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.shaped = None;
        self.matches.clear();
        let Ok(m) = &self.parsed else { return };
        let mut m = shape::faced(m, self.facing);
        if self.resize {
            let pick = |v: f64| (v > 0.0).then_some(v);
            m = shape::resized(
                &m,
                pick(self.width),
                pick(self.height),
                pick(self.depth),
                false,
            );
        }
        let lib = crate::tools::materials::library();
        self.matches = map_materials(&mut m, &lib, self.map_materials);
        self.shaped = Some(m);
    }

    /// Marks the shape settings as changed.
    pub fn touch(&mut self) {
        self.dirty = true;
    }

    /// Changes one side of the new size; with `keep_proportions` the others
    /// follow the file's shape.
    pub fn set_side(&mut self, side: Side, value: f64) {
        let Some([w, d, h]) = self.natural_size() else {
            return;
        };
        let value = value.max(0.01);
        let k = match side {
            Side::Width => value / w.max(1e-9),
            Side::Depth => value / d.max(1e-9),
            Side::Height => value / h.max(1e-9),
        };
        if self.keep_proportions {
            (self.width, self.depth, self.height) = (w * k, d * k, h * k);
        } else {
            match side {
                Side::Width => self.width = value,
                Side::Depth => self.depth = value,
                Side::Height => self.height = value,
            }
        }
        self.dirty = true;
    }

    /// The library settings the import will use.
    pub fn settings(&self) -> ModelImport {
        ModelImport {
            name: self.name.clone(),
            folder: parse_folder(&self.folder),
            options: self.options,
            placement: self.placement,
        }
    }

    /// Adds the symbol to the User Catalog; the new item's id and name.
    pub fn import(&mut self) -> Result<(String, String), String> {
        if let Err(e) = &self.parsed {
            return Err(e.clone());
        }
        self.refresh();
        let shaped = self.shaped.as_ref().ok_or("The file has nothing to import")?;
        let (mut item, model) = store::build_model_item(shaped, &self.settings())?;
        item.elevation = self.elevation.max(0.0);
        if !self.layer.trim().is_empty() {
            item.layer = Some(self.layer.trim().to_string());
        }
        let added = store::add(item, Some(&model))?;
        Ok((added.id.clone(), added.name.clone()))
    }
}

/// One side of the size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Width,
    Depth,
    Height,
}

// ----- the window -----

thread_local! {
    static WINDOW: RefCell<Option<ImportSymbol>> = const { RefCell::new(None) };
}

/// Asks for a 3D file and opens the dialog for it.
pub fn start_import(cx: &mut EditorContext) {
    if cfg!(test) {
        // No native file dialog under test; `open_path` is the entry then.
        cx.status = "Import 3D Symbol: choose a file".into();
        return;
    }
    let Some(path) = rfd::FileDialog::new()
        .set_title("Import 3D Symbol")
        .add_filter("3D models", &plan_import::formats::EXTENSIONS)
        .pick_file()
    else {
        cx.status = "Import cancelled".into();
        return;
    };
    open_path(cx, &path);
}

/// Opens the dialog for the file at `path`.
pub fn open_path(cx: &mut EditorContext, path: &Path) {
    let s = ImportSymbol::open(path);
    cx.status = match s.parsed() {
        Ok(_) => "Check the units and direction, then Import".into(),
        Err(e) => e.to_string(),
    };
    WINDOW.with(|w| *w.borrow_mut() = Some(s));
}

/// True while the dialog is open.
pub fn is_open() -> bool {
    WINDOW.with(|w| w.borrow().is_some())
}

/// Runs `f` on the open dialog (used by tests and the shell).
pub fn with_dialog<R>(f: impl FnOnce(&mut ImportSymbol) -> R) -> Option<R> {
    WINDOW.with(|w| w.borrow_mut().as_mut().map(f))
}

/// Closes the dialog.
pub fn close() {
    WINDOW.with(|w| *w.borrow_mut() = None);
}

/// Imports from the open dialog and, on success, makes the new symbol the
/// Library tool's active item.
pub fn accept(cx: &mut EditorContext) -> bool {
    let Some(mut s) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return false;
    };
    match s.import() {
        Ok((id, name)) => {
            crate::tools::library::set_active_item(cx, &id);
            cx.requests.push(EditorRequest::SetTool(ToolId::Library));
            cx.status = format!("Imported \"{name}\" into the User Catalog; click in the plan to place it");
            true
        }
        Err(e) => {
            s.error = e;
            WINDOW.with(|w| *w.borrow_mut() = Some(s));
            false
        }
    }
}

const RED: Color32 = Color32::from_rgb(0xB0, 0x30, 0x30);

fn placement_name(p: Placement) -> &'static str {
    match p {
        Placement::WallMounted => "Wall mounted (origin at the back center)",
        Placement::FreeStanding => "Free standing (origin at the center)",
        Placement::Ceiling => "Ceiling",
        Placement::Countertop => "Countertop",
    }
}

fn dim(v: f64) -> String {
    let t = format!("{v:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Draws the dialog; call once a frame.
pub fn show_import(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut s) = WINDOW.with(|w| w.borrow_mut().take()) else {
        return;
    };
    let mut open = true;
    let mut go = false;
    let mut cancel = false;
    let folders: Vec<String> = {
        let meta = store::meta();
        meta.all_folders(&store::items())
            .iter()
            .map(|f| folder_text(f))
            .collect()
    };
    s.refresh();
    // Preview of the shaped model.
    let key = s.shaped.as_ref().map_or(0, |m| {
        m.triangle_count() * 7 + (s.width * 10.0) as usize + (s.height * 100.0) as usize
    });
    if s.preview.as_ref().map(|(k, _)| *k) != Some(key) {
        s.preview = s.shaped.as_ref().map(|m| {
            let model = crate::tools::library::make::model_from_import(m);
            let img = plan_library::preview::render_preview(&model, 160, 30.0, 20.0);
            let color = egui::ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba);
            (
                key,
                ctx.load_texture("import3d_preview", color, egui::TextureOptions::LINEAR),
            )
        });
    }
    let mut reparse = false;
    let mut reshape = false;
    egui::Window::new("Import 3D Symbol")
        .id(egui::Id::new("import_3d_symbol"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(460.0);
            ui.strong(s.file_name());
            match s.parsed() {
                Ok(m) => {
                    ui.weak(format!(
                        "{} triangles in {} part(s)",
                        m.triangle_count(),
                        m.parts.len()
                    ));
                }
                Err(e) => {
                    ui.colored_label(RED, e);
                }
            }
            ui.horizontal(|ui| {
                if let Some((_, tex)) = &s.preview {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::Vec2::splat(120.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 3.0, Color32::from_gray(0xEC));
                    ui.painter().image(
                        tex.id(),
                        rect,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                ui.vertical(|ui| {
                    if let Some([w, d, h]) = s.natural_size() {
                        ui.label(format!(
                            "3D bounding box: {} wide x {} deep x {} high (in)",
                            dim(w),
                            dim(d),
                            dim(h)
                        ));
                    }
                    let note = if s.suggestion().from_file {
                        "Units and up axis come from the file."
                    } else {
                        "Units were guessed from the model's size; check them."
                    };
                    ui.weak(note);
                });
            });
            ui.separator();
            egui::Grid::new("import3d_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.text_edit_singleline(&mut s.name);
                    ui.end_row();
                    ui.label("Category");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut s.folder);
                        ui.menu_button("\u{25BE}", |ui| {
                            for f in &folders {
                                if ui.button(f).clicked() {
                                    s.folder = f.clone();
                                    ui.close_menu();
                                }
                            }
                        });
                    });
                    ui.end_row();
                    ui.label("Scale units");
                    let shown = s
                        .unit_index
                        .map_or("Declared by the file", |i| UNITS[i].0);
                    egui::ComboBox::from_id_salt("i3d_units")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            for (i, (n, _)) in UNITS.iter().enumerate() {
                                if ui.selectable_label(s.unit_index == Some(i), *n).clicked() {
                                    s.unit_index = Some(i);
                                    reparse = true;
                                }
                            }
                        });
                    ui.end_row();
                    ui.label("Up axis");
                    egui::ComboBox::from_id_salt("i3d_up")
                        .selected_text(match s.options.up_axis {
                            UpAxis::Y => "Y up",
                            UpAxis::Z => "Z up",
                        })
                        .show_ui(ui, |ui| {
                            for (a, t) in [(UpAxis::Y, "Y up"), (UpAxis::Z, "Z up")] {
                                if ui.selectable_label(s.options.up_axis == a, t).clicked() {
                                    s.options.up_axis = a;
                                    reparse = true;
                                }
                            }
                        });
                    ui.end_row();
                    ui.label("Symbol faces direction");
                    egui::ComboBox::from_id_salt("i3d_facing")
                        .selected_text(s.facing.label(s.options.up_axis))
                        .show_ui(ui, |ui| {
                            for f in Facing::ALL {
                                if ui
                                    .selectable_label(s.facing == f, f.label(s.options.up_axis))
                                    .clicked()
                                {
                                    s.facing = f;
                                    reparse = true;
                                }
                            }
                        });
                    ui.end_row();
                    ui.label("Placement");
                    egui::ComboBox::from_id_salt("i3d_placement")
                        .selected_text(s.placement.map_or("Automatic", placement_name))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut s.placement, None, "Automatic");
                            for p in [
                                Placement::WallMounted,
                                Placement::FreeStanding,
                                Placement::Ceiling,
                                Placement::Countertop,
                            ] {
                                ui.selectable_value(&mut s.placement, Some(p), placement_name(p));
                            }
                        });
                    ui.end_row();
                    ui.label("Elevation");
                    ui.add(egui::DragValue::new(&mut s.elevation).speed(0.5).range(0.0..=240.0).suffix(" in"));
                    ui.end_row();
                    ui.label("Layer");
                    ui.add(
                        egui::TextEdit::singleline(&mut s.layer)
                            .hint_text("Automatic")
                            .desired_width(160.0),
                    );
                    ui.end_row();
                    ui.label("Size");
                    if ui.checkbox(&mut s.resize, "Resize the symbol").changed() {
                        reshape = true;
                    }
                    ui.end_row();
                });
            if s.resize {
                ui.horizontal(|ui| {
                    ui.add_space(8.0);
                    for (label, side) in [
                        ("W", Side::Width),
                        ("D", Side::Depth),
                        ("H", Side::Height),
                    ] {
                        ui.label(label);
                        let mut v = match side {
                            Side::Width => s.width,
                            Side::Depth => s.depth,
                            Side::Height => s.height,
                        };
                        if ui
                            .add(egui::DragValue::new(&mut v).speed(0.25).range(0.1..=2400.0).suffix(" in"))
                            .changed()
                        {
                            s.set_side(side, v);
                        }
                    }
                    if ui.checkbox(&mut s.keep_proportions, "Keep proportions").changed() {
                        reshape = true;
                    }
                });
            }
            ui.separator();
            if ui
                .checkbox(&mut s.map_materials, "Map textures and colors to Plan Studio materials")
                .changed()
            {
                reshape = true;
            }
            let matches = s.matches.clone();
            egui::ScrollArea::vertical()
                .max_height(110.0)
                .id_salt("i3d_materials")
                .show(ui, |ui| {
                    for m in &matches {
                        let what = match (&m.texture, &m.file_material) {
                            (Some(t), _) => format!("texture {t}"),
                            (None, Some(f)) => format!("material {f}"),
                            _ => "no material".to_string(),
                        };
                        let to = m.plan_material.as_deref().map_or("no match".to_string(), |p| {
                            if m.applied {
                                format!("{p} (color applied)")
                            } else {
                                format!("nearest: {p}")
                            }
                        });
                        let name = if m.part.is_empty() { "(part)" } else { m.part.as_str() };
                        ui.horizontal(|ui| {
                            if let Some(c) = m.color {
                                let (r, _) =
                                    ui.allocate_exact_size(egui::Vec2::splat(12.0), egui::Sense::hover());
                                ui.painter()
                                    .rect_filled(r, 2.0, Color32::from_rgb(c[0], c[1], c[2]));
                            }
                            ui.label(format!("{name}: {what} -> {to}"));
                        });
                    }
                });
            if !s.error.is_empty() {
                ui.colored_label(RED, &s.error);
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(s.parsed().is_ok(), egui::Button::new("Import"))
                    .clicked()
                {
                    go = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if reparse {
        s.reparse();
    } else if reshape {
        s.touch();
    }
    // Typed names and the like are not part of the shape.
    if go {
        WINDOW.with(|w| *w.borrow_mut() = Some(s));
        accept(cx);
        return;
    }
    if open && !cancel {
        WINDOW.with(|w| {
            let mut slot = w.borrow_mut();
            if slot.is_none() {
                *slot = Some(s);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_names_find_plan_materials() {
        let lib = plan_materials::core_library();
        let (name, _) = match_texture(&lib, "textures/oak_floor_01.jpg").expect("oak");
        assert!(name.to_ascii_lowercase().contains("oak"), "{name}");
        assert!(match_texture(&lib, "zzqq_diffuse.png").is_none());
        // Noise words alone match nothing.
        assert!(match_texture(&lib, "texture_diffuse.png").is_none());
    }

    #[test]
    fn mapping_applies_the_found_color_and_keeps_the_rest() {
        let lib = plan_materials::core_library();
        let mut m = ImportedModel {
            parts: vec![
                plan_import::ImportedPart {
                    name: "top".into(),
                    texture: Some("oak.jpg".into()),
                    color: Some([1, 2, 3]),
                    positions: vec![[0.0; 3]],
                    ..Default::default()
                },
                plan_import::ImportedPart {
                    name: "legs".into(),
                    color: Some([200, 30, 30]),
                    positions: vec![[0.0; 3]],
                    ..Default::default()
                },
            ],
        };
        let r = map_materials(&mut m, &lib, true);
        assert!(r[0].applied && r[0].plan_material.is_some());
        assert_ne!(m.parts[0].color, Some([1, 2, 3]));
        assert!(!r[1].applied);
        assert_eq!(m.parts[1].color, Some([200, 30, 30]));
        assert!(r[1].plan_material.is_some(), "nearest named material");
        // Off: nothing changes.
        let mut again = ImportedModel {
            parts: vec![m.parts[0].clone()],
        };
        again.parts[0].color = Some([1, 2, 3]);
        let r = map_materials(&mut again, &lib, false);
        assert!(!r[0].applied);
        assert_eq!(again.parts[0].color, Some([1, 2, 3]));
    }

    #[test]
    fn folder_text_round_trips_with_the_user_root() {
        assert_eq!(parse_folder("Furniture > Chairs")[0], USER_ROOT);
        let f = parse_folder("User > 3D Models");
        assert_eq!(folder_text(&f), "User > 3D Models");
    }
}
