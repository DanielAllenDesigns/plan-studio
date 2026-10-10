//! Plan Studio: a 2D floor-plan editor on top of `plan-core`.
//!
//! World units are inches with Y up; the camera flips Y when mapping to screen.
//!
//! `main.rs` owns the window, the panels, the menu/toolbar wiring and the
//! specification dialogs. Everything that edits the plan lives in `editor/`
//! (shared services, [`EditorContext`]) and `tools/` (one module per Chief
//! tool behind the [`Tool`] trait); see `docs/architecture-tools.md`.

// Round 16 landed partially (see docs/integration-queue.md); several dialogs and
// commands are built but not yet reachable from the UI. Remove at the Round 16 gate.
#![allow(dead_code)]
mod chief_link;
mod dialogs;
mod editor;
mod files;
mod fonts;
mod icons;
#[cfg(target_os = "macos")]
mod mac_open;
mod menus;
mod paths;
mod plan_defaults;
#[cfg(test)]
mod scenarios;
mod shell;
mod spell;
mod templates;
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
use toolbar::{Action, BarState, Dock, Toolbars};
use tools::{KeyEvent, PointerEvent, ToolId, ToolResult, ToolSet};

const ZOOM_STEP: f64 = 1.25;
const ZOOM_HISTORY_CAP: usize = 50;
const VIEW_BAR_WIDTH: f32 = 36.0;

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
    /// Tiling, the rubber-band Zoom and the open views (`shell::view_commands`).
    shell_views: shell::view_commands::ViewShell,
    toolbars: Toolbars,
    dock: Option<Dock>,
    hotkeys: shell::hotkeys::HotkeyState,
    docks: shell::docks::DockState,
    settings: AppSettings,
    /// The settings last written to disk.
    saved_settings: AppSettings,
    /// A primary press started on the canvas and has not been released.
    pressed_in_canvas: bool,
    /// The press was a double-click the tool took; skip its release.
    suppress_release: bool,
    dialog: Option<ActiveDialog>,
    /// Specification dialogs of every other object kind.
    spec: shell::spec_dialogs::SpecDialogs,
    defaults_dialog: Option<DefaultsDialog>,
    /// The Default Settings list dialog open on top of it (dimension sets,
    /// room types, text styles).
    lists: Option<dialogs::DefaultsList>,
    view3d: shell::view3d_panel::View3dState,
    /// Save state, autosave, recovery and the unsaved-changes prompts.
    files: files::FileState,
}

/// The state the toolbars and menus draw themselves from.
fn bar_state<'a>(
    cx: &'a EditorContext,
    tool: ToolId,
    dock: Option<Dock>,
    brightness: f32,
    hotkeys: &'a shell::hotkeys::HotkeyMap,
) -> BarState<'a> {
    BarState {
        tool,
        flags: &cx.view_flags,
        dock,
        floor: cx.floor,
        floor_count: cx.project.floors.len(),
        view_name: &cx.project.active_plan_view,
        views: &cx.project.plan_views,
        brightness,
        undo_label: cx.undo_label(),
        redo_label: cx.redo_label(),
        hotkeys: Some(hotkeys),
    }
}

impl PlanApp {
    fn new(settings: AppSettings, defaults: PlanDefaults, note: Option<String>) -> Self {
        let mut defaults = defaults;
        editor::code::seed_new_plan(&mut defaults);
        let mut cx = EditorContext::new(defaults);
        // The snap, Edit Type and Replicate defaults saved in Preferences.
        dialogs::preferences::pages::apply_editing(&mut cx.defaults.editing);
        // Whatever behavior was left active, Default starts (manual p. 252).
        editor::behaviors::restore_default(&mut cx);
        cx.status = note.unwrap_or_default();
        cx.palette = settings.theme.palette();
        Self {
            cx,
            tools: ToolSet::new(),
            path: None,
            camera: Camera::default_view(),
            zoom_history: Vec::new(),
            shell_views: Default::default(),
            toolbars: Toolbars::new(),
            dock: None,
            hotkeys: shell::hotkeys::HotkeyState::default(),
            docks: shell::docks::DockState::default(),
            settings,
            saved_settings: settings,
            pressed_in_canvas: false,
            suppress_release: false,
            dialog: None,
            spec: Default::default(),
            defaults_dialog: None,
            lists: None,
            view3d: Default::default(),
            files: Default::default(),
        }
    }

