//! The Materials List view of the window: its toolbar, the grid of the 21
//! columns and the footer.

use super::{cmd, CellEdit, State};
use crate::editor::EditorContext;
use crate::shell::layout_window as lw;
use eframe::egui::{self, Align, Color32, Layout, RichText, Sense, UiBuilder, Vec2};
use plan_core::materials_data::{ListKind, ListScope, MlColumn};
use plan_docs::materials::list::{self, ListLine, Row};

const ROW_H: f32 = 22.0;
const NUM_W: f32 = 44.0;

/// One displayed row of the grid.
#[derive(Clone)]
enum Disp {
    Group(String),
    Line(usize),
    /// An object of an expanded line.
    Child(Box<ListLine>),
    Subtotal(String, Option<f64>),
    Total(Option<f64>),
}

pub fn list_view(ui: &mut egui::Ui, ctx: &egui::Context, cx: &mut EditorContext, st: &mut State) {
    toolbar(ui, cx, st);
    let calc = super::compute(cx, st);
    let lines = calc.lines;
    ui.horizontal(|ui| {
        ui.strong(&st.spec.name);
        ui.label(match st.spec.kind {
            ListKind::Live => format!("Live List \u{2013} {}", scope_text(cx, st)),
            ListKind::Report => "Report (not linked to the plan)".to_string(),
        });
        ui.weak(format!("{} lines", lines.len()));
        if let Some(n) = &calc.note {
            ui.colored_label(Color32::from_rgb(0xD0, 0x6A, 0x1C), n);
        }
    });
    ui.separator();
    draw_grid(ui, ctx, cx, st, &lines);
    ui.separator();
    footer(ui, cx, st, &lines);
    st.shown = lines;
}

fn scope_text(cx: &EditorContext, st: &State) -> String {
    match &st.spec.scope {
        ListScope::AllFloors => "all floors".into(),
        ListScope::Floor(f) => cx
            .project
            .floors
            .get(*f)
            .map_or("a floor".into(), |f| f.name.clone()),
        ListScope::Polyline(_) => "from a Materials List Polyline".into(),
        ListScope::Room { .. } => "in a room".into(),
        ListScope::Selection(a) => format!("{} selected object(s)", a.len()),
    }
}

fn toolbar(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut State) {
    let live = st.spec.kind == ListKind::Live;
    ui.horizontal_wrapped(|ui| {
        ui.label("Calculate:");
        if ui.button("All Floors").clicked() {
            super::calculate_all(st);
        }
        if ui.button("This Floor").clicked() {
            let name = cx.floor().name.clone();
            super::calculate_floor(st, cx.floor, &name);
        }
        if ui
            .button("From Selection")
            .on_hover_text("Calculate Materials From Selection")
            .clicked()
            && !super::calculate_selection(cx, st)
        {
            cx.status = st.status.clone();
        }
        if ui
            .button("In Room")
            .on_hover_text("Calculate Materials in Room")
            .clicked()
            && !super::calculate_room(cx, st)
        {
            cx.status = st.status.clone();
        }
        ui.menu_button("From Area", |ui| {
            let polys: Vec<(plan_core::Id, String)> = cx
                .project
                .materials
                .polylines
                .iter()
                .map(|p| (p.cad_id, p.name.clone()))
                .collect();
            if polys.is_empty() {
                ui.weak("Draw a Materials List Polyline first");
            }
            for (id, name) in polys {
                if ui.button(&name).clicked() {
                    super::calculate_polyline(cx, st, id);
                    ui.close_menu();
                }
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .button("Save")
            .on_hover_text("Save Active View")
            .clicked()
        {
            super::save_list(cx, st, None);
        }
        if ui.button("Save As\u{2026}").clicked() {
            st.extras.ask_save_as(&st.spec.name);
        }
        if ui
            .add_enabled(live, egui::Button::new("Generate a Report"))
            .on_hover_text("Freeze the list as a static Report")
            .clicked()
        {
            super::generate_report(cx, st);
        }
        if ui
            .button("Edit Active View\u{2026}")
            .on_hover_text("The Materials List Specification")
            .clicked()
        {
            st.spec_dialog = Some(super::spec::SpecDialog::new(&st.spec, cx));
        }
        if ui.button("Management\u{2026}").clicked() {
            st.extras.open_management();
        }
    });
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!live, egui::Button::new("Update From Master List"))
            .clicked()
        {
            super::update_from_master(cx, st);
        }
        if ui
            .button("Update To Master List")
            .on_hover_text("Saves the selected rows (all rows when none is selected)")
            .clicked()
        {
            super::update_to_master(cx, st);
        }
        ui.separator();
        if ui
            .add_enabled(live, egui::Button::new("Expand"))
            .on_hover_text("Show the objects behind the selected lines")
            .clicked()
        {
            let sel: Vec<usize> = st.selected.iter().copied().collect();
            st.expanded.extend(sel);
        }
        if ui
            .add_enabled(live, egui::Button::new("Collapse"))
            .clicked()
        {
            st.expanded.clear();
        }
        if ui
            .add_enabled(live, egui::Button::new("Find Object in Plan"))
            .clicked()
        {
            find_selected(st);
        }
        if ui.button("Details\u{2026}").clicked() {
            open_details(st);
        }
    });
}

