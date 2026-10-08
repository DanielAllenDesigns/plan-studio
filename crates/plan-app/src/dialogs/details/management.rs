//! The CAD detail windows: CAD Detail Management (list, rename, duplicate,
//! delete, open in a tab, send to layout), the Auto Detail chooser and the
//! Detail Components chooser.
//!
//! The state is per session (which window is open, the selection, the typed
//! name); the details themselves are floors of the plan marked as details
//! (`plan_core::details::CadDetailInfo`) and every change goes through the
//! undoable operations of `tools::details`.

use crate::editor::{EditorContext, EditorRequest};
use crate::tools::details::{self as td, AutoDetailOptions, ComponentCategory};
use crate::tools::{details::DetailsVariant, ToolId};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use plan_core::cad::CadItem;
use plan_core::details::DetailSource;
use plan_core::geometry::Point;
use plan_core::Id;
use std::cell::RefCell;

#[derive(Default)]
struct State {
    management: bool,
    auto: bool,
    components: bool,
    /// The selected detail in the management list (by name).
    selected: Option<String>,
    /// The name typed for the selected detail.
    rename: String,
    rename_for: Option<String>,
    /// Auto Detail: the camera chosen and what to draw.
    camera: Option<Id>,
    options: AutoDetailOptions,
    category: usize,
    component: Option<&'static str>,
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State {
        options: AutoDetailOptions::default(),
        ..State::default()
    });
}

