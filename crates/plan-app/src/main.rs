//! Plan Studio: a 2D floor-plan editor on top of `plan-core`.
//!
//! World units are inches with Y up; the camera flips Y when mapping to screen.

mod dialogs;
mod icons;
mod menus;
mod plan_defaults;
mod theme;
mod toolbar;

use dialogs::{
    DefaultsDialog, DefaultsEntry, DefaultsOutcome, OpeningDialog, OpeningExtras, OpeningTarget,
    Outcome, WallDialog, WallExtras, WallTarget,
};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use plan_core::geometry::{dist_to_segment, project_on_segment, Point};
use plan_core::{
    detect_rooms, Floor, Id, Opening, OpeningKind, PlanDefaults, Project, Room, Wall, WallKind,
    DEFAULT_WALL_LAYER,
};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use theme::{AppSettings, CanvasTheme, Palette};
use toolbar::{Action, BarState, Dock, Hotkeys, Tool, Toolbars, ViewFlag};

const PICK_RADIUS_PX: f64 = 10.0;
const STATUS_GRAY: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x2C);
const ZOOM_STEP: f64 = 1.25;
const ZOOM_HISTORY_CAP: usize = 50;
const VIEW_BAR_WIDTH: f32 = 36.0;
const DOCK_WIDTH: f32 = 300.0;

/// Maps between world inches (Y up) and screen pixels (Y down).
#[derive(Clone, Copy)]
struct Camera {
    /// World point shown at the viewport center.
    center: Point,
    px_per_in: f64,
}

impl Camera {
    fn default_view() -> Self {
        Self {
            center: Point::new(240.0, 150.0),
            px_per_in: 2.0,
        }
    }

    fn world_to_screen(&self, rect: Rect, p: Point) -> Pos2 {
        let c = rect.center();
        Pos2::new(
            c.x + ((p.x - self.center.x) * self.px_per_in) as f32,
            c.y - ((p.y - self.center.y) * self.px_per_in) as f32,
        )
    }

    fn screen_to_world(&self, rect: Rect, s: Pos2) -> Point {
        let c = rect.center();
        Point::new(
            self.center.x + (s.x - c.x) as f64 / self.px_per_in,
            self.center.y - (s.y - c.y) as f64 / self.px_per_in,
        )
    }

    /// Zoom by `factor` keeping the world point under screen position `at` fixed.
    fn zoom_about(&mut self, rect: Rect, at: Pos2, factor: f64) {
        let anchor = self.screen_to_world(rect, at);
        self.px_per_in = (self.px_per_in * factor).clamp(0.05, 50.0);
        let c = rect.center();
        self.center = Point::new(
            anchor.x - (at.x - c.x) as f64 / self.px_per_in,
            anchor.y + (at.y - c.y) as f64 / self.px_per_in,
        );
    }

    fn pan_by_pixels(&mut self, d: Vec2) {
        self.center.x -= d.x as f64 / self.px_per_in;
        self.center.y += d.y as f64 / self.px_per_in;
    }
}

/// The specification dialog that is open, if any (one at a time).
enum ActiveDialog {
    Wall(Box<WallDialog>),
    Opening(Box<OpeningDialog>),
}

/// Settings the model has no fields for yet, kept for the session only.
#[derive(Default)]
struct SessionExtras {
    walls: HashMap<Id, WallExtras>,
    openings: HashMap<Id, OpeningExtras>,
}

struct PlanApp {
    project: Project,
    floor: usize,
    path: Option<PathBuf>,
    camera: Camera,
    zoom_history: Vec<Camera>,
    canvas_rect: Rect,
    tool: Tool,
    toolbars: Toolbars,
    flags: HashSet<ViewFlag>,
    dock: Option<Dock>,
    hotkeys: Hotkeys,
    settings: AppSettings,
    /// The settings last written to disk.
    saved_settings: AppSettings,
    /// The brightness the egui visuals were last built for.
    applied_brightness: f32,
    show_about: bool,
    /// Wall types, wall/door/window defaults, grid, dimension format, ...
    /// (see `plan_defaults.rs` for where they are loaded from).
    defaults: PlanDefaults,
    selected: Option<Id>,
    pending_start: Option<Point>,
    rooms: Vec<Room>,
    rooms_dirty: bool,
    cursor_world: Option<Point>,
    snapped: Option<Point>,
    hover_wall: Option<Id>,
    message: String,
    dialog: Option<ActiveDialog>,
    defaults_dialog: Option<DefaultsDialog>,
    extras: SessionExtras,
}

impl PlanApp {
    fn new(settings: AppSettings, defaults: PlanDefaults, note: Option<String>) -> Self {
        Self {
            project: Project::from_defaults("Untitled", &defaults),
            floor: 0,
            path: None,
            camera: Camera::default_view(),
            zoom_history: Vec::new(),
            canvas_rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
            tool: Tool::Select,
            toolbars: Toolbars::new(),
            flags: HashSet::from([ViewFlag::TemporaryDimensions, ViewFlag::ReferenceGrid]),
            dock: None,
            hotkeys: Hotkeys::default(),
            settings,
            saved_settings: settings,
            applied_brightness: settings.brightness,
            show_about: false,
            defaults,
            selected: None,
            pending_start: None,
            rooms: Vec::new(),
            rooms_dirty: true,
            cursor_world: None,
            snapped: None,
            hover_wall: None,
            message: note.unwrap_or_default(),
            dialog: None,
            defaults_dialog: None,
            extras: SessionExtras::default(),
        }
    }

    fn floor(&self) -> &Floor {
        &self.project.floors[self.floor]
    }

    fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            // Switching wall flavors keeps a chain in progress.
            let keep_chain = matches!((self.tool, tool), (Tool::Wall { .. }, Tool::Wall { .. }));
            self.tool = tool;
            if !keep_chain {
                self.pending_start = None;
            }
            self.hover_wall = None;
        }
    }

    fn wall_kind(&self) -> WallKind {
        match self.tool {
            Tool::Wall { kind } => kind,
            _ => WallKind::Exterior,
        }
    }

    fn push_zoom_history(&mut self) {
        self.zoom_history.push(self.camera);
        if self.zoom_history.len() > ZOOM_HISTORY_CAP {
            self.zoom_history.remove(0);
        }
    }

    fn zoom_about_center(&mut self, factor: f64) {
        self.push_zoom_history();
        self.camera.px_per_in = (self.camera.px_per_in * factor).clamp(0.05, 50.0);
    }

    fn fill_window(&mut self) {
        self.push_zoom_history();
        let points: Vec<Point> = self
            .floor()
            .walls
            .iter()
            .flat_map(|w| w.footprint())
            .collect();
        if points.is_empty() {
            self.camera = Camera::default_view();
            return;
        }
        let (mut lo, mut hi) = (points[0], points[0]);
        for p in &points {
            lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        // 10% margin on every side.
        let w = ((hi.x - lo.x) * 1.2).max(1.0);
        let h = ((hi.y - lo.y) * 1.2).max(1.0);
        let r = self.canvas_rect;
        let scale = (r.width() as f64 / w).min(r.height() as f64 / h);
        self.camera = Camera {
            center: Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5),
            px_per_in: scale.clamp(0.05, 50.0),
        };
    }

    fn change_floor(&mut self, up: bool) {
        let next = if up {
            (self.floor + 1 < self.project.floors.len()).then_some(self.floor + 1)
        } else {
            self.floor.checked_sub(1)
        };
        if let Some(n) = next {
            self.floor = n;
            self.reset_view_state();
        }
    }

    fn apply(&mut self, action: Action) {
        match action {
            Action::SetTool(t) => self.set_tool(t),
            Action::CurrentWall => self.apply(self.toolbars.wall_action()),
            Action::FileNew => self.new_project(),
            Action::FileOpen => self.open_project(),
            Action::FileSave => self.save_project(),
            Action::FileSaveAs => self.save_project_as(),
            Action::SaveTemplate => self.save_template(),
            Action::ResetTemplate => self.reset_template(),
            Action::ZoomIn => self.zoom_about_center(ZOOM_STEP),
            Action::ZoomOut => self.zoom_about_center(1.0 / ZOOM_STEP),
            Action::UndoZoom => {
                if let Some(cam) = self.zoom_history.pop() {
                    self.camera = cam;
                }
            }
            Action::FillWindow => self.fill_window(),
            Action::FloorUp => self.change_floor(true),
            Action::FloorDown => self.change_floor(false),
            Action::TogglePan => {
                let next = if self.tool == Tool::Pan {
                    Tool::Select
                } else {
                    Tool::Pan
                };
                self.set_tool(next);
            }
            Action::ToggleFlag(f) => {
                if !self.flags.remove(&f) {
                    self.flags.insert(f);
                }
            }
            Action::ToggleDock(d) => {
                self.dock = if self.dock == Some(d) { None } else { Some(d) };
            }
            // The window close request needs the egui context; see `update`.
            Action::Quit => {}
            Action::SetTheme(t) => self.settings.theme = t,
            Action::ShowAbout => self.show_about = true,
            Action::DefaultSettings => {
                if self.defaults_dialog.is_none() {
                    self.defaults_dialog = Some(DefaultsDialog::new());
                }
            }
            Action::NotImplemented(name) => {
                self.message = format!("Not yet implemented: {name}");
            }
        }
    }

    fn tool_hint(&self) -> String {
        match self.tool {
            Tool::Select => "Select: click a wall to select it; Delete removes it".into(),
            Tool::Wall { .. } => {
                "Wall: click to place points; Alt disables angle snap; Esc/right-click ends".into()
            }
            Tool::Door => "Door: click on a wall to place a door".into(),
            Tool::Window => "Window: click on a wall to place a window".into(),
            Tool::Pan => "Pan: drag to move the view; Esc or Select returns".into(),
            Tool::Unimplemented(name) => format!("{name}: not yet implemented"),
        }
    }

    fn reset_view_state(&mut self) {
        self.selected = None;
        self.pending_start = None;
        self.hover_wall = None;
        self.rooms_dirty = true;
    }

    // ----- file operations -----

    fn new_project(&mut self) {
        self.project = Project::from_defaults("Untitled", &self.defaults);
        self.path = None;
        self.reset_view_state();
        self.message = "New project".into();
    }

    fn open_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &["psplan"])
            .pick_file()
        else {
            return;
        };
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| Project::from_json(&s).map_err(|e| e.to_string()))
        {
            Ok(p) if !p.floors.is_empty() => {
                self.project = p;
                self.floor = 0;
                self.message = format!("Opened {}", path.display());
                self.path = Some(path);
                self.reset_view_state();
            }
            Ok(_) => self.message = "File contains no floors".into(),
            Err(e) => self.message = format!("Open failed: {e}"),
        }
    }

    fn save_project(&mut self) {
        match self.path.clone() {
            Some(p) => self.write_to(p),
            None => self.save_project_as(),
        }
    }

    fn save_project_as(&mut self) {
        let Some(mut path) = rfd::FileDialog::new()
            .add_filter("Plan Studio", &["psplan"])
            .set_file_name("plan.psplan")
            .save_file()
        else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("psplan");
        }
        self.write_to(path);
    }

    fn write_to(&mut self, path: PathBuf) {
        let result = self
            .project
            .to_json()
            .map_err(|e| e.to_string())
            .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.message = format!("Saved {}", path.display());
                self.path = Some(path);
            }
            Err(e) => self.message = format!("Save failed: {e}"),
        }
    }

    /// File > Templates > Save Current Defaults as My Template.
    fn save_template(&mut self) {
        self.message = match plan_defaults::save_user(&self.defaults) {
            Ok(path) => format!("Saved your template to {}", path.display()),
            Err(e) => format!("Could not save the template: {e}"),
        };
    }

    /// File > Templates > Reset to Chief X18 Template.
    fn reset_template(&mut self) {
        self.defaults = plan_defaults::embedded();
        // Forget the per-session edits of the default dialogs so they show
        // the template's values again.
        for key in [
            WallTarget::DefaultExterior.key(),
            WallTarget::DefaultInterior.key(),
            WallTarget::DefaultFoundation.key(),
        ] {
            self.extras.walls.remove(&key);
        }
        for target in [
            OpeningTarget::DefaultDoor,
            OpeningTarget::DefaultExteriorDoor,
            OpeningTarget::DefaultWindow,
        ] {
            self.extras.openings.remove(&target.key());
        }
        self.message = match plan_defaults::clear_user() {
            Ok(()) => "Reset to the Chief X18 template".into(),
            Err(e) => format!("Reset, but could not remove your saved template: {e}"),
        };
    }

    // ----- model edits -----

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.project.remove_wall(self.floor, id);
            self.rooms_dirty = true;
        }
    }

    fn current_thickness(&self) -> f64 {
        match self.wall_kind() {
            WallKind::Exterior => self.defaults.exterior_thickness(),
            WallKind::Interior => self.defaults.interior_thickness(),
        }
    }

    fn default_height(&self, kind: WallKind) -> f64 {
        self.defaults.walls_for(kind).height
    }

    /// Dimension text in the active dimension defaults' format.
    fn fmt_dim(&self, inches: f64) -> String {
        self.defaults.dim_format().fmt_len(inches)
    }

    /// Nearest wall whose centerline is within the pick radius of `p`.
    fn nearest_wall(&self, p: Point) -> Option<Id> {
        let max = PICK_RADIUS_PX / self.camera.px_per_in;
        self.floor()
            .walls
            .iter()
            .map(|w| (w.id, dist_to_segment(p, w.start, w.end)))
            .filter(|(_, d)| *d <= max)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    /// Wall-tool snapping: existing endpoints first, then the grid, then 15 degree angles.
    fn snap_wall_point(&self, raw: Point, alt: bool) -> Point {
        let max = PICK_RADIUS_PX / self.camera.px_per_in;
        let endpoint = self
            .floor()
            .walls
            .iter()
            .flat_map(|w| [w.start, w.end])
            .map(|e| (e, e.dist(raw)))
            .filter(|(_, d)| *d <= max)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(e, _)| e);
        if let Some(e) = endpoint {
            return e;
        }
        let snap = self.defaults.grid.snap;
        let grid = snap_to_grid(raw, snap);
        match self.pending_start {
            Some(start) if !alt => {
                angle_snap(start, raw, snap, self.defaults.grid.angle_snap_deg).unwrap_or(grid)
            }
            _ => grid,
        }
    }

    /// Applies hotkeys (see `toolbar::BINDINGS`) plus Esc and Delete.
    fn handle_keys(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if self.dialog.is_some() || self.defaults_dialog.is_some() {
            return;
        }
        actions.extend(self.hotkeys.poll(ctx));
        if ctx.wants_keyboard_input() {
            return;
        }
        let (esc, del) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            )
        });
        if esc {
            if self.pending_start.is_some() {
                self.pending_start = None;
            } else {
                self.set_tool(Tool::Select);
            }
        }
        if del {
            self.delete_selected();
        }
    }

    // ----- UI panels -----

    fn menu_bar(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let state = BarState {
                    tool: self.tool,
                    flags: &self.flags,
                    dock: self.dock,
                    floor: self.floor,
                    floor_count: self.project.floors.len(),
                    view_name: "Floor Plan View",
                    brightness: self.settings.brightness,
                };
                menus::bar(
                    ui,
                    &state,
                    self.settings.theme,
                    &mut self.settings.brightness,
                    actions,
                );
                let name = self
                    .path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned());
                ui.label(egui::RichText::new(name.unwrap_or_else(|| "(unsaved)".into())).weak());
            });
        });
    }

    fn toolbar_rows(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        for (id, second) in [("row1", false), ("row2", true)] {
            egui::TopBottomPanel::top(id).show(ctx, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt(id)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        let state = BarState {
                            tool: self.tool,
                            flags: &self.flags,
                            dock: self.dock,
                            floor: self.floor,
                            floor_count: self.project.floors.len(),
                            view_name: "Floor Plan View",
                            brightness: self.settings.brightness,
                        };
                        let slots = if second {
                            &mut self.toolbars.row2
                        } else {
                            &mut self.toolbars.row1
                        };
                        actions.extend(toolbar::row(ui, slots, &state));
                    });
            });
        }
    }

    fn view_bar(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        let frame = egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::same(4));
        egui::SidePanel::right("view_bar")
            .exact_width(VIEW_BAR_WIDTH)
            .resizable(false)
            .frame(frame)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("view_bar_scroll")
                    .auto_shrink([true, false])
                    .show(ui, |ui| {
                        let state = BarState {
                            tool: self.tool,
                            flags: &self.flags,
                            dock: self.dock,
                            floor: self.floor,
                            floor_count: self.project.floors.len(),
                            view_name: "Floor Plan View",
                            brightness: self.settings.brightness,
                        };
                        actions.extend(toolbar::column(ui, &mut self.toolbars.view, &state));
                    });
            });
    }

    fn dock_panel(&mut self, ctx: &egui::Context) {
        let Some(dock) = self.dock else { return };
        egui::SidePanel::right("dock")
            .exact_width(DOCK_WIDTH)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading(dock.title());
                ui.separator();
                ui.weak("Coming in Phase 1");
            });
    }

    fn properties_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("properties")
            .default_width(240.0)
            .show(ctx, |ui| {
                ui.heading("Properties");
                ui.separator();
                ui.label("Default walls");
                let d = &mut self.defaults;
                wall_type_combo(
                    ui,
                    "Exterior wall",
                    &mut d.exterior_wall.wall_type,
                    &d.wall_types,
                    WallKind::Exterior,
                );
                inch_drag(
                    ui,
                    "Exterior height",
                    &mut d.exterior_wall.height,
                    24.0..=240.0,
                );
                wall_type_combo(
                    ui,
                    "Interior wall",
                    &mut d.interior_wall.wall_type,
                    &d.wall_types,
                    WallKind::Interior,
                );
                inch_drag(
                    ui,
                    "Interior height",
                    &mut d.interior_wall.height,
                    24.0..=240.0,
                );
                ui.separator();
                ui.label("Grid");
                inch_drag(ui, "Grid spacing", &mut d.grid.spacing, 1.0..=240.0);
                inch_drag(ui, "Snap spacing", &mut d.grid.snap, 0.25..=48.0);
                ui.separator();
                ui.label("Display");
                ui.horizontal(|ui| {
                    ui.label("Theme");
                    egui::ComboBox::from_id_salt("canvas_theme")
                        .selected_text(self.settings.theme.label())
                        .show_ui(ui, |ui| {
                            for t in CanvasTheme::ALL {
                                ui.selectable_value(&mut self.settings.theme, t, t.label());
                            }
                        });
                });
                ui.separator();
                ui.label("Rooms");
                if self.rooms.is_empty() {
                    ui.weak("None detected (close a loop of walls)");
                }
                for room in &self.rooms {
                    ui.label(format!(
                        "{}  -  {} sq ft",
                        room.label,
                        room.area_sq_ft().round()
                    ));
                }
                ui.separator();
                self.selected_wall_section(ui);
            });
    }

    fn selected_wall_section(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected else {
            ui.weak("No wall selected");
            return;
        };
        let Some(wall) = self.floor().wall(id) else {
            self.selected = None;
            return;
        };
        let (len, kind, mut thickness) = (wall.length(), wall.kind, wall.thickness);
        let fmt = self.defaults.dim_format();
        let openings: Vec<(Id, OpeningKind, f64, f64)> = self
            .floor()
            .openings_on(id)
            .map(|o| (o.id, o.kind, o.center_offset, o.width))
            .collect();

        ui.label("Selected wall");
        ui.label(format!("Length: {}", fmt.fmt_len(len)));
        ui.label(format!(
            "Kind: {}",
            if kind == WallKind::Exterior {
                "Exterior"
            } else {
                "Interior"
            }
        ));
        if inch_drag(ui, "Thickness", &mut thickness, 1.0..=24.0) {
            let floor = self.floor;
            if let Some(w) = self.project.floors[floor].wall_mut(id) {
                w.thickness = thickness;
            }
            self.rooms_dirty = true;
        }
        let mut open_wall_spec = false;
        let mut open_opening_spec = None;
        ui.horizontal(|ui| {
            open_wall_spec = ui.button("Open Specification\u{2026}").clicked();
            if ui.button("Delete wall").clicked() {
                self.delete_selected();
            }
        });
        if open_wall_spec {
            self.open_wall_dialog(id);
        }
        if self.selected != Some(id) {
            return;
        }
        ui.label("Openings");
        if openings.is_empty() {
            ui.weak("None");
        }
        let mut remove = None;
        for (oid, okind, center, width) in openings {
            ui.horizontal(|ui| {
                let name = if okind == OpeningKind::Door {
                    "Door"
                } else {
                    "Window"
                };
                ui.label(format!(
                    "{name} {} @ {}",
                    fmt.fmt_len(width),
                    fmt.fmt_len(center)
                ));
                if ui
                    .small_button("\u{2026}")
                    .on_hover_text("Open Specification")
                    .clicked()
                {
                    open_opening_spec = Some(oid);
                }
                if ui.small_button("✕").clicked() {
                    remove = Some(oid);
                }
            });
        }
        if let Some(oid) = remove {
            self.project.remove_opening(self.floor, oid);
        }
        if let Some(oid) = open_opening_spec {
            self.open_opening_dialog(oid);
        }
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::side_top_panel(&ctx.style())
            .fill(theme::scale(STATUS_GRAY, self.settings.brightness));
        egui::TopBottomPanel::bottom("status")
            .frame(frame)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    match self.cursor_world {
                        Some(p) => ui.monospace(format!(
                            "X: {}  Y: {}",
                            self.fmt_dim(p.x),
                            self.fmt_dim(p.y)
                        )),
                        None => ui.monospace("X: --  Y: --"),
                    };
                    if let Some(prefix) = self.hotkeys.pending_label() {
                        ui.separator();
                        ui.strong(prefix);
                    }
                    if let (Some(start), Some(end)) = (self.pending_start, self.snapped) {
                        ui.separator();
                        ui.label(format!("Length: {}", self.fmt_dim(start.dist(end))));
                    }
                    ui.separator();
                    ui.label(self.tool_hint());
                    if !self.message.is_empty() {
                        ui.separator();
                        ui.weak(&self.message);
                    }
                });
            });
    }

    // ----- canvas -----

    fn canvas(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let rect = resp.rect;
        self.canvas_rect = rect;

        self.handle_camera_input(ui, &resp, rect);

        let alt = ctx.input(|i| i.modifiers.alt);
        let new_cursor = resp
            .hover_pos()
            .map(|p| self.camera.screen_to_world(rect, p));
        if new_cursor.map(|p| (p.x, p.y)) != self.cursor_world.map(|p| (p.x, p.y)) {
            ctx.request_repaint();
        }
        self.cursor_world = new_cursor;
        self.snapped = None;
        self.hover_wall = None;
        if let Some(raw) = new_cursor {
            match self.tool {
                Tool::Wall { .. } => self.snapped = Some(self.snap_wall_point(raw, alt)),
                Tool::Door | Tool::Window => self.hover_wall = self.nearest_wall(raw),
                Tool::Select | Tool::Pan | Tool::Unimplemented(_) => {}
            }
        }

        // Clicks are ignored while a specification dialog is open.
        if self.dialog.is_none() {
            if resp.double_clicked_by(egui::PointerButton::Primary) && self.tool == Tool::Select {
                if let Some(raw) = new_cursor {
                    self.handle_double_click(raw);
                }
            } else if resp.clicked_by(egui::PointerButton::Primary) {
                if let Some(raw) = new_cursor {
                    self.handle_click(raw);
                }
            }
            if resp.clicked_by(egui::PointerButton::Secondary) {
                self.pending_start = None;
            }
        }

        self.draw(&painter, rect);
    }

    fn handle_camera_input(&mut self, ui: &egui::Ui, resp: &egui::Response, rect: Rect) {
        if resp.dragged_by(egui::PointerButton::Middle)
            || resp.dragged_by(egui::PointerButton::Secondary)
            || (self.tool == Tool::Pan && resp.dragged_by(egui::PointerButton::Primary))
        {
            self.camera.pan_by_pixels(resp.drag_delta());
        }
        if let Some(pos) = resp.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll as f64 * 0.005).exp() * zoom as f64;
            if (factor - 1.0).abs() > 1e-9 {
                self.camera.zoom_about(rect, pos, factor);
            }
        }
    }

    // ----- specification dialogs -----

    /// The opening under `p`: inside its extent along the wall and the wall
    /// thickness (plus a couple of pixels of slop).
    fn hit_opening(&self, p: Point) -> Option<Id> {
        let slop = PICK_RADIUS_PX * 0.5 / self.camera.px_per_in;
        let floor = self.floor();
        for w in &floor.walls {
            let perp = p.sub(w.start).dot(w.normal()).abs();
            if perp > w.thickness * 0.5 + slop {
                continue;
            }
            let (t, _) = project_on_segment(p, w.start, w.end);
            let along = t * w.length();
            if let Some(o) = floor
                .openings_on(w.id)
                .find(|o| along >= o.start_offset() && along <= o.end_offset())
            {
                return Some(o.id);
            }
        }
        None
    }

    fn handle_double_click(&mut self, raw: Point) {
        if let Some(oid) = self.hit_opening(raw) {
            self.open_opening_dialog(oid);
        } else if let Some(wid) = self.nearest_wall(raw) {
            self.selected = Some(wid);
            self.open_wall_dialog(wid);
        }
    }

    fn open_wall_dialog(&mut self, id: Id) {
        let Some(wall) = self.floor().wall(id).cloned() else {
            return;
        };
        let openings: Vec<Opening> = self.floor().openings_on(id).cloned().collect();
        let extras = self.extras.walls.get(&id).cloned().unwrap_or_default();
        let default_height = self.default_height(wall.kind);
        self.dialog = Some(ActiveDialog::Wall(Box::new(WallDialog::new(
            WallTarget::Wall(id),
            wall,
            openings,
            extras,
            default_height,
            self.defaults.wall_types.clone(),
        ))));
    }

    fn open_opening_dialog(&mut self, id: Id) {
        let floor = self.floor();
        let Some(opening) = floor.openings.iter().find(|o| o.id == id).cloned() else {
            return;
        };
        let Some(wall) = floor.wall(opening.wall_id).cloned() else {
            return;
        };
        let others: Vec<Opening> = floor
            .openings_on(wall.id)
            .filter(|o| o.id != id)
            .cloned()
            .collect();
        self.selected = Some(wall.id);
        let extras = self.extras.openings.get(&id).cloned().unwrap_or_default();
        self.dialog = Some(ActiveDialog::Opening(Box::new(OpeningDialog::for_opening(
            opening, &wall, others, extras,
        ))));
    }

    /// The extras a default door or window dialog starts from: the ones kept
    /// this session, else the values in the plan defaults.
    fn default_opening_extras(&self, target: OpeningTarget) -> OpeningExtras {
        if let Some(e) = self.extras.openings.get(&target.key()) {
            return e.clone();
        }
        match target {
            OpeningTarget::DefaultWindow => {
                OpeningExtras::from_window_defaults(&self.defaults.window)
            }
            OpeningTarget::DefaultExteriorDoor => {
                OpeningExtras::from_door_defaults(&self.defaults.exterior_door, true)
            }
            _ => OpeningExtras::from_door_defaults(&self.defaults.interior_door, false),
        }
    }

    fn open_defaults_entry(&mut self, entry: DefaultsEntry) {
        let wall_dialog = |target: WallTarget, app: &Self| {
            let w = match target {
                WallTarget::DefaultInterior => &app.defaults.interior_wall,
                WallTarget::DefaultFoundation => &app.defaults.foundation_wall,
                _ => &app.defaults.exterior_wall,
            };
            let thickness = match target {
                WallTarget::DefaultInterior => app.defaults.interior_thickness(),
                WallTarget::DefaultFoundation => app.defaults.foundation_thickness(),
                _ => app.defaults.exterior_thickness(),
            };
            let extras = app.extras.walls.get(&target.key()).cloned();
            ActiveDialog::Wall(Box::new(WallDialog::for_default(
                target,
                thickness,
                w.height,
                extras.unwrap_or_default(),
                app.defaults.wall_types.clone(),
                &w.wall_type,
            )))
        };
        let opening_dialog = |target: OpeningTarget, template: Opening, app: &Self| {
            ActiveDialog::Opening(Box::new(OpeningDialog::for_default(
                target,
                template,
                app.default_opening_extras(target),
            )))
        };
        self.dialog = match entry {
            DefaultsEntry::ExteriorWall => Some(wall_dialog(WallTarget::DefaultExterior, self)),
            DefaultsEntry::InteriorWall => Some(wall_dialog(WallTarget::DefaultInterior, self)),
            DefaultsEntry::FoundationWall => Some(wall_dialog(WallTarget::DefaultFoundation, self)),
            DefaultsEntry::InteriorDoor => Some(opening_dialog(
                OpeningTarget::DefaultDoor,
                plan_defaults::door_template(&self.defaults, false),
                self,
            )),
            DefaultsEntry::ExteriorDoor => Some(opening_dialog(
                OpeningTarget::DefaultExteriorDoor,
                plan_defaults::door_template(&self.defaults, true),
                self,
            )),
            DefaultsEntry::Window => Some(opening_dialog(
                OpeningTarget::DefaultWindow,
                plan_defaults::window_template(&self.defaults),
                self,
            )),
            DefaultsEntry::Dimensions | DefaultsEntry::RoomTypes => {
                self.message = "Coming in a later phase".into();
                None
            }
        };
    }

    /// Shows the open dialogs and applies an OK.
    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.dialog.take() {
            let outcome = match &mut dialog {
                ActiveDialog::Wall(d) => d.show(ctx),
                ActiveDialog::Opening(d) => d.show(ctx),
            };
            match outcome {
                Outcome::Open => self.dialog = Some(dialog),
                Outcome::Cancel => {}
                Outcome::Ok => {
                    match &dialog {
                        ActiveDialog::Wall(d) => self.apply_wall_dialog(d),
                        ActiveDialog::Opening(d) => self.apply_opening_dialog(d),
                    }
                    self.rooms_dirty = true;
                }
            }
        }
        if let Some(mut defaults) = self.defaults_dialog.take() {
            match defaults.show(ctx, self.dialog.is_none()) {
                DefaultsOutcome::Open => {}
                DefaultsOutcome::Edit(entry) => self.open_defaults_entry(entry),
                DefaultsOutcome::Close => return,
            }
            self.defaults_dialog = Some(defaults);
        }
    }

    fn apply_wall_dialog(&mut self, d: &WallDialog) {
        let draft = d.draft();
        match d.target() {
            WallTarget::Wall(id) => {
                let floor = &mut self.project.floors[self.floor];
                if let Some(w) = floor.wall_mut(id) {
                    *w = draft.clone();
                }
                for adjusted in d.adjusted_openings() {
                    if let Some(o) = floor.openings.iter_mut().find(|o| o.id == adjusted.id) {
                        o.center_offset = adjusted.center_offset;
                    }
                }
            }
            WallTarget::DefaultExterior
            | WallTarget::DefaultInterior
            | WallTarget::DefaultFoundation => {
                let kind = if d.target() == WallTarget::DefaultInterior {
                    WallKind::Interior
                } else {
                    WallKind::Exterior
                };
                let name = plan_defaults::resolve_wall_type(
                    &mut self.defaults,
                    kind,
                    d.picked_type(),
                    draft.thickness,
                );
                let w = match d.target() {
                    WallTarget::DefaultInterior => &mut self.defaults.interior_wall,
                    WallTarget::DefaultFoundation => &mut self.defaults.foundation_wall,
                    _ => &mut self.defaults.exterior_wall,
                };
                w.wall_type = name;
                w.height = draft.height;
            }
        }
        self.extras
            .walls
            .insert(d.target().key(), d.extras().clone());
    }

    fn apply_opening_dialog(&mut self, d: &OpeningDialog) {
        let draft = d.draft().clone();
        match d.target() {
            OpeningTarget::Placed(id) => {
                let floor = &mut self.project.floors[self.floor];
                if let Some(o) = floor.openings.iter_mut().find(|o| o.id == id) {
                    *o = draft;
                }
            }
            OpeningTarget::DefaultDoor => {
                let base = &self.defaults.interior_door;
                self.defaults.interior_door = d.extras().to_door_defaults(&draft, base, false);
            }
            OpeningTarget::DefaultExteriorDoor => {
                let base = &self.defaults.exterior_door;
                self.defaults.exterior_door = d.extras().to_door_defaults(&draft, base, true);
            }
            OpeningTarget::DefaultWindow => {
                let base = &self.defaults.window;
                self.defaults.window = d.extras().to_window_defaults(&draft, base);
            }
        }
        self.extras
            .openings
            .insert(d.target().key(), d.extras().clone());
    }

    fn handle_click(&mut self, raw: Point) {
        match self.tool {
            Tool::Select => self.selected = self.nearest_wall(raw),
            Tool::Pan | Tool::Unimplemented(_) => {}
            Tool::Wall { kind } => {
                let Some(p) = self.snapped else { return };
                match self.pending_start {
                    None => self.pending_start = Some(p),
                    Some(start) if start.dist(p) >= 1.0 => {
                        let id = self.project.add_wall(
                            self.floor,
                            start,
                            p,
                            self.current_thickness(),
                            self.default_height(kind),
                            kind,
                        );
                        self.selected = Some(id);
                        self.pending_start = Some(p);
                        self.rooms_dirty = true;
                    }
                    Some(_) => {}
                }
            }
            Tool::Door | Tool::Window => {
                let Some(wid) = self.nearest_wall(raw) else {
                    return;
                };
                let Some(wall) = self.floor().wall(wid) else {
                    return;
                };
                let (t, _) = project_on_segment(raw, wall.start, wall.end);
                let offset = t * wall.length();
                let kind = if self.tool == Tool::Door {
                    OpeningKind::Door
                } else {
                    OpeningKind::Window
                };
                // Doors in exterior walls use the exterior door defaults.
                let (target, template) = if kind == OpeningKind::Window {
                    (
                        OpeningTarget::DefaultWindow,
                        plan_defaults::window_template(&self.defaults),
                    )
                } else if wall.kind == WallKind::Exterior {
                    (
                        OpeningTarget::DefaultExteriorDoor,
                        plan_defaults::door_template(&self.defaults, true),
                    )
                } else {
                    (
                        OpeningTarget::DefaultDoor,
                        plan_defaults::door_template(&self.defaults, false),
                    )
                };
                let extras = self.default_opening_extras(target);
                match dialogs::place_from_template(
                    &mut self.project,
                    self.floor,
                    wid,
                    offset,
                    &template,
                ) {
                    Some(id) => {
                        self.extras.openings.insert(id, extras);
                        self.selected = Some(wid);
                        self.message.clear();
                    }
                    None => {
                        self.message =
                            "Opening does not fit there (wall too short or overlap)".into()
                    }
                }
            }
        }
    }

    fn draw(&self, painter: &egui::Painter, rect: Rect) {
        let pal = self.settings.theme.palette();
        painter.rect_filled(rect, 0.0, pal.background);
        self.draw_grid(painter, rect, &pal);
        self.draw_rooms(painter, rect, &pal);
        let floor = self.floor();
        for wall in &floor.walls {
            self.draw_wall(painter, rect, wall, &pal);
        }
        for wall in &floor.walls {
            for o in floor.openings_on(wall.id) {
                self.draw_opening(painter, rect, wall, o, &pal);
            }
        }
        if let Some(w) = self.selected.and_then(|id| floor.wall(id)) {
            self.draw_wall_outline(painter, rect, w, Stroke::new(3.0_f32, pal.selection));
        }
        if let Some(w) = self.hover_wall.and_then(|id| floor.wall(id)) {
            self.draw_wall_outline(painter, rect, w, Stroke::new(3.0_f32, pal.hover));
        }
        self.draw_rubber_band(painter, rect, &pal);
        self.draw_crosshairs(painter, rect, &pal);
    }

    fn draw_crosshairs(&self, painter: &egui::Painter, rect: Rect, pal: &Palette) {
        if !self.flags.contains(&ViewFlag::Crosshairs) {
            return;
        }
        let Some(p) = self.cursor_world else { return };
        let at = self.camera.world_to_screen(rect, p);
        let stroke = Stroke::new(1.0_f32, pal.text.gamma_multiply(0.45));
        painter.line_segment(
            [Pos2::new(rect.left(), at.y), Pos2::new(rect.right(), at.y)],
            stroke,
        );
        painter.line_segment(
            [Pos2::new(at.x, rect.top()), Pos2::new(at.x, rect.bottom())],
            stroke,
        );
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect, pal: &Palette) {
        let cam = self.camera;
        let mut spacing = self.defaults.grid.spacing.max(0.25);
        while spacing * cam.px_per_in < 8.0 {
            spacing *= 5.0;
        }
        if self.flags.contains(&ViewFlag::ReferenceGrid) {
            self.draw_grid_lines(painter, rect, spacing, pal);
        }
        let o = cam.world_to_screen(rect, Point::ZERO);
        let red = Stroke::new(1.5_f32, pal.origin_marker);
        painter.line_segment([o - Vec2::new(6.0, 0.0), o + Vec2::new(6.0, 0.0)], red);
        painter.line_segment([o - Vec2::new(0.0, 6.0), o + Vec2::new(0.0, 6.0)], red);
    }

    fn draw_grid_lines(&self, painter: &egui::Painter, rect: Rect, spacing: f64, pal: &Palette) {
        let cam = self.camera;
        let tl = cam.screen_to_world(rect, rect.left_top());
        let br = cam.screen_to_world(rect, rect.right_bottom());
        let minor = Stroke::new(1.0_f32, pal.grid_minor);
        let major = Stroke::new(1.0_f32, pal.grid_major);
        let x0 = (tl.x / spacing).floor() as i64;
        let x1 = (br.x / spacing).ceil() as i64;
        for k in x0..=x1 {
            let x = cam
                .world_to_screen(rect, Point::new(k as f64 * spacing, 0.0))
                .x;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                stroke,
            );
        }
        let y0 = (br.y / spacing).floor() as i64;
        let y1 = (tl.y / spacing).ceil() as i64;
        for k in y0..=y1 {
            let y = cam
                .world_to_screen(rect, Point::new(0.0, k as f64 * spacing))
                .y;
            let stroke = if k % 5 == 0 { major } else { minor };
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                stroke,
            );
        }
    }

    fn draw_rooms(&self, painter: &egui::Painter, rect: Rect, pal: &Palette) {
        let outline = Stroke::new(1.5_f32, pal.room_outline);
        for room in &self.rooms {
            let pts: Vec<Pos2> = room
                .polygon
                .iter()
                .map(|p| self.camera.world_to_screen(rect, *p))
                .collect();
            painter.add(Shape::closed_line(pts, outline));
            painter.text(
                self.camera.world_to_screen(rect, room.centroid),
                Align2::CENTER_CENTER,
                format!("{}\n{} sq ft", room.label, room.area_sq_ft().round()),
                FontId::proportional(13.0),
                pal.room_label,
            );
        }
    }

    fn quad(&self, rect: Rect, pts: [Point; 4]) -> Vec<Pos2> {
        pts.iter()
            .map(|p| self.camera.world_to_screen(rect, *p))
            .collect()
    }

    fn draw_wall(&self, painter: &egui::Painter, rect: Rect, wall: &Wall, pal: &Palette) {
        let fill = match wall.kind {
            WallKind::Exterior => pal.wall_fill_exterior,
            WallKind::Interior => pal.wall_fill_interior,
        };
        let pts = self.quad(rect, wall.footprint());
        painter.add(Shape::convex_polygon(
            pts,
            fill,
            Stroke::new(1.0_f32, pal.wall_stroke),
        ));
    }

    fn draw_wall_outline(&self, painter: &egui::Painter, rect: Rect, wall: &Wall, stroke: Stroke) {
        painter.add(Shape::closed_line(
            self.quad(rect, wall.footprint()),
            stroke,
        ));
    }

    fn draw_opening(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        wall: &Wall,
        o: &plan_core::Opening,
        pal: &Palette,
    ) {
        let cam = self.camera;
        let sc = |p: Point| cam.world_to_screen(rect, p);
        let line = Stroke::new(1.0_f32, pal.opening_line);
        let d = wall.direction();
        let n = wall.normal();
        let half = wall.thickness * 0.5;
        let over = half + 1.0 + 1.0 / cam.px_per_in;
        let pa = wall.point_at(o.start_offset());
        let pb = wall.point_at(o.end_offset());

        // A canvas-colored quad hides the wall fill and its stroke across the opening.
        let gap = [
            pa.add(n.scale(over)),
            pb.add(n.scale(over)),
            pb.sub(n.scale(over)),
            pa.sub(n.scale(over)),
        ];
        painter.add(Shape::convex_polygon(
            self.quad(rect, gap),
            pal.background,
            Stroke::NONE,
        ));
        for j in [pa, pb] {
            painter.line_segment([sc(j.add(n.scale(half))), sc(j.sub(n.scale(half)))], line);
        }

        match o.kind {
            OpeningKind::Door => {
                let (hinge, toward_other, side) = if o.swing_flipped {
                    (pb, d.scale(-1.0), n.scale(-1.0))
                } else {
                    (pa, d, n)
                };
                painter.line_segment([sc(hinge), sc(hinge.add(side.scale(o.width)))], line);
                let arc: Vec<Pos2> = (0..=16)
                    .map(|i| {
                        let a = i as f64 / 16.0 * std::f64::consts::FRAC_PI_2;
                        let v = toward_other.scale(a.cos()).add(side.scale(a.sin()));
                        sc(hinge.add(v.scale(o.width)))
                    })
                    .collect();
                painter.add(Shape::line(arc, Stroke::new(1.0_f32, pal.door_arc)));
            }
            OpeningKind::Window => {
                for off in [half, 0.0, -half] {
                    let s = n.scale(off);
                    painter.line_segment(
                        [sc(pa.add(s)), sc(pb.add(s))],
                        Stroke::new(0.8_f32, pal.opening_line),
                    );
                }
            }
        }
    }

    fn draw_rubber_band(&self, painter: &egui::Painter, rect: Rect, pal: &Palette) {
        if !matches!(self.tool, Tool::Wall { .. }) {
            return;
        }
        let Some(snapped) = self.snapped else { return };
        if let Some(start) = self.pending_start {
            let len = start.dist(snapped);
            if len > 0.01 {
                let ghost = Wall {
                    id: 0,
                    start,
                    end: snapped,
                    thickness: self.current_thickness(),
                    height: self.default_height(self.wall_kind()),
                    kind: self.wall_kind(),
                    layer: DEFAULT_WALL_LAYER.to_string(),
                };
                painter.add(Shape::convex_polygon(
                    self.quad(rect, ghost.footprint()),
                    pal.ghost_fill,
                    Stroke::new(1.0_f32, pal.ghost_stroke),
                ));
                if self.flags.contains(&ViewFlag::TemporaryDimensions) {
                    let mid = Point::lerp(start, snapped, 0.5)
                        .add(ghost.normal().scale(ghost.thickness * 0.5));
                    painter.text(
                        self.camera.world_to_screen(rect, mid)
                            + Vec2::new(0.0, -8.0) * ghost.normal().y.signum() as f32,
                        Align2::CENTER_CENTER,
                        self.fmt_dim(len),
                        FontId::proportional(13.0),
                        pal.dimension_text,
                    );
                }
            }
        }
        painter.circle_stroke(
            self.camera.world_to_screen(rect, snapped),
            5.0,
            Stroke::new(1.5_f32, pal.ghost_stroke),
        );
    }
}

