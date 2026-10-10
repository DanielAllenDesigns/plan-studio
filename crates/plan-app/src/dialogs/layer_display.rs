//! Layer Display Options: the layer table of the right-hand dock and the same
//! table as a modal window.
//!
//! The table has the columns Name, Used, Disp, Lock, Ref, Color, Weight, Line
//! Style and Text Style. Every cell edits the layer in the *shown* layer set
//! (or in every set while "Modify All Layer Sets" is on), for all selected
//! rows at once when the edited row is part of a multi-selection (click,
//! Cmd/Ctrl-click to toggle, Shift-click for a range). Above the table are
//! the layer set selector with New, Copy, Rename and Delete, a name filter,
//! Select All / None, Reset and Select Objects on the layers; below it the
//! properties of the selection and Copy To Other Sets.
//!
//! Layer edits go through `docks::edit_layers`, which is one undo step per
//! edit (a drag shares one).

use crate::dialogs::layer_sets as sets;
use crate::editor::selection::{all_selectable, layer_of};
use crate::editor::EditorContext;
use crate::shell::docks::{add_layer, edit_layers, layer_rows, search_field, tree_node};
use eframe::egui::{self, Align, Align2, Key, Layout, Modifiers, Vec2};
use plan_core::fill_styles::{FillStyle, FillTarget};
use plan_core::layer_sets::LayerEdit;
use plan_core::layers::{common_props, is_system_layer, LayerCommon, LayerUse, Mixed};
use plan_core::{Layer, LineStyle};
use std::collections::BTreeSet;

/// What the set buttons are asking a name for.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SetMode {
    #[default]
    None,
    New,
    Copy,
    Rename,
}

/// The table can be sorted by these columns.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum SortKey {
    /// The order of the plan's layer list.
    #[default]
    Plan,
    Name,
    Used,
    Disp,
    Lock,
    Ref,
    Weight,
}

/// State of the layer table (the dock and the modal share the widget).
#[derive(Default)]
pub struct LayerPanelState {
    /// The Name Filter text box.
    pub filter: String,
    /// The layer whose "Properties for Selected Layer" show (the last one
    /// clicked).
    pub selected: Option<String>,
    /// Every selected row.
    pub multi: BTreeSet<String>,
    /// The row a Shift-click range starts from.
    pub anchor: Option<String>,
    /// "Modify All Layer Sets": edits go to every set.
    pub all_sets: bool,
    pub sort: SortKey,
    pub descending: bool,
    pub set_mode: SetMode,
    pub set_name: String,
    /// Sets ticked in "Copy To Other Sets".
    pub copy_targets: BTreeSet<String>,
    /// The last message of a set button or command.
    pub message: String,
    /// The New button is asking for a layer name.
    pub naming: bool,
    /// The text of the New Layer Name box.
    pub new_name: String,
    /// Only these layers are listed (Object Layer Properties: the layers of
    /// the selected objects); `None` lists them all.
    pub only: Option<Vec<String>>,
    /// The table's heading, when it is not "Properties for Active Layer Set".
    pub heading: Option<String>,
    /// Object Layer Properties: no layer set controls, no management
    /// buttons.
    pub compact: bool,
    /// While objects are selected the dock lists their layers (Object Layer
    /// Properties, manual p. 211); this shows every layer instead (the Show
    /// All Layers box).
    pub show_all_layers: bool,
}

impl LayerPanelState {
    /// Selects exactly `name`.
    pub fn select_only(&mut self, name: &str) {
        self.multi.clear();
        self.multi.insert(name.to_string());
        self.selected = Some(name.to_string());
        self.anchor = Some(name.to_string());
    }

    /// The selected layers in the plan's layer order.
    pub fn selected_layers(&self, cx: &EditorContext) -> Vec<String> {
        cx.project
            .layers
            .layers
            .iter()
            .filter(|l| self.multi.contains(&l.name) || self.selected.as_deref() == Some(&l.name))
            .map(|l| l.name.clone())
            .collect()
    }

    /// The layers an edit of the cell of `row` applies to: the whole selection
    /// when the row is part of a multi-selection, else the row alone.
    pub fn targets_of(&self, cx: &EditorContext, row: &str) -> Vec<String> {
        if self.multi.len() > 1 && self.multi.contains(row) {
            self.selected_layers(cx)
        } else {
            vec![row.to_string()]
        }
    }

    /// Click handling of a row name: plain click selects it, Cmd/Ctrl toggles,
    /// Shift extends from the anchor over `rows`.
    pub fn click_row(&mut self, name: &str, rows: &[String], mods: Modifiers) {
        if mods.shift {
            let anchor = self.anchor.clone().unwrap_or_else(|| name.to_string());
            let a = rows.iter().position(|r| *r == anchor);
            let b = rows.iter().position(|r| r == name);
            if let (Some(a), Some(b)) = (a, b) {
                let (lo, hi) = (a.min(b), a.max(b));
                self.multi = rows[lo..=hi].iter().cloned().collect();
                self.selected = Some(name.to_string());
                return;
            }
        }
        if mods.command || mods.ctrl {
            if !self.multi.remove(name) {
                self.multi.insert(name.to_string());
                self.selected = Some(name.to_string());
            } else if self.selected.as_deref() == Some(name) {
                self.selected = self.multi.iter().next().cloned();
            }
            self.anchor = Some(name.to_string());
            return;
        }
        self.select_only(name);
    }

    /// Select All: every row the filter shows.
    pub fn select_all(&mut self, rows: &[String]) {
        self.multi = rows.iter().cloned().collect();
        self.selected = rows.first().cloned();
        self.anchor = self.selected.clone();
    }

    /// Select None.
    pub fn select_none(&mut self) {
        self.multi.clear();
        self.selected = None;
        self.anchor = None;
    }
}

/// Select All on Layer: selects every object of the shown floor on `layers`
/// (displayed and unlocked layers only: hidden and locked layers hold nothing
/// selectable). Returns how many.
pub fn select_all_on_layers(cx: &mut EditorContext, layers: &[String]) -> usize {
    let found: Vec<_> = all_selectable(cx)
        .into_iter()
        .filter(|o| layer_of(cx.floor(), *o).is_some_and(|l| layers.contains(&l)))
        .collect();
    crate::editor::rooms_edit::clear_room_selection();
    cx.selection.items = found;
    cx.selection.len()
}