/// Asks to find the first object of the first selected line.
fn find_selected(st: &mut State) {
    let Some(&i) = st.selected.iter().next() else {
        st.status = "Select a line first".into();
        return;
    };
    let Some(l) = st.shown.get(i) else { return };
    match list::find_targets(l).into_iter().next() {
        Some(t) => st.find = Some(t),
        None => st.status = "That line has no object in the plan".into(),
    }
}

fn open_details(st: &mut State) {
    let rows: Vec<usize> = if st.selected.is_empty() {
        Vec::new()
    } else {
        st.selected.iter().copied().collect()
    };
    if rows.is_empty() {
        st.status = "Select a line first".into();
        return;
    }
    let lines: Vec<ListLine> = rows
        .iter()
        .filter_map(|i| st.shown.get(*i).cloned())
        .collect();
    st.extras.open_details(lines);
}

fn footer(ui: &mut egui::Ui, cx: &mut EditorContext, st: &mut State, lines: &[ListLine]) {
    let total = list::total_of(lines);
    ui.horizontal(|ui| {
        match total {
            Some(t) => ui.strong(format!("Total {}", plan_docs::fmt_money(Some(t)))),
            None => ui.weak("No prices yet: type them in the Price column or in the Master List"),
        };
    });
    ui.horizontal(|ui| {
        if ui.button("Export\u{2026}").clicked() {
            st.extras.open_export();
        }
        ui.menu_button("Print\u{2026}", |ui| {
            use crate::dialogs::print::Destination as D;
            for (label, dest) in [
                ("Print to the Printer", D::Printer),
                ("Preview in the Viewer", D::Viewer),
                ("Save as a PDF File\u{2026}", D::Pdf),
            ] {
                if ui.button(label).clicked() {
                    cx.status = super::print_list(cx, st, dest);
                    ui.close_menu();
                }
            }
        });
        if ui
            .button("Send to Layout")
            .on_hover_text("Add this list to the layout as a table box")
            .clicked()
        {
            let floor = match &st.spec.scope {
                ListScope::Floor(f) => Some(*f),
                _ => None,
            };
            st.status = lw::send_materials(cx, floor, None);
        }
        let _ = cmd::OPEN;
    });
}

// ------------------------------------------------------------------- grid --

fn build_display(cx: &EditorContext, st: &State, lines: &[ListLine]) -> Vec<Disp> {
    let mut out = Vec::new();
    for r in list::arrange(lines, &st.spec, false) {
        match r {
            Row::Group(g) => out.push(Disp::Group(g)),
            Row::Line(i) => {
                out.push(Disp::Line(i));
                if st.expanded.contains(&i) && st.spec.kind == ListKind::Live {
                    for k in list::expand(&cx.project, lines, i) {
                        out.push(Disp::Child(Box::new(k)));
                    }
                }
            }
            Row::Subtotal(g, t) => out.push(Disp::Subtotal(g, t)),
            Row::GrandTotal(t) => out.push(Disp::Total(t)),
        }
    }
    out
}