impl PlanApp {
    /// Rebuilds the egui visuals when the brightness changed, and writes the
    /// settings file once a change has settled (not while a slider is dragged).
    fn sync_settings(&mut self, ctx: &egui::Context) {
        if (self.settings.brightness - self.applied_brightness).abs() > f32::EPSILON {
            theme::apply_chrome(ctx, self.settings.brightness);
            self.applied_brightness = self.settings.brightness;
        }
        if self.settings != self.saved_settings && !ctx.input(|i| i.pointer.any_down()) {
            match self.settings.save() {
                Ok(()) => {}
                Err(e) => self.message = format!("Could not save settings: {e}"),
            }
            self.saved_settings = self.settings;
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new("About Plan Studio")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.heading("Plan Studio");
                ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                ui.label("A 2D floor-plan editor built with Rust and egui.");
                ui.separator();
                ui.label("Released under the MIT License.");
            });
        self.show_about = open;
    }
}

impl eframe::App for PlanApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.rooms_dirty {
            self.rooms = detect_rooms(&self.floor().walls, 0.5);
            self.rooms_dirty = false;
        }
        let mut actions = Vec::new();
        self.handle_keys(ctx, &mut actions);
        self.menu_bar(ctx, &mut actions);
        self.toolbar_rows(ctx, &mut actions);
        self.status_bar(ctx);
        self.properties_panel(ctx);
        self.view_bar(ctx, &mut actions);
        if !actions.is_empty() {
            ctx.request_repaint();
        }
        for action in actions {
            if action == Action::Quit {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            self.apply(action);
        }
        self.dock_panel(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| self.canvas(ctx, ui));
        self.about_window(ctx);
        self.dialogs(ctx);
        self.sync_settings(ctx);
        if self.rooms_dirty {
            ctx.request_repaint();
        }
    }
}

