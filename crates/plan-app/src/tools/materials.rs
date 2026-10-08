//! Material tools: the Materials list, Material Painter, Adjust Materials and
//! Material Builder (3D menu and the 3D toolbar row).
//!
//! * The **material library** is `plan_materials::core_library()` plus the
//!   user's own materials in `~/.plan-studio/materials.json` (what the
//!   Material Builder saves). One material is *active*.
//! * **Material Painter** is a mode of the 3D view: with it on, a click on a
//!   surface applies the active material to the object that surface belongs
//!   to (the object id of the picked mesh, `shell/view3d_panel/pick.rs`)
//!   instead of selecting it. Material Eyedropper makes the clicked object's
//!   material the active one; Delete Surface removes its override.
//! * The override is stored per object in `Project::object_materials`
//!   (`plan_core::object_materials`), by library name: one material for the
//!   whole object or per part (Adjust Materials lists the parts of
//!   `plan_materials::default_assignments_for`).
//! * [`apply_overrides`] recolors the 3D scene: the viewport shades with a
//!   fixed set of scene materials, so an override shows the scene material
//!   that is closest to the library material (`plan_materials::scene_material`).

use crate::editor::selection::ObjectRef;
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Color32};
use plan_3d::{Material, Scene};
use plan_core::object_materials::WHOLE_OBJECT;
use plan_core::{Id, OpeningKind, Project};
use plan_materials::{
    build_material, core_library, default_assignments_for, scene_material, MaterialDef,
    MaterialLibrary, Pattern,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// Command ids (menu rows and toolbar buttons run them with `Action::Custom`).
pub const PAINTER: &str = "materials.painter";
pub const EYEDROPPER: &str = "materials.eyedropper";
pub const ERASE: &str = "materials.erase";
pub const LIST: &str = "materials.list";
pub const ADJUST: &str = "materials.adjust";
pub const BUILDER: &str = "materials.builder";

/// What a click in the 3D view does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PainterMode {
    /// Clicks select.
    #[default]
    Off,
    /// Apply the active material to the clicked object.
    Paint,
    /// Make the clicked object's material the active one.
    Eyedropper,
    /// Remove the clicked object's override (Delete Surface).
    Erase,
}

// ----- the user library -----

/// `~/.plan-studio/materials.json`.
pub fn user_library_path() -> Option<PathBuf> {
    crate::paths::user_file("materials.json")
}

/// The user library in `path` (empty when the file is missing or unreadable).
pub fn load_user_library_at(path: &Path) -> MaterialLibrary {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| MaterialLibrary::from_json(&t).ok())
        .unwrap_or_default()
}

/// Writes the user library to `path`.
pub fn save_user_library_at(path: &Path, lib: &MaterialLibrary) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let text = lib.to_json().map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

// ----- state -----

/// The Material Builder's draft.
#[derive(Clone)]
struct Builder {
    name: String,
    category: String,
    color: [u8; 3],
    roughness: f32,
    metallic: f32,
    transparency: f32,
    pattern: &'static str,
    texture_path: String,
    /// The user material being edited (kept when saving under its name).
    editing: Option<String>,
    note: String,
}

impl Builder {
    fn new() -> Self {
        Self {
            name: String::new(),
            category: "Custom".into(),
            color: [200, 200, 200],
            roughness: 0.8,
            metallic: 0.0,
            transparency: 0.0,
            pattern: "None",
            texture_path: String::new(),
            editing: None,
            note: String::new(),
        }
    }

    fn from_def(d: &MaterialDef) -> Self {
        Self {
            name: d.name.clone(),
            category: d.category.join(" > "),
            color: d.color,
            roughness: d.roughness,
            metallic: d.metallic,
            transparency: d.transparency,
            pattern: pattern_name(&d.pattern),
            texture_path: d.texture_path.clone().unwrap_or_default(),
            editing: Some(d.name.clone()),
            note: String::new(),
        }
    }

    fn def(&self) -> Result<MaterialDef, String> {
        let mut d = build_material(
            &self.name,
            &self.category,
            self.color,
            self.roughness,
            pattern_by_name(self.pattern),
            &self.texture_path,
        )?;
        d.metallic = self.metallic.clamp(0.0, 1.0);
        d.transparency = self.transparency.clamp(0.0, 1.0);
        Ok(d)
    }
}