fn state<R>(f: impl FnOnce(&mut State) -> R) -> R {
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Opens CAD Detail Management.
pub fn open_management() {
    state(|s| s.management = true);
}

/// Is CAD Detail Management open?
pub fn management_open() -> bool {
    state(|s| s.management)
}

/// Opens the Auto Detail chooser on the plan's sections and elevations; says
/// so in the status bar (and stays closed) when there are none.
pub fn open_auto_detail(cx: &mut EditorContext) {
    let cams = td::eligible_cameras(&cx.project);
    if cams.is_empty() {
        cx.status =
            "Auto Detail needs a cross section or elevation: draw one with the camera tools first"
                .into();
        return;
    }
    state(|s| {
        s.auto = true;
        if s.camera.is_none_or(|c| !cams.iter().any(|x| x.0 == c)) {
            s.camera = Some(cams[0].0);
        }
    });
}

/// Is the Auto Detail chooser open?
fn auto_detail_open() -> bool {
    state(|s| s.auto)
}

/// Opens the Detail Components chooser.
pub fn open_components() {
    state(|s| s.components = true);
}

/// Is the Detail Components chooser open?
pub fn components_open() -> bool {
    state(|s| s.components)
}

/// Selects a detail in the management list.
pub fn select_detail(name: Option<&str>) {
    state(|s| s.selected = name.map(str::to_string));
}

/// What a click in a window asked for (run after the window is drawn, so the
/// editor is not borrowed while it draws).
enum Act {
    Open(usize),
    Rename(usize, String),
    Duplicate(usize),
    Delete(usize),
    ToLayout(usize),
    New,
    MakeDetail,
    PlaceByClick(&'static str),
    InsertNow(&'static str),
}

fn source_text(cx: &EditorContext, i: usize) -> String {
    let Some(d) = cx.project.floors[i].detail.as_ref() else {
        return String::new();
    };
    match &d.source {
        DetailSource::Blank => "Drawn".into(),
        DetailSource::Camera { camera } => match cx.project.camera(*camera) {
            Some(c) => format!("From {}", c.name),
            None => "From a view that is gone".into(),
        },
        DetailSource::PlanView { floor } => format!("From plan {floor}"),
    }
}

/// Draws the open windows. Called every frame by the shell.
pub fn show_windows(ctx: &egui::Context, cx: &mut EditorContext) {
    td::sync_names(cx);
    let mut acts: Vec<Act> = Vec::new();
    if management_open() {
        management(ctx, cx, &mut acts);
    }
    if auto_detail_open() {
        auto_window(ctx, cx, &mut acts);
    }
    if components_open() {
        components_window(ctx, cx, &mut acts);
    }
    for a in acts {
        run(cx, a);
    }
}

fn run(cx: &mut EditorContext, act: Act) {
    match act {
        Act::Open(i) => {
            td::open_detail(cx, i);
        }
        Act::Rename(i, name) => {
            if td::rename_detail(cx, i, &name) {
                let new = cx.project.floors[i].name.clone();
                state(|s| {
                    s.selected = Some(new.clone());
                    s.rename_for = Some(new);
                });
            } else {
                cx.status = "Type a name for the detail".into();
            }
        }
        Act::Duplicate(i) => {
            if let Some(n) = td::duplicate_detail(cx, i) {
                let name = cx.project.floors[n].name.clone();
                state(|s| s.selected = Some(name));
            }
        }
        Act::Delete(i) => {
            if td::delete_detail(cx, i) {
                state(|s| s.selected = None);
            }
        }
        Act::ToLayout(i) => {
            if let Err(e) = td::send_to_layout(cx, i) {
                cx.status = e;
            }
        }
        Act::New => {
            let i = td::new_detail(cx);
            let name = cx.project.floors[i].name.clone();
            state(|s| s.selected = Some(name));
        }
        Act::MakeDetail => {
            let (cam, opts) = state(|s| (s.camera, s.options.clone()));
            let Some(cam) = cam else {
                return;
            };
            match td::auto_detail(cx, cam, &opts) {
                Ok(i) => {
                    let name = cx.project.floors[i].name.clone();
                    state(|s| {
                        s.auto = false;
                        s.selected = Some(name);
                    });
                    td::open_detail(cx, i);
                }
                Err(e) => cx.status = e,
            }
        }
        Act::PlaceByClick(id) => {
            td::arm(Some(id));
            cx.requests
                .push(EditorRequest::SetTool(ToolId::DetailsVariant(
                    DetailsVariant::Component,
                )));
        }
        Act::InsertNow(id) => {
            if let Some(c) = td::find_component(id) {
                // The middle of what the floor already holds, else the origin.
                let at = cx.floor().cad_extent().map_or(Point::ZERO, |(lo, hi)| {
                    Point::new((lo.x + hi.x) * 0.5, lo.y - 24.0)
                });
                td::place_component(cx, &c, at);
            }
        }
    }
}

fn management(ctx: &egui::Context, cx: &mut EditorContext, acts: &mut Vec<Act>) {
    let mut open = true;
    egui::Window::new("CAD Detail Management")
        .open(&mut open)
        .default_width(540.0)
        .show(ctx, |ui| {
            let details = cx.project.cad_detail_floors();
            if details.is_empty() {
                ui.weak("No CAD details yet. Make one with Auto Detail or CAD Detail From View, or start a blank one.");
            } else {
                egui::Grid::new("detail_list")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Name");
                        ui.strong("Source");
                        ui.strong("Objects");
                        ui.strong("Scale");
                        ui.end_row();
                        for &i in &details {
                            let f = &cx.project.floors[i];
                            let is_sel = state(|s| s.selected.as_deref() == Some(f.name.as_str()));
                            if ui.selectable_label(is_sel, &f.name).clicked() {
                                let n = f.name.clone();
                                state(|s| s.selected = Some(n));
                            }
                            ui.label(source_text(cx, i));
                            ui.label(f.cad.len().to_string());
                            ui.label(td::scale_label(f.detail.as_ref().map_or(1.5, |d| d.scale)));
                            ui.end_row();
                        }
                    });
            }
            ui.separator();
            // The selected detail.
            let sel = state(|s| s.selected.clone())
                .and_then(|n| details.iter().copied().find(|&i| cx.project.floors[i].name == n));
            match sel {
                Some(i) => {
                    let name = cx.project.floors[i].name.clone();
                    state(|s| {
                        if s.rename_for.as_deref() != Some(name.as_str()) {
                            s.rename = name.clone();
                            s.rename_for = Some(name.clone());
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        let mut text = state(|s| s.rename.clone());
                        let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(220.0));
                        state(|s| s.rename = text.clone());
                        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.button("Rename").clicked() || enter {
                            acts.push(Act::Rename(i, text));
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .button("Open")
                            .on_hover_text("Show the detail in a tab of its own")
                            .clicked()
                        {
                            acts.push(Act::Open(i));
                        }
                        if ui.button("Duplicate").clicked() {
                            acts.push(Act::Duplicate(i));
                        }
                        if ui.button("Delete").clicked() {
                            acts.push(Act::Delete(i));
                        }
                        if ui
                            .button("Send to Layout")
                            .on_hover_text("Put the detail on the last page of the layout")
                            .clicked()
                        {
                            acts.push(Act::ToLayout(i));
                        }
                    });
                }
                None => {
                    ui.weak("Pick a detail to open, rename, duplicate, delete or send to layout.");
                }
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("New Detail").clicked() {
                    acts.push(Act::New);
                }
                if ui.button("Auto Detail\u{2026}").clicked() {
                    open_auto_detail(cx);
                }
                if ui.button("Detail Components\u{2026}").clicked() {
                    open_components();
                }
            });
        });
    if !open {
        state(|s| s.management = false);
    }
}

fn auto_window(ctx: &egui::Context, cx: &mut EditorContext, acts: &mut Vec<Act>) {
    let mut open = true;
    egui::Window::new("Auto Detail")
        .open(&mut open)
        .collapsible(false)
        .default_width(360.0)
        .show(ctx, |ui| {
            let cams = td::eligible_cameras(&cx.project);
            if cams.is_empty() {
                ui.weak("There is no cross section or elevation in the plan.");
                return;
            }
            let chosen = state(|s| s.camera).unwrap_or(cams[0].0);
            let chosen_name = cams
                .iter()
                .find(|c| c.0 == chosen)
                .map_or(String::new(), |c| c.1.clone());
            ui.horizontal(|ui| {
                ui.label("Section or elevation");
                egui::ComboBox::from_id_salt("auto_detail_camera")
                    .selected_text(chosen_name)
                    .show_ui(ui, |ui| {
                        for (id, name) in &cams {
                            if ui.selectable_label(*id == chosen, name).clicked() {
                                let id = *id;
                                state(|s| s.camera = Some(id));
                            }
                        }
                    });
            });
            let mut o = state(|s| s.options.clone());
            ui.horizontal(|ui| {
                ui.label("Scale");
                egui::ComboBox::from_id_salt("auto_detail_scale")
                    .selected_text(td::scale_label(o.scale))
                    .show_ui(ui, |ui| {
                        for s in [0.75, 1.0, 1.5, 3.0] {
                            ui.selectable_value(&mut o.scale, s, td::scale_label(s));
                        }
                    });
            });
            ui.checkbox(&mut o.hatch, "Material hatch");
            ui.checkbox(&mut o.framing, "Framing members (or assumed plates)");
            ui.checkbox(&mut o.insulation, "Insulation in wall cavities");
            ui.checkbox(&mut o.notes, "Title, scale and layer names");
            state(|s| s.options = o);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Create Detail").clicked() {
                    acts.push(Act::MakeDetail);
                }
                if ui.button("Cancel").clicked() {
                    state(|s| s.auto = false);
                }
            });
        });
    if !open {
        state(|s| s.auto = false);
    }
}