fn line_style_label(s: LineStyle) -> &'static str {
    match s {
        LineStyle::Solid => "Solid",
        LineStyle::Dashed => "Dashed",
        LineStyle::Dotted => "Dotted",
        LineStyle::DashDot => "Dash-Dot",
    }
}

const LINE_STYLES: [LineStyle; 4] = [
    LineStyle::Solid,
    LineStyle::Dashed,
    LineStyle::Dotted,
    LineStyle::DashDot,
];

const DEFAULT_TEXT_STYLE: &str = "(Default)";

/// One row of the table.
struct Row {
    name: String,
    layer: Layer,
    used: usize,
    /// Where the layer is used (the Used column icon and tool tip).
    info: LayerUse,
    /// The layer's Fill Style (the Fill column); `None` is no fill.
    fill: Option<FillStyle>,
}

/// The rows the filter shows, in the chosen sort order.
fn table_rows(cx: &EditorContext, st: &LayerPanelState) -> Vec<Row> {
    let used = cx.project.layer_object_counts();
    let mut rows: Vec<Row> = layer_rows(&cx.project, &st.filter)
        .into_iter()
        .filter_map(|name| {
            let layer = cx.layers().get(&name)?.clone();
            let used = used.get(&name).copied().unwrap_or(0);
            let info = LayerUse {
                objects: used,
                defaults: cx.project.layer_defaults(&name),
                system: is_system_layer(&name),
            };
            let fill = cx
                .project
                .styles
                .fill_for(&FillTarget::Layer(name.clone()))
                .cloned();
            Some(Row {
                name,
                layer,
                used,
                info,
                fill,
            })
        })
        .filter(|r| st.only.as_ref().is_none_or(|only| only.contains(&r.name)))
        .collect();
    sort_rows(&mut rows, st.sort, st.descending);
    rows
}

fn sort_rows(rows: &mut [Row], key: SortKey, descending: bool) {
    match key {
        SortKey::Plan => {}
        SortKey::Name => rows.sort_by_key(|r| r.name.to_lowercase()),
        SortKey::Used => rows.sort_by_key(|r| r.used),
        SortKey::Disp => rows.sort_by_key(|r| r.layer.display),
        SortKey::Lock => rows.sort_by_key(|r| r.layer.locked),
        SortKey::Ref => rows.sort_by_key(|r| r.layer.reference),
        SortKey::Weight => rows.sort_by_key(|r| r.layer.line_weight),
    }
    if descending && key != SortKey::Plan {
        rows.reverse();
    }
}

/// The names of the table rows in order (for Shift-click ranges and Select All).
#[cfg(test)]
pub fn visible_names(cx: &EditorContext, st: &LayerPanelState) -> Vec<String> {
    table_rows(cx, st).into_iter().map(|r| r.name).collect()
}

/// A pending cell edit: which layers, what, and whether a drag shares a step.
type Pending = (Vec<String>, LayerEdit, bool);

/// The layer table and "Properties for Selected Layer". `table_share` is the
/// fraction of the free height the table may take.
pub fn layer_panel(
    ui: &mut egui::Ui,
    cx: &mut EditorContext,
    st: &mut LayerPanelState,
    table_share: f32,
) {
    // With objects selected the table lists the layers they touch, as
    // Object Layer Properties does, until Show All Layers is ticked.
    if st.compact || cx.selection.is_empty() {
        return layer_panel_body(ui, cx, st, table_share);
    }
    ui.checkbox(&mut st.show_all_layers, "Show All Layers")
        .on_hover_text("Off lists only the layers of the selected objects");
    if st.show_all_layers {
        return layer_panel_body(ui, cx, st, table_share);
    }
    let saved = (st.only.take(), st.heading.take(), st.compact);
    st.only = Some(crate::dialogs::object_layers::layers_of_selection(cx).all());
    st.heading = Some(format!(
        "Properties of Object Layers \u{2013} {}",
        cx.project.shown_layer_set()
    ));
    st.compact = true;
    layer_panel_body(ui, cx, st, table_share);
    (st.only, st.heading, st.compact) = saved;
}

