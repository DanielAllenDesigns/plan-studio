//! The right-hand dock: Chief's three docked panels, switched by the view-bar
//! toggles (`toolbar::Dock`).
//!
//! * **Active Layer Display Options**: the layer table (Name, Used, Disp,
//!   Lock, Color) with a name filter, and "Properties for Selected Layer".
//!   Every edit goes through `begin_change` on `project.layers` so it is
//!   undoable, and marks the plan dirty so hidden layers vanish at once.
//! * **Project Browser**: Plan > Floors / Cameras / Saved Views, and Layout.
//! * **Library Browser**: see [`super::library_browser`].
//!
//! The panels never switch floors or tools themselves (the app owns the tool
//! set); they queue [`DockRequest`]s that `main.rs` applies.
//!
//! [`panel`] is the resizable side panel: a tab strip (Library, Project,
//! Layers), the dock's title and its body. The width of each dock is kept in
//! `AppSettings::dock_widths`. Everything in it is reachable from the
//! keyboard: Tab / Shift+Tab walk the controls, the arrow keys move between
//! rows and open or close tree nodes ([`tree_node`]), Enter and Space press
//! the focused control, Escape gives the focus back, and F6
//! ([`toggle_focus_region`]) jumps in and out of the dock.

use super::hotkeys::HotkeyState;
use super::library_browser::{self, LibraryBrowserState};
use crate::dialogs::hotkeys::HotkeyDialog;
use crate::dialogs::layer_display::LayerDisplayDialog;
use crate::dialogs::Outcome;
use crate::editor::{EditorContext, ObjectRef};
use crate::theme::{self, AppSettings};
use crate::toolbar::{Action, Dock};
use crate::tools::ToolId;
use eframe::egui::{self, Key, Modifiers, Sense, Stroke, Vec2};
use plan_core::layer_sets::LayerEdit;
use plan_core::{DimensionKind, Layer, OpeningKind, Project};
use std::collections::HashMap;

/// Something a dock panel needs the application to do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockRequest {
    SetTool(ToolId),
    SwitchFloor(usize),
    /// Select a camera object, show its floor and pan the plan to it.
    SelectCamera(plan_core::Id),
    /// Center the plan view on this point (the Project Browser's jump to a
    /// schedule or a CAD detail; the camera lives in the shell).
    PanTo(plan_core::geometry::Point),
    /// Activate the saved plan view at this index of `Project::plan_views`.
    /// (The Project Browser and the tab strip switch views themselves through
    /// `editor::plan_tabs`; the shell still handles this request.)
    #[allow(dead_code)]
    ActivatePlanView(usize),
    /// Run a menu action (the Layout section's Create Construction Set).
    Run(crate::toolbar::Action),
}

/// The layer table and its state (see `dialogs::layer_display`).
pub use crate::dialogs::layer_display::{layer_panel, LayerPanelState};

#[derive(Default)]
pub struct DockState {
    pub layers: LayerPanelState,
    pub library: LibraryBrowserState,
    /// Floor switches and tool changes for the app to apply.
    pub requests: Vec<DockRequest>,
    pub hotkey_dialog: Option<HotkeyDialog>,
    pub layer_dialog: Option<LayerDisplayDialog>,
}

impl DockState {
    /// A shell dialog is open: the canvas keys must not reach the tools.
    pub fn modal_open(&self) -> bool {
        self.hotkey_dialog.is_some() || self.layer_dialog.is_some()
    }

    pub fn open_hotkey_dialog(&mut self, hk: &HotkeyState) {
        if self.hotkey_dialog.is_none() {
            self.hotkey_dialog = Some(HotkeyDialog::new(&hk.map));
        }
    }

    pub fn open_layer_dialog(&mut self) {
        if self.layer_dialog.is_none() {
            self.layer_dialog = Some(LayerDisplayDialog::default());
        }
    }
}

/// Draws the dock panel for `dock`.
pub fn show(ui: &mut egui::Ui, dock: Dock, cx: &mut EditorContext, st: &mut DockState) {
    dock_tabs(ui, dock, &mut st.requests);
    ui.add_space(2.0);
    ui.strong(dock.title());
    ui.separator();
    match dock {
        Dock::LayerDisplay => layer_panel(ui, cx, &mut st.layers, 0.55),
        Dock::Project => project_browser(ui, cx, &mut st.requests),
        Dock::Library => {
            if let Some(ev) = library_browser::show(ui, &mut st.library) {
                if let Some(tool) = library_browser::apply_event(ev, &mut st.library, cx) {
                    st.requests.push(DockRequest::SetTool(tool));
                }
            }
        }
    }
}

/// The resizable right-hand dock for `dock`: width from and back into
/// `settings.dock_widths`, then the tab strip and body ([`show`]).
pub fn panel(
    ctx: &egui::Context,
    dock: Dock,
    cx: &mut EditorContext,
    st: &mut DockState,
    settings: &mut AppSettings,
) {
    let max = (ctx.screen_rect().width() * 0.5).clamp(theme::DOCK_WIDTH_MIN, theme::DOCK_WIDTH_MAX);
    let resp = egui::SidePanel::right(format!("dock_panel_{dock:?}"))
        .default_width(settings.dock_widths.get(dock).min(max))
        .width_range(theme::DOCK_WIDTH_MIN..=max)
        .resizable(true)
        .show(ctx, |ui| show(ui, dock, cx, st));
    settings.dock_widths.set(dock, resp.response.rect.width());
}

/// Records the width of a side panel (the Properties panel) in points.
pub fn remember_width(slot: &mut f32, width: f32) {
    if (*slot - width).abs() > 0.5 && width.is_finite() {
        *slot = width.clamp(theme::DOCK_WIDTH_MIN, theme::DOCK_WIDTH_MAX);
    }
}

/// The three docks in tab order, with the tab's icon and short label.
const TABS: [(Dock, &str, &str); 3] = [
    (Dock::Library, "library_browser", "Library"),
    (Dock::Project, "project_browser", "Project"),
    (Dock::LayerDisplay, "layer_display", "Layers"),
];

const TAB_HEIGHT: f32 = 32.0;
const TAB_ICON_PX: f32 = 18.0;