#[derive(Default)]
struct State {
    user: Option<MaterialLibrary>,
    active: Option<String>,
    mode: PainterMode,
    list_open: bool,
    adjust_open: bool,
    builder: Option<Builder>,
    search: String,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    static CORE: MaterialLibrary = core_library();
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn user_library() -> MaterialLibrary {
    state(|s| {
        s.user
            .get_or_insert_with(|| {
                user_library_path()
                    .map(|p| load_user_library_at(&p))
                    .unwrap_or_default()
            })
            .clone()
    })
}

/// Core library plus the user's materials (a user material replaces the core
/// one of the same name).
pub fn library() -> MaterialLibrary {
    CORE.with(|c| c.merged_with(&user_library()))
}

/// The material the painter applies.
pub fn active_material() -> Option<String> {
    state(|s| s.active.clone())
}

pub fn set_active(name: Option<String>) {
    state(|s| s.active = name);
}

pub fn painter_mode() -> PainterMode {
    state(|s| s.mode)
}

/// Does a click in the 3D view paint (or sample, or erase) instead of select?
pub fn painter_active() -> bool {
    painter_mode() != PainterMode::Off
}

pub fn set_painter_mode(mode: PainterMode) {
    state(|s| s.mode = mode);
}

/// Is the toolbar toggle for the command `id` lit?
pub fn is_mode_active(id: &str) -> bool {
    match id {
        PAINTER => painter_mode() == PainterMode::Paint,
        EYEDROPPER => painter_mode() == PainterMode::Eyedropper,
        ERASE => painter_mode() == PainterMode::Erase,
        _ => false,
    }
}

// ----- patterns of the builder -----

const PATTERNS: [&str; 14] = [
    "None",
    "Lines",
    "Cross Hatch",
    "Brick",
    "Block",
    "Shingle",
    "Lap Siding",
    "Board and Batten",
    "Tile",
    "Herringbone",
    "Insulation",
    "Concrete",
    "Earth",
    "Grass",
];

/// The pattern called `name` with its usual size.
fn pattern_by_name(name: &str) -> Pattern {
    match name {
        "Lines" => Pattern::Lines {
            angle_deg: 45.0,
            spacing: 3.0,
        },
        "Cross Hatch" => Pattern::CrossHatch {
            angle_deg: 45.0,
            spacing: 4.0,
        },
        "Brick" => Pattern::brick(),
        "Block" => Pattern::block(),
        "Shingle" => Pattern::shingle(),
        "Lap Siding" => Pattern::lap_siding(),
        "Board and Batten" => Pattern::board_and_batten(),
        "Tile" => Pattern::Tile { w: 12.0, h: 12.0 },
        "Herringbone" => Pattern::Herringbone {
            length: 12.0,
            width: 3.0,
        },
        "Insulation" => Pattern::Insulation,
        "Concrete" => Pattern::Concrete,
        "Earth" => Pattern::Earth,
        "Grass" => Pattern::Grass,
        _ => Pattern::None,
    }
}

fn pattern_name(p: &Pattern) -> &'static str {
    match p {
        Pattern::None => "None",
        Pattern::Lines { .. } => "Lines",
        Pattern::CrossHatch { .. } => "Cross Hatch",
        Pattern::Brick { .. } => "Brick",
        Pattern::Block { .. } => "Block",
        Pattern::Shingle { .. } => "Shingle",
        Pattern::LapSiding { .. } => "Lap Siding",
        Pattern::BoardAndBatten { .. } => "Board and Batten",
        Pattern::Tile { .. } => "Tile",
        Pattern::Herringbone { .. } => "Herringbone",
        Pattern::Insulation => "Insulation",
        Pattern::Concrete => "Concrete",
        Pattern::Earth => "Earth",
        Pattern::Grass => "Grass",
    }
}

// ----- objects and parts -----