fn layer_panel_body(
    ui: &mut egui::Ui,
    cx: &mut EditorContext,
    st: &mut LayerPanelState,
    table_share: f32,
) {
    match &st.heading {
        Some(h) => ui.strong(h.clone()),
        None => ui.strong(format!(
            "Properties for Active Layer Set \u{2013} {}",
            cx.project.shown_layer_set()
        )),
    };
    if !st.compact {
        set_selector(ui, cx, st);
        ui.checkbox(&mut st.all_sets, "Modify All Layer Sets")
            .on_hover_text("Edits below change this layer in every layer set");
    }
    ui.label("Name Filter");
    search_field(ui, &mut st.filter, "Type to filter");
    ui.add_space(2.0);

    let rows = table_rows(cx, st);
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("Select All").clicked() {
            st.select_all(&names);
        }
        if ui.small_button("Select None").clicked() {
            st.select_none();
        }
        let sel = st.selected_layers(cx);
        if ui
            .add_enabled(!sel.is_empty(), egui::Button::new("Reset").small())
            .on_hover_text("Put the selected layers back to the plan's own look")
            .clicked()
        {
            reset_layers(cx, &sel, st.all_sets);
        }
        if ui
            .add_enabled(!sel.is_empty(), egui::Button::new("Select Objects").small())
            .on_hover_text("Select every object on the selected layers")
            .clicked()
        {
            let n = select_all_on_layers(cx, &sel);
            cx.status = format!("Selected {n} objects on {} layers", sel.len());
        }
    });

    let max_h = (ui.available_height() * table_share).max(120.0);
    let mut pending: Vec<Pending> = Vec::new();
    let mut click: Option<(String, Modifiers)> = None;
    let mut sort_click: Option<SortKey> = None;
    let mut context: Option<(String, ContextAction)> = None;
    let text_styles = text_style_names(cx);
    egui::ScrollArea::both()
        .id_salt("layer_table")
        .max_height(max_h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("layer_grid")
                .num_columns(10)
                .striped(true)
                .spacing(Vec2::new(8.0, 3.0))
                .show(ui, |ui| {
                    for (label, key) in [
                        ("Name", Some(SortKey::Name)),
                        ("Used", Some(SortKey::Used)),
                        ("Disp", Some(SortKey::Disp)),
                        ("Lock", Some(SortKey::Lock)),
                        ("Ref", Some(SortKey::Ref)),
                        ("Color", None),
                        ("Fill", None),
                        ("Weight", Some(SortKey::Weight)),
                        ("Line Style", None),
                        ("Text Style", None),
                    ] {
                        let text = match key {
                            Some(k) if k == st.sort => format!(
                                "{label} {}",
                                if st.descending {
                                    "\u{25BE}"
                                } else {
                                    "\u{25B4}"
                                }
                            ),
                            _ => label.to_string(),
                        };
                        match key {
                            Some(k) => {
                                if ui
                                    .add(
                                        egui::Label::new(egui::RichText::new(text).strong())
                                            .sense(egui::Sense::click()),
                                    )
                                    .on_hover_text("Click to sort")
                                    .clicked()
                                {
                                    sort_click = Some(k);
                                }
                            }
                            None => {
                                ui.strong(text);
                            }
                        }
                    }
                    ui.end_row();
                    for row in &rows {
                        draw_row(
                            ui,
                            cx,
                            st,
                            row,
                            &names,
                            &text_styles,
                            &mut pending,
                            &mut click,
                            &mut context,
                        );
                        ui.end_row();
                    }
                });
            if rows.is_empty() {
                ui.weak("No layers match the filter.");
            }
        });
    if let Some(k) = sort_click {
        if st.sort == k {
            if st.descending {
                st.sort = SortKey::Plan;
                st.descending = false;
            } else {
                st.descending = true;
            }
        } else {
            st.sort = k;
            st.descending = false;
        }
    }
    if let Some((name, mods)) = click {
        st.click_row(&name, &names, mods);
    }
    if let Some((name, action)) = context {
        if !st.multi.contains(&name) {
            st.select_only(&name);
        }
        let sel = st.selected_layers(cx);
        match action {
            ContextAction::SelectObjects => {
                let n = select_all_on_layers(cx, &sel);
                cx.status = format!("Selected {n} objects on {} layers", sel.len());
            }
            ContextAction::Reset => {
                reset_layers(cx, &sel, st.all_sets);
            }
            ContextAction::EditFill => {
                crate::dialogs::fill_style::open_for_layers(cx, &sel);
            }
            ContextAction::Find => {
                crate::dialogs::object_layers::find_objects_on_layers(cx, &sel);
            }
        }
    }
    for (layers, edit, merged) in pending {
        edit_layers(cx, &layers, edit, st.all_sets, merged);
    }

    if !st.compact {
        management_buttons(ui, cx, st);
    }
    ui.separator();
    tree_node(ui, "layer_selected_props", "Selected Layer", true, |ui| {
        selected_layer_properties(ui, cx, st, &text_styles);
    });
    if !st.compact {
        tree_node(
            ui,
            "layer_copy_to_sets",
            "Copy To Other Sets",
            false,
            |ui| {
                copy_to_sets(ui, cx, st);
            },
        );
    }
    if !st.message.is_empty() {
        ui.weak(&st.message);
    }
}

#[derive(Clone, Copy)]
enum ContextAction {
    SelectObjects,
    Reset,
    /// The Fill cell was clicked: open the Fill Style dialog.
    EditFill,
    /// Find Objects on Layer(s) (LAY-62).
    Find,
}

/// The text style names a layer may use: the plan's styles plus any name a
/// layer already holds.
fn text_style_names(cx: &EditorContext) -> Vec<String> {
    let mut v: Vec<String> = cx
        .project
        .text_styles
        .styles
        .iter()
        .map(|s| s.name.clone())
        .collect();
    for l in &cx.project.layers.layers {
        if !l.text_style.is_empty() && !v.contains(&l.text_style) {
            v.push(l.text_style.clone());
        }
    }
    v.sort();
    v.dedup();
    v
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    ui: &mut egui::Ui,
    cx: &EditorContext,
    st: &LayerPanelState,
    row: &Row,
    names: &[String],
    text_styles: &[String],
    pending: &mut Vec<Pending>,
    click: &mut Option<(String, Modifiers)>,
    context: &mut Option<(String, ContextAction)>,
) {
    let name = &row.name;
    let layer = &row.layer;
    let targets = st.targets_of(cx, name);
    let selected =
        st.multi.contains(name.as_str()) || st.selected.as_deref() == Some(name.as_str());
    let r = ui.selectable_label(selected, name);
    if r.clicked() {
        *click = Some((name.clone(), ui.input(|i| i.modifiers)));
    }
    r.context_menu(|ui| {
        if ui.button("Select All on Layer").clicked() {
            *context = Some((name.clone(), ContextAction::SelectObjects));
            ui.close_menu();
        }
        if ui.button("Find Objects on Layer(s)\u{2026}").clicked() {
            *context = Some((name.clone(), ContextAction::Find));
            ui.close_menu();
        }
        if ui.button("Reset to Defaults").clicked() {
            *context = Some((name.clone(), ContextAction::Reset));
            ui.close_menu();
        }
    });
    let _ = names;
    used_cell(ui, row);

    let mut disp = layer.display;
    if ui.checkbox(&mut disp, "").changed() {
        pending.push((targets.clone(), LayerEdit::Display(disp), false));
    }
    let mut locked = layer.locked;
    if ui.checkbox(&mut locked, "").changed() {
        pending.push((targets.clone(), LayerEdit::Locked(locked), false));
    }
    let mut reference = layer.reference;
    if ui
        .checkbox(&mut reference, "")
        .on_hover_text("Show this layer on the reference floor")
        .changed()
    {
        pending.push((targets.clone(), LayerEdit::Reference(reference), false));
    }
    let mut rgb = layer.color;
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        pending.push((targets.clone(), LayerEdit::Color(rgb), true));
    }
    if fill_cell(ui, cx, row).clicked() {
        *context = Some((name.clone(), ContextAction::EditFill));
    }
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
        pending.push((
            targets.clone(),
            LayerEdit::LineWeight((mm * 100.0).round().max(0.0) as u32),
            true,
        ));
    }
    let mut style = layer.line_style;
    egui::ComboBox::from_id_salt(("lt_style", name))
        .selected_text(line_style_label(style))
        .width(70.0)
        .show_ui(ui, |ui| {
            for s in LINE_STYLES {
                if ui
                    .selectable_value(&mut style, s, line_style_label(s))
                    .clicked()
                {
                    pending.push((targets.clone(), LayerEdit::LineStyle(s), false));
                }
            }
        });
    text_style_combo(ui, ("lt_text", name), &layer.text_style, text_styles, |t| {
        pending.push((targets.clone(), LayerEdit::TextStyle(t), false));
    });
}

