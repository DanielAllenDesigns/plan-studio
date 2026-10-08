//! Material tools: the Materials list, Material Painter, Adjust Materials and
//! Material Builder (3D menu and the 3D toolbar row).
//!
//! * The **material library** is `plan_materials::core_library()` plus the
//!   user's own materials in `~/.plan-studio/materials.json` (what the
//!   Material Builder saves). One material is *active*.
//! * **Material Painter** is a mode of the 3D view: with it on, a click on a
//!   surface applies the active material to what the palette's mode and
//!   scope reach from there (Component, Object, Room, Floor, Plan, Blend
//!   Colors; see [`paint`]) instead of selecting it. Material Eyedropper
//!   makes the clicked surface's material the active one, Object Eyedropper
//!   remembers an object's whole set of paint, Adjust Material Definition
//!   opens the clicked material's specification ([`spec`]) and Delete Surface
//!   removes the paint.
//! * The **Material Specification** dialog ([`spec`]: Pattern, Texture,
//!   Properties and Materials List tabs) edits one library material; the
//!   **Materials Defaults** window ([`defaults`]) sets the material of each
//!   object class's parts for the whole plan; the **library browser**
//!   ([`browser`]) lists the materials in the Library dock; the **surfaces
//!   take-off** ([`surfaces`]) adds the areas up per material for the
//!   Materials List window.
//! * The override is stored per object in `Project::object_materials`
//!   (`plan_core::object_materials`), by library name: one material for the
//!   whole object or per part (Adjust Materials lists the parts of
//!   `plan_materials::default_assignments_for`).
//! * [`apply_overrides`] recolors the 3D scene: a painted mesh is drawn as the
//!   scene material that stands for the library material
//!   (`plan_materials::scene_material`, which keeps its gloss and clear glass)
//!   with the library material's exact colour in `Mesh::color`; a library
//!   material with a bitmap (a Material Builder image, a generated texture)
//!   also gets that bitmap per mesh ([`painted_textures`]).

use crate::editor::selection::ObjectRef;
use crate::editor::EditorContext;
use eframe::egui::{self, Align2, Color32};
use plan_3d::{Material, Scene};
use plan_core::object_materials::WHOLE_OBJECT;
use plan_core::{Id, OpeningKind, Project};
use plan_materials::{
    core_library, default_assignments_for, scene_material, MaterialDef, MaterialLibrary, PaintMode,
    PaintScope, Pattern,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

pub mod browser;
pub mod defaults;
pub mod paint;
pub mod spec;
pub mod surfaces;

pub use paint::{note_pick, paint_click};

/// Command ids (menu rows and toolbar buttons run them with `Action::Custom`).
pub const PAINTER: &str = "materials.painter";
pub const EYEDROPPER: &str = "materials.eyedropper";
pub const ERASE: &str = "materials.erase";
pub const LIST: &str = "materials.list";
pub const ADJUST: &str = "materials.adjust";
pub const BUILDER: &str = "materials.builder";
/// Object Eyedropper: copies an object's whole set of paint.
pub const OBJECT_EYEDROPPER: &str = "materials.object_eyedropper";
/// Adjust Material Definition: click a surface to open its specification.
pub const ADJUST_DEFINITION: &str = "materials.adjust_definition";
/// Default Settings > Materials.
pub const DEFAULTS: &str = "materials.defaults";
/// The palette's Use Default Material button.
pub const USE_DEFAULT: &str = "materials.use_default";

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
    /// Copy the clicked object's whole set of paint.
    ObjectEyedropper,
    /// Open the specification of the clicked surface's material.
    Adjust,
}

// ----- the user library -----

/// `~/.plan-studio/materials.json`. Tests never touch the real file: they
/// have no path unless one is set with `set_user_path_for_test`.
pub fn user_library_path() -> Option<PathBuf> {
    #[cfg(test)]
    {
        TEST_PATH.with(|p| p.borrow().clone())
    }
    #[cfg(not(test))]
    {
        crate::paths::user_file("materials.json")
    }
}