/// The id the 3D meshes of `obj` carry as their `object_id`, for the kinds
/// that have meshes.
pub fn object_id_of(obj: ObjectRef) -> Option<Id> {
    match obj {
        ObjectRef::Opening(i)
        | ObjectRef::Wall(i)
        | ObjectRef::Symbol(i)
        | ObjectRef::Cabinet(i)
        | ObjectRef::Stair(i)
        | ObjectRef::RoofPlane(i)
        | ObjectRef::Foundation(i)
        | ObjectRef::Framing(i)
        | ObjectRef::Detail(i)
        | ObjectRef::Device(i) => Some(i),
        _ => None,
    }
}

/// The component kind of `default_assignments_for` an object belongs to.
fn kind_of(project: &Project, floor: usize, obj: ObjectRef) -> Option<&'static str> {
    Some(match obj {
        ObjectRef::Wall(_) => "Wall",
        ObjectRef::Opening(id) => {
            let o = project
                .floors
                .get(floor)?
                .openings
                .iter()
                .find(|o| o.id == id)?;
            if o.kind == OpeningKind::Door {
                "Door"
            } else {
                "Window"
            }
        }
        ObjectRef::Cabinet(_) => "Cabinet",
        ObjectRef::RoofPlane(_) => "Roof",
        _ => return None,
    })
}

/// The parts of an object (the component names of its kind).
pub fn parts_of(project: &Project, floor: usize, obj: ObjectRef) -> Vec<String> {
    kind_of(project, floor, obj)
        .map(|k| {
            default_assignments_for(k)
                .into_iter()
                .map(|a| a.component)
                .collect()
        })
        .unwrap_or_default()
}

/// The parts a mesh of scene material `m` is made of.
fn parts_of_mesh(m: Material) -> &'static [&'static str] {
    match m {
        Material::WallExterior
        | Material::Stucco
        | Material::Siding
        | Material::Brick
        | Material::Stone
        | Material::Concrete => &["Exterior Wall Surface"],
        Material::WallInterior => &["Interior Wall Surface"],
        Material::Floor => &["Floor Finish"],
        Material::Ceiling => &["Ceiling Finish"],
        Material::DoorPanel => &["Door Panel"],
        Material::WindowGlass | Material::Glass => &["Glass"],
        Material::WindowFrame => &["Frame", "Sash"],
        Material::Trim => &[
            "Casing",
            "Jamb",
            "Sill",
            "Base Molding",
            "Crown",
            "Fascia",
            "Soffit",
        ],
        Material::Metal => &["Hardware", "Gutter"],
        Material::Roof => &["Roof Surface"],
        Material::Framing => &["Sill Plate", "Box"],
        _ => &[],
    }
}

/// The scene material the project's overrides give a mesh of `object` that
/// would otherwise be drawn as `current`; `None` when nothing overrides it.
pub fn override_for_mesh(
    project: &Project,
    lib: &MaterialLibrary,
    object: Id,
    current: Material,
) -> Option<Material> {
    let parts = project.object_materials_of(object);
    if parts.is_empty() {
        return None;
    }
    let by_part = parts_of_mesh(current)
        .iter()
        .find_map(|c| parts.iter().find(|p| p.part == *c));
    let chosen = by_part.or_else(|| {
        // The whole-object material leaves windows' glass clear unless it is
        // glass itself.
        parts.iter().find(|p| p.part == WHOLE_OBJECT)
    })?;
    let m = scene_material(lib.find(&chosen.material)?);
    let glass = matches!(current, Material::WindowGlass | Material::Glass);
    if glass && chosen.part == WHOLE_OBJECT && !matches!(m, Material::Glass) {
        return None;
    }
    Some(m)
}

/// Recolors the meshes of `scene` whose object has a material override.
/// Returns how many meshes changed.
pub fn apply_overrides(project: &Project, scene: &mut Scene) -> usize {
    if project.object_materials.is_empty() {
        return 0;
    }
    let lib = library();
    let mut n = 0;
    for mesh in &mut scene.meshes {
        let Some(id) = mesh.object_id else { continue };
        if let Some(m) = override_for_mesh(project, &lib, id, mesh.material) {
            if m != mesh.material {
                mesh.material = m;
                n += 1;
            }
        }
    }
    n
}