/// The Fill column: a small swatch of the layer's Fill Style (an empty box
/// when it has none); a click opens the Fill Style dialog for the layer.
fn fill_cell(ui: &mut egui::Ui, cx: &EditorContext, row: &Row) -> egui::Response {
    let size = Vec2::new(30.0, 16.0);
    match &row.fill {
        Some(style) => {
            let patterns = cx.project.styles.all_patterns();
            crate::dialogs::fill_style::preview_sized(
                ui,
                style,
                &patterns,
                row.layer.color,
                24.0,
                size,
            )
            .on_hover_text(format!("Fill: {}. Click to edit", style.summary()))
        }
        None => {
            let (rect, r) = ui.allocate_exact_size(size, egui::Sense::click());
            ui.painter().rect_stroke(
                rect,
                2.0,
                ui.visuals().widgets.noninteractive.fg_stroke,
                egui::StrokeKind::Inside,
            );
            r.on_hover_text("No fill. Click to choose a Fill Style")
        }
    }
}

fn text_style_combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash,
    current: &str,
    styles: &[String],
    mut pick: impl FnMut(String),
) {
    let shown = if current.is_empty() {
        DEFAULT_TEXT_STYLE
    } else {
        current
    };
    egui::ComboBox::from_id_salt(salt)
        .selected_text(shown)
        .width(110.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_empty(), DEFAULT_TEXT_STYLE)
                .clicked()
            {
                pick(String::new());
            }
            for s in styles {
                if ui.selectable_label(current == s, s).clicked() {
                    pick(s.clone());
                }
            }
        });
}

/// The layer set selector with New, Copy, Rename, Delete and Manage.
fn set_selector(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut LayerPanelState) {
    let names: Vec<String> = cx
        .project
        .layer_sets
        .names()
        .into_iter()
        .map(str::to_string)
        .collect();
    let shown = cx.project.shown_layer_set().to_string();
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.label("Layer Set");
        egui::ComboBox::from_id_salt("active_layer_set")
            .selected_text(shown.clone())
            .show_ui(ui, |ui| {
                for s in &names {
                    if ui.selectable_label(*s == shown, s).clicked() {
                        picked = Some(s.clone());
                    }
                }
            });
    });
    if let Some(s) = picked {
        sets::activate_set(cx, &s);
    }
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("New").clicked() {
            st.set_mode = SetMode::New;
            st.set_name = cx.project.layer_sets.free_name("New Layer Set");
        }
        if ui.small_button("Copy Set").clicked() {
            st.set_mode = SetMode::Copy;
            st.set_name = cx.project.layer_sets.free_name(&format!("{shown} Copy"));
        }
        if ui.small_button("Rename").clicked() {
            st.set_mode = SetMode::Rename;
            st.set_name = shown.clone();
        }
        if ui.small_button("Delete").clicked() {
            st.message = match sets::delete_set(cx, &shown) {
                Ok(()) => format!("Deleted layer set {shown}"),
                Err(e) => e,
            };
        }
        if ui.small_button("Manage\u{2026}").clicked() {
            sets::open();
        }
    });
    if st.set_mode != SetMode::None {
        ui.horizontal(|ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut st.set_name).desired_width(140.0));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.small_button("OK").clicked() || enter {
                let name = st.set_name.clone();
                let res = match st.set_mode {
                    SetMode::New => sets::new_set(cx, &name),
                    SetMode::Copy => sets::copy_set(cx, &shown, &name),
                    SetMode::Rename => sets::rename_set(cx, &shown, &name),
                    SetMode::None => unreachable!(),
                };
                match res {
                    Ok(n) => {
                        // A new or copied set is the one to work in.
                        if st.set_mode != SetMode::Rename {
                            sets::activate_set(cx, &n);
                        }
                        st.message = format!("Layer set {n}");
                        st.set_mode = SetMode::None;
                    }
                    Err(e) => st.message = e,
                }
            }
            if ui.small_button("Cancel").clicked() {
                st.set_mode = SetMode::None;
            }
        });
    }
}

/// Reset: puts `layers` back to the plan's own look in the shown set (every
/// set when `all_sets`). One undo step.
pub fn reset_layers(cx: &mut EditorContext, layers: &[String], all_sets: bool) -> usize {
    let existing: Vec<String> = layers
        .iter()
        .filter(|l| cx.project.layers.get(l).is_some())
        .cloned()
        .collect();
    if existing.is_empty() {
        return 0;
    }
    cx.begin_change("Reset Layers");
    let n = cx.project.reset_layers(&existing, all_sets);
    cx.mark_dirty();
    cx.refresh_layer_view();
    n
}

/// Copy To Other Sets: copies how `layers` look in the shown set into the sets
/// `to`. One undo step.
pub fn copy_layers_to_sets(cx: &mut EditorContext, layers: &[String], to: &[String]) -> usize {
    if layers.is_empty() || to.is_empty() {
        return 0;
    }
    cx.begin_change("Copy Layers To Layer Sets");
    let from = cx.project.shown_layer_set().to_string();
    let n = cx.project.layer_sets.copy_layers_to(&from, layers, to);
    cx.mark_dirty();
    n
}