/// Chief's dock tab strip: the active tab is lit (panel color, accent bar on
/// top, icon and name), the others are dark icons; clicking one switches the
/// dock, the close button hides it. Tabs are focusable.
fn dock_tabs(ui: &mut egui::Ui, active: Dock, requests: &mut Vec<DockRequest>) {
    let chrome = theme::current_chrome();
    let want_focus = ui.ctx().data_mut(|d| {
        let id = egui::Id::new("dock_focus_request");
        let v: bool = d.get_temp(id).unwrap_or(false);
        d.insert_temp(id, false);
        v
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (dock, icon, label) in TABS {
            let is_active = dock == active;
            let text = egui::WidgetText::from(label).into_galley(
                ui,
                Some(egui::TextWrapMode::Extend),
                f32::INFINITY,
                egui::TextStyle::Button,
            );
            let width = if is_active {
                8.0 + TAB_ICON_PX + 6.0 + text.size().x + 8.0
            } else {
                TAB_ICON_PX + 16.0
            };
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, TAB_HEIGHT), Sense::click());
            let resp = resp.on_hover_text(dock.title());
            if is_active && want_focus {
                resp.request_focus();
            }
            let fill = if is_active {
                chrome.panel
            } else if resp.hovered() {
                chrome.hover
            } else {
                chrome.status
            };
            let top = egui::CornerRadius {
                nw: 6,
                ne: 6,
                sw: 0,
                se: 0,
            };
            let p = ui.painter();
            p.rect_filled(rect, top, fill);
            if is_active {
                let bar = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), 3.0));
                p.rect_filled(bar, top, chrome.accent);
            }
            if resp.hovered() && !is_active {
                p.rect_stroke(
                    rect.shrink(0.5),
                    top,
                    Stroke::new(1.5_f32, chrome.outline),
                    egui::StrokeKind::Inside,
                );
            }
            if resp.has_focus() {
                p.rect_stroke(
                    rect,
                    top,
                    Stroke::new(2.0_f32, chrome.text),
                    egui::StrokeKind::Inside,
                );
            }
            let icon_x = if is_active {
                rect.min.x + 8.0
            } else {
                rect.center().x - TAB_ICON_PX / 2.0
            };
            let icon_rect = egui::Rect::from_min_size(
                egui::pos2(icon_x, rect.center().y - TAB_ICON_PX / 2.0 + 1.0),
                Vec2::splat(TAB_ICON_PX),
            );
            egui::Image::new(crate::icons::icon(icon)).paint_at(ui, icon_rect);
            if is_active {
                let pos = egui::pos2(
                    icon_rect.max.x + 6.0,
                    rect.center().y - text.size().y / 2.0 + 1.0,
                );
                ui.painter().galley(pos, text, chrome.text);
            }
            if resp.clicked() && !is_active {
                requests.push(DockRequest::Run(Action::ToggleDock(dock)));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(egui::Button::new("\u{2715}").small())
                .on_hover_text("Close the panel")
                .clicked()
            {
                requests.push(DockRequest::Run(Action::ToggleDock(active)));
            }
        });
    });
}

/// F6: moves the keyboard focus into the dock (its active tab), or out of the
/// chrome back to the canvas when it is already in some panel. Returns true
/// when it did something.
pub fn toggle_focus_region(ctx: &egui::Context, dock_open: bool) -> bool {
    if focus_in_chrome(ctx) {
        if let Some(id) = ctx.memory(|m| m.focused()) {
            ctx.memory_mut(|m| m.surrender_focus(id));
        }
        return true;
    }
    if dock_open {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("dock_focus_request"), true));
        ctx.request_repaint();
        return true;
    }
    false
}

/// The rectangle of the drawing area, stored each frame by the app so
/// [`focus_in_chrome`] can tell canvas focus from panel focus.
pub fn set_central_rect(ctx: &egui::Context, rect: egui::Rect) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("central_rect"), rect));
}

/// Does a widget outside the drawing area (a toolbar button, a dock control,
/// a menu) have the keyboard focus? Then Tab, Enter and the arrow keys
/// belong to it, not to the active tool.
pub fn focus_in_chrome(ctx: &egui::Context) -> bool {
    let Some(id) = ctx.memory(|m| m.focused()) else {
        return false;
    };
    let central: Option<egui::Rect> = ctx.data(|d| d.get_temp(egui::Id::new("central_rect")));
    match (central, ctx.read_response(id)) {
        (Some(c), Some(r)) => !c.contains(r.rect.center()),
        _ => false,
    }
}

/// A tree node (a collapsible section): a [`egui::CollapsingHeader`] that the
/// keyboard drives, Right opens it and Left closes it while it has focus
/// (Enter and Space toggle it, Up and Down move to the neighbouring rows).
/// Motion follows the "Reduce motion" setting through `animation_time`.
pub fn tree_node<R>(
    ui: &mut egui::Ui,
    salt: &str,
    title: impl Into<egui::WidgetText>,
    default_open: bool,
    add_body: impl FnOnce(&mut egui::Ui) -> R,
) -> Option<R> {
    let id = ui.make_persistent_id(salt);
    let resp = egui::CollapsingHeader::new(title)
        .id_salt(salt)
        .default_open(default_open)
        .show(ui, add_body);
    if resp.header_response.has_focus() {
        // Keep Left and Right for the node instead of moving the focus.
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.header_response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    ..Default::default()
                },
            );
        });
        let (right, left) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowRight),
                i.consume_key(Modifiers::NONE, Key::ArrowLeft),
            )
        });
        if right || left {
            let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                default_open,
            );
            state.set_open(right);
            state.store(ui.ctx());
        }
    }
    resp.body_returned
}

/// A search field with a clear button. Escape in the field clears it too.
pub fn search_field(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let button = ui.spacing().interact_size.y;
        let width = (ui.available_width() - button - ui.spacing().item_spacing.x).max(60.0);
        let edit = ui.add(
            egui::TextEdit::singleline(text)
                .hint_text(hint)
                .desired_width(width),
        );
        if edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            text.clear();
        }
        let clear = ui
            .add_enabled(
                !text.is_empty(),
                egui::Button::new("\u{2715}").min_size(Vec2::splat(button)),
            )
            .on_hover_text("Clear the search");
        if clear.clicked() {
            text.clear();
            edit.request_focus();
        }
        edit
    })
    .inner
}

/// Shows the shell's modal dialogs and applies their results.
pub fn show_dialogs(
    ctx: &egui::Context,
    cx: &mut EditorContext,
    docks: &mut DockState,
    hk: &mut HotkeyState,
) {
    if let Some(mut dialog) = docks.hotkey_dialog.take() {
        // The dialog records chords: keep the global hotkeys quiet.
        hk.suspended = true;
        match dialog.show(ctx) {
            Outcome::Open => docks.hotkey_dialog = Some(dialog),
            Outcome::Cancel => {}
            Outcome::Ok => {
                cx.status = match dialog.commit(&mut hk.map) {
                    Ok(()) => "Hotkeys saved".into(),
                    Err(e) => format!("Hotkeys changed, but could not be saved: {e}"),
                };
            }
        }
    }
    if let Some(mut dialog) = docks.layer_dialog.take() {
        if dialog.show(ctx, cx) {
            docks.layer_dialog = Some(dialog);
        }
    }
    crate::dialogs::layer_sets::show_all(ctx, cx);
    crate::dialogs::plan_views::show_all(ctx, cx);
}

// ----- layers -----

/// How many objects of the whole plan are on each layer.
pub fn layer_usage(project: &Project) -> HashMap<String, usize> {
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut add = |name: &str| *used.entry(name.to_string()).or_insert(0) += 1;
    for f in &project.floors {
        for w in &f.walls {
            add(&w.layer);
        }
        for o in &f.openings {
            add(match o.kind {
                OpeningKind::Door => "Doors",
                OpeningKind::Window => "Windows",
            });
        }
        for d in &f.dimensions {
            add(match d.kind {
                DimensionKind::AutoExterior => "Dimensions, Automatic",
                _ => "Dimensions, Manual",
            });
        }
        for c in &f.cad {
            add(&c.layer);
        }
        for s in &f.symbols {
            add(&s.layer);
        }
    }
    used
}

