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

use super::hotkeys::HotkeyState;
use super::library_browser::{self, LibraryBrowserState, LibraryEvent};
use crate::dialogs::hotkeys::HotkeyDialog;
use crate::dialogs::layer_display::LayerDisplayDialog;
use crate::dialogs::Outcome;
use crate::editor::EditorContext;
use crate::toolbar::Dock;
use crate::tools::ToolId;
use eframe::egui::{self, Color32, Sense, Stroke, Vec2};
use plan_core::{DimensionKind, Layer, LineStyle, OpeningKind, Project};
use std::collections::HashMap;

/// Something a dock panel needs the application to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockRequest {
    SetTool(ToolId),
    SwitchFloor(usize),
}

/// State of the layer table (the dock and the modal share the widget).
#[derive(Default)]
pub struct LayerPanelState {
    /// The Name Filter text box.
    pub filter: String,
    /// The layer whose "Properties for Selected Layer" show.
    pub selected: Option<String>,
}

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
    ui.heading(dock.title());
    ui.separator();
    match dock {
        Dock::LayerDisplay => layer_panel(ui, cx, &mut st.layers, 0.55),
        Dock::Project => project_browser(ui, cx, &mut st.requests),
        Dock::Library => match library_browser::show(ui, &mut st.library) {
            Some(LibraryEvent::Activate(id)) => {
                // The Library tool keeps the id it places; the browser keeps
                // its own copy to highlight the row.
                if st.library.activate(&id) && crate::tools::library::set_active_item(cx, &id) {
                    st.requests.push(DockRequest::SetTool(ToolId::Library));
                }
            }
            Some(LibraryEvent::Message(m)) => cx.status = m,
            None => {}
        },
    }
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

/// Edits layer `name` as one undoable step. `merged` lets a drag share one
/// step. Returns false when the layer does not exist.
pub fn edit_layer(
    cx: &mut EditorContext,
    name: &str,
    label: &str,
    merged: bool,
    edit: impl FnOnce(&mut Layer),
) -> bool {
    if cx.project.layers.get(name).is_none() {
        return false;
    }
    if merged {
        cx.begin_change_merged(label);
    } else {
        cx.begin_change(label);
    }
    if let Some(layer) = cx.project.layers.get_mut(name) {
        edit(layer);
    }
    cx.mark_dirty();
    true
}

/// Makes `name` the active layer set (one undo step).
pub fn set_active_layer_set(cx: &mut EditorContext, name: &str) -> bool {
    if cx.project.layer_sets.get(name).is_none() || cx.project.layer_sets.active == name {
        return false;
    }
    cx.begin_change("Active Layer Set");
    cx.project.layer_sets.set_active(name);
    cx.mark_dirty();
    true
}

/// Turns a layer's display on or off (the Disp checkbox).
pub fn set_layer_display(cx: &mut EditorContext, name: &str, on: bool) -> bool {
    let ok = edit_layer(cx, name, "Layer Display", false, |l| l.display = on);
    if ok {
        let set = cx.project.layer_sets.active.clone();
        if let Some(def) = cx.project.layer_sets.get_mut(&set) {
            def.ensure_state(name).display = on;
        }
    }
    ok
}

/// Locks or unlocks a layer (the Lock checkbox).
pub fn set_layer_locked(cx: &mut EditorContext, name: &str, locked: bool) -> bool {
    let ok = edit_layer(cx, name, "Layer Lock", false, |l| l.locked = locked);
    if ok {
        let set = cx.project.layer_sets.active.clone();
        if let Some(def) = cx.project.layer_sets.get_mut(&set) {
            def.ensure_state(name).locked = locked;
        }
    }
    ok
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

fn line_style_label(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash-Dot",
    }
}

const DEFAULT_TEXT_STYLE: &str = "(Default)";