fn copy_to_sets(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut LayerPanelState) {
    let shown = cx.project.shown_layer_set().to_string();
    let others: Vec<String> = cx
        .project
        .layer_sets
        .names()
        .into_iter()
        .filter(|n| *n != shown)
        .map(str::to_string)
        .collect();
    if others.is_empty() {
        ui.weak("There is no other layer set.");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("copy_to_sets")
        .max_height(120.0)
        .show(ui, |ui| {
            for o in &others {
                let mut on = st.copy_targets.contains(o);
                if ui.checkbox(&mut on, o).changed() {
                    if on {
                        st.copy_targets.insert(o.clone());
                    } else {
                        st.copy_targets.remove(o);
                    }
                }
            }
        });
    ui.horizontal(|ui| {
        if ui.small_button("All").clicked() {
            st.copy_targets = others.iter().cloned().collect();
        }
        if ui.small_button("None").clicked() {
            st.copy_targets.clear();
        }
        let sel = st.selected_layers(cx);
        let to: Vec<String> = st
            .copy_targets
            .iter()
            .filter(|t| others.contains(t))
            .cloned()
            .collect();
        if ui
            .add_enabled(
                !sel.is_empty() && !to.is_empty(),
                egui::Button::new("Copy").small(),
            )
            .clicked()
        {
            let n = copy_layers_to_sets(cx, &sel, &to);
            st.message = format!("Copied {} layers to {} sets", n / to.len().max(1), to.len());
        }
    });
}

fn selected_layer_properties(
    ui: &mut egui::Ui,
    cx: &mut EditorContext,
    st: &mut LayerPanelState,
    text_styles: &[String],
) {
    let Some(name) = st.selected.clone() else {
        ui.weak("Select a layer to see its properties.");
        return;
    };
    let Some(layer) = cx.layers().get(&name).cloned() else {
        st.selected = None;
        return;
    };
    let targets = st.selected_layers(cx);
    let common = selection_common(cx, &targets, &layer);
    if targets.len() > 1 {
        ui.strong(format!(
            "Properties for {} Selected Layers \u{2013} {name}",
            targets.len()
        ));
    } else {
        ui.strong(format!("Properties for Selected Layer \u{2013} {name}"));
    }
    let mut pending: Vec<Pending> = Vec::new();
    let mut fill_open = false;
    let mut fill_clear = false;
    egui::Grid::new("layer_props")
        .num_columns(2)
        .spacing(Vec2::new(8.0, 4.0))
        .show(ui, |ui| {
            ui.label(tagged("Display", &common.display));
            let mut on = common.display.value().copied().unwrap_or(layer.display);
            if ui
                .add(egui::Checkbox::new(&mut on, "").indeterminate(common.display.is_mixed()))
                .changed()
            {
                pending.push((targets.clone(), LayerEdit::Display(on), false));
            }
            ui.end_row();

            ui.label(tagged("Lock", &common.locked));
            let mut on = common.locked.value().copied().unwrap_or(layer.locked);
            if ui
                .add(egui::Checkbox::new(&mut on, "").indeterminate(common.locked.is_mixed()))
                .changed()
            {
                pending.push((targets.clone(), LayerEdit::Locked(on), false));
            }
            ui.end_row();

            ui.label(tagged("Reference", &common.reference));
            let mut on = common.reference.value().copied().unwrap_or(layer.reference);
            if ui
                .add(egui::Checkbox::new(&mut on, "").indeterminate(common.reference.is_mixed()))
                .changed()
            {
                pending.push((targets.clone(), LayerEdit::Reference(on), false));
            }
            ui.end_row();

            ui.label(tagged("Color", &common.color));
            let mut rgb = common.color.value().copied().unwrap_or(layer.color);
            if ui.color_edit_button_srgb(&mut rgb).changed() {
                pending.push((targets.clone(), LayerEdit::Color(rgb), true));
            }
            ui.end_row();

            ui.label(tagged("Line Weight", &common.line_weight));
            let mut mm = common
                .line_weight
                .value()
                .copied()
                .unwrap_or(layer.line_weight) as f64
                / 100.0;
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
                pending.push((
                    targets.clone(),
                    LayerEdit::LineWeight((mm * 100.0).round().max(0.0) as u32),
                    true,
                ));
            }
            ui.end_row();

            ui.label(tagged("Line Style", &common.line_style));
            let mut style = common
                .line_style
                .value()
                .copied()
                .unwrap_or(layer.line_style);
            egui::ComboBox::from_id_salt("layer_line_style")
                .selected_text(if common.line_style.is_mixed() {
                    "No Change"
                } else {
                    line_style_label(style)
                })
                .show_ui(ui, |ui| {
                    for s in LINE_STYLES {
                        if ui
                            .selectable_value(&mut style, s, line_style_label(s))
                            .clicked()
                        {
                            pending.push((targets.clone(), LayerEdit::LineStyle(s), false));
                        }
                    }
                });
            ui.end_row();

            ui.label(tagged("Text Style", &common.text_style));
            let shown_style = if common.text_style.is_mixed() {
                "No Change".to_string()
            } else {
                layer.text_style.clone()
            };
            text_style_combo(ui, "layer_text_style", &shown_style, text_styles, |t| {
                pending.push((targets.clone(), LayerEdit::TextStyle(t), false));
            });
            ui.end_row();

            // Fill Style: a preview of the shared fill, "No Change" when the
            // selected layers differ, and the button that edits it.
            let fills: Vec<Option<FillStyle>> = targets
                .iter()
                .map(|n| {
                    cx.project
                        .styles
                        .fill_for(&FillTarget::Layer(n.clone()))
                        .cloned()
                })
                .collect();
            let mixed_fill = fills.windows(2).any(|w| w[0] != w[1]);
            ui.label(if mixed_fill {
                "Fill Style (No Change)"
            } else {
                "Fill Style"
            });
            ui.vertical(|ui| {
                match (&fills[0], mixed_fill) {
                    (_, true) => {
                        ui.weak("No Change");
                    }
                    (Some(style), false) => {
                        let patterns = cx.project.styles.all_patterns();
                        crate::dialogs::fill_style::preview_sized(
                            ui,
                            style,
                            &patterns,
                            layer.color,
                            48.0,
                            Vec2::new(120.0, 60.0),
                        );
                        ui.weak(style.summary());
                    }
                    (None, false) => {
                        ui.weak("No fill");
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Fill Style\u{2026}").clicked() {
                        fill_open = true;
                    }
                    if ui
                        .add_enabled(
                            fills.iter().any(Option::is_some),
                            egui::Button::new("Remove Fill"),
                        )
                        .clicked()
                    {
                        fill_clear = true;
                    }
                });
            });
            ui.end_row();
        });
    if fill_open {
        crate::dialogs::fill_style::open_for_layers(cx, &targets);
    }
    if fill_clear {
        clear_layer_fills(cx, &targets);
    }
    for (layers, edit, merged) in pending {
        edit_layers(cx, &layers, edit, st.all_sets, merged);
    }
}

/// Remove Fill: the layers go back to no fill style. One undo step.
pub fn clear_layer_fills(cx: &mut EditorContext, layers: &[String]) -> usize {
    let had: Vec<&String> = layers
        .iter()
        .filter(|n| {
            cx.project
                .styles
                .fill_for(&FillTarget::Layer((*n).clone()))
                .is_some()
        })
        .collect();
    if had.is_empty() {
        return 0;
    }
    let n = had.len();
    let doomed: Vec<String> = had.into_iter().cloned().collect();
    cx.begin_change("Remove Layer Fill");
    for name in doomed {
        cx.project.styles.apply_fill(FillTarget::Layer(name), None);
    }
    cx.mark_dirty();
    n
}

