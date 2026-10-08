//! Plan Studio: a 2D floor-plan editor on top of `plan-core`.
//!
//! World units are inches with Y up; the camera flips Y when mapping to screen.
//!
//! `main.rs` owns the window, the panels, the menu/toolbar wiring and the
//! specification dialogs. Everything that edits the plan lives in `editor/`
//! (shared services, [`EditorContext`]) and `tools/` (one module per Chief
//! tool behind the [`Tool`] trait); see `docs/architecture-tools.md`.

mod dialogs;
mod editor;
mod icons;
mod menus;
mod plan_defaults;
mod theme;
mod toolbar;
mod tools;

use dialogs::{
    DefaultsDialog, DefaultsEntry, DefaultsOutcome, OpeningDialog, OpeningTarget, Outcome,
    WallDialog, WallTarget,
};
use editor::{render, Camera, EditorContext, EditorRequest, ObjectRef};
use eframe::egui::{self, Color32, Pos2, Vec2};
use plan_core::geometry::Point;
use plan_core::{Id, Opening, OpeningKind, PlanDefaults, Project, WallKind};
use std::path::PathBuf;
use theme::{AppSettings, CanvasTheme};
use toolbar::{Action, BarState, Dock, Hotkeys, Toolbars};
use tools::{KeyEvent, PointerEvent, ToolId, ToolResult, ToolSet};

const STATUS_GRAY: Color32 = Color32::from_rgb(0x2C, 0x2C, 0x2C);
const ZOOM_STEP: f64 = 1.25;
const ZOOM_HISTORY_CAP: usize = 50;
const VIEW_BAR_WIDTH: f32 = 36.0;
const DOCK_WIDTH: f32 = 300.0;

/// The specification dialog that is open, if any (one at a time).
enum ActiveDialog {
    Wall(Box<WallDialog>),
    Opening(Box<OpeningDialog>),
}

struct PlanApp {
    cx: EditorContext,
    tools: ToolSet,
    path: Option<PathBuf>,
    camera: Camera,
    zoom_history: Vec<Camera>,
    toolbars: Toolbars,
    dock: Option<Dock>,
    hotkeys: Hotkeys,
    settings: AppSettings,
    /// The settings last written to disk.
    saved_settings: AppSettings,
    /// The brightness the egui visuals were last built for.
    applied_brightness: f32,
    show_about: bool,
    /// A primary press started on the canvas and has not been released.
    pressed_in_canvas: bool,
    /// The press was a double-click the tool took; skip its release.
    suppress_release: bool,
    dialog: Option<ActiveDialog>,
    defaults_dialog: Option<DefaultsDialog>,
}

/// The state the toolbars and menus draw themselves from.
fn bar_state<'a>(
    cx: &'a EditorContext,
    tool: ToolId,
    dock: Option<Dock>,
    brightness: f32,
) -> BarState<'a> {
    BarState {
        tool,
        flags: &cx.view_flags,
        dock,
        floor: cx.floor,
        floor_count: cx.project.floors.len(),
        view_name: "Floor Plan View",
        brightness,
        undo_label: cx.undo_label(),
        redo_label: cx.redo_label(),
    }
}

impl PlanApp {
    fn new(settings: AppSettings, defaults: PlanDefaults, note: Option<String>) -> Self {
        let mut cx = EditorContext::new(defaults);
        cx.status = note.unwrap_or_default();
        cx.palette = settings.theme.palette();
        Self {
            cx,
            tools: ToolSet::new(),
            path: None,
            camera: Camera::default_view(),
            zoom_history: Vec::new(),
            toolbars: Toolbars::new(),
            dock: None,
            hotkeys: Hotkeys::default(),
            settings,
            saved_settings: settings,
            applied_brightness: settings.brightness,
            show_about: false,
            pressed_in_canvas: false,
            suppress_release: false,
            dialog: None,
            defaults_dialog: None,
        }
    }