/// A combo box of the wall types of `kind`, with the thickness of each.
fn wall_type_combo(
    ui: &mut egui::Ui,
    label: &str,
    current: &mut String,
    types: &[plan_core::WallTypeDef],
    kind: WallKind,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let shown = types
            .iter()
            .find(|t| t.name == *current)
            .map_or_else(|| current.clone(), type_label);
        egui::ComboBox::from_id_salt(("default_wall_type", label))
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for t in types.iter().filter(|t| t.kind == kind) {
                    ui.selectable_value(current, t.name.clone(), type_label(t));
                }
            });
    });
}

fn type_label(t: &plan_core::WallTypeDef) -> String {
    format!("{} ({})", t.name, dialogs::fmt_short(t.thickness()))
}

/// DragValue row in inches; returns true if the value changed.
fn inch_drag(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .speed(0.25)
                .range(range)
                .suffix("\""),
        )
        .changed()
    })
    .inner
}

fn snap_to_grid(p: Point, step: f64) -> Point {
    if step <= 0.0 {
        return p;
    }
    Point::new((p.x / step).round() * step, (p.y / step).round() * step)
}

/// Snap `p` to a multiple of `increment_deg` from `start`, then re-snap the length to `step`.
fn angle_snap(start: Point, p: Point, step: f64, increment_deg: f64) -> Option<Point> {
    let v = p.sub(start);
    let len = v.length();
    if len < 1e-6 {
        return None;
    }
    let inc = increment_deg.max(1.0).to_radians();
    let a = (v.angle() / inc).round() * inc;
    let len = if step > 0.0 {
        (len / step).round() * step
    } else {
        len
    };
    Some(start.add(Point::new(a.cos(), a.sin()).scale(len)))
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Plan Studio"),
        ..Default::default()
    };
    eframe::run_native(
        "Plan Studio",
        options,
        Box::new(|cc| {
            icons::install(&cc.egui_ctx);
            let settings = AppSettings::load();
            theme::apply_chrome(&cc.egui_ctx, settings.brightness);
            let (defaults, note) = plan_defaults::load();
            Ok(Box::new(PlanApp::new(settings, defaults, note)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> PlanApp {
        PlanApp::new(AppSettings::default(), plan_defaults::embedded(), None)
    }

    #[test]
    fn app_starts_from_the_chief_template() {
        let mut a = app();
        assert_eq!(a.project.floors[0].ceiling_height, 109.125);
        a.tool = Tool::Wall {
            kind: WallKind::Exterior,
        };
        assert_eq!(a.current_thickness(), 7.625);
        a.tool = Tool::Wall {
            kind: WallKind::Interior,
        };
        assert_eq!(a.current_thickness(), 4.5);
        assert_eq!(a.default_height(WallKind::Exterior), 109.125);
        assert_eq!(a.defaults.grid.snap, 1.0);
        // Dimension text uses the 1/8" smallest fraction from the defaults.
        assert_eq!(a.fmt_dim(10.1875), "0'-10 1/4\"");
    }

    #[test]
    fn new_project_uses_the_defaults() {
        let mut a = app();
        a.defaults.rooms.ceiling_height = 120.0;
        a.new_project();
        assert_eq!(a.project.floors[0].ceiling_height, 120.0);
    }

    #[test]
    fn doors_in_exterior_walls_use_the_exterior_door_defaults() {
        let mut a = app();
        let w = a.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            7.625,
            109.125,
            WallKind::Exterior,
        );
        a.tool = Tool::Door;
        a.cursor_world = Some(Point::new(120.0, 0.0));
        a.handle_click(Point::new(120.0, 0.0));
        let o = a.project.floors[0].openings_on(w).next().unwrap();
        assert_eq!((o.width, o.height), (36.0, 96.0));
        // Windows come from the window defaults, with their extras recorded.
        a.tool = Tool::Window;
        a.handle_click(Point::new(40.0, 0.0));
        let win = a.project.floors[0]
            .openings
            .iter()
            .find(|o| o.kind == OpeningKind::Window)
            .unwrap();
        assert_eq!((win.width, win.height, win.sill_height), (32.0, 72.0, 24.0));
        assert!(a.extras.openings.contains_key(&win.id));
    }
}