/// The modal window: the same table plus New Layer, Active Layers by Tool and
/// A property label that says "No Change" when the selected layers differ.
fn tagged<T>(label: &str, value: &Mixed<T>) -> String {
    if value.is_mixed() {
        format!("{label} (No Change)")
    } else {
        label.to_string()
    }
}

/// The shared properties of the selected layers as the table shows them
/// (the effective look in the shown layer set).
fn selection_common(cx: &EditorContext, targets: &[String], first: &Layer) -> LayerCommon {
    let layers: Vec<Layer> = targets
        .iter()
        .filter_map(|n| cx.layers().get(n).cloned())
        .collect();
    let refs: Vec<&Layer> = layers.iter().collect();
    common_props(&refs).unwrap_or_else(|| common_props(&[first]).expect("one layer"))
}

/// The Used cell: an icon for objects on the layer, for a defaults page that
/// names it, or for a system layer, with a tool tip saying where.
fn used_cell(ui: &mut egui::Ui, row: &Row) {
    let text = if row.info.objects > 0 {
        format!("\u{25CF} {}", row.info.objects)
    } else if !row.info.defaults.is_empty() {
        "\u{25D0}".to_string()
    } else if row.info.system {
        "\u{25CB}".to_string()
    } else {
        String::new()
    };
    let r = ui.label(text);
    let tip = row.info.tooltip();
    if !tip.is_empty() {
        r.on_hover_text(tip);
    }
}

// ----- layer management (LAY-67), one undo step each -----

/// New: a layer with a unique name, in every set (hidden in all but the
/// active one).
pub fn new_layer_named(cx: &mut EditorContext, name: &str) -> Result<String, String> {
    cx.begin_change("New Layer");
    match cx.project.new_layer(name) {
        Ok(n) => {
            cx.mark_dirty();
            cx.status = format!("Added layer {n}");
            Ok(n)
        }
        Err(e) => {
            cx.cancel_change();
            Err(e)
        }
    }
}

/// Copy: a copy of `name` below it.
pub fn copy_layer(cx: &mut EditorContext, name: &str) -> Result<String, String> {
    cx.begin_change("Copy Layer");
    match cx.project.copy_layer(name) {
        Ok(n) => {
            cx.mark_dirty();
            cx.status = format!("Copied {name} to {n}");
            Ok(n)
        }
        Err(e) => {
            cx.cancel_change();
            Err(e)
        }
    }
}

/// Merge: the first of `layers` (plan order) keeps; the others fold into
/// it with their objects and defaults. Returns how many objects moved.
pub fn merge_layers(cx: &mut EditorContext, layers: &[String]) -> Result<usize, String> {
    let Some((keep, rest)) = layers.split_first() else {
        return Err("Select two or more layers to merge".into());
    };
    cx.begin_change("Merge Layers");
    match cx.project.merge_layers(keep, rest) {
        Ok(n) => {
            cx.mark_dirty();
            cx.status = format!(
                "Merged {} layer{} into {keep} ({n} object{} moved)",
                rest.len(),
                if rest.len() == 1 { "" } else { "s" },
                if n == 1 { "" } else { "s" }
            );
            Ok(n)
        }
        Err(e) => {
            cx.cancel_change();
            Err(e)
        }
    }
}

/// Delete: every layer of `layers` or none (a system or used layer stops
/// the whole delete).
pub fn delete_layers(cx: &mut EditorContext, layers: &[String]) -> Result<usize, String> {
    if layers.is_empty() {
        return Err("Select a layer to delete".into());
    }
    for l in layers {
        if is_system_layer(l) {
            return Err(format!("{l} is a system layer and cannot be deleted"));
        }
        if cx.project.layer_use(l).in_use() {
            return Err(format!("{l} is in use and cannot be deleted"));
        }
    }
    cx.begin_change("Delete Layers");
    for l in layers {
        if let Err(e) = cx.project.delete_layer(l) {
            cx.cancel_change();
            return Err(e);
        }
    }
    cx.mark_dirty();
    cx.status = format!(
        "Deleted {} layer{}",
        layers.len(),
        if layers.len() == 1 { "" } else { "s" }
    );
    Ok(layers.len())
}

/// Delete Unused Layers. Returns how many went (no undo step for none).
pub fn delete_unused_layers(cx: &mut EditorContext) -> usize {
    cx.begin_change("Delete Unused Layers");
    let gone = cx.project.delete_unused_layers();
    if gone.is_empty() {
        cx.cancel_change();
        cx.status = "Every layer is in use".into();
        return 0;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Deleted {} unused layer{}",
        gone.len(),
        if gone.len() == 1 { "" } else { "s" }
    );
    gone.len()
}

/// Reset Layer Names: brings back missing system layers.
pub fn reset_layer_names(cx: &mut EditorContext) -> usize {
    cx.begin_change("Reset Layer Names");
    let n = cx.project.reset_layer_names();
    if n == 0 {
        cx.cancel_change();
        cx.status = "The system layers are already there".into();
        return 0;
    }
    cx.mark_dirty();
    cx.status = format!("Restored {n} system layer{}", if n == 1 { "" } else { "s" });
    n
}

/// Wall Layers: adds the wall system layers (Walls Layers, Main Layer Only,
/// Through Wall Lines, Footings, Brick Ledge Lines, No Locate, Attic) the
/// plan lacks, to every layer set. One undo step; returns how many.
pub fn add_wall_layers(cx: &mut EditorContext) -> usize {
    cx.begin_change("Add Wall Layers");
    let n = cx.project.ensure_wall_system_layers().len();
    if n == 0 {
        cx.cancel_change();
        cx.status = "The wall system layers are already there".into();
        return 0;
    }
    cx.mark_dirty();
    cx.status = format!(
        "Added {n} wall system layer{}",
        if n == 1 { "" } else { "s" }
    );
    n
}