// ----- painting -----

/// A click of the painter on `obj` (the pick hook of the 3D view calls this
/// when [`painter_active`]). Paint applies the active material to the whole
/// object as one undo step; the eyedropper makes the object's whole-object
/// material active; Delete Surface removes the object's overrides. Returns
/// whether the plan changed.
pub fn paint_object(cx: &mut EditorContext, floor: usize, obj: ObjectRef) -> bool {
    let Some(id) = object_id_of(obj) else {
        cx.status = "That surface belongs to no object that can take a material".into();
        return false;
    };
    let _ = floor;
    match painter_mode() {
        PainterMode::Off => false,
        PainterMode::Paint => {
            let Some(name) = active_material() else {
                cx.status = "Pick a material in the Materials list first (3D > Materials)".into();
                return false;
            };
            if library().find(&name).is_none() {
                cx.status = format!("The material {name} is not in the library");
                return false;
            }
            if cx.project.object_material(id, WHOLE_OBJECT) == Some(name.as_str())
                && cx.project.object_materials_of(id).len() == 1
            {
                return false;
            }
            cx.begin_change("Paint Material");
            cx.project.set_object_material(id, WHOLE_OBJECT, &name);
            cx.mark_dirty();
            cx.status = format!("Painted with {name}");
            true
        }
        PainterMode::Eyedropper => {
            match cx
                .project
                .object_material(id, WHOLE_OBJECT)
                .map(str::to_string)
            {
                Some(name) => {
                    cx.status = format!("Material {name} is active");
                    set_active(Some(name));
                }
                None => cx.status = "That object has no painted material".into(),
            }
            false
        }
        PainterMode::Erase => {
            if cx.project.object_materials_of(id).is_empty() {
                return false;
            }
            cx.begin_change("Delete Surface");
            cx.project.clear_object_material(id, None);
            cx.mark_dirty();
            cx.status = "Removed the painted material".into();
            true
        }
    }
}

// ----- commands -----

/// Runs a material command by id; false when the id is not one of ours.
pub fn run_command(cx: &mut EditorContext, id: &str) -> bool {
    let toggle = |mode: PainterMode, cx: &mut EditorContext, hint: &str| {
        if painter_mode() == mode {
            set_painter_mode(PainterMode::Off);
            cx.status = "Material Painter off".into();
        } else {
            set_painter_mode(mode);
            cx.status = hint.to_string();
            if mode == PainterMode::Paint && active_material().is_none() {
                state(|s| s.list_open = true);
            }
        }
    };
    match id {
        PAINTER => toggle(
            PainterMode::Paint,
            cx,
            "Material Painter: click a surface in the 3D view (Esc turns it off)",
        ),
        EYEDROPPER => toggle(
            PainterMode::Eyedropper,
            cx,
            "Material Eyedropper: click a painted surface in the 3D view",
        ),
        ERASE => toggle(
            PainterMode::Erase,
            cx,
            "Delete Surface: click a painted surface in the 3D view",
        ),
        LIST => state(|s| s.list_open = true),
        ADJUST => state(|s| s.adjust_open = true),
        BUILDER => {
            let lib = library();
            state(|s| {
                s.builder = Some(match s.active.as_deref().and_then(|n| lib.find(n)) {
                    Some(d) => Builder::from_def(d),
                    None => Builder::new(),
                })
            });
        }
        _ => return false,
    }
    true
}

// ----- windows -----

fn swatch(ui: &mut egui::Ui, c: [u8; 3]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 2.0, Color32::from_rgb(c[0], c[1], c[2]));
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0_f32, Color32::from_gray(90)),
        egui::StrokeKind::Inside,
    );
}

/// Draws the Materials list, Adjust Materials and Material Builder windows.
pub fn show_windows(ctx: &egui::Context, cx: &mut EditorContext) {
    if state(|s| s.list_open) {
        list_window(ctx);
    }
    if state(|s| s.adjust_open) {
        adjust_window(ctx, cx);
    }
    if state(|s| s.builder.is_some()) {
        builder_window(ctx);
    }
}

