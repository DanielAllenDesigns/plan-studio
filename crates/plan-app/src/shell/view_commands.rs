//! Window and view commands of the application shell: the rubber-band Zoom
//! tool, Zoom Previous, Fill Window Building Only, Reverse Plan, Rotate Plan
//! View, Tile Horizontally/Vertically, Swap Views, Next/Previous Tab and the
//! Close View family.
//!
//! The commands are `Action::Custom` ids (`view.*`); [`is_command`] tells the
//! shell which ones are ours. The state they keep between frames is a
//! [`ViewShell`] held by the application; the commands themselves are methods
//! of `PlanApp` (this module is a child of the crate root, so it can reach the
//! application's own fields).
//!
//! A plan can be shown in three kinds of views: the floor plan (its saved
//! plan views are tabs, `editor::plan_tabs`), the 3D view and the layout. The
//! tab order is every plan tab, then the 3D view and the layout when they are
//! open.

use crate::editor::{plan_tabs, selection, transform, Camera, EditorContext};
use crate::toolbar::Action;
use crate::PlanApp;
use eframe::egui::{self, Pos2, Rect};
use plan_core::geometry::Point;
use plan_core::transform::{union_box, Xform};

/// Window > Zoom: arms the rubber-band zoom (drag a rectangle to zoom to it).
pub const ZOOM_WINDOW: &str = "view.zoom_window";
/// Window > Zoom Previous: back to the previous zoom, again to come back.
pub const ZOOM_PREVIOUS: &str = "view.zoom_previous";
/// Window > Fill Window Building Only: fit the walls, not the CAD and text.
pub const FILL_BUILDING: &str = "view.fill_building";
/// Edit > Reverse Plan: mirror the whole plan left to right.
pub const REVERSE_PLAN: &str = "view.reverse_plan";
/// Window > Rotate Plan View > 90 degrees left.
pub const ROTATE_LEFT: &str = "view.rotate_left";
/// Window > Rotate Plan View > 90 degrees right.
pub const ROTATE_RIGHT: &str = "view.rotate_right";
/// Window > Rotate Plan View > back to north up.
pub const ROTATE_RESET: &str = "view.rotate_reset";
/// Tools > Rotate Plan View...: type the angle (relative to north up).
pub const ROTATE_DIALOG: &str = "view.rotate_dialog";
/// Window > Tile Horizontally (the views one above the other).
pub const TILE_HORIZONTALLY: &str = "view.tile_horizontally";
/// Window > Tile Vertically (the views side by side).
pub const TILE_VERTICALLY: &str = "view.tile_vertically";
/// Window > Tab Windows (back to one view at a time).
pub const TAB_WINDOWS: &str = "view.tab_windows";
/// Window > Swap Views (F7).
pub const SWAP_VIEWS: &str = "view.swap_views";
/// Window > Select Next Tab (Ctrl+Tab).
pub const NEXT_TAB: &str = "view.next_tab";
/// Window > Select Previous Tab (Ctrl+Shift+Tab).
pub const PREVIOUS_TAB: &str = "view.previous_tab";
/// File > Close View.
pub const CLOSE_VIEW: &str = "view.close_view";
/// File > Close All 3D Views.
pub const CLOSE_ALL_3D: &str = "view.close_all_3d";
/// File > Close All Views.
pub const CLOSE_ALL_VIEWS: &str = "view.close_all_views";

const ALL: &[&str] = &[
    ZOOM_WINDOW,
    ZOOM_PREVIOUS,
    FILL_BUILDING,
    REVERSE_PLAN,
    ROTATE_LEFT,
    ROTATE_RIGHT,
    ROTATE_RESET,
    ROTATE_DIALOG,
    TILE_HORIZONTALLY,
    TILE_VERTICALLY,
    TAB_WINDOWS,
    SWAP_VIEWS,
    NEXT_TAB,
    PREVIOUS_TAB,
    CLOSE_VIEW,
    CLOSE_ALL_3D,
    CLOSE_ALL_VIEWS,
];

/// Does the shell run command `id` here?
pub fn is_command(id: &str) -> bool {
    ALL.contains(&id)
}

/// The shortest drag (screen pixels) that the rubber-band Zoom takes as a
/// rectangle; a shorter one is a click that zooms in on the point.
const MIN_DRAG: f32 = 4.0;
/// Pixels between two tiled views.
const TILE_GAP: f32 = 4.0;