/// Paints a component's parts scaled into `rect`.
fn preview(painter: &egui::Painter, rect: Rect, c: &td::Component) {
    let (lo, hi) = c.bounds();
    let (w, h) = ((hi.x - lo.x).max(0.1), (hi.y - lo.y).max(0.1));
    let k = ((rect.width() as f64 - 16.0) / w)
        .min((rect.height() as f64 - 16.0) / h)
        .max(0.01);
    let origin = Pos2::new(
        rect.center().x - (w * k / 2.0) as f32,
        rect.center().y + (h * k / 2.0) as f32,
    );
    let to = |p: Point| {
        Pos2::new(
            origin.x + ((p.x - lo.x) * k) as f32,
            origin.y - ((p.y - lo.y) * k) as f32,
        )
    };
    let stroke = Stroke::new(1.2_f32, Color32::from_gray(60));
    for p in &c.parts {
        match &p.item {
            CadItem::Line { a, b } => {
                painter.line_segment([to(*a), to(*b)], stroke);
            }
            CadItem::Polyline { points, closed } => {
                let mut pts: Vec<Pos2> = points.iter().map(|q| to(*q)).collect();
                if *closed && !pts.is_empty() {
                    pts.push(pts[0]);
                }
                painter.add(egui::Shape::line(pts, stroke));
            }
            CadItem::Circle { center, radius } => {
                painter.circle_stroke(to(*center), (*radius * k) as f32, stroke);
            }
            _ => {}
        }
    }
}