/// The layer table and "Properties for Selected Layer". `table_share` is the
/// fraction of the free height the table may take.
pub fn layer_panel(
    ui: &mut egui::Ui,
    cx: &mut EditorContext,
    st: &mut LayerPanelState,
    table_share: f32,
) {
    ui.strong(format!(
        "Properties for Active Layer Set \u{2013} {}",
        cx.project.layer_sets.active
    ));
    let sets: Vec<String> = cx
        .project
        .layer_sets
        .names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label("Layer Set");
        egui::ComboBox::from_id_salt("active_layer_set")
            .selected_text(cx.project.layer_sets.active.clone())
            .show_ui(ui, |ui| {
                for s in &sets {
                    if ui
                        .selectable_label(*s == cx.project.layer_sets.active, s)
                        .clicked()
                    {
                        picked = Some(s.clone());
                    }
                }
            });
    });
    if let Some(s) = picked {
        set_active_layer_set(cx, &s);
    }
    ui.horizontal(|ui| {
        ui.label("Name Filter");
        ui.add(
            egui::TextEdit::singleline(&mut st.filter)
                .desired_width(ui.available_width())
                .hint_text("Type to filter"),
        );
    });
    ui.add_space(2.0);

    let rows = layer_rows(&cx.project, &st.filter);
    let used = layer_usage(&cx.project);
    let max_h = (ui.available_height() * table_share).max(120.0);
    let mut display_edit: Option<(String, bool)> = None;
    let mut lock_edit: Option<(String, bool)> = None;
    egui::ScrollArea::vertical()
        .id_salt("layer_table")
        .max_height(max_h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("layer_grid")
                .num_columns(5)
                .striped(true)
                .spacing(Vec2::new(8.0, 3.0))
                .show(ui, |ui| {
                    for h in ["Name", "Used", "Disp", "Lock", "Color"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for name in &rows {
                        let Some(layer) = cx.layers().get(name) else {
                            continue;
                        };
                        let selected = st.selected.as_deref() == Some(name.as_str());
                        if ui.selectable_label(selected, name).clicked() {
                            st.selected = Some(name.clone());
                        }
                        let count = used.get(name).copied().unwrap_or(0);
                        ui.label(count.to_string());
                        let mut disp = layer.display;
                        if ui.checkbox(&mut disp, "").changed() {
                            display_edit = Some((name.clone(), disp));
                        }
                        let mut locked = layer.locked;
                        if ui.checkbox(&mut locked, "").changed() {
                            lock_edit = Some((name.clone(), locked));
                        }
                        let [r, g, b] = layer.color;
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(22.0, 14.0), Sense::click());
                        ui.painter()
                            .rect_filled(rect, 2.0, Color32::from_rgb(r, g, b));
                        ui.painter().rect_stroke(
                            rect,
                            2.0,
                            Stroke::new(
                                1.0_f32,
                                ui.visuals().widgets.noninteractive.fg_stroke.color,
                            ),
                            egui::StrokeKind::Inside,
                        );
                        if resp.clicked() {
                            st.selected = Some(name.clone());
                        }
                        ui.end_row();
                    }
                });
            if rows.is_empty() {
                ui.weak("No layers match the filter.");
            }
        });
    if let Some((name, on)) = display_edit {
        set_layer_display(cx, &name, on);
    }
    if let Some((name, locked)) = lock_edit {
        set_layer_locked(cx, &name, locked);
    }

    ui.separator();
    selected_layer_properties(ui, cx, st);
}