/// The kinds of view the shell shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ViewKind {
    Plan,
    ThreeD,
    Layout,
}

/// How the open views share the window.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tile {
    /// One view at a time, the others are tabs.
    #[default]
    Tabs,
    /// One above the other (Tile Horizontally).
    Horizontal,
    /// Side by side (Tile Vertically).
    Vertical,
}

/// One entry of the tab order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ViewEntry {
    Plan(String),
    ThreeD,
    Layout,
}

/// What the view commands keep between frames.
#[derive(Default)]
pub struct ViewShell {
    /// How the plan and the 3D view share the window.
    pub tile: Tile,
    /// The 3D view is first (left or top) when tiled and the plan second.
    pub swapped: bool,
    /// The rubber-band Zoom is armed: the next drag zooms to its rectangle.
    pub zoom_armed: bool,
    /// Where the rubber band started.
    zoom_from: Option<Pos2>,
    /// A 3D view has been opened and not closed.
    pub open_3d: bool,
    /// The layout has been opened and not closed.
    pub open_layout: bool,
}

// ===================================================================
// Pure helpers
// ===================================================================

/// The index after (or before) `current` in a ring of `len` entries.
pub fn cycle_index(len: usize, current: usize, forward: bool) -> usize {
    if len == 0 {
        return 0;
    }
    if forward {
        (current + 1) % len
    } else {
        (current + len - 1) % len
    }
}

/// `cam` framing the box `lo`..`hi` with a margin, keeping the rotation.
pub fn fit_camera(cam: &Camera, lo: Point, hi: Point) -> Camera {
    let mid = Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5);
    // The box as the rotated view sees it.
    let (s, c) = cam.rotation.sin_cos();
    let (hw, hh) = ((hi.x - lo.x) * 0.5, (hi.y - lo.y) * 0.5);
    let ex = hw * c.abs() + hh * s.abs();
    let ey = hw * s.abs() + hh * c.abs();
    let w = (ex * 2.0 * 1.2).max(1.0);
    let h = (ey * 2.0 * 1.2).max(1.0);
    let r = cam.rect;
    let scale = (r.width() as f64 / w).min(r.height() as f64 / h);
    Camera {
        center: mid,
        px_per_in: scale.clamp(0.05, 50.0),
        ..*cam
    }
}

/// The camera after a rubber-band drag from `a` to `b` (screen points):
/// the dragged rectangle fills the canvas. `None` for a click.
pub fn window_camera(cam: &Camera, a: Pos2, b: Pos2) -> Option<Camera> {
    let (w, h) = ((a.x - b.x).abs(), (a.y - b.y).abs());
    if w < MIN_DRAG && h < MIN_DRAG {
        return None;
    }
    let f = (cam.rect.width() / w.max(1.0)).min(cam.rect.height() / h.max(1.0));
    let mid = Pos2::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5);
    Some(Camera {
        center: cam.screen_to_world(mid),
        px_per_in: (cam.px_per_in * f as f64).clamp(0.05, 50.0),
        ..*cam
    })
}

/// The two panes of `rect` for a tiling: `(first, second)`.
pub fn split(rect: Rect, tile: Tile) -> (Rect, Rect) {
    match tile {
        Tile::Horizontal => {
            let mid = rect.center().y;
            (
                Rect::from_min_max(rect.min, egui::pos2(rect.max.x, mid - TILE_GAP * 0.5)),
                Rect::from_min_max(egui::pos2(rect.min.x, mid + TILE_GAP * 0.5), rect.max),
            )
        }
        _ => {
            let mid = rect.center().x;
            (
                Rect::from_min_max(rect.min, egui::pos2(mid - TILE_GAP * 0.5, rect.max.y)),
                Rect::from_min_max(egui::pos2(mid + TILE_GAP * 0.5, rect.min.y), rect.max),
            )
        }
    }
}

/// The box around the walls of the active floor (the building).
pub fn building_bounds(cx: &EditorContext) -> Option<(Point, Point)> {
    let points: Vec<Point> = cx
        .floor()
        .walls
        .iter()
        .flat_map(|w| w.footprint())
        .collect();
    plan_core::transform::bounds_of(points)
}