#[cfg(test)]
thread_local! {
    static TEST_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

/// Points the user library at `path` for this thread (tests).
#[cfg(test)]
pub fn set_user_path_for_test(path: Option<PathBuf>) {
    TEST_PATH.with(|p| *p.borrow_mut() = path);
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

#[derive(Default)]
struct State {
    user: Option<MaterialLibrary>,
    active: Option<String>,
    mode: PainterMode,
    list_open: bool,
    adjust_open: bool,
    defaults_open: bool,
    /// The Material Specification being edited.
    spec: Option<spec::MaterialSpec>,
    /// The draft shown live in the 3D view while the specification is open.
    preview: Option<MaterialDef>,
    search: String,
    /// The Library dock lists materials instead of library objects.
    browse_materials: bool,
    browser: browser::BrowserState,
    /// Material Painter palette: what a click reaches, which of it is
    /// painted, and whether the click puts the default material back.
    paint_mode: PaintMode,
    scope: PaintScope,
    use_default: bool,
    /// Percent of the active material Blend Colors mixes in.
    blend_pct: u8,
    /// The paint the Object Eyedropper copied.
    object_set: Option<paint::ObjectSet>,
    /// Bumped whenever the user library changes (the 3D view repaints the
    /// painted objects).
    revision: u64,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        blend_pct: 50,
        ..State::default()
    });
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

/// Changes whenever the user's material library does: a painted object must
/// be drawn again when the colour of its material was edited.
pub fn library_revision() -> u64 {
    state(|s| s.revision)
}

/// Core library plus the user's materials (a user material replaces the core
/// one of the same name).
pub fn library() -> MaterialLibrary {
    let mut lib = CORE.with(|c| c.merged_with(&user_library()));
    // The specification being edited shows live in the 3D view.
    if let Some(p) = state(|s| s.preview.clone()) {
        lib.add(p);
    }
    lib
}

/// [`library`] plus the blends (Blend Colors) the plan's paint and defaults
/// refer to by name.
pub fn library_with_blends(project: &Project) -> MaterialLibrary {
    let mut lib = library();
    let names = project
        .object_materials
        .iter()
        .flat_map(|o| o.parts.iter().map(|p| p.material.as_str()))
        .chain(
            project
                .material_defaults
                .iter()
                .map(|d| d.material.as_str()),
        );
    let mut seen = std::collections::HashSet::new();
    let blends: Vec<MaterialDef> = names
        .filter(|n| n.starts_with(plan_materials::BLEND_PREFIX) && seen.insert(*n))
        .filter_map(|n| lib.resolve(n))
        .collect();
    for b in blends {
        lib.add(b);
    }
    lib
}

/// The material the painter applies.
pub fn active_material() -> Option<String> {
    state(|s| s.active.clone())
}

pub fn set_active(name: Option<String>) {
    state(|s| s.active = name);
}

/// The palette's mode, scope and Use Default Material switch.
pub fn paint_options() -> (PaintMode, PaintScope, bool) {
    state(|s| (s.paint_mode, s.scope, s.use_default))
}

pub fn set_paint_options(mode: PaintMode, scope: PaintScope, use_default: bool) {
    state(|s| {
        s.paint_mode = mode;
        s.scope = scope;
        s.use_default = use_default;
    });
}

/// Does the Library dock list materials instead of library objects?
pub fn browse_materials() -> bool {
    state(|s| s.browse_materials)
}

pub fn set_browse_materials(on: bool) {
    state(|s| s.browse_materials = on);
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
        OBJECT_EYEDROPPER => painter_mode() == PainterMode::ObjectEyedropper,
        ADJUST_DEFINITION => painter_mode() == PainterMode::Adjust,
        USE_DEFAULT => state(|s| s.use_default),
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
    override_def_for_mesh(project, lib, object, current).map(scene_material)
}

/// The library material the project's overrides give a mesh of `object` that
/// would otherwise be drawn as `current`; `None` when nothing overrides it.
pub fn override_def_for_mesh<'a>(
    project: &Project,
    lib: &'a MaterialLibrary,
    object: Id,
    current: Material,
) -> Option<&'a MaterialDef> {
    override_def_with_class(project, lib, object, None, current)
}

/// [`override_def_for_mesh`] that also falls back to the Materials Defaults
/// of the object's `class` ("Wall", "Door", ...).
fn override_def_with_class<'a>(
    project: &Project,
    lib: &'a MaterialLibrary,
    object: Id,
    class: Option<&str>,
    current: Material,
) -> Option<&'a MaterialDef> {
    let parts = project.object_materials_of(object);
    let own_part = parts_of_mesh(current).first().copied().unwrap_or("");
    // A class default of the part itself, else the whole class's (which, like
    // a whole-object paint, leaves a window's glass clear).
    let class_default = |class: &str| -> Option<(String, bool)> {
        let part = parts_of_mesh(current)
            .iter()
            .find(|c| project.class_material(class, c).is_some())
            .copied()
            .unwrap_or(own_part);
        let exact = project
            .material_defaults
            .iter()
            .any(|c| c.class.eq_ignore_ascii_case(class) && c.part == part);
        project
            .class_material(class, part)
            .map(|m| (m.to_string(), !exact))
    };
    let chosen: Option<(String, bool)> = if parts.is_empty() {
        // No paint of its own: the class default of the part, if any.
        class_default(class?)
    } else {
        let by_part = parts_of_mesh(current)
            .iter()
            .find_map(|c| parts.iter().find(|p| p.part == *c));
        by_part
            .or_else(|| parts.iter().find(|p| p.part == WHOLE_OBJECT))
            .map(|p| (p.material.clone(), p.part == WHOLE_OBJECT))
            // A part with no paint of its own takes the class default.
            .or_else(|| class.and_then(class_default))
    };
    let (name, whole_object) = chosen?;
    let def = lib.find(&name)?;
    // The whole-object material leaves windows' glass clear unless it is
    // glass itself.
    let glass = matches!(current, Material::WindowGlass | Material::Glass);
    if glass && whole_object && !matches!(scene_material(def), Material::Glass) {
        return None;
    }
    Some(def)
}