fn list_window(ctx: &egui::Context) {
    let mut open = true;
    egui::Window::new("Materials")
        .id(egui::Id::new("materials_list"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([320.0, 420.0])
        .pivot(Align2::LEFT_TOP)
        .default_pos(ctx.screen_rect().left_top() + egui::vec2(24.0, 110.0))
        .show(ctx, |ui| {
            let lib = library();
            let mut search = state(|s| s.search.clone());
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.add(egui::TextEdit::singleline(&mut search).desired_width(160.0));
                if ui.button("New...").clicked() {
                    state(|s| s.builder = Some(Builder::new()));
                }
            });
            state(|s| s.search = search.clone());
            let active = active_material();
            ui.horizontal(|ui| {
                ui.label("Active:");
                match active.as_deref().and_then(|n| lib.find(n)) {
                    Some(d) => {
                        swatch(ui, d.color);
                        ui.strong(&d.name);
                    }
                    None => {
                        ui.weak("none");
                    }
                }
            });
            ui.horizontal(|ui| {
                let mode = painter_mode();
                for (m, label) in [
                    (PainterMode::Paint, "Paint"),
                    (PainterMode::Eyedropper, "Eyedropper"),
                    (PainterMode::Erase, "Delete Surface"),
                ] {
                    if ui.selectable_label(mode == m, label).clicked() {
                        set_painter_mode(if mode == m { PainterMode::Off } else { m });
                    }
                }
                if ui
                    .add_enabled(active.is_some(), egui::Button::new("Edit..."))
                    .clicked()
                {
                    if let Some(d) = active.as_deref().and_then(|n| lib.find(n)) {
                        let b = Builder::from_def(d);
                        state(|s| s.builder = Some(b));
                    }
                }
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                if search.trim().is_empty() {
                    for (cat, mats) in lib.by_category() {
                        egui::CollapsingHeader::new(&cat)
                            .id_salt(("mat_cat", cat.clone()))
                            .default_open(false)
                            .show(ui, |ui| {
                                for m in mats {
                                    material_row(ui, m, active.as_deref());
                                }
                            });
                    }
                } else {
                    for m in lib.search(&search) {
                        material_row(ui, m, active.as_deref());
                    }
                }
            });
        });
    if !open {
        state(|s| s.list_open = false);
    }
}

fn material_row(ui: &mut egui::Ui, m: &MaterialDef, active: Option<&str>) {
    ui.horizontal(|ui| {
        swatch(ui, m.color);
        if ui
            .selectable_label(active == Some(m.name.as_str()), &m.name)
            .clicked()
        {
            set_active(Some(m.name.clone()));
        }
    });
}