/// The box around every visible, unlocked object of the active floor.
pub fn all_bounds(cx: &EditorContext) -> Option<(Point, Point)> {
    transform::items_bounds(cx, &selection::all_selectable(cx))
}

// ===================================================================
// The commands
// ===================================================================

impl PlanApp {
    /// Notes which views are open (call before every view command and once a
    /// frame).
    pub(crate) fn sync_views(&mut self) {
        if self.view3d.active {
            self.shell_views.open_3d = true;
        }
        if crate::shell::layout_window::is_active() {
            self.shell_views.open_layout = true;
        }
        if !self.shell_views.open_3d && self.shell_views.tile != Tile::Tabs {
            self.shell_views.tile = Tile::Tabs;
        }
    }

    /// The tab order: every plan tab, then the 3D view and the layout.
    pub(crate) fn view_entries(&mut self) -> Vec<ViewEntry> {
        self.sync_views();
        let mut v: Vec<ViewEntry> = plan_tabs::with_tabs(|t| {
            t.sync(&self.cx.project);
            t.open().iter().cloned().map(ViewEntry::Plan).collect()
        });
        if v.is_empty() {
            v.push(ViewEntry::Plan(self.cx.project.active_plan_view.clone()));
        }
        if self.shell_views.open_3d {
            v.push(ViewEntry::ThreeD);
        }
        if self.shell_views.open_layout {
            v.push(ViewEntry::Layout);
        }
        v
    }

    /// The entry on show.
    pub(crate) fn current_view_entry(&self) -> ViewEntry {
        if crate::shell::layout_window::is_active() {
            ViewEntry::Layout
        } else if self.view3d.active {
            ViewEntry::ThreeD
        } else {
            ViewEntry::Plan(self.cx.project.active_plan_view.clone())
        }
    }

    /// Shows `entry`.
    pub(crate) fn show_view_entry(&mut self, entry: &ViewEntry) {
        match entry {
            ViewEntry::Plan(name) => {
                crate::shell::layout_window::deactivate();
                self.view3d.active = false;
                plan_tabs::with_tabs(|t| t.switch_to(&mut self.cx, name));
            }
            ViewEntry::ThreeD => {
                crate::shell::layout_window::deactivate();
                self.view3d.active = true;
            }
            ViewEntry::Layout => {
                self.view3d.active = false;
                self.apply(Action::Layout(
                    crate::shell::layout_window::LayoutCommand::ShowLayout,
                ));
            }
        }
    }

    /// Select Next Tab / Select Previous Tab.
    pub(crate) fn cycle_view(&mut self, forward: bool) {
        let entries = self.view_entries();
        if entries.len() < 2 {
            self.cx.status = "There is only one view open".into();
            return;
        }
        let cur = self.current_view_entry();
        let at = entries.iter().position(|e| *e == cur).unwrap_or(0);
        let next = entries[cycle_index(entries.len(), at, forward)].clone();
        self.show_view_entry(&next);
        self.cx.status = match &next {
            ViewEntry::Plan(n) => format!("Plan view: {n}"),
            ViewEntry::ThreeD => "3D view".into(),
            ViewEntry::Layout => "Layout".into(),
        };
    }

    /// Tile Horizontally / Tile Vertically / Tab Windows.
    fn set_tile(&mut self, tile: Tile) {
        self.sync_views();
        if tile != Tile::Tabs && !self.shell_views.open_3d {
            self.cx.status = "Open a 3D view to tile it with the plan".into();
            return;
        }
        self.shell_views.tile = tile;
        self.cx.status = match tile {
            Tile::Tabs => "Views as tabs".into(),
            Tile::Horizontal => "Views tiled one above the other".into(),
            Tile::Vertical => "Views tiled side by side".into(),
        };
    }

    /// Swap Views (F7): the two tiled views trade places; with tabs, the
    /// plan and the 3D view trade the window.
    fn swap_views(&mut self) {
        self.sync_views();
        if !self.shell_views.open_3d {
            self.cx.status = "Open a 3D view to swap views".into();
            return;
        }
        if self.shell_views.tile == Tile::Tabs {
            let to = if self.view3d.active {
                ViewEntry::Plan(self.cx.project.active_plan_view.clone())
            } else {
                ViewEntry::ThreeD
            };
            self.show_view_entry(&to);
        } else {
            self.shell_views.swapped = !self.shell_views.swapped;
        }
        self.cx.status = "Swapped the views".into();
    }