fn draw_grid(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    cx: &mut EditorContext,
    st: &mut State,
    lines: &[ListLine],
) {
    let cols: Vec<_> = st
        .spec
        .visible_columns()
        .into_iter()
        .filter(|c| c.col.in_materials_list())
        .collect();
    if cols.is_empty() {
        ui.weak("No column is shown: choose some in Edit Active View > Columns.");
        return;
    }
    let disp = build_display(cx, st, lines);
    let app = st.spec.appearance.clone();
    let size = app.font_size.clamp(7.0, 24.0);
    let text_color = if app.custom_colors {
        Color32::from_rgb(app.text[0], app.text[1], app.text[2])
    } else {
        ui.visuals().text_color()
    };
    let grid_color = if app.custom_colors {
        Color32::from_rgb(app.grid[0], app.grid[1], app.grid[2])
    } else {
        ui.visuals().widgets.noninteractive.bg_stroke.color
    };
    let bg = app
        .custom_colors
        .then(|| Color32::from_rgb(app.background[0], app.background[1], app.background[2]));
    let total_w: f32 = NUM_W + cols.iter().map(|c| c.width).sum::<f32>();
    let mut resize: Option<(MlColumn, f32)> = None;
    let mut commit: Option<CellEdit> = None;
    let mut cancel = false;
    let mut start_edit: Option<CellEdit> = None;
    let mut select: Option<(usize, bool, bool)> = None;
    let mut find: Option<usize> = None;
    let mut toggle_expand: Option<usize> = None;
    let mut menu_action: Option<(usize, &'static str)> = None;
    let live = st.spec.kind == ListKind::Live;

    egui::ScrollArea::horizontal()
        .id_salt("ml_h")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_min_width(total_w);
            // Header.
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                cell_box(ui, NUM_W, ROW_H, |ui| {
                    ui.weak("#");
                });
                for c in &cols {
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(c.width, ROW_H), Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
                    ui.painter().text(
                        rect.left_center() + Vec2::new(4.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        c.col.title(),
                        egui::FontId::proportional(size),
                        text_color,
                    );
                    // The right edge drags the column's width.
                    let handle = egui::Rect::from_min_size(
                        egui::pos2(rect.right() - 4.0, rect.top()),
                        Vec2::new(8.0, ROW_H),
                    );
                    let h = ui.interact(handle, ui.id().with(("ml_w", c.col as u8)), Sense::drag());
                    if h.hovered() || h.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                    }
                    if h.dragged() {
                        resize = Some((c.col, c.width + h.drag_delta().x));
                    }
                    let _ = resp;
                }
            });
            // Body: only the visible rows are laid out.
            egui::ScrollArea::vertical()
                .id_salt("ml_v")
                .auto_shrink([false, false])
                .max_height(ui.available_height() - 4.0)
                .show_rows(ui, ROW_H, disp.len(), |ui, range| {
                    for di in range {
                        let d = &disp[di];
                        let is_line = matches!(d, Disp::Line(_));
                        let line_idx = match d {
                            Disp::Line(i) => Some(*i),
                            _ => None,
                        };
                        let selected = line_idx.is_some_and(|i| st.selected.contains(&i));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;
                            // Row number: click selects, double-click finds.
                            let (rect, resp) =
                                ui.allocate_exact_size(Vec2::new(NUM_W, ROW_H), Sense::click());
                            let fill = if selected {
                                ui.visuals().selection.bg_fill
                            } else {
                                ui.visuals().faint_bg_color
                            };
                            ui.painter().rect_filled(rect, 0.0, fill);
                            if let Some(i) = line_idx {
                                let arrow = if live && lines[i].sources.len() > 1 {
                                    if st.expanded.contains(&i) {
                                        "\u{25BE} "
                                    } else {
                                        "\u{25B8} "
                                    }
                                } else {
                                    ""
                                };
                                ui.painter().text(
                                    rect.left_center() + Vec2::new(3.0, 0.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{arrow}{}", i + 1),
                                    egui::FontId::proportional(size),
                                    text_color,
                                );
                                if resp.clicked() {
                                    let m = ui.input(|i| i.modifiers);
                                    if live
                                        && lines[i].sources.len() > 1
                                        && resp
                                            .interact_pointer_pos()
                                            .is_some_and(|p| p.x < rect.left() + 14.0)
                                    {
                                        toggle_expand = Some(i);
                                    } else {
                                        select = Some((i, m.command || m.ctrl, m.shift));
                                    }
                                }
                                if resp.double_clicked() {
                                    find = Some(i);
                                }
                                resp.context_menu(|ui| {
                                    if ui.button("Details\u{2026}").clicked() {
                                        menu_action = Some((i, "details"));
                                        ui.close_menu();
                                    }
                                    if live && ui.button("Find Object in Plan").clicked() {
                                        menu_action = Some((i, "find"));
                                        ui.close_menu();
                                    }
                                    if live
                                        && lines[i].sources.len() > 1
                                        && ui.button("Expand / Collapse").clicked()
                                    {
                                        menu_action = Some((i, "expand"));
                                        ui.close_menu();
                                    }
                                    ui.menu_button("Move to Category", |ui| {
                                        for c in plan_core::materials_data::CATEGORIES {
                                            if ui.button(c).clicked() {
                                                start_edit = Some(CellEdit {
                                                    line: i,
                                                    col: MlColumn::Id,
                                                    text: c.to_string(),
                                                });
                                                commit = start_edit.take();
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                    if ui.button("Update To Master List").clicked() {
                                        menu_action = Some((i, "to_master"));
                                        ui.close_menu();
                                    }
                                });
                            }
                            for c in &cols {
                                let (rect, resp) = ui
                                    .allocate_exact_size(Vec2::new(c.width, ROW_H), Sense::click());
                                let stripe = di % 2 == 1 && !app.custom_colors;
                                let fill = if selected {
                                    ui.visuals().selection.bg_fill.gamma_multiply(0.6)
                                } else if let Some(b) = bg {
                                    b
                                } else if stripe {
                                    ui.visuals().faint_bg_color
                                } else {
                                    Color32::TRANSPARENT
                                };
                                ui.painter().rect_filled(rect, 0.0, fill);
                                if app.horizontal_lines {
                                    ui.painter().hline(
                                        rect.x_range(),
                                        rect.bottom(),
                                        egui::Stroke::new(0.5_f32, grid_color),
                                    );
                                }
                                if app.vertical_lines {
                                    ui.painter().vline(
                                        rect.right(),
                                        rect.y_range(),
                                        egui::Stroke::new(0.5_f32, grid_color),
                                    );
                                }
                                let editing = st
                                    .edit
                                    .as_ref()
                                    .is_some_and(|e| Some(e.line) == line_idx && e.col == c.col);
                                if editing {
                                    let e = st.edit.as_mut().expect("editing");
                                    let r = ui.put(
                                        rect,
                                        egui::TextEdit::singleline(&mut e.text)
                                            .font(egui::FontId::proportional(size)),
                                    );
                                    r.request_focus();
                                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                                    let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                                    if enter || (r.lost_focus() && !esc) {
                                        commit = Some(e.clone());
                                    } else if esc {
                                        cancel = true;
                                    }
                                    continue;
                                }
                                let (text, bold, depth) = match d {
                                    Disp::Line(i) => (list::cell(&lines[*i], c.col), false, 0),
                                    Disp::Child(k) => (list::cell(k, c.col), false, 1),
                                    Disp::Group(g) => (
                                        if c.col == cols[0].col {
                                            g.clone()
                                        } else {
                                            String::new()
                                        },
                                        true,
                                        0,
                                    ),
                                    Disp::Subtotal(g, t) => (
                                        if c.col == cols[0].col {
                                            format!("Subtotal {g}")
                                        } else if c.col == MlColumn::TotalCost {
                                            plan_docs::fmt_money(*t)
                                        } else {
                                            String::new()
                                        },
                                        true,
                                        0,
                                    ),
                                    Disp::Total(t) => (
                                        if c.col == cols[0].col {
                                            "Total".to_string()
                                        } else if c.col == MlColumn::TotalCost {
                                            plan_docs::fmt_money(*t)
                                        } else {
                                            String::new()
                                        },
                                        true,
                                        0,
                                    ),
                                };
                                let mut rich =
                                    RichText::new(text.clone()).size(size).color(text_color);
                                if bold || app.bold {
                                    rich = rich.strong();
                                }
                                if app.italic {
                                    rich = rich.italics();
                                }
                                if app.underline {
                                    rich = rich.underline();
                                }
                                if app.strikeout {
                                    rich = rich.strikethrough();
                                }
                                let layout = if c.col.numeric() {
                                    Layout::right_to_left(Align::Center)
                                } else {
                                    Layout::left_to_right(Align::Center)
                                };
                                let inner = rect.shrink2(Vec2::new(4.0 + 10.0 * depth as f32, 0.0));
                                let mut child =
                                    ui.new_child(UiBuilder::new().max_rect(inner).layout(layout));
                                child.add(egui::Label::new(rich).truncate().selectable(false));
                                if let Some(i) = line_idx {
                                    if c.col == MlColumn::Count {
                                        resp.clone().on_hover_text(super::count_tip(&lines[i]));
                                    }
                                    if resp.double_clicked() {
                                        if c.col == MlColumn::Id || c.col == MlColumn::Count {
                                            find = Some(i);
                                        } else if cell_editable(st.spec.kind, c.col) {
                                            start_edit = Some(CellEdit {
                                                line: i,
                                                col: c.col,
                                                text: edit_text(&lines[i], c.col),
                                            });
                                        } else {
                                            find = Some(i);
                                        }
                                    } else if resp.clicked() {
                                        let m = ui.input(|i| i.modifiers);
                                        select = Some((i, m.command || m.ctrl, m.shift));
                                    }
                                }
                                let _ = (is_line, ctx);
                            }
                        });
                    }
                });
        });

    if let Some((col, w)) = resize {
        st.spec.set_width(col, w);
        st.dirty = true;
    }
    if let Some((i, additive, range)) = select {
        if additive {
            if !st.selected.remove(&i) {
                st.selected.insert(i);
            }
        } else if range && !st.selected.is_empty() {
            let lo = *st.selected.iter().next().expect("not empty");
            let (a, b) = (lo.min(i), lo.max(i));
            st.selected = (a..=b).collect();
        } else {
            st.selected.clear();
            st.selected.insert(i);
        }
    }
    if let Some(i) = toggle_expand {
        if !st.expanded.remove(&i) {
            st.expanded.insert(i);
        }
    }
    if let Some(e) = start_edit {
        st.edit = Some(e);
    }
    if cancel {
        st.edit = None;
    }
    if let Some(e) = commit {
        st.edit = None;
        apply_edit(cx, st, lines, &e);
    }
    if let Some(i) = find {
        st.selected.clear();
        st.selected.insert(i);
        find_selected(st);
    }
    if let Some((i, what)) = menu_action {
        st.selected.clear();
        st.selected.insert(i);
        match what {
            "details" => open_details(st),
            "find" => find_selected(st),
            "expand" => {
                if !st.expanded.remove(&i) {
                    st.expanded.insert(i);
                }
            }
            "to_master" => super::update_to_master(cx, st),
            _ => {}
        }
    }
}