/// The class ("Wall", "Door", ...) of every object id of `project`'s floors
/// that takes class defaults.
fn classes_of(project: &Project) -> std::collections::HashMap<Id, &'static str> {
    let mut out = std::collections::HashMap::new();
    for (fl, f) in project.floors.iter().enumerate() {
        for w in &f.walls {
            out.insert(w.id, "Wall");
        }
        for o in &f.openings {
            if let Some(k) = kind_of(project, fl, ObjectRef::Opening(o.id)) {
                out.insert(o.id, k);
            }
        }
        for c in crate::editor::placed::load_cabinets(f) {
            out.insert(c.id, "Cabinet");
        }
        for v in &f.roofs {
            if v.get("kind").and_then(|k| k.as_str()) == Some("plane") {
                if let Some(id) = v.get("id").and_then(|i| i.as_u64()) {
                    out.insert(id, "Roof");
                }
            }
        }
    }
    out
}

/// Paints the meshes of `scene` whose object has a material override (or a
/// class default): each becomes the scene material that stands for its
/// library material and takes that material's exact colour; its roughness,
/// metalness, transparency and glow are registered for the viewport and the
/// ray tracer ([`plan_3d::surface`]). Returns how many meshes changed.
pub fn apply_overrides(project: &Project, scene: &mut Scene) -> usize {
    if project.object_materials.is_empty() && project.material_defaults.is_empty() {
        return 0;
    }
    let lib = library_with_blends(project);
    let classes = if project.material_defaults.is_empty() {
        Default::default()
    } else {
        classes_of(project)
    };
    let mut n = 0;
    for mesh in &mut scene.meshes {
        let Some(id) = mesh.object_id else { continue };
        let class = classes.get(&id).copied();
        if let Some(def) = override_def_with_class(project, &lib, id, class, mesh.material) {
            let m = scene_material(def);
            let changed = m != mesh.material || mesh.color != Some(def.color);
            mesh.material = m;
            mesh.color = Some(def.color);
            register_surface(id, m, def);
            if changed {
                n += 1;
            }
        }
    }
    n
}

/// Tells the renderers how the painted material `def` responds to light.
fn register_surface(object: Id, material: Material, def: &MaterialDef) {
    let s = def.surface();
    let base = plan_materials::scene_surface(material);
    // A flat, matte, opaque, dark material needs no entry: the scene
    // material's own gloss is what the library says too.
    let plain = (s.roughness - base.roughness).abs() < 1e-3
        && (s.metallic - base.metallic).abs() < 1e-3
        && s.transparency < 1e-3
        && s.emissive < 1e-3;
    plan_3d::surface::register(
        object,
        material,
        def.color,
        (!plain).then_some(plan_3d::surface::PaintSurface {
            roughness: s.roughness,
            metallic: s.metallic,
            transparency: s.transparency,
            emissive: s.emissive,
        }),
    );
}

/// Does `def` paint with a bitmap (an image file, a generated texture) rather
/// than its flat colour?
fn has_bitmap(def: &MaterialDef) -> bool {
    def.texture_path.is_some() || !matches!(def.texture, plan_materials::Texture::Solid)
}

/// Content key of the bitmap of `def` (its name, image path and tile size).
fn bitmap_key(def: &MaterialDef) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    def.name.hash(&mut h);
    def.texture_path.hash(&mut h);
    def.texture_scale_in.0.to_bits().hash(&mut h);
    def.texture_scale_in.1.to_bits().hash(&mut h);
    def.texture_offset_in.0.to_bits().hash(&mut h);
    def.texture_offset_in.1.to_bits().hash(&mut h);
    def.texture_angle_deg.to_bits().hash(&mut h);
    def.blend_color.hash(&mut h);
    def.blend_amount.to_bits().hash(&mut h);
    format!("{:?}", def.texture).hash(&mut h);
    h.finish()
}

thread_local! {
    /// Pixels handed to the viewport, by bitmap key (one copy per bitmap).
    static BITMAPS: RefCell<std::collections::HashMap<u64, std::sync::Arc<Vec<u8>>>> =
        RefCell::new(std::collections::HashMap::new());
}