fn components_window(ctx: &egui::Context, cx: &mut EditorContext, acts: &mut Vec<Act>) {
    let mut open = true;
    egui::Window::new("Detail Components")
        .open(&mut open)
        .default_width(520.0)
        .show(ctx, |ui| {
            let cats = ComponentCategory::ALL;
            let mut cat = state(|s| s.category).min(cats.len() - 1);
            ui.horizontal_wrapped(|ui| {
                for (i, c) in cats.iter().enumerate() {
                    if ui.selectable_label(cat == i, c.name()).clicked() {
                        cat = i;
                    }
                }
            });
            state(|s| s.category = cat);
            let list: Vec<td::Component> = td::catalogue()
                .into_iter()
                .filter(|c| c.category == cats[cat])
                .collect();
            let mut sel = state(|s| s.component).filter(|id| list.iter().any(|c| c.id == *id));
            if sel.is_none() {
                sel = list.first().map(|c| c.id);
            }
            ui.separator();
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);
                    for c in &list {
                        if ui.selectable_label(sel == Some(c.id), c.name).clicked() {
                            sel = Some(c.id);
                        }
                    }
                });
                ui.vertical(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(220.0, 180.0), egui::Sense::hover());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 2.0, Color32::from_gray(250));
                    painter.rect_stroke(
                        rect,
                        2.0,
                        Stroke::new(1.0_f32, Color32::from_gray(160)),
                        egui::StrokeKind::Inside,
                    );
                    if let Some(c) = sel.and_then(td::find_component) {
                        preview(&painter, rect, &c);
                        let (w, h) = c.size();
                        ui.label(format!("{} x {}", cx.fmt_dim(w), cx.fmt_dim(h)));
                    }
                });
            });
            state(|s| s.component = sel);
            ui.separator();
            ui.horizontal(|ui| {
                let id = sel.and_then(td::find_component).map(|c| c.id);
                ui.add_enabled_ui(id.is_some(), |ui| {
                    if ui
                        .button("Place by Clicking")
                        .on_hover_text("Click in the plan to place it; Esc stops")
                        .clicked()
                    {
                        if let Some(id) = id {
                            acts.push(Act::PlaceByClick(id));
                        }
                    }
                    if ui
                        .button("Insert Below the Drawing")
                        .on_hover_text("Put it under what the floor already holds")
                        .clicked()
                    {
                        if let Some(id) = id {
                            acts.push(Act::InsertNow(id));
                        }
                    }
                });
            });
        });
    if !open {
        state(|s| s.components = false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_defaults;

    fn new_cx() -> EditorContext {
        EditorContext::new(plan_defaults::embedded())
    }

    fn frame(ctx: &egui::Context, cx: &mut EditorContext) {
        let _ = ctx.run(egui::RawInput::default(), |ctx| show_windows(ctx, cx));
    }

    #[test]
    fn the_windows_draw_headlessly_with_and_without_details() {
        let mut cx = new_cx();
        let ctx = egui::Context::default();
        open_management();
        open_components();
        frame(&ctx, &mut cx);
        cx.begin_change("New");
        let i = cx
            .project
            .add_cad_detail("Sill", plan_core::details::CadDetailInfo::default());
        cx.project.add_cad(
            i,
            plan_core::details::DETAIL_LINES_LAYER,
            CadItem::Line {
                a: Point::ZERO,
                b: Point::new(10.0, 0.0),
            },
        );
        select_detail(Some("Sill"));
        frame(&ctx, &mut cx);
        frame(&ctx, &mut cx);
        // Auto Detail with no camera stays closed and says why.
        open_auto_detail(&mut cx);
        assert!(cx.status.contains("cross section"), "{}", cx.status);
        state(|s| {
            s.management = false;
            s.components = false;
        });
    }
}
