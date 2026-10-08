//! The Image and Distributed Objects flyouts and the 3D Solid Feature tool.
//!
//! * **Create Image** / **Create Billboard Image**: the first click opens a
//!   file picker (PNG or JPEG) and places the picture at the click; later
//!   clicks place the same picture again until Esc. A picture is a
//!   [`PlacedSymbol`] with an [`ImageSpec`] (`plan_core::images`); PNG pictures
//!   are decoded with the Library Browser's decoder and drawn as a textured
//!   quad, JPEG pictures as a placeholder frame with a note (no JPEG decoder
//!   is built in). The 3D view gets a flat-coloured quad
//!   (`placed::image_meshes`), billboards turn to face the camera.
//! * **Create Image Library**: saves the picture under the click (or one
//!   picked from a file) in the user library as a reusable item, and makes
//!   it the active library item. Items are kept in
//!   `~/.plan-studio/user-library.json`.
//! * **Polyline / Spline Distribution Path / Region**: click the points,
//!   Enter or double-click to finish. Places copies of the active library
//!   item along the path or filling the region; the [`Distribution`] record
//!   regenerates them when its specification changes.
//! * **3D Solid Feature**: places the active library item as a solid
//!   (`PlacedSymbol::solid`, which only changes how the 3D view draws it).

use super::library::{active_item, find_item, placement_for, set_active_item};
use super::{KeyEvent, PointerEvent, Tool, ToolId, ToolResult};
use crate::editor::placed::hit_symbol;
use crate::editor::{Camera, EditorContext, ObjectRef};
use crate::shell::library_browser::png;
use eframe::egui::{self, Color32, Key, Pos2, Shape, Stroke, TextureHandle, TextureId};
use plan_core::geometry::Point;
use plan_core::images::{
    DistKind, Distribution, ImageFormat, ImageSpec, BILLBOARD_PLAN_DEPTH, DEFAULT_BILLBOARD_HEIGHT,
    JPEG_NOTE,
};
use plan_core::{Id, PlacedSymbol};
use plan_library::{user, CatalogItem};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// The flyout entries this tool serves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImageMode {
    CreateImage,
    BillboardImage,
    ImageLibrary,
    SolidFeature,
    PolylinePath,
    PolylineRegion,
    SplinePath,
    SplineRegion,
}

impl ImageMode {
    pub const ALL: [ImageMode; 8] = [
        ImageMode::CreateImage,
        ImageMode::BillboardImage,
        ImageMode::ImageLibrary,
        ImageMode::SolidFeature,
        ImageMode::PolylinePath,
        ImageMode::PolylineRegion,
        ImageMode::SplinePath,
        ImageMode::SplineRegion,
    ];

    /// Chief's name for the tool (the flyout entry).
    pub fn name(self) -> &'static str {
        match self {
            ImageMode::CreateImage => "Create Image",
            ImageMode::BillboardImage => "Create Billboard Image",
            ImageMode::ImageLibrary => "Create Image Library",
            ImageMode::SolidFeature => "3D Solid Feature",
            ImageMode::PolylinePath => "Polyline Distribution Path",
            ImageMode::PolylineRegion => "Polyline Distribution Region",
            ImageMode::SplinePath => "Spline Distribution Path",
            ImageMode::SplineRegion => "Spline Distribution Region",
        }
    }

    /// Kind and spline flag for the distribution modes.
    pub fn distribution(self) -> Option<(DistKind, bool)> {
        match self {
            ImageMode::PolylinePath => Some((DistKind::Path, false)),
            ImageMode::PolylineRegion => Some((DistKind::Region, false)),
            ImageMode::SplinePath => Some((DistKind::Path, true)),
            ImageMode::SplineRegion => Some((DistKind::Region, true)),
            _ => None,
        }
    }
}

// ----- picture files -----

thread_local! {
    static NEXT_PICK: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
}

/// Makes the next [`pick_image_file`] answer `path` instead of opening the
/// native dialog (tests, scripted runs).
pub fn inject_pick(path: Option<&str>) {
    NEXT_PICK.with(|p| *p.borrow_mut() = Some(path.map(str::to_string)));
}

/// The native file dialog for a PNG or JPEG picture.
pub fn pick_image_file() -> Option<String> {
    if let Some(v) = NEXT_PICK.with(|p| p.borrow_mut().take()) {
        return v;
    }
    if cfg!(test) {
        return None;
    }
    rfd::FileDialog::new()
        .add_filter("Pictures", &["png", "jpg", "jpeg"])
        .pick_file()
        .map(|p| p.to_string_lossy().into_owned())
}

/// The mean colour of the visible pixels.
fn average_color(img: &png::Rgba) -> [u8; 3] {
    let (mut acc, mut n) = ([0u64; 3], 0u64);
    for px in img.pixels.as_chunks::<4>().0 {
        let a = u64::from(px[3]);
        for (sum, &c) in acc.iter_mut().zip(&px[..3]) {
            *sum += u64::from(c) * a;
        }
        n += a;
    }
    if n == 0 {
        return [180, 180, 180];
    }
    [(acc[0] / n) as u8, (acc[1] / n) as u8, (acc[2] / n) as u8]
}