    /// File > Close View: the view on show closes. The last plan tab stays.
    fn close_view(&mut self) {
        self.sync_views();
        match self.current_view_entry() {
            ViewEntry::Layout => {
                self.shell_views.open_layout = false;
                self.apply(Action::Layout(
                    crate::shell::layout_window::LayoutCommand::ShowPlan,
                ));
            }
            ViewEntry::ThreeD => {
                self.view3d.active = false;
                self.shell_views.open_3d = false;
                self.shell_views.tile = Tile::Tabs;
            }
            ViewEntry::Plan(name) => {
                let closed = plan_tabs::with_tabs(|t| t.close(&mut self.cx, &name));
                if !closed {
                    self.cx.status = "The last plan view stays open".into();
                }
            }
        }
    }

    /// File > Close All 3D Views.
    fn close_all_3d(&mut self) {
        self.view3d.active = false;
        self.shell_views.open_3d = false;
        self.shell_views.tile = Tile::Tabs;
        self.cx.status = "Closed the 3D views".into();
    }

    /// File > Close All Views: back to one plan view (the last plan tab stays).
    fn close_all_views(&mut self) {
        self.close_all_3d();
        if crate::shell::layout_window::is_active() {
            self.apply(Action::Layout(
                crate::shell::layout_window::LayoutCommand::ShowPlan,
            ));
        }
        self.shell_views.open_layout = false;
        let active = self.cx.project.active_plan_view.clone();
        plan_tabs::with_tabs(|t| {
            t.sync(&self.cx.project);
            for name in t.open().to_vec() {
                if name != active {
                    t.close(&mut self.cx, &name);
                }
            }
        });
        self.cx.status = "Closed all views but the plan".into();
    }

    /// Fits the plan to the camera: the building only (walls), or with
    /// everything else on the floor.
    pub(crate) fn fit_to(&mut self, bounds: Option<(Point, Point)>) {
        self.push_zoom_history();
        match bounds {
            Some((lo, hi)) => self.camera = fit_camera(&self.camera, lo, hi),
            None => {
                let (rect, rotation) = (self.camera.rect, self.camera.rotation);
                self.camera = Camera::default_view();
                self.camera.rect = rect;
                self.camera.rotation = rotation;
            }
        }
    }

    /// Zoom Previous: the zoom before this one; run again it comes back.
    fn zoom_previous(&mut self) {
        let Some(prev) = self.zoom_history.pop() else {
            self.cx.status = "No earlier zoom".into();
            return;
        };
        let now = self.camera;
        let rect = now.rect;
        self.camera = prev;
        self.camera.rect = rect;
        self.zoom_history.push(now);
    }

    /// Rotate Plan View: only the view turns.
    fn rotate_view(&mut self, degrees: Option<f64>) {
        self.push_zoom_history();
        self.camera.rotation = match degrees {
            Some(d) => (self.camera.rotation + d.to_radians()).rem_euclid(std::f64::consts::TAU),
            None => 0.0,
        };
        let deg = self.camera.rotation.to_degrees().round();
        self.cx.status = format!("Plan view rotated {deg} degrees");
    }