/// The bitmaps of the painted materials `scene` (already painted by
/// [`apply_overrides`]) uses: for each object with an override whose library
/// material has a bitmap, that bitmap on the object's meshes drawn as the
/// scene material standing for it. An image file is read through `store`
/// (cached); a material without a file gets its generated texture. Flat
/// materials need nothing (their colour is on the mesh).
pub fn painted_textures(
    project: &Project,
    scene: &Scene,
    store: &plan_materials::textures::TextureStore,
) -> Vec<plan_view3d::SurfaceTexture> {
    if project.object_materials.is_empty() && project.material_defaults.is_empty() {
        return Vec::new();
    }
    let lib = library_with_blends(project);
    // The paint of every object: its own, else its class's defaults.
    let mut entries: Vec<(Id, Vec<(String, String)>)> = project
        .object_materials
        .iter()
        .map(|om| {
            (
                om.object,
                om.parts
                    .iter()
                    .map(|p| (p.part.clone(), p.material.clone()))
                    .collect(),
            )
        })
        .collect();
    if !project.material_defaults.is_empty() {
        let classes = classes_of(project);
        let mut seen = std::collections::HashSet::new();
        for id in scene.meshes.iter().filter_map(|m| m.object_id) {
            if !seen.insert(id) || !project.object_materials_of(id).is_empty() {
                continue;
            }
            let Some(class) = classes.get(&id) else {
                continue;
            };
            let parts: Vec<(String, String)> = project
                .material_defaults
                .iter()
                .filter(|d| d.class.eq_ignore_ascii_case(class))
                .map(|d| (d.part.clone(), d.material.clone()))
                .collect();
            if !parts.is_empty() {
                entries.push((id, parts));
            }
        }
    }
    let mut out = Vec::new();
    for (object, parts) in &entries {
        let drawn: std::collections::HashSet<Material> = scene
            .meshes
            .iter()
            .filter(|m| m.object_id == Some(*object) && m.color.is_some())
            .map(|m| m.material)
            .collect();
        if drawn.is_empty() {
            continue;
        }
        // Part overrides first: they win over the whole-object material on
        // the meshes they cover.
        let mut parts: Vec<_> = parts.iter().collect();
        parts.sort_by_key(|(part, _)| part == WHOLE_OBJECT);
        for (_, name) in parts {
            let Some(def) = lib.find(name).filter(|d| has_bitmap(d)) else {
                continue;
            };
            let material = scene_material(def);
            if !drawn.contains(&material) {
                continue;
            }
            let image = store.definition(def);
            let key = bitmap_key(def);
            let rgba = BITMAPS.with(|b| {
                std::sync::Arc::clone(b.borrow_mut().entry(key).or_insert_with(|| {
                    std::sync::Arc::new(plan_materials::transform_rgba(
                        &image.image.rgba,
                        image.image.width,
                        image.image.height,
                        image.scale_in,
                        def,
                    ))
                }))
            });
            out.push(plan_view3d::SurfaceTexture {
                object_id: *object,
                material: Some(material),
                key,
                width: image.image.width,
                height: image.image.height,
                rgba,
                scale_in: image.scale_in,
            });
        }
    }
    out
}

// ----- painting -----

/// A click of the painter on `obj` (see [`paint_click`], which the pick hook
/// of the 3D view calls with the recorded ray). Returns whether the plan
/// changed.
pub fn paint_object(cx: &mut EditorContext, floor: usize, obj: ObjectRef) -> bool {
    paint_click(cx, Some((floor, obj)))
}

/// The material a placed symbol is painted with (its whole-object paint), for
/// the Symbol Specification's Materials tab.
pub fn symbol_material(project: &Project, id: Id) -> Option<String> {
    project
        .object_materials_of(id)
        .iter()
        .find(|p| p.part == WHOLE_OBJECT)
        .map(|p| p.material.clone())
}

/// Applies the Materials tab's choice to symbol `id` (`None` puts the symbol's
/// own look back). Does not open an undo step: the Symbol Specification's OK
/// already has. True when the plan changed.
pub fn apply_symbol_material(project: &mut Project, id: Id, choice: Option<&str>) -> bool {
    match choice {
        Some(name) => {
            let had = project.object_materials_of(id).len() > 1;
            if had {
                project.clear_object_material(id, None);
            }
            project.set_object_material(id, WHOLE_OBJECT, name) || had
        }
        None => project.clear_object_material(id, None),
    }
}

// ----- plan and elevation hatching -----

/// The library material a hatch for `name` uses: the material called `name`
/// (the user's copy first, blends included), else the first one whose name or
/// category contains it.
pub fn hatch_material(project: &Project, name: &str) -> Option<MaterialDef> {
    let lib = library_with_blends(project);
    lib.resolve(name)
        .or_else(|| lib.search(name).into_iter().next().cloned())
}

/// The hatch of material `name` over `polygon` (inches), clipped to it: its
/// CAD pattern at the Pattern tab's scale and angle, for a drawing at `paper`
/// paper inches per foot. Empty for a material with no pattern or an
/// unknown name. Plan regions, wall hatching and elevations use this so a
/// material's pattern is drawn the same everywhere.
pub fn hatch_strokes(
    project: &Project,
    name: &str,
    polygon: &[plan_core::Point],
    paper: f64,
) -> Vec<(plan_core::Point, plan_core::Point)> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let Some(def) = hatch_material(project, name) else {
        return Vec::new();
    };
    let fold = |(lo, hi): (plan_core::Point, plan_core::Point), p: &plan_core::Point| {
        (
            plan_core::Point::new(lo.x.min(p.x), lo.y.min(p.y)),
            plan_core::Point::new(hi.x.max(p.x), hi.y.max(p.y)),
        )
    };
    let first = polygon[0];
    let rect = polygon.iter().fold((first, first), fold);
    plan_materials::clip_strokes_to_polygon(&def.hatch_strokes(rect, paper), polygon)
}

// ----- commands -----

/// Opens the Material Specification on a copy of `def`.
pub fn open_spec_for(def: &MaterialDef) {
    state(|s| s.spec = Some(spec::MaterialSpec::from_def(def)));
}

/// Opens the Material Specification on a new material.
pub fn open_new_spec() {
    state(|s| s.spec = Some(spec::MaterialSpec::new()));
}

/// Is the Material Specification open?
pub fn spec_open() -> bool {
    state(|s| s.spec.is_some())
}

/// The name in the open Material Specification.
pub fn spec_name() -> Option<String> {
    state(|s| s.spec.as_ref().map(|sp| sp.name().to_string()))
}

/// Closes the Material Specification without saving; the 3D view goes back to
/// the saved material.
pub fn close_spec() {
    state(|s| {
        s.spec = None;
        if s.preview.take().is_some() {
            s.revision += 1;
        }
    });
}