/// Names of the layers whose name contains `filter` (case-insensitive), in
/// layer order.
pub fn layer_rows(project: &Project, filter: &str) -> Vec<String> {
    let needle = filter.trim().to_lowercase();
    project
        .layers
        .layers
        .iter()
        .filter(|l| needle.is_empty() || l.name.to_lowercase().contains(&needle))
        .map(|l| l.name.clone())
        .collect()
}

/// Edits `layers` as one undoable step: in the shown layer set, or in every
/// set when `all_sets` ("Modify All Layer Sets"). `merged` lets a drag share
/// one step. Returns how many (set, layer) cells were written (0 when no
/// layer exists, and then no undo step is made).
pub fn edit_layers(
    cx: &mut EditorContext,
    layers: &[String],
    edit: LayerEdit,
    all_sets: bool,
    merged: bool,
) -> usize {
    let existing: Vec<String> = layers
        .iter()
        .filter(|l| cx.project.layers.get(l).is_some())
        .cloned()
        .collect();
    if existing.is_empty() {
        return 0;
    }
    let label = match edit {
        LayerEdit::Display(_) => "Layer Display",
        LayerEdit::Locked(_) => "Layer Lock",
        LayerEdit::Reference(_) => "Layer Reference",
        LayerEdit::Color(_) => "Layer Color",
        LayerEdit::LineWeight(_) => "Layer Line Weight",
        LayerEdit::LineStyle(_) => "Layer Line Style",
        LayerEdit::TextStyle(_) => "Layer Text Style",
    };
    if merged {
        cx.begin_change_merged(label);
    } else {
        cx.begin_change(label);
    }
    let n = cx.project.edit_layers(&existing, &edit, all_sets);
    cx.mark_dirty();
    // The plan reads the layers through the context: bring them up to date now.
    cx.refresh_layer_view();
    n
}

/// Makes `name` the shown layer set (one undo step).
#[cfg(test)]
pub fn set_active_layer_set(cx: &mut EditorContext, name: &str) -> bool {
    crate::dialogs::layer_sets::activate_set(cx, name)
}

/// Turns a layer's display on or off (the Disp checkbox).
#[cfg(test)]
pub fn set_layer_display(cx: &mut EditorContext, name: &str, on: bool) -> bool {
    edit_layers(
        cx,
        &[name.to_string()],
        LayerEdit::Display(on),
        false,
        false,
    ) > 0
}

/// Locks or unlocks a layer (the Lock checkbox).
#[cfg(test)]
pub fn set_layer_locked(cx: &mut EditorContext, name: &str, locked: bool) -> bool {
    edit_layers(
        cx,
        &[name.to_string()],
        LayerEdit::Locked(locked),
        false,
        false,
    ) > 0
}

/// Turns the "Ref" box of a layer on or off: whether its objects show on a
/// reference floor (LAY-10).
#[cfg(test)]
pub fn set_layer_reference(cx: &mut EditorContext, name: &str, on: bool) -> bool {
    edit_layers(
        cx,
        &[name.to_string()],
        LayerEdit::Reference(on),
        false,
        false,
    ) > 0
}

/// Adds a layer with a fresh name; returns the name.
pub fn add_layer(cx: &mut EditorContext) -> String {
    let mut n = cx.project.layers.layers.len() + 1;
    let name = loop {
        let candidate = format!("New Layer {n}");
        if cx.project.layers.get(&candidate).is_none() {
            break candidate;
        }
        n += 1;
    };
    cx.begin_change("New Layer");
    cx.project
        .layers
        .add(Layer::new(name.clone(), [0, 0, 0], 18));
    cx.mark_dirty();
    name
}

// ----- project browser -----

/// The sections of the Project Browser under the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserNode {
    Floors,
    PlanViews,
    Cameras,
    Schedules,
    CadDetails,
    Layout,
}

impl BrowserNode {
    /// The order the browser lists them in.
    pub const ALL: [BrowserNode; 6] = [
        BrowserNode::Floors,
        BrowserNode::PlanViews,
        BrowserNode::Cameras,
        BrowserNode::Schedules,
        BrowserNode::CadDetails,
        BrowserNode::Layout,
    ];

    pub fn title(self) -> &'static str {
        match self {
            BrowserNode::Floors => "Floors",
            BrowserNode::PlanViews => "Plan Views",
            BrowserNode::Cameras => "Cameras",
            BrowserNode::Schedules => "Schedules",
            BrowserNode::CadDetails => "CAD Details",
            BrowserNode::Layout => "Layout",
        }
    }

    fn salt(self) -> &'static str {
        match self {
            BrowserNode::Floors => "pb_floors",
            BrowserNode::PlanViews => "pb_views",
            BrowserNode::Cameras => "pb_cameras",
            BrowserNode::Schedules => "pb_schedules",
            BrowserNode::CadDetails => "pb_cad_details",
            BrowserNode::Layout => "pb_layout_pages",
        }
    }
}