    /// Tools > Reverse Plan: every object of every floor is mirrored left to
    /// right about the vertical line through the middle of the walls, as one
    /// undo step. Objects on hidden and locked layers turn with the rest
    /// (manual p. 179: the whole plan is flipped, nothing is left behind).
    /// Text keeps reading left to right; swings, hinges and the stair turns
    /// flip with their objects. Terrain is not touched.
    pub(crate) fn reverse_plan(&mut self) {
        let cx = &mut self.cx;
        let keep = cx.floor;
        // Hidden and locked layers count: show and unlock everything while
        // the objects are gathered and turned, then put the layers back.
        let saved_layers = (cx.project.layers.clone(), cx.project.layer_sets.clone());
        open_all_layers(cx);
        let mut union: Option<(Point, Point)> = None;
        let mut per_floor = Vec::new();
        for f in 0..cx.project.floors.len() {
            cx.floor = f;
            let items = selection::all_selectable(cx);
            // The line runs through the middle of the walls of every floor, so
            // reversing twice is the identity whatever else is drawn.
            let walls = building_bounds(cx);
            if let Some(b) = walls.or_else(|| transform::items_bounds(cx, &items)) {
                union = Some(union.map_or(b, |u| union_box(u, b)));
            }
            per_floor.push(items);
        }
        cx.project.layers = saved_layers.0.clone();
        cx.project.layer_sets = saved_layers.1.clone();
        cx.refresh_layer_view();
        let Some((lo, hi)) = union else {
            cx.floor = keep;
            cx.status = "Nothing to reverse".into();
            return;
        };
        let mid = (lo.x + hi.x) * 0.5;
        let x = Xform::reflect(Point::new(mid, 0.0), Point::new(mid, 1.0));
        cx.begin_change("Reverse Plan");
        open_all_layers(cx);
        let mut skipped: Vec<&'static str> = Vec::new();
        let mut changed = 0;
        for (f, items) in per_floor.iter().enumerate() {
            if items.is_empty() {
                continue;
            }
            cx.floor = f;
            let rep = transform::apply_xform(cx, items, &x);
            changed += rep.changed;
            for s in rep.skipped {
                if !skipped.contains(&s) {
                    skipped.push(s);
                }
            }
        }
        cx.project.layers = saved_layers.0;
        cx.project.layer_sets = saved_layers.1;
        cx.refresh_layer_view();
        cx.floor = keep;
        cx.selection.items.clear();
        cx.mark_dirty();
        cx.status = if skipped.is_empty() {
            format!("Reversed the plan ({changed} objects)")
        } else {
            format!(
                "Reversed the plan ({changed} objects); left alone: {}",
                skipped.join(", ")
            )
        };
        let b = building_bounds(&self.cx);
        self.fit_to(b);
    }

    /// Runs one `view.*` command.
    pub(crate) fn view_command(&mut self, id: &str) {
        self.sync_views();
        match id {
            ZOOM_WINDOW => {
                self.shell_views.zoom_armed = !self.shell_views.zoom_armed;
                self.shell_views.zoom_from = None;
                self.cx.status = if self.shell_views.zoom_armed {
                    "Zoom: drag a rectangle to zoom to it".into()
                } else {
                    "Zoom off".into()
                };
            }
            ZOOM_PREVIOUS => self.zoom_previous(),
            FILL_BUILDING => {
                let b = building_bounds(&self.cx);
                self.fit_to(b);
            }
            REVERSE_PLAN => self.reverse_plan(),
            ROTATE_LEFT => self.rotate_view(Some(90.0)),
            ROTATE_RIGHT => self.rotate_view(Some(-90.0)),
            ROTATE_RESET => self.rotate_view(None),
            ROTATE_DIALOG => open_rotate_dialog(self.camera.rotation.to_degrees()),
            TILE_HORIZONTALLY => self.set_tile(Tile::Horizontal),
            TILE_VERTICALLY => self.set_tile(Tile::Vertical),
            TAB_WINDOWS => self.set_tile(Tile::Tabs),
            SWAP_VIEWS => self.swap_views(),
            NEXT_TAB => self.cycle_view(true),
            PREVIOUS_TAB => self.cycle_view(false),
            CLOSE_VIEW => self.close_view(),
            CLOSE_ALL_3D => self.close_all_3d(),
            CLOSE_ALL_VIEWS => self.close_all_views(),
            _ => {}
        }
    }

    /// The rubber band ended: zoom to the rectangle from `a` to `b` (screen
    /// points), or in on the point for a click.
    pub(crate) fn zoom_window_drag(&mut self, a: Pos2, b: Pos2) {
        self.shell_views.zoom_armed = false;
        self.shell_views.zoom_from = None;
        self.push_zoom_history();
        match window_camera(&self.camera, a, b) {
            Some(cam) => self.camera = cam,
            None => self.camera.zoom_about(b, 2.0),
        }
    }