/// Shows `def` live in the 3D view in place of the library material of its
/// name (`None` goes back to the saved one).
fn set_preview(def: Option<MaterialDef>) {
    state(|s| {
        if s.preview != def {
            s.preview = def;
            s.revision += 1;
        }
    });
}

/// The Material Specification's own edit of `def` as the painter would see
/// it (tests and the palette).
pub fn preview_name() -> Option<String> {
    state(|s| s.preview.as_ref().map(|d| d.name.clone()))
}

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
            if mode == PainterMode::Eyedropper {
                state(|s| s.object_set = None);
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
            "Material Eyedropper: click a surface in the 3D view",
        ),
        OBJECT_EYEDROPPER => toggle(
            PainterMode::ObjectEyedropper,
            cx,
            "Object Eyedropper: click a painted object to copy all its materials",
        ),
        ADJUST_DEFINITION => toggle(
            PainterMode::Adjust,
            cx,
            "Adjust Material Definition: click a surface in the 3D view",
        ),
        ERASE => toggle(
            PainterMode::Erase,
            cx,
            "Delete Surface: click a painted surface in the 3D view",
        ),
        USE_DEFAULT => {
            let on = !state(|s| s.use_default);
            state(|s| s.use_default = on);
            if on {
                set_painter_mode(PainterMode::Paint);
                state(|s| s.list_open = true);
            }
            cx.status = if on {
                "Use Default Material: click surfaces to put their default material back".into()
            } else {
                "Material Painter paints with the active material".into()
            };
        }
        LIST => state(|s| s.list_open = true),
        ADJUST => state(|s| s.adjust_open = true),
        DEFAULTS => state(|s| s.defaults_open = true),
        BUILDER => {
            let lib = library();
            match active_material().and_then(|n| lib.resolve(&n)) {
                Some(d) => open_spec_for(&d),
                None => open_new_spec(),
            }
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

/// Draws the Material Painter palette, Adjust Materials, Material
/// Specification and Materials Defaults windows.
pub fn show_windows(ctx: &egui::Context, cx: &mut EditorContext) {
    if state(|s| s.list_open) {
        palette_window(ctx);
    }
    if state(|s| s.adjust_open) {
        adjust_window(ctx, cx);
    }
    if state(|s| s.spec.is_some()) {
        spec_window(ctx);
    }
    if state(|s| s.defaults_open) {
        defaults::window(ctx, cx);
    }
}

/// The Material Painter palette: modes, scope, Use Default Material, the
/// tools and the material list.
fn palette_window(ctx: &egui::Context) {
    let mut open = true;
    egui::Window::new("Material Painter")
        .id(egui::Id::new("materials_painter_palette"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([340.0, 480.0])
        .pivot(Align2::LEFT_TOP)
        .default_pos(ctx.screen_rect().left_top() + egui::vec2(24.0, 110.0))
        .show(ctx, |ui| {
            let lib = library();
            let (mut mode, mut scope, mut use_default, mut pct) =
                state(|s| (s.paint_mode, s.scope, s.use_default, s.blend_pct));
            ui.horizontal_wrapped(|ui| {
                for m in PaintMode::ALL {
                    if ui.selectable_label(mode == m, m.label()).clicked() {
                        mode = m;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Scope");
                egui::ComboBox::from_id_salt("painter_scope")
                    .selected_text(scope.label())
                    .show_ui(ui, |ui| {
                        for sc in PaintScope::ALL {
                            ui.selectable_value(&mut scope, sc, sc.label());
                        }
                    });
                ui.add_enabled_ui(mode.is_wide(), |ui| {
                    ui.weak(if mode.is_wide() {
                        "narrows the reach"
                    } else {
                        "(Room, Floor, Plan)"
                    });
                });
            });
            if mode == PaintMode::BlendColors {
                ui.horizontal(|ui| {
                    ui.label("Blend");
                    let mut p = i32::from(pct);
                    ui.add(egui::Slider::new(&mut p, 1..=99).suffix("%"));
                    pct = p as u8;
                });
            }
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(use_default, "Use Default Material")
                    .on_hover_text("Clicks put the default material back instead of painting")
                    .clicked()
                {
                    use_default = !use_default;
                    if use_default {
                        set_painter_mode(PainterMode::Paint);
                    }
                }
            });
            state(|s| {
                s.paint_mode = mode;
                s.scope = scope;
                s.use_default = use_default;
                s.blend_pct = pct;
            });
            let active = active_material();
            ui.horizontal(|ui| {
                ui.label("Active:");
                match active.as_deref().and_then(|n| lib.resolve(n)) {
                    Some(d) => {
                        swatch(ui, d.color);
                        ui.strong(&d.name);
                    }
                    None => {
                        ui.weak("none");
                    }
                }
            });
            if let Some(set) = state(|s| s.object_set.clone()) {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Material set: {} ({} parts)",
                        set.kind,
                        set.parts.len()
                    ));
                    if ui.small_button("Clear").clicked() {
                        state(|s| s.object_set = None);
                    }
                });
            }
            ui.horizontal_wrapped(|ui| {
                let m = painter_mode();
                for (t, label) in [
                    (PainterMode::Paint, "Paint"),
                    (PainterMode::Eyedropper, "Eyedropper"),
                    (PainterMode::ObjectEyedropper, "Object Eyedropper"),
                    (PainterMode::Adjust, "Adjust Definition"),
                    (PainterMode::Erase, "Delete Surface"),
                ] {
                    if ui.selectable_label(m == t, label).clicked() {
                        set_painter_mode(if m == t { PainterMode::Off } else { t });
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("New...").clicked() {
                    open_new_spec();
                }
                if ui
                    .add_enabled(active.is_some(), egui::Button::new("Edit..."))
                    .clicked()
                {
                    if let Some(d) = active.as_deref().and_then(|n| lib.resolve(n)) {
                        open_spec_for(&d);
                    }
                }
            });
            ui.separator();
            browser::list(ui, &lib, "painter");
        });
    if !open {
        state(|s| s.list_open = false);
    }
}

/// The Material Specification window.
fn spec_window(ctx: &egui::Context) {
    let Some(mut sp) = state(|s| s.spec.take()) else {
        return;
    };
    let outcome = sp.show(ctx);
    finish_spec(sp, outcome);
}

/// What OK, Revert to Library, Cancel and a frame without a click do.
fn finish_spec(mut sp: spec::MaterialSpec, outcome: spec::Outcome) {
    match outcome {
        spec::Outcome::Keep => {
            set_preview(sp.live_def());
            state(|s| s.spec = Some(sp));
        }
        spec::Outcome::Save => match sp.def().and_then(|d| save_to_user_library(&d)) {
            Ok(name) => {
                set_active(Some(name));
                set_preview(None);
            }
            Err(e) => {
                sp.note = e;
                state(|s| s.spec = Some(sp));
            }
        },
        spec::Outcome::Revert => {
            let name = sp.original_name().to_string();
            match remove_from_user_library(&name) {
                Ok(()) => set_preview(None),
                Err(e) => {
                    sp.note = e;
                    state(|s| s.spec = Some(sp));
                }
            }
        }
        spec::Outcome::Cancel => set_preview(None),
    }
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

/// Adds (or replaces) `def` in the user library and saves the file; returns
/// the material's name.
pub fn save_to_user_library(def: &MaterialDef) -> Result<String, String> {
    let mut lib = user_library();
    lib.add(def.clone());
    if let Some(p) = user_library_path() {
        save_user_library_at(&p, &lib).map_err(|e| format!("Could not save: {e}"))?;
    }
    state(|s| {
        s.user = Some(lib);
        s.revision += 1;
    });
    Ok(def.name.clone())
}

/// Replaces the user library for this thread without touching any file
/// (tests).
#[cfg(test)]
pub fn set_user_library_for_test(lib: MaterialLibrary) {
    state(|s| {
        s.user = Some(lib);
        s.revision += 1;
    });
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
        s.revision += 1;
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
    use plan_materials::build_material;

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
            color: None,
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
    fn painted_meshes_take_the_exact_colour_of_the_library_material() {
        let (mut cx, wall) = cx_with_wall();
        let bone = library().find("Color – Bone").unwrap().color;
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Color – Bone");
        let mut scene = scene_with(wall, Material::WallInterior);
        scene.meshes.extend(scene_with(999, Material::Roof).meshes);
        assert_eq!(apply_overrides(&cx.project, &mut scene), 1);
        assert_eq!(scene.meshes[0].color, Some(bone));
        assert_eq!(scene.meshes[1].color, None, "other objects keep their own");
        // Painting again changes nothing; a plain colour needs no bitmap.
        assert_eq!(apply_overrides(&cx.project, &mut scene), 0);
        let store = plan_materials::textures::TextureStore::with_dirs(Vec::new());
        assert!(painted_textures(&cx.project, &scene, &store).is_empty());
    }

    #[test]
    fn a_material_with_an_image_file_binds_its_bitmap_to_the_painted_meshes() {
        let dir = std::env::temp_dir().join(format!("plan-studio-paint-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("red.png");
        let img = plan_render::Image {
            width: 2,
            height: 2,
            rgba: [200, 10, 10, 255].repeat(4),
            hdr: vec![[0.0; 3]; 4],
        };
        plan_render::write_png(&png, &img).unwrap();
        let def = build_material(
            "Test Red Image",
            "Custom",
            [200, 10, 10],
            0.5,
            Pattern::None,
            &png.to_string_lossy(),
        )
        .unwrap();
        let mut user = MaterialLibrary::default();
        user.add(def.clone());
        set_user_library_for_test(user);
        let (mut cx, wall) = cx_with_wall();
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Test Red Image");
        let mut scene = scene_with(wall, Material::WallInterior);
        apply_overrides(&cx.project, &mut scene);
        assert_eq!(scene.meshes[0].color, Some([200, 10, 10]));
        let store = plan_materials::textures::TextureStore::with_dirs(Vec::new());
        let tex = painted_textures(&cx.project, &scene, &store);
        assert_eq!(tex.len(), 1);
        let t = &tex[0];
        assert_eq!(t.object_id, wall);
        assert_eq!(t.material, Some(scene_material(&def)));
        assert!(t.is_valid());
        assert_eq!(t.rgba[..4], [200, 10, 10, 255], "the file's pixels");
        // The same bitmap is shared, not copied, for the next call.
        let again = painted_textures(&cx.project, &scene, &store);
        assert!(std::sync::Arc::ptr_eq(&t.rgba, &again[0].rgba));
        set_user_library_for_test(MaterialLibrary::default());
        let _ = std::fs::remove_dir_all(&dir);
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
    fn the_specification_saves_to_the_user_library_file() {
        let path = scratch("materials.json");
        let _ = std::fs::remove_file(&path);
        assert!(load_user_library_at(&path).materials.is_empty());
        let mut sp = spec::MaterialSpec::new();
        sp.set_pattern("Brick");
        {
            let d = sp.draft_def_mut();
            d.name = "Barn Red".into();
            d.color = [160, 75, 55];
            d.texture_path = Some("/tmp/red.png".into());
            d.set_class(plan_materials::MaterialClass::Plastic);
            d.manufacturer = "Acme".into();
            d.price = 3.25;
            d.pattern_scale = 2.0;
        }
        let def = sp.def().unwrap();
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
        assert_eq!(
            back.materials[0].class,
            plan_materials::MaterialClass::Plastic
        );
        assert_eq!(back.materials[0].pattern_scale, 2.0);
        // A nameless draft is refused.
        sp.draft_def_mut().name.clear();
        assert!(sp.def().is_err());
        let merged = core_library().merged_with(&back);
        assert!(merged.find("Barn Red").is_some() && merged.find("Drywall").is_some());
        // Every builder pattern name maps back to itself.
        for p in PATTERNS {
            assert_eq!(pattern_name(&pattern_by_name(p)), p);
        }
    }

    #[test]
    fn the_specification_previews_live_and_cancel_puts_the_library_back() {
        let (mut cx, wall) = cx_with_wall();
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Color – Bone");
        let original = library().find("Color – Bone").unwrap().clone();
        let rev = library_revision();
        let mut edited = original.clone();
        edited.color = [10, 20, 30];
        open_spec_for(&edited);
        assert!(spec_open() && spec_name().as_deref() == Some("Color – Bone"));
        // A frame of the dialog puts the draft in the library, live.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &mut cx));
        assert_eq!(library().find("Color – Bone").unwrap().color, [10, 20, 30]);
        assert!(library_revision() > rev, "the 3D view rebuilds");
        let mut scene = scene_with(wall, Material::WallInterior);
        apply_overrides(&cx.project, &mut scene);
        assert_eq!(scene.meshes[0].color, Some([10, 20, 30]));
        // Closing without saving restores the saved material.
        close_spec();
        assert!(!spec_open() && preview_name().is_none());
        assert_eq!(library().find("Color – Bone"), Some(&original));
        let mut scene = scene_with(wall, Material::WallInterior);
        apply_overrides(&cx.project, &mut scene);
        assert_eq!(scene.meshes[0].color, Some(original.color));
    }

    #[test]
    fn ok_saves_the_specification_to_my_materials_and_revert_goes_back_to_the_core_one() {
        let path = scratch("ok-materials.json");
        let _ = std::fs::remove_file(&path);
        set_user_path_for_test(Some(path.clone()));
        set_user_library_for_test(MaterialLibrary::default());
        set_active(None);
        let core = core_library().find("Drywall").unwrap().clone();
        let mut edited = core.clone();
        edited.color = [1, 2, 3];
        edited.manufacturer = "Acme".into();
        edited.price = 0.75;
        // A nameless draft stays open with a note instead of saving.
        let mut nameless = spec::MaterialSpec::new();
        nameless.draft_def_mut().color = [9, 9, 9];
        finish_spec(nameless, spec::Outcome::Save);
        assert!(spec_open());
        assert!(state(|s| s.spec.as_ref().map(|sp| sp.note.clone()))
            .unwrap()
            .contains("name"));
        close_spec();
        // OK on an edited core material: my copy takes its place.
        finish_spec(spec::MaterialSpec::from_def(&edited), spec::Outcome::Save);
        assert!(!spec_open());
        assert_eq!(active_material().as_deref(), Some("Drywall"));
        assert_eq!(library().find("Drywall").unwrap().color, [1, 2, 3]);
        assert_eq!(library().find("Drywall").unwrap().manufacturer, "Acme");
        let on_disk = load_user_library_at(&path);
        assert_eq!(on_disk.find("Drywall").unwrap().price, 0.75);
        // Revert to Library deletes my copy; the core material is back.
        finish_spec(spec::MaterialSpec::from_def(&edited), spec::Outcome::Revert);
        assert_eq!(library().find("Drywall"), Some(&core));
        assert!(load_user_library_at(&path).materials.is_empty());
        set_user_path_for_test(None);
        set_user_library_for_test(MaterialLibrary::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn class_defaults_paint_unpainted_objects_and_the_wall_paint_wins() {
        let (mut cx, wall) = cx_with_wall();
        let other = cx.project.add_wall(
            0,
            Point::new(0.0, 100.0),
            Point::new(120.0, 100.0),
            6.0,
            96.0,
            WallKind::Exterior,
        );
        cx.project
            .set_class_material("Wall", "Interior Wall Surface", "Color – Bone");
        let mut scene = scene_with(wall, Material::WallInterior);
        scene
            .meshes
            .extend(scene_with(other, Material::WallInterior).meshes);
        scene
            .meshes
            .extend(scene_with(other, Material::WallExterior).meshes);
        assert_eq!(apply_overrides(&cx.project, &mut scene), 2);
        let bone = library().find("Color – Bone").unwrap().color;
        assert_eq!(scene.meshes[0].color, Some(bone));
        assert_eq!(scene.meshes[1].color, Some(bone));
        assert_eq!(
            scene.meshes[2].color, None,
            "the exterior face has no default"
        );
        // The wall's own paint wins over the default.
        cx.project
            .set_object_material(wall, "Interior Wall Surface", "Drywall");
        let mut scene = scene_with(wall, Material::WallInterior);
        apply_overrides(&cx.project, &mut scene);
        assert_eq!(
            scene.meshes[0].color,
            library().find("Drywall").map(|d| d.color)
        );
        // A whole-class default leaves a window's glass clear.
        let win = cx
            .project
            .add_opening(0, wall, 60.0, OpeningKind::Window)
            .unwrap();
        cx.project.set_class_material("Window", "", "Brick – Red");
        let mut scene = scene_with(win, Material::WindowFrame);
        scene
            .meshes
            .extend(scene_with(win, Material::WindowGlass).meshes);
        apply_overrides(&cx.project, &mut scene);
        assert!(scene.meshes[0].color.is_some());
        assert_eq!(scene.meshes[1].material, Material::WindowGlass);
        assert_eq!(scene.meshes[1].color, None);
    }

    #[test]
    fn a_painted_materials_class_reaches_the_renderers_through_the_surface_table() {
        let (mut cx, wall) = cx_with_wall();
        let mut mirror = MaterialDef::new("Test Mirror R14", &["Custom"], [201, 202, 203]);
        mirror.set_class(plan_materials::MaterialClass::Mirror);
        let mut lamp = MaterialDef::new("Test Lamp R14", &["Custom"], [211, 212, 213]);
        lamp.set_class(plan_materials::MaterialClass::Emissive);
        let mut user = MaterialLibrary::default();
        user.add(mirror.clone());
        user.add(lamp.clone());
        set_user_library_for_test(user);
        cx.project
            .set_object_material(wall, "Interior Wall Surface", "Test Mirror R14");
        cx.project
            .set_object_material(wall, "Exterior Wall Surface", "Test Lamp R14");
        let mut scene = scene_with(wall, Material::WallInterior);
        scene
            .meshes
            .extend(scene_with(wall, Material::WallExterior).meshes);
        apply_overrides(&cx.project, &mut scene);
        let m = scene.meshes[0].paint_surface().expect("mirror registered");
        assert_eq!((m.metallic, m.transparency, m.emissive), (1.0, 0.0, 0.0));
        assert!(m.roughness <= 0.05);
        let l = scene.meshes[1].paint_surface().expect("lamp registered");
        assert!(l.emissive >= 0.3 && l.metallic == 0.0);
        // A plain matte material registers nothing.
        let mut plain = scene_with(wall, Material::WallInterior);
        cx.project.clear_object_material(wall, None);
        cx.project
            .set_object_material(wall, WHOLE_OBJECT, "Drywall");
        apply_overrides(&cx.project, &mut plain);
        let drywall = library().find("Drywall").unwrap().clone();
        if plain.meshes[0].color == Some(drywall.color) {
            let s = plan_materials::scene_surface(plain.meshes[0].material);
            let want = (drywall.surface().roughness - s.roughness).abs() < 1e-3;
            assert_eq!(plain.meshes[0].paint_surface().is_none(), want);
        }
        set_user_library_for_test(MaterialLibrary::default());
    }

    #[test]
    fn a_symbols_materials_tab_paints_and_unpaints_the_whole_object() {
        let mut p = Project::new("t");
        assert_eq!(symbol_material(&p, 5), None);
        assert!(apply_symbol_material(&mut p, 5, Some("Quartz – White")));
        assert!(!apply_symbol_material(&mut p, 5, Some("Quartz – White")));
        assert_eq!(symbol_material(&p, 5).as_deref(), Some("Quartz – White"));
        assert!(apply_symbol_material(&mut p, 5, Some("Drywall")));
        assert_eq!(symbol_material(&p, 5).as_deref(), Some("Drywall"));
        assert!(apply_symbol_material(&mut p, 5, None));
        assert!(!apply_symbol_material(&mut p, 5, None));
        assert!(p.object_materials.is_empty());
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
        assert!(spec_open());
        close_spec();
        assert!(run_command(&mut cx, DEFAULTS) && state(|s| s.defaults_open));
        assert!(run_command(&mut cx, OBJECT_EYEDROPPER) && is_mode_active(OBJECT_EYEDROPPER));
        assert!(run_command(&mut cx, ADJUST_DEFINITION) && is_mode_active(ADJUST_DEFINITION));
        assert!(run_command(&mut cx, USE_DEFAULT) && is_mode_active(USE_DEFAULT));
        assert!(run_command(&mut cx, USE_DEFAULT) && !is_mode_active(USE_DEFAULT));
        set_painter_mode(PainterMode::Off);
        assert!(!run_command(&mut cx, "materials.nope"));
        // The windows draw headlessly.
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, &mut cx));
    }
}