/// Reads the header (and, for PNG, the pixels' average colour) of the
/// picture at `path`.
pub fn load_spec(path: &str) -> Result<ImageSpec, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Cannot read {path}: {e}"))?;
    let (w, h, format) = plan_core::images::image_size(&bytes)
        .ok_or_else(|| "Not a PNG or JPEG picture".to_string())?;
    let mut spec = ImageSpec::new(path, w, h);
    spec.format = format;
    spec.note.clear();
    match format {
        ImageFormat::Png => match png::decode(&bytes) {
            Ok(img) => spec.color = average_color(&img),
            Err(e) => spec.note = format!("PNG could not be decoded ({e}); drawn as a frame"),
        },
        ImageFormat::Jpeg => spec.note = JPEG_NOTE.to_string(),
        ImageFormat::Other => {}
    }
    Ok(spec)
}

/// Pixels of the picture with the transparency key applied, at most
/// `max_side` on the long side. `None` for formats without a decoder.
fn decode_pixels(spec: &ImageSpec, max_side: usize) -> Option<png::Rgba> {
    if spec.format != ImageFormat::Png {
        return None;
    }
    let bytes = std::fs::read(&spec.path).ok()?;
    let mut img = png::decode(&bytes).ok()?.downscaled(max_side);
    if let Some(key) = spec.transparency {
        let tol = i32::from(spec.tolerance);
        for px in img.pixels.as_chunks_mut::<4>().0 {
            if (0..3).all(|i| (i32::from(px[i]) - i32::from(key[i])).abs() <= tol) {
                px[3] = 0;
            }
        }
    }
    Some(img)
}

// ----- textures -----

thread_local! {
    static TEXTURES: RefCell<HashMap<String, Option<TextureHandle>>> = RefCell::new(HashMap::new());
}

/// The egui texture of a picture (loaded once per path and key colour);
/// `None` when it cannot be decoded.
pub fn texture(ctx: &egui::Context, spec: &ImageSpec) -> Option<TextureId> {
    let key = format!("{}|{:?}|{}", spec.path, spec.transparency, spec.tolerance);
    TEXTURES.with(|t| {
        let mut t = t.borrow_mut();
        let entry = t.entry(key.clone()).or_insert_with(|| {
            decode_pixels(spec, 1024).map(|img| {
                ctx.load_texture(key, img.to_color_image(), egui::TextureOptions::LINEAR)
            })
        });
        entry.as_ref().map(TextureHandle::id)
    })
}

/// Draws a picture symbol in the plan: a textured quad, or a framed
/// placeholder when there is no texture (JPEG, missing file).
pub fn draw_image(painter: &egui::Painter, cam: &Camera, s: &PlacedSymbol, ink: Color32) {
    let Some(spec) = &s.image else { return };
    let foot = s.footprint();
    let pts: Vec<Pos2> = foot.iter().map(|p| cam.world_to_screen(*p)).collect();
    let tex = if spec.billboard {
        None
    } else {
        texture(painter.ctx(), spec)
    };
    match tex {
        Some(id) => {
            // Footprint order is back-left, back-right, front-right,
            // front-left; the picture's top edge is the front.
            let (u0, u1) = if s.flip { (1.0, 0.0) } else { (0.0, 1.0) };
            let uv = [[u0, 1.0], [u1, 1.0], [u1, 0.0], [u0, 0.0]];
            let mut mesh = egui::Mesh::with_texture(id);
            for (p, uv) in pts.iter().zip(uv) {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: *p,
                    uv: Pos2::new(uv[0], uv[1]),
                    color: Color32::WHITE,
                });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            painter.add(Shape::mesh(mesh));
            painter.add(Shape::closed_line(
                pts,
                Stroke::new(0.8_f32, ink.gamma_multiply(0.5)),
            ));
        }
        None => {
            let [r, g, b] = spec.color;
            painter.add(Shape::convex_polygon(
                pts.clone(),
                Color32::from_rgba_unmultiplied(r, g, b, 90),
                Stroke::new(1.2_f32, ink),
            ));
            if !spec.billboard {
                painter.add(Shape::line(vec![pts[0], pts[2]], Stroke::new(0.6_f32, ink)));
                painter.add(Shape::line(vec![pts[1], pts[3]], Stroke::new(0.6_f32, ink)));
            }
        }
    }
}

/// Draws a distribution record: its path (open) or outline (closed), dashed.
pub fn draw_distribution(painter: &egui::Painter, cam: &Camera, s: &PlacedSymbol, ink: Color32) {
    let Some(d) = &s.distribution else { return };
    let mut pts: Vec<Pos2> = d
        .polyline()
        .iter()
        .map(|p| cam.world_to_screen(*p))
        .collect();
    if d.kind == DistKind::Region {
        if let Some(first) = pts.first().copied() {
            pts.push(first);
        }
    }
    painter.extend(Shape::dashed_line(
        &pts,
        Stroke::new(1.0_f32, ink.gamma_multiply(0.7)),
        8.0,
        5.0,
    ));
}

// ----- the user library -----

thread_local! {
    static USER_ITEMS: RefCell<Vec<Arc<CatalogItem>>> = const { RefCell::new(Vec::new()) };
    static USER_LOADED: RefCell<bool> = const { RefCell::new(false) };
    static USER_PATH: RefCell<Option<Option<PathBuf>>> = const { RefCell::new(None) };
}