fn adjust_window(ctx: &egui::Context, cx: &mut EditorContext) {
    let mut open = true;
    egui::Window::new("Adjust Materials")
        .id(egui::Id::new("materials_adjust"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::RIGHT_TOP)
        .default_pos(ctx.screen_rect().right_top() + egui::vec2(-24.0, 110.0))
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            let Some(obj) = cx.selection.single() else {
                ui.weak("Select one object (in the plan or the 3D view).");
                return;
            };
            let Some(id) = object_id_of(obj) else {
                ui.weak("That kind of object takes no material.");
                return;
            };
            let lib = library();
            let mut parts = vec![(WHOLE_OBJECT.to_string(), "Whole object".to_string())];
            parts.extend(
                parts_of(&cx.project, cx.floor, obj)
                    .into_iter()
                    .map(|p| (p.clone(), p)),
            );
            let defaults = kind_of(&cx.project, cx.floor, obj)
                .map(default_assignments_for)
                .unwrap_or_default();
            let mut pick: Option<(String, Option<String>)> = None;
            egui::Grid::new("adjust_materials_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    for (part, label) in &parts {
                        ui.label(label);
                        let own = cx.project.object_material(id, part).map(str::to_string);
                        let own_exact = cx
                            .project
                            .object_materials_of(id)
                            .iter()
                            .any(|p| p.part == *part);
                        let default = defaults
                            .iter()
                            .find(|a| a.component == *part)
                            .map(|a| a.material.clone());
                        let shown = match (&own, own_exact) {
                            (Some(n), true) => n.clone(),
                            _ => default.clone().map_or_else(
                                || "Default".to_string(),
                                |d| format!("{d} (default)"),
                            ),
                        };
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(("adjust_part", part))
                                .selected_text(shown)
                                .width(210.0)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(!own_exact, "Default").clicked() {
                                        pick = Some((part.clone(), None));
                                    }
                                    for m in &lib.materials {
                                        if ui
                                            .selectable_label(
                                                own_exact && own.as_deref() == Some(&m.name),
                                                &m.name,
                                            )
                                            .clicked()
                                        {
                                            pick = Some((part.clone(), Some(m.name.clone())));
                                        }
                                    }
                                });
                            if let Some(n) = own.as_deref().filter(|_| own_exact) {
                                if let Some(d) = lib.find(n) {
                                    swatch(ui, d.color);
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            if let Some((part, material)) = pick {
                cx.begin_change("Adjust Materials");
                match material {
                    Some(m) => {
                        cx.project.set_object_material(id, &part, &m);
                    }
                    None => {
                        cx.project.clear_object_material(id, Some(&part));
                    }
                }
                cx.mark_dirty();
            }
            ui.separator();
            if ui
                .add_enabled(
                    !cx.project.object_materials_of(id).is_empty(),
                    egui::Button::new("Clear all overrides"),
                )
                .clicked()
            {
                cx.begin_change("Delete Surface");
                cx.project.clear_object_material(id, None);
                cx.mark_dirty();
            }
        });
    if !open {
        state(|s| s.adjust_open = false);
    }
}

fn builder_window(ctx: &egui::Context) {
    let mut open = true;
    let mut b = state(|s| s.builder.clone()).unwrap_or_else(Builder::new);
    let mut save = false;
    let mut delete = false;
    egui::Window::new("Material Builder")
        .id(egui::Id::new("materials_builder"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            egui::Grid::new("builder_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.add(egui::TextEdit::singleline(&mut b.name).desired_width(220.0));
                    ui.end_row();
                    ui.label("Category");
                    ui.add(
                        egui::TextEdit::singleline(&mut b.category)
                            .hint_text("Siding > Custom")
                            .desired_width(220.0),
                    );
                    ui.end_row();
                    ui.label("Color");
                    ui.color_edit_button_srgb(&mut b.color);
                    ui.end_row();
                    ui.label("Roughness");
                    ui.add(egui::Slider::new(&mut b.roughness, 0.0..=1.0));
                    ui.end_row();
                    ui.label("Metallic");
                    ui.add(egui::Slider::new(&mut b.metallic, 0.0..=1.0));
                    ui.end_row();
                    ui.label("Transparency");
                    ui.add(egui::Slider::new(&mut b.transparency, 0.0..=1.0));
                    ui.end_row();
                    ui.label("2D pattern");
                    egui::ComboBox::from_id_salt("builder_pattern")
                        .selected_text(b.pattern)
                        .show_ui(ui, |ui| {
                            for p in PATTERNS {
                                ui.selectable_value(&mut b.pattern, p, p);
                            }
                        });
                    ui.end_row();
                    ui.label("Texture image");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut b.texture_path)
                                .hint_text("optional PNG or JPEG")
                                .desired_width(170.0),
                        );
                        if ui.button("Browse...").clicked() {
                            if let Some(p) = rfd::FileDialog::new()
                                .add_filter("Image", &["png", "jpg", "jpeg"])
                                .pick_file()
                            {
                                b.texture_path = p.to_string_lossy().into_owned();
                            }
                        }
                    });
                    ui.end_row();
                });
            ui.horizontal(|ui| {
                swatch(ui, b.color);
                match b.def() {
                    Ok(d) => ui.weak(format!("Shows in 3D as {}", scene_material(&d).name())),
                    Err(e) => ui.weak(e),
                };
            });
            if !b.note.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, b.note.clone());
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Save to My Materials").clicked() {
                    save = true;
                }
                let user_has = b
                    .editing
                    .as_deref()
                    .is_some_and(|n| user_library().find(n).is_some());
                if ui
                    .add_enabled(user_has, egui::Button::new("Delete"))
                    .clicked()
                {
                    delete = true;
                }
            });
        });
    if save {
        match b.def().and_then(|d| save_to_user_library(&d)) {
            Ok(name) => {
                set_active(Some(name));
                b.note.clear();
                open = false;
            }
            Err(e) => b.note = e,
        }
    }
    if delete {
        if let Some(n) = b.editing.clone() {
            b.note = remove_from_user_library(&n).err().unwrap_or_default();
            if b.note.is_empty() {
                open = false;
            }
        }
    }
    state(|s| s.builder = open.then_some(b));
}