/// What a row of the Project Browser stands for.
#[derive(Clone, Debug, PartialEq)]
pub enum BrowserItem {
    Floor(usize),
    /// Index into `Project::plan_views`.
    PlanView(usize),
    Camera(plan_core::Id),
    Schedule {
        floor: usize,
        id: plan_core::Id,
    },
    CadDetail {
        floor: usize,
        group: plan_core::Id,
    },
    /// Index into the layout's pages.
    Page(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BrowserEntry {
    pub item: BrowserItem,
    pub label: String,
}

/// A camera's row in the Project Browser.
fn camera_label(cam: &plan_core::CameraObject) -> String {
    if cam.name.is_empty() {
        format!("Camera {}", cam.id)
    } else {
        cam.name.clone()
    }
}

/// The rows of the Project Browser, node by node: floors, plan views,
/// cameras, schedules (every floor's), CAD details (the named CAD blocks of
/// every floor) and the layout's pages.
pub fn browser_nodes(cx: &EditorContext) -> Vec<(BrowserNode, Vec<BrowserEntry>)> {
    let p = &cx.project;
    let entry = |item, label: String| BrowserEntry { item, label };
    let several = p.floors.len() > 1;
    let tag = |floor: &plan_core::Floor, text: String| {
        if several {
            format!("{} \u{2013} {}", floor.name, text)
        } else {
            text
        }
    };
    let mut schedules = Vec::new();
    let mut details = Vec::new();
    for (fi, f) in p.floors.iter().enumerate() {
        for s in plan_core::schedules::ScheduleLayer::load(f).schedules {
            let title = if s.title.is_empty() {
                s.kind.name().to_string()
            } else {
                s.title.clone()
            };
            schedules.push(entry(
                BrowserItem::Schedule {
                    floor: fi,
                    id: s.id,
                },
                tag(f, title),
            ));
        }
        for b in f.cad_blocks() {
            details.push(entry(
                BrowserItem::CadDetail {
                    floor: fi,
                    group: b.group,
                },
                tag(f, b.name),
            ));
        }
    }
    BrowserNode::ALL
        .into_iter()
        .map(|node| {
            let rows = match node {
                BrowserNode::Floors => p
                    .floors
                    .iter()
                    .enumerate()
                    .map(|(i, f)| entry(BrowserItem::Floor(i), f.name.clone()))
                    .collect(),
                BrowserNode::PlanViews => p
                    .plan_views
                    .iter()
                    .enumerate()
                    .map(|(i, v)| entry(BrowserItem::PlanView(i), v.name.clone()))
                    .collect(),
                BrowserNode::Cameras => p
                    .cameras
                    .iter()
                    .map(|c| entry(BrowserItem::Camera(c.id), camera_label(c)))
                    .collect(),
                BrowserNode::Schedules => schedules.clone(),
                BrowserNode::CadDetails => details.clone(),
                BrowserNode::Layout => super::layout_window::page_list(p)
                    .into_iter()
                    .map(|(i, label, _)| entry(BrowserItem::Page(i), label))
                    .collect(),
            };
            (node, rows)
        })
        .collect()
}

/// Shows floor `floor` (the Project Browser's jump); clears the selection
/// when the floor changes.
fn jump_to_floor(cx: &mut EditorContext, floor: usize) {
    if floor < cx.project.floors.len() && floor != cx.floor {
        cx.floor = floor;
        cx.reset_view_state();
    }
}

/// Jump to a schedule: shows its floor and selects the table.
pub fn jump_to_schedule(cx: &mut EditorContext, floor: usize, id: plan_core::Id) -> bool {
    let Some(f) = cx.project.floors.get(floor) else {
        return false;
    };
    if !crate::editor::schedule_view::exists(f, id) {
        return false;
    }
    jump_to_floor(cx, floor);
    crate::editor::schedule_view::select(cx, id);
    cx.status = "Schedule selected".into();
    true
}

/// Jump to a CAD detail: shows its floor and selects the block's objects.
pub fn jump_to_cad_detail(cx: &mut EditorContext, floor: usize, group: plan_core::Id) -> bool {
    let Some(f) = cx.project.floors.get(floor) else {
        return false;
    };
    let ids = f.group_members_cad(group);
    if ids.is_empty() {
        return false;
    }
    jump_to_floor(cx, floor);
    cx.selection.items = ids.into_iter().map(ObjectRef::Cad).collect();
    true
}

/// The middle of schedule `id`'s table on the active floor (what the plan
/// pans to after [`jump_to_schedule`]).
pub fn schedule_center(
    cx: &EditorContext,
    id: plan_core::Id,
) -> Option<plan_core::geometry::Point> {
    crate::editor::schedule_view::extents(cx)
        .into_iter()
        .find(|(i, _, _)| *i == id)
        .map(|(_, lo, hi)| plan_core::geometry::Point::lerp(lo, hi, 0.5))
}

/// The middle of the CAD detail group `group` of `floor` (what the plan pans
/// to after [`jump_to_cad_detail`]).
pub fn cad_detail_center(
    cx: &EditorContext,
    floor: usize,
    group: plan_core::Id,
) -> Option<plan_core::geometry::Point> {
    let f = cx.project.floors.get(floor)?;
    let ids = f.group_members_cad(group);
    let mut bounds: Option<(plan_core::geometry::Point, plan_core::geometry::Point)> = None;
    for o in f.cad.iter().filter(|o| ids.contains(&o.id)) {
        let (lo, hi) = o.bounds();
        bounds = Some(match bounds {
            None => (lo, hi),
            Some((a, b)) => (
                plan_core::geometry::Point::new(a.x.min(lo.x), a.y.min(lo.y)),
                plan_core::geometry::Point::new(b.x.max(hi.x), b.y.max(hi.y)),
            ),
        });
    }
    bounds.map(|(a, b)| plan_core::geometry::Point::lerp(a, b, 0.5))
}

/// Rename a camera (one undo step). `false` when it does not exist or the
/// name is empty.
pub fn rename_camera(cx: &mut EditorContext, id: plan_core::Id, name: &str) -> bool {
    let name = name.trim();
    if name.is_empty() || cx.project.camera(id).is_none() {
        return false;
    }
    cx.begin_change("Rename Camera");
    cx.project.update_camera(id, |c| c.name = name.to_string());
    cx.mark_dirty();
    true
}

/// Delete a camera (one undo step). `false` when it does not exist.
pub fn delete_camera(cx: &mut EditorContext, id: plan_core::Id) -> bool {
    if cx.project.camera(id).is_none() {
        return false;
    }
    cx.begin_change("Delete Camera");
    cx.project.remove_camera(id);
    cx.selection.items.retain(|o| *o != ObjectRef::Camera(id));
    cx.mark_dirty();
    true
}

/// The Layout section: the active layout's sheet (what View > Drawing Sheet
/// and Print Preview draw) and the construction set output.
fn layout_section(ui: &mut egui::Ui, cx: &mut EditorContext, requests: &mut Vec<DockRequest>) {
    ui.label("Active layout sheet");
    egui::ComboBox::from_id_salt("pb_sheet_size")
        .selected_text(cx.sheet.size.label())
        .show_ui(ui, |ui| {
            for s in plan_docs::SheetSize::ALL {
                ui.selectable_value(&mut cx.sheet.size, s, s.label());
            }
        });
    egui::ComboBox::from_id_salt("pb_sheet_scale")
        .selected_text(cx.sheet.scale.label())
        .show_ui(ui, |ui| {
            for s in plan_docs::Scale::ALL {
                ui.selectable_value(&mut cx.sheet.scale, s, s.label());
            }
        });
    ui.weak("Shown by View > Drawing Sheet and Print Preview.");
    ui.separator();
    layout_pages(ui, cx, requests);
    if ui.button("Create Construction Set\u{2026}").clicked() {
        requests.push(DockRequest::Run(Action::CreateConstructionSet));
    }
}

/// The project's layout: its pages (click to open one in the layout view)
/// and the buttons that make, open and print it.
fn layout_pages(ui: &mut egui::Ui, cx: &EditorContext, requests: &mut Vec<DockRequest>) {
    use super::layout_window::{self, LayoutCommand as C};
    let run = |requests: &mut Vec<DockRequest>, c: C| {
        requests.push(DockRequest::Run(Action::Layout(c)));
    };
    let pages = layout_window::page_list(&cx.project);
    if pages.is_empty() {
        ui.weak("This plan has no layout");
        if ui.button("New Layout").clicked() {
            requests.push(DockRequest::Run(Action::FileNewLayout));
        }
        return;
    }
    let showing = layout_window::is_active().then(layout_window::current_page_index);
    for (i, label, template) in pages {
        let text = if template {
            egui::RichText::new(label).italics()
        } else {
            egui::RichText::new(label)
        };
        if ui.selectable_label(showing == Some(i), text).clicked() {
            run(requests, C::GoToPage(i));
        }
    }
    ui.horizontal_wrapped(|ui| {
        if ui.button("Open Layout").clicked() {
            run(requests, C::ShowLayout);
        }
        if ui.button("Add Page").clicked() {
            run(requests, C::InsertPageAfter);
        }
        if ui.button("Page Setup\u{2026}").clicked() {
            run(requests, C::PageSetup);
        }
        if ui.button("Print\u{2026}").clicked() {
            run(requests, C::Print);
        }
    });
}

/// A plan view row being dragged to a new place in the list.
struct PlanViewDrag(usize);

/// The row being renamed in place: `key` names the row, `text` is the edit.
#[derive(Clone, Default)]
struct RenameState {
    key: String,
    text: String,
}

fn rename_id() -> egui::Id {
    egui::Id::new("project_browser_rename")
}

/// A row that turns into a text box while it is being renamed. Returns the
/// new name when Enter was pressed on a changed name.
fn rename_row(ui: &mut egui::Ui, key: &str) -> Option<String> {
    let mut st: RenameState = ui.data(|d| d.get_temp(rename_id())).unwrap_or_default();
    if st.key != key {
        return None;
    }
    let edit = ui.add(egui::TextEdit::singleline(&mut st.text).desired_width(f32::INFINITY));
    if !edit.has_focus() && !edit.lost_focus() {
        edit.request_focus();
    }
    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    if enter || escape || (edit.lost_focus() && !enter) {
        ui.data_mut(|d| d.remove_temp::<RenameState>(rename_id()));
        return enter.then(|| st.text.trim().to_string());
    }
    ui.data_mut(|d| d.insert_temp(rename_id(), st));
    None
}

fn is_renaming(ui: &egui::Ui, key: &str) -> bool {
    ui.data(|d| d.get_temp::<RenameState>(rename_id()))
        .is_some_and(|s| s.key == key)
}

fn start_rename(ui: &egui::Ui, key: &str, text: &str) {
    ui.data_mut(|d| {
        d.insert_temp(
            rename_id(),
            RenameState {
                key: key.to_string(),
                text: text.to_string(),
            },
        )
    });
}

/// The Plan Views rows: click selects, double-click opens the view as a tab,
/// right-click offers Specification, Rename, Duplicate and Delete, and a row
/// can be dragged to a new place in the list.
fn plan_view_rows(ui: &mut egui::Ui, cx: &mut EditorContext, rows: &[BrowserEntry]) {
    use crate::dialogs::plan_views as pv;
    let selected_id = egui::Id::new("pb_selected_view");
    let selected: Option<String> = ui.data(|d| d.get_temp(selected_id));
    let mut open: Option<String> = None;
    let mut spec: Option<String> = None;
    let mut dup: Option<String> = None;
    let mut delete: Option<String> = None;
    let mut mv: Option<(usize, usize)> = None;
    let mut renamed: Option<(String, String)> = None;
    let mut pick: Option<String> = None;
    for e in rows {
        let BrowserItem::PlanView(i) = e.item else {
            continue;
        };
        let Some(view) = cx.project.plan_views.get(i) else {
            continue;
        };
        let name = view.name.clone();
        let shown = name == cx.project.active_plan_view;
        let key = format!("view:{name}");
        if is_renaming(ui, &key) {
            if let Some(new_name) = rename_row(ui, &key) {
                renamed = Some((name.clone(), new_name));
            }
            continue;
        }
        let label = if shown {
            egui::RichText::new(&name).strong()
        } else {
            egui::RichText::new(&name)
        };
        let on = selected.as_deref() == Some(name.as_str()) || (shown && selected.is_none());
        let r = ui
            .selectable_label(on, label)
            .interact(Sense::click_and_drag())
            .on_hover_text(format!(
                "{}\nDouble-click to open as a tab; drag to reorder",
                pv::summary(view)
            ));
        r.dnd_set_drag_payload(PlanViewDrag(i));
        if r.clicked() {
            pick = Some(name.clone());
        }
        if r.double_clicked() {
            open = Some(name.clone());
        }
        if let Some(p) = r.dnd_hover_payload::<PlanViewDrag>() {
            if p.0 != i {
                let y = if p.0 < i {
                    r.rect.bottom()
                } else {
                    r.rect.top()
                };
                ui.painter().hline(
                    r.rect.x_range(),
                    y,
                    Stroke::new(2.0_f32, theme::current_chrome().accent),
                );
            }
        }
        if let Some(p) = r.dnd_release_payload::<PlanViewDrag>() {
            if p.0 != i {
                mv = Some((p.0, i));
            }
        }
        r.context_menu(|ui| {
            if ui.button("Open as Tab").clicked() {
                open = Some(name.clone());
                ui.close_menu();
            }
            if ui.button("Specification\u{2026}").clicked() {
                spec = Some(name.clone());
                ui.close_menu();
            }
            if ui.button("Rename").clicked() {
                start_rename(ui, &key, &name);
                ui.close_menu();
            }
            if ui.button("Duplicate").clicked() {
                dup = Some(name.clone());
                ui.close_menu();
            }
            if ui.button("Delete").clicked() {
                delete = Some(name.clone());
                ui.close_menu();
            }
        });
    }
    if let Some(n) = pick {
        ui.data_mut(|d| d.insert_temp(selected_id, n));
    }
    if let Some((old, new_name)) = renamed {
        cx.status = match pv::rename_view(cx, &old, &new_name) {
            Ok(()) => format!("Renamed plan view to {new_name}"),
            Err(e) => e,
        };
    }
    if let Some((from, to)) = mv {
        pv::move_view(cx, from, to);
    }
    if let Some(n) = dup {
        pv::duplicate_view(cx, &n);
    }
    if let Some(n) = delete {
        cx.status = match pv::delete_view(cx, &n) {
            Ok(()) => format!("Deleted plan view {n}"),
            Err(e) => e,
        };
    }
    if let Some(n) = spec {
        pv::open_on(&n);
    }
    if let Some(n) = open {
        ui.data_mut(|d| d.insert_temp(selected_id, n.clone()));
        crate::editor::plan_tabs::with_tabs(|t| t.open_view(cx, &n));
    }
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("New Plan View").clicked() {
            pv::new_view(cx);
        }
        if ui
            .small_button("Add Template Views")
            .on_hover_text("Add Daniel's template plan views this plan lacks")
            .clicked()
        {
            pv::run_command(cx, pv::SEED);
        }
    });
}