/// The buttons under the table: New, Copy, Merge, Delete, Delete Unused
/// Layers and Reset Names.
fn management_buttons(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut LayerPanelState) {
    let sel = st.selected_layers(cx);
    let mut result: Option<Result<String, String>> = None;
    ui.horizontal_wrapped(|ui| {
        if ui.small_button("New").clicked() {
            st.naming = true;
            st.new_name = cx.project.free_layer_name_for_new();
        }
        if ui
            .add_enabled(sel.len() == 1, egui::Button::new("Copy").small())
            .on_hover_text("Copy the selected layer below itself")
            .clicked()
        {
            result = Some(copy_layer(cx, &sel[0]).map(|n| {
                st.select_only(&n);
                format!("Copied to {n}")
            }));
        }
        if ui
            .add_enabled(sel.len() > 1, egui::Button::new("Merge").small())
            .on_hover_text("Fold the other selected layers into the first one")
            .clicked()
        {
            result = Some(merge_layers(cx, &sel).map(|n| {
                st.select_only(&sel[0]);
                format!("Merged; {n} objects moved")
            }));
        }
        if ui
            .add_enabled(!sel.is_empty(), egui::Button::new("Delete").small())
            .on_hover_text("Delete the selected layers (not system or used layers)")
            .clicked()
        {
            result = Some(delete_layers(cx, &sel).map(|n| {
                st.select_none();
                format!("Deleted {n}")
            }));
        }
        if ui
            .small_button("Delete Unused Layers")
            .on_hover_text("Remove every layer nothing uses")
            .clicked()
        {
            let n = delete_unused_layers(cx);
            st.select_none();
            result = Some(Ok(format!("Deleted {n} unused")));
        }
        if ui
            .small_button("Wall Layers")
            .on_hover_text("Add the wall display layers: Layers, Main Layer Only, Through Wall Lines, Footings, Brick Ledge Lines, No Locate, Attic")
            .clicked()
        {
            let n = add_wall_layers(cx);
            result = Some(Ok(format!("Added {n} wall layers")));
        }
        if ui
            .small_button("Reset Names")
            .on_hover_text("Bring back any missing system layer")
            .clicked()
        {
            let n = reset_layer_names(cx);
            result = Some(Ok(format!("Restored {n}")));
        }
    });
    if st.naming {
        ui.horizontal(|ui| {
            ui.label("New Layer Name");
            let edit = ui.text_edit_singleline(&mut st.new_name);
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if ui.small_button("OK").clicked() || enter {
                let name = st.new_name.clone();
                result = Some(new_layer_named(cx, &name).map(|n| {
                    st.naming = false;
                    st.filter.clear();
                    st.select_only(&n);
                    format!("Added {n}")
                }));
            }
            if ui.small_button("Cancel").clicked() {
                st.naming = false;
            }
        });
    }
    match result {
        Some(Ok(m)) => st.message = m,
        Some(Err(e)) => {
            st.message = e.clone();
            cx.status = e;
        }
        None => {}
    }
}

/// Layer Set Management.
#[derive(Default)]
pub struct LayerDisplayDialog {
    panel: LayerPanelState,
}

impl LayerDisplayDialog {
    /// New Layer...: adds a layer and selects it.
    pub fn new_layer(&mut self, cx: &mut EditorContext) -> String {
        let name = add_layer(cx);
        self.panel.filter.clear();
        self.panel.select_only(&name);
        name
    }

    /// The table state (for tests).
    #[cfg(test)]
    pub fn panel(&self) -> &LayerPanelState {
        &self.panel
    }

    /// Draws the window; returns false once it should close.
    pub fn show(&mut self, ctx: &egui::Context, cx: &mut EditorContext) -> bool {
        self.show_with_id(ctx, cx, "layer_display_options")
    }

    /// [`show`](Self::show) with its own window id, so the window the Define
    /// buttons open can sit beside the one of the Tools menu.
    fn show_with_id(&mut self, ctx: &egui::Context, cx: &mut EditorContext, id: &str) -> bool {
        let mut open = true;
        let mut close = false;
        egui::Window::new("Layer Display Options")
            .id(egui::Id::new(id))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(620.0, 600.0))
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                layer_panel(ui, cx, &mut self.panel, 0.5);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("New Layer\u{2026}").clicked() {
                        let name = self.new_layer(cx);
                        cx.status = format!("Added layer {name}");
                    }
                    if ui.button("Layer Set Management\u{2026}").clicked() {
                        sets::open();
                    }
                    if ui.button("Active Layers by Tool\u{2026}").clicked() {
                        sets::open_tools();
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Close").clicked() {
                            close = true;
                        }
                    });
                });
            });
        if !ctx.wants_keyboard_input()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape))
        {
            close = true;
        }
        open && !close
    }
}

thread_local! {
    static DEFINE: std::cell::RefCell<Option<LayerDisplayDialog>> =
        const { std::cell::RefCell::new(None) };
}

/// The Define button of a Layer panel (the painter bar, Select Layer, the
/// Layer Set Defaults): opens Layer Display Options to pick, change or add a
/// layer.
pub fn open_define() {
    DEFINE.with(|d| {
        let mut d = d.borrow_mut();
        if d.is_none() {
            *d = Some(LayerDisplayDialog::default());
        }
    });
}

/// Is the Define window open?
pub fn define_open() -> bool {
    DEFINE.with(|d| d.borrow().is_some())
}