    /// While the Zoom is armed, takes the pointer over the canvas: draws the
    /// rubber band and zooms when it is released. Returns whether the canvas
    /// belongs to the Zoom this frame (the tools see nothing).
    pub(crate) fn zoom_window_input(
        &mut self,
        ctx: &egui::Context,
        resp: &egui::Response,
        painter: &egui::Painter,
    ) -> bool {
        if !self.shell_views.zoom_armed {
            return false;
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.shell_views.zoom_armed = false;
            self.shell_views.zoom_from = None;
            self.cx.status = "Zoom off".into();
            return true;
        }
        if resp.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::ZoomIn);
        }
        let pos = ctx.input(|i| i.pointer.latest_pos());
        if self.shell_views.zoom_from.is_none()
            && resp.hovered()
            && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
        {
            self.shell_views.zoom_from = pos;
        }
        if let (Some(a), Some(b)) = (self.shell_views.zoom_from, pos) {
            let band = Rect::from_two_pos(a, b);
            painter.rect_stroke(
                band,
                0.0,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(0x4A, 0x9E, 0xFF)),
                egui::StrokeKind::Inside,
            );
            painter.rect_filled(
                band,
                0.0,
                egui::Color32::from_rgba_unmultiplied(0x4A, 0x9E, 0xFF, 30),
            );
            if ctx.input(|i| i.pointer.button_released(egui::PointerButton::Primary)) {
                self.zoom_window_drag(a, b);
                ctx.request_repaint();
            }
        }
        true
    }

    /// Draws the plan and the 3D view in two panes when the views are tiled.
    /// Returns false when they are not (the caller draws the one view).
    pub(crate) fn tiled_central(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) -> bool {
        self.sync_views();
        let tile = self.shell_views.tile;
        if tile == Tile::Tabs || crate::shell::layout_window::is_active() {
            return false;
        }
        let (first, second) = split(ui.max_rect(), tile);
        let (plan_rect, td_rect) = if self.shell_views.swapped {
            (second, first)
        } else {
            (first, second)
        };
        // The pane that is pressed has the keyboard and the toolbars.
        let pressed = ctx.input(|i| {
            i.pointer
                .button_pressed(egui::PointerButton::Primary)
                .then(|| i.pointer.latest_pos())
                .flatten()
        });
        if let Some(p) = pressed {
            if td_rect.contains(p) {
                self.view3d.active = true;
            } else if plan_rect.contains(p) {
                self.view3d.active = false;
            }
        }
        self.view3d.frame(ctx, &mut self.cx);
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(plan_rect), |ui| {
            self.canvas(ctx, ui);
        });
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(td_rect), |ui| {
            self.view3d.select_tool = self.tools.active_id().base() == crate::tools::ToolId::Select;
            crate::shell::view3d_panel::show(ui, &mut self.cx, &mut self.view3d);
        });
        true
    }
}

// ===================================================================
// Rotate Plan View (manual p. 178)
// ===================================================================

/// Shows and unlocks every layer in the plan (and in every layer set) so an
/// operation reaches objects that the layer settings keep out of reach.
fn open_all_layers(cx: &mut EditorContext) {
    for l in &mut cx.project.layers.layers {
        l.display = true;
        l.locked = false;
    }
    for set in &mut cx.project.layer_sets.sets {
        for st in &mut set.states {
            st.display = true;
            st.locked = false;
        }
    }
    cx.refresh_layer_view();
}