    fn set_tool(&mut self, tool: ToolId) {
        self.tools.set_active(&mut self.cx, tool);
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
            .cx
            .floor()
            .walls
            .iter()
            .flat_map(|w| w.footprint())
            .collect();
        if points.is_empty() {
            let rect = self.camera.rect;
            self.camera = Camera::default_view();
            self.camera.rect = rect;
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
        let r = self.camera.rect;
        let scale = (r.width() as f64 / w).min(r.height() as f64 / h);
        self.camera = Camera {
            center: Point::new((lo.x + hi.x) * 0.5, (lo.y + hi.y) * 0.5),
            px_per_in: scale.clamp(0.05, 50.0),
            rect: r,
        };
    }

    fn change_floor(&mut self, up: bool) {
        let next = if up {
            (self.cx.floor + 1 < self.cx.project.floors.len()).then_some(self.cx.floor + 1)
        } else {
            self.cx.floor.checked_sub(1)
        };
        if let Some(n) = next {
            self.cx.floor = n;
            self.reset_view_state();
        }
    }

    /// Drops the selection and any tool in progress (floor change, new file).
    fn reset_view_state(&mut self) {
        self.cx.reset_view_state();
        self.tools.restart(&mut self.cx);
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
                    let rect = self.camera.rect;
                    self.camera = cam;
                    self.camera.rect = rect;
                }
            }
            Action::Undo => {
                self.cx.status = match self.cx.undo() {
                    Some(l) => format!("Undid {l}"),
                    None => "Nothing to undo".into(),
                };
            }
            Action::Redo => {
                self.cx.status = match self.cx.redo() {
                    Some(l) => format!("Redid {l}"),
                    None => "Nothing to redo".into(),
                };
            }
            Action::FillWindow => self.fill_window(),
            Action::FloorUp => self.change_floor(true),
            Action::FloorDown => self.change_floor(false),
            Action::TogglePan => {
                let next = if self.tools.active_id() == ToolId::Pan {
                    ToolId::Select
                } else {
                    ToolId::Pan
                };
                self.set_tool(next);
            }
            Action::ToggleFlag(f) => {
                if !self.cx.view_flags.remove(&f) {
                    self.cx.view_flags.insert(f);
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
                self.cx.status = format!("Not yet implemented: {name}");
            }
        }
    }

    // ----- file operations -----

    fn new_project(&mut self) {
        let project = Project::from_defaults("Untitled", &self.cx.defaults);
        self.cx.set_project(project);
        self.path = None;
        self.tools.restart(&mut self.cx);
        self.cx.status = "New project".into();
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
                self.cx.set_project(p);
                self.cx.status = format!("Opened {}", path.display());
                self.path = Some(path);
                self.tools.restart(&mut self.cx);
            }
            Ok(_) => self.cx.status = "File contains no floors".into(),
            Err(e) => self.cx.status = format!("Open failed: {e}"),
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
            .cx
            .project
            .to_json()
            .map_err(|e| e.to_string())
            .and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.cx.status = format!("Saved {}", path.display());
                self.path = Some(path);
            }
            Err(e) => self.cx.status = format!("Save failed: {e}"),
        }
    }

    /// File > Templates > Save Current Defaults as My Template.
    fn save_template(&mut self) {
        self.cx.status = match plan_defaults::save_user(&self.cx.defaults) {
            Ok(path) => format!("Saved your template to {}", path.display()),
            Err(e) => format!("Could not save the template: {e}"),
        };
    }

    /// File > Templates > Reset to Chief X18 Template.
    fn reset_template(&mut self) {
        self.cx.defaults = plan_defaults::embedded();
        // Forget the per-session edits of the default dialogs so they show
        // the template's values again.
        for key in [
            WallTarget::DefaultExterior.key(),
            WallTarget::DefaultInterior.key(),
            WallTarget::DefaultFoundation.key(),
        ] {
            self.cx.extras.walls.remove(&key);
        }
        for target in [
            OpeningTarget::DefaultDoor,
            OpeningTarget::DefaultExteriorDoor,
            OpeningTarget::DefaultWindow,
        ] {
            self.cx.extras.openings.remove(&target.key());
        }
        self.cx.status = match plan_defaults::clear_user() {
            Ok(()) => "Reset to the Chief X18 template".into(),
            Err(e) => format!("Reset, but could not remove your saved template: {e}"),
        };
    }

    // ----- input -----

    /// Runs what a tool asked for: dialogs, panning, tool switches.
    fn finish_tool_call(&mut self, ctx: &egui::Context, res: &ToolResult) {
        if res.repaint || res.commit.is_some() {
            ctx.request_repaint();
        }
        if let Some(t) = res.switch_to {
            self.set_tool(t);
        }
        for req in std::mem::take(&mut self.cx.requests) {
            match req {
                EditorRequest::OpenSpec(o) => self.open_spec(o),
                EditorRequest::PanPixels(d) => self.camera.pan_by_pixels(d),
            }
        }
    }

    fn open_spec(&mut self, o: ObjectRef) {
        match o {
            ObjectRef::Wall(id) => self.open_wall_dialog(id),
            ObjectRef::Opening(id) => self.open_opening_dialog(id),
            other => {
                self.cx.status =
                    format!("{} specification: not yet implemented", other.type_name());
            }
        }
    }

    fn send_key(&mut self, ctx: &egui::Context, k: KeyEvent) {
        let is_esc = k.is(egui::Key::Escape);
        let is_del = k.is(egui::Key::Delete) || k.is(egui::Key::Backspace);
        let res = self.tools.active_mut().key(&mut self.cx, k);
        self.finish_tool_call(ctx, &res);
        if !res.consumed {
            if is_esc && self.tools.active_id() != ToolId::Select {
                self.set_tool(ToolId::Select);
            } else if is_del {
                self.cx.delete_selection();
            }
        }
    }

    /// Applies hotkeys (see `toolbar::BINDINGS`) and forwards editing keys to
    /// the active tool.
    fn handle_keys(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if self.dialog.is_some() || self.defaults_dialog.is_some() {
            return;
        }
        let editing = self.cx.temp.editing.is_some();
        if !editing {
            actions.extend(self.hotkeys.poll(ctx));
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } => {
                    use egui::Key::*;
                    let arrow = matches!(key, ArrowLeft | ArrowRight | ArrowUp | ArrowDown);
                    let wanted = matches!(key, Escape | Delete | Backspace | Tab | Enter) || arrow;
                    // Cmd/Ctrl chords belong to the hotkey table.
                    if wanted && (!repeat || arrow || key == Backspace) && !modifiers.command {
                        self.send_key(
                            ctx,
                            KeyEvent {
                                key: Some(key),
                                text: None,
                                modifiers,
                            },
                        );
                    }
                }
                egui::Event::Text(t) if editing => {
                    self.send_key(ctx, KeyEvent::text(&t));
                }
                _ => {}
            }
        }
    }

    fn canvas_event(&self, ctx: &egui::Context, pos: Pos2, down: bool) -> PointerEvent {
        let (mods, delta) = ctx.input(|i| (i.modifiers, i.pointer.delta()));
        let world = self.camera.screen_to_world(pos);
        let snap = self.cx.snap_at(world, None, mods.alt, &[]);
        PointerEvent {
            world,
            snapped: snap.point,
            snap,
            screen: pos,
            modifiers: mods,
            button: egui::PointerButton::Primary,
            down,
            drag_delta: delta,
        }
    }

    /// Feeds the pointer to the active tool.
    fn dispatch_pointer(&mut self, ctx: &egui::Context, resp: &egui::Response) {
        let (pressed, released, down, double, latest) = ctx.input(|i| {
            let p = &i.pointer;
            (
                p.button_pressed(egui::PointerButton::Primary),
                p.button_released(egui::PointerButton::Primary),
                p.button_down(egui::PointerButton::Primary),
                p.button_double_clicked(egui::PointerButton::Primary),
                p.latest_pos(),
            )
        });
        if self.dialog.is_some() {
            self.pressed_in_canvas = false;
            return;
        }
        let Some(pos) = latest else { return };
        if pressed && resp.hovered() {
            self.pressed_in_canvas = true;
            self.suppress_release = false;
            let ev = self.canvas_event(ctx, pos, true);
            if double {
                let res = self.tools.active_mut().double_click(&mut self.cx, ev);
                self.finish_tool_call(ctx, &res);
                if res.consumed {
                    self.suppress_release = true;
                    return;
                }
            }
            let res = self.tools.active_mut().pointer_down(&mut self.cx, ev);
            self.finish_tool_call(ctx, &res);
        } else if self.pressed_in_canvas && released {
            self.pressed_in_canvas = false;
            if !self.suppress_release {
                let ev = self.canvas_event(ctx, pos, false);
                let res = self.tools.active_mut().pointer_up(&mut self.cx, ev);
                self.finish_tool_call(ctx, &res);
            }
            self.suppress_release = false;
        } else if (resp.hovered() || self.pressed_in_canvas)
            && (down || ctx.input(|i| i.pointer.delta() != Vec2::ZERO))
        {
            let ev = self.canvas_event(ctx, pos, down && self.pressed_in_canvas);
            let res = self.tools.active_mut().pointer_move(&mut self.cx, ev);
            self.finish_tool_call(ctx, &res);
        }
        if resp.clicked_by(egui::PointerButton::Secondary) {
            // Right-click ends a wall chain (the tool's Esc), without leaving the tool.
            let res = self
                .tools
                .active_mut()
                .key(&mut self.cx, KeyEvent::escape());
            self.finish_tool_call(ctx, &res);
        }
    }

    // ----- UI panels -----

    fn menu_bar(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                let state = bar_state(
                    &self.cx,
                    self.tools.active_id(),
                    self.dock,
                    self.settings.brightness,
                );
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
                        let state = bar_state(
                            &self.cx,
                            self.tools.active_id(),
                            self.dock,
                            self.settings.brightness,
                        );
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
                        let state = bar_state(
                            &self.cx,
                            self.tools.active_id(),
                            self.dock,
                            self.settings.brightness,
                        );
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
                let d = &mut self.cx.defaults;
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
                if self.cx.rooms.is_empty() {
                    ui.weak("None detected (close a loop of walls)");
                }
                for room in &self.cx.rooms {
                    ui.label(format!(
                        "{}  -  {} sq ft",
                        self.cx.room_name(room),
                        room.area_sq_ft().round()
                    ));
                }
                ui.separator();
                self.selected_wall_section(ui);
            });
    }

    /// The wall the selection stands for: the wall itself, or an opening's host.
    fn selected_wall_id(&self) -> Option<Id> {
        match self.cx.selection.single()? {
            ObjectRef::Wall(id) => Some(id),
            ObjectRef::Opening(id) => self
                .cx
                .floor()
                .openings
                .iter()
                .find(|o| o.id == id)
                .map(|o| o.wall_id),
            _ => None,
        }
    }

    fn selected_wall_section(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected_wall_id() else {
            ui.weak("No wall selected");
            return;
        };
        let Some(wall) = self.cx.floor().wall(id) else {
            return;
        };
        let (len, kind, mut thickness) = (wall.length(), wall.kind, wall.thickness);
        let fmt = self.cx.defaults.dim_format();
        let openings: Vec<(Id, OpeningKind, f64, f64)> = self
            .cx
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
            self.cx.begin_change_merged("Change Wall Thickness");
            let floor = self.cx.floor;
            if let Some(w) = self.cx.project.floors[floor].wall_mut(id) {
                w.thickness = thickness;
            }
            self.cx.mark_dirty();
        }
        let mut open_wall_spec = false;
        let mut open_opening_spec = None;
        let mut delete_wall = false;
        ui.horizontal(|ui| {
            open_wall_spec = ui.button("Open Specification\u{2026}").clicked();
            delete_wall = ui.button("Delete wall").clicked();
        });
        if delete_wall {
            self.cx.selection.set(ObjectRef::Wall(id));
            self.cx.delete_selection();
            return;
        }
        if open_wall_spec {
            self.open_wall_dialog(id);
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
            self.cx.begin_change("Delete Opening");
            let floor = self.cx.floor;
            self.cx.project.remove_opening(floor, oid);
            self.cx.mark_dirty();
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
                    match self.cx.cursor_world {
                        Some(p) => ui.monospace(format!(
                            "X: {}  Y: {}",
                            self.cx.fmt_dim(p.x),
                            self.cx.fmt_dim(p.y)
                        )),
                        None => ui.monospace("X: --  Y: --"),
                    };
                    if let Some(prefix) = self.hotkeys.pending_label() {
                        ui.separator();
                        ui.strong(prefix);
                    }
                    if let Some(r) = &self.cx.readout {
                        ui.separator();
                        ui.label(r);
                    }
                    if let Some(s) = self.cx.last_snap.filter(|s| s.kind.is_object_snap()) {
                        ui.separator();
                        ui.label(format!("Snap: {}", s.kind.label()));
                    }
                    ui.separator();
                    ui.label(self.tools.active().hint());
                    if !self.cx.status.is_empty() {
                        ui.separator();
                        ui.weak(&self.cx.status);
                    }
                });
            });
    }

    // ----- canvas -----

    fn canvas(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        self.camera.rect = resp.rect;
        self.cx.px_per_in = self.camera.px_per_in;
        self.cx.palette = self.settings.theme.palette();

        self.handle_camera_input(ui, &resp);

        let new_cursor = resp.hover_pos().map(|p| self.camera.screen_to_world(p));
        if new_cursor.map(|p| (p.x, p.y)) != self.cx.cursor_world.map(|p| (p.x, p.y)) {
            ctx.request_repaint();
        }
        self.cx.cursor_world = new_cursor;
        self.cx.px_per_in = self.camera.px_per_in;
        self.dispatch_pointer(ctx, &resp);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(self.tools.active().cursor());
        }

        self.cx.refresh();
        render::draw_plan(&self.cx, &painter, &self.camera);
        self.tools
            .active()
            .draw_overlay(&self.cx, &painter, &self.camera);
        render::draw_crosshairs(&self.cx, &painter, &self.camera);
    }

    fn handle_camera_input(&mut self, ui: &egui::Ui, resp: &egui::Response) {
        if resp.dragged_by(egui::PointerButton::Middle)
            || resp.dragged_by(egui::PointerButton::Secondary)
        {
            self.camera.pan_by_pixels(resp.drag_delta());
        }
        if let Some(pos) = resp.hover_pos() {
            let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
            let factor = (scroll as f64 * 0.005).exp() * zoom as f64;
            if (factor - 1.0).abs() > 1e-9 {
                self.camera.zoom_about(pos, factor);
            }
        }
    }

    /// The floating Edit toolbar at the bottom left of the canvas, built from
    /// the active tool's `edit_toolbar`.
    fn edit_toolbar(&mut self, ctx: &egui::Context) {
        if self.dialog.is_some() {
            return;
        }
        let actions = self.tools.active().edit_toolbar(&self.cx);
        if actions.is_empty() {
            return;
        }
        let rect = self.camera.rect;
        let mut clicked = None;
        egui::Area::new(egui::Id::new("edit_toolbar"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(rect.left() + 10.0, rect.bottom() - 44.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for a in &actions {
                            let btn = match a.icon {
                                Some(id) => egui::Button::image(
                                    egui::Image::new(icons::icon(id))
                                        .fit_to_exact_size(Vec2::splat(18.0))
                                        .tint(Color32::WHITE),
                                ),
                                None => egui::Button::new("Copy"),
                            };
                            let r = ui.add_enabled(a.enabled, btn).on_hover_text(a.label);
                            if r.clicked() {
                                clicked = Some(a.kind);
                            }
                        }
                    });
                });
            });
        if let Some(kind) = clicked {
            self.cx.apply_edit_action(kind);
            for req in std::mem::take(&mut self.cx.requests) {
                if let EditorRequest::OpenSpec(o) = req {
                    self.open_spec(o);
                }
            }
        }
    }

    // ----- specification dialogs -----

    fn open_wall_dialog(&mut self, id: Id) {
        let Some(wall) = self.cx.floor().wall(id).cloned() else {
            return;
        };
        let openings: Vec<Opening> = self.cx.floor().openings_on(id).cloned().collect();
        let extras = self.cx.extras.walls.get(&id).cloned().unwrap_or_default();
        let default_height = self.cx.wall_height(wall.kind);
        self.dialog = Some(ActiveDialog::Wall(Box::new(WallDialog::new(
            WallTarget::Wall(id),
            wall,
            openings,
            extras,
            default_height,
            self.cx.wall_types().to_vec(),
        ))));
    }

    fn open_opening_dialog(&mut self, id: Id) {
        let floor = self.cx.floor();
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
        let extras = self
            .cx
            .extras
            .openings
            .get(&id)
            .cloned()
            .unwrap_or_default();
        self.dialog = Some(ActiveDialog::Opening(Box::new(OpeningDialog::for_opening(
            opening, &wall, others, extras,
        ))));
    }

    fn open_defaults_entry(&mut self, entry: DefaultsEntry) {
        let wall_dialog = |target: WallTarget, app: &Self| {
            let defaults = &app.cx.defaults;
            let w = match target {
                WallTarget::DefaultInterior => &defaults.interior_wall,
                WallTarget::DefaultFoundation => &defaults.foundation_wall,
                _ => &defaults.exterior_wall,
            };
            let thickness = match target {
                WallTarget::DefaultInterior => defaults.interior_thickness(),
                WallTarget::DefaultFoundation => defaults.foundation_thickness(),
                _ => defaults.exterior_thickness(),
            };
            let extras = app.cx.extras.walls.get(&target.key()).cloned();
            ActiveDialog::Wall(Box::new(WallDialog::for_default(
                target,
                thickness,
                w.height,
                extras.unwrap_or_default(),
                defaults.wall_types.clone(),
                &w.wall_type,
            )))
        };
        let opening_dialog = |target: OpeningTarget, template: Opening, app: &Self| {
            ActiveDialog::Opening(Box::new(OpeningDialog::for_default(
                target,
                template,
                app.cx.default_opening_extras(target),
            )))
        };
        self.dialog = match entry {
            DefaultsEntry::ExteriorWall => Some(wall_dialog(WallTarget::DefaultExterior, self)),
            DefaultsEntry::InteriorWall => Some(wall_dialog(WallTarget::DefaultInterior, self)),
            DefaultsEntry::FoundationWall => Some(wall_dialog(WallTarget::DefaultFoundation, self)),
            DefaultsEntry::InteriorDoor => Some(opening_dialog(
                OpeningTarget::DefaultDoor,
                plan_defaults::door_template(&self.cx.defaults, false),
                self,
            )),
            DefaultsEntry::ExteriorDoor => Some(opening_dialog(
                OpeningTarget::DefaultExteriorDoor,
                plan_defaults::door_template(&self.cx.defaults, true),
                self,
            )),
            DefaultsEntry::Window => Some(opening_dialog(
                OpeningTarget::DefaultWindow,
                plan_defaults::window_template(&self.cx.defaults),
                self,
            )),
            DefaultsEntry::Dimensions | DefaultsEntry::RoomTypes => {
                self.cx.status = "Coming in a later phase".into();
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
                    self.cx.mark_dirty();
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
        // Wall types edited through "Define..." go to the defaults (and so to
        // the saved template) and to the plan's registry.
        for t in d.edited_types() {
            match self
                .cx
                .defaults
                .wall_types
                .iter_mut()
                .find(|x| x.name == t.name)
            {
                Some(slot) => *slot = t.clone(),
                None => self.cx.defaults.wall_types.push(t.clone()),
            }
            if matches!(d.target(), WallTarget::Wall(_)) || !self.cx.project.wall_types.is_empty() {
                self.cx.project.register_wall_type(t.clone());
            }
        }
        match d.target() {
            WallTarget::Wall(id) => {
                self.cx.begin_change("Wall Specification");
                let fl = self.cx.floor;
                let orig = self.cx.floor().wall(id).cloned();
                let floor = &mut self.cx.project.floors[fl];
                if let Some(w) = floor.wall_mut(id) {
                    *w = draft.clone();
                }
                for adjusted in d.adjusted_openings() {
                    if let Some(o) = floor.openings.iter_mut().find(|o| o.id == adjusted.id) {
                        o.center_offset = adjusted.center_offset;
                    }
                }
                // A new wall type keeps the reference line fixed (W-27).
                if let (Some(orig), Some(name)) = (orig, draft.wall_type.clone()) {
                    let changed = orig.wall_type.as_deref() != Some(name.as_str())
                        || d.edited_types().iter().any(|t| t.name == name);
                    let def = self.cx.project.wall_type_def(&name).cloned();
                    if let (true, Some(def)) = (changed, def) {
                        if let Some(w) = self.cx.project.floors[fl].wall_mut(id) {
                            w.thickness = orig.thickness;
                            w.wall_type = orig.wall_type.clone();
                        }
                        let about = draft.resize_about;
                        self.cx.project.set_wall_type(fl, id, &def, about);
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
                    &mut self.cx.defaults,
                    kind,
                    d.picked_type(),
                    draft.thickness,
                );
                let w = match d.target() {
                    WallTarget::DefaultInterior => &mut self.cx.defaults.interior_wall,
                    WallTarget::DefaultFoundation => &mut self.cx.defaults.foundation_wall,
                    _ => &mut self.cx.defaults.exterior_wall,
                };
                w.wall_type = name;
                w.height = draft.height;
            }
        }
        self.cx
            .extras
            .walls
            .insert(d.target().key(), d.extras().clone());
    }

    fn apply_opening_dialog(&mut self, d: &OpeningDialog) {
        let draft = d.draft().clone();
        match d.target() {
            OpeningTarget::Placed(id) => {
                self.cx.begin_change("Opening Specification");
                let floor = &mut self.cx.project.floors[self.cx.floor];
                if let Some(o) = floor.openings.iter_mut().find(|o| o.id == id) {
                    *o = draft;
                }
            }
            OpeningTarget::DefaultDoor => {
                let base = &self.cx.defaults.interior_door;
                self.cx.defaults.interior_door = d.extras().to_door_defaults(&draft, base, false);
            }
            OpeningTarget::DefaultExteriorDoor => {
                let base = &self.cx.defaults.exterior_door;
                self.cx.defaults.exterior_door = d.extras().to_door_defaults(&draft, base, true);
            }
            OpeningTarget::DefaultWindow => {
                let base = &self.cx.defaults.window;
                self.cx.defaults.window = d.extras().to_window_defaults(&draft, base);
            }
        }
        self.cx
            .extras
            .openings
            .insert(d.target().key(), d.extras().clone());
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
                Err(e) => self.cx.status = format!("Could not save settings: {e}"),
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
        self.cx.refresh();
        if !ctx.input(|i| i.pointer.any_down()) {
            self.cx.end_merge();
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
        self.edit_toolbar(ctx);
        self.about_window(ctx);
        self.dialogs(ctx);
        self.sync_settings(ctx);
        if self.cx.is_dirty() {
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
        let a = app();
        assert_eq!(a.cx.project.floors[0].ceiling_height, 109.125);
        assert_eq!(a.cx.wall_thickness(WallKind::Exterior), 7.625);
        assert_eq!(a.cx.wall_thickness(WallKind::Interior), 4.5);
        assert_eq!(a.cx.wall_height(WallKind::Exterior), 109.125);
        assert_eq!(a.cx.defaults.grid.snap, 1.0);
        // Dimension text uses the 1/8" smallest fraction from the defaults.
        assert_eq!(a.cx.fmt_dim(10.1875), "0'-10 1/4\"");
    }

    #[test]
    fn new_project_uses_the_defaults() {
        let mut a = app();
        a.cx.defaults.rooms.ceiling_height = 120.0;
        a.new_project();
        assert_eq!(a.cx.project.floors[0].ceiling_height, 120.0);
    }

    #[test]
    fn undo_and_redo_actions_use_step_labels() {
        let mut a = app();
        a.cx.begin_change("Move Wall");
        a.cx.project.floors[0].name = "Changed".into();
        a.apply(Action::Undo);
        assert_eq!(a.cx.status, "Undid Move Wall");
        assert_eq!(a.cx.project.floors[0].name, "1st Floor");
        a.apply(Action::Redo);
        assert_eq!(a.cx.status, "Redid Move Wall");
        a.apply(Action::Redo);
        assert_eq!(a.cx.status, "Nothing to redo");
    }

    #[test]
    fn switching_tools_keeps_a_wall_chain_between_flavors() {
        let mut a = app();
        a.set_tool(ToolId::Wall {
            kind: WallKind::Exterior,
        });
        let p = PointerEvent::at(&a.cx, Point::new(0.0, 0.0));
        a.tools.active_mut().pointer_down(&mut a.cx, p);
        a.set_tool(ToolId::Wall {
            kind: WallKind::Interior,
        });
        assert_eq!(
            a.tools.active_id(),
            ToolId::Wall {
                kind: WallKind::Interior
            }
        );
        let q = PointerEvent::at(&a.cx, Point::new(120.0, 0.0));
        a.tools.active_mut().pointer_down(&mut a.cx, q);
        a.tools.active_mut().pointer_up(&mut a.cx, q);
        assert_eq!(a.cx.project.floors[0].walls.len(), 1);
        assert_eq!(a.cx.project.floors[0].walls[0].kind, WallKind::Interior);
    }
}