/// Where the user library is saved; `None` disables saving. Tests default
/// to no file, the app to `~/.plan-studio/user-library.json`.
pub fn user_library_path() -> Option<PathBuf> {
    if let Some(p) = USER_PATH.with(|p| p.borrow().clone()) {
        return p;
    }
    if cfg!(test) {
        None
    } else {
        crate::paths::user_file("user-library.json")
    }
}

/// Overrides the user library file (`Some(None)` turns saving off).
pub fn set_user_library_path(path: Option<Option<PathBuf>>) {
    USER_PATH.with(|p| *p.borrow_mut() = path);
    USER_LOADED.with(|l| *l.borrow_mut() = false);
    USER_ITEMS.with(|u| u.borrow_mut().clear());
}

fn ensure_user_loaded() {
    if USER_LOADED.with(|l| std::mem::replace(&mut *l.borrow_mut(), true)) {
        return;
    }
    let Some(path) = user_library_path() else {
        return;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return;
    };
    if let Ok(cat) = plan_library::Catalog::from_json(&text) {
        USER_ITEMS.with(|u| u.borrow_mut().extend(cat.items.into_iter().map(Arc::new)));
    }
}

/// The user-library item with this id.
pub fn user_item(id: &str) -> Option<Arc<CatalogItem>> {
    if !id.starts_with("user.") {
        return None;
    }
    ensure_user_loaded();
    USER_ITEMS.with(|u| u.borrow().iter().find(|i| i.id == id).cloned())
}

/// The user library's items.
pub fn user_items() -> Vec<Arc<CatalogItem>> {
    ensure_user_loaded();
    USER_ITEMS.with(|u| u.borrow().clone())
}

/// Adds (or replaces) an item and saves the library file.
pub fn register_user_item(item: CatalogItem) -> Result<Arc<CatalogItem>, String> {
    ensure_user_loaded();
    let arc = Arc::new(item);
    USER_ITEMS.with(|u| {
        let mut u = u.borrow_mut();
        u.retain(|i| i.id != arc.id);
        u.push(arc.clone());
    });
    save_user_library()?;
    Ok(arc)
}

fn save_user_library() -> Result<(), String> {
    let Some(path) = user_library_path() else {
        return Ok(());
    };
    let items: Vec<CatalogItem> =
        USER_ITEMS.with(|u| u.borrow().iter().map(|i| (**i).clone()).collect());
    let json = user::user_catalog(&items)
        .to_json()
        .map_err(|e| e.to_string())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, json).map_err(|e| format!("Cannot save {}: {e}", path.display()))
}

/// The picture a library item places, if it is a picture item.
pub fn spec_from_item(item: &CatalogItem) -> Option<ImageSpec> {
    let path = user::image_path(item)?;
    Some(load_spec(path).unwrap_or_else(|_| {
        let mut s = ImageSpec::new(path, 0, 0);
        s.note = "Picture file not found; drawn as a frame".into();
        s
    }))
}

