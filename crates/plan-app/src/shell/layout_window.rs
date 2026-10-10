//! Chief's layout view: the project's layout drawn at zoom in the main area,
//! with page tabs at the bottom, layout boxes you select, move and resize,
//! Send to Layout, the Layout Box Specification, Page Setup, Project
//! Information, the Layout Page Table and Print / PDF.
//!
//! * The layout lives in `Project::layout` as the JSON of a
//!   `plan_layout::Layout` ([`load`] / [`store`]), so it is saved and opened
//!   with the `.psplan`. The title block macros read `Project::info`
//!   (Tools > Project Information); see [`macro_context`].
//! * [`LayoutView`] holds the window state (current page, selection, zoom,
//!   a local undo history of the layout, the drawing cache) and every edit as a
//!   plain method on `&mut Project`, so the model logic runs without egui.
//! * The app owns one [`LayoutView`] per thread; `main.rs` calls [`dispatch`],
//!   [`show_central`] and [`show_dialogs`].
//!
//! Layout space is paper inches with the origin at the sheet's bottom-left.
//!
//! Undo: plan and layout share the one history of the editor context
//! (`EditorContext::history`, whole-project snapshots, layout JSON included),
//! so Edit > Undo, Cmd+Z and the Undo button step back through plan and layout
//! edits in the order they were made, and undoing a plan edit can never
//! silently roll back a later layout edit (or the other way round). A layout
//! edit only has the project, not the context, at hand: it parks the project
//! as it was before the edit in [`LayoutView::steps`], and the entry points
//! that hold the context ([`dispatch`], [`show_central`], [`show_dialogs`],
//! [`new_layout`], [`undo`], [`redo`]) record those as undo steps with
//! `EditorContext::record_undo_step` at the end of the frame.

use crate::dialogs::camera as cam;
use crate::dialogs::layout::{
    default_layout_template, layout_templates_dir, list_layout_templates, save_layout_template,
    BoxSpec, BoxSpecDialog, CadTextDialog, CadTextSpec, CloudDialog, CloudSpec, CopyBoxDialog,
    LayoutDefaults, LayoutDefaultsDialog, LayoutLayersDialog, LayoutTarget, LayoutTemplateDialog,
    LeaderDialog, LeaderSpec, NameDialog, PageChoice, PageSetup, PageSetupDialog, Placement,
    PrintDialog, SendSource, SendSpec, SheetSizes, SheetSizesDialog, SnapshotSpec, TemplateMode,
    TextBoxDialog, TextBoxSpec,
};
use crate::dialogs::layout_box::{
    link_choices, BoxSpecChoices, ChangeScaleDialog, LayoutBoxDialog, LayoutLineDialog,
};
use crate::dialogs::layout_revisions::RevisionDialog;
use crate::dialogs::page_info::PageInfoDialog;
use crate::dialogs::print::{
    self, Image3dDialog, ImageDialog, ModelDialog, PrintPreviewDialog, PrintTarget,
};
use crate::dialogs::send_to_layout::{SendAnswers, SendDetails, SendToLayoutDialog};
use crate::dialogs::Outcome;
use crate::editor::EditorContext;
use eframe::egui::{
    self, Align2, Color32, CursorIcon, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
};
use plan_core::{CadItem, CadObject, Id, Point, Project};
use plan_docs::{MasterList, Scale, CHIEF_SHEET_BACKGROUND};
use plan_elevation::LineWeight;
use plan_layout::{
    render_pdf, send_to_layout, source_size_in, AlignEdge, BoxArtwork, BoxSource, BoxText,
    CameraLink, Layout, LayoutBox, LayoutLayers, LayoutPage, LayoutRenderContext, MacroContext,
    NewScale, PerspectiveImage, PerspectiveRequest, PrintOptions, SendOptions, SendRequest,
    SendScale, Sent, Spread, TitleBlockStyle, UpdateScope, AUTO_SCALE_CEILING, LAYER_CAD,
    LAYER_REVISION_CLOUDS, LAYER_TEXT, LAYER_TITLE_BLOCK,
};
use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::f64::consts::TAU;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::{Arc, Mutex, PoisonError};

/// Smallest side of a layout box, paper inches.
const MIN_BOX_IN: f64 = 0.25;
/// Drag snap of a layout that has not set its own, paper inches (1/16").
const SNAP_IN: f64 = plan_layout::DEFAULT_SNAP_UNIT_IN;

thread_local! {
    /// The open layout's General Layout Defaults: Use Snap Grid and the Grid
    /// Snap Unit (kept in step by [`LayoutView::sync_snap`]).
    static SNAP: Cell<(bool, f64)> = const { Cell::new((true, SNAP_IN)) };
    /// The part of the plan view on screen, plan inches `[x0, y0, x1, y1]`
    /// (set each frame by the application): what Send to Layout's Current
    /// Screen sends.
    static PLAN_SCREEN: Cell<Option<[f64; 4]>> = const { Cell::new(None) };
    /// The answers of the last Send to Layout, kept for the session: the next
    /// dialog starts from them (manual p. 1400).
    static LAST_SEND: RefCell<Option<SendAnswers>> = const { RefCell::new(None) };
}

/// Tells the layout window which part of the plan view is on screen (plan
/// inches), for Send to Layout > Current Screen. `None` when the plan view is
/// not showing.
pub fn set_plan_screen(extent: Option<[f64; 4]>) {
    PLAN_SCREEN.with(|s| s.set(extent));
}

/// The Grid Snap Unit drags snap to and arrow keys nudge by, paper inches.
fn snap_unit() -> f64 {
    SNAP.with(Cell::get).1
}
const MIN_ZOOM: f32 = 2.0;
const MAX_ZOOM: f32 = 400.0;
const HANDLE_PX: f32 = 7.0;

fn st(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

const INK: Color32 = Color32::from_gray(0x22);
const SURROUND: Color32 = Color32::from_gray(0x3A);
const SELECT_BLUE: Color32 = Color32::from_rgb(0x2F, 0x6C, 0xB3);

// ------------------------------------------------------------- storage --

/// The project's layout, if it has one and its JSON reads back.
pub fn load(project: &Project) -> Option<Layout> {
    let mut layout: Layout = serde_json::from_value(project.layout.clone()?).ok()?;
    // Layouts saved before the Layout Box Labels layer existed gain it.
    layout.layers.complete();
    Some(layout)
}

/// Writes `layout` into the project.
pub fn store(project: &mut Project, layout: &Layout) {
    project.layout = serde_json::to_value(layout).ok();
}

/// The values the title block macros expand to: the project's name and its
/// Project Information (Tools > Project Information, `Project::info`).
pub fn macro_context(project: &Project) -> MacroContext {
    // Includes %company%, %client.phone%, %drawn.by%, %custom.<key>%...
    plan_layout::macros_for(project)
}

// ------------------------------------- master list, pictures, perspectives --

thread_local! {
    /// The master list in memory instead of `~/.plan-studio/master-list.json`
    /// (tests; nothing sets it in the application).
    static MASTER_OVERRIDE: RefCell<Option<MasterList>> = const { RefCell::new(None) };
    /// Tests: render every perspective view at this `(width, height, samples)`
    /// instead of the size its box asks for.
    static PERSPECTIVE_TEST_SIZE: Cell<Option<(u32, u32, u32)>> = const { Cell::new(None) };
    /// Default Settings > Door and Window Labels, as the plan's editor holds
    /// them (the render contexts see only the project); refreshed every frame
    /// by [`show_dialogs`].
    static OPENING_LABELS: RefCell<plan_core::OpeningLabelDefaults> =
        RefCell::new(plan_core::OpeningLabelDefaults::default());
}

/// Hands the layout the label settings of the plan's Default Settings.
pub fn set_opening_labels(labels: &plan_core::OpeningLabelDefaults) {
    OPENING_LABELS.with(|l| {
        if *l.borrow() != *labels {
            *l.borrow_mut() = labels.clone();
        }
    });
}

/// The label settings plan boxes print their openings with.
fn opening_labels() -> plan_core::OpeningLabelDefaults {
    OPENING_LABELS.with(|l| l.borrow().clone())
}

/// `~/.plan-studio/master-list.json`: prices, waste and stock lengths of the
/// Materials List.
pub fn master_list_path() -> Option<std::path::PathBuf> {
    crate::paths::user_file("master-list.json")
}

/// The master list: the user's file, else the defaults.
pub fn load_master_list() -> MasterList {
    if let Some(m) = MASTER_OVERRIDE.with(|m| m.borrow().clone()) {
        return m;
    }
    master_list_path().map_or_else(MasterList::default, |p| MasterList::load(&p))
}

/// Saves the master list (to its file, or the in-memory stand-in in tests).
pub fn save_master_list(list: &MasterList) -> Result<(), String> {
    if MASTER_OVERRIDE.with(|m| m.borrow().is_some()) {
        MASTER_OVERRIDE.with(|m| *m.borrow_mut() = Some(list.clone()));
        return Ok(());
    }
    let path = master_list_path().ok_or_else(|| crate::paths::NO_HOME.to_string())?;
    list.save(&path)
        .map_err(|e| format!("Could not save the master list: {e}"))
}

/// Keeps the master list in memory (tests).
#[cfg(test)]
pub fn use_memory_master_list(list: MasterList) {
    MASTER_OVERRIDE.with(|m| *m.borrow_mut() = Some(list));
}

/// Makes the perspective renders small and fast (tests).
#[cfg(test)]
pub fn set_perspective_size(w: u32, h: u32, samples: u32) {
    PERSPECTIVE_TEST_SIZE.with(|s| s.set(Some((w, h, samples))));
}

/// Renders perspective views at the size their boxes ask for again (tests).
#[cfg(test)]
pub fn clear_perspective_size() {
    PERSPECTIVE_TEST_SIZE.with(|s| s.set(None));
}

/// The request as it will be rendered: the test size, when one is set.
fn effective_request(req: PerspectiveRequest) -> PerspectiveRequest {
    match PERSPECTIVE_TEST_SIZE.with(|s| s.get()) {
        Some((width, height, samples)) => PerspectiveRequest {
            width,
            height,
            samples,
            ..req
        },
        None => req,
    }
}

type PerspectiveCache = Vec<(u64, Arc<PerspectiveImage>)>;
static PERSPECTIVES: Mutex<PerspectiveCache> = Mutex::new(Vec::new());
/// Perspective images kept (oldest dropped first).
const PERSPECTIVE_KEEP: usize = 12;
/// Most pixel memory the cached perspective images may hold.
const PERSPECTIVE_KEEP_BYTES: usize = 192 * 1024 * 1024;

/// Names the camera view: the camera, the model it sees and the render size
/// and quality.
fn perspective_key(project: &Project, req: &PerspectiveRequest) -> Option<u64> {
    let c = project.camera(req.camera_id)?;
    let mut h = DefaultHasher::new();
    format!("{c:?}").hash(&mut h);
    serde_json::to_string(&project.floors)
        .unwrap_or_default()
        .hash(&mut h);
    format!(
        "{:?}{:?}{:?}",
        project.terrain, project.lights, project.light_options
    )
    .hash(&mut h);
    (req.width, req.height, req.samples).hash(&mut h);
    Some(h.finish())
}

/// The perspective image for `req` if it was rendered for the plan as it is.
pub fn cached_perspective(
    project: &Project,
    req: &PerspectiveRequest,
) -> Option<Arc<PerspectiveImage>> {
    let key = perspective_key(project, req)?;
    let cache = PERSPECTIVES.lock().unwrap_or_else(PoisonError::into_inner);
    cache
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, i)| i.clone())
}

/// Ray traces the view of `req` (its camera at its size and sample count).
/// Limits: the default clear-day sun and sky; the plan's point lights shine.
fn trace_perspective(project: &Project, req: &PerspectiveRequest) -> Option<PerspectiveImage> {
    let c = project.camera(req.camera_id)?;
    let (w, h, samples) = (req.width.max(1), req.height.max(1), req.samples.max(1));
    // The 3D view's own scene: roofs, stairs, cabinets, terrain and the rest,
    // not just the walls and openings.
    let scene = view_scene(project);
    let renderer = plan_render::Renderer::new(&scene);
    let elevation = project.floors.get(c.floor).map_or(0.0, |f| f.elevation);
    let fov = crate::shell::view3d_panel::vertical_fov(c.fov_deg as f32, w as f32 / h as f32);
    let camera = plan_render::Camera::from_plan(
        c.position,
        c.direction_deg,
        elevation + c.eye_height,
        f64::from(fov),
    );
    let settings = plan_render::RenderSettings {
        width: w,
        height: h,
        samples,
        ..plan_render::RenderSettings::default()
    };
    // The plan's lights shine (Adjust Lights, and the electrical fixtures when
    // the plan asks for them).
    let lights = cam::render_lights(project);
    let image = renderer.render(
        &camera,
        &plan_render::Environment::default(),
        &lights,
        &settings,
    );
    Some(PerspectiveImage {
        width: image.width,
        height: image.height,
        rgba: image.rgba,
    })
}

/// The perspective image for `req`, rendered now when it is not cached
/// (Update Views, printing). The cache is keyed by the camera, the model and
/// the render size and quality, so an unchanged plan never renders twice.
pub fn perspective_image(project: &Project, req: &PerspectiveRequest) -> Option<PerspectiveImage> {
    if let Some(hit) = cached_perspective(project, req) {
        return Some((*hit).clone());
    }
    let key = perspective_key(project, req)?;
    let img = trace_perspective(project, req)?;
    let mut cache = PERSPECTIVES.lock().unwrap_or_else(PoisonError::into_inner);
    cache.push((key, Arc::new(img.clone())));
    // Oldest out first: by count, and by memory (a print-resolution render is megabytes).
    let bytes = |c: &PerspectiveCache| c.iter().map(|(_, i)| i.rgba.len()).sum::<usize>();
    while cache.len() > PERSPECTIVE_KEEP
        || (cache.len() > 1 && bytes(&cache) > PERSPECTIVE_KEEP_BYTES)
    {
        cache.remove(0);
    }
    Some(img)
}

/// Longest side of a picture kept for a layout box, pixels.
const PICTURE_MAX_PX: usize = 1600;

type PictureCache = HashMap<String, (std::time::SystemTime, Arc<PerspectiveImage>)>;
static PICTURES: Mutex<Option<PictureCache>> = Mutex::new(None);

/// The pixels of the picture file at `path` (PNG, or JPEG baseline or
/// progressive, by the shared `plan_library::image` decoder; reloaded when the
/// file changes). A picture the decoder refuses prints a framed placeholder
/// with its name.
pub fn load_picture(path: &str) -> Option<PerspectiveImage> {
    let stamp = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    let mut guard = PICTURES.lock().unwrap_or_else(PoisonError::into_inner);
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some((t, img)) = cache.get(path) {
        if *t == stamp {
            return Some((**img).clone());
        }
    }
    let img = plan_library::image::decode_file(std::path::Path::new(path))
        .ok()?
        .downscaled(PICTURE_MAX_PX as u32);
    let out = PerspectiveImage {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
    };
    cache.insert(path.to_string(), (stamp, Arc::new(out.clone())));
    Some(out)
}

/// The scene elevations, sections and perspective views are drawn from: the
/// 3D view's (`view3d_panel::build_view_scene`), so roofs, stairs, cabinets,
/// dormers and the terrain appear, and the plan's opening display (casing,
/// jambs, sills) applies.
pub fn view_scene(project: &Project) -> plan_3d::Scene {
    crate::shell::view3d_panel::build_view_scene(
        project,
        &crate::shell::view3d_panel::ViewScope::default(),
    )
}

/// A picture of the 3D view from the camera it was taken with, `w` x `h`
/// pixels at `samples` per pixel, lit by the plan's point lights.
fn render_snapshot(
    project: &Project,
    view: &crate::shell::view3d_panel::Snapshot3d,
    w: u32,
    h: u32,
    samples: u32,
) -> PerspectiveImage {
    let settings = plan_render::RenderSettings {
        width: w.max(8),
        height: h.max(8),
        samples: samples.clamp(1, 512),
        ..plan_render::RenderSettings::default()
    };
    let renderer = plan_render::Renderer::new(&view.scene);
    let lights = cam::render_lights(project);
    let image = renderer.render(
        &view.camera,
        &plan_render::Environment::default(),
        &lights,
        &settings,
    );
    PerspectiveImage {
        width: image.width,
        height: image.height,
        rgba: image.rgba,
    }
}

thread_local! {
    /// The open 3D view, offered to the next Send to Layout so it can be sent
    /// as a picture (set by `main.rs` when the 3D view is showing).
    static SNAPSHOT_3D: RefCell<Option<crate::shell::view3d_panel::Snapshot3d>> =
        const { RefCell::new(None) };
}

/// Offers the open 3D view to Send to Layout (a picture of it can be sent).
pub fn offer_snapshot_3d(view: crate::shell::view3d_panel::Snapshot3d) {
    SNAPSHOT_3D.with(|s| *s.borrow_mut() = Some(view));
}

/// The render context for printing and sending: camera drawings, perspective
/// renders (made now when missing), picture files, the user's master list and
/// the title block macros.
pub fn render_context(project: &Project) -> LayoutRenderContext<'_> {
    let mut rcx = cam::layout_context(project)
        .with_scene_builder(view_scene)
        .with_plan_overlay(move |floor| crate::editor::plan_overlay::overlay(project, floor))
        .with_perspective_render(move |req| perspective_image(project, &effective_request(*req)))
        .with_picture_loader(load_picture)
        .with_master_list(load_master_list());
    rcx.macros = macro_context(project);
    rcx.opening_labels = opening_labels();
    rcx
}

/// Like [`render_context`] for the screen: perspective views show only when
/// they were rendered already (Update Views, Send to Layout), so drawing the
/// page never waits for the ray tracer.
fn ui_context(project: &Project) -> LayoutRenderContext<'_> {
    let mut rcx = cam::layout_context(project)
        .with_scene_builder(view_scene)
        .with_plan_overlay(move |floor| crate::editor::plan_overlay::overlay(project, floor))
        .with_perspective_render(move |req| {
            cached_perspective(project, &effective_request(*req)).map(|i| (*i).clone())
        })
        .with_picture_loader(load_picture)
        .with_master_list(load_master_list());
    rcx.macros = macro_context(project);
    rcx.opening_labels = opening_labels();
    rcx
}

/// Today as `2026-10-08` (UTC).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    crate::templates::format_time(secs)
        .chars()
        .take(10)
        .collect()
}

/// The scale a page announces: its boxes' common scale, `AS NOTED` when they
/// differ, `NTS` when no box is scaled.
fn page_scale_label(page: &LayoutPage) -> String {
    let mut labels: Vec<&str> = page
        .boxes
        .iter()
        .filter(|b| is_scaled(&b.source))
        .map(|b| b.scale.label())
        .collect();
    labels.sort_unstable();
    labels.dedup();
    match labels.as_slice() {
        [] => "NTS".to_string(),
        [one] => (*one).to_string(),
        _ => "AS NOTED".to_string(),
    }
}

fn is_scaled(s: &BoxSource) -> bool {
    matches!(
        s,
        BoxSource::PlanView { .. }
            | BoxSource::Elevation { .. }
            | BoxSource::Section { .. }
            | BoxSource::Camera { .. }
            | BoxSource::CadDetail { .. }
    )
}

// ---------------------------------------------------------- page logic --

/// Content pages are numbered consecutively in page order, from the lowest
/// number any of them has (so moving the cover, sheet 0, away from the front
/// does not shift the whole set up by one); template pages keep their number.
pub fn renumber_pages(layout: &mut Layout) {
    layout.renumber_pages();
}

fn next_box_id(layout: &Layout) -> Id {
    layout
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter().map(|b| b.id))
        .max()
        .map_or(1, |m| m + 1)
}

fn next_page_number(layout: &Layout) -> u32 {
    layout
        .pages
        .iter()
        .map(|p| p.number)
        .max()
        .map_or(1, |m| m + 1)
}

/// Inserts an empty page before or after `index` and returns its index.
pub fn insert_page(layout: &mut Layout, index: usize, before: bool) -> usize {
    let at = if before { index } else { index + 1 }.min(layout.pages.len());
    let number = next_page_number(layout);
    layout.add_page(number, format!("Page {number}"));
    let page = layout.pages.pop().expect("just added");
    layout.pages.insert(at, page);
    renumber_pages(layout);
    at
}

/// Copies page `index` (with new box ids) right after it; returns the copy's index.
pub fn duplicate_page(layout: &mut Layout, index: usize) -> Option<usize> {
    let mut copy = layout.pages.get(index)?.clone();
    let first = next_box_id(layout);
    for (id, b) in (first..).zip(copy.boxes.iter_mut()) {
        b.id = id;
    }
    copy.title = format!("{} copy", copy.title);
    copy.number = next_page_number(layout);
    layout.pages.insert(index + 1, copy);
    renumber_pages(layout);
    Some(index + 1)
}

/// Deletes page `index`. The last printed page cannot be deleted.
pub fn delete_page(layout: &mut Layout, index: usize) -> Result<(), &'static str> {
    let Some(page) = layout.pages.get(index) else {
        return Err("No such page");
    };
    if !page.template_page && layout.content_pages().len() <= 1 {
        return Err("A layout keeps at least one page");
    }
    if let Some(why) = layout.delete_blocker(index) {
        return Err(why);
    }
    layout.pages.remove(index);
    renumber_pages(layout);
    Ok(())
}

/// Swaps page `index` with its neighbour (`forward` = the next one) and
/// returns the page's new index.
pub fn exchange_page(layout: &mut Layout, index: usize, forward: bool) -> Option<usize> {
    let other = if forward {
        index.checked_add(1).filter(|o| *o < layout.pages.len())?
    } else {
        index.checked_sub(1)?
    };
    layout.pages.swap(index, other);
    renumber_pages(layout);
    Some(other)
}

/// Removes pages outside `range` (1-based ordinals among the printed pages);
/// template pages stay so their content still repeats.
pub fn pages_in_range(layout: &Layout, range: Option<(usize, usize)>) -> Layout {
    let mut out = layout.clone();
    // The pages left out still count for the labels and page macros.
    out.bake_numbering();
    if let Some((from, to)) = range {
        let mut ordinal = 0;
        out.pages.retain(|p| {
            if p.template_page {
                return true;
            }
            ordinal += 1;
            (from..=to).contains(&ordinal)
        });
    }
    out
}

/// The layout with its Update on Demand views updated: they update when the
/// page they are on prints (manual p. 1403). Plot Lines views stay as they are.
fn updated_for_print(layout: Layout, rcx: &LayoutRenderContext) -> Layout {
    let semi = layout
        .pages
        .iter()
        .flat_map(|p| p.boxes.iter())
        .any(|b| b.update_kind() == plan_layout::UpdateKind::SemiDynamic);
    if !semi {
        return layout;
    }
    let mut layout = layout;
    plan_layout::update_views(&mut layout, rcx, &UpdateScope::OnPrint, None);
    layout
}

/// The layout as a PDF: every printed page, or the pages in `range`, with the
/// Project Information in the title blocks and camera boxes drawn.
pub fn print_bytes(layout: &Layout, project: &Project, range: Option<(usize, usize)>) -> Vec<u8> {
    let rcx = render_context(project);
    let layout = updated_for_print(pages_in_range(layout, range), &rcx);
    render_pdf(&layout, &rcx)
}

/// The layout printed with the Print dialog's options: paper, scale, tiling,
/// colour mode, line weights and range (see [`plan_layout::print_layout_pdf`]).
pub fn print_with(layout: &Layout, project: &Project, opts: &PrintOptions) -> Vec<u8> {
    let rcx = render_context(project);
    let layout = updated_for_print(layout.clone(), &rcx);
    plan_layout::print_layout_pdf(&layout, &rcx, opts)
}

/// Floor `floor`'s plan printed at the options' scale. Returns the PDF and the
/// drawing scale used.
pub fn print_plan(
    project: &Project,
    floor: usize,
    title: &str,
    opts: &PrintOptions,
) -> (Vec<u8>, Scale) {
    plan_layout::print_plan_view_pdf(
        &render_context(project),
        floor,
        &project.layer_sets.active,
        title,
        opts,
    )
}

/// The Materials List as a one-sheet PDF: a table per category, priced from
/// the master list. `None` when the plan has no materials.
pub fn materials_pdf(project: &Project) -> Option<Vec<u8>> {
    let rcx = render_context(project);
    let mut layout = Layout::new(
        format!("{} Materials List", project.name),
        plan_docs::SheetSize::ArchC,
    );
    plan_layout::add_materials_page(&mut layout, &rcx, 1).then(|| render_pdf(&layout, &rcx))
}

/// Floor `floor`'s plan as PNG bytes `width_px` wide: its lines on white, at
/// `scale` or fitted to the image when `None`.
pub fn plan_png(
    project: &Project,
    floor: usize,
    layer_set: &str,
    scale: Option<Scale>,
    width_px: u32,
) -> Vec<u8> {
    let rcx = render_context(project);
    // The image is `width_px` wide whatever the scale; the scale sets how big
    // the plan's text and dimensions are against its walls.
    let scale = scale.unwrap_or(Scale::QuarterInch);
    let (w, h, rgba) = plan_layout::plan_view_image(&rcx, floor, layer_set, scale, width_px);
    plan_render::encode_png(&plan_render::Image {
        width: w,
        height: h,
        rgba,
        hdr: Vec::new(),
    })
}

/// Writes an edited box back (moving it to its new page when that changed).
/// False when the box no longer exists.
pub fn apply_box_spec(layout: &mut Layout, spec: &BoxSpec) -> bool {
    let id = spec.layout_box.id;
    let Some((pi, bi)) = layout
        .pages
        .iter()
        .enumerate()
        .find_map(|(pi, p)| p.boxes.iter().position(|b| b.id == id).map(|bi| (pi, bi)))
    else {
        return false;
    };
    if layout.pages[pi].number == spec.page || layout.page(spec.page).is_none() {
        layout.pages[pi].boxes[bi] = spec.layout_box.clone();
    } else {
        layout.pages[pi].boxes.remove(bi);
        if let Some(p) = layout.page_mut(spec.page) {
            p.boxes.push(spec.layout_box.clone());
        }
    }
    true
}

// -------------------------------------------------------------- geometry --

/// `[x_min, y_min, x_max, y_max]` of a box, paper inches.
pub fn bounds(b: &LayoutBox) -> [f64; 4] {
    let (a, c) = b.rect_in;
    [a.x.min(c.x), a.y.min(c.y), a.x.max(c.x), a.y.max(c.y)]
}

fn set_bounds(b: &mut LayoutBox, r: [f64; 4]) {
    b.rect_in = (Point::new(r[0], r[1]), Point::new(r[2], r[3]));
}

/// The rectangle a box's turned content occupies: the box turned about its
/// centre by its quarter turns (the same rectangle for 0 and 180 degrees, width
/// and height swapped for 90 and 270). The frame and the clip stay at
/// [`bounds`].
pub fn content_bounds(b: &LayoutBox) -> [f64; 4] {
    let r = bounds(b);
    if b.quarter_turns().is_multiple_of(2) {
        return r;
    }
    let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
    let (hw, hh) = ((r[3] - r[1]) / 2.0, (r[2] - r[0]) / 2.0);
    [cx - hw, cy - hh, cx + hw, cy + hh]
}

/// The area that selects a box: its frame, plus (for a box that does not clip)
/// the turned content, which then sticks out of the frame.
pub fn hit_bounds(b: &LayoutBox) -> [f64; 4] {
    let r = bounds(b);
    if b.clip {
        return r;
    }
    let c = content_bounds(b);
    [
        r[0].min(c[0]),
        r[1].min(c[1]),
        r[2].max(c[2]),
        r[3].max(c[3]),
    ]
}

/// Screen pixels between the top edge of a selected box and its rotate knob.
const ROTATE_KNOB_PX: f32 = 18.0;

/// Where the rotate knob of a box with hit area `r` sits: centred above it.
pub fn rotate_knob(r: [f64; 4], z: f32) -> (f64, f64) {
    ((r[0] + r[2]) / 2.0, r[3] + f64::from(ROTATE_KNOB_PX / z))
}

/// Is `(x, y)` on the rotate knob of a box with hit area `r` (zoom `z`)?
pub fn on_rotate_knob(r: [f64; 4], x: f64, y: f64, z: f32) -> bool {
    let (kx, ky) = rotate_knob(r, z);
    let tol = f64::from(HANDLE_PX / z);
    (kx - x).abs() <= tol && (ky - y).abs() <= tol
}

/// The topmost box of `page` containing the paper point `(x, y)`: inside its
/// frame, or (when it does not clip) inside its turned content.
pub fn box_at(page: &LayoutPage, x: f64, y: f64) -> Option<Id> {
    let inside = |r: [f64; 4]| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3];
    page.boxes
        .iter()
        .rev()
        .find_map(|b| (inside(bounds(b)) || (!b.clip && inside(content_bounds(b)))).then_some(b.id))
}

/// The page annotation (CAD, leader or revision cloud) within `tol` paper
/// inches of `(x, y)`, nearest first.
pub fn cad_at(page: &LayoutPage, x: f64, y: f64, tol: f64) -> Option<Id> {
    page.annotation_at(x, y, tol)
}

/// A resize handle of a selected box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

impl Handle {
    const ALL: [Handle; 8] = [
        Handle::N,
        Handle::S,
        Handle::E,
        Handle::W,
        Handle::NE,
        Handle::NW,
        Handle::SE,
        Handle::SW,
    ];

    /// The handle's position on the rectangle `r`, paper inches.
    fn position(self, r: [f64; 4]) -> (f64, f64) {
        let (mx, my) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
        match self {
            Handle::N => (mx, r[3]),
            Handle::S => (mx, r[1]),
            Handle::E => (r[2], my),
            Handle::W => (r[0], my),
            Handle::NE => (r[2], r[3]),
            Handle::NW => (r[0], r[3]),
            Handle::SE => (r[2], r[1]),
            Handle::SW => (r[0], r[1]),
        }
    }

    fn cursor(self) -> CursorIcon {
        match self {
            Handle::N | Handle::S => CursorIcon::ResizeVertical,
            Handle::E | Handle::W => CursorIcon::ResizeHorizontal,
            Handle::NE | Handle::SW => CursorIcon::ResizeNeSw,
            Handle::NW | Handle::SE => CursorIcon::ResizeNwSe,
        }
    }
}

/// The handle of `r` within `tol` paper inches of `(x, y)`.
pub fn handle_at(r: [f64; 4], x: f64, y: f64, tol: f64) -> Option<Handle> {
    Handle::ALL.into_iter().find(|h| {
        let (hx, hy) = h.position(r);
        (hx - x).abs() <= tol && (hy - y).abs() <= tol
    })
}

fn snap(v: f64, on: bool) -> f64 {
    let (grid, unit) = SNAP.with(Cell::get);
    if on && grid {
        (v / unit).round() * unit
    } else {
        v
    }
}

/// `r` with handle `h` dragged by `(dx, dy)` paper inches (edges snap when
/// `snapping`); the box never gets smaller than [`MIN_BOX_IN`].
pub fn resized(r: [f64; 4], h: Handle, dx: f64, dy: f64, snapping: bool) -> [f64; 4] {
    let mut o = r;
    let (west, east) = (
        matches!(h, Handle::W | Handle::NW | Handle::SW),
        matches!(h, Handle::E | Handle::NE | Handle::SE),
    );
    let (south, north) = (
        matches!(h, Handle::S | Handle::SE | Handle::SW),
        matches!(h, Handle::N | Handle::NE | Handle::NW),
    );
    if west {
        o[0] = snap(r[0] + dx, snapping).min(r[2] - MIN_BOX_IN);
    }
    if east {
        o[2] = snap(r[2] + dx, snapping).max(r[0] + MIN_BOX_IN);
    }
    if south {
        o[1] = snap(r[1] + dy, snapping).min(r[3] - MIN_BOX_IN);
    }
    if north {
        o[3] = snap(r[3] + dy, snapping).max(r[1] + MIN_BOX_IN);
    }
    o
}

/// `r` moved by `(dx, dy)`, its lower-left corner snapped when `snapping`.
pub fn moved(r: [f64; 4], dx: f64, dy: f64, snapping: bool) -> [f64; 4] {
    let (w, h) = (r[2] - r[0], r[3] - r[1]);
    let x = snap(r[0] + dx, snapping);
    let y = snap(r[1] + dy, snapping);
    [x, y, x + w, y + h]
}

// ---------------------------------------------------------------- state --

/// What a click or drag on the page does.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LayoutTool {
    /// Select, move and resize boxes and page CAD.
    #[default]
    Select,
    /// Layout CAD: drag a line.
    Line,
    /// Layout CAD: drag a rectangle.
    Box,
    /// Layout CAD: click the corners, double-click to finish.
    Polyline,
    /// Layout CAD: click where the text goes.
    Text,
    /// Drag the rectangle of a new text box.
    TextBox,
    /// Layout CAD: drag a circle from its centre.
    Circle,
    /// Layout CAD: click the centre, the start and the end of an arc.
    Arc,
    /// Drag from what the leader points at to where its text goes.
    Leader,
    /// Drag the rectangle a revision cloud goes around.
    Cloud,
    /// Pan/Scale Layout Box: drag to pan the contents of the selected box
    /// (not part of [`LayoutTool::ALL`]; it has its own edit button).
    PanScale,
    /// Edit Layout Lines: select, draw and delete the lines of a Plot Lines
    /// view.
    EditLines,
    /// Point to Point Move: click where the selection moves from, then where
    /// it moves to (object snaps apply).
    PointToPoint,
}

impl LayoutTool {
    pub const ALL: [LayoutTool; 10] = [
        LayoutTool::Select,
        LayoutTool::Line,
        LayoutTool::Box,
        LayoutTool::Polyline,
        LayoutTool::Circle,
        LayoutTool::Arc,
        LayoutTool::Text,
        LayoutTool::TextBox,
        LayoutTool::Leader,
        LayoutTool::Cloud,
    ];

    pub fn name(self) -> &'static str {
        match self {
            LayoutTool::Select => "Select",
            LayoutTool::Line => "Line",
            LayoutTool::Box => "Box",
            LayoutTool::Polyline => "Polyline",
            LayoutTool::Text => "Text",
            LayoutTool::TextBox => "Text Box",
            LayoutTool::Circle => "Circle",
            LayoutTool::Arc => "Arc",
            LayoutTool::Leader => "Leader",
            LayoutTool::Cloud => "Revision Cloud",
            LayoutTool::PanScale => "Pan/Scale Layout Box",
            LayoutTool::EditLines => "Edit Layout Lines",
            LayoutTool::PointToPoint => "Point to Point Move",
        }
    }
}

/// Everything the layout window needs to ask the application for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayoutCommand {
    /// File > Open Layout / Window > Layout: show the layout view.
    ShowLayout,
    /// Window > Floor Plan View: back to the plan.
    ShowPlan,
    SendToLayout,
    SendAllFloors,
    PageSetup,
    ProjectInfo,
    /// Tools > Layout > Layout Page Table: click a page to place the table.
    PageTable,
    /// Tools > Layout > Layout Revision Table: click a page to place the
    /// table of that page's revisions.
    RevisionTable,
    /// Tools > Layout > Add Layout Revision: one revision on chosen pages.
    AddLayoutRevision,
    /// Edit > Default Settings > Layout: General Layout Defaults (Use Snap
    /// Grid, Grid Snap Unit).
    LayoutDefaults,
    /// Copy the page's border and drawings (CAD, leaders, clouds) to
    /// another page.
    CopyDrawingsToPage,
    /// Drag the page at `.0` to the position `.1` (the Project Browser and
    /// the page tabs): the pages and their `#` labels renumber.
    MovePage(usize, usize),
    InsertPageBefore,
    InsertPageAfter,
    DuplicatePage,
    DeletePage,
    ExchangeWithPrevious,
    ExchangeWithNext,
    NextPage,
    PreviousPage,
    GoToPage(usize),
    BoxSpecification,
    DeleteBox,
    UpdateViews,
    FitPage,
    /// Print Layout: the layout's pages.
    Print,
    /// File > Print: the layout when its view is open, else the plan view.
    PrintDialog,
    /// File > Print > Print Image: the plan view as a PNG.
    PrintImage,
    ExportPdf,
    /// Add a text box (then double-click it to edit).
    AddTextBox,
    /// Add the Materials List as a table box.
    AddMaterialsBox,
    /// Add a picture file as a box.
    AddImageBox,
    /// Turn the selected box's content a quarter turn.
    RotateBox,
    /// Add the sheet index as a table box.
    AddSheetIndex,
    /// Layer Display Options of the layout.
    LayerDisplay,
    /// Send camera `id` (a Project Browser camera) to the layout: the Send to
    /// Layout dialog opens on it.
    SendCamera(Id),
    /// File > Print Model: a perspective camera rendered at a chosen DPI.
    PrintModel,
    /// Add Daniel's sheet set (cover, site, plans, elevations, sections,
    /// details, schedules) to the layout.
    CreateConstructionSet,
    /// Layout > Save As Template: store the layout under a name in
    /// `~/.plan-studio/templates`.
    SaveAsTemplate,
    /// Layout > Apply Template: replace the layout with a saved template.
    ApplyTemplate,
    /// Tools > Layout > Edit Page Information: Layout Page Information
    /// (label, title, description, comments, page templates, revisions, the
    /// page's own sheet size and title block).
    PageInformation,
    /// Layout > Customize Sheet Sizes: custom sizes and which standard sizes
    /// the lists show.
    CustomizeSheetSizes,
    /// Line the selected boxes up (a single box lines up with the drawing
    /// area).
    Align(AlignEdge),
    /// Spread three or more selected boxes with equal gaps.
    Distribute(Spread),
    /// Copy the selected boxes to another page.
    CopyBoxToPage,
    /// Copy the selected boxes on the same page.
    DuplicateBox,
    /// Go back to the view the selected box shows: a plan box opens its
    /// floor plan.
    OpenSourceView,
    /// Layout > New Layout File: the plan gets a second layout file and it
    /// opens.
    NewLayoutFile,
    /// Open the plan's `n`th layout file (0 is the open one).
    SwitchLayout(usize),
    /// Save the selected table box (or every table on the page) as CSV.
    ExportTableCsv,
    /// Save the selected table box (or every table on the page) as an Excel
    /// workbook, a sheet each.
    ExportTableExcel,
    /// Choose the tool that clicks and drags on the page use.
    Tool(LayoutTool),
    /// Send every floor plan and elevation camera, with one dialog (or one
    /// set of answers for all of them).
    SendAllViews,
    /// The Update View edit tool: the selected views.
    UpdateView,
    /// Tools > Layout > Update Layout Views > Update All Live Views.
    UpdateLiveViews,
    /// Update All Plot Line Views.
    UpdatePlotLineViews,
    /// The Rescale Layout View edit tool: the Change Scale dialog.
    RescaleView,
    /// Recenter Layout Box Contents.
    RecenterBox,
    /// Scale Layout Box Contents to Fit.
    ScaleBoxToFit,
    /// The Layout Box Layers edit tool: the box's layer set.
    LayoutBoxLayers,
    /// Unlink Saved Plan View: the box keeps the floor and layer set it shows.
    UnlinkSavedView,
    /// Center Object: the selection moves to the middle of the drawing area.
    CenterObject,
    Undo,
    Redo,
}

#[derive(Clone)]
enum Drag {
    Pan,
    Move {
        id: Id,
        start: (f64, f64),
        orig: [f64; 4],
        /// The other selected boxes and where they were: they move along.
        group: Vec<(Id, [f64; 4])>,
        /// The page drawings (CAD, leaders, clouds) selected with them move
        /// along too, from the annotations as the drag found them.
        cad: Vec<Id>,
        cad_orig: Box<LayoutPage>,
        before: Box<Layout>,
    },
    Resize {
        id: Id,
        handle: Handle,
        start: (f64, f64),
        orig: [f64; 4],
        before: Box<Layout>,
    },
    /// Drawing layout CAD or a text box: from `start` to `cur`, paper inches.
    Draw {
        start: (f64, f64),
        cur: (f64, f64),
    },
    /// Moving the selected page annotation. `orig` is the page's annotations
    /// as they were (boxes left out), `before` the whole layout for undo.
    MoveCad {
        id: Id,
        start: (f64, f64),
        orig: Box<LayoutPage>,
        /// The other selected page drawings.
        with_cad: Vec<Id>,
        /// The boxes selected with it and where they were.
        with_boxes: Vec<(Id, [f64; 4])>,
        before: Box<Layout>,
    },
    /// Pan/Scale Layout Box: panning the contents of box `id`.
    PanBox {
        id: Id,
        start: (f64, f64),
        pan0: (f64, f64),
        before: Box<Layout>,
    },
    /// Edit Layout Lines: drawing a new line in box `id` (view inches).
    DrawLine {
        id: Id,
        start: Point,
        cur: Point,
        before: Box<Layout>,
    },
    /// Edit Layout Lines: moving the selected lines from `start` (view
    /// inches), as `orig` had them.
    MoveLines {
        id: Id,
        start: Point,
        orig: Box<plan_layout::ViewArt>,
        before: Box<Layout>,
    },
    /// Edit Layout Lines: a marquee from `start` to `cur` (view inches).
    Marquee {
        id: Id,
        start: Point,
        cur: Point,
    },
    /// Resizing the selected page annotation by a handle.
    ResizeCad {
        id: Id,
        handle: Handle,
        start: (f64, f64),
        orig: Box<LayoutPage>,
        orig_bounds: [f64; 4],
        before: Box<Layout>,
    },
}

/// A Send to Layout waiting for the click that places it.
struct Placing {
    source: BoxSource,
    scale: Option<Scale>,
    /// The Send Options, scaling and camera link the dialog answered.
    details: Option<SendDetails>,
    page: PageChoice,
    /// The size of the box that will be placed, paper inches (the ghost).
    size: (f64, f64),
}

#[derive(Default)]
struct BoxCache {
    map: HashMap<Id, (u64, Rc<BoxArtwork>)>,
}

#[derive(Default)]
struct Dialogs {
    send: Option<SendToLayoutDialog>,
    spec: Option<BoxSpecDialog>,
    /// Layout Box Specification of a view box.
    box_view: Option<LayoutBoxDialog>,
    /// Layout Line Specification of the selected plot lines.
    line_spec: Option<LayoutLineDialog>,
    /// Rescale Layout View: Change Scale.
    change_scale: Option<ChangeScaleDialog>,
    setup: Option<PageSetupDialog>,
    print: Option<PrintDialog>,
    image: Option<ImageDialog>,
    text_box: Option<TextBoxDialog>,
    cad_text: Option<CadTextDialog>,
    leader: Option<LeaderDialog>,
    cloud: Option<CloudDialog>,
    layers: Option<LayoutLayersDialog>,
    model: Option<ModelDialog>,
    /// Save As Template and Apply Template.
    template: Option<LayoutTemplateDialog>,
    page_info: Option<PageInfoDialog>,
    /// Copy Drawings to Page: the dialog and the page they come from.
    copy_drawings: Option<CopyBoxDialog>,
    /// Add Layout Revision.
    revision: Option<RevisionDialog>,
    /// General Layout Defaults.
    defaults: Option<LayoutDefaultsDialog>,
    sheet_sizes: Option<SheetSizesDialog>,
    copy_box: Option<CopyBoxDialog>,
    new_layout: Option<NameDialog>,
    /// Print Preview: the pages as they will print.
    preview: Option<PrintPreviewDialog>,
    /// Print Image of the 3D view: the dialog and the view it will render.
    image3d: Option<(Image3dDialog, crate::shell::view3d_panel::Snapshot3d)>,
}

impl Dialogs {
    fn any(&self) -> bool {
        self.send.is_some()
            || self.spec.is_some()
            || self.box_view.is_some()
            || self.line_spec.is_some()
            || self.change_scale.is_some()
            || self.setup.is_some()
            || self.print.is_some()
            || self.image.is_some()
            || self.text_box.is_some()
            || self.cad_text.is_some()
            || self.leader.is_some()
            || self.cloud.is_some()
            || self.layers.is_some()
            || self.model.is_some()
            || self.template.is_some()
            || self.page_info.is_some()
            || self.copy_drawings.is_some()
            || self.revision.is_some()
            || self.defaults.is_some()
            || self.sheet_sizes.is_some()
            || self.copy_box.is_some()
            || self.new_layout.is_some()
            || self.preview.is_some()
            || self.image3d.is_some()
    }
}

/// The layout window's state.
pub struct LayoutView {
    /// The layout view replaces the plan canvas.
    pub active: bool,
    layout: Option<Layout>,
    /// The JSON last read from or written to the project.
    stored: Option<serde_json::Value>,
    /// Index of the page shown (into `Layout::pages`).
    pub page: usize,
    pub selected: Option<Id>,
    /// The other boxes selected with Shift (Align, Distribute, Copy work on
    /// them together with [`selected`](Self::selected)).
    also: Vec<Id>,
    /// The other page drawings (CAD, leaders, clouds) selected with Shift,
    /// besides [`selected_cad`](Self::selected_cad); they move with the boxes.
    also_cad: Vec<Id>,
    /// Undo steps made since the last [`flush`](Self::flush): the label and
    /// the project as it was before the edit.
    steps: Vec<(String, Project)>,
    /// What the unit tests undo through (they have no editor context).
    #[cfg(test)]
    test_history: crate::editor::history::ChangeHistory,
    /// Pixels per paper inch.
    zoom: f32,
    /// Screen offset of the sheet's top-left from the view's top-left.
    offset: Vec2,
    fit_pending: bool,
    drag: Option<Drag>,
    placing: Option<Placing>,
    renaming: Option<(usize, String)>,
    cache: BoxCache,
    sig: u64,
    dialogs: Dialogs,
    /// The paper-to-screen mapping of the last frame (hit tests, tests).
    last_xf: Option<Xf>,
    /// The layout sheet the plan's Drawing Sheet was last matched to.
    synced_sheet: Option<plan_docs::SheetSize>,
    /// What clicks and drags on the page do.
    pub tool: LayoutTool,
    /// The selected page CAD object.
    pub selected_cad: Option<Id>,
    /// The corners of the polyline being drawn, paper inches (and the centre
    /// and start of an arc being drawn).
    poly: Vec<Point>,
    /// Pictures and renders on screen, per box: the key they were made for.
    textures: HashMap<Id, (u64, egui::TextureHandle)>,
    /// Update Views rendering perspective views on a thread.
    update: Option<UpdateJob>,
    /// The 3D view offered to the open Send to Layout dialog (sent as a
    /// picture when the dialog asks).
    snapshot_src: Option<crate::shell::view3d_panel::Snapshot3d>,
    /// Edit Layout Lines: the selected lines of the selected box.
    line_sel: Vec<Id>,
    /// Pan/Scale Layout Box: the scale being typed.
    pan_scale_text: String,
    /// Views waiting for their Send to Layout dialog (Send All Views).
    send_queue: Vec<SendSource>,
    /// What the last send made (the warning when it was too big).
    last_sent: Option<Sent>,
}

/// Update Views in progress: perspective renders on a thread, with a count
/// the progress window reads.
struct UpdateJob {
    rx: std::sync::mpsc::Receiver<usize>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
    total: usize,
    done: usize,
}

impl Default for LayoutView {
    fn default() -> Self {
        Self {
            active: false,
            layout: None,
            stored: None,
            page: 0,
            selected: None,
            also: Vec::new(),
            also_cad: Vec::new(),
            steps: Vec::new(),
            #[cfg(test)]
            test_history: crate::editor::history::ChangeHistory::new(),
            zoom: 20.0,
            offset: Vec2::ZERO,
            fit_pending: true,
            drag: None,
            placing: None,
            renaming: None,
            cache: BoxCache::default(),
            sig: 0,
            dialogs: Dialogs::default(),
            last_xf: None,
            synced_sheet: None,
            tool: LayoutTool::Select,
            selected_cad: None,
            poly: Vec::new(),
            textures: HashMap::new(),
            update: None,
            snapshot_src: None,
            line_sel: Vec::new(),
            pan_scale_text: String::new(),
            send_queue: Vec::new(),
            last_sent: None,
        }
    }
}

/// Names the state of the plan that plan views, elevations and camera
/// drawings were drawn from: it differs after every change signal of the
/// editor context (`EditorContext::cache_key`), so the box caches are dropped
/// exactly when the plan may have changed. (This used to serialize the floors,
/// layers, cameras and terrain to JSON and hash the text every frame, which
/// costs tens of milliseconds on a real house.)
fn project_sig(cx: &EditorContext) -> u64 {
    let mut h = DefaultHasher::new();
    cx.cache_key().hash(&mut h);
    h.finish()
}

fn box_key(b: &LayoutBox, sig: u64) -> u64 {
    let mut h = DefaultHasher::new();
    sig.hash(&mut h);
    for p in [b.rect_in.0, b.rect_in.1] {
        p.x.to_bits().hash(&mut h);
        p.y.to_bits().hash(&mut h);
    }
    b.scale.hash(&mut h);
    b.border.hash(&mut h);
    b.clip.hash(&mut h);
    b.line_weight_scale.to_bits().hash(&mut h);
    b.hatch_materials.hash(&mut h);
    b.rotation_deg.to_bits().hash(&mut h);
    b.label.hash(&mut h);
    b.view.cache_key().hash(&mut h);
    match &b.source {
        BoxSource::ImageData {
            width,
            height,
            rgba,
        } => (width, height, rgba.len()).hash(&mut h),
        other => format!("{other:?}").hash(&mut h),
    }
    h.finish()
}

/// A new layout called `name` from Daniel's template; a template saved as the
/// default for its sheet size takes over.
fn fresh_layout(name: &str, seed: Option<&crate::templates::LayoutInfoSeed>) -> Layout {
    let mut layout = crate::templates::new_layout(name, seed);
    if let Some(t) = layout_templates_dir().and_then(|d| default_layout_template(&d, layout.sheet))
    {
        layout = t.instantiate(name);
    }
    layout
}

fn json_layout_name(v: &serde_json::Value) -> String {
    v.get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("Layout")
        .to_string()
}

/// The names of the plan's layout files, the open one first, then the ones
/// parked in `Project::layout_files`.
pub fn layout_names(project: &Project) -> Vec<String> {
    project
        .layout
        .iter()
        .chain(project.layout_files.iter())
        .map(json_layout_name)
        .collect()
}

/// The layout files parked in the plan, as `(index for
/// [`LayoutCommand::SwitchLayout`], name)`; the open layout is index 0 and
/// is not listed.
pub fn parked_layouts(project: &Project) -> Vec<(usize, String)> {
    project
        .layout_files
        .iter()
        .enumerate()
        .map(|(i, v)| (i + 1, json_layout_name(v)))
        .collect()
}

/// The Drawing Scale the plan's Drawing Sheet Setup gives its plan view, when
/// the plan has set one: where Send to Layout starts (manual p. 1426).
fn sheet_scale(project: &Project) -> Option<Scale> {
    use plan_core::drawing_sheet::ViewType;
    project
        .print_setup
        .has_own(ViewType::Plan)
        .then(|| crate::dialogs::drawing_sheet::default_scale(project, ViewType::Plan))
}

/// The scaling and options of a send that only knows a [`SendSpec`]: the
/// scale asked for (else the largest that fits, up to 1/4"), a Live View,
/// the whole view.
pub fn default_details(spec: &SendSpec) -> SendDetails {
    SendDetails {
        scale: match spec.scale {
            Some(s) => SendScale::Named(s),
            None => SendScale::Largest(AUTO_SCALE_CEILING),
        },
        options: SendOptions::default(),
        snap_to_point: false,
        show_page: true,
        all_remaining: false,
        as_image: false,
    }
}

impl LayoutView {
    // ----- model access -----

    /// Re-reads the layout when the project's JSON changed under us (a file
    /// was opened, or the plan's undo restored an older snapshot).
    pub fn sync(&mut self, project: &Project) {
        if project.layout == self.stored {
            return;
        }
        self.layout = load(project);
        self.stored = project.layout.clone();
        self.steps.clear();
        #[cfg(test)]
        self.test_history.clear();
        self.cache.map.clear();
        self.textures.clear();
        self.drag = None;
        self.placing = None;
        self.poly.clear();
        self.selected_cad = None;
        self.also.clear();
        self.also_cad.clear();
        self.line_sel.clear();
        self.clamp_page();
    }

    pub fn layout(&self) -> Option<&Layout> {
        self.layout.as_ref()
    }

    pub fn current_page(&self) -> Option<&LayoutPage> {
        self.layout.as_ref()?.pages.get(self.page)
    }

    fn clamp_page(&mut self) {
        let n = self.layout.as_ref().map_or(0, |l| l.pages.len());
        self.page = self.page.min(n.saturating_sub(1));
        if let Some(id) = self.selected {
            let there = self
                .current_page()
                .is_some_and(|p| p.boxes.iter().any(|b| b.id == id));
            if !there {
                self.selected = None;
            }
        }
        let boxes: Vec<Id> = self
            .current_page()
            .map(|p| p.boxes.iter().map(|b| b.id).collect())
            .unwrap_or_default();
        self.also.retain(|id| boxes.contains(id));
        if self.selected.is_none() {
            self.selected = self.also.pop();
        }
    }

    fn write_back(&mut self, project: &mut Project) {
        if let Some(l) = &self.layout {
            store(project, l);
            self.stored = project.layout.clone();
        }
    }

    /// Parks the project as it was before an edit that turned the layout
    /// `before` into the current one (the undo step `label`).
    fn record(&mut self, project: &Project, label: &str, before: &Layout) {
        let mut snapshot = project.clone();
        snapshot.layout = serde_json::to_value(before).ok();
        self.steps.push((label.to_string(), snapshot));
    }

    /// Hands the parked undo steps to the editor's history, oldest first.
    pub fn flush(&mut self, cx: &mut EditorContext) {
        for (label, before) in self.steps.drain(..) {
            cx.record_undo_step(&before, &label);
        }
    }

    /// Replaces the layout with `new`, recording the step for undo.
    fn commit(&mut self, project: &mut Project, label: &str, new: Layout) {
        if let Some(old) = self.layout.take() {
            self.record(project, label, &old);
        }
        self.layout = Some(new);
        self.write_back(project);
        self.clamp_page();
    }

    /// Runs `f` on a copy of the layout; when it returns true the copy
    /// becomes the layout, as the undo step `label`.
    pub fn edit(
        &mut self,
        project: &mut Project,
        label: &str,
        f: impl FnOnce(&mut Layout) -> bool,
    ) -> bool {
        let Some(mut copy) = self.layout.clone() else {
            return false;
        };
        if !f(&mut copy) {
            return false;
        }
        self.commit(project, label, copy);
        true
    }

    /// Steps back through the unit tests' stand-in for the editor history.
    #[cfg(test)]
    pub fn undo(&mut self, project: &mut Project) -> Option<String> {
        self.drain_into_test_history();
        let label = self.test_history.undo(project);
        self.reload_after_restore(project);
        label
    }

    #[cfg(test)]
    pub fn redo(&mut self, project: &mut Project) -> Option<String> {
        self.drain_into_test_history();
        let label = self.test_history.redo(project);
        self.reload_after_restore(project);
        label
    }

    #[cfg(test)]
    fn reload_after_restore(&mut self, project: &Project) {
        self.layout = load(project);
        self.stored = project.layout.clone();
        self.cache.map.clear();
        self.clamp_page();
    }

    #[cfg(test)]
    fn drain_into_test_history(&mut self) {
        for (label, before) in self.steps.drain(..) {
            self.test_history.begin(&before, &label);
        }
    }

    #[cfg(test)]
    pub fn undo_label(&self) -> Option<&str> {
        self.steps
            .last()
            .map(|(l, _)| l.as_str())
            .or_else(|| self.test_history.undo_label())
    }

    // ----- creating -----

    /// Makes the project's layout from the template (Daniel's title block, a
    /// template page 0 and an empty page 1) unless it has one. True when a
    /// layout was created.
    pub fn create(
        &mut self,
        project: &mut Project,
        seed: Option<&crate::templates::LayoutInfoSeed>,
    ) -> bool {
        self.sync(project);
        if self.layout.is_some() {
            return false;
        }
        let layout = fresh_layout(&format!("{} Layout", project.name), seed);
        // Making the layout is an undo step too, so an older plan edit's
        // undo cannot drop it unseen.
        self.steps.push(("New Layout".to_string(), project.clone()));
        self.layout = Some(layout);
        self.write_back(project);
        // %date% needs a date; fill it in when Project Information has none.
        if project.info.date.trim().is_empty() {
            project.info.date = today();
        }
        self.page = self
            .layout
            .as_ref()
            .and_then(|l| l.pages.iter().position(|p| !p.template_page))
            .unwrap_or(0);
        self.selected = None;
        self.fit_pending = true;
        true
    }

    // ----- pages -----

    pub fn set_page(&mut self, index: usize) {
        let n = self.layout.as_ref().map_or(0, |l| l.pages.len());
        if index < n && index != self.page {
            let before = self.sheet_of(self.page);
            self.page = index;
            self.selected = None;
            self.also.clear();
            self.line_sel.clear();
            self.renaming = None;
            if self.sheet_of(index) != before {
                self.fit_pending = true;
            }
        }
    }

    pub fn add_page(&mut self, project: &mut Project, before: bool) -> bool {
        let at = self.page;
        let mut new_index = at;
        let done = self.edit(project, "Insert Page", |l| {
            new_index = insert_page(l, at, before);
            true
        });
        if done {
            self.set_page(new_index);
        }
        done
    }

    pub fn duplicate_current_page(&mut self, project: &mut Project) -> bool {
        let at = self.page;
        let mut new_index = None;
        let done = self.edit(project, "Duplicate Page", |l| {
            new_index = duplicate_page(l, at);
            new_index.is_some()
        });
        if let Some(i) = new_index.filter(|_| done) {
            self.set_page(i);
        }
        done
    }

    pub fn delete_current_page(&mut self, project: &mut Project) -> Result<(), &'static str> {
        let at = self.page;
        let mut result = Ok(());
        self.edit(project, "Delete Page", |l| {
            result = delete_page(l, at);
            result.is_ok()
        });
        self.clamp_page();
        self.selected = None;
        result
    }

    pub fn exchange_current_page(&mut self, project: &mut Project, forward: bool) -> bool {
        let at = self.page;
        let mut new_index = None;
        let done = self.edit(project, "Exchange Pages", |l| {
            new_index = exchange_page(l, at, forward);
            new_index.is_some()
        });
        if let Some(i) = new_index.filter(|_| done) {
            self.page = i;
        }
        done
    }

    pub fn rename_page(&mut self, project: &mut Project, index: usize, title: &str) -> bool {
        let title = title.trim().to_string();
        self.edit(project, "Rename Page", |l| {
            match l.pages.get_mut(index).filter(|p| p.title != title) {
                Some(p) => {
                    p.title = title;
                    true
                }
                None => false,
            }
        })
    }

    /// Save As Template: writes the layout as a template named in the
    /// window. Returns what to tell the user.
    pub fn save_template(&self, name: &str, default_for_sheet: bool) -> String {
        let Some(layout) = &self.layout else {
            return "There is no layout to save".into();
        };
        let Some(dir) = layout_templates_dir() else {
            return format!("Could not save the template: {}", crate::paths::NO_HOME);
        };
        let template = plan_layout::LayoutTemplate::new(name, layout, default_for_sheet);
        match save_layout_template(&dir, &template) {
            Ok(file) => format!(
                "Saved the layout template \"{}\" to {}",
                template.name,
                file.display()
            ),
            Err(e) => format!("Could not save the template: {e}"),
        }
    }

    /// Replaces the layout with `template` (one undo step). The layout keeps
    /// its name; the page shown is the first printed page.
    pub fn apply_template(
        &mut self,
        project: &mut Project,
        template: &plan_layout::LayoutTemplate,
    ) -> bool {
        let name = self
            .layout
            .as_ref()
            .map_or_else(String::new, |l| l.name.clone());
        let done = self.edit(project, "Apply Layout Template", |l| {
            *l = template.instantiate(&name);
            true
        });
        if done {
            self.page = self
                .layout
                .as_ref()
                .and_then(|l| l.pages.iter().position(|p| !p.template_page))
                .unwrap_or(0);
            self.selected = None;
            self.selected_cad = None;
            self.fit_pending = true;
        }
        done
    }

    /// OK in the template window: save, or apply, as it was opened for.
    fn finish_template(&mut self, project: &mut Project, d: &LayoutTemplateDialog) -> String {
        match d.mode() {
            TemplateMode::Save => self.save_template(&d.name, d.default_for_sheet),
            TemplateMode::Apply => match d.picked() {
                Some(t) if self.apply_template(project, t) => {
                    format!("Applied the layout template \"{}\"", t.name)
                }
                _ => "The layout was not changed".into(),
            },
        }
    }

    /// Layout Page Information on the page shown: every page of the layout
    /// can be edited from it (the Selected Page list).
    pub fn page_info_dialog(&self, project: &Project) -> Option<PageInfoDialog> {
        let l = self.layout.as_ref()?;
        Some(PageInfoDialog::new(
            l,
            self.page,
            &today(),
            &macro_context(project).designer,
        ))
    }

    /// Applies what Layout Page Information edited as one undo step: label,
    /// title, description, comments, page templates, revisions and each
    /// page's own sheet and title block. `Ok(false)` when nothing changed;
    /// the error says why the edit was refused (nothing is changed then).
    pub fn apply_page_info(
        &mut self,
        project: &mut Project,
        d: &PageInfoDialog,
    ) -> Result<bool, String> {
        let mut refused = None;
        let changed = self.edit(project, "Page Information", |l| {
            let before = l.clone();
            if let Err(e) = l.set_page_infos(&d.infos()) {
                refused = Some(e);
                return false;
            }
            for e in d.entries() {
                if let Some(i) = l.pages.iter().position(|p| p.number == e.number) {
                    l.pages[i].no_title_block = e.sheet.no_title_block;
                    l.set_page_sheet(i, e.sheet.sheet.as_ref(), e.sheet.portrait);
                }
            }
            *l != before
        });
        match refused {
            Some(e) => Err(e),
            None => Ok(changed),
        }
    }

    /// Add Layout Revision: `revision` goes on every page in `numbers`, one
    /// undo step.
    pub fn add_layout_revision(
        &mut self,
        project: &mut Project,
        numbers: &[u32],
        revision: &plan_layout::PageRevision,
    ) -> bool {
        self.edit(project, "Add Layout Revision", |l| {
            l.add_revision(numbers, revision) > 0
        })
    }

    /// General Layout Defaults, one undo step.
    pub fn apply_layout_defaults(&mut self, project: &mut Project, d: &LayoutDefaults) -> bool {
        let changed = self.edit(project, "General Layout Defaults", |l| {
            let changed =
                l.snap_grid != d.snap_grid || (l.snap_unit_in - d.snap_unit_in).abs() > 1e-12;
            l.snap_grid = d.snap_grid;
            l.snap_unit_in = d
                .snap_unit_in
                .clamp(plan_layout::MIN_SNAP_UNIT_IN, plan_layout::MAX_SNAP_UNIT_IN);
            changed
        });
        self.sync_snap();
        changed
    }

    /// Makes the open layout's snap settings the ones drags and the arrow
    /// keys use.
    pub fn sync_snap(&self) {
        let v = self
            .layout
            .as_ref()
            .map_or((true, SNAP_IN), |l| (l.snap_grid, l.snap_unit_in));
        SNAP.with(|c| c.set(v));
    }

    /// Drags page `from` to position `to` (an undo step); every `#` label
    /// follows. Returns whether anything moved.
    pub fn move_page_to(&mut self, project: &mut Project, from: usize, to: usize) -> bool {
        // Moving renumbers every page, so the page shown is followed by its
        // position, not its number.
        let shown = self.page;
        let len = self.layout.as_ref().map_or(0, |l| l.pages.len());
        let moved = self.edit(project, "Move Page", |l| {
            from != to && from < l.pages.len() && l.move_page(from, to).is_some()
        });
        if moved && len > 0 {
            let to = to.min(len - 1);
            self.page = if shown == from {
                to
            } else if from < shown && shown <= to {
                shown - 1
            } else if to <= shown && shown < from {
                shown + 1
            } else {
                shown
            };
        }
        moved
    }

    /// Copy Drawings to Page: the page shown gives its border and drawings
    /// (CAD, leaders, clouds) to the page numbered `to`, one undo step.
    /// Returns how many were copied.
    pub fn copy_drawings_to(&mut self, project: &mut Project, to: u32) -> usize {
        let from = self.page;
        let mut n = 0;
        self.edit(project, "Copy Drawings to Page", |l| {
            let Some(t) = l.pages.iter().position(|p| p.number == to) else {
                return false;
            };
            n = l.copy_page_drawings(from, t);
            n > 0
        });
        n
    }

    // ----- page setup and project information -----

    pub fn page_setup(&self) -> Option<PageSetup> {
        let l = self.layout.as_ref()?;
        Some(PageSetup {
            sheet: l.sheet_choice(),
            margins_in: l.margins_in,
            page_background: l.page_background,
            edge_line_weight: l.edge_line_weight,
            sheet_index: l.sheet_index,
            portrait: l.portrait,
        })
    }

    pub fn apply_page_setup(&mut self, project: &mut Project, s: &PageSetup) -> bool {
        self.edit(project, "Page Setup", |l| {
            let changed = l.sheet_choice() != s.sheet
                || (l.margins_in - s.margins_in).abs() > 1e-9
                || l.page_background != s.page_background
                || l.edge_line_weight != s.edge_line_weight
                || l.sheet_index != s.sheet_index
                || l.portrait != s.portrait;
            l.portrait = s.portrait;
            l.set_sheet_choice(&s.sheet);
            l.margins_in = s.margins_in;
            l.page_background = s.page_background;
            l.edge_line_weight = s.edge_line_weight;
            l.sheet_index = s.sheet_index;
            changed
        })
    }

    // ----- page information, sheet sizes, arranging boxes (L-6, L-7, L-8) -----

    /// The sheet page `index` prints on, paper inches: its own size from
    /// Page Information, else the layout's.
    fn sheet_of(&self, index: usize) -> (f64, f64) {
        match &self.layout {
            Some(l) => l
                .pages
                .get(index)
                .map_or_else(|| l.sheet_inches(), |p| l.page_sheet_inches(p)),
            None => (36.0, 24.0),
        }
    }

    /// The layout's Customize Sheet Sizes answers.
    pub fn sheet_sizes(&self) -> Option<SheetSizes> {
        let l = self.layout.as_ref()?;
        Some(SheetSizes {
            custom: l.custom_sizes.clone(),
            hidden: l.hidden_sizes.clone(),
        })
    }

    /// Stores Customize Sheet Sizes as one undo step.
    pub fn apply_sheet_sizes(&mut self, project: &mut Project, s: &SheetSizes) -> bool {
        self.edit(project, "Customize Sheet Sizes", |l| {
            let changed = l.custom_sizes != s.custom || l.hidden_sizes != s.hidden;
            l.custom_sizes = s.custom.clone();
            l.hidden_sizes = s.hidden.clone();
            changed
        })
    }

    /// The selected boxes: the one clicked first, then the ones added with
    /// Shift.
    pub fn selection_ids(&self) -> Vec<Id> {
        let mut v: Vec<Id> = self.selected.into_iter().collect();
        v.extend(
            self.also
                .iter()
                .copied()
                .filter(|i| Some(*i) != self.selected),
        );
        v
    }

    /// Copies the selected boxes on the page shown, offset a little.
    pub fn duplicate_here(&mut self, project: &mut Project) -> usize {
        match self.current_page().map(|p| p.number) {
            Some(here) => self.copy_selected_to(project, here),
            None => 0,
        }
    }

    /// Selects `ids` on the page shown.
    pub fn select_boxes(&mut self, ids: &[Id]) {
        self.selected = ids.first().copied();
        self.also = ids.iter().skip(1).copied().collect();
        self.selected_cad = None;
        self.also_cad.clear();
    }

    /// The selected page drawings: the one clicked first, then the ones added
    /// with Shift.
    pub fn cad_selection_ids(&self) -> Vec<Id> {
        let mut v: Vec<Id> = self.selected_cad.into_iter().collect();
        v.extend(
            self.also_cad
                .iter()
                .copied()
                .filter(|i| Some(*i) != self.selected_cad),
        );
        v
    }

    /// Shift-click on a page drawing: adds it to the selection (the boxes
    /// stay selected), or takes it out when it is in already.
    pub fn extend_cad_selection(&mut self, id: Id) {
        let mut ids = self.cad_selection_ids();
        match ids.iter().position(|i| *i == id) {
            Some(at) => {
                ids.remove(at);
            }
            None => ids.push(id),
        }
        self.selected_cad = ids.first().copied();
        self.also_cad = ids.iter().skip(1).copied().collect();
    }

    /// Moves the boxes `boxes` (with the places they started at) and the
    /// page drawings `cad` (from `cad_orig`, the annotations before the move)
    /// by `(dx, dy)` paper inches, without a history step (mid-drag).
    fn move_group_live(
        &mut self,
        project: &mut Project,
        boxes: &[(Id, [f64; 4])],
        cad: &[Id],
        cad_orig: &LayoutPage,
        (dx, dy): (f64, f64),
    ) {
        for (id, r) in boxes {
            self.live_bounds(project, *id, [r[0] + dx, r[1] + dy, r[2] + dx, r[3] + dy]);
        }
        if !cad.is_empty() {
            self.live_annotation(project, cad_orig, |p| {
                for id in cad {
                    p.move_annotation(*id, dx, dy);
                }
            });
        }
    }

    /// Lines the selected boxes up on `edge` as one undo step. A single box
    /// lines up with the page's drawing area. Returns how many moved.
    pub fn align_selected(&mut self, project: &mut Project, edge: AlignEdge) -> usize {
        let ids = self.selection_ids();
        let at = self.page;
        let mut moved = 0;
        self.edit(project, edge.label(), |l| {
            let to = (ids.len() == 1).then(|| {
                let (lo, hi) = l.page_drawing_area(&l.pages[at]);
                [lo.x, lo.y, hi.x, hi.y]
            });
            moved = plan_layout::align_boxes(&mut l.pages[at], &ids, edge, to);
            moved > 0
        });
        moved
    }

    /// Spreads three or more selected boxes with equal gaps as one undo
    /// step. Returns how many moved.
    pub fn distribute_selected(&mut self, project: &mut Project, axis: Spread) -> usize {
        let ids = self.selection_ids();
        let at = self.page;
        let mut moved = 0;
        let label = match axis {
            Spread::Horizontal => "Distribute Boxes Horizontally",
            Spread::Vertical => "Distribute Boxes Vertically",
        };
        self.edit(project, label, |l| {
            moved = plan_layout::distribute_boxes(&mut l.pages[at], &ids, axis);
            moved > 0
        });
        moved
    }

    /// Copies the selected boxes onto the page numbered `to` as one undo
    /// step and selects the copies (on the page shown, when that is `to`).
    /// Returns how many were copied.
    pub fn copy_selected_to(&mut self, project: &mut Project, to: u32) -> usize {
        let ids = self.selection_ids();
        let Some(from) = self.current_page().map(|p| p.number) else {
            return 0;
        };
        let mut made = Vec::new();
        self.edit(project, "Copy Layout Boxes", |l| {
            made = plan_layout::copy_boxes(l, from, &ids, to);
            !made.is_empty()
        });
        if !made.is_empty() {
            if let Some(i) = self
                .layout
                .as_ref()
                .and_then(|l| l.pages.iter().position(|p| p.number == to))
            {
                self.set_page(i);
            }
            self.select_boxes(&made);
        }
        made.len()
    }

    // ----- the plan's layout files (L-2) -----

    /// Reads the open layout again after the project's JSON changed.
    fn reload_layout(&mut self, project: &Project) {
        self.layout = load(project);
        self.stored = project.layout.clone();
        self.cache.map.clear();
        self.textures.clear();
        self.drag = None;
        self.placing = None;
        self.poly.clear();
        self.selected = None;
        self.also.clear();
        self.also_cad.clear();
        self.selected_cad = None;
        self.page = self
            .layout
            .as_ref()
            .and_then(|l| l.pages.iter().position(|p| !p.template_page))
            .unwrap_or(0);
        self.fit_pending = true;
    }

    /// Opens layout file `index` of [`layout_names`] (0 is the open one):
    /// the open layout is parked and the other takes its place. No history.
    fn swap_in(&mut self, project: &mut Project, index: usize) -> bool {
        if index == 0 || index > project.layout_files.len() {
            return false;
        }
        let incoming = project.layout_files.remove(index - 1);
        if let Some(current) = project.layout.replace(incoming) {
            project.layout_files.insert(index - 1, current);
        }
        self.reload_layout(project);
        true
    }

    /// Opens another of the plan's layout files as one undo step.
    pub fn switch_layout(&mut self, project: &mut Project, index: usize) -> bool {
        if index == 0 || index > project.layout_files.len() {
            return false;
        }
        self.steps
            .push(("Open Layout".to_string(), project.clone()));
        self.swap_in(project, index)
    }

    /// Makes a new layout file called `name` from Daniel's template and
    /// opens it; the layout that was open is parked. One undo step. `false`
    /// when the name is empty or taken.
    pub fn new_layout_file(
        &mut self,
        project: &mut Project,
        name: &str,
        seed: Option<&crate::templates::LayoutInfoSeed>,
    ) -> bool {
        let name = name.trim();
        if name.is_empty()
            || layout_names(project)
                .iter()
                .any(|n| n.trim().eq_ignore_ascii_case(name))
        {
            return false;
        }
        self.steps
            .push(("New Layout File".to_string(), project.clone()));
        if let Some(current) = project.layout.take() {
            project.layout_files.insert(0, current);
        }
        let layout = fresh_layout(name, seed);
        store(project, &layout);
        if project.info.date.trim().is_empty() {
            project.info.date = today();
        }
        self.reload_layout(project);
        true
    }

    /// Sends `spec` to the layout file `target` names, as one undo step. A
    /// file other than the open one is opened (the open one parked); the box
    /// goes to a new page there, in the first free spot.
    pub fn send_to(
        &mut self,
        project: &mut Project,
        spec: &SendSpec,
        target: &LayoutTarget,
        snapshot: Option<(SnapshotSpec, crate::shell::view3d_panel::Snapshot3d)>,
        center: Option<(f64, f64)>,
    ) -> Result<Id, String> {
        let details = default_details(spec);
        self.send_to_with(project, spec, &details, target, snapshot, center)
    }

    /// [`send_to`](Self::send_to) with the Send Options, scaling and camera
    /// link the dialog answered.
    pub fn send_to_with(
        &mut self,
        project: &mut Project,
        spec: &SendSpec,
        details: &SendDetails,
        target: &LayoutTarget,
        snapshot: Option<(SnapshotSpec, crate::shell::view3d_panel::Snapshot3d)>,
        center: Option<(f64, f64)>,
    ) -> Result<Id, String> {
        let other = !matches!(target, LayoutTarget::Current);
        let before = other.then(|| project.clone());
        let marks = self.steps.len();
        match target {
            LayoutTarget::Current => {}
            LayoutTarget::Existing(name) => {
                let i = layout_names(project)
                    .iter()
                    .position(|n| n == name)
                    .ok_or_else(|| format!("There is no layout file called {name}"))?;
                self.swap_in(project, i);
            }
            LayoutTarget::New(name) => {
                let settings = crate::templates::load_settings();
                let seed = crate::templates::refresh(&settings, false);
                if !self.new_layout_file(project, name, seed.cache.layout.as_ref()) {
                    return Err("That layout file name is empty or taken".into());
                }
                self.steps.truncate(marks);
            }
        }
        let mut spec = spec.clone();
        if other {
            // The pages named in the dialog belong to the open layout.
            spec.page = PageChoice::New;
            if spec.placement == Placement::Click {
                spec.placement = Placement::FirstFree;
            }
        }
        let result = match snapshot {
            Some((snap, view)) => self.send_snapshot(project, &view, snap, spec.page),
            None => self.send_with(project, &spec, details, center),
        };
        if let Some(before) = before {
            match &result {
                // One step for the whole action, from before the switch.
                Ok(_) => {
                    self.steps.truncate(marks);
                    self.steps.push(("Send to Layout".to_string(), before));
                }
                Err(_) => {
                    self.steps.truncate(marks);
                    *project = before;
                    self.reload_layout(project);
                }
            }
        }
        result
    }

    /// Sends a picture of the 3D view as it is now: the view is ray traced
    /// at the box's size and quality and embedded as a picture box (it does
    /// not change when the plan or the camera does).
    fn send_snapshot(
        &mut self,
        project: &mut Project,
        view: &crate::shell::view3d_panel::Snapshot3d,
        snap: SnapshotSpec,
        page: PageChoice,
    ) -> Result<Id, String> {
        let Some(mut layout) = self.layout.clone() else {
            return Err("There is no layout to send to".into());
        };
        if view.scene.meshes.is_empty() {
            return Err("The 3D view has nothing to send".into());
        }
        let w_in = snap.width_in.clamp(1.0, 60.0);
        let h_in = w_in * 0.75;
        let (px_w, px_h) = plan_layout::perspective_pixels(w_in, h_in, snap.dpi);
        let img = render_snapshot(project, view, px_w, px_h, snap.samples);
        let page_no = match page {
            PageChoice::Existing(n) if layout.page(n).is_some() => n,
            _ => {
                let n = next_page_number(&layout);
                layout.add_page(n, "3D View");
                n
            }
        };
        let source = BoxSource::ImageData {
            width: img.width,
            height: img.height,
            rgba: img.rgba,
        };
        let rcx = render_context(project);
        let id = plan_layout::send_to_layout_sized(
            &mut layout,
            &rcx,
            page_no,
            source,
            Scale::QuarterInch,
            None,
            Some((w_in, h_in)),
        );
        drop(rcx);
        if let Some(b) = layout
            .page_mut(page_no)
            .and_then(|p| p.boxes.iter_mut().find(|b| b.id == id))
        {
            b.label = Some("3D VIEW".to_string());
        }
        self.commit(project, "Send 3D View to Layout", layout);
        if let Some(i) = self
            .layout
            .as_ref()
            .and_then(|l| l.pages.iter().position(|p| p.number == page_no))
        {
            self.page = i;
        }
        self.selected = Some(id);
        Ok(id)
    }

    // ----- tables to Excel and CSV (L-32, L-37) -----

    /// The tables of the selected table box, else of every table box on the
    /// page shown: door, window, room and wall schedules, placed schedules,
    /// the Materials List and the sheet index.
    pub fn page_tables(&self, project: &Project) -> Vec<plan_docs::Schedule> {
        let (Some(layout), Some(page)) = (self.layout.as_ref(), self.current_page()) else {
            return Vec::new();
        };
        let rcx = render_context(project);
        rcx.set_sheet_index(layout);
        let chosen: Vec<&LayoutBox> = match self.selected_box() {
            Some(b) => vec![b],
            None => page.boxes.iter().collect(),
        };
        chosen
            .into_iter()
            .filter_map(|b| plan_layout::box_table(b, &rcx))
            .collect()
    }

    /// The file for [`page_tables`](Self::page_tables): CSV text, or an Excel
    /// workbook with a sheet per table. `(file name, bytes)`; `None` when the
    /// page shows no table.
    pub fn tables_file(&self, project: &Project, excel: bool) -> Option<(String, Vec<u8>)> {
        let tables = self.page_tables(project);
        let first = tables.first()?;
        let name = if tables.len() == 1 {
            first.title.clone()
        } else {
            self.current_page()
                .map_or_else(|| "Schedules".to_string(), |p| p.title.clone())
        };
        let bytes = if excel {
            plan_docs::xlsx::schedules_to_xlsx(&tables)
        } else if tables.len() == 1 {
            first.to_csv().into_bytes()
        } else {
            // Several tables in one text file: each under its title, a blank
            // line between.
            tables
                .iter()
                .map(|t| format!("{}\n{}", t.title, t.to_csv()))
                .collect::<Vec<_>>()
                .join("\n")
                .into_bytes()
        };
        Some((name, bytes))
    }

    // ----- boxes -----

    fn selected_box(&self) -> Option<&LayoutBox> {
        let id = self.selected?;
        self.current_page()?.boxes.iter().find(|b| b.id == id)
    }

    /// Moves a box to `r` (paper inches) as one undo step.
    pub fn set_box_bounds(
        &mut self,
        project: &mut Project,
        id: Id,
        r: [f64; 4],
        label: &str,
    ) -> bool {
        self.edit(project, label, |l| {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    if bounds(b) == r {
                        return false;
                    }
                    set_bounds(b, r);
                    return true;
                }
            }
            false
        })
    }

    pub fn delete_selected(&mut self, project: &mut Project) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let done = self.edit(project, "Delete Layout Box", |l| {
            let mut found = false;
            for p in &mut l.pages {
                let n = p.boxes.len();
                p.boxes.retain(|b| b.id != id);
                found |= p.boxes.len() != n;
            }
            found
        });
        if done {
            self.selected = None;
        }
        done
    }

    pub fn apply_spec(&mut self, project: &mut Project, spec: &BoxSpec) -> bool {
        // A Plot Lines or Update on Demand view keeps a picture; a Live View,
        // Always Update keeps none.
        let mut spec = spec.clone();
        {
            let b = &mut spec.layout_box;
            if b.has_camera_options() {
                if b.view.camera == CameraLink::Always {
                    b.view.art = None;
                } else if b.view.art.is_none() {
                    let rcx = render_context(project);
                    b.view.art = plan_layout::make_art(b, &rcx);
                }
            }
        }
        let spec = &spec;
        let done = self.edit(project, "Layout Box Specification", |l| {
            let before = l.clone();
            apply_box_spec(l, spec) && *l != before
        });
        if done {
            if let Some(i) = self
                .layout
                .as_ref()
                .and_then(|l| l.pages.iter().position(|p| p.number == spec.page))
            {
                self.page = i;
            }
            self.cache.map.remove(&spec.layout_box.id);
        }
        done
    }

    // ----- text boxes, tables, pictures, rotation -----

    /// Adds a box showing `source` to the current page, in the first free
    /// area, and selects it. `None` without a layout.
    pub fn add_source_box(
        &mut self,
        project: &mut Project,
        label: &str,
        source: BoxSource,
    ) -> Option<Id> {
        let mut layout = self.layout.clone()?;
        let page_no = layout.pages.get(self.page)?.number;
        let rcx = render_context(project);
        let id = send_to_layout(&mut layout, &rcx, page_no, source, Scale::QuarterInch, None);
        drop(rcx);
        self.commit(project, label, layout);
        self.selected = Some(id);
        self.selected_cad = None;
        Some(id)
    }

    /// Adds a text box to the current page. Text boxes have no frame.
    pub fn add_text_box(
        &mut self,
        project: &mut Project,
        text: &str,
        height_pt: f64,
    ) -> Option<Id> {
        let id = self.add_source_box(project, "Add Text Box", BoxSource::text(text, height_pt))?;
        self.set_box_border(project, id, false);
        Some(id)
    }

    /// Draws a text box in `r` (paper inches) on the current page.
    pub fn add_text_box_in(&mut self, project: &mut Project, r: [f64; 4]) -> Option<Id> {
        let mut layout = self.layout.clone()?;
        let page = layout.pages.get(self.page)?.number;
        let id = next_box_id(&layout);
        let mut b = LayoutBox::new(
            id,
            (Point::new(r[0], r[1]), Point::new(r[2], r[3])),
            BoxSource::text("Text", 12.0),
            Scale::QuarterInch,
        );
        b.border = false;
        layout.page_mut(page)?.boxes.push(b);
        self.commit(project, "Add Text Box", layout);
        self.selected = Some(id);
        self.selected_cad = None;
        Some(id)
    }

    fn set_box_border(&mut self, project: &mut Project, id: Id, border: bool) {
        if let Some(l) = &mut self.layout {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    b.border = border;
                }
            }
        }
        self.write_back(project);
    }

    /// Writes an edited text box (double-click it): text, height, bold and
    /// alignment. False when nothing changed.
    pub fn edit_text_box(&mut self, project: &mut Project, spec: &TextBoxSpec) -> bool {
        let done = self.edit(project, "Edit Text Box", |l| {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == spec.id) {
                    let new = BoxSource::Text {
                        text: spec.text.clone(),
                        height_pt: spec.height_pt,
                        align: spec.align,
                        bold: spec.bold,
                    };
                    if b.source == new && b.text_fit == spec.fit {
                        return false;
                    }
                    b.source = new;
                    b.text_fit = spec.fit;
                    return true;
                }
            }
            false
        });
        if done {
            self.cache.map.remove(&spec.id);
        }
        done
    }

    /// Adds the Materials List (`floor` `None` = every floor, `category`
    /// `None` = every category) as a table box.
    pub fn add_materials_box(
        &mut self,
        project: &mut Project,
        floor: Option<usize>,
        category: Option<String>,
    ) -> Option<Id> {
        self.add_source_box(
            project,
            "Add Materials List",
            BoxSource::Materials { floor, category },
        )
    }

    /// Adds the picture file at `path` as an image box. PNG pictures show;
    /// other formats print a framed placeholder with the file name.
    pub fn add_image_box(&mut self, project: &mut Project, path: &str) -> Result<Id, String> {
        if !std::path::Path::new(path).is_file() {
            return Err(format!("Cannot read {path}"));
        }
        self.add_source_box(
            project,
            "Add Image Box",
            BoxSource::Image {
                path: path.to_string(),
            },
        )
        .ok_or_else(|| "There is no layout".to_string())
    }

    /// Turns the selected box's content a quarter turn counter-clockwise.
    pub fn rotate_selected(&mut self, project: &mut Project) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        let done = self.edit(project, "Rotate Layout Box", |l| {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    b.rotation_deg = f64::from((b.quarter_turns() + 1) % 4) * 90.0;
                    return true;
                }
            }
            false
        });
        if done {
            self.cache.map.remove(&id);
        }
        done
    }

    // ----- layout CAD (lines, boxes, text drawn on the page) -----

    /// Adds layout CAD to the current page and selects it.
    pub fn add_cad(&mut self, project: &mut Project, label: &str, item: CadItem) -> Option<Id> {
        let page = self.page;
        let mut made = None;
        let done = self.edit(project, label, |l| match l.pages.get_mut(page) {
            Some(p) => {
                made = Some(p.add_cad(item));
                true
            }
            None => false,
        });
        if done {
            self.selected_cad = made;
            self.selected = None;
        }
        made
    }

    /// A line on the page from `a` to `b` (paper inches).
    pub fn add_cad_line(&mut self, project: &mut Project, a: Point, b: Point) -> Option<Id> {
        self.add_cad(project, "Add Layout Line", CadItem::Line { a, b })
    }

    /// A rectangle on the page from corner `a` to corner `b`.
    pub fn add_cad_box(&mut self, project: &mut Project, a: Point, b: Point) -> Option<Id> {
        let item = CadItem::Polyline {
            points: vec![a, Point::new(b.x, a.y), b, Point::new(a.x, b.y)],
            closed: true,
        };
        self.add_cad(project, "Add Layout Box Outline", item)
    }

    /// A polyline on the page through `points`.
    pub fn add_cad_polyline(&mut self, project: &mut Project, points: Vec<Point>) -> Option<Id> {
        (points.len() >= 2).then_some(())?;
        self.add_cad(
            project,
            "Add Layout Polyline",
            CadItem::Polyline {
                points,
                closed: false,
            },
        )
    }

    /// A circle on the page.
    pub fn add_cad_circle(
        &mut self,
        project: &mut Project,
        center: Point,
        radius: f64,
    ) -> Option<Id> {
        (radius > 1e-6).then_some(())?;
        self.add_cad(
            project,
            "Add Layout Circle",
            CadItem::Circle { center, radius },
        )
    }

    /// An arc on the page from its centre, start point and end point.
    pub fn add_cad_arc(
        &mut self,
        project: &mut Project,
        center: Point,
        start: Point,
        end: Point,
    ) -> Option<Id> {
        let item = plan_layout::arc_from_points(center, start, end)?;
        self.add_cad(project, "Add Layout Arc", item)
    }

    /// Adds or edits (when `spec.id` is set) a leader.
    pub fn apply_leader(&mut self, project: &mut Project, spec: &LeaderSpec) -> bool {
        let page = self.page;
        let mut made = None;
        let done = self.edit(project, "Leader", |l| {
            let Some(p) = l.pages.get_mut(page) else {
                return false;
            };
            match spec.id {
                None => {
                    let id = p.add_leader_bent(
                        spec.tip,
                        spec.bends.clone(),
                        spec.elbow,
                        &spec.text,
                        spec.height_in,
                    );
                    made = Some(id);
                    if let Some(l) = p.leaders.iter_mut().find(|l| l.id == id) {
                        l.arrow = spec.arrow;
                    }
                    true
                }
                Some(id) => {
                    let Some(l) = p.leaders.iter_mut().find(|l| l.id == id) else {
                        return false;
                    };
                    let new = (spec.text.clone(), spec.height_in, spec.arrow);
                    if (l.text.clone(), l.height_in, l.arrow) == new {
                        return false;
                    }
                    (l.text, l.height_in, l.arrow) = new;
                    true
                }
            }
        });
        if done {
            if let Some(id) = made {
                self.selected_cad = Some(id);
                self.selected = None;
            }
        }
        done
    }

    /// Adds or edits (when `spec.id` is set) a revision cloud.
    pub fn apply_cloud(&mut self, project: &mut Project, spec: &CloudSpec) -> bool {
        let page = self.page;
        let mut made = None;
        let done = self.edit(project, "Revision Cloud", |l| {
            let Some(p) = l.pages.get_mut(page) else {
                return false;
            };
            match spec.id {
                None => {
                    made = Some(p.add_cloud(spec.rect.0, spec.rect.1, &spec.revision));
                    true
                }
                Some(id) => {
                    let Some(c) = p.clouds.iter_mut().find(|c| c.id == id) else {
                        return false;
                    };
                    if c.revision == spec.revision {
                        return false;
                    }
                    c.revision = spec.revision.clone();
                    true
                }
            }
        });
        if done {
            if let Some(id) = made {
                self.selected_cad = Some(id);
                self.selected = None;
            }
        }
        done
    }

    /// Writes the layout's layer settings (Layer Display Options).
    pub fn apply_layers(&mut self, project: &mut Project, layers: &LayoutLayers) -> bool {
        let done = self.edit(project, "Layout Layer Display", |l| {
            if l.layers == *layers {
                return false;
            }
            l.layers = layers.clone();
            true
        });
        if done {
            self.cache.map.clear();
            self.textures.clear();
        }
        done
    }

    /// Adds the sheet index (every printed sheet's number and title) as a table box.
    pub fn add_sheet_index_box(&mut self, project: &mut Project) -> Option<Id> {
        self.add_source_box(project, "Add Sheet Index", BoxSource::SheetIndex)
    }

    /// Adds or edits (when `spec.id` is set) a line of page text.
    pub fn apply_cad_text(&mut self, project: &mut Project, spec: &CadTextSpec) -> bool {
        match spec.id {
            None => self
                .add_cad(
                    project,
                    "Add Layout Text",
                    CadItem::Text {
                        pos: spec.pos,
                        text: spec.text.clone(),
                        height: spec.height_in,
                        angle: 0.0,
                    },
                )
                .is_some(),
            Some(id) => {
                let page = self.page;
                self.edit(project, "Edit Layout Text", |l| {
                    let Some(o) = l
                        .pages
                        .get_mut(page)
                        .and_then(|p| p.cad.iter_mut().find(|o| o.id == id))
                    else {
                        return false;
                    };
                    let CadItem::Text { text, height, .. } = &mut o.item else {
                        return false;
                    };
                    if *text == spec.text && (*height - spec.height_in).abs() < 1e-12 {
                        return false;
                    }
                    *text = spec.text.clone();
                    *height = spec.height_in;
                    true
                })
            }
        }
    }

    /// Deletes the selected page CAD object.
    pub fn delete_selected_cad(&mut self, project: &mut Project) -> bool {
        let Some(id) = self.selected_cad else {
            return false;
        };
        let page = self.page;
        let done = self.edit(project, "Delete Layout CAD", |l| {
            l.pages
                .get_mut(page)
                .is_some_and(|p| p.remove_annotation(id))
        });
        if done {
            self.selected_cad = None;
        }
        done
    }

    /// The perspective renders the layout's boxes ask for, as they will be
    /// rendered, without repeats.
    fn perspective_requests(&self) -> Vec<PerspectiveRequest> {
        let mut out: Vec<PerspectiveRequest> = Vec::new();
        for r in self
            .layout
            .iter()
            .flat_map(|l| l.pages.iter().flat_map(|p| p.boxes.iter()))
            .filter_map(plan_layout::perspective_request)
            .map(effective_request)
        {
            if !out.contains(&r) {
                out.push(r);
            }
        }
        out
    }

    /// Renders the perspective views of the current layout that are not
    /// cached for the plan as it is now, one after the other, and waits for
    /// them. Returns how many were rendered. (Update Views itself renders on
    /// a thread with a progress indicator: see [`start_update`](Self::start_update).)
    #[cfg(test)]
    pub fn render_perspectives(&mut self, project: &Project) -> usize {
        let mut n = 0;
        for r in self.perspective_requests() {
            if cached_perspective(project, &r).is_none() && perspective_image(project, &r).is_some()
            {
                n += 1;
            }
        }
        n
    }

    /// Update Views: drops the drawing caches and renders the perspective
    /// views that are out of date on a thread (the window shows a progress
    /// bar and keeps working). Returns the number of renders started.
    pub fn start_update(&mut self, project: &Project) -> usize {
        self.cache.map.clear();
        self.textures.clear();
        let todo: Vec<PerspectiveRequest> = self
            .perspective_requests()
            .into_iter()
            .filter(|r| {
                cached_perspective(project, r).is_none() && project.camera(r.camera_id).is_some()
            })
            .collect();
        if todo.is_empty() || self.update.is_some() {
            return 0;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (project, flag, total) = (project.clone(), cancel.clone(), todo.len());
        std::thread::spawn(move || {
            for (i, req) in todo.iter().enumerate() {
                if flag.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }
                perspective_image(&project, req);
                if tx.send(i + 1).is_err() {
                    break;
                }
            }
        });
        self.update = Some(UpdateJob {
            rx,
            cancel,
            total,
            done: 0,
        });
        total
    }

    /// Reads the progress of Update Views. Returns `Some(message)` when the
    /// job has finished (or was cancelled), `None` while it runs or when there
    /// is none.
    fn poll_update(&mut self) -> Option<String> {
        let job = self.update.as_mut()?;
        let mut disconnected = false;
        loop {
            match job.rx.try_recv() {
                Ok(n) => job.done = job.done.max(n),
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if !disconnected && job.done < job.total {
            return None;
        }
        let (done, total) = (job.done, job.total);
        self.update = None;
        // Pictures made while the job ran: draw them.
        self.cache.map.clear();
        self.textures.clear();
        Some(if done < total {
            format!("Update Views stopped after {done} of {total} perspective view(s)")
        } else {
            format!("Updated the layout views; rendered {total} perspective view(s)")
        })
    }

    /// Is Update Views still rendering?
    pub fn updating(&self) -> bool {
        self.update.is_some()
    }

    /// Waits for Update Views to finish (tests, headless callers).
    #[cfg(test)]
    fn wait_update(&mut self) -> Option<String> {
        while self.update.is_some() {
            if let Some(m) = self.poll_update() {
                return Some(m);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        None
    }

    // ----- Send to Layout -----

    /// Sends a view to the layout (L-1..L-3): a new box on the chosen page at
    /// the chosen scale, "largest that fits" when `spec.scale` is `None`.
    /// `center` is the paper point a [`Placement::Click`] box is centered on.
    pub fn send(
        &mut self,
        project: &mut Project,
        spec: &SendSpec,
        center: Option<(f64, f64)>,
    ) -> Result<Id, String> {
        self.send_with(project, spec, &default_details(spec), center)
    }

    /// The box source a [`SendSource`] stands for, or why it cannot be sent.
    fn box_source(project: &Project, source: &SendSource) -> Result<BoxSource, String> {
        match source {
            SendSource::Plan { floor, layer_set } => {
                if *floor >= project.floors.len() {
                    return Err("That floor does not exist".into());
                }
                Ok(BoxSource::PlanView {
                    floor: *floor,
                    layer_set: layer_set.clone(),
                })
            }
            SendSource::Camera { id, .. } => {
                if project.camera(*id).is_none() {
                    return Err("That camera does not exist".into());
                }
                Ok(BoxSource::Camera { camera_id: *id })
            }
            SendSource::Perspective { id, .. } => {
                if project.camera(*id).is_none() {
                    return Err("That camera does not exist".into());
                }
                Ok(BoxSource::Perspective { camera_id: *id })
            }
        }
    }

    /// [`send`](Self::send) with the dialog's other answers: the scaling
    /// (the lists, a typed ratio, Fit to Sheet), Send Options (Entire
    /// Plan/View, Current Screen, As Image, Link Saved Plan View), Camera View
    /// Options (Live View or Plot Lines), Snap to Active CAD Point and Show
    /// Layout Page. What it made (and whether it is too big for the sheet) is
    /// in [`last_sent`](Self::last_sent).
    pub fn send_with(
        &mut self,
        project: &mut Project,
        spec: &SendSpec,
        details: &SendDetails,
        center: Option<(f64, f64)>,
    ) -> Result<Id, String> {
        let Some(mut layout) = self.layout.clone() else {
            return Err("There is no layout to send to".into());
        };
        let source = Self::box_source(project, &spec.source)?;
        let page_no = match spec.page {
            PageChoice::Existing(n) if layout.page(n).is_some() => n,
            _ => {
                let n = next_page_number(&layout);
                layout.add_page(n, format!("Page {n}"));
                n
            }
        };
        let rcx = render_context(project);
        let centre = match (spec.placement, center) {
            (Placement::FirstFree, _) | (Placement::Click, None) => None,
            (Placement::Centered, _) => {
                let (lo, hi) = layout
                    .page(page_no)
                    .map_or_else(|| layout.drawing_area(), |p| layout.page_drawing_area(p));
                Some(((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0))
            }
            (Placement::Click, Some(c)) => Some(c),
        };
        let req = SendRequest {
            page: page_no,
            source,
            scale: details.scale,
            at: None,
            centre: centre.map(|(x, y)| Point::new(x, y)),
            options: details.options.clone(),
        };
        let sent = if details.as_image {
            plan_layout::send_as_image(&mut layout, &rcx, &req)
        } else {
            plan_layout::send_view(&mut layout, &rcx, &req)
        };
        drop(rcx);
        let id = sent.id;
        if details.snap_to_point {
            // Snap to Active CAD Point: the box's lower left takes the nearest
            // corner or end point already on the page.
            let ll = layout
                .page(page_no)
                .and_then(|p| p.boxes.iter().find(|b| b.id == id))
                .map(|b| {
                    let r = bounds(b);
                    Point::new(r[0], r[1])
                });
            if let Some(ll) = ll {
                let snapped = plan_layout::snap_point_except(&layout, page_no, ll, 1.0, Some(id));
                if snapped != ll {
                    if let Some(b) = layout
                        .page_mut(page_no)
                        .and_then(|p| p.boxes.iter_mut().find(|b| b.id == id))
                    {
                        let r = bounds(b);
                        set_bounds(b, moved(r, snapped.x - ll.x, snapped.y - ll.y, false));
                    }
                }
            }
        }
        let request = layout
            .pages
            .iter()
            .flat_map(|p| p.boxes.iter())
            .find(|b| b.id == id)
            .and_then(plan_layout::perspective_request)
            .map(effective_request);
        if let Some(r) = request {
            // Render the view now so the page shows it (and print finds it cached).
            perspective_image(project, &r);
        }
        self.commit(project, "Send to Layout", layout);
        if details.show_page {
            if let Some(i) = self
                .layout
                .as_ref()
                .and_then(|l| l.pages.iter().position(|p| p.number == page_no))
            {
                self.page = i;
            }
        }
        self.selected = Some(id);
        self.line_sel.clear();
        self.last_sent = Some(sent);
        Ok(id)
    }

    /// The warning of the last send when its view did not fit the sheet.
    pub fn last_send_warning(&self) -> Option<String> {
        self.last_sent.as_ref().and_then(Sent::warning)
    }

    /// Sends every floor plan, one per page: the first onto the current page
    /// when it is empty, the rest onto new pages titled by their floor.
    pub fn send_all_floors(&mut self, project: &mut Project) -> usize {
        let layer_set = project.layer_sets.active.clone();
        let mut sent = 0;
        for floor in 0..project.floors.len() {
            let empty_here = sent == 0
                && self
                    .current_page()
                    .is_some_and(|p| !p.template_page && p.boxes.is_empty());
            let page = if empty_here {
                PageChoice::Existing(self.current_page().map_or(1, |p| p.number))
            } else {
                PageChoice::New
            };
            let spec = SendSpec {
                source: SendSource::Plan {
                    floor,
                    layer_set: layer_set.clone(),
                },
                page,
                scale: None,
                placement: Placement::FirstFree,
            };
            if self.send(project, &spec, None).is_ok() {
                let title = plan_layout::plan_label(project, floor);
                let at = self.page;
                self.rename_page_quiet(at, &title);
                sent += 1;
            }
        }
        self.write_back(project);
        sent
    }

    /// Renames a page without a history step (part of a bigger one).
    fn rename_page_quiet(&mut self, index: usize, title: &str) {
        if let Some(p) = self.layout.as_mut().and_then(|l| l.pages.get_mut(index)) {
            p.title = title.to_string();
        }
    }

    // ----- layout boxes: scale, pan, keeping views current, plot lines -----

    /// Edits the selected boxes (those that show a scaled view) with `f`,
    /// given the render context of the plan, as one undo step. True when `f`
    /// changed any of them.
    fn edit_selected_boxes(
        &mut self,
        project: &mut Project,
        label: &str,
        mut f: impl FnMut(&mut LayoutBox, &LayoutRenderContext) -> bool,
    ) -> bool {
        let ids = self.selection_ids();
        if ids.is_empty() {
            return false;
        }
        let Some(mut layout) = self.layout.clone() else {
            return false;
        };
        let rcx = render_context(project);
        let mut changed = false;
        for p in &mut layout.pages {
            for b in p.boxes.iter_mut().filter(|b| ids.contains(&b.id)) {
                changed |= f(b, &rcx);
            }
        }
        drop(rcx);
        if changed {
            self.commit(project, label, layout);
        }
        changed
    }

    /// Rescale Layout View: sets the scale of the selected views (No Scale, a
    /// scale of the lists, a typed one), resizing the boxes with it unless
    /// Scale Layout Box Contents Only is off, and Use Layout Line Scaling.
    pub fn rescale_selected(
        &mut self,
        project: &mut Project,
        new: NewScale,
        line_scaling: Option<bool>,
    ) -> bool {
        self.edit_selected_boxes(project, "Rescale Layout View", |b, _| {
            if !is_scaled(&b.source) {
                return false;
            }
            let mut changed = plan_layout::rescale(b, new);
            if let Some(on) = line_scaling {
                changed |= b.view.layout_line_scaling != on;
                b.view.layout_line_scaling = on;
            }
            changed
        })
    }

    /// Pan/Scale Layout Box, the typed scale: sets the selected view's scale
    /// from text such as `1:48` or `1/4" = 1'`.
    pub fn set_scale_text(&mut self, project: &mut Project, text: &str) -> Result<bool, String> {
        let ipf = plan_layout::parse_scale_text(text)
            .ok_or_else(|| "Type a scale such as 1:48 or 1/4\" = 1'".to_string())?;
        Ok(self.rescale_selected(project, NewScale::PerFoot(ipf), None))
    }

    /// Recenter Layout Box Contents on the selected views.
    pub fn recenter_selected(&mut self, project: &mut Project) -> bool {
        self.edit_selected_boxes(project, "Recenter Layout Box Contents", |b, cx| {
            let Some(frame) = plan_layout::view_frame_in(&b.source, &b.view, cx) else {
                return false;
            };
            let before = b.view.pan_in;
            plan_layout::recenter(b, frame);
            b.view.pan_in != before
        })
    }

    /// Scale Layout Box Contents to Fit: the scale at which the whole view
    /// fills each selected box, whether or not it is on a list.
    pub fn scale_selected_to_fit(&mut self, project: &mut Project) -> bool {
        self.edit_selected_boxes(project, "Scale Layout Box Contents to Fit", |b, cx| {
            let Some(frame) = plan_layout::view_frame_in(&b.source, &b.view, cx) else {
                return false;
            };
            let before = (b.scale, b.view.scale_mode, b.view.pan_in);
            plan_layout::scale_box_to_fit(b, frame);
            (b.scale, b.view.scale_mode, b.view.pan_in) != before
        })
    }

    /// Pan/Scale Layout Box: moves the contents of the selected box by `d`
    /// paper inches, as one undo step (a drag does it live).
    pub fn pan_selected(&mut self, project: &mut Project, d: (f64, f64)) -> bool {
        self.edit_selected_boxes(project, "Pan Layout Box", |b, _| {
            let before = b.view.pan_in;
            plan_layout::pan_by(b, d);
            b.view.pan_in != before
        })
    }

    /// Unlink Saved Plan View: the selected plan views keep the floor and
    /// layer set the saved view gave them.
    pub fn unlink_saved_view(&mut self, project: &mut Project) -> bool {
        let snapshot = project.clone();
        self.edit_selected_boxes(project, "Unlink Saved Plan View", |b, _| {
            let Some(name) = b.view.saved_view.take() else {
                return false;
            };
            if let BoxSource::PlanView { floor, layer_set } = &mut b.source {
                if let Some((fl, set)) = plan_layout::saved_view_target(&snapshot, &name) {
                    if let Some(fl) = fl {
                        *floor = fl;
                    }
                    if !set.is_empty() {
                        *layer_set = set;
                    }
                }
            }
            true
        })
    }

    /// Update Layout Views: makes the pictures of the views `scope` names
    /// again (one undo step), and says what it did. Perspective views are
    /// rendered by [`start_update`](Self::start_update).
    pub fn update_views(
        &mut self,
        project: &mut Project,
        scope: &UpdateScope,
    ) -> plan_layout::UpdateReport {
        let Some(mut layout) = self.layout.clone() else {
            return plan_layout::UpdateReport::default();
        };
        let rcx = render_context(project);
        let report = plan_layout::update_views(&mut layout, &rcx, scope, None);
        drop(rcx);
        if report.updated > 0 {
            self.commit(project, "Update Layout Views", layout);
            self.cache.map.clear();
        }
        report
    }

    /// The status text for an update.
    fn update_status(r: plan_layout::UpdateReport) -> String {
        match (r.updated, r.skipped) {
            (0, 0) => "No layout view needed updating".to_string(),
            (n, 0) => format!("Updated {n} layout view(s)"),
            (n, k) => format!("Updated {n} layout view(s); {k} cannot be updated"),
        }
    }

    // ----- Edit Layout Lines -----

    /// The selected plot lines box with the map from its view to paper.
    fn plot_box(&self, project: &Project) -> Option<(LayoutBox, plan_layout::ContentMap)> {
        let b = self.selected_box()?.clone();
        b.view.art.as_ref()?;
        let rcx = ui_context(project);
        let map = plan_layout::box_content_map(&b, &rcx)?;
        Some((b, map))
    }

    /// Selects the plot line near the paper point `(x, y)` of the selected
    /// (or hit) Plot Lines box; with `extend` it joins or leaves the
    /// selection. True when a line was hit.
    pub fn select_line_at(
        &mut self,
        project: &Project,
        x: f64,
        y: f64,
        tol: f64,
        extend: bool,
    ) -> bool {
        // The box under the pointer becomes the working box.
        if let Some(id) = self.current_page().and_then(|p| box_at(p, x, y)) {
            if Some(id) != self.selected {
                self.selected = Some(id);
                self.line_sel.clear();
            }
        }
        let Some((b, map)) = self.plot_box(project) else {
            return false;
        };
        let k = b.points_per_inch() / 72.0;
        let at = map.to_source(Point::new(x, y));
        let hit = b
            .view
            .art
            .as_ref()
            .and_then(|a| a.hit_line(at, tol / k.max(1e-9)));
        match (hit, extend) {
            (Some(id), true) => {
                if let Some(i) = self.line_sel.iter().position(|l| *l == id) {
                    self.line_sel.remove(i);
                } else {
                    self.line_sel.push(id);
                }
            }
            (Some(id), false) => self.line_sel = vec![id],
            (None, false) => self.line_sel.clear(),
            (None, true) => {}
        }
        hit.is_some()
    }

    /// The lines selected with Edit Layout Lines.
    pub fn selected_lines(&self) -> &[Id] {
        &self.line_sel
    }

    /// Runs `f` on the picture of the selected Plot Lines box as one undo
    /// step.
    fn edit_art(
        &mut self,
        project: &mut Project,
        label: &str,
        f: impl FnOnce(&mut plan_layout::ViewArt) -> bool,
    ) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        self.edit(project, label, |l| {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    return b.view.art.as_mut().is_some_and(f);
                }
            }
            false
        })
    }

    /// Draws a new line in the selected Plot Lines box, `a` to `b` in the
    /// view's own space (it keeps its place against the view).
    pub fn add_plot_line(&mut self, project: &mut Project, a: Point, b: Point) -> Option<Id> {
        let mut made = None;
        let done = self.edit_art(project, "Draw Layout Line", |art| {
            made = Some(art.add_line(a, b, plan_layout::LineType::Edge));
            true
        });
        if done {
            self.line_sel = made.into_iter().collect();
        }
        made.filter(|_| done)
    }

    /// Deletes the selected plot lines.
    pub fn delete_selected_lines(&mut self, project: &mut Project) -> usize {
        let ids = self.line_sel.clone();
        let mut n = 0;
        if self.edit_art(project, "Delete Layout Lines", |art| {
            n = art.delete_lines(&ids);
            n > 0
        }) {
            self.line_sel.clear();
        }
        n
    }

    /// Applies the Layout Line Specification to the selected plot lines.
    pub fn apply_line_spec(
        &mut self,
        project: &mut Project,
        spec: &plan_layout::LineSpec,
    ) -> usize {
        let ids = self.line_sel.clone();
        let mut n = 0;
        self.edit_art(project, "Layout Line Specification", |art| {
            n = art.set_spec(&ids, spec);
            n > 0
        });
        n
    }

    /// The Layout Line Specification of the selected plot lines.
    fn open_line_spec(&mut self) -> bool {
        let Some(b) = self.selected_box() else {
            return false;
        };
        let (Some(art), opts) = (&b.view.art, &b.view.plot) else {
            return false;
        };
        let lines: Vec<_> = art
            .lines
            .iter()
            .filter(|l| self.line_sel.contains(&l.id))
            .map(|l| (l.clone(), plan_layout::effective_pen(l, opts)))
            .collect();
        if lines.is_empty() {
            return false;
        }
        self.dialogs.line_spec = Some(LayoutLineDialog::new(&lines));
        true
    }

    /// Moves the selected lines live (no history step) from `orig` by `d`
    /// view inches.
    fn live_move_lines(
        &mut self,
        project: &mut Project,
        id: Id,
        orig: &plan_layout::ViewArt,
        d: Point,
    ) {
        let ids = self.line_sel.clone();
        if let Some(l) = &mut self.layout {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    let mut art = orig.clone();
                    art.nudge_lines(&ids, d);
                    b.view.art = Some(art);
                }
            }
        }
        self.write_back(project);
    }

    /// Pans a box live (no history step): its contents at `pan0` moved by the
    /// drag `d` (paper inches).
    fn live_pan(&mut self, project: &mut Project, id: Id, pan0: (f64, f64), d: (f64, f64)) {
        if let Some(l) = &mut self.layout {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    b.view.pan_in = pan0;
                    plan_layout::pan_by(b, d);
                }
            }
        }
        self.write_back(project);
    }

    /// Resizes a non-scaled box by its corner live, the view growing with it
    /// (the Alternate edit behavior).
    fn live_resize_no_scale(
        &mut self,
        project: &mut Project,
        id: Id,
        orig: &LayoutBox,
        r: [f64; 4],
    ) {
        if let Some(l) = &mut self.layout {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    *b = orig.clone();
                    plan_layout::resize_no_scale(b, r);
                }
            }
        }
        self.write_back(project);
    }

    // ----- object snaps and moving by points (R16-02, item 6) -----

    /// A point snapped to the corners and ends already on the page (object
    /// snap), else to the grid; `snapping` off leaves it where it is.
    fn snap_xy(&self, x: f64, y: f64, snapping: bool, tol: f64) -> (f64, f64) {
        if snapping {
            if let (Some(l), Some(p)) = (self.layout.as_ref(), self.current_page()) {
                let at = Point::new(x, y);
                let q = plan_layout::snap_point(l, p.number, at, tol * 0.6);
                if q != at {
                    return (q.x, q.y);
                }
            }
        }
        (snap(x, snapping), snap(y, snapping))
    }

    /// The rectangle the whole selection covers (boxes and page drawings).
    fn selection_bounds(&self) -> Option<[f64; 4]> {
        let page = self.current_page()?;
        let mut all: Vec<[f64; 4]> = page
            .boxes
            .iter()
            .filter(|b| self.selection_ids().contains(&b.id))
            .map(bounds)
            .collect();
        all.extend(
            self.cad_selection_ids()
                .into_iter()
                .filter_map(|id| page.annotation_bounds(id)),
        );
        all.into_iter().reduce(|a, b| {
            [
                a[0].min(b[0]),
                a[1].min(b[1]),
                a[2].max(b[2]),
                a[3].max(b[3]),
            ]
        })
    }

    /// Moves the selected boxes and page drawings by `(dx, dy)` paper inches
    /// as one undo step.
    pub fn move_selection(&mut self, project: &mut Project, d: (f64, f64), label: &str) -> bool {
        if d.0.abs() < 1e-9 && d.1.abs() < 1e-9 {
            return false;
        }
        let boxes = self.selection_ids();
        let cads = self.cad_selection_ids();
        let page = self.page;
        self.edit(project, label, |l| {
            let mut any = false;
            if let Some(p) = l.pages.get_mut(page) {
                for b in p.boxes.iter_mut().filter(|b| boxes.contains(&b.id)) {
                    let r = bounds(b);
                    set_bounds(b, [r[0] + d.0, r[1] + d.1, r[2] + d.0, r[3] + d.1]);
                    any = true;
                }
                for id in &cads {
                    any |= p.move_annotation(*id, d.0, d.1);
                }
            }
            any
        })
    }

    /// Center Object: moves the selection so it is centered in the page's
    /// drawing area.
    pub fn center_selection(&mut self, project: &mut Project) -> bool {
        let (Some(r), Some(l), Some(p)) = (
            self.selection_bounds(),
            self.layout.as_ref(),
            self.current_page(),
        ) else {
            return false;
        };
        let (lo, hi) = l.page_drawing_area(p);
        let d = (
            (lo.x + hi.x) / 2.0 - (r[0] + r[2]) / 2.0,
            (lo.y + hi.y) / 2.0 - (r[1] + r[3]) / 2.0,
        );
        self.move_selection(project, d, "Center Object")
    }

    // ----- viewing -----

    fn fit(&mut self, area: Rect) {
        if self.layout.is_none() {
            return;
        }
        let (w, h) = self.sheet_of(self.page);
        let z = ((area.width() - 40.0) / w as f32).min((area.height() - 40.0) / h as f32);
        self.zoom = z.clamp(MIN_ZOOM, MAX_ZOOM);
        self.offset = Vec2::new(
            (area.width() - w as f32 * self.zoom) / 2.0,
            (area.height() - h as f32 * self.zoom) / 2.0,
        );
        self.fit_pending = false;
    }

    fn zoom_about(&mut self, area: Rect, pointer: Pos2, factor: f32) {
        let z0 = self.zoom;
        let z1 = (z0 * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let origin = area.min + self.offset;
        // The paper point under the pointer stays under it.
        let px = (pointer.x - origin.x) / z0;
        let py = (pointer.y - origin.y) / z0;
        self.zoom = z1;
        self.offset = Vec2::new(
            pointer.x - px * z1 - area.min.x,
            pointer.y - py * z1 - area.min.y,
        );
    }
}

// ---------------------------------------------------------------- thread --

thread_local! {
    static VIEW: RefCell<LayoutView> = RefCell::new(LayoutView::default());
}

fn with_view<R>(f: impl FnOnce(&mut LayoutView) -> R) -> R {
    VIEW.with(|v| f(&mut v.borrow_mut()))
}

/// Is the layout view showing (instead of the plan or the 3D view)?
pub fn is_active() -> bool {
    VIEW.with(|v| v.try_borrow().map(|v| v.active).unwrap_or(true))
}

/// Is a layout dialog open (the canvas keys must not reach the tools)?
pub fn dialog_open() -> bool {
    VIEW.with(|v| v.try_borrow().map(|v| v.dialogs.any()).unwrap_or(true))
}

/// Leaves the layout view (the plan or 3D view takes over).
pub fn deactivate() {
    with_view(|v| v.active = false);
}

/// `(index, "A-1  Title", template page)` of every page, read from the project's
/// layout JSON (for the Project Browser).
pub fn page_list(project: &Project) -> Vec<(usize, String, bool)> {
    let Some(pages) = project
        .layout
        .as_ref()
        .and_then(|l| l.get("pages"))
        .and_then(|p| p.as_array())
    else {
        return Vec::new();
    };
    let fields: Vec<(String, bool, u32)> = pages
        .iter()
        .map(|p| {
            let n = p.get("number").and_then(|n| n.as_u64()).unwrap_or(0) as u32;
            let label = p.get("label").and_then(|t| t.as_str()).unwrap_or("");
            let template = p
                .get("template_page")
                .and_then(|t| t.as_bool())
                .unwrap_or(false);
            (label.to_string(), template, n)
        })
        .collect();
    // The labels follow the pages: a `#` takes the next number among the
    // pages with the same label.
    let labels = plan_layout::resolve_labels(fields.iter().map(|(l, t, n)| (l.as_str(), *t, *n)));
    pages
        .iter()
        .zip(labels)
        .zip(&fields)
        .enumerate()
        .map(|(i, ((p, label), (_, template, _)))| {
            let title = p.get("title").and_then(|t| t.as_str()).unwrap_or("");
            (i, format!("{label}  {title}"), *template)
        })
        .collect()
}

/// Shows page `index` (the benchmark draws a floor plan sheet).
#[cfg(test)]
pub(crate) fn show_page(index: usize) {
    with_view(|v| v.set_page(index));
}

/// The page the layout view shows.
pub fn current_page_index() -> usize {
    with_view(|v| v.page)
}

/// File > New Layout: makes the project's layout from Daniel's template and
/// shows it. When the plan has a layout already, it becomes a second layout
/// file (`Project::layout_files`): the New Layout File name dialog opens, and
/// the layout that was open is parked when the file is made.
pub fn new_layout(cx: &mut EditorContext) {
    let settings = crate::templates::load_settings();
    let seed = crate::templates::refresh(&settings, false);
    let made = with_view(|v| {
        let made = v.create(&mut cx.project, seed.cache.layout.as_ref());
        v.active = true;
        v.flush(cx);
        made
    });
    if made {
        cx.status = "New layout: page template and page 1".into();
    } else {
        with_view(|v| v.active = true);
        dispatch(LayoutCommand::NewLayoutFile, cx, None);
        cx.status = "This plan already has a layout; name the new layout file".into();
    }
    sync_sheet(cx);
}

/// File > Templates > New Layout from Template: makes the plan's layout and
/// applies `template` to it (one undo step for the template). A plan that
/// has a layout already keeps it: Layout > Apply Template changes that one.
/// Returns whether the layout was made.
pub fn new_layout_from_template(
    cx: &mut EditorContext,
    template: &plan_layout::LayoutTemplate,
) -> bool {
    if cx.project.layout.is_some() {
        cx.status =
            "This plan has a layout already: use Layout > Apply Template to change it".into();
        return false;
    }
    new_layout(cx);
    let done = with_view(|v| {
        let ok = v.apply_template(&mut cx.project, template);
        v.flush(cx);
        ok
    });
    sync_sheet(cx);
    cx.status = if done {
        format!("New layout from the template \"{}\"", template.name)
    } else {
        "The layout was not changed".into()
    };
    done
}

/// Keeps the plan's Drawing Sheet outline on the layout's sheet size.
fn sync_sheet(cx: &mut EditorContext) {
    let changed = with_view(|v| {
        let now = v.layout().map(|l| l.sheet);
        (now != v.synced_sheet).then(|| {
            v.synced_sheet = now;
            now
        })
    });
    if let Some(Some(size)) = changed {
        cx.sheet.size = size;
    }
}

/// Runs a layout command. `camera` is the elevation or section camera of the
/// open 3D view, which Send to Layout sends instead of the plan.
pub fn dispatch(cmd: LayoutCommand, cx: &mut EditorContext, camera: Option<Id>) {
    let mut view = with_view(std::mem::take);
    view.run(cx, cmd, camera);
    view.flush(cx);
    with_view(|slot| *slot = view);
    sync_sheet(cx);
}

/// Send to Layout for the camera `id` (what the 3D panel's buttons call):
/// opens the Send to Layout dialog on that camera.
#[allow(dead_code)] // The 3D panel moves to this once it drops its own session layout.
pub fn send_camera(cx: &mut EditorContext, id: Id) {
    dispatch(LayoutCommand::SendToLayout, cx, Some(id));
}

/// The project's layout as PDF bytes, if it has any printed pages.
pub fn layout_pdf(project: &Project) -> Option<Vec<u8>> {
    let layout = load(project)?;
    (!layout.content_pages().is_empty()).then(|| print_bytes(&layout, project, None))
}

/// Sends the Materials List (`floor` `None` = every floor, `category` `None` =
/// every category) to the current layout page as a table box, making the
/// layout when the plan has none, and shows the layout.
pub fn send_materials(
    cx: &mut EditorContext,
    floor: Option<usize>,
    category: Option<String>,
) -> String {
    let mut view = with_view(std::mem::take);
    view.ensure(cx);
    let made = view.add_materials_box(&mut cx.project, floor, category);
    view.active = true;
    view.flush(cx);
    with_view(|slot| *slot = view);
    sync_sheet(cx);
    match made {
        Some(_) => "Sent the Materials List to the layout".into(),
        None => "There is no layout page to add to".into(),
    }
}

/// File > Print > Print Image while the 3D view shows: opens the size dialog
/// for `view` (see `View3dState::snapshot_source`); OK ray traces it at that
/// size and saves a PNG.
pub fn print_image_3d(cx: &mut EditorContext, view: crate::shell::view3d_panel::Snapshot3d) {
    with_view(|v| v.dialogs.image3d = Some((Image3dDialog::new(4.0 / 3.0), view)));
    cx.status = "Print Image: choose the size of the 3D view".into();
}

/// Create Construction Set: adds Daniel's sheet set (cover with the sheet
/// index, site, floor plans, elevations, section, details, schedules) to the
/// live layout, making the layout first when the plan has none. Returns the
/// status text.
pub fn install_construction_set(cx: &mut EditorContext) -> String {
    dispatch(LayoutCommand::CreateConstructionSet, cx, None);
    cx.status.clone()
}

/// Edit > Undo while the layout view shows.
pub fn undo(cx: &mut EditorContext) -> Option<String> {
    with_view(|v| v.flush(cx));
    cx.undo()
}

pub fn redo(cx: &mut EditorContext) -> Option<String> {
    with_view(|v| v.flush(cx));
    cx.redo()
}

// ------------------------------------------------------------- commands --

impl LayoutView {
    fn ensure(&mut self, cx: &mut EditorContext) {
        self.sync(&cx.project);
        if self.layout.is_none() {
            let settings = crate::templates::load_settings();
            let seed = crate::templates::refresh(&settings, false);
            self.create(&mut cx.project, seed.cache.layout.as_ref());
        }
    }

    fn page_list(&self) -> Vec<(u32, String)> {
        self.layout
            .as_ref()
            .map(|l| {
                l.pages
                    .iter()
                    .zip(l.page_labels())
                    .filter(|(p, _)| !p.template_page)
                    .map(|(p, label)| (p.number, format!("{label}  {}", p.title)))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Starts placing a Layout Page Table or Layout Revision Table: the
    /// next click on the page puts it there.
    fn begin_table_placing(&mut self, cx: &mut EditorContext, source: BoxSource) -> String {
        let Some((layout, page)) = self.layout.as_ref().zip(self.current_page()) else {
            return "There is no page to place the table on".into();
        };
        let rcx = render_context(&cx.project);
        rcx.set_sheet_index(layout);
        rcx.set_current_page(page.number);
        let size = source_size_in(&source, Scale::QuarterInch, &rcx);
        drop(rcx);
        self.placing = Some(Placing {
            source,
            scale: None,
            details: None,
            page: PageChoice::Existing(page.number),
            size,
        });
        self.active = true;
        "Click on the page to place the table (Esc cancels)".into()
    }

    /// The click that places a table: its centre lands at `(x, y)`.
    fn place_table(&mut self, project: &mut Project, source: BoxSource, x: f64, y: f64) -> bool {
        let Some(mut layout) = self.layout.clone() else {
            return false;
        };
        let Some(page_no) = layout.pages.get(self.page).map(|p| p.number) else {
            return false;
        };
        let rcx = render_context(project);
        rcx.set_sheet_index(&layout);
        rcx.set_current_page(page_no);
        let (w, h) = source_size_in(&source, Scale::QuarterInch, &rcx);
        let at = Point::new(
            snap(x - w / 2.0, true).max(0.0),
            snap(y - h / 2.0, true).max(0.0),
        );
        let id = send_to_layout(
            &mut layout,
            &rcx,
            page_no,
            source,
            Scale::QuarterInch,
            Some(at),
        );
        drop(rcx);
        self.commit(project, "Layout Table", layout);
        self.selected = Some(id);
        self.selected_cad = None;
        true
    }

    /// Executes `cmd`; the result is reported in the status bar.
    pub fn run(&mut self, cx: &mut EditorContext, cmd: LayoutCommand, camera: Option<Id>) {
        use LayoutCommand as C;
        if cmd == C::ShowPlan {
            self.active = false;
            return;
        }
        if matches!(cmd, C::PrintDialog | C::PrintImage) {
            // Printing the plan view must not create a layout.
            self.sync(&cx.project);
            let status = self.open_print(cx, cmd == C::PrintImage);
            if !status.is_empty() {
                cx.status = status;
            }
            return;
        }
        self.ensure(cx);
        let stays = matches!(
            cmd,
            C::SendToLayout
                | C::SendCamera(_)
                | C::PrintModel
                | C::LayerDisplay
                | C::Print
                | C::PrintDialog
                | C::PrintImage
                | C::ExportPdf
                | C::PageSetup
                | C::ProjectInfo
                | C::SaveAsTemplate
                | C::OpenSourceView
                | C::Undo
                | C::Redo
                | C::FitPage
        );
        if !stays {
            self.active = true;
        }
        let project = &mut cx.project;
        let status: String = match cmd {
            C::ShowLayout => String::new(),
            C::ShowPlan => String::new(),
            C::SendToLayout | C::SendCamera(_) => {
                let camera = match cmd {
                    C::SendCamera(id) => Some(id),
                    _ => camera,
                };
                let source = self.send_source(project, camera, cx.floor);
                let floors = project.floors.iter().map(|f| f.name.clone()).collect();
                let sets = project
                    .layer_sets
                    .sets
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                let current = self
                    .current_page()
                    .filter(|p| !p.template_page)
                    .map(|p| p.number);
                let mut dialog =
                    SendToLayoutDialog::new(source, self.page_list(), current, floors, sets)
                        .with_layouts(layout_names(project))
                        .with_saved_view(project.current_plan_view().map(|v| v.name.clone()))
                        .with_default_scale(sheet_scale(project))
                        .with_screen(PLAN_SCREEN.with(Cell::get));
                if let Some(last) = LAST_SEND.with(|l| l.borrow().clone()) {
                    dialog = dialog.with_defaults(&last);
                }
                // A 3D view that is showing can be sent as a picture.
                if let Some(view) = SNAPSHOT_3D.with(|s| s.borrow_mut().take()) {
                    dialog = dialog.with_snapshot(camera.is_none());
                    self.snapshot_src = Some(view);
                }
                self.dialogs.send = Some(dialog);
                String::new()
            }
            C::SendAllViews => {
                let layer_set = project.layer_sets.active.clone();
                let mut sources: Vec<SendSource> = (0..project.floors.len())
                    .map(|floor| SendSource::Plan {
                        floor,
                        layer_set: layer_set.clone(),
                    })
                    .collect();
                sources.extend(
                    project
                        .cameras
                        .iter()
                        .filter(|c| cam::is_elevation_camera(c))
                        .map(|c| SendSource::Camera {
                            id: c.id,
                            name: c.name.clone(),
                        }),
                );
                if sources.is_empty() {
                    "There are no views to send".into()
                } else {
                    self.begin_send_many(project, sources);
                    String::new()
                }
            }
            C::SendAllFloors => {
                let n = self.send_all_floors(project);
                format!("Sent {n} floor plan(s) to the layout")
            }
            C::PageSetup => {
                let choices = self.layout.as_ref().map(|l| l.size_choices());
                self.dialogs.setup = self.page_setup().map(|s| {
                    let d = PageSetupDialog::new(s);
                    match choices {
                        Some(c) => d.with_choices(c),
                        None => d,
                    }
                });
                String::new()
            }
            C::PageInformation => match self.page_info_dialog(project) {
                Some(d) => {
                    self.dialogs.page_info = Some(d);
                    String::new()
                }
                None => "There is no page to edit".into(),
            },
            C::CustomizeSheetSizes => match (self.sheet_sizes(), self.layout.as_ref()) {
                (Some(sizes), Some(l)) => {
                    self.dialogs.sheet_sizes = Some(SheetSizesDialog::new(sizes, l.sheet));
                    String::new()
                }
                _ => "There is no layout".into(),
            },
            C::Align(edge) => {
                if self.selection_ids().is_empty() {
                    "Select a layout box first (Shift-click for several)".into()
                } else {
                    let n = self.align_selected(project, edge);
                    if n == 0 {
                        "The boxes are already lined up".into()
                    } else {
                        format!("Moved {n} layout box(es): {}", edge.label())
                    }
                }
            }
            C::Distribute(axis) => {
                if self.selection_ids().len() < 3 {
                    "Select three or more layout boxes (Shift-click) to distribute them".into()
                } else {
                    let n = self.distribute_selected(project, axis);
                    if n == 0 {
                        "The boxes are already evenly spaced".into()
                    } else {
                        format!("Spread the layout boxes: moved {n}")
                    }
                }
            }
            C::CopyBoxToPage => {
                let ids = self.selection_ids();
                match self.current_page().map(|p| p.number) {
                    Some(here) if !ids.is_empty() => {
                        self.dialogs.copy_box =
                            Some(CopyBoxDialog::new(self.page_list(), here, ids.len()));
                        String::new()
                    }
                    _ => "Select a layout box first".into(),
                }
            }
            C::DuplicateBox => {
                if self.selection_ids().is_empty() {
                    "Select a layout box first".into()
                } else {
                    let n = self.duplicate_here(project);
                    format!("Copied {n} layout box(es)")
                }
            }
            C::OpenSourceView => match self.selected_box().map(|b| b.source.clone()) {
                Some(BoxSource::PlanView { floor, .. }) => {
                    cx.floor = floor.min(project.floors.len().saturating_sub(1));
                    self.active = false;
                    format!("Opened {}", plan_layout::plan_label(&cx.project, cx.floor))
                }
                Some(BoxSource::Camera { camera_id } | BoxSource::Perspective { camera_id }) => {
                    let name = project
                        .camera(camera_id)
                        .map_or("the camera".to_string(), |c| c.name.clone());
                    format!("Open {name} from the Project Browser (cameras list)")
                }
                Some(_) => "That box does not come from a view of the plan".into(),
                None => "Select a layout box first".into(),
            },
            C::NewLayoutFile => {
                let taken = layout_names(project);
                let suggestion = format!("{} Layout {}", project.name, taken.len() + 1);
                self.dialogs.new_layout = Some(NameDialog::new(
                    "New Layout File",
                    "Name",
                    &suggestion,
                    taken,
                ));
                String::new()
            }
            C::SwitchLayout(i) => {
                if self.switch_layout(project, i) {
                    self.active = true;
                    format!(
                        "Opened layout {}",
                        self.layout.as_ref().map_or("", |l| l.name.as_str())
                    )
                } else {
                    String::new()
                }
            }
            C::ExportTableCsv | C::ExportTableExcel => {
                self.export_tables(project, cmd == C::ExportTableExcel)
            }
            C::ProjectInfo => {
                // Tools > Project Information (the schedules builder's dialog).
                crate::dialogs::build_tools::open_project_info(cx);
                String::new()
            }
            C::PageTable => self.begin_table_placing(cx, BoxSource::PageTable),
            C::RevisionTable => self.begin_table_placing(cx, BoxSource::RevisionTable),
            C::AddLayoutRevision => match self.layout.as_ref() {
                Some(l) => {
                    let labels = l.page_labels();
                    let pages: Vec<(u32, String)> = l
                        .pages
                        .iter()
                        .zip(labels)
                        .filter(|(p, _)| !p.template_page)
                        .map(|(p, label)| (p.number, format!("{label}  {}", p.title)))
                        .collect();
                    let here = self
                        .current_page()
                        .filter(|p| !p.template_page)
                        .map(|p| p.number)
                        .into_iter()
                        .collect();
                    self.dialogs.revision = Some(RevisionDialog::add(
                        pages,
                        here,
                        &today(),
                        &macro_context(project).designer,
                    ));
                    String::new()
                }
                None => "There is no layout".into(),
            },
            C::LayoutDefaults => match self.layout.as_ref() {
                Some(l) => {
                    self.dialogs.defaults = Some(LayoutDefaultsDialog::new(LayoutDefaults::of(l)));
                    String::new()
                }
                None => "There is no layout".into(),
            },
            C::CopyDrawingsToPage => {
                let drawings = self
                    .current_page()
                    .map_or(0, |p| p.cad.len() + p.leaders.len() + p.clouds.len());
                let here = self.current_page().map_or(0, |p| p.number);
                if drawings == 0 {
                    "This page has no drawings to copy".into()
                } else if self.page_list().len() < 2 {
                    "There is no other page to copy to".into()
                } else {
                    self.dialogs.copy_drawings =
                        Some(CopyBoxDialog::drawings(self.page_list(), here, drawings));
                    String::new()
                }
            }
            C::MovePage(from, to) => {
                if self.move_page_to(project, from, to) {
                    "Moved the page; the labels follow".into()
                } else {
                    String::new()
                }
            }
            C::SaveAsTemplate => {
                let existing = layout_templates_dir()
                    .map(|d| list_layout_templates(&d))
                    .unwrap_or_default();
                let name = self
                    .layout
                    .as_ref()
                    .map_or_else(String::new, |l| l.name.clone());
                self.dialogs.template = Some(LayoutTemplateDialog::save(&name, existing));
                String::new()
            }
            C::ApplyTemplate => {
                let existing = layout_templates_dir()
                    .map(|d| list_layout_templates(&d))
                    .unwrap_or_default();
                if existing.is_empty() {
                    "There are no saved layout templates (Layout > Save As Template)".into()
                } else {
                    self.dialogs.template = Some(LayoutTemplateDialog::apply(existing));
                    String::new()
                }
            }
            C::InsertPageBefore | C::InsertPageAfter => {
                self.add_page(project, cmd == C::InsertPageBefore);
                "Inserted a page".into()
            }
            C::DuplicatePage => {
                self.duplicate_current_page(project);
                "Duplicated the page".into()
            }
            C::DeletePage => match self.delete_current_page(project) {
                Ok(()) => "Deleted the page".into(),
                Err(e) => e.to_string(),
            },
            C::ExchangeWithPrevious | C::ExchangeWithNext => {
                if self.exchange_current_page(project, cmd == C::ExchangeWithNext) {
                    "Exchanged pages".into()
                } else {
                    "There is no page to exchange with".into()
                }
            }
            C::NextPage => {
                self.set_page(self.page + 1);
                String::new()
            }
            C::PreviousPage => {
                self.set_page(self.page.saturating_sub(1));
                String::new()
            }
            C::GoToPage(i) => {
                self.set_page(i);
                String::new()
            }
            C::BoxSpecification => {
                if self.tool == LayoutTool::EditLines
                    && !self.line_sel.is_empty()
                    && self.open_line_spec()
                {
                    String::new()
                } else {
                    match self.selected_box().cloned() {
                        Some(b) => {
                            self.open_spec(project, &b);
                            String::new()
                        }
                        None => "Select a layout box first".into(),
                    }
                }
            }
            C::DeleteBox => {
                if self.delete_selected(project) {
                    "Deleted the layout box".into()
                } else {
                    "Select a layout box first".into()
                }
            }
            C::UpdateViews => {
                if self.updating() {
                    "Update Views is already running".into()
                } else {
                    // Update All Views: the semi-dynamic and Plot Lines views
                    // first (one undo step), then the perspective renders.
                    let report = self.update_views(project, &UpdateScope::All);
                    let n = self.start_update(project);
                    if n > 0 {
                        format!("Updating the layout views: {n} perspective view(s) to render")
                    } else if report.updated > 0 {
                        Self::update_status(report)
                    } else {
                        "Updated the layout views".into()
                    }
                }
            }
            C::UpdateView => {
                let ids = self.selection_ids();
                if ids.is_empty() {
                    "Select a layout view first".into()
                } else {
                    let report = self.update_views(project, &UpdateScope::Selected(ids));
                    Self::update_status(report)
                }
            }
            C::UpdateLiveViews => {
                let report = self.update_views(project, &UpdateScope::LiveViews);
                Self::update_status(report)
            }
            C::UpdatePlotLineViews => {
                let report = self.update_views(project, &UpdateScope::PlotLines);
                Self::update_status(report)
            }
            C::RescaleView => match self.selected_box().filter(|b| is_scaled(&b.source)) {
                Some(b) => {
                    self.dialogs.change_scale = Some(ChangeScaleDialog::new(b));
                    String::new()
                }
                None => "Select a plan, section, elevation or detail view first".into(),
            },
            C::RecenterBox => {
                if self.recenter_selected(project) {
                    "Recentered the contents of the layout box".into()
                } else {
                    "The contents are centered already, or no view is selected".into()
                }
            }
            C::ScaleBoxToFit => {
                if self.scale_selected_to_fit(project) {
                    "Scaled the contents to fit the layout box".into()
                } else {
                    "Select a plan, section, elevation or detail view first".into()
                }
            }
            C::LayoutBoxLayers => match self.selected_box().cloned() {
                Some(b) if matches!(b.source, BoxSource::PlanView { .. }) => {
                    self.open_spec_on(project, &b, Some("Layer Set"));
                    String::new()
                }
                Some(_) => "Layout Box Layers works on dynamic plan views".into(),
                None => "Select a layout view first".into(),
            },
            C::CenterObject => {
                if self.center_selection(project) {
                    "Centered the selection in the drawing area".into()
                } else {
                    "Select a layout box or page drawing first".into()
                }
            }
            C::UnlinkSavedView => {
                if self.unlink_saved_view(project) {
                    "Unlinked the saved plan view: the box keeps its floor and layer set".into()
                } else {
                    "The selected view is not linked to a saved plan view".into()
                }
            }
            C::AddSheetIndex => match self.add_sheet_index_box(project) {
                Some(_) => "Added the sheet index".into(),
                None => "There is no page to add to".into(),
            },
            C::LayerDisplay => {
                self.dialogs.layers = self
                    .layout
                    .as_ref()
                    .map(|l| LayoutLayersDialog::new(l.layers.clone()));
                String::new()
            }
            C::PrintModel => {
                let cameras: Vec<(Id, String)> = project
                    .cameras
                    .iter()
                    .filter(|c| !cam::is_elevation_camera(c))
                    .map(|c| (c.id, c.name.clone()))
                    .collect();
                if cameras.is_empty() {
                    "Print Model needs a perspective camera: add one with the Camera tools".into()
                } else {
                    self.dialogs.model = Some(ModelDialog::new(cameras, camera));
                    String::new()
                }
            }
            C::CreateConstructionSet => {
                let snapshot = project.clone();
                let master = load_master_list();
                let floors = snapshot.floors.len();
                let mut added = 0;
                let done = self.edit(project, "Create Construction Set", |l| {
                    added = plan_layout::append_construction_set_in(
                        l,
                        &snapshot,
                        floors,
                        &master,
                        Some(&view_scene),
                    );
                    added > 0
                });
                if done {
                    // Show the cover of the set.
                    if let Some(i) = self.layout.as_ref().and_then(|l| {
                        l.pages
                            .iter()
                            .position(|p| p.title == "Cover" && !p.template_page)
                    }) {
                        self.set_page(i);
                    }
                    self.cache.map.clear();
                    self.textures.clear();
                    self.fit_pending = true;
                    format!("Added the construction set: {added} sheet(s)")
                } else {
                    "The construction set could not be added".into()
                }
            }
            C::PrintDialog | C::PrintImage => String::new(),
            C::AddTextBox => match self.add_text_box(project, "Text", 12.0) {
                Some(id) => {
                    if let Some(b) = self.selected_box().filter(|b| b.id == id) {
                        self.dialogs.text_box = TextBoxDialog::new(b);
                    }
                    "Added a text box; double-click it to edit".into()
                }
                None => "There is no page to add to".into(),
            },
            C::AddMaterialsBox => match self.add_materials_box(project, None, None) {
                Some(_) => "Added the Materials List".into(),
                None => "There is no page to add to".into(),
            },
            C::AddImageBox => match crate::tools::images::pick_image_file() {
                Some(path) => match self.add_image_box(project, &path) {
                    Ok(_) => "Added the picture".into(),
                    Err(e) => e,
                },
                None => String::new(),
            },
            C::RotateBox => {
                if self.rotate_selected(project) {
                    "Turned the layout box".into()
                } else {
                    "Select a layout box first".into()
                }
            }
            C::Tool(t) => {
                self.tool = t;
                self.poly.clear();
                self.drag = None;
                self.line_sel.clear();
                if !matches!(
                    t,
                    LayoutTool::Select
                        | LayoutTool::PanScale
                        | LayoutTool::EditLines
                        | LayoutTool::PointToPoint
                ) {
                    self.selected = None;
                    self.selected_cad = None;
                }
                match t {
                    LayoutTool::PointToPoint => {
                        "Point to Point Move: click where the selection moves from, then where it moves to".into()
                    }
                    LayoutTool::PanScale => {
                        "Pan/Scale Layout Box: drag to pan the contents of the selected view; type a scale in the box that opens".into()
                    }
                    LayoutTool::EditLines => {
                        "Edit Layout Lines: click a line of a Plot Lines view to select it, drag to draw a new one, Delete removes the selected lines".into()
                    }
                    LayoutTool::Select => String::new(),
                    LayoutTool::Line
                    | LayoutTool::Box
                    | LayoutTool::TextBox
                    | LayoutTool::Cloud => {
                        format!("{}: drag on the page (Esc to stop)", t.name())
                    }
                    LayoutTool::Circle => "Circle: drag from the centre outward".into(),
                    LayoutTool::Arc => {
                        "Arc: click the centre, then the start, then the end of the arc".into()
                    }
                    LayoutTool::Leader => {
                        "Leader: drag from what it points at to where the text goes, or click the tip and each bend and double-click where the text goes".into()
                    }
                    LayoutTool::Polyline => {
                        "Polyline: click the corners, double-click to finish".into()
                    }
                    LayoutTool::Text => "Text: click where the text goes".into(),
                }
            }
            C::FitPage => {
                self.fit_pending = true;
                String::new()
            }
            C::Print => {
                self.dialogs.print = self.layout_print_dialog();
                String::new()
            }
            C::ExportPdf => self.export_pdf(project, None),
            C::Undo => {
                self.flush(cx);
                match cx.undo() {
                    Some(l) => format!("Undid {l}"),
                    None => "Nothing to undo".into(),
                }
            }
            C::Redo => {
                self.flush(cx);
                match cx.redo() {
                    Some(l) => format!("Redid {l}"),
                    None => "Nothing to redo".into(),
                }
            }
        };
        if !status.is_empty() {
            cx.status = status;
        }
    }

    /// Saves the tables on the page (or the selected table box) as CSV or an
    /// Excel workbook; returns the status text.
    fn export_tables(&self, project: &Project, excel: bool) -> String {
        match self.tables_file(project, excel) {
            Some((name, bytes)) => save_bytes_as(&name, if excel { "xlsx" } else { "csv" }, &bytes),
            None => "Select a schedule or Materials List box, or open a page that has one".into(),
        }
    }

    fn send_source(&self, project: &Project, camera: Option<Id>, floor: usize) -> SendSource {
        if let Some(c) = camera.and_then(|id| project.camera(id)) {
            return if cam::is_elevation_camera(c) {
                SendSource::Camera {
                    id: c.id,
                    name: c.name.clone(),
                }
            } else {
                SendSource::Perspective {
                    id: c.id,
                    name: c.name.clone(),
                }
            };
        }
        SendSource::Plan {
            floor: floor.min(project.floors.len().saturating_sub(1)),
            layer_set: project.layer_sets.active.clone(),
        }
    }

    fn open_spec(&mut self, project: &Project, b: &LayoutBox) {
        self.open_spec_on(project, b, None);
    }

    /// The choices a view box's specification offers.
    fn box_choices(&self, project: &Project) -> BoxSpecChoices {
        let pages = self.page_list();
        let cameras: Vec<(Id, String)> = project
            .cameras
            .iter()
            .map(|c| (c.id, c.name.clone()))
            .collect();
        let details: Vec<String> = project
            .floors
            .iter()
            .filter(|f| f.is_cad_detail())
            .map(|f| f.name.clone())
            .collect();
        BoxSpecChoices {
            floors: project.floors.iter().map(|f| f.name.clone()).collect(),
            layer_sets: project
                .layer_sets
                .sets
                .iter()
                .map(|s| s.name.clone())
                .collect(),
            plan_views: project.plan_views.iter().map(|v| v.name.clone()).collect(),
            default_sets: Vec::new(),
            file_name: self
                .layout
                .as_ref()
                .map_or_else(String::new, |l| format!("{} ({})", project.name, l.name)),
            links: link_choices(&cameras, &details, &pages),
            pages,
        }
    }

    /// Opens the Layout Box Specification of `b` (the view-box dialog for a
    /// view of the plan, else the plain one), on the panel `tab` names.
    fn open_spec_on(&mut self, project: &Project, b: &LayoutBox, tab: Option<&str>) {
        let Some(page) = self.current_page() else {
            return;
        };
        let number = page.number;
        if LayoutBoxDialog::handles(b) {
            let mut d = LayoutBoxDialog::new(b, number, self.box_choices(project));
            if let Some(t) = tab {
                d = d.on_tab(t);
            }
            self.dialogs.box_view = Some(d);
            return;
        }
        let floors = project.floors.iter().map(|f| f.name.clone()).collect();
        let sets = project
            .layer_sets
            .sets
            .iter()
            .map(|s| s.name.clone())
            .collect();
        self.dialogs.spec = Some(BoxSpecDialog::new(
            b,
            number,
            self.page_list(),
            floors,
            sets,
        ));
    }

    /// The Print dialog for the layout's printed pages.
    fn layout_print_dialog(&self) -> Option<PrintDialog> {
        let l = self.layout.as_ref()?;
        Some(
            PrintDialog::for_layout(l.content_pages().len(), l.sheet_inches(), &l.name)
                .with_custom_papers(
                    l.custom_sizes
                        .iter()
                        .map(|c| (c.label(), c.inches()))
                        .collect(),
                ),
        )
    }

    /// File > Print (or Print Image): the layout's dialog when the layout view
    /// is open, else the plan view's. Returns a status message when nothing opens.
    fn open_print(&mut self, cx: &EditorContext, image: bool) -> String {
        let floor = cx.floor.min(cx.project.floors.len().saturating_sub(1));
        let layer_set = cx.project.layer_sets.active.clone();
        if image {
            self.dialogs.image = Some(ImageDialog::new(floor, &layer_set));
            return String::new();
        }
        if self.active && self.layout.is_some() {
            self.dialogs.print = self.layout_print_dialog();
        } else {
            let title = plan_layout::plan_label(&cx.project, floor);
            self.dialogs.print = Some(PrintDialog::for_plan(floor, &layer_set, &title));
        }
        String::new()
    }

    /// The Print dialog was accepted: build the PDF and deliver it.
    fn finish_print(&mut self, cx: &mut EditorContext, d: &PrintDialog) {
        let opts = d.options();
        cx.status = match d.target() {
            PrintTarget::Layout { name, .. } => {
                let Some(layout) = &self.layout else {
                    cx.status = "There is no layout".into();
                    return;
                };
                // Print Model's quality override: perspective boxes at the asked DPI.
                let tuned;
                let layout = if d.perspective_dpi() > 0 || d.perspective_samples() > 0 {
                    tuned = plan_layout::with_perspective_quality(
                        layout,
                        d.perspective_dpi(),
                        d.perspective_samples(),
                    );
                    &tuned
                } else {
                    layout
                };
                let bytes = print_with(layout, &cx.project, &opts);
                print::deliver_to(&bytes, d.destination(), d.copies(), name, d.printer())
            }
            PrintTarget::PlanView { floor, title, .. } => {
                let (bytes, _) = print_plan(&cx.project, *floor, title, &opts);
                print::deliver_to(&bytes, d.destination(), d.copies(), title, d.printer())
            }
        };
    }

    /// Print Model was accepted: renders the chosen camera at the chosen DPI
    /// onto the paper and delivers the PDF.
    fn finish_model(&mut self, cx: &mut EditorContext, d: &ModelDialog) {
        let title = d.camera_name().to_uppercase();
        let bytes = {
            // The renderer is asked at the size the page makes of it.
            let rcx = render_context(&cx.project);
            plan_layout::print_model_pdf(&rcx, d.camera, d.dpi, d.samples, &title, &d.options())
        };
        cx.status = print::deliver_to(&bytes, d.destination(), d.copies(), &title, d.printer());
    }

    /// Print Image of the 3D view was accepted: ray traces the view at the
    /// chosen size and saves the PNG.
    fn finish_image_3d(
        &mut self,
        cx: &mut EditorContext,
        d: &Image3dDialog,
        view: &crate::shell::view3d_panel::Snapshot3d,
    ) {
        let Some(png) = view.render_png(d.width_px, d.height_px, d.samples) else {
            cx.status = "The 3D view has nothing to print".into();
            return;
        };
        if cfg!(test) {
            cx.status = format!("Rendered {} bytes of PNG", png.len());
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("3d-view.png")
            .add_filter("PNG", &["png"])
            .save_file()
        else {
            cx.status = "Print Image cancelled".into();
            return;
        };
        cx.status = match std::fs::write(&path, png) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Could not save: {e}"),
        };
    }

    /// Print Preview from the Print dialog: the Drawing Sheet outline at the
    /// chosen paper and drawing scale, the plan outside it grayed.
    fn preview_print(&mut self, cx: &mut EditorContext, d: &PrintDialog) {
        use crate::toolbar::ViewFlag;
        let opts = d.options();
        if let plan_layout::PaperSize::Standard(z) = opts.paper {
            cx.sheet.size = z;
        }
        if let PrintTarget::PlanView { floor, title, .. } = d.target() {
            let rcx = render_context(&cx.project);
            let source = BoxSource::PlanView {
                floor: *floor,
                layer_set: cx.project.layer_sets.active.clone(),
            };
            cx.sheet.scale = plan_layout::plan_print_scale(&rcx, &source, &opts);
            let _ = title;
        }
        cx.view_flags.insert(ViewFlag::DrawingSheet);
        cx.view_flags.insert(ViewFlag::PrintPreview);
        crate::editor::sheet::set_preview_color(opts.color);
        self.open_preview(&cx.project, d);
        let mode = crate::editor::sheet::preview_color_label(opts.color);
        cx.status = if mode.is_empty() {
            "Print Preview: the sheet shows the chosen paper and scale".into()
        } else {
            format!("Print Preview: the sheet shows the chosen paper and scale, in {mode}")
        };
    }

    /// The pages of the layout as `opts` will print them (Print Preview),
    /// without any rendering of perspective boxes that were not rendered
    /// already.
    pub fn print_preview_pages(
        &self,
        project: &Project,
        opts: &PrintOptions,
    ) -> Vec<plan_layout::PreviewPage> {
        let Some(layout) = &self.layout else {
            return Vec::new();
        };
        let rcx = ui_context(project);
        rcx.set_sheet_index(layout);
        plan_layout::layout_print_preview(layout, &rcx, opts)
    }

    /// Opens the Print Preview window: the pages as the Print dialog's
    /// options will print them (colour mode, line weights, scale, tiles).
    /// Perspective boxes show when they were rendered already (Update Views).
    fn open_preview(&mut self, project: &Project, d: &PrintDialog) {
        let opts = d.options();
        let (title, note, pages) = match d.target() {
            PrintTarget::Layout { name, .. } => (
                name.clone(),
                String::new(),
                self.print_preview_pages(project, &opts),
            ),
            PrintTarget::PlanView {
                floor,
                layer_set,
                title,
            } => {
                let rcx = ui_context(project);
                let (pages, scale) =
                    plan_layout::plan_view_print_preview(&rcx, *floor, layer_set, title, &opts);
                (title.clone(), format!("SCALE: {}", scale.label()), pages)
            }
        };
        self.dialogs.preview = Some(PrintPreviewDialog::new(&title, &note, pages));
    }

    /// Print Image was accepted: saves the plan view as a PNG.
    fn finish_image(&mut self, cx: &mut EditorContext, d: &ImageDialog) {
        let png = plan_png(&cx.project, d.floor, &d.layer_set, d.scale, d.width_px);
        if cfg!(test) {
            cx.status = format!("Rendered {} bytes of PNG", png.len());
            return;
        }
        let Some(path) = rfd::FileDialog::new()
            .set_file_name("plan.png")
            .add_filter("PNG", &["png"])
            .save_file()
        else {
            cx.status = "Print Image cancelled".into();
            return;
        };
        cx.status = match std::fs::write(&path, png) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Could not save: {e}"),
        };
    }

    /// Asks for a file and saves the layout as a PDF; returns the status text.
    fn export_pdf(&self, project: &Project, range: Option<(usize, usize)>) -> String {
        let Some(layout) = &self.layout else {
            return "There is no layout".into();
        };
        if layout.content_pages().is_empty() {
            return "The layout has no pages to print".into();
        }
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(format!("{}.pdf", layout.name))
            .add_filter("PDF", &["pdf"])
            .save_file()
        else {
            return "Print cancelled".into();
        };
        let bytes = print_bytes(layout, project, range);
        match std::fs::write(&path, bytes) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(e) => format!("Could not save: {e}"),
        }
    }
}

// --------------------------------------------------------------- dialogs --

/// Shows the layout dialogs and applies what was accepted.
pub fn show_dialogs(ctx: &egui::Context, cx: &mut EditorContext) {
    set_opening_labels(&cx.defaults.opening_labels);
    let mut view = with_view(std::mem::take);
    view.show_dialogs(ctx, cx);
    view.flush(cx);
    with_view(|slot| *slot = view);
    sync_sheet(cx);
}

impl LayoutView {
    fn show_dialogs(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        self.sync(&cx.project);
        if let Some(mut d) = self.dialogs.send.take() {
            // The "too big for the sheet" warning: would the box the answers
            // make fit the page's drawing area?
            let outcome = {
                let layout = self.layout.as_ref();
                let project = &cx.project;
                let mut warn = |a: &SendAnswers| -> Option<String> {
                    let layout = layout?;
                    let source = Self::box_source(project, &a.spec.source).ok()?;
                    let page = match a.spec.page {
                        PageChoice::Existing(n) => n,
                        PageChoice::New => next_page_number(layout),
                    };
                    let rcx = render_context(project);
                    let req = SendRequest {
                        page,
                        source,
                        scale: a.details.scale,
                        at: None,
                        centre: None,
                        options: a.details.options.clone(),
                    };
                    let (fit, _) = plan_layout::check_send(layout, &rcx, &req);
                    fit.too_large.then(|| plan_layout::fit_warning(&fit))
                };
                d.show(ctx, &mut warn)
            };
            match outcome {
                Outcome::Open => self.dialogs.send = Some(d),
                Outcome::Cancel => {
                    self.snapshot_src = None;
                    self.send_queue.clear();
                }
                Outcome::Ok => {
                    let answers = d.answers();
                    self.finish_send_answers(cx, answers);
                }
            }
        }
        if let Some(mut d) = self.dialogs.spec.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.spec = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_spec(&mut cx.project, &d.result()) {
                        cx.status = "Updated the layout box".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.box_view.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.box_view = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_spec(&mut cx.project, &d.result()) {
                        cx.status = "Updated the layout box".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.line_spec.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.line_spec = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    let n = self.apply_line_spec(&mut cx.project, &d.spec());
                    cx.status = format!("Changed {n} layout line(s)");
                }
            }
        }
        if let Some(mut d) = self.dialogs.change_scale.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.change_scale = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if let Some((new, lines)) = d.result() {
                        let changed = self.rescale_selected(&mut cx.project, new, Some(lines));
                        cx.status = if changed {
                            "Rescaled the layout view".into()
                        } else {
                            "The scale did not change".into()
                        };
                    }
                }
            }
        }
        self.pan_scale_window(ctx, cx);
        if let Some(mut d) = self.dialogs.setup.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.setup = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_page_setup(&mut cx.project, d.setup()) {
                        self.fit_pending = true;
                        cx.status = "Page setup changed".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.print.take() {
            let out = d.show(ctx);
            if std::mem::take(&mut d.preview_requested) {
                self.preview_print(cx, &d);
            }
            match out {
                Outcome::Open => self.dialogs.print = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => self.finish_print(cx, &d),
            }
        }
        if let Some(mut d) = self.dialogs.image.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.image = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => self.finish_image(cx, &d),
            }
        }
        if let Some(mut d) = self.dialogs.text_box.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.text_box = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.edit_text_box(&mut cx.project, d.spec()) {
                        cx.status = "Updated the text box".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.cad_text.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.cad_text = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_cad_text(&mut cx.project, d.spec()) {
                        cx.status = "Page text updated".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.leader.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.leader = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_leader(&mut cx.project, d.spec()) {
                        cx.status = "Leader updated".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.cloud.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.cloud = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_cloud(&mut cx.project, d.spec()) {
                        cx.status = "Revision cloud updated".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.layers.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.layers = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_layers(&mut cx.project, d.layers()) {
                        cx.status = "Layout layers updated".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.template.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.template = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => cx.status = self.finish_template(&mut cx.project, &d),
            }
        }
        if let Some(mut d) = self.dialogs.page_info.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.page_info = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => match self.apply_page_info(&mut cx.project, &d) {
                    Ok(true) => {
                        self.fit_pending = true;
                        cx.status = "Page information changed".into();
                    }
                    Ok(false) => {}
                    Err(e) => cx.status = e,
                },
            }
        }
        if let Some(mut d) = self.dialogs.revision.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.revision = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    let pages = d.revised_pages();
                    if self.add_layout_revision(&mut cx.project, &pages, &d.revision()) {
                        cx.status = format!("Added a revision to {} page(s)", pages.len());
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.defaults.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.defaults = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_layout_defaults(&mut cx.project, d.defaults()) {
                        cx.status = "General layout defaults changed".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.copy_drawings.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.copy_drawings = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    let n = self.copy_drawings_to(&mut cx.project, d.to());
                    cx.status = format!("Copied {n} drawing(s) to the other page");
                }
            }
        }
        if let Some(mut d) = self.dialogs.sheet_sizes.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.sheet_sizes = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    if self.apply_sheet_sizes(&mut cx.project, d.sizes()) {
                        cx.status = "Sheet sizes customized".into();
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.copy_box.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.copy_box = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    let n = self.copy_selected_to(&mut cx.project, d.to());
                    cx.status = format!("Copied {n} layout box(es) to A-{}", d.to());
                }
            }
        }
        if let Some(mut d) = self.dialogs.new_layout.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.new_layout = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    let settings = crate::templates::load_settings();
                    let seed = crate::templates::refresh(&settings, false);
                    if self.new_layout_file(&mut cx.project, d.name(), seed.cache.layout.as_ref()) {
                        self.active = true;
                        cx.status = format!("Made layout file {}", d.name());
                    }
                }
            }
        }
        if let Some(mut d) = self.dialogs.preview.take() {
            if d.show(ctx) == Outcome::Open {
                self.dialogs.preview = Some(d);
            }
        }
        if let Some(mut d) = self.dialogs.model.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.model = Some(d),
                Outcome::Cancel => {}
                Outcome::Ok => self.finish_model(cx, &d),
            }
        }
        if let Some((mut d, view)) = self.dialogs.image3d.take() {
            match d.show(ctx) {
                Outcome::Open => self.dialogs.image3d = Some((d, view)),
                Outcome::Cancel => {}
                Outcome::Ok => self.finish_image_3d(cx, &d, &view),
            }
        }
        self.update_progress(ctx, cx);
    }

    /// Pan/Scale Layout Box: while the tool is active and a view is selected,
    /// a small window takes the scale to type (the inline text fields of the
    /// tool in Chief).
    fn pan_scale_window(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if self.tool != LayoutTool::PanScale || self.dialogs.any() {
            return;
        }
        let Some(b) = self.selected_box().filter(|b| is_scaled(&b.source)) else {
            return;
        };
        let now = b.scale_note();
        let mut apply = false;
        egui::Window::new("Pan/Scale Layout Box")
            .id(egui::Id::new("layout_pan_scale"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::RIGHT_TOP, Vec2::new(-12.0, 120.0))
            .show(ctx, |ui| {
                ui.label(format!("Scale now: {now}"));
                ui.horizontal(|ui| {
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut self.pan_scale_text)
                            .hint_text("1:48 or 1/4\" = 1'")
                            .desired_width(140.0),
                    );
                    if ui.button("Apply").clicked()
                        || (edit.lost_focus() && ctx.input(|i| i.key_pressed(egui::Key::Enter)))
                    {
                        apply = true;
                    }
                });
                ui.weak("Drag on the view to pan it.");
            });
        if apply {
            let text = self.pan_scale_text.clone();
            cx.status = match self.set_scale_text(&mut cx.project, &text) {
                Ok(true) => format!("Rescaled the layout view to {text}"),
                Ok(false) => "The scale did not change".into(),
                Err(e) => e,
            };
        }
    }

    /// Update Views: reads the render thread's progress and shows the
    /// "Updating views" window with its progress bar and Cancel.
    fn update_progress(&mut self, ctx: &egui::Context, cx: &mut EditorContext) {
        if let Some(msg) = self.poll_update() {
            cx.status = msg;
            ctx.request_repaint();
            return;
        }
        let Some(job) = &self.update else { return };
        let (done, total) = (job.done, job.total);
        let mut cancel = false;
        egui::Window::new("Update Views")
            .id(egui::Id::new("layout_update_views"))
            .collapsible(false)
            .resizable(false)
            .pivot(Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .show(ctx, |ui| {
                ui.set_min_width(300.0);
                ui.label(format!("Rendering perspective views: {done} of {total}"));
                ui.add(
                    egui::ProgressBar::new(done as f32 / total.max(1) as f32)
                        .show_percentage()
                        .animate(true),
                );
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        if cancel {
            job.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    /// The Send to Layout dialog was accepted with the plain answers (a
    /// [`SendSpec`] only): the scale as asked, a Live View, the whole view.
    fn finish_send(
        &mut self,
        cx: &mut EditorContext,
        spec: SendSpec,
        target: LayoutTarget,
        snapshot: Option<SnapshotSpec>,
    ) {
        let details = default_details(&spec);
        self.finish_send_answers(
            cx,
            SendAnswers {
                spec,
                details,
                target,
                snapshot,
            },
        );
    }

    /// Starts sending `sources`: the first gets its dialog, and a check box
    /// there sends all the remaining views with the same settings.
    pub fn begin_send_many(&mut self, project: &Project, mut sources: Vec<SendSource>) {
        if sources.is_empty() {
            return;
        }
        let first = sources.remove(0);
        self.send_queue = sources;
        self.open_send_dialog(project, first);
    }

    /// Opens the Send to Layout dialog on `source`, with the settings used
    /// last and the views still waiting behind it.
    fn open_send_dialog(&mut self, project: &Project, source: SendSource) {
        let floors = project.floors.iter().map(|f| f.name.clone()).collect();
        let sets = project
            .layer_sets
            .sets
            .iter()
            .map(|s| s.name.clone())
            .collect();
        let current = self
            .current_page()
            .filter(|p| !p.template_page)
            .map(|p| p.number);
        let mut dialog = SendToLayoutDialog::new(source, self.page_list(), current, floors, sets)
            .with_layouts(layout_names(project))
            .with_saved_view(project.current_plan_view().map(|v| v.name.clone()))
            .with_default_scale(sheet_scale(project))
            .with_screen(PLAN_SCREEN.with(Cell::get))
            .with_remaining(self.send_queue.len());
        if let Some(last) = LAST_SEND.with(|l| l.borrow().clone()) {
            dialog = dialog.with_defaults(&last);
        }
        self.dialogs.send = Some(dialog);
    }

    /// The Send to Layout dialog was accepted.
    fn finish_send_answers(&mut self, cx: &mut EditorContext, answers: SendAnswers) {
        LAST_SEND.with(|l| *l.borrow_mut() = Some(answers.clone()));
        let SendAnswers {
            spec,
            details,
            target,
            snapshot,
        } = answers;
        let picture = snapshot.zip(self.snapshot_src.take());
        if picture.is_some() || target != LayoutTarget::Current {
            match self.send_to_with(&mut cx.project, &spec, &details, &target, picture, None) {
                Ok(_) => {
                    self.active = true;
                    cx.status = match &target {
                        LayoutTarget::Current => self.sent_status(),
                        LayoutTarget::Existing(n) | LayoutTarget::New(n) => {
                            format!("Sent to layout file {n}")
                        }
                    };
                }
                Err(e) => cx.status = e,
            }
            self.send_rest(cx, &spec, &details);
            return;
        }
        if spec.placement == Placement::Click && self.send_queue.is_empty() {
            let Ok(source) = Self::box_source(&cx.project, &spec.source) else {
                cx.status = "That view cannot be sent".into();
                return;
            };
            let rcx = render_context(&cx.project);
            let size = self
                .layout
                .as_ref()
                .map(|l| {
                    let req = SendRequest {
                        page: match spec.page {
                            PageChoice::Existing(n) => n,
                            PageChoice::New => next_page_number(l),
                        },
                        source: source.clone(),
                        scale: details.scale,
                        at: None,
                        centre: None,
                        options: details.options.clone(),
                    };
                    plan_layout::check_send(l, &rcx, &req).0.size_in
                })
                .unwrap_or_else(|| source_size_in(&source, AUTO_SCALE_CEILING, &rcx));
            drop(rcx);
            self.placing = Some(Placing {
                source,
                scale: spec.scale,
                details: Some(details),
                page: spec.page,
                size,
            });
            // Pages are chosen now so the click lands on the right sheet.
            if let PageChoice::Existing(n) = spec.page {
                if let Some(i) = self
                    .layout
                    .as_ref()
                    .and_then(|l| l.pages.iter().position(|p| p.number == n))
                {
                    self.set_page(i);
                }
            }
            self.active = true;
            cx.status = "Click on the page to place the layout box (Esc cancels)".into();
            return;
        }
        let spec_now = SendSpec {
            placement: if spec.placement == Placement::Click {
                Placement::FirstFree
            } else {
                spec.placement
            },
            ..spec.clone()
        };
        match self.send_with(&mut cx.project, &spec_now, &details, None) {
            Ok(_) => {
                if details.show_page {
                    self.active = true;
                }
                cx.status = self.sent_status();
            }
            Err(e) => cx.status = e,
        }
        self.send_rest(cx, &spec, &details);
    }

    /// After a send: the rest of a Send All Views, either all at once with the
    /// same settings (the dialog's check box) or one dialog after another.
    fn send_rest(&mut self, cx: &mut EditorContext, spec: &SendSpec, details: &SendDetails) {
        if self.send_queue.is_empty() {
            return;
        }
        if details.all_remaining {
            let rest = std::mem::take(&mut self.send_queue);
            let mut n = 1;
            for source in rest {
                let each = SendSpec {
                    source,
                    page: PageChoice::New,
                    placement: Placement::FirstFree,
                    ..spec.clone()
                };
                if self
                    .send_with(&mut cx.project, &each, details, None)
                    .is_ok()
                {
                    n += 1;
                }
            }
            cx.status = format!("Sent {n} views to layout");
        } else {
            let next = self.send_queue.remove(0);
            self.open_send_dialog(&cx.project, next);
        }
    }

    /// The status line after a send: the scale, and the warning when the view
    /// is too big for the sheet.
    fn sent_status(&self) -> String {
        match (&self.last_sent, self.last_send_warning()) {
            (_, Some(w)) => format!("Sent to layout. {w}"),
            (Some(s), None) => {
                let (named, mode) = plan_layout::scale_for_ipf(s.ipf);
                match mode {
                    plan_layout::ScaleMode::Named => {
                        format!("Sent to layout at {}", named.label())
                    }
                    _ => format!("Sent to layout at {}", plan_layout::custom_label(s.ipf)),
                }
            }
            (None, None) => "Sent to layout".into(),
        }
    }

    /// The click that places a pending Send to Layout.
    pub(crate) fn place_at(&mut self, cx: &mut EditorContext, x: f64, y: f64) {
        let Some(p) = self.placing.take() else { return };
        if matches!(p.source, BoxSource::PageTable | BoxSource::RevisionTable) {
            cx.status = if self.place_table(&mut cx.project, p.source, x, y) {
                "Placed the table".into()
            } else {
                "The table was not placed".into()
            };
            return;
        }
        let page = match p.page {
            PageChoice::Existing(_) => self
                .current_page()
                .map_or(PageChoice::New, |pg| PageChoice::Existing(pg.number)),
            PageChoice::New => PageChoice::New,
        };
        let source = match &p.source {
            BoxSource::PlanView { floor, layer_set } => SendSource::Plan {
                floor: *floor,
                layer_set: layer_set.clone(),
            },
            BoxSource::Camera { camera_id } => SendSource::Camera {
                id: *camera_id,
                name: String::new(),
            },
            BoxSource::Perspective { camera_id } => SendSource::Perspective {
                id: *camera_id,
                name: String::new(),
            },
            _ => return,
        };
        let spec = SendSpec {
            source,
            page,
            scale: p.scale,
            placement: Placement::Click,
        };
        let details = p.details.unwrap_or_else(|| default_details(&spec));
        cx.status = match self.send_with(&mut cx.project, &spec, &details, Some((x, y))) {
            Ok(_) => self.sent_status(),
            Err(e) => e,
        };
    }
}

/// Asks for a file name and writes `bytes` there; returns the status text.
fn save_bytes_as(name: &str, ext: &str, bytes: &[u8]) -> String {
    if cfg!(test) {
        return format!("Saved {} bytes of {ext}", bytes.len());
    }
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_()".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{}.{ext}", stem.trim()))
        .add_filter(ext, &[ext])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    match std::fs::write(&path, bytes) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save: {e}"),
    }
}

// --------------------------------------------------------------- painting --

/// Paper-to-screen mapping of the sheet being drawn.
#[derive(Clone, Copy)]
struct Xf {
    origin: Pos2,
    z: f32,
    h: f64,
}

impl Xf {
    fn pt(&self, x: f64, y: f64) -> Pos2 {
        Pos2::new(
            self.origin.x + x as f32 * self.z,
            self.origin.y + (self.h - y) as f32 * self.z,
        )
    }

    fn paper(&self, p: Pos2) -> (f64, f64) {
        (
            f64::from((p.x - self.origin.x) / self.z),
            self.h - f64::from((p.y - self.origin.y) / self.z),
        )
    }

    fn rect(&self, r: [f64; 4]) -> Rect {
        Rect::from_two_pos(self.pt(r[0], r[1]), self.pt(r[2], r[3]))
    }
}

fn line_width(w: LineWeight, z: f32) -> f32 {
    let k = (z / 20.0).clamp(0.7, 1.8);
    k * match w {
        LineWeight::Heavy => 1.5,
        LineWeight::Medium => 1.0,
        LineWeight::Light => 0.6,
    }
}

thread_local! {
    /// The page being painted and its macros: what a Layout Revision Table
    /// and the macros in a text box depend on ([`box_art`] reads it).
    static PAGE_CTX: RefCell<Option<(u32, MacroContext)>> = const { RefCell::new(None) };
}

/// What a box draws (lines, texts, pictures), from the cache when nothing it
/// depends on changed.
fn box_art<'a>(
    cache: &mut BoxCache,
    rcx: &mut Option<LayoutRenderContext<'a>>,
    project: &'a Project,
    sig: u64,
    b: &LayoutBox,
    layout: &Layout,
) -> Rc<BoxArtwork> {
    let mut key = box_key(b, sig);
    if matches!(b.source, BoxSource::SheetIndex) {
        // The index follows the page titles.
        let mut h = DefaultHasher::new();
        key.hash(&mut h);
        for p in layout.content_pages() {
            (p.number, &p.title).hash(&mut h);
        }
        key = h.finish();
    }
    let page_ctx = PAGE_CTX.with(|c| c.borrow().clone());
    let table_page = matches!(b.source, BoxSource::PageTable | BoxSource::RevisionTable);
    let macro_text = matches!(&b.source, BoxSource::Text { text, .. } if text.contains('%'));
    if table_page || macro_text {
        // The tables follow the pages' information and revisions, and text
        // boxes their macros (which differ from page to page).
        let mut h = DefaultHasher::new();
        key.hash(&mut h);
        if table_page {
            format!("{:?}", layout.page_table_rows()).hash(&mut h);
        }
        if let Some((n, ctx)) = &page_ctx {
            n.hash(&mut h);
            if b.source == BoxSource::RevisionTable {
                format!("{:?}", layout.revision_table_rows(*n)).hash(&mut h);
            }
            if macro_text {
                format!("{ctx:?}").hash(&mut h);
            }
        }
        key = h.finish();
    }
    if let Some((k, art)) = cache.map.get(&b.id) {
        if *k == key {
            return art.clone();
        }
    }
    let rcx = rcx.get_or_insert_with(|| ui_context(project));
    rcx.set_sheet_index(layout);
    if let Some((n, ctx)) = page_ctx {
        rcx.set_current_page(n);
        rcx.set_page_macros(Some(ctx));
    }
    let art = Rc::new(plan_layout::render_box_artwork_in(b, rcx, &layout.layers));
    cache.map.insert(b.id, (key, art.clone()));
    art
}

/// One line of a box's text on screen, turned like the printed page.
fn paint_text(painter: &egui::Painter, xf: &Xf, t: &BoxText, faint: bool) {
    let px = (t.size_pt as f32 / 72.0 * xf.z).max(0.0);
    if px < 3.0 {
        return;
    }
    let mut color = Color32::from_gray((t.gray.clamp(0.0, 1.0) * 255.0) as u8);
    if faint {
        color = color.gamma_multiply(0.45);
    }
    // The text style's installed font, else the bundled one.
    let font = match &t.font {
        Some(spec) => crate::fonts::font_id(painter.ctx(), spec, px),
        None => FontId::proportional(px),
    };
    let at = xf.pt(t.x, t.y);
    if t.angle.abs() < 1e-9 {
        painter.text(
            at + Vec2::new(0.0, px * 0.2),
            Align2::LEFT_BOTTOM,
            &t.text,
            font,
            color,
        );
        return;
    }
    // egui turns text clockwise about its top-left corner; the page turns it
    // counter-clockwise about the baseline's left end.
    let galley = painter.layout_no_wrap(t.text.clone(), font, color);
    let a = -(t.angle as f32);
    let (sin, cos) = a.sin_cos();
    let up = -px * 0.8;
    let corner = at + Vec2::new(-up * sin, up * cos);
    painter.add(egui::epaint::TextShape::new(corner, galley, color).with_angle(a));
}

fn paint_box(
    painter: &egui::Painter,
    xf: &Xf,
    b: &LayoutBox,
    art: &BoxArtwork,
    textures: &mut HashMap<Id, (u64, egui::TextureHandle)>,
    key: u64,
    faint: bool,
) {
    let clip = painter.clip_rect().expand(20.0);
    let tint = |c: Color32| if faint { c.gamma_multiply(0.45) } else { c };
    // Fills that have a color of their own (Color Fill, the box's Fill Style).
    for f in &art.fills {
        if f.polygon.len() >= 3 {
            let pts: Vec<Pos2> = f.polygon.iter().map(|p| xf.pt(p.x, p.y)).collect();
            let [r, g, b] = f.rgb;
            painter.add(egui::Shape::convex_polygon(
                pts,
                tint(Color32::from_rgb(r, g, b)),
                Stroke::NONE,
            ));
        }
    }
    for im in &art.images {
        let fresh = textures.get(&b.id).is_some_and(|(k, _)| *k == key);
        if !fresh && im.width > 0 && im.height > 0 {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [im.width as usize, im.height as usize],
                &im.rgba,
            );
            let tex = painter.ctx().load_texture(
                format!("layout_box_{}", b.id),
                image,
                egui::TextureOptions::LINEAR,
            );
            textures.insert(b.id, (key, tex));
        }
        if let Some((_, tex)) = textures.get(&b.id) {
            painter
                .with_clip_rect(xf.rect(bounds(b)).intersect(clip))
                .image(
                    tex.id(),
                    xf.rect(im.rect_in),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    tint(Color32::WHITE),
                );
        }
    }
    let shapes: Vec<egui::Shape> = art
        .lines
        .iter()
        .filter_map(|l| {
            let (a, c) = (xf.pt(l.a.x, l.a.y), xf.pt(l.b.x, l.b.y));
            Rect::from_two_pos(a, c).intersects(clip).then(|| {
                egui::Shape::line_segment([a, c], st(line_width(l.weight, xf.z), tint(INK)))
            })
        })
        .collect();
    painter.extend(shapes);
    // Text that runs past the frame of a clipping box is cut at the frame.
    let text_painter = if b.clip {
        painter.with_clip_rect(xf.rect(bounds(b)).intersect(clip))
    } else {
        painter.clone()
    };
    for t in &art.texts {
        paint_text(&text_painter, xf, t, faint);
    }
    // The label, its callout shape and the scale note: the same strokes and
    // texts the page prints (under the Layout Box Labels layer).
    if let Some(label) = &art.label {
        for l in &label.lines {
            let (a, c) = (xf.pt(l.a.x, l.a.y), xf.pt(l.b.x, l.b.y));
            painter.line_segment([a, c], st(1.0, tint(INK)));
        }
        for t in &label.texts {
            paint_text(painter, xf, t, faint);
        }
    }
}

/// The stroke of a layout layer on screen: its colour and its weight in points
/// at the zoom (never thinner than a pixel); `None` when the layer is hidden.
fn layer_stroke(layers: &LayoutLayers, name: &str, z: f32) -> Option<Stroke> {
    if !layers.is_visible(name) {
        return None;
    }
    let [r, g, b] = layers.color(name);
    let px = (layers.weight_pt(name) as f32 / 72.0 * z).max(1.0);
    Some(Stroke::new(px, Color32::from_rgb(r, g, b)))
}

fn paint_cad(
    painter: &egui::Painter,
    xf: &Xf,
    o: &CadObject,
    ctx: &MacroContext,
    layers: &LayoutLayers,
) {
    let Some(stroke) = layer_stroke(layers, LayoutLayers::layer_of(o), xf.z) else {
        return;
    };
    let p = |pt: Point| xf.pt(pt.x, pt.y);
    match &o.item {
        CadItem::Line { a, b } => {
            painter.line_segment([p(*a), p(*b)], stroke);
        }
        CadItem::Polyline { points, closed } => {
            let pts: Vec<Pos2> = points.iter().map(|q| p(*q)).collect();
            if *closed {
                painter.add(egui::Shape::closed_line(pts, stroke));
            } else {
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        CadItem::Circle { center, radius } => {
            painter.circle_stroke(p(*center), *radius as f32 * xf.z, stroke);
        }
        CadItem::Arc {
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            let sweep = (end_angle - start_angle).rem_euclid(TAU);
            let n = 32;
            let pts: Vec<Pos2> = (0..=n)
                .map(|i| {
                    let a = start_angle + sweep * f64::from(i) / f64::from(n);
                    p(Point::new(
                        center.x + radius * a.cos(),
                        center.y + radius * a.sin(),
                    ))
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
        }
        CadItem::Text {
            pos, text, height, ..
        } => {
            let px = (*height as f32 * xf.z).max(1.0);
            if px >= 3.0 {
                painter.text(
                    p(*pos),
                    Align2::LEFT_BOTTOM,
                    ctx.expand(text),
                    FontId::proportional(px),
                    stroke.color,
                );
            }
        }
    }
}

/// A leader on screen: line, landing, filled arrowhead and text.
fn paint_leader(
    painter: &egui::Painter,
    xf: &Xf,
    l: &plan_layout::PageLeader,
    ctx: &MacroContext,
    layers: &LayoutLayers,
) {
    if let Some(stroke) = layer_stroke(layers, LAYER_CAD, xf.z) {
        for line in l.polylines() {
            let pts: Vec<Pos2> = line.iter().map(|q| xf.pt(q.x, q.y)).collect();
            painter.add(egui::Shape::line(pts, stroke));
        }
        if let Some(tri) = l.arrowhead() {
            let pts: Vec<Pos2> = tri.iter().map(|q| xf.pt(q.x, q.y)).collect();
            painter.add(egui::Shape::convex_polygon(pts, stroke.color, Stroke::NONE));
        }
    }
    if let Some(stroke) = layer_stroke(layers, LAYER_TEXT, xf.z) {
        let px = (l.height_in as f32 * xf.z).max(1.0);
        if px >= 3.0 {
            let pos = l.text_pos();
            for (i, line) in ctx.expand(&l.text).lines().enumerate() {
                painter.text(
                    xf.pt(pos.x, pos.y - l.height_in * 1.2 * i as f64),
                    Align2::LEFT_BOTTOM,
                    line,
                    FontId::proportional(px),
                    stroke.color,
                );
            }
        }
    }
}

/// A revision cloud on screen: the scalloped loop and its revision tag.
fn paint_cloud(
    painter: &egui::Painter,
    xf: &Xf,
    c: &plan_layout::RevisionCloud,
    layers: &LayoutLayers,
) {
    let Some(stroke) = layer_stroke(layers, LAYER_REVISION_CLOUDS, xf.z) else {
        return;
    };
    let pts: Vec<Pos2> = c.outline().iter().map(|q| xf.pt(q.x, q.y)).collect();
    painter.add(egui::Shape::closed_line(pts, stroke));
    if let Some((tri, mid)) = c.tag() {
        let pts: Vec<Pos2> = tri.iter().map(|q| xf.pt(q.x, q.y)).collect();
        painter.add(egui::Shape::closed_line(
            pts,
            Stroke::new(stroke.width * 0.7, stroke.color),
        ));
        painter.text(
            xf.pt(mid.x, mid.y),
            Align2::CENTER_CENTER,
            &c.revision,
            FontId::proportional((xf.z * 0.1).clamp(7.0, 16.0)),
            stroke.color,
        );
    }
}

/// A page's annotations, each on its layout layer.
fn paint_annotations(
    painter: &egui::Painter,
    xf: &Xf,
    page: &LayoutPage,
    ctx: &MacroContext,
    layers: &LayoutLayers,
) {
    for o in &page.cad {
        paint_cad(painter, xf, o, ctx, layers);
    }
    for l in &page.leaders {
        paint_leader(painter, xf, l, ctx, layers);
    }
    for c in &page.clouds {
        paint_cloud(painter, xf, c, layers);
    }
}

fn paint_field(painter: &egui::Painter, r: Rect, label: &str, value: &str) {
    painter.rect_stroke(r, 0.0, st(0.75, INK), StrokeKind::Inside);
    let small = (r.height() * 0.14).clamp(5.0, 9.0);
    painter.text(
        r.left_top() + Vec2::new(3.0, 2.0),
        Align2::LEFT_TOP,
        label,
        FontId::proportional(small),
        Color32::GRAY,
    );
    let big = (r.height() * 0.28).clamp(7.0, 15.0);
    painter.with_clip_rect(r).text(
        r.left_bottom() + Vec2::new(4.0, -3.0),
        Align2::LEFT_BOTTOM,
        value,
        FontId::proportional(big),
        INK,
    );
}

/// The border and title block of a page, macros expanded for that sheet.
fn paint_title_block(
    painter: &egui::Painter,
    xf: &Xf,
    layout: &Layout,
    ctx: &MacroContext,
    size: (f64, f64),
) {
    let (w, h) = size;
    let m = layout.margins_in;
    let edge =
        (f32::from(u16::try_from(layout.edge_line_weight).unwrap_or(18)) / 100.0 * xf.z * 0.04)
            .clamp(1.0, 3.0);
    painter.rect_stroke(
        xf.rect([m, m, w - m, h - m]),
        0.0,
        st(edge, INK),
        StrokeKind::Middle,
    );
    let fields = layout.title_block.expand_macros(ctx);
    let (lo, hi) = layout.drawing_area_for(size);
    match &layout.title_block.style {
        TitleBlockStyle::RightStrip => {
            let x0 = hi.x;
            let rows = layout.title_block.revision_rows;
            let rev_h = if rows > 0 {
                0.45 + 0.25 * rows as f64
            } else {
                0.0
            };
            let avail = (h - 2.0 * m - rev_h).max(0.0);
            let each = avail / fields.len().max(1) as f64;
            let mut top = h - m;
            for (label, value) in &fields {
                paint_field(painter, xf.rect([x0, top - each, w - m, top]), label, value);
                top -= each;
            }
            if rows > 0 {
                let table = xf.rect([x0, m, w - m, m + rev_h]);
                painter.rect_stroke(table, 0.0, st(0.75, INK), StrokeKind::Inside);
                painter.text(
                    table.left_top() + Vec2::new(3.0, 2.0),
                    Align2::LEFT_TOP,
                    "REVISIONS",
                    FontId::proportional(8.0),
                    Color32::GRAY,
                );
                for (i, (n, d, t)) in ctx.revisions.iter().rev().take(rows).enumerate() {
                    painter.text(
                        xf.pt(x0 + 0.05, m + rev_h - 0.45 - 0.25 * i as f64),
                        Align2::LEFT_TOP,
                        format!("{n}  {d}  {t}"),
                        FontId::proportional((xf.z * 0.13).clamp(6.0, 11.0)),
                        INK,
                    );
                }
            }
        }
        TitleBlockStyle::BottomStrip => {
            let each = (w - 2.0 * m) / fields.len().max(1) as f64;
            for (i, (label, value)) in fields.iter().enumerate() {
                let x = m + each * i as f64;
                paint_field(painter, xf.rect([x, m, x + each, lo.y]), label, value);
            }
        }
        TitleBlockStyle::Custom(items) => {
            let plain = LayoutLayers::default();
            for o in items {
                paint_cad(painter, xf, o, ctx, &plain);
            }
        }
    }
}

impl LayoutView {
    /// Draws page `index`: background, template pages, boxes, page CAD, border
    /// and title block.
    fn paint_page(&mut self, painter: &egui::Painter, xf: &Xf, project: &Project, index: usize) {
        let Some(layout) = self.layout.take() else {
            return;
        };
        self.paint_layout_page(&layout, painter, xf, project, index);
        self.layout = Some(layout);
    }

    fn paint_layout_page(
        &mut self,
        layout: &Layout,
        painter: &egui::Painter,
        xf: &Xf,
        project: &Project,
        index: usize,
    ) {
        let (w, h) = layout
            .pages
            .get(index)
            .map_or_else(|| layout.sheet_inches(), |p| layout.page_sheet_inches(p));
        let sheet = xf.rect([0.0, 0.0, w, h]);
        painter.rect_filled(
            sheet.translate(Vec2::new(3.0, 3.0)),
            0.0,
            Color32::from_black_alpha(90),
        );
        let bg = if layout.page_background {
            let (r, g, b) = CHIEF_SHEET_BACKGROUND;
            Color32::from_rgb(r, g, b)
        } else {
            Color32::WHITE
        };
        painter.rect_filled(sheet, 0.0, bg);
        let Some(page) = layout.pages.get(index) else {
            return;
        };
        let mut ctx = macro_context(project);
        // The sheet number is the page's label; the page macros and the
        // REVISIONS table come with it.
        ctx.apply_page(layout, page);
        ctx.scale = page_scale_label(page);
        ctx.page_count = layout.content_pages().len();
        PAGE_CTX.with(|c| *c.borrow_mut() = Some((page.number, ctx.clone())));
        // The cached drawings depend on the layers too.
        let sig = {
            let mut h = DefaultHasher::new();
            self.sig.hash(&mut h);
            format!("{:?}", layout.layers).hash(&mut h);
            format!("{:?}", opening_labels()).hash(&mut h);
            h.finish()
        };
        let mut cache = std::mem::take(&mut self.cache);
        let mut textures = std::mem::take(&mut self.textures);
        let mut rcx = None;
        let painter = painter.with_clip_rect(painter.clip_rect().intersect(sheet.expand(4.0)));
        for t in layout.templates_for(index) {
            if !std::ptr::eq(t, page) {
                for b in &t.boxes {
                    let art = box_art(&mut cache, &mut rcx, project, sig, b, layout);
                    paint_box(&painter, xf, b, &art, &mut textures, box_key(b, sig), true);
                }
                paint_annotations(&painter, xf, t, &ctx, &layout.layers);
            }
        }
        for b in &page.boxes {
            let art = box_art(&mut cache, &mut rcx, project, sig, b, layout);
            paint_box(&painter, xf, b, &art, &mut textures, box_key(b, sig), false);
        }
        paint_annotations(&painter, xf, page, &ctx, &layout.layers);
        PAGE_CTX.with(|c| *c.borrow_mut() = None);
        self.cache = cache;
        self.textures = textures;
        if layout.layers.is_visible(LAYER_TITLE_BLOCK) && !layout.effective_no_title_block(index) {
            paint_title_block(&painter, xf, layout, &ctx, (w, h));
        }
        if page.template_page {
            painter.text(
                sheet.center(),
                Align2::CENTER_CENTER,
                "PAGE TEMPLATE",
                FontId::proportional((xf.z * 1.2).clamp(14.0, 80.0)),
                Color32::from_black_alpha(28),
            );
        }
    }

    /// The selected plot lines of the Edit Layout Lines tool, in blue.
    fn paint_line_selection(&self, painter: &egui::Painter, xf: &Xf, project: &Project) {
        if self.tool != LayoutTool::EditLines || self.line_sel.is_empty() {
            return;
        }
        let Some((b, map)) = self.plot_box(project) else {
            return;
        };
        let Some(art) = &b.view.art else { return };
        let clip = painter.with_clip_rect(xf.rect(bounds(&b)));
        for l in art.lines.iter().filter(|l| self.line_sel.contains(&l.id)) {
            let (a, c) = (map.to_paper(l.a), map.to_paper(l.b));
            clip.line_segment([xf.pt(a.x, a.y), xf.pt(c.x, c.y)], st(2.5, SELECT_BLUE));
        }
    }

    fn paint_selection(&self, painter: &egui::Painter, xf: &Xf) {
        if let (Some(id), Some(page)) = (self.selected_cad, self.current_page()) {
            if let Some(r) = page.annotation_bounds(id) {
                painter.rect_stroke(
                    xf.rect(r).expand(3.0),
                    0.0,
                    st(1.5, SELECT_BLUE),
                    StrokeKind::Middle,
                );
                for h in Handle::ALL {
                    let (x, y) = h.position(r);
                    let hr = Rect::from_center_size(xf.pt(x, y), Vec2::splat(HANDLE_PX));
                    painter.rect_filled(hr, 0.0, Color32::WHITE);
                    painter.rect_stroke(hr, 0.0, st(1.0, SELECT_BLUE), StrokeKind::Inside);
                }
            }
        }
        if let Some(page) = self.current_page() {
            for b in page.boxes.iter().filter(|b| self.also.contains(&b.id)) {
                painter.rect_stroke(
                    xf.rect(bounds(b)),
                    0.0,
                    st(1.5, SELECT_BLUE),
                    StrokeKind::Outside,
                );
            }
        }
        let Some(b) = self.selected_box() else { return };
        let r = bounds(b);
        painter.rect_stroke(xf.rect(r), 0.0, st(1.5, SELECT_BLUE), StrokeKind::Outside);
        let c = content_bounds(b);
        if c != r {
            // The turned content (width and height swapped), dashed.
            let corners = {
                let rc = xf.rect(c);
                [
                    rc.left_top(),
                    rc.right_top(),
                    rc.right_bottom(),
                    rc.left_bottom(),
                ]
            };
            painter.extend(egui::Shape::dashed_line(
                &[corners[0], corners[1], corners[2], corners[3], corners[0]],
                st(1.0, SELECT_BLUE),
                5.0,
                3.0,
            ));
        }
        for h in Handle::ALL {
            let (x, y) = h.position(r);
            let c = xf.pt(x, y);
            let hr = Rect::from_center_size(c, Vec2::splat(HANDLE_PX));
            painter.rect_filled(hr, 0.0, Color32::WHITE);
            painter.rect_stroke(hr, 0.0, st(1.0, SELECT_BLUE), StrokeKind::Inside);
        }
        // A view with no scale reads out how big it is drawn at the corner
        // handle (Chief shows the factor while you resize it).
        if b.is_no_scale() {
            let (px, py) = Handle::NE.position(r);
            painter.text(
                xf.pt(px, py) + Vec2::new(8.0, -4.0),
                Align2::LEFT_BOTTOM,
                format!("not to scale: {:.3}\" = 1'", b.effective_ipf()),
                FontId::proportional(11.0),
                SELECT_BLUE,
            );
        }
        // The rotate knob above the top edge of what the box covers.
        let hit = hit_bounds(b);
        let (kx, ky) = rotate_knob(hit, xf.z);
        let knob = xf.pt(kx, ky);
        painter.line_segment([xf.pt(kx, hit[3]), knob], st(1.0, SELECT_BLUE));
        painter.circle_filled(knob, HANDLE_PX * 0.7, Color32::WHITE);
        painter.circle_stroke(knob, HANDLE_PX * 0.7, st(1.5, SELECT_BLUE));
    }
}

// ------------------------------------------------------------------ view --

/// Draws the layout view in the main area.
pub fn show_central(ctx: &egui::Context, ui: &mut Ui, cx: &mut EditorContext) {
    let mut view = with_view(std::mem::take);
    view.show(ctx, ui, cx);
    view.flush(cx);
    with_view(|slot| *slot = view);
}

impl LayoutView {
    fn show(&mut self, ctx: &egui::Context, ui: &mut Ui, cx: &mut EditorContext) {
        self.sync(&cx.project);
        if self.layout.is_none() {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.heading("This plan has no layout yet");
                if ui.button("New Layout").clicked() {
                    self.ensure(cx);
                }
                if ui.button("Back to the plan").clicked() {
                    self.active = false;
                }
            });
            return;
        }
        let mut cmds: Vec<LayoutCommand> = Vec::new();
        egui::TopBottomPanel::top("layout_tools").show_inside(ui, |ui| {
            self.toolbar(ui, cx, &mut cmds);
        });
        egui::TopBottomPanel::top("layout_draw_tools").show_inside(ui, |ui| {
            self.tool_row(ui, &mut cmds);
        });
        egui::TopBottomPanel::top("layout_arrange_tools").show_inside(ui, |ui| {
            self.arrange_row(ui, cx, &mut cmds);
        });
        egui::TopBottomPanel::top("layout_view_tools").show_inside(ui, |ui| {
            self.view_row(ui, &mut cmds);
        });
        egui::TopBottomPanel::bottom("layout_tabs").show_inside(ui, |ui| {
            self.page_tabs(ui, cx, &mut cmds);
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show_inside(ui, |ui| self.sheet_view(ctx, ui, cx));
        if !cmds.is_empty() {
            ctx.request_repaint();
        }
        for c in cmds {
            self.run(cx, c, None);
        }
    }

    fn toolbar(&mut self, ui: &mut Ui, cx: &EditorContext, out: &mut Vec<LayoutCommand>) {
        use LayoutCommand as C;
        let _ = cx;
        egui::ScrollArea::horizontal()
            .id_salt("layout_toolbar")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut btn = |ui: &mut Ui, text: &str, tip: &str, c: C, on: bool| {
                        if ui
                            .add_enabled(on, egui::Button::new(text))
                            .on_hover_text(tip)
                            .on_disabled_hover_text(tip)
                            .clicked()
                        {
                            out.push(c);
                        }
                    };
                    btn(ui, "Plan", "Back to the floor plan", C::ShowPlan, true);
                    ui.separator();
                    btn(
                        ui,
                        "Send to Layout",
                        "Send the current plan view",
                        C::SendToLayout,
                        true,
                    );
                    btn(
                        ui,
                        "Box Specification",
                        "Layout Box Specification (double-click a box)",
                        C::BoxSpecification,
                        self.selected.is_some(),
                    );
                    btn(
                        ui,
                        "Delete Box",
                        "Delete the selected layout box",
                        C::DeleteBox,
                        self.selected.is_some(),
                    );
                    ui.separator();
                    btn(
                        ui,
                        "Page Before",
                        "Insert Page Before",
                        C::InsertPageBefore,
                        true,
                    );
                    btn(
                        ui,
                        "Page After",
                        "Insert Page After",
                        C::InsertPageAfter,
                        true,
                    );
                    btn(ui, "Duplicate", "Duplicate Page", C::DuplicatePage, true);
                    btn(ui, "Delete Page", "Delete Page", C::DeletePage, true);
                    btn(
                        ui,
                        "\u{25C0}",
                        "Exchange With Previous Page",
                        C::ExchangeWithPrevious,
                        self.page > 0,
                    );
                    btn(
                        ui,
                        "\u{25B6}",
                        "Exchange With Next Page",
                        C::ExchangeWithNext,
                        self.layout
                            .as_ref()
                            .is_some_and(|l| self.page + 1 < l.pages.len()),
                    );
                    ui.separator();
                    btn(
                        ui,
                        "Page Table",
                        "Layout Page Table: click a page to place it",
                        C::PageTable,
                        true,
                    );
                    btn(
                        ui,
                        "Update Views",
                        "Update Layout Views",
                        C::UpdateViews,
                        true,
                    );
                    btn(ui, "Page Setup", "Page Setup", C::PageSetup, true);
                    btn(
                        ui,
                        "Project Info",
                        "Project Information",
                        C::ProjectInfo,
                        true,
                    );
                    ui.separator();
                    btn(
                        ui,
                        "Undo",
                        "Undo the last edit",
                        C::Undo,
                        cx.can_undo() || !self.steps.is_empty(),
                    );
                    btn(ui, "Redo", "Redo", C::Redo, cx.can_redo());
                    btn(ui, "Fit", "Fit the page in the window", C::FitPage, true);
                    btn(ui, "Print", "Print Layout", C::Print, true);
                    ui.label(format!("{:.0}%", self.zoom / 0.96));
                    if let Some(job) = &self.update {
                        ui.add(
                            egui::ProgressBar::new(job.done as f32 / job.total.max(1) as f32)
                                .desired_width(90.0)
                                .text(format!("Views {}/{}", job.done, job.total)),
                        );
                    }
                });
            });
    }

    /// The second toolbar row: the drawing tools (layout CAD and text boxes),
    /// the boxes that are not views, rotation and printing.
    fn tool_row(&mut self, ui: &mut Ui, out: &mut Vec<LayoutCommand>) {
        use LayoutCommand as C;
        egui::ScrollArea::horizontal()
            .id_salt("layout_tool_row")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Tool:");
                    for t in LayoutTool::ALL {
                        let tip = match t {
                            LayoutTool::Select => "Select, move and resize boxes and page drawings",
                            LayoutTool::Line => "Draw a line on the page",
                            LayoutTool::Box => "Draw a rectangle on the page",
                            LayoutTool::Polyline => "Draw a polyline (double-click to finish)",
                            LayoutTool::Text => "Place a line of text on the page",
                            LayoutTool::TextBox => "Draw a text box",
                            LayoutTool::Circle => "Draw a circle from its centre",
                            LayoutTool::Arc => "Draw an arc: centre, start, end",
                            LayoutTool::Leader => "Draw a leader: text with an arrow",
                            LayoutTool::Cloud => "Draw a revision cloud around an area",
                            LayoutTool::PanScale => "Pan/Scale Layout Box",
                            LayoutTool::EditLines => "Edit Layout Lines",
                            LayoutTool::PointToPoint => "Point to Point Move",
                        };
                        if ui
                            .selectable_label(self.tool == t, t.name())
                            .on_hover_text(tip)
                            .clicked()
                        {
                            out.push(C::Tool(t));
                        }
                    }
                    ui.separator();
                    for (text, tip, c) in [
                        ("Add Text Box", "Add a text box to the page", C::AddTextBox),
                        (
                            "Add Materials List",
                            "Add the Materials List as a table",
                            C::AddMaterialsBox,
                        ),
                        ("Add Picture", "Add a picture file", C::AddImageBox),
                        (
                            "Add Sheet Index",
                            "Add the sheet index as a table",
                            C::AddSheetIndex,
                        ),
                        (
                            "Layers\u{2026}",
                            "Layout Layer Display Options",
                            C::LayerDisplay,
                        ),
                        (
                            "Construction Set",
                            "Add Daniel's sheet set to the layout",
                            C::CreateConstructionSet,
                        ),
                        (
                            "Save Template\u{2026}",
                            "Save this layout as a template",
                            C::SaveAsTemplate,
                        ),
                        (
                            "Apply Template\u{2026}",
                            "Replace this layout with a saved template",
                            C::ApplyTemplate,
                        ),
                    ] {
                        if ui.button(text).on_hover_text(tip).clicked() {
                            out.push(c);
                        }
                    }
                    if ui
                        .add_enabled(self.selected.is_some(), egui::Button::new("Rotate"))
                        .on_hover_text("Turn the selected box's content a quarter turn")
                        .clicked()
                    {
                        out.push(C::RotateBox);
                    }
                    ui.separator();
                    if ui
                        .button("Print\u{2026}")
                        .on_hover_text("Print the layout")
                        .clicked()
                    {
                        out.push(C::PrintDialog);
                    }
                    if ui
                        .button("Print Image\u{2026}")
                        .on_hover_text("Save the plan view as a PNG")
                        .clicked()
                    {
                        out.push(C::PrintImage);
                    }
                    if ui
                        .button("Print Model\u{2026}")
                        .on_hover_text("Render a perspective camera at a chosen DPI onto the paper")
                        .clicked()
                    {
                        out.push(C::PrintModel);
                    }
                });
            });
    }

    /// The fourth toolbar row: the edit tools of a layout view (Rescale
    /// Layout View, Pan/Scale, Recenter, Scale to Fit, Update View, Layout Box
    /// Layers, Unlink Saved Plan View, Edit Layout Lines) and Send All Views.
    fn view_row(&mut self, ui: &mut Ui, out: &mut Vec<LayoutCommand>) {
        use LayoutCommand as C;
        let picked = self.selection_ids().len();
        let scaled = self.selected_box().is_some_and(|b| is_scaled(&b.source));
        let plan = self
            .selected_box()
            .is_some_and(|b| matches!(b.source, BoxSource::PlanView { .. }));
        let linked = self
            .selected_box()
            .is_some_and(|b| b.view.saved_view.is_some());
        let local: RefCell<Vec<C>> = RefCell::new(Vec::new());
        egui::ScrollArea::horizontal()
            .id_salt("layout_view_row")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("View:");
                    let btn = |ui: &mut Ui, text: &str, tip: &str, c: C, on: bool| {
                        if ui
                            .add_enabled(on, egui::Button::new(text))
                            .on_hover_text(tip)
                            .on_disabled_hover_text(tip)
                            .clicked()
                        {
                            local.borrow_mut().push(c);
                        }
                    };
                    btn(
                        ui,
                        "Rescale\u{2026}",
                        "Rescale Layout View: Change Scale",
                        C::RescaleView,
                        scaled,
                    );
                    btn(
                        ui,
                        "Recenter",
                        "Recenter Layout Box Contents",
                        C::RecenterBox,
                        scaled,
                    );
                    btn(
                        ui,
                        "Scale to Fit",
                        "Scale Layout Box Contents to Fit",
                        C::ScaleBoxToFit,
                        scaled,
                    );
                    btn(
                        ui,
                        "Update View",
                        "Update the selected semi-dynamic or Plot Lines view",
                        C::UpdateView,
                        picked > 0,
                    );
                    btn(
                        ui,
                        "Layers\u{2026}",
                        "Layout Box Layers: the layer set of the selected plan view",
                        C::LayoutBoxLayers,
                        plan,
                    );
                    btn(
                        ui,
                        "Unlink Saved View",
                        "Unlink Saved Plan View",
                        C::UnlinkSavedView,
                        linked,
                    );
                    btn(
                        ui,
                        "Center Object",
                        "Center the selection in the page's drawing area",
                        C::CenterObject,
                        picked > 0 || self.selected_cad.is_some(),
                    );
                    ui.separator();
                    for (t, text, tip, on) in [
                        (
                            LayoutTool::PointToPoint,
                            "Point to Point Move",
                            "Move the selection from one point to another",
                            picked > 0 || self.selected_cad.is_some(),
                        ),
                        (
                            LayoutTool::PanScale,
                            "Pan/Scale",
                            "Pan/Scale Layout Box: drag to pan, type a scale",
                            scaled,
                        ),
                        (
                            LayoutTool::EditLines,
                            "Edit Layout Lines",
                            "Select, draw and delete the lines of a Plot Lines view",
                            true,
                        ),
                    ] {
                        let r = ui
                            .add_enabled_ui(on, |ui| ui.selectable_label(self.tool == t, text))
                            .inner
                            .on_hover_text(tip);
                        if r.clicked() {
                            local.borrow_mut().push(C::Tool(if self.tool == t {
                                LayoutTool::Select
                            } else {
                                t
                            }));
                        }
                    }
                    ui.separator();
                    btn(
                        ui,
                        "Update Live Views",
                        "Update All Live Views (Update on Demand)",
                        C::UpdateLiveViews,
                        true,
                    );
                    btn(
                        ui,
                        "Update Plot Line Views",
                        "Update All Plot Line Views",
                        C::UpdatePlotLineViews,
                        true,
                    );
                    btn(
                        ui,
                        "Send All Views\u{2026}",
                        "Send every floor plan and elevation to the layout",
                        C::SendAllViews,
                        true,
                    );
                });
            });
        out.extend(local.into_inner());
    }

    /// The third toolbar row: the plan's layout files, Page Specification and
    /// sheet sizes, aligning, spreading and copying boxes, and the tables to
    /// Excel.
    fn arrange_row(&mut self, ui: &mut Ui, cx: &EditorContext, out: &mut Vec<LayoutCommand>) {
        use LayoutCommand as C;
        let names = layout_names(&cx.project);
        let picked = self.selection_ids().len();
        egui::ScrollArea::horizontal()
            .id_salt("layout_arrange_row")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Layout file:");
                    egui::ComboBox::from_id_salt("layout_file_picker")
                        .selected_text(names.first().cloned().unwrap_or_default())
                        .show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                if ui.selectable_label(i == 0, n).clicked() && i > 0 {
                                    out.push(C::SwitchLayout(i));
                                }
                            }
                        });
                    if ui
                        .button("New Layout File\u{2026}")
                        .on_hover_text("Make another layout file in this plan and open it")
                        .clicked()
                    {
                        out.push(C::NewLayoutFile);
                    }
                    ui.separator();
                    if ui
                        .button("Page Information\u{2026}")
                        .on_hover_text("Label, title, description, page templates, revisions and sheet size of this page")
                        .clicked()
                    {
                        out.push(C::PageInformation);
                    }
                    if ui
                        .button("Add Revision\u{2026}")
                        .on_hover_text("Add a layout revision to chosen pages")
                        .clicked()
                    {
                        out.push(C::AddLayoutRevision);
                    }
                    if ui
                        .button("Revision Table")
                        .on_hover_text("Click a page to place the table of that page's revisions")
                        .clicked()
                    {
                        out.push(C::RevisionTable);
                    }
                    if ui
                        .button("Layout Defaults\u{2026}")
                        .on_hover_text("General Layout Defaults: Use Snap Grid and the Grid Snap Unit")
                        .clicked()
                    {
                        out.push(C::LayoutDefaults);
                    }
                    if ui
                        .button("Sheet Sizes\u{2026}")
                        .on_hover_text("Customize Sheet Sizes")
                        .clicked()
                    {
                        out.push(C::CustomizeSheetSizes);
                    }
                    ui.separator();
                    ui.label("Align:");
                    for (edge, text, tip) in [
                        (AlignEdge::Left, "Left", "Line the selected boxes up on their left edge"),
                        (AlignEdge::HCenter, "Center", "Line up their centers on a vertical line"),
                        (AlignEdge::Right, "Right", "Line up on the right edge"),
                        (AlignEdge::Top, "Top", "Line up on the top edge"),
                        (AlignEdge::VCenter, "Middle", "Line up their middles on a horizontal line"),
                        (AlignEdge::Bottom, "Bottom", "Line up on the bottom edge"),
                    ] {
                        if ui
                            .add_enabled(picked > 0, egui::Button::new(text))
                            .on_hover_text(format!(
                                "{tip} (one box lines up with the drawing area; Shift-click selects several)"
                            ))
                            .clicked()
                        {
                            out.push(C::Align(edge));
                        }
                    }
                    for (axis, text, tip) in [
                        (Spread::Horizontal, "Spread H", "Equal gaps between boxes, left to right"),
                        (Spread::Vertical, "Spread V", "Equal gaps between boxes, bottom to top"),
                    ] {
                        if ui
                            .add_enabled(picked >= 3, egui::Button::new(text))
                            .on_hover_text(format!("{tip} (three or more boxes)"))
                            .clicked()
                        {
                            out.push(C::Distribute(axis));
                        }
                    }
                    ui.separator();
                    if ui
                        .add_enabled(picked > 0, egui::Button::new("Copy to Page\u{2026}"))
                        .on_hover_text("Copy the selected boxes to another page")
                        .clicked()
                    {
                        out.push(C::CopyBoxToPage);
                    }
                    if ui
                        .add_enabled(picked == 1, egui::Button::new("Open Source View"))
                        .on_hover_text("Go back to the floor plan this box shows")
                        .clicked()
                    {
                        out.push(C::OpenSourceView);
                    }
                    if ui
                        .add_enabled(picked > 0, egui::Button::new("Duplicate Box"))
                        .on_hover_text("Copy the selected boxes on this page")
                        .clicked()
                    {
                        out.push(C::DuplicateBox);
                    }
                    ui.separator();
                    if ui
                        .button("Export CSV\u{2026}")
                        .on_hover_text("Save the selected table (or the tables on this page) as CSV")
                        .clicked()
                    {
                        out.push(C::ExportTableCsv);
                    }
                    if ui
                        .button("Export Excel\u{2026}")
                        .on_hover_text("Save the selected table (or the tables on this page) as an Excel workbook")
                        .clicked()
                    {
                        out.push(C::ExportTableExcel);
                    }
                });
            });
    }

    fn page_tabs(&mut self, ui: &mut Ui, cx: &mut EditorContext, out: &mut Vec<LayoutCommand>) {
        use LayoutCommand as C;
        let titles: Vec<(String, bool)> = self
            .layout
            .as_ref()
            .map(|l| {
                l.pages
                    .iter()
                    .zip(l.page_labels())
                    .map(|(p, label)| {
                        let name = if p.template_page {
                            format!("{} (template)", p.title)
                        } else {
                            format!("{label}  {}", p.title)
                        };
                        (name, p.template_page)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut rename: Option<(usize, String)> = None;
        egui::ScrollArea::horizontal()
            .id_salt("layout_page_tabs")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, (name, template)) in titles.iter().enumerate() {
                        if let Some((ri, text)) = self.renaming.as_mut().filter(|r| r.0 == i) {
                            let _ = ri;
                            let r = ui.add(egui::TextEdit::singleline(text).desired_width(140.0));
                            r.request_focus();
                            if r.lost_focus() {
                                rename = Some((i, text.clone()));
                            }
                            continue;
                        }
                        let mut label = egui::RichText::new(name);
                        if *template {
                            label = label.italics();
                        }
                        let r = ui.selectable_label(i == self.page, label);
                        if r.clicked() {
                            out.push(C::GoToPage(i));
                        }
                        if r.double_clicked() {
                            let title = self
                                .layout
                                .as_ref()
                                .and_then(|l| l.pages.get(i))
                                .map(|p| p.title.clone())
                                .unwrap_or_default();
                            self.renaming = Some((i, title));
                        }
                        r.context_menu(|ui| {
                            for (text, c) in [
                                ("Edit Page Information...", C::PageInformation),
                                ("Copy Drawings to Page...", C::CopyDrawingsToPage),
                                ("Insert Page Before", C::InsertPageBefore),
                                ("Insert Page After", C::InsertPageAfter),
                                ("Duplicate Page", C::DuplicatePage),
                                ("Exchange With Previous Page", C::ExchangeWithPrevious),
                                ("Exchange With Next Page", C::ExchangeWithNext),
                                ("Delete Page", C::DeletePage),
                            ] {
                                if ui.button(text).clicked() {
                                    out.push(C::GoToPage(i));
                                    out.push(c);
                                    ui.close_menu();
                                }
                            }
                        });
                    }
                    if ui.button("+").on_hover_text("Add a page").clicked() {
                        let last = titles.len().saturating_sub(1);
                        out.push(C::GoToPage(last));
                        out.push(C::InsertPageAfter);
                    }
                });
            });
        if let Some((i, title)) = rename {
            self.renaming = None;
            if self.rename_page(&mut cx.project, i, &title) {
                cx.status = "Renamed the page".into();
            }
        }
    }

    fn sheet_view(&mut self, ctx: &egui::Context, ui: &mut Ui, cx: &mut EditorContext) {
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        if self.layout.is_none() {
            return;
        }
        let (_, sheet_h) = self.sheet_of(self.page);
        if self.fit_pending {
            self.fit(rect);
        }
        let pointer = resp.hover_pos();
        // Zoom: wheel / pinch about the pointer.
        if let Some(pos) = pointer {
            let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll * 0.005).exp() * pinch;
            if (factor - 1.0).abs() > 1e-4 {
                self.zoom_about(rect, pos, factor);
            }
        }
        let xf = Xf {
            origin: rect.min + self.offset,
            z: self.zoom,
            h: sheet_h,
        };
        self.last_xf = Some(xf);
        if self.drag.is_none() {
            self.sig = project_sig(cx);
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, SURROUND);
        let page = self.page;
        self.paint_page(&painter, &xf, &cx.project, page);
        self.paint_line_selection(&painter, &xf, &cx.project);
        self.paint_selection(&painter, &xf);
        self.interact(ctx, ui, cx, &resp, &xf, &painter);
    }

    fn interact(
        &mut self,
        ctx: &egui::Context,
        ui: &mut Ui,
        cx: &mut EditorContext,
        resp: &egui::Response,
        xf: &Xf,
        painter: &egui::Painter,
    ) {
        let snapping = !ui.input(|i| i.modifiers.alt);
        let tol = f64::from(HANDLE_PX) / f64::from(xf.z);

        // Cursor feedback and the placement ghost.
        if let Some(pos) = resp.hover_pos() {
            if let Some(b) = self.selected_box() {
                let (x, y) = xf.paper(pos);
                if let Some(h) = handle_at(bounds(b), x, y, tol) {
                    ctx.set_cursor_icon(h.cursor());
                }
            }
            if let Some(r) = self.selected_cad_bounds() {
                let (x, y) = xf.paper(pos);
                if let Some(h) = handle_at(r, x, y, tol) {
                    ctx.set_cursor_icon(h.cursor());
                }
            }
            if self.placing.is_some() || self.tool != LayoutTool::Select {
                ctx.set_cursor_icon(CursorIcon::Crosshair);
            }
        }

        // Starting a drag.
        if resp.drag_started() {
            let origin = ui.input(|i| i.pointer.press_origin());
            let primary = ui.input(|i| i.pointer.primary_down());
            let drawing = matches!(
                self.tool,
                LayoutTool::Line
                    | LayoutTool::Box
                    | LayoutTool::TextBox
                    | LayoutTool::Circle
                    | LayoutTool::Leader
                    | LayoutTool::Cloud
            );
            if let (Some(pos), true, true) = (origin, primary && self.placing.is_none(), drawing) {
                let (x, y) = xf.paper(pos);
                let p = self.snap_xy(x, y, snapping, tol);
                self.drag = Some(Drag::Draw { start: p, cur: p });
            } else if let (Some(pos), true, true) = (
                origin,
                primary && self.placing.is_none(),
                self.tool == LayoutTool::PanScale,
            ) {
                // Pan/Scale Layout Box: drag the contents of the view under
                // the pointer.
                let (x, y) = xf.paper(pos);
                let target = self.hit(xf, pos).filter(|id| {
                    self.current_page()
                        .and_then(|p| p.boxes.iter().find(|b| b.id == *id))
                        .is_some_and(|b| is_scaled(&b.source))
                });
                match (target, self.layout.clone()) {
                    (Some(id), Some(before)) => {
                        self.selected = Some(id);
                        let pan0 = self.selected_box().map_or((0.0, 0.0), |b| b.view.pan_in);
                        self.drag = Some(Drag::PanBox {
                            id,
                            start: (x, y),
                            pan0,
                            before: Box::new(before),
                        });
                    }
                    _ => self.drag = Some(Drag::Pan),
                }
            } else if let (Some(pos), true, true) = (
                origin,
                primary && self.placing.is_none(),
                self.tool == LayoutTool::EditLines,
            ) {
                // Edit Layout Lines: drag a selected line to move it, hold
                // Shift for a marquee, else draw a new line in the view.
                let (x, y) = xf.paper(pos);
                let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
                let shift = ui.input(|i| i.modifiers.shift);
                if let Some(id) = self.hit(xf, pos) {
                    if Some(id) != self.selected {
                        self.selected = Some(id);
                        self.line_sel.clear();
                    }
                }
                match (self.plot_box(&cx.project), self.layout.clone()) {
                    (Some((b, map)), Some(before)) => {
                        let at = map.to_source(Point::new(x, y));
                        let k = (b.points_per_inch() / 72.0).max(1e-9);
                        let on_selected = b.view.art.as_ref().is_some_and(|a| {
                            a.hit_line(at, tol / k)
                                .is_some_and(|l| self.line_sel.contains(&l))
                        });
                        if on_selected {
                            self.drag = Some(Drag::MoveLines {
                                id: b.id,
                                start: at,
                                orig: Box::new(b.view.art.clone().unwrap_or_default()),
                                before: Box::new(before),
                            });
                        } else if shift {
                            self.drag = Some(Drag::Marquee {
                                id: b.id,
                                start: at,
                                cur: at,
                            });
                        } else {
                            self.drag = Some(Drag::DrawLine {
                                id: b.id,
                                start: at,
                                cur: at,
                                before: Box::new(before),
                            });
                        }
                    }
                    _ => self.drag = Some(Drag::Pan),
                }
            } else if let (Some(pos), true) = (
                origin,
                primary && self.placing.is_none() && self.tool == LayoutTool::Select,
            ) {
                let (x, y) = xf.paper(pos);
                let handle = self
                    .selected_box()
                    .and_then(|b| handle_at(bounds(b), x, y, tol).map(|h| (b.id, bounds(b), h)));
                let cad_handle = self
                    .selected_cad
                    .zip(self.selected_cad_bounds())
                    .and_then(|(id, r)| handle_at(r, x, y, tol).map(|h| (id, r, h)));
                let cad_hit = self
                    .current_page()
                    .and_then(|p| cad_at(p, x, y, tol))
                    .filter(|_| handle.is_none() && cad_handle.is_none());
                if let (Some((id, r, h)), Some(before), Some(orig)) =
                    (cad_handle, self.layout.clone(), self.annotations_only())
                {
                    self.drag = Some(Drag::ResizeCad {
                        id,
                        handle: h,
                        start: (x, y),
                        orig: Box::new(orig),
                        orig_bounds: r,
                        before: Box::new(before),
                    });
                } else if let (Some(id), Some(before), Some(orig)) =
                    (cad_hit, self.layout.clone(), self.annotations_only())
                {
                    // A drawing that is one of several selected moves them
                    // all (and the selected boxes); any other becomes the
                    // selection.
                    let in_group = self.cad_selection_ids().contains(&id);
                    let (with_cad, with_boxes) = if in_group {
                        let boxes = self.selection_ids();
                        (
                            self.cad_selection_ids()
                                .into_iter()
                                .filter(|c| *c != id)
                                .collect(),
                            self.current_page()
                                .map(|p| {
                                    p.boxes
                                        .iter()
                                        .filter(|b| boxes.contains(&b.id))
                                        .map(|b| (b.id, bounds(b)))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        )
                    } else {
                        self.selected_cad = Some(id);
                        self.selected = None;
                        self.also.clear();
                        self.also_cad.clear();
                        (Vec::new(), Vec::new())
                    };
                    self.drag = Some(Drag::MoveCad {
                        id,
                        start: (x, y),
                        orig: Box::new(orig),
                        with_cad,
                        with_boxes,
                        before: Box::new(before),
                    });
                } else if let (Some((id, orig, h)), Some(before)) = (handle, self.layout.clone()) {
                    self.drag = Some(Drag::Resize {
                        id,
                        handle: h,
                        start: (x, y),
                        orig,
                        before: Box::new(before),
                    });
                } else if let (Some(id), Some(before), Some(cad_orig)) = (
                    self.hit(xf, pos),
                    self.layout.clone(),
                    self.annotations_only().map(Box::new),
                ) {
                    // Dragging a box that is one of several selected moves
                    // them all; any other box becomes the selection.
                    let in_group = self.selection_ids().contains(&id);
                    if !in_group {
                        self.also.clear();
                        self.selected_cad = None;
                        self.also_cad.clear();
                    }
                    self.selected = if in_group { self.selected } else { Some(id) };
                    let page = self.current_page();
                    let orig = page
                        .and_then(|p| p.boxes.iter().find(|b| b.id == id))
                        .map_or([0.0; 4], bounds);
                    let group: Vec<(Id, [f64; 4])> = if in_group {
                        page.map(|p| {
                            p.boxes
                                .iter()
                                .filter(|b| b.id != id && self.selection_ids().contains(&b.id))
                                .map(|b| (b.id, bounds(b)))
                                .collect()
                        })
                        .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    let cad = if in_group {
                        self.cad_selection_ids()
                    } else {
                        Vec::new()
                    };
                    self.drag = Some(Drag::Move {
                        id,
                        start: (x, y),
                        orig,
                        group,
                        cad,
                        cad_orig,
                        before: Box::new(before),
                    });
                } else {
                    self.drag = Some(Drag::Pan);
                }
            } else {
                self.drag = Some(Drag::Pan);
            }
        }
        // Dragging.
        if resp.dragged() {
            let cur = ui.input(|i| i.pointer.interact_pos());
            match (self.drag.clone(), cur) {
                (Some(Drag::Pan), _) => self.offset += ui.input(|i| i.pointer.delta()),
                (
                    Some(Drag::Move {
                        id,
                        start,
                        orig,
                        group,
                        cad,
                        cad_orig,
                        ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    let to = moved(orig, x - start.0, y - start.1, snapping);
                    self.live_bounds(&mut cx.project, id, to);
                    // The rest of the selection keeps its place around it:
                    // the other boxes and the page drawings.
                    let (dx, dy) = (to[0] - orig[0], to[1] - orig[1]);
                    self.move_group_live(&mut cx.project, &group, &cad, &cad_orig, (dx, dy));
                }
                (
                    Some(Drag::Resize {
                        id,
                        handle,
                        start,
                        orig,
                        ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    let to = resized(orig, handle, x - start.0, y - start.1, snapping);
                    // The Alternate edit behavior on a corner handle of a
                    // box with no scale resizes the view with the border.
                    let alternate = ui.input(|i| i.modifiers.command);
                    let corner =
                        matches!(handle, Handle::NE | Handle::NW | Handle::SE | Handle::SW);
                    let first = match &self.drag {
                        Some(Drag::Resize { before, .. }) => before
                            .pages
                            .iter()
                            .flat_map(|p| p.boxes.iter())
                            .find(|b| b.id == id)
                            .cloned(),
                        _ => None,
                    };
                    match first {
                        Some(b0) if alternate && corner && b0.is_no_scale() => {
                            self.live_resize_no_scale(&mut cx.project, id, &b0, to);
                        }
                        _ => self.live_bounds(&mut cx.project, id, to),
                    }
                }
                (Some(Drag::Draw { start, .. }), Some(pos)) => {
                    let (x, y) = xf.paper(pos);
                    self.drag = Some(Drag::Draw {
                        start,
                        cur: self.snap_xy(x, y, snapping, tol),
                    });
                }
                (
                    Some(Drag::PanBox {
                        id, start, pan0, ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    self.live_pan(&mut cx.project, id, pan0, (x - start.0, y - start.1));
                }
                (
                    Some(Drag::DrawLine {
                        id, start, before, ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    if let Some((_, map)) = self.plot_box(&cx.project) {
                        self.drag = Some(Drag::DrawLine {
                            id,
                            start,
                            cur: map.to_source(Point::new(x, y)),
                            before,
                        });
                    }
                }
                (Some(Drag::Marquee { id, start, .. }), Some(pos)) => {
                    let (x, y) = xf.paper(pos);
                    if let Some((_, map)) = self.plot_box(&cx.project) {
                        self.drag = Some(Drag::Marquee {
                            id,
                            start,
                            cur: map.to_source(Point::new(x, y)),
                        });
                    }
                }
                (
                    Some(Drag::MoveLines {
                        id, start, orig, ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    if let Some((_, map)) = self.plot_box(&cx.project) {
                        let at = map.to_source(Point::new(x, y));
                        let d = Point::new(at.x - start.x, at.y - start.y);
                        self.live_move_lines(&mut cx.project, id, &orig, d);
                    }
                }
                (
                    Some(Drag::MoveCad {
                        id,
                        start,
                        orig,
                        with_cad,
                        with_boxes,
                        ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    let (dx, dy) = (snap(x - start.0, snapping), snap(y - start.1, snapping));
                    let mut all = with_cad;
                    all.push(id);
                    self.move_group_live(&mut cx.project, &with_boxes, &all, &orig, (dx, dy));
                }
                (
                    Some(Drag::ResizeCad {
                        id,
                        handle,
                        start,
                        orig,
                        orig_bounds,
                        ..
                    }),
                    Some(pos),
                ) => {
                    let (x, y) = xf.paper(pos);
                    let r = resized(orig_bounds, handle, x - start.0, y - start.1, snapping);
                    self.live_annotation(&mut cx.project, &orig, |p| {
                        p.resize_annotation(id, r);
                    });
                }
                _ => {}
            }
        }
        if resp.drag_stopped() {
            match self.drag.take() {
                Some(Drag::Move { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Move Layout Box")
                }
                Some(Drag::Resize { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Resize Layout Box");
                }
                Some(Drag::MoveCad { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Move Layout Drawing");
                }
                Some(Drag::ResizeCad { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Resize Layout Drawing");
                }
                Some(Drag::Draw { start, cur }) => self.finish_draw(cx, start, cur),
                Some(Drag::PanBox { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Pan Layout Box");
                }
                Some(Drag::MoveLines { before, .. }) => {
                    self.finish_drag(&mut cx.project, *before, "Move Layout Lines");
                }
                Some(Drag::DrawLine { start, cur, .. }) => {
                    if start.dist(cur) > 1e-6
                        && self.add_plot_line(&mut cx.project, start, cur).is_some()
                    {
                        cx.status = "Drew a layout line".into();
                    }
                }
                Some(Drag::Marquee { id, start, cur }) if self.selected == Some(id) => {
                    self.line_sel = self
                        .selected_box()
                        .and_then(|b| b.view.art.as_ref())
                        .map(|a| a.lines_in_rect(start, cur))
                        .unwrap_or_default();
                    cx.status = format!("Selected {} layout line(s)", self.line_sel.len());
                }
                _ => {}
            }
            if let Some((l, _)) = self.steps.last() {
                cx.status = l.clone();
            }
        }
        // Clicks.
        if resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let (x, y) = xf.paper(pos);
                if self.placing.is_some() {
                    self.place_at(cx, x, y);
                } else {
                    let extend = ui.input(|i| i.modifiers.shift);
                    self.click_with(cx, xf, pos, snapping, extend);
                }
            }
        }
        if resp.double_clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                self.double_click(cx, xf, pos);
            }
        }
        // Placement ghost.
        if let (Some(p), Some(pos)) = (&self.placing, resp.hover_pos()) {
            let (w, h) = p.size;
            let (x, y) = xf.paper(pos);
            let r = xf.rect([x - w / 2.0, y - h / 2.0, x + w / 2.0, y + h / 2.0]);
            painter.rect_stroke(r, 0.0, st(1.5, SELECT_BLUE), StrokeKind::Middle);
        }
        // The drawing in progress.
        let draft = st(1.0, SELECT_BLUE);
        if let Some(Drag::Draw { start, cur }) = &self.drag {
            let (a, c) = (xf.pt(start.0, start.1), xf.pt(cur.0, cur.1));
            match self.tool {
                LayoutTool::Line | LayoutTool::Leader => {
                    painter.line_segment([a, c], draft);
                }
                LayoutTool::Circle => {
                    painter.circle_stroke(a, a.distance(c), draft);
                }
                _ => {
                    painter.rect_stroke(Rect::from_two_pos(a, c), 0.0, draft, StrokeKind::Middle);
                }
            }
        }
        if let Some(Drag::DrawLine { id, start, cur, .. } | Drag::Marquee { id, start, cur }) =
            &self.drag
        {
            let marquee = matches!(self.drag, Some(Drag::Marquee { .. }));
            if let Some((b, map)) = self
                .layout
                .as_ref()
                .and_then(|l| {
                    l.pages
                        .iter()
                        .flat_map(|p| p.boxes.iter())
                        .find(|b| b.id == *id)
                        .cloned()
                })
                .and_then(|b| {
                    let rcx = ui_context(&cx.project);
                    plan_layout::box_content_map(&b, &rcx).map(|m| (b, m))
                })
            {
                let _ = b;
                let (a, c) = (map.to_paper(*start), map.to_paper(*cur));
                let (pa, pc) = (xf.pt(a.x, a.y), xf.pt(c.x, c.y));
                if marquee {
                    painter.rect_stroke(Rect::from_two_pos(pa, pc), 0.0, draft, StrokeKind::Middle);
                } else {
                    painter.line_segment([pa, pc], draft);
                }
            }
        }
        if self.tool == LayoutTool::Arc && !self.poly.is_empty() {
            let pts: Vec<Pos2> = self.poly.iter().map(|p| xf.pt(p.x, p.y)).collect();
            if let Some(h) = resp.hover_pos() {
                painter.line_segment([pts[pts.len() - 1], h], draft);
            }
            painter.add(egui::Shape::line(pts, draft));
        }
        if !self.poly.is_empty() && matches!(self.tool, LayoutTool::Polyline | LayoutTool::Leader) {
            let mut pts: Vec<Pos2> = self.poly.iter().map(|p| xf.pt(p.x, p.y)).collect();
            if let Some(h) = resp.hover_pos() {
                let (x, y) = xf.paper(h);
                let (sx, sy) = self.snap_xy(x, y, snapping, tol);
                pts.push(xf.pt(sx, sy));
            }
            painter.add(egui::Shape::line(pts, draft));
        }
        // Keys.
        if !ctx.wants_keyboard_input() && !self.dialogs.any() {
            let (del, esc, nudge) = ui.input(|i| {
                let big = if i.modifiers.shift { 4.0 } else { 1.0 } * snap_unit();
                let mut n = (0.0, 0.0);
                if i.key_pressed(egui::Key::ArrowLeft) {
                    n.0 -= big;
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    n.0 += big;
                }
                if i.key_pressed(egui::Key::ArrowUp) {
                    n.1 += big;
                }
                if i.key_pressed(egui::Key::ArrowDown) {
                    n.1 -= big;
                }
                (
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                    i.key_pressed(egui::Key::Escape),
                    n,
                )
            });
            if del && self.tool == LayoutTool::EditLines && !self.line_sel.is_empty() {
                let n = self.delete_selected_lines(&mut cx.project);
                cx.status = format!("Deleted {n} layout line(s)");
            } else if del {
                if self.selected_cad.is_some() {
                    if self.delete_selected_cad(&mut cx.project) {
                        cx.status = "Deleted the page drawing".into();
                    }
                } else if self.delete_selected(&mut cx.project) {
                    cx.status = "Deleted the layout box".into();
                }
            }
            if esc {
                if !self.poly.is_empty() {
                    self.poly.clear();
                } else if self.tool != LayoutTool::Select {
                    self.tool = LayoutTool::Select;
                    self.drag = None;
                } else {
                    self.placing = None;
                    self.selected = None;
                    self.selected_cad = None;
                }
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && self.tool == LayoutTool::Polyline {
                self.finish_polyline(cx);
            }
            if nudge != (0.0, 0.0) {
                if let Some(id) = self.selected_cad {
                    self.nudge_annotation(&mut cx.project, id, nudge.0, nudge.1);
                } else if let Some(b) = self.selected_box() {
                    let r = moved(bounds(b), nudge.0, nudge.1, false);
                    let id = b.id;
                    self.set_box_bounds(&mut cx.project, id, r, "Move Layout Box");
                }
            }
        }
    }

    /// The topmost box under the screen point.
    fn hit(&self, xf: &Xf, pos: Pos2) -> Option<Id> {
        let (x, y) = xf.paper(pos);
        self.current_page().and_then(|p| box_at(p, x, y))
    }

    /// A click on the page, by tool: select (rotate knob, page CAD, boxes),
    /// add a polyline corner, or place page text.
    #[cfg(test)]
    fn click(&mut self, cx: &mut EditorContext, xf: &Xf, pos: Pos2, snapping: bool) {
        self.click_with(cx, xf, pos, snapping, false);
    }

    /// [`click`](Self::click); with `extend` (Shift) a click on a box adds it
    /// to the selection, or takes it out when it is already in.
    fn click_with(
        &mut self,
        cx: &mut EditorContext,
        xf: &Xf,
        pos: Pos2,
        snapping: bool,
        extend: bool,
    ) {
        let (x, y) = xf.paper(pos);
        match self.tool {
            LayoutTool::Select => {
                if let Some(b) = self.selected_box() {
                    if on_rotate_knob(hit_bounds(b), x, y, xf.z) {
                        if self.rotate_selected(&mut cx.project) {
                            cx.status = "Turned the layout box".into();
                        }
                        return;
                    }
                }
                let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
                let cad = self.current_page().and_then(|p| cad_at(p, x, y, tol));
                match cad {
                    // Shift adds a page drawing to the selection (the boxes
                    // stay), so a group drag moves both.
                    Some(id) if extend => self.extend_cad_selection(id),
                    Some(id) => {
                        self.selected_cad = Some(id);
                        self.selected = None;
                        self.also.clear();
                        self.also_cad.clear();
                    }
                    None => {
                        let hit = self.hit(xf, pos);
                        if !extend {
                            self.selected_cad = None;
                            self.also_cad.clear();
                        }
                        if extend {
                            if let Some(id) = hit {
                                let mut ids = self.selection_ids();
                                match ids.iter().position(|i| *i == id) {
                                    Some(at) => {
                                        ids.remove(at);
                                    }
                                    None => ids.push(id),
                                }
                                // The page drawings selected stay selected.
                                self.selected = ids.first().copied();
                                self.also = ids.iter().skip(1).copied().collect();
                            }
                        } else {
                            self.selected = hit;
                            self.also.clear();
                        }
                    }
                }
            }
            LayoutTool::Polyline | LayoutTool::Leader => {
                let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
                let (sx, sy) = self.snap_xy(x, y, snapping, tol);
                self.poly.push(Point::new(sx, sy));
            }
            LayoutTool::Text => {
                self.dialogs.cad_text = Some(CadTextDialog::new(CadTextSpec {
                    id: None,
                    pos: {
                        let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
                        let (sx, sy) = self.snap_xy(x, y, snapping, tol);
                        Point::new(sx, sy)
                    },
                    text: String::new(),
                    height_in: 0.125,
                }));
            }
            LayoutTool::Arc => {
                let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
                let (sx, sy) = self.snap_xy(x, y, snapping, tol);
                let p = Point::new(sx, sy);
                self.poly.push(p);
                if self.poly.len() == 3 {
                    let (c, a, b) = (self.poly[0], self.poly[1], self.poly[2]);
                    self.poly.clear();
                    if self.add_cad_arc(&mut cx.project, c, a, b).is_some() {
                        cx.status = "Added an arc to the page".into();
                    }
                }
            }
            LayoutTool::PointToPoint => {
                let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
                let (sx, sy) = self.snap_xy(x, y, snapping, tol);
                self.poly.push(Point::new(sx, sy));
                if self.poly.len() >= 2 {
                    let (a, b) = (self.poly[0], self.poly[1]);
                    self.poly.clear();
                    self.tool = LayoutTool::Select;
                    cx.status = if self.move_selection(
                        &mut cx.project,
                        (b.x - a.x, b.y - a.y),
                        "Point to Point Move",
                    ) {
                        "Moved the selection".into()
                    } else {
                        "Select a layout box or page drawing before Point to Point Move".into()
                    };
                } else {
                    cx.status = "Click where the selection moves to".into();
                }
            }
            LayoutTool::PanScale => {
                // Click a view to pan and scale it.
                if let Some(id) = self.hit(xf, pos) {
                    self.selected = Some(id);
                    self.also.clear();
                }
            }
            LayoutTool::EditLines => {
                let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
                if !self.select_line_at(&cx.project, x, y, tol, extend) {
                    cx.status = if self.plot_box(&cx.project).is_some() {
                        "No layout line there: drag to draw one".into()
                    } else {
                        "Select a view sent as Plot Lines to edit its lines".into()
                    };
                } else {
                    cx.status = format!("{} layout line(s) selected", self.line_sel.len());
                }
            }
            LayoutTool::Line
            | LayoutTool::Box
            | LayoutTool::TextBox
            | LayoutTool::Circle
            | LayoutTool::Cloud => {}
        }
    }

    /// A double-click: finish a polyline, edit page text or a text box, or open
    /// the Layout Box Specification.
    fn double_click(&mut self, cx: &mut EditorContext, xf: &Xf, pos: Pos2) {
        if self.tool == LayoutTool::Polyline {
            self.finish_polyline(cx);
            return;
        }
        if self.tool == LayoutTool::Leader {
            self.finish_leader();
            return;
        }
        if self.tool == LayoutTool::EditLines {
            if !self.line_sel.is_empty() && self.open_line_spec() {
                cx.status = String::new();
            }
            return;
        }
        if self.tool != LayoutTool::Select {
            return;
        }
        let (x, y) = xf.paper(pos);
        let tol = f64::from(HANDLE_PX) / f64::from(xf.z);
        let cad_text = self.current_page().and_then(|p| {
            let id = cad_at(p, x, y, tol)?;
            let o = p.cad.iter().find(|o| o.id == id)?;
            match &o.item {
                CadItem::Text {
                    pos, text, height, ..
                } => Some(CadTextSpec {
                    id: Some(id),
                    pos: *pos,
                    text: text.clone(),
                    height_in: *height,
                }),
                _ => None,
            }
        });
        if let Some(spec) = cad_text {
            self.selected_cad = Some(spec.id.unwrap_or_default());
            self.dialogs.cad_text = Some(CadTextDialog::new(spec));
            return;
        }
        if let Some(id) = self.current_page().and_then(|p| cad_at(p, x, y, tol)) {
            let page = self.current_page();
            if let Some(l) = page.and_then(|p| p.leaders.iter().find(|l| l.id == id)) {
                let spec = LeaderSpec {
                    id: Some(id),
                    tip: l.tip,
                    elbow: l.elbow,
                    bends: l.bends.clone(),
                    text: l.text.clone(),
                    height_in: l.height_in,
                    arrow: l.arrow,
                };
                self.selected_cad = Some(id);
                self.dialogs.leader = Some(LeaderDialog::new(spec));
                return;
            }
            if let Some(c) = page.and_then(|p| p.clouds.iter().find(|c| c.id == id)) {
                let spec = CloudSpec {
                    id: Some(id),
                    rect: c.rect,
                    revision: c.revision.clone(),
                };
                self.selected_cad = Some(id);
                self.dialogs.cloud = Some(CloudDialog::new(spec));
                return;
            }
        }
        if let Some(b) = self.hit(xf, pos) {
            self.selected = Some(b);
            self.selected_cad = None;
            if let Some(bx) = self.selected_box().cloned() {
                match TextBoxDialog::new(&bx) {
                    Some(d) => self.dialogs.text_box = Some(d),
                    None => self.open_spec(&cx.project, &bx),
                }
            }
        }
    }

    /// Ends the multi-segment leader being clicked out: the first click is the
    /// arrow tip, the last the elbow where the text goes, the ones between
    /// are bends. Fewer than two distinct points start nothing.
    fn finish_leader(&mut self) {
        let mut pts = std::mem::take(&mut self.poly);
        pts.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
        if pts.len() < 2 {
            return;
        }
        let tip = pts.remove(0);
        let elbow = pts.pop().unwrap_or(tip);
        self.dialogs.leader = Some(LeaderDialog::new(LeaderSpec {
            id: None,
            tip,
            elbow,
            bends: pts,
            text: String::new(),
            height_in: plan_layout::LEADER_TEXT_IN,
            arrow: true,
        }));
    }

    /// Ends the polyline being drawn (two or more distinct corners).
    fn finish_polyline(&mut self, cx: &mut EditorContext) {
        let mut pts = std::mem::take(&mut self.poly);
        pts.dedup_by(|a, b| (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9);
        if self.add_cad_polyline(&mut cx.project, pts).is_some() {
            cx.status = "Added a polyline to the page".into();
        }
    }

    /// The drag of a Line, Box or Text Box tool ended.
    fn finish_draw(&mut self, cx: &mut EditorContext, start: (f64, f64), cur: (f64, f64)) {
        let (a, b) = (Point::new(start.0, start.1), Point::new(cur.0, cur.1));
        if a.sub(b).length() < SNAP_IN * 0.9 {
            return;
        }
        match self.tool {
            LayoutTool::Line => {
                self.add_cad_line(&mut cx.project, a, b);
                cx.status = "Added a line to the page".into();
            }
            LayoutTool::Box => {
                self.add_cad_box(&mut cx.project, a, b);
                cx.status = "Added a box outline to the page".into();
            }
            LayoutTool::Circle => {
                if self.add_cad_circle(&mut cx.project, a, a.dist(b)).is_some() {
                    cx.status = "Added a circle to the page".into();
                }
            }
            LayoutTool::Leader => {
                // Dragged from the arrow tip to where the text goes; clicks
                // before the drag are the tip and the bends.
                let mut pts = std::mem::take(&mut self.poly);
                let (tip, bends) = if pts.is_empty() {
                    (a, Vec::new())
                } else {
                    let tip = pts.remove(0);
                    (tip, pts)
                };
                self.dialogs.leader = Some(LeaderDialog::new(LeaderSpec {
                    id: None,
                    tip,
                    elbow: b,
                    bends,
                    text: String::new(),
                    height_in: plan_layout::LEADER_TEXT_IN,
                    arrow: true,
                }));
            }
            LayoutTool::Cloud => {
                if (b.x - a.x).abs() >= plan_layout::MIN_CLOUD_IN
                    && (b.y - a.y).abs() >= plan_layout::MIN_CLOUD_IN
                {
                    self.dialogs.cloud = Some(CloudDialog::new(CloudSpec {
                        id: None,
                        rect: (a, b),
                        revision: self.next_revision(),
                    }));
                }
            }
            LayoutTool::TextBox => {
                let r = [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)];
                if r[2] - r[0] >= MIN_BOX_IN && r[3] - r[1] >= MIN_BOX_IN {
                    if let Some(id) = self.add_text_box_in(&mut cx.project, r) {
                        if let Some(bx) = self.selected_box().filter(|bx| bx.id == id) {
                            self.dialogs.text_box = TextBoxDialog::new(bx);
                        }
                        cx.status = "Added a text box; double-click it to edit".into();
                    }
                }
            }
            _ => {}
        }
    }

    /// Moves a box without a history step (mid-drag); the drag's start
    /// snapshot becomes the step when the mouse is released.
    fn live_bounds(&mut self, project: &mut Project, id: Id, r: [f64; 4]) {
        if let Some(l) = &mut self.layout {
            for p in &mut l.pages {
                if let Some(b) = p.boxes.iter_mut().find(|b| b.id == id) {
                    set_bounds(b, r);
                }
            }
        }
        self.write_back(project);
    }

    /// Bounds of the selected page annotation.
    fn selected_cad_bounds(&self) -> Option<[f64; 4]> {
        self.current_page()?.annotation_bounds(self.selected_cad?)
    }

    /// The current page with its boxes left out: the annotations as a drag
    /// started with them.
    fn annotations_only(&self) -> Option<LayoutPage> {
        let mut p = self.current_page()?.clone();
        p.boxes.clear();
        Some(p)
    }

    /// Re-applies a drag to the annotations of the page as they were when it
    /// began (no history step; the mouse release makes it).
    fn live_annotation(
        &mut self,
        project: &mut Project,
        orig: &LayoutPage,
        f: impl FnOnce(&mut LayoutPage),
    ) {
        let mut moved = orig.clone();
        f(&mut moved);
        let index = self.page;
        if let Some(p) = self.layout.as_mut().and_then(|l| l.pages.get_mut(index)) {
            p.cad = moved.cad;
            p.leaders = moved.leaders;
            p.clouds = moved.clouds;
        }
        self.write_back(project);
    }

    /// Moves the selected annotation by `(dx, dy)` paper inches as one step.
    pub fn nudge_annotation(&mut self, project: &mut Project, id: Id, dx: f64, dy: f64) -> bool {
        let page = self.page;
        self.edit(project, "Move Layout Drawing", |l| {
            l.pages
                .get_mut(page)
                .is_some_and(|p| p.move_annotation(id, dx, dy))
        })
    }

    /// The next revision mark: one past the highest number among the page's clouds.
    fn next_revision(&self) -> String {
        let n = self
            .layout
            .iter()
            .flat_map(|l| l.pages.iter().flat_map(|p| p.clouds.iter()))
            .filter_map(|c| c.revision.trim().parse::<u32>().ok())
            .max()
            .map_or(1, |m| m + 1);
        n.to_string()
    }

    fn finish_drag(&mut self, project: &mut Project, before: Layout, label: &str) {
        let changed = self.layout.as_ref().is_some_and(|l| *l != before);
        if changed {
            self.record(project, label, &before);
            self.write_back(project);
        }
    }
}

// ----------------------------------------------------------------- tests --

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::WallKind;
    use plan_docs::SheetSize;

    fn no_layout() -> Layout {
        Layout::new("t", plan_docs::SheetSize::ArchC)
    }

    /// The request a default perspective box makes, as it is rendered.
    fn test_request(id: Id) -> PerspectiveRequest {
        effective_request(PerspectiveRequest {
            camera_id: id,
            width: 480,
            height: 360,
            samples: 8,
        })
    }

    fn project() -> Project {
        let mut p = Project::new("Smith Residence");
        for (a, b) in [
            ((0.0, 0.0), (480.0, 0.0)),
            ((480.0, 0.0), (480.0, 360.0)),
            ((480.0, 360.0), (0.0, 360.0)),
            ((0.0, 360.0), (0.0, 0.0)),
        ] {
            p.add_wall(
                0,
                Point::new(a.0, a.1),
                Point::new(b.0, b.1),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        p
    }

    fn view_with_layout() -> (LayoutView, Project) {
        let mut p = project();
        let mut v = LayoutView::default();
        assert!(v.create(&mut p, None));
        (v, p)
    }

    fn plan_spec(floor: usize) -> SendSpec {
        SendSpec {
            source: SendSource::Plan {
                floor,
                layer_set: "Default Set".into(),
            },
            page: PageChoice::Existing(1),
            scale: None,
            placement: Placement::FirstFree,
        }
    }

    #[test]
    fn layout_round_trips_through_project_json() {
        let (mut v, mut p) = view_with_layout();
        v.send(&mut p, &plan_spec(0), None).unwrap();
        let json = p.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(load(&back).unwrap(), *v.layout().unwrap());
        assert!(load(&Project::new("x")).is_none());
        // A fresh view reads the opened project's layout.
        let mut v2 = LayoutView::default();
        v2.sync(&back);
        assert_eq!(v2.layout(), v.layout());
    }

    #[test]
    fn new_layout_has_a_template_page_and_page_one_with_the_title_block() {
        let (v, mut p) = view_with_layout();
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 2);
        assert!(l.pages[0].template_page && l.pages[0].number == 0);
        assert_eq!(l.pages[1].number, 1);
        assert_eq!(
            l.title_block,
            plan_layout::TitleBlockTemplate::from_daniel_18x24()
        );
        assert_eq!(l.sheet, SheetSize::ArchC);
        assert_eq!(v.page, 1, "the first printed page is showing");
        // The title block fields expand with the Project Information.
        p.info.client_name = "J. Smith".into();
        let ctx = macro_context(&p);
        let fields = l.title_block.expand_macros(&ctx);
        assert!(fields.contains(&("PROJECT".to_string(), "Smith Residence".to_string())));
        assert!(fields.contains(&("CLIENT".to_string(), "J. Smith".to_string())));
        // Today's date is filled in for the DATE field.
        assert_eq!(p.info.date.len(), 10);
        // A second New Layout opens the same one.
        let mut v2 = LayoutView::default();
        assert!(!v2.create(&mut p.clone(), None));
    }

    #[test]
    fn send_to_layout_adds_a_box_at_the_auto_scale() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        let l = v.layout().unwrap();
        let b = l.pages[1].boxes.iter().find(|b| b.id == id).unwrap();
        // A 40' x 30' plan on 18x24: 1/4" is the auto ceiling and it fits.
        assert_eq!(b.scale, Scale::QuarterInch);
        assert_eq!(b.label.as_deref(), Some("1ST FLOOR PLAN"));
        assert!(matches!(b.source, BoxSource::PlanView { floor: 0, .. }));
        assert_eq!(v.selected, Some(id));
        assert_eq!(v.undo_label(), Some("Send to Layout"));
        // A chosen scale is used as is.
        let spec = SendSpec {
            scale: Some(Scale::EighthInch),
            ..plan_spec(0)
        };
        let id2 = v.send(&mut p, &spec, None).unwrap();
        let l = v.layout().unwrap();
        assert_eq!(
            l.pages[1].boxes.iter().find(|b| b.id == id2).unwrap().scale,
            Scale::EighthInch
        );
        // Undo takes it away again.
        assert_eq!(v.undo(&mut p).as_deref(), Some("Send to Layout"));
        assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 1);
    }

    #[test]
    fn send_centered_and_clicked_and_new_page() {
        let (mut v, mut p) = view_with_layout();
        let centered = SendSpec {
            placement: Placement::Centered,
            ..plan_spec(0)
        };
        let id = v.send(&mut p, &centered, None).unwrap();
        let (lo, hi) = v.layout().unwrap().drawing_area();
        let b = v.layout().unwrap().pages[1]
            .boxes
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .clone();
        let r = bounds(&b);
        assert!(((r[0] + r[2]) / 2.0 - (lo.x + hi.x) / 2.0).abs() < 1e-9);
        assert!(((r[1] + r[3]) / 2.0 - (lo.y + hi.y) / 2.0).abs() < 1e-9);
        let clicked = SendSpec {
            placement: Placement::Click,
            page: PageChoice::New,
            ..plan_spec(0)
        };
        let id2 = v.send(&mut p, &clicked, Some((10.0, 8.0))).unwrap();
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 3, "a new page was added");
        assert_eq!(v.page, 2);
        let b2 = l.pages[2].boxes.iter().find(|b| b.id == id2).unwrap();
        let r2 = bounds(b2);
        assert!(((r2[0] + r2[2]) / 2.0 - 10.0).abs() < 1e-9);
        assert!(((r2[1] + r2[3]) / 2.0 - 8.0).abs() < 1e-9);
        assert!(v.send(&mut p, &plan_spec(9), None).is_err());
    }

    #[test]
    fn box_move_and_resize_update_the_model_and_undo_restores() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        let before = bounds(&v.layout().unwrap().pages[1].boxes[0]);
        let moved_to = moved(before, 1.0, 2.0, true);
        assert!(v.set_box_bounds(&mut p, id, moved_to, "Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), moved_to);
        // The project holds the move too.
        assert_eq!(bounds(&load(&p).unwrap().pages[1].boxes[0]), moved_to);
        let grown = resized(moved_to, Handle::NE, 1.0, 1.0, true);
        assert!(v.set_box_bounds(&mut p, id, grown, "Resize Layout Box"));
        assert!(grown[2] > moved_to[2] && grown[3] > moved_to[3]);
        assert_eq!(v.undo(&mut p).as_deref(), Some("Resize Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), moved_to);
        assert_eq!(v.undo(&mut p).as_deref(), Some("Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), before);
        assert_eq!(bounds(&load(&p).unwrap().pages[1].boxes[0]), before);
        assert_eq!(v.redo(&mut p).as_deref(), Some("Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), moved_to);
    }

    #[test]
    fn resize_handles_keep_a_minimum_size_and_hit_tests_work() {
        let r = [2.0, 2.0, 6.0, 5.0];
        assert_eq!(resized(r, Handle::E, 1.0, 9.0, false), [2.0, 2.0, 7.0, 5.0]);
        assert_eq!(resized(r, Handle::W, 10.0, 0.0, false)[0], 6.0 - MIN_BOX_IN);
        assert_eq!(
            resized(r, Handle::N, 0.0, -10.0, false)[3],
            2.0 + MIN_BOX_IN
        );
        assert_eq!(moved(r, 0.3, 0.3, true)[0], 2.3125);
        assert_eq!(handle_at(r, 6.0, 5.0, 0.1), Some(Handle::NE));
        assert_eq!(handle_at(r, 4.0, 2.0, 0.1), Some(Handle::S));
        assert_eq!(handle_at(r, 4.0, 3.5, 0.1), None);
        let (v, _) = view_with_layout();
        let page = &v.layout().unwrap().pages[1];
        assert_eq!(box_at(page, 1.0, 1.0), None);
    }

    #[test]
    fn delete_box_and_page_operations() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        assert_eq!(v.selected, Some(id));
        assert!(v.delete_selected(&mut p));
        assert!(v.layout().unwrap().pages[1].boxes.is_empty());
        assert!(!v.delete_selected(&mut p));
        v.undo(&mut p);
        assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 1);

        // Pages: add, rename, duplicate, exchange, delete.
        assert!(v.add_page(&mut p, false));
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 3);
        assert_eq!(v.page, 2);
        assert_eq!(l.pages[2].number, 2);
        assert!(v.rename_page(&mut p, 2, " Elevations "));
        assert_eq!(v.layout().unwrap().pages[2].title, "Elevations");
        assert!(v.add_page(&mut p, true));
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 4);
        // Inserting before renumbers: the old page 2 became page 3.
        assert_eq!(
            l.pages.iter().map(|x| x.number).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        assert_eq!(l.pages[3].title, "Elevations");
        v.set_page(1);
        assert!(v.duplicate_current_page(&mut p));
        let l = v.layout().unwrap();
        assert_eq!(l.pages.len(), 5);
        assert_eq!(l.pages[2].boxes.len(), 1);
        assert_ne!(
            l.pages[2].boxes[0].id, l.pages[1].boxes[0].id,
            "new box ids"
        );
        assert!(v.exchange_current_page(&mut p, true));
        assert_eq!(v.page, 3);
        assert!(v.exchange_current_page(&mut p, false));
        assert_eq!(v.page, 2);
        assert!(v.delete_current_page(&mut p).is_ok());
        assert_eq!(v.layout().unwrap().pages.len(), 4);
        // The template page stays while pages use it (round 16: LP4), the
        // last printed page cannot go.
        v.set_page(0);
        assert!(v.delete_current_page(&mut p).is_err());
        assert_eq!(v.layout().unwrap().pages.len(), 4);
        while v.layout().unwrap().content_pages().len() > 1 {
            v.set_page(1);
            assert!(v.delete_current_page(&mut p).is_ok());
        }
        v.set_page(1);
        assert_eq!(
            v.delete_current_page(&mut p),
            Err("A layout keeps at least one page")
        );
        // Everything was undoable step by step.
        assert!(v.undo(&mut p).is_some());
    }

    #[test]
    fn page_setup_changes_the_sheet() {
        let (mut v, mut p) = view_with_layout();
        let mut s = v.page_setup().unwrap();
        assert_eq!(
            s.sheet,
            plan_layout::SheetChoice::Standard(SheetSize::ArchC)
        );
        s.sheet = plan_layout::SheetChoice::Standard(SheetSize::Tabloid);
        s.margins_in = 0.25;
        s.page_background = false;
        s.edge_line_weight = 35;
        assert!(v.apply_page_setup(&mut p, &s));
        let l = load(&p).unwrap();
        assert_eq!(l.sheet, SheetSize::Tabloid);
        assert_eq!(l.margins_in, 0.25);
        assert!(!l.page_background);
        assert_eq!(l.edge_line_weight, 35);
        assert!(!v.apply_page_setup(&mut p, &s), "no change, no step");
        v.undo(&mut p);
        assert_eq!(v.layout().unwrap().sheet, SheetSize::ArchC);
    }

    #[test]
    fn title_block_macros_read_the_project_information() {
        let (mut v, mut p) = view_with_layout();
        // A new layout fills in today's date when Project Information has none.
        assert_eq!(p.info.date.len(), 10);
        p.info.client_name = "J. Smith".into();
        p.info.client_address = vec!["1 Main St".into(), "Atlanta".into()];
        p.info.project_number = "26-014".into();
        p.info.revision = "B".into();
        p.info.date = "2026-10-08".into();
        p.info.drawn_by = "DAD".into();
        p.info.revisions = vec![("1".into(), "2026-10-08".into(), "Issued".into())];
        let ctx = macro_context(&p);
        assert_eq!(
            ctx.expand(
                "%project.name%|%client%|%address%|%project.number%|%revision%|%designer%|%date%"
            ),
            "Smith Residence|J. Smith|1 Main St, Atlanta|26-014|B|DAD|2026-10-08"
        );
        assert_eq!(ctx.revisions.len(), 1);
        // The layout and the information live side by side in the project.
        v.add_page(&mut p, false);
        assert_eq!(p.info.project_number, "26-014");
        assert!(load(&p).is_some());
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(macro_context(&back), ctx);
    }

    #[test]
    fn box_specification_edits_and_moves_a_box() {
        let (mut v, mut p) = view_with_layout();
        let id = v.send(&mut p, &plan_spec(0), None).unwrap();
        v.add_page(&mut p, false);
        let mut b = v.layout().unwrap().pages[1].boxes[0].clone();
        b.scale = Scale::EighthInch;
        b.border = false;
        b.label = Some("MAIN LEVEL".into());
        let spec = BoxSpec {
            layout_box: b.clone(),
            page: 2,
        };
        assert!(v.apply_spec(&mut p, &spec));
        let l = v.layout().unwrap();
        assert!(l.pages[1].boxes.is_empty());
        let moved = l.pages[2].boxes.iter().find(|x| x.id == id).unwrap();
        assert_eq!(moved.scale, Scale::EighthInch);
        assert!(!moved.border);
        assert_eq!(moved.label.as_deref(), Some("MAIN LEVEL"));
        // A box that no longer exists is refused.
        let gone = BoxSpec {
            layout_box: LayoutBox { id: 999, ..b },
            page: 1,
        };
        assert!(!v.apply_spec(&mut p, &gone));
    }

    #[test]
    fn page_information_renames_and_flags_templates() {
        let (mut v, mut p) = view_with_layout();
        let mut d = v.page_info_dialog(&p).unwrap();
        assert_eq!(d.entries().len(), 2);
        d.select(1);
        d.info_mut().unwrap().title = "Main Level".into();
        assert_eq!(v.apply_page_info(&mut p, &d), Ok(true));
        assert_eq!(v.layout().unwrap().pages[1].title, "Main Level");
        assert_eq!(v.apply_page_info(&mut p, &d), Ok(false));
        assert_eq!(page_list(&p)[1].1, "A-1  Main Level");
        assert!(page_list(&p)[0].2, "page 0 is the template");
    }

    #[test]
    fn print_renders_the_printed_pages_only_and_ranges() {
        let (mut v, mut p) = view_with_layout();
        v.send(&mut p, &plan_spec(0), None).unwrap();
        v.add_page(&mut p, false);
        v.add_page(&mut p, false);
        let layout = v.layout().unwrap().clone();
        assert_eq!(layout.content_pages().len(), 3);
        let count = |bytes: &[u8]| {
            String::from_utf8_lossy(bytes)
                .matches("/Type /Page")
                .count()
        };
        let all = print_bytes(&layout, &p, None);
        assert!(all.starts_with(b"%PDF"));
        assert_eq!(count(&all), 3, "the template page is not printed");
        let range = print_bytes(&layout, &p, Some((2, 3)));
        assert_eq!(count(&range), 2);
        let one = print_bytes(&layout, &p, Some((1, 1)));
        assert_eq!(count(&one), 1);
        assert_eq!(layout_pdf(&p).map(|b| count(&b)), Some(3));
    }

    #[test]
    fn plan_and_layout_edits_share_one_undo_stack_in_order() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut v = LayoutView::default();
        let wall = |cx: &mut EditorContext, label: &str, y: f64| {
            cx.begin_change(label);
            cx.project.add_wall(
                0,
                Point::new(0.0, y),
                Point::new(240.0, y),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        };
        wall(&mut cx, "Add Wall A", 0.0);
        v.create(&mut cx.project, None);
        v.flush(&mut cx);
        wall(&mut cx, "Add Wall B", 100.0);
        let id = v.send(&mut cx.project, &plan_spec(0), None).unwrap();
        v.flush(&mut cx);
        let boxes = |cx: &EditorContext| load(&cx.project).map(|l| l.pages[1].boxes.len());
        assert_eq!(boxes(&cx), Some(1));
        assert!(v.layout().unwrap().pages[1]
            .boxes
            .iter()
            .any(|b| b.id == id));
        assert_eq!(cx.undo_label(), Some("Send to Layout"));

        // Undo walks back through both kinds of edit, newest first, and a
        // plan undo never rolls back a layout edit made after it.
        assert_eq!(cx.undo().as_deref(), Some("Send to Layout"));
        assert_eq!(boxes(&cx), Some(0));
        assert_eq!(cx.project.floors[0].walls.len(), 2, "wall B survives");
        assert_eq!(cx.undo().as_deref(), Some("Add Wall B"));
        assert_eq!(cx.project.floors[0].walls.len(), 1);
        assert_eq!(boxes(&cx), Some(0), "the layout stays");
        assert_eq!(cx.undo().as_deref(), Some("New Layout"));
        assert!(cx.project.layout.is_none());
        assert_eq!(cx.undo().as_deref(), Some("Add Wall A"));
        assert!(cx.project.floors[0].walls.is_empty());

        // Redo replays them in order; the view follows the project.
        for label in ["Add Wall A", "New Layout", "Add Wall B", "Send to Layout"] {
            assert_eq!(cx.redo().as_deref(), Some(label));
        }
        v.sync(&cx.project);
        assert_eq!(boxes(&cx), Some(1));
        assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 1);
        assert_eq!(cx.project.floors[0].walls.len(), 2);
    }

    #[test]
    fn the_entry_points_record_layout_edits_in_the_editor_history() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        with_view(|v| *v = LayoutView::default());
        new_layout_for_test(&mut cx);
        assert_eq!(cx.undo_label(), Some("New Layout"));
        dispatch(LayoutCommand::InsertPageAfter, &mut cx, None);
        assert_eq!(cx.undo_label(), Some("Insert Page"));
        assert_eq!(undo(&mut cx).as_deref(), Some("Insert Page"));
        assert_eq!(load(&cx.project).unwrap().pages.len(), 2);
        assert_eq!(redo(&mut cx).as_deref(), Some("Insert Page"));
        assert_eq!(load(&cx.project).unwrap().pages.len(), 3);
        with_view(|v| *v = LayoutView::default());
    }

    fn new_layout_for_test(cx: &mut EditorContext) {
        with_view(|v| {
            v.create(&mut cx.project, None);
            v.active = true;
            v.flush(cx);
        });
    }

    #[test]
    fn external_changes_reload_the_layout_and_drop_history() {
        let (mut v, mut p) = view_with_layout();
        v.send(&mut p, &plan_spec(0), None).unwrap();
        assert!(v.undo_label().is_some());
        let mut other = Project::new("Other");
        let mut v2 = LayoutView::default();
        v2.create(&mut other, None);
        // The project's layout is replaced (a file was opened).
        p.layout = other.layout.clone();
        v.sync(&p);
        assert_eq!(v.layout().unwrap().pages[1].boxes.len(), 0);
        assert!(v.undo_label().is_none());
        // And removed.
        p.layout = None;
        v.sync(&p);
        assert!(v.layout().is_none());
    }

    #[test]
    fn send_all_floors_makes_one_page_per_floor() {
        let mut p = project();
        p.build_new_floor(false);
        let mut v = LayoutView::default();
        v.create(&mut p, None);
        assert_eq!(v.send_all_floors(&mut p), 2);
        let l = v.layout().unwrap();
        assert_eq!(l.content_pages().len(), 2);
        assert_eq!(l.pages[1].title, "FIRST FLOOR PLAN");
        assert_eq!(l.pages[2].title, "SECOND FLOOR PLAN");
    }

    #[test]
    fn dispatch_commands_drive_the_view() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.run(&mut cx, LayoutCommand::ShowLayout, None);
        assert!(v.active);
        v.run(&mut cx, LayoutCommand::InsertPageAfter, None);
        assert_eq!(v.layout().unwrap().pages.len(), 3);
        v.run(&mut cx, LayoutCommand::Undo, None);
        assert_eq!(cx.status, "Undid Insert Page");
        v.run(&mut cx, LayoutCommand::SendToLayout, None);
        assert!(v.dialogs.send.is_some());
        v.run(&mut cx, LayoutCommand::PageSetup, None);
        v.run(&mut cx, LayoutCommand::ProjectInfo, None);
        v.run(&mut cx, LayoutCommand::PageTable, None);
        v.run(&mut cx, LayoutCommand::Print, None);
        assert!(v.dialogs.any());
        v.run(&mut cx, LayoutCommand::BoxSpecification, None);
        assert_eq!(cx.status, "Select a layout box first");
        v.run(&mut cx, LayoutCommand::ShowPlan, None);
        assert!(!v.active);
    }

    #[test]
    fn the_new_commands_open_their_dialogs_and_work_the_selection() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.run(&mut cx, LayoutCommand::ShowLayout, None);
        v.run(&mut cx, LayoutCommand::PageInformation, None);
        assert!(v.dialogs.page_info.is_some());
        v.dialogs.page_info = None;
        v.run(&mut cx, LayoutCommand::CustomizeSheetSizes, None);
        assert!(v.dialogs.sheet_sizes.is_some());
        v.dialogs.sheet_sizes = None;
        v.run(&mut cx, LayoutCommand::NewLayoutFile, None);
        assert!(v.dialogs.new_layout.is_some());
        v.dialogs.new_layout = None;
        // Nothing selected: the arranging commands say so.
        for cmd in [
            LayoutCommand::Align(AlignEdge::Left),
            LayoutCommand::CopyBoxToPage,
            LayoutCommand::DuplicateBox,
        ] {
            v.run(&mut cx, cmd, None);
            assert!(
                cx.status.starts_with("Select a layout box"),
                "{}",
                cx.status
            );
        }
        v.run(&mut cx, LayoutCommand::Distribute(Spread::Horizontal), None);
        assert!(cx.status.contains("three or more"), "{}", cx.status);
        // Three boxes: align, spread, copy through the commands.
        let a = v
            .add_text_box_in(&mut cx.project, [1.0, 1.0, 3.0, 2.0])
            .unwrap();
        let b = v
            .add_text_box_in(&mut cx.project, [4.0, 5.0, 8.0, 7.0])
            .unwrap();
        let c = v
            .add_text_box_in(&mut cx.project, [2.0, 9.0, 3.0, 10.0])
            .unwrap();
        v.select_boxes(&[a, b, c]);
        v.run(&mut cx, LayoutCommand::Align(AlignEdge::Left), None);
        assert!(cx.status.starts_with("Moved 2 layout box"), "{}", cx.status);
        v.run(&mut cx, LayoutCommand::Align(AlignEdge::Left), None);
        assert_eq!(cx.status, "The boxes are already lined up");
        v.run(&mut cx, LayoutCommand::Distribute(Spread::Vertical), None);
        assert!(cx.status.starts_with("Spread"), "{}", cx.status);
        v.run(&mut cx, LayoutCommand::CopyBoxToPage, None);
        assert!(v.dialogs.copy_box.is_some());
        v.dialogs.copy_box = None;
        v.run(&mut cx, LayoutCommand::DuplicateBox, None);
        assert_eq!(cx.status, "Copied 3 layout box(es)");
        assert_eq!(v.current_page().unwrap().boxes.len(), 6);
        // Export with no table box says what to select.
        v.select_boxes(&[a]);
        v.run(&mut cx, LayoutCommand::ExportTableExcel, None);
        assert!(cx.status.starts_with("Select a schedule"), "{}", cx.status);
        // A table box exports (the file dialog is skipped under test).
        let t = v
            .add_source_box(
                &mut cx.project,
                "Add Schedule",
                BoxSource::Schedule {
                    kind: plan_layout::ScheduleKind::Door,
                },
            )
            .unwrap();
        v.select_boxes(&[t]);
        v.run(&mut cx, LayoutCommand::ExportTableExcel, None);
        assert!(
            cx.status.starts_with("Saved ") && cx.status.ends_with("of xlsx"),
            "{}",
            cx.status
        );
        v.run(&mut cx, LayoutCommand::ExportTableCsv, None);
        assert!(cx.status.ends_with("of csv"), "{}", cx.status);
        // Open Source View: a text box has no view; a plan box opens its floor.
        v.select_boxes(&[a]);
        v.run(&mut cx, LayoutCommand::OpenSourceView, None);
        assert!(
            cx.status.contains("does not come from a view"),
            "{}",
            cx.status
        );
        let plan = v.send(&mut cx.project, &plan_spec(0), None).unwrap();
        v.select_boxes(&[plan]);
        v.active = true;
        v.run(&mut cx, LayoutCommand::OpenSourceView, None);
        assert!(!v.active, "back on the plan");
        assert!(cx.status.starts_with("Opened "), "{}", cx.status);
        v.active = true;
        // Switching to a layout file that is not there does nothing.
        v.run(&mut cx, LayoutCommand::SwitchLayout(3), None);
        assert!(v.layout().is_some());
    }

    #[test]
    fn shift_click_adds_boxes_to_the_selection_and_takes_them_out() {
        let (ctx, mut v, mut cx, _) = interactive();
        let _ = ctx;
        let page = v.current_page().unwrap().clone();
        let first = page.boxes[0].id;
        let second = v
            .add_text_box_in(&mut cx.project, [1.0, 1.0, 3.0, 2.0])
            .unwrap();
        v.select_boxes(&[]);
        let xf = v.last_xf.expect("the view drew a frame");
        let at = |id: Id| {
            let b = v
                .current_page()
                .unwrap()
                .boxes
                .iter()
                .find(|b| b.id == id)
                .unwrap()
                .bounds_in();
            xf.pt((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0)
        };
        let (p1, p2) = (at(first), at(second));
        v.click_with(&mut cx, &xf, p1, false, true);
        assert_eq!(v.selection_ids(), vec![first]);
        v.click_with(&mut cx, &xf, p2, false, true);
        assert_eq!(v.selection_ids(), vec![first, second]);
        // Shift on a selected box takes it out; a plain click selects one.
        v.click_with(&mut cx, &xf, p1, false, true);
        assert_eq!(v.selection_ids(), vec![second]);
        v.click_with(&mut cx, &xf, p1, false, false);
        assert_eq!(v.selection_ids(), vec![first]);
        // The selection does not outlive its boxes.
        v.select_boxes(&[first, second]);
        v.delete_selected(&mut cx.project);
        assert!(v.selection_ids().len() <= 1);
    }

    #[test]
    fn a_page_with_its_own_sheet_paints_and_fits_at_that_size() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.run(&mut cx, LayoutCommand::ShowLayout, None);
        v.run(&mut cx, LayoutCommand::InsertPageAfter, None);
        let second = v.page;
        let mut d = v.page_info_dialog(&cx.project).unwrap();
        d.sheet_mut().unwrap().sheet = Some(plan_layout::SheetChoice::Standard(SheetSize::ArchE));
        assert_eq!(v.apply_page_info(&mut cx.project, &d), Ok(true));
        assert_eq!(v.sheet_of(second), (48.0, 36.0));
        assert_eq!(v.sheet_of(1), (24.0, 18.0));
        // Going to the other page refits the window to its sheet.
        v.set_page(1);
        assert!(v.fit_pending);
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, &mut cx));
            });
        }
        let zoom_small = v.zoom;
        v.set_page(second);
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, &mut cx));
            });
        }
        assert!(v.zoom < zoom_small, "the bigger sheet needs a smaller zoom");
        assert_eq!(v.last_xf.unwrap().h, 36.0);
    }

    #[test]
    fn print_preview_opens_from_the_print_dialog_and_draws() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.send(&mut cx.project, &plan_spec(0), None).unwrap();
        let mut d = v.layout_print_dialog().unwrap();
        d.set_color(plan_layout::PrintColor::Grayscale);
        v.preview_print(&mut cx, &d);
        let prev = v.dialogs.preview.as_ref().expect("the preview window");
        assert_eq!(prev.pages().len(), 1);
        assert_eq!(prev.pages()[0].color, plan_layout::PrintColor::Grayscale);
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                v.show_dialogs(ctx, &mut cx);
            });
        }
        assert!(v.dialogs.preview.is_some(), "it stays until closed");
        // A plan view prints a preview too, with its scale.
        let plan = PrintDialog::for_plan(0, "Default Set", "FIRST FLOOR");
        v.preview_print(&mut cx, &plan);
        assert_eq!(v.dialogs.preview.as_ref().unwrap().pages().len(), 1);
    }

    #[test]
    fn the_view_and_its_dialogs_draw_headless() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.send(&mut cx.project, &plan_spec(0), None).unwrap();
        v.active = true;
        for cmd in [
            LayoutCommand::SendToLayout,
            LayoutCommand::PageSetup,
            LayoutCommand::ProjectInfo,
            LayoutCommand::PageTable,
            LayoutCommand::Print,
            LayoutCommand::BoxSpecification,
        ] {
            v.run(&mut cx, cmd, None);
        }
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, &mut cx));
                v.show_dialogs(ctx, &mut cx);
            });
        }
        assert!(v.zoom > MIN_ZOOM);
        // The page tabs and a template page draw too.
        v.set_page(0);
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, &mut cx));
            });
        }
        // No layout: the prompt draws.
        let mut empty = LayoutView::default();
        let mut cx2 = EditorContext::new(plan_defaults::embedded());
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| empty.show(ctx, ui, &mut cx2));
        });
    }

    #[test]
    fn a_camera_view_is_sent_and_drawn_through_the_hook() {
        use plan_core::{CameraKind, CameraObject};
        let (mut v, mut p) = view_with_layout();
        let id = p.add_camera(CameraObject::new(
            CameraKind::Elevation,
            Point::new(240.0, -200.0),
            90.0,
            "Front Elevation",
            0,
        ));
        let spec = SendSpec {
            source: SendSource::Camera {
                id,
                name: "Front Elevation".into(),
            },
            ..plan_spec(0)
        };
        let box_id = v.send(&mut p, &spec, None).unwrap();
        let b = v.layout().unwrap().pages[1]
            .boxes
            .iter()
            .find(|b| b.id == box_id)
            .unwrap()
            .clone();
        assert!(matches!(b.source, BoxSource::Camera { camera_id } if camera_id == id));
        // The caption carries the view number of the camera's plan callout.
        assert_eq!(b.label.as_deref(), Some("1 - FRONT ELEVATION"));
        // The drawing cache asks the camera hook; a plain context would draw
        // only the 4 border edges.
        let mut cache = BoxCache::default();
        let mut rcx = None;
        let lines = box_art(&mut cache, &mut rcx, &p, 1, &b, &no_layout());
        assert!(lines.lines.len() > 4, "{} lines", lines.lines.len());
        let again = box_art(&mut cache, &mut rcx, &p, 1, &b, &no_layout());
        assert!(Rc::ptr_eq(&lines, &again), "cached");
        let changed = box_art(&mut cache, &mut rcx, &p, 2, &b, &no_layout());
        assert!(
            !Rc::ptr_eq(&lines, &changed),
            "a new project signature redraws"
        );
        drop(rcx);
        // A camera that is gone is refused.
        let gone = SendSpec {
            source: SendSource::Camera {
                id: 999,
                name: String::new(),
            },
            ..plan_spec(0)
        };
        assert!(v.send(&mut p, &gone, None).is_err());
        // The send source picks the open 3D view's elevation camera, else the plan.
        assert!(matches!(
            v.send_source(&p, Some(id), 0),
            SendSource::Camera { .. }
        ));
        assert!(matches!(
            v.send_source(&p, None, 0),
            SendSource::Plan { .. }
        ));
    }

    // ----- pointer interaction through egui -----

    fn frame(
        ctx: &egui::Context,
        v: &mut LayoutView,
        cx: &mut EditorContext,
        events: Vec<egui::Event>,
        t: f64,
    ) {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 900.0))),
            time: Some(t),
            events,
            ..egui::RawInput::default()
        };
        let _ = ctx.run(raw, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, cx));
        });
    }

    fn press(pos: Pos2, down: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: down,
            modifiers: egui::Modifiers::NONE,
        }
    }

    /// Drags from `from` to `to` over several frames; returns the next time.
    fn drag(
        ctx: &egui::Context,
        v: &mut LayoutView,
        cx: &mut EditorContext,
        from: Pos2,
        to: Pos2,
        mut t: f64,
    ) -> f64 {
        frame(ctx, v, cx, vec![egui::Event::PointerMoved(from)], t);
        t += 0.05;
        frame(ctx, v, cx, vec![press(from, true)], t);
        for i in 1..=6 {
            t += 0.05;
            let p = from + (to - from) * (i as f32 / 6.0);
            frame(ctx, v, cx, vec![egui::Event::PointerMoved(p)], t);
        }
        t += 0.05;
        frame(ctx, v, cx, vec![press(to, false)], t);
        t + 0.05
    }

    fn interactive() -> (egui::Context, LayoutView, EditorContext, Id) {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.steps.clear();
        let id = v.send(&mut cx.project, &plan_spec(0), None).unwrap();
        v.selected = None;
        v.active = true;
        frame(&ctx, &mut v, &mut cx, vec![], 0.0);
        frame(&ctx, &mut v, &mut cx, vec![], 0.1);
        assert!(v.last_xf.is_some());
        (ctx, v, cx, id)
    }

    fn center_of(v: &LayoutView, id: Id) -> (Pos2, [f64; 4]) {
        let b = v
            .current_page()
            .unwrap()
            .boxes
            .iter()
            .find(|b| b.id == id)
            .unwrap();
        let r = bounds(b);
        let xf = v.last_xf.unwrap();
        (xf.pt((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0), r)
    }

    #[test]
    fn dragging_a_box_moves_it_and_undo_restores() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (c, before) = center_of(&v, id);
        let t = drag(&ctx, &mut v, &mut cx, c, c + Vec2::new(60.0, -40.0), 1.0);
        let (_, after) = center_of(&v, id);
        assert_eq!(v.selected, Some(id), "dragging selects the box");
        assert!(
            after[0] > before[0] && after[1] > before[1],
            "{before:?} -> {after:?}"
        );
        assert!(
            ((after[2] - after[0]) - (before[2] - before[0])).abs() < 1e-9,
            "size kept"
        );
        // The project holds the move and one undo step covers the whole drag.
        assert_eq!(bounds(&load(&cx.project).unwrap().pages[1].boxes[0]), after);
        assert_eq!(v.steps.len(), 2, "send + move");
        assert_eq!(v.undo(&mut cx.project).as_deref(), Some("Move Layout Box"));
        assert_eq!(bounds(&v.layout().unwrap().pages[1].boxes[0]), before);
        let _ = t;
    }

    #[test]
    fn dragging_one_of_several_selected_boxes_moves_them_all_in_one_undo_step() {
        let (ctx, mut v, mut cx, id) = interactive();
        let other = v
            .add_text_box_in(&mut cx.project, [1.0, 1.0, 3.0, 2.0])
            .unwrap();
        v.steps.clear();
        v.select_boxes(&[id, other]);
        frame(&ctx, &mut v, &mut cx, vec![], 0.5);
        let (c, before) = center_of(&v, id);
        let (_, other_before) = center_of(&v, other);
        let to = c + Vec2::new(60.0, 30.0);
        drag(&ctx, &mut v, &mut cx, c, to, 1.0);
        let (_, after) = center_of(&v, id);
        let (_, other_after) = center_of(&v, other);
        let (dx, dy) = (after[0] - before[0], after[1] - before[1]);
        assert!(dx > 0.0 && dy < 0.0, "{dx} {dy}");
        assert!(((other_after[0] - other_before[0]) - dx).abs() < 1e-9);
        assert!(((other_after[1] - other_before[1]) - dy).abs() < 1e-9);
        assert_eq!(v.selection_ids().len(), 2, "both stay selected");
        assert_eq!(v.steps.len(), 1);
        assert_eq!(v.undo(&mut cx.project).as_deref(), Some("Move Layout Box"));
        assert_eq!(center_of(&v, id).1, before);
        assert_eq!(center_of(&v, other).1, other_before);
        // Dragging a box outside the selection selects that one alone.
        v.select_boxes(&[other]);
        frame(&ctx, &mut v, &mut cx, vec![], 3.0);
        let (c, _) = center_of(&v, id);
        drag(&ctx, &mut v, &mut cx, c, c + Vec2::new(20.0, 0.0), 4.0);
        assert_eq!(v.selection_ids(), vec![id]);
        assert_eq!(center_of(&v, other).1, other_before, "the other stayed");
    }

    #[test]
    fn dragging_a_handle_resizes_and_clicking_empty_space_deselects() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (c, _) = center_of(&v, id);
        // Click selects.
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(c)],
            1.0,
        );
        frame(&ctx, &mut v, &mut cx, vec![press(c, true)], 1.05);
        frame(&ctx, &mut v, &mut cx, vec![press(c, false)], 1.1);
        assert_eq!(v.selected, Some(id));
        assert!(v.steps.len() == 1, "a click is not an edit");
        // Drag the NE handle.
        let (_, before) = center_of(&v, id);
        let xf = v.last_xf.unwrap();
        let ne = xf.pt(before[2], before[3]);
        let t = drag(&ctx, &mut v, &mut cx, ne, ne + Vec2::new(-50.0, 40.0), 2.0);
        let (_, after) = center_of(&v, id);
        assert!(after[2] < before[2], "narrower: {before:?} -> {after:?}");
        assert!(after[3] < before[3], "shorter, dragged down on screen");
        assert_eq!(
            (after[0], after[1]),
            (before[0], before[1]),
            "SW corner fixed"
        );
        assert_eq!(
            v.undo(&mut cx.project).as_deref(),
            Some("Resize Layout Box")
        );
        assert_eq!(center_of(&v, id).1, before);
        // Click on bare sheet deselects.
        let empty = xf.pt(1.0, 1.0);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(empty)],
            t,
        );
        frame(&ctx, &mut v, &mut cx, vec![press(empty, true)], t + 0.05);
        frame(&ctx, &mut v, &mut cx, vec![press(empty, false)], t + 0.1);
        assert_eq!(v.selected, None);
    }

    #[test]
    fn double_click_opens_the_box_specification_and_delete_removes_the_box() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (c, _) = center_of(&v, id);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(c)],
            1.0,
        );
        for k in 0..2 {
            let t = 1.05 + 0.1 * f64::from(k);
            frame(&ctx, &mut v, &mut cx, vec![press(c, true)], t);
            frame(&ctx, &mut v, &mut cx, vec![press(c, false)], t + 0.05);
        }
        assert!(
            v.dialogs.box_view.is_some(),
            "double-click opens the specification of a view box"
        );
        v.dialogs.box_view = None;
        let key = egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut v, &mut cx, vec![key], 2.0);
        assert!(v.current_page().unwrap().boxes.is_empty());
        assert_eq!(v.undo_label(), Some("Delete Layout Box"));
    }

    #[test]
    fn a_click_places_a_pending_send() {
        let (ctx, mut v, mut cx, _) = interactive();
        v.steps.clear();
        let spec = SendSpec {
            placement: Placement::Click,
            scale: Some(Scale::EighthInch),
            ..plan_spec(0)
        };
        v.finish_send(&mut cx, spec, LayoutTarget::Current, None);
        assert!(v.placing.is_some());
        let xf = v.last_xf.unwrap();
        let at = xf.pt(8.0, 9.0);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(at)],
            1.0,
        );
        frame(&ctx, &mut v, &mut cx, vec![press(at, true)], 1.05);
        frame(&ctx, &mut v, &mut cx, vec![press(at, false)], 1.1);
        assert!(v.placing.is_none());
        let page = v.current_page().unwrap();
        assert_eq!(page.boxes.len(), 2);
        let r = bounds(page.boxes.last().unwrap());
        assert!((((r[0] + r[2]) / 2.0) - 8.0).abs() < 0.05, "{r:?}");
        assert!((((r[1] + r[3]) / 2.0) - 9.0).abs() < 0.05, "{r:?}");
    }
    // ----- boxes beyond plan views, layout CAD, rotation, printing -----

    fn paper_point_to_screen(v: &LayoutView, x: f64, y: f64) -> Pos2 {
        v.last_xf.unwrap().pt(x, y)
    }

    fn click_at(
        ctx: &egui::Context,
        v: &mut LayoutView,
        cx: &mut EditorContext,
        at: Pos2,
        t: f64,
    ) -> f64 {
        frame(ctx, v, cx, vec![egui::Event::PointerMoved(at)], t);
        frame(ctx, v, cx, vec![press(at, true)], t + 0.05);
        frame(ctx, v, cx, vec![press(at, false)], t + 0.1);
        // Far enough on that the next click is not a double-click.
        t + 0.6
    }

    #[test]
    fn text_box_edit_round_trips_through_the_project_and_undo() {
        let (mut v, mut p) = view_with_layout();
        v.steps.clear();
        let id = v.add_text_box_in(&mut p, [2.0, 2.0, 7.0, 3.0]).unwrap();
        let text = TextBoxSpec {
            id,
            text: "GENERAL NOTES\n1. VERIFY DIMENSIONS".into(),
            height_pt: 14.0,
            align: plan_layout::TextAlign::Center,
            bold: true,
            fit: plan_layout::TextFit::Wrap,
        };
        assert!(v.edit_text_box(&mut p, &text));
        assert!(!v.edit_text_box(&mut p, &text), "no change, no step");
        // The project JSON holds it and reads back.
        let back = load(&p).unwrap();
        let b = back.pages[1].boxes.iter().find(|b| b.id == id).unwrap();
        assert_eq!(
            b.source,
            BoxSource::Text {
                text: text.text.clone(),
                height_pt: 14.0,
                align: plan_layout::TextAlign::Center,
                bold: true,
            }
        );
        assert!(!b.border, "text boxes have no frame");
        assert_eq!(v.undo(&mut p).as_deref(), Some("Edit Text Box"));
        assert!(matches!(
            &v.layout().unwrap().pages[1].boxes[0].source,
            BoxSource::Text { text, .. } if text == "Text"
        ));
        // The dialog describes the same spec.
        let d = TextBoxDialog::new(&v.layout().unwrap().pages[1].boxes[0]).unwrap();
        assert_eq!(d.spec().text, "Text");
        assert!(TextBoxDialog::new(&LayoutBox::new(
            9,
            (Point::new(0.0, 0.0), Point::new(1.0, 1.0)),
            BoxSource::Image { path: "x".into() },
            Scale::QuarterInch
        ))
        .is_none());
    }

    #[test]
    fn double_clicking_a_text_box_opens_the_text_dialog_and_ok_applies_it() {
        let (ctx, mut v, mut cx, _) = interactive();
        v.selected = None;
        let id = v
            .add_text_box_in(&mut cx.project, [2.0, 2.0, 7.0, 3.0])
            .unwrap();
        frame(&ctx, &mut v, &mut cx, vec![], 1.0);
        let c = paper_point_to_screen(&v, 4.5, 2.5);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(c)],
            1.1,
        );
        for k in 0..2 {
            let t = 1.2 + 0.1 * f64::from(k);
            frame(&ctx, &mut v, &mut cx, vec![press(c, true)], t);
            frame(&ctx, &mut v, &mut cx, vec![press(c, false)], t + 0.05);
        }
        assert!(
            v.dialogs.text_box.is_some(),
            "text boxes edit in their own dialog"
        );
        assert!(v.dialogs.spec.is_none());
        let mut d = v.dialogs.text_box.take().unwrap();
        assert_eq!(d.spec().id, id);
        let _ = d.show(&ctx);
        let spec = TextBoxSpec {
            text: "HELLO".into(),
            align: plan_layout::TextAlign::Right,
            ..d.spec().clone()
        };
        assert!(v.edit_text_box(&mut cx.project, &spec));
        frame(&ctx, &mut v, &mut cx, vec![], 3.0);
    }

    #[test]
    fn layout_cad_lines_boxes_polylines_and_text_land_on_the_page_and_print() {
        let (mut v, mut p) = view_with_layout();
        v.steps.clear();
        let l = v
            .add_cad_line(&mut p, Point::new(1.0, 2.0), Point::new(4.0, 2.0))
            .unwrap();
        v.add_cad_box(&mut p, Point::new(5.0, 5.0), Point::new(7.0, 6.0))
            .unwrap();
        assert!(v
            .add_cad_polyline(&mut p, vec![Point::new(1.0, 1.0)])
            .is_none());
        v.add_cad_polyline(
            &mut p,
            vec![
                Point::new(1.0, 8.0),
                Point::new(2.0, 9.0),
                Point::new(3.0, 8.0),
            ],
        )
        .unwrap();
        assert!(v.apply_cad_text(
            &mut p,
            &CadTextSpec {
                id: None,
                pos: Point::new(1.0, 10.0),
                text: "SEE DETAIL 3".into(),
                height_in: 0.2,
            }
        ));
        let page = &load(&p).unwrap().pages[1];
        assert_eq!(page.cad.len(), 4);
        let pdf = print_bytes(&load(&p).unwrap(), &p, None);
        let t: String = pdf.iter().map(|&b| b as char).collect();
        assert!(t.contains("72 144 m 288 144 l S"), "the line prints");
        assert!(t.contains("(SEE DETAIL 3)"));
        // Edit the text, then delete the line; each is one undo step.
        let text_id = page
            .cad
            .iter()
            .find(|o| matches!(o.item, CadItem::Text { .. }))
            .unwrap()
            .id;
        assert!(v.apply_cad_text(
            &mut p,
            &CadTextSpec {
                id: Some(text_id),
                pos: Point::new(1.0, 10.0),
                text: "SEE DETAIL 4".into(),
                height_in: 0.2,
            }
        ));
        v.selected_cad = Some(l);
        assert!(v.delete_selected_cad(&mut p));
        assert_eq!(v.layout().unwrap().pages[1].cad.len(), 3);
        assert_eq!(v.undo(&mut p).as_deref(), Some("Delete Layout CAD"));
        assert_eq!(v.layout().unwrap().pages[1].cad.len(), 4);
        assert_eq!(v.undo(&mut p).as_deref(), Some("Edit Layout Text"));
    }

    #[test]
    fn the_line_box_and_text_tools_draw_with_the_mouse() {
        let (ctx, mut v, mut cx, id) = interactive();
        v.steps.clear();
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Line), None);
        assert_eq!(v.tool, LayoutTool::Line);
        let (a, b) = (
            paper_point_to_screen(&v, 3.0, 12.0),
            paper_point_to_screen(&v, 9.0, 12.0),
        );
        drag(&ctx, &mut v, &mut cx, a, b, 1.0);
        let cad = &v.current_page().unwrap().cad;
        assert_eq!(cad.len(), 1);
        match &cad[0].item {
            CadItem::Line { a, b } => {
                assert!(
                    (a.x - 3.0).abs() < 0.1 && (b.x - 9.0).abs() < 0.1,
                    "{a:?} {b:?}"
                );
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(v.selected_cad, Some(cad[0].id));
        // The plan view box was not moved by the drag.
        assert!(v.current_page().unwrap().boxes.iter().any(|x| x.id == id));
        // Box tool.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Box), None);
        let (a, b) = (
            paper_point_to_screen(&v, 3.0, 14.0),
            paper_point_to_screen(&v, 6.0, 16.0),
        );
        drag(&ctx, &mut v, &mut cx, a, b, 2.0);
        assert_eq!(v.current_page().unwrap().cad.len(), 2);
        // Polyline tool: three clicks and a double-click.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Polyline), None);
        let mut t = 3.0;
        for (x, y) in [(10.0, 13.0), (12.0, 15.0), (14.0, 13.0)] {
            let at = paper_point_to_screen(&v, x, y);
            t = click_at(&ctx, &mut v, &mut cx, at, t);
        }
        assert_eq!(v.poly.len(), 3);
        let at = paper_point_to_screen(&v, 14.0, 13.0);
        frame(
            &ctx,
            &mut v,
            &mut cx,
            vec![egui::Event::PointerMoved(at)],
            t,
        );
        for k in 0..2 {
            let tt = t + 0.1 + 0.1 * f64::from(k);
            frame(&ctx, &mut v, &mut cx, vec![press(at, true)], tt);
            frame(&ctx, &mut v, &mut cx, vec![press(at, false)], tt + 0.05);
        }
        assert!(v.poly.is_empty());
        let pl = v.current_page().unwrap().cad.last().unwrap();
        assert!(
            matches!(&pl.item, CadItem::Polyline { points, closed: false } if points.len() == 3)
        );
        // Text tool: a click opens the prompt.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Text), None);
        let at = paper_point_to_screen(&v, 3.0, 17.0);
        click_at(&ctx, &mut v, &mut cx, at, 6.0);
        assert!(v.dialogs.cad_text.is_some());
        // Esc leaves the tool.
        v.dialogs.cad_text = None;
        let esc = egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut v, &mut cx, vec![esc], 7.0);
        assert_eq!(v.tool, LayoutTool::Select);
        // Each drawing is its own undo step.
        let labels: Vec<&str> = v.steps.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Add Layout Line",
                "Add Layout Box Outline",
                "Add Layout Polyline"
            ]
        );
    }

    #[test]
    fn page_cad_selects_by_click_and_deletes_with_the_key() {
        let (ctx, mut v, mut cx, _) = interactive();
        let l = v
            .add_cad_line(
                &mut cx.project,
                Point::new(3.0, 12.0),
                Point::new(9.0, 12.0),
            )
            .unwrap();
        v.selected_cad = None;
        v.steps.clear();
        frame(&ctx, &mut v, &mut cx, vec![], 1.0);
        let at = paper_point_to_screen(&v, 6.0, 12.02);
        click_at(&ctx, &mut v, &mut cx, at, 1.1);
        assert_eq!(v.selected_cad, Some(l));
        assert_eq!(v.selected, None);
        let key = egui::Event::Key {
            key: egui::Key::Delete,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, &mut v, &mut cx, vec![key], 2.0);
        assert!(v.current_page().unwrap().cad.is_empty());
    }

    #[test]
    fn a_rotated_box_is_hit_and_handled_by_its_turned_content() {
        // A 6" x 2" box turned 90 degrees covers 2" x 6" about the same centre.
        let mut b = LayoutBox::new(
            1,
            (Point::new(2.0, 4.0), Point::new(8.0, 6.0)),
            BoxSource::text("NOTE", 10.0),
            Scale::QuarterInch,
        );
        b.clip = false;
        assert_eq!(content_bounds(&b), bounds(&b));
        b.rotation_deg = 90.0;
        assert_eq!(content_bounds(&b), [4.0, 2.0, 6.0, 8.0]);
        assert_eq!(hit_bounds(&b), [2.0, 2.0, 8.0, 8.0]);
        let page = LayoutPage {
            number: 1,
            title: "t".into(),
            boxes: vec![b.clone()],
            cad: vec![],
            template_page: false,
            leaders: vec![],
            clouds: vec![],
            size_override_in: None,
            no_title_block: false,
            label: String::new(),
            description: String::new(),
            comments: String::new(),
            in_layout_table: true,
            template: None,
            revisions: vec![],
            baked: None,
        };
        assert_eq!(
            box_at(&page, 5.0, 7.5),
            Some(1),
            "the turned content, above the frame"
        );
        assert_eq!(box_at(&page, 5.0, 2.5), Some(1), "and below it");
        assert_eq!(box_at(&page, 2.5, 7.5), None, "empty corner of the union");
        // A clipping box shows nothing outside its frame, so it is not hit there.
        let mut clipped = page.clone();
        clipped.boxes[0].clip = true;
        assert_eq!(box_at(&clipped, 5.0, 7.5), None);
        assert_eq!(box_at(&clipped, 5.0, 5.0), Some(1));
        // The rotate knob sits above what the box covers.
        let (kx, ky) = rotate_knob(hit_bounds(&b), 20.0);
        assert!((kx - 5.0).abs() < 1e-9 && ky > 8.0);
        assert!(on_rotate_knob(hit_bounds(&b), kx + 0.1, ky - 0.1, 20.0));
        assert!(!on_rotate_knob(hit_bounds(&b), 5.0, 5.0, 20.0));
    }

    #[test]
    fn the_rotate_knob_and_command_turn_the_selected_box() {
        let (ctx, mut v, mut cx, id) = interactive();
        v.steps.clear();
        v.selected = Some(id);
        frame(&ctx, &mut v, &mut cx, vec![], 1.0);
        let b = v.selected_box().unwrap().clone();
        let (kx, ky) = rotate_knob(hit_bounds(&b), v.last_xf.unwrap().z);
        let at = paper_point_to_screen(&v, kx, ky);
        click_at(&ctx, &mut v, &mut cx, at, 1.1);
        assert_eq!(v.selected_box().unwrap().quarter_turns(), 1);
        assert_eq!(v.selected, Some(id), "the knob keeps the selection");
        v.run(&mut cx, LayoutCommand::RotateBox, None);
        assert_eq!(v.selected_box().unwrap().rotation_deg, 180.0);
        for _ in 0..2 {
            v.run(&mut cx, LayoutCommand::RotateBox, None);
        }
        assert_eq!(v.selected_box().unwrap().quarter_turns(), 0);
        assert_eq!(v.steps.len(), 4);
        assert_eq!(
            v.undo(&mut cx.project).as_deref(),
            Some("Rotate Layout Box")
        );
        // The box key changes with the rotation, so the drawing redraws.
        let mut b2 = b.clone();
        b2.rotation_deg = 90.0;
        assert_ne!(box_key(&b, 1), box_key(&b2, 1));
    }

    #[test]
    fn a_perspective_camera_box_renders_an_image_into_the_pdf_and_caches_it() {
        set_perspective_size(32, 24, 1);
        let mut p = project();
        let id = p.add_camera(plan_core::CameraObject::new(
            plan_core::camera::CameraKind::FullCamera,
            Point::new(240.0, -100.0),
            90.0,
            "Front View",
            0,
        ));
        let mut v = LayoutView::default();
        v.create(&mut p, None);
        let spec = SendSpec {
            source: SendSource::Perspective {
                id,
                name: "Front View".into(),
            },
            ..plan_spec(0)
        };
        let box_id = v.send(&mut p, &spec, None).unwrap();
        let b = v.layout().unwrap().pages[1]
            .boxes
            .iter()
            .find(|b| b.id == box_id)
            .unwrap()
            .clone();
        assert!(matches!(b.source, BoxSource::Perspective { camera_id } if camera_id == id));
        assert_eq!(b.label.as_deref(), Some("FRONT VIEW"));
        assert!(
            cached_perspective(&p, &test_request(id)).is_some(),
            "rendered when sent"
        );
        let pdf = print_bytes(&load(&p).unwrap(), &p, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        assert!(text.contains("/Subtype/Image"), "an image XObject");
        assert!(text.contains("/Width 32 /Height 24"));
        // Unchanged plan: Update Views renders nothing new.
        assert_eq!(v.render_perspectives(&p), 0);
        // The screen artwork shows the cached image.
        let mut cache = BoxCache::default();
        let mut rcx = None;
        let art = box_art(&mut cache, &mut rcx, &p, 1, &b, &no_layout());
        assert_eq!(art.images.len(), 1);
        drop(rcx);
        // Changing the model makes it stale: the screen shows a placeholder
        // until Update Views renders again.
        p.add_wall(
            0,
            Point::new(10.0, 10.0),
            Point::new(60.0, 10.0),
            6.0,
            96.0,
            plan_core::WallKind::Interior,
        );
        assert!(cached_perspective(&p, &test_request(id)).is_none());
        assert_eq!(v.render_perspectives(&p), 1);
        assert!(cached_perspective(&p, &test_request(id)).is_some());
        // The send source for a full camera is a perspective, not an elevation.
        assert!(matches!(
            v.send_source(&p, Some(id), 0),
            SendSource::Perspective { .. }
        ));
    }

    #[test]
    fn schedule_boxes_refresh_with_update_views() {
        let (mut v, mut p) = view_with_layout();
        let a = p.floors[0].walls[0].id;
        p.add_opening(0, a, 100.0, plan_core::OpeningKind::Door)
            .unwrap();
        let id = v
            .add_source_box(
                &mut p,
                "Add Schedule",
                BoxSource::Schedule {
                    kind: plan_layout::ScheduleKind::Door,
                },
            )
            .unwrap();
        let b = v.selected_box().unwrap().clone();
        assert_eq!(b.id, id);
        let mut cache = BoxCache::default();
        let mut rcx = None;
        let before = box_art(&mut cache, &mut rcx, &p, 1, &b, &no_layout());
        assert!(before.texts.iter().any(|t| t.text == "Door Schedule"));
        drop(rcx);
        // A second door appears in the table once the signature moves on.
        p.add_opening(0, a, 300.0, plan_core::OpeningKind::Door)
            .unwrap();
        let mut rcx2 = None;
        // The box keeps its size, so let the new row show.
        let mut open_box = b.clone();
        open_box.clip = false;
        let after = box_art(&mut cache, &mut rcx2, &p, 2, &open_box, &no_layout());
        assert!(after.texts.len() > before.texts.len());
        drop(rcx2);
        // Update Views clears the box cache.
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        v.cache.map.insert(id, (0, Rc::new(BoxArtwork::default())));
        v.run(&mut cx, LayoutCommand::UpdateViews, None);
        assert!(v.cache.map.is_empty());
    }

    #[test]
    fn materials_and_picture_boxes_come_from_the_plan_and_a_file() {
        use plan_docs::MasterList;
        use_memory_master_list(MasterList::default());
        let (mut v, mut p) = view_with_layout();
        let id = v
            .add_materials_box(&mut p, None, Some("Siding".into()))
            .unwrap();
        let b = v.selected_box().unwrap().clone();
        assert_eq!(b.id, id);
        let mut cache = BoxCache::default();
        let mut rcx = None;
        let art = box_art(&mut cache, &mut rcx, &p, 1, &b, &no_layout());
        assert!(art
            .texts
            .iter()
            .any(|t| t.text == "MATERIALS LIST - SIDING"));
        drop(rcx);
        // A PNG on disk becomes an image box.
        let dir = std::env::temp_dir().join(format!("ps-picture-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("pic.png");
        let png = plan_render::encode_png(&plan_render::Image {
            width: 4,
            height: 2,
            rgba: vec![200; 4 * 2 * 4],
            hdr: Vec::new(),
        });
        std::fs::write(&file, png).unwrap();
        let path = file.to_string_lossy().into_owned();
        assert!(v.add_image_box(&mut p, "/no/such/file.png").is_err());
        let pic = v.add_image_box(&mut p, &path).unwrap();
        let pdf = print_bytes(&load(&p).unwrap(), &p, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        assert!(
            text.contains("/Width 4 /Height 2"),
            "the picture is embedded"
        );
        let b = v.selected_box().unwrap().clone();
        assert_eq!(b.id, pic);
        let mut rcx = None;
        let art = box_art(&mut cache, &mut rcx, &p, 2, &b, &no_layout());
        assert_eq!(art.images.len(), 1);
        drop(rcx);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn print_to_pdf_honors_paper_scale_and_tiling_and_modes() {
        use plan_layout::{PaperSize, PrintColor, PrintScale};
        let (mut v, mut p) = view_with_layout();
        v.send(&mut p, &plan_spec(0), None).unwrap();
        let layout = v.layout().unwrap().clone();
        let n = layout.content_pages().len();
        let sheet = layout.sheet_inches();
        // The dialog's tiling arithmetic agrees with the PDF.
        let mut d = PrintDialog::for_layout(n, sheet, "L");
        let _ = &mut d;
        let opts = PrintOptions {
            paper: PaperSize::Standard(plan_docs::SheetSize::Letter),
            landscape: false,
            scale: PrintScale::Percent(40.0),
            tiling: true,
            color: PrintColor::Grayscale,
            line_weights: false,
            ..PrintOptions::default()
        };
        let g = plan_layout::tile_grid(sheet, 0.4, opts.printable_in(), opts.overlap_in);
        let pdf = print_with(&layout, &p, &opts);
        let t: String = pdf.iter().map(|&b| b as char).collect();
        assert_eq!(t.matches("/Type /Page ").count(), g.count() * n);
        assert!(
            !t.contains(" rg") && !t.contains(" RG"),
            "grayscale has no colour operators"
        );
        assert!(t.contains("/MediaBox [0 0 612 792]"));
        assert_eq!(t.matches("/Title (").count(), n, "a bookmark per sheet");
        // A plan view prints at its drawing scale.
        let (pdf, scale) = print_plan(
            &p,
            0,
            "1ST FLOOR PLAN",
            &PrintOptions {
                scale: PrintScale::Drawing(Scale::EighthInch),
                ..PrintOptions::default()
            },
        );
        assert_eq!(scale, Scale::EighthInch);
        assert!(pdf.starts_with(b"%PDF"));
        // Print Image: a PNG of the plan.
        let png = plan_png(&p, 0, "All", None, 300);
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    }

    #[test]
    fn file_print_opens_the_plan_dialog_without_a_layout_and_the_layout_dialog_with_one() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.run(&mut cx, LayoutCommand::PrintDialog, None);
        assert!(v.layout().is_none(), "printing the plan makes no layout");
        let d = v.dialogs.print.take().unwrap();
        assert!(matches!(d.target(), PrintTarget::PlanView { floor: 0, .. }));
        v.run(&mut cx, LayoutCommand::PrintImage, None);
        assert!(v.dialogs.image.take().is_some());
        // With the layout view open the layout prints.
        v.create(&mut cx.project, None);
        v.active = true;
        v.run(&mut cx, LayoutCommand::PrintDialog, None);
        let d = v.dialogs.print.take().unwrap();
        assert!(matches!(d.target(), PrintTarget::Layout { .. }));
        // Accepting delivers (skipped under test) and reports it.
        v.finish_print(&mut cx, &d);
        assert_eq!(cx.status, "Print skipped in tests");
        // Print Preview applies the paper and scale to the drawing sheet.
        let plan = PrintDialog::for_plan(0, "Default Set", "1ST FLOOR PLAN");
        v.preview_print(&mut cx, &plan);
        assert!(cx
            .view_flags
            .contains(&crate::toolbar::ViewFlag::PrintPreview));
        assert!(cx
            .view_flags
            .contains(&crate::toolbar::ViewFlag::DrawingSheet));
        assert_eq!(cx.sheet.size, plan_docs::SheetSize::Letter);
        // ... and shows the colour mode the dialog chose.
        assert_eq!(
            crate::editor::sheet::preview_color(),
            plan_layout::PrintColor::Color
        );
        let mut plan = plan;
        plan.set_color(plan_layout::PrintColor::BlackWhite);
        v.preview_print(&mut cx, &plan);
        assert_eq!(
            crate::editor::sheet::preview_color(),
            plan_layout::PrintColor::BlackWhite
        );
        assert!(cx.status.contains("Black and white"), "{}", cx.status);
        crate::editor::sheet::set_preview_color(plan_layout::PrintColor::Color);
    }

    #[test]
    fn the_new_toolbar_row_and_dialogs_draw_headless() {
        let ctx = egui::Context::default();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.active = true;
        v.add_text_box_in(&mut cx.project, [2.0, 2.0, 6.0, 3.0])
            .unwrap();
        v.add_cad_line(&mut cx.project, Point::new(1.0, 1.0), Point::new(3.0, 1.0));
        v.selected = None;
        v.dialogs.text_box = TextBoxDialog::new(&v.layout().unwrap().pages[1].boxes[0]);
        v.dialogs.cad_text = Some(CadTextDialog::new(CadTextSpec {
            id: None,
            pos: Point::new(1.0, 1.0),
            text: "T".into(),
            height_in: 0.125,
        }));
        v.dialogs.print = v.layout_print_dialog();
        v.dialogs.image = Some(ImageDialog::new(0, "All"));
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| v.show(ctx, ui, &mut cx));
                v.show_dialogs(ctx, &mut cx);
            });
        }
    }

    // ----- page drawings: select, move, resize -----

    fn select_tool_drag(
        ctx: &egui::Context,
        v: &mut LayoutView,
        cx: &mut EditorContext,
        from: (f64, f64),
        to: (f64, f64),
        t: f64,
    ) -> f64 {
        let (a, b) = (
            paper_point_to_screen(v, from.0, from.1),
            paper_point_to_screen(v, to.0, to.1),
        );
        drag(ctx, v, cx, a, b, t)
    }

    #[test]
    fn dragging_a_page_drawing_moves_it_and_undo_restores_it() {
        let (ctx, mut v, mut cx, _) = interactive();
        let l = v
            .add_cad_line(
                &mut cx.project,
                Point::new(3.0, 12.0),
                Point::new(9.0, 12.0),
            )
            .unwrap();
        v.steps.clear();
        v.selected_cad = None;
        select_tool_drag(&ctx, &mut v, &mut cx, (6.0, 12.0), (6.0, 14.0), 1.0);
        assert_eq!(v.selected_cad, Some(l));
        let line = &v.current_page().unwrap().cad[0].item;
        let CadItem::Line { a, b } = line else {
            panic!()
        };
        assert!(
            (a.y - 14.0).abs() < 0.1 && (b.y - 14.0).abs() < 0.1,
            "{a:?} {b:?}"
        );
        assert!((a.x - 3.0).abs() < 1e-9, "only moved up");
        assert_eq!(v.steps.len(), 1);
        assert_eq!(v.steps[0].0, "Move Layout Drawing");
        // The step restores the line where it was.
        let mut cxu = EditorContext::new(plan_defaults::embedded());
        cxu.project = v.steps[0].1.clone();
        let back = load(&cxu.project).unwrap();
        assert_eq!(
            back.pages[1].cad[0].item,
            CadItem::Line {
                a: Point::new(3.0, 12.0),
                b: Point::new(9.0, 12.0)
            }
        );
        // Dragging empty paper still pans and adds no step.
        let before = v.offset;
        select_tool_drag(&ctx, &mut v, &mut cx, (20.0, 3.0), (21.0, 3.0), 3.0);
        assert_ne!(v.offset, before);
        assert_eq!(v.steps.len(), 1);
    }

    /// A page line well clear of the box `r`, and the points on it to grab.
    fn line_clear_of(v: &LayoutView, id: Id) -> (Point, Point) {
        let (_, r) = {
            let b = v
                .current_page()
                .unwrap()
                .boxes
                .iter()
                .find(|b| b.id == id)
                .unwrap();
            (0, bounds(b))
        };
        let (lo, hi) = v.layout().unwrap().drawing_area();
        let y = if r[1] - lo.y > 1.0 {
            r[1] - 0.5
        } else {
            (r[3] + 0.5).min(hi.y - 0.1)
        };
        let x0 = (r[0] + 0.5).max(lo.x + 0.1);
        (Point::new(x0, y), Point::new(x0 + 2.0, y))
    }

    #[test]
    fn dragging_a_box_moves_the_page_drawings_selected_with_it() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (a, b) = line_clear_of(&v, id);
        let l = v.add_cad_line(&mut cx.project, a, b).unwrap();
        v.steps.clear();
        v.select_boxes(&[id]);
        v.extend_cad_selection(l);
        assert_eq!(v.selection_ids(), vec![id]);
        assert_eq!(v.cad_selection_ids(), vec![l]);
        frame(&ctx, &mut v, &mut cx, vec![], 0.5);
        let (c, before) = center_of(&v, id);
        drag(&ctx, &mut v, &mut cx, c, c + Vec2::new(40.0, 25.0), 1.0);
        let (_, after) = center_of(&v, id);
        let (dx, dy) = (after[0] - before[0], after[1] - before[1]);
        assert!(dx > 0.0 && dy != 0.0, "{dx} {dy}");
        let CadItem::Line { a: a2, b: b2 } = v.current_page().unwrap().cad[0].item.clone() else {
            panic!("a line")
        };
        assert!(((a2.x - a.x) - dx).abs() < 1e-9 && ((a2.y - a.y) - dy).abs() < 1e-9);
        assert!(((b2.x - b.x) - dx).abs() < 1e-9 && ((b2.y - b.y) - dy).abs() < 1e-9);
        // One undo step puts the box and the line back.
        assert_eq!(v.steps.len(), 1);
        assert_eq!(v.undo(&mut cx.project).as_deref(), Some("Move Layout Box"));
        assert_eq!(center_of(&v, id).1, before);
        let CadItem::Line { a: a3, .. } = v.current_page().unwrap().cad[0].item.clone() else {
            panic!("a line")
        };
        assert_eq!(a3, a);
        // A drawing left out of the selection stays where it is.
        v.select_boxes(&[id]);
        frame(&ctx, &mut v, &mut cx, vec![], 3.0);
        let (c, _) = center_of(&v, id);
        drag(&ctx, &mut v, &mut cx, c, c + Vec2::new(30.0, 0.0), 4.0);
        let CadItem::Line { a: a4, .. } = v.current_page().unwrap().cad[0].item.clone() else {
            panic!("a line")
        };
        assert_eq!(a4, a, "not selected, not moved");
    }

    #[test]
    #[ignore = "R16-02 in progress: group drag of page CAD"]
    fn dragging_a_selected_page_drawing_moves_the_boxes_selected_with_it() {
        let (ctx, mut v, mut cx, id) = interactive();
        let (a, b) = line_clear_of(&v, id);
        let l = v.add_cad_line(&mut cx.project, a, b).unwrap();
        v.steps.clear();
        v.select_boxes(&[id]);
        v.extend_cad_selection(l);
        frame(&ctx, &mut v, &mut cx, vec![], 0.5);
        let (_, before) = center_of(&v, id);
        let mid = ((a.x + b.x) / 2.0, a.y);
        select_tool_drag(&ctx, &mut v, &mut cx, mid, (mid.0 + 0.5, mid.1 + 0.25), 1.0);
        let (_, after) = center_of(&v, id);
        let CadItem::Line { a: a2, .. } = v.current_page().unwrap().cad[0].item.clone() else {
            panic!("a line")
        };
        let (dx, dy) = (a2.x - a.x, a2.y - a.y);
        assert!(dx.abs() > 1e-6 || dy.abs() > 1e-6);
        assert!(((after[0] - before[0]) - dx).abs() < 1e-9);
        assert!(((after[1] - before[1]) - dy).abs() < 1e-9);
        assert_eq!(v.steps.len(), 1);
        assert_eq!(v.steps[0].0, "Move Layout Drawing");
        // Shift-clicking the drawing again takes it out of the selection.
        v.extend_cad_selection(l);
        assert!(v.cad_selection_ids().is_empty());
    }

    #[test]
    fn handles_resize_a_page_drawing_and_leaders_and_clouds_move_too() {
        let (ctx, mut v, mut cx, _) = interactive();
        let l = v
            .add_cad_line(
                &mut cx.project,
                Point::new(3.0, 12.0),
                Point::new(9.0, 12.0),
            )
            .unwrap();
        v.steps.clear();
        assert_eq!(v.selected_cad, Some(l));
        frame(&ctx, &mut v, &mut cx, vec![], 0.5);
        // The east handle of the line's bounds (9, 12) to x = 11.
        select_tool_drag(&ctx, &mut v, &mut cx, (9.0, 12.0), (11.0, 12.0), 1.0);
        let CadItem::Line { a, b } = &v.current_page().unwrap().cad[0].item else {
            panic!()
        };
        assert!(
            (a.x - 3.0).abs() < 1e-9 && (b.x - 11.0).abs() < 0.1,
            "{a:?} {b:?}"
        );
        assert_eq!(v.steps.last().unwrap().0, "Resize Layout Drawing");
        // A leader and a cloud move by dragging them.
        v.apply_leader(
            &mut cx.project,
            &LeaderSpec {
                id: None,
                tip: Point::new(12.0, 8.0),
                elbow: Point::new(14.0, 9.0),
                bends: Vec::new(),
                text: "SEE DETAIL".into(),
                height_in: 0.125,
                arrow: true,
            },
        );
        let ld = v.selected_cad.unwrap();
        v.selected_cad = None;
        frame(&ctx, &mut v, &mut cx, vec![], 6.0);
        select_tool_drag(&ctx, &mut v, &mut cx, (13.0, 8.5), (13.0, 7.5), 7.0);
        assert_eq!(v.selected_cad, Some(ld));
        let leader = &v.current_page().unwrap().leaders[0];
        assert!((leader.tip.y - 7.0).abs() < 0.1 && (leader.elbow.y - 8.0).abs() < 0.1);
        // Keys nudge the selected drawing by 1/16".
        let moved_to = leader.tip;
        v.nudge_annotation(&mut cx.project, ld, 0.0625, 0.0);
        assert!((v.current_page().unwrap().leaders[0].tip.x - moved_to.x - 0.0625).abs() < 1e-9);
    }

    #[test]
    fn leaders_clouds_arcs_and_circles_add_edit_delete_with_undo() {
        let (mut v, mut p) = view_with_layout();
        v.steps.clear();
        let spec = LeaderSpec {
            id: None,
            tip: Point::new(5.0, 5.0),
            elbow: Point::new(7.0, 6.0),
            bends: Vec::new(),
            text: "NEW 2x6 WALL".into(),
            height_in: 0.125,
            arrow: true,
        };
        assert!(v.apply_leader(&mut p, &spec));
        let ld = v.selected_cad.unwrap();
        assert_eq!(v.current_page().unwrap().leaders.len(), 1);
        // Edit: text changes, arrow off; an unchanged edit makes no step.
        let edit = LeaderSpec {
            id: Some(ld),
            text: "NEW 2x4 WALL".into(),
            arrow: false,
            ..spec.clone()
        };
        assert!(v.apply_leader(&mut p, &edit));
        assert!(!v.apply_leader(&mut p, &edit));
        let l = &v.current_page().unwrap().leaders[0];
        assert_eq!((l.text.as_str(), l.arrow), ("NEW 2x4 WALL", false));
        // Cloud with a revision tag; editing only changes the tag.
        let cs = CloudSpec {
            id: None,
            rect: (Point::new(8.0, 8.0), Point::new(11.0, 10.0)),
            revision: "1".into(),
        };
        assert!(v.apply_cloud(&mut p, &cs));
        let cl = v.selected_cad.unwrap();
        assert!(v.apply_cloud(
            &mut p,
            &CloudSpec {
                id: Some(cl),
                revision: "2".into(),
                ..cs.clone()
            }
        ));
        assert_eq!(v.current_page().unwrap().clouds[0].revision, "2");
        // Circle and arc.
        assert!(v
            .add_cad_circle(&mut p, Point::new(14.0, 10.0), 1.0)
            .is_some());
        assert!(v
            .add_cad_circle(&mut p, Point::new(14.0, 10.0), 0.0)
            .is_none());
        assert!(v
            .add_cad_arc(
                &mut p,
                Point::new(4.0, 14.0),
                Point::new(5.0, 14.0),
                Point::new(4.0, 15.0)
            )
            .is_some());
        let page = v.current_page().unwrap();
        assert_eq!(
            (page.cad.len(), page.leaders.len(), page.clouds.len()),
            (2, 1, 1)
        );
        assert!(page.cad.iter().all(|o| o.layer == LAYER_CAD));
        // Every kind prints.
        let pdf = print_bytes(v.layout().unwrap(), &p, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        assert!(text.contains("NEW 2x4 WALL"));
        // Delete the cloud, then undo it.
        v.selected_cad = Some(cl);
        assert!(v.delete_selected_cad(&mut p));
        assert!(v.current_page().unwrap().clouds.is_empty());
        let label = v.undo(&mut p);
        assert_eq!(label.as_deref(), Some("Delete Layout CAD"));
        assert_eq!(v.current_page().unwrap().clouds.len(), 1);
        // The undo history names every step.
        let page = v.current_page().unwrap();
        assert_eq!(page.clouds[0].revision, "2");
    }

    #[test]
    fn reordering_pages_keeps_the_numbering_and_the_sheet_index_follows() {
        let mut l = Layout::new("t", plan_docs::SheetSize::ArchC);
        for (n, t) in [(0, "Cover"), (1, "Plan"), (2, "Elevations")] {
            l.add_page(n, t);
        }
        // Cover to second place: it is no longer first, but the set still
        // runs A-0, A-1, A-2 and the index reads the new order.
        assert_eq!(exchange_page(&mut l, 0, true), Some(1));
        let rows: Vec<(String, String)> = l
            .content_pages()
            .iter()
            .map(|p| (p.sheet_number(), p.title.clone()))
            .collect();
        assert_eq!(
            rows,
            [
                ("A-0".to_string(), "Plan".to_string()),
                ("A-1".to_string(), "Cover".to_string()),
                ("A-2".to_string(), "Elevations".to_string())
            ]
        );
        // The same through the printed index.
        let p = project();
        l.pages[1].boxes.push(plan_layout::LayoutBox::new(
            1,
            (Point::new(1.0, 1.0), Point::new(6.0, 4.0)),
            BoxSource::SheetIndex,
            plan_docs::Scale::QuarterInch,
        ));
        let pdf = print_bytes(&l, &p, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        let (plan_at, cover_at) = (text.find("(PLAN)"), text.find("(COVER)"));
        assert!(plan_at.is_some() && cover_at.is_some());
        // Pages with a template page keep their own number.
        l.add_page(9, "Template").template_page = true;
        assert_eq!(exchange_page(&mut l, 1, false), Some(0));
        assert_eq!(l.page(9).map(|t| t.number), Some(9));
        assert_eq!(
            l.content_pages()
                .iter()
                .map(|p| p.number)
                .collect::<Vec<_>>(),
            [0, 1, 2]
        );
    }

    #[test]
    fn a_layout_saves_as_a_template_and_new_layouts_start_from_the_default() {
        let dir =
            std::env::temp_dir().join(format!("plan-studio-lw-templates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        crate::dialogs::layout::set_layout_templates_dir_for_tests(Some(dir.clone()));
        let (mut v, mut p) = view_with_layout();
        let pages_before = v.layout().unwrap().pages.len();
        assert!(v.add_page(&mut p, false));
        v.add_text_box(&mut p, "SEE PLAN", 12.0).unwrap();
        let pages = v.layout().unwrap().pages.len();
        assert_eq!(pages, pages_before + 1);
        // Save As Template (the default for this sheet size).
        let said = v.save_template("My Set", true);
        assert!(said.starts_with("Saved the layout template"), "{said}");
        assert!(dir.join("My Set.layout.json").exists());
        // A new layout of that sheet size starts from it.
        let mut p2 = project();
        let mut v2 = LayoutView::default();
        assert!(v2.create(&mut p2, None));
        assert_eq!(v2.layout().unwrap().pages.len(), pages);
        assert!(
            v2.layout().unwrap().name.ends_with("Layout"),
            "keeps its own name"
        );
        // Without the default flag a new layout is the plain one.
        let said = v.save_template("My Set", false);
        assert!(said.starts_with("Saved"), "{said}");
        let mut p3 = project();
        let mut v3 = LayoutView::default();
        assert!(v3.create(&mut p3, None));
        assert_eq!(v3.layout().unwrap().pages.len(), pages_before);
        // Apply Template replaces the layout in one undo step.
        let t = crate::dialogs::layout::list_layout_templates(&dir).remove(0);
        assert!(v3.apply_template(&mut p3, &t));
        assert_eq!(v3.layout().unwrap().pages.len(), pages);
        assert_eq!(v3.undo(&mut p3).as_deref(), Some("Apply Layout Template"));
        assert_eq!(v3.layout().unwrap().pages.len(), pages_before);
        // Boxes of this plan's cameras are not in the template.
        let t = crate::dialogs::layout::list_layout_templates(&dir).remove(0);
        assert!(t
            .layout
            .pages
            .iter()
            .flat_map(|p| &p.boxes)
            .all(|b| !matches!(
                b.source,
                BoxSource::Camera { .. } | BoxSource::Perspective { .. }
            )));
        crate::dialogs::layout::set_layout_templates_dir_for_tests(None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn perspective_boxes_are_lit_by_the_plans_point_lights() {
        set_perspective_size(24, 16, 1);
        let mut p = project();
        let id = p.add_camera(plan_core::CameraObject::new(
            plan_core::camera::CameraKind::FullCamera,
            Point::new(100.0, 100.0),
            0.0,
            "Living",
            0,
        ));
        let req = test_request(id);
        let dark = perspective_image(&p, &req).expect("a render");
        let mut light = plan_core::camera::PlanLight::new(Point::new(150.0, 100.0), 70.0);
        light.intensity = 6.0;
        p.add_light(0, light).unwrap();
        let lit = perspective_image(&p, &req).expect("a render");
        assert_eq!((dark.width, dark.height), (lit.width, lit.height));
        assert_ne!(dark.rgba, lit.rgba, "the light changes the picture");
        let sum = |i: &PerspectiveImage| i.rgba.iter().map(|&b| u64::from(b)).sum::<u64>();
        assert!(sum(&lit) > sum(&dark), "and brightens it");
        clear_perspective_size();
    }

    #[test]
    fn a_leader_can_be_clicked_out_with_bends() {
        let (ctx, mut v, mut cx, _) = interactive();
        v.steps.clear();
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Leader), None);
        // Tip, two bends, then a double-click where the text goes.
        let mut t = 1.0;
        for (x, y) in [(6.0, 8.0), (6.0, 10.0), (8.0, 11.0)] {
            let at = paper_point_to_screen(&v, x, y);
            t = click_at(&ctx, &mut v, &mut cx, at, t);
        }
        assert_eq!(v.poly.len(), 3);
        let xf = v.last_xf.unwrap();
        let end = xf.pt(11.0, 11.0);
        v.click(&mut cx, &xf, end, false);
        v.double_click(&mut cx, &xf, end);
        assert!(v.poly.is_empty());
        let mut spec = v.dialogs.leader.take().expect("the prompt").spec().clone();
        assert!((spec.tip.x - 6.0).abs() < 0.1 && (spec.tip.y - 8.0).abs() < 0.1);
        assert_eq!(spec.bends.len(), 2, "{spec:?}");
        assert!((spec.elbow.x - 11.0).abs() < 0.1);
        spec.text = "SEE NOTE 4".into();
        assert!(v.apply_leader(&mut cx.project, &spec));
        let l = &v.current_page().unwrap().leaders[0];
        assert_eq!(l.bends.len(), 2);
        assert_eq!(l.polylines()[0].len(), 5);
        // Double-clicking it later keeps the bends.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Select), None);
        let xf = v.last_xf.unwrap();
        let on = xf.pt(6.0, 9.0);
        v.double_click(&mut cx, &xf, on);
        assert_eq!(v.dialogs.leader.as_ref().unwrap().spec().bends.len(), 2);
        // One click is not a leader.
        v.dialogs.leader = None;
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Leader), None);
        let at = paper_point_to_screen(&v, 3.0, 3.0);
        click_at(&ctx, &mut v, &mut cx, at, 20.0);
        let xf = v.last_xf.unwrap();
        v.double_click(&mut cx, &xf, at);
        assert!(v.dialogs.leader.is_none());
    }

    #[test]
    fn the_circle_arc_leader_and_cloud_tools_draw_with_the_mouse() {
        let (ctx, mut v, mut cx, _) = interactive();
        v.steps.clear();
        // Circle: drag from the centre outward.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Circle), None);
        let t = select_tool_drag(&ctx, &mut v, &mut cx, (6.0, 13.0), (7.5, 13.0), 1.0);
        let CadItem::Circle { center, radius } = v.current_page().unwrap().cad[0].item.clone()
        else {
            panic!()
        };
        assert!(
            (center.x - 6.0).abs() < 0.1 && (radius - 1.5).abs() < 0.1,
            "{radius}"
        );
        // Arc: centre, start, end.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Arc), None);
        let mut t = t + 1.0;
        for (x, y) in [(12.0, 13.0), (13.0, 13.0), (12.0, 14.0)] {
            let at = paper_point_to_screen(&v, x, y);
            t = click_at(&ctx, &mut v, &mut cx, at, t);
        }
        assert!(v.poly.is_empty());
        let CadItem::Arc {
            radius,
            start_angle,
            end_angle,
            ..
        } = v.current_page().unwrap().cad[1].item.clone()
        else {
            panic!()
        };
        assert!((radius - 1.0).abs() < 0.1 && start_angle.abs() < 0.1);
        assert!((end_angle - std::f64::consts::FRAC_PI_2).abs() < 0.1);
        // Leader: drag from the tip to the text, the prompt asks for the words.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Leader), None);
        let t = select_tool_drag(&ctx, &mut v, &mut cx, (16.0, 8.0), (18.0, 9.0), t + 1.0);
        assert!(v.dialogs.leader.is_some());
        let mut spec = v.dialogs.leader.take().unwrap().spec().clone();
        assert!((spec.tip.x - 16.0).abs() < 0.1 && (spec.elbow.x - 18.0).abs() < 0.1);
        spec.text = "FIELD VERIFY".into();
        assert!(v.apply_leader(&mut cx.project, &spec));
        // Revision cloud: drag its rectangle; the prompt offers the next revision mark.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Cloud), None);
        select_tool_drag(&ctx, &mut v, &mut cx, (4.0, 3.0), (8.0, 6.0), t + 1.0);
        let cs = v.dialogs.cloud.take().unwrap().spec().clone();
        assert_eq!(cs.revision, "1");
        assert!(v.apply_cloud(&mut cx.project, &cs));
        let cs2 = CloudSpec { id: None, ..cs };
        assert!(v.apply_cloud(&mut cx.project, &cs2));
        assert_eq!(
            v.next_revision(),
            "2",
            "numbering follows the existing clouds"
        );
        let labels: Vec<&str> = v.steps.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Add Layout Circle",
                "Add Layout Arc",
                "Leader",
                "Revision Cloud",
                "Revision Cloud"
            ]
        );
        // Double-clicking a leader opens its prompt.
        v.run(&mut cx, LayoutCommand::Tool(LayoutTool::Select), None);
        let xf = v.last_xf.unwrap();
        let at = xf.pt(17.0, 8.5);
        v.double_click(&mut cx, &xf, at);
        assert_eq!(
            v.dialogs.leader.as_ref().unwrap().spec().text,
            "FIELD VERIFY"
        );
    }

    // ----- layout layers -----

    #[test]
    fn layer_display_options_hide_and_weigh_the_layout_layers() {
        let (mut v, mut p) = view_with_layout();
        let text_id = v.add_text_box(&mut p, "NOTES ON THE PLAN", 12.0).unwrap();
        let b = v.selected_box().unwrap().clone();
        assert_eq!(b.id, text_id);
        let mut layers = v.layout().unwrap().layers.clone();
        let layout = v.layout().unwrap().clone();
        let mut cache = BoxCache::default();
        let mut rcx = None;
        let shown = box_art(&mut cache, &mut rcx, &p, 1, &b, &layout);
        assert!(shown.texts.iter().any(|t| t.text.contains("NOTES")));
        drop(rcx);
        layers.set_visible(LAYER_TEXT, false);
        assert!(v.apply_layers(&mut p, &layers));
        assert!(!v.apply_layers(&mut p, &layers), "no change, no step");
        assert_eq!(v.undo_label(), Some("Layout Layer Display"));
        let layout = v.layout().unwrap().clone();
        let mut rcx = None;
        // (The window folds the layers into the signature the cache is keyed by.)
        let hidden = box_art(&mut cache, &mut rcx, &p, 2, &b, &layout);
        assert!(hidden.texts.is_empty(), "the Text layer is off");
        // Page CAD text follows the same layer; and the setting is in the print.
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p.clone();
        v.add_cad(
            &mut cx.project,
            "Add Layout Text",
            CadItem::Text {
                pos: Point::new(2.0, 2.0),
                text: "PAGE TEXT".into(),
                height: 0.2,
                angle: 0.0,
            },
        );
        let pdf = print_bytes(v.layout().unwrap(), &cx.project, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        assert!(!text.contains("PAGE TEXT"));
        assert!(!text.contains("NOTES ON THE PLAN"));
        // The dialog opens from the command, and OK applies its draft.
        v.run(&mut cx, LayoutCommand::LayerDisplay, None);
        assert!(v.dialogs.layers.is_some());
        let ctx = egui::Context::default();
        for _ in 0..2 {
            let _ = ctx.run(egui::RawInput::default(), |c| v.show_dialogs(c, &mut cx));
        }
        assert!(v.dialogs.layers.is_some(), "still open until OK");
    }

    // ----- text fit -----

    #[test]
    fn a_text_box_wraps_clips_and_can_shrink_to_fit() {
        let (mut v, mut p) = view_with_layout();
        let id = v
            .add_text_box(
                &mut p,
                "General notes: all dimensions are to face of stud unless noted otherwise",
                12.0,
            )
            .unwrap();
        v.set_box_bounds(&mut p, id, [2.0, 2.0, 3.5, 2.6], "Resize");
        let layout = v.layout().unwrap().clone();
        let art_of = |p: &Project, b: &LayoutBox, layout: &Layout| {
            let mut cache = BoxCache::default();
            let mut rcx = None;
            box_art(&mut cache, &mut rcx, p, 1, b, layout)
        };
        let b = v.selected_box().unwrap().clone();
        let wrapped = art_of(&p, &b, &layout);
        // 0.6" = 43 pt: two and a half lines of 12 pt at 1.25 spacing show; each is inside the width.
        assert!(wrapped.texts.len() >= 2);
        assert!(wrapped
            .texts
            .iter()
            .all(|t| (t.size_pt - 12.0).abs() < 1e-9));
        // Shrink to fit through the text box edit.
        let spec = TextBoxSpec {
            id,
            text: "General notes: all dimensions are to face of stud unless noted otherwise".into(),
            height_pt: 12.0,
            align: plan_layout::TextAlign::Left,
            bold: false,
            fit: plan_layout::TextFit::Shrink,
        };
        assert!(v.edit_text_box(&mut p, &spec));
        let layout = v.layout().unwrap().clone();
        let b = v.selected_box().unwrap().clone();
        assert_eq!(b.text_fit, plan_layout::TextFit::Shrink);
        let shrunk = art_of(&p, &b, &layout);
        assert!(shrunk.texts.iter().all(|t| t.size_pt < 12.0));
        let words: Vec<&str> = shrunk.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            words.join(" "),
            "General notes: all dimensions are to face of stud unless noted otherwise",
            "nothing is dropped"
        );
        // The dialog shows the fit.
        assert_eq!(
            TextBoxDialog::new(&b).unwrap().spec().fit,
            plan_layout::TextFit::Shrink
        );
    }

    // ----- perspective quality and Update Views -----

    fn project_with_camera() -> (Project, Id) {
        let mut p = project();
        let id = p.add_camera(plan_core::CameraObject::new(
            plan_core::camera::CameraKind::FullCamera,
            Point::new(240.0, -100.0),
            90.0,
            "Front View",
            0,
        ));
        (p, id)
    }

    #[test]
    fn a_perspective_box_renders_at_its_own_dpi_and_samples() {
        clear_perspective_size();
        let (mut p, id) = project_with_camera();
        let mut v = LayoutView::default();
        v.create(&mut p, None);
        let spec = SendSpec {
            source: SendSource::Perspective {
                id,
                name: "Front View".into(),
            },
            ..plan_spec(0)
        };
        // Sending renders the default view; make it small by lowering the box's DPI first.
        set_perspective_size(16, 12, 1);
        let box_id = v.send(&mut p, &spec, None).unwrap();
        clear_perspective_size();
        let mut b = v.layout().unwrap().pages[1]
            .boxes
            .iter()
            .find(|b| b.id == box_id)
            .unwrap()
            .clone();
        assert_eq!(plan_layout::perspective_request(&b).unwrap().width, 480);
        b.dpi = 10;
        b.samples = 2;
        let (w, h) = b.size_in();
        let want = plan_layout::perspective_pixels(w, h, 10);
        assert_eq!(want, (60, 45));
        assert!(v.apply_spec(
            &mut p,
            &BoxSpec {
                layout_box: b.clone(),
                page: 1
            }
        ));
        let req = plan_layout::perspective_request(&b).unwrap();
        assert_eq!((req.width, req.height, req.samples), (60, 45, 2));
        assert!(cached_perspective(&p, &req).is_none());
        assert_eq!(v.render_perspectives(&p), 1);
        let img = cached_perspective(&p, &req).unwrap();
        assert_eq!((img.width, img.height), (60, 45));
        // The printed page embeds that image.
        let pdf = print_bytes(&load(&p).unwrap(), &p, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        assert!(text.contains("/Width 60 /Height 45"));
    }

    #[test]
    fn update_views_renders_on_a_thread_with_progress() {
        set_perspective_size(24, 18, 1);
        let (mut p, id) = project_with_camera();
        let mut v = LayoutView::default();
        v.create(&mut p, None);
        let spec = SendSpec {
            source: SendSource::Perspective {
                id,
                name: "Front View".into(),
            },
            ..plan_spec(0)
        };
        v.send(&mut p, &spec, None).unwrap();
        let req = test_request(id);
        assert!(cached_perspective(&p, &req).is_some());
        // Nothing stale: nothing to do.
        assert_eq!(v.start_update(&p), 0);
        assert!(!v.updating());
        // The model changes; the view is stale.
        p.add_wall(
            0,
            Point::new(10.0, 10.0),
            Point::new(60.0, 10.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        assert!(cached_perspective(&p, &req).is_none());
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        v.run(&mut cx, LayoutCommand::UpdateViews, None);
        assert!(cx.status.contains("1 perspective view"), "{}", cx.status);
        assert!(v.updating(), "the render runs on a thread");
        // A second request while it runs is refused.
        v.run(&mut cx, LayoutCommand::UpdateViews, None);
        // The progress window and toolbar bar draw while it runs.
        let ctx = egui::Context::default();
        let mut frames = 0;
        while v.updating() {
            let _ = ctx.run(egui::RawInput::default(), |c| v.show_dialogs(c, &mut cx));
            std::thread::sleep(std::time::Duration::from_millis(5));
            frames += 1;
            assert!(frames < 6000, "Update Views did not finish");
        }
        let done = cx.status.clone();
        assert!(done.contains("rendered 1 perspective view"), "{done}");
        assert!(cached_perspective(&cx.project, &req).is_some());
        // A cancelled job stops early and says so.
        cx.project.add_wall(
            0,
            Point::new(10.0, 50.0),
            Point::new(60.0, 50.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        assert_eq!(v.start_update(&cx.project), 1);
        v.update
            .as_ref()
            .unwrap()
            .cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let msg = v.wait_update().unwrap();
        assert!(msg.contains("rendered") || msg.contains("stopped"), "{msg}");
    }

    // ----- Print Model, Print Image of the 3D view, printing quality -----

    #[test]
    fn print_model_renders_a_chosen_camera_at_its_dpi() {
        let (p, id) = project_with_camera();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.run(&mut cx, LayoutCommand::PrintModel, None);
        let mut d = v.dialogs.model.take().expect("the dialog opens");
        assert_eq!(d.camera, id);
        d.dpi = 20;
        d.samples = 1;
        let (px_w, px_h) = d.pixels();
        assert!(px_w >= 200 && px_h >= 100);
        v.finish_model(&mut cx, &d);
        assert_eq!(cx.status, "Print skipped in tests");
        // The render the print asked for is now cached at that size.
        let req = PerspectiveRequest {
            camera_id: id,
            width: px_w,
            height: px_h,
            samples: 1,
        };
        assert!(cached_perspective(&cx.project, &req).is_some());
        // Without a camera the command says what to do.
        let mut bare = EditorContext::new(plan_defaults::embedded());
        bare.project = project();
        let mut v2 = LayoutView::default();
        v2.create(&mut bare.project, None);
        v2.run(&mut bare, LayoutCommand::PrintModel, None);
        assert!(v2.dialogs.model.is_none());
        assert!(
            bare.status.contains("perspective camera"),
            "{}",
            bare.status
        );
    }

    #[test]
    fn print_image_of_the_3d_view_renders_a_png_at_the_chosen_size() {
        let p = project();
        let scene = plan_3d::build_scene(&p);
        let view = crate::shell::view3d_panel::Snapshot3d {
            scene,
            camera: plan_render::Camera::from_plan(Point::new(240.0, -600.0), 90.0, 66.0, 50.0),
        };
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        let mut v = LayoutView::default();
        v.dialogs.image3d = Some((Image3dDialog::new(4.0 / 3.0), view.clone()));
        assert!(v.dialogs.any(), "the dialog blocks the canvas keys");
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |c| v.show_dialogs(c, &mut cx));
        let (mut d, view) = v.dialogs.image3d.take().unwrap();
        d.width_px = 64;
        d.height_px = 48;
        d.samples = 1;
        v.finish_image_3d(&mut cx, &d, &view);
        assert!(cx.status.starts_with("Rendered "), "{}", cx.status);
        let png = view.render_png(40, 30, 1).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        // An empty scene has nothing to print.
        let empty = crate::shell::view3d_panel::Snapshot3d {
            scene: plan_3d::Scene::default(),
            camera: view.camera,
        };
        assert!(empty.render_png(32, 32, 1).is_none());
        // The public entry opens the dialog.
        print_image_3d(&mut cx, view);
        assert!(dialog_open() || with_view(|v| v.dialogs.image3d.is_some()));
        with_view(|v| v.dialogs.image3d = None);
    }

    // ----- Send to Layout from the Project Browser, construction set, pictures -----

    #[test]
    fn send_camera_opens_send_to_layout_on_that_camera() {
        let (p, id) = project_with_camera();
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = p;
        let mut v = LayoutView::default();
        v.run(&mut cx, LayoutCommand::SendCamera(id), None);
        let d = v.dialogs.send.as_ref().expect("the dialog opens");
        assert!(matches!(&d.spec().source, SendSource::Perspective { id: i, .. } if *i == id));
        // The layout view was not forced open behind the dialog.
        assert!(v.layout().is_some());
    }

    #[test]
    fn create_construction_set_fills_the_live_layout_as_one_undo_step() {
        let mut cx = EditorContext::new(plan_defaults::embedded());
        cx.project = project();
        let mut v = LayoutView::default();
        v.create(&mut cx.project, None);
        v.steps.clear();
        v.run(&mut cx, LayoutCommand::CreateConstructionSet, None);
        assert!(
            cx.status.starts_with("Added the construction set"),
            "{}",
            cx.status
        );
        let l = v.layout().unwrap();
        let titles: Vec<&str> = l.content_pages().iter().map(|p| p.title.as_str()).collect();
        assert!(titles.contains(&"Cover") && titles.contains(&"Site Plan"));
        assert!(titles.contains(&"Details") && titles.contains(&"Schedules"));
        assert!(
            !titles.contains(&"Page 1"),
            "the empty first page is replaced: {titles:?}"
        );
        let cover = l.pages.iter().find(|p| p.title == "Cover").unwrap();
        assert!(cover
            .boxes
            .iter()
            .any(|b| b.source == BoxSource::SheetIndex));
        assert_eq!(l.pages[v.page].title, "Cover", "the window shows the cover");
        assert_eq!(v.steps.len(), 1);
        assert_eq!(v.steps[0].0, "Create Construction Set");
        // The window draws the new sheets, and the PDF carries the index.
        let ctx = egui::Context::default();
        v.active = true;
        frame(&ctx, &mut v, &mut cx, vec![], 0.0);
        frame(&ctx, &mut v, &mut cx, vec![], 0.1);
        let pdf = print_bytes(v.layout().unwrap(), &cx.project, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        assert!(text.contains("(SHEET INDEX)") && text.contains("(SITE PLAN)"));
        // Run again with no layout: the entry point makes one.
        let mut fresh = EditorContext::new(plan_defaults::embedded());
        fresh.project = project();
        let mut v2 = LayoutView::default();
        v2.run(&mut fresh, LayoutCommand::CreateConstructionSet, None);
        assert!(v2.layout().unwrap().content_pages().len() >= 9);
    }

    #[test]
    fn the_sheet_index_box_adds_and_follows_page_titles() {
        let (mut v, mut p) = view_with_layout();
        v.add_page(&mut p, false);
        v.rename_page(&mut p, 2, "Elevations");
        let id = v.add_sheet_index_box(&mut p).unwrap();
        let b = v.selected_box().unwrap().clone();
        assert_eq!(b.id, id);
        let layout = v.layout().unwrap().clone();
        let mut cache = BoxCache::default();
        let mut rcx = None;
        let art = box_art(&mut cache, &mut rcx, &p, 1, &b, &layout);
        assert!(
            art.texts.iter().any(|t| t.text == "ELEVATIONS"),
            "{:?}",
            art.texts
        );
        drop(rcx);
        v.rename_page(&mut p, 2, "Roof Plan");
        let layout = v.layout().unwrap().clone();
        let mut rcx = None;
        let again = box_art(&mut cache, &mut rcx, &p, 1, &b, &layout);
        assert!(
            again.texts.iter().any(|t| t.text == "ROOF PLAN"),
            "refreshed with the page title"
        );
    }

    #[test]
    fn jpeg_pictures_load_and_a_picture_box_embeds_them() {
        let path = fixture_path("rgb444.jpg");
        let img = load_picture(&path).expect("the shared decoder reads baseline JPEG");
        assert!(img.width > 0 && img.height > 0);
        assert_eq!(img.rgba.len(), (img.width * img.height * 4) as usize);
        // Progressive JPEG reads too (the shared decoder), a file that is not a picture does not.
        assert!(load_picture(&fixture_path("progressive.jpg")).is_some());
        assert!(load_picture(&fixture_path("gray.rgb")).is_none());
        assert!(load_picture("/no/such/picture.jpg").is_none());
        let (mut v, mut p) = view_with_layout();
        v.add_image_box(&mut p, &path).unwrap();
        let pdf = print_bytes(v.layout().unwrap(), &p, None);
        let text: String = pdf.iter().map(|&b| b as char).collect();
        assert!(
            text.contains("/Subtype/Image"),
            "the JPEG is an image XObject now"
        );
    }

    fn fixture_path(name: &str) -> String {
        format!(
            "{}/src/tools/underlay/testdata/{name}",
            env!("CARGO_MANIFEST_DIR")
        )
    }
}