/// Adds (or replaces) `def` in the user library and saves the file; returns
/// the material's name.
pub fn save_to_user_library(def: &MaterialDef) -> Result<String, String> {
    let mut lib = user_library();
    lib.add(def.clone());
    if let Some(p) = user_library_path() {
        save_user_library_at(&p, &lib).map_err(|e| format!("Could not save: {e}"))?;
    }
    state(|s| s.user = Some(lib));
    Ok(def.name.clone())
}

/// Removes a material from the user library and saves the file.
pub fn remove_from_user_library(name: &str) -> Result<(), String> {
    let mut lib = user_library();
    if !lib.remove(name) {
        return Ok(());
    }
    if let Some(p) = user_library_path() {
        save_user_library_at(&p, &lib).map_err(|e| format!("Could not save: {e}"))?;
    }
    state(|s| {
        s.user = Some(lib);
        if s.active.as_deref() == Some(name) {
            s.active = None;
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_3d::{Mesh, Vertex};
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn cx_with_wall() -> (EditorContext, Id) {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let w = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        (cx, w)
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("plan-studio-mats-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn scene_with(object: Id, material: Material) -> Scene {
        let v = |x: f32| Vertex {
            position: [x, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        let mut scene = Scene::default();
        scene.meshes.push(Mesh {
            vertices: vec![v(0.0), v(1.0), v(2.0)],
            indices: vec![0, 1, 2],
            material,
            object_id: Some(object),
        });
        scene
    }

    #[test]
    fn the_painter_applies_the_active_material_through_the_pick_hook() {
        let (mut cx, wall) = cx_with_wall();
        set_active(None);
        set_painter_mode(PainterMode::Paint);
        // No active material: nothing happens and the status says why.
        assert!(!paint_object(&mut cx, 0, ObjectRef::Wall(wall)));
        assert!(cx.status.contains("Pick a material"));
        let name = library()
            .materials
            .iter()
            .find(|m| m.category.first().map(String::as_str) == Some("Masonry"))
            .unwrap()
            .name
            .clone();
        set_active(Some(name.clone()));
        assert!(paint_object(&mut cx, 0, ObjectRef::Wall(wall)));
        assert_eq!(
            cx.project.object_material(wall, WHOLE_OBJECT),
            Some(name.as_str())
        );
        assert_eq!(cx.undo_label(), Some("Paint Material"));
        // The same click again changes nothing.
        assert!(!paint_object(&mut cx, 0, ObjectRef::Wall(wall)));
        cx.undo();
        assert_eq!(cx.project.object_material(wall, WHOLE_OBJECT), None);
        set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn eyedropper_and_delete_surface() {
        let (mut cx, wall) = cx_with_wall();
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Drywall");
        set_active(None);
        set_painter_mode(PainterMode::Eyedropper);
        assert!(!paint_object(&mut cx, 0, ObjectRef::Wall(wall)));
        assert_eq!(active_material().as_deref(), Some("Drywall"));
        set_painter_mode(PainterMode::Erase);
        assert!(paint_object(&mut cx, 0, ObjectRef::Wall(wall)));
        assert!(cx.project.object_materials.is_empty());
        assert!(!paint_object(&mut cx, 0, ObjectRef::Wall(wall)));
        // A room or the terrain surface has no object to paint.
        set_painter_mode(PainterMode::Paint);
        assert!(!paint_object(&mut cx, 0, ObjectRef::Terrain));
        set_painter_mode(PainterMode::Off);
    }

    #[test]
    fn overrides_recolor_the_scene_by_part_and_keep_glass_clear() {
        let (mut cx, wall) = cx_with_wall();
        cx.project
            .set_object_material(wall, "Interior Wall Surface", "Color – Bone");
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Painted White Trim");
        let mut scene = scene_with(wall, Material::WallInterior);
        scene
            .meshes
            .extend(scene_with(wall, Material::WallExterior).meshes);
        scene
            .meshes
            .extend(scene_with(wall, Material::WindowGlass).meshes);
        scene.meshes.extend(scene_with(999, Material::Roof).meshes);
        assert_eq!(apply_overrides(&cx.project, &mut scene), 2);
        let lib = library();
        // The interior face took its own part's material, the exterior the
        // whole-object one, the glass stayed clear and the other object
        // was left alone.
        assert_eq!(
            scene.meshes[0].material,
            scene_material(lib.find("Color – Bone").unwrap())
        );
        assert_eq!(scene.meshes[1].material, Material::Trim);
        assert_eq!(scene.meshes[2].material, Material::WindowGlass);
        assert_eq!(scene.meshes[3].material, Material::Roof);
    }

    #[test]
    fn overrides_survive_save_and_load() {
        let (mut cx, wall) = cx_with_wall();
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Drywall");
        cx.project
            .set_object_material(wall, "Sill Plate", "Fir Framing");
        let back = Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(back.object_materials, cx.project.object_materials);
        assert_eq!(
            back.object_material(wall, "Sill Plate"),
            Some("Fir Framing")
        );
    }

    #[test]
    fn parts_come_from_the_default_assignments() {
        let (cx, wall) = cx_with_wall();
        let parts = parts_of(&cx.project, 0, ObjectRef::Wall(wall));
        assert!(parts.contains(&"Exterior Wall Surface".to_string()));
        assert!(parts_of(&cx.project, 0, ObjectRef::Terrain).is_empty());
        assert_eq!(object_id_of(ObjectRef::Wall(wall)), Some(wall));
        assert_eq!(object_id_of(ObjectRef::Terrain), None);
    }

    #[test]
    fn the_builder_saves_to_the_user_library_file() {
        let path = scratch("materials.json");
        let _ = std::fs::remove_file(&path);
        assert!(load_user_library_at(&path).materials.is_empty());
        let mut b = Builder::new();
        b.name = "Barn Red".into();
        b.color = [160, 75, 55];
        b.pattern = "Brick";
        b.texture_path = "/tmp/red.png".into();
        let def = b.def().unwrap();
        let mut lib = MaterialLibrary::default();
        lib.add(def.clone());
        save_user_library_at(&path, &lib).unwrap();
        let back = load_user_library_at(&path);
        assert_eq!(back.find("Barn Red"), Some(&def));
        assert_eq!(pattern_name(&back.materials[0].pattern), "Brick");
        assert_eq!(
            back.materials[0].texture_path.as_deref(),
            Some("/tmp/red.png")
        );
        // A nameless draft is refused.
        b.name.clear();
        assert!(b.def().is_err());
        let merged = core_library().merged_with(&back);
        assert!(merged.find("Barn Red").is_some() && merged.find("Drywall").is_some());
        // Every builder pattern name maps back to itself.
        for p in PATTERNS {
            assert_eq!(pattern_name(&pattern_by_name(p)), p);
        }
    }

    #[test]
    fn commands_toggle_the_painter_and_open_windows() {
        let (mut cx, _) = cx_with_wall();
        set_painter_mode(PainterMode::Off);
        assert!(run_command(&mut cx, PAINTER));
        assert!(is_mode_active(PAINTER) && painter_active());
        assert!(run_command(&mut cx, PAINTER));
        assert!(!painter_active());
        assert!(run_command(&mut cx, ERASE) && is_mode_active(ERASE));
        set_painter_mode(PainterMode::Off);
        assert!(run_command(&mut cx, ADJUST));
        assert!(run_command(&mut cx, BUILDER));
        assert!(!run_command(&mut cx, "materials.nope"));
        // The windows draw headlessly.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &mut cx));
    }
}