fn stable_hash(text: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in text.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Create Image Library: saves `spec` as a user-library item sized
/// `width` x `depth` and makes it the active library item. Returns its id.
pub fn save_to_library(
    cx: &mut EditorContext,
    spec: &ImageSpec,
    width: f64,
    depth: f64,
) -> Result<String, String> {
    let stem = std::path::Path::new(&spec.path).file_stem().map_or_else(
        || "Picture".to_string(),
        |s| s.to_string_lossy().into_owned(),
    );
    let id = format!("user.image.{:016x}", stable_hash(&spec.path));
    let item = user::image_item(id.clone(), stem, &spec.path, width, depth);
    register_user_item(item)?;
    set_active_item(cx, &id);
    Ok(id)
}

// ----- placing -----

/// Places `spec` with its centre on `at`. Returns the new symbol's id.
pub fn place_image(cx: &mut EditorContext, spec: &ImageSpec, at: Point, billboard: bool) -> Id {
    let (w, d) = spec.default_size();
    let sym = if billboard {
        let h = DEFAULT_BILLBOARD_HEIGHT;
        PlacedSymbol::billboard(
            spec.clone(),
            Point::new(at.x, at.y - BILLBOARD_PLAN_DEPTH * 0.5),
            h * spec.aspect(),
            h,
        )
    } else {
        PlacedSymbol::picture(spec.clone(), Point::new(at.x, at.y - d * 0.5), w, d)
    };
    let fl = cx.floor;
    cx.project.add_symbol(fl, sym)
}

/// Stores an edited distribution record (the Specification OK) and
/// regenerates its copies, as one undo step. `false` when it is gone.
pub fn apply_distribution(cx: &mut EditorContext, draft: &PlacedSymbol) -> bool {
    cx.begin_change("Distribution Specification");
    let fl = cx.floor;
    let Some(slot) = cx.project.floors[fl]
        .symbols
        .iter_mut()
        .find(|s| s.id == draft.id && s.distribution.is_some())
    else {
        cx.cancel_change();
        return false;
    };
    *slot = draft.clone();
    cx.project.rebuild_distribution(fl, draft.id);
    cx.mark_dirty();
    true
}

/// Rebuilds the distributions whose record was moved (the Select tool moves
/// the record only). Returns how many were rebuilt.
pub fn sync_distributions(cx: &mut EditorContext) -> usize {
    let fl = cx.floor;
    let n = cx.project.sync_moved_distributions(fl);
    if n > 0 {
        cx.mark_dirty();
    }
    n
}

// ----- the tool -----

pub struct ImagesTool {
    mode: ImageMode,
    /// Picture chosen for repeated placement.
    pending: Option<ImageSpec>,
    /// Points of the path or region being drawn.
    points: Vec<Point>,
    hover: Option<Point>,
}

impl Default for ImagesTool {
    fn default() -> Self {
        Self {
            mode: ImageMode::CreateImage,
            pending: None,
            points: Vec::new(),
            hover: None,
        }
    }
}

impl ImagesTool {
    pub fn new(mode: ImageMode) -> Self {
        Self {
            mode,
            ..Self::default()
        }
    }

    pub fn mode(&self) -> ImageMode {
        self.mode
    }

    /// The points drawn so far (distribution modes).
    pub fn points(&self) -> &[Point] {
        &self.points
    }

    fn picture(&mut self, cx: &mut EditorContext) -> Option<ImageSpec> {
        if let Some(s) = &self.pending {
            return Some(s.clone());
        }
        let Some(path) = pick_image_file() else {
            cx.status = "No picture chosen".into();
            return None;
        };
        match load_spec(&path) {
            Ok(spec) => {
                self.pending = Some(spec.clone());
                Some(spec)
            }
            Err(e) => {
                cx.status = e;
                None
            }
        }
    }

    fn place_picture(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let billboard = self.mode == ImageMode::BillboardImage;
        let Some(spec) = self.picture(cx) else {
            return ToolResult::consumed();
        };
        let label = self.mode.name();
        cx.begin_change(label);
        let id = place_image(cx, &spec, p.snapped, billboard);
        cx.selection.set(ObjectRef::Symbol(id));
        cx.mark_dirty();
        cx.status = if spec.note.is_empty() {
            format!("Placed {}", file_name(&spec.path))
        } else {
            format!("Placed {}: {}", file_name(&spec.path), spec.note)
        };
        ToolResult::committed(label)
    }

    fn library_save(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let hit = hit_symbol(cx, p.world, 0.0)
            .and_then(|id| cx.floor().symbol(id).cloned())
            .and_then(|s| s.image.clone().map(|im| (im, s.width, s.depth)));
        let (spec, w, d) = match hit {
            Some(found) => found,
            None => {
                let Some(path) = pick_image_file() else {
                    cx.status = "No picture chosen".into();
                    return ToolResult::consumed();
                };
                match load_spec(&path) {
                    Ok(spec) => {
                        let (w, d) = spec.default_size();
                        (spec, w, d)
                    }
                    Err(e) => {
                        cx.status = e;
                        return ToolResult::consumed();
                    }
                }
            }
        };
        cx.status = match save_to_library(cx, &spec, w, d) {
            Ok(_) => format!("Saved {} in the user library", file_name(&spec.path)),
            Err(e) => e,
        };
        ToolResult::consumed()
    }

    fn place_solid(&mut self, cx: &mut EditorContext, p: &PointerEvent) -> ToolResult {
        let Some(item) = active_item().and_then(|id| find_item(&id)) else {
            cx.status = self.hint();
            return ToolResult::consumed();
        };
        let mut sym = placement_for(cx, &item, p);
        sym.solid = true;
        cx.begin_change("3D Solid Feature");
        let fl = cx.floor;
        let id = cx.project.add_symbol(fl, sym);
        cx.selection.set(ObjectRef::Symbol(id));
        cx.mark_dirty();
        cx.status = format!("Placed {} as a 3D solid", item.name);
        ToolResult::committed("3D Solid Feature")
    }

    fn add_point(&mut self, p: Point) {
        if self.points.last().is_none_or(|l| l.dist(p) > 0.5) {
            self.points.push(p);
        }
    }

    fn finish(&mut self, cx: &mut EditorContext) -> ToolResult {
        let Some((kind, spline)) = self.mode.distribution() else {
            return ToolResult::ignored();
        };
        let need = if kind == DistKind::Region { 3 } else { 2 };
        if self.points.len() < need {
            cx.status = format!("{}: click at least {need} points", self.mode.name());
            return ToolResult::consumed();
        }
        let Some(item) = active_item().and_then(|id| find_item(&id)) else {
            cx.status = self.hint();
            return ToolResult::consumed();
        };
        let mut d = Distribution::new(
            kind,
            spline,
            std::mem::take(&mut self.points),
            item.id.clone(),
            [item.width, item.depth, item.height],
        );
        d.item_elevation = item.elevation;
        d.item_image = spec_from_item(&item);
        let label = self.mode.name();
        cx.begin_change(label);
        let fl = cx.floor;
        let id = cx.project.add_distribution(fl, d);
        let n = cx.project.distribution_copies(fl, id);
        cx.selection.set(ObjectRef::Symbol(id));
        cx.mark_dirty();
        cx.status = format!("{label}: {n} x {}", item.name);
        self.hover = None;
        ToolResult::committed(label)
    }

    /// The copies a half-drawn distribution would make (preview dots).
    fn preview_copies(&self) -> Vec<Point> {
        let Some((kind, spline)) = self.mode.distribution() else {
            return Vec::new();
        };
        let Some(item) = active_item().and_then(|id| find_item(&id)) else {
            return Vec::new();
        };
        let mut pts = self.points.clone();
        if let Some(h) = self.hover {
            pts.push(h);
        }
        let need = if kind == DistKind::Region { 3 } else { 2 };
        if pts.len() < need {
            return Vec::new();
        }
        Distribution::new(
            kind,
            spline,
            pts,
            item.id.clone(),
            [item.width, item.depth, item.height],
        )
        .copies()
        .into_iter()
        .map(|c| c.center)
        .collect()
    }
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned())
}