    /// Picks up the first-launch template scan when its thread has finished:
    /// the plan template seeds the defaults (a plan that is still untouched
    /// starts over from them) unless the user saved defaults of their own.
    fn poll_template_detection(&mut self, ctx: &egui::Context) {
        if templates::detection_running() {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        let Some(found) = templates::poll_detection() else {
            return;
        };
        self.cx.status = self.apply_detection(found);
    }

    /// The window-level requests of the menu commands (`dialogs::app_info`):
    /// open a recent file, full screen, frame the selection; and the windows
    /// that go with them.
    fn app_commands(&mut self, ctx: &egui::Context) {
        dialogs::app_info::set_current_path(self.path.clone());
        dialogs::app_info::show_windows(ctx, &self.cx);
        if dialogs::room::take_room_types_request() {
            self.lists = Some(dialogs::DefaultsList::room_types(&self.cx));
        }
        if let Some(p) = dialogs::app_info::take_open_request() {
            self.request_file_action(files::Pending::Open(Some(p)));
        }
        if dialogs::app_info::take_full_screen_request() {
            let on = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!on));
        }
        if dialogs::app_info::take_fill_selected_request() {
            match dialogs::app_info::selection_frame(&self.cx) {
                Some((lo, hi)) => self.fill_rect(lo, hi),
                None => self.cx.status = "Select something to fill the window with".into(),
            }
        }
    }

    /// Applies a finished scan; returns the status line.
    fn apply_detection(&mut self, found: templates::Detection) -> String {
        let own = plan_defaults::user_path().is_some_and(|p| p.exists());
        self.apply_detection_with(found, own)
    }

    /// [`apply_detection`](Self::apply_detection) when the user's own saved
    /// defaults (`own`) do or do not take priority.
    fn apply_detection_with(&mut self, found: templates::Detection, own: bool) -> String {
        let mut parts: Vec<String> = Vec::new();
        match (&found.settings.plan, &found.settings.layout) {
            (None, None) => {
                parts.push("No Chief templates found; using the shipped defaults".into())
            }
            _ => parts.extend(found.note.clone()),
        }
        if let Some(e) = &found.save_error {
            parts.push(format!(
                "Could not save the template settings ({e}); not scanning again this session"
            ));
        }
        if parts.is_empty() {
            parts.push("Chief templates found".into());
        }
        if !own && found.defaults != self.cx.defaults {
            let mut seeded = editor::code::seeded(plan_defaults::embedded());
            dialogs::preferences::pages::apply_editing(&mut seeded.editing);
            let untouched = self.path.is_none()
                && !self.cx.can_undo()
                && self.cx.defaults == seeded
                && self.cx.project.floors.iter().all(|f| f.walls.is_empty());
            self.cx.defaults = found.defaults;
            dialogs::preferences::pages::apply_editing(&mut self.cx.defaults.editing);
            if untouched {
                self.new_project_plain();
            }
        }
        parts.join("; ")
    }

    fn set_tool(&mut self, tool: ToolId) {
        self.tools.set_active(&mut self.cx, tool);
    }

    /// Is a specification dialog open (any kind)?
    fn has_dialog(&self) -> bool {
        self.dialog.is_some()
            || dialogs::enter_coordinates::is_open()
            || dialogs::number_style::is_open()
            || dialogs::input_line::is_open()
            || dialogs::input_arc::is_open()
            || dialogs::move_point::is_open()
            || self.spec.is_open()
            || self.lists.is_some()
            || shell::layout_window::dialog_open()
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

    /// Fill Window: everything visible on the floor; the building alone when
    /// nothing else is drawn.
    fn fill_window(&mut self) {
        let bounds = shell::view_commands::all_bounds(&self.cx)
            .or_else(|| shell::view_commands::building_bounds(&self.cx));
        self.fit_to(bounds);
    }

    /// Frames the rectangle `lo`..`hi` (Fill Window Selected Objects).
    fn fill_rect(&mut self, lo: Point, hi: Point) {
        self.push_zoom_history();
        self.fit_camera(lo, hi);
    }

    fn fit_camera(&mut self, lo: Point, hi: Point) {
        self.camera = shell::view_commands::fit_camera(&self.camera, lo, hi);
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

    /// Runs a menu or toolbar command as ONE undo step, and none when it
    /// changed nothing (QA-24, QA-25, QA-26): every object family a command
    /// touches opens its own step, and the group folds them together.
    fn apply(&mut self, action: Action) {
        self.cx.begin_undo_group();
        let run =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.apply_command(action)));
        self.cx.end_undo_group();
        if let Err(e) = run {
            std::panic::resume_unwind(e);
        }
    }

    fn apply_command(&mut self, action: Action) {
        match action {
            Action::SetTool(t) => {
                // Plan tools work in the plan view.
                self.view3d.active = false;
                self.set_tool(t);
            }
            // File > Close, Revert, Save a Copy, Backup, Clear Menu, Archives.
            Action::Custom(id) if files::is_command(id) => self.file_command(id),
            // Edit Area, Stretch CAD and Marquee Selection (Select Objects); the
            // Arc Creation Modes (Draw Arc).
            Action::Custom(id) if tools::select::is_command(id) => {
                if tools::select::run_command(&mut self.cx, id) {
                    self.set_tool(ToolId::Select);
                }
            }
            Action::Custom(id) if tools::cad::is_command(id) => {
                tools::cad::run_command(&mut self.cx, id)
            }
            // File > Export > Picture and File > Import > 3D Symbol.
            Action::Custom(id) if dialogs::export_picture::is_command(id) => {
                let views = dialogs::export_picture::ViewInfo {
                    snapshot_3d: self.view3d.snapshot_source(),
                    view_3d_active: self.view3d.active,
                };
                dialogs::export_picture::run_command(&mut self.cx, id, views)
            }
            // Property Manager and the Excel property exchange.
            Action::Custom(id) if dialogs::property_manager::is_command(id) => {
                dialogs::property_manager::run_command(&mut self.cx, id);
            }
            // File > Print: Drawing Sheet Setup, Scale to Fit, Center Sheet,
            // Clear Printer Info, Customize Sheet Sizes; View > Watermark and
            // its defaults.
            Action::Custom(id) if dialogs::drawing_sheet::is_command(id) => {
                dialogs::drawing_sheet::run_command(&mut self.cx, id);
            }
            Action::Custom(id) if dialogs::watermark::is_command(id) => {
                dialogs::watermark::run_command(&mut self.cx, id);
            }
            // Customize Sheet Sizes is program-wide: one list for every plan
            // and layout, wherever the command comes from.
            Action::Layout(shell::layout_window::LayoutCommand::CustomizeSheetSizes) => {
                dialogs::drawing_sheet::open_customize();
            }
            // Zoom, Reverse Plan, Rotate Plan View, tiling and tabs.
            Action::Custom(id) if shell::view_commands::is_command(id) => self.view_command(id),
            // Edit > Replace Fonts opens the Text Styles list (Replace Fonts section).
            Action::Custom(menus::REPLACE_FONTS) => {
                self.open_defaults_entry(DefaultsEntry::TextStyles)
            }
            Action::Custom(id) => {
                // The Edit commands work on the plan, not on the 3D or layout view.
                if id.starts_with("edit.")
                    && (self.view3d.active || shell::layout_window::is_active())
                {
                    self.cx.status = "Switch to the plan view to use this command".into();
                } else {
                    self.cx.run_custom(id);
                }
            }
            Action::PlanView(i) => self.activate_plan_view(i),
            Action::Terrain(c) => self.terrain_command(c),
            Action::File(c) => dialogs::exchange::dispatch_file(&mut self.cx, c),
            Action::Framing(c) => dialogs::exchange::dispatch_framing(&mut self.cx, c),
            Action::CurrentWall => self.apply(self.toolbars.wall_action()),
            Action::FileNew => self.request_file_action(files::Pending::New),
            Action::ImportChiefPlan => self.request_file_action(files::Pending::ImportChief),
            Action::FileOpen => self.request_file_action(files::Pending::Open(None)),
            Action::FileSave => self.save_project(),
            Action::FileSaveAs => self.save_project_as(),
            Action::SaveTemplate => self.save_template(),
            Action::ResetTemplate => self.reset_template(),
            Action::ImportChiefTemplate => self.import_chief_template(),
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
                // The layout view has its own history.
                let undone = if shell::layout_window::is_active() {
                    shell::layout_window::undo(&mut self.cx)
                } else {
                    self.cx.undo()
                };
                self.cx.status = match undone {
                    Some(l) => format!("Undid {l}"),
                    None => "Nothing to undo".into(),
                };
            }
            Action::Redo => {
                let redone = if shell::layout_window::is_active() {
                    shell::layout_window::redo(&mut self.cx)
                } else {
                    self.cx.redo()
                };
                self.cx.status = match redone {
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
            // The prompt, then the window close; see `files::drive_files`.
            Action::Quit => self.request_file_action(files::Pending::Quit),
            Action::SetTheme(t) => self.settings.theme = t,
            Action::ShowAbout => {
                dialogs::app_info::run_command(&mut self.cx, dialogs::app_info::ABOUT);
            }
            Action::DefaultSettings => {
                if self.defaults_dialog.is_none() {
                    self.defaults_dialog = Some(DefaultsDialog::new());
                }
            }
            Action::BuildNewFloor
            | Action::InsertFloor
            | Action::InsertFloorBelow
            | Action::FloorDefaults
            | Action::PlanFloorDefaults
            | Action::FoundationDefaults
            | Action::ReferenceDisplayOptions
            | Action::DeleteFloor
            | Action::DeleteFoundation
            | Action::ExchangeFloorAbove
            | Action::ExchangeFloorBelow
            | Action::BuildFoundation
            | Action::RebuildAll
            | Action::SpacePlanning
            | Action::PlanCheck
            | Action::DoorWindowCheck
            | Action::PlanFootprint
            | Action::MaterialsList
            | Action::DoorSchedule
            | Action::WindowSchedule
            | Action::RoomSchedule
            | Action::WallSchedule
            | Action::CreateConstructionSet
            | Action::FileNewLayout
            | Action::FindReplaceText
            | Action::SnapSettings
            | Action::EditBehaviors
            | Action::ProjectInfo => dialogs::build_tools::dispatch(&mut self.cx, action),
            Action::OpenHotkeyDialog => self.docks.open_hotkey_dialog(&self.hotkeys),
            Action::OpenLayerDisplay => self.docks.open_layer_dialog(),
            Action::View3d(c) => {
                shell::layout_window::deactivate();
                shell::view3d_panel::dispatch(c, &mut self.cx, &mut self.tools, &mut self.view3d)
            }
            Action::Layout(c) => {
                // Send to Layout sends the open 3D view's camera, else the plan view.
                if c == shell::layout_window::LayoutCommand::ShowPlan {
                    self.view3d.active = false;
                }
                // Print Image while the 3D view shows prints that view.
                if c == shell::layout_window::LayoutCommand::PrintImage && self.view3d.active {
                    if let Some(source) = self.view3d.snapshot_source() {
                        shell::layout_window::print_image_3d(&mut self.cx, source);
                        return;
                    }
                }
                // Send to Layout from the 3D view can send a picture of it too.
                if c == shell::layout_window::LayoutCommand::SendToLayout && self.view3d.active {
                    if let Some(view) = self.view3d.snapshot_source() {
                        shell::layout_window::offer_snapshot_3d(view);
                    }
                }
                let camera = self
                    .view3d
                    .active
                    .then_some(self.view3d.active_camera)
                    .flatten();
                shell::layout_window::dispatch(c, &mut self.cx, camera);
            }
            Action::NotImplemented(name) => {
                self.cx.status = format!("Not yet implemented: {name}");
            }
        }
    }

    /// Row-1 view selector: makes a saved plan view the active one and shows
    /// its floor.
    fn activate_plan_view(&mut self, i: usize) {
        let Some(view) = self.cx.project.plan_views.get(i).cloned() else {
            return;
        };
        self.cx.begin_change("Plan View");
        self.cx.project.activate_plan_view(&view.name);
        if view.reference_display {
            self.cx
                .view_flags
                .insert(toolbar::ViewFlag::ReferenceDisplay);
        } else {
            self.cx
                .view_flags
                .remove(&toolbar::ViewFlag::ReferenceDisplay);
        }
        if let Some(f) = view.floor.filter(|f| *f < self.cx.project.floors.len()) {
            self.cx.floor = f;
        }
        if let Some((center, px)) = view.camera {
            self.camera.center = center;
            self.camera.px_per_in = px.clamp(0.05, 50.0);
        }
        // Show Color, the Selected Defaults and the rotation of the view.
        dialogs::plan_views::view_shown(&mut self.cx, &view);
        self.cx.mark_dirty();
        self.cx.status = format!("Plan view: {}", view.name);
    }

    /// Project Browser: selects the camera, shows its floor and pans the plan
    /// to it.
    fn select_camera(&mut self, id: Id) {
        let Some((floor, pos)) = self
            .cx
            .project
            .camera(id)
            .map(|c| (c.floor.min(self.cx.project.floors.len() - 1), c.position))
        else {
            return;
        };
        self.view3d.active = false;
        if floor != self.cx.floor {
            self.cx.floor = floor;
            self.reset_view_state();
        }
        self.camera.center = pos;
        self.cx.selection.set(ObjectRef::Camera(id));
        self.cx.status = "Selected the camera".into();
    }

    /// Terrain menu commands.
    fn terrain_command(&mut self, c: toolbar::TerrainCommand) {
        use toolbar::TerrainCommand as C;
        match c {
            C::Specification => {
                self.open_spec(ObjectRef::Terrain);
            }
            C::Clear => {
                if editor::site_view::load_terrain(&self.cx.project).is_some() {
                    // Only what Build Terrain generated goes (manual p. 1309):
                    // the perimeter, elevation data and objects stay.
                    editor::site_view::edit_terrain(&mut self.cx, "Clear Terrain", |rec| {
                        rec.clear_generated();
                    });
                    self.cx.status = "Cleared the generated terrain".into();
                } else {
                    self.cx.status = "There is no terrain to clear".into();
                }
            }
            C::HoleAroundBuilding => {
                if editor::site_view::auto_building_hole(&mut self.cx) {
                    self.cx.status = "Made a terrain hole around the building".into();
                }
            }
        }
    }

    // ----- file operations -----

    /// File > New Plan: a plan from the default Plan Studio template when one
    /// is set (File > Templates > Save as Template), else from the defaults.
    fn new_project(&mut self) {
        if self.new_project_from_default_template() {
            return;
        }
        self.new_project_plain();
    }

    /// A new plan from the installed defaults.
    fn new_project_plain(&mut self) {
        editor::code::seed_new_plan(&mut self.cx.defaults);
        dialogs::preferences::pages::apply_editing(&mut self.cx.defaults.editing);
        let project = Project::from_defaults("Untitled", &self.cx.defaults);
        self.cx.set_project(project);
        self.cx.seed_template_plan_views();
        self.path = None;
        self.tools.restart(&mut self.cx);
        self.files.rebaseline(&self.cx);
        self.cx.status = "New project".into();
    }

    /// File > Import > Chief Plan...: walls, openings, floors, rooms,
    /// dimensions and text from a Chief Architect `.plan` become a new,
    /// untitled project (`plan_chiefplan::import`). Chief content is read
    /// from Daniel's files at run time and never bundled.
    fn import_chief_plan(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Chief Architect plan", &["plan"])
            .pick_file()
        else {
            return;
        };
        self.import_chief_plan_from(&path);
    }

    /// Imports the Chief `.plan` at `path` as a new, untitled project. The
    /// status bar gets the one-line headline; the full summary (counts and
    /// warnings) opens in the report window.
    fn import_chief_plan_from(&mut self, path: &std::path::Path) {
        let opts = plan_chiefplan::import::ImportOptions {
            symbol_resolver: Some(chief_link::resolve_symbol),
            ..Default::default()
        };
        match plan_chiefplan::import::import_plan(path, &opts) {
            Ok(res) => self.apply_chief_import(res),
            Err(e) => {
                self.cx.status = format!("Could not import {}: {e}", path.display());
            }
        }
    }

    /// Makes an imported Chief plan the open (untitled) project.
    fn apply_chief_import(&mut self, res: plan_chiefplan::import::ImportResult) {
        let headline = res.report.headline();
        let summary = res.report.summary();
        self.cx.set_project(res.project);
        self.path = None;
        self.tools.restart(&mut self.cx);
        self.files.rebaseline(&self.cx);
        self.cx.status = format!("Imported {headline}");
        dialogs::plan_check::open_text_report("Chief Plan Import", &headline, &summary);
    }

    // Open, save, save as, revert, backup and the prompts live in `files.rs`.

    /// File > Templates > Save Current Defaults as My Template.
    fn save_template(&mut self) {
        self.cx.status = match plan_defaults::save_user(&self.cx.defaults) {
            Ok(path) => format!("Saved your template to {}", path.display()),
            Err(e) => format!("Could not save the template: {e}"),
        };
    }

    /// File > Templates > Import Chief Template...: picks a Chief `.plan` /
    /// `.tpl` / `.layout`, then the Import Chief Template window shows what
    /// was decoded and offers to import it or make it the default template.
    fn import_chief_template(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Chief Architect plan or template",
                &["plan", "tpl", "layout"],
            )
            .pick_file()
        else {
            return;
        };
        dialogs::exchange::open_chief_template(&mut self.cx, &path);
    }

    /// File > Templates > Reset to Chief X18 Template.
    fn reset_template(&mut self) {
        // The embedded template, seeded from the Chief plan template when
        // that is set up.
        self.cx.defaults = plan_defaults::template_base().0;
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
        self.process_requests();
    }

    /// Runs what tools and Edit toolbar commands queued on the context.
    fn process_requests(&mut self) {
        for req in std::mem::take(&mut self.cx.requests) {
            match req {
                EditorRequest::OpenSpec(o) => self.open_spec(o),
                EditorRequest::PanPixels(d) => self.camera.pan_by_pixels(d),
                EditorRequest::SetTool(t) => self.set_tool(t),
            }
        }
        // The double-click on a Door or Window Tools button asks for the
        // Defaults dialog of its type (manual p. 603).
        if let Some(key) = dialogs::take_type_defaults_request() {
            use plan_core::openings::types::DefaultKey;
            use plan_core::{OpeningKind, OpeningStyle};
            // The plain Door and Window keep the dialogs they always had.
            let hinged = |exterior| DefaultKey::new(OpeningKind::Door, OpeningStyle::Hinged, exterior);
            self.open_defaults_entry(if key == DefaultKey::main_window() {
                DefaultsEntry::Window
            } else if key == hinged(false) {
                DefaultsEntry::InteriorDoor
            } else if key == hinged(true) {
                DefaultsEntry::ExteriorDoor
            } else {
                DefaultsEntry::OpeningType(key)
            });
        }
    }

    /// Opens the specification dialog of `o` (the one place that maps every
    /// object kind to its dialog).
    fn open_spec(&mut self, o: ObjectRef) {
        // A temporary point opens Move Point (CAD-112).
        if let ObjectRef::Cad(id) = o {
            if let Some(p) = tools::cad::survey::temporary_point_of(&self.cx, id) {
                dialogs::move_point::open(p);
                return;
            }
        }
        match o {
            ObjectRef::Wall(id) => {
                // Open Object over a selection of walls: one dialog for all
                // of them (W-83).
                let many = shell::spec_dialogs::SpecDialogs::selected_walls(&self.cx)
                    .filter(|ids| ids.contains(&id));
                match many {
                    Some(ids) if self.spec.open_walls(&mut self.cx, &ids) => {}
                    _ => self.open_wall_dialog(id),
                }
            }
            ObjectRef::Opening(id) => self.open_opening_dialog(id),
            other => {
                if !self.spec.open(&mut self.cx, other) {
                    self.cx.status =
                        format!("{} specification: nothing to open", other.type_name());
                }
            }
        }
    }

    fn send_key(&mut self, ctx: &egui::Context, k: KeyEvent) {
        let is_esc = k.is(egui::Key::Escape);
        let is_del = k.is(egui::Key::Delete) || k.is(egui::Key::Backspace);
        if shell::layout_window::is_active() {
            // The layout view reads its own keys.
            return;
        }
        if is_del && self.view3d.active {
            // The 3D view has no selection to delete.
            return;
        }
        let arrow = [
            egui::Key::ArrowUp,
            egui::Key::ArrowDown,
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
        ]
        .into_iter()
        .any(|a| k.is(a));
        if arrow && self.view3d.active {
            // The 3D view reads the arrows itself: they nudge the selection
            // along the axes as seen from the camera, or walk Full Camera.
            return;
        }
        // Tab or Enter with nothing typed, while a start location exists:
        // ask for the new location by number (manual p. 196).
        let origin = self.tools.active().coordinate_origin(&self.cx);
        if dialogs::enter_coordinates::try_open(&self.cx, origin, &k) {
            return;
        }
        let res = self.tools.active_mut().key(&mut self.cx, k);
        self.finish_tool_call(ctx, &res);
        if !res.consumed {
            if is_esc && self.tools.active_id() != ToolId::Select {
                self.set_tool(ToolId::Select);
            } else if is_del {
                // With nothing selected, Delete removes the latest temporary
                // point (the Current Point).
                if !tools::cad::survey::delete_key(&mut self.cx) {
                    self.cx.delete_selection();
                }
            }
        }
    }

    /// Applies hotkeys (see `toolbar::BINDINGS`) and forwards editing keys to
    /// the active tool.
    fn handle_keys(&mut self, ctx: &egui::Context, actions: &mut Vec<Action>) {
        if self.has_dialog()
            || self.defaults_dialog.is_some()
            || self.docks.modal_open()
            || self.files.modal_open()
        {
            return;
        }
        let editing = self.cx.temp.editing.is_some();
        if !editing {
            shell::hotkeys::handle(ctx, &self.cx, &mut self.hotkeys, actions);
        }
        if ctx.wants_keyboard_input() {
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        // A toolbar button or dock control has the focus: Tab, Enter and the
        // arrows move and press it instead of reaching the active tool.
        let chrome_focus = shell::docks::focus_in_chrome(ctx);
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
                    // Key 1 clears the extension anchors.
                    if key == Num1 && !modifiers.any() && !self.cx.typed_input.is_armed() {
                        editor::snap::clear_anchors();
                    }
                    let arrow = matches!(key, ArrowLeft | ArrowRight | ArrowUp | ArrowDown);
                    let navigating = chrome_focus && (arrow || matches!(key, Tab | Enter));
                    let wanted = matches!(key, Escape | Delete | Backspace | Tab | Enter) || arrow;
                    let wanted = wanted && !navigating;
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
                egui::Event::Text(t) if editing || self.cx.typed_input.is_armed() => {
                    self.send_key(ctx, KeyEvent::text(&t));
                }
                _ => {}
            }
        }
    }

    fn canvas_event(&self, ctx: &egui::Context, pos: Pos2, down: bool) -> PointerEvent {
        let (mods, delta) = ctx.input(|i| (i.modifiers, i.pointer.delta()));
        // Shift slows the pointer while a click-and-drag drawing is under way
        // (manual p. 195); Select Objects keeps Shift for its marquee.
        let slow = mods.shift && down && self.tools.active_id() != ToolId::Select;
        let pos = editor::snap::slow_pointer(pos, slow);
        // Keys that change how the pointer is read while they are held:
        // Shift restricts the angle snaps, S drops the object snaps, and the
        // summon keys call up an edit behavior (manual pp. 192, 193, 253).
        let s_key = ctx.input(|i| {
            i.key_down(egui::Key::S)
                && !i.modifiers.command
                && !i.modifiers.ctrl
                && !i.modifiers.alt
        });
        editor::snap::set_held(editor::snap::HeldKeys {
            shift: mods.shift,
            s_key,
        });
        editor::behaviors::note_held_summon(ctx.input(editor::behaviors::held_summon));
        let world = self.camera.screen_to_world(pos);
        let snap = self
            .cx
            .snap_at(world, None, editor::snap::overrides(&mods), &[]);
        if !down {
            // Resting on an endpoint, midpoint or quadrant sets an extension
            // anchor.
            editor::snap::note_hover(&snap, self.cx.defaults.editing.anchor_history as usize);
        }
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
        if self.has_dialog() {
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
        if resp.clicked_by(egui::PointerButton::Secondary) && editor::transform::mode_active() {
            // A hanging paste or a click-driven edit mode ends on a right click.
            editor::transform::cancel_mode(&mut self.cx);
        } else if self.has_canvas_menu() {
            // The context menu (S-8, DW-109): the object under the pointer is
            // selected first, then the menu opens on it.
            if resp.secondary_clicked() {
                self.select_for_context_menu(resp);
            }
            self.canvas_context_menu(ctx, resp);
        } else if resp.clicked_by(egui::PointerButton::Secondary) {
            // Right-click is the tool's Esc without leaving the tool (the
            // wall tool keeps its chain, W-3).
            let res = self.tools.active_mut().secondary_click(&mut self.cx);
            self.finish_tool_call(ctx, &res);
        }
    }

    /// Does the active tool show the right-click menu? Select Objects does;
    /// so do the door and window tools, whose menu starts with Select Objects
    /// (DW-109). The drawing tools keep right click as their Esc.
    fn has_canvas_menu(&self) -> bool {
        matches!(
            self.tools.active_id().base(),
            ToolId::Select | ToolId::Door | ToolId::Window
        )
    }

    /// Right click in Select Objects: the object under the pointer becomes
    /// the selection (a group member selects its group); empty space clears it.
    fn select_for_context_menu(&mut self, resp: &egui::Response) {
        if self.tools.active_id().base() != ToolId::Select {
            return;
        }
        let Some(pos) = resp.interact_pointer_pos() else {
            return;
        };
        let world = self.camera.screen_to_world(pos);
        // A wall's notification icon opens that wall's menu (W-132).
        if editor::wall_edit::select_for_icon(&mut self.cx, world) {
            return;
        }
        let hits = editor::selection::hit_test_cx(&self.cx, world, self.cx.pick_tol());
        match hits.into_iter().find(|o| !matches!(o, ObjectRef::Room(_))) {
            Some(o) => {
                if !self.cx.selection.contains(o) {
                    editor::rooms_edit::clear_room_selection();
                    self.cx.selection.items = editor::selection::expand_groups(&self.cx, &[o]);
                }
            }
            None => self.cx.selection.clear(),
        }
    }

    /// The right-click menu of the canvas: the entries come from
    /// `EditorContext::context_entries` and run as ordinary actions.
    fn canvas_context_menu(&mut self, ctx: &egui::Context, resp: &egui::Response) {
        let ghost = self.tools.active_id().base() != ToolId::Select;
        let toolbar = self.tools.active().edit_toolbar(&self.cx);
        let entries = self.cx.context_entries(&toolbar, ghost);
        let mut chosen: Option<Action> = None;
        resp.context_menu(|ui| {
            for e in &entries {
                if e.sep_before {
                    ui.separator();
                }
                if ui
                    .add_enabled(e.enabled, egui::Button::new(e.label.as_str()))
                    .clicked()
                {
                    chosen = Some(e.action);
                    ui.close_menu();
                }
            }
        });
        if let Some(action) = chosen {
            self.apply(action);
            self.process_requests();
            ctx.request_repaint();
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
                    &self.hotkeys.map,
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
        // The toolbar set follows the view: plan, 3D, vector elevation or layout.
        toolbar::config::set_active_view(if self.view3d.active {
            if shell::view3d_panel::is_vector_technique(self.view3d.technique) {
                toolbar::config::ViewKind::Elevation
            } else {
                toolbar::config::ViewKind::View3d
            }
        } else {
            toolbar::config::ViewKind::Plan
        });
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
                            &self.hotkeys.map,
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
                            &self.hotkeys.map,
                        );
                        actions.extend(toolbar::column(ui, &mut self.toolbars.view, &state));
                    });
            });
    }

    fn dock_panel(&mut self, ctx: &egui::Context) {
        let Some(dock) = self.dock else { return };
        shell::docks::panel(ctx, dock, &mut self.cx, &mut self.docks, &mut self.settings);
        for req in std::mem::take(&mut self.docks.requests) {
            match req {
                shell::docks::DockRequest::SetTool(t) => {
                    self.view3d.active = false;
                    self.set_tool(t);
                }
                shell::docks::DockRequest::SwitchFloor(n) if n < self.cx.project.floors.len() => {
                    self.cx.floor = n;
                    self.reset_view_state();
                }
                shell::docks::DockRequest::SwitchFloor(_) => {}
                shell::docks::DockRequest::SelectCamera(id) => self.select_camera(id),
                shell::docks::DockRequest::PanTo(at) => self.camera.center = at,
                shell::docks::DockRequest::ActivatePlanView(i) => self.activate_plan_view(i),
                shell::docks::DockRequest::Run(a) => self.apply(a),
            }
        }
    }

    fn properties_panel(&mut self, ctx: &egui::Context) {
        let panel = egui::SidePanel::left("properties")
            .default_width(self.settings.dock_widths.properties)
            .width_range(theme::DOCK_WIDTH_MIN..=theme::DOCK_WIDTH_MAX)
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
                    ui.label(room_line(&self.cx.room_name(room), room));
                }
                ui.separator();
                self.selected_wall_section(ui);
            });
        shell::docks::remember_width(
            &mut self.settings.dock_widths.properties,
            panel.response.rect.width(),
        );
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
        let type_line = wall_type_line(wall, self.cx.wall_types());
        let fmt = self.cx.dim_format();
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
        ui.label(type_line);
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
        let frame = egui::Frame::side_top_panel(&ctx.style()).fill(theme::scale(
            theme::current_chrome().status,
            self.settings.brightness,
        ));
        let fields = shell::status::StatusFields {
            cursor: self
                .cx
                .cursor_world
                .map(|p| (self.cx.fmt_dim(p.x), self.cx.fmt_dim(p.y))),
            pending_prefix: self.hotkeys.pending_label(),
            readout: self.cx.readout.clone(),
            snap: self
                .cx
                .last_snap
                .filter(|s| s.kind.is_object_snap())
                .map(|s| s.kind.label().to_string()),
            hint: self.tools.active().hint(),
            message: self.cx.status.clone(),
            floor: self.cx.floor().name.clone(),
            layer_set: self.cx.project.layer_sets.active.clone(),
            zoom: shell::status::zoom_label(self.camera.px_per_in),
            undo: self.cx.undo_label().map(str::to_string),
            saved: self.saved_status(std::time::Instant::now()),
            // Z, the selection text, the hover text and the Edit Behavior.
            ..shell::status::context_fields(
                &self.cx,
                self.tools.active_id().base() == ToolId::Select,
            )
        };
        egui::TopBottomPanel::bottom("status")
            .frame(frame)
            .show(ctx, |ui| {
                shell::status::show(ui, &fields, &mut self.settings);
            });
    }

    // ----- canvas -----

    fn canvas(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let (resp, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        self.camera.rect = resp.rect;
        self.cx.px_per_in = self.camera.px_per_in;
        self.cx.palette = self.settings.theme.palette();
        dialogs::preferences::tint_palette(&mut self.cx.palette);

        self.handle_camera_input(ui, &resp);

        let new_cursor = resp.hover_pos().map(|p| self.camera.screen_to_world(p));
        if new_cursor.map(|p| (p.x, p.y)) != self.cx.cursor_world.map(|p| (p.x, p.y)) {
            ctx.request_repaint();
        }
        self.cx.cursor_world = new_cursor;
        self.cx.px_per_in = self.camera.px_per_in;
        self.auto_scroll(ctx);
        // The rubber-band Zoom (Window > Zoom) takes the pointer while armed.
        let zooming = self.shell_views.zoom_armed;
        // The Drawing Sheet is an object: its border moves it, its corners
        // resize it (File > Print, View > Drawing Sheet).
        let sheet_took = !zooming
            && !self.has_dialog()
            && dialogs::drawing_sheet::pointer(
                ctx,
                &resp,
                &mut self.cx,
                &self.camera,
                self.tools.active_id().base() == ToolId::Select,
            );
        if !zooming && !sheet_took {
            self.dispatch_pointer(ctx, &resp);
        }
        if resp.hovered() && !zooming {
            ui.ctx().set_cursor_icon(self.tools.active().cursor());
            // The name of the object under the pointer, after a short rest.
            if self.tools.active_id().base() == ToolId::Select && !self.pressed_in_canvas {
                if let Some(text) = shell::status::hover_tooltip(&self.cx) {
                    resp.clone().on_hover_text_at_pointer(text);
                }
            }
        }

        self.cx.refresh();
        // The editor's sheet follows the Drawing Sheet Setup; the Print dialog
        // reads the active view's setup and what is on screen.
        dialogs::drawing_sheet::sync(&mut self.cx, &self.camera);
        render::draw_plan(&self.cx, &painter, &self.camera);
        // The Watermark, the printable-area border and the sheet's handles.
        dialogs::drawing_sheet::paint_overlays(&self.cx, &painter, &self.camera);
        // The Off Angle and unconnected-wall icons (W-132).
        editor::wall_edit::draw_icons(&self.cx, &painter, &self.camera);
        self.tools
            .active()
            .draw_overlay(&self.cx, &painter, &self.camera);
        render::draw_crosshairs(&self.cx, &painter, &self.camera);
        self.zoom_window_input(ctx, &resp, &painter);
    }

    /// A Select Objects drag whose pointer reaches the edge of the canvas
    /// scrolls the view that way (S-99).
    fn auto_scroll(&mut self, ctx: &egui::Context) {
        if !self.pressed_in_canvas || !tools::select::drag_in_progress() {
            return;
        }
        let (pos, dt) = ctx.input(|i| (i.pointer.latest_pos(), i.stable_dt.min(0.05)));
        let Some(pos) = pos else { return };
        let v = tools::select::auto_scroll_vector(self.camera.rect, pos, dt);
        if v != Vec2::ZERO {
            self.camera.pan_by_pixels(v);
            ctx.request_repaint();
        }
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
        if self.has_dialog() || self.view3d.active {
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
            // Anchored by its bottom-left corner, so a second row grows upward.
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(Pos2::new(rect.left() + 10.0, rect.bottom() - 8.0))
            .show(ctx, |ui| {
                ui.set_max_width((rect.width() - 24.0).max(200.0));
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    // The buttons wrap when the object has many commands.
                    ui.horizontal_wrapped(|ui| {
                        for a in &actions {
                            let btn = match a.icon {
                                Some(id) => egui::Button::image(
                                    egui::Image::new(icons::icon(id))
                                        .fit_to_exact_size(Vec2::splat(18.0))
                                        .tint(Color32::WHITE),
                                ),
                                None => egui::Button::new(a.label),
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
            self.process_requests();
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
        let mut dialog = WallDialog::new(
            WallTarget::Wall(id),
            wall,
            openings,
            extras,
            default_height,
            self.cx.wall_types().to_vec(),
        );
        dialog.set_framing_retained(&[editor::framing_view::wall_retained(&self.cx.project, id)]);
        dialog.set_attic_above(editor::roof_view::attic_wall_above(
            &self.cx.project,
            self.cx.floor,
            id,
        ));
        self.dialog = Some(ActiveDialog::Wall(Box::new(dialog)));
        self.spec.arm_main_props(&self.cx, ObjectRef::Wall(id));
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
        let layers: Vec<String> = self
            .cx
            .project
            .layers
            .layers
            .iter()
            .map(|l| l.name.clone())
            .collect();
        let dialog = OpeningDialog::for_opening(opening, &wall, others, extras)
            .with_label_defaults(&self.cx.defaults.opening_labels)
            .with_layer_choices(layers);
        self.dialog = Some(ActiveDialog::Opening(Box::new(dialog)));
        self.spec.arm_main_props(&self.cx, ObjectRef::Opening(id));
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
            ActiveDialog::Opening(Box::new(
                OpeningDialog::for_default(target, template, app.cx.default_opening_extras(target))
                    .with_label_defaults(&app.cx.defaults.opening_labels),
            ))
        };
        // A default dialog has no object, so no Properties tab.
        self.spec.take_main_props();
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
            DefaultsEntry::OpeningType(key) => {
                // The opening the type places now: its own default where one
                // was set, else the plan default sized for the type.
                let base = if key.kind == plan_core::OpeningKind::Window {
                    plan_defaults::window_template(&self.cx.defaults)
                } else {
                    plan_defaults::door_template(&self.cx.defaults, key.exterior)
                };
                let template =
                    self.cx
                        .defaults
                        .opening_variants
                        .place(&base, key.style, key.exterior);
                Some(opening_dialog(OpeningTarget::DefaultType(key), template, self))
            }
            DefaultsEntry::Dimensions => {
                self.lists = Some(dialogs::DefaultsList::dimensions(&self.cx));
                None
            }
            DefaultsEntry::RoomTypes => {
                self.lists = Some(dialogs::DefaultsList::room_types(&self.cx));
                None
            }
            DefaultsEntry::FloorDefaults => {
                dialogs::build_tools::dispatch(&mut self.cx, toolbar::Action::PlanFloorDefaults);
                None
            }
            DefaultsEntry::Foundation => {
                dialogs::build_tools::dispatch(&mut self.cx, toolbar::Action::FoundationDefaults);
                None
            }
            DefaultsEntry::TextStyles => {
                self.lists = Some(dialogs::DefaultsList::text_styles(&self.cx));
                None
            }
            DefaultsEntry::Templates => {
                dialogs::exchange::open_templates_page();
                None
            }
        };
    }

    /// Shows the open dialogs and applies an OK.
    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(mut dialog) = self.dialog.take() {
            // The Properties tab of the plan's own walls and openings.
            let props = self.spec.main_props().cloned();
            let prev_info = dialogs::object_info::set_current(self.spec.main_info().cloned());
            let outcome =
                dialogs::property_manager::with_current(props.as_ref(), || match &mut dialog {
                    ActiveDialog::Wall(d) => d.show(ctx),
                    ActiveDialog::Opening(d) => d.show(ctx),
                });
            dialogs::object_info::set_current(prev_info);
            match outcome {
                Outcome::Open => self.dialog = Some(dialog),
                Outcome::Cancel => {
                    self.spec.take_main_props();
                }
                Outcome::Ok => {
                    let info = self.spec.take_main_info();
                    let props = self.spec.take_main_props();
                    let depth = dialogs::property_manager::before_apply(&self.cx);
                    // One step, and none when OK changed nothing (QA-26).
                    self.cx.begin_undo_group();
                    match &dialog {
                        ActiveDialog::Wall(d) => self.apply_wall_dialog(d),
                        ActiveDialog::Opening(d) => self.apply_opening_dialog(d),
                    }
                    self.cx.end_undo_group();
                    dialogs::property_manager::after_apply(&mut self.cx, props.as_ref(), depth);
                    dialogs::object_info::after_apply(&mut self.cx, info.as_ref(), depth);
                    self.cx.mark_dirty();
                }
            }
        }
        self.spec.show(ctx, &mut self.cx);
        if let Some(mut list) = self.lists.take() {
            if list.show(ctx, &mut self.cx) {
                self.lists = Some(list);
            }
        }
        if let Some(mut defaults) = self.defaults_dialog.take() {
            match defaults.show(ctx, self.dialog.is_none() && self.lists.is_none()) {
                DefaultsOutcome::Open => {}
                DefaultsOutcome::Edit(entry) => self.open_defaults_entry(entry),
                DefaultsOutcome::Run(action) => self.apply(action),
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
                let orig_wall = orig.clone();
                let orig_materials = orig.as_ref().map(|w| w.spec.materials.clone());
                let orig_offsets: Vec<(plan_core::Id, f64)> = self
                    .cx
                    .floor()
                    .openings
                    .iter()
                    .map(|o| (o.id, o.center_offset))
                    .collect();
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
                // Generate Between Platforms may have been switched, and the
                // Materials tab hands its paint to the project.
                self.cx.project.sync_platform_walls();
                if let Some(orig) = &orig_materials {
                    self.cx.project.sync_wall_materials_from(id, orig);
                }
                // Retain Wall Framing lives in the plan's framing settings.
                let retain_changed = d.retain_framing_change().is_some_and(|v| {
                    editor::framing_view::retain_walls_in(&mut self.cx.project, &[id], v) > 0
                });
                // OK with nothing edited leaves no undo step.
                let same_wall = match (&orig_wall, self.cx.floor().wall(id)) {
                    (Some(a), Some(b)) => {
                        plan_core::walls_equal(std::slice::from_ref(a), std::slice::from_ref(b))
                    }
                    _ => false,
                };
                if same_wall
                    && self
                        .cx
                        .floor()
                        .openings
                        .iter()
                        .map(|o| (o.id, o.center_offset))
                        .collect::<Vec<_>>()
                        == orig_offsets
                    && d.edited_types().is_empty()
                    && !retain_changed
                {
                    self.cx.cancel_change();
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
                // The Materials tab reaches the per-object paint too.
                self.cx.project.sync_opening_materials(id);
            }
            OpeningTarget::DefaultDoor => {
                let base = &self.cx.defaults.interior_door;
                self.cx.defaults.interior_door = d.extras().to_door_defaults(&draft, base, false);
                self.cx.defaults.opening_labels.door = d.label_settings().clone();
                d.apply_to_variants(&mut self.cx.defaults.opening_variants);
            }
            OpeningTarget::DefaultExteriorDoor => {
                let base = &self.cx.defaults.exterior_door;
                self.cx.defaults.exterior_door = d.extras().to_door_defaults(&draft, base, true);
                self.cx.defaults.opening_labels.door = d.label_settings().clone();
                d.apply_to_variants(&mut self.cx.defaults.opening_variants);
            }
            OpeningTarget::DefaultWindow => {
                let base = &self.cx.defaults.window;
                self.cx.defaults.window = d.extras().to_window_defaults(&draft, base);
                self.cx.defaults.opening_labels.window = d.label_settings().clone();
                d.apply_to_variants(&mut self.cx.defaults.opening_variants);
            }
            // A type of its own: the default of that type only, and the
            // openings using it follow (Minimum Separation and the Mulled
            // Unit Defaults are the plan's, kept with the window defaults).
            OpeningTarget::DefaultType(key) => {
                if key.kind == plan_core::OpeningKind::Window {
                    let base = &self.cx.defaults.window;
                    let w = d.extras().to_window_defaults(&draft, base);
                    self.cx.defaults.window.min_separation = w.min_separation;
                    self.cx.defaults.window.ignore_casing = w.ignore_casing;
                    self.cx.defaults.window.mulled = w.mulled;
                }
                d.apply_to_variants(&mut self.cx.defaults.opening_variants);
            }
        }
        self.cx
            .extras
            .openings
            .insert(d.target().key(), d.extras().clone());
    }
}

impl PlanApp {
    /// Rebuilds the egui visuals when the theme, brightness, text size or
    /// motion setting changed, keeps the UI scale and egui's zoom in step, and
    /// writes the settings file once a change has settled (not while a slider
    /// is dragged).
    fn sync_settings(&mut self, ctx: &egui::Context) {
        theme::sync_chrome(ctx, &self.settings);
        theme::sync_scale(ctx, &mut self.settings);
        if self.settings != self.saved_settings && !ctx.input(|i| i.pointer.any_down()) {
            match self.settings.save() {
                Ok(()) => {}
                Err(e) => self.cx.status = format!("Could not save settings: {e}"),
            }
            self.saved_settings = self.settings;
        }
    }
}

impl eframe::App for PlanApp {
    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        editor::behaviors::restore_default(&mut self.cx);
        self.files.on_exit(&self.cx);
        if let (Some(gl), Some(vp)) = (gl, self.view3d.viewport.as_mut()) {
            vp.destroy(gl);
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.cx.refresh();
        // The Enter Coordinates dialog answers as a typed length and angle,
        // which the tool commits on Enter.
        if let Some(dialogs::enter_coordinates::Outcome::Ok { length, angle_deg }) =
            dialogs::enter_coordinates::show(ctx)
        {
            dialogs::enter_coordinates::deliver(&mut self.cx, length, angle_deg);
            self.send_key(ctx, KeyEvent::key(egui::Key::Enter));
        }
        // The survey entry dialogs: Number Style, New CAD Line / Arc and Move
        // Point (`tools::cad::survey`).
        dialogs::number_style::show(ctx, &mut self.cx);
        dialogs::input_line::show(ctx, &mut self.cx);
        dialogs::input_arc::show(ctx, &mut self.cx);
        dialogs::move_point::show(ctx, &mut self.cx);
        // Preferences > Architectural > Auto Rebuild Roofs is the global switch;
        // each roof also has its own.
        if dialogs::preferences::pages::current()
            .architectural
            .auto_rebuild_roofs
            && editor::roof_view::auto_rebuild(&mut self.cx)
        {
            self.cx.refresh();
        }
        // Residential-template behaviour (RF-164, off by default): closing the
        // exterior walls builds the roof.
        if dialogs::preferences::pages::current()
            .architectural
            .build_roof_when_room_closes
            && editor::roof_view::build_when_room_closes(&mut self.cx)
        {
            self.cx.refresh();
        }
        // Framing groups with Auto rebuild on follow the walls (it refreshes
        // the context itself when it rebuilds).
        editor::framing_view::auto_rebuild(&mut self.cx);
        // Auto Rebuild Foundation follows Floor 1; the Attic floor warns when
        // walls or objects are drawn on it (Round 16, brief 17).
        editor::foundation_view::frame(&mut self.cx);
        // Code minimums for the dialogs and the live Plan Check count.
        editor::code::frame(&mut self.cx);
        if !ctx.input(|i| i.pointer.any_down()) {
            self.cx.end_merge();
        }
        let mut actions = Vec::new();
        self.handle_keys(ctx, &mut actions);
        // Refresh Display.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F5)) {
            actions.push(Action::Custom(dialogs::app_info::REFRESH));
        }
        // Select Next/Previous Tab (Ctrl+Tab) and Swap Views (F7).
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
                egui::Key::Tab,
            )
        }) {
            actions.push(Action::Custom(shell::view_commands::PREVIOUS_TAB));
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, egui::Key::Tab)) {
            actions.push(Action::Custom(shell::view_commands::NEXT_TAB));
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F7)) {
            actions.push(Action::Custom(shell::view_commands::SWAP_VIEWS));
        }
        // F6: the keyboard focus into the dock, or back out to the canvas.
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F6)) {
            shell::docks::toggle_focus_region(ctx, self.dock.is_some());
        }
        // Preferences... (Cmd+, on macOS, Ctrl+, elsewhere).
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Comma)) {
            actions.push(Action::Custom(dialogs::preferences::OPEN));
        }
        self.menu_bar(ctx, &mut actions);
        if dialogs::preferences::show_toolbars() {
            self.toolbar_rows(ctx, &mut actions);
        }
        if dialogs::preferences::show_status_bar() {
            self.status_bar(ctx);
        }
        self.properties_panel(ctx);
        self.view_bar(ctx, &mut actions);
        if !actions.is_empty() {
            ctx.request_repaint();
        }
        for action in actions {
            self.apply(action);
        }
        // The Plan Agent works whether or not its dock is open; a finished
        // run replaces the plan as one undo step.
        if shell::agent_panel::pump(&mut self.docks.agent, &mut self.cx, ctx) {
            self.tools.restart(&mut self.cx);
        }
        self.dock_panel(ctx);
        self.tools.frame(&mut self.cx, ctx);
        self.process_requests();
        // The plan-view tabs are a panel of their own above the drawing area.
        shell::docks::plan_view_tab_strip(ctx, &mut self.cx);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                shell::docks::set_central_rect(ctx, ui.max_rect());
                if shell::layout_window::is_active() {
                    shell::layout_window::show_central(ctx, ui, &mut self.cx);
                } else if self.tiled_central(ctx, ui) {
                    // Window > Tile: the plan and the 3D view share the window.
                } else if self.view3d.frame(ctx, &mut self.cx) {
                    // Dragging a selected object in the 3D view needs the
                    // Select tool.
                    self.view3d.select_tool = self.tools.active_id().base() == ToolId::Select;
                    shell::view3d_panel::show(ui, &mut self.cx, &mut self.view3d);
                } else {
                    self.canvas(ctx, ui);
                }
            });
        self.edit_toolbar(ctx);
        self.dialogs(ctx);
        // The plan-view tabs read the camera before the dialogs run and may
        // ask for another one (a tab switch, Reset Plan View).
        editor::plan_tabs::report_camera(self.camera.center, self.camera.px_per_in);
        shell::view_commands::report_rotation(self.camera.rotation);
        shell::view_commands::show_rotate_dialog(ctx, &mut self.cx);
        shell::docks::show_dialogs(ctx, &mut self.cx, &mut self.docks, &mut self.hotkeys);
        if let Some((center, zoom)) = editor::plan_tabs::take_pending_camera() {
            self.camera.center = center;
            self.camera.px_per_in = zoom.clamp(0.05, 50.0);
        }
        // A saved plan view's rotation, or the Rotate Plan View dialog's.
        if let Some(rotation) = shell::view_commands::take_pending_rotation() {
            self.camera.rotation = rotation;
        }
        // New Plan from Template, Save as Template and the missing-template
        // prompt answer here.
        self.poll_template_requests();
        dialogs::build_tools::show_all(ctx, &mut self.cx, &mut self.camera);
        dialogs::property_manager::show_all(ctx, &mut self.cx, self.path.as_deref());
        shell::layout_window::show_dialogs(ctx, &mut self.cx);
        dialogs::transform::show_edit_windows(ctx, &mut self.cx);
        // Copy and Cut leave a note on the system clipboard: egui only sends
        // a Paste event for Cmd+V while that holds some text.
        if editor::clipboard::take_system_clipboard_note() {
            ctx.copy_text(editor::clipboard::SYSTEM_NOTE.to_string());
        }
        dialogs::exchange::show_all(ctx, &mut self.cx);
        dialogs::underlay::show_all(ctx, &mut self.cx);
        dialogs::drawing_sheet::show_all(ctx, &mut self.cx);
        tools::materials::show_windows(ctx, &mut self.cx);
        dialogs::spell_check::show_all(ctx, &mut self.cx);
        let mut pref_actions = Vec::new();
        dialogs::preferences::show_all(ctx, &mut self.cx, &mut self.settings, &mut pref_actions);
        for action in pref_actions {
            self.apply(action);
        }
        self.poll_template_detection(ctx);
        self.app_commands(ctx);
        self.drive_files(ctx);
        fonts::post_notes(&mut self.cx.status);
        shell::status::record(&self.cx.status);
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