fn cell_editable(kind: ListKind, col: MlColumn) -> bool {
    match kind {
        // Count is set in the Components panel of the object.
        ListKind::Live => col.editable() && col != MlColumn::Count,
        ListKind::Report => col.editable(),
    }
}

/// What a cell holds when typing starts: the number without its decoration.
fn edit_text(l: &ListLine, col: MlColumn) -> String {
    match col {
        MlColumn::Price => l
            .line
            .unit_price
            .map_or(String::new(), |p| format!("{p:.2}")),
        MlColumn::Labor => nz(l.labor),
        MlColumn::Equipment => nz(l.equipment),
        MlColumn::Markup => nz(l.markup),
        MlColumn::Extra => nz(l.extra),
        MlColumn::Count => format!("{}", l.line.quantity),
        _ => list::cell(l, col),
    }
}

fn nz(v: f64) -> String {
    if v == 0.0 {
        String::new()
    } else {
        format!("{v}")
    }
}

/// Commits a typed cell: a live list writes the objects behind the row (one
/// undo step), a Report writes its own row.
pub fn apply_edit(cx: &mut EditorContext, st: &mut State, lines: &[ListLine], e: &CellEdit) {
    let Some(line) = lines.get(e.line) else {
        return;
    };
    match st.spec.kind {
        ListKind::Live => {
            cx.begin_change("Materials List Edit");
            if list::edit_cell(&mut cx.project, line, e.col, &e.text) {
                cx.mark_dirty();
                st.dirty = true;
            } else {
                cx.cancel_change();
                st.status = format!("That is not a valid {}", e.col.title());
            }
        }
        ListKind::Report => {
            let Some(row) = st.rows.get_mut(e.line) else {
                return;
            };
            if list::edit_report_cell(row, e.col, &e.text) {
                st.dirty = true;
            } else {
                st.status = format!("That is not a valid {}", e.col.title());
            }
        }
    }
}

fn cell_box(ui: &mut egui::Ui, w: f32, h: f32, add: impl FnOnce(&mut egui::Ui)) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let mut child = ui.new_child(UiBuilder::new().max_rect(rect.shrink2(Vec2::new(4.0, 0.0))));
    add(&mut child);
}