thread_local! {
    /// The rotation the shell last reported, radians counterclockwise.
    static REPORTED: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    /// A rotation to show next frame (radians), asked for by a view that was
    /// opened or by the Rotate Plan View dialog.
    static PENDING: std::cell::Cell<Option<f64>> = const { std::cell::Cell::new(None) };
    /// The Rotate Plan View dialog: the text being typed.
    static DIALOG: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The shell reports the camera's rotation each frame (radians), so a saved
/// plan view can store it.
pub fn report_rotation(radians: f64) {
    REPORTED.with(|r| r.set(radians));
}

/// The rotation of the plan view being shown, in degrees from -180 to 180.
pub fn reported_rotation_deg() -> f64 {
    plan_core::defaults::views::normalize_deg(REPORTED.with(|r| r.get()).to_degrees())
}

/// Asks the shell to show the plan rotated to `deg` (relative to north up).
pub fn request_rotation(deg: f64) {
    let rad = plan_core::defaults::views::normalize_deg(deg)
        .to_radians()
        .rem_euclid(std::f64::consts::TAU);
    PENDING.with(|p| p.set(Some(rad)));
    REPORTED.with(|r| r.set(rad));
}

/// The rotation the shell should adopt now, if one was asked for.
pub fn take_pending_rotation() -> Option<f64> {
    PENDING.with(|p| p.take())
}

/// Opens the Rotate Plan View dialog showing `current_deg` (it reads in
/// -180 to 180 degrees: 270 shows as -90).
pub fn open_rotate_dialog(current_deg: f64) {
    let shown = plan_core::defaults::views::normalize_deg(current_deg);
    DIALOG.with(|d| *d.borrow_mut() = Some(format!("{shown}")));
}

/// Is the Rotate Plan View dialog open?
pub fn rotate_dialog_open() -> bool {
    DIALOG.with(|d| d.borrow().is_some())
}

/// The angle a typed text means, if it is a number.
pub fn parse_angle(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches(['\u{b0}', 'd']).trim();
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Draws the Rotate Plan View dialog when it is open. OK rotates the view to
/// the typed angle (not by it: 90 twice is still 90) and, for a saved view
/// that remembers its rotation, keeps it with the view.
pub fn show_rotate_dialog(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut text) = DIALOG.with(|d| d.borrow_mut().take()) else {
        return;
    };
    let mut open = true;
    let mut ok = false;
    let mut cancel = false;
    let angle = parse_angle(&text);
    egui::Window::new("Rotate Plan View")
        .id(egui::Id::new("rotate_plan_view"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Rotation Angle");
                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(80.0));
                ui.label("degrees");
                if r.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && angle.is_some()
                {
                    ok = true;
                }
            });
            ui.weak("Rotates the view relative to north up; the plan itself does not turn.");
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(angle.is_some(), egui::Button::new("   OK   "))
                    .clicked()
                {
                    ok = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        cancel = true;
    }
    if ok {
        if let Some(a) = angle {
            request_rotation(a);
            cx.status = format!(
                "Plan view rotated to {} degrees",
                plan_core::defaults::views::normalize_deg(a)
            );
        }
    } else if open && !cancel {
        DIALOG.with(|d| *d.borrow_mut() = Some(text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam() -> Camera {
        let mut c = Camera::default_view();
        c.rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0));
        c
    }

    #[test]
    fn the_tab_ring_wraps_both_ways() {
        assert_eq!(cycle_index(3, 2, true), 0);
        assert_eq!(cycle_index(3, 0, false), 2);
        assert_eq!(cycle_index(1, 0, true), 0);
        assert_eq!(cycle_index(0, 0, true), 0);
    }

    #[test]
    fn a_dragged_rectangle_fills_the_canvas() {
        let c = cam();
        let a = Pos2::new(100.0, 100.0);
        let b = Pos2::new(300.0, 250.0);
        let z = window_camera(&c, a, b).unwrap();
        // 200 x 150 px grows to 800 x 600: four times the zoom.
        assert!((z.px_per_in / c.px_per_in - 4.0).abs() < 1e-6);
        let mid = c.screen_to_world(Pos2::new(200.0, 175.0));
        assert!((z.center.x - mid.x).abs() < 1e-6 && (z.center.y - mid.y).abs() < 1e-6);
        // A click is not a rectangle.
        assert!(window_camera(&c, a, Pos2::new(101.0, 102.0)).is_none());
    }

    #[test]
    fn fitting_a_box_leaves_a_margin_and_keeps_the_rotation() {
        let mut c = cam();
        let f = fit_camera(&c, Point::new(0.0, 0.0), Point::new(400.0, 300.0));
        assert!((f.center.x - 200.0).abs() < 1e-9);
        // 480 x 360 with the margin fills 800 x 600 exactly.
        assert!((f.px_per_in - 800.0 / 480.0).abs() < 1e-9);
        c.rotation = std::f64::consts::FRAC_PI_2;
        let r = fit_camera(&c, Point::new(0.0, 0.0), Point::new(400.0, 300.0));
        assert!((r.rotation - c.rotation).abs() < 1e-12);
        // Turned a quarter the box is 300 wide and 400 tall on screen.
        assert!((r.px_per_in - 600.0 / 480.0).abs() < 1e-9);
    }

    #[test]
    fn tiles_split_the_window_in_two_panes() {
        let r = Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 600.0));
        let (a, b) = split(r, Tile::Vertical);
        assert!(a.right() < b.left() && a.height() == 600.0);
        let (a, b) = split(r, Tile::Horizontal);
        assert!(a.bottom() < b.top() && a.width() == 1000.0);
    }

    #[test]
    fn the_view_ids_are_ours() {
        assert!(is_command(NEXT_TAB));
        assert!(!is_command("edit.select_all"));
        assert!(!is_command("view.nothing"));
    }
}