impl Tool for ImagesTool {
    fn id(&self) -> ToolId {
        ToolId::Images
    }

    fn name(&self) -> &'static str {
        self.mode.name()
    }

    fn hint(&self) -> String {
        match self.mode {
            ImageMode::CreateImage => "Create Image: click to place a picture (PNG or JPEG)".into(),
            ImageMode::BillboardImage => {
                "Create Billboard Image: click to place a picture that faces the camera".into()
            }
            ImageMode::ImageLibrary => {
                "Create Image Library: click a picture to save it, or click empty space to pick a file"
                    .into()
            }
            ImageMode::SolidFeature => match active_item().and_then(|id| find_item(&id)) {
                Some(i) => format!("3D Solid Feature: click to place {} as a solid", i.name),
                None => "3D Solid Feature: pick an item in the Library Browser first".into(),
            },
            _ => match active_item().and_then(|id| find_item(&id)) {
                Some(i) => format!(
                    "{}: click points, Enter or double-click to finish ({})",
                    self.mode.name(),
                    i.name
                ),
                None => format!(
                    "{}: pick an item in the Library Browser first",
                    self.mode.name()
                ),
            },
        }
    }

    fn cursor(&self) -> egui::CursorIcon {
        egui::CursorIcon::Crosshair
    }

    fn set_variant(&mut self, id: ToolId) {
        if let ToolId::ImagesVariant(m) = id {
            if m != self.mode {
                self.points.clear();
                self.hover = None;
                if !matches!(m, ImageMode::CreateImage | ImageMode::BillboardImage) {
                    self.pending = None;
                }
            }
            self.mode = m;
        }
    }

    fn frame(&mut self, cx: &mut EditorContext, _ctx: &egui::Context) {
        sync_distributions(cx);
    }

    fn activate(&mut self, cx: &mut EditorContext) {
        self.points.clear();
        self.hover = None;
        cx.status = self.hint();
    }

    fn deactivate(&mut self, _cx: &mut EditorContext) {
        self.points.clear();
        self.hover = None;
    }

    fn pointer_move(&mut self, _cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        self.hover = Some(p.snapped);
        ToolResult {
            repaint: self.mode.distribution().is_some(),
            ..ToolResult::default()
        }
    }

    fn pointer_down(&mut self, cx: &mut EditorContext, p: PointerEvent) -> ToolResult {
        match self.mode {
            ImageMode::CreateImage | ImageMode::BillboardImage => self.place_picture(cx, &p),
            ImageMode::ImageLibrary => self.library_save(cx, &p),
            ImageMode::SolidFeature => self.place_solid(cx, &p),
            _ => {
                if active_item().and_then(|id| find_item(&id)).is_none() {
                    cx.status = self.hint();
                    return ToolResult::consumed();
                }
                self.add_point(p.snapped);
                cx.status = self.hint();
                ToolResult::consumed()
            }
        }
    }

    fn double_click(&mut self, cx: &mut EditorContext, _p: PointerEvent) -> ToolResult {
        if self.mode.distribution().is_some() && !self.points.is_empty() {
            return self.finish(cx);
        }
        ToolResult::ignored()
    }

    fn key(&mut self, cx: &mut EditorContext, k: KeyEvent) -> ToolResult {
        if k.is(Key::Escape) {
            if !self.points.is_empty() {
                self.points.clear();
                return ToolResult::consumed();
            }
            if self.pending.take().is_some() {
                cx.status = self.hint();
                return ToolResult::consumed();
            }
            return ToolResult::ignored();
        }
        if self.mode.distribution().is_some() {
            if k.is(Key::Enter) && !self.points.is_empty() {
                return self.finish(cx);
            }
            if (k.is(Key::Backspace) || k.is(Key::Delete)) && self.points.pop().is_some() {
                return ToolResult::consumed();
            }
        }
        ToolResult::ignored()
    }

    fn draw_overlay(&self, cx: &EditorContext, painter: &egui::Painter, cam: &Camera) {
        if self.mode.distribution().is_none() {
            return;
        }
        let pal = &cx.palette;
        let mut pts = self.points.clone();
        if let Some(h) = self.hover {
            pts.push(h);
        }
        if let Some((kind, spline)) = self.mode.distribution() {
            let mut d = Distribution::new(kind, spline, Vec::new(), "", [1.0; 3]);
            d.points = pts;
            let mut line: Vec<Pos2> = d
                .polyline()
                .iter()
                .map(|p| cam.world_to_screen(*p))
                .collect();
            if kind == DistKind::Region && line.len() > 2 {
                line.push(line[0]);
            }
            painter.add(Shape::line(line, Stroke::new(1.5_f32, pal.ghost_stroke)));
        }
        for q in self.points.iter() {
            painter.circle_filled(cam.world_to_screen(*q), 3.0, pal.selection);
        }
        for c in self.preview_copies() {
            painter.circle_stroke(
                cam.world_to_screen(c),
                3.0,
                Stroke::new(1.0_f32, pal.ghost_stroke),
            );
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::editor::placed;
    use crate::plan_defaults;
    use crate::tools::library::{clear_active_item, library_catalog};

    fn crc32(data: &[u8]) -> u32 {
        let mut c = !0u32;
        for &b in data {
            c ^= u32::from(b);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut v = (data.len() as u32).to_be_bytes().to_vec();
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        v.extend_from_slice(&body);
        v.extend_from_slice(&crc32(&body).to_be_bytes());
        v
    }

    /// A valid RGBA8 PNG (stored deflate block) of `w` x `h` made of `px`
    /// (one RGBA quadruple per pixel).
    pub(crate) fn png_fixture(w: u32, h: u32, px: &[u8]) -> Vec<u8> {
        let mut raw = Vec::new();
        for row in px.chunks(w as usize * 4) {
            raw.push(0);
            raw.extend_from_slice(row);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw {
            a = (a + u32::from(x)) % 65521;
            b = (b + a) % 65521;
        }
        let mut z = vec![0x78, 0x01, 0x01];
        z.extend_from_slice(&(raw.len() as u16).to_le_bytes());
        z.extend_from_slice(&(!(raw.len() as u16)).to_le_bytes());
        z.extend_from_slice(&raw);
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut ihdr = w.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        out.extend(chunk(b"IHDR", &ihdr));
        out.extend(chunk(b"IDAT", &z));
        out.extend(chunk(b"IEND", &[]));
        out
    }

    /// Writes a 4 x 2 PNG (left half red, right half white) to a temp file.
    pub(crate) fn fixture_file(name: &str) -> String {
        let dir =
            std::env::temp_dir().join(format!("plan-studio-image-tests-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut px = Vec::new();
        for _ in 0..2 {
            for x in 0..4 {
                px.extend_from_slice(if x < 2 {
                    &[200, 0, 0, 255]
                } else {
                    &[255, 255, 255, 255]
                });
            }
        }
        let path = dir.join(name);
        std::fs::write(&path, png_fixture(4, 2, &px)).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn click(t: &mut ImagesTool, cx: &mut EditorContext, x: f64, y: f64) -> ToolResult {
        let p = PointerEvent::at(cx, Point::new(x, y));
        t.pointer_move(cx, p);
        t.pointer_down(cx, p.with_down(true))
    }

    fn ctx() -> EditorContext {
        set_user_library_path(Some(None));
        clear_active_item();
        EditorContext::new(plan_defaults::embedded())
    }

    #[test]
    fn a_small_png_decodes_and_keys_out_a_colour() {
        let path = fixture_file("fixture.png");
        let spec = load_spec(&path).unwrap();
        assert_eq!(
            (spec.px_w, spec.px_h, spec.format),
            (4, 2, ImageFormat::Png)
        );
        assert!(spec.note.is_empty());
        // Mean of 4 red and 4 white pixels.
        assert_eq!(spec.color, [227, 127, 127]);
        let img = decode_pixels(&spec, 1024).unwrap();
        assert_eq!((img.width, img.height), (4, 2));
        assert_eq!(&img.pixels[..4], &[200, 0, 0, 255]);
        let mut keyed = spec.clone();
        keyed.transparency = Some([255, 255, 255]);
        let img = decode_pixels(&keyed, 1024).unwrap();
        assert_eq!(img.pixels[3], 255, "red stays");
        assert_eq!(img.pixels[2 * 4 + 3], 0, "white is transparent");
        // Tolerance widens the key.
        keyed.transparency = Some([255, 40, 40]);
        keyed.tolerance = 0;
        assert_eq!(decode_pixels(&keyed, 1024).unwrap().pixels[2 * 4 + 3], 255);
        keyed.tolerance = 215;
        assert_eq!(decode_pixels(&keyed, 1024).unwrap().pixels[2 * 4 + 3], 0);
        assert!(load_spec("/definitely/not/here.png").is_err());
    }

    #[test]
    fn jpegs_are_a_placeholder_frame_with_a_note() {
        let path = fixture_file("x.png").replace("x.png", "photo.jpg");
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 11, 8, 0, 20, 0, 30, 3, 1, 0x22, 0,
        ];
        std::fs::write(&path, jpeg).unwrap();
        let spec = load_spec(&path).unwrap();
        assert_eq!(
            (spec.px_w, spec.px_h, spec.format),
            (30, 20, ImageFormat::Jpeg)
        );
        assert_eq!(spec.note, JPEG_NOTE);
        assert!(decode_pixels(&spec, 512).is_none());
        // It still draws (frame) without a texture.
        let ctx = egui::Context::default();
        let mut cx = ctx_for_draw();
        let id = place_image(&mut cx, &spec, Point::new(50.0, 50.0), false);
        let sym = cx.floor().symbol(id).unwrap().clone();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let painter = ui.painter().clone();
                draw_image(&painter, &Camera::default_view(), &sym, Color32::BLACK);
            });
        });
    }

    fn ctx_for_draw() -> EditorContext {
        ctx()
    }

    #[test]
    fn create_image_picks_once_places_and_undoes() {
        let mut cx = ctx();
        let path = fixture_file("create.png");
        let mut t = ImagesTool::new(ImageMode::CreateImage);
        // A cancelled picker places nothing.
        inject_pick(None);
        let r = click(&mut t, &mut cx, 100.0, 100.0);
        assert!(r.commit.is_none() && cx.floor().symbols.is_empty());
        inject_pick(Some(&path));
        let r = click(&mut t, &mut cx, 100.0, 100.0);
        assert_eq!(r.commit.as_deref(), Some("Create Image"));
        let s = &cx.floor().symbols[0];
        let spec = s.image.as_ref().unwrap();
        assert_eq!((spec.px_w, spec.px_h, spec.billboard), (4, 2, false));
        // Longest side 36", proportions kept, centred on the click.
        assert!((s.width - 36.0).abs() < 1e-9 && (s.depth - 18.0).abs() < 1e-9);
        let c = placed::symbol_center(s);
        let want = PointerEvent::at(&cx, Point::new(100.0, 100.0)).snapped;
        assert!(c.dist(want) < 1e-6, "{c:?} vs {want:?}");
        // The second click reuses the picture without asking.
        let r = click(&mut t, &mut cx, 300.0, 100.0);
        assert_eq!(r.commit.as_deref(), Some("Create Image"));
        assert_eq!(cx.floor().symbols.len(), 2);
        // Esc forgets it; the next click asks again.
        assert!(t.key(&mut cx, KeyEvent::escape()).consumed);
        let r = click(&mut t, &mut cx, 400.0, 100.0);
        assert!(r.commit.is_none());
        assert_eq!(cx.undo().as_deref(), Some("Create Image"));
        assert_eq!(cx.floor().symbols.len(), 1);
        // The plan file keeps the picture by path.
        let q = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert_eq!(q.floors[0].symbols[0].image.as_ref().unwrap().path, path);
    }

    #[test]
    fn billboards_are_upright_and_picked_like_symbols() {
        let mut cx = ctx();
        let path = fixture_file("bb.png");
        let mut t = ImagesTool::new(ImageMode::BillboardImage);
        inject_pick(Some(&path));
        let r = click(&mut t, &mut cx, 60.0, 60.0);
        assert_eq!(r.commit.as_deref(), Some("Create Billboard Image"));
        let s = cx.floor().symbols[0].clone();
        assert!(s.image.as_ref().unwrap().billboard);
        assert_eq!(s.height, DEFAULT_BILLBOARD_HEIGHT);
        assert_eq!(s.depth, BILLBOARD_PLAN_DEPTH);
        assert!((s.width - 144.0).abs() < 1e-9, "72\" tall at 2:1");
        assert_eq!(hit_symbol(&cx, placed::symbol_center(&s), 0.0), Some(s.id));
        // The camera orientation helper turns it toward the eye.
        let eye = Some([500.0f32, 60.0, -60.0]);
        let a = placed::billboard_orientation(&s, eye);
        assert!((a - 270.0).abs() < 1.0, "{a}");
        let m = placed::image_meshes_facing(&cx.project, eye);
        assert_eq!(m.len(), 1);
        assert_eq!(placed::image_meshes(&cx.project).len(), 1);
    }

    #[test]
    fn image_library_saves_a_reusable_picture_item() {
        let mut cx = ctx();
        let path = fixture_file("lib.png");
        let lib_file = std::env::temp_dir()
            .join(format!("plan-studio-image-tests-{}", std::process::id()))
            .join("user-library.json");
        let _ = std::fs::remove_file(&lib_file);
        set_user_library_path(Some(Some(lib_file.clone())));
        let mut t = ImagesTool::new(ImageMode::ImageLibrary);
        inject_pick(Some(&path));
        let r = click(&mut t, &mut cx, 10.0, 10.0);
        assert!(r.commit.is_none(), "no model change");
        let id = active_item().expect("the saved item becomes active");
        assert!(id.starts_with("user.image."));
        let item = find_item(&id).unwrap();
        assert_eq!(plan_library::user::image_path(&item), Some(path.as_str()));
        assert!(lib_file.exists());
        // Placing the item with the Library tool places the picture.
        let p = PointerEvent::at(&cx, Point::new(200.0, 200.0));
        let sym = placement_for(&cx, &item, &p);
        assert_eq!(sym.image.as_ref().unwrap().path, path);
        // A fresh session reloads it from the file.
        set_user_library_path(Some(Some(lib_file)));
        assert!(find_item(&id).is_some());
        assert_eq!(user_items().len(), 1);
        // Clicking a placed picture saves that picture's size.
        let mut cx = ctx();
        set_user_library_path(Some(None));
        let mut t = ImagesTool::new(ImageMode::CreateImage);
        inject_pick(Some(&path));
        click(&mut t, &mut cx, 100.0, 100.0);
        let mut t = ImagesTool::new(ImageMode::ImageLibrary);
        click(&mut t, &mut cx, 100.0, 100.0);
        assert!(cx.status.starts_with("Saved"), "{}", cx.status);
    }

    fn first_item() -> &'static plan_library::CatalogItem {
        library_catalog().all_items().next().unwrap()
    }

    #[test]
    fn distribution_tool_draws_a_path_and_places_copies() {
        let mut cx = ctx();
        let mut t = ImagesTool::new(ImageMode::PolylinePath);
        // No active item: nothing is drawn.
        click(&mut t, &mut cx, 0.0, 0.0);
        assert!(t.points().is_empty());
        set_active_item(&mut cx, &first_item().id);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 240.0, 0.0);
        // A double-click's second press does not double the last point.
        click(&mut t, &mut cx, 240.0, 0.0);
        assert_eq!(t.points().len(), 2);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert_eq!(r.commit.as_deref(), Some("Polyline Distribution Path"));
        let rec = cx
            .floor()
            .symbols
            .iter()
            .find(|s| s.distribution.is_some())
            .unwrap()
            .clone();
        let d = rec.distribution.clone().unwrap();
        let n = (240.0 / d.spacing).floor() as usize + 1;
        assert_eq!(cx.project.distribution_copies(0, rec.id), n);
        assert_eq!(cx.floor().symbols.len(), n + 1);
        assert!(t.points().is_empty());
        // The record is picked on its path (between two copies); copies are
        // separate symbols.
        let mid = Point::new(d.spacing * 0.5, 0.0);
        assert_eq!(hit_symbol(&cx, mid, 0.0), Some(rec.id));
        assert_eq!(hit_symbol(&cx, Point::new(mid.x, 200.0), 0.0), None);
        // One undo removes the record and its copies together.
        assert_eq!(cx.undo().as_deref(), Some("Polyline Distribution Path"));
        assert!(cx.floor().symbols.is_empty());
    }

    #[test]
    fn region_tools_need_three_points_and_spline_modes_sample() {
        let mut cx = ctx();
        set_active_item(&mut cx, &first_item().id);
        let mut t = ImagesTool::new(ImageMode::SplineRegion);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 240.0, 0.0);
        let r = t.key(&mut cx, KeyEvent::key(Key::Enter));
        assert!(r.commit.is_none(), "two points are not a region");
        click(&mut t, &mut cx, 240.0, 240.0);
        click(&mut t, &mut cx, 0.0, 240.0);
        assert!(t.key(&mut cx, KeyEvent::key(Key::Backspace)).consumed);
        click(&mut t, &mut cx, 0.0, 240.0);
        let ev = PointerEvent::at(&cx, Point::new(0.0, 240.0));
        let r = t.double_click(&mut cx, ev);
        assert_eq!(r.commit.as_deref(), Some("Spline Distribution Region"));
        let rec = cx
            .floor()
            .symbols
            .iter()
            .find(|s| s.distribution.is_some())
            .unwrap();
        let d = rec.distribution.as_ref().unwrap();
        assert!(d.spline && d.kind == DistKind::Region);
        assert!(d.polyline().len() > d.points.len());
        assert!(cx.project.distribution_copies(0, rec.id) > 0);
    }

    #[test]
    fn solid_feature_places_the_active_item_as_a_solid() {
        let mut cx = ctx();
        let mut t = ImagesTool::new(ImageMode::SolidFeature);
        let r = click(&mut t, &mut cx, 50.0, 50.0);
        assert!(r.commit.is_none(), "needs an active item");
        set_active_item(&mut cx, &first_item().id);
        let r = click(&mut t, &mut cx, 50.0, 50.0);
        assert_eq!(r.commit.as_deref(), Some("3D Solid Feature"));
        let s = &cx.floor().symbols[0];
        assert!(s.solid);
        let meshes = placed::solid_meshes(&cx.project);
        assert!(!meshes.is_empty());
        assert!(meshes
            .iter()
            .all(|m| m.material == plan_3d::Material::Concrete));
        // The flag survives a save.
        let q = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        assert!(q.floors[0].symbols[0].solid);
    }

    #[test]
    fn moved_records_take_their_copies_along() {
        let mut cx = ctx();
        set_active_item(&mut cx, &first_item().id);
        let mut t = ImagesTool::new(ImageMode::PolylinePath);
        click(&mut t, &mut cx, 0.0, 0.0);
        click(&mut t, &mut cx, 120.0, 0.0);
        t.key(&mut cx, KeyEvent::key(Key::Enter));
        let id = cx
            .floor()
            .symbols
            .iter()
            .find(|s| s.distribution.is_some())
            .unwrap()
            .id;
        let first = cx
            .floor()
            .symbols
            .iter()
            .find(|s| s.owner == Some(id))
            .unwrap()
            .position;
        let fl = cx.floor;
        let to = cx.floor().symbol(id).unwrap().position + Point::new(60.0, 30.0);
        cx.project.move_symbol(fl, id, to);
        assert_eq!(sync_distributions(&mut cx), 1);
        let moved = cx
            .floor()
            .symbols
            .iter()
            .find(|s| s.owner == Some(id))
            .unwrap()
            .position;
        assert!(moved.dist(first + Point::new(60.0, 30.0)) < 1e-6);
    }

    #[test]
    fn every_images_flyout_entry_selects_its_tool() {
        use crate::tools::ToolSet;
        let mut cx = ctx();
        let mut set = ToolSet::new();
        for m in ImageMode::ALL {
            let id = ToolId::ImagesVariant(m);
            set.set_active(&mut cx, id);
            assert_eq!(set.active().name(), m.name());
            assert_eq!(set.active_id(), id);
            assert!(!set.active().hint().is_empty());
        }
        let names: Vec<_> = crate::toolbar::image()
            .entries
            .iter()
            .chain(crate::toolbar::distributed_objects().entries.iter())
            .chain(crate::toolbar::solid_3d().entries.iter())
            .filter(|e| {
                e.name.contains("Image")
                    || e.name.contains("Distribution")
                    || e.name == "3D Solid Feature"
            })
            .map(|e| e.name)
            .collect();
        assert_eq!(names.len(), 8);
        for n in names {
            assert!(ImageMode::ALL.iter().any(|m| m.name() == n), "{n}");
        }
    }
}