/// One Rooms-list row: the name and the interior area (what the plan label
/// shows), not the area to the wall centerlines.
fn room_line(name: &str, room: &plan_core::Room) -> String {
    format!("{name}  -  {} sq ft", room.interior_area_sq_ft().round())
}

/// The Properties line for a wall's type: its name and the thickness the
/// type defines, noting when the wall has been resized away from it.
fn wall_type_line(wall: &plan_core::Wall, types: &[plan_core::WallTypeDef]) -> String {
    let def = wall
        .wall_type
        .as_deref()
        .and_then(|n| types.iter().find(|t| t.name == n));
    match (wall.wall_type.as_deref(), def) {
        (Some(name), Some(t)) if (t.thickness() - wall.thickness).abs() < 1e-6 => {
            format!("Type: {name} ({})", dialogs::fmt_short(t.thickness()))
        }
        (Some(name), Some(t)) => format!(
            "Type: {name} ({}; this wall {})",
            dialogs::fmt_short(t.thickness()),
            dialogs::fmt_short(wall.thickness)
        ),
        (Some(name), None) => format!("Type: {name}"),
        (None, _) => format!("Type: none ({})", dialogs::fmt_short(wall.thickness)),
    }
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
    files::install_panic_hook();
    // Finder's "open document" event (a double-click on a .psplan).
    #[cfg(target_os = "macos")]
    mac_open::install();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_title("Plan Studio")
            .with_icon(dialogs::app_info::window_icon()),
        depth_buffer: 24,
        ..Default::default()
    };
    eframe::run_native(
        "Plan Studio",
        options,
        Box::new(|cc| {
            icons::install(&cc.egui_ctx);
            let mut settings = AppSettings::load();
            theme::apply_settings(&cc.egui_ctx, &settings);
            // Nothing here waits for the disk scan of a first launch: the
            // templates are looked for on a thread (`PlanApp::update` picks
            // the result up) and the window opens on the shipped defaults.
            let (defaults, mut note) = plan_defaults::load();
            if templates::begin_detection() && note.is_none() {
                note = Some("Looking for your Chief templates in the background...".into());
            }
            dialogs::preferences::apply_startup(&cc.egui_ctx);
            theme::sync_scale(&cc.egui_ctx, &mut settings);
            let mut app = PlanApp::new(settings, defaults, note);
            // A plan named on the command line (a double-click on Windows and
            // Linux) opens at the first frame; so does the recovery offer.
            app.files.begin_startup(std::env::args_os().skip(1));
            Ok(Box::new(app))
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

    /// A plan with one object of every kind; returns the refs.
    fn synthetic_plan(a: &mut PlanApp) -> Vec<ObjectRef> {
        use editor::{placed, roof_view, site_view, stairs_view};
        use plan_cabinets::CabinetKind;
        use plan_core::cad::CadItem;
        use plan_core::{CameraKind, CameraObject, Dimension, DimensionKind, PlacedSymbol};
        let c = [
            Point::new(0.0, 0.0),
            Point::new(480.0, 0.0),
            Point::new(480.0, 288.0),
            Point::new(0.0, 288.0),
        ];
        let mut walls = Vec::new();
        for i in 0..4 {
            walls.push(a.cx.project.add_wall(
                0,
                c[i],
                c[(i + 1) % 4],
                6.0,
                109.0,
                WallKind::Exterior,
            ));
        }
        let mut refs = vec![ObjectRef::Wall(walls[0])];
        let door =
            a.cx.project
                .add_opening(0, walls[0], 100.0, OpeningKind::Door)
                .unwrap();
        refs.push(ObjectRef::Opening(door));
        refs.push(ObjectRef::Dimension(a.cx.project.add_dimension(
            0,
            Dimension::new(0, DimensionKind::Manual, c[0], c[1], 24.0),
        )));
        refs.push(ObjectRef::Cad(a.cx.project.add_cad(
            0,
            "CAD, Default",
            CadItem::Line { a: c[0], b: c[2] },
        )));
        refs.push(ObjectRef::Text(a.cx.project.add_cad(
            0,
            "Text",
            CadItem::Text {
                pos: c[3],
                text: "Note".into(),
                height: 3.0,
                angle: 0.0,
            },
        )));
        let cab = tools::cabinet::default_cabinet(&a.cx, CabinetKind::Base);
        let cab_id = placed::add_cabinet(&mut a.cx.project, 0, cab).unwrap();
        refs.push(ObjectRef::Cabinet(cab_id));
        refs.push(ObjectRef::Symbol(a.cx.project.add_symbol(
            0,
            PlacedSymbol::new("none", Point::new(100.0, 100.0), 24.0, 24.0, 30.0),
        )));
        let stair = stairs_view::build(
            &a.cx.project,
            0,
            stairs_view::StairKind::Draw,
            plan_stairs::Turn::Left,
            Point::new(200.0, 50.0),
            Some(Point::new(350.0, 50.0)),
        );
        refs.push(ObjectRef::Stair(stairs_view::add(
            &mut a.cx.project,
            0,
            stair,
        )));
        let settings = roof_view::RoofSettings::from_defaults(&a.cx.defaults);
        roof_view::rebuild(&mut a.cx.project, 0, settings, false).unwrap();
        let plane = roof_view::load(&a.cx.project.floors[0]).planes[0].id;
        refs.push(ObjectRef::RoofPlane(plane));
        refs.push(ObjectRef::Camera(a.cx.project.add_camera(
            CameraObject::new(
                CameraKind::FullCamera,
                Point::new(50.0, 50.0),
                45.0,
                "Camera 1",
                0,
            ),
        )));
        let kind = tools::electrical::ElecVariant::Outlet110.kind().unwrap();
        let dev = tools::electrical::placement(
            &a.cx,
            kind,
            Point::new(100.0, 0.0),
            Point::new(100.0, 0.0),
        )
        .unwrap();
        let mut dev_id = 0;
        site_view::edit_electrical(&mut a.cx, "Place", |layer, _| dev_id = layer.add(dev));
        refs.push(ObjectRef::Device(dev_id));
        site_view::save_terrain(&mut a.cx.project, &site_view::TerrainRecord::new());
        refs.push(ObjectRef::Terrain);
        a.cx.mark_dirty();
        a.cx.refresh();
        assert!(!a.cx.rooms.is_empty());
        refs.push(ObjectRef::Room(0));
        refs
    }

    #[test]
    fn open_spec_opens_a_dialog_for_every_object_kind() {
        let mut a = app();
        let refs = synthetic_plan(&mut a);
        for o in refs {
            a.dialog = None;
            a.spec = Default::default();
            a.open_spec(o);
            let opened = match o {
                // Rooms and cameras are hosted by the room dialog and 3D panel.
                ObjectRef::Room(_) => editor::rooms_edit::take_room_dialog_request(&a.cx).is_some(),
                ObjectRef::Camera(id) => shell::view3d_panel::Outbox::global()
                    .take()
                    .contains(&shell::view3d_panel::ViewRequest::OpenCameraSpec(id)),
                _ => a.has_dialog(),
            };
            assert!(opened, "no dialog for {o:?}");
        }
    }

    #[test]
    fn every_object_kind_is_found_by_hit_testing_and_deleted_by_the_selection() {
        let mut a = app();
        let refs = synthetic_plan(&mut a);
        for o in &refs {
            assert!(
                matches!(o, ObjectRef::Room(_)) || o.exists_in(&a.cx.project, 0),
                "{o:?} does not exist"
            );
            assert!(
                editor::selection::layer_of(a.cx.floor(), *o).is_some()
                    || matches!(o, ObjectRef::Room(_))
            );
        }
        // Kinds deleted through the shared selection.
        for o in refs.iter().filter(|o| {
            matches!(
                o,
                ObjectRef::Cabinet(_)
                    | ObjectRef::Symbol(_)
                    | ObjectRef::Stair(_)
                    | ObjectRef::RoofPlane(_)
                    | ObjectRef::Camera(_)
                    | ObjectRef::Device(_)
            )
        }) {
            a.cx.selection.set(*o);
            a.cx.refresh();
            assert!(
                a.cx.selection.contains(*o),
                "{o:?} dropped from the selection"
            );
            a.cx.delete_selection();
            assert!(!o.exists_in(&a.cx.project, 0), "{o:?} not deleted");
        }
    }

    #[test]
    fn wall_specification_checkboxes_write_the_wall_flags() {
        let mut a = app();
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        let ids: Vec<Id> = (0..4)
            .map(|i| {
                a.cx.project
                    .add_wall(0, c[i], c[(i + 1) % 4], 6.0, 109.0, WallKind::Exterior)
            })
            .collect();
        a.cx.refresh();
        assert_eq!(a.cx.rooms.len(), 1);
        a.open_wall_dialog(ids[0]);
        let Some(ActiveDialog::Wall(mut d)) = a.dialog.take() else {
            panic!("no wall dialog");
        };
        {
            let f = &mut d.draft_mut().flags;
            f.invisible = true;
            f.no_room_definition = true;
            f.no_locate = true;
        }
        a.apply_wall_dialog(&d);
        let flags = &a.cx.floor().wall(ids[0]).unwrap().flags;
        assert!(flags.invisible && flags.no_room_definition && flags.no_locate);
        // The model honors them: the room is no longer closed, and the wall
        // is not drawn.
        a.cx.refresh();
        assert!(a.cx.rooms.is_empty());
        // Nothing is kept in the session extras for them any more.
        assert!(
            a.cx.extras.walls[&WallTarget::Wall(ids[0]).key()].eq(&dialogs::WallExtras::default())
        );
    }

    #[test]
    fn door_dialog_hinge_side_is_independent_of_swing_side() {
        let mut a = app();
        let w = a.cx.project.add_wall(
            0,
            Point::ZERO,
            Point::new(240.0, 0.0),
            6.0,
            109.0,
            WallKind::Exterior,
        );
        let door =
            a.cx.project
                .add_opening(0, w, 100.0, OpeningKind::Door)
                .unwrap();
        a.open_opening_dialog(door);
        let Some(ActiveDialog::Opening(mut d)) = a.dialog.take() else {
            panic!("no door dialog");
        };
        d.draft_mut().hinge_at_end = true;
        a.apply_opening_dialog(&d);
        let o = a.cx.floor().openings.iter().find(|o| o.id == door).unwrap();
        assert!(o.hinge_at_end && !o.swing_flipped);
    }

    #[test]
    fn properties_rooms_list_shows_interior_area_like_the_label() {
        let mut a = app();
        let c = [
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            Point::new(240.0, 192.0),
            Point::new(0.0, 192.0),
        ];
        for i in 0..4 {
            a.cx.project
                .add_wall(0, c[i], c[(i + 1) % 4], 7.625, 109.0, WallKind::Exterior);
        }
        a.cx.refresh();
        let room = &a.cx.rooms[0];
        let line = room_line("Great Room", room);
        assert!(line.contains(&format!("{} sq ft", room.interior_area_sq_ft().round())));
        assert_ne!(
            room.interior_area_sq_ft().round(),
            room.area_sq_ft().round()
        );
        assert!(editor::rooms_edit::room_label_text(&a.cx, room)
            .contains(&format!("{} sq ft", room.interior_area_sq_ft().round())));
    }

    #[test]
    fn properties_shows_the_wall_type_thickness() {
        let a = app();
        let mut wall = plan_core::Wall {
            wall_type: Some("Siding-6".into()),
            thickness: 7.625,
            ..Default::default()
        };
        let types = a.cx.wall_types().to_vec();
        let def = types.iter().find(|t| t.name == "Siding-6").unwrap();
        wall.thickness = def.thickness();
        let line = wall_type_line(&wall, &types);
        assert!(line.starts_with("Type: Siding-6 ("), "{line}");
        wall.thickness = 12.0;
        assert!(wall_type_line(&wall, &types).contains("this wall"));
        wall.wall_type = None;
        assert!(wall_type_line(&wall, &types).starts_with("Type: none"));
    }

    #[test]
    fn menus_show_the_live_hotkeys() {
        let a = app();
        let st = bar_state(&a.cx, ToolId::Select, None, 1.0, &a.hotkeys.map);
        // Daniel's customized "-" is Zoom In (his file), and 3D View Defaults
        // is bound (Cmd+1).
        assert_eq!(
            st.hotkey("Zoom In", "?"),
            a.hotkeys.map.hotkey_text("Zoom In")
        );
        assert!(st.hotkey("3D View Defaults", "").ends_with('1'));
        assert_eq!(
            a.hotkeys.map.lookup(&[shell::hotkeys::Chord::from_config(
                &plan_config::KeyChord::parse_file("Ctrl+1").unwrap()
            )
            .unwrap()]),
            Some(Action::View3d(shell::view3d_panel::View3dCommand::Defaults))
        );
    }

    #[test]
    fn color_is_on_by_default_and_saved_views_set_reference_display() {
        let mut a = app();
        assert!(a.cx.view_flags.contains(&toolbar::ViewFlag::Color));
        a.cx.project.plan_views[0].reference_display = true;
        a.activate_plan_view(0);
        assert!(a
            .cx
            .view_flags
            .contains(&toolbar::ViewFlag::ReferenceDisplay));
    }

    #[test]
    fn project_browser_requests_select_cameras_and_activate_views() {
        let mut a = app();
        a.cx.project.build_new_floor(false);
        let id = a.cx.project.add_camera(plan_core::CameraObject::new(
            plan_core::CameraKind::FullCamera,
            Point::new(300.0, 120.0),
            45.0,
            "Camera 1",
            1,
        ));
        a.docks
            .requests
            .push(shell::docks::DockRequest::SelectCamera(id));
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| a.dock_panel(ctx));
        // dock_panel only drains when a dock is open; drain explicitly.
        a.dock = Some(Dock::Project);
        let _ = ctx.run(egui::RawInput::default(), |ctx| a.dock_panel(ctx));
        assert_eq!(a.cx.floor, 1, "the camera's floor is shown");
        assert_eq!(a.camera.center, Point::new(300.0, 120.0));
        assert!(a.cx.selection.contains(ObjectRef::Camera(id)));
    }

    fn detection(ceiling: f64, plan: bool) -> templates::Detection {
        let mut defaults = plan_defaults::embedded();
        defaults.rooms.ceiling_height = ceiling;
        templates::Detection {
            settings: templates::TemplateSettings {
                plan: plan.then(|| PathBuf::from("/chief/x18.plan")),
                layout: None,
                seed_from_chief: true,
            },
            defaults,
            note: Some("Defaults seeded from x18.plan".into()),
            save_error: None,
        }
    }

    #[test]
    fn a_finished_template_scan_seeds_an_untouched_plan() {
        let mut a = app();
        let status = a.apply_detection_with(detection(120.0, true), false);
        assert_eq!(status, "Defaults seeded from x18.plan");
        assert_eq!(a.cx.defaults.rooms.ceiling_height, 120.0);
        // The plan nobody has touched starts over from the new defaults.
        assert_eq!(a.cx.project.floors[0].ceiling_height, 120.0);
    }

    #[test]
    fn a_finished_template_scan_leaves_a_worked_plan_and_saved_defaults_alone() {
        let mut a = app();
        a.cx.begin_change("Move Wall");
        a.cx.project.floors[0].name = "Mine".into();
        a.apply_detection_with(detection(120.0, true), false);
        assert_eq!(
            a.cx.defaults.rooms.ceiling_height, 120.0,
            "new plans use it"
        );
        assert_eq!(a.cx.project.floors[0].name, "Mine", "the open plan is kept");
        // The user's own template wins over the scan.
        let mut b = app();
        b.apply_detection_with(detection(120.0, true), true);
        assert_eq!(
            b.cx.defaults,
            editor::code::seeded(plan_defaults::embedded())
        );
        // Nothing found, and a settings file that could not be written, are said.
        let mut c = app();
        let mut found = detection(109.125, false);
        found.note = None;
        found.save_error = Some("read-only".into());
        let status = c.apply_detection_with(found, false);
        assert!(status.contains("No Chief templates found"), "{status}");
        assert!(
            status.contains("not scanning again this session"),
            "{status}"
        );
    }

    #[test]
    fn startup_never_scans_the_home_folder_itself() {
        // The scan runs through `templates::begin_detection` (a thread); the
        // paths the window opens through only read saved settings.
        for (name, src) in [
            ("main.rs", include_str!("main.rs")),
            ("plan_defaults.rs", include_str!("plan_defaults.rs")),
        ] {
            let code = src.split("#[cfg(test)]").next().unwrap();
            assert!(!code.contains("detect_chief_templates"), "{name}");
            assert!(!code.contains("load_or_detect"), "{name}");
        }
    }

    /// Runs one frame of the canvas with `events`.
    fn canvas_frame(a: &mut PlanApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                Vec2::new(1200.0, 800.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| a.canvas(ctx, ui));
        });
    }

    fn right_click(a: &mut PlanApp, ctx: &egui::Context, world: Point) {
        let pos = a.camera.world_to_screen(world);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        };
        canvas_frame(a, ctx, vec![egui::Event::PointerMoved(pos)]);
        canvas_frame(a, ctx, vec![button(true)]);
        canvas_frame(a, ctx, vec![button(false)]);
    }

    #[test]
    fn right_click_selects_the_object_and_the_menu_commands_run() {
        use editor::edit_commands::ids;
        let mut a = app();
        let w = a.cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(240.0, 0.0),
            6.0,
            96.0,
            WallKind::Interior,
        );
        let ctx = egui::Context::default();
        canvas_frame(&mut a, &ctx, vec![]);
        assert!(a.has_canvas_menu(), "Select Objects shows the menu");
        // A right click on the wall selects it.
        right_click(&mut a, &ctx, Point::new(120.0, 0.0));
        assert_eq!(a.cx.selection.single(), Some(ObjectRef::Wall(w)));
        // The menu offers the wall's commands; Copy then Paste Hold Position
        // run through the same path the menu rows take.
        let bar = a.tools.active().edit_toolbar(&a.cx);
        let entries = a.cx.context_entries(&bar, false);
        assert!(entries.iter().any(|e| e.label == "Copy"));
        a.apply(entries.iter().find(|e| e.label == "Copy").unwrap().action);
        a.apply(Action::Custom(ids::PASTE_HOLD));
        assert_eq!(a.cx.floor().walls.len(), 2);
        // A right click on empty plan clears the selection and offers the
        // view menu.
        right_click(&mut a, &ctx, Point::new(300.0, 100.0));
        assert!(a.cx.selection.is_empty());
        let bar = a.tools.active().edit_toolbar(&a.cx);
        let names: Vec<String> =
            a.cx.context_entries(&bar, false)
                .into_iter()
                .map(|e| e.label)
                .collect();
        assert!(names.contains(&"Select All".to_string()), "{names:?}");
        // The drawing tools keep right click as their Esc.
        a.set_tool(ToolId::Wall {
            kind: WallKind::Exterior,
        });
        assert!(!a.has_canvas_menu());
        a.set_tool(ToolId::Door);
        assert!(a.has_canvas_menu(), "the door tool shows the menu too");
        // The edit commands are for the plan, not the 3D view.
        a.set_tool(ToolId::Select);
        a.view3d.active = true;
        let before = a.cx.floor().walls.len();
        a.apply(Action::Custom(ids::PASTE_HOLD));
        assert_eq!(a.cx.floor().walls.len(), before);
        assert!(a.cx.status.contains("plan view"));
    }
}