/// The Cameras rows: click selects the camera and pans to it, right-click
/// offers Rename, Delete and Send to Layout.
fn camera_rows(
    ui: &mut egui::Ui,
    cx: &mut EditorContext,
    rows: &[BrowserEntry],
    requests: &mut Vec<DockRequest>,
) {
    if rows.is_empty() {
        ui.weak("None");
    }
    let selected = cx.selection.single();
    let mut renamed: Option<(plan_core::Id, String)> = None;
    let mut delete: Option<plan_core::Id> = None;
    for e in rows {
        let BrowserItem::Camera(id) = e.item else {
            continue;
        };
        let key = format!("camera:{id}");
        if is_renaming(ui, &key) {
            if let Some(n) = rename_row(ui, &key) {
                renamed = Some((id, n));
            }
            continue;
        }
        let current = selected == Some(ObjectRef::Camera(id));
        let r = ui
            .selectable_label(current, &e.label)
            .on_hover_text("Select the camera and pan the plan to it (right-click: Rename, Delete, Send to Layout)");
        if r.clicked() {
            requests.push(DockRequest::SelectCamera(id));
        }
        let label = e.label.clone();
        r.context_menu(|ui| {
            if ui.button("Rename").clicked() {
                start_rename(ui, &key, &label);
                ui.close_menu();
            }
            if ui.button("Delete").clicked() {
                delete = Some(id);
                ui.close_menu();
            }
            if ui.button("Send to Layout\u{2026}").clicked() {
                requests.push(DockRequest::Run(Action::Layout(
                    super::layout_window::LayoutCommand::SendCamera(id),
                )));
                ui.close_menu();
            }
        });
    }
    if let Some((id, n)) = renamed {
        if !rename_camera(cx, id, &n) {
            cx.status = "Type a name for the camera".into();
        }
    }
    if let Some(id) = delete {
        delete_camera(cx, id);
    }
}