/// Draws the Define window while it is open.
pub fn show_define(ctx: &egui::Context, cx: &mut EditorContext) {
    let Some(mut d) = DEFINE.with(|d| d.borrow_mut().take()) else {
        return;
    };
    if d.show_with_id(ctx, cx, "layer_display_define") {
        DEFINE.with(|slot| *slot.borrow_mut() = Some(d));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::docks::set_layer_display;

    fn cx() -> EditorContext {
        crate::editor::plan_tabs::plain_cx()
    }

    #[test]
    fn new_layer_is_selected_and_listed() {
        let mut cx = cx();
        let mut d = LayerDisplayDialog::default();
        assert!(d.panel().selected.is_none());
        let name = d.new_layer(&mut cx);
        assert_eq!(d.panel().selected.as_deref(), Some(name.as_str()));
        assert!(cx.project.layers.get(&name).is_some());
    }

    #[test]
    fn row_clicks_select_toggle_and_extend() {
        let cx = cx();
        let mut st = LayerPanelState::default();
        let rows = visible_names(&cx, &st);
        let none = Modifiers::NONE;
        st.click_row(&rows[2], &rows, none);
        assert_eq!(st.selected.as_deref(), Some(rows[2].as_str()));
        assert_eq!(st.multi.len(), 1);
        // Shift extends from the anchor.
        st.click_row(&rows[5], &rows, Modifiers::SHIFT);
        assert_eq!(st.multi.len(), 4);
        assert!(st.multi.contains(&rows[3]) && st.multi.contains(&rows[5]));
        // Cmd toggles one row off and one on.
        st.click_row(&rows[3], &rows, Modifiers::COMMAND);
        assert!(!st.multi.contains(&rows[3]));
        st.click_row(&rows[9], &rows, Modifiers::COMMAND);
        assert!(st.multi.contains(&rows[9]));
        assert_eq!(st.selected_layers(&cx).len(), 4);
        // An edit of a selected row reaches the whole selection; of any other
        // row, itself alone.
        assert_eq!(st.targets_of(&cx, &rows[2]).len(), 4);
        assert_eq!(st.targets_of(&cx, &rows[0]), vec![rows[0].clone()]);
        st.select_all(&rows);
        assert_eq!(st.multi.len(), rows.len());
        st.select_none();
        assert!(st.multi.is_empty() && st.selected.is_none());
        // A plain click selects one again.
        st.click_row(&rows[1], &rows, none);
        assert_eq!(st.multi.len(), 1);
    }

    #[test]
    fn sorting_and_filtering_the_rows() {
        let mut cx = cx();
        set_layer_display(&mut cx, "Doors", false);
        let mut st = LayerPanelState::default();
        let plan = visible_names(&cx, &st);
        assert_eq!(plan[0], "Walls, Normal", "plan order");
        st.sort = SortKey::Name;
        let by_name = visible_names(&cx, &st);
        assert!(by_name
            .windows(2)
            .all(|w| w[0].to_lowercase() <= w[1].to_lowercase()));
        st.descending = true;
        assert_eq!(visible_names(&cx, &st)[0], *by_name.last().unwrap());
        st.sort = SortKey::Disp;
        st.descending = false;
        assert_eq!(
            visible_names(&cx, &st)[0],
            "Doors",
            "the hidden layer sorts first"
        );
        st.filter = "walls".into();
        assert!(visible_names(&cx, &st)
            .iter()
            .all(|n| n.to_lowercase().contains("walls")));
    }

    #[test]
    fn multi_select_edits_every_selected_layer_in_one_step() {
        let mut cx = cx();
        let mut st = LayerPanelState::default();
        let rows = visible_names(&cx, &st);
        st.click_row("Doors", &rows, Modifiers::NONE);
        st.click_row("Windows", &rows, Modifiers::COMMAND);
        let t = st.targets_of(&cx, "Doors");
        assert_eq!(t.len(), 2);
        assert_eq!(
            edit_layers(&mut cx, &t, LayerEdit::Color([9, 9, 9]), false, false),
            2
        );
        assert_eq!(cx.layers().get("Doors").unwrap().color, [9, 9, 9]);
        assert_eq!(cx.layers().get("Windows").unwrap().color, [9, 9, 9]);
        assert_eq!(cx.undo_label(), Some("Layer Color"));
        cx.undo();
        assert_ne!(cx.layers().get("Doors").unwrap().color, [9, 9, 9]);
        assert_ne!(cx.layers().get("Windows").unwrap().color, [9, 9, 9]);
    }

    #[test]
    fn reset_and_copy_to_sets_are_undoable() {
        let mut cx = cx();
        sets::copy_set(&mut cx, "Default Set", "B").unwrap();
        let l = vec!["Rooms".to_string()];
        edit_layers(&mut cx, &l, LayerEdit::Color([1, 2, 3]), false, false);
        assert_eq!(copy_layers_to_sets(&mut cx, &l, &["B".to_string()]), 1);
        assert_eq!(
            cx.project
                .layer_sets
                .effective_for("B", &cx.project.layers)
                .get("Rooms")
                .unwrap()
                .color,
            [1, 2, 3]
        );
        cx.undo();
        assert_ne!(
            cx.project
                .layer_sets
                .effective_for("B", &cx.project.layers)
                .get("Rooms")
                .unwrap()
                .color,
            [1, 2, 3]
        );
        assert_eq!(copy_layers_to_sets(&mut cx, &[], &["B".to_string()]), 0);
        assert_eq!(reset_layers(&mut cx, &l, false), 1);
        assert_ne!(cx.layers().get("Rooms").unwrap().color, [1, 2, 3]);
        assert_eq!(reset_layers(&mut cx, &["Nope".to_string()], false), 0);
        cx.undo();
        assert_eq!(cx.layers().get("Rooms").unwrap().color, [1, 2, 3]);
    }

    #[test]
    fn select_all_on_layer_skips_hidden_and_locked_layers() {
        use plan_core::geometry::Point;
        use plan_core::WallKind;
        let mut cx = cx();
        for i in 0..3 {
            cx.project.add_wall(
                0,
                Point::new(0.0, i as f64 * 100.0),
                Point::new(120.0, i as f64 * 100.0),
                6.0,
                96.0,
                WallKind::Exterior,
            );
        }
        let cad = cx.project.add_cad(
            0,
            "CAD, Default",
            plan_core::cad::CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
        );
        let walls = vec!["Walls, Normal".to_string()];
        assert_eq!(select_all_on_layers(&mut cx, &walls), 3);
        assert_eq!(
            select_all_on_layers(&mut cx, &["CAD, Default".to_string()]),
            1
        );
        assert!(cx.selection.contains(crate::editor::ObjectRef::Cad(cad)));
        // Locked and hidden layers hold nothing selectable.
        edit_layers(&mut cx, &walls, LayerEdit::Locked(true), false, false);
        assert_eq!(select_all_on_layers(&mut cx, &walls), 0);
        edit_layers(&mut cx, &walls, LayerEdit::Locked(false), false, false);
        edit_layers(&mut cx, &walls, LayerEdit::Display(false), false, false);
        assert_eq!(select_all_on_layers(&mut cx, &walls), 0);
    }

    #[test]
    fn the_dialog_and_table_draw_with_every_control() {
        let ctx = egui::Context::default();
        let mut cx = cx();
        sets::copy_set(&mut cx, "Default Set", "B").unwrap();
        let mut d = LayerDisplayDialog::default();
        d.panel.select_only("Doors");
        d.panel.multi.insert("Windows".into());
        d.panel.set_mode = SetMode::Copy;
        d.panel.all_sets = true;
        for sort in [SortKey::Plan, SortKey::Name, SortKey::Used, SortKey::Weight] {
            d.panel.sort = sort;
            for _ in 0..2 {
                let _ = ctx.run(egui::RawInput::default(), |ctx| {
                    d.show(ctx, &mut cx);
                });
            }
        }
    }
}