fn selected_layer_properties(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut LayerPanelState) {
    let Some(name) = st.selected.clone() else {
        ui.weak("Select a layer to see its properties.");
        return;
    };
    let Some(original) = cx.project.layers.get(&name).cloned() else {
        st.selected = None;
        return;
    };
    ui.strong(format!("Properties for Selected Layer \u{2013} {name}"));
    let mut layer = original.clone();
    let mut label: Option<(&str, bool)> = None;
    let text_styles: Vec<String> = {
        let mut v: Vec<String> = cx
            .project
            .layers
            .layers
            .iter()
            .map(|l| l.text_style.clone())
            .filter(|s| !s.is_empty())
            .collect();
        v.sort();
        v.dedup();
        v
    };
    egui::Grid::new("layer_props")
        .num_columns(2)
        .spacing(Vec2::new(8.0, 4.0))
        .show(ui, |ui| {
            ui.label("Display");
            if ui.checkbox(&mut layer.display, "").changed() {
                label = Some(("Layer Display", false));
            }
            ui.end_row();

            ui.label("Lock");
            if ui.checkbox(&mut layer.locked, "").changed() {
                label = Some(("Layer Lock", false));
            }
            ui.end_row();

            ui.label("Color");
            if ui.color_edit_button_srgb(&mut layer.color).changed() {
                label = Some(("Layer Color", true));
            }
            ui.end_row();

            ui.label("Line Weight");
            let mut mm = layer.line_weight as f64 / 100.0;
            if ui
                .add(
                    egui::DragValue::new(&mut mm)
                        .speed(0.01)
                        .range(0.0..=5.0)
                        .max_decimals(2)
                        .suffix(" mm"),
                )
                .changed()
            {
                layer.line_weight = (mm * 100.0).round().max(0.0) as u32;
                label = Some(("Layer Line Weight", true));
            }
            ui.end_row();

            ui.label("Line Style");
            egui::ComboBox::from_id_salt("layer_line_style")
                .selected_text(line_style_label(layer.line_style))
                .show_ui(ui, |ui| {
                    for s in [
                        LineStyle::Solid,
                        LineStyle::Dashed,
                        LineStyle::Dotted,
                        LineStyle::DashDot,
                    ] {
                        if ui
                            .selectable_value(&mut layer.line_style, s, line_style_label(s))
                            .clicked()
                        {
                            label = Some(("Layer Line Style", false));
                        }
                    }
                });
            ui.end_row();

            ui.label("Text Style");
            let shown = if layer.text_style.is_empty() {
                DEFAULT_TEXT_STYLE.to_string()
            } else {
                layer.text_style.clone()
            };
            egui::ComboBox::from_id_salt("layer_text_style")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(layer.text_style.is_empty(), DEFAULT_TEXT_STYLE)
                        .clicked()
                    {
                        layer.text_style.clear();
                        label = Some(("Layer Text Style", false));
                    }
                    for s in &text_styles {
                        if ui.selectable_label(layer.text_style == *s, s).clicked() {
                            layer.text_style = s.clone();
                            label = Some(("Layer Text Style", false));
                        }
                    }
                });
            ui.end_row();

            ui.label("Fill Style");
            ui.add_enabled(false, egui::Label::new("Solid (not stored yet)"));
            ui.end_row();
        });
    if let Some((label, merged)) = label {
        if layer != original {
            edit_layer(cx, &name, label, merged, |l| *l = layer);
        }
    }
}

// ----- project browser -----

fn project_browser(ui: &mut egui::Ui, cx: &mut EditorContext, requests: &mut Vec<DockRequest>) {
    egui::ScrollArea::vertical()
        .id_salt("project_browser")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::CollapsingHeader::new(format!("Plan \u{2013} {}", cx.project.name))
                .id_salt("pb_plan")
                .default_open(true)
                .show(ui, |ui| {
                    egui::CollapsingHeader::new("Floors")
                        .id_salt("pb_floors")
                        .default_open(true)
                        .show(ui, |ui| {
                            for (i, floor) in cx.project.floors.iter().enumerate() {
                                let current = i == cx.floor;
                                if ui.selectable_label(current, &floor.name).clicked() && !current {
                                    requests.push(DockRequest::SwitchFloor(i));
                                }
                            }
                        });
                    egui::CollapsingHeader::new("Cameras")
                        .id_salt("pb_cameras")
                        .default_open(true)
                        .show(ui, |ui| {
                            if cx.project.cameras.is_empty() {
                                ui.weak("None");
                            }
                            for cam in &cx.project.cameras {
                                let name = if cam.name.is_empty() {
                                    format!("Camera {}", cam.id)
                                } else {
                                    cam.name.clone()
                                };
                                ui.label(name);
                            }
                        });
                    egui::CollapsingHeader::new("Saved Views")
                        .id_salt("pb_views")
                        .default_open(true)
                        .show(ui, |ui| {
                            let _ = ui.selectable_label(true, "Floor Plan View");
                        });
                });
            egui::CollapsingHeader::new("Layout")
                .id_salt("pb_layout")
                .default_open(false)
                .show(ui, |ui| {
                    ui.weak("Layout pages: coming");
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;
    use plan_core::geometry::Point;
    use plan_core::WallKind;

    fn cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
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
    fn lock_and_property_edits_write_the_layer() {
        let mut cx = cx();
        assert!(set_layer_locked(&mut cx, "Text", true));
        assert!(cx.layers().is_locked("Text"));
        assert!(edit_layer(&mut cx, "Text", "Layer Color", true, |l| {
            l.color = [1, 2, 3];
            l.line_weight = 35;
            l.line_style = LineStyle::Dashed;
            l.text_style = "Arial".into();
        }));
        let l = cx.project.layers.get("Text").unwrap();
        assert_eq!(l.color, [1, 2, 3]);
        assert_eq!(l.line_weight, 35);
        assert_eq!(l.line_style, LineStyle::Dashed);
        assert_eq!(l.text_style, "Arial");
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
}