fn project_browser(ui: &mut egui::Ui, cx: &mut EditorContext, requests: &mut Vec<DockRequest>) {
    let nodes = browser_nodes(cx);
    egui::ScrollArea::vertical()
        .id_salt("project_browser")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            tree_node(
                ui,
                "pb_plan",
                format!("Plan \u{2013} {}", cx.project.name),
                true,
                |ui| {
                    for (node, rows) in &nodes {
                        match node {
                            BrowserNode::Floors => {
                                tree_node(ui, node.salt(), node.title(), true, |ui| {
                                    for e in rows {
                                        let BrowserItem::Floor(i) = e.item else {
                                            continue;
                                        };
                                        let current = i == cx.floor;
                                        if ui.selectable_label(current, &e.label).clicked()
                                            && !current
                                        {
                                            requests.push(DockRequest::SwitchFloor(i));
                                        }
                                    }
                                });
                            }
                            BrowserNode::PlanViews => {
                                tree_node(ui, node.salt(), node.title(), true, |ui| {
                                    plan_view_rows(ui, cx, rows);
                                });
                            }
                            BrowserNode::Cameras => {
                                tree_node(ui, node.salt(), node.title(), true, |ui| {
                                    camera_rows(ui, cx, rows, requests);
                                });
                            }
                            BrowserNode::Schedules => {
                                tree_node(ui, node.salt(), node.title(), false, |ui| {
                                    if rows.is_empty() {
                                        ui.weak("None");
                                    }
                                    for e in rows {
                                        if let BrowserItem::Schedule { floor, id } = e.item {
                                            let on = floor == cx.floor
                                                && crate::editor::schedule_view::is_selected(
                                                    cx, id,
                                                );
                                            if ui
                                                .selectable_label(on, &e.label)
                                                .on_hover_text("Jump to the schedule")
                                                .clicked()
                                                && jump_to_schedule(cx, floor, id)
                                            {
                                                if let Some(at) = schedule_center(cx, id) {
                                                    requests.push(DockRequest::PanTo(at));
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                            BrowserNode::CadDetails => {
                                tree_node(ui, node.salt(), node.title(), false, |ui| {
                                    if rows.is_empty() {
                                        ui.weak("None");
                                    }
                                    for e in rows {
                                        if let BrowserItem::CadDetail { floor, group } = e.item {
                                            if ui
                                                .selectable_label(false, &e.label)
                                                .on_hover_text("Jump to the detail and select it")
                                                .clicked()
                                                && jump_to_cad_detail(cx, floor, group)
                                            {
                                                if let Some(at) =
                                                    cad_detail_center(cx, floor, group)
                                                {
                                                    requests.push(DockRequest::PanTo(at));
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                            BrowserNode::Layout => {}
                        }
                    }
                },
            );
            tree_node(ui, "pb_layout", "Layout", false, |ui| {
                layout_section(ui, cx, requests)
            });
        });
}

/// A plan view tab being dragged to a new place in the strip.
struct TabDrag(usize);

/// Plan views open as tabs above the canvas (shown from two tabs up): click a
/// tab to switch to it, its x to close it, drag to reorder, "+" to open
/// another saved view. Each tab keeps its floor, layer set, reference display
/// and zoom (see `editor::plan_tabs`). A top panel of its own: the app shows
/// it each frame after the side panels and before the central panel, so the
/// drawing area starts below the strip instead of the strip covering it.
pub fn plan_view_tab_strip(ctx: &egui::Context, cx: &mut EditorContext) {
    use crate::editor::plan_tabs::with_tabs;
    if super::layout_window::is_active() {
        return;
    }
    let tabs: Vec<String> = with_tabs(|t| {
        t.sync(&cx.project);
        t.open().to_vec()
    });
    if tabs.len() < 2 {
        return;
    }
    let chrome = theme::current_chrome();
    let active = cx.project.active_plan_view.clone();
    let others: Vec<String> = cx
        .project
        .plan_views
        .iter()
        .map(|v| v.name.clone())
        .filter(|n| !tabs.contains(n))
        .collect();
    let mut switch: Option<String> = None;
    let mut close: Option<String> = None;
    let mut open: Option<String> = None;
    let mut mv: Option<(usize, usize)> = None;
    egui::TopBottomPanel::top("plan_view_tabs")
        .frame(egui::Frame::NONE.fill(chrome.status))
        .show_separator_line(false)
        .show(ctx, |ui| {
            ui.scope(|ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("plan_view_tabs_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            for (i, name) in tabs.iter().enumerate() {
                                let on = *name == active;
                                let r = ui
                                    .selectable_label(on, name)
                                    .interact(Sense::click_and_drag())
                                    .on_hover_text("Click to switch; drag to reorder");
                                r.dnd_set_drag_payload(TabDrag(i));
                                if r.clicked() && !on {
                                    switch = Some(name.clone());
                                }
                                if r.middle_clicked() {
                                    close = Some(name.clone());
                                }
                                if let Some(p) = r.dnd_release_payload::<TabDrag>() {
                                    if p.0 != i {
                                        mv = Some((p.0, i));
                                    }
                                }
                                if ui
                                    .add(egui::Button::new("\u{2715}").small().frame(false))
                                    .on_hover_text("Close this tab")
                                    .clicked()
                                {
                                    close = Some(name.clone());
                                }
                                ui.add_space(4.0);
                            }
                            if !others.is_empty() {
                                ui.menu_button("+", |ui| {
                                    for n in &others {
                                        if ui.button(n).clicked() {
                                            open = Some(n.clone());
                                            ui.close_menu();
                                        }
                                    }
                                })
                                .response
                                .on_hover_text("Open a saved plan view as a tab");
                            }
                        });
                    });
            });
        });
    with_tabs(|t| {
        if let Some((a, b)) = mv {
            t.move_tab(a, b);
        }
        if let Some(n) = &switch {
            t.switch_to(cx, n);
        }
        if let Some(n) = &open {
            t.open_view(cx, n);
        }
        if let Some(n) = &close {
            t.close(cx, n);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        crate::editor::plan_tabs::plain_cx()
    }

    #[test]
    fn disp_checkbox_hides_the_layer_and_undo_restores_it() {
        let mut cx = cx();
        assert!(cx.layers().is_visible("Doors"));
        assert!(set_layer_display(&mut cx, "Doors", false));
        assert!(!cx.project.layers.get("Doors").unwrap().display);
        assert!(!cx.layers().is_visible("Doors"));
        assert!(cx.is_dirty(), "the plan must redraw at once");
        assert_eq!(cx.undo_label(), Some("Layer Display"));
        cx.undo();
        assert!(cx.layers().is_visible("Doors"));
        // Unknown layers are refused and leave no undo step.
        assert!(!set_layer_display(&mut cx, "Nope", false));
    }

    #[test]
    fn lock_and_property_edits_write_the_shown_set() {
        use plan_core::LineStyle;
        let mut cx = cx();
        assert!(set_layer_locked(&mut cx, "Text", true));
        assert!(cx.layers().is_locked("Text"));
        let text = vec!["Text".to_string()];
        edit_layers(&mut cx, &text, LayerEdit::Color([1, 2, 3]), false, true);
        edit_layers(&mut cx, &text, LayerEdit::LineWeight(35), false, true);
        edit_layers(
            &mut cx,
            &text,
            LayerEdit::LineStyle(LineStyle::Dashed),
            false,
            false,
        );
        edit_layers(
            &mut cx,
            &text,
            LayerEdit::TextStyle("Arial".into()),
            false,
            false,
        );
        let l = cx.layers().get("Text").unwrap();
        assert_eq!(l.color, [1, 2, 3]);
        assert_eq!(l.line_weight, 35);
        assert_eq!(l.line_style, LineStyle::Dashed);
        assert_eq!(l.text_style, "Arial");
        // The look lives in the layer set; the plan's own layer is unchanged.
        assert_ne!(cx.project.layers.get("Text").unwrap().color, [1, 2, 3]);
        assert!(set_layer_reference(&mut cx, "Text", false));
        assert!(!cx.layers().shows_in_reference("Text"));
        assert_eq!(
            edit_layers(
                &mut cx,
                &["Nope".to_string()],
                LayerEdit::Display(false),
                false,
                false
            ),
            0
        );
    }

    #[test]
    fn layer_display_edits_persist_per_set_and_modify_all_changes_every_set() {
        use crate::dialogs::layer_sets;
        let mut cx = cx();
        layer_sets::copy_set(&mut cx, "Default Set", "Second").unwrap();
        layer_sets::copy_set(&mut cx, "Default Set", "Third").unwrap();
        let doors = vec!["Doors".to_string()];
        // Edit in Second only.
        assert!(set_active_layer_set(&mut cx, "Second"));
        edit_layers(&mut cx, &doors, LayerEdit::Color([7, 7, 7]), false, false);
        edit_layers(&mut cx, &doors, LayerEdit::Display(false), false, false);
        assert!(!cx.layers().is_visible("Doors"));
        // Back in the Default Set the edits are not there ...
        assert!(set_active_layer_set(&mut cx, "Default Set"));
        cx.refresh();
        assert!(cx.layers().is_visible("Doors"));
        assert_ne!(cx.layers().get("Doors").unwrap().color, [7, 7, 7]);
        // ... and they come back with the set.
        assert!(set_active_layer_set(&mut cx, "Second"));
        cx.refresh();
        assert!(!cx.layers().is_visible("Doors"));
        assert_eq!(cx.layers().get("Doors").unwrap().color, [7, 7, 7]);
        // The file keeps them per set.
        let back = plan_core::Project::from_json(&cx.project.to_json().unwrap()).unwrap();
        let eff = back.layer_sets.effective_for("Second", &back.layers);
        assert_eq!(eff.get("Doors").unwrap().color, [7, 7, 7]);
        assert_ne!(
            back.layer_sets
                .effective_for("Third", &back.layers)
                .get("Doors")
                .unwrap()
                .color,
            [7, 7, 7]
        );
        // Modify All Layer Sets reaches every set.
        let rooms = vec!["Rooms".to_string()];
        assert_eq!(
            edit_layers(&mut cx, &rooms, LayerEdit::Color([5, 6, 7]), true, false),
            3
        );
        for set in ["Default Set", "Second", "Third"] {
            let eff = cx.project.layer_sets.effective_for(set, &cx.project.layers);
            assert_eq!(eff.get("Rooms").unwrap().color, [5, 6, 7], "{set}");
        }
        // One undo step undoes it in all of them.
        cx.undo();
        for set in ["Default Set", "Second", "Third"] {
            let eff = cx.project.layer_sets.effective_for(set, &cx.project.layers);
            assert_ne!(eff.get("Rooms").unwrap().color, [5, 6, 7], "{set}");
        }
    }

    #[test]
    fn name_filter_narrows_the_rows() {
        let cx = cx();
        let all = layer_rows(&cx.project, "");
        assert_eq!(all.len(), cx.project.layers.layers.len());
        let walls = layer_rows(&cx.project, "  WALLS ");
        assert!(walls.len() >= 2 && walls.iter().all(|n| n.to_lowercase().contains("walls")));
        assert_eq!(layer_rows(&cx.project, "dimensions").len(), 2);
        assert!(layer_rows(&cx.project, "zzz").is_empty());
    }

    #[test]
    fn used_counts_objects_on_each_layer() {
        let mut cx = cx();
        assert!(layer_usage(&cx.project).is_empty());
        let a = cx.project.add_wall(
            0,
            Point::new(0.0, 0.0),
            Point::new(120.0, 0.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        cx.project.add_wall(
            0,
            Point::new(120.0, 0.0),
            Point::new(120.0, 100.0),
            6.0,
            100.0,
            WallKind::Exterior,
        );
        let _ = a;
        let used = layer_usage(&cx.project);
        assert_eq!(used.get("Walls, Normal"), Some(&2));
        assert_eq!(used.get("Doors"), None);
    }

    #[test]
    fn new_layer_gets_a_unique_name_and_is_undoable() {
        let mut cx = cx();
        let before = cx.project.layers.layers.len();
        let a = add_layer(&mut cx);
        let b = add_layer(&mut cx);
        assert_ne!(a, b);
        assert_eq!(cx.project.layers.layers.len(), before + 2);
        cx.undo();
        assert_eq!(cx.project.layers.layers.len(), before + 1);
    }

    #[test]
    fn every_panel_and_dialog_draws_frames_without_panicking() {
        let ctx = egui::Context::default();
        let mut cx = cx();
        cx.project.layers.add(Layer::new("Extra", [10, 20, 30], 18));
        let mut st = DockState::default();
        st.layers.selected = Some("Doors".into());
        st.library.query = "toilet".into();
        let mut hk = HotkeyState::new(super::super::hotkeys::HotkeyMap::defaults());
        st.open_hotkey_dialog(&hk);
        st.open_layer_dialog();
        for _ in 0..3 {
            for dock in [Dock::LayerDisplay, Dock::Project, Dock::Library] {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    egui::SidePanel::right("dock").show(ctx, |ui| show(ui, dock, &mut cx, &mut st));
                    show_dialogs(ctx, &mut cx, &mut st, &mut hk);
                });
            }
        }
        assert!(st.modal_open());
        assert!(hk.suspended, "an open hotkey dialog silences the hotkeys");
    }

    #[test]
    fn dock_requests_and_modal_flag() {
        let mut st = DockState::default();
        assert!(!st.modal_open());
        st.open_layer_dialog();
        assert!(st.modal_open());
        st.layer_dialog = None;
        st.library.activate("core.plumbing.toilet_elongated");
        assert_eq!(
            st.library.active_item.as_deref(),
            Some("core.plumbing.toilet_elongated")
        );
    }

    #[test]
    fn the_project_browser_lists_every_node() {
        use plan_core::cad::CadItem;
        let mut cx = cx();
        cx.project.cameras.clear();
        cx.project.add_camera(plan_core::CameraObject::new(
            plan_core::camera::CameraKind::FullCamera,
            Point::new(10.0, 10.0),
            0.0,
            "",
            0,
        ));
        // A schedule on the first floor and a named CAD block.
        crate::editor::schedule_view::add(
            &mut cx,
            plan_core::schedules::ScheduleKind::Door,
            Point::new(0.0, 0.0),
        );
        let ids: Vec<_> = (0..2)
            .map(|i| {
                cx.project.add_cad(
                    0,
                    "CAD, Default",
                    CadItem::Line {
                        a: Point::new(i as f64, 0.0),
                        b: Point::new(i as f64 + 5.0, 3.0),
                    },
                )
            })
            .collect();
        let refs: Vec<_> = ids
            .iter()
            .map(|i| plan_core::groups::ObjectRef::Cad(*i))
            .collect();
        let g = cx.project.make_group(0, &refs).unwrap();
        cx.project.floors[0]
            .cad_blocks
            .push(plan_core::cad::CadBlockInfo {
                group: g,
                name: "Footing Detail".into(),
                insertion: None,
                backoff: None,
            });
        let nodes = browser_nodes(&cx);
        let titles: Vec<&str> = nodes.iter().map(|(n, _)| n.title()).collect();
        assert_eq!(
            titles,
            [
                "Floors",
                "Plan Views",
                "Cameras",
                "Schedules",
                "CAD Details",
                "Layout"
            ]
        );
        let rows = |n: BrowserNode| nodes.iter().find(|(k, _)| *k == n).unwrap().1.clone();
        assert_eq!(rows(BrowserNode::Floors).len(), cx.project.floors.len());
        assert_eq!(rows(BrowserNode::PlanViews)[0].label, "Floor Plan View");
        assert_eq!(rows(BrowserNode::Cameras).len(), 1);
        let sched = rows(BrowserNode::Schedules);
        assert_eq!(sched.len(), 1);
        assert!(matches!(
            sched[0].item,
            BrowserItem::Schedule { floor: 0, .. }
        ));
        let det = rows(BrowserNode::CadDetails);
        assert_eq!(det.len(), 1);
        assert_eq!(det[0].label, "Footing Detail");
        assert!(rows(BrowserNode::Layout).is_empty(), "no layout yet");
        // Jumps: the schedule from another floor, then the detail.
        cx.project.insert_floor_above(0).unwrap();
        cx.floor = 1;
        let BrowserItem::Schedule { floor, id } = sched[0].item else {
            unreachable!()
        };
        assert!(jump_to_schedule(&mut cx, floor, id));
        assert_eq!(cx.floor, 0);
        assert_eq!(cx.selection.single(), Some(ObjectRef::Schedule(id)));
        // The plan pans to the middle of the table.
        let (lo, hi) = {
            let e = crate::editor::schedule_view::extents(&cx);
            let e = e.iter().find(|x| x.0 == id).unwrap();
            (e.1, e.2)
        };
        let at = schedule_center(&cx, id).expect("a table to pan to");
        assert!(at.dist(Point::lerp(lo, hi, 0.5)) < 1e-9);
        assert!(at.x > lo.x && at.x < hi.x && at.y > lo.y && at.y < hi.y);
        assert!(schedule_center(&cx, 9999).is_none());
        cx.floor = 1;
        assert!(jump_to_cad_detail(&mut cx, 0, g));
        assert_eq!(cx.floor, 0);
        assert_eq!(cx.selection.len(), 2);
        // ... and to the middle of the detail's objects.
        let mid = cad_detail_center(&cx, 0, g).expect("a detail to pan to");
        let ids = cx.project.floors[0].group_members_cad(g);
        let (mut lo, mut hi) = (
            Point::new(f64::MAX, f64::MAX),
            Point::new(f64::MIN, f64::MIN),
        );
        for o in cx.project.floors[0]
            .cad
            .iter()
            .filter(|o| ids.contains(&o.id))
        {
            let (a, b) = o.bounds();
            lo = Point::new(lo.x.min(a.x), lo.y.min(a.y));
            hi = Point::new(hi.x.max(b.x), hi.y.max(b.y));
        }
        assert!(mid.dist(Point::lerp(lo, hi, 0.5)) < 1e-9);
        assert!(cad_detail_center(&cx, 0, 9999).is_none());
        assert!(cad_detail_center(&cx, 5, g).is_none());
        assert!(!jump_to_cad_detail(&mut cx, 0, 9999));
        assert!(!jump_to_schedule(&mut cx, 0, 9999));
        assert!(!jump_to_schedule(&mut cx, 7, id));
    }

    #[test]
    fn cameras_rename_and_delete_with_undo() {
        let mut cx = cx();
        let id = cx.project.add_camera(plan_core::CameraObject::new(
            plan_core::camera::CameraKind::FullCamera,
            Point::new(5.0, 5.0),
            0.0,
            "",
            0,
        ));
        assert!(rename_camera(&mut cx, id, "  Living Room  "));
        assert_eq!(cx.project.camera(id).unwrap().name, "Living Room");
        assert_eq!(cx.undo_label(), Some("Rename Camera"));
        assert!(!rename_camera(&mut cx, id, "   "));
        assert!(!rename_camera(&mut cx, 9999, "X"));
        let rows = browser_nodes(&cx);
        let cams = &rows
            .iter()
            .find(|(n, _)| *n == BrowserNode::Cameras)
            .unwrap()
            .1;
        assert_eq!(cams[0].label, "Living Room");
        cx.selection.set(ObjectRef::Camera(id));
        assert!(delete_camera(&mut cx, id));
        assert!(cx.project.camera(id).is_none());
        assert!(cx.selection.is_empty());
        assert!(!delete_camera(&mut cx, id));
        cx.undo();
        assert!(cx.project.camera(id).is_some());
    }

    #[test]
    fn plan_view_tabs_draw_above_the_canvas() {
        use crate::editor::plan_tabs::with_tabs;
        let ctx = egui::Context::default();
        let mut cx = cx();
        cx.project
            .add_plan_view(plan_core::SavedPlanView::new("Second View", "Default Set"));
        with_tabs(|t| t.open_view(&mut cx, "Second View"));
        let mut top = 0.0_f32;
        for _ in 0..3 {
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                let before = ctx.available_rect().top();
                plan_view_tab_strip(ctx, &mut cx);
                // A real panel: the space for the drawing starts below it.
                top = ctx.available_rect().top() - before;
            });
        }
        assert!(top > 8.0, "the strip takes {top} px off the drawing area");
        assert!(with_tabs(|t| t.open().len()) >= 2);
        // With one tab there is no strip and nothing is taken.
        with_tabs(|t| {
            t.close(&mut cx, "Second View");
        });
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            let before = ctx.available_rect().top();
            plan_view_tab_strip(ctx, &mut cx);
            top = ctx.available_rect().top() - before;
        });
        assert!(top.abs() < 1e-3, "{top}");
        // The Project Browser draws every node, including the new ones.
        let mut st = DockState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::SidePanel::right("dock")
                .show(ctx, |ui| show(ui, Dock::Project, &mut cx, &mut st));
        });
    }
}
